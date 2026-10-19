use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use super::{add_layer, footage_layer, select_only};
use crate::engine::{Engine, Res, pb, pf, ps, pu};
use crate::model::prop::{Group, GroupKind, Node, Ui};
use crate::model::{Controller, ItemKind, LayerKind, Project, Value};
use crate::render::essential::{GROUP, overridden};

pub const EXT: &str = "fvxtemplate";
pub const MOGRT: &str = "mogrt";
const INNER: &str = "project.fvxtemplate";
const FORMAT: &str = "formvfx.template/1";

pub fn templates_dir() -> PathBuf {
    crate::render::diskcache::data_dir().join("Templates")
}

fn comp_param(e: &Engine, p: &Json) -> Result<u64, String> {
    e.comp_id(p.get("comp"))
}

fn ctrl_index(e: &Engine, cid: u64, p: &Json) -> Result<usize, String> {
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let key = p.get("controller").ok_or("pass `controller` (its id, name or 1-based index)")?;
    let found = match key {
        Json::Number(n) => n.as_u64().and_then(|i| (i as usize).checked_sub(1)).filter(|i| *i < comp.essential.len()),
        Json::String(s) => comp.essential.iter().position(|c| &c.id == s).or_else(|| comp.essential.iter().position(|c| &c.name == s)),
        _ => None,
    };
    found.ok_or_else(|| "no such Essential Graphics controller (essential.list shows them)".into())
}

fn next_id(list: &[Controller]) -> String {
    let n = list.iter().filter_map(|c| c.id.strip_prefix('c')?.parse::<u64>().ok()).max().unwrap_or(0);
    format!("c{}", n + 1)
}

fn start_template(e: &mut Engine, cid: u64) {
    let pj = e.proj();
    let name = pj.items.iter().find(|i| i.id == cid).map(|i| i.name.clone()).unwrap_or_default();
    if let Some(c) = pj.comp_mut(cid)
        && c.template_name.is_empty()
    {
        c.template_name = name;
    }
}

pub fn add_property(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = if p.get("path").is_none() && p.get("prop").is_none() {
        let cid = comp_param(e, p)?;
        let (lid, uid) = e.state.selected_props.iter().copied().find(|(l, _)| e.project.comp(cid).is_some_and(|c| c.layer(*l).is_some())).ok_or("select a property in the Timeline or pass `layer` and `path`")?;
        (cid, lid, uid)
    } else {
        super::prop::locate(e, p)?
    };
    let l = e.layer_ref(cid, lid)?;
    let pr = l.props.find(uid).ok_or("only properties can be added, not groups")?;
    if matches!(pr.ui, Ui::Hidden | Ui::Mask | Ui::Path) {
        return Err(format!("{} cannot be added to Essential Graphics", pr.name));
    }
    let comp = e.project.comp(cid).ok_or("no composition")?;
    if let Some(c) = comp.essential.iter().find(|c| c.layer == lid && c.prop == uid) {
        return Ok(json!({"controller": c.id, "existing": true}));
    }
    let name = ps(p, "name").map(str::to_string).unwrap_or_else(|| pr.name.clone());
    let id = next_id(&comp.essential);
    let group = ps(p, "group").unwrap_or("").to_string();
    start_template(e, cid);
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    c.essential.push(Controller { id: id.clone(), name: name.clone(), kind: "property".into(), layer: lid, prop: uid, group, text: String::new() });
    e.set_label(format!("Add {name} to Essential Graphics"));
    Ok(json!({"controller": id, "name": name}))
}

pub fn add_comment(e: &mut Engine, p: &Json) -> Res {
    let cid = comp_param(e, p)?;
    let text = ps(p, "text").unwrap_or("Comment").to_string();
    start_template(e, cid);
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    let id = next_id(&c.essential);
    c.essential.push(Controller { id: id.clone(), name: "Comment".into(), kind: "comment".into(), text, ..Default::default() });
    e.set_label("Add Comment");
    Ok(json!({"controller": id}))
}

pub fn add_group(e: &mut Engine, p: &Json) -> Res {
    let cid = comp_param(e, p)?;
    let name = ps(p, "name").unwrap_or("Group").to_string();
    start_template(e, cid);
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    let id = next_id(&c.essential);
    c.essential.push(Controller { id: id.clone(), name, kind: "group".into(), ..Default::default() });
    e.set_label("Add Group");
    Ok(json!({"controller": id}))
}

pub fn remove(e: &mut Engine, p: &Json) -> Res {
    let cid = comp_param(e, p)?;
    let i = ctrl_index(e, cid, p)?;
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    let gone = c.essential.remove(i);
    c.essential.retain(|x| !(x.kind == "mirror" && x.text == gone.id));
    if gone.kind == "group" {
        for x in c.essential.iter_mut().filter(|x| x.group == gone.id) {
            x.group.clear();
        }
    }
    e.set_label(format!("Remove {}", gone.name));
    Ok(json!({"removed": gone.id}))
}

pub fn rename(e: &mut Engine, p: &Json) -> Res {
    let cid = comp_param(e, p)?;
    let i = ctrl_index(e, cid, p)?;
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    let x = &mut c.essential[i];
    if let Some(t) = ps(p, "text") {
        x.text = t.to_string();
    }
    if let Some(n) = ps(p, "name") {
        x.name = n.to_string();
    }
    let id = x.id.clone();
    e.set_label("Rename Controller");
    Ok(json!({"controller": id}))
}

pub fn move_to(e: &mut Engine, p: &Json) -> Res {
    let cid = comp_param(e, p)?;
    let i = ctrl_index(e, cid, p)?;
    let to = pu(p, "index").ok_or("pass `index` (1-based)")? as usize;
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    let mut x = c.essential.remove(i);
    if let Some(g) = p.get("group") {
        x.group = g.as_str().unwrap_or("").to_string();
    }
    let at = to.saturating_sub(1).min(c.essential.len());
    c.essential.insert(at, x);
    e.set_label("Move Controller");
    Ok(json!({"index": at + 1}))
}

