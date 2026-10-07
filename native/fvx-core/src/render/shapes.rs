use kurbo::{ParamCurve, ParamCurveArclen, PathSeg};

use super::shapeops;
use super::vector::{DrawOp, OpKind, Paint};
use crate::eval::Eval;
use crate::geom::Mat3;
use crate::model::path::BezPath;
use crate::model::{Group, Layer, Node, Value};

struct Ctx<'a, 'b> {
    ev: &'a Eval<'b>,
    layer: &'a Layer,
}

impl Ctx<'_, '_> {
    fn v(&self, g: &Group, id: &str) -> Option<Value> {
        g.get(id).map(|p| self.ev.value(self.layer, p))
    }
    fn f(&self, g: &Group, id: &str, d: f64) -> f64 {
        self.v(g, id).map(|v| v.num()).unwrap_or(d)
    }
    fn v2(&self, g: &Group, id: &str, d: [f64; 2]) -> [f64; 2] {
        self.v(g, id).map(|v| v.vec2()).unwrap_or(d)
    }
    fn e(&self, g: &Group, id: &str) -> u32 {
        self.v(g, id).map(|v| v.index()).unwrap_or(0)
    }
    fn c(&self, g: &Group, id: &str) -> [f32; 4] {
        let c = self.v(g, id).map(|v| v.color()).unwrap_or([1.0; 4]);
        [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32]
    }
}

pub fn group_matrix(ev: &Eval, layer: &Layer, tr: &Group) -> (Mat3, f64) {
    let c = Ctx { ev, layer };
    let a = c.v2(tr, "anchor", [0.0; 2]);
    let p = c.v2(tr, "position", [0.0; 2]);
    let s = c.v2(tr, "scale", [100.0; 2]);
    let r = c.f(tr, "rotation", 0.0);
    let sk = c.f(tr, "skew", 0.0);
    let ska = c.f(tr, "skewAxis", 0.0);
    let o = c.f(tr, "opacity", 100.0) / 100.0;
    let m = Mat3::translate(p[0], p[1]) * Mat3::rotate_deg(r) * Mat3::skew(sk, ska) * Mat3::scale(s[0] / 100.0, s[1] / 100.0) * Mat3::translate(-a[0], -a[1]);
    (m, o)
}

pub fn item_path(ev: &Eval, layer: &Layer, item: &Group) -> Option<BezPath> {
    let c = Ctx { ev, layer };
    Some(match item.id.as_str() {
        "rect" => {
            let s = c.v2(item, "size", [100.0; 2]);
            let p = c.v2(item, "position", [0.0; 2]);
            BezPath::rect(p[0], p[1], s[0], s[1], c.f(item, "roundness", 0.0))
        }
        "ellipse" => {
            let s = c.v2(item, "size", [100.0; 2]);
            let p = c.v2(item, "position", [0.0; 2]);
            BezPath::ellipse(p[0], p[1], s[0], s[1])
        }
        "star" => {
            let p = c.v2(item, "position", [0.0; 2]);
            let poly = c.e(item, "type") == 1;
            BezPath::star(
                p[0],
                p[1],
                c.f(item, "points", 5.0),
                c.f(item, "outerRadius", 50.0),
                c.f(item, "innerRadius", 25.0),
                c.f(item, "rotation", 0.0),
                c.f(item, "outerRoundness", 0.0),
                c.f(item, "innerRoundness", 0.0),
                poly,
            )
        }
        "path" => c.v(item, "path").and_then(|v| v.as_path().cloned())?,
        _ => return None,
    })
}

struct Out {
    paths: Vec<BezPath>,
    ops: Vec<DrawOp>,
}

