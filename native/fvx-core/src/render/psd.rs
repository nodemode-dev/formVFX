use std::io::Write;

use super::image::Img;
use crate::model::BlendMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Pixel,
    Group,
    End,
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub index: usize,
    pub name: String,
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub blend: [u8; 4],
    pub opacity: u8,
    pub clipping: bool,
    pub visible: bool,
    pub kind: Kind,
    pub parent: Option<usize>,
    chans: Vec<(i16, Vec<f32>)>,
    mask: Option<Mask>,
}

#[derive(Clone, Debug)]
struct Mask {
    top: i32,
    left: i32,
    w: usize,
    h: usize,
    default: f32,
}

#[derive(Clone, Debug)]
pub struct Doc {
    pub width: usize,
    pub height: usize,
    pub depth: u16,
    pub mode: u16,
    pub layers: Vec<Layer>,
    merged: Vec<Vec<f32>>,
    merged_alpha: bool,
}

struct R<'a> {
    b: &'a [u8],
    i: usize,
    big: bool,
}

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.b.get(self.i..self.i.checked_add(n).ok_or("truncated")?).ok_or("the PSD file is truncated")?;
        self.i += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(self.u16()? as i16)
    }
    fn u32(&mut self) -> Result<u32, String> {
        let s = self.take(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()? as i32)
    }
    fn u64(&mut self) -> Result<u64, String> {
        let s = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        Ok(u64::from_be_bytes(a))
    }
    fn len(&mut self) -> Result<usize, String> {
        Ok(if self.big { self.u64()? as usize } else { self.u32()? as usize })
    }
}

pub fn is_psd(b: &[u8]) -> bool {
    b.len() > 26 && &b[0..4] == b"8BPS"
}

fn unpack_bits(src: &[u8], n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n);
    let mut i = 0;
    while out.len() < n && i < src.len() {
        let h = src[i] as i8;
        i += 1;
        if h >= 0 {
            let c = h as usize + 1;
            let end = (i + c).min(src.len());
            out.extend_from_slice(&src[i..end]);
            i = end;
        } else if h != -128 {
            let c = 1 - h as isize;
            if let Some(&v) = src.get(i) {
                out.extend(std::iter::repeat_n(v, c as usize));
            }
            i += 1;
        }
    }
    out.resize(n, 0);
    out
}

fn pack_bits(src: &[u8], out: &mut Vec<u8>) {
    let n = src.len();
    let mut i = 0;
    while i < n {
        let mut run = 1;
        while i + run < n && run < 128 && src[i + run] == src[i] {
            run += 1;
        }
        if run >= 2 {
            out.push((1i16 - run as i16) as i8 as u8);
            out.push(src[i]);
            i += run;
            continue;
        }
        let start = i;
        let mut lit = 0;
        while i < n && lit < 128 {
            if i + 2 < n && src[i] == src[i + 1] && src[i] == src[i + 2] {
                break;
            }
            i += 1;
            lit += 1;
        }
        out.push((lit - 1) as u8);
        out.extend_from_slice(&src[start..start + lit]);
    }
}

fn inflate(src: &[u8]) -> Result<Vec<u8>, String> {
    miniz_oxide::inflate::decompress_to_vec_zlib(src).map_err(|e| format!("the PSD file's zip data: {e:?}"))
}

fn samples(raw: &[u8], depth: u16, n: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; n];
    match depth {
        8 => {
            for (o, b) in v.iter_mut().zip(raw) {
                *o = *b as f32 / 255.0;
            }
        }
        16 => {
            for (o, b) in v.iter_mut().zip(raw.chunks_exact(2)) {
                *o = u16::from_be_bytes([b[0], b[1]]) as f32 / 65535.0;
            }
        }
        32 => {
            for (o, b) in v.iter_mut().zip(raw.chunks_exact(4)) {
                *o = f32::from_be_bytes([b[0], b[1], b[2], b[3]]);
            }
        }
        _ => {
            for (k, o) in v.iter_mut().enumerate() {
                let byte = raw.get(k / 8).copied().unwrap_or(0);
                *o = if byte & (0x80 >> (k % 8)) != 0 { 0.0 } else { 1.0 };
            }
        }
    }
    v
}

