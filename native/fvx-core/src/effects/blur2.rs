use rayon::prelude::*;

use super::util::{box_h, box_v, gaussian, lum};
use super::{EffectSpec, FxCtx, angle, check, layer_param, no_pad, num, pct, point, popup, slider};
use crate::render::image::Img;

const CAT: &str = "Blur & Sharpen";

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.blur.bilateral",
            name: "Bilateral Blur",
            category: CAT,
            params: vec![num("radius", "Radius", 5.0, 0.0, 100.0, 0.0, 30.0), pct("threshold", "Threshold", 20.0), check("colorize", "Colorize", true)],
            render: bilateral,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.blur.cameraLens",
            name: "Camera Lens Blur",
            category: CAT,
            params: vec![
                num("radius", "Blur Radius", 5.0, 0.0, 500.0, 0.0, 50.0),
                popup("shape", "Iris Shape", 3, &["Triangle", "Square", "Pentagon", "Hexagon", "Heptagon", "Octagon", "Nonagon", "Decagon"]),
                angle("rotation", "Iris Rotation", 0.0),
                pct("roundness", "Iris Roundness", 0.0),
                slider("aspect", "Iris Aspect Ratio", 1.0, 0.1, 10.0, 2),
                layer_param("blurMap", "Blur Map Layer"),
                popup("blurMapChannel", "Blur Map Channel", 0, &["Luminance", "Red", "Green", "Blue", "Alpha"]),
                slider("focalDistance", "Blur Focal Distance", 0.0, 0.0, 255.0, 0),
                check("invertMap", "Invert Blur Map", false),
                pct("gain", "Highlight Gain", 0.0),
                slider("threshold", "Highlight Threshold", 255.0, 0.0, 255.0, 0),
                pct("saturation", "Highlight Saturation", 0.0),
                check("repeatEdge", "Repeat Edge Pixels", false),
            ],
            render: camera_lens,
            pad: |c| if c.b("repeatEdge") { 0.0 } else { c.f("radius") },
        },
        EffectSpec {
            id: "fx.blur.ccCrossBlur",
            name: "CC Cross Blur",
            category: CAT,
            params: vec![num("radiusX", "Radius X", 10.0, 0.0, 200.0, 0.0, 100.0), num("radiusY", "Radius Y", 10.0, 0.0, 200.0, 0.0, 100.0), popup("mode", "Transfer Mode", 0, &["Normal", "Add", "Screen", "Lighten", "Darken"]), check("repeatEdge", "Repeat Edge Pixels", false)],
            render: cross_blur,
            pad: |c| c.f("radiusX").max(c.f("radiusY")),
        },
        EffectSpec {
            id: "fx.blur.ccRadialBlur",
            name: "CC Radial Blur",
            category: CAT,
            params: vec![
                popup("type", "Type", 0, &["Straight Zoom", "Fading Zoom", "Centered", "Rotate", "Rotate Fading"]),
                num("amount", "Amount", 20.0, -360.0, 360.0, -100.0, 100.0),
                slider("quality", "Quality", 50.0, 0.0, 100.0, 0),
                point("center", "Center", 0.5, 0.5),
            ],
            render: cc_radial,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.blur.ccRadialFastBlur",
            name: "CC Radial Fast Blur",
            category: CAT,
            params: vec![point("center", "Center", 0.5, 0.5), num("amount", "Amount", 50.0, 0.0, 100.0, 0.0, 100.0), popup("zoom", "Zoom", 0, &["Standard", "Brightest", "Darkest"])],
            render: radial_fast,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.blur.ccVectorBlur",
            name: "CC Vector Blur",
            category: CAT,
            params: vec![
                popup("type", "Type", 0, &["Natural", "Constant Length", "Perpendicular", "Direction Center", "Direction Fading"]),
                num("amount", "Amount", 10.0, 0.0, 200.0, 0.0, 100.0),
                angle("angleOffset", "Angle Offset", 0.0),
                num("ridgeSmoothness", "Ridge Smoothness", 10.0, 0.0, 100.0, 0.0, 100.0),
                layer_param("vectorMap", "Vector Map"),
                popup("property", "Property", 0, &["Lightness", "Red", "Green", "Blue", "Alpha"]),
                slider("mapSoftness", "Map Softness", 10.0, 0.0, 100.0, 1),
            ],
            render: vector_blur,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.blur.channel",
            name: "Channel Blur",
            category: CAT,
            params: vec![
                num("red", "Red Blurriness", 0.0, 0.0, 2000.0, 0.0, 50.0),
                num("green", "Green Blurriness", 0.0, 0.0, 2000.0, 0.0, 50.0),
                num("blue", "Blue Blurriness", 0.0, 0.0, 2000.0, 0.0, 50.0),
                num("alpha", "Alpha Blurriness", 0.0, 0.0, 2000.0, 0.0, 50.0),
                popup("edge", "Edge Behavior", 0, &["Off", "Repeat Edge Pixels"]),
                popup("dimensions", "Blur Dimensions", 0, &["Horizontal and Vertical", "Horizontal", "Vertical"]),
            ],
            render: channel_blur,
            pad: |c| ["red", "green", "blue", "alpha"].iter().map(|k| c.f(k)).fold(0.0, f64::max) * 1.6,
        },
        EffectSpec {
            id: "fx.blur.compound",
            name: "Compound Blur",
            category: CAT,
            params: vec![layer_param("blurLayer", "Blur Layer"), num("maxBlur", "Maximum Blur", 20.0, 0.0, 400.0, 0.0, 100.0), check("stretchMap", "Stretch Map to Fit", true), check("invert", "Invert Blur", false)],
            render: compound_blur,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.blur.smart",
            name: "Smart Blur",
            category: CAT,
            params: vec![num("radius", "Radius", 3.0, 0.0, 100.0, 0.0, 20.0), num("threshold", "Threshold", 25.0, 0.0, 255.0, 0.0, 100.0), popup("mode", "Mode", 0, &["Normal", "Edge Only", "Overlay Edge"])],
            render: smart_blur,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.obsolete.gaussianLegacy",
            name: "Gaussian Blur (Legacy)",
            category: "Obsolete",
            params: vec![num("blurriness", "Blurriness", 0.0, 0.0, 1000.0, 0.0, 50.0), popup("dimensions", "Blur Dimensions", 0, &["Horizontal and Vertical", "Horizontal", "Vertical"])],
            render: |c, img| {
                let s = c.px(c.f("blurriness")) * 0.5;
                let d = c.e("dimensions");
                gaussian(img, if d == 2 { 0.0 } else { s }, if d == 1 { 0.0 } else { s }, false);
            },
            pad: |c| c.f("blurriness") * 1.6,
        },
        EffectSpec {
            id: "fx.obsolete.reduceInterlaceFlicker",
            name: "Reduce Interlace Flicker",
            category: "Obsolete",
            params: vec![slider("softness", "Softness", 1.0, 0.0, 3.0, 2)],
            render: |c, img| {
                let s = c.f("softness") * c.r;
                gaussian(img, 0.0, s * 0.5, true);
            },
            pad: no_pad,
        },
    ]
}

