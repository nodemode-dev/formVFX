use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{Value as Json, json};

use super::host::{self, Cx, H, hv};
use super::interp::*;
use super::scriptui;
use crate::engine::Engine;
use crate::eval::Eval;
use crate::model::{BlendMode, Comp, GroupKind, ItemKind, Justify, Layer, LayerInit, LayerKind, MatteKind, Project, Prop, TextDoc, Ui, Value, build_layer};

pub const SCRIPT_STEPS: u64 = 400_000_000;

struct Dummy {
    project: Project,
    comp: Comp,
    layer: Layer,
    prop: Prop,
}

fn make_dummy() -> Dummy {
    let mut project = Project::default();
    let layer = build_layer(&mut project, (100, 100), LayerInit { kind: LayerKind::Null, name: String::new(), source: None, size: (100.0, 100.0), start: 0.0, duration: 1.0 });
    Dummy { project, comp: Comp::default(), layer, prop: Prop::new(0, "value", "Value", Value::Num(0.0), Ui::Percent) }
}

thread_local! {
    static DUMMY: &'static Dummy = Box::leak(Box::new(make_dummy()));
    static ENGINE: Cell<*mut Engine> = const { Cell::new(std::ptr::null_mut()) };
    static OUT: RefCell<String> = const { RefCell::new(String::new()) };
    static UNDO_NAME: RefCell<Option<String>> = const { RefCell::new(None) };
    static SETTINGS: RefCell<BTreeMap<String, String>> = const { RefCell::new(BTreeMap::new()) };
    static FILE_NAME: RefCell<String> = const { RefCell::new(String::new()) };
}

fn dummy() -> &'static Dummy {
    DUMMY.with(|d| *d)
}

fn engine_ptr() -> *mut Engine {
    ENGINE.with(|e| e.get())
}

pub fn running() -> bool {
    !engine_ptr().is_null()
}

fn eng() -> Result<&'static mut Engine, Flow> {
    let p = engine_ptr();
    if p.is_null() {
        return Err(throw("Error", "no project is open to script"));
    }
    Ok(unsafe { &mut *p })
}

fn live<'x>() -> &'x Project {
    unsafe { &*Arc::as_ptr(&(*engine_ptr()).project) }
}

fn park(it: &mut Interp) {
    it.cx.project = &dummy().project;
}

fn sync(it: &mut Interp) {
    if running() {
        it.cx.project = live();
    }
}

pub fn exec(it: &mut Interp, id: &str, p: Json) -> Result<Json, Flow> {
    park(it);
    let r = eng().map(|e| e.execute(id, p));
    sync(it);
    match r? {
        Ok(v) => Ok(v),
        Err(m) => Err(throw("Error", m)),
    }
}

pub fn output(text: &str) {
    OUT.with(|o| o.borrow_mut().push_str(text));
}

fn line_of(m: &str) -> u32 {
    m.rsplit("(line ").next().and_then(|t| t.trim_end_matches(')').parse().ok()).unwrap_or(0)
}

struct Session {
    undo: Option<String>,
}

fn enter(e: &mut Engine, file: &str) -> Result<Session, String> {
    if running() {
        return Err("a script is already running".into());
    }
    ENGINE.with(|x| x.set(e as *mut Engine));
    OUT.with(|o| o.borrow_mut().clear());
    UNDO_NAME.with(|u| *u.borrow_mut() = None);
    FILE_NAME.with(|f| *f.borrow_mut() = file.to_string());
    Ok(Session { undo: None })
}

impl Drop for Session {
    fn drop(&mut self) {
        self.undo = UNDO_NAME.with(|u| u.borrow_mut().take());
        ENGINE.with(|x| x.set(std::ptr::null_mut()));
    }
}

fn new_interp<'x>(now: f64) -> Interp<'x> {
    let d = dummy();
    let cx = Cx::new(live(), 0, &d.comp, &d.layer, &d.prop, now);
    let mut it = Interp::new(cx);
    it.script = true;
    it.max_steps = SCRIPT_STEPS;
    it
}

fn finish(e: &mut Engine, ok: Result<Json, String>) -> Json {
    let output = OUT.with(|o| std::mem::take(&mut *o.borrow_mut()));
    let undo = UNDO_NAME.with(|u| u.borrow_mut().take());
    e.set_label(undo.unwrap_or_else(|| "Run Script".into()));
    scriptui::flush(e);
    match ok {
        Ok(result) => json!({"ok": true, "result": result, "output": output, "error": null}),
        Err(m) => json!({"ok": false, "result": null, "output": output, "error": {"message": m, "line": line_of(&m)}}),
    }
}

pub fn run(e: &mut Engine, code: &str, file: &str, this: Option<V>) -> Json {
    run_with(e, code, file, this, vec![])
}

pub fn run_with(e: &mut Engine, code: &str, file: &str, this: Option<V>, answers: Vec<scriptui::Answer>) -> Json {
    let before = e.project.clone();
    scriptui::begin_run(answers);
    let out = run_once(e, code, file, this);
    if let Some((w, first, answers)) = scriptui::end_run() {
        e.project = before;
        scriptui::suspend(code.to_string(), file.to_string(), w, first, answers);
        scriptui::flush(e);
        e.set_label("Run Script");
        return json!({"ok": true, "suspended": true, "dialog": w, "result": null, "output": out["output"], "error": null});
    }
    out
}

fn run_once(e: &mut Engine, code: &str, file: &str, this: Option<V>) -> Json {
    let parsed = super::program(code);
    let prog = match &*parsed {
        Ok(p) => p,
        Err(m) => return json!({"ok": false, "result": null, "output": "", "error": {"message": m, "line": line_of(m)}}),
    };
    let now = e.now();
    let session = match enter(e, file) {
        Ok(s) => s,
        Err(m) => return json!({"ok": false, "result": null, "output": "", "error": {"message": m, "line": 0}}),
    };
    let r = {
        let mut it = new_interp(now);
        if let Some(t) = this {
            it.this = t;
        }
        match it.run(prog) {
            Ok(v) => to_json(&mut it, &v, 0).map_err(|f| flow_message(&mut it, f)),
            Err(m) => Err(m),
        }
    };
    let undo = UNDO_NAME.with(|u| u.borrow().clone());
    drop(session);
    if let Some(u) = undo {
        UNDO_NAME.with(|x| *x.borrow_mut() = Some(u));
    }
    finish(e, r)
}

pub fn call(e: &mut Engine, f: &V, this: V, args: Vec<V>) -> Json {
    let now = e.now();
    let session = match enter(e, "") {
        Ok(s) => s,
        Err(m) => return json!({"ok": false, "result": null, "output": "", "error": {"message": m, "line": 0}}),
    };
    let r = {
        let mut it = new_interp(now);
        match it.call(f, this, args) {
            Ok(v) => to_json(&mut it, &v, 0).map_err(|fl| flow_message(&mut it, fl)),
            Err(fl) => Err(flow_message(&mut it, fl)),
        }
    };
    let undo = UNDO_NAME.with(|u| u.borrow().clone());
    drop(session);
    if let Some(u) = undo {
        UNDO_NAME.with(|x| *x.borrow_mut() = Some(u));
    }
    finish(e, r)
}

fn flow_message(it: &mut Interp, f: Flow) -> String {
    match f {
        Flow::Throw(v) => format!("{} (line {})", it.describe_error(&v), it.line),
        Flow::Fatal(m) => m,
        _ => format!("SyntaxError: misplaced return, break or continue (line {})", it.line),
    }
}

pub fn to_json(it: &mut Interp, v: &V, depth: u32) -> Result<Json, Flow> {
    if depth > 24 {
        return Ok(Json::Null);
    }
    Ok(match v {
        V::Undef | V::Null => Json::Null,
        V::Bool(b) => json!(b),
        V::Num(n) => {
            if n.is_finite() {
                json!(n)
            } else {
                Json::Null
            }
        }
        V::Str(st) => json!(&**st),
        V::Arr(a) => {
            let items = a.borrow().clone();
            let mut out = Vec::with_capacity(items.len());
            for x in &items {
                out.push(to_json(it, x, depth + 1)?);
            }
            Json::Array(out)
        }
        V::Obj(o) => {
            let props = o.borrow().props.clone();
            let mut m = serde_json::Map::new();
            for (k, x) in props {
                if matches!(x, V::Func(_) | V::Native(_)) {
                    continue;
                }
                m.insert(k.to_string(), to_json(it, &x, depth + 1)?);
            }
            Json::Object(m)
        }
        V::Func(_) | V::Native(_) => Json::Null,
        V::Path(p) => json!({"vertices": p.vertices, "inTangents": p.ins, "outTangents": p.outs, "closed": p.closed}),
        V::Host(h) => host_json(it, h)?,
    })
}

fn host_json(it: &mut Interp, h: &Rc<H>) -> Result<Json, Flow> {
    let p = pj(it);
    Ok(match &**h {
        H::Layer(c, l) => {
            let name = p.comp(*c).and_then(|cc| cc.layer(*l)).map(|x| x.name.clone()).unwrap_or_default();
            json!({"type": "Layer", "id": l, "comp": c, "name": name})
        }
        H::Comp(id) | H::Footage(id) | H::Folder(id) => {
            let item = p.item(*id);
            json!({"type": item.map(type_name).unwrap_or("Folder"), "id": id, "name": item.map(|i| i.name.clone()).unwrap_or_else(|| "Root".into())})
        }
        H::Prop(..) | H::Key(..) => {
            let v = it.primitive(&V::Host(h.clone()))?;
            if matches!(v, V::Host(_)) { json!(h.describe()) } else { to_json(it, &v, 1)? }
        }
        H::Group(c, l, u) => {
            let name = p.comp(*c).and_then(|cc| cc.layer(*l)).and_then(|x| x.props.find_group(*u)).map(|g| g.name.clone()).unwrap_or_default();
            json!({"type": "PropertyGroup", "name": name})
        }
        other => json!(other.describe()),
    })
}

fn type_name(i: &crate::model::Item) -> &'static str {
    match i.kind {
        ItemKind::Comp(_) => "CompItem",
        ItemKind::Folder => "FolderItem",
        _ => "FootageItem",
    }
}

fn pj<'x>(it: &Interp<'x>) -> &'x Project {
    it.cx.project
}

const CLASSES: [&str; 32] = [
    "Date", "File", "Folder", "ImportOptions", "KeyframeEase", "TextDocument", "Window", "Shape", "MarkerValue", "CompItem", "FootageItem", "FolderItem", "AVItem", "Item", "Layer", "AVLayer", "TextLayer", "ShapeLayer", "CameraLayer", "LightLayer", "ThreeDModelLayer",
    "Property", "PropertyGroup", "MaskPropertyGroup", "PropertyBase", "Project", "RenderQueueItem", "Application", "Panel", "SolidSource", "FileSource", "PlaceholderSource",
];

pub fn is_class(name: &str) -> bool {
    CLASSES.contains(&name)
}

fn class_name(name: &str) -> Option<&'static str> {
    CLASSES.iter().copied().find(|c| *c == name)
}

pub fn describe(h: &H) -> Option<String> {
    Some(
        match h {
            H::App => "[object Application]",
            H::Project => "[object Project]",
            H::Items(_) => "[object ItemCollection]",
            H::Folder(_) => "[object FolderItem]",
            H::Layers(_) => "[object LayerCollection]",
            H::Queue => "[object RenderQueue]",
            H::QueueItem(_) => "[object RenderQueueItem]",
            H::Window(_) => "[object Window]",
            H::Control(..) => "[object Control]",
            _ => return None,
        }
        .to_string(),
    )
}

fn enum_obj(pairs: &[(&str, V)]) -> V {
    obj(pairs.iter().map(|(k, v)| (*k, v.clone())).collect())
}

fn blend_enum() -> V {
    let mut pairs: Vec<(String, V)> = BlendMode::ALL.iter().map(|m| (m.label().to_ascii_uppercase().replace(' ', "_"), s(m.label()))).collect();
    pairs.push(("SILHOUETE_ALPHA".into(), s("Silhouette Alpha")));
    pairs.push(("SILHOUETE_LUMA".into(), s("Silhouette Luma")));
    V::Obj(Rc::new(RefCell::new(Obj { props: pairs.into_iter().map(|(k, v)| (Rc::from(k.as_str()), v)).collect() })))
}

pub const KIT_LINEAR: f64 = 6612.0;
pub const KIT_BEZIER: f64 = 6613.0;
pub const KIT_HOLD: f64 = 6614.0;

pub fn global(_it: &mut Interp, name: &str) -> Result<Option<V>, Flow> {
    if let Some(c) = class_name(name) {
        return Ok(Some(native(c, s(c), g_class_call)));
    }
    Ok(Some(match name {
        "app" => hv(H::App),
        "$" => dollar(),
        "alert" => native("alert", V::Undef, g_alert),
        "writeLn" => native("writeLn", V::Undef, g_writeln),
        "write" => native("write", V::Undef, g_write),
        "clearOutput" => native("clearOutput", V::Undef, g_noop),
        "confirm" => native("confirm", V::Undef, g_confirm),
        "prompt" => native("prompt", V::Undef, g_prompt),
        "isValid" => native("isValid", V::Undef, g_is_valid),
        "generateRandomNumber" => native("generateRandomNumber", V::Undef, g_random),
        "KeyframeInterpolationType" => enum_obj(&[("LINEAR", V::Num(KIT_LINEAR)), ("BEZIER", V::Num(KIT_BEZIER)), ("HOLD", V::Num(KIT_HOLD))]),
        "BlendingMode" => blend_enum(),
        "TrackMatteType" => enum_obj(&[("ALPHA", s("alpha")), ("ALPHA_INVERTED", s("alphaInverted")), ("LUMA", s("luma")), ("LUMA_INVERTED", s("lumaInverted")), ("NO_TRACK_MATTE", s("none"))]),
        "LightType" => enum_obj(&[("PARALLEL", s("Parallel")), ("SPOT", s("Spot")), ("POINT", s("Point")), ("AMBIENT", s("Ambient")), ("ENVIRONMENT", s("Environment"))]),
        "ParagraphJustification" => enum_obj(&[
            ("LEFT_JUSTIFY", s("left")),
            ("CENTER_JUSTIFY", s("center")),
            ("RIGHT_JUSTIFY", s("right")),
            ("FULL_JUSTIFY_LASTLINE_LEFT", s("justifyLeft")),
            ("FULL_JUSTIFY_LASTLINE_CENTER", s("justifyCenter")),
            ("FULL_JUSTIFY_LASTLINE_RIGHT", s("justifyRight")),
            ("FULL_JUSTIFY_LASTLINE_FULL", s("justifyAll")),
            ("MULTIPLE_JUSTIFICATIONS", s("mixed")),
        ]),
        "PropertyValueType" => enum_obj(&[
            ("NO_VALUE", s("NO_VALUE")),
            ("ThreeD_SPATIAL", s("ThreeD_SPATIAL")),
            ("ThreeD", s("ThreeD")),
            ("TwoD_SPATIAL", s("TwoD_SPATIAL")),
            ("TwoD", s("TwoD")),
            ("OneD", s("OneD")),
            ("COLOR", s("COLOR")),
            ("CUSTOM_VALUE", s("CUSTOM_VALUE")),
            ("MARKER", s("MARKER")),
            ("LAYER_INDEX", s("LAYER_INDEX")),
            ("MASK_INDEX", s("MASK_INDEX")),
            ("SHAPE", s("SHAPE")),
            ("TEXT_DOCUMENT", s("TEXT_DOCUMENT")),
        ]),
        "PropertyType" => enum_obj(&[("PROPERTY", s("PROPERTY")), ("INDEXED_GROUP", s("INDEXED_GROUP")), ("NAMED_GROUP", s("NAMED_GROUP"))]),
        "ImportAsType" => enum_obj(&[("COMP", s("composition")), ("COMP_CROPPED_LAYERS", s("compositionLayerSizes")), ("FOOTAGE", s("footage")), ("PROJECT", s("footage"))]),
        "LayerQuality" => enum_obj(&[("BEST", s("best")), ("DRAFT", s("draft")), ("WIREFRAME", s("wireframe"))]),
        "AutoOrientType" => enum_obj(&[("NO_AUTO_ORIENT", s("off")), ("ALONG_PATH", s("alongPath")), ("CAMERA_OR_POINT_OF_INTEREST", s("towardsCamera"))]),
        "CloseOptions" => enum_obj(&[("DO_NOT_SAVE_CHANGES", V::Num(1.0)), ("PROMPT_TO_SAVE_CHANGES", V::Num(2.0)), ("SAVE_CHANGES", V::Num(3.0))]),
        "MaskMode" => enum_obj(&[("NONE", s("None")), ("ADD", s("Add")), ("SUBTRACT", s("Subtract")), ("INTERSECT", s("Intersect")), ("LIGHTEN", s("Lighten")), ("DARKEN", s("Darken")), ("DIFFERENCE", s("Difference"))]),
        "RQItemStatus" => enum_obj(&[("QUEUED", s("queued")), ("DONE", s("done")), ("RENDERING", s("rendering")), ("UNQUEUED", s("unqueued")), ("ERR_STOPPED", s("failed")), ("USER_STOPPED", s("stopped"))]),
        _ => return scriptui::global(name),
    }))
}

fn dollar() -> V {
    let file = FILE_NAME.with(|f| f.borrow().clone());
    obj(vec![
        ("writeln", native("writeln", V::Undef, g_writeln)),
        ("write", native("write", V::Undef, g_write)),
        ("sleep", native("sleep", V::Undef, g_noop)),
        ("gc", native("gc", V::Undef, g_noop)),
        ("os", s(std::env::consts::OS)),
        ("version", s(env!("CARGO_PKG_VERSION"))),
        ("level", V::Num(0.0)),
        ("fileName", s(&file)),
        ("engineName", s("formvfx")),
        ("global", V::Undef),
    ])
}