fn walk(c: &Ctx, contents: &Group, m: Mat3, opacity: f64) -> Out {
    let mut out = Out { paths: vec![], ops: vec![] };
    for node in &contents.children {
        let Node::Group(item) = node else { continue };
        if !item.enabled {
            continue;
        }
        match item.id.as_str() {
            "rect" | "ellipse" | "star" | "path" => {
                if let Some(p) = item_path(c.ev, c.layer, item) {
                    out.paths.push(p.transformed(&m));
                }
            }
            "group" => {
                let (gm, go) = item.sub("transform").map(|t| group_matrix(c.ev, c.layer, t)).unwrap_or((Mat3::IDENTITY, 1.0));
                if let Some(inner) = item.sub("contents") {
                    let sub = walk(c, inner, m * gm, opacity * go);
                    out.paths.extend(sub.paths);
                    out.ops.extend(sub.ops);
                }
            }
            "fill" | "gradientFill" => {
                let op_opacity = (c.f(item, "opacity", 100.0) / 100.0 * opacity) as f32;
                let paint = if item.id == "fill" {
                    Paint::Solid(c.c(item, "color"))
                } else {
                    gradient_paint(c, item)
                };
                out.ops.push(DrawOp { paths: out.paths.clone(), paint, opacity: op_opacity, kind: OpKind::Fill { even_odd: c.e(item, "rule") == 1 }, transform: Mat3::IDENTITY });
            }
            "stroke" | "gradientStroke" => {
                let op_opacity = (c.f(item, "opacity", 100.0) / 100.0 * opacity) as f32;
                let paint = if item.id == "stroke" { Paint::Solid(c.c(item, "color")) } else { gradient_paint(c, item) };
                let scale = m.max_scale();
                let mut dashes = vec![];
                for (dk, gk) in [("dash", "gap"), ("dash2", "gap2"), ("dash3", "gap3")] {
                    let dash = c.f(item, dk, 0.0);
                    let gap = c.f(item, gk, 0.0);
                    if dash > 0.0 || (dk == "dash" && gap > 0.0) {
                        dashes.push(dash.max(0.01) * scale);
                        dashes.push((if gap > 0.0 { gap } else { dash }) * scale);
                    }
                }
                let taper = item.sub("taper").map(|t| shapeops::Taper {
                    percent: c.e(t, "units") == 1,
                    start_len: c.f(t, "startLength", 0.0) * if c.e(t, "units") == 1 { 1.0 } else { scale },
                    end_len: c.f(t, "endLength", 0.0) * if c.e(t, "units") == 1 { 1.0 } else { scale },
                    start_w: c.f(t, "startWidth", 100.0),
                    end_w: c.f(t, "endWidth", 100.0),
                    start_ease: c.f(t, "startEase", 0.0),
                    end_ease: c.f(t, "endEase", 0.0),
                });
                let wave = item.sub("wave").map(|w| shapeops::WaveSpec {
                    amount: c.f(w, "amount", 0.0),
                    cycles_units: c.e(w, "units") == 1,
                    wavelength: c.f(w, "wavelength", 20.0) * scale,
                    cycles: c.f(w, "cycles", 10.0),
                    phase: c.f(w, "phase", 0.0),
                });
                let tapering = taper.as_ref().is_some_and(|t| (t.start_len > 0.0 && t.start_w < 100.0) || (t.end_len > 0.0 && t.end_w < 100.0)) || wave.as_ref().is_some_and(|w| w.amount > 0.0);
                if tapering {
                    let t = taper.unwrap_or(shapeops::Taper { percent: false, start_len: 0.0, end_len: 0.0, start_w: 100.0, end_w: 100.0, start_ease: 0.0, end_ease: 0.0 });
                    let w = wave.unwrap_or(shapeops::WaveSpec { amount: 0.0, cycles_units: false, wavelength: 20.0, cycles: 10.0, phase: 0.0 });
                    let width = c.f(item, "width", 2.0) * scale;
                    let polys: Vec<BezPath> = out.paths.iter().flat_map(|p| shapeops::tapered(p, width, c.e(item, "cap"), &t, &w)).collect();
                    out.ops.push(DrawOp { paths: polys, paint, opacity: op_opacity, kind: OpKind::Fill { even_odd: false }, transform: Mat3::IDENTITY });
                    continue;
                }
                out.ops.push(DrawOp {
                    paths: out.paths.clone(),
                    paint,
                    opacity: op_opacity,
                    kind: OpKind::Stroke { width: c.f(item, "width", 2.0) * scale, cap: c.e(item, "cap"), join: c.e(item, "join"), miter: c.f(item, "miter", 4.0), dash: dashes, dash_offset: c.f(item, "dashOffset", 0.0) * scale },
                    transform: Mat3::IDENTITY,
                });
            }
            "trim" => {
                let s = c.f(item, "start", 0.0) / 100.0;
                let e = c.f(item, "end", 100.0) / 100.0;
                let o = c.f(item, "offset", 0.0) / 360.0;
                let individually = c.e(item, "mode") == 1;
                out.paths = trim_paths(&out.paths, s, e, o, individually);
                for op in out.ops.iter_mut() {
                    op.paths = trim_paths(&op.paths, s, e, o, individually);
                }
            }
            "roundCorners" => {
                let r = c.f(item, "radius", 0.0) * m.max_scale();
                if r > 0.0 {
                    out.paths = out.paths.iter().map(|p| round_corners(p, r)).collect();
                    for op in out.ops.iter_mut() {
                        op.paths = op.paths.iter().map(|p| round_corners(p, r)).collect();
                    }
                }
            }
            "mergePaths" => {
                out.paths = shapeops::merge(&out.paths, c.e(item, "mode"));
                out.ops.clear();
            }
            "offsetPaths" => {
                let k = m.max_scale();
                let (a, j, mi, n, co) = (c.f(item, "amount", 10.0) * k, c.e(item, "join"), c.f(item, "miter", 4.0), c.f(item, "copies", 1.0), c.f(item, "copyOffset", 0.0));
                operate(&mut out, |ps| shapeops::offset(ps, a, j, mi, n, co));
            }
            "puckerBloat" => {
                let a = c.f(item, "amount", 0.0);
                operate(&mut out, |ps| shapeops::pucker_bloat(ps, a));
            }
            "twist" => {
                let a = c.f(item, "angle", 0.0);
                let ctr = c.v2(item, "center", [0.0; 2]);
                let ctr = m.apply(ctr[0], ctr[1]);
                operate(&mut out, |ps| shapeops::twist(ps, a, ctr));
            }
            "zigZag" => {
                let (s, r, sm) = (c.f(item, "size", 10.0) * m.max_scale(), c.f(item, "ridges", 5.0), c.e(item, "points") == 1);
                operate(&mut out, |ps| shapeops::zigzag(ps, s, r, sm));
            }
            "wiggle" => {
                let w = shapeops::Wiggle {
                    size: c.f(item, "size", 10.0) * m.max_scale(),
                    detail: c.f(item, "detail", 10.0),
                    smooth: c.e(item, "points") != 0 || item.get("points").is_none(),
                    wps: c.f(item, "wigglesPerSecond", 2.0),
                    correlation: c.f(item, "correlation", 50.0),
                    temporal: c.f(item, "temporalPhase", 0.0),
                    spatial: c.f(item, "spatialPhase", 0.0),
                    seed: c.f(item, "seed", 0.0) + item.uid as f64,
                };
                let t = c.layer.layer_time(c.ev.t);
                operate(&mut out, |ps| shapeops::wiggle_paths(ps, &w, t));
            }
            "wiggleTransform" => {
                let tr = item.sub("transform");
                let g2 = |id: &str| tr.map(|t| c.v2(t, id, [0.0; 2])).unwrap_or([0.0; 2]);
                let w = shapeops::WiggleXf {
                    wps: c.f(item, "wigglesPerSecond", 2.0),
                    correlation: c.f(item, "correlation", 50.0),
                    temporal: c.f(item, "temporalPhase", 0.0),
                    spatial: c.f(item, "spatialPhase", 0.0),
                    seed: c.f(item, "seed", 0.0) + item.uid as f64,
                    anchor: g2("anchor"),
                    position: g2("position"),
                    scale: g2("scale"),
                    rotation: tr.map(|t| c.f(t, "rotation", 0.0)).unwrap_or(0.0),
                };
                let t = c.layer.layer_time(c.ev.t);
                let inv = m.inverse().unwrap_or(Mat3::IDENTITY);
                let xf = |i: usize| m * shapeops::wiggle_matrix(&w, t, i) * inv;
                out.paths = out.paths.iter().enumerate().map(|(i, p)| p.transformed(&xf(i))).collect();
                for op in out.ops.iter_mut() {
                    op.paths = op.paths.iter().enumerate().map(|(i, p)| p.transformed(&xf(i))).collect();
                }
            }
            "repeater" => {
                let copies = c.f(item, "copies", 3.0).max(0.0);
                let offset = c.f(item, "offset", 0.0);
                let above = c.e(item, "composite") == 1;
                let tr = item.sub("transform");
                let (a, p, s, r, so, eo) = match tr {
                    Some(t) => (
                        c.v2(t, "anchor", [0.0; 2]),
                        c.v2(t, "position", [100.0, 0.0]),
                        c.v2(t, "scale", [100.0; 2]),
                        c.f(t, "rotation", 0.0),
                        c.f(t, "startOpacity", 100.0) / 100.0,
                        c.f(t, "endOpacity", 100.0) / 100.0,
                    ),
                    None => ([0.0; 2], [100.0, 0.0], [100.0; 2], 0.0, 1.0, 1.0),
                };
                let n = copies.ceil() as usize;
                let inv = m.inverse().unwrap_or(Mat3::IDENTITY);
                let step = |k: f64| -> Mat3 {
                    let sx = (s[0] / 100.0).powf(k);
                    let sy = (s[1] / 100.0).powf(k);
                    Mat3::translate(p[0] * k, p[1] * k) * Mat3::translate(a[0], a[1]) * Mat3::rotate_deg(r * k) * Mat3::scale(sx, sy) * Mat3::translate(-a[0], -a[1])
                };
                let base_paths = out.paths.clone();
                let base_ops = std::mem::take(&mut out.ops);
                let mut paths = vec![];
                let mut ops = vec![];
                for i in 0..n {
                    let k = i as f64 + offset;
                    let frac = if copies > 1.0 { i as f64 / (copies - 1.0).max(1.0) } else { 0.0 };
                    let mut op_o = (so + (eo - so) * frac) as f32;
                    if i + 1 == n && copies.fract() > 0.0 {
                        op_o *= copies.fract() as f32;
                    }
                    let tm = m * step(k) * inv;
                    let copy_ops: Vec<DrawOp> = base_ops
                        .iter()
                        .map(|o| DrawOp { paths: o.paths.iter().map(|q| q.transformed(&tm)).collect(), opacity: o.opacity * op_o, ..o.clone() })
                        .collect();
                    paths.extend(base_paths.iter().map(|q| q.transformed(&tm)));
                    if above {
                        ops.splice(0..0, copy_ops);
                    } else {
                        ops.extend(copy_ops);
                    }
                }
                out.paths = paths;
                out.ops = ops;
            }
            _ => {}
        }
    }
    out
}

