use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value as Json;

use super::image::Img;
use crate::geom::Mat4;

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub base: [f32; 4],
    pub texture: Option<Arc<Img>>,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub double_sided: bool,
}

impl Default for Material {
    fn default() -> Self {
        Material { name: String::new(), base: [0.8, 0.8, 0.8, 1.0], texture: None, metallic: 0.0, roughness: 0.5, emissive: [0.0; 3], double_sided: true }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub idx: Vec<u32>,
    pub material: usize,
}

#[derive(Clone, Debug)]
pub struct ModelCamera {
    pub name: String,
    pub world: Mat4,
    pub yfov: f64,
}

#[derive(Clone, Debug)]
pub struct ModelLight {
    pub name: String,
    pub world: Mat4,
    pub kind: u32,
    pub color: [f64; 3],
    pub intensity: f64,
    pub cone: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub bounds: [f64; 6],
    pub cameras: Vec<ModelCamera>,
    pub lights: Vec<ModelLight>,
    pub fit: f64,
}

pub const FIT_SIZE: f64 = 500.0;

impl Model {
    pub fn finish(mut self) -> Model {
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for m in &self.meshes {
            for p in &m.pos {
                for i in 0..3 {
                    lo[i] = lo[i].min(p[i] as f64);
                    hi[i] = hi[i].max(p[i] as f64);
                }
            }
        }
        if lo[0] > hi[0] {
            lo = [0.0; 3];
            hi = [0.0; 3];
        }
        let ext = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(hi[2] - lo[2]).max(1e-9);
        self.fit = FIT_SIZE / ext;
        self.bounds = [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]];
        if self.materials.is_empty() {
            self.materials.push(Material::default());
        }
        for m in &mut self.meshes {
            if m.nrm.len() != m.pos.len() {
                flatten(m);
            }
            m.material = m.material.min(self.materials.len() - 1);
        }
        self
    }

    pub fn to_layer(&self) -> Mat4 {
        let b = &self.bounds;
        let c = [(b[0] + b[3]) * 0.5, (b[1] + b[4]) * 0.5, (b[2] + b[5]) * 0.5];
        Mat4::scale(self.fit, -self.fit, -self.fit) * Mat4::translate(-c[0], -c[1], -c[2])
    }

    pub fn triangles(&self) -> usize {
        self.meshes.iter().map(|m| m.idx.len() / 3).sum()
    }
}

fn flatten(m: &mut Mesh) {
    let mut pos = Vec::with_capacity(m.idx.len());
    let mut uv = Vec::with_capacity(m.idx.len());
    let has_uv = m.uv.len() == m.pos.len();
    for t in m.idx.chunks_exact(3) {
        if t.iter().any(|&i| i as usize >= m.pos.len()) {
            continue;
        }
        for &i in t {
            pos.push(m.pos[i as usize]);
            if has_uv {
                uv.push(m.uv[i as usize]);
            }
        }
    }
    let idx: Vec<u32> = (0..pos.len() as u32).collect();
    m.nrm = smooth_normals(&pos, &idx);
    m.pos = pos;
    m.uv = uv;
    m.idx = idx;
}

pub fn smooth_normals(pos: &[[f32; 3]], idx: &[u32]) -> Vec<[f32; 3]> {
    let mut n = vec![[0.0f32; 3]; pos.len()];
    for t in idx.chunks_exact(3) {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        if a >= pos.len() || b >= pos.len() || c >= pos.len() {
            continue;
        }
        let e1 = [pos[b][0] - pos[a][0], pos[b][1] - pos[a][1], pos[b][2] - pos[a][2]];
        let e2 = [pos[c][0] - pos[a][0], pos[c][1] - pos[a][1], pos[c][2] - pos[a][2]];
        let f = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        for v in [a, b, c] {
            for i in 0..3 {
                n[v][i] += f[i];
            }
        }
    }
    for v in &mut n {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if l > 1e-12 {
            for x in v.iter_mut() {
                *x /= l;
            }
        } else {
            *v = [0.0, 0.0, 1.0];
        }
    }
    n
}

