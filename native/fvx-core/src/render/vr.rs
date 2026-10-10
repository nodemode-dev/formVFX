use rayon::prelude::*;

use super::comp::{RenderOpts, Renderer};
use super::gpu::Backend;
use super::image::Img;
use crate::eval::{Camera, Eval};
use crate::geom::Mat4;
use crate::model::prop::GroupKind;
use crate::model::{Comp, Project, Stereo, Vr};

pub const STEREO_FX: &str = "fx.control.stereo3d";

type V3 = [f64; 3];

const FACES: [(V3, V3, V3); 6] = [
    ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
    ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ([-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]),
    ([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]),
    ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
    ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
];

pub const FACE_NAMES: [&str; 6] = ["front", "right", "back", "left", "top", "bottom"];

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: V3) -> V3 {
    let k = dot(a, a).sqrt().max(1e-12);
    [a[0] / k, a[1] / k, a[2] / k]
}

fn axes(view: &Mat4) -> [V3; 3] {
    let m = &view.0;
    [norm([m[0][0], m[0][1], m[0][2]]), norm([m[1][0], m[1][1], m[1][2]]), norm([m[2][0], m[2][1], m[2][2]])]
}

fn to_world(ax: &[V3; 3], v: V3) -> V3 {
    [ax[0][0] * v[0] + ax[1][0] * v[1] + ax[2][0] * v[2], ax[0][1] * v[0] + ax[1][1] * v[1] + ax[2][1] * v[2], ax[0][2] * v[0] + ax[1][2] * v[1] + ax[2][2] * v[2]]
}

fn orient(vr: &Vr) -> Mat4 {
    Mat4::rot_y(vr.yaw) * Mat4::rot_x(-vr.pitch) * Mat4::rot_z(vr.roll)
}

fn face_camera(ax: &[V3; 3], eye: V3, r: V3, d: V3, f: V3, zoom: f64) -> Camera {
    let (rw, dw, fw) = (to_world(ax, r), to_world(ax, d), to_world(ax, f));
    let row = |a: V3| [a[0], a[1], a[2], -dot(a, eye)];
    let view = Mat4([row(rw), row(dw), row(fw), [0.0, 0.0, 0.0, 1.0]]);
    Camera { view, zoom, eye, forward: fw, ortho: None, dof: None }
}

fn sub_renderer<'a, 'b: 'a>(r: &Renderer<'b>, pj: &'a Project, view: Camera) -> Renderer<'a> {
    let opts = RenderOpts { view: Some(view), backend: Backend::Cpu, roi: None, ..r.opts.clone() };
    Renderer { project: pj, media: r.media, opts, pipe: r.pipe.clone() }
}

pub fn source_camera(pj: &Project, src: u64, t: f64) -> Option<Camera> {
    let c = pj.comp(src)?;
    Some(Eval::new(pj, src, c, t).camera())
}

pub fn faces(r: &Renderer, vr: &Vr, t: f64, s: f64) -> Option<(Vec<Img>, f64)> {
    let cam = source_camera(r.project, vr.source, t)?;
    let ax = axes(&cam.view);
    let rot = orient(vr);
    let face = vr.face.max(16) as f64;
    let mut pj = r.project.clone();
    {
        let c = pj.comp_mut(vr.source)?;
        c.width = face as u32;
        c.height = face as u32;
        c.vr = None;
        c.stereo = None;
    }
    let pj = &pj;
    let imgs: Vec<Img> = FACES
        .par_iter()
        .map(|(fr, fd, ff)| {
            let (fr, fd, ff) = (rot.apply_dir(*fr), rot.apply_dir(*fd), rot.apply_dir(*ff));
            let fc = face_camera(&ax, cam.eye, fr, fd, ff, face / 2.0);
            sub_renderer(r, pj, fc).render_comp(vr.source, t, s, 0)
        })
        .collect();
    Some((imgs, face * s))
}

fn lookup(faces: &[Img], size: f64, d: V3) -> [f32; 4] {
    let (ax, ay, az) = (d[0].abs(), d[1].abs(), d[2].abs());
    let i = if az >= ax && az >= ay {
        if d[2] > 0.0 { 0 } else { 2 }
    } else if ax >= ay {
        if d[0] > 0.0 { 1 } else { 3 }
    } else if d[1] < 0.0 {
        4
    } else {
        5
    };
    let (fr, fd, ff) = FACES[i];
    let z = dot(d, ff).max(1e-9);
    let (u, v) = (dot(d, fr) / z, dot(d, fd) / z);
    let img = &faces[i];
    let k = img.w as f64 / size.max(1e-9);
    let x = (size / 2.0 + size / 2.0 * u) * k;
    let y = (size / 2.0 + size / 2.0 * v) * k;
    img.sample(x, y, true)
}

pub fn equirect(faces: &[Img], size: f64, w: usize, h: usize) -> Img {
    let mut out = Img::new(w, h);
    out.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        let lat = (0.5 - (y as f64 + 0.5) / h as f64) * std::f64::consts::PI;
        for x in 0..w {
            let lon = ((x as f64 + 0.5) / w as f64 - 0.5) * std::f64::consts::TAU;
            let d = [lat.cos() * lon.sin(), -lat.sin(), lat.cos() * lon.cos()];
            let c = lookup(faces, size, d);
            row[x * 4..x * 4 + 4].copy_from_slice(&c);
        }
    });
    out
}

pub fn direction(u: f64, v: f64) -> V3 {
    let lat = (0.5 - v) * std::f64::consts::PI;
    let lon = (u - 0.5) * std::f64::consts::TAU;
    [lat.cos() * lon.sin(), -lat.sin(), lat.cos() * lon.cos()]
}