fn bilateral(c: &FxCtx, img: &mut Img) {
    let r = c.px(c.f("radius"));
    if r < 0.5 {
        return;
    }
    let th = (c.f("threshold") as f32 / 100.0).max(0.01);
    let colorize = c.b("colorize");
    let src = img.clone();
    let w = img.w as isize;
    let rr = r.ceil() as isize;
    let step = ((rr as f64) / 8.0).ceil().max(1.0) as isize;
    let sig2 = (r * r * 0.5) as f32;
    img.px.par_chunks_mut(w as usize * 4).enumerate().for_each(|(y, row)| {
        let y = y as isize;
        for x in 0..w {
            let c0 = src.get(x, y);
            let l0 = if c0[3] > 1e-6 { lum(c0[0] / c0[3], c0[1] / c0[3], c0[2] / c0[3]) } else { 0.0 };
            let mut acc = [0.0f32; 4];
            let mut wsum = 0.0f32;
            let mut dy = -rr;
            while dy <= rr {
                let mut dx = -rr;
                while dx <= rr {
                    let d2 = (dx * dx + dy * dy) as f32;
                    if d2 <= (rr * rr) as f32 {
                        let s = src.get_clamped(x + dx, y + dy);
                        let l = if s[3] > 1e-6 { lum(s[0] / s[3], s[1] / s[3], s[2] / s[3]) } else { 0.0 };
                        let dl = (l - l0) / th;
                        let wgt = (-d2 / (2.0 * sig2) - dl * dl * 2.0).exp();
                        for k in 0..4 {
                            acc[k] += s[k] * wgt;
                        }
                        wsum += wgt;
                    }
                    dx += step;
                }
                dy += step;
            }
            if wsum > 0.0 {
                for k in 0..4 {
                    row[x as usize * 4 + k] = acc[k] / wsum;
                }
            }
            if !colorize {
                let p = &mut row[x as usize * 4..x as usize * 4 + 4];
                if p[3] > 1e-6 {
                    let g = lum(p[0] / p[3], p[1] / p[3], p[2] / p[3]);
                    p[0] = g * p[3];
                    p[1] = g * p[3];
                    p[2] = g * p[3];
                }
            }
        }
    });
}

