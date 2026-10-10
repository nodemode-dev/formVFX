use rayon::prelude::*;

use super::util::{lum, minmax, rgb_to_hsl, hsl_to_rgb};
use super::{EffectSpec, FxCtx, check, color, layer_param, no_pad, num, pct, popup, slider};
use crate::render::image::Img;

const CAT: &str = "Channel";

const SRC_CH: [&str; 14] = [
    "Red", "Green", "Blue", "Alpha", "Luminance", "Hue", "Lightness", "Saturation", "Full", "Half", "Off", "Full On", "Full Off", "Matte",
];

const ARITH: [&str; 13] = ["And", "Or", "Xor", "Add", "Subtract", "Difference", "Min", "Max", "Block Above", "Block Below", "Slice", "Multiply", "Screen"];

const BLEND_MODES: [&str; 5] = ["Crossfade", "Color Only", "Tint Only", "Darken Only", "Lighten Only"];

const COMPOUND_OPS: [&str; 15] = ["Copy", "Add", "Subtract", "Multiply", "Difference", "And", "Or", "Xor", "Lighten", "Darken", "Minimum", "Maximum", "Screen", "Overlay", "Hard Light"];

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.channel.arithmetic",
            name: "Arithmetic",
            category: CAT,
            params: vec![
                popup("operator", "Operator", 0, &ARITH),
                num("red", "Red Value", 0.0, 0.0, 255.0, 0.0, 255.0),
                num("green", "Green Value", 0.0, 0.0, 255.0, 0.0, 255.0),
                num("blue", "Blue Value", 0.0, 0.0, 255.0, 0.0, 255.0),
                check("clip", "Clip Result Values", true),
            ],
            render: arithmetic,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.blend",
            name: "Blend",
            category: CAT,
            params: vec![
                layer_param("layer", "Blend With Layer"),
                popup("mode", "Mode", 0, &BLEND_MODES),
                pct("blend", "Blend With Original", 50.0),
                popup("ifSizesDiffer", "If Layer Sizes Differ", 0, &["Center", "Stretch to Fit"]),
            ],
            render: blend_fx,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.calculations",
            name: "Calculations",
            category: CAT,
            params: vec![
                popup("inputChannel", "Input Channel", 0, &["RGBA", "Gray", "Red", "Green", "Blue", "Alpha"]),
                check("invertInput", "Invert Input", false),
                layer_param("secondLayer", "Second Layer"),
                popup("secondChannel", "Second Layer Channel", 0, &["RGBA", "Gray", "Red", "Green", "Blue", "Alpha"]),
                pct("secondOpacity", "Second Layer Opacity", 100.0),
                check("invertSecond", "Invert Second Layer", false),
                check("stretchSecond", "Stretch Second Layer to Fit", true),
                popup("mode", "Blending Mode", 0, &["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light", "Darken", "Lighten", "Difference", "Exclusion"]),
                check("preserveTransparency", "Preserve Transparency", false),
            ],
            render: calculations,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.ccComposite",
            name: "CC Composite",
            category: CAT,
            params: vec![
                pct("opacity", "Opacity", 100.0),
                popup("mode", "Composite Original", 0, &["Behind", "In Front", "Add", "Multiply", "Screen", "Darken", "Lighten", "Difference"]),
                check("rgbOnly", "RGB Only", false),
            ],
            render: cc_composite,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.channelCombiner",
            name: "Channel Combiner",
            category: CAT,
            params: vec![
                check("useSecondLayer", "Use 2nd Layer", false),
                layer_param("sourceLayer", "Source Layer"),
                popup("from", "From", 0, &["RGB to HLS", "HLS to RGB", "RGB to YUV", "YUV to RGB", "Red Only", "Green Only", "Blue Only", "Alpha Only", "Lightness Only", "Hue Only", "Saturation Only", "Min RGB", "Max RGB"]),
                popup("to", "To", 0, &["Red Only", "Green Only", "Blue Only", "Alpha Only", "Lightness Only", "Hue Only", "Saturation Only"]),
                check("invert", "Invert", false),
                check("solidAlpha", "Solid Alpha", false),
            ],
            render: channel_combiner,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.compoundArithmetic",
            name: "Compound Arithmetic",
            category: CAT,
            params: vec![
                layer_param("secondSource", "Second Source Layer"),
                popup("operator", "Operator", 0, &COMPOUND_OPS),
                popup("operateOn", "Operate On Channels", 0, &["RGB", "ARGB", "Alpha"]),
                popup("overflow", "Overflow Behavior", 0, &["Clip", "Wrap", "Scale"]),
                check("stretch", "Stretch Second Source to Fit", true),
                pct("blend", "Blend With Original", 0.0),
            ],
            render: compound_arithmetic,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.minimax",
            name: "Minimax",
            category: CAT,
            params: vec![
                popup("operation", "Operation", 0, &["Maximum", "Minimum", "Minimum then Maximum", "Maximum then Minimum"]),
                num("radius", "Radius", 0.0, 0.0, 400.0, 0.0, 50.0),
                popup("channel", "Channel", 0, &["Color", "Alpha and Color", "Alpha", "Red", "Green", "Blue"]),
                popup("direction", "Direction", 0, &["Horizontal & Vertical", "Just Horizontal", "Just Vertical"]),
                check("dontShrink", "Don't Shrink Edges", false),
            ],
            render: minimax,
            pad: |c| c.f("radius"),
        },
        EffectSpec {
            id: "fx.channel.removeColorMatting",
            name: "Remove Color Matting",
            category: CAT,
            params: vec![color("background", "Background Color", [0.0, 0.0, 0.0]), check("clip", "Clipping", true)],
            render: |c, img| {
                let bg = c.c("background");
                let clip = c.b("clip");
                img.px.par_chunks_mut(4).for_each(|p| {
                    let a = p[3];
                    if a <= 1e-6 {
                        return;
                    }
                    for k in 0..3 {
                        let u = p[k] / a;
                        let mut v = (u - bg[k]) / a + bg[k];
                        if clip {
                            v = v.clamp(0.0, 1.0);
                        }
                        p[k] = v * a;
                    }
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.setChannels",
            name: "Set Channels",
            category: CAT,
            params: vec![
                layer_param("redLayer", "Source Layer 1"),
                popup("redFrom", "Set Red To Source 1's", 0, &SRC_CH),
                layer_param("greenLayer", "Source Layer 2"),
                popup("greenFrom", "Set Green To Source 2's", 1, &SRC_CH),
                layer_param("blueLayer", "Source Layer 3"),
                popup("blueFrom", "Set Blue To Source 3's", 2, &SRC_CH),
                layer_param("alphaLayer", "Source Layer 4"),
                popup("alphaFrom", "Set Alpha To Source 4's", 3, &SRC_CH),
                popup("ifSizesDiffer", "If Layer Sizes Differ", 1, &["Center", "Stretch to Fit"]),
            ],
            render: set_channels,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.setMatte",
            name: "Set Matte",
            category: CAT,
            params: vec![
                layer_param("takeMatteFrom", "Take Matte From Layer"),
                popup("useForMatte", "Use For Matte", 3, &SRC_CH[..11]),
                check("invert", "Invert Matte", false),
                check("stretch", "Stretch Matte to Fit", true),
                check("composite", "Composite Matte with Original", true),
                check("premultiply", "Premultiply Matte Layer", true),
            ],
            render: set_matte,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.shiftChannels",
            name: "Shift Channels",
            category: CAT,
            params: vec![
                popup("alpha", "Take Alpha From", 3, &SRC_CH[..13]),
                popup("red", "Take Red From", 0, &SRC_CH[..13]),
                popup("green", "Take Green From", 1, &SRC_CH[..13]),
                popup("blue", "Take Blue From", 2, &SRC_CH[..13]),
            ],
            render: shift_channels,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.solidComposite",
            name: "Solid Composite",
            category: CAT,
            params: vec![
                pct("sourceOpacity", "Source Opacity", 100.0),
                color("color", "Color", [1.0, 1.0, 1.0]),
                pct("opacity", "Opacity", 100.0),
                popup("mode", "Blending Mode", 0, &["Normal", "Add", "Multiply", "Screen", "Overlay", "Darken", "Lighten", "Difference"]),
            ],
            render: solid_composite,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.ccThreshold",
            name: "CC Threshold",
            category: "Stylize",
            params: vec![slider("threshold", "Threshold", 127.0, 0.0, 255.0, 1), popup("channel", "Channel", 0, &["Luminance", "Red", "Green", "Blue", "Alpha"]), check("invert", "Invert", false), pct("blend", "Blend w. Original", 0.0)],
            render: |c, img| {
                let th = c.f("threshold") as f32 / 255.0;
                let ch = c.e("channel");
                let inv = c.b("invert");
                let bl = c.f("blend") as f32 / 100.0;
                img.px.par_chunks_mut(4).for_each(|p| {
                    let a = p[3];
                    if a <= 1e-6 {
                        return;
                    }
                    let u = [p[0] / a, p[1] / a, p[2] / a];
                    let v = match ch {
                        1 => u[0],
                        2 => u[1],
                        3 => u[2],
                        4 => a,
                        _ => lum(u[0], u[1], u[2]),
                    };
                    let mut o = if v >= th { 1.0 } else { 0.0 };
                    if inv {
                        o = 1.0 - o;
                    }
                    for k in 0..3 {
                        p[k] = (o + (u[k] - o) * bl) * a;
                    }
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.ccThresholdRgb",
            name: "CC Threshold RGB",
            category: "Stylize",
            params: vec![
                slider("red", "Red Threshold", 127.0, 0.0, 255.0, 1),
                slider("green", "Green Threshold", 127.0, 0.0, 255.0, 1),
                slider("blue", "Blue Threshold", 127.0, 0.0, 255.0, 1),
                check("invertRed", "Invert Red Channel", false),
                check("invertGreen", "Invert Green Channel", false),
                check("invertBlue", "Invert Blue Channel", false),
                pct("blend", "Blend w. Original", 0.0),
            ],
            render: |c, img| {
                let th = [c.f("red") as f32 / 255.0, c.f("green") as f32 / 255.0, c.f("blue") as f32 / 255.0];
                let inv = [c.b("invertRed"), c.b("invertGreen"), c.b("invertBlue")];
                let bl = c.f("blend") as f32 / 100.0;
                img.px.par_chunks_mut(4).for_each(|p| {
                    let a = p[3];
                    if a <= 1e-6 {
                        return;
                    }
                    for k in 0..3 {
                        let u = p[k] / a;
                        let mut o = if u >= th[k] { 1.0 } else { 0.0 };
                        if inv[k] {
                            o = 1.0 - o;
                        }
                        p[k] = (o + (u - o) * bl) * a;
                    }
                });
            },
            pad: no_pad,
        },
    ]
}

fn chan_of(p: &[f32], which: u32) -> f32 {
    let a = p[3];
    let u = if a > 1e-6 { [p[0] / a, p[1] / a, p[2] / a] } else { [0.0; 3] };
    match which {
        0 => u[0],
        1 => u[1],
        2 => u[2],
        3 => a,
        4 => lum(u[0], u[1], u[2]),
        5 => rgb_to_hsl(u)[0],
        6 => rgb_to_hsl(u)[2],
        7 => rgb_to_hsl(u)[1],
        8 | 11 => 1.0,
        9 => 0.5,
        10 | 12 => 0.0,
        _ => a,
    }
}

fn arithmetic(c: &FxCtx, img: &mut Img) {
    let op = c.e("operator");
    let v = [c.f("red") as f32 / 255.0, c.f("green") as f32 / 255.0, c.f("blue") as f32 / 255.0];
    let clip = c.b("clip");
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        for k in 0..3 {
            let x = p[k] / a;
            let (xi, vi) = ((x.clamp(0.0, 1.0) * 255.0) as u8, (v[k] * 255.0) as u8);
            let mut y = match op {
                0 => (xi & vi) as f32 / 255.0,
                1 => (xi | vi) as f32 / 255.0,
                2 => (xi ^ vi) as f32 / 255.0,
                3 => x + v[k],
                4 => x - v[k],
                5 => (x - v[k]).abs(),
                6 => x.min(v[k]),
                7 => x.max(v[k]),
                8 => if x > v[k] { 0.0 } else { x },
                9 => if x < v[k] { 0.0 } else { x },
                10 => if x >= v[k] { 1.0 } else { 0.0 },
                11 => x * v[k],
                _ => 1.0 - (1.0 - x) * (1.0 - v[k]),
            };
            if clip {
                y = y.clamp(0.0, 1.0);
            }
            p[k] = y * a;
        }
    });
}

fn blend_fx(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let o = c.other_fit("layer", true, w, h, c.e("ifSizesDiffer")).unwrap_or_else(|| Img::new(w, h));
    let mode = c.e("mode");
    let k = 1.0 - c.f("blend") as f32 / 100.0;
    img.px.par_chunks_mut(4).zip(o.px.par_chunks(4)).for_each(|(p, q)| {
        let (a, b) = (p[3], q[3]);
        let u = if a > 1e-6 { [p[0] / a, p[1] / a, p[2] / a] } else { [0.0; 3] };
        let v = if b > 1e-6 { [q[0] / b, q[1] / b, q[2] / b] } else { [0.0; 3] };
        let (mut r, na) = match mode {
            1 => {
                let mut hu = rgb_to_hsl(u);
                let hv = rgb_to_hsl(v);
                hu[0] = hv[0];
                hu[1] = hv[1];
                (hsl_to_rgb(hu), a)
            }
            2 => {
                let mut hu = rgb_to_hsl(u);
                hu[0] = rgb_to_hsl(v)[0];
                (hsl_to_rgb(hu), a)
            }
            3 => ([u[0].min(v[0]), u[1].min(v[1]), u[2].min(v[2])], a),
            4 => ([u[0].max(v[0]), u[1].max(v[1]), u[2].max(v[2])], a),
            _ => (v, b),
        };
        let fa = a + (na - a) * k;
        for i in 0..3 {
            r[i] = u[i] + (r[i] - u[i]) * k;
            p[i] = r[i] * fa;
        }
        p[3] = fa;
    });
}

fn pick(p: &[f32], ch: u32) -> [f32; 4] {
    let a = p[3];
    let u = if a > 1e-6 { [p[0] / a, p[1] / a, p[2] / a] } else { [0.0; 3] };
    match ch {
        1 => {
            let g = lum(u[0], u[1], u[2]);
            [g, g, g, a]
        }
        2 => [u[0], u[0], u[0], a],
        3 => [u[1], u[1], u[1], a],
        4 => [u[2], u[2], u[2], a],
        5 => [a, a, a, 1.0],
        _ => [u[0], u[1], u[2], a],
    }
}

pub fn blend_ch(mode: u32, a: f32, b: f32) -> f32 {
    match mode {
        1 => a + b,
        2 => a * b,
        3 => 1.0 - (1.0 - a) * (1.0 - b),
        4 => {
            if a < 0.5 {
                2.0 * a * b
            } else {
                1.0 - 2.0 * (1.0 - a) * (1.0 - b)
            }
        }
        5 => {
            if b < 0.5 {
                a - (1.0 - 2.0 * b) * a * (1.0 - a)
            } else {
                a + (2.0 * b - 1.0) * (a.sqrt() - a)
            }
        }
        6 => {
            if b < 0.5 {
                2.0 * a * b
            } else {
                1.0 - 2.0 * (1.0 - a) * (1.0 - b)
            }
        }
        7 => a.min(b),
        8 => a.max(b),
        9 => (a - b).abs(),
        10 => a + b - 2.0 * a * b,
        _ => b,
    }
}

fn calculations(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let ic = c.e("inputChannel");
    let inv1 = c.b("invertInput");
    let second = c.other_fit("secondLayer", true, w, h, if c.b("stretchSecond") { 1 } else { 0 });
    let sc = c.e("secondChannel");
    let op2 = c.f("secondOpacity") as f32 / 100.0;
    let inv2 = c.b("invertSecond");
    let mode = c.e("mode");
    let keep = c.b("preserveTransparency");
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let mut x = pick(p, ic);
        if inv1 {
            for k in 0..3 {
                x[k] = 1.0 - x[k];
            }
        }
        let mut out = x;
        if let Some(s) = &second {
            let mut y = pick(&s.px[i * 4..i * 4 + 4], sc);
            if inv2 {
                for k in 0..3 {
                    y[k] = 1.0 - y[k];
                }
            }
            for k in 0..3 {
                let b = blend_ch(mode, x[k], y[k]);
                out[k] = x[k] + (b - x[k]) * op2 * y[3];
            }
            if !keep {
                out[3] = x[3] + (1.0 - x[3]) * y[3] * op2;
            }
        }
        for k in 0..3 {
            p[k] = out[k].clamp(0.0, 1.0) * out[3];
        }
        p[3] = out[3];
    });
}

