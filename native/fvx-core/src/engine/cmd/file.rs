use std::path::Path;

use serde_json::{Value as Json, json};

use super::{add_layer, footage_layer, formats, select_only};
use crate::engine::{Engine, Res, ecproj, pb, pf, ps};
use crate::model::{AlphaMode, EXTENSION, FORMAT, Footage, Item, ItemKind, Project};
use crate::render::media;

pub fn new_project(e: &mut Engine, _: &Json) -> Res {
    e.reset(Project::default(), None);
    e.imported_from = None;
    Ok(json!({"project": "new"}))
}

pub fn read_project(path: &str) -> Result<(Project, bool), String> {
    let ext = Path::new(path).extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    if !Path::new(path).is_file() {
        return Err(format!("no project at {path}"));
    }
    match ext.as_str() {
        "ecproj" | "ecprojx" => Ok((ecproj::import(path)?, true)),
        "fvxprojx" => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            let j = crate::engine::xmldoc::to_json(&text).map_err(|e| format!("{path} is not a readable .fvxprojx: {e}"))?;
            if !j.get("format").and_then(Json::as_str).is_some_and(|f| f.starts_with("formvfx.project/")) {
                return Err(format!("{path} is not a formvfx project"));
            }
            Ok((serde_json::from_value(j).map_err(|e| format!("{path}: {e}"))?, false))
        }
        _ => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            let j: Json = serde_json::from_str(&text).map_err(|e| format!("{path} is not a formvfx project: {e}"))?;
            if j.get("format").and_then(Json::as_str).is_some_and(|f| f.starts_with("formvfx.project/")) {
                let p: Project = serde_json::from_value(j).map_err(|e| format!("{path}: {e}"))?;
                Ok((p, false))
            } else if j.get("items").is_some() && j.get("version").is_some() {
                Ok((ecproj::import_json(&j, path)?, true))
            } else {
                Err(format!("{path} is not a formvfx project"))
            }
        }
    }
}

