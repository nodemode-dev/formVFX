use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::image::Img;
use super::sam::{self, Sam};
use super::vision::{self, Gray};

pub struct ModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    pub licence: &'static str,
}

pub const MOBILESAM: ModelInfo = ModelInfo {
    id: "mobilesam",
    name: "MobileSAM",
    file: "mobile_sam.pt",
    url: "https://raw.githubusercontent.com/ChaoningZhang/MobileSAM/master/weights/mobile_sam.pt",
    size: 40_728_226,
    sha256: "6dbb90523a35330fedd7f1d3dfc66f995213d81b29a5ca8108dbcdd4e37d6c2f",
    licence: "Apache-2.0",
};

pub const SAM2: ModelInfo = ModelInfo {
    id: "sam2",
    name: "SAM 2.1 tiny",
    file: "sam2.1_hiera_tiny.pt",
    url: "https://dl.fbaipublicfiles.com/segment_anything_2/092824/sam2.1_hiera_tiny.pt",
    size: 156_008_466,
    sha256: "7402e0d864fa82708a20fbd15bc84245c2f26dff0eb43a4b5b93452deb34be69",
    licence: "Apache-2.0",
};

pub const MODELS: [&ModelInfo; 2] = [&MOBILESAM, &SAM2];

pub fn model_info(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().copied().find(|m| m.id == id)
}

pub fn path_of(m: &ModelInfo) -> PathBuf {
    models_dir().join(m.file)
}

pub fn installed_of(m: &ModelInfo) -> bool {
    std::fs::metadata(path_of(m)).map(|x| x.len() == m.size).unwrap_or(false)
}

pub const NEEDS_SAM2: &str = "the SAM 2 Roto Brush uses the SAM 2.1 tiny model: install it in Settings > Roto Brush (Download), or run roto.model.download {id: sam2}";

type Sam2Slot = Mutex<Option<Arc<super::sam2::Sam2>>>;

pub fn load_sam2() -> Result<Arc<super::sam2::Sam2>, String> {
    static S: OnceLock<Sam2Slot> = OnceLock::new();
    let slot = S.get_or_init(Default::default);
    let mut g = slot.lock();
    if let Some(m) = g.as_ref() {
        return Ok(m.clone());
    }
    let bytes = std::fs::read(path_of(&SAM2)).map_err(|_| NEEDS_SAM2.to_string())?;
    if sha256_hex(&bytes) != SAM2.sha256 {
        return Err("the installed SAM 2 file does not match its checksum: download it again in Settings > Roto Brush".into());
    }
    let w = super::ptload::read(&bytes)?;
    let m = Arc::new(super::sam2::Sam2::load(&w)?);
    *g = Some(m.clone());
    Ok(m)
}

pub const NEEDS: &str = "Object Matte and the MobileSAM Roto Brush use the MobileSAM model: install it in Settings > Roto Brush (Download), or run roto.model.download {id: mobilesam}";

pub fn models_dir() -> PathBuf {
    if let Ok(d) = std::env::var("FORMVFX_MODELS") {
        return PathBuf::from(d);
    }
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".formvfx").join("models")
}

pub fn model_path() -> PathBuf {
    models_dir().join(MOBILESAM.file)
}

pub fn installed() -> bool {
    std::fs::metadata(model_path()).map(|m| m.len() == MOBILESAM.size).unwrap_or(false)
}

pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v[7] = v[6];
            v[6] = v[5];
            v[5] = v[4];
            v[4] = v[3].wrapping_add(t1);
            v[3] = v[2];
            v[2] = v[1];
            v[1] = v[0];
            v[0] = t1.wrapping_add(t2);
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

type SamSlot = Mutex<Option<(PathBuf, Arc<Sam>)>>;

