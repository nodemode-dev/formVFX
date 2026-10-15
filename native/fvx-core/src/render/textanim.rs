use super::layout::{Glyph, Layout, Mods, layout_with, outline_of};
use super::text::{GlyphPaint, order_ops, paint_ops, style_paint};
use super::vector::DrawOp;
use crate::eval::Eval;
use crate::expr::noise;
use crate::geom::{Mat3, Mat4};
use crate::model::path::BezPath;
use crate::model::{Group, GroupKind, Layer, Node, Prop, TextDoc, Value};

pub struct Glyph3d {
    pub matrix: Mat4,
    pub ops: Vec<DrawOp>,
    pub anchor: [f64; 3],
}

pub type Tint = Option<([f64; 3], f64)>;

pub struct TextRender {
    pub ops: Vec<DrawOp>,
    pub blurred: Vec<(Vec<DrawOp>, [f64; 2])>,
    pub glyphs3d: Option<Vec<Glyph3d>>,
    pub parts: Option<Vec<(Vec<DrawOp>, [Tint; 3])>>,
}

pub fn tint(base: [f64; 4], t: Tint) -> [f64; 4] {
    match t {
        Some((c, w)) => [base[0] + (c[0] - base[0]) * w, base[1] + (c[1] - base[1]) * w, base[2] + (c[2] - base[2]) * w, base[3]],
        None => base,
    }
}

fn add_tint(t: &mut Tint, c: [f64; 3], k: f64) {
    let k = k.clamp(0.0, 1.0);
    if k <= 0.0 {
        return;
    }
    *t = Some(match *t {
        None => (c, k),
        Some((c0, w)) => {
            let nw = 1.0 - (1.0 - w) * (1.0 - k);
            ([0, 1, 2].map(|d| (c0[d] * w * (1.0 - k) + c[d] * k) / nw.max(1e-9)), nw)
        }
    });
}

struct Anim<'a> {
    props: Vec<(&'a Prop, Value)>,
    selectors: Vec<&'a Group>,
}

#[derive(Clone)]
struct Acc {
    pos: [f64; 3],
    anchor: [f64; 3],
    scale: [f64; 3],
    skew: f64,
    skew_axis: f64,
    rot: [f64; 3],
    opacity: f64,
    paint: [f64; 8],
    stroke_width: f64,
    blur: [f64; 2],
    line_spacing: [f64; 2],
    axes: Vec<(String, f64)>,
    parts: [Tint; 3],
}

fn v<'a>(ev: &Eval, l: &Layer, g: &'a Group, id: &str) -> Option<Value> {
    g.get(id).map(|p| ev.value(l, p))
}

fn f(ev: &Eval, l: &Layer, g: &Group, id: &str, d: f64) -> f64 {
    v(ev, l, g, id).map(|x| x.num()).unwrap_or(d)
}

fn e(ev: &Eval, l: &Layer, g: &Group, id: &str) -> u32 {
    v(ev, l, g, id).map(|x| x.index()).unwrap_or(0)
}

pub fn has_animation(l: &Layer) -> bool {
    let Some(t) = l.props.sub("text") else { return false };
    let anims = t.sub("animators").is_some_and(|a| a.groups().any(|g| g.enabled));
    let path = t.sub("pathOptions").and_then(|p| p.get("path")).is_some_and(|p| p.value.index() > 0);
    anims || path
}

fn units_of(lay: &Layout, based: u32) -> (Vec<Option<usize>>, usize) {
    let mut out = Vec::with_capacity(lay.chars.len());
    let mut n = 0usize;
    match based {
        2 => {
            for c in &lay.chars {
                out.push(c.word);
            }
            n = lay.words;
        }
        3 => {
            for c in &lay.chars {
                out.push(if c.newline { None } else { Some(c.line) });
            }
            n = lay.lines.len().max(1);
        }
        1 => {
            for c in &lay.chars {
                if c.space || c.newline {
                    out.push(None);
                } else {
                    out.push(Some(n));
                    n += 1;
                }
            }
        }
        _ => {
            for c in &lay.chars {
                if c.newline {
                    out.push(None);
                } else {
                    out.push(Some(n));
                    n += 1;
                }
            }
        }
    }
    (out, n)
}

