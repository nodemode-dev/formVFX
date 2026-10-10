use std::collections::HashSet;

use rayon::prelude::*;

use super::blend::{apply_matte, composite};
use super::image::{Img, Tile, warp};
use super::media::Media;
use super::vector::{self, DrawOp, OpKind, Paint};
use super::{shapes, text, textanim};
use crate::effects::{self, FxCtx, util::blur_scalar};
use crate::eval::{Camera, Eval};
use crate::geom::Mat3;
use crate::model::path::{BezPath, union_bounds};
use crate::model::{AlphaMode, Comp, GroupKind, ItemKind, Layer, LayerKind, MaskMode, Node, Project};

#[derive(Clone, Debug)]
pub struct RenderOpts {
    pub scale: f64,
    pub guides: bool,
    pub draft: bool,
    pub motion_blur: bool,
    pub roi: Option<[f64; 4]>,
    pub view: Option<Camera>,
    pub backend: super::gpu::Backend,
}

impl Default for RenderOpts {
    fn default() -> Self {
        RenderOpts { scale: 1.0, guides: true, draft: false, motion_blur: true, roi: None, view: None, backend: super::gpu::Backend::Auto }
    }
}

pub struct Renderer<'a> {
    pub project: &'a Project,
    pub media: &'a Media,
    pub opts: RenderOpts,
    pub pipe: super::color::Pipe,
}

type Converted = std::collections::VecDeque<(std::sync::Arc<Img>, String, std::sync::Arc<Img>)>;

fn converted() -> &'static parking_lot::Mutex<Converted> {
    static C: std::sync::OnceLock<parking_lot::Mutex<Converted>> = std::sync::OnceLock::new();
    C.get_or_init(|| parking_lot::Mutex::new(Converted::new()))
}

fn convert_frame(img: std::sync::Arc<Img>, key: &str, p: &super::ocio::Proc) -> std::sync::Arc<Img> {
    if let Some(hit) = converted().lock().iter().find(|e| std::sync::Arc::ptr_eq(&e.0, &img) && e.1 == key).map(|e| e.2.clone()) {
        return hit;
    }
    let mut o = (*img).clone();
    p.apply_img(&mut o);
    let out = std::sync::Arc::new(o);
    let mut c = converted().lock();
    c.push_back((img, key.to_string(), out.clone()));
    while c.len() > 24 {
        c.pop_front();
    }
    out
}

pub struct MaskEval {
    pub path: BezPath,
    pub mode: MaskMode,
    pub inverted: bool,
    pub feather: [f64; 2],
    pub opacity: f64,
    pub expansion: f64,
    pub points: Vec<super::maskfx::FeatherPoint>,
    pub linear: bool,
    pub samples: Vec<BezPath>,
}

enum Parts {
    Tile(Tile),
    Buffer { buf: Img, out: Mat3, nearest: bool, fx: Vec<super::gpu::GpuFx> },
}

enum Source {
    Vector(Vec<DrawOp>, Option<[f64; 4]>),
    Raster(std::sync::Arc<Img>, f64, bool),
    Blurred(Vec<DrawOp>, Vec<(Vec<DrawOp>, [f64; 2])>),
    Glyphs3d(Vec<textanim::Glyph3d>),
}

fn blurred_bounds(base: &[DrawOp], blurred: &[(Vec<DrawOp>, [f64; 2])]) -> Option<[f64; 4]> {
    let mut b = vector::ops_bounds(base);
    for (ops, r) in blurred {
        if let Some(x) = vector::ops_bounds(ops) {
            let p = r[0].max(r[1]) * 1.5 + 2.0;
            b = union_bounds(b, Some([x[0] - p, x[1] - p, x[2] + p, x[3] + p]));
        }
    }
    b
}

fn draw_blurred(buf: &mut Img, blurred: &[(Vec<DrawOp>, [f64; 2])], m: &Mat3, r: f64) {
    for (ops, rad) in blurred {
        let Some(mut tmp) = vector::rasterize(ops, m, buf.w, buf.h, 0, 0) else { continue };
        crate::effects::util::gaussian(&mut tmp, rad[0] * r * 0.5, rad[1] * r * 0.5, false);
        buf.px.par_chunks_mut(4).zip(tmp.px.par_chunks(4)).for_each(|(d, s)| {
            let k = 1.0 - s[3];
            for i in 0..4 {
                d[i] = s[i] + d[i] * k;
            }
        });
    }
}

pub fn active_masks(ev: &Eval, l: &Layer) -> Vec<MaskEval> {
    let mut out = vec![];
    let Some(g) = l.masks() else { return out };
    let text_path = l.props.sub("text").and_then(|t| t.sub("pathOptions")).and_then(|p| p.get("path")).map(|p| p.value.index() as usize).unwrap_or(0);
    for (i, m) in g.groups().enumerate() {
        let GroupKind::Mask { mode, inverted, .. } = &m.kind else { continue };
        if !m.enabled || *mode == MaskMode::None || i + 1 == text_path {
            continue;
        }
        let comp = ev.comp;
        let fd = comp.frame_duration();
        let layer_mb = comp.motion_blur && l.switches.motion_blur;
        let mb = m.get("motionBlur").map(|p| p.value.index()).unwrap_or(0);
        let at = if mb == 2 && layer_mb { ev.at(((ev.t - comp.shutter_phase / 360.0 * fd) / fd + 1e-6).floor() * fd) } else { ev.at(ev.t) };
        let Some(path) = m.get("path").map(|p| at.value(l, p)).and_then(|v| v.as_path().cloned()) else { continue };
        if path.vertices.len() < 2 {
            continue;
        }
        let samples: Vec<BezPath> = match (mb, layer_mb, m.get("path")) {
            (1, false, Some(pp)) if comp.motion_blur && pp.keys.len() >= 2 => {
                let n = comp.motion_blur_samples.clamp(2, 32) as usize;
                (0..n).filter_map(|k| {
                    let ts = ev.t + (comp.shutter_phase + k as f64 / (n - 1) as f64 * comp.shutter_angle) / 360.0 * fd;
                    ev.at(ts).value(l, pp).as_path().cloned()
                }).collect()
            }
            _ => vec![],
        };
        let f = |id: &str, d: f64| m.get(id).map(|p| at.value(l, p).num()).unwrap_or(d);
        let feather = m.get("feather").map(|p| at.value(l, p).vec2()).unwrap_or([0.0; 2]);
        let points = match m.get("featherPoints").map(|p| &p.value) {
            Some(crate::model::Value::Str(s)) => super::maskfx::sorted(super::maskfx::parse(s)),
            _ => vec![],
        };
        let linear = m.get("featherFalloff").is_some_and(|p| p.value.index() == 1);
        out.push(MaskEval { path, mode: *mode, inverted: *inverted, feather, opacity: f("opacity", 100.0) / 100.0, expansion: f("expansion", 0.0), points, linear, samples });
    }
    out
}