pub struct StereoParams {
    pub view: u32,
    pub interaxial: f64,
    pub converge: bool,
    pub offset: f64,
    pub swap: bool,
}

pub fn stereo_params(pj: &Project, st: &Stereo, t: f64) -> StereoParams {
    let width = pj.comp(st.source).map(|c| c.width as f64).unwrap_or(1920.0);
    let mut out = StereoParams { view: 0, interaxial: width * 0.035, converge: true, offset: 0.0, swap: false };
    let Some([cid, lid]) = st.controls else { return out };
    let Some(comp) = pj.comp(cid) else { return out };
    let Some(l) = comp.layer(lid) else { return out };
    let Some(g) = l.effects().and_then(|fx| fx.groups().find(|g| matches!(&g.kind, GroupKind::Effect { effect } if effect == STEREO_FX))) else { return out };
    let ev = Eval::new(pj, cid, comp, t);
    let v = |id: &str| g.get(id).map(|p| ev.value(l, p));
    if let Some(x) = v("view") {
        out.view = x.index();
    }
    if let Some(x) = v("sceneDepth") {
        out.interaxial = width * x.num() / 100.0;
    }
    if let Some(x) = v("convergence") {
        out.converge = x.boolean();
    }
    if let Some(x) = v("convergenceOffset") {
        out.offset = x.num();
    }
    if let Some(x) = v("swap") {
        out.swap = x.boolean();
    }
    out
}

fn shifted(img: &Img, dx: f64) -> Img {
    let mut out = Img::new(img.w, img.h);
    out.px.par_chunks_mut(img.w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..img.w {
            let c = img.sample(x as f64 + 0.5 - dx, y as f64 + 0.5, false);
            row[x * 4..x * 4 + 4].copy_from_slice(&c);
        }
    });
    out
}

pub fn eye(r: &Renderer, src: u64, t: f64, s: f64, sp: &StereoParams, left: bool) -> Option<Img> {
    let mut cam = source_camera(r.project, src, t)?;
    let side = if left != sp.swap { -1.0 } else { 1.0 };
    let half = sp.interaxial / 2.0;
    cam.view.0[0][3] -= side * half;
    let ax = axes(&cam.view);
    cam.eye = [cam.eye[0] + ax[0][0] * side * half, cam.eye[1] + ax[0][1] * side * half, cam.eye[2] + ax[0][2] * side * half];
    let mut img = sub_renderer(r, r.project, cam).render_comp(src, t, s, 0);
    if sp.converge {
        let dist = (cam.zoom + sp.offset).max(1.0);
        let k = cam.zoom * half / dist * s;
        img = shifted(&img, side * k);
    }
    Some(img)
}

fn luma(c: &[f32]) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

pub fn output(r: &Renderer, st: &Stereo, t: f64, s: f64) -> Option<Img> {
    let sp = stereo_params(r.project, st, t);
    match st.eye.as_str() {
        "left" => return eye(r, st.source, t, s, &sp, true),
        "right" => return eye(r, st.source, t, s, &sp, false),
        _ => {}
    }
    let (l, rr) = (eye(r, st.source, t, s, &sp, true)?, eye(r, st.source, t, s, &sp, false)?);
    let (w, h) = (l.w, l.h);
    let mut out = Img::new(w, h);
    match sp.view {
        1 => {
            out.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
                for x in 0..w {
                    let (src, sx) = if x < w / 2 { (&l, (x as f64 + 0.5) * 2.0) } else { (&rr, (x as f64 - (w / 2) as f64 + 0.5) * 2.0) };
                    let c = src.sample(sx, y as f64 + 0.5, true);
                    row[x * 4..x * 4 + 4].copy_from_slice(&c);
                }
            });
        }
        2 => return Some(l),
        3 => return Some(rr),
        4 => {
            for i in 0..w * h {
                let (a, b) = (&l.px[i * 4..i * 4 + 4], &rr.px[i * 4..i * 4 + 4]);
                out.px[i * 4] = luma(a);
                out.px[i * 4 + 1] = luma(b);
                out.px[i * 4 + 2] = luma(b);
                out.px[i * 4 + 3] = a[3].max(b[3]);
            }
        }
        _ => {
            for i in 0..w * h {
                let (a, b) = (&l.px[i * 4..i * 4 + 4], &rr.px[i * 4..i * 4 + 4]);
                out.px[i * 4] = a[0];
                out.px[i * 4 + 1] = b[1];
                out.px[i * 4 + 2] = b[2];
                out.px[i * 4 + 3] = a[3].max(b[3]);
            }
        }
    }
    Some(out)
}

pub fn frame(r: &Renderer, comp: &Comp, t: f64, s: f64) -> Option<Img> {
    let (w, h) = (((comp.width as f64 * s).round() as usize).max(1), ((comp.height as f64 * s).round() as usize).max(1));
    if let Some(vr) = &comp.vr {
        if r.project.comp(vr.source).is_none_or(|c| c.vr.is_some() || c.stereo.is_some()) {
            return Some(Img::new(w, h));
        }
        let (faces, size) = faces(r, vr, t, s)?;
        return Some(equirect(&faces, size, w, h));
    }
    let st = comp.stereo.as_ref()?;
    if r.project.comp(st.source).is_none_or(|c| c.vr.is_some() || c.stereo.is_some()) {
        return Some(Img::new(w, h));
    }
    let img = output(r, st, t, s)?;
    if img.w == w && img.h == h {
        return Some(img);
    }
    let mut out = Img::new(w, h);
    let (kx, ky) = (img.w as f64 / w as f64, img.h as f64 / h as f64);
    for y in 0..h {
        for x in 0..w {
            let c = img.sample((x as f64 + 0.5) * kx, (y as f64 + 0.5) * ky, true);
            out.px[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&c);
        }
    }
    Some(out)
}