pub fn set_template_name(e: &mut Engine, p: &Json) -> Res {
    let cid = comp_param(e, p)?;
    let name = ps(p, "name").ok_or("pass `name`")?.to_string();
    e.proj().comp_mut(cid).ok_or("no composition")?.template_name = name.clone();
    e.state.essential_comp = Some(cid);
    e.set_label("Set Template Name");
    Ok(json!({"comp": cid, "templateName": name}))
}

pub fn open(e: &mut Engine, p: &Json) -> Res {
    let cid = comp_param(e, p)?;
    e.state.essential_comp = Some(cid);
    e.frontend("window.panel", json!({"panel": "essential"}));
    Ok(json!({"comp": cid}))
}

pub fn prop_paths(c: &crate::model::Comp) -> Vec<Option<String>> {
    c.essential.iter().map(|x| c.layer(x.layer).and_then(|l| l.props.path_of(x.prop))).collect()
}

pub fn remap(c: &mut crate::model::Comp, layers: &HashMap<u64, u64>, paths: Vec<Option<String>>) {
    let fixed: Vec<(u64, Option<u64>)> = c
        .essential
        .iter()
        .zip(paths)
        .map(|(x, p)| {
            let lid = layers.get(&x.layer).copied().unwrap_or(x.layer);
            (lid, p.and_then(|p| c.layer(lid)?.props.resolve_uid(&p)))
        })
        .collect();
    for (x, (lid, u)) in c.essential.iter_mut().zip(fixed) {
        x.layer = lid;
        if let Some(u) = u {
            x.prop = u;
        }
    }
}

fn master_value(pr: &crate::model::Prop) -> Value {
    pr.keys.first().map(|k| k.value.clone()).unwrap_or_else(|| pr.value.clone())
}

pub fn controllers_json(pj: &Project, cid: u64) -> Json {
    let Some(comp) = pj.comp(cid) else { return json!([]) };
    let rows: Vec<Json> = comp
        .essential
        .iter()
        .map(|c| {
            let mut o = json!({"id": c.id, "name": c.name, "kind": c.kind, "group": c.group});
            if c.kind == "comment" {
                o["text"] = json!(c.text);
            }
            if c.kind == "mirror" {
                o["mirrorOf"] = json!(c.text);
            }
            if c.kind == "media" {
                match comp.layer(c.layer) {
                    Some(l) => {
                        o["layer"] = json!(l.id);
                        o["layerName"] = json!(l.name);
                        o["value"] = json!(l.source);
                        o["itemName"] = json!(l.source.and_then(|s| pj.item(s)).map(|i| i.name.clone()));
                        o["type"] = json!("media");
                    }
                    None => o["missing"] = json!(true),
                }
            }
            if c.kind == "property" || c.kind == "mirror" {
                match comp.layer(c.layer).and_then(|l| Some((l, l.props.find(c.prop)?))) {
                    Some((l, pr)) => {
                        o["layer"] = json!(l.id);
                        o["layerName"] = json!(l.name);
                        o["path"] = json!(l.props.path_of(pr.uid));
                        o["value"] = master_value(pr).to_json();
                        o["type"] = json!(pr.value.kind_name());
                        o["ui"] = serde_json::to_value(&pr.ui).unwrap_or(Json::Null);
                        o["animated"] = json!(pr.is_animated());
                    }
                    None => o["missing"] = json!(true),
                }
            }
            o
        })
        .collect();
    json!(rows)
}

pub fn list(e: &mut Engine, p: &Json) -> Res {
    let cid = match p.get("comp") {
        Some(v) if !v.is_null() => e.comp_id(Some(v))?,
        _ => e.state.essential_comp.filter(|c| e.project.comp(*c).is_some()).or(e.state.active_comp).ok_or("open a composition")?,
    };
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let name = e.project.item(cid).map(|i| i.name.clone()).unwrap_or_default();
    let instances: Vec<Json> = e
        .project
        .items
        .iter()
        .filter_map(|it| match &it.kind {
            ItemKind::Comp(c) => Some((it.id, c)),
            _ => None,
        })
        .flat_map(|(oid, c)| c.layers.iter().filter(|l| l.source == Some(cid) && l.kind == LayerKind::Precomp).map(move |l| json!({"comp": oid, "layer": l.id, "name": l.name})))
        .collect();
    Ok(json!({"comp": cid, "compName": name, "templateName": if comp.template_name.is_empty() { name.clone() } else { comp.template_name.clone() }, "controllers": controllers_json(&e.project, cid), "instances": instances}))
}

pub fn revert(e: &mut Engine, p: &Json) -> Res {
    let (cid, lids) = e.layers_param(p)?;
    let only = ps(p, "controller").map(str::to_string);
    let mut n = 0;
    for lid in lids {
        let l = e.layer_mut(cid, lid)?;
        let Some(g) = l.props.sub_mut(GROUP) else { continue };
        for c in g.children.iter_mut() {
            let Node::Prop(pr) = c else { continue };
            if only.as_ref().is_some_and(|o| o != &pr.id && o != &pr.name) {
                continue;
            }
            pr.value = pr.default.clone();
            pr.keys.clear();
            pr.expr = None;
            n += 1;
        }
    }
    e.set_label("Revert to Master");
    Ok(json!({"reverted": n}))
}

type Wanted = Vec<(String, String, Ui, Value)>;

fn wanted(pj: &Project) -> HashMap<u64, Wanted> {
    let mut out = HashMap::new();
    for it in &pj.items {
        let ItemKind::Comp(c) = &it.kind else { continue };
        let w: Wanted = c
            .essential
            .iter()
            .filter(|x| x.kind == "property" || x.kind == "media")
            .filter_map(|x| {
                let l = c.layer(x.layer)?;
                if x.kind == "media" {
                    return Some((x.id.clone(), x.name.clone(), Ui::Plain, Value::Num(l.source? as f64)));
                }
                let pr = l.props.find(x.prop)?;
                Some((x.id.clone(), x.name.clone(), pr.ui.clone(), master_value(pr)))
            })
            .collect();
        if !w.is_empty() {
            out.insert(it.id, w);
        }
    }
    out
}

