use serde_json::{Value as Json, json};

use super::layer::{comp_dims, finish};
use super::{add_layer, footage_layer, select_only, unique_layer_name};
use crate::engine::{Engine, Res, pb, pf, ps};
use crate::eval::Eval;
use crate::model::prop::{self, Expr, Group, Node};
use crate::model::value::parse_color;
use crate::model::{AutoOrient, ItemKind, Layer, LayerInit, LayerKind, Value, build_layer};

fn set(g: &mut Group, id: &str, v: Value, lt: Option<f64>) {
    if let Some(pr) = g.get_mut(id) {
        super::prop::set_value_at(pr, v, lt);
    }
}

fn v3(p: &Json, k: &str) -> Option<[f64; 3]> {
    let a = p.get(k)?.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2).and_then(Json::as_f64).unwrap_or(0.0)])
}

fn enum_of(v: &Json, opts: &[&str]) -> Option<u32> {
    if let Some(n) = v.as_u64() {
        return Some(n as u32);
    }
    if let Some(b) = v.as_bool() {
        return Some(b as u32);
    }
    let s: String = v.as_str()?.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase();
    opts.iter().position(|o| o.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase() == s).map(|i| i as u32)
}

pub fn apply_camera(l: &mut Layer, p: &Json, cw: f64, lt: Option<f64>) -> Result<(), String> {
    if let Some(n) = ps(p, "name") {
        l.name = n.to_string();
    }
    match ps(p, "type") {
        Some("oneNode") => l.auto_orient = AutoOrient::Off,
        Some("twoNode") => l.auto_orient = AutoOrient::TowardsPointOfInterest,
        Some(t) => return Err(format!("camera type is oneNode or twoNode, not {t}")),
        None => {}
    }
    let preset = ps(p, "preset").and_then(|s| s.trim_end_matches("mm").parse::<f64>().ok());
    let focal = pf(p, "focalLength").or(preset);
    let zoom = pf(p, "zoom").or_else(|| focal.map(|f| cw * f / 36.0)).or_else(|| pf(p, "angleOfView").map(|a| cw / 2.0 / (a.to_radians() / 2.0).tan().max(1e-6)));
    let g = l.props.sub_mut("cameraOptions").ok_or("not a camera")?;
    let cur_zoom = g.get("zoom").map(|p| p.value.num()).unwrap_or(cw * 50.0 / 36.0);
    if let Some(z) = zoom {
        set(g, "zoom", Value::Num(z.max(1.0)), lt);
    }
    let zoom = zoom.unwrap_or(cur_zoom);
    if let Some(d) = pb(p, "dof") {
        set(g, "dof", Value::Bool(d), lt);
    }
    if let Some(f) = pf(p, "focusDistance") {
        set(g, "focusDistance", Value::Num(f.max(1.0)), lt);
    } else if pb(p, "lockToZoom").unwrap_or(false) {
        set(g, "focusDistance", Value::Num(zoom), lt);
    }
    if let Some(a) = pf(p, "aperture").or_else(|| pf(p, "fStop").map(|f| zoom / f.max(0.1))) {
        set(g, "aperture", Value::Num(a.max(0.0)), lt);
    }
    if let Some(b) = pf(p, "blurLevel") {
        set(g, "blurLevel", Value::Num(b.max(0.0)), lt);
    }
    let tr = l.props.sub_mut("transform").ok_or("no transform")?;
    if let Some(v) = v3(p, "position") {
        set(tr, "position", Value::Vec3(v), lt);
    }
    if let Some(v) = v3(p, "poi") {
        set(tr, "poi", Value::Vec3(v), lt);
    }
    Ok(())
}

