use std::collections::HashSet;
use std::path::Path;

use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pu, ps};
use crate::model::key::insert;
use crate::model::prop::{Group, GroupKind, Node};
use crate::model::{FORMAT, Item, ItemKind, Key, Project};

pub fn used_by(p: &Project, roots: &[u64]) -> HashSet<u64> {
    let mut keep: HashSet<u64> = HashSet::new();
    let mut stack: Vec<u64> = roots.to_vec();
    while let Some(id) = stack.pop() {
        if !keep.insert(id) {
            continue;
        }
        let Some(it) = p.item(id) else { continue };
        if let Some(par) = it.parent {
            stack.push(par);
        }
        if let ItemKind::Comp(c) = &it.kind {
            for l in &c.layers {
                if let Some(s) = l.source {
                    stack.push(s);
                }
            }
        }
    }
    keep
}

fn drop_items(e: &mut Engine, gone: &HashSet<u64>) {
    let pj = e.proj();
    pj.items.retain(|it| !gone.contains(&it.id));
    for it in pj.items.iter_mut() {
        if let ItemKind::Comp(c) = &mut it.kind {
            c.layers.retain(|l| !l.source.is_some_and(|s| gone.contains(&s)));
        }
    }
    pj.render_queue.retain(|r| !gone.contains(&r.comp));
    e.state.project_selection.retain(|i| !gone.contains(i));
    e.state.open_comps.retain(|c| !gone.contains(c));
    if e.state.active_comp.is_some_and(|c| gone.contains(&c)) {
        e.state.active_comp = e.state.open_comps.last().copied();
    }
}

pub fn remove_unused(e: &mut Engine, _: &Json) -> Res {
    let p = &e.project;
    let gone: HashSet<u64> = p.items.iter().filter(|it| matches!(it.kind, ItemKind::Footage(_) | ItemKind::Solid(_)) && p.uses_item(it.id) == 0).map(|it| it.id).collect();
    let n = gone.len();
    drop_items(e, &gone);
    e.set_label("Remove Unused Footage");
    Ok(json!({"removed": n}))
}

pub fn consolidate(e: &mut Engine, _: &Json) -> Res {
    let mut seen: Vec<(String, u64)> = vec![];
    let mut remap: Vec<(u64, u64)> = vec![];
    for it in &e.project.items {
        let ItemKind::Footage(f) = &it.kind else { continue };
        if f.path.is_empty() {
            continue;
        }
        let key = format!("{}|{}|{:?}|{}|{}", f.path, f.frame_rate, f.alpha, f.loops, f.pixel_aspect);
        match seen.iter().find(|(k, _)| *k == key) {
            Some((_, keep)) => remap.push((it.id, *keep)),
            None => seen.push((key, it.id)),
        }
    }
    let n = remap.len();
    if n == 0 {
        return Ok(json!({"consolidated": 0}));
    }
    let pj = e.proj();
    for it in pj.items.iter_mut() {
        if let ItemKind::Comp(c) = &mut it.kind {
            for l in c.layers.iter_mut() {
                if let Some((_, to)) = remap.iter().find(|(from, _)| Some(*from) == l.source) {
                    l.source = Some(*to);
                }
            }
        }
    }
    let gone: HashSet<u64> = remap.iter().map(|x| x.0).collect();
    drop_items(e, &gone);
    e.set_label("Consolidate All Footage");
    Ok(json!({"consolidated": n}))
}

pub fn reduce(e: &mut Engine, _: &Json) -> Res {
    let roots: Vec<u64> = {
        let sel: Vec<u64> = e.state.project_selection.iter().copied().filter(|i| e.project.comp(*i).is_some()).collect();
        if sel.is_empty() { e.state.active_comp.into_iter().collect() } else { sel }
    };
    if roots.is_empty() {
        return Err("select the compositions to keep".into());
    }
    let keep = used_by(&e.project, &roots);
    let gone: HashSet<u64> = e.project.items.iter().map(|i| i.id).filter(|i| !keep.contains(i)).collect();
    let n = gone.len();
    drop_items(e, &gone);
    e.set_label("Reduce Project");
    Ok(json!({"removed": n, "kept": keep.len()}))
}

