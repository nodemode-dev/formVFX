use std::f64::consts::{PI, TAU};

use rayon::prelude::*;

use super::draw::{Seg, add_light, contour, fft, paint, path_fill, path_stroke, polyline, segments_cov, segs_of, trim};
use super::generate::generate;
use super::util::{hsl_to_rgb, lum, perlin3, rand01, smooth};
use super::{EffectSpec, FxCtx, angle, check, color, layer_param, mask_param, no_pad, num, pct, point, popup, slider};
use crate::render::image::Img;

const CAT: &str = "Generate";

const PAINT_STYLE: [&str; 3] = ["On Original Image", "On Transparent", "Reveal Original Image"];

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.generate.advancedLightning",
            name: "Advanced Lightning",
            category: CAT,
            params: vec![
                popup("type", "Lightning Type", 0, &["Direction", "Strike", "Breaking", "Bouncy", "Omni", "Anywhere", "Vertical", "Two-Way Strike"]),
                point("origin", "Origin", 0.2, 0.2),
                point("direction", "Direction", 0.8, 0.8),
                num("conductivity", "Conductivity State", 10.0, 0.0, 100.0, 0.0, 100.0),
                num("coreRadius", "Core Radius", 2.0, 0.0, 30.0, 0.0, 10.0),
                pct("coreOpacity", "Core Opacity", 75.0),
                color("coreColor", "Core Color", [1.0, 1.0, 1.0]),
                num("glowRadius", "Glow Radius", 30.0, 0.0, 300.0, 0.0, 100.0),
                pct("glowOpacity", "Glow Opacity", 50.0),
                color("glowColor", "Glow Color", [0.25, 0.25, 1.0]),
                slider("turbulence", "Turbulence", 1.0, 0.0, 10.0, 2),
                pct("forking", "Forking", 25.0),
                slider("decay", "Decay", 0.3, 0.0, 1.0, 2),
                check("composite", "Composite on Original", false),
            ],
            render: advanced_lightning,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.obsolete.lightning",
            name: "Lightning",
            category: "Obsolete",
            params: vec![
                point("start", "Start Point", 0.2, 0.5),
                point("end", "End Point", 0.8, 0.5),
                slider("segments", "Segments", 7.0, 1.0, 50.0, 0),
                num("amplitude", "Amplitude", 10.0, 0.0, 500.0, 0.0, 100.0),
                slider("detail", "Detail Level", 2.0, 0.0, 10.0, 0),
                pct("branching", "Branching", 25.0),
                num("width", "Width", 6.0, 0.0, 50.0, 0.0, 20.0),
                color("outside", "Outside Color", [0.0, 0.0, 1.0]),
                color("inside", "Inside Color", [1.0, 1.0, 1.0]),
                slider("speed", "Speed", 10.0, -10.0, 10.0, 0),
                slider("seed", "Random Seed", 1.0, 0.0, 10000.0, 0),
            ],
            render: lightning,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.audioSpectrum",
            name: "Audio Spectrum",
            category: CAT,
            params: vec![
                layer_param("audioLayer", "Audio Layer"),
                point("start", "Start Point", 0.1, 0.5),
                point("end", "End Point", 0.9, 0.5),
                mask_param("path", "Path"),
                slider("startFrequency", "Start Frequency", 20.0, 1.0, 10000.0, 0),
                slider("endFrequency", "End Frequency", 1000.0, 1.0, 22000.0, 0),
                slider("bands", "Frequency bands", 64.0, 1.0, 4096.0, 0),
                num("maxHeight", "Maximum Height", 400.0, 0.0, 10000.0, 0.0, 2000.0),
                slider("duration", "Audio Duration (milliseconds)", 90.0, 1.0, 3000.0, 0),
                num("thickness", "Thickness", 3.0, 0.0, 100.0, 0.0, 20.0),
                pct("softness", "Softness", 50.0),
                color("inside", "Inside Color", [1.0, 1.0, 1.0]),
                color("outside", "Outside Color", [0.3, 0.6, 1.0]),
                popup("display", "Display Options", 0, &["Digital", "Analog lines", "Analog dots"]),
                popup("side", "Side Options", 2, &["Side A", "Side B", "Side A & B"]),
                check("composite", "Composite On Original", false),
            ],
            render: audio_spectrum,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.audioWaveform",
            name: "Audio Waveform",
            category: CAT,
            params: vec![
                layer_param("audioLayer", "Audio Layer"),
                point("start", "Start Point", 0.1, 0.5),
                point("end", "End Point", 0.9, 0.5),
                mask_param("path", "Path"),
                slider("samples", "Displayed Samples", 200.0, 2.0, 2000.0, 0),
                num("maxHeight", "Maximum Height", 100.0, 0.0, 4000.0, 0.0, 500.0),
                slider("duration", "Audio Duration (milliseconds)", 200.0, 1.0, 10000.0, 0),
                num("thickness", "Thickness", 2.0, 0.0, 100.0, 0.0, 20.0),
                pct("softness", "Softness", 10.0),
                color("inside", "Inside Color", [1.0, 1.0, 1.0]),
                color("outside", "Outside Color", [0.3, 0.6, 1.0]),
                popup("display", "Display Options", 1, &["Digital", "Analog lines", "Analog dots"]),
                check("composite", "Composite On Original", false),
            ],
            render: audio_waveform,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.beam",
            name: "Beam",
            category: CAT,
            params: vec![
                point("start", "Starting Point", 0.2, 0.5),
                point("end", "Ending Point", 0.8, 0.5),
                pct("length", "Length", 30.0),
                pct("time", "Time", 0.0),
                num("startThickness", "Starting Thickness", 8.0, 0.0, 1000.0, 0.0, 100.0),
                num("endThickness", "Ending Thickness", 8.0, 0.0, 1000.0, 0.0, 100.0),
                pct("softness", "Softness", 50.0),
                color("inside", "Inside Color", [1.0, 1.0, 1.0]),
                color("outside", "Outside Color", [1.0, 0.0, 0.0]),
                check("composite", "Composite On Original", true),
            ],
            render: beam,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.ccLightBurst",
            name: "CC Light Burst 2.5",
            category: CAT,
            params: vec![
                point("center", "Center", 0.5, 0.5),
                num("intensity", "Intensity", 100.0, 0.0, 1000.0, 0.0, 500.0),
                num("rayLength", "Ray Length", 50.0, 0.0, 500.0, 0.0, 200.0),
                popup("burst", "Burst", 0, &["Straight", "Fade", "Center"]),
                check("halfStar", "Halo Alpha", false),
                check("setColor", "Set Color", false),
                color("color", "Color", [1.0, 1.0, 1.0]),
            ],
            render: light_burst,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.ccLightRays",
            name: "CC Light Rays",
            category: CAT,
            params: vec![
                num("intensity", "Intensity", 100.0, 0.0, 1000.0, 0.0, 500.0),
                point("center", "Center", 0.5, 0.5),
                num("radius", "Radius", 40.0, 0.0, 500.0, 0.0, 200.0),
                num("warpSoftness", "Warp Softness", 50.0, 0.0, 100.0, 0.0, 100.0),
                popup("shape", "Shape", 0, &["Round", "Square"]),
                check("colorFromSource", "Color from Source", true),
                check("allowBrightening", "Allow Brightening", true),
                color("color", "Color", [1.0, 1.0, 1.0]),
                popup("mode", "Transfer Mode", 0, &["Add", "Lighten", "Screen", "None"]),
            ],
            render: light_rays,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.ccLightSweep",
            name: "CC Light Sweep",
            category: CAT,
            params: vec![
                point("center", "Center", 0.5, 0.5),
                angle("direction", "Direction", -30.0),
                popup("shape", "Shape", 1, &["Linear", "Smooth", "Sharp"]),
                num("width", "Width", 50.0, 0.0, 1000.0, 0.0, 300.0),
                num("intensity", "Sweep Intensity", 50.0, 0.0, 500.0, 0.0, 200.0),
                num("edgeIntensity", "Edge Intensity", 100.0, 0.0, 500.0, 0.0, 200.0),
                num("edgeThickness", "Edge Thickness", 1.0, 0.0, 10.0, 0.0, 10.0),
                color("color", "Light Color", [1.0, 1.0, 1.0]),
                popup("reception", "Light Reception", 0, &["Add", "Composite", "Cutout"]),
            ],
            render: light_sweep,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.ccThreads",
            name: "CC Threads",
            category: CAT,
            params: vec![
                num("width", "Width", 20.0, 1.0, 200.0, 1.0, 100.0),
                num("height", "Height", 20.0, 1.0, 200.0, 1.0, 100.0),
                slider("overlaps", "Overlaps", 2.0, 1.0, 10.0, 0),
                angle("direction", "Direction", 0.0),
                point("center", "Center", 0.5, 0.5),
                pct("coverage", "Coverage", 80.0),
                pct("shadowing", "Shadowing", 50.0),
                pct("texture", "Texture", 20.0),
            ],
            render: threads,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.cellPattern",
            name: "Cell Pattern",
            category: CAT,
            params: vec![
                popup("pattern", "Cell Pattern", 0, &["Bubbles", "Crystals", "Plates", "Static Plates", "Crystallized", "Pillow", "Mixed Crystals", "Tubular"]),
                check("invert", "Invert", false),
                slider("contrast", "Contrast", 100.0, 0.0, 10000.0, 1),
                popup("overflow", "Overflow", 1, &["Clip", "Soft Clamp", "Wrap Back"]),
                slider("disperse", "Disperse", 1.0, 0.0, 1.5, 2),
                num("size", "Size", 60.0, 1.0, 1000.0, 10.0, 200.0),
                point("offset", "Offset", 0.5, 0.5),
                angle("evolution", "Evolution", 0.0),
                slider("seed", "Random Seed", 0.0, 0.0, 10000.0, 0),
            ],
            render: cell_pattern,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.ellipse",
            name: "Ellipse",
            category: CAT,
            params: vec![
                point("center", "Center", 0.5, 0.5),
                num("width", "Width", 200.0, 0.0, 10000.0, 0.0, 1000.0),
                num("height", "Height", 200.0, 0.0, 10000.0, 0.0, 1000.0),
                num("thickness", "Thickness", 4.0, 0.0, 1000.0, 0.0, 100.0),
                pct("softness", "Softness", 50.0),
                color("inside", "Inside Color", [1.0, 1.0, 1.0]),
                color("outside", "Outside Color", [0.0, 0.0, 1.0]),
                check("composite", "Composite On Original", false),
            ],
            render: ellipse,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.eyedropperFill",
            name: "Eyedropper Fill",
            category: CAT,
            params: vec![
                point("samplePoint", "Sample Point", 0.5, 0.5),
                num("sampleRadius", "Sample Radius", 0.0, 0.0, 500.0, 0.0, 50.0),
                popup("average", "Average Pixel Colors", 0, &["Skip Empty", "All", "All Premultiplied", "Including Alpha"]),
                check("maintainAlpha", "Maintain Original Alpha", true),
                pct("blend", "Blend With Original", 0.0),
            ],
            render: eyedropper,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.fractalSet",
            name: "Fractal",
            category: CAT,
            params: vec![
                popup("set", "Set Choice", 0, &["Mandelbrot", "Mandelbrot Inverse", "Julia", "Julia Inverse"]),
                slider("x", "X (Real)", -0.75, -2.0, 2.0, 4),
                slider("y", "Y (Imaginary)", 0.0, -2.0, 2.0, 4),
                slider("magnification", "Magnification", 0.0, -5.0, 30.0, 2),
                slider("escape", "Escape Limit", 100.0, 1.0, 1024.0, 0),
                slider("juliaX", "Julia X (Real)", -0.8, -2.0, 2.0, 4),
                slider("juliaY", "Julia Y (Imaginary)", 0.156, -2.0, 2.0, 4),
                popup("palette", "Palette", 1, &["Lightness Gradient", "Hue", "Rainbow"]),
                slider("cycleSteps", "Cycle Steps", 30.0, 1.0, 1000.0, 0),
                angle("cycleOffset", "Cycle Offset", 0.0),
                check("transparency", "Transparency", false),
            ],
            render: fractal_set,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.lensFlare",
            name: "Lens Flare",
            category: CAT,
            params: vec![point("center", "Flare Center", 0.3, 0.3), slider("brightness", "Flare Brightness", 100.0, 0.0, 300.0, 0), popup("lens", "Lens Type", 0, &["50-300mm Zoom", "35mm Prime", "105mm Prime"]), pct("blend", "Blend With Original", 0.0)],
            render: lens_flare,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.paintBucket",
            name: "Paint Bucket",
            category: CAT,
            params: vec![
                point("fillPoint", "Fill Point", 0.5, 0.5),
                popup("selector", "Fill Selector", 0, &["Color & Alpha", "Straight Color", "Transparency", "Opacity", "Alpha Channel"]),
                slider("tolerance", "Tolerance", 10.0, 0.0, 100.0, 1),
                check("viewThreshold", "View Threshold", false),
                popup("stroke", "Stroke", 0, &["Antialias", "Feather", "Spread", "Choke", "Stroke"]),
                num("strokeWidth", "Spread / Feather / Stroke Width", 3.0, 0.0, 100.0, 0.0, 30.0),
                check("invert", "Invert Fill", false),
                color("color", "Color", [1.0, 0.0, 0.0]),
                pct("opacity", "Opacity", 100.0),
                popup("mode", "Blending Mode", 0, &["Normal", "Add", "Multiply", "Screen", "Behind", "Fill Only"]),
            ],
            render: paint_bucket,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.radioWaves",
            name: "Radio Waves",
            category: CAT,
            params: vec![
                point("producer", "Producer Point", 0.5, 0.5),
                popup("waveType", "Wave Type", 0, &["Polygon", "Mask"]),
                slider("sides", "Sides", 64.0, 3.0, 64.0, 0),
                check("star", "Star", false),
                pct("starDepth", "Star Depth", 50.0),
                mask_param("mask", "Mask"),
                slider("frequency", "Frequency", 1.0, 0.0, 30.0, 2),
                slider("expansion", "Expansion", 5.0, 0.0, 50.0, 2),
                angle("orientation", "Orientation", 0.0),
                angle("spin", "Spin", 0.0),
                num("velocity", "Velocity", 0.0, -100.0, 100.0, -50.0, 50.0),
                angle("direction", "Direction", 90.0),
                slider("lifespan", "Lifespan (sec)", 10.0, 0.0, 100.0, 2),
                color("color", "Color", [1.0, 1.0, 1.0]),
                pct("startOpacity", "Start Opacity", 100.0),
                pct("endOpacity", "End Opacity", 0.0),
                num("startWidth", "Start Width", 5.0, 0.0, 200.0, 0.0, 50.0),
                num("endWidth", "End Width", 5.0, 0.0, 200.0, 0.0, 50.0),
                slider("fadeIn", "Fade-in Time", 0.0, 0.0, 10.0, 2),
                slider("fadeOut", "Fade-out Time", 0.0, 0.0, 10.0, 2),
            ],
            render: radio_waves,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.scribble",
            name: "Scribble",
            category: CAT,
            params: vec![
                mask_param("mask", "Mask"),
                popup("fill", "Fill Type", 0, &["Inside", "Centered Edge", "Inside Edge", "Outside Edge"]),
                num("edgeWidth", "Edge Width", 10.0, 0.0, 300.0, 0.0, 100.0),
                color("color", "Color", [1.0, 0.0, 0.0]),
                pct("opacity", "Opacity", 100.0),
                angle("angle", "Angle", 45.0),
                num("strokeWidth", "Stroke Width", 3.0, 0.1, 50.0, 0.1, 20.0),
                num("spacing", "Spacing", 6.0, 0.5, 100.0, 1.0, 30.0),
                pct("curviness", "Curviness", 50.0),
                pct("spacingVariation", "Spacing Variation", 25.0),
                pct("start", "Start", 0.0),
                pct("end", "End", 100.0),
                popup("wiggle", "Wiggle Type", 1, &["Static", "Jumpy", "Smooth"]),
                slider("wigglesPerSecond", "Wiggles/Second", 5.0, 0.0, 30.0, 2),
                popup("composite", "Composite", 1, &["On Original Image", "On Transparent", "Reveal Original Image"]),
            ],
            render: scribble,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.stroke",
            name: "Stroke",
            category: CAT,
            params: vec![
                mask_param("path", "Path"),
                check("allMasks", "All Masks", false),
                color("color", "Color", [1.0, 1.0, 1.0]),
                num("brushSize", "Brush Size", 2.0, 0.0, 50.0, 0.0, 50.0),
                pct("hardness", "Brush Hardness", 75.0),
                pct("opacity", "Opacity", 100.0),
                pct("start", "Start", 0.0),
                pct("end", "End", 100.0),
                popup("style", "Paint Style", 0, &PAINT_STYLE),
            ],
            render: stroke_fx,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.vegas",
            name: "Vegas",
            category: CAT,
            params: vec![
                popup("strokeFrom", "Stroke", 0, &["Image Contours", "Mask/Path"]),
                mask_param("path", "Path"),
                slider("threshold", "Threshold", 50.0, 0.0, 100.0, 1),
                slider("segments", "Segments", 32.0, 1.0, 1000.0, 0),
                slider("length", "Length", 0.5, 0.0, 1.0, 3),
                angle("rotation", "Rotation", 0.0),
                popup("mode", "Blend Mode", 1, &["Transparent", "Over", "Under", "Stencil"]),
                color("color", "Color", [1.0, 1.0, 1.0]),
                num("width", "Width", 2.0, 0.0, 100.0, 0.0, 20.0),
                slider("hardness", "Hardness", 0.5, 0.0, 1.0, 2),
                pct("opacity", "Opacity", 100.0),
            ],
            render: vegas,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.writeOn",
            name: "Write-on",
            category: CAT,
            params: vec![
                point("brushPosition", "Brush Position", 0.5, 0.5),
                color("color", "Color", [1.0, 1.0, 1.0]),
                num("brushSize", "Brush Size", 10.0, 0.0, 300.0, 0.0, 100.0),
                pct("hardness", "Brush Hardness", 75.0),
                pct("opacity", "Brush Opacity", 100.0),
                slider("strokeLength", "Stroke Length (secs)", 0.0, 0.0, 600.0, 2),
                slider("spacing", "Brush Spacing (secs)", 0.01, 0.001, 1.0, 3),
                popup("style", "Paint Style", 0, &PAINT_STYLE),
            ],
            render: write_on,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.generate.ccGlueGun",
            name: "CC Glue Gun",
            category: CAT,
            params: vec![
                point("brushPosition", "Brush Position", 0.5, 0.5),
                num("strokeWidth", "Stroke Width", 30.0, 0.0, 300.0, 0.0, 100.0),
                pct("density", "Density", 100.0),
                slider("timeSpan", "Time Span (sec)", 1.0, 0.0, 30.0, 2),
                pct("reflection", "Reflection", 50.0),
                color("color", "Color", [0.8, 0.9, 1.0]),
                angle("lightDirection", "Light Direction", -45.0),
            ],
            render: glue_gun,
            pad: no_pad,
        },
    ]
}

