use std::f64::consts::{PI, TAU};

use rayon::prelude::*;

use super::util::{gaussian, lum, perlin3, rand01, remap, smooth, value3};
use super::{EffectSpec, FxCtx, angle, check, color, layer_param, no_pad, num, pct, point, popup, slider};
use crate::render::image::Img;

const CAT: &str = "Stylize";
const PERSP: &str = "Perspective";

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.stylize.brushStrokes",
            name: "Brush Strokes",
            category: CAT,
            params: vec![angle("angle", "Stroke Angle", 135.0), num("size", "Brush Size", 2.0, 0.0, 5.0, 0.0, 5.0), num("length", "Stroke Length", 8.0, 0.0, 40.0, 0.0, 40.0), pct("density", "Stroke Density", 100.0), slider("randomness", "Stroke Randomness", 1.0, 0.0, 2.0, 2), popup("surface", "Paint Surface", 0, &["Paint On Original Image", "Paint On Transparent", "Paint On White", "Paint On Black"]), pct("blend", "Blend With Original", 0.0)],
            render: brush_strokes,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.cartoon",
            name: "Cartoon",
            category: CAT,
            params: vec![
                popup("render", "Render", 0, &["Fill & Edges", "Fill", "Edges"]),
                num("detailRadius", "Detail Radius", 8.0, 0.0, 100.0, 0.0, 30.0),
                num("detailThreshold", "Detail Threshold", 10.0, 0.0, 100.0, 0.0, 100.0),
                slider("shadingSteps", "Shading Steps", 8.0, 1.0, 64.0, 0),
                pct("shadingSmoothness", "Shading Smoothness", 70.0),
                num("edgeThreshold", "Edge Threshold", 1.6, 0.0, 5.0, 0.0, 5.0),
                num("edgeWidth", "Edge Width", 1.5, 0.0, 5.0, 0.0, 5.0),
                pct("edgeSoftness", "Edge Softness", 60.0),
                pct("edgeOpacity", "Edge Opacity", 100.0),
            ],
            render: cartoon,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.ccBlockLoad",
            name: "CC Block Load",
            category: CAT,
            params: vec![pct("completion", "Completion", 100.0), check("scanlining", "Scanlining", true), check("antialiasing", "Antialiasing", true), slider("startCleared", "Start Cleared", 1.0, 0.0, 1.0, 0)],
            render: block_load,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.ccBurnFilm",
            name: "CC Burn Film",
            category: CAT,
            params: vec![pct("burn", "Burn", 0.0), point("center", "Center", 0.5, 0.5), slider("seed", "Random Seed", 1.0, 0.0, 10000.0, 0)],
            render: burn_film,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.ccGlass",
            name: "CC Glass",
            category: CAT,
            params: vec![
                layer_param("bumpMap", "Bump Map"),
                popup("property", "Property", 4, &["Red", "Green", "Blue", "Alpha", "Lightness"]),
                num("height", "Height", 50.0, -100.0, 100.0, -100.0, 100.0),
                num("displacement", "Displacement", 100.0, -500.0, 500.0, -200.0, 200.0),
                slider("softness", "Softness", 10.0, 0.0, 100.0, 1),
                pct("lightIntensity", "Light Intensity", 100.0),
                color("lightColor", "Light Color", [1.0, 1.0, 1.0]),
                angle("lightDirection", "Light Direction", -45.0),
                pct("ambient", "Ambient", 30.0),
            ],
            render: cc_glass,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.ccHexTile",
            name: "CC HexTile",
            category: CAT,
            params: vec![num("radius", "Radius", 30.0, 1.0, 1000.0, 1.0, 200.0), point("center", "Center", 0.5, 0.5), angle("rotate", "Rotate", 0.0), pct("smearing", "Smearing", 0.0)],
            render: hex_tile,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.ccKaleida",
            name: "CC Kaleida",
            category: CAT,
            params: vec![point("center", "Center", 0.5, 0.5), num("size", "Size", 20.0, 0.0, 200.0, 0.0, 100.0), popup("mirroring", "Mirroring", 0, &["Unfold", "Flower", "Pinwheel", "Star", "Diamonds", "Wheel", "Kaleidoscope"]), angle("rotation", "Rotation", 0.0), check("floatingCenter", "Floating Center", false)],
            render: kaleida,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.ccMrSmoothie",
            name: "CC Mr. Smoothie",
            category: CAT,
            params: vec![popup("property", "Property", 4, &["Red", "Green", "Blue", "Alpha", "Lightness", "Hue", "Saturation"]), slider("smoothness", "Smoothness", 5.0, 0.0, 100.0, 1), point("sampleA", "Sample A", 0.2, 0.5), point("sampleB", "Sample B", 0.8, 0.5), slider("phase", "Phase", 0.0, 0.0, 360.0, 1), pct("blend", "Blend w. Original", 0.0)],
            render: mr_smoothie,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.ccPlastic",
            name: "CC Plastic",
            category: CAT,
            params: vec![
                layer_param("bumpLayer", "Bump Layer"),
                popup("property", "Property", 4, &["Red", "Green", "Blue", "Alpha", "Lightness"]),
                num("height", "Height", 25.0, -100.0, 100.0, -100.0, 100.0),
                slider("softness", "Softness", 10.0, 0.0, 100.0, 1),
                angle("lightDirection", "Light Direction", -45.0),
                pct("lightIntensity", "Light Intensity", 100.0),
                color("lightColor", "Light Color", [1.0, 1.0, 1.0]),
                pct("ambient", "Ambient Light", 75.0),
                pct("reflection", "Reflection", 60.0),
                slider("roughness", "Roughness", 0.05, 0.0, 1.0, 3),
            ],
            render: plastic,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.ccRepeTile",
            name: "CC RepeTile",
            category: CAT,
            params: vec![num("expandRight", "Expand Right", 0.0, 0.0, 4000.0, 0.0, 1000.0), num("expandLeft", "Expand Left", 0.0, 0.0, 4000.0, 0.0, 1000.0), num("expandDown", "Expand Down", 0.0, 0.0, 4000.0, 0.0, 1000.0), num("expandUp", "Expand Up", 0.0, 0.0, 4000.0, 0.0, 1000.0), popup("tiling", "Tiling", 0, &["Repeat", "Checker Flip H", "Checker Flip V", "Checker Flip", "Unfold", "Twist"]), pct("blendBorders", "Blend Borders", 0.0)],
            render: repetile,
            pad: |c| c.f("expandRight").max(c.f("expandLeft")).max(c.f("expandDown")).max(c.f("expandUp")),
        },
        EffectSpec {
            id: "fx.stylize.ccVignette",
            name: "CC Vignette",
            category: CAT,
            params: vec![num("amount", "Amount", 50.0, -100.0, 100.0, -100.0, 100.0), pct("angleOfView", "Angle of View", 30.0), point("center", "Center", 0.5, 0.5), check("pinHighlights", "Pin Highlights", false)],
            render: cc_vignette,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.colorEmboss",
            name: "Color Emboss",
            category: CAT,
            params: vec![angle("direction", "Direction", 45.0), num("relief", "Relief", 1.0, 1.0, 10.0, 1.0, 10.0), num("contrast", "Contrast", 100.0, 0.0, 500.0, 0.0, 200.0), pct("blend", "Blend With Original", 0.0)],
            render: color_emboss,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.motionTile",
            name: "Motion Tile",
            category: CAT,
            params: vec![
                point("center", "Tile Center", 0.5, 0.5),
                num("tileWidth", "Tile Width", 100.0, 1.0, 1000.0, 1.0, 100.0),
                num("tileHeight", "Tile Height", 100.0, 1.0, 1000.0, 1.0, 100.0),
                num("outputWidth", "Output Width", 100.0, 1.0, 1000.0, 1.0, 400.0),
                num("outputHeight", "Output Height", 100.0, 1.0, 1000.0, 1.0, 400.0),
                check("mirrorEdges", "Mirror Edges", false),
                angle("phase", "Phase", 0.0),
                check("horizontalPhase", "Horizontal Phase Shift", false),
            ],
            render: motion_tile,
            pad: |c| {
                let ow = (c.f("outputWidth") - 100.0).max(0.0) / 100.0 * c.size[0] * 0.5;
                let oh = (c.f("outputHeight") - 100.0).max(0.0) / 100.0 * c.size[1] * 0.5;
                ow.max(oh)
            },
        },
        EffectSpec {
            id: "fx.stylize.roughenEdges",
            name: "Roughen Edges",
            category: CAT,
            params: vec![
                popup("edgeType", "Edge Type", 0, &["Roughen", "Roughen Color", "Cut", "Spiky", "Rusty", "Rusty Color", "Photocopy", "Photocopy Color"]),
                color("edgeColor", "Edge Color", [0.7, 0.3, 0.0]),
                num("border", "Border", 8.0, 0.0, 500.0, 0.0, 50.0),
                slider("edgeSharpness", "Edge Sharpness", 1.0, 0.0, 10.0, 2),
                slider("fractalInfluence", "Fractal Influence", 1.0, 0.0, 1.0, 2),
                num("scale", "Scale", 100.0, 10.0, 1000.0, 10.0, 300.0),
                angle("evolution", "Evolution", 0.0),
                slider("seed", "Random Seed", 0.0, 0.0, 10000.0, 0),
                slider("complexity", "Complexity", 2.0, 1.0, 10.0, 0),
            ],
            render: roughen,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.scatter",
            name: "Scatter",
            category: CAT,
            params: vec![num("amount", "Scatter Amount", 20.0, 0.0, 127.0, 0.0, 127.0), popup("grain", "Grain", 0, &["Both", "Horizontal", "Vertical"]), check("randomize", "Randomize Every Frame", false)],
            render: scatter,
            pad: |c| c.f("amount"),
        },
        EffectSpec {
            id: "fx.stylize.strobeLight",
            name: "Strobe Light",
            category: CAT,
            params: vec![
                color("color", "Strobe Color", [1.0, 1.0, 1.0]),
                pct("blend", "Blend With Original", 0.0),
                slider("duration", "Strobe Duration (secs)", 0.05, 0.0, 10.0, 3),
                slider("period", "Strobe Period (secs)", 0.1, 0.0, 10.0, 3),
                pct("probability", "Random Strobe Probability", 0.0),
                popup("strobe", "Strobe", 0, &["Operates On Color Only", "Makes Layer Transparent"]),
                popup("operator", "Strobe Operator", 0, &["Copy", "Add", "Subtract", "Multiply", "Difference", "Screen"]),
                slider("seed", "Random Seed", 0.0, 0.0, 10000.0, 0),
            ],
            render: strobe,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.texturize",
            name: "Texturize",
            category: CAT,
            params: vec![layer_param("textureLayer", "Texture Layer"), angle("lightDirection", "Light Direction", 135.0), slider("contrast", "Texture Contrast", 1.0, 0.0, 2.0, 2), popup("placement", "Texture Placement", 0, &["Tile Texture", "Center Texture", "Stretch Texture to Fit"])],
            render: texturize,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.perspective.threeDGlasses",
            name: "3D Glasses",
            category: PERSP,
            params: vec![layer_param("left", "Left View"), layer_param("right", "Right View"), slider("convergence", "Scene Convergence", 0.0, -1000.0, 1000.0, 1), slider("vertical", "Vertical Alignment", 0.0, -1000.0, 1000.0, 1), popup("view", "3D View", 2, &["Stereo Pair (Side by Side)", "Interlace Upper L Lower R", "Red Blue LR", "Red Green LR", "Balanced Red Blue LR", "Difference"]), check("swap", "Swap Left-Right", false)],
            render: glasses,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.perspective.bevelAlpha",
            name: "Bevel Alpha",
            category: PERSP,
            params: vec![num("edgeThickness", "Edge Thickness", 2.0, 0.0, 200.0, 0.0, 30.0), angle("lightAngle", "Light Angle", -60.0), color("lightColor", "Light Color", [1.0, 1.0, 1.0]), pct("lightIntensity", "Light Intensity", 40.0)],
            render: bevel_alpha,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.perspective.bevelEdges",
            name: "Bevel Edges",
            category: PERSP,
            params: vec![pct("edgeThickness", "Edge Thickness", 10.0), angle("lightAngle", "Light Angle", -60.0), color("lightColor", "Light Color", [1.0, 1.0, 1.0]), pct("lightIntensity", "Light Intensity", 40.0)],
            render: bevel_edges,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.perspective.ccCylinder",
            name: "CC Cylinder",
            category: PERSP,
            params: vec![pct("radius", "Radius (%)", 50.0), point("position", "Position", 0.5, 0.5), angle("rotationX", "Rotation X", 0.0), angle("rotationY", "Rotation Y", 0.0), angle("rotationZ", "Rotation Z", 0.0), popup("render", "Render", 0, &["Full", "Outside", "Inside"]), angle("lightDirection", "Light Direction", -45.0), pct("lightIntensity", "Light Intensity", 100.0), pct("ambient", "Ambient", 30.0)],
            render: cylinder,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.perspective.ccSphere",
            name: "CC Sphere",
            category: PERSP,
            params: vec![angle("rotationX", "Rotation X", 0.0), angle("rotationY", "Rotation Y", 0.0), angle("rotationZ", "Rotation Z", 0.0), num("radius", "Radius", 100.0, 1.0, 4000.0, 10.0, 1000.0), point("offset", "Offset", 0.5, 0.5), popup("render", "Render", 0, &["Full", "Outside", "Inside"]), angle("lightDirection", "Light Direction", -45.0), pct("lightIntensity", "Light Intensity", 100.0), pct("ambient", "Ambient", 30.0), pct("reflective", "Reflective", 0.0)],
            render: sphere,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.perspective.ccEnvironment",
            name: "CC Environment",
            category: PERSP,
            params: vec![layer_param("environment", "Environment"), popup("mapping", "Mapping", 0, &["Spherical", "Probe"]), angle("horizontalOffset", "Horizontal Offset", 0.0), check("filterEnvironment", "Filter Environment", true)],
            render: environment,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.perspective.ccSpotlight",
            name: "CC Spotlight",
            category: PERSP,
            params: vec![point("from", "From", 0.3, 0.3), point("to", "To", 0.5, 0.5), num("height", "Height", 50.0, 0.0, 400.0, 0.0, 200.0), angle("coneAngle", "Cone Angle", 25.0), pct("edgeSoftness", "Edge Softness", 50.0), color("color", "Color", [1.0, 1.0, 1.0]), pct("intensity", "Intensity", 100.0), popup("render", "Render", 1, &["Light Only", "Light", "Light & Shadow"])],
            render: spotlight,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.perspective.radialShadow",
            name: "Radial Shadow",
            category: PERSP,
            params: vec![color("color", "Shadow Color", [0.0, 0.0, 0.0]), pct("opacity", "Opacity", 50.0), point("lightSource", "Light Source", 0.3, 0.2), num("distance", "Projection Distance", 10.0, 1.0, 100.0, 1.0, 100.0), num("softness", "Softness", 0.0, 0.0, 1000.0, 0.0, 100.0), popup("render", "Render", 0, &["Regular", "Glass Edge"]), check("shadowOnly", "Shadow Only", false)],
            render: radial_shadow,
            pad: |c| c.f("distance") * 4.0 + c.f("softness"),
        },
    ]
}