fn operate(out: &mut Out, f: impl Fn(&[BezPath]) -> Vec<BezPath>) {
    out.paths = f(&out.paths);
    for op in out.ops.iter_mut() {
        op.paths = f(&op.paths);
    }
}

fn gradient_paint(c: &Ctx, item: &Group) -> Paint {
    let grad = c.v(item, "colors").and_then(|v| v.as_gradient().cloned()).unwrap_or_default();
    let start = c.v2(item, "start", [0.0; 2]);
    let end = c.v2(item, "end", [100.0, 0.0]);
    if c.e(item, "type") == 1 {
        Paint::Radial { center: start, edge: end, grad }
    } else {
        Paint::Linear { start, end, grad }
    }
}

pub fn shape_ops(ev: &Eval, layer: &Layer) -> Vec<DrawOp> {
    let Some(contents) = layer.contents() else { return vec![] };
    let c = Ctx { ev, layer };
    let out = walk(&c, contents, Mat3::IDENTITY, 1.0);
    let mut ops = out.ops;
    ops.reverse();
    ops
}

pub fn shape_paths(ev: &Eval, layer: &Layer) -> Vec<(u64, BezPath)> {
    let mut out = vec![];
    if let Some(contents) = layer.contents() {
        collect_paths(ev, layer, contents, Mat3::IDENTITY, &mut out);
    }
    out
}

