use rayon::prelude::*;

use super::util::{gaussian, lum, minmax, rgb_to_hsl, smooth};
use super::{EffectSpec, FxCtx, check, color, grp, layer_param, mask_param, no_pad, num, pct, point, popup, slider};
use crate::render::image::Img;
use crate::render::vector;

const KEY: &str = "Keying";
const MATTE: &str = "Matte";

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.keying.keylight",
            name: "Keylight",
            category: KEY,
            params: vec![
                popup("view", "View", 10, &VIEWS),
                check("unpremultiply", "Unpremultiply Result", true),
                color("screenColour", "Screen Colour", [0.0, 1.0, 0.0]),
                num("screenGain", "Screen Gain", 100.0, 0.0, 200.0, 0.0, 200.0),
                num("screenBalance", "Screen Balance", 50.0, 0.0, 100.0, 0.0, 100.0),
                num("despillBias", "Despill Bias", 50.0, 0.0, 100.0, 0.0, 100.0),
                slider("screenPreblur", "Screen Pre-blur", 0.0, 0.0, 10.0, 1),
                grp("screenMatte", "Screen Matte"),
                num("screenMatte/clipBlack", "Clip Black", 0.0, 0.0, 100.0, 0.0, 100.0),
                num("screenMatte/clipWhite", "Clip White", 100.0, 0.0, 100.0, 0.0, 100.0),
                num("screenMatte/clipRollback", "Clip Rollback", 0.0, 0.0, 20.0, 0.0, 20.0),
                num("screenMatte/shrinkGrow", "Screen Shrink/Grow", 0.0, -10.0, 10.0, -10.0, 10.0),
                num("screenMatte/softness", "Screen Softness", 0.0, 0.0, 20.0, 0.0, 20.0),
                num("screenMatte/despotBlack", "Screen Despot Black", 0.0, 0.0, 20.0, 0.0, 20.0),
                num("screenMatte/despotWhite", "Screen Despot White", 0.0, 0.0, 20.0, 0.0, 20.0),
                popup("screenMatte/replaceMethod", "Replace Method", 3, &["None", "Source", "Hard Colour", "Soft Colour"]),
                color("screenMatte/replaceColour", "Replace Colour", [0.5, 0.5, 0.5]),
                grp("insideMask", "Inside Mask"),
                mask_param("insideMask/mask", "Inside Mask"),
                num("insideMask/softness", "Inside Mask Softness", 0.0, 0.0, 20.0, 0.0, 20.0),
                check("insideMask/invert", "Invert", false),
                grp("outsideMask", "Outside Mask"),
                mask_param("outsideMask/mask", "Outside Mask"),
                num("outsideMask/softness", "Outside Mask Softness", 0.0, 0.0, 20.0, 0.0, 20.0),
                check("outsideMask/invert", "Invert", false),
            ],
            render: keylight,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.keying.keyCleaner",
            name: "Key Cleaner",
            category: KEY,
            params: vec![
                num("edgeRadius", "Additional Edge Radius", 1.0, 0.0, 30.0, 0.0, 10.0),
                check("reduceChatter", "Reduce Chatter", false),
                pct("alphaContrast", "Alpha Contrast", 100.0),
                pct("strength", "Strength", 100.0),
            ],
            render: key_cleaner,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.keying.advancedSpillSuppressor",
            name: "Advanced Spill Suppressor",
            category: KEY,
            params: vec![
                popup("method", "Method", 0, &["Standard", "Ultra"]),
                check("autoKeyColor", "Key Color Detection", true),
                color("keyColor", "Key Color", [0.0, 1.0, 0.0]),
                pct("suppression", "Suppression", 100.0),
                grp("ultra", "Ultra Settings"),
                pct("ultra/tolerance", "Tolerance", 50.0),
                pct("ultra/desaturate", "Desaturate", 50.0),
                pct("ultra/spillRange", "Spill Range", 0.0),
                pct("ultra/spillColorCorrection", "Spill Color Correction", 50.0),
                pct("ultra/lumaCorrection", "Luma Correction", 50.0),
            ],
            render: adv_spill,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.keying.colorDifferenceKey",
            name: "Color Difference Key",
            category: KEY,
            params: vec![
                popup("view", "View", 0, &["Final Output", "Matte Partial A", "Matte Partial B", "Matte"]),
                color("keyColor", "Key Color", [0.0, 0.0, 1.0]),
                popup("accuracy", "Color Matching Accuracy", 0, &["Faster", "More Accurate"]),
                num("aInBlack", "Partial A In Black", 0.0, 0.0, 255.0, 0.0, 255.0),
                num("aInWhite", "Partial A In White", 255.0, 0.0, 255.0, 0.0, 255.0),
                slider("aGamma", "Partial A Gamma", 1.0, 0.0, 5.0, 2),
                num("bInBlack", "Partial B In Black", 0.0, 0.0, 255.0, 0.0, 255.0),
                num("bInWhite", "Partial B In White", 255.0, 0.0, 255.0, 0.0, 255.0),
                slider("bGamma", "Partial B Gamma", 1.0, 0.0, 5.0, 2),
                num("matteInBlack", "Matte In Black", 0.0, 0.0, 255.0, 0.0, 255.0),
                num("matteInWhite", "Matte In White", 255.0, 0.0, 255.0, 0.0, 255.0),
                slider("matteGamma", "Matte Gamma", 1.0, 0.0, 5.0, 2),
            ],
            render: color_difference_key,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.keying.colorRange",
            name: "Color Range",
            category: KEY,
            params: vec![
                num("fuzziness", "Fuzziness", 0.0, 0.0, 255.0, 0.0, 255.0),
                popup("colorSpace", "Color Space", 0, &["Lab", "YUV", "RGB"]),
                num("minL", "Min (L, Y, R)", 0.0, 0.0, 255.0, 0.0, 255.0),
                num("maxL", "Max (L, Y, R)", 255.0, 0.0, 255.0, 0.0, 255.0),
                num("minA", "Min (a, U, G)", 0.0, 0.0, 255.0, 0.0, 255.0),
                num("maxA", "Max (a, U, G)", 255.0, 0.0, 255.0, 0.0, 255.0),
                num("minB", "Min (b, V, B)", 0.0, 0.0, 255.0, 0.0, 255.0),
                num("maxB", "Max (b, V, B)", 255.0, 0.0, 255.0, 0.0, 255.0),
            ],
            render: color_range,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.keying.differenceMatte",
            name: "Difference Matte",
            category: KEY,
            params: vec![
                popup("view", "View", 0, &["Final Output", "Matte Only"]),
                layer_param("differenceLayer", "Difference Layer"),
                popup("ifSizesDiffer", "If Layer Sizes Differ", 0, &["Center", "Stretch to Fit"]),
                pct("tolerance", "Matching Tolerance", 15.0),
                pct("softness", "Matching Softness", 0.0),
                num("blur", "Blur Before Difference", 0.0, 0.0, 1000.0, 0.0, 20.0),
            ],
            render: difference_matte,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.keying.extract",
            name: "Extract",
            category: KEY,
            params: vec![
                popup("channel", "Channel", 0, &["Luminance", "Red", "Green", "Blue", "Alpha"]),
                num("blackPoint", "Black Point", 0.0, 0.0, 255.0, 0.0, 255.0),
                num("whitePoint", "White Point", 255.0, 0.0, 255.0, 0.0, 255.0),
                num("blackSoftness", "Black Softness", 0.0, 0.0, 255.0, 0.0, 255.0),
                num("whiteSoftness", "White Softness", 0.0, 0.0, 255.0, 0.0, 255.0),
                check("invert", "Invert", false),
            ],
            render: extract,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.keying.innerOuterKey",
            name: "Inner/Outer Key",
            category: KEY,
            params: vec![
                mask_param("foreground", "Foreground (Inside)"),
                mask_param("background", "Background (Outside)"),
                slider("edgeThin", "Edge Thin", 0.0, -100.0, 100.0, 1),
                num("edgeFeather", "Edge Feather", 0.0, 0.0, 100.0, 0.0, 50.0),
                pct("edgeThreshold", "Edge Threshold", 0.0),
                check("invertExtraction", "Invert Extraction", false),
                pct("blend", "Blend with Original", 0.0),
            ],
            render: inner_outer,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.keying.linearColorKey",
            name: "Linear Color Key",
            category: KEY,
            params: vec![
                popup("view", "View", 0, &["Final Output", "Source Only", "Matte Only"]),
                color("keyColor", "Key Color", [0.0, 0.0, 1.0]),
                popup("matchColors", "Match Colors", 0, &["Using RGB", "Using Hue", "Using Chroma"]),
                pct("tolerance", "Matching Tolerance", 10.0),
                pct("softness", "Matching Softness", 10.0),
                popup("operation", "Key Operation", 0, &["Key Colors", "Keep Colors"]),
            ],
            render: linear_color_key,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.keying.unmult",
            name: "Unmult",
            category: KEY,
            params: vec![],
            render: |_, img| {
                img.px.par_chunks_mut(4).for_each(|p| {
                    let a = p[3];
                    if a <= 1e-6 {
                        return;
                    }
                    let c = [p[0] / a, p[1] / a, p[2] / a];
                    let m = c[0].max(c[1]).max(c[2]).clamp(0.0, 1.0);
                    let na = a * m;
                    if m > 1e-6 {
                        for i in 0..3 {
                            p[i] = c[i] / m * na;
                        }
                    } else {
                        p[0] = 0.0;
                        p[1] = 0.0;
                        p[2] = 0.0;
                    }
                    p[3] = na;
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.keying.simpleWireRemoval",
            name: "CC Simple Wire Removal",
            category: KEY,
            params: vec![
                point("pointA", "Point A", 0.3, 0.3),
                point("pointB", "Point B", 0.7, 0.7),
                popup("removalStyle", "Removal Style", 0, &["Fade", "Frame Offset", "Displace", "Displace Horizontal"]),
                num("thickness", "Thickness", 6.0, 0.0, 200.0, 0.0, 50.0),
                slider("slope", "Slope", 50.0, 0.0, 100.0, 1),
                pct("mirrorBlend", "Mirror Blend", 0.0),
            ],
            render: wire_removal,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.matte.matteChoker",
            name: "Matte Choker",
            category: MATTE,
            params: vec![
                num("softness1", "Geometric Softness 1", 4.0, 0.0, 20.0, 0.0, 10.0),
                slider("choke1", "Choke 1", 75.0, -127.0, 127.0, 0),
                pct("graySoftness1", "Gray Level Softness 1", 10.0),
                num("softness2", "Geometric Softness 2", 0.0, 0.0, 20.0, 0.0, 10.0),
                slider("choke2", "Choke 2", 0.0, -127.0, 127.0, 0),
                pct("graySoftness2", "Gray Level Softness 2", 100.0),
                slider("iterations", "Iterations", 1.0, 1.0, 10.0, 0),
            ],
            render: matte_choker,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.matte.simpleChoker",
            name: "Simple Choker",
            category: MATTE,
            params: vec![popup("view", "View", 0, &["Final Output", "Matte"]), slider("choke", "Choke Matte", 0.0, -100.0, 100.0, 2)],
            render: simple_choker,
            pad: |c| (-c.f("choke")).max(0.0),
        },
        EffectSpec {
            id: "fx.matte.refineSoftMatte",
            name: "Refine Soft Matte",
            category: MATTE,
            params: vec![
                num("edgeRadius", "Additional Edge Radius", 10.0, 0.0, 100.0, 0.0, 30.0),
                check("viewEdgeRegion", "View Edge Region", false),
                pct("smooth", "Smooth", 0.0),
                pct("feather", "Feather", 0.0),
                pct("contrast", "Contrast", 0.0),
                slider("shiftEdge", "Shift Edge", 0.0, -100.0, 100.0, 0),
                pct("reduceChatter", "Reduce Chatter", 0.0),
                check("decontaminate", "Decontaminate Edge Colors", true),
                pct("decontaminationAmount", "Decontamination Amount", 100.0),
            ],
            render: |c, img| refine(c, img, true),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.matte.refineHardMatte",
            name: "Refine Hard Matte",
            category: MATTE,
            params: vec![
                num("edgeRadius", "Additional Edge Radius", 2.0, 0.0, 100.0, 0.0, 30.0),
                check("viewEdgeRegion", "View Edge Region", false),
                pct("smooth", "Smooth", 0.0),
                pct("feather", "Feather", 50.0),
                pct("contrast", "Contrast", 50.0),
                slider("shiftEdge", "Shift Edge", 0.0, -100.0, 100.0, 0),
                pct("reduceChatter", "Reduce Chatter", 0.0),
                check("decontaminate", "Decontaminate Edge Colors", true),
                pct("decontaminationAmount", "Decontamination Amount", 100.0),
            ],
            render: |c, img| refine(c, img, false),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.obsolete.colorKey",
            name: "Color Key",
            category: "Obsolete",
            params: vec![color("keyColor", "Key Color", [0.0, 0.0, 1.0]), num("tolerance", "Color Tolerance", 0.0, 0.0, 255.0, 0.0, 255.0), num("edgeThin", "Edge Thin", 0.0, -5.0, 5.0, -5.0, 5.0), num("edgeFeather", "Edge Feather", 0.0, 0.0, 500.0, 0.0, 50.0)],
            render: color_key,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.obsolete.lumaKey",
            name: "Luma Key",
            category: "Obsolete",
            params: vec![
                popup("keyType", "Key Type", 0, &["Key Out Brighter", "Key Out Darker", "Key Out Similar", "Key Out Dissimilar"]),
                num("threshold", "Threshold", 0.0, 0.0, 255.0, 0.0, 255.0),
                num("tolerance", "Tolerance", 0.0, 0.0, 255.0, 0.0, 255.0),
                num("edgeThin", "Edge Thin", 0.0, -5.0, 5.0, -5.0, 5.0),
                num("edgeFeather", "Edge Feather", 0.0, 0.0, 500.0, 0.0, 50.0),
            ],
            render: luma_key,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.obsolete.spillSuppressor",
            name: "Spill Suppressor",
            category: "Obsolete",
            params: vec![color("colorToSuppress", "Color To Suppress", [0.0, 0.0, 1.0]), popup("accuracy", "Color Accuracy", 0, &["Faster", "Better"]), pct("suppression", "Suppression", 100.0)],
            render: |c, img| {
                let k = c.c("colorToSuppress");
                let s = c.f("suppression") as f32 / 100.0;
                spill(img, [k[0], k[1], k[2]], s, 0.0);
            },
            pad: no_pad,
        },
    ]
}