fn heights(img: &Img, prop: u32) -> Vec<f32> {
    img.px
        .chunks_exact(4)
        .map(|p| {
            let a = p[3];
            if a <= 1e-6 {
                return 0.0;
            }
            match prop {
                0 => p[0] / a * a,
                1 => p[1] / a * a,
                2 => p[2] / a * a,
                3 => a,
                _ => lum(p[0] / a, p[1] / a, p[2] / a) * a,
            }
        })
        .collect()
}

fn grad(hf: &[f32], w: usize, h: usize, x: usize, y: usize) -> (f32, f32) {
    let g = |xx: usize, yy: usize| hf[yy.min(h - 1) * w + xx.min(w - 1)];
    (g(x + 1, y) - g(x.saturating_sub(1), y), g(x, y + 1) - g(x, y.saturating_sub(1)))
}

fn brush_strokes(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let a = c.f("angle").to_radians();
    let size = c.px(c.f("size")).max(0.5);
    let len = c.px(c.f("length")).max(0.5);
    let dens = c.f("density") / 100.0;
    let rnd = c.f("randomness");
    let src = img.clone();
    let mut out = match c.e("surface") {
        1 => Img::new(w, h),
        2 => Img::filled(w, h, [1.0, 1.0, 1.0, 1.0]),
        3 => Img::filled(w, h, [0.0, 0.0, 0.0, 1.0]),
        _ => src.clone(),
    };
    let step = (size * 1.5 / dens.max(0.05)).max(1.0);
    let cols = (w as f64 / step).ceil() as i32;
    let rows = (h as f64 / step).ceil() as i32;
    let mut strokes = vec![];
    for j in 0..rows {
        for i in 0..cols {
            let jx = (rand01(i, j, 0, 3) as f64 - 0.5) * step * rnd;
            let jy = (rand01(i, j, 1, 3) as f64 - 0.5) * step * rnd;
            let p = [(i as f64 + 0.5) * step + jx, (j as f64 + 0.5) * step + jy];
            let aa = a + (rand01(i, j, 2, 3) as f64 - 0.5) * 0.6 * rnd;
            let l = len * (0.6 + 0.8 * rand01(i, j, 4, 3) as f64);
            let col = src.sample(p[0], p[1], true);
            strokes.push((p, aa, l, col));
        }
    }
    let bw = w;
    let rows_px: Vec<(usize, Vec<f32>)> = (0..h)
        .into_par_iter()
        .map(|y| {
            let mut row = out.px[y * bw * 4..(y + 1) * bw * 4].to_vec();
            let py = y as f64 + 0.5;
            for (p, aa, l, col) in &strokes {
                if (py - p[1]).abs() > l + size {
                    continue;
                }
                let (dx, dy) = (aa.cos(), aa.sin());
                let a0 = [p[0] - dx * l * 0.5, p[1] - dy * l * 0.5];
                let a1 = [p[0] + dx * l * 0.5, p[1] + dy * l * 0.5];
                let x0 = (p[0] - l - size).floor().max(0.0) as usize;
                let x1 = ((p[0] + l + size).ceil() as usize).min(bw.saturating_sub(1));
                for x in x0..=x1 {
                    let d = super::draw::seg_dist([x as f64 + 0.5, py], a0, a1);
                    if d < size {
                        let k = (1.0 - d / size).min(1.0) as f32;
                        for i in 0..4 {
                            row[x * 4 + i] += (col[i] - row[x * 4 + i]) * k;
                        }
                    }
                }
            }
            (y, row)
        })
        .collect();
    for (y, r) in rows_px {
        out.px[y * bw * 4..(y + 1) * bw * 4].copy_from_slice(&r);
    }
    let blend = c.f("blend") as f32 / 100.0;
    img.px.par_iter_mut().zip(out.px.par_iter()).for_each(|(d, o)| *d = o + (*d - o) * blend);
}

