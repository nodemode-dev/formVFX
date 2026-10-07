use rayon::prelude::*;

use super::util::{gaussian, lum};
use super::{EffectSpec, FxCtx, check, color, layer_param, no_pad, num, pct, popup, slider, text};
use crate::render::exr::Aux;
use crate::render::image::Img;

pub const CAT: &str = "3D Channel";

const Z: &[&str] = &["Z", "depth.Z", "Depth", "zdepth", "depth.R", "depth.V", "Depth.Z"];
const OBJECT: &[&str] = &["objectId", "ObjectID", "object_id", "id", "IndexOB.X", "IndexOB", "objectId.R", "ID"];
const MATERIAL: &[&str] = &["materialId", "MaterialID", "material_id", "IndexMA.X", "IndexMA", "materialId.R"];
const COVERAGE: &[&str] = &["coverage", "Coverage", "cov"];
const NORMAL: &[&str] = &["N", "Normal", "normals", "normal", "Nn"];
const UV: &[&str] = &["UV", "uv", "texcoord"];

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.3d.channelExtract",
            name: "3D Channel Extract",
            category: CAT,
            params: vec![
                popup("channel", "3D Channel", 0, &["Z-Depth", "Object ID", "Texture UV", "Surface Normals", "Coverage", "Background RGB", "Unclamped RGB", "Material ID"]),
                num("blackPoint", "Black Point", 5000.0, -1.0e6, 1.0e6, 0.0, 10000.0),
                num("whitePoint", "White Point", 0.0, -1.0e6, 1.0e6, 0.0, 10000.0),
                check("antiAlias", "Anti-alias", false),
                check("clamp", "Clamp Output", true),
                check("invert", "Invert Depth Map", false),
            ],
            render: channel_extract,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.3d.depthMatte",
            name: "Depth Matte",
            category: CAT,
            params: vec![num("depth", "Depth", 0.0, -1.0e6, 1.0e6, 0.0, 10000.0), num("feather", "Feather", 0.0, 0.0, 1.0e6, 0.0, 1000.0), check("invert", "Invert", false)],
            render: depth_matte,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.3d.depthOfField",
            name: "Depth of Field",
            category: CAT,
            params: vec![
                num("focalPlane", "Focal Plane", 100.0, -1.0e6, 1.0e6, 0.0, 10000.0),
                num("maxRadius", "Maximum Radius", 0.0, 0.0, 100.0, 0.0, 50.0),
                num("thickness", "Focal Plane Thickness", 0.0, 0.0, 1.0e6, 0.0, 1000.0),
                slider("bias", "Focal Bias", 50.0, 0.0, 100.0, 1),
            ],
            render: depth_of_field,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.3d.extractor",
            name: "EXtractoR",
            category: CAT,
            params: vec![
                text("red", "Red", "R"),
                text("green", "Green", "G"),
                text("blue", "Blue", "B"),
                text("alpha", "Alpha", "A"),
                num("blackPoint", "Black Point", 0.0, -1.0e6, 1.0e6, 0.0, 1.0),
                num("whitePoint", "White Point", 1.0, -1.0e6, 1.0e6, 0.0, 1.0),
                check("unmult", "UnMult", false),
            ],
            render: extractor,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.3d.fog3d",
            name: "Fog 3D",
            category: CAT,
            params: vec![
                color("fogColor", "Fog Color", [0.5, 0.5, 0.5]),
                num("start", "Fog Start Depth", 0.0, -1.0e6, 1.0e6, 0.0, 10000.0),
                num("end", "Fog End Depth", 1000.0, -1.0e6, 1.0e6, 0.0, 10000.0),
                pct("opacity", "Fog Opacity", 100.0),
                slider("density", "Scattering Density", 50.0, 0.0, 100.0, 1),
                check("foggyBackground", "Foggy Background", true),
                layer_param("gradientLayer", "Gradient Layer"),
                pct("contribution", "Layer Contribution", 0.0),
            ],
            render: fog3d,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.3d.idMatte",
            name: "ID Matte",
            category: CAT,
            params: vec![
                popup("auxChannel", "Aux. Channel", 0, &["Object ID", "Material ID"]),
                slider("idSelection", "ID Selection", 0.0, 0.0, 65535.0, 0),
                num("feather", "Feather", 0.0, 0.0, 100.0, 0.0, 10.0),
                check("invert", "Invert", false),
                check("useCoverage", "Use Coverage", false),
            ],
            render: id_matte,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.3d.identifier",
            name: "IDentifier",
            category: CAT,
            params: vec![text("channel", "Channel Object ID", ""), popup("display", "Display", 0, &["Colors", "Luma Matte", "Alpha Matte", "Raw"]), slider("id", "ID", 0.0, 0.0, 65535.0, 0)],
            render: identifier,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.3d.cryptomatte",
            name: "Cryptomatte",
            category: CAT,
            params: vec![
                text("layer", "Layer Selection", ""),
                text("selection", "Matte Selection", ""),
                popup("display", "Display", 0, &["Colors", "Edges", "Matte"]),
                check("matteOnly", "Matte Only", false),
                check("invert", "Invert Selection", false),
            ],
            render: cryptomatte,
            pad: no_pad,
        },
    ]
}