fn join_args(it: &mut Interp, a: &[V]) -> Result<String, Flow> {
    let mut parts = vec![];
    for x in a {
        parts.push(it.to_string(x)?);
    }
    Ok(parts.join(" "))
}

fn g_alert(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let t = join_args(it, &a[..a.len().min(1)])?;
    output(&t);
    output("\n");
    Ok(V::Undef)
}

fn g_writeln(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let t = join_args(it, &a)?;
    output(&t);
    output("\n");
    Ok(V::Undef)
}

fn g_write(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let t = join_args(it, &a)?;
    output(&t);
    Ok(V::Undef)
}

fn g_noop(_: &mut Interp, _: &V, _: Vec<V>) -> R {
    Ok(V::Undef)
}

fn g_confirm(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    if !scriptui::modal_active() {
        return Ok(V::Bool(true));
    }
    let msg = it.to_string(&arg(&a, 0))?;
    let title = match arg(&a, 2) {
        V::Undef | V::Null => "Confirm".to_string(),
        v => it.to_string(&v)?,
    };
    scriptui::ask(it, false, &title, &msg, "")
}

fn g_prompt(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    if !scriptui::modal_active() {
        return Ok(match arg(&a, 1) {
            V::Undef => V::Null,
            v => v,
        });
    }
    let msg = it.to_string(&arg(&a, 0))?;
    let default = match arg(&a, 1) {
        V::Undef | V::Null => String::new(),
        v => it.to_string(&v)?,
    };
    let title = match arg(&a, 2) {
        V::Undef | V::Null => "Script Prompt".to_string(),
        v => it.to_string(&v)?,
    };
    scriptui::ask(it, true, &title, &msg, &default)
}

fn g_is_valid(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let v = arg(&a, 0);
    Ok(V::Bool(match &v {
        V::Undef | V::Null => false,
        V::Host(h) => exists(it, h),
        _ => true,
    }))
}

fn exists(it: &Interp, h: &H) -> bool {
    let p = pj(it);
    match h {
        H::Comp(id) | H::Footage(id) => p.item(*id).is_some(),
        H::Folder(id) => *id == 0 || p.item(*id).is_some(),
        H::Layer(c, l) => p.comp(*c).and_then(|x| x.layer(*l)).is_some(),
        H::Group(c, l, u) => p.comp(*c).and_then(|x| x.layer(*l)).and_then(|x| x.props.find_group(*u)).is_some(),
        H::Prop(c, l, u) => p.comp(*c).and_then(|x| x.layer(*l)).and_then(|x| x.props.find(*u)).is_some(),
        H::Window(w) => scriptui::exists(*w),
        _ => true,
    }
}

fn g_random(it: &mut Interp, _: &V, _: Vec<V>) -> R {
    let _ = it;
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    Ok(V::Num(super::noise::unit(super::noise::hash(&[n as u64, 7]))))
}

fn g_class_call(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let name = plain_string(this);
    construct(it, &name, a)
}

pub fn static_get(class: &str, key: &str) -> Option<V> {
    match (class, key) {
        ("Date", "now") => Some(native("now", V::Undef, g_date_now)),
        ("Folder", "desktop") => Some(folder_obj(&home().join("Desktop").to_string_lossy())),
        ("Folder", "myDocuments") => Some(folder_obj(&home().join("Documents").to_string_lossy())),
        ("Folder", "temp") => Some(folder_obj(&std::env::temp_dir().to_string_lossy())),
        ("Folder", "userData") | ("Folder", "appData") => Some(folder_obj(&crate::render::diskcache::data_dir().to_string_lossy())),
        ("Folder", "current") => Some(folder_obj(&std::env::current_dir().unwrap_or_default().to_string_lossy())),
        ("Folder", "selectDialog") | ("File", "openDialog") | ("File", "saveDialog") => Some(native("dialog", V::Undef, g_null)),
        ("File", "decode") | ("File", "encode") => Some(native("decode", V::Undef, g_identity)),
        ("File", "fs") => Some(s(if cfg!(windows) { "Windows" } else if cfg!(target_os = "macos") { "Macintosh" } else { "Unix" })),
        _ => None,
    }
}

fn g_null(_: &mut Interp, _: &V, _: Vec<V>) -> R {
    Ok(V::Null)
}

fn g_identity(_: &mut Interp, _: &V, a: Vec<V>) -> R {
    Ok(arg(&a, 0))
}

fn home() -> std::path::PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from).unwrap_or_default()
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

fn civil(ms: f64) -> [i64; 8] {
    let total = ms.floor() as i64;
    let days = total.div_euclid(86_400_000);
    let rem = total.rem_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    let weekday = (days + 4).rem_euclid(7);
    [y, m - 1, d, rem / 3_600_000, rem / 60_000 % 60, rem / 1000 % 60, rem % 1000, weekday]
}

fn date_ms(this: &V) -> f64 {
    match field(this, "_ms") {
        V::Num(n) => n,
        _ => 0.0,
    }
}

fn date_part(this: &V, i: usize) -> R {
    Ok(V::Num(civil(date_ms(this))[i] as f64))
}

fn dt_time(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    Ok(V::Num(date_ms(this)))
}
fn dt_year(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    date_part(this, 0)
}
fn dt_month(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    date_part(this, 1)
}
fn dt_date(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    date_part(this, 2)
}
fn dt_hours(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    date_part(this, 3)
}
fn dt_minutes(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    date_part(this, 4)
}
fn dt_seconds(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    date_part(this, 5)
}
fn dt_ms(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    date_part(this, 6)
}
fn dt_day(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    date_part(this, 7)
}
fn dt_string(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    let c = civil(date_ms(this));
    Ok(s(&format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", c[0], c[1] + 1, c[2], c[3], c[4], c[5], c[6])))
}

fn date_obj(ms: f64) -> V {
    let o = obj(vec![("_ms", V::Num(ms))]);
    for (n, f) in [
        ("getTime", dt_time as NativeImpl),
        ("valueOf", dt_time),
        ("getFullYear", dt_year),
        ("getUTCFullYear", dt_year),
        ("getMonth", dt_month),
        ("getUTCMonth", dt_month),
        ("getDate", dt_date),
        ("getUTCDate", dt_date),
        ("getHours", dt_hours),
        ("getUTCHours", dt_hours),
        ("getMinutes", dt_minutes),
        ("getUTCMinutes", dt_minutes),
        ("getSeconds", dt_seconds),
        ("getUTCSeconds", dt_seconds),
        ("getMilliseconds", dt_ms),
        ("getDay", dt_day),
        ("toString", dt_string),
        ("toISOString", dt_string),
        ("toUTCString", dt_string),
        ("toLocaleString", dt_string),
        ("toJSON", dt_string),
    ] {
        add_method(&o, n, f);
    }
    o
}

fn g_date_now(_: &mut Interp, _: &V, _: Vec<V>) -> R {
    Ok(V::Num(now_ms()))
}

pub fn construct(it: &mut Interp, name: &str, a: Vec<V>) -> R {
    match name {
        "Date" => {
            let ms = match (arg(&a, 0), a.len()) {
                (V::Undef, _) => now_ms(),
                (v, 1) => it.to_number(&v)?,
                (_, _) => {
                    let mut f = [1970.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
                    for (i, x) in a.iter().take(7).enumerate() {
                        f[i] = it.to_number(x)?;
                    }
                    let (y, mo) = (f[0] as i64 + (f[1] as i64).div_euclid(12), (f[1] as i64).rem_euclid(12) + 1);
                    let yy = if mo <= 2 { y - 1 } else { y };
                    let era = yy.div_euclid(400);
                    let yoe = yy - era * 400;
                    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
                    let doy = (153 * mp + 2) / 5 + f[2] as i64 - 1;
                    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
                    let days = era * 146_097 + doe - 719_468;
                    days as f64 * 86_400_000.0 + f[3] * 3_600_000.0 + f[4] * 60_000.0 + f[5] * 1000.0 + f[6]
                }
            };
            Ok(date_obj(ms))
        }
        "File" => {
            let path = path_arg(it, &arg(&a, 0))?;
            Ok(file_obj(&path))
        }
        "Folder" => {
            let path = path_arg(it, &arg(&a, 0))?;
            Ok(folder_obj(&path))
        }
        "ImportOptions" => {
            let f = arg(&a, 0);
            let o = obj(vec![("file", f), ("importAs", s("footage")), ("sequence", V::Bool(false)), ("forceAlphabetical", V::Bool(false)), ("rangeStart", V::Num(0.0)), ("rangeEnd", V::Num(0.0))]);
            add_method(&o, "canImportAs", g_true);
            Ok(o)
        }
        "KeyframeEase" => {
            let sp = it.to_number(&arg(&a, 0))?;
            let inf = match arg(&a, 1) {
                V::Undef => crate::model::key::DEFAULT_INFLUENCE,
                v => it.to_number(&v)?,
            };
            Ok(obj(vec![("speed", V::Num(sp)), ("influence", V::Num(inf))]))
        }
        "TextDocument" => {
            let t = match arg(&a, 0) {
                V::Undef => String::new(),
                v => it.to_string(&v)?,
            };
            let doc = TextDoc { text: t, ..TextDoc::default() };
            Ok(text_doc_v(&doc))
        }
        "Shape" => Ok(obj(vec![("vertices", arr(vec![])), ("inTangents", arr(vec![])), ("outTangents", arr(vec![])), ("closed", V::Bool(true)), ("featherSegLocs", arr(vec![])), ("featherRelSegLocs", arr(vec![])), ("featherRadii", arr(vec![]))])),
        "MarkerValue" => {
            let c = match arg(&a, 0) {
                V::Undef => String::new(),
                v => it.to_string(&v)?,
            };
            Ok(obj(vec![
                ("comment", s(&c)),
                ("duration", V::Num(0.0)),
                ("chapter", s("")),
                ("url", s("")),
                ("frameTarget", s("")),
                ("cuePointName", s("")),
                ("eventCuePoint", V::Bool(false)),
                ("label", V::Num(0.0)),
                ("protectedRegion", V::Bool(false)),
            ]))
        }
        "Window" | "Panel" => scriptui::construct(it, name, a),
        other => Err(type_err(format!("`{other}` objects are made by formvfx, not with `new`"))),
    }
}

fn g_true(_: &mut Interp, _: &V, _: Vec<V>) -> R {
    Ok(V::Bool(true))
}

fn add_method(o: &V, name: &'static str, f: NativeImpl) {
    if let V::Obj(ob) = o {
        let m = native(name, o.clone(), f);
        ob.borrow_mut().set(name, m);
    }
}

fn path_arg(it: &mut Interp, v: &V) -> Result<String, Flow> {
    Ok(match v {
        V::Undef | V::Null => String::new(),
        V::Obj(o) => {
            let f = o.borrow().get("fsName").cloned();
            match f {
                Some(x) => it.to_string(&x)?,
                None => it.to_string(v)?,
            }
        }
        _ => {
            let t = it.to_string(v)?;
            expand(&t)
        }
    })
}

fn expand(t: &str) -> String {
    let t = t.strip_prefix("file://").unwrap_or(t);
    if let Some(rest) = t.strip_prefix("~/") {
        return home().join(rest).to_string_lossy().into_owned();
    }
    if cfg!(windows) && t.len() > 3 && t.starts_with('/') && t.as_bytes()[2] == b'/' {
        let drive = t.as_bytes()[1] as char;
        return format!("{}:{}", drive.to_ascii_uppercase(), t[2..].replace('/', "\\"));
    }
    t.to_string()
}

fn file_parts(path: &str) -> (String, String) {
    let p = std::path::Path::new(path);
    (p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string(), p.parent().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default())
}

pub fn file_obj(path: &str) -> V {
    let (name, parent) = file_parts(path);
    let o = obj(vec![
        ("fsName", s(path)),
        ("fullName", s(path)),
        ("absoluteURI", s(path)),
        ("name", s(&name)),
        ("displayName", s(&name)),
        ("path", s(&parent)),
        ("encoding", s("UTF-8")),
        ("lineFeed", s("Unix")),
        ("eof", V::Bool(false)),
        ("error", s("")),
        ("_mode", s("")),
        ("_buf", s("")),
        ("_pos", V::Num(0.0)),
    ]);
    for (n, f) in [("open", f_open as NativeImpl), ("close", f_close), ("read", f_read), ("readln", f_readln), ("write", f_write), ("writeln", f_writeln), ("remove", f_remove), ("copy", f_copy), ("rename", f_rename), ("toString", f_tostring), ("execute", g_true), ("resolve", f_self)] {
        add_method(&o, n, f);
    }
    if let V::Obj(ob) = &o {
        ob.borrow_mut().set("parent", s(&parent));
    }
    o
}

pub fn folder_obj(path: &str) -> V {
    let (name, parent) = file_parts(path);
    let o = obj(vec![("fsName", s(path)), ("fullName", s(path)), ("absoluteURI", s(path)), ("name", s(&name)), ("displayName", s(&name)), ("path", s(&parent)), ("parent", s(&parent))]);
    for (n, f) in [("create", d_create as NativeImpl), ("getFiles", d_files), ("remove", f_remove), ("toString", f_tostring), ("execute", g_true)] {
        add_method(&o, n, f);
    }
    o
}

fn this_path(it: &mut Interp, this: &V) -> Result<String, Flow> {
    path_arg(it, this)
}

fn field(this: &V, k: &str) -> V {
    match this {
        V::Obj(o) => o.borrow().get(k).cloned().unwrap_or(V::Undef),
        _ => V::Undef,
    }
}

fn set_field(this: &V, k: &str, v: V) {
    if let V::Obj(o) = this {
        o.borrow_mut().set(k, v);
    }
}

fn f_self(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    Ok(this.clone())
}

fn f_tostring(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    Ok(s(&this_path(it, this)?))
}

fn f_open(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let mode = match arg(&a, 0) {
        V::Undef => "r".to_string(),
        v => it.to_string(&v)?,
    };
    let path = this_path(it, this)?;
    set_field(this, "_mode", s(&mode));
    set_field(this, "_pos", V::Num(0.0));
    match mode.as_str() {
        "r" | "e" => match std::fs::read_to_string(&path) {
            Ok(t) => {
                set_field(this, "_buf", s(&t));
                set_field(this, "eof", V::Bool(t.is_empty()));
                Ok(V::Bool(true))
            }
            Err(e) => {
                set_field(this, "error", s(&e.to_string()));
                Ok(V::Bool(false))
            }
        },
        "a" => {
            let t = std::fs::read_to_string(&path).unwrap_or_default();
            set_field(this, "_buf", s(&t));
            Ok(V::Bool(true))
        }
        _ => {
            set_field(this, "_buf", s(""));
            Ok(V::Bool(true))
        }
    }
}

fn f_close(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let mode = plain_string(&field(this, "_mode"));
    if matches!(mode.as_str(), "w" | "a" | "e") {
        let path = this_path(it, this)?;
        let buf = plain_string(&field(this, "_buf"));
        if let Err(e) = std::fs::write(&path, buf) {
            set_field(this, "error", s(&e.to_string()));
            return Ok(V::Bool(false));
        }
    }
    set_field(this, "_mode", s(""));
    Ok(V::Bool(true))
}

fn f_read(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    let buf = plain_string(&field(this, "_buf"));
    let pos = match field(this, "_pos") {
        V::Num(n) => n as usize,
        _ => 0,
    };
    let rest: String = buf.chars().skip(pos).collect();
    set_field(this, "_pos", V::Num(buf.chars().count() as f64));
    set_field(this, "eof", V::Bool(true));
    Ok(s(&rest))
}

fn f_readln(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    let buf = plain_string(&field(this, "_buf"));
    let chars: Vec<char> = buf.chars().collect();
    let mut pos = match field(this, "_pos") {
        V::Num(n) => n as usize,
        _ => 0,
    };
    let start = pos;
    while pos < chars.len() && chars[pos] != '\n' {
        pos += 1;
    }
    let line: String = chars[start.min(chars.len())..pos.min(chars.len())].iter().collect();
    let next = (pos + 1).min(chars.len());
    set_field(this, "_pos", V::Num(next as f64));
    set_field(this, "eof", V::Bool(next >= chars.len()));
    Ok(s(line.trim_end_matches('\r')))
}

fn f_write(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let t = join_args(it, &a)?;
    let mut buf = plain_string(&field(this, "_buf"));
    buf.push_str(&t);
    set_field(this, "_buf", s(&buf));
    Ok(V::Bool(true))
}

fn f_writeln(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let t = join_args(it, &a)?;
    let mut buf = plain_string(&field(this, "_buf"));
    buf.push_str(&t);
    buf.push('\n');
    set_field(this, "_buf", s(&buf));
    Ok(V::Bool(true))
}

fn f_remove(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let path = this_path(it, this)?;
    let p = std::path::Path::new(&path);
    let ok = if p.is_dir() { std::fs::remove_dir(p).is_ok() } else { std::fs::remove_file(p).is_ok() };
    Ok(V::Bool(ok))
}

fn f_copy(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let from = this_path(it, this)?;
    let to = path_arg(it, &arg(&a, 0))?;
    Ok(V::Bool(std::fs::copy(from, to).is_ok()))
}

fn f_rename(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let from = this_path(it, this)?;
    let name = it.to_string(&arg(&a, 0))?;
    let to = std::path::Path::new(&from).with_file_name(name);
    Ok(V::Bool(std::fs::rename(&from, to).is_ok()))
}

fn d_create(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let path = this_path(it, this)?;
    Ok(V::Bool(std::fs::create_dir_all(path).is_ok()))
}

fn d_files(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let path = this_path(it, this)?;
    let mask = match arg(&a, 0) {
        V::Undef => String::new(),
        v => it.to_string(&v)?,
    };
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(&path) {
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for ent in entries {
            let p = ent.path();
            let name = ent.file_name().to_string_lossy().into_owned();
            if !mask.is_empty() && !glob(&mask, &name) {
                continue;
            }
            let ps = p.to_string_lossy().into_owned();
            out.push(if p.is_dir() { folder_obj(&ps) } else { file_obj(&ps) });
        }
    }
    Ok(arr(out))
}

fn glob(mask: &str, name: &str) -> bool {
    mask.split([';', ',']).any(|m| {
        let m = m.trim().to_ascii_lowercase();
        let n = name.to_ascii_lowercase();
        match m.split_once('*') {
            Some((a, b)) => n.starts_with(a) && n.ends_with(b) && n.len() >= a.len() + b.len(),
            None => n == m,
        }
    })
}

fn m(h: &Rc<H>, name: &'static str, f: NativeImpl) -> V {
    native(name, V::Host(h.clone()), f)
}

fn this_h(this: &V) -> Result<Rc<H>, Flow> {
    match this {
        V::Host(h) => Ok(h.clone()),
        _ => Err(type_err("this method needs its object")),
    }
}

fn item_v(p: &Project, id: u64) -> V {
    match p.item(id).map(|i| &i.kind) {
        Some(ItemKind::Comp(_)) => hv(H::Comp(id)),
        Some(ItemKind::Folder) => hv(H::Folder(id)),
        Some(_) => hv(H::Footage(id)),
        None => V::Null,
    }
}

fn item_list(p: &Project, folder: Option<u64>) -> Vec<u64> {
    match folder {
        None => p.items.iter().map(|i| i.id).collect(),
        Some(0) => p.items.iter().filter(|i| i.parent.is_none()).map(|i| i.id).collect(),
        Some(f) => p.items.iter().filter(|i| i.parent == Some(f)).map(|i| i.id).collect(),
    }
}

fn index_arg(it: &mut Interp, a: &[V], n: usize, what: &str) -> Result<usize, Flow> {
    let i = it.to_number(&arg(a, 0))?;
    if i < 1.0 || i.fract() != 0.0 || i as usize > n {
        return Err(throw("RangeError", format!("{what} index {} is out of range (1 to {n})", num_to_string(i))));
    }
    Ok(i as usize - 1)
}

fn num_key(key: &str) -> Option<usize> {
    key.parse::<usize>().ok()
}

pub fn get(it: &mut Interp, h: &Rc<H>, key: &str) -> Result<Option<V>, Flow> {
    sync(it);
    match &**h {
        H::App => app_get(it, h, key),
        H::Project => project_get(it, h, key),
        H::Items(f) => items_get(it, h, *f, key),
        H::Folder(id) => folder_get(it, h, *id, key),
        H::Comp(c) => comp_get(it, h, *c, key),
        H::Footage(id) => footage_get(it, h, *id, key),
        H::Layers(c) => layers_get(it, h, *c, key),
        H::Layer(c, l) => layer_get(it, h, *c, *l, key),
        H::Group(c, l, u) => group_get(it, h, *c, *l, *u, key),
        H::Prop(c, l, u) => prop_get(it, h, *c, *l, *u, key),
        H::Queue => queue_get(it, h, key),
        H::QueueItem(id) => queue_item_get(it, h, *id, key),
        H::Window(_) | H::Control(..) => scriptui::get(it, h, key),
        _ => Ok(None),
    }
}

pub fn set(it: &mut Interp, h: &Rc<H>, key: &str, v: V) -> Result<(), Flow> {
    sync(it);
    match &**h {
        H::Comp(c) => {
            if item_set(it, *c, key, &v)? {
                return Ok(());
            }
            comp_set(it, *c, key, v)
        }
        H::Footage(id) | H::Folder(id) => {
            if item_set(it, *id, key, &v)? {
                return Ok(());
            }
            Err(type_err(format!("`{key}` cannot be set on this item")))
        }
        H::Layer(c, l) => layer_set(it, *c, *l, key, v),
        H::Group(c, l, u) => group_set(it, *c, *l, *u, key, v),
        H::Prop(c, l, u) => prop_set(it, *c, *l, *u, key, v),
        H::Project => project_set(it, key, v),
        H::Window(_) | H::Control(..) => scriptui::set(it, h, key, v),
        H::App | H::Items(_) | H::Layers(_) | H::Queue | H::QueueItem(_) => Ok(()),
        _ => Err(type_err(format!("`{key}` is read-only"))),
    }
}

pub fn instance_of(it: &mut Interp, h: &Rc<H>, class: &str) -> bool {
    let p = pj(it);
    let layer_kind = |c: u64, l: u64| p.comp(c).and_then(|x| x.layer(l)).map(|x| x.kind);
    match (&**h, class) {
        (H::App, "Application") | (H::Project, "Project") | (H::QueueItem(_), "RenderQueueItem") => true,
        (H::Comp(_), "CompItem" | "AVItem" | "Item") => true,
        (H::Footage(_), "FootageItem" | "AVItem" | "Item") => true,
        (H::Folder(_), "FolderItem" | "Item") => true,
        (H::Layer(..), "Layer") => true,
        (H::Layer(c, l), "AVLayer") => !matches!(layer_kind(*c, *l), Some(LayerKind::Camera | LayerKind::Light) | None),
        (H::Layer(c, l), "TextLayer") => layer_kind(*c, *l) == Some(LayerKind::Text),
        (H::Layer(c, l), "ShapeLayer") => layer_kind(*c, *l) == Some(LayerKind::Shape),
        (H::Layer(c, l), "CameraLayer") => layer_kind(*c, *l) == Some(LayerKind::Camera),
        (H::Layer(c, l), "LightLayer") => layer_kind(*c, *l) == Some(LayerKind::Light),
        (H::Layer(c, l), "ThreeDModelLayer") => layer_kind(*c, *l) == Some(LayerKind::Model),
        (H::Prop(..), "Property" | "PropertyBase") => true,
        (H::Group(..), "PropertyGroup" | "PropertyBase") => true,
        (H::Group(c, l, u), "MaskPropertyGroup") => p.comp(*c).and_then(|x| x.layer(*l)).and_then(|x| x.props.find_group(*u)).is_some_and(|g| matches!(g.kind, GroupKind::Mask { .. })),
        (H::Window(_), "Window") => true,
        (H::Window(_), "Panel") => scriptui::is_panel(h),
        _ => false,
    }
}

fn app_get(_it: &mut Interp, h: &Rc<H>, key: &str) -> Result<Option<V>, Flow> {
    Ok(Some(match key {
        "project" => hv(H::Project),
        "version" => s("26.5"),
        "buildName" => s(concat!("formvfx ", env!("CARGO_PKG_VERSION"))),
        "buildNumber" => V::Num(1.0),
        "isoLanguage" => s("en_US"),
        "isWatchFolder" | "isRenderEngine" | "isUISuppressed" => V::Bool(false),
        "memoryInUse" => V::Num(0.0),
        "beginUndoGroup" => m(h, "beginUndoGroup", app_begin_undo),
        "endUndoGroup" => m(h, "endUndoGroup", g_noop),
        "beginSuppressDialogs" | "endSuppressDialogs" | "activate" | "purge" | "pauseWatchFolder" | "endWatchFolder" | "cancelTask" | "quit" | "setMemoryUsageLimits" | "setMultiFrameRenderingConfig" => m(h, "noop", g_noop),
        "newProject" => m(h, "newProject", app_new_project),
        "open" => m(h, "open", app_open),
        "effects" => effects_list(),
        "settings" => settings_obj(),
        "executeCommand" => m(h, "executeCommand", app_execute_command),
        "findMenuCommandId" => m(h, "findMenuCommandId", app_find_menu),
        "scheduleTask" => m(h, "scheduleTask", app_schedule),
        "exitAfterLaunchAndEval" | "exitCode" | "saveProjectOnCrash" | "onError" => V::Undef,
        _ => return Ok(None),
    }))
}

fn app_begin_undo(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let name = match arg(&a, 0) {
        V::Undef => "Script".to_string(),
        v => it.to_string(&v)?,
    };
    UNDO_NAME.with(|u| {
        let mut u = u.borrow_mut();
        if u.is_none() {
            *u = Some(name);
        }
    });
    Ok(V::Undef)
}

fn app_new_project(it: &mut Interp, _: &V, _: Vec<V>) -> R {
    exec(it, "file.newProject", json!({}))?;
    Ok(hv(H::Project))
}

fn app_open(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let path = path_arg(it, &arg(&a, 0))?;
    if path.is_empty() {
        return Ok(V::Null);
    }
    exec(it, "file.open", json!({"path": path}))?;
    Ok(hv(H::Project))
}

fn app_execute_command(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let v = arg(&a, 0);
    if let V::Str(id) = &v
        && crate::engine::registry::find(id).is_some()
    {
        let params = match arg(&a, 1) {
            V::Undef => json!({}),
            p => to_json(it, &p, 0)?,
        };
        let r = exec(it, id, params)?;
        return json_v(&r);
    }
    Err(throw("Error", "app.executeCommand takes a formvfx command id (command.list lists them) and its parameters"))
}

fn app_find_menu(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let label = it.to_string(&arg(&a, 0))?;
    let found = crate::engine::registry::all().iter().find(|c| c.label.trim_end_matches("...") == label.trim_end_matches("...")).map(|c| c.id);
    Ok(match found {
        Some(id) => s(id),
        None => V::Num(0.0),
    })
}

fn app_schedule(_: &mut Interp, _: &V, _: Vec<V>) -> R {
    Err(throw("Error", "app.scheduleTask is not in formvfx yet"))
}

fn effects_list() -> V {
    arr(crate::effects::all()
        .iter()
        .filter(|e| crate::effects::listed(e))
        .map(|e| obj(vec![("displayName", s(e.name)), ("matchName", s(e.id)), ("category", s(e.category)), ("version", s("1.0"))]))
        .collect())
}

fn settings_obj() -> V {
    let o = obj(vec![]);
    add_method(&o, "getSetting", st_get);
    add_method(&o, "saveSetting", st_save);
    add_method(&o, "haveSetting", st_have);
    o
}

fn setting_key(it: &mut Interp, a: &[V]) -> Result<String, Flow> {
    let sec = it.to_string(&arg(a, 0))?;
    let k = it.to_string(&arg(a, 1))?;
    Ok(format!("{sec}\u{1f}{k}"))
}

fn st_get(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let k = setting_key(it, &a)?;
    let stored = SETTINGS.with(|s| s.borrow().get(&k).cloned()).or_else(|| load_setting(&k));
    Ok(stored.map(|t| s(&t)).unwrap_or(s("")))
}

fn st_have(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let k = setting_key(it, &a)?;
    Ok(V::Bool(SETTINGS.with(|s| s.borrow().contains_key(&k)) || load_setting(&k).is_some()))
}

fn st_save(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let k = setting_key(it, &a)?;
    let v = it.to_string(&arg(&a, 2))?;
    SETTINGS.with(|s| s.borrow_mut().insert(k.clone(), v.clone()));
    store_setting(&k, &v);
    Ok(V::Undef)
}

fn settings_path() -> std::path::PathBuf {
    crate::render::diskcache::data_dir().join("script-settings.json")
}

fn load_setting(k: &str) -> Option<String> {
    let t = std::fs::read_to_string(settings_path()).ok()?;
    let j: Json = serde_json::from_str(&t).ok()?;
    j.get(k).and_then(Json::as_str).map(str::to_string)
}

fn store_setting(k: &str, v: &str) {
    let path = settings_path();
    let mut j: Json = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}));
    if let Some(o) = j.as_object_mut() {
        o.insert(k.to_string(), json!(v));
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, j.to_string());
}

