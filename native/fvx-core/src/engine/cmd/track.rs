use serde_json::{Value as Json, json};

use crate::engine::jobs::{self, Apply, Ctx};
use crate::engine::{Engine, Res, pb, pf, ps};
use crate::eval::Eval;
use crate::geom::Mat3;
use crate::model::key::{self, Key};
use crate::model::path::BezPath;
use crate::model::prop::{Group, GroupKind, Node, Prop, Ui};
use crate::model::{Layer, Value};
use crate::render::analysis::{Source, frame_times};
use crate::render::comp::fx_frame;
use crate::render::face;
use crate::render::vision::{self, H3, Model};

pub const KINDS: [&str; 5] = ["transform", "stabilize", "affine", "perspective", "raw"];
pub const METHODS: [&str; 7] = ["position", "positionScale", "positionScaleRotation", "positionScaleRotationSkew", "perspective", "faceOutline", "faceDetailed"];
const MAX_SIDE: f64 = 1920.0;

#[derive(Clone, Debug)]
pub struct Opts {
    pub channel: u32,
    pub blur: f64,
    pub subpixel: bool,
    pub adapt_every: bool,
    pub threshold: f64,
    pub action: String,
}

pub fn default_options() -> Json {
    json!({"channel": "luminance", "blur": 0, "enhance": false, "subpixel": true, "adaptEveryFrame": false, "threshold": 80, "action": "adapt", "trackShape": false})
}

fn opts_of(j: &Json) -> Opts {
    Opts {
        channel: match j.get("channel").and_then(Json::as_str) {
            Some("rgb") => 0,
            Some("saturation") => 2,
            _ => 1,
        },
        blur: j.get("blur").and_then(Json::as_f64).unwrap_or(0.0),
        subpixel: j.get("subpixel").and_then(Json::as_bool).unwrap_or(true),
        adapt_every: j.get("adaptEveryFrame").and_then(Json::as_bool).unwrap_or(false),
        threshold: j.get("threshold").and_then(Json::as_f64).unwrap_or(80.0),
        action: j.get("action").and_then(Json::as_str).unwrap_or("adapt").to_string(),
    }
}

fn hidden(ids: &mut crate::model::Ids, id: &str, name: &str, v: Value) -> Node {
    Node::Prop(ids.prop(id, name, v, Ui::Hidden))
}

fn point_group(ids: &mut crate::model::Ids, i: usize, c: [f64; 2], fs: f64, ss: f64) -> Group {
    let children = vec![
        Node::Prop(ids.prop("featureCenter", "Feature Center", Value::Vec2(c), Ui::Point).spatial()),
        Node::Prop(ids.prop("featureSize", "Feature Size", Value::Vec2([fs, fs]), Ui::Plain)),
        Node::Prop(ids.prop("searchOffset", "Search Offset", Value::Vec2([0.0, 0.0]), Ui::Plain)),
        Node::Prop(ids.prop("searchSize", "Search Size", Value::Vec2([ss, ss]), Ui::Plain)),
        Node::Prop(ids.prop("confidence", "Confidence", Value::Num(0.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 1))),
        Node::Prop(ids.prop("attachPoint", "Attach Point", Value::Vec2(c), Ui::Point).spatial()),
        Node::Prop(ids.prop("attachPointOffset", "Attach Point Offset", Value::Vec2([0.0, 0.0]), Ui::Plain)),
    ];
    ids.group(&format!("trackPoint{i}"), &format!("Track Point {i}"), GroupKind::Plain, children)
}

fn point_count(kind: &str, rot: bool, scale: bool) -> usize {
    match kind {
        "affine" => 3,
        "perspective" => 4,
        "raw" => 1,
        _ => {
            if rot || scale {
                2
            } else {
                1
            }
        }
    }
}

fn layout(n: usize, f: [f64; 4]) -> Vec<[f64; 2]> {
    let c = [(f[0] + f[2]) * 0.5, (f[1] + f[3]) * 0.5];
    let d = (f[2] - f[0]).min(f[3] - f[1]) * 0.18;
    match n {
        1 => vec![c],
        2 => vec![[c[0] - d, c[1]], [c[0] + d, c[1]]],
        3 => vec![[c[0] - d, c[1] - d], [c[0] + d, c[1] - d], [c[0] - d, c[1] + d]],
        _ => vec![[c[0] - d, c[1] - d], [c[0] + d, c[1] - d], [c[0] - d, c[1] + d], [c[0] + d, c[1] + d]],
    }
}

