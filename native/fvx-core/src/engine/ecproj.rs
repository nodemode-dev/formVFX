use std::collections::HashMap;

use serde_json::Value as Json;

use crate::effects;
use crate::model::key::{Ease, Interp, Key};
use crate::model::prop::{self, Group, GroupKind, Ids, MaskMode, Node};
use crate::model::value::{GradStop, Gradient, Justify, TextDoc};
use crate::model::{
    AlphaMode, AutoOrient, BezPath, BlendMode, Comp, Footage, FootageKind, Item, ItemKind, Layer, LayerInit, LayerKind, Marker, Matte, MatteKind, Project, Quality, Solid, Switches, Value, build_layer, label_parse,
};

const TPS: f64 = 254_016_000_000.0;

fn secs(j: Option<&Json>) -> f64 {
    j.and_then(Json::as_f64).unwrap_or(0.0) / TPS
}

fn rate(j: Option<&Json>) -> f64 {
    match j {
        Some(Json::Object(o)) => {
            let n = o.get("num").and_then(Json::as_f64).unwrap_or(30.0);
            let d = o.get("den").and_then(Json::as_f64).unwrap_or(1.0).max(1.0);
            n / d
        }
        Some(v) => v.as_f64().unwrap_or(30.0),
        None => 30.0,
    }
}

fn s<'a>(j: &'a Json, k: &str) -> &'a str {
    j.get(k).and_then(Json::as_str).unwrap_or("")
}

fn f(j: &Json, k: &str, d: f64) -> f64 {
    j.get(k).and_then(Json::as_f64).unwrap_or(d)
}

fn b(j: &Json, k: &str, d: bool) -> bool {
    j.get(k).and_then(Json::as_bool).unwrap_or(d)
}

fn arr(j: Option<&Json>) -> Vec<f64> {
    j.and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).collect()).unwrap_or_default()
}

fn pts(j: Option<&Json>) -> Vec<[f64; 2]> {
    j.and_then(Json::as_array).map(|a| a.iter().filter_map(|p| Some([p.get(0)?.as_f64()?, p.get(1)?.as_f64()?])).collect()).unwrap_or_default()
}

fn text_doc(v: &Json) -> TextDoc {
    let mut d = TextDoc::default();
    d.text = s(v, "text").to_string();
    if !s(v, "font").is_empty() {
        d.font = s(v, "font").to_string();
    }
    if !s(v, "style").is_empty() {
        d.style = s(v, "style").to_string();
    }
    d.size = f(v, "size", 72.0);
    let c3 = |k: &str, dflt: [f64; 3]| {
        let a = arr(v.get(k));
        if a.len() >= 3 { [a[0], a[1], a[2]] } else { dflt }
    };
    d.fill = c3("fill", [1.0; 3]);
    d.stroke = c3("stroke", [0.0; 3]);
    d.stroke_width = f(v, "stroke_width", 0.0);
    d.apply_fill = b(v, "apply_fill", true);
    d.apply_stroke = b(v, "apply_stroke", false);
    d.stroke_over_fill = b(v, "stroke_over_fill", true);
    d.tracking = f(v, "tracking", 0.0);
    d.leading = v.get("leading").and_then(Json::as_f64);
    d.justify = Justify::parse(s(v, "justify")).unwrap_or(Justify::Left);
    d.faux_bold = b(v, "faux_bold", false);
    d.faux_italic = b(v, "faux_italic", false);
    d.all_caps = b(v, "all_caps", false);
    d.small_caps = b(v, "small_caps", false);
    d.baseline_shift = f(v, "baseline_shift", 0.0);
    d.h_scale = f(v, "h_scale", 100.0);
    d.v_scale = f(v, "v_scale", 100.0);
    d.indent_left = f(v, "indent_left", 0.0);
    d.indent_right = f(v, "indent_right", 0.0);
    let bs = arr(v.get("box_size"));
    if bs.len() == 2 {
        let bp = arr(v.get("box_pos"));
        let (x, y) = if bp.len() == 2 { (bp[0], bp[1]) } else { (0.0, 0.0) };
        d.box_size = Some([x, y, bs[0], bs[1]]);
    }
    d
}