fn cartoon(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let mut fill = img.clone();
    let r = c.px(c.f("detailRadius"));
    if r > 0.3 {
        let th = c.f("detailThreshold") as f32 / 100.0;
        let src = img.clone();
        gaussian(&mut fill, r * 0.5, r * 0.5, true);
        fill.px.par_chunks_mut(4).zip(src.px.par_chunks(4)).for_each(|(f, s)| {
            let d = (lum(f[0], f[1], f[2]) - lum(s[0], s[1], s[2])).abs();
            if d > th * 0.5 {
                f.copy_from_slice(s);
            }
        });
    }
    let steps = c.f("shadingSteps").max(1.0) as f32;
    let smoothv = c.f("shadingSmoothness") as f32 / 100.0;
    fill.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let u = [p[0] / a, p[1] / a, p[2] / a];
        let l = lum(u[0], u[1], u[2]);
        let q = (l * steps).floor() / steps + 0.5 / steps;
        let f = l * steps - (l * steps).floor();
        let t = smooth(0.5 - smoothv * 0.5, 0.5 + smoothv * 0.5, f) / steps;
        let nl = q - 0.5 / steps + t;
        let k = if l > 1e-4 { nl / l } else { 1.0 };
        for i in 0..3 {
            p[i] = (u[i] * k).clamp(0.0, 1.0) * a;
        }
    });
    let lv: Vec<f32> = img.px.chunks_exact(4).map(|p| lum(p[0], p[1], p[2])).collect();
    let et = c.f("edgeThreshold") as f32 * 0.1;
    let ew = c.px(c.f("edgeWidth"));
    let mut edge: Vec<f32> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let (gx, gy) = grad(&lv, w, h, x, y);
            if (gx * gx + gy * gy).sqrt() > et { 1.0 } else { 0.0 }
        })
        .collect();
    if ew > 1.0 {
        edge = super::util::minmax(&edge, w, h, (ew * 0.5).round() as usize, true);
    }
    let soft = c.f("edgeSoftness") / 100.0;
    if soft > 0.0 {
        super::util::blur_scalar(&mut edge, w, h, soft * 1.2, soft * 1.2);
    }
    let eo = c.f("edgeOpacity") as f32 / 100.0;
    let mode = c.e("render");
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let f = &fill.px[i * 4..i * 4 + 4];
        let e = edge[i] * eo;
        match mode {
            1 => p.copy_from_slice(f),
            2 => {
                let v = 1.0 - e;
                p[0] = v * p[3];
                p[1] = v * p[3];
                p[2] = v * p[3];
            }
            _ => {
                for k in 0..3 {
                    p[k] = f[k] * (1.0 - e);
                }
                p[3] = f[3];
            }
        }
    });
}

