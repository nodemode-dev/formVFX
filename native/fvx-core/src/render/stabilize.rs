use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use rayon::prelude::*;

use super::analysis::Source;
use super::image::Img;
use super::vision::{self, H3, Model};
use crate::effects::FxCtx;
use crate::engine::jobs::Ctx;

pub const GRID: usize = 6;

#[derive(Clone, Debug, Default)]
pub struct WarpData {
    pub fps: f64,
    pub t0: f64,
    pub frame: [f64; 4],
    pub motion: Vec<H3>,
    pub residual: Vec<Vec<[f32; 2]>>,
}

impl WarpData {
    pub fn encode(&self) -> String {
        let mut s = format!("w1;{};{};{};{};{};{};{}", self.fps, self.t0, self.frame[0], self.frame[1], self.frame[2], self.frame[3], self.motion.len());
        for (i, h) in self.motion.iter().enumerate() {
            s.push(';');
            let v: Vec<String> = h.iter().flatten().take(8).map(|x| format!("{x:.6}")).collect();
            s.push_str(&v.join(","));
            s.push('|');
            let r: Vec<String> = self.residual.get(i).map(|g| g.iter().map(|p| format!("{:.2} {:.2}", p[0], p[1])).collect()).unwrap_or_default();
            s.push_str(&r.join(","));
        }
        s
    }

    pub fn decode(s: &str) -> Option<WarpData> {
        let mut it = s.split(';');
        if it.next()? != "w1" {
            return None;
        }
        let mut num = || it.next().and_then(|x| x.parse::<f64>().ok());
        let fps = num()?;
        let t0 = num()?;
        let frame = [num()?, num()?, num()?, num()?];
        let n = num()? as usize;
        let mut motion = Vec::with_capacity(n);
        let mut residual = Vec::with_capacity(n);
        for part in it.take(n) {
            let (h, r) = part.split_once('|').unwrap_or((part, ""));
            let v: Vec<f64> = h.split(',').filter_map(|x| x.parse().ok()).collect();
            if v.len() != 8 {
                return None;
            }
            motion.push([[v[0], v[1], v[2]], [v[3], v[4], v[5]], [v[6], v[7], 1.0]]);
            residual.push(r.split(',').filter(|x| !x.is_empty()).filter_map(|p| {
                let (a, b) = p.split_once(' ')?;
                Some([a.parse().ok()?, b.parse().ok()?])
            }).collect());
        }
        Some(WarpData { fps, t0, frame, motion, residual })
    }
}

fn cache() -> &'static Mutex<HashMap<u64, Arc<WarpData>>> {
    static C: OnceLock<Mutex<HashMap<u64, Arc<WarpData>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

pub fn parsed(s: &str) -> Option<Arc<WarpData>> {
    if s.is_empty() {
        return None;
    }
    let k = hash(s);
    if let Some(d) = cache().lock().get(&k) {
        return Some(d.clone());
    }
    let d = Arc::new(WarpData::decode(s)?);
    let mut c = cache().lock();
    if c.len() > 16 {
        c.clear();
    }
    c.insert(k, d.clone());
    Some(d)
}

