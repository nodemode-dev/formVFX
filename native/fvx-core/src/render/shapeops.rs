use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::style::{LineJoin, OutlineStyle};

use crate::expr::noise;
use crate::geom::Mat3;
use crate::model::path::BezPath;

type Contour = Vec<[f64; 2]>;

pub fn contours(paths: &[BezPath], tol: f64) -> Vec<Contour> {
    paths
        .iter()
        .filter(|p| p.vertices.len() >= 2)
        .map(|p| {
            let mut pts = p.flatten(tol);
            if pts.len() > 1 && pts.first() == pts.last() {
                pts.pop();
            }
            pts
        })
        .filter(|c| c.len() >= 3)
        .collect()
}

fn from_shapes(shapes: Vec<Vec<Vec<[f64; 2]>>>) -> Vec<BezPath> {
    shapes.into_iter().flatten().filter(|c| c.len() >= 3).map(|c| BezPath::polygon(&c, true)).collect()
}

fn tol_for(paths: &[BezPath]) -> f64 {
    let b = paths.iter().fold(None, |acc, p| crate::model::path::union_bounds(acc, p.bounds()));
    match b {
        Some(b) => ((b[2] - b[0]).max(b[3] - b[1]) / 2000.0).clamp(0.05, 0.5),
        None => 0.25,
    }
}

pub fn merge(paths: &[BezPath], mode: u32) -> Vec<BezPath> {
    if paths.is_empty() {
        return vec![];
    }
    let tol = tol_for(paths);
    let cs = contours(paths, tol);
    if cs.is_empty() {
        return vec![];
    }
    let rule = match mode {
        2 => OverlayRule::Difference,
        3 => OverlayRule::Intersect,
        4 => OverlayRule::Xor,
        _ => OverlayRule::Union,
    };
    if mode == 0 {
        let empty: Vec<Contour> = vec![];
        return from_shapes(cs.overlay(&empty, OverlayRule::Subject, FillRule::NonZero));
    }
    let mut acc: Vec<Vec<Contour>> = cs[0].overlay(&Vec::<Contour>::new(), OverlayRule::Subject, FillRule::NonZero);
    for c in &cs[1..] {
        let clip: Vec<Contour> = vec![c.clone()];
        acc = acc.overlay(&clip, rule, FillRule::NonZero);
    }
    from_shapes(acc)
}

pub fn offset(paths: &[BezPath], amount: f64, join: u32, miter: f64, copies: f64, copy_offset: f64) -> Vec<BezPath> {
    let n = copies.round().max(0.0) as usize;
    if n == 0 {
        return paths.to_vec();
    }
    let tol = tol_for(paths);
    let mut out = vec![];
    for p in paths {
        if !p.closed {
            out.extend(offset_open(p, amount, n, copy_offset));
            continue;
        }
        let cs = contours(std::slice::from_ref(p), tol);
        for k in 0..n {
            let d = amount * (k as f64 + 1.0 + copy_offset);
            if d.abs() < 1e-9 {
                out.push(p.clone());
                continue;
            }
            let lj = match join {
                1 => LineJoin::Round(0.1),
                2 => LineJoin::Bevel,
                _ => LineJoin::Miter((1.0 / miter.max(1.0)).asin() * 2.0),
            };
            let style = OutlineStyle::new(d).line_join(lj);
            let shapes = cs.outline(&style);
            out.extend(from_shapes(shapes));
        }
    }
    out
}

fn offset_open(p: &BezPath, amount: f64, n: usize, copy_offset: f64) -> Vec<BezPath> {
    let pts = p.flatten(0.25);
    if pts.len() < 2 {
        return vec![p.clone()];
    }
    let normals = normals(&pts, false);
    (0..n)
        .map(|k| {
            let d = amount * (k as f64 + 1.0 + copy_offset);
            let q: Vec<[f64; 2]> = pts.iter().zip(&normals).map(|(a, nn)| [a[0] + nn[0] * d, a[1] + nn[1] * d]).collect();
            BezPath::polygon(&q, false)
        })
        .collect()
}

fn normals(pts: &[[f64; 2]], closed: bool) -> Vec<[f64; 2]> {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let prev = if i > 0 { pts[i - 1] } else if closed { pts[n - 1] } else { pts[i] };
            let next = if i + 1 < n { pts[i + 1] } else if closed { pts[0] } else { pts[i] };
            let d = [next[0] - prev[0], next[1] - prev[1]];
            let l = d[0].hypot(d[1]).max(1e-12);
            [d[1] / l, -d[0] / l]
        })
        .collect()
}

