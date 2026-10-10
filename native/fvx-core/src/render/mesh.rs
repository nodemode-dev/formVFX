use std::f64::consts::{PI, TAU};
use std::sync::Arc;

use rayon::prelude::*;

use super::image::{Img, Tile};
use super::media::Media;
use super::model3d::{Material, Model};
use crate::eval::{Camera, Eval};
use crate::geom::Mat4;
use crate::model::{ItemKind, Layer};

#[derive(Clone, Copy, Debug)]
pub struct Tri {
    pub v: [[f64; 3]; 3],
    pub n: [[f32; 3]; 3],
    pub uv: [[f32; 2]; 3],
    pub mat: u16,
}

#[derive(Clone, Debug)]
pub struct MatEval {
    pub base: [f32; 4],
    pub texture: Option<Arc<Img>>,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
}

impl MatEval {
    pub fn color(&self, uv: [f32; 2]) -> [f32; 4] {
        let mut c = self.base;
        if let Some(t) = &self.texture {
            let x = (uv[0].rem_euclid(1.0) as f64) * t.w as f64;
            let y = (uv[1].rem_euclid(1.0) as f64) * t.h as f64;
            let s = t.sample(x, y, true);
            let a = s[3].max(1e-6);
            c = [c[0] * s[0] / a, c[1] * s[1] / a, c[2] * s[2] / a, c[3] * s[3]];
        }
        c
    }
}

pub struct LayerMesh {
    pub tris: Vec<Tri>,
    pub mats: Vec<MatEval>,
}

fn quad(out: &mut Vec<Tri>, p: [[f64; 3]; 4], n: [f32; 3], uv: [[f32; 2]; 4]) {
    out.push(Tri { v: [p[0], p[1], p[2]], n: [n; 3], uv: [uv[0], uv[1], uv[2]], mat: 0 });
    out.push(Tri { v: [p[0], p[2], p[3]], n: [n; 3], uv: [uv[0], uv[2], uv[3]], mat: 0 });
}