fn in_step(g: Option<&Group>, want: Option<&Wanted>) -> bool {
    match (g, want) {
        (None, None) => true,
        (Some(g), Some(w)) => {
            g.children.len() == w.len()
                && g.children.iter().zip(w.iter()).all(|(c, (id, name, ui, v))| match c {
                    Node::Prop(pr) => &pr.id == id && &pr.name == name && &pr.ui == ui && &pr.default == v && (overridden(pr) || &pr.value == v),
                    Node::Group(_) => false,
                })
        }
        _ => false,
    }
}

pub fn sync(e: &mut Engine) {
    let want = wanted(&e.project);
    let mut todo: Vec<(u64, u64)> = vec![];
    for it in &e.project.items {
        let ItemKind::Comp(c) = &it.kind else { continue };
        for l in &c.layers {
            if l.kind != LayerKind::Precomp {
                continue;
            }
            let w = l.source.and_then(|s| want.get(&s));
            if !in_step(l.props.sub(GROUP), w) {
                todo.push((it.id, l.id));
            }
        }
    }
    if todo.is_empty() {
        return;
    }
    let pj = e.proj();
    for (cid, lid) in todo {
        let src = pj.comp(cid).and_then(|c| c.layer(lid)).and_then(|l| l.source);
        let w = src.and_then(|s| want.get(&s)).cloned();
        let mut uid = pj.next_uid;
        let Some(l) = pj.comp_mut(cid).and_then(|c| c.layer_mut(lid)) else { continue };
        match w {
            None => l.props.children.retain(|c| c.id() != GROUP),
            Some(w) => {
                let old: Vec<crate::model::Prop> = l.props.sub(GROUP).map(|g| g.children.iter().filter_map(|c| if let Node::Prop(pr) = c { Some(pr.clone()) } else { None }).collect()).unwrap_or_default();
                let children: Vec<Node> = w
                    .into_iter()
                    .map(|(id, name, ui, v)| {
                        let pr = match old.iter().find(|o| o.id == id) {
                            Some(o) => {
                                let mut pr = o.clone();
                                let keep = overridden(o);
                                pr.name = name;
                                pr.ui = ui;
                                pr.default = v.clone();
                                if !keep {
                                    pr.value = v;
                                }
                                pr
                            }
                            None => {
                                uid += 1;
                                crate::model::Prop::new(uid, &id, &name, v, ui)
                            }
                        };
                        Node::Prop(pr)
                    })
                    .collect();
                match l.props.sub_mut(GROUP) {
                    Some(g) => g.children = children,
                    None => {
                        uid += 1;
                        let g = Group::new(uid, GROUP, "Essential Properties", GroupKind::Essential).with(children);
                        let at = l.props.children.iter().position(|c| c.id() == "transform").unwrap_or(l.props.children.len());
                        l.props.children.insert(at, Node::Group(g));
                    }
                }
            }
        }
        pj.next_uid = uid;
    }
}

fn safe(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' }).collect();
    let t = t.trim().to_string();
    if t.is_empty() { "Template".into() } else { t }
}

fn files_of(f: &crate::model::Footage) -> Vec<String> {
    if f.kind == crate::model::FootageKind::Sequence && !f.sequence.is_empty() { f.sequence.clone() } else if f.path.is_empty() { vec![] } else { vec![f.path.clone()] }
}

fn poster(e: &Engine, pj: &Project, cid: u64) -> Option<Vec<u8>> {
    let comp = pj.comp(cid)?;
    let s = (320.0 / comp.width.max(comp.height).max(1) as f64).min(1.0);
    let opts = crate::render::comp::RenderOpts { scale: s, guides: false, ..Default::default() };
    let r = crate::render::comp::Renderer::new(pj, &e.media, opts);
    let img = r.comp_frame(cid, comp.poster_time);
    let buf = image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.to_rgba8())?;
    let mut out = std::io::Cursor::new(vec![]);
    buf.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

pub fn export_template(e: &mut Engine, p: &Json) -> Res {
    let cid = comp_param(e, p)?;
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let comp_name = e.project.item(cid).map(|i| i.name.clone()).unwrap_or_default();
    let tname = ps(p, "name").map(str::to_string).unwrap_or_else(|| if comp.template_name.is_empty() { comp_name.clone() } else { comp.template_name.clone() });
    let path = match ps(p, "path") {
        Some(x) if Path::new(x).extension().is_some_and(|e| e.eq_ignore_ascii_case(EXT) || e.eq_ignore_ascii_case(MOGRT)) => PathBuf::from(x),
        Some(x) => PathBuf::from(x).with_extension(EXT),
        None => ps(p, "folder").map(PathBuf::from).unwrap_or_else(templates_dir).join(format!("{}.{EXT}", safe(&tname))),
    };
    if path.exists() && !pb(p, "overwrite").unwrap_or(true) {
        return Err(format!("{} exists", path.display()));
    }
    let keep = super::deps::used_by(&e.project, &[cid]);
    let mut pj: Project = (*e.project).clone();
    pj.items.retain(|i| keep.contains(&i.id));
    pj.render_queue.clear();
    let mut media: Vec<(String, Vec<u8>)> = vec![];
    let mut names: HashMap<String, String> = HashMap::new();
    let mut missing = vec![];
    let mut take = |f: &mut crate::model::Footage| {
        let mut rename = |src: &str| -> String {
            if let Some(n) = names.get(src) {
                return n.clone();
            }
            let base = Path::new(src).file_name().and_then(|n| n.to_str()).unwrap_or("media").to_string();
            let n = format!("{}_{}", names.len() + 1, base);
            match std::fs::read(src) {
                Ok(b) => media.push((format!("media/{n}"), b)),
                Err(_) => missing.push(src.to_string()),
            }
            names.insert(src.to_string(), n.clone());
            n
        };
        let seq: Vec<String> = files_of(f).iter().map(|s| rename(s)).collect();
        if f.kind == crate::model::FootageKind::Sequence && !f.sequence.is_empty() {
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
    pj.format = crate::model::FORMAT.into();
    pj.saved_by = format!("formvfx {}", env!("CARGO_PKG_VERSION"));
    let controllers = controllers_json(&e.project, cid);
    let doc = json!({"format": FORMAT, "name": tname, "comp": cid, "compName": comp_name, "width": comp.width, "height": comp.height, "duration": comp.duration, "frameRate": comp.frame_rate, "controllers": controllers, "createdBy": pj.saved_by, "project": serde_json::to_value(&pj).map_err(|x| x.to_string())?});
    let mut files: Vec<(&str, Vec<u8>)> = vec![("template.json", serde_json::to_vec_pretty(&doc).map_err(|x| x.to_string())?)];
    let shot = poster(e, &e.project, cid);
    if let Some(png) = &shot {
        files.push(("poster.png", png.clone()));
    }
    for (n, b) in &media {
        files.push((n.as_str(), b.clone()));
    }
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|x| format!("{}: {x}", d.display()))?;
    }
    let mut bytes = crate::engine::lottie::zip(&files);
    if is_mogrt(&path.to_string_lossy()) {
        let def = mogrt_definition(&doc);
        let mut outer: Vec<(&str, Vec<u8>)> = vec![("definition.json", serde_json::to_vec_pretty(&def).map_err(|x| x.to_string())?)];
        if let Some(png) = shot {
            outer.push(("thumb.png", png));
        }
        outer.push((INNER, bytes));
        bytes = crate::engine::lottie::zip(&outer);
    }
    std::fs::write(&path, bytes).map_err(|x| format!("{}: {x}", path.display()))?;
    Ok(json!({"path": path.to_string_lossy(), "name": tname, "controllers": doc["controllers"].as_array().map(Vec::len).unwrap_or(0), "media": media.len(), "missing": missing}))
}

