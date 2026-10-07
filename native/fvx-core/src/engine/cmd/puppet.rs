use serde_json::{Value as Json, json};

use super::prop::set_value_at;
use crate::effects::puppet::{self as fx, MeshParams};
use crate::engine::{Engine, Res, pb, pf, ps};
use crate::eval::Eval;
use crate::model::Value;
use crate::model::key::{self, Key};
use crate::model::prop::{Group, GroupKind, Node};
use crate::render::analysis::Source;
use crate::render::puppet::{self, KINDS, Pin};

fn is_puppet(g: &Group) -> bool {
    matches!(&g.kind, GroupKind::Effect { effect } if effect == fx::ID)
}

fn puppet_uid(e: &Engine, cid: u64, lid: u64) -> Result<Option<u64>, String> {
    Ok(e.layer_ref(cid, lid)?.effects().and_then(|g| g.groups().find(|x| is_puppet(x)).map(|x| x.uid)))
}

fn ensure_puppet(e: &mut Engine, cid: u64, lid: u64) -> Result<u64, String> {
    if let Some(u) = puppet_uid(e, cid, lid)? {
        return Ok(u);
    }
    let r = super::effect::apply(e, &json!({"effect": fx::ID, "layers": [lid], "comp": cid}))?;
    r.get("applied").and_then(|a| a.get(0)).and_then(|a| a.get("uid")).and_then(Json::as_u64).ok_or_else(|| "could not add Puppet".into())
}