pub fn open(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path`")?.to_string();
    let (mut project, imported) = read_project(&path)?;
    relink(&mut project, &path);
    project.upgrade_3d(None);
    if project.uid.is_empty() {
        project.uid = crate::model::project::new_project_uid();
    }
    let saved_by = project.saved_by.clone();
    if imported {
        e.reset(project, None);
        e.imported_from = Some(path.clone());
        e.revision += 1;
        Ok(json!({"path": null, "imported": path, "note": "imported: Save writes a new .fvxproj"}))
    } else {
        e.reset(project, Some(path.clone()));
        e.imported_from = None;
        super::session::note_project(e, &path);
        Ok(json!({"path": path, "savedBy": saved_by}))
    }
}

pub fn relink(p: &mut Project, project_path: &str) {
    let dir = Path::new(project_path).parent().map(Path::to_path_buf);
    for it in p.items.iter_mut() {
        if let ItemKind::Footage(f) = &mut it.kind {
            if f.path.is_empty() {
                continue;
            }
            if Path::new(&f.path).exists() {
                f.missing = false;
                continue;
            }
            let name = Path::new(&f.path).file_name().map(|n| n.to_owned());
            let found = match (&dir, name) {
                (Some(d), Some(n)) => [d.join(&n), d.join("Footage").join(&n)].into_iter().find(|c| c.exists()),
                _ => None,
            };
            match found {
                Some(c) => {
                    f.path = c.to_string_lossy().to_string();
                    f.missing = false;
                }
                None => f.missing = true,
            }
        }
    }
}

pub fn write_project(e: &Engine, path: &str) -> Result<(), String> {
    let mut p = (*e.project).clone();
    p.format = FORMAT.into();
    p.saved_by = format!("formvfx {}", env!("CARGO_PKG_VERSION"));
    let text = if path.to_ascii_lowercase().ends_with(".fvxprojx") {
        crate::engine::xmldoc::from_json("fvxprojx", &[("version", "1")], &serde_json::to_value(&p).map_err(|e| e.to_string())?)
    } else {
        serde_json::to_string_pretty(&p).map_err(|e| e.to_string())?
    };
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, text).map_err(|err| format!("{path}: {err}"))?;
    std::fs::rename(&tmp, path).map_err(|err| format!("{path}: {err}"))?;
    Ok(())
}

fn with_ext(path: &str) -> Result<String, String> {
    let p = Path::new(path);
    match p.extension().and_then(|x| x.to_str()).map(str::to_ascii_lowercase) {
        Some(x) if x == EXTENSION || x == "fvxprojx" => Ok(path.to_string()),
        Some(x) if x == "ecproj" || x == "ecprojx" => Err("formvfx saves .fvxproj projects; .ecproj files can only be opened".into()),
        _ => Ok(p.with_extension(EXTENSION).to_string_lossy().to_string()),
    }
}

pub fn save(e: &mut Engine, p: &Json) -> Res {
    let path = match ps(p, "path") {
        Some(x) => with_ext(x)?,
        None => e.path.clone().ok_or("pass `path` for the first save (a .fvxproj file)")?,
    };
    write_project(e, &path)?;
    e.path = Some(path.clone());
    e.saved_revision = e.revision;
    e.imported_from = None;
    super::session::note_project(e, &path);
    Ok(json!({"path": path}))
}

pub fn save_as(e: &mut Engine, p: &Json) -> Res {
    let path = with_ext(ps(p, "path").ok_or("pass `path`")?)?;
    write_project(e, &path)?;
    e.path = Some(path.clone());
    e.saved_revision = e.revision;
    super::session::note_project(e, &path);
    Ok(json!({"path": path}))
}

pub fn save_copy(e: &mut Engine, p: &Json) -> Res {
    let path = with_ext(ps(p, "path").ok_or("pass `path`")?)?;
    write_project(e, &path)?;
    Ok(json!({"path": path}))
}

pub fn increment_and_save(e: &mut Engine, _: &Json) -> Res {
    let cur = e.path.clone().ok_or("save the project first")?;
    let pb_ = Path::new(&cur);
    let stem = pb_.file_stem().and_then(|s| s.to_str()).unwrap_or("Project");
    let base = stem.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end_matches([' ', '_']);
    let dir = pb_.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut n = 2;
    let path = loop {
        let cand = dir.join(format!("{base} {n}.{EXTENSION}"));
        if !cand.exists() {
            break cand;
        }
        n += 1;
    };
    let path = path.to_string_lossy().to_string();
    write_project(e, &path)?;
    e.path = Some(path.clone());
    e.saved_revision = e.revision;
    super::session::note_project(e, &path);
    Ok(json!({"path": path}))
}

pub fn revert(e: &mut Engine, _: &Json) -> Res {
    let path = e.path.clone().ok_or("the project has not been saved")?;
    open(e, &json!({"path": path}))
}

fn import_one(e: &mut Engine, path: &str, folder: Option<u64>, p: &Json, warnings: &mut Vec<String>) -> Result<u64, String> {
    let ext = Path::new(path).extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    if matches!(ext.as_str(), "fvxproj" | "ecproj" | "ecprojx") {
        return import_project(e, path, folder);
    }
    if matches!(ext.as_str(), "json" | "lottie") && crate::engine::lottie::is_lottie(path) {
        return crate::engine::lottie::import(e, path, folder).map(|r| r.0);
    }
    if !media::is_media(path) && !Path::new(path).is_file() {
        return Err(format!("no file at {path}"));
    }
    let (f, name) = formats::probe(path, p, warnings)?;
    let pj = e.proj();
    let id = pj.new_id();
    pj.items.push(Item { id, name, parent: folder, label: formats::label_of(&f), comment: String::new(), kind: ItemKind::Footage(f), proxy: None });
    Ok(id)
}

pub fn import_project(e: &mut Engine, path: &str, folder: Option<u64>) -> Result<u64, String> {
    let (mut sub, _) = read_project(path)?;
    relink(&mut sub, path);
    let pj = e.proj();
    let fid = pj.new_id();
    let name = Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("Project").to_string();
    pj.items.push(Item { id: fid, name, parent: folder, label: 0, comment: String::new(), kind: ItemKind::Folder, proxy: None });
    let mut map = std::collections::HashMap::new();
    for it in &sub.items {
        let nid = pj.new_id();
        map.insert(it.id, nid);
    }
    let mut layer_map = std::collections::HashMap::new();
    for it in &sub.items {
        if let ItemKind::Comp(c) = &it.kind {
            for l in &c.layers {
                let nid = pj.new_id();
                layer_map.insert(l.id, nid);
            }
        }
    }
    for it in sub.items {
        let mut it = it;
        it.id = map[&it.id];
        it.parent = Some(it.parent.and_then(|p| map.get(&p).copied()).unwrap_or(fid));
        if let ItemKind::Comp(c) = &mut it.kind {
            let paths = super::essential::prop_paths(c);
            for l in c.layers.iter_mut() {
                l.id = layer_map[&l.id];
                l.source = l.source.and_then(|s| map.get(&s).copied());
                l.parent = l.parent.and_then(|s| layer_map.get(&s).copied());
                if let Some(m) = l.track_matte.as_mut() {
                    m.layer = layer_map.get(&m.layer).copied().unwrap_or(m.layer);
                }
                l.props.renumber(&mut pj.next_uid);
            }
            super::essential::remap(c, &layer_map, paths);
        }
        pj.items.push(it);
    }
    Ok(fid)
}

pub fn import(e: &mut Engine, p: &Json) -> Res {
    let mut paths: Vec<String> = p.get("paths").and_then(Json::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    if let Some(one) = ps(p, "path") {
        paths.push(one.to_string());
    }
    if paths.is_empty() {
        return Err("pass `paths` (absolute file paths)".into());
    }
    let folder = e.state.project_selection.iter().copied().find(|i| e.project.item(*i).is_some_and(|it| matches!(it.kind, ItemKind::Folder)));
    let retain = formats::import_as(p)?;
    let mut items = vec![];
    let mut errors = vec![];
    let mut warnings = vec![];
    let mut comps = vec![];
    for path in &paths {
        let lower = path.to_ascii_lowercase();
        let r = match retain {
            Some(keep) if lower.ends_with(".psd") || lower.ends_with(".psb") => formats::import_psd_comp(e, path, keep, folder).map(|(c, its)| {
                comps.push(c);
                its
            }),
            Some(_) if lower.ends_with(".pdf") || lower.ends_with(".ai") => formats::import_pdf_comp(e, path, p, folder).map(|(c, its)| {
                comps.push(c);
                its
            }),
            _ => import_one(e, path, folder, p, &mut warnings).map(|id| vec![id]),
        };
        match r {
            Ok(ids) => items.extend(ids),
            Err(err) => errors.push(err),
        }
    }
    if items.is_empty() {
        return Err(errors.join("; "));
    }
    for w in &warnings {
        e.toast(w, true);
    }
    let mut layers = vec![];
    if pb(p, "addToComp").unwrap_or(false)
        && let Ok(cid) = e.comp_id(p.get("comp"))
    {
        let time = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
        let pos = p.get("position").and_then(Json::as_array).and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]));
        let placed = if comps.is_empty() { items.clone() } else { comps.clone() };
        for id in &placed {
            if e.project.item(*id).is_some_and(|i| matches!(i.kind, ItemKind::Footage(_) | ItemKind::Comp(_) | ItemKind::Solid(_))) {
                let l = footage_layer(e, cid, *id, time, pos, None)?;
                let lid = add_layer(e, cid, l, None);
                layers.push(lid);
            }
        }
        if !layers.is_empty() {
            select_only(e, &layers);
        }
    }
    if layers.is_empty()
        && let Some(c) = comps.first().copied()
    {
        super::comp::open(e, &json!({"comp": c}))?;
    }
    e.state.project_selection = if comps.is_empty() { items.clone() } else { comps.clone() };
    let media: Vec<String> = paths.iter().filter(|x| !x.to_ascii_lowercase().ends_with(".fvxproj")).cloned().collect();
    super::session::note_footage(e, &media);
    e.set_label("Import");
    Ok(json!({"items": items, "layers": layers, "errors": errors, "comps": comps, "warnings": warnings}))
}

pub fn open_demo(e: &mut Engine, _: &Json) -> Res {
    let p = crate::engine::demo::build();
    e.reset(p, None);
    Ok(json!({"demo": true, "comp": e.state.active_comp}))
}

pub fn project_settings(e: &mut Engine, p: &Json) -> Res {
    let s = &mut e.proj().settings;
    if let Some(b) = p.get("bitDepth") {
        let v = b.as_u64().or_else(|| b.as_str().and_then(|x| x.split_whitespace().next()?.parse().ok())).unwrap_or(8);
        s.bit_depth = match v {
            16 => 16,
            32 => 32,
            _ => 8,
        };
    }
    if let Some(t) = ps(p, "timeDisplay") {
        s.time_display = t.to_string();
    }
    if let Some(f) = pf(p, "frameStart") {
        s.frame_start = f as u32;
    }
    if let Some(r) = pf(p, "audioRate") {
        s.audio_rate = r as u32;
    }
    if let Some(l) = pb(p, "linearize") {
        s.linearize = l;
    }
    if let Some(b) = pb(p, "blendLinear") {
        s.blend_linear = b;
    }
    if let Some(r) = ps(p, "renderer") {
        s.renderer = if r.eq_ignore_ascii_case("cpu") || r.to_ascii_lowercase().contains("software") { "cpu".into() } else { "gpu".into() };
    }
    let engine_changed = ps(p, "colorEngine").map(|x| crate::legacy::canon(x).to_ascii_lowercase());
    if let Some(eng) = &engine_changed {
        if eng != "profiles" && eng != "ocio" {
            return Err("colorEngine is `profiles` (formvfx's profiles) or `ocio`".into());
        }
        s.color_engine = eng.clone();
    }
    if let Some(c) = ps(p, "ocioConfig") {
        crate::render::ocio::load(c)?;
        s.ocio_config = c.to_string();
    }
    let ocio = s.color_engine == "ocio";
    if let Some(w) = ps(p, "workingSpace") {
        if ocio {
            let cfg = crate::render::ocio::load(&s.ocio_config)?;
            let sp = cfg.space(w).or_else(|| crate::legacy::alias(w).and_then(|c| cfg.space(c))).filter(|x| !x.display && !x.data).ok_or_else(|| format!("`{w}` is not a scene colour space in the OCIO config"))?;
            s.working_space = sp.name.clone();
        } else if w.eq_ignore_ascii_case("none") || w.is_empty() {
            s.working_space = "none".into();
        } else {
            let sp = crate::render::color::find(w).filter(|x| x.working).ok_or_else(|| format!("`{w}` is not a working space ({})", crate::render::color::SPACES.iter().filter(|x| x.working).map(|x| x.id).collect::<Vec<_>>().join(", ")))?;
            s.working_space = sp.id.to_string();
        }
    } else if let Some(eng) = &engine_changed {
        if eng == "ocio" {
            let cfg = crate::render::ocio::load(&s.ocio_config)?;
            s.working_space = cfg.role("scene_linear").and_then(|r| cfg.space(r)).map(|x| x.name.clone()).unwrap_or_default();
        } else if crate::render::color::find(&s.working_space).is_none() {
            s.working_space = "none".into();
        }
    }
    if ocio && (p.get("display").is_some() || p.get("view").is_some()) {
        let cfg = crate::render::ocio::load(&s.ocio_config)?;
        let d = ps(p, "display").map(str::to_string).unwrap_or_else(|| s.display.clone());
        let d = if d.is_empty() { cfg.display_names().first().cloned().unwrap_or_default() } else { d };
        if !cfg.display_names().iter().any(|x| x.eq_ignore_ascii_case(&d)) && !cfg.displays.iter().any(|x| x.0.eq_ignore_ascii_case(&d)) {
            return Err(format!("no display `{d}` in the OCIO config"));
        }
        let v = ps(p, "view").map(str::to_string).unwrap_or_default();
        if !v.is_empty() && !cfg.views(&d).iter().any(|x| x.eq_ignore_ascii_case(&v)) {
            return Err(format!("no view `{v}` for `{d}`"));
        }
        s.display = d;
        s.view = v;
    }
    if let Some(h) = ps(p, "hdr") {
        s.hdr = match h {
            "clip" | "toneMap" => h.to_string(),
            _ => return Err("hdr is `clip` or `toneMap`".into()),
        };
    }
    if let Some(o) = ps(p, "outputSpace") {
        s.output_space = if o.eq_ignore_ascii_case("working") { String::new() } else { o.to_string() };
    }
    Ok(serde_json::to_value(&e.project.settings).unwrap_or_default())
}

pub fn cycle_bit_depth(e: &mut Engine, _: &Json) -> Res {
    let s = &mut e.proj().settings;
    s.bit_depth = match s.bit_depth {
        8 => 16,
        16 => 32,
        _ => 8,
    };
    Ok(json!({"bitDepth": e.project.settings.bit_depth}))
}

fn items_param(e: &Engine, p: &Json) -> Vec<u64> {
    match p.get("items").and_then(Json::as_array) {
        Some(a) => a.iter().filter_map(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| e.project.items.iter().find(|i| i.name == s).map(|i| i.id)))).collect(),
        None => p.get("item").and_then(Json::as_u64).map(|x| vec![x]).unwrap_or_else(|| e.state.project_selection.clone()),
    }
}

pub fn interpret(e: &mut Engine, p: &Json) -> Res {
    let ids = items_param(e, p);
    if ids.is_empty() {
        return Err("select footage in the Project panel or pass `items`".into());
    }
    let mut n = 0;
    for id in ids {
        let pj = e.proj();
        let Some(ItemKind::Footage(f)) = pj.item_mut(id).map(|i| &mut i.kind) else { continue };
        if let Some(fr) = pf(p, "frameRate").filter(|x| *x > 0.0) {
            let old = f.frame_rate.max(0.001);
            f.duration = f.duration * old / fr;
            f.frame_rate = fr;
        }
        if let Some(a) = ps(p, "alpha") {
            f.alpha = match a.to_ascii_lowercase().as_str() {
                "ignore" => AlphaMode::Ignore,
                "premultiplied" | "premultiply" => AlphaMode::Premultiplied,
                _ => AlphaMode::Straight,
            };
        }
        if let Some(l) = pf(p, "loops") {
            f.loops = l.max(1.0) as u32;
        }
        if let Some(pa) = pf(p, "pixelAspect").filter(|x| *x > 0.0) {
            f.pixel_aspect = pa;
        }
        if let Some(cp) = ps(p, "colorProfile") {
            f.color_profile = if cp.eq_ignore_ascii_case("auto") || cp.eq_ignore_ascii_case("embedded") { String::new() } else { cp.to_string() };
        }
        if let Some(pr) = pb(p, "preserveRgb") {
            f.preserve_rgb = pr;
        }
        n += 1;
    }
    Ok(json!({"items": n}))
}

pub fn replace_footage(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path`")?.to_string();
    let ids = items_param(e, p);
    let id = *ids.first().ok_or("select the footage to replace")?;
    let f: Footage = media::probe(&path)?;
    let name = Path::new(&path).file_name().and_then(|n| n.to_str()).unwrap_or(&path).to_string();
    let media = e.media.clone();
    let pj = e.proj();
    let it = pj.item_mut(id).ok_or("no such item")?;
    if let ItemKind::Footage(old) = &it.kind {
        media.forget(&old.path);
    }
    it.kind = ItemKind::Footage(f);
    it.name = name;
    Ok(json!({"item": id}))
}

