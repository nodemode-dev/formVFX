use std::path::PathBuf;

use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pb, pf, ps, pu};
use crate::model::prop::{GroupKind, Node};
use crate::model::{Comp, Stereo, Value, Vr};
use crate::render::comp::{RenderOpts, Renderer};
use crate::render::image::Img;
use crate::render::vr::{FACE_NAMES, STEREO_FX};

fn plain_3d(e: &Engine, cid: u64) -> Result<&Comp, String> {
    let c = e.project.comp(cid).ok_or("no composition")?;
    if c.vr.is_some() || c.stereo.is_some() {
        return Err("pick the 3D composition itself, not a VR or stereo output".into());
    }
    Ok(c)
}

fn new_comp_like(e: &mut Engine, src: u64, name: &str, w: u32, h: u32) -> Result<u64, String> {
    let c = e.project.comp(src).ok_or("no composition")?;
    let (fps, dur, bg) = (c.frame_rate, c.duration, c.background);
    Ok(super::comp::make_comp(e, name, w, h, fps, dur, bg))
}

fn item_name(e: &Engine, id: u64) -> String {
    e.project.item(id).map(|i| i.name.clone()).unwrap_or_default()
}

pub fn create_environment(e: &mut Engine, p: &Json) -> Res {
    let src = e.comp_id(p.get("comp"))?;
    let has_3d = plain_3d(e, src)?.has_3d();
    let w = (pu(p, "width").unwrap_or(2048).clamp(64, 16384) as u32) & !1;
    let face = pu(p, "faceSize").map(|x| x as u32).unwrap_or(w / 4).clamp(16, 8192);
    let name = ps(p, "name").map(str::to_string).unwrap_or_else(|| format!("{} VR 360", item_name(e, src)));
    let cid = new_comp_like(e, src, &name, w, w / 2)?;
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    c.vr = Some(Vr { source: src, face, yaw: pf(p, "yaw").unwrap_or(0.0), pitch: pf(p, "pitch").unwrap_or(0.0), roll: pf(p, "roll").unwrap_or(0.0) });
    if pb(p, "open").unwrap_or(true) {
        super::comp::open(e, &json!({"comp": cid}))?;
    }
    e.set_label("Create VR Environment");
    Ok(json!({"comp": cid, "source": src, "width": w, "height": w / 2, "faceSize": face, "sourceIs3d": has_3d}))
}

pub fn environments(e: &mut Engine, _: &Json) -> Res {
    let rows: Vec<Json> = e
        .project
        .items
        .iter()
        .filter_map(|it| match &it.kind {
            crate::model::ItemKind::Comp(c) => c.vr.as_ref().map(|v| (it, c, v)),
            _ => None,
        })
        .map(|(it, c, v)| json!({"comp": it.id, "name": it.name, "source": v.source, "sourceName": item_name(e, v.source), "width": c.width, "height": c.height, "faceSize": v.face, "yaw": v.yaw, "pitch": v.pitch, "roll": v.roll}))
        .collect();
    Ok(json!({"environments": rows}))
}

fn vr_comp(e: &Engine, p: &Json) -> Result<u64, String> {
    let cid = e.comp_id(p.get("comp"))?;
    if e.project.comp(cid).is_some_and(|c| c.vr.is_some()) {
        return Ok(cid);
    }
    e.project
        .items
        .iter()
        .find(|it| matches!(&it.kind, crate::model::ItemKind::Comp(c) if c.vr.as_ref().is_some_and(|v| v.source == cid)))
        .map(|it| it.id)
        .ok_or_else(|| "pass a VR environment composition (comp.vr.environments lists them)".into())
}

pub fn set_view(e: &mut Engine, p: &Json) -> Res {
    let cid = vr_comp(e, p)?;
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    let v = c.vr.as_mut().ok_or("not a VR environment")?;
    if let Some(x) = pf(p, "yaw") {
        v.yaw = x;
    }
    if let Some(x) = pf(p, "pitch") {
        v.pitch = x.clamp(-90.0, 90.0);
    }
    if let Some(x) = pf(p, "roll") {
        v.roll = x;
    }
    if let Some(x) = pu(p, "faceSize") {
        v.face = (x as u32).clamp(16, 8192);
    }
    let out = json!({"comp": cid, "yaw": v.yaw, "pitch": v.pitch, "roll": v.roll, "faceSize": v.face});
    e.set_label("VR View Orientation");
    Ok(out)
}

