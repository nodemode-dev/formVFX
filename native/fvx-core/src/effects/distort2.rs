use std::f64::consts::{FRAC_PI_2, PI, TAU};

use rayon::prelude::*;

use super::util::{grid_points, gaussian, lum, mesh_render, remap, smooth, unsharp_into};
use super::{EffectSpec, FxCtx, angle, check, color, hidden, layer_param, mask_param, no_pad, num, pct, point, popup, slider, v2};
use crate::geom::homography;
use crate::model::BezPath;
use crate::render::image::Img;

const CAT: &str = "Distort";

const DISP_CH: [&str; 11] = ["Red", "Green", "Blue", "Alpha", "Luminance", "Hue", "Lightness", "Saturation", "Full", "Half", "Off"];

const WARP_STYLES: [&str; 15] = ["Arc", "Arc Lower", "Arc Upper", "Arch", "Bulge", "Shell Lower", "Shell Upper", "Flag", "Wave", "Fish", "Rise", "Fisheye", "Inflate", "Squeeze", "Twist"];

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.distort.bezierWarp",
            name: "Bezier Warp",
            category: CAT,
            params: vec![
                point("topLeft", "Top Left Vertex", 0.0, 0.0),
                point("topLeftTangent", "Top Left Tangent", 1.0 / 3.0, 0.0),
                point("topRightTangent", "Top Right Tangent", 2.0 / 3.0, 0.0),
                point("topRight", "Right Top Vertex", 1.0, 0.0),
                point("rightTopTangent", "Right Top Tangent", 1.0, 1.0 / 3.0),
                point("rightBottomTangent", "Right Bottom Tangent", 1.0, 2.0 / 3.0),
                point("bottomRight", "Bottom Right Vertex", 1.0, 1.0),
                point("bottomRightTangent", "Bottom Right Tangent", 2.0 / 3.0, 1.0),
                point("bottomLeftTangent", "Bottom Left Tangent", 1.0 / 3.0, 1.0),
                point("bottomLeft", "Left Bottom Vertex", 0.0, 1.0),
                point("leftBottomTangent", "Left Bottom Tangent", 0.0, 2.0 / 3.0),
                point("leftTopTangent", "Left Top Tangent", 0.0, 1.0 / 3.0),
                slider("quality", "Quality", 8.0, 1.0, 10.0, 0),
            ],
            render: bezier_warp,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.meshWarp",
            name: "Mesh Warp",
            category: CAT,
            params: vec![slider("rows", "Rows", 7.0, 1.0, 31.0, 0), slider("columns", "Columns", 7.0, 1.0, 31.0, 0), slider("quality", "Quality", 8.0, 1.0, 10.0, 0), hidden("mesh", "Distortion Mesh", "")],
            render: mesh_warp,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.liquify",
            name: "Liquify",
            category: CAT,
            params: vec![
                popup("tool", "Tool", 0, &["Warp", "Turbulence", "Twirl Clockwise", "Twirl Counterclockwise", "Pucker", "Bloat", "Shift Pixels", "Reflection", "Clone", "Reconstruction"]),
                num("brushSize", "Brush Size", 64.0, 1.0, 600.0, 1.0, 600.0),
                num("brushPressure", "Brush Pressure", 50.0, 1.0, 100.0, 1.0, 100.0),
                check("viewMesh", "View Mesh", false),
                hidden("mesh", "Distortion Mesh", ""),
                v2("meshOffset", "Distortion Mesh Offset", [0.0, 0.0]),
                pct("percentage", "Distortion Percentage", 100.0),
            ],
            render: liquify,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.displacementMap",
            name: "Displacement Map",
            category: CAT,
            params: vec![
                layer_param("mapLayer", "Displacement Map Layer"),
                popup("horizontal", "Use For Horizontal Displacement", 0, &DISP_CH),
                num("maxHorizontal", "Max Horizontal Displacement", 5.0, -32000.0, 32000.0, -100.0, 100.0),
                popup("vertical", "Use For Vertical Displacement", 1, &DISP_CH),
                num("maxVertical", "Max Vertical Displacement", 5.0, -32000.0, 32000.0, -100.0, 100.0),
                popup("behavior", "Displacement Map Behavior", 1, &["Center Map", "Stretch Map to Fit", "Tile Map"]),
                check("wrap", "Wrap Pixels Around", false),
                check("expand", "Expand Output", true),
            ],
            render: displacement_map,
            pad: |c| if c.b("expand") { c.f("maxHorizontal").abs().max(c.f("maxVertical").abs()) } else { 0.0 },
        },
        EffectSpec {
            id: "fx.distort.magnify",
            name: "Magnify",
            category: CAT,
            params: vec![
                popup("shape", "Shape", 0, &["Circle", "Square"]),
                point("center", "Center", 0.5, 0.5),
                num("magnification", "Magnification", 200.0, 1.0, 1600.0, 100.0, 600.0),
                num("size", "Size", 100.0, 1.0, 4000.0, 10.0, 600.0),
                num("feather", "Feather", 0.0, 0.0, 1000.0, 0.0, 100.0),
                pct("opacity", "Opacity", 100.0),
                popup("scaling", "Scaling", 0, &["Standard", "Soft", "Scatter"]),
            ],
            render: magnify,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.opticsCompensation",
            name: "Optics Compensation",
            category: CAT,
            params: vec![
                slider("fov", "Field Of View (FOV)", 0.0, 0.0, 180.0, 1),
                check("reverse", "Reverse Lens Distortion", false),
                popup("orientation", "FOV Orientation", 0, &["Horizontal", "Vertical", "Diagonal"]),
                point("center", "View Center", 0.5, 0.5),
            ],
            render: optics,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.polarCoordinates",
            name: "Polar Coordinates",
            category: CAT,
            params: vec![pct("interpolation", "Interpolation", 0.0), popup("type", "Type of Conversion", 0, &["Rect to Polar", "Polar to Rect"])],
            render: polar,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ripple",
            name: "Ripple",
            category: CAT,
            params: vec![
                pct("radius", "Radius", 50.0),
                point("center", "Center of Ripple", 0.5, 0.5),
                popup("conversion", "Type of Conversion", 0, &["Asymmetric", "Symmetric"]),
                slider("speed", "Wave Speed", 1.0, -10.0, 10.0, 2),
                num("width", "Wave Width", 30.0, 1.0, 500.0, 2.0, 200.0),
                num("height", "Wave Height", 30.0, 0.0, 500.0, 0.0, 200.0),
                angle("phase", "Ripple Phase", 0.0),
            ],
            render: ripple,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.spherize",
            name: "Spherize",
            category: CAT,
            params: vec![num("radius", "Radius", 0.0, 0.0, 2500.0, 0.0, 500.0), point("center", "Center of Sphere", 0.5, 0.5)],
            render: spherize,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.warp",
            name: "Warp",
            category: CAT,
            params: vec![
                popup("style", "Warp Style", 0, &WARP_STYLES),
                popup("axis", "Warp Axis", 0, &["Horizontal", "Vertical"]),
                slider("bend", "Bend", 50.0, -100.0, 100.0, 0),
                slider("horizontal", "Horizontal Distortion", 0.0, -100.0, 100.0, 0),
                slider("vertical", "Vertical Distortion", 0.0, -100.0, 100.0, 0),
            ],
            render: warp_fx,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.smear",
            name: "Smear",
            category: CAT,
            params: vec![
                mask_param("sourceMask", "Source Mask"),
                mask_param("boundaryMask", "Boundary Mask"),
                v2("offset", "Mask Offset", [0.0, 0.0]),
                angle("rotation", "Mask Rotation", 0.0),
                pct("scale", "Mask Scale", 100.0),
                pct("percent", "Percent", 100.0),
                popup("elasticity", "Elasticity", 2, &["Stiff", "Less Stiff", "Below Normal", "Normal", "Above Normal", "Loose", "Liquid"]),
            ],
            render: smear,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.reshape",
            name: "Reshape",
            category: CAT,
            params: vec![
                mask_param("sourceMask", "Source Mask"),
                mask_param("destinationMask", "Destination Mask"),
                mask_param("boundaryMask", "Boundary Mask"),
                pct("percent", "Percent", 100.0),
                popup("elasticity", "Elasticity", 3, &["Stiff", "Less Stiff", "Below Normal", "Normal", "Above Normal", "Loose", "Liquid"]),
            ],
            render: reshape,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.twirlLegacy",
            name: "Twirl (Legacy)",
            category: "Obsolete",
            params: vec![angle("angle", "Angle", 0.0), num("radius", "Twirl Radius", 30.0, 0.0, 100.0, 0.0, 100.0), point("center", "Twirl Center", 0.5, 0.5)],
            render: |c, img| {
                let a = c.f("angle").to_radians();
                let ctr = c.pt("center");
                let rect = c.layer_rect();
                let rad = ((rect[2] - rect[0]).min(rect[3] - rect[1]) * 0.5 * c.f("radius") / 100.0).max(1.0);
                remap(img, false, |x, y| {
                    let (dx, dy) = (x - ctr[0], y - ctr[1]);
                    let d = dx.hypot(dy);
                    if d >= rad {
                        return Some([x, y]);
                    }
                    let t = (1.0 - d / rad).powi(2) * a;
                    let (s, co) = t.sin_cos();
                    Some([ctr[0] + dx * co - dy * s, ctr[1] + dx * s + dy * co])
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccBendIt",
            name: "CC Bend It",
            category: CAT,
            params: vec![num("bend", "Bend", 10.0, -360.0, 360.0, -180.0, 180.0), point("start", "Start", 0.5, 0.2), point("end", "End", 0.5, 0.8), popup("prestart", "Render Prestart", 1, &["None", "Static", "Bend"])],
            render: bend_it,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccBender",
            name: "CC Bender",
            category: CAT,
            params: vec![
                num("amount", "Amount", 10.0, -100.0, 100.0, -100.0, 100.0),
                popup("style", "Style", 0, &["Bend", "Marilyn", "Sharp", "Boxer"]),
                check("adjust", "Adjust To Distance", false),
                point("top", "Top", 0.5, 0.0),
                point("base", "Base", 0.5, 1.0),
            ],
            render: bender,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccBlobbylize",
            name: "CC Blobbylize",
            category: CAT,
            params: vec![
                layer_param("blobLayer", "Blob Layer"),
                popup("property", "Property", 4, &["Red", "Green", "Blue", "Alpha", "Luminance"]),
                num("softness", "Softness", 10.0, 0.0, 100.0, 0.0, 100.0),
                pct("cutAway", "Cut Away", 0.0),
                pct("lightIntensity", "Light Intensity", 100.0),
                color("lightColor", "Light Color", [1.0, 1.0, 1.0]),
                angle("lightDirection", "Light Direction", -45.0),
                pct("shadowAmount", "Shadow Amount", 0.0),
                num("refraction", "Refraction", 10.0, 0.0, 200.0, 0.0, 100.0),
            ],
            render: blobbylize,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccFloMotion",
            name: "CC Flo Motion",
            category: CAT,
            params: vec![
                point("knot1", "Knot 1", 0.3, 0.5),
                num("amount1", "Amount 1", 50.0, -1000.0, 1000.0, -100.0, 100.0),
                point("knot2", "Knot 2", 0.7, 0.5),
                num("amount2", "Amount 2", 50.0, -1000.0, 1000.0, -100.0, 100.0),
                check("tileEdges", "Tile Edges", true),
                slider("falloff", "Falloff", 1.0, 0.0, 10.0, 2),
            ],
            render: flo_motion,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccGriddler",
            name: "CC Griddler",
            category: CAT,
            params: vec![pct("horizontalScale", "Horizontal Scale", 100.0), pct("verticalScale", "Vertical Scale", 100.0), num("tileSize", "Tile Size", 10.0, 1.0, 200.0, 1.0, 100.0), angle("rotation", "Rotation", 0.0), check("cut", "Cut Tiles", false)],
            render: griddler,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccLens",
            name: "CC Lens",
            category: CAT,
            params: vec![point("center", "Center", 0.5, 0.5), num("size", "Size", 50.0, 0.0, 300.0, 0.0, 100.0), num("convergence", "Convergence", 50.0, -100.0, 100.0, -100.0, 100.0)],
            render: cc_lens,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccPageTurn",
            name: "CC Page Turn",
            category: CAT,
            params: vec![
                point("foldPosition", "Fold Position", 0.8, 0.8),
                angle("foldDirection", "Fold Direction", 225.0),
                num("foldRadius", "Fold Radius", 20.0, 0.0, 500.0, 0.0, 200.0),
                angle("lightDirection", "Light Direction", -45.0),
                popup("render", "Render", 0, &["Front & Back Page", "Front Page", "Back Page"]),
                layer_param("backPage", "Back Page"),
                pct("backOpacity", "Back Opacity", 100.0),
                color("paperColor", "Paper Color", [1.0, 1.0, 1.0]),
            ],
            render: page_turn,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccPowerPin",
            name: "CC Power Pin",
            category: CAT,
            params: vec![
                point("topLeft", "Top Left", 0.0, 0.0),
                point("topRight", "Top Right", 1.0, 0.0),
                point("bottomLeft", "Bottom Left", 0.0, 1.0),
                point("bottomRight", "Bottom Right", 1.0, 1.0),
                pct("perspective", "Perspective", 100.0),
                slider("expansionTop", "Expansion Top (%)", 0.0, -100.0, 100.0, 1),
                slider("expansionLeft", "Expansion Left (%)", 0.0, -100.0, 100.0, 1),
                slider("expansionRight", "Expansion Right (%)", 0.0, -100.0, 100.0, 1),
                slider("expansionBottom", "Expansion Bottom (%)", 0.0, -100.0, 100.0, 1),
            ],
            render: power_pin,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccRipplePulse",
            name: "CC Ripple Pulse",
            category: CAT,
            params: vec![
                point("center", "Center", 0.5, 0.5),
                num("pulseLevel", "Pulse Level (Animate)", 0.0, -1000.0, 1000.0, 0.0, 100.0),
                num("timeSpan", "Time Span (sec)", 1.0, 0.0, 100.0, 0.0, 10.0),
                num("amplitude", "Amplitude", 4.0, -100.0, 100.0, -20.0, 20.0),
                check("renderBump", "Render Bump Map", false),
            ],
            render: ripple_pulse,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccSlant",
            name: "CC Slant",
            category: CAT,
            params: vec![num("slant", "Slant", 0.0, -100.0, 100.0, -100.0, 100.0), check("stretching", "Stretching", false), num("height", "Height", 100.0, 0.0, 400.0, 0.0, 200.0), num("floor", "Floor", 0.0, -10000.0, 10000.0, -500.0, 500.0), check("setColor", "Set Color", false), color("color", "Color", [1.0, 1.0, 1.0])],
            render: slant,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccSmear",
            name: "CC Smear",
            category: CAT,
            params: vec![point("from", "From", 0.4, 0.5), point("to", "To", 0.6, 0.5), num("reach", "Reach", 50.0, 0.0, 100.0, 0.0, 100.0), num("radius", "Radius", 50.0, 0.0, 1000.0, 0.0, 300.0)],
            render: cc_smear,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccSplit",
            name: "CC Split",
            category: CAT,
            params: vec![point("pointA", "Point A", 0.5, 0.3), point("pointB", "Point B", 0.5, 0.7), num("split", "Split", 30.0, 0.0, 500.0, 0.0, 200.0)],
            render: |c, img| split(c, img, c.f("split"), c.f("split")),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccSplit2",
            name: "CC Split 2",
            category: CAT,
            params: vec![point("pointA", "Point A", 0.5, 0.3), point("pointB", "Point B", 0.5, 0.7), num("split1", "Split 1", 30.0, 0.0, 500.0, 0.0, 200.0), num("split2", "Split 2", 30.0, 0.0, 500.0, 0.0, 200.0)],
            render: |c, img| split(c, img, c.f("split1"), c.f("split2")),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.ccTiler",
            name: "CC Tiler",
            category: CAT,
            params: vec![pct("scale", "Scale", 50.0), point("center", "Center", 0.5, 0.5), pct("blend", "Blend w. Original", 0.0)],
            render: tiler,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.distort.detailPreservingUpscale",
            name: "Detail-preserving Upscale",
            category: CAT,
            params: vec![num("scale", "Scale", 100.0, 1.0, 1000.0, 100.0, 400.0), pct("detailReduction", "Reduce Detail Noise", 20.0), popup("alpha", "Alpha", 0, &["Detail-preserving", "Bicubic"])],
            render: upscale,
            pad: no_pad,
        },
    ]
}

fn cubic(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2], t: f64) -> [f64; 2] {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    [a[0] * w0 + b[0] * w1 + c[0] * w2 + d[0] * w3, a[1] * w0 + b[1] * w1 + c[1] * w2 + d[1] * w3]
}