fn is_mogrt(path: &str) -> bool {
    Path::new(path).extension().is_some_and(|e| e.eq_ignore_ascii_case(MOGRT))
}

fn read_template(path: &str) -> Result<(Json, HashMap<String, Vec<u8>>), String> {
    let mut bytes = std::fs::read(path).map_err(|x| format!("{path}: {x}"))?;
    if is_mogrt(path) {
        let outer = crate::engine::lottie::unzip(&bytes).map_err(|x| format!("{path}: {x}"))?;
        bytes = match outer.get(INNER) {
            Some(b) => b.clone(),
            None if outer.keys().any(|k| k.ends_with(".aegraphic") || k.ends_with(".prgraphic")) => {
                return Err(format!("{path} was made in another app: its project is in the closed .aegraphic or .prgraphic format, which formvfx cannot open (essential.templateInfo lists its controls)"));
            }
            None => return Err(format!("{path} has no project formvfx can read")),
        };
    }
    let files = crate::engine::lottie::unzip(&bytes).map_err(|x| format!("{path}: {x}"))?;
    let doc: Json = serde_json::from_slice(files.get("template.json").ok_or_else(|| format!("{path} is not a formvfx template (no template.json)"))?).map_err(|x| format!("{path}: {x}"))?;
    if !doc.get("format").and_then(Json::as_str).is_some_and(|f| f.starts_with("formvfx.template/")) {
        return Err(format!("{path} is not a formvfx template"));
    }
    Ok((doc, files))
}

fn digest(path: &str) -> String {
    let b = std::fs::read(path).unwrap_or_default();
    crate::render::roto::sha256_hex(&b)[..12].to_string()
}

pub fn import_template(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (a .fvxtemplate file)")?.to_string();
    if is_mogrt(&path)
        && let Some((comp, folder, name, warnings)) = super::aep::mogrt_project(e, &path)?
    {
        let mut r = finish_import(e, p, comp, folder, &name)?;
        r["warnings"] = json!(warnings);
        return Ok(r);
    }
    let (doc, files) = read_template(&path)?;
    let name = doc.get("name").and_then(Json::as_str).unwrap_or("Template").to_string();
    let dir = templates_dir().join(".imported").join(format!("{}-{}", safe(&name), digest(&path)));
    std::fs::create_dir_all(&dir).map_err(|x| format!("{}: {x}", dir.display()))?;
    for (n, b) in &files {
        if let Some(rest) = n.strip_prefix("media/") {
            let target = dir.join(rest);
            if !target.exists() {
                std::fs::write(&target, b).map_err(|x| format!("{}: {x}", target.display()))?;
            }
        }
    }
    let mut pj: Project = serde_json::from_value(doc.get("project").cloned().ok_or("the template has no project")?).map_err(|x| format!("{path}: {x}"))?;
    let at = |n: &str| if n.is_empty() { String::new() } else { dir.join(n).to_string_lossy().to_string() };
    let fix = |f: &mut crate::model::Footage| {
        f.path = at(&f.path);
        f.sequence = f.sequence.iter().map(|s| at(s)).collect();
        f.missing = !f.path.is_empty() && !Path::new(&f.path).exists();
    };
    for it in pj.items.iter_mut() {
        if let ItemKind::Footage(f) = &mut it.kind {
            fix(f);
        }
        if let Some(px) = it.proxy.as_mut() {
            fix(&mut px.footage);
        }
    }
    let proj_path = dir.join(format!("{}.fvxproj", safe(&name)));
    std::fs::write(&proj_path, serde_json::to_string(&pj).map_err(|x| x.to_string())?).map_err(|x| format!("{}: {x}", proj_path.display()))?;
    let folder = super::file::import_project(e, &proj_path.to_string_lossy(), None)?;
    if let Some(f) = e.proj().items.iter_mut().find(|i| i.id == folder) {
        f.name = name.clone();
    }
    let comp_name = doc.get("compName").and_then(Json::as_str).unwrap_or("");
    let comp = e
        .project
        .items
        .iter()
        .filter(|i| i.parent == Some(folder) || e.project.item(i.parent.unwrap_or(0)).is_some_and(|f| f.parent == Some(folder)))
        .filter_map(|i| match &i.kind {
            ItemKind::Comp(c) => Some((i.id, i.name.as_str(), c)),
            _ => None,
        })
        .find(|(_, n, c)| !c.essential.is_empty() && (comp_name.is_empty() || *n == comp_name))
        .map(|x| x.0)
        .ok_or("the template's primary composition was not found")?;
    finish_import(e, p, comp, folder, &name)
}

