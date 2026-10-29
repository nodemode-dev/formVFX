use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use super::export::{FORMATS, Progress, color_args, ext_of, renders_dir};
use super::{Engine, Res, pb, pf, ps, pu};
use crate::model::{Comp, ItemKind, OutputModule, Project, RenderItem, RenderSettings};
use crate::render::{Img, Media, RenderOpts, Renderer, audio, exr, media};

static NOTIFY: AtomicBool = AtomicBool::new(true);
static OVERFLOW: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn notify_on() -> bool {
    NOTIFY.load(Ordering::SeqCst)
}

pub fn is_seq(fmt: &str) -> bool {
    matches!(fmt, "png" | "jpeg" | "tiff" | "exr")
}

pub fn is_audio(fmt: &str) -> bool {
    matches!(fmt, "wav" | "aiff")
}

fn pick<'a>(p: &'a Json, key: &str, allowed: &[&str]) -> Result<Option<&'a str>, String> {
    match ps(p, key) {
        Some(v) if allowed.contains(&v) => Ok(Some(v)),
        Some(v) => Err(format!("{key}: {}, not `{v}`", allowed.join("|"))),
        None => Ok(None),
    }
}

pub fn apply_settings(s: &mut RenderSettings, p: &Json) -> Result<(), String> {
    let before = s.clone();
    apply_settings_raw(s, p)?;
    if ps(p, "template").or(ps(p, "renderSettings")).is_none() && *s != before {
        s.template = "Custom".into();
    }
    Ok(())
}

fn apply_settings_raw(s: &mut RenderSettings, p: &Json) -> Result<(), String> {
    if let Some(t) = ps(p, "template").or(ps(p, "renderSettings")) {
        *s = settings_template(t).ok_or_else(|| format!("no Render Settings template `{t}` ({})", load().render_settings.keys().cloned().collect::<Vec<_>>().join(", ")))?;
    }
    if let Some(q) = p.get("quality").and_then(Json::as_str).filter(|q| matches!(*q, "best" | "draft" | "current" | "wireframe")) {
        s.quality = q.to_string();
    }
    if let Some(r) = p.get("resolution") {
        s.resolution = match r {
            Json::Number(n) => n.as_f64().unwrap_or(1.0).clamp(0.05, 1.0),
            Json::String(x) => match x.as_str() {
                "full" | "current" => 1.0,
                "half" => 0.5,
                "third" => 1.0 / 3.0,
                "quarter" => 0.25,
                other => other.parse::<f64>().map_err(|_| format!("resolution: full|half|third|quarter or a scale, not `{other}`"))?.clamp(0.05, 1.0),
            },
            _ => s.resolution,
        };
    }
    if let Some(sc) = pf(p, "scale") {
        s.resolution = sc.clamp(0.05, 1.0);
    }
    if let Some(v) = pick(p, "proxyUse", &["current", "all", "comp", "none"])? {
        s.proxy_use = v.into();
    }
    if let Some(v) = pick(p, "effects", &["current", "allOn", "allOff"])? {
        s.effects = v.into();
    }
    if let Some(v) = pick(p, "solo", &["current", "allOff"])? {
        s.solo = v.into();
    }
    if let Some(v) = pick(p, "guideLayers", &["current", "allOff"])? {
        s.guide_layers = v.into();
    }
    if let Some(d) = p.get("colorDepth") {
        s.color_depth = match d.as_u64().or_else(|| d.as_str().and_then(|x| x.split_whitespace().next()?.parse().ok())) {
            Some(8) => 8,
            Some(16) => 16,
            Some(32) => 32,
            _ if d.as_str() == Some("current") => 0,
            _ => return Err("colorDepth: current|8|16|32".into()),
        };
    }
    if let Some(v) = pick(p, "frameBlending", &["current", "onForChecked", "offForAll"])? {
        s.frame_blending = v.into();
    }
    if let Some(v) = pick(p, "fieldRender", &["off", "upper", "lower"])? {
        s.field_render = v.into();
    }
    if let Some(v) = pick(p, "pulldown", &["off", "WSSWW", "SSWWW", "SWWWS", "WWWSS", "WWSSW"])? {
        s.pulldown = v.into();
    }
    match p.get("motionBlur") {
        Some(Json::Bool(b)) => s.motion_blur = if *b { "onForChecked" } else { "offForAll" }.into(),
        Some(Json::String(_)) => {
            if let Some(v) = pick(p, "motionBlur", &["current", "onForChecked", "offForAll"])? {
                s.motion_blur = v.into();
            }
        }
        _ => {}
    }
    if let Some(v) = pick(p, "timeSpan", &["workArea", "comp", "custom"])? {
        s.time_span = v.into();
    }
    match p.get("frameRate") {
        Some(Json::Null) => s.frame_rate = 0.0,
        Some(v) => s.frame_rate = v.as_f64().filter(|r| *r > 0.0 && *r < 1000.0).ok_or("frameRate: frames per second, or null for the composition's")?,
        None => {}
    }
    if let Some(b) = pb(p, "skipExisting") {
        s.skip_existing = b;
    }
    if let Some(b) = pb(p, "storageOverflow") {
        s.storage_overflow = b;
    }
    Ok(())
}

fn fix_ext(m: &mut OutputModule) {
    if m.output.is_empty() {
        return;
    }
    let ext = ext_of(&m.format);
    let mut stem = Path::new(&m.output).with_extension("").to_string_lossy().to_string();
    if is_seq(&m.format) && !stem.contains('[') {
        stem.push_str("_[#####]");
    }
    if !is_seq(&m.format)
        && let Some(i) = stem.find("_[#")
    {
        stem.truncate(i);
    }
    m.output = format!("{stem}.{ext}");
}

pub fn apply_module(m: &mut OutputModule, p: &Json) -> Result<(), String> {
    let before = m.clone();
    apply_module_raw(m, p)?;
    let mut a = m.clone();
    a.output = before.output.clone();
    if ps(p, "template").or(ps(p, "outputModule")).is_none() && a != before {
        let name = FORMATS.iter().find(|f| f.0 == m.format).map(|f| f.2).unwrap_or("Custom");
        m.template = format!("Custom: {name}");
    }
    Ok(())
}

