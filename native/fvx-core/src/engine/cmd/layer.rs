use serde_json::{Value as Json, json};

use super::{add_layer, footage_layer, select_only, solid_item, unique_layer_name, vec2_param};
use crate::engine::{Engine, Res, TextEdit, pb, pf, ps, pu};
use crate::eval::Eval;
use crate::model::prop::{self, GroupKind, Node};
use crate::model::value::{apply_text_attrs, parse_color};
use crate::model::{AutoOrient, BezPath, BlendMode, ItemKind, Justify, LayerInit, LayerKind, MaskMode, Matte, MatteKind, Quality, TextDoc, Value, build_layer, label_rgb, set_text};

pub fn comp_dims(e: &Engine, cid: u64) -> Result<(u32, u32, f64, f64), String> {
    let c = e.project.comp(cid).ok_or("no composition")?;
    Ok((c.width, c.height, c.duration, c.frame_rate))
}

pub fn finish(e: &mut Engine, cid: u64, l: crate::model::Layer, label: &str) -> Res {
    let id = add_layer(e, cid, l, None);
    select_only(e, &[id]);
    e.set_label(label);
    let index = e.project.comp(cid).and_then(|c| c.index_of(id)).map(|i| i + 1);
    Ok(json!({"layer": id, "index": index}))
}

pub fn new_solid(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let (cw, ch, dur, _) = comp_dims(e, cid)?;
    let c = p.get("color").and_then(parse_color).unwrap_or([0.5, 0.5, 0.5, 1.0]);
    let w = pu(p, "width").unwrap_or(cw as u64) as u32;
    let h = pu(p, "height").unwrap_or(ch as u64) as u32;
    let n = e.project.comp(cid).map(|c| c.layers.iter().filter(|l| l.kind == LayerKind::Solid).count()).unwrap_or(0) + 1;
    let name = ps(p, "name").map(str::to_string).unwrap_or_else(|| format!("Solid {n}"));
    let item = solid_item(e, &name, [c[0], c[1], c[2]], w.max(1), h.max(1));
    let l = build_layer(e.proj(), (cw, ch), LayerInit { kind: LayerKind::Solid, name, source: Some(item), size: (w as f64, h as f64), start: 0.0, duration: dur });
    finish(e, cid, l, "New Solid")
}

pub fn new_adjustment(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let (cw, ch, dur, _) = comp_dims(e, cid)?;
    let n = e.project.comp(cid).map(|c| c.layers.iter().filter(|l| l.switches.adjustment).count()).unwrap_or(0) + 1;
    let name = ps(p, "name").map(str::to_string).unwrap_or_else(|| format!("Adjustment Layer {n}"));
    let item = solid_item(e, &name, [1.0, 1.0, 1.0], cw, ch);
    let mut l = build_layer(e.proj(), (cw, ch), LayerInit { kind: LayerKind::Solid, name, source: Some(item), size: (cw as f64, ch as f64), start: 0.0, duration: dur });
    l.switches.adjustment = true;
    l.label = 5;
    finish(e, cid, l, "New Adjustment Layer")
}

pub fn new_null(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let (cw, ch, dur, _) = comp_dims(e, cid)?;
    let base = ps(p, "name").unwrap_or("Null 1");
    let name = unique_layer_name(e.project.comp(cid).unwrap(), base);
    let mut l = build_layer(e.proj(), (cw, ch), LayerInit { kind: LayerKind::Null, name, source: None, size: (100.0, 100.0), start: 0.0, duration: dur });
    if let Some(pos) = vec2_param(p, "position")
        && let Some(pr) = l.props.sub_mut("transform").and_then(|g| g.get_mut("position"))
    {
        pr.value = Value::Vec3([pos[0], pos[1], 0.0]);
    }
    finish(e, cid, l, "New Null Object")
}

pub fn shape_contents(e: &mut Engine, kind: &str, size: [f64; 2], fill: Option<[f64; 4]>, stroke: Option<[f64; 4]>, stroke_width: f64) -> Option<crate::model::Group> {
    let pj = e.proj();
    let mut ids = pj.ids();
    let (pk, name) = match kind {
        "rect" | "rounded" => ("rect", "Rectangle 1"),
        "ellipse" => ("ellipse", "Ellipse 1"),
        "star" => ("star", "Polystar 1"),
        "polygon" => ("polygon", "Polystar 1"),
        _ => return None,
    };
    let mut path = prop::shape_item(&mut ids, pk, None)?;
    match pk {
        "rect" | "ellipse" => {
            if let Some(s) = path.get_mut("size") {
                s.value = Value::Vec2(size);
            }
            if kind == "rounded"
                && let Some(r) = path.get_mut("roundness")
            {
                r.value = Value::Num(size[0].abs().min(size[1].abs()) * 0.15);
            }
        }
        _ => {
            let r = size[0].abs().min(size[1].abs()) / 2.0;
            if let Some(o) = path.get_mut("outerRadius") {
                o.value = Value::Num(r);
            }
            if let Some(i) = path.get_mut("innerRadius") {
                i.value = Value::Num(r * 0.5);
            }
        }
    }
    let mut contents = vec![Node::Group(path)];
    if let Some(sc) = stroke.filter(|_| stroke_width > 0.0) {
        let mut st = prop::shape_item(&mut ids, "stroke", None)?;
        if let Some(c) = st.get_mut("color") {
            c.value = Value::Color(sc);
        }
        if let Some(w) = st.get_mut("width") {
            w.value = Value::Num(stroke_width);
        }
        contents.push(Node::Group(st));
    }
    if let Some(fc) = fill {
        let mut f = prop::shape_item(&mut ids, "fill", None)?;
        if let Some(c) = f.get_mut("color") {
            c.value = Value::Color(fc);
        }
        contents.push(Node::Group(f));
    }
    let mut grp = prop::shape_item(&mut ids, "group", Some(name))?;
    if let Some(cg) = grp.sub_mut("contents") {
        cg.children = contents;
    }
    Some(grp)
}