fn iris_kernel(radius: f64, sides: usize, rot: f64, round: f64, aspect: f64) -> Vec<(isize, isize)> {
    let r = radius.max(0.5);
    let ri = r.ceil() as isize;
    let step = ((ri as f64) / 10.0).ceil().max(1.0) as isize;
    let mut out = vec![];
    let n = sides as f64;
    let apothem = (std::f64::consts::PI / n).cos();
    let mut y = -ri;
    while y <= ri {
        let mut x = -ri;
        while x <= ri {
            let (px, py) = (x as f64 / aspect.sqrt(), y as f64 * aspect.sqrt());
            let d = px.hypot(py) / r;
            let a = py.atan2(px) - rot.to_radians();
            let seg = (a * n / std::f64::consts::TAU).floor();
            let ca = a - (seg + 0.5) * std::f64::consts::TAU / n;
            let poly = apothem / ca.cos();
            let limit = poly + (1.0 - poly) * round;
            if d <= limit {
                out.push((x, y));
            }
            x += step;
        }
        y += step;
    }
    if out.is_empty() {
        out.push((0, 0));
    }
    out
}

fn camera_lens(c: &FxCtx, img: &mut Img) {
    let rmax = c.px(c.f("radius"));
    if rmax < 0.5 {
        return;
    }
    let (w, h) = (img.w, img.h);
    let map = c.other_fit("blurMap", true, w, h, 1);
    let mch = c.e("blurMapChannel");
    let focal = c.f("focalDistance") as f32 / 255.0;
    let inv = c.b("invertMap");
    let gain = c.f("gain") as f32 / 100.0;
    let th = c.f("threshold") as f32 / 255.0;
    let hsat = c.f("saturation") as f32 / 100.0;
    let clamp = c.b("repeatEdge");
    let sides = c.e("shape") as usize + 3;
    let mut src = img.clone();
    if gain > 0.0 {
        src.px.par_chunks_mut(4).for_each(|p| {
            if p[3] <= 1e-6 {
                return;
            }
            let l = lum(p[0] / p[3], p[1] / p[3], p[2] / p[3]);
            if l > th {
                let k = 1.0 + gain * 4.0 * (l - th) / (1.0 - th).max(0.01);
                let g = l * p[3];
                for i in 0..3 {
                    let v = p[i] * k;
                    p[i] = g * k + (v - g * k) * (1.0 + hsat);
                }
            }
        });
    }
    let levels = 6usize;
    let kernels: Vec<Vec<(isize, isize)>> = (1..=levels).map(|i| iris_kernel(rmax * i as f64 / levels as f64, sides, c.f("rotation"), c.f("roundness") / 100.0, c.f("aspect"))).collect();
    let amt: Vec<f32> = match &map {
        Some(m) => m
            .px
            .chunks_exact(4)
            .map(|p| {
                let v = match mch {
                    1 => p[0],
                    2 => p[1],
                    3 => p[2],
                    4 => p[3],
                    _ => lum(p[0], p[1], p[2]),
                };
                let v = if inv { 1.0 - v } else { v };
                (v - focal).abs() / (1.0 - focal).max(focal).max(1e-3)
            })
            .collect(),
        None => vec![1.0; w * h],
    };
    let out_px: Vec<f32> = (0..h)
        .into_par_iter()
        .flat_map_iter(|y| {
            let src = &src;
            let kernels = &kernels;
            let amt = &amt;
            (0..w).flat_map(move |x| {
                let a = amt[y * w + x].clamp(0.0, 1.0);
                let li = ((a * levels as f32).round() as usize).min(levels);
                if li == 0 {
                    let i = (y * w + x) * 4;
                    return [src.px[i], src.px[i + 1], src.px[i + 2], src.px[i + 3]];
                }
                let k = &kernels[li - 1];
                let mut acc = [0.0f32; 4];
                for (dx, dy) in k {
                    let (sx, sy) = (x as isize + dx, y as isize + dy);
                    let s = if clamp { src.get_clamped(sx, sy) } else { src.get(sx, sy) };
                    for j in 0..4 {
                        acc[j] += s[j];
                    }
                }
                let n = k.len() as f32;
                [acc[0] / n, acc[1] / n, acc[2] / n, acc[3] / n]
            })
        })
        .collect();
    img.px = out_px;
}