pub fn load_sam() -> Result<Arc<Sam>, String> {
    static S: OnceLock<SamSlot> = OnceLock::new();
    let slot = S.get_or_init(Default::default);
    let path = model_path();
    let mut g = slot.lock();
    if let Some((p, m)) = g.as_ref()
        && *p == path
    {
        return Ok(m.clone());
    }
    let bytes = std::fs::read(&path).map_err(|_| NEEDS.to_string())?;
    if sha256_hex(&bytes) != MOBILESAM.sha256 {
        return Err("the installed MobileSAM file does not match its checksum: download it again in Settings > Roto Brush".into());
    }
    let w = super::ptload::read(&bytes)?;
    let m = Arc::new(Sam::load(&w)?);
    *g = Some((path, m.clone()));
    Ok(m)
}

pub struct Embedding {
    pub data: Vec<f32>,
    pub k: f64,
    pub w: usize,
    pub h: usize,
}

fn img_key(img: &Img) -> u64 {
    let mut h: u64 = 1469598103934665603;
    let step = (img.px.len() / 8192).max(1);
    for i in (0..img.px.len()).step_by(step) {
        h ^= img.px[i].to_bits() as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h ^ ((img.w as u64) << 32) ^ img.h as u64
}

pub fn embedding(model: &Sam, img: &Img) -> Arc<Embedding> {
    static C: OnceLock<Mutex<Vec<(u64, Arc<Embedding>)>>> = OnceLock::new();
    let c = C.get_or_init(Default::default);
    let key = img_key(img);
    if let Some(e) = c.lock().iter().find(|e| e.0 == key).map(|e| e.1.clone()) {
        return e;
    }
    let (rgb, w, h, k) = sam::prepare(img);
    let e = Arc::new(Embedding { data: model.embed(&rgb, h, w), k, w, h });
    let mut g = c.lock();
    if g.len() >= 6 {
        g.remove(0);
    }
    g.push((key, e.clone()));
    e
}

pub fn low_at(low: &[f32], k: f64, x: f64, y: f64) -> f32 {
    let u = ((x + 0.5) * k / 4.0 - 0.5).clamp(0.0, 255.0);
    let v = ((y + 0.5) * k / 4.0 - 0.5).clamp(0.0, 255.0);
    let (x0, y0) = (u.floor() as usize, v.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(255), (y0 + 1).min(255));
    let (tx, ty) = ((u - x0 as f64) as f32, (v - y0 as f64) as f32);
    let g = |a: usize, b: usize| low[b * 256 + a];
    let top = g(x0, y0) + (g(x1, y0) - g(x0, y0)) * tx;
    let bot = g(x0, y1) + (g(x1, y1) - g(x0, y1)) * tx;
    top + (bot - top) * ty
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stroke {
    pub f: i64,
    pub k: u8,
    pub r: f32,
    pub p: Vec<[f32; 2]>,
}

pub fn parse_strokes(s: &str) -> Vec<Stroke> {
    serde_json::from_str(s).unwrap_or_default()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let v = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        s.push(B64[(v >> 18) as usize & 63] as char);
        s.push(B64[(v >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { B64[(v >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { B64[v as usize & 63] as char } else { '=' });
    }
    s
}

fn unb64(s: &str) -> Vec<u8> {
    let mut map = [255u8; 256];
    for (i, c) in B64.iter().enumerate() {
        map[*c as usize] = i as u8;
    }
    let mut out = vec![];
    let mut buf = 0u32;
    let mut n = 0;
    for ch in s.bytes() {
        let v = map[ch as usize];
        if v == 255 {
            continue;
        }
        buf = buf << 6 | v as u32;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((buf >> n) as u8);
        }
    }
    out
}

pub fn runs(mask: &[bool]) -> Vec<u32> {
    let mut out = vec![];
    let mut cur = false;
    let mut n = 0u32;
    for &m in mask {
        if m == cur {
            n += 1;
        } else {
            out.push(n);
            cur = m;
            n = 1;
        }
    }
    out.push(n);
    out
}

pub fn from_runs(r: &[u32], len: usize) -> Vec<bool> {
    let mut out = Vec::with_capacity(len);
    let mut cur = false;
    for n in r {
        out.extend(std::iter::repeat_n(cur, *n as usize));
        cur = !cur;
    }
    out.resize(len, false);
    out
}

pub fn encode_rle(mask: &[bool]) -> String {
    let mut bytes = vec![];
    for mut v in runs(mask) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                bytes.push(b);
                break;
            }
            bytes.push(b | 0x80);
        }
    }
    b64(&bytes)
}

pub fn decode_rle(s: &str, len: usize) -> Vec<bool> {
    let bytes = unb64(s);
    let mut r = vec![];
    let (mut v, mut shift) = (0u32, 0);
    for b in bytes {
        v |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            r.push(v);
            v = 0;
            shift = 0;
        } else {
            shift += 7;
        }
    }
    from_runs(&r, len)
}

#[derive(Clone, Debug, Default)]
pub struct Mattes {
    pub w: usize,
    pub h: usize,
    pub scale: f64,
    pub origin: [f64; 2],
    pub frames: BTreeMap<i64, String>,
}

impl Mattes {
    pub fn encode(&self) -> String {
        let mut s = format!("m1;{};{};{};{};{}", self.w, self.h, self.scale, self.origin[0], self.origin[1]);
        for (f, r) in &self.frames {
            s.push(';');
            s.push_str(&format!("{f}:{r}"));
        }
        s
    }

    pub fn decode(s: &str) -> Option<Mattes> {
        let mut it = s.split(';');
        if it.next()? != "m1" {
            return None;
        }
        let w = it.next()?.parse().ok()?;
        let h = it.next()?.parse().ok()?;
        let scale = it.next()?.parse().ok()?;
        let origin = [it.next()?.parse().ok()?, it.next()?.parse().ok()?];
        let mut frames = BTreeMap::new();
        for part in it {
            let (f, r) = part.split_once(':')?;
            frames.insert(f.parse().ok()?, r.to_string());
        }
        Some(Mattes { w, h, scale, origin, frames })
    }

    pub fn get(&self, f: i64) -> Option<Vec<bool>> {
        self.frames.get(&f).map(|r| decode_rle(r, self.w * self.h))
    }
}

pub fn parsed(s: &str) -> Option<Arc<Mattes>> {
    static C: OnceLock<Mutex<HashMap<u64, Arc<Mattes>>>> = OnceLock::new();
    use std::hash::{Hash, Hasher};
    if s.is_empty() {
        return None;
    }
    let mut hs = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hs);
    let k = hs.finish();
    let c = C.get_or_init(Default::default);
    if let Some(m) = c.lock().get(&k) {
        return Some(m.clone());
    }
    let m = Arc::new(Mattes::decode(s)?);
    let mut g = c.lock();
    if g.len() > 8 {
        g.clear();
    }
    g.insert(k, m.clone());
    Some(m)
}

