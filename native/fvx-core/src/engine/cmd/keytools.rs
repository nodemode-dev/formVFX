use std::collections::BTreeMap;

use serde_json::{Value as Json, json};

use crate::engine::{Engine, KeyRef, Res, pb, pf, ps, pu};
use crate::eval::Eval;
use crate::model::key::{Ease, Interp, Key, insert, same_time, sort};
use crate::model::prop::{Node, Ui};
use crate::model::{Prop, Value, label_parse};

fn by_prop(e: &Engine) -> BTreeMap<(u64, u64), Vec<f64>> {
    let mut m: BTreeMap<(u64, u64), Vec<f64>> = BTreeMap::new();
    for r in &e.state.selected_keys {
        m.entry((r.layer, r.prop)).or_default().push(r.time);
    }
    for v in m.values_mut() {
        v.sort_by(f64::total_cmp);
    }
    m
}

fn need_keys(e: &Engine) -> Result<u64, String> {
    if e.state.selected_keys.is_empty() {
        return Err("select keyframes first".into());
    }
    e.comp_id(None)
}

pub fn toggle_hold(e: &mut Engine, _: &Json) -> Res {
    let cid = need_keys(e)?;
    let sel = e.state.selected_keys.clone();
    let all_hold = sel.iter().all(|r| e.layer_ref(cid, r.layer).ok().and_then(|l| l.props.find(r.prop)).and_then(|p| p.keys.iter().find(|k| same_time(k.time, r.time))).is_some_and(|k| k.out_interp == Interp::Hold));
    for r in &sel {
        let Ok(l) = e.layer_mut(cid, r.layer) else { continue };
        let Some(pr) = l.props.find_mut(r.prop) else { continue };
        if let Some(i) = pr.keys.iter().position(|k| same_time(k.time, r.time)) {
            pr.keys[i].out_interp = if all_hold { Interp::Linear } else { Interp::Hold };
            if i + 1 < pr.keys.len() && !all_hold {
                pr.keys[i + 1].in_interp = Interp::Hold;
            } else if i + 1 < pr.keys.len() && pr.keys[i + 1].in_interp == Interp::Hold {
                pr.keys[i + 1].in_interp = Interp::Linear;
            }
        }
    }
    e.set_label("Toggle Hold Keyframe");
    Ok(json!({"hold": !all_hold, "keys": sel.len()}))
}

pub fn copy(e: &mut Engine, p: &Json) -> Res {
    need_keys(e)?;
    super::edit::copy(e, p)
}

pub fn paste(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let Some(crate::engine::Clip::Keys(keys)) = e.clipboard.clone() else { return Err("copy keyframes first".into()) };
    let now = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let (_, targets) = e.layers_param(p)?;
    if targets.is_empty() {
        return Err("select a layer to paste keyframes onto".into());
    }
    let forced = match p.get("prop").or_else(|| p.get("path")) {
        Some(_) => {
            let l = e.layer_ref(cid, targets[0])?;
            Some(e.prop_uid(l, p)?)
        }
        None => None,
    };
    let mut sel = vec![];
    for lid in targets {
        for (path, dt, k) in &keys {
            let l = e.layer_mut(cid, lid)?;
            let lt = l.layer_time(now + dt);
            let uid = match forced {
                Some(u) => u,
                None => match l.props.resolve_uid(path) {
                    Some(u) => u,
                    None => continue,
                },
            };
            let Some(pr) = l.props.find_mut(uid) else { continue };
            if std::mem::discriminant(&pr.value) != std::mem::discriminant(&k.value) {
                continue;
            }
            insert(&mut pr.keys, Key { time: lt, ..k.clone() });
            sel.push(KeyRef { layer: lid, prop: uid, time: lt });
        }
    }
    let n = sel.len();
    e.state.selected_keys = sel;
    e.set_label("Paste Keyframes");
    Ok(json!({"keys": n}))
}