pub fn json_v(j: &Json) -> R {
    Ok(match j {
        Json::Null => V::Null,
        Json::Bool(b) => V::Bool(*b),
        Json::Number(n) => V::Num(n.as_f64().unwrap_or(0.0)),
        Json::String(t) => s(t),
        Json::Array(a) => {
            let mut out = vec![];
            for x in a {
                out.push(json_v(x)?);
            }
            arr(out)
        }
        Json::Object(o) => {
            let mut props = vec![];
            for (k, x) in o {
                props.push((Rc::from(k.as_str()), json_v(x)?));
            }
            V::Obj(Rc::new(RefCell::new(Obj { props })))
        }
    })
}

fn project_get(it: &mut Interp, h: &Rc<H>, key: &str) -> Result<Option<V>, Flow> {
    let p = pj(it);
    let e = eng()?;
    Ok(Some(match key {
        "items" => hv(H::Items(None)),
        "numItems" => V::Num(p.items.len() as f64),
        "item" => m(h, "item", project_item),
        "itemByID" => m(h, "itemByID", project_item_by_id),
        "layerByID" => m(h, "layerByID", project_layer_by_id),
        "activeItem" => match e.state.active_comp.filter(|c| p.comp(*c).is_some()) {
            Some(c) => hv(H::Comp(c)),
            None => match e.state.project_selection.first() {
                Some(id) => item_v(p, *id),
                None => V::Null,
            },
        },
        "selection" => arr(e.state.project_selection.iter().filter(|id| p.item(**id).is_some()).map(|id| item_v(p, *id)).collect()),
        "rootFolder" => hv(H::Folder(0)),
        "file" => match &e.path {
            Some(path) => file_obj(path),
            None => V::Null,
        },
        "dirty" => V::Bool(e.is_dirty()),
        "bitsPerChannel" => V::Num(p.settings.bit_depth as f64),
        "workingSpace" => s(&p.settings.working_space),
        "linearBlending" => V::Bool(p.settings.blend_linear),
        "linearizeWorkingSpace" => V::Bool(p.settings.linearize),
        "renderQueue" => hv(H::Queue),
        "save" => m(h, "save", project_save),
        "saveWithDialog" | "importFileWithDialog" | "importPlaceholder" => m(h, "dialog", g_null),
        "close" => m(h, "close", project_close),
        "importFile" => m(h, "importFile", project_import),
        "consolidateFootage" => m(h, "consolidateFootage", project_consolidate),
        "removeUnusedFootage" => m(h, "removeUnusedFootage", project_remove_unused),
        "reduceProject" => m(h, "reduceProject", project_reduce),
        "autoFixExpressions" | "showWindow" => m(h, "noop", g_noop),
        "usedFonts" => arr(vec![]),
        "toolType" => s(""),
        _ => return Ok(None),
    }))
}

fn project_set(it: &mut Interp, key: &str, v: V) -> Result<(), Flow> {
    match key {
        "bitsPerChannel" => {
            let n = it.to_number(&v)?;
            exec(it, "file.projectSettings", json!({"bitDepth": n as u64}))?;
        }
        "workingSpace" => {
            let t = it.to_string(&v)?;
            exec(it, "file.projectSettings", json!({"workingSpace": t}))?;
        }
        "linearBlending" => {
            exec(it, "file.projectSettings", json!({"blendLinear": truthy(&v)}))?;
        }
        "linearizeWorkingSpace" => {
            exec(it, "file.projectSettings", json!({"linearize": truthy(&v)}))?;
        }
        _ => {}
    }
    Ok(())
}

fn project_item(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let ids = item_list(pj(it), None);
    let i = index_arg(it, &a, ids.len(), "item")?;
    Ok(item_v(pj(it), ids[i]))
}

fn project_item_by_id(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let id = it.to_number(&arg(&a, 0))? as u64;
    Ok(item_v(pj(it), id))
}

fn project_layer_by_id(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let id = it.to_number(&arg(&a, 0))? as u64;
    Ok(match pj(it).comp_of_layer(id) {
        Some(c) => hv(H::Layer(c, id)),
        None => V::Null,
    })
}

fn project_save(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let path = path_arg(it, &arg(&a, 0))?;
    let params = if path.is_empty() { json!({}) } else { json!({"path": path}) };
    exec(it, "file.save", params)?;
    Ok(V::Bool(true))
}

fn project_close(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    if it.to_number(&arg(&a, 0)).unwrap_or(1.0) == 3.0 && eng()?.path.is_some() {
        exec(it, "file.save", json!({}))?;
    }
    exec(it, "file.newProject", json!({}))?;
    Ok(V::Bool(true))
}

fn project_import(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let io = arg(&a, 0);
    let (file, as_type, seq) = match &io {
        V::Obj(o) if o.borrow().get("fsName").is_none() => {
            let b = o.borrow();
            (b.get("file").cloned().unwrap_or(V::Undef), b.get("importAs").cloned().unwrap_or(V::Undef), b.get("sequence").cloned().unwrap_or(V::Undef))
        }
        other => (other.clone(), V::Undef, V::Undef),
    };
    let path = path_arg(it, &file)?;
    if path.is_empty() {
        return Err(throw("Error", "importFile needs an ImportOptions or a File"));
    }
    let before: Vec<u64> = pj(it).items.iter().map(|i| i.id).collect();
    let mut params = json!({"paths": [path]});
    if let V::Str(t) = &as_type {
        params["importAs"] = json!(&**t);
    }
    if !matches!(seq, V::Undef) {
        params["sequence"] = json!(truthy(&seq));
    }
    exec(it, "file.import", params)?;
    let p = pj(it);
    let added: Vec<u64> = p.items.iter().map(|i| i.id).filter(|id| !before.contains(id)).collect();
    let top = added.iter().copied().find(|id| p.item(*id).is_some_and(|i| i.parent.is_none_or(|par| before.contains(&par)))).or(added.first().copied());
    Ok(top.map(|id| item_v(p, id)).unwrap_or(V::Null))
}

fn project_consolidate(it: &mut Interp, _: &V, _: Vec<V>) -> R {
    let n = pj(it).items.len();
    exec(it, "file.consolidateFootage", json!({}))?;
    Ok(V::Num(n.saturating_sub(pj(it).items.len()) as f64))
}

fn project_remove_unused(it: &mut Interp, _: &V, _: Vec<V>) -> R {
    let n = pj(it).items.len();
    exec(it, "file.removeUnusedFootage", json!({}))?;
    Ok(V::Num(n.saturating_sub(pj(it).items.len()) as f64))
}

