use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, ps};
use crate::expr::script;

pub fn extensions_dir() -> PathBuf {
    crate::render::diskcache::data_dir().join("Extensions")
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let at = tag.find(&key)? + key.len();
    let end = tag[at..].find('"')? + at;
    Some(tag[at..end].to_string())
}

fn element<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}");
    let at = xml.find(&open)?;
    let rest = &xml[at + open.len()..];
    let gt = rest.find('>')?;
    if rest[..gt].ends_with('/') {
        return Some("");
    }
    let close = format!("</{name}>");
    let end = rest.find(&close)?;
    Some(&rest[gt + 1..end])
}

fn text_of(xml: &str, name: &str) -> Option<String> {
    element(xml, name).map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

fn read_manifest(dir: &Path) -> Option<Json> {
    let folder = dir.file_name()?.to_str()?.to_string();
    if let Ok(t) = std::fs::read_to_string(dir.join("manifest.json"))
        && let Ok(j) = serde_json::from_str::<Json>(&t)
    {
        let main = j["main"].as_str().unwrap_or("index.html").to_string();
        return Some(json!({"id": j["id"].as_str().unwrap_or(&folder), "name": j["name"].as_str().unwrap_or(&folder), "main": main, "width": j["width"].as_u64().unwrap_or(320), "height": j["height"].as_u64().unwrap_or(480), "folder": folder, "kind": "formvfx"}));
    }
    let xml = std::fs::read_to_string(dir.join("CSXS").join("manifest.xml")).ok()?;
    let ext = element(&xml, "DispatchInfoList").unwrap_or(&xml);
    let id_tag = ext.find("<Extension").map(|i| &ext[i..i + ext[i..].find('>').unwrap_or(0)]).unwrap_or("");
    let id = attr(id_tag, "Id").unwrap_or_else(|| folder.clone());
    let main = text_of(ext, "MainPath").unwrap_or_else(|| "./index.html".into()).trim_start_matches("./").to_string();
    let name = text_of(ext, "Menu").unwrap_or_else(|| id.clone());
    let size = element(ext, "Size").unwrap_or("");
    let w = text_of(size, "Width").and_then(|v| v.parse::<u64>().ok()).unwrap_or(320);
    let h = text_of(size, "Height").and_then(|v| v.parse::<u64>().ok()).unwrap_or(480);
    let hosts: Vec<String> = xml.match_indices("<Host ").filter_map(|(i, _)| attr(&xml[i..i + xml[i..].find('>').unwrap_or(0)], "Name")).collect();
    Some(json!({"id": id, "name": name, "main": main, "width": w, "height": h, "folder": folder, "kind": "cep", "hosts": hosts}))
}

fn all() -> Vec<Json> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(extensions_dir()) {
        let mut dirs: Vec<PathBuf> = rd.flatten().map(|d| d.path()).filter(|p| p.is_dir()).collect();
        dirs.sort();
        for d in dirs {
            if let Some(m) = read_manifest(&d) {
                out.push(m);
            }
        }
    }
    out
}

fn find(key: &str) -> Result<Json, String> {
    all().into_iter().find(|m| m["id"] == key || m["folder"] == key || m["name"] == key).ok_or_else(|| format!("no extension \"{key}\" (extensions.list shows them)"))
}

pub fn list(_: &mut Engine, _: &Json) -> Res {
    Ok(json!({"folder": extensions_dir().to_string_lossy(), "extensions": all()}))
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let p = entry.path();
        let to = dst.join(entry.file_name());
        if p.is_dir() {
            copy_dir(&p, &to)?;
        } else {
            std::fs::copy(&p, &to)?;
        }
    }
    Ok(())
}

pub fn install(e: &mut Engine, p: &Json) -> Res {
    let src = PathBuf::from(ps(p, "path").ok_or("pass `path` (an extension folder, or a .zxp or .zip of one)")?);
    let root = extensions_dir();
    std::fs::create_dir_all(&root).map_err(|x| x.to_string())?;
    let name = src.file_stem().and_then(|s| s.to_str()).ok_or("bad path")?.to_string();
    let dest = root.join(&name);
    if src.is_dir() {
        copy_dir(&src, &dest).map_err(|x| format!("{}: {x}", src.display()))?;
    } else {
        let bytes = std::fs::read(&src).map_err(|x| format!("{}: {x}", src.display()))?;
        let files = crate::engine::lottie::unzip(&bytes).map_err(|x| format!("{}: {x}", src.display()))?;
        for (n, b) in files {
            if n.ends_with('/') || n.starts_with('/') || n.contains('\\') || n.starts_with("META-INF/") || n.split('/').any(|seg| seg == ".." || seg.is_empty()) {
                continue;
            }
            let to = dest.join(&n);
            if let Some(d) = to.parent() {
                std::fs::create_dir_all(d).map_err(|x| x.to_string())?;
            }
            std::fs::write(&to, b).map_err(|x| format!("{}: {x}", to.display()))?;
        }
    }
    let m = read_manifest(&dest).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&dest);
        "that is not an extension (no CSXS/manifest.xml or manifest.json)".to_string()
    })?;
    e.toast(&format!("Extension {} installed in Window > Extensions", m["name"].as_str().unwrap_or("")), false);
    Ok(m)
}

pub fn uninstall(_: &mut Engine, p: &Json) -> Res {
    let m = find(ps(p, "id").ok_or("pass `id`")?)?;
    let dir = extensions_dir().join(m["folder"].as_str().unwrap_or(""));
    std::fs::remove_dir_all(&dir).map_err(|x| format!("{}: {x}", dir.display()))?;
    Ok(json!({"removed": m["id"]}))
}

pub fn open(e: &mut Engine, p: &Json) -> Res {
    let m = find(ps(p, "id").ok_or("pass `id`")?)?;
    let url = format!("/api/extensions/{}/{}", m["folder"].as_str().unwrap_or(""), m["main"].as_str().unwrap_or("index.html"));
    let out = json!({"id": m["id"], "name": m["name"], "url": url, "width": m["width"], "height": m["height"]});
    e.frontend("extensions.open", out.clone());
    Ok(out)
}

pub fn eval_script(e: &mut Engine, p: &Json) -> Res {
    let code = ps(p, "script").or_else(|| ps(p, "code")).ok_or("pass `script` (ExtendScript to run, as CSInterface.evalScript sends it)")?.to_string();
    if crate::expr::scriptui::pending_window().is_some() {
        return Ok(json!({"result": "EvalScript error.", "error": "a script is waiting for its dialog"}));
    }
    let file = ps(p, "extension").map(|x| format!("extension:{x}")).unwrap_or_default();
    let r = script::run(e, &code, &file, None);
    let text = if r["ok"] == true {
        match &r["result"] {
            Json::Null => String::new(),
            Json::String(s) => s.clone(),
            Json::Number(n) => match n.as_f64() {
                Some(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", f as i64),
                _ => n.to_string(),
            },
            other => other.to_string(),
        }
    } else {
        "EvalScript error.".to_string()
    };
    Ok(json!({"result": text, "ok": r["ok"], "suspended": r.get("suspended"), "error": r["error"], "output": r["output"]}))
}

pub fn menu_items() -> Vec<(String, Json)> {
    all().iter().filter_map(|m| Some((m["name"].as_str()?.to_string(), json!({"id": m["id"]})))).collect()
}
