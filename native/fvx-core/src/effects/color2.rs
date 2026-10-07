use rayon::prelude::*;

use super::util::{gaussian, hsl_to_rgb, lum, map_color, rgb_to_hsl, smooth};
use super::{EffectSpec, FxCtx, angle, check, color, grp, hidden, layer_param, no_pad, num, pct, point, popup, slider, text};
use crate::render::image::Img;

const CAT: &str = "Color Correction";

fn curve_params() -> Vec<super::ParamSpec> {
    vec![
        popup("channel", "Channel", 0, &["RGB", "Red", "Green", "Blue", "Alpha"]),
        hidden("rgb", "RGB", "0,0 1,1"),
        hidden("red", "Red", "0,0 1,1"),
        hidden("green", "Green", "0,0 1,1"),
        hidden("blue", "Blue", "0,0 1,1"),
        hidden("alpha", "Alpha", "0,0 1,1"),
    ]
}

fn levels_group(prefix: &'static str, label: &'static str) -> Vec<super::ParamSpec> {
    let ids: [&'static str; 5] = match prefix {
        "rgb" => ["rgb/inBlack", "rgb/inWhite", "rgb/gamma", "rgb/outBlack", "rgb/outWhite"],
        "red" => ["red/inBlack", "red/inWhite", "red/gamma", "red/outBlack", "red/outWhite"],
        "green" => ["green/inBlack", "green/inWhite", "green/gamma", "green/outBlack", "green/outWhite"],
        "blue" => ["blue/inBlack", "blue/inWhite", "blue/gamma", "blue/outBlack", "blue/outWhite"],
        _ => ["alpha/inBlack", "alpha/inWhite", "alpha/gamma", "alpha/outBlack", "alpha/outWhite"],
    };
    vec![
        grp(prefix, label),
        slider(ids[0], "Input Black", 0.0, 0.0, 1.0, 3),
        slider(ids[1], "Input White", 1.0, 0.0, 1.0, 3),
        slider(ids[2], "Gamma", 1.0, 0.1, 10.0, 2),
        slider(ids[3], "Output Black", 0.0, 0.0, 1.0, 3),
        slider(ids[4], "Output White", 1.0, 0.0, 1.0, 3),
    ]
}

const SEL: [&str; 9] = ["reds", "yellows", "greens", "cyans", "blues", "magentas", "whites", "neutrals", "blacks"];

