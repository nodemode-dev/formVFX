use rayon::prelude::*;

use super::image::{Img, Tile};
use crate::model::{BlendMode, MatteKind};

#[inline]
fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

#[inline]
fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut o = c;
    if n < 0.0 {
        let d = l - n;
        if d > 1e-6 {
            for v in o.iter_mut() {
                *v = l + (*v - l) * l / d;
            }
        }
    }
    if x > 1.0 {
        let d = x - l;
        if d > 1e-6 {
            for v in o.iter_mut() {
                *v = l + (*v - l) * (1.0 - l) / d;
            }
        }
    }
    o
}

#[inline]
fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color([c[0] + d, c[1] + d, c[2] + d])
}

#[inline]
fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let mut idx = [0usize, 1, 2];
    idx.sort_by(|a, b| c[*a].total_cmp(&c[*b]));
    let (mn, md, mx) = (idx[0], idx[1], idx[2]);
    let mut o = [0.0f32; 3];
    if c[mx] > c[mn] {
        o[md] = (c[md] - c[mn]) * s / (c[mx] - c[mn]);
        o[mx] = s;
    }
    o[mn] = 0.0;
    o
}

#[inline]
fn soft_d(x: f32) -> f32 {
    if x <= 0.25 { ((16.0 * x - 12.0) * x + 4.0) * x } else { x.sqrt() }
}

#[inline]
fn burn(b: f32, s: f32) -> f32 {
    if b >= 1.0 {
        1.0
    } else if s <= 0.0 {
        0.0
    } else {
        1.0 - ((1.0 - b) / s).min(1.0)
    }
}

#[inline]
fn dodge(b: f32, s: f32) -> f32 {
    if b <= 0.0 {
        0.0
    } else if s >= 1.0 {
        1.0
    } else {
        (b / (1.0 - s)).min(1.0)
    }
}

fn separable(mode: BlendMode, b: f32, s: f32) -> f32 {
    match mode {
        BlendMode::Darken => b.min(s),
        BlendMode::Multiply => b * s,
        BlendMode::ColorBurn | BlendMode::ClassicColorBurn => burn(b, s),
        BlendMode::LinearBurn => (b + s - 1.0).max(0.0),
        BlendMode::Add | BlendMode::LinearDodge => (b + s).min(1.0),
        BlendMode::Lighten => b.max(s),
        BlendMode::Screen => b + s - b * s,
        BlendMode::ColorDodge | BlendMode::ClassicColorDodge => dodge(b, s),
        BlendMode::Overlay => {
            if b <= 0.5 {
                2.0 * b * s
            } else {
                1.0 - 2.0 * (1.0 - b) * (1.0 - s)
            }
        }
        BlendMode::SoftLight => {
            if s <= 0.5 {
                b - (1.0 - 2.0 * s) * b * (1.0 - b)
            } else {
                b + (2.0 * s - 1.0) * (soft_d(b) - b)
            }
        }
        BlendMode::HardLight => {
            if s <= 0.5 {
                2.0 * b * s
            } else {
                let s2 = 2.0 * s - 1.0;
                b + s2 - b * s2
            }
        }
        BlendMode::LinearLight => (b + 2.0 * s - 1.0).clamp(0.0, 1.0),
        BlendMode::VividLight => {
            if s <= 0.5 {
                burn(b, 2.0 * s)
            } else {
                dodge(b, 2.0 * (s - 0.5))
            }
        }
        BlendMode::PinLight => {
            if s <= 0.5 {
                b.min(2.0 * s)
            } else {
                b.max(2.0 * s - 1.0)
            }
        }
        BlendMode::HardMix => {
            if b + s >= 1.0 {
                1.0
            } else {
                0.0
            }
        }
        BlendMode::Difference | BlendMode::ClassicDifference => (b - s).abs(),
        BlendMode::Exclusion => b + s - 2.0 * b * s,
        BlendMode::Subtract => (b - s).max(0.0),
        BlendMode::Divide => {
            if s <= 0.0 {
                1.0
            } else {
                (b / s).min(1.0)
            }
        }
        _ => s,
    }
}