fn src_index(c: &FxCtx, a: &Aux, x: usize, y: usize) -> Option<usize> {
    let sx = ((x as f64 + 0.5) / c.r + c.origin[0]).floor();
    let sy = ((y as f64 + 0.5) / c.r + c.origin[1]).floor();
    (sx >= 0.0 && sy >= 0.0 && (sx as usize) < a.w && (sy as usize) < a.h).then(|| sy as usize * a.w + sx as usize)
}

fn sampled(c: &FxCtx, a: &Aux, plane: &[f32], w: usize, h: usize, missing: f32) -> Vec<f32> {
    let mut out = vec![missing; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            if let Some(i) = src_index(c, a, x, y) {
                *o = plane[i];
            }
        }
    });
    out
}

fn triple<'a>(a: &'a Aux, prefixes: &[&str], comps: &[[&str; 3]]) -> Option<[&'a [f32]; 3]> {
    for p in prefixes {
        for set in comps {
            let names: Vec<String> = set.iter().map(|k| format!("{p}.{k}")).collect();
            if let (Some(x), Some(y), Some(z)) = (a.find(&[names[0].as_str()]), a.find(&[names[1].as_str()]), a.find(&[names[2].as_str()])) {
                return Some([x, y, z]);
            }
        }
    }
    None
}

fn depth(a: &Aux) -> Option<&[f32]> {
    a.find(Z)
}

fn map_points(v: f32, black: f32, white: f32, clamp: bool) -> f32 {
    let d = white - black;
    let t = if d.abs() < 1e-12 { if v >= white { 1.0 } else { 0.0 } } else { (v - black) / d };
    if clamp { t.clamp(0.0, 1.0) } else { t }
}

fn soften(vals: &mut [[f32; 3]], w: usize, h: usize) {
    let src = vals.to_vec();
    vals.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let mut acc = [0.0f32; 3];
            let mut n = 0.0;
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (xx, yy) = (x as i64 + dx, y as i64 + dy);
                    if xx < 0 || yy < 0 || xx >= w as i64 || yy >= h as i64 {
                        continue;
                    }
                    let s = src[yy as usize * w + xx as usize];
                    for k in 0..3 {
                        acc[k] += s[k];
                    }
                    n += 1.0;
                }
            }
            *o = [acc[0] / n, acc[1] / n, acc[2] / n];
        }
    });
}

