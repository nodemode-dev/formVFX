use serde_json::{Value as Json, json};

use super::{layer, select_only};
use crate::engine::{Clip, Engine, KeyRef, Res, pb, pf, ps, pu, registry};
use crate::model::key::{self, same_time};
use crate::model::prop::GroupKind;
use crate::model::{Guide, label_parse};

pub fn undo(e: &mut Engine, _: &Json) -> Res {
    e.undo()
}

pub fn redo(e: &mut Engine, _: &Json) -> Res {
    e.redo()
}

pub fn history_list(e: &mut Engine, _: &Json) -> Res {
    let mut states = vec![json!({"index": 0, "label": "Open", "current": e.history.undo.is_empty()})];
    for (i, s) in e.history.undo.iter().enumerate() {
        states.push(json!({"index": i + 1, "label": s.label, "current": i + 1 == e.history.undo.len()}));
    }
    let base = e.history.undo.len();
    for (k, s) in e.history.redo.iter().rev().enumerate() {
        states.push(json!({"index": base + k + 1, "label": s.label, "current": false, "future": true}));
    }
    Ok(json!({"states": states, "current": base}))
}

pub fn history_goto(e: &mut Engine, p: &Json) -> Res {
    let cur = e.history.undo.len() as i64;
    let target = match (pu(p, "index"), p.get("steps").and_then(Json::as_i64)) {
        (Some(i), _) => i as i64,
        (None, Some(s)) => cur + s,
        _ => return Err("pass `index` or `steps`".into()),
    };
    let max = cur + e.history.redo.len() as i64;
    let target = target.clamp(0, max);
    let mut n = cur;
    while n > target {
        e.undo()?;
        n -= 1;
    }
    while n < target {
        e.redo()?;
        n += 1;
    }
    Ok(json!({"current": target}))
}

pub fn clear(e: &mut Engine, p: &Json) -> Res {
    if !e.state.selected_keys.is_empty() && p.get("layers").is_none() {
        return super::keys::delete(e, p);
    }
    if !e.state.selected_vertices.is_empty() && p.get("layers").is_none() {
        return super::mask::delete_vertices(e, p);
    }
    let cid = e.comp_id(p.get("comp"))?;
    let groups: Vec<(u64, u64)> = e
        .state
        .selected_props
        .iter()
        .copied()
        .filter(|(l, u)| e.layer_ref(cid, *l).ok().and_then(|ly| ly.props.parent_of(*u)).is_some_and(|par| matches!(par.kind, GroupKind::Indexed)))
        .collect();
    if !groups.is_empty() && p.get("layers").is_none() {
        for (lid, uid) in &groups {
            if let Ok(l) = e.layer_mut(cid, *lid)
                && let Some(par) = l.props.parent_of_mut(*uid)
            {
                par.children.retain(|c| c.uid() != *uid);
            }
        }
        e.state.selected_props.clear();
        e.set_label("Clear");
        return Ok(json!({"removed": groups.len()}));
    }
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("nothing selected".into());
    }
    let n = layer::delete_layers(e, cid, &ids)?;
    e.set_label("Delete Layers");
    Ok(json!({"deleted": n}))
}

pub fn copy(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    if !e.state.selected_keys.is_empty() && p.get("layers").is_none() {
        let t0 = e.state.selected_keys.iter().filter_map(|k| e.layer_ref(cid, k.layer).ok().map(|l| l.comp_time(k.time))).fold(f64::MAX, f64::min);
        let mut keys = vec![];
        for r in &e.state.selected_keys {
            let Ok(l) = e.layer_ref(cid, r.layer) else { continue };
            let Some(path) = l.props.path_of(r.prop) else { continue };
            let Some(pr) = l.props.find(r.prop) else { continue };
            if let Some(k) = pr.keys.iter().find(|k| same_time(k.time, r.time)) {
                keys.push((path, l.comp_time(k.time) - t0, k.clone()));
            }
        }
        let n = keys.len();
        e.clipboard = Some(Clip::Keys(keys));
        return Ok(json!({"keys": n}));
    }
    let (cid, ids) = e.layers_param(p)?;
    let c = e.project.comp(cid).ok_or("no comp")?;
    let layers: Vec<crate::model::Layer> = c.layers.iter().filter(|l| ids.contains(&l.id)).cloned().collect();
    let n = layers.len();
    e.clipboard = Some(Clip::Layers(layers, cid));
    Ok(json!({"layers": n}))
}