fn str_of(g: &Group, id: &str) -> String {
    match g.get(id).map(|p| &p.value) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

fn bool_of(g: &Group, id: &str) -> bool {
    g.get(id).map(|p| p.value.boolean()).unwrap_or(false)
}

fn frame_of(e: &Engine, cid: u64, lid: u64) -> Result<[f64; 4], String> {
    let c = e.project.comp(cid).ok_or("no composition")?;
    let l = c.layer(lid).ok_or("no layer")?;
    let (sw, sh) = e.project.source_size(l, c);
    Ok(fx_frame(l, sw, sh))
}

fn trackable(l: &Layer) -> bool {
    l.source.is_some() && !l.is_model()
}

pub fn add_tracker(e: &mut Engine, cid: u64, lid: u64, kind: &str, pos: bool, rot: bool, scale: bool, target: Option<u64>) -> Result<u64, String> {
    let f = frame_of(e, cid, lid)?;
    let l = e.layer_ref(cid, lid)?;
    if !trackable(l) {
        return Err("track a footage or composition layer".into());
    }
    let n_existing = l.props.sub("motionTrackers").map(|g| g.groups().count()).unwrap_or(0);
    let side = (f[2] - f[0]).min(f[3] - f[1]);
    let fs = (side * 0.05).clamp(12.0, 80.0).round();
    let ss = (fs * 2.5).round();
    let n = point_count(kind, rot, scale);
    let (tracker, container) = {
        let pj = e.proj();
        let mut ids = pj.ids();
        let mut children = vec![
            hidden(&mut ids, "kind", "Track Type", Value::Str(kind.to_string())),
            hidden(&mut ids, "trackPosition", "Position", Value::Bool(pos)),
            hidden(&mut ids, "trackRotation", "Rotation", Value::Bool(rot)),
            hidden(&mut ids, "trackScale", "Scale", Value::Bool(scale)),
            hidden(&mut ids, "target", "Motion Target", Value::Layer(target)),
            hidden(&mut ids, "options", "Options", Value::Str(default_options().to_string())),
        ];
        for (i, c) in layout(n, f).into_iter().enumerate() {
            children.push(Node::Group(point_group(&mut ids, i + 1, c, fs, ss)));
        }
        let t = ids.group("tracker", &format!("Tracker {}", n_existing + 1), GroupKind::Plain, children);
        let container = ids.group("motionTrackers", "Motion Trackers", GroupKind::Indexed, vec![]);
        (t, container)
    };
    let uid = tracker.uid;
    let l = e.layer_mut(cid, lid)?;
    if l.props.sub("motionTrackers").is_none() {
        l.props.children.insert(0, Node::Group(container));
    }
    l.props.sub_mut("motionTrackers").ok_or("no trackers")?.children.push(Node::Group(tracker));
    e.state.current_tracker.insert(lid, uid);
    Ok(uid)
}

fn tracker_uid(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<u64, String> {
    let l = e.layer_ref(cid, lid)?;
    let g = l.props.sub("motionTrackers").ok_or("this layer has no trackers (Track Motion makes one)")?;
    let all: Vec<&Group> = g.groups().collect();
    if all.is_empty() {
        return Err("this layer has no trackers".into());
    }
    match p.get("tracker") {
        Some(Json::Number(n)) => {
            let n = n.as_u64().unwrap_or(0);
            if all.iter().any(|t| t.uid == n) {
                Ok(n)
            } else {
                all.get((n as usize).wrapping_sub(1)).map(|t| t.uid).ok_or_else(|| "no such tracker".into())
            }
        }
        Some(Json::String(s)) => {
            if let Some(i) = s.strip_prefix('#').and_then(|x| x.parse::<usize>().ok()) {
                return all.get(i.wrapping_sub(1)).map(|t| t.uid).ok_or_else(|| "no such tracker".into());
            }
            all.iter().find(|t| t.name == *s).map(|t| t.uid).ok_or_else(|| format!("no tracker \"{s}\""))
        }
        _ => Ok(e.state.current_tracker.get(&lid).copied().filter(|u| all.iter().any(|t| t.uid == *u)).unwrap_or(all[all.len() - 1].uid)),
    }
}

pub fn track_layer(e: &Engine, p: &Json) -> Result<(u64, u64), String> {
    let (cid, comp) = e.comp_of(p)?;
    if let Some(v) = p.get("layer").filter(|v| !v.is_null()) {
        return Ok((cid, e.layer_in(comp, v)?));
    }
    let sel = e.state.selected_layers.iter().copied().find(|id| comp.layer(*id).is_some_and(trackable));
    sel.map(|id| (cid, id)).ok_or_else(|| "select a footage or composition layer to track".into())
}

fn new_kind(e: &mut Engine, p: &Json, kind: &str) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let rot = pb(p, "rotation").unwrap_or(false);
    let scale = pb(p, "scale").unwrap_or(false);
    let named = match p.get("target").filter(|v| !v.is_null()) {
        Some(v) => Some(e.layer_in(e.project.comp(cid).ok_or("no comp")?, v)?),
        None => None,
    };
    let target = match named {
        Some(t) => Some(t),
        None if kind == "stabilize" => Some(lid),
        None => e.project.comp(cid).and_then(|c| {
            let i = c.index_of(lid)?;
            i.checked_sub(1).and_then(|j| c.layers.get(j)).map(|l| l.id)
        }),
    };
    let uid = add_tracker(e, cid, lid, kind, pb(p, "position").unwrap_or(true), rot, scale, target)?;
    super::select_only(e, &[lid]);
    e.frontend("window.panel", json!({"panel": "tracker"}));
    e.set_label(if kind == "stabilize" { "Stabilize Motion" } else { "Track Motion" });
    Ok(json!({"layer": lid, "tracker": uid}))
}

pub fn motion(e: &mut Engine, p: &Json) -> Res {
    new_kind(e, p, "transform")
}

pub fn stabilize(e: &mut Engine, p: &Json) -> Res {
    new_kind(e, p, "stabilize")
}

pub fn new_tracker(e: &mut Engine, p: &Json) -> Res {
    let kind = ps(p, "kind").unwrap_or("transform");
    if !KINDS.contains(&kind) {
        return Err(format!("kind is one of {}", KINDS.join(", ")));
    }
    new_kind(e, p, kind)
}

pub fn track_property(e: &mut Engine, p: &Json) -> Res {
    let (cid, comp) = e.comp_of(p)?;
    let target = match p.get("layer") {
        Some(v) => e.layer_in(comp, v)?,
        None => *e.state.selected_layers.first().ok_or("select the layer the motion should drive")?,
    };
    let source = match p.get("source") {
        Some(v) => e.layer_in(comp, v)?,
        None => comp.layers.iter().find(|l| trackable(l) && l.id != target).map(|l| l.id).ok_or("pass `source`: the footage layer to track")?,
    };
    let uid = add_tracker(e, cid, source, "transform", true, false, false, Some(target))?;
    e.set_label("Track this Property");
    Ok(json!({"layer": source, "tracker": uid, "target": target}))
}

pub fn select(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let uid = tracker_uid(e, cid, lid, p)?;
    e.state.current_tracker.insert(lid, uid);
    e.revision += 1;
    Ok(json!({"tracker": uid}))
}

fn with_tracker(e: &mut Engine, p: &Json, f: impl FnOnce(&mut Group, [f64; 4]) -> Result<(), String>) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let uid = tracker_uid(e, cid, lid, p)?;
    let fr = frame_of(e, cid, lid)?;
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no tracker")?;
    f(g, fr)?;
    Ok(json!({"layer": lid, "tracker": uid}))
}

fn set_v(g: &mut Group, id: &str, v: Value) {
    if let Some(p) = g.get_mut(id) {
        p.value = v;
    }
}

pub fn set_type(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let uid = tracker_uid(e, cid, lid, p)?;
    let fr = frame_of(e, cid, lid)?;
    let (kind, pos, rot, scale, existing) = {
        let l = e.layer_ref(cid, lid)?;
        let g = l.props.find_group(uid).ok_or("no tracker")?;
        let kind = ps(p, "kind").map(str::to_string).unwrap_or_else(|| str_of(g, "kind"));
        if !KINDS.contains(&kind.as_str()) {
            return Err(format!("kind is one of {}", KINDS.join(", ")));
        }
        (kind, pb(p, "position").unwrap_or_else(|| bool_of(g, "trackPosition")), pb(p, "rotation").unwrap_or_else(|| bool_of(g, "trackRotation")), pb(p, "scale").unwrap_or_else(|| bool_of(g, "trackScale")), g.groups().count())
    };
    let n = point_count(&kind, rot, scale);
    let extra: Vec<Group> = if n > existing {
        let side = (fr[2] - fr[0]).min(fr[3] - fr[1]);
        let fs = (side * 0.05).clamp(12.0, 80.0).round();
        let pts = layout(n, fr);
        let pj = e.proj();
        let mut ids = pj.ids();
        (existing..n).map(|i| point_group(&mut ids, i + 1, pts[i], fs, (fs * 2.5).round())).collect()
    } else {
        vec![]
    };
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no tracker")?;
    set_v(g, "kind", Value::Str(kind.clone()));
    set_v(g, "trackPosition", Value::Bool(pos));
    set_v(g, "trackRotation", Value::Bool(rot));
    set_v(g, "trackScale", Value::Bool(scale));
    if n < existing {
        let mut seen = 0;
        g.children.retain(|c| match c {
            Node::Group(_) => {
                seen += 1;
                seen <= n
            }
            _ => true,
        });
    }
    for x in extra {
        g.children.push(Node::Group(x));
    }
    if kind == "stabilize" && g.get("target").is_some_and(|t| t.value.layer().is_none()) {
        set_v(g, "target", Value::Layer(Some(lid)));
    }
    e.set_label("Track Type");
    Ok(json!({"tracker": uid, "kind": kind, "points": n}))
}

pub fn set_target(e: &mut Engine, p: &Json) -> Res {
    let target = match p.get("target") {
        None | Some(Json::Null) => None,
        Some(v) => {
            let (_, comp) = e.comp_of(p)?;
            Some(e.layer_in(comp, v)?)
        }
    };
    let r = with_tracker(e, p, |g, _| {
        set_v(g, "target", Value::Layer(target));
        Ok(())
    })?;
    e.set_label("Edit Target");
    Ok(r)
}