pub fn primitive(kind: u32, w: f64, h: f64, d: f64, r: f64, tube: f64, seg: usize, rings: usize) -> Vec<Tri> {
    let seg = seg.clamp(3, 256);
    let rings = rings.clamp(2, 256);
    let uvq = [[0.0f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mut out = vec![];
    match kind {
        0 => {
            let (x, y, z) = (w / 2.0, h / 2.0, d / 2.0);
            quad(&mut out, [[-x, -y, -z], [x, -y, -z], [x, y, -z], [-x, y, -z]], [0.0, 0.0, -1.0], uvq);
            quad(&mut out, [[x, -y, z], [-x, -y, z], [-x, y, z], [x, y, z]], [0.0, 0.0, 1.0], uvq);
            quad(&mut out, [[-x, -y, z], [-x, -y, -z], [-x, y, -z], [-x, y, z]], [-1.0, 0.0, 0.0], uvq);
            quad(&mut out, [[x, -y, -z], [x, -y, z], [x, y, z], [x, y, -z]], [1.0, 0.0, 0.0], uvq);
            quad(&mut out, [[-x, -y, z], [x, -y, z], [x, -y, -z], [-x, -y, -z]], [0.0, -1.0, 0.0], uvq);
            quad(&mut out, [[-x, y, -z], [x, y, -z], [x, y, z], [-x, y, z]], [0.0, 1.0, 0.0], uvq);
        }
        2 => quad(&mut out, [[-w / 2.0, -h / 2.0, 0.0], [w / 2.0, -h / 2.0, 0.0], [w / 2.0, h / 2.0, 0.0], [-w / 2.0, h / 2.0, 0.0]], [0.0, 0.0, -1.0], uvq),
        1 => {
            let pt = |i: usize, j: usize| -> ([f64; 3], [f32; 3], [f32; 2]) {
                let u = i as f64 / seg as f64;
                let v = j as f64 / rings as f64;
                let (th, ph) = (u * TAU, v * PI);
                let n = [ph.sin() * th.sin(), -ph.cos(), -ph.sin() * th.cos()];
                ([n[0] * r, n[1] * r, n[2] * r], [n[0] as f32, n[1] as f32, n[2] as f32], [u as f32, v as f32])
            };
            for j in 0..rings {
                for i in 0..seg {
                    let a = pt(i, j);
                    let b = pt(i + 1, j);
                    let c = pt(i + 1, j + 1);
                    let e = pt(i, j + 1);
                    out.push(Tri { v: [a.0, b.0, c.0], n: [a.1, b.1, c.1], uv: [a.2, b.2, c.2], mat: 0 });
                    out.push(Tri { v: [a.0, c.0, e.0], n: [a.1, c.1, e.1], uv: [a.2, c.2, e.2], mat: 0 });
                }
            }
        }
        3 => {
            let pt = |i: usize, j: usize| -> ([f64; 3], [f32; 3], [f32; 2]) {
                let u = i as f64 / seg as f64;
                let v = j as f64 / rings as f64;
                let (th, ph) = (u * TAU, v * TAU);
                let c = [th.sin() * r, 0.0, -th.cos() * r];
                let n = [ph.cos() * th.sin(), ph.sin(), -ph.cos() * th.cos()];
                ([c[0] + n[0] * tube, n[1] * tube, c[2] + n[2] * tube], [n[0] as f32, n[1] as f32, n[2] as f32], [u as f32, v as f32])
            };
            for j in 0..rings {
                for i in 0..seg {
                    let a = pt(i, j);
                    let b = pt(i + 1, j);
                    let c = pt(i + 1, j + 1);
                    let e = pt(i, j + 1);
                    out.push(Tri { v: [a.0, b.0, c.0], n: [a.1, b.1, c.1], uv: [a.2, b.2, c.2], mat: 0 });
                    out.push(Tri { v: [a.0, c.0, e.0], n: [a.1, c.1, e.1], uv: [a.2, c.2, e.2], mat: 0 });
                }
            }
        }
        _ => {
            let cone = kind == 4;
            let (top, bot) = (-h / 2.0, h / 2.0);
            let slope = if cone { r / h.max(1e-9) } else { 0.0 };
            for i in 0..seg {
                let (u0, u1) = (i as f64 / seg as f64, (i + 1) as f64 / seg as f64);
                let (a0, a1) = (u0 * TAU, u1 * TAU);
                let ring = |a: f64, y: f64, rr: f64| [a.sin() * rr, y, -a.cos() * rr];
                let nrm = |a: f64| {
                    let l = (1.0 + slope * slope).sqrt();
                    [(a.sin() / l) as f32, (-slope / l) as f32, (-a.cos() / l) as f32]
                };
                let rt = if cone { 0.0 } else { r };
                let p = [ring(a0, top, rt), ring(a1, top, rt), ring(a1, bot, r), ring(a0, bot, r)];
                let n = [nrm(a0), nrm(a1), nrm(a1), nrm(a0)];
                let uv = [[u0 as f32, 0.0], [u1 as f32, 0.0], [u1 as f32, 1.0], [u0 as f32, 1.0]];
                out.push(Tri { v: [p[0], p[1], p[2]], n: [n[0], n[1], n[2]], uv: [uv[0], uv[1], uv[2]], mat: 0 });
                out.push(Tri { v: [p[0], p[2], p[3]], n: [n[0], n[2], n[3]], uv: [uv[0], uv[2], uv[3]], mat: 0 });
                out.push(Tri { v: [[0.0, bot, 0.0], p[3], p[2]], n: [[0.0, 1.0, 0.0]; 3], uv: [[0.5, 0.5], [0.0, 0.0], [1.0, 0.0]], mat: 0 });
                if !cone {
                    out.push(Tri { v: [[0.0, top, 0.0], p[1], p[0]], n: [[0.0, -1.0, 0.0]; 3], uv: [[0.5, 0.5], [1.0, 0.0], [0.0, 0.0]], mat: 0 });
                }
            }
        }
    }
    out
}

fn geom_num(ev: &Eval, l: &Layer, id: &str, d: f64) -> f64 {
    l.props.sub("geometry").and_then(|g| g.get(id)).map(|p| ev.value(l, p).num()).unwrap_or(d)
}

pub fn primitive_of(ev: &Eval, l: &Layer) -> Option<Vec<Tri>> {
    let g = l.props.sub("geometry")?;
    let kind = g.get("kind").map(|p| p.value.index()).unwrap_or(0);
    let f = |id: &str, d: f64| geom_num(ev, l, id, d);
    Some(primitive(kind, f("width", 200.0), f("height", 200.0), f("depth", 200.0), f("radius", 100.0), f("tubeRadius", 30.0), f("segments", 48.0) as usize, f("rings", 24.0) as usize))
}

pub fn as_mesh(ev: &Eval, l: &Layer) -> bool {
    l.is_model() || super::extrude::active(ev, l)
}

pub fn model_item<'p>(ev: &Eval<'p>, l: &Layer) -> Option<&'p crate::model::Footage> {
    match &ev.project.item(l.source?)?.kind {
        ItemKind::Footage(f) if f.kind == crate::model::FootageKind::Model => Some(f),
        _ => None,
    }
}