fn finish_import(e: &mut Engine, p: &Json, comp: u64, folder: u64, name: &str) -> Res {
    let mut layer = None;
    if pb(p, "addToComp").unwrap_or(false)
        && let Ok(cid) = e.comp_id(p.get("comp"))
        && cid != comp
    {
        let time = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
        let l = footage_layer(e, cid, comp, time, None, None)?;
        let lid = add_layer(e, cid, l, None);
        select_only(e, &[lid]);
        layer = Some(lid);
    }
    e.state.project_selection = vec![comp];
    e.set_label(format!("Import {name}"));
    Ok(json!({"comp": comp, "folder": folder, "layer": layer, "name": name}))
}

pub fn templates(_: &mut Engine, _: &Json) -> Res {
    let dir = templates_dir();
    let mut rows = vec![];
    if let Ok(rd) = std::fs::read_dir(&dir) {
        let mut paths: Vec<PathBuf> = rd.filter_map(|x| x.ok().map(|x| x.path())).filter(|x| x.extension().is_some_and(|e| e.eq_ignore_ascii_case(EXT) || e.eq_ignore_ascii_case(MOGRT))).collect();
        paths.sort();
        for path in paths {
            let s = path.to_string_lossy().to_string();
            match read_template(&s) {
                Ok((doc, files)) => {
                    let poster = files.get("poster.png").map(|b| format!("data:image/png;base64,{}", crate::engine::lottie::b64(b)));
                    rows.push(json!({"path": s, "name": doc["name"], "width": doc["width"], "height": doc["height"], "duration": doc["duration"], "controllers": doc["controllers"].as_array().map(Vec::len).unwrap_or(0), "poster": poster}));
                }
                Err(err) => match mogrt_info(&s) {
                    Ok(info) if is_mogrt(&s) => rows.push(json!({"path": s, "name": info["name"], "width": info["width"], "height": info["height"], "duration": info["duration"], "controllers": info["controls"].as_array().map(Vec::len).unwrap_or(0), "poster": info["poster"], "error": err})),
                    _ => rows.push(json!({"path": s, "error": err})),
                },
            }
        }
    }
    Ok(json!({"folder": dir.to_string_lossy(), "templates": rows}))
}

pub fn install_template(_: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (a .fvxtemplate or .mogrt file)")?;
    let doc = match read_template(path) {
        Ok((doc, _)) => doc,
        Err(err) if is_mogrt(path) => json!({"name": mogrt_info(path).map_err(|_| err)?["name"]}),
        Err(err) => return Err(err),
    };
    let dir = templates_dir();
    std::fs::create_dir_all(&dir).map_err(|x| format!("{}: {x}", dir.display()))?;
    let name = Path::new(path).file_name().ok_or("bad path")?;
    let target = dir.join(name);
    std::fs::copy(path, &target).map_err(|x| format!("{}: {x}", target.display()))?;
    Ok(json!({"path": target.to_string_lossy(), "name": doc["name"]}))
}

const TICKS_PER_SECOND: f64 = 254_016_000_000.0;

const MOGRT_TYPES: [(u64, &str); 7] = [(1, "checkbox"), (2, "slider"), (3, "angle"), (4, "color"), (5, "point"), (6, "text"), (10, "group")];

fn kind_of(v: &Json) -> &'static str {
    match v {
        Json::Bool(_) => "checkbox",
        Json::Number(_) => "slider",
        Json::String(_) => "text",
        Json::Array(a) if a.len() >= 3 && a.iter().all(|x| x.as_f64().is_some_and(|f| (0.0..=1.0).contains(&f))) => "color",
        Json::Array(a) if a.iter().all(Json::is_string) => "group",
        Json::Array(a) if a.len() == 2 || a.len() == 3 => "point",
        Json::Object(o) if o.contains_key("x") && o.contains_key("y") => "point",
        Json::Object(o) if o.contains_key("text") || o.contains_key("strDB") => "text",
        _ => "other",
    }
}

fn str_db(s: &str) -> Json {
    json!({"strDB": [{"localeString": "en_US", "str": s}]})
}

fn mogrt_definition(doc: &Json) -> Json {
    let name = doc["name"].as_str().unwrap_or("Template");
    let all = doc["controllers"].as_array().cloned().unwrap_or_default();
    let controls: Vec<Json> = all
        .iter()
        .filter(|c| matches!(c["kind"].as_str(), Some("property" | "media" | "group" | "comment")))
        .map(|c| {
            let ui = c["ui"]["kind"].as_str().unwrap_or("");
            let raw = if c["value"].get("text").is_some() { c["value"]["text"].clone() } else { c["value"].clone() };
            let kind = match c["kind"].as_str() {
                Some("property") if ui == "Angle" => "angle",
                Some("property") => kind_of(&raw),
                Some(k) => k,
                None => "other",
            };
            let value = match kind {
                "text" => str_db(raw.as_str().unwrap_or("")),
                "point" => json!({"x": raw[0], "y": raw[1]}),
                "group" => json!(all.iter().filter(|x| x["group"] == c["id"]).map(|x| x["id"].clone()).collect::<Vec<_>>()),
                _ => raw,
            };
            let mut o = json!({"id": c["id"], "canAnimate": true, "uiName": str_db(c["name"].as_str().unwrap_or("")), "formvfxKind": c["kind"], "value": value});
            if let Some((t, _)) = MOGRT_TYPES.iter().find(|(_, k)| *k == kind) {
                o["type"] = json!(t);
            }
            if ui == "Slider" {
                o["min"] = c["ui"]["slider_min"].clone();
                o["max"] = c["ui"]["slider_max"].clone();
            }
            o
        })
        .collect();
    let fps = doc["frameRate"].as_f64().unwrap_or(30.0).max(0.001);
    let ms = (doc["duration"].as_f64().unwrap_or(0.0) * 1000.0).round();
    json!({
        "apiVersion": "1.6", "authorApp": "formvfx", "authorAppVersion": env!("CARGO_PKG_VERSION"), "capsuleName": name, "capsuleNameLocalized": str_db(name),
        "sourceInfoLocalized": {"en_US": {"name": name, "framesize": {"size": {"x": doc["width"], "y": doc["height"]}, "topleft": {"x": 0, "y": 0}}, "duration": {"scale": 1000, "value": ms}, "framerate": {"ticksperframe": (TICKS_PER_SECOND / fps).round()}}},
        "platformSupport": ["formvfx"], "clientControls": controls,
    })
}