pub fn labels(strokes: &[&Stroke], w: usize, h: usize, scale: f64, origin: [f64; 2]) -> Vec<u8> {
    let mut out = vec![0u8; w * h];
    for s in strokes {
        let lab = match s.k {
            0 => 1,
            1 => 2,
            _ => continue,
        };
        let r = (s.r as f64 * scale).max(1.0);
        let pts: Vec<[f64; 2]> = s.p.iter().map(|p| [(p[0] as f64 - origin[0]) * scale, (p[1] as f64 - origin[1]) * scale]).collect();
        let mut seg: Vec<([f64; 2], [f64; 2])> = pts.windows(2).map(|w| (w[0], w[1])).collect();
        if pts.len() == 1 {
            seg.push((pts[0], pts[0]));
        }
        for (a, b) in seg {
            let (x0, x1) = ((a[0].min(b[0]) - r).floor().max(0.0) as usize, ((a[0].max(b[0]) + r).ceil().max(0.0) as usize).min(w));
            let (y0, y1) = ((a[1].min(b[1]) - r).floor().max(0.0) as usize, ((a[1].max(b[1]) + r).ceil().max(0.0) as usize).min(h));
            let d = [b[0] - a[0], b[1] - a[1]];
            let l2 = (d[0] * d[0] + d[1] * d[1]).max(1e-12);
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = [x as f64 + 0.5, y as f64 + 0.5];
                    let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0);
                    let q = [a[0] + d[0] * t, a[1] + d[1] * t];
                    if (p[0] - q[0]).hypot(p[1] - q[1]) <= r {
                        out[y * w + x] = lab;
                    }
                }
            }
        }
    }
    out
}

