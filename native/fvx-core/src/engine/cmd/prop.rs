use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pb, pf, ps};
use crate::eval::Eval;
use crate::model::key::{self, Key, same_time};
use crate::model::prop::{Expr, GroupKind, Node};
use crate::model::{Prop, Value};

pub fn locate(e: &Engine, p: &Json) -> Result<(u64, u64, u64), String> {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    let uid = e.prop_uid(l, p)?;
    Ok((cid, lid, uid))
}

pub fn prop_of<'a>(e: &'a Engine, cid: u64, lid: u64, uid: u64) -> Result<&'a Prop, String> {
    e.layer_ref(cid, lid)?.props.find(uid).ok_or_else(|| "that is a group, not a property".to_string())
}

fn options(pr: &Prop) -> Vec<String> {
    pr.ui.options().to_vec()
}

pub fn get(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let comp = e.project.comp(cid).ok_or("no comp")?;
    let l = comp.layer(lid).ok_or("no layer")?;
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let ev = Eval::new(&e.project, cid, comp, t);
    if let Some(pr) = l.props.find(uid) {
        let v = ev.value(l, pr);
        return Ok(json!({
            "path": l.props.path_of(uid), "name": pr.name, "type": v.kind_name(), "value": v.to_json(),
            "animated": pr.is_animated(), "expression": pr.expr.as_ref().map(|x| json!({"text": x.text, "enabled": x.enabled})),
            "keys": pr.keys.iter().map(|k| json!({"time": l.comp_time(k.time), "layerTime": k.time, "value": k.value.to_json(), "in": format!("{:?}", k.in_interp).to_lowercase(), "out": format!("{:?}", k.out_interp).to_lowercase()})).collect::<Vec<_>>(),
            "options": if options(pr).is_empty() { Json::Null } else { json!(options(pr)) },
        }));
    }
    let g = l.props.find_group(uid).ok_or("no such property")?;
    Ok(json!({"path": l.props.path_of(uid), "name": g.name, "group": true, "children": g.children.iter().map(|c| c.id().to_string()).collect::<Vec<_>>()}))
}

pub fn set_value_at(pr: &mut Prop, v: Value, lt: Option<f64>) {
    let v = pr.ui.clamp(v);
    match lt {
        Some(t) if !pr.keys.is_empty() => {
            key::insert(&mut pr.keys, Key::new(t, v, pr.spatial));
        }
        _ => pr.value = v,
    }
}

pub fn set(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let now = e.time_of(cid);
    let jv = p.get("value").ok_or("pass `value`")?.clone();
    let l = e.layer_ref(cid, lid)?;
    let lt = pf(p, "time").unwrap_or_else(|| l.layer_time(now));
    let pr = l.props.find(uid).ok_or("that is a group, not a property")?;
    if pr.static_only && pr.is_animated() {
        return Err(format!("{} cannot be animated", pr.name));
    }
    let cur = pr.at(lt);
    let v = cur.parse_like(&jv, &options(pr)).map_err(|err| format!("{}: {err}", pr.name))?;
    let name = pr.name.clone();
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("no property")?;
    set_value_at(pr, v.clone(), Some(lt));
    e.set_label(format!("Change {name}"));
    Ok(json!({"value": v.to_json()}))
}

pub fn add_key(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let now = e.time_of(cid);
    let l = e.layer_ref(cid, lid)?;
    let lt = pf(p, "time").unwrap_or_else(|| l.layer_time(now));
    let pr = l.props.find(uid).ok_or("that is a group, not a property")?;
    if pr.static_only {
        return Err(format!("{} cannot be animated", pr.name));
    }
    let cur = pr.at(lt);
    let v = match p.get("value") {
        Some(j) if !j.is_null() => cur.parse_like(j, &options(pr)).map_err(|err| format!("{}: {err}", pr.name))?,
        _ => cur,
    };
    let hold = pr.hold_only || !v.is_numeric() && !matches!(v, Value::Path(_) | Value::Gradient(_));
    let name = pr.name.clone();
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("no property")?;
    let v = pr.ui.clamp(v);
    let mut k = Key::new(lt, v, pr.spatial);
    if hold {
        k.in_interp = crate::model::Interp::Hold;
        k.out_interp = crate::model::Interp::Hold;
    }
    let i = key::insert(&mut pr.keys, k);
    e.set_label(format!("Add {name} Keyframe"));
    Ok(json!({"index": i + 1, "time": lt}))
}

pub fn remove_key(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let now = e.time_of(cid);
    let l = e.layer_ref(cid, lid)?;
    let lt = pf(p, "time").unwrap_or_else(|| l.layer_time(now));
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("no property")?;
    let before = pr.keys.len();
    let val = pr.at(lt);
    pr.keys.retain(|k| !same_time(k.time, lt));
    if pr.keys.is_empty() && before > 0 {
        pr.value = val;
    }
    Ok(json!({"removed": before - pr.keys.len()}))
}

pub fn toggle_key(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let now = e.time_of(cid);
    let l = e.layer_ref(cid, lid)?;
    let lt = l.layer_time(now);
    let pr = l.props.find(uid).ok_or("that is a group")?;
    if pr.keys.iter().any(|k| same_time(k.time, lt)) {
        remove_key(e, &json!({"layer": lid, "prop": uid, "time": lt, "comp": cid}))
    } else {
        add_key(e, &json!({"layer": lid, "prop": uid, "time": lt, "comp": cid}))
    }
}

