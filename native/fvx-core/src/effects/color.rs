use rayon::prelude::*;

use super::util::{hsl_to_rgb, lum, map_color, mix_with_original, rgb_to_hsl};
use super::{EffectSpec, FxCtx, angle, check, color, no_pad, num, p, pct, popup};
use crate::model::Value;
use crate::model::prop::Ui;
use crate::render::image::Img;

const CAT: &str = "Color Correction";

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.color.brightnessContrast",
            name: "Brightness & Contrast",
            category: CAT,
            params: vec![num("brightness", "Brightness", 0.0, -150.0, 150.0, -150.0, 150.0), num("contrast", "Contrast", 0.0, -100.0, 100.0, -100.0, 100.0)],
            render: |c, img| {
                let b = (c.f("brightness") / 150.0) as f32 * 0.5;
                let k = c.f("contrast") as f32 / 100.0;
                let k = if k >= 0.0 { 1.0 / (1.0 - k * 0.99) } else { 1.0 + k };
                map_color(img, |v| v.map(|x| ((x + b - 0.5) * k + 0.5).clamp(0.0, 1.0)));
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.hueSaturation",
            name: "Hue/Saturation",
            category: CAT,
            params: vec![
                angle("hue", "Master Hue", 0.0),
                num("saturation", "Master Saturation", 0.0, -100.0, 100.0, -100.0, 100.0),
                num("lightness", "Master Lightness", 0.0, -100.0, 100.0, -100.0, 100.0),
                check("colorize", "Colorize", false),
                angle("colorizeHue", "Colorize Hue", 0.0),
                num("colorizeSaturation", "Colorize Saturation", 25.0, 0.0, 100.0, 0.0, 100.0),
                num("colorizeLightness", "Colorize Lightness", 0.0, -100.0, 100.0, -100.0, 100.0),
            ],
            render: hue_sat,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.levels",
            name: "Levels",
            category: CAT,
            params: vec![
                popup("channel", "Channel", 0, &["RGB", "Red", "Green", "Blue", "Alpha"]),
                num("inputBlack", "Input Black", 0.0, -1000.0, 1000.0, 0.0, 255.0),
                num("inputWhite", "Input White", 255.0, -1000.0, 1000.0, 0.0, 255.0),
                p("gamma", "Gamma", Value::Num(1.0), Ui::slider(0.1, 10.0, 0.1, 3.0, 2)),
                num("outputBlack", "Output Black", 0.0, -1000.0, 1000.0, 0.0, 255.0),
                num("outputWhite", "Output White", 255.0, -1000.0, 1000.0, 0.0, 255.0),
            ],
            render: levels,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.tint",
            name: "Tint",
            category: CAT,
            params: vec![color("black", "Map Black To", [0.0, 0.0, 0.0]), color("white", "Map White To", [1.0, 1.0, 1.0]), pct("amount", "Amount to Tint", 100.0)],
            render: |c, img| {
                let (b, w, k) = (c.c("black"), c.c("white"), c.f("amount") as f32 / 100.0);
                map_color(img, |v| {
                    let l = lum(v[0], v[1], v[2]);
                    let t = [b[0] + (w[0] - b[0]) * l, b[1] + (w[1] - b[1]) * l, b[2] + (w[2] - b[2]) * l];
                    [v[0] + (t[0] - v[0]) * k, v[1] + (t[1] - v[1]) * k, v[2] + (t[2] - v[2]) * k]
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.tritone",
            name: "Tritone",
            category: CAT,
            params: vec![color("highlights", "Highlights", [1.0, 1.0, 1.0]), color("midtones", "Midtones", [0.6, 0.47, 0.35]), color("shadows", "Shadows", [0.0, 0.0, 0.0]), pct("blend", "Blend With Original", 0.0)],
            render: |c, img| {
                let orig = img.clone();
                let (h, m, s) = (c.c("highlights"), c.c("midtones"), c.c("shadows"));
                map_color(img, |v| {
                    let l = lum(v[0], v[1], v[2]);
                    let (a, b, t) = if l < 0.5 { (s, m, l * 2.0) } else { (m, h, (l - 0.5) * 2.0) };
                    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
                });
                mix_with_original(&orig, img, c.f("blend") as f32 / 100.0);
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.exposure",
            name: "Exposure",
            category: CAT,
            params: vec![
                p("exposure", "Exposure", Value::Num(0.0), Ui::slider(-20.0, 20.0, -5.0, 5.0, 2)),
                p("offset", "Offset", Value::Num(0.0), Ui::slider(-2.0, 2.0, -0.5, 0.5, 3)),
                p("gamma", "Gamma Correction", Value::Num(1.0), Ui::slider(0.1, 10.0, 0.1, 3.0, 2)),
            ],
            render: |c, img| {
                let k = 2f32.powf(c.f("exposure") as f32);
                let o = c.f("offset") as f32;
                let g = 1.0 / c.f("gamma").max(0.01) as f32;
                map_color(img, |v| v.map(|x| ((x * k + o).max(0.0)).powf(g)));
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.channel.invert",
            name: "Invert",
            category: "Channel",
            params: vec![popup("channel", "Channel", 0, &["RGB", "Red", "Green", "Blue", "Alpha", "Luminance"]), pct("blend", "Blend With Original", 0.0)],
            render: |c, img| {
                let orig = img.clone();
                let ch = c.e("channel");
                if ch == 4 {
                    img.px.par_chunks_mut(4).for_each(|p| {
                        let a = p[3];
                        let na = 1.0 - a;
                        let k = if a > 1e-6 { na / a } else { 0.0 };
                        p[0] *= k;
                        p[1] *= k;
                        p[2] *= k;
                        p[3] = na;
                    });
                } else {
                    map_color(img, |v| match ch {
                        1 => [1.0 - v[0], v[1], v[2]],
                        2 => [v[0], 1.0 - v[1], v[2]],
                        3 => [v[0], v[1], 1.0 - v[2]],
                        5 => {
                            let l = lum(v[0], v[1], v[2]);
                            let d = 1.0 - 2.0 * l;
                            [(v[0] + d).clamp(0.0, 1.0), (v[1] + d).clamp(0.0, 1.0), (v[2] + d).clamp(0.0, 1.0)]
                        }
                        _ => [1.0 - v[0], 1.0 - v[1], 1.0 - v[2]],
                    });
                }
                mix_with_original(&orig, img, c.f("blend") as f32 / 100.0);
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.blackWhite",
            name: "Black & White",
            category: CAT,
            params: vec![
                num("reds", "Reds", 40.0, -200.0, 300.0, -200.0, 300.0),
                num("yellows", "Yellows", 60.0, -200.0, 300.0, -200.0, 300.0),
                num("greens", "Greens", 40.0, -200.0, 300.0, -200.0, 300.0),
                num("cyans", "Cyans", 60.0, -200.0, 300.0, -200.0, 300.0),
                num("blues", "Blues", 20.0, -200.0, 300.0, -200.0, 300.0),
                num("magentas", "Magentas", 80.0, -200.0, 300.0, -200.0, 300.0),
                check("tint", "Tint", false),
                color("tintColor", "Tint Color", [0.89, 0.78, 0.6]),
            ],
            render: black_white,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.vibrance",
            name: "Vibrance",
            category: CAT,
            params: vec![num("vibrance", "Vibrance", 0.0, -100.0, 100.0, -100.0, 100.0), num("saturation", "Saturation", 0.0, -100.0, 100.0, -100.0, 100.0)],
            render: |c, img| {
                let vb = c.f("vibrance") as f32 / 100.0;
                let sa = c.f("saturation") as f32 / 100.0;
                map_color(img, |v| {
                    let mx = v[0].max(v[1]).max(v[2]);
                    let mn = v[0].min(v[1]).min(v[2]);
                    let cur = mx - mn;
                    let k = 1.0 + sa + vb * (1.0 - cur);
                    let l = lum(v[0], v[1], v[2]);
                    v.map(|x| (l + (x - l) * k).clamp(0.0, 1.0))
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.threshold",
            name: "Threshold",
            category: "Stylize",
            params: vec![num("level", "Level", 128.0, 0.0, 255.0, 0.0, 255.0)],
            render: |c, img| {
                let th = c.f("level") as f32 / 255.0;
                map_color(img, |v| if lum(v[0], v[1], v[2]) >= th { [1.0; 3] } else { [0.0; 3] });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.stylize.posterize",
            name: "Posterize",
            category: "Stylize",
            params: vec![p("level", "Level", Value::Num(6.0), Ui::slider(2.0, 255.0, 2.0, 32.0, 0))],
            render: |c, img| {
                let n = c.f("level").round().clamp(2.0, 255.0) as f32 - 1.0;
                map_color(img, |v| v.map(|x| (x * n).round() / n));
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.colorBalance",
            name: "Color Balance",
            category: CAT,
            params: vec![
                num("shadowRed", "Shadow Red Balance", 0.0, -100.0, 100.0, -100.0, 100.0),
                num("shadowGreen", "Shadow Green Balance", 0.0, -100.0, 100.0, -100.0, 100.0),
                num("shadowBlue", "Shadow Blue Balance", 0.0, -100.0, 100.0, -100.0, 100.0),
                num("midRed", "Midtone Red Balance", 0.0, -100.0, 100.0, -100.0, 100.0),
                num("midGreen", "Midtone Green Balance", 0.0, -100.0, 100.0, -100.0, 100.0),
                num("midBlue", "Midtone Blue Balance", 0.0, -100.0, 100.0, -100.0, 100.0),
                num("highRed", "Highlight Red Balance", 0.0, -100.0, 100.0, -100.0, 100.0),
                num("highGreen", "Highlight Green Balance", 0.0, -100.0, 100.0, -100.0, 100.0),
                num("highBlue", "Highlight Blue Balance", 0.0, -100.0, 100.0, -100.0, 100.0),
                check("preserve", "Preserve Luminosity", false),
            ],
            render: color_balance,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.color.channelMixer",
            name: "Channel Mixer",
            category: "Channel",
            params: vec![
                num("rr", "Red-Red", 100.0, -200.0, 200.0, -200.0, 200.0),
                num("rg", "Red-Green", 0.0, -200.0, 200.0, -200.0, 200.0),
                num("rb", "Red-Blue", 0.0, -200.0, 200.0, -200.0, 200.0),
                num("gr", "Green-Red", 0.0, -200.0, 200.0, -200.0, 200.0),
                num("gg", "Green-Green", 100.0, -200.0, 200.0, -200.0, 200.0),
                num("gb", "Green-Blue", 0.0, -200.0, 200.0, -200.0, 200.0),
                num("br", "Blue-Red", 0.0, -200.0, 200.0, -200.0, 200.0),
                num("bg", "Blue-Green", 0.0, -200.0, 200.0, -200.0, 200.0),
                num("bb", "Blue-Blue", 100.0, -200.0, 200.0, -200.0, 200.0),
                check("mono", "Monochrome", false),
            ],
            render: |c, img| {
                let k = |id: &str| c.f(id) as f32 / 100.0;
                let m = [[k("rr"), k("rg"), k("rb")], [k("gr"), k("gg"), k("gb")], [k("br"), k("bg"), k("bb")]];
                let mono = c.b("mono");
                map_color(img, |v| {
                    let o = [0, 1, 2].map(|i| (m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2]).clamp(0.0, 1.0));
                    if mono { [o[0]; 3] } else { o }
                });
            },
            pad: no_pad,
        },
    ]
}

fn hue_sat(c: &FxCtx, img: &mut Img) {
    if c.b("colorize") {
        let h = (c.f("colorizeHue") / 360.0).rem_euclid(1.0) as f32;
        let s = c.f("colorizeSaturation") as f32 / 100.0;
        let l = c.f("colorizeLightness") as f32 / 100.0;
        map_color(img, |v| {
            let lum0 = lum(v[0], v[1], v[2]);
            let ll = if l >= 0.0 { lum0 + (1.0 - lum0) * l } else { lum0 * (1.0 + l) };
            hsl_to_rgb([h, s, ll])
        });
        return;
    }
    let dh = (c.f("hue") / 360.0) as f32;
    let ds = c.f("saturation") as f32 / 100.0;
    let dl = c.f("lightness") as f32 / 100.0;
    if dh == 0.0 && ds == 0.0 && dl == 0.0 {
        return;
    }
    map_color(img, |v| {
        let mut h = rgb_to_hsl(v);
        h[0] = (h[0] + dh).rem_euclid(1.0);
        h[1] = if ds >= 0.0 { h[1] + (1.0 - h[1]) * ds * h[1].min(1.0).max(0.0).sqrt() } else { h[1] * (1.0 + ds) }.clamp(0.0, 1.0);
        h[2] = if dl >= 0.0 { h[2] + (1.0 - h[2]) * dl } else { h[2] * (1.0 + dl) };
        hsl_to_rgb(h)
    });
}

fn levels(c: &FxCtx, img: &mut Img) {
    let ib = c.f("inputBlack") as f32 / 255.0;
    let iw = c.f("inputWhite") as f32 / 255.0;
    let g = 1.0 / c.f("gamma").max(0.01) as f32;
    let ob = c.f("outputBlack") as f32 / 255.0;
    let ow = c.f("outputWhite") as f32 / 255.0;
    let f = move |x: f32| {
        let t = ((x - ib) / (iw - ib).max(1e-6)).clamp(0.0, 1.0).powf(g);
        ob + (ow - ob) * t
    };
    match c.e("channel") {
        4 => img.px.par_chunks_mut(4).for_each(|p| {
            let a = p[3];
            let na = f(a).clamp(0.0, 1.0);
            let k = if a > 1e-6 { na / a } else { 0.0 };
            p[0] *= k;
            p[1] *= k;
            p[2] *= k;
            p[3] = na;
        }),
        ch => map_color(img, move |v| match ch {
            1 => [f(v[0]), v[1], v[2]],
            2 => [v[0], f(v[1]), v[2]],
            3 => [v[0], v[1], f(v[2])],
            _ => v.map(f),
        }),
    }
}

fn black_white(c: &FxCtx, img: &mut Img) {
    let w = ["reds", "yellows", "greens", "cyans", "blues", "magentas"].map(|k| c.f(k) as f32 / 100.0);
    let tint = c.b("tint").then(|| c.c("tintColor"));
    map_color(img, |v| {
        let mn = v[0].min(v[1]).min(v[2]);
        let r = v[0] - mn;
        let g = v[1] - mn;
        let b = v[2] - mn;
        let (rg, gb, br) = (r.min(g), g.min(b), b.min(r));
        let (rr, gg, bb) = (r - rg - br, g - rg - gb, b - gb - br);
        let l = mn + rr * w[0] + rg * w[1] + gg * w[2] + gb * w[3] + bb * w[4] + br * w[5];
        let l = l.clamp(0.0, 1.0);
        match tint {
            Some(t) => {
                let h = rgb_to_hsl([t[0], t[1], t[2]]);
                hsl_to_rgb([h[0], h[1], l])
            }
            None => [l; 3],
        }
    });
}

fn color_balance(c: &FxCtx, img: &mut Img) {
    let k = |id: &str| c.f(id) as f32 / 100.0;
    let sh = [k("shadowRed"), k("shadowGreen"), k("shadowBlue")];
    let md = [k("midRed"), k("midGreen"), k("midBlue")];
    let hi = [k("highRed"), k("highGreen"), k("highBlue")];
    let keep = c.b("preserve");
    map_color(img, |v| {
        let l = lum(v[0], v[1], v[2]);
        let ws = (1.0 - l * 3.0).clamp(0.0, 1.0);
        let wh = (l * 3.0 - 2.0).clamp(0.0, 1.0);
        let wm = 1.0 - ws - wh;
        let mut o = [0.0f32; 3];
        for i in 0..3 {
            o[i] = (v[i] + 0.3 * (sh[i] * ws + md[i] * wm + hi[i] * wh)).clamp(0.0, 1.0);
        }
        if keep {
            let d = l - lum(o[0], o[1], o[2]);
            o = o.map(|x| (x + d).clamp(0.0, 1.0));
        }
        o
    });
}