fn value(v: &Json, like: Option<&Value>) -> Option<Value> {
    let t = s(v, "t");
    let x = v.get("v")?;
    Some(match t {
        "Scalar" => Value::Num(x.as_f64()?),
        "Vec2" => {
            let a = arr(Some(x));
            Value::Vec2([*a.first()?, *a.get(1)?])
        }
        "Vec3" => {
            let a = arr(Some(x));
            Value::Vec3([*a.first()?, *a.get(1)?, a.get(2).copied().unwrap_or(0.0)])
        }
        "Color" => {
            let a = arr(Some(x));
            Value::Color([*a.first()?, *a.get(1)?, *a.get(2)?, a.get(3).copied().unwrap_or(1.0)])
        }
        "Bool" => Value::Bool(x.as_bool()?),
        "Enum" => Value::Enum(x.as_u64()? as u32),
        "Path" => {
            let vertices = pts(x.get("vertices"));
            let n = vertices.len();
            let mut ins = pts(x.get("in_tangents"));
            let mut outs = pts(x.get("out_tangents"));
            ins.resize(n, [0.0; 2]);
            outs.resize(n, [0.0; 2]);
            Value::Path(BezPath { vertices, ins, outs, closed: b(x, "closed", true) })
        }
        "Text" => Value::Text(Box::new(text_doc(x))),
        "Gradient" => {
            let stops = x.get("stops").or(Some(x)).and_then(Json::as_array)?;
            let mut out = vec![];
            for st in stops {
                let pos = st.get("pos").or_else(|| st.get("position")).and_then(Json::as_f64).or_else(|| st.get(0).and_then(Json::as_f64)).unwrap_or(0.0);
                let c = arr(st.get("color"));
                let c = if c.len() >= 3 { c } else { arr(Some(st)).into_iter().skip(1).collect() };
                if c.len() >= 3 {
                    out.push(GradStop { pos, color: [c[0], c[1], c[2], c.get(3).copied().unwrap_or(1.0)] });
                }
            }
            if out.len() < 2 {
                return None;
            }
            Value::Gradient(Gradient { stops: out })
        }
        _ => return None,
    })
    .map(|v| match (like, v) {
        (Some(Value::Vec3(_)), Value::Vec2(a)) => Value::Vec3([a[0], a[1], 0.0]),
        (Some(Value::Vec2(_)), Value::Vec3(a)) => Value::Vec2([a[0], a[1]]),
        (Some(Value::Color(_)), Value::Vec3(a)) => Value::Color([a[0], a[1], a[2], 1.0]),
        (Some(Value::Enum(_)), Value::Num(n)) => Value::Enum(n.max(0.0) as u32),
        (Some(Value::Num(_)), Value::Enum(n)) => Value::Num(n as f64),
        (Some(Value::Bool(_)), Value::Num(n)) => Value::Bool(n != 0.0),
        (_, v) => v,
    })
}

fn interp(j: Option<&Json>) -> Interp {
    match j.and_then(Json::as_str).unwrap_or("Linear") {
        "Bezier" => Interp::Bezier,
        "Hold" => Interp::Hold,
        _ => Interp::Linear,
    }
}

fn eases(j: Option<&Json>) -> Vec<Ease> {
    j.and_then(Json::as_array)
        .map(|a| a.iter().map(|e| Ease { speed: f(e, "speed", 0.0), influence: (f(e, "influence", 1.0 / 6.0) * 100.0).clamp(0.1, 100.0) }).collect())
        .unwrap_or_default()
}

fn copy_prop(src: &Json, dst: &mut crate::model::Prop) {
    if let Some(v) = src.get("value").and_then(|v| value(v, Some(&dst.value))) {
        if std::mem::discriminant(&v) == std::mem::discriminant(&dst.value) {
            dst.value = v;
        }
    }
    if let Some(keys) = src.get("keys").and_then(Json::as_array) {
        let mut out = vec![];
        for k in keys {
            let Some(v) = k.get("value").and_then(|v| value(v, Some(&dst.value))) else { continue };
            if std::mem::discriminant(&v) != std::mem::discriminant(&dst.value) {
                continue;
            }
            let mut key = Key::new(secs(k.get("time")), v, dst.spatial);
            key.in_interp = interp(k.get("in_interp"));
            key.out_interp = interp(k.get("out_interp"));
            key.in_ease = eases(k.get("in_ease"));
            key.out_ease = eases(k.get("out_ease"));
            key.auto_bezier = b(k, "auto_bezier", false);
            key.continuous = b(k, "continuous", false);
            let si = arr(k.get("spatial_in"));
            let so = arr(k.get("spatial_out"));
            if si.len() >= 2 {
                key.spatial_in = [si[0], si[1], si.get(2).copied().unwrap_or(0.0)];
            }
            if so.len() >= 2 {
                key.spatial_out = [so[0], so[1], so.get(2).copied().unwrap_or(0.0)];
            }
            key.spatial_auto = b(k, "spatial_auto", dst.spatial);
            key.roving = b(k, "roving", false);
            out.push(key);
        }
        out.sort_by(|a, b| a.time.total_cmp(&b.time));
        dst.keys = out;
    }
    if let Some(e) = src.get("expr").filter(|e| !e.is_null()) {
        let text = s(e, "text").to_string();
        if !text.is_empty() {
            dst.expr = Some(prop::Expr { text, enabled: b(e, "enabled", true) });
        }
    }
}

