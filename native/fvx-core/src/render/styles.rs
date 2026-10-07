use rayon::prelude::*;

use super::image::Img;
use crate::effects::util::blur_scalar;
use crate::eval::Eval;
use crate::model::prop::{g, p};
use crate::model::{Gradient, Group, GroupKind, Ids, Layer, Ui, Value};

pub const STYLES: [(&str, &str); 9] = [
    ("dropShadow", "Drop Shadow"),
    ("innerShadow", "Inner Shadow"),
    ("outerGlow", "Outer Glow"),
    ("innerGlow", "Inner Glow"),
    ("bevelEmboss", "Bevel and Emboss"),
    ("satin", "Satin"),
    ("colorOverlay", "Color Overlay"),
    ("gradientOverlay", "Gradient Overlay"),
    ("stroke", "Stroke"),
];

pub const MODES: [&str; 6] = ["Normal", "Multiply", "Screen", "Overlay", "Linear Dodge", "Color Burn"];

pub fn style_kind(name: &str) -> Option<&'static str> {
    let flat = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase();
    let w = flat(name);
    STYLES.iter().find(|(id, label)| flat(id) == w || flat(label) == w).map(|s| s.0).or(match w.as_str() {
        "bevel" | "emboss" | "bevelandemboss" => Some("bevelEmboss"),
        _ => None,
    })
}

fn pct(ids: &mut Ids, id: &str, name: &str, v: f64) -> crate::model::Node {
    p(ids.prop(id, name, Value::Num(v), Ui::slider(0.0, 100.0, 0.0, 100.0, 0)))
}

fn px(ids: &mut Ids, id: &str, name: &str, v: f64, max: f64) -> crate::model::Node {
    p(ids.prop(id, name, Value::Num(v), Ui::slider(0.0, 1000.0, 0.0, max, 1)))
}

fn mode(ids: &mut Ids, v: u32) -> crate::model::Node {
    p(ids.prop("blendMode", "Blend Mode", Value::Enum(v), Ui::popup(&MODES)))
}

fn color(ids: &mut Ids, id: &str, name: &str, c: [f64; 3]) -> crate::model::Node {
    p(ids.prop(id, name, Value::Color([c[0], c[1], c[2], 1.0]), Ui::Color))
}

fn check(ids: &mut Ids, id: &str, name: &str, v: bool) -> crate::model::Node {
    p(ids.prop(id, name, Value::Bool(v), Ui::Checkbox))
}