fn channel_combiner(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let src = if c.b("useSecondLayer") { c.other_fit("sourceLayer", true, w, h, 1) } else { None };
    let from = c.e("from");
    let to = c.e("to");
    let inv = c.b("invert");
    let solid = c.b("solidAlpha");
    let base = img.clone();
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let sp = match &src {
            Some(s) => [s.px[i * 4], s.px[i * 4 + 1], s.px[i * 4 + 2], s.px[i * 4 + 3]],
            None => [base.px[i * 4], base.px[i * 4 + 1], base.px[i * 4 + 2], base.px[i * 4 + 3]],
        };
        let a = p[3];
        let sa = sp[3];
        let su = if sa > 1e-6 { [sp[0] / sa, sp[1] / sa, sp[2] / sa] } else { [0.0; 3] };
        let mut u = if a > 1e-6 { [p[0] / a, p[1] / a, p[2] / a] } else { [0.0; 3] };
        let mut na = if solid { 1.0 } else { a };
        match from {
            0 => {
                let hl = rgb_to_hsl(su);
                u = [hl[0], hl[2], hl[1]];
            }
            1 => u = hsl_to_rgb([su[0], su[2], su[1]]),
            2 => {
                let y = 0.299 * su[0] + 0.587 * su[1] + 0.114 * su[2];
                u = [y, (su[2] - y) * 0.492 + 0.5, (su[0] - y) * 0.877 + 0.5];
            }
            3 => {
                let (y, uu, vv) = (su[0], su[1] - 0.5, su[2] - 0.5);
                u = [y + 1.14 * vv, y - 0.395 * uu - 0.581 * vv, y + 2.032 * uu];
            }
            _ => {
                let v = match from {
                    4 => su[0],
                    5 => su[1],
                    6 => su[2],
                    7 => sa,
                    8 => rgb_to_hsl(su)[2],
                    9 => rgb_to_hsl(su)[0],
                    10 => rgb_to_hsl(su)[1],
                    11 => su[0].min(su[1]).min(su[2]),
                    _ => su[0].max(su[1]).max(su[2]),
                };
                let v = if inv { 1.0 - v } else { v };
                match to {
                    0 => u[0] = v,
                    1 => u[1] = v,
                    2 => u[2] = v,
                    3 => na = v,
                    4 => {
                        let mut hl = rgb_to_hsl(u);
                        hl[2] = v;
                        u = hsl_to_rgb(hl);
                    }
                    5 => {
                        let mut hl = rgb_to_hsl(u);
                        hl[0] = v;
                        u = hsl_to_rgb(hl);
                    }
                    _ => {
                        let mut hl = rgb_to_hsl(u);
                        hl[1] = v;
                        u = hsl_to_rgb(hl);
                    }
                }
            }
        }
        if inv && from < 4 {
            u = u.map(|x| 1.0 - x);
        }
        for k in 0..3 {
            p[k] = u[k].clamp(0.0, 1.0) * na;
        }
        p[3] = na;
    });
}

