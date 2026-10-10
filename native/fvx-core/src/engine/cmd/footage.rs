use std::collections::HashMap;
use std::path::Path;

use serde_json::{Value as Json, json};

use super::{add_layer, footage_layer, select_only};
use crate::engine::jobs::{self, Apply};
use crate::engine::{Engine, Res, pb, pf, pu};
use crate::model::{Footage, FootageKind, ItemKind};

#[derive(Default)]
pub struct View {
    pub item: Option<u64>,
    pub time: f64,
    pub marks: HashMap<u64, (Option<f64>, Option<f64>)>,
}

fn footage_of(e: &Engine, id: u64) -> Result<&Footage, String> {
    match e.project.item(id).map(|i| &i.kind) {
        Some(ItemKind::Footage(f)) => Ok(f),
        Some(_) => Err("only footage opens in the Footage panel".into()),
        None => Err(format!("no item {id}")),
    }
}

fn length(f: &Footage) -> f64 {
    if f.is_still() || f.duration <= 0.0 { 10.0 } else { f.duration * f.loops.max(1) as f64 }
}

fn fps(f: &Footage) -> f64 {
    if f.frame_rate > 0.0 { f.frame_rate } else { 30.0 }
}

fn current(e: &Engine) -> Result<u64, String> {
    let id = e.session.footage.item.filter(|i| e.project.item(*i).is_some()).ok_or("open footage in the Footage panel first (footage.open)")?;
    footage_of(e, id)?;
    Ok(id)
}

fn time_param(e: &Engine, p: &Json, f: &Footage) -> f64 {
    let t = pf(p, "time").or_else(|| pu(p, "frame").map(|n| n as f64 / fps(f))).unwrap_or(e.session.footage.time);
    t.clamp(0.0, length(f))
}

fn marks(e: &Engine, id: u64) -> (f64, f64) {
    let f = footage_of(e, id).ok();
    let len = f.map(length).unwrap_or(0.0);
    let (a, b) = e.session.footage.marks.get(&id).copied().unwrap_or((None, None));
    (a.unwrap_or(0.0), b.unwrap_or(len))
}

pub fn info(e: &mut Engine, _: &Json) -> Res {
    let Ok(id) = current(e) else { return Ok(Json::Null) };
    let f = footage_of(e, id)?;
    let (a, b) = e.session.footage.marks.get(&id).copied().unwrap_or((None, None));
    let (i, o) = marks(e, id);
    Ok(json!({
        "item": id,
        "name": e.project.item(id).map(|x| x.name.clone()),
        "time": e.session.footage.time,
        "frameRate": fps(f),
        "duration": length(f),
        "still": f.is_still(),
        "width": f.width,
        "height": f.height,
        "hasVideo": f.has_video,
        "hasAudio": f.has_audio,
        "in": a,
        "out": b,
        "range": [i, o],
    }))
}

pub fn open(e: &mut Engine, p: &Json) -> Res {
    let id = p.get("item").and_then(|v| super::project::item_ref(e, v)).or_else(|| e.state.project_selection.first().copied()).ok_or("pass `item`")?;
    footage_of(e, id)?;
    let start = e.session.footage.marks.get(&id).and_then(|m| m.0).unwrap_or(0.0);
    e.session.footage.item = Some(id);
    e.session.footage.time = start;
    e.state.project_selection = vec![id];
    e.frontend("window.panel", json!({"panel": "footage"}));
    info(e, &Json::Null)
}

pub fn set_time(e: &mut Engine, p: &Json) -> Res {
    let id = current(e)?;
    let t = time_param(e, p, footage_of(e, id)?);
    e.session.footage.time = t;
    Ok(json!({"time": t}))
}

fn mark(e: &mut Engine, p: &Json, out: bool) -> Res {
    let id = current(e)?;
    let f = footage_of(e, id)?;
    let fd = 1.0 / fps(f);
    let t = time_param(e, p, f);
    let (i, o) = marks(e, id);
    let m = e.session.footage.marks.entry(id).or_insert((None, None));
    if out {
        m.1 = Some(t.max(i + fd));
    } else {
        m.0 = Some(t.min(o - fd).max(0.0));
    }
    let (i, o) = marks(e, id);
    Ok(json!({"item": id, "in": i, "out": o, "duration": o - i}))
}

pub fn set_in(e: &mut Engine, p: &Json) -> Res {
    mark(e, p, false)
}

pub fn set_out(e: &mut Engine, p: &Json) -> Res {
    mark(e, p, true)
}

pub fn clear_in_out(e: &mut Engine, _: &Json) -> Res {
    let id = current(e)?;
    e.session.footage.marks.remove(&id);
    let (i, o) = marks(e, id);
    Ok(json!({"item": id, "in": i, "out": o}))
}