pub fn analyze(ctx: &Ctx, src: &Source, times: &[f64], layer_times: &[f64], fps: f64, detailed: bool) -> Result<WarpData, String> {
    let g0 = src.gray(times[0], 1, 0.0).ok_or("cannot read the layer's frames")?;
    let (w, h) = (g0.w as f32, g0.h as f32);
    let max = if detailed { 900 } else { 450 };
    let md = (w.max(h) / 45.0).max(5.0);
    let mut feats = vision::corners(&g0, &|_, _| true, max, md);
    if feats.len() < 8 {
        return Err("the footage has too little detail to stabilize".into());
    }
    let mut prev = g0.pyramid(4);
    let k = src.scale;
    let f = src.frame;
    let to_layer = [[1.0 / k, 0.0, f[0]], [0.0, 1.0 / k, f[1]], [0.0, 0.0, 1.0]];
    let to_buf = [[k, 0.0, -f[0] * k], [0.0, k, -f[1] * k], [0.0, 0.0, 1.0]];
    let mut cum = vision::IDENTITY;
    let mut motion = vec![vision::IDENTITY];
    let mut residual = vec![vec![[0.0f32; 2]; GRID * GRID]];
    for (fi, t) in times.iter().enumerate().skip(1) {
        if ctx.stopped() {
            return Err("stopped".into());
        }
        ctx.step(fi, times.len() - 1, "Analyzing");
        let g = src.gray(*t, 1, 0.0).ok_or("cannot read the layer's frames")?;
        let pyr = g.pyramid(4);
        let moved = vision::klt(&prev, &pyr, &feats, None, 7);
        let (mut s, mut d) = (vec![], vec![]);
        for (i, m) in moved.iter().enumerate() {
            if let Some(q) = m {
                s.push([feats[i][0] as f64, feats[i][1] as f64]);
                d.push([q[0] as f64, q[1] as f64]);
            }
        }
        let step = vision::ransac(Model::Homography, &s, &d, 1.5, 400).map(|r| r.0).or_else(|| vision::ransac(Model::Similarity, &s, &d, 2.0, 200).map(|r| r.0)).unwrap_or(vision::IDENTITY);
        cum = vision::h_mul(&step, &cum);
        let mut grid = vec![[0.0f32; 2]; GRID * GRID];
        let mut wsum = vec![0.0f32; GRID * GRID];
        for (a, b) in s.iter().zip(&d) {
            let pr = vision::h_apply(&step, *a);
            let r = [(b[0] - pr[0]) as f32, (b[1] - pr[1]) as f32];
            if r[0].hypot(r[1]) > 6.0 {
                continue;
            }
            for gy in 0..GRID {
                for gx in 0..GRID {
                    let c = [(gx as f32 + 0.5) / GRID as f32 * w, (gy as f32 + 0.5) / GRID as f32 * h];
                    let dd = ((b[0] as f32 - c[0]).powi(2) + (b[1] as f32 - c[1]).powi(2)) / (w * w / (GRID * GRID) as f32);
                    let wt = 1.0 / (1.0 + dd * 4.0);
                    let gi = gy * GRID + gx;
                    grid[gi][0] += r[0] * wt;
                    grid[gi][1] += r[1] * wt;
                    wsum[gi] += wt;
                }
            }
        }
        let prev_res = residual.last().cloned().unwrap_or_default();
        let res: Vec<[f32; 2]> = grid.iter().zip(&wsum).zip(&prev_res).map(|((g, w), p)| if *w > 1e-6 { [p[0] + g[0] / w / k as f32, p[1] + g[1] / w / k as f32] } else { *p }).collect();
        residual.push(res);
        motion.push(vision::h_mul(&to_layer, &vision::h_mul(&cum, &to_buf)));
        feats = d.iter().map(|q| [q[0] as f32, q[1] as f32]).collect();
        if feats.len() < max * 6 / 10 {
            let existing = feats.clone();
            let more = vision::corners(&g, &|x, y| existing.iter().all(|q| (q[0] - x).powi(2) + (q[1] - y).powi(2) > md * md), max - feats.len(), md);
            feats.extend(more);
        }
        prev = pyr;
    }
    Ok(WarpData { fps, t0: layer_times[0], frame: f, motion, residual })
}

fn gauss(v: &[f64], sigma: f64) -> Vec<f64> {
    if sigma < 0.3 || v.len() < 3 {
        return v.to_vec();
    }
    let r = (sigma * 3.0).ceil() as isize;
    let n = v.len() as isize;
    (0..n)
        .map(|i| {
            let mut acc = 0.0;
            let mut ws = 0.0;
            for k in -r..=r {
                let j = (i + k).clamp(0, n - 1) as usize;
                let w = (-(k * k) as f64 / (2.0 * sigma * sigma)).exp();
                acc += v[j] * w;
                ws += w;
            }
            acc / ws
        })
        .collect()
}

pub struct Plan {
    pub corrections: Vec<H3>,
    pub residual: Vec<Vec<[f32; 2]>>,
    pub scale: f64,
}