fn selective_params() -> Vec<super::ParamSpec> {
    let mut v = vec![popup("method", "Method", 0, &["Relative", "Absolute"])];
    let names = ["Reds", "Yellows", "Greens", "Cyans", "Blues", "Magentas", "Whites", "Neutrals", "Blacks"];
    let ids: [[&'static str; 4]; 9] = [
        ["reds/cyan", "reds/magenta", "reds/yellow", "reds/black"],
        ["yellows/cyan", "yellows/magenta", "yellows/yellow", "yellows/black"],
        ["greens/cyan", "greens/magenta", "greens/yellow", "greens/black"],
        ["cyans/cyan", "cyans/magenta", "cyans/yellow", "cyans/black"],
        ["blues/cyan", "blues/magenta", "blues/yellow", "blues/black"],
        ["magentas/cyan", "magentas/magenta", "magentas/yellow", "magentas/black"],
        ["whites/cyan", "whites/magenta", "whites/yellow", "whites/black"],
        ["neutrals/cyan", "neutrals/magenta", "neutrals/yellow", "neutrals/black"],
        ["blacks/cyan", "blacks/magenta", "blacks/yellow", "blacks/black"],
    ];
    for (i, n) in names.iter().enumerate() {
        v.push(grp(SEL[i], n));
        for (k, lbl) in ["Cyan", "Magenta", "Yellow", "Black"].iter().enumerate() {
            v.push(slider(ids[i][k], lbl, 0.0, -100.0, 100.0, 0));
        }
    }
    v
}

pub fn specs() -> Vec<EffectSpec> {
    let mut lv = vec![];
    for (p, l) in [("rgb", "RGB"), ("red", "Red"), ("green", "Green"), ("blue", "Blue"), ("alpha", "Alpha")] {
        lv.extend(levels_group(p, l));
    }
    lv.push(check("clipBlack", "Clip To Output Black", true));
    lv.push(check("clipWhite", "Clip To Output White", true));
    vec![
        EffectSpec { id: "fx.color.curves", name: "Curves", category: CAT, params: curve_params(), render: curves, pad: no_pad },
        EffectSpec { id: "fx.color.levelsIndividual", name: "Levels (Individual Controls)", category: CAT, params: lv, render: levels_ic, pad: no_pad },
        EffectSpec {
            id: "fx.color.lumetri",
            name: "Lumetri Color",
            category: CAT,
            params: vec![
                grp("basic", "Basic Correction"),
                text("basic/inputLut", "Input LUT", ""),
                slider("basic/temperature", "Temperature", 0.0, -100.0, 100.0, 1),
                slider("basic/tint", "Tint", 0.0, -100.0, 100.0, 1),
                slider("basic/exposure", "Exposure", 0.0, -5.0, 5.0, 2),
                slider("basic/contrast", "Contrast", 0.0, -100.0, 100.0, 1),
                slider("basic/highlights", "Highlights", 0.0, -100.0, 100.0, 1),
                slider("basic/shadows", "Shadows", 0.0, -100.0, 100.0, 1),
                slider("basic/whites", "Whites", 0.0, -100.0, 100.0, 1),
                slider("basic/blacks", "Blacks", 0.0, -100.0, 100.0, 1),
                slider("basic/saturation", "Saturation", 100.0, 0.0, 200.0, 1),
                grp("creative", "Creative"),
                text("creative/look", "Look", ""),
                pct("creative/lookIntensity", "Intensity", 100.0),
                slider("creative/fadedFilm", "Faded Film", 0.0, 0.0, 100.0, 1),
                slider("creative/sharpen", "Sharpen", 0.0, -100.0, 100.0, 1),
                slider("creative/vibrance", "Vibrance", 0.0, -100.0, 100.0, 1),
                slider("creative/saturation", "Saturation", 100.0, 0.0, 200.0, 1),
                color("creative/shadowTint", "Shadow Tint", [0.5, 0.5, 0.5]),
                color("creative/highlightTint", "Highlight Tint", [0.5, 0.5, 0.5]),
                slider("creative/tintBalance", "Tint Balance", 0.0, -100.0, 100.0, 1),
                grp("curves", "Curves"),
                hidden("curves/rgb", "RGB Curves", "0,0 1,1"),
                hidden("curves/red", "Red Curve", "0,0 1,1"),
                hidden("curves/green", "Green Curve", "0,0 1,1"),
                hidden("curves/blue", "Blue Curve", "0,0 1,1"),
                grp("wheels", "Color Wheels"),
                color("wheels/shadows", "Shadows", [0.5, 0.5, 0.5]),
                slider("wheels/shadowLightness", "Shadow Lightness", 0.0, -100.0, 100.0, 1),
                color("wheels/midtones", "Midtones", [0.5, 0.5, 0.5]),
                slider("wheels/midtoneLightness", "Midtone Lightness", 0.0, -100.0, 100.0, 1),
                color("wheels/highlights", "Highlights", [0.5, 0.5, 0.5]),
                slider("wheels/highlightLightness", "Highlight Lightness", 0.0, -100.0, 100.0, 1),
                grp("hsl", "HSL Secondary"),
                check("hsl/enable", "Enable", false),
                angle("hsl/hue", "Key Hue", 0.0),
                slider("hsl/hueRange", "Hue Range", 30.0, 0.0, 180.0, 1),
                slider("hsl/satMin", "Saturation Min", 10.0, 0.0, 100.0, 1),
                slider("hsl/lumMin", "Luminance Min", 0.0, 0.0, 100.0, 1),
                slider("hsl/lumMax", "Luminance Max", 100.0, 0.0, 100.0, 1),
                slider("hsl/denoise", "Denoise", 10.0, 0.0, 100.0, 1),
                check("hsl/showMask", "Show Mask", false),
                color("hsl/correction", "Correction Tint", [0.5, 0.5, 0.5]),
                slider("hsl/temperature", "Temperature", 0.0, -100.0, 100.0, 1),
                slider("hsl/saturation", "Saturation", 100.0, 0.0, 200.0, 1),
                slider("hsl/contrast", "Contrast", 0.0, -100.0, 100.0, 1),
                grp("vignette", "Vignette"),
                slider("vignette/amount", "Amount", 0.0, -5.0, 5.0, 2),
                slider("vignette/midpoint", "Midpoint", 50.0, 0.0, 100.0, 1),
                slider("vignette/roundness", "Roundness", 0.0, -100.0, 100.0, 1),
                slider("vignette/feather", "Feather", 50.0, 0.0, 100.0, 1),
            ],
            render: lumetri,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.colorBalanceHls",
            name: "Color Balance (HLS)",
            category: CAT,
            params: vec![angle("hue", "Hue", 0.0), slider("lightness", "Lightness", 0.0, -100.0, 100.0, 1), slider("saturation", "Saturation", 0.0, -100.0, 100.0, 1)],
            render: |c, img| {
                let (dh, dl, ds) = ((c.f("hue") / 360.0) as f32, c.f("lightness") as f32 / 100.0, c.f("saturation") as f32 / 100.0);
                map_color(img, |v| {
                    let mut h = rgb_to_hsl(v);
                    h[0] = (h[0] + dh).rem_euclid(1.0);
                    h[1] = if ds >= 0.0 { h[1] + (1.0 - h[1]) * ds } else { h[1] * (1.0 + ds) };
                    h[2] = if dl >= 0.0 { h[2] + (1.0 - h[2]) * dl } else { h[2] * (1.0 + dl) };
                    hsl_to_rgb(h)
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.changeColor",
            name: "Change Color",
            category: CAT,
            params: vec![
                popup("view", "View", 0, &["Corrected Layer", "Color Correction Mask"]),
                angle("hueTransform", "Hue Transform", 0.0),
                slider("lightnessTransform", "Lightness Transform", 0.0, -100.0, 100.0, 1),
                slider("saturationTransform", "Saturation Transform", 0.0, -100.0, 100.0, 1),
                color("colorToChange", "Color To Change", [1.0, 0.0, 0.0]),
                pct("tolerance", "Matching Tolerance", 15.0),
                pct("softness", "Matching Softness", 0.0),
                popup("matchColors", "Match Colors", 0, &["Using RGB", "Using Hue", "Using Chroma"]),
                check("invert", "Invert Color Correction Mask", false),
            ],
            render: change_color,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.changeToColor",
            name: "Change to Color",
            category: CAT,
            params: vec![
                color("from", "From", [1.0, 0.0, 0.0]),
                color("to", "To", [0.0, 0.0, 1.0]),
                popup("change", "Change", 0, &["Hue", "Hue & Lightness", "Hue & Saturation", "Hue, Lightness & Saturation"]),
                popup("changeBy", "Change By", 0, &["Setting To Color", "Transforming To Color"]),
                pct("toleranceHue", "Hue Tolerance", 5.0),
                pct("toleranceLightness", "Lightness Tolerance", 50.0),
                pct("toleranceSaturation", "Saturation Tolerance", 50.0),
                pct("softness", "Softness", 50.0),
                check("viewMatte", "View Correction Matte", false),
            ],
            render: change_to_color,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.leaveColor",
            name: "Leave Color",
            category: CAT,
            params: vec![
                pct("amount", "Amount to Decolor", 0.0),
                color("colorToLeave", "Color To Leave", [1.0, 0.0, 0.0]),
                pct("tolerance", "Tolerance", 15.0),
                pct("softness", "Edge Softness", 0.0),
                popup("matchColors", "Match colors", 0, &["Using RGB", "Using Hue"]),
            ],
            render: leave_color,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.colorama",
            name: "Colorama",
            category: CAT,
            params: vec![
                grp("input", "Input Phase"),
                popup("input/getPhaseFrom", "Get Phase From", 0, &PHASE_FROM),
                angle("input/phaseShift", "Phase Shift", 0.0),
                grp("output", "Output Cycle"),
                popup("output/palette", "Use Preset Palette", 0, &["Hue Cycle", "Ramp Grayscale", "Fire", "Ice", "Ramp Red", "Ramp Green", "Ramp Blue", "Pastel Cycle", "Custom"]),
                text("output/custom", "Custom Palette", "0:1,0,0 0.5:0,1,0 1:0,0,1"),
                slider("output/cycles", "Cycle Repetitions", 1.0, 0.0, 10.0, 2),
                check("output/interpolate", "Interpolate Palette", true),
                grp("modify", "Modify"),
                popup("modify/channels", "Modify", 0, &["All", "Red", "Green", "Blue", "Hue", "Lightness", "Saturation"]),
                check("modify/alpha", "Modify Alpha", false),
                pct("blend", "Blend With Original", 0.0),
            ],
            render: colorama,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.photoFilter",
            name: "Photo Filter",
            category: CAT,
            params: vec![
                popup("filter", "Filter", 0, &PHOTO_FILTERS),
                color("color", "Color", [0.925, 0.541, 0.0]),
                pct("density", "Density", 25.0),
                check("preserveLuminosity", "Preserve Luminosity", true),
            ],
            render: photo_filter,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.autoColor",
            name: "Auto Color",
            category: CAT,
            params: vec![
                slider("smoothing", "Temporal Smoothing (seconds)", 0.0, 0.0, 10.0, 2),
                check("sceneDetect", "Scene Detect", false),
                slider("blackClip", "Black Clip", 0.1, 0.0, 10.0, 2),
                slider("whiteClip", "White Clip", 0.1, 0.0, 10.0, 2),
                check("snapNeutral", "Snap Neutral Midtones", false),
                pct("blend", "Blend With Original", 0.0),
            ],
            render: |c, img| auto_levels(c, img, 2),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.autoContrast",
            name: "Auto Contrast",
            category: CAT,
            params: vec![
                slider("smoothing", "Temporal Smoothing (seconds)", 0.0, 0.0, 10.0, 2),
                check("sceneDetect", "Scene Detect", false),
                slider("blackClip", "Black Clip", 0.1, 0.0, 10.0, 2),
                slider("whiteClip", "White Clip", 0.1, 0.0, 10.0, 2),
                pct("blend", "Blend With Original", 0.0),
            ],
            render: |c, img| auto_levels(c, img, 0),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.autoLevels",
            name: "Auto Levels",
            category: CAT,
            params: vec![
                slider("smoothing", "Temporal Smoothing (seconds)", 0.0, 0.0, 10.0, 2),
                check("sceneDetect", "Scene Detect", false),
                slider("blackClip", "Black Clip", 0.1, 0.0, 10.0, 2),
                slider("whiteClip", "White Clip", 0.1, 0.0, 10.0, 2),
                pct("blend", "Blend With Original", 0.0),
            ],
            render: |c, img| auto_levels(c, img, 1),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.shadowHighlight",
            name: "Shadow/Highlight",
            category: CAT,
            params: vec![
                check("auto", "Auto Amounts", true),
                pct("shadowAmount", "Shadow Amount", 50.0),
                pct("highlightAmount", "Highlight Amount", 0.0),
                grp("more", "More Options"),
                pct("more/shadowTonalWidth", "Shadow Tonal Width", 50.0),
                num("more/shadowRadius", "Shadow Radius", 30.0, 0.0, 2500.0, 0.0, 100.0),
                pct("more/highlightTonalWidth", "Highlight Tonal Width", 50.0),
                num("more/highlightRadius", "Highlight Radius", 30.0, 0.0, 2500.0, 0.0, 100.0),
                slider("more/colorCorrection", "Color Correction", 20.0, -100.0, 100.0, 0),
                slider("more/midtoneContrast", "Midtone Contrast", 0.0, -100.0, 100.0, 0),
                pct("blend", "Blend With Original", 0.0),
            ],
            render: shadow_highlight,
            pad: no_pad,
        },
        EffectSpec { id: "fx.color.selectiveColor", name: "Selective Color", category: CAT, params: selective_params(), render: selective_color, pad: no_pad },
        EffectSpec {
            id: "fx.color.gammaPedestalGain",
            name: "Gamma/Pedestal/Gain",
            category: CAT,
            params: vec![
                slider("blackStretch", "Black Stretch", 0.0, 0.0, 4.0, 2),
                slider("redGamma", "Red Gamma", 1.0, 0.1, 4.0, 2),
                slider("redPedestal", "Red Pedestal", 0.0, -1.0, 1.0, 2),
                slider("redGain", "Red Gain", 1.0, 0.0, 4.0, 2),
                slider("greenGamma", "Green Gamma", 1.0, 0.1, 4.0, 2),
                slider("greenPedestal", "Green Pedestal", 0.0, -1.0, 1.0, 2),
                slider("greenGain", "Green Gain", 1.0, 0.0, 4.0, 2),
                slider("blueGamma", "Blue Gamma", 1.0, 0.1, 4.0, 2),
                slider("bluePedestal", "Blue Pedestal", 0.0, -1.0, 1.0, 2),
                slider("blueGain", "Blue Gain", 1.0, 0.0, 4.0, 2),
            ],
            render: |c, img| {
                let bs = c.f("blackStretch") as f32;
                let g = [c.f("redGamma") as f32, c.f("greenGamma") as f32, c.f("blueGamma") as f32];
                let pd = [c.f("redPedestal") as f32, c.f("greenPedestal") as f32, c.f("bluePedestal") as f32];
                let gn = [c.f("redGain") as f32, c.f("greenGain") as f32, c.f("blueGain") as f32];
                map_color(img, |v| {
                    let mut o = [0.0; 3];
                    for k in 0..3 {
                        let x = v[k] + bs * 0.1 * (1.0 - v[k]).powi(4) * v[k];
                        o[k] = (x.max(0.0).powf(1.0 / g[k].max(0.01)) * gn[k] + pd[k]).clamp(0.0, 1.0);
                    }
                    o
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.broadcastColors",
            name: "Broadcast Colors",
            category: CAT,
            params: vec![
                popup("locale", "Broadcast Locale", 0, &["NTSC", "PAL"]),
                popup("method", "How To Make Color Safe", 0, &["Reduce Luminance", "Reduce Saturation", "Key Out Unsafe", "Key Out Safe"]),
                slider("maxAmplitude", "Maximum Signal Amplitude (IRE)", 110.0, 90.0, 120.0, 0),
            ],
            render: broadcast,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.equalize",
            name: "Equalize",
            category: CAT,
            params: vec![popup("style", "Equalize", 0, &["RGB", "Brightness", "Photoshop Style"]), pct("amount", "Amount to Equalize", 100.0)],
            render: equalize,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.colorLink",
            name: "Color Link",
            category: CAT,
            params: vec![
                layer_param("source", "Source Layer"),
                popup("sample", "Sample", 0, &["Average", "Median", "Brightest Values", "Darkest Values", "Max RGB", "Min RGB", "Average Alpha", "Alpha Only", "Max Alpha"]),
                pct("clip", "Clip (%)", 0.0),
                check("stencil", "Stencil Original Alpha", false),
                pct("opacity", "Opacity", 100.0),
                popup("mode", "Blending Mode", 0, &["Normal", "Multiply", "Screen", "Overlay", "Color", "Hue"]),
            ],
            render: color_link,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.videoLimiter",
            name: "Video Limiter",
            category: CAT,
            params: vec![popup("method", "Clip Method", 0, &["Clip Luma and Chroma", "Clip Signal"]), slider("lumaMin", "Luma Min", 0.0, 0.0, 1.0, 3), slider("lumaMax", "Luma Max", 1.0, 0.0, 1.0, 3), slider("chromaMax", "Chroma Max", 0.5, 0.0, 1.0, 3)],
            render: |c, img| {
                let (lo, hi, cm) = (c.f("lumaMin") as f32, c.f("lumaMax") as f32, c.f("chromaMax") as f32);
                let signal = c.e("method") == 1;
                map_color(img, move |v| {
                    if signal {
                        return v.map(|x| x.clamp(lo, hi));
                    }
                    let y = 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2];
                    let yc = y.clamp(lo, hi);
                    let mut d = [v[0] - y, v[1] - y, v[2] - y];
                    let ch = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                    if ch > cm && ch > 1e-6 {
                        let k = cm / ch;
                        d = d.map(|x| x * k);
                    }
                    [yc + d[0], yc + d[1], yc + d[2]]
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.psArbitraryMap",
            name: "PS Arbitrary Map",
            category: CAT,
            params: vec![slider("phase", "Phase", 0.0, 0.0, 255.0, 0), check("mapAlpha", "Apply Phase Map To Alpha", false), hidden("map", "Map", "")],
            render: arbitrary_map,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.ccColorOffset",
            name: "CC Color Offset",
            category: CAT,
            params: vec![angle("red", "Red Phase", 0.0), angle("green", "Green Phase", 0.0), angle("blue", "Blue Phase", 0.0), popup("overflow", "Overflow", 0, &["Wrap", "Solarize", "Polarize"])],
            render: |c, img| {
                let ph = [(c.f("red") / 360.0) as f32, (c.f("green") / 360.0) as f32, (c.f("blue") / 360.0) as f32];
                let ov = c.e("overflow");
                map_color(img, move |v| {
                    let mut o = [0.0; 3];
                    for k in 0..3 {
                        let x = v[k] + ph[k];
                        o[k] = match ov {
                            1 => {
                                let f = x.rem_euclid(2.0);
                                if f > 1.0 { 2.0 - f } else { f }
                            }
                            2 => (x * std::f32::consts::PI).sin().abs(),
                            _ => x.rem_euclid(1.0),
                        };
                    }
                    o
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.ccToner",
            name: "CC Toner",
            category: CAT,
            params: vec![
                popup("tones", "Tones", 1, &["Duotone", "Tritone", "Pentone", "Solid"]),
                color("highlights", "Highlights", [1.0, 1.0, 1.0]),
                color("brights", "Brights", [0.85, 0.75, 0.5]),
                color("midtones", "Midtones", [0.6, 0.45, 0.3]),
                color("darktones", "Darktones", [0.3, 0.2, 0.1]),
                color("shadows", "Shadows", [0.0, 0.0, 0.0]),
                pct("blend", "Blend w. Original", 0.0),
            ],
            render: cc_toner,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.ccColorNeutralizer",
            name: "CC Color Neutralizer",
            category: CAT,
            params: vec![
                color("shadowsUnbalance", "Shadows Unbalance", [0.0, 0.0, 0.0]),
                color("midtonesUnbalance", "Midtones Unbalance", [0.5, 0.5, 0.5]),
                color("highlightsUnbalance", "Highlights Unbalance", [1.0, 1.0, 1.0]),
                slider("pinning", "Pinning", 0.0, 0.0, 100.0, 1),
                pct("blend", "Blend w. Original", 0.0),
            ],
            render: neutralizer,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.ccKernel",
            name: "CC Kernel",
            category: CAT,
            params: vec![
                text("kernel", "Kernel", "0 -1 0 -1 5 -1 0 -1 0"),
                slider("divisor", "Divisor", 1.0, -100.0, 100.0, 2),
                slider("offset", "Offset", 0.0, -1.0, 1.0, 3),
                check("absolute", "Absolute Value", false),
                pct("blend", "Blend w. Original", 0.0),
            ],
            render: cc_kernel,
            pad: |_| 2.0,
        },
        EffectSpec {
            id: "fx.color.colorStabilizer",
            name: "Color Stabilizer",
            category: CAT,
            params: vec![
                popup("stabilize", "Stabilize", 0, &["Brightness", "Levels", "Curves"]),
                slider("referenceFrame", "Set Frame (s)", 0.0, -100000.0, 100000.0, 2),
                point("blackPoint", "Black Point", 0.2, 0.5),
                point("midPoint", "Mid Point", 0.5, 0.5),
                point("whitePoint", "White Point", 0.8, 0.5),
                num("sampleSize", "Sample Size", 5.0, 1.0, 50.0, 1.0, 50.0),
            ],
            render: color_stabilizer,
            pad: no_pad,
        },
    ]
}

const PHASE_FROM: [&str; 10] = ["Intensity", "Red", "Green", "Blue", "Hue", "Lightness", "Saturation", "Value", "Alpha", "Zero"];

const PHOTO_FILTERS: [&str; 20] = [
    "Warming Filter (85)",
    "Warming Filter (LBA)",
    "Warming Filter (81)",
    "Cooling Filter (80)",
    "Cooling Filter (LBB)",
    "Cooling Filter (82)",
    "Red",
    "Orange",
    "Yellow",
    "Green",
    "Cyan",
    "Blue",
    "Violet",
    "Magenta",
    "Sepia",
    "Deep Red",
    "Deep Blue",
    "Deep Emerald",
    "Deep Yellow",
    "Custom",
];

const PHOTO_RGB: [[f32; 3]; 19] = [
    [0.925, 0.541, 0.0],
    [0.98, 0.588, 0.0],
    [0.922, 0.694, 0.075],
    [0.0, 0.427, 1.0],
    [0.0, 0.365, 1.0],
    [0.0, 0.71, 1.0],
    [0.918, 0.102, 0.102],
    [0.953, 0.518, 0.09],
    [0.976, 0.89, 0.11],
    [0.098, 0.788, 0.098],
    [0.114, 0.796, 0.918],
    [0.114, 0.208, 0.918],
    [0.608, 0.114, 0.918],
    [0.89, 0.094, 0.89],
    [0.675, 0.478, 0.2],
    [1.0, 0.0, 0.0],
    [0.0, 0.133, 0.804],
    [0.0, 0.553, 0.0],
    [1.0, 0.835, 0.0],
];

pub fn parse_curve(s: &str) -> Vec<(f32, f32)> {
    let mut pts: Vec<(f32, f32)> = s
        .split_whitespace()
        .filter_map(|p| {
            let mut it = p.split(',');
            Some((it.next()?.trim().parse().ok()?, it.next()?.trim().parse().ok()?))
        })
        .collect();
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-6);
    if pts.len() < 2 {
        return vec![(0.0, 0.0), (1.0, 1.0)];
    }
    pts
}

pub fn curve_lut(s: &str) -> Option<Vec<f32>> {
    let pts = parse_curve(s);
    if pts.len() == 2 && pts[0] == (0.0, 0.0) && pts[1] == (1.0, 1.0) {
        return None;
    }
    let n = pts.len();
    let mut m = vec![0.0f32; n];
    let dk: Vec<f32> = (0..n - 1).map(|i| (pts[i + 1].1 - pts[i].1) / (pts[i + 1].0 - pts[i].0).max(1e-6)).collect();
    m[0] = dk[0];
    m[n - 1] = dk[n - 2];
    for i in 1..n - 1 {
        m[i] = if dk[i - 1] * dk[i] <= 0.0 { 0.0 } else { (dk[i - 1] + dk[i]) * 0.5 };
    }
    for i in 0..n - 1 {
        if dk[i].abs() < 1e-7 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
            continue;
        }
        let a = m[i] / dk[i];
        let b = m[i + 1] / dk[i];
        let s2 = a * a + b * b;
        if s2 > 9.0 {
            let t = 3.0 / s2.sqrt();
            m[i] = t * a * dk[i];
            m[i + 1] = t * b * dk[i];
        }
    }
    let mut lut = vec![0.0f32; 1024];
    for (j, o) in lut.iter_mut().enumerate() {
        let x = j as f32 / 1023.0;
        *o = if x <= pts[0].0 {
            pts[0].1
        } else if x >= pts[n - 1].0 {
            pts[n - 1].1
        } else {
            let i = (0..n - 1).find(|i| x <= pts[i + 1].0).unwrap_or(n - 2);
            let hh = pts[i + 1].0 - pts[i].0;
            let t = (x - pts[i].0) / hh;
            let (t2, t3) = (t * t, t * t * t);
            (2.0 * t3 - 3.0 * t2 + 1.0) * pts[i].1 + (t3 - 2.0 * t2 + t) * hh * m[i] + (-2.0 * t3 + 3.0 * t2) * pts[i + 1].1 + (t3 - t2) * hh * m[i + 1]
        };
    }
    Some(lut)
}

#[inline]
pub fn lut_at(lut: &[f32], x: f32) -> f32 {
    let f = x.clamp(0.0, 1.0) * 1023.0;
    let i = f.floor() as usize;
    let j = (i + 1).min(1023);
    lut[i] + (lut[j] - lut[i]) * (f - i as f32)
}

fn curves(c: &FxCtx, img: &mut Img) {
    let luts: Vec<Option<Vec<f32>>> = ["rgb", "red", "green", "blue", "alpha"].iter().map(|k| curve_lut(&c.s(k))).collect();
    if luts.iter().all(|l| l.is_none()) {
        return;
    }
    img.px.par_chunks_mut(4).for_each(|p| {
        let a0 = p[3];
        let a = match &luts[4] {
            Some(l) => lut_at(l, a0),
            None => a0,
        };
        if a0 <= 1e-6 {
            p[3] = a;
            return;
        }
        let mut u = [p[0] / a0, p[1] / a0, p[2] / a0];
        for k in 0..3 {
            if let Some(l) = &luts[0] {
                u[k] = lut_at(l, u[k]);
            }
            if let Some(l) = &luts[k + 1] {
                u[k] = lut_at(l, u[k]);
            }
        }
        for k in 0..3 {
            p[k] = u[k] * a;
        }
        p[3] = a;
    });
}

fn lev(v: f32, ib: f32, iw: f32, g: f32, ob: f32, ow: f32, cb: bool, cw: bool) -> f32 {
    let x = (v - ib) / (iw - ib).max(1e-5);
    let x = if x > 0.0 { x.powf(1.0 / g.max(0.01)) } else { x };
    let mut y = ob + (ow - ob) * x;
    if cb {
        y = y.max(ob.min(ow));
    }
    if cw {
        y = y.min(ow.max(ob));
    }
    y
}

fn levels_ic(c: &FxCtx, img: &mut Img) {
    let get = |p: &str| [c.f(&format!("{p}/inBlack")) as f32, c.f(&format!("{p}/inWhite")) as f32, c.f(&format!("{p}/gamma")) as f32, c.f(&format!("{p}/outBlack")) as f32, c.f(&format!("{p}/outWhite")) as f32];
    let ch: Vec<[f32; 5]> = ["rgb", "red", "green", "blue", "alpha"].iter().map(|p| get(p)).collect();
    let (cb, cw) = (c.b("clipBlack"), c.b("clipWhite"));
    img.px.par_chunks_mut(4).for_each(|p| {
        let a0 = p[3];
        let a = lev(a0, ch[4][0], ch[4][1], ch[4][2], ch[4][3], ch[4][4], true, true);
        if a0 <= 1e-6 {
            p[3] = a;
            return;
        }
        for k in 0..3 {
            let u = p[k] / a0;
            let m = lev(u, ch[0][0], ch[0][1], ch[0][2], ch[0][3], ch[0][4], cb, cw);
            let x = &ch[k + 1];
            p[k] = lev(m, x[0], x[1], x[2], x[3], x[4], cb, cw) * a;
        }
        p[3] = a;
    });
}

fn tint_shift(c: [f32; 4]) -> [f32; 3] {
    [(c[0] - 0.5) * 0.6, (c[1] - 0.5) * 0.6, (c[2] - 0.5) * 0.6]
}

fn lumetri(c: &FxCtx, img: &mut Img) {
    super::ocio::lut_file(img, &c.s("basic/inputLut"), 1.0);
    lumetri_grade(c, img);
    super::ocio::lut_file(img, &c.s("creative/look"), c.f("creative/lookIntensity") as f32 / 100.0);
}

fn lumetri_grade(c: &FxCtx, img: &mut Img) {
    let f = |k: &str| c.f(k) as f32;
    let temp = f("basic/temperature") / 100.0;
    let tint = f("basic/tint") / 100.0;
    let expo = 2f32.powf(f("basic/exposure"));
    let con = f("basic/contrast") / 100.0;
    let (hi, sh, wh, bl) = (f("basic/highlights") / 100.0, f("basic/shadows") / 100.0, f("basic/whites") / 100.0, f("basic/blacks") / 100.0);
    let sat = f("basic/saturation") / 100.0 * f("creative/saturation") / 100.0;
    let faded = f("creative/fadedFilm") / 100.0;
    let vib = f("creative/vibrance") / 100.0;
    let (st, ht) = (tint_shift(c.c_raw("creative/shadowTint")), tint_shift(c.c_raw("creative/highlightTint")));
    let tb = f("creative/tintBalance") / 100.0;
    let luts: Vec<Option<Vec<f32>>> = ["curves/rgb", "curves/red", "curves/green", "curves/blue"].iter().map(|k| curve_lut(&c.s(k))).collect();
    let (ws, wm, whh) = (tint_shift(c.c_raw("wheels/shadows")), tint_shift(c.c_raw("wheels/midtones")), tint_shift(c.c_raw("wheels/highlights")));
    let (ls, lm, lh) = (f("wheels/shadowLightness") / 200.0, f("wheels/midtoneLightness") / 200.0, f("wheels/highlightLightness") / 200.0);
    let hsl_on = c.b("hsl/enable");
    let key_h = (c.f("hsl/hue") / 360.0) as f32;
    let range = f("hsl/hueRange") / 360.0;
    let (smin, lmin, lmax) = (f("hsl/satMin") / 100.0, f("hsl/lumMin") / 100.0, f("hsl/lumMax") / 100.0);
    let show = c.b("hsl/showMask");
    let corr = tint_shift(c.c_raw("hsl/correction"));
    let (ht2, hs2, hc2) = (f("hsl/temperature") / 100.0, f("hsl/saturation") / 100.0, f("hsl/contrast") / 100.0);
    let vig = f("vignette/amount");
    let mid = f("vignette/midpoint") / 100.0;
    let round = f("vignette/roundness") / 100.0;
    let feather = f("vignette/feather") / 100.0;
    let sharp = f("creative/sharpen") / 100.0;
    if sharp.abs() > 0.001 {
        let mut b = img.clone();
        gaussian(&mut b, c.px(1.5), c.px(1.5), true);
        img.px.par_iter_mut().zip(b.px.par_iter()).for_each(|(v, bb)| *v += (*v - *bb) * sharp * 2.0);
    }
    let w = img.w;
    let rect = c.layer_rect();
    let (cx, cy) = ((rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5);
    let (hw, hh) = (((rect[2] - rect[0]) * 0.5).max(1.0), ((rect[3] - rect[1]) * 0.5).max(1.0));
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let p = &mut row[x * 4..x * 4 + 4];
            let a = p[3];
            if a <= 1e-6 {
                continue;
            }
            let mut u = [p[0] / a, p[1] / a, p[2] / a];
            u[0] *= 1.0 + temp * 0.3;
            u[2] *= 1.0 - temp * 0.3;
            u[1] *= 1.0 - tint * 0.3;
            for v in u.iter_mut() {
                *v *= expo;
            }
            let l = lum(u[0], u[1], u[2]);
            let wsh = (1.0 - l / 0.5).clamp(0.0, 1.0);
            let whi = ((l - 0.5) / 0.5).clamp(0.0, 1.0);
            let dl = sh * 0.35 * wsh * (1.0 - wsh * 0.5) + hi * 0.35 * whi * (1.0 - whi * 0.5) + wh * 0.25 * l * l + bl * 0.25 * (1.0 - l) * (1.0 - l);
            for v in u.iter_mut() {
                *v += dl;
                *v = (*v - 0.5) * (1.0 + con) + 0.5;
            }
            if faded > 0.0 {
                for v in u.iter_mut() {
                    *v = *v * (1.0 - faded * 0.25) + faded * 0.12;
                }
            }
            let l = lum(u[0], u[1], u[2]);
            for k in 0..3 {
                u[k] = l + (u[k] - l) * sat;
            }
            if vib.abs() > 1e-4 {
                let mx = u[0].max(u[1]).max(u[2]);
                let mn = u[0].min(u[1]).min(u[2]);
                let s = (mx - mn).clamp(0.0, 1.0);
                let k = 1.0 + vib * (1.0 - s);
                for v in u.iter_mut() {
                    *v = l + (*v - l) * k;
                }
            }
            let l = lum(u[0], u[1], u[2]).clamp(0.0, 1.0);
            let wsh = ((1.0 - l) * (1.0 + tb)).clamp(0.0, 1.0);
            let whi = (l * (1.0 - tb)).clamp(0.0, 1.0);
            let wmid = 1.0 - (2.0 * l - 1.0).abs();
            for k in 0..3 {
                u[k] += st[k] * wsh * 0.5 + ht[k] * whi * 0.5;
                u[k] += ws[k] * (1.0 - l) * (1.0 - l) + wm[k] * wmid + whh[k] * l * l;
                u[k] += ls * (1.0 - l) * (1.0 - l) * 2.0 + lm * wmid + lh * l * l * 2.0;
            }
            for k in 0..3 {
                if let Some(lt) = &luts[0] {
                    u[k] = lut_at(lt, u[k]);
                }
                if let Some(lt) = &luts[k + 1] {
                    u[k] = lut_at(lt, u[k]);
                }
            }
            if hsl_on {
                let hs = rgb_to_hsl([u[0].clamp(0.0, 1.0), u[1].clamp(0.0, 1.0), u[2].clamp(0.0, 1.0)]);
                let dh = (hs[0] - key_h).abs();
                let dh = dh.min(1.0 - dh);
                let m = (1.0 - smooth(range * 0.5, range * 0.5 + 0.03, dh)) * smooth(smin - 0.05, smin, hs[1]) * smooth(lmin - 0.05, lmin, hs[2]) * (1.0 - smooth(lmax, lmax + 0.05, hs[2]));
                if show {
                    let g = lum(u[0], u[1], u[2]);
                    u = if m > 0.01 { [u[0] * m + g * (1.0 - m), u[1] * m + g * (1.0 - m), u[2] * m + g * (1.0 - m)] } else { [0.5, 0.5, 0.5] };
                } else if m > 0.0 {
                    let mut v = u;
                    v[0] *= 1.0 + ht2 * 0.3;
                    v[2] *= 1.0 - ht2 * 0.3;
                    let g = lum(v[0], v[1], v[2]);
                    for k in 0..3 {
                        v[k] = g + (v[k] - g) * hs2;
                        v[k] = (v[k] - 0.5) * (1.0 + hc2) + 0.5 + corr[k];
                        u[k] += (v[k] - u[k]) * m;
                    }
                }
            }
            if vig.abs() > 1e-4 {
                let dx = ((x as f64 + 0.5 - cx) / hw) as f32;
                let dy = ((y as f64 + 0.5 - cy) / hh) as f32;
                let rx = dx * (1.0 + round.min(0.0) * -0.5);
                let ry = dy * (1.0 + round.max(0.0) * 0.5);
                let d = (rx * rx + ry * ry).sqrt();
                let edge = smooth(mid, mid + feather.max(0.02), d);
                let k = if vig < 0.0 { 1.0 - edge * (-vig / 5.0).min(1.0) } else { 1.0 };
                for v in u.iter_mut() {
                    *v = if vig > 0.0 { *v + (1.0 - *v) * edge * (vig / 5.0) } else { *v * k };
                }
            }
            for k in 0..3 {
                p[k] = u[k].max(0.0) * a;
            }
        }
    });
}

fn match_dist(u: [f32; 3], k: [f32; 3], mode: u32) -> f32 {
    match mode {
        1 => {
            let a = rgb_to_hsl(u);
            let b = rgb_to_hsl(k);
            let d = (a[0] - b[0]).abs();
            d.min(1.0 - d) * 2.0
        }
        2 => {
            let ya = 0.299 * u[0] + 0.587 * u[1] + 0.114 * u[2];
            let yb = 0.299 * k[0] + 0.587 * k[1] + 0.114 * k[2];
            (((u[2] - ya) - (k[2] - yb)).powi(2) + ((u[0] - ya) - (k[0] - yb)).powi(2)).sqrt()
        }
        _ => ((u[0] - k[0]).powi(2) + (u[1] - k[1]).powi(2) + (u[2] - k[2]).powi(2)).sqrt() / 3f32.sqrt(),
    }
}

fn soft_match(d: f32, tol: f32, soft: f32) -> f32 {
    if d <= tol {
        1.0
    } else if soft <= 1e-5 {
        0.0
    } else {
        1.0 - ((d - tol) / soft).clamp(0.0, 1.0)
    }
}

fn change_color(c: &FxCtx, img: &mut Img) {
    let k = c.c("colorToChange");
    let k = [k[0], k[1], k[2]];
    let (dh, dl, ds) = ((c.f("hueTransform") / 360.0) as f32, c.f("lightnessTransform") as f32 / 100.0, c.f("saturationTransform") as f32 / 100.0);
    let (tol, soft) = (c.f("tolerance") as f32 / 100.0, c.f("softness") as f32 / 100.0);
    let mode = c.e("matchColors");
    let inv = c.b("invert");
    let show = c.e("view") == 1;
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let u = [p[0] / a, p[1] / a, p[2] / a];
        let mut m = soft_match(match_dist(u, k, mode), tol, soft);
        if inv {
            m = 1.0 - m;
        }
        if show {
            p[0] = m;
            p[1] = m;
            p[2] = m;
            p[3] = 1.0;
            return;
        }
        let mut h = rgb_to_hsl(u);
        h[0] = (h[0] + dh).rem_euclid(1.0);
        h[1] = (h[1] + h[1] * ds).clamp(0.0, 1.0);
        h[2] = (h[2] + dl * if dl > 0.0 { 1.0 - h[2] } else { h[2] }).clamp(0.0, 1.0);
        let o = hsl_to_rgb(h);
        for i in 0..3 {
            p[i] = (u[i] + (o[i] - u[i]) * m) * a;
        }
    });
}

fn change_to_color(c: &FxCtx, img: &mut Img) {
    let fr = c.c("from");
    let to = c.c("to");
    let fh = rgb_to_hsl([fr[0], fr[1], fr[2]]);
    let th = rgb_to_hsl([to[0], to[1], to[2]]);
    let ch = c.e("change");
    let transform = c.e("changeBy") == 1;
    let (tolh, toll, tols) = (c.f("toleranceHue") as f32 / 100.0, c.f("toleranceLightness") as f32 / 100.0, c.f("toleranceSaturation") as f32 / 100.0);
    let soft = c.f("softness") as f32 / 100.0;
    let view = c.b("viewMatte");
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let u = [p[0] / a, p[1] / a, p[2] / a];
        let h = rgb_to_hsl(u);
        let dh = (h[0] - fh[0]).abs();
        let dh = dh.min(1.0 - dh) * 2.0;
        let m = soft_match(dh, tolh, soft * 0.5) * soft_match((h[2] - fh[2]).abs(), toll, soft * 0.5) * soft_match((h[1] - fh[1]).abs(), tols, soft * 0.5);
        if view {
            p[0] = m;
            p[1] = m;
            p[2] = m;
            p[3] = 1.0;
            return;
        }
        let mut n = h;
        if transform {
            n[0] = (h[0] + th[0] - fh[0]).rem_euclid(1.0);
            if ch == 1 || ch == 3 {
                n[2] = (h[2] + th[2] - fh[2]).clamp(0.0, 1.0);
            }
            if ch == 2 || ch == 3 {
                n[1] = (h[1] + th[1] - fh[1]).clamp(0.0, 1.0);
            }
        } else {
            n[0] = th[0];
            if ch == 1 || ch == 3 {
                n[2] = th[2];
            }
            if ch == 2 || ch == 3 {
                n[1] = th[1];
            }
        }
        let o = hsl_to_rgb(n);
        for i in 0..3 {
            p[i] = (u[i] + (o[i] - u[i]) * m) * a;
        }
    });
}

fn leave_color(c: &FxCtx, img: &mut Img) {
    let k = c.c("colorToLeave");
    let k = [k[0], k[1], k[2]];
    let amt = c.f("amount") as f32 / 100.0;
    let (tol, soft) = (c.f("tolerance") as f32 / 100.0, c.f("softness") as f32 / 100.0);
    let mode = c.e("matchColors");
    map_color(img, move |u| {
        let keep = soft_match(match_dist(u, k, mode), tol, soft);
        let g = lum(u[0], u[1], u[2]);
        let t = amt * (1.0 - keep);
        [u[0] + (g - u[0]) * t, u[1] + (g - u[1]) * t, u[2] + (g - u[2]) * t]
    });
}

fn palette(c: &FxCtx) -> Vec<(f32, [f32; 3])> {
    match c.e("output/palette") {
        1 => vec![(0.0, [0.0; 3]), (1.0, [1.0; 3])],
        2 => vec![(0.0, [0.0; 3]), (0.33, [0.8, 0.0, 0.0]), (0.66, [1.0, 0.6, 0.0]), (1.0, [1.0, 1.0, 0.8])],
        3 => vec![(0.0, [0.0; 3]), (0.33, [0.0, 0.0, 0.6]), (0.66, [0.3, 0.7, 1.0]), (1.0, [1.0; 3])],
        4 => vec![(0.0, [0.0; 3]), (1.0, [1.0, 0.0, 0.0])],
        5 => vec![(0.0, [0.0; 3]), (1.0, [0.0, 1.0, 0.0])],
        6 => vec![(0.0, [0.0; 3]), (1.0, [0.0, 0.0, 1.0])],
        7 => vec![(0.0, [1.0, 0.7, 0.7]), (0.33, [0.7, 1.0, 0.7]), (0.66, [0.7, 0.7, 1.0]), (1.0, [1.0, 0.7, 0.7])],
        8 => {
            let mut v: Vec<(f32, [f32; 3])> = c
                .s("output/custom")
                .split_whitespace()
                .filter_map(|e| {
                    let (t, rgb) = e.split_once(':')?;
                    let cs: Vec<f32> = rgb.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    Some((t.parse().ok()?, [*cs.first()?, *cs.get(1)?, *cs.get(2)?]))
                })
                .collect();
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            if v.is_empty() { vec![(0.0, [0.0; 3]), (1.0, [1.0; 3])] } else { v }
        }
        _ => (0..=6).map(|i| (i as f32 / 6.0, hsl_to_rgb([i as f32 / 6.0, 1.0, 0.5]))).collect(),
    }
}

fn pal_at(p: &[(f32, [f32; 3])], t: f32, interp: bool) -> [f32; 3] {
    let t = t.rem_euclid(1.0);
    for i in 0..p.len() {
        if t < p[i].0 || i == p.len() - 1 {
            if i == 0 {
                return p[0].1;
            }
            let (a, b) = (p[i - 1], p[i]);
            if !interp || t >= b.0 {
                return if t >= b.0 { b.1 } else { a.1 };
            }
            let f = (t - a.0) / (b.0 - a.0).max(1e-6);
            return [a.1[0] + (b.1[0] - a.1[0]) * f, a.1[1] + (b.1[1] - a.1[1]) * f, a.1[2] + (b.1[2] - a.1[2]) * f];
        }
    }
    p[p.len() - 1].1
}

fn colorama(c: &FxCtx, img: &mut Img) {
    let pal = palette(c);
    let from = c.e("input/getPhaseFrom");
    let shift = (c.f("input/phaseShift") / 360.0) as f32;
    let cycles = c.f("output/cycles") as f32;
    let interp = c.b("output/interpolate");
    let modify = c.e("modify/channels");
    let blend = c.f("blend") as f32 / 100.0;
    let mod_alpha = c.b("modify/alpha");
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let u = [p[0] / a, p[1] / a, p[2] / a];
        let hs = rgb_to_hsl(u);
        let ph = match from {
            1 => u[0],
            2 => u[1],
            3 => u[2],
            4 => hs[0],
            5 => hs[2],
            6 => hs[1],
            7 => u[0].max(u[1]).max(u[2]),
            8 => a,
            9 => 0.0,
            _ => lum(u[0], u[1], u[2]),
        };
        let col = pal_at(&pal, ph * cycles + shift, interp);
        let mut o = u;
        match modify {
            1 => o[0] = col[0],
            2 => o[1] = col[1],
            3 => o[2] = col[2],
            4 => {
                let mut h = hs;
                h[0] = rgb_to_hsl(col)[0];
                o = hsl_to_rgb(h);
            }
            5 => {
                let mut h = hs;
                h[2] = rgb_to_hsl(col)[2];
                o = hsl_to_rgb(h);
            }
            6 => {
                let mut h = hs;
                h[1] = rgb_to_hsl(col)[1];
                o = hsl_to_rgb(h);
            }
            _ => o = col,
        }
        let na = if mod_alpha { lum(col[0], col[1], col[2]) * a } else { a };
        for i in 0..3 {
            p[i] = (o[i] + (u[i] - o[i]) * blend) * na;
        }
        p[3] = na;
    });
}

fn photo_filter(c: &FxCtx, img: &mut Img) {
    let f = c.e("filter") as usize;
    let col = if f < PHOTO_RGB.len() {
        PHOTO_RGB[f]
    } else {
        let x = c.c("color");
        [x[0], x[1], x[2]]
    };
    let d = c.f("density") as f32 / 100.0;
    let keep = c.b("preserveLuminosity");
    map_color(img, move |u| {
        let l0 = lum(u[0], u[1], u[2]);
        let mut o = [0.0; 3];
        for k in 0..3 {
            o[k] = u[k] + (u[k] * col[k] - u[k]) * d;
        }
        if keep {
            let l1 = lum(o[0], o[1], o[2]).max(1e-5);
            let s = l0 / l1;
            o = o.map(|x| x * s);
        }
        o
    });
}

fn percentile(h: &[u32; 1024], total: u64, q: f64) -> f32 {
    let target = (total as f64 * q) as u64;
    let mut acc = 0u64;
    for (i, n) in h.iter().enumerate() {
        acc += *n as u64;
        if acc > target {
            return i as f32 / 1023.0;
        }
    }
    1.0
}

fn auto_levels(c: &FxCtx, img: &mut Img, mode: u32) {
    let mut hist = [[0u32; 1024]; 4];
    let mut total = 0u64;
    for p in img.px.chunks_exact(4) {
        if p[3] <= 0.5 {
            continue;
        }
        let u = [p[0] / p[3], p[1] / p[3], p[2] / p[3]];
        for k in 0..3 {
            hist[k][(u[k].clamp(0.0, 1.0) * 1023.0) as usize] += 1;
        }
        hist[3][(lum(u[0], u[1], u[2]).clamp(0.0, 1.0) * 1023.0) as usize] += 1;
        total += 1;
    }
    if total == 0 {
        return;
    }
    let (bc, wc) = (c.f("blackClip") / 100.0, c.f("whiteClip") / 100.0);
    let range = |h: &[u32; 1024]| (percentile(h, total, bc), percentile(h, total, 1.0 - wc));
    let ranges: Vec<(f32, f32)> = match mode {
        0 => {
            let r = range(&hist[3]);
            vec![r, r, r]
        }
        _ => (0..3).map(|k| range(&hist[k])).collect(),
    };
    let snap = mode == 2 && c.b("snapNeutral");
    let blend = c.f("blend") as f32 / 100.0;
    let mut gamma = [1.0f32; 3];
    if snap {
        let mid: Vec<f32> = (0..3).map(|k| {
            let (lo, hi) = ranges[k];
            ((percentile(&hist[k], total, 0.5) - lo) / (hi - lo).max(1e-3)).clamp(0.05, 0.95)
        }).collect();
        let avg = (mid[0] + mid[1] + mid[2]) / 3.0;
        for k in 0..3 {
            gamma[k] = (avg.ln() / mid[k].ln()).clamp(0.3, 3.0);
        }
    }
    map_color(img, move |u| {
        let mut o = [0.0; 3];
        for k in 0..3 {
            let (lo, hi) = ranges[k];
            let x = ((u[k] - lo) / (hi - lo).max(1e-3)).clamp(0.0, 1.0).powf(gamma[k]);
            o[k] = x + (u[k] - x) * blend;
        }
        o
    });
}

fn shadow_highlight(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let lumv: Vec<f32> = img.px.chunks_exact(4).map(|p| if p[3] > 1e-6 { lum(p[0] / p[3], p[1] / p[3], p[2] / p[3]) } else { 0.0 }).collect();
    let mut sa = c.f("shadowAmount") as f32 / 100.0;
    let mut ha = c.f("highlightAmount") as f32 / 100.0;
    if c.b("auto") {
        let mean = lumv.iter().sum::<f32>() / lumv.len().max(1) as f32;
        sa = ((0.5 - mean) * 1.6).clamp(0.0, 0.8);
        ha = ((mean - 0.5) * 1.6).clamp(0.0, 0.8);
    }
    let mut ls = lumv.clone();
    let r = c.px(c.f("more/shadowRadius")).max(0.5);
    super::util::blur_scalar(&mut ls, w, h, r * 0.5, r * 0.5);
    let mut lh = lumv.clone();
    let r2 = c.px(c.f("more/highlightRadius")).max(0.5);
    super::util::blur_scalar(&mut lh, w, h, r2 * 0.5, r2 * 0.5);
    let stw = (c.f("more/shadowTonalWidth") as f32 / 100.0).max(0.02);
    let htw = (c.f("more/highlightTonalWidth") as f32 / 100.0).max(0.02);
    let cc = 1.0 + c.f("more/colorCorrection") as f32 / 100.0;
    let mc = c.f("more/midtoneContrast") as f32 / 100.0;
    let blend = c.f("blend") as f32 / 100.0;
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let u = [p[0] / a, p[1] / a, p[2] / a];
        let l = lumv[i];
        let sw = (1.0 - ls[i] / stw).clamp(0.0, 1.0);
        let hw = ((lh[i] - (1.0 - htw)) / htw).clamp(0.0, 1.0);
        let mut nl = l + sa * sw * (1.0 - l) * 0.8 - ha * hw * l * 0.8;
        nl = 0.5 + (nl - 0.5) * (1.0 + mc);
        let k = if l > 1e-4 { nl / l } else { 1.0 };
        let mut o = [0.0f32; 3];
        for j in 0..3 {
            let x = u[j] * k;
            o[j] = nl + (x - nl) * cc;
            o[j] = o[j] + (u[j] - o[j]) * blend;
            p[j] = o[j].max(0.0) * a;
        }
    });
}