fn edit(e: &mut Engine, p: &Json, ripple: bool) -> Res {
    let id = match p.get("item") {
        Some(v) => super::project::item_ref(e, v).ok_or("no such item")?,
        None => current(e)?,
    };
    let f = footage_of(e, id)?.clone();
    let (mi, mo) = marks(e, id);
    let a = pf(p, "in").unwrap_or(mi).clamp(0.0, length(&f));
    let b = pf(p, "out").unwrap_or(mo).clamp(0.0, length(&f));
    if b <= a {
        return Err("the out point must come after the in point".into());
    }
    let cid = e.comp_id(p.get("comp"))?;
    let at = pf(p, "at").unwrap_or_else(|| e.time_of(cid)).max(0.0);
    let d = b - a;
    let mut moved = 0;
    let mut split = 0;
    if ripple {
        let ids: Vec<u64> = e.project.comp(cid).ok_or("no composition")?.layers.iter().map(|l| l.id).collect();
        for lid in ids {
            let l = e.layer_ref(cid, lid)?.clone();
            if l.in_point < at - 1e-9 && l.out_point > at + 1e-9 {
                let mut tail = l.clone();
                let pj = e.proj();
                tail.id = pj.new_id();
                tail.props.renumber(&mut pj.next_uid);
                tail.in_point = at + d;
                tail.out_point = l.out_point + d;
                tail.start_time = l.start_time + d;
                let c = pj.comp_mut(cid).ok_or("no composition")?;
                let i = c.index_of(lid).unwrap_or(0);
                c.layers[i].out_point = at;
                c.layers.insert(i, tail);
                split += 1;
            } else if l.in_point >= at - 1e-9 {
                let m = e.layer_mut(cid, lid)?;
                m.in_point += d;
                m.out_point += d;
                m.start_time += d;
                moved += 1;
            }
        }
    }
    let mut l = footage_layer(e, cid, id, at, None, Some(d))?;
    l.start_time = at - a;
    l.in_point = at;
    l.out_point = at + d;
    let lid = add_layer(e, cid, l, Some(0));
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    let needed = c.layers.iter().map(|l| l.out_point).fold(0.0, f64::max);
    let grew = ripple && needed > c.duration + 1e-9;
    if grew {
        c.duration = needed;
    }
    select_only(e, &[lid]);
    e.set_label(if ripple { "Ripple Insert Edit" } else { "Overlay Edit" });
    Ok(json!({"layer": lid, "comp": cid, "at": at, "in": a, "out": b, "duration": d, "shifted": moved, "split": split, "compDuration": if grew { json!(needed) } else { Json::Null }}))
}

pub fn overlay_edit(e: &mut Engine, p: &Json) -> Res {
    edit(e, p, false)
}

pub fn ripple_insert_edit(e: &mut Engine, p: &Json) -> Res {
    edit(e, p, true)
}

pub fn check(e: &mut Engine, p: &Json) -> Res {
    let wanted: Vec<u64> = p.get("items").and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_u64).collect()).unwrap_or_default();
    let list: Vec<(u64, Vec<String>)> = e
        .project
        .items
        .iter()
        .filter(|i| wanted.is_empty() || wanted.contains(&i.id))
        .filter_map(|i| match &i.kind {
            ItemKind::Footage(f) if !f.path.is_empty() => Some((i.id, if f.kind == FootageKind::Sequence && !f.sequence.is_empty() { f.sequence.clone() } else { vec![f.path.clone()] })),
            _ => None,
        })
        .collect();
    let wait = pb(p, "wait").unwrap_or(false);
    jobs::run(e, "footageCheck", 0, "Check Footage", wait, move |ctx| {
        let mut missing: Vec<(u64, bool, usize)> = vec![];
        for (n, (id, files)) in list.iter().enumerate() {
            if ctx.stopped() {
                return Err("stopped".into());
            }
            ctx.step(n, list.len(), "checking footage");
            let gone = files.iter().filter(|f| !Path::new(f).exists()).count();
            missing.push((*id, gone == files.len(), gone));
        }
        let apply: Apply = Box::new(move |e: &mut Engine| {
            let mut out = vec![];
            for (id, gone, frames) in &missing {
                let was = matches!(e.project.item(*id).map(|i| &i.kind), Some(ItemKind::Footage(f)) if f.missing);
                if was != *gone
                    && let Some(ItemKind::Footage(f)) = e.proj().item_mut(*id).map(|i| &mut i.kind)
                {
                    f.missing = *gone;
                }
                if *gone || *frames > 0 {
                    out.push(json!({"item": id, "missing": gone, "missingFiles": frames}));
                }
            }
            Ok(json!({"checked": missing.len(), "missing": out}))
        });
        Ok(apply)
    })
}