pub fn toggle_animation(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let now = e.time_of(cid);
    let l = e.layer_ref(cid, lid)?;
    let lt = l.layer_time(now);
    let pr = l.props.find(uid).ok_or("that is a group")?;
    if pr.static_only {
        return Err(format!("{} cannot be animated", pr.name));
    }
    if pr.is_animated() {
        let v = pr.at(lt);
        let l = e.layer_mut(cid, lid)?;
        let pr = l.props.find_mut(uid).ok_or("no property")?;
        pr.keys.clear();
        pr.value = v;
        e.state.selected_keys.retain(|k| k.prop != uid);
        Ok(json!({"animated": false}))
    } else {
        add_key(e, &json!({"layer": lid, "prop": uid, "time": lt, "comp": cid}))?;
        Ok(json!({"animated": true}))
    }
}

pub fn reset(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let l = e.layer_mut(cid, lid)?;
    if let Some(pr) = l.props.find_mut(uid) {
        pr.keys.clear();
        pr.value = pr.default.clone();
        return Ok(json!({"reset": uid}));
    }
    let g = l.props.find_group_mut(uid).ok_or("no such property")?;
    g.for_each_prop_mut(&mut |pr| {
        pr.keys.clear();
        pr.value = pr.default.clone();
    });
    Ok(json!({"reset": uid}))
}

pub fn select(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let add = pb(p, "add").unwrap_or(false);
    if add {
        if let Some(i) = e.state.selected_props.iter().position(|x| *x == (lid, uid)) {
            e.state.selected_props.remove(i);
        } else {
            e.state.selected_props.push((lid, uid));
        }
    } else {
        e.state.selected_props = vec![(lid, uid)];
        e.state.selected_keys.clear();
    }
    if !e.state.selected_layers.contains(&lid) {
        if add {
            e.state.selected_layers.push(lid);
        } else {
            e.state.selected_layers = vec![lid];
        }
    }
    if pb(p, "selectKeys").unwrap_or(true)
        && let Some(pr) = e.layer_ref(cid, lid)?.props.find(uid)
    {
        let keys: Vec<crate::engine::KeyRef> = pr.keys.iter().map(|k| crate::engine::KeyRef { layer: lid, prop: uid, time: k.time }).collect();
        if add {
            e.state.selected_keys.extend(keys);
        } else {
            e.state.selected_keys = keys;
        }
    }
    Ok(json!({"props": e.state.selected_props.iter().map(|x| json!([x.0, x.1])).collect::<Vec<_>>()}))
}

pub fn set_expression(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let text = ps(p, "expression").or_else(|| ps(p, "text")).map(str::to_string);
    let enabled = pb(p, "enabled");
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("that is a group")?;
    match (text, enabled) {
        (Some(t), en) => {
            if t.trim().is_empty() {
                pr.expr = None;
            } else {
                pr.expr = Some(Expr { text: t, enabled: en.unwrap_or(true) });
            }
        }
        (None, Some(en)) => {
            if let Some(x) = pr.expr.as_mut() {
                x.enabled = en;
            }
        }
        (None, None) => {
            pr.expr = if pr.expr.is_some() { None } else { Some(Expr { text: "value".into(), enabled: true }) };
        }
    }
    let out = pr.expr.as_ref().map(|x| json!({"text": x.text, "enabled": x.enabled}));
    e.set_label("Expression");
    let t = e.time_of(cid);
    let comp = e.project.comp(cid).ok_or("no comp")?;
    let l = comp.layer(lid).ok_or("no layer")?;
    let error = l.props.find(uid).and_then(|pr| crate::expr::check(&Eval::new(&e.project, cid, comp, t), l, pr));
    Ok(json!({"expression": out, "error": error}))
}

pub fn remove_group(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let l = e.layer_mut(cid, lid)?;
    let parent = l.props.parent_of_mut(uid).ok_or("no such group")?;
    if !matches!(parent.kind, GroupKind::Indexed) {
        return Err("only masks, effects, shape items and animators can be removed".into());
    }
    parent.children.retain(|c| c.uid() != uid);
    e.state.selected_props.retain(|x| x.1 != uid);
    Ok(json!({"removed": uid}))
}

pub fn duplicate_group(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let src = e.layer_ref(cid, lid)?.props.find_group(uid).cloned().ok_or("no such group")?;
    let mut copy = src.clone();
    {
        let pj = e.proj();
        copy.renumber(&mut pj.next_uid);
    }
    let base = copy.name.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end().to_string();
    let l = e.layer_mut(cid, lid)?;
    let parent = l.props.parent_of_mut(uid).ok_or("no such group")?;
    if !matches!(parent.kind, GroupKind::Indexed) {
        return Err("this group cannot be duplicated".into());
    }
    let n = parent.children.iter().filter(|c| c.name().starts_with(&base)).count() + 1;
    copy.name = format!("{base} {n}");
    let nu = copy.uid;
    let i = parent.children.iter().position(|c| c.uid() == uid).unwrap_or(0);
    parent.children.insert(i + 1, Node::Group(copy));
    Ok(json!({"group": nu}))
}

pub fn rename_group(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let name = ps(p, "name").ok_or("pass `name`")?.to_string();
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no such group")?;
    g.name = name.clone();
    Ok(json!({"name": name}))
}