fn project_reduce(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let list = match arg(&a, 0) {
        V::Undef => vec![],
        v => it.iterate(&v)?,
    };
    let ids: Vec<u64> = list.iter().filter_map(|x| match x {
        V::Host(h) => match &**h {
            H::Comp(id) | H::Footage(id) | H::Folder(id) => Some(*id),
            _ => None,
        },
        _ => None,
    }).collect();
    if !ids.is_empty() {
        exec(it, "project.select", json!({"items": ids}))?;
    }
    let n = pj(it).items.len();
    exec(it, "file.reduceProject", json!({}))?;
    Ok(V::Num(n.saturating_sub(pj(it).items.len()) as f64))
}

fn items_get(it: &mut Interp, h: &Rc<H>, folder: Option<u64>, key: &str) -> Result<Option<V>, Flow> {
    let p = pj(it);
    let ids = item_list(p, folder);
    if let Some(i) = num_key(key) {
        return Ok(Some(ids.get(i.wrapping_sub(1)).map(|id| item_v(p, *id)).unwrap_or(V::Undef)));
    }
    Ok(Some(match key {
        "length" => V::Num(ids.len() as f64),
        "addComp" => m(h, "addComp", items_add_comp),
        "addFolder" => m(h, "addFolder", items_add_folder),
        _ => return Ok(None),
    }))
}

fn folder_of(this: &V) -> Option<u64> {
    match this {
        V::Host(h) => match &**h {
            H::Items(Some(f)) | H::Folder(f) if *f != 0 => Some(*f),
            _ => None,
        },
        _ => None,
    }
}

fn items_add_comp(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let name = it.to_string(&arg(&a, 0))?;
    let w = it.to_number(&arg(&a, 1))?;
    let hgt = it.to_number(&arg(&a, 2))?;
    let pa = match arg(&a, 3) {
        V::Undef => 1.0,
        v => it.to_number(&v)?,
    };
    let dur = it.to_number(&arg(&a, 4))?;
    let fps = it.to_number(&arg(&a, 5))?;
    if !(w >= 4.0 && hgt >= 4.0 && dur > 0.0 && fps > 0.0) {
        return Err(throw("Error", "addComp(name, width, height, pixelAspect, duration, frameRate) needs a size of at least 4 px, a duration and a frame rate"));
    }
    let r = exec(it, "comp.new", json!({"name": name, "width": w.round(), "height": hgt.round(), "pixelAspect": pa, "duration": dur, "frameRate": fps, "open": false}))?;
    let id = r.get("comp").and_then(Json::as_u64).ok_or_else(|| throw("Error", "the composition was not made"))?;
    if let Some(f) = folder_of(this) {
        exec(it, "project.move", json!({"items": [id], "folder": f}))?;
    }
    Ok(hv(H::Comp(id)))
}

fn items_add_folder(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let name = it.to_string(&arg(&a, 0))?;
    let r = exec(it, "project.newFolder", json!({"name": name}))?;
    let id = r.get("item").and_then(Json::as_u64).ok_or_else(|| throw("Error", "the folder was not made"))?;
    if let Some(f) = folder_of(this) {
        exec(it, "project.move", json!({"items": [id], "folder": f}))?;
    }
    Ok(hv(H::Folder(id)))
}

fn item_common(it: &mut Interp, h: &Rc<H>, id: u64, key: &str) -> Result<Option<V>, Flow> {
    let p = pj(it);
    let Some(item) = p.item(id) else {
        return Err(throw("Error", "this item no longer exists"));
    };
    let e = eng()?;
    Ok(Some(match key {
        "name" => s(&item.name),
        "id" => V::Num(id as f64),
        "typeName" => s(item.type_name()),
        "comment" => s(&item.comment),
        "label" => V::Num(item.label as f64),
        "parentFolder" => hv(H::Folder(item.parent.unwrap_or(0))),
        "selected" => V::Bool(e.state.project_selection.contains(&id)),
        "usedIn" => {
            let users: Vec<V> = p.items.iter().filter(|i| matches!(&i.kind, ItemKind::Comp(c) if c.layers.iter().any(|l| l.source == Some(id)))).map(|i| hv(H::Comp(i.id))).collect();
            arr(users)
        }
        "remove" => m(h, "remove", item_remove),
        "dynamicLinkGUID" => s(&format!("formvfx-{id}")),
        _ => return Ok(None),
    }))
}

