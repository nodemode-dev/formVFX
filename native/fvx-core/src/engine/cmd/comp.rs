use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pb, pf, ps, pu};
use crate::model::value::parse_color;
use crate::model::{Comp, Item, ItemKind, Marker};

pub fn make_comp(e: &mut Engine, name: &str, w: u32, h: u32, fps: f64, dur: f64, bg: [f32; 3]) -> u64 {
    let pj = e.proj();
    let id = pj.new_id();
    let name = pj.unique_item_name(name);
    let comp = Comp { width: w.clamp(4, 30000), height: h.clamp(4, 30000), frame_rate: fps.clamp(1.0, 999.0), duration: dur.max(1.0 / fps.max(1.0)), work_area: [0.0, dur], background: bg, ..Comp::default() };
    pj.items.push(Item { id, name, parent: None, label: 15, comment: String::new(), kind: ItemKind::Comp(comp), proxy: None });
    id
}

pub fn new(e: &mut Engine, p: &Json) -> Res {
    let n = e.project.items.iter().filter(|i| matches!(i.kind, ItemKind::Comp(_))).count() + 1;
    let name = ps(p, "name").map(str::to_string).unwrap_or_else(|| format!("Comp {n}"));
    let w = pu(p, "width").unwrap_or(1920) as u32;
    let h = pu(p, "height").unwrap_or(1080) as u32;
    let fps = pf(p, "frameRate").unwrap_or(30.0);
    let dur = pf(p, "duration").unwrap_or(10.0);
    let bg = p.get("background").and_then(parse_color).map(|c| [c[0] as f32, c[1] as f32, c[2] as f32]).unwrap_or([0.0; 3]);
    let id = make_comp(e, &name, w, h, fps, dur, bg);
    if let Some(c) = e.proj().comp_mut(id) {
        if let Some(pa) = pf(p, "pixelAspect") {
            c.pixel_aspect = pa;
        }
        if let Some(st) = pf(p, "startTime") {
            c.display_start = st;
        }
        if let Some(sa) = pf(p, "shutterAngle") {
            c.shutter_angle = sa.clamp(0.0, 720.0);
        }
        if let Some(sp) = pf(p, "shutterPhase") {
            c.shutter_phase = sp.clamp(-360.0, 360.0);
        }
        if let Some(n) = pu(p, "motionBlurSamples") {
            c.motion_blur_samples = n.clamp(2, 64) as u32;
        }
    }
    if pb(p, "open").unwrap_or(true) {
        e.state.active_comp = Some(id);
        if !e.state.open_comps.contains(&id) {
            e.state.open_comps.push(id);
        }
        e.state.selected_layers.clear();
    }
    Ok(json!({"comp": id, "name": e.project.item(id).map(|i| i.name.clone())}))
}

pub fn open(e: &mut Engine, p: &Json) -> Res {
    let id = e.comp_id(p.get("comp"))?;
    e.state.active_comp = Some(id);
    if !e.state.open_comps.contains(&id) {
        e.state.open_comps.push(id);
    }
    e.state.selected_layers.retain(|l| e.project.comp(id).is_some_and(|c| c.layer(*l).is_some()));
    e.emit(json!({"type": "openComp", "comp": id}));
    Ok(json!({"comp": id}))
}

pub fn close(e: &mut Engine, p: &Json) -> Res {
    let id = e.comp_id(p.get("comp"))?;
    e.state.open_comps.retain(|c| *c != id);
    if e.state.active_comp == Some(id) {
        e.state.active_comp = e.state.open_comps.last().copied();
    }
    Ok(json!({"closed": id}))
}

pub fn set_poster_time(e: &mut Engine, p: &Json) -> Res {
    let id = e.comp_id(p.get("comp"))?;
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(id));
    let c = e.proj().comp_mut(id).ok_or("no composition")?;
    c.poster_time = t.clamp(0.0, c.duration);
    let t = c.poster_time;
    Ok(json!({"comp": id, "posterTime": t}))
}

pub fn reveal_in_project(e: &mut Engine, p: &Json) -> Res {
    let id = e.comp_id(p.get("comp"))?;
    let mut open = vec![];
    let mut at = e.project.item(id).and_then(|i| i.parent);
    while let Some(f) = at {
        open.push(f);
        at = e.project.item(f).and_then(|i| i.parent);
    }
    e.state.project_selection = vec![id];
    e.frontend("window.panel", json!({"panel": "project"}));
    e.emit(json!({"type": "uiAction", "action": "revealItem", "item": id, "folders": open}));
    Ok(json!({"comp": id, "folders": open}))
}