fn save_png(img: &Img, path: &PathBuf) -> Result<(), String> {
    let buf = image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.to_rgba8()).ok_or("bad image")?;
    buf.save(path).map_err(|x| format!("{}: {x}", path.display()))
}

pub fn extract_cubemap(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let c = e.project.comp(cid).ok_or("no composition")?;
    let vr = match &c.vr {
        Some(v) => Vr { face: pu(p, "faceSize").map(|x| x as u32).unwrap_or(v.face), ..v.clone() },
        None if c.stereo.is_none() => Vr { source: cid, face: pu(p, "faceSize").unwrap_or(1024) as u32, ..Vr::default() },
        None => return Err("pick a 3D composition or a VR environment".into()),
    };
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let folder = ps(p, "folder").map(PathBuf::from).unwrap_or_else(|| crate::render::diskcache::data_dir().join("VR"));
    std::fs::create_dir_all(&folder).map_err(|x| format!("{}: {x}", folder.display()))?;
    let base: String = item_name(e, cid).chars().map(|ch| if ch.is_alphanumeric() || "-_ ".contains(ch) { ch } else { '_' }).collect();
    let r = Renderer::new(&e.project, &e.media, RenderOpts { guides: false, ..Default::default() });
    let (faces, _) = crate::render::vr::faces(&r, &vr, t, 1.0).ok_or("nothing to render")?;
    let layout = ps(p, "layout").unwrap_or("faces");
    let mut paths = vec![];
    if layout == "strip" {
        let n = faces[0].w;
        let mut strip = Img::new(n * 3, n * 2);
        for (i, f) in faces.iter().enumerate() {
            let (ox, oy) = ((i % 3) * n, (i / 3) * n);
            for y in 0..n.min(f.h) {
                let row = &f.px[y * f.w * 4..(y * f.w + n.min(f.w)) * 4];
                let at = ((oy + y) * n * 3 + ox) * 4;
                strip.px[at..at + row.len()].copy_from_slice(row);
            }
        }
        let path = folder.join(format!("{base}_cubemap.png"));
        save_png(&strip, &path)?;
        paths.push(path.to_string_lossy().to_string());
    } else {
        for (f, n) in faces.iter().zip(FACE_NAMES) {
            let path = folder.join(format!("{base}_{n}.png"));
            save_png(f, &path)?;
            paths.push(path.to_string_lossy().to_string());
        }
    }
    let mut imported = Json::Null;
    if pb(p, "import").unwrap_or(false) {
        imported = super::file::import(e, &json!({"paths": paths, "sequence": false}))?;
    }
    Ok(json!({"paths": paths, "faceSize": vr.face, "layout": layout, "imported": imported}))
}

pub fn stereo_rig(e: &mut Engine, p: &Json) -> Res {
    let src = e.comp_id(p.get("comp"))?;
    let c = plain_3d(e, src)?;
    let (w, h) = (c.width, c.height);
    let base = item_name(e, src);
    let view = match ps(p, "view").unwrap_or("anaglyph") {
        "anaglyph" => 0,
        "sideBySide" => 1,
        "left" => 2,
        "right" => 3,
        "balanced" => 4,
        other => return Err(format!("view is anaglyph, sideBySide, left, right or balanced, not {other}")),
    };
    let left = new_comp_like(e, src, &format!("{base} Left Eye"), w, h)?;
    let right = new_comp_like(e, src, &format!("{base} Right Eye"), w, h)?;
    let out = new_comp_like(e, src, &format!("{base} Stereo 3D"), w, h)?;
    let ctl = super::layer::new_null(e, &json!({"comp": out, "name": "Stereo 3D Controls"}))?["layer"].as_u64().ok_or("no controls layer")?;
    super::effect::apply(e, &json!({"comp": out, "layers": [ctl], "effect": STEREO_FX}))?;
    let depth = pf(p, "sceneDepth");
    let converge = pb(p, "convergence");
    let l = e.layer_mut(out, ctl)?;
    if let Some(g) = l.props.sub_mut("effects").and_then(|fx| fx.children.iter_mut().find_map(|c| match c {
        Node::Group(g) if matches!(&g.kind, GroupKind::Effect { effect } if effect == STEREO_FX) => Some(g),
        _ => None,
    })) {
        if let Some(pr) = g.get_mut("view") {
            pr.value = Value::Enum(view);
        }
        if let (Some(d), Some(pr)) = (depth, g.get_mut("sceneDepth")) {
            pr.value = Value::Num(d.max(0.0));
        }
        if let (Some(b), Some(pr)) = (converge, g.get_mut("convergence")) {
            pr.value = Value::Bool(b);
        }
    }
    let controls = Some([out, ctl]);
    let pj = e.proj();
    for (cid, eye) in [(left, "left"), (right, "right"), (out, "output")] {
        if let Some(c) = pj.comp_mut(cid) {
            c.stereo = Some(Stereo { source: src, eye: eye.into(), controls });
        }
    }
    super::comp::open(e, &json!({"comp": out}))?;
    e.set_label("Create Stereo 3D Rig");
    Ok(json!({"left": left, "right": right, "output": out, "controls": ctl}))
}