const VIEWS: [&str; 11] = [
    "Source",
    "Source Alpha",
    "Corrected Source",
    "Colour Correction Edges",
    "Screen Matte",
    "Inside Mask",
    "Outside Mask",
    "Combined Matte",
    "Status",
    "Intermediate Result",
    "Final Result",
];

fn dominant(k: [f32; 3]) -> (usize, usize, usize) {
    let d = if k[1] >= k[0] && k[1] >= k[2] {
        1
    } else if k[2] >= k[0] {
        2
    } else {
        0
    };
    let o = [(1, 2), (0, 2), (0, 1)][d];
    (d, o.0, o.1)
}

fn screen_diff(p: [f32; 3], d: (usize, usize, usize), bal: f32) -> f32 {
    p[d.0] - (p[d.1] * bal + p[d.2] * (1.0 - bal))
}

pub fn mask_cov(c: &FxCtx, index: usize, w: usize, h: usize, soft: f64, invert: bool) -> Option<Vec<f32>> {
    let path = c.mask(index)?;
    let mut cov = vector::coverage(std::slice::from_ref(&path), &crate::geom::Mat3::IDENTITY, w, h, 0, 0, false, 0.0);
    if soft > 0.3 {
        super::util::blur_scalar(&mut cov, w, h, c.px(soft) * 0.5, c.px(soft) * 0.5);
    }
    if invert {
        cov.iter_mut().for_each(|v| *v = 1.0 - *v);
    }
    Some(cov)
}