fn rgb(img: &Img, i: usize) -> [f32; 3] {
    [img.px[i * 4], img.px[i * 4 + 1], img.px[i * 4 + 2]]
}

fn kmeans(samples: &[[f32; 3]], k: usize) -> Vec<[f32; 3]> {
    if samples.is_empty() {
        return vec![];
    }
    let k = k.min(samples.len());
    let mut c: Vec<[f32; 3]> = (0..k).map(|i| samples[i * samples.len() / k]).collect();
    for _ in 0..8 {
        let mut sum = vec![[0.0f32; 3]; k];
        let mut n = vec![0usize; k];
        for s in samples {
            let j = (0..k).min_by(|a, b| d2(s, &c[*a]).total_cmp(&d2(s, &c[*b]))).unwrap_or(0);
            for ch in 0..3 {
                sum[j][ch] += s[ch];
            }
            n[j] += 1;
        }
        for j in 0..k {
            if n[j] > 0 {
                c[j] = [sum[j][0] / n[j] as f32, sum[j][1] / n[j] as f32, sum[j][2] / n[j] as f32];
            }
        }
    }
    c
}

fn d2(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

fn nearest(c: &[[f32; 3]], p: &[f32; 3]) -> f32 {
    c.iter().map(|x| d2(x, p)).fold(f32::MAX, f32::min)
}

pub type Models = (Vec<[f32; 3]>, Vec<[f32; 3]>);

pub fn models(img: &Img, lab: &[u8], matte: Option<&[bool]>) -> Models {
    let (w, h) = (img.w, img.h);
    let n = w * h;
    let mut fgs = vec![];
    let mut bgs = vec![];
    let stride = (n / 20000).max(1);
    let inner = |i: usize, want: bool| -> bool {
        let Some(m) = matte else { return false };
        let (x, y) = (i % w, i / w);
        let r = 3;
        if x < r || y < r || x + r >= w || y + r >= h {
            return m[i] == want && !want;
        }
        [i - r, i + r, i - r * w, i + r * w, i].iter().all(|j| m[*j] == want)
    };
    for i in (0..n).step_by(stride) {
        match lab[i] {
            1 => fgs.push(rgb(img, i)),
            2 => bgs.push(rgb(img, i)),
            _ if inner(i, true) => fgs.push(rgb(img, i)),
            _ if inner(i, false) => bgs.push(rgb(img, i)),
            _ => {}
        }
    }
    for i in 0..n {
        if lab[i] == 1 && fgs.len() < 4000 {
            fgs.push(rgb(img, i));
        }
        if lab[i] == 2 && bgs.len() < 4000 {
            bgs.push(rgb(img, i));
        }
    }
    if bgs.is_empty() {
        for x in (0..w).step_by((w / 64).max(1)) {
            bgs.push(rgb(img, x));
            bgs.push(rgb(img, (h - 1) * w + x));
        }
        for y in (0..h).step_by((h / 64).max(1)) {
            bgs.push(rgb(img, y * w));
            bgs.push(rgb(img, y * w + w - 1));
        }
    }
    (kmeans(&fgs, 6), kmeans(&bgs, 6))
}

pub fn classic(img: &Img, lab: &[u8], prior: Option<&[f32]>, fixed: Option<&Models>) -> Vec<f32> {
    let (w, h) = (img.w, img.h);
    let n = w * h;
    let learned;
    let (cf, cb) = match fixed {
        Some(m) => (&m.0, &m.1),
        None => {
            let seed: Option<Vec<bool>> = prior.map(|p| p.iter().map(|v| *v >= 0.5).collect());
            learned = models(img, lab, seed.as_deref());
            (&learned.0, &learned.1)
        }
    };
    let sigma = 0.08f32;
    let data: Vec<f32> = (0..n)
        .into_par_iter()
        .map(|i| {
            let p = rgb(img, i);
            if cf.is_empty() {
                return 0.0;
            }
            let (df, db) = (nearest(cf, &p).sqrt(), nearest(cb, &p).sqrt());
            match prior {
                Some(pr) => {
                    let q = pr[i].clamp(0.15, 0.85);
                    let lf = (-df / sigma).exp();
                    let lb = (-db / sigma).exp();
                    q * lf / (q * lf + (1.0 - q) * lb + 1e-12)
                }
                None => db / (df + db + 1e-6),
            }
        })
        .collect();
    let mut beta = 0.0f64;
    let mut cnt = 0.0f64;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if x + 1 < w {
                beta += d2(&rgb(img, i), &rgb(img, i + 1)) as f64;
                cnt += 1.0;
            }
            if y + 1 < h {
                beta += d2(&rgb(img, i), &rgb(img, i + w)) as f64;
                cnt += 1.0;
            }
        }
    }
    let beta = (1.0 / (2.0 * (beta / cnt.max(1.0)).max(1e-6))) as f32;
    let wr: Vec<f32> = (0..n).map(|i| if i % w + 1 < w { (-beta * d2(&rgb(img, i), &rgb(img, i + 1))).exp() } else { 0.0 }).collect();
    let wd: Vec<f32> = (0..n).map(|i| if i + w < n { (-beta * d2(&rgb(img, i), &rgb(img, i + w))).exp() } else { 0.0 }).collect();
    let lambda = 0.08f32;
    let conf: Vec<f32> = (0..n).map(|i| if lab[i] != 0 { 1e4 } else { lambda * (0.05 + 2.0 * (data[i] - 0.5).abs()) }).collect();
    let target: Vec<f32> = (0..n).map(|i| match lab[i] {
        1 => 1.0,
        2 => 0.0,
        _ => data[i],
    }).collect();
    let apply = |x: &[f32], out: &mut [f32]| {
        out.par_iter_mut().enumerate().for_each(|(i, o)| {
            let mut s = conf[i] * x[i];
            if i % w + 1 < w {
                s += wr[i] * (x[i] - x[i + 1]);
            }
            if i % w > 0 {
                s += wr[i - 1] * (x[i] - x[i - 1]);
            }
            if i + w < n {
                s += wd[i] * (x[i] - x[i + w]);
            }
            if i >= w {
                s += wd[i - w] * (x[i] - x[i - w]);
            }
            *o = s;
        });
    };
    let b: Vec<f32> = (0..n).map(|i| conf[i] * target[i]).collect();
    let mut x = target.clone();
    let mut ax = vec![0.0f32; n];
    apply(&x, &mut ax);
    let mut r: Vec<f32> = (0..n).map(|i| b[i] - ax[i]).collect();
    let mut p = r.clone();
    let mut rr: f64 = r.iter().map(|v| (*v as f64) * (*v as f64)).sum();
    let mut ap = vec![0.0f32; n];
    for _ in 0..250 {
        if rr < 1e-10 {
            break;
        }
        apply(&p, &mut ap);
        let pap: f64 = p.iter().zip(&ap).map(|(a, b)| (*a as f64) * (*b as f64)).sum();
        if pap.abs() < 1e-30 {
            break;
        }
        let a = (rr / pap) as f32;
        x.par_iter_mut().zip(p.par_iter()).for_each(|(xv, pv)| *xv += a * pv);
        r.par_iter_mut().zip(ap.par_iter()).for_each(|(rv, av)| *rv -= a * av);
        let rn: f64 = r.iter().map(|v| (*v as f64) * (*v as f64)).sum();
        let be = (rn / rr) as f32;
        rr = rn;
        p.par_iter_mut().zip(r.par_iter()).for_each(|(pv, rv)| *pv = rv + be * *pv);
    }
    x.iter().map(|v| v.clamp(0.0, 1.0)).collect()
}

