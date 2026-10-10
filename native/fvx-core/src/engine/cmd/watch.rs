use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use serde_json::{Value as Json, json};

use crate::engine::export::Progress;
use crate::engine::{Engine, Res, pb, ps};
use crate::render::Media;

pub struct Watch {
    pub folder: PathBuf,
    stop: Arc<AtomicBool>,
    working: Arc<AtomicBool>,
    rendered: Arc<Mutex<Vec<Json>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Watch {
    pub fn busy(&self) -> bool {
        self.working.load(Ordering::SeqCst)
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn status_path(project: &Path) -> PathBuf {
    let stem = project.file_stem().and_then(|s| s.to_str()).unwrap_or("project");
    project.with_file_name(format!("{stem}.status.json"))
}

fn pending(folder: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(folder) else { return vec![] };
    let mut out: Vec<PathBuf> = rd
        .flatten()
        .map(|x| x.path())
        .filter(|p| p.is_file() && p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("fvxproj") || x.eq_ignore_ascii_case("fvxprojx")))
        .filter(|p| !status_path(p).exists())
        .filter(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_none_or(|age| age.as_secs_f64() >= 1.0))
        .collect();
    out.sort();
    out
}

fn render_project(path: &Path, media: &Media, stop: &AtomicBool) -> Json {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    let _ = std::fs::write(status_path(path), serde_json::to_string_pretty(&json!({"project": name, "state": "rendering"})).unwrap_or_default());
    let s = path.to_string_lossy().to_string();
    let result = (|| -> Result<Vec<Json>, String> {
        let (mut pj, _) = super::file::read_project(&s)?;
        super::file::relink(&mut pj, &s);
        let queued: Vec<_> = pj.render_queue.iter().filter(|r| r.render && r.status != "Done").cloned().collect();
        if queued.is_empty() {
            return Err("its Render Queue holds nothing queued".into());
        }
        let out_dir = path.parent().unwrap_or(Path::new(".")).join(format!("{} Output", path.file_stem().and_then(|x| x.to_str()).unwrap_or("project")));
        std::fs::create_dir_all(&out_dir).map_err(|x| format!("{}: {x}", out_dir.display()))?;
        let mut items = vec![];
        for mut it in queued {
            it.fix();
            for m in it.modules.iter_mut() {
                if m.output.is_empty() {
                    m.output = "[compName].[fileExtension]".into();
                }
                if !Path::new(&m.output).is_absolute() {
                    m.output = out_dir.join(&m.output).to_string_lossy().to_string();
                }
            }
            let comp = pj.item(it.comp).map(|i| i.name.clone()).unwrap_or_default();
            let progress = Mutex::new(Progress::default());
            let r = crate::engine::rq::render_item(&pj, media, &it, &progress, stop, rayon::current_num_threads().clamp(1, 16));
            items.push(match r {
                Ok(o) => json!({"comp": comp, "status": "done", "output": o.lines().collect::<Vec<_>>()}),
                Err(err) => json!({"comp": comp, "status": "failed", "error": err}),
            });
        }
        Ok(items)
    })();
    let row = match result {
        Ok(items) => {
            let failed = items.iter().any(|i| i["status"] == "failed");
            json!({"project": name, "path": s, "state": if failed { "failed" } else { "done" }, "items": items})
        }
        Err(err) => json!({"project": name, "path": s, "state": "failed", "error": err, "items": []}),
    };
    let _ = std::fs::write(status_path(path), serde_json::to_string_pretty(&row).unwrap_or_default());
    row
}

pub fn watch_folder(e: &mut Engine, p: &Json) -> Res {
    if pb(p, "stop").unwrap_or(false) {
        let was = e.session.watch.take().map(|w| w.folder.to_string_lossy().to_string());
        return Ok(json!({"watching": false, "stopped": was}));
    }
    let folder = PathBuf::from(ps(p, "folder").ok_or("pass `folder`")?);
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    e.session.watch = None;
    let stop = Arc::new(AtomicBool::new(false));
    let working = Arc::new(AtomicBool::new(false));
    let rendered = Arc::new(Mutex::new(vec![]));
    let media = e.media.clone();
    let (s, w, r, f) = (stop.clone(), working.clone(), rendered.clone(), folder.clone());
    let handle = std::thread::spawn(move || {
        while !s.load(Ordering::SeqCst) {
            if let Some(next) = pending(&f).into_iter().next() {
                w.store(true, Ordering::SeqCst);
                let row = render_project(&next, &media, &s);
                r.lock().push(row);
                w.store(false, Ordering::SeqCst);
                continue;
            }
            for _ in 0..10 {
                if s.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    });
    e.session.watch = Some(Watch { folder: folder.clone(), stop, working, rendered, handle: Some(handle) });
    Ok(json!({"watching": true, "folder": folder.to_string_lossy(), "pending": pending(&folder).len()}))
}

pub fn poll(e: &mut Engine, p: &Json) -> Res {
    let asked = ps(p, "folder").map(PathBuf::from);
    if let Some(w) = &e.session.watch
        && asked.as_ref().is_none_or(|a| *a == w.folder)
    {
        return Ok(json!({"watching": true, "folder": w.folder.to_string_lossy(), "busy": w.busy(), "pending": pending(&w.folder).len(), "rendered": w.rendered.lock().clone()}));
    }
    let Some(folder) = asked else {
        return Ok(json!({"watching": false, "rendered": []}));
    };
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    let stop = AtomicBool::new(false);
    let rendered: Vec<Json> = pending(&folder).iter().map(|x| render_project(x, &e.media, &stop)).collect();
    Ok(json!({"watching": false, "folder": folder.to_string_lossy(), "rendered": rendered}))
}
