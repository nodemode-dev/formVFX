use serde_json::{Value as Json, json};

use super::track::track_layer;
use crate::engine::jobs::{self, Apply};
use crate::engine::{Engine, Res, pb};
use crate::model::Value;
use crate::model::prop::{Group, GroupKind};
use crate::render::analysis::{Source, frame_times};
use crate::render::stabilize::{self, WarpData};

pub const EFFECT: &str = "fx.distort.warpStabilizer";
const MAX_SIDE: f64 = 960.0;

fn is_warp(g: &Group) -> bool {
    matches!(&g.kind, GroupKind::Effect { effect } if effect == EFFECT)
}

fn effect_uid(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<Option<u64>, String> {
    let l = e.layer_ref(cid, lid)?;
    let Some(fx) = l.effects() else { return Ok(None) };
    match p.get("effect").and_then(Json::as_u64) {
        Some(u) => fx.groups().find(|g| g.uid == u && is_warp(g)).map(|g| Some(g.uid)).ok_or_else(|| "no Wrap Stabilization with that uid on the layer".into()),
        None => Ok(fx.groups().filter(|g| is_warp(g)).last().map(|g| g.uid)),
    }
}

pub fn add(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let uid = match effect_uid(e, cid, lid, p)? {
        Some(u) => u,
        None => {
            let r = e.apply_step("Wrap Stabilization", |e| super::effect::apply(e, &json!({"effect": EFFECT, "layers": [lid], "comp": cid})))?;
            r.get("applied").and_then(|a| a.get(0)).and_then(|a| a.get("uid")).and_then(Json::as_u64).ok_or("could not add the Wrap Stabilization")?
        }
    };
    super::select_only(e, &[lid]);
    let mut q = p.clone();
    if let Some(o) = q.as_object_mut() {
        o.insert("layer".into(), json!(lid));
        o.insert("comp".into(), json!(cid));
        o.insert("effect".into(), json!(uid));
    }
    let r = analyze(e, &q)?;
    Ok(json!({"layer": lid, "effect": uid, "analysis": r}))
}

pub fn analyze(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let uid = effect_uid(e, cid, lid, p)?.ok_or("the layer has no Wrap Stabilization (Animation > Wrap Stabilization adds one)")?;
    let (times, layer_times, fps, detailed) = {
        let c = e.project.comp(cid).ok_or("no composition")?;
        let l = c.layer(lid).ok_or("no layer")?;
        let times = frame_times(&e.project, cid, lid, l.in_point, c.duration);
        if times.len() < 2 {
            return Err("the layer needs at least two frames inside the composition".into());
        }
        let lt: Vec<f64> = times.iter().map(|t| l.layer_time(*t)).collect();
        let span = (lt[lt.len() - 1] - lt[0]).abs().max(1e-9);
        let fps = (lt.len() - 1) as f64 / span;
        let g = l.props.find_group(uid).ok_or("no Wrap Stabilization")?;
        let detailed = g.get("detailed").map(|pr| pr.value.boolean()).unwrap_or(false);
        (times, lt, fps, detailed)
    };
    let wait = pb(p, "wait").unwrap_or(false);
    jobs::run(e, "warp", lid, "Analyze Wrap Stabilization", wait, move |ctx| {
        let src = Source::new(&ctx.project, &ctx.media, cid, lid, MAX_SIDE).ok_or("the layer is gone")?;
        let data = stabilize::analyze(ctx, &src, &times, &layer_times, fps, detailed)?;
        let encoded = data.encode();
        let frames = data.motion.len();
        let apply: Apply = Box::new(move |e: &mut Engine| {
            let l = e.layer_mut(cid, lid)?;
            let g = l.props.find_group_mut(uid).ok_or("the Wrap Stabilization is gone")?;
            let pr = g.get_mut("analysis").ok_or("the Wrap Stabilization has no analysis slot")?;
            pr.value = Value::Str(encoded);
            pr.keys.clear();
            Ok(json!({"effect": uid, "frames": frames}))
        });
        Ok(apply)
    })
}

pub fn cancel(e: &mut Engine, _: &Json) -> Res {
    Ok(json!({"stopped": e.jobs.stop(Some("warp"))}))
}

pub fn status(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let uid = effect_uid(e, cid, lid, p)?;
    let progress = e.jobs.progress(lid, "warp");
    let running = e.jobs.running_for(lid, "warp");
    let Some(uid) = uid else {
        return Ok(json!({"layer": lid, "effect": null, "analyzed": false, "running": running, "progress": progress}));
    };
    let l = e.layer_ref(cid, lid)?;
    let g = l.props.find_group(uid).ok_or("no Wrap Stabilization")?;
    let get = |id: &str| g.get(id).map(|pr| pr.value.clone());
    let s = match get("analysis") {
        Some(Value::Str(s)) => s,
        _ => String::new(),
    };
    let d = stabilize::parsed(&s);
    let plan = d.as_ref().map(|d| {
        let num = |id: &str| get(id).map(|v| v.num()).unwrap_or(0.0);
        let idx = |id: &str| get(id).map(|v| v.index()).unwrap_or(0);
        stabilize::plan(d, idx("result"), num("smoothness"), idx("method"), get("preserveScale").is_some_and(|v| v.boolean()))
    });
    let frames = d.as_ref().map(|d: &std::sync::Arc<WarpData>| d.motion.len()).unwrap_or(0);
    Ok(json!({
        "layer": lid, "effect": uid, "analyzed": frames > 0, "frames": frames,
        "autoScale": plan.map(|p| (p.scale * 1000.0).round() / 10.0),
        "running": running, "progress": progress,
    }))
}
