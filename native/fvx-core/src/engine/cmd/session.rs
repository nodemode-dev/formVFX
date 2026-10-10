use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, ps, pu};
use crate::model::ItemKind;
use crate::render::diskcache::data_dir;

const KEEP: usize = 10;

#[derive(Default)]
pub struct AutoSave {
    pub mark: f64,
    pub last: Option<(f64, String)>,
    pub error: Option<String>,
    pub count: u64,
    pub revision: u64,
}

#[derive(Default)]
pub struct Session {
    pub recent_projects: Vec<String>,
    pub recent_footage: Vec<String>,
    pub interpretation: Option<(String, Json)>,
    pub autosave: AutoSave,
    pub watch: Option<super::watch::Watch>,
    pub footage: super::footage::View,
    pub browser: super::browser::Browser,
    pub learn: super::help::Learn,
    pub last_effect: Option<String>,
    pub effect_clip: Vec<crate::model::Group>,
}

fn store() -> PathBuf {
    data_dir().join("recent.json")
}

pub fn clock() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

impl Session {
    pub fn load() -> Session {
        let j: Json = std::fs::read_to_string(store()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Json::Null);
        let list = |k: &str| -> Vec<String> { j.get(k).and_then(Json::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).take(KEEP).collect()).unwrap_or_default() };
        Session { recent_projects: list("projects"), recent_footage: list("footage"), ..Session::default() }
    }

    fn save(&self) {
        let _ = std::fs::create_dir_all(data_dir());
        let _ = std::fs::write(store(), serde_json::to_string_pretty(&json!({"projects": self.recent_projects, "footage": self.recent_footage})).unwrap_or_default());
    }
}

fn push(list: &mut Vec<String>, path: &str) {
    list.retain(|x| x != path);
    list.insert(0, path.to_string());
    list.truncate(KEEP);
}

pub fn note_project(e: &mut Engine, path: &str) {
    push(&mut e.session.recent_projects, path);
    e.session.autosave.mark = clock();
    e.session.save();
}

pub fn note_footage(e: &mut Engine, paths: &[String]) {
    for p in paths.iter().rev() {
        push(&mut e.session.recent_footage, p);
    }
    e.session.save();
}

fn pick(list: &[String], p: &Json, what: &str) -> Result<String, String> {
    if let Some(x) = ps(p, "path") {
        return Ok(x.to_string());
    }
    let i = pu(p, "index").unwrap_or(0) as usize;
    list.get(i).cloned().ok_or_else(|| if list.is_empty() { format!("there are no recent {what}") } else { format!("index {i} is past the {} recent {what}", list.len()) })
}

pub fn open_recent(e: &mut Engine, p: &Json) -> Res {
    let path = pick(&e.session.recent_projects, p, "projects")?;
    if !Path::new(&path).is_file() {
        e.session.recent_projects.retain(|x| *x != path);
        e.session.save();
        return Err(format!("{path} is no longer there; it is taken off Open Recent"));
    }
    super::file::open(e, &json!({"path": path}))
}

pub fn import_recent(e: &mut Engine, p: &Json) -> Res {
    let path = pick(&e.session.recent_footage, p, "footage")?;
    if !Path::new(&path).exists() {
        e.session.recent_footage.retain(|x| *x != path);
        e.session.save();
        return Err(format!("{path} is no longer there; it is taken off Import Recent Footage"));
    }
    super::file::import(e, &json!({"paths": [path]}))
}

pub fn clear_recent(e: &mut Engine, _: &Json) -> Res {
    let n = e.session.recent_projects.len();
    e.session.recent_projects.clear();
    e.session.save();
    Ok(json!({"cleared": n}))
}

pub fn clear_recent_footage(e: &mut Engine, _: &Json) -> Res {
    let n = e.session.recent_footage.len();
    e.session.recent_footage.clear();
    e.session.save();
    Ok(json!({"cleared": n}))
}

