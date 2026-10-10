use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pb, pf, ps};
use crate::model::{FootageKind, ItemKind, Project};

pub const EXT: &str = "fvxprojt";
const FORMAT: &str = "formvfx.projectTemplate/1";
const TW: usize = 256;
const TH: usize = 144;

struct Builtin {
    slug: &'static str,
    name: &'static str,
    category: &'static str,
    description: &'static str,
    size: (u32, u32),
    duration: f64,
}

const BUILTINS: [Builtin; 4] = [
    Builtin { slug: "title-card", name: "Title Card", category: "Titles", description: "A 1080p title that fades up over a dark ground, with a line under it.", size: (1920, 1080), duration: 6.0 },
    Builtin { slug: "lower-third", name: "Lower Third", category: "Titles", description: "A name and role bar that slides in at the lower left of a 1080p frame.", size: (1920, 1080), duration: 5.0 },
    Builtin { slug: "social-square", name: "Social Square", category: "Social", description: "A 1080 by 1080 post with a headline and a coloured band.", size: (1080, 1080), duration: 8.0 },
    Builtin { slug: "vertical-story", name: "Vertical Story", category: "Social", description: "A 1080 by 1920 story frame with a headline at the top and a caption at the foot.", size: (1080, 1920), duration: 10.0 },
];

pub fn templates_dir() -> PathBuf {
    crate::render::diskcache::data_dir().join("Project Templates")
}

fn safe(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' }).collect();
    if t.trim().is_empty() { "Template".into() } else { t.trim().to_string() }
}

fn build_builtin(b: &Builtin) -> Result<Project, String> {
    let mut s = Engine::new();
    let (w, h) = b.size;
    let (wf, hf) = (w as f64, h as f64);
    let run = |s: &mut Engine, id: &str, p: Json| s.execute(id, p);
    run(&mut s, "comp.new", json!({"name": b.name, "width": w, "height": h, "frameRate": 30, "duration": b.duration, "background": "#101114", "open": true}))?;
    let text = |s: &mut Engine, t: &str, size: f64, pos: [f64; 2], justify: &str| -> Result<u64, String> {
        let r = s.execute("layer.newText", json!({"text": t, "size": size, "position": pos, "fill": "#f2f2f2", "justify": justify}))?;
        r.get("layer").and_then(Json::as_u64).ok_or_else(|| "no text layer".to_string())
    };
    let fade = |s: &mut Engine, l: u64, t0: f64, t1: f64| -> Result<(), String> {
        s.execute("prop.addKey", json!({"layer": l, "path": "transform/opacity", "time": t0, "value": 0}))?;
        s.execute("prop.addKey", json!({"layer": l, "path": "transform/opacity", "time": t1, "value": 100}))?;
        Ok(())
    };
    match b.slug {
        "title-card" => {
            let l = text(&mut s, "Your Title", 120.0, [wf / 2.0, hf / 2.0], "center")?;
            fade(&mut s, l, 0.0, 0.8)?;
            let sub = text(&mut s, "A subtitle goes here", 44.0, [wf / 2.0, hf / 2.0 + 110.0], "center")?;
            fade(&mut s, sub, 0.4, 1.2)?;
        }
        "lower-third" => {
            run(&mut s, "layer.newSolid", json!({"name": "Bar", "color": "#ff7a1a", "width": 720, "height": 120}))?;
            let bar = s.state.selected_layers.first().copied().ok_or("no bar layer")?;
            s.execute("prop.addKey", json!({"layer": bar, "path": "transform/position", "time": 0.0, "value": [-400.0, hf - 200.0]}))?;
            s.execute("prop.addKey", json!({"layer": bar, "path": "transform/position", "time": 0.6, "value": [460.0, hf - 200.0]}))?;
            let n = text(&mut s, "Name Surname", 56.0, [140.0, hf - 205.0], "left")?;
            fade(&mut s, n, 0.4, 0.9)?;
            let r = text(&mut s, "Role or place", 34.0, [140.0, hf - 120.0], "left")?;
            fade(&mut s, r, 0.6, 1.1)?;
        }
        "social-square" => {
            run(&mut s, "layer.newSolid", json!({"name": "Band", "color": "#ff7a1a", "width": w, "height": 220}))?;
            let band = s.state.selected_layers.first().copied().ok_or("no band layer")?;
            s.execute("layer.setTransform", json!({"layers": [band], "prop": "position", "value": [wf / 2.0, hf - 160.0]}))?;
            let l = text(&mut s, "Headline", 110.0, [wf / 2.0, hf / 2.0 - 60.0], "center")?;
            fade(&mut s, l, 0.0, 0.8)?;
        }
        _ => {
            let l = text(&mut s, "Headline", 120.0, [wf / 2.0, 360.0], "center")?;
            fade(&mut s, l, 0.0, 0.8)?;
            let c = text(&mut s, "Caption at the foot of the frame", 52.0, [wf / 2.0, hf - 260.0], "center")?;
            fade(&mut s, c, 0.6, 1.4)?;
        }
    }
    Ok((*s.project).clone())
}

