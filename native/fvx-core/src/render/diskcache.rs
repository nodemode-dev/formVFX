use std::collections::BTreeSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

use parking_lot::Mutex;
use serde_json::{Map, Value as Json, json};

use crate::model::{ItemKind, Project};

pub fn data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("FORMVFX_DATA") {
        return PathBuf::from(d);
    }
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".formvfx")
}

pub fn default_folder() -> PathBuf {
    std::env::var("FORMVFX_DISK_CACHE").map(PathBuf::from).unwrap_or_else(|_| data_dir().join("disk-cache"))
}

fn prefs_path() -> PathBuf {
    data_dir().join("engine-prefs.json")
}

pub fn load_prefs() -> Map<String, Json> {
    std::fs::read_to_string(prefs_path()).ok().and_then(|t| serde_json::from_str::<Json>(&t).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

pub fn save_pref(key: &str, value: &Json) {
    let mut m = load_prefs();
    m.insert(key.to_string(), value.clone());
    let _ = std::fs::create_dir_all(data_dir());
    let _ = std::fs::write(prefs_path(), serde_json::to_string_pretty(&Json::Object(m)).unwrap_or_default());
}

pub fn remove_pref(key: &str) {
    let mut m = load_prefs();
    if m.remove(key).is_some() {
        let _ = std::fs::write(prefs_path(), serde_json::to_string_pretty(&Json::Object(m)).unwrap_or_default());
    }
}

pub fn persisted(key: &str) -> bool {
    key.starts_with("diskCache.") || key.starts_with("previews.") || key.starts_with("autoSave.") || key.starts_with("logging.")
}

struct State {
    enabled: bool,
    folder: PathBuf,
    max_bytes: u64,
    bytes: Option<u64>,
    files: u64,
    hits: u64,
    writes: u64,
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| {
        let p = load_prefs();
        let enabled = p.get("diskCache.enabled").and_then(Json::as_bool).unwrap_or(true);
        let folder = p.get("diskCache.folder").and_then(Json::as_str).filter(|s| !s.trim().is_empty()).map(PathBuf::from).unwrap_or_else(default_folder);
        let gb = p.get("diskCache.maxGB").and_then(Json::as_f64).unwrap_or(10.0).clamp(0.1, 10000.0);
        Mutex::new(State { enabled, folder, max_bytes: (gb * 1e9) as u64, bytes: None, files: 0, hits: 0, writes: 0 })
    })
}

pub fn configure(key: &str, v: &Json) {
    let mut s = state().lock();
    match key {
        "diskCache.enabled" => s.enabled = v.as_bool().unwrap_or(s.enabled),
        "diskCache.folder" => {
            s.folder = v.as_str().filter(|x| !x.trim().is_empty()).map(PathBuf::from).unwrap_or_else(default_folder);
            s.bytes = None;
        }
        "diskCache.maxGB" => s.max_bytes = (v.as_f64().unwrap_or(10.0).clamp(0.1, 10000.0) * 1e9) as u64,
        _ => {}
    }
}

pub fn value(key: &str) -> Option<Json> {
    let s = state().lock();
    match key {
        "diskCache.enabled" => Some(json!(s.enabled)),
        "diskCache.folder" => Some(json!(s.folder.to_string_lossy())),
        "diskCache.maxGB" => Some(json!(s.max_bytes as f64 / 1e9)),
        _ => None,
    }
}

pub fn enabled() -> bool {
    state().lock().enabled
}

fn file_for(folder: &Path, key: &str) -> PathBuf {
    folder.join(&key[..2.min(key.len())]).join(format!("{key}.png"))
}

fn scan(folder: &Path) -> (u64, u64, Vec<(SystemTime, u64, PathBuf)>) {
    let mut out = vec![];
    let (mut bytes, mut files) = (0u64, 0u64);
    let Ok(rd) = std::fs::read_dir(folder) else { return (0, 0, out) };
    for d in rd.flatten() {
        let Ok(sub) = std::fs::read_dir(d.path()) else { continue };
        for f in sub.flatten() {
            let p = f.path();
            if p.extension().and_then(|x| x.to_str()) != Some("png") {
                continue;
            }
            if let Ok(m) = f.metadata() {
                bytes += m.len();
                files += 1;
                out.push((m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len(), p));
            }
        }
    }
    (bytes, files, out)
}

pub fn get(key: &str) -> Option<(u32, u32, Vec<u8>)> {
    let folder = {
        let s = state().lock();
        if !s.enabled {
            return None;
        }
        s.folder.clone()
    };
    let p = file_for(&folder, key);
    let bytes = std::fs::read(&p).ok()?;
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    if let Ok(f) = std::fs::File::options().write(true).open(&p) {
        let _ = f.set_modified(SystemTime::now());
    }
    state().lock().hits += 1;
    Some((img.width(), img.height(), img.into_raw()))
}

pub fn put(key: &str, w: u32, h: u32, px: &[u8]) {
    let (folder, max) = {
        let s = state().lock();
        if !s.enabled {
            return;
        }
        (s.folder.clone(), s.max_bytes)
    };
    let p = file_for(&folder, key);
    if let Some(dir) = p.parent()
        && std::fs::create_dir_all(dir).is_err()
    {
        return;
    }
    let mut buf = Cursor::new(Vec::with_capacity(px.len() / 3));
    let enc = image::codecs::png::PngEncoder::new_with_quality(&mut buf, image::codecs::png::CompressionType::Fast, image::codecs::png::FilterType::Sub);
    if image::ImageEncoder::write_image(enc, px, w, h, image::ExtendedColorType::Rgba8).is_err() {
        return;
    }
    let data = buf.into_inner();
    let tmp = p.with_extension("tmp");
    if std::fs::write(&tmp, &data).is_err() || std::fs::rename(&tmp, &p).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    let over = {
        let mut s = state().lock();
        s.writes += 1;
        if s.bytes.is_none() {
            let (b, f, _) = scan(&folder);
            s.bytes = Some(b);
            s.files = f;
        } else {
            s.bytes = Some(s.bytes.unwrap_or(0) + data.len() as u64);
            s.files += 1;
        }
        s.bytes.unwrap_or(0) > max
    };
    if over {
        evict(&folder, max);
    }
}

fn evict(folder: &Path, max: u64) {
    let (mut bytes, mut files, mut list) = scan(folder);
    list.sort_by_key(|x| x.0);
    let target = max / 10 * 9;
    for (_, len, p) in list {
        if bytes <= target {
            break;
        }
        if std::fs::remove_file(&p).is_ok() {
            bytes = bytes.saturating_sub(len);
            files = files.saturating_sub(1);
        }
    }
    let mut s = state().lock();
    s.bytes = Some(bytes);
    s.files = files;
}

pub fn stats() -> Json {
    let folder = state().lock().folder.clone();
    let (b, f, _) = scan(&folder);
    let mut s = state().lock();
    s.bytes = Some(b);
    s.files = f;
    json!({"enabled": s.enabled, "folder": s.folder.to_string_lossy(), "bytes": b, "files": f, "maxBytes": s.max_bytes, "hits": s.hits, "writes": s.writes})
}

pub fn clean() -> u64 {
    let folder = state().lock().folder.clone();
    let (_, _, list) = scan(&folder);
    let mut freed = 0;
    for (_, len, p) in list {
        if std::fs::remove_file(&p).is_ok() {
            freed += len;
        }
    }
    let mut s = state().lock();
    s.bytes = Some(0);
    s.files = 0;
    freed
}

fn stamp(path: &str) -> Json {
    match std::fs::metadata(path) {
        Ok(m) => json!([path, m.len(), m.modified().ok().and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64).unwrap_or(0)]),
        Err(_) => json!([path, 0, 0]),
    }
}

