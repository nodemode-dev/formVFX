use rayon::prelude::*;

use super::image::Img;

#[derive(Clone, Debug)]
pub struct Gray {
    pub w: usize,
    pub h: usize,
    pub px: Vec<f32>,
}

impl Gray {
    pub fn new(w: usize, h: usize) -> Gray {
        Gray { w, h, px: vec![0.0; w * h] }
    }

    pub fn from_img(img: &Img, channel: u32) -> Gray {
        let px = img
            .px
            .par_chunks_exact(4)
            .map(|p| {
                let a = p[3].max(1e-6);
                let (r, g, b) = (p[0] / a, p[1] / a, p[2] / a);
                let v = match channel {
                    1 => 0.2126 * r + 0.7152 * g + 0.0722 * b,
                    2 => {
                        let mx = r.max(g).max(b);
                        let mn = r.min(g).min(b);
                        if mx > 1e-6 { (mx - mn) / mx } else { 0.0 }
                    }
                    _ => (r + g + b) / 3.0,
                };
                v * p[3].min(1.0) * 255.0
            })
            .collect();
        Gray { w: img.w, h: img.h, px }
    }

    #[inline]
    pub fn at(&self, x: isize, y: isize) -> f32 {
        let xx = x.clamp(0, self.w as isize - 1) as usize;
        let yy = y.clamp(0, self.h as isize - 1) as usize;
        self.px[yy * self.w + xx]
    }

    #[inline]
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let x0 = x.floor();
        let y0 = y.floor();
        let fx = x - x0;
        let fy = y - y0;
        let (xi, yi) = (x0 as isize, y0 as isize);
        let a = self.at(xi, yi);
        let b = self.at(xi + 1, yi);
        let c = self.at(xi, yi + 1);
        let d = self.at(xi + 1, yi + 1);
        (a + (b - a) * fx) * (1.0 - fy) + (c + (d - c) * fx) * fy
    }

    pub fn half(&self) -> Gray {
        let (w, h) = ((self.w / 2).max(1), (self.h / 2).max(1));
        let px = (0..w * h)
            .into_par_iter()
            .map(|i| {
                let (x, y) = ((i % w) as isize * 2, (i / w) as isize * 2);
                (self.at(x, y) + self.at(x + 1, y) + self.at(x, y + 1) + self.at(x + 1, y + 1)) * 0.25
            })
            .collect();
        Gray { w, h, px }
    }

    pub fn blur(&mut self, sigma: f32) {
        if sigma < 0.3 {
            return;
        }
        crate::effects::util::blur_scalar(&mut self.px, self.w, self.h, sigma as f64, sigma as f64);
    }

    pub fn pyramid(&self, levels: usize) -> Vec<Gray> {
        let mut out = vec![self.clone()];
        while out.len() < levels {
            let last = &out[out.len() - 1];
            if last.w < 24 || last.h < 24 {
                break;
            }
            let mut n = last.half();
            n.blur(0.6);
            out.push(n);
        }
        out
    }

    pub fn grad(&self) -> (Vec<f32>, Vec<f32>) {
        let (w, h) = (self.w, self.h);
        let gx: Vec<f32> = (0..w * h)
            .into_par_iter()
            .map(|i| {
                let (x, y) = ((i % w) as isize, (i / w) as isize);
                (self.at(x + 1, y) - self.at(x - 1, y)) * 0.5
            })
            .collect();
        let gy: Vec<f32> = (0..w * h)
            .into_par_iter()
            .map(|i| {
                let (x, y) = ((i % w) as isize, (i / w) as isize);
                (self.at(x, y + 1) - self.at(x, y - 1)) * 0.5
            })
            .collect();
        (gx, gy)
    }
}

pub struct Match {
    pub center: [f32; 2],
    pub score: f32,
}

