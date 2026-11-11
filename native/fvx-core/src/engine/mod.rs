use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Value as Json, json};

use crate::model::{Comp, ItemKind, Layer, Project};
use crate::render::Media;

pub mod cmd;
pub mod demo;
pub mod ecproj;
pub mod export;
pub mod jobs;
pub mod lottie;
pub mod menus;
pub mod registry;
pub mod rq;
pub mod state;
pub mod xmldoc;

pub type Res = Result<Json, String>;

#[derive(Clone, Debug, PartialEq)]
pub struct KeyRef {
    pub layer: u64,
    pub prop: u64,
    pub time: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VertexRef {
    pub layer: u64,
    pub mask: u64,
    pub index: usize,
}

#[derive(Clone, Debug, Default)]
pub struct TextEdit {
    pub layer: u64,
    pub anchor: usize,
    pub caret: usize,
}

#[derive(Clone, Debug)]
pub struct ViewerState {
    pub channel: String,
    pub colorized: bool,
    pub exposure: f64,
    pub display_cm: bool,
    pub simulate: String,
    pub preserve_rgb: bool,
    pub custom_rgb: [bool; 3],
    pub fast_previews: String,
    pub extended: bool,
    pub share_view_options: bool,
    pub scrub_audio: bool,
    pub playback: Json,
}

impl Default for ViewerState {
    fn default() -> Self {
        ViewerState {
            channel: "RGB".into(),
            colorized: false,
            exposure: 0.0,
            display_cm: true,
            simulate: "none".into(),
            preserve_rgb: false,
            custom_rgb: [true, true, false],
            fast_previews: "off".into(),
            extended: false,
            share_view_options: true,
            scrub_audio: true,
            playback: json!({"includeAudio": true, "loop": true, "cacheBeforePlayback": false, "range": "workArea", "resolution": "auto", "frameRate": "auto", "skip": 0, "fullScreen": false}),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub id: u64,
    pub comp: u64,
    pub time: f64,
    pub width: u32,
    pub height: u32,
    pub px: Arc<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct LockedViewer {
    pub comp: u64,
    pub name: String,
    pub view: String,
    pub time: f64,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    pub snapshot: Option<Snapshot>,
    pub show_snapshot: bool,
    pub locked_viewer: Option<LockedViewer>,
    pub active_comp: Option<u64>,
    pub open_comps: Vec<u64>,
    pub times: HashMap<u64, f64>,
    pub selected_layers: Vec<u64>,
    pub selected_props: Vec<(u64, u64)>,
    pub selected_keys: Vec<KeyRef>,
    pub selected_vertices: Vec<VertexRef>,
    pub project_selection: Vec<u64>,
    pub snapping: bool,
    pub hide_locked_masks: bool,
    pub roi: Option<[f64; 4]>,
    pub view_layout: u32,
    pub viewer: ViewerState,
    pub text_edit: Option<TextEdit>,
    pub views3d: HashMap<u64, cmd::view3d::View3d>,
    pub current_tracker: HashMap<u64, u64>,
    pub mask_method: String,
    pub solve_points: Vec<u32>,
    pub puppet_pins: Vec<u64>,
    pub essential_comp: Option<u64>,
}

pub struct Step {
    pub label: String,
    pub project: Arc<Project>,
}

#[derive(Default)]
pub struct History {
    pub undo: Vec<Step>,
    pub redo: Vec<Step>,
    pub merge: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Clip {
    Layers(Vec<Layer>, u64),
    Keys(Vec<(String, f64, crate::model::Key)>),
    Links(Vec<(String, String)>),
}

pub struct Engine {
    pub project: Arc<Project>,
    pub path: Option<String>,
    pub revision: u64,
    pub saved_revision: u64,
    pub history: History,
    pub state: State,
    pub events: Vec<Json>,
    pub media: Arc<Media>,
    pub clipboard: Option<Clip>,
    pub label: Option<String>,
    pub prefs: HashMap<String, Json>,
    pub shortcuts: HashMap<String, Vec<String>>,
    pub shortcut_preset: String,
    pub shortcut_presets: std::collections::BTreeMap<String, HashMap<String, Vec<String>>>,
    pub recent_presets: Vec<String>,
    pub snapshot_seq: u64,
    pub render: export::Queue,
    pub key_drag: Option<(String, Vec<(KeyRef, f64, Json)>)>,
    pub merge_hint: Option<String>,
    changed: bool,
    batch_depth: u32,
    pub imported_from: Option<String>,
    pub jobs: jobs::Jobs,
    pub multi_frame: bool,
    pub session: cmd::session::Session,
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new()
    }
}

pub fn pf(p: &Json, k: &str) -> Option<f64> {
    p.get(k).and_then(Json::as_f64)
}

pub fn ps<'a>(p: &'a Json, k: &str) -> Option<&'a str> {
    p.get(k).and_then(Json::as_str)
}

pub fn pb(p: &Json, k: &str) -> Option<bool> {
    p.get(k).and_then(Json::as_bool)
}

pub fn pu(p: &Json, k: &str) -> Option<u64> {
    p.get(k).and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|x| x.max(0.0) as u64)))
}

