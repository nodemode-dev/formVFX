use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use parking_lot::RwLock;
use serde_json::{Value as Json, json};
use wasmi::{Caller, Config, Engine as WasmEngine, Extern, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

use super::{EffectSpec, FxCtx, ParamSpec, hidden, no_pad, p};
use crate::model::Value;
use crate::model::prop::Ui;
use crate::render::image::Img;

pub const CAT: &str = "Plug-ins";
pub const ABI: i32 = 1;
const MEMORY_LIMIT: usize = 2 << 30;

pub struct Plugin {
    pub id: String,
    pub name: String,
    pub category: String,
    pub vendor: String,
    pub version: String,
    pub path: String,
    module: Module,
    pub spec: &'static EffectSpec,
}

struct Data {
    limits: StoreLimits,
    log: String,
}

fn wasm() -> &'static WasmEngine {
    static E: OnceLock<WasmEngine> = OnceLock::new();
    E.get_or_init(|| {
        let mut c = Config::default();
        c.consume_fuel(true);
        WasmEngine::new(&c)
    })
}

fn registry() -> &'static RwLock<Vec<Arc<Plugin>>> {
    static R: OnceLock<RwLock<Vec<Arc<Plugin>>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

fn failures() -> &'static RwLock<Vec<(String, String)>> {
    static F: OnceLock<RwLock<Vec<(String, String)>>> = OnceLock::new();
    F.get_or_init(Default::default)
}

pub fn folder() -> PathBuf {
    crate::render::diskcache::data_dir().join("Plug-ins")
}

pub fn specs() -> Vec<&'static EffectSpec> {
    registry().read().iter().map(|p| p.spec).collect()
}

pub fn loaded() -> Vec<Arc<Plugin>> {
    registry().read().clone()
}