fn keylight(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let sc = c.c("screenColour");
    let sc = [sc[0], sc[1], sc[2]];
    let d = dominant(sc);
    let bal = c.f("screenBalance") as f32 / 100.0;
    let gain = c.f("screenGain") as f32 / 100.0;
    let sd = screen_diff(sc, d, bal).max(1e-4);
    let mut src = img.clone();
    let pre = c.px(c.f("screenPreblur"));
    if pre > 0.3 {
        gaussian(&mut src, pre * 0.5, pre * 0.5, true);
    }
    let mut matte: Vec<f32> = src
        .px
        .par_chunks(4)
        .map(|p| {
            if p[3] <= 1e-6 {
                return 0.0;
            }
            let u = [p[0] / p[3], p[1] / p[3], p[2] / p[3]];
            (1.0 - gain * screen_diff(u, d, bal) / sd).clamp(0.0, 1.0)
        })
        .collect();
    let screen_only = matte.clone();
    let (cb, cw) = (c.f("screenMatte/clipBlack") as f32 / 100.0, c.f("screenMatte/clipWhite") as f32 / 100.0);
    let rb = c.f("screenMatte/clipRollback");
    let clipped: Vec<f32> = matte.iter().map(|m| ((m - cb) / (cw - cb).max(1e-4)).clamp(0.0, 1.0)).collect();
    if rb > 0.0 {
        let grown = minmax(&clipped, w, h, c.px(rb).round() as usize, true);
        matte = clipped.iter().zip(&grown).zip(&matte).map(|((a, g), m)| if *g > 0.0 && *a < 1.0 { a.max(*m * g) } else { *a }).collect();
    } else {
        matte = clipped;
    }
    for (k, white) in [("screenMatte/despotBlack", false), ("screenMatte/despotWhite", true)] {
        let r = c.px(c.f(k)).round() as usize;
        if r > 0 {
            let (a, b) = if white { (false, true) } else { (true, false) };
            matte = minmax(&minmax(&matte, w, h, r, a), w, h, r, b);
        }
    }
    let sg = c.f("screenMatte/shrinkGrow");
    if sg.abs() > 0.05 {
        matte = minmax(&matte, w, h, c.px(sg.abs()).round().max(1.0) as usize, sg > 0.0);
    }
    let soft = c.px(c.f("screenMatte/softness"));
    if soft > 0.3 {
        super::util::blur_scalar(&mut matte, w, h, soft * 0.5, soft * 0.5);
    }
    let inside = mask_cov(c, c.f("insideMask/mask") as usize, w, h, c.f("insideMask/softness"), c.b("insideMask/invert"));
    let outside = mask_cov(c, c.f("outsideMask/mask") as usize, w, h, c.f("outsideMask/softness"), c.b("outsideMask/invert"));
    let combined: Vec<f32> = (0..w * h)
        .map(|i| {
            let mut m = matte[i];
            if let Some(ins) = &inside {
                m = m.max(ins[i]);
            }
            if let Some(out) = &outside {
                m *= out[i];
            }
            m
        })
        .collect();
    let view = c.e("view");
    let bias = c.f("despillBias") as f32 / 100.0;
    let replace = c.e("screenMatte/replaceMethod");
    let rc = c.c("screenMatte/replaceColour");
    let unp = c.b("unpremultiply");
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let a = p[3];
        let u = if a > 1e-6 { [p[0] / a, p[1] / a, p[2] / a] } else { [0.0; 3] };
        let mut cs = u;
        let lim = cs[d.1] * bias + cs[d.2] * (1.0 - bias);
        if cs[d.0] > lim {
            cs[d.0] = lim;
        }
        let m = combined[i];
        match view {
            0 => {}
            1 => {
                p[0] = a;
                p[1] = a;
                p[2] = a;
                p[3] = 1.0;
            }
            2 => {
                for k in 0..3 {
                    p[k] = cs[k] * a;
                }
            }
            3 | 4 | 7 => {
                let v = match view {
                    4 => screen_only[i],
                    3 => (screen_only[i] - matte[i]).abs(),
                    _ => m,
                };
                p[0] = v;
                p[1] = v;
                p[2] = v;
                p[3] = 1.0;
            }
            5 | 6 => {
                let v = if view == 5 { inside.as_ref().map(|x| x[i]).unwrap_or(0.0) } else { outside.as_ref().map(|x| x[i]).unwrap_or(1.0) };
                p[0] = v;
                p[1] = v;
                p[2] = v;
                p[3] = 1.0;
            }
            8 => {
                let v = if m <= 0.001 {
                    0.0
                } else if m >= 0.999 {
                    1.0
                } else {
                    0.5
                };
                p[0] = v;
                p[1] = v;
                p[2] = v;
                p[3] = 1.0;
            }
            _ => {
                let na = a * m;
                let mut col = cs;
                if m < 0.999 && m > 0.001 {
                    match replace {
                        1 => col = u,
                        2 => col = [rc[0], rc[1], rc[2]],
                        3 => {
                            let t = 1.0 - m;
                            for k in 0..3 {
                                col[k] = col[k] + (rc[k] * lum(cs[0], cs[1], cs[2]) * 2.0 - col[k]) * t * 0.25;
                            }
                        }
                        _ => {}
                    }
                }
                let k = if unp || view == 9 { na } else { a };
                for j in 0..3 {
                    p[j] = col[j] * k;
                }
                p[3] = if view == 9 { a } else { na };
            }
        }
    });
}

