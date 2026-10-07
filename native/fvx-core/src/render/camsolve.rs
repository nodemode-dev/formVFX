use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::analysis::Source;
use super::vision::{self, Model};
use crate::engine::jobs::Ctx;

pub type V3 = [f64; 3];
pub type M3 = [[f64; 3]; 3];

pub const I3: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
pub const SHOT_TYPES: [&str; 3] = ["Fixed Angle of View", "Variable Zoom", "Specify Angle of View"];
pub const SOLVE_METHODS: [&str; 4] = ["Auto Detect", "Typical", "Mostly Flat Scene", "Tripod Pan"];

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn scale(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}

pub fn unit(a: V3) -> V3 {
    let l = len(a);
    if l < 1e-15 { [0.0, 0.0, 1.0] } else { scale(a, 1.0 / l) }
}

pub fn mm(a: &M3, b: &M3) -> M3 {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    o
}

pub fn mt(a: &M3) -> M3 {
    [[a[0][0], a[1][0], a[2][0]], [a[0][1], a[1][1], a[2][1]], [a[0][2], a[1][2], a[2][2]]]
}

pub fn mv(a: &M3, v: V3) -> V3 {
    [dot(a[0], v), dot(a[1], v), dot(a[2], v)]
}

fn cols(a: V3, b: V3, c: V3) -> M3 {
    [[a[0], b[0], c[0]], [a[1], b[1], c[1]], [a[2], b[2], c[2]]]
}

fn col(m: &M3, j: usize) -> V3 {
    [m[0][j], m[1][j], m[2][j]]
}

pub fn col3(m: &M3, j: usize) -> V3 {
    col(m, j)
}

fn det(m: &M3) -> f64 {
    dot(m[0], cross(m[1], m[2]))
}

fn skew(w: V3) -> M3 {
    [[0.0, -w[2], w[1]], [w[2], 0.0, -w[0]], [-w[1], w[0], 0.0]]
}

pub fn rodrigues(w: V3) -> M3 {
    let th = len(w);
    let k = skew(w);
    let k2 = mm(&k, &k);
    let (a, b) = if th < 1e-9 { (1.0, 0.5) } else { (th.sin() / th, (1.0 - th.cos()) / (th * th)) };
    let mut r = I3;
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] += a * k[i][j] + b * k2[i][j];
        }
    }
    r
}

pub fn euler_xyz(r: &M3) -> V3 {
    let b = r[0][2].clamp(-1.0, 1.0).asin();
    let (a, c) = if b.cos().abs() > 1e-6 { ((-r[1][2]).atan2(r[2][2]), (-r[0][1]).atan2(r[0][0])) } else { (r[2][1].atan2(r[1][1]), 0.0) };
    let wrap = |d: f64| {
        let d = d.to_degrees() % 360.0;
        if d < 0.0 { d + 360.0 } else { d }
    };
    [wrap(a), wrap(b), wrap(c)]
}

pub fn sym_eig(mut a: Vec<Vec<f64>>) -> (Vec<f64>, Vec<Vec<f64>>) {
    let n = a.len();
    let mut v: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    for _ in 0..80 {
        let mut off = 0.0;
        for p in 0..n {
            for q in p + 1..n {
                off += a[p][q] * a[p][q];
            }
        }
        if off < 1e-26 {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                if a[p][q].abs() < 1e-300 {
                    continue;
                }
                let th = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let t = if th == 0.0 { 1.0 } else { th.signum() / (th.abs() + (th * th + 1.0).sqrt()) };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let (x, y) = (a[k][p], a[k][q]);
                    a[k][p] = c * x - s * y;
                    a[k][q] = s * x + c * y;
                }
                for k in 0..n {
                    let (x, y) = (a[p][k], a[q][k]);
                    a[p][k] = c * x - s * y;
                    a[q][k] = s * x + c * y;
                }
                for k in 0..n {
                    let (x, y) = (v[k][p], v[k][q]);
                    v[k][p] = c * x - s * y;
                    v[k][q] = s * x + c * y;
                }
            }
        }
    }
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&i, &j| a[i][i].total_cmp(&a[j][j]));
    (idx.iter().map(|&i| a[i][i]).collect(), idx.iter().map(|&i| (0..n).map(|k| v[k][i]).collect()).collect())
}

fn null_vec(rows: &[Vec<f64>], n: usize) -> Vec<f64> {
    let mut ata = vec![vec![0.0; n]; n];
    for r in rows {
        for i in 0..n {
            for j in i..n {
                ata[i][j] += r[i] * r[j];
            }
        }
    }
    for i in 0..n {
        for j in 0..i {
            ata[i][j] = ata[j][i];
        }
    }
    sym_eig(ata).1.swap_remove(0)
}

pub fn svd3(m: &M3) -> (M3, V3, M3) {
    let mtm = mm(&mt(m), m);
    let (vals, vecs) = sym_eig(mtm.iter().map(|r| r.to_vec()).collect());
    let v: Vec<V3> = [2, 1, 0].iter().map(|&i| [vecs[i][0], vecs[i][1], vecs[i][2]]).collect();
    let s: V3 = [vals[2].max(0.0).sqrt(), vals[1].max(0.0).sqrt(), vals[0].max(0.0).sqrt()];
    let u0 = if s[0] > 1e-15 { scale(mv(m, v[0]), 1.0 / s[0]) } else { [1.0, 0.0, 0.0] };
    let u1 = if s[1] > s[0] * 1e-12 { scale(mv(m, v[1]), 1.0 / s[1]) } else { unit(cross(u0, if u0[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] })) };
    let u1 = unit(sub(u1, scale(u0, dot(u0, u1))));
    let u2 = if s[2] > s[0] * 1e-9 {
        let c = scale(mv(m, v[2]), 1.0 / s[2]);
        let x = cross(u0, u1);
        if dot(c, x) >= 0.0 { x } else { scale(x, -1.0) }
    } else {
        cross(u0, u1)
    };
    (cols(u0, u1, u2), s, cols(v[0], v[1], v[2]))
}

fn nearest_rotation(m: &M3) -> M3 {
    let (u, _, v) = svd3(m);
    let mut r = mm(&u, &mt(&v));
    if det(&r) < 0.0 {
        let mut u2 = u;
        for row in u2.iter_mut() {
            row[2] = -row[2];
        }
        r = mm(&u2, &mt(&v));
    }
    r
}

fn kabsch(a: &[V3], b: &[V3]) -> M3 {
    let mut h = [[0.0; 3]; 3];
    for (p, q) in a.iter().zip(b) {
        for i in 0..3 {
            for j in 0..3 {
                h[i][j] += q[i] * p[j];
            }
        }
    }
    nearest_rotation(&h)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub id: u32,
    pub start: usize,
    pub pts: Vec<[f32; 2]>,
}