fn shuffle(n: usize, seed: f64) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let r = noise::unit(noise::hash(&[seed.to_bits(), i as u64, 0x7e57])) * (i + 1) as f64;
        idx.swap(i, (r as usize).min(i));
    }
    idx
}

fn hermite(t: f64, m0: f64, m1: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    ((t3 - 2.0 * t2 + t) * m0 + (-2.0 * t3 + 3.0 * t2) + (t3 - t2) * m1).clamp(0.0, 1.0)
}

pub fn shape_value(shape: u32, s: f64, e: f64, x0: f64, smooth: f64) -> f64 {
    if e - s <= 1e-9 {
        return 0.0;
    }
    match shape {
        0 => {
            let cov = ((x0 + 1.0).min(e) - x0.max(s)).clamp(0.0, 1.0);
            let step = if cov >= 0.5 { 1.0 } else { 0.0 };
            step + (cov - step) * (smooth / 100.0).clamp(0.0, 1.0)
        }
        1 => ((x0 + 0.5 - s) / (e - s)).clamp(0.0, 1.0),
        2 => 1.0 - ((x0 + 0.5 - s) / (e - s)).clamp(0.0, 1.0),
        _ => {
            let x = x0 + 0.5;
            if x < s || x > e {
                return 0.0;
            }
            let u = 2.0 * (x - s) / (e - s) - 1.0;
            match shape {
                3 => 1.0 - u.abs(),
                4 => (1.0 - u * u).max(0.0).sqrt(),
                _ => 0.5 * (1.0 + (std::f64::consts::PI * u).cos()),
            }
        }
    }
}

fn combine(mode: u32, acc: f64, v: f64) -> f64 {
    match mode {
        1 => acc - v,
        2 => acc * v,
        3 => acc.min(v),
        4 => acc.max(v),
        5 => (acc - v).abs(),
        _ => acc + v,
    }
}