fn key_cleaner(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let a: Vec<f32> = img.px.chunks_exact(4).map(|p| p[3]).collect();
    let r = c.px(c.f("edgeRadius")).max(0.0);
    let strength = c.f("strength") as f32 / 100.0;
    let contrast = c.f("alphaContrast") as f32 / 100.0;
    let mut soft = a.clone();
    if r > 0.3 {
        super::util::blur_scalar(&mut soft, w, h, r * 0.5, r * 0.5);
    }
    if c.b("reduceChatter") {
        let mut s2 = soft.clone();
        super::util::blur_scalar(&mut s2, w, h, 1.5, 1.5);
        soft = s2;
    }
    let out: Vec<f32> = a
        .iter()
        .zip(&soft)
        .map(|(o, s)| {
            let edge = *o > 0.0 && *o < 1.0 || (*s > 0.02 && *s < 0.98);
            let mut v = if edge { o + (s - o) * strength } else { *o };
            v = ((v - 0.5) * contrast.max(0.01) + 0.5).clamp(0.0, 1.0);
            if contrast >= 1.0 { v } else { o + (v - o) * strength }
        })
        .collect();
    set_alpha(img, &out);
}

fn set_alpha(img: &mut Img, a: &[f32]) {
    img.px.par_chunks_mut(4).zip(a.par_iter()).for_each(|(p, na)| {
        let o = p[3];
        if o > 1e-6 {
            let k = na / o;
            p[0] *= k;
            p[1] *= k;
            p[2] *= k;
        } else {
            p[0] = 0.0;
            p[1] = 0.0;
            p[2] = 0.0;
        }
        p[3] = *na;
    });
}