impl Track {
    pub fn at(&self, k: usize) -> Option<[f64; 2]> {
        let i = k.checked_sub(self.start)?;
        self.pts.get(i).map(|p| [p[0] as f64, p[1] as f64])
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tracks {
    pub frame: [f64; 4],
    pub t0: f64,
    pub fps: f64,
    pub frames: usize,
    pub px: f64,
    pub tracks: Vec<Track>,
}

impl Tracks {
    pub fn encode(&self) -> String {
        let mut s = format!("c1;{};{};{};{};{};{};{};{}", self.frame[0], self.frame[1], self.frame[2], self.frame[3], self.t0, self.fps, self.frames, self.px);
        for t in &self.tracks {
            s.push(';');
            s.push_str(&format!("{}:{}:", t.id, t.start));
            let v: Vec<String> = t.pts.iter().map(|p| format!("{:.1} {:.1}", p[0], p[1])).collect();
            s.push_str(&v.join(","));
        }
        s
    }

    pub fn decode(s: &str) -> Option<Tracks> {
        let mut it = s.split(';');
        if it.next()? != "c1" {
            return None;
        }
        let mut num = || it.next().and_then(|x| x.parse::<f64>().ok());
        let frame = [num()?, num()?, num()?, num()?];
        let (t0, fps, frames, px) = (num()?, num()?, num()? as usize, num()?);
        let mut tracks = vec![];
        for part in it {
            let mut f = part.splitn(3, ':');
            let id = f.next()?.parse().ok()?;
            let start = f.next()?.parse().ok()?;
            let pts = f.next()?.split(',').filter_map(|p| {
                let (a, b) = p.split_once(' ')?;
                Some([a.parse().ok()?, b.parse().ok()?])
            }).collect();
            tracks.push(Track { id, start, pts });
        }
        Some(Tracks { frame, t0, fps, frames, px, tracks })
    }

    pub fn frame_of(&self, lt: f64) -> usize {
        (((lt - self.t0) * self.fps).round().max(0.0) as usize).min(self.frames.saturating_sub(1))
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Cam {
    pub r: M3,
    pub c: V3,
    pub f: f64,
}

impl Cam {
    pub fn depth(&self, x: V3) -> f64 {
        mv(&self.r, sub(x, self.c))[2]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Point {
    pub id: u32,
    pub x: V3,
    pub err: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Solve {
    pub frame: [f64; 4],
    pub t0: f64,
    pub fps: f64,
    pub cams: Vec<Cam>,
    pub solved: Vec<bool>,
    pub points: Vec<Point>,
    pub method: String,
    pub error: f64,
}

impl Solve {
    pub fn center(&self) -> [f64; 2] {
        [(self.frame[0] + self.frame[2]) * 0.5, (self.frame[1] + self.frame[3]) * 0.5]
    }

    pub fn project(&self, k: usize, x: V3) -> Option<[f64; 2]> {
        let cam = self.cams.get(k)?;
        let c = self.center();
        proj(cam, c, x)
    }

    pub fn frame_of(&self, lt: f64) -> usize {
        (((lt - self.t0) * self.fps).round().max(0.0) as usize).min(self.cams.len().saturating_sub(1))
    }
}

fn proj(cam: &Cam, c: [f64; 2], x: V3) -> Option<[f64; 2]> {
    let p = mv(&cam.r, sub(x, cam.c));
    if p[2] <= 1e-9 {
        return None;
    }
    Some([c[0] + cam.f * p[0] / p[2], c[1] + cam.f * p[1] / p[2]])
}

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

type Cache<T> = Mutex<HashMap<u64, Arc<T>>>;

fn cached<T>(c: &'static OnceLock<Cache<T>>, s: &str, parse: impl Fn(&str) -> Option<T>) -> Option<Arc<T>> {
    if s.is_empty() {
        return None;
    }
    let k = hash(s);
    let c = c.get_or_init(Default::default);
    if let Some(d) = c.lock().get(&k) {
        return Some(d.clone());
    }
    let d = Arc::new(parse(s)?);
    let mut g = c.lock();
    if g.len() > 8 {
        g.clear();
    }
    g.insert(k, d.clone());
    Some(d)
}

pub fn parsed_tracks(s: &str) -> Option<Arc<Tracks>> {
    static C: OnceLock<Cache<Tracks>> = OnceLock::new();
    cached(&C, s, Tracks::decode)
}

pub fn parsed_solve(s: &str) -> Option<Arc<Solve>> {
    static C: OnceLock<Cache<Solve>> = OnceLock::new();
    cached(&C, s, |x| serde_json::from_str(x).ok())
}

pub fn track(ctx: &Ctx, src: &Source, times: &[f64], layer_times: &[f64], fps: f64, detailed: bool) -> Result<Tracks, String> {
    let g0 = src.gray(times[0], 1, 0.0).ok_or("cannot read the layer's frames")?;
    let (w, h) = (g0.w as f32, g0.h as f32);
    let max = if detailed { 700 } else { 350 };
    let md = (w.max(h) / if detailed { 60.0 } else { 40.0 }).max(4.0);
    let mut tracks: Vec<Track> = vec![];
    let mut live: Vec<(usize, [f32; 2])> = vec![];
    for p in vision::corners(&g0, &|_, _| true, max, md) {
        live.push((tracks.len(), p));
        tracks.push(Track { id: 0, start: 0, pts: vec![to_layer(src, p)] });
    }
    if live.len() < 12 {
        return Err("the footage has too little detail to track".into());
    }
    let mut prev = g0.pyramid(4);
    for (fi, t) in times.iter().enumerate().skip(1) {
        if ctx.stopped() {
            return Err("stopped".into());
        }
        ctx.step(fi, times.len() - 1, "Tracking features");
        let g = src.gray(*t, 1, 0.0).ok_or("cannot read the layer's frames")?;
        let pyr = g.pyramid(4);
        let pts: Vec<[f32; 2]> = live.iter().map(|l| l.1).collect();
        let fwd = vision::klt(&prev, &pyr, &pts, None, 7);
        let moved: Vec<[f32; 2]> = fwd.iter().zip(&pts).map(|(f, p)| f.unwrap_or(*p)).collect();
        let back = vision::klt(&pyr, &prev, &moved, Some(&pts), 7);
        let mut next = vec![];
        for (i, (f, b)) in fwd.iter().zip(&back).enumerate() {
            let (Some(q), Some(r)) = (f, b) else { continue };
            if (r[0] - pts[i][0]).hypot(r[1] - pts[i][1]) > 0.6 || q[0] < 2.0 || q[1] < 2.0 || q[0] > w - 3.0 || q[1] > h - 3.0 {
                continue;
            }
            let ti = live[i].0;
            tracks[ti].pts.push(to_layer(src, *q));
            next.push((ti, *q));
        }
        live = next;
        if live.len() < max * 7 / 10 {
            let existing: Vec<[f32; 2]> = live.iter().map(|l| l.1).collect();
            let more = vision::corners(&g, &|x, y| existing.iter().all(|q| (q[0] - x).powi(2) + (q[1] - y).powi(2) > md * md), max - live.len(), md);
            for p in more {
                live.push((tracks.len(), p));
                tracks.push(Track { id: 0, start: fi, pts: vec![to_layer(src, p)] });
            }
        }
        prev = pyr;
    }
    let min_len = 6.min(times.len());
    let mut kept: Vec<Track> = tracks.into_iter().filter(|t| t.pts.len() >= min_len).collect();
    for (i, t) in kept.iter_mut().enumerate() {
        t.id = i as u32 + 1;
    }
    Ok(Tracks { frame: src.frame, t0: layer_times[0], fps, frames: times.len(), px: 1.0 / src.scale, tracks: kept })
}

fn to_layer(src: &Source, p: [f32; 2]) -> [f32; 2] {
    let q = src.to_layer(p);
    [q[0] as f32, q[1] as f32]
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub shot: u32,
    pub hfov: f64,
    pub method: u32,
    pub deleted: Vec<u32>,
}

fn focal_of(hfov: f64, w: f64) -> f64 {
    (w * 0.5) / (hfov.clamp(1.0, 170.0).to_radians() * 0.5).tan()
}

fn huber(r: f64, k: f64) -> f64 {
    let a = r.abs();
    if a <= k { 0.5 * a * a } else { k * (a - 0.5 * k) }
}

fn lm(x0: &[f64], f: &dyn Fn(&[f64]) -> Vec<f64>, iters: usize, k: f64) -> Vec<f64> {
    let n = x0.len();
    let mut x = x0.to_vec();
    let mut r = f(&x);
    let cost = |r: &[f64]| r.iter().map(|v| huber(*v, k)).sum::<f64>();
    let mut c = cost(&r);
    let mut lam = 1e-3;
    for _ in 0..iters {
        let m = r.len();
        if m == 0 {
            break;
        }
        let mut jac = vec![vec![0.0; n]; m];
        for j in 0..n {
            let h = 1e-6 * x[j].abs().max(1.0);
            let mut xp = x.clone();
            xp[j] += h;
            let rp = f(&xp);
            if rp.len() != m {
                return x;
            }
            for i in 0..m {
                jac[i][j] = (rp[i] - r[i]) / h;
            }
        }
        let mut jtj = vec![vec![0.0; n]; n];
        let mut jtr = vec![0.0; n];
        for i in 0..m {
            let a = r[i].abs();
            let wgt = if a <= k { 1.0 } else { k / a };
            for p in 0..n {
                jtr[p] += wgt * jac[i][p] * r[i];
                for q in 0..n {
                    jtj[p][q] += wgt * jac[i][p] * jac[i][q];
                }
            }
        }
        let mut improved = false;
        for _ in 0..6 {
            let mut a = jtj.clone();
            for p in 0..n {
                a[p][p] += lam * a[p][p].max(1e-9);
            }
            let mut b: Vec<f64> = jtr.iter().map(|v| -v).collect();
            let Some(dx) = vision::solve(&mut a, &mut b) else {
                lam *= 10.0;
                continue;
            };
            let xn: Vec<f64> = x.iter().zip(&dx).map(|(a, b)| a + b).collect();
            let rn = f(&xn);
            if rn.len() != m {
                lam *= 10.0;
                continue;
            }
            let cn = cost(&rn);
            if cn < c {
                let gain = c - cn;
                x = xn;
                r = rn;
                c = cn;
                lam = (lam * 0.3).max(1e-9);
                improved = gain > 1e-10 * c.max(1e-12);
                break;
            }
            lam *= 10.0;
        }
        if !improved {
            break;
        }
    }
    x
}

fn bearing(p: [f64; 2], c: [f64; 2], f: f64) -> V3 {
    unit([(p[0] - c[0]) / f, (p[1] - c[1]) / f, 1.0])
}

fn triangulate(views: &[(&Cam, [f64; 2])], c: [f64; 2]) -> Option<V3> {
    if views.len() < 2 {
        return None;
    }
    let mut ata = vec![vec![0.0; 3]; 3];
    let mut atb = vec![0.0; 3];
    for (cam, p) in views {
        let x = [(p[0] - c[0]) / cam.f, (p[1] - c[1]) / cam.f];
        for row in [sub(scale(cam.r[2], x[0]), cam.r[0]), sub(scale(cam.r[2], x[1]), cam.r[1])] {
            let b = dot(row, cam.c);
            for i in 0..3 {
                atb[i] += row[i] * b;
                for j in 0..3 {
                    ata[i][j] += row[i] * row[j];
                }
            }
        }
    }
    let v = vision::solve(&mut ata, &mut atb)?;
    Some([v[0], v[1], v[2]])
}

fn reproj(views: &[(&Cam, [f64; 2])], c: [f64; 2], x: V3) -> Option<f64> {
    let mut worst: f64 = 0.0;
    for (cam, p) in views {
        let q = proj(cam, c, x)?;
        worst = worst.max((q[0] - p[0]).hypot(q[1] - p[1]));
    }
    Some(worst)
}

fn essential_ransac(x1: &[V3], x2: &[V3], thresh: f64) -> Option<(M3, Vec<bool>)> {
    epipolar_ransac(x1, x2, thresh, true)
}

fn poses_from_e(e: &M3) -> Vec<(M3, V3)> {
    let (mut u, _, mut v) = svd3(e);
    if det(&u) < 0.0 {
        u = mm(&u, &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]);
    }
    if det(&v) < 0.0 {
        v = mm(&v, &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]);
    }
    let w = [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
    let r1 = mm(&mm(&u, &w), &mt(&v));
    let r2 = mm(&mm(&u, &mt(&w)), &mt(&v));
    let t = col(&u, 2);
    vec![(r1, t), (r1, scale(t, -1.0)), (r2, t), (r2, scale(t, -1.0))]
}

fn poses_from_h(h: &M3) -> Vec<(M3, V3)> {
    let (_, s, _) = svd3(h);
    if s[1] < 1e-12 {
        return vec![];
    }
    let h = h.map(|r| r.map(|x| x / s[1]));
    let m = mm(&mt(&h), &h);
    let (vals, vecs) = sym_eig(m.iter().map(|r| r.to_vec()).collect());
    let (l3, l1) = (vals[0], vals[2]);
    let v1: V3 = [vecs[2][0], vecs[2][1], vecs[2][2]];
    let v2: V3 = [vecs[1][0], vecs[1][1], vecs[1][2]];
    let v3: V3 = [vecs[0][0], vecs[0][1], vecs[0][2]];
    if l1 - l3 < 1e-9 {
        return vec![(nearest_rotation(&h), [0.0; 3])];
    }
    let (a, b, d) = ((1.0 - l3).max(0.0).sqrt(), (l1 - 1.0).max(0.0).sqrt(), (l1 - l3).sqrt());
    let mut out = vec![];
    for sgn in [1.0, -1.0] {
        let u = scale(add(scale(v1, a), scale(v3, sgn * b)), 1.0 / d);
        let uu = cols(v2, u, cross(v2, u));
        let hv2 = mv(&h, v2);
        let hu = mv(&h, u);
        let ww = cols(hv2, hu, cross(hv2, hu));
        let r = mm(&ww, &mt(&uu));
        let n = cross(v2, u);
        let mut hr = h;
        for i in 0..3 {
            for j in 0..3 {
                hr[i][j] -= r[i][j];
            }
        }
        let t = mv(&hr, n);
        out.push((r, t));
        out.push((r, scale(t, -1.0)));
    }
    out
}

fn cam_from(r: M3, t: V3, f: f64) -> Cam {
    Cam { r, c: scale(mv(&mt(&r), t), -1.0), f }
}

struct Obs {
    frames: Vec<Vec<(usize, [f64; 2])>>,
}

fn observations(tr: &Tracks, keep: &[usize]) -> Obs {
    let mut frames = vec![vec![]; tr.frames];
    for &ti in keep {
        let t = &tr.tracks[ti];
        for (i, p) in t.pts.iter().enumerate() {
            if let Some(f) = frames.get_mut(t.start + i) {
                f.push((ti, [p[0] as f64, p[1] as f64]));
            }
        }
    }
    Obs { frames }
}

fn common(tr: &Tracks, keep: &[usize], a: usize, b: usize) -> Vec<(usize, [f64; 2], [f64; 2])> {
    keep.iter().filter_map(|&ti| {
        let t = &tr.tracks[ti];
        Some((ti, t.at(a)?, t.at(b)?))
    }).collect()
}

fn h_residual(pairs: &[(usize, [f64; 2], [f64; 2])], px: f64) -> Option<(f64, vision::H3)> {
    let s: Vec<[f64; 2]> = pairs.iter().map(|p| p.1).collect();
    let d: Vec<[f64; 2]> = pairs.iter().map(|p| p.2).collect();
    let (h, _) = vision::ransac(Model::Homography, &s, &d, 1.5 * px, 300)?;
    let mut r: Vec<f64> = s.iter().zip(&d).map(|(a, b)| {
        let q = vision::h_apply(&h, *a);
        (q[0] - b[0]).hypot(q[1] - b[1])
    }).collect();
    r.sort_by(|a, b| a.total_cmp(b));
    Some((r[r.len() * 4 / 5], h))
}

fn kinv_h_k(h: &vision::H3, f: f64, c: [f64; 2]) -> M3 {
    let k = [[f, 0.0, c[0]], [0.0, f, c[1]], [0.0, 0.0, 1.0]];
    let ki = [[1.0 / f, 0.0, -c[0] / f], [0.0, 1.0 / f, -c[1] / f], [0.0, 0.0, 1.0]];
    mm(&ki, &mm(h, &k))
}

fn rotation_cost(hs: &[vision::H3], f: f64, c: [f64; 2]) -> f64 {
    hs.iter().map(|h| {
        let m = kinv_h_k(h, f, c);
        let d = det(&m);
        if d.abs() < 1e-12 {
            return 1.0;
        }
        let m = m.map(|r| r.map(|x| x / d.cbrt()));
        let r = nearest_rotation(&m);
        let mut s = 0.0;
        for i in 0..3 {
            for j in 0..3 {
                s += (m[i][j] - r[i][j]).powi(2);
            }
        }
        s.sqrt()
    }).sum()
}

fn fundamental(pairs: &[(usize, [f64; 2], [f64; 2])], c: [f64; 2], w: f64, px: f64) -> Option<M3> {
    let k = 2.0 / w;
    let x1: Vec<V3> = pairs.iter().map(|p| [(p.1[0] - c[0]) * k, (p.1[1] - c[1]) * k, 1.0]).collect();
    let x2: Vec<V3> = pairs.iter().map(|p| [(p.2[0] - c[0]) * k, (p.2[1] - c[1]) * k, 1.0]).collect();
    let (fn_, inl) = epipolar_ransac(&x1, &x2, 1.2 * px * k, false)?;
    if inl.iter().filter(|b| **b).count() < 12 {
        return None;
    }
    let t = [[k, 0.0, -c[0] * k], [0.0, k, -c[1] * k], [0.0, 0.0, 1.0]];
    Some(mm(&mt(&t), &mm(&fn_, &t)))
}

fn epipolar_ransac(x1: &[V3], x2: &[V3], thresh: f64, essential: bool) -> Option<(M3, Vec<bool>)> {
    let n = x1.len();
    if n < 8 {
        return None;
    }
    let fit = |idx: &[usize]| -> M3 {
        let rows: Vec<Vec<f64>> = idx.iter().map(|&i| {
            let (a, b) = (x1[i], x2[i]);
            vec![b[0] * a[0], b[0] * a[1], b[0] * a[2], b[1] * a[0], b[1] * a[1], b[1] * a[2], b[2] * a[0], b[2] * a[1], b[2] * a[2]]
        }).collect();
        let e = null_vec(&rows, 9);
        let m = [[e[0], e[1], e[2]], [e[3], e[4], e[5]], [e[6], e[7], e[8]]];
        let (u, s, v) = svd3(&m);
        let (a, b) = if essential { (1.0, 1.0) } else { (s[0], s[1]) };
        let d = cols(scale(col(&u, 0), a), scale(col(&u, 1), b), [0.0; 3]);
        mm(&d, &mt(&v))
    };
    let err = |e: &M3, i: usize| {
        let (a, b) = (x1[i], x2[i]);
        let ea = mv(e, a);
        let etb = mv(&mt(e), b);
        let num = dot(b, ea);
        num * num / (ea[0] * ea[0] + ea[1] * ea[1] + etb[0] * etb[0] + etb[1] * etb[1]).max(1e-30)
    };
    let t2 = thresh * thresh;
    let mut seed = 0x9e37_79b9u64;
    let mut best: Option<(usize, M3)> = None;
    for _ in 0..300 {
        let mut idx: Vec<usize> = Vec::with_capacity(8);
        let mut guard = 0;
        while idx.len() < 8 && guard < 200 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let i = (seed % n as u64) as usize;
            if !idx.contains(&i) {
                idx.push(i);
            }
            guard += 1;
        }
        let e = fit(&idx);
        let count = (0..n).filter(|&i| err(&e, i) < t2).count();
        if best.as_ref().is_none_or(|b| count > b.0) {
            best = Some((count, e));
        }
    }
    let (_, e) = best?;
    let idx: Vec<usize> = (0..n).filter(|&i| err(&e, i) < t2).collect();
    let e = if idx.len() >= 8 { fit(&idx) } else { e };
    let inl = (0..n).map(|i| err(&e, i) < t2).collect();
    Some((e, inl))
}

fn self_calibrate(fs: &[M3], w: f64, c: [f64; 2]) -> Option<f64> {
    if fs.is_empty() {
        return None;
    }
    let cost = |hfov: f64| {
        let f = focal_of(hfov, w);
        let k = [[f, 0.0, c[0]], [0.0, f, c[1]], [0.0, 0.0, 1.0]];
        fs.iter().map(|fm| {
            let e = mm(&mt(&k), &mm(fm, &k));
            let (_, s, _) = svd3(&e);
            (s[0] - s[1]) / (s[0] + s[1]).max(1e-30)
        }).sum::<f64>()
    };
    let grid: Vec<(f64, f64)> = (0..=84).map(|i| 12.0 + i as f64 * 1.5).map(|h| (h, cost(h))).collect();
    let (best_h, best_c) = grid.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1))?;
    let worst = grid.iter().map(|g| g.1).fold(0.0f64, f64::max);
    if worst - best_c < 0.02 * fs.len() as f64 {
        return None;
    }
    let (mut lo, mut hi) = ((best_h - 1.5).max(5.0), (best_h + 1.5).min(160.0));
    for _ in 0..30 {
        let m1 = lo + (hi - lo) / 3.0;
        let m2 = hi - (hi - lo) / 3.0;
        if cost(m1) < cost(m2) {
            hi = m2;
        } else {
            lo = m1;
        }
    }
    Some(focal_of((lo + hi) * 0.5, w))
}

fn pick_pose(cands: &[(M3, V3)], pairs: &[(usize, [f64; 2], [f64; 2])], f: f64, c: [f64; 2]) -> Option<(Cam, usize)> {
    let a = Cam { r: I3, c: [0.0; 3], f };
    let mut best: Option<(Cam, usize)> = None;
    for (r, t) in cands {
        if len(*t) < 1e-9 {
            continue;
        }
        let b = cam_from(*r, unit(*t), f);
        let good = pairs.iter().filter(|p| {
            let Some(x) = triangulate(&[(&a, p.1), (&b, p.2)], c) else { return false };
            a.depth(x) > 0.0 && b.depth(x) > 0.0
        }).count();
        if best.as_ref().is_none_or(|bb| good > bb.1) {
            best = Some((b, good));
        }
    }
    best
}

fn two_view_refine(b: Cam, pairs: &[(usize, [f64; 2], [f64; 2])], c: [f64; 2], free_f: bool) -> (Cam, f64) {
    let pairs: Vec<_> = pairs.to_vec();
    let base = b;
    let build = move |x: &[f64]| -> (Cam, Cam) {
        let f = if free_f { base.f * x[6].exp() } else { base.f };
        let r = mm(&rodrigues([x[0], x[1], x[2]]), &base.r);
        let cc = unit(add(base.c, [x[3], x[4], x[5]]));
        (Cam { r: I3, c: [0.0; 3], f }, Cam { r, c: cc, f })
    };
    let resid = |x: &[f64]| -> Vec<f64> {
        let (ca, cb) = build(x);
        let mut out = Vec::with_capacity(pairs.len() * 4);
        for p in &pairs {
            let views = [(&ca, p.1), (&cb, p.2)];
            let q = triangulate(&views, c).and_then(|x| Some((proj(&ca, c, x)?, proj(&cb, c, x)?)));
            match q {
                Some((qa, qb)) => out.extend_from_slice(&[qa[0] - p.1[0], qa[1] - p.1[1], qb[0] - p.2[0], qb[1] - p.2[1]]),
                None => out.extend_from_slice(&[20.0; 4]),
            }
        }
        out
    };
    let x0 = vec![0.0; if free_f { 7 } else { 6 }];
    let x = lm(&x0, &resid, 25, 2.0);
    let r = resid(&x);
    let err = r.chunks(2).map(|v| v[0].hypot(v[1])).sum::<f64>() / (r.len() / 2).max(1) as f64;
    (build(&x).1, err)
}

fn refine_pose(cam: &Cam, obs: &[(usize, [f64; 2])], pts: &[Option<V3>], c: [f64; 2], free_f: bool, k: f64) -> Cam {
    let data: Vec<(V3, [f64; 2])> = obs.iter().filter_map(|(ti, p)| Some((pts[*ti]?, *p))).collect();
    if data.len() < 6 {
        return *cam;
    }
    let base = *cam;
    let build = move |x: &[f64]| Cam { r: mm(&rodrigues([x[0], x[1], x[2]]), &base.r), c: add(base.c, [x[3], x[4], x[5]]), f: if free_f { base.f * x[6].exp() } else { base.f } };
    let resid = |x: &[f64]| -> Vec<f64> {
        let cm = build(x);
        data.iter().flat_map(|(xw, p)| match proj(&cm, c, *xw) {
            Some(q) => [q[0] - p[0], q[1] - p[1]],
            None => [50.0, 50.0],
        }).collect()
    };
    let x = lm(&vec![0.0; if free_f { 7 } else { 6 }], &resid, 12, k);
    build(&x)
}

fn refine_point(x: V3, views: &[(&Cam, [f64; 2])], c: [f64; 2], k: f64) -> V3 {
    let resid = |v: &[f64]| -> Vec<f64> {
        let p = [v[0], v[1], v[2]];
        views.iter().flat_map(|(cam, o)| match proj(cam, c, p) {
            Some(q) => [q[0] - o[0], q[1] - o[1]],
            None => [50.0, 50.0],
        }).collect()
    };
    let v = lm(&x, &resid, 8, k);
    [v[0], v[1], v[2]]
}

fn views_of<'a>(tr: &Tracks, ti: usize, cams: &'a [Option<Cam>]) -> Vec<(&'a Cam, [f64; 2])> {
    let t = &tr.tracks[ti];
    t.pts.iter().enumerate().filter_map(|(i, p)| Some((cams.get(t.start + i)?.as_ref()?, [p[0] as f64, p[1] as f64]))).collect()
}

fn max_angle(views: &[(&Cam, [f64; 2])], x: V3) -> f64 {
    let dirs: Vec<V3> = views.iter().map(|(cam, _)| unit(sub(x, cam.c))).collect();
    let mut best: f64 = 1.0;
    for i in 0..dirs.len() {
        for j in i + 1..dirs.len() {
            best = best.min(dot(dirs[i], dirs[j]));
        }
    }
    best.clamp(-1.0, 1.0).acos().to_degrees()
}

fn intersect(tr: &Tracks, cams: &[Option<Cam>], pts: &mut [Option<V3>], keep: &[usize], c: [f64; 2], px: f64, refine: bool) {
    let found: Vec<(usize, Option<V3>)> = keep
        .par_iter()
        .map(|&ti| {
            let views = views_of(tr, ti, cams);
            if views.len() < 2 {
                return (ti, None);
            }
            let x = match (pts[ti], refine) {
                (Some(x), true) => refine_point(x, &views, c, 2.0 * px),
                (Some(x), false) => x,
                (None, _) => {
                    let Some(x) = triangulate(&views, c) else { return (ti, None) };
                    if max_angle(&views, x) < 1.0 {
                        return (ti, None);
                    }
                    refine_point(x, &views, c, 2.0 * px)
                }
            };
            let ok = views.iter().all(|(cam, _)| cam.depth(x) > 0.0) && reproj(&views, c, x).is_some_and(|e| e < 4.0 * px);
            (ti, ok.then_some(x))
        })
        .collect();
    for (ti, x) in found {
        if x.is_some() || refine {
            pts[ti] = x;
        }
    }
}

fn resect_all(cams: &mut [Option<Cam>], obs: &Obs, pts: &[Option<V3>], c: [f64; 2], free_f: bool, px: f64) {
    let next: Vec<Option<Cam>> = cams.par_iter().enumerate().map(|(k, cam)| cam.map(|cm| refine_pose(&cm, &obs.frames[k], pts, c, free_f, 2.0 * px))).collect();
    cams.copy_from_slice(&next);
}

fn mean_error(tr: &Tracks, cams: &[Option<Cam>], pts: &[Option<V3>], keep: &[usize], c: [f64; 2]) -> f64 {
    let (mut s, mut n) = (0.0, 0usize);
    for &ti in keep {
        let Some(x) = pts[ti] else { continue };
        for (cam, p) in views_of(tr, ti, cams) {
            if let Some(q) = proj(cam, c, x) {
                s += (q[0] - p[0]).hypot(q[1] - p[1]);
                n += 1;
            }
        }
    }
    if n == 0 { 0.0 } else { s / n as f64 }
}

fn order_from(a: usize, b: usize, n: usize) -> Vec<usize> {
    let mut out: Vec<usize> = (a + 1..b).collect();
    let (mut lo, mut hi) = (a as isize - 1, b + 1);
    while lo >= 0 || hi < n {
        if hi < n {
            out.push(hi);
            hi += 1;
        }
        if lo >= 0 {
            out.push(lo as usize);
            lo -= 1;
        }
    }
    out
}

fn fill_gaps(cams: &mut [Option<Cam>]) -> Vec<bool> {
    let solved: Vec<bool> = cams.iter().map(|c| c.is_some()).collect();
    let Some(first) = cams.iter().position(|c| c.is_some()) else { return solved };
    let mut last = cams[first];
    for c in cams.iter_mut() {
        match c {
            Some(_) => last = *c,
            None => *c = last,
        }
    }
    solved
}

pub fn solve(ctx: &Ctx, tr: &Tracks, st: &Settings) -> Result<Solve, String> {
    let n = tr.frames;
    let w = tr.frame[2] - tr.frame[0];
    let c = [(tr.frame[0] + tr.frame[2]) * 0.5, (tr.frame[1] + tr.frame[3]) * 0.5];
    let px = tr.px.max(1.0);
    let keep: Vec<usize> = (0..tr.tracks.len()).filter(|&i| !st.deleted.contains(&tr.tracks[i].id)).collect();
    if n < 2 || keep.len() < 12 {
        return Err("not enough tracked features to solve a camera".into());
    }
    ctx.step(0, 4, "Solving camera");
    let step = (n / 14).max(1);
    let mut pairs_scored: Vec<(f64, usize, usize, f64)> = vec![];
    let mut fms: Vec<M3> = vec![];
    for a in (0..n.saturating_sub(1)).step_by(step) {
        let mut b = a + 1;
        let mut gap = 1;
        while b < n {
            let pairs = common(tr, &keep, a, b);
            if pairs.len() < 24 {
                break;
            }
            if let Some((res, _)) = h_residual(&pairs, px) {
                pairs_scored.push(((res / px).min(6.0) * (pairs.len() as f64).sqrt(), a, b, res));
                if res > 1.5 * px && fms.len() < 12
                    && let Some(fm) = fundamental(&pairs, c, w, px)
                {
                    fms.push(fm);
                }
            }
            gap = (gap * 2).min(n);
            b = a + gap;
        }
    }
    if ctx.stopped() {
        return Err("stopped".into());
    }
    let (_, ka, kb, parallax) = pairs_scored.iter().copied().max_by(|x, y| x.0.total_cmp(&y.0)).ok_or("the tracks are too short to solve a camera")?;
    let mut by_gap: Vec<&(f64, usize, usize, f64)> = pairs_scored.iter().collect();
    by_gap.sort_by(|x, y| (y.2 - y.1).cmp(&(x.2 - x.1)));
    let long_hs: Vec<vision::H3> = by_gap.iter().take(12).filter_map(|p| h_residual(&common(tr, &keep, p.1, p.2), px).map(|r| r.1)).collect();
    let tripod_f = |hs: &[vision::H3]| -> f64 {
        let grid: Vec<(f64, f64)> = (0..=60).map(|i| 10.0 + i as f64 * 2.0).map(|h| (h, rotation_cost(hs, focal_of(h, w), c))).collect();
        let h = grid.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1)).map(|g| g.0).unwrap_or(40.0);
        focal_of(h, w)
    };
    let rot_like = |f: f64| rotation_cost(&long_hs, f, c) / long_hs.len().max(1) as f64;
    let tripod = match st.method {
        3 => true,
        1 | 2 => false,
        _ => parallax < 3.0 * px && {
            let f = if st.shot == 2 { focal_of(st.hfov, w) } else { tripod_f(&long_hs) };
            rot_like(f) < 0.03
        },
    };
    let free_f = st.shot != 2;
    if tripod {
        let f = if st.shot == 2 { focal_of(st.hfov, w) } else if long_hs.is_empty() { focal_of(40.0, w) } else { tripod_f(&long_hs) };
        return solve_tripod(ctx, tr, &keep, f, c, px);
    }
    let f0 = if st.shot == 2 { focal_of(st.hfov, w) } else { self_calibrate(&fms, w, c).unwrap_or_else(|| focal_of(40.0, w)) };
    ctx.step(1, 4, "Solving camera");
    let pairs = common(tr, &keep, ka, kb);
    let x1: Vec<V3> = pairs.iter().map(|p| [(p.1[0] - c[0]) / f0, (p.1[1] - c[1]) / f0, 1.0]).collect();
    let x2: Vec<V3> = pairs.iter().map(|p| [(p.2[0] - c[0]) / f0, (p.2[1] - c[1]) / f0, 1.0]).collect();
    let mut inits: Vec<(Cam, f64, &str)> = vec![];
    if st.method != 2
        && let Some((e, inl)) = essential_ransac(&x1, &x2, 1.5 * px / f0)
    {
        let good: Vec<_> = pairs.iter().zip(&inl).filter(|x| *x.1).map(|x| *x.0).collect();
        if let Some((cam, _)) = pick_pose(&poses_from_e(&e), &good, f0, c) {
            let (cam, err) = two_view_refine(cam, &good, c, free_f);
            inits.push((cam, err, "Typical"));
        }
    }
    if st.method != 1 {
        let s: Vec<[f64; 2]> = x1.iter().map(|v| [v[0], v[1]]).collect();
        let d: Vec<[f64; 2]> = x2.iter().map(|v| [v[0], v[1]]).collect();
        if let Some((h, inl)) = vision::ransac(Model::Homography, &s, &d, 1.5 * px / f0, 300) {
            let good: Vec<_> = pairs.iter().zip(&inl).filter(|x| *x.1).map(|x| *x.0).collect();
            if let Some((cam, _)) = pick_pose(&poses_from_h(&h), &good, f0, c) {
                let (cam, err) = two_view_refine(cam, &good, c, free_f);
                inits.push((cam, err, "Mostly Flat Scene"));
            }
        }
    }
    let (cb, _, method) = inits.into_iter().min_by(|a, b| a.1.total_cmp(&b.1)).ok_or("could not find a starting camera pair (try Tripod Pan or Specify Angle of View)")?;
    let f = cb.f;
    let obs = observations(tr, &keep);
    let mut cams: Vec<Option<Cam>> = vec![None; n];
    cams[ka] = Some(Cam { r: I3, c: [0.0; 3], f });
    cams[kb] = Some(cb);
    let mut pts: Vec<Option<V3>> = vec![None; tr.tracks.len()];
    intersect(tr, &cams, &mut pts, &keep, c, px, false);
    let order = order_from(ka, kb, n);
    let total = order.len().max(1);
    for (i, &k) in order.iter().enumerate() {
        if ctx.stopped() {
            return Err("stopped".into());
        }
        ctx.step(i, total, "Solving camera");
        let near = (1..n).find_map(|d| [k.checked_sub(d), Some(k + d)].into_iter().flatten().find_map(|j| cams.get(j).copied().flatten())).ok_or("lost the camera")?;
        let have = obs.frames[k].iter().filter(|(ti, _)| pts[*ti].is_some()).count();
        if have < 6 {
            continue;
        }
        cams[k] = Some(refine_pose(&near, &obs.frames[k], &pts, c, st.shot == 1, 2.0 * px));
        let fresh: Vec<usize> = obs.frames[k].iter().filter(|(ti, _)| pts[*ti].is_none()).map(|(ti, _)| *ti).collect();
        intersect(tr, &cams, &mut pts, &fresh, c, px, false);
        if i % 12 == 11 {
            intersect(tr, &cams, &mut pts, &keep, c, px, true);
        }
    }
    ctx.step(total, total, "Refining camera");
    for round in 0..4 {
        if ctx.stopped() {
            return Err("stopped".into());
        }
        resect_all(&mut cams, &obs, &pts, c, st.shot == 1, px);
        intersect(tr, &cams, &mut pts, &keep, c, px, true);
        if round == 1 {
            let fresh: Vec<usize> = keep.iter().copied().filter(|ti| pts[*ti].is_none()).collect();
            intersect(tr, &cams, &mut pts, &fresh, c, px, false);
        }
    }
    let mut error = mean_error(tr, &cams, &pts, &keep, c);
    if st.shot == 0 {
        ctx.step(total, total, "Refining angle of view");
        let trial = |k: f64| -> (f64, Vec<Option<Cam>>, Vec<Option<V3>>) {
            let mut cs = cams.clone();
            for cm in cs.iter_mut().flatten() {
                cm.f *= k;
            }
            let mut ps = pts.clone();
            resect_all(&mut cs, &obs, &ps, c, false, px);
            intersect(tr, &cs, &mut ps, &keep, c, px, true);
            resect_all(&mut cs, &obs, &ps, c, false, px);
            (mean_error(tr, &cs, &ps, &keep, c), cs, ps)
        };
        let (mut lo, mut hi) = (-0.2f64, 0.2f64);
        for _ in 0..9 {
            if ctx.stopped() {
                return Err("stopped".into());
            }
            let m1 = lo + (hi - lo) * 0.382;
            let m2 = lo + (hi - lo) * 0.618;
            if trial(m1.exp()).0 < trial(m2.exp()).0 {
                hi = m2;
            } else {
                lo = m1;
            }
        }
        let (e2, cs, ps) = trial(((lo + hi) * 0.5).exp());
        let kept_before = pts.iter().filter(|p| p.is_some()).count();
        if e2 < error && ps.iter().filter(|p| p.is_some()).count() * 10 >= kept_before * 9 {
            error = e2;
            cams = cs;
            pts = ps;
        }
    }
    let solved = fill_gaps(&mut cams);
    let all: Vec<Option<Cam>> = cams.iter().map(|c| Some(c.unwrap_or(Cam { r: I3, c: [0.0; 3], f }))).collect();
    let cams: Vec<Cam> = all.iter().flatten().copied().collect();
    let points = keep.iter().filter_map(|&ti| {
        let x = pts[ti]?;
        let views = views_of(tr, ti, &all);
        let e = views.iter().filter_map(|(cam, p)| proj(cam, c, x).map(|q| (q[0] - p[0]).hypot(q[1] - p[1]))).sum::<f64>() / views.len().max(1) as f64;
        Some(Point { id: tr.tracks[ti].id, x, err: e })
    }).collect::<Vec<_>>();
    if points.len() < 8 {
        return Err("the camera solve failed (too few points stayed consistent); try another Solve Method or Specify Angle of View".into());
    }
    Ok(Solve { frame: tr.frame, t0: tr.t0, fps: tr.fps, cams, solved, points, method: method.to_string(), error })
}

