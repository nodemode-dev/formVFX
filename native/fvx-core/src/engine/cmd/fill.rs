use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{Map, Value as Json, json};

use super::{add_layer, footage_layer, select_only};
use crate::engine::jobs::{self, Apply};
use crate::engine::{Engine, Res, pb, ps};
use crate::model::LayerKind;
use crate::render::analysis::{Source, frame_times};
use crate::render::fill::{self, LIGHTING, METHODS, Settings};
use crate::render::image::Img;
use crate::render::media::tool;

const KEYS: [&str; 6] = ["method", "range", "alphaExpansion", "lightingCorrection", "referenceLayer", "referenceTime"];

fn current(e: &Engine) -> Json {
    let mut d = json!({"method": "object", "range": "workArea", "alphaExpansion": 0.0, "lightingCorrection": "none", "referenceLayer": null, "referenceTime": null});
    if let (Some(o), Some(saved)) = (d.as_object_mut(), e.prefs.get("contentFill").and_then(Json::as_object)) {
        for (k, v) in saved {
            o.insert(k.clone(), v.clone());
        }
    }
    d
}

pub fn set(e: &mut Engine, p: &Json) -> Res {
    let mut cur = e.prefs.get("contentFill").and_then(Json::as_object).cloned().unwrap_or_default();
    let mut m = Map::new();
    if let Some(o) = p.as_object() {
        for k in KEYS {
            if let Some(v) = o.get(k) {
                m.insert(k.into(), v.clone());
            }
        }
    }
    if let Some(v) = m.get("method").and_then(Json::as_str)
        && !METHODS.contains(&v)
    {
        return Err(format!("method is one of {}", METHODS.join(", ")));
    }
    if let Some(v) = m.get("lightingCorrection").and_then(Json::as_str)
        && !LIGHTING.contains(&v)
    {
        return Err(format!("lightingCorrection is one of {}", LIGHTING.join(", ")));
    }
    for (k, v) in m {
        cur.insert(k, v);
    }
    e.prefs.insert("contentFill".into(), Json::Object(cur));
    Ok(current(e))
}

fn write_png(img: &Img, path: &PathBuf) -> Result<(), String> {
    let mut bytes = Vec::with_capacity(img.w * img.h * 4);
    for p in img.px.chunks_exact(4) {
        let a = p[3].clamp(0.0, 1.0);
        let un = |v: f32| if a > 1e-6 { (v / a).clamp(0.0, 1.0) } else { 0.0 };
        bytes.extend_from_slice(&[(un(p[0]) * 255.0).round() as u8, (un(p[1]) * 255.0).round() as u8, (un(p[2]) * 255.0).round() as u8, (a * 255.0).round() as u8]);
    }
    image::save_buffer(path, &bytes, img.w as u32, img.h as u32, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())
}

fn out_dir(e: &Engine, p: &Json) -> PathBuf {
    if let Some(d) = ps(p, "outputDir") {
        return PathBuf::from(d);
    }
    match e.path.as_ref().and_then(|p| PathBuf::from(p).parent().map(|d| d.to_path_buf())) {
        Some(d) => d.join("Fill"),
        None => std::env::temp_dir().join("formvfx-fill"),
    }
}