pub fn style_group(ids: &mut Ids, kind: &str) -> Option<Group> {
    let name = STYLES.iter().find(|s| s.0 == kind)?.1;
    let c = match kind {
        "dropShadow" => vec![
            mode(ids, 1),
            color(ids, "color", "Color", [0.0; 3]),
            pct(ids, "opacity", "Opacity", 75.0),
            check(ids, "useGlobalLight", "Use Global Light", true),
            p(ids.prop("angle", "Angle", Value::Num(120.0), Ui::Angle)),
            px(ids, "distance", "Distance", 5.0, 100.0),
            pct(ids, "spread", "Spread", 0.0),
            px(ids, "size", "Size", 5.0, 100.0),
            pct(ids, "noise", "Noise", 0.0),
            check(ids, "knocksOut", "Layer Knocks Out Drop Shadow", true),
        ],
        "innerShadow" => vec![
            mode(ids, 1),
            color(ids, "color", "Color", [0.0; 3]),
            pct(ids, "opacity", "Opacity", 75.0),
            check(ids, "useGlobalLight", "Use Global Light", true),
            p(ids.prop("angle", "Angle", Value::Num(120.0), Ui::Angle)),
            px(ids, "distance", "Distance", 5.0, 100.0),
            pct(ids, "choke", "Choke", 0.0),
            px(ids, "size", "Size", 5.0, 100.0),
            pct(ids, "noise", "Noise", 0.0),
        ],
        "outerGlow" => vec![
            mode(ids, 2),
            pct(ids, "opacity", "Opacity", 75.0),
            pct(ids, "noise", "Noise", 0.0),
            color(ids, "color", "Color", [1.0, 1.0, 0.745]),
            p(ids.prop("technique", "Technique", Value::Enum(0), Ui::popup(&["Softer", "Precise"]))),
            pct(ids, "spread", "Spread", 0.0),
            px(ids, "size", "Size", 5.0, 100.0),
            pct(ids, "range", "Range", 50.0),
        ],
        "innerGlow" => vec![
            mode(ids, 2),
            pct(ids, "opacity", "Opacity", 75.0),
            pct(ids, "noise", "Noise", 0.0),
            color(ids, "color", "Color", [1.0, 1.0, 0.745]),
            p(ids.prop("technique", "Technique", Value::Enum(0), Ui::popup(&["Softer", "Precise"]))),
            p(ids.prop("source", "Source", Value::Enum(1), Ui::popup(&["Center", "Edge"]))),
            pct(ids, "choke", "Choke", 0.0),
            px(ids, "size", "Size", 5.0, 100.0),
            pct(ids, "range", "Range", 50.0),
        ],
        "bevelEmboss" => vec![
            p(ids.prop("style", "Style", Value::Enum(1), Ui::popup(&["Outer Bevel", "Inner Bevel", "Emboss", "Pillow Emboss", "Stroke Emboss"]))),
            p(ids.prop("technique", "Technique", Value::Enum(0), Ui::popup(&["Smooth", "Chisel Hard", "Chisel Soft"]))),
            p(ids.prop("depth", "Depth", Value::Num(100.0), Ui::slider(1.0, 1000.0, 1.0, 500.0, 0))),
            p(ids.prop("direction", "Direction", Value::Enum(0), Ui::popup(&["Up", "Down"]))),
            px(ids, "size", "Size", 5.0, 100.0),
            px(ids, "soften", "Soften", 0.0, 16.0),
            check(ids, "useGlobalLight", "Use Global Light", true),
            p(ids.prop("angle", "Angle", Value::Num(120.0), Ui::Angle)),
            p(ids.prop("altitude", "Altitude", Value::Num(30.0), Ui::slider(0.0, 90.0, 0.0, 90.0, 0))),
            p(ids.prop("highlightMode", "Highlight Mode", Value::Enum(2), Ui::popup(&MODES))),
            color(ids, "highlightColor", "Highlight Color", [1.0; 3]),
            pct(ids, "highlightOpacity", "Highlight Opacity", 75.0),
            p(ids.prop("shadowMode", "Shadow Mode", Value::Enum(1), Ui::popup(&MODES))),
            color(ids, "shadowColor", "Shadow Color", [0.0; 3]),
            pct(ids, "shadowOpacity", "Shadow Opacity", 75.0),
        ],
        "satin" => vec![
            mode(ids, 1),
            color(ids, "color", "Color", [0.0; 3]),
            pct(ids, "opacity", "Opacity", 50.0),
            p(ids.prop("angle", "Angle", Value::Num(19.0), Ui::Angle)),
            px(ids, "distance", "Distance", 11.0, 100.0),
            px(ids, "size", "Size", 14.0, 100.0),
            check(ids, "invert", "Invert", true),
        ],
        "colorOverlay" => vec![mode(ids, 0), color(ids, "color", "Color", [1.0, 0.0, 0.0]), pct(ids, "opacity", "Opacity", 100.0)],
        "gradientOverlay" => vec![
            mode(ids, 0),
            pct(ids, "opacity", "Opacity", 100.0),
            p(ids.prop("colors", "Colors", Value::Gradient(Gradient::default()), Ui::Gradient)),
            p(ids.prop("angle", "Angle", Value::Num(90.0), Ui::Angle)),
            p(ids.prop("style", "Style", Value::Enum(0), Ui::popup(&["Linear", "Radial", "Angle", "Reflected", "Diamond"]))),
            check(ids, "reverse", "Reverse", false),
            p(ids.prop("scale", "Scale", Value::Num(100.0), Ui::slider(10.0, 150.0, 10.0, 150.0, 0))),
            p(ids.prop("offset", "Offset", Value::Vec2([0.0, 0.0]), Ui::Point)),
        ],
        "stroke" => vec![
            color(ids, "color", "Color", [1.0, 0.0, 0.0]),
            px(ids, "size", "Size", 3.0, 50.0),
            pct(ids, "opacity", "Opacity", 100.0),
            p(ids.prop("position", "Position", Value::Enum(0), Ui::popup(&["Outside", "Inside", "Center"]))),
            mode(ids, 0),
        ],
        _ => return None,
    };
    Some(ids.group(kind, name, GroupKind::Plain, c))
}

