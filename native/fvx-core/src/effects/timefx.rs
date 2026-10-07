use rayon::prelude::*;

use super::util::lum;
use super::{EffectSpec, FxCtx, check, layer_param, no_pad, num, pct, popup, slider};
use crate::render::image::Img;

const CAT: &str = "Time";

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.time.echo",
            name: "Echo",
            category: CAT,
            params: vec![
                slider("echoTime", "Echo Time (seconds)", -0.033, -10.0, 10.0, 3),
                slider("echoes", "Number Of Echoes", 1.0, 0.0, 50.0, 0),
                slider("startingIntensity", "Starting Intensity", 1.0, 0.0, 1.0, 2),
                slider("decay", "Decay", 1.0, 0.0, 1.0, 2),
                popup("operator", "Echo Operator", 0, &["Add", "Maximum", "Minimum", "Screen", "Composite In Back", "Composite In Front", "Blend"]),
            ],
            render: echo,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.time.posterizeTime",
            name: "Posterize Time",
            category: CAT,
            params: vec![slider("frameRate", "Frame Rate", 12.0, 0.1, 99.0, 2)],
            render: |c, img| {
                let fr = c.f("frameRate").max(0.1);
                let t = c.comp_time();
                let snapped = (t * fr + 1e-6).floor() / fr;
                let dt = snapped - t;
                if dt.abs() < 1e-6 {
                    return;
                }
                if let Some(f) = c.frame(dt).filter(|f| f.w == img.w && f.h == img.h) {
                    *img = f;
                }
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.time.timeDifference",
            name: "Time Difference",
            category: CAT,
            params: vec![layer_param("target", "Target"), slider("timeOffset", "Time Offset (sec)", 0.0, -100.0, 100.0, 3), pct("contrast", "Contrast", 50.0), check("absolute", "Absolute Difference", true), popup("alpha", "Alpha Channel", 0, &["Original", "Target", "Blend", "Max", "Full On", "Lightness of Result", "Max of Result", "Alpha Difference", "Alpha Difference Only"])],
            render: time_difference,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.time.timeDisplacement",
            name: "Time Displacement",
            category: CAT,
            params: vec![layer_param("map", "Time Displacement Layer"), slider("maxDisplacement", "Max Displacement Time [sec]", 1.0, -10.0, 10.0, 3), slider("resolution", "Time Resolution [fps]", 60.0, 1.0, 120.0, 1), check("stretch", "If Layer Sizes Differ: Stretch Map to Fit", true)],
            render: time_displacement,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.time.timewarp",
            name: "Timewarp",
            category: CAT,
            params: vec![popup("method", "Method", 1, &["Whole Frames", "Frame Mix", "Pixel Motion"]), popup("adjust", "Adjust Time By", 0, &["Speed", "Source Frame"]), num("speed", "Speed", 50.0, -1000.0, 1000.0, 0.0, 200.0), slider("sourceFrame", "Source Frame", 0.0, -100000.0, 100000.0, 2)],
            render: timewarp,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.time.ccForceMotionBlur",
            name: "CC Force Motion Blur",
            category: CAT,
            params: vec![slider("samples", "Motion Blur Samples", 8.0, 2.0, 64.0, 0), check("override", "Override Shutter Angle", true), slider("shutterAngle", "Shutter Angle", 180.0, 0.0, 3600.0, 0)],
            render: force_motion_blur,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.time.ccWideTime",
            name: "CC Wide Time",
            category: CAT,
            params: vec![slider("forward", "Forward Steps", 3.0, 0.0, 50.0, 0), slider("backward", "Backward Steps", 3.0, 0.0, 50.0, 0), check("nativeMotionBlur", "Native Motion Blur", false)],
            render: wide_time,
            pad: no_pad,
        },
    ]
}