fn compound_arithmetic(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let Some(o) = c.other_fit("secondSource", true, w, h, if c.b("stretch") { 1 } else { 0 }) else { return };
    let op = c.e("operator");
    let on = c.e("operateOn");
    let of = c.e("overflow");
    let bl = c.f("blend") as f32 / 100.0;
    img.px.par_chunks_mut(4).zip(o.px.par_chunks(4)).for_each(|(p, q)| {
        let a = p[3];
        let b = q[3];
        let u = if a > 1e-6 { [p[0] / a, p[1] / a, p[2] / a, a] } else { [0.0, 0.0, 0.0, a] };
        let v = if b > 1e-6 { [q[0] / b, q[1] / b, q[2] / b, b] } else { [0.0, 0.0, 0.0, b] };
        let f = |x: f32, y: f32| -> f32 {
            let r = match op {
                0 => y,
                1 => x + y,
                2 => x - y,
                3 => x * y,
                4 => (x - y).abs(),
                5 => (((x * 255.0) as u8) & ((y * 255.0) as u8)) as f32 / 255.0,
                6 => (((x * 255.0) as u8) | ((y * 255.0) as u8)) as f32 / 255.0,
                7 => (((x * 255.0) as u8) ^ ((y * 255.0) as u8)) as f32 / 255.0,
                8 | 11 => x.max(y),
                9 | 10 => x.min(y),
                12 => 1.0 - (1.0 - x) * (1.0 - y),
                13 => blend_ch(4, x, y),
                _ => blend_ch(6, x, y),
            };
            match of {
                1 => r.rem_euclid(1.0001),
                2 => r * 0.5,
                _ => r.clamp(0.0, 1.0),
            }
        };
        let mut r = u;
        if on != 2 {
            for k in 0..3 {
                r[k] = f(u[k], v[k]);
            }
        }
        if on >= 1 {
            r[3] = f(u[3], v[3]);
        }
        for k in 0..4 {
            r[k] = r[k] + (u[k] - r[k]) * bl;
        }
        for k in 0..3 {
            p[k] = r[k] * r[3];
        }
        p[3] = r[3];
    });
}