pub fn select_all(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let mut sel = vec![];
    if let Some(list) = p.get("visible").and_then(Json::as_array) {
        for v in list {
            let (Some(lid), Some(uid)) = (v.get("layer").and_then(Json::as_u64), v.get("prop").and_then(Json::as_u64)) else { continue };
            let Ok(l) = e.layer_ref(cid, lid) else { continue };
            if let Some(pr) = l.props.find(uid) {
                sel.extend(pr.keys.iter().map(|k| KeyRef { layer: lid, prop: uid, time: k.time }));
            }
        }
    } else {
        let (_, mut ids) = e.layers_param(p)?;
        if ids.is_empty() {
            ids = e.project.comp(cid).map(|c| c.layers.iter().map(|l| l.id).collect()).unwrap_or_default();
        }
        for lid in ids {
            let l = e.layer_ref(cid, lid)?;
            l.props.for_each_prop(&mut |pr| sel.extend(pr.keys.iter().map(|k| KeyRef { layer: lid, prop: pr.uid, time: k.time })));
        }
    }
    let n = sel.len();
    e.state.selected_keys = sel;
    Ok(json!({"selected": n}))
}

fn nudge_by(e: &mut Engine, frames: f64, merge: Option<&Json>) -> Res {
    let cid = need_keys(e)?;
    let fd = e.project.comp(cid).map(|c| c.frame_duration()).unwrap_or(1.0 / 30.0);
    let mut p = json!({"delta": frames * fd});
    if let Some(m) = merge {
        p["merge"] = m.clone();
    }
    super::keys::move_keys(e, &p)
}

pub fn nudge(e: &mut Engine, p: &Json) -> Res {
    let f = pf(p, "frames").ok_or("pass `frames`")?;
    nudge_by(e, f, p.get("merge"))
}

pub fn nudge_forward(e: &mut Engine, _: &Json) -> Res {
    nudge_by(e, 1.0, None)
}

pub fn nudge_backward(e: &mut Engine, _: &Json) -> Res {
    nudge_by(e, -1.0, None)
}

pub fn nudge_forward10(e: &mut Engine, _: &Json) -> Res {
    nudge_by(e, 10.0, None)
}

pub fn nudge_backward10(e: &mut Engine, _: &Json) -> Res {
    nudge_by(e, -10.0, None)
}

fn reselect(e: &mut Engine, cid: u64, pick: impl Fn(&Prop, &[f64], &Key) -> bool) -> Res {
    let groups = by_prop(e);
    let mut sel = vec![];
    for ((lid, uid), times) in &groups {
        let Ok(l) = e.layer_ref(cid, *lid) else { continue };
        let Some(pr) = l.props.find(*uid) else { continue };
        for k in &pr.keys {
            if pick(pr, times, k) {
                sel.push(KeyRef { layer: *lid, prop: *uid, time: k.time });
            }
        }
    }
    let n = sel.len();
    e.state.selected_keys = sel;
    Ok(json!({"selected": n}))
}

pub fn select_equal(e: &mut Engine, _: &Json) -> Res {
    let cid = need_keys(e)?;
    reselect(e, cid, |pr, times, k| {
        let vals: Vec<&Value> = pr.keys.iter().filter(|x| times.iter().any(|t| same_time(*t, x.time))).map(|x| &x.value).collect();
        vals.iter().any(|v| **v == k.value)
    })
}

pub fn select_previous(e: &mut Engine, _: &Json) -> Res {
    let cid = need_keys(e)?;
    reselect(e, cid, |_, times, k| k.time <= times[0] + 1e-4)
}

pub fn select_following(e: &mut Engine, _: &Json) -> Res {
    let cid = need_keys(e)?;
    reselect(e, cid, |_, times, k| k.time + 1e-4 >= times[times.len() - 1])
}