fn selective_color(c: &FxCtx, img: &mut Img) {
    let abs = c.e("method") == 1;
    let adj: Vec<[f32; 4]> = SEL
        .iter()
        .map(|s| ["cyan", "magenta", "yellow", "black"].map(|k| c.f(&format!("{s}/{k}")) as f32 / 100.0))
        .collect();
    if adj.iter().all(|a| a.iter().all(|v| v.abs() < 1e-6)) {
        return;
    }
    map_color(img, move |u| {
        let mx = u[0].max(u[1]).max(u[2]);
        let mn = u[0].min(u[1]).min(u[2]);
        let mid = u[0] + u[1] + u[2] - mx - mn;
        let mut w = [0.0f32; 9];
        let hue_w = mx - mid;
        let sec_w = mid - mn;
        let (ri, gi, bi) = (u[0] == mx, u[1] == mx, u[2] == mx);
        if ri {
            w[0] = hue_w;
        } else if gi {
            w[2] = hue_w;
        } else if bi {
            w[4] = hue_w;
        }
        let rmin = u[0] == mn;
        let gmin = u[1] == mn;
        if rmin {
            w[3] = sec_w;
        } else if gmin {
            w[5] = sec_w;
        } else {
            w[1] = sec_w;
        }
        let l = (u[0] + u[1] + u[2]) / 3.0;
        w[6] = ((mn - 0.5) * 2.0).max(0.0);
        w[8] = ((0.5 - mx) * 2.0).max(0.0);
        w[7] = 1.0 - (mx - mn) - w[6] - w[8];
        w[7] = w[7].max(0.0) * (1.0 - (l - 0.5).abs() * 2.0).max(0.0);
        let mut cmy = [1.0 - u[0], 1.0 - u[1], 1.0 - u[2]];
        for (i, a) in adj.iter().enumerate() {
            if w[i] <= 0.0 {
                continue;
            }
            for k in 0..3 {
                let base = if abs { 1.0 } else { cmy[k] };
                let delta = (a[k] + a[3]) * base;
                cmy[k] = (cmy[k] + delta * w[i]).clamp(0.0, 1.0);
            }
        }
        [1.0 - cmy[0], 1.0 - cmy[1], 1.0 - cmy[2]]
    });
}

