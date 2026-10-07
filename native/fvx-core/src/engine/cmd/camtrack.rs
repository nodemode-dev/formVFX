use serde_json::{Value as Json, json};

use super::track::track_layer;
use crate::engine::jobs::{self, Apply};
use crate::engine::{Engine, Res, pb, pf, ps};
use crate::eval::Eval;
use crate::model::key::{self, Key};
use crate::model::prop::{Group, GroupKind};
use crate::model::{AutoOrient, Value};
use crate::render::analysis::{Source, frame_times};
use crate::render::camsolve::{self, Ground, Placed, Settings, Solve, Tracks, V3};

pub const EFFECT: &str = "fx.perspective.cameraTracker";
const MAX_SIDE: f64 = 1280.0;
const KINDS: [&str; 5] = ["text", "solid", "null", "shadowCatcher", "camera"];

fn is_tracker(g: &Group) -> bool {
    matches!(&g.kind, GroupKind::Effect { effect } if effect == EFFECT)
}

fn effect_uid(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<Option<u64>, String> {
    let l = e.layer_ref(cid, lid)?;
    let Some(fx) = l.effects() else { return Ok(None) };
    match p.get("effect").and_then(Json::as_u64) {
        Some(u) => fx.groups().find(|g| g.uid == u && is_tracker(g)).map(|g| Some(g.uid)).ok_or_else(|| "no 3D Camera Tracker with that uid on the layer".into()),
        None => Ok(fx.groups().filter(|g| is_tracker(g)).last().map(|g| g.uid)),
    }
}

fn target(e: &Engine, p: &Json) -> Result<(u64, u64, u64), String> {
    let (cid, lid) = match p.get("layer").filter(|v| !v.is_null()) {
        Some(_) => track_layer(e, p)?,
        None => {
            let (cid, comp) = e.comp_of(p)?;
            let found = e.state.selected_layers.iter().copied().chain(comp.layers.iter().map(|l| l.id)).find(|id| comp.layer(*id).and_then(|l| l.effects()).is_some_and(|fx| fx.groups().any(is_tracker)));
            match found {
                Some(id) => (cid, id),
                None => track_layer(e, p)?,
            }
        }
    };
    let uid = effect_uid(e, cid, lid, p)?.ok_or("the layer has no 3D Camera Tracker (Animation > Track Camera adds one)")?;
    Ok((cid, lid, uid))
}

fn str_param(g: &Group, path: &str) -> String {
    match g.prop(path).map(|pr| &pr.value) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

fn set_param(g: &mut Group, path: &str, v: Value) {
    let (head, last) = path.rsplit_once('/').unwrap_or(("", path));
    if let Some(pr) = g.group_mut(head).and_then(|h| h.get_mut(last)) {
        pr.value = v;
        pr.keys.clear();
    }
}

fn deleted_of(g: &Group) -> Vec<u32> {
    str_param(g, "deleted").split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

fn settings_of(g: &Group) -> Settings {
    let idx = |path: &str| g.prop(path).map(|pr| pr.value.index()).unwrap_or(0);
    Settings { shot: idx("shotType"), hfov: g.prop("horizontalAngleOfView").map(|pr| pr.value.num()).unwrap_or(40.0), method: idx("advanced/solveMethod"), deleted: deleted_of(g) }
}

pub fn track_camera(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = track_layer(e, p)?;
    let uid = match effect_uid(e, cid, lid, p)? {
        Some(u) => u,
        None => {
            let r = e.apply_step("3D Camera Tracker", |e| super::effect::apply(e, &json!({"effect": EFFECT, "layers": [lid], "comp": cid})))?;
            r.get("applied").and_then(|a| a.get(0)).and_then(|a| a.get("uid")).and_then(Json::as_u64).ok_or("could not add the 3D Camera Tracker")?
        }
    };
    let shot = match ps(p, "shotType") {
        Some("variable") => Some(1),
        Some("specify") => Some(2),
        Some("fixed") => Some(0),
        Some(other) => return Err(format!("shotType is fixed, variable or specify, not {other}")),
        None => None,
    };
    let method = match ps(p, "solveMethod") {
        Some("auto") => Some(0),
        Some("typical") => Some(1),
        Some("flat") => Some(2),
        Some("tripod") => Some(3),
        Some(other) => return Err(format!("solveMethod is auto, typical, flat or tripod, not {other}")),
        None => None,
    };
    let aov = pf(p, "aov");
    let detailed = pb(p, "detailed");
    if shot.is_some() || method.is_some() || aov.is_some() || detailed.is_some() {
        e.apply_step("3D Camera Tracker Settings", |e| {
            let g = e.layer_mut(cid, lid)?.props.find_group_mut(uid).ok_or("no 3D Camera Tracker")?;
            if let Some(s) = shot {
                set_param(g, "shotType", Value::Enum(s));
            }
            if let Some(m) = method {
                set_param(g, "advanced/solveMethod", Value::Enum(m));
            }
            if let Some(a) = aov {
                set_param(g, "horizontalAngleOfView", Value::Num(a.clamp(1.0, 170.0)));
                if shot.is_none() {
                    set_param(g, "shotType", Value::Enum(2));
                }
            }
            if let Some(d) = detailed {
                set_param(g, "advanced/detailedAnalysis", Value::Bool(d));
                set_param(g, "tracks", Value::Str(String::new()));
            }
            Ok(json!({}))
        })?;
    }
    super::select_only(e, &[lid]);
    let r = analyze(e, &json!({"comp": cid, "layer": lid, "effect": uid, "wait": pb(p, "wait").unwrap_or(false)}))?;
    Ok(json!({"layer": lid, "effect": uid, "analysis": r}))
}

pub fn analyze(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = target(e, p)?;
    let (times, layer_times, fps, detailed, tracks, settings) = {
        let c = e.project.comp(cid).ok_or("no composition")?;
        let l = c.layer(lid).ok_or("no layer")?;
        let times = frame_times(&e.project, cid, lid, l.in_point, c.duration);
        if times.len() < 3 {
            return Err("the layer needs at least three frames inside the composition".into());
        }
        let lt: Vec<f64> = times.iter().map(|t| l.layer_time(*t)).collect();
        let span = (lt[lt.len() - 1] - lt[0]).abs().max(1e-9);
        let g = l.props.find_group(uid).ok_or("no 3D Camera Tracker")?;
        let detailed = g.prop("advanced/detailedAnalysis").is_some_and(|pr| pr.value.boolean());
        let tracks = camsolve::parsed_tracks(&str_param(g, "tracks")).filter(|t| t.frames == times.len() && (t.t0 - lt[0]).abs() < 1e-6);
        let fps = (lt.len() - 1) as f64 / span;
        (times, lt, fps, detailed, tracks, settings_of(g))
    };
    let wait = pb(p, "wait").unwrap_or(false);
    jobs::run(e, "camera", lid, "Track Camera", wait, move |ctx| {
        let tr: Tracks = match tracks {
            Some(t) => (*t).clone(),
            None => {
                let src = Source::new(&ctx.project, &ctx.media, cid, lid, MAX_SIDE).ok_or("the layer is gone")?;
                camsolve::track(ctx, &src, &times, &layer_times, fps, detailed)?
            }
        };
        let sv = camsolve::solve(ctx, &tr, &settings)?;
        let (enc_t, enc_s) = (tr.encode(), serde_json::to_string(&sv).map_err(|e| e.to_string())?);
        let (method, error, points) = (sv.method.clone(), sv.error, sv.points.len());
        let apply: Apply = Box::new(move |e: &mut Engine| {
            let g = e.layer_mut(cid, lid)?.props.find_group_mut(uid).ok_or("the 3D Camera Tracker is gone")?;
            set_param(g, "tracks", Value::Str(enc_t));
            set_param(g, "solve", Value::Str(enc_s));
            set_param(g, "advanced/methodUsed", Value::Str(method.clone()));
            set_param(g, "advanced/averageError", Value::Num((error * 1000.0).round() / 1000.0));
            refresh_camera(e, cid, lid, uid)?;
            Ok(json!({"effect": uid, "method": method, "error": error, "points": points}))
        });
        Ok(apply)
    })
}

pub fn cancel(e: &mut Engine, _: &Json) -> Res {
    Ok(json!({"stopped": e.jobs.stop(Some("camera"))}))
}

struct Ctx3 {
    sv: std::sync::Arc<Solve>,
    tr: std::sync::Arc<Tracks>,
    placed: Placed,
    deleted: Vec<u32>,
    target_size: f64,
    camera: Option<u64>,
}

fn layer_scale(ev: &Eval, l: &crate::model::Layer) -> f64 {
    let m = ev.layer_to_comp(l).0;
    m[0][0].hypot(m[1][0]).max(1e-9)
}

fn ctx3(e: &Engine, cid: u64, lid: u64, uid: u64, ground: Option<Ground>) -> Result<Ctx3, String> {
    let c = e.project.comp(cid).ok_or("no composition")?;
    let l = c.layer(lid).ok_or("no layer")?;
    let g = l.props.find_group(uid).ok_or("no 3D Camera Tracker")?;
    let sv = camsolve::parsed_solve(&str_param(g, "solve")).ok_or("the camera is not solved yet (camera.analyze)")?;
    let tr = camsolve::parsed_tracks(&str_param(g, "tracks")).ok_or("the camera is not solved yet (camera.analyze)")?;
    let ground = ground.unwrap_or_else(|| serde_json::from_str(&str_param(g, "ground")).unwrap_or_default());
    let ev = Eval::new(&e.project, cid, c, e.time_of(cid));
    let placed = Placed::new(&sv, c.width as f64, c.height as f64, layer_scale(&ev, l), ground);
    let camera = str_param(g, "camera").parse::<u64>().ok().filter(|id| c.layer(*id).is_some_and(|x| x.is_camera()));
    Ok(Ctx3 { sv, tr, placed, deleted: deleted_of(g), target_size: g.prop("targetSize").map(|pr| pr.value.num()).unwrap_or(100.0), camera })
}

fn frame_at(e: &Engine, cid: u64, lid: u64, sv: &Solve, t: f64) -> Result<usize, String> {
    let l = e.layer_ref(cid, lid)?;
    Ok(sv.frame_of(l.layer_time(t)))
}

fn visible(x: &Ctx3, k: usize) -> Vec<(u32, V3)> {
    let seen: std::collections::HashSet<u32> = x.tr.tracks.iter().filter(|t| t.at(k).is_some()).map(|t| t.id).collect();
    x.sv.points.iter().filter(|p| seen.contains(&p.id) && !x.deleted.contains(&p.id)).map(|p| (p.id, x.placed.world(p.x))).collect()
}

fn project(cam: &(V3, camsolve::M3, f64), cw: f64, ch: f64, p: V3) -> Option<([f64; 2], f64)> {
    let local = camsolve::mv(&camsolve::mt(&cam.1), camsolve::sub(p, cam.0));
    if local[2] <= 1e-6 {
        return None;
    }
    Some(([cw * 0.5 + cam.2 * local[0] / local[2], ch * 0.5 + cam.2 * local[1] / local[2]], local[2]))
}

pub fn status(e: &mut Engine, p: &Json) -> Res {
    let Ok((cid, lid, uid)) = target(e, p) else {
        return Ok(json!({"layer": null, "effect": null, "solved": false, "running": e.jobs.running(), "progress": null}));
    };
    let progress = e.jobs.progress(lid, "camera");
    let running = e.jobs.running_for(lid, "camera");
    let g = e.layer_ref(cid, lid)?.props.find_group(uid).ok_or("no 3D Camera Tracker")?;
    let tracks = camsolve::parsed_tracks(&str_param(g, "tracks"));
    let sv = camsolve::parsed_solve(&str_param(g, "solve"));
    let x = ctx3(e, cid, lid, uid, None).ok();
    Ok(json!({
        "layer": lid, "effect": uid, "running": running, "progress": progress,
        "tracked": tracks.as_ref().map(|t| t.tracks.len()).unwrap_or(0),
        "solved": sv.is_some(),
        "method": sv.as_ref().map(|s| s.method.clone()),
        "error": sv.as_ref().map(|s| s.error),
        "points": sv.as_ref().map(|s| s.points.len()).unwrap_or(0),
        "frames": sv.as_ref().map(|s| s.cams.len()).unwrap_or(0),
        "angleOfView": sv.as_ref().and_then(|s| s.cams.first()).map(|c| 2.0 * ((s_width(&sv) * 0.5) / c.f).atan().to_degrees()),
        "deleted": deleted_of(g),
        "selected": e.state.solve_points,
        "camera": x.and_then(|x| x.camera),
    }))
}

fn s_width(sv: &Option<std::sync::Arc<Solve>>) -> f64 {
    sv.as_ref().map(|s| s.frame[2] - s.frame[0]).unwrap_or(1.0)
}

pub fn points(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = target(e, p)?;
    let x = ctx3(e, cid, lid, uid, None)?;
    let c = e.project.comp(cid).ok_or("no composition")?;
    let (cw, ch) = (c.width as f64, c.height as f64);
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let k = frame_at(e, cid, lid, &x.sv, t)?;
    let cam = x.placed.camera(&x.sv.cams[k]);
    let vis = visible(&x, k);
    let mut depths: Vec<f64> = vis.iter().filter_map(|(_, w)| project(&cam, cw, ch, *w).map(|q| q.1)).collect();
    depths.sort_by(|a, b| a.total_cmp(b));
    let dmed = depths.get(depths.len() / 2).copied().unwrap_or(1.0);
    let size = cw * 0.004;
    let out: Vec<Json> = vis
        .iter()
        .filter_map(|(id, w)| {
            let (q, d) = project(&cam, cw, ch, *w)?;
            let col = camsolve::color_of(*id);
            Some(json!({"id": id, "x": q[0], "y": q[1], "radius": size * (dmed / d).clamp(0.4, 2.5), "world": w, "color": col, "selected": e.state.solve_points.contains(id)}))
        })
        .collect();
    Ok(json!({"frame": k, "points": out, "selected": e.state.solve_points}))
}

fn ids_of(p: &Json, k: &str) -> Option<Vec<u32>> {
    p.get(k)?.as_array().map(|a| a.iter().filter_map(|v| v.as_u64().map(|x| x as u32)).collect())
}

pub fn select_points(e: &mut Engine, p: &Json) -> Res {
    let ids = ids_of(p, "points").ok_or("pass `points`: track point ids from camera.points")?;
    if pb(p, "toggle").unwrap_or(false) {
        for id in ids {
            match e.state.solve_points.iter().position(|x| *x == id) {
                Some(i) => {
                    e.state.solve_points.remove(i);
                }
                None => e.state.solve_points.push(id),
            }
        }
    } else if pb(p, "add").unwrap_or(false) {
        for id in ids {
            if !e.state.solve_points.contains(&id) {
                e.state.solve_points.push(id);
            }
        }
    } else {
        e.state.solve_points = ids;
    }
    Ok(json!({"selected": e.state.solve_points}))
}

fn chosen(e: &Engine, p: &Json) -> Vec<u32> {
    ids_of(p, "points").unwrap_or_else(|| e.state.solve_points.clone())
}

fn unwrap_deg(prev: Option<V3>, v: V3) -> V3 {
    let Some(pv) = prev else { return v };
    let mut o = v;
    for i in 0..3 {
        while o[i] - pv[i] > 180.0 {
            o[i] -= 360.0;
        }
        while o[i] - pv[i] < -180.0 {
            o[i] += 360.0;
        }
    }
    o
}

fn write_camera(e: &mut Engine, cid: u64, lid: u64, cam_id: u64, x: &Ctx3) -> Result<(), String> {
    let src = e.layer_ref(cid, lid)?.clone();
    let rows: Vec<(f64, V3, V3, f64)> = {
        let mut prev = None;
        x.sv.cams.iter().enumerate().map(|(k, c)| {
            let (pos, rcw, zoom) = x.placed.camera(c);
            let o = unwrap_deg(prev, camsolve::euler_xyz(&rcw));
            prev = Some(o);
            (src.comp_time(x.sv.t0 + k as f64 / x.sv.fps.max(1e-9)), pos, o, zoom)
        }).collect()
    };
    let cl = e.layer_mut(cid, cam_id)?;
    cl.auto_orient = AutoOrient::Off;
    let lts: Vec<f64> = rows.iter().map(|r| cl.layer_time(r.0)).collect();
    if let Some(tr) = cl.props.sub_mut("transform") {
        for id in ["position", "orientation", "rotationX", "rotationY", "rotation"] {
            if let Some(pr) = tr.get_mut(id) {
                pr.keys.clear();
                if id.starts_with("rotation") {
                    pr.value = Value::Num(0.0);
                }
            }
        }
        for (i, r) in rows.iter().enumerate() {
            if let Some(pr) = tr.get_mut("position") {
                key::insert(&mut pr.keys, Key::new(lts[i], Value::Vec3(r.1), false));
            }
            if let Some(pr) = tr.get_mut("orientation") {
                key::insert(&mut pr.keys, Key::new(lts[i], Value::Vec3(r.2), false));
            }
        }
    }
    if let Some(g) = cl.props.sub_mut("cameraOptions")
        && let Some(z) = g.get_mut("zoom")
    {
        z.keys.clear();
        let fixed = rows.iter().all(|r| (r.3 - rows[0].3).abs() < 1e-6);
        if fixed {
            z.value = Value::Num(rows[0].3);
        } else {
            for (i, r) in rows.iter().enumerate() {
                key::insert(&mut z.keys, Key::new(lts[i], Value::Num(r.3), false));
            }
        }
        if let Some(fd) = g.get_mut("focusDistance") {
            fd.value = Value::Num(rows[0].3);
        }
    }
    Ok(())
}

fn ensure_camera(e: &mut Engine, cid: u64, lid: u64, uid: u64, x: &Ctx3) -> Result<u64, String> {
    let cam_id = match x.camera {
        Some(id) => id,
        None => {
            let r = super::layer::new_camera(e, &json!({"comp": cid, "name": "3D Tracker Camera", "type": "oneNode"}))?;
            let id = r.get("layer").and_then(Json::as_u64).ok_or("could not make the camera")?;
            if let Some(g) = e.layer_mut(cid, lid)?.props.find_group_mut(uid) {
                set_param(g, "camera", Value::Str(id.to_string()));
            }
            id
        }
    };
    write_camera(e, cid, lid, cam_id, x)?;
    Ok(cam_id)
}

fn refresh_camera(e: &mut Engine, cid: u64, lid: u64, uid: u64) -> Result<(), String> {
    if let Ok(x) = ctx3(e, cid, lid, uid, None)
        && let Some(id) = x.camera
    {
        write_camera(e, cid, lid, id, &x)?;
    }
    Ok(())
}

pub fn create(e: &mut Engine, p: &Json) -> Res {
    let mut q = p.clone();
    if let Some(o) = q.as_object_mut() {
        o.insert("kind".into(), json!("camera"));
    }
    create_from_solve(e, &q)
}

struct Target {
    center: V3,
    normal: V3,
    size: f64,
}

fn target_of(x: &Ctx3, cam: &(V3, camsolve::M3, f64), world: &[V3], cw: f64) -> Target {
    let (center, normal) = camsolve::plane_fit(world, cam.0);
    let normal = if world.len() < 3 { camsolve::unit(camsolve::sub(cam.0, center)) } else { normal };
    let fwd = camsolve::col3(&cam.1, 2);
    let depth = camsolve::dot(camsolve::sub(center, cam.0), fwd).max(1e-6);
    let size = x.target_size / 100.0 * 0.15 * cw * depth / cam.2.max(1e-9);
    Target { center, normal, size }
}

pub fn create_from_solve(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = target(e, p)?;
    let kind = ps(p, "kind").unwrap_or("null");
    if !KINDS.contains(&kind) {
        return Err(format!("kind is one of {}", KINDS.join(", ")));
    }
    let x = ctx3(e, cid, lid, uid, None)?;
    let cam_id = ensure_camera(e, cid, lid, uid, &x)?;
    if kind == "camera" {
        e.set_label("Create Camera");
        return Ok(json!({"camera": cam_id, "layers": []}));
    }
    let cw = e.project.comp(cid).map(|c| c.width as f64).ok_or("no composition")?;
    let t = e.time_of(cid);
    let k = frame_at(e, cid, lid, &x.sv, t)?;
    let cam = x.placed.camera(&x.sv.cams[k]);
    let vis = visible(&x, k);
    let ids = chosen(e, p);
    let mut targets: Vec<Target> = vec![];
    if let Some(tg) = p.get("target").filter(|v| v.is_object()) {
        let v3 = |k: &str| tg.get(k).and_then(Json::as_array).and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?]));
        let center = v3("center").ok_or("target.center is [x, y, z]")?;
        targets.push(Target { center, normal: camsolve::unit(v3("normal").unwrap_or_else(|| camsolve::sub(cam.0, center))), size: tg.get("size").and_then(Json::as_f64).unwrap_or(100.0) });
    } else {
        let world: Vec<V3> = vis.iter().filter(|(id, _)| ids.contains(id)).map(|(_, w)| *w).collect();
        if world.is_empty() {
            return Err("select track points first (camera.selectPoints with ids from camera.points)".into());
        }
        if pb(p, "multiple").unwrap_or(false) {
            for w in &world {
                targets.push(target_of(&x, &cam, std::slice::from_ref(w), cw));
            }
        } else {
            targets.push(target_of(&x, &cam, &world, cw));
        }
    }
    let right = camsolve::col3(&cam.1, 0);
    let mut made = vec![];
    for (i, tg) in targets.iter().enumerate() {
        let (ax, ay, az) = camsolve::plane_basis(tg.normal, right);
        let orient = camsolve::euler_xyz(&[[ax[0], ay[0], az[0]], [ax[1], ay[1], az[1]], [ax[2], ay[2], az[2]]]);
        let n = i + 1;
        let id = match kind {
            "text" => super::layer::new_text(e, &json!({"comp": cid, "text": "Text", "name": format!("Track Text {n}")}))?,
            "null" => super::layer::new_null(e, &json!({"comp": cid, "name": format!("Track Null {n}")}))?,
            "shadowCatcher" => {
                let s = (tg.size * 6.0).round().max(16.0);
                super::layer::new_solid(e, &json!({"comp": cid, "name": "Shadow Catcher", "width": s, "height": s, "color": [1.0, 1.0, 1.0]}))?
            }
            _ => {
                let s = tg.size.round().max(4.0);
                super::layer::new_solid(e, &json!({"comp": cid, "name": format!("Track Solid {n}"), "width": s, "height": s, "color": [0.6, 0.6, 0.6]}))?
            }
        };
        let lay = id.get("layer").and_then(Json::as_u64).ok_or("could not make the layer")?;
        let l = e.layer_mut(cid, lay)?;
        l.switches.three_d = true;
        if let Some(tr) = l.props.sub_mut("transform") {
            if let Some(pr) = tr.get_mut("position") {
                pr.value = Value::Vec3(tg.center);
            }
            if let Some(pr) = tr.get_mut("orientation") {
                pr.value = Value::Vec3(orient);
            }
            if kind == "text"
                && let Some(pr) = tr.get_mut("scale")
            {
                let k = (tg.size / 150.0 * 100.0).max(1.0);
                pr.value = Value::Vec3([k, k, k]);
            }
        }
        if kind == "shadowCatcher"
            && let Some(g) = l.props.sub_mut("materialOptions")
        {
            if let Some(pr) = g.get_mut("acceptsShadows") {
                pr.value = Value::Enum(2);
            }
            if let Some(pr) = g.get_mut("castsShadows") {
                pr.value = Value::Enum(0);
            }
        }
        made.push(lay);
    }
    if kind == "shadowCatcher" && !e.project.comp(cid).is_some_and(|c| c.layers.iter().any(|l| l.kind == crate::model::LayerKind::Light)) {
        let tg = &targets[0];
        let up = tg.normal;
        let pos = camsolve::add(camsolve::add(tg.center, camsolve::scale(up, tg.size * 4.0)), camsolve::scale(right, tg.size * 2.0));
        let r = super::layer::new_light(e, &json!({"comp": cid, "name": "Shadow Light", "kind": "point", "castsShadows": true, "position": pos}))?;
        if let Some(id) = r.get("layer").and_then(Json::as_u64) {
            made.push(id);
        }
    }
    super::select_only(e, &made);
    e.set_label(match kind {
        "text" => "Create Text",
        "solid" => "Create Solid",
        "null" => "Create Null",
        _ => "Create Shadow Catcher",
    });
    Ok(json!({"camera": cam_id, "layers": made}))
}