fn noise1(seed: u64, channel: u64, x: f64) -> f64 {
    let mut total = 0.0;
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for o in 0..3u64 {
        let xx = x * freq;
        let i = xx.floor();
        let t = xx - i;
        let a = noise::unit(noise::hash(&[seed, channel, o, i as i64 as u64])) * 2.0 - 1.0;
        let b = noise::unit(noise::hash(&[seed, channel, o, (i as i64 + 1) as u64])) * 2.0 - 1.0;
        let w = t * t * (3.0 - 2.0 * t);
        total += (a + (b - a) * w) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    total / norm
}

fn selections(ev: &Eval, l: &Layer, lay: &Layout, sels: &[&Group]) -> Vec<[f64; 3]> {
    let nch = lay.chars.len();
    let mut acc = vec![[0.0f64; 3]; nch];
    let lt = l.layer_time(ev.t);
    for (si, g) in sels.iter().enumerate() {
        if !g.enabled {
            continue;
        }
        match g.id.as_str() {
            "rangeSelector" => {
                let adv = g.sub("advanced");
                let a = |id: &str, d: f64| adv.map(|x| f(ev, l, x, id, d)).unwrap_or(d);
                let ae = |id: &str| adv.map(|x| e(ev, l, x, id)).unwrap_or(0);
                let based = ae("basedOn");
                let mode = ae("mode");
                let (units, n) = units_of(lay, based);
                let nf = n as f64;
                let index_units = ae("units") == 1;
                let (mut s, mut en, o) = (f(ev, l, g, "start", 0.0), f(ev, l, g, "end", 100.0), f(ev, l, g, "offset", 0.0));
                let (mut s, mut en) = if index_units {
                    s += o;
                    en += o;
                    (s, en)
                } else {
                    s = (s + o) / 100.0 * nf;
                    en = (en + o) / 100.0 * nf;
                    (s, en)
                };
                if s > en {
                    std::mem::swap(&mut s, &mut en);
                }
                let amount = a("amount", 100.0) / 100.0;
                let shape = ae("shape");
                let smooth = a("smoothness", 100.0);
                let (eh, el) = (a("easeHigh", 0.0), a("easeLow", 0.0));
                let perm = if adv.and_then(|x| v(ev, l, x, "randomize")).is_some_and(|x| x.boolean()) { Some(shuffle(n, a("randomSeed", 0.0))) } else { None };
                let first = si == 0 || !sels[..si].iter().any(|x| x.enabled);
                for (ci, u) in units.iter().enumerate() {
                    let val = match u {
                        Some(u) => {
                            let pos = perm.as_ref().map(|p| p[*u]).unwrap_or(*u) as f64;
                            let mut x = shape_value(shape, s, en, pos, smooth);
                            if shape != 0 && (eh != 0.0 || el != 0.0) {
                                x = hermite(x, 1.0 - el / 100.0, 1.0 - eh / 100.0);
                            }
                            x * amount
                        }
                        None => 0.0,
                    };
                    for d in 0..3 {
                        let base = if first && matches!(mode, 1 | 2 | 3) { 1.0 } else { acc[ci][d] };
                        acc[ci][d] = combine(mode, base, val);
                    }
                }
            }
            "wigglySelector" => {
                let mode = e(ev, l, g, "mode");
                let based = e(ev, l, g, "basedOn");
                let (units, _) = units_of(lay, based);
                let mx = f(ev, l, g, "maxAmount", 100.0) / 100.0;
                let mn = f(ev, l, g, "minAmount", -100.0) / 100.0;
                let wps = f(ev, l, g, "wigglesPerSecond", 2.0);
                let corr = (f(ev, l, g, "correlation", 50.0) / 100.0).clamp(0.0, 1.0);
                let tp = f(ev, l, g, "temporalPhase", 0.0) / 360.0;
                let sp = f(ev, l, g, "spatialPhase", 0.0) / 360.0;
                let lock = v(ev, l, g, "lockDimensions").is_some_and(|x| x.boolean());
                let seed = f(ev, l, g, "randomSeed", 0.0).to_bits() ^ l.id.wrapping_mul(0x9E37) ^ g.uid;
                let x = lt * wps + tp;
                let first = si == 0 || !sels[..si].iter().any(|x| x.enabled);
                for (ci, u) in units.iter().enumerate() {
                    let Some(u) = u else { continue };
                    for d in 0..3 {
                        let ch = if lock { 0 } else { d as u64 };
                        let shared = noise1(seed, 1000 + ch, x);
                        let own = noise1(seed, (*u as u64 + 1) * 7 + ch, x + sp * (*u as f64 + 1.0));
                        let nval = corr * shared + (1.0 - corr) * own;
                        let w = mn + (mx - mn) * (0.5 + 0.5 * nval.clamp(-1.0, 1.0));
                        let base = if first && matches!(mode, 1 | 2 | 3) { 1.0 } else { acc[ci][d] };
                        acc[ci][d] = combine(mode, base, w);
                    }
                }
            }
            "expressionSelector" => {
                let based = e(ev, l, g, "basedOn");
                let (units, n) = units_of(lay, based);
                let Some(amount) = g.get("amount") else { continue };
                for (ci, u) in units.iter().enumerate() {
                    let Some(u) = u else {
                        acc[ci] = [0.0; 3];
                        continue;
                    };
                    let sv = [acc[ci][0] * 100.0, acc[ci][1] * 100.0, acc[ci][2] * 100.0];
                    let r = crate::expr::evaluate_text(ev, l, amount, (*u + 1) as f64, n as f64, sv);
                    let c = r.components();
                    let c0 = c.first().copied().unwrap_or(0.0);
                    for d in 0..3 {
                        acc[ci][d] = c.get(d).copied().unwrap_or(c0) / 100.0;
                    }
                }
            }
            _ => {}
        }
    }
    for a in acc.iter_mut() {
        for d in a.iter_mut() {
            *d = d.clamp(-1.0, 1.0);
        }
    }
    acc
}

fn rgb_to_hsv(c: [f64; 3]) -> [f64; 3] {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let d = mx - mn;
    let h = if d < 1e-9 {
        0.0
    } else if mx == c[0] {
        ((c[1] - c[2]) / d).rem_euclid(6.0)
    } else if mx == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    } * 60.0;
    [h, if mx <= 1e-9 { 0.0 } else { d / mx }, mx]
}

