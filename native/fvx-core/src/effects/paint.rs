use std::collections::HashMap;

use rayon::prelude::*;

use super::{EffectSpec, FxCtx, check, no_pad};
use crate::model::path::BezPath;
use crate::model::prop::{Group, GroupKind, Ids, Node, Ui};
use crate::model::{BlendMode, Value};
use crate::render::blend::blend_px;
use crate::render::image::Img;

pub const ID: &str = "fx.paint.paint";
pub const CHANNELS: [&str; 3] = ["RGBA", "RGB", "Alpha"];
pub const DURATIONS: [&str; 4] = ["constant", "writeOn", "singleFrame", "custom"];
pub const ERASE_MODES: [&str; 3] = ["layerSourceAndPaint", "paintOnly", "lastStrokeOnly"];
pub const KINDS: [&str; 3] = ["brush", "clone", "eraser"];
pub const FOREVER: f64 = 1.0e9;
pub const MODES: [BlendMode; 22] = [
    BlendMode::Normal,
    BlendMode::Darken,
    BlendMode::Multiply,
    BlendMode::ColorBurn,
    BlendMode::LinearBurn,
    BlendMode::Add,
    BlendMode::Lighten,
    BlendMode::Screen,
    BlendMode::ColorDodge,
    BlendMode::LinearDodge,
    BlendMode::Overlay,
    BlendMode::SoftLight,
    BlendMode::HardLight,
    BlendMode::LinearLight,
    BlendMode::VividLight,
    BlendMode::PinLight,
    BlendMode::Difference,
    BlendMode::Exclusion,
    BlendMode::Hue,
    BlendMode::Saturation,
    BlendMode::Color,
    BlendMode::Luminosity,
];

pub fn specs() -> Vec<EffectSpec> {
    vec![EffectSpec { id: ID, name: "Paint", category: "Paint", params: vec![check("paintOnTransparent", "Paint on Transparent", false)], render, pad: no_pad }]
}

#[derive(Clone, Debug)]
pub struct Tip {
    pub color: [f64; 4],
    pub diameter: f64,
    pub angle: f64,
    pub hardness: f64,
    pub roundness: f64,
    pub spacing: f64,
    pub opacity: f64,
    pub flow: f64,
    pub channels: u32,
    pub mode: u32,
}

impl Default for Tip {
    fn default() -> Self {
        Tip { color: [1.0, 1.0, 1.0, 1.0], diameter: 13.0, angle: 0.0, hardness: 100.0, roundness: 100.0, spacing: 25.0, opacity: 100.0, flow: 100.0, channels: 0, mode: 0 }
    }
}

pub struct NewStroke {
    pub kind: usize,
    pub points: Vec<[f64; 3]>,
    pub tip: Tip,
    pub in_time: f64,
    pub out_time: f64,
    pub erase_mode: u32,
    pub dynamics: [f64; 4],
    pub clone: Option<(Option<u64>, [f64; 2], f64, bool, f64)>,
}

fn hidden(ids: &mut Ids, id: &str, name: &str, v: Value) -> Node {
    Node::Prop(ids.prop(id, name, v, Ui::Hidden))
}

fn slider(ids: &mut Ids, id: &str, name: &str, v: f64, max: f64, smax: f64) -> Node {
    Node::Prop(ids.prop(id, name, Value::Num(v), Ui::slider(0.0, max, 0.0, smax, 1)))
}