fn collect_paths(ev: &Eval, layer: &Layer, contents: &Group, m: Mat3, out: &mut Vec<(u64, BezPath)>) {
    for node in &contents.children {
        let Node::Group(item) = node else { continue };
        match item.id.as_str() {
            "path" => {
                if let Some(p) = item_path(ev, layer, item) {
                    out.push((item.uid, p.transformed(&m)));
                }
            }
            "group" => {
                let (gm, _) = item.sub("transform").map(|t| group_matrix(ev, layer, t)).unwrap_or((Mat3::IDENTITY, 1.0));
                if let Some(inner) = item.sub("contents") {
                    collect_paths(ev, layer, inner, m * gm, out);
                }
            }
            _ => {}
        }
    }
}

fn segs(p: &BezPath) -> Vec<PathSeg> {
    let mut v = vec![];
    for i in 0..p.segments() {
        let s = p.segment(i);
        let pt = |a: [f64; 2]| kurbo::Point::new(a[0], a[1]);
        if s[1] == s[0] && s[2] == s[3] {
            v.push(PathSeg::Line(kurbo::Line::new(pt(s[0]), pt(s[3]))));
        } else {
            v.push(PathSeg::Cubic(kurbo::CubicBez::new(pt(s[0]), pt(s[1]), pt(s[2]), pt(s[3]))));
        }
    }
    v
}