fn corners(f: [f64; 4]) -> [[f64; 2]; 5] {
    [[f[0], f[1]], [f[2], f[1]], [f[0], f[3]], [f[2], f[3]], [(f[0] + f[2]) * 0.5, (f[1] + f[3]) * 0.5]]
}

pub fn plan(d: &WarpData, result: u32, smooth: f64, method: u32, preserve_scale: bool) -> Plan {
    let n = d.motion.len();
    let q = corners(d.frame);
    let sigma = if result == 1 { 0.0 } else { smooth / 100.0 * d.fps.max(1.0) };
    let still = result == 1;
    let pick = |series: &[f64]| -> Vec<f64> { if still { vec![series[0]; series.len()] } else { gauss(series, sigma) } };
    let mut corrections = vec![vision::IDENTITY; n];
    match method {
        0 => {
            let cx: Vec<f64> = d.motion.iter().map(|h| vision::h_apply(h, q[4])[0]).collect();
            let cy: Vec<f64> = d.motion.iter().map(|h| vision::h_apply(h, q[4])[1]).collect();
            let (sx, sy) = (pick(&cx), pick(&cy));
            for i in 0..n {
                corrections[i] = [[1.0, 0.0, sx[i] - cx[i]], [0.0, 1.0, sy[i] - cy[i]], [0.0, 0.0, 1.0]];
            }
        }
        1 => {
            let src: Vec<[f64; 2]> = q.to_vec();
            let sims: Vec<H3> = d.motion.iter().map(|h| {
                let dst: Vec<[f64; 2]> = q.iter().map(|p| vision::h_apply(h, *p)).collect();
                vision::fit(Model::Similarity, &src, &dst).unwrap_or(vision::IDENTITY)
            }).collect();
            let tx: Vec<f64> = sims.iter().map(|s| s[0][2]).collect();
            let ty: Vec<f64> = sims.iter().map(|s| s[1][2]).collect();
            let an: Vec<f64> = sims.iter().map(|s| s[1][0].atan2(s[0][0])).collect();
            let ls: Vec<f64> = sims.iter().map(|s| (s[0][0].hypot(s[1][0])).max(1e-9).ln()).collect();
            let (stx, sty, san, sls) = (pick(&tx), pick(&ty), pick(&an), if preserve_scale { ls.clone() } else { pick(&ls) });
            for i in 0..n {
                let s = sls[i].exp();
                let (sn, cs) = san[i].sin_cos();
                let target = [[s * cs, -s * sn, stx[i]], [s * sn, s * cs, sty[i]], [0.0, 0.0, 1.0]];
                corrections[i] = vision::h_mul(&target, &vision::h_inv(&sims[i]).unwrap_or(vision::IDENTITY));
            }
        }
        _ => {
            let pts: Vec<Vec<[f64; 2]>> = d.motion.iter().map(|h| q[..4].iter().map(|p| vision::h_apply(h, *p)).collect()).collect();
            let mut smoothed: Vec<Vec<[f64; 2]>> = vec![vec![[0.0; 2]; 4]; n];
            for k in 0..4 {
                for c in 0..2 {
                    let series: Vec<f64> = pts.iter().map(|p| p[k][c]).collect();
                    let s = pick(&series);
                    for i in 0..n {
                        smoothed[i][k][c] = s[i];
                    }
                }
            }
            if preserve_scale {
                for i in 0..n {
                    let area = |p: &[[f64; 2]]| ((p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1])).abs().max(1e-9);
                    let k = (area(&pts[i]) / area(&smoothed[i])).sqrt();
                    let c = [smoothed[i].iter().map(|p| p[0]).sum::<f64>() / 4.0, smoothed[i].iter().map(|p| p[1]).sum::<f64>() / 4.0];
                    for p in &mut smoothed[i] {
                        *p = [c[0] + (p[0] - c[0]) * k, c[1] + (p[1] - c[1]) * k];
                    }
                }
            }
            for i in 0..n {
                let target = vision::fit(Model::Homography, &q[..4], &smoothed[i]).unwrap_or(vision::IDENTITY);
                corrections[i] = vision::h_mul(&target, &vision::h_inv(&d.motion[i]).unwrap_or(vision::IDENTITY));
            }
        }
    }
    let residual: Vec<Vec<[f32; 2]>> = if method == 3 {
        let gn = d.residual.first().map(|g| g.len()).unwrap_or(0);
        let mut out = vec![vec![[0.0f32; 2]; gn]; n];
        for gi in 0..gn {
            for c in 0..2 {
                let series: Vec<f64> = d.residual.iter().map(|g| g.get(gi).map(|p| p[c] as f64).unwrap_or(0.0)).collect();
                let s = pick(&series);
                for i in 0..n {
                    out[i][gi][c] = (series[i] - s[i]) as f32;
                }
            }
        }
        out
    } else {
        vec![]
    };
    let scale = auto_scale(d.frame, &corrections);
    Plan { corrections, residual, scale }
}

