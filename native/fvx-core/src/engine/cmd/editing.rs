use std::path::Path;

use serde_json::{Value as Json, json};

use super::select_only;
use crate::engine::jobs::{self, Apply};
use crate::engine::{Clip, Engine, Res, pb, pf, ps, pu};
use crate::eval::Eval;
use crate::model::path::BezPath;
use crate::model::prop::{Expr, Group, GroupKind, Node};
use crate::model::{ItemKind, Layer, LayerKind, Matte, MatteKind, Value};

fn go(e: &mut Engine, cid: u64, t: f64) -> Res {
    super::edit::time_set(e, &json!({"comp": cid, "time": t}))
}

fn now(e: &Engine, cid: u64) -> f64 {
    e.time_of(cid)
}

fn first_layer(e: &Engine, cid: u64) -> Result<u64, String> {
    e.state.selected_layers.iter().copied().find(|l| e.project.comp(cid).is_some_and(|c| c.layer(*l).is_some())).ok_or_else(|| "select a layer".to_string())
}

pub fn layer_in(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let l = e.layer_ref(cid, first_layer(e, cid)?)?.in_point;
    go(e, cid, l)
}

pub fn layer_out(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let c = e.project.comp(cid).ok_or("no composition")?;
    let fd = c.frame_duration();
    let l = e.layer_ref(cid, first_layer(e, cid)?)?.out_point;
    go(e, cid, l - fd)
}

pub fn step(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let fd = e.project.comp(cid).ok_or("no composition")?.frame_duration();
    let n = p.get("frames").and_then(Json::as_f64).ok_or("pass `frames` (negative steps back)")?;
    let t = now(e, cid) + n * fd;
    go(e, cid, t)
}

fn key_times(e: &Engine, cid: u64, p: &Json) -> Vec<f64> {
    let Some(c) = e.project.comp(cid) else { return vec![] };
    let only: Option<Vec<(u64, u64)>> = p.get("visible").and_then(Json::as_array).map(|a| a.iter().filter_map(|v| Some((v.get("layer")?.as_u64()?, v.get("prop")?.as_u64()?))).collect());
    let mut out: Vec<f64> = c.markers.iter().map(|m| m.time).collect();
    for l in &c.layers {
        out.extend(l.markers.iter().map(|m| l.comp_time(m.time)));
        l.props.for_each_prop(&mut |pr| {
            if only.as_ref().is_none_or(|o| o.contains(&(l.id, pr.uid))) {
                out.extend(pr.keys.iter().map(|k| l.comp_time(k.time)));
            }
        });
    }
    out.sort_by(f64::total_cmp);
    out
}

pub fn next_key(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let t = now(e, cid);
    let hit = key_times(e, cid, p).into_iter().find(|k| *k > t + 1e-6).ok_or("no later keyframe or marker")?;
    go(e, cid, hit)
}

pub fn previous_key(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let t = now(e, cid);
    let hit = key_times(e, cid, p).into_iter().rev().find(|k| *k < t - 1e-6).ok_or("no earlier keyframe or marker")?;
    go(e, cid, hit)
}

fn select_step(e: &mut Engine, p: &Json, dir: i64) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let c = e.project.comp(cid).ok_or("no composition")?;
    if c.layers.is_empty() {
        return Err("the composition has no layers".into());
    }
    let n = c.layers.len() as i64;
    let cur = e.state.selected_layers.iter().filter_map(|l| c.index_of(*l)).map(|i| i as i64);
    let base = if dir > 0 { cur.max().unwrap_or(-1) } else { cur.min().unwrap_or(n) };
    let i = (base + dir).rem_euclid(n) as usize;
    let id = c.layers[i].id;
    let mut sel = if pb(p, "add").unwrap_or(false) { e.state.selected_layers.clone() } else { vec![] };
    if !sel.contains(&id) {
        sel.push(id);
    }
    select_only(e, &sel);
    Ok(json!({"layer": id, "index": i + 1}))
}

pub fn select_next(e: &mut Engine, p: &Json) -> Res {
    select_step(e, p, 1)
}

pub fn select_previous(e: &mut Engine, p: &Json) -> Res {
    select_step(e, p, -1)
}

pub fn hide_other_video(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select the layers to keep visible".into());
    }
    let mut n = 0;
    for l in e.proj().comp_mut(cid).ok_or("no composition")?.layers.iter_mut() {
        if !ids.contains(&l.id) && l.switches.video {
            l.switches.video = false;
            n += 1;
        }
    }
    Ok(json!({"hidden": n}))
}

pub fn show_all_video(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let mut n = 0;
    for l in e.proj().comp_mut(cid).ok_or("no composition")?.layers.iter_mut() {
        if !l.switches.video && !matches!(l.kind, LayerKind::Camera | LayerKind::Light) {
            l.switches.video = true;
            n += 1;
        }
    }
    Ok(json!({"shown": n}))
}

pub fn unlock_all(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let mut n = 0;
    for l in e.proj().comp_mut(cid).ok_or("no composition")?.layers.iter_mut() {
        if l.switches.locked {
            l.switches.locked = false;
            n += 1;
        }
    }
    Ok(json!({"unlocked": n}))
}

pub fn sampling(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let s = match ps(p, "sampling").ok_or("pass `sampling` (bilinear or bicubic)")?.to_ascii_lowercase().as_str() {
        "bilinear" => "Bilinear",
        "bicubic" => "Bicubic",
        other => return Err(format!("`{other}` is not a sampling method (bilinear, bicubic)")),
    };
    for id in &ids {
        e.layer_mut(cid, *id)?.switches.sampling = s.into();
    }
    Ok(json!({"layers": ids, "sampling": s}))
}

pub fn delete_all_markers(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let mut n = 0;
    let mut locked = 0;
    for id in &ids {
        let l = e.layer_mut(cid, *id)?;
        if l.switches.markers_locked {
            locked += 1;
            continue;
        }
        n += l.markers.len();
        l.markers.clear();
    }
    if locked > 0 && locked == ids.len() {
        return Err("the markers on these layers are locked".into());
    }
    Ok(json!({"deleted": n, "locked": locked}))
}

pub fn markers_lock(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let first = ids.first().ok_or("select a layer")?;
    let v = pb(p, "value").unwrap_or(!e.layer_ref(cid, *first)?.switches.markers_locked);
    for id in &ids {
        e.layer_mut(cid, *id)?.switches.markers_locked = v;
    }
    Ok(json!({"locked": v, "layers": ids}))
}

pub fn update_markers_from_source(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let mut n = 0;
    for id in ids {
        let l = e.layer_ref(cid, id)?;
        let Some(src) = l.source.and_then(|s| e.project.comp(s)) else { continue };
        let fresh: Vec<_> = src.markers.clone();
        let l = e.layer_mut(cid, id)?;
        if l.switches.markers_locked {
            continue;
        }
        for m in fresh {
            if !l.markers.iter().any(|x| (x.time - m.time).abs() < 1e-6 && x.comment == m.comment) {
                l.markers.push(m);
                n += 1;
            }
        }
        l.markers.sort_by(|a, b| a.time.total_cmp(&b.time));
    }
    Ok(json!({"added": n}))
}

pub fn hide_locked_masks(e: &mut Engine, p: &Json) -> Res {
    let v = pb(p, "value").unwrap_or(!e.state.hide_locked_masks);
    e.state.hide_locked_masks = v;
    Ok(json!({"hideLockedMasks": v}))
}

fn slip_by(e: &mut Engine, p: &Json, d: f64) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let mut done = vec![];
    for id in ids {
        let l = e.layer_mut(cid, id)?;
        if !matches!(l.kind, LayerKind::Footage | LayerKind::Precomp) {
            continue;
        }
        l.start_time += d;
        done.push(id);
    }
    if done.is_empty() {
        return Err("slip edits apply to footage and precomp layers".into());
    }
    e.set_label("Slip Edit");
    Ok(json!({"layers": done, "delta": d}))
}

pub fn slip(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let fd = e.project.comp(cid).ok_or("no composition")?.frame_duration();
    let d = pf(p, "delta").or_else(|| pf(p, "frames").map(|f| f * fd)).ok_or("pass `delta` (s) or `frames`")?;
    slip_by(e, p, d)
}

pub fn slip_back(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let fd = e.project.comp(cid).ok_or("no composition")?.frame_duration();
    slip_by(e, p, -fd)
}

pub fn slip_forward(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let fd = e.project.comp(cid).ok_or("no composition")?.frame_duration();
    slip_by(e, p, fd)
}

