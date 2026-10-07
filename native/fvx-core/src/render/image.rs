use rayon::prelude::*;

use crate::geom::Mat3;

#[derive(Clone, Debug, Default)]
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub px: Vec<f32>,
}

impl Img {
    pub fn new(w: usize, h: usize) -> Img {
        Img { w, h, px: vec![0.0; w * h * 4] }
    }

    pub fn filled(w: usize, h: usize, c: [f32; 4]) -> Img {
        let mut px = Vec::with_capacity(w * h * 4);
        for _ in 0..w * h {
            px.extend_from_slice(&c);
        }
        Img { w, h, px }
    }

    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    #[inline]
    pub fn get(&self, x: isize, y: isize) -> [f32; 4] {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return [0.0; 4];
        }
        let i = (y as usize * self.w + x as usize) * 4;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }

    #[inline]
    pub fn get_clamped(&self, x: isize, y: isize) -> [f32; 4] {
        if self.is_empty() {
            return [0.0; 4];
        }
        let x = x.clamp(0, self.w as isize - 1) as usize;
        let y = y.clamp(0, self.h as isize - 1) as usize;
        let i = (y * self.w + x) * 4;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }

    #[inline]
    pub fn sample(&self, x: f64, y: f64, clamp: bool) -> [f32; 4] {
        let fx = x - 0.5;
        let fy = y - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor();
        let tx = (fx - x0) as f32;
        let ty = (fy - y0) as f32;
        let (xi, yi) = (x0 as isize, y0 as isize);
        let g = |a: isize, b: isize| if clamp { self.get_clamped(a, b) } else { self.get(a, b) };
        let a = g(xi, yi);
        let b = g(xi + 1, yi);
        let c = g(xi, yi + 1);
        let d = g(xi + 1, yi + 1);
        let mut o = [0.0f32; 4];
        for k in 0..4 {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bot = c[k] + (d[k] - c[k]) * tx;
            o[k] = top + (bot - top) * ty;
        }
        o
    }

    pub fn from_rgba8(w: usize, h: usize, data: &[u8], premultiplied: bool) -> Img {
        let mut px = vec![0.0f32; w * h * 4];
        px.par_chunks_mut(4).zip(data.par_chunks(4)).for_each(|(o, s)| {
            let a = s[3] as f32 / 255.0;
            if premultiplied {
                o[0] = s[0] as f32 / 255.0;
                o[1] = s[1] as f32 / 255.0;
                o[2] = s[2] as f32 / 255.0;
            } else {
                o[0] = s[0] as f32 / 255.0 * a;
                o[1] = s[1] as f32 / 255.0 * a;
                o[2] = s[2] as f32 / 255.0 * a;
            }
            o[3] = a;
        });
        Img { w, h, px }
    }

    pub fn from_rgba16(w: usize, h: usize, data: &[u16]) -> Img {
        let mut px = vec![0.0f32; w * h * 4];
        px.par_chunks_mut(4).zip(data.par_chunks(4)).for_each(|(o, s)| {
            let a = s[3] as f32 / 65535.0;
            o[0] = s[0] as f32 / 65535.0 * a;
            o[1] = s[1] as f32 / 65535.0 * a;
            o[2] = s[2] as f32 / 65535.0 * a;
            o[3] = a;
        });
        Img { w, h, px }
    }

    pub fn from_pixmap(pm: &tiny_skia::Pixmap) -> Img {
        Img::from_rgba8(pm.width() as usize, pm.height() as usize, pm.data(), true)
    }

    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.w * self.h * 4];
        out.par_chunks_mut(4).zip(self.px.par_chunks(4)).for_each(|(o, p)| {
            let a = p[3].clamp(0.0, 1.0);
            if a > 1e-6 {
                o[0] = to8(p[0] / a);
                o[1] = to8(p[1] / a);
                o[2] = to8(p[2] / a);
            }
            o[3] = to8(a);
        });
        out
    }

    pub fn to_rgba16(&self) -> Vec<u16> {
        let to16 = |v: f32| (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
        let mut out = vec![0u16; self.w * self.h * 4];
        out.par_chunks_mut(4).zip(self.px.par_chunks(4)).for_each(|(o, p)| {
            let a = p[3].clamp(0.0, 1.0);
            if a > 1e-6 {
                o[0] = to16(p[0] / a);
                o[1] = to16(p[1] / a);
                o[2] = to16(p[2] / a);
            }
            o[3] = to16(a);
        });
        out
    }

    pub fn flattened(&self, bg: [f32; 3]) -> Img {
        let mut out = self.clone();
        out.px.par_chunks_mut(4).for_each(|p| {
            let k = 1.0 - p[3].clamp(0.0, 1.0);
            p[0] += bg[0] * k;
            p[1] += bg[1] * k;
            p[2] += bg[2] * k;
            p[3] = 1.0;
        });
        out
    }

    pub fn to_rgba8_over(&self, bg: [f32; 3]) -> Vec<u8> {
        let mut out = vec![0u8; self.w * self.h * 4];
        out.par_chunks_mut(4).zip(self.px.par_chunks(4)).for_each(|(o, p)| {
            let a = p[3].clamp(0.0, 1.0);
            o[0] = to8(p[0] + bg[0] * (1.0 - a));
            o[1] = to8(p[1] + bg[1] * (1.0 - a));
            o[2] = to8(p[2] + bg[2] * (1.0 - a));
            o[3] = 255;
        });
        out
    }

    pub fn half(&self) -> Img {
        let w = (self.w / 2).max(1);
        let h = (self.h / 2).max(1);
        let mut out = Img::new(w, h);
        out.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                let mut acc = [0.0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = self.get_clamped((x * 2 + dx) as isize, (y * 2 + dy) as isize);
                    for k in 0..4 {
                        acc[k] += p[k];
                    }
                }
                for k in 0..4 {
                    row[x * 4 + k] = acc[k] * 0.25;
                }
            }
        });
        out
    }

    pub fn crop(&self, x0: isize, y0: isize, w: usize, h: usize) -> Img {
        let mut out = Img::new(w, h);
        out.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                let p = self.get(x0 + x as isize, y0 + y as isize);
                row[x * 4..x * 4 + 4].copy_from_slice(&p);
            }
        });
        out
    }

    pub fn pad(&self, p: usize) -> Img {
        if p == 0 {
            return self.clone();
        }
        let mut out = Img::new(self.w + 2 * p, self.h + 2 * p);
        for y in 0..self.h {
            let src = &self.px[y * self.w * 4..(y + 1) * self.w * 4];
            let di = ((y + p) * out.w + p) * 4;
            out.px[di..di + self.w * 4].copy_from_slice(src);
        }
        out
    }

    pub fn multiply_alpha(&mut self, cov: &[f32]) {
        self.px.par_chunks_mut(4).zip(cov.par_iter()).for_each(|(p, c)| {
            for v in p.iter_mut() {
                *v *= *c;
            }
        });
    }

    pub fn scale_all(&mut self, k: f32) {
        if (k - 1.0).abs() < 1e-6 {
            return;
        }
        self.px.par_iter_mut().for_each(|v| *v *= k);
    }
}

