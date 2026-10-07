use std::sync::Arc;

use rayon::prelude::*;

use super::{EffectSpec, FxCtx, check, grp, no_pad, popup, slider, text};
use crate::render::image::Img;
use crate::render::ocio::{self, Interp, Op, Proc};
use crate::render::lut;

const CAT: &str = "Color Correction";
const CONFIGS: [&str; 3] = ["Project", "Built-in ACES 1.3", "Custom (.ocio file)"];

pub fn lut_file(img: &mut Img, path: &str, intensity: f32) {
    let path = path.trim();
    if path.is_empty() || intensity <= 0.0 {
        return;
    }
    let Ok(ops) = lut::load(path, "") else { return };
    blend_proc(img, &Proc { ops: ops.to_vec() }, intensity);
}

fn blend_proc(img: &mut Img, p: &Proc, k: f32) {
    if p.is_identity() {
        return;
    }
    if k >= 0.999 {
        p.apply_img(img);
        return;
    }
    img.px.par_chunks_mut(4).for_each(|px| {
        let a = px[3];
        if a <= 1e-6 {
            return;
        }
        let u = [px[0] / a, px[1] / a, px[2] / a];
        let o = p.apply32(u);
        for i in 0..3 {
            px[i] = (u[i] + (o[i] - u[i]) * k) * a;
        }
    });
}

fn config(c: &FxCtx) -> Option<Arc<ocio::Config>> {
    let project = c.settings.filter(|s| s.color_engine == "ocio").map(|s| s.ocio_config.clone()).unwrap_or_default();
    match c.e("config") {
        1 => ocio::load(""),
        2 => ocio::load(&c.s("configFile")),
        _ => ocio::load(&project),
    }
    .ok()
}

fn source(c: &FxCtx, cfg: &ocio::Config, key: &str) -> String {
    let s = c.s(key);
    if !s.trim().is_empty() {
        return s;
    }
    if let Some(st) = c.settings
        && st.color_engine == "ocio"
        && cfg.space(&st.working_space).is_some()
    {
        return st.working_space.clone();
    }
    cfg.role("color_picking").or(cfg.role("default")).unwrap_or("").to_string()
}

fn config_params() -> Vec<super::ParamSpec> {
    vec![popup("config", "Config", 0, &CONFIGS), text("configFile", "Config File", "")]
}

fn colorspace_proc(c: &FxCtx) -> Option<Proc> {
    let cfg = config(c)?;
    let src = source(c, &cfg, "source");
    let dst = c.s("destination");
    let dst = if dst.trim().is_empty() { src.clone() } else { dst };
    let p = cfg.convert(&src, &dst).ok()?;
    Some(if c.b("inverse") { p.inverse() } else { (*p).clone() })
}

fn display_proc(c: &FxCtx) -> Option<Proc> {
    let cfg = config(c)?;
    let src = source(c, &cfg, "source");
    let disp = c.s("display");
    let disp = if disp.trim().is_empty() { cfg.display_names().first().cloned().unwrap_or_default() } else { disp };
    let looks = c.s("looks");
    let looks = (!looks.trim().is_empty()).then_some(looks.as_str());
    cfg.display_view(&src, &disp, &c.s("view"), looks, c.b("inverse")).ok().map(|p| (*p).clone())
}

fn look_proc(c: &FxCtx) -> Option<Proc> {
    let cfg = config(c)?;
    let src = source(c, &cfg, "source");
    let dst = c.s("destination");
    let dst = if dst.trim().is_empty() { src.clone() } else { dst };
    let looks = c.s("looks");
    if looks.trim().is_empty() {
        return None;
    }
    cfg.look(&src, &dst, &looks, c.b("inverse")).ok().map(|p| (*p).clone())
}

fn file_proc(c: &FxCtx) -> Option<Proc> {
    let path = c.s("file");
    if path.trim().is_empty() {
        return None;
    }
    let ops = lut::load(path.trim(), c.s("cccId").trim()).ok()?;
    let interp = [Interp::Nearest, Interp::Linear, Interp::Tetrahedral, Interp::Best][c.e("interpolation").min(3) as usize];
    let ops: Vec<Op> = ops
        .iter()
        .map(|o| match o {
            Op::Lut1(l, _, i) => Op::Lut1(l.clone(), interp, *i),
            Op::Lut3(l, _, i) => Op::Lut3(l.clone(), interp, *i),
            x => x.clone(),
        })
        .collect();
    let p = Proc { ops };
    Some(if c.b("inverse") { p.inverse() } else { p })
}