pub fn mask_coverage(masks: &[MaskEval], m: &Mat3, w: usize, h: usize, x0: i64, y0: i64) -> Option<Vec<f32>> {
    if masks.is_empty() {
        return None;
    }
    let k = m.max_scale();
    let n = w * h;
    let mut cov: Option<Vec<f32>> = None;
    for mk in masks {
        let one = |path: &BezPath| -> Vec<f32> {
            let paths = std::slice::from_ref(path);
            let mut c = vector::coverage(paths, m, w, h, x0, y0, false, 0.0);
            let e = mk.expansion * k;
            if e.abs() > 0.05 {
                let st = vector::coverage(paths, m, w, h, x0, y0, false, 2.0 * e.abs());
                if e > 0.0 {
                    c.par_iter_mut().zip(st.par_iter()).for_each(|(a, b)| *a = a.max(*b));
                } else {
                    c.par_iter_mut().zip(st.par_iter()).for_each(|(a, b)| *a *= 1.0 - b);
                }
            }
            super::maskfx::apply(&mut c, path, &mk.points, mk.linear, m, w, h, x0, y0, k);
            c
        };
        let mut c = if mk.samples.is_empty() {
            one(&mk.path)
        } else {
            let mut acc = vec![0.0f32; n];
            for sp in &mk.samples {
                acc.par_iter_mut().zip(one(sp).par_iter()).for_each(|(a, b)| *a += b);
            }
            let inv = 1.0 / mk.samples.len() as f32;
            acc.par_iter_mut().for_each(|a| *a *= inv);
            acc
        };
        let (fx, fy) = (mk.feather[0] * k, mk.feather[1] * k);
        if fx > 0.3 || fy > 0.3 {
            blur_scalar(&mut c, w, h, fx * 0.5, fy * 0.5);
        }
        let op = mk.opacity as f32;
        let inv = mk.inverted;
        c.par_iter_mut().for_each(|v| {
            let x = if inv { 1.0 - *v } else { *v };
            *v = x.clamp(0.0, 1.0) * op;
        });
        let base = cov.get_or_insert_with(|| vec![if matches!(mk.mode, MaskMode::Subtract | MaskMode::Intersect | MaskMode::Darken) { 1.0 } else { 0.0 }; n]);
        let mode = mk.mode;
        base.par_iter_mut().zip(c.par_iter()).for_each(|(a, b)| {
            *a = match mode {
                MaskMode::Add => *a + b - *a * b,
                MaskMode::Subtract => *a * (1.0 - b),
                MaskMode::Intersect => *a * b,
                MaskMode::Lighten => a.max(*b),
                MaskMode::Darken => a.min(*b),
                MaskMode::Difference => *a + b - 2.0 * *a * b,
                MaskMode::None => *a,
            }
        });
    }
    cov
}

fn local_scale(m: &Mat3, x: f64, y: f64) -> f64 {
    let a = m.apply(x, y);
    let b = m.apply(x + 1.0, y);
    let c = m.apply(x, y + 1.0);
    ((b[0] - a[0]).hypot(b[1] - a[1])).max((c[0] - a[0]).hypot(c[1] - a[1]))
}