pub fn stroke_group(ids: &mut Ids, gid: &str, name: &str, s: &NewStroke) -> Group {
    let verts: Vec<[f64; 2]> = s.points.iter().map(|p| [p[0], p[1]]).collect();
    let pressure: Vec<String> = s.points.iter().map(|p| format!("{:.3}", p[2])).collect();
    let first = verts.first().copied().unwrap_or([0.0; 2]);
    let t = &s.tip;
    let mut opts = vec![
        slider(ids, "start", "Start", 0.0, 100.0, 100.0),
        slider(ids, "end", "End", 100.0, 100.0, 100.0),
        Node::Prop(ids.prop("color", "Color", Value::Color(t.color), Ui::Color)),
        slider(ids, "diameter", "Diameter", t.diameter, 10_000.0, 500.0),
        Node::Prop(ids.prop("angle", "Angle", Value::Num(t.angle), Ui::Angle)),
        slider(ids, "hardness", "Hardness", t.hardness, 100.0, 100.0),
        slider(ids, "roundness", "Roundness", t.roundness, 100.0, 100.0),
        slider(ids, "spacing", "Spacing", t.spacing, 1000.0, 200.0),
        Node::Prop(ids.prop("channels", "Channels", Value::Enum(t.channels), Ui::popup(&CHANNELS))),
        slider(ids, "opacity", "Opacity", t.opacity, 100.0, 100.0),
        slider(ids, "flow", "Flow", t.flow, 100.0, 100.0),
    ];
    if let Some((src, pos, shift, lock, at)) = s.clone {
        opts.push(Node::Prop(ids.prop("cloneSource", "Clone Source", Value::Layer(src), Ui::Layer)));
        opts.push(Node::Prop(ids.prop("clonePosition", "Clone Position", Value::Vec2(pos), Ui::Point)));
        opts.push(Node::Prop(ids.prop("cloneTimeShift", "Clone Time Shift", Value::Num(shift), Ui::Plain)));
        opts.push(hidden(ids, "lockSourceTime", "Lock Source Time", Value::Bool(lock)));
        opts.push(hidden(ids, "cloneTime", "Clone Time", Value::Num(at)));
    }
    let so = ids.group("strokeOptions", "Stroke Options", GroupKind::Plain, opts);
    let tr = vec![
        Node::Prop(ids.prop("anchor", "Anchor Point", Value::Vec2(first), Ui::Point)),
        Node::Prop(ids.prop("position", "Position", Value::Vec2(first), Ui::Point).spatial()),
        Node::Prop(ids.prop("scale", "Scale", Value::Vec2([100.0, 100.0]), Ui::Plain)),
        Node::Prop(ids.prop("rotation", "Rotation", Value::Num(0.0), Ui::Angle)),
    ];
    let tg = ids.group("transform", &format!("Transform: {name}"), GroupKind::Plain, tr);
    let children = vec![
        hidden(ids, "kind", "Kind", Value::Str(KINDS[s.kind.min(2)].into())),
        Node::Prop(ids.prop("path", "Path", Value::Path(BezPath::polygon(&verts, false)), Ui::Path)),
        Node::Group(so),
        Node::Group(tg),
        hidden(ids, "mode", "Mode", Value::Enum(t.mode)),
        hidden(ids, "inTime", "In", Value::Num(s.in_time)),
        hidden(ids, "outTime", "Out", Value::Num(s.out_time)),
        hidden(ids, "eraseMode", "Erase Mode", Value::Enum(s.erase_mode)),
        hidden(ids, "pressure", "Pressure", Value::Str(pressure.join(","))),
        hidden(ids, "dynamics", "Brush Dynamics", Value::Str(format!("{},{},{},{}", s.dynamics[0], s.dynamics[1], s.dynamics[2], s.dynamics[3]))),
    ];
    ids.group(gid, name, GroupKind::Plain, children)
}

struct Stroke {
    kind: usize,
    path: BezPath,
    pressure: Vec<f64>,
    dynamics: [f64; 4],
    start: f64,
    end: f64,
    tip: Tip,
    anchor: [f64; 2],
    position: [f64; 2],
    scale: [f64; 2],
    rotation: f64,
    in_time: f64,
    out_time: f64,
    erase_mode: u32,
    clone_source: Option<u64>,
    clone_position: [f64; 2],
    clone_shift: f64,
    lock_time: bool,
    clone_time: f64,
}

