use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pb, ps};
use crate::render::diskcache::data_dir;
use crate::render::media;

const ACTIONS: [(&str, &str); 4] = [("refresh", "Refresh"), ("home", "Go to Home Folder"), ("reveal", "Reveal in Finder"), ("importAll", "Import Everything Here")];

#[derive(Default)]
pub struct Browser {
    pub path: Option<PathBuf>,
    pub favorites: Vec<String>,
    pub importable_only: bool,
    loaded: bool,
}

fn store() -> PathBuf {
    data_dir().join("media-browser.json")
}

fn home() -> PathBuf {
    std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/"))
}

fn load(e: &mut Engine) {
    let b = &mut e.session.browser;
    if b.loaded {
        return;
    }
    b.loaded = true;
    let j: Json = std::fs::read_to_string(store()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Json::Null);
    b.favorites = j.get("favorites").and_then(Json::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    b.path = j.get("path").and_then(Json::as_str).map(PathBuf::from).filter(|p| p.is_dir());
    b.importable_only = j.get("importableOnly").and_then(Json::as_bool).unwrap_or(false);
}

fn save(e: &Engine) {
    let b = &e.session.browser;
    let _ = std::fs::create_dir_all(data_dir());
    let _ = std::fs::write(store(), serde_json::to_string_pretty(&json!({"favorites": b.favorites, "path": b.path, "importableOnly": b.importable_only})).unwrap_or_default());
}

fn importable(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    media::is_media(path) || [".fvxproj", ".fvxprojx", ".ecproj", ".ecprojx", ".lottie", ".json", ".xml", ".fcpxml", ".otio", ".edl"].iter().any(|x| lower.ends_with(x))
}

fn split_number(stem: &str) -> Option<(&str, &str)> {
    let digits = stem.bytes().rev().take_while(u8::is_ascii_digit).count();
    (digits > 0).then(|| stem.split_at(stem.len() - digits))
}

fn still(path: &Path) -> bool {
    let e = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    media::IMAGE_EXT.contains(&e.as_str()) && !matches!(e.as_str(), "gif" | "psd" | "psb")
}

fn modified(m: &std::fs::Metadata) -> Option<f64> {
    m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs_f64())
}

pub fn listing(dir: &Path, only: bool) -> Result<Vec<Json>, String> {
    let rd = std::fs::read_dir(dir).map_err(|x| format!("{}: {x}", dir.display()))?;
    let mut folders = vec![];
    let mut files: Vec<(PathBuf, std::fs::Metadata)> = vec![];
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = ent.metadata() else { continue };
        let p = ent.path();
        if meta.is_dir() {
            folders.push(json!({"name": name, "path": p.to_string_lossy(), "kind": "folder", "importable": true}));
        } else {
            files.push((p, meta));
        }
    }
    let mut groups: BTreeMap<(String, String, String), Vec<(u64, usize)>> = BTreeMap::new();
    for (i, (p, _)) in files.iter().enumerate() {
        if !still(p) {
            continue;
        }
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if let Some((pre, num)) = split_number(stem) {
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
            groups.entry((pre.to_string(), ext, num.len().to_string())).or_default().push((num.parse().unwrap_or(0), i));
        }
    }
    let mut taken = vec![false; files.len()];
    let mut entries = vec![];
    for ((pre, ext, _), mut members) in groups {
        if members.len() < 2 {
            continue;
        }
        members.sort();
        let first = &files[members[0].1].0;
        let bytes: u64 = members.iter().map(|m| files[m.1].1.len()).sum();
        for m in &members {
            taken[m.1] = true;
        }
        let num = |i: usize| files[i].0.file_stem().and_then(|s| s.to_str()).map(|s| s[pre.len()..].to_string()).unwrap_or_default();
        let (a, b) = (num(members[0].1), num(members[members.len() - 1].1));
        entries.push(json!({
            "name": format!("{pre}[{a}-{b}].{ext}"),
            "path": first.to_string_lossy(),
            "kind": "sequence",
            "frames": members.len(),
            "size": bytes,
            "modified": modified(&files[members[0].1].1),
            "importable": true,
        }));
    }
    for (i, (p, meta)) in files.iter().enumerate() {
        if taken[i] {
            continue;
        }
        let s = p.to_string_lossy().to_string();
        let ok = importable(&s);
        if only && !ok {
            continue;
        }
        entries.push(json!({"name": p.file_name().and_then(|n| n.to_str()).unwrap_or(""), "path": s, "kind": "file", "size": meta.len(), "modified": modified(meta), "importable": ok}));
    }
    entries.sort_by(|a, b| a["name"].as_str().unwrap_or("").to_ascii_lowercase().cmp(&b["name"].as_str().unwrap_or("").to_ascii_lowercase()));
    folders.sort_by(|a, b| a["name"].as_str().unwrap_or("").to_ascii_lowercase().cmp(&b["name"].as_str().unwrap_or("").to_ascii_lowercase()));
    folders.extend(entries);
    Ok(folders)
}

fn current(e: &Engine) -> PathBuf {
    e.session.browser.path.clone().filter(|p| p.is_dir()).unwrap_or_else(home)
}