pub fn track_matte(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let op = ps(p, "op").ok_or("pass `op` (none, alpha, alphaInverted, luma, lumaInverted, above, below)")?;
    let mut out = vec![];
    for id in ids {
        let c = e.project.comp(cid).ok_or("no composition")?;
        let i = c.index_of(id).ok_or("no such layer")?;
        let cur = c.layers[i].track_matte;
        let above = i.checked_sub(1).map(|j| c.layers[j].id);
        let below = c.layers.get(i + 1).map(|l| l.id);
        let kind = cur.map(|m| m.kind).unwrap_or(MatteKind::Alpha);
        let src = cur.map(|m| m.layer).or(above);
        let m = match op {
            "none" => None,
            "alpha" => src.map(|l| Matte { layer: l, kind: MatteKind::Alpha }),
            "alphaInverted" => src.map(|l| Matte { layer: l, kind: MatteKind::AlphaInverted }),
            "luma" => src.map(|l| Matte { layer: l, kind: MatteKind::Luma }),
            "lumaInverted" => src.map(|l| Matte { layer: l, kind: MatteKind::LumaInverted }),
            "above" => above.map(|l| Matte { layer: l, kind }),
            "below" => below.map(|l| Matte { layer: l, kind }),
            other => return Err(format!("`{other}` is not a track matte operation")),
        };
        if op != "none" && m.is_none() {
            return Err("there is no layer to use as the matte".into());
        }
        if m.is_some_and(|m| m.layer == id) {
            return Err("a layer cannot be its own matte".into());
        }
        e.layer_mut(cid, id)?.track_matte = m;
        if let Some(m) = m
            && let Ok(ml) = e.layer_mut(cid, m.layer)
        {
            ml.switches.video = false;
        }
        out.push(json!({"layer": id, "matte": m.map(|m| m.layer), "kind": m.map(|m| m.kind.label())}));
    }
    e.set_label("Track Matte");
    Ok(json!(out))
}

fn set_xy(l: &mut Layer, id: &str, f: impl Fn([f64; 3]) -> [f64; 3]) {
    if let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut(id)) {
        pr.value = match &pr.value {
            Value::Vec3(v) => Value::Vec3(f(*v)),
            Value::Vec2(v) => {
                let r = f([v[0], v[1], 0.0]);
                Value::Vec2([r[0], r[1]])
            }
            v => v.clone(),
        };
        pr.keys.clear();
    }
}

fn get_xy(ev: &Eval, l: &Layer, id: &str) -> [f64; 3] {
    l.props.sub("transform").and_then(|t| t.get(id)).map(|pr| ev.value(l, pr).vec3()).unwrap_or([0.0; 3])
}

pub fn transform(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let op = ps(p, "op").ok_or("pass `op` (reset, center, fit, fitWidth, fitHeight, flipH, flipV)")?.to_string();
    let c = e.project.comp(cid).ok_or("no composition")?.clone();
    let (cw, ch) = (c.width as f64, c.height as f64);
    for id in &ids {
        let l = c.layer(*id).ok_or("no such layer")?;
        let (sw, sh) = e.project.source_size(l, &c);
        let l = e.layer_mut(cid, *id)?;
        match op.as_str() {
            "reset" => {
                set_xy(l, "anchor", |v| [sw / 2.0, sh / 2.0, v[2] * 0.0]);
                set_xy(l, "position", |_| [cw / 2.0, ch / 2.0, 0.0]);
                set_xy(l, "scale", |_| [100.0, 100.0, 100.0]);
                for r in ["rotation", "rotationX", "rotationY", "orientation"] {
                    if let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut(r)) {
                        pr.keys.clear();
                        pr.value = match pr.value {
                            Value::Vec3(_) => Value::Vec3([0.0; 3]),
                            _ => Value::Num(0.0),
                        };
                    }
                }
                if let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut("opacity")) {
                    pr.keys.clear();
                    pr.value = Value::Num(100.0);
                }
            }
            "center" => set_xy(l, "position", |v| [cw / 2.0, ch / 2.0, v[2]]),
            "fit" => set_xy(l, "scale", |v| [100.0 * cw / sw.max(1.0), 100.0 * ch / sh.max(1.0), v[2]]),
            "fitWidth" => set_xy(l, "scale", |v| {
                let k = 100.0 * cw / sw.max(1.0);
                [k, k, v[2]]
            }),
            "fitHeight" => set_xy(l, "scale", |v| {
                let k = 100.0 * ch / sh.max(1.0);
                [k, k, v[2]]
            }),
            "flipH" => set_xy(l, "scale", |v| [-v[0], v[1], v[2]]),
            "flipV" => set_xy(l, "scale", |v| [v[0], -v[1], v[2]]),
            other => return Err(format!("`{other}` is not a transform operation")),
        }
        if matches!(op.as_str(), "fit" | "fitWidth" | "fitHeight") {
            set_xy(l, "position", |v| [cw / 2.0, ch / 2.0, v[2]]);
            set_xy(l, "anchor", |v| [sw / 2.0, sh / 2.0, v[2]]);
        }
    }
    e.set_label("Transform");
    Ok(json!({"layers": ids, "op": op}))
}

pub fn center_anchor(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let pj = e.project.clone();
    let c = pj.comp(cid).ok_or("no composition")?;
    let t = now(e, cid);
    let ev = Eval::new(&pj, cid, c, t);
    let mut out = vec![];
    for id in ids {
        let l = c.layer(id).ok_or("no such layer")?;
        let b = match crate::render::comp::content_bounds(&ev, l) {
            Some(b) => b,
            None => {
                let (w, h) = pj.source_size(l, c);
                [0.0, 0.0, w, h]
            }
        };
        let na = [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0];
        let a = get_xy(&ev, l, "anchor");
        let s = get_xy(&ev, l, "scale");
        let r = l.props.sub("transform").and_then(|tr| tr.get("rotation")).map(|pr| ev.value(l, pr).num()).unwrap_or(0.0).to_radians();
        let (dx, dy) = ((na[0] - a[0]) * s[0] / 100.0, (na[1] - a[1]) * s[1] / 100.0);
        let (rx, ry) = (dx * r.cos() - dy * r.sin(), dx * r.sin() + dy * r.cos());
        let lm = e.layer_mut(cid, id)?;
        set_xy(lm, "anchor", |v| [na[0], na[1], v[2]]);
        set_xy(lm, "position", |v| [v[0] + rx, v[1] + ry, v[2]]);
        out.push(json!({"layer": id, "anchor": na}));
    }
    e.set_label("Center Anchor Point in Layer Content");
    Ok(json!(out))
}

fn mask_uid(e: &Engine, cid: u64, lid: u64, v: Option<&Json>) -> Result<u64, String> {
    let l = e.layer_ref(cid, lid)?;
    let masks = l.masks().ok_or("this layer has no masks")?;
    let sel = e.state.selected_props.iter().find(|(ly, u)| *ly == lid && masks.groups().any(|g| g.uid == *u)).map(|x| x.1);
    match v {
        Some(Json::Number(n)) => {
            let n = n.as_u64().unwrap_or(0);
            if masks.groups().any(|g| g.uid == n) { Ok(n) } else { masks.groups().nth((n as usize).wrapping_sub(1)).map(|g| g.uid).ok_or_else(|| "no such mask".into()) }
        }
        Some(Json::String(s)) => masks.groups().find(|g| g.name == *s).map(|g| g.uid).ok_or_else(|| format!("no mask named {s}")),
        _ => sel.or_else(|| masks.groups().next().map(|g| g.uid)).ok_or_else(|| "the layer has no masks".into()),
    }
}

pub fn mask_remove(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = mask_uid(e, cid, lid, p.get("mask"))?;
    let l = e.layer_mut(cid, lid)?;
    l.props.sub_mut("masks").ok_or("no masks")?.children.retain(|c| c.uid() != uid);
    e.state.selected_props.retain(|(_, u)| *u != uid);
    e.set_label("Remove Mask");
    Ok(json!({"removed": uid}))
}

pub fn mask_remove_all(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.sub_mut("masks").ok_or("this layer has no masks")?;
    let n = g.children.len();
    g.children.clear();
    e.set_label("Remove All Masks");
    Ok(json!({"removed": n}))
}