pub fn cut(e: &mut Engine, p: &Json) -> Res {
    copy(e, p)?;
    clear(e, p)
}

pub fn paste(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let now = e.time_of(cid);
    match e.clipboard.clone() {
        Some(Clip::Layers(layers, _)) => {
            let mut made = vec![];
            let top = e.state.selected_layers.first().and_then(|s| e.project.comp(cid).and_then(|c| c.index_of(*s))).unwrap_or(0);
            let mut map = std::collections::HashMap::new();
            let mut copies = vec![];
            for l in layers {
                let pj = e.proj();
                let mut nl = l.clone();
                nl.id = pj.new_id();
                map.insert(l.id, nl.id);
                nl.props.renumber(&mut pj.next_uid);
                copies.push(nl);
            }
            for nl in copies.iter_mut() {
                nl.parent = nl.parent.and_then(|x| map.get(&x).copied());
                if let Some(m) = nl.track_matte {
                    nl.track_matte = map.get(&m.layer).map(|x| crate::model::Matte { layer: *x, kind: m.kind });
                }
            }
            let c = e.proj().comp_mut(cid).ok_or("no comp")?;
            for (k, nl) in copies.into_iter().enumerate() {
                made.push(nl.id);
                c.layers.insert((top + k).min(c.layers.len()), nl);
            }
            select_only(e, &made);
            e.set_label("Paste");
            Ok(json!({"layers": made}))
        }
        Some(Clip::Keys(keys)) => {
            let lid = *e.state.selected_layers.first().ok_or("select a layer to paste keyframes onto")?;
            let mut n = 0;
            let mut sel = vec![];
            for (path, dt, k) in keys {
                let l = e.layer_mut(cid, lid)?;
                let lt = l.layer_time(now + dt);
                let Some(uid) = l.props.resolve_uid(&path) else { continue };
                let Some(pr) = l.props.find_mut(uid) else { continue };
                if std::mem::discriminant(&pr.value) != std::mem::discriminant(&k.value) {
                    continue;
                }
                key::insert(&mut pr.keys, crate::model::Key { time: lt, ..k });
                sel.push(KeyRef { layer: lid, prop: uid, time: lt });
                n += 1;
            }
            e.state.selected_keys = sel;
            e.set_label("Paste Keyframes");
            Ok(json!({"keys": n}))
        }
        Some(Clip::Links(links)) => {
            let (cid, ids) = e.layers_param(p)?;
            if ids.is_empty() {
                return Err("select the layers to paste the property links onto".into());
            }
            let mut n = 0;
            for lid in ids {
                for (path, text) in &links {
                    let l = e.layer_mut(cid, lid)?;
                    if let Some(uid) = l.props.resolve_uid(path)
                        && let Some(pr) = l.props.find_mut(uid)
                    {
                        pr.expr = Some(crate::model::prop::Expr { text: text.clone(), enabled: true });
                        n += 1;
                    }
                }
            }
            e.set_label("Paste Property Links");
            Ok(json!({"linked": n}))
        }
        None => Err("the clipboard is empty".into()),
    }
}

pub fn select_all(e: &mut Engine, p: &Json) -> Res {
    let (_, c) = e.comp_of(p)?;
    let ids: Vec<u64> = c.layers.iter().map(|l| l.id).collect();
    select_only(e, &ids);
    Ok(json!({"layers": ids}))
}

pub fn deselect_all(e: &mut Engine, _: &Json) -> Res {
    select_only(e, &[]);
    e.state.text_edit = None;
    Ok(json!({"ok": true}))
}

pub fn label(e: &mut Engine, p: &Json) -> Res {
    let lv = match p.get("label") {
        Some(Json::Number(n)) => n.as_u64().unwrap_or(0) as u8,
        Some(Json::String(s)) => label_parse(s).ok_or_else(|| format!("unknown label `{s}`"))?,
        _ => return Err("pass `label` (a name like Red or 0-16)".into()),
    };
    if let Some(items) = p.get("items").and_then(Json::as_array) {
        let ids: Vec<u64> = items.iter().filter_map(Json::as_u64).collect();
        for id in &ids {
            if let Some(it) = e.proj().item_mut(*id) {
                it.label = lv;
            }
        }
        return Ok(json!({"items": ids.len()}));
    }
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() && !e.state.project_selection.is_empty() {
        let items = e.state.project_selection.clone();
        for id in &items {
            if let Some(it) = e.proj().item_mut(*id) {
                it.label = lv;
            }
        }
        return Ok(json!({"items": items.len()}));
    }
    for id in &ids {
        e.layer_mut(cid, *id)?.label = lv;
    }
    Ok(json!({"layers": ids.len()}))
}

