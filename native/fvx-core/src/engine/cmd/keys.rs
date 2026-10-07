use serde_json::{Value as Json, json};

use crate::engine::{Engine, KeyRef, Res, pb, pf, ps};
use crate::model::key::{EASY_INFLUENCE, Ease, Interp, ease_dims, same_time, sort};
use crate::model::{Prop, Value};

fn resolve_key(e: &Engine, k: &Json) -> Result<KeyRef, String> {
    let (_, comp) = e.comp_of(k)?;
    let lid = e.layer_in(comp, k.get("layer").ok_or("each key needs `layer`")?)?;
    let l = comp.layer(lid).ok_or("no layer")?;
    let uid = e.prop_uid(l, k)?;
    let t = pf(k, "time").ok_or("each key needs `time` (layer seconds)")?;
    let pr = l.props.find(uid).ok_or("not a property")?;
    let kt = pr.keys.iter().map(|x| x.time).min_by(|a, b| (a - t).abs().total_cmp(&(b - t).abs())).filter(|x| (x - t).abs() < 0.5 / comp.frame_rate.max(1.0) + 1e-4).ok_or_else(|| format!("no keyframe of {} at {t:.3} s", pr.name))?;
    Ok(KeyRef { layer: lid, prop: uid, time: kt })
}

pub fn select(e: &mut Engine, p: &Json) -> Res {
    let list = p.get("keys").and_then(Json::as_array).cloned().unwrap_or_default();
    let mut refs = vec![];
    for k in &list {
        let mut k = k.clone();
        if k.get("comp").is_none()
            && let Some(c) = p.get("comp")
        {
            k["comp"] = c.clone();
        }
        refs.push(resolve_key(e, &k)?);
    }
    let add = pb(p, "add").unwrap_or(false);
    let toggle = pb(p, "toggle").unwrap_or(false);
    if toggle {
        for r in refs {
            if let Some(i) = e.state.selected_keys.iter().position(|x| x.layer == r.layer && x.prop == r.prop && same_time(x.time, r.time)) {
                e.state.selected_keys.remove(i);
            } else {
                e.state.selected_keys.push(r);
            }
        }
    } else if add {
        for r in refs {
            if !e.state.selected_keys.iter().any(|x| x.layer == r.layer && x.prop == r.prop && same_time(x.time, r.time)) {
                e.state.selected_keys.push(r);
            }
        }
    } else {
        e.state.selected_keys = refs;
    }
    Ok(json!({"selected": e.state.selected_keys.len()}))
}

fn for_selected(e: &mut Engine, mut f: impl FnMut(&mut Prop, usize)) -> Result<usize, String> {
    let sel = e.state.selected_keys.clone();
    if sel.is_empty() {
        return Err("select keyframes first".into());
    }
    let mut n = 0;
    for r in sel {
        let Some(cid) = e.project.comp_of_layer(r.layer) else { continue };
        let Ok(l) = e.layer_mut(cid, r.layer) else { continue };
        let Some(pr) = l.props.find_mut(r.prop) else { continue };
        if let Some(i) = pr.keys.iter().position(|k| same_time(k.time, r.time)) {
            f(pr, i);
            n += 1;
        }
    }
    Ok(n)
}

pub fn delete(e: &mut Engine, _: &Json) -> Res {
    let cid = e.comp_id(None)?;
    let now = e.time_of(cid);
    let sel = std::mem::take(&mut e.state.selected_keys);
    let mut n = 0;
    for r in sel {
        let Ok(l) = e.layer_mut(cid, r.layer) else { continue };
        let lt = l.layer_time(now);
        let Some(pr) = l.props.find_mut(r.prop) else { continue };
        let v = pr.at(lt);
        let before = pr.keys.len();
        pr.keys.retain(|k| !same_time(k.time, r.time));
        n += before - pr.keys.len();
        if pr.keys.is_empty() {
            pr.value = v;
        }
    }
    e.set_label("Delete Keyframes");
    Ok(json!({"deleted": n}))
}

