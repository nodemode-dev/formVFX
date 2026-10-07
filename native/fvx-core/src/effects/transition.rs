use std::f64::consts::{PI, TAU};

use rayon::prelude::*;

use super::util::{lum, rand01, smooth};
use super::{EffectSpec, FxCtx, angle, check, color, layer_param, no_pad, num, pct, point, popup, slider};
use crate::render::image::Img;

const CAT: &str = "Transition";

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.transition.blockDissolve",
            name: "Block Dissolve",
            category: CAT,
            params: vec![pct("completion", "Transition Completion", 0.0), num("blockWidth", "Block Width", 1.0, 1.0, 1000.0, 1.0, 100.0), num("blockHeight", "Block Height", 1.0, 1.0, 1000.0, 1.0, 100.0), num("feather", "Feather", 0.0, 0.0, 100.0, 0.0, 50.0), check("softEdges", "Soft Edges (Best Quality)", true)],
            render: block_dissolve,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.cardWipe",
            name: "Card Wipe",
            category: CAT,
            params: vec![
                pct("completion", "Transition Completion", 0.0),
                num("width", "Transition Width", 50.0, 0.0, 100.0, 0.0, 100.0),
                layer_param("backLayer", "Back Layer"),
                slider("rows", "Rows", 6.0, 1.0, 1000.0, 0),
                slider("columns", "Columns", 10.0, 1.0, 1000.0, 0),
                popup("flipAxis", "Flip Axis", 0, &["X", "Y", "Random"]),
                popup("flipDirection", "Flip Direction", 0, &["Positive", "Negative", "Random"]),
                popup("flipOrder", "Flip Order", 0, &["Left to Right", "Right to Left", "Top to Bottom", "Bottom to Top", "Top Left to Bottom Right", "Top Right to Bottom Left", "Bottom Left to Top Right", "Bottom Right to Top Left", "Gradient"]),
                layer_param("gradientLayer", "Gradient Layer"),
                pct("timingRandomness", "Timing Randomness", 0.0),
                slider("seed", "Random Seed", 1.0, 0.0, 10000.0, 0),
            ],
            render: card_wipe,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.ccGlassWipe",
            name: "CC Glass Wipe",
            category: CAT,
            params: vec![pct("completion", "Completion", 0.0), layer_param("layerToReveal", "Layer to Reveal"), layer_param("gradientLayer", "Gradient Layer"), num("softness", "Softness", 10.0, 0.0, 100.0, 0.0, 100.0), num("displacement", "Displacement Amount", 10.0, 0.0, 200.0, 0.0, 100.0)],
            render: glass_wipe,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.ccGridWipe",
            name: "CC Grid Wipe",
            category: CAT,
            params: vec![pct("completion", "Completion", 0.0), point("center", "Center", 0.5, 0.5), angle("rotation", "Rotation", 0.0), num("border", "Border", 0.0, 0.0, 100.0, 0.0, 100.0), num("tiles", "Tiles", 10.0, 1.0, 100.0, 1.0, 50.0), popup("shape", "Shape", 2, &["Doors", "Radial", "Rectangular"]), check("reverse", "Reverse Transition", false)],
            render: grid_wipe,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.ccImageWipe",
            name: "CC Image Wipe",
            category: CAT,
            params: vec![pct("completion", "Completion", 0.0), num("border", "Border Softness", 10.0, 0.0, 100.0, 0.0, 100.0), check("autoSoftness", "Auto Softness", true), layer_param("gradientLayer", "Gradient Layer"), popup("property", "Property", 4, &["Red", "Green", "Blue", "Alpha", "Lightness"]), slider("blur", "Blur", 0.0, 0.0, 100.0, 1), check("invert", "Inverse Gradient", false)],
            render: image_wipe,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.ccJaws",
            name: "CC Jaws",
            category: CAT,
            params: vec![pct("completion", "Completion", 0.0), point("center", "Center", 0.5, 0.5), angle("direction", "Direction", 0.0), num("height", "Height", 50.0, 0.0, 100.0, 0.0, 100.0), num("width", "Width", 50.0, 0.0, 400.0, 0.0, 200.0), popup("shape", "Shape", 0, &["Spikes", "RoboJaw", "Block", "Waves"])],
            render: jaws,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.ccLightWipe",
            name: "CC Light Wipe",
            category: CAT,
            params: vec![pct("completion", "Completion", 0.0), point("center", "Center", 0.5, 0.5), num("intensity", "Intensity", 100.0, 0.0, 1000.0, 0.0, 500.0), popup("shape", "Shape", 0, &["Doors", "Round", "Square"]), angle("direction", "Direction", 0.0), check("colorFromSource", "Color from Source", false), color("color", "Color", [1.0, 1.0, 1.0]), check("reverse", "Reverse Transition", false)],
            render: light_wipe,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.ccLineSweep",
            name: "CC Line Sweep",
            category: CAT,
            params: vec![pct("completion", "Completion", 0.0), angle("direction", "Direction", 45.0), num("thickness", "Thickness", 20.0, 1.0, 500.0, 1.0, 100.0), num("slant", "Slant", 25.0, -100.0, 100.0, -100.0, 100.0), check("flip", "Flip Direction", false)],
            render: line_sweep,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.ccRadialScaleWipe",
            name: "CC Radial ScaleWipe",
            category: CAT,
            params: vec![pct("completion", "Completion", 0.0), point("center", "Center", 0.5, 0.5), check("reverse", "Reverse Transition", false)],
            render: radial_scale_wipe,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.ccScaleWipe",
            name: "CC Scale Wipe",
            category: CAT,
            params: vec![num("stretch", "Stretch", 10.0, 0.0, 1000.0, 0.0, 200.0), point("center", "Center", 0.5, 0.5), angle("direction", "Direction", 0.0)],
            render: scale_wipe,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.ccTwister",
            name: "CC Twister",
            category: CAT,
            params: vec![pct("completion", "Completion", 0.0), layer_param("backside", "Backside"), check("shading", "Shading", true), point("center", "Center", 0.5, 0.5), angle("axis", "Axis", 90.0)],
            render: twister,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.ccWarpoMatic",
            name: "CC WarpoMatic",
            category: CAT,
            params: vec![pct("completion", "Completion", 0.0), layer_param("layerToReveal", "Layer to Reveal"), popup("reactor", "Reactor", 0, &["Brightness", "Contrast Differences", "Brightness Differences", "Local Differences"]), num("smoothness", "Smoothness", 10.0, 0.0, 100.0, 0.0, 100.0), num("warpAmount", "Warp Amount", 50.0, -1000.0, 1000.0, -200.0, 200.0), angle("warpDirection", "Warp Direction", 0.0), num("blending", "Blend Span", 20.0, 0.0, 100.0, 0.0, 100.0)],
            render: warpomatic,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.gradientWipe",
            name: "Gradient Wipe",
            category: CAT,
            params: vec![pct("completion", "Transition Completion", 0.0), pct("softness", "Transition Softness", 0.0), layer_param("gradientLayer", "Gradient Layer"), popup("placement", "Gradient Placement", 1, &["Tile Gradient", "Center Gradient", "Stretch Gradient to Fit"]), check("invert", "Invert Gradient", false)],
            render: gradient_wipe,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.irisWipe",
            name: "Iris Wipe",
            category: CAT,
            params: vec![point("center", "Iris Center", 0.5, 0.5), slider("points", "Iris Points", 6.0, 3.0, 32.0, 0), num("outerRadius", "Outer Radius", 0.0, 0.0, 10000.0, 0.0, 1000.0), check("useInner", "Use Inner Radius", false), num("innerRadius", "Inner Radius", 0.0, 0.0, 10000.0, 0.0, 1000.0), angle("rotation", "Rotation", 0.0), num("feather", "Feather", 0.0, 0.0, 1000.0, 0.0, 100.0)],
            render: iris_wipe,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.venetianBlinds",
            name: "Venetian Blinds",
            category: CAT,
            params: vec![pct("completion", "Transition Completion", 0.0), angle("direction", "Direction", 0.0), num("width", "Width", 10.0, 1.0, 1000.0, 1.0, 200.0), num("feather", "Feather", 0.0, 0.0, 100.0, 0.0, 50.0)],
            render: venetian,
            pad: no_pad,
        },
    ]
}

