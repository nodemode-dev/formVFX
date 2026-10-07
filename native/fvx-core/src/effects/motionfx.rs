use std::sync::Arc;

use rayon::prelude::*;

use super::util::lum;
use super::{EffectSpec, FxCtx, no_pad, num, popup, slider};
use crate::render::image::Img;
use crate::render::vision::{Flow, flow_cached};

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.blur.cameraShakeDeblur",
            name: "Camera-Shake Deblur",
            category: "Blur & Sharpen",
            params: vec![
                slider("duration", "Blur Duration", 3.0, 1.0, 10.0, 0),
                popup("method", "Deblur Method", 0, &["High Quality", "Standard"]),
                slider("strength", "Strength", 100.0, 100.0, 200.0, 0),
                slider("sensitivity", "Shake Sensitivity", 50.0, 0.0, 100.0, 0),
            ],
            render: deblur,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.rollingShutterRepair",
            name: "Rolling Shutter Repair",
            category: "Distort",
            params: vec![
                slider("rate", "Rolling Shutter Rate", 50.0, 0.0, 100.0, 0),
                popup("direction", "Scan Direction", 0, &["Top \u{2192} Bottom", "Bottom \u{2192} Top", "Left \u{2192} Right", "Right \u{2192} Left"]),
                popup("method", "Method", 0, &["Warp", "Pixel Motion"]),
            ],
            render: rolling_shutter,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.time.pixelMotionBlur",
            name: "Pixel Motion Blur",
            category: "Time",
            params: vec![
                popup("shutterControl", "Shutter Control", 0, &["Automatic", "Manual"]),
                num("shutterAngle", "Shutter Angle", 180.0, 0.0, 720.0, 0.0, 360.0),
                slider("samples", "Shutter Samples", 16.0, 2.0, 64.0, 0),
                slider("vectorDetail", "Vector Detail", 20.0, 0.0, 100.0, 0),
            ],
            render: pixel_motion_blur,
            pad: no_pad,
        },
    ]
}

fn sharpness(img: &Img) -> f32 {
    let step = (img.w.max(img.h) / 480).max(1);
    let (w, h) = (img.w / step, img.h / step);
    if w < 3 || h < 3 {
        return 0.0;
    }
    let g = |x: usize, y: usize| {
        let p = &img.px[((y * step) * img.w + x * step) * 4..][..4];
        lum(p[0], p[1], p[2])
    };
    let mut acc = 0.0f64;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let l = 4.0 * g(x, y) - g(x - 1, y) - g(x + 1, y) - g(x, y - 1) - g(x, y + 1);
            acc += (l * l) as f64;
        }
    }
    (acc / ((w - 2) * (h - 2)) as f64) as f32
}

fn mean_flow(f: &Flow) -> [f32; 2] {
    let n = f.v.len().max(1) as f32;
    let s = f.v.iter().fold([0.0f32; 2], |a, v| [a[0] + v[0], a[1] + v[1]]);
    [s[0] / n / f.scale, s[1] / n / f.scale]
}

fn smoothed(f: &Flow, sigma: f32) -> Flow {
    if sigma < 0.5 {
        return Flow { w: f.w, h: f.h, scale: f.scale, v: f.v.clone() };
    }
    let mut out = f.v.clone();
    for k in 0..2 {
        let mut ch: Vec<f32> = f.v.iter().map(|v| v[k]).collect();
        super::util::blur_scalar(&mut ch, f.w, f.h, sigma as f64, sigma as f64);
        for (o, v) in out.iter_mut().zip(ch) {
            o[k] = v;
        }
    }
    Flow { w: f.w, h: f.h, scale: f.scale, v: out }
}

fn frame_of(c: &FxCtx, dt: f64, w: usize, h: usize) -> Option<Arc<Img>> {
    c.frame(dt).filter(|f| f.w == w && f.h == h).map(Arc::new)
}