pub fn apply_light(l: &mut Layer, p: &Json, lt: Option<f64>) -> Result<(), String> {
    if let Some(n) = ps(p, "name") {
        l.name = n.to_string();
    }
    let g = l.props.sub_mut("lightOptions").ok_or("not a light")?;
    if let Some(k) = p.get("kind").or_else(|| p.get("type")) {
        let k = enum_of(k, &prop::LIGHT_TYPES).ok_or_else(|| format!("light kind is one of {}", prop::LIGHT_TYPES.join(", ")))?;
        if let Some(pr) = g.get_mut("lightType") {
            pr.value = Value::Enum(k);
        }
    }
    if let Some(c) = p.get("color").and_then(parse_color) {
        set(g, "color", Value::Color(c), lt);
    }
    for (k, id) in [("intensity", "intensity"), ("coneAngle", "coneAngle"), ("coneFeather", "coneFeather"), ("radius", "radius"), ("falloffDistance", "falloffDistance"), ("shadowDarkness", "shadowDarkness"), ("shadowDiffusion", "shadowDiffusion"), ("environmentRotation", "environmentRotation")] {
        if let Some(v) = pf(p, k) {
            set(g, id, Value::Num(v), lt);
        }
    }
    if let Some(f) = p.get("falloff") {
        let v = enum_of(f, &["None", "Smooth", "Inverse Square Clamped"]).ok_or("falloff is None, Smooth or Inverse Square Clamped")?;
        set(g, "falloff", Value::Enum(v), lt);
    }
    if let Some(b) = pb(p, "castsShadows") {
        set(g, "castsShadows", Value::Bool(b), lt);
    }
    if let Some(s) = p.get("source") {
        set(g, "source", Value::Layer(s.as_u64()), lt);
    }
    let tr = l.props.sub_mut("transform").ok_or("no transform")?;
    if let Some(v) = v3(p, "position") {
        set(tr, "position", Value::Vec3(v), lt);
    }
    if let Some(v) = v3(p, "poi") {
        set(tr, "poi", Value::Vec3(v), lt);
    }
    Ok(())
}

fn settings(e: &mut Engine, p: &Json, camera: bool) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let (cw, ..) = comp_dims(e, cid)?;
    let now = e.time_of(cid);
    let l = e.layer_ref(cid, lid)?;
    if camera != l.is_camera() || (!camera && !l.is_light()) {
        return Err(if camera { "select a camera layer".into() } else { "select a light layer".into() });
    }
    let lt = Some(l.layer_time(now));
    let l = e.layer_mut(cid, lid)?;
    if camera {
        apply_camera(l, p, cw as f64, lt)?;
    } else {
        apply_light(l, p, lt)?;
    }
    e.set_label(if camera { "Camera Settings" } else { "Light Settings" });
    Ok(json!({"layer": lid}))
}

pub fn camera_settings(e: &mut Engine, p: &Json) -> Res {
    settings(e, p, true)
}

pub fn light_settings(e: &mut Engine, p: &Json) -> Res {
    settings(e, p, false)
}

pub fn renderer(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let want = ps(p, "renderer").or_else(|| ps(p, "value"));
    let Some(w) = want else {
        return Ok(json!({"renderer": e.project.comp(cid).map(|c| c.renderer.clone())}));
    };
    let r = match w.to_ascii_lowercase().replace([' ', '-', '_'], "").as_str() {
        "advanced3d" | "advanced" => "advanced3d",
        "classic3d" | "classic" => "classic3d",
        _ => return Err("renderer is advanced3d or classic3d".into()),
    };
    e.proj().comp_mut(cid).ok_or("no composition")?.renderer = r.to_string();
    e.set_label("Renderer");
    Ok(json!({"renderer": r}))
}