pub fn find_missing(e: &mut Engine, p: &Json) -> Res {
    let what = ps(p, "what").unwrap_or("footage");
    let pj = &e.project;
    let out: Vec<Json> = match what {
        "effects" => {
            let mut v = vec![];
            for it in &pj.items {
                let ItemKind::Comp(c) = &it.kind else { continue };
                for l in &c.layers {
                    for g in l.effects().map(|g| g.groups().collect::<Vec<_>>()).unwrap_or_default() {
                        if let GroupKind::Effect { effect } = &g.kind
                            && crate::effects::find(effect).is_none()
                        {
                            v.push(json!({"comp": it.id, "layer": l.id, "layerName": l.name, "effect": effect, "name": g.name}));
                        }
                    }
                }
            }
            v
        }
        "fonts" => {
            let fams: HashSet<String> = crate::render::text::families().into_iter().map(|(f, _)| f.to_ascii_lowercase()).collect();
            let mut v = vec![];
            for it in &pj.items {
                let ItemKind::Comp(c) = &it.kind else { continue };
                for l in &c.layers {
                    let Some(td) = l.text_prop().and_then(|p| p.value.as_text()) else { continue };
                    if !fams.contains(&td.font.to_ascii_lowercase()) {
                        v.push(json!({"comp": it.id, "layer": l.id, "layerName": l.name, "font": td.font}));
                    }
                }
            }
            v
        }
        _ => pj
            .items
            .iter()
            .filter_map(|it| match &it.kind {
                ItemKind::Footage(f) if !f.path.is_empty() && !Path::new(&f.path).exists() => Some(json!({"item": it.id, "name": it.name, "path": f.path})),
                _ => None,
            })
            .collect(),
    };
    let ids: Vec<u64> = out.iter().filter_map(|x| x.get("item").and_then(Json::as_u64)).collect();
    if !ids.is_empty() {
        e.state.project_selection = ids;
    }
    Ok(json!({"what": what, "missing": out}))
}

fn safe_name(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' }).collect()
}

pub fn collect_files(e: &mut Engine, p: &Json) -> Res {
    let folder = ps(p, "folder").ok_or("pass `folder`")?;
    let stem = e.path.as_deref().and_then(|x| Path::new(x).file_stem()).and_then(|x| x.to_str()).unwrap_or("Untitled Project").to_string();
    let root = Path::new(folder).join(format!("{} folder", safe_name(&stem)));
    let foot = root.join("Footage");
    std::fs::create_dir_all(&foot).map_err(|err| format!("{}: {err}", foot.display()))?;
    let mut proj = (*e.project).clone();
    let mut copied = 0;
    let mut missing = vec![];
    let mut taken: HashSet<String> = HashSet::new();
    for it in proj.items.iter_mut() {
        let ItemKind::Footage(f) = &mut it.kind else { continue };
        let mut paths: Vec<String> = if f.sequence.is_empty() { vec![f.path.clone()] } else { f.sequence.clone() };
        paths.retain(|x| !x.is_empty());
        let mut new_paths = vec![];
        for src in paths {
            let sp = Path::new(&src);
            if !sp.exists() {
                missing.push(src.clone());
                new_paths.push(src);
                continue;
            }
            let base = sp.file_name().and_then(|x| x.to_str()).unwrap_or("footage").to_string();
            let mut name = base.clone();
            let mut n = 2;
            while taken.contains(&name) {
                name = format!("{n}_{base}");
                n += 1;
            }
            taken.insert(name.clone());
            let dst = foot.join(&name);
            std::fs::copy(sp, &dst).map_err(|err| format!("{}: {err}", dst.display()))?;
            copied += 1;
            new_paths.push(dst.to_string_lossy().to_string());
        }
        if f.sequence.is_empty() {
            if let Some(x) = new_paths.into_iter().next() {
                f.path = x;
            }
        } else {
            f.path = new_paths.first().cloned().unwrap_or_default();
            f.sequence = new_paths;
        }
    }
    proj.format = FORMAT.into();
    proj.saved_by = format!("formvfx {}", env!("CARGO_PKG_VERSION"));
    let pp = root.join(format!("{}.{}", safe_name(&stem), crate::model::EXTENSION));
    std::fs::write(&pp, serde_json::to_string_pretty(&proj).map_err(|x| x.to_string())?).map_err(|err| format!("{}: {err}", pp.display()))?;
    let report = format!(
        "Collect Files report\nProject: {}\nItems: {}\nFiles copied: {copied}\nMissing files: {}\n{}",
        pp.display(),
        proj.items.len(),
        missing.len(),
        missing.iter().map(|m| format!("  {m}\n")).collect::<String>()
    );
    let rp = root.join(format!("{} Report.txt", safe_name(&stem)));
    std::fs::write(&rp, report).map_err(|err| format!("{}: {err}", rp.display()))?;
    Ok(json!({"folder": root.to_string_lossy(), "project": pp.to_string_lossy(), "copied": copied, "missing": missing, "report": rp.to_string_lossy()}))
}