pub fn set_label(e: &mut Engine, p: &Json) -> Res {
    let cid = need_keys(e)?;
    let lv = match p.get("label") {
        Some(Json::Number(n)) => n.as_u64().unwrap_or(0).min(16) as u8,
        Some(Json::String(s)) if s.eq_ignore_ascii_case("none") => 0,
        Some(Json::String(s)) => label_parse(s).ok_or_else(|| format!("unknown label `{s}`"))?,
        _ => return Err("pass `label` (a name like Red, none, or 0-16)".into()),
    };
    let sel = e.state.selected_keys.clone();
    for r in &sel {
        let Ok(l) = e.layer_mut(cid, r.layer) else { continue };
        if let Some(k) = l.props.find_mut(r.prop).and_then(|pr| pr.keys.iter_mut().find(|k| same_time(k.time, r.time))) {
            k.label = lv;
        }
    }
    e.set_label("Keyframe Label");
    Ok(json!({"keys": sel.len(), "label": lv}))
}

pub fn select_label_group(e: &mut Engine, p: &Json) -> Res {
    let cid = need_keys(e)?;
    let scope = ps(p, "scope").unwrap_or("all").to_string();
    let visible: Vec<u64> = p.get("visible").and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_u64).collect()).unwrap_or_default();
    let labels: Vec<u8> = e.state.selected_keys.iter().filter_map(|r| e.layer_ref(cid, r.layer).ok()?.props.find(r.prop)?.keys.iter().find(|k| same_time(k.time, r.time)).map(|k| k.label)).collect();
    let props: Vec<(u64, u64)> = by_prop(e).keys().copied().collect();
    let c = e.project.comp(cid).ok_or("no composition")?;
    let mut sel = vec![];
    for l in &c.layers {
        l.props.for_each_prop(&mut |pr| {
            let ok = match scope.as_str() {
                "selected" => props.contains(&(l.id, pr.uid)),
                "visibleSelected" => props.contains(&(l.id, pr.uid)) && (visible.is_empty() || visible.contains(&pr.uid)),
                "visibleAll" => visible.is_empty() || visible.contains(&pr.uid),
                _ => true,
            };
            if ok {
                sel.extend(pr.keys.iter().filter(|k| labels.contains(&k.label)).map(|k| KeyRef { layer: l.id, prop: pr.uid, time: k.time }));
            }
        });
    }
    let n = sel.len();
    e.state.selected_keys = sel;
    Ok(json!({"selected": n}))
}

pub fn time_reverse(e: &mut Engine, _: &Json) -> Res {
    let cid = need_keys(e)?;
    let groups = by_prop(e);
    let mut sel = vec![];
    for ((lid, uid), times) in groups {
        let (a, b) = (times[0], times[times.len() - 1]);
        let l = e.layer_mut(cid, lid)?;
        let Some(pr) = l.props.find_mut(uid) else { continue };
        for k in pr.keys.iter_mut() {
            if times.iter().any(|t| same_time(*t, k.time)) {
                k.time = a + b - k.time;
                std::mem::swap(&mut k.in_interp, &mut k.out_interp);
                std::mem::swap(&mut k.in_ease, &mut k.out_ease);
                std::mem::swap(&mut k.spatial_in, &mut k.spatial_out);
                sel.push(KeyRef { layer: lid, prop: uid, time: k.time });
            }
        }
        sort(&mut pr.keys);
    }
    e.state.selected_keys = sel;
    e.set_label("Time-Reverse Keyframes");
    Ok(json!({"keys": e.state.selected_keys.len()}))
}