fn unpredict(buf: &mut [u8], w: usize, h: usize, depth: u16) {
    match depth {
        8 => {
            for row in buf.chunks_mut(w.max(1)).take(h) {
                for x in 1..row.len() {
                    row[x] = row[x].wrapping_add(row[x - 1]);
                }
            }
        }
        16 => {
            for row in buf.chunks_mut((w * 2).max(1)).take(h) {
                let mut prev = 0u16;
                for (x, c) in row.chunks_exact_mut(2).enumerate() {
                    let mut v = u16::from_be_bytes([c[0], c[1]]);
                    if x > 0 {
                        v = v.wrapping_add(prev);
                    }
                    prev = v;
                    c.copy_from_slice(&v.to_be_bytes());
                }
            }
        }
        32 => {
            for row in buf.chunks_mut((w * 4).max(1)).take(h) {
                for x in 1..row.len() {
                    row[x] = row[x].wrapping_add(row[x - 1]);
                }
                let planar = row.to_vec();
                for x in 0..w {
                    for k in 0..4 {
                        row[x * 4 + k] = planar.get(k * w + x).copied().unwrap_or(0);
                    }
                }
            }
        }
        _ => {}
    }
}

fn row_bytes(w: usize, depth: u16) -> usize {
    if depth == 1 { w.div_ceil(8) } else { w * (depth as usize / 8) }
}

fn channel(r: &mut R, end: usize, w: usize, h: usize, depth: u16) -> Result<Vec<f32>, String> {
    let n = w * h;
    if end < r.i + 2 {
        r.i = end;
        return Ok(vec![0.0; n]);
    }
    let comp = r.u16()?;
    let rb = row_bytes(w, depth);
    let body = r.b.get(r.i..end.min(r.b.len())).ok_or("the PSD file is truncated")?;
    r.i = end;
    let raw = match comp {
        0 => body.to_vec(),
        1 => {
            let cw = if r.big { 4 } else { 2 };
            let mut counts = Vec::with_capacity(h);
            for y in 0..h {
                let c = body.get(y * cw..y * cw + cw).ok_or("the PSD file's RLE table is truncated")?;
                counts.push(if cw == 4 { u32::from_be_bytes([c[0], c[1], c[2], c[3]]) as usize } else { u16::from_be_bytes([c[0], c[1]]) as usize });
            }
            let mut p = h * cw;
            let mut out = Vec::with_capacity(rb * h);
            for c in counts {
                let s = body.get(p..(p + c).min(body.len())).unwrap_or(&[]);
                out.extend(unpack_bits(s, rb));
                p += c;
            }
            out
        }
        2 | 3 => {
            let mut v = inflate(body)?;
            v.resize(rb * h, 0);
            if comp == 3 {
                unpredict(&mut v, w, h, depth);
            }
            v
        }
        _ => return Err(format!("unknown PSD compression {comp}")),
    };
    Ok(samples(&raw, depth, n))
}