fn finish(img: &mut Img, layer: Img, composite: bool) {
    if composite {
        img.px.par_chunks_mut(4).zip(layer.px.par_chunks(4)).for_each(|(d, s)| {
            let k = 1.0 - s[3];
            for i in 0..4 {
                d[i] = s[i] + d[i] * k;
            }
        });
    } else {
        *img = layer;
    }
}

fn bolt(rng: &mut u32, a: [f64; 2], b: [f64; 2], depth: u32, amp: f64, fork: f64, width: f64, decay: f64, out: &mut Vec<Seg>) {
    let mut next = || {
        *rng ^= *rng << 13;
        *rng ^= *rng >> 17;
        *rng ^= *rng << 5;
        (*rng as f64) / (u32::MAX as f64)
    };
    let mut pts = vec![a, b];
    let mut a_amp = amp;
    for _ in 0..depth {
        let mut np = vec![pts[0]];
        for w in pts.windows(2) {
            let (p, q) = (w[0], w[1]);
            let m = [(p[0] + q[0]) * 0.5, (p[1] + q[1]) * 0.5];
            let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
            let l = dx.hypot(dy).max(1e-6);
            let off = (next() - 0.5) * 2.0 * a_amp;
            np.push([m[0] - dy / l * off, m[1] + dx / l * off]);
            np.push(q);
        }
        pts = np;
        a_amp *= 0.55;
    }
    let n = pts.len();
    for (i, w) in pts.windows(2).enumerate() {
        let t = i as f64 / n as f64;
        out.push((w[0], w[1], width * (1.0 - t * decay)));
        if fork > 0.0 && next() < fork * 0.15 && width > 0.6 {
            let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
            let ang = dy.atan2(dx) + (next() - 0.5) * 1.6;
            let len = (b[0] - a[0]).hypot(b[1] - a[1]) * (1.0 - t) * (0.2 + next() * 0.4);
            let e = [w[0][0] + ang.cos() * len, w[0][1] + ang.sin() * len];
            let mut sub = (next() * u32::MAX as f64) as u32 | 1;
            bolt(&mut sub, w[0], e, depth.saturating_sub(1).max(2), amp * 0.5, fork * 0.5, width * 0.6, decay, out);
        }
    }
}