fn path_target(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<u64, String> {
    let l = e.layer_ref(cid, lid)?;
    if l.masks().is_some_and(|m| !m.children.is_empty()) && (p.get("mask").is_some() || l.kind != LayerKind::Shape) {
        let m = mask_uid(e, cid, lid, p.get("mask"))?;
        return l.props.find_group(m).and_then(|g| g.get("path")).map(|pr| pr.uid).ok_or_else(|| "the mask has no path".into());
    }
    let sel = e.state.selected_props.iter().filter(|(ly, _)| *ly == lid).map(|x| x.1);
    for u in sel {
        if let Some(pr) = l.props.find(u)
            && matches!(pr.value, Value::Path(_))
        {
            return Ok(u);
        }
        if let Some(g) = l.props.find_group(u)
            && let Some(pr) = g.get("path").filter(|pr| matches!(pr.value, Value::Path(_)))
        {
            return Ok(pr.uid);
        }
    }
    let mut found = None;
    l.props.for_each_prop(&mut |pr| {
        if found.is_none() && matches!(pr.value, Value::Path(_)) {
            found = Some(pr.uid);
        }
    });
    found.ok_or_else(|| "the layer has no mask or shape path".into())
}

fn auto_tangents(bp: &mut BezPath) {
    let n = bp.vertices.len();
    for i in 0..n {
        let prev = if i == 0 { if bp.closed { bp.vertices[n - 1] } else { bp.vertices[0] } } else { bp.vertices[i - 1] };
        let next = if i + 1 == n { if bp.closed { bp.vertices[0] } else { bp.vertices[n - 1] } } else { bp.vertices[i + 1] };
        let d = [(next[0] - prev[0]) / 6.0, (next[1] - prev[1]) / 6.0];
        bp.ins[i] = [-d[0], -d[1]];
        bp.outs[i] = d;
    }
}

pub fn roto_bezier(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = path_target(e, cid, lid, p)?;
    let on = pb(p, "value").unwrap_or(true);
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("no path")?;
    let fix = |bp: &mut BezPath| {
        if on {
            auto_tangents(bp);
        } else {
            for i in 0..bp.vertices.len() {
                bp.ins[i] = [0.0; 2];
                bp.outs[i] = [0.0; 2];
            }
        }
    };
    if let Value::Path(bp) = &mut pr.value {
        fix(bp);
    }
    for k in pr.keys.iter_mut() {
        if let Value::Path(bp) = &mut k.value {
            fix(bp);
        }
    }
    e.set_label("RotoBezier");
    Ok(json!({"path": uid, "rotoBezier": on}))
}

pub fn set_first_vertex(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = path_target(e, cid, lid, p)?;
    let i = pu(p, "index").ok_or("pass `index` (the vertex to make first)")? as usize;
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("no path")?;
    let rot = |bp: &mut BezPath| {
        let n = bp.vertices.len();
        if n == 0 || !bp.closed {
            return;
        }
        let k = i % n;
        bp.vertices.rotate_left(k);
        bp.ins.rotate_left(k);
        bp.outs.rotate_left(k);
    };
    if let Value::Path(bp) = &mut pr.value {
        rot(bp);
    }
    for k in pr.keys.iter_mut() {
        if let Value::Path(bp) = &mut k.value {
            rot(bp);
        }
    }
    e.set_label("Set First Vertex");
    Ok(json!({"path": uid, "first": i}))
}

pub fn convert_to_bezier(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let pj = e.project.clone();
    let c = pj.comp(cid).ok_or("no composition")?;
    let l = c.layer(lid).ok_or("no such layer")?;
    if l.kind != LayerKind::Shape || p.get("mask").is_some() {
        return roto_bezier(e, &json!({"comp": cid, "layer": lid, "mask": p.get("mask"), "value": false}));
    }
    let ev = Eval::new(&pj, cid, c, now(e, cid));
    let sel: Vec<u64> = match pu(p, "prop") {
        Some(u) => vec![u],
        None => e.state.selected_props.iter().filter(|(ly, _)| *ly == lid).map(|x| x.1).collect(),
    };
    let mut targets = vec![];
    fn walk(g: &Group, out: &mut Vec<u64>, sel: &[u64], all: bool) {
        for ch in &g.children {
            if let Node::Group(s) = ch {
                let hit = all || sel.contains(&s.uid);
                if matches!(s.id.as_str(), "rect" | "ellipse" | "star") && hit {
                    out.push(s.uid);
                }
                walk(s, out, sel, hit);
            }
        }
    }
    if let Some(cg) = l.props.sub("contents") {
        walk(cg, &mut targets, &sel, sel.is_empty());
    }
    if targets.is_empty() {
        return Err("no rectangle, ellipse or polystar path to convert".into());
    }
    let mut made = vec![];
    for uid in targets {
        let g = l.props.find_group(uid).ok_or("no shape")?;
        let v = |id: &str| g.get(id).map(|pr| ev.value(l, pr));
        let pos = v("position").map(|x| x.vec2()).unwrap_or([0.0; 2]);
        let bp = match g.id.as_str() {
            "rect" => {
                let s = v("size").map(|x| x.vec2()).unwrap_or([100.0; 2]);
                BezPath::rect(pos[0], pos[1], s[0], s[1], v("roundness").map(|x| x.num()).unwrap_or(0.0))
            }
            "ellipse" => {
                let s = v("size").map(|x| x.vec2()).unwrap_or([100.0; 2]);
                BezPath::ellipse(pos[0], pos[1], s[0], s[1])
            }
            _ => {
                let n = |id: &str, d: f64| v(id).map(|x| x.num()).unwrap_or(d);
                BezPath::star(pos[0], pos[1], n("points", 5.0), n("outerRadius", 50.0), n("innerRadius", 25.0), n("rotation", 0.0), n("outerRoundness", 0.0), n("innerRoundness", 0.0), v("type").map(|x| x.index() == 1).unwrap_or(false))
            }
        };
        let name = g.name.replace("Rectangle Path", "Path").replace("Ellipse Path", "Path").replace("Polystar Path", "Path");
        let pjm = e.proj();
        let mut ids = pjm.ids();
        let mut ng = crate::model::prop::shape_item(&mut ids, "path", Some(&name)).ok_or("no path item")?;
        if let Some(pr) = ng.get_mut("path") {
            pr.value = Value::Path(bp);
        }
        let new_uid = ng.uid;
        let lm = e.layer_mut(cid, lid)?;
        let parent = lm.props.parent_of_mut(uid).ok_or("no parent group")?;
        if let Some(i) = parent.children.iter().position(|c| c.uid() == uid) {
            parent.children[i] = Node::Group(ng);
        }
        made.push(new_uid);
    }
    e.set_label("Convert To Bezier Path");
    Ok(json!({"paths": made}))
}

pub fn group_shapes(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    if l.kind != LayerKind::Shape {
        return Err("Group Shapes works on shape layers".into());
    }
    let items: Vec<u64> = match p.get("items").and_then(Json::as_array) {
        Some(a) => a.iter().filter_map(Json::as_u64).collect(),
        None => e.state.selected_props.iter().filter(|(ly, _)| *ly == lid).map(|x| x.1).collect(),
    };
    if items.is_empty() {
        return Err("select the shape contents to group (or pass `items`)".into());
    }
    let parent_uid = l.props.parent_of(items[0]).map(|g| g.uid).ok_or("no such shape item")?;
    if items.iter().any(|u| l.props.parent_of(*u).map(|g| g.uid) != Some(parent_uid)) {
        return Err("the items to group must sit in the same group".into());
    }
    let pj = e.proj();
    let mut ids = pj.ids();
    let mut grp = crate::model::prop::shape_item(&mut ids, "group", None).ok_or("no group")?;
    let gid = grp.uid;
    let lm = e.layer_mut(cid, lid)?;
    let parent = lm.props.find_group_mut(parent_uid).ok_or("no parent")?;
    let at = parent.children.iter().position(|c| items.contains(&c.uid())).unwrap_or(0);
    let mut moved = vec![];
    parent.children.retain(|c| {
        if items.contains(&c.uid()) {
            moved.push(c.clone());
            false
        } else {
            true
        }
    });
    if let Some(cg) = grp.sub_mut("contents") {
        cg.children = moved;
    }
    let n = parent.children.iter().filter(|c| matches!(c, Node::Group(g) if g.id == "group")).count() + 1;
    grp.name = format!("Group {n}");
    parent.children.insert(at.min(parent.children.len()), Node::Group(grp));
    e.state.selected_props = vec![(lid, gid)];
    e.set_label("Group Shapes");
    Ok(json!({"group": gid, "items": items}))
}

pub fn ungroup_shapes(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let groups: Vec<u64> = match p.get("items").and_then(Json::as_array) {
        Some(a) => a.iter().filter_map(Json::as_u64).collect(),
        None => e.state.selected_props.iter().filter(|(ly, _)| *ly == lid).map(|x| x.1).collect(),
    };
    let lm = e.layer_mut(cid, lid)?;
    let mut n = 0;
    for gid in groups {
        let Some(g) = lm.props.find_group(gid).cloned() else { continue };
        if g.id != "group" {
            continue;
        }
        let kids = g.sub("contents").map(|c| c.children.clone()).unwrap_or_default();
        let Some(parent) = lm.props.parent_of_mut(gid) else { continue };
        if let Some(i) = parent.children.iter().position(|c| c.uid() == gid) {
            parent.children.remove(i);
            for (k, c) in kids.into_iter().enumerate() {
                parent.children.insert(i + k, c);
            }
            n += 1;
        }
    }
    if n == 0 {
        return Err("select a shape group to ungroup".into());
    }
    e.state.selected_props.clear();
    e.set_label("Ungroup Shapes");
    Ok(json!({"ungrouped": n}))
}

pub fn move_group(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = pu(p, "prop").ok_or("pass `prop` (the group's uid)")?;
    let to = pu(p, "index").ok_or("pass `index` (1-based)")?.max(1) as usize - 1;
    let lm = e.layer_mut(cid, lid)?;
    let parent = lm.props.parent_of_mut(uid).ok_or("no such group")?;
    if !matches!(parent.kind, GroupKind::Indexed) {
        return Err("only effects, masks, shape contents, animators and other indexed groups can be reordered".into());
    }
    let i = parent.children.iter().position(|c| c.uid() == uid).ok_or("no such group")?;
    let node = parent.children.remove(i);
    let to = to.min(parent.children.len());
    parent.children.insert(to, node);
    e.set_label("Reorder");
    Ok(json!({"prop": uid, "index": to + 1}))
}

pub fn set_group_enabled(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = pu(p, "prop").ok_or("pass `prop` (the group's uid)")?;
    let lm = e.layer_mut(cid, lid)?;
    let g = lm.props.find_group_mut(uid).ok_or("no such group")?;
    g.enabled = pb(p, "value").unwrap_or(!g.enabled);
    let v = g.enabled;
    Ok(json!({"prop": uid, "enabled": v}))
}

fn link_text(e: &Engine, cid: u64, l: &Layer, uid: u64, relative: bool) -> Option<String> {
    let t = super::expression::reference(l, uid, false)?;
    if relative {
        return Some(t);
    }
    let comp = e.project.item(cid)?.name.clone();
    Some(t.replacen("thisComp", &format!("comp({})", serde_json::to_string(&comp).ok()?), 1))
}

fn copy_links(e: &mut Engine, p: &Json, relative: bool) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let props: Vec<(u64, u64)> = e.state.selected_props.clone();
    if !props.is_empty() && p.get("layers").is_none() {
        let mut links = vec![];
        for (lid, uid) in props {
            let l = e.layer_ref(cid, lid)?;
            if l.props.find(uid).is_none() {
                continue;
            }
            if let (Some(path), Some(text)) = (l.props.path_of(uid), link_text(e, cid, l, uid, relative)) {
                links.push((path, text));
            }
        }
        if links.is_empty() {
            return Err("select properties (not groups) to copy as links".into());
        }
        let n = links.len();
        e.clipboard = Some(Clip::Links(links));
        return Ok(json!({"properties": n}));
    }
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select layers or properties to copy".into());
    }
    let c = e.project.comp(cid).ok_or("no composition")?;
    let mut layers = vec![];
    for l in c.layers.iter().filter(|l| ids.contains(&l.id)) {
        let mut copy = l.clone();
        let mut uids = vec![];
        l.props.for_each_prop(&mut |pr| {
            if pr.value.is_numeric() && !pr.hold_only {
                uids.push(pr.uid);
            }
        });
        for uid in uids {
            if let Some(text) = link_text(e, cid, l, uid, relative)
                && let Some(pr) = copy.props.find_mut(uid)
            {
                pr.expr = Some(Expr { text, enabled: true });
            }
        }
        layers.push(copy);
    }
    let n = layers.len();
    e.clipboard = Some(Clip::Layers(layers, cid));
    Ok(json!({"layers": n}))
}