fn children(j: &Json) -> Vec<&Json> {
    j.get("children").and_then(Json::as_array).map(|a| a.iter().collect()).unwrap_or_default()
}

fn child<'a>(j: &'a Json, m: &str) -> Option<&'a Json> {
    children(j).into_iter().find(|c| s(c, "match") == m)
}

fn copy_group(src: &Json, dst: &mut Group, ids: &mut Ids, warn: &mut Vec<String>) {
    for c in children(src) {
        let m = s(c, "match");
        let is_prop = s(c, "node") == "Prop" || c.get("value").is_some();
        if is_prop {
            if let Some(p) = dst.get_mut(m) {
                copy_prop(c, p);
            }
            continue;
        }
        match m {
            "masks" | "effects" | "contents" => {}
            _ => {
                if let Some(g) = dst.sub_mut(m) {
                    copy_group(c, g, ids, warn);
                }
            }
        }
    }
}

fn animator_prop(ids: &mut Ids, m: &str, three: bool) -> Option<crate::model::Prop> {
    let kind = match m {
        "skewAxis" => "skew",
        "trackingType" => "tracking",
        "characterAlignment" => "characterOffset",
        "characterRange" => "characterValue",
        other => other,
    };
    if let Some(tag) = m.strip_prefix("axis_") {
        return Some(prop::font_axis_prop(ids, tag, tag, 0.0, -1000.0, 1000.0));
    }
    prop::text_anim_props(ids, kind, three).into_iter().find(|p| p.id == m)
}

fn import_text_animation(t: &Json, l: &mut Layer, ids: &mut Ids) {
    let Some(dst) = l.props.sub_mut("text") else { return };
    if let Some(p) = child(t, "perChar3d")
        && let Some(d) = dst.get_mut("perChar3d")
    {
        copy_prop(p, d);
    }
    for g in ["pathOptions", "moreOptions"] {
        if let (Some(sg), Some(dg)) = (child(t, g), dst.sub_mut(g)) {
            let mut w = vec![];
            copy_group(sg, dg, ids, &mut w);
        }
    }
    let three = dst.get("perChar3d").is_some_and(|p| p.value.boolean());
    let Some(src) = child(t, "animators") else { return };
    let mut made = vec![];
    for a in children(src) {
        let mut props = vec![];
        if let Some(pg) = child(a, "properties") {
            for pc in children(pg) {
                if let Some(mut pr) = animator_prop(ids, s(pc, "match"), three) {
                    copy_prop(pc, &mut pr);
                    props.push(pr);
                }
            }
        }
        let name = s(a, "name");
        let mut an = prop::text_animator(ids, if name.is_empty() { "Animator" } else { name }, props, None);
        an.enabled = b(a, "enabled", true);
        if let Some(sg) = child(a, "selectors") {
            let mut sels = vec![];
            for sc in children(sg) {
                let kind = match s(sc, "match") {
                    "wigglySelector" => "wiggly",
                    "expressionSelector" => "expression",
                    _ => "range",
                };
                let nm = s(sc, "name");
                let Some(mut g) = prop::text_selector(ids, kind, if nm.is_empty() { "Range Selector 1" } else { nm }) else { continue };
                let mut w = vec![];
                copy_group(sc, &mut g, ids, &mut w);
                g.enabled = b(sc, "enabled", true);
                sels.push(Node::Group(g));
            }
            if let Some(dsg) = an.sub_mut("selectors") {
                dsg.children = sels;
            }
        }
        made.push(Node::Group(an));
    }
    if let Some(dst_anims) = l.props.sub_mut("text").and_then(|t| t.sub_mut("animators")) {
        dst_anims.children = made;
    }
}