fn mul_alpha(img: &mut Img, f: impl Fn(f64, f64) -> f32 + Sync) {
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let k = f(x as f64 + 0.5, y as f64 + 0.5).clamp(0.0, 1.0);
            for v in &mut row[x * 4..x * 4 + 4] {
                *v *= k;
            }
        }
    });
}

fn block_dissolve(c: &FxCtx, img: &mut Img) {
    let comp = (c.f("completion") / 100.0) as f32;
    if comp <= 0.0 {
        return;
    }
    let (bw, bh) = (c.px(c.f("blockWidth")).max(1.0), c.px(c.f("blockHeight")).max(1.0));
    let fe = c.px(c.f("feather"));
    let soft = c.b("softEdges");
    let mut m: Vec<f32> = (0..img.w * img.h)
        .map(|i| {
            let (x, y) = ((i % img.w) as f64, (i / img.w) as f64);
            let r = rand01((x / bw) as i32, (y / bh) as i32, 0, 77);
            if r < comp { 0.0 } else { 1.0 }
        })
        .collect();
    if fe > 0.3 && soft {
        super::util::blur_scalar(&mut m, img.w, img.h, fe * 0.5, fe * 0.5);
    }
    img.px.par_chunks_mut(4).zip(m.par_iter()).for_each(|(p, k)| p.iter_mut().for_each(|v| *v *= k));
}