pub fn metadata(e: &mut Engine, p: &Json) -> Res {
    let id = match p.get("item") {
        Some(Json::Number(n)) => n.as_u64().unwrap_or(0),
        Some(Json::String(s)) => e.project.items.iter().find(|i| i.name == *s).map(|i| i.id).ok_or_else(|| format!("no item \"{s}\""))?,
        _ => e.state.project_selection.first().copied().ok_or("select a project item or pass `item`")?,
    };
    let it: &Item = e.project.item(id).ok_or("no such item")?;
    let mut o = json!({"item": id, "name": it.name, "type": it.type_name(), "comment": it.comment, "usedIn": e.project.uses_item(id)});
    match &it.kind {
        ItemKind::Footage(f) => {
            let size = std::fs::metadata(&f.path).ok();
            o["file"] = json!({
                "path": f.path, "kind": format!("{:?}", f.kind), "width": f.width, "height": f.height, "duration": f.duration, "frameRate": f.frame_rate,
                "codec": f.codec, "pixelAspect": f.pixel_aspect, "alpha": format!("{:?}", f.alpha), "hasVideo": f.has_video, "hasAudio": f.has_audio,
                "audioRate": f.audio_rate, "audioChannels": f.audio_channels, "frames": f.sequence.len(), "missing": f.missing,
                "bytes": size.as_ref().map(|m| m.len()),
                "modified": size.and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()),
            });
        }
        ItemKind::Comp(c) => o["comp"] = json!({"width": c.width, "height": c.height, "duration": c.duration, "frameRate": c.frame_rate, "layers": c.layers.len(), "markers": c.markers.len()}),
        ItemKind::Solid(s) => o["solid"] = json!({"width": s.width, "height": s.height, "color": s.color}),
        ItemKind::Folder => o["children"] = json!(e.project.items.iter().filter(|i| i.parent == Some(id)).count()),
    }
    Ok(o)
}

const PRESET_FORMAT: &str = "formvfx.preset/1";

