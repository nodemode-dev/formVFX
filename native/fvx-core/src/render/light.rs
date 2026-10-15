use std::f64::consts::PI;
use std::sync::Arc;

use rayon::prelude::*;

use super::image::Img;
use super::mesh::{Proj, Tri, depth_map};
use super::mirror::Mirror;
use crate::eval::{Camera, Eval};
use crate::geom::Mat4;
use crate::model::Layer;

pub const PARALLEL: u32 = 0;
pub const SPOT: u32 = 1;
pub const POINT: u32 = 2;
pub const AMBIENT: u32 = 3;
pub const ENVIRONMENT: u32 = 4;

#[derive(Clone)]
pub struct Light {
    pub id: u64,
    pub kind: u32,
    pub pos: [f64; 3],
    pub dir: [f64; 3],
    pub color: [f32; 3],
    pub cos_outer: f64,
    pub cos_inner: f64,
    pub falloff: u32,
    pub radius: f64,
    pub falloff_dist: f64,
    pub shadows: bool,
    pub darkness: f32,
    pub diffusion: f64,
    pub source: Option<u64>,
    pub env_rot: f64,
}

pub fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub fn len(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

pub fn norm(a: [f64; 3]) -> [f64; 3] {
    let l = len(a);
    if l < 1e-12 { [0.0, 0.0, -1.0] } else { scale(a, 1.0 / l) }
}

pub fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn lights_of(ev: &Eval) -> Vec<Light> {
    let mut out = vec![];
    for l in ev.comp.layers.iter().filter(|l| l.is_light() && l.switches.video && l.is_active_at(ev.t)) {
        let Some(g) = l.props.sub("lightOptions") else { continue };
        let num = |id: &str, d: f64| g.get(id).map(|p| ev.value(l, p).num()).unwrap_or(d);
        let kind = g.get("lightType").map(|p| p.value.index()).unwrap_or(POINT);
        let c = g.get("color").map(|p| ev.value(l, p).color()).unwrap_or([1.0; 4]);
        let k = num("intensity", 100.0) / 100.0;
        let w = ev.world_matrix(l);
        let pos = w.apply([0.0; 3]);
        let dir = norm(sub(w.apply([0.0, 0.0, 1.0]), pos));
        let half = (num("coneAngle", 90.0) / 2.0).clamp(0.0, 90.0).to_radians();
        let feather = (num("coneFeather", 50.0) / 100.0).clamp(0.0, 1.0);
        out.push(Light {
            id: l.id,
            kind,
            pos,
            dir,
            color: [(c[0] * k) as f32, (c[1] * k) as f32, (c[2] * k) as f32],
            cos_outer: half.cos(),
            cos_inner: (half * (1.0 - feather)).cos(),
            falloff: g.get("falloff").map(|p| ev.value(l, p).index()).unwrap_or(0),
            radius: num("radius", 500.0),
            falloff_dist: num("falloffDistance", 500.0).max(1e-3),
            shadows: g.get("castsShadows").map(|p| ev.value(l, p).boolean()).unwrap_or(false),
            darkness: (num("shadowDarkness", 100.0) / 100.0) as f32,
            diffusion: num("shadowDiffusion", 0.0),
            source: g.get("source").and_then(|p| ev.value(l, p).layer()),
            env_rot: num("environmentRotation", 0.0),
        });
    }
    out
}

pub struct EnvMap {
    levels: Vec<Img>,
    rot: f64,
}

impl EnvMap {
    pub fn uniform(c: [f32; 3]) -> EnvMap {
        let mut i = Img::new(1, 1);
        i.px.copy_from_slice(&[c[0], c[1], c[2], 1.0]);
        EnvMap { levels: vec![i], rot: 0.0 }
    }

    pub fn from_img(src: &Img, tint: [f32; 3], rot: f64) -> EnvMap {
        let (w, h) = (512usize, 256usize);
        let mut l0 = Img::new(w, h);
        let (sx, sy) = (src.w as f64 / w as f64, src.h as f64 / h as f64);
        l0.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
            let (x, y) = ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
            let s = src.sample(x * sx, y * sy, true);
            p[0] = s[0] * tint[0];
            p[1] = s[1] * tint[1];
            p[2] = s[2] * tint[2];
            p[3] = 1.0;
        });
        let mut levels = vec![l0];
        for _ in 0..3 {
            let prev = &levels[levels.len() - 1];
            let mut b = prev.clone();
            crate::effects::util::gaussian(&mut b, 3.0, 3.0, true);
            let (nw, nh) = ((prev.w / 4).max(2), (prev.h / 4).max(1));
            let mut d = Img::new(nw, nh);
            let (kx, ky) = (prev.w as f64 / nw as f64, prev.h as f64 / nh as f64);
            d.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
                let (x, y) = ((i % nw) as f64 + 0.5, (i / nw) as f64 + 0.5);
                p.copy_from_slice(&b.sample(x * kx, y * ky, true));
            });
            levels.push(d);
        }
        EnvMap { levels, rot }
    }

    fn at(&self, lvl: usize, d: [f64; 3]) -> [f32; 3] {
        let img = &self.levels[lvl.min(self.levels.len() - 1)];
        let u = (0.5 + d[0].atan2(-d[2]) / (2.0 * PI) + self.rot / 360.0).rem_euclid(1.0);
        let v = (-d[1]).clamp(-1.0, 1.0).acos() / PI;
        let x = u * img.w as f64;
        let y = v * img.h as f64;
        let x0 = x.floor() as isize;
        let fx = (x - x0 as f64) as f32;
        let wrap = |xx: isize| xx.rem_euclid(img.w as isize);
        let a = img.sample(wrap(x0) as f64 + 0.5, y, true);
        let b = img.sample(wrap(x0 + 1) as f64 + 0.5, y, true);
        [a[0] + (b[0] - a[0]) * fx, a[1] + (b[1] - a[1]) * fx, a[2] + (b[2] - a[2]) * fx]
    }

    pub fn sample(&self, d: [f64; 3], rough: f32) -> [f32; 3] {
        let n = self.levels.len();
        if n == 1 {
            return self.at(0, d);
        }
        let l = (rough.clamp(0.0, 1.0) * (n - 1) as f32).max(0.0);
        let (a, b) = (l.floor() as usize, (l.ceil() as usize).min(n - 1));
        let t = l - a as f32;
        let ca = self.at(a, d);
        if a == b {
            return ca;
        }
        let cb = self.at(b, d);
        [ca[0] + (cb[0] - ca[0]) * t, ca[1] + (cb[1] - ca[1]) * t, ca[2] + (cb[2] - ca[2]) * t]
    }
}

