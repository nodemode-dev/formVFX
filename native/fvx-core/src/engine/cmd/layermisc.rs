use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pb, pf, ps, pu};
use crate::geom::Mat3;
use crate::model::key::{Key, insert};
use crate::model::path::BezPath;
use crate::model::prop::{GroupKind, MaskMode};
use crate::model::value::parse_color;
use crate::model::{ItemKind, Quality, Solid, Value};

pub fn set_transform(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let which = ps(p, "prop").ok_or("pass `prop` (anchor, position, scale, orientation, rotation or opacity)")?.to_string();
    let jv = p.get("value").ok_or("pass `value`")?.clone();
    let now = e.time_of(cid);
    let mut n = 0;
    for lid in ids {
        let l = e.layer_mut(cid, lid)?;
        let lt = l.layer_time(now);
        let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut(&which)) else { continue };
        let cur = pr.at(lt);
        let jv2 = match (&cur, &jv) {
            (Value::Vec3(c), Json::Array(a)) if a.len() == 2 => json!([a[0], a[1], c[2]]),
            (Value::Vec3(_), Json::Number(x)) => json!([x, x, x]),
            _ => jv.clone(),
        };
        let v = cur.parse_like(&jv2, &[]).map_err(|err| format!("{}: {err}", pr.name))?;
        super::prop::set_value_at(pr, v, Some(lt));
        n += 1;
    }
    e.set_label(format!("Change {which}"));
    Ok(json!({"layers": n}))
}

pub fn settings(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let src = e.layer_ref(cid, lid)?.source;
    let solid = src.and_then(|s| e.project.item(s)).and_then(|it| if let ItemKind::Solid(s) = &it.kind { Some(s.clone()) } else { None });
    if let Some(n) = ps(p, "name") {
        e.layer_mut(cid, lid)?.name = n.to_string();
    }
    let Some(mut s) = solid else {
        e.set_label("Layer Settings");
        return Ok(json!({"layer": lid}));
    };
    let mut item = src.ok_or("no source")?;
    if let Some(c) = p.get("color").and_then(parse_color) {
        s.color = [c[0], c[1], c[2]];
    }
    if let Some(w) = pu(p, "width") {
        s.width = (w as u32).max(1);
    }
    if let Some(h) = pu(p, "height") {
        s.height = (h as u32).max(1);
    }
    if let Some(a) = pf(p, "pixelAspect") {
        s.pixel_aspect = a.max(0.01);
    }
    let shared = e.project.uses_item(item) > 1;
    if shared && !pb(p, "affectAll").unwrap_or(true) {
        let name = e.layer_ref(cid, lid)?.name.clone();
        item = super::solid_item(e, &name, s.color, s.width, s.height);
        e.layer_mut(cid, lid)?.source = Some(item);
    }
    let it = e.proj().item_mut(item).ok_or("no item")?;
    it.kind = ItemKind::Solid(Solid { ..s });
    if let Some(n) = ps(p, "name") {
        it.name = n.to_string();
    }
    e.set_label("Solid Settings");
    Ok(json!({"layer": lid, "solid": item}))
}

pub fn reveal_source(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    let src = l.source.ok_or_else(|| format!("{} has no source item", l.name))?;
    e.state.project_selection = vec![src];
    e.frontend("window.panel", json!({"panel": "project"}));
    Ok(json!({"item": src}))
}

pub fn quality(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let q = match ps(p, "quality").unwrap_or("best").to_ascii_lowercase().as_str() {
        "draft" => Quality::Draft,
        "wireframe" => Quality::Wireframe,
        _ => Quality::Best,
    };
    for lid in &ids {
        e.layer_mut(cid, *lid)?.switches.quality = q;
    }
    e.set_label("Quality");
    Ok(json!({"quality": format!("{q:?}"), "layers": ids}))
}

pub fn frame_blending(e: &mut Engine, p: &Json) -> Res {
    use crate::model::FrameBlend;
    let (cid, ids) = e.layers_param(p)?;
    let m = match ps(p, "mode").unwrap_or("frameMix") {
        "off" => FrameBlend::Off,
        "frameMix" => FrameBlend::FrameMix,
        "pixelMotion" => FrameBlend::PixelMotion,
        other => return Err(format!("mode is off, frameMix or pixelMotion, not {other}")),
    };
    for lid in &ids {
        e.layer_mut(cid, *lid)?.switches.frame_blend = m;
    }
    if m != FrameBlend::Off
        && let Some(c) = e.proj().comp_mut(cid)
    {
        c.frame_blending = true;
    }
    e.set_label("Frame Blending");
    Ok(json!({"mode": format!("{m:?}"), "layers": ids}))
}