impl Engine {
    pub fn new() -> Engine {
        let media = Arc::new(Media::default());
        if crate::render::media::shared().is_none() {
            crate::render::media::share(&media);
        }
        static PLUGINS: std::sync::Once = std::sync::Once::new();
        PLUGINS.call_once(|| {
            crate::effects::plugin::load_folder(&crate::effects::plugin::folder());
            crate::effects::ofx::recover();
            crate::effects::ofx::load_folder(&crate::effects::plugin::folder());
            for dir in crate::effects::ofx::system_folders() {
                crate::effects::ofx::load_folder(&dir);
            }
        });
        Engine {
            project: Arc::new(Project::default()),
            path: None,
            revision: 0,
            saved_revision: 0,
            history: History::default(),
            state: State { view_layout: 1, snapping: true, ..State::default() },
            events: vec![],
            media,
            clipboard: None,
            label: None,
            prefs: crate::render::diskcache::load_prefs().into_iter().filter(|(k, _)| crate::render::diskcache::persisted(k)).collect(),
            shortcuts: HashMap::new(),
            shortcut_preset: cmd::viewer::DEFAULT_PRESET.to_string(),
            shortcut_presets: std::collections::BTreeMap::new(),
            recent_presets: vec![],
            snapshot_seq: 0,
            render: export::Queue::default(),
            key_drag: None,
            merge_hint: None,
            changed: false,
            batch_depth: 0,
            imported_from: None,
            jobs: jobs::Jobs::default(),
            multi_frame: true,
            session: cmd::session::Session::load(),
        }
    }

    pub fn apply_step(&mut self, label: &str, f: impl FnOnce(&mut Engine) -> Result<Json, String>) -> Res {
        let before = self.project.clone();
        self.changed = false;
        match f(self) {
            Err(e) => {
                self.project = before;
                self.changed = false;
                Err(e)
            }
            Ok(v) => {
                if self.changed {
                    self.changed = false;
                    self.push_undo(Step { label: label.to_string(), project: before });
                    self.history.redo.clear();
                    self.history.merge = None;
                }
                self.revision += 1;
                Ok(v)
            }
        }
    }

    pub fn proj(&mut self) -> &mut Project {
        self.changed = true;
        Arc::make_mut(&mut self.project)
    }

    fn push_undo(&mut self, step: Step) {
        self.history.undo.push(step);
        let extra = self.history.undo.len().saturating_sub(300);
        self.history.undo.drain(..extra);
    }

    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    pub fn emit(&mut self, ev: Json) {
        self.events.push(ev);
    }

    pub fn frontend(&mut self, command: &str, params: Json) {
        self.events.push(json!({"type": "frontend", "command": command, "params": params}));
    }

    pub fn toast(&mut self, msg: &str, error: bool) {
        self.events.push(json!({"type": "toast", "message": msg, "error": error}));
    }

    pub fn drain_events(&mut self) -> Vec<Json> {
        std::mem::take(&mut self.events)
    }

    pub fn time_of(&self, comp: u64) -> f64 {
        self.state.times.get(&comp).copied().unwrap_or(0.0)
    }