fn channel_extract(c: &FxCtx, img: &mut Img) {
    let Some(a) = c.host.and_then(|h| h.aux()) else { return };
    let (w, h) = (img.w, img.h);
    let (black, white) = (c.f("blackPoint") as f32, c.f("whitePoint") as f32);
    let clamp = c.b("clamp");
    let invert = c.b("invert");
    let ch = c.e("channel");
    let grey = |p: Option<&[f32]>| -> Option<Vec<[f32; 3]>> {
        let s = sampled(c, &a, p?, w, h, f32::NAN);
        Some(s.into_iter().map(|v| {
            let g = if v.is_finite() { map_points(v, black, white, clamp) } else if white >= black { 1.0 } else { 0.0 };
            let g = if invert { 1.0 - g } else { g };
            [g, g, g]
        }).collect())
    };
    let vec3 = |p: Option<[&[f32]; 3]>, f: &dyn Fn([f32; 3]) -> [f32; 3]| -> Option<Vec<[f32; 3]>> {
        let p = p?;
        let s: Vec<Vec<f32>> = p.iter().map(|q| sampled(c, &a, q, w, h, 0.0)).collect();
        Some((0..w * h).map(|i| f([s[0][i], s[1][i], s[2][i]])).collect())
    };
    let vals = match ch {
        0 => grey(depth(&a)),
        1 => grey(a.find(OBJECT)),
        2 => {
            let uv = triple(&a, UV, &[["U", "V", "W"], ["R", "G", "B"], ["X", "Y", "Z"], ["u", "v", "w"]]).or_else(|| {
                let u = a.find(&["UV.U", "uv.R", "UV.X", "u"])?;
                let v = a.find(&["UV.V", "uv.G", "UV.Y", "v"])?;
                Some([u, v, v])
            });
            vec3(uv, &|v| [v[0], v[1], 0.0])
        }
        3 => vec3(triple(&a, NORMAL, &[["X", "Y", "Z"], ["R", "G", "B"], ["x", "y", "z"]]), &|n| [n[0] * 0.5 + 0.5, n[1] * 0.5 + 0.5, n[2] * 0.5 + 0.5]),
        4 => {
            let p = a.find(COVERAGE);
            p.map(|p| sampled(c, &a, p, w, h, 0.0).into_iter().map(|v| [v, v, v]).collect())
        }
        5 => vec3(triple(&a, &["background", "Background", "bg", "BG"], &[["R", "G", "B"], ["r", "g", "b"]]), &|v| v),
        6 => vec3(a.find(&["R"]).zip(a.find(&["G"])).zip(a.find(&["B"])).map(|((r, g), b)| [r, g, b]), &|v| v),
        _ => grey(a.find(MATERIAL)),
    };
    let Some(mut vals) = vals else { return };
    if c.b("antiAlias") {
        soften(&mut vals, w, h);
    }
    img.px.par_chunks_mut(4).zip(vals.par_iter()).for_each(|(p, v)| {
        let al = p[3];
        p[0] = v[0] * al;
        p[1] = v[1] * al;
        p[2] = v[2] * al;
    });
}

fn depth_matte(c: &FxCtx, img: &mut Img) {
    let Some(a) = c.host.and_then(|h| h.aux()) else { return };
    let Some(zp) = depth(&a) else { return };
    let z = sampled(c, &a, zp, img.w, img.h, f32::INFINITY);
    let (d, f) = (c.f("depth") as f32, c.f("feather") as f32);
    let invert = c.b("invert");
    img.px.par_chunks_mut(4).zip(z.par_iter()).for_each(|(p, &zv)| {
        let keep = if f > 0.0 { ((zv - d) / f + 0.5).clamp(0.0, 1.0) } else if zv >= d { 1.0 } else { 0.0 };
        let keep = if invert { 1.0 - keep } else { keep };
        for v in p.iter_mut() {
            *v *= keep;
        }
    });
}

fn depth_of_field(c: &FxCtx, img: &mut Img) {
    let Some(a) = c.host.and_then(|h| h.aux()) else { return };
    let Some(zp) = depth(&a) else { return };
    let max_r = c.px(c.f("maxRadius")) as f32;
    if max_r < 0.25 {
        return;
    }
    let (w, h) = (img.w, img.h);
    let z = sampled(c, &a, zp, w, h, f32::NAN);
    let (fp, half) = (c.f("focalPlane") as f32, (c.f("thickness") * 0.5) as f32);
    let (lo, hi) = z.iter().filter(|v| v.is_finite()).fold((f32::MAX, f32::MIN), |m, v| (m.0.min(*v), m.1.max(*v)));
    let range = if hi > lo { hi - lo } else { 1.0 };
    let bias = (c.f("bias") as f32 / 100.0).clamp(0.0, 1.0);
    let fall = (range * 0.5 * (1.0 - bias)).max(range * 1e-3);
    let rad: Vec<f32> = z
        .iter()
        .map(|v| {
            let v = if v.is_finite() { *v } else { hi };
            let d = ((v - fp).abs() - half).max(0.0);
            max_r * d / (d + fall)
        })
        .collect();
    let levels = 5usize;
    let mut stack = vec![img.clone()];
    for i in 1..levels {
        let r = max_r * i as f32 / (levels - 1) as f32;
        let mut b = img.clone();
        gaussian(&mut b, (r * 0.5) as f64, (r * 0.5) as f64, true);
        stack.push(b);
    }
    let step = max_r / (levels - 1) as f32;
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let x = (rad[i] / step).clamp(0.0, (levels - 1) as f32);
        let k = (x.floor() as usize).min(levels - 2);
        let t = x - k as f32;
        for j in 0..4 {
            p[j] = stack[k].px[i * 4 + j] * (1.0 - t) + stack[k + 1].px[i * 4 + j] * t;
        }
    });
}