fn extract(segs: &[PathSeg], lens: &[f64], a: f64, b: f64) -> Option<BezPath> {
    if b - a <= 1e-9 {
        return None;
    }
    let mut out = BezPath::default();
    let mut acc = 0.0;
    for (s, &l) in segs.iter().zip(lens) {
        let s0 = acc;
        let s1 = acc + l;
        acc = s1;
        if s1 <= a || s0 >= b || l <= 1e-12 {
            continue;
        }
        let t0 = if a > s0 { s.inv_arclen(a - s0, 1e-4) } else { 0.0 };
        let t1 = if b < s1 { s.inv_arclen(b - s0, 1e-4) } else { 1.0 };
        let sub = s.subsegment(t0..t1);
        let (p0, c1, c2, p3) = match sub {
            PathSeg::Line(l) => (l.p0, l.p0, l.p1, l.p1),
            PathSeg::Cubic(c) => (c.p0, c.p1, c.p2, c.p3),
            PathSeg::Quad(q) => {
                let c = q.raise();
                (c.p0, c.p1, c.p2, c.p3)
            }
        };
        if out.vertices.is_empty() {
            out.vertices.push([p0.x, p0.y]);
            out.ins.push([0.0; 2]);
            out.outs.push([0.0; 2]);
        }
        let last = out.outs.len() - 1;
        out.outs[last] = [c1.x - p0.x, c1.y - p0.y];
        out.vertices.push([p3.x, p3.y]);
        out.ins.push([c2.x - p3.x, c2.y - p3.y]);
        out.outs.push([0.0; 2]);
    }
    if out.vertices.len() < 2 { None } else { Some(out) }
}

fn trim_one(p: &BezPath, s: f64, e: f64) -> Vec<BezPath> {
    let sg = segs(p);
    let lens: Vec<f64> = sg.iter().map(|x| x.arclen(1e-3)).collect();
    let total: f64 = lens.iter().sum();
    if total <= 1e-9 {
        return vec![];
    }
    let mut out = vec![];
    let (a, b) = (s * total, e * total);
    if b <= total + 1e-9 {
        out.extend(extract(&sg, &lens, a.max(0.0), b.min(total)));
    } else {
        out.extend(extract(&sg, &lens, a, total));
        let wrap = extract(&sg, &lens, 0.0, b - total);
        if p.closed && !out.is_empty() {
            if let Some(w) = wrap {
                let first = out.remove(0);
                let mut joined = first.clone();
                for i in 1..w.vertices.len() {
                    joined.vertices.push(w.vertices[i]);
                    joined.ins.push(w.ins[i]);
                    joined.outs.push(w.outs[i]);
                }
                let li = first.outs.len() - 1;
                joined.outs[li] = w.outs[0];
                out.insert(0, joined);
            }
        } else {
            out.extend(wrap);
        }
    }
    out
}