    pub fn now(&self) -> f64 {
        self.state.active_comp.map(|c| self.time_of(c)).unwrap_or(0.0)
    }

    pub fn execute(&mut self, id: &str, params: Json) -> Res {
        let started = std::time::Instant::now();
        let r = self.run_command(id, params);
        cmd::help::after(self, id, &r, started.elapsed().as_secs_f64());
        r
    }

    fn run_command(&mut self, id: &str, params: Json) -> Res {
        let Some(cmd) = registry::find(id) else {
            return Err(format!("unknown command `{id}` (command.list shows them)"));
        };
        let params = if params.is_null() { json!({}) } else { params };
        match cmd.kind {
            registry::K::Front => {
                self.frontend(id, params);
                return Ok(Json::Null);
            }
            registry::K::Todo => return Err(format!("{} is not in formvfx yet", cmd.label.trim_end_matches("..."))),
            _ => {}
        }
        if !cmd.undo {
            let r = (cmd.run)(self, &params);
            if self.changed && self.batch_depth == 0 {
                self.changed = false;
                self.revision += 1;
            }
            return r;
        }
        let before = self.project.clone();
        let outer = self.batch_depth == 0;
        if outer {
            self.changed = false;
            self.label = None;
            self.merge_hint = None;
        }
        let r = (cmd.run)(self, &params);
        match r {
            Err(e) => {
                if outer {
                    self.project = before;
                    self.changed = false;
                }
                Err(e)
            }
            Ok(v) => {
                if outer && self.changed {
                    cmd::essential::sync(self);
                    self.changed = false;
                    let merge = ps(&params, "merge").map(str::to_string).or_else(|| self.merge_hint.take());
                    let label = self.label.take().unwrap_or_else(|| cmd.label.trim_end_matches("...").to_string());
                    if merge.is_none() || merge != self.history.merge {
                        self.push_undo(Step { label, project: before });
                    }
                    self.history.redo.clear();
                    self.history.merge = merge;
                    self.revision += 1;
                }
                Ok(v)
            }
        }
    }

    pub fn tick(&mut self, now: f64) -> Json {
        let mut busy = vec![];
        if self.render.busy() {
            busy.push("render");
        }
        if !export::poll(self).is_empty() {
            self.revision += 1;
        }
        if jobs::poll(self) {
            self.revision += 1;
        }
        if self.jobs.running() {
            busy.push("analysis");
        }
        if self.session.watch.as_ref().is_some_and(|w| w.busy()) {
            busy.push("watchFolder");
        }
        let autosave = cmd::session::tick(self, now);
        json!({"busy": busy, "autosave": autosave, "revision": self.revision})
    }

    pub fn end_gesture(&mut self) {
        self.history.merge = None;
        self.key_drag = None;
    }

    pub fn batch(&mut self, p: &Json) -> Res {
        let steps = p.get("steps").and_then(Json::as_array).cloned().ok_or("`steps` must be a list of {command, params}")?;
        let atomic = pb(p, "atomic").unwrap_or(true);
        let label = ps(p, "label").unwrap_or("Batch").to_string();
        let before = self.project.clone();
        let outer = self.batch_depth == 0;
        if outer {
            self.changed = false;
        }
        self.batch_depth += 1;
        let mut results: Vec<Json> = vec![];
        let mut failed = None;
        for (i, s) in steps.iter().enumerate() {
            let Some(cmd) = s.get("command").and_then(Json::as_str) else {
                failed = Some(format!("step {}: missing `command`", i + 1));
                break;
            };
            let params = resolve_refs(s.get("params").cloned().unwrap_or(json!({})), &results);
            match self.execute(cmd, params) {
                Ok(v) => results.push(v),
                Err(e) => {
                    let msg = format!("step {} ({cmd}) failed: {e}", i + 1);
                    if atomic {
                        failed = Some(msg);
                        break;
                    }
                    results.push(json!({"error": msg}));
                }
            }
        }
        self.batch_depth -= 1;
        if let Some(e) = failed {
            if outer {
                self.project = before;
                self.changed = false;
            }
            return Err(e);
        }
        if outer && self.changed {
            self.changed = false;
            self.push_undo(Step { label, project: before });
            self.history.redo.clear();
            self.history.merge = None;
            self.revision += 1;
        }
        Ok(json!({"steps": results.len(), "results": results}))
    }