fn rnd(seed: u64, i: u64, d: u64) -> f64 {
    let mut z = seed.wrapping_add(i.wrapping_mul(0x9E37_79B9_7F4A_7C15)).wrapping_add(d.wrapping_mul(0xD1B5_4A32_D192_ED03));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

pub fn wiggle(e: &mut Engine, p: &Json) -> Res {
    let cid = need_keys(e)?;
    let temporal = ps(p, "apply") == Some("temporal");
    let smooth = ps(p, "noise") != Some("jagged");
    let dims_mode = ps(p, "dimensions").unwrap_or("independent").to_string();
    let only = pu(p, "dimension").unwrap_or(0) as usize;
    let freq = pf(p, "frequency").unwrap_or(5.0).max(0.01);
    let mag = pf(p, "magnitude").unwrap_or(1.0);
    let seed = pu(p, "seed").unwrap_or(1);
    let groups = by_prop(e);
    let mut added = 0;
    let mut sel = e.state.selected_keys.clone();
    for ((lid, uid), times) in groups {
        if times.len() < 2 {
            continue;
        }
        let (a, b) = (times[0], times[times.len() - 1]);
        let l = e.layer_mut(cid, lid)?;
        let Some(pr) = l.props.find_mut(uid) else { continue };
        if !pr.value.is_numeric() {
            continue;
        }
        let n = ((b - a) * freq).floor() as u64;
        let noise = |i: u64, d: u64| if smooth { (rnd(seed, i.wrapping_sub(1), d) * 0.25 + rnd(seed, i, d) * 0.5 + rnd(seed, i + 1, d) * 0.25) * 1.6 } else { rnd(seed, i, d) };
        let base = pr.clone();
        let mut new = vec![];
        for i in 1..=n {
            let t = a + i as f64 / freq;
            if t >= b - 1e-4 {
                break;
            }
            let v = if temporal {
                let dt = (noise(i, 0) * mag / 100.0 / freq).clamp(-0.45 / freq, 0.45 / freq);
                base.at((t + dt).clamp(a, b))
            } else {
                let mut c = base.at(t).components();
                for (d, x) in c.iter_mut().enumerate() {
                    let r = match dims_mode.as_str() {
                        "one" if d != only => 0.0,
                        "same" => noise(i, 0),
                        _ => noise(i, d as u64),
                    };
                    *x += r * mag;
                }
                base.at(t).with_components(&c)
            };
            new.push(Key::new(t, pr.ui.clamp(v), pr.spatial));
        }
        for k in new {
            sel.push(KeyRef { layer: lid, prop: uid, time: k.time });
            insert(&mut pr.keys, k);
            added += 1;
        }
    }
    if added == 0 {
        return Err("select at least two keyframes of one property".into());
    }
    e.state.selected_keys = sel;
    e.set_label("Wiggler");
    Ok(json!({"added": added}))
}

fn dist(a: &Value, b: &Value) -> f64 {
    a.components().iter().zip(b.components()).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

fn simplify(keys: &[Key], tol: f64) -> Vec<bool> {
    let n = keys.len();
    let mut keep = vec![false; n];
    if n == 0 {
        return keep;
    }
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((i, j)) = stack.pop() {
        if j <= i + 1 {
            continue;
        }
        let (a, b) = (&keys[i], &keys[j]);
        let span = (b.time - a.time).max(1e-9);
        let mut worst = (0.0, 0usize);
        for (k, key) in keys.iter().enumerate().take(j).skip(i + 1) {
            let lin = a.value.lerp(&b.value, (key.time - a.time) / span);
            let d = dist(&lin, &key.value);
            if d > worst.0 {
                worst = (d, k);
            }
        }
        if worst.0 > tol {
            keep[worst.1] = true;
            stack.push((i, worst.1));
            stack.push((worst.1, j));
        }
    }
    keep
}

fn auto(k: &mut Key) {
    k.in_interp = Interp::Bezier;
    k.out_interp = Interp::Bezier;
    k.auto_bezier = true;
    k.continuous = true;
    k.in_ease.clear();
    k.out_ease.clear();
}

fn smooth_range(pr: &mut Prop, a: f64, b: f64, tol: f64) -> usize {
    let span: Vec<Key> = pr.keys.iter().filter(|k| k.time + 1e-6 >= a && k.time <= b + 1e-6).cloned().collect();
    if span.len() < 3 || !pr.value.is_numeric() {
        return 0;
    }
    let keep = simplify(&span, tol);
    let mut kept: Vec<Key> = span.into_iter().zip(&keep).filter(|(_, k)| **k).map(|(k, _)| k).collect();
    let removed = keep.iter().filter(|k| !**k).count();
    let last = kept.len().saturating_sub(1);
    for (i, k) in kept.iter_mut().enumerate() {
        if i > 0 && i < last {
            auto(k);
        }
    }
    pr.keys.retain(|k| k.time + 1e-6 < a || k.time > b + 1e-6);
    pr.keys.extend(kept);
    sort(&mut pr.keys);
    removed
}

pub fn smooth(e: &mut Engine, p: &Json) -> Res {
    let cid = need_keys(e)?;
    let tol = p.get("tolerance").and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))).unwrap_or(1.0).max(0.0);
    let groups = by_prop(e);
    let mut removed = 0;
    let mut sel = vec![];
    for ((lid, uid), times) in groups {
        let (a, b) = (times[0], times[times.len() - 1]);
        let l = e.layer_mut(cid, lid)?;
        let Some(pr) = l.props.find_mut(uid) else { continue };
        removed += smooth_range(pr, a, b, tol);
        sel.extend(pr.keys.iter().filter(|k| k.time + 1e-6 >= a && k.time <= b + 1e-6).map(|k| KeyRef { layer: lid, prop: uid, time: k.time }));
    }
    e.state.selected_keys = sel;
    e.set_label("Smoother");
    Ok(json!({"removed": removed}))
}

