use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pb, ps};
use crate::model::{Item, ItemKind};

fn item_ref(e: &Engine, v: &Json) -> Option<u64> {
    match v {
        Json::Number(n) => n.as_u64().filter(|id| e.project.item(*id).is_some()),
        Json::String(s) => s.parse::<u64>().ok().filter(|id| e.project.item(*id).is_some()).or_else(|| e.project.items.iter().find(|i| i.name == *s).map(|i| i.id)),
        _ => None,
    }
}

fn items_of(e: &Engine, p: &Json) -> Vec<u64> {
    match p.get("items").and_then(Json::as_array) {
        Some(a) => a.iter().filter_map(|v| item_ref(e, v)).collect(),
        None => p.get("item").and_then(|v| item_ref(e, v)).map(|x| vec![x]).unwrap_or_else(|| e.state.project_selection.clone()),
    }
}

pub fn summary(e: &mut Engine, _: &Json) -> Res {
    let items: Vec<Json> = e.project.items.iter().map(|i| json!({"id": i.id, "name": i.name, "type": i.type_name(), "parent": i.parent})).collect();
    Ok(json!({
        "path": e.path,
        "dirty": e.is_dirty(),
        "items": items,
        "activeComp": e.state.active_comp,
        "time": e.now(),
        "selectedLayers": e.state.selected_layers,
        "bitDepth": format!("{} bpc", e.project.settings.bit_depth),
        "undo": e.history.undo.iter().rev().take(20).map(|s| s.label.clone()).collect::<Vec<_>>(),
        "renderer": {"active": "cpu", "renderer": "formvfx CPU", "gpuAcceleration": false},
        "importedFrom": e.imported_from,
    }))
}

pub fn select(e: &mut Engine, p: &Json) -> Res {
    let ids = items_of(e, p);
    if pb(p, "add").unwrap_or(false) {
        for id in ids {
            if let Some(i) = e.state.project_selection.iter().position(|x| *x == id) {
                e.state.project_selection.remove(i);
            } else {
                e.state.project_selection.push(id);
            }
        }
    } else {
        e.state.project_selection = ids;
    }
    Ok(json!({"items": e.state.project_selection}))
}

pub fn rename(e: &mut Engine, p: &Json) -> Res {
    let id = p.get("item").and_then(|v| item_ref(e, v)).ok_or("pass `item`")?;
    let name = ps(p, "name").ok_or("pass `name`")?.to_string();
    e.proj().item_mut(id).ok_or("no such item")?.name = name.clone();
    Ok(json!({"item": id, "name": name}))
}

pub fn delete(e: &mut Engine, p: &Json) -> Res {
    let ids = items_of(e, p);
    if ids.is_empty() {
        return Err("select project items or pass `items`".into());
    }
    let mut all = ids.clone();
    let mut i = 0;
    while i < all.len() {
        let id = all[i];
        for it in e.project.items.iter() {
            if it.parent == Some(id) && !all.contains(&it.id) {
                all.push(it.id);
            }
        }
        i += 1;
    }
    let pj = e.proj();
    pj.items.retain(|it| !all.contains(&it.id));
    for it in pj.items.iter_mut() {
        if let ItemKind::Comp(c) = &mut it.kind {
            c.layers.retain(|l| !l.source.is_some_and(|s| all.contains(&s)));
        }
    }
    pj.render_queue.retain(|r| !all.contains(&r.comp));
    e.state.project_selection.clear();
    e.state.open_comps.retain(|c| !all.contains(c));
    if e.state.active_comp.is_some_and(|c| all.contains(&c)) {
        e.state.active_comp = e.state.open_comps.last().copied();
    }
    Ok(json!({"deleted": all.len()}))
}

pub fn new_folder(e: &mut Engine, p: &Json) -> Res {
    let pj = e.proj();
    let name = pj.unique_item_name(ps(p, "name").unwrap_or("Untitled 1"));
    let id = pj.new_id();
    pj.items.push(Item { id, name, parent: None, label: 0, comment: String::new(), kind: ItemKind::Folder, proxy: None });
    e.state.project_selection = vec![id];
    Ok(json!({"item": id}))
}

pub fn move_items(e: &mut Engine, p: &Json) -> Res {
    let ids = items_of(e, p);
    let folder = match p.get("folder") {
        None | Some(Json::Null) => None,
        Some(v) => Some(item_ref(e, v).ok_or("no such folder")?),
    };
    if let Some(f) = folder
        && !e.project.item(f).is_some_and(|i| matches!(i.kind, ItemKind::Folder))
    {
        return Err("`folder` is not a folder".into());
    }
    let pj = e.proj();
    for id in &ids {
        if Some(*id) == folder {
            continue;
        }
        if let Some(it) = pj.item_mut(*id) {
            it.parent = folder;
        }
    }
    Ok(json!({"moved": ids.len()}))
}

pub fn set_comment(e: &mut Engine, p: &Json) -> Res {
    let ids = items_of(e, p);
    let c = ps(p, "comment").unwrap_or("").to_string();
    for id in &ids {
        if let Some(it) = e.proj().item_mut(*id) {
            it.comment = c.clone();
        }
    }
    Ok(json!({"items": ids.len()}))
}

pub fn usage(e: &mut Engine, p: &Json) -> Res {
    let ids = items_of(e, p);
    let out: Vec<Json> = ids.iter().map(|id| json!({"item": id, "uses": e.project.uses_item(*id)})).collect();
    Ok(json!(out))
}