fn shift_layers(c: &mut Comp, dx: f64, dy: f64) {
    let shift = |v: &mut Value| {
        let mut q = v.vec3();
        q[0] += dx;
        q[1] += dy;
        *v = match v {
            Value::Vec2(_) => Value::Vec2([q[0], q[1]]),
            _ => Value::Vec3(q),
        };
    };
    for l in c.layers.iter_mut() {
        if l.parent.is_some() {
            continue;
        }
        let Some(t) = l.props.sub_mut("transform") else { continue };
        for id in ["position", "poi"] {
            if let Some(pr) = t.get_mut(id) {
                shift(&mut pr.value);
                for k in pr.keys.iter_mut() {
                    shift(&mut k.value);
                }
            }
        }
        for id in ["positionX", "positionY"] {
            if let Some(pr) = t.get_mut(id) {
                let d = if id == "positionX" { dx } else { dy };
                if let Value::Num(n) = &mut pr.value {
                    *n += d;
                }
                for k in pr.keys.iter_mut() {
                    if let Value::Num(n) = &mut k.value {
                        *n += d;
                    }
                }
            }
        }
    }
}

fn crop(e: &mut Engine, cid: u64, r: [f64; 4], label: &str) -> Res {
    let (x0, y0) = (r[0].floor(), r[1].floor());
    let (w, h) = (((r[0] + r[2]).ceil() - x0).max(4.0) as u32, ((r[1] + r[3]).ceil() - y0).max(4.0) as u32);
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    c.width = w.min(30000);
    c.height = h.min(30000);
    shift_layers(c, -x0, -y0);
    for g in c.guides.iter_mut() {
        g.position -= if g.vertical { x0 } else { y0 };
    }
    e.state.roi = None;
    e.set_label(label);
    Ok(json!({"comp": cid, "width": w, "height": h, "offset": [x0, y0]}))
}

pub fn crop_to_roi(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let r = match p.get("rect").and_then(Json::as_array) {
        Some(a) if a.len() == 4 => [a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0), a[2].as_f64().unwrap_or(0.0), a[3].as_f64().unwrap_or(0.0)],
        _ => e.state.roi.ok_or("draw a region of interest in the viewer, or pass `rect: [x, y, w, h]`")?,
    };
    if r[2] < 1.0 || r[3] < 1.0 {
        return Err("the region of interest is empty".into());
    }
    crop(e, cid, r, "Crop Comp to Region of Interest")
}

pub fn crop_to_layer_bounds(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select the layers to crop to".into());
    }
    let t = e.time_of(cid);
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let ev = crate::eval::Eval::new(&e.project, cid, comp, t);
    let mut b: Option<[f64; 4]> = None;
    for l in comp.layers.iter().filter(|l| ids.contains(&l.id)) {
        let Some(r) = crate::render::comp::content_bounds(&ev, l) else { continue };
        let m = ev.layer_to_comp(l);
        for (x, y) in [(r[0], r[1]), (r[2], r[1]), (r[2], r[3]), (r[0], r[3])] {
            let q = m.apply(x, y);
            b = Some(match b {
                None => [q[0], q[1], q[0], q[1]],
                Some(o) => [o[0].min(q[0]), o[1].min(q[1]), o[2].max(q[0]), o[3].max(q[1])],
            });
        }
    }
    let b = b.ok_or("the selected layers have no bounds (cameras and lights)")?;
    crop(e, cid, [b[0], b[1], b[2] - b[0], b[3] - b[1]], "Crop Comp to Layer Bounds")
}
