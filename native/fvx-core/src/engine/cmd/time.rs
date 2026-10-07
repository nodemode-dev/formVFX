use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pb, pf, ps};
use crate::model::key::{Interp, Key, insert, same_time};
use crate::model::prop::{Node, Prop, Ui};
use crate::model::{ItemKind, Layer, Marker, Value};

fn layers(e: &Engine, p: &Json) -> Result<(u64, Vec<u64>), String> {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select a layer first".into());
    }
    Ok((cid, ids))
}

fn restretch(l: &mut Layer, s: f64, hold: f64) {
    let (lt_in, lt_out) = (l.layer_time(l.in_point), l.layer_time(l.out_point));
    let lh = l.layer_time(hold);
    l.stretch = s;
    l.start_time = hold - lh * s / 100.0;
    let (a, b) = (l.comp_time(lt_in), l.comp_time(lt_out));
    l.in_point = a.min(b);
    l.out_point = a.max(b);
}

pub fn time_stretch(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = layers(e, p)?;
    let now = e.time_of(cid);
    let reverse = ps(p, "op") == Some("reverse");
    let hold = ps(p, "hold").unwrap_or("in").to_string();
    let percent = pf(p, "percent").or_else(|| pf(p, "stretch"));
    let duration = pf(p, "duration");
    if percent.is_none() && duration.is_none() && !reverse {
        return Err("pass `percent` or `duration` (s)".into());
    }
    let mut out = vec![];
    for lid in ids {
        let l = e.layer_mut(cid, lid)?;
        let cur = if l.stretch.abs() < 1e-9 { 100.0 } else { l.stretch };
        let mut s = match (percent, duration) {
            (Some(x), _) => x,
            (None, Some(d)) => cur.abs() * d / (l.out_point - l.in_point).max(1e-6),
            _ => cur.abs(),
        };
        if s.abs() < 1e-3 {
            return Err("the stretch cannot be zero".into());
        }
        if reverse || (percent.is_none() && cur < 0.0) {
            s = -s.abs();
        }
        let h = match hold.as_str() {
            "current" => now,
            "out" => l.out_point,
            _ => l.in_point,
        };
        restretch(l, s, h);
        out.push(json!({"layer": lid, "stretch": l.stretch, "in": l.in_point, "out": l.out_point}));
    }
    e.set_label("Time Stretch");
    Ok(json!({"layers": out}))
}

pub fn time_reverse(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = layers(e, p)?;
    for lid in &ids {
        let l = e.layer_mut(cid, *lid)?;
        let s = if l.stretch.abs() < 1e-9 { 100.0 } else { l.stretch };
        l.start_time = l.in_point + l.out_point - l.start_time;
        l.stretch = -s;
    }
    e.set_label("Time-Reverse Layer");
    Ok(json!({"layers": ids}))
}

fn source_duration(e: &Engine, l: &Layer) -> Option<f64> {
    match &e.project.item(l.source?)?.kind {
        ItemKind::Footage(f) if !f.is_still() => Some(f.duration * f.loops.max(1) as f64),
        ItemKind::Comp(c) => Some(c.duration),
        _ => None,
    }
}

fn remap_prop(e: &mut Engine, max: f64) -> Prop {
    let pj = e.proj();
    let mut ids = pj.ids();
    ids.prop("timeRemap", "Time Remap", Value::Num(0.0), Ui::slider(-100_000.0, 100_000.0, 0.0, max.max(1.0), 2))
}

fn remap_on(e: &mut Engine, cid: u64, lid: u64) -> Result<bool, String> {
    let (lt_in, lt_out, max, has) = {
        let l = e.layer_ref(cid, lid)?;
        if !matches!(l.kind, crate::model::LayerKind::Footage | crate::model::LayerKind::Precomp) {
            return Err(format!("{} has no time to remap (footage and precomp layers only)", l.name));
        }
        let max = source_duration(e, l).unwrap_or(l.out_point - l.in_point);
        (l.layer_time(l.in_point), l.layer_time(l.out_point), max, l.props.get("timeRemap").is_some())
    };
    if has {
        return Ok(false);
    }
    let fd = e.project.comp(cid).map(|c| c.frame_duration()).unwrap_or(1.0 / 30.0);
    let (a, b) = (lt_in.min(lt_out), lt_in.max(lt_out));
    let mut pr = remap_prop(e, max);
    pr.value = Value::Num(a);
    pr.keys.push(Key::new(a, Value::Num(a), false));
    let last = (b - fd).max(a);
    if last > a + 1e-6 {
        pr.keys.push(Key::new(last, Value::Num(last), false));
    }
    let l = e.layer_mut(cid, lid)?;
    l.props.children.insert(0, Node::Prop(pr));
    Ok(true)
}

pub fn enable_time_remap(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = layers(e, p)?;
    let want = pb(p, "value");
    let mut out = vec![];
    for lid in ids {
        let has = e.layer_ref(cid, lid)?.props.get("timeRemap").is_some();
        let on = want.unwrap_or(!has);
        if on {
            remap_on(e, cid, lid)?;
        } else if has {
            let l = e.layer_mut(cid, lid)?;
            l.props.children.retain(|c| c.id() != "timeRemap");
        }
        out.push(json!({"layer": lid, "timeRemap": on}));
    }
    e.set_label("Time Remap");
    Ok(json!({"layers": out}))
}

