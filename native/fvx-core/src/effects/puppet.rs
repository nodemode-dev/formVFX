use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

use super::{EffectSpec, FxCtx, check, no_pad};
use crate::model::prop::{Group, GroupKind, Ids, Node, Ui};
use crate::model::Value;
use crate::render::image::Img;
use crate::render::puppet::{self, KINDS, Mesh, Pin};

pub const ID: &str = "fx.distort.puppet";

pub fn specs() -> Vec<EffectSpec> {
    vec![EffectSpec { id: ID, name: "Puppet", category: "Distort", params: vec![check("onTransparent", "On Transparent", false)], render, pad }]
}

fn pad(c: &FxCtx) -> f64 {
    let meshes = read(c.p);
    let reach = meshes.iter().flat_map(|m| m.pins.iter()).map(|(_, p)| (p.pos[0] - p.rest[0]).hypot(p.pos[1] - p.rest[1])).fold(0.0f64, f64::max);
    let bend = meshes.iter().flat_map(|m| m.pins.iter()).any(|(_, p)| p.kind == 1 || p.kind == 2);
    if reach <= 0.0 && !bend {
        return no_pad(c);
    }
    reach * 1.5 + if bend { 200.0 } else { 20.0 }
}

pub fn mesh_group(ids: &mut Ids, gid: &str, name: &str, density: f64, expansion: f64, triangles: f64, mesh_time: f64) -> Group {
    let children = vec![
        Node::Prop(ids.prop("density", "Density", Value::Num(density), Ui::slider(1.0, 100.0, 1.0, 30.0, 1))),
        Node::Prop(ids.prop("expansion", "Expansion", Value::Num(expansion), Ui::slider(-100.0, 1000.0, 0.0, 50.0, 1))),
        Node::Prop(ids.prop("triangles", "Triangles", Value::Num(triangles), Ui::slider(10.0, 10_000.0, 50.0, 1000.0, 0))),
        Node::Prop(ids.prop("meshTime", "Mesh Time", Value::Num(mesh_time), Ui::Hidden)),
        Node::Prop(ids.prop("seed", "Seed", Value::Vec2([0.0; 2]), Ui::Hidden)),
        Node::Group(ids.group("deform", "Deform", GroupKind::Plain, vec![])),
    ];
    ids.group(gid, name, GroupKind::Plain, children)
}

pub fn pin_group(ids: &mut Ids, gid: &str, name: &str, kind: usize, rest: [f64; 2], pos: [f64; 2]) -> Group {
    let mut children = vec![
        Node::Prop(ids.prop("kind", "Kind", Value::Str(KINDS[kind.min(4)].into()), Ui::Hidden)),
        Node::Prop(ids.prop("rest", "Rest Position", Value::Vec2(rest), Ui::Hidden)),
    ];
    if kind != 2 && kind != 3 {
        children.push(Node::Prop(ids.prop("position", "Position", Value::Vec2(pos), Ui::Point).spatial()));
    }
    if kind == 1 || kind == 2 {
        children.push(Node::Prop(ids.prop("scale", "Scale", Value::Num(100.0), Ui::slider(0.0, 10_000.0, 0.0, 400.0, 1))));
        children.push(Node::Prop(ids.prop("rotation", "Rotation", Value::Num(0.0), Ui::Angle)));
    }
    if kind == 3 {
        children.push(Node::Prop(ids.prop("amount", "Amount", Value::Num(10.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 1))));
    }
    if kind == 3 || kind == 4 {
        children.push(Node::Prop(ids.prop("extent", "Extent", Value::Num(15.0), Ui::slider(0.0, 10_000.0, 0.0, 200.0, 1))));
    }
    if kind == 4 {
        children.push(Node::Prop(ids.prop("inFront", "In Front", Value::Num(50.0), Ui::slider(-100.0, 100.0, -100.0, 100.0, 1))));
    }
    ids.group(gid, name, GroupKind::Plain, children)
}

pub struct MeshParams {
    pub id: String,
    pub density: f64,
    pub expansion: f64,
    pub triangles: f64,
    pub mesh_time: f64,
    pub seed: [f64; 2],
    pub pins: Vec<(String, Pin)>,
}

pub fn read(p: &HashMap<String, Value>) -> Vec<MeshParams> {
    let mut ids: Vec<&str> = p.keys().filter_map(|k| k.strip_suffix("/meshTime")).filter(|k| !k.contains('/')).collect();
    ids.sort();
    let num = |k: String, d: f64| p.get(&k).map(|v| v.num()).unwrap_or(d);
    ids.into_iter()
        .map(|m| {
            let mut pins: Vec<&str> = p.keys().filter_map(|k| k.strip_prefix(&format!("{m}/deform/")).and_then(|r| r.strip_suffix("/kind"))).collect();
            pins.sort();
            let pins = pins
                .into_iter()
                .map(|pid| {
                    let g = |k: &str| format!("{m}/deform/{pid}/{k}");
                    let kind = match p.get(&g("kind")) {
                        Some(Value::Str(s)) => KINDS.iter().position(|k| k == s).unwrap_or(0),
                        _ => 0,
                    };
                    let rest = p.get(&g("rest")).map(|v| v.vec2()).unwrap_or([0.0; 2]);
                    let pin = Pin {
                        kind,
                        rest,
                        pos: p.get(&g("position")).map(|v| v.vec2()).unwrap_or(rest),
                        scale: num(g("scale"), 100.0),
                        rotation: num(g("rotation"), 0.0),
                        amount: num(g("amount"), 10.0),
                        extent: num(g("extent"), 15.0),
                        in_front: num(g("inFront"), 50.0),
                    };
                    (pid.to_string(), pin)
                })
                .collect();
            MeshParams {
                id: m.to_string(),
                density: num(format!("{m}/density"), 10.0),
                expansion: num(format!("{m}/expansion"), 3.0),
                triangles: num(format!("{m}/triangles"), 350.0),
                mesh_time: num(format!("{m}/meshTime"), 0.0),
                seed: p.get(&format!("{m}/seed")).map(|v| v.vec2()).unwrap_or([0.0; 2]),
                pins,
            }
        })
        .collect()
}