fn view(e: &mut Engine, dir: &Path) -> Res {
    let only = e.session.browser.importable_only;
    let entries = listing(dir, only)?;
    let b = &e.session.browser;
    Ok(json!({
        "path": dir.to_string_lossy(),
        "parent": dir.parent().map(|p| p.to_string_lossy().to_string()),
        "importableOnly": only,
        "entries": entries,
        "favorites": b.favorites,
        "actions": ACTIONS.iter().map(|(id, label)| json!({"id": id, "label": label})).collect::<Vec<_>>(),
    }))
}

pub fn list(e: &mut Engine, p: &Json) -> Res {
    load(e);
    if let Some(o) = pb(p, "importableOnly") {
        e.session.browser.importable_only = o;
    }
    let dir = ps(p, "path").map(PathBuf::from).unwrap_or_else(|| current(e));
    view(e, &dir)
}

pub fn go(e: &mut Engine, p: &Json) -> Res {
    load(e);
    if let Some(o) = pb(p, "importableOnly") {
        e.session.browser.importable_only = o;
    }
    let cur = current(e);
    let dir = match ps(p, "path") {
        Some("..") => cur.parent().map(Path::to_path_buf).unwrap_or(cur),
        Some(x) => PathBuf::from(x),
        None => home(),
    };
    if !dir.is_dir() {
        return Err(format!("{} is not a folder", dir.display()));
    }
    e.session.browser.path = Some(dir.clone());
    save(e);
    view(e, &dir)
}

pub fn add_favorite(e: &mut Engine, p: &Json) -> Res {
    load(e);
    let path = ps(p, "path").map(str::to_string).unwrap_or_else(|| current(e).to_string_lossy().to_string());
    if !Path::new(&path).is_dir() {
        return Err(format!("{path} is not a folder"));
    }
    if !e.session.browser.favorites.contains(&path) {
        e.session.browser.favorites.push(path);
    }
    save(e);
    Ok(json!({"favorites": e.session.browser.favorites}))
}

pub fn remove_favorite(e: &mut Engine, p: &Json) -> Res {
    load(e);
    let path = ps(p, "path").map(str::to_string).unwrap_or_else(|| current(e).to_string_lossy().to_string());
    let before = e.session.browser.favorites.len();
    e.session.browser.favorites.retain(|x| *x != path);
    if before == e.session.browser.favorites.len() {
        return Err(format!("{path} is not a favourite"));
    }
    save(e);
    Ok(json!({"favorites": e.session.browser.favorites}))
}

pub fn file_info(_: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path`")?;
    let meta = std::fs::metadata(path).map_err(|x| format!("{path}: {x}"))?;
    let mut out = json!({"path": path, "name": Path::new(path).file_name().and_then(|n| n.to_str()), "size": meta.len(), "modified": modified(&meta), "folder": meta.is_dir(), "importable": meta.is_dir() || importable(path)});
    if meta.is_file() && media::is_media(path) {
        let mut warnings = vec![];
        match super::formats::probe(path, &json!({"sequence": false}), &mut warnings) {
            Ok((f, _)) => {
                out["width"] = json!(f.width);
                out["height"] = json!(f.height);
                out["duration"] = json!(f.duration);
                out["frameRate"] = json!(f.frame_rate);
                out["codec"] = json!(f.codec);
                out["hasVideo"] = json!(f.has_video);
                out["hasAudio"] = json!(f.has_audio);
                out["kind"] = json!(format!("{:?}", f.kind).to_lowercase());
            }
            Err(err) => out["error"] = json!(err),
        }
        let seq = media::sequence_files(path);
        if seq.len() > 1 {
            out["sequenceFrames"] = json!(seq.len());
        }
    }
    Ok(out)
}

pub fn import(e: &mut Engine, p: &Json) -> Res {
    let paths: Vec<String> = p.get("paths").and_then(Json::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    if paths.is_empty() {
        return Err("pass `paths`".into());
    }
    let mut files = vec![];
    for path in paths {
        let pb_ = PathBuf::from(&path);
        if pb_.is_dir() {
            let entries = listing(&pb_, true)?;
            let seqs: Vec<String> = entries.iter().filter(|x| x["kind"] == "sequence").filter_map(|x| x["path"].as_str().map(str::to_string)).collect();
            if seqs.is_empty() {
                files.extend(entries.iter().filter(|x| x["kind"] == "file" && x["importable"] == true).filter_map(|x| x["path"].as_str().map(str::to_string)));
            } else {
                files.extend(seqs);
            }
        } else {
            files.push(path);
        }
    }
    if files.is_empty() {
        return Err("nothing importable there".into());
    }
    let mut q = p.clone();
    q["paths"] = json!(files);
    if let Some(o) = q.as_object_mut() {
        o.entry("sequence").or_insert(json!(true));
    }
    super::file::import(e, &q)
}

pub fn action(e: &mut Engine, p: &Json) -> Res {
    load(e);
    let dir = current(e);
    match ps(p, "action").ok_or("pass `action` (refresh, home, reveal or importAll)")? {
        "refresh" => view(e, &dir),
        "home" => go(e, &json!({})),
        "reveal" => super::file::reveal_path(&dir.to_string_lossy()),
        "importAll" => import(e, &json!({"paths": [dir.to_string_lossy()]})),
        other => Err(format!("`{other}` is not a Media Browser action (refresh, home, reveal, importAll)")),
    }
}
