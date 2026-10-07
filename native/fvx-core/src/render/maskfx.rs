use rayon::prelude::*;

use crate::geom::Mat3;
use crate::model::path::BezPath;

pub const FALLOFFS: [&str; 2] = ["Smooth", "Linear"];
pub const MOTION_BLUR: [&str; 3] = ["Same as Layer", "On", "Off"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatherPoint {
    pub segment: usize,
    pub t: f64,
    pub radius: f64,
    pub tension: f64,
}

impl FeatherPoint {
    pub fn u(&self) -> f64 {
        self.segment as f64 + self.t
    }
}

pub fn parse(s: &str) -> Vec<FeatherPoint> {
    let Ok(v) = serde_json::from_str::<Vec<[f64; 4]>>(s) else { return vec![] };
    v.into_iter().map(|a| FeatherPoint { segment: a[0].max(0.0) as usize, t: a[1].clamp(0.0, 1.0), radius: a[2], tension: a[3].clamp(0.0, 1.0) }).collect()
}

pub fn encode(pts: &[FeatherPoint]) -> String {
    let v: Vec<[f64; 4]> = pts.iter().map(|p| [p.segment as f64, p.t, p.radius, p.tension]).collect();
    serde_json::to_string(&v).unwrap_or_else(|_| "[]".into())
}

pub fn sorted(mut pts: Vec<FeatherPoint>) -> Vec<FeatherPoint> {
    pts.sort_by(|a, b| a.u().total_cmp(&b.u()));
    pts
}

fn radius_at(pts: &[FeatherPoint], u: f64, span: f64, closed: bool) -> f64 {
    match pts.len() {
        0 => 0.0,
        1 => pts[0].radius,
        n => {
            let after = pts.iter().position(|p| p.u() > u);
            let (a, b, ua, ub) = match after {
                Some(0) if closed => (pts[n - 1], pts[0], pts[n - 1].u() - span, pts[0].u()),
                Some(0) => return pts[0].radius,
                Some(i) => (pts[i - 1], pts[i], pts[i - 1].u(), pts[i].u()),
                None if closed => (pts[n - 1], pts[0], pts[n - 1].u(), pts[0].u() + span),
                None => return pts[n - 1].radius,
            };
            let x = ((u - ua) / (ub - ua).max(1e-9)).clamp(0.0, 1.0);
            let ten = (a.tension + b.tension) * 0.5;
            let s = x * x * (3.0 - 2.0 * x);
            let x = x + (s - x) * ten;
            a.radius + (b.radius - a.radius) * x
        }
    }
}

pub fn apply(cov: &mut [f32], path: &BezPath, pts: &[FeatherPoint], linear: bool, m: &Mat3, w: usize, h: usize, x0: i64, y0: i64, k: f64) {
    let segs = path.segments();
    if pts.is_empty() || segs == 0 {
        return;
    }
    let span = segs as f64;
    let rmax = pts.iter().map(|p| p.radius.abs()).fold(0.0f64, f64::max) * k;
    if rmax < 0.5 {
        return;
    }
    let per = 24usize;
    let mut poly: Vec<([f64; 2], f64)> = Vec::with_capacity(segs * per + 1);
    for i in 0..segs {
        for j in 0..per {
            let t = j as f64 / per as f64;
            let q = path.point_on_segment(i, t);
            let b = m.apply(q[0], q[1]);
            poly.push(([b[0] - x0 as f64, b[1] - y0 as f64], i as f64 + t));
        }
    }
    let end = path.point_on_segment(segs - 1, 1.0);
    let eb = m.apply(end[0], end[1]);
    poly.push(([eb[0] - x0 as f64, eb[1] - y0 as f64], span));
    let (mut bx0, mut by0, mut bx1, mut by1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (p, _) in &poly {
        bx0 = bx0.min(p[0]);
        by0 = by0.min(p[1]);
        bx1 = bx1.max(p[0]);
        by1 = by1.max(p[1]);
    }
    let (bx0, by0, bx1, by1) = (bx0 - rmax - 1.0, by0 - rmax - 1.0, bx1 + rmax + 1.0, by1 + rmax + 1.0);
    let cell = rmax.max(4.0);
    let gw = ((bx1 - bx0) / cell).ceil().max(1.0) as usize;
    let gh = ((by1 - by0) / cell).ceil().max(1.0) as usize;
    let mut grid: Vec<Vec<usize>> = vec![vec![]; gw * gh];
    for s in 0..poly.len() - 1 {
        let (a, b) = (poly[s].0, poly[s + 1].0);
        let cx0 = (((a[0].min(b[0]) - bx0) / cell).floor().max(0.0) as usize).min(gw - 1);
        let cx1 = (((a[0].max(b[0]) - bx0) / cell).floor().max(0.0) as usize).min(gw - 1);
        let cy0 = (((a[1].min(b[1]) - by0) / cell).floor().max(0.0) as usize).min(gh - 1);
        let cy1 = (((a[1].max(b[1]) - by0) / cell).floor().max(0.0) as usize).min(gh - 1);
        for gy in cy0..=cy1 {
            for gx in cx0..=cx1 {
                grid[gy * gw + gx].push(s);
            }
        }
    }
    let rad: Vec<f64> = pts.iter().map(|p| p.radius * k).collect();
    let scaled: Vec<FeatherPoint> = pts.iter().zip(&rad).map(|(p, r)| FeatherPoint { radius: *r, ..*p }).collect();
    let falloff = |x: f64| -> f32 {
        let x = x.clamp(0.0, 1.0);
        (if linear { x } else { x * x * (3.0 - 2.0 * x) }) as f32
    };
    let ys = (by0.floor().max(0.0) as usize)..(by1.ceil().max(0.0) as usize).min(h);
    let xs = (bx0.floor().max(0.0) as usize, (bx1.ceil().max(0.0) as usize).min(w));
    let rows: Vec<(usize, Vec<(usize, f32)>)> = ys
        .into_par_iter()
        .map(|y| {
            let mut out = vec![];
            for x in xs.0..xs.1 {
                let p = [x as f64 + 0.5, y as f64 + 0.5];
                let gx = (((p[0] - bx0) / cell).floor().max(0.0) as usize).min(gw - 1);
                let gy = (((p[1] - by0) / cell).floor().max(0.0) as usize).min(gh - 1);
                let mut best = (f64::MAX, 0.0);
                for ny in gy.saturating_sub(1)..=(gy + 1).min(gh - 1) {
                    for nx in gx.saturating_sub(1)..=(gx + 1).min(gw - 1) {
                        for &s in &grid[ny * gw + nx] {
                            let (a, ua) = poly[s];
                            let (b, ub) = poly[s + 1];
                            let d = [b[0] - a[0], b[1] - a[1]];
                            let l2 = d[0] * d[0] + d[1] * d[1];
                            let t = if l2 > 1e-12 { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
                            let q = [a[0] + d[0] * t, a[1] + d[1] * t];
                            let dist = (p[0] - q[0]).hypot(p[1] - q[1]);
                            if dist < best.0 {
                                best = (dist, ua + (ub - ua) * t);
                            }
                        }
                    }
                }
                if best.0 > rmax {
                    continue;
                }
                let r = radius_at(&scaled, best.1, span, path.closed);
                let i = y * w + x;
                let c = cov[i];
                if c < 0.5 && r > 0.5 && best.0 < r {
                    out.push((x, c.max(falloff(1.0 - best.0 / r))));
                } else if c >= 0.5 && r < -0.5 && best.0 < -r {
                    out.push((x, c.min(falloff(best.0 / -r))));
                }
            }
            (y, out)
        })
        .collect();
    for (y, row) in rows {
        for (x, v) in row {
            cov[y * w + x] = v;
        }
    }
}

fn resample(p: &BezPath, n: usize) -> BezPath {
    let mut out = p.clone();
    while out.vertices.len() < n {
        let segs = out.segments();
        if segs == 0 {
            break;
        }
        let (mut bi, mut bl) = (0usize, -1.0);
        for i in 0..segs {
            let s = out.segment(i);
            let l = (s[3][0] - s[0][0]).hypot(s[3][1] - s[0][1]) + 1e-9;
            if l > bl {
                bl = l;
                bi = i;
            }
        }
        let s = out.segment(bi);
        let mid = |a: [f64; 2], b: [f64; 2]| [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
        let (p01, p12, p23) = (mid(s[0], s[1]), mid(s[1], s[2]), mid(s[2], s[3]));
        let (p012, p123) = (mid(p01, p12), mid(p12, p23));
        let m = mid(p012, p123);
        let a = bi;
        let b = (bi + 1) % out.vertices.len();
        out.outs[a] = [p01[0] - s[0][0], p01[1] - s[0][1]];
        out.ins[b] = [p23[0] - s[3][0], p23[1] - s[3][1]];
        out.vertices.insert(a + 1, m);
        out.ins.insert(a + 1, [p012[0] - m[0], p012[1] - m[1]]);
        out.outs.insert(a + 1, [p123[0] - m[0], p123[1] - m[1]]);
    }
    out
}

fn rotated(p: &BezPath, k: usize) -> BezPath {
    let n = p.vertices.len();
    if n == 0 || k == 0 {
        return p.clone();
    }
    let r = |v: &Vec<[f64; 2]>| -> Vec<[f64; 2]> { (0..n).map(|i| v[(i + k) % n]).collect() };
    BezPath { vertices: r(&p.vertices), ins: r(&p.ins), outs: r(&p.outs), closed: p.closed }
}

fn similarity(a: &[[f64; 2]], b: &[[f64; 2]]) -> ([f64; 2], [f64; 2], f64, f64) {
    let n = a.len().max(1) as f64;
    let ca = [a.iter().map(|p| p[0]).sum::<f64>() / n, a.iter().map(|p| p[1]).sum::<f64>() / n];
    let cb = [b.iter().map(|p| p[0]).sum::<f64>() / n, b.iter().map(|p| p[1]).sum::<f64>() / n];
    let (mut sxx, mut sxy, mut na) = (0.0, 0.0, 0.0);
    for (p, q) in a.iter().zip(b) {
        let (x, y) = (p[0] - ca[0], p[1] - ca[1]);
        let (u, v) = (q[0] - cb[0], q[1] - cb[1]);
        sxx += x * u + y * v;
        sxy += x * v - y * u;
        na += x * x + y * y;
    }
    let ang = sxy.atan2(sxx);
    let sc = if na > 1e-12 { (sxx * sxx + sxy * sxy).sqrt() / na } else { 1.0 };
    (ca, cb, ang, sc.max(1e-9).ln())
}

pub struct Matched {
    pub a: BezPath,
    pub b: BezPath,
}

pub fn match_paths(a: &BezPath, b: &BezPath, add_vertices: bool, first_match: bool) -> Result<Matched, String> {
    let (na, nb) = (a.vertices.len(), b.vertices.len());
    if na == 0 || nb == 0 {
        return Err("both mask shapes need vertices".into());
    }
    let (a, b) = (a.clone().normalized(), b.clone().normalized());
    let (a, b) = if na == nb {
        (a, b)
    } else if add_vertices {
        let n = na.max(nb);
        (resample(&a, n), resample(&b, n))
    } else {
        return Err("the two mask shapes have different vertex counts; turn on Add Mask Shape Vertices".into());
    };
    let b = if first_match || !b.closed {
        b
    } else {
        let n = b.vertices.len();
        let cost = |k: usize| -> f64 { (0..n).map(|i| { let (p, q) = (a.vertices[i], b.vertices[(i + k) % n]); (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) }).sum() };
        let best = (0..n).min_by(|x, y| cost(*x).total_cmp(&cost(*y))).unwrap_or(0);
        rotated(&b, best)
    };
    Ok(Matched { a, b })
}

pub fn between(m: &Matched, t: f64, linear: bool, bending: f64) -> BezPath {
    let lin = m.a.lerp(&m.b, t);
    if linear {
        return lin;
    }
    let (ca, cb, ang, ls) = similarity(&m.a.vertices, &m.b.vertices);
    let (s, c) = (ang * t).sin_cos();
    let k = (ls * t).exp();
    let center = [ca[0] + (cb[0] - ca[0]) * t, ca[1] + (cb[1] - ca[1]) * t];
    let place = |p: [f64; 2]| -> [f64; 2] {
        let (x, y) = (p[0] - ca[0], p[1] - ca[1]);
        [center[0] + k * (c * x - s * y), center[1] + k * (s * x + c * y)]
    };
    let dir = |p: [f64; 2]| -> [f64; 2] { [k * (c * p[0] - s * p[1]), k * (s * p[0] + c * p[1])] };
    let (sb, cbn) = ang.sin_cos();
    let kb = ls.exp();
    let to_b = |p: [f64; 2]| -> [f64; 2] {
        let (x, y) = (p[0] - ca[0], p[1] - ca[1]);
        [cb[0] + kb * (cbn * x - sb * y), cb[1] + kb * (sb * x + cbn * y)]
    };
    let w = (bending / 100.0).clamp(0.0, 1.0);
    let mut out = lin.clone();
    for i in 0..out.vertices.len() {
        let rigid = place(m.a.vertices[i]);
        let resid_b = [m.b.vertices[i][0] - to_b(m.a.vertices[i])[0], m.b.vertices[i][1] - to_b(m.a.vertices[i])[1]];
        let v = [rigid[0] + resid_b[0] * t, rigid[1] + resid_b[1] * t];
        out.vertices[i] = [lin.vertices[i][0] + (v[0] - lin.vertices[i][0]) * w, lin.vertices[i][1] + (v[1] - lin.vertices[i][1]) * w];
        let dir_b = |p: [f64; 2]| -> [f64; 2] { [kb * (cbn * p[0] - sb * p[1]), kb * (sb * p[0] + cbn * p[1])] };
        let tan = |a: [f64; 2], b: [f64; 2]| -> [f64; 2] {
            let (r, rb) = (dir(a), dir_b(a));
            [r[0] + (b[0] - rb[0]) * t, r[1] + (b[1] - rb[1]) * t]
        };
        let (ri, ro) = (tan(m.a.ins[i], m.b.ins[i]), tan(m.a.outs[i], m.b.outs[i]));
        out.ins[i] = [lin.ins[i][0] + (ri[0] - lin.ins[i][0]) * w, lin.ins[i][1] + (ri[1] - lin.ins[i][1]) * w];
        out.outs[i] = [lin.outs[i][0] + (ro[0] - lin.outs[i][0]) * w, lin.outs[i][1] + (ro[1] - lin.outs[i][1]) * w];
    }
    out
}