pub struct Caster {
    pub id: u64,
    pub inv: Mat4,
    pub img: Img,
    pub rect: [f64; 2],
    pub r: f64,
    pub transmission: f32,
}

pub struct SMap {
    pub proj: Proj,
    pub size: usize,
    pub depth: Vec<f32>,
    pub bias: f64,
}

impl SMap {
    fn lookup(&self, p: [f64; 3], radius: i64) -> Option<f32> {
        let v = self.proj.view.apply(p);
        let s = match self.proj.ortho {
            Some(o) => [self.proj.cx + o * v[0], self.proj.cy + o * v[1]],
            None => {
                if v[2] <= 1.0 {
                    return None;
                }
                [self.proj.cx + self.proj.zoom * v[0] / v[2], self.proj.cy + self.proj.zoom * v[1] / v[2]]
            }
        };
        if s[0] < 0.0 || s[1] < 0.0 || s[0] >= self.size as f64 || s[1] >= self.size as f64 {
            return None;
        }
        let (cx, cy) = (s[0] as i64, s[1] as i64);
        let mut lit = 0usize;
        let mut total = 0usize;
        let bias = (self.bias * if self.proj.ortho.is_some() { 1.0 } else { v[2].max(1.0) * 0.012 }) as f32;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let (x, y) = ((cx + dx).clamp(0, self.size as i64 - 1) as usize, (cy + dy).clamp(0, self.size as i64 - 1) as usize);
                total += 1;
                if (v[2] as f32) <= self.depth[y * self.size + x] + bias {
                    lit += 1;
                }
            }
        }
        Some(lit as f32 / total.max(1) as f32)
    }
}

pub struct Scene {
    pub lights: Vec<Light>,
    pub casters: Vec<Caster>,
    pub maps: Vec<Vec<SMap>>,
    pub env: Option<Arc<EnvMap>>,
    pub eye: [f64; 3],
    pub ortho_fwd: Option<[f64; 3]>,
    pub mirror: Option<Arc<Mirror>>,
}