fn block_load(c: &FxCtx, img: &mut Img) {
    let comp = (c.f("completion") / 100.0).clamp(0.0, 1.0);
    if comp >= 1.0 {
        return;
    }
    let (w, h) = (img.w, img.h);
    let passes = 6;
    let pass = (comp * passes as f64).floor() as usize;
    let within = comp * passes as f64 - pass as f64;
    let size = (1usize << (passes - pass.min(passes - 1))).max(1);
    let scan = c.b("scanlining");
    let cleared = c.f("startCleared") >= 0.5;
    let src = img.clone();
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        let by = (y / size) * size;
        let row_done = (by as f64 / h as f64) < within || !scan;
        for x in 0..w {
            let bx = (x / size) * size;
            let s = src.get((bx + size / 2).min(w - 1) as isize, (by + size / 2).min(h - 1) as isize);
            let v = if pass == 0 && !row_done && cleared { [0.0; 4] } else if row_done { s } else {
                let ps = size * 2;
                let (px, py) = ((x / ps) * ps, (y / ps) * ps);
                if pass == 0 && cleared { [0.0; 4] } else { src.get((px + ps / 2).min(w - 1) as isize, (py + ps / 2).min(h - 1) as isize) }
            };
            row[x * 4..x * 4 + 4].copy_from_slice(&v);
        }
    });
}

fn burn_film(c: &FxCtx, img: &mut Img) {
    let burn = c.f("burn") / 100.0;
    if burn <= 0.0 {
        return;
    }
    let ctr = c.pt("center");
    let seed = c.f("seed") as u32;
    let rect = c.layer_rect();
    let diag = (rect[2] - rect[0]).hypot(rect[3] - rect[1]).max(1.0);
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (dx, dy) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let d = dx.hypot(dy) / diag;
            let n = perlin3((x as f64 / diag * 12.0) as f32, (y as f64 / diag * 12.0) as f32, 0.0, seed) as f64 * 0.12;
            let r = burn * 1.2 - d + n;
            let p = &mut row[x * 4..x * 4 + 4];
            if r > 0.0 {
                p.iter_mut().for_each(|v| *v = 0.0);
            } else if r > -0.04 {
                let t = (1.0 + r / 0.04) as f32;
                p[0] = p[0] * (1.0 - t) + t * p[3] * 1.0;
                p[1] = p[1] * (1.0 - t) + t * p[3] * 0.45;
                p[2] *= 1.0 - t;
            } else if r > -0.12 {
                let t = (1.0 + (r + 0.04) / 0.08) as f32;
                for k in 0..3 {
                    p[k] *= 1.0 - t * 0.7;
                }
            }
        }
    });
}

fn cc_glass(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let bump = c.other_fit("bumpMap", true, w, h, 1).unwrap_or_else(|| img.clone());
    let mut hf = heights(&bump, c.e("property"));
    let s = c.f("softness") / 100.0 * 10.0 * c.r;
    if s > 0.3 {
        super::util::blur_scalar(&mut hf, w, h, s, s);
    }
    let height = c.f("height") as f32 / 100.0;
    let disp = c.px(c.f("displacement")) as f32 / 100.0 * 20.0;
    let li = c.f("lightIntensity") as f32 / 100.0;
    let lc = c.c("lightColor");
    let la = c.f("lightDirection").to_radians();
    let (lx, ly) = (la.cos() as f32, la.sin() as f32);
    let amb = c.f("ambient") as f32 / 100.0;
    let src = img.clone();
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (gx, gy) = grad(&hf, w, h, x, y);
            let (gx, gy) = (gx * height, gy * height);
            let s = src.sample(x as f64 + 0.5 + (gx * disp) as f64, y as f64 + 0.5 + (gy * disp) as f64, true);
            let shade = (-(gx * lx + gy * ly) * 8.0).max(0.0);
            let spec = shade.powi(3) * li;
            let diffuse = amb + (1.0 - amb) * (0.5 + shade * 0.5).min(1.0);
            let p = &mut row[x * 4..x * 4 + 4];
            for k in 0..3 {
                p[k] = s[k] * diffuse + lc[k] * spec * s[3];
            }
            p[3] = s[3];
        }
    });
}

fn hex_tile(c: &FxCtx, img: &mut Img) {
    let r = c.px(c.f("radius")).max(1.0);
    let ctr = c.pt("center");
    let rot = c.f("rotate").to_radians();
    let smear = c.f("smearing") / 100.0;
    let (s, co) = rot.sin_cos();
    remap(img, true, |x, y| {
        let (dx, dy) = (x - ctr[0], y - ctr[1]);
        let (u, v) = (dx * co + dy * s, -dx * s + dy * co);
        let q = (u * 3f64.sqrt() / 3.0 - v / 3.0) / r;
        let rr = v * 2.0 / 3.0 / r;
        let (mut rx, mut ry, rz) = (q.round(), rr.round(), (-q - rr).round());
        let (dxq, dyr, dzs) = ((rx - q).abs(), (ry - rr).abs(), (rz - (-q - rr)).abs());
        if dxq > dyr && dxq > dzs {
            rx = -ry - rz;
        } else if dyr > dzs {
            ry = -rx - rz;
        }
        let cu = r * 3f64.sqrt() * (rx + ry / 2.0);
        let cv = r * 1.5 * ry;
        let (lu, lv) = (u - cu, v - cv);
        let su = cu * smear + lu;
        let sv = cv * smear + lv;
        Some([ctr[0] + su * co - sv * s, ctr[1] + su * s + sv * co])
    });
}

