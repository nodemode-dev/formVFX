use rayon::prelude::*;

use super::util::{gaussian, hsl_to_rgb, lum, perlin3, rand01, rgb_to_hsl, value3};
use super::{EffectSpec, FxCtx, angle, check, layer_param, no_pad, num, pct, popup, slider};
use crate::render::image::Img;

const CAT: &str = "Noise & Grain";

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.noise.addGrain",
            name: "Add Grain",
            category: CAT,
            params: vec![
                popup("viewMode", "Viewing Mode", 2, &["Preview", "Blending Matte", "Final Output"]),
                popup("preset", "Preset", 0, &["None", "Eastman EXR 50D", "Kodak 5248", "Kodak Vision 250D", "Fuji Eterna 250", "Kodak 5279"]),
                slider("intensity", "Intensity", 1.0, 0.0, 10.0, 2),
                slider("size", "Size", 1.0, 0.1, 5.0, 2),
                slider("softness", "Softness", 0.0, 0.0, 3.0, 2),
                slider("aspect", "Aspect Ratio", 1.0, 0.5, 2.0, 2),
                slider("redIntensity", "Red Intensity", 1.0, 0.0, 2.0, 2),
                slider("greenIntensity", "Green Intensity", 1.0, 0.0, 2.0, 2),
                slider("blueIntensity", "Blue Intensity", 1.0, 0.0, 2.0, 2),
                check("monochromatic", "Monochromatic", false),
                slider("saturation", "Saturation", 1.0, 0.0, 2.0, 2),
                pct("shadows", "Shadows", 100.0),
                pct("midtones", "Midtones", 100.0),
                pct("highlights", "Highlights", 100.0),
                slider("seed", "Random Seed", 0.0, 0.0, 10000.0, 0),
                check("animate", "Animate Noise", true),
                slider("animationSpeed", "Animation Speed", 1.0, 0.0, 10.0, 2),
            ],
            render: add_grain,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.noise.dustScratches",
            name: "Dust & Scratches",
            category: CAT,
            params: vec![num("radius", "Radius", 1.0, 0.0, 16.0, 0.0, 16.0), num("threshold", "Threshold", 0.0, 0.0, 255.0, 0.0, 255.0), check("operateAlpha", "Operate On Alpha Channel", false)],
            render: dust,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.noise.matchGrain",
            name: "Match Grain",
            category: CAT,
            params: vec![layer_param("noiseSource", "Noise Source Layer"), slider("intensity", "Intensity", 1.0, 0.0, 10.0, 2), slider("size", "Size", 1.0, 0.1, 5.0, 2), slider("seed", "Random Seed", 0.0, 0.0, 10000.0, 0)],
            render: match_grain,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.noise.median",
            name: "Median",
            category: CAT,
            params: vec![num("radius", "Radius", 0.0, 0.0, 100.0, 0.0, 20.0), check("operateAlpha", "Operate On Alpha Channel", false)],
            render: |c, img| median(img, c.px(c.f("radius")).round() as isize, c.b("operateAlpha")),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.noise.medianLegacy",
            name: "Median (Legacy)",
            category: CAT,
            params: vec![num("radius", "Radius", 0.0, 0.0, 100.0, 0.0, 20.0), check("operateAlpha", "Operate On Alpha Channel", false)],
            render: |c, img| median(img, c.px(c.f("radius")).round() as isize, c.b("operateAlpha")),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.noise.noiseAlpha",
            name: "Noise Alpha",
            category: CAT,
            params: vec![
                popup("noise", "Noise", 0, &["Uniform Random", "Squared Random", "Uniform Animation", "Squared Animation"]),
                pct("amount", "Amount", 0.0),
                popup("original", "Original Alpha", 0, &["Add", "Clamp", "Scale", "Edges"]),
                popup("overflow", "Overflow", 0, &["Clip", "Wrap Back", "Wrap"]),
                slider("seed", "Random Seed", 0.0, 0.0, 10000.0, 0),
                angle("phase", "Noise Phase", 0.0),
            ],
            render: noise_alpha,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.noise.noiseHls",
            name: "Noise HLS",
            category: CAT,
            params: vec![popup("noise", "Noise", 0, &["Uniform", "Squared", "Grain"]), pct("hue", "Hue", 0.0), pct("lightness", "Lightness", 0.0), pct("saturation", "Saturation", 0.0), slider("grainSize", "Grain Size", 1.0, 1.0, 100.0, 1), angle("phase", "Noise Phase", 0.0)],
            render: |c, img| noise_hls(c, img, false),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.noise.noiseHlsAuto",
            name: "Noise HLS Auto",
            category: CAT,
            params: vec![popup("noise", "Noise", 0, &["Uniform", "Squared", "Grain"]), pct("hue", "Hue", 0.0), pct("lightness", "Lightness", 0.0), pct("saturation", "Saturation", 0.0), slider("grainSize", "Grain Size", 1.0, 1.0, 100.0, 1), slider("animationSpeed", "Noise Animation Speed", 10.0, 0.0, 100.0, 1)],
            render: |c, img| noise_hls(c, img, true),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.noise.removeGrain",
            name: "Remove Grain",
            category: CAT,
            params: vec![popup("viewMode", "Viewing Mode", 1, &["Preview", "Final Output"]), slider("passes", "Noise Reduction", 1.0, 0.0, 5.0, 2), slider("passCount", "Passes", 1.0, 1.0, 5.0, 0), slider("detail", "Fine Tuning: Detail Preservation", 0.5, 0.0, 1.0, 2), slider("temporal", "Temporal Filtering Amount", 0.0, 0.0, 1.0, 2)],
            render: remove_grain,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.noise.turbulentNoise",
            name: "Turbulent Noise",
            category: CAT,
            params: vec![
                popup("fractalType", "Fractal Type", 0, &["Basic", "Turbulent Smooth", "Turbulent Basic", "Dynamic", "Max", "Rocky"]),
                popup("noiseType", "Noise Type", 1, &["Block", "Linear", "Soft Linear", "Spline"]),
                check("invert", "Invert", false),
                slider("contrast", "Contrast", 100.0, 0.0, 10000.0, 1),
                slider("brightness", "Brightness", 0.0, -200.0, 200.0, 1),
                num("scale", "Scale", 100.0, 10.0, 10000.0, 20.0, 600.0),
                angle("rotation", "Rotation", 0.0),
                slider("complexity", "Complexity", 6.0, 1.0, 20.0, 1),
                angle("evolution", "Evolution", 0.0),
                pct("opacity", "Opacity", 100.0),
            ],
            render: turbulent_noise,
            pad: no_pad,
        },
    ]
}