fn layer_info(r: &mut R, end: usize, depth: u16) -> Result<(Vec<Layer>, bool), String> {
    let signed = r.i16()?;
    let count = signed.unsigned_abs() as usize;
    let mut layers = vec![];
    let mut sizes = vec![];
    for index in 0..count {
        let (top, left, bottom, right) = (r.i32()?, r.i32()?, r.i32()?, r.i32()?);
        let nch = r.u16()? as usize;
        let mut chs = vec![];
        for _ in 0..nch {
            let id = r.i16()?;
            let len = r.len()?;
            chs.push((id, len));
        }
        r.take(4)?;
        let mut blend = [0u8; 4];
        blend.copy_from_slice(r.take(4)?);
        let opacity = r.u8()?;
        let clipping = r.u8()? != 0;
        let flags = r.u8()?;
        r.u8()?;
        let extra = r.u32()? as usize;
        let xend = r.i + extra;
        let mlen = r.u32()? as usize;
        let mut mask = None;
        if mlen >= 18 {
            let mut m = R { b: r.take(mlen)?, i: 0, big: false };
            let (t, l, b, rr) = (m.i32()?, m.i32()?, m.i32()?, m.i32()?);
            let def = m.u8()?;
            let mflags = m.u8()?;
            if mflags & 2 == 0 && b > t && rr > l {
                mask = Some(Mask { top: t, left: l, w: (rr - l) as usize, h: (b - t) as usize, default: def as f32 / 255.0 });
            }
        } else {
            r.take(mlen)?;
        }
        let blen = r.u32()? as usize;
        r.take(blen)?;
        let nl = r.u8()? as usize;
        let name_bytes = r.take(nl)?;
        let mut name: String = name_bytes.iter().map(|b| *b as char).collect();
        let pad = (4 - (nl + 1) % 4) % 4;
        r.take(pad.min(xend.saturating_sub(r.i)))?;
        let mut kind = Kind::Pixel;
        while r.i + 12 <= xend {
            let sig = r.take(4)?;
            if sig != b"8BIM" && sig != b"8B64" {
                break;
            }
            let mut key = [0u8; 4];
            key.copy_from_slice(r.take(4)?);
            let long = r.big && matches!(&key, b"LMsk" | b"Lr16" | b"Lr32" | b"Layr" | b"Mt16" | b"Mt32" | b"Mtrn" | b"Alph" | b"FMsk" | b"lnk2" | b"FEid" | b"FXid" | b"PxSD");
            let len = if long { r.u64()? as usize } else { r.u32()? as usize };
            let data = r.take(len.min(xend.saturating_sub(r.i)))?;
            match &key {
                b"luni" if data.len() >= 4 => {
                    let n = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
                    let u: Vec<u16> = data[4..].chunks_exact(2).take(n).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                    name = String::from_utf16_lossy(&u).trim_end_matches('\0').to_string();
                }
                b"lsct" | b"lsdk" if data.len() >= 4 => {
                    kind = match u32::from_be_bytes([data[0], data[1], data[2], data[3]]) {
                        1 | 2 => Kind::Group,
                        3 => Kind::End,
                        _ => kind,
                    };
                    if data.len() >= 12 && &data[4..8] == b"8BIM" {
                        blend.copy_from_slice(&data[8..12]);
                    }
                }
                _ => {}
            }
        }
        r.i = xend;
        sizes.push(chs);
        layers.push(Layer { index, name, top, left, bottom, right, blend, opacity, clipping, visible: flags & 2 == 0, kind, parent: None, chans: vec![], mask });
    }
    for (l, chs) in layers.iter_mut().zip(sizes) {
        let (w, h) = ((l.right - l.left).max(0) as usize, (l.bottom - l.top).max(0) as usize);
        for (id, len) in chs {
            let cend = (r.i + len).min(end);
            let (cw, chh) = match (&l.mask, id) {
                (Some(m), -2) => (m.w, m.h),
                _ => (w, h),
            };
            let data = channel(r, cend, cw, chh, depth)?;
            l.chans.push((id, data));
        }
    }
    let mut stack: Vec<usize> = vec![];
    for i in (0..layers.len()).rev() {
        layers[i].parent = stack.last().copied();
        match layers[i].kind {
            Kind::Group => stack.push(i),
            Kind::End => {
                stack.pop();
            }
            Kind::Pixel => {}
        }
    }
    Ok((layers, signed < 0))
}