pub fn generate(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let o = current(e);
    let get = |k: &str| p.get(k).filter(|v| !v.is_null()).or_else(|| o.get(k).filter(|v| !v.is_null())).cloned();
    let method = METHODS.iter().position(|m| Some(*m) == get("method").as_ref().and_then(Json::as_str)).unwrap_or(0);
    let lighting = LIGHTING.iter().position(|m| Some(*m) == get("lightingCorrection").as_ref().and_then(Json::as_str)).unwrap_or(0);
    let expansion = get("alphaExpansion").and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0);
    let (times, name, fps, start) = {
        let c = e.project.comp(cid).ok_or("no composition")?;
        let l = c.layer(lid).ok_or("no layer")?;
        if !matches!(l.kind, LayerKind::Footage | LayerKind::Precomp | LayerKind::Solid) {
            return Err("Selection-Aware-Fill works on a footage or composition layer".into());
        }
        let (a, b) = match get("range").as_ref().and_then(Json::as_str) {
            Some("entire") => (l.in_point, l.out_point),
            _ => (c.work_area[0].max(l.in_point), c.work_area[1].min(l.out_point)),
        };
        let times = frame_times(&e.project, cid, lid, a, b);
        if times.is_empty() {
            return Err("the layer has no frames in that range".into());
        }
        let first = times[0];
        (times, l.name.clone(), c.frame_rate, first)
    };
    let reference = get("referenceLayer").and_then(|v| v.as_u64()).map(|r| (r, get("referenceTime").and_then(|v| v.as_f64()).unwrap_or(start)));
    if reference.is_some_and(|(r, _)| e.layer_ref(cid, r).is_err()) {
        return Err("the reference layer is not in this composition".into());
    }
    let dir = out_dir(e, p);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let movie = dir.join(format!("{safe} Fill {stamp}.mov"));
    let frames_dir = dir.join(format!(".{safe}_fill_{stamp}"));
    let settings = Settings { method, expansion, lighting };
    let wait = pb(p, "wait").unwrap_or(false);
    let label_name = format!("{name} Fill");
    jobs::run(e, "fill", lid, "Selection-Aware-Fill", wait, move |ctx| {
        let src = Source::new(&ctx.project, &ctx.media, cid, lid, 4096.0).ok_or("the layer is gone")?;
        let total = times.len() * 3;
        let mut frames = Vec::with_capacity(times.len());
        for (i, t) in times.iter().enumerate() {
            if ctx.stopped() {
                return Err("stopped".into());
            }
            ctx.step(i, total, "Reading frames");
            frames.push(fill::read(&src, *t, expansion).ok_or("cannot read the layer's frames")?);
        }
        if !frames.iter().any(|f| f.hole.iter().any(|b| *b)) {
            return Err("nothing to fill: make the area transparent first (a Subtract mask around the object, or alpha in the footage)".into());
        }
        let to_ref = fill::motion(ctx, &frames, times.len(), total);
        let ref_img = match reference {
            Some((rl, rt)) => {
                let rs = Source::new(&ctx.project, &ctx.media, cid, rl, 4096.0).ok_or("the reference layer is gone")?;
                let img = rs.image(rt).ok_or("cannot read the reference layer")?;
                let at = times.iter().enumerate().min_by(|a, b| (a.1 - rt).abs().total_cmp(&(b.1 - rt).abs())).map(|x| x.0).unwrap_or(0);
                let (w, h) = (frames[0].raw.w, frames[0].raw.h);
                let img = if img.w == w && img.h == h {
                    img
                } else {
                    let mut o = Img::new(w, h);
                    let (kx, ky) = (img.w as f64 / w as f64, img.h as f64 / h as f64);
                    for y in 0..h {
                        for x in 0..w {
                            let c = img.sample((x as f64 + 0.5) * kx, (y as f64 + 0.5) * ky, true);
                            o.px[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&c);
                        }
                    }
                    o
                };
                Some((img, at))
            }
            None => None,
        };
        std::fs::create_dir_all(&frames_dir).map_err(|e| format!("cannot write the fill frames: {e}"))?;
        for k in 0..frames.len() {
            if ctx.stopped() {
                return Err("stopped".into());
            }
            ctx.step(times.len() * 2 + k, total, "Filling");
            let out = fill::fill_frame(&frames, &to_ref, k, &settings, ref_img.as_ref().map(|(i, a)| (i, *a)));
            write_png(&out, &frames_dir.join(format!("f{k:05}.png")))?;
        }
        let status = Command::new(tool("ffmpeg"))
            .args(["-v", "error", "-nostdin", "-y", "-framerate", &format!("{fps}"), "-i"])
            .arg(frames_dir.join("f%05d.png"))
            .args(["-c:v", "png", "-pix_fmt", "rgba"])
            .arg(&movie)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| "Selection-Aware-Fill needs FFmpeg to write the fill layer".to_string())?;
        if !status.success() {
            return Err("FFmpeg could not write the fill layer".into());
        }
        let _ = std::fs::remove_dir_all(&frames_dir);
        let n = frames.len();
        let movie_s = movie.to_string_lossy().to_string();
        let apply: Apply = Box::new(move |e: &mut Engine| {
            let r = super::file::import(e, &json!({"paths": [movie_s]}))?;
            let item = r.get("items").and_then(|a| a.get(0)).and_then(Json::as_u64).ok_or("could not import the fill")?;
            let mut l = footage_layer(e, cid, item, start, None, None)?;
            l.name = label_name.clone();
            let at = e.project.comp(cid).and_then(|c| c.index_of(lid));
            let fl = add_layer(e, cid, l, at);
            select_only(e, &[fl]);
            Ok(json!({"layer": fl, "item": item, "frames": n, "path": movie_s}))
        });
        Ok(apply)
    })
}

pub fn new_layer(e: &mut Engine, p: &Json) -> Res {
    if let Some(m) = ps(p, "method")
        && !METHODS.contains(&m)
    {
        return Err(format!("method is one of {}", METHODS.join(", ")));
    }
    generate(e, p)
}
