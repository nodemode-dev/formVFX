use exr::prelude::{AnyChannel, AnyChannels, Encoding, FlatImage, FlatSamples, Image, Layer, LayerAttributes, ReadChannels, ReadLayers, SmallVec, Text, Vec2, WritableImage, f16};

use super::image::Img;

pub struct Info {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<String>,
}

fn load(path: &str) -> Result<FlatImage, String> {
    exr::prelude::read().no_deep_data().largest_resolution_level().all_channels().all_layers().all_attributes().from_file(path).map_err(|e| format!("{path}: {e}"))
}

fn split(part: Option<&Text>, channel: &str) -> (String, String) {
    let (pre, last) = match channel.rsplit_once('.') {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (String::new(), channel.to_string()),
    };
    let group = match part.map(|t| t.to_string()).filter(|s| !s.is_empty()) {
        Some(p) if pre.is_empty() => p,
        Some(p) => format!("{p}.{pre}"),
        None => pre,
    };
    (group, last.to_ascii_uppercase())
}

fn groups(img: &FlatImage) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for l in &img.layer_data {
        for c in &l.channel_data.list {
            let (g, last) = split(l.attributes.layer_name.as_ref(), &c.name.to_string());
            if matches!(last.as_str(), "R" | "G" | "B" | "A" | "Y") && !out.contains(&g) {
                out.push(g);
            }
        }
    }
    out
}

pub fn info(path: &str) -> Result<Info, String> {
    let meta = exr::meta::MetaData::read_from_file(path, false).map_err(|e| format!("{path}: {e}"))?;
    let h0 = meta.headers.first().ok_or_else(|| format!("{path}: no image"))?;
    let dw = h0.shared_attributes.display_window;
    let mut layers: Vec<String> = vec![];
    for h in &meta.headers {
        for c in &h.channels.list {
            let (g, last) = split(h.own_attributes.layer_name.as_ref(), &c.name.to_string());
            if matches!(last.as_str(), "R" | "G" | "B" | "A" | "Y") && !layers.contains(&g) {
                layers.push(g);
            }
        }
    }
    Ok(Info { width: dw.size.0 as u32, height: dw.size.1 as u32, layers })
}

pub fn read(path: &str, layer: Option<&str>) -> Result<Img, String> {
    let img = load(path)?;
    let dw = img.attributes.display_window;
    let (w, h) = (dw.size.0.max(1), dw.size.1.max(1));
    let names = groups(&img);
    let want = match layer {
        Some(l) => names.iter().find(|n| n.as_str() == l).cloned().ok_or_else(|| format!("{path}: no layer `{l}` ({})", names.join(", ")))?,
        None => names.iter().find(|n| n.is_empty()).or(names.first()).cloned().ok_or_else(|| format!("{path}: no colour channels"))?,
    };
    let mut out = Img::new(w, h);
    let mut has_alpha = false;
    let has_rgb = img.layer_data.iter().any(|l| l.channel_data.list.iter().any(|c| split(l.attributes.layer_name.as_ref(), &c.name.to_string()) == (want.clone(), "R".to_string())));
    for l in &img.layer_data {
        let pos = l.attributes.layer_position;
        let (lw, lh) = (l.size.0, l.size.1);
        let ox = pos.0 as i64 - dw.position.0 as i64;
        let oy = pos.1 as i64 - dw.position.1 as i64;
        for c in &l.channel_data.list {
            let (g, last) = split(l.attributes.layer_name.as_ref(), &c.name.to_string());
            if g != want {
                continue;
            }
            let ks: &[usize] = match last.as_str() {
                "R" => &[0],
                "G" => &[1],
                "B" => &[2],
                "A" => &[3],
                "Y" => &[0, 1, 2],
                _ => continue,
            };
            if last == "Y" && has_rgb {
                continue;
            }
            if last == "A" {
                has_alpha = true;
            }
            for (i, v) in c.sample_data.values_as_f32().enumerate() {
                let (x, y) = ((i % lw) as i64 + ox, (i / lw) as i64 + oy);
                if i / lw >= lh || x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
                    continue;
                }
                let o = (y as usize * w + x as usize) * 4;
                for k in ks {
                    out.px[o + k] = if v.is_finite() { v } else { 0.0 };
                }
            }
        }
    }
    if !has_alpha {
        for p in out.px.chunks_mut(4) {
            p[3] = 1.0;
        }
    }
    Ok(out)
}