fn glow_layer(w: usize, h: usize, segs: &[Seg], core_r: f64, glow_r: f64, core_c: [f32; 4], core_o: f32, glow_c: [f32; 4], glow_o: f32) -> Img {
    let core: Vec<Seg> = segs.iter().map(|s| (s.0, s.1, (s.2 * core_r * 2.0).max(0.5))).collect();
    let cov = segments_cov(w, h, &core, 0.6);
    let mut out = Img::new(w, h);
    if glow_r > 0.3 {
        let mut g = cov.clone();
        super::util::blur_scalar(&mut g, w, h, glow_r * 0.5, glow_r * 0.5);
        add_light(&mut out, &g, [glow_c[0], glow_c[1], glow_c[2]], glow_o * 2.0);
    }
    add_light(&mut out, &cov, [core_c[0], core_c[1], core_c[2]], core_o);
    out.px.par_chunks_mut(4).for_each(|p| {
        let m = p[0].max(p[1]).max(p[2]).min(1.0);
        p[3] = p[3].max(m);
    });
    out
}

fn advanced_lightning(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let o = c.pt("origin");
    let d = c.pt("direction");
    let ty = c.e("type");
    let len = (d[0] - o[0]).hypot(d[1] - o[1]).max(10.0);
    let mut rng = (c.f("conductivity") * 1000.0) as u32 ^ 0x2545_F491;
    if rng == 0 {
        rng = 1;
    }
    let amp = len * 0.15 * c.f("turbulence").max(0.0) * 0.5;
    let fork = c.f("forking") / 100.0;
    let decay = c.f("decay");
    let mut segs = vec![];
    let mut targets = vec![];
    match ty {
        4 => {
            for k in 0..6 {
                let a = k as f64 / 6.0 * TAU + rand01(k, 0, 0, rng) as f64;
                targets.push([o[0] + a.cos() * len, o[1] + a.sin() * len]);
            }
        }
        6 => targets.push([o[0], h as f64]),
        7 => {
            let m = [(o[0] + d[0]) * 0.5, (o[1] + d[1]) * 0.5];
            bolt(&mut rng, m, o, 5, amp * 0.5, fork, 1.0, decay, &mut segs);
            targets.push(d);
        }
        _ => {
            let (dx, dy) = ((d[0] - o[0]) / len, (d[1] - o[1]) / len);
            let reach = if ty == 1 || ty == 2 { len } else { len.max((w.max(h)) as f64) };
            targets.push([o[0] + dx * reach, o[1] + dy * reach]);
        }
    }
    for t in targets {
        bolt(&mut rng, o, t, 6, amp, fork, 1.0, decay, &mut segs);
    }
    let layer = glow_layer(w, h, &segs, c.px(c.f("coreRadius")), c.px(c.f("glowRadius")), c.c("coreColor"), c.f("coreOpacity") as f32 / 100.0, c.c("glowColor"), c.f("glowOpacity") as f32 / 100.0);
    finish(img, layer, c.b("composite"));
}

fn lightning(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let a = c.pt("start");
    let b = c.pt("end");
    let speed = c.f("speed");
    let frame = (c.t * speed * 2.0).floor() as u32;
    let mut rng = (c.f("seed") as u32).wrapping_mul(2654435761).wrapping_add(frame.wrapping_mul(97)) | 1;
    let len = (b[0] - a[0]).hypot(b[1] - a[1]).max(1.0);
    let mut segs = vec![];
    let depth = (c.f("segments").ln().max(0.0) / 2f64.ln()).ceil() as u32 + c.f("detail") as u32;
    bolt(&mut rng, a, b, depth.clamp(1, 9), c.px(c.f("amplitude")).min(len), c.f("branching") / 100.0, 1.0, 0.3, &mut segs);
    let inside = c.c("inside");
    let outside = c.c("outside");
    let layer = glow_layer(w, h, &segs, c.px(c.f("width")) * 0.25, c.px(c.f("width")) * 1.5, inside, 1.0, outside, 0.6);
    finish(img, layer, true);
}

