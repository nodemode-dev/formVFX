use serde_json::{Value as Json, json};

use super::layermisc::mask_uid;
use crate::engine::{Engine, Res, pb, pf, ps, pu};
use crate::model::Value;
use crate::model::key::{self, Key};
use crate::model::prop::{Group, Node, Ui};
use crate::render::maskfx::{self, FALLOFFS, FeatherPoint, MOTION_BLUR};

fn mask_group(e: &mut Engine, p: &Json) -> Result<(u64, u64, u64), String> {
    let (cid, lid) = e.layer_param(p)?;
    let uid = mask_uid(e, cid, lid, p)?;
    Ok((cid, lid, uid))
}

fn ensure(e: &mut Engine, cid: u64, lid: u64, uid: u64, id: &str, name: &str, v: Value, ui: Ui) -> Result<(), String> {
    let has = e.layer_ref(cid, lid)?.props.find_group(uid).ok_or("no such mask")?.get(id).is_some();
    if !has {
        let pr = e.proj().ids().prop(id, name, v, ui);
        e.layer_mut(cid, lid)?.props.find_group_mut(uid).ok_or("no such mask")?.children.push(Node::Prop(pr));
    }
    Ok(())
}

fn points_of(g: &Group) -> Vec<FeatherPoint> {
    match g.get("featherPoints").map(|p| &p.value) {
        Some(Value::Str(s)) => maskfx::sorted(maskfx::parse(s)),
        _ => vec![],
    }
}

fn write_points(e: &mut Engine, cid: u64, lid: u64, uid: u64, pts: Vec<FeatherPoint>) -> Result<Vec<FeatherPoint>, String> {
    ensure(e, cid, lid, uid, "featherPoints", "Feather Points", Value::Str("[]".into()), Ui::Hidden)?;
    let pts = maskfx::sorted(pts);
    let g = e.layer_mut(cid, lid)?.props.find_group_mut(uid).ok_or("no such mask")?;
    if let Some(pr) = g.get_mut("featherPoints") {
        pr.value = Value::Str(maskfx::encode(&pts));
    }
    Ok(pts)
}

fn segments(e: &Engine, cid: u64, lid: u64, uid: u64) -> Result<usize, String> {
    let g = e.layer_ref(cid, lid)?.props.find_group(uid).ok_or("no such mask")?;
    let path = g.get("path").and_then(|p| p.value.as_path().cloned()).ok_or("the mask has no path")?;
    Ok(path.segments())
}

fn points_json(pts: &[FeatherPoint]) -> Json {
    Json::Array(pts.iter().enumerate().map(|(i, p)| json!({"index": i, "segment": p.segment, "t": p.t, "radius": p.radius, "tension": p.tension * 100.0})).collect())
}

pub fn feather_add(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = mask_group(e, p)?;
    let segs = segments(e, cid, lid, uid)?;
    let segment = pu(p, "segment").ok_or("pass `segment` (0-based)")? as usize;
    if segment >= segs.max(1) {
        return Err(format!("the mask has {segs} segments"));
    }
    let pt = FeatherPoint { segment, t: pf(p, "t").unwrap_or(0.5).clamp(0.0, 1.0), radius: pf(p, "radius").unwrap_or(10.0), tension: (pf(p, "tension").unwrap_or(0.0) / 100.0).clamp(0.0, 1.0) };
    let mut pts = points_of(e.layer_ref(cid, lid)?.props.find_group(uid).ok_or("no such mask")?);
    pts.push(pt);
    let pts = write_points(e, cid, lid, uid, pts)?;
    let index = pts.iter().position(|q| *q == pt).unwrap_or(0);
    e.set_label("Add Feather Point");
    Ok(json!({"index": index, "points": points_json(&pts)}))
}

