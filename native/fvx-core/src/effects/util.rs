use rayon::prelude::*;

use crate::render::image::Img;

pub fn box_h(img: &mut Img, r: usize, clamp: bool) {
    if r == 0 || img.is_empty() {
        return;
    }
    let w = img.w;
    let norm = 1.0 / (2 * r + 1) as f32;
    img.px.par_chunks_mut(w * 4).for_each(|row| {
        let src = row.to_vec();
        let at = |x: isize, k: usize| -> f32 {
            if x < 0 || x >= w as isize {
                if clamp {
                    src[x.clamp(0, w as isize - 1) as usize * 4 + k]
                } else {
                    0.0
                }
            } else {
                src[x as usize * 4 + k]
            }
        };
        for k in 0..4 {
            let mut acc = 0.0f32;
            for x in -(r as isize)..=(r as isize) {
                acc += at(x, k);
            }
            for x in 0..w {
                row[x * 4 + k] = acc * norm;
                acc += at(x as isize + r as isize + 1, k) - at(x as isize - r as isize, k);
            }
        }
    });
}

pub fn box_v(img: &mut Img, r: usize, clamp: bool) {
    if r == 0 || img.is_empty() {
        return;
    }
    let (w, h) = (img.w, img.h);
    let norm = 1.0 / (2 * r + 1) as f32;
    let src = img.px.clone();
    let cols: Vec<usize> = (0..w).collect();
    let results: Vec<Vec<f32>> = cols
        .par_iter()
        .map(|&x| {
            let at = |y: isize, k: usize| -> f32 {
                if y < 0 || y >= h as isize {
                    if clamp {
                        src[(y.clamp(0, h as isize - 1) as usize * w + x) * 4 + k]
                    } else {
                        0.0
                    }
                } else {
                    src[(y as usize * w + x) * 4 + k]
                }
            };
            let mut col = vec![0.0f32; h * 4];
            for k in 0..4 {
                let mut acc = 0.0f32;
                for y in -(r as isize)..=(r as isize) {
                    acc += at(y, k);
                }
                for y in 0..h {
                    col[y * 4 + k] = acc * norm;
                    acc += at(y as isize + r as isize + 1, k) - at(y as isize - r as isize, k);
                }
            }
            col
        })
        .collect();
    for (x, col) in results.iter().enumerate() {
        for y in 0..h {
            let i = (y * w + x) * 4;
            img.px[i..i + 4].copy_from_slice(&col[y * 4..y * 4 + 4]);
        }
    }
}

pub fn box_sizes(sigma: f64) -> [usize; 3] {
    let n = 3.0;
    let w_ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let m_ideal = (12.0 * sigma * sigma - n * (wl * wl) as f64 - 4.0 * n * wl as f64 - 3.0 * n) / (-4.0 * wl as f64 - 4.0);
    let m = m_ideal.round() as i64;
    let mut out = [0usize; 3];
    for (i, o) in out.iter_mut().enumerate() {
        let size = if (i as i64) < m { wl } else { wu };
        *o = ((size.max(1) - 1) / 2) as usize;
    }
    out
}

pub fn gaussian(img: &mut Img, sigma_x: f64, sigma_y: f64, clamp: bool) {
    if sigma_x > 0.3 {
        for r in box_sizes(sigma_x) {
            box_h(img, r, clamp);
        }
    }
    if sigma_y > 0.3 {
        for r in box_sizes(sigma_y) {
            box_v(img, r, clamp);
        }
    }
}

pub fn blur_scalar(data: &mut [f32], w: usize, h: usize, sigma_x: f64, sigma_y: f64) {
    if w == 0 || h == 0 {
        return;
    }
    let mut img = Img { w, h, px: data.iter().flat_map(|v| [*v, 0.0, 0.0, 0.0]).collect() };
    gaussian(&mut img, sigma_x, sigma_y, true);
    for (d, p) in data.iter_mut().zip(img.px.chunks_exact(4)) {
        *d = p[0];
    }
}

