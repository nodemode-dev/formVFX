use rayon::prelude::*;

use crate::geom::Mat3;
use crate::model::BezPath;
use crate::render::image::Img;
use crate::render::vector;

pub type Seg = ([f64; 2], [f64; 2], f64);

#[inline]
pub fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
    (p[0] - a[0] - dx * t).hypot(p[1] - a[1] - dy * t)
}

pub fn segments_cov(w: usize, h: usize, segs: &[Seg], hardness: f64) -> Vec<f32> {
    let mut cov = vec![0.0f32; w * h];
    if segs.is_empty() {
        return cov;
    }
    let soft = (1.0 - hardness.clamp(0.0, 1.0)).max(0.02);
    cov.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let py = y as f64 + 0.5;
        for s in segs {
            let r = s.2 * 0.5 + 1.0;
            if py < s.0[1].min(s.1[1]) - r || py > s.0[1].max(s.1[1]) + r {
                continue;
            }
            let x0 = (s.0[0].min(s.1[0]) - r).floor().max(0.0) as usize;
            let x1 = ((s.0[0].max(s.1[0]) + r).ceil() as isize).min(w as isize - 1);
            if x1 < 0 {
                continue;
            }
            for x in x0..=(x1 as usize) {
                let d = seg_dist([x as f64 + 0.5, py], s.0, s.1);
                let half = s.2 * 0.5;
                let inner = half * (1.0 - soft);
                let v = if d <= inner { 1.0 } else if d >= half + 0.5 { 0.0 } else { 1.0 - ((d - inner) / (half + 0.5 - inner)).clamp(0.0, 1.0) };
                if v as f32 > row[x] {
                    row[x] = v as f32;
                }
            }
        }
    });
    cov
}

pub fn path_fill(paths: &[BezPath], w: usize, h: usize) -> Vec<f32> {
    vector::coverage(paths, &Mat3::IDENTITY, w, h, 0, 0, false, 0.0)
}

pub fn path_stroke(paths: &[BezPath], w: usize, h: usize, width: f64) -> Vec<f32> {
    vector::coverage(paths, &Mat3::IDENTITY, w, h, 0, 0, false, width.max(0.1))
}

pub fn polyline(p: &BezPath, tol: f64) -> Vec<[f64; 2]> {
    let mut pts = p.flatten(tol);
    if p.closed && pts.len() > 1 && pts[0] != pts[pts.len() - 1] {
        pts.push(pts[0]);
    }
    pts
}

pub fn trim(pts: &[[f64; 2]], start: f64, end: f64) -> Vec<[f64; 2]> {
    if pts.len() < 2 {
        return pts.to_vec();
    }
    let mut len = vec![0.0];
    for i in 1..pts.len() {
        len.push(len[i - 1] + (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1]));
    }
    let total = len[len.len() - 1];
    let (a, b) = (start.clamp(0.0, 1.0) * total, end.clamp(0.0, 1.0) * total);
    if b <= a {
        return vec![];
    }
    let at = |s: f64| -> [f64; 2] {
        let i = len.partition_point(|x| *x < s).clamp(1, len.len() - 1);
        let seg = (len[i] - len[i - 1]).max(1e-9);
        let t = ((s - len[i - 1]) / seg).clamp(0.0, 1.0);
        [pts[i - 1][0] + (pts[i][0] - pts[i - 1][0]) * t, pts[i - 1][1] + (pts[i][1] - pts[i - 1][1]) * t]
    };
    let mut out = vec![at(a)];
    for i in 0..pts.len() {
        if len[i] > a && len[i] < b {
            out.push(pts[i]);
        }
    }
    out.push(at(b));
    out
}

pub fn segs_of(pts: &[[f64; 2]], width: f64) -> Vec<Seg> {
    pts.windows(2).map(|w| (w[0], w[1], width)).collect()
}