pub fn copy_with_property_links(e: &mut Engine, p: &Json) -> Res {
    copy_links(e, p, false)
}

pub fn copy_with_relative_property_links(e: &mut Engine, p: &Json) -> Res {
    copy_links(e, p, true)
}

pub fn paste_reversed_keyframes(e: &mut Engine, p: &Json) -> Res {
    let Some(Clip::Keys(keys)) = e.clipboard.clone() else { return Err("copy keyframes first".into()) };
    let span = keys.iter().map(|k| k.1).fold(0.0, f64::max);
    let rev: Vec<_> = keys.into_iter().map(|(path, t, k)| (path, span - t, k)).collect();
    let saved = e.clipboard.replace(Clip::Keys(rev));
    let r = super::edit::paste(e, p);
    e.clipboard = saved;
    e.set_label("Paste Reversed Keyframes");
    r
}

fn clipboard_text(e: &Engine) -> Option<crate::model::TextDoc> {
    match &e.clipboard {
        Some(Clip::Layers(ls, _)) => ls.iter().find_map(|l| l.text_prop().and_then(|p| p.value.as_text().cloned())),
        _ => None,
    }
}

fn text_targets(e: &Engine, p: &Json) -> Result<(u64, Vec<u64>), String> {
    let (cid, ids) = e.layers_param(p)?;
    let ids: Vec<u64> = ids.into_iter().filter(|l| e.layer_ref(cid, *l).is_ok_and(|x| x.kind == LayerKind::Text)).collect();
    if ids.is_empty() {
        return Err("select a text layer".into());
    }
    Ok((cid, ids))
}

fn with_doc(e: &mut Engine, cid: u64, lid: u64, f: impl Fn(&mut crate::model::TextDoc)) -> Result<(), String> {
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.sub_mut("text").and_then(|t| t.get_mut("sourceText")).ok_or("no source text")?;
    if let Value::Text(d) = &mut pr.value {
        f(d);
    }
    for k in pr.keys.iter_mut() {
        if let Value::Text(d) = &mut k.value {
            f(d);
        }
    }
    Ok(())
}

pub fn paste_text_formatting_only(e: &mut Engine, p: &Json) -> Res {
    let src = clipboard_text(e).ok_or("copy a text layer first")?;
    let (cid, ids) = text_targets(e, p)?;
    for id in &ids {
        with_doc(e, cid, *id, |d| {
            let text = std::mem::take(&mut d.text);
            *d = src.clone();
            d.text = text;
            d.runs.clear();
            let n = d.text.chars().count();
            for r in src.runs.iter().filter(|r| r.start < n) {
                d.runs.push(crate::model::StyleRun { start: r.start, end: r.end.min(n), style: r.style.clone() });
            }
        })?;
    }
    e.set_label("Paste Text Formatting Only");
    Ok(json!({"layers": ids}))
}

pub fn paste_text_match_formatting(e: &mut Engine, p: &Json) -> Res {
    let text = match ps(p, "text") {
        Some(t) => t.to_string(),
        None => clipboard_text(e).map(|d| d.text).ok_or("pass `text` or copy a text layer first")?,
    };
    let (cid, ids) = text_targets(e, p)?;
    let caret = e.state.text_edit.as_ref().filter(|t| ids.contains(&t.layer)).map(|t| (t.layer, t.anchor.min(t.caret), t.anchor.max(t.caret)));
    let ins = text.chars().count();
    for id in &ids {
        let range = caret.filter(|c| c.0 == *id).map(|c| (c.1, c.2));
        with_doc(e, cid, *id, |d| {
            let n = d.text.chars().count();
            let (a, b) = range.map(|(a, b)| (a.min(n), b.min(n))).unwrap_or((n, n));
            let chars: Vec<char> = d.text.chars().collect();
            d.text = chars[..a].iter().chain(text.chars().collect::<Vec<_>>().iter()).chain(chars[b..].iter()).collect();
            let removed = b - a;
            for r in d.runs.iter_mut() {
                let shift = |x: usize| if x <= a { x } else if x >= b { x + ins - removed } else { a + ins };
                let (s, en) = (r.start, r.end);
                r.start = shift(s);
                r.end = if en >= a && s <= a { en.max(a) + ins - removed.min(en.saturating_sub(a)) } else { shift(en) };
            }
        })?;
    }
    e.set_label("Paste Text and Match Formatting");
    Ok(json!({"layers": ids, "inserted": ins}))
}

pub fn history(e: &mut Engine, p: &Json) -> Res {
    let undo = pu(p, "steps").unwrap_or(0) as usize;
    let redo = pu(p, "redo").unwrap_or(0) as usize;
    if undo == 0 && redo == 0 {
        let list: Vec<Json> = e.history.undo.iter().enumerate().map(|(i, s)| json!({"index": i, "label": s.label})).collect();
        let redo: Vec<String> = e.history.redo.iter().rev().map(|s| s.label.clone()).collect();
        return Ok(json!({"undo": list, "redo": redo}));
    }
    let mut done = 0;
    for _ in 0..undo {
        if e.undo().is_err() {
            break;
        }
        done += 1;
    }
    let mut redone = 0;
    for _ in 0..redo {
        if e.redo().is_err() {
            break;
        }
        redone += 1;
    }
    Ok(json!({"undone": done, "redone": redone, "undoLeft": e.history.undo.len(), "redoLeft": e.history.redo.len()}))
}

pub fn purge(e: &mut Engine, p: &Json) -> Res {
    let what = ps(p, "what").unwrap_or("all");
    let mut done = vec![];
    if matches!(what, "all" | "memory" | "memoryAndDisk" | "image") {
        e.media.purge();
        done.push("image caches");
    }
    if matches!(what, "all" | "memoryAndDisk" | "disk") {
        crate::render::diskcache::clean();
        done.push("disk cache");
    }
    if matches!(what, "all" | "snapshot") {
        e.state.snapshot = None;
        e.state.show_snapshot = false;
        done.push("snapshot");
    }
    if matches!(what, "all" | "3d") {
        e.media.models.clear();
        done.push("3D models");
    }
    if done.is_empty() {
        return Err(format!("`{what}` is not something to purge (all, memoryAndDisk, memory, disk, 3d, image, snapshot)"));
    }
    e.emit(json!({"type": "purgeCaches"}));
    Ok(json!({"purged": done}))
}

pub fn purge_undo(e: &mut Engine, _: &Json) -> Res {
    let n = e.history.undo.len() + e.history.redo.len();
    e.history.undo.clear();
    e.history.redo.clear();
    e.history.merge = None;
    Ok(json!({"cleared": n}))
}

pub fn select_label_group(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let c = e.project.comp(cid).ok_or("no composition")?;
    let labels: Vec<u8> = e.state.selected_layers.iter().filter_map(|l| c.layer(*l)).map(|l| l.label).collect();
    if labels.is_empty() {
        return Err("select a layer with the label to match".into());
    }
    let ids: Vec<u64> = c.layers.iter().filter(|l| labels.contains(&l.label)).map(|l| l.id).collect();
    select_only(e, &ids);
    Ok(json!({"layers": ids}))
}