pub fn styles_group(ids: &mut Ids) -> Group {
    let blend = vec![pct(ids, "fillOpacity", "Fill Opacity", 100.0), check(ids, "knockout", "Blend Interior Styles as Group", false)];
    let bo = g(ids.group("blendingOptions", "Blending Options", GroupKind::Plain, blend));
    ids.group("layerStyles", "Layer Styles", GroupKind::Plain, vec![bo])
}

pub fn active(l: &Layer) -> bool {
    l.props.sub("layerStyles").is_some_and(|s| s.enabled && s.groups().any(|g| g.enabled && g.id != "blendingOptions"))
}

struct P<'a> {
    ev: &'a Eval<'a>,
    l: &'a Layer,
    g: &'a Group,
}

impl P<'_> {
    fn f(&self, id: &str, d: f64) -> f64 {
        self.g.get(id).map(|x| self.ev.value(self.l, x).num()).unwrap_or(d)
    }
    fn e(&self, id: &str) -> u32 {
        self.g.get(id).map(|x| self.ev.value(self.l, x).index()).unwrap_or(0)
    }
    fn b(&self, id: &str, d: bool) -> bool {
        self.g.get(id).map(|x| self.ev.value(self.l, x).boolean()).unwrap_or(d)
    }
    fn c(&self, id: &str) -> [f32; 3] {
        let c = self.g.get(id).map(|x| self.ev.value(self.l, x).color()).unwrap_or([0.0, 0.0, 0.0, 1.0]);
        [c[0] as f32, c[1] as f32, c[2] as f32]
    }
    fn light(&self, comp_light: [f64; 2]) -> (f64, f64) {
        if self.b("useGlobalLight", true) { (comp_light[0], comp_light[1]) } else { (self.f("angle", 120.0), self.f("altitude", 30.0)) }
    }
}

pub fn pad(ev: &Eval, l: &Layer) -> f64 {
    let Some(s) = l.props.sub("layerStyles") else { return 0.0 };
    let mut m: f64 = 0.0;
    for gr in s.groups().filter(|x| x.enabled) {
        let pp = P { ev, l, g: gr };
        let v = match gr.id.as_str() {
            "dropShadow" => pp.f("distance", 5.0) + pp.f("size", 5.0) * 2.0 + 2.0,
            "outerGlow" => pp.f("size", 5.0) * 2.0 + 2.0,
            "stroke" => pp.f("size", 3.0) + 2.0,
            "bevelEmboss" if pp.e("style") == 0 => pp.f("size", 5.0) + 2.0,
            _ => 0.0,
        };
        m = m.max(v);
    }
    m
}

fn alpha(img: &Img) -> Vec<f32> {
    img.px.chunks_exact(4).map(|p| p[3]).collect()
}