fn paint_param(p: &Json, k: &str) -> Option<Option<[f64; 4]>> {
    match p.get(k) {
        None => None,
        Some(Json::Null) | Some(Json::Bool(false)) => Some(None),
        Some(v) => Some(parse_color(v)),
    }
}

pub fn new_shape(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let (cw, ch, dur, _) = comp_dims(e, cid)?;
    let kind = ps(p, "kind").unwrap_or("none").to_string();
    let n = e.project.comp(cid).map(|c| c.layers.iter().filter(|l| l.kind == LayerKind::Shape).count()).unwrap_or(0) + 1;
    let name = ps(p, "name").map(str::to_string).unwrap_or_else(|| format!("Shape Layer {n}"));
    let mut l = build_layer(e.proj(), (cw, ch), LayerInit { kind: LayerKind::Shape, name, source: None, size: (cw as f64, ch as f64), start: 0.0, duration: dur });
    let size = vec2_param(p, "size").unwrap_or([200.0, 200.0]);
    let fill = paint_param(p, "fill").unwrap_or(Some([0.98, 0.45, 0.09, 1.0]));
    let stroke = paint_param(p, "stroke").flatten().or(Some([1.0, 1.0, 1.0, 1.0]));
    let sw = pf(p, "strokeWidth").unwrap_or(if p.get("stroke").is_some_and(|s| !s.is_null() && s != &Json::Bool(false)) { 2.0 } else { 0.0 });
    if kind != "none"
        && let Some(grp) = shape_contents(e, &kind, size, fill, stroke, sw)
        && let Some(c) = l.props.sub_mut("contents")
    {
        c.children.push(Node::Group(grp));
    }
    if let Some(pos) = vec2_param(p, "position")
        && let Some(pr) = l.props.sub_mut("transform").and_then(|g| g.get_mut("position"))
    {
        pr.value = Value::Vec3([pos[0], pos[1], 0.0]);
    }
    finish(e, cid, l, "New Shape Layer")
}

pub fn new_text(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let (cw, ch, dur, _) = comp_dims(e, cid)?;
    let mut doc = TextDoc::default();
    let has_pos = p.get("position").is_some() || p.get("box").is_some();
    doc.justify = if has_pos { Justify::Left } else { Justify::Center };
    apply_text_attrs(&mut doc, p)?;
    let text = ps(p, "text").unwrap_or("Text").to_string();
    doc.text = text.clone();
    let mut boxed: Option<[f64; 4]> = None;
    if let Some(b) = p.get("box").and_then(Json::as_array) {
        let v: Vec<f64> = b.iter().filter_map(Json::as_f64).collect();
        if v.len() == 4 {
            boxed = Some([v[0], v[1], v[2], v[3]]);
        }
    }
    let name = ps(p, "name").map(str::to_string).unwrap_or_else(|| {
        let t = text.lines().next().unwrap_or("").trim();
        if t.is_empty() { "Text".into() } else { t.chars().take(40).collect() }
    });
    let mut l = build_layer(e.proj(), (cw, ch), LayerInit { kind: LayerKind::Text, name, source: None, size: (cw as f64, ch as f64), start: 0.0, duration: dur });
    let pos = match boxed {
        Some(b) => {
            doc.box_size = Some([0.0, 0.0, b[2], b[3]]);
            [b[0], b[1]]
        }
        None => vec2_param(p, "position").unwrap_or([cw as f64 / 2.0, ch as f64 / 2.0 + doc.size * 0.35]),
    };
    if let Some(pr) = l.props.sub_mut("transform").and_then(|g| g.get_mut("position")) {
        pr.value = Value::Vec3([pos[0], pos[1], 0.0]);
    }
    set_text(&mut l, doc);
    let r = finish(e, cid, l, "New Text Layer")?;
    if pb(p, "edit").unwrap_or(false)
        && let Some(id) = r.get("layer").and_then(Json::as_u64)
    {
        let n = text.chars().count();
        e.state.text_edit = Some(TextEdit { layer: id, anchor: 0, caret: n });
    }
    Ok(r)
}

