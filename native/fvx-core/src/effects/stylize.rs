use rayon::prelude::*;

use super::util::{gaussian, lum, mix_with_original, rand01};
use super::{EffectSpec, FxCtx, angle, check, color, no_pad, num, p, pct, popup};
use crate::model::Value;
use crate::model::prop::Ui;
use crate::render::image::Img;

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.stylize.glow",
            name: "Glow",
            category: "Stylize",
            params: vec![
                popup("basedOn", "Glow Based On", 1, &["Alpha Channel", "Color Channels"]),
                pct("threshold", "Glow Threshold", 60.0),
                num("radius", "Glow Radius", 10.0, 0.0, 1000.0, 0.0, 200.0),
                p("intensity", "Glow Intensity", Value::Num(1.0), Ui::slider(0.0, 255.0, 0.0, 4.0, 2)),
                popup("composite", "Composite Original", 1, &["On Top", "Behind", "None"]),
                popup("colors", "Glow Colors", 0, &["Original Colors", "A & B Colors"]),
                color("colorA", "Color A", [1.0, 1.0, 1.0]),
                color("colorB", "Color B", [0.0, 0.0, 0.0]),
            ],
            render: glow,
            pad: |c| c.f("radius") * 1.6,
        },
        EffectSpec {
            id: "fx.stylize.mosaic",
            name: "Mosaic",
            category: "Stylize",
            params: vec![
                p("horizontal", "Horizontal Blocks", Value::Num(10.0), Ui::slider(1.0, 4000.0, 1.0, 200.0, 0)),
                p("vertical", "Vertical Blocks", Value::Num(10.0), Ui::slider(1.0, 4000.0, 1.0, 200.0, 0)),
                check("sharp", "Sharp Colors", false),
            ],
            render: mosaic,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.findEdges",
            name: "Find Edges",
            category: "Stylize",
            params: vec![check("invert", "Invert", false), pct("blend", "Blend With Original", 0.0)],
            render: find_edges,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.emboss",
            name: "Emboss",
            category: "Stylize",
            params: vec![angle("direction", "Direction", 45.0), num("relief", "Relief", 1.5, 0.0, 10.0, 0.0, 10.0), pct("contrast", "Contrast", 100.0), pct("blend", "Blend With Original", 0.0)],
            render: emboss,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.perspective.dropShadow",
            name: "Drop Shadow",
            category: "Perspective",
            params: vec![
                color("color", "Shadow Color", [0.0, 0.0, 0.0]),
                pct("opacity", "Opacity", 50.0),
                angle("direction", "Direction", 135.0),
                num("distance", "Distance", 5.0, 0.0, 4000.0, 0.0, 120.0),
                num("softness", "Softness", 0.0, 0.0, 1000.0, 0.0, 250.0),
                check("shadowOnly", "Shadow Only", false),
            ],
            render: drop_shadow,
            pad: |c| c.f("distance") + c.f("softness") * 1.2,
        },
        EffectSpec {
            id: "fx.noise.noise",
            name: "Noise",
            category: "Noise & Grain",
            params: vec![pct("amount", "Amount of Noise", 0.0), check("color", "Use Color Noise", true), check("clip", "Clip Result Values", true)],
            render: noise,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.stripes",
            name: "Scanlines",
            category: "Stylize",
            params: vec![num("spacing", "Spacing", 4.0, 1.0, 200.0, 1.0, 40.0), pct("darkness", "Darkness", 40.0), angle("angle", "Angle", 0.0)],
            render: |c, img| {
                let sp = c.px(c.f("spacing")).max(1.0);
                let k = c.f("darkness") as f32 / 100.0;
                let a = c.f("angle").to_radians();
                let (s, co) = a.sin_cos();
                let w = img.w;
                img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
                    for x in 0..w {
                        let d = (x as f64 * -s + y as f64 * co) / sp;
                        if d.rem_euclid(1.0) < 0.5 {
                            for i in 0..3 {
                                row[x * 4 + i] *= 1.0 - k;
                            }
                        }
                    }
                });
            },
            pad: no_pad,
        },
    ]
}