fn strokes(c: &FxCtx) -> Vec<Stroke> {
    let mut prefixes: Vec<&str> = c.p.keys().filter_map(|k| k.strip_suffix("/kind")).filter(|k| !k.contains('/')).collect();
    prefixes.sort();
    let num = |k: String, d: f64| c.p.get(&k).map(|v| v.num()).unwrap_or(d);
    prefixes
        .into_iter()
        .filter_map(|g| {
            let kind = match c.p.get(&format!("{g}/kind")) {
                Some(Value::Str(s)) => KINDS.iter().position(|k| k == s).unwrap_or(0),
                _ => 0,
            };
            let path = c.p.get(&format!("{g}/path")).and_then(|v| v.as_path().cloned())?;
            let o = |id: &str| format!("{g}/strokeOptions/{id}");
            let tr = |id: &str| format!("{g}/transform/{id}");
            let s = |id: String| match c.p.get(&id) {
                Some(Value::Str(s)) => s.clone(),
                _ => String::new(),
            };
            let pressure: Vec<f64> = s(format!("{g}/pressure")).split(',').filter_map(|x| x.trim().parse().ok()).collect();
            let dy: Vec<f64> = s(format!("{g}/dynamics")).split(',').filter_map(|x| x.trim().parse().ok()).collect();
            let col = c.p.get(&o("color")).map(|v| v.color()).unwrap_or([1.0; 4]);
            let v2 = |k: String, d: [f64; 2]| c.p.get(&k).map(|v| v.vec2()).unwrap_or(d);
            let tip = Tip {
                color: col,
                diameter: num(o("diameter"), 13.0),
                angle: num(o("angle"), 0.0),
                hardness: num(o("hardness"), 100.0),
                roundness: num(o("roundness"), 100.0),
                spacing: num(o("spacing"), 25.0),
                opacity: num(o("opacity"), 100.0),
                flow: num(o("flow"), 100.0),
                channels: c.p.get(&o("channels")).map(|v| v.index()).unwrap_or(0),
                mode: c.p.get(&format!("{g}/mode")).map(|v| v.index()).unwrap_or(0),
            };
            Some(Stroke {
                kind,
                path,
                pressure,
                dynamics: [dy.first().copied().unwrap_or(0.0), dy.get(1).copied().unwrap_or(0.0), dy.get(2).copied().unwrap_or(0.0), dy.get(3).copied().unwrap_or(0.0)],
                start: num(o("start"), 0.0),
                end: num(o("end"), 100.0),
                tip,
                anchor: v2(tr("anchor"), [0.0; 2]),
                position: v2(tr("position"), [0.0; 2]),
                scale: v2(tr("scale"), [100.0, 100.0]),
                rotation: num(tr("rotation"), 0.0),
                in_time: num(format!("{g}/inTime"), 0.0),
                out_time: num(format!("{g}/outTime"), FOREVER),
                erase_mode: c.p.get(&format!("{g}/eraseMode")).map(|v| v.index()).unwrap_or(0),
                clone_source: c.p.get(&o("cloneSource")).and_then(|v| v.layer()),
                clone_position: v2(o("clonePosition"), [0.0; 2]),
                clone_shift: num(o("cloneTimeShift"), 0.0),
                lock_time: c.p.get(&o("lockSourceTime")).is_some_and(|v| v.boolean()),
                clone_time: num(o("cloneTime"), 0.0),
            })
        })
        .collect()
}

struct Dab {
    p: [f64; 2],
    size: f64,
    alpha: f64,
}

fn dabs(s: &Stroke, c: &FxCtx) -> Vec<Dab> {
    let (sn, cs) = s.rotation.to_radians().sin_cos();
    let place = |p: [f64; 2]| -> [f64; 2] {
        let d = [(p[0] - s.anchor[0]) * s.scale[0] / 100.0, (p[1] - s.anchor[1]) * s.scale[1] / 100.0];
        c.to_buf([s.position[0] + d[0] * cs - d[1] * sn, s.position[1] + d[0] * sn + d[1] * cs])
    };
    let verts: Vec<([f64; 2], f64)> = s.path.vertices.iter().enumerate().map(|(i, v)| (place(*v), s.pressure.get(i).copied().unwrap_or(1.0).clamp(0.0, 1.0))).collect();
    if verts.is_empty() {
        return vec![];
    }
    let mut cum = vec![0.0];
    for w in verts.windows(2) {
        let l = (w[1].0[0] - w[0].0[0]).hypot(w[1].0[1] - w[0].0[1]);
        cum.push(cum.last().copied().unwrap_or(0.0) + l);
    }
    let total = cum.last().copied().unwrap_or(0.0);
    let (a, b) = ((s.start.min(s.end) / 100.0).clamp(0.0, 1.0) * total, (s.end.max(s.start) / 100.0).clamp(0.0, 1.0) * total);
    if s.end <= s.start + 1e-9 && total > 0.0 {
        return vec![];
    }
    let base = (s.tip.diameter * c.r).max(0.5);
    let step = (base * s.tip.spacing / 100.0).max(0.5);
    let dyn_size = |pr: f64| if s.dynamics[0] > 0.0 { (s.dynamics[1] / 100.0) + (1.0 - s.dynamics[1] / 100.0) * pr } else { 1.0 };
    let dyn_alpha = |pr: f64| if s.dynamics[2] > 0.0 { pr } else { 1.0 };
    let at = |d: f64| -> ([f64; 2], f64) {
        let i = cum.partition_point(|x| *x < d).clamp(1, cum.len().max(2) - 1).min(verts.len() - 1);
        if verts.len() == 1 {
            return verts[0];
        }
        let (p0, p1) = (verts[i - 1], verts[i]);
        let seg = (cum[i] - cum[i - 1]).max(1e-9);
        let t = ((d - cum[i - 1]) / seg).clamp(0.0, 1.0);
        ([p0.0[0] + (p1.0[0] - p0.0[0]) * t, p0.0[1] + (p1.0[1] - p0.0[1]) * t], p0.1 + (p1.1 - p0.1) * t)
    };
    let mut out = vec![];
    let mut d = a;
    loop {
        let (p, pr) = at(d);
        out.push(Dab { p, size: base * dyn_size(pr), alpha: dyn_alpha(pr) });
        if d >= b {
            break;
        }
        d = (d + step).min(b);
        if out.len() > 200_000 {
            break;
        }
    }
    out
}