fn card_wipe(c: &FxCtx, img: &mut Img) {
    let comp = c.f("completion") / 100.0;
    if comp <= 0.0 {
        return;
    }
    let (w, h) = (img.w, img.h);
    let back = c.other_fit("backLayer", true, w, h, 1);
    let grad = c.other_fit("gradientLayer", true, w, h, 1);
    let rect = c.layer_rect();
    let rows = c.f("rows").round().max(1.0);
    let cols = c.f("columns").round().max(1.0);
    let (cw, ch) = ((rect[2] - rect[0]) / cols, (rect[3] - rect[1]) / rows);
    let width = (c.f("width") / 100.0).max(0.01);
    let order = c.e("flipOrder");
    let axis_mode = c.e("flipAxis");
    let dir_mode = c.e("flipDirection");
    let rnd = c.f("timingRandomness") / 100.0;
    let seed = c.f("seed") as u32;
    let src = img.clone();
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let (ci, ri) = (((px - rect[0]) / cw).floor(), ((py - rect[1]) / ch).floor());
            if ci < 0.0 || ri < 0.0 || ci >= cols || ri >= rows {
                continue;
            }
            let (u, v) = ((ci + 0.5) / cols, (ri + 0.5) / rows);
            let mut start = match order {
                1 => 1.0 - u,
                2 => v,
                3 => 1.0 - v,
                4 => (u + v) * 0.5,
                5 => ((1.0 - u) + v) * 0.5,
                6 => (u + 1.0 - v) * 0.5,
                7 => (2.0 - u - v) * 0.5,
                8 => {
                    let cx = rect[0] + (ci + 0.5) * cw;
                    let cy = rect[1] + (ri + 0.5) * ch;
                    grad.as_ref().map(|g| { let s = g.sample(cx, cy, true); lum(s[0], s[1], s[2]) as f64 }).unwrap_or(u)
                }
                _ => u,
            };
            start += (rand01(ci as i32, ri as i32, 0, seed) as f64 - 0.5) * rnd;
            let t = ((comp * (1.0 + width) - start * 1.0) / width).clamp(0.0, 1.0);
            let angle = t * PI;
            let axis_y = match axis_mode {
                1 => true,
                2 => rand01(ci as i32, ri as i32, 1, seed) > 0.5,
                _ => false,
            };
            let sign: f64 = match dir_mode {
                1 => -1.0,
                2 => if rand01(ci as i32, ri as i32, 2, seed) > 0.5 { 1.0 } else { -1.0 },
                _ => 1.0,
            };
            let (ccx, ccy) = (rect[0] + (ci + 0.5) * cw, rect[1] + (ri + 0.5) * ch);
            let cosv = angle.cos();
            let scale = cosv.abs().max(1e-3);
            let (lx, ly) = (px - ccx, py - ccy);
            let (sx, sy) = if axis_y { (lx / scale * sign.signum(), ly) } else { (lx, ly / scale) };
            let inside = if axis_y { lx.abs() <= cw * 0.5 * scale } else { ly.abs() <= ch * 0.5 * scale };
            let shade = (0.6 + 0.4 * scale) as f32;
            let out = if !inside {
                [0.0f32; 4]
            } else if cosv >= 0.0 {
                let s = src.sample(ccx + sx, ccy + sy, true);
                [s[0] * shade, s[1] * shade, s[2] * shade, s[3]]
            } else {
                let (bx, by) = if axis_y { (ccx - sx, ccy + sy) } else { (ccx + sx, ccy - sy) };
                let s = back.as_ref().map(|b| b.sample(bx, by, true)).unwrap_or([0.0; 4]);
                [s[0] * shade, s[1] * shade, s[2] * shade, s[3]]
            };
            row[x * 4..x * 4 + 4].copy_from_slice(&out);
        }
    });
}