fn broadcast(c: &FxCtx, img: &mut Img) {
    let pal = c.e("locale") == 1;
    let lim = c.f("maxAmplitude") as f32 / 100.0;
    let method = c.e("method");
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let mut u = [p[0] / a, p[1] / a, p[2] / a];
        let y = if pal { 0.299 * u[0] + 0.587 * u[1] + 0.114 * u[2] } else { 0.2126 * u[0] + 0.7152 * u[1] + 0.0722 * u[2] };
        let ch = ((u[0] - y).powi(2) + (u[1] - y).powi(2) + (u[2] - y).powi(2)).sqrt();
        let amp = y + ch * 0.9;
        match method {
            2 => {
                if amp > lim {
                    p.iter_mut().for_each(|v| *v = 0.0);
                }
                return;
            }
            3 => {
                if amp <= lim {
                    p.iter_mut().for_each(|v| *v = 0.0);
                }
                return;
            }
            _ => {}
        }
        if amp > lim {
            if method == 1 {
                let k = ((lim - y) / (ch * 0.9).max(1e-5)).clamp(0.0, 1.0);
                u = u.map(|v| y + (v - y) * k);
            } else {
                let k = lim / amp;
                u = u.map(|v| v * k);
            }
        }
        for i in 0..3 {
            p[i] = u[i] * a;
        }
    });
}