pub fn load(path: &str) -> Result<Model, String> {
    let lower = path.to_ascii_lowercase();
    let data = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let base = Path::new(path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    if lower.ends_with(".obj") {
        return Ok(obj(&String::from_utf8_lossy(&data), &base).finish());
    }
    if data.len() >= 12 && &data[0..4] == b"glTF" {
        let (json, bin) = glb_chunks(&data)?;
        return Ok(gltf(&json, bin, &base)?.finish());
    }
    let json: Json = serde_json::from_slice(&data).map_err(|e| format!("{path} is not a glTF file: {e}"))?;
    Ok(gltf(&json, None, &base)?.finish())
}

fn u32le(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}

fn glb_chunks(data: &[u8]) -> Result<(Json, Option<Vec<u8>>), String> {
    let total = u32le(data, 8).ok_or("short GLB")? as usize;
    let mut at = 12;
    let mut json = None;
    let mut bin = None;
    while at + 8 <= total.min(data.len()) {
        let len = u32le(data, at).ok_or("bad GLB chunk")? as usize;
        let kind = u32le(data, at + 4).ok_or("bad GLB chunk")?;
        let body = data.get(at + 8..at + 8 + len).ok_or("GLB chunk runs past the file")?;
        match kind {
            0x4E4F_534A => json = Some(serde_json::from_slice(body).map_err(|e| format!("GLB JSON: {e}"))?),
            0x004E_4942 => bin = Some(body.to_vec()),
            _ => {}
        }
        at += 8 + len.div_ceil(4) * 4;
    }
    Ok((json.ok_or("GLB without a JSON chunk")?, bin))
}

pub fn base64(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => continue,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

fn uri_bytes(uri: &str, base: &Path) -> Option<Vec<u8>> {
    if let Some(rest) = uri.strip_prefix("data:") {
        let (_, b64) = rest.split_once(";base64,")?;
        return Some(base64(b64));
    }
    let decoded = uri.replace("%20", " ");
    std::fs::read(base.join(decoded)).ok()
}

struct Gltf<'a> {
    j: &'a Json,
    buffers: Vec<Vec<u8>>,
}

fn arr<'a>(j: &'a Json, k: &str) -> &'a [Json] {
    j.get(k).and_then(Json::as_array).map(|v| v.as_slice()).unwrap_or(&[])
}

fn num(j: &Json, k: &str, d: f64) -> f64 {
    j.get(k).and_then(Json::as_f64).unwrap_or(d)
}

fn idx(j: &Json, k: &str) -> Option<usize> {
    j.get(k).and_then(Json::as_u64).map(|v| v as usize)
}

impl Gltf<'_> {
    fn view(&self, i: usize) -> Option<(&[u8], usize)> {
        let v = arr(self.j, "bufferViews").get(i)?;
        let b = self.buffers.get(idx(v, "buffer")?)?;
        let off = idx(v, "byteOffset").unwrap_or(0);
        let len = idx(v, "byteLength")?;
        Some((b.get(off..off + len)?, idx(v, "byteStride").unwrap_or(0)))
    }

    fn accessor(&self, i: usize) -> Option<Vec<Vec<f32>>> {
        let a = arr(self.j, "accessors").get(i)?;
        let count = idx(a, "count")?;
        let comps = match a.get("type")?.as_str()? {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            "MAT4" => 16,
            _ => return None,
        };
        let ct = idx(a, "componentType")?;
        let size = match ct {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            5125 | 5126 => 4,
            _ => return None,
        };
        let normalized = a.get("normalized").and_then(Json::as_bool).unwrap_or(false);
        let Some(vi) = idx(a, "bufferView") else { return Some(vec![vec![0.0; comps]; count]) };
        let (data, stride) = self.view(vi)?;
        let off = idx(a, "byteOffset").unwrap_or(0);
        let stride = if stride > 0 { stride } else { size * comps };
        let mut out = Vec::with_capacity(count);
        for k in 0..count {
            let mut v = Vec::with_capacity(comps);
            for c in 0..comps {
                let at = off + k * stride + c * size;
                let b = data.get(at..at + size)?;
                let x = match ct {
                    5120 => {
                        let x = b[0] as i8 as f32;
                        if normalized { (x / 127.0).max(-1.0) } else { x }
                    }
                    5121 => {
                        let x = b[0] as f32;
                        if normalized { x / 255.0 } else { x }
                    }
                    5122 => {
                        let x = i16::from_le_bytes([b[0], b[1]]) as f32;
                        if normalized { (x / 32767.0).max(-1.0) } else { x }
                    }
                    5123 => {
                        let x = u16::from_le_bytes([b[0], b[1]]) as f32;
                        if normalized { x / 65535.0 } else { x }
                    }
                    5125 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
                    _ => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                };
                v.push(x);
            }
            out.push(v);
        }
        Some(out)
    }

    fn indices(&self, i: usize) -> Option<Vec<u32>> {
        let a = arr(self.j, "accessors").get(i)?;
        let count = idx(a, "count")?;
        let ct = idx(a, "componentType")?;
        let (data, _) = self.view(idx(a, "bufferView")?)?;
        let off = idx(a, "byteOffset").unwrap_or(0);
        let size = match ct {
            5121 => 1,
            5123 => 2,
            _ => 4,
        };
        (0..count)
            .map(|k| {
                let at = off + k * size;
                let b = data.get(at..at + size)?;
                Some(match size {
                    1 => b[0] as u32,
                    2 => u16::from_le_bytes([b[0], b[1]]) as u32,
                    _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                })
            })
            .collect()
    }

    fn image(&self, i: usize, base: &Path) -> Option<Arc<Img>> {
        let im = arr(self.j, "images").get(i)?;
        let bytes = match (im.get("uri").and_then(Json::as_str), idx(im, "bufferView")) {
            (Some(u), _) => uri_bytes(u, base)?,
            (None, Some(v)) => self.view(v)?.0.to_vec(),
            _ => return None,
        };
        let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
        Some(Arc::new(Img::from_rgba8(img.width() as usize, img.height() as usize, img.as_raw(), false)))
    }
}