fn quad_bounds(m: &Mat3, r: [f64; 4], w: usize, h: usize) -> Option<(i64, i64, usize, usize)> {
    let pts = [m.apply(r[0], r[1]), m.apply(r[2], r[1]), m.apply(r[2], r[3]), m.apply(r[0], r[3])];
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in pts {
        if !p[0].is_finite() || !p[1].is_finite() {
            return Some((0, 0, w, h));
        }
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    if !m.is_affine() {
        let wr = &m.0[2];
        if [(r[0], r[1]), (r[2], r[1]), (r[2], r[3]), (r[0], r[3])].iter().any(|(x, y)| wr[0] * x + wr[1] * y + wr[2] <= 0.0) {
            return Some((0, 0, w, h));
        }
    }
    let bx0 = (x0.floor() as i64 - 2).max(0);
    let by0 = (y0.floor() as i64 - 2).max(0);
    let bx1 = (x1.ceil() as i64 + 2).min(w as i64);
    let by1 = (y1.ceil() as i64 + 2).min(h as i64);
    if bx1 <= bx0 || by1 <= by0 {
        return None;
    }
    Some((bx0, by0, (bx1 - bx0) as usize, (by1 - by0) as usize))
}

impl<'a> Renderer<'a> {
    pub fn new(project: &'a Project, media: &'a Media, opts: RenderOpts) -> Renderer<'a> {
        Renderer { project, media, opts, pipe: super::color::Pipe::of(&project.settings) }
    }

    fn src_buf(&self, src: &Source, to_buf: &Mat3, bw: usize, bh: usize, r: f64) -> Img {
        let mut buf = raster_source(src, to_buf, bw, bh, r);
        if let (Some(a), Source::Vector(..) | Source::Blurred(..)) = (&self.pipe.authored, src) {
            a.apply_img(&mut buf);
        }
        buf
    }

    fn cconv(&self) -> Option<&super::ocio::Proc> {
        self.pipe.authored.as_deref()
    }

    pub fn comp_frame(&self, comp: u64, t: f64) -> Img {
        self.render_comp(comp, t, self.opts.scale, 0)
    }

    pub fn comp_size(&self, comp: &Comp, s: f64) -> (usize, usize) {
        (((comp.width as f64 * s).round() as usize).max(1), ((comp.height as f64 * s).round() as usize).max(1))
    }

    fn renders(&self, l: &Layer, depth: u32) -> bool {
        if matches!(l.kind, LayerKind::Camera | LayerKind::Light | LayerKind::Null) || l.environment {
            return false;
        }
        if l.switches.guide && !(depth == 0 && self.opts.guides) {
            return false;
        }
        if l.kind == LayerKind::Footage {
            let has_video = l.source.and_then(|s| self.project.item(s)).is_some_and(|i| matches!(&i.kind, ItemKind::Footage(f) if f.has_video));
            if !has_video {
                return false;
            }
        }
        true
    }

    fn order<'c>(&self, ev: &Eval, comp: &'c Comp, cam: Option<&Camera>) -> Vec<&'c Layer> {
        let rev: Vec<&Layer> = comp.layers.iter().rev().collect();
        let Some(cam) = cam else { return rev };
        let mut out = Vec::with_capacity(rev.len());
        let mut run: Vec<(&Layer, f64)> = vec![];
        let flush = |run: &mut Vec<(&'c Layer, f64)>, out: &mut Vec<&'c Layer>| {
            run.sort_by(|a, b| b.1.total_cmp(&a.1));
            out.extend(run.drain(..).map(|x| x.0));
        };
        for l in rev {
            if l.is_3d() && !l.is_camera() && !l.is_light() {
                let a = ev.tr_v3(l, "anchor", [0.0; 3]);
                let w = ev.world_matrix(l).apply(a);
                run.push((l, cam.depth(w)));
            } else {
                flush(&mut run, &mut out);
                out.push(l);
            }
        }
        flush(&mut run, &mut out);
        out
    }

    pub fn render_comp(&self, id: u64, t: f64, s: f64, depth: u32) -> Img {
        use super::gpu::Backend;
        if depth == 0
            && self.opts.backend != Backend::Cpu
            && self.project.settings.renderer != "cpu"
            && let Some(g) = super::gpu::current()
        {
            let key = id ^ ((s * 1000.0) as u64) << 40;
            let pick = if self.opts.backend == Backend::Gpu { Backend::Gpu } else { g.pick(key) };
            let started = std::time::Instant::now();
            if pick == Backend::Gpu {
                if let Some(img) = self.render_comp_gpu(&g, id, t, s) {
                    g.record(key, Backend::Gpu, started.elapsed().as_secs_f64() * 1000.0);
                    return img;
                }
            } else {
                let img = self.render_comp_cpu(id, t, s, depth);
                g.record(key, Backend::Cpu, started.elapsed().as_secs_f64() * 1000.0);
                return img;
            }
        }
        self.render_comp_cpu(id, t, s, depth)
    }

    fn render_comp_gpu(&self, g: &super::gpu::Gpu, id: u64, t: f64, s: f64) -> Option<Img> {
        use crate::model::BlendMode;
        let comp = self.project.comp(id)?;
        let (w, h) = self.comp_size(comp, s);
        let ev = Eval::new(self.project, id, comp, t);
        let cam = self.camera_for(&ev, comp, 0);
        if (comp.advanced_3d() && cam.is_some()) || self.pipe.blend.is_some() {
            return None;
        }
        let order = self.order(&ev, comp, cam.as_ref());
        if order.iter().any(|l| matches!(l.blend, BlendMode::Dissolve | BlendMode::DancingDissolve)) {
            return None;
        }
        let scene = cam.as_ref().and_then(|c| self.build_scene(&ev, comp, c, 0));
        let any_solo = comp.layers.iter().any(|l| l.switches.solo && self.renders(l, 0));
        let quant = match self.pipe.depth {
            8 => 255.0,
            16 => 32768.0,
            _ => 0.0,
        };
        let canvas = g.zeros(w, h)?;
        for l in order {
            if !l.switches.video || !l.is_active_at(t) || !self.renders(l, 0) || (any_solo && !l.switches.solo) || super::three::shadow_only(&ev, l) {
                continue;
            }
            if l.switches.adjustment {
                let mut c = g.download(&canvas);
                self.adjust(&ev, l, &mut c, s, cam.as_ref(), 0);
                self.pipe.quantize(&mut c);
                g.write(&canvas, &c);
                continue;
            }
            let mode = BlendMode::ALL.iter().position(|b| *b == l.blend).unwrap_or(0) as u32;
            let opacity = ev.opacity(l) as f32;
            let mb = self.opts.motion_blur && comp.motion_blur && l.switches.motion_blur && !self.opts.draft;
            let simple = !mb && l.track_matte.is_none() && l.env_background.is_none() && !l.is_model() && !(l.is_3d() && cam.is_some());
            let parts = if simple {
                match self.layer_parts(&ev, l, s, cam.as_ref(), 0, true) {
                    Some(p) => p,
                    None => continue,
                }
            } else {
                let tile = if mb { self.motion_blurred(&ev, comp, l, s, 0, scene.as_ref()) } else { self.layer_tile(&ev, l, s, cam.as_ref(), 0, scene.as_ref()) };
                let Some(mut tile) = tile else { continue };
                if let Some(m) = l.track_matte {
                    let mt = comp.layer(m.layer).and_then(|ml| {
                        let mut tl = self.layer_tile(&ev, ml, s, cam.as_ref(), 0, scene.as_ref())?;
                        tl.img.scale_all(ev.opacity(ml) as f32);
                        Some(tl)
                    });
                    apply_matte(&mut tile, mt.as_ref(), m.kind);
                }
                Parts::Tile(tile)
            };
            match parts {
                Parts::Tile(tile) => {
                    if tile.img.is_empty() {
                        continue;
                    }
                    let gb = g.upload(&tile.img)?;
                    let (x0, y0) = (tile.x0, tile.y0);
                    let bbox = [x0.max(0), y0.max(0), (x0 + tile.img.w as i64).min(w as i64), (y0 + tile.img.h as i64).min(h as i64)];
                    if bbox[2] <= bbox[0] || bbox[3] <= bbox[1] {
                        if mode != 32 && mode != 33 {
                            continue;
                        }
                    }
                    let plan = super::gpu::Warp { inv: Mat3::translate(-(x0 as f64), -(y0 as f64)), lscale: 1.0, levels: 0, bbox, nearest: false };
                    g.composite(&canvas, &gb, &plan, mode, opacity, l.preserve_transparency, quant);
                }
                Parts::Buffer { buf, out, nearest, fx } => {
                    let Some(plan) = super::gpu::plan(buf.w, buf.h, &out, w, h, nearest) else { continue };
                    let mut gb = g.upload(&buf)?;
                    gb = g.effects(gb, &fx)?;
                    for _ in 0..plan.levels {
                        gb = g.half(&gb)?;
                    }
                    g.composite(&canvas, &gb, &plan, mode, opacity, l.preserve_transparency, quant);
                }
            }
        }
        let mut img = g.download(&canvas);
        self.pipe.quantize(&mut img);
        Some(img)
    }

    fn render_comp_cpu(&self, id: u64, t: f64, s: f64, depth: u32) -> Img {
        let Some(comp) = self.project.comp(id) else { return Img::new(1, 1) };
        let (w, h) = self.comp_size(comp, s);
        let mut canvas = Img::new(w, h);
        if depth > 24 {
            return canvas;
        }
        let ev = Eval::new(self.project, id, comp, t);
        let cam = self.camera_for(&ev, comp, depth);
        let scene = cam.as_ref().and_then(|c| self.build_scene(&ev, comp, c, depth));
        let any_solo = comp.layers.iter().any(|l| l.switches.solo && self.renders(l, depth));
        let advanced = comp.advanced_3d() && cam.is_some();
        let mut run: Vec<(Tile, Vec<f32>, f32)> = vec![];
        for l in self.order(&ev, comp, cam.as_ref()) {
            if !l.switches.video || !l.is_active_at(t) || !self.renders(l, depth) || (any_solo && !l.switches.solo) || super::three::shadow_only(&ev, l) {
                continue;
            }
            if advanced
                && l.is_3d()
                && !l.switches.adjustment
                && let Some(c) = cam.as_ref()
            {
                if let Some(mut f) = self.depth_fragment(&ev, l, s, c, scene.as_ref(), depth) {
                    if let Some((dec, _)) = &self.pipe.blend {
                        dec.apply_img(&mut f.0.img);
                    }
                    run.push(f);
                }
                continue;
            }
            if !run.is_empty() {
                super::three::composite_depth(&mut canvas, &run);
                run.clear();
            }
            let mb = self.opts.motion_blur && comp.motion_blur && l.switches.motion_blur && !self.opts.draft;
            if l.switches.adjustment {
                if let Some((dec, enc)) = &self.pipe.blend {
                    enc.apply_img(&mut canvas);
                    self.adjust(&ev, l, &mut canvas, s, cam.as_ref(), depth);
                    dec.apply_img(&mut canvas);
                } else {
                    self.adjust(&ev, l, &mut canvas, s, cam.as_ref(), depth);
                    self.pipe.quantize(&mut canvas);
                }
                continue;
            }
            let tile = if mb { self.motion_blurred(&ev, comp, l, s, depth, scene.as_ref()) } else { self.layer_tile(&ev, l, s, cam.as_ref(), depth, scene.as_ref()) };
            let Some(mut tile) = tile else { continue };
            if let Some(m) = l.track_matte {
                let mt = comp.layer(m.layer).and_then(|ml| {
                    let mut tl = self.layer_tile(&ev, ml, s, cam.as_ref(), depth, scene.as_ref())?;
                    tl.img.scale_all(ev.opacity(ml) as f32);
                    Some(tl)
                });
                apply_matte(&mut tile, mt.as_ref(), m.kind);
            }
            if let Some((dec, _)) = &self.pipe.blend {
                dec.apply_img(&mut tile.img);
            }
            composite(&mut canvas, &tile, l.blend, ev.opacity(l) as f32, l.preserve_transparency, (t * comp.frame_rate).floor() as i64);
            if self.pipe.blend.is_none() {
                self.pipe.quantize(&mut canvas);
            }
        }
        if !run.is_empty() {
            super::three::composite_depth(&mut canvas, &run);
        }
        if let Some((_, enc)) = &self.pipe.blend {
            enc.apply_img(&mut canvas);
        }
        self.pipe.quantize(&mut canvas);
        canvas
    }

    fn camera_for(&self, ev: &Eval, comp: &Comp, depth: u32) -> Option<Camera> {
        if !comp.has_3d() {
            return None;
        }
        match (depth, self.opts.view) {
            (0, Some(v)) => Some(v),
            _ => Some(ev.camera()),
        }
    }

    fn depth_fragment(&self, ev: &Eval, l: &Layer, s: f64, cam: &Camera, scene: Option<&super::light::Scene>, depth: u32) -> Option<(Tile, Vec<f32>, f32)> {
        let op = ev.opacity(l) as f32;
        if super::mesh::as_mesh(ev, l) {
            let (mut tile, d) = self.model_tile(ev, l, s, cam, scene)?;
            if cam.dof.is_some() {
                let before = (tile.x0, tile.y0, tile.img.w);
                self.dof(cam, s, &mut tile, &d, self.comp_size(ev.comp, s));
                let finite: Vec<f32> = d.iter().copied().filter(|z| z.is_finite()).collect();
                let mean = if finite.is_empty() { f32::INFINITY } else { finite.iter().sum::<f32>() / finite.len() as f32 };
                let (tw, th) = (tile.img.w, tile.img.h);
                let oh = d.len() / before.2.max(1);
                let nd: Vec<f32> = (0..tw * th)
                    .map(|i| {
                        let (x, y) = ((i % tw) as i64 + tile.x0 - before.0, (i / tw) as i64 + tile.y0 - before.1);
                        if x >= 0 && y >= 0 && (x as usize) < before.2 && (y as usize) < oh {
                            let z = d[y as usize * before.2 + x as usize];
                            if z.is_finite() { z } else { mean }
                        } else {
                            mean
                        }
                    })
                    .collect();
                return Some((tile, nd, op));
            }
            return Some((tile, d, op));
        }
        let tile = self.layer_tile(ev, l, s, Some(cam), depth, scene)?;
        let d = self.plane_depths(ev, l, cam, s, &tile);
        Some((tile, d, op))
    }

    fn motion_blurred(&self, ev: &Eval, comp: &Comp, l: &Layer, s: f64, depth: u32, scene: Option<&super::light::Scene>) -> Option<Tile> {
        let n = comp.motion_blur_samples.clamp(2, 64) as usize;
        let fd = comp.frame_duration();
        let (w, h) = self.comp_size(comp, s);
        let mut acc = Img::new(w, h);
        let mut any = false;
        for k in 0..n {
            let frac = k as f64 / (n - 1) as f64;
            let ts = ev.t + (comp.shutter_phase + frac * comp.shutter_angle) / 360.0 * fd;
            let sev = ev.at(ts);
            let cam = self.camera_for(&sev, comp, depth);
            if let Some(tile) = self.layer_tile(&sev, l, s, cam.as_ref(), depth, scene) {
                any = true;
                let tw = tile.img.w;
                for yy in 0..tile.img.h {
                    let y = tile.y0 + yy as i64;
                    if y < 0 || y >= h as i64 {
                        continue;
                    }
                    for xx in 0..tw {
                        let x = tile.x0 + xx as i64;
                        if x < 0 || x >= w as i64 {
                            continue;
                        }
                        let si = (yy * tw + xx) * 4;
                        let di = (y as usize * w + x as usize) * 4;
                        for c in 0..4 {
                            acc.px[di + c] += tile.img.px[si + c];
                        }
                    }
                }
            }
        }
        if !any {
            return None;
        }
        acc.scale_all(1.0 / n as f32);
        Some(Tile::full(acc))
    }

    fn effects_on(&self, l: &Layer) -> bool {
        l.switches.effects && l.effects().is_some_and(|g| g.groups().any(|e| e.enabled && !matches!(&e.kind, GroupKind::Effect { effect } if effects::passive(effect))))
    }

    fn source(&self, ev: &Eval, l: &Layer, s: f64, scale: f64, depth: u32) -> Option<Source> {
        match l.kind {
            LayerKind::Shape => Some(Source::Vector(shapes::shape_ops(ev, l), None)),
            LayerKind::Text => {
                let doc = l.text_prop().map(|p| ev.value(l, p)).and_then(|v| v.as_text().cloned())?;
                if !textanim::has_animation(l) && !(l.is_3d() && l.props.sub("text").and_then(|t| t.get("perChar3d")).is_some_and(|p| p.value.boolean())) {
                    return Some(Source::Vector(text::text_ops(&doc), None));
                }
                let tr = textanim::render_text(ev, l, &doc);
                Some(match tr.glyphs3d {
                    Some(g) => Source::Glyphs3d(g),
                    None if !tr.blurred.is_empty() => Source::Blurred(tr.ops, tr.blurred),
                    None => Source::Vector(tr.ops, None),
                })
            }
            LayerKind::Solid => {
                let item = l.source.and_then(|s| self.project.item(s))?;
                let ItemKind::Solid(sd) = &item.kind else { return None };
                let (w, h) = (sd.width as f64, sd.height as f64);
                let c = sd.color;
                let op = DrawOp { paths: vec![BezPath::polygon(&[[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]], true)], paint: Paint::Solid([c[0] as f32, c[1] as f32, c[2] as f32, 1.0]), opacity: 1.0, kind: OpKind::Fill { even_odd: false }, transform: Mat3::IDENTITY };
                Some(Source::Vector(vec![op], Some([0.0, 0.0, w, h])))
            }
            LayerKind::Footage => {
                let item = l.source.and_then(|s| self.project.item(s))?;
                let ItemKind::Footage(orig) = &item.kind else { return None };
                let (f, proxied) = match &item.proxy {
                    Some(p) if p.enabled && !p.footage.missing => (&p.footage, true),
                    _ => (orig, false),
                };
                if f.kind == crate::model::FootageKind::Vector && !proxied {
                    let doc = super::svg::load(&f.path)?;
                    return Some(Source::Vector(super::svg::ops(&doc), Some([0.0, 0.0, f.width as f64, f.height as f64])));
                }
                let lt = ev.source_time(l);
                let blend = ev.comp.frame_blending && l.switches.frame_blend != crate::model::FrameBlend::Off && f.kind == crate::model::FootageKind::Video && f.frame_rate > 0.0;
                let img = if blend {
                    let x = lt * f.frame_rate;
                    let n = x.floor();
                    let frac = (x - n) as f32;
                    let a = self.media.frame(f, (n + 0.5) / f.frame_rate)?;
                    if frac < 1e-3 {
                        a
                    } else {
                        match self.media.frame(f, (n + 1.5) / f.frame_rate) {
                            Some(b) if b.w == a.w && b.h == a.h && l.switches.frame_blend == crate::model::FrameBlend::PixelMotion => {
                                let flow = super::vision::flow_cached(&a, &b);
                                std::sync::Arc::new(super::vision::pixel_motion(&a, &b, &flow, frac))
                            }
                            Some(b) if b.w == a.w && b.h == a.h => {
                                let mut o = (*a).clone();
                                o.px.par_iter_mut().zip(b.px.par_iter()).for_each(|(x, y)| *x += (*y - *x) * frac);
                                std::sync::Arc::new(o)
                            }
                            _ => a,
                        }
                    }
                } else {
                    self.media.frame(f, lt)?
                };
                let img = match f.alpha {
                    AlphaMode::Ignore => {
                        let mut o = (*img).clone();
                        o.px.par_chunks_mut(4).for_each(|p| {
                            let a = p[3].max(1e-6);
                            p[0] /= a;
                            p[1] /= a;
                            p[2] /= a;
                            p[3] = 1.0;
                        });
                        std::sync::Arc::new(o)
                    }
                    _ => img,
                };
                let img = match self.pipe.footage(f) {
                    Some(p) => {
                        let cfg = self.pipe.cfg.as_ref().map(|c| std::sync::Arc::as_ptr(c) as usize).unwrap_or(0);
                        let key = format!("{}|{}|{}|{cfg}|{}|{}|{}", self.pipe.working, self.pipe.linear, self.pipe.ocio, f.color_profile, f.transfer, f.preserve_rgb);
                        convert_frame(img, &key, &p)
                    }
                    None => img,
                };
                let k = if proxied { img.w as f64 / orig.width.max(1) as f64 } else { 1.0 };
                Some(Source::Raster(img, k, false))
            }
            LayerKind::Precomp => {
                let src = l.source?;
                let sub = self.project.comp(src)?;
                let lt = if l.props.get("timeRemap").is_some() { ev.source_time(l) } else { responsive_time(sub, l, ev.t) };
                if let Some(p) = self.project.item(src).and_then(|i| i.proxy.as_ref()).filter(|p| p.enabled && !p.footage.missing) {
                    let img = self.media.frame(&p.footage, lt)?;
                    let img = match self.pipe.footage(&p.footage) {
                        Some(pr) => convert_frame(img, &format!("proxy|{}|{}|{}|{}", self.pipe.working, self.pipe.linear, p.footage.color_profile, p.footage.transfer), &pr),
                        None => img,
                    };
                    let k = img.w as f64 / sub.width.max(1) as f64;
                    return Some(Source::Raster(img, k, false));
                }
                let sn = if l.switches.collapse { (s * scale).clamp(1.0 / 64.0, 4.0) } else { (s * scale).clamp(1.0 / 64.0, 1.0) };
                let mut sn = sn;
                if sub.preserve_resolution {
                    sn = 1.0;
                }
                let lt = if sub.preserve_frame_rate { (lt * sub.frame_rate + 1e-6).floor() / sub.frame_rate } else { lt };
                let img = match super::essential::patched(self.project, ev, l, src) {
                    Some(pj) => Renderer { project: &pj, media: self.media, opts: self.opts.clone(), pipe: self.pipe.clone() }.render_comp(src, lt, sn, depth + 1),
                    None => self.render_comp(src, lt, sn, depth + 1),
                };
                Some(Source::Raster(std::sync::Arc::new(img), sn, true))
            }
            _ => None,
        }
    }

    pub fn layer_tile(&self, ev: &Eval, l: &Layer, s: f64, cam: Option<&Camera>, depth: u32, scene: Option<&super::light::Scene>) -> Option<Tile> {
        if let Some(light) = l.env_background {
            let c = cam.copied().unwrap_or_else(|| ev.camera());
            return self.sky_tile(ev, s, &c, scene, light);
        }
        if super::mesh::as_mesh(ev, l) {
            let c = cam.copied().unwrap_or_else(|| ev.camera());
            let (mut tile, d) = self.model_tile(ev, l, s, &c, scene)?;
            if c.dof.is_some() {
                self.dof(&c, s, &mut tile, &d, self.comp_size(ev.comp, s));
            }
            return Some(tile);
        }
        let mut tile = self.layer_tile_base(ev, l, s, cam, depth)?;
        if l.is_3d()
            && let Some(c) = cam
        {
            if let Some(sc) = scene {
                self.plane_light(ev, l, c, s, sc, &mut tile);
            }
            if c.dof.is_some() {
                let d = self.plane_depths(ev, l, c, s, &tile);
                self.dof(c, s, &mut tile, &d, self.comp_size(ev.comp, s));
            }
        }
        Some(tile)
    }

    fn layer_tile_base(&self, ev: &Eval, l: &Layer, s: f64, cam: Option<&Camera>, depth: u32) -> Option<Tile> {
        let (cw, ch) = self.comp_size(ev.comp, s);
        match self.layer_parts(ev, l, s, cam, depth, false)? {
            Parts::Tile(t) => Some(t),
            Parts::Buffer { buf, out, nearest, .. } => super::image::with_nearest(nearest, || warp(&buf, &out, cw, ch, false)),
        }
    }

    fn layer_parts(&self, ev: &Eval, l: &Layer, s: f64, cam: Option<&Camera>, depth: u32, gpu: bool) -> Option<Parts> {
        let comp = ev.comp;
        let (cw, ch) = self.comp_size(comp, s);
        let m_lc = match cam {
            Some(c) if l.is_3d() => ev.layer_to_screen(l, c),
            _ => ev.layer_to_comp(l),
        };
        let m = Mat3::scale(s, s) * m_lc;
        let (sw, sh) = self.project.source_size(l, comp);
        let lscale = local_scale(&m_lc, sw / 2.0, sh / 2.0).max(1e-6);
        if l.switches.quality == crate::model::Quality::Wireframe {
            let b = content_bounds(ev, l).unwrap_or([0.0, 0.0, sw, sh]);
            let k = m.max_scale().max(1e-6);
            let op = DrawOp { paths: vec![BezPath::polygon(&[[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]], true)], paint: Paint::Solid([1.0, 1.0, 1.0, 1.0]), opacity: 1.0, kind: OpKind::Stroke { width: 1.0 / k, cap: 0, join: 0, miter: 4.0, dash: vec![], dash_offset: 0.0 }, transform: Mat3::IDENTITY };
            let img = vector::rasterize(&[op], &m, cw, ch, 0, 0)?;
            return Some(Parts::Tile(Tile { x0: 0, y0: 0, img }));
        }
        let src = self.source(ev, l, s, lscale, depth)?;
        if let Source::Glyphs3d(gs) = &src {
            return self.glyphs3d_tile(ev, l, gs, s, cam).map(Parts::Tile);
        }
        let masks = active_masks(ev, l);
        let fx = self.effects_on(l);
        let sty = super::styles::active(l);
        if let Source::Vector(ops, rect) = &src
            && !fx
            && !sty
        {
            let b = union_bounds(vector::ops_bounds(ops), *rect)?;
            let (x0, y0, w, h) = quad_bounds(&m, b, cw, ch)?;
            let mut img = vector::rasterize(ops, &m, w, h, x0, y0)?;
            if let Some(a) = &self.pipe.authored {
                a.apply_img(&mut img);
            }
            if let Some(cov) = mask_coverage(&masks, &m, w, h, x0, y0) {
                img.multiply_alpha(&cov);
            }
            self.pipe.quantize(&mut img);
            return Some(Parts::Tile(Tile { x0, y0, img }));
        }
        let mut pad = 0.0;
        let fx_groups: Vec<&crate::model::Group> = if fx { l.effects().map(|g| g.groups().filter(|e| e.enabled).collect()).unwrap_or_default() } else { vec![] };
        let lt = l.layer_time(ev.t);
        let fps = comp.frame_rate;
        let frame = fx_frame(l, sw, sh);
        let base_rect = match &src {
            Source::Vector(ops, rect) => union_bounds(vector::ops_bounds(ops), *rect).unwrap_or(frame),
            Source::Blurred(ops, bl) => blurred_bounds(ops, bl).unwrap_or(frame),
            Source::Raster(..) | Source::Glyphs3d(_) => frame,
        };
        let base_rect = if fx { union_bounds(Some(base_rect), Some(frame)).unwrap_or(frame) } else { base_rect };
        let mut r = match &src {
            Source::Vector(..) | Source::Blurred(..) | Source::Glyphs3d(_) => (s * lscale).clamp(1.0 / 64.0, 4.0),
            Source::Raster(_, sn, true) => *sn,
            Source::Raster(..) => (s * lscale).clamp(1.0 / 64.0, 1.0),
        };
        let params: Vec<(&effects::EffectSpec, std::collections::HashMap<String, crate::model::Value>)> = fx_groups
            .iter()
            .filter_map(|g| {
                let GroupKind::Effect { effect } = &g.kind else { return None };
                let spec = effects::find(effect)?;
                Some((spec, effects::params_at(ev, l, g)))
            })
            .collect();
        for (spec, p) in &params {
            let c = FxCtx { p, t: lt, r: 1.0, origin: [0.0; 2], size: [sw, sh], fps, host: None, cconv: self.cconv(), settings: Some(&self.project.settings) };
            pad += (spec.pad)(&c).max(0.0);
        }
        if sty {
            pad += super::styles::pad(ev, l);
        }
        let rect = [base_rect[0] - pad, base_rect[1] - pad, base_rect[2] + pad, base_rect[3] + pad];
        let max_side = 8192.0;
        let span = (rect[2] - rect[0]).max(rect[3] - rect[1]).max(1.0);
        if span * r > max_side {
            r = max_side / span;
        }
        let bw = ((rect[2] - rect[0]) * r).ceil().max(1.0) as usize;
        let bh = ((rect[3] - rect[1]) * r).ceil().max(1.0) as usize;
        let to_buf = Mat3::scale(r, r) * Mat3::translate(-rect[0], -rect[1]);
        let mut buf = self.src_buf(&src, &to_buf, bw, bh, r);
        if let Some(cov) = mask_coverage(&masks, &to_buf, bw, bh, 0, 0) {
            buf.multiply_alpha(&cov);
        }
        self.pipe.quantize(&mut buf);
        let mut cpu_upto = params.len();
        let mut gfx: Vec<super::gpu::GpuFx> = vec![];
        if gpu && !sty {
            let quant = match self.pipe.depth {
                8 => 255.0,
                16 => 32768.0,
                _ => 0.0,
            };
            let mut tail = vec![];
            while cpu_upto > 0 {
                let (spec, p) = &params[cpu_upto - 1];
                let c = FxCtx { p, t: lt, r, origin: [rect[0] - frame[0], rect[1] - frame[1]], size: [sw, sh], fps, host: None, cconv: self.cconv(), settings: Some(&self.project.settings) };
                let Some(v) = super::gpu::effect(spec.id, &c, quant) else { break };
                tail.push(v);
                cpu_upto -= 1;
            }
            tail.reverse();
            gfx = tail.concat();
        }
        for (i, (spec, p)) in params.iter().enumerate().take(cpu_upto) {
            let hx = Hx { rd: self, ev, l, s, depth, rect, r, bw, bh, upto: i };
            let c = FxCtx { p, t: lt, r, origin: [rect[0] - frame[0], rect[1] - frame[1]], size: [sw, sh], fps, host: Some(&hx), cconv: self.cconv(), settings: Some(&self.project.settings) };
            (spec.render)(&c, &mut buf);
            self.pipe.quantize(&mut buf);
        }
        if sty {
            super::styles::apply(ev, l, &mut buf, r);
        }
        let out = m * Mat3::translate(rect[0], rect[1]) * Mat3::scale(1.0 / r, 1.0 / r);
        Some(Parts::Buffer { buf, out, nearest: l.switches.quality == crate::model::Quality::Draft, fx: gfx })
    }

    fn glyphs3d_tile(&self, ev: &Eval, l: &Layer, gs: &[textanim::Glyph3d], s: f64, cam: Option<&Camera>) -> Option<Tile> {
        let comp = ev.comp;
        let (cw, ch) = self.comp_size(comp, s);
        let cam = cam.copied().unwrap_or_else(|| ev.camera());
        let (w, h) = (comp.width as f64, comp.height as f64);
        let world = ev.world_matrix(l);
        let pv = cam.projection(w, h) * cam.view;
        let mut items: Vec<(f64, Vec<DrawOp>)> = gs
            .iter()
            .map(|g| {
                let wm = world * g.matrix;
                let depth = cam.depth(wm.apply(g.anchor));
                let m3 = Mat3::scale(s, s) * (pv * wm).plane_to_mat3();
                (depth, g.ops.iter().map(|op| DrawOp { transform: m3 * op.transform, ..op.clone() }).collect())
            })
            .collect();
        items.sort_by(|a, b| b.0.total_cmp(&a.0));
        let ops: Vec<DrawOp> = items.into_iter().flat_map(|x| x.1).collect();
        let mut img = vector::rasterize(&ops, &Mat3::IDENTITY, cw, ch, 0, 0)?;
        if self.effects_on(l)
            && let Some(g) = l.effects()
        {
            let lt = l.layer_time(ev.t);
            for e in g.groups().filter(|e| e.enabled) {
                let GroupKind::Effect { effect } = &e.kind else { continue };
                let Some(spec) = effects::find(effect) else { continue };
                let p = effects::params_at(ev, l, e);
                let c = FxCtx { p: &p, t: lt, r: s, origin: [0.0, 0.0], size: [w, h], fps: comp.frame_rate, host: None, cconv: self.cconv(), settings: Some(&self.project.settings) };
                (spec.render)(&c, &mut img);
            }
        }
        Some(Tile { x0: 0, y0: 0, img })
    }

    fn adjust(&self, ev: &Eval, l: &Layer, canvas: &mut Img, s: f64, cam: Option<&Camera>, _depth: u32) {
        if !self.effects_on(l) {
            return;
        }
        let comp = ev.comp;
        let (cw, ch) = (canvas.w, canvas.h);
        let m_lc = match cam {
            Some(c) if l.is_3d() => ev.layer_to_screen(l, c),
            _ => ev.layer_to_comp(l),
        };
        let m = Mat3::scale(s, s) * m_lc;
        let (sw, sh) = self.project.source_size(l, comp);
        let area = DrawOp { paths: vec![BezPath::polygon(&[[0.0, 0.0], [sw, 0.0], [sw, sh], [0.0, sh]], true)], paint: Paint::Solid([1.0; 4]), opacity: 1.0, kind: OpKind::Fill { even_odd: false }, transform: Mat3::IDENTITY };
        let mut cov: Vec<f32> = vector::rasterize(&[area], &m, cw, ch, 0, 0).map(|i| i.px.chunks_exact(4).map(|p| p[3]).collect()).unwrap_or_else(|| vec![0.0; cw * ch]);
        let masks = active_masks(ev, l);
        if let Some(mc) = mask_coverage(&masks, &m, cw, ch, 0, 0) {
            cov.iter_mut().zip(mc.iter()).for_each(|(a, b)| *a *= b);
        }
        let op = ev.opacity(l) as f32;
        let lt = l.layer_time(ev.t);
        let mut fxbuf = canvas.clone();
        if let Some(g) = l.effects() {
            for e in g.groups().filter(|e| e.enabled) {
                let GroupKind::Effect { effect } = &e.kind else { continue };
                let Some(spec) = effects::find(effect) else { continue };
                let p = effects::params_at(ev, l, e);
                let c = FxCtx { p: &p, t: lt, r: s, origin: [0.0, 0.0], size: [comp.width as f64, comp.height as f64], fps: comp.frame_rate, host: None, cconv: self.cconv(), settings: Some(&self.project.settings) };
                (spec.render)(&c, &mut fxbuf);
                self.pipe.quantize(&mut fxbuf);
            }
        }
        canvas.px.par_chunks_mut(4).zip(fxbuf.px.par_chunks(4)).zip(cov.par_iter()).for_each(|((d, f), c)| {
            let k = c * op;
            for i in 0..4 {
                d[i] += (f[i] - d[i]) * k;
            }
        });
    }
}

