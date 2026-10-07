use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pb, pf, ps};
use crate::model::prop::Node;
use crate::render::styles::{STYLES, style_group, style_kind, styles_group};

fn ensure_styles(e: &mut Engine, cid: u64, lid: u64) -> Result<(), String> {
    let has = e.layer_ref(cid, lid)?.props.sub("layerStyles").is_some();
    if has {
        return Ok(());
    }
    let g = {
        let pj = e.proj();
        let mut ids = pj.ids();
        styles_group(&mut ids)
    };
    let l = e.layer_mut(cid, lid)?;
    let at = l.props.children.iter().position(|c| c.id() == "transform").map(|i| i + 1).unwrap_or(l.props.children.len());
    l.props.children.insert(at, Node::Group(g));
    Ok(())
}

fn add_kind(e: &mut Engine, p: &Json, kind: &'static str) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select a layer to add the style to".into());
    }
    let mut out = vec![];
    for lid in ids {
        ensure_styles(e, cid, lid)?;
        let existing = e.layer_ref(cid, lid)?.props.sub("layerStyles").and_then(|s| s.groups().find(|g| g.id == kind).map(|g| g.uid));
        if let Some(uid) = existing {
            let l = e.layer_mut(cid, lid)?;
            if let Some(g) = l.props.find_group_mut(uid) {
                g.enabled = true;
            }
            out.push(json!({"layer": lid, "uid": uid}));
            continue;
        }
        let g = {
            let pj = e.proj();
            let mut ids_ = pj.ids();
            style_group(&mut ids_, kind).ok_or("unknown layer style")?
        };
        let uid = g.uid;
        let l = e.layer_mut(cid, lid)?;
        let s = l.props.sub_mut("layerStyles").ok_or("no layer styles")?;
        let order = |id: &str| STYLES.iter().position(|x| x.0 == id).map(|i| i + 1).unwrap_or(0);
        let mine = order(kind);
        let at = s.children.iter().position(|c| order(c.id()) > mine).unwrap_or(s.children.len());
        s.children.insert(at, Node::Group(g));
        out.push(json!({"layer": lid, "uid": uid}));
    }
    let label = STYLES.iter().find(|s| s.0 == kind).map(|s| s.1).unwrap_or("Layer Style");
    e.set_label(format!("Add {label}"));
    Ok(json!({"style": kind, "added": out}))
}

pub fn add(e: &mut Engine, p: &Json) -> Res {
    let name = ps(p, "style").ok_or("pass `style` (dropShadow, innerShadow, outerGlow, innerGlow, bevelEmboss, satin, colorOverlay, gradientOverlay or stroke)")?;
    let kind = style_kind(name).ok_or_else(|| format!("no layer style `{name}`"))?;
    add_kind(e, p, kind)
}

pub fn drop_shadow(e: &mut Engine, p: &Json) -> Res {
    add_kind(e, p, "dropShadow")
}

pub fn inner_shadow(e: &mut Engine, p: &Json) -> Res {
    add_kind(e, p, "innerShadow")
}

pub fn outer_glow(e: &mut Engine, p: &Json) -> Res {
    add_kind(e, p, "outerGlow")
}

pub fn inner_glow(e: &mut Engine, p: &Json) -> Res {
    add_kind(e, p, "innerGlow")
}

pub fn bevel_emboss(e: &mut Engine, p: &Json) -> Res {
    add_kind(e, p, "bevelEmboss")
}

pub fn satin(e: &mut Engine, p: &Json) -> Res {
    add_kind(e, p, "satin")
}

pub fn color_overlay(e: &mut Engine, p: &Json) -> Res {
    add_kind(e, p, "colorOverlay")
}

pub fn gradient_overlay(e: &mut Engine, p: &Json) -> Res {
    add_kind(e, p, "gradientOverlay")
}

pub fn stroke(e: &mut Engine, p: &Json) -> Res {
    add_kind(e, p, "stroke")
}

