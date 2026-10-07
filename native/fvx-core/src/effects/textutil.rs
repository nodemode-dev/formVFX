use rayon::prelude::*;

use super::draw::polyline;
use super::util::{lum, rand01};
use super::{EffectSpec, FxCtx, angle, check, color, mask_param, no_pad, num, pct, point, popup, slider, text};
use crate::geom::{Mat3, homography};
use crate::model::TextDoc;
use crate::render::image::Img;
use crate::render::{text as rtext, vector};

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.text.numbers",
            name: "Numbers",
            category: "Text",
            params: vec![
                text("font", "Font", "Inter"),
                popup("type", "Type", 0, &["Number", "Number [Leading Zeros]", "Timecode [30]", "Timecode [25]", "Timecode [24]", "Time", "Numerical Date", "Short Date", "Long Date", "Hexadecimal"]),
                check("random", "Random Values", false),
                slider("value", "Value/Offset/Random Max", 0.0, -30000.0, 30000.0, 2),
                slider("decimals", "Decimal Places", 2.0, 0.0, 10.0, 0),
                check("currentTime", "Current Time/Date", false),
                point("position", "Position", 0.5, 0.5),
                popup("display", "Display Options", 0, &["Fill Only", "Stroke Only", "Fill Over Stroke", "Stroke Over Fill"]),
                color("fillColor", "Fill Color", [1.0, 0.0, 0.0]),
                color("strokeColor", "Stroke Color", [1.0, 1.0, 1.0]),
                num("strokeWidth", "Stroke Width", 2.0, 0.0, 50.0, 0.0, 20.0),
                num("size", "Size", 36.0, 0.0, 1024.0, 0.0, 200.0),
                slider("tracking", "Tracking", 0.0, -20.0, 50.0, 0),
                check("composite", "Composite On Original", true),
            ],
            render: numbers,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.text.timecode",
            name: "Timecode",
            category: "Text",
            params: vec![
                popup("displayFormat", "Display Format", 0, &["SMPTE HH:MM:SS:FF", "Frames", "Feet + Frames (35mm)", "Feet + Frames (16mm)"]),
                popup("timeSource", "Time Source", 0, &["Layer Source", "Composition", "Custom"]),
                slider("frameRate", "Time Units", 30.0, 1.0, 120.0, 2),
                slider("offset", "Offset", 0.0, -100000.0, 100000.0, 0),
                text("prefix", "Text Position Prefix", ""),
                point("position", "Text Position", 0.5, 0.9),
                num("size", "Text Size", 36.0, 1.0, 512.0, 6.0, 200.0),
                color("textColor", "Text Color", [1.0, 1.0, 1.0]),
                check("showBox", "Show Box", true),
                color("boxColor", "Box Color", [0.0, 0.0, 0.0]),
                pct("opacity", "Opacity", 100.0),
                check("composite", "Composite On Original", true),
            ],
            render: timecode,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.obsolete.basicText",
            name: "Basic Text",
            category: "Obsolete",
            params: vec![text("text", "Text", "Text"), text("font", "Font", "Inter"), point("position", "Position", 0.5, 0.5), popup("display", "Display Options", 0, &["Fill Only", "Stroke Only", "Fill Over Stroke", "Stroke Over Fill"]), color("fillColor", "Fill Color", [1.0, 0.0, 0.0]), color("strokeColor", "Stroke Color", [1.0, 1.0, 1.0]), num("strokeWidth", "Stroke Width", 2.0, 0.0, 50.0, 0.0, 20.0), num("size", "Size", 36.0, 0.0, 1024.0, 0.0, 200.0), slider("tracking", "Tracking", 0.0, -20.0, 50.0, 0), check("composite", "Composite On Original", true)],
            render: basic_text,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.obsolete.pathText",
            name: "Path Text",
            category: "Obsolete",
            params: vec![text("text", "Text", "Text on a path"), text("font", "Font", "Inter"), mask_param("path", "Custom Path"), color("fillColor", "Fill Color", [1.0, 0.0, 0.0]), num("size", "Size", 36.0, 0.0, 1024.0, 0.0, 200.0), slider("tracking", "Tracking", 0.0, -20.0, 50.0, 0), pct("leftMargin", "Left Margin", 0.0), pct("visible", "Visible Characters", 100.0), check("composite", "Composite On Original", true)],
            render: path_text,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.obsolete.basic3d",
            name: "Basic 3D",
            category: "Obsolete",
            params: vec![angle("swivel", "Swivel", 0.0), angle("tilt", "Tilt", 0.0), num("distance", "Distance to Image", 0.0, -1000.0, 1000.0, -200.0, 200.0), check("specular", "Specular Highlight", false), check("preview", "Preview: Draw Preview Wireframe", false)],
            render: basic3d,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.utility.applyColorLut",
            name: "Apply Color LUT",
            category: "Utility",
            params: vec![text("path", "LUT File (.cube, .3dl, .spi3d, .clf, .cdl...)", ""), pct("intensity", "Intensity", 100.0)],
            render: apply_lut,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.utility.ccOverbrights",
            name: "CC Overbrights",
            category: "Utility",
            params: vec![popup("channel", "Channel", 0, &["RGB", "Red", "Green", "Blue", "Alpha", "Luminance"]), color("clipColor", "Clip Color", [1.0, 0.0, 1.0]), popup("show", "Show", 0, &["Overbrights", "Underblacks", "Both"])],
            render: overbrights,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.utility.cineonConverter",
            name: "Cineon Converter",
            category: "Utility",
            params: vec![popup("conversion", "Conversion Type", 1, &["Linear to Log", "Log to Linear", "Log to Log"]), slider("blackPoint", "10 Bit Black Point", 95.0, 0.0, 1023.0, 0), slider("internalBlack", "Internal Black Point", 0.0, 0.0, 1.0, 3), slider("whitePoint", "10 Bit White Point", 685.0, 0.0, 1023.0, 0), slider("internalWhite", "Internal White Point", 1.0, 0.0, 1.0, 3), slider("gamma", "Gamma", 1.7, 0.1, 5.0, 2), slider("rolloff", "Highlight Rolloff", 20.0, 0.0, 100.0, 0)],
            render: cineon,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.utility.growBounds",
            name: "Grow Bounds",
            category: "Utility",
            params: vec![num("pixels", "Number of Pixels", 0.0, 0.0, 10000.0, 0.0, 500.0)],
            render: |_, _| {},
            pad: |c| c.f("pixels"),
        },
        EffectSpec {
            id: "fx.utility.hdrCompander",
            name: "HDR Compander",
            category: "Utility",
            params: vec![popup("mode", "Mode", 0, &["Compress Range", "Expand Range"]), slider("gain", "Gain", 1.0, 0.1, 100.0, 2), slider("gamma", "Gamma", 1.0, 0.1, 10.0, 2)],
            render: |c, img| {
                let gain = c.f("gain").max(0.01) as f32;
                let g = c.f("gamma").max(0.01) as f32;
                let expand = c.e("mode") == 1;
                img.px.par_chunks_mut(4).for_each(|p| {
                    let a = p[3];
                    if a <= 1e-6 {
                        return;
                    }
                    for k in 0..3 {
                        let u = (p[k] / a).max(0.0);
                        let v = if expand { u.powf(g) * gain } else { (u / gain).powf(1.0 / g) };
                        p[k] = v * a;
                    }
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.utility.hdrHighlightCompression",
            name: "HDR Highlight Compression",
            category: "Utility",
            params: vec![pct("amount", "Amount", 100.0)],
            render: |c, img| {
                let k = c.f("amount") as f32 / 100.0;
                img.px.par_chunks_mut(4).for_each(|p| {
                    let a = p[3];
                    if a <= 1e-6 {
                        return;
                    }
                    for i in 0..3 {
                        let u = p[i] / a;
                        let comp = if u > 0.8 { 0.8 + (u - 0.8) / (1.0 + (u - 0.8) / 0.2) } else { u };
                        p[i] = (u + (comp - u) * k) * a;
                    }
                });
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.utility.colorProfileConverter",
            name: "Color Profile Converter",
            category: "Utility",
            params: vec![popup("input", "Input Profile", 0, &PROFILES), popup("output", "Output Profile", 1, &PROFILES), popup("intent", "Intent", 1, &["Perceptual", "Relative Colorimetric", "Saturation", "Absolute Colorimetric"])],
            render: profile_convert,
            pad: no_pad,
        },
    ]
}

fn layout_doc(text_s: &str, font: &str, size: f64, fill: [f32; 4], stroke: [f32; 4], sw: f64, display: u32, tracking: f64) -> TextDoc {
    TextDoc {
        text: text_s.to_string(),
        font: font.to_string(),
        size: size.max(0.1),
        fill: [fill[0] as f64, fill[1] as f64, fill[2] as f64],
        stroke: [stroke[0] as f64, stroke[1] as f64, stroke[2] as f64],
        stroke_width: sw,
        apply_fill: display != 1,
        apply_stroke: display != 0,
        stroke_over_fill: display != 2,
        tracking: tracking * 10.0,
        justify: crate::model::Justify::Center,
        ..TextDoc::default()
    }
}

fn draw_text(img: &mut Img, doc: &TextDoc, at: [f64; 2], r: f64, composite: bool) {
    let ops = rtext::text_ops(doc);
    let m = Mat3::translate(at[0], at[1]) * Mat3::scale(r, r);
    let layer = vector::rasterize(&ops, &m, img.w, img.h, 0, 0).unwrap_or_else(|| Img::new(img.w, img.h));
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

fn numbers(c: &FxCtx, img: &mut Img) {
    let ty = c.e("type");
    let dec = c.f("decimals").round().clamp(0.0, 10.0) as usize;
    let mut v = c.f("value");
    if c.b("random") {
        let r = rand01((c.t * c.fps) as i32, 0, 0, 5) as f64;
        v = r * v;
    }
    let now = c.comp_time();
    let s = match ty {
        1 => format!("{:0width$.dec$}", v, width = 8 + if dec > 0 { dec + 1 } else { 0 }, dec = dec),
        2..=4 => {
            let fps = [30.0, 25.0, 24.0][(ty - 2) as usize];
            let t = if c.b("currentTime") { now + v } else { v };
            let fr = (t * fps).round() as i64;
            let f = fr.rem_euclid(fps as i64);
            let secs = fr.div_euclid(fps as i64);
            format!("{:02}:{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60, f)
        }
        5 => {
            let t = if c.b("currentTime") { now + v } else { v };
            let secs = t.max(0.0);
            format!("{:02}:{:02}:{:0w$.d$}", (secs / 3600.0) as i64, ((secs / 60.0) as i64) % 60, secs % 60.0, w = 3 + dec, d = dec)
        }
        6 => {
            let days = v as i64;
            let (y, m, d) = civil(days);
            format!("{m}/{d}/{y}")
        }
        7 => {
            let (y, m, d) = civil(v as i64);
            format!("{m}/{d}/{:02}", y % 100)
        }
        8 => {
            let (y, m, d) = civil(v as i64);
            let months = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
            format!("{} {d}, {y}", months[(m as usize).saturating_sub(1).min(11)])
        }
        9 => format!("{:X}", v as i64),
        _ => format!("{:.dec$}", v, dec = dec),
    };
    let doc = layout_doc(&s, &c.s("font"), c.f("size"), c.c("fillColor"), c.c("strokeColor"), c.f("strokeWidth"), c.e("display"), c.f("tracking"));
    draw_text(img, &doc, c.pt("position"), c.r, c.b("composite"));
}

fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn timecode(c: &FxCtx, img: &mut Img) {
    let fps = c.f("frameRate").max(1.0);
    let t = match c.e("timeSource") {
        1 => c.comp_time(),
        2 => 0.0,
        _ => c.t,
    };
    let frames = (t * fps).round() as i64 + c.f("offset") as i64;
    let s = match c.e("displayFormat") {
        1 => format!("{frames}"),
        2 => format!("{}+{:02}", frames.div_euclid(16), frames.rem_euclid(16)),
        3 => format!("{}+{:02}", frames.div_euclid(40), frames.rem_euclid(40)),
        _ => {
            let f = frames.rem_euclid(fps.round() as i64);
            let secs = frames.div_euclid(fps.round() as i64);
            format!("{:02}:{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60, f)
        }
    };
    let s = format!("{}{}", c.s("prefix"), s);
    let col = c.c("textColor");
    let op = c.f("opacity") as f32 / 100.0;
    let size = c.f("size");
    let at = c.pt("position");
    let doc = layout_doc(&s, "Inter", size, [col[0], col[1], col[2], 1.0], [0.0; 4], 0.0, 0, 0.0);
    let comp = c.b("composite");
    let mut layer = Img::new(img.w, img.h);
    if c.b("showBox") {
        let bc = c.c("boxColor");
        let bw = size * c.r * 0.62 * s.chars().count() as f64 * 0.5 + size * c.r * 0.3;
        let bh = size * c.r * 0.7;
        let w = img.w;
        layer.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
            let py = y as f64 + 0.5;
            if py < at[1] - bh * 1.05 || py > at[1] + bh * 0.4 {
                return;
            }
            for x in 0..w {
                let px = x as f64 + 0.5;
                if (px - at[0]).abs() <= bw {
                    row[x * 4..x * 4 + 4].copy_from_slice(&[bc[0], bc[1], bc[2], 1.0]);
                }
            }
        });
    }
    draw_text(&mut layer, &doc, at, c.r, true);
    layer.px.iter_mut().for_each(|v| *v *= op);
    if comp {
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

fn basic_text(c: &FxCtx, img: &mut Img) {
    let doc = layout_doc(&c.s("text"), &c.s("font"), c.f("size"), c.c("fillColor"), c.c("strokeColor"), c.f("strokeWidth"), c.e("display"), c.f("tracking"));
    draw_text(img, &doc, c.pt("position"), c.r, c.b("composite"));
}

fn path_text(c: &FxCtx, img: &mut Img) {
    let Some(path) = c.mask(c.f("path") as usize) else { return };
    let pts = polyline(&path, 0.5);
    if pts.len() < 2 {
        return;
    }
    let mut len = vec![0.0];
    for i in 1..pts.len() {
        len.push(len[i - 1] + (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1]));
    }
    let total = len[len.len() - 1];
    let at = |s: f64| -> ([f64; 2], f64) {
        let i = len.partition_point(|x| *x < s).clamp(1, len.len() - 1);
        let seg = (len[i] - len[i - 1]).max(1e-9);
        let t = ((s - len[i - 1]) / seg).clamp(0.0, 1.0);
        let p = [pts[i - 1][0] + (pts[i][0] - pts[i - 1][0]) * t, pts[i - 1][1] + (pts[i][1] - pts[i - 1][1]) * t];
        (p, (pts[i][1] - pts[i - 1][1]).atan2(pts[i][0] - pts[i - 1][0]))
    };
    let text_s = c.s("text");
    let size = c.f("size") * c.r;
    let col = c.c("fillColor");
    let visible = (c.f("visible") / 100.0 * text_s.chars().count() as f64).round() as usize;
    let mut s = total * c.f("leftMargin") / 100.0;
    let adv = size * 0.6 + c.f("tracking") * c.r;
    let mut ops = vec![];
    for ch in text_s.chars().take(visible) {
        if s > total {
            break;
        }
        let (p, a) = at(s);
        let doc = TextDoc { text: ch.to_string(), font: c.s("font"), size: size.max(0.1), fill: [col[0] as f64, col[1] as f64, col[2] as f64], ..TextDoc::default() };
        let m = Mat3::translate(p[0], p[1]) * Mat3::rotate_deg(a.to_degrees());
        for op in rtext::text_ops(&doc) {
            ops.push(vector::DrawOp { transform: m * op.transform, ..op });
        }
        s += adv;
    }
    let layer = vector::rasterize(&ops, &Mat3::IDENTITY, img.w, img.h, 0, 0).unwrap_or_else(|| Img::new(img.w, img.h));
    if c.b("composite") {
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

fn basic3d(c: &FxCtx, img: &mut Img) {
    let (sw, tl) = (c.f("swivel").to_radians(), c.f("tilt").to_radians());
    let dist = c.px(c.f("distance"));
    if sw.abs() < 1e-6 && tl.abs() < 1e-6 && dist.abs() < 1e-6 {
        return;
    }
    let r = c.layer_rect();
    let (cx, cy) = ((r[0] + r[2]) * 0.5, (r[1] + r[3]) * 0.5);
    let f = (r[2] - r[0]).max(r[3] - r[1]) * 1.5;
    let proj = |x: f64, y: f64| -> [f64; 2] {
        let (dx, dy) = (x - cx, y - cy);
        let (x1, z1) = (dx * sw.cos(), dx * sw.sin());
        let (y2, z2) = (dy * tl.cos() - z1 * tl.sin(), dy * tl.sin() + z1 * tl.cos());
        let z = z2 + dist;
        let k = f / (f + z).max(1.0);
        [cx + x1 * k, cy + y2 * k]
    };
    let src = [[r[0], r[1]], [r[2], r[1]], [r[0], r[3]], [r[2], r[3]]];
    let dst = [proj(r[0], r[1]), proj(r[2], r[1]), proj(r[0], r[3]), proj(r[2], r[3])];
    let Some(hm) = homography(dst, src) else { return };
    let spec = c.b("specular");
    let shade = (sw.cos() * tl.cos()).abs() as f32;
    super::util::remap(img, false, |x, y| {
        let wv = hm.0[2][0] * x + hm.0[2][1] * y + hm.0[2][2];
        if wv <= 0.0 {
            return None;
        }
        Some(hm.apply(x, y))
    });
    if spec {
        img.px.par_chunks_mut(4).for_each(|p| {
            let s = shade.powi(20) * 0.6 * p[3];
            for k in 0..3 {
                p[k] += s;
            }
        });
    }
    if c.b("preview") {
        let w = img.w;
        for k in 0..4 {
            let (a, b) = [(dst[0], dst[1]), (dst[1], dst[3]), (dst[3], dst[2]), (dst[2], dst[0])][k];
            let n = (a[0] - b[0]).hypot(a[1] - b[1]).ceil() as usize + 1;
            for j in 0..=n {
                let t = j as f64 / n as f64;
                let (x, y) = ((a[0] + (b[0] - a[0]) * t) as isize, (a[1] + (b[1] - a[1]) * t) as isize);
                if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < img.h {
                    let i = (y as usize * w + x as usize) * 4;
                    img.px[i..i + 4].copy_from_slice(&[1.0, 1.0, 1.0, 1.0]);
                }
            }
        }
    }
}

fn apply_lut(c: &FxCtx, img: &mut Img) {
    super::ocio::lut_file(img, &c.s("path"), c.f("intensity") as f32 / 100.0);
}

fn overbrights(c: &FxCtx, img: &mut Img) {
    let ch = c.e("channel");
    let col = c.c("clipColor");
    let show = c.e("show");
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        let u = if a > 1e-6 { [p[0] / a, p[1] / a, p[2] / a] } else { [0.0; 3] };
        let v = match ch {
            1 => u[0],
            2 => u[1],
            3 => u[2],
            4 => a,
            5 => lum(u[0], u[1], u[2]),
            _ => u[0].max(u[1]).max(u[2]),
        };
        let vmin = if ch == 0 { u[0].min(u[1]).min(u[2]) } else { v };
        let over = v > 1.0 && show != 1;
        let under = vmin < 0.0 && show != 0;
        if over || under {
            p[0] = col[0] * a.max(0.001);
            p[1] = col[1] * a.max(0.001);
            p[2] = col[2] * a.max(0.001);
        } else {
            for k in 0..3 {
                p[k] = p[k].clamp(0.0, a);
            }
        }
    });
}

fn cineon(c: &FxCtx, img: &mut Img) {
    let conv = c.e("conversion");
    let bp = c.f("blackPoint") as f32;
    let wp = c.f("whitePoint") as f32;
    let ib = c.f("internalBlack") as f32;
    let iw = c.f("internalWhite") as f32;
    let g = c.f("gamma") as f32;
    let roll = c.f("rolloff") as f32 / 100.0;
    let gain = |code: f32| 10f32.powf((code - wp) * 0.002 / 0.6 * g / 1.7);
    let off = gain(bp);
    let to_lin = |v: f32| -> f32 {
        let code = v * 1023.0;
        let l = (gain(code) - off) / (1.0 - off);
        let l = ib + l * (iw - ib);
        if l > 1.0 - roll && roll > 0.0 { (1.0 - roll) + (l - (1.0 - roll)) / (1.0 + (l - (1.0 - roll)) / roll) } else { l }
    };
    let to_log = |l: f32| -> f32 {
        let l = ((l - ib) / (iw - ib).max(1e-5)).max(0.0);
        let x = l * (1.0 - off) + off;
        let code = wp + x.max(1e-6).log10() * 0.6 / 0.002 * 1.7 / g;
        code / 1023.0
    };
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        for k in 0..3 {
            let u = p[k] / a;
            let v = match conv {
                0 => to_log(u),
                2 => to_log(to_lin(u)),
                _ => to_lin(u),
            };
            p[k] = v * a;
        }
    });
}

const PROFILES: [&str; 6] = ["sRGB", "Linear sRGB", "Rec. 709", "Display P3", "Rec. 2020", "Adobe RGB (1998)"];

fn to_linear(p: u32, v: f32) -> f32 {
    match p {
        1 => v,
        2 | 4 => {
            if v < 0.081 { v / 4.5 } else { ((v + 0.099) / 1.099).powf(1.0 / 0.45) }
        }
        5 => v.max(0.0).powf(2.199),
        _ => {
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        }
    }
}

fn from_linear(p: u32, v: f32) -> f32 {
    match p {
        1 => v,
        2 | 4 => {
            if v < 0.018 { v * 4.5 } else { 1.099 * v.powf(0.45) - 0.099 }
        }
        5 => v.max(0.0).powf(1.0 / 2.199),
        _ => {
            if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
        }
    }
}

fn to_xyz(p: u32) -> [[f32; 3]; 3] {
    match p {
        3 => [[0.4866, 0.2657, 0.1982], [0.2290, 0.6917, 0.0793], [0.0, 0.0451, 1.0439]],
        4 => [[0.6370, 0.1446, 0.1689], [0.2627, 0.6780, 0.0593], [0.0, 0.0281, 1.0610]],
        5 => [[0.5767, 0.1856, 0.1882], [0.2974, 0.6273, 0.0753], [0.0270, 0.0707, 0.9911]],
        _ => [[0.4124, 0.3576, 0.1805], [0.2126, 0.7152, 0.0722], [0.0193, 0.1192, 0.9505]],
    }
}

fn inv3(m: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let d = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let id = 1.0 / d;
    [
        [(m[1][1] * m[2][2] - m[1][2] * m[2][1]) * id, (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * id, (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * id],
        [(m[1][2] * m[2][0] - m[1][0] * m[2][2]) * id, (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * id, (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * id],
        [(m[1][0] * m[2][1] - m[1][1] * m[2][0]) * id, (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * id, (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * id],
    ]
}

fn mul3(a: [[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2], a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2], a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2]]
}

fn profile_convert(c: &FxCtx, img: &mut Img) {
    let (i, o) = (c.e("input"), c.e("output"));
    if i == o {
        return;
    }
    let mi = to_xyz(i);
    let mo = inv3(to_xyz(o));
    let clip = c.e("intent") != 3;
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let lin = [to_linear(i, p[0] / a), to_linear(i, p[1] / a), to_linear(i, p[2] / a)];
        let x = mul3(mi, lin);
        let l2 = mul3(mo, x);
        for k in 0..3 {
            let v = from_linear(o, if clip { l2[k].max(0.0) } else { l2[k] });
            p[k] = v * a;
        }
    });
}