fn cross_blur(c: &FxCtx, img: &mut Img) {
    let (rx, ry) = (c.px(c.f("radiusX")).round() as usize, c.px(c.f("radiusY")).round() as usize);
    let clamp = c.b("repeatEdge");
    let mut hz = img.clone();
    box_h(&mut hz, rx, clamp);
    let mut vt = img.clone();
    box_v(&mut vt, ry, clamp);
    let mode = c.e("mode");
    img.px.par_iter_mut().zip(hz.px.par_iter().zip(vt.px.par_iter())).for_each(|(o, (a, b))| {
        *o = match mode {
            1 => a + b,
            2 => 1.0 - (1.0 - a) * (1.0 - b),
            3 => a.max(*b),
            4 => a.min(*b),
            _ => (a + b) * 0.5,
        };
    });
}

fn cc_radial(c: &FxCtx, img: &mut Img) {
    let amt = c.f("amount");
    if amt.abs() < 0.01 {
        return;
    }
    let ty = c.e("type");
    let q = (c.f("quality") / 100.0 * 48.0 + 8.0) as usize;
    let ctr = c.pt("center");
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (px, py) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let mut acc = [0.0f32; 4];
            let mut ws = 0.0f32;
            for k in 0..q {
                let t = k as f64 / (q - 1) as f64;
                let (sx, sy, wgt) = match ty {
                    3 | 4 => {
                        let a = (if ty == 3 { t - 0.5 } else { -t }) * amt.to_radians() * 0.5;
                        let (sn, cs) = a.sin_cos();
                        (px * cs - py * sn, px * sn + py * cs, if ty == 4 { (1.0 - t) as f32 } else { 1.0 })
                    }
                    _ => {
                        let s = match ty {
                            2 => 1.0 + (t - 0.5) * amt / 100.0,
                            _ => 1.0 - t * amt / 100.0,
                        };
                        (px * s, py * s, if ty == 1 { (1.0 - t) as f32 } else { 1.0 })
                    }
                };
                let s = src.sample(sx + ctr[0], sy + ctr[1], false);
                for i in 0..4 {
                    acc[i] += s[i] * wgt;
                }
                ws += wgt;
            }
            for i in 0..4 {
                row[x * 4 + i] = acc[i] / ws.max(1e-6);
            }
        }
    });
}