pub fn localized(v: &Json) -> Option<String> {
    match v {
        Json::String(s) => Some(s.clone()),
        Json::Object(o) => o
            .get("strDB")
            .and_then(Json::as_array)
            .and_then(|a| a.iter().find(|x| x["localeString"].as_str().is_some_and(|l| l.starts_with("en"))).or_else(|| a.first()))
            .and_then(|x| x["str"].as_str().map(str::to_string))
            .or_else(|| o.get("en_US").and_then(|x| x.get("name").and_then(Json::as_str).or(x.as_str())).map(str::to_string)),
        _ => None,
    }
}

fn mogrt_info(path: &str) -> Result<Json, String> {
    let bytes = std::fs::read(path).map_err(|x| format!("{path}: {x}"))?;
    let files = crate::engine::lottie::unzip(&bytes).map_err(|x| format!("{path}: {x}"))?;
    let def: Json = files.get("definition.json").map(|b| serde_json::from_slice(b)).transpose().map_err(|x| format!("{path}: {x}"))?.ok_or_else(|| format!("{path} has no definition.json"))?;
    let name = def.get("capsuleName").and_then(localized).or_else(|| def.get("sourceInfoLocalized").and_then(localized)).unwrap_or_else(|| Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("Template").to_string());
    let raw: Vec<Json> = def.get("clientControls").and_then(Json::as_array).cloned().unwrap_or_default();
    let parent_of = |id: &Json| raw.iter().find(|g| g["type"] == 10 && g["value"].as_array().is_some_and(|a| a.contains(id))).and_then(|g| g["id"].as_str()).map(str::to_string);
    let controls: Vec<Json> = raw
        .iter()
        .map(|c| {
            let v = c.get("value").cloned().unwrap_or(Json::Null);
            let by_type = c.get("type").and_then(Json::as_u64).and_then(|t| MOGRT_TYPES.iter().find(|x| x.0 == t)).map(|x| x.1);
            let kind = c.get("formvfxKind").and_then(Json::as_str).filter(|k| *k == "media" || *k == "comment").or(by_type).unwrap_or_else(|| kind_of(&v)).to_string();
            let value = match kind.as_str() {
                "text" => localized(&v).map(Json::String).unwrap_or(v),
                "point" if v.get("x").is_some() => json!([v["x"], v["y"]]),
                _ => v,
            };
            let mut o = json!({"id": c.get("id"), "name": c.get("uiName").and_then(localized).unwrap_or_default(), "kind": kind, "type": c.get("type"), "value": value, "group": parent_of(&c["id"])});
            if c.get("min").is_some() {
                o["min"] = c["min"].clone();
                o["max"] = c["max"].clone();
            }
            if let Some(f) = c.get("fonteditinfo") {
                o["font"] = f["fontEditValue"].clone();
                o["fontSize"] = f["fontSizeEditValue"].clone();
            }
            o
        })
        .collect();
    let src = def.get("sourceInfoLocalized").and_then(|s| s.get("en_US").or_else(|| s.as_object().and_then(|o| o.values().next())));
    let size = src.and_then(|s| s.get("framesize")).map(|f| (f["size"]["x"].clone(), f["size"]["y"].clone())).unwrap_or_else(|| (def["width"].clone(), def["height"].clone()));
    let duration = src.and_then(|s| s.get("duration")).and_then(|d| Some(d["value"].as_f64()? / d["scale"].as_f64().filter(|x| *x > 0.0)?)).or_else(|| def["duration"].as_f64());
    let fps = src.and_then(|s| s["framerate"]["ticksperframe"].as_f64()).filter(|t| *t > 0.0).map(|t| (TICKS_PER_SECOND / t * 1000.0).round() / 1000.0).or_else(|| def["frameRate"].as_f64());
    let fonts = def.get("usedFontsLocalized").and_then(|f| f.get("en_US").or_else(|| f.as_object().and_then(|o| o.values().next()))).cloned().unwrap_or(json!([]));
    let effects = def.get("usedEffects").cloned().unwrap_or(json!([]));
    let app = if files.contains_key(INNER) {
        "formvfx"
    } else if files.keys().any(|k| k.ends_with(".aegraphic") || k.ends_with(".prgraphic")) {
        "another app"
    } else {
        "unknown"
    };
    let poster = files.get("thumb.png").map(|b| format!("data:image/png;base64,{}", crate::engine::lottie::b64(b)));
    let inner: Vec<String> = files.iter().filter(|(k, _)| k.ends_with(".aegraphic") || k.ends_with(".prgraphic")).filter_map(|(_, b)| crate::engine::lottie::unzip(b).ok()).flat_map(|f| f.into_keys()).collect();
    let opens = app == "formvfx" || (files.keys().any(|k| k.ends_with(".aegraphic")) && inner.iter().any(|k| k.to_ascii_lowercase().ends_with(".aep")));
    Ok(json!({"path": path, "format": "mogrt", "name": name, "madeIn": app, "opens": opens, "width": size.0, "height": size.1, "duration": duration, "frameRate": fps, "platforms": def.get("platformSupport"), "fonts": fonts, "effects": effects, "controls": controls, "poster": poster, "files": files.keys().collect::<Vec<_>>(), "inner": inner}))
}

