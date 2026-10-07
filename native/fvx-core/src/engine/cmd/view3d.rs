use std::collections::BTreeMap;

use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pb, pf, ps};
use crate::eval::{Camera, Eval};
use crate::model::{Layer, Value};

pub const VIEWS: [(&str, &str, &str); 11] = [
    ("activeCamera", "Active Camera", "ActiveCamera"),
    ("default", "Default", "Default"),
    ("front", "Front", "Front"),
    ("left", "Left", "Left"),
    ("top", "Top", "Top"),
    ("back", "Back", "Back"),
    ("right", "Right", "Right"),
    ("bottom", "Bottom", "Bottom"),
    ("custom1", "Custom View 1", "CustomView1"),
    ("custom2", "Custom View 2", "CustomView2"),
    ("custom3", "Custom View 3", "CustomView3"),
];

#[derive(Clone, Copy, Debug)]
pub struct ViewCam {
    pub eye: [f64; 3],
    pub poi: [f64; 3],
    pub zoom: f64,
}

#[derive(Clone, Debug, Default)]
pub struct View3d {
    pub current: String,
    pub last: String,
    pub cams: BTreeMap<String, ViewCam>,
}

pub fn normalize(v: &str) -> Option<&'static str> {
    let flat: String = v.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase();
    let flat = flat.replace("view", "");
    VIEWS.iter().find(|(id, label, _)| id.to_ascii_lowercase() == flat || label.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase().replace("view", "") == flat).map(|v| v.0)
}

pub fn is_ortho(view: &str) -> bool {
    matches!(view, "front" | "left" | "top" | "back" | "right" | "bottom")
}