fn radial_fast(c: &FxCtx, img: &mut Img) {
    let amt = c.f("amount") / 100.0;
    if amt <= 0.0 {
        return;
    }
    let ctr = c.pt("center");
    let zoom = c.e("zoom");
    let src = img.clone();
    let w = img.w;
    let n = 24usize;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (px, py) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let mut acc = [0.0f32; 4];
            let mut best = src.sample(x as f64 + 0.5, y as f64 + 0.5, false);
            for k in 0..n {
                let s = 1.0 - amt * 0.5 * k as f64 / n as f64;
                let v = src.sample(px * s + ctr[0], py * s + ctr[1], false);
                match zoom {
                    1 => {
                        if lum(v[0], v[1], v[2]) > lum(best[0], best[1], best[2]) {
                            best = v;
                        }
                    }
                    2 => {
                        if lum(v[0], v[1], v[2]) < lum(best[0], best[1], best[2]) {
                            best = v;
                        }
                    }
                    _ => {
                        for i in 0..4 {
                            acc[i] += v[i];
                        }
                    }
                }
            }
            let o = if zoom == 0 { acc.map(|v| v / n as f32) } else { best };
            row[x * 4..x * 4 + 4].copy_from_slice(&o);
        }
    });
}

fn vector_blur(c: &FxCtx, img: &mut Img) {
    let amt = c.px(c.f("amount"));
    if amt < 0.5 {
        return;
    }
    let (w, h) = (img.w, img.h);
    let prop = c.e("property");
    let mapimg = c.other_fit("vectorMap", true, w, h, 1).unwrap_or_else(|| img.clone());
    let mut field: Vec<f32> = mapimg
        .px
        .chunks_exact(4)
        .map(|p| match prop {
            1 => p[0],
            2 => p[1],
            3 => p[2],
            4 => p[3],
            _ => if p[3] > 1e-6 { lum(p[0] / p[3], p[1] / p[3], p[2] / p[3]) } else { 0.0 },
        })
        .collect();
    let soft = c.f("mapSoftness") / 100.0 * 10.0 * c.r;
    if soft > 0.3 {
        super::util::blur_scalar(&mut field, w, h, soft, soft);
    }
    let ty = c.e("type");
    let off = c.f("angleOffset").to_radians();
    let src = img.clone();
    let n = 16usize;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let g = |xx: usize, yy: usize| field[yy.min(h - 1) * w + xx.min(w - 1)];
            let gx = g(x + 1, y) - g(x.saturating_sub(1), y);
            let gy = g(x, y + 1) - g(x, y.saturating_sub(1));
            let mag = (gx * gx + gy * gy).sqrt();
            let (mut dx, mut dy) = if mag > 1e-6 { ((gx / mag) as f64, (gy / mag) as f64) } else { (0.0, 0.0) };
            if ty == 2 {
                std::mem::swap(&mut dx, &mut dy);
                dx = -dx;
            }
            let (sn, cs) = off.sin_cos();
            let (rx, ry) = (dx * cs - dy * sn, dx * sn + dy * cs);
            let len = match ty {
                1 => amt,
                4 => amt * (1.0 - field[y * w + x] as f64),
                _ => amt * (mag as f64 * 4.0).min(1.0),
            };
            if len < 0.5 {
                continue;
            }
            let mut acc = [0.0f32; 4];
            for k in 0..n {
                let t = (k as f64 / (n - 1) as f64 - 0.5) * len;
                let s = src.sample(x as f64 + 0.5 + rx * t, y as f64 + 0.5 + ry * t, true);
                for i in 0..4 {
                    acc[i] += s[i];
                }
            }
            for i in 0..4 {
                row[x * 4 + i] = acc[i] / n as f32;
            }
        }
    });
}

fn channel_blur(c: &FxCtx, img: &mut Img) {
    let d = c.e("dimensions");
    let clamp = c.e("edge") == 1;
    let (w, h) = (img.w, img.h);
    let unp: Vec<[f32; 4]> = img.px.chunks_exact(4).map(|p| if p[3] > 1e-6 { [p[0] / p[3], p[1] / p[3], p[2] / p[3], p[3]] } else { [0.0, 0.0, 0.0, 0.0] }).collect();
    let mut out = unp.clone();
    for (k, key) in ["red", "green", "blue", "alpha"].iter().enumerate() {
        let s = c.px(c.f(key)) * 0.5;
        if s < 0.3 {
            continue;
        }
        let mut plane: Vec<f32> = unp.iter().map(|p| if k < 3 { p[k] * p[3] } else { p[3] }).collect();
        let (sx, sy) = (if d == 2 { 0.0 } else { s }, if d == 1 { 0.0 } else { s });
        let mut tmp = Img { w, h, px: plane.iter().flat_map(|v| [*v, 0.0, 0.0, 0.0]).collect() };
        gaussian(&mut tmp, sx, sy, clamp);
        plane = tmp.px.chunks_exact(4).map(|p| p[0]).collect();
        for (i, v) in plane.into_iter().enumerate() {
            out[i][k] = if k < 3 { if unp[i][3] > 1e-6 { v / unp[i][3] } else { 0.0 } } else { v };
        }
    }
    img.px.par_chunks_mut(4).zip(out.par_iter()).for_each(|(p, o)| {
        for k in 0..3 {
            p[k] = o[k] * o[3];
        }
        p[3] = o[3];
    });
}