pub fn patch(g: &Gray, c: [f32; 2], hw: usize, hh: usize) -> Vec<f32> {
    let mut v = Vec::with_capacity((2 * hw + 1) * (2 * hh + 1));
    for dy in -(hh as isize)..=hh as isize {
        for dx in -(hw as isize)..=hw as isize {
            v.push(g.sample(c[0] + dx as f32, c[1] + dy as f32));
        }
    }
    v
}

fn ncc(t: &[f32], tm: f32, tn: f32, g: &Gray, x: isize, y: isize, hw: isize, hh: isize) -> f32 {
    let n = t.len() as f32;
    let mut sum = 0.0f32;
    let mut sq = 0.0f32;
    let mut cross = 0.0f32;
    let mut k = 0;
    for dy in -hh..=hh {
        for dx in -hw..=hw {
            let v = g.at(x + dx, y + dy);
            sum += v;
            sq += v * v;
            cross += v * (t[k] - tm);
            k += 1;
        }
    }
    let var = (sq - sum * sum / n).max(0.0);
    if var < 1e-3 || tn < 1e-3 {
        return 0.0;
    }
    cross / (var.sqrt() * tn)
}

pub fn template_stats(t: &[f32]) -> (f32, f32) {
    let n = t.len() as f32;
    let m = t.iter().sum::<f32>() / n;
    let v = t.iter().map(|x| (x - m) * (x - m)).sum::<f32>();
    (m, v.sqrt())
}

pub fn find(t: &[f32], hw: usize, hh: usize, g: &Gray, center: [f32; 2], sw: usize, sh: usize) -> Match {
    let (tm, tn) = template_stats(t);
    let (cx, cy) = (center[0].round() as isize, center[1].round() as isize);
    let (sw, sh) = (sw as isize, sh as isize);
    let rows: Vec<(f32, isize, isize)> = (-sh..=sh)
        .into_par_iter()
        .map(|dy| {
            let mut best = (f32::MIN, 0isize, dy);
            for dx in -sw..=sw {
                let s = ncc(t, tm, tn, g, cx + dx, cy + dy, hw as isize, hh as isize);
                if s > best.0 {
                    best = (s, dx, dy);
                }
            }
            best
        })
        .collect();
    let best = rows.into_iter().fold((f32::MIN, 0, 0), |a, b| if b.0 > a.0 { b } else { a });
    let (bx, by) = (cx + best.1, cy + best.2);
    let s = |dx: isize, dy: isize| ncc(t, tm, tn, g, bx + dx, by + dy, hw as isize, hh as isize);
    let (l, r, u, d) = (s(-1, 0), s(1, 0), s(0, -1), s(0, 1));
    let c = best.0;
    let ox = {
        let den = l - 2.0 * c + r;
        if den.abs() > 1e-6 { (0.5 * (l - r) / den).clamp(-0.5, 0.5) } else { 0.0 }
    };
    let oy = {
        let den = u - 2.0 * c + d;
        if den.abs() > 1e-6 { (0.5 * (u - d) / den).clamp(-0.5, 0.5) } else { 0.0 }
    };
    Match { center: [bx as f32 + ox, by as f32 + oy], score: c.max(0.0) }
}