fn blend_color(mode: BlendMode, cb: [f32; 3], cs: [f32; 3]) -> [f32; 3] {
    match mode {
        BlendMode::Hue => set_lum(set_sat(cs, sat(cb)), lum(cb)),
        BlendMode::Saturation => set_lum(set_sat(cb, sat(cs)), lum(cb)),
        BlendMode::Color => set_lum(cs, lum(cb)),
        BlendMode::Luminosity => set_lum(cb, lum(cs)),
        BlendMode::DarkerColor => {
            if lum(cs) < lum(cb) {
                cs
            } else {
                cb
            }
        }
        BlendMode::LighterColor => {
            if lum(cs) > lum(cb) {
                cs
            } else {
                cb
            }
        }
        _ => [separable(mode, cb[0], cs[0]), separable(mode, cb[1], cs[1]), separable(mode, cb[2], cs[2])],
    }
}

#[inline]
fn hash(x: i64, y: i64) -> f32 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

pub fn composite(dst: &mut Img, src: &Tile, mode: BlendMode, opacity: f32, preserve: bool, frame: i64) {
    let seed = if mode == BlendMode::DancingDissolve { frame.wrapping_mul(104_729) } else { 0 };
    if opacity <= 0.0 || src.img.is_empty() {
        return;
    }
    let dw = dst.w as i64;
    let dh = dst.h as i64;
    let x0 = src.x0.max(0);
    let y0 = src.y0.max(0);
    let x1 = (src.x0 + src.img.w as i64).min(dw);
    let y1 = (src.y0 + src.img.h as i64).min(dh);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let sw = src.img.w;
    let stride = dst.w * 4;
    let rows: Vec<(usize, &mut [f32])> = dst.px.chunks_mut(stride).enumerate().filter(|(y, _)| (*y as i64) >= y0 && (*y as i64) < y1).collect();
    rows.into_par_iter().for_each(|(y, row)| {
        let sy = (y as i64 - src.y0) as usize;
        for x in x0..x1 {
            let sx = (x - src.x0) as usize;
            let si = (sy * sw + sx) * 4;
            let mut s = [src.img.px[si], src.img.px[si + 1], src.img.px[si + 2], src.img.px[si + 3]];
            if s[3] <= 0.0 && !matches!(mode, BlendMode::StencilAlpha | BlendMode::StencilLuma) {
                continue;
            }
            for v in s.iter_mut() {
                *v *= opacity;
            }
            let di = x as usize * 4;
            let d = [row[di], row[di + 1], row[di + 2], row[di + 3]];
            let out = blend_px(mode, d, s, x, y as i64 + seed);
            let out = if preserve { [out[0] * pres(d[3], out[3]), out[1] * pres(d[3], out[3]), out[2] * pres(d[3], out[3]), d[3]] } else { out };
            row[di..di + 4].copy_from_slice(&out);
        }
    });
    if matches!(mode, BlendMode::StencilAlpha | BlendMode::StencilLuma) {
        let (tx0, ty0, tx1, ty1) = (src.x0, src.y0, src.x0 + src.img.w as i64, src.y0 + src.img.h as i64);
        dst.px.par_chunks_mut(stride).enumerate().for_each(|(y, row)| {
            let y = y as i64;
            for x in 0..dw {
                if y >= ty0 && y < ty1 && x >= tx0 && x < tx1 {
                    continue;
                }
                let di = x as usize * 4;
                for k in 0..4 {
                    row[di + k] = 0.0;
                }
            }
        });
    }
}

#[inline]
fn pres(da: f32, oa: f32) -> f32 {
    if oa > 1e-6 { da / oa } else { 0.0 }
}