    pub fn undo(&mut self) -> Res {
        let Some(step) = self.history.undo.pop() else { return Err("nothing to undo".into()) };
        let cur = std::mem::replace(&mut self.project, step.project);
        self.history.redo.push(Step { label: step.label.clone(), project: cur });
        self.history.merge = None;
        self.revision += 1;
        self.after_history();
        Ok(json!({"undone": step.label}))
    }

    pub fn redo(&mut self) -> Res {
        let Some(step) = self.history.redo.pop() else { return Err("nothing to redo".into()) };
        let cur = std::mem::replace(&mut self.project, step.project);
        self.history.undo.push(Step { label: step.label.clone(), project: cur });
        self.history.merge = None;
        self.revision += 1;
        self.after_history();
        Ok(json!({"redone": step.label}))
    }

    fn after_history(&mut self) {
        let p = self.project.clone();
        if let Some(c) = self.state.active_comp
            && p.comp(c).is_none()
        {
            self.state.active_comp = None;
        }
        self.state.open_comps.retain(|c| p.comp(*c).is_some());
        self.state.selected_layers.retain(|l| p.comp_of_layer(*l).is_some());
    }

    pub fn reset(&mut self, project: Project, path: Option<String>) {
        self.project = Arc::new(project);
        self.path = path;
        self.history = History::default();
        self.state = State { view_layout: 1, snapping: true, ..State::default() };
        self.revision += 1;
        self.saved_revision = self.revision;
        self.clipboard = None;
        let first = self.project.items.iter().find(|i| matches!(i.kind, ItemKind::Comp(_))).map(|i| i.id);
        if let Some(c) = first {
            self.state.active_comp = Some(c);
            self.state.open_comps = vec![c];
        }
    }

    pub fn comp_id(&self, p: Option<&Json>) -> Result<u64, String> {
        match p {
            Some(Json::Number(n)) => {
                let id = n.as_u64().unwrap_or(0);
                if self.project.comp(id).is_some() {
                    Ok(id)
                } else {
                    Err(format!("no composition with id {id}"))
                }
            }
            Some(Json::String(s)) => {
                if let Ok(id) = s.parse::<u64>()
                    && self.project.comp(id).is_some()
                {
                    return Ok(id);
                }
                self.project.items.iter().find(|i| matches!(i.kind, ItemKind::Comp(_)) && i.name == *s).map(|i| i.id).ok_or_else(|| format!("no composition \"{s}\""))
            }
            _ => self.state.active_comp.filter(|c| self.project.comp(*c).is_some()).ok_or_else(|| "no active composition (comp.new makes one)".to_string()),
        }
    }

    pub fn comp_of(&self, p: &Json) -> Result<(u64, &Comp), String> {
        let id = self.comp_id(p.get("comp"))?;
        Ok((id, self.project.comp(id).ok_or("no composition")?))
    }

    pub fn layer_in(&self, comp: &Comp, v: &Json) -> Result<u64, String> {
        match v {
            Json::Number(n) => {
                let id = n.as_u64().unwrap_or(0);
                if comp.layer(id).is_some() {
                    return Ok(id);
                }
                Err(format!("no layer with id {id} in this composition"))
            }
            Json::String(s) => {
                if let Some(i) = s.strip_prefix('#').and_then(|x| x.parse::<usize>().ok()) {
                    return comp.layers.get(i.wrapping_sub(1)).map(|l| l.id).ok_or_else(|| format!("no layer {s}"));
                }
                if let Ok(id) = s.parse::<u64>()
                    && comp.layer(id).is_some()
                {
                    return Ok(id);
                }
                comp.layers.iter().find(|l| l.name == *s).map(|l| l.id).ok_or_else(|| format!("no layer \"{s}\""))
            }
            _ => Err("expected a layer id, #index or name".into()),
        }
    }

