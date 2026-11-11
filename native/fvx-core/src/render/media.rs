use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::Value as Json;

use super::image::Img;
use crate::model::{AlphaMode, Footage, FootageKind};

pub const VIDEO_EXT: [&str; 12] = ["mp4", "mov", "m4v", "mkv", "webm", "avi", "mxf", "mpg", "mpeg", "wmv", "flv", "ts"];
pub const AUDIO_EXT: [&str; 10] = ["wav", "aif", "aiff", "mp3", "aac", "m4a", "flac", "ogg", "opus", "wma"];
pub const IMAGE_EXT: [&str; 12] = ["png", "jpg", "jpeg", "gif", "webp", "tif", "tiff", "bmp", "tga", "exr", "psd", "psb"];
pub const MODEL_EXT: [&str; 3] = ["gltf", "glb", "obj"];

pub fn is_media(path: &str) -> bool {
    let e = ext(path);
    VIDEO_EXT.contains(&e.as_str()) || AUDIO_EXT.contains(&e.as_str()) || IMAGE_EXT.contains(&e.as_str()) || MODEL_EXT.contains(&e.as_str()) || super::pdf::EXT.contains(&e.as_str()) || e == "svg"
}

fn split_number(stem: &str) -> Option<(&str, &str)> {
    let digits = stem.bytes().rev().take_while(u8::is_ascii_digit).count();
    (digits > 0).then(|| stem.split_at(stem.len() - digits))
}

pub fn sequence_files(path: &str) -> Vec<String> {
    let p = Path::new(path);
    let e = ext(path);
    if !IMAGE_EXT.contains(&e.as_str()) || matches!(e.as_str(), "gif" | "psd" | "psb") {
        return vec![];
    }
    let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { return vec![] };
    let Some((prefix, _)) = split_number(stem) else { return vec![] };
    let dir = p.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<(u64, String)> = rd
        .filter_map(|x| x.ok())
        .filter_map(|x| {
            let q = x.path();
            let s = q.to_string_lossy().to_string();
            if ext(&s) != e || !q.is_file() {
                return None;
            }
            let (pre, num) = split_number(q.file_stem()?.to_str()?)?;
            (pre == prefix).then_some((num.parse().ok()?, s))
        })
        .collect();
    out.sort();
    out.into_iter().map(|x| x.1).collect()
}

pub fn missing_frames(files: &[String]) -> Option<String> {
    let mut ns: Vec<i64> = files.iter().filter_map(|f| split_number(Path::new(f).file_stem()?.to_str()?)?.1.parse().ok()).collect();
    ns.sort_unstable();
    ns.dedup();
    let mut gaps = vec![];
    let mut total = 0;
    for w in ns.windows(2) {
        if w[1] - w[0] > 1 {
            let (a, b) = (w[0] + 1, w[1] - 1);
            total += b - a + 1;
            gaps.push(if a == b { a.to_string() } else { format!("{a} to {b}") });
        }
    }
    (!gaps.is_empty()).then(|| format!("{total} missing frame{} ({})", if total == 1 { "" } else { "s" }, gaps.join(", ")))
}