pub fn clean(soft: &[f32], lab: &[u8], w: usize, h: usize) -> Vec<bool> {
    let n = w * h;
    let on: Vec<bool> = soft.iter().map(|v| *v >= 0.5).collect();
    let mut comp = vec![u32::MAX; n];
    let mut sizes = vec![];
    let mut seeded = vec![];
    for s in 0..n {
        if !on[s] || comp[s] != u32::MAX {
            continue;
        }
        let id = sizes.len() as u32;
        let mut stack = vec![s];
        comp[s] = id;
        let mut size = 0usize;
        let mut seed = false;
        while let Some(i) = stack.pop() {
            size += 1;
            seed |= lab[i] == 1;
            let (x, y) = (i % w, i / w);
            let nb = [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)];
            for j in nb.into_iter().flatten() {
                if on[j] && comp[j] == u32::MAX {
                    comp[j] = id;
                    stack.push(j);
                }
            }
        }
        sizes.push(size);
        seeded.push(seed);
    }
    let any_seed = seeded.iter().any(|s| *s);
    let biggest = sizes.iter().copied().max().unwrap_or(0);
    let keep: Vec<bool> = (0..sizes.len()).map(|i| if any_seed { seeded[i] || sizes[i] * 4 >= biggest } else { sizes[i] * 20 >= biggest }).collect();
    let mut out: Vec<bool> = (0..n).map(|i| comp[i] != u32::MAX && keep[comp[i] as usize]).collect();
    let mut outside = vec![false; n];
    let mut stack: Vec<usize> = (0..w).flat_map(|x| [x, (h - 1) * w + x]).chain((0..h).flat_map(|y| [y * w, y * w + w - 1])).filter(|i| !out[*i]).collect();
    for &i in &stack {
        outside[i] = true;
    }
    while let Some(i) = stack.pop() {
        let (x, y) = (i % w, i / w);
        let nb = [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)];
        for j in nb.into_iter().flatten() {
            if !out[j] && !outside[j] {
                outside[j] = true;
                stack.push(j);
            }
        }
    }
    let area = out.iter().filter(|b| **b).count();
    let mut hole = vec![u32::MAX; n];
    let mut hid = 0u32;
    for s in 0..n {
        if out[s] || outside[s] || hole[s] != u32::MAX || lab[s] == 2 {
            continue;
        }
        let mut stack = vec![s];
        let mut members = vec![];
        hole[s] = hid;
        while let Some(i) = stack.pop() {
            members.push(i);
            let (x, y) = (i % w, i / w);
            let nb = [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)];
            for j in nb.into_iter().flatten() {
                if !out[j] && !outside[j] && hole[j] == u32::MAX {
                    hole[j] = hid;
                    stack.push(j);
                }
            }
        }
        if members.len() * 50 < area.max(1) && !members.iter().any(|i| lab[*i] == 2) {
            for i in members {
                out[i] = true;
            }
        }
        hid += 1;
    }
    for i in 0..n {
        if lab[i] == 1 {
            out[i] = true;
        }
        if lab[i] == 2 {
            out[i] = false;
        }
    }
    out
}