pub fn options(e: &mut Engine, p: &Json) -> Res {
    let r = with_tracker(e, p, |g, _| {
        let cur = str_of(g, "options");
        let mut o: Json = serde_json::from_str(&cur).unwrap_or_else(|_| default_options());
        for k in ["channel", "blur", "enhance", "subpixel", "adaptEveryFrame", "threshold", "action", "trackShape"] {
            if let Some(v) = p.get(k) {
                o[k] = v.clone();
            }
        }
        set_v(g, "options", Value::Str(o.to_string()));
        Ok(())
    })?;
    if let Some(n) = ps(p, "name") {
        let (cid, lid) = track_layer(e, p)?;
        let uid = tracker_uid(e, cid, lid, p)?;
        if let Some(g) = e.layer_mut(cid, lid)?.props.find_group_mut(uid) {
            g.name = n.to_string();
        }
    }
    e.set_label("Motion Tracker Options");
    Ok(r)
}

fn v2(p: &Json, k: &str) -> Option<[f64; 2]> {
    let a = p.get(k)?.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
}

fn put(pr: &mut Prop, lt: Option<f64>, v: Value) {
    match lt {
        Some(t) if !pr.keys.is_empty() => {
            key::insert(&mut pr.keys, Key::new(t, v, pr.spatial));
        }
        _ => pr.value = v,
    }
}

pub fn set_point(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let now = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let lt = e.layer_ref(cid, lid)?.layer_time(now);
    let idx = p.get("point").and_then(Json::as_u64).ok_or("pass `point` (1-based)")? as usize;
    let ev_vals = {
        let c = e.project.comp(cid).ok_or("no composition")?;
        let l = c.layer(lid).ok_or("no layer")?;
        let uid = tracker_uid(e, cid, lid, p)?;
        let g = l.props.find_group(uid).ok_or("no tracker")?;
        let pg = g.groups().nth(idx.wrapping_sub(1)).ok_or("no such track point")?;
        let ev = Eval::new(&e.project, cid, c, now);
        let get = |id: &str| pg.get(id).map(|pr| ev.value(l, pr).vec2()).unwrap_or([0.0; 2]);
        (uid, get("featureCenter"), get("attachPointOffset"))
    };
    let (uid, cur_c, cur_ao) = ev_vals;
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no tracker")?;
    let pg = g.groups_mut().nth(idx - 1).ok_or("no such track point")?;
    let mut c = v2(p, "center").unwrap_or(cur_c);
    if let Some(m) = v2(p, "move") {
        c = [c[0] + m[0], c[1] + m[1]];
    }
    let ao = v2(p, "attachOffset").unwrap_or(cur_ao);
    if let Some(pr) = pg.get_mut("featureCenter") {
        put(pr, Some(lt), Value::Vec2(c));
    }
    for id in ["featureSize", "searchSize"] {
        if let Some(v) = v2(p, id)
            && let Some(pr) = pg.get_mut(id)
        {
            pr.value = Value::Vec2([v[0].abs().max(2.0), v[1].abs().max(2.0)]);
        }
    }
    if let Some(v) = v2(p, "searchOffset")
        && let Some(pr) = pg.get_mut("searchOffset")
    {
        pr.value = Value::Vec2(v);
    }
    if let Some(pr) = pg.get_mut("attachPointOffset") {
        pr.value = Value::Vec2(ao);
    }
    if let Some(pr) = pg.get_mut("attachPoint") {
        put(pr, Some(lt), Value::Vec2([c[0] + ao[0], c[1] + ao[1]]));
    }
    if let Some(m) = ps(p, "merge") {
        e.merge_hint = Some(m.to_string());
    }
    e.set_label("Move Track Point");
    Ok(json!({"tracker": uid, "point": idx}))
}

pub fn reset(e: &mut Engine, p: &Json) -> Res {
    let r = with_tracker(e, p, |g, _| {
        for pg in g.groups_mut() {
            for id in ["featureCenter", "confidence", "attachPoint"] {
                if let Some(pr) = pg.get_mut(id) {
                    if let Some(k) = pr.keys.first() {
                        pr.value = k.value.clone();
                    }
                    pr.keys.clear();
                }
            }
        }
        Ok(())
    })?;
    e.set_label("Reset Tracker");
    Ok(r)
}

pub fn delete(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let uid = tracker_uid(e, cid, lid, p)?;
    let l = e.layer_mut(cid, lid)?;
    if let Some(g) = l.props.sub_mut("motionTrackers") {
        g.children.retain(|c| c.uid() != uid);
        if g.children.is_empty() {
            l.props.children.retain(|c| !matches!(c, Node::Group(g) if g.id == "motionTrackers"));
        }
    }
    e.state.current_tracker.remove(&lid);
    e.set_label("Delete Tracker");
    Ok(json!({"deleted": uid}))
}

fn point_json(ev: &Eval, l: &Layer, pg: &Group) -> Json {
    let get = |id: &str| pg.get(id).map(|pr| ev.value(l, pr).vec2()).unwrap_or([0.0; 2]);
    json!({
        "name": pg.name, "keys": pg.get("featureCenter").map(|pr| pr.keys.len()).unwrap_or(0),
        "center": get("featureCenter"), "featureSize": get("featureSize"), "searchOffset": get("searchOffset"), "searchSize": get("searchSize"), "attachOffset": get("attachPointOffset"),
        "confidence": pg.get("confidence").map(|pr| ev.value(l, pr).num()).unwrap_or(0.0),
    })
}

pub fn status(e: &mut Engine, p: &Json) -> Res {
    let (cid, comp) = e.comp_of(p)?;
    let lid = match p.get("layer").filter(|v| !v.is_null()) {
        Some(v) => Some(e.layer_in(comp, v)?),
        None => e.state.selected_layers.iter().copied().find(|id| comp.layer(*id).is_some()),
    };
    let method = if e.state.mask_method.is_empty() { "position".to_string() } else { e.state.mask_method.clone() };
    let Some(lid) = lid else {
        return Ok(json!({"layer": null, "mask": null, "maskMethod": method, "running": e.jobs.running(), "progress": null, "tracker": null, "trackers": []}));
    };
    let l = comp.layer(lid).ok_or("no layer")?;
    let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
    let trackers: Vec<Json> = l.props.sub("motionTrackers").map(|g| g.groups().map(|t| json!({"uid": t.uid, "name": t.name})).collect()).unwrap_or_default();
    let tracker = tracker_uid(e, cid, lid, p).ok().and_then(|u| l.props.find_group(u)).map(|g| {
        let opts: Json = serde_json::from_str(&str_of(g, "options")).unwrap_or_else(|_| default_options());
        json!({
            "uid": g.uid, "name": g.name,
            "points": g.groups().map(|pg| point_json(&ev, l, pg)).collect::<Vec<_>>(),
            "settings": {"kind": str_of(g, "kind"), "position": bool_of(g, "trackPosition"), "rotation": bool_of(g, "trackRotation"), "scale": bool_of(g, "trackScale"), "target": g.get("target").and_then(|t| t.value.layer()), "options": opts},
        })
    });
    let kinds = ["track", "mask", "warp", "camera"];
    let running = kinds.iter().any(|k| e.jobs.running_for(lid, k));
    let progress = kinds.iter().map(|k| e.jobs.progress(lid, k)).find(|v| !v.is_null()).unwrap_or(Json::Null);
    let mask = if e.jobs.running_for(lid, "mask") { json!({"done": progress["done"], "total": progress["total"]}) } else { Json::Null };
    Ok(json!({"layer": lid, "mask": mask, "maskMethod": method, "running": running, "progress": progress, "tracker": tracker, "trackers": trackers}))
}

pub fn stop(e: &mut Engine, p: &Json) -> Res {
    let n = e.jobs.stop(ps(p, "kind"));
    Ok(json!({"stopped": n}))
}