fn grain_field(x: f64, y: f64, t: f64, size: f64, seed: u32, ch: u32) -> f32 {
    let s = size.max(0.1);
    value3((x / s) as f32, (y / s) as f32, t as f32, seed.wrapping_add(ch * 101), true) * 0.7 + (rand01((x / s) as i32, (y / s) as i32, (t * 10.0) as i32, seed ^ ch) - 0.5) * 0.6
}

fn add_grain(c: &FxCtx, img: &mut Img) {
    let mut int = c.f("intensity") as f32 * 0.08;
    let mut size = c.px(c.f("size"));
    match c.e("preset") {
        1 => {
            int *= 0.6;
            size *= 0.8;
        }
        2 => int *= 1.2,
        3 => size *= 1.2,
        4 => int *= 0.9,
        5 => {
            int *= 1.4;
            size *= 1.3;
        }
        _ => {}
    }
    let asp = c.f("aspect").max(0.1);
    let chans = [c.f("redIntensity") as f32, c.f("greenIntensity") as f32, c.f("blueIntensity") as f32];
    let mono = c.b("monochromatic");
    let sat = c.f("saturation") as f32;
    let tone = [c.f("shadows") as f32 / 100.0, c.f("midtones") as f32 / 100.0, c.f("highlights") as f32 / 100.0];
    let seed = c.f("seed") as u32;
    let t = if c.b("animate") { c.t * c.fps * c.f("animationSpeed") } else { 0.0 };
    let view = c.e("viewMode");
    let soft = c.f("softness");
    let w = img.w;
    let mut grain = vec![[0.0f32; 3]; img.w * img.h];
    grain.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, g) in row.iter_mut().enumerate() {
            let (fx, fy) = (x as f64 * asp.sqrt(), y as f64 / asp.sqrt());
            let base = grain_field(fx, fy, t, size, seed, 0);
            for k in 0..3 {
                let v = if mono { base } else { base * (1.0 - sat.min(1.0)) + grain_field(fx, fy, t, size, seed, k as u32 + 1) * sat.min(1.0) };
                g[k] = v * chans[k];
            }
        }
    });
    if soft > 0.0 {
        let (h, ww) = (img.h, img.w);
        for k in 0..3 {
            let mut plane: Vec<f32> = grain.iter().map(|g| g[k]).collect();
            super::util::blur_scalar(&mut plane, ww, h, soft * c.r, soft * c.r);
            for (g, v) in grain.iter_mut().zip(plane) {
                g[k] = v;
            }
        }
    }
    img.px.par_chunks_mut(4).zip(grain.par_iter()).for_each(|(p, g)| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let u = [p[0] / a, p[1] / a, p[2] / a];
        let l = lum(u[0], u[1], u[2]).clamp(0.0, 1.0);
        let wt = tone[0] * (1.0 - l).powi(2) + tone[1] * 4.0 * l * (1.0 - l) * 0.5 + tone[2] * l * l;
        if view == 1 {
            p[0] = wt * a;
            p[1] = wt * a;
            p[2] = wt * a;
            return;
        }
        for k in 0..3 {
            let v = u[k] + g[k] * int * wt;
            p[k] = if view == 0 { (0.5 + g[k] * int) * a } else { v.max(0.0) * a };
        }
    });
}