pub fn template_info(_: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (a .fvxtemplate or .mogrt file)")?;
    if is_mogrt(path) {
        return mogrt_info(path);
    }
    let (doc, files) = read_template(path)?;
    let controls: Vec<Json> = doc["controllers"].as_array().cloned().unwrap_or_default().iter().map(|c| json!({"id": c["id"], "name": c["name"], "kind": c["kind"], "value": c["value"]})).collect();
    let poster = files.get("poster.png").map(|b| format!("data:image/png;base64,{}", crate::engine::lottie::b64(b)));
    Ok(json!({"path": path, "format": "fvxtemplate", "name": doc["name"], "madeIn": "formvfx", "opens": true, "width": doc["width"], "height": doc["height"], "duration": doc["duration"], "controls": controls, "poster": poster, "media": files.keys().filter(|k| k.starts_with("media/")).count()}))
}

pub fn add_media(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    if !matches!(l.kind, LayerKind::Footage | LayerKind::Precomp) || l.source.is_none() {
        return Err("Media Replacement works on footage and precomp layers".into());
    }
    let comp = e.project.comp(cid).ok_or("no composition")?;
    if let Some(c) = comp.essential.iter().find(|c| c.kind == "media" && c.layer == lid) {
        return Ok(json!({"controller": c.id, "existing": true}));
    }
    let name = ps(p, "name").map(str::to_string).unwrap_or_else(|| l.name.clone());
    let id = next_id(&comp.essential);
    start_template(e, cid);
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    c.essential.push(Controller { id: id.clone(), name: name.clone(), kind: "media".into(), layer: lid, group: ps(p, "group").unwrap_or("").to_string(), ..Default::default() });
    e.set_label(format!("Add {name} as Media Replacement"));
    Ok(json!({"controller": id, "name": name}))
}

pub fn add_mirror(e: &mut Engine, p: &Json) -> Res {
    let cid = comp_param(e, p)?;
    let i = ctrl_index(e, cid, p)?;
    let c = e.project.comp(cid).ok_or("no composition")?;
    let src = c.essential[i].clone();
    if src.kind != "property" {
        return Err("only property controllers can be mirrored".into());
    }
    let id = next_id(&c.essential);
    let name = ps(p, "name").map(str::to_string).unwrap_or_else(|| src.name.clone());
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    c.essential.insert(i + 1, Controller { id: id.clone(), name, kind: "mirror".into(), layer: src.layer, prop: src.prop, group: src.group.clone(), text: src.id.clone() });
    e.set_label("Add Mirror");
    Ok(json!({"controller": id, "mirrorOf": src.id}))
}

fn addable(e: &Engine, cid: u64, lid: u64, uid: u64) -> Result<(), String> {
    let l = e.layer_ref(cid, lid)?;
    let pr = l.props.find(uid).ok_or("only properties can be added, not groups")?;
    if matches!(pr.ui, Ui::Hidden | Ui::Mask | Ui::Path | Ui::Layer | Ui::Gradient) {
        return Err(format!("{} is not a type Essential Graphics supports", pr.name));
    }
    if l.props.path_of(uid).is_some_and(|x| x.starts_with(GROUP)) {
        return Err("Essential Properties of a precomp cannot be added again".into());
    }
    let comp = e.project.comp(cid).ok_or("no composition")?;
    if comp.essential.iter().any(|c| c.kind == "property" && c.layer == lid && c.prop == uid) {
        return Err(format!("{} is already in Essential Graphics", pr.name));
    }
    Ok(())
}

pub fn can_add(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = super::prop::locate(e, p)?;
    Ok(match addable(e, cid, lid, uid) {
        Ok(()) => json!({"can": true}),
        Err(reason) => json!({"can": false, "reason": reason}),
    })
}

fn instance_layer(e: &Engine, p: &Json) -> Result<(u64, u64, u64), String> {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    let src = l.source.filter(|_| l.kind == LayerKind::Precomp).ok_or("pass a precomp layer (an instance of a template composition)")?;
    Ok((cid, lid, src))
}