pub fn model_bounds(ev: &Eval, l: &Layer) -> Option<[f64; 6]> {
    if let Some(f) = model_item(ev, l) {
        let (w, h, d) = (f.width as f64, f.height as f64, f.model_depth as f64);
        return Some([-w / 2.0, -h / 2.0, -d / 2.0, w / 2.0, h / 2.0, d / 2.0]);
    }
    let tris = primitive_of(ev, l)?;
    let mut b = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
    for t in &tris {
        for v in &t.v {
            for i in 0..3 {
                b[i] = b[i].min(v[i]);
                b[i + 3] = b[i + 3].max(v[i]);
            }
        }
    }
    (b[0] <= b[3]).then_some(b)
}

fn mat_override(ev: &Eval, l: &Layer, m: &Material, force: bool) -> MatEval {
    let g = l.props.sub("materialOptions");
    let on = force || g.and_then(|g| g.get("overrideMaterial")).map(|p| ev.value(l, p).boolean()).unwrap_or(false);
    if !on {
        return MatEval { base: m.base, texture: m.texture.clone(), metallic: m.metallic, roughness: m.roughness, emissive: m.emissive };
    }
    let col = |id: &str, d: [f64; 4]| g.and_then(|g| g.get(id)).map(|p| ev.value(l, p).color()).unwrap_or(d);
    let num = |id: &str, d: f64| g.and_then(|g| g.get(id)).map(|p| ev.value(l, p).num()).unwrap_or(d);
    let b = col("baseColor", [1.0; 4]);
    let e = col("emissive", [0.0, 0.0, 0.0, 1.0]);
    MatEval {
        base: [b[0] as f32, b[1] as f32, b[2] as f32, (b[3] * m.base[3] as f64) as f32],
        texture: if force { None } else { m.texture.clone() },
        metallic: (num("metallic", 0.0) / 100.0) as f32,
        roughness: (num("roughness", 50.0) / 100.0).clamp(0.02, 1.0) as f32,
        emissive: [e[0] as f32, e[1] as f32, e[2] as f32],
    }
}