pub fn settings(e: &mut Engine, p: &Json) -> Res {
    let id = e.comp_id(p.get("comp"))?;
    if let Some(n) = ps(p, "name") {
        e.proj().item_mut(id).ok_or("no comp")?.name = n.to_string();
    }
    let c = e.proj().comp_mut(id).ok_or("no comp")?;
    let (ow, oh) = (c.width, c.height);
    if let Some(w) = pu(p, "width") {
        c.width = (w as u32).clamp(4, 30000);
    }
    if let Some(h) = pu(p, "height") {
        c.height = (h as u32).clamp(4, 30000);
    }
    if let Some(fr) = pf(p, "frameRate") {
        c.frame_rate = fr.clamp(1.0, 999.0);
    }
    if let Some(d) = pf(p, "duration") {
        let d = d.max(c.frame_duration());
        if (c.work_area[1] - c.duration).abs() < 1e-6 || c.work_area[1] > d {
            c.work_area[1] = d;
        }
        c.work_area[0] = c.work_area[0].min(c.work_area[1]);
        c.duration = d;
    }
    if let Some(b) = p.get("background").and_then(parse_color) {
        c.background = [b[0] as f32, b[1] as f32, b[2] as f32];
    }
    if let Some(pa) = pf(p, "pixelAspect") {
        c.pixel_aspect = pa;
    }
    if let Some(st) = pf(p, "startTime") {
        c.display_start = st;
    }
    if let Some(sa) = pf(p, "shutterAngle") {
        c.shutter_angle = sa.clamp(0.0, 720.0);
    }
    if let Some(sp) = pf(p, "shutterPhase") {
        c.shutter_phase = sp.clamp(-360.0, 360.0);
    }
    if let Some(n) = pu(p, "motionBlurSamples") {
        c.motion_blur_samples = n.clamp(2, 64) as u32;
    }
    if let Some(b) = pb(p, "preserveFrameRate") {
        c.preserve_frame_rate = b;
    }
    if let Some(b) = pb(p, "preserveResolution") {
        c.preserve_resolution = b;
    }
    let (nw, nh) = (c.width, c.height);
    if (nw, nh) != (ow, oh) {
        let anchor = pu(p, "anchor").unwrap_or(4).min(8);
        let (ax, ay) = ((anchor % 3) as f64 / 2.0, (anchor / 3) as f64 / 2.0);
        let dx = (nw as f64 - ow as f64) * ax;
        let dy = (nh as f64 - oh as f64) * ay;
        for l in c.layers.iter_mut() {
            if l.parent.is_some() {
                continue;
            }
            if let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut("position")) {
                let shift = |v: &mut crate::model::Value| {
                    let mut q = v.vec3();
                    q[0] += dx;
                    q[1] += dy;
                    *v = crate::model::Value::Vec3(q);
                };
                shift(&mut pr.value);
                for k in pr.keys.iter_mut() {
                    shift(&mut k.value);
                }
            }
        }
    }
    e.set_label("Composition Settings");
    Ok(json!({"comp": id}))
}

pub fn list(e: &mut Engine, _: &Json) -> Res {
    let comps: Vec<Json> = e
        .project
        .items
        .iter()
        .filter_map(|i| e.project.comp(i.id).map(|c| json!({"id": i.id, "name": i.name, "width": c.width, "height": c.height, "frameRate": c.frame_rate, "duration": c.duration, "workArea": c.work_area, "layers": c.layers.len(), "active": e.state.active_comp == Some(i.id)})))
        .collect();
    Ok(json!({"comps": comps}))
}

pub fn info(e: &mut Engine, p: &Json) -> Res {
    let (id, c) = e.comp_of(p)?;
    let name = e.project.item(id).map(|i| i.name.clone()).unwrap_or_default();
    let layers: Vec<Json> = c
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let fx: Vec<String> = l.effects().map(|g| g.groups().map(|x| x.name.clone()).collect()).unwrap_or_default();
            json!({
                "id": l.id, "index": i + 1, "name": l.name, "type": l.type_name(),
                "in": l.in_point, "out": l.out_point, "start": l.start_time, "stretch": l.stretch,
                "parent": l.parent, "enabled": l.switches.video, "solo": l.switches.solo, "locked": l.switches.locked,
                "threeD": l.is_3d(), "adjustment": l.switches.adjustment, "blendMode": l.blend.label(),
                "trackMatte": l.track_matte.map(|m| json!({"layer": m.layer, "kind": m.kind.label()})),
                "source": l.source, "effects": fx, "masks": l.masks().map(|m| m.children.len()).unwrap_or(0),
                "animated": l.props.any_animated(),
            })
        })
        .collect();
    Ok(json!({
        "id": id, "name": name, "width": c.width, "height": c.height, "pixelAspect": c.pixel_aspect,
        "frameRate": c.frame_rate, "duration": c.duration, "workArea": c.work_area,
        "background": c.background, "displayStart": c.display_start, "time": e.time_of(id),
        "motionBlur": c.motion_blur, "layers": layers,
        "markers": c.markers.iter().map(|m| json!({"time": m.time, "comment": m.comment, "duration": m.duration})).collect::<Vec<_>>(),
    }))
}

