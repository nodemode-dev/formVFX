use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use serde_json::{Value as Json, json};

use super::{Engine, Res, pb, ps, pu, rq};
use crate::model::RenderItem;

pub const FORMATS: [(&str, &str, &str); 12] = [
    ("h264", "mp4", "H.264 (MP4)"),
    ("hevc", "mp4", "HEVC (MP4)"),
    ("av1", "mp4", "AV1 (MP4)"),
    ("prores", "mov", "Apple ProRes (MOV): Proxy, LT, 422, HQ, 4444, 4444 XQ"),
    ("webm", "webm", "WebM (VP9 or AV1)"),
    ("gif", "gif", "Animated GIF"),
    ("png", "png", "PNG Sequence"),
    ("jpeg", "jpg", "JPEG Sequence"),
    ("tiff", "tif", "TIFF Sequence"),
    ("exr", "exr", "OpenEXR Sequence"),
    ("wav", "wav", "WAV Audio"),
    ("aiff", "aif", "AIFF Audio"),
];

const RESULTS_KEPT: usize = 64;

#[derive(Default)]
pub struct Progress {
    pub item: u64,
    pub done: usize,
    pub total: usize,
    pub finished: Vec<(u64, Result<String, String>)>,
    pub running: bool,
}

#[derive(Default)]
pub struct Queue {
    pub progress: Arc<Mutex<Progress>>,
    pub results: Vec<Json>,
    pub stop: Arc<AtomicBool>,
    pub thread: Option<std::thread::JoinHandle<()>>,
}

impl Queue {
    pub fn busy(&self) -> bool {
        self.progress.lock().running
    }
}

pub fn ext_of(fmt: &str) -> &'static str {
    FORMATS.iter().find(|f| f.0 == fmt).map(|f| f.1).unwrap_or("mp4")
}

pub fn renders_dir() -> PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into());
    let d = std::env::var("FORMVFX_RENDERS").map(PathBuf::from).unwrap_or_else(|_| Path::new(&home).join("Formvfx").join("Renders"));
    let _ = std::fs::create_dir_all(&d);
    d
}

fn default_output(name: &str, fmt: &str) -> String {
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect();
    let ext = ext_of(fmt);
    let seq = rq::is_seq(fmt);
    let file = if seq { format!("{safe}_[#####].{ext}") } else { format!("{safe}.{ext}") };
    renders_dir().join(file).to_string_lossy().to_string()
}

pub fn add(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let c = e.project.comp(cid).ok_or("no comp")?.clone();
    let name = e.project.item(cid).map(|i| i.name.clone()).unwrap_or_default();
    let slot = ps(p, "slot").unwrap_or("movie");
    let mut settings = rq::default_settings(slot);
    rq::apply_settings(&mut settings, p)?;
    let mut module = match ps(p, "outputModule") {
        Some(t) => rq::module_template(t).ok_or_else(|| format!("no Output Module template `{t}`"))?,
        None => rq::default_module(slot),
    };
    let mut mp = p.clone();
    if let Some(o) = mp.as_object_mut() {
        o.remove("template");
        o.remove("outputModule");
        if o.get("quality").and_then(Json::as_str).is_some() {
            o.remove("quality");
        }
    }
    rq::apply_module(&mut module, &mp)?;
    if module.output.is_empty() {
        module.output = default_output(&name, &module.format);
    }
    let pj = e.proj();
    let id = pj.new_id();
    let mut it = RenderItem { id, comp: cid, settings, modules: vec![module], ..RenderItem::default() };
    if let Some(l) = ps(p, "log") {
        it.log = l.to_string();
    }
    rq::span_from(&mut it, p, c.work_area, c.duration);
    it.end = it.end.max(it.start + c.frame_duration());
    pj.render_queue.push(it);
    Ok(json!({"item": id, "index": pj.render_queue.len()}))
}

pub fn set_color(e: &mut Engine, p: &Json) -> Res {
    if p.get("outputSpace").is_none() && p.get("depth").is_none() {
        return Err("the output module needs `outputSpace` or `depth`".into());
    }
    let pipe = crate::render::color::Pipe::of(&e.project.settings);
    if let Some(s) = ps(p, "outputSpace") {
        pipe.output(s)?;
    }
    let mut q = json!({});
    for k in ["outputSpace", "depth"] {
        if let Some(v) = p.get(k) {
            q[k] = v.clone();
        }
    }
    let k = pu(p, "module").unwrap_or(1).max(1) as usize - 1;
    let r = item_mut(e, p)?;
    let m = r.modules.get_mut(k).ok_or("no such output module")?;
    rq::apply_module(m, &q)?;
    Ok(json!({"outputSpace": m.output_space, "depth": m.depth}))
}