fn centroid(p: &BezPath) -> [f64; 2] {
    let n = p.vertices.len().max(1) as f64;
    let s = p.vertices.iter().fold([0.0, 0.0], |a, v| [a[0] + v[0], a[1] + v[1]]);
    [s[0] / n, s[1] / n]
}

pub fn pucker_bloat(paths: &[BezPath], amount: f64) -> Vec<BezPath> {
    let k = amount / 100.0;
    if k.abs() < 1e-9 {
        return paths.to_vec();
    }
    paths
        .iter()
        .map(|p| {
            let c = centroid(p);
            let mut q = p.clone().normalized();
            for i in 0..q.vertices.len() {
                let v = q.vertices[i];
                let a_in = [v[0] + q.ins[i][0], v[1] + q.ins[i][1]];
                let a_out = [v[0] + q.outs[i][0], v[1] + q.outs[i][1]];
                let nv = [v[0] + (c[0] - v[0]) * k, v[1] + (c[1] - v[1]) * k];
                let ni = [a_in[0] - (c[0] - a_in[0]) * k, a_in[1] - (c[1] - a_in[1]) * k];
                let no = [a_out[0] - (c[0] - a_out[0]) * k, a_out[1] - (c[1] - a_out[1]) * k];
                q.vertices[i] = nv;
                q.ins[i] = [ni[0] - nv[0], ni[1] - nv[1]];
                q.outs[i] = [no[0] - nv[0], no[1] - nv[1]];
            }
            q
        })
        .collect()
}

pub fn twist(paths: &[BezPath], angle: f64, center: [f64; 2]) -> Vec<BezPath> {
    if angle.abs() < 1e-9 {
        return paths.to_vec();
    }
    let tol = tol_for(paths) * 8.0;
    let all: Vec<Vec<[f64; 2]>> = paths.iter().map(|p| dense(p, tol.max(1.0))).collect();
    let r = all.iter().flatten().map(|q| (q[0] - center[0]).hypot(q[1] - center[1])).fold(0.0f64, f64::max).max(1e-6);
    paths
        .iter()
        .zip(all)
        .map(|(p, pts)| {
            let mut pts = pts;
            if p.closed && pts.len() > 1 && pts.first() == pts.last() {
                pts.pop();
            }
            let q: Vec<[f64; 2]> = pts
                .iter()
                .map(|v| {
                    let dx = v[0] - center[0];
                    let dy = v[1] - center[1];
                    let d = dx.hypot(dy);
                    let a = (angle * (1.0 - (d / r).min(1.0))).to_radians();
                    let (s, c) = a.sin_cos();
                    [center[0] + dx * c - dy * s, center[1] + dx * s + dy * c]
                })
                .collect();
            BezPath::polygon(&q, p.closed)
        })
        .collect()
}

fn dense(p: &BezPath, step: f64) -> Vec<[f64; 2]> {
    let mut out = vec![];
    if p.vertices.is_empty() {
        return out;
    }
    out.push(p.vertices[0]);
    for i in 0..p.segments() {
        let s = p.segment(i);
        let l = crate::model::path::dist(s[0], s[1]) + crate::model::path::dist(s[1], s[2]) + crate::model::path::dist(s[2], s[3]);
        let n = ((l / step).ceil() as usize).clamp(1, 512);
        for k in 1..=n {
            out.push(crate::model::path::cubic(&s, k as f64 / n as f64));
        }
    }
    out
}

fn seg_samples(p: &BezPath, per_seg: usize) -> Vec<([f64; 2], [f64; 2], bool)> {
    let mut out = vec![];
    let segs = p.segments();
    for i in 0..segs {
        let s = p.segment(i);
        for k in 0..per_seg {
            let t = k as f64 / per_seg as f64;
            let pt = crate::model::path::cubic(&s, t);
            let e = 1e-3;
            let a = crate::model::path::cubic(&s, (t - e).max(0.0));
            let b = crate::model::path::cubic(&s, (t + e).min(1.0));
            out.push((pt, [b[0] - a[0], b[1] - a[1]], k == 0));
        }
    }
    if !p.closed && let Some(last) = p.vertices.last() {
        let s = p.segment(segs.saturating_sub(1));
        out.push((*last, [s[3][0] - s[2][0], s[3][1] - s[2][1]], true));
    }
    out
}