pub fn new_model(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let item = if let Some(path) = ps(p, "path") {
        let r = super::file::import(e, &json!({"paths": [path]}))?;
        r.get("items").and_then(Json::as_array).and_then(|a| a.first()).and_then(Json::as_u64).ok_or_else(|| format!("could not import {path}"))?
    } else {
        let v = p.get("item").ok_or("pass `item` (a 3D model footage item) or `path` (.gltf, .glb or .obj)")?;
        match v {
            Json::Number(n) => n.as_u64().unwrap_or(0),
            Json::String(s) => e.project.items.iter().find(|i| i.name == *s).map(|i| i.id).ok_or_else(|| format!("no item \"{s}\""))?,
            _ => return Err("`item` is an id or a name".into()),
        }
    };
    match e.project.item(item).map(|i| &i.kind) {
        Some(ItemKind::Footage(f)) if f.kind == crate::model::FootageKind::Model => {}
        _ => return Err("that item is not a 3D model (.gltf, .glb or .obj)".into()),
    }
    let time = pf(p, "time").unwrap_or(0.0);
    let mut l = footage_layer(e, cid, item, time, super::vec2_param(p, "position"), None)?;
    if let Some(n) = ps(p, "name") {
        l.name = n.to_string();
    }
    let index = p.get("index").and_then(Json::as_u64).map(|i| (i as usize).saturating_sub(1));
    let id = add_layer(e, cid, l, index);
    select_only(e, &[id]);
    e.set_label("New 3D Model Layer");
    Ok(json!({"layer": id, "item": item}))
}

fn material_params(g: &mut Group, p: &Json, lt: Option<f64>) -> Result<(), String> {
    let modes = prop::SHADOW_MODES;
    if let Some(v) = p.get("castsShadows") {
        set(g, "castsShadows", Value::Enum(enum_of(v, &modes).ok_or("castsShadows is off, on or only")?), lt);
    }
    if let Some(v) = p.get("acceptsShadows") {
        set(g, "acceptsShadows", Value::Enum(enum_of(v, &modes).ok_or("acceptsShadows is off, on or only")?), lt);
    }
    for k in ["acceptsLights", "appearsInReflections", "overrideMaterial"] {
        if let Some(b) = pb(p, k) {
            set(g, k, Value::Bool(b), lt);
        }
    }
    for k in ["lightTransmission", "ambient", "diffuse", "specularIntensity", "specularShininess", "metal", "reflectionIntensity"] {
        if let Some(v) = pf(p, k) {
            set(g, k, Value::Num(v), lt);
        }
    }
    let mut any_pbr = false;
    for k in ["metallic", "roughness"] {
        if let Some(v) = pf(p, k) {
            let v = if v <= 1.0 { v * 100.0 } else { v };
            set(g, k, Value::Num(v.clamp(0.0, 100.0)), lt);
            any_pbr = true;
        }
    }
    for k in ["baseColor", "emissive"] {
        if let Some(c) = p.get(k).and_then(parse_color) {
            set(g, k, Value::Color(c), lt);
            any_pbr = true;
        }
    }
    if any_pbr && p.get("overrideMaterial").is_none() {
        set(g, "overrideMaterial", Value::Bool(true), lt);
    }
    Ok(())
}

pub fn new_primitive(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let (cw, ch, dur, _) = comp_dims(e, cid)?;
    let kind = p.get("kind").and_then(|k| enum_of(k, &prop::PRIMITIVES)).ok_or_else(|| format!("kind is one of {}", prop::PRIMITIVES.join(", ").to_ascii_lowercase()))?;
    let base = ps(p, "name").map(str::to_string).unwrap_or_else(|| prop::PRIMITIVES[kind as usize].to_string());
    let name = unique_layer_name(e.project.comp(cid).ok_or("no composition")?, &base);
    let size = (ch as f64 * 0.4).round();
    let mut l = build_layer(e.proj(), (cw, ch), LayerInit { kind: LayerKind::Model, name, source: None, size: (size, size), start: 0.0, duration: dur });
    let geo = {
        let pj = e.proj();
        let mut ids = pj.ids();
        prop::primitive_group(&mut ids, kind, size)
    };
    l.props.children.insert(0, Node::Group(geo));
    if let Some(g) = l.props.sub_mut("geometry") {
        for k in ["width", "height", "depth", "radius", "tubeRadius", "segments", "rings"] {
            if let Some(v) = pf(p, k) {
                set(g, k, Value::Num(v), None);
            }
        }
    }
    if let Some(g) = l.props.sub_mut("materialOptions") {
        if let Some(pr) = g.get_mut("overrideMaterial") {
            pr.value = Value::Bool(true);
        }
        material_params(g, p, None)?;
    }
    if let Some(v) = v3(p, "position")
        && let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut("position"))
    {
        pr.value = Value::Vec3(v);
    }
    finish(e, cid, l, "New 3D Primitive")
}