#[inline]
pub fn lum(r: f32, g: f32, b: f32) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

#[inline]
pub fn unpremul(p: &[f32]) -> [f32; 4] {
    let a = p[3];
    if a > 1e-6 { [p[0] / a, p[1] / a, p[2] / a, a] } else { [0.0, 0.0, 0.0, 0.0] }
}

#[inline]
pub fn premul(c: [f32; 4]) -> [f32; 4] {
    [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]
}

pub fn map_color(img: &mut Img, f: impl Fn([f32; 3]) -> [f32; 3] + Sync) {
    img.px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        if a <= 1e-6 {
            return;
        }
        let c = f([p[0] / a, p[1] / a, p[2] / a]);
        p[0] = c[0] * a;
        p[1] = c[1] * a;
        p[2] = c[2] * a;
    });
}

pub fn mix_with_original(orig: &Img, img: &mut Img, amount: f32) {
    if amount <= 0.0 {
        return;
    }
    let a = amount.min(1.0);
    img.px.par_iter_mut().zip(orig.px.par_iter()).for_each(|(v, o)| *v = *v + (*o - *v) * a);
}

pub fn rgb_to_hsl(c: [f32; 3]) -> [f32; 3] {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let l = (mx + mn) / 2.0;
    if (mx - mn).abs() < 1e-7 {
        return [0.0, 0.0, l];
    }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if mx == c[0] {
        (c[1] - c[2]) / d + if c[1] < c[2] { 6.0 } else { 0.0 }
    } else if mx == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    };
    [h / 6.0, s, l]
}