fn minimax(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let r = c.px(c.f("radius")).round() as usize;
    if r == 0 {
        return;
    }
    let op = c.e("operation");
    let ch = c.e("channel");
    let dir = c.e("direction");
    let pass = |data: &[f32], max: bool| -> Vec<f32> {
        match dir {
            1 => {
                let mut out = vec![0.0f32; w * h];
                for y in 0..h {
                    let row = &data[y * w..(y + 1) * w];
                    let t = minmax(row, w, 1, r, max);
                    out[y * w..(y + 1) * w].copy_from_slice(&t);
                }
                out
            }
            2 => {
                let mut out = vec![0.0f32; w * h];
                for x in 0..w {
                    let col: Vec<f32> = (0..h).map(|y| data[y * w + x]).collect();
                    let t = minmax(&col, 1, h, r, max);
                    for y in 0..h {
                        out[y * w + x] = t[y];
                    }
                }
                out
            }
            _ => minmax(data, w, h, r, max),
        }
    };
    let run = |data: &[f32]| -> Vec<f32> {
        match op {
            1 => pass(data, false),
            2 => pass(&pass(data, false), true),
            3 => pass(&pass(data, true), false),
            _ => pass(data, true),
        }
    };
    let chans: Vec<usize> = match ch {
        0 => vec![0, 1, 2],
        1 => vec![0, 1, 2, 3],
        2 => vec![3],
        3 => vec![0],
        4 => vec![1],
        _ => vec![2],
    };
    let unp: Vec<[f32; 4]> = img.px.chunks_exact(4).map(|p| if p[3] > 1e-6 { [p[0] / p[3], p[1] / p[3], p[2] / p[3], p[3]] } else { [0.0, 0.0, 0.0, 0.0] }).collect();
    let mut res = unp.clone();
    for k in chans {
        let plane: Vec<f32> = unp.iter().map(|p| p[k]).collect();
        let o = run(&plane);
        for (i, v) in o.into_iter().enumerate() {
            res[i][k] = v;
        }
    }
    img.px.par_chunks_mut(4).zip(res.par_iter()).for_each(|(p, r)| {
        for k in 0..3 {
            p[k] = r[k] * r[3];
        }
        p[3] = r[3];
    });
}