fn echo(c: &FxCtx, img: &mut Img) {
    let et = c.f("echoTime");
    let n = c.f("echoes").round().clamp(0.0, 50.0) as usize;
    if n == 0 || et.abs() < 1e-6 {
        return;
    }
    let start = c.f("startingIntensity") as f32;
    let decay = c.f("decay") as f32;
    let op = c.e("operator");
    let base = img.clone();
    let mut layers = vec![(base, start)];
    let mut k = start;
    for i in 1..=n {
        k *= decay;
        if let Some(f) = c.frame(et * i as f64).filter(|f| f.w == img.w && f.h == img.h) {
            layers.push((f, k));
        }
    }
    let wsum: f32 = layers.iter().map(|l| l.1).sum::<f32>().max(1e-6);
    img.px.par_iter_mut().enumerate().for_each(|(i, out)| {
        let ch = i % 4;
        let mut acc = if op == 2 { f32::MAX } else { 0.0 };
        match op {
            4 => {
                for (l, w) in layers.iter().rev() {
                    let a = l.px[i - ch + 3] * w;
                    let v = l.px[i] * w;
                    acc = v + acc * (1.0 - a);
                }
            }
            5 => {
                for (l, w) in layers.iter() {
                    let a = l.px[i - ch + 3] * w;
                    let v = l.px[i] * w;
                    acc = v + acc * (1.0 - a);
                }
            }
            6 => {
                for (l, w) in layers.iter() {
                    acc += l.px[i] * w / wsum;
                }
            }
            _ => {
                for (l, w) in layers.iter() {
                    let v = l.px[i] * w;
                    acc = match op {
                        1 => acc.max(v),
                        2 => acc.min(v),
                        3 => 1.0 - (1.0 - acc) * (1.0 - v.min(1.0)),
                        _ => acc + v,
                    };
                }
            }
        }
        *out = if ch == 3 { acc.clamp(0.0, 1.0) } else { acc.max(0.0) };
    });
}

fn time_difference(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let off = c.f("timeOffset");
    let target = match (c.layer_id("target"), c.host) {
        (Some(id), Some(host)) => host.layer(id, true, off).map(|(i, s)| super::util::fit_layer(&i, s, c, w, h, 1)),
        _ => c.frame(off),
    };
    let Some(t) = target.filter(|t| t.w == w && t.h == h) else { return };
    let con = c.f("contrast") as f32 / 100.0;
    let abs = c.b("absolute");
    let amode = c.e("alpha");
    img.px.par_chunks_mut(4).zip(t.px.par_chunks(4)).for_each(|(p, q)| {
        let mut o = [0.0f32; 4];
        for k in 0..3 {
            let d = p[k] - q[k];
            o[k] = if abs { (d.abs() * (1.0 + con * 4.0)).clamp(0.0, 1.0) } else { (d * (1.0 + con * 4.0) + 0.5).clamp(0.0, 1.0) };
        }
        o[3] = match amode {
            1 => q[3],
            2 => (p[3] + q[3]) * 0.5,
            3 => p[3].max(q[3]),
            4 => 1.0,
            5 => lum(o[0], o[1], o[2]),
            6 => o[0].max(o[1]).max(o[2]),
            7 | 8 => (p[3] - q[3]).abs(),
            _ => p[3],
        };
        if amode == 8 {
            o[0] = o[3];
            o[1] = o[3];
            o[2] = o[3];
        }
        for k in 0..3 {
            p[k] = o[k] * o[3];
        }
        p[3] = o[3];
    });
}

fn time_displacement(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let map = c.other_fit("map", true, w, h, if c.b("stretch") { 1 } else { 0 }).unwrap_or_else(|| img.clone());
    let maxd = c.f("maxDisplacement");
    let res = c.f("resolution").clamp(1.0, 120.0);
    let steps = ((maxd.abs() * res).ceil() as usize).clamp(1, 120);
    let mut frames: Vec<(f64, Img)> = vec![];
    for i in 0..=steps * 2 {
        let dt = (i as f64 - steps as f64) / steps as f64 * maxd.abs();
        if dt.abs() < 1e-9 {
            frames.push((0.0, img.clone()));
        } else if let Some(f) = c.frame(dt).filter(|f| f.w == w && f.h == h) {
            frames.push((dt, f));
        }
    }
    if frames.len() < 2 {
        return;
    }
    let lut: Vec<f64> = frames.iter().map(|f| f.0).collect();
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let m = &map.px[i * 4..i * 4 + 4];
        let v = if m[3] > 1e-6 { lum(m[0] / m[3], m[1] / m[3], m[2] / m[3]) } else { 0.5 };
        let dt = (v as f64 - 0.5) * 2.0 * maxd;
        let k = lut.iter().enumerate().min_by(|a, b| (a.1 - dt).abs().total_cmp(&(b.1 - dt).abs())).map(|x| x.0).unwrap_or(0);
        p.copy_from_slice(&frames[k].1.px[i * 4..i * 4 + 4]);
    });
}