fn shape_kind(m: &str) -> Option<&'static str> {
    Some(match m {
        "group" | "vectorGroup" => "group",
        "rect" | "rectangle" => "rect",
        "ellipse" => "ellipse",
        "star" | "polystar" => "star",
        "path" | "shape" => "path",
        "fill" => "fill",
        "stroke" => "stroke",
        "gradientFill" | "gfill" => "gradientFill",
        "gradientStroke" | "gstroke" => "gradientStroke",
        "trim" | "trimPaths" => "trim",
        "repeater" => "repeater",
        "roundCorners" | "round" => "roundCorners",
        "offsetPaths" | "offset" => "offsetPaths",
        "twist" => "twist",
        "wiggle" | "wigglePaths" => "wiggle",
        "mergePaths" | "merge" => "mergePaths",
        "puckerBloat" | "pucker" => "puckerBloat",
        "zigZag" | "zigzag" => "zigZag",
        "wiggleTransform" => "wiggleTransform",
        _ => return None,
    })
}

fn shape_renames(kind: &str, src: &Json, g: &mut Group) {
    let mv = |g: &mut Group, from: &Json, to: &str| {
        if let Some(p) = g.get_mut(to) {
            copy_prop(from, p);
        }
    };
    match kind {
        "wiggle" => {
            for (a, b) in [("speed", "wigglesPerSecond"), ("phase", "temporalPhase")] {
                if let Some(c) = child(src, a) {
                    mv(g, c, b);
                }
            }
        }
        "stroke" | "gradientStroke" => {
            if let Some(d) = child(src, "dashes") {
                for (a, b) in [("dash", "dash"), ("gap", "gap"), ("offset", "dashOffset"), ("dash2", "dash2"), ("gap2", "gap2"), ("dash3", "dash3"), ("gap3", "gap3")] {
                    if let Some(c) = child(d, a) {
                        mv(g, c, b);
                    }
                }
            }
        }
        _ => {}
    }
}

fn shape_contents(src: &Json, ids: &mut Ids, warn: &mut Vec<String>) -> Vec<Node> {
    let mut out = vec![];
    for c in children(src) {
        let m = s(c, "match");
        let Some(kind) = shape_kind(m) else {
            warn.push(format!("shape item `{m}` was skipped"));
            continue;
        };
        let name = c.get("name").and_then(Json::as_str);
        let Some(mut g) = prop::shape_item(ids, kind, name) else { continue };
        g.enabled = b(c, "enabled", true);
        if kind == "group" {
            if let Some(inner) = child(c, "contents") {
                let nodes = shape_contents(inner, ids, warn);
                if let Some(cg) = g.sub_mut("contents") {
                    cg.children = nodes;
                }
            }
            if let (Some(st), Some(dt)) = (child(c, "transform"), g.sub_mut("transform")) {
                copy_group(st, dt, ids, warn);
            }
        } else {
            copy_group(c, &mut g, ids, warn);
            shape_renames(kind, c, &mut g);
            if kind == "repeater"
                && let (Some(st), Some(dt)) = (child(c, "transform"), g.sub_mut("transform"))
            {
                copy_group(st, dt, ids, warn);
            }
        }
        out.push(Node::Group(g));
    }
    out
}

fn mask_mode(m: &str) -> MaskMode {
    MaskMode::parse(m).unwrap_or(MaskMode::Add)
}

fn effect_of(c: &Json) -> Option<&'static effects::EffectSpec> {
    let id = c.get("kind").map(|k| s(k, "effect")).unwrap_or("");
    let name = s(c, "name");
    let base = name.trim_end_matches(|ch: char| ch.is_ascii_digit()).trim_end();
    effects::find(base).or_else(|| effects::find(id.rsplit('.').next().unwrap_or(id))).or_else(|| effects::find(name))
}