fn bezier_warp(c: &FxCtx, img: &mut Img) {
    let p = |k: &str| c.pt(k);
    let (tl, tr, br, bl) = (p("topLeft"), p("topRight"), p("bottomRight"), p("bottomLeft"));
    let top = [tl, p("topLeftTangent"), p("topRightTangent"), tr];
    let right = [tr, p("rightTopTangent"), p("rightBottomTangent"), br];
    let bottom = [bl, p("bottomLeftTangent"), p("bottomRightTangent"), br];
    let left = [tl, p("leftTopTangent"), p("leftBottomTangent"), bl];
    let n = (c.f("quality").clamp(1.0, 10.0) * 4.0) as usize;
    let from = grid_points(c.layer_rect(), n, n);
    let mut to = Vec::with_capacity(from.len());
    for r in 0..=n {
        for k in 0..=n {
            let (u, v) = (k as f64 / n as f64, r as f64 / n as f64);
            let t = cubic(top[0], top[1], top[2], top[3], u);
            let b = cubic(bottom[0], bottom[1], bottom[2], bottom[3], u);
            let l = cubic(left[0], left[1], left[2], left[3], v);
            let rr = cubic(right[0], right[1], right[2], right[3], v);
            let mut q = [0.0; 2];
            for i in 0..2 {
                q[i] = (1.0 - v) * t[i] + v * b[i] + (1.0 - u) * l[i] + u * rr[i] - ((1.0 - u) * (1.0 - v) * tl[i] + u * (1.0 - v) * tr[i] + (1.0 - u) * v * bl[i] + u * v * br[i]);
            }
            to.push(q);
        }
    }
    *img = mesh_render(img, n, n, &from, &to);
}