pub fn environment(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select the layers to use as environment layers".into());
    }
    let mut out = vec![];
    for id in ids {
        let l = e.layer_mut(cid, id)?;
        let on = pb(p, "on").unwrap_or(!l.environment);
        l.environment = on;
        if on {
            l.switches.three_d = true;
        }
        out.push(json!({"layer": id, "environment": on}));
    }
    e.set_label("Environment Layer");
    Ok(json!({"layers": out}))
}

pub fn material_set(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select layers or pass `layers`".into());
    }
    let now = e.time_of(cid);
    for id in &ids {
        let lt = pf(p, "time").or_else(|| e.layer_ref(cid, *id).ok().map(|l| l.layer_time(now)));
        let l = e.layer_mut(cid, *id)?;
        let g = l.props.sub_mut("materialOptions").ok_or_else(|| format!("{} has no Material Options", l.name))?;
        material_params(g, p, lt)?;
    }
    e.set_label("Material Options");
    Ok(json!({"layers": ids}))
}

pub fn material_reset(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    for id in &ids {
        let l = e.layer_mut(cid, *id)?;
        if let Some(g) = l.props.sub_mut("materialOptions") {
            g.for_each_prop_mut(&mut |pr| {
                pr.keys.clear();
                pr.expr = None;
                pr.value = pr.default.clone();
            });
        }
    }
    e.set_label("Reset Material");
    Ok(json!({"layers": ids}))
}

pub fn material_reveal(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let src = e.layer_ref(cid, lid)?.source.ok_or("this layer has no source item (a primitive carries its own material)")?;
    e.state.project_selection = vec![src];
    e.frontend("window.panel", json!({"panel": "project"}));
    e.revision += 1;
    Ok(json!({"item": src}))
}

pub fn material_duplicate(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.len() < 2 {
        return Err("select the source layer first, then the layers to give its material".into());
    }
    let src = e.layer_ref(cid, ids[0])?.props.sub("materialOptions").cloned().ok_or("the first layer has no Material Options")?;
    for id in &ids[1..] {
        let l = e.layer_mut(cid, *id)?;
        if let Some(g) = l.props.sub_mut("materialOptions") {
            for c in &src.children {
                if let Node::Prop(sp) = c
                    && let Some(dp) = g.get_mut(&sp.id)
                {
                    dp.value = sp.value.clone();
                    dp.keys = sp.keys.clone();
                    dp.expr = sp.expr.clone();
                }
            }
        }
    }
    e.set_label("Duplicate and Assign Material");
    Ok(json!({"source": ids[0], "layers": &ids[1..]}))
}

fn camera_of(e: &Engine, cid: u64, p: &Json) -> Result<u64, String> {
    let comp = e.project.comp(cid).ok_or("no composition")?;
    if let Some(v) = p.get("camera") {
        let id = e.layer_in(comp, v)?;
        return if comp.layer(id).is_some_and(|l| l.is_camera()) { Ok(id) } else { Err("that layer is not a camera".into()) };
    }
    if let Some(id) = e.state.selected_layers.iter().copied().find(|id| comp.layer(*id).is_some_and(|l| l.is_camera())) {
        return Ok(id);
    }
    let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
    ev.camera_layer().map(|l| l.id).ok_or_else(|| "the composition has no camera".into())
}