fn style_uid(e: &Engine, cid: u64, lid: u64, v: Option<&Json>) -> Result<u64, String> {
    let l = e.layer_ref(cid, lid)?;
    let s = l.props.sub("layerStyles").ok_or_else(|| format!("{} has no layer styles", l.name))?;
    match v {
        None | Some(Json::Null) => Ok(s.uid),
        Some(Json::Number(n)) => {
            let n = n.as_u64().unwrap_or(0);
            if n == s.uid || s.groups().any(|g| g.uid == n) { Ok(n) } else { Err(format!("no layer style {n}")) }
        }
        Some(Json::String(x)) => {
            if x == "layerStyles" || x.eq_ignore_ascii_case("layer styles") {
                return Ok(s.uid);
            }
            let kind = style_kind(x).unwrap_or(x.as_str());
            s.groups().find(|g| g.id == kind || g.name == *x).map(|g| g.uid).ok_or_else(|| format!("no layer style \"{x}\" on {}", l.name))
        }
        _ => Err("pass `style` (id, name or uid)".into()),
    }
}

pub fn remove(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = style_uid(e, cid, lid, p.get("style"))?;
    let l = e.layer_mut(cid, lid)?;
    let root = l.props.sub("layerStyles").map(|s| s.uid);
    if root == Some(uid) {
        l.props.children.retain(|c| c.id() != "layerStyles");
    } else if let Some(s) = l.props.sub_mut("layerStyles") {
        s.children.retain(|c| c.uid() != uid);
    }
    e.set_label("Remove Layer Style");
    Ok(json!({"removed": uid}))
}

pub fn remove_all(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let mut n = 0;
    for lid in &ids {
        let l = e.layer_mut(cid, *lid)?;
        let before = l.props.children.len();
        l.props.children.retain(|c| c.id() != "layerStyles");
        n += before - l.props.children.len();
    }
    e.set_label("Remove All Layer Styles");
    Ok(json!({"layers": n}))
}

pub fn toggle(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = style_uid(e, cid, lid, p.get("style"))?;
    let v = pb(p, "value").or_else(|| pb(p, "enabled"));
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no layer style")?;
    g.enabled = v.unwrap_or(!g.enabled);
    let on = g.enabled;
    e.set_label("Toggle Layer Style");
    Ok(json!({"enabled": on}))
}

pub fn show_all(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    for lid in &ids {
        let l = e.layer_mut(cid, *lid)?;
        if let Some(s) = l.props.sub_mut("layerStyles") {
            s.enabled = true;
            for g in s.groups_mut() {
                g.enabled = true;
            }
        }
    }
    e.set_label("Show All Layer Styles");
    Ok(json!({"layers": ids}))
}

pub fn convert_to_editable(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    for lid in &ids {
        ensure_styles(e, cid, *lid)?;
    }
    e.set_label("Convert to Editable Styles");
    Ok(json!({"layers": ids}))
}

pub fn global_light(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let (angle, alt) = (pf(p, "angle"), pf(p, "altitude"));
    if angle.is_none() && alt.is_none() {
        let c = e.project.comp(cid).ok_or("no composition")?;
        return Ok(json!({"angle": c.global_light[0], "altitude": c.global_light[1]}));
    }
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    if let Some(a) = angle {
        c.global_light[0] = a;
    }
    if let Some(a) = alt {
        c.global_light[1] = a.clamp(0.0, 90.0);
    }
    let gl = c.global_light;
    e.set_label("Global Light");
    Ok(json!({"angle": gl[0], "altitude": gl[1]}))
}

pub fn list(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    let styles: Vec<Json> = l.props.sub("layerStyles").map(|s| s.groups().map(|g| json!({"uid": g.uid, "style": g.id, "name": g.name, "enabled": g.enabled})).collect()).unwrap_or_default();
    Ok(json!({"layer": lid, "enabled": l.props.sub("layerStyles").map(|s| s.enabled), "styles": styles, "available": STYLES.iter().map(|s| json!({"id": s.0, "name": s.1})).collect::<Vec<_>>()}))
}