pub fn time_set(e: &mut Engine, p: &Json) -> Res {
    let (cid, c) = e.comp_of(p)?;
    let fr = c.frame_rate;
    let t = if let Some(t) = pf(p, "time") {
        t
    } else if let Some(f) = pf(p, "frame") {
        f / fr
    } else if let Some(tc) = ps(p, "timecode") {
        let parts: Vec<f64> = tc.split([':', ';']).filter_map(|x| x.parse().ok()).collect();
        match parts.len() {
            4 => parts[0] * 3600.0 + parts[1] * 60.0 + parts[2] + parts[3] / fr,
            3 => parts[0] * 60.0 + parts[1] + parts[2] / fr,
            2 => parts[0] + parts[1] / fr,
            1 => parts[0] / fr,
            _ => return Err("bad timecode".into()),
        }
    } else {
        return Err("pass `time` (seconds), `frame` or `timecode`".into());
    };
    let t = c.snap(t.clamp(0.0, (c.duration - c.frame_duration()).max(0.0)));
    e.state.times.insert(cid, t);
    Ok(json!({"time": t}))
}

fn step(e: &mut Engine, p: &Json, frames: f64) -> Res {
    let (cid, c) = e.comp_of(p)?;
    let n = pf(p, "frames").unwrap_or(1.0);
    let t = (e.time_of(cid) + frames * n * c.frame_duration()).clamp(0.0, (c.duration - c.frame_duration()).max(0.0));
    let t = c.snap(t + 1e-9);
    e.state.times.insert(cid, t);
    Ok(json!({"time": t}))
}

pub fn next_frame(e: &mut Engine, p: &Json) -> Res {
    step(e, p, 1.0)
}

pub fn prev_frame(e: &mut Engine, p: &Json) -> Res {
    step(e, p, -1.0)
}

pub fn forward10(e: &mut Engine, p: &Json) -> Res {
    step(e, p, 10.0)
}

pub fn back10(e: &mut Engine, p: &Json) -> Res {
    step(e, p, -10.0)
}

pub fn time_start(e: &mut Engine, p: &Json) -> Res {
    time_set(e, &json!({"time": 0.0, "comp": p.get("comp")}))
}

pub fn time_end(e: &mut Engine, p: &Json) -> Res {
    let (_, c) = e.comp_of(p)?;
    let d = c.duration;
    time_set(e, &json!({"time": d, "comp": p.get("comp")}))
}

pub fn time_go(e: &mut Engine, p: &Json) -> Res {
    let (cid, c) = e.comp_of(p)?;
    let now = e.time_of(cid);
    let to = ps(p, "to").ok_or("pass `to`")?.to_string();
    let sel_layer = pu(p, "layer").or_else(|| e.state.selected_layers.first().copied());
    let t = match to.as_str() {
        "start" => 0.0,
        "end" => c.duration,
        "workStart" => c.work_area[0],
        "workEnd" => c.work_area[1] - c.frame_duration(),
        "layerIn" => sel_layer.and_then(|l| c.layer(l)).map(|l| l.in_point).ok_or("select a layer")?,
        "layerOut" => sel_layer.and_then(|l| c.layer(l)).map(|l| l.out_point - c.frame_duration()).ok_or("select a layer")?,
        "nextKey" | "prevKey" => {
            let mut times = vec![];
            let layers: Vec<&crate::model::Layer> = match sel_layer.and_then(|l| c.layer(l)) {
                Some(l) => vec![l],
                None => c.layers.iter().collect(),
            };
            let only = pu(p, "prop");
            for l in layers {
                l.props.for_each_prop(&mut |pr| {
                    if only.is_none_or(|u| u == pr.uid) {
                        times.extend(pr.keys.iter().map(|k| l.comp_time(k.time)));
                    }
                });
            }
            let eps = c.frame_duration() * 0.5;
            let pick = if to == "nextKey" {
                times.iter().copied().filter(|t| *t > now + eps).fold(None, |a: Option<f64>, t| Some(a.map_or(t, |x| x.min(t))))
            } else {
                times.iter().copied().filter(|t| *t < now - eps).fold(None, |a: Option<f64>, t| Some(a.map_or(t, |x| x.max(t))))
            };
            match pick {
                Some(t) => t,
                None => return Ok(json!({"time": now})),
            }
        }
        _ => return Err(format!("unknown `to`: {to}")),
    };
    time_set(e, &json!({"time": t, "comp": cid}))
}