fn parse_offsets(s: &str) -> Vec<[f64; 2]> {
    s.split_whitespace()
        .filter_map(|e| {
            let mut it = e.split(',');
            Some([it.next()?.parse().ok()?, it.next()?.parse().ok()?])
        })
        .collect()
}

fn catmull(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    0.5 * (2.0 * p1 + (-p0 + p2) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t)
}

fn dense_offsets(off: &[[f64; 2]], cols: usize, rows: usize, k: usize) -> Vec<[f64; 2]> {
    let at = |c: isize, r: isize| -> [f64; 2] {
        let c = c.clamp(0, cols as isize) as usize;
        let r = r.clamp(0, rows as isize) as usize;
        off.get(r * (cols + 1) + c).copied().unwrap_or([0.0; 2])
    };
    let (dc, dr) = (cols * k, rows * k);
    let mut out = Vec::with_capacity((dc + 1) * (dr + 1));
    for r in 0..=dr {
        for cc in 0..=dc {
            let (fc, fr) = (cc as f64 / k as f64, r as f64 / k as f64);
            let (ic, ir) = (fc.floor() as isize, fr.floor() as isize);
            let (tc, tr) = (fc - ic as f64, fr - ir as f64);
            let mut v = [0.0; 2];
            for d in 0..2 {
                let row = |rr: isize| catmull(at(ic - 1, rr)[d], at(ic, rr)[d], at(ic + 1, rr)[d], at(ic + 2, rr)[d], tc);
                v[d] = catmull(row(ir - 1), row(ir), row(ir + 1), row(ir + 2), tr);
            }
            out.push(v);
        }
    }
    out
}

fn mesh_warp(c: &FxCtx, img: &mut Img) {
    let cols = c.f("columns").round().clamp(1.0, 31.0) as usize;
    let rows = c.f("rows").round().clamp(1.0, 31.0) as usize;
    let off = parse_offsets(&c.s("mesh"));
    if off.len() != (cols + 1) * (rows + 1) || off.iter().all(|o| o[0] == 0.0 && o[1] == 0.0) {
        return;
    }
    let k = (c.f("quality").clamp(1.0, 10.0) / 2.0).ceil() as usize + 1;
    let dense = dense_offsets(&off, cols, rows, k);
    let from = grid_points(c.layer_rect(), cols * k, rows * k);
    let to: Vec<[f64; 2]> = from.iter().zip(&dense).map(|(p, d)| [p[0] + c.px(d[0]), p[1] + c.px(d[1])]).collect();
    *img = mesh_render(img, cols * k, rows * k, &from, &to);
}

pub const LIQUIFY_GRID: usize = 48;