pub fn instance(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, src) = instance_layer(e, p)?;
    let l = e.layer_ref(cid, lid)?;
    let src_comp = e.project.comp(src).ok_or("no source composition")?;
    let rows: Vec<Json> = l
        .props
        .sub(GROUP)
        .map(|g| {
            g.children
                .iter()
                .filter_map(|c| if let Node::Prop(pr) = c { Some(pr) } else { None })
                .map(|pr| {
                    let kind = src_comp.essential.iter().find(|c| c.id == pr.id).map(|c| c.kind.clone()).unwrap_or_default();
                    json!({"controller": pr.id, "name": pr.name, "kind": kind, "uid": pr.uid, "value": pr.value.to_json(), "master": pr.default.to_json(), "overridden": overridden(pr), "animated": pr.is_animated()})
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({"layer": lid, "source": src, "templateName": src_comp.template_name, "properties": rows}))
}

fn instance_prop(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<u64, String> {
    let key = p.get("controller").ok_or("pass `controller` (its id or name)")?;
    let l = e.layer_ref(cid, lid)?;
    let g = l.props.sub(GROUP).ok_or("this layer has no Essential Properties")?;
    g.children
        .iter()
        .filter_map(|c| if let Node::Prop(pr) = c { Some(pr) } else { None })
        .find(|pr| key.as_str().is_some_and(|k| pr.id == k || pr.name == k) || key.as_u64().is_some_and(|n| pr.uid == n))
        .map(|pr| pr.uid)
        .ok_or_else(|| "no such Essential Property on this layer".into())
}

pub fn set(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, _) = instance_layer(e, p)?;
    let uid = instance_prop(e, cid, lid, p)?;
    let v = p.get("value").ok_or("pass `value`")?;
    let item = v.as_str().and_then(|s| e.project.items.iter().find(|i| i.name == s && matches!(i.kind, ItemKind::Footage(_) | ItemKind::Comp(_)))).map(|i| i.id);
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("no such property")?;
    let value = match (&pr.value, v) {
        (Value::Num(_), Json::String(s)) if pr.ui == Ui::Plain => Value::Num(item.ok_or_else(|| format!("no footage or composition named {s}"))? as f64),
        (Value::Text(d), Json::String(s)) => {
            let mut d = d.clone();
            d.text = s.clone();
            Value::Text(d)
        }
        _ => pr.value.parse_like(v, pr.ui.options())?,
    };
    pr.value = value.clone();
    let name = pr.name.clone();
    e.set_label(format!("Set {name}"));
    Ok(json!({"layer": lid, "controller": name, "value": value.to_json()}))
}

pub fn push_to_comp(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, src) = instance_layer(e, p)?;
    let only = ps(p, "controller").map(str::to_string);
    let t = e.time_of(cid);
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let l = comp.layer(lid).ok_or("no layer")?;
    let ev = crate::eval::Eval::new(&e.project, cid, comp, t);
    let src_comp = e.project.comp(src).ok_or("no source composition")?;
    let mut pushes: Vec<(String, u64, Option<u64>, Value)> = vec![];
    if let Some(g) = l.props.sub(GROUP) {
        for c in &g.children {
            let Node::Prop(pr) = c else { continue };
            if !overridden(pr) || only.as_ref().is_some_and(|o| o != &pr.id && o != &pr.name) {
                continue;
            }
            let Some(ctrl) = src_comp.essential.iter().find(|c| c.id == pr.id) else { continue };
            let v = ev.value(l, pr);
            pushes.push((pr.id.clone(), ctrl.layer, (ctrl.kind == "property").then_some(ctrl.prop), v));
        }
    }
    if pushes.is_empty() {
        return Err("this instance has no overridden Essential Properties to push".into());
    }
    let mut skipped = vec![];
    let mut done = vec![];
    for (id, slid, prop, v) in &pushes {
        let sl = e.layer_mut(src, *slid)?;
        match prop {
            Some(u) => {
                let Some(pr) = sl.props.find_mut(*u) else { continue };
                if pr.is_animated() {
                    skipped.push(id.clone());
                    continue;
                }
                pr.value = v.clone();
            }
            None => sl.source = Some(v.num().round() as u64),
        }
        done.push(id.clone());
    }
    let l = e.layer_mut(cid, lid)?;
    if let Some(g) = l.props.sub_mut(GROUP) {
        for c in g.children.iter_mut() {
            let Node::Prop(pr) = c else { continue };
            if let Some((_, _, _, v)) = pushes.iter().find(|x| x.0 == pr.id && done.contains(&x.0)) {
                pr.value = v.clone();
                pr.default = v.clone();
                pr.keys.clear();
                pr.expr = None;
            }
        }
    }
    e.set_label("Push Override Values to Source");
    Ok(json!({"pushed": done, "skipped": skipped}))
}

pub fn link_property(e: &mut Engine, p: &Json) -> Res {
    let cid = comp_param(e, p)?;
    let i = ctrl_index(e, cid, p)?;
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let ctrl = comp.essential[i].clone();
    if !matches!(ctrl.kind.as_str(), "property" | "mirror") {
        return Err("link to a property controller".into());
    }
    let mut q = p.clone();
    if let Some(o) = q.as_object_mut() {
        o.remove("controller");
        o.insert("comp".into(), json!(cid));
    }
    let (_, lid, uid) = super::prop::locate(e, &q)?;
    if lid == ctrl.layer && uid == ctrl.prop {
        return Err("a property cannot be linked to its own controller".into());
    }
    let src_layer = comp.layer(ctrl.layer).ok_or("the controller's layer is gone")?;
    let src_kind = src_layer.props.find(ctrl.prop).map(|x| x.value.kind_name()).ok_or("the controller's property is gone")?;
    let text = super::expression::reference(src_layer, ctrl.prop, lid == ctrl.layer).ok_or("cannot reference that property")?;
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("that is a group, not a property")?;
    if pr.value.kind_name() != src_kind {
        return Err(format!("{} holds a {} and the controller a {}", pr.name, pr.value.kind_name(), src_kind));
    }
    pr.expr = Some(crate::model::prop::Expr { text: text.clone(), enabled: true });
    e.set_label("Link Property to Control");
    Ok(json!({"controller": ctrl.id, "layer": lid, "prop": uid, "expression": text}))
}

pub fn unlink_property(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = super::prop::locate(e, p)?;
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let refs: Vec<String> = comp.essential.iter().filter(|c| c.kind == "property").filter_map(|c| super::expression::reference(comp.layer(c.layer)?, c.prop, c.layer == lid)).collect();
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("that is a group, not a property")?;
    match &pr.expr {
        Some(x) if refs.iter().any(|r| r == x.text.trim()) => pr.expr = None,
        _ => return Err(format!("{} is not linked to an Essential Graphics control", pr.name)),
    }
    e.set_label("Unlink Property");
    Ok(json!({"layer": lid, "prop": uid}))
}

pub fn set_primary(e: &mut Engine, p: &Json) -> Res {
    let cid = comp_param(e, p)?;
    start_template(e, cid);
    e.state.essential_comp = Some(cid);
    Ok(json!({"comp": cid}))
}

pub fn solo_supported(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let ids: Vec<u64> = if ids.is_empty() { comp.layers.iter().map(|l| l.id).collect() } else { ids };
    let mut props = vec![];
    for lid in &ids {
        let Some(l) = comp.layer(*lid) else { continue };
        let mut uids = vec![];
        l.props.for_each_prop(&mut |pr| uids.push(pr.uid));
        for u in uids {
            if addable(e, cid, *lid, u).is_ok() {
                props.push(json!({"layer": lid, "prop": u}));
            }
        }
    }
    let n = props.len();
    e.frontend("timeline.revealProps", json!({"props": props, "kind": "supported", "layers": ids}));
    Ok(json!({"props": n}))
}