fn focus_expr(e: &mut Engine, cid: u64, cam: u64, text: Option<String>, value: Option<f64>) -> Res {
    let now = e.time_of(cid);
    let lt = e.layer_ref(cid, cam)?.layer_time(now);
    let l = e.layer_mut(cid, cam)?;
    let g = l.props.sub_mut("cameraOptions").ok_or("not a camera")?;
    let pr = g.get_mut("focusDistance").ok_or("no Focus Distance")?;
    if let Some(t) = text {
        pr.expr = Some(Expr { text: t, enabled: true });
    }
    if let Some(v) = value {
        super::prop::set_value_at(pr, Value::Num(v.max(1.0)), Some(lt));
    }
    if let Some(d) = g.get_mut("dof") {
        d.value = Value::Bool(true);
    }
    Ok(json!({"camera": cam}))
}

pub fn link_focus_poi(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let cam = camera_of(e, cid, p)?;
    let r = focus_expr(e, cid, cam, Some("length(transform.position, transform.pointOfInterest)".into()), None)?;
    e.set_label("Link Focus Distance to Point of Interest");
    Ok(r)
}

fn target_layer(e: &Engine, cid: u64, p: &Json, cam: u64) -> Result<u64, String> {
    let comp = e.project.comp(cid).ok_or("no composition")?;
    if let Some(v) = p.get("layer") {
        return e.layer_in(comp, v);
    }
    e.state.selected_layers.iter().copied().find(|id| *id != cam && comp.layer(*id).is_some_and(|l| !l.is_camera())).ok_or_else(|| "select the layer to focus on as well as the camera, or pass `layer`".into())
}

pub fn link_focus_layer(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let cam = camera_of(e, cid, p)?;
    let target = target_layer(e, cid, p, cam)?;
    let name = e.layer_ref(cid, target)?.name.replace('"', "\\\"");
    let text = format!("m = thisComp.layer(\"{name}\");\np = m.toWorld(m.transform.anchorPoint);\nv = sub(p, toWorld([0, 0, 0]));\nf = sub(toWorld([0, 0, 1]), toWorld([0, 0, 0]));\nMath.abs(dot(v, normalize(f)))");
    let r = focus_expr(e, cid, cam, Some(text), None)?;
    e.set_label("Link Focus Distance to Layer");
    Ok(r)
}

pub fn set_focus_layer(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let cam = camera_of(e, cid, p)?;
    let target = target_layer(e, cid, p, cam)?;
    let d = {
        let comp = e.project.comp(cid).ok_or("no composition")?;
        let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
        let c = comp.layer(cam).ok_or("no camera")?;
        let t = comp.layer(target).ok_or("no layer")?;
        let a = ev.tr_v3(t, "anchor", [0.0; 3]);
        let pt = ev.world_matrix(t).apply(a);
        let cw = ev.world_matrix(c);
        let eye = cw.apply([0.0; 3]);
        let f = crate::render::light::norm(crate::render::light::sub(cw.apply([0.0, 0.0, 1.0]), eye));
        crate::render::light::dot(crate::render::light::sub(pt, eye), f).abs()
    };
    let r = focus_expr(e, cid, cam, None, Some(d))?;
    e.set_label("Set Focus Distance to Layer");
    Ok(r)
}

