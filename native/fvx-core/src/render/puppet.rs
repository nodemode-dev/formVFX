use rayon::prelude::*;

use super::image::Img;

pub const KINDS: [&str; 5] = ["position", "advanced", "bend", "starch", "overlap"];

#[derive(Clone, Debug)]
pub struct Mesh {
    pub rest: Vec<[f64; 2]>,
    pub tris: Vec<[usize; 3]>,
    pub nbrs: Vec<Vec<usize>>,
    pub spacing: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Pin {
    pub kind: usize,
    pub rest: [f64; 2],
    pub pos: [f64; 2],
    pub scale: f64,
    pub rotation: f64,
    pub amount: f64,
    pub extent: f64,
    pub in_front: f64,
}

pub fn build(alpha: &(dyn Fn(f64, f64) -> f32 + Sync), bounds: [f64; 4], px: f64, triangles: f64, density: f64, expansion: f64, seed: Option<[f64; 2]>) -> Option<Mesh> {
    let (x0, y0, x1, y1) = (bounds[0], bounds[1], bounds[2], bounds[3]);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let fine = px.max(0.5);
    let (fw, fh) = (((x1 - x0) / fine).ceil() as usize + 1, ((y1 - y0) / fine).ceil() as usize + 1);
    if fw * fh > 16_000_000 {
        return None;
    }
    let mut cov: Vec<bool> = (0..fw * fh).into_par_iter().map(|i| alpha(x0 + (i % fw) as f64 * fine, y0 + (i / fw) as f64 * fine) > 0.02).collect();
    let grow = (expansion / fine).round().max(0.0) as usize;
    if grow > 0 {
        let src = cov.clone();
        let rows: Vec<Vec<bool>> = (0..fh)
            .into_par_iter()
            .map(|y| {
                (0..fw)
                    .map(|x| {
                        let (ya, yb) = (y.saturating_sub(grow), (y + grow).min(fh - 1));
                        let (xa, xb) = (x.saturating_sub(grow), (x + grow).min(fw - 1));
                        (ya..=yb).any(|yy| (xa..=xb).any(|xx| src[yy * fw + xx] && ((xx as f64 - x as f64).powi(2) + (yy as f64 - y as f64).powi(2)) <= (grow * grow) as f64))
                    })
                    .collect()
            })
            .collect();
        cov = rows.into_iter().flatten().collect();
    }
    let area = cov.iter().filter(|b| **b).count() as f64 * fine * fine;
    if area < 1.0 {
        return None;
    }
    let cells = (triangles * density / 10.0 / 2.0).clamp(4.0, 20_000.0);
    let s = (area / cells).sqrt().max(fine * 2.0);
    let (gw, gh) = (((x1 - x0) / s).ceil() as usize + 1, ((y1 - y0) / s).ceil() as usize + 1);
    let per = (s / fine).ceil() as usize;
    let mut used = vec![false; gw * gh];
    for gy in 0..gh {
        for gx in 0..gw {
            let (fx0, fy0) = ((gx as f64 * s / fine) as usize, (gy as f64 * s / fine) as usize);
            let hit = (fy0..(fy0 + per).min(fh)).any(|y| (fx0..(fx0 + per).min(fw)).any(|x| cov[y * fw + x]));
            used[gy * gw + gx] = hit;
        }
    }
    if let Some(sp) = seed {
        let sx = (((sp[0] - x0) / s).floor().max(0.0) as usize).min(gw - 1);
        let sy = (((sp[1] - y0) / s).floor().max(0.0) as usize).min(gh - 1);
        let start = if used[sy * gw + sx] {
            Some(sy * gw + sx)
        } else {
            (0..gw * gh).filter(|i| used[*i]).min_by_key(|i| {
                let (x, y) = ((i % gw) as i64, (i / gw) as i64);
                (x - sx as i64).pow(2) + (y - sy as i64).pow(2)
            })
        };
        if let Some(st) = start {
            let mut keep = vec![false; gw * gh];
            let mut stack = vec![st];
            keep[st] = true;
            while let Some(i) = stack.pop() {
                let (x, y) = (i % gw, i / gw);
                let mut push = |j: usize| {
                    if used[j] && !keep[j] {
                        keep[j] = true;
                        stack.push(j);
                    }
                };
                if x > 0 {
                    push(i - 1);
                }
                if x + 1 < gw {
                    push(i + 1);
                }
                if y > 0 {
                    push(i - gw);
                }
                if y + 1 < gh {
                    push(i + gw);
                }
            }
            used = keep;
        }
    }
    let mut index = vec![usize::MAX; (gw + 1) * (gh + 1)];
    let mut rest = vec![];
    let mut tris = vec![];
    let mut vid = |cx: usize, cy: usize, rest: &mut Vec<[f64; 2]>| -> usize {
        let k = cy * (gw + 1) + cx;
        if index[k] == usize::MAX {
            index[k] = rest.len();
            rest.push([x0 + cx as f64 * s, y0 + cy as f64 * s]);
        }
        index[k]
    };
    for gy in 0..gh {
        for gx in 0..gw {
            if !used[gy * gw + gx] {
                continue;
            }
            let a = vid(gx, gy, &mut rest);
            let b = vid(gx + 1, gy, &mut rest);
            let c = vid(gx, gy + 1, &mut rest);
            let d = vid(gx + 1, gy + 1, &mut rest);
            if (gx + gy) % 2 == 0 {
                tris.push([a, b, d]);
                tris.push([a, d, c]);
            } else {
                tris.push([a, b, c]);
                tris.push([b, d, c]);
            }
        }
    }
    let mut nbrs = vec![vec![]; rest.len()];
    for t in &tris {
        for (i, j) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            if !nbrs[i].contains(&j) {
                nbrs[i].push(j);
            }
            if !nbrs[j].contains(&i) {
                nbrs[j].push(i);
            }
        }
    }
    Some(Mesh { rest, tris, nbrs, spacing: s })
}