fn glow(c: &FxCtx, img: &mut Img) {
    let th = c.f("threshold") as f32 / 100.0;
    let radius = c.px(c.f("radius"));
    let k = c.f("intensity") as f32;
    let alpha_based = c.e("basedOn") == 0;
    let ab = c.e("colors") == 1;
    let (ca, cb) = (c.c("colorA"), c.c("colorB"));
    let mut bright = img.clone();
    bright.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let v = if alpha_based { a } else { lum(p[0] / a, p[1] / a, p[2] / a) };
        let m = ((v - th) / (1.0 - th).max(1e-3)).clamp(0.0, 1.0);
        if ab {
            let col = [cb[0] + (ca[0] - cb[0]) * m, cb[1] + (ca[1] - cb[1]) * m, cb[2] + (ca[2] - cb[2]) * m];
            let aa = m * a;
            p[0] = col[0] * aa;
            p[1] = col[1] * aa;
            p[2] = col[2] * aa;
            p[3] = aa;
        } else {
            for v in p.iter_mut() {
                *v *= m;
            }
        }
    });
    gaussian(&mut bright, radius * 0.5, radius * 0.5, false);
    let mode = c.e("composite");
    img.px.par_chunks_mut(4).zip(bright.px.par_chunks(4)).for_each(|(p, g)| {
        let gl = [g[0] * k, g[1] * k, g[2] * k, (g[3] * k).min(1.0)];
        match mode {
            2 => p.copy_from_slice(&[gl[0].min(1.0), gl[1].min(1.0), gl[2].min(1.0), gl[3]]),
            1 => {
                let a = p[3];
                for i in 0..3 {
                    p[i] = (p[i] + gl[i] * (1.0 - a)).min(1.0);
                }
                p[3] = (a + gl[3] * (1.0 - a)).min(1.0);
            }
            _ => {
                for i in 0..3 {
                    p[i] = (p[i] + gl[i]).min(1.0);
                }
                p[3] = (p[3] + gl[3] - p[3] * gl[3]).min(1.0);
            }
        }
    });
}

fn mosaic(c: &FxCtx, img: &mut Img) {
    let r = c.layer_rect();
    let (lw, lh) = ((r[2] - r[0]).max(1.0), (r[3] - r[1]).max(1.0));
    let bw = lw / c.f("horizontal").max(1.0);
    let bh = lh / c.f("vertical").max(1.0);
    let sharp = c.b("sharp");
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let bx = ((x as f64 + 0.5 - r[0]) / bw).floor();
            let by = ((y as f64 + 0.5 - r[1]) / bh).floor();
            let c0 = [r[0] + bx * bw, r[1] + by * bh];
            let col = if sharp {
                src.sample(c0[0] + bw / 2.0, c0[1] + bh / 2.0, true)
            } else {
                let mut acc = [0.0f32; 4];
                let n = 4;
                for i in 0..n {
                    for j in 0..n {
                        let s = src.sample(c0[0] + bw * (i as f64 + 0.5) / n as f64, c0[1] + bh * (j as f64 + 0.5) / n as f64, true);
                        for k in 0..4 {
                            acc[k] += s[k];
                        }
                    }
                }
                acc.map(|v| v / (n * n) as f32)
            };
            row[x * 4..x * 4 + 4].copy_from_slice(&col);
        }
    });
}