pub fn view_channel(e: &mut Engine, p: &Json) -> Res {
    e.state.viewer.channel = ps(p, "channel").unwrap_or("RGB").to_string();
    Ok(json!({"channel": e.state.viewer.channel}))
}

pub fn view_snapping(e: &mut Engine, p: &Json) -> Res {
    e.state.snapping = pb(p, "value").unwrap_or(!e.state.snapping);
    Ok(json!({"snapping": e.state.snapping}))
}

pub fn view_layout(e: &mut Engine, p: &Json) -> Res {
    e.state.view_layout = pu(p, "views").unwrap_or(1).clamp(1, 4) as u32;
    Ok(json!({"views": e.state.view_layout}))
}

pub fn view_roi(e: &mut Engine, p: &Json) -> Res {
    e.state.roi = p.get("rect").and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).collect::<Vec<_>>()).filter(|v| v.len() == 4).map(|v| [v[0], v[1], v[2], v[3]]);
    Ok(json!({"roi": e.state.roi}))
}

pub fn view_exposure(e: &mut Engine, p: &Json) -> Res {
    e.state.viewer.exposure = pf(p, "value").unwrap_or(0.0);
    Ok(json!({"exposure": e.state.viewer.exposure}))
}

pub fn add_guide(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let vertical = ps(p, "orientation").unwrap_or("horizontal") == "vertical";
    let pos = pf(p, "position").ok_or("pass `position`")?;
    e.proj().comp_mut(cid).ok_or("no comp")?.guides.push(Guide { vertical, position: pos });
    Ok(json!({"guides": e.project.comp(cid).map(|c| c.guides.len())}))
}

pub fn clear_guides(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    e.proj().comp_mut(cid).ok_or("no comp")?.guides.clear();
    Ok(json!({"guides": 0}))
}

pub fn command_list(_: &mut Engine, p: &Json) -> Res {
    let filter = ps(p, "filter").unwrap_or("").to_ascii_lowercase();
    let schemas = pb(p, "schemas").unwrap_or(false);
    let rows: Vec<Json> = registry::all()
        .iter()
        .filter(|c| filter.is_empty() || c.id.to_ascii_lowercase().contains(&filter) || c.label.to_ascii_lowercase().contains(&filter))
        .map(|c| {
            let mut o = json!({
                "id": c.id, "label": c.label, "menu": registry::menu_path(c), "shortcut": if c.key.is_empty() { Json::Null } else { json!(c.key) },
                "params": c.params, "enabled": true, "undoable": c.undo, "query": c.query,
            });
            if schemas {
                o["schema"] = registry::schema(c);
            }
            o
        })
        .collect();
    Ok(Json::Array(rows))
}

pub fn command_describe(_: &mut Engine, p: &Json) -> Res {
    let id = ps(p, "command").or_else(|| ps(p, "id")).ok_or("pass `command`")?;
    let c = registry::find(id).ok_or_else(|| format!("unknown command `{id}`"))?;
    Ok(json!({"id": c.id, "label": c.label, "menu": registry::menu_path(c), "params": c.params, "schema": registry::schema(c), "shortcut": if c.key.is_empty() { Json::Null } else { json!(c.key) }, "undoable": c.undo, "enabled": true, "why": null}))
}

pub fn capabilities(e: &mut Engine, _: &Json) -> Res {
    Ok(e.capabilities())
}

pub fn batch(e: &mut Engine, p: &Json) -> Res {
    e.batch(p)
}