fn cdl_proc(c: &FxCtx) -> Option<Proc> {
    let f = |k: &str| c.f(k);
    let file = c.s("file");
    let base = if file.trim().is_empty() { None } else { lut::load(file.trim(), c.s("cccId").trim()).ok().map(|o| o.to_vec()) };
    let ops = base.unwrap_or_else(|| {
        vec![Op::Cdl {
            s: [f("slope/red"), f("slope/green"), f("slope/blue")],
            o: [f("offset/red"), f("offset/green"), f("offset/blue")],
            p: [f("power/red"), f("power/green"), f("power/blue")],
            sat: f("saturation"),
            clamp: c.e("style") == 0,
            inv: false,
        }]
    });
    let p = Proc { ops };
    Some(if c.e("direction") == 1 { p.inverse() } else { p })
}

pub fn proc_of(id: &str, c: &FxCtx) -> Option<Proc> {
    match id {
        "fx.color.ocioColorSpace" => colorspace_proc(c),
        "fx.color.ocioDisplay" => display_proc(c),
        "fx.color.ocioLook" => look_proc(c),
        "fx.color.ocioFile" => file_proc(c),
        "fx.color.ocioCdl" => cdl_proc(c),
        _ => None,
    }
}

fn colorspace_tf(c: &FxCtx, img: &mut Img) {
    if let Some(p) = colorspace_proc(c) {
        p.apply_img(img);
    }
}

fn display_tf(c: &FxCtx, img: &mut Img) {
    if let Some(p) = display_proc(c) {
        p.apply_img(img);
    }
}

fn look_tf(c: &FxCtx, img: &mut Img) {
    if let Some(p) = look_proc(c) {
        p.apply_img(img);
    }
}

fn file_tf(c: &FxCtx, img: &mut Img) {
    if let Some(p) = file_proc(c) {
        p.apply_img(img);
    }
}

fn cdl_tf(c: &FxCtx, img: &mut Img) {
    if let Some(p) = cdl_proc(c) {
        p.apply_img(img);
    }
}

pub fn specs() -> Vec<EffectSpec> {
    let mut cs = config_params();
    cs.extend([text("source", "Source (empty: the project's working space)", ""), text("destination", "Destination", "ACEScct"), check("inverse", "Inverse", false)]);
    let mut dv = config_params();
    dv.extend([text("source", "Source (empty: the project's working space)", ""), text("display", "Display (empty: the first)", ""), text("view", "View (empty: the first)", ""), text("looks", "Looks Override", ""), check("inverse", "Inverse", false)]);
    let mut lk = config_params();
    lk.extend([text("source", "Source (empty: the project's working space)", ""), text("destination", "Destination (empty: the source)", ""), text("looks", "Looks", "ACES 1.3 Reference Gamut Compression"), check("inverse", "Inverse", false)]);
    let mut cdl = vec![grp("slope", "Slope"), slider("slope/red", "Red", 1.0, 0.0, 4.0, 3), slider("slope/green", "Green", 1.0, 0.0, 4.0, 3), slider("slope/blue", "Blue", 1.0, 0.0, 4.0, 3)];
    cdl.extend([grp("offset", "Offset"), slider("offset/red", "Red", 0.0, -1.0, 1.0, 3), slider("offset/green", "Green", 0.0, -1.0, 1.0, 3), slider("offset/blue", "Blue", 0.0, -1.0, 1.0, 3)]);
    cdl.extend([grp("power", "Power"), slider("power/red", "Red", 1.0, 0.1, 4.0, 3), slider("power/green", "Green", 1.0, 0.1, 4.0, 3), slider("power/blue", "Blue", 1.0, 0.1, 4.0, 3)]);
    cdl.extend([slider("saturation", "Saturation", 1.0, 0.0, 4.0, 3), popup("style", "Style", 0, &["ASC CDL (clamped)", "No Clamp"]), popup("direction", "Direction", 0, &["Forward", "Inverse"]), text("file", "CDL File (.cc, .ccc, .cdl; overrides the values)", ""), text("cccId", "CCC ID", "")]);
    vec![
        EffectSpec { id: "fx.color.ocioColorSpace", name: "OCIO Color Space Transform", category: CAT, params: cs, render: colorspace_tf, pad: no_pad },
        EffectSpec { id: "fx.color.ocioDisplay", name: "OCIO Display Transform", category: CAT, params: dv, render: display_tf, pad: no_pad },
        EffectSpec { id: "fx.color.ocioLook", name: "OCIO Look Transform", category: CAT, params: lk, render: look_tf, pad: no_pad },
        EffectSpec {
            id: "fx.color.ocioFile",
            name: "OCIO File Transform",
            category: CAT,
            params: vec![text("file", "File (.cube, .3dl, .spi1d, .spi3d, .clf, .ctf, .cc, .ccc, .cdl)", ""), text("cccId", "CCC ID", ""), popup("interpolation", "Interpolation", 1, &["Nearest", "Linear", "Tetrahedral", "Best"]), check("inverse", "Inverse", false)],
            render: file_tf,
            pad: no_pad,
        },
        EffectSpec { id: "fx.color.ocioCdl", name: "OCIO CDL Transform", category: CAT, params: cdl, render: cdl_tf, pad: no_pad },
    ]
}