fn multiply_alpha(img: &mut Img, k: &[f32]) {
    img.px.par_chunks_mut(4).zip(k.par_iter()).for_each(|(p, m)| {
        for v in p.iter_mut() {
            *v *= *m;
        }
    });
}

pub fn spill(img: &mut Img, key: [f32; 3], amount: f32, tolerance: f32) {
    let d = dominant(key);
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let mut u = [p[0] / a, p[1] / a, p[2] / a];
        let lim = (u[d.1] + u[d.2]) * 0.5 + tolerance * (u[d.1].max(u[d.2]) - (u[d.1] + u[d.2]) * 0.5);
        if u[d.0] > lim {
            let excess = u[d.0] - lim;
            u[d.0] -= excess * amount;
        }
        for i in 0..3 {
            p[i] = u[i] * a;
        }
    });
}

fn adv_spill(c: &FxCtx, img: &mut Img) {
    let mut key = c.c("keyColor");
    if c.b("autoKeyColor") {
        let mut acc = [0.0f64; 3];
        let mut n = 0.0;
        for p in img.px.chunks_exact(4).step_by(7) {
            if p[3] > 0.5 {
                let u = [p[0] / p[3], p[1] / p[3], p[2] / p[3]];
                let hsl = rgb_to_hsl(u);
                if hsl[1] > 0.3 {
                    for k in 0..3 {
                        acc[k] += u[k] as f64;
                    }
                    n += 1.0;
                }
            }
        }
        if n > 0.0 {
            let m = [acc[0] / n, acc[1] / n, acc[2] / n];
            let d = dominant([m[0] as f32, m[1] as f32, m[2] as f32]).0;
            key = [0.0, 0.0, 0.0, 1.0];
            key[d] = 1.0;
        }
    }
    let s = c.f("suppression") as f32 / 100.0;
    if c.e("method") == 0 {
        spill(img, [key[0], key[1], key[2]], s, 0.0);
        return;
    }
    let tol = c.f("ultra/tolerance") as f32 / 100.0;
    let desat = c.f("ultra/desaturate") as f32 / 100.0;
    let range = c.f("ultra/spillRange") as f32 / 100.0;
    let luma = c.f("ultra/lumaCorrection") as f32 / 100.0;
    let d = dominant([key[0], key[1], key[2]]);
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let mut u = [p[0] / a, p[1] / a, p[2] / a];
        let l0 = lum(u[0], u[1], u[2]);
        let lim = (u[d.1] + u[d.2]) * 0.5 * (1.0 - range) + u[d.1].max(u[d.2]) * range + tol * 0.1;
        if u[d.0] > lim {
            let ex = (u[d.0] - lim) * s;
            u[d.0] -= ex;
            let g = lum(u[0], u[1], u[2]);
            for k in 0..3 {
                u[k] += (g - u[k]) * desat * (ex * 2.0).min(1.0);
            }
            let dl = (l0 - lum(u[0], u[1], u[2])) * luma;
            for k in 0..3 {
                u[k] += dl;
            }
        }
        for i in 0..3 {
            p[i] = u[i].max(0.0) * a;
        }
    });
}

fn ycc(c: [f32; 3]) -> [f32; 3] {
    let y = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
    [y, (c[2] - y) * 0.564, (c[0] - y) * 0.713]
}

fn levels01(v: f32, b: f32, w: f32, g: f32) -> f32 {
    let x = ((v - b) / (w - b).max(1e-4)).clamp(0.0, 1.0);
    if (g - 1.0).abs() < 1e-4 { x } else { x.powf(1.0 / g.max(0.01)) }
}

fn color_difference_key(c: &FxCtx, img: &mut Img) {
    let k = c.c("keyColor");
    let kc = ycc([k[0], k[1], k[2]]);
    let kn = (kc[1] * kc[1] + kc[2] * kc[2]).sqrt().max(1e-4);
    let d = dominant([k[0], k[1], k[2]]);
    let g = |n: &str| c.f(n) as f32 / 255.0;
    let (ab, aw, ag) = (g("aInBlack"), g("aInWhite"), c.f("aGamma") as f32);
    let (bb, bw, bg) = (g("bInBlack"), g("bInWhite"), c.f("bGamma") as f32);
    let (mb, mw, mg) = (g("matteInBlack"), g("matteInWhite"), c.f("matteGamma") as f32);
    let view = c.e("view");
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let u = [p[0] / a, p[1] / a, p[2] / a];
        let yc = ycc(u);
        let proj = (yc[1] * kc[1] + yc[2] * kc[2]) / kn;
        let pa = levels01((1.0 - proj / kn).clamp(0.0, 1.0), ab, aw, ag);
        let diff = (u[d.0] - u[d.1].max(u[d.2])) / (k[d.0] - k[d.1].max(k[d.2])).max(1e-3);
        let pb = levels01((1.0 - diff).clamp(0.0, 1.0), bb, bw, bg);
        let m = levels01(pa.max(pb), mb, mw, mg);
        let v = match view {
            1 => Some(pa),
            2 => Some(pb),
            3 => Some(m),
            _ => None,
        };
        match v {
            Some(v) => {
                p[0] = v;
                p[1] = v;
                p[2] = v;
                p[3] = 1.0;
            }
            None => {
                let na = a * m;
                for i in 0..3 {
                    p[i] = u[i] * na;
                }
                p[3] = na;
            }
        }
    });
}