pub fn reload_footage(e: &mut Engine, p: &Json) -> Res {
    let ids = items_param(e, p);
    let mut n = 0;
    for id in ids {
        let path = match e.project.item(id).map(|i| &i.kind) {
            Some(ItemKind::Footage(f)) => f.path.clone(),
            _ => continue,
        };
        e.media.forget(&path);
        if let Ok(f) = media::probe(&path) {
            if let Some(it) = e.proj().item_mut(id) {
                it.kind = ItemKind::Footage(f);
            }
            n += 1;
        }
    }
    Ok(json!({"reloaded": n}))
}

pub fn new_comp_from_selection(e: &mut Engine, p: &Json) -> Res {
    let ids = items_param(e, p);
    let id = *ids.first().ok_or("select footage in the Project panel")?;
    let it = e.project.item(id).ok_or("no such item")?.clone();
    let (w, h) = it.dimensions().unwrap_or((1920, 1080));
    let fr = it.frame_rate().unwrap_or(30.0);
    let dur = it.duration().filter(|d| *d > 0.0).unwrap_or(10.0);
    let cid = super::comp::make_comp(e, &it.name, w, h, fr, dur, [0.0; 3]);
    let l = footage_layer(e, cid, id, 0.0, None, None)?;
    add_layer(e, cid, l, None);
    e.state.active_comp = Some(cid);
    if !e.state.open_comps.contains(&cid) {
        e.state.open_comps.push(cid);
    }
    e.set_label("New Comp from Selection");
    Ok(json!({"comp": cid}))
}