fn liquify(c: &FxCtx, img: &mut Img) {
    let s = c.s("mesh");
    let off = parse_offsets(&s);
    let n = LIQUIFY_GRID;
    if off.len() != (n + 1) * (n + 1) {
        return;
    }
    let pct = c.f("percentage") / 100.0;
    let mo = c.v2("meshOffset");
    let from = grid_points(c.layer_rect(), n, n);
    let to: Vec<[f64; 2]> = from.iter().zip(&off).map(|(p, d)| [p[0] + c.px(d[0] * pct + mo[0]), p[1] + c.px(d[1] * pct + mo[1])]).collect();
    let mut out = mesh_render(img, n, n, &from, &to);
    if c.b("viewMesh") {
        for r in 0..=n {
            for k in 0..=n {
                let p = to[r * (n + 1) + k];
                let (x, y) = (p[0] as isize, p[1] as isize);
                if x >= 0 && y >= 0 && (x as usize) < out.w && (y as usize) < out.h {
                    let i = (y as usize * out.w + x as usize) * 4;
                    out.px[i..i + 4].copy_from_slice(&[0.5, 0.5, 0.5, 1.0]);
                }
            }
        }
    }
    *img = out;
}

pub fn chan_value(p: &[f32], which: u32) -> f32 {
    let a = p[3];
    let u = if a > 1e-6 { [p[0] / a, p[1] / a, p[2] / a] } else { [0.0; 3] };
    match which {
        0 => u[0],
        1 => u[1],
        2 => u[2],
        3 => a,
        4 => lum(u[0], u[1], u[2]),
        5 => super::util::rgb_to_hsl(u)[0],
        6 => super::util::rgb_to_hsl(u)[2],
        7 => super::util::rgb_to_hsl(u)[1],
        8 => 1.0,
        9 => 0.5,
        _ => 0.5,
    }
}

fn displacement_map(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let map = match c.other_fit("mapLayer", true, w, h, c.e("behavior")) {
        Some(m) => m,
        None => img.clone(),
    };
    let (hc, vc) = (c.e("horizontal"), c.e("vertical"));
    let (mh, mv) = (c.px(c.f("maxHorizontal")), c.px(c.f("maxVertical")));
    let wrap = c.b("wrap");
    let src = img.clone();
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let m = &map.px[(y * w + x) * 4..(y * w + x) * 4 + 4];
            let dx = (chan_value(m, hc) as f64 - 0.5) * 2.0 * mh;
            let dy = (chan_value(m, vc) as f64 - 0.5) * 2.0 * mv;
            let (mut sx, mut sy) = (x as f64 + 0.5 - dx, y as f64 + 0.5 - dy);
            if wrap {
                sx = sx.rem_euclid(w as f64);
                sy = sy.rem_euclid(h as f64);
            }
            let s = src.sample(sx, sy, wrap);
            row[x * 4..x * 4 + 4].copy_from_slice(&s);
        }
    });
}

fn magnify(c: &FxCtx, img: &mut Img) {
    let ctr = c.pt("center");
    let mag = (c.f("magnification") / 100.0).max(0.01);
    let size = c.px(c.f("size")).max(0.5);
    let fe = c.px(c.f("feather"));
    let op = c.f("opacity") as f32 / 100.0;
    let square = c.e("shape") == 1;
    let soft = c.e("scaling") == 1;
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (dx, dy) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let d = if square { dx.abs().max(dy.abs()) } else { dx.hypot(dy) };
            if d > size {
                continue;
            }
            let k = if fe > 0.0 { 1.0 - smooth((size - fe) as f32, size as f32, d as f32) } else { 1.0 };
            let mut s = src.sample(ctr[0] + dx / mag, ctr[1] + dy / mag, true);
            if soft {
                let s2 = src.sample(ctr[0] + dx / mag + 0.5 / mag, ctr[1] + dy / mag + 0.5 / mag, true);
                for i in 0..4 {
                    s[i] = (s[i] + s2[i]) * 0.5;
                }
            }
            let t = k * op;
            for i in 0..4 {
                row[x * 4 + i] += (s[i] - row[x * 4 + i]) * t;
            }
        }
    });
}

fn optics(c: &FxCtx, img: &mut Img) {
    let fov = c.f("fov").clamp(0.0, 179.0).to_radians();
    if fov < 1e-3 {
        return;
    }
    let rect = c.layer_rect();
    let ctr = c.pt("center");
    let (hw, hh) = ((rect[2] - rect[0]) * 0.5, (rect[3] - rect[1]) * 0.5);
    let norm = match c.e("orientation") {
        1 => hh,
        2 => hw.hypot(hh),
        _ => hw,
    }
    .max(1.0);
    let half = fov * 0.5;
    let rev = c.b("reverse");
    remap(img, false, |x, y| {
        let (dx, dy) = ((x - ctr[0]) / norm, (y - ctr[1]) / norm);
        let r = dx.hypot(dy);
        if r < 1e-9 {
            return Some([x, y]);
        }
        let rs = if rev { (r * half.tan()).atan() / half } else { (r * half).tan() / half.tan() };
        if !rs.is_finite() {
            return None;
        }
        let k = rs / r;
        Some([ctr[0] + dx * k * norm, ctr[1] + dy * k * norm])
    });
}

fn polar(c: &FxCtx, img: &mut Img) {
    let t = c.f("interpolation") / 100.0;
    if t <= 0.0 {
        return;
    }
    let rect = c.layer_rect();
    let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
    let (cx, cy) = (rect[0] + w * 0.5, rect[1] + h * 0.5);
    let to_polar = c.e("type") == 0;
    let rmax = w.min(h) * 0.5;
    remap(img, false, |x, y| {
        let s = if to_polar {
            let (dx, dy) = (x - cx, y - cy);
            let a = dy.atan2(dx) + FRAC_PI_2;
            let u = a.rem_euclid(TAU) / TAU;
            let v = dx.hypot(dy) / rmax;
            [rect[0] + u * w, rect[1] + v * h]
        } else {
            let u = (x - rect[0]) / w;
            let v = (y - rect[1]) / h;
            let a = u * TAU - FRAC_PI_2;
            [cx + a.cos() * v * rmax, cy + a.sin() * v * rmax]
        };
        Some([x + (s[0] - x) * t, y + (s[1] - y) * t])
    });
}

fn ripple(c: &FxCtx, img: &mut Img) {
    let rect = c.layer_rect();
    let rad = ((rect[2] - rect[0]).min(rect[3] - rect[1]) * 0.5 * c.f("radius") / 100.0).max(1.0);
    let ctr = c.pt("center");
    let speed = c.f("speed");
    let width = c.px(c.f("width")).max(0.5);
    let height = c.px(c.f("height"));
    let phase = c.f("phase").to_radians();
    let sym = c.e("conversion") == 1;
    let t = c.t;
    remap(img, false, |x, y| {
        let (dx, dy) = (x - ctr[0], y - ctr[1]);
        let d = dx.hypot(dy);
        if d >= rad || d < 1e-9 {
            return Some([x, y]);
        }
        let fall = 1.0 - d / rad;
        let wv = ((d / width - t * speed) * TAU + phase).sin() * height * 0.5 * fall;
        if sym {
            let k = (d + wv) / d;
            Some([ctr[0] + dx * k, ctr[1] + dy * k])
        } else {
            Some([x + wv * 0.7, y + wv])
        }
    });
}

fn spherize(c: &FxCtx, img: &mut Img) {
    let rad = c.px(c.f("radius"));
    if rad < 0.5 {
        return;
    }
    let ctr = c.pt("center");
    remap(img, false, |x, y| {
        let (dx, dy) = (x - ctr[0], y - ctr[1]);
        let d = dx.hypot(dy);
        if d >= rad || d < 1e-9 {
            return Some([x, y]);
        }
        let rs = (d / rad).asin() / FRAC_PI_2 * rad;
        let k = rs / d;
        Some([ctr[0] + dx * k, ctr[1] + dy * k])
    });
}