fn source_at(e: &Engine, cid: u64, lid: u64, t: f64) -> Result<f64, String> {
    let c = e.project.comp(cid).ok_or("no composition")?;
    let l = c.layer(lid).ok_or("no layer")?;
    Ok(crate::eval::Eval::new(&e.project, cid, c, t).source_time(l))
}

pub fn freeze_frame(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = layers(e, p)?;
    let now = e.time_of(cid);
    for lid in &ids {
        let v = source_at(e, cid, *lid, now)?;
        remap_on(e, cid, *lid)?;
        let l = e.layer_mut(cid, *lid)?;
        let lt = l.layer_time(now);
        let pr = l.props.get_mut("timeRemap").ok_or("no time remap")?;
        let mut k = Key::new(lt, Value::Num(v), false);
        k.in_interp = Interp::Hold;
        k.out_interp = Interp::Hold;
        pr.keys = vec![k];
    }
    e.set_label("Freeze Frame");
    Ok(json!({"layers": ids}))
}

pub fn freeze_on_last_frame(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = layers(e, p)?;
    let (fd, dur) = e.project.comp(cid).map(|c| (c.frame_duration(), c.duration)).ok_or("no composition")?;
    for lid in &ids {
        let src = {
            let l = e.layer_ref(cid, *lid)?;
            source_duration(e, l).ok_or_else(|| format!("{} has no source duration to freeze", l.name))?
        };
        remap_on(e, cid, *lid)?;
        let l = e.layer_mut(cid, *lid)?;
        let last = (src - fd).max(0.0);
        let pr = l.props.get_mut("timeRemap").ok_or("no time remap")?;
        pr.keys.retain(|k| k.time < last - 1e-6);
        let mut k = Key::new(last, Value::Num(last), false);
        k.out_interp = Interp::Hold;
        insert(&mut pr.keys, k);
        l.out_point = l.out_point.max(dur);
    }
    e.set_label("Freeze On Last Frame");
    Ok(json!({"layers": ids}))
}

pub fn trim_to_work_area(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    let [a, b] = c.work_area;
    if b - a < c.frame_duration() * 0.5 {
        return Err("the work area is empty".into());
    }
    for l in c.layers.iter_mut() {
        l.start_time -= a;
        l.in_point -= a;
        l.out_point -= a;
    }
    c.markers.retain(|m| m.time + 1e-9 >= a && m.time <= b + 1e-9);
    for m in c.markers.iter_mut() {
        m.time -= a;
    }
    c.duration = b - a;
    c.work_area = [0.0, b - a];
    let d = c.duration;
    e.state.times.insert(cid, (e.time_of(cid) - a).clamp(0.0, d));
    e.set_label("Trim Comp to Work Area");
    Ok(json!({"duration": d}))
}

fn opacity_keys(l: &mut Layer, a: f64, b: f64, fade_in: bool) {
    let (ta, tb) = (l.layer_time(a), l.layer_time(b));
    let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut("opacity")) else { return };
    let full = pr.at(ta).num();
    let (va, vb) = if fade_in { (0.0, full) } else { (full, 0.0) };
    insert(&mut pr.keys, Key::new(ta, Value::Num(va), false));
    insert(&mut pr.keys, Key::new(tb, Value::Num(vb), false));
}

pub fn sequence(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = layers(e, p)?;
    let overlap = pb(p, "overlap").unwrap_or(false);
    let od = if overlap { pf(p, "duration").unwrap_or(1.0).max(0.0) } else { 0.0 };
    let tr = ps(p, "transition").unwrap_or("off").to_string();
    let mut prev_out: Option<f64> = None;
    let mut out = vec![];
    for (i, lid) in ids.iter().enumerate() {
        let l = e.layer_mut(cid, *lid)?;
        if let Some(po) = prev_out {
            let target = po - od;
            let d = target - l.in_point;
            l.start_time += d;
            l.in_point += d;
            l.out_point += d;
        }
        if od > 0.0 && tr != "off" {
            if i > 0 && tr == "crossDissolve" {
                opacity_keys(l, l.in_point, l.in_point + od, true);
            }
            if i + 1 < ids.len() {
                opacity_keys(l, l.out_point - od, l.out_point, false);
            }
        }
        prev_out = Some(l.out_point);
        out.push(json!({"layer": lid, "in": l.in_point, "out": l.out_point}));
    }
    e.set_label("Sequence Layers");
    Ok(json!({"layers": out}))
}

pub fn responsive_time(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let now = e.time_of(cid);
    let op = ps(p, "op").ok_or("pass `op` (intro, outro or workArea)")?.to_string();
    let c = e.project.comp(cid).ok_or("no composition")?;
    let (a, b) = match op.as_str() {
        "intro" => (0.0, now),
        "outro" => (now, c.duration),
        "workArea" => (c.work_area[0], c.work_area[1]),
        _ => return Err("`op` is intro, outro or workArea".into()),
    };
    if b - a < c.frame_duration() * 0.5 {
        return Err("move the current time first: the protected region would be empty".into());
    }
    let comment = match op.as_str() {
        "intro" => "Intro",
        "outro" => "Outro",
        _ => "Protected Region",
    };
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    c.markers.retain(|m| !(m.protected && same_time(m.time, a)));
    c.markers.push(Marker { time: a, duration: b - a, comment: comment.into(), protected: true, ..Marker::default() });
    c.markers.sort_by(|x, y| x.time.total_cmp(&y.time));
    e.set_label(format!("Create {comment}"));
    Ok(json!({"time": a, "duration": b - a}))
}