fn inside(quad: &[[f64; 2]; 4], p: [f64; 2]) -> bool {
    let order = [0, 1, 3, 2];
    let mut sign = 0.0;
    for k in 0..4 {
        let a = quad[order[k]];
        let b = quad[order[(k + 1) % 4]];
        let c = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        if c.abs() < 1e-9 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

fn auto_scale(f: [f64; 4], corrections: &[H3]) -> f64 {
    let q = corners(f);
    let c = q[4];
    let mut need: f64 = 1.0;
    for h in corrections {
        let quad = [vision::h_apply(h, q[0]), vision::h_apply(h, q[1]), vision::h_apply(h, q[2]), vision::h_apply(h, q[3])];
        let (mut lo, mut hi) = (1.0f64, 4.0f64);
        let ok = |s: f64| q[..4].iter().all(|p| inside(&quad, [c[0] + (p[0] - c[0]) / s, c[1] + (p[1] - c[1]) / s]));
        if ok(1.0) {
            continue;
        }
        if !ok(hi) {
            need = need.max(hi);
            continue;
        }
        for _ in 0..30 {
            let m = (lo + hi) * 0.5;
            if ok(m) {
                hi = m;
            } else {
                lo = m;
            }
        }
        need = need.max(hi);
    }
    need
}

fn residual_at(grid: &[[f32; 2]], f: [f64; 4], p: [f64; 2]) -> [f64; 2] {
    if grid.len() != GRID * GRID {
        return [0.0; 2];
    }
    let u = ((p[0] - f[0]) / (f[2] - f[0]).max(1e-9) * GRID as f64 - 0.5).clamp(0.0, (GRID - 1) as f64);
    let v = ((p[1] - f[1]) / (f[3] - f[1]).max(1e-9) * GRID as f64 - 0.5).clamp(0.0, (GRID - 1) as f64);
    let (x0, y0) = (u.floor() as usize, v.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(GRID - 1), (y0 + 1).min(GRID - 1));
    let (fx, fy) = ((u - x0 as f64) as f32, (v - y0 as f64) as f32);
    let g = |x: usize, y: usize| grid[y * GRID + x];
    let a = [g(x0, y0)[0] + (g(x1, y0)[0] - g(x0, y0)[0]) * fx, g(x0, y0)[1] + (g(x1, y0)[1] - g(x0, y0)[1]) * fx];
    let b = [g(x0, y1)[0] + (g(x1, y1)[0] - g(x0, y1)[0]) * fx, g(x0, y1)[1] + (g(x1, y1)[1] - g(x0, y1)[1]) * fx];
    [(a[0] + (b[0] - a[0]) * fy) as f64, (a[1] + (b[1] - a[1]) * fy) as f64]
}

fn warp_into(out: &mut Img, src: &Img, c: &FxCtx, corr: &H3, res: Option<&[[f32; 2]]>, frame: [f64; 4], scale: f64, only_holes: bool) {
    let Some(inv) = vision::h_inv(corr) else { return };
    let w = out.w;
    let r = c.r;
    let center = [(frame[0] + frame[2]) * 0.5, (frame[1] + frame[3]) * 0.5];
    out.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let px = &mut row[x * 4..x * 4 + 4];
            if only_holes && px[3] >= 0.999 {
                continue;
            }
            let lp = [(x as f64 + 0.5) / r + c.origin[0] + frame[0], (y as f64 + 0.5) / r + c.origin[1] + frame[1]];
            let lp = [center[0] + (lp[0] - center[0]) / scale, center[1] + (lp[1] - center[1]) / scale];
            let mut sp = vision::h_apply(&inv, lp);
            if let Some(g) = res {
                let d = residual_at(g, frame, sp);
                sp = [sp[0] + d[0], sp[1] + d[1]];
            }
            let s = src.sample((sp[0] - frame[0] - c.origin[0]) * r, (sp[1] - frame[1] - c.origin[1]) * r, false);
            if only_holes {
                let k = 1.0 - px[3];
                for i in 0..4 {
                    px[i] += s[i] * k;
                }
            } else {
                px.copy_from_slice(&s);
            }
        }
    });
}