pub fn errors() -> Vec<(String, String)> {
    failures().read().clone()
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

fn start(module: &Module, fuel: u64) -> Result<(Store<Data>, Instance), String> {
    let mut store = Store::new(wasm(), Data { limits: StoreLimitsBuilder::new().memory_size(MEMORY_LIMIT).build(), log: String::new() });
    store.limiter(|d| &mut d.limits);
    store.set_fuel(fuel).map_err(|e| e.to_string())?;
    let mut linker: Linker<Data> = Linker::new(wasm());
    linker
        .func_wrap("env", "fvx_log", |caller: Caller<'_, Data>, ptr: i32, len: i32| {
            let mem = caller.get_export("memory").and_then(Extern::into_memory);
            if let Some(m) = mem {
                let mut buf = vec![0u8; len.clamp(0, 4096) as usize];
                if m.read(&caller, ptr as usize, &mut buf).is_ok() {
                    let text = String::from_utf8_lossy(&buf).to_string();
                    let mut caller = caller;
                    let d = caller.data_mut();
                    if d.log.len() < 16384 {
                        d.log.push_str(&text);
                        d.log.push('\n');
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;
    let inst = linker.instantiate_and_start(&mut store, module).map_err(|e| e.to_string())?;
    Ok((store, inst))
}

fn read_out(store: &Store<Data>, inst: &Instance, packed: i64) -> Result<Vec<u8>, String> {
    let mem = inst.get_memory(store, "memory").ok_or("the plug-in exports no `memory`")?;
    let (ptr, len) = ((packed as u64 >> 32) as usize, (packed as u64 & 0xffff_ffff) as usize);
    let mut buf = vec![0u8; len];
    mem.read(store, ptr, &mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

fn num(j: &Json, k: &str, d: f64) -> f64 {
    j.get(k).and_then(Json::as_f64).unwrap_or(d)
}

fn param(j: &Json) -> Result<ParamSpec, String> {
    let id = j.get("id").and_then(Json::as_str).ok_or("every parameter needs an `id`")?;
    if id.is_empty() || id.contains('/') || id == "plugin" {
        return Err(format!("parameter id `{id}` is not allowed"));
    }
    let name = j.get("name").and_then(Json::as_str).unwrap_or(id);
    let kind = j.get("type").and_then(Json::as_str).unwrap_or("slider");
    let def = j.get("default");
    let (v, ui) = match kind {
        "slider" | "number" => {
            let (lo, hi) = (num(j, "min", 0.0), num(j, "max", 100.0));
            let d = def.and_then(Json::as_f64).unwrap_or(lo);
            (Value::Num(d), Ui::slider(lo, hi, num(j, "sliderMin", lo), num(j, "sliderMax", hi), num(j, "decimals", 1.0) as u8))
        }
        "angle" => (Value::Num(def.and_then(Json::as_f64).unwrap_or(0.0)), Ui::Angle),
        "checkbox" => (Value::Bool(def.and_then(Json::as_bool).unwrap_or(false)), Ui::Checkbox),
        "color" => {
            let c = def.and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).collect::<Vec<_>>()).unwrap_or_default();
            let g = |i: usize, d: f64| c.get(i).copied().unwrap_or(d);
            (Value::Color([g(0, 1.0), g(1, 1.0), g(2, 1.0), g(3, 1.0)]), Ui::Color)
        }
        "point" => {
            let c = def.and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).collect::<Vec<_>>()).unwrap_or_default();
            (Value::Vec2([c.first().copied().unwrap_or(0.0), c.get(1).copied().unwrap_or(0.0)]), Ui::Point)
        }
        "popup" => {
            let opts: Vec<String> = j.get("options").and_then(Json::as_array).map(|a| a.iter().filter_map(|o| o.as_str().map(str::to_string)).collect()).unwrap_or_default();
            if opts.is_empty() {
                return Err(format!("popup `{id}` needs `options`"));
            }
            (Value::Enum(def.and_then(Json::as_u64).unwrap_or(0) as u32), Ui::Popup { options: opts })
        }
        "layer" => (Value::Layer(None), Ui::Layer),
        other => return Err(format!("parameter type `{other}` is not supported (slider, angle, checkbox, color, point, popup, layer)")),
    };
    Ok(p(leak(id), leak(name), v, ui))
}

pub fn load(path: &str) -> Result<Arc<Plugin>, String> {
    let r = load_inner(path);
    let mut f = failures().write();
    f.retain(|x| x.0 != path);
    if let Err(e) = &r {
        f.push((path.to_string(), e.clone()));
    }
    r
}

fn load_inner(path: &str) -> Result<Arc<Plugin>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let module = Module::new(wasm(), &bytes).map_err(|e| format!("{path}: {e}"))?;
    let (mut store, inst) = start(&module, 50_000_000).map_err(|e| format!("{path}: {e}"))?;
    if let Ok(f) = inst.get_typed_func::<(), i32>(&store, "fvx_abi") {
        let v = f.call(&mut store, ()).map_err(|e| format!("{path}: {e}"))?;
        if v != ABI {
            return Err(format!("{path}: plug-in ABI {v}, formvfx speaks {ABI}"));
        }
    }
    let describe = inst.get_typed_func::<(), i64>(&store, "fvx_describe").map_err(|_| format!("{path}: no `fvx_describe` export (see docs/PLUGINS.md)"))?;
    inst.get_typed_func::<(i32, i32, i32, i32, i32, f64), i32>(&store, "fvx_render").map_err(|_| format!("{path}: no `fvx_render(px, w, h, params, len, time) -> i32` export"))?;
    inst.get_typed_func::<i32, i32>(&store, "fvx_alloc").map_err(|_| format!("{path}: no `fvx_alloc(size) -> ptr` export"))?;
    let packed = describe.call(&mut store, ()).map_err(|e| format!("{path}: {e}"))?;
    let doc: Json = serde_json::from_slice(&read_out(&store, &inst, packed)?).map_err(|e| format!("{path}: the description is not JSON: {e}"))?;
    let raw = doc.get("id").and_then(Json::as_str).ok_or_else(|| format!("{path}: the description needs an `id`"))?;
    if raw.is_empty() || !raw.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') {
        return Err(format!("{path}: plug-in id `{raw}` must be letters, digits, dots, dashes or underscores"));
    }
    let id = format!("plugin.{raw}");
    let name = doc.get("name").and_then(Json::as_str).unwrap_or(raw).to_string();
    let category = doc.get("category").and_then(Json::as_str).filter(|c| !c.is_empty()).unwrap_or(CAT).to_string();
    let mut params = vec![hidden("plugin", "Plug-in", &id)];
    for j in doc.get("params").and_then(Json::as_array).cloned().unwrap_or_default() {
        params.push(param(&j).map_err(|e| format!("{path}: {e}"))?);
    }
    if super::all().iter().any(|e| e.name == name || e.id == id) {
        return Err(format!("{path}: `{name}` is the name of a built-in effect"));
    }
    let spec: &'static EffectSpec = Box::leak(Box::new(EffectSpec { id: leak(&id), name: leak(&name), category: leak(&category), params, render, pad: no_pad }));
    let plugin = Arc::new(Plugin {
        id: id.clone(),
        name,
        category,
        vendor: doc.get("vendor").and_then(Json::as_str).unwrap_or("").to_string(),
        version: doc.get("version").and_then(Json::as_str).unwrap_or("").to_string(),
        path: path.to_string(),
        module,
        spec,
    });
    let mut reg = registry().write();
    reg.retain(|p| p.id != id);
    reg.push(plugin.clone());
    Ok(plugin)
}

pub fn load_folder(dir: &Path) -> (usize, Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return (0, vec![]) };
    let mut paths: Vec<PathBuf> = rd.filter_map(|x| x.ok().map(|x| x.path())).filter(|x| x.extension().is_some_and(|e| e.eq_ignore_ascii_case("wasm") || e.eq_ignore_ascii_case("wat"))).collect();
    paths.sort();
    let mut n = 0;
    let mut errs = vec![];
    for p in paths {
        match load(&p.to_string_lossy()) {
            Ok(_) => n += 1,
            Err(e) => errs.push(e),
        }
    }
    (n, errs)
}