fn span(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<(Vec<f64>, bool), String> {
    let c = e.project.comp(cid).ok_or("no composition")?;
    let fd = c.frame_duration();
    let now = c.snap(e.time_of(cid));
    let dir = ps(p, "direction").unwrap_or("forward");
    let (a, b) = (pf(p, "start"), pf(p, "end"));
    let (times, back) = match dir {
        "backward" => (frame_times(&e.project, cid, lid, a.unwrap_or(0.0), b.unwrap_or(now)), true),
        "frameForward" => (frame_times(&e.project, cid, lid, now, now + fd * 1.5), false),
        "frameBackward" => (frame_times(&e.project, cid, lid, now - fd * 1.5, now), true),
        _ => (frame_times(&e.project, cid, lid, a.unwrap_or(now), b.unwrap_or(c.duration)), false),
    };
    let mut times = times;
    if back {
        times.reverse();
    }
    if times.len() < 2 {
        return Err("nothing to analyze in that direction (move the current time inside the layer)".into());
    }
    Ok((times, back))
}

struct PtIn {
    c: [f64; 2],
    fs: [f64; 2],
    so: [f64; 2],
    ss: [f64; 2],
    ao: [f64; 2],
}

pub fn analyze(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let uid = tracker_uid(e, cid, lid, p)?;
    let (times, _) = span(e, cid, lid, p)?;
    let (pts, opts) = {
        let c = e.project.comp(cid).ok_or("no composition")?;
        let l = c.layer(lid).ok_or("no layer")?;
        let g = l.props.find_group(uid).ok_or("no tracker")?;
        let ev = Eval::new(&e.project, cid, c, times[0]);
        let pts: Vec<PtIn> = g
            .groups()
            .map(|pg| {
                let get = |id: &str| pg.get(id).map(|pr| ev.value(l, pr).vec2()).unwrap_or([0.0; 2]);
                PtIn { c: get("featureCenter"), fs: get("featureSize"), so: get("searchOffset"), ss: get("searchSize"), ao: get("attachPointOffset") }
            })
            .collect();
        (pts, opts_of(&serde_json::from_str(&str_of(g, "options")).unwrap_or_else(|_| default_options())))
    };
    let wait = pb(p, "wait").unwrap_or(false);
    let layer_times: Vec<f64> = {
        let l = e.layer_ref(cid, lid)?;
        times.iter().map(|t| l.layer_time(*t)).collect()
    };
    jobs::run(e, "track", lid, "Track Motion", wait, move |ctx| {
        let tracks = track_points(ctx, cid, lid, &times, &pts, &opts)?;
        let aos: Vec<[f64; 2]> = pts.iter().map(|q| q.ao).collect();
        let apply: Apply = Box::new(move |e: &mut Engine| {
            let l = e.layer_mut(cid, lid)?;
            let g = l.props.find_group_mut(uid).ok_or("the tracker is gone")?;
            for (pi, pg) in g.groups_mut().enumerate() {
                let Some(tr) = tracks.get(pi) else { continue };
                for (fi, (c, conf)) in tr.iter().enumerate() {
                    let Some(c) = c else { continue };
                    let lt = layer_times[fi];
                    for (id, v) in [("featureCenter", Value::Vec2(*c)), ("confidence", Value::Num(*conf)), ("attachPoint", Value::Vec2([c[0] + aos[pi][0], c[1] + aos[pi][1]]))] {
                        if let Some(pr) = pg.get_mut(id) {
                            key::insert(&mut pr.keys, Key::new(lt, v, pr.spatial));
                        }
                    }
                }
            }
            Ok(json!({"tracker": uid, "frames": layer_times.len()}))
        });
        Ok(apply)
    })
}

type Track = Vec<(Option<[f64; 2]>, f64)>;

fn track_points(ctx: &Ctx, cid: u64, lid: u64, times: &[f64], pts: &[PtIn], o: &Opts) -> Result<Vec<Track>, String> {
    let src = Source::new(&ctx.project, &ctx.media, cid, lid, MAX_SIDE).ok_or("the layer is gone")?;
    let k = src.scale as f32;
    let g0 = src.gray(times[0], o.channel, o.blur as f32).ok_or("cannot read the layer's frames")?;
    let half = |v: [f64; 2]| ((v[0] * k as f64 * 0.5).round().max(2.0) as usize, (v[1] * k as f64 * 0.5).round().max(2.0) as usize);
    let mut tpl: Vec<Vec<f32>> = pts.iter().map(|q| {
        let (hw, hh) = half(q.fs);
        vision::patch(&g0, src.to_buf(q.c), hw, hh)
    }).collect();
    let mut cur: Vec<[f32; 2]> = pts.iter().map(|q| src.to_buf(q.c)).collect();
    let mut vel: Vec<[f32; 2]> = vec![[0.0; 2]; pts.len()];
    let mut alive = vec![true; pts.len()];
    let mut out: Vec<Track> = pts.iter().map(|q| vec![(Some(q.c), 100.0)]).collect();
    for (fi, t) in times.iter().enumerate().skip(1) {
        if ctx.stopped() {
            return Err("stopped".into());
        }
        ctx.step(fi, times.len() - 1, "Tracking");
        let g = src.gray(*t, o.channel, o.blur as f32).ok_or("cannot read the layer's frames")?;
        for (i, q) in pts.iter().enumerate() {
            if !alive[i] {
                out[i].push((None, 0.0));
                continue;
            }
            let (hw, hh) = half(q.fs);
            let (sw, sh) = half(q.ss);
            let rx = sw.saturating_sub(hw).max(2);
            let ry = sh.saturating_sub(hh).max(2);
            let pred = [cur[i][0] + vel[i][0], cur[i][1] + vel[i][1]];
            let so = [(q.so[0] * k as f64) as f32, (q.so[1] * k as f64) as f32];
            let m = vision::find(&tpl[i], hw, hh, &g, [pred[0] + so[0], pred[1] + so[1]], rx, ry);
            let conf = (m.score * 100.0) as f64;
            let mut next = m.center;
            if !o.subpixel {
                next = [next[0].round(), next[1].round()];
            }
            let mut refresh = o.adapt_every;
            if conf < o.threshold {
                match o.action.as_str() {
                    "stop" => {
                        alive[i] = false;
                        out[i].push((None, conf));
                        continue;
                    }
                    "extrapolate" => next = pred,
                    "adapt" => refresh = true,
                    _ => {}
                }
            }
            if refresh {
                tpl[i] = vision::patch(&g, next, hw, hh);
            }
            vel[i] = [next[0] - cur[i][0], next[1] - cur[i][1]];
            cur[i] = next;
            out[i].push((Some(src.to_layer(next)), conf));
        }
    }
    Ok(out)
}

fn comp_point(ev: &Eval, l: &Layer, p: [f64; 2]) -> [f64; 2] {
    ev.layer_to_comp(l).apply(p[0], p[1])
}

pub fn apply(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let uid = tracker_uid(e, cid, lid, p)?;
    let dims = ps(p, "dimensions").unwrap_or("xy").to_string();
    let c = e.project.comp(cid).ok_or("no composition")?.clone();
    let l = c.layer(lid).ok_or("no layer")?.clone();
    let g = l.props.find_group(uid).ok_or("no tracker")?.clone();
    let kind = str_of(&g, "kind");
    let target = g.get("target").and_then(|t| t.value.layer()).or_else(|| (kind == "stabilize").then_some(lid)).ok_or("set a Motion Target first (Edit Target)")?;
    let tl = c.layer(target).ok_or("the target layer is gone")?.clone();
    let groups: Vec<&Group> = g.groups().collect();
    let attach: Vec<&Prop> = groups.iter().filter_map(|pg| pg.get("attachPoint")).collect();
    let first = attach.first().ok_or("the tracker has no points")?;
    if first.keys.is_empty() {
        return Err("analyze the tracker first".into());
    }
    let key_times: Vec<f64> = first.keys.iter().map(|k| k.time).collect();
    let (rot, scale, pos) = (bool_of(&g, "trackRotation"), bool_of(&g, "trackScale"), bool_of(&g, "trackPosition"));
    let pt_at = |i: usize, lt: f64| -> [f64; 2] { attach.get(i).map(|pr| pr.at(lt).vec2()).unwrap_or([0.0; 2]) };
    let pj = &e.project;
    let comp_t = |lt: f64| l.comp_time(lt);
    match kind.as_str() {
        "raw" => Err("a Raw tracker keeps its data only; copy its keys where you need them".into()),
        "transform" | "stabilize" => {
            let stab = kind == "stabilize";
            let t0 = key_times[0];
            let a0 = (pt_at(1, t0)[1] - pt_at(0, t0)[1]).atan2(pt_at(1, t0)[0] - pt_at(0, t0)[0]);
            let d0 = (pt_at(1, t0)[0] - pt_at(0, t0)[0]).hypot(pt_at(1, t0)[1] - pt_at(0, t0)[1]).max(1e-6);
            let ev0 = Eval::new(pj, cid, &c, e.time_of(cid));
            let base_rot = ev0.tr_f(&tl, "rotation", 0.0);
            let base_scale = ev0.tr_v3(&tl, "scale", [100.0; 3]);
            let base_pos = ev0.position(&tl);
            let mut rows: Vec<(f64, [f64; 2], f64, f64)> = vec![];
            for lt in &key_times {
                let ct = comp_t(*lt);
                let ev = Eval::new(pj, cid, &c, ct);
                let p0 = pt_at(0, *lt);
                let anchor_val = if stab { p0 } else { comp_point(&ev, &l, p0) };
                let (mut ang, mut sc) = (0.0, 1.0);
                if groups.len() >= 2 {
                    let p1 = pt_at(1, *lt);
                    ang = ((p1[1] - p0[1]).atan2(p1[0] - p0[0]) - a0).to_degrees();
                    sc = (p1[0] - p0[0]).hypot(p1[1] - p0[1]) / d0;
                }
                rows.push((ct, anchor_val, ang, sc));
            }
            let target_parent_inv = Eval::new(pj, cid, &c, e.time_of(cid)).parent_to_comp(&tl).inverse();
            let count = rows.len();
            let tgt = e.layer_mut(cid, tl.id)?;
            let tr = tgt.props.sub_mut("transform").ok_or("the target has no transform")?;
            for (ct, v, ang, sc) in rows {
                let lt = tl.layer_time(ct);
                if pos || stab {
                    let id = if stab { "anchor" } else { "position" };
                    if let Some(pr) = tr.get_mut(id) {
                        let cur = pr.at(lt).vec3();
                        let v = if stab { v } else { target_parent_inv.map(|m| m.apply(v[0], v[1])).unwrap_or(v) };
                        let nv = match dims.as_str() {
                            "x" => [v[0], cur[1], cur[2]],
                            "y" => [cur[0], v[1], cur[2]],
                            _ => [v[0], v[1], cur[2]],
                        };
                        key::insert(&mut pr.keys, Key::new(lt, Value::Vec3(nv), pr.spatial));
                    }
                }
                if rot && let Some(pr) = tr.get_mut("rotation") {
                    let r = if stab { base_rot - ang } else { base_rot + ang };
                    key::insert(&mut pr.keys, Key::new(lt, Value::Num(r), false));
                }
                if scale && let Some(pr) = tr.get_mut("scale") {
                    let s = if stab { 1.0 / sc.max(1e-6) } else { sc };
                    key::insert(&mut pr.keys, Key::new(lt, Value::Vec3([base_scale[0] * s, base_scale[1] * s, base_scale[2]]), false));
                }
            }
            if stab
                && let Some(pr) = tr.get_mut("position")
                && pr.keys.is_empty()
            {
                pr.value = Value::Vec3(base_pos);
            }
            e.set_label("Apply Motion Tracker");
            Ok(json!({"target": target, "keys": count}))
        }
        _ => {
            let n = groups.len();
            if (kind == "affine" && n < 3) || (kind == "perspective" && n < 4) {
                return Err("the tracker needs more track points".into());
            }
            let mut rows: Vec<(f64, [[f64; 2]; 4])> = vec![];
            for lt in &key_times {
                let ct = comp_t(*lt);
                let ev = Eval::new(pj, cid, &c, ct);
                let q: Vec<[f64; 2]> = (0..n.min(4)).map(|i| comp_point(&ev, &l, pt_at(i, *lt))).collect();
                let corners = if kind == "affine" { [q[0], q[1], q[2], [q[1][0] + q[2][0] - q[0][0], q[1][1] + q[2][1] - q[0][1]]] } else { [q[0], q[1], q[2], q[3]] };
                let inv = ev.layer_to_comp(&tl).inverse();
                let to_t = |pt: [f64; 2]| inv.map(|m| m.apply(pt[0], pt[1])).unwrap_or(pt);
                rows.push((ct, corners.map(to_t)));
            }
            corner_pin_keys(e, cid, target, &rows)?;
            e.set_label("Apply Motion Tracker");
            Ok(json!({"target": target, "keys": rows.len()}))
        }
    }
}

pub fn corner_pin_keys(e: &mut Engine, cid: u64, target: u64, rows: &[(f64, [[f64; 2]; 4])]) -> Result<u64, String> {
    let existing = e.layer_ref(cid, target)?.effects().and_then(|fx| fx.groups().find(|g| matches!(&g.kind, GroupKind::Effect { effect } if effect == "fx.distort.cornerPin")).map(|g| g.uid));
    let uid = match existing {
        Some(u) => u,
        None => {
            let r = super::effect::apply(e, &json!({"effect": "fx.distort.cornerPin", "layers": [target], "comp": cid}))?;
            r.get("applied").and_then(|a| a.get(0)).and_then(|a| a.get("uid")).and_then(Json::as_u64).ok_or("could not add Corner Pin")?
        }
    };
    let tl = e.layer_ref(cid, target)?.clone();
    let l = e.layer_mut(cid, target)?;
    let g = l.props.find_group_mut(uid).ok_or("no Corner Pin")?;
    for (ct, q) in rows {
        let lt = tl.layer_time(*ct);
        for (id, v) in ["ul", "ur", "ll", "lr"].iter().zip(q) {
            if let Some(pr) = g.get_mut(id) {
                key::insert(&mut pr.keys, Key::new(lt, Value::Vec2(*v), pr.spatial));
            }
        }
    }
    Ok(uid)
}

pub fn mask_method(e: &mut Engine, p: &Json) -> Res {
    let m = ps(p, "method").ok_or("pass `method`")?;
    if !METHODS.contains(&m) {
        return Err(format!("method is one of {}", METHODS.join(", ")));
    }
    e.state.mask_method = m.to_string();
    e.revision += 1;
    Ok(json!({"method": m}))
}

fn inside_poly(poly: &[[f32; 2]], x: f32, y: f32) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
    }
    c
}