pub fn fx_frame(l: &Layer, sw: f64, sh: f64) -> [f64; 4] {
    match l.kind {
        LayerKind::Shape | LayerKind::Text => [-sw / 2.0, -sh / 2.0, sw / 2.0, sh / 2.0],
        _ => [0.0, 0.0, sw, sh],
    }
}

fn raster_source(src: &Source, to_buf: &Mat3, bw: usize, bh: usize, r: f64) -> Img {
    match src {
        Source::Vector(ops, _) => vector::rasterize(ops, to_buf, bw, bh, 0, 0).unwrap_or_else(|| Img::new(bw, bh)),
        Source::Glyphs3d(_) => Img::new(bw, bh),
        Source::Blurred(ops, bl) => {
            let mut b = vector::rasterize(ops, to_buf, bw, bh, 0, 0).unwrap_or_else(|| Img::new(bw, bh));
            draw_blurred(&mut b, bl, to_buf, r);
            b
        }
        Source::Raster(img, sn, _) => {
            let mm = *to_buf * Mat3::scale(1.0 / sn, 1.0 / sn);
            match warp(img, &mm, bw, bh, false) {
                Some(t) => {
                    let mut full = Img::new(bw, bh);
                    let tw = t.img.w;
                    for yy in 0..t.img.h {
                        let y = t.y0 as usize + yy;
                        if y >= bh {
                            continue;
                        }
                        let si = yy * tw * 4;
                        let di = (y * bw + t.x0 as usize) * 4;
                        let len = (tw.min(bw - t.x0 as usize)) * 4;
                        full.px[di..di + len].copy_from_slice(&t.img.px[si..si + len]);
                    }
                    full
                }
                None => Img::new(bw, bh),
            }
        }
    }
}