fn path_points(c: &FxCtx, mask_key: &str, start: &str, end: &str, n: usize) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let mut pts = vec![];
    if let Some(p) = c.mask(c.f(mask_key) as usize) {
        let pl = polyline(&p, 0.5);
        let tr = trim(&pl, 0.0, 1.0);
        pts = resample(&tr, n);
    }
    if pts.len() < 2 {
        let (a, b) = (c.pt(start), c.pt(end));
        pts = (0..n).map(|i| {
            let t = i as f64 / (n - 1).max(1) as f64;
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        }).collect();
    }
    let normals: Vec<[f64; 2]> = (0..pts.len())
        .map(|i| {
            let p = pts[i.saturating_sub(1)];
            let q = pts[(i + 1).min(pts.len() - 1)];
            let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
            let l = dx.hypot(dy).max(1e-9);
            [dy / l, -dx / l]
        })
        .collect();
    (pts, normals)
}

fn resample(pts: &[[f64; 2]], n: usize) -> Vec<[f64; 2]> {
    if pts.len() < 2 || n < 2 {
        return pts.to_vec();
    }
    (0..n).map(|i| {
        let t = i as f64 / (n - 1) as f64;
        let tr = trim(pts, 0.0, t.max(1e-9));
        *tr.last().unwrap_or(&pts[0])
    }).collect()
}

fn draw_bars(c: &FxCtx, img: &mut Img, values: &[f64], max_h: f64, both: (bool, bool), display: u32) {
    let (w, h) = (img.w, img.h);
    let n = values.len().max(2);
    let (pts, normals) = path_points(c, "path", "start", "end", n);
    let thick = c.px(c.f("thickness")).max(0.5);
    let soft = c.f("softness") / 100.0;
    let mut segs: Vec<Seg> = vec![];
    match display {
        0 => {
            for i in 0..n.min(pts.len()) {
                let v = values[i] * max_h;
                let p = pts[i];
                let nn = normals[i];
                let a = if both.0 { [p[0] + nn[0] * v, p[1] + nn[1] * v] } else { p };
                let b = if both.1 { [p[0] - nn[0] * v, p[1] - nn[1] * v] } else { p };
                segs.push((a, b, thick));
            }
        }
        _ => {
            let side = |s: f64| -> Vec<[f64; 2]> { (0..n.min(pts.len())).map(|i| [pts[i][0] + normals[i][0] * values[i] * max_h * s, pts[i][1] + normals[i][1] * values[i] * max_h * s]).collect() };
            for s in [(both.0, 1.0), (both.1, -1.0)] {
                if !s.0 {
                    continue;
                }
                let line = side(s.1);
                if display == 2 {
                    for p in &line {
                        segs.push((*p, *p, thick * 1.5));
                    }
                } else {
                    segs.extend(segs_of(&line, thick));
                }
            }
        }
    }
    let cov = segments_cov(w, h, &segs, 1.0 - soft);
    let core = segments_cov(w, h, &segs.iter().map(|s| (s.0, s.1, s.2 * 0.4)).collect::<Vec<_>>(), 1.0 - soft);
    let (ic, oc) = (c.c("inside"), c.c("outside"));
    let mut layer = Img::new(w, h);
    layer.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let a = cov[i];
        if a <= 0.0 {
            return;
        }
        let k = core[i];
        for j in 0..3 {
            p[j] = (oc[j] + (ic[j] - oc[j]) * k) * a;
        }
        p[3] = a;
    });
    finish(img, layer, c.b("composite"));
}

fn audio_spectrum(c: &FxCtx, img: &mut Img) {
    let Some(host) = c.host else { return };
    let dur = c.f("duration") / 1000.0;
    let t = c.comp_time();
    let rate = 44100u32;
    let buf = host.audio(c.layer_id("audioLayer"), t - dur * 0.5, t + dur * 0.5, rate);
    let n = 4096usize;
    let mut re = vec![0.0f64; n];
    let mut im = vec![0.0f64; n];
    let frames = buf.len() / 2;
    for i in 0..n.min(frames) {
        let win = 0.5 - 0.5 * (TAU * i as f64 / n.min(frames) as f64).cos();
        re[i] = ((buf[i * 2] + buf[i * 2 + 1]) as f64 * 0.5) * win;
    }
    fft(&mut re, &mut im);
    let bands = c.f("bands").round().clamp(1.0, 4096.0) as usize;
    let (f0, f1) = (c.f("startFrequency").max(1.0), c.f("endFrequency").max(2.0));
    let bin = |f: f64| ((f / rate as f64) * n as f64).clamp(0.0, (n / 2 - 1) as f64);
    let vals: Vec<f64> = (0..bands)
        .map(|k| {
            let fa = f0 + (f1 - f0) * k as f64 / bands as f64;
            let fb = f0 + (f1 - f0) * (k + 1) as f64 / bands as f64;
            let (ba, bb) = (bin(fa) as usize, (bin(fb) as usize).max(bin(fa) as usize + 1));
            let mut m: f64 = 0.0;
            for j in ba..bb.min(n / 2) {
                m = m.max(re[j].hypot(im[j]));
            }
            (m / (n as f64 * 0.05)).min(1.0)
        })
        .collect();
    let side = match c.e("side") {
        0 => (true, false),
        1 => (false, true),
        _ => (true, true),
    };
    draw_bars(c, img, &vals, c.px(c.f("maxHeight")), side, c.e("display"));
}

fn audio_waveform(c: &FxCtx, img: &mut Img) {
    let Some(host) = c.host else { return };
    let dur = c.f("duration") / 1000.0;
    let t = c.comp_time();
    let buf = host.audio(c.layer_id("audioLayer"), t - dur * 0.5, t + dur * 0.5, 44100);
    let n = c.f("samples").round().clamp(2.0, 2000.0) as usize;
    let frames = (buf.len() / 2).max(1);
    let vals: Vec<f64> = (0..n)
        .map(|i| {
            let j = (i * frames / n).min(frames - 1);
            ((buf.get(j * 2).copied().unwrap_or(0.0) + buf.get(j * 2 + 1).copied().unwrap_or(0.0)) * 0.5) as f64
        })
        .collect();
    let display = c.e("display");
    if display == 0 {
        let abs: Vec<f64> = vals.iter().map(|v| v.abs()).collect();
        draw_bars(c, img, &abs, c.px(c.f("maxHeight")), (true, true), 0);
    } else {
        draw_bars(c, img, &vals, c.px(c.f("maxHeight")), (true, false), display);
    }
}

fn beam(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let a = c.pt("start");
    let b = c.pt("end");
    let len = c.f("length") / 100.0;
    let t = c.f("time") / 100.0;
    let head = t * (1.0 + len);
    let tail = head - len;
    let (s0, s1) = (head.clamp(0.0, 1.0), tail.clamp(0.0, 1.0));
    if s0 <= s1 {
        if !c.b("composite") {
            *img = Img::new(w, h);
        }
        return;
    }
    let p = |u: f64| [a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u];
    let (st, et) = (c.px(c.f("startThickness")), c.px(c.f("endThickness")));
    let soft = c.f("softness") / 100.0;
    let n = 24;
    let mut segs = vec![];
    for i in 0..n {
        let u0 = s1 + (s0 - s1) * i as f64 / n as f64;
        let u1 = s1 + (s0 - s1) * (i + 1) as f64 / n as f64;
        segs.push((p(u0), p(u1), st + (et - st) * (u0 + u1) * 0.5));
    }
    let cov = segments_cov(w, h, &segs, 1.0 - soft);
    let core = segments_cov(w, h, &segs.iter().map(|s| (s.0, s.1, s.2 * 0.35)).collect::<Vec<_>>(), 1.0 - soft);
    let (ic, oc) = (c.c("inside"), c.c("outside"));
    let mut layer = Img::new(w, h);
    layer.px.par_chunks_mut(4).enumerate().for_each(|(i, px)| {
        let al = cov[i];
        if al <= 0.0 {
            return;
        }
        let k = core[i];
        for j in 0..3 {
            px[j] = (oc[j] + (ic[j] - oc[j]) * k) * al;
        }
        px[3] = al;
    });
    finish(img, layer, c.b("composite"));
}

