use std::collections::HashMap;
use std::path::Path;

use serde_json::{Map, Value as Json, json};

use super::cmd::comp::make_comp;
use super::cmd::{add_layer, solid_item};
use super::{Engine, Res, pb, ps};
use crate::eval::Eval;
use crate::model::key::{Ease, Interp, Key, ease_dims, roving_times, side_ease, spatial_tangents};
use crate::model::prop::{mask_group, shape_item};
use crate::model::{
    AutoOrient, BezPath, BlendMode, Comp, Footage, Gradient, Group, GroupKind, Item, ItemKind, Justify, Layer, LayerInit, LayerKind, MaskMode, Matte, MatteKind, Node, Project, Prop, TextDoc, Value, build_layer,
};
use crate::model::value::GradStop;

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(B64[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for b in data {
        c ^= *b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

pub fn zip(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![];
    let mut central = vec![];
    for (name, data) in files {
        let comp = miniz_oxide::deflate::compress_to_vec(data, 6);
        let crc = crc32(data);
        let off = out.len() as u32;
        let head = |sig: u32, central: bool| {
            let mut h = vec![];
            h.extend_from_slice(&sig.to_le_bytes());
            if central {
                h.extend_from_slice(&20u16.to_le_bytes());
            }
            h.extend_from_slice(&20u16.to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            h.extend_from_slice(&8u16.to_le_bytes());
            h.extend_from_slice(&0u32.to_le_bytes());
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&(comp.len() as u32).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            if central {
                h.extend_from_slice(&[0; 10]);
                h.extend_from_slice(&off.to_le_bytes());
            }
            h.extend_from_slice(name.as_bytes());
            h
        };
        out.extend(head(0x0403_4b50, false));
        out.extend_from_slice(&comp);
        central.extend(head(0x0201_4b50, true));
    }
    let cd = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

pub fn unzip(b: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    let bad = || "the file is not a readable zip archive".to_string();
    let u16_at = |i: usize| b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize);
    let u32_at = |i: usize| b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize);
    let lo = b.len().saturating_sub(65_557);
    let eocd = (lo..b.len().saturating_sub(21)).rev().find(|&i| u32_at(i) == Some(0x0605_4b50)).ok_or_else(bad)?;
    let count = u16_at(eocd + 10).ok_or_else(bad)?;
    let mut i = u32_at(eocd + 16).ok_or_else(bad)?;
    let mut out = HashMap::new();
    for _ in 0..count {
        if u32_at(i) != Some(0x0201_4b50) {
            return Err(bad());
        }
        let method = u16_at(i + 10).ok_or_else(bad)?;
        let csize = u32_at(i + 20).ok_or_else(bad)?;
        let nlen = u16_at(i + 28).ok_or_else(bad)?;
        let xlen = u16_at(i + 30).ok_or_else(bad)?;
        let clen = u16_at(i + 32).ok_or_else(bad)?;
        let local = u32_at(i + 42).ok_or_else(bad)?;
        let name = String::from_utf8_lossy(b.get(i + 46..i + 46 + nlen).ok_or_else(bad)?).to_string();
        i += 46 + nlen + xlen + clen;
        let ln = u16_at(local + 26).ok_or_else(bad)?;
        let lx = u16_at(local + 28).ok_or_else(bad)?;
        let start = local + 30 + ln + lx;
        let raw = b.get(start..start + csize).ok_or_else(bad)?;
        let data = match method {
            0 => raw.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec(raw).map_err(|_| bad())?,
            _ => return Err(format!("zip compression {method} is not supported")),
        };
        out.insert(name, data);
    }
    Ok(out)
}

fn emit(v: &Json, out: &mut String) {
    match v {
        Json::Object(m) => {
            out.push('{');
            let mut first = true;
            let keys = m.keys().filter(|k| k.as_str() == "ty").chain(m.keys().filter(|k| k.as_str() != "ty"));
            for k in keys {
                if !first {
                    out.push(',');
                }
                first = false;
                out.push_str(&serde_json::to_string(k).unwrap_or_default());
                out.push(':');
                emit(&m[k], out);
            }
            out.push('}');
        }
        Json::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                emit(x, out);
            }
            out.push(']');
        }
        other => out.push_str(&serde_json::to_string(other).unwrap_or_default()),
    }
}

fn read_json(path: &str) -> Result<Json, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if path.to_ascii_lowercase().ends_with(".lottie") || b.starts_with(b"PK") {
        let files = unzip(&b)?;
        let manifest: Json = files.get("manifest.json").and_then(|m| serde_json::from_slice(m).ok()).unwrap_or(json!({}));
        let id = manifest.get("animations").and_then(Json::as_array).and_then(|a| a.first()).and_then(|a| a.get("id")).and_then(Json::as_str).map(str::to_string);
        let key = id
            .map(|id| format!("animations/{id}.json"))
            .filter(|k| files.contains_key(k))
            .or_else(|| files.keys().filter(|k| k.starts_with("animations/") && k.ends_with(".json")).min().cloned())
            .ok_or("the .lottie file has no animation")?;
        return serde_json::from_slice(&files[&key]).map_err(|e| format!("{path}: {e}"));
    }
    serde_json::from_slice(&b).map_err(|e| format!("{path}: {e}"))
}

pub fn is_lottie(path: &str) -> bool {
    if path.to_ascii_lowercase().ends_with(".lottie") {
        return true;
    }
    read_json(path).is_ok_and(|j| j.get("layers").is_some_and(Json::is_array) && j.get("fr").is_some() && j.get("w").is_some())
}

fn blend_code(m: BlendMode) -> u32 {
    match m {
        BlendMode::Multiply => 1,
        BlendMode::Screen => 2,
        BlendMode::Overlay => 3,
        BlendMode::Darken => 4,
        BlendMode::Lighten => 5,
        BlendMode::ColorDodge | BlendMode::ClassicColorDodge => 6,
        BlendMode::ColorBurn | BlendMode::ClassicColorBurn => 7,
        BlendMode::HardLight => 8,
        BlendMode::SoftLight => 9,
        BlendMode::Difference | BlendMode::ClassicDifference => 10,
        BlendMode::Exclusion => 11,
        BlendMode::Hue => 12,
        BlendMode::Saturation => 13,
        BlendMode::Color => 14,
        BlendMode::Luminosity => 15,
        BlendMode::Add | BlendMode::LinearDodge => 16,
        BlendMode::HardMix => 17,
        _ => 0,
    }
}

fn blend_of(n: u64) -> BlendMode {
    match n {
        1 => BlendMode::Multiply,
        2 => BlendMode::Screen,
        3 => BlendMode::Overlay,
        4 => BlendMode::Darken,
        5 => BlendMode::Lighten,
        6 => BlendMode::ColorDodge,
        7 => BlendMode::ColorBurn,
        8 => BlendMode::HardLight,
        9 => BlendMode::SoftLight,
        10 => BlendMode::Difference,
        11 => BlendMode::Exclusion,
        12 => BlendMode::Hue,
        13 => BlendMode::Saturation,
        14 => BlendMode::Color,
        15 => BlendMode::Luminosity,
        16 => BlendMode::Add,
        17 => BlendMode::HardMix,
        _ => BlendMode::Normal,
    }
}

fn path_json(p: &BezPath) -> Json {
    json!({"i": p.ins, "o": p.outs, "v": p.vertices, "c": p.closed})
}

fn grad_json(g: &Gradient) -> Json {
    let mut v = vec![];
    for s in &g.stops {
        v.extend([s.pos, s.color[0], s.color[1], s.color[2]]);
    }
    if g.stops.iter().any(|s| s.color[3] < 0.999) {
        for s in &g.stops {
            v.extend([s.pos, s.color[3]]);
        }
    }
    json!(v)
}

fn value_json(v: &Value) -> Json {
    match v {
        Value::Num(x) => json!([x]),
        Value::Vec2(a) => json!(a),
        Value::Vec3(a) => json!(a),
        Value::Color(c) => json!(c),
        Value::Bool(b) => json!([if *b { 1 } else { 0 }]),
        Value::Enum(n) => json!([n]),
        Value::Path(p) => json!([path_json(p)]),
        Value::Gradient(g) => grad_json(g),
        _ => json!([0]),
    }
}

fn static_json(v: &Value) -> Json {
    match v {
        Value::Num(x) => json!(x),
        Value::Path(p) => path_json(p),
        other => value_json(other),
    }
}

fn arc_len(p0: [f64; 3], p1: [f64; 3], p2: [f64; 3], p3: [f64; 3]) -> f64 {
    let mut len = 0.0;
    let mut prev = p0;
    for k in 1..=32 {
        let u = k as f64 / 32.0;
        let v = 1.0 - u;
        let p = [0, 1, 2].map(|i| v * v * v * p0[i] + 3.0 * v * v * u * p1[i] + 3.0 * v * u * u * p2[i] + u * u * u * p3[i]);
        len += ((p[0] - prev[0]).powi(2) + (p[1] - prev[1]).powi(2) + (p[2] - prev[2]).powi(2)).sqrt();
        prev = p;
    }
    len
}