fn warp_point(style: u32, b: f64, u: f64, v: f64) -> [f64; 2] {
    match style {
        0 => {
            let th = b * FRAC_PI_2;
            if th.abs() < 1e-4 {
                return [u, v];
            }
            let r0 = 1.0 / th;
            let a = u * th;
            [(r0 - v) * a.sin(), r0 - (r0 - v) * a.cos()]
        }
        1 => [u, v + if v > 0.0 { -b * (1.0 - u * u) * v * 0.5 } else { 0.0 }],
        2 => [u, v - if v < 0.0 { b * (1.0 - u * u) * (-v) * 0.5 } else { 0.0 }],
        3 => [u, v - b * (1.0 - u * u) * 0.5],
        4 => [u * (1.0 + b * 0.35 * (1.0 - v * v)), v * (1.0 + b * 0.5 * (1.0 - u * u))],
        5 => [u, v + b * 0.5 * (u * u) * ((v + 1.0) * 0.5)],
        6 => [u, v - b * 0.5 * (u * u) * ((1.0 - v) * 0.5)],
        7 => [u, v + b * 0.25 * (u * PI).sin()],
        8 => [u + b * 0.12 * (v * PI).sin(), v + b * 0.25 * (u * PI).sin()],
        9 => [u, v * (1.0 + b * u * 0.5) + b * 0.15 * (u * PI).sin()],
        10 => [u, v - b * u * 0.5],
        11 => {
            let r = u.hypot(v);
            if r >= 1.0 || r < 1e-9 {
                return [u, v];
            }
            let k = 1.0 + b * (1.0 - r * r) * 0.6;
            [u * k, v * k]
        }
        12 => [u * (1.0 + b * 0.4 * (1.0 - v * v) * (1.0 - u * u)), v * (1.0 + b * 0.4 * (1.0 - u * u) * (1.0 - v * v))],
        13 => [u * (1.0 - b * 0.4 * (1.0 - v * v)), v * (1.0 + b * 0.4 * (1.0 - u * u))],
        _ => {
            let r = u.hypot(v).min(1.5);
            let a = b * PI * (1.0 - r / 1.5).max(0.0);
            let (s, co) = a.sin_cos();
            [u * co - v * s, u * s + v * co]
        }
    }
}

fn warp_fx(c: &FxCtx, img: &mut Img) {
    let b = c.f("bend") / 100.0;
    let hd = c.f("horizontal") / 100.0;
    let vd = c.f("vertical") / 100.0;
    if b.abs() < 1e-4 && hd.abs() < 1e-4 && vd.abs() < 1e-4 {
        return;
    }
    let rect = c.layer_rect();
    let (cx, cy) = ((rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5);
    let (hw, hh) = (((rect[2] - rect[0]) * 0.5).max(1.0), ((rect[3] - rect[1]) * 0.5).max(1.0));
    let style = c.e("style");
    let vert = c.e("axis") == 1;
    let n = 32;
    let from = grid_points(rect, n, n);
    let to: Vec<[f64; 2]> = from
        .iter()
        .map(|p| {
            let (mut u, mut v) = ((p[0] - cx) / hw, (p[1] - cy) / hh);
            if vert {
                std::mem::swap(&mut u, &mut v);
            }
            let mut q = warp_point(style, b, u, v);
            if vert {
                q.swap(0, 1);
            }
            let (mut x, mut y) = (q[0], q[1]);
            x *= 1.0 + hd * y * 0.5;
            y *= 1.0 + vd * x * 0.5;
            [cx + x * hw, cy + y * hh]
        })
        .collect();
    *img = mesh_render(img, n, n, &from, &to);
}

fn mask_points(p: &BezPath, n: usize) -> Vec<[f64; 2]> {
    let pts = p.flatten(0.5);
    if pts.len() < 2 {
        return pts;
    }
    let mut len = vec![0.0];
    for i in 1..pts.len() {
        len.push(len[i - 1] + (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1]));
    }
    let total = *len.last().unwrap_or(&0.0);
    if total <= 0.0 {
        return vec![pts[0]; n];
    }
    let mut out = Vec::with_capacity(n);
    let mut j = 0;
    for k in 0..n {
        let s = total * k as f64 / n as f64;
        while j + 1 < len.len() - 1 && len[j + 1] < s {
            j += 1;
        }
        let seg = (len[j + 1] - len[j]).max(1e-9);
        let t = ((s - len[j]) / seg).clamp(0.0, 1.0);
        out.push([pts[j][0] + (pts[j + 1][0] - pts[j][0]) * t, pts[j][1] + (pts[j + 1][1] - pts[j][1]) * t]);
    }
    out
}

fn idw(anchors: &[([f64; 2], [f64; 2])], p: [f64; 2], power: f64) -> [f64; 2] {
    let mut acc = [0.0; 2];
    let mut ws = 0.0;
    for (a, d) in anchors {
        let dd = (p[0] - a[0]).hypot(p[1] - a[1]);
        if dd < 1e-6 {
            return *d;
        }
        let w = 1.0 / dd.powf(power);
        acc[0] += d[0] * w;
        acc[1] += d[1] * w;
        ws += w;
    }
    if ws > 0.0 { [acc[0] / ws, acc[1] / ws] } else { [0.0; 2] }
}

fn elastic_power(e: u32) -> f64 {
    [4.0, 3.5, 3.0, 2.5, 2.2, 2.0, 1.7][e.min(6) as usize]
}

fn deform(img: &mut Img, rect: [f64; 4], anchors: &[([f64; 2], [f64; 2])], power: f64) {
    let n = 40;
    let from = grid_points(rect, n, n);
    let to: Vec<[f64; 2]> = from.par_iter().map(|p| {
        let d = idw(anchors, *p, power);
        [p[0] + d[0], p[1] + d[1]]
    }).collect();
    *img = mesh_render(img, n, n, &from, &to);
}

fn boundary_anchors(c: &FxCtx, idx: usize, rect: [f64; 4]) -> Vec<([f64; 2], [f64; 2])> {
    match c.mask(idx) {
        Some(b) => mask_points(&b, 48).into_iter().map(|p| (p, [0.0, 0.0])).collect(),
        None => {
            let mut v = vec![];
            for k in 0..12 {
                let t = k as f64 / 12.0;
                v.push(([rect[0] + (rect[2] - rect[0]) * t, rect[1]], [0.0, 0.0]));
                v.push(([rect[0] + (rect[2] - rect[0]) * t, rect[3]], [0.0, 0.0]));
                v.push(([rect[0], rect[1] + (rect[3] - rect[1]) * t], [0.0, 0.0]));
                v.push(([rect[2], rect[1] + (rect[3] - rect[1]) * t], [0.0, 0.0]));
            }
            v
        }
    }
}