fn light_burst(c: &FxCtx, img: &mut Img) {
    let ctr = c.pt("center");
    let k = c.f("intensity") as f32 / 100.0;
    let rl = c.f("rayLength") / 100.0;
    let burst = c.e("burst");
    let set = c.b("setColor");
    let col = c.c("color");
    let src = img.clone();
    let w = img.w;
    let n = 32usize;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (dx, dy) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let mut acc = [0.0f32; 4];
            let mut ws = 0.0f32;
            for j in 0..n {
                let t = j as f64 / n as f64;
                let s = match burst {
                    2 => 1.0 - rl * (t - 0.5),
                    _ => 1.0 - rl * t,
                };
                let wgt = if burst == 1 { (1.0 - t) as f32 } else { 1.0 };
                let v = src.sample(ctr[0] + dx * s, ctr[1] + dy * s, false);
                for i in 0..4 {
                    acc[i] += v[i] * wgt;
                }
                ws += wgt;
            }
            let p = &mut row[x * 4..x * 4 + 4];
            for i in 0..4 {
                let ray = acc[i] / ws * k;
                p[i] = (p[i] + ray).min(if i == 3 { 1.0 } else { f32::MAX });
            }
            if set && p[3] > 1e-6 {
                let l = lum(p[0], p[1], p[2]);
                p[0] = col[0] * l;
                p[1] = col[1] * l;
                p[2] = col[2] * l;
            }
        }
    });
}

fn light_rays(c: &FxCtx, img: &mut Img) {
    let ctr = c.pt("center");
    let k = c.f("intensity") as f32 / 100.0;
    let rad = c.px(c.f("radius")).max(1.0);
    let square = c.e("shape") == 1;
    let from_src = c.b("colorFromSource");
    let col = c.c("color");
    let mode = c.e("mode");
    let bright = c.b("allowBrightening");
    let src = img.clone();
    let w = img.w;
    let n = 40usize;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (dx, dy) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let mut acc = [0.0f32; 4];
            for j in 0..n {
                let s = j as f64 / n as f64;
                let (sx, sy) = (ctr[0] + dx * s, ctr[1] + dy * s);
                let d = if square { (sx - ctr[0]).abs().max((sy - ctr[1]).abs()) } else { (sx - ctr[0]).hypot(sy - ctr[1]) };
                let mask = (1.0 - d / rad).clamp(0.0, 1.0) as f32;
                let v = src.sample(sx, sy, false);
                let l = if from_src { 1.0 } else { lum(v[0], v[1], v[2]) };
                for i in 0..3 {
                    acc[i] += if from_src { v[i] } else { col[i] * l * v[3] } * mask;
                }
                acc[3] += v[3] * mask;
            }
            let p = &mut row[x * 4..x * 4 + 4];
            let ray: Vec<f32> = acc.iter().map(|v| v / n as f32 * k).collect();
            for i in 0..3 {
                p[i] = match mode {
                    1 => p[i].max(ray[i]),
                    2 => 1.0 - (1.0 - p[i]) * (1.0 - ray[i].min(1.0)),
                    3 => ray[i],
                    _ => p[i] + ray[i],
                };
                if !bright {
                    p[i] = p[i].min(1.0);
                }
            }
            p[3] = (p[3] + ray[3]).min(1.0);
        }
    });
}

fn light_sweep(c: &FxCtx, img: &mut Img) {
    let ctr = c.pt("center");
    let a = c.f("direction").to_radians();
    let (nx, ny) = (a.cos(), a.sin());
    let width = c.px(c.f("width")).max(0.5);
    let shape = c.e("shape");
    let si = c.f("intensity") as f32 / 100.0;
    let ei = c.f("edgeIntensity") as f32 / 100.0;
    let et = c.px(c.f("edgeThickness")).max(0.1);
    let col = c.c("color");
    let rec = c.e("reception");
    let (w, h) = (img.w, img.h);
    let alpha: Vec<f32> = img.px.chunks_exact(4).map(|p| p[3]).collect();
    let mut edge = alpha.clone();
    super::util::blur_scalar(&mut edge, w, h, et, et);
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let d = ((x as f64 + 0.5 - ctr[0]) * nx + (y as f64 + 0.5 - ctr[1]) * ny).abs() / (width * 0.5);
            let band = match shape {
                0 => (1.0 - d).max(0.0),
                2 => if d < 1.0 { 1.0 } else { 0.0 },
                _ => (1.0 - smooth(0.0, 1.0, d as f32)) as f64,
            } as f32;
            if band <= 0.0 {
                continue;
            }
            let i = y * w + x;
            let e = (alpha[i] - edge[i]).abs() * 2.0;
            let light = band * (si + e * ei);
            let p = &mut row[x * 4..x * 4 + 4];
            match rec {
                2 => {
                    let k = 1.0 - band;
                    for v in p.iter_mut() {
                        *v *= k;
                    }
                }
                1 => {
                    for k in 0..3 {
                        p[k] = p[k] + (col[k] * p[3] - p[k]) * light.min(1.0);
                    }
                }
                _ => {
                    for k in 0..3 {
                        p[k] += col[k] * light * p[3];
                    }
                }
            }
        }
    });
}

fn threads(c: &FxCtx, img: &mut Img) {
    let (tw, th) = (c.px(c.f("width")).max(1.0), c.px(c.f("height")).max(1.0));
    let over = c.f("overlaps").max(1.0);
    let a = c.f("direction").to_radians();
    let (s, co) = a.sin_cos();
    let ctr = c.pt("center");
    let cov = c.f("coverage") as f32 / 100.0;
    let shad = c.f("shadowing") as f32 / 100.0;
    let tex = c.f("texture") as f32 / 100.0;
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (dx, dy) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let (u, v) = (dx * co + dy * s, -dx * s + dy * co);
            let (cu, cv) = ((u / tw).floor(), (v / th).floor());
            let (fu, fv) = ((u / tw - cu) as f32, (v / th - cv) as f32);
            let horiz_top = ((cu + cv) as i64).rem_euclid((over as i64).max(1) * 2) < over as i64;
            let in_h = (fv - 0.5).abs() < cov * 0.5;
            let in_v = (fu - 0.5).abs() < cov * 0.5;
            let (vis, depth) = if horiz_top {
                if in_h { (true, (fv - 0.5).abs() * 2.0) } else if in_v { (true, (fu - 0.5).abs() * 2.0 + 0.5) } else { (false, 0.0) }
            } else if in_v {
                (true, (fu - 0.5).abs() * 2.0)
            } else if in_h {
                (true, (fv - 0.5).abs() * 2.0 + 0.5)
            } else {
                (false, 0.0)
            };
            let p = &mut row[x * 4..x * 4 + 4];
            if !vis {
                p.iter_mut().for_each(|v| *v = 0.0);
                continue;
            }
            let n = rand01((u * 0.7) as i32, (v * 0.7) as i32, 3, 11) * tex;
            let k = (1.0 - depth.min(1.0) * shad) * (1.0 - n);
            for i in 0..3 {
                p[i] *= k;
            }
        }
    });
}