fn ext(path: &str) -> String {
    Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

pub fn tool(name: &str) -> String {
    let var = format!("FORMVFX_{}", name.to_ascii_uppercase());
    std::env::var(&var).unwrap_or_else(|_| name.to_string())
}

pub fn has_ffmpeg() -> bool {
    Command::new(tool("ffmpeg")).arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn rate(s: &str) -> f64 {
    match s.split_once('/') {
        Some((a, b)) => {
            let a: f64 = a.parse().unwrap_or(0.0);
            let b: f64 = b.parse().unwrap_or(1.0);
            if b == 0.0 { 0.0 } else { a / b }
        }
        None => s.parse().unwrap_or(0.0),
    }
}

pub fn probe(path: &str) -> Result<Footage, String> {
    if !Path::new(path).exists() {
        return Err(format!("no file at {path}"));
    }
    let e = ext(path);
    if MODEL_EXT.contains(&e.as_str()) {
        let m = super::model3d::load(path)?;
        let b = m.bounds;
        let k = m.fit;
        let dim = |a: f64| (a * k).round().max(1.0) as u32;
        return Ok(Footage { path: path.into(), kind: FootageKind::Model, width: dim(b[3] - b[0]), height: dim(b[4] - b[1]), model_depth: dim(b[5] - b[2]), has_video: true, codec: e.to_uppercase(), ..Footage::default() });
    }
    if e == "svg" {
        let doc = super::svg::load(path).ok_or_else(|| format!("{path} is not readable SVG"))?;
        return Ok(Footage { path: path.into(), kind: FootageKind::Vector, width: doc.width.round().max(1.0) as u32, height: doc.height.round().max(1.0) as u32, has_video: true, codec: "SVG".into(), ..Footage::default() });
    }
    if e == "exr" {
        let i = super::exr::info(path)?;
        return Ok(Footage { path: path.into(), kind: FootageKind::Image, width: i.width.max(1), height: i.height.max(1), has_video: true, codec: "EXR".into(), alpha: AlphaMode::Premultiplied, transfer: "linearSrgb".into(), ..Footage::default() });
    }
    if e == "psd" || e == "psb" {
        let d = super::psd::open(path)?;
        return Ok(Footage { path: path.into(), kind: FootageKind::Image, width: d.width.max(1) as u32, height: d.height.max(1) as u32, has_video: true, codec: "PSD".into(), ..Footage::default() });
    }
    if super::pdf::EXT.contains(&e.as_str()) {
        let pages = super::pdf::pages(path)?;
        let (w, h) = pages[0];
        return Ok(Footage { path: path.into(), kind: FootageKind::Image, width: w.round().max(1.0) as u32, height: h.round().max(1.0) as u32, has_video: true, codec: e.to_uppercase(), ..Footage::default() });
    }
    if IMAGE_EXT.contains(&e.as_str()) && e != "gif" {
        let (w, h) = image::image_dimensions(path).map_err(|err| format!("{path}: {err}"))?;
        return Ok(Footage { path: path.into(), kind: FootageKind::Image, width: w, height: h, has_video: true, codec: e.to_uppercase(), ..Footage::default() });
    }
    let out = Command::new(tool("ffprobe"))
        .args(["-v", "error", "-print_format", "json", "-show_streams", "-show_format", path])
        .output()
        .map_err(|_| "importing video and audio needs FFmpeg (ffprobe was not found on PATH; set FORMVFX_FFPROBE to its path)".to_string())?;
    if !out.status.success() {
        return Err(format!("{path}: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let j: Json = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let streams = j.get("streams").and_then(Json::as_array).cloned().unwrap_or_default();
    let v = streams.iter().find(|s| s.get("codec_type").and_then(Json::as_str) == Some("video") && s.get("disposition").and_then(|d| d.get("attached_pic")).and_then(Json::as_i64) != Some(1));
    let a = streams.iter().find(|s| s.get("codec_type").and_then(Json::as_str) == Some("audio"));
    let dur = j.get("format").and_then(|f| f.get("duration")).and_then(Json::as_str).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
    let mut f = Footage { path: path.into(), ..Footage::default() };
    if let Some(v) = v {
        f.width = v.get("width").and_then(Json::as_u64).unwrap_or(0) as u32;
        f.height = v.get("height").and_then(Json::as_u64).unwrap_or(0) as u32;
        let fr = v.get("avg_frame_rate").and_then(Json::as_str).map(rate).filter(|x| *x > 0.0).or_else(|| v.get("r_frame_rate").and_then(Json::as_str).map(rate)).unwrap_or(30.0);
        f.frame_rate = if fr > 0.0 && fr < 1000.0 { fr } else { 30.0 };
        f.codec = v.get("codec_name").and_then(Json::as_str).unwrap_or("").to_string();
        let vdur = v.get("duration").and_then(Json::as_str).and_then(|s| s.parse::<f64>().ok()).unwrap_or(dur);
        f.duration = if vdur > 0.0 { vdur } else { dur };
        f.has_video = true;
        let nb = v.get("nb_frames").and_then(Json::as_str).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        f.kind = if (e == "gif" && nb <= 1) || (IMAGE_EXT.contains(&e.as_str()) && f.duration <= 0.05) { FootageKind::Image } else { FootageKind::Video };
        if let Some(sar) = v.get("sample_aspect_ratio").and_then(Json::as_str)
            && let Some((x, y)) = sar.split_once(':')
        {
            let (x, y): (f64, f64) = (x.parse().unwrap_or(1.0), y.parse().unwrap_or(1.0));
            if x > 0.0 && y > 0.0 {
                f.pixel_aspect = x / y;
            }
        }
        if v.get("pix_fmt").and_then(Json::as_str).is_some_and(|p| !p.contains('a') || (p.starts_with("yuv") && !p.starts_with("yuva"))) {
            f.alpha = AlphaMode::Ignore;
        }
        let trc = v.get("color_transfer").and_then(Json::as_str).unwrap_or("");
        let prim = v.get("color_primaries").and_then(Json::as_str).unwrap_or("");
        f.transfer = match trc {
            "smpte2084" => "rec2100pq",
            "arib-std-b67" => "rec2100hlg",
            _ if prim == "bt2020" => "rec2020",
            _ => "",
        }
        .into();
    } else {
        f.has_video = false;
        f.kind = FootageKind::Audio;
        f.duration = dur;
    }
    if let Some(a) = a {
        f.has_audio = true;
        f.audio_rate = a.get("sample_rate").and_then(Json::as_str).and_then(|s| s.parse().ok()).unwrap_or(48000);
        f.audio_channels = a.get("channels").and_then(Json::as_u64).unwrap_or(2) as u32;
        if f.kind == FootageKind::Audio && f.duration <= 0.0 {
            f.duration = a.get("duration").and_then(Json::as_str).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        }
    }
    if !f.has_video && !f.has_audio {
        return Err(format!("{path}: no video or audio streams"));
    }
    Ok(f)
}

fn open_image(path: &str) -> Option<Img> {
    let dynimg = image::open(path).ok()?;
    let deep = matches!(dynimg.color(), image::ColorType::Rgb16 | image::ColorType::Rgba16 | image::ColorType::L16 | image::ColorType::La16);
    Some(if deep {
        let img = dynimg.to_rgba16();
        Img::from_rgba16(img.width() as usize, img.height() as usize, img.as_raw())
    } else {
        let img = dynimg.to_rgba8();
        Img::from_rgba8(img.width() as usize, img.height() as usize, img.as_raw(), false)
    })
}

struct Decoder {
    child: Child,
    out: ChildStdout,
    next: i64,
    w: usize,
    h: usize,
    deep: bool,
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Decoder {
    fn open(path: &str, frame: i64, fps: f64, w: usize, h: usize, deep: bool) -> Option<Decoder> {
        let t = (frame as f64 / fps).max(0.0);
        let mut child = Command::new(tool("ffmpeg"))
            .args(["-v", "error", "-nostdin"])
            .args(["-ss", &format!("{t:.6}")])
            .args(["-i", path, "-an", "-sn", "-dn"])
            .args(["-vf", &format!("scale={w}:{h}:flags=bicubic")])
            .args(["-f", "rawvideo", "-pix_fmt", if deep { "rgba64le" } else { "rgba" }, "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let out = child.stdout.take()?;
        Some(Decoder { child, out, next: frame, w, h, deep })
    }

    fn read(&mut self) -> Option<Img> {
        let mut buf = vec![0u8; self.w * self.h * if self.deep { 8 } else { 4 }];
        self.out.read_exact(&mut buf).ok()?;
        self.next += 1;
        if self.deep {
            let v: Vec<u16> = buf.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
            return Some(Img::from_rgba16(self.w, self.h, &v));
        }
        Some(Img::from_rgba8(self.w, self.h, &buf, false))
    }
}

const FRAME_BUDGET: usize = 1 << 30;

#[derive(Default)]
struct Frames {
    map: HashMap<(String, i64), Arc<Img>>,
    order: VecDeque<(String, i64)>,
    bytes: usize,
}

impl Frames {
    fn get(&mut self, k: &(String, i64)) -> Option<Arc<Img>> {
        self.map.get(k).cloned()
    }
    fn put(&mut self, k: (String, i64), img: Arc<Img>) {
        self.bytes += img.px.len() * 4;
        self.map.insert(k.clone(), img);
        self.order.push_back(k);
        while self.bytes > FRAME_BUDGET {
            let Some(old) = self.order.pop_front() else { break };
            if let Some(i) = self.map.remove(&old) {
                self.bytes -= i.px.len() * 4;
            }
        }
    }
}

static SHARED: std::sync::OnceLock<Mutex<std::sync::Weak<Media>>> = std::sync::OnceLock::new();

pub fn share(m: &Arc<Media>) {
    *SHARED.get_or_init(|| Mutex::new(std::sync::Weak::new())).lock() = Arc::downgrade(m);
}

pub fn shared() -> Option<Arc<Media>> {
    SHARED.get()?.lock().upgrade()
}

#[derive(Default)]
pub struct Media {
    pub models: super::model3d::ModelCache,
    stills: Mutex<HashMap<String, Arc<Img>>>,
    psd: Mutex<HashMap<String, Arc<super::psd::Doc>>>,
    frames: Mutex<Frames>,
    decoders: Mutex<HashMap<String, Arc<Mutex<Option<Decoder>>>>>,
    audio: Mutex<HashMap<(String, u32), Arc<Vec<f32>>>>,
    aux: Mutex<VecDeque<(String, Arc<super::exr::Aux>)>>,
}

impl Media {
    pub fn aux(&self, f: &Footage, source_t: f64) -> Option<Arc<super::exr::Aux>> {
        if f.missing {
            return None;
        }
        let path = if f.kind == FootageKind::Sequence && !f.sequence.is_empty() {
            let len = f.sequence.len() as i64;
            let fps = if f.frame_rate > 0.0 { f.frame_rate } else { 30.0 };
            let mut n = (source_t * fps + 1e-6).floor() as i64;
            if f.loops > 1 {
                n = n.rem_euclid(len);
            }
            f.sequence[n.clamp(0, len - 1) as usize].clone()
        } else {
            f.path.clone()
        };
        if ext(&path) != "exr" {
            return None;
        }
        if let Some(a) = self.aux.lock().iter().find(|a| a.0 == path) {
            return Some(a.1.clone());
        }
        let a = Arc::new(super::exr::read_aux(&path).ok()?);
        let mut q = self.aux.lock();
        q.push_back((path, a.clone()));
        while q.len() > 8 {
            q.pop_front();
        }
        Some(a)
    }

    pub fn psd_doc(&self, path: &str) -> Option<Arc<super::psd::Doc>> {
        if let Some(d) = self.psd.lock().get(path) {
            return Some(d.clone());
        }
        let d = Arc::new(super::psd::open(path).ok()?);
        self.psd.lock().insert(path.to_string(), d.clone());
        Some(d)
    }

    pub fn bars(&self, w: usize, h: usize) -> Arc<Img> {
        let key = format!("\u{1}bars\u{1}{w}x{h}");
        if let Some(i) = self.stills.lock().get(&key) {
            return i.clone();
        }
        const TOP: [[u8; 3]; 7] = [[191, 191, 191], [191, 191, 0], [0, 191, 191], [0, 191, 0], [191, 0, 191], [191, 0, 0], [0, 0, 191]];
        const MID: [[u8; 3]; 7] = [[0, 0, 191], [19, 19, 19], [191, 0, 191], [19, 19, 19], [0, 191, 191], [19, 19, 19], [191, 191, 191]];
        const LOW: [[u8; 3]; 4] = [[0, 33, 76], [255, 255, 255], [50, 0, 106], [19, 19, 19]];
        let mut px = vec![255u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let c = if y * 3 < h * 2 {
                    TOP[(x * 7 / w).min(6)]
                } else if y * 12 < h * 9 {
                    MID[(x * 7 / w).min(6)]
                } else {
                    LOW[(x * 4 / w).min(3)]
                };
                px[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&c);
            }
        }
        let img = Arc::new(Img::from_rgba8(w, h, &px, false));
        self.stills.lock().insert(key, img.clone());
        img
    }

    fn still_of(&self, f: &Footage, path: &str) -> Option<Arc<Img>> {
        let key = format!("{path}\u{1}{:?}\u{1}{}\u{1}{}\u{1}{}\u{1}{}x{}", f.layer, f.layer_name, f.layer_size, f.page, f.width, f.height);
        if let Some(i) = self.stills.lock().get(&key) {
            return Some(i.clone());
        }
        let out = Arc::new(self.decode(f, path)?);
        self.stills.lock().insert(key, out.clone());
        Some(out)
    }

    fn decode(&self, f: &Footage, path: &str) -> Option<Img> {
        Some(match ext(path).as_str() {
            "exr" => super::exr::read(path, (!f.layer_name.is_empty()).then_some(f.layer_name.as_str())).ok()?,
            "psd" | "psb" => {
                let d = self.psd_doc(path)?;
                match f.layer.map(|i| i as usize).filter(|i| *i < d.layers.len()) {
                    Some(i) if f.layer_size => d.layer(i).0,
                    Some(i) => d.layer_canvas(i),
                    None => d.merged(),
                }
            }
            "pdf" | "ai" => super::pdf::render(path, f.page as usize, f.width.max(1) as usize, f.height.max(1) as usize).ok()?,
            _ => open_image(path)?,
        })
    }

    pub fn still(&self, path: &str) -> Option<Arc<Img>> {
        if let Some(i) = self.stills.lock().get(path) {
            return Some(i.clone());
        }
        let out = Arc::new(open_image(path)?);
        self.stills.lock().insert(path.to_string(), out.clone());
        Some(out)
    }

    pub fn frame(&self, f: &Footage, source_t: f64) -> Option<Arc<Img>> {
        if f.missing && f.has_video && f.width > 0 && f.height > 0 {
            return Some(self.bars(f.width as usize, f.height as usize));
        }
        if !f.has_video || f.missing {
            return None;
        }
        if f.kind == FootageKind::Image {
            return self.still_of(f, &f.path);
        }
        if f.kind == FootageKind::Sequence {
            let len = f.sequence.len() as i64;
            if len == 0 {
                return self.still_of(f, &f.path);
            }
            let fps = if f.frame_rate > 0.0 { f.frame_rate } else { 30.0 };
            let mut n = (source_t * fps + 1e-6).floor() as i64;
            if f.loops > 1 {
                n = n.rem_euclid(len);
            }
            let n = n.clamp(0, len - 1);
            let key = (f.path.clone(), n);
            if let Some(i) = self.frames.lock().get(&key) {
                return Some(i);
            }
            let img = Arc::new(self.decode(f, &f.sequence[n as usize])?);
            self.frames.lock().put(key, img.clone());
            return Some(img);
        }
        if f.kind == FootageKind::Vector {
            let doc = super::svg::load(&f.path)?;
            let ops = super::svg::ops(&doc);
            return super::vector::rasterize(&ops, &crate::geom::Mat3::IDENTITY, f.width.max(1) as usize, f.height.max(1) as usize, 0, 0).map(Arc::new);
        }
        let fps = if f.frame_rate > 0.0 { f.frame_rate } else { 30.0 };
        let total = ((f.duration * fps).round() as i64).max(1);
        let mut n = (source_t * fps + 1e-6).floor() as i64;
        if f.loops > 1 {
            n = n.rem_euclid(total);
        }
        let n = n.clamp(0, total - 1);
        let key = (f.path.clone(), n);
        if let Some(i) = self.frames.lock().get(&key) {
            return Some(i);
        }
        let slot = self.decoders.lock().entry(f.path.clone()).or_default().clone();
        let mut dec = slot.lock();
        if let Some(i) = self.frames.lock().get(&key) {
            return Some(i);
        }
        let (w, h) = (f.width.max(1) as usize, f.height.max(1) as usize);
        let deep = !f.transfer.is_empty();
        let reuse = matches!(dec.as_ref(), Some(d) if d.next <= n && n - d.next < 48);
        if !reuse {
            *dec = Decoder::open(&f.path, n, fps, w, h, deep);
        }
        let mut last = None;
        for _ in 0..2 {
            let Some(d) = dec.as_mut() else { break };
            while d.next <= n {
                let idx = d.next;
                let Some(img) = d.read() else {
                    *dec = None;
                    break;
                };
                let img = Arc::new(img);
                self.frames.lock().put((f.path.clone(), idx), img.clone());
                if idx == n {
                    last = Some(img);
                }
            }
            if last.is_some() {
                break;
            }
            if dec.is_none() {
                *dec = Decoder::open(&f.path, (n - 1).max(0), fps, w, h, deep);
            } else {
                break;
            }
        }
        last.or_else(|| {
            let fr = self.frames.lock();
            (0..8).find_map(|k| fr.map.get(&(f.path.clone(), n - k)).cloned())
        })
    }

    pub fn audio(&self, path: &str, rate: u32) -> Option<Arc<Vec<f32>>> {
        let key = (path.to_string(), rate);
        if let Some(a) = self.audio.lock().get(&key) {
            return Some(a.clone());
        }
        let out = Command::new(tool("ffmpeg"))
            .args(["-v", "error", "-nostdin", "-i", path, "-vn", "-sn", "-f", "f32le", "-ac", "2", "-ar", &rate.to_string(), "-"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let samples: Vec<f32> = out.stdout.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let a = Arc::new(samples);
        self.audio.lock().insert(key, a.clone());
        Some(a)
    }

    pub fn forget(&self, path: &str) {
        self.stills.lock().retain(|k, _| k != path && !k.starts_with(&format!("{path}\u{1}")));
        self.psd.lock().remove(path);
        self.decoders.lock().remove(path);
        let mut fr = self.frames.lock();
        let keys: Vec<_> = fr.map.keys().filter(|k| k.0 == path).cloned().collect();
        for k in keys {
            if let Some(i) = fr.map.remove(&k) {
                fr.bytes -= i.px.len() * 4;
            }
        }
        fr.order.retain(|k| k.0 != path);
        self.audio.lock().retain(|k, _| k.0 != path);
    }

    pub fn purge(&self) {
        self.stills.lock().clear();
        self.psd.lock().clear();
        let mut fr = self.frames.lock();
        fr.map.clear();
        fr.order.clear();
        fr.bytes = 0;
        drop(fr);
        self.audio.lock().clear();
        self.aux.lock().clear();
    }
}