fn hsv_to_rgb(h: [f64; 3]) -> [f64; 3] {
    let (hh, s, v) = (h[0].rem_euclid(360.0) / 60.0, h[1].clamp(0.0, 1.0), h[2].max(0.0));
    let c = v * s;
    let x = c * (1.0 - (hh % 2.0 - 1.0).abs());
    let (r, g, b) = match hh as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r + m, g + m, b + m]
}

fn shift_char(c: char, off: i64, full: bool) -> char {
    if off == 0 {
        return c;
    }
    if full {
        return char::from_u32((c as i64 + off).max(0) as u32).unwrap_or(c);
    }
    let cyc = |base: u8, n: i64| -> char { (base + ((c as i64 - base as i64 + off).rem_euclid(n)) as u8) as char };
    match c {
        'a'..='z' => cyc(b'a', 26),
        'A'..='Z' => cyc(b'A', 26),
        '0'..='9' => cyc(b'0', 10),
        _ => c,
    }
}

struct PathMeasure {
    pts: Vec<[f64; 2]>,
    cum: Vec<f64>,
    closed: bool,
}

impl PathMeasure {
    fn new(p: &BezPath, reverse: bool) -> Option<PathMeasure> {
        let mut pts = p.flatten(0.2);
        if pts.len() < 2 {
            return None;
        }
        if reverse {
            pts.reverse();
        }
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            let d = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
            cum.push(cum.last().unwrap() + d);
        }
        Some(PathMeasure { pts, cum, closed: p.closed })
    }

    fn len(&self) -> f64 {
        *self.cum.last().unwrap_or(&0.0)
    }

    fn at(&self, d: f64) -> ([f64; 2], f64) {
        let total = self.len().max(1e-9);
        let d = if self.closed { d.rem_euclid(total) } else { d };
        let n = self.pts.len();
        let seg = if d <= 0.0 {
            0
        } else if d >= total {
            n - 2
        } else {
            self.cum.partition_point(|c| *c <= d).saturating_sub(1).min(n - 2)
        };
        let (a, b) = (self.pts[seg], self.pts[seg + 1]);
        let len = (self.cum[seg + 1] - self.cum[seg]).max(1e-9);
        let t = (d - self.cum[seg]) / len;
        ([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], (b[1] - a[1]).atan2(b[0] - a[0]))
    }
}

fn mask_path(ev: &Eval, l: &Layer, index: u32) -> Option<BezPath> {
    let masks = l.masks()?;
    let m = masks.groups().nth(index.checked_sub(1)? as usize)?;
    m.get("path").map(|p| ev.value(l, p)).and_then(|x| x.as_path().cloned())
}

fn base_acc(g: &Glyph) -> Acc {
    let gp = style_paint(g);
    Acc {
        pos: [0.0; 3],
        anchor: [0.0; 3],
        scale: [1.0; 3],
        skew: 0.0,
        skew_axis: 0.0,
        rot: [0.0; 3],
        opacity: 1.0,
        paint: [gp.fill[0], gp.fill[1], gp.fill[2], gp.fill_opacity, gp.stroke[0], gp.stroke[1], gp.stroke[2], gp.stroke_opacity],
        stroke_width: gp.stroke_width,
        blur: [0.0; 2],
        line_spacing: [0.0; 2],
        axes: vec![],
        parts: [None; 3],
    }
}