type MeshCache = Mutex<HashMap<u64, Arc<Mesh>>>;

fn cache() -> &'static MeshCache {
    static C: OnceLock<MeshCache> = OnceLock::new();
    C.get_or_init(Default::default)
}

fn checksum(img: &Img) -> u64 {
    let mut h: u64 = 1469598103934665603;
    let step = (img.px.len() / 4 / 4096).max(1);
    for i in (0..img.px.len() / 4).step_by(step) {
        h ^= (img.px[i * 4 + 3] * 255.0) as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h ^ ((img.w as u64) << 32) ^ img.h as u64
}

pub fn mesh_from(img: &Img, origin: [f64; 2], r: f64, m: &MeshParams) -> Option<Arc<Mesh>> {
    use std::hash::{Hash, Hasher};
    let mut hs = std::collections::hash_map::DefaultHasher::new();
    checksum(img).hash(&mut hs);
    for v in [m.density, m.expansion, m.triangles, origin[0], origin[1], r, m.seed[0], m.seed[1]] {
        v.to_bits().hash(&mut hs);
    }
    let key = hs.finish();
    if let Some(found) = cache().lock().get(&key) {
        return Some(found.clone());
    }
    let (mut bx0, mut by0, mut bx1, mut by1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..img.h {
        for x in 0..img.w {
            if img.px[(y * img.w + x) * 4 + 3] > 0.02 {
                bx0 = bx0.min(x);
                by0 = by0.min(y);
                bx1 = bx1.max(x);
                by1 = by1.max(y);
            }
        }
    }
    if bx0 == usize::MAX {
        return None;
    }
    let to_l = |b: f64| b / r;
    let e = m.expansion.max(0.0) + 2.0;
    let bounds = [to_l(bx0 as f64) + origin[0] - e, to_l(by0 as f64) + origin[1] - e, to_l(bx1 as f64 + 1.0) + origin[0] + e, to_l(by1 as f64 + 1.0) + origin[1] + e];
    let alpha = |x: f64, y: f64| -> f32 { img.sample((x - origin[0]) * r, (y - origin[1]) * r, false)[3] };
    let seed = if m.pins.is_empty() { None } else { Some(m.seed) };
    let mesh = Arc::new(puppet::build(&alpha, bounds, (1.0 / r).max(0.5), m.triangles, m.density, m.expansion, seed)?);
    let mut c = cache().lock();
    if c.len() > 32 {
        c.clear();
    }
    c.insert(key, mesh.clone());
    Some(mesh)
}

fn render(c: &FxCtx, img: &mut Img) {
    let meshes = read(c.p);
    if meshes.is_empty() || meshes.iter().all(|m| m.pins.is_empty()) {
        return;
    }
    let src = img.clone();
    let mut out = Img::new(img.w, img.h);
    for (mi, m) in meshes.iter().enumerate() {
        if m.pins.is_empty() {
            continue;
        }
        let at = if (m.mesh_time - c.t).abs() < 1e-6 { None } else { c.frame(m.mesh_time - c.t).filter(|f| f.w == src.w && f.h == src.h) };
        let base = at.as_ref().unwrap_or(&src);
        let Some(mesh) = mesh_from(base, c.origin, c.r, m) else { continue };
        let pins: Vec<Pin> = m.pins.iter().map(|p| p.1).collect();
        let moved = puppet::deform(&mesh, &pins);
        let from: Vec<[f64; 2]> = mesh.rest.iter().map(|v| [(v[0] - c.origin[0]) * c.r, (v[1] - c.origin[1]) * c.r]).collect();
        let to: Vec<[f64; 2]> = moved.iter().map(|v| [(v[0] - c.origin[0]) * c.r, (v[1] - c.origin[1]) * c.r]).collect();
        let depth: Vec<f64> = puppet::depths(&mesh, &pins).into_iter().map(|d| d + mi as f64 * 1000.0).collect();
        let mut order: Vec<usize> = (0..mesh.tris.len()).collect();
        order.sort_by(|a, b| depth[*a].total_cmp(&depth[*b]));
        let layer = puppet::rasterize(&src, &from, &to, &mesh.tris, &order, &depth);
        for (d, s) in out.px.chunks_exact_mut(4).zip(layer.px.chunks_exact(4)) {
            let k = 1.0 - s[3].min(1.0);
            for ch in 0..4 {
                d[ch] = s[ch] + d[ch] * k;
            }
        }
    }
    *img = out;
}