fn smear(c: &FxCtx, img: &mut Img) {
    let Some(src) = c.mask(c.f("sourceMask") as usize) else { return };
    let rect = c.layer_rect();
    let pct = c.f("percent") / 100.0;
    let off = c.v2("offset");
    let rot = c.f("rotation").to_radians();
    let sc = c.f("scale") / 100.0;
    let pts = mask_points(&src, 48);
    let n = pts.len().max(1) as f64;
    let ctr = [pts.iter().map(|p| p[0]).sum::<f64>() / n, pts.iter().map(|p| p[1]).sum::<f64>() / n];
    let (s, co) = rot.sin_cos();
    let mut anchors: Vec<([f64; 2], [f64; 2])> = pts
        .iter()
        .chain(std::iter::once(&ctr))
        .map(|p| {
            let (dx, dy) = ((p[0] - ctr[0]) * sc, (p[1] - ctr[1]) * sc);
            let q = [ctr[0] + dx * co - dy * s + c.px(off[0]), ctr[1] + dx * s + dy * co + c.px(off[1])];
            (*p, [(q[0] - p[0]) * pct, (q[1] - p[1]) * pct])
        })
        .collect();
    anchors.extend(boundary_anchors(c, c.f("boundaryMask") as usize, rect));
    deform(img, rect, &anchors, elastic_power(c.e("elasticity")));
}

fn reshape(c: &FxCtx, img: &mut Img) {
    let (Some(a), Some(b)) = (c.mask(c.f("sourceMask") as usize), c.mask(c.f("destinationMask") as usize)) else { return };
    let rect = c.layer_rect();
    let pct = c.f("percent") / 100.0;
    let pa = mask_points(&a, 64);
    let pb = mask_points(&b, 64);
    let start = (0..pb.len()).min_by(|i, j| {
        let di = (pb[*i][0] - pa[0][0]).hypot(pb[*i][1] - pa[0][1]);
        let dj = (pb[*j][0] - pa[0][0]).hypot(pb[*j][1] - pa[0][1]);
        di.total_cmp(&dj)
    }).unwrap_or(0);
    let mut anchors: Vec<([f64; 2], [f64; 2])> = pa.iter().enumerate().map(|(i, p)| {
        let q = pb[(i + start) % pb.len()];
        (*p, [(q[0] - p[0]) * pct, (q[1] - p[1]) * pct])
    }).collect();
    anchors.extend(boundary_anchors(c, c.f("boundaryMask") as usize, rect));
    deform(img, rect, &anchors, elastic_power(c.e("elasticity")));
}

fn bend_it(c: &FxCtx, img: &mut Img) {
    let bend = c.f("bend").to_radians();
    if bend.abs() < 1e-5 {
        return;
    }
    let a = c.pt("start");
    let b = c.pt("end");
    let (ax, ay) = (b[0] - a[0], b[1] - a[1]);
    let len = ax.hypot(ay).max(1.0);
    let (ux, uy) = (ax / len, ay / len);
    let (nx, ny) = (-uy, ux);
    let pre = c.e("prestart");
    let r = len / bend;
    remap(img, false, |x, y| {
        let (dx, dy) = (x - a[0], y - a[1]);
        let t = dx * ux + dy * uy;
        let s = dx * nx + dy * ny;
        if t < 0.0 {
            return if pre == 0 { None } else { Some([x, y]) };
        }
        let cxp = -r;
        let (px, py) = (s - cxp, t);
        let ang = py.atan2(px);
        let rr = px.hypot(py);
        let tt = ang * r.abs() * r.signum();
        let ss = rr + cxp;
        if tt.abs() > len * 4.0 || (r > 0.0 && px < 0.0) {
            return None;
        }
        Some([a[0] + ux * tt + nx * ss, a[1] + uy * tt + ny * ss])
    });
}

fn bender(c: &FxCtx, img: &mut Img) {
    let amt = c.f("amount") / 100.0;
    if amt.abs() < 1e-5 {
        return;
    }
    let top = c.pt("top");
    let base = c.pt("base");
    let (ax, ay) = (top[0] - base[0], top[1] - base[1]);
    let len = ax.hypot(ay).max(1.0);
    let (ux, uy) = (ax / len, ay / len);
    let (nx, ny) = (-uy, ux);
    let style = c.e("style");
    let adjust = c.b("adjust");
    remap(img, false, |x, y| {
        let (dx, dy) = (x - base[0], y - base[1]);
        let t = (dx * ux + dy * uy) / len;
        let s = dx * nx + dy * ny;
        if t <= 0.0 {
            return Some([x, y]);
        }
        let tt = t.min(1.5);
        let shift = match style {
            1 => amt * len * 0.5 * (tt * PI).sin() * (1.0 - (s / len).abs()).max(0.0),
            2 => amt * len * 0.5 * tt.min(1.0),
            3 => amt * len * 0.5 * if tt < 0.5 { tt * 2.0 } else { 1.0 },
            _ => amt * len * 0.5 * tt * tt,
        };
        let scale = if adjust { 1.0 / (1.0 + (shift / len).abs()) } else { 1.0 };
        let ns = s - shift;
        let nt = t * len * scale;
        Some([base[0] + ux * nt + nx * ns, base[1] + uy * nt + ny * ns])
    });
}

fn blobbylize(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let blob = c.other_fit("blobLayer", true, w, h, 1).unwrap_or_else(|| img.clone());
    let prop = c.e("property");
    let mut hf: Vec<f32> = blob.px.chunks_exact(4).map(|p| chan_value(p, [0, 1, 2, 3, 4][prop.min(4) as usize])).collect();
    let soft = c.px(c.f("softness") * 0.3);
    if soft > 0.3 {
        super::util::blur_scalar(&mut hf, w, h, soft, soft);
    }
    let cut = c.f("cutAway") as f32 / 100.0;
    let li = c.f("lightIntensity") as f32 / 100.0;
    let lc = c.c("lightColor");
    let la = c.f("lightDirection").to_radians();
    let (lx, ly) = (la.cos() as f32, la.sin() as f32);
    let sh = c.f("shadowAmount") as f32 / 100.0;
    let refr = c.px(c.f("refraction"));
    let src = img.clone();
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let g = |xx: usize, yy: usize| hf[yy.min(h - 1) * w + xx.min(w - 1)];
            let gx = g(x + 1, y) - g(x.saturating_sub(1), y);
            let gy = g(x, y + 1) - g(x, y.saturating_sub(1));
            let hv = hf[y * w + x];
            let mut s = src.sample(x as f64 + 0.5 - gx as f64 * refr, y as f64 + 0.5 - gy as f64 * refr, true);
            let shade = -(gx * lx + gy * ly) * 4.0;
            let spec = shade.max(0.0).powf(3.0) * li;
            let dark = (-shade).max(0.0) * sh;
            for k in 0..3 {
                s[k] = (s[k] * (1.0 - dark) + lc[k] * spec * s[3]).max(0.0);
            }
            if hv < cut {
                s = [0.0; 4];
            }
            row[x * 4..x * 4 + 4].copy_from_slice(&s);
        }
    });
}