const CONE: [[f64; 2]; 7] = [[0.0, 0.0], [1.0, 0.0], [0.5, 0.866], [-0.5, 0.866], [-1.0, 0.0], [-0.5, -0.866], [0.5, -0.866]];

#[derive(Clone, Copy, Debug)]
pub struct Surface {
    pub ambient: f32,
    pub diffuse: f32,
    pub spec: f32,
    pub shininess: f32,
    pub metal: f32,
    pub shadows: u32,
    pub reflection: f32,
    pub sharpness: f32,
    pub refl_rolloff: f32,
    pub transparency: f32,
    pub trans_rolloff: f32,
    pub ior: f32,
}

impl Surface {
    pub fn reflection_at(&self, nv: f32) -> f32 {
        let fresnel = 0.04 + 0.96 * (1.0 - nv.clamp(0.0, 1.0)).powi(5);
        self.reflection * (1.0 - self.refl_rolloff + self.refl_rolloff * fresnel)
    }

    pub fn transparency_at(&self, nv: f32) -> f32 {
        self.transparency * (1.0 - self.trans_rolloff * (1.0 - nv.clamp(0.0, 1.0)))
    }
}

impl Surface {
    pub fn of(ev: &Eval, l: &Layer) -> (Surface, bool, u32, f32) {
        let g = l.props.sub("materialOptions");
        let num = |id: &str, d: f64| g.and_then(|g| g.get(id)).map(|p| ev.value(l, p).num()).unwrap_or(d) as f32;
        let accepts_lights = g.and_then(|g| g.get("acceptsLights")).map(|p| ev.value(l, p).boolean()).unwrap_or(true);
        let casts = g.and_then(|g| g.get("castsShadows")).map(|p| ev.value(l, p).index()).unwrap_or(0);
        let shadows = g.and_then(|g| g.get("acceptsShadows")).map(|p| ev.value(l, p).index()).unwrap_or(1);
        let s = Surface {
            ambient: num("ambient", 100.0) / 100.0,
            diffuse: num("diffuse", 50.0) / 100.0,
            spec: num("specularIntensity", 50.0) / 100.0,
            shininess: num("specularShininess", 5.0),
            metal: num("metal", 100.0) / 100.0,
            shadows,
            reflection: num("reflectionIntensity", 0.0) / 100.0,
            sharpness: (num("reflectionSharpness", num("specularShininess", 5.0) as f64) / 100.0).clamp(0.0, 1.0),
            refl_rolloff: (num("reflectionRolloff", 0.0) / 100.0).clamp(0.0, 1.0),
            transparency: (num("transparency", 0.0) / 100.0).clamp(0.0, 1.0),
            trans_rolloff: (num("transparencyRolloff", 0.0) / 100.0).clamp(0.0, 1.0),
            ior: num("indexOfRefraction", 1.0).clamp(1.0, 5.0),
        };
        (s, accepts_lights, casts, num("lightTransmission", 0.0) / 100.0)
    }
}