fn apply_module_raw(m: &mut OutputModule, p: &Json) -> Result<(), String> {
    if let Some(t) = ps(p, "template").or(ps(p, "outputModule")) {
        let out = std::mem::take(&mut m.output);
        *m = module_template(t).ok_or_else(|| format!("no Output Module template `{t}` ({})", load().output_modules.keys().cloned().collect::<Vec<_>>().join(", ")))?;
        m.output = out;
        fix_ext(m);
    }
    if let Some(f) = ps(p, "format") {
        let f = f.to_ascii_lowercase();
        if !FORMATS.iter().any(|x| x.0 == f) {
            return Err(format!("unknown format `{f}` ({})", FORMATS.iter().map(|x| x.0).collect::<Vec<_>>().join(", ")));
        }
        if p.get("channels").is_none() {
            m.channels = if matches!(f.as_str(), "png" | "tiff" | "exr" | "webm") { "rgba" } else { "rgb" }.into();
        }
        m.format = f;
        fix_ext(m);
    }
    if let Some(o) = ps(p, "output").or(ps(p, "path")) {
        m.output = o.to_string();
    }
    if let Some(v) = pick(p, "channels", &["rgb", "rgba", "alpha"])? {
        m.channels = v.into();
    }
    if let Some(v) = pick(p, "color", &["straight", "premultiplied"])? {
        m.color = v.into();
    }
    if let Some(q) = p.get("quality").and_then(Json::as_u64) {
        m.quality = q.clamp(1, 100) as u32;
    }
    if let Some(b) = pu(p, "bitrate") {
        m.bitrate = b.min(1_000_000) as u32;
    }
    if pb(p, "webmBitrate") == Some(false) || ps(p, "rateControl") == Some("quality") {
        m.bitrate = 0;
    }
    if let Some(k) = pu(p, "keyframeInterval") {
        m.keyframe_interval = k.min(10_000) as u32;
    }
    if let Some(v) = pick(p, "proresProfile", &["proxy", "lt", "standard", "hq", "4444", "4444xq"])? {
        m.prores_profile = v.into();
    }
    if let Some(v) = pick(p, "profile", &["main", "main10", "high"])? {
        m.profile = v.into();
    }
    if let Some(v) = pick(p, "webmCodec", &["vp9", "av1"])? {
        m.webm_codec = v.into();
    }
    if let Some(v) = pick(p, "audio", &["auto", "on", "off"])? {
        m.audio = v.into();
    }
    if let Some(r) = pu(p, "sampleRate") {
        m.sample_rate = r.clamp(8000, 192_000) as u32;
    }
    if let Some(v) = pick(p, "audioChannels", &["mono", "stereo"])? {
        m.audio_channels = v.into();
    }
    if let Some(b) = pu(p, "audioFormat") {
        m.audio_format = match b {
            24 => 24,
            32 => 32,
            _ => 16,
        };
    }
    if let Some(b) = pu(p, "audioBitrate") {
        m.audio_bitrate = b.clamp(16, 1024) as u32;
    }
    match p.get("crop") {
        Some(Json::Bool(false)) => m.crop = [0; 4],
        Some(Json::Object(o)) => {
            let g = |k: &str| o.get(k).and_then(Json::as_i64).unwrap_or(0).max(0) as i32;
            m.crop = [g("top"), g("left"), g("bottom"), g("right")];
        }
        _ => {}
    }
    for (k, i) in [("cropTop", 0), ("cropLeft", 1), ("cropBottom", 2), ("cropRight", 3)] {
        if let Some(v) = p.get(k).and_then(Json::as_i64) {
            m.crop[i] = v.max(0) as i32;
        }
    }
    match p.get("resize") {
        Some(Json::Bool(false)) => m.resize = None,
        Some(Json::Object(o)) => {
            let w = o.get("width").and_then(Json::as_u64).unwrap_or(0) as u32;
            let h = o.get("height").and_then(Json::as_u64).unwrap_or(0) as u32;
            m.resize = (w > 0 && h > 0).then_some([w.min(16384), h.min(16384)]);
        }
        _ => {}
    }
    if let (Some(w), Some(h)) = (pu(p, "resizeWidth"), pu(p, "resizeHeight")) {
        m.resize = (w > 0 && h > 0).then_some([w.min(16384) as u32, h.min(16384) as u32]);
    }
    if let Some(v) = pick(p, "postRenderAction", &["none", "import", "importAndReplace", "setProxy"])? {
        m.post_render = v.into();
    }
    if let Some(s) = ps(p, "outputSpace") {
        m.output_space = if s.eq_ignore_ascii_case("working") { String::new() } else { s.to_string() };
    }
    if let Some(d) = p.get("depth").and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim_end_matches(" bpc").parse().ok()))) {
        m.depth = match d {
            32.. => 32,
            16..=31 => 16,
            _ => 8,
        };
    }
    if let Some(b) = pb(p, "loop") {
        m.loop_gif = b;
    }
    Ok(())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Templates {
    pub render_settings: BTreeMap<String, RenderSettings>,
    pub output_modules: BTreeMap<String, OutputModule>,
    pub defaults: BTreeMap<String, String>,
}

const BUILTIN_RS: [&str; 5] = ["Best Settings", "Current Settings", "Draft Settings", "DV Settings", "Multi-Machine Settings"];

fn builtin() -> Templates {
    let mut t = Templates::default();
    let base = RenderSettings::default();
    let rs = |name: &str, f: &dyn Fn(&mut RenderSettings)| {
        let mut s = base.clone();
        s.template = name.into();
        f(&mut s);
        s
    };
    t.render_settings.insert("Best Settings".into(), rs("Best Settings", &|_| {}));
    t.render_settings.insert(
        "Current Settings".into(),
        rs("Current Settings", &|s| {
            s.quality = "current".into();
            s.proxy_use = "current".into();
            s.guide_layers = "current".into();
            s.frame_blending = "current".into();
            s.motion_blur = "current".into();
        }),
    );
    t.render_settings.insert(
        "Draft Settings".into(),
        rs("Draft Settings", &|s| {
            s.quality = "draft".into();
            s.resolution = 0.5;
            s.frame_blending = "offForAll".into();
            s.motion_blur = "offForAll".into();
        }),
    );
    t.render_settings.insert(
        "DV Settings".into(),
        rs("DV Settings", &|s| {
            s.field_render = "lower".into();
            s.frame_rate = 29.97;
        }),
    );
    t.render_settings.insert("Multi-Machine Settings".into(), rs("Multi-Machine Settings", &|s| s.skip_existing = true));
    let om = |name: &str, f: &dyn Fn(&mut OutputModule)| {
        let mut m = OutputModule { template: name.into(), ..OutputModule::default() };
        f(&mut m);
        m
    };
    let list: Vec<OutputModule> = vec![
        om("H.264 - Match Render Settings - 15 Mbps", &|m| m.bitrate = 15000),
        om("H.264 - Match Render Settings - 40 Mbps", &|m| m.bitrate = 40000),
        om("H.264 - Match Render Settings - 5 Mbps", &|m| m.bitrate = 5000),
        om("HEVC - Match Render Settings", &|m| m.format = "hevc".into()),
        om("AV1 - Match Render Settings", &|m| m.format = "av1".into()),
        om("High Quality", &|m| {
            m.format = "prores".into();
            m.prores_profile = "hq".into();
        }),
        om("High Quality with Alpha", &|m| {
            m.format = "prores".into();
            m.prores_profile = "4444".into();
            m.channels = "rgba".into();
        }),
        om("Lossless", &|m| {
            m.format = "prores".into();
            m.prores_profile = "4444xq".into();
        }),
        om("Lossless with Alpha", &|m| {
            m.format = "prores".into();
            m.prores_profile = "4444xq".into();
            m.channels = "rgba".into();
        }),
        om("Alpha Only", &|m| {
            m.format = "prores".into();
            m.prores_profile = "4444".into();
            m.channels = "alpha".into();
        }),
        om("WebM", &|m| {
            m.format = "webm".into();
            m.channels = "rgba".into();
        }),
        om("Animated GIF", &|m| m.format = "gif".into()),
        om("PNG Sequence", &|m| m.format = "png".into()),
        om("PNG Sequence with Alpha", &|m| {
            m.format = "png".into();
            m.channels = "rgba".into();
        }),
        om("TIFF Sequence with Alpha", &|m| {
            m.format = "tiff".into();
            m.channels = "rgba".into();
        }),
        om("JPEG Sequence", &|m| m.format = "jpeg".into()),
        om("OpenEXR", &|m| {
            m.format = "exr".into();
            m.channels = "rgba".into();
            m.color = "premultiplied".into();
        }),
        om("WAV", &|m| m.format = "wav".into()),
        om("AIFF 48kHz", &|m| m.format = "aiff".into()),
    ];
    for m in list {
        t.output_modules.insert(m.template.clone(), m);
    }
    for (k, v) in [
        ("renderSettings.movie", "Best Settings"),
        ("renderSettings.still", "Best Settings"),
        ("renderSettings.preRender", "Best Settings"),
        ("renderSettings.movieProxy", "Draft Settings"),
        ("renderSettings.stillProxy", "Best Settings"),
        ("outputModule.movie", "H.264 - Match Render Settings - 15 Mbps"),
        ("outputModule.still", "PNG Sequence"),
        ("outputModule.preRender", "Lossless with Alpha"),
        ("outputModule.movieProxy", "Lossless with Alpha"),
        ("outputModule.stillProxy", "PNG Sequence with Alpha"),
    ] {
        t.defaults.insert(k.into(), v.into());
    }
    t
}

