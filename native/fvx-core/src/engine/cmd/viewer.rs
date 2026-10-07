use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Value as Json, json};

use crate::engine::state::FrameJob;
use crate::engine::{Engine, LockedViewer, Res, Snapshot, pb, pf, ps};
use crate::model::Guide;

pub const DEFAULT_PRESET: &str = "formvfx Default";

pub fn take_snapshot(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let t = e.time_of(cid);
    let scale = pf(p, "scale").unwrap_or(1.0).clamp(0.05, 1.0);
    let (w, h, px) = FrameJob::new(e, cid, t, scale, false, false).run();
    e.snapshot_seq += 1;
    let id = e.snapshot_seq;
    e.state.snapshot = Some(Snapshot { id, comp: cid, time: t, width: w, height: h, px: Arc::new(px) });
    Ok(json!({"id": id, "comp": cid, "time": t, "width": w, "height": h}))
}

pub fn show_snapshot(e: &mut Engine, p: &Json) -> Res {
    if e.state.snapshot.is_none() {
        return Err("take a snapshot first (Shift+F5)".into());
    }
    e.state.show_snapshot = pb(p, "value").unwrap_or(!e.state.show_snapshot);
    Ok(json!({"showSnapshot": e.state.show_snapshot}))
}

fn view_label(v: &str) -> String {
    match v {
        "activeCamera" => "Active Camera".into(),
        other => {
            let mut s = String::new();
            for (i, ch) in other.chars().enumerate() {
                if i == 0 {
                    s.extend(ch.to_uppercase());
                } else if ch.is_uppercase() {
                    s.push(' ');
                    s.push(ch);
                } else {
                    s.push(ch);
                }
            }
            s
        }
    }
}

pub fn split_locked_viewer(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let name = e.project.item(cid).map(|i| i.name.clone()).unwrap_or_default();
    let view = ps(p, "view").unwrap_or("activeCamera").to_string();
    e.state.locked_viewer = Some(LockedViewer { comp: cid, name: name.clone(), view: view.clone(), time: e.time_of(cid) });
    Ok(json!({"comp": cid, "name": name, "view": view}))
}

pub fn close_locked_viewer(e: &mut Engine, _: &Json) -> Res {
    let was = e.state.locked_viewer.take().is_some();
    Ok(json!({"closed": was}))
}

pub fn locked_json(e: &Engine) -> Json {
    match &e.state.locked_viewer {
        Some(l) if e.project.comp(l.comp).is_some() => json!({"comp": l.comp, "name": l.name, "view": l.view, "label": view_label(&l.view), "time": e.time_of(l.comp)}),
        _ => Json::Null,
    }
}

pub fn snapshot_json(e: &Engine) -> Json {
    match &e.state.snapshot {
        Some(s) => json!({"comp": s.comp, "time": s.time, "width": s.width, "height": s.height, "id": s.id}),
        None => Json::Null,
    }
}

pub fn export_guides(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path`")?.to_string();
    let (_, c) = e.comp_of(p)?;
    let doc = json!({"format": "formvfx.guides/1", "width": c.width, "height": c.height, "guides": c.guides.iter().map(|g| json!({"vertical": g.vertical, "position": g.position})).collect::<Vec<_>>()});
    std::fs::write(&path, serde_json::to_string_pretty(&doc).map_err(|x| x.to_string())?).map_err(|err| format!("{path}: {err}"))?;
    Ok(json!({"path": path, "guides": c.guides.len()}))
}

pub fn import_guides(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path`")?.to_string();
    let text = std::fs::read_to_string(&path).map_err(|err| format!("{path}: {err}"))?;
    let doc: Json = serde_json::from_str(&text).map_err(|err| format!("{path}: {err}"))?;
    let list = doc.get("guides").and_then(Json::as_array).ok_or("no `guides` in that file")?;
    let gs: Vec<Guide> = list
        .iter()
        .filter_map(|g| {
            let pos = g.get("position").and_then(Json::as_f64)?;
            let vertical = g.get("vertical").and_then(Json::as_bool).unwrap_or_else(|| g.get("orientation").and_then(Json::as_str) == Some("vertical"));
            Some(Guide { vertical, position: pos })
        })
        .collect();
    let cid = e.comp_id(p.get("comp"))?;
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    let n = gs.len();
    c.guides = gs;
    e.set_label("Import Guides");
    Ok(json!({"guides": n}))
}

fn store_current(e: &mut Engine) {
    let cur = e.shortcut_preset.clone();
    e.shortcut_presets.insert(cur, e.shortcuts.clone());
}

