use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, export, jobs, pb, pf, ps, pu};
use crate::model::{ItemKind, LayerKind};
use crate::render::diskcache::{self, data_dir};

struct Step {
    title: &'static str,
    text: &'static str,
    target: &'static str,
    accepts: &'static [&'static str],
    show: (&'static str, &'static str),
}

struct Tutorial {
    id: &'static str,
    title: &'static str,
    summary: &'static str,
    minutes: u32,
    steps: &'static [Step],
}

const TUTORIALS: [Tutorial; 3] = [
    Tutorial {
        id: "first-animation",
        title: "Your first animation",
        summary: "Make a composition, add a title and fade it in with two keyframes.",
        minutes: 5,
        steps: &[
            Step { title: "Make a composition", text: "A composition is the canvas you animate in. Choose New Composition (Cmd or Ctrl+N) and keep the 1920 by 1080 preset.", target: "home.new", accepts: &["comp.new"], show: ("comp.new", r#"{"name": "First Animation", "width": 1920, "height": 1080, "duration": 5}"#) },
            Step { title: "Add a title", text: "Layer > New > Text, then type a word. Each line of type is its own layer in the Timeline.", target: "timeline", accepts: &["layer.newText"], show: ("layer.newText", r#"{"text": "Hello"}"#) },
            Step { title: "Key the start", text: "Twirl the text layer's Transform open, set Opacity to 0 at the first frame and click its stopwatch: that is a keyframe.", target: "timeline", accepts: &["prop.addKey", "anim.addKeyframe", "keys.set"], show: ("prop.addKey", r#"{"path": "transform/opacity", "time": 0, "value": 0}"#) },
            Step { title: "Key the end", text: "Move the current time to 1 second and set Opacity to 100. formvfx adds the second keyframe and fades between them.", target: "timeline", accepts: &["prop.addKey", "anim.addKeyframe", "keys.set", "prop.set"], show: ("prop.addKey", r#"{"path": "transform/opacity", "time": 1, "value": 100}"#) },
            Step { title: "Play it", text: "Press Space to preview. The green bar in the Timeline fills as frames are cached.", target: "composition", accepts: &["playback.toggle"], show: ("playback.toggle", "{}") },
        ],
    },
    Tutorial {
        id: "footage-edit",
        title: "Trim and insert footage",
        summary: "Import a clip, mark in and out points in the Footage panel and ripple-insert it into a composition.",
        minutes: 4,
        steps: &[
            Step { title: "Find your footage", text: "Window > Panels > Media Browser lists the folders on this computer; double-click a clip to import it, or use File > Import.", target: "mediaBrowser", accepts: &["file.import", "file.importMultiple", "mediaBrowser.import"], show: ("window.panel", r#"{"panel": "mediaBrowser"}"#) },
            Step { title: "Open it in the Footage panel", text: "Double-click the footage in the Project panel. The Footage panel shows the clip itself, outside any composition.", target: "project", accepts: &["footage.open"], show: ("window.panel", r#"{"panel": "project"}"#) },
            Step { title: "Mark the in point", text: "Scrub to where the shot should start and press Set In.", target: "footage", accepts: &["footage.setIn"], show: ("footage.setIn", "{}") },
            Step { title: "Mark the out point", text: "Scrub to where it should end and press Set Out.", target: "footage", accepts: &["footage.setOut"], show: ("footage.setOut", "{}") },
            Step { title: "Ripple insert", text: "With a composition open, Ripple Insert puts the trimmed shot at the current time and pushes later layers back by its length; Overlay lays it on top.", target: "footage", accepts: &["footage.rippleInsertEdit", "footage.overlayEdit"], show: ("footage.rippleInsertEdit", "{}") },
        ],
    },
    Tutorial {
        id: "render-movie",
        title: "Render a movie",
        summary: "Queue a composition and render it to a movie file.",
        minutes: 3,
        steps: &[
            Step { title: "Add to the Render Queue", text: "Composition > Add to Render Queue (Cmd or Ctrl+M) queues the open composition.", target: "renderQueue", accepts: &["renderQueue.add"], show: ("renderQueue.add", "{}") },
            Step { title: "Look at the queue", text: "The Render Queue panel shows each item's settings and output; click the output name to change format or file.", target: "renderQueue", accepts: &["window.panel"], show: ("window.panel", r#"{"panel": "renderQueue"}"#) },
            Step { title: "Render", text: "Press Render. Progress shows in the panel and in Window > Progress; formvfx keeps working while it renders.", target: "renderQueue", accepts: &["renderQueue.render"], show: ("renderQueue.render", r#"{"wait": false}"#) },
        ],
    },
];

#[derive(Default)]
pub struct Learn {
    pub tutorial: Option<usize>,
    pub step: usize,
    pub done: Vec<bool>,
}

fn log_path() -> PathBuf {
    data_dir().join("logs").join("formvfx.log")
}

fn logging(e: &Engine) -> bool {
    e.prefs.get("logging.enabled").and_then(Json::as_bool).unwrap_or(false)
}

pub fn after(e: &mut Engine, id: &str, r: &Res, secs: f64) {
    if logging(e) && id != "help.enableLogging" {
        let line = format!("{:.3}\t{id}\t{}\t{:.1} ms{}\n", super::session::clock(), if r.is_ok() { "ok" } else { "error" }, secs * 1000.0, r.as_ref().err().map(|x| format!("\t{}", x.replace('\n', " "))).unwrap_or_default());
        let path = log_path();
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = f.write_all(line.as_bytes());
        }
    }
    if r.is_err() || id.starts_with("learn.") {
        return;
    }
    let Some(t) = e.session.learn.tutorial else { return };
    let steps = TUTORIALS[t].steps;
    let i = e.session.learn.step;
    if i < steps.len() && steps[i].accepts.contains(&id) {
        e.session.learn.done[i] = true;
        if i + 1 < steps.len() {
            e.session.learn.step = i + 1;
        }
        let st = state_json(e);
        e.frontend("learn.update", st);
    }
}

pub fn enable_logging(e: &mut Engine, p: &Json) -> Res {
    let on = pb(p, "on").unwrap_or(!logging(e));
    e.prefs.insert("logging.enabled".into(), json!(on));
    diskcache::save_pref("logging.enabled", &json!(on));
    if on {
        let path = log_path();
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|x| format!("{}: {x}", d.display()))?;
        }
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).map_err(|x| format!("{}: {x}", path.display()))?;
        let _ = writeln!(f, "{:.3}\tlogging on\tformvfx {}", super::session::clock(), env!("CARGO_PKG_VERSION"));
    }
    Ok(json!({"on": on, "path": log_path().to_string_lossy()}))
}

pub fn reveal_log_file(_: &mut Engine, _: &Json) -> Res {
    let path = log_path();
    if !path.exists() {
        return Err("there is no log yet (Help > Enable Logging)".into());
    }
    super::file::reveal_path(&path.to_string_lossy())
}

fn dir_size(dir: &Path) -> (u64, u64) {
    let mut bytes = 0;
    let mut files = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for ent in rd.flatten() {
            let Ok(m) = ent.metadata() else { continue };
            if m.is_dir() {
                stack.push(ent.path());
            } else {
                bytes += m.len();
                files += 1;
            }
        }
    }
    (files, bytes)
}

fn user_dir() -> PathBuf {
    match std::env::var("FORMVFX_DATA_DIR").ok().filter(|x| !x.trim().is_empty()) {
        Some(d) => PathBuf::from(d),
        None => std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).map(PathBuf::from).unwrap_or_default().join("Formvfx"),
    }
}

fn ffmpeg_version() -> Option<String> {
    let out = std::process::Command::new(crate::render::media::tool("ffmpeg")).arg("-version").output().ok()?;
    String::from_utf8_lossy(&out.stdout).lines().next().map(str::to_string)
}

pub fn system_info(e: &mut Engine, _: &Json) -> Res {
    let gpu = crate::render::gpu::current().map(|g| format!("{g:?}"));
    Ok(json!({
        "formvfx": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        "renderThreads": rayon::current_num_threads(),
        "multiFrame": e.multi_frame,
        "gpu": gpu,
        "ffmpeg": ffmpeg_version(),
        "engineData": data_dir().to_string_lossy(),
        "userData": user_dir().to_string_lossy(),
        "diskCache": diskcache::stats(),
        "commands": crate::engine::registry::all().len(),
        "effects": crate::effects::every().len(),
        "plugins": crate::effects::plugin::specs().len(),
        "mobileSam": crate::render::roto::installed(),
        "faceLandmarker": crate::render::facemesh::installed(),
        "logging": logging(e),
    }))
}

pub fn system_report(e: &mut Engine, p: &Json) -> Res {
    let info = system_info(e, &Json::Null)?;
    let mut checks = vec![];
    let mut add = |name: &str, ok: bool, note: String| checks.push(json!({"check": name, "ok": ok, "note": note}));
    add("FFmpeg", info["ffmpeg"].is_string(), info["ffmpeg"].as_str().map(str::to_string).unwrap_or_else(|| "not found: movies and audio files cannot be read or written; install FFmpeg".into()));
    add("GPU", info["gpu"].is_string(), info["gpu"].as_str().map(str::to_string).unwrap_or_else(|| "no GPU in use: every composition renders on the CPU".into()));
    let probe = data_dir().join(".write-test");
    let writable = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(&probe);
    add("Data folder", writable, data_dir().to_string_lossy().to_string());
    add("Disk cache", info["diskCache"]["enabled"].as_bool().unwrap_or(true), format!("{} files", info["diskCache"]["files"]));
    add("Fonts", true, format!("{} faces", crate::render::text::db().len()));
    let mut text = format!("formvfx {} system report\n\n", info["formvfx"].as_str().unwrap_or(""));
    for c in &checks {
        text.push_str(&format!("{} {}: {}\n", if c["ok"] == true { "OK  " } else { "WARN" }, c["check"].as_str().unwrap_or(""), c["note"].as_str().unwrap_or("")));
    }
    text.push_str(&format!("\n{}\n", serde_json::to_string_pretty(&info).unwrap_or_default()));
    let path = data_dir().join("System Report.txt");
    let _ = std::fs::create_dir_all(data_dir());
    std::fs::write(&path, &text).map_err(|x| format!("{}: {x}", path.display()))?;
    if !pb(p, "quiet").unwrap_or(false) {
        e.frontend("app.showReport", json!({"title": "System Compatibility Report", "text": text}));
    }
    Ok(json!({"path": path.to_string_lossy(), "checks": checks, "ok": checks.iter().all(|c| c["ok"] == true)}))
}

pub fn docs(e: &mut Engine, p: &Json) -> Res {
    const BASE: &str = "https://github.com/nodemode-ai/formvfx/blob/main/";
    let page = ps(p, "page").unwrap_or("help");
    let file = match page {
        "help" => ".docs/wiki/Home.md",
        "start" | "gettingStarted" => ".docs/wiki/Getting-Started.md",
        "scripting" | "expressions" => ".claude/skills/formvfx-harness/sub-skills/expressions-and-scripting/SUB-SKILL.md",
        "effects" => ".claude/skills/formvfx-harness/references/effects.md",
        "commands" => ".claude/skills/formvfx-harness/references/commands.md",
        "agents" => ".docs/wiki/Agents.md",
        "parity" => "docs/PARITY.md",
        "plugins" => "docs/PLUGINS.md",
        other => return Err(format!("`{other}` is not a help page (help, start, scripting, expressions, effects, commands, agents, parity, plugins)")),
    };
    let url = format!("{BASE}{file}");
    e.emit(json!({"type": "openUrl", "url": url}));
    Ok(json!({"page": page, "url": url}))
}

fn tutorial_json(t: &Tutorial) -> Json {
    json!({"id": t.id, "title": t.title, "summary": t.summary, "minutes": t.minutes, "steps": t.steps.len()})
}

fn state_json(e: &Engine) -> Json {
    let Some(t) = e.session.learn.tutorial else { return Json::Null };
    let tut = &TUTORIALS[t];
    let i = e.session.learn.step.min(tut.steps.len() - 1);
    let s = &tut.steps[i];
    json!({
        "tutorial": tutorial_json(tut),
        "step": i,
        "steps": tut.steps.len(),
        "completed": e.session.learn.done,
        "done": e.session.learn.done.iter().all(|d| *d),
        "current": {"title": s.title, "text": s.text, "target": s.target, "accepts": s.accepts},
    })
}

pub fn learn_list(_: &mut Engine, _: &Json) -> Res {
    Ok(Json::Array(TUTORIALS.iter().map(tutorial_json).collect()))
}

pub fn learn_start(e: &mut Engine, p: &Json) -> Res {
    let id = ps(p, "id").ok_or("pass `id` (learn.list shows them)")?;
    let t = TUTORIALS.iter().position(|t| t.id == id).ok_or_else(|| format!("no tutorial `{id}`"))?;
    e.session.learn = Learn { tutorial: Some(t), step: 0, done: vec![false; TUTORIALS[t].steps.len()] };
    let st = state_json(e);
    e.frontend("learn.update", st.clone());
    Ok(st)
}

pub fn learn_state(e: &mut Engine, _: &Json) -> Res {
    Ok(state_json(e))
}

pub fn learn_step(e: &mut Engine, p: &Json) -> Res {
    let t = e.session.learn.tutorial.ok_or("no tutorial is running (learn.start)")?;
    let n = TUTORIALS[t].steps.len();
    match ps(p, "action").unwrap_or("next") {
        "next" => e.session.learn.step = (e.session.learn.step + 1).min(n - 1),
        "back" => e.session.learn.step = e.session.learn.step.saturating_sub(1),
        "goto" => e.session.learn.step = (pu(p, "index").ok_or("pass `index`")? as usize).min(n - 1),
        "showMe" => {
            let (cmd, params) = TUTORIALS[t].steps[e.session.learn.step].show;
            let params: Json = serde_json::from_str(params).unwrap_or(Json::Null);
            if let Err(err) = e.execute(cmd, params) {
                let st = state_json(e);
                e.frontend("learn.update", st);
                return Err(err);
            }
        }
        other => return Err(format!("`{other}` is not a step action (next, back, showMe, goto)")),
    }
    let st = state_json(e);
    e.frontend("learn.update", st.clone());
    Ok(st)
}

pub fn learn_stop(e: &mut Engine, _: &Json) -> Res {
    let was = e.session.learn.tutorial.map(|t| TUTORIALS[t].id);
    e.session.learn = Learn::default();
    e.frontend("learn.update", Json::Null);
    Ok(json!({"stopped": was}))
}

pub fn in_app_tutorials(e: &mut Engine, _: &Json) -> Res {
    e.frontend("help.inAppTutorials", json!({}));
    learn_list(e, &Json::Null)
}

fn job_rows(e: &Engine) -> Vec<Json> {
    let mut rows = vec![];
    if e.render.busy() {
        let pr = e.render.progress.lock();
        let name = e.project.render_queue.iter().find(|r| r.id == pr.item).and_then(|r| e.project.item(r.comp)).map(|i| i.name.clone());
        rows.push(json!({"id": "render", "kind": "render", "label": format!("Render {}", name.unwrap_or_default()), "done": pr.done, "total": pr.total, "cancellable": true}));
    }
    for (i, j) in e.jobs.active.iter().enumerate() {
        let pr = j.prog.lock();
        rows.push(json!({"id": format!("task:{i}"), "kind": j.kind, "layer": j.layer, "label": j.label, "done": pr.done, "total": pr.total, "task": pr.task, "seconds": pr.started.elapsed().as_secs_f64(), "cancellable": true}));
    }
    if let Some(w) = &e.session.watch {
        rows.push(json!({"id": "watch", "kind": "watchFolder", "label": format!("Watch Folder {}", w.folder.display()), "busy": w.busy(), "cancellable": true}));
    }
    rows
}

pub fn jobs_list(e: &mut Engine, _: &Json) -> Res {
    Ok(Json::Array(job_rows(e)))
}

pub fn jobs_cancel(e: &mut Engine, p: &Json) -> Res {
    let which = ps(p, "job").unwrap_or("all");
    let mut cancelled = vec![];
    let all = which == "all";
    if (all || which == "render") && e.render.busy() {
        export::stop(e, &Json::Null)?;
        cancelled.push("render".to_string());
    }
    if (all || which == "watch" || which == "watchFolder") && e.session.watch.is_some() {
        e.session.watch = None;
        cancelled.push("watch".to_string());
    }
    if let Some(n) = which.strip_prefix("task:") {
        let i: usize = n.parse().map_err(|_| format!("`{which}` is not a job id (jobs.list shows them)"))?;
        let j = e.jobs.active.get(i).ok_or_else(|| format!("no job {which}"))?;
        j.stop.store(true, Ordering::Relaxed);
        cancelled.push(which.to_string());
    } else if !matches!(which, "render" | "watch" | "watchFolder") {
        let kind = match which {
            "all" => None,
            "maskTrack" => Some("mask"),
            k => Some(k),
        };
        for j in &e.jobs.active {
            if kind.is_none_or(|k| j.kind == k) {
                j.stop.store(true, Ordering::Relaxed);
                cancelled.push(format!("{} ({})", j.kind, j.label));
            }
        }
    }
    if cancelled.is_empty() && !all {
        return Err(format!("no running job matches `{which}`"));
    }
    Ok(json!({"cancelled": cancelled}))
}

pub fn jobs_wait(e: &mut Engine, p: &Json) -> Res {
    let limit = Duration::from_secs_f64(pf(p, "timeout").unwrap_or(600.0).clamp(0.0, 86400.0));
    let started = Instant::now();
    loop {
        if !export::poll(e).is_empty() {
            e.revision += 1;
        }
        if jobs::poll(e) {
            e.revision += 1;
        }
        let busy = e.render.busy() || e.jobs.running() || e.session.watch.as_ref().is_some_and(|w| w.busy());
        if !busy {
            return Ok(json!({"waited": started.elapsed().as_secs_f64(), "running": []}));
        }
        if started.elapsed() >= limit {
            return Ok(json!({"waited": started.elapsed().as_secs_f64(), "timedOut": true, "running": job_rows(e)}));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn prefs_open(e: &mut Engine, p: &Json) -> Res {
    let page = ps(p, "page").unwrap_or("general");
    e.frontend("app.settings", json!({"page": page}));
    Ok(json!({"page": page}))
}

pub fn prefs_reset(e: &mut Engine, p: &Json) -> Res {
    let pages = super::edit::prefs_pages(e, &Json::Null)?;
    let page = ps(p, "page");
    let keys: Vec<String> = pages
        .as_array()
        .into_iter()
        .flatten()
        .filter(|pg| page.is_none_or(|w| pg["id"] == w))
        .flat_map(|pg| pg["items"].as_array().cloned().unwrap_or_default())
        .filter_map(|i| i.get("key").and_then(Json::as_str).map(str::to_string))
        .collect();
    if let Some(w) = page
        && !pages.as_array().into_iter().flatten().any(|pg| pg["id"] == w)
    {
        return Err(format!("`{w}` is not a settings page"));
    }
    for k in &keys {
        e.prefs.remove(k);
        diskcache::remove_pref(k);
        if k.starts_with("diskCache.") {
            let default = match k.as_str() {
                "diskCache.enabled" => json!(true),
                "diskCache.maxGB" => json!(10.0),
                _ => json!(""),
            };
            diskcache::configure(k, &default);
        }
    }
    if page.is_none() || page == Some("interface") {
        e.frontend("prefs.resetInterface", json!({}));
    }
    Ok(json!({"page": page, "reset": keys}))
}

fn auto_save_dirs(e: &Engine) -> Vec<PathBuf> {
    let mut v = vec![data_dir().join("Auto-Save")];
    if let Some(d) = e.path.as_deref().and_then(|p| Path::new(p).parent()) {
        v.push(d.join("formvfx Auto-Save"));
    }
    v
}

fn free_bytes(dir: &Path) -> Option<u64> {
    let out = std::process::Command::new("df").args(["-Pk", &dir.to_string_lossy()]).output().ok()?;
    let line = String::from_utf8_lossy(&out.stdout).lines().nth(1)?.to_string();
    line.split_whitespace().nth(3)?.parse::<u64>().ok().map(|k| k * 1024)
}

pub fn storage_info(e: &mut Engine, _: &Json) -> Res {
    let (media_n, media_b) = dir_size(&user_dir().join("imports"));
    let (auto_n, auto_b) = auto_save_dirs(e).iter().map(|d| dir_size(d)).fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    let (eng_n, eng_b) = dir_size(&data_dir());
    let projects = std::fs::read_dir(user_dir()).map(|rd| rd.flatten().filter(|x| x.path().extension().is_some_and(|x| x == "fvxproj" || x == "fvxprojx")).count()).unwrap_or(0);
    let cache = diskcache::stats();
    let usage = eng_b + media_b;
    let free = free_bytes(&data_dir());
    Ok(json!({
        "available": true,
        "backend": "disk",
        "usage": usage,
        "quota": free.map(|f| f + usage),
        "persisted": true,
        "folders": {"engine": data_dir().to_string_lossy(), "user": user_dir().to_string_lossy()},
        "files": {"count": eng_n + media_n, "bytes": usage, "media": {"count": media_n, "bytes": media_b}, "projects": projects, "autoSaves": {"count": auto_n, "bytes": auto_b}},
        "diskCache": cache,
    }))
}

pub fn storage_persist(_: &mut Engine, _: &Json) -> Res {
    Ok(json!({"requested": true, "persisted": true, "note": "formvfx keeps its data in folders on disk, which are never evicted"}))
}

fn used_paths(e: &Engine) -> Vec<String> {
    e.project.items.iter().filter_map(|i| match &i.kind {
        ItemKind::Footage(f) => Some(f.path.clone()),
        _ => None,
    }).collect()
}

pub fn storage_clear(e: &mut Engine, p: &Json) -> Res {
    let what = ps(p, "what").ok_or("pass `what` (diskCache, media, autoSaves or all)")?;
    if what == "projects" {
        return Err("formvfx never deletes saved projects; remove them in the file manager".into());
    }
    if !matches!(what, "diskCache" | "media" | "autoSaves" | "all") {
        return Err(format!("`{what}` is not a storage kind (diskCache, media, autoSaves, all)"));
    }
    let mut entries = 0u64;
    let mut bytes = 0u64;
    let mut cleared = vec![];
    if matches!(what, "diskCache" | "all") {
        let before = diskcache::stats();
        bytes += diskcache::clean();
        entries += before["files"].as_u64().unwrap_or(0);
        cleared.push("diskCache");
    }
    let mut kept = 0;
    let mut sweep = |dirs: Vec<PathBuf>, keep: &[String], entries: &mut u64, bytes: &mut u64| {
        for d in dirs {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for ent in rd.flatten() {
                let path = ent.path();
                if !path.is_file() {
                    continue;
                }
                if keep.iter().any(|k| Path::new(k) == path) {
                    kept += 1;
                    continue;
                }
                let len = ent.metadata().map(|m| m.len()).unwrap_or(0);
                if std::fs::remove_file(&path).is_ok() {
                    *entries += 1;
                    *bytes += len;
                }
            }
        }
    };
    if matches!(what, "media" | "all") {
        let keep = used_paths(e);
        sweep(vec![user_dir().join("imports")], &keep, &mut entries, &mut bytes);
        cleared.push("media");
    }
    if matches!(what, "autoSaves" | "all") {
        sweep(auto_save_dirs(e), &[], &mut entries, &mut bytes);
        cleared.push("autoSaves");
    }
    Ok(json!({"cleared": cleared, "entries": entries, "bytes": bytes, "keptInUse": kept}))
}

pub fn editor_state(e: &mut Engine, _: &Json) -> Res {
    let comp = e.state.active_comp;
    Ok(json!({
        "project": {"path": e.path, "dirty": e.is_dirty(), "items": e.project.items.len(), "revision": e.revision},
        "activeComp": comp,
        "openComps": e.state.open_comps,
        "time": comp.map(|c| e.time_of(c)),
        "selection": {"items": e.state.project_selection, "layers": e.state.selected_layers, "properties": e.state.selected_props.len(), "keys": e.state.selected_keys.len()},
        "viewer": {"layout": e.state.view_layout, "snapping": e.state.snapping, "channel": e.state.viewer.channel},
        "footagePanel": super::footage::info(e, &Json::Null)?,
        "mediaBrowser": e.session.browser.path.as_ref().map(|p| p.to_string_lossy().to_string()),
        "learn": state_json(e),
        "jobs": job_rows(e),
        "undo": e.history.undo.last().map(|s| s.label.clone()),
        "redo": e.history.redo.last().map(|s| s.label.clone()),
    }))
}

pub fn open_source(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?.clone();
    let src = l.source.ok_or_else(|| format!("{} has no source", l.name))?;
    match (l.kind, e.project.item(src).map(|i| &i.kind)) {
        (LayerKind::Precomp, _) | (_, Some(ItemKind::Comp(_))) => super::comp::open(e, &json!({"comp": src})),
        (_, Some(ItemKind::Footage(_))) => super::footage::open(e, &json!({"item": src})),
        _ => super::layermisc::reveal_source(e, p),
    }
}

pub fn layer_reveal_in_finder(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    let src = l.source.ok_or_else(|| format!("{} has no source file", l.name))?;
    match e.project.item(src).map(|i| &i.kind) {
        Some(ItemKind::Footage(f)) if !f.path.is_empty() => {
            let path = f.path.clone();
            super::file::reveal_path(&path)
        }
        _ => Err(format!("{} has no source file on disk", l.name)),
    }
}