fn coverage(s: &Stroke, c: &FxCtx, w: usize, h: usize) -> Option<(Vec<f32>, [usize; 4])> {
    let ds = dabs(s, c);
    if ds.is_empty() {
        return None;
    }
    let maxr = ds.iter().map(|d| d.size).fold(0.0f64, f64::max) * 0.5 + 2.0;
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for d in &ds {
        x0 = x0.min(d.p[0]);
        y0 = y0.min(d.p[1]);
        x1 = x1.max(d.p[0]);
        y1 = y1.max(d.p[1]);
    }
    let bx0 = (x0 - maxr).floor().max(0.0) as usize;
    let by0 = (y0 - maxr).floor().max(0.0) as usize;
    let bx1 = ((x1 + maxr).ceil().max(0.0) as usize).min(w);
    let by1 = ((y1 + maxr).ceil().max(0.0) as usize).min(h);
    if bx1 <= bx0 || by1 <= by0 {
        return None;
    }
    let (bw, bh) = (bx1 - bx0, by1 - by0);
    let flow = (s.tip.flow / 100.0).clamp(0.0, 1.0) as f32;
    let hard = (s.tip.hardness / 100.0).clamp(0.0, 1.0);
    let round = (s.tip.roundness / 100.0).clamp(0.01, 1.0);
    let (sa, ca) = (-s.tip.angle.to_radians()).sin_cos();
    let cell = (maxr * 2.0).max(8.0);
    let gw = ((bw as f64) / cell).ceil().max(1.0) as usize;
    let gh = ((bh as f64) / cell).ceil().max(1.0) as usize;
    let mut grid: Vec<Vec<usize>> = vec![vec![]; gw * gh];
    for (i, d) in ds.iter().enumerate() {
        let r = d.size * 0.5 + 1.0;
        let gx0 = (((d.p[0] - r - bx0 as f64) / cell).floor().max(0.0) as usize).min(gw - 1);
        let gx1 = (((d.p[0] + r - bx0 as f64) / cell).floor().max(0.0) as usize).min(gw - 1);
        let gy0 = (((d.p[1] - r - by0 as f64) / cell).floor().max(0.0) as usize).min(gh - 1);
        let gy1 = (((d.p[1] + r - by0 as f64) / cell).floor().max(0.0) as usize).min(gh - 1);
        for gy in gy0..=gy1 {
            for gx in gx0..=gx1 {
                grid[gy * gw + gx].push(i);
            }
        }
    }
    let mut cov = vec![0.0f32; bw * bh];
    cov.par_chunks_mut(bw).enumerate().for_each(|(yy, row)| {
        let y = (by0 + yy) as f64 + 0.5;
        let gy = ((yy as f64 / cell) as usize).min(gh - 1);
        for (xx, v) in row.iter_mut().enumerate() {
            let x = (bx0 + xx) as f64 + 0.5;
            let gx = ((xx as f64 / cell) as usize).min(gw - 1);
            let mut acc = 0.0f32;
            for &i in &grid[gy * gw + gx] {
                let d = &ds[i];
                let r = d.size * 0.5;
                let (dx, dy) = (x - d.p[0], y - d.p[1]);
                let (u, w2) = (dx * ca - dy * sa, (dx * sa + dy * ca) / round);
                let dist = u.hypot(w2);
                if dist > r + 0.5 {
                    continue;
                }
                let inner = r * hard;
                let k = if dist <= inner {
                    (r + 0.5 - dist).clamp(0.0, 1.0)
                } else {
                    let t = ((dist - inner) / (r - inner).max(1e-6)).clamp(0.0, 1.0);
                    (1.0 - t * t * (3.0 - 2.0 * t)) * (r + 0.5 - dist).clamp(0.0, 1.0).max(if hard < 1.0 { 1.0 } else { 0.0 })
                };
                let k = (k * d.alpha) as f32 * flow;
                acc = acc + (1.0 - acc) * k;
            }
            *v = acc;
        }
    });
    let op = (s.tip.opacity / 100.0).clamp(0.0, 1.0) as f32;
    cov.iter_mut().for_each(|v| *v *= op);
    Some((cov, [bx0, by0, bw, bh]))
}