fn set_channels(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let mode = c.e("ifSizesDiffer");
    let base = img.clone();
    let srcs: Vec<Option<Img>> = ["redLayer", "greenLayer", "blueLayer", "alphaLayer"].iter().map(|k| c.other_fit(k, true, w, h, mode)).collect();
    let froms = [c.e("redFrom"), c.e("greenFrom"), c.e("blueFrom"), c.e("alphaFrom")];
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let mut out = [0.0f32; 4];
        for k in 0..4 {
            let s = match &srcs[k] {
                Some(x) => &x.px[i * 4..i * 4 + 4],
                None => &base.px[i * 4..i * 4 + 4],
            };
            out[k] = chan_of(s, froms[k]);
        }
        for k in 0..3 {
            p[k] = out[k] * out[3];
        }
        p[3] = out[3];
    });
}

fn set_matte(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let src = c.other_fit("takeMatteFrom", true, w, h, if c.b("stretch") { 1 } else { 0 });
    let base = img.clone();
    let from = c.e("useForMatte");
    let inv = c.b("invert");
    let comp = c.b("composite");
    let premul = c.b("premultiply");
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let s = match &src {
            Some(x) => &x.px[i * 4..i * 4 + 4],
            None => &base.px[i * 4..i * 4 + 4],
        };
        let mut m = chan_of(s, from);
        if premul && from != 3 && from < 8 {
            m *= s[3];
        }
        if inv {
            m = 1.0 - m;
        }
        let a = p[3];
        let na = if comp { a * m } else { m };
        if a > 1e-6 {
            let k = na / a;
            p[0] *= k;
            p[1] *= k;
            p[2] *= k;
        }
        p[3] = na;
    });
}

