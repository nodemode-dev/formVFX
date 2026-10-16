use std::f64::consts::FRAC_PI_2;

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;

use super::mesh::{MatEval, Tri};
use super::vector::{DrawOp, OpKind, Paint};
use super::textanim::{Tint, tint};
use super::{shapes, text, textanim};
use crate::eval::Eval;
use crate::model::{Layer, LayerKind};

type P2 = [f64; 2];

struct Geo {
    style: u32,
    bevel: f64,
    hole: f64,
    depth: f64,
    side: Option<[f64; 4]>,
    bevel_col: Option<[f64; 4]>,
}

fn geo(ev: &Eval, l: &Layer) -> Option<Geo> {
    let g = l.props.sub("geometryOptions")?;
    let num = |id: &str, d: f64| g.get(id).map(|p| ev.value(l, p).num()).unwrap_or(d);
    let style = g.get("bevelStyle").map(|p| ev.value(l, p).index()).unwrap_or(0);
    let depth = num("extrusionDepth", 0.0).max(0.0);
    let bevel = if style == 0 { 0.0 } else { num("bevelDepth", 0.0).max(0.0) };
    if depth <= 0.0 && bevel <= 0.0 {
        return None;
    }
    let fill = g.get("sidesUseFill").map(|p| ev.value(l, p).boolean()).unwrap_or(true);
    let side = (!fill).then(|| g.get("sideColor").map(|p| ev.value(l, p).color()).unwrap_or([0.5, 0.5, 0.5, 1.0]));
    let front = g.get("bevelUsesFront").map(|p| ev.value(l, p).boolean()).unwrap_or(true);
    let bevel_col = (!front).then(|| g.get("bevelColor").map(|p| ev.value(l, p).color()).unwrap_or([0.5, 0.5, 0.5, 1.0]));
    Some(Geo { style, bevel, hole: num("holeBevelDepth", 100.0).clamp(0.0, 100.0) / 100.0, depth, side, bevel_col })
}

pub fn thickness(ev: &Eval, l: &Layer) -> Option<f64> {
    active(ev, l).then(|| geo(ev, l).map(|g| g.depth.max(g.bevel))).flatten()
}

pub fn active(ev: &Eval, l: &Layer) -> bool {
    matches!(l.kind, LayerKind::Text | LayerKind::Shape) && l.is_3d() && ev.comp.renderer == "advanced3d" && geo(ev, l).is_some()
}

fn plain(ops: Vec<DrawOp>) -> Vec<(DrawOp, [Tint; 3])> {
    ops.into_iter().map(|o| (o, [None; 3])).collect()
}

fn layer_ops(ev: &Eval, l: &Layer) -> Vec<(DrawOp, [Tint; 3])> {
    match l.kind {
        LayerKind::Shape => plain(shapes::shape_ops(ev, l)),
        LayerKind::Text => {
            let Some(doc) = l.text_prop().map(|p| ev.value(l, p)).and_then(|v| v.as_text().cloned()) else { return vec![] };
            if !textanim::has_animation(l) {
                return plain(text::text_ops(&doc));
            }
            let r = textanim::render_text(ev, l, &doc);
            match r.parts {
                Some(parts) => parts.into_iter().flat_map(|(ops, t)| ops.into_iter().map(move |o| (o, t))).collect(),
                None => plain(r.ops),
            }
        }
        _ => vec![],
    }
}

fn paint_color(p: &Paint) -> [f64; 4] {
    match p {
        Paint::Solid(c) => [c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64],
        Paint::Linear { grad, .. } | Paint::Radial { grad, .. } => grad.stops.first().map(|s| s.color).unwrap_or([1.0; 4]),
    }
}