pub fn list(e: &mut Engine, _: &Json) -> Res {
    let pr = e.render.progress.lock();
    let rows: Vec<Json> = e
        .project
        .render_queue
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut row = rq::item_json(&e.project, r, i);
            if pr.running && pr.item == r.id {
                row["status"] = json!("Rendering");
                row["progress"] = json!(if pr.total > 0 { pr.done as f64 / pr.total as f64 } else { 0.0 });
            }
            row
        })
        .collect();
    Ok(json!({"items": rows, "rendering": pr.running, "notify": rq::notify_on(), "finished": e.render.results}))
}

fn item_mut<'a>(e: &'a mut Engine, p: &Json) -> Result<&'a mut RenderItem, String> {
    let i = rq::item_index(e, p)?;
    let r = &mut e.proj().render_queue[i];
    r.fix();
    Ok(r)
}

pub fn remove(e: &mut Engine, p: &Json) -> Res {
    let i = rq::item_index(e, p)?;
    let pj = e.proj();
    if pj.render_queue[i].status == "Rendering" {
        return Err("that item is rendering; stop the queue first".into());
    }
    let it = pj.render_queue.remove(i);
    Ok(json!({"removed": it.id}))
}

pub fn set_output(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path`")?.to_string();
    let k = pu(p, "module").unwrap_or(1).max(1) as usize - 1;
    let r = item_mut(e, p)?;
    let m = r.modules.get_mut(k).ok_or("no such output module")?;
    m.output = path;
    r.status = "Queued".into();
    Ok(json!({"ok": true}))
}

pub fn set_render(e: &mut Engine, p: &Json) -> Res {
    let r = item_mut(e, p)?;
    r.render = pb(p, "render").unwrap_or(!r.render);
    if r.render && r.status == "Done" {
        r.status = "Queued".into();
    }
    Ok(json!({"ok": true, "render": r.render}))
}

pub fn set_format(e: &mut Engine, p: &Json) -> Res {
    let f = ps(p, "format").ok_or("pass `format`")?.to_ascii_lowercase();
    let k = pu(p, "module").unwrap_or(1).max(1) as usize - 1;
    let r = item_mut(e, p)?;
    let m = r.modules.get_mut(k).ok_or("no such output module")?;
    rq::apply_module(m, &json!({"format": f}))?;
    Ok(json!({"ok": true, "output": m.output}))
}

pub fn formats(_: &mut Engine, _: &Json) -> Res {
    Ok(json!(FORMATS.iter().map(|f| json!({"id": f.0, "extension": f.1, "name": f.2, "sequence": rq::is_seq(f.0), "audioOnly": rq::is_audio(f.0), "needsFfmpeg": !rq::is_seq(f.0) && !rq::is_audio(f.0)})).collect::<Vec<_>>()))
}

pub fn stop(e: &mut Engine, _: &Json) -> Res {
    e.render.stop.store(true, Ordering::SeqCst);
    Ok(json!({"stopping": true}))
}

pub fn render(e: &mut Engine, p: &Json) -> Res {
    if e.render.busy() {
        return Err("the Render Queue is already rendering".into());
    }
    let mut items: Vec<RenderItem> = e.project.render_queue.iter().filter(|r| r.render && r.status != "Done").cloned().collect();
    if items.is_empty() {
        return Err("nothing is queued (renderQueue.add)".into());
    }
    resolve_outputs(e.path.as_deref(), &mut items)?;
    let project = e.project.clone();
    let media = e.media.clone();
    let progress = e.render.progress.clone();
    let stop = e.render.stop.clone();
    stop.store(false, Ordering::SeqCst);
    {
        let mut pr = progress.lock();
        pr.running = true;
        pr.finished.clear();
    }
    let wait = pb(p, "wait").unwrap_or(true);
    let batch = if e.multi_frame { rayon::current_num_threads().clamp(1, 16) } else { 1 };
    let ids: Vec<u64> = items.iter().map(|r| r.id).collect();
    e.render.results.retain(|r| r["item"].as_u64().is_none_or(|i| !ids.contains(&i)));
    let handle = std::thread::spawn(move || {
        for it in items {
            {
                let mut pr = progress.lock();
                pr.item = it.id;
                pr.done = 0;
                pr.total = 0;
            }
            let r = rq::render_item(&project, &media, &it, &progress, &stop, batch);
            progress.lock().finished.push((it.id, r));
            if stop.load(Ordering::SeqCst) {
                break;
            }
        }
        progress.lock().running = false;
    });
    if wait {
        let _ = handle.join();
        let out = poll(e);
        return Ok(json!({"rendered": ids, "results": out}));
    }
    e.render.thread = Some(handle);
    Ok(json!({"started": ids}))
}

pub fn resolve_outputs(project: Option<&str>, items: &mut [RenderItem]) -> Result<(), String> {
    let base = project.and_then(|p| Path::new(p).parent()).filter(|d| d.is_absolute());
    for it in items.iter_mut() {
        for m in it.modules.iter_mut() {
            let out = Path::new(&m.output);
            if m.output.is_empty() || out.is_absolute() || out.parent().is_none_or(|d| d.as_os_str().is_empty()) {
                continue;
            }
            let Some(dir) = base else {
                return Err(format!("the output `{}` is a relative path and the project is not saved, so it has no folder to resolve from: pass an absolute output path, or save the project first (relative paths resolve from the project's folder)", m.output));
            };
            m.output = dir.join(out).to_string_lossy().to_string();
        }
    }
    Ok(())
}

pub fn poll(e: &mut Engine) -> Vec<Json> {
    let finished = std::mem::take(&mut e.render.progress.lock().finished);
    if finished.is_empty() {
        return vec![];
    }
    let mut out = vec![];
    for (id, r) in finished {
        let msg;
        let mut actions = vec![];
        let first = e.project.render_queue.iter().find(|x| x.id == id).map_or(0, |it| rq::first_frame(it, e.project.comp(it.comp).map_or(0.0, |c| c.frame_rate)));
        if let Some(it) = Arc::make_mut(&mut e.project).render_queue.iter_mut().find(|x| x.id == id) {
            it.fix();
            match &r {
                Ok(paths) => {
                    it.status = "Done".into();
                    it.progress = 1.0;
                    it.error = None;
                    msg = format!("Rendered {}", paths.replace('\n', ", "));
                    for (m, path) in it.modules.iter().zip(paths.lines()) {
                        if m.post_render != "none" {
                            actions.push((it.comp, m.post_render.clone(), path.to_string(), m.format.clone(), first));
                        }
                    }
                }
                Err(err) => {
                    it.status = "Failed".into();
                    it.error = Some(err.clone());
                    msg = format!("Render failed: {err}");
                }
            }
        } else {
            msg = String::new();
        }
        let mut imported = vec![];
        for (comp, action, path, fmt, first) in actions {
            match post_render(e, comp, &action, &path, &fmt, first) {
                Ok(v) => imported.push(v),
                Err(err) => e.toast(&format!("Post-render action: {err}"), true),
            }
        }
        out.push(json!({"item": id, "ok": r.is_ok(), "output": r.as_ref().ok().map(|s| s.lines().next().unwrap_or("").to_string()), "outputs": r.as_ref().ok().map(|s| s.lines().collect::<Vec<_>>()), "error": r.as_ref().err(), "postRender": imported}));
        if rq::notify_on() || r.is_err() {
            e.frontend("renderQueue.notify", json!({"message": msg, "error": r.is_err()}));
        }
    }
    e.render.results.extend(out.iter().cloned());
    let extra = e.render.results.len().saturating_sub(RESULTS_KEPT);
    e.render.results.drain(..extra);
    out
}

fn post_render(e: &mut Engine, comp: u64, action: &str, path: &str, fmt: &str, n0: usize) -> Result<Json, String> {
    let first = if rq::is_seq(fmt) { rq::seq_path(path, n0) } else { path.to_string() };
    if action == "setProxy" {
        return super::cmd::formats::set_proxy(e, &json!({"items": [comp], "path": first}));
    }
    let r = super::cmd::file::import(e, &json!({"paths": [first]}))?;
    let item = r["items"].as_array().and_then(|a| a.first()).and_then(Json::as_u64).ok_or("the render could not be imported")?;
    if action == "importAndReplace" {
        let pj = e.proj();
        for it in pj.items.iter_mut() {
            if let crate::model::ItemKind::Comp(c) = &mut it.kind {
                for l in c.layers.iter_mut() {
                    if l.source == Some(comp) && l.kind == crate::model::LayerKind::Precomp {
                        l.source = Some(item);
                        l.kind = crate::model::LayerKind::Footage;
                    }
                }
            }
        }
    }
    Ok(json!({"action": action, "item": item}))
}

pub fn color_args(space: &str) -> Vec<&'static str> {
    match crate::render::color::find(space).map(|s| s.id) {
        Some("rec2100pq") => vec!["-color_primaries", "bt2020", "-color_trc", "smpte2084", "-colorspace", "bt2020nc"],
        Some("rec2100hlg") => vec!["-color_primaries", "bt2020", "-color_trc", "arib-std-b67", "-colorspace", "bt2020nc"],
        Some("rec2020") => vec!["-color_primaries", "bt2020", "-color_trc", "bt2020-10", "-colorspace", "bt2020nc"],
        Some("rec709") => vec!["-color_primaries", "bt709", "-color_trc", "bt709", "-colorspace", "bt709"],
        Some("srgb") => vec!["-color_primaries", "bt709", "-color_trc", "iec61966-2-1", "-colorspace", "bt709"],
        _ => vec![],
    }
}