pub fn feather_set(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = mask_group(e, p)?;
    let segs = segments(e, cid, lid, uid)?;
    let mut pts = points_of(e.layer_ref(cid, lid)?.props.find_group(uid).ok_or("no such mask")?);
    let i = pu(p, "index").ok_or("pass `index`")? as usize;
    let pt = pts.get_mut(i).ok_or("no feather point at that index")?;
    if let Some(s) = pu(p, "segment") {
        pt.segment = (s as usize).min(segs.saturating_sub(1));
    }
    if let Some(t) = pf(p, "t") {
        pt.t = t.clamp(0.0, 1.0);
    }
    if let Some(r) = pf(p, "radius") {
        pt.radius = r;
    }
    if let Some(t) = pf(p, "tension") {
        pt.tension = (t / 100.0).clamp(0.0, 1.0);
    }
    let pts = write_points(e, cid, lid, uid, pts)?;
    if let Some(m) = ps(p, "merge") {
        e.merge_hint = Some(m.to_string());
    }
    e.set_label("Move Feather Point");
    Ok(json!({"points": points_json(&pts)}))
}

pub fn feather_remove(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = mask_group(e, p)?;
    let mut pts = points_of(e.layer_ref(cid, lid)?.props.find_group(uid).ok_or("no such mask")?);
    if pb(p, "all").unwrap_or(false) {
        pts.clear();
    } else {
        let i = pu(p, "index").ok_or("pass `index` or `all: true`")? as usize;
        if i >= pts.len() {
            return Err("no feather point at that index".into());
        }
        pts.remove(i);
    }
    let pts = write_points(e, cid, lid, uid, pts)?;
    e.set_label("Delete Feather Point");
    Ok(json!({"points": points_json(&pts)}))
}

pub fn feather_list(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = mask_group(e, p)?;
    let g = e.layer_ref(cid, lid)?.props.find_group(uid).ok_or("no such mask")?;
    let falloff = g.get("featherFalloff").map(|p| p.value.index()).unwrap_or(0) as usize;
    Ok(json!({"mask": uid, "falloff": FALLOFFS[falloff.min(1)], "points": points_json(&points_of(g))}))
}

pub fn feather_falloff(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = mask_group(e, p)?;
    let v = match ps(p, "falloff").unwrap_or("smooth").to_ascii_lowercase().as_str() {
        "smooth" => 0,
        "linear" => 1,
        other => return Err(format!("falloff is smooth or linear, not {other}")),
    };
    ensure(e, cid, lid, uid, "featherFalloff", "Mask Feather Falloff", Value::Enum(0), Ui::Hidden)?;
    if let Some(pr) = e.layer_mut(cid, lid)?.props.find_group_mut(uid).and_then(|g| g.get_mut("featherFalloff")) {
        pr.value = Value::Enum(v);
    }
    e.set_label("Mask Feather Falloff");
    Ok(json!({"mask": uid, "falloff": FALLOFFS[v as usize]}))
}

pub fn motion_blur(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = mask_group(e, p)?;
    let v = match ps(p, "mode").unwrap_or("sameAsLayer") {
        "sameAsLayer" => 0,
        "on" => 1,
        "off" => 2,
        other => return Err(format!("mode is sameAsLayer, on or off, not {other}")),
    };
    ensure(e, cid, lid, uid, "motionBlur", "Mask Motion Blur", Value::Enum(0), Ui::Hidden)?;
    if let Some(pr) = e.layer_mut(cid, lid)?.props.find_group_mut(uid).and_then(|g| g.get_mut("motionBlur")) {
        pr.value = Value::Enum(v);
    }
    e.set_label("Mask Motion Blur");
    Ok(json!({"mask": uid, "motionBlur": MOTION_BLUR[v as usize]}))
}

fn interp_defaults() -> Json {
    json!({"rate": 0, "keyframeFields": false, "linearVertexPaths": false, "bendingResistance": 50, "quality": 50, "addVertices": true, "vertexSpacing": 10, "firstVerticesMatch": true})
}