fn equalize(c: &FxCtx, img: &mut Img) {
    let style = c.e("style");
    let amt = c.f("amount") as f32 / 100.0;
    let mut hist = [[0u32; 256]; 4];
    let mut total = 0u32;
    for p in img.px.chunks_exact(4) {
        if p[3] <= 0.5 {
            continue;
        }
        let u = [p[0] / p[3], p[1] / p[3], p[2] / p[3]];
        for k in 0..3 {
            hist[k][(u[k].clamp(0.0, 1.0) * 255.0) as usize] += 1;
        }
        hist[3][(lum(u[0], u[1], u[2]).clamp(0.0, 1.0) * 255.0) as usize] += 1;
        total += 1;
    }
    if total == 0 {
        return;
    }
    let cdf = |h: &[u32; 256]| -> [f32; 256] {
        let mut o = [0.0f32; 256];
        let mut acc = 0u32;
        for i in 0..256 {
            acc += h[i];
            o[i] = acc as f32 / total as f32;
        }
        o
    };
    let maps: Vec<[f32; 256]> = (0..4).map(|k| cdf(&hist[k])).collect();
    map_color(img, move |u| {
        let mut o = u;
        match style {
            1 | 2 => {
                let l = lum(u[0], u[1], u[2]).clamp(0.0, 1.0);
                let nl = maps[3][(l * 255.0) as usize];
                let k = if l > 1e-4 { nl / l } else { 1.0 };
                o = if style == 2 { u.map(|v| (v * k).min(1.0)) } else { u.map(|v| v * k) };
            }
            _ => {
                for k in 0..3 {
                    o[k] = maps[k][(u[k].clamp(0.0, 1.0) * 255.0) as usize];
                }
            }
        }
        [u[0] + (o[0] - u[0]) * amt, u[1] + (o[1] - u[1]) * amt, u[2] + (o[2] - u[2]) * amt]
    });
}