fn worley(x: f64, y: f64, z: f64, seed: u32, disperse: f64) -> (f64, f64, u32) {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (mut d1, mut d2, mut id) = (f64::MAX, f64::MAX, 0u32);
    for j in -1..=1 {
        for i in -1..=1 {
            let (cx, cy) = (xi + i, yi + j);
            let h = super::util::hash3(cx, cy, 0, seed);
            let ph = z + (h as f64 / u32::MAX as f64) * TAU;
            let jx = 0.5 + (rand01(cx, cy, 1, seed) as f64 - 0.5) * disperse + ph.cos() * 0.15 * disperse;
            let jy = 0.5 + (rand01(cx, cy, 2, seed) as f64 - 0.5) * disperse + ph.sin() * 0.15 * disperse;
            let d = (cx as f64 + jx - x).hypot(cy as f64 + jy - y);
            if d < d1 {
                d2 = d1;
                d1 = d;
                id = h;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    (d1, d2, id)
}

fn cell_pattern(c: &FxCtx, img: &mut Img) {
    let size = c.px(c.f("size")).max(1.0);
    let off = c.pt("offset");
    let pat = c.e("pattern");
    let inv = c.b("invert");
    let con = c.f("contrast") / 100.0;
    let disp = c.f("disperse");
    let evo = c.f("evolution").to_radians();
    let seed = c.f("seed") as u32;
    let ov = c.e("overflow");
    generate(img, 1.0, None, true, |x, y| {
        let (u, v) = ((x - off[0]) / size, (y - off[1]) / size);
        let (d1, d2, id) = worley(u, v, evo, seed, disp);
        let r = (id >> 8) as f64 / (1u32 << 24) as f64;
        let mut val = match pat {
            1 => d2 - d1,
            2 => if d2 - d1 > 0.05 { r } else { 0.0 },
            3 => r,
            4 => (d2 - d1) * 2.0 * r,
            5 => 1.0 - d1 * d1 * 1.5,
            6 => (d2 - d1) + r * 0.5,
            7 => ((d1 * 8.0).sin() * 0.5 + 0.5) * (1.0 - d1),
            _ => (1.0 - d1 * 1.3).max(0.0),
        };
        val = (val - 0.5) * con + 0.5;
        val = match ov {
            0 => val.clamp(0.0, 1.0),
            2 => {
                let f = val.rem_euclid(2.0);
                if f > 1.0 { 2.0 - f } else { f }
            }
            _ => 1.0 / (1.0 + (-(val - 0.5) * 6.0).exp()),
        };
        if inv {
            val = 1.0 - val;
        }
        let g = val as f32;
        [g, g, g, 1.0]
    });
}

fn ellipse(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let ctr = c.pt("center");
    let (ew, eh) = (c.px(c.f("width")) * 0.5, c.px(c.f("height")) * 0.5);
    let th = c.px(c.f("thickness")).max(0.5);
    let soft = c.f("softness") / 100.0;
    let (ic, oc) = (c.c("inside"), c.c("outside"));
    let mut layer = Img::new(w, h);
    layer.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (dx, dy) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let r = (dx / ew.max(0.5)).hypot(dy / eh.max(0.5));
            let d = (r - 1.0).abs() * ew.min(eh).max(0.5);
            let half = th * 0.5;
            let edge = half * (1.0 + soft * 2.0);
            if d > edge {
                continue;
            }
            let a = if d <= half * (1.0 - soft) { 1.0 } else { 1.0 - ((d - half * (1.0 - soft)) / (edge - half * (1.0 - soft)).max(1e-6)) } as f32;
            let k = (1.0 - (d / half).min(1.0)) as f32;
            for i in 0..3 {
                row[x * 4 + i] = (oc[i] + (ic[i] - oc[i]) * k) * a;
            }
            row[x * 4 + 3] = a;
        }
    });
    finish(img, layer, c.b("composite"));
}

fn eyedropper(c: &FxCtx, img: &mut Img) {
    let p = c.pt("samplePoint");
    let r = c.px(c.f("sampleRadius")).round() as isize;
    let mode = c.e("average");
    let mut acc = [0.0f64; 4];
    let mut n = 0.0;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let s = img.get_clamped(p[0] as isize + dx, p[1] as isize + dy);
            if mode == 0 && s[3] <= 1e-4 {
                continue;
            }
            let u = if mode == 2 { [s[0], s[1], s[2]] } else if s[3] > 1e-6 { [s[0] / s[3], s[1] / s[3], s[2] / s[3]] } else { [0.0; 3] };
            for k in 0..3 {
                acc[k] += u[k] as f64;
            }
            acc[3] += s[3] as f64;
            n += 1.0;
        }
    }
    if n == 0.0 {
        return;
    }
    let col = [(acc[0] / n) as f32, (acc[1] / n) as f32, (acc[2] / n) as f32, (acc[3] / n) as f32];
    let keep = c.b("maintainAlpha");
    let blend = c.f("blend") as f32 / 100.0;
    img.px.par_chunks_mut(4).for_each(|q| {
        let a = if keep { q[3] } else if mode == 3 { col[3] } else { 1.0 };
        for k in 0..3 {
            q[k] = col[k] * a + (q[k] - col[k] * a) * blend;
        }
        q[3] = a + (q[3] - a) * blend;
    });
}

fn fractal_set(c: &FxCtx, img: &mut Img) {
    let set = c.e("set");
    let (cx, cy) = (c.f("x"), c.f("y"));
    let mag = 2f64.powf(c.f("magnification"));
    let lim = c.f("escape").round().clamp(1.0, 1024.0) as u32;
    let (jx, jy) = (c.f("juliaX"), c.f("juliaY"));
    let pal = c.e("palette");
    let steps = c.f("cycleSteps").max(1.0);
    let off = c.f("cycleOffset") / 360.0;
    let transp = c.b("transparency");
    let rect = c.layer_rect();
    let (lw, lh) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    let scale = 3.0 / mag / lw.min(lh);
    let (mx, my) = ((rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5);
    generate(img, 1.0, None, false, |x, y| {
        let (mut re, mut im) = (cx + (x - mx) * scale, cy + (y - my) * scale);
        if set == 1 || set == 3 {
            let d = re * re + im * im;
            if d > 1e-12 {
                re /= d;
                im = -im / d;
            }
        }
        let (zr0, zi0, cr, ci) = if set >= 2 { (re, im, jx, jy) } else { (0.0, 0.0, re, im) };
        let (mut zr, mut zi) = (zr0, zi0);
        let mut i = 0u32;
        while i < lim && zr * zr + zi * zi <= 4.0 {
            let t = zr * zr - zi * zi + cr;
            zi = 2.0 * zr * zi + ci;
            zr = t;
            i += 1;
        }
        if i >= lim {
            return if transp { [0.0, 0.0, 0.0, 0.0] } else { [0.0, 0.0, 0.0, 1.0] };
        }
        let sm = i as f64 + 1.0 - ((zr * zr + zi * zi).sqrt().ln().ln() / 2f64.ln());
        let t = (sm / steps + off).rem_euclid(1.0) as f32;
        let col = match pal {
            0 => [t, t, t],
            2 => hsl_to_rgb([t, 1.0, 0.5]),
            _ => hsl_to_rgb([t, 0.8, 0.45 + 0.1 * (t * std::f32::consts::TAU).sin()]),
        };
        [col[0], col[1], col[2], 1.0]
    });
}

fn lens_flare(c: &FxCtx, img: &mut Img) {
    let ctr = c.pt("center");
    let br = c.f("brightness") as f32 / 100.0;
    let lens = c.e("lens");
    let blend = c.f("blend") as f32 / 100.0;
    let rect = c.layer_rect();
    let mid = [(rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5];
    let diag = (rect[2] - rect[0]).hypot(rect[3] - rect[1]).max(1.0);
    let elems: Vec<(f64, f64, [f32; 3], f32)> = match lens {
        1 => vec![(0.4, 0.03, [0.5, 0.8, 1.0], 0.25), (0.7, 0.05, [1.0, 0.6, 0.3], 0.2), (1.2, 0.08, [0.4, 1.0, 0.5], 0.15)],
        2 => vec![(0.5, 0.02, [1.0, 0.8, 0.6], 0.3), (1.0, 0.04, [0.6, 0.6, 1.0], 0.2)],
        _ => vec![(0.3, 0.02, [0.6, 0.8, 1.0], 0.3), (0.55, 0.06, [1.0, 0.5, 0.2], 0.15), (0.8, 0.03, [0.3, 1.0, 0.5], 0.2), (1.3, 0.1, [0.5, 0.3, 1.0], 0.12), (1.6, 0.04, [1.0, 1.0, 0.6], 0.2)],
    };
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let p = [x as f64 + 0.5, y as f64 + 0.5];
            let d = (p[0] - ctr[0]).hypot(p[1] - ctr[1]) / diag;
            let mut light = [0.0f32; 3];
            let glow = (0.02 / (d + 0.02)).powf(1.6) as f32 * br;
            let ring = (-(d - 0.12).powi(2) / 0.0002).exp() as f32 * 0.25 * br;
            let ang = (p[1] - ctr[1]).atan2(p[0] - ctr[0]);
            let streak = ((ang * 6.0).cos().abs().powf(40.0) as f32) * (0.08 / (d + 0.08)) as f32 * br * 0.6;
            for k in 0..3 {
                light[k] += glow + ring * [1.0, 0.8, 0.6][k] + streak;
            }
            for (pos, size, col, int) in &elems {
                let e = [ctr[0] + (mid[0] - ctr[0]) * 2.0 * pos, ctr[1] + (mid[1] - ctr[1]) * 2.0 * pos];
                let de = (p[0] - e[0]).hypot(p[1] - e[1]) / diag;
                let v = (1.0 - (de / size).min(1.0)).powf(0.6) as f32 * int * br;
                for k in 0..3 {
                    light[k] += col[k] * v;
                }
            }
            let q = &mut row[x * 4..x * 4 + 4];
            for k in 0..3 {
                let add = light[k] * (1.0 - blend);
                q[k] += add;
            }
            let m = light[0].max(light[1]).max(light[2]) * (1.0 - blend);
            q[3] = (q[3] + m).min(1.0);
        }
    });
}