pub fn new_camera(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let (cw, ch, dur, _) = comp_dims(e, cid)?;
    let base = ps(p, "name").unwrap_or("Camera 1");
    let name = unique_layer_name(e.project.comp(cid).unwrap(), base);
    let mut l = build_layer(e.proj(), (cw, ch), LayerInit { kind: LayerKind::Camera, name, source: None, size: (0.0, 0.0), start: 0.0, duration: dur });
    let preset = ps(p, "preset").and_then(|s| s.trim_end_matches("mm").parse::<f64>().ok());
    let focal = pf(p, "focalLength").or(preset).unwrap_or(50.0);
    let zoom = pf(p, "zoom").or_else(|| pf(p, "angleOfView").map(|a| cw as f64 / 2.0 / (a.to_radians() / 2.0).tan().max(1e-6))).unwrap_or(cw as f64 * focal / 36.0);
    if let Some(g) = l.props.sub_mut("cameraOptions") {
        if let Some(z) = g.get_mut("zoom") {
            z.value = Value::Num(zoom);
        }
        if let Some(f) = g.get_mut("focusDistance") {
            f.value = Value::Num(zoom);
        }
    }
    if let Some(pr) = l.props.sub_mut("transform").and_then(|g| g.get_mut("position")) {
        pr.value = Value::Vec3([cw as f64 / 2.0, ch as f64 / 2.0, -zoom]);
    }
    let mut rest = p.clone();
    if let Some(o) = rest.as_object_mut() {
        for k in ["zoom", "focalLength", "preset", "angleOfView", "name"] {
            o.remove(k);
        }
    }
    super::three::apply_camera(&mut l, &rest, cw as f64, None)?;
    finish(e, cid, l, "New Camera")
}

pub fn new_light(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let (cw, ch, dur, _) = comp_dims(e, cid)?;
    let base = ps(p, "name").unwrap_or("Light 1");
    let name = unique_layer_name(e.project.comp(cid).unwrap(), base);
    let mut l = build_layer(e.proj(), (cw, ch), LayerInit { kind: LayerKind::Light, name, source: None, size: (0.0, 0.0), start: 0.0, duration: dur });
    if let Some(t) = l.props.sub_mut("lightOptions").and_then(|g| g.get_mut("lightType")) {
        t.value = Value::Enum(1);
    }
    let mut rest = p.clone();
    if let Some(o) = rest.as_object_mut() {
        o.remove("name");
    }
    super::three::apply_light(&mut l, &rest, None)?;
    finish(e, cid, l, "New Light")
}

pub fn add_item(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let item = match p.get("item") {
        Some(Json::Number(n)) => n.as_u64().ok_or("bad item")?,
        Some(Json::String(s)) => s.parse::<u64>().ok().filter(|x| e.project.item(*x).is_some()).or_else(|| e.project.items.iter().find(|i| i.name == *s).map(|i| i.id)).ok_or_else(|| format!("no project item \"{s}\""))?,
        _ => *e.state.project_selection.first().ok_or("pass `item` (a project item id or name)")?,
    };
    let time = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let l = footage_layer(e, cid, item, time, vec2_param(p, "position"), pf(p, "duration"))?;
    let idx = pu(p, "index").map(|i| (i as usize).saturating_sub(1));
    let id = add_layer(e, cid, l, idx);
    select_only(e, &[id]);
    e.set_label("Add Footage to Comp");
    Ok(json!({"layer": id}))
}

pub fn select(e: &mut Engine, p: &Json) -> Res {
    let (_, ids) = e.layers_param(p)?;
    let toggle = pb(p, "toggle").unwrap_or(false);
    let add = pb(p, "add").unwrap_or(false);
    if toggle {
        for id in ids {
            if let Some(i) = e.state.selected_layers.iter().position(|x| *x == id) {
                e.state.selected_layers.remove(i);
            } else {
                e.state.selected_layers.push(id);
            }
        }
    } else if add {
        for id in ids {
            if !e.state.selected_layers.contains(&id) {
                e.state.selected_layers.push(id);
            }
        }
    } else {
        e.state.selected_layers = ids;
        e.state.selected_props.retain(|(l, _)| e.state.selected_layers.contains(l));
        e.state.selected_vertices.retain(|v| e.state.selected_layers.contains(&v.layer));
    }
    if let Some(te) = &e.state.text_edit
        && !e.state.selected_layers.contains(&te.layer)
    {
        e.state.text_edit = None;
    }
    Ok(json!({"layers": e.state.selected_layers}))
}

pub fn rename(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let name = ps(p, "name").ok_or("pass `name`")?.to_string();
    e.layer_mut(cid, lid)?.name = name.clone();
    Ok(json!({"layer": lid, "name": name}))
}

pub fn set_comment(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select layers or pass `layers`".into());
    }
    let text = ps(p, "comment").ok_or("pass `comment`")?.to_string();
    for id in &ids {
        e.layer_mut(cid, *id)?.comment = text.clone();
    }
    e.set_label("Layer Comment");
    Ok(json!({"layers": ids, "comment": text}))
}

pub fn copy_to_comp(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select layers or pass `layers`".into());
    }
    let target = e.comp_id(p.get("target"))?;
    if target == cid {
        return Err("the target is the same composition (use edit.duplicate)".into());
    }
    let src = e.project.comp(cid).ok_or("no comp")?.clone();
    let mut map = std::collections::HashMap::new();
    let mut copies = vec![];
    for l in src.layers.iter().filter(|l| ids.contains(&l.id)) {
        let pj = e.proj();
        let mut nl = l.clone();
        nl.id = pj.new_id();
        map.insert(l.id, nl.id);
        nl.props.renumber(&mut pj.next_uid);
        copies.push(nl);
    }
    for nl in copies.iter_mut() {
        nl.parent = nl.parent.and_then(|x| map.get(&x).copied());
        nl.track_matte = nl.track_matte.and_then(|m| map.get(&m.layer).map(|x| crate::model::Matte { layer: *x, kind: m.kind }));
    }
    let made: Vec<u64> = copies.iter().map(|l| l.id).collect();
    let c = e.proj().comp_mut(target).ok_or("no target comp")?;
    for (k, nl) in copies.into_iter().enumerate() {
        c.layers.insert(k.min(c.layers.len()), nl);
    }
    e.set_label("Copy to Composition");
    Ok(json!({"comp": target, "layers": made}))
}