pub fn edit_original(e: &mut Engine, p: &Json) -> Res {
    let path = match e.state.selected_layers.first().and_then(|l| e.state.active_comp.and_then(|c| e.layer_ref(c, *l).ok())).and_then(|l| l.source) {
        Some(src) => src,
        None => *e.state.project_selection.first().ok_or("select footage or a footage layer")?,
    };
    let f = match e.project.item(path).map(|i| &i.kind) {
        Some(ItemKind::Footage(f)) if !f.path.is_empty() => f.path.clone(),
        _ => return Err("only footage with a file can be edited in its original application".into()),
    };
    if !Path::new(&f).exists() {
        return Err(format!("{f} is missing"));
    }
    if std::env::var_os("FORMVFX_NO_REVEAL").is_some() || pb(p, "dryRun").unwrap_or(false) {
        return Ok(json!({"path": f, "opened": false}));
    }
    let spawned = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(&f).spawn()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", "", &f]).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(&f).spawn()
    };
    spawned.map_err(|x| format!("could not open {f}: {x}"))?;
    Ok(json!({"path": f, "opened": true}))
}

fn work_area_edit(e: &mut Engine, p: &Json, extract: bool) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let c = e.project.comp(cid).ok_or("no composition")?;
    let [a, b] = c.work_area;
    let ids: Vec<u64> = if ids.is_empty() { c.layers.iter().filter(|l| !l.switches.locked).map(|l| l.id).collect() } else { ids };
    let gap = b - a;
    if gap <= 0.0 {
        return Err("the work area is empty".into());
    }
    let mut cut = 0;
    for id in ids.clone() {
        let l = e.layer_ref(cid, id)?.clone();
        if l.out_point <= a || l.in_point >= b {
            if extract && l.in_point >= b {
                let lm = e.layer_mut(cid, id)?;
                lm.in_point -= gap;
                lm.out_point -= gap;
                lm.start_time -= gap;
            }
            continue;
        }
        cut += 1;
        let head = l.in_point < a;
        let tail = l.out_point > b;
        match (head, tail) {
            (true, true) => {
                let mut t = l.clone();
                let pj = e.proj();
                t.id = pj.new_id();
                t.props.renumber(&mut pj.next_uid);
                t.in_point = if extract { a } else { b };
                t.out_point = l.out_point - if extract { gap } else { 0.0 };
                t.start_time = l.start_time - if extract { gap } else { 0.0 };
                let cm = pj.comp_mut(cid).ok_or("no composition")?;
                let i = cm.index_of(id).unwrap_or(0);
                cm.layers[i].out_point = a;
                cm.layers.insert(i, t);
            }
            (true, false) => e.layer_mut(cid, id)?.out_point = a,
            (false, true) => {
                let lm = e.layer_mut(cid, id)?;
                lm.in_point = b;
                if extract {
                    lm.in_point -= gap;
                    lm.out_point -= gap;
                    lm.start_time -= gap;
                }
            }
            (false, false) => {
                let cm = e.proj().comp_mut(cid).ok_or("no composition")?;
                cm.layers.retain(|x| x.id != id);
            }
        }
    }
    e.set_label(if extract { "Extract Work Area" } else { "Lift Work Area" });
    Ok(json!({"cut": cut, "workArea": [a, b], "closedGap": extract}))
}

pub fn lift_work_area(e: &mut Engine, p: &Json) -> Res {
    work_area_edit(e, p, false)
}

pub fn extract_work_area(e: &mut Engine, p: &Json) -> Res {
    work_area_edit(e, p, true)
}

fn effect_uids(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<Vec<u64>, String> {
    let l = e.layer_ref(cid, lid)?;
    let fx = l.effects().ok_or("this layer has no effects")?;
    let pick = |v: &Json| -> Option<u64> {
        match v {
            Json::Number(n) => {
                let n = n.as_u64()?;
                if fx.groups().any(|g| g.uid == n) { Some(n) } else { fx.groups().nth((n as usize).checked_sub(1)?).map(|g| g.uid) }
            }
            Json::String(s) => fx.groups().find(|g| g.name == *s).map(|g| g.uid),
            _ => None,
        }
    };
    let list: Vec<u64> = match (p.get("effects").and_then(Json::as_array), p.get("effect")) {
        (Some(a), _) => a.iter().filter_map(pick).collect(),
        (None, Some(v)) => pick(v).into_iter().collect(),
        _ => e.state.selected_props.iter().filter(|(ly, u)| *ly == lid && fx.groups().any(|g| g.uid == *u)).map(|x| x.1).collect(),
    };
    if list.is_empty() {
        return Err("select an effect (or pass `effect`)".into());
    }
    Ok(list)
}

pub fn effect_copy(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uids = effect_uids(e, cid, lid, p)?;
    let l = e.layer_ref(cid, lid)?;
    let groups: Vec<Group> = uids.iter().filter_map(|u| l.props.find_group(*u).cloned()).collect();
    let names: Vec<String> = groups.iter().map(|g| g.name.clone()).collect();
    e.session.effect_clip = groups;
    Ok(json!({"effects": names}))
}

fn add_effects(e: &mut Engine, cid: u64, lid: u64, groups: &[Group], after: Option<u64>) -> Result<Vec<u64>, String> {
    let pj = e.proj();
    let mut fresh = vec![];
    for g in groups {
        let mut n = g.clone();
        n.renumber(&mut pj.next_uid);
        fresh.push(Node::Group(n));
    }
    let l = pj.comp_mut(cid).and_then(|c| c.layer_mut(lid)).ok_or("no such layer")?;
    let fx = l.props.sub_mut("effects").ok_or("this layer cannot take effects")?;
    let mut at = after.and_then(|u| fx.children.iter().position(|c| c.uid() == u)).map(|i| i + 1).unwrap_or(fx.children.len());
    let mut ids = vec![];
    for n in fresh {
        ids.push(n.uid());
        fx.children.insert(at, n);
        at += 1;
    }
    Ok(ids)
}

pub fn effect_paste(e: &mut Engine, p: &Json) -> Res {
    if e.session.effect_clip.is_empty() {
        return Err("copy effects first (effect.copy)".into());
    }
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select the layers to paste the effects onto".into());
    }
    let groups = e.session.effect_clip.clone();
    let mut made = vec![];
    for id in ids {
        made.extend(add_effects(e, cid, id, &groups, None)?);
    }
    e.set_label("Paste Effects");
    Ok(json!({"effects": made}))
}

pub fn effect_duplicate(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uids = effect_uids(e, cid, lid, p)?;
    let mut made = vec![];
    for u in uids {
        let l = e.layer_ref(cid, lid)?;
        let Some(mut g) = l.props.find_group(u).cloned() else { continue };
        let names: Vec<String> = l.effects().map(|fx| fx.groups().map(|x| x.name.clone()).collect()).unwrap_or_default();
        let base = g.name.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end().to_string();
        let mut k = 2;
        while names.contains(&format!("{base} {k}")) {
            k += 1;
        }
        g.name = format!("{base} {k}");
        made.extend(add_effects(e, cid, lid, &[g], Some(u))?);
    }
    e.set_label("Duplicate Effect");
    Ok(json!({"effects": made}))
}

pub fn effect_apply_last(e: &mut Engine, p: &Json) -> Res {
    let id = e.session.last_effect.clone().ok_or("no effect has been applied yet")?;
    let mut q = p.clone();
    q["effect"] = json!(id);
    super::effect::apply(e, &q)
}

pub fn effect_pick_color(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let (x, y) = (pf(p, "x").ok_or("pass `x` (layer px)")?, pf(p, "y").ok_or("pass `y` (layer px)")?);
    let t = pf(p, "time").unwrap_or_else(|| now(e, cid));
    let uids = effect_uids(e, cid, lid, &json!({"effect": p.get("effect").cloned().unwrap_or(json!(1))}))?;
    let fx = uids[0];
    let pj = e.project.clone();
    let src = crate::render::analysis::Source::new(&pj, &e.media, cid, lid, 4096.0).ok_or("no layer source")?;
    let img = src.image(t).ok_or("nothing to sample at that time")?;
    let b = src.to_buf([x, y]);
    let r = if pb(p, "average").unwrap_or(false) { 2 } else { 0 };
    let mut acc = [0.0f64; 4];
    let mut n = 0.0;
    for dy in -r..=r {
        for dx in -r..=r {
            let (px, py) = (b[0] as i64 + dx, b[1] as i64 + dy);
            if px < 0 || py < 0 || px as usize >= img.w || py as usize >= img.h {
                continue;
            }
            let i = (py as usize * img.w + px as usize) * 4;
            for k in 0..4 {
                acc[k] += img.px[i + k] as f64;
            }
            n += 1.0;
        }
    }
    if n == 0.0 {
        return Err("that point is outside the layer".into());
    }
    let a = (acc[3] / n).max(1e-6);
    let c = [acc[0] / n / a, acc[1] / n / a, acc[2] / n / a, 1.0];
    let lm = e.layer_mut(cid, lid)?;
    let g = lm.props.find_group_mut(fx).ok_or("no effect")?;
    let target = match (ps(p, "param"), pu(p, "prop")) {
        (Some(id), _) => g.get_mut(id).map(|pr| pr.uid),
        (None, Some(u)) => Some(u),
        (None, None) => {
            let mut found = None;
            for ch in &g.children {
                if let Node::Prop(pr) = ch
                    && matches!(pr.value, Value::Color(_))
                {
                    found = Some(pr.uid);
                    break;
                }
            }
            found
        }
    }
    .ok_or("the effect has no colour parameter of that name")?;
    let pr = lm.props.find_mut(target).ok_or("no such parameter")?;
    if !matches!(pr.value, Value::Color(_)) {
        return Err("that parameter is not a colour".into());
    }
    pr.value = Value::Color(c);
    e.set_label("Pick Effect Colour");
    Ok(json!({"color": c, "prop": target}))
}