fn item_set(it: &mut Interp, id: u64, key: &str, v: &V) -> Result<bool, Flow> {
    match key {
        "name" => {
            let n = it.to_string(v)?;
            exec(it, "project.rename", json!({"item": id, "name": n}))?;
        }
        "comment" => {
            let c = it.to_string(v)?;
            exec(it, "project.setComment", json!({"items": [id], "comment": c}))?;
        }
        "label" => {
            let n = it.to_number(v)?;
            exec(it, "edit.label", json!({"items": [id], "label": n as u64}))?;
        }
        "parentFolder" => {
            let f = match v {
                V::Host(h) => match &**h {
                    H::Folder(0) => Json::Null,
                    H::Folder(f) => json!(f),
                    _ => return Err(type_err("parentFolder must be a FolderItem")),
                },
                _ => Json::Null,
            };
            exec(it, "project.move", json!({"items": [id], "folder": f}))?;
        }
        "selected" => {
            if truthy(v) {
                exec(it, "project.select", json!({"items": [id], "add": true}))?;
            } else {
                let keep: Vec<u64> = eng()?.state.project_selection.iter().copied().filter(|x| *x != id).collect();
                exec(it, "project.select", json!({"items": keep}))?;
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn item_remove(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let h = this_h(this)?;
    let id = match &*h {
        H::Comp(id) | H::Footage(id) | H::Folder(id) => *id,
        _ => return Ok(V::Undef),
    };
    exec(it, "project.delete", json!({"items": [id]}))?;
    Ok(V::Undef)
}

fn folder_get(it: &mut Interp, h: &Rc<H>, id: u64, key: &str) -> Result<Option<V>, Flow> {
    if id == 0 {
        let n = item_list(pj(it), Some(0)).len();
        return Ok(Some(match key {
            "name" => s("Root"),
            "id" => V::Num(0.0),
            "typeName" => s("Folder"),
            "items" => hv(H::Items(Some(0))),
            "numItems" => V::Num(n as f64),
            "item" => m(h, "item", folder_item),
            "parentFolder" => V::Null,
            _ => return Ok(None),
        }));
    }
    match key {
        "items" => return Ok(Some(hv(H::Items(Some(id))))),
        "numItems" => return Ok(Some(V::Num(item_list(pj(it), Some(id)).len() as f64))),
        "item" => return Ok(Some(m(h, "item", folder_item))),
        _ => {}
    }
    item_common(it, h, id, key)
}

fn folder_item(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let h = this_h(this)?;
    let H::Folder(f) = &*h else { return Ok(V::Undef) };
    let ids = item_list(pj(it), Some(*f));
    let i = index_arg(it, &a, ids.len(), "item")?;
    Ok(item_v(pj(it), ids[i]))
}

fn comp_get(it: &mut Interp, h: &Rc<H>, c: u64, key: &str) -> Result<Option<V>, Flow> {
    if let Some(v) = item_common(it, h, c, key)? {
        return Ok(Some(v));
    }
    let p = pj(it);
    let comp = p.comp(c).ok_or_else(|| throw("Error", "this composition no longer exists"))?;
    let e = eng()?;
    Ok(Some(match key {
        "layers" => hv(H::Layers(c)),
        "selectedLayers" => arr(e.state.selected_layers.iter().filter(|l| comp.layer(**l).is_some()).map(|l| hv(H::Layer(c, *l))).collect()),
        "selectedProperties" => arr(vec![]),
        "time" => V::Num(e.time_of(c)),
        "workAreaStart" => V::Num(comp.work_area[0]),
        "workAreaDuration" => V::Num(comp.work_area[1] - comp.work_area[0]),
        "bgColor" => nums(&[comp.background[0] as f64, comp.background[1] as f64, comp.background[2] as f64]),
        "hideShyLayers" => V::Bool(comp.hide_shy),
        "motionBlur" => V::Bool(comp.motion_blur),
        "frameBlending" => V::Bool(comp.frame_blending),
        "preserveNestedFrameRate" => V::Bool(comp.preserve_frame_rate),
        "preserveNestedResolution" => V::Bool(comp.preserve_resolution),
        "renderer" => s(&comp.renderer),
        "renderers" => arr(vec![s("classic3d"), s("advanced3d")]),
        "motionBlurSamplesPerFrame" => V::Num(comp.motion_blur_samples as f64),
        "hasVideo" => V::Bool(true),
        "hasAudio" => V::Bool(comp.layers.iter().any(|l| l.switches.audio && matches!(l.kind, LayerKind::Footage | LayerKind::Precomp))),
        "footageMissing" | "useProxy" | "isDropFrame" => V::Bool(false),
        "proxySource" => V::Null,
        "markerProperty" => hv(H::Marker(c, None)),
        "openInViewer" => m(h, "openInViewer", comp_open),
        "duplicate" => m(h, "duplicate", comp_duplicate),
        "motionGraphicsTemplateName" => s(if comp.template_name.is_empty() { p.item(c).map(|i| i.name.as_str()).unwrap_or("") } else { &comp.template_name }),
        "motionGraphicsTemplateControllerCount" => V::Num(comp.essential.iter().filter(|x| x.kind == "property").count() as f64),
        "getMotionGraphicsTemplateControllerName" => m(h, "getMotionGraphicsTemplateControllerName", comp_mgt_name),
        "setMotionGraphicsControllerName" => m(h, "setMotionGraphicsControllerName", comp_mgt_rename),
        "exportAsMotionGraphicsTemplate" => m(h, "exportAsMotionGraphicsTemplate", comp_mgt_export),
        "openInEssentialGraphics" => m(h, "openInEssentialGraphics", comp_mgt_open),
        "layer" | "numLayers" | "width" | "height" | "duration" | "frameRate" | "frameDuration" | "pixelAspect" | "displayStartTime" | "shutterAngle" | "shutterPhase" => return Ok(None),
        _ => return Ok(None),
    }))
}

fn comp_set(it: &mut Interp, c: u64, key: &str, v: V) -> Result<(), Flow> {
    let setting = |k: &str| matches!(k, "width" | "height" | "duration" | "frameRate" | "pixelAspect" | "displayStartTime" | "shutterAngle" | "shutterPhase");
    match key {
        k if setting(k) => {
            let n = it.to_number(&v)?;
            let name = match k {
                "displayStartTime" => "startTime",
                other => other,
            };
            exec(it, "comp.settings", json!({"comp": c, name: n}))?;
        }
        "frameDuration" => {
            let n = it.to_number(&v)?;
            if n > 0.0 {
                exec(it, "comp.settings", json!({"comp": c, "frameRate": 1.0 / n}))?;
            }
        }
        "bgColor" => {
            let col = to_json(it, &v, 0)?;
            exec(it, "comp.settings", json!({"comp": c, "background": col}))?;
        }
        "time" => {
            let t = it.to_number(&v)?;
            exec(it, "time.set", json!({"comp": c, "time": t}))?;
        }
        "workAreaStart" | "workAreaDuration" => {
            let n = it.to_number(&v)?;
            let wa = pj(it).comp(c).map(|x| x.work_area).unwrap_or([0.0, 0.0]);
            let (start, end) = if key == "workAreaStart" { (n, n + (wa[1] - wa[0])) } else { (wa[0], wa[0] + n) };
            exec(it, "comp.workArea", json!({"comp": c, "start": start, "end": end}))?;
        }
        "hideShyLayers" | "motionBlur" | "frameBlending" => {
            let sw = match key {
                "hideShyLayers" => "hideShy",
                other => other,
            };
            exec(it, "comp.setSwitch", json!({"comp": c, "switch": sw, "value": truthy(&v)}))?;
        }
        "renderer" => {
            let r = it.to_string(&v)?;
            exec(it, "comp.renderer", json!({"comp": c, "renderer": r}))?;
        }
        "motionBlurSamplesPerFrame" => {
            let n = it.to_number(&v)?;
            exec(it, "comp.settings", json!({"comp": c, "motionBlurSamples": n}))?;
        }
        "motionGraphicsTemplateName" => {
            let n = it.to_string(&v)?;
            exec(it, "essential.setTemplateName", json!({"comp": c, "name": n}))?;
        }
        _ => return Err(type_err(format!("`{key}` cannot be set on a composition"))),
    }
    Ok(())
}

fn comp_open(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let h = this_h(this)?;
    let H::Comp(c) = &*h else { return Ok(V::Null) };
    exec(it, "comp.open", json!({"comp": c}))?;
    Ok(V::Null)
}

fn mgt_controller(it: &mut Interp, c: u64, a: &[V]) -> Result<String, Flow> {
    let ids: Vec<String> = pj(it).comp(c).ok_or_else(|| throw("Error", "this composition no longer exists"))?.essential.iter().filter(|x| x.kind == "property").map(|x| x.id.clone()).collect();
    let i = index_arg(it, a, ids.len(), "controller")?;
    Ok(ids[i].clone())
}

fn comp_mgt_name(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let h = this_h(this)?;
    let H::Comp(c) = &*h else { return Ok(V::Null) };
    let id = mgt_controller(it, *c, &a)?;
    Ok(pj(it).comp(*c).and_then(|x| x.essential.iter().find(|k| k.id == id)).map(|k| s(&k.name)).unwrap_or(V::Null))
}

fn comp_mgt_rename(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let h = this_h(this)?;
    let H::Comp(c) = &*h else { return Ok(V::Null) };
    let id = mgt_controller(it, *c, &a)?;
    let name = it.to_string(&arg(&a, 1))?;
    exec(it, "essential.rename", json!({"comp": c, "controller": id, "name": name}))?;
    Ok(V::Undef)
}

fn comp_mgt_export(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let h = this_h(this)?;
    let H::Comp(c) = &*h else { return Ok(V::Bool(false)) };
    let overwrite = truthy(&arg(&a, 0));
    let path = match arg(&a, 1) {
        V::Undef | V::Null => None,
        v => Some(path_arg(it, &v)?),
    };
    let r = exec(it, "essential.exportTemplate", json!({"comp": c, "overwrite": overwrite, "folder": path}));
    Ok(V::Bool(r.is_ok()))
}

fn comp_mgt_open(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let h = this_h(this)?;
    let H::Comp(c) = &*h else { return Ok(V::Null) };
    exec(it, "essential.open", json!({"comp": c}))?;
    Ok(V::Undef)
}

fn comp_duplicate(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let h = this_h(this)?;
    let H::Comp(c) = &*h else { return Ok(V::Null) };
    let before: Vec<u64> = pj(it).items.iter().map(|i| i.id).collect();
    let r = exec(it, "comp.duplicate", json!({"comp": c}))?;
    let id = r.get("comp").and_then(Json::as_u64).or_else(|| pj(it).items.iter().map(|i| i.id).find(|id| !before.contains(id)));
    Ok(id.map(|i| hv(H::Comp(i))).unwrap_or(V::Null))
}

fn footage_get(it: &mut Interp, h: &Rc<H>, id: u64, key: &str) -> Result<Option<V>, Flow> {
    if let Some(v) = item_common(it, h, id, key)? {
        return Ok(Some(v));
    }
    let p = pj(it);
    let item = p.item(id).ok_or_else(|| throw("Error", "this item no longer exists"))?;
    Ok(Some(match (&item.kind, key) {
        (ItemKind::Footage(f), "file") => file_obj(&f.path),
        (ItemKind::Footage(f), "mainSource") => obj(vec![
            ("file", file_obj(&f.path)),
            ("isStill", V::Bool(f.is_still())),
            ("hasAlpha", V::Bool(!matches!(f.alpha, crate::model::AlphaMode::Ignore))),
            ("nativeFrameRate", V::Num(f.frame_rate)),
            ("conformFrameRate", V::Num(f.frame_rate)),
            ("displayFrameRate", V::Num(f.frame_rate)),
            ("loop", V::Num(f.loops.max(1) as f64)),
            ("missingFootagePath", s(if f.missing { &f.path } else { "" })),
        ]),
        (ItemKind::Solid(sd), "mainSource") => obj(vec![("color", nums(&sd.color)), ("isStill", V::Bool(true)), ("hasAlpha", V::Bool(false))]),
        (ItemKind::Solid(_), "file") => V::Null,
        (ItemKind::Footage(f), "footageMissing") => V::Bool(f.missing),
        (ItemKind::Footage(f), "hasVideo") => V::Bool(f.has_video),
        (ItemKind::Footage(f), "hasAudio") => V::Bool(f.has_audio),
        (ItemKind::Solid(_), "hasVideo") => V::Bool(true),
        (ItemKind::Solid(_), "hasAudio" | "footageMissing") => V::Bool(false),
        (_, "useProxy") => V::Bool(item.proxy.as_ref().is_some_and(|x| x.enabled)),
        (_, "proxySource") => match &item.proxy {
            Some(px) => obj(vec![("file", file_obj(&px.footage.path))]),
            None => V::Null,
        },
        (_, "replace") => m(h, "replace", footage_replace),
        (_, "setProxy") => m(h, "setProxy", footage_set_proxy),
        (_, "setProxyToNone") => m(h, "setProxyToNone", footage_proxy_none),
        _ => return Ok(None),
    }))
}

fn footage_replace(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let h = this_h(this)?;
    let H::Footage(id) = &*h else { return Ok(V::Undef) };
    let path = path_arg(it, &arg(&a, 0))?;
    exec(it, "file.replaceFootage", json!({"path": path, "items": [id]}))?;
    Ok(V::Undef)
}

fn footage_set_proxy(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let h = this_h(this)?;
    let id = match &*h {
        H::Footage(id) | H::Comp(id) => *id,
        _ => return Ok(V::Undef),
    };
    let path = path_arg(it, &arg(&a, 0))?;
    exec(it, "file.setProxy", json!({"path": path, "items": [id]}))?;
    Ok(V::Undef)
}

fn footage_proxy_none(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let h = this_h(this)?;
    let id = match &*h {
        H::Footage(id) | H::Comp(id) => *id,
        _ => return Ok(V::Undef),
    };
    exec(it, "file.setProxyNone", json!({"items": [id]}))?;
    Ok(V::Undef)
}

fn layers_get(it: &mut Interp, h: &Rc<H>, c: u64, key: &str) -> Result<Option<V>, Flow> {
    let comp = pj(it).comp(c).ok_or_else(|| throw("Error", "this composition no longer exists"))?;
    if let Some(i) = num_key(key) {
        return Ok(Some(comp.layers.get(i.wrapping_sub(1)).map(|l| hv(H::Layer(c, l.id))).unwrap_or(V::Undef)));
    }
    Ok(Some(match key {
        "length" => V::Num(comp.layers.len() as f64),
        "addSolid" => m(h, "addSolid", lc_add_solid),
        "addText" | "addVerticalText" => m(h, "addText", lc_add_text),
        "addBoxText" | "addVerticalBoxText" => m(h, "addBoxText", lc_add_box_text),
        "addNull" => m(h, "addNull", lc_add_null),
        "addShape" => m(h, "addShape", lc_add_shape),
        "addCamera" => m(h, "addCamera", lc_add_camera),
        "addLight" => m(h, "addLight", lc_add_light),
        "add" => m(h, "add", lc_add),
        "byName" => m(h, "byName", lc_by_name),
        "precompose" => m(h, "precompose", lc_precompose),
        _ => return Ok(None),
    }))
}

fn lc_comp(this: &V) -> Result<u64, Flow> {
    match &*this_h(this)? {
        H::Layers(c) => Ok(*c),
        _ => Err(type_err("this method belongs to a LayerCollection")),
    }
}

fn made_layer(it: &mut Interp, c: u64, cmd: &str, mut params: Json) -> Result<u64, Flow> {
    params["comp"] = json!(c);
    let r = exec(it, cmd, params)?;
    let id = r.get("layer").and_then(Json::as_u64).ok_or_else(|| throw("Error", "the layer was not made"))?;
    let at = pj(it).comp(c).and_then(|x| x.index_of(id)).unwrap_or(0);
    if at != 0 {
        exec(it, "layer.arrange", json!({"comp": c, "layers": [id], "to": "front"}))?;
    }
    Ok(id)
}

fn set_duration(it: &mut Interp, c: u64, id: u64, d: &V) -> Result<(), Flow> {
    if matches!(d, V::Undef | V::Null) {
        return Ok(());
    }
    let dur = it.to_number(d)?;
    let inp = pj(it).comp(c).and_then(|x| x.layer(id)).map(|l| l.in_point).unwrap_or(0.0);
    exec(it, "layer.timing", json!({"comp": c, "layers": [id], "out": inp + dur}))?;
    Ok(())
}

fn lc_add_solid(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = lc_comp(this)?;
    let color = to_json(it, &arg(&a, 0), 0)?;
    let name = it.to_string(&arg(&a, 1))?;
    let w = it.to_number(&arg(&a, 2))?;
    let hgt = it.to_number(&arg(&a, 3))?;
    let id = made_layer(it, c, "layer.newSolid", json!({"name": name, "color": color, "width": w.round(), "height": hgt.round()}))?;
    set_duration(it, c, id, &arg(&a, 5))?;
    Ok(hv(H::Layer(c, id)))
}

fn lc_add_text(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = lc_comp(this)?;
    let t = arg(&a, 0);
    let mut params = json!({});
    match &t {
        V::Obj(_) => {
            let attrs = text_attrs(it, &t)?;
            params["text"] = attrs.get("text").cloned().unwrap_or(json!(""));
            let id = made_layer(it, c, "layer.newText", params)?;
            set_text_attrs(it, c, id, attrs)?;
            return Ok(hv(H::Layer(c, id)));
        }
        V::Undef => {}
        other => params["text"] = json!(it.to_string(other)?),
    }
    let id = made_layer(it, c, "layer.newText", params)?;
    Ok(hv(H::Layer(c, id)))
}

fn lc_add_box_text(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = lc_comp(this)?;
    let size = it.vector(&arg(&a, 0))?.unwrap_or_else(|| vec![400.0, 200.0]);
    let (bw, bh) = (size.first().copied().unwrap_or(400.0), size.get(1).copied().unwrap_or(200.0));
    let t = arg(&a, 1);
    let text = match &t {
        V::Undef => String::new(),
        V::Obj(_) => text_attrs(it, &t)?.get("text").and_then(Json::as_str).unwrap_or("").to_string(),
        other => it.to_string(other)?,
    };
    let id = made_layer(it, c, "layer.newText", json!({"text": text, "box": [-bw / 2.0, -bh / 2.0, bw, bh]}))?;
    if let V::Obj(_) = &t {
        let attrs = text_attrs(it, &t)?;
        set_text_attrs(it, c, id, attrs)?;
    }
    Ok(hv(H::Layer(c, id)))
}

fn lc_add_null(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = lc_comp(this)?;
    let id = made_layer(it, c, "layer.newNull", json!({}))?;
    set_duration(it, c, id, &arg(&a, 0))?;
    Ok(hv(H::Layer(c, id)))
}

fn lc_add_shape(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let c = lc_comp(this)?;
    let id = made_layer(it, c, "layer.newShape", json!({"kind": "none"}))?;
    Ok(hv(H::Layer(c, id)))
}

fn lc_add_camera(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = lc_comp(this)?;
    let name = it.to_string(&arg(&a, 0))?;
    let id = made_layer(it, c, "layer.newCamera", json!({"name": name}))?;
    if let Some(pt) = it.vector(&arg(&a, 1))? {
        let poi = json!([pt.first().copied().unwrap_or(0.0), pt.get(1).copied().unwrap_or(0.0), 0.0]);
        let _ = exec(it, "prop.set", json!({"comp": c, "layer": id, "path": "transform/poi", "value": poi}));
    }
    Ok(hv(H::Layer(c, id)))
}

fn lc_add_light(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = lc_comp(this)?;
    let name = it.to_string(&arg(&a, 0))?;
    let id = made_layer(it, c, "layer.newLight", json!({"name": name}))?;
    if let Some(pt) = it.vector(&arg(&a, 1))? {
        let poi = json!([pt.first().copied().unwrap_or(0.0), pt.get(1).copied().unwrap_or(0.0), 0.0]);
        let _ = exec(it, "prop.set", json!({"comp": c, "layer": id, "path": "transform/poi", "value": poi}));
    }
    Ok(hv(H::Layer(c, id)))
}

fn lc_add(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = lc_comp(this)?;
    let item = match arg(&a, 0) {
        V::Host(h) => match &*h {
            H::Comp(id) | H::Footage(id) => *id,
            _ => return Err(type_err("add() takes a footage or composition item")),
        },
        _ => return Err(type_err("add() takes a footage or composition item")),
    };
    let id = made_layer(it, c, "layer.addItem", json!({"item": item}))?;
    set_duration(it, c, id, &arg(&a, 1))?;
    Ok(hv(H::Layer(c, id)))
}

fn lc_by_name(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = lc_comp(this)?;
    let name = it.to_string(&arg(&a, 0))?;
    Ok(pj(it).comp(c).and_then(|x| x.layers.iter().find(|l| l.name == name)).map(|l| hv(H::Layer(c, l.id))).unwrap_or(V::Null))
}

fn lc_precompose(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = lc_comp(this)?;
    let list = it.iterate(&arg(&a, 0))?;
    let comp = pj(it).comp(c).ok_or_else(|| throw("Error", "no composition"))?;
    let mut ids = vec![];
    for x in &list {
        let i = it.to_number(x)?;
        let l = comp.layers.get((i as usize).wrapping_sub(1)).ok_or_else(|| throw("RangeError", "precompose takes layer indices"))?;
        ids.push(l.id);
    }
    let name = it.to_string(&arg(&a, 1))?;
    let r = exec(it, "layer.precompose", json!({"comp": c, "layers": ids, "name": name, "open": false}))?;
    Ok(r.get("comp").and_then(Json::as_u64).map(|x| hv(H::Comp(x))).unwrap_or(V::Null))
}

const LAYER_SWITCHES: [(&str, &str); 11] = [
    ("enabled", "video"),
    ("solo", "solo"),
    ("locked", "lock"),
    ("shy", "shy"),
    ("threeDLayer", "threeD"),
    ("motionBlur", "motionBlur"),
    ("adjustmentLayer", "adjustment"),
    ("guideLayer", "guide"),
    ("collapseTransformation", "collapse"),
    ("effectsActive", "fx"),
    ("audioEnabled", "audio"),
];

fn layer_get(it: &mut Interp, h: &Rc<H>, c: u64, l: u64, key: &str) -> Result<Option<V>, Flow> {
    let p = pj(it);
    let comp = p.comp(c).ok_or_else(|| throw("Error", "this layer's composition no longer exists"))?;
    let layer = comp.layer(l).ok_or_else(|| throw("Error", "this layer no longer exists"))?;
    let e = eng()?;
    let sw = &layer.switches;
    let group = |id: &str| layer.props.sub(id).map(|g| hv(H::Group(c, l, g.uid))).unwrap_or(V::Null);
    Ok(Some(match key {
        "enabled" => V::Bool(sw.video),
        "solo" => V::Bool(sw.solo),
        "locked" => V::Bool(sw.locked),
        "shy" => V::Bool(sw.shy),
        "threeDLayer" => V::Bool(sw.three_d),
        "motionBlur" => V::Bool(sw.motion_blur),
        "adjustmentLayer" => V::Bool(sw.adjustment),
        "guideLayer" => V::Bool(sw.guide),
        "collapseTransformation" => V::Bool(sw.collapse),
        "effectsActive" => V::Bool(sw.effects),
        "audioEnabled" => V::Bool(sw.audio),
        "preserveTransparency" => V::Bool(layer.preserve_transparency),
        "nullLayer" => V::Bool(layer.kind == LayerKind::Null),
        "isNameSet" => V::Bool(true),
        "isNameFromSource" => V::Bool(layer.source.and_then(|sid| p.item(sid)).is_some_and(|i| i.name == layer.name)),
        "threeDPerChar" => V::Bool(layer.props.sub("text").and_then(|g| g.get("perChar3d")).is_some_and(|pr| matches!(pr.value, Value::Bool(true)))),
        "containingComp" => hv(H::Comp(c)),
        "parent" => layer.parent.map(|x| hv(H::Layer(c, x))).unwrap_or(V::Null),
        "source" => match layer.source {
            Some(sid) => item_v(p, sid),
            None => V::Null,
        },
        "quality" => s(match sw.quality {
            crate::model::Quality::Best => "best",
            crate::model::Quality::Draft => "draft",
            crate::model::Quality::Wireframe => "wireframe",
        }),
        "blendingMode" => s(layer.blend.label()),
        "trackMatteType" => s(match layer.track_matte.map(|x| x.kind) {
            None => "none",
            Some(MatteKind::Alpha) => "alpha",
            Some(MatteKind::AlphaInverted) => "alphaInverted",
            Some(MatteKind::Luma) => "luma",
            Some(MatteKind::LumaInverted) => "lumaInverted",
        }),
        "autoOrient" => s(match layer.auto_orient {
            crate::model::AutoOrient::Off => "off",
            crate::model::AutoOrient::AlongPath => "alongPath",
            _ => "towardsCamera",
        }),
        "comment" => s(&layer.comment),
        "stretch" => V::Num(layer.stretch),
        "time" => V::Num(e.time_of(c)),
        "selected" => V::Bool(e.state.selected_layers.contains(&l)),
        "matchName" => s(match layer.kind {
            LayerKind::Text => "ADBE Text Layer",
            LayerKind::Shape => "ADBE Vector Layer",
            LayerKind::Camera => "ADBE Camera Layer",
            LayerKind::Light => "ADBE Light Layer",
            _ => "ADBE AV Layer",
        }),
        "propertyType" => s("NAMED_GROUP"),
        "propertyDepth" => V::Num(0.0),
        "parentProperty" => V::Null,
        "isEffect" | "isMask" | "elided" | "isModified" => V::Bool(false),
        "canAddProperty" => m(h, "canAddProperty", g_false),
        "property" => m(h, "property", layer_property),
        "Effects" | "effect" => group("effects"),
        "Masks" | "mask" => group("masks"),
        "Transform" => group("transform"),
        "Text" => group("text"),
        "Contents" | "content" => group("contents"),
        "essentialProperty" => group("essentialProperties"),
        "Marker" => hv(H::Marker(c, Some(l))),
        "remove" => m(h, "remove", layer_remove),
        "duplicate" => m(h, "duplicate", layer_duplicate),
        "moveToBeginning" => m(h, "moveToBeginning", layer_move_first),
        "moveToEnd" => m(h, "moveToEnd", layer_move_last),
        "moveBefore" => m(h, "moveBefore", layer_move_before),
        "moveAfter" => m(h, "moveAfter", layer_move_after),
        "setParentWithJump" => m(h, "setParentWithJump", layer_set_parent),
        "applyPreset" => m(h, "applyPreset", layer_apply_preset),
        "copyToComp" => m(h, "copyToComp", layer_copy_to_comp),
        "openInViewer" => m(h, "openInViewer", g_null),
        "setTrackMatte" => m(h, "setTrackMatte", layer_set_track_matte),
        "removeTrackMatte" => m(h, "removeTrackMatte", layer_remove_track_matte),
        "activeAtTime" => m(h, "activeAtTime", layer_active_at),
        "addToMotionGraphicsTemplate" | "addToMotionGraphicsTemplateAs" | "canAddToMotionGraphicsTemplate" => return Ok(None),
        _ => return Ok(None),
    }))
}

fn g_false(_: &mut Interp, _: &V, _: Vec<V>) -> R {
    Ok(V::Bool(false))
}

fn layer_set(it: &mut Interp, c: u64, l: u64, key: &str, v: V) -> Result<(), Flow> {
    if let Some((_, sw)) = LAYER_SWITCHES.iter().find(|(k, _)| *k == key) {
        exec(it, "layer.setSwitch", json!({"comp": c, "layers": [l], "switch": sw, "value": truthy(&v)}))?;
        return Ok(());
    }
    match key {
        "name" => {
            let n = it.to_string(&v)?;
            exec(it, "layer.rename", json!({"comp": c, "layer": l, "name": n}))?;
        }
        "preserveTransparency" => {
            exec(it, "layer.setSwitch", json!({"comp": c, "layers": [l], "switch": "preserveTransparency", "value": truthy(&v)}))?;
        }
        "blendingMode" => {
            let mode = it.to_string(&v)?;
            exec(it, "layer.setBlendMode", json!({"comp": c, "layers": [l], "mode": mode}))?;
        }
        "parent" => {
            let parent = layer_json(&v);
            exec(it, "layer.setParent", json!({"comp": c, "layers": [l], "parent": parent}))?;
        }
        "inPoint" | "outPoint" | "startTime" | "stretch" => {
            let n = it.to_number(&v)?;
            let k = match key {
                "inPoint" => "in",
                "outPoint" => "out",
                "startTime" => "start",
                other => other,
            };
            exec(it, "layer.timing", json!({"comp": c, "layers": [l], k: n}))?;
        }
        "label" => {
            let n = it.to_number(&v)?;
            exec(it, "edit.label", json!({"comp": c, "layers": [l], "label": n as u64}))?;
        }
        "comment" => {
            let t = it.to_string(&v)?;
            exec(it, "layer.setComment", json!({"comp": c, "layers": [l], "comment": t}))?;
        }
        "autoOrient" => {
            let t = it.to_string(&v)?;
            exec(it, "layer.autoOrient", json!({"comp": c, "layers": [l], "mode": t}))?;
        }
        "quality" => {
            let t = it.to_string(&v)?;
            exec(it, "layer.quality", json!({"comp": c, "layers": [l], "quality": t}))?;
        }
        "threeDPerChar" => {
            exec(it, "layer.enablePerChar3D", json!({"comp": c, "layer": l, "enabled": truthy(&v)}))?;
        }
        "trackMatteType" => {
            let kind = it.to_string(&v)?;
            if kind == "none" {
                exec(it, "layer.setTrackMatte", json!({"comp": c, "layer": l, "matte": null}))?;
            } else {
                let above = pj(it).comp(c).and_then(|x| x.index_of(l).and_then(|i| i.checked_sub(1)).and_then(|i| x.layers.get(i)).map(|x| x.id));
                let Some(m) = above else { return Err(throw("Error", "there is no layer above to use as the matte")) };
                exec(it, "layer.setTrackMatte", json!({"comp": c, "layer": l, "matte": m, "kind": kind}))?;
            }
        }
        "selected" => {
            let on = truthy(&v);
            let is = eng()?.state.selected_layers.contains(&l);
            if on != is {
                exec(it, "layer.select", json!({"comp": c, "layers": [l], "toggle": true}))?;
            }
        }
        _ => return Err(type_err(format!("`{key}` cannot be set on a layer"))),
    }
    Ok(())
}

fn layer_json(v: &V) -> Json {
    match v {
        V::Host(h) => match &**h {
            H::Layer(_, l) => json!(l),
            _ => Json::Null,
        },
        _ => Json::Null,
    }
}

fn this_layer(this: &V) -> Result<(u64, u64), Flow> {
    match &*this_h(this)? {
        H::Layer(c, l) => Ok((*c, *l)),
        _ => Err(type_err("this method belongs to a layer")),
    }
}

fn layer_property(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let h = this_h(this)?;
    let (c, l) = this_layer(this)?;
    if let V::Str(t) = arg(&a, 0) {
        let k = match_alias(&t);
        let p = pj(it);
        let layer = p.comp(c).and_then(|x| x.layer(l)).ok_or_else(|| throw("Error", "this layer no longer exists"))?;
        let deep = match k {
            "sourceText" | "Source Text" => layer.props.sub("text").and_then(|g| g.get("sourceText")).map(|pr| hv(H::Prop(c, l, pr.uid))),
            "marker" | "Marker" => Some(hv(H::Marker(c, Some(l)))),
            "timeRemap" | "Time Remap" => layer.props.get("timeRemap").map(|pr| hv(H::Prop(c, l, pr.uid))),
            _ => None,
        };
        if let Some(v) = deep {
            return Ok(v);
        }
        return host::call(it, &h, vec![s(k)]);
    }
    host::call(it, &h, vec![arg(&a, 0)])
}

fn layer_remove(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let (c, l) = this_layer(this)?;
    exec(it, "edit.clear", json!({"comp": c, "layers": [l]}))?;
    Ok(V::Undef)
}

fn layer_duplicate(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let (c, l) = this_layer(this)?;
    let before: Vec<u64> = pj(it).comp(c).map(|x| x.layers.iter().map(|y| y.id).collect()).unwrap_or_default();
    exec(it, "edit.duplicate", json!({"comp": c, "layers": [l]}))?;
    let new = pj(it).comp(c).and_then(|x| x.layers.iter().map(|y| y.id).find(|id| !before.contains(id)));
    Ok(new.map(|id| hv(H::Layer(c, id))).unwrap_or(V::Null))
}

fn layer_move_first(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let (c, l) = this_layer(this)?;
    exec(it, "layer.arrange", json!({"comp": c, "layers": [l], "to": "front"}))?;
    Ok(V::Undef)
}

fn layer_move_last(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let (c, l) = this_layer(this)?;
    exec(it, "layer.arrange", json!({"comp": c, "layers": [l], "to": "back"}))?;
    Ok(V::Undef)
}

fn other_layer(v: &V) -> Result<u64, Flow> {
    match v {
        V::Host(h) => match &**h {
            H::Layer(_, l) => Ok(*l),
            _ => Err(type_err("this method takes a layer")),
        },
        _ => Err(type_err("this method takes a layer")),
    }
}

fn layer_move_before(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l) = this_layer(this)?;
    let o = other_layer(&arg(&a, 0))?;
    exec(it, "layer.arrange", json!({"comp": c, "layers": [l], "above": o}))?;
    Ok(V::Undef)
}

fn layer_move_after(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l) = this_layer(this)?;
    let o = other_layer(&arg(&a, 0))?;
    let below = pj(it).comp(c).and_then(|x| x.index_of(o).and_then(|i| x.layers.get(i + 1)).map(|y| y.id));
    match below {
        Some(b) if b != l => {
            exec(it, "layer.arrange", json!({"comp": c, "layers": [l], "above": b}))?;
        }
        Some(_) => {}
        None => {
            exec(it, "layer.arrange", json!({"comp": c, "layers": [l], "to": "back"}))?;
        }
    }
    Ok(V::Undef)
}

fn layer_set_parent(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l) = this_layer(this)?;
    let parent = layer_json(&arg(&a, 0));
    exec(it, "layer.setParent", json!({"comp": c, "layers": [l], "parent": parent}))?;
    Ok(V::Undef)
}

fn layer_apply_preset(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l) = this_layer(this)?;
    let path = path_arg(it, &arg(&a, 0))?;
    exec(it, "anim.applyPreset", json!({"comp": c, "layers": [l], "path": path}))?;
    Ok(V::Undef)
}

