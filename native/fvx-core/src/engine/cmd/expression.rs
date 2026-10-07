use serde_json::{Value as Json, json};

use super::prop::locate;
use crate::engine::{Engine, Res, pb, pf};
use crate::eval::Eval;
use crate::model::key::{self, Key};
use crate::model::Prop;

fn errors_in(e: &Engine, cid: u64, t: f64) -> Result<Vec<Json>, String> {
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let ev = Eval::new(&e.project, cid, comp, t);
    let mut out = vec![];
    for (i, l) in comp.layers.iter().enumerate() {
        let mut props: Vec<&Prop> = vec![];
        l.props.for_each_prop(&mut |p| {
            if crate::expr::active(p) {
                props.push(p);
            }
        });
        for p in props {
            if let Some(err) = crate::expr::check(&ev, l, p) {
                out.push(json!({"layer": l.id, "index": i + 1, "layerName": l.name, "prop": p.uid, "path": l.props.path_of(p.uid), "name": p.name, "error": err}));
            }
        }
    }
    Ok(out)
}

pub fn errors(e: &mut Engine, p: &Json) -> Res {
    let (cid, _) = e.comp_of(p)?;
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let list = errors_in(e, cid, t)?;
    Ok(json!({"time": t, "count": list.len(), "errors": list}))
}

pub fn reveal(e: &mut Engine, p: &Json) -> Res {
    let (cid, _) = e.comp_of(p)?;
    let t = e.time_of(cid);
    let list = errors_in(e, cid, t)?;
    let mut layers: Vec<u64> = list.iter().filter_map(|x| x["layer"].as_u64()).collect();
    layers.dedup();
    if !layers.is_empty() {
        super::select_only(e, &layers);
        e.state.selected_props = list.iter().filter_map(|x| Some((x["layer"].as_u64()?, x["prop"].as_u64()?))).collect();
    }
    Ok(json!({"count": list.len(), "errors": list}))
}

pub fn enable(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select layers or pass `layers`".into());
    }
    let on = pb(p, "enabled").ok_or("pass `enabled` (true or false)")?;
    let mut n = 0;
    for lid in ids {
        let l = e.layer_mut(cid, lid)?;
        l.props.for_each_prop_mut(&mut |pr| {
            if let Some(x) = pr.expr.as_mut() {
                x.enabled = on;
                n += 1;
            }
        });
    }
    e.set_label(if on { "Enable Expressions" } else { "Disable Expressions" });
    Ok(json!({"changed": n, "enabled": on}))
}

fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn reference(l: &crate::model::Layer, uid: u64, same_layer: bool) -> Option<String> {
    let path = l.props.path_of(uid)?;
    let parts: Vec<&str> = path.split('/').collect();
    let head = if same_layer { String::new() } else { format!("thisComp.layer({}).", quoted(&l.name)) };
    let ae = |id: &str| -> String {
        match id {
            "anchor" => "anchorPoint".into(),
            "rotationX" => "xRotation".into(),
            "rotationY" => "yRotation".into(),
            "rotation" if l.is_3d() => "zRotation".into(),
            "poi" => "pointOfInterest".into(),
            other => other.into(),
        }
    };
    let name_at = |depth: usize| -> Option<String> {
        let sub = parts[..=depth].join("/");
        Some(match l.props.resolve(&sub)? {
            crate::model::NodeRef::Prop(p) => p.name.clone(),
            crate::model::NodeRef::Group(g) => g.name.clone(),
        })
    };
    let body = match parts.as_slice() {
        ["transform", id] => format!("transform.{}", ae(id)),
        ["effects", _, _] => format!("effect({})({})", quoted(&name_at(1)?), quoted(&name_at(2)?)),
        ["masks", _, id] => format!("mask({}).mask{}{}", quoted(&name_at(1)?), id[..1].to_uppercase(), &id[1..]),
        ["text", "sourceText"] => "text.sourceText".into(),
        _ => {
            let mut s = if same_layer { "thisLayer".to_string() } else { head.trim_end_matches('.').to_string() };
            for d in 0..parts.len() {
                s.push_str(&format!("({})", quoted(&name_at(d)?)));
            }
            return Some(s);
        }
    };
    Some(format!("{head}{body}"))
}