fn find_edges(c: &FxCtx, img: &mut Img) {
    let orig = img.clone();
    let inv = c.b("invert");
    let w = img.w;
    let src = img.clone();
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let g = |dx: isize, dy: isize| {
                let p = src.get_clamped(x as isize + dx, y as isize + dy);
                let a = p[3].max(1e-6);
                [p[0] / a, p[1] / a, p[2] / a]
            };
            let a = src.get_clamped(x as isize, y as isize)[3];
            let mut out = [0.0f32; 3];
            for k in 0..3 {
                let gx = g(1, -1)[k] + 2.0 * g(1, 0)[k] + g(1, 1)[k] - g(-1, -1)[k] - 2.0 * g(-1, 0)[k] - g(-1, 1)[k];
                let gy = g(-1, 1)[k] + 2.0 * g(0, 1)[k] + g(1, 1)[k] - g(-1, -1)[k] - 2.0 * g(0, -1)[k] - g(1, -1)[k];
                let e = (gx * gx + gy * gy).sqrt().min(1.0);
                out[k] = if inv { e } else { 1.0 - e };
            }
            row[x * 4] = out[0] * a;
            row[x * 4 + 1] = out[1] * a;
            row[x * 4 + 2] = out[2] * a;
            row[x * 4 + 3] = a;
        }
    });
    mix_with_original(&orig, img, c.f("blend") as f32 / 100.0);
}

fn emboss(c: &FxCtx, img: &mut Img) {
    let orig = img.clone();
    let a = (c.f("direction") - 90.0).to_radians();
    let rel = c.px(c.f("relief")).max(0.5);
    let (dx, dy) = (a.cos() * rel, a.sin() * rel);
    let k = c.f("contrast") as f32 / 100.0;
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let p1 = src.sample(px + dx, py + dy, true);
            let p0 = src.sample(px - dx, py - dy, true);
            let l = |p: [f32; 4]| if p[3] > 1e-6 { lum(p[0] / p[3], p[1] / p[3], p[2] / p[3]) } else { 0.0 };
            let v = (0.5 + (l(p1) - l(p0)) * k * 2.0).clamp(0.0, 1.0);
            let al = src.get(x as isize, y as isize)[3];
            row[x * 4] = v * al;
            row[x * 4 + 1] = v * al;
            row[x * 4 + 2] = v * al;
            row[x * 4 + 3] = al;
        }
    });
    mix_with_original(&orig, img, c.f("blend") as f32 / 100.0);
}

fn drop_shadow(c: &FxCtx, img: &mut Img) {
    let col = c.c("color");
    let op = c.f("opacity") as f32 / 100.0;
    let a = (c.f("direction") - 90.0).to_radians();
    let d = c.px(c.f("distance"));
    let (ox, oy) = (a.cos() * d, a.sin() * d);
    let soft = c.px(c.f("softness"));
    let mut sh = Img::new(img.w, img.h);
    let src = img.clone();
    let w = img.w;
    sh.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let s = src.sample(x as f64 + 0.5 - ox, y as f64 + 0.5 - oy, false);
            let al = s[3] * op;
            row[x * 4] = col[0] * al;
            row[x * 4 + 1] = col[1] * al;
            row[x * 4 + 2] = col[2] * al;
            row[x * 4 + 3] = al;
        }
    });
    if soft > 0.3 {
        gaussian(&mut sh, soft * 0.4, soft * 0.4, false);
    }
    let only = c.b("shadowOnly");
    img.px.par_chunks_mut(4).zip(sh.px.par_chunks(4)).for_each(|(p, s)| {
        if only {
            p.copy_from_slice(s);
        } else {
            let a = p[3];
            for i in 0..4 {
                p[i] += s[i] * (1.0 - a);
            }
        }
    });
}

fn noise(c: &FxCtx, img: &mut Img) {
    let amt = c.f("amount") as f32 / 100.0;
    if amt <= 0.0 {
        return;
    }
    let colored = c.b("color");
    let clip = c.b("clip");
    let frame = (c.t * c.fps.max(1.0)).floor() as i32;
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let a = row[x * 4 + 3];
            if a <= 1e-6 {
                continue;
            }
            for k in 0..3 {
                let r = rand01(x as i32, y as i32, frame * 3 + if colored { k as i32 } else { 0 }, 77) - 0.5;
                let mut v = row[x * 4 + k] / a + r * amt;
                if clip {
                    v = v.clamp(0.0, 1.0);
                }
                row[x * 4 + k] = v * a;
            }
        }
    });
}