pub fn exponential_scale(e: &mut Engine, _: &Json) -> Res {
    let cid = need_keys(e)?;
    let groups = by_prop(e);
    let ((lid, uid), times) = groups.into_iter().next().ok_or("select two Scale keyframes")?;
    if times.len() != 2 {
        return Err("select exactly two Scale keyframes".into());
    }
    let fd = e.project.comp(cid).map(|c| c.frame_duration()).unwrap_or(1.0 / 30.0);
    let l = e.layer_mut(cid, lid)?;
    let stretch = (l.stretch / 100.0).abs().max(1e-6);
    let pr = l.props.find_mut(uid).ok_or("no property")?;
    if pr.id != "scale" {
        return Err("Exponential Scale works on two Scale keyframes".into());
    }
    let (a, b) = (times[0], times[1]);
    let (s0, s1) = (pr.at(a).components(), pr.at(b).components());
    pr.keys.retain(|k| k.time <= a + 1e-6 || k.time + 1e-6 >= b);
    let step = fd / stretch;
    let mut t = a + step;
    let mut n = 0;
    let template = pr.at(a);
    while t < b - step * 0.5 {
        let u = (t - a) / (b - a);
        let c: Vec<f64> = s0.iter().zip(&s1).map(|(x, y)| if *x > 1e-9 && *y > 1e-9 { x * (y / x).powf(u) } else { x + (y - x) * u }).collect();
        insert(&mut pr.keys, Key::new(t, template.with_components(&c), false));
        n += 1;
        t += step;
    }
    e.set_label("Exponential Scale");
    Ok(json!({"added": n}))
}

