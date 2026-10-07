use rayon::prelude::*;

use super::util::hash3;
use super::{EffectSpec, FxCtx, angle, check, num, point, popup};
use crate::render::image::Img;

pub fn specs() -> Vec<EffectSpec> {
    vec![EffectSpec {
        id: "fx.noise.curlNoise",
        name: "Curl Noise Generator",
        category: "Noise & Grain",
        params: vec![
            popup("inputNoise", "Input Noise", 0, &["Internal", "This Layer"]),
            num("speed", "Speed", 10.0, 0.0, 100.0, 0.0, 100.0),
            angle("direction", "Direction", 0.0),
            check("mirrorAtEdge", "Mirror at Edge", false),
            num("scale", "Scale", 100.0, 1.0, 10000.0, 1.0, 1000.0),
            check("uniformScaling", "Uniform Scaling", true),
            num("scaleWidth", "Scale Width", 100.0, 1.0, 10000.0, 1.0, 1000.0),
            num("scaleHeight", "Scale Height", 100.0, 1.0, 10000.0, 1.0, 1000.0),
            point("offset", "Offset", 0.5, 0.5),
            angle("rotation", "Rotation", 0.0),
            angle("evolution", "Evolution", 0.0),
            num("turbulenceSpeed", "Turbulence Speed", 10.0, 0.0, 500.0, 0.0, 500.0),
            num("noiseSpeed", "Noise Speed", 10.0, 0.0, 500.0, 0.0, 100.0),
            num("swirl", "Swirl", 90.0, -360.0, 360.0, -360.0, 360.0),
            num("density", "Density", 0.0, -100.0, 100.0, -100.0, 100.0),
            num("smoothness", "Smoothness", 50.0, 0.0, 100.0, 0.0, 100.0),
            num("verticalBias", "Vertical Bias", 50.0, 0.0, 100.0, 0.0, 100.0),
            num("sampleCount", "Sample Count", 12.0, 3.0, 24.0, 3.0, 24.0),
            num("sampleRadius", "Sample Radius", 40.0, 0.0, 200.0, 0.0, 200.0),
            num("flowSoftness", "Flow Softness", 50.0, 0.0, 100.0, 0.0, 100.0),
            num("edgeDefinition", "Edge Definition", 50.0, 0.0, 100.0, 0.0, 100.0),
            num("flowFalloff", "Flow Falloff", 0.0, 0.0, 100.0, 0.0, 100.0),
            popup("view", "View", 2, &["Input Noise", "Curl Generation", "Final Render"]),
            num("contrast", "Contrast", 100.0, 0.0, 10000.0, 0.0, 400.0),
            num("brightness", "Brightness", 0.0, -100.0, 100.0, -100.0, 100.0),
            check("clipHdr", "Clip HDR Results", true),
            popup("channel", "Channel", 0, &["RGB", "Red", "Green", "Blue", "Alpha"]),
        ],
        render,
        pad: super::no_pad,
    }]
}