pub fn set_ground(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = target(e, p)?;
    let x = ctx3(e, cid, lid, uid, Some(Ground::default()))?;
    let t = e.time_of(cid);
    let k = frame_at(e, cid, lid, &x.sv, t)?;
    let cam = x.placed.camera(&x.sv.cams[k]);
    let ids = chosen(e, p);
    let world: Vec<V3> = visible(&x, k).into_iter().filter(|(id, _)| ids.contains(id)).map(|(_, w)| w).collect();
    if world.len() < 3 {
        return Err("select at least three track points on the ground".into());
    }
    let (c, n) = camsolve::plane_fit(&world, cam.0);
    let g = camsolve::ground_from(c, n, camsolve::col3(&cam.1, 0));
    let enc = serde_json::to_string(&g).map_err(|e| e.to_string())?;
    {
        let grp = e.layer_mut(cid, lid)?.props.find_group_mut(uid).ok_or("no 3D Camera Tracker")?;
        set_param(grp, "ground", Value::Str(enc));
    }
    refresh_camera(e, cid, lid, uid)?;
    e.set_label("Set Ground Plane and Origin");
    Ok(json!({"origin": c, "normal": n, "points": world.len()}))
}

pub fn delete_points(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = target(e, p)?;
    let ids = chosen(e, p);
    if ids.is_empty() {
        return Err("select the track points to delete".into());
    }
    e.apply_step("Delete Track Points", |e| {
        let g = e.layer_mut(cid, lid)?.props.find_group_mut(uid).ok_or("no 3D Camera Tracker")?;
        let mut del = deleted_of(g);
        for id in &ids {
            if !del.contains(id) {
                del.push(*id);
            }
        }
        del.sort_unstable();
        let s: Vec<String> = del.iter().map(|d| d.to_string()).collect();
        set_param(g, "deleted", Value::Str(s.join(",")));
        Ok(json!({}))
    })?;
    e.state.solve_points.retain(|x| !ids.contains(x));
    let r = analyze(e, &json!({"comp": cid, "layer": lid, "effect": uid, "wait": pb(p, "wait").unwrap_or(false)}))?;
    Ok(json!({"deleted": ids, "analysis": r}))
}