fn area(c: &[P2]) -> f64 {
    let mut a = 0.0;
    for i in 0..c.len() {
        let (p, q) = (c[i], c[(i + 1) % c.len()]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    a * 0.5
}

fn cross(o: P2, a: P2, b: P2) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

fn inside(p: P2, a: P2, b: P2, c: P2) -> bool {
    cross(a, b, p) > 1e-12 && cross(b, c, p) > 1e-12 && cross(c, a, p) > 1e-12
}

fn seg_cross(a: P2, b: P2, c: P2, d: P2) -> bool {
    let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
    ((d1 > 1e-12 && d2 < -1e-12) || (d1 < -1e-12 && d2 > 1e-12)) && ((d3 > 1e-12 && d4 < -1e-12) || (d3 < -1e-12 && d4 > 1e-12))
}

fn bridge(poly: &mut Vec<usize>, pts: &[P2], hole: &[usize]) {
    let hi = (0..hole.len()).max_by(|a, b| pts[hole[*a]][0].total_cmp(&pts[hole[*b]][0])).unwrap_or(0);
    let h = pts[hole[hi]];
    let edges: Vec<(P2, P2)> = (0..poly.len()).map(|i| (pts[poly[i]], pts[poly[(i + 1) % poly.len()]])).chain((0..hole.len()).map(|i| (pts[hole[i]], pts[hole[(i + 1) % hole.len()]]))).collect();
    let mut best: Option<(usize, f64)> = None;
    for (i, &v) in poly.iter().enumerate() {
        let p = pts[v];
        let d = (p[0] - h[0]).powi(2) + (p[1] - h[1]).powi(2);
        if best.is_some_and(|b| b.1 <= d) {
            continue;
        }
        if edges.iter().all(|(a, b)| !seg_cross(h, p, *a, *b)) {
            best = Some((i, d));
        }
    }
    let vi = best.map(|b| b.0).unwrap_or(0);
    let mut spliced = Vec::with_capacity(poly.len() + hole.len() + 2);
    spliced.extend_from_slice(&poly[..=vi]);
    spliced.extend(hole[hi..].iter().copied());
    spliced.extend(hole[..=hi].iter().copied());
    spliced.push(poly[vi]);
    spliced.extend_from_slice(&poly[vi + 1..]);
    *poly = spliced;
}

pub fn triangulate(outer: &[P2], holes: &[Vec<P2>]) -> Vec<[P2; 3]> {
    let mut pts: Vec<P2> = outer.to_vec();
    let mut poly: Vec<usize> = (0..outer.len()).collect();
    if area(outer) < 0.0 {
        poly.reverse();
    }
    let mut hs: Vec<Vec<usize>> = vec![];
    for h in holes {
        if h.len() < 3 {
            continue;
        }
        let base = pts.len();
        pts.extend_from_slice(h);
        let mut idx: Vec<usize> = (base..base + h.len()).collect();
        if area(h) > 0.0 {
            idx.reverse();
        }
        hs.push(idx);
    }
    hs.sort_by(|a, b| {
        let mx = |v: &Vec<usize>| v.iter().map(|i| pts[*i][0]).fold(f64::MIN, f64::max);
        mx(b).total_cmp(&mx(a))
    });
    for h in &hs {
        bridge(&mut poly, &pts, h);
    }
    let mut out = vec![];
    let mut guard = 0;
    while poly.len() > 3 && guard < 100_000 {
        guard += 1;
        let n = poly.len();
        let mut clipped = false;
        for i in 0..n {
            let (a, b, c) = (poly[(i + n - 1) % n], poly[i], poly[(i + 1) % n]);
            let (pa, pb, pc) = (pts[a], pts[b], pts[c]);
            if cross(pa, pb, pc) <= 1e-12 {
                continue;
            }
            let blocked = poly.iter().any(|&j| {
                let q = pts[j];
                j != a && j != b && j != c && q != pa && q != pb && q != pc && inside(q, pa, pb, pc)
            });
            if blocked {
                continue;
            }
            out.push([pa, pb, pc]);
            poly.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            let i = (0..n).max_by(|x, y| {
                let k = |i: usize| cross(pts[poly[(i + n - 1) % n]], pts[poly[i]], pts[poly[(i + 1) % n]]);
                k(*x).total_cmp(&k(*y))
            });
            let i = i.unwrap_or(0);
            let (a, b, c) = (poly[(i + n - 1) % n], poly[i], poly[(i + 1) % n]);
            if cross(pts[a], pts[b], pts[c]) > 1e-12 {
                out.push([pts[a], pts[b], pts[c]]);
            }
            poly.remove(i);
        }
    }
    if poly.len() == 3 && cross(pts[poly[0]], pts[poly[1]], pts[poly[2]]) > 1e-12 {
        out.push([pts[poly[0]], pts[poly[1]], pts[poly[2]]]);
    }
    out
}

fn cap(outer: &[P2], holes: &[Vec<P2>]) -> Vec<[P2; 3]> {
    let mut paths: Vec<Vec<P2>> = vec![outer.to_vec()];
    paths.extend(holes.iter().cloned());
    let empty: Vec<Vec<P2>> = vec![];
    let shapes = paths.overlay(&empty, OverlayRule::Subject, FillRule::NonZero);
    let mut out = vec![];
    for sh in shapes {
        let Some((o, hs)) = sh.split_first() else { continue };
        if o.len() >= 3 {
            out.extend(triangulate(o, hs));
        }
    }
    out
}

fn norm3(v: [f64; 3]) -> [f32; 3] {
    let k = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
    [(v[0] / k) as f32, (v[1] / k) as f32, (v[2] / k) as f32]
}

fn edge_out(a: P2, b: P2) -> P2 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let k = (dx * dx + dy * dy).sqrt().max(1e-12);
    [dy / k, -dx / k]
}

fn miter(c: &[P2], i: usize) -> (P2, f64) {
    let n = c.len();
    let (p0, p1, p2) = (c[(i + n - 1) % n], c[i], c[(i + 1) % n]);
    let (o1, o2) = (edge_out(p0, p1), edge_out(p1, p2));
    let m = [o1[0] + o2[0], o1[1] + o2[1]];
    let k = (m[0] * m[0] + m[1] * m[1]).sqrt();
    let m = if k < 1e-9 { o1 } else { [m[0] / k, m[1] / k] };
    (m, 1.0 / (m[0] * o1[0] + m[1] * o1[1]).max(0.35))
}

fn inset_cap(all: &[(Vec<P2>, f64)], ci: usize) -> f64 {
    let c = &all[ci].0;
    let n = c.len();
    (0..n)
        .map(|i| {
            let (m, s) = miter(c, i);
            let (p, d) = (c[i], [-m[0], -m[1]]);
            let mut best = f64::INFINITY;
            for (cj, (o, _)) in all.iter().enumerate() {
                let k = o.len();
                for j in 0..k {
                    if cj == ci && (j == i || (j + 1) % k == i) {
                        continue;
                    }
                    let (a, b) = (o[j], o[(j + 1) % k]);
                    let e = [b[0] - a[0], b[1] - a[1]];
                    let den = d[0] * e[1] - d[1] * e[0];
                    if den.abs() < 1e-12 {
                        continue;
                    }
                    let ap = [a[0] - p[0], a[1] - p[1]];
                    let t = (ap[0] * e[1] - ap[1] * e[0]) / den;
                    let u = (ap[0] * d[1] - ap[1] * d[0]) / den;
                    if t > 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u) {
                        best = best.min(t);
                    }
                }
            }
            best * 0.45 / s
        })
        .fold(f64::INFINITY, f64::min)
}