pub fn orbit_null(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let cam = match p.get("layer") {
        Some(_) => {
            let (_, id) = e.layer_param(p)?;
            id
        }
        None => camera_of(e, cid, p)?,
    };
    let (poi, name) = {
        let comp = e.project.comp(cid).ok_or("no composition")?;
        let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
        let c = comp.layer(cam).ok_or("no camera")?;
        if !c.is_camera() {
            return Err("select a camera".into());
        }
        (ev.tr_v3(c, "poi", [comp.width as f64 / 2.0, comp.height as f64 / 2.0, 0.0]), c.name.clone())
    };
    let (cw, ch, dur, _) = comp_dims(e, cid)?;
    let nm = unique_layer_name(e.project.comp(cid).ok_or("no composition")?, &format!("{name} Orbit Null"));
    let mut null = build_layer(e.proj(), (cw, ch), LayerInit { kind: LayerKind::Null, name: nm, source: None, size: (100.0, 100.0), start: 0.0, duration: dur });
    null.switches.three_d = true;
    if let Some(tr) = null.props.sub_mut("transform") {
        set(tr, "position", Value::Vec3(poi), None);
        set(tr, "anchor", Value::Vec3([50.0, 50.0, 0.0]), None);
    }
    let nid = null.id;
    let at = e.project.comp(cid).and_then(|c| c.index_of(cam));
    add_layer(e, cid, null, at);
    let l = e.layer_mut(cid, cam)?;
    l.parent = Some(nid);
    if let Some(tr) = l.props.sub_mut("transform") {
        for id in ["position", "poi"] {
            if let Some(pr) = tr.get_mut(id) {
                let v = pr.value.vec3();
                pr.value = Value::Vec3([v[0] - poi[0] + 50.0, v[1] - poi[1] + 50.0, v[2] - poi[2]]);
                for k in &mut pr.keys {
                    let kv = k.value.vec3();
                    k.value = Value::Vec3([kv[0] - poi[0] + 50.0, kv[1] - poi[1] + 50.0, kv[2] - poi[2]]);
                }
            }
        }
    }
    select_only(e, &[nid]);
    e.set_label("Create Orbit Null");
    Ok(json!({"null": nid, "camera": cam}))
}

fn model_layers(e: &Engine, cid: u64, p: &Json) -> Result<Vec<u64>, String> {
    let (_, ids) = e.layers_param(p)?;
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let ids: Vec<u64> = ids.into_iter().filter(|id| comp.layer(*id).is_some_and(|l| l.is_model() && l.source.is_some())).collect();
    if ids.is_empty() {
        return Err("select 3D model layers made from a model file".into());
    }
    Ok(ids)
}

pub fn cameras_from_model(e: &mut Engine, p: &Json) -> Res {
    from_model(e, p, true)
}

pub fn lights_from_model(e: &mut Engine, p: &Json) -> Res {
    from_model(e, p, false)
}

fn from_model(e: &mut Engine, p: &Json, cameras: bool) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let ids = model_layers(e, cid, p)?;
    let (cw, ch, _, _) = comp_dims(e, cid)?;
    let mut made = vec![];
    for id in ids {
        let (world, model) = {
            let comp = e.project.comp(cid).ok_or("no composition")?;
            let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
            let l = comp.layer(id).ok_or("no layer")?;
            let f = crate::render::mesh::model_item(&ev, l).ok_or("not a model")?;
            let m = e.media.models.get(&f.path).ok_or_else(|| format!("cannot read {}", f.path))?;
            (ev.world_matrix(l) * m.to_layer(), m)
        };
        if cameras {
            for c in &model.cameras {
                let m = world * c.world;
                let eye = m.apply([0.0; 3]);
                let poi = m.apply([0.0, 0.0, -1.0]);
                let dir = crate::render::light::norm(crate::render::light::sub(poi, eye));
                let zoom = ch as f64 / 2.0 / (c.yfov / 2.0).tan().max(1e-6);
                let r = super::layer::new_camera(e, &json!({"comp": cid, "name": c.name, "zoom": zoom}))?;
                let lid = r.get("layer").and_then(Json::as_u64).ok_or("no camera")?;
                let l = e.layer_mut(cid, lid)?;
                if let Some(tr) = l.props.sub_mut("transform") {
                    set(tr, "position", Value::Vec3(eye), None);
                    set(tr, "poi", Value::Vec3(crate::render::light::add(eye, crate::render::light::scale(dir, zoom))), None);
                }
                made.push(lid);
            }
        } else {
            for li in &model.lights {
                let m = world * li.world;
                let pos = m.apply([0.0; 3]);
                let dir = crate::render::light::norm(crate::render::light::sub(m.apply([0.0, 0.0, -1.0]), pos));
                let kind = ["Parallel", "Spot", "Point"][li.kind.min(2) as usize];
                let r = super::layer::new_light(e, &json!({"comp": cid, "name": li.name, "type": kind}))?;
                let lid = r.get("layer").and_then(Json::as_u64).ok_or("no light")?;
                let l = e.layer_mut(cid, lid)?;
                apply_light(l, &json!({"color": [li.color[0], li.color[1], li.color[2]], "intensity": (li.intensity * 100.0).clamp(1.0, 400.0), "coneAngle": li.cone.clamp(1.0, 180.0), "position": pos, "poi": crate::render::light::add(pos, crate::render::light::scale(dir, cw as f64 * 0.5))}), None)?;
                made.push(lid);
            }
        }
    }
    if made.is_empty() {
        return Err(if cameras { "the model has no cameras".into() } else { "the model has no lights".into() });
    }
    select_only(e, &made);
    e.set_label(if cameras { "Create Cameras from 3D Model" } else { "Create Lights from 3D Model" });
    Ok(json!({"layers": made}))
}

