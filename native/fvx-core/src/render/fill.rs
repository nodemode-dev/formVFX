use rayon::prelude::*;

use super::analysis::Source;
use super::image::Img;
use super::vision::{self, Gray, H3, Model};
use crate::engine::jobs::Ctx;

pub const METHODS: [&str; 3] = ["object", "surface", "edgeBlend"];
pub const LIGHTING: [&str; 4] = ["none", "subtle", "moderate", "strong"];

#[derive(Clone, Debug)]
pub struct Settings {
    pub method: usize,
    pub expansion: f64,
    pub lighting: usize,
}

pub struct Frame {
    pub raw: Img,
    pub hole: Vec<bool>,
}

fn dilate(mask: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    if r == 0 {
        return mask.to_vec();
    }
    let mut row = vec![false; w * h];
    row.par_chunks_mut(w).enumerate().for_each(|(y, out)| {
        let mut last: i64 = -1_000_000;
        for x in 0..w {
            if mask[y * w + x] {
                last = x as i64;
            }
            out[x] = x as i64 - last <= r as i64;
        }
        let mut next: i64 = 1_000_000;
        for x in (0..w).rev() {
            if mask[y * w + x] {
                next = x as i64;
            }
            out[x] |= next - x as i64 <= r as i64;
        }
    });
    let mut out = vec![false; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, o)| {
        let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            o[x] = (a..=b).any(|yy| row[yy * w + x]);
        }
    });
    out
}

pub fn read(src: &Source, t: f64, expansion: f64) -> Option<Frame> {
    let raw = src.image(t)?;
    let masked = src.masked(t)?;
    let (w, h) = (raw.w, raw.h);
    let hole: Vec<bool> = raw.px.chunks_exact(4).zip(masked.px.chunks_exact(4)).map(|(r, m)| r[3] > 0.5 && m[3] < 0.5).collect();
    let r = (expansion * src.scale).round().max(0.0) as usize;
    Some(Frame { hole: dilate(&hole, w, h, r), raw })
}