pub fn audio_to_keyframes(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    if !crate::render::audio::comp_has_audio(&e.project, cid) {
        return Err("this composition has no audio".into());
    }
    let c = e.project.comp(cid).ok_or("no composition")?;
    let (fps, wa, size) = (c.frame_rate.max(1.0), c.work_area, (c.width as f64, c.height as f64));
    let rate = 48_000u32;
    let per = (rate as f64 / fps).round().max(1.0) as usize;
    let frames = ((wa[1] - wa[0]) * fps).round().max(1.0) as usize;
    let buf = crate::render::audio::mix_comp(&e.project, &e.media, cid, wa[0], frames * per, rate);
    let mut chans: [Vec<(f64, f64)>; 3] = [vec![], vec![], vec![]];
    for f in 0..frames {
        let s = &buf[f * per * 2..((f + 1) * per * 2).min(buf.len())];
        let (mut l, mut r) = (0.0f64, 0.0f64);
        for fr in s.chunks_exact(2) {
            l += (fr[0] as f64).powi(2);
            r += (fr[1] as f64).powi(2);
        }
        let k = (s.len() / 2).max(1) as f64;
        let (l, r) = ((l / k).sqrt() * 100.0, (r / k).sqrt() * 100.0);
        let t = f as f64 / fps;
        chans[0].push((t, l));
        chans[1].push((t, r));
        chans[2].push((t, (l + r) / 2.0));
    }
    super::layer::new_null(e, &json!({"comp": cid, "name": "Audio Amplitude"}))?;
    let lid = *e.state.selected_layers.first().ok_or("no layer")?;
    let spec = crate::effects::find("fx.control.slider").ok_or("no slider control")?;
    for (i, name) in ["Left Channel", "Right Channel", "Both Channels"].iter().enumerate() {
        let mut g = {
            let pj = e.proj();
            let mut ids = pj.ids();
            crate::effects::instance(spec, &mut ids, name, [size.0, size.1])
        };
        if let Some(pr) = g.get_mut("slider") {
            pr.keys = chans[i].iter().map(|(t, v)| Key::new(*t + wa[0], Value::Num(*v), false)).collect();
        }
        let l = e.layer_mut(cid, lid)?;
        l.props.sub_mut("effects").ok_or("no effects group")?.children.push(Node::Group(g));
    }
    e.set_label("Convert Audio to Keyframes");
    Ok(json!({"layer": lid, "frames": frames}))
}

fn parent_inverse(e: &Engine, cid: u64, lid: u64, t: f64) -> Option<crate::geom::Mat3> {
    let c = e.project.comp(cid)?;
    let l = c.layer(lid)?;
    Eval::new(&e.project, cid, c, t).parent_to_comp(l).inverse()
}

pub fn motion_sketch(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let start = pf(p, "start").unwrap_or_else(|| e.time_of(cid));
    let speed = (pf(p, "captureSpeed").unwrap_or(100.0) / 100.0).max(0.01);
    let tol = pf(p, "smoothing").unwrap_or(1.0).max(0.0);
    let fd = e.project.comp(cid).map(|c| c.frame_duration()).ok_or("no composition")?;
    let pts = p.get("points").and_then(Json::as_array).ok_or("pass `points`")?;
    let mut samples: Vec<(f64, f64, f64)> = vec![];
    for (i, pt) in pts.iter().enumerate() {
        let a: Vec<f64> = pt.as_array().map(|a| a.iter().filter_map(Json::as_f64).collect()).unwrap_or_default();
        match a.len() {
            2 => samples.push((start + i as f64 * fd, a[0], a[1])),
            n if n >= 3 => samples.push((start + a[0] / speed, a[1], a[2])),
            _ => {}
        }
    }
    if samples.len() < 2 {
        return Err("motion sketch needs at least two points".into());
    }
    let inv = parent_inverse(e, cid, lid, start).ok_or("the parent transform cannot be inverted")?;
    let c = e.project.comp(cid).ok_or("no composition")?;
    let mut frames: BTreeMap<i64, (f64, f64)> = BTreeMap::new();
    for (t, x, y) in samples {
        let q = inv.apply(x, y);
        frames.insert((c.snap(t) / fd).round() as i64, (q[0], q[1]));
    }
    let (t0, t1) = (*frames.keys().next().unwrap_or(&0) as f64 * fd, *frames.keys().last().unwrap_or(&0) as f64 * fd);
    let l = e.layer_mut(cid, lid)?;
    let (lt0, lt1) = (l.layer_time(t0), l.layer_time(t1));
    let separated = l.transform().is_some_and(|t| t.get("xPosition").is_some());
    let comp_times: Vec<(f64, f64, f64)> = frames.iter().map(|(f, (x, y))| (*f as f64 * fd, *x, *y)).collect();
    let tg = l.props.sub_mut("transform").ok_or("this layer has no transform")?;
    let z = tg.get("position").map(|pr| pr.at(lt0).vec3()[2]).unwrap_or(0.0);
    let mut uids = vec![];
    let stretch = |t: f64| (t - t0) / (t1 - t0).max(1e-9) * (lt1 - lt0) + lt0;
    if separated {
        for (id, axis) in [("xPosition", 0usize), ("yPosition", 1)] {
            let pr = tg.get_mut(id).ok_or("no position")?;
            pr.keys.retain(|k| k.time < lt0.min(lt1) - 1e-6 || k.time > lt0.max(lt1) + 1e-6);
            for (t, x, y) in &comp_times {
                insert(&mut pr.keys, Key::new(stretch(*t), Value::Num(if axis == 0 { *x } else { *y }), false));
            }
            smooth_range(pr, lt0.min(lt1), lt0.max(lt1), tol);
            uids.push(pr.uid);
        }
    } else {
        let pr = tg.get_mut("position").ok_or("no position")?;
        pr.keys.retain(|k| k.time < lt0.min(lt1) - 1e-6 || k.time > lt0.max(lt1) + 1e-6);
        for (t, x, y) in &comp_times {
            insert(&mut pr.keys, Key::new(stretch(*t), Value::Vec3([*x, *y, z]), true));
        }
        smooth_range(pr, lt0.min(lt1), lt0.max(lt1), tol);
        for k in pr.keys.iter_mut() {
            k.spatial_auto = true;
        }
        uids.push(pr.uid);
    }
    let n = comp_times.len();
    e.set_label("Motion Sketch");
    Ok(json!({"layer": lid, "samples": n, "props": uids}))
}