pub fn layer_mesh(ev: &Eval, l: &Layer, media: &Media) -> Option<LayerMesh> {
    let world = ev.world_matrix(l);
    if let Some(f) = model_item(ev, l) {
        let model: Arc<Model> = media.models.get(&f.path)?;
        let m = world * model.to_layer();
        let nm = m.inverse().map(|i| i.transpose()).unwrap_or(m);
        let mats: Vec<MatEval> = model.materials.iter().map(|mt| mat_override(ev, l, mt, false)).collect();
        let mut tris = Vec::with_capacity(model.triangles());
        for mesh in &model.meshes {
            let pv: Vec<[f64; 3]> = mesh.pos.iter().map(|p| m.apply([p[0] as f64, p[1] as f64, p[2] as f64])).collect();
            let nv: Vec<[f32; 3]> = mesh
                .nrm
                .iter()
                .map(|n| {
                    let o = nm.apply_dir([n[0] as f64, n[1] as f64, n[2] as f64]);
                    let k = (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt().max(1e-12);
                    [(o[0] / k) as f32, (o[1] / k) as f32, (o[2] / k) as f32]
                })
                .collect();
            for t in mesh.idx.chunks_exact(3) {
                let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
                if a >= pv.len() || b >= pv.len() || c >= pv.len() {
                    continue;
                }
                let uv = |i: usize| mesh.uv.get(i).copied().unwrap_or([0.0; 2]);
                tris.push(Tri { v: [pv[a], pv[b], pv[c]], n: [nv[a], nv[b], nv[c]], uv: [uv(a), uv(b), uv(c)], mat: mesh.material as u16 });
            }
        }
        return Some(LayerMesh { tris, mats });
    }
    let (local, mats) = if super::extrude::active(ev, l) {
        super::extrude::tris(ev, l)?
    } else {
        (primitive_of(ev, l)?, vec![mat_override(ev, l, &Material { base: [1.0; 4], metallic: 0.0, roughness: 0.5, ..Material::default() }, true)])
    };
    let nm = world.inverse().map(|i| i.transpose()).unwrap_or(world);
    let tris = local
        .into_iter()
        .map(|t| {
            let n = t.n.map(|n| {
                let o = nm.apply_dir([n[0] as f64, n[1] as f64, n[2] as f64]);
                let k = (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt().max(1e-12);
                [(o[0] / k) as f32, (o[1] / k) as f32, (o[2] / k) as f32]
            });
            Tri { v: t.v.map(|v| world.apply(v)), n, uv: t.uv, mat: t.mat }
        })
        .collect();
    Some(LayerMesh { tris, mats })
}

#[derive(Clone, Copy)]
struct SV {
    s: [f64; 2],
    z: f64,
    w: [f64; 3],
    n: [f32; 3],
    uv: [f32; 2],
}

fn lerp_sv(a: &SV, b: &SV, t: f64) -> SV {
    let l = |x: f64, y: f64| x + (y - x) * t;
    let lf = |x: f32, y: f32| x + (y - x) * t as f32;
    SV { s: [l(a.s[0], b.s[0]), l(a.s[1], b.s[1])], z: l(a.z, b.z), w: [l(a.w[0], b.w[0]), l(a.w[1], b.w[1]), l(a.w[2], b.w[2])], n: [lf(a.n[0], b.n[0]), lf(a.n[1], b.n[1]), lf(a.n[2], b.n[2])], uv: [lf(a.uv[0], b.uv[0]), lf(a.uv[1], b.uv[1])] }
}

pub struct Proj {
    pub view: Mat4,
    pub zoom: f64,
    pub ortho: Option<f64>,
    pub cx: f64,
    pub cy: f64,
    pub k: f64,
}

impl Proj {
    pub fn of(cam: &Camera, cw: f64, ch: f64, k: f64) -> Proj {
        Proj { view: cam.view, zoom: cam.zoom, ortho: cam.ortho, cx: cw / 2.0, cy: ch / 2.0, k }
    }

    fn screen(&self, v: [f64; 3]) -> [f64; 2] {
        match self.ortho {
            Some(o) => [(self.cx + o * v[0]) * self.k, (self.cy + o * v[1]) * self.k],
            None => [(self.cx + self.zoom * v[0] / v[2]) * self.k, (self.cy + self.zoom * v[1] / v[2]) * self.k],
        }
    }
}

const NEAR: f64 = 1.0;

fn clip_tri(p: &Proj, t: &Tri) -> Vec<[SV; 3]> {
    let mut poly: Vec<SV> = (0..3)
        .map(|i| {
            let v = p.view.apply(t.v[i]);
            SV { s: [0.0; 2], z: v[2], w: v, n: t.n[i], uv: t.uv[i] }
        })
        .collect();
    if p.ortho.is_none() {
        let inside = |v: &SV| v.z >= NEAR;
        if poly.iter().all(|v| !inside(v)) {
            return vec![];
        }
        if !poly.iter().all(inside) {
            let mut out = vec![];
            for i in 0..poly.len() {
                let a = poly[i];
                let b = poly[(i + 1) % poly.len()];
                if inside(&a) {
                    out.push(a);
                }
                if inside(&a) != inside(&b) {
                    let t = (NEAR - a.z) / (b.z - a.z);
                    out.push(lerp_sv(&a, &b, t));
                }
            }
            poly = out;
        }
    }
    for v in &mut poly {
        v.s = p.screen(v.w);
    }
    let inv = p.view.inverse().unwrap_or(Mat4::IDENTITY);
    for v in &mut poly {
        v.w = inv.apply(v.w);
    }
    (1..poly.len().saturating_sub(1)).map(|k| [poly[0], poly[k], poly[k + 1]]).collect()
}

pub struct Frag {
    pub p: [f64; 3],
    pub n: [f32; 3],
    pub uv: [f32; 2],
    pub mat: u16,
    pub z: f64,
}

pub fn rasterize(tris: &[Tri], proj: &Proj, w: usize, h: usize, ss: usize, shade: &(dyn Fn(&Frag) -> [f32; 4] + Sync)) -> Option<(Tile, Vec<f32>)> {
    let ss = ss.max(1);
    let clipped: Vec<([SV; 3], u16)> = tris.par_iter().flat_map_iter(|t| clip_tri(proj, t).into_iter().map(move |c| (c, t.mat))).collect();
    if clipped.is_empty() {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (c, _) in &clipped {
        for v in c {
            x0 = x0.min(v.s[0]);
            y0 = y0.min(v.s[1]);
            x1 = x1.max(v.s[0]);
            y1 = y1.max(v.s[1]);
        }
    }
    let bx0 = (x0.floor() as i64 - 1).max(0);
    let by0 = (y0.floor() as i64 - 1).max(0);
    let bx1 = (x1.ceil() as i64 + 1).min(w as i64);
    let by1 = (y1.ceil() as i64 + 1).min(h as i64);
    if bx1 <= bx0 || by1 <= by0 {
        return None;
    }
    let (tw, th) = ((bx1 - bx0) as usize, (by1 - by0) as usize);
    let (sw, sh) = (tw * ss, th * ss);
    let band = 16usize;
    let nb = sh.div_ceil(band);
    let mut bins: Vec<Vec<usize>> = vec![vec![]; nb];
    for (i, (c, _)) in clipped.iter().enumerate() {
        let ya = c.iter().map(|v| v.s[1]).fold(f64::MAX, f64::min);
        let yb = c.iter().map(|v| v.s[1]).fold(f64::MIN, f64::max);
        let sa = (((ya - by0 as f64) * ss as f64).floor().max(0.0) as usize) / band;
        let sb = ((((yb - by0 as f64) * ss as f64).ceil().max(0.0) as usize) / band).min(nb.saturating_sub(1));
        if sa > sb {
            continue;
        }
        for b in bins.iter_mut().take(sb + 1).skip(sa) {
            b.push(i);
        }
    }
    let ssf = ss as f64;
    let bands: Vec<(Vec<[f32; 4]>, Vec<f32>)> = (0..nb)
        .into_par_iter()
        .map(|bi| {
            let ry0 = bi * band;
            let ry1 = (ry0 + band).min(sh);
            let rows = ry1 - ry0;
            let mut depth = vec![f32::INFINITY; rows * sw];
            let mut hit: Vec<(u32, f32, f32)> = vec![(u32::MAX, 0.0, 0.0); rows * sw];
            for &ti in &bins[bi] {
                let (c, _) = &clipped[ti];
                let pts: Vec<[f64; 2]> = c.iter().map(|v| [(v.s[0] - bx0 as f64) * ssf, (v.s[1] - by0 as f64) * ssf]).collect();
                let area = (pts[1][0] - pts[0][0]) * (pts[2][1] - pts[0][1]) - (pts[2][0] - pts[0][0]) * (pts[1][1] - pts[0][1]);
                if area.abs() < 1e-12 {
                    continue;
                }
                let xa = pts.iter().map(|p| p[0]).fold(f64::MAX, f64::min).floor().max(0.0) as usize;
                let xb = (pts.iter().map(|p| p[0]).fold(f64::MIN, f64::max).ceil() as usize).min(sw);
                let ya = (pts.iter().map(|p| p[1]).fold(f64::MAX, f64::min).floor().max(ry0 as f64) as usize).max(ry0);
                let yb = (pts.iter().map(|p| p[1]).fold(f64::MIN, f64::max).ceil() as usize).min(ry1);
                let persp = proj.ortho.is_none();
                for y in ya..yb {
                    let py = y as f64 + 0.5;
                    for x in xa..xb {
                        let px = x as f64 + 0.5;
                        let e = |a: [f64; 2], b: [f64; 2]| (b[0] - a[0]) * (py - a[1]) - (b[1] - a[1]) * (px - a[0]);
                        let w0 = e(pts[1], pts[2]) / area;
                        let w1 = e(pts[2], pts[0]) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 {
                            continue;
                        }
                        let (b0, b1, b2) = if persp {
                            let (i0, i1, i2) = (w0 / c[0].z, w1 / c[1].z, w2 / c[2].z);
                            let s = i0 + i1 + i2;
                            (i0 / s, i1 / s, i2 / s)
                        } else {
                            (w0, w1, w2)
                        };
                        let z = (b0 * c[0].z + b1 * c[1].z + b2 * c[2].z) as f32;
                        let i = (y - ry0) * sw + x;
                        if z < depth[i] {
                            depth[i] = z;
                            hit[i] = (ti as u32, b1 as f32, b2 as f32);
                        }
                    }
                }
            }
            let color: Vec<[f32; 4]> = hit
                .iter()
                .zip(&depth)
                .map(|(&(ti, b1, b2), z)| {
                    if ti == u32::MAX {
                        return [0.0; 4];
                    }
                    let (c, mat) = &clipped[ti as usize];
                    let b0 = 1.0 - b1 - b2;
                    let (fb0, fb1, fb2) = (b0 as f64, b1 as f64, b2 as f64);
                    let p = [0, 1, 2].map(|k| c[0].w[k] * fb0 + c[1].w[k] * fb1 + c[2].w[k] * fb2);
                    let n = [0, 1, 2].map(|k| c[0].n[k] * b0 + c[1].n[k] * b1 + c[2].n[k] * b2);
                    let uv = [0, 1].map(|k| c[0].uv[k] * b0 + c[1].uv[k] * b1 + c[2].uv[k] * b2);
                    shade(&Frag { p, n, uv, mat: *mat, z: *z as f64 })
                })
                .collect();
            (color, depth)
        })
        .collect();
    let mut img = Img::new(tw, th);
    let mut zbuf = vec![f32::INFINITY; tw * th];
    let inv = 1.0 / (ss * ss) as f32;
    let get = |sx: usize, sy: usize| -> ([f32; 4], f32) {
        let (b, r) = (sy / band, sy % band);
        let i = r * sw + sx;
        (bands[b].0[i], bands[b].1[i])
    };
    for y in 0..th {
        for x in 0..tw {
            let mut acc = [0.0f32; 4];
            let mut zm = f32::INFINITY;
            for dy in 0..ss {
                for dx in 0..ss {
                    let (c, z) = get(x * ss + dx, y * ss + dy);
                    for k in 0..4 {
                        acc[k] += c[k];
                    }
                    zm = zm.min(z);
                }
            }
            let i = (y * tw + x) * 4;
            for k in 0..4 {
                img.px[i + k] = acc[k] * inv;
            }
            zbuf[y * tw + x] = zm;
        }
    }
    Some((Tile { x0: bx0, y0: by0, img }, zbuf))
}

pub fn depth_map(tris: &[Tri], proj: &Proj, size: usize) -> Vec<f32> {
    let clipped: Vec<[SV; 3]> = tris.par_iter().flat_map_iter(|t| clip_tri(proj, t)).collect();
    let rows: Vec<Vec<f32>> = (0..size)
        .into_par_iter()
        .map(|y| {
            let py = y as f64 + 0.5;
            let mut row = vec![f32::INFINITY; size];
            for c in &clipped {
                let ya = c.iter().map(|v| v.s[1]).fold(f64::MAX, f64::min);
                let yb = c.iter().map(|v| v.s[1]).fold(f64::MIN, f64::max);
                if py < ya || py > yb {
                    continue;
                }
                let pts = [c[0].s, c[1].s, c[2].s];
                let area = (pts[1][0] - pts[0][0]) * (pts[2][1] - pts[0][1]) - (pts[2][0] - pts[0][0]) * (pts[1][1] - pts[0][1]);
                if area.abs() < 1e-12 {
                    continue;
                }
                let xa = pts.iter().map(|p| p[0]).fold(f64::MAX, f64::min).floor().max(0.0) as usize;
                let xb = (pts.iter().map(|p| p[0]).fold(f64::MIN, f64::max).ceil().max(0.0) as usize).min(size);
                for (x, d) in row.iter_mut().enumerate().take(xb).skip(xa) {
                    let px = x as f64 + 0.5;
                    let e = |a: [f64; 2], b: [f64; 2]| (b[0] - a[0]) * (py - a[1]) - (b[1] - a[1]) * (px - a[0]);
                    let w0 = e(pts[1], pts[2]) / area;
                    let w1 = e(pts[2], pts[0]) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 {
                        continue;
                    }
                    let z = if proj.ortho.is_none() { 1.0 / (w0 / c[0].z + w1 / c[1].z + w2 / c[2].z) } else { w0 * c[0].z + w1 * c[1].z + w2 * c[2].z } as f32;
                    if z < *d {
                        *d = z;
                    }
                }
            }
            row
        })
        .collect();
    rows.concat()
}