pub fn set_switch(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select layers or pass `layers`".into());
    }
    let sw = ps(p, "switch").ok_or("pass `switch`")?.to_string();
    let val = pb(p, "value");
    let first = e.layer_ref(cid, ids[0])?.clone();
    let cur = |l: &crate::model::Layer| -> bool {
        let s = &l.switches;
        match sw.as_str() {
            "video" => s.video,
            "audio" => s.audio,
            "solo" => s.solo,
            "lock" | "locked" => s.locked,
            "shy" => s.shy,
            "collapse" => s.collapse,
            "quality" => s.quality == Quality::Best,
            "fx" | "effects" => s.effects,
            "frameBlend" => s.frame_blend != crate::model::FrameBlend::Off,
            "motionBlur" => s.motion_blur,
            "adjustment" => s.adjustment,
            "threeD" | "3d" => s.three_d,
            "guide" => s.guide,
            "preserveTransparency" => l.preserve_transparency,
            _ => false,
        }
    };
    let known = ["video", "audio", "solo", "lock", "locked", "shy", "collapse", "quality", "fx", "effects", "frameBlend", "motionBlur", "adjustment", "threeD", "3d", "guide", "preserveTransparency"];
    if !known.contains(&sw.as_str()) {
        return Err(format!("unknown switch `{sw}` ({})", known.join(", ")));
    }
    let v = val.unwrap_or(!cur(&first));
    for id in &ids {
        let l = e.layer_mut(cid, *id)?;
        if matches!(l.kind, LayerKind::Camera | LayerKind::Light) && matches!(sw.as_str(), "threeD" | "3d" | "collapse" | "adjustment") {
            continue;
        }
        let s = &mut l.switches;
        match sw.as_str() {
            "video" => s.video = v,
            "audio" => s.audio = v,
            "solo" => s.solo = v,
            "lock" | "locked" => s.locked = v,
            "shy" => s.shy = v,
            "collapse" => s.collapse = v,
            "quality" => s.quality = if v { Quality::Best } else { Quality::Draft },
            "fx" | "effects" => s.effects = v,
            "frameBlend" => s.frame_blend = if v { crate::model::FrameBlend::FrameMix } else { crate::model::FrameBlend::Off },
            "motionBlur" => s.motion_blur = v,
            "adjustment" => s.adjustment = v,
            "threeD" | "3d" => s.three_d = v,
            "guide" => s.guide = v,
            _ => l.preserve_transparency = v,
        }
    }
    if sw == "motionBlur" && v
        && let Some(c) = e.proj().comp_mut(cid)
    {
        c.motion_blur = true;
    }
    Ok(json!({"switch": sw, "value": v, "layers": ids}))
}

pub fn set_parent(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let parent = match p.get("parent") {
        None | Some(Json::Null) => None,
        Some(v) => Some(e.layer_in(e.project.comp(cid).unwrap(), v)?),
    };
    let ev_proj = e.project.clone();
    let comp = ev_proj.comp(cid).ok_or("no comp")?;
    let ev = Eval::new(&ev_proj, cid, comp, e.time_of(cid));
    for id in &ids {
        if Some(*id) == parent {
            continue;
        }
        let mut cur = parent;
        let mut cyc = false;
        let mut guard = 0;
        while let Some(c) = cur {
            if c == *id {
                cyc = true;
                break;
            }
            guard += 1;
            if guard > 64 {
                break;
            }
            cur = comp.layer(c).and_then(|l| l.parent);
        }
        if cyc {
            return Err("that parent would create a loop".into());
        }
        let l = comp.layer(*id).ok_or("no layer")?;
        let world = ev.world_matrix(l);
        let new_parent_world = parent.and_then(|pid| comp.layer(pid)).map(|pl| ev.world_matrix(pl)).unwrap_or_default();
        let local = new_parent_world.inverse().map(|inv| inv * world);
        let lm = e.layer_mut(cid, *id)?;
        lm.parent = parent;
        if let Some(local) = local
            && !lm.props.sub("transform").and_then(|t| t.get("position")).is_some_and(|q| q.is_animated())
        {
            let m = &local.0;
            let anchor = lm.props.sub("transform").and_then(|t| t.get("anchor")).map(|a| a.value.vec3()).unwrap_or([0.0; 3]);
            let pos = local.apply(anchor);
            let sx = (m[0][0] * m[0][0] + m[1][0] * m[1][0] + m[2][0] * m[2][0]).sqrt();
            let sy = (m[0][1] * m[0][1] + m[1][1] * m[1][1] + m[2][1] * m[2][1]).sqrt();
            let rot = m[1][0].atan2(m[0][0]).to_degrees();
            if let Some(t) = lm.props.sub_mut("transform") {
                if let Some(q) = t.get_mut("position") {
                    q.value = Value::Vec3(pos);
                }
                if let Some(q) = t.get_mut("scale")
                    && !q.is_animated()
                {
                    let z = q.value.vec3()[2];
                    q.value = Value::Vec3([sx * 100.0, sy * 100.0, z]);
                }
                if let Some(q) = t.get_mut("rotation")
                    && !q.is_animated()
                {
                    q.value = Value::Num(rot);
                }
            }
        }
    }
    Ok(json!({"layers": ids, "parent": parent}))
}