fn kaleida(c: &FxCtx, img: &mut Img) {
    let ctr = c.pt("center");
    let size = c.px(c.f("size")).max(1.0);
    let rot = c.f("rotation").to_radians();
    let mode = c.e("mirroring");
    let n = match mode {
        0 => 2.0,
        1 => 6.0,
        2 => 4.0,
        3 => 5.0,
        4 => 4.0,
        5 => 8.0,
        _ => 6.0,
    };
    let seg = TAU / n;
    remap(img, true, |x, y| {
        let (dx, dy) = (x - ctr[0], y - ctr[1]);
        let r = dx.hypot(dy);
        let mut a = dy.atan2(dx) - rot;
        a = a.rem_euclid(seg);
        if a > seg * 0.5 && mode != 2 {
            a = seg - a;
        }
        let rr = if mode == 4 { r.rem_euclid(size * 2.0) } else { r };
        let rr = if mode == 4 && rr > size { size * 2.0 - rr } else { rr };
        Some([ctr[0] + (a + rot).cos() * rr, ctr[1] + (a + rot).sin() * rr])
    });
}

fn mr_smoothie(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let prop = c.e("property");
    let sm = c.f("smoothness") * c.r;
    let mut field = heights(img, prop.min(4));
    if prop == 5 || prop == 6 {
        field = img.px.chunks_exact(4).map(|p| if p[3] > 1e-6 { super::util::rgb_to_hsl([p[0] / p[3], p[1] / p[3], p[2] / p[3]])[if prop == 5 { 0 } else { 1 }] } else { 0.0 }).collect();
    }
    if sm > 0.3 {
        super::util::blur_scalar(&mut field, w, h, sm, sm);
    }
    let (a, b) = (c.pt("sampleA"), c.pt("sampleB"));
    let n = 64;
    let pal: Vec<[f32; 4]> = (0..n).map(|i| {
        let t = i as f64 / (n - 1) as f64;
        img.sample(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, true)
    }).collect();
    let ph = c.f("phase") as f32 / 360.0;
    let blend = c.f("blend") as f32 / 100.0;
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let t = (field[i] + ph).rem_euclid(1.0) * (n - 1) as f32;
        let (i0, f) = (t.floor() as usize, t - t.floor());
        let i1 = (i0 + 1).min(n - 1);
        for k in 0..4 {
            let v = pal[i0][k] + (pal[i1][k] - pal[i0][k]) * f;
            p[k] = v + (p[k] - v) * blend;
        }
    });
}

fn plastic(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let bump = c.other_fit("bumpLayer", true, w, h, 1).unwrap_or_else(|| img.clone());
    let mut hf = heights(&bump, c.e("property"));
    let s = c.f("softness") / 100.0 * 10.0 * c.r;
    if s > 0.3 {
        super::util::blur_scalar(&mut hf, w, h, s, s);
    }
    let height = c.f("height") as f32 / 100.0 * 8.0;
    let la = c.f("lightDirection").to_radians();
    let (lx, ly) = (la.cos() as f32, la.sin() as f32);
    let li = c.f("lightIntensity") as f32 / 100.0;
    let lc = c.c("lightColor");
    let amb = c.f("ambient") as f32 / 100.0;
    let refl = c.f("reflection") as f32 / 100.0;
    let rough = (c.f("roughness") as f32).max(0.005);
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (gx, gy) = grad(&hf, w, h, x, y);
            let (nx, ny) = (-gx * height, -gy * height);
            let nl = (nx * nx + ny * ny + 1.0).sqrt();
            let ndl = ((nx * -lx + ny * -ly + 0.7) / nl / 1.22).clamp(0.0, 1.0);
            let spec = ndl.powf(1.0 / rough) * refl * li;
            let p = &mut row[x * 4..x * 4 + 4];
            let a = p[3];
            for k in 0..3 {
                p[k] = p[k] * (amb + (1.0 - amb) * ndl * li) + lc[k] * spec * a;
            }
        }
    });
}

fn repetile(c: &FxCtx, img: &mut Img) {
    let rect = c.layer_rect();
    let (lw, lh) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    let tiling = c.e("tiling");
    let (r, l, d, u) = (c.px(c.f("expandRight")), c.px(c.f("expandLeft")), c.px(c.f("expandDown")), c.px(c.f("expandUp")));
    remap(img, true, |x, y| {
        if x < rect[0] - l || x > rect[2] + r || y < rect[1] - u || y > rect[3] + d {
            return None;
        }
        let (fx, fy) = ((x - rect[0]) / lw, (y - rect[1]) / lh);
        let (ix, iy) = (fx.floor() as i64, fy.floor() as i64);
        let (mut tx, mut ty) = (fx - ix as f64, fy - iy as f64);
        let flip_h = matches!(tiling, 1 | 3) && (ix + iy).rem_euclid(2) == 1 || tiling == 4 && ix.rem_euclid(2) == 1;
        let flip_v = matches!(tiling, 2 | 3) && (ix + iy).rem_euclid(2) == 1 || tiling == 4 && iy.rem_euclid(2) == 1;
        if flip_h {
            tx = 1.0 - tx;
        }
        if flip_v {
            ty = 1.0 - ty;
        }
        if tiling == 5 && (ix + iy).rem_euclid(2) == 1 {
            let t = tx;
            tx = 1.0 - ty;
            ty = t;
        }
        Some([rect[0] + tx * lw, rect[1] + ty * lh])
    });
}

fn cc_vignette(c: &FxCtx, img: &mut Img) {
    let amt = c.f("amount") as f32 / 100.0;
    let aov = (c.f("angleOfView") / 100.0).max(0.01);
    let ctr = c.pt("center");
    let pin = c.b("pinHighlights");
    let rect = c.layer_rect();
    let diag = ((rect[2] - rect[0]).hypot(rect[3] - rect[1]) * 0.5).max(1.0);
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let d = (x as f64 + 0.5 - ctr[0]).hypot(y as f64 + 0.5 - ctr[1]) / diag;
            let fall = (d * aov * 1.6).atan().cos().powi(4) as f32;
            let k = 1.0 + (fall - 1.0) * amt;
            let p = &mut row[x * 4..x * 4 + 4];
            for i in 0..3 {
                let v = p[i] * k;
                p[i] = if pin && p[i] > p[3] * 0.9 { p[i] } else { v.max(0.0) };
            }
        }
    });
}