pub fn state_name(view: &str) -> &'static str {
    VIEWS.iter().find(|v| v.0 == view).map(|v| v.2).unwrap_or("ActiveCamera")
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn len(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn norm(a: [f64; 3]) -> [f64; 3] {
    let l = len(a);
    if l < 1e-12 { [0.0, 0.0, 1.0] } else { scale(a, 1.0 / l) }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn rotate(v: [f64; 3], axis: [f64; 3], deg: f64) -> [f64; 3] {
    let k = norm(axis);
    let (s, c) = deg.to_radians().sin_cos();
    let kv = cross(k, v);
    let kd = k[0] * v[0] + k[1] * v[1] + k[2] * v[2];
    [v[0] * c + kv[0] * s + k[0] * kd * (1.0 - c), v[1] * c + kv[1] * s + k[1] * kd * (1.0 - c), v[2] * c + kv[2] * s + k[2] * kd * (1.0 - c)]
}

pub fn default_cam(view: &str, cw: f64, ch: f64) -> ViewCam {
    let zoom = cw * 50.0 / 36.0;
    let c = [cw / 2.0, ch / 2.0, 0.0];
    let d = zoom * 2.0;
    let ortho = |dir: [f64; 3]| ViewCam { eye: add(c, scale(dir, d)), poi: c, zoom: 0.5 };
    match view {
        "front" => ortho([0.0, 0.0, -1.0]),
        "back" => ortho([0.0, 0.0, 1.0]),
        "left" => ortho([-1.0, 0.0, 0.0]),
        "right" => ortho([1.0, 0.0, 0.0]),
        "top" => ortho([0.0, -1.0, 0.0]),
        "bottom" => ortho([0.0, 1.0, 0.0]),
        "custom1" | "custom2" | "custom3" => {
            let (yaw, pitch) = match view {
                "custom1" => (-35.0, 25.0),
                "custom2" => (0.0, 35.0),
                _ => (35.0, 25.0),
            };
            let base = [0.0, 0.0, -zoom * 1.6];
            let v = rotate(rotate(base, [1.0, 0.0, 0.0], -pitch), [0.0, 1.0, 0.0], yaw);
            ViewCam { eye: add(c, v), poi: c, zoom }
        }
        _ => ViewCam { eye: [cw / 2.0, ch / 2.0, -zoom], poi: c, zoom },
    }
}

pub fn camera_of(view: &str, vc: &ViewCam) -> Camera {
    Camera::looking(vc.eye, vc.poi, vc.zoom, is_ortho(view).then_some(vc.zoom))
}

pub fn current(e: &Engine, cid: u64) -> &'static str {
    e.state.views3d.get(&cid).and_then(|v| normalize(&v.current)).unwrap_or("activeCamera")
}

fn view_cam(e: &Engine, cid: u64, view: &str) -> ViewCam {
    let comp = e.project.comp(cid);
    let (cw, ch) = comp.map(|c| (c.width as f64, c.height as f64)).unwrap_or((1920.0, 1080.0));
    e.state.views3d.get(&cid).and_then(|v| v.cams.get(view)).copied().unwrap_or_else(|| default_cam(view, cw, ch))
}

pub fn view_camera(e: &Engine, cid: u64, view: &str) -> Option<Camera> {
    let view = normalize(view)?;
    if view == "activeCamera" {
        return None;
    }
    Some(camera_of(view, &view_cam(e, cid, view)))
}

fn set_cam(e: &mut Engine, cid: u64, view: &str, vc: ViewCam) {
    e.state.views3d.entry(cid).or_default().cams.insert(view.to_string(), vc);
    e.revision += 1;
}

pub fn set_view(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let v = ps(p, "view").ok_or("pass `view`: activeCamera, default, front, left, top, back, right, bottom, custom1, custom2 or custom3")?;
    let view = normalize(v).ok_or_else(|| format!("no 3D view `{v}`"))?;
    switch(e, cid, view)
}

fn switch(e: &mut Engine, cid: u64, view: &str) -> Res {
    let st = e.state.views3d.entry(cid).or_default();
    let prev = normalize(&st.current).unwrap_or("activeCamera");
    if prev != view {
        st.last = prev.to_string();
    }
    st.current = view.to_string();
    e.revision += 1;
    Ok(json!({"view": view, "label": VIEWS.iter().find(|x| x.0 == view).map(|x| x.1)}))
}

macro_rules! view_cmds {
    ($($f:ident => $v:expr),* $(,)?) => {
        $(pub fn $f(e: &mut Engine, p: &Json) -> Res {
            let cid = e.comp_id(p.get("comp"))?;
            switch(e, cid, $v)
        })*
    };
}

view_cmds! {
    v_active => "activeCamera",
    v_default => "default",
    v_front => "front",
    v_left => "left",
    v_top => "top",
    v_back => "back",
    v_right => "right",
    v_bottom => "bottom",
    v_custom1 => "custom1",
    v_custom2 => "custom2",
    v_custom3 => "custom3",
}

pub fn v_last(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let last = e.state.views3d.get(&cid).and_then(|v| normalize(&v.last)).unwrap_or("activeCamera");
    switch(e, cid, last)
}

pub fn reset(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let view = ps(p, "view").and_then(normalize).unwrap_or_else(|| current(e, cid));
    if let Some(st) = e.state.views3d.get_mut(&cid) {
        st.cams.remove(view);
    }
    e.revision += 1;
    Ok(json!({"view": view}))
}

fn v3(p: &Json, k: &str) -> Option<[f64; 3]> {
    let a = p.get(k)?.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2).and_then(Json::as_f64).unwrap_or(0.0)])
}

pub fn set_view_camera(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let view = ps(p, "view").and_then(normalize).unwrap_or_else(|| current(e, cid));
    if view == "activeCamera" {
        return Err("the Active Camera view follows the camera layer; move the camera instead".into());
    }
    let mut vc = view_cam(e, cid, view);
    if let Some(v) = v3(p, "eye") {
        vc.eye = v;
    }
    if let Some(v) = v3(p, "poi") {
        vc.poi = v;
    }
    if let Some(z) = pf(p, "zoom") {
        vc.zoom = z.max(1e-3);
    }
    set_cam(e, cid, view, vc);
    Ok(json!({"view": view, "eye": vc.eye, "poi": vc.poi, "zoom": vc.zoom}))
}

