use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, ps, pu};
use crate::expr::{script, scriptui};

pub fn scripts_dir() -> PathBuf {
    crate::render::diskcache::data_dir().join("Scripts")
}

pub fn panels_dir() -> PathBuf {
    scripts_dir().join("ScriptUI Panels")
}

fn guarded(e: &mut Engine, f: impl FnOnce(&mut Engine) -> Json) -> Res {
    if script::running() {
        return Err("a script is already running".into());
    }
    e.batch_depth += 1;
    let r = f(e);
    e.batch_depth -= 1;
    Ok(r)
}

pub fn run(e: &mut Engine, p: &Json) -> Res {
    let code = ps(p, "code").ok_or("pass `code` (JavaScript with the After Effects scripting object model)")?.to_string();
    let file = ps(p, "file").unwrap_or("").to_string();
    guarded(e, |e| script::run(e, &code, &file, None))
}

pub fn run_file(e: &mut Engine, p: &Json) -> Res {
    let path = match ps(p, "path") {
        Some(x) => PathBuf::from(x),
        None => {
            let name = ps(p, "name").ok_or("pass `path` or `name` (an installed script)")?;
            find_installed(&scripts_dir(), name).ok_or_else(|| format!("no installed script \"{name}\""))?
        }
    };
    let code = std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let file = path.to_string_lossy().into_owned();
    guarded(e, |e| script::run(e, &code, &file, None))
}

fn find_installed(dir: &Path, name: &str) -> Option<PathBuf> {
    let want = name.to_ascii_lowercase();
    std::fs::read_dir(dir).ok()?.flatten().map(|d| d.path()).find(|p| {
        let stem = p.file_stem().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        let file = p.file_name().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        p.is_file() && (stem == want || file == want)
    })
}

fn script_files(dir: &Path) -> Vec<Json> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut files: Vec<PathBuf> = rd.flatten().map(|d| d.path()).filter(|p| p.is_file() && p.extension().and_then(|x| x.to_str()).is_some_and(|x| matches!(x.to_ascii_lowercase().as_str(), "jsx" | "js" | "jsxbin"))).collect();
        files.sort();
        for f in files {
            out.push(json!({"name": f.file_stem().and_then(|x| x.to_str()).unwrap_or(""), "path": f.to_string_lossy()}));
        }
    }
    out
}

pub fn list(_: &mut Engine, _: &Json) -> Res {
    Ok(json!({"folder": scripts_dir().to_string_lossy(), "scripts": script_files(&scripts_dir()), "panels": script_files(&panels_dir())}))
}

fn install_into(dir: PathBuf, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (a .jsx or .js file)")?;
    let src = Path::new(path);
    let name = src.file_name().ok_or("that path has no file name")?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let dest = dir.join(name);
    std::fs::copy(src, &dest).map_err(|err| format!("{}: {err}", src.display()))?;
    Ok(json!({"installed": dest.to_string_lossy()}))
}

pub fn install(e: &mut Engine, p: &Json) -> Res {
    let r = install_into(scripts_dir(), p)?;
    e.toast("Script installed in File > Scripts", false);
    Ok(r)
}

pub fn install_panel(e: &mut Engine, p: &Json) -> Res {
    let r = install_into(panels_dir(), p)?;
    e.toast("ScriptUI panel installed in the Window menu", false);
    Ok(r)
}

pub fn ui_list(_: &mut Engine, _: &Json) -> Res {
    Ok(json!({"windows": scriptui::list(), "panels": script_files(&panels_dir())}))
}

pub fn ui_event(e: &mut Engine, p: &Json) -> Res {
    let w = pu(p, "window").ok_or("pass `window`")?;
    let c = pu(p, "control").ok_or("pass `control`")?;
    let event = ps(p, "event").unwrap_or("click");
    scriptui::update(w, c, p)?;
    let name = match event {
        "click" => "onClick",
        "change" => "onChange",
        "changing" => "onChanging",
        "doubleclick" => "onDoubleClick",
        other => other,
    };
    let Some(f) = scriptui::handler(w, c, name) else {
        scriptui::flush(e);
        return Ok(json!({"ok": true, "handled": false}));
    };
    let this = scriptui::control(w, c);
    guarded(e, |e| script::call(e, &f, this, vec![]))
}

pub fn ui_close(e: &mut Engine, p: &Json) -> Res {
    let w = pu(p, "window").ok_or("pass `window`")?;
    let handler = scriptui::close(w);
    let out = match handler {
        Some(f) => {
            let this = scriptui::control(w, 0);
            guarded(e, |e| script::call(e, &f, this, vec![]))?
        }
        None => json!({"ok": true}),
    };
    scriptui::flush(e);
    Ok(out)
}

pub fn open_panel(e: &mut Engine, p: &Json) -> Res {
    let name = ps(p, "name").ok_or("pass `name` (an installed ScriptUI panel)")?;
    let path = find_installed(&panels_dir(), name).ok_or_else(|| format!("no installed ScriptUI panel \"{name}\""))?;
    let code = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let title = path.file_stem().and_then(|x| x.to_str()).unwrap_or(name).to_string();
    let w = scriptui::panel_window(&title);
    let this = scriptui::control(w, 0);
    let file = path.to_string_lossy().into_owned();
    let r = guarded(e, move |e| {
        scriptui::show(w);
        script::run(e, &code, &file, Some(this))
    })?;
    Ok(r)
}

pub fn panel_menu_items() -> Vec<(String, Json)> {
    script_files(&panels_dir()).iter().filter_map(|j| j.get("name").and_then(Json::as_str).map(|n| (n.to_string(), json!({"name": n})))).collect()
}

pub fn script_menu_items() -> Vec<(String, Json)> {
    script_files(&scripts_dir()).iter().filter_map(|j| j.get("name").and_then(Json::as_str).map(|n| (n.to_string(), json!({"name": n})))).collect()
}