fn median(img: &mut Img, r: isize, alpha: bool) {
    if r <= 0 {
        return;
    }
    let src = img.clone();
    let w = img.w as isize;
    let step = ((r as f64) / 4.0).ceil().max(1.0) as usize;
    img.px.par_chunks_mut(w as usize * 4).enumerate().for_each(|(y, row)| {
        let y = y as isize;
        let mut buf: Vec<[f32; 4]> = Vec::with_capacity(((2 * r + 1) * (2 * r + 1)) as usize);
        for x in 0..w {
            buf.clear();
            for dy in (-r..=r).step_by(step) {
                for dx in (-r..=r).step_by(step) {
                    if dx * dx + dy * dy <= r * r {
                        buf.push(src.get_clamped(x + dx, y + dy));
                    }
                }
            }
            let n = buf.len();
            let mut o = [0.0f32; 4];
            for k in 0..4 {
                if k == 3 && !alpha {
                    o[3] = row[x as usize * 4 + 3];
                    continue;
                }
                let mut v: Vec<f32> = buf.iter().map(|p| p[k]).collect();
                v.select_nth_unstable_by(n / 2, |a, b| a.total_cmp(b));
                o[k] = v[n / 2];
            }
            if !alpha {
                for k in 0..3 {
                    o[k] = o[k].min(o[3]);
                }
            }
            row[x as usize * 4..x as usize * 4 + 4].copy_from_slice(&o);
        }
    });
}

fn dust(c: &FxCtx, img: &mut Img) {
    let r = c.px(c.f("radius")).round() as isize;
    if r <= 0 {
        return;
    }
    let th = c.f("threshold") as f32 / 255.0;
    let orig = img.clone();
    median(img, r, c.b("operateAlpha"));
    img.px.par_chunks_mut(4).zip(orig.px.par_chunks(4)).for_each(|(m, o)| {
        let d = (lum(m[0], m[1], m[2]) - lum(o[0], o[1], o[2])).abs();
        if d <= th {
            m.copy_from_slice(o);
        }
    });
}

fn match_grain(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let mut amp = 0.04f32;
    if let Some(src) = c.other_fit("noiseSource", true, w, h, 1) {
        let mut blur = src.clone();
        gaussian(&mut blur, 1.5, 1.5, true);
        let mut acc = 0.0f64;
        let mut n = 0.0;
        for (a, b) in src.px.chunks_exact(4).zip(blur.px.chunks_exact(4)).step_by(3) {
            if a[3] > 0.5 {
                acc += ((lum(a[0], a[1], a[2]) - lum(b[0], b[1], b[2])) as f64).powi(2);
                n += 1.0;
            }
        }
        if n > 0.0 {
            amp = ((acc / n).sqrt() * 1.6) as f32;
        }
    }
    let int = c.f("intensity") as f32;
    let size = c.px(c.f("size"));
    let seed = c.f("seed") as u32;
    let t = c.t * c.fps;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let g = grain_field(x as f64, y as f64, t, size, seed, 0) * amp * int;
            let p = &mut row[x * 4..x * 4 + 4];
            for k in 0..3 {
                p[k] = (p[k] + g * p[3]).max(0.0);
            }
        }
    });
}