fn color_link(c: &FxCtx, img: &mut Img) {
    let Some((src, _)) = c.other("source", true) else { return };
    let mode = c.e("sample");
    let clip = c.f("clip") / 100.0;
    let mut cols: Vec<[f32; 4]> = src.px.chunks_exact(4).filter(|p| p[3] > 1e-4 || mode >= 6).map(|p| [p[0], p[1], p[2], p[3]]).collect();
    if cols.is_empty() {
        return;
    }
    let lumk = |p: &[f32; 4]| if p[3] > 1e-6 { lum(p[0] / p[3], p[1] / p[3], p[2] / p[3]) } else { 0.0 };
    let pick = |v: &mut Vec<[f32; 4]>, bright: bool| -> [f32; 4] {
        v.sort_by(|a, b| lumk(a).total_cmp(&lumk(b)));
        let n = v.len();
        let skip = ((n as f64) * clip) as usize;
        let i = if bright { n.saturating_sub(1 + skip) } else { skip.min(n - 1) };
        v[i]
    };
    let s: [f32; 4] = match mode {
        1 => {
            cols.sort_by(|a, b| lumk(a).total_cmp(&lumk(b)));
            cols[cols.len() / 2]
        }
        2 => pick(&mut cols, true),
        3 => pick(&mut cols, false),
        4 => {
            let mut m = [0.0f32; 4];
            for p in &cols {
                for k in 0..4 {
                    m[k] = m[k].max(p[k]);
                }
            }
            m
        }
        5 => {
            let mut m = [f32::MAX; 4];
            for p in &cols {
                for k in 0..4 {
                    m[k] = m[k].min(p[k]);
                }
            }
            m
        }
        _ => {
            let mut m = [0.0f64; 4];
            for p in &cols {
                for k in 0..4 {
                    m[k] += p[k] as f64;
                }
            }
            let n = cols.len() as f64;
            [(m[0] / n) as f32, (m[1] / n) as f32, (m[2] / n) as f32, (m[3] / n) as f32]
        }
    };
    let su = if s[3] > 1e-6 { [s[0] / s[3], s[1] / s[3], s[2] / s[3]] } else { [0.0; 3] };
    let su = match mode {
        7 | 8 => [s[3]; 3],
        _ => su,
    };
    let op = c.f("opacity") as f32 / 100.0;
    let stencil = c.b("stencil");
    let blend = c.e("mode");
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        let u = if a > 1e-6 { [p[0] / a, p[1] / a, p[2] / a] } else { [0.0; 3] };
        let mut o = [0.0f32; 3];
        for k in 0..3 {
            o[k] = match blend {
                1 => u[k] * su[k],
                2 => 1.0 - (1.0 - u[k]) * (1.0 - su[k]),
                3 => {
                    if u[k] < 0.5 {
                        2.0 * u[k] * su[k]
                    } else {
                        1.0 - 2.0 * (1.0 - u[k]) * (1.0 - su[k])
                    }
                }
                _ => su[k],
            };
        }
        if blend == 4 || blend == 5 {
            let mut hl = rgb_to_hsl(u);
            let sh = rgb_to_hsl(su);
            hl[0] = sh[0];
            if blend == 4 {
                hl[1] = sh[1];
            }
            o = hsl_to_rgb(hl);
        }
        let na = if stencil { a } else { a + (1.0 - a) * op };
        for k in 0..3 {
            let base = u[k] * a;
            p[k] = base + (o[k] * na - base) * op;
        }
        p[3] = na;
    });
}