pub fn membrane(vals: &mut [[f32; 4]], known: &[bool], w: usize, h: usize) {
    if known.iter().all(|k| *k) || !known.iter().any(|k| *k) {
        return;
    }
    let mut levels: Vec<(Vec<[f32; 4]>, Vec<f32>, usize, usize)> = vec![(vals.iter().zip(known).map(|(v, k)| if *k { *v } else { [0.0; 4] }).collect(), known.iter().map(|k| if *k { 1.0 } else { 0.0 }).collect(), w, h)];
    while {
        let l = levels.last().map(|l| (l.2, l.3)).unwrap_or((1, 1));
        l.0 > 1 || l.1 > 1
    } {
        let (v, wt, lw, lh) = levels.last().cloned().unwrap_or_default();
        let (nw, nh) = (lw.div_ceil(2).max(1), lh.div_ceil(2).max(1));
        let mut nv = vec![[0.0f32; 4]; nw * nh];
        let mut nwt = vec![0.0f32; nw * nh];
        for y in 0..lh {
            for x in 0..lw {
                let i = y * lw + x;
                let j = (y / 2) * nw + x / 2;
                if wt[i] > 0.0 {
                    for c in 0..4 {
                        nv[j][c] += v[i][c] * wt[i];
                    }
                    nwt[j] += wt[i];
                }
            }
        }
        for j in 0..nw * nh {
            if nwt[j] > 0.0 {
                for c in 0..4 {
                    nv[j][c] /= nwt[j];
                }
                nwt[j] = nwt[j].min(1.0);
            }
        }
        levels.push((nv, nwt, nw, nh));
        if levels.len() > 24 {
            break;
        }
    }
    for li in (0..levels.len() - 1).rev() {
        let (cv, _, cww, chh) = levels[li + 1].clone();
        let (v, wt, lw, lh) = &mut levels[li];
        for y in 0..*lh {
            for x in 0..*lw {
                let i = y * *lw + x;
                if wt[i] >= 0.999 {
                    continue;
                }
                let fx = ((x as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, (cww - 1) as f32);
                let fy = ((y as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, (chh - 1) as f32);
                let (x0, y0) = (fx as usize, fy as usize);
                let (x1, y1) = ((x0 + 1).min(cww - 1), (y0 + 1).min(chh - 1));
                let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
                let g = |xx: usize, yy: usize| cv[yy * cww + xx];
                let mut up = [0.0f32; 4];
                for c in 0..4 {
                    let a = g(x0, y0)[c] + (g(x1, y0)[c] - g(x0, y0)[c]) * tx;
                    let b = g(x0, y1)[c] + (g(x1, y1)[c] - g(x0, y1)[c]) * tx;
                    up[c] = a + (b - a) * ty;
                }
                let k = wt[i];
                for c in 0..4 {
                    v[i][c] = v[i][c] * k + up[c] * (1.0 - k);
                }
            }
        }
    }
    let base = &levels[0].0;
    for (i, k) in known.iter().enumerate() {
        if !*k {
            vals[i] = base[i];
        }
    }
    let unknown: Vec<usize> = (0..w * h).filter(|i| !known[*i]).collect();
    for _ in 0..30 {
        for &i in &unknown {
            let (x, y) = (i % w, i / w);
            let mut s = [0.0f32; 4];
            let mut n = 0.0;
            for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                for c in 0..4 {
                    s[c] += vals[j][c];
                }
                n += 1.0;
            }
            if n > 0.0 {
                for c in 0..4 {
                    vals[i][c] = s[c] / n;
                }
            }
        }
    }
}

fn h_apply(h: &H3, p: [f64; 2]) -> [f64; 2] {
    vision::h_apply(h, p)
}

pub fn motion(ctx: &Ctx, frames: &[Frame], step_base: usize, total: usize) -> Vec<H3> {
    let n = frames.len();
    let mut out = vec![vision::IDENTITY; n];
    let grays: Vec<(Gray, Vec<bool>)> = frames
        .par_iter()
        .map(|f| {
            let mut g = Gray::from_img(&f.raw, 1);
            let mut hole = f.hole.clone();
            let (mut w, mut h) = (g.w, g.h);
            while w.max(h) > 960 {
                let half = g.half();
                let mut nh = vec![false; half.w * half.h];
                for y in 0..half.h {
                    for x in 0..half.w {
                        let (sx, sy) = ((x * 2).min(w - 1), (y * 2).min(h - 1));
                        nh[y * half.w + x] = hole[sy * w + sx] || hole[sy * w + (sx + 1).min(w - 1)] || hole[(sy + 1).min(h - 1) * w + sx];
                    }
                }
                hole = nh;
                g = half;
                w = g.w;
                h = g.h;
            }
            (g, hole)
        })
        .collect();
    let k = frames.first().map(|f| grays[0].0.w as f64 / f.raw.w.max(1) as f64).unwrap_or(1.0);
    let mut cum = vision::IDENTITY;
    for i in 1..n {
        if ctx.stopped() {
            break;
        }
        ctx.step(step_base + i, total, "Aligning frames");
        let (ga, ha) = &grays[i - 1];
        let (gb, _) = &grays[i];
        let near = dilate(ha, ga.w, ga.h, 6);
        let pts = vision::corners(ga, &|x, y| !near[(y as usize).min(ga.h - 1) * ga.w + (x as usize).min(ga.w - 1)], 400, (ga.w.max(ga.h) as f32 / 50.0).max(4.0));
        let pa = ga.pyramid(4);
        let pb = gb.pyramid(4);
        let moved = vision::klt(&pa, &pb, &pts, None, 7);
        let (mut s, mut d) = (vec![], vec![]);
        for (p, q) in pts.iter().zip(&moved) {
            if let Some(q) = q {
                s.push([p[0] as f64 / k, p[1] as f64 / k]);
                d.push([q[0] as f64 / k, q[1] as f64 / k]);
            }
        }
        let step = vision::ransac(Model::Homography, &s, &d, 1.5 / k, 300).map(|r| r.0).unwrap_or(vision::IDENTITY);
        cum = vision::h_mul(&cum, &vision::h_inv(&step).unwrap_or(vision::IDENTITY));
        out[i] = cum;
    }
    out
}

fn ring(hole: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let big = dilate(hole, w, h, r);
    big.iter().zip(hole).map(|(b, h)| *b && !*h).collect()
}

pub fn fill_frame(frames: &[Frame], to_ref: &[H3], k: usize, s: &Settings, reference: Option<(&Img, usize)>) -> Img {
    let f = &frames[k];
    let (w, h) = (f.raw.w, f.raw.h);
    let mut out = Img::new(w, h);
    if !f.hole.iter().any(|b| *b) {
        return out;
    }
    let edge = ring(&f.hole, w, h, 3);
    let want: Vec<usize> = (0..w * h).filter(|i| f.hole[*i] || edge[*i]).collect();
    let mut val: Vec<Option<[f32; 4]>> = vec![None; w * h];
    if s.method != 2 {
        let from_k = |j: usize| -> H3 { vision::h_mul(&vision::h_inv(&to_ref[j]).unwrap_or(vision::IDENTITY), &to_ref[k]) };
        let mut order: Vec<usize> = vec![];
        let n = frames.len();
        for d in 1..n {
            if k >= d {
                order.push(k - d);
            }
            if k + d < n {
                order.push(k + d);
            }
        }
        let take = |img: &Img, hole: Option<&Vec<bool>>, m: &H3, val: &mut Vec<Option<[f32; 4]>>, only_full: bool| -> usize {
            let picks: Vec<(usize, [f32; 4])> = want
                .par_iter()
                .filter(|i| val[**i].is_none())
                .filter_map(|&i| {
                    let p = [(i % w) as f64 + 0.5, (i / w) as f64 + 0.5];
                    let q = h_apply(m, p);
                    if q[0] < 0.5 || q[1] < 0.5 || q[0] > w as f64 - 0.5 || q[1] > h as f64 - 0.5 {
                        return None;
                    }
                    let qi = (q[1] as usize).min(h - 1) * w + (q[0] as usize).min(w - 1);
                    if hole.is_some_and(|hh| hh[qi]) {
                        return None;
                    }
                    let c = img.sample(q[0], q[1], true);
                    (c[3] > 0.5).then_some((i, c))
                })
                .collect();
            let n = picks.len();
            if only_full && n < want.iter().filter(|i| val[**i].is_none()).count() {
                return 0;
            }
            for (i, c) in picks {
                val[i] = Some(c);
            }
            n
        };
        if let Some((img, at)) = reference {
            take(img, None, &from_k(at), &mut val, false);
        }
        if s.method == 1 {
            for &j in &order {
                if take(&frames[j].raw, Some(&frames[j].hole), &from_k(j), &mut val, true) > 0 {
                    break;
                }
            }
        }
        for &j in &order {
            if want.iter().all(|i| val[*i].is_some()) {
                break;
            }
            take(&frames[j].raw, Some(&frames[j].hole), &from_k(j), &mut val, false);
        }
    }
    let mut fill: Vec<[f32; 4]> = (0..w * h).map(|i| val[i].unwrap_or([0.0; 4])).collect();
    let known: Vec<bool> = (0..w * h).map(|i| (!f.hole[i]) || val[i].is_some()).collect();
    for i in 0..w * h {
        if !f.hole[i] {
            let p = &f.raw.px[i * 4..i * 4 + 4];
            if val[i].is_none() || s.method == 2 {
                fill[i] = [p[0], p[1], p[2], p[3]];
            }
        }
    }
    if s.method == 2 {
        let kn: Vec<bool> = f.hole.iter().map(|b| !b).collect();
        membrane(&mut fill, &kn, w, h);
    } else {
        let mut diff: Vec<[f32; 4]> = vec![[0.0; 4]; w * h];
        let mut dk = vec![false; w * h];
        let mut mean = [0.0f32; 4];
        let mut cnt = 0.0f32;
        for i in 0..w * h {
            if edge[i]
                && let Some(v) = val[i]
            {
                let p = &f.raw.px[i * 4..i * 4 + 4];
                for c in 0..4 {
                    diff[i][c] = p[c] - v[c];
                    mean[c] += diff[i][c];
                }
                dk[i] = true;
                cnt += 1.0;
            }
        }
        if cnt > 0.0 {
            for m in mean.iter_mut() {
                *m /= cnt;
            }
        }
        let strength = [0.0f32, 0.35, 0.7, 1.0][s.lighting.min(3)];
        for i in 0..w * h {
            if !dk[i] && !f.hole[i] {
                dk[i] = true;
                diff[i] = mean;
            }
        }
        membrane(&mut diff, &dk, w, h);
        let rest: Vec<bool> = known.clone();
        for i in 0..w * h {
            if f.hole[i] && val[i].is_some() {
                for c in 0..4 {
                    let d = mean[c] + (diff[i][c] - mean[c]) * (0.5 + 0.5 * strength);
                    fill[i][c] += d;
                }
            }
        }
        membrane(&mut fill, &rest, w, h);
    }
    for i in 0..w * h {
        if f.hole[i] {
            let v = fill[i];
            let a = v[3].clamp(0.0, 1.0);
            out.px[i * 4] = v[0].clamp(0.0, a);
            out.px[i * 4 + 1] = v[1].clamp(0.0, a);
            out.px[i * 4 + 2] = v[2].clamp(0.0, a);
            out.px[i * 4 + 3] = a;
        }
    }
    out
}