struct Ex<'a> {
    project: &'a Project,
    media: &'a crate::render::Media,
    expr: bool,
    text_shapes: bool,
    assets: Vec<Json>,
    comps: HashMap<u64, String>,
    images: HashMap<u64, String>,
    fonts: Vec<Json>,
    warnings: Vec<String>,
}

impl<'a> Ex<'a> {
    fn warn(&mut self, w: String) {
        if !self.warnings.contains(&w) {
            self.warnings.push(w);
        }
    }

    fn anim(&mut self, cid: u64, l: &Layer, p: &Prop, fps: f64) -> Json {
        let comp = self.project.comp(cid);
        if let (Some(ex), Some(comp)) = (&p.expr, comp)
            && ex.enabled
        {
            if self.expr {
                let mut o = self.keyed(p, fps);
                o["x"] = json!(ex.text);
                return o;
            }
            let (f0, f1) = ((l.in_point * comp.frame_rate).floor() as i64, (l.out_point * comp.frame_rate).ceil() as i64);
            let mut keys = vec![];
            let mut first: Option<Value> = None;
            let mut varies = false;
            for f in f0..=f1.max(f0) {
                let t = f as f64 / comp.frame_rate;
                let v = Eval::new(self.project, cid, comp, t).value(l, p);
                if let Some(a) = &first {
                    varies |= *a != v;
                } else {
                    first = Some(v.clone());
                }
                keys.push(json!({"t": (l.layer_time(t) * fps * 1e6).round() / 1e6, "s": value_json(&v), "o": {"x": [0.0], "y": [0.0]}, "i": {"x": [1.0], "y": [1.0]}}));
            }
            if !varies {
                return json!({"a": 0, "k": static_json(first.as_ref().unwrap_or(&p.value))});
            }
            if let Some(Json::Object(last)) = keys.last_mut() {
                last.remove("o");
                last.remove("i");
            }
            return json!({"a": 1, "k": keys});
        }
        self.keyed(p, fps)
    }

    fn keyed(&self, p: &Prop, fps: f64) -> Json {
        if p.keys.is_empty() {
            return json!({"a": 0, "k": static_json(&p.value)});
        }
        let spatial = p.spatial && matches!(p.value, Value::Vec2(_) | Value::Vec3(_));
        let keys = if spatial && crate::model::key::has_roving(&p.keys) { roving_times(&p.keys) } else { p.keys.clone() };
        let n = keys.len();
        let mut out = vec![];
        for i in 0..n {
            let k = &keys[i];
            let mut o = Map::new();
            o.insert("t".into(), json!(k.time * fps));
            o.insert("s".into(), value_json(&k.value));
            if i + 1 < n {
                let b = &keys[i + 1];
                if k.out_interp == Interp::Hold {
                    o.insert("h".into(), json!(1));
                } else {
                    let dt = (b.time - k.time).max(1e-9);
                    let dims = ease_dims(&k.value, spatial);
                    let (mut ox, mut oy, mut ix, mut iy) = (vec![], vec![], vec![], vec![]);
                    let span = if spatial {
                        let (_, to) = spatial_tangents(&keys, i);
                        let (ti, _) = spatial_tangents(&keys, i + 1);
                        let p0 = k.value.vec3();
                        let p3 = b.value.vec3();
                        o.insert("to".into(), json!(to));
                        o.insert("ti".into(), json!(ti));
                        vec![arc_len(p0, [p0[0] + to[0], p0[1] + to[1], p0[2] + to[2]], [p3[0] + ti[0], p3[1] + ti[1], p3[2] + ti[2]], p3)]
                    } else if k.value.is_numeric() {
                        let (a, c) = (k.value.components(), b.value.components());
                        (0..dims).map(|d| c.get(d).copied().unwrap_or(0.0) - a.get(d).copied().unwrap_or(0.0)).collect()
                    } else {
                        vec![1.0]
                    };
                    let scale_ease = |e: Option<Ease>| if !spatial && !k.value.is_numeric() { e.map(|e| Ease { speed: e.speed * dt / 100.0, influence: e.influence }) } else { e };
                    for d in 0..span.len() {
                        let dy = span[d];
                        let eo = scale_ease(side_ease(&keys, i, d, spatial, true));
                        let ei = scale_ease(side_ease(&keys, i + 1, d, spatial, false));
                        if eo.is_none() && ei.is_none() {
                            ox.push(0.0);
                            oy.push(0.0);
                            ix.push(1.0);
                            iy.push(1.0);
                            continue;
                        }
                        let (x0, y0) = match eo {
                            Some(e) => {
                                let x = (e.influence / 100.0).clamp(0.001, 1.0);
                                (x, if dy.abs() > 1e-9 { e.speed * x * dt / dy } else { 0.0 })
                            }
                            None => (1.0 / 3.0, 1.0 / 3.0),
                        };
                        let (x1, y1) = match ei {
                            Some(e) => {
                                let x = (e.influence / 100.0).clamp(0.001, 1.0);
                                (1.0 - x, if dy.abs() > 1e-9 { 1.0 - e.speed * x * dt / dy } else { 1.0 })
                            }
                            None => (2.0 / 3.0, 2.0 / 3.0),
                        };
                        ox.push(x0);
                        oy.push(y0);
                        ix.push(x1);
                        iy.push(y1);
                    }
                    o.insert("o".into(), json!({"x": ox, "y": oy}));
                    o.insert("i".into(), json!({"x": ix, "y": iy}));
                }
            }
            out.push(Json::Object(o));
        }
        json!({"a": 1, "k": out})
    }

    fn prop(&mut self, cid: u64, l: &Layer, g: &Group, id: &str, fps: f64) -> Option<Json> {
        let p = g.get(id)?;
        Some(self.anim(cid, l, p, fps))
    }

    fn transform(&mut self, cid: u64, l: &Layer, fps: f64) -> Json {
        let mut o = Map::new();
        let Some(tr) = l.transform() else { return Json::Object(o) };
        for (id, key) in [("anchor", "a"), ("position", "p"), ("scale", "s"), ("opacity", "o")] {
            if let Some(v) = self.prop(cid, l, tr, id, fps) {
                o.insert(key.into(), v);
            }
        }
        if l.is_3d() {
            for (id, key) in [("rotation", "rz"), ("rotationX", "rx"), ("rotationY", "ry"), ("orientation", "or")] {
                if let Some(v) = self.prop(cid, l, tr, id, fps) {
                    o.insert(key.into(), v);
                }
            }
        } else if let Some(v) = self.prop(cid, l, tr, "rotation", fps) {
            o.insert("r".into(), v);
        }
        if !o.contains_key("o") {
            o.insert("o".into(), json!({"a": 0, "k": 100}));
        }
        Json::Object(o)
    }

    fn shape_items(&mut self, cid: u64, l: &Layer, g: &Group, fps: f64) -> Vec<Json> {
        let mut out = vec![];
        for c in g.groups() {
            if let Some(mut j) = self.shape_item(cid, l, c, fps) {
                j["nm"] = json!(c.name);
                if !c.enabled {
                    j["hd"] = json!(true);
                }
                out.push(j);
            }
        }
        out
    }