fn convert_layer(lj: &Json, comp: &Comp, comp_size: (u32, u32), items: &HashMap<u64, u64>, pj: &mut Project, warn: &mut Vec<String>) -> Option<Layer> {
    let src = lj.get("source")?;
    let st = s(src, "type");
    let item = src.get("item").and_then(Json::as_u64).and_then(|i| items.get(&i).copied());
    let kind = match st {
        "Null" => LayerKind::Null,
        "Text" => LayerKind::Text,
        "Shape" => LayerKind::Shape,
        "Camera" => LayerKind::Camera,
        "Light" => LayerKind::Light,
        "Solid" => LayerKind::Solid,
        _ => match item.and_then(|i| pj.item(i)).map(|i| &i.kind) {
            Some(ItemKind::Comp(_)) => LayerKind::Precomp,
            Some(ItemKind::Footage(f)) if f.kind == crate::model::FootageKind::Model => LayerKind::Model,
            Some(ItemKind::Footage(_)) => LayerKind::Footage,
            Some(ItemKind::Solid(_)) => LayerKind::Solid,
            _ => {
                warn.push(format!("layer \"{}\" ({st}) was imported as a null", s(lj, "name")));
                LayerKind::Null
            }
        },
    };
    let size = match item.and_then(|i| pj.item(i)).and_then(|i| i.dimensions()) {
        Some((w, h)) => (w as f64, h as f64),
        None => (comp.width as f64, comp.height as f64),
    };
    let mut l = build_layer(pj, comp_size, LayerInit { kind, name: s(lj, "name").to_string(), source: if matches!(kind, LayerKind::Null | LayerKind::Text | LayerKind::Shape | LayerKind::Camera | LayerKind::Light) { None } else { item }, size, start: 0.0, duration: comp.duration });
    l.start_time = secs(lj.get("start_time"));
    l.in_point = secs(lj.get("in_point"));
    l.out_point = secs(lj.get("out_point"));
    l.stretch = f(lj, "stretch", 100.0);
    l.label = label_parse(s(lj, "label")).unwrap_or(l.label);
    l.comment = s(lj, "comment").to_string();
    if let Some(sw) = lj.get("switches") {
        let mut w = Switches::default();
        w.video = b(sw, "video", true);
        w.audio = b(sw, "audio", true);
        w.solo = b(sw, "solo", false);
        w.locked = b(sw, "locked", false);
        w.shy = b(sw, "shy", false);
        w.collapse = b(sw, "collapse", false);
        w.quality = if s(sw, "quality") == "Draft" { Quality::Draft } else { Quality::Best };
        w.effects = b(sw, "effects", true);
        w.motion_blur = b(sw, "motion_blur", false);
        w.adjustment = b(sw, "adjustment", false);
        w.three_d = b(sw, "three_d", false);
        w.guide = b(sw, "guide", false);
        l.switches = w;
    }
    l.blend = BlendMode::parse(s(lj, "blend_mode")).unwrap_or(BlendMode::Normal);
    l.preserve_transparency = b(lj, "preserve_transparency", false);
    l.auto_orient = match s(lj, "auto_orient") {
        "AlongPath" => AutoOrient::AlongPath,
        "TowardsCamera" => AutoOrient::TowardsCamera,
        "TowardsPointOfInterest" => AutoOrient::TowardsPointOfInterest,
        _ => l.auto_orient,
    };
    l.markers = lj.get("markers").and_then(Json::as_array).map(|a| a.iter().map(|m| Marker { time: secs(m.get("time")), duration: secs(m.get("duration")), comment: s(m, "comment").to_string(), ..Marker::default() }).collect()).unwrap_or_default();
    let props = lj.get("props")?;
    let mut ids = Ids(&mut pj.next_uid);
    if let Some(tr) = child(props, "transform")
        && let Some(dt) = l.props.sub_mut("transform")
    {
        copy_group(tr, dt, &mut ids, warn);
    }
    for g in ["cameraOptions", "lightOptions", "materialOptions", "audio"] {
        if let Some(sg) = child(props, g)
            && let Some(dg) = l.props.sub_mut(g)
        {
            copy_group(sg, dg, &mut ids, warn);
        }
    }
    if let Some(t) = child(props, "text")
        && let Some(st) = child(t, "sourceText")
        && let Some(dt) = l.props.sub_mut("text").and_then(|g| g.get_mut("sourceText"))
    {
        copy_prop(st, dt);
        import_text_animation(t, &mut l, &mut ids);
    }
    if let Some(m) = child(props, "masks") {
        let mut nodes = vec![];
        for (i, mg) in children(m).into_iter().enumerate() {
            let k = mg.get("kind").cloned().unwrap_or(Json::Null);
            let color = arr(k.get("color"));
            let color = if color.len() >= 3 { [color[0] as u8, color[1] as u8, color[2] as u8] } else { [200, 200, 80] };
            let name = mg.get("name").and_then(Json::as_str).map(str::to_string).unwrap_or_else(|| format!("Mask {}", i + 1));
            let mut g = prop::mask_group(&mut ids, &name, BezPath::default(), mask_mode(s(&k, "mode")), color);
            if let GroupKind::Mask { inverted, locked, .. } = &mut g.kind {
                *inverted = b(&k, "inverted", false);
                *locked = b(&k, "locked", false);
            }
            g.enabled = b(mg, "enabled", true);
            copy_group(mg, &mut g, &mut ids, warn);
            nodes.push(Node::Group(g));
        }
        if let Some(dm) = l.props.sub_mut("masks") {
            dm.children = nodes;
        }
    }
    if let Some(fx) = child(props, "effects") {
        let (sw, sh) = size;
        let mut nodes = vec![];
        for eg in children(fx) {
            match effect_of(eg) {
                Some(spec) => {
                    let mut g = effects::instance(spec, &mut ids, s(eg, "name"), [sw, sh]);
                    g.enabled = b(eg, "enabled", true);
                    copy_group(eg, &mut g, &mut ids, warn);
                    nodes.push(Node::Group(g));
                }
                None => warn.push(format!("effect \"{}\" on \"{}\" has no formvfx equivalent yet and was skipped", s(eg, "name"), l.name)),
            }
        }
        if let Some(df) = l.props.sub_mut("effects") {
            df.children = nodes;
        }
    }
    if let Some(c) = child(props, "contents") {
        let nodes = shape_contents(c, &mut ids, warn);
        if let Some(dc) = l.props.sub_mut("contents") {
            dc.children = nodes;
        }
    }
    Some(l)
}