pub fn work_area(e: &mut Engine, p: &Json) -> Res {
    let id = e.comp_id(p.get("comp"))?;
    let now = e.time_of(id);
    let c = e.proj().comp_mut(id).ok_or("no comp")?;
    let fd = c.frame_duration();
    let mut wa = c.work_area;
    match ps(p, "set") {
        Some("begin") => wa[0] = now,
        Some("end") => wa[1] = now + fd,
        _ => {}
    }
    if let Some(s) = pf(p, "start") {
        wa[0] = s;
    }
    if let Some(x) = pf(p, "end") {
        wa[1] = x;
    }
    wa[0] = wa[0].clamp(0.0, c.duration);
    wa[1] = wa[1].clamp(wa[0] + fd, c.duration.max(wa[0] + fd));
    c.work_area = wa;
    Ok(json!({"workArea": wa}))
}

pub fn set_switch(e: &mut Engine, p: &Json) -> Res {
    let id = e.comp_id(p.get("comp"))?;
    let sw = ps(p, "switch").ok_or("pass `switch`: motionBlur|frameBlending|draft3d|hideShy")?.to_string();
    let v = pb(p, "value");
    let c = e.proj().comp_mut(id).ok_or("no comp")?;
    let slot = match sw.as_str() {
        "motionBlur" => &mut c.motion_blur,
        "frameBlending" => &mut c.frame_blending,
        "draft3d" => &mut c.draft_3d,
        "hideShy" | "shy" => &mut c.hide_shy,
        _ => return Err(format!("unknown composition switch `{sw}`")),
    };
    *slot = v.unwrap_or(!*slot);
    Ok(json!({"switch": sw, "value": *slot}))
}

pub fn duplicate(e: &mut Engine, p: &Json) -> Res {
    let id = e.comp_id(p.get("comp"))?;
    let item = e.project.item(id).ok_or("no comp")?.clone();
    let pj = e.proj();
    let nid = pj.new_id();
    let mut it = item;
    it.id = nid;
    it.name = pj.unique_item_name(&it.name);
    let mut map = std::collections::HashMap::new();
    if let ItemKind::Comp(c) = &mut it.kind {
        let paths = super::essential::prop_paths(c);
        for l in c.layers.iter_mut() {
            let lid = pj.new_id();
            map.insert(l.id, lid);
            l.id = lid;
            l.props.renumber(&mut pj.next_uid);
        }
        for l in c.layers.iter_mut() {
            l.parent = l.parent.and_then(|x| map.get(&x).copied());
            if let Some(m) = l.track_matte.as_mut() {
                m.layer = map.get(&m.layer).copied().unwrap_or(m.layer);
            }
        }
        super::essential::remap(c, &map, paths);
    }
    pj.items.push(it);
    Ok(json!({"comp": nid}))
}

pub fn add_marker(e: &mut Engine, p: &Json) -> Res {
    let id = e.comp_id(p.get("comp"))?;
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(id));
    let m = Marker { time: t, duration: pf(p, "duration").unwrap_or(0.0), comment: ps(p, "comment").unwrap_or("").to_string(), ..Marker::default() };
    let c = e.proj().comp_mut(id).ok_or("no comp")?;
    c.markers.push(m);
    c.markers.sort_by(|a, b| a.time.total_cmp(&b.time));
    Ok(json!({"time": t}))
}

pub fn save_frame(e: &mut Engine, p: &Json) -> Res {
    let id = e.comp_id(p.get("comp"))?;
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(id));
    if pb(p, "queue").unwrap_or(false) {
        let fd = e.project.comp(id).map(|c| c.frame_duration()).unwrap_or(1.0 / 30.0);
        let mut q = json!({"comp": id, "slot": "still", "start": t, "end": t + fd});
        if let Some(o) = ps(p, "path").or(ps(p, "output")) {
            q["output"] = json!(o);
        }
        if let Some(s) = pf(p, "scale") {
            q["resolution"] = json!(s.clamp(0.05, 1.0));
        }
        return crate::engine::export::add(e, &q);
    }
    let path = ps(p, "path").ok_or("pass `path` (.png, .jpg, .tif or .exr)")?.to_string();
    let scale = pf(p, "scale").unwrap_or(1.0).clamp(0.05, 4.0);
    if path.to_ascii_lowercase().ends_with(".exr") {
        return super::formats::save_frame_exr(e, &json!({"path": path, "comp": id, "time": t, "scale": scale, "flat": true}));
    }
    let job = crate::engine::state::FrameJob::new(e, id, t, scale, false, false);
    let (w, h, px) = job.run();
    let img = image::RgbaImage::from_raw(w, h, px).ok_or("render failed")?;
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        image::DynamicImage::ImageRgba8(img).to_rgb8().save(&path).map_err(|err| err.to_string())?;
    } else {
        img.save(&path).map_err(|err| err.to_string())?;
    }
    Ok(json!({"path": path, "width": w, "height": h}))
}