pub fn get(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let view = current(e, cid);
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
    let cam_layer = ev.camera_layer().map(|l| l.id);
    let lights: Vec<Json> = comp
        .layers
        .iter()
        .filter(|l| l.is_light())
        .map(|l| json!({"layer": l.id, "name": l.name, "kind": l.props.sub("lightOptions").and_then(|g| g.get("lightType")).map(|p| crate::model::prop::LIGHT_TYPES.get(p.value.index() as usize).copied().unwrap_or("Point")).unwrap_or("Point")}))
        .collect();
    let camera = if view == "activeCamera" {
        let c = ev.camera();
        json!({"eye": c.eye, "forward": c.forward, "zoom": c.zoom, "ortho": false})
    } else {
        let vc = view_cam(e, cid, view);
        json!({"eye": vc.eye, "poi": vc.poi, "zoom": vc.zoom, "ortho": is_ortho(view)})
    };
    Ok(json!({"view": view, "camera": camera, "activeCamera": cam_layer, "lights": lights, "renderer": comp.renderer, "last": e.state.views3d.get(&cid).map(|v| v.last.clone()).unwrap_or_default()}))
}

fn layer_corners(ev: &Eval, l: &Layer) -> Vec<[f64; 3]> {
    if l.is_model() {
        if let Some(b) = crate::render::mesh::model_bounds(ev, l) {
            let w = ev.world_matrix(l);
            return [[b[0], b[1], b[2]], [b[3], b[1], b[2]], [b[0], b[4], b[2]], [b[3], b[4], b[2]], [b[0], b[1], b[5]], [b[3], b[1], b[5]], [b[0], b[4], b[5]], [b[3], b[4], b[5]]].iter().map(|q| w.apply(*q)).collect();
        }
        return vec![ev.world_matrix(l).apply([0.0; 3])];
    }
    let Some(b) = crate::render::comp::content_bounds(ev, l) else {
        return vec![ev.world_matrix(l).apply([0.0; 3])];
    };
    let w = ev.world_matrix(l);
    [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]].iter().map(|q| if l.is_3d() { w.apply([q[0], q[1], 0.0]) } else { let m = ev.layer_to_comp(l).apply(q[0], q[1]); [m[0], m[1], 0.0] }).collect()
}

fn look(e: &mut Engine, p: &Json, all: bool) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let ids: Vec<u64> = if all {
        e.project.comp(cid).ok_or("no composition")?.layers.iter().filter(|l| !l.is_camera() && !l.is_light() && l.switches.video).map(|l| l.id).collect()
    } else {
        let (_, ids) = e.layers_param(p)?;
        ids
    };
    if ids.is_empty() {
        return Err(if all { "the composition has no visible layers".into() } else { "select layers to look at".into() });
    }
    let view = current(e, cid);
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let (cw, ch) = (comp.width as f64, comp.height as f64);
    let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
    let pts: Vec<[f64; 3]> = ids.iter().filter_map(|id| comp.layer(*id)).flat_map(|l| layer_corners(&ev, l)).collect();
    if pts.is_empty() {
        return Err("nothing to frame".into());
    }
    if view == "activeCamera" || !comp.has_3d() {
        let cam = comp.has_3d().then(|| ev.camera());
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for q in &pts {
            let s = match &cam {
                Some(c) => match c.project(cw, ch, *q) {
                    Some(s) => s,
                    None => continue,
                },
                None => [q[0], q[1]],
            };
            x0 = x0.min(s[0]);
            y0 = y0.min(s[1]);
            x1 = x1.max(s[0]);
            y1 = y1.max(s[1]);
        }
        if x1 < x0 {
            return Err("the layers are behind the camera".into());
        }
        let rect = [x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0)];
        e.frontend("view.lookAt", json!({"rect": rect}));
        return Ok(json!({"view": view, "rect": rect}));
    }
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for q in &pts {
        for i in 0..3 {
            lo[i] = lo[i].min(q[i]);
            hi[i] = hi[i].max(q[i]);
        }
    }
    let center = scale(add(lo, hi), 0.5);
    let radius = (len(sub(hi, lo)) * 0.5).max(1.0);
    let mut vc = view_cam(e, cid, view);
    let dir = norm(sub(vc.poi, vc.eye));
    let dist = len(sub(vc.poi, vc.eye)).max(1.0);
    if is_ortho(view) {
        vc.zoom = (cw.min(ch) * 0.45 / radius).clamp(0.01, 50.0);
        vc.poi = center;
        vc.eye = sub(center, scale(dir, dist.max(radius * 4.0)));
    } else {
        let d = (vc.zoom * radius / (cw.min(ch) * 0.45)).max(radius * 1.05);
        vc.poi = center;
        vc.eye = sub(center, scale(dir, d));
    }
    set_cam(e, cid, view, vc);
    Ok(json!({"view": view, "eye": vc.eye, "poi": vc.poi, "zoom": vc.zoom}))
}