fn luminance(c: [f64; 4]) -> f32 {
    (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]) as f32
}

fn stroke_layer(s: &Stroke, c: &FxCtx, w: usize, h: usize, sources: &mut HashMap<(Option<u64>, i64), Option<Img>>) -> Option<(Img, Vec<f32>, [usize; 4])> {
    let (cov, b) = coverage(s, c, w, h)?;
    let mut img = Img::new(b[2], b[3]);
    match s.kind {
        1 => {
            let first = s.path.vertices.first().copied().unwrap_or([0.0; 2]);
            let off = [s.clone_position[0] - first[0], s.clone_position[1] - first[1]];
            let dt = if s.lock_time { s.clone_time - c.t } else { s.clone_shift };
            let key = (s.clone_source, (dt * 1000.0).round() as i64);
            let src = sources.entry(key).or_insert_with(|| match s.clone_source {
                Some(id) => c.host.and_then(|hst| hst.layer(id, false, dt)).map(|x| x.0),
                None => c.host.and_then(|hst| hst.original()),
            });
            let src = src.as_ref()?;
            let own = s.clone_source.is_none();
            for yy in 0..b[3] {
                for xx in 0..b[2] {
                    let k = cov[yy * b[2] + xx];
                    if k <= 0.0 {
                        continue;
                    }
                    let (bx, by) = ((b[0] + xx) as f64 + 0.5, (b[1] + yy) as f64 + 0.5);
                    let px = if own {
                        src.sample(bx + off[0] * c.r, by + off[1] * c.r, true)
                    } else {
                        let lp = [bx / c.r + c.origin[0] + off[0], by / c.r + c.origin[1] + off[1]];
                        src.sample(lp[0] * c.r, lp[1] * c.r, true)
                    };
                    let i = (yy * b[2] + xx) * 4;
                    for ch in 0..4 {
                        img.px[i + ch] = px[ch] * k;
                    }
                }
            }
        }
        _ => {
            let col = s.tip.color;
            for (i, k) in cov.iter().enumerate() {
                img.px[i * 4] = col[0] as f32 * k;
                img.px[i * 4 + 1] = col[1] as f32 * k;
                img.px[i * 4 + 2] = col[2] as f32 * k;
                img.px[i * 4 + 3] = *k;
            }
        }
    }
    Some((img, cov, b))
}

fn over(dst: &mut Img, src: &Img, b: [usize; 4], mode: BlendMode, channels: u32, alpha_lum: f32) {
    for yy in 0..b[3] {
        for xx in 0..b[2] {
            let si = (yy * b[2] + xx) * 4;
            let s = [src.px[si], src.px[si + 1], src.px[si + 2], src.px[si + 3]];
            if s[3] <= 0.0 {
                continue;
            }
            let (x, y) = (b[0] + xx, b[1] + yy);
            let di = (y * dst.w + x) * 4;
            let d = [dst.px[di], dst.px[di + 1], dst.px[di + 2], dst.px[di + 3]];
            let o = match channels {
                1 => {
                    let mixed = blend_px(mode, d, s, x as i64, y as i64);
                    let a = d[3];
                    let k = if mixed[3] > 1e-6 { a / mixed[3] } else { 0.0 };
                    [mixed[0] * k, mixed[1] * k, mixed[2] * k, a]
                }
                2 => {
                    let target = alpha_lum;
                    let na = d[3] + (target - d[3]) * s[3];
                    let k = if d[3] > 1e-6 { na / d[3] } else { 0.0 };
                    [d[0] * k, d[1] * k, d[2] * k, na]
                }
                _ => blend_px(mode, d, s, x as i64, y as i64),
            };
            dst.px[di..di + 4].copy_from_slice(&o);
        }
    }
}

fn erase(dst: &mut Img, cov: &[f32], b: [usize; 4]) {
    for yy in 0..b[3] {
        for xx in 0..b[2] {
            let k = 1.0 - cov[yy * b[2] + xx];
            if k >= 1.0 {
                continue;
            }
            let di = ((b[1] + yy) * dst.w + b[0] + xx) * 4;
            for ch in 0..4 {
                dst.px[di + ch] *= k;
            }
        }
    }
}

