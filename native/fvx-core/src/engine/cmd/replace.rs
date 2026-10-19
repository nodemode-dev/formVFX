use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pf, ps, pu};
use crate::model::value::parse_color;
use crate::model::{Item, ItemKind, LayerKind, Solid};

fn target(e: &Engine, p: &Json) -> Result<u64, String> {
    let id = pu(p, "item")
        .or_else(|| p.get("items").and_then(Json::as_array).and_then(|a| a.first()).and_then(Json::as_u64))
        .or_else(|| e.state.project_selection.iter().copied().find(|i| e.project.item(*i).is_some_and(|it| matches!(it.kind, ItemKind::Footage(_) | ItemKind::Solid(_)))))
        .ok_or("select the footage to replace (or pass `item`)")?;
    match e.project.item(id).map(|i| &i.kind) {
        Some(ItemKind::Footage(_) | ItemKind::Solid(_)) => Ok(id),
        Some(_) => Err("only footage and solids can be replaced".into()),
        None => Err(format!("no item {id}")),
    }
}

fn retype(e: &mut Engine, id: u64, to: LayerKind) -> usize {
    let mut n = 0;
    for it in e.proj().items.iter_mut() {
        if let ItemKind::Comp(c) = &mut it.kind {
            for l in c.layers.iter_mut().filter(|l| l.source == Some(id) && matches!(l.kind, LayerKind::Footage | LayerKind::Solid | LayerKind::Precomp)) {
                l.kind = to;
                n += 1;
            }
        }
    }
    n
}

fn forget(e: &Engine, id: u64) {
    if let Some(ItemKind::Footage(f)) = e.project.item(id).map(|i| &i.kind) {
        e.media.forget(&f.path);
    }
}

fn dims(e: &Engine, id: u64, p: &Json) -> (u32, u32, f64, f64) {
    let it = e.project.item(id);
    let (w, h) = it.and_then(Item::dimensions).unwrap_or((1920, 1080));
    let fps = it.and_then(Item::frame_rate).unwrap_or(30.0);
    let dur = it.and_then(Item::duration).unwrap_or(0.0);
    (pu(p, "width").map(|x| x as u32).unwrap_or(w), pu(p, "height").map(|x| x as u32).unwrap_or(h), pf(p, "frameRate").unwrap_or(fps), pf(p, "duration").unwrap_or(dur))
}

pub fn import_placeholder(e: &mut Engine, p: &Json) -> Res {
    let name = ps(p, "name").map(str::trim).filter(|s| !s.is_empty()).unwrap_or("Placeholder");
    let f = super::timeline::placeholder(pu(p, "width").unwrap_or(1920) as u32, pu(p, "height").unwrap_or(1080) as u32, pf(p, "frameRate").unwrap_or(30.0), pf(p, "duration").unwrap_or(30.0));
    let folder = e.state.project_selection.iter().copied().find(|i| e.project.item(*i).is_some_and(|it| matches!(it.kind, ItemKind::Folder)));
    let pj = e.proj();
    let id = pj.new_id();
    let name = pj.unique_item_name(name);
    pj.items.push(Item { id, name: name.clone(), parent: folder, label: 1, comment: String::new(), kind: ItemKind::Footage(f), proxy: None });
    e.state.project_selection = vec![id];
    e.set_label("Import Placeholder");
    Ok(json!({"item": id, "name": name}))
}

pub fn replace_with_placeholder(e: &mut Engine, p: &Json) -> Res {
    let id = target(e, p)?;
    let (w, h, fps, dur) = dims(e, id, p);
    forget(e, id);
    let f = super::timeline::placeholder(w, h, fps, dur);
    let layers = retype(e, id, LayerKind::Footage);
    let pj = e.proj();
    let it = pj.item_mut(id).ok_or("no such item")?;
    it.kind = ItemKind::Footage(f);
    if let Some(n) = ps(p, "name") {
        it.name = n.to_string();
    }
    e.set_label("Replace with Placeholder");
    Ok(json!({"item": id, "layers": layers, "width": w, "height": h}))
}

pub fn replace_with_solid(e: &mut Engine, p: &Json) -> Res {
    let id = target(e, p)?;
    let (w, h, _, _) = dims(e, id, p);
    let c = p.get("color").and_then(parse_color).unwrap_or([0.5, 0.5, 0.5, 1.0]);
    forget(e, id);
    let layers = retype(e, id, LayerKind::Solid);
    let pj = e.proj();
    let it = pj.item_mut(id).ok_or("no such item")?;
    it.kind = ItemKind::Solid(Solid { color: [c[0], c[1], c[2]], width: w.max(1), height: h.max(1), pixel_aspect: 1.0 });
    it.proxy = None;
    if let Some(n) = ps(p, "name") {
        it.name = n.to_string();
    }
    e.set_label("Replace with Solid");
    Ok(json!({"item": id, "layers": layers, "width": w, "height": h}))
}

pub fn replace_with_layered_comp(e: &mut Engine, p: &Json) -> Res {
    let id = target(e, p)?;
    let (path, parent) = match e.project.item(id) {
        Some(Item { kind: ItemKind::Footage(f), parent, .. }) => (f.path.clone(), *parent),
        _ => return Err("only PSD, PDF and .ai footage can become a layered composition".into()),
    };
    let lower = path.to_ascii_lowercase();
    let (comp, _) = if lower.ends_with(".psd") || lower.ends_with(".psb") {
        super::formats::import_psd_comp(e, &path, true, parent)?
    } else if lower.ends_with(".pdf") || lower.ends_with(".ai") {
        super::formats::import_pdf_comp(e, &path, &json!({"importAs": "composition"}), parent)?
    } else {
        return Err(format!("{path} has no layers to build a composition from (PSD, PDF and .ai files do)"));
    };
    let mut layers = 0;
    for it in e.proj().items.iter_mut() {
        if it.id == comp {
            continue;
        }
        if let ItemKind::Comp(c) = &mut it.kind {
            for l in c.layers.iter_mut().filter(|l| l.source == Some(id)) {
                l.kind = LayerKind::Precomp;
                l.source = Some(comp);
                layers += 1;
            }
        }
    }
    forget(e, id);
    let pj = e.proj();
    pj.items.retain(|i| i.id != id);
    e.state.project_selection = vec![comp];
    e.set_label("Replace with Layered Comp");
    Ok(json!({"comp": comp, "layers": layers, "removed": id}))
}