pub fn anim_reveal(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let kind = ps(p, "kind").unwrap_or("keyframes").to_string();
    let c = e.project.comp(cid).ok_or("no composition")?;
    let ids: Vec<u64> = match p.get("layers").and_then(Json::as_array) {
        Some(a) => a.iter().filter_map(Json::as_u64).collect(),
        None if !e.state.selected_layers.is_empty() => e.state.selected_layers.clone(),
        None => c.layers.iter().map(|l| l.id).collect(),
    };
    let mut props = vec![];
    for l in c.layers.iter().filter(|l| ids.contains(&l.id)) {
        l.props.for_each_prop(&mut |pr| {
            let hit = match kind.as_str() {
                "keyframes" => !pr.keys.is_empty(),
                "expressions" => pr.expr.is_some(),
                "modified" => false,
                _ => !pr.keys.is_empty() || pr.expr.is_some(),
            };
            if hit {
                props.push(json!({"layer": l.id, "prop": pr.uid}));
            }
        });
    }
    e.frontend("timeline.revealProps", json!({"props": props, "kind": kind, "layers": ids}));
    Ok(json!({"props": props.len(), "kind": kind}))
}

fn null_at(e: &mut Engine, cid: u64, name: &str, pos: [f64; 2]) -> Result<u64, String> {
    let r = super::layer::new_null(e, &json!({"comp": cid, "name": name, "position": pos}))?;
    r.get("layer").and_then(Json::as_u64).ok_or_else(|| "no null".into())
}

fn path_of_layer(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<(u64, String, BezPath), String> {
    let uid = match p.get("path").or_else(|| p.get("prop")) {
        Some(Json::String(s)) => e.layer_ref(cid, lid)?.props.resolve_uid(s).ok_or("no such path")?,
        Some(Json::Number(n)) => n.as_u64().ok_or("bad prop")?,
        _ => path_target(e, cid, lid, p)?,
    };
    let l = e.layer_ref(cid, lid)?;
    let pr = l.props.find(uid).ok_or("no such path")?;
    let pj = &e.project;
    let c = pj.comp(cid).ok_or("no composition")?;
    let ev = Eval::new(pj, cid, c, now(e, cid));
    let bp = ev.value(l, pr).as_path().cloned().ok_or("that property is not a path")?;
    let r = super::expression::reference(l, uid, false).ok_or("cannot write a reference to that path")?;
    Ok((uid, r, bp))
}

fn to_comp_point(e: &Engine, cid: u64, lid: u64, v: [f64; 2]) -> [f64; 2] {
    let pj = &e.project;
    let Some(c) = pj.comp(cid) else { return v };
    let Some(l) = c.layer(lid) else { return v };
    let ev = Eval::new(pj, cid, c, now(e, cid));
    let w = ev.world_matrix(l).apply([v[0], v[1], 0.0]);
    [w[0], w[1]]
}

pub fn nulls_follow_points(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let (_, r, bp) = path_of_layer(e, cid, lid, p)?;
    let lname = e.layer_ref(cid, lid)?.name.clone();
    let mut made = vec![];
    for i in 0..bp.vertices.len() {
        let at = to_comp_point(e, cid, lid, bp.vertices[i]);
        let nid = null_at(e, cid, &format!("{lname}: Point {}", i + 1), at)?;
        let expr = format!("var src = {r};\nvar l = thisComp.layer({});\nl.toComp(src.points()[{i}]);", serde_json::to_string(&lname).unwrap_or_default());
        let nl = e.layer_mut(cid, nid)?;
        if let Some(pr) = nl.props.sub_mut("transform").and_then(|t| t.get_mut("position")) {
            pr.expr = Some(Expr { text: expr, enabled: true });
        }
        made.push(nid);
    }
    e.set_label("Nulls Follow Points");
    Ok(json!({"nulls": made}))
}

pub fn points_follow_nulls(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let (uid, _, bp) = path_of_layer(e, cid, lid, p)?;
    let lname = e.layer_ref(cid, lid)?.name.clone();
    let mut names = vec![];
    for i in 0..bp.vertices.len() {
        let at = to_comp_point(e, cid, lid, bp.vertices[i]);
        let n = format!("{lname}: Point {}", i + 1);
        null_at(e, cid, &n, at)?;
        names.push(n);
    }
    let list = names.iter().map(|n| format!("fromComp(thisComp.layer({}).toComp([0,0]))", serde_json::to_string(n).unwrap_or_default())).collect::<Vec<_>>().join(",\n  ");
    let expr = format!("var pts = [\n  {list}\n];\ncreatePath(pts, inTangents(), outTangents(), isClosed());");
    let lm = e.layer_mut(cid, lid)?;
    let pr = lm.props.find_mut(uid).ok_or("no path")?;
    pr.expr = Some(Expr { text: expr, enabled: true });
    e.set_label("Points Follow Nulls");
    Ok(json!({"nulls": names}))
}

pub fn trace_path(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let (_, r, bp) = path_of_layer(e, cid, lid, p)?;
    let lname = e.layer_ref(cid, lid)?.name.clone();
    let at = to_comp_point(e, cid, lid, bp.vertices.first().copied().unwrap_or([0.0; 2]));
    let nid = null_at(e, cid, &format!("Trace {lname}"), at)?;
    let fx = super::effect::apply(e, &json!({"comp": cid, "layers": [nid], "effect": "Slider Control"}))?;
    let _ = fx;
    let looped = pb(p, "loop").unwrap_or(false);
    let nl = e.layer_mut(cid, nid)?;
    let slider_uid = nl.effects().and_then(|fx| fx.groups().last()).map(|g| g.uid);
    if let Some(su) = slider_uid {
        if let Some(g) = nl.props.find_group_mut(su) {
            g.name = "Progress".into();
        }
        let pr_uid = nl.props.find_group(su).and_then(|g| g.children.iter().find_map(|c| if let Node::Prop(p) = c { Some(p.uid) } else { None }));
        let (in_t, out_t) = (nl.in_point, nl.out_point);
        if let Some(pu_) = pr_uid
            && let Some(pr) = nl.props.find_mut(pu_)
        {
            crate::model::key::insert(&mut pr.keys, crate::model::Key::new(in_t - nl.start_time, Value::Num(0.0), false));
            crate::model::key::insert(&mut pr.keys, crate::model::Key::new((out_t - nl.start_time).min(in_t - nl.start_time + 2.0), Value::Num(100.0), false));
        }
    }
    let progress = if looped { "(effect(\"Progress\")(1) % 100) / 100" } else { "clamp(effect(\"Progress\")(1), 0, 100) / 100" };
    let expr = format!("var src = {r};\nvar l = thisComp.layer({});\nl.toComp(src.pointOnPath({progress}));", serde_json::to_string(&lname).unwrap_or_default());
    let nl = e.layer_mut(cid, nid)?;
    if let Some(pr) = nl.props.sub_mut("transform").and_then(|t| t.get_mut("position")) {
        pr.expr = Some(Expr { text: expr, enabled: true });
    }
    e.set_label("Trace Path");
    Ok(json!({"null": nid}))
}

fn parse_clock(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Ok(x) = s.parse::<f64>() {
        return Some(x);
    }
    if let Some((date, time)) = s.split_once(['T', ' ']) {
        let d: Vec<i64> = date.split('-').filter_map(|x| x.parse().ok()).collect();
        if d.len() == 3 {
            let (y, m, dd) = (d[0], d[1], d[2]);
            let y2 = if m <= 2 { y - 1 } else { y };
            let era = y2.div_euclid(400);
            let yoe = y2 - era * 400;
            let mp = (m + 9) % 12;
            let doy = (153 * mp + 2) / 5 + dd - 1;
            let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
            let days = era * 146097 + doe - 719468;
            let mut t = time.trim_end_matches('Z').to_string();
            let mut tz = 0.0;
            if let Some(i) = t.rfind(['+', '-']).filter(|i| *i > 0) {
                let off = t[i..].to_string();
                t.truncate(i);
                let sign = if off.starts_with('-') { -1.0 } else { 1.0 };
                let hm: Vec<f64> = off[1..].split(':').filter_map(|x| x.parse().ok()).collect();
                tz = sign * (hm.first().unwrap_or(&0.0) * 3600.0 + hm.get(1).unwrap_or(&0.0) * 60.0);
            }
            return Some(days as f64 * 86400.0 + parse_clock(&t)? - tz);
        }
    }
    let parts: Vec<f64> = s.split(':').filter_map(|x| x.parse().ok()).collect();
    match parts.len() {
        3 => Some(parts[0] * 3600.0 + parts[1] * 60.0 + parts[2]),
        4 => Some(parts[0] * 3600.0 + parts[1] * 60.0 + parts[2] + parts[3] / 30.0),
        2 => Some(parts[0] * 60.0 + parts[1]),
        _ => None,
    }
}

fn data_start(path: &str, key: Option<&str>) -> Result<f64, String> {
    let text = std::fs::read_to_string(path).map_err(|x| format!("{path}: {x}"))?;
    if text.trim_start().starts_with(['{', '[']) {
        let j: Json = serde_json::from_str(&text).map_err(|x| format!("{path}: {x}"))?;
        let rows = j.as_array().cloned().or_else(|| j.as_object().and_then(|o| o.values().find_map(|v| v.as_array().cloned()))).ok_or("no list of samples in the JSON")?;
        let first = rows.first().ok_or("the data has no samples")?;
        let obj = first.as_object().ok_or("samples are not objects")?;
        let v = match key {
            Some(k) => obj.get(k).ok_or_else(|| format!("no field `{k}`"))?.clone(),
            None => obj.iter().find(|(k, _)| k.to_ascii_lowercase().contains("time") || k.to_ascii_lowercase().contains("date")).map(|x| x.1.clone()).ok_or("pass `key` (the time field)")?,
        };
        return match v {
            Json::Number(n) => n.as_f64().ok_or_else(|| "bad time".into()),
            Json::String(s) => parse_clock(&s).ok_or_else(|| format!("`{s}` is not a time")),
            _ => Err("the time field is not a time".into()),
        };
    }
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let head: Vec<String> = lines.next().ok_or("empty data file")?.split([',', '\t', ';']).map(|x| x.trim().trim_matches('"').to_string()).collect();
    let col = match key {
        Some(k) => head.iter().position(|h| h == k).ok_or_else(|| format!("no column `{k}`"))?,
        None => head.iter().position(|h| h.to_ascii_lowercase().contains("time") || h.to_ascii_lowercase().contains("date")).ok_or("pass `key` (the time column)")?,
    };
    let row: Vec<String> = lines.next().ok_or("the data has no rows")?.split([',', '\t', ';']).map(|x| x.trim().trim_matches('"').to_string()).collect();
    let cell = row.get(col).ok_or("short row")?;
    parse_clock(cell).ok_or_else(|| format!("`{cell}` is not a time"))
}

pub fn align_video_to_data(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?.clone();
    let footage = l.source.and_then(|s| e.project.item(s)).and_then(|i| if let ItemKind::Footage(f) = &i.kind { Some(f.path.clone()) } else { None }).ok_or("Align Video to Data works on a footage layer")?;
    let data = match p.get("data") {
        Some(Json::String(s)) if Path::new(s).is_file() => s.clone(),
        Some(v) => super::project::item_ref(e, v).and_then(|i| e.project.item(i)).and_then(|i| if let ItemKind::Footage(f) = &i.kind { Some(f.path.clone()) } else { None }).ok_or("`data` is a CSV or JSON file, or a footage item made from one")?,
        None => return Err("pass `data` (a CSV or JSON file with timestamps)".into()),
    };
    let d0 = data_start(&data, ps(p, "key"))?;
    let v0 = match ps(p, "videoStart") {
        Some(s) => parse_clock(s).ok_or_else(|| format!("`{s}` is not a time"))?,
        None => std::fs::metadata(&footage).and_then(|m| m.created().or_else(|_| m.modified())).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs_f64()).ok_or("pass `videoStart`: the file has no creation time")?,
    };
    let (v0, d0) = if (v0 > 86400.0 * 365.0) != (d0 > 86400.0 * 365.0) { (v0.rem_euclid(86400.0), d0.rem_euclid(86400.0)) } else { (v0, d0) };
    let at = pf(p, "dataStart").unwrap_or(0.0) + (v0 - d0);
    let shift = at - l.start_time;
    let lm = e.layer_mut(cid, lid)?;
    lm.start_time += shift;
    lm.in_point += shift;
    lm.out_point += shift;
    e.set_label("Align Video to Data");
    Ok(json!({"layer": lid, "start": at, "offset": v0 - d0}))
}