fn apply_prop(a: &mut Acc, id: &str, val: &Value, s: [f64; 3], axis_defaults: &dyn Fn(&str) -> f64) {
    let s0 = s[0];
    match id {
        "position" => {
            let p = val.vec3();
            for d in 0..3 {
                a.pos[d] += p[d] * s[d];
            }
        }
        "anchor" => {
            let p = val.vec3();
            for d in 0..3 {
                a.anchor[d] += p[d] * s[d];
            }
        }
        "scale" => {
            let p = val.vec3();
            for d in 0..3 {
                a.scale[d] *= 1.0 + (p[d] / 100.0 - 1.0) * s[d];
            }
        }
        "skew" => a.skew += val.num() * s0,
        "skewAxis" => a.skew_axis = val.num(),
        "rotation" => a.rot[2] += val.num() * s0,
        "rotationX" => a.rot[0] += val.num() * s0,
        "rotationY" => a.rot[1] += val.num() * s0,
        "opacity" => a.opacity *= (1.0 + (val.num() / 100.0 - 1.0) * s0).max(0.0),
        "fillColor" | "strokeColor" => {
            let c = val.color();
            let o = if id == "fillColor" { 0 } else { 4 };
            let k = (s0 * c[3]).clamp(-1.0, 1.0);
            for d in 0..3 {
                a.paint[o + d] += (c[d] - a.paint[o + d]) * k;
            }
        }
        "fillHue" | "fillSaturation" | "fillBrightness" | "strokeHue" | "strokeSaturation" | "strokeBrightness" => {
            let o = if id.starts_with("fill") { 0 } else { 4 };
            let mut hsv = rgb_to_hsv([a.paint[o], a.paint[o + 1], a.paint[o + 2]]);
            let x = val.num() * s0;
            if id.ends_with("Hue") {
                hsv[0] += x;
            } else if id.ends_with("Saturation") {
                hsv[1] = (hsv[1] + x / 100.0).clamp(0.0, 1.0);
            } else {
                hsv[2] = (hsv[2] + x / 100.0).max(0.0);
            }
            let rgb = hsv_to_rgb(hsv);
            a.paint[o..o + 3].copy_from_slice(&rgb);
        }
        "frontColor" | "bevelColor" | "sideColor" => {
            let c = val.color();
            let i = match id {
                "frontColor" => 0,
                "bevelColor" => 1,
                _ => 2,
            };
            add_tint(&mut a.parts[i], [c[0], c[1], c[2]], s0 * c[3]);
        }
        "fillOpacity" => a.paint[3] *= (1.0 + (val.num() / 100.0 - 1.0) * s0).max(0.0),
        "strokeOpacity" => a.paint[7] *= (1.0 + (val.num() / 100.0 - 1.0) * s0).max(0.0),
        "strokeWidth" => a.stroke_width = (a.stroke_width + val.num() * s0).max(0.0),
        "blur" => {
            let b = val.vec2();
            a.blur[0] += b[0] * s0;
            a.blur[1] += b[1] * s0;
        }
        "lineSpacing" => {
            let b = val.vec2();
            a.line_spacing[0] += b[0] * s0;
            a.line_spacing[1] += b[1] * s0;
        }
        other => {
            if let Some(tag) = other.strip_prefix("axis_") {
                let base = a.axes.iter().find(|x| x.0 == tag).map(|x| x.1).unwrap_or_else(|| axis_defaults(tag));
                let nv = base + (val.num() - base) * s0;
                a.axes.retain(|x| x.0 != tag);
                a.axes.push((tag.to_string(), nv));
            }
        }
    }
}