fn model_of(method: &str) -> Model {
    match method {
        "positionScale" => Model::ScaleOnly,
        "positionScaleRotation" => Model::Similarity,
        "positionScaleRotationSkew" => Model::Affine,
        "perspective" => Model::Homography,
        _ => Model::Translation,
    }
}

pub fn surface_track(ctx: &Ctx, src: &Source, times: &[f64], region: &[[f32; 2]], model: Model) -> Result<Vec<Option<H3>>, String> {
    let g0 = src.gray(times[0], 1, 0.0).ok_or("cannot read the layer's frames")?;
    let poly = region.to_vec();
    let mut feats = vision::corners(&g0, &|x, y| inside_poly(&poly, x, y), 300, 7.0);
    if feats.len() < 6 {
        return Err("too few trackable details inside the mask; draw it around textured detail".into());
    }
    let ref_pts: Vec<[f32; 2]> = feats.clone();
    let tpls: Vec<Vec<f32>> = ref_pts.iter().map(|p| vision::patch(&g0, *p, 5, 5)).collect();
    let mut prev_pyr = g0.pyramid(4);
    let mut h_cur: H3 = vision::IDENTITY;
    let mut out: Vec<Option<H3>> = vec![Some(vision::IDENTITY)];
    let mut alive: Vec<bool> = vec![true; feats.len()];
    for (fi, t) in times.iter().enumerate().skip(1) {
        if ctx.stopped() {
            return Err("stopped".into());
        }
        ctx.step(fi, times.len() - 1, "Tracking mask");
        let g = src.gray(*t, 1, 0.0).ok_or("cannot read the layer's frames")?;
        let pyr = g.pyramid(4);
        let moved = vision::klt(&prev_pyr, &pyr, &feats, None, 7);
        let (mut s, mut d) = (vec![], vec![]);
        for (i, m) in moved.iter().enumerate() {
            if let (true, Some(q)) = (alive[i], m) {
                s.push([feats[i][0] as f64, feats[i][1] as f64]);
                d.push([q[0] as f64, q[1] as f64]);
            }
        }
        let step = vision::ransac(model, &s, &d, 2.0, 300).map(|r| r.0);
        let Some(step) = step else {
            out.push(None);
            prev_pyr = pyr;
            continue;
        };
        let predicted = vision::h_mul(&step, &h_cur);
        let (mut rs, mut rd) = (vec![], vec![]);
        for (i, rp) in ref_pts.iter().enumerate() {
            let q = vision::h_apply(&predicted, [rp[0] as f64, rp[1] as f64]);
            let m = vision::find(&tpls[i], 5, 5, &g, [q[0] as f32, q[1] as f32], 4, 4);
            if m.score > 0.75 {
                rs.push([rp[0] as f64, rp[1] as f64]);
                rd.push([m.center[0] as f64, m.center[1] as f64]);
                alive[i] = true;
            } else {
                alive[i] = false;
            }
        }
        h_cur = match vision::ransac(model, &rs, &rd, 1.5, 300) {
            Some((h, _)) if rs.len() >= 8 => h,
            _ => predicted,
        };
        feats = ref_pts.iter().map(|rp| {
            let q = vision::h_apply(&h_cur, [rp[0] as f64, rp[1] as f64]);
            [q[0] as f32, q[1] as f32]
        }).collect();
        for (i, a) in alive.iter_mut().enumerate() {
            if !*a {
                *a = feats[i][0] >= 0.0 && feats[i][1] >= 0.0 && (feats[i][0] as usize) < g.w && (feats[i][1] as usize) < g.h;
            }
        }
        out.push(Some(h_cur));
        prev_pyr = pyr;
    }
    Ok(out)
}