pub fn parse(b: &[u8]) -> Result<Doc, String> {
    if !is_psd(b) {
        return Err("not a PSD file".into());
    }
    let mut r = R { b, i: 4, big: false };
    let version = r.u16()?;
    r.big = version == 2;
    r.take(6)?;
    let channels = r.u16()? as usize;
    let height = r.u32()? as usize;
    let width = r.u32()? as usize;
    let depth = r.u16()?;
    let mode = r.u16()?;
    if !matches!(mode, 1 | 3 | 0) {
        return Err(format!("PSD colour mode {mode} is not supported (RGB, Grayscale and Bitmap are)"));
    }
    let cm = r.u32()? as usize;
    r.take(cm)?;
    let ir = r.u32()? as usize;
    r.take(ir)?;
    let lm = r.len()?;
    let lm_end = r.i + lm;
    let mut layers = vec![];
    let mut merged_alpha = false;
    if lm > 0 {
        let li = r.len()?;
        let li_end = r.i + li;
        if li > 0 {
            (layers, merged_alpha) = layer_info(&mut r, li_end, depth)?;
        }
        r.i = li_end;
        if r.i + 4 <= lm_end {
            let g = r.u32()? as usize;
            r.i += g;
        }
        while layers.is_empty() && r.i + 12 <= lm_end {
            let sig = r.take(4)?;
            if sig != b"8BIM" && sig != b"8B64" {
                break;
            }
            let mut key = [0u8; 4];
            key.copy_from_slice(r.take(4)?);
            let long = r.big && matches!(&key, b"LMsk" | b"Lr16" | b"Lr32" | b"Layr" | b"Mt16" | b"Mt32" | b"Mtrn" | b"Alph" | b"FMsk" | b"lnk2" | b"FEid" | b"FXid" | b"PxSD");
            let len = if long { r.u64()? as usize } else { r.u32()? as usize };
            let end = r.i + len;
            if matches!(&key, b"Lr16" | b"Lr32" | b"Layr") {
                (layers, merged_alpha) = layer_info(&mut r, end, depth)?;
                merged_alpha = merged_alpha || !layers.is_empty();
            }
            r.i = end.div_ceil(2) * 2;
        }
    }
    r.i = lm_end;
    let mut merged = vec![];
    if r.i + 2 <= b.len() {
        let comp = r.u16()?;
        let rb = row_bytes(width, depth);
        let planes = channels.min(4);
        match comp {
            0 => {
                for _ in 0..planes {
                    let raw = r.take(rb * height).unwrap_or(&[]);
                    merged.push(samples(raw, depth, width * height));
                }
            }
            1 => {
                let cw = if r.big { 4 } else { 2 };
                let mut counts = vec![];
                for _ in 0..channels * height {
                    counts.push(if cw == 4 { r.u32()? as usize } else { r.u16()? as usize });
                }
                for c in 0..planes {
                    let mut raw = Vec::with_capacity(rb * height);
                    for y in 0..height {
                        let n = counts[c * height + y];
                        let s = r.take(n).unwrap_or(&[]);
                        raw.extend(unpack_bits(s, rb));
                    }
                    merged.push(samples(&raw, depth, width * height));
                }
            }
            _ => {
                let rest = r.b.get(r.i..).unwrap_or(&[]);
                let mut v = inflate(rest)?;
                v.resize(rb * height * planes, 0);
                for c in 0..planes {
                    let mut plane = v[c * rb * height..(c + 1) * rb * height].to_vec();
                    if comp == 3 {
                        unpredict(&mut plane, width, height, depth);
                    }
                    merged.push(samples(&plane, depth, width * height));
                }
            }
        }
    }
    Ok(Doc { width, height, depth, mode, layers, merged, merged_alpha })
}

pub fn open(path: &str) -> Result<Doc, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    parse(&b).map_err(|e| format!("{path}: {e}"))
}

impl Doc {
    fn gray(&self) -> bool {
        self.mode != 3
    }

    fn pixel(&self, ch: &[(i16, Vec<f32>)], k: usize, i: usize) -> Option<f32> {
        let id = if self.gray() { 0 } else { k as i16 };
        ch.iter().find(|c| c.0 == id).and_then(|c| c.1.get(i).copied())
    }