pub fn rpf_camera_import(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (.json or .csv camera data)")?;
    let text = std::fs::read_to_string(path).map_err(|x| format!("{path}: {x}"))?;
    let cid = e.comp_id(p.get("comp"))?;
    let c = e.project.comp(cid).ok_or("no composition")?;
    let (fps, cw) = (c.frame_rate, c.width as f64);
    let mut rows: Vec<(f64, [f64; 3], [f64; 3], f64)> = vec![];
    if text.trim_start().starts_with(['{', '[']) {
        let j: Json = serde_json::from_str(&text).map_err(|x| format!("{path}: {x}"))?;
        let list = j.get("frames").and_then(Json::as_array).or_else(|| j.as_array()).ok_or("no `frames` list")?;
        for (i, f) in list.iter().enumerate() {
            let v3 = |k: &str| f.get(k).and_then(Json::as_array).map(|a| [a.first().and_then(Json::as_f64).unwrap_or(0.0), a.get(1).and_then(Json::as_f64).unwrap_or(0.0), a.get(2).and_then(Json::as_f64).unwrap_or(0.0)]);
            let t = f.get("time").and_then(Json::as_f64).or_else(|| f.get("frame").and_then(Json::as_f64).map(|n| n / fps)).unwrap_or(i as f64 / fps);
            rows.push((t, v3("position").ok_or("a frame has no position")?, v3("rotation").or_else(|| v3("orientation")).unwrap_or([0.0; 3]), f.get("fov").and_then(Json::as_f64).unwrap_or(0.0)));
        }
    } else {
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let head: Vec<String> = lines.next().ok_or("empty file")?.split(',').map(|x| x.trim().to_ascii_lowercase()).collect();
        let col = |n: &str| head.iter().position(|h| h == n);
        let need = |n: &str| col(n).ok_or_else(|| format!("no `{n}` column"));
        let (x, y, z) = (need("x")?, need("y")?, need("z")?);
        for (i, line) in lines.enumerate() {
            let v: Vec<f64> = line.split(',').map(|s| s.trim().parse().unwrap_or(0.0)).collect();
            let g = |c: Option<usize>| c.and_then(|c| v.get(c)).copied().unwrap_or(0.0);
            let t = col("time").map(|c| g(Some(c))).or_else(|| col("frame").map(|c| g(Some(c)) / fps)).unwrap_or(i as f64 / fps);
            rows.push((t, [g(Some(x)), g(Some(y)), g(Some(z))], [g(col("rx")), g(col("ry")), g(col("rz"))], g(col("fov"))));
        }
    }
    if rows.is_empty() {
        return Err("no camera samples".into());
    }
    let r = super::layer::new_camera(e, &json!({"comp": cid, "name": Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("Camera"), "type": "oneNode"}))?;
    let cam = r.get("layer").and_then(Json::as_u64).ok_or("no camera")?;
    let l = e.layer_mut(cid, cam)?;
    let st = l.start_time;
    let set = |l: &mut Layer, path: &str, keys: Vec<(f64, Value)>| {
        if let Some(uid) = l.props.resolve_uid(path)
            && let Some(pr) = l.props.find_mut(uid)
        {
            pr.keys.clear();
            let spatial = pr.spatial;
            for (t, v) in keys {
                crate::model::key::insert(&mut pr.keys, crate::model::Key::new(t - st, v, spatial));
            }
        }
    };
    set(l, "transform/position", rows.iter().map(|r| (r.0, Value::Vec3(r.1))).collect());
    set(l, "transform/rotationX", rows.iter().map(|r| (r.0, Value::Num(r.2[0]))).collect());
    set(l, "transform/rotationY", rows.iter().map(|r| (r.0, Value::Num(r.2[1]))).collect());
    set(l, "transform/rotation", rows.iter().map(|r| (r.0, Value::Num(r.2[2]))).collect());
    if rows.iter().any(|r| r.3 > 0.0) {
        set(l, "cameraOptions/zoom", rows.iter().filter(|r| r.3 > 0.0).map(|r| (r.0, Value::Num(cw / 2.0 / (r.3.to_radians() / 2.0).tan()))).collect());
    }
    e.set_label("RPF Camera Import");
    Ok(json!({"camera": cam, "frames": rows.len()}))
}

fn trace_contours(g: &crate::render::vision::Gray, threshold: f32, invert: bool) -> Vec<Vec<[f64; 2]>> {
    let (w, h) = (g.w, g.h);
    let on = |x: i64, y: i64| -> bool {
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
            return false;
        }
        let v = g.px[y as usize * w + x as usize] >= threshold;
        v != invert
    };
    let mut edges: std::collections::HashMap<(i64, i64), Vec<(i64, i64)>> = std::collections::HashMap::new();
    for y in -1..h as i64 {
        for x in -1..w as i64 {
            let a = on(x, y);
            if a != on(x + 1, y) {
                let (p, q) = ((x + 1, y), (x + 1, y + 1));
                if a { edges.entry(p).or_default().push(q) } else { edges.entry(q).or_default().push(p) }
            }
            if a != on(x, y + 1) {
                let (p, q) = ((x + 1, y + 1), (x, y + 1));
                if a { edges.entry(p).or_default().push(q) } else { edges.entry(q).or_default().push(p) }
            }
        }
    }
    let mut out = vec![];
    while let Some(&start) = edges.keys().next() {
        let mut ring = vec![];
        let mut cur = start;
        loop {
            ring.push([cur.0 as f64, cur.1 as f64]);
            let Some(list) = edges.get_mut(&cur) else { break };
            let next = list.pop();
            if list.is_empty() {
                edges.remove(&cur);
            }
            match next {
                Some(n) if n != start => cur = n,
                _ => break,
            }
        }
        if ring.len() >= 4 {
            out.push(ring);
        }
    }
    out
}