fn smooth_handles(pts: &[[f64; 2]], closed: bool) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let n = pts.len();
    let mut ins = vec![[0.0; 2]; n];
    let mut outs = vec![[0.0; 2]; n];
    for i in 0..n {
        let prev = if i > 0 { pts[i - 1] } else if closed { pts[n - 1] } else { continue };
        let next = if i + 1 < n { pts[i + 1] } else if closed { pts[0] } else { continue };
        let t = [(next[0] - prev[0]) / 6.0, (next[1] - prev[1]) / 6.0];
        ins[i] = [-t[0], -t[1]];
        outs[i] = t;
    }
    (ins, outs)
}

pub fn zigzag(paths: &[BezPath], size: f64, ridges: f64, smooth: bool) -> Vec<BezPath> {
    let per = ridges.round().max(0.0) as usize + 1;
    paths
        .iter()
        .map(|p| {
            let samples = seg_samples(p, per);
            let mut pts = vec![];
            for (i, (pt, tan, _)) in samples.iter().enumerate() {
                let l = tan[0].hypot(tan[1]).max(1e-12);
                let nrm = [tan[1] / l, -tan[0] / l];
                let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
                pts.push([pt[0] + nrm[0] * size * sign, pt[1] + nrm[1] * size * sign]);
            }
            if smooth {
                let (ins, outs) = smooth_handles(&pts, p.closed);
                BezPath { vertices: pts, ins, outs, closed: p.closed }
            } else {
                BezPath::polygon(&pts, p.closed)
            }
        })
        .collect()
}

pub fn noise1(seed: u64, channel: u64, x: f64) -> f64 {
    let i = x.floor();
    let t = x - i;
    let a = noise::unit(noise::hash(&[seed, channel, i as i64 as u64])) * 2.0 - 1.0;
    let b = noise::unit(noise::hash(&[seed, channel, (i as i64 + 1) as u64])) * 2.0 - 1.0;
    let w = t * t * (3.0 - 2.0 * t);
    a + (b - a) * w
}

pub struct Wiggle {
    pub size: f64,
    pub detail: f64,
    pub smooth: bool,
    pub wps: f64,
    pub correlation: f64,
    pub temporal: f64,
    pub spatial: f64,
    pub seed: f64,
}

pub fn wiggle_paths(paths: &[BezPath], w: &Wiggle, t: f64) -> Vec<BezPath> {
    let per = (w.detail.round().max(0.0) as usize).max(1);
    let seed = w.seed.to_bits() ^ 0x5157;
    let x = t * w.wps + w.temporal / 360.0;
    let corr = (w.correlation / 100.0).clamp(0.0, 1.0);
    let mut idx = 0u64;
    paths
        .iter()
        .map(|p| {
            let samples = seg_samples(p, per);
            let mut pts = vec![];
            for (pt, _, _) in samples {
                idx += 1;
                let sp = w.spatial / 360.0 * idx as f64;
                let dx = corr * noise1(seed, 1, x) + (1.0 - corr) * noise1(seed, idx * 2 + 10, x + sp);
                let dy = corr * noise1(seed, 2, x) + (1.0 - corr) * noise1(seed, idx * 2 + 11, x + sp);
                pts.push([pt[0] + dx * w.size, pt[1] + dy * w.size]);
            }
            if p.closed && pts.len() > 1 && pts.first() == pts.last() {
                pts.pop();
            }
            if w.smooth {
                let (ins, outs) = smooth_handles(&pts, p.closed);
                BezPath { vertices: pts, ins, outs, closed: p.closed }
            } else {
                BezPath::polygon(&pts, p.closed)
            }
        })
        .collect()
}

pub struct WiggleXf {
    pub wps: f64,
    pub correlation: f64,
    pub temporal: f64,
    pub spatial: f64,
    pub seed: f64,
    pub anchor: [f64; 2],
    pub position: [f64; 2],
    pub scale: [f64; 2],
    pub rotation: f64,
}