fn color_emboss(c: &FxCtx, img: &mut Img) {
    let a = c.f("direction").to_radians();
    let rel = c.px(c.f("relief")).max(0.5);
    let (dx, dy) = (a.cos() * rel, -a.sin() * rel);
    let con = c.f("contrast") as f32 / 100.0;
    let blend = c.f("blend") as f32 / 100.0;
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let s1 = src.sample(px + dx, py + dy, true);
            let s2 = src.sample(px - dx, py - dy, true);
            let p = &mut row[x * 4..x * 4 + 4];
            let o = [p[0], p[1], p[2]];
            for k in 0..3 {
                let v = (0.5 * p[3] + (s1[k] - s2[k]) * con).clamp(0.0, p[3].max(0.0));
                p[k] = v + (o[k] - v) * blend;
            }
        }
    });
}

fn motion_tile(c: &FxCtx, img: &mut Img) {
    let rect = c.layer_rect();
    let ctr = c.pt("center");
    let (tw, th) = ((rect[2] - rect[0]) * c.f("tileWidth") / 100.0, (rect[3] - rect[1]) * c.f("tileHeight") / 100.0);
    let (tw, th) = (tw.max(1.0), th.max(1.0));
    let (ow, oh) = ((rect[2] - rect[0]) * c.f("outputWidth") / 100.0, (rect[3] - rect[1]) * c.f("outputHeight") / 100.0);
    let mid = [(rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5];
    let mirror = c.b("mirrorEdges");
    let phase = c.f("phase") / 360.0;
    let hph = c.b("horizontalPhase");
    remap(img, true, |x, y| {
        if (x - mid[0]).abs() > ow * 0.5 || (y - mid[1]).abs() > oh * 0.5 {
            return None;
        }
        let (mut u, mut v) = ((x - ctr[0]) / tw + 0.5, (y - ctr[1]) / th + 0.5);
        if hph {
            v += phase * (u.floor().rem_euclid(2.0));
        } else {
            u += phase * (v.floor().rem_euclid(2.0));
        }
        let (iu, iv) = (u.floor(), v.floor());
        let (mut fu, mut fv) = (u - iu, v - iv);
        if mirror {
            if iu.rem_euclid(2.0) == 1.0 {
                fu = 1.0 - fu;
            }
            if iv.rem_euclid(2.0) == 1.0 {
                fv = 1.0 - fv;
            }
        }
        Some([ctr[0] + (fu - 0.5) * tw, ctr[1] + (fv - 0.5) * th])
    });
}

fn roughen(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let ty = c.e("edgeType");
    let border = c.px(c.f("border")).max(0.5);
    let sharp = c.f("edgeSharpness") as f32;
    let infl = c.f("fractalInfluence") as f32;
    let scale = c.px(c.f("scale")).max(1.0) * 0.25;
    let evo = (c.f("evolution") / 360.0) as f32;
    let seed = c.f("seed") as u32;
    let oct = c.f("complexity").round().clamp(1.0, 10.0) as usize;
    let alpha: Vec<f32> = img.px.chunks_exact(4).map(|p| p[3]).collect();
    let inside = crate::render::styles::distance(&alpha, w, h, false);
    let col = c.c("edgeColor");
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let i = y * w + x;
            if alpha[i] <= 0.0 {
                continue;
            }
            let mut n = 0.0f32;
            let mut amp = 1.0f32;
            let mut f = 1.0f32;
            for o in 0..oct {
                n += value3(x as f32 / scale as f32 * f, y as f32 / scale as f32 * f, evo * 4.0 + o as f32 * 7.0, seed, true) * amp;
                amp *= 0.5;
                f *= 2.0;
            }
            if ty == 3 {
                n = n.abs() * 2.0 - 0.5;
            }
            let d = inside[i] / border as f32;
            let v = d + n * infl - 0.5;
            let m = ((v * (1.0 + sharp * 4.0)) + 0.5).clamp(0.0, 1.0);
            let p = &mut row[x * 4..x * 4 + 4];
            let edge_zone = (1.0 - d).clamp(0.0, 1.0);
            let colored = matches!(ty, 1 | 5 | 7);
            if colored && edge_zone > 0.0 {
                for k in 0..3 {
                    p[k] += (col[k] * p[3] - p[k]) * edge_zone * 0.6;
                }
            }
            if matches!(ty, 4 | 5) {
                let rust = (value3(x as f32 / 6.0, y as f32 / 6.0, 3.0, seed, true) * 0.5 + 0.5) * edge_zone;
                for k in 0..3 {
                    p[k] *= 1.0 - rust * 0.5;
                }
            }
            if matches!(ty, 6 | 7) {
                let g = lum(p[0], p[1], p[2]);
                let t = if g > p[3] * 0.5 { 1.0 } else { 0.0 };
                if ty == 6 {
                    p[0] = t * p[3];
                    p[1] = t * p[3];
                    p[2] = t * p[3];
                }
            }
            for v in p.iter_mut() {
                *v *= m;
            }
        }
    });
}

fn scatter(c: &FxCtx, img: &mut Img) {
    let amt = c.px(c.f("amount"));
    if amt < 0.5 {
        return;
    }
    let grain = c.e("grain");
    let seed = if c.b("randomize") { (c.t * c.fps).round() as u32 + 1 } else { 1 };
    remap(img, true, |x, y| {
        let (xi, yi) = (x as i32, y as i32);
        let dx = if grain == 2 { 0.0 } else { (rand01(xi, yi, 0, seed) as f64 - 0.5) * amt };
        let dy = if grain == 1 { 0.0 } else { (rand01(xi, yi, 1, seed) as f64 - 0.5) * amt };
        Some([x + dx, y + dy])
    });
}

fn strobe(c: &FxCtx, img: &mut Img) {
    let period = c.f("period");
    let dur = c.f("duration");
    if period <= 0.0 || dur <= 0.0 {
        return;
    }
    let t = c.t;
    let k = (t / period).floor() as i64;
    let within = t - k as f64 * period;
    let prob = c.f("probability") / 100.0;
    let seed = c.f("seed") as u32;
    let on = within < dur && (prob <= 0.0 || (rand01(k as i32, 0, 0, seed) as f64) < prob.max(1e-9) || prob == 0.0);
    if !on {
        return;
    }
    let col = c.c("color");
    let blend = c.f("blend") as f32 / 100.0;
    if c.e("strobe") == 1 {
        img.px.iter_mut().for_each(|v| *v *= blend);
        return;
    }
    let op = c.e("operator");
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        for i in 0..3 {
            let u = p[i] / a;
            let v = match op {
                1 => u + col[i],
                2 => u - col[i],
                3 => u * col[i],
                4 => (u - col[i]).abs(),
                5 => 1.0 - (1.0 - u) * (1.0 - col[i]),
                _ => col[i],
            };
            p[i] = (v.clamp(0.0, 1.0) + (u - v.clamp(0.0, 1.0)) * blend) * a;
        }
    });
}

