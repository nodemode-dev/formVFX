use rayon::prelude::*;

use super::util::{perlin3, remap};
use super::{EffectSpec, FxCtx, angle, check, no_pad, num, p, pct, point, popup};
use crate::geom::{Mat3, homography};
use crate::model::Value;
use crate::model::prop::Ui;
use crate::render::image::Img;

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.distort.transform",
            name: "Transform",
            category: "Distort",
            params: vec![
                point("anchor", "Anchor Point", 0.5, 0.5),
                point("position", "Position", 0.5, 0.5),
                check("uniformScale", "Uniform Scale", true),
                num("scaleHeight", "Scale Height", 100.0, -10000.0, 10000.0, 0.0, 600.0),
                num("scaleWidth", "Scale Width", 100.0, -10000.0, 10000.0, 0.0, 600.0),
                num("skew", "Skew", 0.0, -85.0, 85.0, -85.0, 85.0),
                angle("skewAxis", "Skew Axis", 0.0),
                angle("rotation", "Rotation", 0.0),
                pct("opacity", "Opacity", 100.0),
            ],
            render: transform_fx,
            pad: |c| c.size[0].max(c.size[1]) * 0.5,
        },
        EffectSpec {
            id: "fx.distort.cornerPin",
            name: "Corner Pin",
            category: "Distort",
            params: vec![point("ul", "Upper Left", 0.0, 0.0), point("ur", "Upper Right", 1.0, 0.0), point("ll", "Lower Left", 0.0, 1.0), point("lr", "Lower Right", 1.0, 1.0)],
            render: corner_pin,
            pad: |c| {
                let pts = [c.v2("ul"), c.v2("ur"), c.v2("ll"), c.v2("lr")];
                let mut m: f64 = 0.0;
                for q in pts {
                    m = m.max(-q[0]).max(-q[1]).max(q[0] - c.size[0]).max(q[1] - c.size[1]);
                }
                m.max(0.0)
            },
        },
        EffectSpec {
            id: "fx.distort.waveWarp",
            name: "Wave Warp",
            category: "Distort",
            params: vec![
                popup("type", "Wave Type", 0, &["Sine", "Square", "Triangle", "Sawtooth", "Circle", "Semicircle", "Noise"]),
                num("height", "Wave Height", 10.0, -10000.0, 10000.0, -100.0, 100.0),
                num("width", "Wave Width", 40.0, 1.0, 10000.0, 1.0, 1000.0),
                angle("direction", "Direction", 90.0),
                num("speed", "Wave Speed", 1.0, -100.0, 100.0, -10.0, 10.0),
                popup("pinning", "Pinning", 0, &["None", "All Edges"]),
                angle("phase", "Phase", 0.0),
            ],
            render: wave_warp,
            pad: |c| c.f("height").abs(),
        },
        EffectSpec {
            id: "fx.distort.twirl",
            name: "Twirl",
            category: "Distort",
            params: vec![angle("angle", "Angle", 0.0), num("radius", "Twirl Radius", 30.0, 0.0, 100.0, 0.0, 100.0), point("center", "Twirl Center", 0.5, 0.5)],
            render: twirl,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.bulge",
            name: "Bulge",
            category: "Distort",
            params: vec![
                num("hRadius", "Horizontal Radius", 50.0, 0.0, 10000.0, 0.0, 1000.0),
                num("vRadius", "Vertical Radius", 50.0, 0.0, 10000.0, 0.0, 1000.0),
                point("center", "Bulge Center", 0.5, 0.5),
                p("height", "Bulge Height", Value::Num(1.0), Ui::slider(-4.0, 4.0, -4.0, 4.0, 2)),
            ],
            render: bulge,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.offset",
            name: "Offset",
            category: "Distort",
            params: vec![point("shift", "Shift Center To", 0.5, 0.5), pct("blend", "Blend With Original", 0.0)],
            render: |c, img| {
                let s = c.pt("shift");
                let ctr = c.to_buf([c.size[0] / 2.0, c.size[1] / 2.0]);
                let r = c.layer_rect();
                let (w, h) = ((r[2] - r[0]).max(1.0), (r[3] - r[1]).max(1.0));
                let (dx, dy) = (s[0] - ctr[0], s[1] - ctr[1]);
                remap(img, true, |x, y| Some([r[0] + (x - r[0] - dx).rem_euclid(w), r[1] + (y - r[1] - dy).rem_euclid(h)]));
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.mirror",
            name: "Mirror",
            category: "Distort",
            params: vec![point("center", "Reflection Center", 0.5, 0.5), angle("angle", "Reflection Angle", 0.0)],
            render: |c, img| {
                let ctr = c.pt("center");
                let a = c.f("angle").to_radians();
                let (nx, ny) = (a.cos(), a.sin());
                remap(img, false, |x, y| {
                    let d = (x - ctr[0]) * nx + (y - ctr[1]) * ny;
                    if d > 0.0 { Some([x - 2.0 * d * nx, y - 2.0 * d * ny]) } else { Some([x, y]) }
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.turbulentDisplace",
            name: "Turbulent Displace",
            category: "Distort",
            params: vec![
                num("amount", "Amount", 50.0, -10000.0, 10000.0, -200.0, 200.0),
                num("size", "Size", 100.0, 1.0, 10000.0, 2.0, 400.0),
                point("offset", "Offset (Turbulence)", 0.5, 0.5),
                p("complexity", "Complexity", Value::Num(1.0), Ui::slider(1.0, 10.0, 1.0, 10.0, 1)),
                angle("evolution", "Evolution", 0.0),
                popup("pinning", "Pinning", 0, &["None", "Pin All"]),
            ],
            render: turbulent,
            pad: |c| c.f("amount").abs() * 0.6,
        },
    ]
}

fn transform_fx(c: &FxCtx, img: &mut Img) {
    let a = c.pt("anchor");
    let p = c.pt("position");
    let sh = c.f("scaleHeight") / 100.0;
    let sw = if c.b("uniformScale") { sh } else { c.f("scaleWidth") / 100.0 };
    let m = Mat3::translate(p[0], p[1]) * Mat3::rotate_deg(c.f("rotation")) * Mat3::skew(c.f("skew"), c.f("skewAxis")) * Mat3::scale(sw, sh) * Mat3::translate(-a[0], -a[1]);
    let Some(inv) = m.inverse() else {
        img.px.iter_mut().for_each(|v| *v = 0.0);
        return;
    };
    let op = c.f("opacity") as f32 / 100.0;
    remap(img, false, |x, y| Some(inv.apply(x, y)));
    if op < 1.0 {
        img.scale_all(op);
    }
}

fn corner_pin(c: &FxCtx, img: &mut Img) {
    let r = c.layer_rect();
    let src = [[r[0], r[1]], [r[2], r[1]], [r[0], r[3]], [r[2], r[3]]];
    let dst = [c.pt("ul"), c.pt("ur"), c.pt("ll"), c.pt("lr")];
    let Some(h) = homography(dst, src) else { return };
    remap(img, false, |x, y| {
        let w = h.0[2][0] * x + h.0[2][1] * y + h.0[2][2];
        if w <= 0.0 {
            return None;
        }
        Some(h.apply(x, y))
    });
}

fn wave(kind: u32, ph: f64) -> f64 {
    let t = ph.rem_euclid(1.0);
    match kind {
        1 => {
            if t < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        2 => 4.0 * (t - 0.5).abs() - 1.0,
        3 => 2.0 * t - 1.0,
        4 => {
            let u = t * 2.0 - 1.0;
            (1.0 - u * u).max(0.0).sqrt() * if t < 0.5 { 1.0 } else { -1.0 }
        }
        5 => (1.0 - (t * 2.0 - 1.0).powi(2)).max(0.0).sqrt(),
        6 => perlin3(ph as f32 * 2.0, 0.37, 0.0, 9) as f64 * 1.6,
        _ => (ph * std::f64::consts::TAU).sin(),
    }
}

fn wave_warp(c: &FxCtx, img: &mut Img) {
    let kind = c.e("type");
    let h = c.px(c.f("height"));
    let w = c.px(c.f("width")).max(1.0);
    let dir = (c.f("direction") - 90.0).to_radians();
    let (dx, dy) = (dir.cos(), dir.sin());
    let phase = c.f("phase") / 360.0 + c.f("speed") * c.t;
    let pin = c.e("pinning") == 1;
    let r = c.layer_rect();
    remap(img, false, |x, y| {
        let along = (x * dx + y * dy) / w;
        let mut amp = h * wave(kind, along - phase);
        if pin {
            let e = (x - r[0]).min(r[2] - x).min(y - r[1]).min(r[3] - y);
            amp *= (e / (w * 0.5)).clamp(0.0, 1.0);
        }
        Some([x + -dy * amp, y + dx * amp])
    });
}

fn twirl(c: &FxCtx, img: &mut Img) {
    let a = c.f("angle").to_radians();
    if a == 0.0 {
        return;
    }
    let ctr = c.pt("center");
    let r = c.layer_rect();
    let rad = (c.f("radius") / 100.0) * (r[2] - r[0]).max(r[3] - r[1]) * 0.5;
    remap(img, false, |x, y| {
        let (px, py) = (x - ctr[0], y - ctr[1]);
        let d = px.hypot(py);
        if d >= rad || rad <= 0.0 {
            return Some([x, y]);
        }
        let t = 1.0 - d / rad;
        let ang = -a * t * t;
        let (s, co) = ang.sin_cos();
        Some([ctr[0] + px * co - py * s, ctr[1] + px * s + py * co])
    });
}

fn bulge(c: &FxCtx, img: &mut Img) {
    let ctr = c.pt("center");
    let (hr, vr) = (c.px(c.f("hRadius")).max(1e-3), c.px(c.f("vRadius")).max(1e-3));
    let height = c.f("height");
    remap(img, false, |x, y| {
        let (u, v) = ((x - ctr[0]) / hr, (y - ctr[1]) / vr);
        let d = (u * u + v * v).sqrt();
        if d >= 1.0 {
            return Some([x, y]);
        }
        let k = (1.0 - d * d).powf(1.0) * height * 0.5;
        let s = 1.0 - k;
        Some([ctr[0] + u * hr * s, ctr[1] + v * vr * s])
    });
}

fn turbulent(c: &FxCtx, img: &mut Img) {
    let amt = c.px(c.f("amount"));
    let size = c.px(c.f("size")).max(1.0);
    let off = c.pt("offset");
    let evo = (c.f("evolution") / 360.0) as f32;
    let oct = c.f("complexity").round().clamp(1.0, 10.0) as usize;
    let pin = c.e("pinning") == 1;
    let r = c.layer_rect();
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let (u, v) = (((px - off[0]) / size) as f32, ((py - off[1]) / size) as f32);
            let mut nx = 0.0f32;
            let mut ny = 0.0f32;
            let mut a = 1.0f32;
            let mut f = 1.0f32;
            for o in 0..oct {
                nx += perlin3(u * f, v * f, evo + o as f32 * 3.1, 21) * a;
                ny += perlin3(u * f + 17.3, v * f - 9.1, evo + o as f32 * 3.1, 22) * a;
                a *= 0.5;
                f *= 2.0;
            }
            let mut k = amt;
            if pin {
                let e = (px - r[0]).min(r[2] - px).min(py - r[1]).min(r[3] - py);
                k *= (e / size).clamp(0.0, 1.0);
            }
            let s = src.sample(px + nx as f64 * k, py + ny as f64 * k, false);
            row[x * 4..x * 4 + 4].copy_from_slice(&s);
        }
    });
}