pub fn import_json(j: &Json, path: &str) -> Result<Project, String> {
    let items = match j.get("items") {
        Some(Json::Object(m)) => m.values().cloned().collect::<Vec<_>>(),
        Some(Json::Array(a)) => a.clone(),
        _ => return Err(format!("{path} has no items")),
    };
    let mut pj = Project::default();
    let mut warn = vec![];
    let mut map: HashMap<u64, u64> = HashMap::new();
    let mut sorted = items.clone();
    sorted.sort_by_key(|i| i.get("id").and_then(Json::as_u64).unwrap_or(0));
    for it in &sorted {
        let old = it.get("id").and_then(Json::as_u64).unwrap_or(0);
        let id = pj.new_id();
        map.insert(old, id);
    }
    let mut comps: Vec<(u64, Json)> = vec![];
    for it in &sorted {
        let old = it.get("id").and_then(Json::as_u64).unwrap_or(0);
        let id = map[&old];
        let k = it.get("kind").cloned().unwrap_or(Json::Null);
        let kind = match s(&k, "type") {
            "Folder" => ItemKind::Folder,
            "Solid" => {
                let c = arr(k.get("color"));
                ItemKind::Solid(Solid { color: if c.len() >= 3 { [c[0], c[1], c[2]] } else { [0.5; 3] }, width: f(&k, "width", 1920.0) as u32, height: f(&k, "height", 1080.0) as u32, pixel_aspect: f(&k, "pixel_aspect", 1.0) })
            }
            "Footage" => {
                let fk = match s(&k, "kind") {
                    "Video" => FootageKind::Video,
                    "Audio" => FootageKind::Audio,
                    "Sequence" => FootageKind::Sequence,
                    "Vector" => FootageKind::Vector,
                    _ if s(&k, "path").to_ascii_lowercase().ends_with(".svg") => FootageKind::Vector,
                    _ => FootageKind::Image,
                };
                ItemKind::Footage(Footage {
                    path: s(&k, "path").to_string(),
                    kind: if fk == FootageKind::Sequence { FootageKind::Video } else { fk },
                    width: f(&k, "width", 0.0) as u32,
                    height: f(&k, "height", 0.0) as u32,
                    duration: secs(k.get("duration")),
                    frame_rate: rate(k.get("frame_rate")),
                    has_video: b(&k, "has_video", true),
                    has_audio: b(&k, "has_audio", false),
                    codec: s(&k, "codec").to_string(),
                    pixel_aspect: f(&k, "pixel_aspect", 1.0),
                    alpha: match s(&k, "alpha") {
                        "Ignore" => AlphaMode::Ignore,
                        "Premultiplied" => AlphaMode::Premultiplied,
                        _ => AlphaMode::Straight,
                    },
                    missing: b(&k, "missing", false),
                    loops: f(&k, "loop_count", 1.0).max(1.0) as u32,
                    ..Footage::default()
                })
            }
            "Comp" => {
                let c = Comp {
                    width: f(&k, "width", 1920.0) as u32,
                    height: f(&k, "height", 1080.0) as u32,
                    pixel_aspect: f(&k, "pixel_aspect", 1.0),
                    frame_rate: rate(k.get("frame_rate")),
                    duration: secs(k.get("duration")).max(1.0 / 30.0),
                    display_start: secs(k.get("display_start")),
                    background: {
                        let a = arr(k.get("background"));
                        if a.len() >= 3 { [a[0] as f32, a[1] as f32, a[2] as f32] } else { [0.0; 3] }
                    },
                    work_area: {
                        let a = arr(k.get("work_area"));
                        if a.len() == 2 { [a[0] / TPS, a[1] / TPS] } else { [0.0, secs(k.get("duration"))] }
                    },
                    motion_blur: b(&k, "enable_motion_blur", false),
                    shutter_angle: f(&k, "shutter_angle", 180.0),
                    shutter_phase: f(&k, "shutter_phase", -90.0),
                    renderer: if s(&k, "renderer") == "Advanced3D" { "advanced3d".into() } else { "classic3d".into() },
                    ..Comp::default()
                };
                comps.push((id, k.clone()));
                ItemKind::Comp(c)
            }
            other => {
                warn.push(format!("project item \"{}\" ({other}) was skipped", s(it, "name")));
                ItemKind::Folder
            }
        };
        pj.items.push(Item { id, name: s(it, "name").to_string(), parent: it.get("parent").and_then(Json::as_u64).and_then(|p| map.get(&p).copied()), label: label_parse(s(it, "label")).unwrap_or(0), comment: s(it, "comment").to_string(), kind, proxy: None });
    }
    for (cid, k) in comps {
        let comp = pj.comp(cid).cloned().unwrap_or_default();
        let size = (comp.width, comp.height);
        let mut layers = vec![];
        let mut lmap: HashMap<u64, u64> = HashMap::new();
        let src_layers = k.get("layers").and_then(Json::as_array).cloned().unwrap_or_default();
        for lj in &src_layers {
            if let Some(l) = convert_layer(lj, &comp, size, &map, &mut pj, &mut warn) {
                lmap.insert(lj.get("id").and_then(Json::as_u64).unwrap_or(0), l.id);
                layers.push((lj.clone(), l));
            }
        }
        let mut out = vec![];
        for (lj, mut l) in layers {
            l.parent = lj.get("parent").and_then(Json::as_u64).and_then(|p| lmap.get(&p).copied());
            if let Some(tm) = lj.get("track_matte").filter(|t| !t.is_null()) {
                let layer = tm.get("layer").and_then(Json::as_u64).and_then(|x| lmap.get(&x).copied());
                let kind = MatteKind::parse(s(tm, "kind")).unwrap_or(MatteKind::Alpha);
                l.track_matte = layer.map(|layer| Matte { layer, kind });
            }
            out.push(l);
        }
        if let Some(c) = pj.comp_mut(cid) {
            c.layers = out;
            c.markers = k.get("markers").and_then(Json::as_array).map(|a| a.iter().map(|m| Marker { time: secs(m.get("time")), duration: secs(m.get("duration")), comment: s(m, "comment").to_string(), ..Marker::default() }).collect()).unwrap_or_default();
        }
    }
    pj.saved_by = format!("imported from {path}");
    if !warn.is_empty() {
        let id = pj.new_id();
        pj.items.push(Item { id, name: "Import notes".into(), parent: None, label: 0, comment: warn.join("\n"), kind: ItemKind::Folder, proxy: None });
    }
    Ok(pj)
}

pub fn import(path: &str) -> Result<Project, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let j: Json = if super::xmldoc::looks_like_xml(&text) {
        super::xmldoc::to_json(&text).map_err(|e| format!("{path} is not a readable .ecprojx: {e}"))?
    } else {
        serde_json::from_str(&text).map_err(|e| format!("{path} is not a readable .ecproj: {e}"))?
    };
    import_json(&j, path)
}