fn grad(h: u32, x: f64, y: f64, z: f64) -> f64 {
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

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn perlin(x: f64, y: f64, z: f64, s: u32) -> f64 {
    let (xi, yi, zi) = (x.floor(), y.floor(), z.floor());
    let (xf, yf, zf) = (x - xi, y - yi, z - zi);
    let (xi, yi, zi) = (xi as i32, yi as i32, zi as i32);
    let (u, v, w) = (fade(xf), fade(yf), fade(zf));
    let g = |dx: i32, dy: i32, dz: i32| grad(hash3(xi + dx, yi + dy, zi + dz, s), xf - dx as f64, yf - dy as f64, zf - dz as f64);
    let x00 = lerp(g(0, 0, 0), g(1, 0, 0), u);
    let x10 = lerp(g(0, 1, 0), g(1, 1, 0), u);
    let x01 = lerp(g(0, 0, 1), g(1, 0, 1), u);
    let x11 = lerp(g(0, 1, 1), g(1, 1, 1), u);
    lerp(lerp(x00, x10, v), lerp(x01, x11, v), w)
}

fn fbm(x: f64, y: f64, z: f64, octaves: usize, s: u32) -> f64 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..octaves {
        sum += amp * perlin(x * freq + o as f64 * 17.13, y * freq - o as f64 * 9.71, z * (1.0 + o as f64 * 0.37), s.wrapping_add(o as u32));
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

fn white(x: i32, y: i32) -> f64 {
    (hash3(x, y, 7, 0x51ed) >> 8) as f64 / (1u32 << 24) as f64
}

struct Grain {
    w: usize,
    h: usize,
    inv: f32,
    data: Vec<f32>,
}

impl Grain {
    fn new(w: usize, h: usize, grain: f64) -> Grain {
        let (gw, gh) = ((w as f64 / grain).ceil() as usize + 3, (h as f64 / grain).ceil() as usize + 3);
        let data = (0..gw * gh).into_par_iter().map(|i| white((i % gw) as i32, (i / gw) as i32) as f32).collect();
        Grain { w: gw, h: gh, inv: (1.0 / grain) as f32, data }
    }
    fn at(&self, x: f32, y: f32) -> f32 {
        let gx = (x * self.inv).clamp(0.0, (self.w - 2) as f32);
        let gy = (y * self.inv).clamp(0.0, (self.h - 2) as f32);
        let (x0, y0) = (gx as usize, gy as usize);
        let (u, v) = (gx - x0 as f32, gy - y0 as f32);
        let (u, v) = (u * u * (3.0 - 2.0 * u), v * v * (3.0 - 2.0 * v));
        let i = y0 * self.w + x0;
        let d = &self.data;
        let top = d[i] + (d[i + 1] - d[i]) * u;
        let bot = d[i + self.w] + (d[i + self.w + 1] - d[i + self.w]) * u;
        top + (bot - top) * v
    }
}

struct Grid {
    w: usize,
    h: usize,
    step: f64,
    vel: Vec<[f64; 2]>,
    fast: Vec<[f32; 2]>,
    pot: Vec<f64>,
}

impl Grid {
    fn vel(&self, x: f32, y: f32) -> [f32; 2] {
        let s = self.step as f32;
        let gx = (x / s).clamp(0.0, (self.w - 2) as f32);
        let gy = (y / s).clamp(0.0, (self.h - 2) as f32);
        let (x0, y0) = (gx as usize, gy as usize);
        let (u, v) = (gx - x0 as f32, gy - y0 as f32);
        let i = y0 * self.w + x0;
        let a = &self.fast;
        let (p, q, r, t) = (a[i], a[i + 1], a[i + self.w], a[i + self.w + 1]);
        let top = [p[0] + (q[0] - p[0]) * u, p[1] + (q[1] - p[1]) * u];
        let bot = [r[0] + (t[0] - r[0]) * u, r[1] + (t[1] - r[1]) * u];
        [top[0] + (bot[0] - top[0]) * v, top[1] + (bot[1] - top[1]) * v]
    }

    fn sample(&self, x: f64, y: f64) -> ([f64; 2], f64) {
        let gx = (x / self.step).clamp(0.0, (self.w - 1) as f64);
        let gy = (y / self.step).clamp(0.0, (self.h - 1) as f64);
        let (x0, y0) = (gx.floor() as usize, gy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (u, v) = (gx - x0 as f64, gy - y0 as f64);
        let i = |x: usize, y: usize| y * self.w + x;
        let mix2 = |a: [f64; 2], b: [f64; 2], t: f64| [lerp(a[0], b[0], t), lerp(a[1], b[1], t)];
        let top = mix2(self.vel[i(x0, y0)], self.vel[i(x1, y0)], u);
        let bot = mix2(self.vel[i(x0, y1)], self.vel[i(x1, y1)], u);
        let pt = lerp(lerp(self.pot[i(x0, y0)], self.pot[i(x1, y0)], u), lerp(self.pot[i(x0, y1)], self.pot[i(x1, y1)], u), v);
        (mix2(top, bot, v), pt)
    }
}

struct Luma {
    w: usize,
    h: usize,
    mirror: bool,
    data: Vec<f64>,
}

impl Luma {
    fn from(img: &Img, radius: usize, mirror: bool) -> Luma {
        let (w, h) = (img.w, img.h);
        let mut data: Vec<f64> = img.px.chunks_exact(4).map(|p| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64).collect();
        for _ in 0..2 {
            data = box_blur(&data, w, h, radius);
        }
        Luma { w, h, mirror, data }
    }

    fn at(&self, x: f64, y: f64) -> f64 {
        let (w, h) = (self.w as f64, self.h as f64);
        let (mut x, mut y) = (x, y);
        let mut fade_k = 1.0;
        if self.mirror {
            let m = |v: f64, n: f64| {
                let p = v.rem_euclid(2.0 * n);
                if p < n { p } else { 2.0 * n - p }
            };
            x = m(x, w);
            y = m(y, h);
        } else {
            let edge = 0.05 * w.min(h);
            let d = x.min(w - x).min(y).min(h - y);
            fade_k = (d / edge.max(1e-6)).clamp(0.0, 1.0);
            x = x.clamp(0.0, w - 1.0);
            y = y.clamp(0.0, h - 1.0);
        }
        let (x0, y0) = (x.floor().clamp(0.0, w - 1.0) as usize, y.floor().clamp(0.0, h - 1.0) as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (u, v) = (x - x0 as f64, y - y0 as f64);
        let d = &self.data;
        let val = lerp(lerp(d[y0 * self.w + x0], d[y0 * self.w + x1], u), lerp(d[y1 * self.w + x0], d[y1 * self.w + x1], u), v);
        (val * 2.0 - 1.0) * fade_k
    }
}

fn box_blur(src: &[f64], w: usize, h: usize, r: usize) -> Vec<f64> {
    if r == 0 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let mut tmp = vec![0.0; w * h];
    tmp.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let line = &src[y * w..y * w + w];
        let mut acc: f64 = (0..=r).map(|i| line[i.min(w - 1)]).sum::<f64>() + r as f64 * line[0];
        for (x, out) in row.iter_mut().enumerate() {
            *out = acc / (2 * r + 1) as f64;
            acc += line[(x + r + 1).min(w - 1)] - line[x.saturating_sub(r)];
        }
    });
    let mut out = vec![0.0; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let mut s = 0.0;
            for k in 0..=2 * r {
                let yy = (y + k).saturating_sub(r).min(h - 1);
                s += tmp[yy * w + x];
            }
            *o = s / (2 * r + 1) as f64;
        }
    });
    out
}