fn user_path() -> PathBuf {
    crate::render::diskcache::data_dir().join("render-templates.json")
}

fn user() -> Templates {
    std::fs::read(user_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_user(t: &Templates) -> Result<(), String> {
    let p = user_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(&p, serde_json::to_vec_pretty(t).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", p.display()))
}

pub fn load() -> Templates {
    let mut t = builtin();
    let u = user();
    t.render_settings.extend(u.render_settings);
    t.output_modules.extend(u.output_modules);
    t.defaults.extend(u.defaults);
    t
}

pub fn settings_template(name: &str) -> Option<RenderSettings> {
    load().render_settings.get(name).cloned().map(|mut s| {
        s.template = name.into();
        s
    })
}

pub fn module_template(name: &str) -> Option<OutputModule> {
    load().output_modules.get(name).cloned().map(|mut m| {
        m.template = name.into();
        m
    })
}

pub fn default_settings(slot: &str) -> RenderSettings {
    let t = load();
    t.defaults.get(&format!("renderSettings.{slot}")).and_then(|n| t.render_settings.get(n).cloned()).unwrap_or_default()
}

pub fn default_module(slot: &str) -> OutputModule {
    let t = load();
    t.defaults.get(&format!("outputModule.{slot}")).and_then(|n| t.output_modules.get(n).cloned()).unwrap_or_default()
}

pub fn item_index(e: &Engine, p: &Json) -> Result<usize, String> {
    let q = &e.project.render_queue;
    if let Some(id) = pu(p, "item") {
        return q.iter().position(|r| r.id == id).ok_or_else(|| "no such render item".into());
    }
    if let Some(i) = pu(p, "index") {
        return if i >= 1 && (i as usize) <= q.len() { Ok(i as usize - 1) } else { Err(format!("index: 1 to {}", q.len())) };
    }
    if q.is_empty() {
        return Err("the Render Queue is empty (renderQueue.add)".into());
    }
    Ok(q.len() - 1)
}

fn module_index(it: &RenderItem, p: &Json) -> Result<usize, String> {
    match pu(p, "module") {
        Some(n) if n >= 1 && (n as usize) <= it.modules.len() => Ok(n as usize - 1),
        Some(_) => Err(format!("module: 1 to {}", it.modules.len())),
        None => Ok(0),
    }
}

fn editable(r: &RenderItem) -> Result<(), String> {
    if r.status == "Rendering" {
        return Err("that item is rendering".into());
    }
    Ok(())
}

pub fn set_render_settings(e: &mut Engine, p: &Json) -> Res {
    let i = item_index(e, p)?;
    let keys = p.as_object().map(|o| o.keys().filter(|k| !matches!(k.as_str(), "item" | "index")).count()).unwrap_or(0);
    if keys == 0 {
        return Err("Render Settings needs a setting (template, quality, resolution, proxyUse, effects, colorDepth, motionBlur, timeSpan, frameRate ...)".into());
    }
    let wa = {
        let r = &e.project.render_queue[i];
        editable(r)?;
        e.project.comp(r.comp).map(|c| (c.work_area, c.duration)).unwrap_or(([0.0, 10.0], 10.0))
    };
    let r = &mut e.proj().render_queue[i];
    r.fix();
    apply_settings(&mut r.settings, p)?;
    span_from(r, p, wa.0, wa.1);
    r.status = "Queued".into();
    Ok(json!({"item": r.id, "settings": r.settings}))
}

pub fn span_from(r: &mut RenderItem, p: &Json, wa: [f64; 2], dur: f64) {
    let (s, en, d) = (pf(p, "start"), pf(p, "end"), pf(p, "duration"));
    if s.is_some() || en.is_some() || d.is_some() {
        r.settings.time_span = "custom".into();
        let st = s.unwrap_or(r.start).max(0.0);
        r.start = st;
        r.end = en.or(d.map(|x| st + x)).unwrap_or(r.end).max(st);
    } else {
        match r.settings.time_span.as_str() {
            "workArea" => (r.start, r.end) = (wa[0], wa[1]),
            "comp" => (r.start, r.end) = (0.0, dur),
            _ => {}
        }
    }
}

pub fn set_output_module(e: &mut Engine, p: &Json) -> Res {
    let i = item_index(e, p)?;
    editable(&e.project.render_queue[i])?;
    let r = &mut e.proj().render_queue[i];
    r.fix();
    let k = module_index(r, p)?;
    let keys = p.as_object().map(|o| o.keys().filter(|k| !matches!(k.as_str(), "item" | "index" | "module")).count()).unwrap_or(0);
    if keys == 0 {
        return Err("Output Module Settings needs a setting (template, format, channels, quality, bitrate, proresProfile, audio, resize, crop, postRenderAction ...)".into());
    }
    apply_module(&mut r.modules[k], p)?;
    r.status = "Queued".into();
    Ok(json!({"item": r.id, "module": k + 1, "outputModule": r.modules[k]}))
}

pub fn add_output_module(e: &mut Engine, p: &Json) -> Res {
    let i = item_index(e, p)?;
    editable(&e.project.render_queue[i])?;
    let r = &mut e.proj().render_queue[i];
    r.fix();
    let mut m = r.modules.last().cloned().unwrap_or_default();
    let stem = Path::new(&m.output).with_extension("").to_string_lossy().to_string();
    m.output = format!("{stem} {}.{}", r.modules.len() + 1, ext_of(&m.format));
    m.post_render = "none".into();
    apply_module(&mut m, p)?;
    r.modules.push(m);
    Ok(json!({"item": r.id, "module": r.modules.len()}))
}

pub fn move_item(e: &mut Engine, p: &Json) -> Res {
    let i = item_index(e, p)?;
    let to = pu(p, "to").ok_or("pass `to` (1-based position)")? as usize;
    let q = &mut e.proj().render_queue;
    let to = to.clamp(1, q.len()) - 1;
    let it = q.remove(i);
    q.insert(to, it);
    Ok(json!({"index": to + 1}))
}

pub fn duplicate(e: &mut Engine, p: &Json) -> Res {
    let i = item_index(e, p)?;
    let pj = e.proj();
    let id = pj.new_id();
    let mut it = pj.render_queue[i].clone();
    it.fix();
    it.id = id;
    it.status = "Queued".into();
    it.progress = 0.0;
    it.error = None;
    for m in it.modules.iter_mut() {
        let stem = Path::new(&m.output).with_extension("").to_string_lossy().to_string();
        let (base, seq) = match stem.find("_[#") {
            Some(k) => (stem[..k].to_string(), stem[k..].to_string()),
            None => (stem.clone(), String::new()),
        };
        m.output = format!("{base} copy{seq}.{}", ext_of(&m.format));
    }
    pj.render_queue.insert(i + 1, it);
    Ok(json!({"item": id, "index": i + 2}))
}

pub fn templates(_: &mut Engine, _: &Json) -> Res {
    let t = load();
    let u = user();
    Ok(json!({
        "renderSettings": t.render_settings.iter().map(|(k, v)| json!({"name": k, "builtIn": BUILTIN_RS.contains(&k.as_str()) && !u.render_settings.contains_key(k), "settings": v})).collect::<Vec<_>>(),
        "outputModules": t.output_modules.iter().map(|(k, v)| json!({"name": k, "builtIn": builtin().output_modules.contains_key(k) && !u.output_modules.contains_key(k), "settings": v})).collect::<Vec<_>>(),
        "defaults": t.defaults,
    }))
}

pub fn save_template(e: &mut Engine, p: &Json) -> Res {
    let kind = pick(p, "kind", &["renderSettings", "outputModule"])?.ok_or("pass `kind`: renderSettings|outputModule")?;
    let name = ps(p, "name").filter(|n| !n.trim().is_empty()).ok_or("pass `name`")?.trim().to_string();
    let mut u = user();
    let src_item = if p.get("item").is_some() || p.get("index").is_some() { Some(item_index(e, p)?) } else { None };
    let params = p.get("params").cloned().unwrap_or(json!({}));
    if kind == "renderSettings" {
        let mut s = match (ps(p, "from"), src_item) {
            (Some(f), _) => settings_template(f).ok_or_else(|| format!("no Render Settings template `{f}`"))?,
            (None, Some(i)) => e.project.render_queue[i].settings.clone(),
            _ => RenderSettings::default(),
        };
        apply_settings(&mut s, &params)?;
        s.template = name.clone();
        u.render_settings.insert(name.clone(), s);
    } else {
        let mut m = match (ps(p, "from"), src_item) {
            (Some(f), _) => module_template(f).ok_or_else(|| format!("no Output Module template `{f}`"))?,
            (None, Some(i)) => {
                let mut it = e.project.render_queue[i].clone();
                it.fix();
                it.modules[module_index(&it, p)?].clone()
            }
            _ => OutputModule::default(),
        };
        apply_module(&mut m, &params)?;
        m.template = name.clone();
        m.output.clear();
        u.output_modules.insert(name.clone(), m);
    }
    save_user(&u)?;
    Ok(json!({"saved": name, "kind": kind}))
}

pub fn delete_template(_: &mut Engine, p: &Json) -> Res {
    let kind = pick(p, "kind", &["renderSettings", "outputModule"])?.ok_or("pass `kind`: renderSettings|outputModule")?;
    let name = ps(p, "name").ok_or("pass `name`")?;
    let mut u = user();
    let gone = if kind == "renderSettings" { u.render_settings.remove(name).is_some() } else { u.output_modules.remove(name).is_some() };
    if !gone {
        return Err(if builtin().render_settings.contains_key(name) || builtin().output_modules.contains_key(name) { format!("`{name}` is built in and cannot be deleted") } else { format!("no saved template `{name}`") });
    }
    u.defaults.retain(|k, v| !(k.starts_with(kind) && v == name));
    save_user(&u)?;
    Ok(json!({"deleted": name}))
}

pub fn set_template_default(_: &mut Engine, p: &Json) -> Res {
    let kind = pick(p, "kind", &["renderSettings", "outputModule"])?.ok_or("pass `kind`: renderSettings|outputModule")?;
    let slot = pick(p, "slot", &["movie", "still", "preRender", "movieProxy", "stillProxy"])?.ok_or("pass `slot`: movie|still|preRender|movieProxy|stillProxy")?;
    let name = ps(p, "name").ok_or("pass `name`")?;
    let t = load();
    let exists = if kind == "renderSettings" { t.render_settings.contains_key(name) } else { t.output_modules.contains_key(name) };
    if !exists {
        return Err(format!("no {kind} template `{name}`"));
    }
    let mut u = user();
    u.defaults.insert(format!("{kind}.{slot}"), name.to_string());
    save_user(&u)?;
    Ok(json!({"kind": kind, "slot": slot, "name": name}))
}

pub fn apply_template(e: &mut Engine, p: &Json) -> Res {
    let i = item_index(e, p)?;
    editable(&e.project.render_queue[i])?;
    let rs = ps(p, "renderSettings").map(|n| settings_template(n).ok_or_else(|| format!("no Render Settings template `{n}`"))).transpose()?;
    let om = ps(p, "outputModule").map(|n| module_template(n).ok_or_else(|| format!("no Output Module template `{n}`"))).transpose()?;
    if rs.is_none() && om.is_none() {
        return Err("pass `renderSettings` or `outputModule` (a template name)".into());
    }
    let wa = e.project.comp(e.project.render_queue[i].comp).map(|c| (c.work_area, c.duration)).unwrap_or(([0.0, 10.0], 10.0));
    let r = &mut e.proj().render_queue[i];
    r.fix();
    if let Some(s) = rs {
        r.settings = s;
        span_from(r, &json!({}), wa.0, wa.1);
    }
    if let Some(m) = om {
        let k = module_index(r, p)?;
        let out = r.modules[k].output.clone();
        r.modules[k] = m;
        r.modules[k].output = out;
        fix_ext(&mut r.modules[k]);
    }
    Ok(json!({"item": r.id}))
}

pub fn set_log(e: &mut Engine, p: &Json) -> Res {
    let i = item_index(e, p)?;
    let v = pick(p, "log", &["errorsOnly", "plusSettings", "plusPerFrameInfo"])?.ok_or("pass `log`: errorsOnly|plusSettings|plusPerFrameInfo")?;
    e.proj().render_queue[i].log = v.into();
    Ok(json!({"log": v}))
}

pub fn set_notify(_: &mut Engine, p: &Json) -> Res {
    let v = pb(p, "notify").unwrap_or(!notify_on());
    NOTIFY.store(v, Ordering::SeqCst);
    Ok(json!({"notify": v}))
}

pub fn set_overflow(_: &mut Engine, p: &Json) -> Res {
    let folders: Vec<String> = p.get("folders").and_then(Json::as_array).ok_or("pass `folders` (paths, used in order)")?.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
    *OVERFLOW.lock() = folders.clone();
    Ok(json!({"folders": folders}))
}

pub fn pre_render(e: &mut Engine, p: &Json) -> Res {
    let mut q = p.clone();
    if let Some(o) = q.as_object_mut() {
        o.insert("slot".into(), json!("preRender"));
        o.entry("postRenderAction").or_insert(json!("importAndReplace"));
    }
    super::export::add(e, &q)
}

pub fn save_current_preview(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (.mp4)")?.to_string();
    let cid = e.comp_id(p.get("comp"))?;
    let c = e.project.comp(cid).ok_or("no composition")?;
    let mut it = RenderItem { id: 0, comp: cid, start: c.work_area[0], end: c.work_area[1].max(c.work_area[0] + c.frame_duration()), ..RenderItem::default() };
    it.settings = settings_template("Current Settings").unwrap_or_default();
    it.settings.resolution = pf(p, "resolution").unwrap_or(1.0).clamp(0.05, 1.0);
    let mut m = OutputModule { format: "h264".into(), output: path.clone(), ..OutputModule::default() };
    if path.to_ascii_lowercase().ends_with(".mov") {
        m.format = "prores".into();
    }
    it.modules.push(m);
    let progress = Mutex::new(Progress::default());
    let stop = AtomicBool::new(false);
    let out = render_item(&e.project, &e.media, &it, &progress, &stop, if e.multi_frame { rayon::current_num_threads().clamp(1, 16) } else { 1 })?;
    Ok(json!({"path": out}))
}

pub fn item_json(p: &Project, r: &RenderItem, index: usize) -> Json {
    let mut r = r.clone();
    r.fix();
    let name = p.item(r.comp).map(|i| i.name.clone()).unwrap_or_default();
    let m0 = r.modules[0].clone();
    json!({
        "id": r.id, "index": index + 1, "comp": r.comp, "compName": name, "render": r.render, "status": r.status, "progress": r.progress, "error": r.error,
        "start": r.start, "end": r.end, "log": r.log, "settings": r.settings,
        "modules": r.modules.iter().enumerate().map(|(k, m)| json!({"module": k + 1, "template": m.template, "format": m.format, "output": m.output, "channels": m.channels, "outputSpace": m.output_space, "depth": m.depth, "postRenderAction": m.post_render, "settings": m})).collect::<Vec<_>>(),
        "format": m0.format, "output": m0.output, "outputSpace": m0.output_space, "depth": m0.depth, "template": r.settings.template, "outputModule": m0.template,
    })
}

pub fn prepared(project: &Project, it: &RenderItem) -> (Project, RenderOpts) {
    let s = &it.settings;
    let mut pj = project.clone();
    if s.color_depth > 0 {
        pj.settings.bit_depth = s.color_depth;
    }
    for item in pj.items.iter_mut() {
        if let Some(px) = item.proxy.as_mut() {
            match s.proxy_use.as_str() {
                "all" => px.enabled = true,
                "none" => px.enabled = false,
                "comp" => px.enabled = matches!(item.kind, ItemKind::Comp(_)),
                _ => {}
            }
        }
        if let ItemKind::Comp(c) = &mut item.kind {
            match s.frame_blending.as_str() {
                "offForAll" => c.frame_blending = false,
                "onForChecked" => c.frame_blending = true,
                _ => {}
            }
            if s.motion_blur == "onForChecked" {
                c.motion_blur = true;
            }
            for l in c.layers.iter_mut() {
                match s.effects.as_str() {
                    "allOn" => l.switches.effects = true,
                    "allOff" => l.switches.effects = false,
                    _ => {}
                }
                if s.solo == "allOff" {
                    l.switches.solo = false;
                }
            }
        }
    }
    let opts = RenderOpts {
        scale: s.resolution.clamp(0.05, 1.0),
        guides: s.guide_layers == "current",
        draft: s.quality == "draft",
        motion_blur: s.motion_blur != "offForAll",
        roi: None,
        view: None,
        backend: crate::render::gpu::Backend::Auto,
    };
    (pj, opts)
}

fn tokens(template: &str, comp: &str, m: &OutputModule, s: &RenderSettings, w: usize, h: usize, fps: f64, project: &str) -> String {
    let mut out = template.to_string();
    for (k, v) in [
        ("[compName]", comp.to_string()),
        ("[fileExtension]", ext_of(&m.format).to_string()),
        ("[projectName]", project.to_string()),
        ("[renderSettingsName]", s.template.clone()),
        ("[outputModuleName]", m.template.clone()),
        ("[width]", w.to_string()),
        ("[height]", h.to_string()),
        ("[frameRate]", format!("{}", (fps * 1000.0).round() / 1000.0)),
    ] {
        out = out.replace(k, &v);
    }
    if !out.is_empty() && Path::new(&out).parent().is_none_or(|d| d.as_os_str().is_empty()) {
        return renders_dir().join(out).to_string_lossy().to_string();
    }
    out
}

pub fn seq_path(template: &str, n: usize) -> String {
    if let (Some(a), Some(b)) = (template.find("[#"), template.rfind("#]"))
        && b > a
    {
        let w = template[a + 1..b + 1].chars().filter(|c| *c == '#').count().max(1);
        return format!("{}{:0w$}{}", &template[..a], n, &template[b + 2..], w = w);
    }
    let p = Path::new(template);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("frame");
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("png");
    p.with_file_name(format!("{stem}_{n:05}.{ext}")).to_string_lossy().to_string()
}

fn encoders() -> &'static String {
    static E: OnceLock<String> = OnceLock::new();
    E.get_or_init(|| Command::new(media::tool("ffmpeg")).args(["-hide_banner", "-encoders"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default())
}

fn av1_encoder() -> Option<&'static str> {
    let e = encoders();
    ["libsvtav1", "libaom-av1", "librav1e"].into_iter().find(|n| e.contains(n))
}

fn ext80(v: f64) -> [u8; 10] {
    let mut out = [0u8; 10];
    if v <= 0.0 {
        return out;
    }
    let e = v.log2().floor() as i32;
    let m = v / 2f64.powi(e);
    let exp = (e + 16383) as u16;
    out[0..2].copy_from_slice(&exp.to_be_bytes());
    let mant = (m * (1u64 << 63) as f64) as u64;
    out[2..10].copy_from_slice(&mant.to_be_bytes());
    out
}

pub fn write_pcm(path: &str, pcm: &[f32], rate: u32, mono: bool, bits: u32, aiff: bool) -> Result<(), String> {
    let ch: u16 = if mono { 1 } else { 2 };
    let samples: Vec<f32> = if mono { pcm.chunks(2).map(|c| (c[0] + c.get(1).copied().unwrap_or(c[0])) * 0.5).collect() } else { pcm.to_vec() };
    let float = bits == 32 && !aiff;
    let bps = (bits / 8) as usize;
    let mut data = Vec::with_capacity(samples.len() * bps);
    for s in &samples {
        let v = s.clamp(-1.0, 1.0);
        match (bits, float) {
            (32, true) => data.extend_from_slice(&v.to_le_bytes()),
            (32, false) => {
                let i = (v as f64 * 2147483647.0) as i32;
                data.extend_from_slice(&if aiff { i.to_be_bytes() } else { i.to_le_bytes() });
            }
            (24, _) => {
                let i = (v * 8388607.0) as i32;
                let b = i.to_le_bytes();
                if aiff {
                    data.extend_from_slice(&[b[2], b[1], b[0]]);
                } else {
                    data.extend_from_slice(&b[..3]);
                }
            }
            _ => {
                let i = (v * 32767.0) as i16;
                data.extend_from_slice(&if aiff { i.to_be_bytes() } else { i.to_le_bytes() });
            }
        }
    }
    let mut out = vec![];
    if aiff {
        let frames = (samples.len() / ch as usize) as u32;
        let ssnd = 8 + data.len();
        out.extend_from_slice(b"FORM");
        out.extend_from_slice(&((4 + 26 + 8 + ssnd) as u32).to_be_bytes());
        out.extend_from_slice(b"AIFFCOMM");
        out.extend_from_slice(&18u32.to_be_bytes());
        out.extend_from_slice(&ch.to_be_bytes());
        out.extend_from_slice(&frames.to_be_bytes());
        out.extend_from_slice(&(bits as u16).to_be_bytes());
        out.extend_from_slice(&ext80(rate as f64));
        out.extend_from_slice(b"SSND");
        out.extend_from_slice(&(ssnd as u32).to_be_bytes());
        out.extend_from_slice(&[0; 8]);
        out.extend(data);
        if out.len() % 2 == 1 {
            out.push(0);
        }
    } else {
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&(if float { 3u16 } else { 1u16 }).to_le_bytes());
        out.extend_from_slice(&ch.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * ch as u32 * bps as u32).to_le_bytes());
        out.extend_from_slice(&((ch as usize * bps) as u16).to_le_bytes());
        out.extend_from_slice(&(bits as u16).to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend(data);
    }
    std::fs::write(path, out).map_err(|e| format!("{path}: {e}"))
}

fn crop_resize(img: &Img, m: &OutputModule) -> Img {
    let [t, l, b, r] = m.crop;
    let mut cur = if t + l + b + r > 0 {
        let w = (img.w as i32 - l - r).max(1) as usize;
        let h = (img.h as i32 - t - b).max(1) as usize;
        img.crop(l as isize, t as isize, w, h)
    } else {
        img.clone()
    };
    if let Some([w, h]) = m.resize
        && (w as usize != cur.w || h as usize != cur.h)
    {
        let mut out = Img::new(w as usize, h as usize);
        let (kx, ky) = (cur.w as f64 / w as f64, cur.h as f64 / h as f64);
        for y in 0..out.h {
            for x in 0..out.w {
                let px = cur.sample((x as f64 + 0.5) * kx - 0.5, (y as f64 + 0.5) * ky - 0.5, true);
                out.px[(y * out.w + x) * 4..(y * out.w + x) * 4 + 4].copy_from_slice(&px);
            }
        }
        cur = out;
    }
    cur
}

fn bytes(img: &Img, premul: bool, deep: bool) -> Vec<u8> {
    if !premul {
        return if deep { img.to_rgba16().iter().flat_map(|v| v.to_le_bytes()).collect() } else { img.to_rgba8() };
    }
    if deep {
        img.px.iter().flat_map(|v| ((v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16).to_le_bytes()).collect()
    } else {
        img.px.iter().map(|v| crate::render::image::to8(*v)).collect()
    }
}

struct Pipe {
    child: Child,
    stdin: Option<ChildStdin>,
    deep: bool,
    w: usize,
    h: usize,
}

enum Sink {
    Seq,
    Pipe(Pipe),
    Audio,
    Skip,
}

struct Writer {
    m: OutputModule,
    path: String,
    proc: Option<std::sync::Arc<crate::render::ocio::Proc>>,
    bg: [f32; 3],
    sink: Sink,
    skip_existing: bool,
    wav: Option<PathBuf>,
}

impl Writer {
    fn shape(&self, img: &Img) -> Img {
        let mut cur = crop_resize(img, &self.m);
        match self.m.channels.as_str() {
            "alpha" => {
                for p in cur.px.chunks_mut(4) {
                    let a = p[3].clamp(0.0, 1.0);
                    p[0] = a;
                    p[1] = a;
                    p[2] = a;
                    p[3] = 1.0;
                }
                return cur;
            }
            "rgba" => {}
            _ => cur = cur.flattened(self.bg),
        }
        if let Some(p) = &self.proc {
            p.apply_img(&mut cur);
        }
        cur
    }

    fn push(&mut self, img: &Img, n: usize, overflow: &[String]) -> Result<(), String> {
        let premul = self.m.color == "premultiplied" && self.m.channels == "rgba";
        if matches!(self.sink, Sink::Pipe(_)) {
            let shown = self.shape(img);
            let Sink::Pipe(p) = &mut self.sink else { return Ok(()) };
            let shown = if shown.w != p.w || shown.h != p.h { shown.crop(0, 0, p.w, p.h) } else { shown };
            let data = bytes(&shown, premul, p.deep);
            let r = p.stdin.as_mut().ok_or("FFmpeg stdin")?.write_all(&data);
            if let Err(e) = r {
                if let Sink::Pipe(mut p) = std::mem::replace(&mut self.sink, Sink::Skip) {
                    drop(p.stdin.take());
                    if let Ok(out) = p.child.wait_with_output() {
                        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
                        if !msg.is_empty() {
                            return Err(format!("FFmpeg: {msg}"));
                        }
                    }
                }
                return Err(format!("FFmpeg stopped: {e}"));
            }
            return Ok(());
        }
        match &mut self.sink {
            Sink::Skip | Sink::Audio => Ok(()),
            Sink::Seq => {
                let mut path = seq_path(&self.path, n);
                if self.skip_existing && Path::new(&path).exists() {
                    return Ok(());
                }
                let shown = self.shape(img);
                let mut tries: Vec<Option<&String>> = vec![None];
                tries.extend(overflow.iter().map(Some));
                let mut last = String::new();
                for t in tries {
                    if let Some(dir) = t {
                        path = Path::new(dir).join(Path::new(&path).file_name().unwrap_or_default()).to_string_lossy().to_string();
                    }
                    let r = write_still(&self.m, &shown, &path, premul);
                    match r {
                        Ok(()) => return Ok(()),
                        Err(e) => last = e,
                    }
                }
                Err(last)
            }
            Sink::Pipe(_) => Ok(()),
        }
    }

    fn finish(mut self) -> Result<String, String> {
        if let Sink::Pipe(mut p) = std::mem::replace(&mut self.sink, Sink::Skip) {
            drop(p.stdin.take());
            let out = p.child.wait_with_output().map_err(|e| e.to_string())?;
            if let Some(a) = &self.wav {
                let _ = std::fs::remove_file(a);
            }
            if !out.status.success() {
                return Err(format!("FFmpeg: {}", String::from_utf8_lossy(&out.stderr).trim()));
            }
        }
        Ok(self.path)
    }

    fn abort(mut self) {
        if let Sink::Pipe(mut p) = std::mem::replace(&mut self.sink, Sink::Skip) {
            drop(p.stdin.take());
            let _ = p.child.kill();
            let _ = p.child.wait();
        }
        if let Some(a) = &self.wav {
            let _ = std::fs::remove_file(a);
        }
    }
}

fn write_still(m: &OutputModule, img: &Img, path: &str, premul: bool) -> Result<(), String> {
    if let Some(dir) = Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let (w, h) = (img.w as u32, img.h as u32);
    match m.format.as_str() {
        "exr" => exr::write(path, &[(String::new(), img)], m.depth < 32),
        "jpeg" => {
            let buf = image::RgbaImage::from_raw(w, h, img.to_rgba8()).ok_or("frame")?;
            image::DynamicImage::ImageRgba8(buf).to_rgb8().save(path).map_err(|e| format!("{path}: {e}"))
        }
        _ if m.depth >= 16 => {
            let v: Vec<u16> = bytes(img, premul, true).chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
            let buf = image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::from_raw(w, h, v).ok_or("frame")?;
            if m.channels == "rgba" {
                buf.save(path).map_err(|e| format!("{path}: {e}"))
            } else {
                image::DynamicImage::ImageRgba16(buf).to_rgb16().save(path).map_err(|e| format!("{path}: {e}"))
            }
        }
        _ => {
            let buf = image::RgbaImage::from_raw(w, h, bytes(img, premul, false)).ok_or("frame")?;
            if m.channels == "rgba" {
                buf.save(path).map_err(|e| format!("{path}: {e}"))
            } else {
                image::DynamicImage::ImageRgba8(buf).to_rgb8().save(path).map_err(|e| format!("{path}: {e}"))
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn open_writer(m: &OutputModule, path: String, r: &Renderer, comp: &Comp, comp_id: u64, project: &Project, media: &Media, span: (f64, f64), w: usize, h: usize, fps: f64, fields: &str, skip_existing: bool, id: u64) -> Result<Writer, String> {
    let space = if m.output_space.is_empty() { project.settings.output_space.clone() } else { m.output_space.clone() };
    let proc = if m.format == "exr" { r.pipe.scene_linear() } else { r.pipe.output(&space)? };
    let mut bg = comp.background;
    if let Some(a) = &r.pipe.authored {
        bg = a.apply32(bg);
    }
    if let Some(dir) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut wr = Writer { m: m.clone(), path: path.clone(), proc, bg, sink: Sink::Seq, skip_existing, wav: None };
    let has_audio = audio::comp_has_audio(project, comp_id);
    let wants_audio = match m.audio.as_str() {
        "off" => false,
        "on" => true,
        _ => has_audio,
    };
    let mono = m.audio_channels == "mono";
    if is_audio(&m.format) {
        let frames = ((span.1 - span.0) * m.sample_rate as f64) as usize;
        let pcm = audio::mix_comp(project, media, comp_id, span.0, frames, m.sample_rate);
        write_pcm(&path, &pcm, m.sample_rate, mono, m.audio_format, m.format == "aiff")?;
        wr.sink = Sink::Audio;
        return Ok(wr);
    }
    if is_seq(&m.format) {
        return Ok(wr);
    }
    if skip_existing && Path::new(&path).exists() {
        wr.sink = Sink::Skip;
        return Ok(wr);
    }
    if !media::has_ffmpeg() {
        return Err("movie formats need FFmpeg on PATH (or set FORMVFX_FFMPEG); PNG, JPEG, TIFF and EXR sequences, WAV and AIFF render without it".into());
    }
    let main10 = m.profile == "main10";
    let deep = match m.format.as_str() {
        "prores" => true,
        "hevc" | "av1" => main10 || m.depth >= 16,
        _ => false,
    };
    let interlaced = fields != "off";
    let (w2, h2) = (w + (w & 1), if interlaced { h.next_multiple_of(4) } else { h + (h & 1) });
    let field_vf = if fields == "upper" { "setfield=tff" } else { "setfield=bff" };
    if wants_audio && matches!(m.format.as_str(), "h264" | "hevc" | "av1" | "prores" | "webm") {
        let frames = ((span.1 - span.0) * m.sample_rate as f64) as usize;
        let pcm = audio::mix_comp(project, media, comp_id, span.0, frames, m.sample_rate);
        let p = std::env::temp_dir().join(format!("formvfx-audio-{id}-{}.wav", std::process::id()));
        write_pcm(&p.to_string_lossy(), &pcm, m.sample_rate, mono, 32, false)?;
        wr.wav = Some(p);
    }
    let q = m.quality as f64 / 100.0;
    let crf = |lo: f64, hi: f64| (hi - q * (hi - lo)).round().to_string();
    let rate = |cmd: &mut Command, lo: f64, hi: f64| {
        if m.bitrate > 0 {
            cmd.args(["-b:v", &format!("{}k", m.bitrate), "-maxrate", &format!("{}k", m.bitrate * 3 / 2), "-bufsize", &format!("{}k", m.bitrate * 2)]);
        } else {
            cmd.args(["-crf", &crf(lo, hi)]);
        }
    };
    let mut cmd = Command::new(media::tool("ffmpeg"));
    cmd.args(["-y", "-v", "error", "-f", "rawvideo", "-pix_fmt", if deep { "rgba64le" } else { "rgba" }, "-s", &format!("{w}x{h}"), "-r", &format!("{fps}"), "-i", "-"]);
    if let Some(a) = &wr.wav {
        cmd.args(["-i", &a.to_string_lossy()]);
    }
    let mut pad = if w2 != w || h2 != h { format!("pad={w2}:{h2}") } else { "null".into() };
    if interlaced {
        pad = format!("{pad},{field_vf}");
    }
    let gop = if m.keyframe_interval > 0 { Some(m.keyframe_interval.to_string()) } else { None };
    let aac = |cmd: &mut Command| {
        cmd.args(["-c:a", "aac", "-b:a", &format!("{}k", m.audio_bitrate), "-ar", &m.sample_rate.to_string(), "-ac", if mono { "1" } else { "2" }, "-shortest"]);
    };
    match m.format.as_str() {
        "h264" => {
            cmd.args(["-vf", &pad, "-c:v", "libx264", "-preset", "medium"]);
            rate(&mut cmd, 14.0, 32.0);
            cmd.args(["-pix_fmt", "yuv420p", "-movflags", "+faststart"]);
            if m.profile == "high" {
                cmd.args(["-profile:v", "high"]);
            }
            if interlaced {
                cmd.args(["-flags", "+ildct+ilme"]);
            }
            if wr.wav.is_some() {
                aac(&mut cmd);
            }
        }
        "hevc" => {
            cmd.args(["-vf", &pad, "-c:v", "libx265"]);
            rate(&mut cmd, 16.0, 34.0);
            cmd.args(["-pix_fmt", if deep { "yuv420p10le" } else { "yuv420p" }, "-tag:v", "hvc1"]);
            if wr.wav.is_some() {
                aac(&mut cmd);
            }
        }
        "av1" => {
            let enc = av1_encoder().ok_or("this FFmpeg has no AV1 encoder (libsvtav1, libaom-av1 or librav1e)")?;
            cmd.args(["-vf", &pad, "-c:v", enc]);
            if enc == "libaom-av1" {
                cmd.args(["-cpu-used", "6", "-row-mt", "1"]);
            }
            if enc == "librav1e" {
                cmd.args(["-qp", &((1.0 - q) * 200.0 + 40.0).round().to_string()]);
            } else {
                rate(&mut cmd, 18.0, 50.0);
            }
            cmd.args(["-pix_fmt", if deep { "yuv420p10le" } else { "yuv420p" }, "-movflags", "+faststart"]);
            if wr.wav.is_some() {
                aac(&mut cmd);
            }
        }
        "prores" => {
            let prof = match m.prores_profile.as_str() {
                "proxy" => "0",
                "lt" => "1",
                "standard" => "2",
                "4444" => "4",
                "4444xq" => "5",
                _ => "3",
            };
            let four = prof == "4" || prof == "5";
            let pix = if four && m.channels == "rgba" {
                "yuva444p10le"
            } else if four {
                "yuv444p10le"
            } else {
                "yuv422p10le"
            };
            cmd.args(["-c:v", "prores_ks", "-profile:v", prof, "-pix_fmt", pix, "-vendor", "apl0"]);
            if four && m.channels == "rgba" {
                cmd.args(["-alpha_bits", "16"]);
            }
            if interlaced {
                cmd.args(["-vf", field_vf, "-flags", "+ildct+ilme"]);
            }
            if wr.wav.is_some() {
                let pcm = match m.audio_format {
                    24 => "pcm_s24le",
                    32 => "pcm_s32le",
                    _ => "pcm_s16le",
                };
                cmd.args(["-c:a", pcm, "-ar", &m.sample_rate.to_string(), "-ac", if mono { "1" } else { "2" }, "-shortest"]);
            }
        }
        "webm" => {
            if m.webm_codec == "av1" {
                let enc = av1_encoder().ok_or("this FFmpeg has no AV1 encoder (libsvtav1, libaom-av1 or librav1e)")?;
                cmd.args(["-vf", &pad, "-c:v", enc]);
                rate(&mut cmd, 18.0, 50.0);
                cmd.args(["-pix_fmt", "yuv420p"]);
            } else {
                cmd.args(["-c:v", "libvpx-vp9", "-row-mt", "1"]);
                if m.bitrate > 0 {
                    cmd.args(["-b:v", &format!("{}k", m.bitrate)]);
                } else {
                    cmd.args(["-b:v", "0", "-crf", &crf(18.0, 45.0)]);
                }
                cmd.args(["-pix_fmt", if m.channels == "rgba" { "yuva420p" } else { "yuv420p" }]);
            }
            if wr.wav.is_some() {
                cmd.args(["-c:a", "libopus", "-b:a", &format!("{}k", m.audio_bitrate.min(510)), "-ac", if mono { "1" } else { "2" }, "-shortest"]);
            }
        }
        _ => {
            cmd.args(["-vf", "split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer", "-loop", if m.loop_gif { "0" } else { "-1" }]);
        }
    }
    if let Some(g) = &gop
        && m.format != "gif"
    {
        cmd.args(["-g", g]);
    }
    if m.format != "gif" {
        cmd.args(color_args(&space));
    }
    cmd.arg(&path);
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| format!("could not start FFmpeg: {e}"))?;
    let stdin = child.stdin.take();
    wr.sink = Sink::Pipe(Pipe { child, stdin, deep, w, h });
    Ok(wr)
}

fn interleave(a: &Img, b: &Img, upper: bool) -> Img {
    let mut out = a.clone();
    for y in 0..out.h {
        let first = (y % 2 == 0) == upper;
        if !first {
            let s = y * b.w * 4;
            out.px[s..s + b.w * 4].copy_from_slice(&b.px[s..s + b.w * 4]);
        }
    }
    out
}

const PULL: [(usize, usize); 5] = [(0, 0), (1, 1), (1, 2), (2, 3), (3, 3)];

fn pull_rot(phase: &str) -> usize {
    let base = ['W', 'W', 'S', 'S', 'W'];
    let want: Vec<char> = phase.chars().collect();
    (0..5).find(|r| (0..5).all(|i| want.get(i) == Some(&base[(i + r) % 5]))).unwrap_or(0)
}

enum Plan {
    Frame(f64),
    Fields(f64, f64),
}

fn plan(o: usize, start: f64, src_fps: f64, s: &RenderSettings) -> Plan {
    if s.pulldown != "off" {
        let r = pull_rot(&s.pulldown);
        let k = o + r;
        let (g, j) = (k / 5, k % 5);
        let (a, b) = PULL[j];
        let base = (g * 4) as i64 - PULL[r].0 as i64;
        let fa = (base + a as i64).max(0) as f64;
        let fb = (base + b as i64).max(0) as f64;
        return if a == b { Plan::Frame(start + fa / src_fps) } else { Plan::Fields(start + fa / src_fps, start + fb / src_fps) };
    }
    let t = start + o as f64 / src_fps;
    if s.field_render != "off" { Plan::Fields(t, t + 0.5 / src_fps) } else { Plan::Frame(t) }
}

fn blank(img: &Img) -> bool {
    img.px.iter().all(|v| *v == 0.0)
}

pub fn render_item(project: &Project, media: &Media, it: &RenderItem, progress: &Mutex<Progress>, stop: &AtomicBool, batch: usize) -> Result<String, String> {
    let mut it = it.clone();
    it.fix();
    let (pj, opts) = prepared(project, &it);
    let comp = pj.comp(it.comp).ok_or("the composition is gone")?.clone();
    let cname = pj.item(it.comp).map(|i| i.name.clone()).unwrap_or_default();
    let s = it.settings.clone();
    let src_fps = if s.frame_rate > 0.0 { s.frame_rate } else { comp.frame_rate };
    let pulled = s.pulldown != "off";
    let out_fps = if pulled { src_fps * 5.0 / 4.0 } else { src_fps };
    let (start, end) = (it.start, it.end.max(it.start + 1.0 / src_fps));
    let total = (((end - start) * out_fps) - 1e-6).ceil().max(1.0) as usize;
    let n0 = (start * out_fps + 1e-6).round() as usize;
    progress.lock().total = total;
    let r = Renderer::new(&pj, media, opts);
    let (fw, fh) = r.comp_size(&comp, it.settings.resolution);
    let project_name = String::new();
    let overflow: Vec<String> = if s.storage_overflow { OVERFLOW.lock().clone() } else { vec![] };
    let mut log = vec![format!("Render of {cname} started"), format!("Render settings: {}", serde_json::to_string(&s).unwrap_or_default())];
    let mut writers = vec![];
    for m in &it.modules {
        let (w, h) = {
            let cw = (fw as i32 - m.crop[1] - m.crop[3]).max(1) as usize;
            let ch = (fh as i32 - m.crop[0] - m.crop[2]).max(1) as usize;
            m.resize.map(|[a, b]| (a as usize, b as usize)).unwrap_or((cw, ch))
        };
        let path = tokens(&m.output, &cname, m, &s, w, h, out_fps, &project_name);
        if it.log != "errorsOnly" {
            log.push(format!("Output module {}: {path}", m.template));
        }
        match open_writer(m, path, &r, &comp, it.comp, &pj, media, (start, end), w, h, out_fps, if pulled { "lower" } else { &s.field_render }, s.skip_existing, it.id) {
            Ok(wr) => writers.push(wr),
            Err(e) => {
                for wr in writers {
                    wr.abort();
                }
                write_log(&it, &cname, &log, Some(&e));
                return Err(e);
            }
        }
    }
    let needs_frames = writers.iter().any(|w| !matches!(w.sink, Sink::Audio | Sink::Skip));
    if needs_frames {
        let all: Vec<usize> = (0..total).collect();
        let mut k = 0;
        for chunk in all.chunks(batch.max(1)) {
            if stop.load(Ordering::SeqCst) {
                for wr in writers {
                    wr.abort();
                }
                write_log(&it, &cname, &log, Some("stopped"));
                return Err("stopped".into());
            }
            use rayon::prelude::*;
            let frame_at = |o: usize| match plan(o, start, src_fps, &s) {
                Plan::Frame(t) => r.comp_frame(it.comp, t),
                Plan::Fields(a, b) => {
                    let fa = r.comp_frame(it.comp, a);
                    let fb = r.comp_frame(it.comp, b);
                    interleave(&fa, &fb, s.field_render == "upper")
                }
            };
            let imgs: Vec<Img> = chunk.par_iter().map(|o| frame_at(*o)).collect();
            let imgs: Vec<Img> = imgs.into_iter().zip(chunk).map(|(img, o)| if blank(&img) { frame_at(*o) } else { img }).collect();
            for (img, o) in imgs.iter().zip(chunk) {
                for wr in writers.iter_mut() {
                    if let Err(e) = wr.push(img, n0 + o, &overflow) {
                        for w2 in writers {
                            w2.abort();
                        }
                        write_log(&it, &cname, &log, Some(&e));
                        return Err(e);
                    }
                }
                if it.log == "plusPerFrameInfo" {
                    log.push(format!("Frame {} rendered", n0 + o));
                }
                k += 1;
                progress.lock().done = k;
            }
        }
    }
    let mut outs = vec![];
    for wr in writers {
        match wr.finish() {
            Ok(p) => outs.push(p),
            Err(e) => {
                write_log(&it, &cname, &log, Some(&e));
                return Err(e);
            }
        }
    }
    if it.log != "errorsOnly" {
        log.push(format!("Finished: {} frames", total));
        write_log(&it, &cname, &log, None);
    }
    Ok(outs.join("\n"))
}

fn write_log(it: &RenderItem, cname: &str, lines: &[String], err: Option<&str>) {
    if it.log == "errorsOnly" && err.is_none() {
        return;
    }
    let dir = renders_dir().join("Logs");
    let _ = std::fs::create_dir_all(&dir);
    let mut text = lines.join("\n");
    if let Some(e) = err {
        text.push_str(&format!("\nError: {e}"));
    }
    let safe: String = cname.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect();
    let _ = std::fs::write(dir.join(format!("{safe} Render Log.txt")), text);
}