fn shift_channels(c: &FxCtx, img: &mut Img) {
    let f = [c.e("red"), c.e("green"), c.e("blue"), c.e("alpha")];
    if f == [0, 1, 2, 3] {
        return;
    }
    img.px.par_chunks_mut(4).for_each(|p| {
        let s = [p[0], p[1], p[2], p[3]];
        let mut o = [0.0f32; 4];
        for k in 0..4 {
            o[k] = chan_of(&s, f[k]);
        }
        for k in 0..3 {
            p[k] = o[k] * o[3];
        }
        p[3] = o[3];
    });
}

fn cc_composite(c: &FxCtx, img: &mut Img) {
    let Some(orig) = c.host.and_then(|h| h.original()) else { return };
    if orig.w != img.w || orig.h != img.h {
        return;
    }
    let op = c.f("opacity") as f32 / 100.0;
    let mode = c.e("mode");
    let rgb_only = c.b("rgbOnly");
    img.px.par_chunks_mut(4).zip(orig.px.par_chunks(4)).for_each(|(p, o)| {
        let cur = [p[0], p[1], p[2], p[3]];
        let mut out = cur;
        match mode {
            0 => {
                let k = 1.0 - cur[3];
                for i in 0..4 {
                    out[i] = cur[i] + o[i] * op * k;
                }
            }
            1 => {
                let k = 1.0 - o[3] * op;
                for i in 0..4 {
                    out[i] = o[i] * op + cur[i] * k;
                }
            }
            _ => {
                let a = cur[3];
                let b = o[3];
                let u = if a > 1e-6 { [cur[0] / a, cur[1] / a, cur[2] / a] } else { [0.0; 3] };
                let v = if b > 1e-6 { [o[0] / b, o[1] / b, o[2] / b] } else { [0.0; 3] };
                let m = match mode {
                    2 => 1,
                    3 => 2,
                    4 => 3,
                    5 => 7,
                    6 => 8,
                    _ => 9,
                };
                for i in 0..3 {
                    let r = blend_ch(m, u[i], v[i]);
                    out[i] = (u[i] + (r - u[i]) * op * b) * a;
                }
            }
        }
        if rgb_only {
            let a = cur[3];
            let na = out[3].max(1e-6);
            for i in 0..3 {
                out[i] = out[i] / na * a;
            }
            out[3] = a;
        }
        p.copy_from_slice(&out);
    });
}

fn solid_composite(c: &FxCtx, img: &mut Img) {
    let so = c.f("sourceOpacity") as f32 / 100.0;
    let col = c.c("color");
    let op = c.f("opacity") as f32 / 100.0;
    let mode = c.e("mode");
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3] * so;
        let u = if p[3] > 1e-6 { [p[0] / p[3], p[1] / p[3], p[2] / p[3]] } else { [0.0; 3] };
        let mut o = [0.0f32; 4];
        for k in 0..3 {
            let b = match mode {
                1 => blend_ch(1, col[k], u[k]),
                2 => blend_ch(2, col[k], u[k]),
                3 => blend_ch(3, col[k], u[k]),
                4 => blend_ch(4, col[k], u[k]),
                5 => blend_ch(7, col[k], u[k]),
                6 => blend_ch(8, col[k], u[k]),
                7 => blend_ch(9, col[k], u[k]),
                _ => u[k],
            };
            let top = b * a;
            o[k] = top + col[k] * op * (1.0 - a);
        }
        o[3] = a + op * (1.0 - a);
        p.copy_from_slice(&o);
    });
}