fn flo_motion(c: &FxCtx, img: &mut Img) {
    let k1 = c.pt("knot1");
    let k2 = c.pt("knot2");
    let (a1, a2) = (c.f("amount1") / 100.0, c.f("amount2") / 100.0);
    let fall = c.f("falloff").max(0.01);
    let tile = c.b("tileEdges");
    let rect = c.layer_rect();
    let diag = (rect[2] - rect[0]).hypot(rect[3] - rect[1]).max(1.0);
    let w = img.w as f64;
    let hgt = img.h as f64;
    let src = img.clone();
    let ww = img.w;
    img.px.par_chunks_mut(ww * 4).enumerate().for_each(|(y, row)| {
        for x in 0..ww {
            let mut p = [x as f64 + 0.5, y as f64 + 0.5];
            for (k, a) in [(k1, a1), (k2, a2)] {
                let (dx, dy) = (p[0] - k[0], p[1] - k[1]);
                let d = dx.hypot(dy) / diag;
                let pull = a * (1.0 - d).max(0.0).powf(fall);
                p = [p[0] - dx * pull, p[1] - dy * pull];
            }
            if tile {
                p = [p[0].rem_euclid(w), p[1].rem_euclid(hgt)];
            }
            let s = src.sample(p[0], p[1], tile);
            row[x * 4..x * 4 + 4].copy_from_slice(&s);
        }
    });
}

fn griddler(c: &FxCtx, img: &mut Img) {
    let ts = c.px(c.f("tileSize")).max(1.0);
    let (hs, vs) = ((c.f("horizontalScale") / 100.0).max(0.01), (c.f("verticalScale") / 100.0).max(0.01));
    let rot = c.f("rotation").to_radians();
    let cut = c.b("cut");
    let (s, co) = rot.sin_cos();
    remap(img, true, |x, y| {
        let (tx, ty) = ((x / ts).floor(), (y / ts).floor());
        let (cx, cy) = ((tx + 0.5) * ts, (ty + 0.5) * ts);
        let (dx, dy) = (x - cx, y - cy);
        let (rx, ry) = (dx * co + dy * s, -dx * s + dy * co);
        let (sx, sy) = (rx / hs, ry / vs);
        if cut && (sx.abs() > ts * 0.5 || sy.abs() > ts * 0.5) {
            return None;
        }
        Some([cx + sx, cy + sy])
    });
}

fn cc_lens(c: &FxCtx, img: &mut Img) {
    let ctr = c.pt("center");
    let rect = c.layer_rect();
    let size = ((rect[2] - rect[0]).max(rect[3] - rect[1]) * c.f("size") / 100.0 * 0.5).max(1.0);
    let conv = c.f("convergence") / 100.0;
    remap(img, false, |x, y| {
        let (dx, dy) = (x - ctr[0], y - ctr[1]);
        let d = dx.hypot(dy) / size;
        if d >= 1.0 {
            return None;
        }
        let k = if conv >= 0.0 { 1.0 - conv * (1.0 - d * d).sqrt() * 0.9 } else { 1.0 + (-conv) * d * d };
        Some([ctr[0] + dx * k, ctr[1] + dy * k])
    });
}

fn page_turn(c: &FxCtx, img: &mut Img) {
    let fp = c.pt("foldPosition");
    let dir = c.f("foldDirection").to_radians();
    let (nx, ny) = (dir.cos(), dir.sin());
    let r = c.px(c.f("foldRadius")).max(0.5);
    let la = c.f("lightDirection").to_radians();
    let mode = c.e("render");
    let back_op = c.f("backOpacity") as f32 / 100.0;
    let paper = c.c("paperColor");
    let (w, h) = (img.w, img.h);
    let back = c.other_fit("backPage", true, w, h, 1);
    let src = img.clone();
    let rect = c.layer_rect();
    let inside = |p: [f64; 2]| p[0] >= rect[0] && p[1] >= rect[1] && p[0] < rect[2] && p[1] < rect[3];
    let light = (la.cos() * nx + la.sin() * ny) as f32;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let p = [x as f64 + 0.5, y as f64 + 0.5];
            let d = (p[0] - fp[0]) * nx + (p[1] - fp[1]) * ny;
            let foot = [p[0] - nx * d, p[1] - ny * d];
            let front = if d < 0.0 { src.sample(p[0], p[1], false) } else { [0.0; 4] };
            let (t, shade) = if d < 0.0 {
                (-d + PI * r, 0.85 + 0.15 * light)
            } else if d <= r {
                let a = (d / r).asin();
                (PI * r - a * r, 0.6 + 0.4 * (a.cos() as f32) * (0.5 + 0.5 * light))
            } else {
                (-1.0, 0.0)
            };
            let mut out = if mode == 2 { [0.0; 4] } else { front };
            if t >= 0.0 && mode != 1 {
                let q = [foot[0] + nx * t, foot[1] + ny * t];
                if inside(q) {
                    let s = src.sample(q[0], q[1], true);
                    let bk = match &back {
                        Some(b) => b.sample(q[0], q[1], true),
                        None => [paper[0] * s[3], paper[1] * s[3], paper[2] * s[3], s[3]],
                    };
                    let mut col = [0.0f32; 4];
                    for k in 0..4 {
                        col[k] = s[k] + (bk[k] - s[k]) * back_op;
                    }
                    for k in 0..3 {
                        col[k] *= shade;
                    }
                    let ka = 1.0 - col[3];
                    for k in 0..4 {
                        out[k] = col[k] + out[k] * ka;
                    }
                }
            }
            row[x * 4..x * 4 + 4].copy_from_slice(&out);
        }
    });
}

fn power_pin(c: &FxCtx, img: &mut Img) {
    let r = c.layer_rect();
    let (ew, eh) = (r[2] - r[0], r[3] - r[1]);
    let et = c.f("expansionTop") / 100.0 * eh;
    let eb = c.f("expansionBottom") / 100.0 * eh;
    let el = c.f("expansionLeft") / 100.0 * ew;
    let er = c.f("expansionRight") / 100.0 * ew;
    let src = [[r[0] - el, r[1] - et], [r[2] + er, r[1] - et], [r[0] - el, r[3] + eb], [r[2] + er, r[3] + eb]];
    let dst = [c.pt("topLeft"), c.pt("topRight"), c.pt("bottomLeft"), c.pt("bottomRight")];
    let persp = c.f("perspective") / 100.0;
    let Some(hm) = homography(dst, src) else { return };
    let ctr_d = [(dst[0][0] + dst[1][0] + dst[2][0] + dst[3][0]) * 0.25, (dst[0][1] + dst[1][1] + dst[2][1] + dst[3][1]) * 0.25];
    let ctr_s = hm.apply(ctr_d[0], ctr_d[1]);
    remap(img, false, |x, y| {
        let wv = hm.0[2][0] * x + hm.0[2][1] * y + hm.0[2][2];
        if wv <= 0.0 {
            return None;
        }
        let ph = hm.apply(x, y);
        if (persp - 1.0).abs() < 1e-6 {
            return Some(ph);
        }
        let jx = hm.apply(ctr_d[0] + 1.0, ctr_d[1]);
        let jy = hm.apply(ctr_d[0], ctr_d[1] + 1.0);
        let (dx, dy) = (x - ctr_d[0], y - ctr_d[1]);
        let lin = [ctr_s[0] + (jx[0] - ctr_s[0]) * dx + (jy[0] - ctr_s[0]) * dy, ctr_s[1] + (jx[1] - ctr_s[1]) * dx + (jy[1] - ctr_s[1]) * dy];
        Some([lin[0] + (ph[0] - lin[0]) * persp, lin[1] + (ph[1] - lin[1]) * persp])
    });
}