pub fn control_with_camera(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let cam = camera_of(e, cid, p)?;
    let (_, ids) = e.layers_param(p)?;
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let lights: Vec<u64> = ids.into_iter().filter(|id| comp.layer(*id).is_some_and(|l| l.is_light())).collect();
    if lights.is_empty() {
        return Err("select light layers".into());
    }
    let mut out = vec![];
    for id in lights {
        let (on, world_pos) = {
            let comp = e.project.comp(cid).ok_or("no composition")?;
            let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
            let l = comp.layer(id).ok_or("no layer")?;
            (pb(p, "on").unwrap_or(l.parent != Some(cam)), ev.world_matrix(l).apply([0.0; 3]))
        };
        let l = e.layer_mut(cid, id)?;
        if on {
            l.parent = Some(cam);
            l.auto_orient = AutoOrient::Off;
            if let Some(tr) = l.props.sub_mut("transform") {
                for (k, v) in [("position", [0.0; 3]), ("orientation", [0.0; 3])] {
                    if let Some(pr) = tr.get_mut(k) {
                        pr.keys.clear();
                        pr.value = Value::Vec3(v);
                    }
                }
            }
        } else {
            l.parent = None;
            l.auto_orient = AutoOrient::TowardsPointOfInterest;
            if let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut("position")) {
                pr.keys.clear();
                pr.value = Value::Vec3(world_pos);
            }
        }
        out.push(json!({"layer": id, "controlled": on}));
    }
    e.set_label("Control Light with Camera");
    Ok(json!({"lights": out}))
}

pub fn environment_background(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let env_light = |l: &Layer| l.is_light() && l.props.sub("lightOptions").and_then(|g| g.get("lightType")).map(|p| p.value.index()) == Some(4);
    let light = match p.get("layer") {
        Some(v) => {
            let id = e.layer_in(comp, v)?;
            if !comp.layer(id).is_some_and(env_light) {
                return Err("that layer is not an Environment light".into());
            }
            id
        }
        None => e.state.selected_layers.iter().copied().find(|id| comp.layer(*id).is_some_and(env_light)).or_else(|| comp.layers.iter().find(|l| env_light(l)).map(|l| l.id)).ok_or("the composition has no Environment light (layer.newLight {kind: Environment})")?,
    };
    let lname = comp.layer(light).map(|l| l.name.clone()).unwrap_or_default();
    let (cw, ch, dur, _) = comp_dims(e, cid)?;
    let name = unique_layer_name(e.project.comp(cid).ok_or("no composition")?, &format!("{lname} Background"));
    let mut l = build_layer(e.proj(), (cw, ch), LayerInit { kind: LayerKind::Shape, name, source: None, size: (cw as f64, ch as f64), start: 0.0, duration: dur });
    l.env_background = Some(light);
    let n = e.project.comp(cid).map(|c| c.layers.len()).unwrap_or(0);
    let id = add_layer(e, cid, l, Some(n));
    select_only(e, &[id]);
    e.set_label("Create Environment Light Background Layer");
    Ok(json!({"layer": id, "light": light}))
}