pub fn presets(e: &Engine) -> Vec<String> {
    let mut v = vec![DEFAULT_PRESET.to_string()];
    v.extend(e.shortcut_presets.keys().filter(|k| *k != DEFAULT_PRESET).cloned());
    if !v.contains(&e.shortcut_preset) {
        v.push(e.shortcut_preset.clone());
    }
    v
}

pub fn shortcuts_preset(e: &mut Engine, p: &Json) -> Res {
    let op = ps(p, "op").ok_or("pass `op` (select, new, duplicate, delete or rename)")?;
    let name = ps(p, "name").map(str::to_string);
    store_current(e);
    match op {
        "select" => {
            let n = name.ok_or("pass `name`")?;
            if n != DEFAULT_PRESET && !e.shortcut_presets.contains_key(&n) {
                return Err(format!("no shortcut preset \"{n}\""));
            }
            e.shortcuts = e.shortcut_presets.get(&n).cloned().unwrap_or_default();
            e.shortcut_preset = n;
        }
        "new" | "duplicate" => {
            let n = name.ok_or("pass `name`")?;
            if n == DEFAULT_PRESET || e.shortcut_presets.contains_key(&n) && n != e.shortcut_preset {
                return Err(format!("a preset named \"{n}\" exists"));
            }
            let from = if op == "new" { HashMap::new() } else { e.shortcut_presets.get(ps(p, "from").unwrap_or(&e.shortcut_preset)).cloned().unwrap_or_default() };
            e.shortcut_presets.insert(n.clone(), from.clone());
            e.shortcuts = from;
            e.shortcut_preset = n;
        }
        "delete" => {
            let n = name.unwrap_or_else(|| e.shortcut_preset.clone());
            if n == DEFAULT_PRESET {
                return Err("the default preset cannot be deleted".into());
            }
            e.shortcut_presets.remove(&n);
            if e.shortcut_preset == n {
                e.shortcut_preset = DEFAULT_PRESET.into();
                e.shortcuts = e.shortcut_presets.get(DEFAULT_PRESET).cloned().unwrap_or_default();
            }
        }
        "rename" => {
            let n = name.unwrap_or_else(|| e.shortcut_preset.clone());
            let to = ps(p, "newName").ok_or("pass `newName`")?.to_string();
            if n == DEFAULT_PRESET {
                return Err("the default preset cannot be renamed".into());
            }
            let m = e.shortcut_presets.remove(&n).unwrap_or_default();
            e.shortcut_presets.insert(to.clone(), m);
            if e.shortcut_preset == n {
                e.shortcut_preset = to;
            }
        }
        _ => return Err(format!("unknown op `{op}`")),
    }
    Ok(json!({"preset": e.shortcut_preset, "presets": presets(e)}))
}

pub fn shortcuts_export(e: &mut Engine, p: &Json) -> Res {
    store_current(e);
    let name = ps(p, "preset").unwrap_or(&e.shortcut_preset).to_string();
    let map = e.shortcut_presets.get(&name).cloned().unwrap_or_default();
    let doc = json!({"format": "formvfx.shortcuts/1", "name": name, "shortcuts": map});
    match ps(p, "path") {
        Some(path) => {
            std::fs::write(path, serde_json::to_string_pretty(&doc).map_err(|x| x.to_string())?).map_err(|err| format!("{path}: {err}"))?;
            Ok(json!({"path": path, "preset": name}))
        }
        None => Ok(doc),
    }
}

pub fn shortcuts_import(e: &mut Engine, p: &Json) -> Res {
    let doc = match (p.get("preset").filter(|x| x.is_object()), ps(p, "path")) {
        (Some(d), _) => d.clone(),
        (None, Some(path)) => {
            let text = std::fs::read_to_string(path).map_err(|err| format!("{path}: {err}"))?;
            serde_json::from_str(&text).map_err(|err| format!("{path}: {err}"))?
        }
        _ => return Err("pass `path` or `preset`".into()),
    };
    let map: HashMap<String, Vec<String>> = doc.get("shortcuts").and_then(|s| serde_json::from_value(s.clone()).ok()).ok_or("no `shortcuts` in that document")?;
    let mut name = ps(p, "name").or_else(|| doc.get("name").and_then(Json::as_str)).unwrap_or("Imported").to_string();
    if name == DEFAULT_PRESET {
        name = format!("{name} (imported)");
    }
    store_current(e);
    e.shortcut_presets.insert(name.clone(), map.clone());
    e.shortcuts = map;
    e.shortcut_preset = name;
    Ok(json!({"preset": e.shortcut_preset, "presets": presets(e)}))
}