struct Field<'a> {
    w: f64,
    h: f64,
    scale: f64,
    cell: [f64; 2],
    offset: [f64; 2],
    rot: f64,
    drift: [f64; 2],
    z: f64,
    turb_z: f64,
    octaves: usize,
    density: f64,
    swirl: f64,
    evolution: f64,
    bias: f64,
    layer: Option<&'a Luma>,
}

impl Field<'_> {
    fn local(&self, x: f64, y: f64) -> (f64, f64) {
        let lx = x / self.scale - self.offset[0];
        let ly = y / self.scale - self.offset[1];
        let (s, c) = (-self.rot).sin_cos();
        let (rx, ry) = (lx * c - ly * s, lx * s + ly * c);
        let cx = x / self.scale - self.w / (2.0 * self.scale);
        let cy = y / self.scale - self.h / (2.0 * self.scale);
        let r = ((cx * cx + cy * cy).sqrt() / (0.5 * (self.w * self.w + self.h * self.h).sqrt() / self.scale)).min(1.0);
        let m = 2f64.powf(self.density * (1.0 - 2.0 * r));
        (rx * m / self.cell[0] + self.drift[0], ry * m / self.cell[1] + self.drift[1])
    }

    fn potential(&self, x: f64, y: f64) -> f64 {
        if let Some(l) = self.layer {
            return l.at(x, y);
        }
        let (u, v) = self.local(x, y);
        fbm(u, v, self.z, self.octaves, 1)
    }

    fn turbulence(&self, x: f64, y: f64) -> f64 {
        let (u, v) = self.local(x, y);
        fbm(u * 2.3 + 11.0, v * 2.3 - 7.0, self.turb_z, self.octaves.min(3), 5)
    }

    fn at(&self, x: f64, y: f64, e: f64) -> ([f64; 2], f64) {
        let p = self.potential(x, y);
        let dx = (self.potential(x + e, y) - self.potential(x - e, y)) / (2.0 * e);
        let dy = (self.potential(x, y + e) - self.potential(x, y - e)) / (2.0 * e);
        let t = self.turbulence(x, y);
        let mut v = [dy, -dx];
        let n = (v[0] * v[0] + v[1] * v[1]).sqrt().max(1e-12);
        v = [v[0] / n, v[1] / n];
        let a = self.evolution + self.swirl * (p + 0.5 * t);
        let (s, c) = a.sin_cos();
        v = [v[0] * c - v[1] * s, v[0] * s + v[1] * c];
        let strength = (self.bias - 0.5).abs() * 2.0;
        if strength > 0.0 {
            let axis = if self.bias > 0.5 { [0.0, 1.0] } else { [1.0, 0.0] };
            let d = v[0] * axis[0] + v[1] * axis[1];
            let sign = if d < 0.0 { -1.0 } else { 1.0 };
            v = [lerp(v[0], axis[0] * sign, strength), lerp(v[1], axis[1] * sign, strength)];
            let n = (v[0] * v[0] + v[1] * v[1]).sqrt().max(1e-12);
            v = [v[0] / n, v[1] / n];
        }
        (v, p)
    }
}

