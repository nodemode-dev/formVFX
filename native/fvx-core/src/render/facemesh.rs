use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use rayon::prelude::*;

use super::image::Img;
use super::roto::{ModelInfo, models_dir};
use super::tflite;

pub const MODEL: ModelInfo = ModelInfo {
    id: "facelandmarker",
    name: "MediaPipe Face Landmarker",
    file: "face_landmarker.task",
    url: "https://storage.googleapis.com/mediapipe-models/face_landmarker/face_landmarker/float16/1/face_landmarker.task",
    size: 3_758_596,
    sha256: "64184e229b263107bc2b804c6625db1341ff2bb731874b0bcc2fe6544e0bc9ff",
    licence: "Apache-2.0",
};

pub const OVAL: [usize; 36] = [10, 338, 297, 332, 284, 251, 389, 356, 454, 323, 361, 288, 397, 365, 379, 378, 400, 377, 152, 148, 176, 149, 150, 136, 172, 58, 132, 93, 234, 127, 162, 21, 54, 103, 67, 109];

pub const FEATURES: [usize; 23] = [107, 105, 70, 336, 334, 300, 133, 33, 159, 145, 468, 362, 263, 386, 374, 473, 1, 98, 327, 61, 291, 13, 14];

pub struct Models {
    detector: tflite::Model,
    mesh: tflite::Model,
    mesh_size: usize,
}

pub fn model_path() -> std::path::PathBuf {
    models_dir().join(MODEL.file)
}

pub fn installed() -> bool {
    std::fs::metadata(model_path()).map(|m| m.len() == MODEL.size).unwrap_or(false)
}

fn cache() -> &'static Mutex<Option<Arc<Models>>> {
    static C: OnceLock<Mutex<Option<Arc<Models>>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

pub fn unload() {
    *cache().lock() = None;
}

pub fn load() -> Result<Arc<Models>, String> {
    if let Some(m) = cache().lock().as_ref() {
        return Ok(m.clone());
    }
    let path = model_path();
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let files = crate::engine::lottie::unzip(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let get = |n: &str| files.get(n).ok_or_else(|| format!("{} has no {n}", path.display()));
    let detector = tflite::load(get("face_detector.tflite")?)?;
    let mesh = tflite::load(get("face_landmarks_detector.tflite")?)?;
    let mesh_size = mesh.input_shape(0).get(1).copied().unwrap_or(256);
    let m = Arc::new(Models { detector, mesh, mesh_size });
    *cache().lock() = Some(m.clone());
    Ok(m)
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub c: [f32; 2],
    pub size: f32,
    pub angle: f32,
}

#[derive(Clone, Debug)]
pub struct Face {
    pub points: Vec<[f32; 3]>,
    pub score: f32,
    pub rect: Rect,
}

fn straight(img: &Img, x: f32, y: f32) -> [f32; 3] {
    let p = img.sample(x as f64, y as f64, true);
    let a = p[3];
    if a <= 1e-6 {
        return [0.0; 3];
    }
    let f = |v: f32| (v / a).clamp(0.0, 1.0);
    [f(p[0]), f(p[1]), f(p[2])]
}

fn crop(img: &Img, r: &Rect, n: usize, lo: f32, hi: f32) -> Vec<f32> {
    let (s, c) = r.angle.sin_cos();
    let mut out = vec![0f32; n * n * 3];
    out.par_chunks_mut(n * 3).enumerate().for_each(|(v, row)| {
        for u in 0..n {
            let (dx, dy) = (((u as f32 + 0.5) / n as f32 - 0.5) * r.size, ((v as f32 + 0.5) / n as f32 - 0.5) * r.size);
            let (x, y) = (r.c[0] + c * dx - s * dy, r.c[1] + s * dx + c * dy);
            let inside = x >= 0.0 && y >= 0.0 && x < img.w as f32 && y < img.h as f32;
            let px = if inside { straight(img, x, y) } else { [0.0; 3] };
            for k in 0..3 {
                row[u * 3 + k] = lo + px[k] * (hi - lo);
            }
        }
    });
    out
}

fn anchors() -> Vec<[f32; 2]> {
    let mut a = vec![];
    for (grid, per) in [(16usize, 2usize), (8, 6)] {
        for y in 0..grid {
            for x in 0..grid {
                for _ in 0..per {
                    a.push([(x as f32 + 0.5) / grid as f32, (y as f32 + 0.5) / grid as f32]);
                }
            }
        }
    }
    a
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x.clamp(-100.0, 100.0)).exp())
}

struct Det {
    b: [f32; 4],
    k: [[f32; 2]; 6],
    score: f32,
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let ix = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let iy = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let inter = ix * iy;
    let u = (a[2] - a[0]) * (a[3] - a[1]) + (b[2] - b[0]) * (b[3] - b[1]) - inter;
    if u <= 0.0 { 0.0 } else { inter / u }
}