fn ring(c: &[P2], amount: f64) -> Vec<P2> {
    (0..c.len())
        .map(|i| {
            let (m, s) = miter(c, i);
            [c[i][0] - m[0] * s * amount, c[i][1] - m[1] * s * amount]
        })
        .collect()
}

fn vertex_out(c: &[P2], i: usize) -> Option<P2> {
    let n = c.len();
    let (o1, o2) = (edge_out(c[(i + n - 1) % n], c[i]), edge_out(c[i], c[(i + 1) % n]));
    if o1[0] * o2[0] + o1[1] * o2[1] < 0.82 {
        return None;
    }
    let m = [o1[0] + o2[0], o1[1] + o2[1]];
    let k = (m[0] * m[0] + m[1] * m[1]).sqrt().max(1e-12);
    Some([m[0] / k, m[1] / k])
}

fn profile(style: u32, t: f64) -> (f64, f64) {
    match style {
        2 => ((t * FRAC_PI_2).cos(), (t * FRAC_PI_2).sin()),
        3 => (1.0 - (t * FRAC_PI_2).sin(), 1.0 - (t * FRAC_PI_2).cos()),
        _ => (1.0 - t, t),
    }
}

fn push_quad(out: &mut Vec<Tri>, q: [[f64; 3]; 4], n: [[f32; 3]; 4], mat: u16) {
    let uv = [0.0f32; 2];
    out.push(Tri { v: [q[0], q[1], q[2]], n: [n[0], n[1], n[2]], uv: [uv; 3], mat });
    out.push(Tri { v: [q[0], q[2], q[3]], n: [n[0], n[2], n[3]], uv: [uv; 3], mat });
}