pub fn sample_points(lab: &[u8], w: usize, which: u8, max: usize) -> Vec<[f32; 2]> {
    let idx: Vec<[f32; 2]> = (0..lab.len()).filter(|i| lab[*i] == which).map(|i| [(i % w) as f32 + 0.5, (i / w) as f32 + 0.5]).collect();
    if idx.is_empty() {
        return vec![];
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &idx {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    let spacing = ((x1 - x0).max(y1 - y0) / max as f32).max(12.0);
    let mut out: Vec<[f32; 2]> = vec![];
    for p in idx {
        if out.iter().all(|q| (q[0] - p[0]).hypot(q[1] - p[1]) >= spacing) {
            out.push(p);
            if out.len() >= max {
                break;
            }
        }
    }
    out
}

fn interior_points(mask: &[bool], w: usize, h: usize, max: usize) -> Vec<[f32; 2]> {
    let mut dist = vec![0u32; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !mask[i] {
                continue;
            }
            let up = if y > 0 { dist[i - w] } else { 0 };
            let left = if x > 0 { dist[i - 1] } else { 0 };
            dist[i] = up.min(left) + 1;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            if !mask[i] {
                continue;
            }
            let dn = if y + 1 < h { dist[i + w] } else { 0 };
            let rt = if x + 1 < w { dist[i + 1] } else { 0 };
            dist[i] = dist[i].min(dn.min(rt) + 1);
        }
    }
    let mut out: Vec<[f32; 2]> = vec![];
    let mut taken = vec![false; w * h];
    for _ in 0..max {
        let best = (0..w * h).filter(|i| !taken[*i]).max_by_key(|i| dist[*i]);
        let Some(b) = best else { break };
        if dist[b] < 2 {
            break;
        }
        out.push([(b % w) as f32 + 0.5, (b / w) as f32 + 0.5]);
        let r = dist[b] as i64;
        let (bx, by) = ((b % w) as i64, (b / w) as i64);
        for y in (by - r).max(0)..(by + r).min(h as i64) {
            for x in (bx - r).max(0)..(bx + r).min(w as i64) {
                taken[y as usize * w + x as usize] = true;
            }
        }
    }
    out
}

pub fn with_sam(model: &Sam, img: &Img, lab: &[u8], prior: Option<&[f32]>) -> Vec<f32> {
    let (w, h) = (img.w, img.h);
    let emb = embedding(model, img);
    let k = emb.k;
    let to = |p: [f32; 2]| [p[0] * k as f32, p[1] * k as f32];
    let mut pts: Vec<([f32; 2], i32)> = vec![];
    for p in sample_points(lab, w, 1, 6) {
        pts.push((to(p), 1));
    }
    for p in sample_points(lab, w, 2, 6) {
        pts.push((to(p), 0));
    }
    let mut low_in: Option<Vec<f32>> = None;
    if let Some(pr) = prior {
        let m: Vec<bool> = pr.iter().map(|v| *v >= 0.5).collect();
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        for (i, on) in m.iter().enumerate() {
            if *on {
                x0 = x0.min(i % w);
                y0 = y0.min(i / w);
                x1 = x1.max(i % w);
                y1 = y1.max(i / w);
            }
        }
        if x1 >= x0 && y1 >= y0 {
            let pad = 0.08 * ((x1 - x0).max(y1 - y0) as f32);
            pts.push((to([x0 as f32 - pad, y0 as f32 - pad]), 2));
            pts.push((to([x1 as f32 + pad, y1 as f32 + pad]), 3));
            if lab.iter().all(|l| *l != 1) {
                for p in interior_points(&m, w, h, 2) {
                    pts.push((to(p), 1));
                }
            }
            let mut low = vec![-8.0f32; 256 * 256];
            for v in 0..256 {
                for u in 0..256 {
                    let (x, y) = (((u as f64 + 0.5) * 4.0 / k) as usize, ((v as f64 + 0.5) * 4.0 / k) as usize);
                    if x < w && y < h {
                        low[v * 256 + u] = (pr[y * w + x] - 0.5) * 16.0;
                    }
                }
            }
            low_in = Some(low);
        }
    }
    if pts.is_empty() {
        return vec![0.0; w * h];
    }
    let has_box = pts.iter().any(|p| p.1 == 2);
    if !has_box {
        pts.push(([0.0, 0.0], -1));
    }
    let (masks, iou) = model.decode(&emb.data, &pts, low_in.as_deref());
    let single = pts.iter().filter(|p| p.1 == 0 || p.1 == 1).count() == 1 && !has_box;
    let pick = if single { (1..4).max_by(|a, b| iou[*a].total_cmp(&iou[*b])).unwrap_or(0) } else { 0 };
    let low = &masks[pick];
    let mut out = vec![0.0f32; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, v) in row.iter_mut().enumerate() {
            *v = if low_at(low, k, x as f64, y as f64) > 0.0 { 1.0 } else { 0.0 };
        }
    });
    out
}