fn primary(pj: &Project) -> Option<u64> {
    let comps: Vec<u64> = pj.items.iter().filter(|i| matches!(i.kind, ItemKind::Comp(_))).map(|i| i.id).collect();
    let used = |id: u64| pj.items.iter().any(|i| matches!(&i.kind, ItemKind::Comp(c) if c.layers.iter().any(|l| l.source == Some(id))));
    comps.iter().copied().find(|c| !used(*c)).or_else(|| comps.first().copied())
}

fn thumb(e: &Engine, pj: &Project) -> Vec<u8> {
    let mut rgb = vec![0u8; TW * TH * 3];
    let Some(cid) = primary(pj) else { return rgb };
    let Some(comp) = pj.comp(cid) else { return rgb };
    let s = (TW as f64 / comp.width.max(1) as f64).min(TH as f64 / comp.height.max(1) as f64).min(1.0);
    let opts = crate::render::comp::RenderOpts { scale: s, guides: false, ..Default::default() };
    let r = crate::render::comp::Renderer::new(pj, &e.media, opts);
    let t = if comp.poster_time > 0.0 { comp.poster_time } else { (comp.duration * 0.5).min(2.0) };
    let img = r.comp_frame(cid, t);
    let px = img.to_rgba8_over([comp.background[0], comp.background[1], comp.background[2]]);
    let (ox, oy) = ((TW.saturating_sub(img.w)) / 2, (TH.saturating_sub(img.h)) / 2);
    for y in 0..img.h.min(TH) {
        for x in 0..img.w.min(TW) {
            let (si, di) = ((y * img.w + x) * 4, ((y + oy) * TW + x + ox) * 3);
            if si + 2 < px.len() && di + 2 < rgb.len() {
                rgb[di..di + 3].copy_from_slice(&px[si..si + 3]);
            }
        }
    }
    rgb
}

fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

fn read(path: &Path) -> Result<(Json, HashMap<String, Vec<u8>>), String> {
    let bytes = std::fs::read(path).map_err(|x| format!("{}: {x}", path.display()))?;
    let files = crate::engine::lottie::unzip(&bytes).map_err(|x| format!("{}: {x}", path.display()))?;
    let doc: Json = serde_json::from_slice(files.get("template.json").ok_or_else(|| format!("{} is not a formvfx project template", path.display()))?).map_err(|x| format!("{}: {x}", path.display()))?;
    if !doc.get("format").and_then(Json::as_str).is_some_and(|f| f.starts_with("formvfx.projectTemplate/")) {
        return Err(format!("{} is not a formvfx project template", path.display()));
    }
    Ok((doc, files))
}

fn user_files() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(templates_dir()).map(|rd| rd.flatten().map(|x| x.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case(EXT))).collect()).unwrap_or_default();
    v.sort();
    v
}