fn arbitrary_map(c: &FxCtx, img: &mut Img) {
    let vals: Vec<f32> = c.s("map").split(|ch: char| ch == ',' || ch.is_whitespace()).filter_map(|x| x.trim().parse::<f32>().ok()).collect();
    let phase = c.f("phase") as f32 / 255.0;
    let lut: Vec<f32> = if vals.len() >= 2 {
        let mx = vals.iter().cloned().fold(0.0f32, f32::max);
        let k = if mx > 1.5 { 255.0 } else { 1.0 };
        vals.iter().map(|v| v / k).collect()
    } else {
        (0..256).map(|i| 1.0 - i as f32 / 255.0).collect()
    };
    let n = lut.len();
    let at = |x: f32| -> f32 {
        let f = (x + phase).rem_euclid(1.0001).min(1.0) * (n - 1) as f32;
        let i = f.floor() as usize;
        let j = (i + 1).min(n - 1);
        lut[i] + (lut[j] - lut[i]) * (f - i as f32)
    };
    let alpha = c.b("mapAlpha");
    img.px.par_chunks_mut(4).for_each(|p| {
        let a0 = p[3];
        let a = if alpha { at(a0) } else { a0 };
        if a0 <= 1e-6 {
            p[3] = a;
            return;
        }
        for k in 0..3 {
            p[k] = at(p[k] / a0) * a;
        }
        p[3] = a;
    });
}