fn paint_bucket(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let p = c.pt("fillPoint");
    let (sx, sy) = (p[0].floor() as isize, p[1].floor() as isize);
    if sx < 0 || sy < 0 || sx as usize >= w || sy as usize >= h {
        return;
    }
    let sel = c.e("selector");
    let tol = c.f("tolerance") as f32 / 100.0;
    let seed = img.get(sx, sy);
    let val = |q: [f32; 4]| -> [f32; 4] {
        match sel {
            1 => {
                if q[3] > 1e-6 { [q[0] / q[3], q[1] / q[3], q[2] / q[3], 0.0] } else { [0.0; 4] }
            }
            2 => [0.0, 0.0, 0.0, if q[3] < 0.01 { 0.0 } else { 1.0 }],
            3 | 4 => [0.0, 0.0, 0.0, q[3]],
            _ => q,
        }
    };
    let sv = val(seed);
    let mut mask = vec![0.0f32; w * h];
    let mut stack = vec![(sx as usize, sy as usize)];
    let close = |q: [f32; 4]| -> bool {
        let v = val(q);
        (0..4).map(|k| (v[k] - sv[k]).abs()).fold(0.0f32, f32::max) <= tol + 1e-4
    };
    while let Some((x, y)) = stack.pop() {
        let i = y * w + x;
        if mask[i] > 0.0 {
            continue;
        }
        if !close(img.get(x as isize, y as isize)) {
            continue;
        }
        mask[i] = 1.0;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }
    let sw = c.px(c.f("strokeWidth"));
    match c.e("stroke") {
        1 => super::util::blur_scalar(&mut mask, w, h, sw * 0.5, sw * 0.5),
        2 => mask = super::util::minmax(&mask, w, h, sw.round() as usize, true),
        3 => mask = super::util::minmax(&mask, w, h, sw.round() as usize, false),
        4 => {
            let g = super::util::minmax(&mask, w, h, sw.round().max(1.0) as usize, true);
            mask = g.iter().zip(&mask).map(|(a, b)| a - b).collect();
        }
        _ => super::util::blur_scalar(&mut mask, w, h, 0.5, 0.5),
    }
    if c.b("invert") {
        mask.iter_mut().for_each(|v| *v = 1.0 - *v);
    }
    if c.b("viewThreshold") {
        img.px.par_chunks_mut(4).zip(mask.par_iter()).for_each(|(q, m)| {
            q[0] = *m;
            q[1] = *m;
            q[2] = *m;
            q[3] = 1.0;
        });
        return;
    }
    let col = c.c("color");
    let op = c.f("opacity") as f32 / 100.0;
    let mode = c.e("mode");
    img.px.par_chunks_mut(4).zip(mask.par_iter()).for_each(|(q, m)| {
        let a = m * op;
        if a <= 0.0 && mode != 5 {
            return;
        }
        match mode {
            4 => {
                let k = 1.0 - q[3];
                for i in 0..3 {
                    q[i] += col[i] * a * k;
                }
                q[3] += a * k;
            }
            5 => {
                for i in 0..3 {
                    q[i] = col[i] * a;
                }
                q[3] = a;
            }
            _ => {
                let u = if q[3] > 1e-6 { [q[0] / q[3], q[1] / q[3], q[2] / q[3]] } else { [0.0; 3] };
                for i in 0..3 {
                    let b = match mode {
                        1 => u[i] + col[i],
                        2 => u[i] * col[i],
                        3 => 1.0 - (1.0 - u[i]) * (1.0 - col[i]),
                        _ => col[i],
                    };
                    let base = q[i];
                    let na = q[3] + (1.0 - q[3]) * a;
                    q[i] = base + (b * na - base) * a;
                }
                q[3] += (1.0 - q[3]) * a;
            }
        }
    });
}

fn radio_waves(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let t = c.t;
    let freq = c.f("frequency").max(0.0);
    if freq <= 0.0 {
        *img = Img::new(w, h);
        return;
    }
    let life = c.f("lifespan").max(0.01);
    let exp = c.px(c.f("expansion")) * 20.0;
    let p0 = c.pt("producer");
    let dir = c.f("direction").to_radians();
    let vel = c.px(c.f("velocity")) * 10.0;
    let sides = c.f("sides").round().clamp(3.0, 64.0) as usize;
    let star = c.b("star");
    let depth = c.f("starDepth") / 100.0;
    let orient = c.f("orientation").to_radians();
    let spin = c.f("spin").to_radians();
    let (so, eo) = (c.f("startOpacity") as f32 / 100.0, c.f("endOpacity") as f32 / 100.0);
    let (sw, ew) = (c.px(c.f("startWidth")), c.px(c.f("endWidth")));
    let (fi, fo) = (c.f("fadeIn"), c.f("fadeOut"));
    let mask_pts = if c.e("waveType") == 1 { c.mask(c.f("mask") as usize).map(|m| polyline(&m, 0.5)) } else { None };
    let col = c.c("color");
    let mut layer = Img::new(w, h);
    let period = 1.0 / freq;
    let first = ((t - life) / period).floor().max(0.0) as i64;
    let last = (t / period).floor() as i64;
    for k in first..=last {
        let birth = k as f64 * period;
        let age = t - birth;
        if age < 0.0 || age > life {
            continue;
        }
        let u = age / life;
        let mut op = so + (eo - so) * u as f32;
        if fi > 0.0 && age < fi {
            op *= (age / fi) as f32;
        }
        if fo > 0.0 && life - age < fo {
            op *= ((life - age) / fo) as f32;
        }
        let width = sw + (ew - sw) * u;
        let r = exp * age;
        let ctr = [p0[0] + dir.cos() * vel * age, p0[1] + dir.sin() * vel * age];
        let rot = orient + spin * age;
        let pts: Vec<[f64; 2]> = match &mask_pts {
            Some(mp) if mp.len() > 1 => {
                let n = mp.len() as f64;
                let mc = [mp.iter().map(|p| p[0]).sum::<f64>() / n, mp.iter().map(|p| p[1]).sum::<f64>() / n];
                let s = 1.0 + age * exp / 100.0;
                mp.iter().map(|p| [ctr[0] + (p[0] - mc[0]) * s, ctr[1] + (p[1] - mc[1]) * s]).collect()
            }
            _ => {
                let n = if star { sides * 2 } else { sides };
                (0..=n)
                    .map(|i| {
                        let a = rot + i as f64 / n as f64 * TAU - PI / 2.0;
                        let rr = if star && i % 2 == 1 { r * (1.0 - depth) } else { r };
                        [ctr[0] + a.cos() * rr, ctr[1] + a.sin() * rr]
                    })
                    .collect()
            }
        };
        let cov = segments_cov(w, h, &segs_of(&pts, width.max(0.5)), 0.8);
        add_light(&mut layer, &cov, [col[0], col[1], col[2]], op);
    }
    layer.px.par_chunks_mut(4).for_each(|p| {
        let m = p[3].min(1.0);
        p[3] = m;
        for i in 0..3 {
            p[i] = p[i].min(m);
        }
    });
    *img = layer;
}