fn summary(pj: &Project) -> Json {
    let cid = primary(pj);
    let c = cid.and_then(|c| pj.comp(c));
    json!({"width": c.map(|c| c.width), "height": c.map(|c| c.height), "frameRate": c.map(|c| c.frame_rate), "duration": c.map(|c| c.duration), "controls": c.map(|c| c.essential.len()).unwrap_or(0), "comp": cid.and_then(|i| pj.item(i)).map(|i| i.name.clone())})
}

enum Source {
    Builtin(&'static Builtin),
    User(PathBuf),
}

fn resolve(id: &str) -> Result<Source, String> {
    if let Some(slug) = id.strip_prefix("builtin/") {
        return BUILTINS.iter().find(|b| b.slug == slug).map(Source::Builtin).ok_or_else(|| format!("no built-in template `{slug}`"));
    }
    let file = id.strip_prefix("user/").unwrap_or(id);
    let path = templates_dir().join(file);
    let path = if path.extension().is_none() { path.with_extension(EXT) } else { path };
    if Path::new(file).components().count() != 1 || !path.is_file() {
        return Err(format!("no project template `{id}` (templates.list shows them)"));
    }
    Ok(Source::User(path))
}

pub fn list(e: &mut Engine, p: &Json) -> Res {
    let thumbs = pb(p, "thumbnails").unwrap_or(false);
    let mut rows = vec![];
    for b in &BUILTINS {
        let mut row = json!({"id": format!("builtin/{}", b.slug), "name": b.name, "description": b.description, "category": b.category, "builtin": true, "width": b.size.0, "height": b.size.1, "frameRate": 30, "duration": b.duration, "controls": 0});
        if thumbs {
            let pj = build_builtin(b)?;
            row["thumbnail"] = json!(hex(&thumb(e, &pj)));
        }
        rows.push(row);
    }
    for path in user_files() {
        let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        match read(&path) {
            Ok((doc, files)) => {
                let mut row = json!({"id": format!("user/{file}"), "name": doc["name"], "description": doc["description"], "category": doc["category"], "builtin": false, "width": doc["width"], "height": doc["height"], "frameRate": doc["frameRate"], "duration": doc["duration"], "controls": doc["controls"], "path": path.to_string_lossy()});
                if thumbs {
                    row["thumbnail"] = json!(files.get("thumbnail.rgb").map(|b| hex(b)).unwrap_or_default());
                }
                rows.push(row);
            }
            Err(err) => rows.push(json!({"id": format!("user/{file}"), "name": file, "builtin": false, "error": err})),
        }
    }
    Ok(Json::Array(rows))
}

pub fn thumbnail(e: &mut Engine, p: &Json) -> Res {
    let id = ps(p, "id").ok_or("pass `id` (from templates.list)")?;
    let rgb = match resolve(id)? {
        Source::Builtin(b) => thumb(e, &build_builtin(b)?),
        Source::User(path) => {
            let (_, files) = read(&path)?;
            files.get("thumbnail.rgb").cloned().unwrap_or_else(|| vec![0; TW * TH * 3])
        }
    };
    Ok(json!({"id": id, "width": TW, "height": TH, "rgb": hex(&rgb)}))
}

pub fn create(e: &mut Engine, p: &Json) -> Res {
    let src = match (ps(p, "id"), ps(p, "path")) {
        (Some(id), _) => resolve(id)?,
        (None, Some(path)) => Source::User(PathBuf::from(path)),
        _ => return Err("pass `id` (from templates.list) or `path` (a .fvxprojt file)".into()),
    };
    let project_path = ps(p, "projectPath").map(|x| {
        let pb_ = PathBuf::from(x);
        if pb_.extension().is_some_and(|e| e.eq_ignore_ascii_case("fvxproj")) { pb_ } else { pb_.with_extension("fvxproj") }
    });
    let (name, mut pj, files) = match src {
        Source::Builtin(b) => (b.name.to_string(), build_builtin(b)?, HashMap::new()),
        Source::User(path) => {
            let (doc, files) = read(&path)?;
            let pj: Project = serde_json::from_value(doc.get("project").cloned().ok_or("the template holds no project")?).map_err(|x| format!("{}: {x}", path.display()))?;
            (doc.get("name").and_then(Json::as_str).unwrap_or("Template").to_string(), pj, files)
        }
    };
    let embedded: Vec<(&String, &Vec<u8>)> = files.iter().filter(|(n, _)| n.starts_with("media/")).collect();
    let mut extracted = 0;
    if !embedded.is_empty() {
        let dir = match (ps(p, "footageDir"), &project_path) {
            (Some(d), _) => PathBuf::from(d),
            (None, Some(pp)) => pp.with_file_name(format!("{} Footage", pp.file_stem().and_then(|s| s.to_str()).unwrap_or("Project"))),
            (None, None) => crate::render::diskcache::data_dir().join("Template Footage").join(safe(&name)),
        };
        std::fs::create_dir_all(&dir).map_err(|x| format!("{}: {x}", dir.display()))?;
        for (n, b) in &embedded {
            let target = dir.join(&n["media/".len()..]);
            std::fs::write(&target, b).map_err(|x| format!("{}: {x}", target.display()))?;
            extracted += 1;
        }
        let at = |n: &str| if n.starts_with("media/") { dir.join(&n["media/".len()..]).to_string_lossy().to_string() } else { n.to_string() };
        for it in pj.items.iter_mut() {
            let fix = |f: &mut crate::model::Footage| {
                f.path = at(&f.path);
                f.sequence = f.sequence.iter().map(|s| at(s)).collect();
                f.missing = !f.path.is_empty() && !Path::new(&f.path).exists();
            };
            if let ItemKind::Footage(f) = &mut it.kind {
                fix(f);
            }
            if let Some(px) = it.proxy.as_mut() {
                fix(&mut px.footage);
            }
        }
    }
    let comps = pj.items.iter().filter(|i| matches!(i.kind, ItemKind::Comp(_))).count();
    let items = pj.items.len();
    let open = primary(&pj);
    e.reset(pj, None);
    e.imported_from = None;
    if let Some(c) = open {
        let _ = super::comp::open(e, &json!({"comp": c}));
    }
    let mut saved = Json::Null;
    if let Some(pp) = project_path {
        let s = pp.to_string_lossy().to_string();
        super::file::write_project(e, &s)?;
        e.path = Some(s.clone());
        e.saved_revision = e.revision;
        super::session::note_project(e, &s);
        saved = json!(s);
    }
    Ok(json!({"template": name, "comps": comps, "items": items, "footage": extracted, "path": saved, "comp": open}))
}

fn files_of(f: &crate::model::Footage) -> Vec<String> {
    if f.kind == FootageKind::Sequence && !f.sequence.is_empty() { f.sequence.clone() } else if f.path.is_empty() { vec![] } else { vec![f.path.clone()] }
}

pub fn save_as(e: &mut Engine, p: &Json) -> Res {
    let name = ps(p, "name").map(str::trim).filter(|s| !s.is_empty()).ok_or("pass `name`")?.to_string();
    if e.project.items.is_empty() {
        return Err("the project is empty".into());
    }
    let path = templates_dir().join(format!("{}.{EXT}", safe(&name)));
    if path.exists() && !pb(p, "overwrite").unwrap_or(true) {
        return Err(format!("a template named {name} exists (pass overwrite: true to replace it)"));
    }
    let embed = pb(p, "embedFootage").unwrap_or(true);
    let limit = (pf(p, "embedLimitMB").unwrap_or(256.0).max(0.0) * 1_000_000.0) as u64;
    let mut pj: Project = (*e.project).clone();
    pj.render_queue.clear();
    pj.format = crate::model::FORMAT.into();
    pj.saved_by = format!("formvfx {}", env!("CARGO_PKG_VERSION"));
    let mut media: Vec<(String, Vec<u8>)> = vec![];
    let mut names: HashMap<String, String> = HashMap::new();
    let mut referenced: Vec<String> = vec![];
    let mut total: u64 = 0;
    let mut take = |f: &mut crate::model::Footage| {
        if !embed {
            return;
        }
        let mut rename = |src: &str| -> String {
            if let Some(n) = names.get(src) {
                return n.clone();
            }
            let size = std::fs::metadata(src).map(|m| m.len()).unwrap_or(u64::MAX);
            if size == u64::MAX || total + size > limit {
                if !referenced.iter().any(|r| r == src) {
                    referenced.push(src.to_string());
                }
                return src.to_string();
            }
            let base = Path::new(src).file_name().and_then(|n| n.to_str()).unwrap_or("media").to_string();
            let n = format!("media/{}_{}", names.len() + 1, base);
            match std::fs::read(src) {
                Ok(b) => {
                    total += b.len() as u64;
                    media.push((n.clone(), b));
                    names.insert(src.to_string(), n.clone());
                    n
                }
                Err(_) => {
                    referenced.push(src.to_string());
                    src.to_string()
                }
            }
        };
        let seq: Vec<String> = files_of(f).iter().map(|s| rename(s)).collect();
        if f.kind == FootageKind::Sequence && !f.sequence.is_empty() {
            f.path = seq.first().cloned().unwrap_or_default();
            f.sequence = seq;
        } else if let Some(n) = seq.first() {
            f.path = n.clone();
        }
    };
    for it in pj.items.iter_mut() {
        if let ItemKind::Footage(f) = &mut it.kind {
            take(f);
        }
        if let Some(px) = it.proxy.as_mut() {
            take(&mut px.footage);
        }
    }
    let rgb = thumb(e, &e.project);
    let sum = summary(&e.project);
    let doc = json!({
        "format": FORMAT,
        "name": name,
        "description": ps(p, "description").unwrap_or(""),
        "category": ps(p, "category").unwrap_or("User"),
        "width": sum["width"], "height": sum["height"], "frameRate": sum["frameRate"], "duration": sum["duration"], "controls": sum["controls"], "comp": sum["comp"],
        "createdBy": pj.saved_by,
        "project": serde_json::to_value(&pj).map_err(|x| x.to_string())?,
    });
    let mut files: Vec<(&str, Vec<u8>)> = vec![("template.json", serde_json::to_vec_pretty(&doc).map_err(|x| x.to_string())?), ("thumbnail.rgb", rgb.clone())];
    if let Some(img) = image::RgbImage::from_raw(TW as u32, TH as u32, rgb) {
        let mut out = std::io::Cursor::new(vec![]);
        if img.write_to(&mut out, image::ImageFormat::Png).is_ok() {
            files.push(("thumbnail.png", out.into_inner()));
        }
    }
    for (n, b) in &media {
        files.push((n.as_str(), b.clone()));
    }
    std::fs::create_dir_all(templates_dir()).map_err(|x| format!("{}: {x}", templates_dir().display()))?;
    std::fs::write(&path, crate::engine::lottie::zip(&files)).map_err(|x| format!("{}: {x}", path.display()))?;
    let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    Ok(json!({"id": format!("user/{file}"), "path": path.to_string_lossy(), "name": name, "embedded": media.len(), "referenced": referenced}))
}

pub fn delete(_: &mut Engine, p: &Json) -> Res {
    let id = ps(p, "id").ok_or("pass `id` (user/<file> from templates.list)")?;
    match resolve(id)? {
        Source::Builtin(_) => Err("built-in templates cannot be deleted".into()),
        Source::User(path) => {
            std::fs::remove_file(&path).map_err(|x| format!("{}: {x}", path.display()))?;
            Ok(json!({"deleted": id}))
        }
    }
}