fn footage_param(e: &Engine, p: &Json) -> Result<u64, String> {
    let id = pu(p, "item").or_else(|| e.state.project_selection.iter().copied().find(|i| e.project.item(*i).is_some_and(|it| matches!(it.kind, ItemKind::Footage(_))))).ok_or("select a footage item (or pass `item`)")?;
    match e.project.item(id).map(|i| &i.kind) {
        Some(ItemKind::Footage(_)) => Ok(id),
        Some(_) => Err("that item is not footage".into()),
        None => Err(format!("no item {id}")),
    }
}

pub fn remember_interpretation(e: &mut Engine, p: &Json) -> Res {
    let id = footage_param(e, p)?;
    let it = e.project.item(id).ok_or("no item")?;
    let ItemKind::Footage(f) = &it.kind else { return Err("that item is not footage".into()) };
    let alpha = match f.alpha {
        crate::model::AlphaMode::Straight => "straight",
        crate::model::AlphaMode::Premultiplied => "premultiplied",
        crate::model::AlphaMode::Ignore => "ignore",
    };
    let mut v = json!({"alpha": alpha, "loops": f.loops.max(1), "pixelAspect": f.pixel_aspect, "preserveRgb": f.preserve_rgb});
    if !f.is_still() && f.frame_rate > 0.0 {
        v["frameRate"] = json!(f.frame_rate);
    }
    if !f.color_profile.is_empty() {
        v["colorProfile"] = json!(f.color_profile);
    }
    let name = it.name.clone();
    e.session.interpretation = Some((name.clone(), v.clone()));
    Ok(json!({"item": id, "from": name, "interpretation": v}))
}

pub fn apply_interpretation(e: &mut Engine, p: &Json) -> Res {
    let (from, v) = e.session.interpretation.clone().ok_or("nothing is remembered yet (File > Interpret Footage > Remember Interpretation)")?;
    let mut q = v.clone();
    match p.get("items") {
        Some(items) => q["items"] = items.clone(),
        None => q["items"] = json!(e.state.project_selection.iter().copied().filter(|i| e.project.item(*i).is_some_and(|it| matches!(it.kind, ItemKind::Footage(_)))).collect::<Vec<u64>>()),
    }
    if q["items"].as_array().is_none_or(|a| a.is_empty()) {
        return Err("select the footage to apply the interpretation to (or pass `items`)".into());
    }
    let r = super::file::interpret(e, &q)?;
    e.set_label("Apply Interpretation");
    Ok(json!({"from": from, "interpretation": v, "result": r}))
}

pub fn default_pref(k: &str) -> Option<Json> {
    match k {
        "autoSave.enabled" => Some(json!(true)),
        "autoSave.minutes" => Some(json!(20)),
        "autoSave.versions" => Some(json!(5)),
        "autoSave.folder" => Some(json!("")),
        _ => None,
    }
}

fn pref(e: &Engine, k: &str) -> Json {
    e.prefs.get(k).cloned().or_else(|| default_pref(k)).unwrap_or(Json::Null)
}

pub fn pref_items(e: &Engine) -> Vec<Json> {
    vec![
        json!({"section": "Auto-Save"}),
        json!({"key": "autoSave.enabled", "label": "Automatically Save Projects", "type": "bool"}),
        json!({"key": "autoSave.minutes", "label": "Save Every", "type": "number", "min": 1, "unit": "minutes"}),
        json!({"key": "autoSave.versions", "label": "Maximum Project Versions", "type": "number", "min": 1}),
        json!({"key": "autoSave.folder", "label": "Auto-Save Location", "type": "text"}),
        json!({"note": format!("Projects with unsaved changes are saved as numbered copies in {}, never over the project itself; the oldest copy goes once the maximum is reached. Leave the location empty to save beside the project.", folder(e).display())}),
        json!({"button": "Auto-Save Now", "command": "file.autoSave", "params": {}}),
    ]
}

fn folder(e: &Engine) -> PathBuf {
    let custom = pref(e, "autoSave.folder").as_str().map(str::trim).filter(|s| !s.is_empty()).map(PathBuf::from);
    custom.unwrap_or_else(|| match e.path.as_deref().and_then(|p| Path::new(p).parent()) {
        Some(d) => d.join("formvfx Auto-Save"),
        None => data_dir().join("Auto-Save"),
    })
}