pub fn find(id: &str) -> Option<Arc<Plugin>> {
    registry().read().iter().find(|p| p.id == id).cloned()
}

fn params_json(c: &FxCtx) -> Json {
    let mut values = serde_json::Map::new();
    for (k, v) in c.p.iter() {
        if k != "plugin" {
            values.insert(k.clone(), v.to_json());
        }
    }
    json!({"time": c.t, "scale": c.r, "origin": c.origin, "layerSize": c.size, "fps": c.fps, "values": values})
}

pub fn run(plugin: &Plugin, c: &FxCtx, img: &mut Img) -> Result<String, String> {
    let (w, h) = (img.w, img.h);
    let fuel = (w * h) as u64 * 4000 + 2_000_000_000;
    let (mut store, inst) = start(&plugin.module, fuel)?;
    let alloc = inst.get_typed_func::<i32, i32>(&store, "fvx_alloc").map_err(|e| e.to_string())?;
    let render = inst.get_typed_func::<(i32, i32, i32, i32, i32, f64), i32>(&store, "fvx_render").map_err(|e| e.to_string())?;
    let mem = inst.get_memory(&store, "memory").ok_or("the plug-in exports no `memory`")?;
    let bytes: Vec<u8> = img.px.iter().flat_map(|v| v.to_le_bytes()).collect();
    let pj = serde_json::to_vec(&params_json(c)).map_err(|e| e.to_string())?;
    let px = alloc.call(&mut store, bytes.len() as i32).map_err(|e| e.to_string())?;
    let pp = alloc.call(&mut store, pj.len() as i32).map_err(|e| e.to_string())?;
    if px <= 0 || pp <= 0 {
        return Err("the plug-in could not allocate its buffers".into());
    }
    mem.write(&mut store, px as usize, &bytes).map_err(|e| e.to_string())?;
    mem.write(&mut store, pp as usize, &pj).map_err(|e| e.to_string())?;
    let code = render.call(&mut store, (px, w as i32, h as i32, pp, pj.len() as i32, c.t)).map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(format!("the plug-in returned {code}"));
    }
    let mut out = vec![0u8; bytes.len()];
    mem.read(&store, px as usize, &mut out).map_err(|e| e.to_string())?;
    for (d, s) in img.px.iter_mut().zip(out.chunks_exact(4)) {
        let v = f32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        *d = if v.is_finite() { v } else { 0.0 };
    }
    Ok(std::mem::take(&mut store.data_mut().log))
}

fn render(c: &FxCtx, img: &mut Img) {
    let Some(plugin) = find(&c.s("plugin")) else { return };
    let r = run(&plugin, c, img);
    if let Ok(log) = &r
        && !log.is_empty()
    {
        eprint!("{}", log.lines().map(|l| format!("[{}] {l}\n", plugin.id)).collect::<String>());
    }
    let mut f = failures().write();
    f.retain(|x| x.0 != plugin.id);
    if let Err(e) = r {
        f.push((plugin.id.clone(), e));
    }
}