fn layer_copy_to_comp(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l) = this_layer(this)?;
    let target = match arg(&a, 0) {
        V::Host(h) => match &*h {
            H::Comp(x) => *x,
            _ => return Err(type_err("copyToComp takes a composition")),
        },
        _ => return Err(type_err("copyToComp takes a composition")),
    };
    exec(it, "layer.copyToComp", json!({"comp": c, "layers": [l], "target": target}))?;
    Ok(V::Undef)
}

fn layer_set_track_matte(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l) = this_layer(this)?;
    let matte = layer_json(&arg(&a, 0));
    let kind = match arg(&a, 1) {
        V::Undef => "alpha".to_string(),
        v => it.to_string(&v)?,
    };
    if kind == "none" {
        exec(it, "layer.setTrackMatte", json!({"comp": c, "layer": l, "matte": null}))?;
    } else {
        exec(it, "layer.setTrackMatte", json!({"comp": c, "layer": l, "matte": matte, "kind": kind}))?;
    }
    Ok(V::Undef)
}

fn layer_remove_track_matte(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let (c, l) = this_layer(this)?;
    exec(it, "layer.setTrackMatte", json!({"comp": c, "layer": l, "matte": null}))?;
    Ok(V::Undef)
}

fn layer_active_at(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l) = this_layer(this)?;
    let t = it.to_number(&arg(&a, 0))?;
    Ok(V::Bool(pj(it).comp(c).and_then(|x| x.layer(l)).is_some_and(|y| y.switches.video && y.is_active_at(t))))
}

pub fn match_alias(name: &str) -> &str {
    match name {
        "ADBE Transform Group" => "transform",
        "ADBE Anchor Point" => "anchor",
        "ADBE Position" => "position",
        "ADBE Scale" => "scale",
        "ADBE Rotate Z" => "rotation",
        "ADBE Rotate X" => "rotationX",
        "ADBE Rotate Y" => "rotationY",
        "ADBE Orientation" => "orientation",
        "ADBE Opacity" => "opacity",
        "ADBE Effect Parade" => "effects",
        "ADBE Mask Parade" => "masks",
        "ADBE Root Vectors Group" => "contents",
        "ADBE Vectors Group" => "contents",
        "ADBE Text Properties" => "text",
        "ADBE Text Document" => "sourceText",
        "ADBE Text Animators" => "animators",
        "ADBE Text Path Options" => "pathOptions",
        "ADBE Marker" => "marker",
        "ADBE Layer Overrides" | "Essential Properties" | "Primary Properties" => "essentialProperties",
        "ADBE Mask Shape" => "path",
        "ADBE Mask Feather" => "feather",
        "ADBE Mask Opacity" => "opacity",
        "ADBE Mask Offset" => "expansion",
        "ADBE Time Remapping" => "timeRemap",
        "ADBE Audio Group" => "audio",
        "ADBE Audio Levels" => "levels",
        "ADBE Camera Options Group" => "cameraOptions",
        "ADBE Light Options Group" => "lightOptions",
        "ADBE Material Options Group" => "materialOptions",
        "ADBE Layer Styles" => "layerStyles",
        "ADBE Vector Shape" => "path",
        "ADBE Vector Fill Color" | "ADBE Vector Stroke Color" => "color",
        "ADBE Vector Fill Opacity" | "ADBE Vector Stroke Opacity" => "opacity",
        "ADBE Vector Stroke Width" => "width",
        "ADBE Vector Rect Size" => "size",
        "ADBE Vector Rect Position" | "ADBE Vector Ellipse Position" | "ADBE Vector Star Position" => "position",
        "ADBE Vector Rect Roundness" => "roundness",
        "ADBE Vector Ellipse Size" => "size",
        "ADBE Vector Trim Start" => "start",
        "ADBE Vector Trim End" => "end",
        "ADBE Vector Trim Offset" => "offset",
        "ADBE Vector Transform Group" => "transform",
        other => other,
    }
}

pub fn effect_alias(name: &str) -> String {
    let n = name.strip_prefix("ADBE ").unwrap_or(name);
    let known = match n {
        "Gaussian Blur 2" | "Gaussian Blur" => "Gaussian Blur",
        "Fast Blur" => "Fast Box Blur",
        "Glo2" | "Glow" => "Glow",
        "Fill" => "Fill",
        "Ramp" => "Gradient Ramp",
        "Drop Shadow" => "Drop Shadow",
        "Fractal Noise" => "Fractal Noise",
        "Tint" => "Tint",
        "CurvesCustom" | "Curves" => "Curves",
        "Easy Levels2" | "Pro Levels2" => "Levels",
        "HUE SATURATION" => "Hue/Saturation",
        "Slider Control" => "Slider Control",
        "Point Control" => "Point Control",
        "Point3D Control" => "3D Point Control",
        "Angle Control" => "Angle Control",
        "Checkbox Control" => "Checkbox Control",
        "Color Control" => "Color Control",
        "Layer Control" => "Layer Control",
        "Dropdown Control" => "Dropdown Control",
        "Brightness & Contrast 2" => "Brightness & Contrast",
        "Exposure2" => "Exposure",
        "Invert" => "Invert",
        "Turbulent Displace" => "Turbulent Displace",
        "Linear Wipe" => "Linear Wipe",
        "Corner Pin" => "Corner Pin",
        "Geometry2" => "Transform",
        "Mosaic" => "Mosaic",
        "Stroke" => "Stroke",
        other => crate::legacy::alias(other).unwrap_or(other),
    };
    known.trim_end_matches(char::is_numeric).trim().to_string()
}

fn group_info(it: &Interp, c: u64, l: u64, u: u64) -> Result<(String, GroupKind, String, usize), Flow> {
    let p = pj(it);
    let layer = p.comp(c).and_then(|x| x.layer(l)).ok_or_else(|| throw("Error", "this layer no longer exists"))?;
    let g = layer.props.find_group(u).ok_or_else(|| throw("Error", "this property group no longer exists"))?;
    let mut depth = 0;
    let mut cur = u;
    while let Some(par) = layer.props.parent_of(cur) {
        depth += 1;
        if par.uid == layer.props.uid {
            break;
        }
        cur = par.uid;
    }
    Ok((g.id.clone(), g.kind.clone(), g.name.clone(), depth))
}

fn group_get(it: &mut Interp, h: &Rc<H>, c: u64, l: u64, u: u64, key: &str) -> Result<Option<V>, Flow> {
    let (id, kind, _name, depth) = group_info(it, c, l, u)?;
    Ok(Some(match key {
        "matchName" => s(&group_match_name(&id, &kind)),
        "propertyType" => s(if matches!(id.as_str(), "effects" | "masks" | "contents" | "animators") || matches!(kind, GroupKind::Indexed) { "INDEXED_GROUP" } else { "NAMED_GROUP" }),
        "propertyDepth" => V::Num(depth as f64),
        "parentProperty" => parent_v(it, c, l, u),
        "isEffect" => V::Bool(matches!(kind, GroupKind::Effect { .. })),
        "isMask" => V::Bool(matches!(kind, GroupKind::Mask { .. })),
        "elided" | "selected" => V::Bool(false),
        "isModified" => V::Bool(true),
        "canSetEnabled" => V::Bool(matches!(kind, GroupKind::Effect { .. } | GroupKind::Mask { .. })),
        "maskMode" => match &kind {
            GroupKind::Mask { mode, .. } => s(&format!("{mode:?}")),
            _ => return Ok(None),
        },
        "inverted" => match &kind {
            GroupKind::Mask { inverted, .. } => V::Bool(*inverted),
            _ => return Ok(None),
        },
        "locked" => match &kind {
            GroupKind::Mask { locked, .. } => V::Bool(*locked),
            _ => return Ok(None),
        },
        "addProperty" => m(h, "addProperty", group_add_property),
        "canAddProperty" => m(h, "canAddProperty", group_can_add),
        "remove" => m(h, "remove", group_remove),
        "duplicate" => m(h, "duplicate", group_duplicate),
        "moveTo" => m(h, "moveTo", group_move_to),
        "property" => m(h, "property", group_property),
        "propertyGroup" => return Ok(None),
        _ => return Ok(None),
    }))
}

fn group_property(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let h = this_h(this)?;
    let key = match arg(&a, 0) {
        V::Str(t) => s(match_alias(&t)),
        other => other,
    };
    host::call(it, &h, vec![key])
}

fn parent_v(it: &Interp, c: u64, l: u64, u: u64) -> V {
    let p = pj(it);
    let Some(layer) = p.comp(c).and_then(|x| x.layer(l)) else { return V::Null };
    match layer.props.parent_of(u) {
        Some(par) if par.uid != layer.props.uid => hv(H::Group(c, l, par.uid)),
        _ => hv(H::Layer(c, l)),
    }
}

fn group_match_name(id: &str, kind: &GroupKind) -> String {
    match kind {
        GroupKind::Effect { effect } => return effect.clone(),
        GroupKind::Mask { .. } => return "ADBE Mask Atom".into(),
        _ => {}
    }
    match id {
        "transform" => "ADBE Transform Group",
        "effects" => "ADBE Effect Parade",
        "masks" => "ADBE Mask Parade",
        "contents" => "ADBE Root Vectors Group",
        "text" => "ADBE Text Properties",
        "animators" => "ADBE Text Animators",
        "audio" => "ADBE Audio Group",
        "cameraOptions" => "ADBE Camera Options Group",
        "lightOptions" => "ADBE Light Options Group",
        "materialOptions" => "ADBE Material Options Group",
        "layerStyles" => "ADBE Layer Styles",
        "essentialProperties" => "ADBE Layer Overrides",
        other => other,
    }
    .to_string()
}

fn group_set(it: &mut Interp, c: u64, l: u64, u: u64, key: &str, v: V) -> Result<(), Flow> {
    let (_, kind, _, _) = group_info(it, c, l, u)?;
    match key {
        "name" => {
            let n = it.to_string(&v)?;
            exec(it, "prop.renameGroup", json!({"comp": c, "layer": l, "prop": u, "name": n}))?;
        }
        "enabled" | "active" => match kind {
            GroupKind::Effect { .. } => {
                exec(it, "effect.toggle", json!({"comp": c, "layer": l, "effect": u, "enabled": truthy(&v)}))?;
            }
            _ => return Err(type_err("only effects and masks can be switched off")),
        },
        "maskMode" => {
            let mode = it.to_string(&v)?;
            exec(it, "layer.mask.mode", json!({"comp": c, "layer": l, "mask": u, "mode": mode}))?;
        }
        "inverted" => {
            exec(it, "layer.mask.invert", json!({"comp": c, "layer": l, "mask": u, "value": truthy(&v)}))?;
        }
        "locked" => {
            exec(it, "layer.mask.lock", json!({"comp": c, "layer": l, "mask": u, "value": truthy(&v)}))?;
        }
        "selected" => {
            exec(it, "prop.select", json!({"comp": c, "layer": l, "prop": u, "add": truthy(&v)}))?;
        }
        _ => return Err(type_err(format!("`{key}` cannot be set on a property group"))),
    }
    Ok(())
}

fn this_group(this: &V) -> Result<(u64, u64, u64), Flow> {
    match &*this_h(this)? {
        H::Group(c, l, u) => Ok((*c, *l, *u)),
        _ => Err(type_err("this method belongs to a property group")),
    }
}

pub fn shape_kind(name: &str) -> Option<&'static str> {
    Some(match name {
        "ADBE Vector Group" => "group",
        "ADBE Vector Shape - Rect" => "rect",
        "ADBE Vector Shape - Ellipse" => "ellipse",
        "ADBE Vector Shape - Star" => "star",
        "ADBE Vector Shape - Group" => "path",
        "ADBE Vector Graphic - Fill" => "fill",
        "ADBE Vector Graphic - Stroke" => "stroke",
        "ADBE Vector Graphic - G-Fill" => "gradientFill",
        "ADBE Vector Graphic - G-Stroke" => "gradientStroke",
        "ADBE Vector Filter - Trim" => "trim",
        "ADBE Vector Filter - Repeater" => "repeater",
        "ADBE Vector Filter - Offset" => "offset",
        "ADBE Vector Filter - PB" => "puckerBloat",
        "ADBE Vector Filter - Twist" => "twist",
        "ADBE Vector Filter - Zigzag" => "zigZag",
        "ADBE Vector Filter - Roughen" => "wigglePaths",
        "ADBE Vector Filter - Wiggler" => "wiggleTransform",
        "ADBE Vector Filter - RC" => "roundCorners",
        "ADBE Vector Filter - Merge" => "merge",
        _ => return None,
    })
}

pub fn new_uid(r: &Json) -> Option<u64> {
    ["uid", "mask", "animator", "selector", "effect", "prop", "property", "group"].iter().find_map(|k| r.get(*k).and_then(Json::as_u64)).or_else(|| r.get("applied").and_then(Json::as_array).and_then(|a| a.first()).and_then(|x| x.get("uid")).and_then(Json::as_u64))
}

fn group_add_property(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_group(this)?;
    let name = it.to_string(&arg(&a, 0))?;
    let (id, kind, _, _) = group_info(it, c, l, u)?;
    let parent_kind = parent_group_id(it, c, l, u);
    let r = match id.as_str() {
        "effects" => exec(it, "effect.apply", json!({"comp": c, "layers": [l], "effect": effect_alias(&name)}))?,
        "masks" => {
            let (w, hgt) = layer_size(it, c, l);
            exec(it, "mask.new", json!({"comp": c, "layer": l, "vertices": [[0.0, 0.0], [w, 0.0], [w, hgt], [0.0, hgt]], "closed": true, "mode": "Add"}))?
        }
        "animators" => exec(it, "layer.addTextAnimator", json!({"comp": c, "layer": l, "properties": []}))?,
        _ if parent_kind.as_deref() == Some("animators") || id == "selectors" || id == "properties" => {
            if name.contains("Selector") || id == "selectors" {
                let kind = if name.contains("Wiggly") { "wiggly" } else if name.contains("Expressible") || name.contains("Expression") { "expression" } else { "range" };
                exec(it, "layer.addTextSelector", json!({"comp": c, "layer": l, "animator": u, "kind": kind}))?
            } else {
                exec(it, "layer.addTextAnimatorProperty", json!({"comp": c, "layer": l, "animator": u, "property": text_prop_alias(&name)}))?
            }
        }
        _ => match shape_kind(&name).or_else(|| shape_kind(&format!("ADBE Vector {name}"))) {
            Some(k) => {
                let group = if id == "contents" && parent_kind.is_none() {
                    Json::Null
                } else if id == "contents" {
                    parent_uid(it, c, l, u).map(|x| json!(x)).unwrap_or(Json::Null)
                } else {
                    json!(u)
                };
                exec(it, "layer.addShapeItem", json!({"comp": c, "layer": l, "kind": k, "group": group}))?
            }
            None => {
                let _ = kind;
                return Err(throw("Error", format!("`{name}` cannot be added to this group")));
            }
        },
    };
    Ok(match new_uid(&r) {
        Some(nu) => {
            let p = pj(it);
            let layer = p.comp(c).and_then(|x| x.layer(l));
            match layer {
                Some(ly) if ly.props.find_group(nu).is_some() => hv(H::Group(c, l, nu)),
                Some(ly) if ly.props.find(nu).is_some() => hv(H::Prop(c, l, nu)),
                _ => V::Null,
            }
        }
        None => V::Null,
    })
}