pub fn interpolation_options(e: &mut Engine, p: &Json) -> Res {
    let mut cur = e.prefs.get("maskInterpolation").cloned().unwrap_or_else(interp_defaults);
    if let (Some(o), Some(src)) = (cur.as_object_mut(), p.as_object()) {
        for (k, v) in src {
            if interp_defaults().get(k).is_some() {
                o.insert(k.clone(), v.clone());
            }
        }
    }
    if p.as_object().is_some_and(|o| !o.is_empty()) {
        e.prefs.insert("maskInterpolation".into(), cur.clone());
    }
    Ok(cur)
}

pub fn interpolate(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = mask_group(e, p)?;
    let opts = e.prefs.get("maskInterpolation").cloned().unwrap_or_else(interp_defaults);
    let get_b = |k: &str| pb(p, k).or_else(|| opts.get(k).and_then(Json::as_bool)).unwrap_or(false);
    let get_f = |k: &str, d: f64| pf(p, k).or_else(|| opts.get(k).and_then(Json::as_f64)).unwrap_or(d);
    let linear = get_b("linearVertexPaths");
    let add = pb(p, "addVertices").or_else(|| opts.get("addVertices").and_then(Json::as_bool)).unwrap_or(true);
    let first = pb(p, "firstVerticesMatch").or_else(|| opts.get("firstVerticesMatch").and_then(Json::as_bool)).unwrap_or(true);
    let bending = get_f("bendingResistance", 50.0);
    let fps = e.project.comp(cid).map(|c| c.frame_rate).ok_or("no composition")?;
    let rate = get_f("rate", 0.0);
    let rate = if rate > 0.0 { rate.min(fps) } else { fps };
    let (keys, l_stretch) = {
        let l = e.layer_ref(cid, lid)?;
        let g = l.props.find_group(uid).ok_or("no such mask")?;
        let pr = g.get("path").ok_or("the mask has no path")?;
        (pr.keys.clone(), l.stretch)
    };
    if keys.len() < 2 {
        return Err("the mask path needs at least two keyframes".into());
    }
    let (ka, kb) = match (pf(p, "from"), pf(p, "to")) {
        (Some(a), Some(b)) => {
            let near = |t: f64| keys.iter().min_by(|x, y| (x.time - t).abs().total_cmp(&(y.time - t).abs())).cloned();
            (near(a).ok_or("no key")?, near(b).ok_or("no key")?)
        }
        _ => (keys[0].clone(), keys[1].clone()),
    };
    if (kb.time - ka.time).abs() < 1e-9 {
        return Err("pick two different keyframes".into());
    }
    let (ka, kb) = if ka.time < kb.time { (ka, kb) } else { (kb, ka) };
    let (Some(pa), Some(pbz)) = (ka.value.as_path(), kb.value.as_path()) else { return Err("the keys hold no mask shapes".into()) };
    let m = maskfx::match_paths(pa, pbz, add, first)?;
    let step = 1.0 / rate * 100.0 / if l_stretch.abs() < 1e-9 { 100.0 } else { l_stretch.abs() };
    let span = kb.time - ka.time;
    let n = ((span / step).round() as usize).max(1);
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_group_mut(uid).and_then(|g| g.get_mut("path")).ok_or("the mask has no path")?;
    key::insert(&mut pr.keys, Key::new(ka.time, Value::Path(m.a.clone()), false));
    key::insert(&mut pr.keys, Key::new(kb.time, Value::Path(m.b.clone()), false));
    let mut made = 0;
    for i in 1..n {
        let t = i as f64 / n as f64;
        key::insert(&mut pr.keys, Key::new(ka.time + span * t, Value::Path(maskfx::between(&m, t, linear, bending)), false));
        made += 1;
    }
    e.set_label("Smart Mask Interpolation");
    Ok(json!({"mask": uid, "keys": made, "from": ka.time, "to": kb.time, "vertices": m.a.vertices.len()}))
}