pub fn look_selected(e: &mut Engine, p: &Json) -> Res {
    look(e, p, false)
}

pub fn look_all(e: &mut Engine, p: &Json) -> Res {
    look(e, p, true)
}

fn camera_target(e: &Engine, cid: u64, p: &Json) -> Result<Option<u64>, String> {
    let comp = e.project.comp(cid).ok_or("no composition")?;
    if let Some(v) = p.get("layer").filter(|v| !v.is_null()) {
        let id = e.layer_in(comp, v)?;
        return match comp.layer(id) {
            Some(l) if l.is_camera() || l.is_light() => Ok(Some(id)),
            _ => Err("that layer is not a camera or a light".into()),
        };
    }
    if current(e, cid) != "activeCamera" {
        return Ok(None);
    }
    let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
    Ok(ev.camera_layer().map(|l| l.id))
}

fn read_cam_layer(e: &Engine, cid: u64, lid: u64) -> Result<(ViewCam, bool), String> {
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let l = comp.layer(lid).ok_or("no layer")?;
    let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
    let eye = ev.tr_v3(l, "position", [0.0; 3]);
    let two = l.auto_orient == crate::model::AutoOrient::TowardsPointOfInterest;
    let zoom = l.props.sub("cameraOptions").and_then(|g| g.get("zoom")).map(|p| ev.value(l, p).num()).unwrap_or(comp.width as f64 * 50.0 / 36.0);
    let poi = if two {
        ev.tr_v3(l, "poi", [comp.width as f64 / 2.0, comp.height as f64 / 2.0, 0.0])
    } else {
        let c = ev.camera();
        add(eye, scale(c.forward, zoom))
    };
    Ok((ViewCam { eye, poi, zoom }, two))
}

fn write_cam_layer(e: &mut Engine, cid: u64, lid: u64, vc: &ViewCam, two: bool) -> Result<(), String> {
    let now = e.time_of(cid);
    let lt = e.layer_ref(cid, lid)?.layer_time(now);
    let l = e.layer_mut(cid, lid)?;
    let tr = l.props.sub_mut("transform").ok_or("no transform")?;
    if let Some(pr) = tr.get_mut("position") {
        super::prop::set_value_at(pr, Value::Vec3(vc.eye), Some(lt));
    }
    if two && let Some(pr) = tr.get_mut("poi") {
        super::prop::set_value_at(pr, Value::Vec3(vc.poi), Some(lt));
    }
    if !two {
        let d = norm(sub(vc.poi, vc.eye));
        let b = d[0].clamp(-1.0, 1.0).asin().to_degrees();
        let a = (-d[1]).atan2(d[2]).to_degrees();
        if let Some(pr) = tr.get_mut("orientation") {
            super::prop::set_value_at(pr, Value::Vec3([a, b, 0.0]), Some(lt));
        }
    }
    Ok(())
}

fn camera_move(e: &mut Engine, p: &Json, f: impl Fn(&mut ViewCam)) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    match camera_target(e, cid, p)? {
        Some(lid) => {
            let (mut vc, two) = read_cam_layer(e, cid, lid)?;
            f(&mut vc);
            write_cam_layer(e, cid, lid, &vc, two)?;
            if let Some(m) = ps(p, "merge") {
                e.merge_hint = Some(m.to_string());
            }
            Ok(json!({"layer": lid, "eye": vc.eye, "poi": vc.poi}))
        }
        None => {
            let view = current(e, cid);
            if view == "activeCamera" {
                return Err("the composition has no camera; add one with layer.newCamera or switch to a custom 3D view".into());
            }
            let mut vc = view_cam(e, cid, view);
            f(&mut vc);
            set_cam(e, cid, view, vc);
            Ok(json!({"view": view, "eye": vc.eye, "poi": vc.poi, "zoom": vc.zoom}))
        }
    }
}