fn easy(e: &mut Engine, inn: bool, out: bool) -> Res {
    let n = for_selected(e, |pr, i| {
        let dims = ease_dims(&pr.keys[i].value, pr.spatial);
        let k = &mut pr.keys[i];
        let ez = vec![Ease { speed: 0.0, influence: EASY_INFLUENCE }; dims];
        k.auto_bezier = false;
        if inn {
            k.in_interp = Interp::Bezier;
            k.in_ease = ez.clone();
        }
        if out {
            k.out_interp = Interp::Bezier;
            k.out_ease = ez;
        }
    })?;
    e.set_label("Easy Ease");
    Ok(json!({"keys": n}))
}

pub fn easy_ease(e: &mut Engine, p: &Json) -> Res {
    match ps(p, "which") {
        Some("in") => easy(e, true, false),
        Some("out") => easy(e, false, true),
        _ => easy(e, true, true),
    }
}

pub fn easy_ease_in(e: &mut Engine, _: &Json) -> Res {
    easy(e, true, false)
}

pub fn easy_ease_out(e: &mut Engine, _: &Json) -> Res {
    easy(e, false, true)
}

pub fn interpolation(e: &mut Engine, p: &Json) -> Res {
    let both = ps(p, "interpolation").map(str::to_ascii_lowercase);
    let inn = ps(p, "in").and_then(Interp::parse);
    let out = ps(p, "out").and_then(Interp::parse);
    let spatial = ps(p, "spatial").map(str::to_ascii_lowercase);
    let roving = pb(p, "roving");
    let n = for_selected(e, |pr, i| {
        let spatial_prop = pr.spatial;
        let k = &mut pr.keys[i];
        if let Some(b) = &both {
            let it = Interp::parse(b).unwrap_or(Interp::Linear);
            k.in_interp = it;
            k.out_interp = it;
            k.auto_bezier = b == "autobezier" || b == "auto";
            k.continuous = b == "continuousbezier" || k.auto_bezier;
            if it == Interp::Bezier && !k.auto_bezier && k.in_ease.is_empty() {
                let d = ease_dims(&k.value, spatial_prop);
                k.in_ease = vec![Ease::default(); d];
                k.out_ease = vec![Ease::default(); d];
            }
        }
        if let Some(x) = inn {
            k.in_interp = x;
        }
        if let Some(x) = out {
            k.out_interp = x;
        }
        if let Some(s) = &spatial {
            k.spatial_auto = s == "autobezier" || s == "auto";
            if s == "linear" {
                k.spatial_in = [0.0; 3];
                k.spatial_out = [0.0; 3];
            }
        }
        if let Some(r) = roving {
            k.roving = r;
        }
    })?;
    if roving.is_some() {
        bake_roving(e)?;
    }
    e.set_label("Keyframe Interpolation");
    Ok(json!({"keys": n}))
}

pub fn bake_roving(e: &mut Engine) -> Result<(), String> {
    let cid = e.comp_id(None)?;
    let sel = e.state.selected_keys.clone();
    let mut new_sel = vec![];
    for r in &sel {
        let Ok(l) = e.layer_mut(cid, r.layer) else { continue };
        let Some(pr) = l.props.find_mut(r.prop) else { continue };
        let idx = pr.keys.iter().position(|k| same_time(k.time, r.time));
        if pr.spatial && crate::model::key::has_roving(&pr.keys) {
            pr.keys = crate::model::key::roving_times(&pr.keys);
        }
        if let Some(i) = idx {
            new_sel.push(crate::engine::KeyRef { layer: r.layer, prop: r.prop, time: pr.keys[i].time });
        }
    }
    e.state.selected_keys = new_sel;
    Ok(())
}

pub fn velocity(e: &mut Engine, p: &Json) -> Res {
    let is = pf(p, "inSpeed");
    let ii = pf(p, "inInfluence");
    let os = pf(p, "outSpeed");
    let oi = pf(p, "outInfluence");
    let n = for_selected(e, |pr, i| {
        let dims = ease_dims(&pr.keys[i].value, pr.spatial);
        let k = &mut pr.keys[i];
        k.auto_bezier = false;
        if is.is_some() || ii.is_some() {
            k.in_interp = Interp::Bezier;
            let cur = k.in_ease.first().copied().unwrap_or_default();
            k.in_ease = vec![Ease { speed: is.unwrap_or(cur.speed), influence: ii.unwrap_or(cur.influence) }; dims];
        }
        if os.is_some() || oi.is_some() {
            k.out_interp = Interp::Bezier;
            let cur = k.out_ease.first().copied().unwrap_or_default();
            k.out_ease = vec![Ease { speed: os.unwrap_or(cur.speed), influence: oi.unwrap_or(cur.influence) }; dims];
        }
    })?;
    e.set_label("Keyframe Velocity");
    Ok(json!({"keys": n}))
}

