use serde_json::Value as Json;

use super::Engine;
use crate::model::{Item, ItemKind, Layer, LayerInit, LayerKind, Solid, Value, build_layer};

pub mod camtrack;
pub mod color;
pub mod comp;
pub mod deps;
pub mod edit;
pub mod effect;
pub mod essential;
pub mod expression;
pub mod file;
pub mod fill;
pub mod formats;
pub mod keys;
pub mod keytools;
pub mod layer;
pub mod layermisc;
pub mod markers;
pub mod mask;
pub mod maskshape;
pub mod paint;
pub mod project;
pub mod ptemplates;
pub mod puppet;
pub mod replace;
pub mod roto;
pub mod prop;
pub mod script;
pub mod session;
pub mod shapes;
pub mod styles;
pub mod text;
pub mod textanim;
pub mod three;
pub mod time;
pub mod timeline;
pub mod track;
pub mod view3d;
pub mod viewer;
pub mod warp;
pub mod watch;

pub fn select_only(e: &mut Engine, ids: &[u64]) {
    e.state.selected_layers = ids.to_vec();
    e.state.selected_props.clear();
    e.state.selected_keys.clear();
    e.state.selected_vertices.clear();
}

pub fn add_layer(e: &mut Engine, cid: u64, layer: Layer, index: Option<usize>) -> u64 {
    let id = layer.id;
    let sel = e.state.selected_layers.first().copied();
    let comp = e.proj().comp_mut(cid).expect("comp");
    let at = match index {
        Some(i) => i.min(comp.layers.len()),
        None => sel.and_then(|s| comp.index_of(s)).unwrap_or(0),
    };
    comp.layers.insert(at, layer);
    id
}

pub fn unique_layer_name(comp: &crate::model::Comp, base: &str) -> String {
    if !comp.layers.iter().any(|l| l.name == base) {
        return base.to_string();
    }
    let stem = base.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end();
    let mut n = 2;
    loop {
        let cand = format!("{stem} {n}");
        if !comp.layers.iter().any(|l| l.name == cand) {
            return cand;
        }
        n += 1;
    }
}

pub fn solid_item(e: &mut Engine, name: &str, color: [f64; 3], w: u32, h: u32) -> u64 {
    let pj = e.proj();
    let folder = pj.folder_named("Solids");
    let id = pj.new_id();
    let name = pj.unique_item_name(name);
    pj.items.push(Item { id, name, parent: Some(folder), label: 1, comment: String::new(), kind: ItemKind::Solid(Solid { color, width: w, height: h, pixel_aspect: 1.0 }), proxy: None });
    id
}

pub fn footage_layer(e: &mut Engine, cid: u64, item: u64, time: f64, pos: Option<[f64; 2]>, duration: Option<f64>) -> Result<Layer, String> {
    let it = e.project.item(item).ok_or("no such item")?.clone();
    let comp = e.project.comp(cid).ok_or("no composition")?.clone();
    if cid == item {
        return Err("a composition cannot contain itself".into());
    }
    if let ItemKind::Comp(_) = it.kind
        && contains(&e.project, item, cid, 0)
    {
        return Err("that would nest the composition inside itself".into());
    }
    let (kind, size, dur) = match &it.kind {
        ItemKind::Comp(c) => (LayerKind::Precomp, (c.width as f64, c.height as f64), c.duration),
        ItemKind::Footage(f) if f.kind == crate::model::FootageKind::Model => (LayerKind::Model, (f.width as f64, f.height as f64), duration.unwrap_or(comp.duration - time)),
        ItemKind::Footage(f) => {
            let still = f.is_still() || !f.has_video && f.duration <= 0.0;
            (LayerKind::Footage, (f.width as f64, f.height as f64), if still { duration.unwrap_or(comp.duration - time) } else { f.duration * f.loops.max(1) as f64 })
        }
        ItemKind::Solid(s) => (LayerKind::Solid, (s.width as f64, s.height as f64), duration.unwrap_or(comp.duration - time)),
        ItemKind::Folder => return Err("folders cannot be added to a composition".into()),
    };
    let name = it.name.clone();
    let mut l = build_layer(e.proj(), (comp.width, comp.height), LayerInit { kind, name, source: Some(item), size, start: time, duration: dur.max(comp.frame_duration()) });
    if let Some(p) = pos
        && let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut("position"))
    {
        pr.value = Value::Vec3([p[0], p[1], 0.0]);
    }
    if l.out_point > comp.duration && kind != LayerKind::Footage && kind != LayerKind::Precomp {
        l.out_point = comp.duration;
    }
    Ok(l)
}

fn contains(p: &crate::model::Project, outer: u64, target: u64, depth: u32) -> bool {
    if depth > 32 {
        return true;
    }
    let Some(c) = p.comp(outer) else { return false };
    c.layers.iter().any(|l| l.kind == LayerKind::Precomp && l.source.is_some_and(|s| s == target || contains(p, s, target, depth + 1)))
}

pub fn vec2_param(p: &Json, k: &str) -> Option<[f64; 2]> {
    let a = p.get(k)?.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
}