struct Hx<'r> {
    rd: &'r Renderer<'r>,
    ev: &'r Eval<'r>,
    l: &'r Layer,
    s: f64,
    depth: u32,
    rect: [f64; 4],
    r: f64,
    bw: usize,
    bh: usize,
    upto: usize,
}

impl Renderer<'_> {
    pub fn sample_layer(&self, ev: &Eval, l: &Layer, at: [f64; 2], radius: [f64; 2], post: bool) -> Option<[f64; 4]> {
        let (sw, sh) = self.project.source_size(l, ev.comp);
        let frame = fx_frame(l, sw, sh);
        let (rx, ry) = (radius[0].abs().max(0.5), radius[1].abs().max(0.5));
        let (x0, x1) = ((at[0] - rx).max(frame[0]), (at[0] + rx).min(frame[2]));
        let (y0, y1) = ((at[1] - ry).max(frame[1]), (at[1] + ry).min(frame[3]));
        if x1 <= x0 || y1 <= y0 {
            return Some([0.0; 4]);
        }
        let (rect, r) = if post {
            (frame, 1.0f64.min(4096.0 / sw.max(sh).max(1.0)))
        } else {
            ([x0.floor(), y0.floor(), x1.ceil(), y1.ceil()], 1.0)
        };
        let (bw, bh) = (((rect[2] - rect[0]) * r).ceil().max(1.0) as usize, ((rect[3] - rect[1]) * r).ceil().max(1.0) as usize);
        let img = self.layer_buffer(ev, l, 1.0, 1, rect, r, bw, bh, Some(if post { usize::MAX } else { 0 }))?;
        let px0 = (((x0 - rect[0]) * r).floor().max(0.0) as usize).min(bw.saturating_sub(1));
        let py0 = (((y0 - rect[1]) * r).floor().max(0.0) as usize).min(bh.saturating_sub(1));
        let px1 = (((x1 - rect[0]) * r).ceil() as usize).clamp(px0 + 1, bw);
        let py1 = (((y1 - rect[1]) * r).ceil() as usize).clamp(py0 + 1, bh);
        let mut acc = [0.0f64; 4];
        let mut n = 0.0;
        for y in py0..py1 {
            for x in px0..px1 {
                let i = (y * img.w + x) * 4;
                for (k, a) in acc.iter_mut().enumerate() {
                    *a += img.px[i + k] as f64;
                }
                n += 1.0;
            }
        }
        if n == 0.0 {
            return Some([0.0; 4]);
        }
        let a = acc[3] / n;
        let un = |v: f64| if a > 1e-6 { v / n / a } else { 0.0 };
        Some([un(acc[0]), un(acc[1]), un(acc[2]), a])
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn layer_buffer(&self, ev: &Eval, l: &Layer, s: f64, depth: u32, rect: [f64; 4], r: f64, bw: usize, bh: usize, fx_upto: Option<usize>) -> Option<Img> {
        if !l.is_active_at(ev.t) && !matches!(l.kind, LayerKind::Footage | LayerKind::Precomp | LayerKind::Solid) {
            return None;
        }
        let m_lc = ev.layer_to_comp(l);
        let (sw, sh) = self.project.source_size(l, ev.comp);
        let lscale = local_scale(&m_lc, sw / 2.0, sh / 2.0).max(1e-6);
        let src = self.source(ev, l, s, lscale, depth + 1)?;
        let to_buf = Mat3::scale(r, r) * Mat3::translate(-rect[0], -rect[1]);
        let mut buf = self.src_buf(&src, &to_buf, bw, bh, r);
        let Some(upto) = fx_upto else { return Some(buf) };
        if let Some(cov) = mask_coverage(&active_masks(ev, l), &to_buf, bw, bh, 0, 0) {
            buf.multiply_alpha(&cov);
        }
        if !self.effects_on(l) {
            return Some(buf);
        }
        let lt = l.layer_time(ev.t);
        let groups: Vec<&crate::model::Group> = l.effects().map(|g| g.groups().filter(|e| e.enabled).collect()).unwrap_or_default();
        for g in groups.into_iter().take(upto) {
            let GroupKind::Effect { effect } = &g.kind else { continue };
            let Some(spec) = effects::find(effect) else { continue };
            let p = effects::params_at(ev, l, g);
            let frame = fx_frame(l, sw, sh);
            let c = FxCtx { p: &p, t: lt, r, origin: [rect[0] - frame[0], rect[1] - frame[1]], size: [sw, sh], fps: ev.comp.frame_rate, host: None, cconv: self.cconv(), settings: Some(&self.project.settings) };
            (spec.render)(&c, &mut buf);
        }
        Some(buf)
    }
}