fn texturize(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let mode = match c.e("placement") {
        0 => 2,
        1 => 0,
        _ => 1,
    };
    let Some(tex) = c.other_fit("textureLayer", true, w, h, mode) else { return };
    let hf: Vec<f32> = tex.px.chunks_exact(4).map(|p| lum(p[0], p[1], p[2])).collect();
    let la = c.f("lightDirection").to_radians();
    let (lx, ly) = (la.cos() as f32, -la.sin() as f32);
    let con = c.f("contrast") as f32;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (gx, gy) = grad(&hf, w, h, x, y);
            let k = 1.0 + (gx * lx + gy * ly) * con * 2.0;
            let p = &mut row[x * 4..x * 4 + 4];
            for i in 0..3 {
                p[i] = (p[i] * k).clamp(0.0, p[3]);
            }
        }
    });
}

fn glasses(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let left = c.other_fit("left", true, w, h, 0).unwrap_or_else(|| img.clone());
    let right = c.other_fit("right", true, w, h, 0).unwrap_or_else(|| img.clone());
    let (mut l, mut r) = (left, right);
    if c.b("swap") {
        std::mem::swap(&mut l, &mut r);
    }
    let conv = c.px(c.f("convergence"));
    let vert = c.px(c.f("vertical"));
    let view = c.e("view");
    let lw = w as f64;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let a = l.sample(px - conv * 0.5, py, false);
            let b = r.sample(px + conv * 0.5, py + vert, false);
            let gl = lum(a[0], a[1], a[2]);
            let gr = lum(b[0], b[1], b[2]);
            let o: [f32; 4] = match view {
                0 => {
                    if px < lw * 0.5 { l.sample(px * 2.0, py, false) } else { r.sample((px - lw * 0.5) * 2.0, py, false) }
                }
                1 => if y % 2 == 0 { a } else { b },
                3 => [gl, gr, 0.0, a[3].max(b[3])],
                4 => [a[0] * 0.7 + gl * 0.3, 0.0, b[2] * 0.7 + gr * 0.3, a[3].max(b[3])],
                5 => [(a[0] - b[0]).abs(), (a[1] - b[1]).abs(), (a[2] - b[2]).abs(), a[3].max(b[3])],
                _ => [gl, 0.0, gr, a[3].max(b[3])],
            };
            row[x * 4..x * 4 + 4].copy_from_slice(&o);
        }
    });
}

fn bevel_shade(img: &mut Img, hf: &[f32], light_angle: f64, color: [f32; 4], intensity: f32) {
    let (w, h) = (img.w, img.h);
    let (lx, ly) = (light_angle.cos() as f32, light_angle.sin() as f32);
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (gx, gy) = grad(hf, w, h, x, y);
            let s = -(gx * lx + gy * ly) * 2.0;
            let p = &mut row[x * 4..x * 4 + 4];
            let a = p[3];
            if a <= 1e-6 {
                continue;
            }
            let k = (s * intensity * 4.0).clamp(-1.0, 1.0);
            for i in 0..3 {
                if k > 0.0 {
                    p[i] += (color[i] * a - p[i]) * k;
                } else {
                    p[i] *= 1.0 + k;
                }
            }
        }
    });
}

fn bevel_alpha(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let th = c.px(c.f("edgeThickness")).max(0.5);
    let alpha: Vec<f32> = img.px.chunks_exact(4).map(|p| p[3]).collect();
    let d = crate::render::styles::distance(&alpha, w, h, false);
    let hf: Vec<f32> = d.iter().map(|v| (v / th as f32).min(1.0)).collect();
    bevel_shade(img, &hf, c.f("lightAngle").to_radians(), c.c("lightColor"), c.f("lightIntensity") as f32 / 100.0);
}

fn bevel_edges(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let rect = c.layer_rect();
    let th = ((rect[2] - rect[0]).min(rect[3] - rect[1]) * c.f("edgeThickness") / 100.0 * 0.5).max(0.5);
    let hf: Vec<f32> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
            let d = (x - rect[0]).min(rect[2] - x).min(y - rect[1]).min(rect[3] - y);
            (d / th).clamp(0.0, 1.0) as f32
        })
        .collect();
    bevel_shade(img, &hf, c.f("lightAngle").to_radians(), c.c("lightColor"), c.f("lightIntensity") as f32 / 100.0);
}

fn rot3(v: [f64; 3], rx: f64, ry: f64, rz: f64) -> [f64; 3] {
    let (sx, cx) = rx.sin_cos();
    let (sy, cy) = ry.sin_cos();
    let (sz, cz) = rz.sin_cos();
    let v = [v[0], v[1] * cx - v[2] * sx, v[1] * sx + v[2] * cx];
    let v = [v[0] * cy + v[2] * sy, v[1], -v[0] * sy + v[2] * cy];
    [v[0] * cz - v[1] * sz, v[0] * sz + v[1] * cz, v[2]]
}

fn cylinder(c: &FxCtx, img: &mut Img) {
    let rect = c.layer_rect();
    let (lw, lh) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    let rad = lw * c.f("radius") / 100.0 * 0.5 / PI * 2.0;
    let pos = c.pt("position");
    let (rx, ry, rz) = (c.f("rotationX").to_radians(), c.f("rotationY").to_radians(), c.f("rotationZ").to_radians());
    let mode = c.e("render");
    let la = c.f("lightDirection").to_radians();
    let li = c.f("lightIntensity") as f32 / 100.0;
    let amb = c.f("ambient") as f32 / 100.0;
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let p = [x as f64 + 0.5 - pos[0], y as f64 + 0.5 - pos[1], 0.0];
            let d = rot3(p, -rx, 0.0, -rz);
            let mut best: Option<[f32; 4]> = None;
            if d[0].abs() < rad {
                let zz = (rad * rad - d[0] * d[0]).sqrt();
                for (zs, front) in [(-zz, true), (zz, false)] {
                    if (mode == 1 && !front) || (mode == 2 && front) {
                        continue;
                    }
                    let ang = d[0].atan2(-zs) + ry;
                    let u = (ang / TAU + 0.5).rem_euclid(1.0);
                    let v = (d[1] / lh) + 0.5;
                    if !(0.0..=1.0).contains(&v) {
                        continue;
                    }
                    let s = src.sample(rect[0] + u * lw, rect[1] + v * lh, true);
                    if s[3] <= 1e-4 {
                        continue;
                    }
                    let nx = d[0] / rad;
                    let shade = amb + (1.0 - amb) * ((nx * la.cos() + (-zs / rad) * 0.7).max(0.0) as f32) * li;
                    let sh = if front { shade } else { shade * 0.6 };
                    best = Some([s[0] * sh, s[1] * sh, s[2] * sh, s[3]]);
                    break;
                }
            }
            let o = best.unwrap_or([0.0; 4]);
            row[x * 4..x * 4 + 4].copy_from_slice(&o);
        }
    });
}