pub struct Preview {
    pub mask: Vec<bool>,
    pub w: usize,
    pub h: usize,
    pub score: f32,
}

pub fn preview(model: &Sam, img: &Img, point: [f32; 2]) -> Preview {
    let emb = embedding(model, img);
    let k = emb.k as f32;
    let (masks, iou) = model.decode(&emb.data, &[([point[0] * k, point[1] * k], 1), ([0.0, 0.0], -1)], None);
    let pick = (1..4).max_by(|a, b| iou[*a].total_cmp(&iou[*b])).unwrap_or(0);
    let (w, h) = (img.w, img.h);
    let mask: Vec<bool> = (0..w * h).map(|i| low_at(&masks[pick], emb.k, (i % w) as f64, (i / w) as f64) > 0.0).collect();
    Preview { mask, w, h, score: iou[pick] }
}

pub fn warp_prior(prev: &[bool], a: &Gray, b: &Gray) -> Vec<f32> {
    let flow = vision::dense_flow(b, a);
    let (w, h) = (b.w, b.h);
    (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = ((i % w) as f32, (i / w) as f32);
            let f = flow[i];
            let (sx, sy) = (x + f[0], y + f[1]);
            let (x0, y0) = (sx.floor(), sy.floor());
            let (tx, ty) = (sx - x0, sy - y0);
            let g = |xx: f32, yy: f32| -> f32 {
                let (xi, yi) = (xx as i64, yy as i64);
                if xi < 0 || yi < 0 || xi >= w as i64 || yi >= h as i64 {
                    0.0
                } else if prev[yi as usize * w + xi as usize] {
                    1.0
                } else {
                    0.0
                }
            };
            let top = g(x0, y0) + (g(x0 + 1.0, y0) - g(x0, y0)) * tx;
            let bot = g(x0, y0 + 1.0) + (g(x0 + 1.0, y0 + 1.0) - g(x0, y0 + 1.0)) * tx;
            top + (bot - top) * ty
        })
        .collect()
}