fn timewarp(c: &FxCtx, img: &mut Img) {
    let Some(host) = c.host else { return };
    let fps = host.frame_rate().max(1.0);
    let t = c.comp_time();
    let src_t = if c.e("adjust") == 1 { c.f("sourceFrame") / fps } else { t * c.f("speed") / 100.0 };
    let dt = src_t - t;
    let method = c.e("method");
    let fpos = src_t * fps;
    let f0 = fpos.floor();
    let frac = (fpos - f0) as f32;
    if method == 0 || frac < 1e-3 {
        if let Some(f) = c.frame(f0 / fps - t).filter(|f| f.w == img.w && f.h == img.h) {
            *img = f;
        }
        return;
    }
    let a = c.frame(f0 / fps - t).filter(|f| f.w == img.w && f.h == img.h);
    let b = c.frame((f0 + 1.0) / fps - t).filter(|f| f.w == img.w && f.h == img.h);
    match (a, b) {
        (Some(a), Some(b)) => {
            img.px.par_iter_mut().zip(a.px.par_iter().zip(b.px.par_iter())).for_each(|(o, (x, y))| *o = x + (y - x) * frac);
        }
        (Some(a), None) => *img = a,
        _ => {
            if let Some(f) = c.frame(dt) {
                *img = f;
            }
        }
    }
}

fn force_motion_blur(c: &FxCtx, img: &mut Img) {
    let Some(host) = c.host else { return };
    let n = c.f("samples").round().clamp(2.0, 64.0) as usize;
    let ang = if c.b("override") { c.f("shutterAngle") } else { 180.0 };
    let fd = 1.0 / host.frame_rate().max(1.0);
    let span = ang / 360.0 * fd;
    let mut acc = vec![0.0f32; img.px.len()];
    let mut m = 0.0f32;
    for k in 0..n {
        let dt = (k as f64 / (n - 1) as f64 - 0.5) * span;
        let f = if dt.abs() < 1e-9 { Some(img.clone()) } else { c.frame(dt) };
        if let Some(f) = f.filter(|f| f.w == img.w && f.h == img.h) {
            acc.par_iter_mut().zip(f.px.par_iter()).for_each(|(a, v)| *a += v);
            m += 1.0;
        }
    }
    if m > 0.0 {
        img.px.par_iter_mut().zip(acc.par_iter()).for_each(|(o, a)| *o = a / m);
    }
}

fn wide_time(c: &FxCtx, img: &mut Img) {
    let Some(host) = c.host else { return };
    let fd = 1.0 / host.frame_rate().max(1.0);
    let fw = c.f("forward").round().max(0.0) as i64;
    let bw = c.f("backward").round().max(0.0) as i64;
    if fw == 0 && bw == 0 {
        return;
    }
    let sub = if c.b("nativeMotionBlur") { 2 } else { 1 };
    let mut acc = img.px.clone();
    let mut m = 1.0f32;
    for k in -bw * sub..=fw * sub {
        if k == 0 {
            continue;
        }
        if let Some(f) = c.frame(k as f64 * fd / sub as f64).filter(|f| f.w == img.w && f.h == img.h) {
            acc.par_iter_mut().zip(f.px.par_iter()).for_each(|(a, v)| *a += v);
            m += 1.0;
        }
    }
    img.px.par_iter_mut().zip(acc.par_iter()).for_each(|(o, a)| *o = a / m);
}