pub fn corners(g: &Gray, inside: &dyn Fn(f32, f32) -> bool, max: usize, min_dist: f32) -> Vec<[f32; 2]> {
    let (gx, gy) = g.grad();
    let (w, h) = (g.w, g.h);
    let mut sxx: Vec<f32> = gx.iter().map(|v| v * v).collect();
    let mut syy: Vec<f32> = gy.iter().map(|v| v * v).collect();
    let mut sxy: Vec<f32> = gx.iter().zip(&gy).map(|(a, b)| a * b).collect();
    for b in [&mut sxx, &mut syy, &mut sxy] {
        crate::effects::util::blur_scalar(b, w, h, 1.5, 1.5);
    }
    let score: Vec<f32> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (a, c, b) = (sxx[i], syy[i], sxy[i]);
            let tr = a + c;
            let det = a * c - b * b;
            (tr * 0.5 - ((tr * tr * 0.25 - det).max(0.0)).sqrt()).max(0.0)
        })
        .collect();
    let border = 6usize;
    let mut cand: Vec<(f32, usize, usize)> = vec![];
    for y in border..h.saturating_sub(border) {
        for x in border..w.saturating_sub(border) {
            let s = score[y * w + x];
            if s <= 1.0 || !inside(x as f32, y as f32) {
                continue;
            }
            let mut is_max = true;
            'n: for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if (dx != 0 || dy != 0) && score[(y as i32 + dy) as usize * w + (x as i32 + dx) as usize] > s {
                        is_max = false;
                        break 'n;
                    }
                }
            }
            if is_max {
                cand.push((s, x, y));
            }
        }
    }
    cand.sort_by(|a, b| b.0.total_cmp(&a.0));
    let best = cand.first().map(|c| c.0).unwrap_or(0.0);
    let mut out: Vec<[f32; 2]> = vec![];
    let md2 = min_dist * min_dist;
    for (s, x, y) in cand {
        if s < best * 0.01 {
            break;
        }
        let p = [x as f32, y as f32];
        if out.iter().all(|q| (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) >= md2) {
            out.push(p);
            if out.len() >= max {
                break;
            }
        }
    }
    out
}

pub fn klt(a: &[Gray], b: &[Gray], pts: &[[f32; 2]], guess: Option<&[[f32; 2]]>, win: i32) -> Vec<Option<[f32; 2]>> {
    let levels = a.len().min(b.len());
    let grads: Vec<(Vec<f32>, Vec<f32>)> = a.iter().take(levels).map(|g| g.grad()).collect();
    pts.par_iter()
        .enumerate()
        .map(|(pi, p)| {
            let g0 = guess.and_then(|g| g.get(pi)).copied().unwrap_or(*p);
            let mut d = [(g0[0] - p[0]) / (1 << (levels - 1)) as f32, (g0[1] - p[1]) / (1 << (levels - 1)) as f32];
            for lv in (0..levels).rev() {
                let k = (1 << lv) as f32;
                let (ga, gb) = (&a[lv], &b[lv]);
                let (gx, gy) = &grads[lv];
                let c = [p[0] / k, p[1] / k];
                let mut gxx = 0.0f32;
                let mut gyy = 0.0f32;
                let mut gxy = 0.0f32;
                let mut samples = Vec::with_capacity(((2 * win + 1) * (2 * win + 1)) as usize);
                for dy in -win..=win {
                    for dx in -win..=win {
                        let (x, y) = (c[0] + dx as f32, c[1] + dy as f32);
                        let xi = (x.round() as isize).clamp(0, ga.w as isize - 1) as usize;
                        let yi = (y.round() as isize).clamp(0, ga.h as isize - 1) as usize;
                        let (ix, iy) = (gx[yi * ga.w + xi], gy[yi * ga.w + xi]);
                        gxx += ix * ix;
                        gyy += iy * iy;
                        gxy += ix * iy;
                        samples.push((x, y, ix, iy, ga.sample(x, y)));
                    }
                }
                let det = gxx * gyy - gxy * gxy;
                if det.abs() < 1e-3 {
                    return None;
                }
                for _ in 0..12 {
                    let mut bx = 0.0f32;
                    let mut by = 0.0f32;
                    for (x, y, ix, iy, va) in &samples {
                        let it = gb.sample(x + d[0], y + d[1]) - va;
                        bx += it * ix;
                        by += it * iy;
                    }
                    let ux = -(gyy * bx - gxy * by) / det;
                    let uy = -(gxx * by - gxy * bx) / det;
                    d[0] += ux;
                    d[1] += uy;
                    if ux * ux + uy * uy < 1e-4 {
                        break;
                    }
                }
                if lv > 0 {
                    d = [d[0] * 2.0, d[1] * 2.0];
                }
            }
            let q = [p[0] + d[0], p[1] + d[1]];
            let (w0, h0) = (a[0].w as f32, a[0].h as f32);
            if !q[0].is_finite() || !q[1].is_finite() || q[0] < 0.0 || q[1] < 0.0 || q[0] >= w0 || q[1] >= h0 {
                return None;
            }
            let err = {
                let mut e = 0.0f32;
                let mut n = 0.0f32;
                for dy in -3..=3 {
                    for dx in -3..=3 {
                        let va = a[0].sample(p[0] + dx as f32, p[1] + dy as f32);
                        let vb = b[0].sample(q[0] + dx as f32, q[1] + dy as f32);
                        e += (va - vb).abs();
                        n += 1.0;
                    }
                }
                e / n
            };
            (err < 30.0).then_some(q)
        })
        .collect()
}