pub fn render(c: &FxCtx, img: &mut Img) {
    let Some(d) = parsed(&c.s("analysis")) else { return };
    if d.motion.is_empty() {
        return;
    }
    let p = plan(&d, c.e("result"), c.f("smoothness"), c.e("method"), c.b("preserveScale"));
    let idx = |lt: f64| (((lt - d.t0) * d.fps).round().max(0.0) as usize).min(d.motion.len() - 1);
    let i = idx(c.t);
    let framing = c.e("framing");
    let mut scale = c.f("additionalScale") / 100.0;
    let margin = 1.0 + c.f("actionSafe") / 100.0;
    if framing == 2 {
        scale *= (p.scale * margin).min(c.f("maxScale") / 100.0).max(1.0);
    }
    let src = img.clone();
    let mut out = Img::new(img.w, img.h);
    warp_into(&mut out, &src, c, &p.corrections[i], p.residual.get(i).map(|v| v.as_slice()), d.frame, scale, false);
    if framing == 1 {
        let k = p.scale * margin;
        let center = [(d.frame[0] + d.frame[2]) * 0.5, (d.frame[1] + d.frame[3]) * 0.5];
        let half = [(d.frame[2] - d.frame[0]) * 0.5 / k, (d.frame[3] - d.frame[1]) * 0.5 / k];
        let w = out.w;
        let r = c.r;
        out.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                let lp = [(x as f64 + 0.5) / r + c.origin[0] + d.frame[0], (y as f64 + 0.5) / r + c.origin[1] + d.frame[1]];
                if (lp[0] - center[0]).abs() > half[0] || (lp[1] - center[1]).abs() > half[1] {
                    row[x * 4..x * 4 + 4].copy_from_slice(&[0.0; 4]);
                }
            }
        });
    }
    if framing == 3 {
        let span = (c.f("synthRange").max(0.0) * d.fps).round().clamp(1.0, 8.0) as i64;
        for k in 1..=span {
            for sgn in [-1i64, 1] {
                let j = i as i64 + sgn * k;
                if j < 0 || j as usize >= d.motion.len() {
                    continue;
                }
                let dt = (j - i as i64) as f64 / d.fps;
                let Some(nb) = c.frame(dt) else { continue };
                if nb.w != out.w || nb.h != out.h {
                    continue;
                }
                warp_into(&mut out, &nb, c, &p.corrections[j as usize], p.residual.get(j as usize).map(|v| v.as_slice()), d.frame, scale, true);
            }
        }
        let feather = c.f("synthFeather");
        if feather > 0.3 {
            let mut a: Vec<f32> = out.px.chunks_exact(4).map(|p| p[3]).collect();
            crate::effects::util::blur_scalar(&mut a, out.w, out.h, c.px(feather) * 0.5, c.px(feather) * 0.5);
            for (p, b) in out.px.chunks_exact_mut(4).zip(&a) {
                if *b < p[3] && p[3] > 1e-6 {
                    let k = b / p[3];
                    for v in p.iter_mut() {
                        *v *= k;
                    }
                }
            }
        }
    }
    *img = out;
}
