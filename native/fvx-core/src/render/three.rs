use std::sync::Arc;

use rayon::prelude::*;

use super::comp::{Renderer, content_bounds, fx_frame};
use super::image::{Img, Tile};
use super::light::{self, Caster, EnvMap, Scene, Surface};
use super::mesh::{self, Frag, Proj};
use crate::eval::{Camera, Eval};
use crate::geom::Mat3;
use crate::model::{Comp, Layer};

fn casts(ev: &Eval, l: &Layer) -> (u32, f32) {
    let g = l.props.sub("materialOptions");
    let c = g.and_then(|g| g.get("castsShadows")).map(|p| ev.value(l, p).index()).unwrap_or(0);
    let t = g.and_then(|g| g.get("lightTransmission")).map(|p| ev.value(l, p).num()).unwrap_or(0.0);
    (c, (t / 100.0) as f32)
}

pub fn shadow_only(ev: &Eval, l: &Layer) -> bool {
    l.is_3d() && casts(ev, l).0 == 2
}

impl Renderer<'_> {
    fn flat_buffer(&self, ev: &Eval, l: &Layer, depth: u32, max_side: f64) -> Option<(Img, [f64; 4], f64)> {
        let (sw, sh) = self.project.source_size(l, ev.comp);
        let frame = fx_frame(l, sw, sh);
        let rect = match l.kind {
            crate::model::LayerKind::Shape | crate::model::LayerKind::Text => content_bounds(ev, l).unwrap_or(frame),
            _ => frame,
        };
        let span = (rect[2] - rect[0]).max(rect[3] - rect[1]).max(1.0);
        let r = (max_side / span).min(1.0);
        let (bw, bh) = (((rect[2] - rect[0]) * r).ceil().max(1.0) as usize, ((rect[3] - rect[1]) * r).ceil().max(1.0) as usize);
        let img = self.layer_buffer(ev, l, r, depth, rect, r, bw, bh, Some(usize::MAX))?;
        Some((img, rect, r))
    }

    pub(super) fn build_scene(&self, ev: &Eval, comp: &Comp, cam: &Camera, depth: u32) -> Option<Scene> {
        let lights = light::lights_of(ev);
        let models = comp.layers.iter().any(|l| mesh::as_mesh(ev, l) && l.switches.video && l.is_active_at(ev.t));
        let env_layer = comp.layers.iter().find(|l| l.environment);
        if lights.is_empty() && !models && env_layer.is_none() {
            return None;
        }
        let env = lights.iter().find(|l| l.kind == light::ENVIRONMENT).map(|el| {
            let src = el.source.and_then(|id| comp.layer(id)).or(env_layer);
            match src.and_then(|s| self.flat_buffer(ev, s, depth + 1, 1024.0)) {
                Some((img, _, _)) => Arc::new(EnvMap::from_img(&img, [1.0; 3], el.env_rot)),
                None => Arc::new(EnvMap::uniform([1.0; 3])),
            }
        });
        let env = env.or_else(|| env_layer.and_then(|s| self.flat_buffer(ev, s, depth + 1, 1024.0)).map(|(img, _, _)| Arc::new(EnvMap::from_img(&img, [1.0; 3], 0.0))));
        let any_shadows = lights.iter().any(|l| l.shadows);
        let mut casters = vec![];
        let mut caster_tris = vec![];
        if any_shadows {
            for l in comp.layers.iter().filter(|l| l.is_3d() && !l.is_camera() && !l.is_light() && l.switches.video && l.is_active_at(ev.t)) {
                let (c, trans) = casts(ev, l);
                if c == 0 {
                    continue;
                }
                if mesh::as_mesh(ev, l) {
                    if let Some(m) = mesh::layer_mesh(ev, l, self.media) {
                        caster_tris.extend(m.tris);
                    }
                    continue;
                }
                let Some((img, rect, r)) = self.flat_buffer(ev, l, depth + 1, 1024.0) else { continue };
                let Some(inv) = ev.world_matrix(l).inverse() else { continue };
                casters.push(Caster { id: l.id, inv, img, rect: [rect[0], rect[1]], r, transmission: trans });
            }
        }
        let bounds = if caster_tris.is_empty() {
            None
        } else {
            let mut lo = [f64::MAX; 3];
            let mut hi = [f64::MIN; 3];
            for t in &caster_tris {
                for v in &t.v {
                    for i in 0..3 {
                        lo[i] = lo[i].min(v[i]);
                        hi[i] = hi[i].max(v[i]);
                    }
                }
            }
            let c = light::scale(light::add(lo, hi), 0.5);
            Some((c, light::len(light::sub(hi, lo)) * 0.5))
        };
        let maps = light::shadow_maps(&lights, &caster_tris, bounds);
        Some(Scene { lights, casters, maps, env, eye: cam.eye, ortho_fwd: cam.ortho.map(|_| cam.forward) })
    }

    pub(super) fn plane_light(&self, ev: &Eval, l: &Layer, cam: &Camera, s: f64, scene: &Scene, tile: &mut Tile) {
        if scene.lights.is_empty() {
            return;
        }
        let (surf, accepts, _, _) = Surface::of(ev, l);
        if !accepts {
            return;
        }
        let m = Mat3::scale(s, s) * ev.layer_to_screen(l, cam);
        let Some(inv) = m.inverse() else { return };
        let w = ev.world_matrix(l);
        let nm = w.inverse().map(|i| i.transpose()).unwrap_or(w);
        let n = light::norm(nm.apply_dir([0.0, 0.0, -1.0]));
        let (x0, y0, tw) = (tile.x0, tile.y0, tile.img.w);
        tile.img.px.par_chunks_mut(tw * 4).enumerate().for_each(|(y, row)| {
            for x in 0..tw {
                let px = &mut row[x * 4..x * 4 + 4];
                let a = px[3];
                if a <= 1e-5 {
                    continue;
                }
                let lp = inv.apply((x0 + x as i64) as f64 + 0.5, (y0 + y as i64) as f64 + 0.5);
                let p = w.apply([lp[0], lp[1], 0.0]);
                let base = [px[0] / a, px[1] / a, px[2] / a];
                let (c, only) = scene.shade_classic(p, n, base, &surf, l.id);
                if surf.shadows == 2 {
                    px[0] = 0.0;
                    px[1] = 0.0;
                    px[2] = 0.0;
                    px[3] = a * only;
                } else {
                    px[0] = c[0] * a;
                    px[1] = c[1] * a;
                    px[2] = c[2] * a;
                }
            }
        });
    }

    pub(super) fn plane_transparency(&self, ev: &Eval, l: &Layer, cam: &Camera, s: f64, tile: &mut Tile) {
        let (surf, _, _, _) = Surface::of(ev, l);
        if surf.transparency <= 0.0 {
            return;
        }
        let w = ev.world_matrix(l);
        let nm = w.inverse().map(|i| i.transpose()).unwrap_or(w);
        let n = light::norm(nm.apply_dir([0.0, 0.0, -1.0]));
        if surf.trans_rolloff <= 0.0 {
            tile.img.scale_all(1.0 - surf.transparency);
            return;
        }
        let m = Mat3::scale(s, s) * ev.layer_to_screen(l, cam);
        let Some(inv) = m.inverse() else { return };
        let (x0, y0, tw) = (tile.x0, tile.y0, tile.img.w);
        tile.img.px.par_chunks_mut(tw * 4).enumerate().for_each(|(y, row)| {
            for x in 0..tw {
                let px = &mut row[x * 4..x * 4 + 4];
                if px[3] <= 1e-5 {
                    continue;
                }
                let lp = inv.apply((x0 + x as i64) as f64 + 0.5, (y0 + y as i64) as f64 + 0.5);
                let p = w.apply([lp[0], lp[1], 0.0]);
                let v = match cam.ortho {
                    Some(_) => light::scale(cam.forward, -1.0),
                    None => light::norm(light::sub(cam.eye, p)),
                };
                let k = 1.0 - surf.transparency_at(light::dot(n, v).abs() as f32);
                for c in px.iter_mut() {
                    *c *= k;
                }
            }
        });
    }

    pub(super) fn plane_depths(&self, ev: &Eval, l: &Layer, cam: &Camera, s: f64, tile: &Tile) -> Vec<f32> {
        let m = Mat3::scale(s, s) * ev.layer_to_screen(l, cam);
        let inv = m.inverse();
        let w = ev.world_matrix(l);
        let tw = tile.img.w;
        (0..tile.img.w * tile.img.h)
            .into_par_iter()
            .map(|i| {
                let Some(inv) = inv else { return f32::INFINITY };
                let (x, y) = ((i % tw) as i64 + tile.x0, (i / tw) as i64 + tile.y0);
                let lp = inv.apply(x as f64 + 0.5, y as f64 + 0.5);
                cam.depth(w.apply([lp[0], lp[1], 0.0])) as f32
            })
            .collect()
    }

    pub(super) fn model_tile(&self, ev: &Eval, l: &Layer, s: f64, cam: &Camera, scene: Option<&Scene>) -> Option<(Tile, Vec<f32>, Option<Vec<[f32; 2]>>)> {
        let lm = mesh::layer_mesh(ev, l, self.media)?;
        let (cw, ch) = self.comp_size(ev.comp, s);
        let proj = Proj::of(cam, ev.comp.width as f64, ev.comp.height as f64, s);
        let fallback;
        let scene = match scene {
            Some(sc) => sc,
            None => {
                fallback = Scene { lights: vec![], casters: vec![], maps: vec![], env: None, eye: cam.eye, ortho_fwd: cam.ortho.map(|_| cam.forward) };
                &fallback
            }
        };
        let (surf, accepts, _, _) = Surface::of(ev, l);
        let mats = &lm.mats;
        let refracts = surf.transparency > 0.0 && surf.ior > 1.001;
        let thick = super::extrude::thickness(ev, l).unwrap_or(100.0);
        let view = proj.view;
        let (zoom, ortho, k) = (proj.zoom, proj.ortho, proj.k);
        let shade = |f: &Frag| -> ([f32; 4], [f32; 2]) {
            let Some(m) = mats.get(f.mat as usize) else { return ([0.0; 4], [0.0; 2]) };
            let c = m.color(f.uv);
            let n = light::norm([f.n[0] as f64, f.n[1] as f64, f.n[2] as f64]);
            let v = scene.view_dir(f.p);
            let n = if light::dot(n, v) < 0.0 { light::scale(n, -1.0) } else { n };
            let nv = light::dot(n, v) as f32;
            let mut rgb = if accepts { scene.shade_pbr(f.p, n, [c[0], c[1], c[2]], m.metallic, m.roughness, l.id, surf.shadows > 0) } else { [c[0], c[1], c[2]] };
            let refl = surf.reflection_at(nv);
            if refl > 0.0
                && let Some(env) = scene.env.as_ref()
            {
                let r = light::sub(light::scale(n, 2.0 * light::dot(n, v)), v);
                let e = env.sample(r, 1.0 - surf.sharpness);
                for i in 0..3 {
                    rgb[i] += (e[i] - rgb[i]) * refl.min(1.0);
                }
            }
            let mut a = c[3];
            let mut bend = [0.0f32; 2];
            if surf.transparency > 0.0 {
                a *= 1.0 - surf.transparency_at(nv);
                if refracts {
                    let nvw = view.apply_dir(n);
                    let px = match ortho {
                        Some(o) => o * k,
                        None => zoom / f.z.max(1.0) * k,
                    };
                    let amt = (1.0 - 1.0 / surf.ior as f64) * thick * px;
                    bend = [(-nvw[0] * amt) as f32, (-nvw[1] * amt) as f32];
                }
            }
            ([(rgb[0] + m.emissive[0]) * a, (rgb[1] + m.emissive[1]) * a, (rgb[2] + m.emissive[2]) * a, a], bend)
        };
        let ss = if self.opts.draft || cw * ch > 2_500_000 { 1 } else { 2 };
        let (tile, depth, bend) = mesh::rasterize(&lm.tris, &proj, cw, ch, ss, &shade)?;
        Some((tile, depth, refracts.then_some(bend)))
    }

    pub(super) fn sky_tile(&self, ev: &Eval, s: f64, cam: &Camera, scene: Option<&Scene>, light_id: u64) -> Option<Tile> {
        let env = scene.and_then(|sc| sc.env.clone())?;
        let tint = scene.and_then(|sc| sc.lights.iter().find(|l| l.id == light_id).map(|l| l.color)).unwrap_or([1.0; 3]);
        let (cw, ch) = self.comp_size(ev.comp, s);
        let (w, h) = (ev.comp.width as f64, ev.comp.height as f64);
        let mut img = Img::new(cw, ch);
        img.px.par_chunks_mut(cw * 4).enumerate().for_each(|(y, row)| {
            for x in 0..cw {
                let (_, d) = cam.ray(w, h, (x as f64 + 0.5) / s, (y as f64 + 0.5) / s);
                let c = env.sample(d, 0.0);
                row[x * 4..x * 4 + 4].copy_from_slice(&[c[0] * tint[0], c[1] * tint[1], c[2] * tint[2], 1.0]);
            }
        });
        Some(Tile::full(img))
    }

    pub(super) fn dof(&self, cam: &Camera, s: f64, tile: &mut Tile, depth: &[f32], canvas: (usize, usize)) {
        let Some([focus, aperture, level]) = cam.dof else { return };
        let k = aperture * level / 100.0 * s;
        if k <= 0.01 {
            return;
        }
        let coc = |z: f32| -> f32 {
            if !z.is_finite() || z <= 1.0 {
                return 0.0;
            }
            (k * ((z as f64) - focus).abs() / z as f64) as f32
        };
        let finite: Vec<f32> = depth.iter().copied().filter(|z| z.is_finite()).collect();
        if finite.is_empty() {
            return;
        }
        let mean = finite.iter().sum::<f32>() / finite.len() as f32;
        let cmax = finite.iter().map(|z| coc(*z)).fold(0.0f32, f32::max).min(200.0);
        if cmax < 0.5 {
            return;
        }
        let m = cmax.ceil() as i64 + 2;
        let nx0 = (tile.x0 - m).max(0);
        let ny0 = (tile.y0 - m).max(0);
        let nx1 = (tile.x0 + tile.img.w as i64 + m).min(canvas.0 as i64);
        let ny1 = (tile.y0 + tile.img.h as i64 + m).min(canvas.1 as i64);
        let (nw, nh) = ((nx1 - nx0).max(1) as usize, (ny1 - ny0).max(1) as usize);
        let mut big = Img::new(nw, nh);
        let mut cmap = vec![coc(mean); nw * nh];
        for y in 0..tile.img.h {
            for x in 0..tile.img.w {
                let (bx, by) = (x as i64 + tile.x0 - nx0, y as i64 + tile.y0 - ny0);
                if bx < 0 || by < 0 || bx >= nw as i64 || by >= nh as i64 {
                    continue;
                }
                let si = (y * tile.img.w + x) * 4;
                let di = (by as usize * nw + bx as usize) * 4;
                big.px[di..di + 4].copy_from_slice(&tile.img.px[si..si + 4]);
                let z = depth[y * tile.img.w + x];
                if z.is_finite() {
                    cmap[by as usize * nw + bx as usize] = coc(z);
                }
            }
        }
        let mut grown = cmap.clone();
        crate::effects::util::blur_scalar(&mut grown, nw, nh, (cmax * 0.5) as f64, (cmax * 0.5) as f64);
        let steps = [0.0f32, cmax / 3.0, cmax * 2.0 / 3.0, cmax];
        let levels: Vec<Img> = steps
            .iter()
            .map(|r| {
                let mut b = big.clone();
                if *r > 0.3 {
                    crate::effects::util::gaussian(&mut b, (*r * 0.5) as f64, (*r * 0.5) as f64, false);
                }
                b
            })
            .collect();
        let mut out = Img::new(nw, nh);
        out.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
            let c = cmap[i].max(grown[i]);
            let f = (c / cmax * 3.0).clamp(0.0, 3.0);
            let a = (f.floor() as usize).min(2);
            let t = f - a as f32;
            for k in 0..4 {
                let x = levels[a].px[i * 4 + k];
                let y = levels[a + 1].px[i * 4 + k];
                p[k] = x + (y - x) * t;
            }
        });
        *tile = Tile { x0: nx0, y0: ny0, img: out };
    }
}