pub fn mask_uid(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<u64, String> {
    let l = e.layer_ref(cid, lid)?;
    let masks = l.masks().ok_or("this layer has no masks")?;
    match p.get("mask") {
        Some(Json::Number(n)) => {
            let n = n.as_u64().unwrap_or(0);
            if masks.groups().any(|g| g.uid == n) { Ok(n) } else { masks.groups().nth((n as usize).wrapping_sub(1)).map(|g| g.uid).ok_or_else(|| "no such mask".into()) }
        }
        Some(Json::String(s)) => masks.groups().find(|g| g.name == *s).map(|g| g.uid).ok_or_else(|| format!("no mask \"{s}\"")),
        _ => e
            .state
            .selected_props
            .iter()
            .filter(|(l2, _)| *l2 == lid)
            .find_map(|(_, u)| masks.groups().find(|g| g.uid == *u || g.get("path").is_some_and(|pp| pp.uid == *u)).map(|g| g.uid))
            .or_else(|| masks.groups().next().map(|g| g.uid))
            .ok_or_else(|| "this layer has no masks".into()),
    }
}

fn with_mask(e: &mut Engine, p: &Json, f: impl FnOnce(&mut crate::model::Group) -> Result<(), String>) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = mask_uid(e, cid, lid, p)?;
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no such mask")?;
    f(g)?;
    Ok(json!({"layer": lid, "mask": uid}))
}

pub fn mask_shape(e: &mut Engine, p: &Json) -> Res {
    let r: Vec<f64> = p.get("rect").and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).collect()).filter(|v: &Vec<f64>| v.len() == 4).ok_or("pass `rect` [x, y, w, h]")?;
    let ellipse = ps(p, "shape") == Some("ellipse");
    let (cid, lid) = e.layer_param(p)?;
    let bp = if ellipse {
        BezPath::ellipse(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, r[2], r[3])
    } else {
        BezPath::polygon(&[[r[0], r[1]], [r[0] + r[2], r[1]], [r[0] + r[2], r[1] + r[3]], [r[0], r[1] + r[3]]], true)
    };
    let lt = e.layer_ref(cid, lid)?.layer_time(e.time_of(cid));
    let out = with_mask(e, p, |g| {
        let pr = g.get_mut("path").ok_or("no path")?;
        if pr.keys.is_empty() {
            pr.value = Value::Path(bp);
        } else {
            insert(&mut pr.keys, Key::new(lt, Value::Path(bp), false));
        }
        Ok(())
    })?;
    e.set_label("Mask Shape");
    Ok(out)
}

pub fn mask_set(e: &mut Engine, p: &Json) -> Res {
    let field = ps(p, "field").ok_or("pass `field` (feather, opacity or expansion)")?.to_string();
    let jv = p.get("value").ok_or("pass `value`")?.clone();
    let (cid, lid) = e.layer_param(p)?;
    let now = e.time_of(cid);
    let lt = e.layer_ref(cid, lid)?.layer_time(now);
    let out = with_mask(e, p, |g| {
        let pr = g.get_mut(&field).ok_or_else(|| format!("masks have no `{field}`"))?;
        let jv = match (&pr.value, &jv) {
            (Value::Vec2(_), Json::Number(x)) => json!([x, x]),
            _ => jv.clone(),
        };
        let v = pr.at(lt).parse_like(&jv, &[])?;
        super::prop::set_value_at(pr, v, Some(lt));
        Ok(())
    })?;
    e.set_label("Mask Settings");
    Ok(out)
}

pub fn mask_reset(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let (w, h) = {
        let c = e.project.comp(cid).ok_or("no composition")?;
        e.project.source_size(c.layer(lid).ok_or("no layer")?, c)
    };
    let out = with_mask(e, p, |g| {
        g.for_each_prop_mut(&mut |pr| {
            pr.keys.clear();
            pr.expr = None;
            pr.value = pr.default.clone();
        });
        if let Some(pr) = g.get_mut("path") {
            pr.value = Value::Path(BezPath::polygon(&[[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]], true));
        }
        if let GroupKind::Mask { mode, inverted, .. } = &mut g.kind {
            *mode = MaskMode::Add;
            *inverted = false;
        }
        Ok(())
    })?;
    e.set_label("Reset Mask");
    Ok(out)
}

pub fn mask_remove(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = mask_uid(e, cid, lid, p)?;
    let l = e.layer_mut(cid, lid)?;
    if let Some(m) = l.props.sub_mut("masks") {
        m.children.retain(|c| c.uid() != uid);
    }
    e.state.selected_vertices.retain(|v| v.mask != uid);
    e.set_label("Remove Mask");
    Ok(json!({"removed": uid}))
}