fn extractor(c: &FxCtx, img: &mut Img) {
    let Some(a) = c.host.and_then(|h| h.aux()) else { return };
    let (w, h) = (img.w, img.h);
    let (black, white) = (c.f("blackPoint") as f32, c.f("whitePoint") as f32);
    let get = |id: &str, fallback: f32| -> Vec<f32> {
        match a.plane(&c.s(id)) {
            Some(p) => sampled(c, &a, p, w, h, 0.0).into_iter().map(|v| map_points(v, black, white, false)).collect(),
            None => vec![fallback; w * h],
        }
    };
    let (r, g, b) = (get("red", 0.0), get("green", 0.0), get("blue", 0.0));
    let al = match a.plane(&c.s("alpha")) {
        Some(p) => sampled(c, &a, p, w, h, 0.0),
        None => vec![1.0; w * h],
    };
    let unmult = c.b("unmult");
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let al = al[i].clamp(0.0, 1.0);
        let k = if unmult { 1.0 } else { al };
        p[0] = r[i] * k;
        p[1] = g[i] * k;
        p[2] = b[i] * k;
        p[3] = al;
    });
}

fn fog3d(c: &FxCtx, img: &mut Img) {
    let Some(a) = c.host.and_then(|h| h.aux()) else { return };
    let Some(zp) = depth(&a) else { return };
    let (w, h) = (img.w, img.h);
    let z = sampled(c, &a, zp, w, h, f32::INFINITY);
    let (s, e) = (c.f("start") as f32, c.f("end") as f32);
    let op = (c.f("opacity") / 100.0) as f32;
    let k = 1.0 + c.f("density") as f32 / 25.0;
    let fog = c.c("fogColor");
    let bg = c.b("foggyBackground");
    let contrib = (c.f("contribution") / 100.0) as f32;
    let grad = if contrib > 0.0 { c.other_fit("gradientLayer", true, w, h, 1) } else { None };
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let t = if !z[i].is_finite() { 1.0 } else if e > s { ((z[i] - s) / (e - s)).clamp(0.0, 1.0) } else if z[i] >= s { 1.0 } else { 0.0 };
        let mut amt = (1.0 - (1.0 - t).powf(k)) * op;
        if let Some(g) = &grad {
            let q = &g.px[i * 4..i * 4 + 4];
            let gl = if q[3] > 0.0 { lum(q[0] / q[3], q[1] / q[3], q[2] / q[3]) } else { 0.0 };
            amt *= 1.0 - contrib + contrib * gl;
        }
        let al = p[3];
        for j in 0..3 {
            p[j] = p[j] * (1.0 - amt) + fog[j] * amt * al;
        }
        if bg && al < 1.0 {
            let b = op * (1.0 - al);
            for j in 0..3 {
                p[j] += fog[j] * b;
            }
            p[3] = al + b;
        }
    });
}

fn feathered(mut m: Vec<f32>, w: usize, h: usize, px: f64) -> Vec<f32> {
    if px > 0.25 {
        super::util::blur_scalar(&mut m, w, h, px * 0.5, px * 0.5);
    }
    m
}

fn id_matte(c: &FxCtx, img: &mut Img) {
    let Some(a) = c.host.and_then(|h| h.aux()) else { return };
    let Some(ids) = (if c.e("auxChannel") == 1 { a.find(MATERIAL) } else { a.find(OBJECT) }) else { return };
    let (w, h) = (img.w, img.h);
    let ids = sampled(c, &a, ids, w, h, -1.0);
    let cov = if c.b("useCoverage") { a.find(COVERAGE).map(|p| sampled(c, &a, p, w, h, 1.0)) } else { None };
    let want = c.f("idSelection").round() as f32;
    let m: Vec<f32> = ids.iter().enumerate().map(|(i, v)| if (v.round() - want).abs() < 0.5 { cov.as_ref().map(|cv| cv[i].clamp(0.0, 1.0)).unwrap_or(1.0) } else { 0.0 }).collect();
    let m = feathered(m, w, h, c.px(c.f("feather")));
    let invert = c.b("invert");
    img.px.par_chunks_mut(4).zip(m.par_iter()).for_each(|(p, &k)| {
        let k = if invert { 1.0 - k } else { k };
        for v in p.iter_mut() {
            *v *= k;
        }
    });
}