    fn shape_item(&mut self, cid: u64, l: &Layer, g: &Group, fps: f64) -> Option<Json> {
        let pr = |me: &mut Self, id: &str| me.prop(cid, l, g, id, fps).unwrap_or(json!({"a": 0, "k": 0}));
        let en = |id: &str| g.get(id).map(|p| p.value.index()).unwrap_or(0);
        Some(match g.id.as_str() {
            "group" => {
                let mut items = g.sub("contents").map(|c| self.shape_items(cid, l, c, fps)).unwrap_or_default();
                let mut tr = json!({"ty": "tr", "nm": "Transform"});
                if let Some(t) = g.sub("transform") {
                    for (id, key) in [("anchor", "a"), ("position", "p"), ("scale", "s"), ("rotation", "r"), ("opacity", "o"), ("skew", "sk"), ("skewAxis", "sa")] {
                        if let Some(v) = self.prop(cid, l, t, id, fps) {
                            tr[key] = v;
                        }
                    }
                }
                items.push(tr);
                json!({"ty": "gr", "it": items, "np": items.len()})
            }
            "rect" => json!({"ty": "rc", "d": 1, "s": pr(self, "size"), "p": pr(self, "position"), "r": pr(self, "roundness")}),
            "ellipse" => json!({"ty": "el", "d": 1, "s": pr(self, "size"), "p": pr(self, "position")}),
            "star" => json!({"ty": "sr", "d": 1, "sy": en("type") + 1, "pt": pr(self, "points"), "p": pr(self, "position"), "r": pr(self, "rotation"), "ir": pr(self, "innerRadius"), "or": pr(self, "outerRadius"), "is": pr(self, "innerRoundness"), "os": pr(self, "outerRoundness")}),
            "path" => json!({"ty": "sh", "d": 1, "ks": pr(self, "path")}),
            "fill" => json!({"ty": "fl", "c": pr(self, "color"), "o": pr(self, "opacity"), "r": en("rule") + 1}),
            "stroke" | "gradientStroke" => {
                let mut j = json!({"o": pr(self, "opacity"), "w": pr(self, "width"), "lc": en("cap") + 1, "lj": en("join") + 1, "ml": g.get("miter").map(|p| p.value.num()).unwrap_or(4.0)});
                let mut dashes = vec![];
                for (id, n) in [("dash", "d"), ("gap", "g"), ("dash2", "d"), ("gap2", "g"), ("dash3", "d"), ("gap3", "g")] {
                    if g.get(id).is_some_and(|p| p.value.num() > 0.0 || !p.keys.is_empty()) {
                        dashes.push(json!({"n": n, "nm": id, "v": pr(self, id)}));
                    }
                }
                if !dashes.is_empty() {
                    dashes.push(json!({"n": "o", "nm": "offset", "v": pr(self, "dashOffset")}));
                    j["d"] = json!(dashes);
                }
                if g.id == "stroke" {
                    j["ty"] = json!("st");
                    j["c"] = pr(self, "color");
                } else {
                    j["ty"] = json!("gs");
                    self.gradient(&mut j, g, cid, l, fps);
                }
                j
            }
            "gradientFill" => {
                let mut j = json!({"ty": "gf", "o": pr(self, "opacity"), "r": en("rule") + 1});
                self.gradient(&mut j, g, cid, l, fps);
                j
            }
            "trim" => json!({"ty": "tm", "s": pr(self, "start"), "e": pr(self, "end"), "o": pr(self, "offset"), "m": en("mode") + 1}),
            "repeater" => {
                let mut tr = json!({"ty": "tr", "nm": "Transform"});
                if let Some(t) = g.sub("transform") {
                    for (id, key) in [("anchor", "a"), ("position", "p"), ("scale", "s"), ("rotation", "r"), ("startOpacity", "so"), ("endOpacity", "eo")] {
                        if let Some(v) = self.prop(cid, l, t, id, fps) {
                            tr[key] = v;
                        }
                    }
                }
                json!({"ty": "rp", "c": pr(self, "copies"), "o": pr(self, "offset"), "m": if en("composite") == 1 { 1 } else { 2 }, "tr": tr})
            }
            "roundCorners" => json!({"ty": "rd", "r": pr(self, "radius")}),
            "offsetPaths" => json!({"ty": "op", "a": pr(self, "amount"), "lj": en("join") + 1, "ml": pr(self, "miter")}),
            "mergePaths" => json!({"ty": "mm", "mm": en("mode") + 1}),
            "puckerBloat" => json!({"ty": "pb", "a": pr(self, "amount")}),
            "zigZag" => json!({"ty": "zz", "s": pr(self, "size"), "r": pr(self, "ridges"), "pt": json!({"a": 0, "k": en("points") + 1})}),
            "twist" => json!({"ty": "tw", "a": pr(self, "angle"), "c": pr(self, "center")}),
            other => {
                self.warn(format!("{} is not part of Lottie and was left out", match other {
                    "wiggle" => "Wiggle Paths",
                    "wiggleTransform" => "Wiggle Transform",
                    o => o,
                }));
                return None;
            }
        })
    }

    fn gradient(&mut self, j: &mut Json, g: &Group, cid: u64, l: &Layer, fps: f64) {
        let stops = g.get("colors").and_then(|p| p.value.as_gradient().map(|x| x.stops.len())).unwrap_or(2);
        j["t"] = json!(g.get("type").map(|p| p.value.index()).unwrap_or(0) + 1);
        j["s"] = self.prop(cid, l, g, "start", fps).unwrap_or(json!({"a": 0, "k": [0, 0]}));
        j["e"] = self.prop(cid, l, g, "end", fps).unwrap_or(json!({"a": 0, "k": [100, 0]}));
        j["g"] = json!({"p": stops, "k": self.prop(cid, l, g, "colors", fps).unwrap_or(json!({"a": 0, "k": []}))});
    }

    fn masks(&mut self, cid: u64, l: &Layer, fps: f64) -> Vec<Json> {
        let mut out = vec![];
        let Some(ms) = l.masks() else { return out };
        for m in ms.groups() {
            let GroupKind::Mask { mode, inverted, .. } = &m.kind else { continue };
            let code = match mode {
                MaskMode::None => "n",
                MaskMode::Add => "a",
                MaskMode::Subtract => "s",
                MaskMode::Intersect => "i",
                MaskMode::Lighten => "l",
                MaskMode::Darken => "da",
                MaskMode::Difference => "f",
            };
            let mut j = json!({"inv": inverted, "mode": code, "nm": m.name});
            for (id, key) in [("path", "pt"), ("opacity", "o"), ("expansion", "x")] {
                if let Some(v) = self.prop(cid, l, m, id, fps) {
                    j[key] = v;
                }
            }
            out.push(j);
        }
        out
    }