    pub fn merged(&self) -> Img {
        let (w, h) = (self.width, self.height);
        let mut out = Img::new(w, h);
        if self.merged.is_empty() {
            for (i, l) in self.layers.iter().enumerate() {
                if l.kind == Kind::Pixel && l.visible && self.ancestors_visible(i) {
                    let (img, x0, y0) = self.layer(i);
                    over(&mut out, &img, x0, y0, l.opacity as f32 / 255.0);
                }
            }
            return out;
        }
        let gray = self.gray() || self.merged.len() < 3;
        let alpha = if gray { self.merged.get(1) } else { self.merged.get(3) };
        let alpha = alpha.filter(|_| self.merged_alpha);
        for i in 0..w * h {
            let a = alpha.and_then(|p| p.get(i).copied()).unwrap_or(1.0);
            let a = if self.depth == 32 { a } else { a.clamp(0.0, 1.0) };
            for k in 0..3 {
                let c = if gray { self.merged[0][i] } else { self.merged[k][i] };
                out.px[i * 4 + k] = c * a;
            }
            out.px[i * 4 + 3] = a;
        }
        out
    }

    fn ancestors_visible(&self, i: usize) -> bool {
        let mut p = self.layers[i].parent;
        while let Some(j) = p {
            if !self.layers[j].visible {
                return false;
            }
            p = self.layers[j].parent;
        }
        true
    }

    pub fn layer(&self, i: usize) -> (Img, i32, i32) {
        let l = &self.layers[i];
        let (w, h) = ((l.right - l.left).max(0) as usize, (l.bottom - l.top).max(0) as usize);
        let mut out = Img::new(w.max(1), h.max(1));
        if w == 0 || h == 0 {
            return (out, l.left, l.top);
        }
        let alpha = l.chans.iter().find(|c| c.0 == -1);
        let umask = l.chans.iter().find(|c| c.0 == -2);
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let mut a = alpha.and_then(|c| c.1.get(i).copied()).unwrap_or(1.0).clamp(0.0, 1.0);
                if let (Some(m), Some(mc)) = (&l.mask, umask) {
                    let (mx, my) = (x as i32 + l.left - m.left, y as i32 + l.top - m.top);
                    let mv = if mx >= 0 && my >= 0 && (mx as usize) < m.w && (my as usize) < m.h { mc.1.get(my as usize * m.w + mx as usize).copied().unwrap_or(m.default) } else { m.default };
                    a *= mv;
                }
                for k in 0..3 {
                    out.px[i * 4 + k] = self.pixel(&l.chans, k, i).unwrap_or(0.0) * a;
                }
                out.px[i * 4 + 3] = a;
            }
        }
        (out, l.left, l.top)
    }

    pub fn layer_canvas(&self, i: usize) -> Img {
        let (img, x0, y0) = self.layer(i);
        let mut out = Img::new(self.width.max(1), self.height.max(1));
        over(&mut out, &img, x0, y0, 1.0);
        out
    }

    pub fn find(&self, sel: &serde_json::Value) -> Option<usize> {
        let pixel: Vec<usize> = (0..self.layers.len()).filter(|i| self.layers[*i].kind == Kind::Pixel).collect();
        match sel {
            serde_json::Value::Number(n) => n.as_u64().and_then(|k| pixel.iter().copied().find(|i| *i == k as usize)),
            serde_json::Value::String(s) => pixel.iter().copied().find(|i| self.layers[*i].name == *s).or_else(|| s.parse::<usize>().ok().filter(|k| pixel.contains(k))),
            _ => None,
        }
    }
}

fn over(dst: &mut Img, src: &Img, x0: i32, y0: i32, opacity: f32) {
    for y in 0..src.h {
        let dy = y as i32 + y0;
        if dy < 0 || dy as usize >= dst.h {
            continue;
        }
        for x in 0..src.w {
            let dx = x as i32 + x0;
            if dx < 0 || dx as usize >= dst.w {
                continue;
            }
            let s = (y * src.w + x) * 4;
            let d = (dy as usize * dst.w + dx as usize) * 4;
            let a = src.px[s + 3] * opacity;
            for k in 0..3 {
                dst.px[d + k] = src.px[s + k] * opacity + dst.px[d + k] * (1.0 - a);
            }
            dst.px[d + 3] = a + dst.px[d + 3] * (1.0 - a);
        }
    }
}