pub fn save_preset(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path`")?.to_string();
    let cid = e.comp_id(None)?;
    let sel = e.state.selected_props.clone();
    let lid = sel.first().map(|x| x.0).or_else(|| e.state.selected_layers.first().copied()).ok_or("select properties or effects to save")?;
    let l = e.layer_ref(cid, lid)?;
    let picks: Vec<u64> = sel.iter().filter(|x| x.0 == lid).map(|x| x.1).collect();
    let mut effects = vec![];
    let mut props = vec![];
    let mut styles = vec![];
    let t0 = picks.iter().filter_map(|u| l.props.find(*u)).flat_map(|pr| pr.keys.iter().map(|k| k.time)).fold(f64::MAX, f64::min);
    let t0 = if t0 == f64::MAX { 0.0 } else { t0 };
    for u in &picks {
        if let Some(g) = l.props.find_group(*u) {
            match &g.kind {
                GroupKind::Effect { .. } => effects.push(serde_json::to_value(g).map_err(|x| x.to_string())?),
                _ if l.props.sub("layerStyles").is_some_and(|s| s.uid == *u || s.groups().any(|x| x.uid == *u)) => styles.push(serde_json::to_value(g).map_err(|x| x.to_string())?),
                _ => g.for_each_prop(&mut |pr| {
                    if let Some(path) = l.props.path_of(pr.uid) {
                        props.push(prop_json(&path, pr, t0));
                    }
                }),
            }
        } else if let Some(pr) = l.props.find(*u)
            && let Some(path) = l.props.path_of(*u)
        {
            props.push(prop_json(&path, pr, t0));
        }
    }
    if picks.is_empty()
        && let Some(fx) = l.effects()
    {
        for g in fx.groups() {
            effects.push(serde_json::to_value(g).map_err(|x| x.to_string())?);
        }
    }
    if effects.is_empty() && props.is_empty() && styles.is_empty() {
        return Err("select properties, effects or layer styles to save".into());
    }
    let name = ps(p, "name").map(str::to_string).unwrap_or_else(|| Path::new(&path).file_stem().and_then(|x| x.to_str()).unwrap_or("Preset").to_string());
    let doc = json!({"format": PRESET_FORMAT, "name": name, "effects": effects, "styles": styles, "props": props});
    std::fs::write(&path, serde_json::to_string_pretty(&doc).map_err(|x| x.to_string())?).map_err(|err| format!("{path}: {err}"))?;
    remember(e, &path);
    Ok(json!({"path": path, "effects": doc["effects"].as_array().map(Vec::len), "props": doc["props"].as_array().map(Vec::len)}))
}

fn prop_json(path: &str, pr: &crate::model::Prop, t0: f64) -> Json {
    let keys: Vec<Json> = pr.keys.iter().map(|k| serde_json::to_value(Key { time: k.time - t0, ..k.clone() }).unwrap_or(Json::Null)).collect();
    json!({"path": path, "value": serde_json::to_value(&pr.value).unwrap_or(Json::Null), "keys": keys, "expr": pr.expr.as_ref().map(|x| json!({"text": x.text, "enabled": x.enabled}))})
}

fn remember(e: &mut Engine, path: &str) {
    e.recent_presets.retain(|x| x != path);
    e.recent_presets.insert(0, path.to_string());
    e.recent_presets.truncate(10);
}

fn apply_doc(e: &mut Engine, doc: &Json, p: &Json) -> Res {
    if doc.get("format").and_then(Json::as_str) != Some(PRESET_FORMAT) {
        return Err("not a formvfx animation preset".into());
    }
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select a layer to apply the preset to".into());
    }
    let now = e.time_of(cid);
    let parse_groups = |k: &str| -> Vec<Group> { doc.get(k).and_then(Json::as_array).map(|a| a.iter().filter_map(|g| serde_json::from_value::<Group>(g.clone()).ok()).collect()).unwrap_or_default() };
    let (effects, styles) = (parse_groups("effects"), parse_groups("styles"));
    let props = doc.get("props").and_then(Json::as_array).cloned().unwrap_or_default();
    let mut applied = 0;
    for lid in &ids {
        for g in &effects {
            let mut g = g.clone();
            let pj = e.proj();
            g.renumber(&mut pj.next_uid);
            let l = e.layer_mut(cid, *lid)?;
            if let Some(fx) = l.props.sub_mut("effects") {
                fx.children.push(Node::Group(g));
                applied += 1;
            }
        }
        for g in &styles {
            if g.id == "layerStyles" {
                let mut g = g.clone();
                let pj = e.proj();
                g.renumber(&mut pj.next_uid);
                let l = e.layer_mut(cid, *lid)?;
                l.props.children.retain(|c| c.id() != "layerStyles");
                let at = l.props.children.iter().position(|c| c.id() == "transform").map(|i| i + 1).unwrap_or(l.props.children.len());
                l.props.children.insert(at, Node::Group(g));
            } else {
                super::styles::add(e, &json!({"style": g.id, "layers": [lid], "comp": cid}))?;
                let mut g = g.clone();
                let pj = e.proj();
                g.renumber(&mut pj.next_uid);
                let l = e.layer_mut(cid, *lid)?;
                if let Some(s) = l.props.sub_mut("layerStyles") {
                    s.children.retain(|c| c.id() != g.id);
                    s.children.push(Node::Group(g));
                }
            }
            applied += 1;
        }
        let l = e.layer_mut(cid, *lid)?;
        let lt = l.layer_time(now);
        for pj in &props {
            let Some(path) = pj.get("path").and_then(Json::as_str) else { continue };
            let Some(uid) = l.props.resolve_uid(path) else { continue };
            let Some(pr) = l.props.find_mut(uid) else { continue };
            if let Some(v) = pj.get("value").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                if std::mem::discriminant(&pr.value) == std::mem::discriminant(&v) {
                    pr.value = v;
                }
            }
            let keys: Vec<Key> = pj.get("keys").and_then(Json::as_array).map(|a| a.iter().filter_map(|k| serde_json::from_value(k.clone()).ok()).collect()).unwrap_or_default();
            if !keys.is_empty() {
                pr.keys.clear();
                for k in keys {
                    if std::mem::discriminant(&pr.value) == std::mem::discriminant(&k.value) {
                        insert(&mut pr.keys, Key { time: k.time + lt, ..k });
                    }
                }
            }
            if let Some(x) = pj.get("expr").filter(|x| !x.is_null()) {
                pr.expr = Some(crate::model::prop::Expr { text: x.get("text").and_then(Json::as_str).unwrap_or("").to_string(), enabled: x.get("enabled").and_then(Json::as_bool).unwrap_or(true) });
            }
            applied += 1;
        }
    }
    let name = doc.get("name").and_then(Json::as_str).unwrap_or("Preset").to_string();
    e.set_label(format!("Apply {name}"));
    Ok(json!({"applied": applied, "layers": ids}))
}

pub fn apply_preset(e: &mut Engine, p: &Json) -> Res {
    if let Some(doc) = p.get("preset").filter(|x| x.is_object()) {
        return apply_doc(e, &doc.clone(), p);
    }
    let path = ps(p, "path").or_else(|| ps(p, "preset")).ok_or("pass `path`")?.to_string();
    let text = std::fs::read_to_string(&path).map_err(|err| format!("{path}: {err}"))?;
    let doc: Json = serde_json::from_str(&text).map_err(|err| format!("{path}: {err}"))?;
    let r = apply_doc(e, &doc, p)?;
    remember(e, &path);
    Ok(r)
}

pub fn apply_recent(e: &mut Engine, p: &Json) -> Res {
    let path = match ps(p, "path") {
        Some(x) => x.to_string(),
        None => e.recent_presets.get(pu(p, "index").unwrap_or(0) as usize).cloned().ok_or("no recent animation presets")?,
    };
    let mut q = p.clone();
    q["path"] = json!(path);
    apply_preset(e, &q)
}

pub fn clear_recent(e: &mut Engine, _: &Json) -> Res {
    e.recent_presets.clear();
    Ok(json!({"ok": true}))
}

pub fn recent(e: &mut Engine, _: &Json) -> Res {
    Ok(json!(e.recent_presets))
}