fn smooth(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Scene {
    pub fn reflection(&self, p: [f64; 3], n: [f64; 3], v: [f64; 3], self_id: u64, sharp: f32, with_env: bool) -> Option<([f32; 3], f32)> {
        let env = if with_env { self.env.as_ref() } else { None };
        let mirror = self.mirror.as_ref().filter(|m| !m.is_empty());
        if env.is_none() && mirror.is_none() {
            return None;
        }
        let r = norm(sub(scale(n, 2.0 * dot(n, v)), v));
        let blur = 1.0 - sharp.clamp(0.0, 1.0);
        let rays: &[[f64; 2]] = if blur < 0.02 || mirror.is_none() { &CONE[..1] } else { &CONE };
        let up = if r[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let ax = norm(cross(r, up));
        let ay = cross(r, ax);
        let spread = blur as f64 * 0.6;
        let (mut acc, mut w) = ([0.0f32; 3], 0.0f32);
        for o in rays {
            let d = norm(add(r, add(scale(ax, o[0] * spread), scale(ay, o[1] * spread))));
            if dot(d, n) <= 0.0 {
                continue;
            }
            let back = env.map(|e| e.sample(d, blur));
            let hit = mirror.and_then(|m| m.trace(p, d, self_id).map(|h| (h, m)));
            let (c, a) = match hit {
                Some((h, m)) => {
                    let base = [h.color[0], h.color[1], h.color[2]];
                    let lit = match h.mesh.and_then(|(mi, mat)| m.meshes.get(mi).and_then(|x| x.1.get(mat as usize)).map(|mt| (m.meshes[mi].0, mt))) {
                        Some((id, mt)) => {
                            let s = self.shade_pbr(h.p, h.n, base, mt.metallic, mt.roughness, id, false);
                            [s[0] + mt.emissive[0], s[1] + mt.emissive[1], s[2] + mt.emissive[2]]
                        }
                        None => base,
                    };
                    (lit, h.color[3].clamp(0.0, 1.0))
                }
                None => ([0.0; 3], 0.0),
            };
            match back {
                Some(b) => {
                    for k in 0..3 {
                        acc[k] += c[k] * a + b[k] * (1.0 - a);
                    }
                    w += 1.0;
                }
                None => {
                    for k in 0..3 {
                        acc[k] += c[k] * a;
                    }
                    w += a;
                }
            }
        }
        if w <= 1e-4 {
            return None;
        }
        Some((acc.map(|x| x / w), w / rays.len() as f32))
    }

    pub fn view_dir(&self, p: [f64; 3]) -> [f64; 3] {
        match self.ortho_fwd {
            Some(f) => scale(f, -1.0),
            None => norm(sub(self.eye, p)),
        }
    }

    fn direction(&self, l: &Light, p: [f64; 3]) -> ([f64; 3], f64, f32) {
        match l.kind {
            PARALLEL => (scale(l.dir, -1.0), f64::INFINITY, 1.0),
            _ => {
                let d = sub(l.pos, p);
                let dist = len(d);
                let a = match l.falloff {
                    1 => {
                        if dist <= l.radius {
                            1.0
                        } else {
                            1.0 - smooth(0.0, 1.0, (dist - l.radius) / l.falloff_dist)
                        }
                    }
                    2 => {
                        if dist <= l.radius.max(1e-3) {
                            1.0
                        } else {
                            (l.radius / dist).powi(2)
                        }
                    }
                    _ => 1.0,
                };
                let cone = if l.kind == SPOT {
                    let c = dot(l.dir, scale(d, -1.0 / dist.max(1e-9)));
                    if l.cos_inner - l.cos_outer < 1e-6 { if c >= l.cos_outer { 1.0 } else { 0.0 } } else { smooth(l.cos_outer, l.cos_inner, c) }
                } else {
                    1.0
                };
                (scale(d, 1.0 / dist.max(1e-9)), dist, (a * cone) as f32)
            }
        }
    }

    pub fn visibility(&self, li: usize, p: [f64; 3], self_id: u64) -> [f32; 3] {
        let l = &self.lights[li];
        if !l.shadows || matches!(l.kind, AMBIENT | ENVIRONMENT) {
            return [1.0; 3];
        }
        let taps: &[[f64; 2]] = if l.diffusion > 0.5 { &[[0.0, 0.0], [1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0], [0.7, 0.7], [-0.7, 0.7], [0.7, -0.7], [-0.7, -0.7]] } else { &[[0.0, 0.0]] };
        let (to_light, _, _) = self.direction(l, p);
        let up = if to_light[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let ax = norm(cross(to_light, up));
        let ay = cross(to_light, ax);
        let mut acc = [0.0f32; 3];
        for t in taps {
            let off = add(scale(ax, t[0] * l.diffusion), scale(ay, t[1] * l.diffusion));
            let target = if l.kind == PARALLEL { add(p, add(scale(to_light, 1e6), scale(off, 1e6 / 1000.0))) } else { add(l.pos, off) };
            let mut v = [1.0f32; 3];
            for c in &self.casters {
                if c.id == self_id {
                    continue;
                }
                let a = c.inv.apply(p);
                let b = c.inv.apply(target);
                if a[2] * b[2] >= 0.0 {
                    continue;
                }
                let k = a[2] / (a[2] - b[2]);
                let hit = [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k];
                let s = c.img.sample((hit[0] - c.rect[0]) * c.r, (hit[1] - c.rect[1]) * c.r, false);
                let al = s[3];
                if al <= 1e-4 {
                    continue;
                }
                for ch in 0..3 {
                    let col = s[ch] / al.max(1e-6);
                    v[ch] *= 1.0 - al + al * c.transmission * col;
                }
            }
            for (ch, x) in acc.iter_mut().enumerate() {
                *x += v[ch];
            }
        }
        let n = taps.len() as f32;
        let mut v = acc.map(|x| x / n);
        if let Some(faces) = self.maps.get(li) {
            let r = (l.diffusion / 8.0).round().clamp(1.0, 4.0) as i64;
            for m in faces {
                if let Some(lit) = m.lookup(p, r) {
                    v = v.map(|x| x * lit);
                    break;
                }
            }
        }
        v.map(|x| 1.0 - l.darkness * (1.0 - x))
    }

    pub fn shade_classic(&self, p: [f64; 3], n: [f64; 3], base: [f32; 3], s: &Surface, self_id: u64) -> ([f32; 3], f32) {
        let v = self.view_dir(p);
        let n = if dot(n, v) < 0.0 { scale(n, -1.0) } else { n };
        let mut amb = [0.0f32; 3];
        let mut dif = [0.0f32; 3];
        let mut spe = [0.0f32; 3];
        let mut shade_sum = 0.0f32;
        let mut shade_n = 0.0f32;
        let exp = 2.0 + s.shininess * s.shininess * 0.2;
        for (li, l) in self.lights.iter().enumerate() {
            match l.kind {
                AMBIENT => {
                    for k in 0..3 {
                        amb[k] += l.color[k];
                    }
                }
                ENVIRONMENT => {
                    let env = self.env.as_ref();
                    let irr = env.map(|e| e.sample(n, 1.0)).unwrap_or([1.0; 3]);
                    for k in 0..3 {
                        amb[k] += l.color[k] * irr[k];
                    }
                    let refl = s.reflection_at(dot(n, v) as f32);
                    if refl > 0.0 {
                        let r = sub(scale(n, 2.0 * dot(n, v)), v);
                        let c = env.map(|e| e.sample(r, 1.0 - s.sharpness)).unwrap_or([1.0; 3]);
                        for k in 0..3 {
                            spe[k] += l.color[k] * c[k] * refl;
                        }
                    }
                }
                _ => {
                    let (to_l, _, att) = self.direction(l, p);
                    if att <= 0.0 {
                        continue;
                    }
                    let ndl = dot(n, to_l);
                    if ndl <= 0.0 {
                        continue;
                    }
                    let vis = if s.shadows > 0 { self.visibility(li, p, self_id) } else { [1.0; 3] };
                    if l.shadows && s.shadows > 0 {
                        shade_sum += (vis[0] + vis[1] + vis[2]) / 3.0;
                        shade_n += 1.0;
                    }
                    let r = sub(scale(n, 2.0 * ndl), to_l);
                    let sp = (dot(r, v).max(0.0) as f32).powf(exp) * att;
                    for k in 0..3 {
                        dif[k] += l.color[k] * ndl as f32 * att * vis[k];
                        spe[k] += l.color[k] * sp * vis[k];
                    }
                }
            }
        }
        let mut out = [0.0f32; 3];
        for k in 0..3 {
            let spec_col = s.metal * base[k] + (1.0 - s.metal);
            out[k] = base[k] * (amb[k] * s.ambient + dif[k] * s.diffuse) + spe[k] * s.spec * spec_col;
        }
        let only = if shade_n > 0.0 { 1.0 - shade_sum / shade_n } else { 0.0 };
        (out, only)
    }

    pub fn shade_pbr(&self, p: [f64; 3], n: [f64; 3], base: [f32; 3], metallic: f32, rough: f32, self_id: u64, shadows: bool) -> [f32; 3] {
        let v = self.view_dir(p);
        let n = if dot(n, v) < 0.0 { scale(n, -1.0) } else { n };
        let nv = dot(n, v).max(1e-4) as f32;
        let a = (rough * rough).max(0.002);
        let f0 = [0, 1, 2].map(|k| 0.04 * (1.0 - metallic) + base[k] * metallic);
        let kd = [0, 1, 2].map(|k| base[k] * (1.0 - metallic));
        let mut out = [0.0f32; 3];
        let direct = |to_l: [f64; 3], col: [f32; 3], out: &mut [f32; 3]| {
            let nl = dot(n, to_l) as f32;
            if nl <= 0.0 {
                return;
            }
            let h = norm(add(to_l, v));
            let nh = dot(n, h).max(0.0) as f32;
            let vh = dot(v, h).max(0.0) as f32;
            let d = a * a / (std::f32::consts::PI * ((nh * nh) * (a * a - 1.0) + 1.0).powi(2)).max(1e-6);
            let k = (rough + 1.0).powi(2) / 8.0;
            let g = (nv / (nv * (1.0 - k) + k)) * (nl / (nl * (1.0 - k) + k));
            for c in 0..3 {
                let f = f0[c] + (1.0 - f0[c]) * (1.0 - vh).powi(5);
                let spec = d * g * f / (4.0 * nv * nl).max(1e-4);
                out[c] += col[c] * nl * (kd[c] * (1.0 - f) + spec * std::f32::consts::PI);
            }
        };
        if self.lights.iter().all(|l| l.kind == AMBIENT) && self.env.is_none() {
            direct(norm(add(v, [0.0, -0.4, 0.0])), [0.85; 3], &mut out);
            let amb = if self.lights.is_empty() { [0.3f32; 3] } else { [0.0; 3] };
            for c in 0..3 {
                out[c] += kd[c] * amb[c];
            }
        }
        for (li, l) in self.lights.iter().enumerate() {
            match l.kind {
                AMBIENT => {
                    for c in 0..3 {
                        out[c] += kd[c] * l.color[c];
                    }
                }
                ENVIRONMENT => {
                    let Some(env) = self.env.as_ref() else { continue };
                    let irr = env.sample(n, 1.0);
                    let r = sub(scale(n, 2.0 * dot(n, v)), v);
                    let refl = env.sample(r, rough);
                    for c in 0..3 {
                        let f = f0[c] + ((1.0 - rough).max(f0[c]) - f0[c]) * (1.0 - nv).powi(5);
                        out[c] += l.color[c] * (kd[c] * irr[c] + refl[c] * f);
                    }
                }
                _ => {
                    let (to_l, _, att) = self.direction(l, p);
                    if att <= 0.0 {
                        continue;
                    }
                    let vis = if shadows { self.visibility(li, p, self_id) } else { [1.0; 3] };
                    let col = [0, 1, 2].map(|c| l.color[c] * att * vis[c]);
                    direct(to_l, col, &mut out);
                }
            }
        }
        out
    }
}

pub fn shadow_maps(lights: &[Light], tris: &[Tri], bounds: Option<([f64; 3], f64)>) -> Vec<Vec<SMap>> {
    lights
        .iter()
        .map(|l| {
            if !l.shadows || tris.is_empty() {
                return vec![];
            }
            let Some((center, radius)) = bounds else { return vec![] };
            let radius = radius.max(1.0);
            match l.kind {
                PARALLEL => {
                    let size = 1024;
                    let eye = sub(center, scale(l.dir, radius * 4.0));
                    let cam = Camera::looking(eye, center, 1.0, Some(size as f64 / (2.2 * radius)));
                    let proj = Proj { view: cam.view, zoom: 1.0, ortho: cam.ortho, cx: size as f64 / 2.0, cy: size as f64 / 2.0, k: 1.0 };
                    let depth = depth_map(tris, &proj, size);
                    vec![SMap { proj, size, depth, bias: 6.0 * radius / 1000.0 + 2.0 }]
                }
                SPOT => {
                    let size = 1024;
                    let half = l.cos_outer.clamp(-1.0, 1.0).acos().clamp(0.05, 1.5);
                    let zoom = size as f64 / 2.0 / half.tan();
                    let cam = Camera::looking(l.pos, add(l.pos, l.dir), zoom, None);
                    let proj = Proj { view: cam.view, zoom, ortho: None, cx: size as f64 / 2.0, cy: size as f64 / 2.0, k: 1.0 };
                    let depth = depth_map(tris, &proj, size);
                    vec![SMap { proj, size, depth, bias: 1.0 }]
                }
                POINT => {
                    let size = 512;
                    [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]]
                        .iter()
                        .map(|d| {
                            let zoom = size as f64 / 2.0 * 0.98;
                            let cam = Camera::looking(l.pos, add(l.pos, *d), zoom, None);
                            let proj = Proj { view: cam.view, zoom, ortho: None, cx: size as f64 / 2.0, cy: size as f64 / 2.0, k: 1.0 };
                            let depth = depth_map(tris, &proj, size);
                            SMap { proj, size, depth, bias: 1.0 }
                        })
                        .collect()
                }
                _ => vec![],
            }
        })
        .collect()
}