fn deblur(c: &FxCtx, img: &mut Img) {
    let fd = 1.0 / c.fps.max(1.0);
    let n = c.f("duration").round().clamp(1.0, 10.0) as i64;
    let cur = Arc::new(img.clone());
    let s0 = sharpness(&cur);
    let mut best: Option<(Arc<Img>, f32)> = None;
    for k in -n..=n {
        if k == 0 {
            continue;
        }
        let Some(f) = frame_of(c, k as f64 * fd, img.w, img.h) else { continue };
        let s = sharpness(&f);
        if best.as_ref().is_none_or(|b| s > b.1) {
            best = Some((f, s));
        }
    }
    let Some((sharp, sb)) = best else { return };
    let thresh = 0.5 + 0.45 * (c.f("sensitivity") as f32 / 100.0);
    if sb <= 0.0 || s0 >= sb * thresh {
        return;
    }
    let gain = (((sb - s0) / sb) * 2.0).clamp(0.0, 1.0) * (c.f("strength") as f32 / 100.0);
    let flow = flow_cached(&cur, &sharp);
    let global = (c.e("method") == 1).then(|| mean_flow(&flow));
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            let v = global.unwrap_or_else(|| flow.at(cx, cy));
            let q = sharp.sample((cx + v[0]) as f64, (cy + v[1]) as f64, true);
            for k in 0..4 {
                let o = &mut row[x * 4 + k];
                *o += (q[k] - *o) * gain;
                if k < 3 {
                    *o = o.max(0.0);
                }
            }
        }
    });
}

fn rolling_shutter(c: &FxCtx, img: &mut Img) {
    let rate = c.f("rate") as f32 / 100.0;
    if rate <= 0.0 {
        return;
    }
    let fd = 1.0 / c.fps.max(1.0);
    let cur = Arc::new(img.clone());
    let (fwd, back) = (frame_of(c, fd, img.w, img.h), frame_of(c, -fd, img.w, img.h));
    let flow = match (fwd, back) {
        (Some(n), _) => flow_cached(&cur, &n),
        (None, Some(p)) => {
            let b = flow_cached(&cur, &p);
            Arc::new(Flow { w: b.w, h: b.h, scale: b.scale, v: b.v.iter().map(|v| [-v[0], -v[1]]).collect() })
        }
        _ => return,
    };
    let flow = if c.e("method") == 0 { Arc::new(smoothed(&flow, (flow.w.max(flow.h) as f32) / 24.0)) } else { flow };
    let (w, h) = (img.w, img.h);
    let dir = c.e("direction");
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            let pos = match dir {
                0 => cy / h as f32,
                1 => 1.0 - cy / h as f32,
                2 => cx / w as f32,
                _ => 1.0 - cx / w as f32,
            };
            let s = rate * (pos - 0.5);
            let v = flow.at(cx, cy);
            let q = cur.sample((cx + s * v[0]) as f64, (cy + s * v[1]) as f64, true);
            row[x * 4..x * 4 + 4].copy_from_slice(&q);
        }
    });
}

fn pixel_motion_blur(c: &FxCtx, img: &mut Img) {
    let fd = 1.0 / c.fps.max(1.0);
    let angle = if c.e("shutterControl") == 0 { c.host.map(|h| h.shutter_angle()).unwrap_or(180.0) } else { c.f("shutterAngle") };
    let span = (angle / 360.0) as f32;
    if span <= 0.0 {
        return;
    }
    let cur = Arc::new(img.clone());
    let (w, h) = (img.w, img.h);
    let f = frame_of(c, fd, w, h).map(|n| flow_cached(&cur, &n));
    let b = frame_of(c, -fd, w, h).map(|p| flow_cached(&cur, &p));
    let (fw, fh, scale, v) = match (&f, &b) {
        (Some(f), Some(b)) => (f.w, f.h, f.scale, f.v.iter().zip(b.v.iter()).map(|(a, q)| [(a[0] - q[0]) * 0.5, (a[1] - q[1]) * 0.5]).collect::<Vec<_>>()),
        (Some(f), None) => (f.w, f.h, f.scale, f.v.clone()),
        (None, Some(b)) => (b.w, b.h, b.scale, b.v.iter().map(|q| [-q[0], -q[1]]).collect()),
        _ => return,
    };
    let detail = c.f("vectorDetail") as f32 / 100.0;
    let flow = smoothed(&Flow { w: fw, h: fh, scale, v }, (1.0 - detail) * (fw.max(fh) as f32) / 48.0);
    let n = c.f("samples").round().clamp(2.0, 64.0) as usize;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            let v = flow.at(cx, cy);
            let mut acc = [0.0f32; 4];
            for i in 0..n {
                let t = (i as f32 / (n - 1) as f32 - 0.5) * span;
                let q = cur.sample((cx + t * v[0]) as f64, (cy + t * v[1]) as f64, true);
                for k in 0..4 {
                    acc[k] += q[k];
                }
            }
            for k in 0..4 {
                row[x * 4 + k] = acc[k] / n as f32;
            }
        }
    });
}