pub fn prefs_pages(e: &mut Engine, _: &Json) -> Res {
    use crate::render::roto;
    let installed = roto::installed();
    let active = e.prefs.get("rotoModel").and_then(Json::as_str).unwrap_or("classical").to_string();
    let status = if installed {
        format!("MobileSAM is installed in {}. Roto Brush uses the {} model; Object Matte always uses MobileSAM.", roto::models_dir().display(), if active == "mobilesam" { "MobileSAM" } else { "Classic" })
    } else {
        format!("Roto Brush uses the Classic model. Object Matte needs MobileSAM ({} MB, {} licence), downloaded once into {}.", roto::MOBILESAM.size / 1_000_000, roto::MOBILESAM.licence, roto::models_dir().display())
    };
    let mut roto_items = vec![json!({"section": "Roto Brush and Object Matte"}), json!({"note": status})];
    if installed {
        roto_items.push(json!({"button": "Use MobileSAM for Roto Brush", "command": "roto.model.select", "params": {"id": "mobilesam", "wait": true}}));
        roto_items.push(json!({"button": "Use Classic for Roto Brush", "command": "roto.model.select", "params": {"id": "classical"}}));
        roto_items.push(json!({"button": "Remove MobileSAM", "command": "roto.model.remove", "params": {"id": "mobilesam"}}));
    } else {
        roto_items.push(json!({"button": "Download MobileSAM", "command": "roto.model.download", "params": {"id": "mobilesam"}}));
    }
    let st = crate::render::diskcache::stats();
    let used = st.get("bytes").and_then(Json::as_u64).unwrap_or(0) as f64 / 1e9;
    let files = st.get("files").and_then(Json::as_u64).unwrap_or(0);
    let disk = vec![
        json!({"section": "Disk Cache"}),
        json!({"key": "diskCache.enabled", "label": "Enable Disk Cache", "type": "bool"}),
        json!({"key": "diskCache.folder", "label": "Folder", "type": "text"}),
        json!({"key": "diskCache.maxGB", "label": "Maximum Disk Cache Size", "type": "number", "min": 0.1, "unit": "GB"}),
        json!({"note": format!("{files} cached frames, {used:.2} GB. Frames that take a while to render are kept here, so previews of unchanged compositions come back at once, even after a restart.")}),
        json!({"button": "Empty Disk Cache", "command": "cache.cleanDisk", "params": {}}),
    ];
    let profiles: Vec<Json> = crate::render::color::SPACES.iter().filter(|s| matches!(s.id, "srgb" | "p3" | "rec709" | "rec2020" | "adobeRgb" | "dciP3" | "p3d65")).map(|s| json!({"label": s.name, "value": s.id})).collect();
    let previews = vec![
        json!({"section": "Color"}),
        json!({"key": "previews.displayProfile", "label": "Display Profile", "type": "choice", "options": profiles}),
        json!({"note": "Used by display colour management when the project has a working space (File > Project Settings). View > Use Display Color Management and Simulate Output change what the viewer shows; renders are not affected."}),
    ];
    let fm = &crate::render::facemesh::MODEL;
    let learned = super::track::learned_face(e);
    let mut face_items = vec![json!({"section": "Face Tracking"})];
    if crate::render::facemesh::installed() {
        face_items.push(json!({"note": format!("The {} model is installed in {}. Face Tracking (Outline Only and Detailed Features) uses {}.", fm.name, crate::render::roto::models_dir().display(), if learned { "it: a learned face detector and a 478-point face mesh" } else { "the classical detector" })}));
        face_items.push(if learned { json!({"button": "Use the Classical Detector", "command": "face.model.select", "params": {"id": "classical"}}) } else { json!({"button": "Use the Face Landmarker", "command": "face.model.select", "params": {"id": fm.id}}) });
        face_items.push(json!({"button": "Remove the Face Landmarker", "command": "face.model.remove", "params": {"id": fm.id}}));
    } else {
        face_items.push(json!({"note": format!("Face Tracking uses the classical detector. The {} ({:.1} MB, {} licence) finds faces at any angle and tracks 478 points; it is downloaded once into {}.", fm.name, fm.size as f64 / 1_000_000.0, fm.licence, crate::render::roto::models_dir().display())}));
        face_items.push(json!({"button": "Download the Face Landmarker", "command": "face.model.download", "params": {"id": fm.id}}));
    }
    Ok(json!([{"id": "general", "items": super::session::pref_items(e)}, {"id": "previews", "items": previews}, {"id": "disk", "items": disk}, {"id": "memory", "items": []}, {"id": "roto", "items": roto_items}, {"id": "face", "items": face_items}]))
}