fn solve_tripod(ctx: &Ctx, tr: &Tracks, keep: &[usize], f: f64, c: [f64; 2], px: f64) -> Result<Solve, String> {
    let n = tr.frames;
    let mut rots: Vec<Option<M3>> = vec![None; n];
    rots[0] = Some(I3);
    for k in 1..n {
        if ctx.stopped() {
            return Err("stopped".into());
        }
        ctx.step(k, n - 1, "Solving camera");
        let prev = (0..k).rev().find(|j| rots[*j].is_some()).unwrap_or(0);
        let Some(rp) = rots[prev] else { continue };
        let pairs = common(tr, keep, prev, k);
        if pairs.len() < 4 {
            rots[k] = Some(rp);
            continue;
        }
        let a: Vec<V3> = pairs.iter().map(|p| bearing(p.1, c, f)).collect();
        let b: Vec<V3> = pairs.iter().map(|p| bearing(p.2, c, f)).collect();
        let mut rel = kabsch(&a, &b);
        for _ in 0..2 {
            let errs: Vec<f64> = a.iter().zip(&b).map(|(x, y)| len(sub(mv(&rel, *x), *y))).collect();
            let mut sorted = errs.clone();
            sorted.sort_by(|x, y| x.total_cmp(y));
            let cut = (sorted[sorted.len() / 2] * 3.0).max(1.5 * px / f);
            let (aa, bb): (Vec<V3>, Vec<V3>) = a.iter().zip(&b).zip(&errs).filter(|x| *x.1 <= cut).map(|x| (*x.0.0, *x.0.1)).unzip();
            if aa.len() >= 4 {
                rel = kabsch(&aa, &bb);
            }
        }
        rots[k] = Some(mm(&rel, &rp));
    }
    let cams: Vec<Cam> = rots.iter().map(|r| Cam { r: r.unwrap_or(I3), c: [0.0; 3], f }).collect();
    let opt: Vec<Option<Cam>> = cams.iter().copied().map(Some).collect();
    let mut points = vec![];
    for &ti in keep {
        let views = views_of(tr, ti, &opt);
        if views.is_empty() {
            continue;
        }
        let mut d = [0.0; 3];
        for (cam, p) in &views {
            d = add(d, mv(&mt(&cam.r), bearing(*p, c, f)));
        }
        let x = scale(unit(d), 10.0);
        let e = views.iter().filter_map(|(cam, p)| proj(cam, c, x).map(|q| (q[0] - p[0]).hypot(q[1] - p[1]))).sum::<f64>() / views.len() as f64;
        points.push(Point { id: tr.tracks[ti].id, x, err: e });
    }
    if points.len() < 8 {
        return Err("too few features stayed tracked to solve a tripod pan".into());
    }
    let error = points.iter().map(|p| p.err).sum::<f64>() / points.len() as f64;
    Ok(Solve { frame: tr.frame, t0: tr.t0, fps: tr.fps, solved: vec![true; n], cams, points, method: "Tripod Pan".into(), error })
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Ground {
    pub r: M3,
    pub t: V3,
}

impl Default for Ground {
    fn default() -> Self {
        Ground { r: I3, t: [0.0; 3] }
    }
}

pub struct Placed {
    pub s: f64,
    pub r0: M3,
    pub c0: V3,
    pub o: V3,
    pub sl: f64,
    pub ground: Ground,
}

impl Placed {
    pub fn new(sv: &Solve, comp_w: f64, comp_h: f64, sl: f64, ground: Ground) -> Placed {
        let k0 = sv.solved.iter().position(|b| *b).unwrap_or(0);
        let cam = sv.cams.get(k0).copied().unwrap_or(Cam { r: I3, c: [0.0; 3], f: 1000.0 });
        let mut depths: Vec<f64> = sv.points.iter().map(|p| cam.depth(p.x)).filter(|d| *d > 0.0).collect();
        depths.sort_by(|a, b| a.total_cmp(b));
        let dmed = depths.get(depths.len() / 2).copied().unwrap_or(1.0).max(1e-9);
        let zoom = cam.f * sl;
        Placed { s: zoom / dmed, r0: cam.r, c0: cam.c, o: [comp_w * 0.5, comp_h * 0.5, -zoom], sl, ground }
    }

    pub fn base(&self, x: V3) -> V3 {
        add(scale(mv(&self.r0, sub(x, self.c0)), self.s), self.o)
    }

    pub fn world(&self, x: V3) -> V3 {
        add(mv(&self.ground.r, self.base(x)), self.ground.t)
    }

    pub fn camera(&self, cam: &Cam) -> (V3, M3, f64) {
        let pos = self.world(cam.c);
        let rcw = mm(&self.ground.r, &mm(&self.r0, &mt(&cam.r)));
        (pos, rcw, cam.f * self.sl)
    }
}

pub fn plane_fit(pts: &[V3], toward: V3) -> (V3, V3) {
    let n = pts.len().max(1) as f64;
    let c = scale(pts.iter().fold([0.0; 3], |a, p| add(a, *p)), 1.0 / n);
    let normal = if pts.len() >= 3 {
        let mut cov = vec![vec![0.0; 3]; 3];
        for p in pts {
            let d = sub(*p, c);
            for i in 0..3 {
                for j in 0..3 {
                    cov[i][j] += d[i] * d[j];
                }
            }
        }
        let (_, v) = sym_eig(cov);
        [v[0][0], v[0][1], v[0][2]]
    } else {
        unit(sub(toward, c))
    };
    let normal = if dot(normal, sub(toward, c)) < 0.0 { scale(normal, -1.0) } else { normal };
    (c, unit(normal))
}

pub fn plane_basis(normal: V3, right: V3) -> (V3, V3, V3) {
    let x = unit(sub(right, scale(normal, dot(right, normal))));
    let x = if len(sub(right, scale(normal, dot(right, normal)))) < 1e-9 { unit(cross([0.0, 1.0, 0.0], normal)) } else { x };
    let z = scale(normal, -1.0);
    let y = cross(z, x);
    (x, y, z)
}

pub fn ground_from(c: V3, normal_up: V3, right: V3) -> Ground {
    let ey = scale(normal_up, -1.0);
    let ex = {
        let r = sub(right, scale(ey, dot(right, ey)));
        if len(r) < 1e-9 { unit(cross(ey, [0.0, 0.0, 1.0])) } else { unit(r) }
    };
    let ez = cross(ex, ey);
    let r = [ex, ey, ez];
    Ground { r, t: scale(mv(&r, c), -1.0) }
}

pub fn color_of(id: u32) -> [f32; 3] {
    let h = ((id as f64 * 0.618_033_988_75) % 1.0) * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    [r as f32 * 0.85 + 0.15, g as f32 * 0.85 + 0.15, b as f32 * 0.85 + 0.15]
}