fn noise_alpha(c: &FxCtx, img: &mut Img) {
    let amt = c.f("amount") as f32 / 100.0;
    if amt <= 0.0 {
        return;
    }
    let kind = c.e("noise");
    let orig_mode = c.e("original");
    let ov = c.e("overflow");
    let seed = c.f("seed") as u32;
    let ph = (c.f("phase") / 360.0) as f32;
    let anim = kind >= 2;
    let frame = if anim { 0 } else { (c.t * c.fps) as i32 };
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let mut n = if anim { value3(x as f32, y as f32, ph * 8.0, seed, true) * 0.5 + 0.5 } else { rand01(x as i32, y as i32, frame, seed) };
            if kind % 2 == 1 {
                n *= n;
            }
            let p = &mut row[x * 4..x * 4 + 4];
            let a = p[3];
            let na = match orig_mode {
                1 => a.min(n * amt + a * (1.0 - amt)),
                2 => a * (1.0 - amt + n * amt),
                3 => {
                    let e = 1.0 - (a * 2.0 - 1.0).abs();
                    a + (n - 0.5) * amt * e
                }
                _ => a + (n - 0.5) * amt,
            };
            let na = match ov {
                1 => {
                    let f = na.rem_euclid(2.0);
                    if f > 1.0 { 2.0 - f } else { f }
                }
                2 => na.rem_euclid(1.0),
                _ => na.clamp(0.0, 1.0),
            };
            if a > 1e-6 {
                let k = na / a;
                p[0] *= k;
                p[1] *= k;
                p[2] *= k;
            }
            p[3] = na;
        }
    });
}

fn noise_hls(c: &FxCtx, img: &mut Img, auto: bool) {
    let (hh, ll, ss) = (c.f("hue") as f32 / 100.0, c.f("lightness") as f32 / 100.0, c.f("saturation") as f32 / 100.0);
    if hh == 0.0 && ll == 0.0 && ss == 0.0 {
        return;
    }
    let kind = c.e("noise");
    let gs = c.px(c.f("grainSize")).max(1.0);
    let z = if auto { (c.t * c.f("animationSpeed") * 0.2) as f32 } else { (c.f("phase") / 360.0) as f32 * 4.0 };
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let p = &mut row[x * 4..x * 4 + 4];
            let a = p[3];
            if a <= 1e-6 {
                continue;
            }
            let n = |k: u32| -> f32 {
                let v = if kind == 2 { value3((x as f64 / gs) as f32, (y as f64 / gs) as f32, z, k, true) } else { rand01((x as f64 / gs) as i32, (y as f64 / gs) as i32, (z * 30.0) as i32, k) * 2.0 - 1.0 };
                if kind == 1 { v * v.abs() } else { v }
            };
            let mut hsl = rgb_to_hsl([p[0] / a, p[1] / a, p[2] / a]);
            hsl[0] = (hsl[0] + n(1) * hh * 0.5).rem_euclid(1.0);
            hsl[1] = (hsl[1] + n(2) * ss * 0.5).clamp(0.0, 1.0);
            hsl[2] = (hsl[2] + n(3) * ll * 0.5).clamp(0.0, 1.0);
            let o = hsl_to_rgb(hsl);
            for k in 0..3 {
                p[k] = o[k] * a;
            }
        }
    });
}