pub fn wiggle_matrix(w: &WiggleXf, t: f64, instance: usize) -> Mat3 {
    let seed = w.seed.to_bits() ^ 0x77f;
    let x = t * w.wps + w.temporal / 360.0;
    let corr = (w.correlation / 100.0).clamp(0.0, 1.0);
    let sp = w.spatial / 360.0 * instance as f64;
    let n = |ch: u64| corr * noise1(seed, ch, x) + (1.0 - corr) * noise1(seed, ch + 100 * (instance as u64 + 1), x + sp);
    let px = w.position[0] * n(1);
    let py = w.position[1] * n(2);
    let sx = 1.0 + w.scale[0] / 100.0 * n(3);
    let sy = 1.0 + w.scale[1] / 100.0 * n(4);
    let r = w.rotation * n(5);
    Mat3::translate(px, py) * Mat3::translate(w.anchor[0], w.anchor[1]) * Mat3::rotate_deg(r) * Mat3::scale(sx, sy) * Mat3::translate(-w.anchor[0], -w.anchor[1])
}

pub struct Taper {
    pub percent: bool,
    pub start_len: f64,
    pub end_len: f64,
    pub start_w: f64,
    pub end_w: f64,
    pub start_ease: f64,
    pub end_ease: f64,
}

pub struct WaveSpec {
    pub amount: f64,
    pub cycles_units: bool,
    pub wavelength: f64,
    pub cycles: f64,
    pub phase: f64,
}

fn ease_ramp(t: f64, ease: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let s = t * t * (3.0 - 2.0 * t);
    t + (s - t) * (ease / 100.0).clamp(0.0, 1.0)
}

pub fn tapered(path: &BezPath, width: f64, cap: u32, taper: &Taper, wave: &WaveSpec) -> Vec<BezPath> {
    let pts = {
        let mut v = path.flatten(0.2);
        if path.closed && v.len() > 1 && v.first() == v.last() {
            v.pop();
        }
        v
    };
    if pts.len() < 2 {
        return vec![];
    }
    let closed = path.closed;
    let mut cum = vec![0.0];
    for i in 1..pts.len() {
        cum.push(cum[i - 1] + (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1]));
    }
    let total = if closed { cum.last().unwrap() + (pts[0][0] - pts[pts.len() - 1][0]).hypot(pts[0][1] - pts[pts.len() - 1][1]) } else { *cum.last().unwrap() };
    if total <= 1e-9 {
        return vec![];
    }
    let (sl, el) = if taper.percent { (taper.start_len / 100.0 * total, taper.end_len / 100.0 * total) } else { (taper.start_len, taper.end_len) };
    let wl = if wave.cycles_units { total / wave.cycles.max(0.01) } else { wave.wavelength.max(0.1) };
    let half = |s: f64| -> f64 {
        let mut f = 1.0;
        if sl > 1e-9 && s < sl {
            let u = ease_ramp(s / sl, taper.start_ease);
            f *= taper.start_w / 100.0 + (1.0 - taper.start_w / 100.0) * u;
        }
        if el > 1e-9 && s > total - el {
            let u = ease_ramp((total - s) / el, taper.end_ease);
            f *= taper.end_w / 100.0 + (1.0 - taper.end_w / 100.0) * u;
        }
        if wave.amount > 0.0 {
            let th = std::f64::consts::TAU * s / wl + wave.phase.to_radians();
            f *= 1.0 - wave.amount / 100.0 * (0.5 - 0.5 * th.sin());
        }
        (width * f / 2.0).max(0.0)
    };
    let nrm = normals(&pts, closed);
    let left: Vec<[f64; 2]> = pts.iter().zip(&nrm).zip(&cum).map(|((p, n), s)| [p[0] + n[0] * half(*s), p[1] + n[1] * half(*s)]).collect();
    let right: Vec<[f64; 2]> = pts.iter().zip(&nrm).zip(&cum).map(|((p, n), s)| [p[0] - n[0] * half(*s), p[1] - n[1] * half(*s)]).collect();
    if closed {
        let mut r = right;
        r.reverse();
        return vec![BezPath::polygon(&left, true), BezPath::polygon(&r, true)];
    }
    let mut poly = left.clone();
    let cap_pts = |c: [f64; 2], from: [f64; 2], hw: f64, out: &mut Vec<[f64; 2]>| {
        if cap != 1 || hw <= 1e-9 {
            return;
        }
        let a0 = (from[1] - c[1]).atan2(from[0] - c[0]);
        for k in 1..12 {
            let a = a0 - std::f64::consts::PI * k as f64 / 12.0;
            out.push([c[0] + a.cos() * hw, c[1] + a.sin() * hw]);
        }
    };
    let last = pts.len() - 1;
    cap_pts(pts[last], left[last], half(total), &mut poly);
    poly.extend(right.iter().rev());
    cap_pts(pts[0], right[0], half(0.0), &mut poly);
    vec![BezPath::polygon(&poly, true)]
}