pub fn to_lab(c: [f32; 3]) -> [f32; 3] {
    let lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let (r, g, b) = (lin(c[0]), lin(c[1]), lin(c[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn color_range(c: &FxCtx, img: &mut Img) {
    let space = c.e("colorSpace");
    let fz = c.f("fuzziness") as f32;
    let lo = [c.f("minL") as f32, c.f("minA") as f32, c.f("minB") as f32];
    let hi = [c.f("maxL") as f32, c.f("maxA") as f32, c.f("maxB") as f32];
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let u = [p[0] / a, p[1] / a, p[2] / a];
        let v = match space {
            0 => {
                let l = to_lab(u);
                [l[0] * 2.55, l[1] + 128.0, l[2] + 128.0]
            }
            1 => {
                let y = ycc(u);
                [y[0] * 255.0, y[1] * 255.0 + 128.0, y[2] * 255.0 + 128.0]
            }
            _ => [u[0] * 255.0, u[1] * 255.0, u[2] * 255.0],
        };
        let mut out = 0.0f32;
        for k in 0..3 {
            let d = (lo[k] - v[k]).max(v[k] - hi[k]).max(0.0);
            out = out.max(d);
        }
        let keep = if fz <= 0.0 { if out > 0.0 { 1.0 } else { 0.0 } } else { (out / fz).clamp(0.0, 1.0) };
        for x in p.iter_mut() {
            *x *= keep;
        }
    });
}

fn difference_matte(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let Some(mut other) = c.other_fit("differenceLayer", true, w, h, c.e("ifSizesDiffer")) else { return };
    let mut me = img.clone();
    let b = c.px(c.f("blur"));
    if b > 0.3 {
        gaussian(&mut me, b * 0.5, b * 0.5, true);
        gaussian(&mut other, b * 0.5, b * 0.5, true);
    }
    let tol = c.f("tolerance") as f32 / 100.0;
    let soft = c.f("softness") as f32 / 100.0;
    let matte_only = c.e("view") == 1;
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let a = &me.px[i * 4..i * 4 + 4];
        let o = &other.px[i * 4..i * 4 + 4];
        let d = ((a[0] - o[0]).abs() + (a[1] - o[1]).abs() + (a[2] - o[2]).abs()) / 3.0;
        let m = if soft <= 1e-4 { if d > tol { 1.0 } else { 0.0 } } else { smooth(tol, tol + soft, d) };
        if matte_only {
            p[0] = m;
            p[1] = m;
            p[2] = m;
            p[3] = 1.0;
        } else {
            for x in p.iter_mut() {
                *x *= m;
            }
        }
    });
}

fn extract(c: &FxCtx, img: &mut Img) {
    let ch = c.e("channel");
    let g = |n: &str| c.f(n) as f32 / 255.0;
    let (bp, wp, bs, ws) = (g("blackPoint"), g("whitePoint"), g("blackSoftness"), g("whiteSoftness"));
    let inv = c.b("invert");
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
        let lo = if bs > 0.0 { smooth(bp - bs, bp, v) } else if v >= bp { 1.0 } else { 0.0 };
        let hi = if ws > 0.0 { 1.0 - smooth(wp, wp + ws, v) } else if v <= wp { 1.0 } else { 0.0 };
        let mut m = lo * hi;
        if inv {
            m = 1.0 - m;
        }
        for x in p.iter_mut() {
            *x *= m;
        }
    });
}

fn inner_outer(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let Some(inside) = mask_cov(c, c.f("foreground") as usize, w, h, 0.0, false) else { return };
    let outside = mask_cov(c, c.f("background") as usize, w, h, 0.0, false).unwrap_or_else(|| {
        let mut o = minmax(&inside, w, h, (c.px(10.0)).round().max(2.0) as usize, true);
        o.iter_mut().for_each(|v| *v = v.min(1.0));
        o
    });
    let orig = img.clone();
    let col = |mask: &dyn Fn(usize) -> f32| -> Img {
        let mut acc = Img::new(w, h);
        for i in 0..w * h {
            let m = mask(i);
            for k in 0..4 {
                acc.px[i * 4 + k] = orig.px[i * 4 + k] * m;
            }
        }
        let r = (w.max(h) as f64 * 0.02).max(4.0);
        gaussian(&mut acc, r, r, true);
        acc
    };
    let fg = col(&|i| inside[i]);
    let bg = col(&|i| 1.0 - outside[i]);
    let thin = c.f("edgeThin") as f32 / 100.0;
    let th = c.f("edgeThreshold") as f32 / 100.0;
    let inv = c.b("invertExtraction");
    let mut alpha: Vec<f32> = (0..w * h)
        .map(|i| {
            if inside[i] >= 0.999 {
                return 1.0;
            }
            if outside[i] <= 0.001 {
                return 0.0;
            }
            let f = &fg.px[i * 4..i * 4 + 4];
            let b = &bg.px[i * 4..i * 4 + 4];
            let fu = if f[3] > 1e-4 { [f[0] / f[3], f[1] / f[3], f[2] / f[3]] } else { [0.0; 3] };
            let bu = if b[3] > 1e-4 { [b[0] / b[3], b[1] / b[3], b[2] / b[3]] } else { [0.0; 3] };
            let p = &orig.px[i * 4..i * 4 + 4];
            let pu = if p[3] > 1e-4 { [p[0] / p[3], p[1] / p[3], p[2] / p[3]] } else { [0.0; 3] };
            let fb = [fu[0] - bu[0], fu[1] - bu[1], fu[2] - bu[2]];
            let n = fb[0] * fb[0] + fb[1] * fb[1] + fb[2] * fb[2];
            let mut a = if n < 1e-5 { 0.5 } else { ((pu[0] - bu[0]) * fb[0] + (pu[1] - bu[1]) * fb[1] + (pu[2] - bu[2]) * fb[2]) / n };
            a = (a - thin * 0.5).clamp(0.0, 1.0);
            if a < th {
                a = 0.0;
            }
            if inv { 1.0 - a } else { a }
        })
        .collect();
    let fe = c.px(c.f("edgeFeather"));
    if fe > 0.3 {
        super::util::blur_scalar(&mut alpha, w, h, fe * 0.5, fe * 0.5);
    }
    let blend = c.f("blend") as f32 / 100.0;
    let k: Vec<f32> = alpha.iter().map(|a| a + (1.0 - a) * blend).collect();
    multiply_alpha(img, &k);
}