fn grad_field(c: &FxCtx, key: &str, w: usize, h: usize, prop: u32, blur: f64, invert: bool) -> Vec<f32> {
    let g = c.other_fit(key, true, w, h, 1);
    let mut f: Vec<f32> = match &g {
        Some(img) => img.px.chunks_exact(4).map(|p| match prop {
            0 => p[0],
            1 => p[1],
            2 => p[2],
            3 => p[3],
            _ => lum(p[0], p[1], p[2]),
        }).collect(),
        None => (0..w * h).map(|i| (i % w) as f32 / w.max(1) as f32).collect(),
    };
    if blur > 0.3 {
        super::util::blur_scalar(&mut f, w, h, blur, blur);
    }
    if invert {
        f.iter_mut().for_each(|v| *v = 1.0 - *v);
    }
    f
}

fn glass_wipe(c: &FxCtx, img: &mut Img) {
    let comp = (c.f("completion") / 100.0) as f32;
    if comp <= 0.0 {
        return;
    }
    let (w, h) = (img.w, img.h);
    let rev = c.other_fit("layerToReveal", true, w, h, 1);
    let g = grad_field(c, "gradientLayer", w, h, 4, c.px(c.f("softness")) * 0.3, false);
    let disp = c.px(c.f("displacement"));
    let src = img.clone();
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let i = y * w + x;
            let e = comp * 1.2 - g[i];
            let edge = (1.0 - (e.abs() / 0.15)).max(0.0);
            let gx = g[(i + 1).min(w * h - 1)] - g[i.saturating_sub(1)];
            let s = src.sample(x as f64 + 0.5 + gx as f64 * disp * edge as f64 * 10.0, y as f64 + 0.5, true);
            let k = smooth(-0.05, 0.05, e);
            let r = rev.as_ref().map(|r| r.sample(x as f64 + 0.5, y as f64 + 0.5, true)).unwrap_or([0.0; 4]);
            for ch in 0..4 {
                row[x * 4 + ch] = s[ch] * (1.0 - k) + r[ch] * k;
            }
        }
    });
}

fn grid_wipe(c: &FxCtx, img: &mut Img) {
    let mut comp = c.f("completion") / 100.0;
    if c.b("reverse") {
        comp = 1.0 - comp;
    }
    if comp <= 0.0 {
        return;
    }
    let ctr = c.pt("center");
    let rot = c.f("rotation").to_radians();
    let rect = c.layer_rect();
    let tiles = c.f("tiles").max(1.0);
    let ts = (rect[2] - rect[0]).max(rect[3] - rect[1]) / tiles;
    let border = c.f("border") / 100.0;
    let shape = c.e("shape");
    let (s, co) = rot.sin_cos();
    mul_alpha(img, |x, y| {
        let (dx, dy) = (x - ctr[0], y - ctr[1]);
        let (u, v) = (dx * co + dy * s, -dx * s + dy * co);
        let (fu, fv) = ((u / ts).rem_euclid(1.0) - 0.5, (v / ts).rem_euclid(1.0) - 0.5);
        let d = match shape {
            0 => fu.abs() * 2.0,
            1 => fu.hypot(fv) * 2.0 / 2f64.sqrt(),
            _ => fu.abs().max(fv.abs()) * 2.0,
        };
        let open = comp * (1.0 + border);
        if d < open - border { 0.0 } else if d < open { ((d - (open - border)) / border.max(1e-6)) as f32 } else { 1.0 }
    });
}