fn node_matrix(n: &Json) -> Mat4 {
    if let Some(m) = n.get("matrix").and_then(Json::as_array)
        && m.len() == 16
    {
        let v: Vec<f64> = m.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect();
        return Mat4([[v[0], v[4], v[8], v[12]], [v[1], v[5], v[9], v[13]], [v[2], v[6], v[10], v[14]], [v[3], v[7], v[11], v[15]]]);
    }
    let t: Vec<f64> = arr(n, "translation").iter().map(|x| x.as_f64().unwrap_or(0.0)).collect();
    let r: Vec<f64> = arr(n, "rotation").iter().map(|x| x.as_f64().unwrap_or(0.0)).collect();
    let s: Vec<f64> = arr(n, "scale").iter().map(|x| x.as_f64().unwrap_or(1.0)).collect();
    let tm = if t.len() == 3 { Mat4::translate(t[0], t[1], t[2]) } else { Mat4::IDENTITY };
    let rm = if r.len() == 4 { quat(r[0], r[1], r[2], r[3]) } else { Mat4::IDENTITY };
    let sm = if s.len() == 3 { Mat4::scale(s[0], s[1], s[2]) } else { Mat4::IDENTITY };
    tm * rm * sm
}

pub fn quat(x: f64, y: f64, z: f64, w: f64) -> Mat4 {
    let l = (x * x + y * y + z * z + w * w).sqrt().max(1e-12);
    let (x, y, z, w) = (x / l, y / l, z / l, w / l);
    Mat4([
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w), 0.0],
        [2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w), 0.0],
        [2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y), 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