    pub fn layer_param(&self, p: &Json) -> Result<(u64, u64), String> {
        let (cid, comp) = self.comp_of(p)?;
        let id = match p.get("layer") {
            Some(v) if !v.is_null() => self.layer_in(comp, v)?,
            _ => self.state.selected_layers.iter().copied().find(|l| comp.layer(*l).is_some()).ok_or("select a layer or pass `layer`")?,
        };
        Ok((cid, id))
    }

    pub fn layers_param(&self, p: &Json) -> Result<(u64, Vec<u64>), String> {
        let (cid, comp) = self.comp_of(p)?;
        if let Some(a) = p.get("layers").and_then(Json::as_array) {
            let ids: Result<Vec<u64>, String> = a.iter().map(|v| self.layer_in(comp, v)).collect();
            return Ok((cid, ids?));
        }
        if let Some(v) = p.get("layer").filter(|v| !v.is_null()) {
            return Ok((cid, vec![self.layer_in(comp, v)?]));
        }
        let sel: Vec<u64> = self.state.selected_layers.iter().copied().filter(|l| comp.layer(*l).is_some()).collect();
        Ok((cid, sel))
    }

    pub fn prop_uid(&self, layer: &Layer, p: &Json) -> Result<u64, String> {
        if let Some(u) = p.get("prop").and_then(Json::as_u64) {
            return layer.props.find(u).map(|x| x.uid).or_else(|| layer.props.find_group(u).map(|g| g.uid)).ok_or_else(|| format!("no property {u} on {}", layer.name));
        }
        let path = ps(p, "path").or_else(|| ps(p, "prop")).ok_or("pass `path` (like transform/position) or `prop` (a uid)")?;
        layer.props.resolve_uid(path).ok_or_else(|| format!("no property `{path}` on {} (layer.tree lists the paths)", layer.name))
    }

    pub fn layer_ref(&self, cid: u64, lid: u64) -> Result<&Layer, String> {
        self.project.comp(cid).and_then(|c| c.layer(lid)).ok_or_else(|| "no such layer".to_string())
    }

    pub fn layer_mut(&mut self, cid: u64, lid: u64) -> Result<&mut Layer, String> {
        self.proj().comp_mut(cid).and_then(|c| c.layer_mut(lid)).ok_or_else(|| "no such layer".to_string())
    }

    pub fn set_label(&mut self, l: impl Into<String>) {
        self.label = Some(l.into());
    }

    pub fn capabilities(&self) -> Json {
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "engine": "fvx",
            "commands": registry::all().len(),
            "effects": {"total": crate::effects::every().len(), "plugins": crate::effects::plugin::specs().len(), "ofx": crate::effects::ofx::specs().len()},
            "export": export::FORMATS.iter().map(|f| f.0).collect::<Vec<_>>(),
            "projectExtension": crate::model::EXTENSION,
            "presetExtension": "fvxpreset",
            "importProjects": ["fvxproj", "fvxprojx", "ecproj", "ecprojx"],
            "ffmpeg": crate::render::media::has_ffmpeg(),
        })
    }
}

fn lookup<'a>(v: &'a Json, path: &[&str]) -> Option<&'a Json> {
    let mut cur = v;
    for seg in path {
        cur = match cur {
            Json::Object(o) => o.get(*seg)?,
            Json::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

pub fn resolve_refs(v: Json, results: &[Json]) -> Json {
    match v {
        Json::String(s) if s.starts_with('$') => {
            let body = &s[1..];
            let mut parts = body.split('.');
            let Some(n) = parts.next().and_then(|x| x.parse::<usize>().ok()) else { return Json::String(s) };
            let rest: Vec<&str> = parts.collect();
            let Some(r) = n.checked_sub(1).and_then(|i| results.get(i)) else { return Json::String(s) };
            if rest.is_empty() {
                return r.clone();
            }
            lookup(r, &rest).cloned().unwrap_or(Json::String(s))
        }
        Json::Array(a) => Json::Array(a.into_iter().map(|x| resolve_refs(x, results)).collect()),
        Json::Object(o) => Json::Object(o.into_iter().map(|(k, x)| (k, resolve_refs(x, results))).collect()),
        other => other,
    }
}