pub fn add_keyframe(e: &mut Engine, _: &Json) -> Res {
    let cid = e.comp_id(None)?;
    let now = e.time_of(cid);
    let props = e.state.selected_props.clone();
    if props.is_empty() {
        return Err("select properties first".into());
    }
    let mut n = 0;
    for (lid, uid) in props {
        let v = {
            let Some(c) = e.project.comp(cid) else { continue };
            let Some(l) = c.layer(lid) else { continue };
            let Some(pr) = l.props.find(uid) else { continue };
            if pr.static_only {
                continue;
            }
            (l.layer_time(now), pr.at(l.layer_time(now)))
        };
        let l = e.layer_mut(cid, lid)?;
        let Some(pr) = l.props.find_mut(uid) else { continue };
        let spatial = pr.spatial;
        insert(&mut pr.keys, Key::new(v.0, v.1, spatial));
        n += 1;
    }
    e.set_label("Add Keyframe");
    Ok(json!({"keys": n}))
}

pub fn toggle_transform(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let which = ps(p, "prop").ok_or("pass `prop` (anchor, position, scale, rotation or opacity)")?.to_string();
    let now = e.time_of(cid);
    let mut out = vec![];
    for lid in ids {
        let l = e.layer_mut(cid, lid)?;
        let lt = l.layer_time(now);
        let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut(&which)) else { continue };
        if let Some(i) = pr.keys.iter().position(|k| same_time(k.time, lt)) {
            let v = pr.at(lt);
            pr.keys.remove(i);
            if pr.keys.is_empty() {
                pr.value = v;
            }
            out.push(json!({"layer": lid, "key": false}));
        } else {
            let v = pr.at(lt);
            let spatial = pr.spatial;
            insert(&mut pr.keys, Key::new(lt, v, spatial));
            out.push(json!({"layer": lid, "key": true}));
        }
    }
    e.set_label("Add or Remove Keyframe");
    Ok(json!({"layers": out}))
}