impl effects::FxHost for Hx<'_> {
    fn frame(&self, dt: f64) -> Option<Img> {
        let ev = self.ev.at(self.ev.t + dt);
        self.rd.layer_buffer(&ev, self.l, self.s, self.depth, self.rect, self.r, self.bw, self.bh, Some(self.upto))
    }

    fn original(&self) -> Option<Img> {
        self.rd.layer_buffer(self.ev, self.l, self.s, self.depth, self.rect, self.r, self.bw, self.bh, Some(0))
    }

    fn param_at(&self, id: &str, dt: f64) -> Option<crate::model::Value> {
        let g = self.l.effects()?.groups().filter(|e| e.enabled).nth(self.upto)?;
        let ev = self.ev.at(self.ev.t + dt);
        effects::params_at(&ev, self.l, g).remove(id)
    }

    fn layer(&self, id: u64, with_fx: bool, dt: f64) -> Option<(Img, [f64; 2])> {
        let ol = self.ev.comp.layer(id)?;
        let ev = self.ev.at(self.ev.t + dt);
        let (sw, sh) = self.rd.project.source_size(ol, self.ev.comp);
        let r = self.r.min(4096.0 / sw.max(sh).max(1.0));
        let (bw, bh) = ((sw * r).ceil().max(1.0) as usize, (sh * r).ceil().max(1.0) as usize);
        let img = self.rd.layer_buffer(&ev, ol, self.s, self.depth, fx_frame(ol, sw, sh), r, bw, bh, if with_fx { Some(usize::MAX) } else { None })?;
        if (r - self.r).abs() < 1e-9 {
            return Some((img, [sw, sh]));
        }
        let k = self.r / r;
        let (w2, h2) = ((sw * self.r).ceil().max(1.0) as usize, (sh * self.r).ceil().max(1.0) as usize);
        let t = warp(&img, &Mat3::scale(k, k), w2, h2, true)?;
        let mut full = Img::new(w2, h2);
        for yy in 0..t.img.h {
            let y = t.y0 as usize + yy;
            if y >= h2 {
                continue;
            }
            let len = t.img.w.min(w2 - t.x0 as usize) * 4;
            let di = (y * w2 + t.x0 as usize) * 4;
            full.px[di..di + len].copy_from_slice(&t.img.px[yy * t.img.w * 4..yy * t.img.w * 4 + len]);
        }
        Some((full, [sw, sh]))
    }

    fn layer_in_comp(&self, id: u64, dt: f64) -> Option<Img> {
        let ol = self.ev.comp.layer(id)?;
        let ev = self.ev.at(self.ev.t + dt);
        let tile = self.rd.layer_tile(&ev, ol, self.s, None, self.depth + 1, None)?;
        let m = Mat3::scale(self.s, self.s) * self.ev.layer_to_comp(self.l);
        let mut out = Img::new(self.bw, self.bh);
        let (r, rect) = (self.r, self.rect);
        let w = self.bw;
        out.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                let lx = (x as f64 + 0.5) / r + rect[0];
                let ly = (y as f64 + 0.5) / r + rect[1];
                let c = m.apply(lx, ly);
                let px = tile.img.sample(c[0] - tile.x0 as f64, c[1] - tile.y0 as f64, false);
                row[x * 4..x * 4 + 4].copy_from_slice(&px);
            }
        });
        Some(out)
    }

    fn mask(&self, index: usize) -> Option<BezPath> {
        let g = self.l.masks()?.groups().nth(index.checked_sub(1)?)?;
        let path = g.get("path").map(|p| self.ev.value(self.l, p)).and_then(|v| v.as_path().cloned())?;
        Some(path.transformed(&(Mat3::scale(self.r, self.r) * Mat3::translate(-self.rect[0], -self.rect[1]))))
    }

    fn audio(&self, id: Option<u64>, t0: f64, t1: f64, rate: u32) -> Vec<f32> {
        let frames = ((t1 - t0) * rate as f64).round().max(1.0) as usize;
        super::audio::mix_layer(self.rd.project, self.rd.media, self.ev.comp_id, id.unwrap_or(self.l.id), t0, frames, rate)
    }

    fn comp_time(&self) -> f64 {
        self.ev.t
    }

    fn aux(&self) -> Option<std::sync::Arc<super::exr::Aux>> {
        let item = self.l.source.and_then(|s| self.rd.project.item(s))?;
        let ItemKind::Footage(f) = &item.kind else { return None };
        self.rd.media.aux(f, self.ev.source_time(self.l))
    }

    fn shutter_angle(&self) -> f64 {
        self.ev.comp.shutter_angle
    }

    fn frame_rate(&self) -> f64 {
        self.ev.comp.frame_rate
    }

    fn stretch(&self) -> f64 {
        self.l.stretch
    }
}