pub fn mask_remove_all(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    for lid in &ids {
        if let Some(m) = e.layer_mut(cid, *lid)?.props.sub_mut("masks") {
            m.children.clear();
        }
    }
    e.state.selected_vertices.retain(|v| !ids.contains(&v.layer));
    e.set_label("Remove All Masks");
    Ok(json!({"layers": ids}))
}

pub fn mask_mode(e: &mut Engine, p: &Json) -> Res {
    let m = ps(p, "mode").ok_or("pass `mode`")?;
    let mode = MaskMode::parse(m).ok_or_else(|| format!("unknown mask mode `{m}`"))?;
    let out = with_mask(e, p, |g| {
        if let GroupKind::Mask { mode: x, .. } = &mut g.kind {
            *x = mode;
        }
        Ok(())
    })?;
    e.set_label("Mask Mode");
    Ok(out)
}

pub fn mask_invert(e: &mut Engine, p: &Json) -> Res {
    let v = pb(p, "value");
    let out = with_mask(e, p, |g| {
        if let GroupKind::Mask { inverted, .. } = &mut g.kind {
            *inverted = v.unwrap_or(!*inverted);
        }
        Ok(())
    })?;
    e.set_label("Invert Mask");
    Ok(out)
}

pub fn mask_lock(e: &mut Engine, p: &Json) -> Res {
    let v = pb(p, "value");
    let out = with_mask(e, p, |g| {
        if let GroupKind::Mask { locked, .. } = &mut g.kind {
            *locked = v.unwrap_or(!*locked);
        }
        Ok(())
    })?;
    e.set_label("Lock Mask");
    Ok(out)
}

fn lock_all(e: &mut Engine, cid: u64, lid: u64, keep: Option<u64>, lock: bool) -> Result<(), String> {
    if let Some(m) = e.layer_mut(cid, lid)?.props.sub_mut("masks") {
        for g in m.groups_mut() {
            if let GroupKind::Mask { locked, .. } = &mut g.kind {
                *locked = if Some(g.uid) == keep { false } else { lock };
            }
        }
    }
    Ok(())
}

pub fn mask_unlock_all(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    for lid in &ids {
        lock_all(e, cid, *lid, None, false)?;
    }
    e.set_label("Unlock All Masks");
    Ok(json!({"layers": ids}))
}

pub fn mask_lock_others(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = mask_uid(e, cid, lid, p)?;
    lock_all(e, cid, lid, Some(uid), true)?;
    e.set_label("Lock Other Masks");
    Ok(json!({"mask": uid}))
}

pub fn free_transform(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let group = match pu(p, "mask") {
        Some(u) if e.layer_ref(cid, lid)?.props.find_group(u).is_some_and(|g| g.get("path").is_some()) => u,
        _ => mask_uid(e, cid, lid, p)?,
    };
    let now = e.time_of(cid);
    let l = e.layer_mut(cid, lid)?;
    let lt = l.layer_time(now);
    let g = l.props.find_group_mut(group).ok_or("no such mask or path")?;
    let pr = g.get_mut("path").ok_or("that group has no path")?;
    let bp = pr.at(lt).as_path().cloned().unwrap_or_default().normalized();
    let b = bp.bounds().ok_or("the path is empty")?;
    let anchor = super::vec2_param(p, "anchor").unwrap_or([(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0]);
    let (sx, sy) = match p.get("scale") {
        Some(Json::Array(a)) => (a.first().and_then(Json::as_f64).unwrap_or(100.0), a.get(1).and_then(Json::as_f64).unwrap_or(100.0)),
        Some(Json::Number(n)) => (n.as_f64().unwrap_or(100.0), n.as_f64().unwrap_or(100.0)),
        _ => (100.0, 100.0),
    };
    let rot = pf(p, "rotation").unwrap_or(0.0);
    let off = super::vec2_param(p, "offset").unwrap_or([0.0, 0.0]);
    let m = Mat3::translate(anchor[0] + off[0], anchor[1] + off[1]) * Mat3::rotate_deg(rot) * Mat3::scale(sx / 100.0, sy / 100.0) * Mat3::translate(-anchor[0], -anchor[1]);
    let nb = bp.transformed(&m);
    if pr.keys.is_empty() {
        pr.value = Value::Path(nb);
    } else {
        insert(&mut pr.keys, Key::new(lt, Value::Path(nb), false));
    }
    e.set_label("Free Transform Points");
    Ok(json!({"layer": lid, "mask": group}))
}