fn compound_blur(c: &FxCtx, img: &mut Img) {
    let maxb = c.px(c.f("maxBlur"));
    if maxb < 0.5 {
        return;
    }
    let (w, h) = (img.w, img.h);
    let Some(map) = c.other_fit("blurLayer", true, w, h, if c.b("stretchMap") { 1 } else { 0 }) else { return };
    let inv = c.b("invert");
    let levels = 6usize;
    let mut stack = vec![img.clone()];
    for i in 1..=levels {
        let mut b = img.clone();
        let s = maxb * i as f64 / levels as f64 * 0.5;
        gaussian(&mut b, s, s, true);
        stack.push(b);
    }
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let m = &map.px[i * 4..i * 4 + 4];
        let mut v = if m[3] > 1e-6 { lum(m[0] / m[3], m[1] / m[3], m[2] / m[3]) * m[3] } else { 0.0 };
        if inv {
            v = 1.0 - v;
        }
        let f = v.clamp(0.0, 1.0) * levels as f32;
        let a = f.floor() as usize;
        let b = (a + 1).min(levels);
        let t = f - a as f32;
        for k in 0..4 {
            p[k] = stack[a].px[i * 4 + k] + (stack[b].px[i * 4 + k] - stack[a].px[i * 4 + k]) * t;
        }
    });
}

fn smart_blur(c: &FxCtx, img: &mut Img) {
    let r = c.px(c.f("radius")).round() as isize;
    if r <= 0 && c.e("mode") == 0 {
        return;
    }
    let r = r.max(1);
    let step = ((r as f64) / 6.0).ceil().max(1.0) as usize;
    let th = c.f("threshold") as f32 / 255.0;
    let mode = c.e("mode");
    let src = img.clone();
    let w = img.w as isize;
    img.px.par_chunks_mut(w as usize * 4).enumerate().for_each(|(y, row)| {
        let y = y as isize;
        for x in 0..w {
            let c0 = src.get(x, y);
            let l0 = if c0[3] > 1e-6 { lum(c0[0] / c0[3], c0[1] / c0[3], c0[2] / c0[3]) } else { 0.0 };
            let mut acc = [0.0f32; 4];
            let mut n = 0.0f32;
            let mut edge = false;
            for dy in (-r..=r).step_by(step) {
                for dx in (-r..=r).step_by(step) {
                    let s = src.get_clamped(x + dx, y + dy);
                    let l = if s[3] > 1e-6 { lum(s[0] / s[3], s[1] / s[3], s[2] / s[3]) } else { 0.0 };
                    if (l - l0).abs() <= th {
                        for k in 0..4 {
                            acc[k] += s[k];
                        }
                        n += 1.0;
                    } else {
                        edge = true;
                    }
                }
            }
            let p = &mut row[x as usize * 4..x as usize * 4 + 4];
            match mode {
                1 => {
                    let v = if edge { 1.0 } else { 0.0 };
                    p[0] = v;
                    p[1] = v;
                    p[2] = v;
                    p[3] = 1.0;
                }
                2 => {
                    if n > 0.0 {
                        for k in 0..4 {
                            p[k] = acc[k] / n;
                        }
                    }
                    if edge {
                        p[0] = p[3];
                        p[1] = p[3];
                        p[2] = p[3];
                    }
                }
                _ => {
                    if n > 0.0 {
                        for k in 0..4 {
                            p[k] = acc[k] / n;
                        }
                    }
                }
            }
        }
    });
}