fn ripple_pulse(c: &FxCtx, img: &mut Img) {
    let ctr = c.pt("center");
    let level = c.px(c.f("pulseLevel"));
    let amp = c.px(c.f("amplitude"));
    let span = c.f("timeSpan").max(0.01);
    let bump = c.b("renderBump");
    let width = c.px(30.0) * span;
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (dx, dy) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let d = dx.hypot(dy);
            let u = (d - level * 4.0) / width.max(1.0);
            let wave = if u.abs() < 1.0 { (u * PI).sin() * (1.0 - u.abs()) } else { 0.0 };
            if bump {
                let v = (0.5 + wave * 0.5) as f32;
                row[x * 4..x * 4 + 4].copy_from_slice(&[v, v, v, 1.0]);
                continue;
            }
            if d < 1e-9 || wave == 0.0 {
                continue;
            }
            let k = (d + wave * amp) / d;
            let s = src.sample(ctr[0] + dx * k, ctr[1] + dy * k, true);
            row[x * 4..x * 4 + 4].copy_from_slice(&s);
        }
    });
}

fn slant(c: &FxCtx, img: &mut Img) {
    let sl = c.f("slant") / 100.0;
    let floor = c.px(c.f("floor")) + c.layer_rect()[3];
    let hgt = (c.f("height") / 100.0).max(0.01);
    let stretch = c.b("stretching");
    let set = c.b("setColor");
    let col = c.c("color");
    remap(img, false, |x, y| {
        let dy = floor - y;
        let sy = floor - dy / hgt;
        let sx = x + dy * sl * if stretch { 1.0 + dy.abs() / 1000.0 } else { 1.0 };
        Some([sx, sy])
    });
    if set {
        img.px.par_chunks_mut(4).for_each(|p| {
            let a = p[3];
            p[0] = col[0] * a;
            p[1] = col[1] * a;
            p[2] = col[2] * a;
        });
    }
}

fn cc_smear(c: &FxCtx, img: &mut Img) {
    let from = c.pt("from");
    let to = c.pt("to");
    let reach = c.f("reach") / 100.0;
    let rad = c.px(c.f("radius")).max(1.0);
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    remap(img, false, |x, y| {
        let d = (x - to[0]).hypot(y - to[1]);
        let w = (1.0 - d / rad).clamp(0.0, 1.0);
        let w = w * w * (3.0 - 2.0 * w) * reach.max(0.0);
        Some([x - dx * w, y - dy * w])
    });
}

fn split(c: &FxCtx, img: &mut Img, s1: f64, s2: f64) {
    let a = c.pt("pointA");
    let b = c.pt("pointB");
    let (ax, ay) = (b[0] - a[0], b[1] - a[1]);
    let len = ax.hypot(ay).max(1.0);
    let (ux, uy) = (ax / len, ay / len);
    let (nx, ny) = (-uy, ux);
    let (o1, o2) = (c.px(s1) * 0.5, c.px(s2) * 0.5);
    remap(img, false, |x, y| {
        let (dx, dy) = (x - a[0], y - a[1]);
        let t = (dx * ux + dy * uy) / len;
        let s = dx * nx + dy * ny;
        if !(0.0..=1.0).contains(&t) {
            return Some([x, y]);
        }
        let open = (t * PI).sin() * (o1 * (1.0 - t) + o2 * t) * 2.0;
        if s.abs() < open {
            return None;
        }
        let shift = open * s.signum();
        Some([x - nx * shift, y - ny * shift])
    });
}

fn tiler(c: &FxCtx, img: &mut Img) {
    let sc = (c.f("scale") / 100.0).max(0.01);
    let ctr = c.pt("center");
    let rect = c.layer_rect();
    let (w, h) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    let blend = c.f("blend") as f32 / 100.0;
    let orig = img.clone();
    remap(img, true, |x, y| {
        let u = ((x - ctr[0]) / sc + w * 0.5).rem_euclid(w) + rect[0];
        let v = ((y - ctr[1]) / sc + h * 0.5).rem_euclid(h) + rect[1];
        Some([u, v])
    });
    if blend > 0.0 {
        img.px.par_iter_mut().zip(orig.px.par_iter()).for_each(|(v, o)| *v += (*o - *v) * blend);
    }
}

fn upscale(c: &FxCtx, img: &mut Img) {
    let sc = (c.f("scale") / 100.0).max(0.01);
    if (sc - 1.0).abs() < 1e-4 {
        return;
    }
    let rect = c.layer_rect();
    let (cx, cy) = ((rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5);
    remap(img, false, |x, y| Some([cx + (x - cx) / sc, cy + (y - cy) / sc]));
    let det = 1.0 - c.f("detailReduction") as f32 / 100.0;
    if sc > 1.0 && det > 0.0 {
        let mut b = img.clone();
        gaussian(&mut b, sc * 0.6, sc * 0.6, true);
        unsharp_into(img, &b, det * 0.8);
    }
}

pub fn liquify_stroke(mesh: &str, size: [f64; 2], tool: &str, points: &[[f64; 2]], brush: f64, pressure: f64, jitter: f64, clone_off: [f64; 2]) -> String {
    let n = LIQUIFY_GRID;
    let mut off = parse_offsets(mesh);
    if off.len() != (n + 1) * (n + 1) {
        off = vec![[0.0; 2]; (n + 1) * (n + 1)];
    }
    let grid = grid_points([0.0, 0.0, size[0], size[1]], n, n);
    let rad = (brush * 0.5).max(1.0);
    let k = pressure / 100.0;
    for (si, p) in points.iter().enumerate() {
        let prev = if si > 0 { points[si - 1] } else { *p };
        let (mx, my) = (p[0] - prev[0], p[1] - prev[1]);
        for (i, g) in grid.iter().enumerate() {
            let cur = [g[0] + off[i][0], g[1] + off[i][1]];
            let (dx, dy) = (cur[0] - p[0], cur[1] - p[1]);
            let d = dx.hypot(dy);
            if d > rad {
                continue;
            }
            let f = (1.0 - d / rad).powi(2) * k;
            let delta = match tool {
                "turbulence" => {
                    let a = super::util::rand01(i as i32, si as i32, 0, 7) as f64 * TAU;
                    [a.cos() * rad * 0.1 * f * jitter / 100.0, a.sin() * rad * 0.1 * f * jitter / 100.0]
                }
                "twirlClockwise" | "twirlCounterclockwise" => {
                    let s = if tool == "twirlClockwise" { 1.0 } else { -1.0 };
                    let a = 0.15 * f * s;
                    let (sn, cs) = a.sin_cos();
                    [dx * cs - dy * sn - dx, dx * sn + dy * cs - dy]
                }
                "pucker" => [-dx * 0.1 * f, -dy * 0.1 * f],
                "bloat" => [dx * 0.1 * f, dy * 0.1 * f],
                "shiftPixels" => [-my * f, mx * f],
                "reflection" => [-mx * f * 0.5 - dx * 0.05 * f, -my * f * 0.5 - dy * 0.05 * f],
                "clone" => [clone_off[0] * f * 0.1, clone_off[1] * f * 0.1],
                "reconstruction" => [-off[i][0] * f * 0.5, -off[i][1] * f * 0.5],
                _ => [mx * f, my * f],
            };
            off[i][0] += delta[0];
            off[i][1] += delta[1];
        }
    }
    off.iter().map(|o| format!("{:.3},{:.3}", o[0], o[1])).collect::<Vec<_>>().join(" ")
}