pub fn set(e: &mut Engine, p: &Json) -> Res {
    let r = resolve_key(e, p)?;
    let cid = e.comp_id(p.get("comp"))?;
    let new_time = pf(p, "newTime");
    let jv = p.get("value").cloned();
    let l = e.layer_mut(cid, r.layer)?;
    let pr = l.props.find_mut(r.prop).ok_or("no property")?;
    let opts = pr.ui.options().to_vec();
    let i = pr.keys.iter().position(|k| same_time(k.time, r.time)).ok_or("no keyframe")?;
    if let Some(j) = jv {
        let v = pr.keys[i].value.parse_like(&j, &opts)?;
        pr.keys[i].value = pr.ui.clamp(v);
    }
    if let Some(t) = new_time {
        let moved = pr.keys.remove(i);
        pr.keys.retain(|k| !same_time(k.time, t));
        pr.keys.push(crate::model::Key { time: t, ..moved });
        sort(&mut pr.keys);
        for s in e.state.selected_keys.iter_mut() {
            if s.layer == r.layer && s.prop == r.prop && same_time(s.time, r.time) {
                s.time = t;
            }
        }
    }
    e.set_label("Edit Keyframe");
    Ok(json!({"time": new_time.unwrap_or(r.time)}))
}

pub fn move_keys(e: &mut Engine, p: &Json) -> Res {
    let d = pf(p, "delta").ok_or("pass `delta` (seconds)")?;
    let cid = e.comp_id(None)?;
    let sel = e.state.selected_keys.clone();
    for r in &sel {
        let Ok(l) = e.layer_mut(cid, r.layer) else { continue };
        let Some(pr) = l.props.find_mut(r.prop) else { continue };
        if let Some(k) = pr.keys.iter_mut().find(|k| same_time(k.time, r.time)) {
            k.time += d;
        }
    }
    for r in &sel {
        if let Ok(l) = e.layer_mut(cid, r.layer)
            && let Some(pr) = l.props.find_mut(r.prop)
        {
            sort(&mut pr.keys);
        }
    }
    for s in e.state.selected_keys.iter_mut() {
        s.time += d;
    }
    e.set_label("Move Keyframes");
    Ok(json!({"moved": sel.len()}))
}

pub fn transform(e: &mut Engine, p: &Json) -> Res {
    let dt = pf(p, "timeOffset").unwrap_or(0.0);
    let dv = pf(p, "valueOffset").unwrap_or(0.0);
    let dim = p.get("dim").and_then(Json::as_u64).map(|x| x as usize);
    let merge = ps(p, "merge").unwrap_or("").to_string();
    let cid = e.comp_id(None)?;
    let fresh = e.key_drag.as_ref().is_none_or(|(m, _)| *m != merge || merge.is_empty());
    if fresh {
        let mut orig = vec![];
        for r in &e.state.selected_keys {
            if let Ok(l) = e.layer_ref(cid, r.layer)
                && let Some(pr) = l.props.find(r.prop)
                && let Some(k) = pr.keys.iter().find(|k| same_time(k.time, r.time))
            {
                orig.push((r.clone(), k.time, k.value.to_json()));
            }
        }
        e.key_drag = Some((merge.clone(), orig));
    }
    let mut orig = e.key_drag.take().map(|x| x.1).unwrap_or_default();
    let mut new_sel = vec![];
    for (r, t0, v0) in orig.iter_mut() {
        let Ok(l) = e.layer_mut(cid, r.layer) else { continue };
        let Some(pr) = l.props.find_mut(r.prop) else { continue };
        let Some(i) = pr.keys.iter().position(|k| same_time(k.time, r.time)) else { continue };
        let base = pr.keys[i].value.parse_like(v0, &[]).unwrap_or_else(|_| pr.keys[i].value.clone());
        let mut comps = base.components();
        if dv != 0.0 {
            match dim {
                Some(d) if d < comps.len() => comps[d] += dv,
                _ => comps.iter_mut().for_each(|c| *c += dv),
            }
        }
        let nv = base.with_components(&comps);
        pr.keys[i].value = if matches!(base, Value::Path(_)) { base } else { nv };
        pr.keys[i].time = *t0 + dt;
        sort(&mut pr.keys);
        r.time = *t0 + dt;
        new_sel.push(r.clone());
    }
    let n = orig.len();
    e.key_drag = Some((merge, orig));
    e.state.selected_keys = new_sel;
    e.set_label("Move Keyframes");
    Ok(json!({"keys": n}))
}