pub fn responsive_time(sub: &Comp, l: &Layer, t: f64) -> f64 {
    let lt = l.layer_time(t);
    let s = l.stretch / 100.0;
    if (s - 1.0).abs() < 1e-9 || s <= 0.0 {
        return lt;
    }
    let d = sub.duration;
    let mut regions: Vec<(f64, f64)> = sub.markers.iter().filter(|m| m.protected && m.duration > 0.0).map(|m| (m.time.clamp(0.0, d), (m.time + m.duration).clamp(0.0, d))).filter(|r| r.1 > r.0).collect();
    if regions.is_empty() {
        return lt;
    }
    regions.sort_by(|a, b| a.0.total_cmp(&b.0));
    let lp: f64 = regions.iter().map(|r| r.1 - r.0).sum();
    let free = d - lp;
    let dd = d * s;
    let f = if free > 1e-9 { ((dd - lp) / free).max(0.0) } else { 1.0 };
    let u = (t - l.start_time).clamp(0.0, dd);
    let mut acc = 0.0;
    let mut cur = 0.0;
    for (a, b) in regions.iter().copied().chain(std::iter::once((d, d))) {
        let seg = (a - cur) * f;
        if u <= acc + seg {
            return cur + if f > 1e-9 { (u - acc) / f } else { 0.0 };
        }
        acc += seg;
        if u <= acc + (b - a) {
            return a + (u - acc);
        }
        acc += b - a;
        cur = b;
    }
    d
}