#[inline]
pub fn blend_px(mode: BlendMode, d: [f32; 4], s: [f32; 4], x: i64, y: i64) -> [f32; 4] {
    let (sa, da) = (s[3].min(1.0), d[3].min(1.0));
    match mode {
        BlendMode::Normal => [s[0] + d[0] * (1.0 - sa), s[1] + d[1] * (1.0 - sa), s[2] + d[2] * (1.0 - sa), sa + da * (1.0 - sa)],
        BlendMode::Dissolve | BlendMode::DancingDissolve => {
            if hash(x, y) < sa {
                let k = 1.0 / sa.max(1e-6);
                [s[0] * k, s[1] * k, s[2] * k, 1.0]
            } else {
                d
            }
        }
        BlendMode::StencilAlpha => [d[0] * sa, d[1] * sa, d[2] * sa, da * sa],
        BlendMode::StencilLuma => {
            let l = lum([s[0], s[1], s[2]]);
            [d[0] * l, d[1] * l, d[2] * l, da * l]
        }
        BlendMode::SilhouetteAlpha => [d[0] * (1.0 - sa), d[1] * (1.0 - sa), d[2] * (1.0 - sa), da * (1.0 - sa)],
        BlendMode::SilhouetteLuma => {
            let l = lum([s[0], s[1], s[2]]);
            [d[0] * (1.0 - l), d[1] * (1.0 - l), d[2] * (1.0 - l), da * (1.0 - l)]
        }
        BlendMode::AlphaAdd => [s[0] + d[0] * (1.0 - sa), s[1] + d[1] * (1.0 - sa), s[2] + d[2] * (1.0 - sa), (sa + da).min(1.0)],
        BlendMode::LuminescentPremul => [(s[0] + d[0]).min(1.0), (s[1] + d[1]).min(1.0), (s[2] + d[2]).min(1.0), sa + da * (1.0 - sa)],
        BlendMode::Add | BlendMode::LinearDodge if da >= 0.999 => [(s[0] + d[0]).min(1.0), (s[1] + d[1]).min(1.0), (s[2] + d[2]).min(1.0), 1.0],
        _ => {
            let cs = if sa > 1e-6 { [s[0] / sa, s[1] / sa, s[2] / sa] } else { [0.0; 3] };
            let cb = if da > 1e-6 { [d[0] / da, d[1] / da, d[2] / da] } else { [0.0; 3] };
            let b = blend_color(mode, cb, cs);
            let mut o = [0.0f32; 4];
            for k in 0..3 {
                o[k] = (1.0 - sa) * d[k] + (1.0 - da) * s[k] + sa * da * b[k];
            }
            o[3] = sa + da * (1.0 - sa);
            o
        }
    }
}

pub fn apply_matte(layer: &mut Tile, matte: Option<&Tile>, kind: MatteKind) {
    let lw = layer.img.w;
    let x0 = layer.x0;
    let y0 = layer.y0;
    layer.img.px.par_chunks_mut(lw * 4).enumerate().for_each(|(yy, row)| {
        let y = y0 + yy as i64;
        for xx in 0..lw {
            let x = x0 + xx as i64;
            let m = match matte {
                Some(t) => {
                    let mx = x - t.x0;
                    let my = y - t.y0;
                    if mx < 0 || my < 0 || mx >= t.img.w as i64 || my >= t.img.h as i64 {
                        [0.0; 4]
                    } else {
                        let i = (my as usize * t.img.w + mx as usize) * 4;
                        [t.img.px[i], t.img.px[i + 1], t.img.px[i + 2], t.img.px[i + 3]]
                    }
                }
                None => [0.0; 4],
            };
            let f = match kind {
                MatteKind::Alpha => m[3],
                MatteKind::AlphaInverted => 1.0 - m[3],
                MatteKind::Luma => lum([m[0], m[1], m[2]]),
                MatteKind::LumaInverted => 1.0 - lum([m[0], m[1], m[2]]),
            }
            .clamp(0.0, 1.0);
            for k in 0..4 {
                row[xx * 4 + k] *= f;
            }
        }
    });
}
