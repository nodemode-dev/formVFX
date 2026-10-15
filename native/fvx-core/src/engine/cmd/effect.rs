use serde_json::{Value as Json, json};

use crate::effects;
use crate::engine::{Engine, Res, ps, pu};
use crate::model::prop::{GroupKind, Node};

pub fn catalog() -> Json {
    Json::Array(
        effects::every()
            .into_iter()
            .filter(|e| effects::listed(e))
            .map(|e| {
                json!({
                    "id": e.id,
                    "name": e.name,
                    "category": e.category,
                    "gpu": crate::render::gpu::CAPABLE.contains(&e.id),
                    "float": true,
                    "params": e.params.iter().filter(|p| !matches!(p.init, effects::Init::Group)).map(|p| {
                        let d = match &p.init {
                            effects::Init::V(v) => v.to_json(),
                            effects::Init::RelPoint(x, y) => json!({"relative": [x, y]}),
                            effects::Init::Group => Json::Null,
                        };
                        json!({"id": p.id, "name": p.name, "default": d, "ui": serde_json::to_value(&p.ui).unwrap_or_default()})
                    }).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

pub fn apply(e: &mut Engine, p: &Json) -> Res {
    let key = ps(p, "effect").ok_or("pass `effect` (an id like fx.blur.gaussian or a name like Gaussian Blur)")?;
    let spec = effects::find(key).ok_or_else(|| format!("no effect `{key}` (effect.list shows them)"))?;
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select a layer to apply the effect to".into());
    }
    let mut out = vec![];
    for lid in ids {
        let (size, count) = {
            let c = e.project.comp(cid).ok_or("no comp")?;
            let l = c.layer(lid).ok_or("no layer")?;
            let n = l.effects().map(|g| g.groups().filter(|x| matches!(&x.kind, GroupKind::Effect { effect } if effect == spec.id)).count()).unwrap_or(0);
            (e.project.source_size(l, c), n)
        };
        let name = if count == 0 { spec.name.to_string() } else { format!("{} {}", spec.name, count + 1) };
        let g = {
            let pj = e.proj();
            let mut ids_ = pj.ids();
            effects::instance(spec, &mut ids_, &name, [size.0, size.1])
        };
        let uid = g.uid;
        let sound = effects::audiofx::makes_sound(spec.id);
        let levels = (sound && e.layer_ref(cid, lid)?.props.sub("audio").is_none()).then(|| {
            let pj = e.proj();
            let mut ids_ = pj.ids();
            crate::model::prop::audio_group(&mut ids_)
        });
        let l = e.layer_mut(cid, lid)?;
        if sound {
            l.switches.audio = true;
        }
        if let Some(a) = levels {
            l.props.children.push(Node::Group(a));
        }
        let fx = l.props.sub_mut("effects").ok_or("this layer cannot take effects")?;
        fx.children.push(Node::Group(g));
        let index = fx.children.len();
        out.push(json!({"layer": lid, "uid": uid, "index": index}));
    }
    e.session.last_effect = Some(spec.id.to_string());
    e.set_label(format!("Apply {}", spec.name));
    Ok(json!({"effect": spec.id, "name": spec.name, "applied": out}))
}

fn effect_uid(e: &Engine, cid: u64, lid: u64, v: Option<&Json>) -> Result<u64, String> {
    let l = e.layer_ref(cid, lid)?;
    let fx = l.effects().ok_or("no effects")?;
    match v {
        Some(Json::Number(n)) => {
            let n = n.as_u64().unwrap_or(0);
            if fx.groups().any(|g| g.uid == n) {
                Ok(n)
            } else {
                fx.groups().nth((n as usize).wrapping_sub(1)).map(|g| g.uid).ok_or_else(|| "no such effect".into())
            }
        }
        Some(Json::String(s)) => {
            if let Some(i) = s.strip_prefix('#').and_then(|x| x.parse::<usize>().ok()) {
                return fx.groups().nth(i.wrapping_sub(1)).map(|g| g.uid).ok_or_else(|| "no such effect".into());
            }
            fx.groups().find(|g| g.name == *s || matches!(&g.kind, GroupKind::Effect { effect } if effect == s)).map(|g| g.uid).ok_or_else(|| format!("no effect \"{s}\" on {}", l.name))
        }
        _ => Err("pass `effect` (uid, index or name)".into()),
    }
}

pub fn remove(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = effect_uid(e, cid, lid, p.get("effect"))?;
    let l = e.layer_mut(cid, lid)?;
    if let Some(fx) = l.props.sub_mut("effects") {
        fx.children.retain(|c| c.uid() != uid);
    }
    Ok(json!({"removed": uid}))
}

pub fn remove_all(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    for lid in &ids {
        if let Some(fx) = e.layer_mut(cid, *lid)?.props.sub_mut("effects") {
            fx.children.clear();
        }
    }
    Ok(json!({"layers": ids}))
}

pub fn reset(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = effect_uid(e, cid, lid, p.get("effect"))?;
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no effect")?;
    g.for_each_prop_mut(&mut |pr| {
        pr.keys.clear();
        pr.expr = None;
        pr.value = pr.default.clone();
    });
    Ok(json!({"reset": uid}))
}

pub fn toggle(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = effect_uid(e, cid, lid, p.get("effect"))?;
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no effect")?;
    g.enabled = crate::engine::pb(p, "enabled").unwrap_or(!g.enabled);
    Ok(json!({"enabled": g.enabled}))
}

pub fn reorder(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = effect_uid(e, cid, lid, p.get("effect"))?;
    let to = pu(p, "index").ok_or("pass `index` (1-based)")? as usize;
    let l = e.layer_mut(cid, lid)?;
    let fx = l.props.sub_mut("effects").ok_or("no effects")?;
    let i = fx.children.iter().position(|c| c.uid() == uid).ok_or("no effect")?;
    let node = fx.children.remove(i);
    let at = to.saturating_sub(1).min(fx.children.len());
    fx.children.insert(at, node);
    Ok(json!({"index": at + 1}))
}

pub fn list(_: &mut Engine, p: &Json) -> Res {
    let q = ps(p, "filter").or_else(|| ps(p, "query")).unwrap_or("").to_ascii_lowercase();
    let rows: Vec<Json> = effects::every()
        .into_iter()
        .filter(|e| q.is_empty() || e.name.to_ascii_lowercase().contains(&q) || e.id.contains(&q) || e.category.to_ascii_lowercase().contains(&q))
        .map(|e| json!({"id": e.id, "name": e.name, "category": e.category, "params": e.params.iter().map(|p| p.id).collect::<Vec<_>>()}))
        .collect();
    Ok(json!(rows))
}

const LIQUIFY_TOOLS: [&str; 10] = ["warp", "turbulence", "twirlClockwise", "twirlCounterclockwise", "pucker", "bloat", "shiftPixels", "reflection", "clone", "reconstruction"];

fn liquify_uid(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<u64, String> {
    if p.get("effect").is_some() {
        return effect_uid(e, cid, lid, p.get("effect"));
    }
    let l = e.layer_ref(cid, lid)?;
    l.effects()
        .and_then(|fx| fx.groups().find(|g| matches!(&g.kind, GroupKind::Effect { effect } if effect == "fx.distort.liquify")).map(|g| g.uid))
        .ok_or_else(|| "this layer has no Liquify effect (effect.apply Liquify first)".into())
}

pub fn liquify_stroke(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = liquify_uid(e, cid, lid, p)?;
    let pts: Vec<[f64; 2]> = p
        .get("points")
        .and_then(Json::as_array)
        .map(|a| a.iter().filter_map(|q| Some([q.get(0)?.as_f64()?, q.get(1)?.as_f64()?])).collect())
        .unwrap_or_default();
    if pts.is_empty() {
        return Err("pass `points`: [[x, y], ...] in layer pixels".into());
    }
    let (size, frame) = {
        let c = e.project.comp(cid).ok_or("no comp")?;
        let l = c.layer(lid).ok_or("no layer")?;
        let (sw, sh) = e.project.source_size(l, c);
        ([sw, sh], crate::render::comp::fx_frame(l, sw, sh))
    };
    let pts: Vec<[f64; 2]> = pts.iter().map(|q| [q[0] - frame[0], q[1] - frame[1]]).collect();
    let now = e.time_of(cid);
    let lt = e.layer_ref(cid, lid)?.layer_time(now);
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no effect")?;
    let num = |g: &crate::model::prop::Group, id: &str, d: f64| g.get(id).map(|pr| pr.value.num()).unwrap_or(d);
    let tool = match ps(p, "tool") {
        Some(t) => LIQUIFY_TOOLS.iter().find(|n| n.eq_ignore_ascii_case(t)).copied().ok_or_else(|| format!("tool is one of {}", LIQUIFY_TOOLS.join(", ")))?,
        None => LIQUIFY_TOOLS.get(g.get("tool").map(|pr| pr.value.index() as usize).unwrap_or(0)).copied().unwrap_or("warp"),
    };
    let brush = crate::engine::pf(p, "brushSize").unwrap_or_else(|| num(g, "brushSize", 64.0));
    let pressure = crate::engine::pf(p, "brushPressure").unwrap_or_else(|| num(g, "brushPressure", 50.0));
    let jitter = crate::engine::pf(p, "jitter").unwrap_or(70.0);
    let clone = p.get("cloneOffset").and_then(Json::as_array).map(|a| [a.first().and_then(Json::as_f64).unwrap_or(0.0), a.get(1).and_then(Json::as_f64).unwrap_or(0.0)]).unwrap_or([0.0; 2]);
    let mesh = g.get("mesh").map(|pr| match pr.at(lt) {
        crate::model::Value::Str(s) => s,
        _ => String::new(),
    });
    let next = effects::distort2::liquify_stroke(&mesh.unwrap_or_default(), size, tool, &pts, brush, pressure, jitter, clone);
    let pr = g.get_mut("mesh").ok_or("this Liquify has no mesh")?;
    super::prop::set_value_at(pr, crate::model::Value::Str(next), Some(lt));
    e.set_label("Liquify".to_string());
    Ok(json!({"effect": uid, "tool": tool, "points": pts.len()}))
}

pub fn liquify_clear(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = liquify_uid(e, cid, lid, p)?;
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no effect")?;
    let pr = g.get_mut("mesh").ok_or("this Liquify has no mesh")?;
    pr.keys.clear();
    pr.value = crate::model::Value::Str(String::new());
    e.set_label("Clear Liquify".to_string());
    Ok(json!({"effect": uid}))
}

pub fn plugins_list(_: &mut Engine, _: &Json) -> Res {
    let rows: Vec<Json> = effects::plugin::loaded()
        .iter()
        .map(|p| json!({"id": p.id, "name": p.name, "category": p.category, "vendor": p.vendor, "version": p.version, "path": p.path, "params": p.spec.params.iter().filter(|x| x.id != "plugin").map(|x| x.id).collect::<Vec<_>>()}))
        .collect();
    let ofx: Vec<Json> = effects::ofx::loaded()
        .iter()
        .map(|p| json!({"id": p.id, "name": p.name, "category": p.category, "identifier": p.identifier, "version": p.version, "context": p.context, "path": p.path, "params": p.spec.params.iter().filter(|x| x.id != "plugin").map(|x| x.id).collect::<Vec<_>>()}))
        .collect();
    let mut errors: Vec<Json> = effects::plugin::errors().into_iter().map(|(k, m)| json!({"source": k, "error": m})).collect();
    errors.extend(effects::ofx::errors().into_iter().map(|(k, m)| json!({"source": k, "error": m, "kind": "ofx"})));
    let folders: Vec<String> = effects::ofx::system_folders().iter().map(|d| d.to_string_lossy().to_string()).collect();
    Ok(json!({"folder": effects::plugin::folder().to_string_lossy(), "abi": effects::plugin::ABI, "plugins": rows, "ofx": ofx, "ofxFolders": folders, "ofxArch": effects::ofx::arch_dir(), "ofxMessages": effects::ofx::messages().lock().clone(), "errors": errors}))
}

pub fn ofx_overlay(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let action = ps(p, "action").unwrap_or("draw").to_string();
    if !matches!(action.as_str(), "draw" | "down" | "motion" | "up") {
        return Err("action is draw, down, motion or up".into());
    }
    let only = p.get("effect").and_then(Json::as_u64);
    let pen = match (p.get("x").and_then(Json::as_f64), p.get("y").and_then(Json::as_f64)) {
        (Some(x), Some(y)) => Some([x, y]),
        _ => None,
    };
    if action != "draw" && pen.is_none() {
        return Err("pass `x` and `y` (layer pixels) with a pen action".into());
    }
    let scale = p.get("pixelScale").and_then(Json::as_f64).filter(|s| *s > 0.0).unwrap_or(1.0);
    let t = e.time_of(cid);
    let mut out = vec![];
    let mut writes: Vec<(u64, &'static str, crate::model::Value)> = vec![];
    let lt;
    {
        let pj = &e.project;
        let comp = pj.comp(cid).ok_or("no composition")?;
        let l = comp.layer(lid).ok_or("no such layer")?;
        lt = l.layer_time(t);
        let ev = crate::eval::Eval::new(pj, cid, comp, t);
        let (sw, sh) = pj.source_size(l, comp);
        if let Some(fx) = l.effects() {
            for g in fx.groups().filter(|g| g.enabled) {
                let GroupKind::Effect { effect } = &g.kind else { continue };
                if !effect.starts_with("ofx.") || only.is_some_and(|u| u != g.uid) {
                    continue;
                }
                let Some(plug) = effects::ofx::find(effect) else { continue };
                if !plug.has_overlay() {
                    continue;
                }
                let params = effects::params_at(&ev, l, g);
                let c = effects::FxCtx { p: &params, t: lt, r: 1.0, origin: [0.0; 2], size: [sw, sh], fps: comp.frame_rate, host: None, cconv: None, settings: None };
                let act = if only.is_some() { action.as_str() } else { "draw" };
                let o = effects::ofx::interact(&plug, g.uid, &c, act, if act == "draw" { None } else { pen }, scale)?;
                for (id, v) in o.changed {
                    writes.push((g.uid, id, v));
                }
                out.push(json!({"effect": g.uid, "name": g.name, "captured": o.captured, "prims": o.prims}));
            }
        }
    }
    if !writes.is_empty() {
        let l = e.layer_mut(cid, lid)?;
        for (uid, id, v) in writes {
            if let Some(g) = l.props.find_group_mut(uid)
                && let Some(pr) = g.get_mut(id)
            {
                super::prop::set_value_at(pr, v, Some(lt));
            }
        }
        e.set_label("OpenFX Overlay".to_string());
    }
    Ok(json!({"layer": lid, "overlays": out}))
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

fn is_ofx(path: &str) -> bool {
    let l = path.trim_end_matches(['/', '\\']).to_ascii_lowercase();
    l.ends_with(".ofx.bundle") || l.ends_with(".ofx")
}

pub fn plugins_load(e: &mut Engine, p: &Json) -> Res {
    let raw = ps(p, "path").ok_or("pass `path` (a .wasm or .wat plug-in, or an OpenFX .ofx.bundle)")?.trim_end_matches(['/', '\\']).to_string();
    let path = std::path::Path::new(&raw)
        .ancestors()
        .find(|a| a.file_name().is_some_and(|n| n.to_string_lossy().to_ascii_lowercase().ends_with(".ofx.bundle")))
        .map(|a| a.to_string_lossy().to_string())
        .unwrap_or(raw);
    let used = if crate::engine::pb(p, "install").unwrap_or(false) {
        let dir = effects::plugin::folder();
        std::fs::create_dir_all(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
        let target = dir.join(std::path::Path::new(&path).file_name().ok_or("bad path")?);
        copy_tree(std::path::Path::new(&path), &target).map_err(|err| format!("{}: {err}", target.display()))?;
        target.to_string_lossy().to_string()
    } else {
        path
    };
    if is_ofx(&used) {
        effects::ofx::forgive(&used);
        let pls = effects::ofx::load(&used)?;
        e.frontend("effects.refresh", json!({}));
        return Ok(json!({"path": used, "plugins": pls.iter().map(|pl| json!({"id": pl.id, "name": pl.name, "category": pl.category, "context": pl.context})).collect::<Vec<_>>()}));
    }
    let pl = effects::plugin::load(&used)?;
    e.frontend("effects.refresh", json!({}));
    Ok(json!({"id": pl.id, "name": pl.name, "category": pl.category, "path": pl.path}))
}

pub fn plugins_reload(e: &mut Engine, _: &Json) -> Res {
    let (n, mut errors) = effects::plugin::load_folder(&effects::plugin::folder());
    for p in effects::plugin::loaded() {
        if !p.path.starts_with(&*effects::plugin::folder().to_string_lossy()) {
            let _ = effects::plugin::load(&p.path);
        }
    }
    let (mut m, errs) = effects::ofx::load_folder(&effects::plugin::folder());
    errors.extend(errs);
    for dir in effects::ofx::system_folders() {
        let (k, errs) = effects::ofx::load_folder(&dir);
        m += k;
        errors.extend(errs);
    }
    e.frontend("effects.refresh", json!({}));
    Ok(json!({"loaded": n, "ofx": m, "errors": errors}))
}