fn hue_color(h: u32) -> [f32; 3] {
    let hue = (h % 360) as f32 / 360.0;
    let sat = 0.55 + ((h >> 9) % 40) as f32 / 100.0;
    let lig = 0.45 + ((h >> 15) % 20) as f32 / 100.0;
    super::util::hsl_to_rgb([hue, sat, lig])
}

fn id_hash(v: f32) -> u32 {
    let mut x = v.to_bits() ^ 0x9e37_79b9;
    x ^= x >> 16;
    x = x.wrapping_mul(0x85eb_ca6b);
    x ^= x >> 13;
    x = x.wrapping_mul(0xc2b2_ae35);
    x ^ (x >> 16)
}

fn identifier(c: &FxCtx, img: &mut Img) {
    let Some(a) = c.host.and_then(|h| h.aux()) else { return };
    let name = c.s("channel");
    let Some(p) = (if name.trim().is_empty() { a.find(OBJECT) } else { a.plane(&name) }) else { return };
    let ids = sampled(c, &a, p, img.w, img.h, f32::NAN);
    let want = c.f("id").round() as f32;
    let maxid = ids.iter().filter(|v| v.is_finite()).fold(1.0f32, |m, v| m.max(*v));
    let display = c.e("display");
    img.px.par_chunks_mut(4).zip(ids.par_iter()).for_each(|(p, &v)| {
        let hit = v.is_finite() && (v.round() - want).abs() < 0.5;
        match display {
            0 => {
                let col = if v.is_finite() { hue_color(id_hash(v.round())) } else { [0.0; 3] };
                let k = if hit { 1.0 } else { 0.5 };
                p[0] = col[0] * k * p[3];
                p[1] = col[1] * k * p[3];
                p[2] = col[2] * k * p[3];
            }
            1 => {
                let g = if hit { 1.0 } else { 0.0 };
                p[0] = g;
                p[1] = g;
                p[2] = g;
                p[3] = 1.0;
            }
            2 => {
                if !hit {
                    p.iter_mut().for_each(|x| *x = 0.0);
                }
            }
            _ => {
                let g = if v.is_finite() { v / maxid } else { 0.0 };
                p[0] = g;
                p[1] = g;
                p[2] = g;
                p[3] = 1.0;
            }
        }
    });
}

pub fn murmur3(data: &[u8], seed: u32) -> u32 {
    let (c1, c2) = (0xcc9e_2d51u32, 0x1b87_3593u32);
    let mut h = seed;
    let chunks = data.chunks_exact(4);
    let tail = chunks.remainder();
    for ch in chunks {
        let mut k = u32::from_le_bytes([ch[0], ch[1], ch[2], ch[3]]);
        k = k.wrapping_mul(c1).rotate_left(15).wrapping_mul(c2);
        h ^= k;
        h = h.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
    }
    let mut k = 0u32;
    for (i, b) in tail.iter().enumerate() {
        k ^= (*b as u32) << (8 * i);
    }
    if !tail.is_empty() {
        k = k.wrapping_mul(c1).rotate_left(15).wrapping_mul(c2);
        h ^= k;
    }
    h ^= data.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}

pub fn crypto_id(name: &str) -> f32 {
    let mut h = murmur3(name.as_bytes(), 0);
    let exp = (h >> 23) & 0xff;
    if exp == 0 || exp == 255 {
        h ^= 1 << 23;
    }
    f32::from_bits(h)
}

pub struct CryptoLayer {
    pub name: String,
    pub ranks: Vec<(String, String)>,
    pub manifest: Vec<(String, f32)>,
}