fn image_wipe(c: &FxCtx, img: &mut Img) {
    let comp = (c.f("completion") / 100.0) as f32;
    if comp <= 0.0 {
        return;
    }
    let (w, h) = (img.w, img.h);
    let g = grad_field(c, "gradientLayer", w, h, c.e("property"), c.px(c.f("blur")) * 0.3, c.b("invert"));
    let soft = if c.b("autoSoftness") { 0.1 } else { (c.f("border") / 100.0) as f32 * 0.5 };
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let t = comp * (1.0 + soft) - soft;
        let k = 1.0 - smooth(t, t + soft.max(1e-4), g[i]);
        p.iter_mut().for_each(|v| *v *= 1.0 - k);
    });
}

fn jaws(c: &FxCtx, img: &mut Img) {
    let comp = c.f("completion") / 100.0;
    if comp <= 0.0 {
        return;
    }
    let ctr = c.pt("center");
    let a = c.f("direction").to_radians();
    let (nx, ny) = (a.cos(), a.sin());
    let rect = c.layer_rect();
    let size = (rect[2] - rect[0]).hypot(rect[3] - rect[1]);
    let hgt = c.px(c.f("height")).max(1.0) * 0.5;
    let wid = c.px(c.f("width")).max(1.0) * 0.5;
    let shape = c.e("shape");
    mul_alpha(img, |x, y| {
        let (dx, dy) = (x - ctr[0], y - ctr[1]);
        let along = dx * nx + dy * ny;
        let across = -dx * ny + dy * nx;
        let ph = (along / wid).rem_euclid(1.0);
        let tooth = match shape {
            1 => if ph < 0.5 { hgt } else { -hgt },
            2 => if ph < 0.5 { hgt } else { 0.0 },
            3 => (ph * TAU).sin() * hgt,
            _ => (1.0 - (ph * 2.0 - 1.0).abs()) * hgt * 2.0 - hgt,
        };
        let gap = comp * size * 0.5;
        let up = across - tooth;
        if up.abs() < gap { 0.0 } else { 1.0 }
    });
}

fn light_wipe(c: &FxCtx, img: &mut Img) {
    let mut comp = c.f("completion") / 100.0;
    if c.b("reverse") {
        comp = 1.0 - comp;
    }
    if comp <= 0.0 {
        return;
    }
    let ctr = c.pt("center");
    let a = c.f("direction").to_radians();
    let rect = c.layer_rect();
    let size = (rect[2] - rect[0]).hypot(rect[3] - rect[1]) * 0.5;
    let shape = c.e("shape");
    let int = c.f("intensity") as f32 / 100.0;
    let from_src = c.b("colorFromSource");
    let col = c.c("color");
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (dx, dy) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let d = match shape {
                0 => (dx * a.cos() + dy * a.sin()).abs(),
                2 => dx.abs().max(dy.abs()),
                _ => dx.hypot(dy),
            } / size;
            let open = comp * 1.1;
            let p = &mut row[x * 4..x * 4 + 4];
            if d < open - 0.05 {
                p.iter_mut().for_each(|v| *v = 0.0);
                continue;
            }
            let glow = (1.0 - ((d - open) / 0.1).abs()).max(0.0) as f32 * int;
            if glow > 0.0 {
                for k in 0..3 {
                    let base = if from_src && p[3] > 1e-6 { p[k] / p[3] } else { col[k] };
                    p[k] += base * glow;
                }
                p[3] = (p[3] + glow).min(1.0);
            }
        }
    });
}

fn line_sweep(c: &FxCtx, img: &mut Img) {
    let comp = c.f("completion") / 100.0;
    if comp <= 0.0 {
        return;
    }
    let a = c.f("direction").to_radians();
    let (nx, ny) = (a.cos(), a.sin());
    let th = c.px(c.f("thickness")).max(1.0);
    let slant = c.f("slant") / 100.0;
    let flip = c.b("flip");
    let rect = c.layer_rect();
    let corners = [[rect[0], rect[1]], [rect[2], rect[1]], [rect[0], rect[3]], [rect[2], rect[3]]];
    let proj: Vec<f64> = corners.iter().map(|p| p[0] * nx + p[1] * ny).collect();
    let (lo, hi) = (proj.iter().cloned().fold(f64::MAX, f64::min), proj.iter().cloned().fold(f64::MIN, f64::max));
    mul_alpha(img, |x, y| {
        let along = x * nx + y * ny;
        let across = -x * ny + y * nx;
        let line = (across / th).floor();
        let local = (across / th).rem_euclid(1.0);
        let f = (along - lo) / (hi - lo).max(1.0);
        let thresh = comp * (1.0 + slant.abs()) - (if line as i64 % 2 == 0 { 0.0 } else { slant.abs() * 0.5 }) - local * slant.abs() * 0.5;
        let gone = if flip { 1.0 - f < thresh } else { f < thresh };
        if gone { 0.0 } else { 1.0 }
    });
}