fn sphere(c: &FxCtx, img: &mut Img) {
    let rect = c.layer_rect();
    let (lw, lh) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    let rad = c.px(c.f("radius")).max(1.0);
    let off = c.pt("offset");
    let (rx, ry, rz) = (c.f("rotationX").to_radians(), c.f("rotationY").to_radians(), c.f("rotationZ").to_radians());
    let mode = c.e("render");
    let la = c.f("lightDirection").to_radians();
    let (lx, ly) = (la.cos(), la.sin());
    let li = c.f("lightIntensity") as f32 / 100.0;
    let amb = c.f("ambient") as f32 / 100.0;
    let refl = c.f("reflective") as f32 / 100.0;
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (dx, dy) = ((x as f64 + 0.5 - off[0]) / rad, (y as f64 + 0.5 - off[1]) / rad);
            let r2 = dx * dx + dy * dy;
            if r2 >= 1.0 {
                row[x * 4..x * 4 + 4].copy_from_slice(&[0.0; 4]);
                continue;
            }
            let dz = (1.0 - r2).sqrt();
            let side = if mode == 2 { [dx, dy, dz] } else { [dx, dy, -dz] };
            let n = rot3(side, rx, ry, rz);
            let u = (n[0].atan2(-n[2]) / TAU + 0.5).rem_euclid(1.0);
            let v = (n[1].clamp(-1.0, 1.0)).asin() / PI + 0.5;
            let s = src.sample(rect[0] + u * lw, rect[1] + v * lh, true);
            let ndl = (dx * lx + dy * ly + dz * 0.8).max(0.0) as f32;
            let shade = amb + (1.0 - amb) * ndl * li;
            let spec = ndl.powf(30.0) * refl;
            let o = [s[0] * shade + spec * s[3], s[1] * shade + spec * s[3], s[2] * shade + spec * s[3], s[3]];
            row[x * 4..x * 4 + 4].copy_from_slice(&o);
        }
    });
}

fn environment(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let Some((env, esize)) = c.other("environment", true) else { return };
    let probe = c.e("mapping") == 1;
    let hoff = c.f("horizontalOffset") / 360.0;
    let alpha: Vec<f32> = img.px.chunks_exact(4).map(|p| p[3]).collect();
    let mut hf = alpha.clone();
    super::util::blur_scalar(&mut hf, w, h, 3.0 * c.r, 3.0 * c.r);
    let r = c.r;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (gx, gy) = grad(&hf, w, h, x, y);
            let n = [-gx as f64 * 4.0, -gy as f64 * 4.0, 1.0];
            let l = (n[0] * n[0] + n[1] * n[1] + 1.0).sqrt();
            let (nx, ny, nz) = (n[0] / l, n[1] / l, n[2] / l);
            let (u, v) = if probe { (0.5 + nx * 0.5, 0.5 + ny * 0.5) } else { ((nx.atan2(nz) / TAU + 0.5 + hoff).rem_euclid(1.0), ny.asin() / PI + 0.5) };
            let s = env.sample(u * esize[0] * r, v * esize[1] * r, true);
            let p = &mut row[x * 4..x * 4 + 4];
            let a = p[3];
            for k in 0..3 {
                p[k] = if s[3] > 1e-6 { s[k] / s[3] * a } else { 0.0 };
            }
        }
    });
}

fn spotlight(c: &FxCtx, img: &mut Img) {
    let from = c.pt("from");
    let to = c.pt("to");
    let hgt = c.px(c.f("height")).max(1.0);
    let cone = c.f("coneAngle").to_radians().max(0.01);
    let soft = c.f("edgeSoftness") / 100.0;
    let col = c.c("color");
    let int = c.f("intensity") as f32 / 100.0;
    let mode = c.e("render");
    let axis = [to[0] - from[0], to[1] - from[1], -hgt];
    let al = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let v = [x as f64 + 0.5 - from[0], y as f64 + 0.5 - from[1], -hgt];
            let vl = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            let ca = (v[0] * axis[0] + v[1] * axis[1] + v[2] * axis[2]) / (vl * al);
            let ang = ca.clamp(-1.0, 1.0).acos();
            let k = (1.0 - smooth((cone * (1.0 - soft)) as f32, cone as f32 + 1e-4, ang as f32)) * int;
            let p = &mut row[x * 4..x * 4 + 4];
            match mode {
                0 => {
                    for i in 0..3 {
                        p[i] = col[i] * k;
                    }
                    p[3] = k.min(1.0);
                }
                2 => {
                    let s = src.sample(x as f64 + 0.5 + (x as f64 - from[0]) * 0.05, y as f64 + 0.5 + (y as f64 - from[1]) * 0.05, false);
                    let sh = 1.0 - s[3] * 0.5 * (1.0 - p[3]);
                    for i in 0..3 {
                        p[i] = p[i] * col[i] * k * sh;
                    }
                }
                _ => {
                    for i in 0..3 {
                        p[i] *= col[i] * k;
                    }
                }
            }
        }
    });
}

fn radial_shadow(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let ls = c.pt("lightSource");
    let dist = c.f("distance") / 10.0;
    let soft = c.px(c.f("softness"));
    let col = c.c("color");
    let op = c.f("opacity") as f32 / 100.0;
    let glass = c.e("render") == 1;
    let src = img.clone();
    let mut sh = Img::new(w, h);
    let k = 1.0 + dist * 0.15;
    sh.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let s = src.sample(ls[0] + (px - ls[0]) / k, ls[1] + (py - ls[1]) / k, false);
            let a = s[3];
            let c3 = if glass && a > 1e-6 { [s[0] / a, s[1] / a, s[2] / a] } else { [col[0], col[1], col[2]] };
            for i in 0..3 {
                row[x * 4 + i] = c3[i] * a * op;
            }
            row[x * 4 + 3] = a * op;
        }
    });
    if soft > 0.3 {
        gaussian(&mut sh, soft * 0.5, soft * 0.5, false);
    }
    let only = c.b("shadowOnly");
    img.px.par_chunks_mut(4).zip(sh.px.par_chunks(4)).for_each(|(d, s)| {
        if only {
            d.copy_from_slice(s);
            return;
        }
        let k = 1.0 - d[3];
        for i in 0..4 {
            d[i] += s[i] * k;
        }
    });
}