pub struct Aux {
    pub w: usize,
    pub h: usize,
    pub planes: Vec<(String, Vec<f32>)>,
    pub attrs: Vec<(String, String)>,
}

impl Aux {
    pub fn names(&self) -> Vec<String> {
        self.planes.iter().map(|p| p.0.clone()).collect()
    }

    pub fn plane(&self, name: &str) -> Option<&[f32]> {
        let n = name.trim();
        if n.is_empty() {
            return None;
        }
        self.planes.iter().find(|p| p.0 == n).or_else(|| self.planes.iter().find(|p| p.0.eq_ignore_ascii_case(n))).map(|p| p.1.as_slice())
    }

    pub fn find(&self, names: &[&str]) -> Option<&[f32]> {
        names.iter().find_map(|n| self.plane(n)).or_else(|| {
            names.iter().find_map(|n| {
                let tail = format!(".{}", n.to_ascii_lowercase());
                self.planes.iter().find(|p| p.0.to_ascii_lowercase().ends_with(&tail)).map(|p| p.1.as_slice())
            })
        })
    }

    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|a| a.0 == key).map(|a| a.1.as_str())
    }
}

pub fn read_aux(path: &str) -> Result<Aux, String> {
    let img = load(path)?;
    let dw = img.attributes.display_window;
    let (w, h) = (dw.size.0.max(1), dw.size.1.max(1));
    let mut planes: Vec<(String, Vec<f32>)> = vec![];
    let mut attrs: Vec<(String, String)> = vec![];
    let mut keep = |k: &exr::prelude::Text, v: &exr::meta::attribute::AttributeValue| {
        if let exr::meta::attribute::AttributeValue::Text(t) = v {
            attrs.push((k.to_string(), t.to_string()));
        }
    };
    for (k, v) in &img.attributes.other {
        keep(k, v);
    }
    for l in &img.layer_data {
        for (k, v) in &l.attributes.other {
            keep(k, v);
        }
    }
    for l in &img.layer_data {
        let pos = l.attributes.layer_position;
        let (lw, lh) = (l.size.0, l.size.1);
        let ox = pos.0 as i64 - dw.position.0 as i64;
        let oy = pos.1 as i64 - dw.position.1 as i64;
        let part = l.attributes.layer_name.as_ref().map(|t| t.to_string()).filter(|s| !s.is_empty());
        for c in &l.channel_data.list {
            let name = match &part {
                Some(p) => format!("{p}.{}", c.name),
                None => c.name.to_string(),
            };
            let mut plane = vec![0.0f32; w * h];
            for (i, v) in c.sample_data.values_as_f32().enumerate() {
                let (x, y) = ((i % lw) as i64 + ox, (i / lw) as i64 + oy);
                if i / lw >= lh || x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
                    continue;
                }
                plane[y as usize * w + x as usize] = v;
            }
            planes.push((name, plane));
        }
    }
    attrs.sort();
    Ok(Aux { w, h, planes, attrs })
}

pub fn write(path: &str, parts: &[(String, &Img)], half: bool) -> Result<(), String> {
    let first = parts.first().ok_or("nothing to write")?.1;
    let (w, h) = (first.w, first.h);
    let mut list: SmallVec<[AnyChannel<FlatSamples>; 4]> = SmallVec::new();
    for (name, img) in parts {
        if img.w != w || img.h != h {
            return Err("every EXR layer needs the same size".into());
        }
        for (k, c) in ["R", "G", "B", "A"].iter().enumerate() {
            let full = if name.is_empty() { c.to_string() } else { format!("{name}.{c}") };
            let vals = img.px.chunks(4).map(|p| p[k]);
            let data = if half { FlatSamples::F16(vals.map(f16::from_f32).collect()) } else { FlatSamples::F32(vals.collect()) };
            list.push(AnyChannel::new(full.as_str(), data));
        }
    }
    let layer = Layer::new(Vec2(w, h), LayerAttributes::default(), Encoding::SMALL_LOSSLESS, AnyChannels::sort(list));
    let image = Image::from_layer(layer);
    image.write().to_file(path).map_err(|e| format!("{path}: {e}"))
}