fn linear_color_key(c: &FxCtx, img: &mut Img) {
    let k = c.c("keyColor");
    let k = [k[0], k[1], k[2]];
    let mode = c.e("matchColors");
    let tol = c.f("tolerance") as f32 / 100.0;
    let soft = c.f("softness") as f32 / 100.0;
    let keep = c.e("operation") == 1;
    let view = c.e("view");
    let kh = rgb_to_hsl(k);
    let ky = ycc(k);
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let u = [p[0] / a, p[1] / a, p[2] / a];
        let d = match mode {
            1 => {
                let hh = rgb_to_hsl(u);
                let dh = (hh[0] - kh[0]).abs();
                dh.min(1.0 - dh) * 2.0
            }
            2 => {
                let y = ycc(u);
                ((y[1] - ky[1]).powi(2) + (y[2] - ky[2]).powi(2)).sqrt() * 1.6
            }
            _ => ((u[0] - k[0]).powi(2) + (u[1] - k[1]).powi(2) + (u[2] - k[2]).powi(2)).sqrt() / 3f32.sqrt(),
        };
        let mut m = if d <= tol { 0.0 } else if soft <= 1e-5 { 1.0 } else { ((d - tol) / soft).clamp(0.0, 1.0) };
        if keep {
            m = 1.0 - m;
        }
        match view {
            1 => {}
            2 => {
                p[0] = m;
                p[1] = m;
                p[2] = m;
                p[3] = 1.0;
            }
            _ => {
                for x in p.iter_mut() {
                    *x *= m;
                }
            }
        }
    });
}

fn wire_removal(c: &FxCtx, img: &mut Img) {
    let a = c.pt("pointA");
    let b = c.pt("pointB");
    let th = c.px(c.f("thickness")).max(0.5);
    let slope = c.f("slope") as f32 / 100.0;
    let mirror = c.f("mirrorBlend") as f32 / 100.0;
    let style = c.e("removalStyle");
    let prev = if style == 1 { c.frame(-1.0 / c.fps.max(1.0)) } else { None };
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dy).max(1e-6);
    let (nx, ny) = (-dy / len, dx / len);
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let t = ((px - a[0]) * dx + (py - a[1]) * dy) / (len * len);
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let dist = (px - a[0]) * nx + (py - a[1]) * ny;
            let half = th * 0.5;
            if dist.abs() > half * (1.0 + slope as f64) {
                continue;
            }
            let core = (1.0 - ((dist.abs() - half) / (half * slope as f64 + 1e-6)).clamp(0.0, 1.0)) as f32;
            let fill: [f32; 4] = match (style, &prev) {
                (1, Some(pf)) => pf.sample(px, py, true),
                (2, _) | (3, _) => {
                    let off = if style == 3 { [half + 1.0, 0.0] } else { [nx * (half + 1.0), ny * (half + 1.0)] };
                    let s = if dist >= 0.0 { 1.0 } else { -1.0 };
                    src.sample(px + off[0] * s, py + off[1] * s, true)
                }
                _ => {
                    let s1 = src.sample(px + nx * (half + 1.0 - dist), py + ny * (half + 1.0 - dist), true);
                    let s2 = src.sample(px - nx * (half + 1.0 + dist), py - ny * (half + 1.0 + dist), true);
                    let f = ((dist + half) / (2.0 * half)).clamp(0.0, 1.0) as f32;
                    let m = [s2[0] + (s1[0] - s2[0]) * f, s2[1] + (s1[1] - s2[1]) * f, s2[2] + (s1[2] - s2[2]) * f, s2[3] + (s1[3] - s2[3]) * f];
                    if mirror > 0.0 {
                        let mm = src.sample(px - nx * 2.0 * dist, py - ny * 2.0 * dist, true);
                        [m[0] + (mm[0] - m[0]) * mirror, m[1] + (mm[1] - m[1]) * mirror, m[2] + (mm[2] - m[2]) * mirror, m[3] + (mm[3] - m[3]) * mirror]
                    } else {
                        m
                    }
                }
            };
            for k in 0..4 {
                row[x * 4 + k] += (fill[k] - row[x * 4 + k]) * core;
            }
        }
    });
}

fn choke_pass(a: &[f32], w: usize, h: usize, soft: f64, choke: f32, gray: f32) -> Vec<f32> {
    let mut b = a.to_vec();
    if soft > 0.3 {
        super::util::blur_scalar(&mut b, w, h, soft * 0.5, soft * 0.5);
    }
    let center = 0.5 + choke / 255.0;
    let width = (gray.max(0.01)) * 0.5;
    b.iter().map(|v| smooth(center - width, center + width, *v)).collect()
}

fn matte_choker(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let mut a: Vec<f32> = img.px.chunks_exact(4).map(|p| p[3]).collect();
    let n = c.f("iterations").round().clamp(1.0, 10.0) as usize;
    for _ in 0..n {
        a = choke_pass(&a, w, h, c.px(c.f("softness1")), c.f("choke1") as f32, c.f("graySoftness1") as f32 / 100.0);
        if c.f("softness2") > 0.0 || c.f("choke2") != 0.0 {
            a = choke_pass(&a, w, h, c.px(c.f("softness2")), c.f("choke2") as f32, c.f("graySoftness2") as f32 / 100.0);
        }
    }
    set_alpha(img, &a);
}

