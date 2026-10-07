use rayon::prelude::*;

use super::util::{perlin3, rand01, value3};
use super::{EffectSpec, FxCtx, angle, check, color, no_pad, num, p, pct, point, popup};
use crate::model::Value;
use crate::model::prop::Ui;
use crate::render::blend::blend_px;
use crate::render::image::Img;
use crate::model::BlendMode;

pub const BLEND: [&str; 8] = ["None", "Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light"];

pub fn blend_of(i: u32) -> Option<BlendMode> {
    Some(match i {
        1 => BlendMode::Normal,
        2 => BlendMode::Add,
        3 => BlendMode::Multiply,
        4 => BlendMode::Screen,
        5 => BlendMode::Overlay,
        6 => BlendMode::SoftLight,
        7 => BlendMode::HardLight,
        _ => return None,
    })
}

pub fn generate(img: &mut Img, opacity: f32, mode: Option<BlendMode>, keep_alpha: bool, f: impl Fn(f64, f64) -> [f32; 4] + Sync) {
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let g = f(x as f64 + 0.5, y as f64 + 0.5);
            let s = [g[0] * g[3] * opacity, g[1] * g[3] * opacity, g[2] * g[3] * opacity, g[3] * opacity];
            let d = [row[x * 4], row[x * 4 + 1], row[x * 4 + 2], row[x * 4 + 3]];
            let o = match mode {
                None => {
                    let k = opacity;
                    [d[0] + (g[0] * g[3] - d[0]) * k, d[1] + (g[1] * g[3] - d[1]) * k, d[2] + (g[2] * g[3] - d[2]) * k, d[3] + (g[3] - d[3]) * k]
                }
                Some(m) => blend_px(m, d, s, x as i64, y as i64),
            };
            let o = if keep_alpha {
                let da = d[3];
                let oa = o[3].max(1e-6);
                [o[0] * da / oa, o[1] * da / oa, o[2] * da / oa, da]
            } else {
                o
            };
            row[x * 4..x * 4 + 4].copy_from_slice(&o);
        }
    });
}

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.generate.fill",
            name: "Fill",
            category: "Generate",
            params: vec![color("color", "Color", [1.0, 0.0, 0.0]), check("invert", "Invert", false), pct("opacity", "Opacity", 100.0)],
            render: |c, img| {
                let col = c.c("color");
                let k = c.f("opacity") as f32 / 100.0;
                let inv = c.b("invert");
                img.px.par_chunks_mut(4).for_each(|p| {
                    let a = if inv { 1.0 - p[3] } else { p[3] };
                    let target = [col[0] * a, col[1] * a, col[2] * a, a];
                    for i in 0..4 {
                        p[i] += (target[i] - p[i]) * k;
                    }
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.gradientRamp",
            name: "Gradient Ramp",
            category: "Generate",
            params: vec![
                point("start", "Start of Ramp", 0.5, 0.0),
                color("startColor", "Start Color", [0.0, 0.0, 0.0]),
                point("end", "End of Ramp", 0.5, 1.0),
                color("endColor", "End Color", [1.0, 1.0, 1.0]),
                popup("shape", "Ramp Shape", 0, &["Linear Ramp", "Radial Ramp"]),
                num("scatter", "Ramp Scatter", 0.0, 0.0, 1000.0, 0.0, 100.0),
                pct("blend", "Blend With Original", 0.0),
            ],
            render: ramp,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.fourColor",
            name: "4-Color Gradient",
            category: "Generate",
            params: vec![
                point("p1", "Point 1", 0.1, 0.1),
                color("c1", "Color 1", [1.0, 1.0, 0.0]),
                point("p2", "Point 2", 0.9, 0.1),
                color("c2", "Color 2", [0.0, 1.0, 0.0]),
                point("p3", "Point 3", 0.1, 0.9),
                color("c3", "Color 3", [1.0, 0.0, 1.0]),
                point("p4", "Point 4", 0.9, 0.9),
                color("c4", "Color 4", [0.0, 0.0, 1.0]),
                num("blend", "Blend", 100.0, 1.0, 1000.0, 1.0, 1000.0),
                pct("opacity", "Opacity", 100.0),
                popup("mode", "Blending Mode", 0, &BLEND),
            ],
            render: four_color,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.checkerboard",
            name: "Checkerboard",
            category: "Generate",
            params: vec![
                point("anchor", "Anchor", 0.0, 0.0),
                p("size", "Size", Value::Vec2([64.0, 64.0]), Ui::Pixels),
                color("color", "Color", [1.0, 1.0, 1.0]),
                pct("opacity", "Opacity", 100.0),
                popup("mode", "Blending Mode", 0, &BLEND),
            ],
            render: |c, img| {
                let a = c.pt("anchor");
                let s = c.v2("size");
                let (sx, sy) = (c.px(s[0]).max(1.0), c.px(s[1]).max(1.0));
                let col = c.c("color");
                generate(img, c.f("opacity") as f32 / 100.0, blend_of(c.e("mode")), false, |x, y| {
                    let ix = ((x - a[0]) / sx).floor() as i64;
                    let iy = ((y - a[1]) / sy).floor() as i64;
                    if (ix + iy).rem_euclid(2) == 0 { [col[0], col[1], col[2], 1.0] } else { [0.0; 4] }
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.grid",
            name: "Grid",
            category: "Generate",
            params: vec![
                point("anchor", "Anchor", 0.0, 0.0),
                p("size", "Size", Value::Vec2([64.0, 64.0]), Ui::Pixels),
                num("border", "Border", 2.0, 0.0, 1000.0, 0.0, 50.0),
                check("invert", "Invert Grid", false),
                color("color", "Color", [1.0, 1.0, 1.0]),
                pct("opacity", "Opacity", 100.0),
                popup("mode", "Blending Mode", 0, &BLEND),
            ],
            render: |c, img| {
                let a = c.pt("anchor");
                let s = c.v2("size");
                let (sx, sy) = (c.px(s[0]).max(1.0), c.px(s[1]).max(1.0));
                let b = c.px(c.f("border")) / 2.0;
                let inv = c.b("invert");
                let col = c.c("color");
                generate(img, c.f("opacity") as f32 / 100.0, blend_of(c.e("mode")), false, |x, y| {
                    let fx = (x - a[0]).rem_euclid(sx);
                    let fy = (y - a[1]).rem_euclid(sy);
                    let d = fx.min(sx - fx).min(fy.min(sy - fy));
                    let on = ((b + 0.5 - d).clamp(0.0, 1.0)) as f32;
                    let on = if inv { 1.0 - on } else { on };
                    [col[0], col[1], col[2], on]
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.circle",
            name: "Circle",
            category: "Generate",
            params: vec![
                point("center", "Center", 0.5, 0.5),
                num("radius", "Radius", 75.0, 0.0, 10000.0, 0.0, 1000.0),
                popup("edge", "Edge", 0, &["None", "Thickness", "Edge Radius"]),
                num("thickness", "Thickness", 10.0, 0.0, 10000.0, 0.0, 200.0),
                num("featherOuter", "Feather Outer Edge", 0.0, 0.0, 10000.0, 0.0, 200.0),
                num("featherInner", "Feather Inner Edge", 0.0, 0.0, 10000.0, 0.0, 200.0),
                check("invert", "Invert Circle", false),
                color("color", "Color", [1.0, 1.0, 1.0]),
                pct("opacity", "Opacity", 100.0),
                popup("mode", "Blending Mode", 1, &BLEND),
            ],
            render: |c, img| {
                let ctr = c.pt("center");
                let r = c.px(c.f("radius"));
                let edge = c.e("edge");
                let th = c.px(c.f("thickness"));
                let fo = c.px(c.f("featherOuter")).max(0.5);
                let fi = c.px(c.f("featherInner")).max(0.5);
                let inv = c.b("invert");
                let col = c.c("color");
                generate(img, c.f("opacity") as f32 / 100.0, blend_of(c.e("mode")).or(Some(BlendMode::Normal)), false, |x, y| {
                    let d = (x - ctr[0]).hypot(y - ctr[1]);
                    let outer = ((r - d) / fo + 0.5).clamp(0.0, 1.0);
                    let inner = if edge == 1 { ((d - (r - th)) / fi + 0.5).clamp(0.0, 1.0) } else { 1.0 };
                    let a = (outer * inner) as f32;
                    [col[0], col[1], col[2], if inv { 1.0 - a } else { a }]
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.noise.fractalNoise",
            name: "Fractal Noise",
            category: "Noise & Grain",
            params: vec![
                popup("fractalType", "Fractal Type", 0, &["Basic", "Turbulent Smooth", "Turbulent Basic", "Turbulent Sharp", "Dynamic", "Max", "Rocky"]),
                popup("noiseType", "Noise Type", 3, &["Block", "Linear", "Soft Linear", "Spline"]),
                check("invert", "Invert", false),
                num("contrast", "Contrast", 100.0, 0.0, 10000.0, 0.0, 400.0),
                num("brightness", "Brightness", 0.0, -1000.0, 1000.0, -200.0, 200.0),
                popup("overflow", "Overflow", 0, &["Clip", "Soft Clamp", "Wrap Back"]),
                angle("rotation", "Rotation", 0.0),
                num("scale", "Scale", 100.0, 1.0, 10000.0, 20.0, 600.0),
                point("offset", "Offset Turbulence", 0.5, 0.5),
                p("complexity", "Complexity", Value::Num(6.0), Ui::slider(1.0, 20.0, 1.0, 10.0, 1)),
                pct("subInfluence", "Sub Influence (%)", 70.0),
                num("subScaling", "Sub Scaling", 56.0, 25.0, 100.0, 25.0, 100.0),
                angle("subRotation", "Sub Rotation", 0.0),
                angle("evolution", "Evolution", 0.0),
                num("randomSeed", "Random Seed", 0.0, 0.0, 10000.0, 0.0, 100.0),
                pct("opacity", "Opacity", 100.0),
                popup("mode", "Blending Mode", 1, &BLEND),
            ],
            render: fractal,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.linearWipe",
            name: "Linear Wipe",
            category: "Transition",
            params: vec![pct("completion", "Transition Completion", 0.0), angle("angle", "Wipe Angle", 90.0), num("feather", "Feather", 0.0, 0.0, 10000.0, 0.0, 300.0)],
            render: linear_wipe,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.transition.radialWipe",
            name: "Radial Wipe",
            category: "Transition",
            params: vec![pct("completion", "Transition Completion", 0.0), angle("start", "Start Angle", 0.0), point("center", "Wipe Center", 0.5, 0.5), popup("wipe", "Wipe", 0, &["Clockwise", "Counterclockwise", "Both"]), num("feather", "Feather", 0.0, 0.0, 10000.0, 0.0, 300.0)],
            render: radial_wipe,
            pad: no_pad,
        },
    ]
}

fn ramp(c: &FxCtx, img: &mut Img) {
    let s = c.pt("start");
    let e = c.pt("end");
    let (sc, ec) = (c.c("startColor"), c.c("endColor"));
    let radial = c.e("shape") == 1;
    let scatter = c.f("scatter") as f32 / 1000.0;
    let blend = c.f("blend") as f32 / 100.0;
    let (dx, dy) = (e[0] - s[0], e[1] - s[1]);
    let l2 = (dx * dx + dy * dy).max(1e-9);
    let len = l2.sqrt();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let mut t = if radial { ((px - s[0]).hypot(py - s[1]) / len) as f32 } else { (((px - s[0]) * dx + (py - s[1]) * dy) / l2) as f32 };
            if scatter > 0.0 {
                t += (rand01(x as i32, y as i32, 3, 11) - 0.5) * scatter;
            }
            let t = t.clamp(0.0, 1.0);
            let a = row[x * 4 + 3];
            for k in 0..3 {
                let g = sc[k] + (ec[k] - sc[k]) * t;
                row[x * 4 + k] = g * a + (row[x * 4 + k] - g * a) * blend;
            }
        }
    });
}

fn four_color(c: &FxCtx, img: &mut Img) {
    let pts = [c.pt("p1"), c.pt("p2"), c.pt("p3"), c.pt("p4")];
    let cols = [c.c("c1"), c.c("c2"), c.c("c3"), c.c("c4")];
    let power = (c.f("blend") / 100.0).clamp(0.01, 10.0) * 2.0;
    generate(img, c.f("opacity") as f32 / 100.0, blend_of(c.e("mode")), true, |x, y| {
        let mut acc = [0.0f64; 3];
        let mut ws = 0.0;
        for i in 0..4 {
            let d = (x - pts[i][0]).hypot(y - pts[i][1]).max(1e-3);
            let w = 1.0 / d.powf(power);
            ws += w;
            for k in 0..3 {
                acc[k] += cols[i][k] as f64 * w;
            }
        }
        [(acc[0] / ws) as f32, (acc[1] / ws) as f32, (acc[2] / ws) as f32, 1.0]
    });
}

fn fractal(c: &FxCtx, img: &mut Img) {
    let ftype = c.e("fractalType");
    let ntype = c.e("noiseType");
    let invert = c.b("invert");
    let contrast = c.f("contrast") as f32 / 100.0;
    let bright = c.f("brightness") as f32 / 100.0;
    let overflow = c.e("overflow");
    let rot = c.f("rotation").to_radians();
    let scale = c.px(c.f("scale").max(1.0)) / 100.0 * 64.0;
    let off = c.pt("offset");
    let center = c.to_buf([c.size[0] / 2.0, c.size[1] / 2.0]);
    let complexity = c.f("complexity").clamp(1.0, 20.0);
    let sub_inf = c.f("subInfluence") as f32 / 100.0;
    let sub_scale = (c.f("subScaling") / 100.0).clamp(0.25, 1.0);
    let sub_rot = c.f("subRotation").to_radians();
    let evo = c.f("evolution") / 360.0;
    let seed = c.f("randomSeed").round() as u32;
    let (rs, rc) = rot.sin_cos();
    let octaves = complexity.ceil() as usize;
    let frac = complexity - (octaves as f64 - 1.0);
    let base = move |x: f32, y: f32, z: f32, s: u32| -> f32 {
        match ntype {
            0 => value3(x.floor(), y.floor(), z, s, false),
            1 => value3(x, y, z, s, false),
            2 => value3(x, y, z, s, true),
            _ => perlin3(x, y, z, s),
        }
    };
    generate(img, c.f("opacity") as f32 / 100.0, blend_of(c.e("mode")), true, |x, y| {
        let (px, py) = (x - center[0] - (off[0] - center[0]), y - center[1] - (off[1] - center[1]));
        let (mut u, mut v) = ((px * rc + py * rs) / scale, (-px * rs + py * rc) / scale);
        let mut amp = 1.0f32;
        let mut sum = 0.0f32;
        let mut norm = 0.0f32;
        let (ss, sc) = sub_rot.sin_cos();
        for o in 0..octaves {
            let wgt = if o + 1 == octaves { frac as f32 } else { 1.0 };
            let n = base(u as f32, v as f32, (evo * (1.0 + o as f64 * 0.31)) as f32 + o as f32 * 13.7, seed.wrapping_add(o as u32 * 101));
            let n = match ftype {
                1 | 2 | 3 | 6 => n.abs() * 2.0 - 1.0,
                5 => n.abs(),
                _ => n,
            };
            let n = if ftype == 3 { 1.0 - (1.0 - n.abs()).powi(2) * 2.0 } else { n };
            sum += n * amp * wgt;
            norm += amp * wgt;
            amp *= sub_inf;
            let (nu, nv) = (u * sc - v * ss, u * ss + v * sc);
            u = nu / sub_scale + 7.31;
            v = nv / sub_scale - 3.17;
        }
        let mut val = sum / norm.max(1e-6) * 0.5 + 0.5;
        if invert {
            val = 1.0 - val;
        }
        val = (val - 0.5) * contrast + 0.5 + bright;
        val = match overflow {
            1 => 1.0 / (1.0 + (-(val - 0.5) * 6.0).exp()),
            2 => {
                let w = val.rem_euclid(2.0);
                if w > 1.0 { 2.0 - w } else { w }
            }
            _ => val.clamp(0.0, 1.0),
        };
        [val, val, val, 1.0]
    });
}

fn linear_wipe(c: &FxCtx, img: &mut Img) {
    let comp = c.f("completion") / 100.0;
    if comp <= 0.0 {
        return;
    }
    let a = (c.f("angle") - 90.0).to_radians();
    let (dx, dy) = (a.cos(), a.sin());
    let r = c.layer_rect();
    let corners = [[r[0], r[1]], [r[2], r[1]], [r[0], r[3]], [r[2], r[3]]];
    let proj: Vec<f64> = corners.iter().map(|p| p[0] * dx + p[1] * dy).collect();
    let (lo, hi) = (proj.iter().cloned().fold(f64::MAX, f64::min), proj.iter().cloned().fold(f64::MIN, f64::max));
    let feather = c.px(c.f("feather")).max(0.5);
    let edge = lo + (hi - lo + feather) * comp - feather / 2.0;
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let d = (x as f64 + 0.5) * dx + (y as f64 + 0.5) * dy;
            let k = (((d - edge) / feather) + 0.5).clamp(0.0, 1.0) as f32;
            for i in 0..4 {
                row[x * 4 + i] *= k;
            }
        }
    });
}

fn radial_wipe(c: &FxCtx, img: &mut Img) {
    let comp = c.f("completion") / 100.0;
    if comp <= 0.0 {
        return;
    }
    let ctr = c.pt("center");
    let start = c.f("start").to_radians();
    let mode = c.e("wipe");
    let feather = c.f("feather").max(0.0).to_radians() * 0.2 + 1e-3;
    let w = img.w;
    let tau = std::f64::consts::TAU;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let ang = ((x as f64 + 0.5 - ctr[0]).atan2(-(y as f64 + 0.5 - ctr[1])) - start).rem_euclid(tau) / tau;
            let a = match mode {
                1 => 1.0 - ang,
                2 => (ang.min(1.0 - ang)) * 2.0,
                _ => ang,
            };
            let k = (((a - comp) / feather) + 0.5).clamp(0.0, 1.0) as f32;
            for i in 0..4 {
                row[x * 4 + i] *= k;
            }
        }
    });
}