pub fn dense_flow(a: &Gray, b: &Gray) -> Vec<[f32; 2]> {
    let pa = a.pyramid(6);
    let pb = b.pyramid(6);
    let levels = pa.len().min(pb.len());
    let mut flow: Vec<[f32; 2]> = vec![[0.0; 2]; pa[levels - 1].w * pa[levels - 1].h];
    for lv in (0..levels).rev() {
        let (ga, gb) = (&pa[lv], &pb[lv]);
        let (w, h) = (ga.w, ga.h);
        if lv < levels - 1 {
            let (pw, ph) = (pa[lv + 1].w, pa[lv + 1].h);
            let prev = flow;
            flow = (0..w * h)
                .into_par_iter()
                .map(|i| {
                    let (x, y) = ((i % w) as f32 * 0.5, (i / w) as f32 * 0.5);
                    let xi = (x as usize).min(pw - 1);
                    let yi = (y as usize).min(ph - 1);
                    let f = prev[yi * pw + xi];
                    [f[0] * 2.0, f[1] * 2.0]
                })
                .collect();
        }
        let (gx, gy) = ga.grad();
        let mut sxx: Vec<f32> = gx.iter().map(|v| v * v).collect();
        let mut syy: Vec<f32> = gy.iter().map(|v| v * v).collect();
        let mut sxy: Vec<f32> = gx.iter().zip(&gy).map(|(p, q)| p * q).collect();
        for s in [&mut sxx, &mut syy, &mut sxy] {
            crate::effects::util::blur_scalar(s, w, h, 2.0, 2.0);
        }
        for _ in 0..3 {
            let it: Vec<f32> = (0..w * h).into_par_iter().map(|i| gb.sample((i % w) as f32 + flow[i][0], (i / w) as f32 + flow[i][1]) - ga.px[i]).collect();
            let mut bx: Vec<f32> = it.iter().zip(&gx).map(|(t, g)| t * g).collect();
            let mut by: Vec<f32> = it.iter().zip(&gy).map(|(t, g)| t * g).collect();
            crate::effects::util::blur_scalar(&mut bx, w, h, 2.0, 2.0);
            crate::effects::util::blur_scalar(&mut by, w, h, 2.0, 2.0);
            flow.par_iter_mut().enumerate().for_each(|(i, f)| {
                let (a11, a22, a12) = (sxx[i] + 0.5, syy[i] + 0.5, sxy[i]);
                let det = a11 * a22 - a12 * a12;
                if det.abs() < 1e-6 {
                    return;
                }
                let ux = -(a22 * bx[i] - a12 * by[i]) / det;
                let uy = -(a11 * by[i] - a12 * bx[i]) / det;
                f[0] += ux.clamp(-2.0, 2.0);
                f[1] += uy.clamp(-2.0, 2.0);
            });
            let mut fx: Vec<f32> = flow.iter().map(|f| f[0]).collect();
            let mut fy: Vec<f32> = flow.iter().map(|f| f[1]).collect();
            crate::effects::util::blur_scalar(&mut fx, w, h, 1.0, 1.0);
            crate::effects::util::blur_scalar(&mut fy, w, h, 1.0, 1.0);
            flow = fx.into_iter().zip(fy).map(|(x, y)| [x, y]).collect();
        }
    }
    flow
}