fn simple_choker(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let a: Vec<f32> = img.px.chunks_exact(4).map(|p| p[3]).collect();
    let ch = c.px(c.f("choke"));
    let r = ch.abs();
    let out = if r < 0.01 {
        a
    } else {
        let whole = r.floor() as usize;
        let frac = (r - whole as f64) as f32;
        let grow = ch < 0.0;
        let base = if whole > 0 { minmax(&a, w, h, whole, grow) } else { a.clone() };
        let next = minmax(&base, w, h, 1, grow);
        base.iter().zip(&next).map(|(x, y)| x + (y - x) * frac).collect()
    };
    if c.e("view") == 1 {
        img.px.par_chunks_mut(4).zip(out.par_iter()).for_each(|(p, v)| {
            p[0] = *v;
            p[1] = *v;
            p[2] = *v;
            p[3] = 1.0;
        });
        return;
    }
    set_alpha(img, &out);
}

fn refine(c: &FxCtx, img: &mut Img, soft_mode: bool) {
    let (w, h) = (img.w, img.h);
    let a: Vec<f32> = img.px.chunks_exact(4).map(|p| p[3]).collect();
    let r = c.px(c.f("edgeRadius")).max(1.0);
    let hard: Vec<f32> = a.iter().map(|v| if *v >= 0.5 { 1.0 } else { 0.0 }).collect();
    let grown = minmax(&hard, w, h, r.round() as usize, true);
    let shrunk = minmax(&hard, w, h, r.round() as usize, false);
    let region: Vec<f32> = grown.iter().zip(&shrunk).map(|(g, s)| g - s).collect();
    if c.b("viewEdgeRegion") {
        img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
            let v = if region[i] > 0.0 { 0.5 } else { a[i] };
            p[0] = v;
            p[1] = v;
            p[2] = v;
            p[3] = 1.0;
        });
        return;
    }
    let lum_img: Vec<f32> = img.px.chunks_exact(4).map(|p| if p[3] > 1e-6 { lum(p[0] / p[3], p[1] / p[3], p[2] / p[3]) } else { 0.0 }).collect();
    let mut guide = a.clone();
    let sm = c.f("smooth") as f64 / 100.0;
    let s = (r * (0.25 + sm)).max(0.6);
    super::util::blur_scalar(&mut guide, w, h, s, s);
    let mut lb = lum_img.clone();
    super::util::blur_scalar(&mut lb, w, h, s, s);
    let fe = c.f("feather") as f32 / 100.0;
    let con = c.f("contrast") as f32 / 100.0;
    let shift = c.f("shiftEdge") as f32 / 100.0;
    let mut out: Vec<f32> = (0..w * h)
        .map(|i| {
            if region[i] <= 0.0 {
                return a[i];
            }
            let detail = if soft_mode { (lum_img[i] - lb[i]) * 2.0 } else { 0.0 };
            let mut v = guide[i] + detail * (1.0 - fe);
            v += shift * 0.5;
            if con > 0.0 {
                v = ((v - 0.5) * (1.0 + con * 8.0) + 0.5).clamp(0.0, 1.0);
            }
            v.clamp(0.0, 1.0)
        })
        .collect();
    let chatter = c.f("reduceChatter") as f64 / 100.0;
    if chatter > 0.0 {
        let mut b = out.clone();
        super::util::blur_scalar(&mut b, w, h, 1.0 + chatter * 2.0, 1.0 + chatter * 2.0);
        out = out.iter().zip(&b).zip(&region).map(|((o, b), r)| if *r > 0.0 { o + (b - o) * chatter as f32 } else { *o }).collect();
    }
    if c.b("decontaminate") {
        let amt = c.f("decontaminationAmount") as f32 / 100.0;
        let mut fgc = img.clone();
        let core: Vec<f32> = out.iter().map(|v| if *v > 0.95 { 1.0 } else { 0.0 }).collect();
        multiply_alpha(&mut fgc, &core);
        gaussian(&mut fgc, r, r, true);
        img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
            if region[i] <= 0.0 || p[3] <= 1e-6 {
                return;
            }
            let f = &fgc.px[i * 4..i * 4 + 4];
            if f[3] <= 1e-4 {
                return;
            }
            for k in 0..3 {
                let u = p[k] / p[3];
                let target = f[k] / f[3];
                p[k] = (u + (target - u) * amt * (1.0 - out[i])) * p[3];
            }
        });
    }
    set_alpha(img, &out);
}

fn color_key(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let k = c.c("keyColor");
    let tol = c.f("tolerance") as f32 / 255.0;
    let mut m: Vec<f32> = img
        .px
        .chunks_exact(4)
        .map(|p| {
            if p[3] <= 1e-6 {
                return 0.0;
            }
            let d = ((p[0] / p[3] - k[0]).abs()).max((p[1] / p[3] - k[1]).abs()).max((p[2] / p[3] - k[2]).abs());
            if d <= tol { 0.0 } else { 1.0 }
        })
        .collect();
    edge_ops(c, &mut m, w, h);
    multiply_alpha(img, &m);
}

fn edge_ops(c: &FxCtx, m: &mut Vec<f32>, w: usize, h: usize) {
    let thin = c.f("edgeThin");
    if thin.abs() >= 0.5 {
        *m = minmax(m, w, h, c.px(thin.abs()).round().max(1.0) as usize, thin < 0.0);
    }
    let fe = c.px(c.f("edgeFeather"));
    if fe > 0.3 {
        super::util::blur_scalar(m, w, h, fe * 0.5, fe * 0.5);
    }
}

fn luma_key(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let th = c.f("threshold") as f32 / 255.0;
    let tol = c.f("tolerance") as f32 / 255.0;
    let kt = c.e("keyType");
    let mut m: Vec<f32> = img
        .px
        .chunks_exact(4)
        .map(|p| {
            if p[3] <= 1e-6 {
                return 0.0;
            }
            let l = lum(p[0] / p[3], p[1] / p[3], p[2] / p[3]);
            let out = match kt {
                0 => l > th,
                1 => l < th,
                2 => (l - th).abs() <= tol,
                _ => (l - th).abs() > tol,
            };
            if out { 0.0 } else { 1.0 }
        })
        .collect();
    edge_ops(c, &mut m, w, h);
    multiply_alpha(img, &m);
}