pub fn set_blend(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let m = ps(p, "mode").ok_or("pass `mode`")?;
    let mode = BlendMode::parse(m).ok_or_else(|| format!("unknown blending mode `{m}`"))?;
    for id in &ids {
        e.layer_mut(cid, *id)?.blend = mode;
    }
    Ok(json!({"mode": mode.label()}))
}

pub fn set_matte(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let matte = match p.get("matte") {
        None | Some(Json::Null) => None,
        Some(v) => Some(e.layer_in(e.project.comp(cid).unwrap(), v)?),
    };
    if matte == Some(lid) {
        return Err("a layer cannot be its own track matte".into());
    }
    let kind = ps(p, "kind").map(|k| MatteKind::parse(k).ok_or_else(|| format!("unknown matte kind `{k}`"))).transpose()?.unwrap_or(MatteKind::Alpha);
    e.layer_mut(cid, lid)?.track_matte = matte.map(|m| Matte { layer: m, kind });
    if let Some(m) = matte {
        e.layer_mut(cid, m)?.switches.video = false;
    }
    Ok(json!({"layer": lid, "matte": matte, "kind": kind.label()}))
}

pub fn timing(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let now = e.time_of(cid);
    let fd = e.project.comp(cid).map(|c| c.frame_duration()).unwrap_or(1.0 / 30.0);
    for id in &ids {
        let l = e.layer_mut(cid, *id)?;
        match ps(p, "op") {
            Some("moveInToTime") => {
                let d = now - l.in_point;
                l.in_point += d;
                l.out_point += d;
                l.start_time += d;
            }
            Some("moveOutToTime") => {
                let d = now - l.out_point;
                l.in_point += d;
                l.out_point += d;
                l.start_time += d;
            }
            Some("trimInToTime") => l.in_point = now.min(l.out_point - fd),
            Some("trimOutToTime") => l.out_point = (now + fd).max(l.in_point + fd),
            _ => {}
        }
        if let Some(d) = pf(p, "delta") {
            l.in_point += d;
            l.out_point += d;
            l.start_time += d;
        }
        if let Some(s) = pf(p, "start") {
            let d = s - l.start_time;
            l.in_point += d;
            l.out_point += d;
            l.start_time = s;
        }
        if let Some(i) = pf(p, "in") {
            l.in_point = i.min(l.out_point - fd);
        }
        if let Some(o) = pf(p, "out") {
            l.out_point = o.max(l.in_point + fd);
        }
        if let Some(st) = pf(p, "stretch").filter(|x| x.abs() > 0.1) {
            let len = (l.out_point - l.in_point) * st / l.stretch;
            l.stretch = st;
            l.out_point = l.in_point + len;
        }
    }
    Ok(json!({"layers": ids}))
}

pub fn add_mask(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let (cw, ch) = {
        let c = e.project.comp(cid).ok_or("no comp")?;
        let l = c.layer(lid).ok_or("no layer")?;
        e.project.source_size(l, c)
    };
    let rect = p.get("rect").and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).collect::<Vec<_>>()).filter(|v| v.len() == 4).unwrap_or(vec![0.0, 0.0, cw, ch]);
    let shape = ps(p, "shape").unwrap_or("rect");
    let path = match p.get("path") {
        Some(j) => crate::model::value::parse_path(j).ok_or("bad `path`")?,
        None => {
            let (cx, cy) = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
            if shape == "ellipse" { BezPath::ellipse(cx, cy, rect[2], rect[3]) } else { BezPath::polygon(&[[rect[0], rect[1]], [rect[0] + rect[2], rect[1]], [rect[0] + rect[2], rect[1] + rect[3]], [rect[0], rect[1] + rect[3]]], true) }
        }
    };
    let mode = ps(p, "mode").map(|m| MaskMode::parse(m).ok_or_else(|| format!("unknown mask mode `{m}`"))).transpose()?.unwrap_or(MaskMode::Add);
    let count = e.layer_ref(cid, lid)?.masks().map(|m| m.children.len()).unwrap_or(0);
    let color = label_rgb(((count % 15) + 1) as u8);
    let pj = e.proj();
    let mut ids = pj.ids();
    let g = prop::mask_group(&mut ids, &format!("Mask {}", count + 1), path, mode, color);
    let uid = g.uid;
    let l = e.layer_mut(cid, lid)?;
    if l.props.sub("masks").is_none() {
        return Err("this layer cannot have masks".into());
    }
    l.props.sub_mut("masks").unwrap().children.push(Node::Group(g));
    e.set_label("New Mask");
    Ok(json!({"layer": lid, "mask": uid, "index": count + 1}))
}