pub fn close(e: &mut Engine, p: &Json) -> Res {
    if p.get("comp").is_none() && e.state.active_comp.is_none() {
        return Ok(json!({"closed": null}));
    }
    super::comp::close(e, p)
}

pub fn save_copy_as_xml(e: &mut Engine, p: &Json) -> Res {
    let raw = ps(p, "path").ok_or("pass `path` (.fvxprojx)")?;
    let path = Path::new(raw).with_extension("fvxprojx").to_string_lossy().to_string();
    write_project(e, &path)?;
    Ok(json!({"path": path}))
}

pub fn reveal_in_finder(e: &mut Engine, p: &Json) -> Res {
    let ids = items_param(e, p);
    let path = match ids.first().and_then(|i| e.project.item(*i)) {
        Some(Item { kind: ItemKind::Footage(f), .. }) if !f.path.is_empty() => f.path.clone(),
        Some(it) if !matches!(it.kind, ItemKind::Footage(_)) && e.path.is_none() => return Err(format!("{} has no file on disk", it.name)),
        Some(Item { kind: ItemKind::Footage(_), name, .. }) => return Err(format!("{name} is a placeholder with no file")),
        _ => e.path.clone().ok_or("select footage, or save the project first")?,
    };
    reveal_path(&path)
}

pub fn reveal_path(path: &str) -> Res {
    let path = path.to_string();
    if !Path::new(&path).exists() {
        return Err(format!("{path} is missing"));
    }
    if std::env::var_os("FORMVFX_NO_REVEAL").is_some() {
        return Ok(json!({"path": path, "revealed": false}));
    }
    let dir = if Path::new(&path).is_dir() { path.clone() } else { Path::new(&path).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default() };
    let spawned = if cfg!(target_os = "macos") {
        std::process::Command::new("open").args(["-R", &path]).spawn()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("explorer").arg(format!("/select,{path}")).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(&dir).spawn()
    };
    spawned.map_err(|x| format!("could not open the file manager: {x}"))?;
    Ok(json!({"path": path, "revealed": true}))
}

pub fn execute_file(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (a .jsx, .js or .jsonl command file)")?;
    let inside = Path::new(path).parent().is_some_and(|d| d == super::script::scripts_dir());
    if !inside && !pb(p, "confirmed").unwrap_or(false) {
        return Err(format!("{path} runs code from outside the Scripts folder; it needs confirmed: true"));
    }
    super::script::run_file(e, &json!({"path": path}))
}

pub fn import_vanishing_point(_: &mut Engine, _: &Json) -> Res {
    Err("Vanishing Point (.vpe) files are not read: the format has no public description, so formvfx cannot import them faithfully".into())
}