pub type DepthFrag = (Tile, Vec<f32>, f32, Option<Vec<[f32; 2]>>);

fn frag_at(f: &DepthFrag, x: i64, y: i64) -> Option<(f32, [f32; 4], [f32; 2])> {
    let (t, d, op, bend) = f;
    let (tx, ty) = (x - t.x0, y - t.y0);
    if tx < 0 || ty < 0 || tx >= t.img.w as i64 || ty >= t.img.h as i64 {
        return None;
    }
    let i = ty as usize * t.img.w + tx as usize;
    let a = t.img.px[i * 4 + 3] * op;
    if a <= 1e-6 {
        return None;
    }
    let p = &t.img.px[i * 4..i * 4 + 4];
    let b = bend.as_ref().map(|b| b[i]).unwrap_or([0.0; 2]);
    Some((d[i], [p[0] * op, p[1] * op, p[2] * op, a], b))
}

fn behind(orig: &Img, frags: &[DepthFrag], x: i64, y: i64, z: f32) -> [f32; 4] {
    let x = x.clamp(0, orig.w as i64 - 1);
    let y = y.clamp(0, orig.h as i64 - 1);
    let i = (y as usize * orig.w + x as usize) * 4;
    let mut px = [orig.px[i], orig.px[i + 1], orig.px[i + 2], orig.px[i + 3]];
    let mut st: Vec<(f32, [f32; 4])> = frags.iter().filter_map(|f| frag_at(f, x, y)).filter(|e| e.0 > z).map(|e| (e.0, e.1)).collect();
    st.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, c) in &st {
        let k = 1.0 - c[3];
        for j in 0..4 {
            px[j] = c[j] + px[j] * k;
        }
    }
    px
}