fn render(c: &FxCtx, img: &mut Img) {
    let list = strokes(c);
    if list.is_empty() {
        return;
    }
    let (w, h) = (img.w, img.h);
    let transparent = c.b("paintOnTransparent");
    let mut base = if transparent { Img::new(w, h) } else { img.clone() };
    let mut paint = Img::new(w, h);
    let mut pending: Option<(Img, [usize; 4], BlendMode, u32, f32)> = None;
    let mut sources: HashMap<(Option<u64>, i64), Option<Img>> = HashMap::new();
    let flush = |paint: &mut Img, pending: &mut Option<(Img, [usize; 4], BlendMode, u32, f32)>| {
        if let Some((s, b, mode, ch, al)) = pending.take() {
            over(paint, &s, b, mode, ch, al);
        }
    };
    for s in &list {
        if c.t + 1e-9 < s.in_time || c.t >= s.out_time {
            continue;
        }
        if s.kind == 2 {
            let Some((cov, b)) = coverage(s, c, w, h) else { continue };
            match s.erase_mode {
                2 => {
                    if let Some((p, pbm, _, _, _)) = pending.as_mut() {
                        let pb = *pbm;
                        let local: Vec<f32> = (0..pb[3])
                            .flat_map(|yy| {
                                let cov = &cov;
                                (0..pb[2]).map(move |xx| {
                                    let (x, y) = (pb[0] + xx, pb[1] + yy);
                                    if x >= b[0] && y >= b[1] && x < b[0] + b[2] && y < b[1] + b[3] { cov[(y - b[1]) * b[2] + x - b[0]] } else { 0.0 }
                                })
                            })
                            .collect();
                        erase(p, &local, [0, 0, pb[2], pb[3]]);
                    }
                }
                1 => {
                    flush(&mut paint, &mut pending);
                    erase(&mut paint, &cov, b);
                }
                _ => {
                    flush(&mut paint, &mut pending);
                    erase(&mut paint, &cov, b);
                    erase(&mut base, &cov, b);
                }
            }
            continue;
        }
        flush(&mut paint, &mut pending);
        if let Some((layer, _, b)) = stroke_layer(s, c, w, h, &mut sources) {
            let mode = MODES.get(s.tip.mode as usize).copied().unwrap_or(BlendMode::Normal);
            pending = Some((layer, b, mode, s.tip.channels, luminance(s.tip.color)));
        }
    }
    flush(&mut paint, &mut pending);
    for (d, s) in base.px.chunks_exact_mut(4).zip(paint.px.chunks_exact(4)) {
        let k = 1.0 - s[3].min(1.0);
        for ch in 0..4 {
            d[ch] = s[ch] + d[ch] * k;
        }
    }
    *img = base;
}

pub struct Preset {
    pub name: &'static str,
    pub diameter: f64,
    pub hardness: f64,
}

pub const PRESETS: [Preset; 18] = [
    Preset { name: "1 px Hard", diameter: 1.0, hardness: 100.0 },
    Preset { name: "3 px Hard", diameter: 3.0, hardness: 100.0 },
    Preset { name: "5 px Hard", diameter: 5.0, hardness: 100.0 },
    Preset { name: "9 px Hard", diameter: 9.0, hardness: 100.0 },
    Preset { name: "13 px Hard", diameter: 13.0, hardness: 100.0 },
    Preset { name: "19 px Hard", diameter: 19.0, hardness: 100.0 },
    Preset { name: "5 px Soft", diameter: 5.0, hardness: 0.0 },
    Preset { name: "9 px Soft", diameter: 9.0, hardness: 0.0 },
    Preset { name: "13 px Soft", diameter: 13.0, hardness: 0.0 },
    Preset { name: "17 px Soft", diameter: 17.0, hardness: 0.0 },
    Preset { name: "21 px Soft", diameter: 21.0, hardness: 0.0 },
    Preset { name: "27 px Soft", diameter: 27.0, hardness: 0.0 },
    Preset { name: "35 px Soft", diameter: 35.0, hardness: 0.0 },
    Preset { name: "45 px Soft", diameter: 45.0, hardness: 0.0 },
    Preset { name: "65 px Soft", diameter: 65.0, hardness: 0.0 },
    Preset { name: "100 px Soft", diameter: 100.0, hardness: 0.0 },
    Preset { name: "200 px Soft", diameter: 200.0, hardness: 0.0 },
    Preset { name: "300 px Soft", diameter: 300.0, hardness: 0.0 },
];