    fn image_asset(&mut self, item: u64, f: &Footage) -> Option<String> {
        if let Some(id) = self.images.get(&item) {
            return Some(id.clone());
        }
        let img = self.media.frame(f, 0.0)?;
        let buf = image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.to_rgba8())?;
        let mut png = std::io::Cursor::new(vec![]);
        buf.write_to(&mut png, image::ImageFormat::Png).ok()?;
        let id = format!("image_{}", self.images.len());
        self.assets.push(json!({"id": id, "w": img.w, "h": img.h, "u": "", "p": format!("data:image/png;base64,{}", b64(png.get_ref())), "e": 1}));
        self.images.insert(item, id.clone());
        Some(id)
    }

    fn comp_asset(&mut self, cid: u64, depth: u32) -> Option<String> {
        if let Some(id) = self.comps.get(&cid) {
            return Some(id.clone());
        }
        if depth > 16 {
            return None;
        }
        let id = format!("comp_{}", self.comps.len());
        self.comps.insert(cid, id.clone());
        let layers = self.layers(cid, depth + 1);
        let c = self.project.comp(cid)?;
        let nm = self.project.item(cid).map(|i| i.name.clone()).unwrap_or_default();
        self.assets.push(json!({"id": id, "nm": nm, "fr": c.frame_rate, "layers": layers}));
        Some(id)
    }

    fn layers(&mut self, cid: u64, depth: u32) -> Vec<Json> {
        let Some(comp) = self.project.comp(cid) else { return vec![] };
        let comp: &'a Comp = comp;
        let fps = comp.frame_rate;
        let ind: HashMap<u64, usize> = comp.layers.iter().enumerate().map(|(i, l)| (l.id, i + 1)).collect();
        let mattes: Vec<u64> = comp.layers.iter().filter_map(|l| l.track_matte.as_ref().map(|m| m.layer)).collect();
        let mut out = vec![];
        let mut built: Vec<(u64, Json)> = vec![];
        for (i, l) in comp.layers.iter().enumerate() {
            let mut j = json!({"ddd": if l.is_3d() { 1 } else { 0 }, "ind": i + 1, "nm": l.name, "sr": l.stretch / 100.0, "ip": l.in_point * fps, "op": l.out_point * fps, "st": l.start_time * fps, "bm": blend_code(l.blend), "ao": if l.auto_orient == AutoOrient::AlongPath { 1 } else { 0 }});
            let src = l.source.and_then(|s| self.project.item(s));
            match l.kind {
                LayerKind::Solid if l.switches.adjustment => {
                    self.warn(format!("adjustment layer `{}` was left out (Lottie has no adjustment layers)", l.name));
                    continue;
                }
                LayerKind::Solid => {
                    let Some(ItemKind::Solid(s)) = src.map(|i| &i.kind) else { continue };
                    let hex = format!("#{:02x}{:02x}{:02x}", (s.color[0] * 255.0).round() as u8, (s.color[1] * 255.0).round() as u8, (s.color[2] * 255.0).round() as u8);
                    j["ty"] = json!(1);
                    j["sc"] = json!(hex);
                    j["sw"] = json!(s.width);
                    j["sh"] = json!(s.height);
                }
                LayerKind::Null => j["ty"] = json!(3),
                LayerKind::Precomp => {
                    let Some(sid) = l.source else { continue };
                    let Some(asset) = self.comp_asset(sid, depth) else { continue };
                    let Some(sub) = self.project.comp(sid) else { continue };
                    j["ty"] = json!(0);
                    j["refId"] = json!(asset);
                    j["w"] = json!(sub.width);
                    j["h"] = json!(sub.height);
                    if let Some(tm) = l.props.get("timeRemap") {
                        j["tm"] = self.anim(cid, l, tm, fps);
                    }
                }
                LayerKind::Footage => {
                    let Some(it) = src else { continue };
                    let ItemKind::Footage(f) = &it.kind else { continue };
                    if !f.has_video {
                        continue;
                    }
                    if !f.is_still() {
                        self.warn(format!("`{}` is moving footage; Lottie holds still images, so its first frame was used", it.name));
                    }
                    let Some(asset) = self.image_asset(it.id, f) else {
                        self.warn(format!("`{}` could not be read for the Lottie file", it.name));
                        continue;
                    };
                    j["ty"] = json!(2);
                    j["refId"] = json!(asset);
                }
                LayerKind::Shape => {
                    j["ty"] = json!(4);
                    j["shapes"] = json!(l.contents().map(|c| self.shape_items(cid, l, c, fps)).unwrap_or_default());
                }
                LayerKind::Text => {
                    let Some(doc) = l.text_prop().and_then(|p| p.value.as_text().cloned()) else { continue };
                    if self.text_shapes || crate::render::textanim::has_animation(l) {
                        if !self.text_shapes {
                            self.warn(format!("text layer `{}` has animators; it was exported as shapes", l.name));
                        }
                        let ops = crate::render::text::text_ops(&doc);
                        let mut items = vec![];
                        for op in ops {
                            let mut it = vec![];
                            for p in &op.paths {
                                let tp = p.transformed(&op.transform);
                                it.push(json!({"ty": "sh", "ks": {"a": 0, "k": path_json(&tp)}}));
                            }
                            if let crate::render::vector::Paint::Solid(c) = op.paint {
                                match op.kind {
                                    crate::render::vector::OpKind::Fill { .. } => it.push(json!({"ty": "fl", "c": {"a": 0, "k": [c[0], c[1], c[2], 1]}, "o": {"a": 0, "k": op.opacity * 100.0 * c[3]}, "r": 1})),
                                    crate::render::vector::OpKind::Stroke { width, .. } => it.push(json!({"ty": "st", "c": {"a": 0, "k": [c[0], c[1], c[2], 1]}, "o": {"a": 0, "k": op.opacity * 100.0 * c[3]}, "w": {"a": 0, "k": width}, "lc": 2, "lj": 2})),
                                }
                            }
                            it.push(json!({"ty": "tr", "p": {"a": 0, "k": [0, 0]}, "a": {"a": 0, "k": [0, 0]}, "s": {"a": 0, "k": [100, 100]}, "r": {"a": 0, "k": 0}, "o": {"a": 0, "k": 100}}));
                            items.push(json!({"ty": "gr", "it": it}));
                        }
                        j["ty"] = json!(4);
                        j["shapes"] = json!(items);
                    } else {
                        let fname = if doc.style.is_empty() { doc.font.clone() } else { format!("{}-{}", doc.font, doc.style) };
                        if !self.fonts.iter().any(|f| f["fName"] == json!(fname)) {
                            self.fonts.push(json!({"fName": fname, "fFamily": doc.font, "fStyle": if doc.style.is_empty() { "Regular" } else { &doc.style }, "ascent": 75}));
                        }
                        let docj = |d: &TextDoc| {
                            json!({"s": d.size, "f": fname, "t": d.text.replace('\n', "\r"), "j": match d.justify { Justify::Right | Justify::JustifyRight => 1, Justify::Center | Justify::JustifyCenter => 2, _ => 0 }, "tr": d.tracking, "lh": d.leading.unwrap_or(d.size * 1.2), "ls": d.baseline_shift, "fc": d.fill, "sc": d.stroke, "sw": if d.apply_stroke { d.stroke_width } else { 0.0 }, "of": d.stroke_over_fill})
                        };
                        let keys: Vec<Json> = match l.text_prop() {
                            Some(p) if !p.keys.is_empty() => p.keys.iter().filter_map(|k| k.value.as_text().map(|d| json!({"t": k.time * fps, "s": docj(d)}))).collect(),
                            _ => vec![json!({"t": 0, "s": docj(&doc)})],
                        };
                        j["ty"] = json!(5);
                        j["t"] = json!({"d": {"k": keys}, "p": {}, "m": {"g": 1, "a": {"a": 0, "k": [0, 0]}}, "a": []});
                    }
                }
                _ => {
                    self.warn(format!("{} layer `{}` was left out (Lottie players do not draw it)", l.type_name(), l.name));
                    continue;
                }
            }
            j["ks"] = self.transform(cid, l, fps);
            let masks = self.masks(cid, l, fps);
            if !masks.is_empty() {
                j["hasMask"] = json!(true);
                j["masksProperties"] = json!(masks);
            }
            if let Some(p) = l.parent.and_then(|p| ind.get(&p)) {
                j["parent"] = json!(p);
            }
            if !l.switches.video {
                j["hd"] = json!(true);
            }
            if l.effects().is_some_and(|g| g.groups().any(|e| e.enabled)) {
                self.warn(format!("effects on `{}` were left out (Lottie players draw no effects)", l.name));
            }
            built.push((l.id, j));
        }
        let mut next = comp.layers.len() + 1;
        for (id, mut j) in built.clone() {
            let Some(l) = comp.layer(id) else { continue };
            if let Some(m) = &l.track_matte
                && let Some((_, mj)) = built.iter().find(|b| b.0 == m.layer)
            {
                let mut mj = mj.clone();
                if let Some(o) = mj.as_object_mut() {
                    o.remove("hd");
                }
                mj["ind"] = json!(next);
                mj["td"] = json!(1);
                out.push(mj);
                j["tt"] = json!(match m.kind {
                    MatteKind::Alpha => 1,
                    MatteKind::AlphaInverted => 2,
                    MatteKind::Luma => 3,
                    MatteKind::LumaInverted => 4,
                });
                j["tp"] = json!(next);
                next += 1;
            }
            if mattes.contains(&id) && !l.switches.video && !comp.layers.iter().any(|x| x.parent == Some(id)) {
                continue;
            }
            out.push(j);
        }
        out
    }
}

pub fn export(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (.json or .lottie)")?.to_string();
    let cid = e.comp_id(p.get("comp"))?;
    let project = e.project.clone();
    let comp = project.comp(cid).ok_or("no composition")?;
    let name = project.item(cid).map(|i| i.name.clone()).unwrap_or_default();
    let mut ex = Ex { project: &project, media: &e.media, expr: pb(p, "includeExpressions").unwrap_or(false), text_shapes: pb(p, "textAsShapes").unwrap_or(false), assets: vec![], comps: HashMap::new(), images: HashMap::new(), fonts: vec![], warnings: vec![] };
    ex.comps.insert(cid, String::new());
    let layers = ex.layers(cid, 0);
    let fps = comp.frame_rate;
    let markers: Vec<Json> = comp.markers.iter().map(|m| json!({"cm": m.comment, "tm": m.time * fps, "dr": m.duration * fps})).collect();
    let mut root = json!({"v": "5.7.4", "fr": fps, "ip": comp.work_area[0] * fps, "op": comp.work_area[1] * fps, "w": comp.width, "h": comp.height, "nm": name, "ddd": if comp.layers.iter().any(|l| l.is_3d()) { 1 } else { 0 }, "assets": ex.assets, "layers": layers, "markers": markers, "meta": {"g": "formvfx"}});
    if !ex.fonts.is_empty() {
        root["fonts"] = json!({"list": ex.fonts});
    }
    let mut s = String::new();
    emit(&root, &mut s);
    let text = s.into_bytes();
    if let Some(dir) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if path.to_ascii_lowercase().ends_with(".lottie") {
        let manifest = json!({"version": "1", "generator": "formvfx", "author": "", "animations": [{"id": "animation", "speed": 1, "loop": true, "autoplay": true}]});
        let z = zip(&[("manifest.json", serde_json::to_vec(&manifest).unwrap_or_default()), ("animations/animation.json", text.clone())]);
        std::fs::write(&path, z).map_err(|e| format!("{path}: {e}"))?;
    } else {
        std::fs::write(&path, &text).map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(json!({"path": path, "bytes": std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0), "layers": root["layers"].as_array().map(Vec::len).unwrap_or(0), "assets": root["assets"].as_array().map(Vec::len).unwrap_or(0), "warnings": ex.warnings}))
}

#[derive(Clone, Copy)]
enum Vk {
    Num,
    V2,
    V3(f64),
    Col,
    Path,
    Grad(usize),
}

fn nums(j: &Json) -> Vec<f64> {
    match j {
        Json::Number(n) => vec![n.as_f64().unwrap_or(0.0)],
        Json::Array(a) => a.iter().filter_map(Json::as_f64).collect(),
        _ => vec![],
    }
}

fn to_path(j: &Json) -> BezPath {
    let j = match j {
        Json::Array(a) => a.first().unwrap_or(&Json::Null),
        o => o,
    };
    let pts = |k: &str| -> Vec<[f64; 2]> { j.get(k).and_then(Json::as_array).map(|a| a.iter().map(|p| { let v = nums(p); [v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0)] }).collect()).unwrap_or_default() };
    let v = pts("v");
    let mut ins = pts("i");
    let mut outs = pts("o");
    ins.resize(v.len(), [0.0; 2]);
    outs.resize(v.len(), [0.0; 2]);
    BezPath { vertices: v, ins, outs, closed: j.get("c").and_then(Json::as_bool).unwrap_or(false) }
}

fn to_grad(v: &[f64], n: usize) -> Gradient {
    let n = n.max(1);
    let mut stops: Vec<GradStop> = (0..n).filter_map(|i| v.get(i * 4..i * 4 + 4).map(|s| GradStop { pos: s[0], color: [s[1], s[2], s[3], 1.0] })).collect();
    let alpha: Vec<(f64, f64)> = v.get(n * 4..).unwrap_or(&[]).chunks_exact(2).map(|c| (c[0], c[1])).collect();
    if !alpha.is_empty() {
        for s in stops.iter_mut() {
            let a = alpha.iter().min_by(|x, y| (x.0 - s.pos).abs().total_cmp(&(y.0 - s.pos).abs())).map(|x| x.1).unwrap_or(1.0);
            s.color[3] = a;
        }
    }
    if stops.is_empty() {
        return Gradient::default();
    }
    Gradient { stops }
}