pub fn text_prop_alias(name: &str) -> String {
    let n = name.strip_prefix("ADBE Text ").unwrap_or(name);
    let k = match n {
        "Position 3D" | "Position" => "position",
        "Anchor Point 3D" | "Anchor Point" => "anchor",
        "Scale 3D" | "Scale" => "scale",
        "Skew" => "skew",
        "Rotation" | "Rotation Z" => "rotation",
        "Opacity" => "opacity",
        "Fill Color" => "fillColor",
        "Stroke Color" => "strokeColor",
        "Stroke Width" => "strokeWidth",
        "Tracking Amount" | "Tracking" | "Track Type" | "Tracking Type" => "tracking",
        "Line Spacing" => "lineSpacing",
        "Character Offset" => "characterOffset",
        "Blur" => "blur",
        other => other,
    };
    k.to_string()
}

fn parent_uid(it: &Interp, c: u64, l: u64, u: u64) -> Option<u64> {
    let p = pj(it);
    let layer = p.comp(c)?.layer(l)?;
    let par = layer.props.parent_of(u)?;
    (par.uid != layer.props.uid).then_some(par.uid)
}

fn parent_group_id(it: &Interp, c: u64, l: u64, u: u64) -> Option<String> {
    let p = pj(it);
    let layer = p.comp(c)?.layer(l)?;
    let par = layer.props.parent_of(u)?;
    (par.uid != layer.props.uid).then(|| par.id.clone())
}

fn layer_size(it: &Interp, c: u64, l: u64) -> (f64, f64) {
    let p = pj(it);
    let comp = p.comp(c);
    let layer = comp.and_then(|x| x.layer(l));
    let src = layer.and_then(|y| y.source).and_then(|sid| p.item(sid)).and_then(|i| i.dimensions());
    let (w, h) = src.or_else(|| comp.map(|x| (x.width, x.height))).unwrap_or((100, 100));
    (w as f64, h as f64)
}

fn group_can_add(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_group(this)?;
    let name = it.to_string(&arg(&a, 0))?;
    let (id, _, _, _) = group_info(it, c, l, u)?;
    Ok(V::Bool(match id.as_str() {
        "effects" => crate::effects::find(&effect_alias(&name)).is_some(),
        "masks" | "animators" => true,
        _ => shape_kind(&name).is_some(),
    }))
}

fn group_remove(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let (c, l, u) = this_group(this)?;
    exec(it, "prop.removeGroup", json!({"comp": c, "layer": l, "prop": u}))?;
    Ok(V::Undef)
}

fn group_duplicate(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let (c, l, u) = this_group(this)?;
    let r = exec(it, "prop.duplicateGroup", json!({"comp": c, "layer": l, "prop": u}))?;
    Ok(new_uid(&r).map(|x| hv(H::Group(c, l, x))).unwrap_or(V::Null))
}

fn group_move_to(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_group(this)?;
    let i = it.to_number(&arg(&a, 0))?;
    exec(it, "effect.reorder", json!({"comp": c, "layer": l, "effect": u, "index": i as u64}))?;
    Ok(V::Undef)
}

struct PropInfo {
    value: Value,
    keys: Vec<crate::model::Key>,
    spatial: bool,
    static_only: bool,
    ui: Ui,
    id: String,
    expr: Option<(String, bool)>,
    start: f64,
    stretch: f64,
    is3d: bool,
    default: Value,
}

fn prop_info(it: &Interp, c: u64, l: u64, u: u64) -> Result<PropInfo, Flow> {
    let p = pj(it);
    let layer = p.comp(c).and_then(|x| x.layer(l)).ok_or_else(|| throw("Error", "this layer no longer exists"))?;
    let pr = layer.props.find(u).ok_or_else(|| throw("Error", "this property no longer exists"))?;
    Ok(PropInfo {
        value: pr.value.clone(),
        keys: pr.keys.clone(),
        spatial: pr.spatial,
        static_only: pr.static_only,
        ui: pr.ui.clone(),
        id: pr.id.clone(),
        expr: pr.expr.as_ref().map(|e| (e.text.clone(), e.enabled)),
        start: layer.start_time,
        stretch: layer.stretch,
        is3d: layer.is_3d(),
        default: pr.default.clone(),
    })
}

impl PropInfo {
    fn layer_time(&self, t: f64) -> f64 {
        let s = if self.stretch.abs() < 1e-9 { 1.0 } else { self.stretch / 100.0 };
        (t - self.start) / s
    }

    fn comp_time(&self, lt: f64) -> f64 {
        let s = if self.stretch.abs() < 1e-9 { 1.0 } else { self.stretch / 100.0 };
        self.start + lt * s
    }
}

fn prop_get(it: &mut Interp, h: &Rc<H>, c: u64, l: u64, u: u64, key: &str) -> Result<Option<V>, Flow> {
    let info = prop_info(it, c, l, u)?;
    let now = eng()?.time_of(c);
    Ok(Some(match key {
        "value" => value_at(it, c, l, u, now, false)?,
        "valueAtTime" => m(h, "valueAtTime", p_value_at_time),
        "matchName" => s(&prop_match_name(&info.id)),
        "setValue" => m(h, "setValue", p_set_value),
        "setValueAtTime" => m(h, "setValueAtTime", p_set_value_at_time),
        "setValuesAtTimes" => m(h, "setValuesAtTimes", p_set_values_at_times),
        "setValueAtKey" => m(h, "setValueAtKey", p_set_value_at_key),
        "addKey" => m(h, "addKey", p_add_key),
        "removeKey" => m(h, "removeKey", p_remove_key),
        "keyTime" => m(h, "keyTime", p_key_time),
        "keyValue" => m(h, "keyValue", p_key_value),
        "nearestKeyIndex" => m(h, "nearestKeyIndex", p_nearest_key_index),
        "keyInInterpolationType" => m(h, "keyInInterpolationType", p_key_in_interp),
        "keyOutInterpolationType" => m(h, "keyOutInterpolationType", p_key_out_interp),
        "setInterpolationTypeAtKey" => m(h, "setInterpolationTypeAtKey", p_set_interp),
        "keyInTemporalEase" => m(h, "keyInTemporalEase", p_key_in_ease),
        "keyOutTemporalEase" => m(h, "keyOutTemporalEase", p_key_out_ease),
        "setTemporalEaseAtKey" => m(h, "setTemporalEaseAtKey", p_set_ease),
        "keyInSpatialTangent" => m(h, "keyInSpatialTangent", p_key_in_tangent),
        "keyOutSpatialTangent" => m(h, "keyOutSpatialTangent", p_key_out_tangent),
        "setSpatialTangentsAtKey" => m(h, "setSpatialTangentsAtKey", p_set_tangents),
        "keyRoving" => m(h, "keyRoving", p_key_roving),
        "setRovingAtKey" => m(h, "setRovingAtKey", p_set_roving),
        "keyTemporalAutoBezier" => m(h, "keyTemporalAutoBezier", p_key_auto),
        "keyTemporalContinuous" => m(h, "keyTemporalContinuous", p_key_continuous),
        "setTemporalAutoBezierAtKey" => m(h, "setTemporalAutoBezierAtKey", p_set_auto),
        "setTemporalContinuousAtKey" => m(h, "setTemporalContinuousAtKey", p_set_continuous),
        "keySelected" => m(h, "keySelected", g_false),
        "setSelectedAtKey" => m(h, "setSelectedAtKey", g_noop),
        "isInterpolationTypeValid" => m(h, "isInterpolationTypeValid", g_true),
        "keyLabel" => m(h, "keyLabel", p_key_label),
        "addToMotionGraphicsTemplate" => m(h, "addToMotionGraphicsTemplate", p_add_to_mgt),
        "addToMotionGraphicsTemplateAs" => m(h, "addToMotionGraphicsTemplateAs", p_add_to_mgt),
        "canAddToMotionGraphicsTemplate" => m(h, "canAddToMotionGraphicsTemplate", p_can_add_to_mgt),
        "expression" => s(info.expr.as_ref().map(|e| e.0.as_str()).unwrap_or("")),
        "expressionEnabled" => V::Bool(info.expr.as_ref().is_some_and(|e| e.1)),
        "expressionError" => s(&expression_error(it, c, l, u)),
        "canSetExpression" => V::Bool(true),
        "canVaryOverTime" => V::Bool(!info.static_only),
        "isTimeVarying" => V::Bool(!info.keys.is_empty() || info.expr.as_ref().is_some_and(|e| e.1 && !e.0.trim().is_empty())),
        "numKeys" => V::Num(info.keys.len() as f64),
        "propertyValueType" => s(value_type(&info)),
        "propertyType" => s("PROPERTY"),
        "propertyDepth" => V::Num(prop_depth(it, c, l, u) as f64),
        "parentProperty" => parent_v(it, c, l, u),
        "isSpatial" => V::Bool(info.spatial),
        "dimensionsSeparated" | "isSeparationFollower" | "isSeparationLeader" | "selected" | "elided" | "isEffect" | "isMask" => V::Bool(false),
        "isModified" => V::Bool(info.value != info.default || !info.keys.is_empty()),
        "unitsText" => s(match info.ui {
            Ui::Angle => "degrees",
            Ui::Percent => "percent",
            Ui::Pixels | Ui::Point | Ui::Point3 => "pixels",
            _ => "",
        }),
        "hasMin" | "hasMax" => V::Bool(matches!(info.ui, Ui::Slider { .. })),
        "minValue" => match info.ui {
            Ui::Slider { min, .. } => V::Num(min),
            _ => V::Undef,
        },
        "maxValue" => match info.ui {
            Ui::Slider { max, .. } => V::Num(max),
            _ => V::Undef,
        },
        "canSetEnabled" => V::Bool(false),
        _ => return Ok(None),
    }))
}

fn prop_depth(it: &Interp, c: u64, l: u64, u: u64) -> usize {
    let p = pj(it);
    let Some(layer) = p.comp(c).and_then(|x| x.layer(l)) else { return 0 };
    let mut depth = 0;
    let mut cur = u;
    while let Some(par) = layer.props.parent_of(cur) {
        depth += 1;
        if par.uid == layer.props.uid {
            break;
        }
        cur = par.uid;
    }
    depth
}

fn value_type(info: &PropInfo) -> &'static str {
    match &info.value {
        Value::Num(_) | Value::Bool(_) | Value::Enum(_) => "OneD",
        Value::Vec2(_) => {
            if info.spatial {
                "TwoD_SPATIAL"
            } else {
                "TwoD"
            }
        }
        Value::Vec3(_) => {
            let three = matches!(info.ui, Ui::Point3) || info.is3d;
            match (info.spatial, three) {
                (true, true) => "ThreeD_SPATIAL",
                (true, false) => "TwoD_SPATIAL",
                (false, true) => "ThreeD",
                (false, false) => "TwoD",
            }
        }
        Value::Color(_) => "COLOR",
        Value::Text(_) => "TEXT_DOCUMENT",
        Value::Path(_) => "SHAPE",
        Value::Layer(_) => "LAYER_INDEX",
        Value::Str(_) | Value::Gradient(_) => "CUSTOM_VALUE",
    }
}

fn prop_match_name(id: &str) -> String {
    match id {
        "anchor" => "ADBE Anchor Point",
        "position" => "ADBE Position",
        "scale" => "ADBE Scale",
        "rotation" => "ADBE Rotate Z",
        "rotationX" => "ADBE Rotate X",
        "rotationY" => "ADBE Rotate Y",
        "orientation" => "ADBE Orientation",
        "opacity" => "ADBE Opacity",
        "sourceText" => "ADBE Text Document",
        "path" => "ADBE Mask Shape",
        "feather" => "ADBE Mask Feather",
        "expansion" => "ADBE Mask Offset",
        "timeRemap" => "ADBE Time Remapping",
        other => other,
    }
    .to_string()
}

fn expression_error(it: &Interp, c: u64, l: u64, u: u64) -> String {
    let p = pj(it);
    let Some(comp) = p.comp(c) else { return String::new() };
    let Some(layer) = comp.layer(l) else { return String::new() };
    let Some(pr) = layer.props.find(u) else { return String::new() };
    let now = eng().map(|e| e.time_of(c)).unwrap_or(0.0);
    let ev = Eval::new(p, c, comp, now);
    super::check(&ev, layer, pr).unwrap_or_default()
}

fn value_at(it: &mut Interp, c: u64, l: u64, u: u64, t: f64, pre: bool) -> R {
    let p = pj(it);
    let comp = p.comp(c).ok_or_else(|| throw("Error", "this composition no longer exists"))?;
    let layer = comp.layer(l).ok_or_else(|| throw("Error", "this layer no longer exists"))?;
    let pr = layer.props.find(u).ok_or_else(|| throw("Error", "this property no longer exists"))?;
    let v = if pre { pr.at(layer.layer_time(t)) } else { Eval::new(p, c, comp, t).value(layer, pr) };
    Ok(script_value(c, layer, pr, &v))
}

fn script_value(c: u64, layer: &Layer, pr: &Prop, v: &Value) -> V {
    match v {
        Value::Text(d) => text_doc_v(d),
        Value::Enum(i) => V::Num(*i as f64 + 1.0),
        Value::Bool(b) => V::Num(if *b { 1.0 } else { 0.0 }),
        Value::Vec3(a) if matches!(pr.ui, Ui::Percent) || layer.is_3d() || matches!(pr.ui, Ui::Point3) => nums(a),
        Value::Path(path) => obj(vec![
            ("vertices", arr(path.vertices.iter().map(|q| nums(q)).collect())),
            ("inTangents", arr(path.ins.iter().map(|q| nums(q)).collect())),
            ("outTangents", arr(path.outs.iter().map(|q| nums(q)).collect())),
            ("closed", V::Bool(path.closed)),
        ]),
        other => host::to_v(c, layer, pr, other),
    }
}

fn p_value_at_time(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let t = it.to_number(&arg(&a, 0))?;
    let pre = truthy(&arg(&a, 1));
    value_at(it, c, l, u, t, pre)
}

fn this_prop(this: &V) -> Result<(u64, u64, u64), Flow> {
    match &*this_h(this)? {
        H::Prop(c, l, u) => Ok((*c, *l, *u)),
        _ => Err(type_err("this method belongs to a property")),
    }
}

pub fn text_doc_v(d: &TextDoc) -> V {
    let just = match d.justify {
        Justify::Left => "left",
        Justify::Center => "center",
        Justify::Right => "right",
        Justify::JustifyLeft => "justifyLeft",
        Justify::JustifyCenter => "justifyCenter",
        Justify::JustifyRight => "justifyRight",
        Justify::JustifyAll => "justifyAll",
    };
    let o = obj(vec![
        ("text", s(&d.text)),
        ("font", s(&d.font)),
        ("fontFamily", s(&d.font)),
        ("fontStyle", s(&d.style)),
        ("fontSize", V::Num(d.size)),
        ("fillColor", nums(&d.fill)),
        ("strokeColor", nums(&d.stroke)),
        ("strokeWidth", V::Num(d.stroke_width)),
        ("applyFill", V::Bool(d.apply_fill)),
        ("applyStroke", V::Bool(d.apply_stroke)),
        ("strokeOverFill", V::Bool(d.stroke_over_fill)),
        ("tracking", V::Num(d.tracking)),
        ("leading", V::Num(d.leading.unwrap_or(d.size * 1.2))),
        ("autoLeading", V::Bool(d.leading.is_none())),
        ("justification", s(just)),
        ("fauxBold", V::Bool(d.faux_bold)),
        ("fauxItalic", V::Bool(d.faux_italic)),
        ("allCaps", V::Bool(d.all_caps)),
        ("smallCaps", V::Bool(d.small_caps)),
        ("baselineShift", V::Num(d.baseline_shift)),
        ("horizontalScale", V::Num(d.h_scale)),
        ("verticalScale", V::Num(d.v_scale)),
        ("boxText", V::Bool(d.box_size.is_some())),
        ("pointText", V::Bool(d.box_size.is_none())),
        ("boxTextSize", match d.box_size {
            Some(b) => nums(&[b[2], b[3]]),
            None => V::Undef,
        }),
        ("ligature", V::Bool(d.ligatures)),
    ]);
    add_method(&o, "resetCharStyle", td_reset);
    add_method(&o, "resetParagraphStyle", td_reset);
    o
}

fn td_reset(_: &mut Interp, _: &V, _: Vec<V>) -> R {
    Ok(V::Undef)
}