fn buf_h_to_layer(src: &Source, h: &H3) -> Mat3 {
    let k = src.scale;
    let f = src.frame;
    let to_buf = Mat3([[k, 0.0, -f[0] * k], [0.0, k, -f[1] * k], [0.0, 0.0, 1.0]]);
    let to_layer = Mat3([[1.0 / k, 0.0, f[0]], [0.0, 1.0 / k, f[1]], [0.0, 0.0, 1.0]]);
    to_layer * Mat3(*h) * to_buf
}

pub fn track_mask(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let mid = super::layermisc::mask_uid(e, cid, lid, p)?;
    let method = ps(p, "method").map(str::to_string).unwrap_or_else(|| if e.state.mask_method.is_empty() { "position".into() } else { e.state.mask_method.clone() });
    if !METHODS.contains(&method.as_str()) {
        return Err(format!("method is one of {}", METHODS.join(", ")));
    }
    let (times, _) = span(e, cid, lid, p)?;
    let (path0, layer_times) = {
        let c = e.project.comp(cid).ok_or("no composition")?;
        let l = c.layer(lid).ok_or("no layer")?;
        let g = l.props.find_group(mid).ok_or("no mask")?;
        let ev = Eval::new(&e.project, cid, c, times[0]);
        let path = g.get("path").map(|pr| ev.value(l, pr)).and_then(|v| v.as_path().cloned()).ok_or("the mask has no path")?;
        (path, times.iter().map(|t| l.layer_time(*t)).collect::<Vec<f64>>())
    };
    let wait = pb(p, "wait").unwrap_or(false);
    let face_mode = method.starts_with("face");
    let learned = face_mode && learned_face(e);
    let detailed = method == "faceDetailed";
    jobs::run(e, "mask", lid, "Track Mask", wait, move |ctx| {
        let src = Source::new(&ctx.project, &ctx.media, cid, lid, MAX_SIDE).ok_or("the layer is gone")?;
        let mut paths: Vec<Option<BezPath>> = vec![];
        let mut feats: Vec<Option<Vec<[f64; 2]>>> = vec![];
        if face_mode {
            let b = path0.bounds().ok_or("empty mask")?;
            let a = src.to_buf([b[0], b[1]]);
            let z = src.to_buf([b[2], b[3]]);
            let mut prior: Option<face::Ellipse> = None;
            let mut roi = [a[0] as f64, a[1] as f64, z[0] as f64, z[1] as f64];
            let mut mesh = if learned { Some(crate::render::facemesh::Tracker::new()?) } else { None };
            for (fi, t) in times.iter().enumerate() {
                if ctx.stopped() {
                    return Err("stopped".into());
                }
                ctx.step(fi, times.len(), "Tracking face");
                let img = src.image(*t).ok_or("cannot read the layer's frames")?;
                if let Some(tr) = mesh.as_mut() {
                    match tr.next(&img, roi)? {
                        Some(f) => {
                            let at = |i: usize| src.to_layer([f.points[i][0], f.points[i][1]]);
                            let oval: Vec<[f64; 2]> = crate::render::facemesh::OVAL.iter().map(|i| at(*i)).collect();
                            paths.push(Some(smooth_loop(&oval)));
                            feats.push(detailed.then(|| crate::render::facemesh::FEATURES.iter().map(|i| at(*i)).collect()));
                            let r = f.rect;
                            let h = r.size as f64 * 0.75;
                            roi = [r.c[0] as f64 - h, r.c[1] as f64 - h, r.c[0] as f64 + h, r.c[1] as f64 + h];
                        }
                        None => {
                            paths.push(None);
                            feats.push(None);
                        }
                    }
                    continue;
                }
                let found = face::detect(&img, roi, prior.as_ref());
                let el = match (found, prior) {
                    (Some(d), Some(pr)) => Some(pr.lerp(&d, 0.65)),
                    (Some(d), None) => Some(d),
                    (None, pr) => pr,
                };
                let Some(el) = el else {
                    paths.push(None);
                    feats.push(None);
                    continue;
                };
                prior = Some(el);
                let pad = 0.6;
                roi = [el.c[0] - el.a * (1.0 + pad) - el.b * pad, el.c[1] - el.b * (1.0 + pad), el.c[0] + el.a * (1.0 + pad) + el.b * pad, el.c[1] + el.b * (1.0 + pad)];
                let m = Mat3::translate(el.c[0], el.c[1]) * Mat3::rotate_deg(el.angle.to_degrees());
                let bp = BezPath::ellipse(0.0, 0.0, el.a * 2.0, el.b * 2.0).transformed(&m);
                let to_layer = Mat3([[1.0 / src.scale, 0.0, src.frame[0]], [0.0, 1.0 / src.scale, src.frame[1]], [0.0, 0.0, 1.0]]);
                paths.push(Some(bp.transformed(&to_layer)));
                feats.push(detailed.then(|| face::features(&img, &el).into_iter().map(|q| src.to_layer([q[0] as f32, q[1] as f32])).collect()));
            }
        } else {
            let poly: Vec<[f32; 2]> = path0.flatten(1.0).into_iter().map(|q| src.to_buf(q)).collect();
            let hs = surface_track(ctx, &src, &times, &poly, model_of(&method))?;
            let mut last = None;
            for h in hs {
                let h = h.or(last);
                last = h;
                paths.push(h.map(|h| path0.transformed(&buf_h_to_layer(&src, &h))));
            }
        }
        let apply: Apply = Box::new(move |e: &mut Engine| {
            let has_feats = feats.iter().any(|f| f.is_some());
            let fx_uid = if has_feats { Some(face_points_effect(e, cid, lid)?) } else { None };
            let l = e.layer_mut(cid, lid)?;
            let g = l.props.find_group_mut(mid).ok_or("the mask is gone")?;
            let pr = g.get_mut("path").ok_or("no mask path")?;
            let mut n = 0;
            for (i, bp) in paths.iter().enumerate() {
                if let Some(bp) = bp {
                    key::insert(&mut pr.keys, Key::new(layer_times[i], Value::Path(bp.clone()), false));
                    n += 1;
                }
            }
            if let Some(fu) = fx_uid {
                let fg = l.props.find_group_mut(fu).ok_or("no Face Track Points")?;
                for (i, f) in feats.iter().enumerate() {
                    let Some(f) = f else { continue };
                    for (k, (id, _)) in face::POINTS.iter().enumerate() {
                        if let (Some(pr), Some(v)) = (fg.get_mut(id), f.get(k)) {
                            key::insert(&mut pr.keys, Key::new(layer_times[i], Value::Vec2(*v), pr.spatial));
                        }
                    }
                }
            }
            Ok(json!({"mask": mid, "frames": n}))
        });
        Ok(apply)
    })
}