fn to_value(j: &Json, k: Vk) -> Value {
    match k {
        Vk::Path => Value::Path(to_path(j)),
        Vk::Grad(n) => Value::Gradient(to_grad(&nums(j), n)),
        _ => {
            let v = nums(j);
            let g = |i: usize, d: f64| v.get(i).copied().unwrap_or(d);
            match k {
                Vk::Num => Value::Num(g(0, 0.0)),
                Vk::V2 => Value::Vec2([g(0, 0.0), g(1, 0.0)]),
                Vk::V3(z) => Value::Vec3([g(0, 0.0), g(1, 0.0), g(2, z)]),
                _ => {
                    let big = v.iter().take(3).any(|x| *x > 1.0);
                    let s = if big { 255.0 } else { 1.0 };
                    Value::Color([g(0, 0.0) / s, g(1, 0.0) / s, g(2, 0.0) / s, 1.0])
                }
            }
        }
    }
}

fn ease_at(j: Option<&Json>, d: usize) -> Option<(f64, f64)> {
    let j = j?;
    let pick = |k: &str| -> Option<f64> {
        let v = j.get(k)?;
        match v {
            Json::Array(a) => a.get(d).or(a.first()).and_then(Json::as_f64),
            other => other.as_f64(),
        }
    };
    Some((pick("x")?, pick("y")?))
}

fn read_anim(j: Option<&Json>, k: Vk, fps: f64, spatial: bool) -> Option<(Value, Vec<Key>)> {
    let j = j?;
    let kv = j.get("k")?;
    let animated = j.get("a").and_then(Json::as_u64) == Some(1) || kv.as_array().is_some_and(|a| a.first().is_some_and(|x| x.get("t").is_some()));
    if !animated {
        return Some((to_value(kv, k), vec![]));
    }
    let list = kv.as_array()?;
    let mut keys: Vec<Key> = vec![];
    let mut prev_e: Option<&Json> = None;
    for kj in list {
        let t = kj.get("t").and_then(Json::as_f64).unwrap_or(0.0) / fps;
        let s = kj.get("s").or(prev_e)?;
        prev_e = kj.get("e");
        let mut key = Key::new(t, to_value(s, k), spatial);
        key.spatial_auto = false;
        keys.push(key);
    }
    for i in 0..keys.len().saturating_sub(1) {
        let kj = &list[i];
        if kj.get("h").and_then(Json::as_u64) == Some(1) {
            keys[i].out_interp = Interp::Hold;
            keys[i + 1].in_interp = Interp::Hold;
            continue;
        }
        let dt = (keys[i + 1].time - keys[i].time).max(1e-9);
        let spat = spatial && matches!(keys[i].value, Value::Vec2(_) | Value::Vec3(_));
        let to = kj.get("to").map(nums).unwrap_or_default();
        let ti = kj.get("ti").map(nums).unwrap_or_default();
        let a3 = |v: &[f64]| [v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)];
        if spat {
            keys[i].spatial_out = a3(&to);
            keys[i + 1].spatial_in = a3(&ti);
        }
        let spans: Vec<f64> = if spat {
            let p0 = keys[i].value.vec3();
            let p3 = keys[i + 1].value.vec3();
            let (o, n) = (a3(&to), a3(&ti));
            vec![arc_len(p0, [p0[0] + o[0], p0[1] + o[1], p0[2] + o[2]], [p3[0] + n[0], p3[1] + n[1], p3[2] + n[2]], p3)]
        } else if keys[i].value.is_numeric() {
            let (a, b) = (keys[i].value.components(), keys[i + 1].value.components());
            (0..ease_dims(&keys[i].value, false)).map(|d| b.get(d).copied().unwrap_or(0.0) - a.get(d).copied().unwrap_or(0.0)).collect()
        } else {
            vec![100.0 / dt]
        };
        let (mut outs, mut ins) = (vec![], vec![]);
        let mut linear = true;
        for (d, dy) in spans.iter().enumerate() {
            let (ox, oy) = ease_at(kj.get("o"), d).unwrap_or((0.0, 0.0));
            let (ix, iy) = ease_at(kj.get("i"), d).unwrap_or((1.0, 1.0));
            if (ox - oy).abs() > 1e-3 || (ix - iy).abs() > 1e-3 {
                linear = false;
            }
            let ox = ox.clamp(0.001, 1.0);
            let ix = ix.clamp(0.0, 0.999);
            outs.push(Ease { speed: oy * dy / (ox * dt), influence: (ox * 100.0).clamp(0.1, 100.0) });
            ins.push(Ease { speed: (1.0 - iy) * dy / ((1.0 - ix) * dt), influence: ((1.0 - ix) * 100.0).clamp(0.1, 100.0) });
        }
        if !linear {
            keys[i].out_interp = Interp::Bezier;
            keys[i + 1].in_interp = Interp::Bezier;
            keys[i].out_ease = outs;
            keys[i + 1].in_ease = ins;
        }
    }
    let v = keys.first().map(|k| k.value.clone()).unwrap_or(Value::Num(0.0));
    Some((v, keys))
}

fn set(g: &mut Group, id: &str, j: Option<&Json>, k: Vk, fps: f64) {
    let Some(p) = g.get_mut(id) else { return };
    if let Some((v, keys)) = read_anim(j, k, fps, p.spatial) {
        p.value = v;
        p.keys = keys;
    }
    if let Some(x) = j.and_then(|j| j.get("x")).and_then(Json::as_str) {
        p.expr = Some(crate::model::prop::Expr { text: x.to_string(), enabled: true });
    }
}

fn set_enum(g: &mut Group, id: &str, v: Option<u64>) {
    if let (Some(p), Some(v)) = (g.get_mut(id), v) {
        p.value = Value::Enum(v as u32);
    }
}