fn gltf(j: &Json, bin: Option<Vec<u8>>, base: &Path) -> Result<Model, String> {
    let mut buffers = vec![];
    for (i, b) in arr(j, "buffers").iter().enumerate() {
        let bytes = match b.get("uri").and_then(Json::as_str) {
            Some(u) => uri_bytes(u, base).ok_or_else(|| format!("buffer {i}: cannot read {u}"))?,
            None => bin.clone().ok_or("buffer without data")?,
        };
        buffers.push(bytes);
    }
    let g = Gltf { j, buffers };
    let mut tex_cache: HashMap<usize, Option<Arc<Img>>> = HashMap::new();
    let mut materials = vec![];
    for m in arr(j, "materials") {
        let pbr = m.get("pbrMetallicRoughness").cloned().unwrap_or(Json::Null);
        let bc: Vec<f32> = arr(&pbr, "baseColorFactor").iter().map(|x| x.as_f64().unwrap_or(1.0) as f32).collect();
        let texture = pbr.get("baseColorTexture").and_then(|t| idx(t, "index")).and_then(|ti| {
            let src = arr(j, "textures").get(ti).and_then(|t| idx(t, "source"))?;
            tex_cache.entry(src).or_insert_with(|| g.image(src, base)).clone()
        });
        let em: Vec<f32> = arr(m, "emissiveFactor").iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect();
        materials.push(Material {
            name: m.get("name").and_then(Json::as_str).unwrap_or("").to_string(),
            base: if bc.len() == 4 { [bc[0], bc[1], bc[2], bc[3]] } else { [1.0; 4] },
            texture,
            metallic: num(&pbr, "metallicFactor", 1.0) as f32,
            roughness: num(&pbr, "roughnessFactor", 1.0) as f32,
            emissive: if em.len() == 3 { [em[0], em[1], em[2]] } else { [0.0; 3] },
            double_sided: m.get("doubleSided").and_then(Json::as_bool).unwrap_or(false),
        });
    }
    let mut model = Model { materials, ..Model::default() };
    let default_mat = model.materials.len();
    let nodes = arr(j, "nodes");
    let scene = idx(j, "scene").unwrap_or(0);
    let roots: Vec<usize> = match arr(j, "scenes").get(scene) {
        Some(s) => arr(s, "nodes").iter().filter_map(|v| v.as_u64().map(|x| x as usize)).collect(),
        None => {
            let mut child = vec![false; nodes.len()];
            for n in nodes {
                for c in arr(n, "children") {
                    if let Some(c) = c.as_u64().and_then(|c| child.get_mut(c as usize)) {
                        *c = true;
                    }
                }
            }
            (0..nodes.len()).filter(|i| !child[*i]).collect()
        }
    };
    let mut stack: Vec<(usize, Mat4, u32)> = roots.into_iter().map(|r| (r, Mat4::IDENTITY, 0)).collect();
    let mut need_default = false;
    while let Some((ni, parent, depth)) = stack.pop() {
        if depth > 64 {
            continue;
        }
        let Some(n) = nodes.get(ni) else { continue };
        let world = parent * node_matrix(n);
        let name = n.get("name").and_then(Json::as_str).unwrap_or("").to_string();
        if let Some(mi) = idx(n, "mesh")
            && let Some(mesh) = arr(j, "meshes").get(mi)
        {
            let normal_m = world.inverse().map(|i| i.transpose()).unwrap_or(world);
            for prim in arr(mesh, "primitives") {
                let mode = idx(prim, "mode").unwrap_or(4);
                if !matches!(mode, 4..=6) {
                    continue;
                }
                let attrs = prim.get("attributes").cloned().unwrap_or(Json::Null);
                let Some(pos) = idx(&attrs, "POSITION").and_then(|a| g.accessor(a)) else { continue };
                let nrm = idx(&attrs, "NORMAL").and_then(|a| g.accessor(a));
                let uv = idx(&attrs, "TEXCOORD_0").and_then(|a| g.accessor(a));
                let mut ix = match idx(prim, "indices") {
                    Some(a) => g.indices(a).unwrap_or_default(),
                    None => (0..pos.len() as u32).collect(),
                };
                if mode == 5 {
                    ix = (2..ix.len()).flat_map(|k| if k % 2 == 0 { [ix[k - 2], ix[k - 1], ix[k]] } else { [ix[k - 1], ix[k - 2], ix[k]] }).collect();
                } else if mode == 6 {
                    ix = (2..ix.len()).flat_map(|k| [ix[0], ix[k - 1], ix[k]]).collect();
                }
                let p: Vec<[f32; 3]> = pos.iter().map(|v| {
                    let w = world.apply([v[0] as f64, v[1] as f64, v.get(2).copied().unwrap_or(0.0) as f64]);
                    [w[0] as f32, w[1] as f32, w[2] as f32]
                }).collect();
                let nn: Vec<[f32; 3]> = nrm
                    .map(|ns| {
                        ns.iter()
                            .map(|v| {
                                let o = normal_m.apply_dir([v[0] as f64, v[1] as f64, v.get(2).copied().unwrap_or(0.0) as f64]);
                                let l = (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt().max(1e-12);
                                [(o[0] / l) as f32, (o[1] / l) as f32, (o[2] / l) as f32]
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let tc: Vec<[f32; 2]> = uv.map(|u| u.iter().map(|v| [v[0], v.get(1).copied().unwrap_or(0.0)]).collect()).unwrap_or_default();
                let material = match idx(prim, "material") {
                    Some(m) => m,
                    None => {
                        need_default = true;
                        default_mat
                    }
                };
                model.meshes.push(Mesh { pos: p, nrm: nn, uv: tc, idx: ix, material });
            }
        }
        if let Some(ci) = idx(n, "camera")
            && let Some(c) = arr(j, "cameras").get(ci)
        {
            let yfov = c.get("perspective").map(|p| num(p, "yfov", 0.8)).unwrap_or(0.8);
            model.cameras.push(ModelCamera { name: if name.is_empty() { format!("Camera {}", model.cameras.len() + 1) } else { name.clone() }, world, yfov });
        }
        if let Some(li) = n.get("extensions").and_then(|e| e.get("KHR_lights_punctual")).and_then(|e| idx(e, "light"))
            && let Some(l) = j.get("extensions").and_then(|e| e.get("KHR_lights_punctual")).map(|e| arr(e, "lights")).and_then(|a| a.get(li))
        {
            let col: Vec<f64> = arr(l, "color").iter().map(|x| x.as_f64().unwrap_or(1.0)).collect();
            let kind = match l.get("type").and_then(Json::as_str) {
                Some("directional") => 0,
                Some("spot") => 1,
                _ => 2,
            };
            let cone = l.get("spot").map(|s| num(s, "outerConeAngle", std::f64::consts::FRAC_PI_4)).unwrap_or(std::f64::consts::FRAC_PI_4).to_degrees() * 2.0;
            model.lights.push(ModelLight { name: if name.is_empty() { format!("Light {}", model.lights.len() + 1) } else { name.clone() }, world, kind, color: if col.len() == 3 { [col[0], col[1], col[2]] } else { [1.0; 3] }, intensity: num(l, "intensity", 1.0), cone });
        }
        for c in arr(n, "children") {
            if let Some(c) = c.as_u64() {
                stack.push((c as usize, world, depth + 1));
            }
        }
    }
    if need_default {
        model.materials.push(Material::default());
    }
    if model.meshes.is_empty() {
        return Err("the glTF file has no triangle meshes".into());
    }
    Ok(model)
}

fn mtl(text: &str, base: &Path) -> HashMap<String, Material> {
    let mut out = HashMap::new();
    let mut cur: Option<Material> = None;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(k) = it.next() else { continue };
        let rest: Vec<&str> = it.collect();
        let f = |i: usize| rest.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        match k {
            "newmtl" => {
                if let Some(m) = cur.take() {
                    out.insert(m.name.clone(), m);
                }
                cur = Some(Material { name: rest.join(" "), metallic: 0.0, roughness: 0.6, ..Material::default() });
            }
            "Kd" => {
                if let Some(m) = &mut cur {
                    m.base = [f(0), f(1), f(2), m.base[3]];
                }
            }
            "d" => {
                if let Some(m) = &mut cur {
                    m.base[3] = f(0);
                }
            }
            "Tr" => {
                if let Some(m) = &mut cur {
                    m.base[3] = 1.0 - f(0);
                }
            }
            "Ke" => {
                if let Some(m) = &mut cur {
                    m.emissive = [f(0), f(1), f(2)];
                }
            }
            "Ns" => {
                if let Some(m) = &mut cur {
                    m.roughness = (1.0 - (f(0) / 1000.0).sqrt()).clamp(0.02, 1.0);
                }
            }
            "Pm" => {
                if let Some(m) = &mut cur {
                    m.metallic = f(0);
                }
            }
            "Pr" => {
                if let Some(m) = &mut cur {
                    m.roughness = f(0);
                }
            }
            "map_Kd" => {
                if let Some(m) = &mut cur
                    && let Some(file) = rest.last()
                    && let Ok(img) = image::open(base.join(file))
                {
                    let img = img.to_rgba8();
                    m.texture = Some(Arc::new(Img::from_rgba8(img.width() as usize, img.height() as usize, img.as_raw(), false)));
                }
            }
            _ => {}
        }
    }
    if let Some(m) = cur.take() {
        out.insert(m.name.clone(), m);
    }
    out
}

fn obj(text: &str, base: &Path) -> Model {
    let mut v: Vec<[f32; 3]> = vec![];
    let mut vt: Vec<[f32; 2]> = vec![];
    let mut vn: Vec<[f32; 3]> = vec![];
    let mut mats: HashMap<String, Material> = HashMap::new();
    let mut model = Model::default();
    let mut names: Vec<String> = vec![];
    let mut cur = Mesh::default();
    let mut cur_mat = 0usize;
    let mut has_n = true;
    let mut has_t = true;
    let mut map: HashMap<(i64, i64, i64), u32> = HashMap::new();
    let flush = |cur: &mut Mesh, model: &mut Model, map: &mut HashMap<(i64, i64, i64), u32>, has_n: bool, has_t: bool, mat: usize| {
        if !cur.idx.is_empty() {
            let mut m = std::mem::take(cur);
            if !has_n {
                m.nrm.clear();
            }
            if !has_t {
                m.uv.clear();
            }
            m.material = mat;
            model.meshes.push(m);
        }
        map.clear();
    };
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(k) = it.next() else { continue };
        let rest: Vec<&str> = it.collect();
        let f = |i: usize| rest.get(i).and_then(|x| x.parse::<f32>().ok()).unwrap_or(0.0);
        match k {
            "v" => v.push([f(0), f(1), f(2)]),
            "vt" => vt.push([f(0), f(1)]),
            "vn" => vn.push([f(0), f(1), f(2)]),
            "mtllib" => {
                for file in &rest {
                    if let Ok(t) = std::fs::read_to_string(base.join(file)) {
                        mats.extend(mtl(&t, base));
                    }
                }
            }
            "usemtl" => {
                flush(&mut cur, &mut model, &mut map, has_n, has_t, cur_mat);
                has_n = true;
                has_t = true;
                let name = rest.join(" ");
                cur_mat = match names.iter().position(|n| *n == name) {
                    Some(i) => i,
                    None => {
                        names.push(name);
                        names.len() - 1
                    }
                };
            }
            "f" => {
                let mut poly = vec![];
                for tok in &rest {
                    let mut parts = tok.split('/');
                    let r = |s: Option<&str>, n: usize| -> i64 {
                        match s.and_then(|x| x.parse::<i64>().ok()) {
                            Some(i) if i < 0 => n as i64 + i,
                            Some(i) => i - 1,
                            None => -1,
                        }
                    };
                    let a = r(parts.next(), v.len());
                    let b = r(parts.next(), vt.len());
                    let c = r(parts.next(), vn.len());
                    if a < 0 || a as usize >= v.len() {
                        continue;
                    }
                    let key = (a, b, c);
                    let id = *map.entry(key).or_insert_with(|| {
                        cur.pos.push(v[a as usize]);
                        if b >= 0 && (b as usize) < vt.len() {
                            cur.uv.push(vt[b as usize]);
                        } else {
                            has_t = false;
                            cur.uv.push([0.0; 2]);
                        }
                        if c >= 0 && (c as usize) < vn.len() {
                            cur.nrm.push(vn[c as usize]);
                        } else {
                            has_n = false;
                            cur.nrm.push([0.0, 0.0, 1.0]);
                        }
                        (cur.pos.len() - 1) as u32
                    });
                    poly.push(id);
                }
                for k in 2..poly.len() {
                    cur.idx.extend_from_slice(&[poly[0], poly[k - 1], poly[k]]);
                }
            }
            _ => {}
        }
    }
    flush(&mut cur, &mut model, &mut map, has_n, has_t, cur_mat);
    model.materials = if names.is_empty() { vec![Material { roughness: 0.6, metallic: 0.0, ..Material::default() }] } else { names.iter().map(|n| mats.get(n).cloned().unwrap_or(Material { name: n.clone(), roughness: 0.6, metallic: 0.0, ..Material::default() })).collect() };
    model
}

#[derive(Default)]
pub struct ModelCache {
    map: parking_lot::Mutex<HashMap<String, (std::time::SystemTime, Arc<Model>)>>,
}

impl ModelCache {
    pub fn get(&self, path: &str) -> Option<Arc<Model>> {
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
        if let Some((t, m)) = self.map.lock().get(path)
            && *t == mtime
        {
            return Some(m.clone());
        }
        let m = Arc::new(load(path).ok()?);
        self.map.lock().insert(path.to_string(), (mtime, m.clone()));
        Some(m)
    }
}