fn scribble(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let Some(mask) = c.mask(c.f("mask") as usize) else { return };
    let fill = c.e("fill");
    let ew = c.px(c.f("edgeWidth"));
    let region: Vec<f32> = match fill {
        1 => path_stroke(std::slice::from_ref(&mask), w, h, ew),
        2 => {
            let f = path_fill(std::slice::from_ref(&mask), w, h);
            let s = path_stroke(std::slice::from_ref(&mask), w, h, ew * 2.0);
            f.iter().zip(&s).map(|(a, b)| a * b).collect()
        }
        3 => {
            let f = path_fill(std::slice::from_ref(&mask), w, h);
            let s = path_stroke(std::slice::from_ref(&mask), w, h, ew * 2.0);
            f.iter().zip(&s).map(|(a, b)| (1.0 - a) * b).collect()
        }
        _ => path_fill(std::slice::from_ref(&mask), w, h),
    };
    let ang = c.f("angle").to_radians();
    let (dx, dy) = (ang.cos(), ang.sin());
    let (nx, ny) = (-dy, dx);
    let sp = c.px(c.f("spacing")).max(0.5);
    let sv = c.f("spacingVariation") / 100.0;
    let curv = c.f("curviness") / 100.0;
    let wt = c.e("wiggle");
    let wps = c.f("wigglesPerSecond");
    let phase = match wt {
        0 => 0.0,
        1 => (c.t * wps).floor(),
        _ => c.t * wps,
    };
    let b = mask.bounds().unwrap_or([0.0, 0.0, w as f64, h as f64]);
    let ctr = [(b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5];
    let rad = ((b[2] - b[0]).hypot(b[3] - b[1]) * 0.5).max(1.0) + ew;
    let mut path: Vec<[f64; 2]> = vec![];
    let lines = (2.0 * rad / sp).ceil() as i64;
    for i in 0..lines {
        let off = -rad + i as f64 * sp + (perlin3(i as f32 * 0.37, phase as f32 * 0.3, 0.0, 5) as f64) * sp * sv;
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        let (a, bb) = ([ctr[0] + nx * off - dx * rad * side, ctr[1] + ny * off - dy * rad * side], [ctr[0] + nx * off + dx * rad * side, ctr[1] + ny * off + dy * rad * side]);
        let steps = 24;
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let wob = perlin3(i as f32 * 0.5, t as f32 * 3.0, phase as f32 * 0.5, 9) as f64 * sp * curv;
            path.push([a[0] + (bb[0] - a[0]) * t + nx * wob, a[1] + (bb[1] - a[1]) * t + ny * wob]);
        }
    }
    let tr = trim(&path, c.f("start") / 100.0, c.f("end") / 100.0);
    let mut cov = segments_cov(w, h, &segs_of(&tr, c.px(c.f("strokeWidth"))), 0.7);
    cov.iter_mut().zip(&region).for_each(|(a, r)| *a *= r);
    let col = c.c("color");
    let op = c.f("opacity") as f32 / 100.0;
    match c.e("composite") {
        0 => paint(img, &cov, col, op, 0),
        2 => paint(img, &cov, col, op, 2),
        _ => paint(img, &cov, col, op, 1),
    }
}

fn stroke_fx(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let paths: Vec<_> = if c.b("allMasks") {
        (1..=64).map_while(|i| c.mask(i)).collect()
    } else {
        c.mask(c.f("path") as usize).into_iter().collect()
    };
    if paths.is_empty() {
        return;
    }
    let size = c.px(c.f("brushSize")).max(0.1);
    let hard = c.f("hardness") / 100.0;
    let (s, e) = (c.f("start") / 100.0, c.f("end") / 100.0);
    let mut segs = vec![];
    for p in &paths {
        let pl = polyline(p, 0.3);
        segs.extend(segs_of(&trim(&pl, s, e), size));
    }
    let cov = segments_cov(w, h, &segs, hard);
    paint(img, &cov, c.c("color"), c.f("opacity") as f32 / 100.0, c.e("style"));
}

fn vegas(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let lines: Vec<Vec<[f64; 2]>> = if c.e("strokeFrom") == 1 {
        c.mask(c.f("path") as usize).map(|m| vec![polyline(&m, 0.5)]).unwrap_or_default()
    } else {
        let a: Vec<f32> = img.px.chunks_exact(4).map(|p| if p[3] > 1e-6 { lum(p[0] / p[3], p[1] / p[3], p[2] / p[3]) * p[3] } else { 0.0 }).collect();
        contour(&a, w, h, c.f("threshold") as f32 / 100.0)
    };
    let segs_n = c.f("segments").round().max(1.0);
    let len = c.f("length").clamp(0.0, 1.0);
    let rot = c.f("rotation") / 360.0;
    let width = c.px(c.f("width")).max(0.1);
    let mut segs = vec![];
    for l in &lines {
        for k in 0..segs_n as usize {
            let s0 = (k as f64 / segs_n + rot).rem_euclid(1.0);
            let s1 = s0 + len / segs_n;
            if s1 <= 1.0 {
                segs.extend(segs_of(&trim(l, s0, s1), width));
            } else {
                segs.extend(segs_of(&trim(l, s0, 1.0), width));
                segs.extend(segs_of(&trim(l, 0.0, s1 - 1.0), width));
            }
        }
    }
    let cov = segments_cov(w, h, &segs, c.f("hardness"));
    let col = c.c("color");
    let op = c.f("opacity") as f32 / 100.0;
    match c.e("mode") {
        0 => paint(img, &cov, col, op, 1),
        2 => {
            let mut layer = Img::new(w, h);
            paint(&mut layer, &cov, col, op, 1);
            img.px.par_chunks_mut(4).zip(layer.px.par_chunks(4)).for_each(|(d, s)| {
                let k = 1.0 - d[3];
                for i in 0..4 {
                    d[i] += s[i] * k;
                }
            });
        }
        3 => paint(img, &cov, col, op, 2),
        _ => paint(img, &cov, col, op, 0),
    }
}

fn write_on(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let len = c.f("strokeLength");
    let sp = c.f("spacing").max(0.001);
    let start = c.t.min(len.max(0.0));
    let span = if len <= 0.0 { c.t } else { start };
    let n = ((span / sp).ceil() as usize).min(4000);
    let size = c.px(c.f("brushSize")).max(0.1);
    let mut pts = vec![];
    for k in 0..=n {
        let dt = -(k as f64) * sp;
        if -dt > c.t + 1e-9 {
            break;
        }
        pts.push(c.pt_at("brushPosition", dt));
    }
    let segs: Vec<Seg> = if pts.len() < 2 { pts.iter().map(|p| (*p, *p, size)).collect() } else { segs_of(&pts, size) };
    let cov = segments_cov(w, h, &segs, c.f("hardness") / 100.0);
    paint(img, &cov, c.c("color"), c.f("opacity") as f32 / 100.0, c.e("style"));
}

fn glue_gun(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let span = c.f("timeSpan").max(0.0).min(c.t.max(0.0));
    let n = ((span * 60.0).ceil() as usize).clamp(1, 2000);
    let size = c.px(c.f("strokeWidth")).max(0.5);
    let pts: Vec<[f64; 2]> = (0..=n).map(|k| c.pt_at("brushPosition", -span * k as f64 / n as f64)).collect();
    let mut hf = segments_cov(w, h, &segs_of(&pts, size), 0.2);
    let dens = c.f("density") as f32 / 100.0;
    hf.iter_mut().for_each(|v| *v *= dens);
    let mut blur = hf.clone();
    super::util::blur_scalar(&mut blur, w, h, size * 0.15, size * 0.15);
    let la = c.f("lightDirection").to_radians();
    let (lx, ly) = (la.cos() as f32, la.sin() as f32);
    let refl = c.f("reflection") as f32 / 100.0;
    let col = c.c("color");
    let src = img.clone();
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let i = y * w + x;
            let a = hf[i];
            if a <= 0.0 {
                continue;
            }
            let g = |xx: usize, yy: usize| blur[yy.min(h - 1) * w + xx.min(w - 1)];
            let gx = g(x + 1, y) - g(x.saturating_sub(1), y);
            let gy = g(x, y + 1) - g(x, y.saturating_sub(1));
            let s = src.sample(x as f64 + 0.5 - gx as f64 * size * 0.3, y as f64 + 0.5 - gy as f64 * size * 0.3, true);
            let spec = (-(gx * lx + gy * ly) * 6.0).max(0.0).powi(3) * refl;
            let p = &mut row[x * 4..x * 4 + 4];
            for k in 0..3 {
                let v = s[k] * col[k] + spec;
                p[k] = p[k] * (1.0 - a) + v * a;
            }
            p[3] = p[3] + (1.0 - p[3]) * a;
        }
    });
}