pub fn set_ease(e: &mut Engine, p: &Json) -> Res {
    let r = resolve_key(e, p)?;
    let cid = e.comp_id(p.get("comp"))?;
    let side_out = ps(p, "side").unwrap_or("out") == "out";
    let dim = p.get("dim").and_then(Json::as_u64).map(|x| x as usize);
    let speed = pf(p, "speed");
    let infl = pf(p, "influence");
    let l = e.layer_mut(cid, r.layer)?;
    let pr = l.props.find_mut(r.prop).ok_or("no property")?;
    let spatial = pr.spatial;
    let i = pr.keys.iter().position(|k| same_time(k.time, r.time)).ok_or("no keyframe")?;
    let dims = ease_dims(&pr.keys[i].value, spatial);
    let auto = pr.keys[i].auto_bezier;
    let cur_in: Vec<Ease> = (0..dims).map(|d| crate::model::key::side_ease(&pr.keys, i, d, spatial, false).unwrap_or_default()).collect();
    let cur_out: Vec<Ease> = (0..dims).map(|d| crate::model::key::side_ease(&pr.keys, i, d, spatial, true).unwrap_or_default()).collect();
    let k = &mut pr.keys[i];
    if auto {
        k.auto_bezier = false;
        k.in_ease = cur_in.clone();
        k.out_ease = cur_out.clone();
    }
    let current = if side_out { cur_out } else { cur_in };
    let list = if side_out { &mut k.out_ease } else { &mut k.in_ease };
    if list.len() < dims {
        *list = current;
    }
    for d in 0..dims {
        if dim.is_some_and(|x| x != d) {
            continue;
        }
        if let Some(s) = speed {
            list[d].speed = s;
        }
        if let Some(x) = infl {
            list[d].influence = x.clamp(0.1, 100.0);
        }
    }
    if side_out {
        k.out_interp = Interp::Bezier;
    } else {
        k.in_interp = Interp::Bezier;
    }
    if k.continuous && dims > 0 {
        let src = if side_out { k.out_ease.clone() } else { k.in_ease.clone() };
        if side_out {
            k.in_ease = src.iter().zip(k.in_ease.iter().chain(std::iter::repeat(&Ease::default()))).map(|(s, o)| Ease { speed: s.speed, influence: o.influence }).collect();
        } else {
            k.out_ease = src.iter().zip(k.out_ease.iter().chain(std::iter::repeat(&Ease::default()))).map(|(s, o)| Ease { speed: s.speed, influence: o.influence }).collect();
        }
    }
    e.set_label("Keyframe Velocity");
    Ok(json!({"ok": true}))
}

pub fn set_spatial(e: &mut Engine, p: &Json) -> Res {
    let r = resolve_key(e, p)?;
    let cid = e.comp_id(p.get("comp"))?;
    let v3 = |k: &str| -> Option<[f64; 3]> {
        let a = p.get(k)?.as_array()?;
        Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2).and_then(Json::as_f64).unwrap_or(0.0)])
    };
    let (ti, to) = (v3("in"), v3("out"));
    let l = e.layer_mut(cid, r.layer)?;
    let pr = l.props.find_mut(r.prop).ok_or("no property")?;
    let i = pr.keys.iter().position(|k| same_time(k.time, r.time)).ok_or("no keyframe")?;
    let (ci, co) = crate::model::key::spatial_tangents(&pr.keys, i);
    let k = &mut pr.keys[i];
    k.spatial_auto = false;
    k.spatial_in = ti.unwrap_or(ci);
    k.spatial_out = to.unwrap_or(co);
    e.set_label("Move Tangent");
    Ok(json!({"ok": true}))
}