pub fn paint(img: &mut Img, cov: &[f32], color: [f32; 4], opacity: f32, style: u32) {
    img.px.par_chunks_mut(4).zip(cov.par_iter()).for_each(|(p, c)| {
        let a = (c * opacity * color[3]).clamp(0.0, 1.0);
        match style {
            1 => {
                p[0] = color[0] * a;
                p[1] = color[1] * a;
                p[2] = color[2] * a;
                p[3] = a;
            }
            2 => {
                for v in p.iter_mut() {
                    *v *= c.clamp(0.0, 1.0);
                }
            }
            _ => {
                let k = 1.0 - a;
                p[0] = color[0] * a + p[0] * k;
                p[1] = color[1] * a + p[1] * k;
                p[2] = color[2] * a + p[2] * k;
                p[3] = a + p[3] * k;
            }
        }
    });
}

pub fn add_light(img: &mut Img, cov: &[f32], color: [f32; 3], k: f32) {
    img.px.par_chunks_mut(4).zip(cov.par_iter()).for_each(|(p, c)| {
        let v = c * k;
        if v <= 0.0 {
            return;
        }
        for i in 0..3 {
            p[i] += color[i] * v;
        }
        p[3] = (p[3] + v).min(1.0);
    });
}

pub fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        let (wr, wi) = (ang.cos(), ang.sin());
        for i in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0, 0.0);
            for k in 0..len / 2 {
                let (ur, ui) = (re[i + k], im[i + k]);
                let (vr, vi) = (re[i + k + len / 2] * cr - im[i + k + len / 2] * ci, re[i + k + len / 2] * ci + im[i + k + len / 2] * cr);
                re[i + k] = ur + vr;
                im[i + k] = ui + vi;
                re[i + k + len / 2] = ur - vr;
                im[i + k + len / 2] = ui - vi;
                let t = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = t;
            }
        }
        len <<= 1;
    }
}

pub fn contour(alpha: &[f32], w: usize, h: usize, th: f32) -> Vec<Vec<[f64; 2]>> {
    let mut segs: Vec<([f64; 2], [f64; 2])> = vec![];
    let v = |x: usize, y: usize| alpha[y * w + x];
    let lerp = |a: f32, b: f32| ((th - a) / (b - a)).clamp(0.0, 1.0) as f64;
    for y in 0..h.saturating_sub(1) {
        for x in 0..w.saturating_sub(1) {
            let (a, b, c, d) = (v(x, y), v(x + 1, y), v(x + 1, y + 1), v(x, y + 1));
            let idx = (a >= th) as u8 | ((b >= th) as u8) << 1 | ((c >= th) as u8) << 2 | ((d >= th) as u8) << 3;
            if idx == 0 || idx == 15 {
                continue;
            }
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            let top = [fx + lerp(a, b), fy];
            let right = [fx + 1.0, fy + lerp(b, c)];
            let bottom = [fx + lerp(d, c), fy + 1.0];
            let left = [fx, fy + lerp(a, d)];
            let mut add = |p: [f64; 2], q: [f64; 2]| segs.push((p, q));
            match idx {
                1 | 14 => add(left, top),
                2 | 13 => add(top, right),
                3 | 12 => add(left, right),
                4 | 11 => add(right, bottom),
                5 => {
                    add(left, top);
                    add(right, bottom);
                }
                6 | 9 => add(top, bottom),
                7 | 8 => add(left, bottom),
                10 => {
                    add(top, right);
                    add(left, bottom);
                }
                _ => {}
            }
        }
    }
    let mut lines: Vec<Vec<[f64; 2]>> = vec![];
    let mut used = vec![false; segs.len()];
    let key = |p: [f64; 2]| ((p[0] * 64.0).round() as i64, (p[1] * 64.0).round() as i64);
    let mut by: std::collections::HashMap<(i64, i64), Vec<usize>> = std::collections::HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        by.entry(key(s.0)).or_default().push(i);
        by.entry(key(s.1)).or_default().push(i);
    }
    for i in 0..segs.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let mut line = vec![segs[i].0, segs[i].1];
        loop {
            let end = *line.last().unwrap_or(&[0.0, 0.0]);
            let next = by.get(&key(end)).and_then(|v| v.iter().copied().find(|j| !used[*j]));
            let Some(j) = next else { break };
            used[j] = true;
            let s = segs[j];
            line.push(if key(s.0) == key(end) { s.1 } else { s.0 });
        }
        if line.len() > 2 {
            lines.push(line);
        }
    }
    lines
}