fn sample(j: &Json, frame: f64) -> Vec<f64> {
    let Some(kv) = j.get("k") else { return vec![] };
    let Some(list) = kv.as_array().filter(|a| a.first().is_some_and(|x| x.get("t").is_some())) else { return nums(kv) };
    let mut prev: Option<(f64, Vec<f64>)> = None;
    for (i, kj) in list.iter().enumerate() {
        let t = kj.get("t").and_then(Json::as_f64).unwrap_or(0.0);
        let s = nums(kj.get("s").unwrap_or(&Json::Null));
        if frame < t {
            let Some((t0, s0)) = prev else { return s };
            let pk = &list[i - 1];
            if pk.get("h").and_then(Json::as_u64) == Some(1) {
                return s0;
            }
            let u = (frame - t0) / (t - t0).max(1e-9);
            let (ox, oy) = ease_at(pk.get("o"), 0).unwrap_or((0.0, 0.0));
            let (ix, iy) = ease_at(pk.get("i"), 0).unwrap_or((1.0, 1.0));
            let bx = |a: f64| 3.0 * (1.0 - a) * (1.0 - a) * a * ox + 3.0 * (1.0 - a) * a * a * ix + a * a * a;
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..40 {
                let m = (lo + hi) / 2.0;
                if bx(m) < u {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            let a = (lo + hi) / 2.0;
            let y = 3.0 * (1.0 - a) * (1.0 - a) * a * oy + 3.0 * (1.0 - a) * a * a * iy + a * a * a;
            let s = if s.is_empty() { nums(pk.get("e").unwrap_or(&Json::Null)) } else { s };
            return s0.iter().zip(s.iter().chain(std::iter::repeat(&0.0))).map(|(a, b)| a + (b - a) * y).collect();
        }
        prev = Some((t, if s.is_empty() { prev.map(|p| p.1).unwrap_or_default() } else { s }));
    }
    prev.map(|p| p.1).unwrap_or_default()
}

struct Im<'a> {
    fps: f64,
    assets: HashMap<String, &'a Json>,
    made: HashMap<String, u64>,
    folder: u64,
    dir: std::path::PathBuf,
    stem: String,
    fonts: HashMap<String, (String, String)>,
    warnings: Vec<String>,
    images: usize,
}

impl Im<'_> {
    fn warn(&mut self, w: String) {
        if !self.warnings.contains(&w) {
            self.warnings.push(w);
        }
    }

    fn transform(&mut self, l: &mut Layer, ks: Option<&Json>, three: bool) {
        let Some(ks) = ks else { return };
        let fps = self.fps;
        let Some(tr) = l.props.sub_mut("transform") else { return };
        set(tr, "anchor", ks.get("a"), Vk::V3(0.0), fps);
        match ks.get("p") {
            Some(p) if p.get("s").and_then(Json::as_bool) == Some(true) => {
                let (x, y, z) = (p.get("x").cloned().unwrap_or(json!({"k": 0})), p.get("y").cloned().unwrap_or(json!({"k": 0})), p.get("z").cloned());
                let times: Vec<f64> = [&x, &y].iter().flat_map(|j| j.get("k").and_then(Json::as_array).map(|a| a.iter().filter_map(|k| k.get("t").and_then(Json::as_f64)).collect::<Vec<_>>()).unwrap_or_default()).collect();
                if let Some(pr) = tr.get_mut("position") {
                    let at = |f: f64| Value::Vec3([sample(&x, f).first().copied().unwrap_or(0.0), sample(&y, f).first().copied().unwrap_or(0.0), z.as_ref().map(|z| sample(z, f).first().copied().unwrap_or(0.0)).unwrap_or(0.0)]);
                    if times.is_empty() {
                        pr.value = at(0.0);
                    } else {
                        let (lo, hi) = (times.iter().cloned().fold(f64::MAX, f64::min), times.iter().cloned().fold(f64::MIN, f64::max));
                        pr.keys = (lo.floor() as i64..=hi.ceil() as i64).map(|f| Key::new(f as f64 / fps, at(f as f64), false)).collect();
                        for k in pr.keys.iter_mut() {
                            k.spatial_auto = false;
                        }
                        pr.value = at(lo);
                    }
                }
            }
            p => set(tr, "position", p, Vk::V3(0.0), fps),
        }
        set(tr, "scale", ks.get("s"), Vk::V3(100.0), fps);
        set(tr, "opacity", ks.get("o"), Vk::Num, fps);
        if three {
            set(tr, "rotation", ks.get("rz").or(ks.get("r")), Vk::Num, fps);
            set(tr, "rotationX", ks.get("rx"), Vk::Num, fps);
            set(tr, "rotationY", ks.get("ry"), Vk::Num, fps);
            set(tr, "orientation", ks.get("or"), Vk::V3(0.0), fps);
        } else {
            set(tr, "rotation", ks.get("r").or(ks.get("rz")), Vk::Num, fps);
        }
    }

    fn shapes(&mut self, ids: &mut crate::model::Ids, list: &[Json], into: &mut Vec<Node>) {
        let fps = self.fps;
        for s in list {
            let ty = s.get("ty").and_then(Json::as_str).unwrap_or("");
            let kind = match ty {
                "gr" => "group",
                "rc" => "rect",
                "el" => "ellipse",
                "sr" => "star",
                "sh" => "path",
                "fl" => "fill",
                "st" => "stroke",
                "gf" => "gradientFill",
                "gs" => "gradientStroke",
                "tm" => "trim",
                "rp" => "repeater",
                "rd" => "roundCorners",
                "op" => "offsetPaths",
                "mm" => "mergePaths",
                "pb" => "puckerBloat",
                "zz" => "zigZag",
                "tw" => "twist",
                "tr" => continue,
                other => {
                    self.warn(format!("Lottie shape `{other}` is not supported and was left out"));
                    continue;
                }
            };
            let name = s.get("nm").and_then(Json::as_str);
            let Some(mut g) = shape_item(ids, kind, name) else { continue };
            g.enabled = !s.get("hd").and_then(Json::as_bool).unwrap_or(false);
            let en = |k: &str| s.get(k).and_then(|v| v.as_u64().or_else(|| v.get("k").and_then(Json::as_u64)));
            match kind {
                "group" => {
                    let items = s.get("it").and_then(Json::as_array).cloned().unwrap_or_default();
                    if let Some(c) = g.sub_mut("contents") {
                        let mut kids = vec![];
                        self.shapes(ids, &items, &mut kids);
                        c.children = kids;
                    }
                    if let (Some(tr), Some(t)) = (g.sub_mut("transform"), items.iter().find(|x| x.get("ty").and_then(Json::as_str) == Some("tr"))) {
                        set(tr, "anchor", t.get("a"), Vk::V2, fps);
                        set(tr, "position", t.get("p"), Vk::V2, fps);
                        set(tr, "scale", t.get("s"), Vk::V2, fps);
                        set(tr, "rotation", t.get("r"), Vk::Num, fps);
                        set(tr, "opacity", t.get("o"), Vk::Num, fps);
                        set(tr, "skew", t.get("sk"), Vk::Num, fps);
                        set(tr, "skewAxis", t.get("sa"), Vk::Num, fps);
                    }
                }
                "rect" => {
                    set(&mut g, "size", s.get("s"), Vk::V2, fps);
                    set(&mut g, "position", s.get("p"), Vk::V2, fps);
                    set(&mut g, "roundness", s.get("r"), Vk::Num, fps);
                }
                "ellipse" => {
                    set(&mut g, "size", s.get("s"), Vk::V2, fps);
                    set(&mut g, "position", s.get("p"), Vk::V2, fps);
                }
                "star" => {
                    set_enum(&mut g, "type", en("sy").map(|v| v.saturating_sub(1)));
                    for (id, k, vk) in [("points", "pt", Vk::Num), ("position", "p", Vk::V2), ("rotation", "r", Vk::Num), ("innerRadius", "ir", Vk::Num), ("outerRadius", "or", Vk::Num), ("innerRoundness", "is", Vk::Num), ("outerRoundness", "os", Vk::Num)] {
                        set(&mut g, id, s.get(k), vk, fps);
                    }
                }
                "path" => set(&mut g, "path", s.get("ks"), Vk::Path, fps),
                "fill" => {
                    set(&mut g, "color", s.get("c"), Vk::Col, fps);
                    set(&mut g, "opacity", s.get("o"), Vk::Num, fps);
                    set_enum(&mut g, "rule", en("r").map(|v| v.saturating_sub(1)));
                }
                "stroke" | "gradientStroke" => {
                    if kind == "stroke" {
                        set(&mut g, "color", s.get("c"), Vk::Col, fps);
                    }
                    set(&mut g, "opacity", s.get("o"), Vk::Num, fps);
                    set(&mut g, "width", s.get("w"), Vk::Num, fps);
                    set_enum(&mut g, "cap", en("lc").map(|v| v.saturating_sub(1)));
                    set_enum(&mut g, "join", en("lj").map(|v| v.saturating_sub(1)));
                    if let (Some(p), Some(m)) = (g.get_mut("miter"), s.get("ml").and_then(Json::as_f64)) {
                        p.value = Value::Num(m);
                    }
                    let (mut nd, mut ng) = (0, 0);
                    for d in s.get("d").and_then(Json::as_array).cloned().unwrap_or_default() {
                        let id = match d.get("n").and_then(Json::as_str) {
                            Some("d") => {
                                nd += 1;
                                ["dash", "dash2", "dash3"].get(nd - 1).copied()
                            }
                            Some("g") => {
                                ng += 1;
                                ["gap", "gap2", "gap3"].get(ng - 1).copied()
                            }
                            Some("o") => Some("dashOffset"),
                            _ => None,
                        };
                        if let Some(id) = id {
                            set(&mut g, id, d.get("v"), Vk::Num, fps);
                        }
                    }
                    if kind == "gradientStroke" {
                        self.gradient(&mut g, s);
                    }
                }
                "gradientFill" => {
                    set(&mut g, "opacity", s.get("o"), Vk::Num, fps);
                    set_enum(&mut g, "rule", en("r").map(|v| v.saturating_sub(1)));
                    self.gradient(&mut g, s);
                }
                "trim" => {
                    set(&mut g, "start", s.get("s"), Vk::Num, fps);
                    set(&mut g, "end", s.get("e"), Vk::Num, fps);
                    set(&mut g, "offset", s.get("o"), Vk::Num, fps);
                    set_enum(&mut g, "mode", en("m").map(|v| v.saturating_sub(1)));
                }
                "repeater" => {
                    set(&mut g, "copies", s.get("c"), Vk::Num, fps);
                    set(&mut g, "offset", s.get("o"), Vk::Num, fps);
                    set_enum(&mut g, "composite", en("m").map(|v| if v == 1 { 1 } else { 0 }));
                    if let (Some(tr), Some(t)) = (g.sub_mut("transform"), s.get("tr")) {
                        for (id, k, vk) in [("anchor", "a", Vk::V2), ("position", "p", Vk::V2), ("scale", "s", Vk::V2), ("rotation", "r", Vk::Num), ("startOpacity", "so", Vk::Num), ("endOpacity", "eo", Vk::Num)] {
                            set(tr, id, t.get(k), vk, fps);
                        }
                    }
                }
                "roundCorners" => set(&mut g, "radius", s.get("r"), Vk::Num, fps),
                "offsetPaths" => {
                    set(&mut g, "amount", s.get("a"), Vk::Num, fps);
                    set_enum(&mut g, "join", en("lj").map(|v| v.saturating_sub(1)));
                    set(&mut g, "miter", s.get("ml"), Vk::Num, fps);
                }
                "mergePaths" => set_enum(&mut g, "mode", en("mm").map(|v| v.saturating_sub(1).min(4))),
                "puckerBloat" => set(&mut g, "amount", s.get("a"), Vk::Num, fps),
                "zigZag" => {
                    set(&mut g, "size", s.get("s"), Vk::Num, fps);
                    set(&mut g, "ridges", s.get("r"), Vk::Num, fps);
                    set_enum(&mut g, "points", en("pt").map(|v| v.saturating_sub(1)));
                }
                "twist" => {
                    set(&mut g, "angle", s.get("a"), Vk::Num, fps);
                    set(&mut g, "center", s.get("c"), Vk::V2, fps);
                }
                _ => {}
            }
            into.push(Node::Group(g));
        }
    }

    fn gradient(&mut self, g: &mut Group, s: &Json) {
        let fps = self.fps;
        set_enum(g, "type", s.get("t").and_then(Json::as_u64).map(|v| v.saturating_sub(1)));
        set(g, "start", s.get("s"), Vk::V2, fps);
        set(g, "end", s.get("e"), Vk::V2, fps);
        let n = s.get("g").and_then(|x| x.get("p")).and_then(Json::as_u64).unwrap_or(2) as usize;
        set(g, "colors", s.get("g").and_then(|x| x.get("k")), Vk::Grad(n), fps);
    }

    fn masks(&mut self, ids: &mut crate::model::Ids, l: &mut Layer, list: &[Json]) {
        let fps = self.fps;
        let mut groups = vec![];
        for (i, m) in list.iter().enumerate() {
            let mode = match m.get("mode").and_then(Json::as_str).unwrap_or("a") {
                "n" => MaskMode::None,
                "s" => MaskMode::Subtract,
                "i" => MaskMode::Intersect,
                "l" => MaskMode::Lighten,
                "da" => MaskMode::Darken,
                "f" => MaskMode::Difference,
                _ => MaskMode::Add,
            };
            let name = m.get("nm").and_then(Json::as_str).map(str::to_string).unwrap_or_else(|| format!("Mask {}", i + 1));
            let mut g = mask_group(ids, &name, BezPath::default(), mode, [255, 200, 64]);
            if let GroupKind::Mask { inverted, .. } = &mut g.kind {
                *inverted = m.get("inv").and_then(Json::as_bool).unwrap_or(false);
            }
            set(&mut g, "path", m.get("pt"), Vk::Path, fps);
            set(&mut g, "opacity", m.get("o"), Vk::Num, fps);
            set(&mut g, "expansion", m.get("x"), Vk::Num, fps);
            groups.push(Node::Group(g));
        }
        if let Some(ms) = l.props.sub_mut("masks") {
            ms.children = groups;
        }
    }

    fn image_item(&mut self, e: &mut Engine, a: &Json) -> Result<u64, String> {
        let id = a.get("id").and_then(Json::as_str).unwrap_or("").to_string();
        if let Some(i) = self.made.get(&id) {
            return Ok(*i);
        }
        let p = a.get("p").and_then(Json::as_str).unwrap_or("");
        let path = if let Some((_, data)) = p.split_once(";base64,") {
            let bytes = crate::render::model3d::base64(data);
            let ext = if p.starts_with("data:image/jpeg") || p.starts_with("data:image/jpg") { "jpg" } else { "png" };
            let dir = self.dir.join(format!("{} Images", self.stem));
            let dir = if std::fs::create_dir_all(&dir).is_ok() { dir } else { crate::render::diskcache::data_dir().join("lottie-images") };
            let _ = std::fs::create_dir_all(&dir);
            self.images += 1;
            let f = dir.join(format!("{}_{}.{ext}", if id.is_empty() { "image" } else { &id }, self.images));
            std::fs::write(&f, bytes).map_err(|err| format!("{}: {err}", f.display()))?;
            f.to_string_lossy().to_string()
        } else {
            let u = a.get("u").and_then(Json::as_str).unwrap_or("");
            self.dir.join(u).join(p).to_string_lossy().to_string()
        };
        let f = crate::render::media::probe(&path)?;
        let name = Path::new(&path).file_name().and_then(|n| n.to_str()).unwrap_or("image").to_string();
        let pj = e.proj();
        let nid = pj.new_id();
        pj.items.push(Item { id: nid, name, parent: Some(self.folder), label: 5, comment: String::new(), kind: ItemKind::Footage(f), proxy: None });
        self.made.insert(id, nid);
        Ok(nid)
    }

    fn comp(&mut self, e: &mut Engine, name: &str, w: u32, h: u32, dur: f64, layers: &[Json], depth: u32) -> Result<u64, String> {
        let cid = make_comp(e, name, w.max(4), h.max(4), self.fps, dur.max(1.0 / self.fps), [0.0; 3]);
        if let Some(it) = e.proj().item_mut(cid) {
            it.parent = Some(self.folder);
        }
        let fps = self.fps;
        let mut by_ind: HashMap<i64, u64> = HashMap::new();
        let mut parents: Vec<(u64, i64)> = vec![];
        let mut mattes: Vec<(u64, MatteKind, Option<i64>, usize)> = vec![];
        let mut created: Vec<(usize, u64)> = vec![];
        for (pos, lj) in layers.iter().enumerate() {
            let ty = lj.get("ty").and_then(Json::as_u64).unwrap_or(99);
            let nm = lj.get("nm").and_then(Json::as_str).unwrap_or("Layer").to_string();
            let ip = lj.get("ip").and_then(Json::as_f64).unwrap_or(0.0) / fps;
            let op = lj.get("op").and_then(Json::as_f64).unwrap_or(dur * fps) / fps;
            let st = lj.get("st").and_then(Json::as_f64).unwrap_or(0.0) / fps;
            let sr = lj.get("sr").and_then(Json::as_f64).unwrap_or(1.0);
            let three = lj.get("ddd").and_then(Json::as_u64) == Some(1);
            let (kind, source, size) = match ty {
                0 => {
                    let rid = lj.get("refId").and_then(Json::as_str).unwrap_or("");
                    let sub = match self.made.get(rid) {
                        Some(s) => *s,
                        None => {
                            let Some(a) = self.assets.get(rid).copied() else {
                                self.warn(format!("precomp `{rid}` is missing from the assets"));
                                continue;
                            };
                            if depth > 16 {
                                continue;
                            }
                            let (w, h) = (lj.get("w").and_then(Json::as_u64).unwrap_or(100) as u32, lj.get("h").and_then(Json::as_u64).unwrap_or(100) as u32);
                            let lays = a.get("layers").and_then(Json::as_array).cloned().unwrap_or_default();
                            let an = a.get("nm").and_then(Json::as_str).unwrap_or(rid).to_string();
                            let s = self.comp(e, &an, w, h, dur, &lays, depth + 1)?;
                            self.made.insert(rid.to_string(), s);
                            s
                        }
                    };
                    let c = e.project.comp(sub).map(|c| (c.width as f64, c.height as f64)).unwrap_or((100.0, 100.0));
                    (LayerKind::Precomp, Some(sub), c)
                }
                1 => {
                    let hex = lj.get("sc").and_then(Json::as_str).unwrap_or("#000000").trim_start_matches('#').to_string();
                    let c = |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f64 / 255.0;
                    let (w, h) = (lj.get("sw").and_then(Json::as_u64).unwrap_or(100) as u32, lj.get("sh").and_then(Json::as_u64).unwrap_or(100) as u32);
                    let sid = solid_item(e, &nm, [c(0), c(2), c(4)], w, h);
                    (LayerKind::Solid, Some(sid), (w as f64, h as f64))
                }
                2 => {
                    let rid = lj.get("refId").and_then(Json::as_str).unwrap_or("");
                    let Some(a) = self.assets.get(rid).copied() else { continue };
                    match self.image_item(e, a) {
                        Ok(i) => {
                            let sz = e.project.item(i).and_then(|it| it.dimensions()).map(|(w, h)| (w as f64, h as f64)).unwrap_or((100.0, 100.0));
                            (LayerKind::Footage, Some(i), sz)
                        }
                        Err(err) => {
                            self.warn(err);
                            continue;
                        }
                    }
                }
                3 => (LayerKind::Null, None, (100.0, 100.0)),
                4 => (LayerKind::Shape, None, (w as f64, h as f64)),
                5 => (LayerKind::Text, None, (w as f64, h as f64)),
                other => {
                    self.warn(format!("Lottie layer type {other} (`{nm}`) is not supported and was left out"));
                    continue;
                }
            };
            let mut l = build_layer(e.proj(), (w.max(4), h.max(4)), LayerInit { kind, name: nm.clone(), source, size, start: st, duration: (op - st).max(1.0 / fps) });
            l.in_point = ip;
            l.out_point = op;
            l.start_time = st;
            l.stretch = sr * 100.0;
            l.blend = blend_of(lj.get("bm").and_then(Json::as_u64).unwrap_or(0));
            l.switches.three_d = three;
            if lj.get("hd").and_then(Json::as_bool) == Some(true) || lj.get("td").and_then(Json::as_u64) == Some(1) {
                l.switches.video = false;
            }
            if lj.get("ao").and_then(Json::as_u64) == Some(1) {
                l.auto_orient = AutoOrient::AlongPath;
            }
            if kind == LayerKind::Null
                && let Some(a) = l.props.sub_mut("transform").and_then(|t| t.get_mut("anchor"))
            {
                a.value = Value::Vec3([0.0; 3]);
            }
            self.transform(&mut l, lj.get("ks"), three);
            {
                let mut ids = e.proj().ids();
                if let Some(shapes) = lj.get("shapes").and_then(Json::as_array)
                    && kind == LayerKind::Shape
                {
                    let mut kids = vec![];
                    self.shapes(&mut ids, shapes, &mut kids);
                    if let Some(c) = l.props.sub_mut("contents") {
                        c.children = kids;
                    }
                }
                if let Some(ms) = lj.get("masksProperties").and_then(Json::as_array) {
                    self.masks(&mut ids, &mut l, ms);
                }
            }
            if kind == LayerKind::Text {
                self.text(&mut l, lj);
            }
            if kind == LayerKind::Precomp
                && let Some(tm) = lj.get("tm")
            {
                self.warn(format!("time remapping on `{nm}` was read as its keyframes"));
                let mut ids = e.proj().ids();
                let mut p = ids.prop("timeRemap", "Time Remap", Value::Num(0.0), crate::model::Ui::slider(-100_000.0, 100_000.0, 0.0, dur.max(1.0), 2));
                if let Some((v, keys)) = read_anim(Some(tm), Vk::Num, fps, false) {
                    p.value = v;
                    p.keys = keys;
                }
                l.props.children.push(Node::Prop(p));
            }
            if lj.get("ef").and_then(Json::as_array).is_some_and(|a| !a.is_empty()) {
                self.warn(format!("effects on `{nm}` were left out"));
            }
            let id = l.id;
            let at = e.project.comp(cid).map(|c| c.layers.len()).unwrap_or(0);
            add_layer(e, cid, l, Some(at));
            created.push((pos, id));
            if let Some(ind) = lj.get("ind").and_then(Json::as_i64) {
                by_ind.insert(ind, id);
            }
            if let Some(p) = lj.get("parent").and_then(Json::as_i64) {
                parents.push((id, p));
            }
            if let Some(tt) = lj.get("tt").and_then(Json::as_u64).filter(|t| *t > 0) {
                let k = match tt {
                    2 => MatteKind::AlphaInverted,
                    3 => MatteKind::Luma,
                    4 => MatteKind::LumaInverted,
                    _ => MatteKind::Alpha,
                };
                mattes.push((id, k, lj.get("tp").and_then(Json::as_i64), created.len() - 1));
            }
        }
        let pj = e.proj();
        let Some(c) = pj.comp_mut(cid) else { return Ok(cid) };
        for (id, p) in parents {
            if let (Some(pid), Some(l)) = (by_ind.get(&p).copied(), c.layers.iter_mut().find(|l| l.id == id)) {
                l.parent = Some(pid);
            }
        }
        for (id, kind, tp, k) in mattes {
            let matte = match tp {
                Some(t) => by_ind.get(&t).copied(),
                None => k.checked_sub(1).and_then(|j| created.get(j)).map(|x| x.1),
            };
            if let (Some(m), Some(l)) = (matte, c.layers.iter_mut().find(|l| l.id == id)) {
                l.track_matte = Some(Matte { layer: m, kind });
            }
        }
        Ok(cid)
    }

    fn text(&mut self, l: &mut Layer, lj: &Json) {
        let fps = self.fps;
        let Some(t) = lj.get("t") else { return };
        let keys = t.get("d").and_then(|d| d.get("k")).and_then(Json::as_array).cloned().unwrap_or_default();
        let doc_of = |s: &Json, fonts: &HashMap<String, (String, String)>| -> TextDoc {
            let mut d = TextDoc::default();
            d.text = s.get("t").and_then(Json::as_str).unwrap_or("").replace(['\r', '\u{3}'], "\n");
            d.size = s.get("s").and_then(Json::as_f64).unwrap_or(d.size);
            if let Some(f) = s.get("f").and_then(Json::as_str) {
                let (fam, sty) = fonts.get(f).cloned().unwrap_or((f.to_string(), String::new()));
                d.font = fam;
                d.style = sty;
            }
            d.justify = match s.get("j").and_then(Json::as_u64) {
                Some(1) => Justify::Right,
                Some(2) => Justify::Center,
                _ => Justify::Left,
            };
            d.tracking = s.get("tr").and_then(Json::as_f64).unwrap_or(0.0);
            d.leading = s.get("lh").and_then(Json::as_f64);
            d.baseline_shift = s.get("ls").and_then(Json::as_f64).unwrap_or(0.0);
            let col = |k: &str| s.get(k).map(nums).filter(|v| v.len() >= 3).map(|v| [v[0], v[1], v[2]]);
            if let Some(c) = col("fc") {
                d.fill = c;
                d.apply_fill = true;
            }
            if let Some(c) = col("sc") {
                d.stroke = c;
                d.stroke_width = s.get("sw").and_then(Json::as_f64).unwrap_or(0.0);
                d.apply_stroke = d.stroke_width > 0.0;
            }
            d.stroke_over_fill = s.get("of").and_then(Json::as_bool).unwrap_or(false);
            d
        };
        let docs: Vec<(f64, TextDoc)> = keys.iter().map(|k| (k.get("t").and_then(Json::as_f64).unwrap_or(0.0) / fps, doc_of(k.get("s").unwrap_or(&Json::Null), &self.fonts))).collect();
        if t.get("a").and_then(Json::as_array).is_some_and(|a| !a.is_empty()) {
            self.warn(format!("text animators on `{}` were left out", l.name));
        }
        let Some(p) = l.props.sub_mut("text").and_then(|g| g.get_mut("sourceText")) else { return };
        if let Some((_, d)) = docs.first() {
            p.value = Value::Text(Box::new(d.clone()));
        }
        if docs.len() > 1 {
            p.keys = docs
                .into_iter()
                .map(|(t, d)| {
                    let mut k = Key::new(t, Value::Text(Box::new(d)), false);
                    k.in_interp = Interp::Hold;
                    k.out_interp = Interp::Hold;
                    k
                })
                .collect();
        }
    }
}