pub fn prefs_get(e: &mut Engine, p: &Json) -> Res {
    let k = ps(p, "key").ok_or("pass `key`")?;
    if let Some(v) = crate::render::diskcache::value(k) {
        return Ok(v);
    }
    Ok(e.prefs.get(k).cloned().or_else(|| super::session::default_pref(k)).unwrap_or(Json::Null))
}

pub fn prefs_set(e: &mut Engine, p: &Json) -> Res {
    let k = ps(p, "key").ok_or("pass `key`")?.to_string();
    let v = p.get("value").cloned().unwrap_or(Json::Null);
    if crate::render::diskcache::persisted(&k) {
        crate::render::diskcache::configure(&k, &v);
        crate::render::diskcache::save_pref(&k, &v);
    }
    e.prefs.insert(k, v);
    Ok(json!({"ok": true}))
}

pub fn disk_stats(_: &mut Engine, _: &Json) -> Res {
    Ok(crate::render::diskcache::stats())
}

pub fn disk_clean(_: &mut Engine, _: &Json) -> Res {
    Ok(json!({"freed": crate::render::diskcache::clean()}))
}

pub fn shortcuts_list(e: &mut Engine, _: &Json) -> Res {
    let taken: Vec<String> = e.session.param_shortcuts.iter().map(|x| x.0.clone()).collect();
    let mut rows: Vec<Json> = e
        .session
        .param_shortcuts
        .iter()
        .map(|(k, c, p)| {
            let label = registry::find(c).map(|x| x.label).unwrap_or("");
            json!({"key": format!("{c}|{k}"), "command": c, "params": p, "label": format!("{label}: {}", p.get("name").and_then(Json::as_str).unwrap_or("")), "path": "", "scope": "global", "defaults": [], "keys": [k]})
        })
        .collect();
    let base: Vec<Json> = registry::all()
        .iter()
        .map(|c| {
            let defaults: Vec<String> = if c.key.is_empty() { vec![] } else { c.key.split(" or ").map(str::to_string).collect() };
            let keys: Vec<String> = e.shortcuts.get(c.id).cloned().unwrap_or_else(|| defaults.clone()).into_iter().filter(|k| !taken.contains(k)).collect();
            json!({"key": c.id, "command": c.id, "params": null, "label": c.label, "path": registry::menu_path(c), "scope": "global", "defaults": defaults, "keys": keys})
        })
        .collect();
    rows.extend(base);
    Ok(json!({"preset": e.shortcut_preset, "presets": super::viewer::presets(e), "commands": rows}))
}

pub fn shortcuts_set(e: &mut Engine, p: &Json) -> Res {
    let c = ps(p, "command").ok_or("pass `command`")?.to_string();
    let keys: Vec<String> = match p.get("keys") {
        Some(Json::Array(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        Some(Json::String(s)) => vec![s.clone()],
        _ => vec![],
    };
    for (_, v) in e.shortcuts.iter_mut() {
        v.retain(|k| !keys.contains(k));
    }
    e.shortcuts.insert(c, keys);
    Ok(json!({"ok": true}))
}

pub fn shortcuts_reset(e: &mut Engine, p: &Json) -> Res {
    match ps(p, "command") {
        Some(c) => {
            e.shortcuts.remove(c);
        }
        None => e.shortcuts.clear(),
    }
    Ok(json!({"ok": true}))
}

pub fn shortcuts_conflicts(_: &mut Engine, _: &Json) -> Res {
    Ok(json!([]))
}

pub fn layer_marker(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let comment = ps(p, "comment").unwrap_or("").to_string();
    let l = e.layer_mut(cid, lid)?;
    if l.switches.markers_locked {
        return Err("this layer's markers are locked (Layer > Markers > Lock Markers)".into());
    }
    l.markers.push(crate::model::Marker { time: t, comment, ..crate::model::Marker::default() });
    Ok(json!({"time": t}))
}

pub fn keys_info(e: &mut Engine, _: &Json) -> Res {
    Ok(json!({"selected": e.state.selected_keys.iter().map(|k| json!({"layer": k.layer, "prop": k.prop, "time": k.time})).collect::<Vec<_>>()}))
}