const KEYS: [(&[u8; 4], BlendMode); 28] = [
    (b"norm", BlendMode::Normal),
    (b"diss", BlendMode::Dissolve),
    (b"dark", BlendMode::Darken),
    (b"mul ", BlendMode::Multiply),
    (b"idiv", BlendMode::ColorBurn),
    (b"lbrn", BlendMode::LinearBurn),
    (b"dkCl", BlendMode::DarkerColor),
    (b"lite", BlendMode::Lighten),
    (b"scrn", BlendMode::Screen),
    (b"div ", BlendMode::ColorDodge),
    (b"lddg", BlendMode::LinearDodge),
    (b"lgCl", BlendMode::LighterColor),
    (b"over", BlendMode::Overlay),
    (b"sLit", BlendMode::SoftLight),
    (b"hLit", BlendMode::HardLight),
    (b"vLit", BlendMode::VividLight),
    (b"lLit", BlendMode::LinearLight),
    (b"pLit", BlendMode::PinLight),
    (b"hMix", BlendMode::HardMix),
    (b"diff", BlendMode::Difference),
    (b"smud", BlendMode::Exclusion),
    (b"fsub", BlendMode::Subtract),
    (b"fdiv", BlendMode::Divide),
    (b"hue ", BlendMode::Hue),
    (b"sat ", BlendMode::Saturation),
    (b"colr", BlendMode::Color),
    (b"lum ", BlendMode::Luminosity),
    (b"pass", BlendMode::Normal),
];

pub fn blend_of(key: &[u8; 4]) -> BlendMode {
    KEYS.iter().find(|k| k.0 == key).map(|k| k.1).unwrap_or(BlendMode::Normal)
}

pub fn key_of(m: BlendMode) -> [u8; 4] {
    let m = match m {
        BlendMode::Add => BlendMode::LinearDodge,
        BlendMode::ClassicColorBurn => BlendMode::ColorBurn,
        BlendMode::ClassicColorDodge => BlendMode::ColorDodge,
        BlendMode::ClassicDifference => BlendMode::Difference,
        BlendMode::DancingDissolve => BlendMode::Dissolve,
        other => other,
    };
    *KEYS.iter().find(|k| k.1 == m).map(|k| k.0).unwrap_or(b"norm")
}

pub struct OutLayer<'a> {
    pub name: String,
    pub img: &'a Img,
    pub blend: BlendMode,
    pub opacity: f32,
    pub visible: bool,
}

fn straight(img: &Img) -> [Vec<u8>; 4] {
    let n = img.w * img.h;
    let mut ch = [vec![0u8; n], vec![0u8; n], vec![0u8; n], vec![0u8; n]];
    for i in 0..n {
        let p = &img.px[i * 4..i * 4 + 4];
        let a = p[3].clamp(0.0, 1.0);
        for k in 0..3 {
            ch[k][i] = if a > 1e-6 { super::image::to8(p[k] / a) } else { 0 };
        }
        ch[3][i] = super::image::to8(a);
    }
    ch
}

fn rle(plane: &[u8], w: usize, h: usize) -> (Vec<u8>, Vec<u8>) {
    let mut counts = Vec::with_capacity(h * 2);
    let mut data = vec![];
    for y in 0..h {
        let before = data.len();
        pack_bits(&plane[y * w..(y + 1) * w], &mut data);
        counts.extend_from_slice(&((data.len() - before) as u16).to_be_bytes());
    }
    (counts, data)
}