fn radial_scale_wipe(c: &FxCtx, img: &mut Img) {
    let mut comp = c.f("completion") / 100.0;
    if c.b("reverse") {
        comp = 1.0 - comp;
    }
    if comp <= 0.0 {
        return;
    }
    let ctr = c.pt("center");
    let rect = c.layer_rect();
    let size = (rect[2] - rect[0]).hypot(rect[3] - rect[1]);
    let hole = comp * size * 0.6;
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (dx, dy) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let d = dx.hypot(dy);
            let o = if d < hole {
                [0.0; 4]
            } else {
                let k = (d - hole) / d.max(1e-6);
                src.sample(ctr[0] + dx * k, ctr[1] + dy * k, true)
            };
            row[x * 4..x * 4 + 4].copy_from_slice(&o);
        }
    });
}

fn scale_wipe(c: &FxCtx, img: &mut Img) {
    let st = c.f("stretch") / 100.0;
    if st <= 0.0 {
        return;
    }
    let ctr = c.pt("center");
    let a = c.f("direction").to_radians();
    let (nx, ny) = (a.cos(), a.sin());
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (dx, dy) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let along = dx * nx + dy * ny;
            if along <= 0.0 {
                continue;
            }
            let k = 1.0 / (1.0 + st * along / 50.0);
            let s = src.sample(x as f64 + 0.5 - nx * along * (1.0 - k), y as f64 + 0.5 - ny * along * (1.0 - k), true);
            row[x * 4..x * 4 + 4].copy_from_slice(&s);
        }
    });
}

fn twister(c: &FxCtx, img: &mut Img) {
    let comp = c.f("completion") / 100.0;
    if comp <= 0.0 {
        return;
    }
    let (w, h) = (img.w, img.h);
    let back = c.other_fit("backside", true, w, h, 1);
    let ctr = c.pt("center");
    let a = c.f("axis").to_radians();
    let (ax, ay) = (a.cos(), a.sin());
    let shading = c.b("shading");
    let rect = c.layer_rect();
    let len = (rect[2] - rect[0]).hypot(rect[3] - rect[1]);
    let src = img.clone();
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (dx, dy) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let along = dx * ax + dy * ay;
            let across = -dx * ay + dy * ax;
            let t = ((comp * 2.0 - (along / len + 0.5)) * 1.5).clamp(0.0, 1.0);
            let ang = t * PI;
            let cs = ang.cos();
            let k = cs.abs().max(1e-3);
            let sa = across / k;
            let half = len;
            let o = if sa.abs() > half {
                [0.0; 4]
            } else {
                let (sx, sy) = (ctr[0] + along * ax - sa * ay * cs.signum(), ctr[1] + along * ay + sa * ax * cs.signum());
                let s = if cs >= 0.0 { src.sample(sx, sy, false) } else { back.as_ref().map(|b| b.sample(sx, sy, false)).unwrap_or([0.0; 4]) };
                let sh = if shading { (0.5 + 0.5 * k) as f32 } else { 1.0 };
                [s[0] * sh, s[1] * sh, s[2] * sh, s[3]]
            };
            row[x * 4..x * 4 + 4].copy_from_slice(&o);
        }
    });
}