fn stem(e: &Engine) -> String {
    e.path.as_deref().and_then(|p| Path::new(p).file_stem()).and_then(|s| s.to_str()).unwrap_or("Untitled Project").to_string()
}

fn versions(e: &Engine) -> Vec<(u64, PathBuf)> {
    let head = format!("{} auto-save ", stem(e));
    let mut out: Vec<(u64, PathBuf)> = std::fs::read_dir(folder(e))
        .map(|rd| {
            rd.flatten()
                .filter_map(|x| {
                    let name = x.file_name().to_string_lossy().to_string();
                    let n = name.strip_prefix(&head)?.strip_suffix(".fvxproj")?.parse::<u64>().ok()?;
                    Some((n, x.path()))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn write_auto(e: &mut Engine) -> Result<String, String> {
    let dir = folder(e);
    std::fs::create_dir_all(&dir).map_err(|x| format!("{}: {x}", dir.display()))?;
    let n = versions(e).last().map(|v| v.0).unwrap_or(0) + 1;
    let path = dir.join(format!("{} auto-save {n}.fvxproj", stem(e))).to_string_lossy().to_string();
    super::file::write_project(e, &path)?;
    let keep = pref(e, "autoSave.versions").as_u64().unwrap_or(5).max(1) as usize;
    let mut all = versions(e);
    while all.len() > keep {
        let _ = std::fs::remove_file(all.remove(0).1);
    }
    let now = clock();
    let rev = e.revision;
    let a = &mut e.session.autosave;
    a.revision = rev;
    a.mark = now;
    a.last = Some((now, path.clone()));
    a.error = None;
    a.count += 1;
    Ok(path)
}

pub fn auto_save(e: &mut Engine, _: &Json) -> Res {
    match write_auto(e) {
        Ok(path) => Ok(json!({"path": path, "versions": versions(e).len()})),
        Err(err) => {
            e.session.autosave.error = Some(err.clone());
            Err(err)
        }
    }
}

pub fn recovery_info(e: &mut Engine, _: &Json) -> Res {
    let enabled = pref(e, "autoSave.enabled").as_bool().unwrap_or(true);
    let minutes = pref(e, "autoSave.minutes").as_f64().unwrap_or(20.0).max(1.0);
    let a = &e.session.autosave;
    let next = (enabled && e.is_dirty() && !e.project.items.is_empty() && a.mark > 0.0).then(|| (a.mark + minutes * 60.0 - clock()).max(0.0));
    Ok(json!({
        "enabled": enabled,
        "minutes": minutes,
        "versions": pref(e, "autoSave.versions"),
        "folder": folder(e).to_string_lossy(),
        "lastSave": a.last.as_ref().map(|l| l.0),
        "lastPath": a.last.as_ref().map(|l| l.1.clone()),
        "saves": a.count,
        "error": a.error,
        "dirty": e.is_dirty(),
        "nextIn": next,
        "copies": versions(e).into_iter().map(|v| v.1.to_string_lossy().to_string()).collect::<Vec<_>>(),
    }))
}

pub fn tick(e: &mut Engine, now: f64) -> Json {
    if e.session.autosave.mark <= 0.0 {
        e.session.autosave.mark = now;
        return Json::Null;
    }
    if !pref(e, "autoSave.enabled").as_bool().unwrap_or(true) || !e.is_dirty() || e.project.items.is_empty() || e.revision == e.session.autosave.revision {
        return Json::Null;
    }
    let minutes = pref(e, "autoSave.minutes").as_f64().unwrap_or(20.0).max(1.0);
    if now - e.session.autosave.mark < minutes * 60.0 {
        return Json::Null;
    }
    match write_auto(e) {
        Ok(path) => json!({"path": path, "time": now}),
        Err(err) => {
            e.session.autosave.mark = now;
            e.session.autosave.error = Some(err.clone());
            json!({"error": err, "time": now})
        }
    }
}