pub fn set_mask(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    let masks = l.masks().ok_or("no masks")?;
    let uid = match p.get("mask") {
        Some(Json::Number(n)) => {
            let n = n.as_u64().unwrap_or(0);
            if masks.groups().any(|g| g.uid == n) { n } else { masks.groups().nth((n as usize).wrapping_sub(1)).map(|g| g.uid).ok_or("no such mask")? }
        }
        Some(Json::String(s)) => masks.groups().find(|g| g.name == *s).map(|g| g.uid).ok_or("no such mask")?,
        _ => masks.groups().next().map(|g| g.uid).ok_or("no masks")?,
    };
    let mode = ps(p, "mode").map(|m| MaskMode::parse(m).ok_or_else(|| format!("unknown mask mode `{m}`"))).transpose()?;
    let inv = pb(p, "inverted");
    let locked = pb(p, "locked");
    let name = ps(p, "name").map(str::to_string);
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no such mask")?;
    if let Some(n) = name {
        g.name = n;
    }
    if let GroupKind::Mask { mode: m, inverted, locked: lk, .. } = &mut g.kind {
        if let Some(x) = mode {
            *m = x;
        }
        if let Some(x) = inv {
            *inverted = x;
        }
        if let Some(x) = locked {
            *lk = x;
        }
    }
    Ok(json!({"mask": uid}))
}

pub fn set_text_cmd(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = match e.layer_param(p) {
        Ok(x) => x,
        Err(_) => {
            let te = e.state.text_edit.clone().ok_or("select a text layer")?;
            (e.comp_id(None)?, te.layer)
        }
    };
    let now = e.time_of(cid);
    let l = e.layer_ref(cid, lid)?;
    if l.kind != LayerKind::Text {
        return Err(format!("{} is not a text layer", l.name));
    }
    let lt = l.layer_time(now);
    let prop = l.text_prop().ok_or("no text")?.clone();
    let mut doc = prop.at(lt).as_text().cloned().ok_or("no text")?;
    apply_text_attrs(&mut doc, p)?;
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.sub_mut("text").and_then(|g| g.get_mut("sourceText")).ok_or("no text")?;
    if pr.keys.is_empty() {
        pr.value = Value::Text(Box::new(doc));
    } else {
        crate::model::key::insert(&mut pr.keys, crate::model::Key { in_interp: crate::model::Interp::Hold, out_interp: crate::model::Interp::Hold, ..crate::model::Key::new(lt, Value::Text(Box::new(doc)), false) });
    }
    e.set_label("Edit Text");
    Ok(json!({"layer": lid}))
}

pub fn precompose(e: &mut Engine, p: &Json) -> Res {
    let (cid, mut ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select the layers to precompose".into());
    }
    let comp = e.project.comp(cid).ok_or("no comp")?.clone();
    ids.sort_by_key(|id| comp.index_of(*id).unwrap_or(0));
    let n = e.project.items.iter().filter(|i| matches!(i.kind, ItemKind::Comp(_))).count() + 1;
    let name = ps(p, "name").map(str::to_string).unwrap_or_else(|| format!("Pre-comp {n}"));
    let moved: Vec<crate::model::Layer> = ids.iter().filter_map(|id| comp.layer(*id).cloned()).collect();
    let top = ids.iter().filter_map(|id| comp.index_of(*id)).min().unwrap_or(0);
    let nid = super::comp::make_comp(e, &name, comp.width, comp.height, comp.frame_rate, comp.duration, comp.background);
    if let Some(nc) = e.proj().comp_mut(nid) {
        nc.layers = moved.clone();
        for l in nc.layers.iter_mut() {
            if l.parent.is_some_and(|pp| !ids.contains(&pp)) {
                l.parent = None;
            }
            if l.track_matte.is_some_and(|m| !ids.contains(&m.layer)) {
                l.track_matte = None;
            }
        }
    }
    let c = e.proj().comp_mut(cid).ok_or("no comp")?;
    c.layers.retain(|l| !ids.contains(&l.id));
    for l in c.layers.iter_mut() {
        if l.parent.is_some_and(|pp| ids.contains(&pp)) {
            l.parent = None;
        }
    }
    let l = footage_layer(e, cid, nid, 0.0, None, None)?;
    let lid = add_layer(e, cid, l, Some(top));
    select_only(e, &[lid]);
    if pb(p, "open").unwrap_or(false) {
        e.state.active_comp = Some(nid);
        e.state.open_comps.push(nid);
    }
    e.set_label("Pre-compose");
    Ok(json!({"comp": nid, "layer": lid}))
}

pub fn arrange(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select layers".into());
    }
    let above = match p.get("above") {
        Some(v) if !v.is_null() => Some(e.layer_in(e.project.comp(cid).unwrap(), v)?),
        _ => None,
    };
    let to = ps(p, "to").map(str::to_string);
    let index = pu(p, "index").map(|i| (i as usize).saturating_sub(1));
    let c = e.proj().comp_mut(cid).ok_or("no comp")?;
    let mut moving: Vec<crate::model::Layer> = vec![];
    let mut first = usize::MAX;
    for id in &ids {
        if let Some(i) = c.index_of(*id) {
            first = first.min(i);
        }
    }
    let originals: Vec<usize> = ids.iter().filter_map(|id| c.index_of(*id)).collect();
    for id in &ids {
        if let Some(i) = c.index_of(*id) {
            moving.push(c.layers.remove(i));
        }
    }
    let n = c.layers.len();
    let at = if let Some(a) = above {
        c.index_of(a).unwrap_or(0)
    } else if let Some(i) = index {
        i.min(n)
    } else {
        match to.as_deref() {
            Some("front") => 0,
            Some("back") => n,
            Some("forward") => first.saturating_sub(1),
            Some("backward") => (originals.iter().max().copied().unwrap_or(0) + 2 - moving.len()).min(n),
            _ => first.min(n),
        }
    };
    for (k, l) in moving.into_iter().enumerate() {
        c.layers.insert((at + k).min(c.layers.len()), l);
    }
    e.set_label("Arrange Layers");
    Ok(json!({"layers": ids}))
}