pub fn pick_whip(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let target = p.get("target").ok_or("pass `target: {layer, path}`")?;
    let mut tp = target.clone();
    if let Some(o) = tp.as_object_mut()
        && !o.contains_key("comp")
        && let Some(c) = p.get("comp")
    {
        o.insert("comp".into(), c.clone());
    }
    let (tc, tl, tu) = locate(e, &tp)?;
    if tc != cid {
        return Err("the target must be in the same composition".into());
    }
    if tl == lid && tu == uid {
        return Err("a property cannot pick whip to itself".into());
    }
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let target_layer = comp.layer(tl).ok_or("no target layer")?;
    if target_layer.props.find(tu).is_none() {
        return Err("the target is a group; pick a property".into());
    }
    let text = reference(target_layer, tu, tl == lid).ok_or("cannot write a reference to that property")?;
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("that is a group, not a property")?;
    pr.expr = Some(crate::model::prop::Expr { text: text.clone(), enabled: true });
    e.set_label("Pick Whip");
    Ok(json!({"expression": text}))
}

pub fn edit_dropdown(e: &mut Engine, p: &Json) -> Res {
    let items: Vec<String> = p.get("items").and_then(Json::as_array).ok_or("pass `items` (a list of menu entries)")?.iter().filter_map(|v| v.as_str().map(str::to_string)).filter(|s| !s.trim().is_empty()).collect();
    if items.is_empty() {
        return Err("pass at least one menu item".into());
    }
    let (cid, lid) = e.layer_param(p)?;
    let layer = e.layer_ref(cid, lid)?;
    let picked = if p.get("path").is_some() || p.get("prop").is_some() { Some(e.prop_uid(layer, p)?) } else { None };
    let mut uid = None;
    let mut consider = |g: &crate::model::Group| {
        for c in &g.children {
            if let crate::model::Node::Prop(pr) = c
                && matches!(pr.ui, crate::model::Ui::Popup { .. })
                && uid.is_none()
            {
                uid = Some(pr.uid);
            }
        }
    };
    match picked {
        Some(u) if layer.props.find(u).is_some() => uid = Some(u),
        Some(u) => {
            if let Some(g) = layer.props.find_group(u) {
                consider(g);
            }
        }
        None => {
            if let Some(fx) = layer.effects() {
                for g in fx.groups().filter(|g| matches!(&g.kind, crate::model::GroupKind::Effect { effect } if effect == "fx.control.dropdown")) {
                    consider(g);
                }
            }
        }
    }
    let uid = uid.ok_or("no Dropdown Menu Control here (pass `path` to one)")?;
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("no such property")?;
    let crate::model::Ui::Popup { options } = &mut pr.ui else {
        return Err(format!("`{}` is not a dropdown menu", pr.name));
    };
    *options = items.clone();
    let last = items.len() as u32 - 1;
    if let crate::model::Value::Enum(i) = &mut pr.value {
        *i = (*i).min(last);
    }
    for k in pr.keys.iter_mut() {
        if let crate::model::Value::Enum(i) = &mut k.value {
            *i = (*i).min(last);
        }
    }
    e.set_label("Edit Dropdown");
    Ok(json!({"items": items}))
}

pub fn convert_to_keys(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let l = comp.layer(lid).ok_or("no layer")?;
    let pr = l.props.find(uid).ok_or("that is a group, not a property")?;
    if !crate::expr::active(pr) {
        return Err(format!("`{}` has no enabled expression", pr.name));
    }
    let fd = comp.frame_duration();
    let start = l.in_point.max(0.0);
    let end = l.out_point.min(comp.duration);
    if end <= start {
        return Err("the layer is not inside the composition's duration".into());
    }
    let frames = ((end - start) / fd).ceil() as usize;
    if frames > 100_000 {
        return Err("the layer is too long to bake frame by frame".into());
    }
    let mut keys: Vec<Key> = Vec::with_capacity(frames);
    for i in 0..frames {
        let t = start + i as f64 * fd;
        let ev = Eval::new(&e.project, cid, comp, t);
        let v = ev.value(l, pr);
        let mut k = Key::new(l.layer_time(t), v, pr.spatial);
        k.spatial_auto = false;
        keys.push(k);
    }
    let n = keys.len();
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("that is a group")?;
    pr.keys.clear();
    for k in keys {
        key::insert(&mut pr.keys, k);
    }
    if let Some(x) = pr.expr.as_mut() {
        x.enabled = false;
    }
    e.set_label("Convert Expression to Keyframes");
    Ok(json!({"keys": n}))
}