fn simplify(pts: &[[f64; 2]], tol: f64) -> Vec<[f64; 2]> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    let last = pts.len() - 1;
    keep[last] = true;
    let far = pts.iter().enumerate().max_by(|a, b| crate::model::path::dist(*a.1, pts[0]).total_cmp(&crate::model::path::dist(*b.1, pts[0]))).map(|x| x.0).unwrap_or(last);
    keep[far] = true;
    let mut stack = vec![(0, far), (far, last)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb_) = (pts[a], pts[b]);
        let (dx, dy) = (pb_[0] - pa[0], pb_[1] - pa[1]);
        let len = (dx * dx + dy * dy).sqrt().max(1e-9);
        let (mut best, mut bi) = (0.0, a);
        for (i, q) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = ((q[0] - pa[0]) * dy - (q[1] - pa[1]) * dx).abs() / len;
            if d > best {
                best = d;
                bi = i;
            }
        }
        if best > tol {
            keep[bi] = true;
            stack.push((a, bi));
            stack.push((bi, b));
        }
    }
    pts.iter().zip(keep).filter(|x| x.1).map(|x| *x.0).collect()
}

pub fn auto_trace(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let channel = match ps(p, "channel").unwrap_or("alpha") {
        "alpha" => 9,
        "red" => 10,
        "green" => 11,
        "blue" => 12,
        "luminance" => 1,
        other => return Err(format!("`{other}` is not a channel (alpha, red, green, blue, luminance)")),
    };
    let invert = pb(p, "invert").unwrap_or(false);
    let blur = pf(p, "blur").unwrap_or(1.0).max(0.0) as f32;
    let tol = pf(p, "tolerance").unwrap_or(1.0).max(0.1);
    let threshold = (pf(p, "threshold").unwrap_or(50.0) / 100.0).clamp(0.0, 1.0) as f32;
    let min_area = pf(p, "minimumArea").unwrap_or(10.0).max(0.0);
    let round = (pf(p, "cornerRoundness").unwrap_or(50.0) / 100.0).clamp(0.0, 1.0);
    let span = ps(p, "timeSpan").unwrap_or("currentFrame");
    let c = e.project.comp(cid).ok_or("no composition")?.clone();
    let times: Vec<f64> = if span == "workArea" {
        let fd = c.frame_duration();
        let n = (((c.work_area[1] - c.work_area[0]) / fd).round() as usize).clamp(1, 2000);
        (0..n).map(|i| c.work_area[0] + i as f64 * fd).collect()
    } else {
        vec![now(e, cid)]
    };
    let pj = e.project.clone();
    let src = crate::render::analysis::Source::new(&pj, &e.media, cid, lid, 2048.0).ok_or("no layer source")?;
    let mut per_time: Vec<(f64, Vec<BezPath>)> = vec![];
    for t in &times {
        let img = src.image(*t).ok_or("nothing to trace at that time")?;
        let mut g = crate::render::vision::Gray { w: img.w, h: img.h, px: vec![0.0; img.w * img.h] };
        for i in 0..img.w * img.h {
            let px = &img.px[i * 4..i * 4 + 4];
            let a = px[3].max(1e-6);
            g.px[i] = match channel {
                9 => px[3],
                10 => px[0] / a,
                11 => px[1] / a,
                12 => px[2] / a,
                _ => (0.2126 * px[0] + 0.7152 * px[1] + 0.0722 * px[2]) / a,
            };
        }
        g.blur(blur * src.scale as f32);
        let mut paths = vec![];
        for ring in trace_contours(&g, threshold, invert) {
            let pts: Vec<[f64; 2]> = ring.iter().map(|q| src.to_layer([q[0] as f32, q[1] as f32])).collect();
            let area = pts.iter().enumerate().map(|(i, a)| {
                let b = pts[(i + 1) % pts.len()];
                a[0] * b[1] - b[0] * a[1]
            }).sum::<f64>().abs() / 2.0;
            if area < min_area {
                continue;
            }
            let s = simplify(&pts, tol);
            if s.len() < 3 {
                continue;
            }
            let mut bp = BezPath::polygon(&s, true);
            if round > 0.0 {
                auto_tangents(&mut bp);
                for i in 0..bp.vertices.len() {
                    bp.ins[i] = [bp.ins[i][0] * round, bp.ins[i][1] * round];
                    bp.outs[i] = [bp.outs[i][0] * round, bp.outs[i][1] * round];
                }
            }
            paths.push(bp);
        }
        per_time.push((*t, paths));
    }
    let target = if pb(p, "applyToNewLayer").unwrap_or(false) {
        let (w, h) = (c.width, c.height);
        let r = super::layer::new_solid(e, &json!({"comp": cid, "name": "Auto-traced", "color": [1.0, 1.0, 1.0], "width": w, "height": h}))?;
        r.get("layer").and_then(Json::as_u64).ok_or("no layer")?
    } else {
        lid
    };
    let count = per_time.first().map(|x| x.1.len()).unwrap_or(0);
    let mut masks = vec![];
    for i in 0..count {
        let first = per_time[0].1[i].clone();
        let r = super::layer::add_mask(e, &json!({"comp": cid, "layer": target, "path": {"vertices": first.vertices, "in": first.ins, "out": first.outs, "closed": true}}))?;
        let uid = r.get("mask").and_then(Json::as_u64).ok_or("no mask")?;
        if per_time.len() > 1 {
            let tl = e.layer_mut(cid, target)?;
            let st = tl.start_time;
            if let Some(g) = tl.props.find_group_mut(uid)
                && let Some(pr) = g.get_mut("path")
            {
                for (t, ps_) in &per_time {
                    if let Some(bp) = ps_.get(i) {
                        crate::model::key::insert(&mut pr.keys, crate::model::Key::new(t - st, Value::Path(bp.clone()), false));
                    }
                }
            }
        }
        masks.push(uid);
    }
    if masks.is_empty() {
        return Err("nothing crossed the threshold; lower it or pick another channel".into());
    }
    e.set_label("Auto-trace");
    Ok(json!({"layer": target, "masks": masks, "frames": per_time.len()}))
}

pub fn scene_edit_detection(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let mode = ps(p, "mode").unwrap_or("markers").to_string();
    if !matches!(mode.as_str(), "markers" | "split" | "splitPrecompose") {
        return Err("mode is markers, split or splitPrecompose".into());
    }
    let threshold = pf(p, "threshold").unwrap_or(0.25).clamp(0.01, 1.0);
    let l = e.layer_ref(cid, lid)?.clone();
    if l.kind != LayerKind::Footage {
        return Err("Scene Edit Detection works on footage layers".into());
    }
    let times = crate::render::analysis::frame_times(&e.project, cid, lid, l.in_point, l.out_point);
    let wait = pb(p, "wait").unwrap_or(false);
    jobs::run(e, "sceneEdit", lid, "Scene Edit Detection", wait, move |ctx| {
        let src = crate::render::analysis::Source::new(&ctx.project, &ctx.media, cid, lid, 160.0).ok_or("no layer source")?;
        let hist = |t: f64| -> Option<Vec<f32>> {
            let img = src.image(t)?;
            let mut h = vec![0f32; 48];
            let n = (img.w * img.h).max(1) as f32;
            for px in img.px.chunks_exact(4) {
                let a = px[3].max(1e-6);
                for (k, v) in px[..3].iter().enumerate() {
                    let b = ((v / a).clamp(0.0, 0.9999) * 16.0) as usize;
                    h[k * 16 + b] += 1.0 / n;
                }
            }
            Some(h)
        };
        let mut cuts = vec![];
        let mut prev: Option<Vec<f32>> = None;
        for (i, t) in times.iter().enumerate() {
            if ctx.stopped() {
                return Err("stopped".into());
            }
            ctx.step(i, times.len(), "comparing frames");
            let Some(h) = hist(*t) else { continue };
            if let Some(pv) = &prev {
                let d: f32 = h.iter().zip(pv.iter()).map(|(a, b): (&f32, &f32)| (a - b).abs()).sum::<f32>() / 6.0;
                if d as f64 > threshold {
                    cuts.push(*t);
                }
            }
            prev = Some(h);
        }
        let apply: Apply = Box::new(move |e: &mut Engine| {
            let comp_cuts: Vec<f64> = cuts.clone();
            match mode.as_str() {
                "markers" => {
                    let l = e.layer_mut(cid, lid)?;
                    for t in &comp_cuts {
                        let lt = l.layer_time(*t);
                        l.markers.push(crate::model::Marker { time: lt, comment: "Scene".into(), ..Default::default() });
                    }
                    l.markers.sort_by(|a, b| a.time.total_cmp(&b.time));
                }
                _ => {
                    let mut ids = vec![lid];
                    for t in comp_cuts.iter().rev() {
                        let r = super::layer::split(e, &json!({"comp": cid, "layers": [lid], "time": t}))?;
                        ids.extend(r.get("layers").and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_u64));
                    }
                    if mode == "splitPrecompose" {
                        for id in ids {
                            let _ = super::layer::precompose(e, &json!({"comp": cid, "layers": [id], "moveAll": true}));
                        }
                    }
                }
            }
            Ok(json!({"cuts": comp_cuts, "mode": mode}))
        });
        Ok(apply)
    })
}