pub fn crypto_layers(a: &Aux) -> Vec<CryptoLayer> {
    let mut out = vec![];
    for (k, v) in &a.attrs {
        let Some(rest) = k.strip_prefix("cryptomatte/") else { continue };
        let Some(key) = rest.strip_suffix("/name") else { continue };
        let name = v.clone();
        let mut ranks = vec![];
        for i in 0..32 {
            let base = format!("{name}{i:02}");
            match (a.plane(&format!("{base}.R")), a.plane(&format!("{base}.B"))) {
                (Some(_), Some(_)) => {
                    ranks.push((format!("{base}.R"), format!("{base}.G")));
                    ranks.push((format!("{base}.B"), format!("{base}.A")));
                }
                (Some(_), None) => ranks.push((format!("{base}.R"), format!("{base}.G"))),
                _ => break,
            }
        }
        let manifest = a
            .attr(&format!("cryptomatte/{key}/manifest"))
            .and_then(|m| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(m).ok())
            .map(|m| m.into_iter().filter_map(|(n, h)| Some((n, f32::from_bits(u32::from_str_radix(h.as_str()?.trim_start_matches("0x"), 16).ok()?)))).collect())
            .unwrap_or_default();
        out.push(CryptoLayer { name, ranks, manifest });
    }
    out.sort_by(|x, y| x.name.cmp(&y.name));
    out
}

fn selection_ids(sel: &str, layer: &CryptoLayer) -> Vec<f32> {
    let mut ids = vec![];
    for raw in split_names(sel) {
        if let Some(hex) = raw.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
            if let Ok(b) = u32::from_str_radix(hex.trim_start_matches("0x"), 16) {
                ids.push(f32::from_bits(b));
            }
            continue;
        }
        ids.push(layer.manifest.iter().find(|m| m.0 == raw).map(|m| m.1).unwrap_or_else(|| crypto_id(&raw)));
    }
    ids
}

pub fn split_names(sel: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    for ch in sel.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                if !cur.trim().is_empty() {
                    out.push(cur.trim().to_string());
                }
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn cryptomatte(c: &FxCtx, img: &mut Img) {
    let Some(a) = c.host.and_then(|h| h.aux()) else { return };
    let layers = crypto_layers(&a);
    let want = c.s("layer");
    let Some(layer) = layers.iter().find(|l| !want.trim().is_empty() && l.name == want.trim()).or(layers.first()) else { return };
    let (w, h) = (img.w, img.h);
    let ranks: Vec<(Vec<f32>, Vec<f32>)> = layer.ranks.iter().filter_map(|(i, cv)| Some((sampled(c, &a, a.plane(i)?, w, h, 0.0), sampled(c, &a, a.plane(cv)?, w, h, 0.0)))).collect();
    if ranks.is_empty() {
        return;
    }
    let sel = selection_ids(&c.s("selection"), layer);
    let invert = c.b("invert");
    let matte_only = c.b("matteOnly");
    let display = c.e("display");
    img.px.par_chunks_mut(4).enumerate().for_each(|(i, p)| {
        let mut m = 0.0f32;
        let mut col = [0.0f32; 3];
        let mut top = 0.0f32;
        for (ids, cov) in &ranks {
            let (id, cv) = (ids[i], cov[i]);
            if cv <= 0.0 {
                continue;
            }
            if sel.iter().any(|s| s.to_bits() == id.to_bits()) {
                m += cv;
            }
            let hc = hue_color(id.to_bits());
            for k in 0..3 {
                col[k] += hc[k] * cv;
            }
            top = top.max(cv);
        }
        let m = m.clamp(0.0, 1.0);
        let m = if invert { 1.0 - m } else { m };
        if matte_only {
            p[0] = m;
            p[1] = m;
            p[2] = m;
            p[3] = 1.0;
            return;
        }
        match display {
            0 => {
                let lit = 0.35 + 0.65 * m;
                let base = [p[0], p[1], p[2]];
                for k in 0..3 {
                    p[k] = base[k] * 0.4 + col[k] * lit * 0.6;
                }
                p[3] = p[3].max(top);
            }
            1 => {
                let edge = (1.0 - top).clamp(0.0, 1.0) * 2.0;
                for k in 0..3 {
                    p[k] = p[k] * (1.0 - edge.min(1.0)) + col[k] * edge.min(1.0);
                }
            }
            _ => {
                for v in p.iter_mut() {
                    *v *= m;
                }
            }
        }
    });
}