pub fn composite_depth(canvas: &mut Img, frags: &[DepthFrag]) {
    let w = canvas.w;
    let orig = frags.iter().any(|f| f.3.is_some()).then(|| canvas.clone());
    canvas.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        let mut stack: Vec<(f32, [f32; 4], [f32; 2])> = Vec::with_capacity(frags.len());
        for x in 0..w {
            stack.clear();
            stack.extend(frags.iter().filter_map(|f| frag_at(f, x as i64, y as i64)));
            if stack.is_empty() {
                continue;
            }
            stack.sort_by(|a, b| b.0.total_cmp(&a.0));
            let px = &mut row[x * 4..x * 4 + 4];
            for (z, c, b) in &stack {
                if let Some(o) = orig.as_ref()
                    && c[3] < 0.999
                    && (b[0].abs() > 0.05 || b[1].abs() > 0.05)
                {
                    let (fx, fy) = (x as f32 + b[0], y as f32 + b[1]);
                    let (x0, y0) = (fx.floor(), fy.floor());
                    let (tx, ty) = (fx - x0, fy - y0);
                    let mut acc = [0.0f32; 4];
                    for (dx, dy, wt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
                        if wt <= 0.0 {
                            continue;
                        }
                        let s = behind(o, frags, x0 as i64 + dx, y0 as i64 + dy, *z);
                        for j in 0..4 {
                            acc[j] += s[j] * wt;
                        }
                    }
                    px.copy_from_slice(&acc);
                }
                let k = 1.0 - c[3];
                for i in 0..4 {
                    px[i] = c[i] + px[i] * k;
                }
            }
        }
    });
}