fn nearest(m: &Mesh, p: [f64; 2]) -> usize {
    let mut best = (f64::MAX, 0);
    for (i, v) in m.rest.iter().enumerate() {
        let d = (v[0] - p[0]).powi(2) + (v[1] - p[1]).powi(2);
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

fn cg(apply: &dyn Fn(&[f64], &mut [f64]), b: &[f64], x: &mut [f64], iters: usize) {
    let n = b.len();
    let mut ax = vec![0.0; n];
    apply(x, &mut ax);
    let mut r: Vec<f64> = (0..n).map(|i| b[i] - ax[i]).collect();
    let mut p = r.clone();
    let mut rr: f64 = r.iter().map(|v| v * v).sum();
    let bn: f64 = b.iter().map(|v| v * v).sum::<f64>().max(1e-30);
    let mut ap = vec![0.0; n];
    for _ in 0..iters {
        if rr < 1e-20 * bn {
            break;
        }
        apply(&p, &mut ap);
        let pap: f64 = p.iter().zip(&ap).map(|(a, b)| a * b).sum();
        if pap.abs() < 1e-300 {
            break;
        }
        let a = rr / pap;
        for i in 0..n {
            x[i] += a * p[i];
            r[i] -= a * ap[i];
        }
        let rn: f64 = r.iter().map(|v| v * v).sum();
        let beta = rn / rr;
        rr = rn;
        for i in 0..n {
            p[i] = r[i] + beta * p[i];
        }
    }
}

pub fn deform(m: &Mesh, pins: &[Pin]) -> Vec<[f64; 2]> {
    let n = m.rest.len();
    if n == 0 || pins.is_empty() {
        return m.rest.clone();
    }
    let reach = |p: &Pin| if p.extent > 0.0 { p.extent } else { m.spacing * 2.5 };
    let mut weight = vec![1.0f64; n];
    for p in pins.iter().filter(|p| p.kind == 3) {
        let r = reach(p);
        for (i, v) in m.rest.iter().enumerate() {
            let d = (v[0] - p.rest[0]).hypot(v[1] - p.rest[1]);
            if d < r {
                let k = 1.0 - d / r;
                weight[i] += p.amount.max(0.0) / 100.0 * 25.0 * k * k;
            }
        }
    }
    let mut cons: Vec<(usize, [f64; 2])> = vec![];
    let mut frames: Vec<Option<(f64, f64, f64)>> = vec![None; n];
    for p in pins {
        match p.kind {
            0 | 1 => {
                let i = nearest(m, p.rest);
                let off = [m.rest[i][0] - p.rest[0], m.rest[i][1] - p.rest[1]];
                let (s, c) = p.rotation.to_radians().sin_cos();
                let k = if p.kind == 1 { p.scale / 100.0 } else { 1.0 };
                cons.push((i, [p.pos[0] + k * (off[0] * c - off[1] * s), p.pos[1] + k * (off[0] * s + off[1] * c)]));
                if p.kind == 1 {
                    let r = reach(p);
                    for (j, v) in m.rest.iter().enumerate() {
                        let d = (v[0] - p.rest[0]).hypot(v[1] - p.rest[1]);
                        if d < r {
                            frames[j] = Some((p.rotation.to_radians(), p.scale / 100.0, 1.0 - d / r));
                        }
                    }
                }
            }
            2 => {
                let r = reach(p);
                for (j, v) in m.rest.iter().enumerate() {
                    let d = (v[0] - p.rest[0]).hypot(v[1] - p.rest[1]);
                    if d < r {
                        frames[j] = Some((p.rotation.to_radians(), p.scale / 100.0, 1.0 - d / r));
                    }
                }
            }
            _ => {}
        }
    }
    let ew = |i: usize, j: usize| (weight[i] + weight[j]) * 0.5;
    let wc = 1.0e4;
    let eps = 1.0e-6;
    let mut is_con = vec![None; n];
    for (i, t) in &cons {
        is_con[*i] = Some(*t);
    }
    let diag: Vec<f64> = (0..n).map(|i| m.nbrs[i].iter().map(|&j| ew(i, j)).sum::<f64>() + if is_con[i].is_some() { wc } else { 0.0 } + eps).collect();
    let apply = |x: &[f64], out: &mut [f64]| {
        for i in 0..n {
            let mut s = diag[i] * x[i];
            for &j in &m.nbrs[i] {
                s -= ew(i, j) * x[j];
            }
            out[i] = s;
        }
    };
    let shift = if cons.is_empty() {
        [0.0, 0.0]
    } else {
        let k = cons.len() as f64;
        [cons.iter().map(|c| c.1[0] - m.rest[c.0][0]).sum::<f64>() / k, cons.iter().map(|c| c.1[1] - m.rest[c.0][1]).sum::<f64>() / k]
    };
    let mut cur: Vec<[f64; 2]> = m.rest.iter().map(|v| [v[0] + shift[0], v[1] + shift[1]]).collect();
    let mut rots: Vec<[f64; 4]> = vec![[1.0, 0.0, 0.0, 1.0]; n];
    let (mut xs, mut ys): (Vec<f64>, Vec<f64>) = (cur.iter().map(|v| v[0]).collect(), cur.iter().map(|v| v[1]).collect());
    for it in 0..10 {
        if it > 0 {
            for i in 0..n {
                let (mut a, mut b, mut c, mut d) = (0.0, 0.0, 0.0, 0.0);
                for &j in &m.nbrs[i] {
                    let w = ew(i, j);
                    let e = [m.rest[i][0] - m.rest[j][0], m.rest[i][1] - m.rest[j][1]];
                    let f = [cur[i][0] - cur[j][0], cur[i][1] - cur[j][1]];
                    a += w * e[0] * f[0];
                    b += w * e[0] * f[1];
                    c += w * e[1] * f[0];
                    d += w * e[1] * f[1];
                }
                let mut ang = (b - c).atan2(a + d);
                let mut sc = 1.0;
                if let Some((fr, fs, k)) = frames[i] {
                    let diff = (fr - ang + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
                    ang += diff * k;
                    sc = 1.0 + (fs - 1.0) * k;
                }
                let (s, co) = ang.sin_cos();
                rots[i] = [co * sc, -s * sc, s * sc, co * sc];
            }
        }
        let mut bx = vec![0.0; n];
        let mut by = vec![0.0; n];
        for i in 0..n {
            for &j in &m.nbrs[i] {
                let w = ew(i, j) * 0.5;
                let e = [m.rest[i][0] - m.rest[j][0], m.rest[i][1] - m.rest[j][1]];
                let (ri, rj) = (rots[i], rots[j]);
                let r = [ri[0] + rj[0], ri[1] + rj[1], ri[2] + rj[2], ri[3] + rj[3]];
                bx[i] += w * (r[0] * e[0] + r[1] * e[1]);
                by[i] += w * (r[2] * e[0] + r[3] * e[1]);
            }
            if let Some(t) = is_con[i] {
                bx[i] += wc * t[0];
                by[i] += wc * t[1];
            }
            bx[i] += eps * (m.rest[i][0] + shift[0]);
            by[i] += eps * (m.rest[i][1] + shift[1]);
        }
        cg(&apply, &bx, &mut xs, 400);
        cg(&apply, &by, &mut ys, 400);
        for i in 0..n {
            cur[i] = [xs[i], ys[i]];
        }
    }
    cur
}

pub fn depths(m: &Mesh, pins: &[Pin]) -> Vec<f64> {
    m.tris
        .iter()
        .map(|t| {
            let c = [(m.rest[t[0]][0] + m.rest[t[1]][0] + m.rest[t[2]][0]) / 3.0, (m.rest[t[0]][1] + m.rest[t[1]][1] + m.rest[t[2]][1]) / 3.0];
            pins.iter()
                .filter(|p| p.kind == 4)
                .map(|p| {
                    let r = if p.extent > 0.0 { p.extent } else { m.spacing * 3.0 };
                    let d = (c[0] - p.rest[0]).hypot(c[1] - p.rest[1]);
                    if d < r { p.in_front * (1.0 - d / r) } else { 0.0 }
                })
                .sum()
        })
        .collect()
}

pub fn rasterize(src: &Img, from: &[[f64; 2]], to: &[[f64; 2]], tris: &[[usize; 3]], order: &[usize], depth: &[f64]) -> Img {
    let (w, h) = (src.w, src.h);
    let mut out = Img::new(w, h);
    let band = 16usize;
    let nb = h.div_ceil(band).max(1);
    let mut bins: Vec<Vec<usize>> = vec![vec![]; nb];
    for &ti in order {
        let t = tris[ti];
        let ys = [to[t[0]][1], to[t[1]][1], to[t[2]][1]];
        let (lo, hi) = (ys.iter().cloned().fold(f64::MAX, f64::min), ys.iter().cloned().fold(f64::MIN, f64::max));
        if hi < 0.0 || lo > h as f64 {
            continue;
        }
        let (b0, b1) = ((lo.max(0.0) as usize / band).min(nb - 1), (hi.max(0.0) as usize / band).min(nb - 1));
        for b in bins.iter_mut().take(b1 + 1).skip(b0) {
            b.push(ti);
        }
    }
    out.px.par_chunks_mut(w * 4 * band).enumerate().for_each(|(bi, chunk)| {
        let rows = chunk.len() / (w * 4);
        let mut zbuf = vec![f64::NEG_INFINITY; rows * w];
        for &ti in &bins[bi] {
            let t = tris[ti];
            let z = depth.get(ti).copied().unwrap_or(0.0);
            let (p0, p1, p2) = (to[t[0]], to[t[1]], to[t[2]]);
            let den = (p1[1] - p2[1]) * (p0[0] - p2[0]) + (p2[0] - p1[0]) * (p0[1] - p2[1]);
            if den.abs() < 1e-12 {
                continue;
            }
            let xmin = p0[0].min(p1[0]).min(p2[0]).floor().max(0.0) as usize;
            let xmax = (p0[0].max(p1[0]).max(p2[0]).ceil().max(0.0) as usize).min(w.saturating_sub(1));
            let ymin = p0[1].min(p1[1]).min(p2[1]).floor().max((bi * band) as f64) as usize;
            let ymax = (p0[1].max(p1[1]).max(p2[1]).ceil().max(0.0) as usize).min(bi * band + rows - 1);
            let (s0, s1, s2) = (from[t[0]], from[t[1]], from[t[2]]);
            for y in ymin..=ymax {
                let py = y as f64 + 0.5;
                let row = &mut chunk[(y - bi * band) * w * 4..(y - bi * band + 1) * w * 4];
                for x in xmin..=xmax {
                    let px = x as f64 + 0.5;
                    let l0 = ((p1[1] - p2[1]) * (px - p2[0]) + (p2[0] - p1[0]) * (py - p2[1])) / den;
                    let l1 = ((p2[1] - p0[1]) * (px - p2[0]) + (p0[0] - p2[0]) * (py - p2[1])) / den;
                    let l2 = 1.0 - l0 - l1;
                    if l0 < -1e-7 || l1 < -1e-7 || l2 < -1e-7 {
                        continue;
                    }
                    let zi = (y - bi * band) * w + x;
                    if z <= zbuf[zi] + 1e-9 {
                        continue;
                    }
                    zbuf[zi] = z;
                    let sx = s0[0] * l0 + s1[0] * l1 + s2[0] * l2;
                    let sy = s0[1] * l0 + s1[1] * l1 + s2[1] * l2;
                    let c = src.sample(sx, sy, false);
                    if c[3] <= 0.0 {
                        continue;
                    }
                    let d = &mut row[x * 4..x * 4 + 4];
                    let k = 1.0 - c[3].min(1.0);
                    for ch in 0..4 {
                        d[ch] = c[ch] + d[ch] * k;
                    }
                }
            }
        }
    });
    out
}