pub fn bring_to_front(e: &mut Engine, p: &Json) -> Res {
    let mut q = p.clone();
    q["to"] = json!("front");
    arrange(e, &q)
}

pub fn send_to_back(e: &mut Engine, p: &Json) -> Res {
    let mut q = p.clone();
    q["to"] = json!("back");
    arrange(e, &q)
}

pub fn bring_forward(e: &mut Engine, p: &Json) -> Res {
    let mut q = p.clone();
    q["to"] = json!("forward");
    arrange(e, &q)
}

pub fn send_backward(e: &mut Engine, p: &Json) -> Res {
    let mut q = p.clone();
    q["to"] = json!("backward");
    arrange(e, &q)
}

fn layer_box(e: &Engine, cid: u64, lid: u64) -> Option<[f64; 4]> {
    let comp = e.project.comp(cid)?;
    let l = comp.layer(lid)?;
    let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
    let b = crate::render::content_bounds(&ev, l)?;
    let m = ev.layer_to_comp(l);
    let pts = [m.apply(b[0], b[1]), m.apply(b[2], b[1]), m.apply(b[2], b[3]), m.apply(b[0], b[3])];
    crate::model::path::bounds_of(&pts)
}

fn shift_layer(e: &mut Engine, cid: u64, lid: u64, dx: f64, dy: f64) -> Result<(), String> {
    let now = e.time_of(cid);
    let l = e.layer_mut(cid, lid)?;
    let lt = l.layer_time(now);
    let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut("position")) else { return Ok(()) };
    let mut v = pr.at(lt).vec3();
    v[0] += dx;
    v[1] += dy;
    if pr.keys.is_empty() {
        pr.value = Value::Vec3(v);
    } else {
        crate::model::key::insert(&mut pr.keys, crate::model::Key::new(lt, Value::Vec3(v), true));
    }
    Ok(())
}

pub fn align(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let edge = ps(p, "edge").ok_or("pass `edge`: left|hcenter|right|top|vcenter|bottom")?.to_string();
    let c = e.project.comp(cid).ok_or("no comp")?;
    let boxes: Vec<(u64, [f64; 4])> = ids.iter().filter_map(|id| layer_box(e, cid, *id).map(|b| (*id, b))).collect();
    let target = if ps(p, "to") == Some("selection") && boxes.len() > 1 {
        boxes.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, (_, b)| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])])
    } else {
        [0.0, 0.0, c.width as f64, c.height as f64]
    };
    for (id, b) in boxes {
        let (dx, dy) = match edge.as_str() {
            "left" => (target[0] - b[0], 0.0),
            "right" => (target[2] - b[2], 0.0),
            "hcenter" => ((target[0] + target[2]) / 2.0 - (b[0] + b[2]) / 2.0, 0.0),
            "top" => (0.0, target[1] - b[1]),
            "bottom" => (0.0, target[3] - b[3]),
            "vcenter" => (0.0, (target[1] + target[3]) / 2.0 - (b[1] + b[3]) / 2.0),
            _ => return Err(format!("unknown edge `{edge}`")),
        };
        shift_layer(e, cid, id, dx, dy)?;
    }
    e.set_label("Align");
    Ok(json!({"layers": ids}))
}

pub fn distribute(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let mode = ps(p, "mode").unwrap_or("hcenter").to_string();
    let mut boxes: Vec<(u64, [f64; 4])> = ids.iter().filter_map(|id| layer_box(e, cid, *id).map(|b| (*id, b))).collect();
    if boxes.len() < 3 {
        return Err("select three or more layers to distribute".into());
    }
    let key = |b: &[f64; 4]| match mode.as_str() {
        "left" => b[0],
        "right" => b[2],
        "top" => b[1],
        "bottom" => b[3],
        "vcenter" => (b[1] + b[3]) / 2.0,
        _ => (b[0] + b[2]) / 2.0,
    };
    boxes.sort_by(|a, b| key(&a.1).total_cmp(&key(&b.1)));
    let (lo, hi) = (key(&boxes[0].1), key(&boxes[boxes.len() - 1].1));
    let n = boxes.len() - 1;
    let vertical = matches!(mode.as_str(), "top" | "bottom" | "vcenter");
    for (i, (id, b)) in boxes.iter().enumerate() {
        let want = lo + (hi - lo) * i as f64 / n as f64;
        let d = want - key(b);
        if vertical { shift_layer(e, cid, *id, 0.0, d)? } else { shift_layer(e, cid, *id, d, 0.0)? }
    }
    e.set_label("Distribute");
    Ok(json!({"layers": ids}))
}

pub fn split(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let mut made = vec![];
    for id in ids {
        let l = e.layer_ref(cid, id)?.clone();
        if t <= l.in_point || t >= l.out_point {
            continue;
        }
        let mut dup = l.clone();
        let pj = e.proj();
        dup.id = pj.new_id();
        dup.props.renumber(&mut pj.next_uid);
        dup.in_point = t;
        dup.name = format!("{} 2", l.name);
        let c = pj.comp_mut(cid).ok_or("no comp")?;
        let i = c.index_of(id).unwrap_or(0);
        c.layers[i].out_point = t;
        made.push(dup.id);
        c.layers.insert(i, dup);
    }
    select_only(e, &made);
    e.set_label("Split Layer");
    Ok(json!({"layers": made}))
}