pub fn content_bounds(ev: &Eval, l: &Layer) -> Option<[f64; 4]> {
    let comp = ev.comp;
    match l.kind {
        LayerKind::Camera | LayerKind::Light => None,
        LayerKind::Model => super::mesh::model_bounds(ev, l).map(|b| [b[0], b[1], b[3], b[4]]),
        LayerKind::Null => Some([0.0, 0.0, 100.0, 100.0]),
        LayerKind::Shape => vector::ops_bounds(&shapes::shape_ops(ev, l)).or(Some([-1.0, -1.0, 1.0, 1.0])),
        LayerKind::Text => {
            let doc = l.text_prop().map(|p| ev.value(l, p)).and_then(|v| v.as_text().cloned())?;
            let lay = text::layout(&doc);
            let mut b = None;
            for g in &lay.glyphs {
                for p in &g.paths {
                    b = union_bounds(b, p.bounds());
                }
            }
            b.or(Some([0.0, -doc.size * 0.8, doc.size * 0.2, 0.0]))
        }
        _ => {
            let (w, h) = ev.project.source_size(l, comp);
            Some([0.0, 0.0, w, h])
        }
    }
}

pub fn has_effect(l: &Layer, id: &str) -> bool {
    l.effects().is_some_and(|g| g.children.iter().any(|c| matches!(c, Node::Group(e) if matches!(&e.kind, GroupKind::Effect { effect } if effect == id))))
}

pub fn used_mattes(comp: &Comp) -> HashSet<u64> {
    comp.layers.iter().filter_map(|l| l.track_matte.map(|m| m.layer)).collect()
}