fn warpomatic(c: &FxCtx, img: &mut Img) {
    let comp = (c.f("completion") / 100.0) as f32;
    if comp <= 0.0 {
        return;
    }
    let (w, h) = (img.w, img.h);
    let rev = c.other_fit("layerToReveal", true, w, h, 1);
    let mut f: Vec<f32> = img.px.chunks_exact(4).map(|p| lum(p[0], p[1], p[2])).collect();
    let sm = c.px(c.f("smoothness")) * 0.3;
    if sm > 0.3 {
        super::util::blur_scalar(&mut f, w, h, sm, sm);
    }
    if c.e("reactor") > 0 {
        let mut b = f.clone();
        super::util::blur_scalar(&mut b, w, h, 6.0, 6.0);
        f = f.iter().zip(&b).map(|(a, bb)| ((a - bb) * 4.0 + 0.5).clamp(0.0, 1.0)).collect();
    }
    let amt = c.px(c.f("warpAmount"));
    let d = c.f("warpDirection").to_radians();
    let span = (c.f("blending") / 100.0) as f32;
    let src = img.clone();
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let i = y * w + x;
            let k = smooth(comp * (1.0 + span) - span, comp * (1.0 + span), 1.0 - f[i]);
            let off = amt * (k as f64) * (1.0 - k as f64) * 4.0;
            let s = src.sample(x as f64 + 0.5 + d.cos() * off, y as f64 + 0.5 + d.sin() * off, true);
            let r = rev.as_ref().map(|r| r.sample(x as f64 + 0.5 - d.cos() * off, y as f64 + 0.5 - d.sin() * off, true)).unwrap_or([0.0; 4]);
            for ch in 0..4 {
                row[x * 4 + ch] = s[ch] * (1.0 - k) + r[ch] * k;
            }
        }
    });
}

fn gradient_wipe(c: &FxCtx, img: &mut Img) {
    let comp = (c.f("completion") / 100.0) as f32;
    if comp <= 0.0 {
        return;
    }
    let (w, h) = (img.w, img.h);
    let mode = match c.e("placement") {
        0 => 2,
        1 => 0,
        _ => 1,
    };
    let g: Vec<f32> = match c.other_fit("gradientLayer", true, w, h, mode) {
        Some(gi) => gi.px.chunks_exact(4).map(|p| lum(p[0], p[1], p[2])).collect(),
        None => img.px.chunks_exact(4).map(|p| if p[3] > 1e-6 { lum(p[0] / p[3], p[1] / p[3], p[2] / p[3]) } else { 0.0 }).collect(),
    };
    let soft = (c.f("softness") / 100.0) as f32;
    let inv = c.b("invert");
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let v = if inv { 1.0 - g[i] } else { g[i] };
        let t = comp * (1.0 + soft) - soft;
        let k = if soft <= 1e-4 { if v >= comp { 1.0 } else { 0.0 } } else { smooth(t, t + soft, v) };
        p.iter_mut().for_each(|x| *x *= k);
    });
}

fn iris_wipe(c: &FxCtx, img: &mut Img) {
    let outer = c.px(c.f("outerRadius"));
    if outer <= 0.0 {
        return;
    }
    let ctr = c.pt("center");
    let n = c.f("points").round().clamp(3.0, 32.0);
    let inner = if c.b("useInner") { c.px(c.f("innerRadius")) } else { outer * (PI / n).cos() };
    let rot = c.f("rotation").to_radians();
    let fe = c.px(c.f("feather")).max(0.5);
    mul_alpha(img, |x, y| {
        let (dx, dy) = (x - ctr[0], y - ctr[1]);
        let d = dx.hypot(dy);
        let a = (dy.atan2(dx) - rot - PI / 2.0).rem_euclid(TAU);
        let seg = TAU / n;
        let f = (a / seg).rem_euclid(1.0);
        let tri = (f * 2.0 - 1.0).abs();
        let r = inner + (outer - inner) * tri;
        smooth((r - fe * 0.5) as f32, (r + fe * 0.5) as f32, d as f32)
    });
}

fn venetian(c: &FxCtx, img: &mut Img) {
    let comp = c.f("completion") / 100.0;
    if comp <= 0.0 {
        return;
    }
    let a = c.f("direction").to_radians();
    let (nx, ny) = (a.cos(), a.sin());
    let w = c.px(c.f("width")).max(1.0);
    let fe = c.px(c.f("feather"));
    mul_alpha(img, |x, y| {
        let along = (x * ny - y * nx).rem_euclid(w);
        let edge = (1.0 - comp) * w;
        if fe > 0.0 { 1.0 - smooth((edge - fe * 0.5) as f32, (edge + fe * 0.5) as f32, along as f32) } else if along < edge { 1.0 } else { 0.0 }
    });
}