fn split_prop(e: &mut Engine, src: &Prop, axis: usize, id: &str, name: &str) -> Prop {
    let pj = e.proj();
    let mut ids = pj.ids();
    let mut pr = ids.prop(id, name, Value::Num(src.value.vec3()[axis]), Ui::Pixels);
    if axis == 2 {
        pr = pr.only_3d();
    }
    pr.keys = src
        .keys
        .iter()
        .map(|k| {
            let mut n = Key::new(k.time, Value::Num(k.value.vec3()[axis]), false);
            n.in_interp = k.in_interp;
            n.out_interp = k.out_interp;
            n.auto_bezier = k.auto_bezier;
            n.continuous = k.continuous;
            n.label = k.label;
            n.in_ease = k.in_ease.first().map(|x| vec![*x]).unwrap_or_default();
            n.out_ease = k.out_ease.first().map(|x| vec![*x]).unwrap_or_default();
            n
        })
        .collect();
    pr.expr = src.expr.clone();
    pr
}

pub fn separate_dimensions(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    let tg = l.transform().ok_or("this layer has no transform")?;
    let on = tg.get("xPosition").is_some();
    let want = pb(p, "value").unwrap_or(!on);
    if want == on {
        return Ok(json!({"separated": on}));
    }
    if want {
        let src = tg.get("position").ok_or("no position")?.clone();
        let made: Vec<Prop> = [(0usize, "xPosition", "X Position"), (1, "yPosition", "Y Position"), (2, "zPosition", "Z Position")].iter().map(|(a, id, n)| split_prop(e, &src, *a, id, n)).collect();
        let l = e.layer_mut(cid, lid)?;
        let tg = l.props.sub_mut("transform").ok_or("no transform")?;
        let at = tg.children.iter().position(|c| c.id() == "position").map(|i| i + 1).unwrap_or(tg.children.len());
        for (k, pr) in made.into_iter().enumerate() {
            tg.children.insert(at + k, Node::Prop(pr));
        }
    } else {
        let l = e.layer_mut(cid, lid)?;
        let tg = l.props.sub_mut("transform").ok_or("no transform")?;
        let parts: Vec<Prop> = ["xPosition", "yPosition", "zPosition"].iter().filter_map(|id| tg.get(id).cloned()).collect();
        let mut times: Vec<f64> = parts.iter().flat_map(|pr| pr.keys.iter().map(|k| k.time)).collect();
        times.sort_by(f64::total_cmp);
        times.dedup_by(|a, b| same_time(*a, *b));
        let at = |t: f64| -> [f64; 3] {
            let mut v = [0.0; 3];
            for (i, pr) in parts.iter().enumerate().take(3) {
                v[i] = pr.at(t).num();
            }
            v
        };
        let pos = tg.get_mut("position").ok_or("no position")?;
        pos.keys = times
            .iter()
            .map(|t| {
                let mut k = Key::new(*t, Value::Vec3(at(*t)), true);
                if let Some(src) = parts.first().and_then(|pr| pr.keys.iter().find(|x| same_time(x.time, *t))) {
                    k.in_interp = src.in_interp;
                    k.out_interp = src.out_interp;
                    k.in_ease = src.in_ease.first().map(|x: &Ease| vec![*x]).unwrap_or_default();
                    k.out_ease = src.out_ease.first().map(|x: &Ease| vec![*x]).unwrap_or_default();
                }
                k
            })
            .collect();
        if times.is_empty() {
            pos.value = Value::Vec3(at(0.0));
        }
        tg.children.retain(|c| !matches!(c.id(), "xPosition" | "yPosition" | "zPosition"));
    }
    let ids: Vec<u64> = e.layer_ref(cid, lid)?.transform().map(|t| t.children.iter().map(|c| c.uid()).collect()).unwrap_or_default();
    e.state.selected_keys.retain(|k| k.layer != lid || ids.contains(&k.prop));
    e.set_label(if want { "Separate Dimensions" } else { "Merge Dimensions" });
    Ok(json!({"separated": want}))
}