pub fn trim_paths(paths: &[BezPath], start: f64, end: f64, offset: f64, individually: bool) -> Vec<BezPath> {
    let (mut s, mut e) = (start.clamp(0.0, 1.0), end.clamp(0.0, 1.0));
    if s > e {
        std::mem::swap(&mut s, &mut e);
    }
    if s <= 1e-9 && e >= 1.0 - 1e-9 {
        return paths.to_vec();
    }
    let off = offset.rem_euclid(1.0);
    let (s, e) = (s + off, e + off);
    let norm = |s: f64, e: f64| if s >= 1.0 { (s - 1.0, e - 1.0) } else { (s, e) };
    if !individually {
        return paths
            .iter()
            .flat_map(|p| {
                let (a, b) = norm(s, e);
                trim_one(p, a, b)
            })
            .collect();
    }
    let lens: Vec<f64> = paths.iter().map(|p| segs(p).iter().map(|x| x.arclen(1e-3)).sum::<f64>()).collect();
    let total: f64 = lens.iter().sum();
    if total <= 1e-9 {
        return vec![];
    }
    let mut out = vec![];
    let (a, b) = norm(s, e);
    let ranges = if b > 1.0 { vec![(a, 1.0), (0.0, b - 1.0)] } else { vec![(a, b)] };
    let mut acc = 0.0;
    for (p, l) in paths.iter().zip(&lens) {
        let p0 = acc / total;
        let p1 = (acc + l) / total;
        acc += l;
        for (ra, rb) in &ranges {
            let lo = ra.max(p0);
            let hi = rb.min(p1);
            if hi > lo && p1 > p0 {
                out.extend(trim_one(p, (lo - p0) / (p1 - p0), (hi - p0) / (p1 - p0)));
            }
        }
    }
    out
}

fn round_corners(p: &BezPath, r: f64) -> BezPath {
    let n = p.vertices.len();
    if n < 3 {
        return p.clone();
    }
    let mut out = BezPath { vertices: vec![], ins: vec![], outs: vec![], closed: p.closed };
    for i in 0..n {
        let v = p.vertices[i];
        let sharp = p.tangent_in(i) == [0.0; 2] && p.tangent_out(i) == [0.0; 2];
        let end = !p.closed && (i == 0 || i + 1 == n);
        if !sharp || end {
            out.vertices.push(v);
            out.ins.push(p.tangent_in(i));
            out.outs.push(p.tangent_out(i));
            continue;
        }
        let prev = p.vertices[(i + n - 1) % n];
        let next = p.vertices[(i + 1) % n];
        let d1 = [prev[0] - v[0], prev[1] - v[1]];
        let d2 = [next[0] - v[0], next[1] - v[1]];
        let l1 = d1[0].hypot(d1[1]);
        let l2 = d2[0].hypot(d2[1]);
        let rr = r.min(l1 / 2.0).min(l2 / 2.0);
        if rr <= 1e-6 {
            out.vertices.push(v);
            out.ins.push([0.0; 2]);
            out.outs.push([0.0; 2]);
            continue;
        }
        let a = [v[0] + d1[0] / l1 * rr, v[1] + d1[1] / l1 * rr];
        let b = [v[0] + d2[0] / l2 * rr, v[1] + d2[1] / l2 * rr];
        let k = 0.552_284_75;
        out.vertices.push(a);
        out.ins.push([0.0; 2]);
        out.outs.push([(v[0] - a[0]) * k, (v[1] - a[1]) * k]);
        out.vertices.push(b);
        out.ins.push([(v[0] - b[0]) * k, (v[1] - b[1]) * k]);
        out.outs.push([0.0; 2]);
    }
    out
}