fn time_of(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<(f64, f64), String> {
    let fd = e.project.comp(cid).ok_or("no composition")?.frame_duration();
    let ct = match (pf(p, "time"), pf(p, "frame")) {
        (Some(t), _) => t,
        (None, Some(f)) => f * fd,
        _ => e.time_of(cid),
    };
    Ok((ct, e.layer_ref(cid, lid)?.layer_time(ct)))
}

fn params_at(e: &Engine, cid: u64, lid: u64, fxu: u64, ct: f64) -> Result<Vec<MeshParams>, String> {
    let c = e.project.comp(cid).ok_or("no composition")?;
    let l = c.layer(lid).ok_or("no layer")?;
    let g = l.props.find_group(fxu).ok_or("no Puppet")?;
    let ev = Eval::new(&e.project, cid, c, ct);
    Ok(fx::read(&crate::effects::params_at(&ev, l, g)))
}

fn mesh_for(e: &Engine, cid: u64, lid: u64, m: &MeshParams) -> Option<std::sync::Arc<puppet::Mesh>> {
    let src = Source::new(&e.project, &e.media, cid, lid, 2048.0)?;
    let l = e.project.comp(cid)?.layer(lid)?;
    let img = src.image(l.comp_time(m.mesh_time))?;
    fx::mesh_from(&img, [src.frame[0], src.frame[1]], src.scale, m)
}

fn pin_ref(e: &Engine, cid: u64, lid: u64, v: &Json) -> Result<u64, String> {
    let l = e.layer_ref(cid, lid)?;
    let fxg = l.effects().and_then(|g| g.groups().find(|x| is_puppet(x))).ok_or("the layer has no puppet pins")?;
    for m in fxg.groups() {
        if let Some(d) = m.sub("deform") {
            for pg in d.groups() {
                if v.as_u64() == Some(pg.uid) || v.as_str() == Some(pg.name.as_str()) {
                    return Ok(pg.uid);
                }
            }
        }
    }
    Err(format!("no puppet pin {v}"))
}

fn v2(p: &Json, k: &str) -> Option<[f64; 2]> {
    let a = p.get(k)?.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
}

fn rest_for(mesh: &puppet::Mesh, moved: &[[f64; 2]], q: [f64; 2]) -> Option<[f64; 2]> {
    for t in &mesh.tris {
        let (a, b, c) = (moved[t[0]], moved[t[1]], moved[t[2]]);
        let den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if den.abs() < 1e-12 {
            continue;
        }
        let l0 = ((b[1] - c[1]) * (q[0] - c[0]) + (c[0] - b[0]) * (q[1] - c[1])) / den;
        let l1 = ((c[1] - a[1]) * (q[0] - c[0]) + (a[0] - c[0]) * (q[1] - c[1])) / den;
        let l2 = 1.0 - l0 - l1;
        if l0 >= -1e-6 && l1 >= -1e-6 && l2 >= -1e-6 {
            let (ra, rb, rc) = (mesh.rest[t[0]], mesh.rest[t[1]], mesh.rest[t[2]]);
            return Some([ra[0] * l0 + rb[0] * l1 + rc[0] * l2, ra[1] * l0 + rb[1] * l1 + rc[1] * l2]);
        }
    }
    None
}

pub fn add_pin(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let kind = KINDS.iter().position(|k| *k == ps(p, "kind").unwrap_or("position")).ok_or_else(|| format!("kind is one of {}", KINDS.join(", ")))?;
    let pos = v2(p, "position").ok_or("pass `position`: [x, y] in layer space")?;
    let (ct, lt) = time_of(e, cid, lid, p)?;
    let fxu = ensure_puppet(e, cid, lid)?;
    let existing = params_at(e, cid, lid, fxu, ct)?;
    let mesh_uid = {
        let l = e.layer_ref(cid, lid)?;
        let g = l.props.find_group(fxu).ok_or("no Puppet")?;
        let meshes: Vec<&Group> = g.groups().collect();
        match p.get("mesh").and_then(Json::as_u64) {
            Some(u) => Some(meshes.iter().find(|m| m.uid == u).map(|m| m.uid).ok_or("no such mesh")?),
            None if pb(p, "newMesh").unwrap_or(false) => None,
            None => meshes.last().map(|m| m.uid),
        }
    };
    let mesh_uid = match mesh_uid {
        Some(u) => u,
        None => {
            let n = e.layer_ref(cid, lid)?.props.find_group(fxu).map(|g| g.groups().count()).unwrap_or(0) + 1;
            let mut g = {
                let pj = e.proj();
                let mut ids = pj.ids();
                fx::mesh_group(&mut ids, &format!("m{n:02}"), &format!("Mesh {n}"), pf(p, "density").unwrap_or(10.0), pf(p, "expansion").unwrap_or(3.0), pf(p, "triangles").unwrap_or(350.0), lt)
            };
            if let Some(s) = g.get_mut("seed") {
                s.value = Value::Vec2(pos);
            }
            let u = g.uid;
            e.layer_mut(cid, lid)?.props.find_group_mut(fxu).ok_or("no Puppet")?.children.push(Node::Group(g));
            u
        }
    };
    let mesh_id = e.layer_ref(cid, lid)?.props.find_group(mesh_uid).map(|g| g.id.clone()).ok_or("no mesh")?;
    let rest = match existing.iter().find(|m| m.id == mesh_id) {
        Some(m) if !m.pins.is_empty() => {
            let mesh = mesh_for(e, cid, lid, m);
            match mesh {
                Some(mesh) => {
                    let pins: Vec<Pin> = m.pins.iter().map(|x| x.1).collect();
                    let moved = puppet::deform(&mesh, &pins);
                    rest_for(&mesh, &moved, pos).unwrap_or(pos)
                }
                None => pos,
            }
        }
        _ => pos,
    };
    let (gid, name, others_keyed) = {
        let g = e.layer_ref(cid, lid)?.props.find_group(mesh_uid).ok_or("no mesh")?;
        let d = g.sub("deform").ok_or("no Deform group")?;
        let next = d.groups().filter_map(|x| x.id.strip_prefix('p').and_then(|v| v.parse::<u32>().ok())).max().unwrap_or(0) + 1;
        let label = match kind {
            1 => "Advanced Pin",
            2 => "Bend Pin",
            3 => "Starch Pin",
            4 => "Overlap Pin",
            _ => "Puppet Pin",
        };
        let count = d.groups().filter(|x| x.name.starts_with(label)).count() + 1;
        let keyed = d.groups().any(|x| x.get("position").is_some_and(|pr| !pr.keys.is_empty()));
        (format!("p{next:04}"), format!("{label} {count}"), keyed)
    };
    let mut pin = {
        let pj = e.proj();
        let mut ids = pj.ids();
        fx::pin_group(&mut ids, &gid, &name, kind, rest, pos)
    };
    if (kind == 0 || kind == 1 || others_keyed)
        && let Some(pr) = pin.get_mut("position")
    {
        key::insert(&mut pr.keys, Key::new(lt, Value::Vec2(pos), true));
    }
    let uid = pin.uid;
    e.layer_mut(cid, lid)?.props.find_group_mut(mesh_uid).and_then(|g| g.sub_mut("deform")).ok_or("no Deform group")?.children.push(Node::Group(pin));
    e.state.puppet_pins = vec![uid];
    e.set_label(format!("Add {name}"));
    Ok(json!({"layer": lid, "effect": fxu, "mesh": mesh_uid, "pin": uid, "name": name, "rest": rest}))
}

fn with_pin(e: &mut Engine, cid: u64, lid: u64, uid: u64, f: impl FnOnce(&mut Group) -> Result<(), String>) -> Result<(), String> {
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no such pin")?;
    f(g)
}

pub fn move_pin(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = pin_ref(e, cid, lid, p.get("pin").ok_or("pass `pin`")?)?;
    let pos = v2(p, "position").ok_or("pass `position`: [x, y]")?;
    let (_, lt) = time_of(e, cid, lid, p)?;
    with_pin(e, cid, lid, uid, |g| {
        let pr = g.get_mut("position").ok_or("that pin has no position")?;
        set_value_at(pr, Value::Vec2(pos), Some(lt));
        Ok(())
    })?;
    if let Some(m) = ps(p, "merge") {
        e.merge_hint = Some(m.to_string());
    }
    e.set_label("Move Puppet Pin");
    Ok(json!({"pin": uid, "position": pos}))
}

pub fn set_pin(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = pin_ref(e, cid, lid, p.get("pin").ok_or("pass `pin`")?)?;
    let (_, lt) = time_of(e, cid, lid, p)?;
    with_pin(e, cid, lid, uid, |g| {
        if let Some(v) = v2(p, "position") {
            set_value_at(g.get_mut("position").ok_or("that pin has no position")?, Value::Vec2(v), Some(lt));
        }
        for k in ["scale", "rotation", "amount", "extent", "inFront"] {
            if let Some(v) = pf(p, k) {
                set_value_at(g.get_mut(k).ok_or_else(|| format!("that pin has no {k}"))?, Value::Num(v), Some(lt));
            }
        }
        Ok(())
    })?;
    if let Some(m) = ps(p, "merge") {
        e.merge_hint = Some(m.to_string());
    }
    e.set_label("Edit Puppet Pin");
    Ok(json!({"pin": uid}))
}

pub fn remove_pin(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let mut uids = vec![];
    if let Some(v) = p.get("pin") {
        uids.push(pin_ref(e, cid, lid, v)?);
    }
    if let Some(a) = p.get("pins").and_then(Json::as_array) {
        for v in a {
            uids.push(pin_ref(e, cid, lid, v)?);
        }
    }
    if uids.is_empty() {
        uids = e.state.puppet_pins.clone();
    }
    if uids.is_empty() {
        return Err("select pins or pass `pin`".into());
    }
    let l = e.layer_mut(cid, lid)?;
    let fxg = l.props.sub_mut("effects").ok_or("no effects")?;
    let mut n = 0;
    for g in fxg.groups_mut().filter(|g| is_puppet(g)) {
        for m in g.groups_mut() {
            if let Some(d) = m.sub_mut("deform") {
                let before = d.children.len();
                d.children.retain(|c| !uids.contains(&c.uid()));
                n += before - d.children.len();
            }
        }
    }
    e.state.puppet_pins.retain(|u| !uids.contains(u));
    e.set_label("Delete Puppet Pin");
    Ok(json!({"removed": n}))
}

pub fn select_pins(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let list = p.get("pins").and_then(Json::as_array).ok_or("pass `pins`: [uid | name...]")?;
    let mut uids = vec![];
    for v in list {
        uids.push(pin_ref(e, cid, lid, v)?);
    }
    if pb(p, "toggle").unwrap_or(false) {
        for u in uids {
            match e.state.puppet_pins.iter().position(|x| *x == u) {
                Some(i) => {
                    e.state.puppet_pins.remove(i);
                }
                None => e.state.puppet_pins.push(u),
            }
        }
    } else if pb(p, "add").unwrap_or(false) {
        for u in uids {
            if !e.state.puppet_pins.contains(&u) {
                e.state.puppet_pins.push(u);
            }
        }
    } else {
        e.state.puppet_pins = uids;
    }
    Ok(json!({"selected": e.state.puppet_pins}))
}

pub fn mesh(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let fxu = puppet_uid(e, cid, lid)?.ok_or("the layer has no puppet")?;
    let (_, lt) = time_of(e, cid, lid, p)?;
    let target = {
        let g = e.layer_ref(cid, lid)?.props.find_group(fxu).ok_or("no Puppet")?;
        match p.get("mesh").and_then(Json::as_u64) {
            Some(u) => g.groups().find(|m| m.uid == u).map(|m| m.uid).ok_or("no such mesh")?,
            None => g.groups().last().map(|m| m.uid).ok_or("the puppet has no mesh")?,
        }
    };
    if let Some(s) = pb(p, "showMesh") {
        e.prefs.insert("puppetShowMesh".into(), json!(s));
    }
    with_pin(e, cid, lid, target, |g| {
        for k in ["density", "expansion", "triangles"] {
            if let Some(v) = pf(p, k)
                && let Some(pr) = g.get_mut(k)
            {
                set_value_at(pr, Value::Num(v), Some(lt));
            }
        }
        Ok(())
    })?;
    if let Some(m) = ps(p, "merge") {
        e.merge_hint = Some(m.to_string());
    }
    e.set_label("Puppet Mesh");
    Ok(json!({"mesh": target}))
}

pub fn info(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let Some(fxu) = puppet_uid(e, cid, lid)? else { return Ok(json!({"layer": lid, "meshes": []})) };
    let (ct, _) = time_of(e, cid, lid, p)?;
    let meshes = params_at(e, cid, lid, fxu, ct)?;
    let l = e.layer_ref(cid, lid)?;
    let g = l.props.find_group(fxu).ok_or("no Puppet")?;
    let mut out = vec![];
    for mg in g.groups() {
        let Some(m) = meshes.iter().find(|m| m.id == mg.id) else { continue };
        let built = mesh_for(e, cid, lid, m);
        let pins: Vec<Json> = mg
            .sub("deform")
            .map(|d| {
                d.groups()
                    .filter_map(|pg| {
                        let (_, pin) = m.pins.iter().find(|x| x.0 == pg.id)?;
                        Some(json!({"pin": pg.uid, "name": pg.name, "kind": KINDS[pin.kind], "position": pin.pos, "rest": pin.rest, "selected": e.state.puppet_pins.contains(&pg.uid)}))
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(json!({"mesh": mg.uid, "name": mg.name, "triangles": built.as_ref().map(|b| b.tris.len()).unwrap_or(0), "vertices": built.as_ref().map(|b| b.rest.len()).unwrap_or(0), "pins": pins}));
    }
    Ok(json!({"layer": lid, "effect": fxu, "meshes": out, "showMesh": e.prefs.get("puppetShowMesh").cloned().unwrap_or(json!(false))}))
}

pub fn record_options(e: &mut Engine, p: &Json) -> Res {
    let mut cur = e.prefs.get("puppetRecord").cloned().unwrap_or_else(|| json!({"speed": 100.0, "smoothing": 5.0, "useDraftDeformation": false, "showMesh": false}));
    if let (Some(o), Some(src)) = (cur.as_object_mut(), p.as_object()) {
        for k in ["speed", "smoothing", "useDraftDeformation", "showMesh"] {
            if let Some(v) = src.get(k) {
                o.insert(k.into(), v.clone());
            }
        }
    }
    e.prefs.insert("puppetRecord".into(), cur.clone());
    Ok(cur)
}

fn pos_at(e: &Engine, cid: u64, lid: u64, uid: u64, lt: f64) -> Option<[f64; 2]> {
    let l = e.layer_ref(cid, lid).ok()?;
    let g = l.props.find_group(uid)?;
    Some(g.get("position")?.at(lt).vec2())
}

pub fn record_pin(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let leader = pin_ref(e, cid, lid, p.get("pin").ok_or("pass `pin`")?)?;
    let mut followers = vec![];
    if let Some(a) = p.get("pins").and_then(Json::as_array) {
        for v in a {
            let u = pin_ref(e, cid, lid, v)?;
            if u != leader {
                followers.push(u);
            }
        }
    }
    let samples: Vec<[f64; 3]> = p.get("samples").and_then(Json::as_array).ok_or("pass `samples`: [[t, x, y], ...]")?.iter().filter_map(|s| {
        let a = s.as_array()?;
        Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
    }).collect();
    if samples.is_empty() {
        return Err("no samples".into());
    }
    let opts = record_options(e, &json!({}))?;
    let speed = pf(p, "speed").or_else(|| opts.get("speed").and_then(Json::as_f64)).unwrap_or(100.0).max(1.0);
    let smoothing = pf(p, "smoothing").or_else(|| opts.get("smoothing").and_then(Json::as_f64)).unwrap_or(5.0).clamp(0.0, 100.0);
    let (_, lt0) = match pf(p, "start") {
        Some(s) => time_of(e, cid, lid, &json!({"time": s}))?,
        None => time_of(e, cid, lid, &json!({}))?,
    };
    let fd = e.project.comp(cid).ok_or("no composition")?.frame_duration();
    let win = (smoothing / 10.0).round() as usize;
    let smoothed: Vec<[f64; 3]> = (0..samples.len())
        .map(|i| {
            let (a, b) = (i.saturating_sub(win), (i + win).min(samples.len() - 1));
            let n = (b - a + 1) as f64;
            let sx = samples[a..=b].iter().map(|s| s[1]).sum::<f64>() / n;
            let sy = samples[a..=b].iter().map(|s| s[2]).sum::<f64>() / n;
            [samples[i][0], sx, sy]
        })
        .collect();
    let mut keys: Vec<(f64, [f64; 2])> = vec![];
    for s in &smoothed {
        let t = lt0 + s[0] * 100.0 / speed;
        let snapped = (t / fd).round() * fd;
        match keys.last_mut() {
            Some(k) if (k.0 - snapped).abs() < 1e-9 => k.1 = [s[1], s[2]],
            _ => keys.push((snapped, [s[1], s[2]])),
        }
    }
    let base = pos_at(e, cid, lid, leader, lt0).unwrap_or([smoothed[0][1], smoothed[0][2]]);
    let t_end = keys.last().map(|k| k.0).unwrap_or(lt0);
    let follow_base: Vec<(u64, [f64; 2])> = followers.iter().filter_map(|u| Some((*u, pos_at(e, cid, lid, *u, lt0)?))).collect();
    let write = |g: &mut Group, rows: &[(f64, [f64; 2])]| {
        if let Some(pr) = g.get_mut("position") {
            pr.keys.retain(|k| k.time < lt0 - 1e-9 || k.time > t_end + 1e-9);
            for (t, v) in rows {
                key::insert(&mut pr.keys, Key::new(*t, Value::Vec2(*v), true));
            }
        }
    };
    let l = e.layer_mut(cid, lid)?;
    if let Some(g) = l.props.find_group_mut(leader) {
        write(g, &keys);
    }
    for (u, b) in &follow_base {
        let rows: Vec<(f64, [f64; 2])> = keys.iter().map(|(t, v)| (*t, [b[0] + v[0] - base[0], b[1] + v[1] - base[1]])).collect();
        if let Some(g) = l.props.find_group_mut(*u) {
            write(g, &rows);
        }
    }
    e.set_label("Record Puppet Pin");
    Ok(json!({"pin": leader, "keys": keys.len(), "followers": follow_base.len()}))
}

pub fn follow(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let leader = match p.get("leader") {
        Some(v) => pin_ref(e, cid, lid, v)?,
        None => *e.state.puppet_pins.first().ok_or("select the leading pin first")?,
    };
    let mut followers = vec![];
    match p.get("pins").and_then(Json::as_array) {
        Some(a) => {
            for v in a {
                followers.push(pin_ref(e, cid, lid, v)?);
            }
        }
        None => followers = e.state.puppet_pins.iter().copied().filter(|u| *u != leader).collect(),
    }
    followers.retain(|u| *u != leader);
    if followers.is_empty() {
        return Err("choose the pins that follow".into());
    }
    let delay = pf(p, "delay").unwrap_or(0.1);
    let amount = pf(p, "amount").unwrap_or(100.0) / 100.0;
    let cascade = pb(p, "cascade").unwrap_or(true);
    let (lkeys, lrest) = {
        let l = e.layer_ref(cid, lid)?;
        let g = l.props.find_group(leader).ok_or("no leader")?;
        let pr = g.get("position").ok_or("the leader has no position")?;
        let rest = g.get("rest").map(|r| r.value.vec2()).unwrap_or([0.0; 2]);
        (pr.keys.iter().map(|k| (k.time, k.value.vec2())).collect::<Vec<_>>(), rest)
    };
    if lkeys.len() < 2 {
        return Err("the leading pin needs keyframes to follow".into());
    }
    let start = lkeys[0].1;
    let mut order: Vec<(u64, f64, [f64; 2])> = followers
        .iter()
        .filter_map(|u| {
            let l = e.layer_ref(cid, lid).ok()?;
            let g = l.props.find_group(*u)?;
            let rest = g.get("rest").map(|r| r.value.vec2()).unwrap_or([0.0; 2]);
            Some((*u, (rest[0] - lrest[0]).hypot(rest[1] - lrest[1]), g.get("position")?.at(lkeys[0].0).vec2()))
        })
        .collect();
    order.sort_by(|a, b| a.1.total_cmp(&b.1));
    let l = e.layer_mut(cid, lid)?;
    for (k, (u, _, base)) in order.iter().enumerate() {
        let shift = if cascade { delay * (k + 1) as f64 } else { delay };
        if let Some(g) = l.props.find_group_mut(*u)
            && let Some(pr) = g.get_mut("position")
        {
            for (t, v) in &lkeys {
                let d = [(v[0] - start[0]) * amount, (v[1] - start[1]) * amount];
                key::insert(&mut pr.keys, Key::new(t + shift, Value::Vec2([base[0] + d[0], base[1] + d[1]]), true));
            }
        }
    }
    e.set_label("Follow-Through");
    Ok(json!({"leader": leader, "followers": order.len()}))
}