fn shifted(a: &[f32], w: usize, h: usize, dx: f64, dy: f64) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let sx = x as f64 - dx;
            let sy = y as f64 - dy;
            let x0 = sx.floor();
            let y0 = sy.floor();
            let fx = (sx - x0) as f32;
            let fy = (sy - y0) as f32;
            let get = |xx: f64, yy: f64| -> f32 {
                if xx < 0.0 || yy < 0.0 || xx >= w as f64 || yy >= h as f64 { 0.0 } else { a[yy as usize * w + xx as usize] }
            };
            let v = get(x0, y0) * (1.0 - fx) * (1.0 - fy) + get(x0 + 1.0, y0) * fx * (1.0 - fy) + get(x0, y0 + 1.0) * (1.0 - fx) * fy + get(x0 + 1.0, y0 + 1.0) * fx * fy;
            *o = v;
        }
    });
    out
}

fn edt_1d(f: &[f64], out: &mut [f64]) {
    let n = f.len();
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f64; n + 1];
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        loop {
            let r = v[k];
            let s = ((f[q] + (q * q) as f64) - (f[r] + (r * r) as f64)) / (2.0 * q as f64 - 2.0 * r as f64);
            if s <= z[k] {
                if k == 0 {
                    v[0] = q;
                    z[0] = f64::NEG_INFINITY;
                    z[1] = f64::INFINITY;
                    break;
                }
                k -= 1;
                continue;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f64::INFINITY;
            break;
        }
    }
    k = 0;
    for (q, o) in out.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let d = q as f64 - v[k] as f64;
        *o = d * d + f[v[k]];
    }
}

pub fn distance(a: &[f32], w: usize, h: usize, inside: bool) -> Vec<f32> {
    let big = 1e12;
    let mut grid: Vec<f64> = a.iter().map(|v| if (*v >= 0.5) == inside { 0.0 } else { big }).collect();
    let mut tmp = vec![0.0; w.max(h)];
    let mut col = vec![0.0; h];
    for x in 0..w {
        for y in 0..h {
            col[y] = grid[y * w + x];
        }
        edt_1d(&col, &mut tmp[..h]);
        for y in 0..h {
            grid[y * w + x] = tmp[y];
        }
    }
    grid.par_chunks_mut(w).for_each(|row| {
        let mut t = vec![0.0; w];
        edt_1d(row, &mut t);
        row.copy_from_slice(&t);
    });
    grid.iter().map(|d| (d.sqrt()) as f32).collect()
}

fn blend(dst: [f32; 3], src: [f32; 3], mode: u32) -> [f32; 3] {
    let f = |a: f32, b: f32| -> f32 {
        match mode {
            1 => a * b,
            2 => a + b - a * b,
            3 => {
                if a < 0.5 {
                    2.0 * a * b
                } else {
                    1.0 - 2.0 * (1.0 - a) * (1.0 - b)
                }
            }
            4 => (a + b).min(1.0),
            5 => {
                if b <= 0.0 {
                    0.0
                } else {
                    1.0 - ((1.0 - a) / b).min(1.0)
                }
            }
            _ => b,
        }
    };
    [f(dst[0], src[0]), f(dst[1], src[1]), f(dst[2], src[2])]
}

fn paint_inside(img: &mut Img, cov: &[f32], color: [f32; 3], opacity: f32, mode: u32) {
    img.px.par_chunks_mut(4).zip(cov.par_iter()).for_each(|(px, c)| {
        let a = px[3];
        if a <= 1e-6 || *c <= 0.0 {
            return;
        }
        let un = [px[0] / a, px[1] / a, px[2] / a];
        let b = blend(un, color, mode);
        let k = (c * opacity).clamp(0.0, 1.0);
        for i in 0..3 {
            px[i] = (un[i] + (b[i] - un[i]) * k) * a;
        }
    });
}

fn under(img: &mut Img, cov: &[f32], color: [f32; 3], opacity: f32) {
    img.px.par_chunks_mut(4).zip(cov.par_iter()).for_each(|(px, c)| {
        let sa = (c * opacity).clamp(0.0, 1.0);
        if sa <= 0.0 {
            return;
        }
        let k = 1.0 - px[3];
        for i in 0..3 {
            px[i] += color[i] * sa * k;
        }
        px[3] += sa * k;
    });
}

