use rayon::prelude::*;

use super::util::{box_h, box_v, gaussian, lum};
use super::{EffectSpec, FxCtx, angle, check, no_pad, num, p, point, popup};
use crate::model::Value;
use crate::model::prop::Ui;
use crate::render::image::Img;

const DIMS: [&str; 3] = ["Horizontal and Vertical", "Horizontal", "Vertical"];

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.blur.gaussian",
            name: "Gaussian Blur",
            category: "Blur & Sharpen",
            params: vec![num("blurriness", "Blurriness", 0.0, 0.0, 3000.0, 0.0, 50.0), popup("dimensions", "Blur Dimensions", 0, &DIMS), check("repeatEdge", "Repeat Edge Pixels", false)],
            render: gaussian_fx,
            pad: |c| if c.b("repeatEdge") { 0.0 } else { c.f("blurriness") * 1.6 },
        },
        EffectSpec {
            id: "fx.blur.box",
            name: "Fast Box Blur",
            category: "Blur & Sharpen",
            params: vec![
                num("radius", "Blur Radius", 0.0, 0.0, 3000.0, 0.0, 50.0),
                p("iterations", "Iterations", Value::Num(3.0), Ui::slider(1.0, 50.0, 1.0, 5.0, 0)),
                popup("dimensions", "Blur Dimensions", 0, &DIMS),
                check("repeatEdge", "Repeat Edge Pixels", false),
            ],
            render: box_fx,
            pad: |c| if c.b("repeatEdge") { 0.0 } else { c.f("radius") * c.f("iterations").max(1.0) },
        },
        EffectSpec {
            id: "fx.blur.directional",
            name: "Directional Blur",
            category: "Blur & Sharpen",
            params: vec![angle("direction", "Direction", 0.0), num("length", "Blur Length", 0.0, 0.0, 1000.0, 0.0, 100.0)],
            render: directional_fx,
            pad: |c| c.f("length"),
        },
        EffectSpec {
            id: "fx.blur.radial",
            name: "Radial Blur",
            category: "Blur & Sharpen",
            params: vec![num("amount", "Amount", 10.0, 0.0, 100.0, 0.0, 100.0), point("center", "Center", 0.5, 0.5), popup("type", "Type", 0, &["Spin", "Zoom"])],
            render: radial_fx,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.blur.sharpen",
            name: "Sharpen",
            category: "Blur & Sharpen",
            params: vec![num("amount", "Sharpen Amount", 0.0, 0.0, 500.0, 0.0, 100.0)],
            render: |c, img| unsharp(img, c.f("amount") / 100.0, c.px(1.0), 0.0),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.blur.unsharpMask",
            name: "Unsharp Mask",
            category: "Blur & Sharpen",
            params: vec![num("amount", "Amount", 50.0, 0.0, 500.0, 0.0, 500.0), num("radius", "Radius", 1.0, 0.1, 500.0, 0.1, 100.0), num("threshold", "Threshold", 0.0, 0.0, 255.0, 0.0, 255.0)],
            render: |c, img| unsharp(img, c.f("amount") / 100.0, c.px(c.f("radius")), c.f("threshold") / 255.0),
            pad: no_pad,
        },
    ]
}

fn gaussian_fx(c: &FxCtx, img: &mut Img) {
    let sigma = c.px(c.f("blurriness")) * 0.5;
    let d = c.e("dimensions");
    let sx = if d == 2 { 0.0 } else { sigma };
    let sy = if d == 1 { 0.0 } else { sigma };
    gaussian(img, sx, sy, c.b("repeatEdge"));
}

fn box_fx(c: &FxCtx, img: &mut Img) {
    let r = c.px(c.f("radius")).round() as usize;
    let n = c.f("iterations").round().clamp(1.0, 50.0) as usize;
    let d = c.e("dimensions");
    let clamp = c.b("repeatEdge");
    for _ in 0..n {
        if d != 2 {
            box_h(img, r, clamp);
        }
        if d != 1 {
            box_v(img, r, clamp);
        }
    }
}

fn directional_fx(c: &FxCtx, img: &mut Img) {
    let len = c.px(c.f("length"));
    if len < 0.5 {
        return;
    }
    let a = (c.f("direction") - 90.0).to_radians();
    let (dx, dy) = (a.cos(), a.sin());
    let n = (len.ceil() as usize * 2).clamp(4, 128);
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let mut acc = [0.0f32; 4];
            for k in 0..n {
                let t = (k as f64 / (n - 1) as f64 - 0.5) * len;
                let s = src.sample(x as f64 + 0.5 + dx * t, y as f64 + 0.5 + dy * t, false);
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

fn radial_fx(c: &FxCtx, img: &mut Img) {
    let amount = c.f("amount");
    if amount <= 0.0 {
        return;
    }
    let ctr = c.pt("center");
    let zoom = c.e("type") == 1;
    let src = img.clone();
    let w = img.w;
    let n = 32usize;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (px, py) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let mut acc = [0.0f32; 4];
            for k in 0..n {
                let t = k as f64 / (n - 1) as f64 - 0.5;
                let (sx, sy) = if zoom {
                    let s = 1.0 + t * amount / 100.0;
                    (px * s, py * s)
                } else {
                    let a = t * amount.to_radians() * 0.5;
                    let (sn, cs) = a.sin_cos();
                    (px * cs - py * sn, px * sn + py * cs)
                };
                let s = src.sample(sx + ctr[0], sy + ctr[1], false);
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

fn unsharp(img: &mut Img, amount: f64, radius: f64, threshold: f64) {
    if amount <= 0.0 {
        return;
    }
    let mut blurred = img.clone();
    gaussian(&mut blurred, radius.max(0.5), radius.max(0.5), true);
    let k = amount as f32;
    let th = threshold as f32;
    img.px.par_chunks_mut(4).zip(blurred.px.par_chunks(4)).for_each(|(p, b)| {
        let d = lum(p[0], p[1], p[2]) - lum(b[0], b[1], b[2]);
        if d.abs() < th {
            return;
        }
        for i in 0..3 {
            p[i] = (p[i] + (p[i] - b[i]) * k).clamp(0.0, p[3].max(0.0));
        }
    });
}