pub fn iou(a: &[bool], b: &[bool]) -> f64 {
    let (mut i, mut u) = (0usize, 0usize);
    for (x, y) in a.iter().zip(b) {
        if *x && *y {
            i += 1;
        }
        if *x || *y {
            u += 1;
        }
    }
    if u == 0 { 1.0 } else { i as f64 / u as f64 }
}

pub fn guided(mask: &[f32], guide: &[f32], w: usize, h: usize, r: usize, eps: f32) -> Vec<f32> {
    let boxf = |v: &[f32]| -> Vec<f32> {
        let mut tmp = vec![0.0f32; w * h];
        tmp.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            let src = &v[y * w..(y + 1) * w];
            let mut acc = 0.0f32;
            let mut n = 0.0f32;
            for x in 0..=r.min(w - 1) {
                acc += src[x];
                n += 1.0;
            }
            for x in 0..w {
                row[x] = acc / n;
                if x + r + 1 < w {
                    acc += src[x + r + 1];
                    n += 1.0;
                }
                if x >= r {
                    acc -= src[x - r];
                    n -= 1.0;
                }
            }
        });
        let mut out = vec![0.0f32; w * h];
        let cols: Vec<Vec<f32>> = (0..w)
            .into_par_iter()
            .map(|x| {
                let mut col = vec![0.0f32; h];
                let mut acc = 0.0f32;
                let mut n = 0.0f32;
                for y in 0..=r.min(h - 1) {
                    acc += tmp[y * w + x];
                    n += 1.0;
                }
                for y in 0..h {
                    col[y] = acc / n;
                    if y + r + 1 < h {
                        acc += tmp[(y + r + 1) * w + x];
                        n += 1.0;
                    }
                    if y >= r {
                        acc -= tmp[(y - r) * w + x];
                        n -= 1.0;
                    }
                }
                col
            })
            .collect();
        for (x, col) in cols.into_iter().enumerate() {
            for (y, v) in col.into_iter().enumerate() {
                out[y * w + x] = v;
            }
        }
        out
    };
    let mi = boxf(guide);
    let mp = boxf(mask);
    let ip: Vec<f32> = guide.iter().zip(mask).map(|(a, b)| a * b).collect();
    let ii: Vec<f32> = guide.iter().map(|a| a * a).collect();
    let mip = boxf(&ip);
    let mii = boxf(&ii);
    let a: Vec<f32> = (0..w * h).map(|i| (mip[i] - mi[i] * mp[i]) / (mii[i] - mi[i] * mi[i] + eps)).collect();
    let b: Vec<f32> = (0..w * h).map(|i| mp[i] - a[i] * mi[i]).collect();
    let ma = boxf(&a);
    let mb = boxf(&b);
    (0..w * h).map(|i| (ma[i] * guide[i] + mb[i]).clamp(0.0, 1.0)).collect()
}