pub fn orbit(e: &mut Engine, p: &Json) -> Res {
    let yaw = pf(p, "yaw").or_else(|| pf(p, "dx")).unwrap_or(0.0);
    let pitch = pf(p, "pitch").or_else(|| pf(p, "dy")).unwrap_or(0.0);
    camera_move(e, p, move |vc| {
        let off = sub(vc.eye, vc.poi);
        let off = rotate(off, [0.0, -1.0, 0.0], yaw);
        let fwd = norm(scale(off, -1.0));
        let right = norm(cross([0.0, 1.0, 0.0], fwd));
        let mut next = rotate(off, right, pitch);
        let up = norm(next);
        if up[1].abs() > 0.995 {
            next = off;
        }
        vc.eye = add(vc.poi, next);
    })
}

pub fn pan(e: &mut Engine, p: &Json) -> Res {
    let dx = pf(p, "dx").unwrap_or(0.0);
    let dy = pf(p, "dy").unwrap_or(0.0);
    let cid = e.comp_id(p.get("comp"))?;
    let ortho = current(e, cid);
    let k = if is_ortho(ortho) && camera_target(e, cid, p)?.is_none() { 1.0 / view_cam(e, cid, ortho).zoom.max(1e-3) } else { 1.0 };
    camera_move(e, p, move |vc| {
        let fwd = norm(sub(vc.poi, vc.eye));
        let down0 = if fwd[1].abs() > 0.999 { [0.0, 0.0, -fwd[1].signum()] } else { [0.0, 1.0, 0.0] };
        let right = norm(cross(down0, fwd));
        let down = cross(fwd, right);
        let d = add(scale(right, -dx * k), scale(down, -dy * k));
        vc.eye = add(vc.eye, d);
        vc.poi = add(vc.poi, d);
    })
}

pub fn dolly(e: &mut Engine, p: &Json) -> Res {
    let amount = pf(p, "amount").unwrap_or(0.0);
    let cid = e.comp_id(p.get("comp"))?;
    let view = current(e, cid);
    if is_ortho(view) && camera_target(e, cid, p)?.is_none() {
        let mut vc = view_cam(e, cid, view);
        vc.zoom = (vc.zoom * (amount / 400.0).exp()).clamp(0.01, 50.0);
        set_cam(e, cid, view, vc);
        return Ok(json!({"view": view, "zoom": vc.zoom}));
    }
    camera_move(e, p, move |vc| {
        let off = sub(vc.poi, vc.eye);
        let d = len(off);
        let step = amount.min(d - 1.0);
        vc.eye = add(vc.eye, scale(norm(off), step));
    })
}

pub fn from_view(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let view = current(e, cid);
    if view == "activeCamera" {
        return Err("switch to another 3D view first; Create Camera from 3D View copies that view".into());
    }
    let vc = view_cam(e, cid, view);
    let zoom = if is_ortho(view) { e.project.comp(cid).map(|c| c.width as f64 * 50.0 / 36.0).unwrap_or(2666.7) } else { vc.zoom };
    let r = super::layer::new_camera(e, &json!({"comp": cid, "zoom": zoom, "name": format!("{} Camera", VIEWS.iter().find(|v| v.0 == view).map(|v| v.1).unwrap_or("View"))}))?;
    let lid = r.get("layer").and_then(Json::as_u64).ok_or("no camera")?;
    let l = e.layer_mut(cid, lid)?;
    if let Some(tr) = l.props.sub_mut("transform") {
        if let Some(pr) = tr.get_mut("position") {
            pr.value = Value::Vec3(vc.eye);
        }
        if let Some(pr) = tr.get_mut("poi") {
            pr.value = Value::Vec3(vc.poi);
        }
    }
    if pb(p, "switch").unwrap_or(true) {
        switch(e, cid, "activeCamera")?;
    }
    Ok(json!({"layer": lid}))
}