pub type H3 = [[f64; 3]; 3];

pub fn h_apply(h: &H3, p: [f64; 2]) -> [f64; 2] {
    let w = h[2][0] * p[0] + h[2][1] * p[1] + h[2][2];
    let w = if w.abs() < 1e-12 { 1e-12 } else { w };
    [(h[0][0] * p[0] + h[0][1] * p[1] + h[0][2]) / w, (h[1][0] * p[0] + h[1][1] * p[1] + h[1][2]) / w]
}

pub fn h_mul(a: &H3, b: &H3) -> H3 {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

pub fn h_inv(m: &H3) -> Option<H3> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-15 {
        return None;
    }
    let d = 1.0 / det;
    Some([
        [(m[1][1] * m[2][2] - m[1][2] * m[2][1]) * d, (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * d, (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * d],
        [(m[1][2] * m[2][0] - m[1][0] * m[2][2]) * d, (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * d, (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * d],
        [(m[1][0] * m[2][1] - m[1][1] * m[2][0]) * d, (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * d, (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * d],
    ])
}

pub const IDENTITY: H3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

pub fn solve(a: &mut [Vec<f64>], b: &mut [f64]) -> Option<Vec<f64>> {
    let n = b.len();
    for c in 0..n {
        let piv = (c..n).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[piv][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, piv);
        b.swap(c, piv);
        for r in 0..n {
            if r != c {
                let f = a[r][c] / a[c][c];
                for k in c..n {
                    a[r][k] -= f * a[c][k];
                }
                b[r] -= f * b[c];
            }
        }
    }
    Some((0..n).map(|i| b[i] / a[i][i]).collect())
}

fn lsq(rows: &[(Vec<f64>, f64)], n: usize) -> Option<Vec<f64>> {
    let mut ata = vec![vec![0.0; n]; n];
    let mut atb = vec![0.0; n];
    for (r, y) in rows {
        for i in 0..n {
            atb[i] += r[i] * y;
            for j in 0..n {
                ata[i][j] += r[i] * r[j];
            }
        }
    }
    solve(&mut ata, &mut atb)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Model {
    Translation,
    Similarity,
    ScaleOnly,
    Affine,
    Homography,
}

impl Model {
    fn min_pts(&self) -> usize {
        match self {
            Model::Translation => 1,
            Model::ScaleOnly | Model::Similarity => 2,
            Model::Affine => 3,
            Model::Homography => 4,
        }
    }
}

fn normalize(pts: &[[f64; 2]]) -> ([f64; 2], f64) {
    let n = pts.len().max(1) as f64;
    let c = [pts.iter().map(|p| p[0]).sum::<f64>() / n, pts.iter().map(|p| p[1]).sum::<f64>() / n];
    let d = pts.iter().map(|p| (p[0] - c[0]).hypot(p[1] - c[1])).sum::<f64>() / n;
    (c, if d > 1e-9 { std::f64::consts::SQRT_2 / d } else { 1.0 })
}

pub fn fit(model: Model, src: &[[f64; 2]], dst: &[[f64; 2]]) -> Option<H3> {
    if src.len() < model.min_pts() || src.len() != dst.len() {
        return None;
    }
    match model {
        Model::Translation => {
            let n = src.len() as f64;
            let t = [src.iter().zip(dst).map(|(a, b)| b[0] - a[0]).sum::<f64>() / n, src.iter().zip(dst).map(|(a, b)| b[1] - a[1]).sum::<f64>() / n];
            Some([[1.0, 0.0, t[0]], [0.0, 1.0, t[1]], [0.0, 0.0, 1.0]])
        }
        Model::ScaleOnly => {
            let rows: Vec<(Vec<f64>, f64)> = src.iter().zip(dst).flat_map(|(a, b)| [(vec![a[0], 1.0, 0.0], b[0]), (vec![a[1], 0.0, 1.0], b[1])]).collect();
            let v = lsq(&rows, 3)?;
            Some([[v[0], 0.0, v[1]], [0.0, v[0], v[2]], [0.0, 0.0, 1.0]])
        }
        Model::Similarity => {
            let rows: Vec<(Vec<f64>, f64)> = src.iter().zip(dst).flat_map(|(a, b)| [(vec![a[0], -a[1], 1.0, 0.0], b[0]), (vec![a[1], a[0], 0.0, 1.0], b[1])]).collect();
            let v = lsq(&rows, 4)?;
            Some([[v[0], -v[1], v[2]], [v[1], v[0], v[3]], [0.0, 0.0, 1.0]])
        }
        Model::Affine => {
            let rx: Vec<(Vec<f64>, f64)> = src.iter().zip(dst).map(|(a, b)| (vec![a[0], a[1], 1.0], b[0])).collect();
            let ry: Vec<(Vec<f64>, f64)> = src.iter().zip(dst).map(|(a, b)| (vec![a[0], a[1], 1.0], b[1])).collect();
            let x = lsq(&rx, 3)?;
            let y = lsq(&ry, 3)?;
            Some([[x[0], x[1], x[2]], [y[0], y[1], y[2]], [0.0, 0.0, 1.0]])
        }
        Model::Homography => {
            let (cs, ks) = normalize(src);
            let (cd, kd) = normalize(dst);
            let rows: Vec<(Vec<f64>, f64)> = src
                .iter()
                .zip(dst)
                .flat_map(|(a, b)| {
                    let (x, y) = ((a[0] - cs[0]) * ks, (a[1] - cs[1]) * ks);
                    let (u, v) = ((b[0] - cd[0]) * kd, (b[1] - cd[1]) * kd);
                    [(vec![x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y], u), (vec![0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y], v)]
                })
                .collect();
            let h = lsq(&rows, 8)?;
            let hn = [[h[0], h[1], h[2]], [h[3], h[4], h[5]], [h[6], h[7], 1.0]];
            let ts = [[ks, 0.0, -cs[0] * ks], [0.0, ks, -cs[1] * ks], [0.0, 0.0, 1.0]];
            let td_inv = [[1.0 / kd, 0.0, cd[0]], [0.0, 1.0 / kd, cd[1]], [0.0, 0.0, 1.0]];
            let m = h_mul(&td_inv, &h_mul(&hn, &ts));
            let s = m[2][2];
            (s.abs() > 1e-12).then(|| m.map(|r| r.map(|x| x / s)))
        }
    }
}

pub fn ransac(model: Model, src: &[[f64; 2]], dst: &[[f64; 2]], thresh: f64, iters: usize) -> Option<(H3, Vec<bool>)> {
    let n = src.len();
    let k = model.min_pts();
    if n < k {
        return None;
    }
    if n == k {
        let h = fit(model, src, dst)?;
        return Some((h, vec![true; n]));
    }
    let mut seed = 0x9e37_79b9u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let mut best: Option<(usize, H3)> = None;
    for _ in 0..iters {
        let mut idx = Vec::with_capacity(k);
        let mut guard = 0;
        while idx.len() < k && guard < 100 {
            let i = next();
            if !idx.contains(&i) {
                idx.push(i);
            }
            guard += 1;
        }
        let s: Vec<[f64; 2]> = idx.iter().map(|&i| src[i]).collect();
        let d: Vec<[f64; 2]> = idx.iter().map(|&i| dst[i]).collect();
        let Some(h) = fit(model, &s, &d) else { continue };
        let count = src.iter().zip(dst).filter(|(a, b)| {
            let q = h_apply(&h, **a);
            (q[0] - b[0]).hypot(q[1] - b[1]) < thresh
        }).count();
        if best.as_ref().is_none_or(|b| count > b.0) {
            best = Some((count, h));
        }
    }
    let (_, h) = best?;
    let inl: Vec<bool> = src.iter().zip(dst).map(|(a, b)| {
        let q = h_apply(&h, *a);
        (q[0] - b[0]).hypot(q[1] - b[1]) < thresh
    }).collect();
    let s: Vec<[f64; 2]> = src.iter().zip(&inl).filter(|x| *x.1).map(|x| *x.0).collect();
    let d: Vec<[f64; 2]> = dst.iter().zip(&inl).filter(|x| *x.1).map(|x| *x.0).collect();
    let h = fit(model, &s, &d).unwrap_or(h);
    Some((h, inl))
}

pub struct Flow {
    pub w: usize,
    pub h: usize,
    pub scale: f32,
    pub v: Vec<[f32; 2]>,
}

impl Flow {
    pub fn between(a: &Img, b: &Img) -> Flow {
        let (mut ga, mut gb) = (Gray::from_img(a, 1), Gray::from_img(b, 1));
        let mut scale = 1.0f32;
        while ga.w.max(ga.h) > 640 {
            ga = ga.half();
            gb = gb.half();
            scale *= 0.5;
        }
        let v = dense_flow(&ga, &gb);
        Flow { w: ga.w, h: ga.h, scale, v }
    }

    pub fn at(&self, x: f32, y: f32) -> [f32; 2] {
        let (fx, fy) = ((x * self.scale - 0.5).clamp(0.0, (self.w - 1) as f32), (y * self.scale - 0.5).clamp(0.0, (self.h - 1) as f32));
        let (x0, y0) = (fx as usize, fy as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let g = |x: usize, y: usize| self.v[y * self.w + x];
        let mut o = [0.0f32; 2];
        for (k, v) in o.iter_mut().enumerate() {
            let top = g(x0, y0)[k] + (g(x1, y0)[k] - g(x0, y0)[k]) * tx;
            let bot = g(x0, y1)[k] + (g(x1, y1)[k] - g(x0, y1)[k]) * tx;
            *v = (top + (bot - top) * ty) / self.scale;
        }
        o
    }
}

type FlowEntry = (std::sync::Weak<Img>, std::sync::Weak<Img>, std::sync::Arc<Flow>);

pub fn flow_cached(a: &std::sync::Arc<Img>, b: &std::sync::Arc<Img>) -> std::sync::Arc<Flow> {
    use std::sync::{Arc, OnceLock};
    static C: OnceLock<parking_lot::Mutex<Vec<FlowEntry>>> = OnceLock::new();
    let c = C.get_or_init(Default::default);
    let hit = c.lock().iter().find(|e| e.0.upgrade().is_some_and(|x| Arc::ptr_eq(&x, a)) && e.1.upgrade().is_some_and(|x| Arc::ptr_eq(&x, b))).map(|e| e.2.clone());
    if let Some(f) = hit {
        return f;
    }
    let f = Arc::new(Flow::between(a, b));
    let mut g = c.lock();
    g.retain(|e| e.0.strong_count() > 0 && e.1.strong_count() > 0);
    if g.len() >= 6 {
        g.remove(0);
    }
    g.push((Arc::downgrade(a), Arc::downgrade(b), f.clone()));
    f
}

pub fn pixel_motion(a: &Img, b: &Img, flow: &Flow, frac: f32) -> Img {
    let mut out = Img::new(a.w, a.h);
    let w = a.w;
    out.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            let f = flow.at(cx, cy);
            let pa = a.sample((cx - frac * f[0]) as f64, (cy - frac * f[1]) as f64, true);
            let pb = b.sample((cx + (1.0 - frac) * f[0]) as f64, (cy + (1.0 - frac) * f[1]) as f64, true);
            for k in 0..4 {
                row[x * 4 + k] = pa[k] * (1.0 - frac) + pb[k] * frac;
            }
        }
    });
    out
}