pub fn render_text(ev: &Eval, l: &Layer, doc: &TextDoc) -> TextRender {
    let text = l.props.sub("text");
    let anims: Vec<Anim> = text
        .and_then(|t| t.sub("animators"))
        .map(|a| {
            a.groups()
                .filter(|g| g.enabled)
                .map(|g| Anim {
                    props: g.sub("properties").map(|p| p.children.iter().filter_map(|c| if let Node::Prop(p) = c { Some((p, ev.value(l, p))) } else { None }).collect()).unwrap_or_default(),
                    selectors: g.sub("selectors").map(|s| s.groups().collect()).unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    let more = text.and_then(|t| t.sub("moreOptions"));
    let grouping = more.map(|m| e(ev, l, m, "anchorGrouping")).unwrap_or(0);
    let galign = more.and_then(|m| v(ev, l, m, "groupingAlignment")).map(|x| x.vec2()).unwrap_or([0.0; 2]);
    let fill_mode = more.map(|m| e(ev, l, m, "fillStroke")).unwrap_or(0);
    let popts = text.and_then(|t| t.sub("pathOptions"));
    let path_index = popts.and_then(|p| p.get("path")).map(|p| p.value.index()).unwrap_or(0);
    let per_char_3d = l.is_3d() && text.and_then(|t| t.get("perChar3d")).is_some_and(|p| p.value.boolean());

    let first = layout_with(doc, &Mods::default());
    let sel: Vec<Vec<[f64; 3]>> = anims.iter().map(|a| if a.selectors.is_empty() { vec![[1.0; 3]; first.chars.len()] } else { selections(ev, l, &first, &a.selectors) }).collect();
    let nch = first.chars.len();
    let mut mods = Mods { chars: None, track_before: vec![0.0; nch], track_after: vec![0.0; nch] };
    let mut changed = false;
    let mut new_chars: Vec<char> = first.chars.iter().map(|c| c.ch).collect();
    let mut char_off = vec![0i64; nch];
    for (ai, a) in anims.iter().enumerate() {
        let full = a.props.iter().find(|(p, _)| p.id == "characterRange").is_some_and(|(_, v)| v.index() == 1);
        let ttype = a.props.iter().find(|(p, _)| p.id == "trackingType").map(|(_, v)| v.index()).unwrap_or(0);
        for (p, val) in &a.props {
            match p.id.as_str() {
                "tracking" => {
                    for ci in 0..nch {
                        let x = val.num() * sel[ai][ci][0] / 1000.0 * doc.style_at(ci).size;
                        if x != 0.0 {
                            changed = true;
                            match ttype {
                                1 => mods.track_before[ci] += x,
                                2 => mods.track_after[ci] += x,
                                _ => {
                                    mods.track_before[ci] += x / 2.0;
                                    mods.track_after[ci] += x / 2.0;
                                }
                            }
                        }
                    }
                }
                "characterOffset" => {
                    for ci in 0..nch {
                        let off = (val.num() * sel[ai][ci][0]).round() as i64;
                        if off != 0 {
                            char_off[ci] += off;
                        }
                    }
                    for ci in 0..nch {
                        if char_off[ci] != 0 && !first.chars[ci].space && !first.chars[ci].newline {
                            new_chars[ci] = shift_char(first.chars[ci].ch, char_off[ci], full);
                            changed = true;
                        }
                    }
                }
                "characterValue" => {
                    for ci in 0..nch {
                        if sel[ai][ci][0] >= 0.5 && !first.chars[ci].newline {
                            if let Some(c) = char::from_u32(val.num().max(0.0) as u32) {
                                new_chars[ci] = c;
                                changed = true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let lay = if changed {
        mods.chars = Some(new_chars);
        layout_with(doc, &mods)
    } else {
        first
    };

    let axis_default = |tag: &str| -> f64 {
        doc.variations.get(tag).copied().unwrap_or_else(|| {
            lay.glyphs.first().and_then(|g| g.font).map(|id| super::layout::font_axes(id).into_iter().find(|a| a.tag == tag).map(|a| a.default).unwrap_or(0.0)).unwrap_or(0.0)
        })
    };

    let mut accs: Vec<Acc> = lay.glyphs.iter().map(base_acc).collect();
    for (ai, a) in anims.iter().enumerate() {
        for (gi, g) in lay.glyphs.iter().enumerate() {
            let s = sel[ai].get(g.char_index).copied().unwrap_or([0.0; 3]);
            if s == [0.0; 3] {
                continue;
            }
            for (p, val) in &a.props {
                apply_prop(&mut accs[gi], &p.id, val, s, &axis_default);
            }
        }
    }

    let anchor_of = |gi: usize| -> [f64; 2] {
        let g = &lay.glyphs[gi];
        let (x0, x1) = match grouping {
            1 => {
                let w = lay.chars.get(g.char_index).and_then(|c| c.word);
                let same: Vec<&Glyph> = lay.glyphs.iter().filter(|h| h.line == g.line && w.is_some() && lay.chars.get(h.char_index).and_then(|c| c.word) == w).collect();
                if same.is_empty() { (g.x, g.x + g.advance) } else { (same.iter().map(|h| h.x).fold(f64::MAX, f64::min), same.iter().map(|h| h.x + h.advance).fold(f64::MIN, f64::max)) }
            }
            2 => lay.lines.get(g.line).map(|li| (li.0, li.0 + li.2)).unwrap_or((g.x, g.x + g.advance)),
            3 => (lay.lines.iter().map(|li| li.0).fold(f64::MAX, f64::min), lay.lines.iter().map(|li| li.0 + li.2).fold(f64::MIN, f64::max)),
            _ => (g.x, g.x + g.advance),
        };
        let y = match grouping {
            3 => (lay.lines.first().map(|l| l.1).unwrap_or(0.0) + lay.lines.last().map(|l| l.1).unwrap_or(0.0)) / 2.0,
            _ => g.baseline,
        };
        [(x0 + x1) / 2.0 + galign[0] / 100.0 * (x1 - x0), y + galign[1] / 100.0 * g.size]
    };

    let measure = if path_index > 0 {
        let rev = popts.and_then(|p| v(ev, l, p, "reversePath")).is_some_and(|x| x.boolean());
        mask_path(ev, l, path_index).and_then(|p| PathMeasure::new(&p, rev))
    } else {
        None
    };
    let perpendicular = popts.and_then(|p| v(ev, l, p, "perpendicular")).is_none_or(|x| x.boolean());
    let force = popts.and_then(|p| v(ev, l, p, "forceAlignment")).is_some_and(|x| x.boolean());
    let fm = popts.map(|p| f(ev, l, p, "firstMargin", 0.0)).unwrap_or(0.0);
    let lm = popts.map(|p| f(ev, l, p, "lastMargin", 0.0)).unwrap_or(0.0);
    let first_base = lay.lines.first().map(|x| x.1).unwrap_or(0.0);

    let mut per_glyph: Vec<(Vec<DrawOp>, Vec<DrawOp>)> = vec![];
    let mut blurred: Vec<(Vec<DrawOp>, [f64; 2])> = vec![];
    let mut g3: Vec<Glyph3d> = vec![];
    let mut parts = anims.iter().any(|a| a.props.iter().any(|(p, _)| matches!(p.id.as_str(), "frontColor" | "bevelColor" | "sideColor"))).then(Vec::new);
    let line_glyphs: Vec<Vec<usize>> = (0..lay.lines.len().max(1)).map(|li| (0..lay.glyphs.len()).filter(|i| lay.glyphs[*i].line == li).collect()).collect();
    for (gi, g) in lay.glyphs.iter().enumerate() {
        let a = &accs[gi];
        let anchor = if measure.is_some() { [g.x + g.advance / 2.0, g.baseline] } else { anchor_of(gi) };
        let mut base = Mat3::translate(anchor[0], anchor[1]);
        if let Some(m) = &measure {
            let (lx0, _, lw) = lay.lines.get(g.line).copied().unwrap_or((0.0, 0.0, 0.0));
            let total = m.len();
            let along = anchor[0] - lx0;
            let d = if force {
                let ids = &line_glyphs[g.line.min(line_glyphs.len() - 1)];
                let k = ids.iter().position(|x| *x == gi).unwrap_or(0);
                if m.closed {
                    fm + g.advance / 2.0 + (total - fm - lm) * k as f64 / ids.len().max(1) as f64
                } else {
                    let n = ids.len().max(2) - 1;
                    let span = total - fm - lm - g.advance;
                    fm + g.advance / 2.0 + span * k as f64 / n as f64
                }
            } else {
                let start = match doc.justify {
                    crate::model::Justify::Center | crate::model::Justify::JustifyCenter => (fm + total - lm) / 2.0 - lw / 2.0,
                    crate::model::Justify::Right | crate::model::Justify::JustifyRight => total - lm - lw,
                    _ => fm,
                };
                start + along
            };
            let (pt, ang) = m.at(d);
            base = Mat3::translate(pt[0], pt[1]) * if perpendicular { Mat3::rotate_deg(ang.to_degrees()) } else { Mat3::IDENTITY } * Mat3::translate(0.0, g.baseline - first_base);
        }
        let ls = [a.line_spacing[0] * g.line as f64, a.line_spacing[1] * g.line as f64];
        let gp = GlyphPaint { fill: [a.paint[0], a.paint[1], a.paint[2]], fill_opacity: a.paint[3], stroke: [a.paint[4], a.paint[5], a.paint[6]], stroke_opacity: a.paint[7], stroke_width: a.stroke_width, opacity: a.opacity.clamp(0.0, 1.0) };
        let src_paths: Vec<BezPath> = if a.axes.is_empty() {
            g.paths.clone()
        } else {
            let mut vars = g.vars.clone();
            for (t, x) in &a.axes {
                vars.retain(|v| &v.0 != t);
                vars.push((t.clone(), *x));
            }
            match g.font {
                Some(id) => outline_of(id, g.gid, &vars).iter().map(|p| p.transformed(&g.place)).collect(),
                None => g.paths.clone(),
            }
        };
        if per_char_3d {
            let a4 = Mat4::translate(a.pos[0] + ls[0], a.pos[1] + ls[1], a.pos[2])
                * Mat4::rot_x(a.rot[0])
                * Mat4::rot_y(a.rot[1])
                * Mat4::rot_z(a.rot[2])
                * Mat4::from_mat3_affine(&Mat3::skew(a.skew, a.skew_axis))
                * Mat4::scale(a.scale[0], a.scale[1], a.scale[2])
                * Mat4::translate(-a.anchor[0], -a.anchor[1], -a.anchor[2]);
            let m4 = Mat4::from_mat3_affine(&base) * a4 * Mat4::translate(-anchor[0], -anchor[1], 0.0);
            let (mut fo, mut so) = (vec![], vec![]);
            paint_ops(doc, src_paths, g, &gp, &mut fo, &mut so);
            let ops = order_ops(doc, vec![(fo, so)], 0);
            if !ops.is_empty() {
                g3.push(Glyph3d { matrix: m4, ops, anchor: [anchor[0], anchor[1], 0.0] });
            }
            continue;
        }
        let a2 = Mat3::translate(a.pos[0] + ls[0], a.pos[1] + ls[1]) * Mat3::rotate_deg(a.rot[2]) * Mat3::skew(a.skew, a.skew_axis) * Mat3::scale(a.scale[0], a.scale[1]) * Mat3::translate(-a.anchor[0], -a.anchor[1]);
        let m = base * a2 * Mat3::translate(-anchor[0], -anchor[1]);
        let paths: Vec<BezPath> = src_paths.iter().map(|p| p.transformed(&m)).collect();
        let (mut fo, mut so) = (vec![], vec![]);
        paint_ops(doc, paths, g, &gp, &mut fo, &mut so);
        if let Some(pp) = parts.as_mut() {
            pp.push((fo.clone(), a.parts));
        }
        if a.blur[0] > 0.05 || a.blur[1] > 0.05 {
            let ops = order_ops(doc, vec![(fo, so)], 0);
            blurred.push((ops, [a.blur[0].max(0.0), a.blur[1].max(0.0)]));
        } else {
            per_glyph.push((fo, so));
        }
    }
    let ops = order_ops(doc, per_glyph, fill_mode);
    TextRender { ops, blurred, glyphs3d: if per_char_3d { Some(g3) } else { None }, parts }
}

pub fn animator_count(l: &Layer) -> usize {
    l.props.sub("text").and_then(|t| t.sub("animators")).map(|a| a.groups().count()).unwrap_or(0)
}

pub fn ensure_text_options(l: &mut Layer, ids: &mut crate::model::Ids) {
    let Some(t) = l.props.sub_mut("text") else { return };
    if t.get("perChar3d").is_none() {
        t.children.insert(1.min(t.children.len()), Node::Prop(ids.prop("perChar3d", "Per-character 3D", Value::Bool(false), crate::model::Ui::Hidden).fixed()));
    }
    if t.sub("pathOptions").is_none() {
        let at = t.children.iter().position(|c| c.id() == "animators").unwrap_or(t.children.len());
        t.children.insert(at, Node::Group(crate::model::prop::path_options(ids)));
    }
    if t.sub("moreOptions").is_none() {
        let at = t.children.iter().position(|c| c.id() == "animators").unwrap_or(t.children.len());
        t.children.insert(at, Node::Group(crate::model::prop::more_options(ids)));
    }
    if t.sub("animators").is_none() {
        t.children.push(Node::Group(ids.group("animators", "Animators", GroupKind::Indexed, vec![])));
    }
}