fn shape_mesh(out: &mut Vec<Tri>, g: &Geo, outer: &[P2], holes: &[Vec<P2>], [front, bevel_mat, side]: [u16; 3]) {
    let total = g.depth.max(g.bevel);
    let bevel = g.bevel.min(total * 0.5).min(if g.depth > 0.0 { g.depth } else { total });
    let steps = match g.style {
        0 => 0,
        1 => 1,
        _ => 6,
    };
    let mut contours: Vec<(Vec<P2>, f64)> = vec![];
    let mut o = outer.to_vec();
    if area(&o) < 0.0 {
        o.reverse();
    }
    contours.push((o, bevel));
    for h in holes {
        let mut h = h.clone();
        if area(&h) > 0.0 {
            h.reverse();
        }
        contours.push((h, bevel * g.hole));
    }
    let rings: Vec<Vec<(Vec<P2>, f64)>> = contours
        .iter()
        .enumerate()
        .map(|(ci, (c, b))| {
            if steps == 0 || *b <= 0.0 {
                return vec![(c.clone(), 0.0)];
            }
            let width = b.min(inset_cap(&contours, ci));
            (0..=steps)
                .map(|k| {
                    let (inset, z) = profile(g.style, k as f64 / steps as f64);
                    (ring(c, inset * width), z * b)
                })
                .collect()
        })
        .collect();
    let front_outer = &rings[0][0].0;
    let front_holes: Vec<Vec<P2>> = rings[1..].iter().map(|r| r[0].0.clone()).collect();
    for t in cap(front_outer, &front_holes) {
        let v = t.map(|p| [p[0], p[1], 0.0]);
        out.push(Tri { v: [v[0], v[2], v[1]], n: [[0.0, 0.0, -1.0]; 3], uv: [[0.0; 2]; 3], mat: front });
    }
    for (ci, rs) in rings.iter().enumerate() {
        let c = &contours[ci].0;
        let n = c.len();
        for w in rs.windows(2) {
            let ((ra, za), (rb, zb)) = (&w[0], &w[1]);
            for i in 0..n {
                let j = (i + 1) % n;
                let q = [[ra[i][0], ra[i][1], *za], [ra[j][0], ra[j][1], *za], [rb[j][0], rb[j][1], *zb], [rb[i][0], rb[i][1], *zb]];
                let e = [q[1][0] - q[0][0], q[1][1] - q[0][1], q[1][2] - q[0][2]];
                let f = [q[3][0] - q[0][0], q[3][1] - q[0][1], q[3][2] - q[0][2]];
                let mut nn = [e[1] * f[2] - e[2] * f[1], e[2] * f[0] - e[0] * f[2], e[0] * f[1] - e[1] * f[0]];
                let o = edge_out(c[i], c[j]);
                if nn[0] * o[0] + nn[1] * o[1] - nn[2] < 0.0 {
                    nn = [-nn[0], -nn[1], -nn[2]];
                }
                let nf = norm3(nn);
                push_quad(out, q, [nf; 4], bevel_mat);
            }
        }
        let last = &rs[rs.len() - 1];
        if total > last.1 + 1e-9 {
            let (r, z0) = (&last.0, last.1);
            for i in 0..n {
                let j = (i + 1) % n;
                let fo = edge_out(r[i], r[j]);
                let ni = vertex_out(r, i).unwrap_or(fo);
                let nj = vertex_out(r, j).unwrap_or(fo);
                let (a, b) = (norm3([ni[0], ni[1], 0.0]), norm3([nj[0], nj[1], 0.0]));
                push_quad(out, [[r[i][0], r[i][1], z0], [r[j][0], r[j][1], z0], [r[j][0], r[j][1], total], [r[i][0], r[i][1], total]], [a, b, b, a], side);
            }
        }
    }
    if total > 0.0 {
        let back_outer = &rings[0][rings[0].len() - 1].0;
        let back_holes: Vec<Vec<P2>> = rings[1..].iter().map(|r| r[r.len() - 1].0.clone()).collect();
        for t in cap(back_outer, &back_holes) {
            let v = t.map(|p| [p[0], p[1], total]);
            out.push(Tri { v, n: [[0.0, 0.0, 1.0]; 3], uv: [[0.0; 2]; 3], mat: front });
        }
    }
}

pub fn tris(ev: &Eval, l: &Layer) -> Option<(Vec<Tri>, Vec<MatEval>)> {
    let g = geo(ev, l)?;
    let ops = layer_ops(ev, l);
    let mut mats: Vec<MatEval> = vec![];
    let mut out = vec![];
    let mat = |c: [f64; 4], op: f32| MatEval { base: [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32 * op], texture: None, metallic: 0.0, roughness: 0.55, emissive: [0.0; 3], uv_scale: [1.0; 2] };
    for (op, t) in ops.iter() {
        let OpKind::Fill { even_odd } = op.kind else { continue };
        let front_c = tint(paint_color(&op.paint), t[0]);
        let bevel_c = tint(g.bevel_col.unwrap_or(front_c), t[1]);
        let side_c = tint(g.side.unwrap_or(front_c), t[2]);
        let front = mats.len() as u16;
        let ids = [front, front + 1, front + 2];
        for c in [front_c, bevel_c, side_c] {
            mats.push(mat(c, op.opacity));
        }
        let paths: Vec<_> = op.paths.iter().map(|p| p.transformed(&op.transform)).collect();
        let cs = super::shapeops::contours(&paths, 0.35);
        if cs.is_empty() {
            continue;
        }
        let empty: Vec<Vec<P2>> = vec![];
        let shapes = cs.overlay(&empty, OverlayRule::Subject, if even_odd { FillRule::EvenOdd } else { FillRule::NonZero });
        for sh in shapes {
            let Some((outer, holes)) = sh.split_first() else { continue };
            if outer.len() < 3 {
                continue;
            }
            shape_mesh(&mut out, &g, outer, holes, ids);
        }
    }
    for t in out.iter_mut() {
        t.uv = t.v.map(|v| [(v[0] / 512.0) as f32, ((v[1] + v[2]) / 512.0) as f32]);
    }
    (!out.is_empty()).then_some((out, mats))
}