fn smooth_loop(pts: &[[f64; 2]]) -> BezPath {
    let n = pts.len();
    let mut bp = BezPath::polygon(pts, true);
    for i in 0..n {
        let (a, b) = (pts[(i + n - 1) % n], pts[(i + 1) % n]);
        let t = [(b[0] - a[0]) / 6.0, (b[1] - a[1]) / 6.0];
        bp.outs[i] = t;
        bp.ins[i] = [-t[0], -t[1]];
    }
    bp
}

fn face_points_effect(e: &mut Engine, cid: u64, lid: u64) -> Result<u64, String> {
    let found = e.layer_ref(cid, lid)?.effects().and_then(|fx| fx.groups().find(|g| matches!(&g.kind, GroupKind::Effect { effect } if effect == "fx.face.trackPoints")).map(|g| g.uid));
    if let Some(u) = found {
        return Ok(u);
    }
    let r = super::effect::apply(e, &json!({"effect": "fx.face.trackPoints", "layers": [lid], "comp": cid}))?;
    r.get("applied").and_then(|a| a.get(0)).and_then(|a| a.get("uid")).and_then(Json::as_u64).ok_or_else(|| "could not add Face Track Points".into())
}

pub fn extract_face(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let rows = {
        let l = e.layer_ref(cid, lid)?;
        let fx = l.effects().ok_or("no effects")?;
        let g = fx.groups().find(|g| matches!(&g.kind, GroupKind::Effect { effect } if effect == "fx.face.trackPoints")).ok_or("track the face with Face Tracking (Detailed Features) first")?;
        let first = g.get(face::POINTS[0].0).ok_or("no Face Track Points")?;
        let times: Vec<f64> = first.keys.iter().map(|k| k.time).collect();
        if times.is_empty() {
            return Err("the Face Track Points have no keys yet".into());
        }
        let rows: Vec<(f64, Vec<f64>)> = times
            .iter()
            .map(|t| {
                let pts: Vec<[f64; 2]> = face::POINTS.iter().map(|(id, _)| g.get(id).map(|pr| pr.at(*t).vec2()).unwrap_or([0.0; 2])).collect();
                (*t, face::measure(&pts))
            })
            .collect();
        rows
    };
    let existing = e.layer_ref(cid, lid)?.effects().and_then(|fx| fx.groups().find(|g| matches!(&g.kind, GroupKind::Effect { effect } if effect == "fx.face.measurements")).map(|g| g.uid));
    let mu = match existing {
        Some(u) => u,
        None => {
            let r = super::effect::apply(e, &json!({"effect": "fx.face.measurements", "layers": [lid], "comp": cid}))?;
            r.get("applied").and_then(|a| a.get(0)).and_then(|a| a.get("uid")).and_then(Json::as_u64).ok_or("could not add Face Measurements")?
        }
    };
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(mu).ok_or("no Face Measurements")?;
    let mut clip = vec![];
    for (t, vals) in &rows {
        for (k, (id, _)) in face::MEASUREMENTS.iter().enumerate() {
            if let Some(pr) = g.get_mut(id) {
                let key = Key::new(*t, Value::Num(vals[k]), false);
                key::insert(&mut pr.keys, key.clone());
                clip.push((format!("effects/Face Measurements/{id}"), *t, key));
            }
        }
    }
    e.clipboard = Some(crate::engine::Clip::Keys(clip.into_iter().map(|(a, t, k)| (a, t - rows[0].0, k)).collect()));
    e.set_label("Extract & Copy Face Measurements");
    Ok(json!({"effect": mu, "keys": rows.len()}))
}

pub fn learned_face(e: &Engine) -> bool {
    crate::render::facemesh::installed() && e.prefs.get("faceModel").and_then(Json::as_str) != Some("classical")
}

pub fn face_models(e: &mut Engine, _: &Json) -> Res {
    use crate::render::facemesh::{MODEL, installed, model_path};
    let learned = learned_face(e);
    Ok(json!({"models": [
        {"id": "classical", "name": "Classical (built in)", "installed": true, "active": !learned, "size": 0},
        {"id": MODEL.id, "name": MODEL.name, "installed": installed(), "active": learned, "size": MODEL.size, "licence": MODEL.licence, "url": MODEL.url, "sha256": MODEL.sha256, "path": model_path().to_string_lossy()},
    ], "active": if learned { MODEL.id } else { "classical" }, "busy": e.jobs.running()}))
}

pub fn face_model_select(e: &mut Engine, p: &Json) -> Res {
    match ps(p, "id").unwrap_or("classical") {
        "classical" | "classic" => {
            e.prefs.insert("faceModel".into(), json!("classical"));
            Ok(json!({"active": "classical"}))
        }
        "facelandmarker" => {
            if !crate::render::facemesh::installed() {
                return Err("the Face Landmarker model is not installed: face.model.download, or Settings > Face Tracking".into());
            }
            crate::render::facemesh::load()?;
            e.prefs.insert("faceModel".into(), json!("facelandmarker"));
            Ok(json!({"active": "facelandmarker"}))
        }
        other => Err(format!("no face tracking model {other} (classical or facelandmarker)")),
    }
}