fn remove_grain(c: &FxCtx, img: &mut Img) {
    let amt = c.f("passes") as f32;
    if amt <= 0.0 {
        return;
    }
    let passes = c.f("passCount").round().clamp(1.0, 5.0) as usize;
    let original = img.clone();
    let detail = c.f("detail") as f32;
    let temporal = c.f("temporal") as f32;
    let fd = 1.0 / c.fps.max(1.0);
    let neighbors: Vec<Img> = if temporal > 0.0 { [-fd, fd].iter().filter_map(|dt| c.frame(*dt)).filter(|f| f.w == img.w && f.h == img.h).collect() } else { vec![] };
    for _ in 0..passes {
        let src = img.clone();
        let mut b = src.clone();
        gaussian(&mut b, 1.2 * c.r, 1.2 * c.r, true);
        img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
            let s = &src.px[i * 4..i * 4 + 4];
            let bb = &b.px[i * 4..i * 4 + 4];
            let d = (lum(s[0], s[1], s[2]) - lum(bb[0], bb[1], bb[2])).abs();
            let keep = (d * 12.0 * detail).min(1.0);
            let k = (amt * 0.6).min(1.0) * (1.0 - keep);
            for j in 0..4 {
                p[j] = s[j] + (bb[j] - s[j]) * k;
            }
        });
    }
    if !neighbors.is_empty() {
        let n = neighbors.len() as f32;
        img.px.par_iter_mut().enumerate().for_each(|(i, v)| {
            let avg = neighbors.iter().map(|f| f.px[i]).sum::<f32>() / n;
            *v += (avg - *v) * temporal * 0.5;
        });
    }
    if c.e("viewMode") == 0 {
        let (w, h) = (img.w, img.h);
        let (x0, x1, y0, y1) = (w / 3, w * 2 / 3, h / 3, h * 2 / 3);
        img.px.par_chunks_mut(w * 4).zip(original.px.par_chunks(w * 4)).enumerate().for_each(|(y, (row, orow))| {
            for x in 0..w {
                let inside = x >= x0 && x < x1 && y >= y0 && y < y1;
                if !inside {
                    row[x * 4..x * 4 + 4].copy_from_slice(&orow[x * 4..x * 4 + 4]);
                } else if x == x0 || x + 1 == x1 || y == y0 || y + 1 == y1 {
                    row[x * 4..x * 4 + 4].copy_from_slice(&[1.0, 1.0, 1.0, 1.0]);
                }
            }
        });
    }
}

fn noise_basis(kind: u32, x: f32, y: f32, z: f32, seed: u32) -> f32 {
    match kind {
        0 => rand01(x.floor() as i32, y.floor() as i32, z.floor() as i32, seed) * 2.0 - 1.0,
        1 => value3(x, y, z, seed, false),
        3 => perlin3(x, y, z, seed) * 1.4,
        _ => value3(x, y, z, seed, true),
    }
}

fn turbulent_noise(c: &FxCtx, img: &mut Img) {
    let ft = c.e("fractalType");
    let nt = c.e("noiseType");
    let inv = c.b("invert");
    let con = c.f("contrast") as f32 / 100.0;
    let br = c.f("brightness") as f32 / 100.0;
    let sc = c.px(c.f("scale")).max(1.0);
    let rot = c.f("rotation").to_radians();
    let oct = c.f("complexity").clamp(1.0, 20.0);
    let evo = (c.f("evolution") / 360.0) as f32;
    let op = c.f("opacity") as f32 / 100.0;
    let (s, co) = rot.sin_cos();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (u, v) = ((x as f64 * co + y as f64 * s) / sc, (-(x as f64) * s + y as f64 * co) / sc);
            let mut acc = 0.0f32;
            let mut amp = 1.0f32;
            let mut f = 1.0f32;
            let mut norm = 0.0f32;
            let n = oct.ceil() as usize;
            for o in 0..n {
                let wgt = if o + 1 == n { (oct - o as f64) as f32 } else { 1.0 };
                let mut val = noise_basis(nt, u as f32 * f, v as f32 * f, evo * 3.0 + o as f32 * 1.7, 13 + o as u32);
                match ft {
                    1 | 2 => val = val.abs() * 2.0 - 1.0,
                    3 => val = (val * (1.0 + acc.abs())).clamp(-1.0, 1.0),
                    4 => val = val.max(acc),
                    5 => val = 1.0 - val.abs() * 2.0,
                    _ => {}
                }
                acc += val * amp * wgt;
                norm += amp * wgt;
                amp *= 0.5;
                f *= 2.0;
            }
            let mut g = acc / norm.max(1e-6) * 0.5 + 0.5;
            if ft == 1 {
                g = g * g;
            }
            g = ((g - 0.5) * con + 0.5 + br).clamp(0.0, 1.0);
            if inv {
                g = 1.0 - g;
            }
            let p = &mut row[x * 4..x * 4 + 4];
            for k in 0..3 {
                p[k] += (g - p[k]) * op;
            }
            p[3] += (1.0 - p[3]) * op;
        }
    });
}
