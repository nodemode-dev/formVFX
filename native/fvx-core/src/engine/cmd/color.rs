use serde_json::{Value as Json, json};

use super::super::{Engine, Res, pb, ps};
use crate::render::color::{self, Pipe};
use crate::render::ocio;

pub fn display_cm(e: &mut Engine, p: &Json) -> Res {
    let v = pb(p, "value").unwrap_or(!e.state.viewer.display_cm);
    e.state.viewer.display_cm = v;
    Ok(json!({"value": v}))
}

pub fn simulate_output(e: &mut Engine, p: &Json) -> Res {
    let prof = ps(p, "profile").unwrap_or("none");
    if !prof.eq_ignore_ascii_case("none") && color::find(prof).is_none() {
        return Err(format!("unknown output profile `{prof}` ({})", color::SPACES.iter().map(|s| s.id).collect::<Vec<_>>().join(", ")));
    }
    e.state.viewer.simulate = color::find(prof).map(|s| s.id.to_string()).unwrap_or_else(|| "none".into());
    e.state.viewer.preserve_rgb = pb(p, "preserveRgb").unwrap_or(prof == "mac18");
    Ok(json!({"profile": e.state.viewer.simulate, "preserveRgb": e.state.viewer.preserve_rgb}))
}

pub fn display_of(e: &Engine) -> color::Display {
    let profile = e.prefs.get("previews.displayProfile").and_then(Json::as_str).unwrap_or("srgb").to_string();
    color::Display { off: !e.state.viewer.display_cm, profile, simulate: e.state.viewer.simulate.clone(), preserve: e.state.viewer.preserve_rgb }
}

pub fn spaces(e: &mut Engine, p: &Json) -> Res {
    let s = &e.project.settings;
    let path = ps(p, "config").map(str::to_string).unwrap_or_else(|| s.ocio_config.clone());
    let cfg = ocio::load(&path)?;
    let pipe = Pipe::of(s);
    let displays: Vec<Json> = cfg.display_names().iter().map(|d| json!({"name": d, "views": cfg.views(d)})).collect();
    let profiles: Vec<Json> = color::SPACES.iter().map(|x| json!({"id": x.id, "name": x.name, "working": x.working, "linear": x.curve == color::Curve::Linear})).collect();
    Ok(json!({
        "engine": s.color_engine,
        "workingSpace": s.working_space,
        "managed": pipe.managed,
        "linear": pipe.linear,
        "error": pipe.error,
        "profiles": profiles,
        "ocio": {
            "config": if path.is_empty() { "built-in".to_string() } else { path },
            "name": cfg.name,
            "spaces": cfg.space_names(false),
            "displaySpaces": cfg.spaces.iter().filter(|x| x.display).map(|x| x.name.clone()).collect::<Vec<_>>(),
            "displays": displays,
            "looks": cfg.look_names(),
            "roles": cfg.roles.iter().map(|(k, v)| json!({"role": k, "space": v})).collect::<Vec<_>>(),
        },
    }))
}

pub fn convert(e: &mut Engine, p: &Json) -> Res {
    let vals: Vec<[f64; 3]> = p.get("values").and_then(Json::as_array).ok_or("pass `values`: [[r, g, b], ...]")?.iter().filter_map(|v| v.as_array()).map(|a| [0, 1, 2].map(|i| a.get(i).and_then(Json::as_f64).unwrap_or(0.0))).collect();
    let inverse = pb(p, "inverse").unwrap_or(false);
    let src = ps(p, "src").unwrap_or("");
    let dst = ps(p, "dst").unwrap_or("");
    let looks = ps(p, "looks").unwrap_or("");
    let use_ocio = p.get("config").is_some() || p.get("display").is_some() || !looks.is_empty() || (color::find(src).is_none() && !src.is_empty()) || (color::find(dst).is_none() && !dst.is_empty());
    let proc = if use_ocio {
        let path = ps(p, "config").map(str::to_string).unwrap_or_else(|| e.project.settings.ocio_config.clone());
        let cfg = ocio::load(&path)?;
        if let Some(d) = ps(p, "display") {
            cfg.display_view(src, d, ps(p, "view").unwrap_or(""), (!looks.is_empty()).then_some(looks), inverse)?
        } else if !looks.is_empty() {
            cfg.look(src, if dst.is_empty() { src } else { dst }, looks, inverse)?
        } else {
            let pr = cfg.convert(src, dst)?;
            if inverse { std::sync::Arc::new(pr.inverse()) } else { pr }
        }
    } else {
        let a = color::find(src).ok_or_else(|| format!("unknown profile `{src}`"))?;
        let b = color::find(dst).ok_or_else(|| format!("unknown profile `{dst}`"))?;
        let pr = color::convert_spaces(a, false, b, false, false);
        std::sync::Arc::new(if inverse { pr.inverse() } else { pr })
    };
    let out: Vec<[f64; 3]> = vals.iter().map(|v| proc.apply(*v)).collect();
    Ok(json!({"values": out, "ops": proc.ops.len()}))
}