#[inline]
pub fn to8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[derive(Clone, Debug, Default)]
pub struct Tile {
    pub x0: i64,
    pub y0: i64,
    pub img: Img,
}

impl Tile {
    pub fn full(img: Img) -> Tile {
        Tile { x0: 0, y0: 0, img }
    }
}

thread_local! {
    static NEAREST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn with_nearest<R>(on: bool, f: impl FnOnce() -> R) -> R {
    let prev = NEAREST.with(|n| n.replace(on));
    let r = f();
    NEAREST.with(|n| n.set(prev));
    r
}

pub fn warp(src: &Img, m: &Mat3, dst_w: usize, dst_h: usize, clamp_edges: bool) -> Option<Tile> {
    if src.is_empty() {
        return None;
    }
    let nearest = NEAREST.with(|n| n.get());
    let corners = [m.apply(0.0, 0.0), m.apply(src.w as f64, 0.0), m.apply(src.w as f64, src.h as f64), m.apply(0.0, src.h as f64)];
    if !m.is_affine() {
        let w = &m.0[2];
        let ok = [(0.0, 0.0), (src.w as f64, 0.0), (src.w as f64, src.h as f64), (0.0, src.h as f64)].iter().any(|(x, y)| w[0] * x + w[1] * y + w[2] > 0.0);
        if !ok {
            return None;
        }
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for c in corners {
        x0 = x0.min(c[0]);
        y0 = y0.min(c[1]);
        x1 = x1.max(c[0]);
        y1 = y1.max(c[1]);
    }
    if !m.is_affine() {
        x0 = x0.max(0.0);
        y0 = y0.max(0.0);
        x1 = x1.min(dst_w as f64);
        y1 = y1.min(dst_h as f64);
    }
    let bx0 = (x0.floor() as i64 - 1).max(0);
    let by0 = (y0.floor() as i64 - 1).max(0);
    let bx1 = (x1.ceil() as i64 + 1).min(dst_w as i64);
    let by1 = (y1.ceil() as i64 + 1).min(dst_h as i64);
    if bx1 <= bx0 || by1 <= by0 {
        return None;
    }
    let inv = m.inverse()?;
    let mut mip: Option<Img> = None;
    let mut lscale = 1.0;
    if m.is_affine() && !nearest {
        let mut s = m.det().abs().sqrt();
        while s < 0.5 {
            let base = mip.as_ref().unwrap_or(src);
            if base.w <= 2 || base.h <= 2 {
                break;
            }
            let next = base.half();
            mip = Some(next);
            lscale *= 0.5;
            s *= 2.0;
        }
    }
    let level = mip.as_ref().unwrap_or(src);
    let w = (bx1 - bx0) as usize;
    let h = (by1 - by0) as usize;
    let mut out = Img::new(w, h);
    let (sw, sh) = (level.w as f64, level.h as f64);
    out.px.par_chunks_mut(w * 4).enumerate().for_each(|(yy, row)| {
        let py = by0 as f64 + yy as f64 + 0.5;
        for xx in 0..w {
            let px = bx0 as f64 + xx as f64 + 0.5;
            let s = inv.apply(px, py);
            if !m.is_affine() {
                let ww = inv.0[2][0] * px + inv.0[2][1] * py + inv.0[2][2];
                if ww <= 0.0 {
                    continue;
                }
            }
            let sx = s[0] * lscale;
            let sy = s[1] * lscale;
            if !clamp_edges && (sx < -1.0 || sy < -1.0 || sx > sw + 1.0 || sy > sh + 1.0) {
                continue;
            }
            let c = if nearest {
                if !clamp_edges && (sx < 0.0 || sy < 0.0 || sx >= sw || sy >= sh) {
                    continue;
                }
                level.get_clamped(sx.floor() as isize, sy.floor() as isize)
            } else {
                level.sample(sx, sy, clamp_edges)
            };
            row[xx * 4..xx * 4 + 4].copy_from_slice(&c);
        }
    });
    Some(Tile { x0: bx0, y0: by0, img: out })
}