pub fn text_attrs(it: &mut Interp, v: &V) -> Result<Json, Flow> {
    let V::Obj(o) = v else {
        return Ok(json!({"text": it.to_string(v)?}));
    };
    let props = o.borrow().props.clone();
    let mut out = serde_json::Map::new();
    for (k, x) in props {
        if matches!(x, V::Func(_) | V::Native(_) | V::Undef) {
            continue;
        }
        let j = to_json(it, &x, 1)?;
        let name = match &*k {
            "text" => "text",
            "font" | "fontFamily" => "font",
            "fontStyle" => "style",
            "fontSize" => "size",
            "fillColor" => "fill",
            "strokeColor" => "stroke",
            "strokeWidth" => "strokeWidth",
            "applyFill" => "applyFill",
            "applyStroke" => "applyStroke",
            "strokeOverFill" => "strokeOverFill",
            "tracking" => "tracking",
            "justification" => "justify",
            "fauxBold" => "fauxBold",
            "fauxItalic" => "fauxItalic",
            "allCaps" => "allCaps",
            "smallCaps" => "smallCaps",
            "baselineShift" => "baselineShift",
            "horizontalScale" => "hScale",
            "verticalScale" => "vScale",
            "ligature" => "ligatures",
            "leading" => "leading",
            _ => continue,
        };
        out.insert(name.into(), j);
    }
    if out.get("leading").is_some() && o.borrow().get("autoLeading").is_some_and(truthy) {
        out.insert("leading".into(), json!("auto"));
    }
    if out.get("justify").and_then(Json::as_str) == Some("mixed") {
        out.remove("justify");
    }
    Ok(Json::Object(out))
}

fn set_text_attrs(it: &mut Interp, c: u64, l: u64, attrs: Json) -> Result<(), Flow> {
    exec(it, "prop.set", json!({"comp": c, "layer": l, "path": "text/sourceText", "value": attrs}))?;
    Ok(())
}

fn prop_json(it: &mut Interp, info: &PropInfo, v: &V) -> Result<Json, Flow> {
    Ok(match &info.value {
        Value::Text(_) => text_attrs(it, v)?,
        Value::Enum(_) => json!((it.to_number(v)?.round() - 1.0).max(0.0)),
        Value::Bool(_) => json!(truthy(v) && !matches!(v, V::Num(n) if *n == 0.0)),
        Value::Layer(_) => match v {
            V::Host(h) => match &**h {
                H::Layer(_, x) => json!(x),
                _ => Json::Null,
            },
            V::Num(n) if *n > 0.0 => {
                let p = pj(it);
                let _ = p;
                json!(*n as u64)
            }
            _ => Json::Null,
        },
        Value::Path(_) => match v {
            V::Path(pp) => json!({"vertices": pp.vertices, "in": pp.ins, "out": pp.outs, "closed": pp.closed}),
            other => to_json(it, other, 0)?,
        },
        _ => to_json(it, v, 0)?,
    })
}

fn p_add_to_mgt(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    if let V::Host(h) = arg(&a, 0)
        && let H::Comp(target) = &*h
        && *target != c
    {
        return Ok(V::Bool(false));
    }
    let mut p = json!({"comp": c, "layer": l, "prop": u});
    if let V::Str(n) = arg(&a, 1) {
        p["name"] = json!(&*n);
    }
    Ok(V::Bool(exec(it, "essential.addProperty", p).is_ok()))
}

fn p_can_add_to_mgt(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let same = match arg(&a, 0) {
        V::Host(h) => matches!(&*h, H::Comp(t) if *t == c),
        _ => true,
    };
    Ok(V::Bool(same && !matches!(info.ui, crate::model::prop::Ui::Hidden | crate::model::prop::Ui::Mask | crate::model::prop::Ui::Path)))
}

fn p_set_value(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let j = prop_json(it, &info, &arg(&a, 0))?;
    if !info.keys.is_empty() {
        return Err(throw("Error", "setValue on a property with keyframes: use setValueAtTime or remove the keyframes first"));
    }
    exec(it, "prop.set", json!({"comp": c, "layer": l, "prop": u, "value": j}))?;
    Ok(V::Undef)
}

fn p_set_value_at_time(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let t = it.to_number(&arg(&a, 0))?;
    set_at(it, c, l, u, t, &arg(&a, 1))?;
    Ok(V::Undef)
}

fn set_at(it: &mut Interp, c: u64, l: u64, u: u64, t: f64, v: &V) -> Result<(), Flow> {
    let info = prop_info(it, c, l, u)?;
    if info.static_only {
        return Err(throw("Error", "this property cannot be animated"));
    }
    let j = prop_json(it, &info, v)?;
    exec(it, "prop.addKey", json!({"comp": c, "layer": l, "prop": u, "time": info.layer_time(t), "value": j}))?;
    Ok(())
}

fn p_set_values_at_times(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let ts = it.iterate(&arg(&a, 0))?;
    let vs = it.iterate(&arg(&a, 1))?;
    if ts.len() != vs.len() {
        return Err(throw("Error", "setValuesAtTimes needs as many values as times"));
    }
    for (t, v) in ts.iter().zip(&vs) {
        let tt = it.to_number(t)?;
        set_at(it, c, l, u, tt, v)?;
    }
    Ok(V::Undef)
}

fn key_index(it: &mut Interp, info: &PropInfo, a: &[V]) -> Result<usize, Flow> {
    index_arg(it, a, info.keys.len(), "key")
}

fn p_set_value_at_key(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    let j = prop_json(it, &info, &arg(&a, 1))?;
    exec(it, "keys.set", json!({"comp": c, "layer": l, "prop": u, "time": info.keys[i].time, "value": j}))?;
    Ok(V::Undef)
}

fn p_add_key(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let t = it.to_number(&arg(&a, 0))?;
    let info = prop_info(it, c, l, u)?;
    let lt = info.layer_time(t);
    exec(it, "prop.addKey", json!({"comp": c, "layer": l, "prop": u, "time": lt}))?;
    let after = prop_info(it, c, l, u)?;
    let i = after.keys.iter().enumerate().min_by(|x, y| (x.1.time - lt).abs().total_cmp(&(y.1.time - lt).abs())).map(|(i, _)| i + 1).unwrap_or(0);
    Ok(V::Num(i as f64))
}

fn p_remove_key(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    exec(it, "prop.removeKey", json!({"comp": c, "layer": l, "prop": u, "time": info.keys[i].time}))?;
    Ok(V::Undef)
}

fn p_key_time(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    Ok(V::Num(info.comp_time(info.keys[i].time)))
}

fn p_key_value(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    let p = pj(it);
    let layer = p.comp(c).and_then(|x| x.layer(l)).ok_or_else(|| throw("Error", "no layer"))?;
    let pr = layer.props.find(u).ok_or_else(|| throw("Error", "no property"))?;
    Ok(script_value(c, layer, pr, &info.keys[i].value))
}

fn p_nearest_key_index(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let t = it.to_number(&arg(&a, 0))?;
    let lt = info.layer_time(t);
    Ok(match info.keys.iter().enumerate().min_by(|x, y| (x.1.time - lt).abs().total_cmp(&(y.1.time - lt).abs())) {
        Some((i, _)) => V::Num(i as f64 + 1.0),
        None => return Err(throw("Error", "this property has no keyframes")),
    })
}

fn interp_code(i: crate::model::Interp) -> f64 {
    match i {
        crate::model::Interp::Linear => KIT_LINEAR,
        crate::model::Interp::Bezier => KIT_BEZIER,
        crate::model::Interp::Hold => KIT_HOLD,
    }
}

fn interp_name(it: &mut Interp, v: &V) -> Result<&'static str, Flow> {
    let n = it.to_number(v)?;
    Ok(if n == KIT_HOLD {
        "hold"
    } else if n == KIT_BEZIER {
        "bezier"
    } else {
        "linear"
    })
}

fn p_key_in_interp(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    Ok(V::Num(interp_code(info.keys[i].in_interp)))
}

fn p_key_out_interp(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    Ok(V::Num(interp_code(info.keys[i].out_interp)))
}

fn select_key(it: &mut Interp, c: u64, l: u64, u: u64, time: f64) -> Result<(), Flow> {
    exec(it, "keys.select", json!({"comp": c, "keys": [{"layer": l, "prop": u, "time": time}]}))?;
    Ok(())
}

fn p_set_interp(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    let inn = interp_name(it, &arg(&a, 1))?;
    let out = match arg(&a, 2) {
        V::Undef => inn,
        v => interp_name(it, &v)?,
    };
    select_key(it, c, l, u, info.keys[i].time)?;
    exec(it, "keys.interpolation", json!({"comp": c, "in": inn, "out": out}))?;
    Ok(V::Undef)
}

fn ease_list(info: &PropInfo, k: &crate::model::Key, side_in: bool) -> V {
    let dims = match &info.value {
        Value::Vec2(_) | Value::Vec3(_) if !info.spatial => {
            if matches!(info.value, Value::Vec2(_)) || !(info.is3d || matches!(info.ui, Ui::Point3)) {
                2
            } else {
                3
            }
        }
        Value::Color(_) => 1,
        _ => 1,
    };
    let list = if side_in { &k.in_ease } else { &k.out_ease };
    arr((0..dims)
        .map(|d| {
            let e = list.get(d).or(list.first()).copied().unwrap_or_default();
            obj(vec![("speed", V::Num(e.speed)), ("influence", V::Num(e.influence))])
        })
        .collect())
}

fn p_key_in_ease(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    Ok(ease_list(&info, &info.keys[i], true))
}

fn p_key_out_ease(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    Ok(ease_list(&info, &info.keys[i], false))
}

fn p_set_ease(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    let t = info.keys[i].time;
    let ins = it.iterate(&arg(&a, 1))?;
    let outs = match arg(&a, 2) {
        V::Undef => ins.clone(),
        v => it.iterate(&v)?,
    };
    for (side, list) in [("in", &ins), ("out", &outs)] {
        let multi = list.len() > 1;
        for (d, e) in list.iter().enumerate() {
            let spv = it.get_member(e, "speed")?;
            let sp = it.to_number(&spv)?;
            let infv = it.get_member(e, "influence")?;
            let inf = it.to_number(&infv)?;
            let mut p = json!({"comp": c, "layer": l, "prop": u, "time": t, "side": side, "speed": sp, "influence": inf});
            if multi {
                p["dim"] = json!(d);
            }
            exec(it, "keys.setEase", p)?;
        }
    }
    Ok(V::Undef)
}

fn p_key_in_tangent(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    Ok(nums(&info.keys[i].spatial_in))
}

fn p_key_out_tangent(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    Ok(nums(&info.keys[i].spatial_out))
}

fn p_set_tangents(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    let tin = to_json(it, &arg(&a, 1), 0)?;
    let tout = match arg(&a, 2) {
        V::Undef => tin.clone(),
        v => to_json(it, &v, 0)?,
    };
    exec(it, "keys.setSpatialTangents", json!({"comp": c, "layer": l, "prop": u, "time": info.keys[i].time, "in": tin, "out": tout}))?;
    Ok(V::Undef)
}

fn p_key_roving(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    Ok(V::Bool(info.keys[i].roving))
}

fn p_key_auto(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    Ok(V::Bool(info.keys[i].auto_bezier))
}

fn p_key_continuous(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    Ok(V::Bool(info.keys[i].continuous))
}

fn p_key_label(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    Ok(V::Num(info.keys[i].label as f64))
}

fn key_interp_flag(it: &mut Interp, this: &V, a: Vec<V>, params: fn(bool) -> Json) -> R {
    let (c, l, u) = this_prop(this)?;
    let info = prop_info(it, c, l, u)?;
    let i = key_index(it, &info, &a)?;
    let on = truthy(&arg(&a, 1));
    select_key(it, c, l, u, info.keys[i].time)?;
    let mut p = params(on);
    p["comp"] = json!(c);
    exec(it, "keys.interpolation", p)?;
    Ok(V::Undef)
}

fn p_set_roving(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    key_interp_flag(it, this, a, |on| json!({"roving": on}))
}

fn p_set_auto(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    key_interp_flag(it, this, a, |on| if on { json!({"interpolation": "autoBezier"}) } else { json!({"interpolation": "bezier"}) })
}

fn p_set_continuous(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    key_interp_flag(it, this, a, |on| if on { json!({"interpolation": "continuousBezier"}) } else { json!({"interpolation": "bezier"}) })
}

fn prop_set(it: &mut Interp, c: u64, l: u64, u: u64, key: &str, v: V) -> Result<(), Flow> {
    match key {
        "expression" => {
            let t = it.to_string(&v)?;
            exec(it, "prop.setExpression", json!({"comp": c, "layer": l, "prop": u, "expression": t}))?;
        }
        "expressionEnabled" => {
            exec(it, "prop.setExpression", json!({"comp": c, "layer": l, "prop": u, "enabled": truthy(&v)}))?;
        }
        "selected" => {
            exec(it, "prop.select", json!({"comp": c, "layer": l, "prop": u, "add": truthy(&v)}))?;
        }
        "value" => return Err(type_err("`value` is read-only; use setValue")),
        _ => return Err(type_err(format!("`{key}` cannot be set on a property"))),
    }
    Ok(())
}

fn queue_get(it: &mut Interp, h: &Rc<H>, key: &str) -> Result<Option<V>, Flow> {
    let p = pj(it);
    let ids: Vec<u64> = p.render_queue.iter().map(|r| r.id).collect();
    if let Some(i) = num_key(key) {
        return Ok(Some(ids.get(i.wrapping_sub(1)).map(|id| hv(H::QueueItem(*id))).unwrap_or(V::Undef)));
    }
    Ok(Some(match key {
        "numItems" => V::Num(ids.len() as f64),
        "items" => {
            let q = obj(vec![("length", V::Num(ids.len() as f64))]);
            if let V::Obj(o) = &q {
                for (i, id) in ids.iter().enumerate() {
                    o.borrow_mut().set(&(i + 1).to_string(), hv(H::QueueItem(*id)));
                }
                o.borrow_mut().set("add", native("add", V::Host(h.clone()), queue_add));
            }
            q
        }
        "item" => m(h, "item", queue_item),
        "render" => m(h, "render", queue_render),
        "rendering" => V::Bool(eng()?.render.busy()),
        "canQueueInAME" | "queueNotify" => V::Bool(false),
        "showWindow" | "pauseRendering" | "stopRendering" => m(h, "noop", g_noop),
        _ => return Ok(None),
    }))
}

fn queue_add(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let c = match arg(&a, 0) {
        V::Host(h) => match &*h {
            H::Comp(c) => *c,
            _ => return Err(type_err("renderQueue.items.add takes a composition")),
        },
        _ => return Err(type_err("renderQueue.items.add takes a composition")),
    };
    let before: Vec<u64> = pj(it).render_queue.iter().map(|r| r.id).collect();
    exec(it, "renderQueue.add", json!({"comp": c}))?;
    let id = pj(it).render_queue.iter().map(|r| r.id).find(|x| !before.contains(x));
    Ok(id.map(|x| hv(H::QueueItem(x))).unwrap_or(V::Null))
}

fn queue_item(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let ids: Vec<u64> = pj(it).render_queue.iter().map(|r| r.id).collect();
    let i = index_arg(it, &a, ids.len(), "render queue item")?;
    Ok(hv(H::QueueItem(ids[i])))
}

fn queue_render(it: &mut Interp, _: &V, _: Vec<V>) -> R {
    exec(it, "renderQueue.render", json!({"wait": true}))?;
    Ok(V::Undef)
}

fn queue_item_get(it: &mut Interp, h: &Rc<H>, id: u64, key: &str) -> Result<Option<V>, Flow> {
    let p = pj(it);
    let item = p.render_queue.iter().find(|r| r.id == id).ok_or_else(|| throw("Error", "this render queue item no longer exists"))?;
    Ok(Some(match key {
        "comp" => hv(H::Comp(item.comp)),
        "render" => V::Bool(item.render),
        "timeSpanStart" => V::Num(item.start),
        "timeSpanDuration" => V::Num(item.end - item.start),
        "status" => s(if item.render { "queued" } else { "unqueued" }),
        "numOutputModules" => V::Num(1.0),
        "outputModule" => m(h, "outputModule", qi_output_module),
        "remove" => m(h, "remove", qi_remove),
        "duplicate" => m(h, "duplicate", qi_duplicate),
        "applyTemplate" => m(h, "applyTemplate", qi_apply_template),
        _ => return Ok(None),
    }))
}

fn qi_id(this: &V) -> Result<u64, Flow> {
    match &*this_h(this)? {
        H::QueueItem(id) => Ok(*id),
        _ => Err(type_err("this method belongs to a render queue item")),
    }
}

fn qi_output_module(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    let id = qi_id(this)?;
    let o = obj(vec![("_item", V::Num(id as f64))]);
    add_method(&o, "applyTemplate", om_apply_template);
    add_method(&o, "setSettings", om_set_settings);
    if let V::Obj(ob) = &o {
        ob.borrow_mut().set("file", V::Null);
    }
    Ok(o)
}

fn om_apply_template(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let id = field(this, "_item");
    let t = it.to_string(&arg(&a, 0))?;
    let idj = to_json(it, &id, 0)?;
    exec(it, "renderQueue.applyTemplate", json!({"item": idj, "outputModule": t}))?;
    Ok(V::Undef)
}

fn om_set_settings(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let id = field(this, "_item");
    let mut settings = to_json(it, &arg(&a, 0), 0)?;
    if let Some(o) = settings.as_object_mut() {
        if let Some(f) = o.remove("Output File Info").and_then(|x| x.get("Full Flat Path").cloned()) {
            o.insert("output".into(), f);
        }
        o.insert("item".into(), to_json(it, &id, 0)?);
    }
    exec(it, "renderQueue.setOutputModule", settings)?;
    Ok(V::Undef)
}

fn qi_remove(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let id = qi_id(this)?;
    exec(it, "renderQueue.remove", json!({"item": id}))?;
    Ok(V::Undef)
}

fn qi_duplicate(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let id = qi_id(this)?;
    let before: Vec<u64> = pj(it).render_queue.iter().map(|r| r.id).collect();
    exec(it, "renderQueue.duplicate", json!({"item": id}))?;
    let new = pj(it).render_queue.iter().map(|r| r.id).find(|x| !before.contains(x));
    Ok(new.map(|x| hv(H::QueueItem(x))).unwrap_or(V::Null))
}

fn qi_apply_template(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let id = qi_id(this)?;
    let t = it.to_string(&arg(&a, 0))?;
    exec(it, "renderQueue.applyTemplate", json!({"item": id, "renderSettings": t}))?;
    Ok(V::Undef)
}
