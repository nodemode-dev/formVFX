use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::engine::xmldoc::{El, parse};

#[derive(Debug, Clone)]
pub struct Mtl {
    pub name: String,
    pub shader: String,
    pub base: [f64; 4],
    pub metallic: f64,
    pub roughness: f64,
    pub emissive: [f64; 3],
    pub texture: Option<(String, [f64; 2])>,
    pub transparency: f64,
    pub ior: f64,
    pub notes: Vec<String>,
}

struct Doc<'a> {
    nodes: HashMap<String, (&'a El, String)>,
    graphs: HashMap<String, (&'a El, String)>,
    dir: PathBuf,
}

fn floats(s: &str) -> Vec<f64> {
    s.split([',', ' ']).filter(|x| !x.is_empty()).filter_map(|x| x.trim().parse().ok()).collect()
}

fn join_prefix(a: &str, b: &str) -> String {
    format!("{a}{b}")
}

enum In<'a> {
    Value(Vec<f64>),
    Node(&'a El, String),
    None,
}

impl<'a> Doc<'a> {
    fn input(&self, node: &'a El, _prefix: &str, name: &str) -> In<'a> {
        let Some(i) = node.children().find(|c| c.name == "input" && c.attr("name") == Some(name)) else { return In::None };
        if let Some(v) = i.attr("value") {
            return In::Value(floats(v));
        }
        if let Some(g) = i.attr("nodegraph") {
            let Some((graph, gp)) = self.graphs.get(g) else { return In::None };
            let out = i.attr("output").unwrap_or("out");
            let target = graph.children().find(|c| c.name == "output" && c.attr("name") == Some(out)).and_then(|o| o.attr("nodename"));
            let Some(target) = target else { return In::None };
            return match graph.children().find(|c| c.attr("name") == Some(target)) {
                Some(n) => In::Node(n, join_prefix(gp, n.attr("fileprefix").unwrap_or(""))),
                None => In::None,
            };
        }
        if let Some(n) = i.attr("nodename") {
            return match self.nodes.get(n) {
                Some((el, p)) => In::Node(el, join_prefix(p, el.attr("fileprefix").unwrap_or(""))),
                None => In::None,
            };
        }
        In::None
    }

    fn image(&self, node: &El, prefix: &str) -> Option<(String, [f64; 2])> {
        if !matches!(node.name.as_str(), "image" | "tiledimage" | "gltf_image" | "gltf_colorimage" | "UsdUVTexture") {
            return None;
        }
        let file = node.children().find(|c| c.name == "input" && c.attr("name") == Some("file")).and_then(|c| c.attr("value"))?;
        let file = file.replace("<UDIM>", "1001");
        let p = Path::new(&join_prefix(prefix, &file)).to_path_buf();
        let path = if p.is_absolute() { p } else { self.dir.join(p) };
        let tiling = node
            .children()
            .find(|c| c.name == "input" && c.attr("name") == Some("uvtiling"))
            .and_then(|c| c.attr("value"))
            .map(floats)
            .filter(|v| v.len() >= 2)
            .map(|v| [v[0], v[1]])
            .unwrap_or([1.0, 1.0]);
        Some((path.to_string_lossy().to_string(), tiling))
    }

    fn color(&self, node: &'a El, prefix: &str, name: &str, d: [f64; 3], notes: &mut Vec<String>, tex: &mut Option<(String, [f64; 2])>) -> [f64; 3] {
        match self.input(node, prefix, name) {
            In::Value(v) if v.len() >= 3 => [v[0], v[1], v[2]],
            In::Value(v) if v.len() == 1 => [v[0]; 3],
            In::Node(n, p) => match self.image(n, &p) {
                Some(t) => {
                    *tex = Some(t);
                    [1.0; 3]
                }
                None => {
                    notes.push(format!("{name} comes from a `{}` node, which is not read; its default is used", n.name));
                    d
                }
            },
            _ => d,
        }
    }

    fn scalar(&self, node: &'a El, prefix: &str, name: &str, d: f64, notes: &mut Vec<String>) -> f64 {
        match self.input(node, prefix, name) {
            In::Value(v) if !v.is_empty() => v.iter().sum::<f64>() / v.len() as f64,
            In::Node(n, p) => match self.image(n, &p).and_then(|(f, _)| image_mean(&f)) {
                Some(m) => {
                    notes.push(format!("{name} comes from an image; its average ({m:.2}) is used"));
                    m
                }
                None => {
                    notes.push(format!("{name} comes from a `{}` node, which is not read; its default is used", n.name));
                    d
                }
            },
            _ => d,
        }
    }
}

fn image_mean(path: &str) -> Option<f64> {
    let img = image::open(path).ok()?.to_luma32f();
    let n = img.as_raw().len().max(1) as f64;
    Some(img.as_raw().iter().map(|v| *v as f64).sum::<f64>() / n)
}

pub fn materials(text: &str) -> Result<Vec<String>, String> {
    let root = parse(text)?;
    if root.name != "materialx" {
        return Err("not a MaterialX document (no <materialx> root)".into());
    }
    Ok(root.children().filter(|c| c.name == "surfacematerial").filter_map(|c| c.attr("name").map(str::to_string)).collect())
}

pub fn read(path: &str, want: Option<&str>) -> Result<Mtl, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let root = parse(&text).map_err(|e| format!("{path}: {e}"))?;
    if root.name != "materialx" {
        return Err(format!("{path}: not a MaterialX document (no <materialx> root)"));
    }
    let dir = Path::new(path).parent().map(Path::to_path_buf).unwrap_or_default();
    let top = root.attr("fileprefix").unwrap_or("").to_string();
    let mut doc = Doc { nodes: HashMap::new(), graphs: HashMap::new(), dir };
    for c in root.children() {
        let Some(n) = c.attr("name") else { continue };
        if c.name == "nodegraph" {
            doc.graphs.insert(n.to_string(), (c, join_prefix(&top, c.attr("fileprefix").unwrap_or(""))));
        } else {
            doc.nodes.insert(n.to_string(), (c, top.clone()));
        }
    }
    let mats: Vec<&El> = root.children().filter(|c| c.name == "surfacematerial").collect();
    if mats.is_empty() {
        let inc: Vec<&str> = root.children().filter(|c| c.name == "xi:include").filter_map(|c| c.attr("href")).collect();
        if !inc.is_empty() {
            return Err(format!("{path}: no <surfacematerial> here; the materials are in the documents it includes ({}), which formvfx opens directly", inc.join(", ")));
        }
        return Err(format!("{path}: no <surfacematerial> in the document"));
    }
    let mat = match want {
        Some(w) => mats.iter().find(|m| m.attr("name") == Some(w)).copied().ok_or_else(|| format!("{path}: no material `{w}` (materials: {})", mats.iter().filter_map(|m| m.attr("name")).collect::<Vec<_>>().join(", ")))?,
        None => mats[0],
    };
    let name = mat.attr("name").unwrap_or("Material").to_string();
    let shader = match doc.input(mat, &top, "surfaceshader") {
        In::Node(n, p) => (n, p),
        _ => return Err(format!("{path}: material `{name}` has no surface shader")),
    };
    let (s, p) = shader;
    let mut notes = vec![];
    let mut tex = None;
    let mut m = Mtl { name, shader: s.name.clone(), base: [0.8, 0.8, 0.8, 1.0], metallic: 0.0, roughness: 0.5, emissive: [0.0; 3], texture: None, transparency: 0.0, ior: 1.5, notes: vec![] };
    let mean3 = |c: [f64; 3]| (c[0] + c[1] + c[2]) / 3.0;
    match s.name.as_str() {
        "standard_surface" => {
            let k = doc.scalar(s, &p, "base", 1.0, &mut notes);
            let c = doc.color(s, &p, "base_color", [0.8; 3], &mut notes, &mut tex);
            let op = doc.color(s, &p, "opacity", [1.0; 3], &mut notes, &mut None);
            m.base = [c[0] * k, c[1] * k, c[2] * k, mean3(op)];
            m.metallic = doc.scalar(s, &p, "metalness", 0.0, &mut notes);
            m.roughness = doc.scalar(s, &p, "specular_roughness", 0.2, &mut notes);
            let e = doc.scalar(s, &p, "emission", 0.0, &mut notes);
            let ec = doc.color(s, &p, "emission_color", [1.0; 3], &mut notes, &mut None);
            m.emissive = [ec[0] * e, ec[1] * e, ec[2] * e];
            m.transparency = doc.scalar(s, &p, "transmission", 0.0, &mut notes);
            m.ior = doc.scalar(s, &p, "specular_IOR", 1.5, &mut notes);
            let coat = doc.scalar(s, &p, "coat", 0.0, &mut notes);
            if coat > 0.0 {
                let mut coat_tex = None;
                let cc = doc.color(s, &p, "coat_color", [1.0; 3], &mut notes, &mut coat_tex);
                for i in 0..3 {
                    m.base[i] *= 1.0 + (cc[i] - 1.0) * coat.min(1.0);
                }
                if tex.is_none() && coat_tex.is_some() {
                    tex = coat_tex;
                }
                notes.push("coat is approximated as a tint of the base".into());
            }
            for skip in ["sheen", "subsurface", "thin_film_thickness", "specular_anisotropy"] {
                if matches!(doc.input(s, &p, skip), In::Value(ref v) if v.iter().any(|x| *x != 0.0)) {
                    notes.push(format!("{skip} is not modelled by formvfx's 3D renderer"));
                }
            }
        }
        "open_pbr_surface" => {
            let k = doc.scalar(s, &p, "base_weight", 1.0, &mut notes);
            let c = doc.color(s, &p, "base_color", [0.8; 3], &mut notes, &mut tex);
            m.base = [c[0] * k, c[1] * k, c[2] * k, doc.scalar(s, &p, "geometry_opacity", 1.0, &mut notes)];
            m.metallic = doc.scalar(s, &p, "base_metalness", 0.0, &mut notes);
            m.roughness = doc.scalar(s, &p, "specular_roughness", 0.3, &mut notes);
            let lum = doc.scalar(s, &p, "emission_luminance", 0.0, &mut notes);
            let ec = doc.color(s, &p, "emission_color", [1.0; 3], &mut notes, &mut None);
            let e = (lum / 1000.0).min(1.0);
            m.emissive = [ec[0] * e, ec[1] * e, ec[2] * e];
            m.transparency = doc.scalar(s, &p, "transmission_weight", 0.0, &mut notes);
            m.ior = doc.scalar(s, &p, "specular_ior", 1.5, &mut notes);
        }
        "UsdPreviewSurface" => {
            let c = doc.color(s, &p, "diffuseColor", [0.18; 3], &mut notes, &mut tex);
            m.base = [c[0], c[1], c[2], doc.scalar(s, &p, "opacity", 1.0, &mut notes)];
            m.metallic = doc.scalar(s, &p, "metallic", 0.0, &mut notes);
            m.roughness = doc.scalar(s, &p, "roughness", 0.5, &mut notes);
            m.emissive = doc.color(s, &p, "emissiveColor", [0.0; 3], &mut notes, &mut None);
            m.ior = doc.scalar(s, &p, "ior", 1.5, &mut notes);
        }
        "gltf_pbr" => {
            let c = doc.color(s, &p, "base_color", [1.0; 3], &mut notes, &mut tex);
            m.base = [c[0], c[1], c[2], doc.scalar(s, &p, "alpha", 1.0, &mut notes)];
            m.metallic = doc.scalar(s, &p, "metallic", 1.0, &mut notes);
            m.roughness = doc.scalar(s, &p, "roughness", 1.0, &mut notes);
            let k = doc.scalar(s, &p, "emissive_strength", 1.0, &mut notes);
            let e = doc.color(s, &p, "emissive", [0.0; 3], &mut notes, &mut None);
            m.emissive = [e[0] * k, e[1] * k, e[2] * k];
            m.transparency = doc.scalar(s, &p, "transmission", 0.0, &mut notes);
            m.ior = doc.scalar(s, &p, "ior", 1.5, &mut notes);
        }
        other => return Err(format!("{path}: shader `{other}` is not read (standard_surface, open_pbr_surface, UsdPreviewSurface and gltf_pbr are)")),
    }
    if let Some((f, _)) = &tex
        && !Path::new(f).is_file()
    {
        notes.push(format!("the base colour image {f} was not found"));
        tex = None;
    }
    if m.transparency > 0.85 {
        notes.push(format!("transmission {:.2} is kept at 0.85 so the surface and its reflections stay visible (formvfx's transparency fades the whole surface)", m.transparency));
        m.transparency = 0.85;
    }
    m.texture = tex;
    m.notes = notes;
    Ok(m)
}