fn cc_toner(c: &FxCtx, img: &mut Img) {
    let tones = c.e("tones");
    let hl = c.c("highlights");
    let br = c.c("brights");
    let md = c.c("midtones");
    let dk = c.c("darktones");
    let sh = c.c("shadows");
    let stops: Vec<(f32, [f32; 3])> = match tones {
        0 => vec![(0.0, [sh[0], sh[1], sh[2]]), (1.0, [hl[0], hl[1], hl[2]])],
        2 => vec![(0.0, [sh[0], sh[1], sh[2]]), (0.25, [dk[0], dk[1], dk[2]]), (0.5, [md[0], md[1], md[2]]), (0.75, [br[0], br[1], br[2]]), (1.0, [hl[0], hl[1], hl[2]])],
        3 => vec![(0.0, [md[0], md[1], md[2]]), (1.0, [md[0], md[1], md[2]])],
        _ => vec![(0.0, [sh[0], sh[1], sh[2]]), (0.5, [md[0], md[1], md[2]]), (1.0, [hl[0], hl[1], hl[2]])],
    };
    let blend = c.f("blend") as f32 / 100.0;
    map_color(img, move |u| {
        let l = lum(u[0], u[1], u[2]).clamp(0.0, 0.9999);
        let o = pal_at(&stops, l, true);
        let o = if tones == 3 {
            let k = l / lum(o[0], o[1], o[2]).max(1e-4);
            o.map(|v| v * k)
        } else {
            o
        };
        [o[0] + (u[0] - o[0]) * blend, o[1] + (u[1] - o[1]) * blend, o[2] + (u[2] - o[2]) * blend]
    });
}

fn neutralizer(c: &FxCtx, img: &mut Img) {
    let s = c.c("shadowsUnbalance");
    let m = c.c_raw("midtonesUnbalance");
    let h = c.c("highlightsUnbalance");
    let blend = c.f("blend") as f32 / 100.0;
    map_color(img, move |u| {
        let mut o = [0.0f32; 3];
        for k in 0..3 {
            let lo = s[k] - (s[0] + s[1] + s[2]) / 3.0;
            let hi = h[k] - (h[0] + h[1] + h[2]) / 3.0;
            let mid = m[k] - (m[0] + m[1] + m[2]) / 3.0;
            let x = u[k];
            let corr = lo * (1.0 - x) * (1.0 - x) + mid * 4.0 * x * (1.0 - x) * 0.5 + hi * x * x;
            o[k] = x - corr;
            o[k] = o[k] + (x - o[k]) * blend;
        }
        o
    });
}

fn cc_kernel(c: &FxCtx, img: &mut Img) {
    let k: Vec<f32> = c.s("kernel").split_whitespace().filter_map(|x| x.parse().ok()).collect();
    if k.len() != 9 {
        return;
    }
    let div = { let d = c.f("divisor") as f32; if d.abs() < 1e-6 { 1.0 } else { d } };
    let off = c.f("offset") as f32;
    let absv = c.b("absolute");
    let blend = c.f("blend") as f32 / 100.0;
    let src = img.clone();
    let w = img.w as isize;
    img.px.par_chunks_mut(w as usize * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let mut acc = [0.0f32; 3];
            for j in -1..=1isize {
                for i in -1..=1isize {
                    let s = src.get_clamped(x + i, y as isize + j);
                    let kk = k[((j + 1) * 3 + i + 1) as usize];
                    for ch in 0..3 {
                        acc[ch] += s[ch] * kk;
                    }
                }
            }
            let a = row[x as usize * 4 + 3];
            for ch in 0..3 {
                let mut v = acc[ch] / div + off * a;
                if absv {
                    v = v.abs();
                }
                let o = row[x as usize * 4 + ch];
                row[x as usize * 4 + ch] = (v.clamp(0.0, a.max(0.0)) * (1.0 - blend)) + o * blend;
            }
        }
    });
}

fn sample_avg(img: &Img, p: [f64; 2], r: f64) -> [f32; 3] {
    let mut acc = [0.0f32; 3];
    let mut n = 0.0;
    let ri = r.max(1.0) as isize;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let s = img.get_clamped(p[0] as isize + dx, p[1] as isize + dy);
            if s[3] > 1e-6 {
                for k in 0..3 {
                    acc[k] += s[k] / s[3];
                }
                n += 1.0;
            }
        }
    }
    if n > 0.0 { acc.map(|v| v / n) } else { [0.0; 3] }
}

fn color_stabilizer(c: &FxCtx, img: &mut Img) {
    let dt = c.f("referenceFrame") - c.comp_time();
    if dt.abs() < 1e-6 {
        return;
    }
    let Some(refimg) = c.frame(dt) else { return };
    let r = c.px(c.f("sampleSize"));
    let pts = [c.pt("blackPoint"), c.pt("midPoint"), c.pt("whitePoint")];
    let cur: Vec<[f32; 3]> = pts.iter().map(|p| sample_avg(img, *p, r)).collect();
    let refc: Vec<[f32; 3]> = pts.iter().map(|p| sample_avg(&refimg, *p, r)).collect();
    let mode = c.e("stabilize");
    map_color(img, move |u| {
        let mut o = u;
        match mode {
            0 => {
                let lc = lum(cur[1][0], cur[1][1], cur[1][2]).max(1e-4);
                let lr = lum(refc[1][0], refc[1][1], refc[1][2]);
                o = u.map(|v| v * lr / lc);
            }
            1 => {
                for k in 0..3 {
                    let (cb, cw) = (cur[0][k], cur[2][k]);
                    let (rb, rw) = (refc[0][k], refc[2][k]);
                    let t = (u[k] - cb) / (cw - cb).abs().max(1e-3) * (cw - cb).signum();
                    o[k] = rb + t * (rw - rb);
                }
            }
            _ => {
                for k in 0..3 {
                    let xs = [cur[0][k], cur[1][k], cur[2][k]];
                    let ys = [refc[0][k], refc[1][k], refc[2][k]];
                    let x = u[k];
                    o[k] = if x <= xs[1] {
                        ys[0] + (x - xs[0]) * (ys[1] - ys[0]) / (xs[1] - xs[0]).abs().max(1e-3)
                    } else {
                        ys[1] + (x - xs[1]) * (ys[2] - ys[1]) / (xs[2] - xs[1]).abs().max(1e-3)
                    };
                }
            }
        }
        o
    });
}