fn over(img: &mut Img, cov: &[f32], color: [f32; 3], opacity: f32) {
    img.px.par_chunks_mut(4).zip(cov.par_iter()).for_each(|(px, c)| {
        let sa = (c * opacity).clamp(0.0, 1.0);
        if sa <= 0.0 {
            return;
        }
        let k = 1.0 - sa;
        for i in 0..3 {
            px[i] = color[i] * sa + px[i] * k;
        }
        px[3] = sa + px[3] * k;
    });
}

fn blurred(a: &[f32], w: usize, h: usize, sigma: f64) -> Vec<f32> {
    let mut v = a.to_vec();
    if sigma > 0.3 {
        blur_scalar(&mut v, w, h, sigma, sigma);
    }
    v
}

fn spread_alpha(a: &[f32], w: usize, h: usize, size: f64, spread: f64) -> Vec<f32> {
    let grow = size * spread / 100.0;
    let base = if grow > 0.5 {
        let d = distance(a, w, h, true);
        d.iter().zip(a).map(|(dd, aa)| aa.max((grow as f32 - dd + 0.5).clamp(0.0, 1.0))).collect()
    } else {
        a.to_vec()
    };
    blurred(&base, w, h, (size - grow).max(0.0) / 2.0)
}

pub fn apply(ev: &Eval, l: &Layer, img: &mut Img, r: f64) {
    let Some(s) = l.props.sub("layerStyles") else { return };
    if !s.enabled {
        return;
    }
    let (w, h) = (img.w, img.h);
    let a0 = alpha(img);
    let light = ev.comp.global_light;
    let fill = s.sub("blendingOptions").and_then(|b| b.get("fillOpacity")).map(|x| ev.value(l, x).num()).unwrap_or(100.0) as f32 / 100.0;
    if (fill - 1.0).abs() > 1e-4 {
        img.scale_all(fill.clamp(0.0, 1.0));
    }
    let get = |id: &str| s.groups().find(|x| x.id == id && x.enabled);
    for id in ["gradientOverlay", "colorOverlay", "satin", "innerGlow", "innerShadow"] {
        let Some(gr) = get(id) else { continue };
        let pp = P { ev, l, g: gr };
        let op = pp.f("opacity", 75.0) as f32 / 100.0;
        let m = pp.e("blendMode");
        match id {
            "colorOverlay" => paint_inside(img, &a0, pp.c("color"), op, m),
            "gradientOverlay" => {
                let grad = gr.get("colors").map(|x| ev.value(l, x)).and_then(|v| v.as_gradient().cloned()).unwrap_or_default();
                let ang = pp.f("angle", 90.0).to_radians();
                let style = pp.e("style");
                let rev = pp.b("reverse", false);
                let sc = (pp.f("scale", 100.0) / 100.0).max(0.01);
                let (mut x0, mut y0, mut x1, mut y1) = (w as f64, h as f64, 0.0f64, 0.0f64);
                for (i, a) in a0.iter().enumerate() {
                    if *a > 0.01 {
                        let (x, y) = ((i % w) as f64, (i / w) as f64);
                        x0 = x0.min(x);
                        y0 = y0.min(y);
                        x1 = x1.max(x);
                        y1 = y1.max(y);
                    }
                }
                if x1 < x0 {
                    continue;
                }
                let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                let (dx, dy) = (ang.cos(), -ang.sin());
                let half = ((x1 - x0) * dx.abs() + (y1 - y0) * dy.abs()) / 2.0 * sc;
                let rad = ((x1 - x0).hypot(y1 - y0) / 2.0 * sc).max(1.0);
                let a0r = &a0;
                img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
                    for x in 0..w {
                        let a = a0r[y * w + x];
                        if a <= 1e-6 {
                            continue;
                        }
                        let (px_, py_) = (x as f64 - cx, y as f64 - cy);
                        let along = (px_ * dx + py_ * dy) / half.max(1.0);
                        let mut t = match style {
                            1 => px_.hypot(py_) / rad,
                            2 => ((py_.atan2(px_) - (-ang)).rem_euclid(std::f64::consts::TAU)) / std::f64::consts::TAU,
                            3 => along.abs(),
                            4 => (px_.abs() + py_.abs()) / rad,
                            _ => along * 0.5 + 0.5,
                        };
                        if rev {
                            t = 1.0 - t;
                        }
                        let c = grad.sample(t.clamp(0.0, 1.0));
                        let o = &mut row[x * 4..x * 4 + 4];
                        let un = [o[0] / o[3].max(1e-6), o[1] / o[3].max(1e-6), o[2] / o[3].max(1e-6)];
                        let b = blend(un, [c[0] as f32, c[1] as f32, c[2] as f32], m);
                        let k = op * c[3] as f32;
                        for i in 0..3 {
                            o[i] = (un[i] + (b[i] - un[i]) * k) * o[3];
                        }
                    }
                });
            }
            "satin" => {
                let ang = pp.f("angle", 19.0).to_radians();
                let d = pp.f("distance", 11.0) * r;
                let size = pp.f("size", 14.0) * r;
                let b = blurred(&a0, w, h, size / 2.0);
                let s1 = shifted(&b, w, h, ang.cos() * d, -ang.sin() * d);
                let s2 = shifted(&b, w, h, -ang.cos() * d, ang.sin() * d);
                let inv = pp.b("invert", true);
                let cov: Vec<f32> = s1.iter().zip(&s2).zip(&a0).map(|((x, y), a)| {
                    let v = (x - y).abs();
                    (if inv { 1.0 - v } else { v }) * a
                }).collect();
                paint_inside(img, &cov, pp.c("color"), op, m);
            }
            "innerGlow" => {
                let size = pp.f("size", 5.0) * r;
                let choke = pp.f("choke", 0.0) / 100.0;
                let b = blurred(&a0, w, h, (size * (1.0 - choke)).max(0.0) / 2.0);
                let edge = pp.e("source") == 1;
                let cov: Vec<f32> = b.iter().zip(&a0).map(|(x, a)| (if edge { 1.0 - x } else { *x }) * a).collect();
                paint_inside(img, &cov, pp.c("color"), op, m);
            }
            "innerShadow" => {
                let (ang, _) = pp.light(light);
                let ang = ang.to_radians();
                let d = pp.f("distance", 5.0) * r;
                let size = pp.f("size", 5.0) * r;
                let inv: Vec<f32> = a0.iter().map(|a| 1.0 - a).collect();
                let sh = shifted(&blurred(&inv, w, h, size / 2.0), w, h, -ang.cos() * d, ang.sin() * d);
                let cov: Vec<f32> = sh.iter().zip(&a0).map(|(x, a)| x * a).collect();
                paint_inside(img, &cov, pp.c("color"), op, m);
            }
            _ => {}
        }
    }
    if let Some(gr) = get("bevelEmboss") {
        let pp = P { ev, l, g: gr };
        let size = (pp.f("size", 5.0) * r).max(0.5);
        let soften = pp.f("soften", 0.0) * r;
        let style = pp.e("style");
        let (ang, alt) = pp.light(light);
        let alt = if pp.b("useGlobalLight", true) { alt } else { pp.f("altitude", 30.0) };
        let depth = pp.f("depth", 100.0) / 100.0 * if pp.e("direction") == 1 { -1.0 } else { 1.0 };
        let height: Vec<f32> = match style {
            0 => {
                let d = distance(&a0, w, h, true);
                d.iter().zip(&a0).map(|(dd, a)| if *a >= 0.5 { 1.0 } else { (1.0 - dd / size as f32).max(0.0) }).collect()
            }
            _ => {
                let d = distance(&a0, w, h, false);
                d.iter().zip(&a0).map(|(dd, a)| if *a < 0.5 { 0.0 } else { (dd / size as f32).min(1.0) }).collect()
            }
        };
        let height = blurred(&height, w, h, soften / 2.0 + 0.6);
        let (la, lz) = (ang.to_radians(), alt.to_radians());
        let lv = [la.cos() * lz.cos(), -la.sin() * lz.cos(), lz.sin()];
        let flat = lz.sin() as f32;
        let k = (size as f32 * depth as f32).max(0.01);
        let mut hi = vec![0.0f32; w * h];
        let mut lo = vec![0.0f32; w * h];
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let i = y * w + x;
                let gx = (height[i + 1] - height[i - 1]) * 0.5 * k;
                let gy = (height[i + w] - height[i - w]) * 0.5 * k;
                let n = [-gx as f64, -gy as f64, 1.0];
                let nl = (n[0] * n[0] + n[1] * n[1] + 1.0).sqrt();
                let shade = ((n[0] * lv[0] + n[1] * lv[1] + n[2] * lv[2]) / nl) as f32;
                let region = if style == 0 { (1.0 - a0[i]).max(0.0) * (height[i] > 0.0) as u8 as f32 } else { a0[i] };
                hi[i] = (shade - flat).max(0.0) / (1.0 - flat).max(0.05) * region;
                lo[i] = (flat - shade).max(0.0) / flat.max(0.05) * region;
            }
        }
        let (hm, sm) = (pp.e("highlightMode"), pp.e("shadowMode"));
        let (ho, so) = (pp.f("highlightOpacity", 75.0) as f32 / 100.0, pp.f("shadowOpacity", 75.0) as f32 / 100.0);
        if style == 0 {
            under(img, &lo, pp.c("shadowColor"), so);
            under(img, &hi, pp.c("highlightColor"), ho);
        } else {
            paint_inside(img, &hi, pp.c("highlightColor"), ho, hm);
            paint_inside(img, &lo, pp.c("shadowColor"), so, sm);
        }
    }
    if let Some(gr) = get("stroke") {
        let pp = P { ev, l, g: gr };
        let size = pp.f("size", 3.0) * r;
        let op = pp.f("opacity", 100.0) as f32 / 100.0;
        let pos = pp.e("position");
        let (outer, inner) = match pos {
            1 => (0.0, size),
            2 => (size / 2.0, size / 2.0),
            _ => (size, 0.0),
        };
        let dout = if outer > 0.0 { Some(distance(&a0, w, h, true)) } else { None };
        let din = if inner > 0.0 { Some(distance(&a0, w, h, false)) } else { None };
        let cov: Vec<f32> = (0..w * h)
            .map(|i| {
                let a = a0[i];
                let o = dout.as_ref().map(|d| (outer as f32 - d[i] + 0.5).clamp(0.0, 1.0) * (1.0 - a)).unwrap_or(0.0);
                let n = din.as_ref().map(|d| (inner as f32 - d[i] + 0.5).clamp(0.0, 1.0) * a).unwrap_or(0.0);
                o + n
            })
            .collect();
        over(img, &cov, pp.c("color"), op);
    }
    for id in ["outerGlow", "dropShadow"] {
        let Some(gr) = get(id) else { continue };
        let pp = P { ev, l, g: gr };
        let op = pp.f("opacity", 75.0) as f32 / 100.0;
        let size = pp.f("size", 5.0) * r;
        let spread = pp.f("spread", 0.0);
        let mut cov = spread_alpha(&a0, w, h, size, spread);
        if id == "dropShadow" {
            let (ang, _) = pp.light(light);
            let ang = ang.to_radians();
            let d = pp.f("distance", 5.0) * r;
            cov = shifted(&cov, w, h, -ang.cos() * d, ang.sin() * d);
            if pp.b("knocksOut", true) {
                cov.iter_mut().zip(&a0).for_each(|(c, a)| *c *= 1.0 - a);
            }
        }
        under(img, &cov, pp.c("color"), op);
    }
}