fn bounds(img: &Img) -> Option<(usize, usize, usize, usize)> {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for y in 0..img.h {
        for x in 0..img.w {
            if img.px[(y * img.w + x) * 4 + 3] > 0.5 / 255.0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x1 > x0).then_some((x0, y0, x1, y1))
}

pub fn write(path: &str, merged: &Img, layers: &[OutLayer]) -> Result<(), String> {
    let (w, h) = (merged.w, merged.h);
    let mut out: Vec<u8> = vec![];
    out.extend_from_slice(b"8BPS");
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&[0; 6]);
    out.extend_from_slice(&4u16.to_be_bytes());
    out.extend_from_slice(&(h as u32).to_be_bytes());
    out.extend_from_slice(&(w as u32).to_be_bytes());
    out.extend_from_slice(&8u16.to_be_bytes());
    out.extend_from_slice(&3u16.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    let mut records = vec![];
    let mut pixels = vec![];
    for l in layers {
        let (x0, y0, x1, y1) = bounds(l.img).unwrap_or((0, 0, 0, 0));
        let (lw, lh) = (x1 - x0, y1 - y0);
        let crop = if lw > 0 { l.img.crop(x0 as isize, y0 as isize, lw, lh) } else { Img::new(1, 1) };
        let planes = straight(&crop);
        let mut chans = vec![];
        for (id, k) in [(-1i16, 3usize), (0, 0), (1, 1), (2, 2)] {
            let mut data = vec![];
            if lw > 0 {
                data.extend_from_slice(&1u16.to_be_bytes());
                let (counts, body) = rle(&planes[k], lw, lh);
                data.extend(counts);
                data.extend(body);
            } else {
                data.extend_from_slice(&0u16.to_be_bytes());
            }
            chans.push((id, data));
        }
        records.extend_from_slice(&(y0 as i32).to_be_bytes());
        records.extend_from_slice(&(x0 as i32).to_be_bytes());
        records.extend_from_slice(&(y1 as i32).to_be_bytes());
        records.extend_from_slice(&(x1 as i32).to_be_bytes());
        records.extend_from_slice(&4u16.to_be_bytes());
        for (id, data) in &chans {
            records.extend_from_slice(&id.to_be_bytes());
            records.extend_from_slice(&(data.len() as u32).to_be_bytes());
        }
        records.extend_from_slice(b"8BIM");
        records.extend_from_slice(&key_of(l.blend));
        records.push((l.opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
        records.push(0);
        records.push(if l.visible { 8 } else { 10 });
        records.push(0);
        let mut extra = vec![];
        extra.extend_from_slice(&0u32.to_be_bytes());
        extra.extend_from_slice(&0u32.to_be_bytes());
        let ascii: Vec<u8> = l.name.chars().map(|c| if c.is_ascii() { c as u8 } else { b'_' }).take(255).collect();
        extra.push(ascii.len() as u8);
        extra.extend_from_slice(&ascii);
        while extra.len() % 4 != 0 {
            extra.push(0);
        }
        let u: Vec<u16> = l.name.encode_utf16().collect();
        let mut luni = (u.len() as u32).to_be_bytes().to_vec();
        for c in &u {
            luni.extend_from_slice(&c.to_be_bytes());
        }
        while luni.len() % 4 != 0 {
            luni.push(0);
        }
        extra.extend_from_slice(b"8BIMluni");
        extra.extend_from_slice(&(luni.len() as u32).to_be_bytes());
        extra.extend(luni);
        records.extend_from_slice(&(extra.len() as u32).to_be_bytes());
        records.extend(extra);
        for (_, data) in chans {
            pixels.extend(data);
        }
    }
    let mut info = vec![];
    info.extend_from_slice(&(-(layers.len() as i16)).to_be_bytes());
    info.extend(records);
    info.extend(pixels);
    if info.len() % 2 == 1 {
        info.push(0);
    }
    let lm_len = 4 + info.len() + 4;
    out.extend_from_slice(&(lm_len as u32).to_be_bytes());
    out.extend_from_slice(&(info.len() as u32).to_be_bytes());
    out.extend(info);
    out.extend_from_slice(&0u32.to_be_bytes());
    let planes = straight(merged);
    let mut counts = vec![];
    let mut body = vec![];
    for k in 0..4 {
        let (c, b) = rle(&planes[k], w, h);
        counts.extend(c);
        body.extend(b);
    }
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend(counts);
    out.extend(body);
    let mut f = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    f.write_all(&out).map_err(|e| format!("{path}: {e}"))
}