fn render(p: &FxCtx, img: &mut Img) {
    let time = p.t.max(0.0);
    let (w, h) = (img.w, img.h);
    if w == 0 || h == 0 {
        return;
    }
    let scale = p.r.max(1e-3);
    let uniform = p.b("uniformScaling");
    let base = p.f("scale").max(1.0);
    let sw = if uniform { base } else { base * p.f("scaleWidth").max(1.0) / 100.0 };
    let sh = if uniform { base } else { base * p.f("scaleHeight").max(1.0) / 100.0 };
    let cell = [3.0 * sw, 3.0 * sh];
    let smooth = p.f("smoothness").clamp(0.0, 100.0) / 100.0;
    let internal = p.e("inputNoise") == 0;
    let dir = p.f("direction").to_radians();
    let speed = if internal { p.f("speed") / 100.0 * 0.6 * time } else { 0.0 };
    let off = p.pt("offset");
    let luma = if internal { None } else { Some(Luma::from(img, ((2.0 + 30.0 * smooth) * scale).round() as usize, p.b("mirrorAtEdge"))) };
    let field = Field {
        w: w as f64,
        h: h as f64,
        scale,
        cell,
        offset: [off[0] / scale, off[1] / scale],
        rot: p.f("rotation").to_radians(),
        drift: [dir.sin() * speed, -dir.cos() * speed],
        z: if internal { p.f("noiseSpeed") / 100.0 * 0.5 * time } else { 0.0 },
        turb_z: p.f("turbulenceSpeed") / 100.0 * 0.5 * time + 3.7,
        octaves: (5.0 - 4.0 * smooth).round().clamp(1.0, 5.0) as usize,
        density: p.f("density").clamp(-100.0, 100.0) / 100.0,
        swirl: p.f("swirl").to_radians() * 0.5,
        evolution: p.f("evolution").to_radians(),
        bias: p.f("verticalBias").clamp(0.0, 100.0) / 100.0,
        layer: luma.as_ref(),
    };
    let min_cell = cell[0].min(cell[1]) * scale;
    let step = (min_cell / 24.0).clamp(1.0, 8.0);
    let gw = (w as f64 / step).ceil() as usize + 2;
    let gh = (h as f64 / step).ceil() as usize + 2;
    let e = (step * 0.75).max(0.75);
    let cells: Vec<([f64; 2], f64)> = (0..gw * gh).into_par_iter().map(|i| field.at((i % gw) as f64 * step, (i / gw) as f64 * step, e)).collect();
    let grid = Grid { w: gw, h: gh, step, vel: cells.iter().map(|c| c.0).collect(), fast: cells.iter().map(|c| [c.0[0] as f32, c.0[1] as f32]).collect(), pot: cells.iter().map(|c| c.1).collect() };
    let view = p.e("view");
    let samples = p.f("sampleCount").round().clamp(3.0, 24.0) as i32;
    let radius = p.f("sampleRadius").clamp(0.0, 200.0) * scale;
    let soft = p.f("flowSoftness").clamp(0.0, 100.0) / 100.0;
    let edge = p.f("edgeDefinition").clamp(0.0, 100.0) / 100.0;
    let falloff = p.f("flowFalloff").clamp(0.0, 100.0) / 100.0;
    let contrast = p.f("contrast") / 100.0;
    let bright = p.f("brightness") / 100.0;
    let clip = p.b("clipHdr");
    let channel = p.e("channel");
    let grain = lerp(4.0, 1.2, edge) * scale.max(0.25);
    let gain = lerp(5.0, 0.6, soft);
    let lines_mix = (radius / scale / 30.0).clamp(0.0, 1.0);
    let length = radius * samples as f64 / 12.0;
    let steps = ((length / (0.9 * grain)).ceil() as i32).clamp(3, 96);
    let ds = (length / steps as f64) as f32;
    let weights: Vec<f32> = (1..=steps).map(|k| 0.5 + 0.5 * (std::f32::consts::PI * k as f32 / (steps + 1) as f32).cos()).collect();
    let (sw1, sw2) = weights.iter().fold((1.0f32, 1.0f32), |(a, b), w| (a + 2.0 * w, b + 2.0 * w * w));
    let norm = ((sw1 * sw1 / sw2).sqrt() * 3.4) as f64;
    let tex = if view == 2 && lines_mix > 0.0 { Some(Grain::new(w, h, grain)) } else { None };
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            let (v0, pot) = grid.sample(fx, fy);
            let (rgb, val) = match view {
                0 => (None, pot * 0.5 + 0.5),
                1 => (Some([v0[0] * 0.5 + 0.5, v0[1] * 0.5 + 0.5, pot * 0.25 + 0.5]), 0.0),
                _ => {
                    let softn = pot * 0.5 + 0.5;
                    let mut val = softn;
                    if let Some(tex) = &tex {
                        let mut acc = tex.at(fx as f32, fy as f32);
                        for dirn in [ds, -ds] {
                            let (mut qx, mut qy) = (fx as f32, fy as f32);
                            for wk in &weights {
                                let v = grid.vel(qx, qy);
                                qx += dirn * v[0];
                                qy += dirn * v[1];
                                acc += wk * tex.at(qx, qy);
                            }
                        }
                        let lic = (acc / sw1 - 0.5) as f64 * norm;
                        let lines = 0.5 + 0.5 * (lic * gain).tanh();
                        val = lerp(softn, lines * lerp(0.6, 1.0, softn), lines_mix);
                    }
                    if falloff > 0.0 {
                        val *= lerp(1.0, (pot * 0.9 + 0.55).clamp(0.0, 1.2), falloff);
                    }
                    (None, val)
                }
            };
            let tone = |v: f64| {
                let o = (v - 0.5) * contrast + 0.5 + bright;
                if clip { o.clamp(0.0, 1.0) } else { o.max(0.0) }
            };
            let c = match rgb {
                Some(c) => [tone(c[0]), tone(c[1]), tone(c[2])],
                None => {
                    let g = tone(val);
                    [g, g, g]
                }
            };
            let px = &mut row[x * 4..x * 4 + 4];
            let a = px[3] as f64;
            match channel {
                1..=3 => {
                    let i = (channel - 1) as usize;
                    px[i] = (c[i] * a) as f32;
                }
                4 => {
                    let na = (c[0] * a).clamp(0.0, 1.0);
                    if a > 1e-6 {
                        for k in 0..3 {
                            px[k] = (px[k] as f64 / a * na) as f32;
                        }
                    }
                    px[3] = na as f32;
                }
                _ => {
                    for k in 0..3 {
                        px[k] = (c[k] * a) as f32;
                    }
                }
            }
        }
    });
}