pub fn solid_settings(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let item = e.layer_ref(cid, lid)?.source.ok_or("not a solid layer")?;
    let color = p.get("color").and_then(parse_color);
    let w = pu(p, "width");
    let h = pu(p, "height");
    let name = ps(p, "name").map(str::to_string);
    let pj = e.proj();
    let it = pj.item_mut(item).ok_or("no item")?;
    let ItemKind::Solid(s) = &mut it.kind else { return Err("not a solid layer".into()) };
    if let Some(c) = color {
        s.color = [c[0], c[1], c[2]];
    }
    if let Some(w) = w {
        s.width = (w as u32).max(1);
    }
    if let Some(h) = h {
        s.height = (h as u32).max(1);
    }
    if let Some(n) = &name {
        it.name = n.clone();
    }
    if let Some(n) = name {
        e.layer_mut(cid, lid)?.name = n;
    }
    Ok(json!({"layer": lid}))
}

pub fn auto_orient(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let mode = match ps(p, "mode").unwrap_or("off") {
        "alongPath" => AutoOrient::AlongPath,
        "towardsCamera" => AutoOrient::TowardsCamera,
        "towardsPointOfInterest" => AutoOrient::TowardsPointOfInterest,
        _ => AutoOrient::Off,
    };
    for id in &ids {
        e.layer_mut(cid, *id)?.auto_orient = mode;
    }
    Ok(json!({"mode": format!("{mode:?}")}))
}

pub fn tree(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let depth = pu(p, "depth").unwrap_or(16) as usize;
    let comp = e.project.comp(cid).ok_or("no comp")?;
    let l = comp.layer(lid).ok_or("no layer")?;
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let ev = Eval::new(&e.project, cid, comp, t);
    fn walk(ev: &Eval, l: &crate::model::Layer, g: &crate::model::Group, prefix: &str, depth: usize) -> Vec<Json> {
        let mut out = vec![];
        if depth == 0 {
            return out;
        }
        for (i, c) in g.children.iter().enumerate() {
            let seg = if matches!(g.kind, GroupKind::Indexed) { format!("#{}", i + 1) } else { c.id().to_string() };
            let path = if prefix.is_empty() { seg } else { format!("{prefix}/{seg}") };
            match c {
                Node::Prop(pr) => {
                    let v = ev.value(l, pr);
                    let mut o = json!({"path": path, "name": pr.name, "uid": pr.uid, "type": v.kind_name(), "value": v.to_json(), "animated": pr.is_animated(), "keys": pr.keys.len()});
                    if let Some(x) = &pr.expr {
                        o["expression"] = json!(x.text);
                    }
                    if let crate::model::Ui::Popup { options } = &pr.ui {
                        o["options"] = json!(options);
                    }
                    out.push(o);
                }
                Node::Group(sub) => {
                    let mut o = json!({"path": path, "name": sub.name, "uid": sub.uid, "group": true});
                    if let GroupKind::Effect { effect } = &sub.kind {
                        o["effect"] = json!(effect);
                    }
                    o["children"] = json!(walk(ev, l, sub, &path, depth - 1));
                    out.push(o);
                }
            }
        }
        out
    }
    Ok(json!({"layer": lid, "name": l.name, "type": l.type_name(), "nodes": walk(&ev, l, &l.props, "", depth)}))
}

pub fn duplicate(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select layers to duplicate".into());
    }
    let mut made = vec![];
    for id in ids {
        let l = e.layer_ref(cid, id)?.clone();
        let pj = e.proj();
        let mut dup = l.clone();
        dup.id = pj.new_id();
        dup.props.renumber(&mut pj.next_uid);
        let c = pj.comp_mut(cid).ok_or("no comp")?;
        dup.name = unique_layer_name(c, &l.name);
        let i = c.index_of(id).unwrap_or(0);
        made.push(dup.id);
        c.layers.insert(i, dup);
    }
    select_only(e, &made);
    e.set_label("Duplicate");
    Ok(json!({"layers": made}))
}

pub fn delete_layers(e: &mut Engine, cid: u64, ids: &[u64]) -> Result<usize, String> {
    let c = e.proj().comp_mut(cid).ok_or("no comp")?;
    let before = c.layers.len();
    c.layers.retain(|l| !ids.contains(&l.id));
    for l in c.layers.iter_mut() {
        if l.parent.is_some_and(|p| ids.contains(&p)) {
            l.parent = None;
        }
        if l.track_matte.is_some_and(|m| ids.contains(&m.layer)) {
            l.track_matte = None;
        }
    }
    let n = before - c.layers.len();
    e.state.selected_layers.retain(|l| !ids.contains(l));
    e.state.selected_props.retain(|(l, _)| !ids.contains(l));
    Ok(n)
}

pub fn new_solid_item(e: &mut Engine, p: &Json) -> Res {
    let c = p.get("color").and_then(parse_color).unwrap_or([0.5, 0.5, 0.5, 1.0]);
    let (w, h) = (pu(p, "width").unwrap_or(1920) as u32, pu(p, "height").unwrap_or(1080) as u32);
    let id = solid_item(e, ps(p, "name").unwrap_or("Solid"), [c[0], c[1], c[2]], w, h);
    Ok(json!({"item": id}))
}