pub fn planar(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let mid = super::layermisc::mask_uid(e, cid, lid, p)?;
    let reference = pf(p, "reference").unwrap_or_else(|| e.time_of(cid));
    let c = e.project.comp(cid).ok_or("no composition")?.clone();
    let reference = c.snap(reference);
    let l = c.layer(lid).ok_or("no layer")?.clone();
    let ev = Eval::new(&e.project, cid, &c, reference);
    let path = l.props.find_group(mid).and_then(|g| g.get("path")).map(|pr| ev.value(&l, pr)).and_then(|v| v.as_path().cloned()).ok_or("the mask has no path")?;
    let corners: [[f64; 2]; 4] = match p.get("surface").and_then(Json::as_array) {
        Some(a) if a.len() == 4 => {
            let v: Vec<[f64; 2]> = a.iter().filter_map(|q| Some([q.get(0)?.as_f64()?, q.get(1)?.as_f64()?])).collect();
            if v.len() != 4 {
                return Err("`surface` is four [x, y] corners".into());
            }
            [v[0], v[1], v[2], v[3]]
        }
        _ => {
            if path.vertices.len() == 4 {
                let mut v = path.vertices.clone();
                v.sort_by(|a, b| a[1].total_cmp(&b[1]));
                let (mut top, mut bot) = ([v[0], v[1]], [v[2], v[3]]);
                top.sort_by(|a, b| a[0].total_cmp(&b[0]));
                bot.sort_by(|a, b| a[0].total_cmp(&b[0]));
                [top[0], top[1], bot[0], bot[1]]
            } else {
                let b = path.bounds().ok_or("empty mask")?;
                [[b[0], b[1]], [b[2], b[1]], [b[0], b[3]], [b[2], b[3]]]
            }
        }
    };
    let fwd = frame_times(&e.project, cid, lid, pf(p, "start").map(|s| s.max(reference)).unwrap_or(reference), pf(p, "end").unwrap_or(c.duration));
    let mut back = frame_times(&e.project, cid, lid, pf(p, "start").unwrap_or(0.0), reference);
    back.reverse();
    let ctx = Ctx { project: e.project.clone(), media: e.media.clone(), prog: std::sync::Arc::new(parking_lot::Mutex::new(jobs::Prog { done: 0, total: 0, task: String::new(), started: std::time::Instant::now() })), stop: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)) };
    let src = Source::new(&e.project, &e.media, cid, lid, MAX_SIDE).ok_or("the layer is gone")?;
    let poly: Vec<[f32; 2]> = path.flatten(1.0).into_iter().map(|q| src.to_buf(q)).collect();
    let mut rows: Vec<(f64, [[f64; 2]; 4])> = vec![];
    for (times, rev) in [(back, true), (fwd, false)] {
        if times.len() < 2 {
            continue;
        }
        let hs = surface_track(&ctx, &src, &times, &poly, Model::Homography)?;
        let mut last = None;
        for (i, h) in hs.iter().enumerate() {
            if rev && i == 0 {
                continue;
            }
            let h = h.or(last);
            last = h;
            let Some(h) = h else { continue };
            let m = buf_h_to_layer(&src, &h);
            rows.push((times[i], corners.map(|q| m.apply(q[0], q[1]))));
        }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    if rows.is_empty() {
        return Err("nothing to track (move the reference time inside the layer)".into());
    }
    let target = match p.get("target").filter(|v| !v.is_null()) {
        Some(v) => Some(e.layer_in(&c, v)?),
        None => None,
    };
    e.apply_step("Planar Track", move |e| {
        let uid = add_tracker(e, cid, lid, "perspective", true, false, false, target)?;
        if let Some(g) = e.layer_mut(cid, lid)?.props.find_group_mut(uid) {
            g.name = "Planar Tracker".into();
        }
        let ltimes: Vec<f64> = {
            let l = e.layer_ref(cid, lid)?;
            rows.iter().map(|r| l.layer_time(r.0)).collect()
        };
        {
            let l = e.layer_mut(cid, lid)?;
            let g = l.props.find_group_mut(uid).ok_or("no tracker")?;
            for (pi, pg) in g.groups_mut().enumerate() {
                for (ri, r) in rows.iter().enumerate() {
                    let c = r.1[pi];
                    for (id, v) in [("featureCenter", Value::Vec2(c)), ("attachPoint", Value::Vec2(c)), ("confidence", Value::Num(100.0))] {
                        if let Some(pr) = pg.get_mut(id) {
                            key::insert(&mut pr.keys, Key::new(ltimes[ri], v, pr.spatial));
                        }
                    }
                }
            }
        }
        let mut name = Json::Null;
        if let Some(t) = target {
            let comp = e.project.comp(cid).ok_or("no composition")?.clone();
            let src_l = comp.layer(lid).ok_or("no layer")?.clone();
            let tl = comp.layer(t).ok_or("no target layer")?.clone();
            let pinned: Vec<(f64, [[f64; 2]; 4])> = rows
                .iter()
                .map(|(ct, q)| {
                    let ev = Eval::new(&e.project, cid, &comp, *ct);
                    let to_c = ev.layer_to_comp(&src_l);
                    let inv = ev.layer_to_comp(&tl).inverse();
                    (*ct, q.map(|pt| {
                        let c = to_c.apply(pt[0], pt[1]);
                        inv.map(|m| m.apply(c[0], c[1])).unwrap_or(c)
                    }))
                })
                .collect();
            corner_pin_keys(e, cid, t, &pinned)?;
            let has_grow = e.layer_ref(cid, t)?.effects().is_some_and(|fx| fx.groups().any(|g| matches!(&g.kind, GroupKind::Effect { effect } if effect == "fx.utility.growBounds")));
            if !has_grow {
                let _ = super::effect::apply(e, &json!({"effect": "fx.utility.growBounds", "layers": [t], "comp": cid}));
            }
            name = json!(tl.name);
        }
        Ok(json!({"frames": rows.len(), "applied": if target.is_some() { rows.len() } else { 0 }, "targetName": name, "tracker": uid}))
    })
}

fn place_face_model(bytes: &[u8]) -> Result<std::path::PathBuf, String> {
    use crate::render::facemesh::{MODEL, model_path, unload};
    if crate::render::roto::sha256_hex(bytes) != MODEL.sha256 {
        return Err("that file is not the official MediaPipe Face Landmarker bundle (checksum mismatch)".into());
    }
    let path = model_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot write {}: {e}", dir.display()))?;
    }
    std::fs::write(&path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    unload();
    crate::render::facemesh::load()?;
    Ok(path)
}

pub fn face_model_install(_: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path`: the face_landmarker.task file")?;
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let out = place_face_model(&bytes)?;
    Ok(json!({"installed": crate::render::facemesh::MODEL.id, "path": out.to_string_lossy()}))
}

pub fn face_model_download(_: &mut Engine, _: &Json) -> Res {
    use crate::render::facemesh::MODEL;
    let tmp = std::env::temp_dir().join(format!("formvfx-face-{}.task", std::process::id()));
    let ok = std::process::Command::new("curl").args(["-fsSL", "-o"]).arg(&tmp).arg(MODEL.url).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("the download failed; fetch {} yourself and use face.model.install {{path}}", MODEL.url));
    }
    let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp);
    let out = place_face_model(&bytes)?;
    Ok(json!({"installed": MODEL.id, "path": out.to_string_lossy()}))
}

pub fn face_model_remove(_: &mut Engine, _: &Json) -> Res {
    let path = crate::render::facemesh::model_path();
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    crate::render::facemesh::unload();
    Ok(json!({"removed": true}))
}
