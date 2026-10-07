use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use parking_lot::Mutex;
use serde_json::{Value as Json, json};

use super::{Engine, Res};
use crate::model::Project;
use crate::render::Media;

pub type Apply = Box<dyn FnOnce(&mut Engine) -> Result<Json, String> + Send>;

pub struct Prog {
    pub done: usize,
    pub total: usize,
    pub task: String,
    pub started: Instant,
}

pub struct Ctx {
    pub project: Arc<Project>,
    pub media: Arc<Media>,
    pub prog: Arc<Mutex<Prog>>,
    pub stop: Arc<AtomicBool>,
}

impl Ctx {
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    pub fn step(&self, done: usize, total: usize, task: &str) {
        let mut p = self.prog.lock();
        p.done = done;
        p.total = total;
        if p.task != task {
            p.task = task.to_string();
        }
    }
}

pub struct Job {
    pub kind: String,
    pub layer: u64,
    pub label: String,
    pub prog: Arc<Mutex<Prog>>,
    pub stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<Result<Apply, String>>>,
}

#[derive(Default)]
pub struct Jobs {
    pub active: Vec<Job>,
    pub last: HashMap<(u64, String), Json>,
}

fn prog_json(p: &Prog, finished: bool, error: Option<&str>) -> Json {
    let secs = p.started.elapsed().as_secs_f64().max(1e-3);
    json!({"done": p.done, "total": p.total, "fps": (p.done as f64 / secs * 10.0).round() / 10.0, "finished": finished, "error": error, "task": p.task})
}

impl Jobs {
    pub fn running(&self) -> bool {
        !self.active.is_empty()
    }

    pub fn running_for(&self, layer: u64, kind: &str) -> bool {
        self.active.iter().any(|j| j.layer == layer && j.kind == kind)
    }

    pub fn progress(&self, layer: u64, kind: &str) -> Json {
        if let Some(j) = self.active.iter().find(|j| j.layer == layer && j.kind == kind) {
            return prog_json(&j.prog.lock(), false, None);
        }
        self.last.get(&(layer, kind.to_string())).cloned().unwrap_or(Json::Null)
    }

    pub fn stop(&mut self, kind: Option<&str>) -> usize {
        let mut n = 0;
        for j in &self.active {
            if kind.is_none_or(|k| j.kind == k) {
                j.stop.store(true, Ordering::Relaxed);
                n += 1;
            }
        }
        n
    }
}

pub fn run(e: &mut Engine, kind: &str, layer: u64, label: &str, wait: bool, work: impl FnOnce(&Ctx) -> Result<Apply, String> + Send + 'static) -> Res {
    for j in &e.jobs.active {
        if j.layer == layer && j.kind == kind {
            j.stop.store(true, Ordering::Relaxed);
        }
    }
    let prog = Arc::new(Mutex::new(Prog { done: 0, total: 0, task: String::new(), started: Instant::now() }));
    let stop = Arc::new(AtomicBool::new(false));
    let ctx = Ctx { project: e.project.clone(), media: e.media.clone(), prog: prog.clone(), stop: stop.clone() };
    if wait {
        let r = work(&ctx);
        let p = prog.lock();
        match r {
            Ok(apply) => {
                drop(p);
                let out = e.apply_step(label, apply)?;
                e.jobs.last.insert((layer, kind.to_string()), prog_json(&prog.lock(), true, None));
                Ok(out)
            }
            Err(err) => {
                e.jobs.last.insert((layer, kind.to_string()), prog_json(&p, true, Some(&err)));
                Err(err)
            }
        }
    } else {
        let handle = std::thread::Builder::new().name(format!("formvfx-{kind}")).spawn(move || work(&ctx)).map_err(|err| err.to_string())?;
        e.jobs.active.push(Job { kind: kind.to_string(), layer, label: label.to_string(), prog, stop, handle: Some(handle) });
        e.jobs.last.remove(&(layer, kind.to_string()));
        Ok(json!({"running": true, "layer": layer}))
    }
}

pub fn poll(e: &mut Engine) -> bool {
    let done: Vec<usize> = e.jobs.active.iter().enumerate().filter(|(_, j)| j.handle.as_ref().is_none_or(|h| h.is_finished())).map(|(i, _)| i).collect();
    if done.is_empty() {
        return false;
    }
    for i in done.into_iter().rev() {
        let mut job = e.jobs.active.remove(i);
        let stopped = job.stop.load(Ordering::Relaxed);
        let result = job.handle.take().map(|h| h.join().unwrap_or_else(|_| Err("the analysis stopped unexpectedly".into()))).unwrap_or_else(|| Err("no analysis".into()));
        let key = (job.layer, job.kind.clone());
        match result {
            Ok(apply) if !stopped => match e.apply_step(&job.label, apply) {
                Ok(_) => {
                    e.jobs.last.insert(key, prog_json(&job.prog.lock(), true, None));
                    e.toast(&format!("{} finished", job.label), false);
                }
                Err(err) => {
                    e.jobs.last.insert(key, prog_json(&job.prog.lock(), true, Some(&err)));
                    e.toast(&format!("{}: {err}", job.label), true);
                }
            },
            Ok(_) => {
                e.jobs.last.insert(key, prog_json(&job.prog.lock(), true, Some("stopped")));
            }
            Err(err) => {
                e.jobs.last.insert(key, prog_json(&job.prog.lock(), true, Some(&err)));
                e.toast(&format!("{}: {err}", job.label), true);
            }
        }
    }
    true
}