type Prefix = Mutex<Option<(std::sync::Arc<Project>, u64, String)>>;

pub fn closure_hash(project: &std::sync::Arc<Project>, comp: u64) -> String {
    static P: OnceLock<Prefix> = OnceLock::new();
    let cache = P.get_or_init(|| Mutex::new(None));
    if let Some((p, c, h)) = cache.lock().as_ref()
        && std::sync::Arc::ptr_eq(p, project)
        && *c == comp
    {
        return h.clone();
    }
    let mut ids: BTreeSet<u64> = BTreeSet::new();
    let mut stack = vec![comp];
    while let Some(c) = stack.pop() {
        if !ids.insert(c) {
            continue;
        }
        if let Some(cc) = project.comp(c) {
            for l in &cc.layers {
                if let Some(s) = l.source {
                    if project.comp(s).is_some() {
                        stack.push(s);
                    } else {
                        ids.insert(s);
                    }
                }
            }
        }
    }
    let items: Vec<Json> = ids.iter().filter_map(|id| project.item(*id)).map(|it| serde_json::to_value(it).unwrap_or(Json::Null)).collect();
    let mut text = serde_json::to_string(&json!({"settings": project.settings, "items": items})).unwrap_or_default();
    if text.contains("comp(") || text.contains("footage(") {
        text = serde_json::to_string(&**project).unwrap_or_default();
    }
    let mut stamps: Vec<Json> = ids
        .iter()
        .filter_map(|id| project.item(*id))
        .filter_map(|it| match &it.kind {
            ItemKind::Footage(f) => Some(stamp(f.sequence.first().map(String::as_str).unwrap_or(&f.path))),
            _ => None,
        })
        .collect();
    const EXT: [&str; 11] = [".cube", ".3dl", ".spi1d", ".spi3d", ".spimtx", ".clf", ".ctf", ".cc", ".ccc", ".cdl", ".ocio"];
    for tok in text.split('"') {
        let low = tok.to_ascii_lowercase();
        if tok.len() < 300 && EXT.iter().any(|x| low.ends_with(x)) {
            stamps.push(stamp(tok));
        }
    }
    text.push_str(&serde_json::to_string(&stamps).unwrap_or_default());
    text.push_str(env!("CARGO_PKG_VERSION"));
    let h = super::roto::sha256_hex(text.as_bytes());
    *cache.lock() = Some((project.clone(), comp, h.clone()));
    h
}