pub fn import(e: &mut Engine, path: &str, folder: Option<u64>) -> Result<(u64, Vec<String>), String> {
    let root = read_json(path)?;
    let fps = root.get("fr").and_then(Json::as_f64).filter(|f| *f > 0.0).unwrap_or(30.0);
    let (w, h) = (root.get("w").and_then(Json::as_u64).unwrap_or(512) as u32, root.get("h").and_then(Json::as_u64).unwrap_or(512) as u32);
    let ip = root.get("ip").and_then(Json::as_f64).unwrap_or(0.0);
    let op = root.get("op").and_then(Json::as_f64).unwrap_or(fps * 3.0);
    let stem = Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("Lottie").to_string();
    let name = root.get("nm").and_then(Json::as_str).filter(|s| !s.is_empty()).map(str::to_string).unwrap_or_else(|| stem.clone());
    let assets_list = root.get("assets").and_then(Json::as_array).cloned().unwrap_or_default();
    let assets: HashMap<String, &Json> = assets_list.iter().filter_map(|a| Some((a.get("id")?.as_str()?.to_string(), a))).collect();
    let fonts: HashMap<String, (String, String)> = root
        .get("fonts")
        .and_then(|f| f.get("list"))
        .and_then(Json::as_array)
        .map(|l| l.iter().filter_map(|f| Some((f.get("fName")?.as_str()?.to_string(), (f.get("fFamily").and_then(Json::as_str).unwrap_or("").to_string(), f.get("fStyle").and_then(Json::as_str).unwrap_or("").to_string())))).collect())
        .unwrap_or_default();
    let pj = e.proj();
    let fid = pj.new_id();
    pj.items.push(Item { id: fid, name: format!("{name} Assets"), parent: folder, label: 0, comment: String::new(), kind: ItemKind::Folder, proxy: None });
    let dir = Path::new(path).parent().map(Path::to_path_buf).unwrap_or_default();
    let mut im = Im { fps, assets, made: HashMap::new(), folder: fid, dir, stem, fonts, warnings: vec![], images: 0 };
    let layers = root.get("layers").and_then(Json::as_array).cloned().unwrap_or_default();
    let cid = im.comp(e, &name, w, h, op / fps, &layers, 0)?;
    let pj = e.proj();
    if let Some(it) = pj.item_mut(cid) {
        it.parent = folder;
    }
    if let Some(c) = pj.comp_mut(cid) {
        c.work_area = [ip / fps, op / fps];
        for m in root.get("markers").and_then(Json::as_array).cloned().unwrap_or_default() {
            c.markers.push(crate::model::Marker { time: m.get("tm").and_then(Json::as_f64).unwrap_or(0.0) / fps, duration: m.get("dr").and_then(Json::as_f64).unwrap_or(0.0) / fps, comment: m.get("cm").and_then(Json::as_str).unwrap_or("").to_string(), ..Default::default() });
        }
    }
    Ok((cid, im.warnings))
}

pub fn import_cmd(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (.json or .lottie)")?.to_string();
    let folder = e.state.project_selection.iter().copied().find(|i| e.project.item(*i).is_some_and(|it| matches!(it.kind, ItemKind::Folder)));
    let (cid, warnings) = import(e, &path, folder)?;
    for w in &warnings {
        e.toast(w, true);
    }
    super::cmd::comp::open(e, &json!({"comp": cid}))?;
    e.state.project_selection = vec![cid];
    e.set_label("Import Lottie");
    Ok(json!({"comp": cid, "warnings": warnings}))
}