fn hue2rgb(p: f32, q: f32, t: f32) -> f32 {
    let t = t.rem_euclid(1.0);
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 0.5 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

pub fn hsl_to_rgb(h: [f32; 3]) -> [f32; 3] {
    let (hh, s, l) = (h[0], h[1], h[2]);
    if s <= 1e-7 {
        return [l, l, l];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    [hue2rgb(p, q, hh + 1.0 / 3.0), hue2rgb(p, q, hh), hue2rgb(p, q, hh - 1.0 / 3.0)]
}

#[inline]
pub fn hash3(x: i32, y: i32, z: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ (z as u32).wrapping_mul(0xCB1A_B31F) ^ seed.wrapping_mul(0x9E37_79B9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    h
}

#[inline]
pub fn rand01(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    (hash3(x, y, z, seed) >> 8) as f32 / (1u32 << 24) as f32
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn grad3(h: u32, x: f32, y: f32, z: f32) -> f32 {
    match h % 12 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        _ => -y - z,
    }
}

pub fn perlin3(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let (xi, yi, zi) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let (xf, yf, zf) = (x - xi as f32, y - yi as f32, z - zi as f32);
    let (u, v, w) = (fade(xf), fade(yf), fade(zf));
    let g = |dx: i32, dy: i32, dz: i32| grad3(hash3(xi + dx, yi + dy, zi + dz, seed), xf - dx as f32, yf - dy as f32, zf - dz as f32);
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = l(g(0, 0, 0), g(1, 0, 0), u);
    let x10 = l(g(0, 1, 0), g(1, 1, 0), u);
    let x01 = l(g(0, 0, 1), g(1, 0, 1), u);
    let x11 = l(g(0, 1, 1), g(1, 1, 1), u);
    l(l(x00, x10, v), l(x01, x11, v), w) * 0.9
}

pub fn value3(x: f32, y: f32, z: f32, seed: u32, smooth: bool) -> f32 {
    let (xi, yi, zi) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let (mut u, mut v, mut w) = (x - xi as f32, y - yi as f32, z - zi as f32);
    if smooth {
        u = fade(u);
        v = fade(v);
        w = fade(w);
    }
    let r = |dx: i32, dy: i32, dz: i32| rand01(xi + dx, yi + dy, zi + dz, seed) * 2.0 - 1.0;
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = l(r(0, 0, 0), r(1, 0, 0), u);
    let x10 = l(r(0, 1, 0), r(1, 1, 0), u);
    let x01 = l(r(0, 0, 1), r(1, 0, 1), u);
    let x11 = l(r(0, 1, 1), r(1, 1, 1), u);
    l(l(x00, x10, v), l(x01, x11, v), w)
}

pub fn sample_clamped(img: &Img, x: f64, y: f64) -> [f32; 4] {
    img.sample(x, y, true)
}

pub fn sample_clear(img: &Img, x: f64, y: f64) -> [f32; 4] {
    img.sample(x, y, false)
}

#[inline]
pub fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 <= e0 {
        return if x >= e1 { 1.0 } else { 0.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn herk(line: &[f32], out: &mut [f32], r: usize, max: bool) {
    let n = line.len();
    if r == 0 || n == 0 {
        out.copy_from_slice(line);
        return;
    }
    let k = 2 * r + 1;
    let pick = |a: f32, b: f32| if max { a.max(b) } else { a.min(b) };
    let ident = if max { f32::MIN } else { f32::MAX };
    let m = n + 2 * r;
    let get = |i: usize| -> f32 { if i < r || i >= r + n { ident } else { line[i - r] } };
    let mut g = vec![ident; m];
    let mut hh = vec![ident; m];
    for i in 0..m {
        g[i] = if i % k == 0 { get(i) } else { pick(g[i - 1], get(i)) };
    }
    for i in (0..m).rev() {
        hh[i] = if i % k == k - 1 || i == m - 1 { get(i) } else { pick(hh[i + 1], get(i)) };
    }
    for (x, o) in out.iter_mut().enumerate() {
        let a = x;
        let b = x + 2 * r;
        *o = pick(hh[a], g[b]);
    }
}

pub fn minmax(a: &[f32], w: usize, h: usize, r: usize, max: bool) -> Vec<f32> {
    if r == 0 {
        return a.to_vec();
    }
    let mut tmp = vec![0.0f32; w * h];
    tmp.par_chunks_mut(w).zip(a.par_chunks(w)).for_each(|(o, row)| herk(row, o, r, max));
    let mut out = vec![0.0f32; w * h];
    let cols: Vec<Vec<f32>> = (0..w)
        .into_par_iter()
        .map(|x| {
            let col: Vec<f32> = (0..h).map(|y| tmp[y * w + x]).collect();
            let mut o = vec![0.0f32; h];
            herk(&col, &mut o, r, max);
            o
        })
        .collect();
    for (x, col) in cols.iter().enumerate() {
        for y in 0..h {
            out[y * w + x] = col[y];
        }
    }
    out
}

pub fn fit_layer(other: &Img, osize: [f64; 2], c: &super::FxCtx, w: usize, h: usize, mode: u32) -> Img {
    let mut out = Img::new(w, h);
    let (sw, sh) = (c.size[0].max(1.0), c.size[1].max(1.0));
    let (ow, oh) = (osize[0].max(1.0), osize[1].max(1.0));
    let r = c.r;
    out.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let lx = (x as f64 + 0.5) / r + c.origin[0];
            let ly = (y as f64 + 0.5) / r + c.origin[1];
            let (ox, oy) = match mode {
                1 => (lx * ow / sw, ly * oh / sh),
                2 => ((lx - (sw - ow) / 2.0).rem_euclid(ow), (ly - (sh - oh) / 2.0).rem_euclid(oh)),
                _ => (lx - (sw - ow) / 2.0, ly - (sh - oh) / 2.0),
            };
            let s = if mode == 2 { other.sample(ox * r, oy * r, true) } else { other.sample(ox * r, oy * r, false) };
            row[x * 4..x * 4 + 4].copy_from_slice(&s);
        }
    });
    out
}

pub fn unsharp_into(img: &mut Img, blurred: &Img, amount: f32) {
    img.px.par_chunks_mut(4).zip(blurred.px.par_chunks(4)).for_each(|(p, b)| {
        for i in 0..3 {
            p[i] = (p[i] + (p[i] - b[i]) * amount).clamp(0.0, p[3].max(0.0));
        }
    });
}

pub fn mesh_render(src: &Img, cols: usize, rows: usize, from: &[[f64; 2]], to: &[[f64; 2]]) -> Img {
    let (w, h) = (src.w, src.h);
    let mut out = Img::new(w, h);
    let idx = |c: usize, r: usize| r * (cols + 1) + c;
    let mut tris: Vec<[usize; 3]> = Vec::with_capacity(cols * rows * 2);
    for r in 0..rows {
        for c in 0..cols {
            let (a, b, d, e) = (idx(c, r), idx(c + 1, r), idx(c, r + 1), idx(c + 1, r + 1));
            tris.push([a, b, e]);
            tris.push([a, e, d]);
        }
    }
    let rows_out: Vec<(usize, Vec<f32>)> = (0..h)
        .into_par_iter()
        .map(|y| {
            let mut row = vec![0.0f32; w * 4];
            let py = y as f64 + 0.5;
            for t in &tris {
                let (p0, p1, p2) = (to[t[0]], to[t[1]], to[t[2]]);
                let (ymin, ymax) = (p0[1].min(p1[1]).min(p2[1]), p0[1].max(p1[1]).max(p2[1]));
                if py < ymin || py > ymax {
                    continue;
                }
                let xmin = p0[0].min(p1[0]).min(p2[0]).floor().max(0.0) as usize;
                let xmax = (p0[0].max(p1[0]).max(p2[0]).ceil() as isize).min(w as isize - 1);
                if xmax < 0 {
                    continue;
                }
                let den = (p1[1] - p2[1]) * (p0[0] - p2[0]) + (p2[0] - p1[0]) * (p0[1] - p2[1]);
                if den.abs() < 1e-12 {
                    continue;
                }
                for x in xmin..=(xmax as usize) {
                    let px = x as f64 + 0.5;
                    let l0 = ((p1[1] - p2[1]) * (px - p2[0]) + (p2[0] - p1[0]) * (py - p2[1])) / den;
                    let l1 = ((p2[1] - p0[1]) * (px - p2[0]) + (p0[0] - p2[0]) * (py - p2[1])) / den;
                    let l2 = 1.0 - l0 - l1;
                    if l0 < -1e-9 || l1 < -1e-9 || l2 < -1e-9 {
                        continue;
                    }
                    let (s0, s1, s2) = (from[t[0]], from[t[1]], from[t[2]]);
                    let sx = s0[0] * l0 + s1[0] * l1 + s2[0] * l2;
                    let sy = s0[1] * l0 + s1[1] * l1 + s2[1] * l2;
                    let c = src.sample(sx, sy, false);
                    row[x * 4..x * 4 + 4].copy_from_slice(&c);
                }
            }
            (y, row)
        })
        .collect();
    for (y, row) in rows_out {
        out.px[y * w * 4..(y + 1) * w * 4].copy_from_slice(&row);
    }
    out
}

pub fn grid_points(rect: [f64; 4], cols: usize, rows: usize) -> Vec<[f64; 2]> {
    let mut v = Vec::with_capacity((cols + 1) * (rows + 1));
    for r in 0..=rows {
        for c in 0..=cols {
            v.push([rect[0] + (rect[2] - rect[0]) * c as f64 / cols as f64, rect[1] + (rect[3] - rect[1]) * r as f64 / rows as f64]);
        }
    }
    v
}

pub fn remap(img: &mut Img, clamp: bool, f: impl Fn(f64, f64) -> Option<[f64; 2]> + Sync) {
    let src = img.clone();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let c = match f(x as f64 + 0.5, y as f64 + 0.5) {
                Some(s) => src.sample(s[0], s[1], clamp),
                None => [0.0; 4],
            };
            row[x * 4..x * 4 + 4].copy_from_slice(&c);
        }
    });
}