fn weighted_nms(mut d: Vec<Det>) -> Vec<Det> {
    d.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut out = vec![];
    while !d.is_empty() {
        let top = d.remove(0);
        let (group, rest): (Vec<Det>, Vec<Det>) = d.into_iter().partition(|x| iou(&top.b, &x.b) > 0.3);
        d = rest;
        let mut w = top.score;
        let mut b = top.b.map(|v| v * top.score);
        let mut k = top.k.map(|p| [p[0] * top.score, p[1] * top.score]);
        for g in &group {
            w += g.score;
            for i in 0..4 {
                b[i] += g.b[i] * g.score;
            }
            for i in 0..6 {
                k[i][0] += g.k[i][0] * g.score;
                k[i][1] += g.k[i][1] * g.score;
            }
        }
        out.push(Det { b: b.map(|v| v / w), k: k.map(|p| [p[0] / w, p[1] / w]), score: top.score });
    }
    out
}

fn angle_of(a: [f32; 2], b: [f32; 2]) -> f32 {
    let r = -(-(b[1] - a[1])).atan2(b[0] - a[0]);
    let mut r = r;
    while r > std::f32::consts::PI {
        r -= std::f32::consts::TAU;
    }
    while r < -std::f32::consts::PI {
        r += std::f32::consts::TAU;
    }
    r
}

pub fn detect(m: &Models, img: &Img, roi: [f32; 4]) -> Result<Vec<(Rect, f32)>, String> {
    let (rw, rh) = ((roi[2] - roi[0]).max(1.0), (roi[3] - roi[1]).max(1.0));
    let side = rw.max(rh);
    let box_ = Rect { c: [(roi[0] + roi[2]) * 0.5, (roi[1] + roi[3]) * 0.5], size: side, angle: 0.0 };
    let input = crop(img, &box_, 128, -1.0, 1.0);
    let out = m.detector.run(vec![input])?;
    let (reg, cls) = (&out[0].f, &out[1].f);
    let mut dets = vec![];
    for (i, a) in anchors().iter().enumerate() {
        let score = sigmoid(cls[i]);
        if score < 0.5 {
            continue;
        }
        let r = &reg[i * 16..i * 16 + 16];
        let (cx, cy, w, h) = (r[0] / 128.0 + a[0], r[1] / 128.0 + a[1], r[2] / 128.0, r[3] / 128.0);
        let mut k = [[0f32; 2]; 6];
        for j in 0..6 {
            k[j] = [r[4 + 2 * j] / 128.0 + a[0], r[5 + 2 * j] / 128.0 + a[1]];
        }
        dets.push(Det { b: [cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0], k, score });
    }
    let to_img = |p: [f32; 2]| [box_.c[0] + (p[0] - 0.5) * side, box_.c[1] + (p[1] - 0.5) * side];
    Ok(weighted_nms(dets)
        .into_iter()
        .map(|d| {
            let (a, b) = (to_img([d.b[0], d.b[1]]), to_img([d.b[2], d.b[3]]));
            let size = (b[0] - a[0]).max(b[1] - a[1]) * 1.5;
            let angle = angle_of(to_img(d.k[0]), to_img(d.k[1]));
            (Rect { c: [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5], size, angle }, d.score)
        })
        .collect())
}

pub fn landmarks(m: &Models, img: &Img, r: &Rect) -> Result<Face, String> {
    let n = m.mesh_size;
    let out = m.mesh.run(vec![crop(img, r, n, 0.0, 1.0)])?;
    let pts = &out[0].f;
    let score = sigmoid(out.get(1).and_then(|t| t.f.first().copied()).unwrap_or(0.0));
    let (s, c) = r.angle.sin_cos();
    let points = pts
        .chunks_exact(3)
        .map(|p| {
            let (dx, dy) = ((p[0] / n as f32 - 0.5) * r.size, (p[1] / n as f32 - 0.5) * r.size);
            [r.c[0] + c * dx - s * dy, r.c[1] + s * dx + c * dy, p[2] / n as f32 * r.size]
        })
        .collect();
    Ok(Face { points, score, rect: *r })
}

pub fn rect_from(points: &[[f32; 3]]) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in points.iter().take(468) {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    let angle = angle_of([points[33][0], points[33][1]], [points[263][0], points[263][1]]);
    Rect { c: [(x0 + x1) * 0.5, (y0 + y1) * 0.5], size: (x1 - x0).max(y1 - y0) * 1.5, angle }
}

pub struct Tracker {
    models: Arc<Models>,
    prev: Option<Rect>,
}

impl Tracker {
    pub fn new() -> Result<Tracker, String> {
        Ok(Tracker { models: load()?, prev: None })
    }

    pub fn next(&mut self, img: &Img, roi: [f64; 4]) -> Result<Option<Face>, String> {
        if let Some(r) = self.prev {
            let f = landmarks(&self.models, img, &r)?;
            if f.score > 0.5 {
                self.prev = Some(rect_from(&f.points));
                return Ok(Some(f));
            }
        }
        let roi = [roi[0] as f32, roi[1] as f32, roi[2] as f32, roi[3] as f32];
        let mut found = detect(&self.models, img, roi)?;
        if found.is_empty() {
            found = detect(&self.models, img, [0.0, 0.0, img.w as f32, img.h as f32])?;
        }
        let Some((r, _)) = found.into_iter().max_by(|a, b| a.1.total_cmp(&b.1)) else {
            self.prev = None;
            return Ok(None);
        };
        let f = landmarks(&self.models, img, &r)?;
        if f.score < 0.5 {
            self.prev = None;
            return Ok(None);
        }
        self.prev = Some(rect_from(&f.points));
        Ok(Some(f))
    }
}
