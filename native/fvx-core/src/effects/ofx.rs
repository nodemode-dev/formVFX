use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use parking_lot::{Condvar, Mutex, RwLock};

use super::{EffectSpec, FxCtx, Init, ParamSpec, hidden};
use crate::model::Value;
use crate::model::prop::Ui;
use crate::render::image::Img;

pub const CAT: &str = "OpenFX";

type Status = c_int;
const OK: Status = 0;
const FAILED: Status = 1;
const ERR_UNKNOWN: Status = 3;
const ERR_UNSUPPORTED: Status = 5;
const ERR_MEMORY: Status = 8;
const ERR_BAD_HANDLE: Status = 9;
const ERR_BAD_INDEX: Status = 10;
const ERR_VALUE: Status = 11;
const REPLY_DEFAULT: Status = 14;

#[derive(Clone)]
enum Pv {
    Ptr(Vec<usize>),
    Str(Vec<CString>),
    Dbl(Vec<f64>),
    Int(Vec<c_int>),
}

#[derive(Default)]
pub struct Props {
    map: Mutex<HashMap<String, Pv>>,
}

impl Clone for Props {
    fn clone(&self) -> Props {
        Props { map: Mutex::new(self.map.lock().clone()) }
    }
}

fn cs(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

impl Props {
    fn set(&self, k: &str, v: Pv) {
        self.map.lock().insert(k.to_string(), v);
    }
    fn s(&self, k: &str, v: &str) {
        self.set(k, Pv::Str(vec![cs(v)]));
    }
    fn ss(&self, k: &str, v: &[&str]) {
        self.set(k, Pv::Str(v.iter().map(|x| cs(x)).collect()));
    }
    fn i(&self, k: &str, v: &[c_int]) {
        self.set(k, Pv::Int(v.to_vec()));
    }
    fn d(&self, k: &str, v: &[f64]) {
        self.set(k, Pv::Dbl(v.to_vec()));
    }
    fn p(&self, k: &str, v: usize) {
        self.set(k, Pv::Ptr(vec![v]));
    }
    fn get_s(&self, k: &str) -> Vec<String> {
        match self.map.lock().get(k) {
            Some(Pv::Str(v)) => v.iter().map(|c| c.to_string_lossy().to_string()).collect(),
            _ => vec![],
        }
    }
    fn get_d(&self, k: &str) -> Vec<f64> {
        match self.map.lock().get(k) {
            Some(Pv::Dbl(v)) => v.clone(),
            Some(Pv::Int(v)) => v.iter().map(|x| *x as f64).collect(),
            _ => vec![],
        }
    }
    fn get_i(&self, k: &str) -> Vec<c_int> {
        match self.map.lock().get(k) {
            Some(Pv::Int(v)) => v.clone(),
            Some(Pv::Dbl(v)) => v.iter().map(|x| *x as c_int).collect(),
            _ => vec![],
        }
    }
}

struct Clip {
    name: String,
    props: Props,
    buf: Mutex<Option<(usize, usize, usize, usize)>>,
}

struct Param {
    name: String,
    kind: String,
    props: Props,
    val: Mutex<Vec<f64>>,
    text: Mutex<CString>,
}

struct ParamSet {
    props: Props,
    params: Mutex<Vec<Box<Param>>>,
}

struct Effect {
    props: Props,
    params: ParamSet,
    clips: Mutex<Vec<Box<Clip>>>,
}

type Entry = unsafe extern "C" fn(*const c_char, *const c_void, *mut Props, *mut Props) -> Status;

#[repr(C)]
struct RawPlugin {
    api: *const c_char,
    api_version: c_int,
    identifier: *const c_char,
    major: c_uint,
    minor: c_uint,
    set_host: Option<unsafe extern "C" fn(*mut RawHost)>,
    main: Option<Entry>,
}

#[repr(C)]
struct RawHost {
    host: *mut Props,
    fetch: unsafe extern "C" fn(*mut Props, *const c_char, c_int) -> *const c_void,
}

pub struct ParamMap {
    ofx: String,
    id: &'static str,
    kind: String,
    norm: bool,
    point: bool,
}

pub struct Plugin {
    pub id: String,
    pub name: String,
    pub category: String,
    pub identifier: String,
    pub version: String,
    pub path: String,
    pub context: String,
    pub depth: &'static str,
    main: Entry,
    desc: Box<Effect>,
    map: Vec<ParamMap>,
    unsafe_render: Option<Mutex<()>>,
    pool: Mutex<Vec<Pooled>>,
    rendered: std::sync::atomic::AtomicBool,
    pub spec: &'static EffectSpec,
}

impl Drop for Plugin {
    fn drop(&mut self) {
        let all: Vec<Pooled> = self.pool.lock().drain(..).collect();
        for p in all {
            destroy(self, p);
        }
    }
}

unsafe impl Send for Plugin {}
unsafe impl Sync for Plugin {}

fn registry() -> &'static RwLock<Vec<Arc<Plugin>>> {
    static R: OnceLock<RwLock<Vec<Arc<Plugin>>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

fn failures() -> &'static RwLock<Vec<(String, String)>> {
    static F: OnceLock<RwLock<Vec<(String, String)>>> = OnceLock::new();
    F.get_or_init(Default::default)
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

pub fn find(id: &str) -> Option<Arc<Plugin>> {
    registry().read().iter().find(|p| p.id == id).cloned()
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

unsafe fn key<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(p) }.to_str().ok()
}

unsafe fn props<'a>(h: *mut Props) -> Option<&'a Props> {
    unsafe { h.as_ref() }
}

macro_rules! guard {
    ($e:expr) => {
        match $e {
            Some(v) => v,
            None => return ERR_BAD_HANDLE,
        }
    };
}

unsafe extern "C" fn set_ptr(h: *mut Props, k: *const c_char, i: c_int, v: *mut c_void) -> Status {
    unsafe { set_one(h, k, i, |e| if let Pv::Ptr(x) = e { Some(x) } else { None }, || Pv::Ptr(vec![]), v as usize) }
}

unsafe extern "C" fn set_str(h: *mut Props, k: *const c_char, i: c_int, v: *const c_char) -> Status {
    let s = if v.is_null() { CString::default() } else { unsafe { CStr::from_ptr(v) }.to_owned() };
    unsafe { set_one(h, k, i, |e| if let Pv::Str(x) = e { Some(x) } else { None }, || Pv::Str(vec![]), s) }
}

unsafe extern "C" fn set_dbl(h: *mut Props, k: *const c_char, i: c_int, v: f64) -> Status {
    unsafe { set_one(h, k, i, |e| if let Pv::Dbl(x) = e { Some(x) } else { None }, || Pv::Dbl(vec![]), v) }
}

unsafe extern "C" fn set_int(h: *mut Props, k: *const c_char, i: c_int, v: c_int) -> Status {
    unsafe { set_one(h, k, i, |e| if let Pv::Int(x) = e { Some(x) } else { None }, || Pv::Int(vec![]), v) }
}

unsafe fn set_one<T: Clone + Default>(h: *mut Props, k: *const c_char, i: c_int, pick: impl Fn(&mut Pv) -> Option<&mut Vec<T>>, fresh: impl Fn() -> Pv, v: T) -> Status {
    let ps = guard!(unsafe { props(h) });
    let k = guard!(unsafe { key(k) });
    if i < 0 {
        return ERR_BAD_INDEX;
    }
    let mut m = ps.map.lock();
    let e = m.entry(k.to_string()).or_insert_with(&fresh);
    if pick(e).is_none() {
        *e = fresh();
    }
    let Some(vec) = pick(e) else { return ERR_VALUE };
    let i = i as usize;
    if vec.len() <= i {
        vec.resize(i + 1, T::default());
    }
    vec[i] = v;
    OK
}

unsafe extern "C" fn set_ptr_n(h: *mut Props, k: *const c_char, n: c_int, v: *const *mut c_void) -> Status {
    for i in 0..n.max(0) {
        let s = unsafe { set_ptr(h, k, i, *v.add(i as usize)) };
        if s != OK {
            return s;
        }
    }
    OK
}

unsafe extern "C" fn set_str_n(h: *mut Props, k: *const c_char, n: c_int, v: *const *const c_char) -> Status {
    for i in 0..n.max(0) {
        let s = unsafe { set_str(h, k, i, *v.add(i as usize)) };
        if s != OK {
            return s;
        }
    }
    OK
}

unsafe extern "C" fn set_dbl_n(h: *mut Props, k: *const c_char, n: c_int, v: *const f64) -> Status {
    for i in 0..n.max(0) {
        let s = unsafe { set_dbl(h, k, i, *v.add(i as usize)) };
        if s != OK {
            return s;
        }
    }
    OK
}

unsafe extern "C" fn set_int_n(h: *mut Props, k: *const c_char, n: c_int, v: *const c_int) -> Status {
    for i in 0..n.max(0) {
        let s = unsafe { set_int(h, k, i, *v.add(i as usize)) };
        if s != OK {
            return s;
        }
    }
    OK
}

unsafe extern "C" fn get_ptr(h: *mut Props, k: *const c_char, i: c_int, out: *mut *mut c_void) -> Status {
    let ps = guard!(unsafe { props(h) });
    let k = guard!(unsafe { key(k) });
    let m = ps.map.lock();
    match m.get(k) {
        Some(Pv::Ptr(v)) => match v.get(i.max(0) as usize) {
            Some(x) if !out.is_null() => {
                unsafe { *out = *x as *mut c_void };
                OK
            }
            _ => ERR_BAD_INDEX,
        },
        Some(_) => ERR_VALUE,
        None => ERR_UNKNOWN,
    }
}

unsafe extern "C" fn get_str(h: *mut Props, k: *const c_char, i: c_int, out: *mut *const c_char) -> Status {
    let ps = guard!(unsafe { props(h) });
    let k = guard!(unsafe { key(k) });
    let m = ps.map.lock();
    match m.get(k) {
        Some(Pv::Str(v)) => match v.get(i.max(0) as usize) {
            Some(x) if !out.is_null() => {
                unsafe { *out = x.as_ptr() };
                OK
            }
            _ => ERR_BAD_INDEX,
        },
        Some(_) => ERR_VALUE,
        None => ERR_UNKNOWN,
    }
}

unsafe extern "C" fn get_dbl(h: *mut Props, k: *const c_char, i: c_int, out: *mut f64) -> Status {
    let ps = guard!(unsafe { props(h) });
    let k = guard!(unsafe { key(k) });
    let m = ps.map.lock();
    let v = match m.get(k) {
        Some(Pv::Dbl(v)) => v.get(i.max(0) as usize).copied(),
        Some(Pv::Int(v)) => v.get(i.max(0) as usize).map(|x| *x as f64),
        Some(_) => return ERR_VALUE,
        None => return ERR_UNKNOWN,
    };
    match v {
        Some(x) if !out.is_null() => {
            unsafe { *out = x };
            OK
        }
        _ => ERR_BAD_INDEX,
    }
}

unsafe extern "C" fn get_int(h: *mut Props, k: *const c_char, i: c_int, out: *mut c_int) -> Status {
    let ps = guard!(unsafe { props(h) });
    let k = guard!(unsafe { key(k) });
    let m = ps.map.lock();
    let v = match m.get(k) {
        Some(Pv::Int(v)) => v.get(i.max(0) as usize).copied(),
        Some(Pv::Dbl(v)) => v.get(i.max(0) as usize).map(|x| *x as c_int),
        Some(_) => return ERR_VALUE,
        None => return ERR_UNKNOWN,
    };
    match v {
        Some(x) if !out.is_null() => {
            unsafe { *out = x };
            OK
        }
        _ => ERR_BAD_INDEX,
    }
}

unsafe extern "C" fn get_ptr_n(h: *mut Props, k: *const c_char, n: c_int, out: *mut *mut c_void) -> Status {
    for i in 0..n.max(0) {
        let s = unsafe { get_ptr(h, k, i, out.add(i as usize)) };
        if s != OK {
            return s;
        }
    }
    OK
}

unsafe extern "C" fn get_str_n(h: *mut Props, k: *const c_char, n: c_int, out: *mut *const c_char) -> Status {
    for i in 0..n.max(0) {
        let s = unsafe { get_str(h, k, i, out.add(i as usize)) };
        if s != OK {
            return s;
        }
    }
    OK
}

unsafe extern "C" fn get_dbl_n(h: *mut Props, k: *const c_char, n: c_int, out: *mut f64) -> Status {
    for i in 0..n.max(0) {
        let s = unsafe { get_dbl(h, k, i, out.add(i as usize)) };
        if s != OK {
            return s;
        }
    }
    OK
}

unsafe extern "C" fn get_int_n(h: *mut Props, k: *const c_char, n: c_int, out: *mut c_int) -> Status {
    for i in 0..n.max(0) {
        let s = unsafe { get_int(h, k, i, out.add(i as usize)) };
        if s != OK {
            return s;
        }
    }
    OK
}

unsafe extern "C" fn prop_reset(h: *mut Props, k: *const c_char) -> Status {
    let ps = guard!(unsafe { props(h) });
    let k = guard!(unsafe { key(k) });
    if ps.map.lock().contains_key(k) { OK } else { ERR_UNKNOWN }
}

unsafe extern "C" fn prop_dim(h: *mut Props, k: *const c_char, out: *mut c_int) -> Status {
    let ps = guard!(unsafe { props(h) });
    let k = guard!(unsafe { key(k) });
    let n = match ps.map.lock().get(k) {
        Some(Pv::Ptr(v)) => v.len(),
        Some(Pv::Str(v)) => v.len(),
        Some(Pv::Dbl(v)) => v.len(),
        Some(Pv::Int(v)) => v.len(),
        None => return ERR_UNKNOWN,
    };
    if !out.is_null() {
        unsafe { *out = n as c_int };
    }
    OK
}

fn fp(f: *const ()) -> usize {
    f as usize
}

#[repr(C)]
struct PropertySuite {
    f: [usize; 18],
}

fn property_suite() -> &'static PropertySuite {
    static S: OnceLock<PropertySuite> = OnceLock::new();
    S.get_or_init(|| PropertySuite {
    f: [
        fp(set_ptr as *const ()),
        fp(set_str as *const ()),
        fp(set_dbl as *const ()),
        fp(set_int as *const ()),
        fp(set_ptr_n as *const ()),
        fp(set_str_n as *const ()),
        fp(set_dbl_n as *const ()),
        fp(set_int_n as *const ()),
        fp(get_ptr as *const ()),
        fp(get_str as *const ()),
        fp(get_dbl as *const ()),
        fp(get_int as *const ()),
        fp(get_ptr_n as *const ()),
        fp(get_str_n as *const ()),
        fp(get_dbl_n as *const ()),
        fp(get_int_n as *const ()),
        fp(prop_reset as *const ()),
        fp(prop_dim as *const ()),
    ],
    })
}

unsafe fn effect<'a>(h: *mut c_void) -> Option<&'a Effect> {
    unsafe { (h as *mut Effect).as_ref() }
}

unsafe extern "C" fn fx_props(h: *mut c_void, out: *mut *mut Props) -> Status {
    let e = guard!(unsafe { effect(h) });
    unsafe { *out = &e.props as *const Props as *mut Props };
    OK
}

unsafe extern "C" fn fx_params(h: *mut c_void, out: *mut *mut ParamSet) -> Status {
    let e = guard!(unsafe { effect(h) });
    unsafe { *out = &e.params as *const ParamSet as *mut ParamSet };
    OK
}

fn new_clip(name: &str) -> Box<Clip> {
    let props = Props::default();
    props.s("OfxPropType", "OfxTypeClip");
    props.s("OfxPropName", name);
    props.s("OfxPropLabel", name);
    props.ss("OfxImageEffectPropSupportedComponents", &["OfxImageComponentRGBA"]);
    props.i("OfxImageEffectPropTemporalClipAccess", &[0]);
    props.i("OfxImageClipPropOptional", &[0]);
    props.i("OfxImageClipPropIsMask", &[0]);
    props.s("OfxImageClipPropFieldExtraction", "OfxImageFieldDoubled");
    props.i("OfxImageEffectPropSupportsTiles", &[1]);
    Box::new(Clip { name: name.to_string(), props, buf: Mutex::new(None) })
}

unsafe extern "C" fn clip_define(h: *mut c_void, name: *const c_char, out: *mut *mut Props) -> Status {
    let e = guard!(unsafe { effect(h) });
    let name = guard!(unsafe { key(name) });
    let mut clips = e.clips.lock();
    if !clips.iter().any(|c| c.name == name) {
        clips.push(new_clip(name));
    }
    let c = clips.iter().find(|c| c.name == name).map(|c| &c.props as *const Props as *mut Props);
    if !out.is_null() {
        unsafe { *out = c.unwrap_or(std::ptr::null_mut()) };
    }
    OK
}

unsafe extern "C" fn clip_handle(h: *mut c_void, name: *const c_char, clip: *mut *mut Clip, out: *mut *mut Props) -> Status {
    let e = guard!(unsafe { effect(h) });
    let name = guard!(unsafe { key(name) });
    let clips = e.clips.lock();
    let Some(c) = clips.iter().find(|c| c.name == name) else { return ERR_UNKNOWN };
    if !clip.is_null() {
        unsafe { *clip = &**c as *const Clip as *mut Clip };
    }
    if !out.is_null() {
        unsafe { *out = &c.props as *const Props as *mut Props };
    }
    OK
}

unsafe extern "C" fn clip_props(c: *mut Clip, out: *mut *mut Props) -> Status {
    let c = guard!(unsafe { c.as_ref() });
    unsafe { *out = &c.props as *const Props as *mut Props };
    OK
}

#[repr(C)]
struct RectD {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

fn other_frame(t: f64, w: usize, h: usize) -> Option<usize> {
    let live = RENDER.with(|r| r.get()).and_then(|p| unsafe { (p as *const Live).as_ref() })?;
    if (t - live.frame).abs() < 1e-6 {
        return None;
    }
    let c = unsafe { &*live.fx };
    let img = c.frame(t / c.fps.max(1e-6) - c.t).filter(|i| i.w == w && i.h == h)?;
    let b = Bufs::new(live.depth, &img.px);
    let p = b.src();
    live.arena.lock().push(b);
    Some(p)
}

unsafe extern "C" fn clip_image(c: *mut Clip, t: f64, _region: *const RectD, out: *mut *mut Props) -> Status {
    let c = guard!(unsafe { c.as_ref() });
    let Some((ptr, w, h, row)) = *c.buf.lock() else { return FAILED };
    if ptr == 0 {
        return FAILED;
    }
    let ptr = if c.name == "Source" { other_frame(t, w, h).unwrap_or(ptr) } else { ptr };
    let img = Box::new(c.props.clone());
    img.s("OfxPropType", "OfxTypeImage");
    img.p("OfxImagePropData", ptr + (h.saturating_sub(1)) * row);
    img.i("OfxImagePropRowBytes", &[-(row as c_int)]);
    img.i("OfxImagePropBounds", &[0, 0, w as c_int, h as c_int]);
    img.i("OfxImagePropRegionOfDefinition", &[0, 0, w as c_int, h as c_int]);
    img.s("OfxImagePropField", "OfxFieldNone");
    img.s("OfxImagePropUniqueIdentifier", &format!("{}:{ptr}", c.name));
    if !out.is_null() {
        unsafe { *out = Box::into_raw(img) };
    }
    OK
}

unsafe extern "C" fn clip_release(img: *mut Props) -> Status {
    if img.is_null() {
        return ERR_BAD_HANDLE;
    }
    drop(unsafe { Box::from_raw(img) });
    OK
}

unsafe extern "C" fn clip_rod(c: *mut Clip, _t: f64, out: *mut RectD) -> Status {
    let c = guard!(unsafe { c.as_ref() });
    let s = c.props.get_d("OfxImageEffectPropRenderScale");
    let r = s.first().copied().filter(|x| *x > 0.0).unwrap_or(1.0);
    let Some((_, w, h, _)) = *c.buf.lock() else { return FAILED };
    if !out.is_null() {
        unsafe { *out = RectD { x1: 0.0, y1: 0.0, x2: w as f64 / r, y2: h as f64 / r } };
    }
    OK
}

unsafe extern "C" fn fx_abort(_h: *mut c_void) -> c_int {
    0
}

unsafe extern "C" fn image_alloc(_h: *mut c_void, n: usize, out: *mut *mut c_void) -> Status {
    let p = unsafe { mem_alloc(n) };
    if p.is_null() {
        return ERR_MEMORY;
    }
    unsafe { *out = p };
    OK
}

unsafe extern "C" fn image_free(p: *mut c_void) -> Status {
    unsafe { mem_free(p) };
    OK
}

unsafe extern "C" fn image_lock(p: *mut c_void, out: *mut *mut c_void) -> Status {
    unsafe { *out = p };
    OK
}

unsafe extern "C" fn image_unlock(_p: *mut c_void) -> Status {
    OK
}

#[repr(C)]
struct ImageEffectSuite {
    f: [usize; 13],
}

fn image_effect_suite() -> &'static ImageEffectSuite {
    static S: OnceLock<ImageEffectSuite> = OnceLock::new();
    S.get_or_init(|| ImageEffectSuite {
    f: [
        fp(fx_props as *const ()),
        fp(fx_params as *const ()),
        fp(clip_define as *const ()),
        fp(clip_handle as *const ()),
        fp(clip_props as *const ()),
        fp(clip_image as *const ()),
        fp(clip_release as *const ()),
        fp(clip_rod as *const ()),
        fp(fx_abort as *const ()),
        fp(image_alloc as *const ()),
        fp(image_free as *const ()),
        fp(image_lock as *const ()),
        fp(image_unlock as *const ()),
    ],
    })
}

const HEADER: usize = 16;

unsafe fn mem_alloc(n: usize) -> *mut c_void {
    let Ok(layout) = std::alloc::Layout::from_size_align(n + HEADER, 16) else { return std::ptr::null_mut() };
    let p = unsafe { std::alloc::alloc_zeroed(layout) };
    if p.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { (p as *mut usize).write(n) };
    unsafe { p.add(HEADER) as *mut c_void }
}

unsafe fn mem_free(p: *mut c_void) {
    if p.is_null() {
        return;
    }
    let base = unsafe { (p as *mut u8).sub(HEADER) };
    let n = unsafe { (base as *mut usize).read() };
    if let Ok(layout) = std::alloc::Layout::from_size_align(n + HEADER, 16) {
        unsafe { std::alloc::dealloc(base, layout) };
    }
}

unsafe extern "C" fn memory_alloc(_h: *mut c_void, n: usize, out: *mut *mut c_void) -> Status {
    let p = unsafe { mem_alloc(n) };
    if p.is_null() {
        return ERR_MEMORY;
    }
    unsafe { *out = p };
    OK
}

unsafe extern "C" fn memory_free(p: *mut c_void) -> Status {
    unsafe { mem_free(p) };
    OK
}

#[repr(C)]
struct MemorySuite {
    f: [usize; 2],
}

fn memory_suite() -> &'static MemorySuite {
    static S: OnceLock<MemorySuite> = OnceLock::new();
    S.get_or_init(|| MemorySuite { f: [fp(memory_alloc as *const ()), fp(memory_free as *const ())] })
}

thread_local! {
    static THREAD: Cell<Option<(u32, bool)>> = const { Cell::new(None) };
}

type ThreadFn = unsafe extern "C" fn(c_uint, c_uint, *mut c_void);

unsafe extern "C" fn mt_run(f: Option<ThreadFn>, n: c_uint, arg: *mut c_void) -> Status {
    let Some(f) = f else { return FAILED };
    let n = if n == 0 { cpus() } else { n };
    let arg = arg as usize;
    std::thread::scope(|s| {
        for i in 1..n {
            s.spawn(move || {
                THREAD.with(|t| t.set(Some((i, true))));
                unsafe { f(i, n, arg as *mut c_void) };
            });
        }
        let prev = THREAD.with(|t| t.replace(Some((0, false))));
        unsafe { f(0, n, arg as *mut c_void) };
        THREAD.with(|t| t.set(prev));
    });
    OK
}

fn cpus() -> c_uint {
    std::thread::available_parallelism().map(|n| n.get() as c_uint).unwrap_or(1)
}

unsafe extern "C" fn mt_cpus(out: *mut c_uint) -> Status {
    unsafe { *out = cpus() };
    OK
}

unsafe extern "C" fn mt_index(out: *mut c_uint) -> Status {
    unsafe { *out = THREAD.with(|t| t.get()).map(|x| x.0).unwrap_or(0) };
    OK
}

unsafe extern "C" fn mt_spawned() -> c_int {
    THREAD.with(|t| t.get()).is_some_and(|x| x.1) as c_int
}

struct RMutex {
    state: Mutex<(Option<std::thread::ThreadId>, u32)>,
    cv: Condvar,
}

unsafe extern "C" fn mx_create(out: *mut *mut RMutex, count: c_int) -> Status {
    let m = Box::new(RMutex { state: Mutex::new((None, 0)), cv: Condvar::new() });
    if count > 0 {
        *m.state.lock() = (Some(std::thread::current().id()), count as u32);
    }
    unsafe { *out = Box::into_raw(m) };
    OK
}

unsafe extern "C" fn mx_destroy(m: *mut RMutex) -> Status {
    if m.is_null() {
        return ERR_BAD_HANDLE;
    }
    drop(unsafe { Box::from_raw(m) });
    OK
}

unsafe extern "C" fn mx_lock(m: *mut RMutex) -> Status {
    let m = guard!(unsafe { m.as_ref() });
    let me = std::thread::current().id();
    let mut s = m.state.lock();
    while s.0.is_some_and(|o| o != me) {
        m.cv.wait(&mut s);
    }
    s.0 = Some(me);
    s.1 += 1;
    OK
}

unsafe extern "C" fn mx_unlock(m: *mut RMutex) -> Status {
    let m = guard!(unsafe { m.as_ref() });
    let mut s = m.state.lock();
    if s.0 != Some(std::thread::current().id()) || s.1 == 0 {
        return ERR_BAD_HANDLE;
    }
    s.1 -= 1;
    if s.1 == 0 {
        s.0 = None;
        m.cv.notify_one();
    }
    OK
}

unsafe extern "C" fn mx_try(m: *mut RMutex) -> Status {
    let m = guard!(unsafe { m.as_ref() });
    let me = std::thread::current().id();
    let mut s = m.state.lock();
    if s.0.is_some_and(|o| o != me) {
        return FAILED;
    }
    s.0 = Some(me);
    s.1 += 1;
    OK
}

#[repr(C)]
struct MultiThreadSuite {
    f: [usize; 9],
}

fn multi_thread_suite() -> &'static MultiThreadSuite {
    static S: OnceLock<MultiThreadSuite> = OnceLock::new();
    S.get_or_init(|| MultiThreadSuite {
        f: [mt_run as *const (), mt_cpus as *const (), mt_index as *const (), mt_spawned as *const (), mx_create as *const (), mx_destroy as *const (), mx_lock as *const (), mx_unlock as *const (), mx_try as *const ()].map(fp),
    })
}

unsafe extern "C" fn message(_h: *mut c_void, kind: *const c_char, _id: *const c_char, fmt: *const c_char) -> Status {
    let k = unsafe { key(kind) }.unwrap_or("OfxMessageMessage");
    let text = unsafe { key(fmt) }.unwrap_or("");
    if k == "OfxMessageQuestion" {
        return REPLY_DEFAULT;
    }
    let line = format!("[OpenFX {}] {text}", k.trim_start_matches("OfxMessage"));
    eprintln!("{line}");
    let mut log = messages().lock();
    if log.len() > 200 {
        log.remove(0);
    }
    log.push(line);
    OK
}

unsafe extern "C" fn persistent(h: *mut c_void, kind: *const c_char, id: *const c_char, fmt: *const c_char) -> Status {
    unsafe { message(h, kind, id, fmt) }
}

unsafe extern "C" fn clear_persistent(_h: *mut c_void) -> Status {
    OK
}

pub fn messages() -> &'static Mutex<Vec<String>> {
    static M: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    M.get_or_init(Default::default)
}

#[repr(C)]
struct MessageSuite {
    f: [usize; 3],
}

fn message_suite() -> &'static MessageSuite {
    static S: OnceLock<MessageSuite> = OnceLock::new();
    S.get_or_init(|| MessageSuite { f: [message as *const (), persistent as *const (), clear_persistent as *const ()].map(fp) })
}

unsafe fn pset<'a>(h: *mut ParamSet) -> Option<&'a ParamSet> {
    unsafe { h.as_ref() }
}

fn dims(kind: &str) -> usize {
    match kind {
        "OfxParamTypeRGBA" => 4,
        "OfxParamTypeRGB" | "OfxParamTypeDouble3D" | "OfxParamTypeInteger3D" => 3,
        "OfxParamTypeDouble2D" | "OfxParamTypeInteger2D" => 2,
        "OfxParamTypeGroup" | "OfxParamTypePage" | "OfxParamTypePushButton" | "OfxParamTypeString" | "OfxParamTypeCustom" | "OfxParamTypeBytes" => 0,
        _ => 1,
    }
}

fn is_int(kind: &str) -> bool {
    matches!(kind, "OfxParamTypeInteger" | "OfxParamTypeBoolean" | "OfxParamTypeChoice" | "OfxParamTypeInteger2D" | "OfxParamTypeInteger3D")
}

fn new_param(kind: &str, name: &str) -> Box<Param> {
    let props = Props::default();
    props.s("OfxPropType", "OfxTypeParameter");
    props.s("OfxParamPropType", kind);
    props.s("OfxPropName", name);
    props.s("OfxPropLabel", name);
    props.s("OfxPropShortLabel", name);
    props.s("OfxPropLongLabel", name);
    props.i("OfxParamPropSecret", &[0]);
    props.i("OfxParamPropEnabled", &[1]);
    props.i("OfxParamPropAnimates", &[1]);
    props.i("OfxParamPropIsAnimating", &[0]);
    props.i("OfxParamPropIsAutoKeying", &[0]);
    props.i("OfxParamPropPersistant", &[1]);
    props.i("OfxParamPropEvaluateOnChange", &[1]);
    props.i("OfxParamPropPluginMayWrite", &[0]);
    props.i("OfxParamPropCanUndo", &[1]);
    props.s("OfxParamPropCacheInvalidation", "OfxParamInvalidateValueChange");
    props.s("OfxParamPropHint", "");
    props.s("OfxParamPropScriptName", name);
    props.s("OfxParamPropParent", "");
    props.p("OfxParamPropDataPtr", 0);
    let n = dims(kind);
    if n > 0 {
        if is_int(kind) {
            props.i("OfxParamPropDefault", &vec![0; n]);
            props.i("OfxParamPropMin", &vec![c_int::MIN; n]);
            props.i("OfxParamPropMax", &vec![c_int::MAX; n]);
            props.i("OfxParamPropDisplayMin", &vec![c_int::MIN; n]);
            props.i("OfxParamPropDisplayMax", &vec![c_int::MAX; n]);
        } else {
            let d = if kind == "OfxParamTypeRGBA" || kind == "OfxParamTypeRGB" { 1.0 } else { 0.0 };
            props.d("OfxParamPropDefault", &vec![d; n]);
            props.d("OfxParamPropMin", &vec![-f64::MAX; n]);
            props.d("OfxParamPropMax", &vec![f64::MAX; n]);
            props.d("OfxParamPropDisplayMin", &vec![-f64::MAX; n]);
            props.d("OfxParamPropDisplayMax", &vec![f64::MAX; n]);
            props.d("OfxParamPropIncrement", &[1.0]);
            props.i("OfxParamPropDigits", &[2]);
            props.s("OfxParamPropDoubleType", "OfxParamDoubleTypePlain");
            props.s("OfxParamPropDefaultCoordinateSystem", "OfxParamCoordinatesCanonical");
        }
    }
    if kind == "OfxParamTypeString" {
        props.s("OfxParamPropDefault", "");
        props.s("OfxParamPropStringMode", "OfxParamStringIsSingleLine");
        props.i("OfxParamPropStringFilePathExists", &[1]);
    }
    if kind == "OfxParamTypeChoice" {
        props.set("OfxParamPropChoiceOption", Pv::Str(vec![]));
    }
    if kind == "OfxParamTypeGroup" {
        props.i("OfxParamPropGroupOpen", &[1]);
    }
    Box::new(Param { name: name.to_string(), kind: kind.to_string(), props, val: Mutex::new(vec![0.0; n.max(1)]), text: Mutex::new(CString::default()) })
}

unsafe extern "C" fn param_define(ps: *mut ParamSet, kind: *const c_char, name: *const c_char, out: *mut *mut Props) -> Status {
    let ps = guard!(unsafe { pset(ps) });
    let kind = guard!(unsafe { key(kind) });
    let name = guard!(unsafe { key(name) });
    let mut v = ps.params.lock();
    if v.iter().any(|p| p.name == name) {
        return 6;
    }
    v.push(new_param(kind, name));
    if !out.is_null() {
        unsafe { *out = v.last().map(|p| &p.props as *const Props as *mut Props).unwrap_or(std::ptr::null_mut()) };
    }
    OK
}

unsafe extern "C" fn param_handle(ps: *mut ParamSet, name: *const c_char, h: *mut *mut Param, out: *mut *mut Props) -> Status {
    let ps = guard!(unsafe { pset(ps) });
    let name = guard!(unsafe { key(name) });
    let v = ps.params.lock();
    let Some(p) = v.iter().find(|p| p.name == name) else { return ERR_UNKNOWN };
    if !h.is_null() {
        unsafe { *h = &**p as *const Param as *mut Param };
    }
    if !out.is_null() {
        unsafe { *out = &p.props as *const Props as *mut Props };
    }
    OK
}

unsafe extern "C" fn paramset_props(ps: *mut ParamSet, out: *mut *mut Props) -> Status {
    let ps = guard!(unsafe { pset(ps) });
    unsafe { *out = &ps.props as *const Props as *mut Props };
    OK
}

unsafe extern "C" fn param_props(p: *mut Param, out: *mut *mut Props) -> Status {
    let p = guard!(unsafe { p.as_ref() });
    unsafe { *out = &p.props as *const Props as *mut Props };
    OK
}

unsafe fn write_out(p: &Param, vals: &[f64], ptrs: [usize; 4]) -> Status {
    if p.kind == "OfxParamTypeString" || p.kind == "OfxParamTypeCustom" {
        let Some(&out) = ptrs.first() else { return ERR_VALUE };
        if out == 0 {
            return ERR_VALUE;
        }
        let t = p.text.lock();
        unsafe { *(out as *mut *const c_char) = t.as_ptr() };
        return OK;
    }
    let n = dims(&p.kind);
    if n == 0 {
        return ERR_UNSUPPORTED;
    }
    for i in 0..n {
        let out = ptrs[i];
        if out == 0 {
            return ERR_VALUE;
        }
        let v = vals.get(i).copied().unwrap_or(0.0);
        if is_int(&p.kind) {
            unsafe { *(out as *mut c_int) = v.round() as c_int };
        } else {
            unsafe { *(out as *mut f64) = v };
        }
    }
    OK
}

fn read_in(p: &Param, ints: [usize; 4], dbls: [f64; 4]) -> Status {
    let n = dims(&p.kind);
    if n == 0 {
        if p.kind == "OfxParamTypeString" && ints[0] != 0 {
            *p.text.lock() = unsafe { CStr::from_ptr(ints[0] as *const c_char) }.to_owned();
            return OK;
        }
        return ERR_UNSUPPORTED;
    }
    let mut v = p.val.lock();
    for i in 0..n {
        v[i] = if is_int(&p.kind) { ints[i] as u32 as c_int as f64 } else { dbls[i] };
    }
    OK
}

fn at_time(p: &Param, frame: f64) -> Vec<f64> {
    let cur = p.val.lock().clone();
    let ctx = RENDER.with(|r| r.get());
    let Some((ctx, id)) = ctx.and_then(|c| unsafe { (c as *const Live).as_ref() }).and_then(|l| l.ids.get(&p.name).map(|id| (l, *id))) else { return cur };
    let c = unsafe { &*ctx.fx };
    let dt = frame / c.fps.max(1e-6) - c.t;
    if dt.abs() < 1e-9 {
        return cur;
    }
    match c.host.and_then(|h| h.param_at(id, dt)) {
        Some(v) => to_ofx(ctx.map_of(&p.name), &v, ctx),
        None => cur,
    }
}

#[cfg(not(all(target_arch = "aarch64", target_vendor = "apple")))]
mod va {
    use super::*;

    #[cfg(not(windows))]
    fn split(a: [usize; 4], d: [f64; 4]) -> ([usize; 4], [f64; 4]) {
        (a, d)
    }

    #[cfg(windows)]
    fn split(a: [usize; 4], _d: [f64; 4]) -> ([usize; 4], [f64; 4]) {
        (a, a.map(|x| f64::from_bits(x as u64)))
    }

    pub unsafe extern "C" fn get_value(p: *mut Param, a1: usize, a2: usize, a3: usize, a4: usize) -> Status {
        let p = guard!(unsafe { p.as_ref() });
        let v = p.val.lock().clone();
        unsafe { write_out(p, &v, [a1, a2, a3, a4]) }
    }

    pub unsafe extern "C" fn get_value_at(p: *mut Param, t: f64, a1: usize, a2: usize, a3: usize, a4: usize) -> Status {
        let p = guard!(unsafe { p.as_ref() });
        let v = at_time(p, t);
        unsafe { write_out(p, &v, [a1, a2, a3, a4]) }
    }

    pub unsafe extern "C" fn get_derivative(p: *mut Param, _t: f64, a1: usize, a2: usize, a3: usize, a4: usize) -> Status {
        let p = guard!(unsafe { p.as_ref() });
        unsafe { write_out(p, &[0.0; 4], [a1, a2, a3, a4]) }
    }

    pub unsafe extern "C" fn get_integral(p: *mut Param, t1: f64, t2: f64, a1: usize, a2: usize, a3: usize, a4: usize) -> Status {
        let p = guard!(unsafe { p.as_ref() });
        let v: Vec<f64> = p.val.lock().iter().map(|x| x * (t2 - t1)).collect();
        unsafe { write_out(p, &v, [a1, a2, a3, a4]) }
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe extern "C" fn set_value(p: *mut Param, a1: usize, a2: usize, a3: usize, a4: usize, d0: f64, d1: f64, d2: f64, d3: f64) -> Status {
        let p = guard!(unsafe { p.as_ref() });
        let (a, d) = split([a1, a2, a3, a4], [d0, d1, d2, d3]);
        read_in(p, a, d)
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe extern "C" fn set_value_at(p: *mut Param, _t: f64, a1: usize, a2: usize, a3: usize, a4: usize, d1: f64, d2: f64, d3: f64) -> Status {
        let p = guard!(unsafe { p.as_ref() });
        #[cfg(windows)]
        let (a, d) = split([a1, a2, a3, a4], [0.0; 4]);
        #[cfg(not(windows))]
        let (a, d) = ([a1, a2, a3, a4], [d1, d2, d3, 0.0]);
        read_in(p, a, d)
    }
}

#[cfg(all(target_arch = "aarch64", target_vendor = "apple"))]
mod va {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    pub unsafe extern "C" fn get_value(p: *mut Param, _x1: usize, _x2: usize, _x3: usize, _x4: usize, _x5: usize, _x6: usize, _x7: usize, s0: usize, s1: usize, s2: usize, s3: usize) -> Status {
        let p = guard!(unsafe { p.as_ref() });
        let v = p.val.lock().clone();
        unsafe { write_out(p, &v, [s0, s1, s2, s3]) }
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe extern "C" fn get_value_at(p: *mut Param, t: f64, _x1: usize, _x2: usize, _x3: usize, _x4: usize, _x5: usize, _x6: usize, _x7: usize, s0: usize, s1: usize, s2: usize, s3: usize) -> Status {
        let p = guard!(unsafe { p.as_ref() });
        let v = at_time(p, t);
        unsafe { write_out(p, &v, [s0, s1, s2, s3]) }
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe extern "C" fn get_derivative(p: *mut Param, _t: f64, _x1: usize, _x2: usize, _x3: usize, _x4: usize, _x5: usize, _x6: usize, _x7: usize, s0: usize, s1: usize, s2: usize, s3: usize) -> Status {
        let p = guard!(unsafe { p.as_ref() });
        unsafe { write_out(p, &[0.0; 4], [s0, s1, s2, s3]) }
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe extern "C" fn get_integral(p: *mut Param, t1: f64, t2: f64, _x1: usize, _x2: usize, _x3: usize, _x4: usize, _x5: usize, _x6: usize, _x7: usize, s0: usize, s1: usize, s2: usize, s3: usize) -> Status {
        let p = guard!(unsafe { p.as_ref() });
        let v: Vec<f64> = p.val.lock().iter().map(|x| x * (t2 - t1)).collect();
        unsafe { write_out(p, &v, [s0, s1, s2, s3]) }
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe extern "C" fn set_value(p: *mut Param, _x1: usize, _x2: usize, _x3: usize, _x4: usize, _x5: usize, _x6: usize, _x7: usize, s0: usize, s1: usize, s2: usize, s3: usize) -> Status {
        let p = guard!(unsafe { p.as_ref() });
        let a = [s0, s1, s2, s3];
        read_in(p, a, a.map(|x| f64::from_bits(x as u64)))
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe extern "C" fn set_value_at(p: *mut Param, _t: f64, _x1: usize, _x2: usize, _x3: usize, _x4: usize, _x5: usize, _x6: usize, _x7: usize, s0: usize, s1: usize, s2: usize, s3: usize) -> Status {
        let p = guard!(unsafe { p.as_ref() });
        let a = [s0, s1, s2, s3];
        read_in(p, a, a.map(|x| f64::from_bits(x as u64)))
    }
}

unsafe extern "C" fn num_keys(_p: *mut Param, out: *mut c_uint) -> Status {
    unsafe { *out = 0 };
    OK
}

unsafe extern "C" fn key_time(_p: *mut Param, _n: c_uint, _out: *mut f64) -> Status {
    ERR_BAD_INDEX
}

unsafe extern "C" fn key_index(_p: *mut Param, _t: f64, _d: c_int, out: *mut c_int) -> Status {
    unsafe { *out = -1 };
    FAILED
}

unsafe extern "C" fn delete_key(_p: *mut Param, _t: f64) -> Status {
    OK
}

unsafe extern "C" fn delete_all_keys(_p: *mut Param) -> Status {
    OK
}

unsafe extern "C" fn param_copy(to: *mut Param, from: *mut Param, _off: f64, _range: *const c_void) -> Status {
    let (Some(to), Some(from)) = (unsafe { to.as_ref() }, unsafe { from.as_ref() }) else { return ERR_BAD_HANDLE };
    let v = from.val.lock().clone();
    *to.val.lock() = v;
    OK
}

unsafe extern "C" fn edit_begin(_ps: *mut ParamSet, _name: *const c_char) -> Status {
    OK
}

unsafe extern "C" fn edit_end(_ps: *mut ParamSet) -> Status {
    OK
}

#[repr(C)]
struct ParameterSuite {
    f: [usize; 18],
}

fn parameter_suite() -> &'static ParameterSuite {
    static S: OnceLock<ParameterSuite> = OnceLock::new();
    S.get_or_init(|| ParameterSuite {
    f: [
        fp(param_define as *const ()),
        fp(param_handle as *const ()),
        fp(paramset_props as *const ()),
        fp(param_props as *const ()),
        fp(va::get_value as *const ()),
        fp(va::get_value_at as *const ()),
        fp(va::get_derivative as *const ()),
        fp(va::get_integral as *const ()),
        fp(va::set_value as *const ()),
        fp(va::set_value_at as *const ()),
        fp(num_keys as *const ()),
        fp(key_time as *const ()),
        fp(key_index as *const ()),
        fp(delete_key as *const ()),
        fp(delete_all_keys as *const ()),
        fp(param_copy as *const ()),
        fp(edit_begin as *const ()),
        fp(edit_end as *const ()),
    ],
    })
}

fn host_props() -> *mut Props {
    static H: OnceLock<usize> = OnceLock::new();
    *H.get_or_init(|| {
        let p = Props::default();
        p.s("OfxPropType", "OfxTypeImageEffectHost");
        p.s("OfxPropName", "ai.nodemode.formvfx");
        p.s("OfxPropLabel", "formvfx");
        let version = env!("CARGO_PKG_VERSION");
        let v: Vec<c_int> = version.split('.').filter_map(|x| x.parse().ok()).collect();
        p.i("OfxPropVersion", &v);
        p.s("OfxPropVersionLabel", version);
        p.i("OfxPropAPIVersion", &[1, 4]);
        p.i("OfxImageEffectHostPropIsBackground", &[0]);
        p.i("OfxImageEffectPropSupportsOverlays", &[0]);
        p.i("OfxImageEffectPropSupportsMultiResolution", &[1]);
        p.i("OfxImageEffectPropSupportsTiles", &[0]);
        p.i("OfxImageEffectPropTemporalClipAccess", &[1]);
        p.ss("OfxImageEffectPropSupportedComponents", &["OfxImageComponentRGBA"]);
        p.ss("OfxImageEffectPropSupportedContexts", &["OfxImageEffectContextFilter", "OfxImageEffectContextGeneral", "OfxImageEffectContextGenerator"]);
        p.ss("OfxImageEffectPropSupportedPixelDepths", &["OfxBitDepthFloat", "OfxBitDepthShort", "OfxBitDepthByte"]);
        p.i("OfxImageEffectPropMultipleClipDepths", &[0]);
        p.i("OfxImageEffectPropSupportsMultipleClipPARs", &[0]);
        p.i("OfxImageEffectPropSetableFrameRate", &[0]);
        p.i("OfxImageEffectPropSetableFielding", &[0]);
        p.i("OfxImageEffectInstancePropSequentialRender", &[0]);
        p.s("OfxImageEffectHostPropNativeOrigin", "kOfxImageEffectHostPropNativeOriginTopLeft");
        p.i("OfxParamHostPropSupportsCustomInteract", &[0]);
        p.i("OfxParamHostPropSupportsStringAnimation", &[0]);
        p.i("OfxParamHostPropSupportsChoiceAnimation", &[0]);
        p.i("OfxParamHostPropSupportsBooleanAnimation", &[0]);
        p.i("OfxParamHostPropSupportsCustomAnimation", &[0]);
        p.i("OfxParamHostPropSupportsStrChoice", &[0]);
        p.i("OfxParamHostPropSupportsStrChoiceAnimation", &[0]);
        p.i("OfxParamHostPropSupportsParametricAnimation", &[0]);
        p.i("OfxParamHostPropMaxParameters", &[-1]);
        p.i("OfxParamHostPropMaxPages", &[0]);
        p.i("OfxParamHostPropPageRowColumnCount", &[0, 0]);
        p.s("OfxImageEffectPropOpenGLRenderSupported", "false");
        p.s("OfxImageEffectPropCudaRenderSupported", "false");
        p.s("OfxImageEffectPropMetalRenderSupported", "false");
        p.s("OfxImageEffectPropOpenCLRenderSupported", "false");
        p.i("OfxImageEffectPropRenderQualityDraft", &[0]);
        p.p("OfxPropHostOSHandle", 0);
        Box::into_raw(Box::new(p)) as usize
    }) as *mut Props
}

unsafe extern "C" fn fetch_suite(_host: *mut Props, name: *const c_char, version: c_int) -> *const c_void {
    let Some(name) = (unsafe { key(name) }) else { return std::ptr::null() };
    match (name, version) {
        ("OfxPropertySuite", 1) => property_suite() as *const PropertySuite as *const c_void,
        ("OfxImageEffectSuite", 1) => image_effect_suite() as *const ImageEffectSuite as *const c_void,
        ("OfxParameterSuite", 1) => parameter_suite() as *const ParameterSuite as *const c_void,
        ("OfxMemorySuite", 1) => memory_suite() as *const MemorySuite as *const c_void,
        ("OfxMultiThreadSuite", 1) => multi_thread_suite() as *const MultiThreadSuite as *const c_void,
        ("OfxMessageSuite", 1) | ("OfxMessageSuite", 2) => message_suite() as *const MessageSuite as *const c_void,
        _ => std::ptr::null(),
    }
}

fn host() -> *mut RawHost {
    static H: OnceLock<usize> = OnceLock::new();
    *H.get_or_init(|| Box::into_raw(Box::new(RawHost { host: host_props(), fetch: fetch_suite })) as usize) as *mut RawHost
}

pub fn arch_dir() -> &'static str {
    if cfg!(target_os = "macos") {
        "MacOS"
    } else if cfg!(all(windows, target_arch = "x86_64")) {
        "Win64"
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        "Win-arm64"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "Linux-arm64"
    } else {
        "Linux-x86-64"
    }
}

pub fn binary_of(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let name = path.file_name()?.to_string_lossy().to_string();
    let stem = name.strip_suffix(".bundle").unwrap_or(&name);
    let bin = path.join("Contents").join(arch_dir()).join(stem);
    bin.is_file().then_some(bin)
}

pub fn system_folders() -> Vec<PathBuf> {
    let mut out = vec![];
    if let Ok(v) = std::env::var("OFX_PLUGIN_PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        out.extend(v.split(sep).filter(|s| !s.is_empty()).map(PathBuf::from));
    }
    if cfg!(target_os = "macos") {
        out.push(PathBuf::from("/Library/OFX/Plugins"));
    } else if cfg!(windows) {
        let base = std::env::var("CommonProgramFiles").unwrap_or_else(|_| r"C:\Program Files\Common Files".into());
        out.push(PathBuf::from(base).join("OFX").join("Plugins"));
    } else {
        out.push(PathBuf::from("/usr/OFX/Plugins"));
    }
    out
}

fn marker() -> PathBuf {
    super::plugin::folder().join(".ofx-loading")
}

fn crashed() -> Vec<String> {
    std::fs::read_to_string(super::plugin::folder().join(".ofx-crashed")).map(|s| s.lines().map(str::to_string).collect()).unwrap_or_default()
}

pub fn recover() {
    for m in [marker(), render_marker()] {
        if let Ok(p) = std::fs::read_to_string(&m) {
            let mut list = crashed();
            if !p.trim().is_empty() && !list.iter().any(|x| x == p.trim()) {
                list.push(p.trim().to_string());
            }
            let _ = std::fs::write(super::plugin::folder().join(".ofx-crashed"), list.join("\n"));
            let _ = std::fs::remove_file(&m);
        }
    }
}

pub fn forgive(path: &str) {
    let list: Vec<String> = crashed().into_iter().filter(|x| x != path).collect();
    let _ = std::fs::write(super::plugin::folder().join(".ofx-crashed"), list.join("\n"));
}

fn bundles(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut v: Vec<PathBuf> = rd.filter_map(|x| x.ok().map(|x| x.path())).collect();
    v.sort();
    for p in v {
        let name = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        if name.ends_with(".ofx.bundle") && p.is_dir() {
            out.push(p);
        } else if p.is_dir() && depth < 2 && !name.starts_with('.') {
            bundles(&p, out, depth + 1);
        }
    }
}

pub fn load_folder(dir: &Path) -> (usize, Vec<String>) {
    let mut found = vec![];
    bundles(dir, &mut found, 0);
    let skip = crashed();
    let (mut n, mut errs) = (0, vec![]);
    for b in found {
        let path = b.to_string_lossy().to_string();
        if skip.contains(&path) {
            let msg = format!("{path}: skipped, it stopped formvfx while loading or on its first render last time (Load Plug-in loads it again on purpose)");
            failures().write().retain(|x| x.0 != path);
            failures().write().push((path, msg.clone()));
            errs.push(msg);
            continue;
        }
        match load(&path) {
            Ok(v) => n += v.len(),
            Err(e) => errs.push(e),
        }
    }
    (n, errs)
}

pub fn load(path: &str) -> Result<Vec<Arc<Plugin>>, String> {
    let _ = std::fs::create_dir_all(super::plugin::folder());
    let _ = std::fs::write(marker(), path);
    let r = unsafe { load_inner(path) };
    let _ = std::fs::remove_file(marker());
    let mut f = failures().write();
    f.retain(|x| x.0 != path);
    if let Err(e) = &r {
        f.push((path.to_string(), e.clone()));
    }
    r
}

fn call(main: Entry, action: &str, handle: *const c_void, inargs: Option<&Props>, outargs: Option<&Props>) -> Status {
    let a = cs(action);
    let ip = inargs.map(|p| p as *const Props as *mut Props).unwrap_or(std::ptr::null_mut());
    let op = outargs.map(|p| p as *const Props as *mut Props).unwrap_or(std::ptr::null_mut());
    unsafe { main(a.as_ptr(), handle, ip, op) }
}

fn ok(s: Status) -> bool {
    s == OK || s == REPLY_DEFAULT
}

fn new_effect(kind: &str) -> Box<Effect> {
    let props = Props::default();
    props.s("OfxPropType", kind);
    props.s("OfxPropLabel", "");
    props.s("OfxImageEffectPluginPropGrouping", "");
    props.ss("OfxImageEffectPropSupportedContexts", &[]);
    props.ss("OfxImageEffectPropSupportedPixelDepths", &[]);
    props.i("OfxImageEffectPluginPropSingleInstance", &[0]);
    props.s("OfxImageEffectPluginRenderThreadSafety", "OfxImageEffectRenderInstanceSafe");
    props.i("OfxImageEffectPluginPropHostFrameThreading", &[0]);
    props.i("OfxImageEffectPropSupportsMultiResolution", &[1]);
    props.i("OfxImageEffectPropSupportsTiles", &[1]);
    props.i("OfxImageEffectPropTemporalClipAccess", &[0]);
    props.i("OfxImageEffectPluginPropFieldRenderTwiceAlways", &[0]);
    props.i("OfxImageEffectPropMultipleClipDepths", &[0]);
    props.i("OfxImageEffectPropSupportsMultipleClipPARs", &[0]);
    props.p("OfxPropInstanceData", 0);
    let params = ParamSet { props: Props::default(), params: Mutex::new(vec![]) };
    params.props.set("OfxPluginPropParamPageOrder", Pv::Str(vec![]));
    Box::new(Effect { props, params, clips: Mutex::new(vec![]) })
}

fn sanitize(name: &str, used: &[&str]) -> &'static str {
    let mut s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if s.is_empty() || s.chars().next().is_some_and(|c| c.is_ascii_digit()) || s == "plugin" {
        s = format!("p_{s}");
    }
    let base = s.clone();
    let mut i = 2;
    while used.contains(&s.as_str()) {
        s = format!("{base}_{i}");
        i += 1;
    }
    leak(&s)
}

fn finite(v: f64, d: f64) -> f64 {
    if v.is_finite() && v.abs() < 1e9 { v } else { d }
}

fn spec_of(p: &Param, id: &'static str) -> Option<(ParamSpec, bool, bool)> {
    let pr = &p.props;
    if pr.get_i("OfxParamPropSecret").first().copied().unwrap_or(0) != 0 {
        return None;
    }
    let label = pr.get_s("OfxPropLabel").into_iter().next().filter(|s| !s.is_empty()).unwrap_or_else(|| p.name.clone());
    let name = leak(&label);
    let dtype = pr.get_s("OfxParamPropDoubleType").into_iter().next().unwrap_or_default();
    let normsys = pr.get_s("OfxParamPropDefaultCoordinateSystem").first().is_some_and(|s| s == "OfxParamCoordinatesNormalised");
    let def = pr.get_d("OfxParamPropDefault");
    let d = |i: usize, x: f64| def.get(i).copied().unwrap_or(x);
    let range = |k: &str, i: usize, x: f64| finite(pr.get_d(k).get(i).copied().unwrap_or(x), x);
    let spec = |v: Value, ui: Ui| ParamSpec { id, name, init: Init::V(v), ui };
    Some(match p.kind.as_str() {
        "OfxParamTypeDouble" | "OfxParamTypeInteger" => {
            if dtype == "OfxParamDoubleTypeAngle" {
                return Some((spec(Value::Num(d(0, 0.0)), Ui::Angle), false, false));
            }
            let lo = range("OfxParamPropMin", 0, -1e6);
            let hi = range("OfxParamPropMax", 0, 1e6);
            let slo = range("OfxParamPropDisplayMin", 0, lo.max(-1000.0));
            let shi = range("OfxParamPropDisplayMax", 0, hi.min(1000.0));
            let digits = if p.kind == "OfxParamTypeInteger" { 0 } else { pr.get_i("OfxParamPropDigits").first().copied().unwrap_or(2).clamp(0, 6) as u8 };
            (spec(Value::Num(d(0, 0.0)), Ui::slider(lo, hi, slo, shi, digits)), false, false)
        }
        "OfxParamTypeBoolean" => (spec(Value::Bool(d(0, 0.0) != 0.0), Ui::Checkbox), false, false),
        "OfxParamTypeChoice" => {
            let opts = pr.get_s("OfxParamPropChoiceOption");
            if opts.is_empty() {
                return None;
            }
            (spec(Value::Enum(d(0, 0.0).max(0.0) as u32), Ui::Popup { options: opts }), false, false)
        }
        "OfxParamTypeRGB" => (spec(Value::Color([d(0, 1.0), d(1, 1.0), d(2, 1.0), 1.0]), Ui::Color), false, false),
        "OfxParamTypeRGBA" => (spec(Value::Color([d(0, 1.0), d(1, 1.0), d(2, 1.0), d(3, 1.0)]), Ui::Color), false, false),
        "OfxParamTypeDouble2D" if matches!(dtype.as_str(), "OfxParamDoubleTypeXY" | "OfxParamDoubleTypeXYAbsolute") => {
            let norm = dtype == "OfxParamDoubleTypeXY";
            if norm || normsys {
                (ParamSpec { id, name, init: Init::RelPoint(d(0, 0.5), 1.0 - d(1, 0.5)), ui: Ui::Point }, norm, true)
            } else {
                (spec(Value::Vec2([d(0, 0.0), d(1, 0.0)]), Ui::Point), false, true)
            }
        }
        "OfxParamTypeDouble2D" | "OfxParamTypeInteger2D" => (spec(Value::Vec2([d(0, 0.0), d(1, 0.0)]), Ui::Plain), false, false),
        "OfxParamTypeDouble3D" | "OfxParamTypeInteger3D" => (spec(Value::Vec3([d(0, 0.0), d(1, 0.0), d(2, 0.0)]), Ui::Plain), false, false),
        "OfxParamTypeString" => {
            let mode = pr.get_s("OfxParamPropStringMode").into_iter().next().unwrap_or_default();
            if mode == "OfxParamStringIsLabel" {
                return None;
            }
            let t = pr.get_s("OfxParamPropDefault").into_iter().next().unwrap_or_default();
            (spec(Value::Str(t), Ui::Text), false, false)
        }
        _ => return None,
    })
}

unsafe fn load_inner(path: &str) -> Result<Vec<Arc<Plugin>>, String> {
    let bin = binary_of(Path::new(path)).ok_or_else(|| format!("{path}: no {} binary inside the bundle (Contents/{}/)", arch_dir(), arch_dir()))?;
    let lib = unsafe { libloading::Library::new(&bin) }.map_err(|e| format!("{path}: {e}"))?;
    let lib: &'static libloading::Library = Box::leak(Box::new(lib));
    if let Ok(f) = unsafe { lib.get::<unsafe extern "C" fn(*mut RawHost) -> Status>(b"OfxSetHost\0") } {
        unsafe { f(host()) };
    }
    let count = unsafe { lib.get::<unsafe extern "C" fn() -> c_int>(b"OfxGetNumberOfPlugins\0") }.map_err(|_| format!("{path}: not an OpenFX plug-in (no OfxGetNumberOfPlugins)"))?;
    let get = unsafe { lib.get::<unsafe extern "C" fn(c_int) -> *mut RawPlugin>(b"OfxGetPlugin\0") }.map_err(|_| format!("{path}: not an OpenFX plug-in (no OfxGetPlugin)"))?;
    let n = unsafe { count() };
    let mut out = vec![];
    let mut errs = vec![];
    for i in 0..n.max(0) {
        let raw = unsafe { get(i) };
        let Some(raw) = (unsafe { raw.as_ref() }) else { continue };
        let api = unsafe { key(raw.api) }.unwrap_or("");
        let ident = unsafe { key(raw.identifier) }.unwrap_or("").to_string();
        if api != "OfxImageEffectPluginAPI" {
            errs.push(format!("{ident}: API {api} is not an image effect"));
            continue;
        }
        match unsafe { describe(path, raw, &ident) } {
            Ok(p) => out.push(p),
            Err(e) => errs.push(format!("{ident}: {e}")),
        }
    }
    if out.is_empty() {
        return Err(format!("{path}: {}", if errs.is_empty() { "no plug-ins in the bundle".to_string() } else { errs.join("; ") }));
    }
    let mut reg = registry().write();
    for p in &out {
        reg.retain(|x| x.id != p.id);
        reg.push(p.clone());
    }
    if !errs.is_empty() {
        failures().write().push((path.to_string(), errs.join("; ")));
    }
    Ok(out)
}

unsafe fn describe(path: &str, raw: &RawPlugin, ident: &str) -> Result<Arc<Plugin>, String> {
    let main = raw.main.ok_or("no main entry")?;
    if let Some(sh) = raw.set_host {
        unsafe { sh(host()) };
    }
    let s = call(main, "OfxActionLoad", std::ptr::null(), None, None);
    if !ok(s) {
        return Err(format!("load failed ({s})"));
    }
    let desc = new_effect("OfxTypeImageEffect");
    desc.props.s("OfxPluginPropFilePath", path);
    let s = call(main, "OfxActionDescribe", &*desc as *const Effect as *const c_void, None, None);
    if !ok(s) {
        return Err(format!("describe failed ({s})"));
    }
    let contexts = desc.props.get_s("OfxImageEffectPropSupportedContexts");
    let context = ["OfxImageEffectContextFilter", "OfxImageEffectContextGeneral", "OfxImageEffectContextGenerator"]
        .into_iter()
        .find(|c| contexts.iter().any(|x| x == c))
        .ok_or_else(|| format!("contexts {} are not supported (filter, general, generator)", contexts.join(", ")))?;
    let depths = desc.props.get_s("OfxImageEffectPropSupportedPixelDepths");
    let depth = DEPTHS.iter().find(|(d, _)| depths.is_empty() || depths.iter().any(|x| x == d)).map(|x| x.0).ok_or_else(|| format!("needs {} images (formvfx hands plug-ins float, 16-bit or 8-bit)", depths.join(", ")))?;
    let ctx = new_effect("OfxTypeImageEffect");
    *ctx.props.map.lock() = desc.props.map.lock().clone();
    let inargs = Props::default();
    inargs.s("OfxImageEffectPropContext", context);
    let s = call(main, "OfxImageEffectActionDescribeInContext", &*ctx as *const Effect as *const c_void, Some(&inargs), None);
    if !ok(s) {
        return Err(format!("describe in context failed ({s})"));
    }
    let label = ctx.props.get_s("OfxPropLabel").into_iter().next().filter(|s| !s.is_empty()).unwrap_or_else(|| ident.rsplit('.').next().unwrap_or(ident).to_string());
    let grouping = ctx.props.get_s("OfxImageEffectPluginPropGrouping").into_iter().next().unwrap_or_default();
    let category = if grouping.is_empty() { CAT.to_string() } else { format!("{CAT} {}", grouping.replace('/', " ")) };
    let safe: String = ident.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect();
    let id = format!("ofx.{safe}");
    let mut name = label;
    if super::all().iter().any(|e| e.name == name && e.id != id) {
        name = format!("{name} (OFX)");
    }
    let taken = |n: &str| registry().read().iter().any(|p| p.name == n && p.id != id) || super::plugin::loaded().iter().any(|p| p.name == n);
    if taken(&name) {
        name = format!("{name} ({ident})");
    }
    let mut params = vec![hidden("plugin", "Plug-in", &id)];
    let mut map = vec![];
    for p in ctx.params.params.lock().iter() {
        let used: Vec<&str> = map.iter().map(|m: &ParamMap| m.id).collect();
        let fid = sanitize(&p.name, &used);
        if let Some((spec, norm, point)) = spec_of(p, fid) {
            params.push(spec);
            map.push(ParamMap { ofx: p.name.clone(), id: fid, kind: p.kind.clone(), norm, point });
        }
    }
    let safety = ctx.props.get_s("OfxImageEffectPluginRenderThreadSafety").into_iter().next().unwrap_or_default();
    let spec: &'static EffectSpec = Box::leak(Box::new(EffectSpec { id: leak(&id), name: leak(&name), category: leak(&category), params, render, pad }));
    let single = ctx.props.get_i("OfxImageEffectPluginPropSingleInstance").first().copied().unwrap_or(0) != 0;
    Ok(Arc::new(Plugin {
        id,
        name,
        category,
        identifier: ident.to_string(),
        version: format!("{}.{}", raw.major, raw.minor),
        path: path.to_string(),
        context: context.to_string(),
        depth,
        main,
        desc: ctx,
        map,
        unsafe_render: (safety == "OfxImageEffectRenderUnsafe" || single).then(|| Mutex::new(())),
        pool: Mutex::new(vec![]),
        rendered: std::sync::atomic::AtomicBool::new(false),
        spec,
    }))
}

struct Live {
    fx: *const FxCtx<'static>,
    ids: HashMap<String, &'static str>,
    maps: *const Vec<ParamMap>,
    w: f64,
    h: f64,
    depth: &'static str,
    frame: f64,
    arena: Mutex<Vec<Bufs>>,
}

impl Live {
    fn map_of(&self, name: &str) -> &ParamMap {
        let maps = unsafe { &*self.maps };
        maps.iter().find(|m| m.ofx == name).unwrap_or(&maps[0])
    }
}

thread_local! {
    static RENDER: Cell<Option<usize>> = const { Cell::new(None) };
}

fn to_ofx(m: &ParamMap, v: &Value, live: &Live) -> Vec<f64> {
    let c = unsafe { &*live.fx };
    match v {
        Value::Num(x) => vec![*x],
        Value::Bool(b) => vec![*b as i32 as f64],
        Value::Enum(i) => vec![*i as f64],
        Value::Color(c) => c.to_vec(),
        Value::Vec2(p) if m.point => {
            let b = c.to_buf(*p);
            let r = c.r.max(1e-9);
            if m.norm { vec![b[0] / live.w, (live.h - b[1]) / live.h] } else { vec![b[0] / r, (live.h - b[1]) / r] }
        }
        Value::Vec2(p) => p.to_vec(),
        Value::Vec3(p) => p.to_vec(),
        _ => vec![v.num()],
    }
}

const DEPTHS: [(&str, usize); 3] = [("OfxBitDepthFloat", 4), ("OfxBitDepthShort", 2), ("OfxBitDepthByte", 1)];

fn instance(plug: &Plugin) -> Box<Effect> {
    let inst = new_effect("OfxTypeImageEffectInstance");
    *inst.props.map.lock() = plug.desc.props.map.lock().clone();
    let p = &inst.props;
    p.s("OfxPropType", "OfxTypeImageEffectInstance");
    p.s("OfxImageEffectPropContext", &plug.context);
    p.p("OfxPropInstanceData", 0);
    p.d("OfxImageEffectPropPixelAspectRatio", &[1.0]);
    p.d("OfxImageEffectInstancePropEffectDuration", &[1.0]);
    p.i("OfxImageEffectInstancePropSequentialRender", &[0]);
    p.i("OfxPropIsInteractive", &[0]);
    for dc in plug.desc.clips.lock().iter() {
        let cl = Box::new(Clip { name: dc.name.clone(), props: dc.props.clone(), buf: Mutex::new(None) });
        let connected = dc.name == "Output" || dc.name == "Source";
        let q = &cl.props;
        q.i("OfxImageClipPropConnected", &[connected as c_int]);
        q.s("OfxImageEffectPropPixelDepth", plug.depth);
        q.s("OfxImageEffectPropComponents", "OfxImageComponentRGBA");
        q.s("OfxImageClipPropUnmappedPixelDepth", plug.depth);
        q.s("OfxImageClipPropUnmappedComponents", "OfxImageComponentRGBA");
        q.s("OfxImageEffectPropPreMultiplication", "OfxImageAlphaPremultiplied");
        q.d("OfxImagePropPixelAspectRatio", &[1.0]);
        q.s("OfxImageClipPropFieldOrder", "OfxFieldNone");
        q.i("OfxImageClipPropContinuousSamples", &[0]);
        inst.clips.lock().push(cl);
    }
    *inst.params.props.map.lock() = plug.desc.params.props.map.lock().clone();
    for dp in plug.desc.params.params.lock().iter() {
        let np = Box::new(Param { name: dp.name.clone(), kind: dp.kind.clone(), props: dp.props.clone(), val: Mutex::new(vec![0.0; dims(&dp.kind).max(1)]), text: Mutex::new(CString::default()) });
        np.props.s("OfxPropType", "OfxTypeParameterInstance");
        let def = dp.props.get_d("OfxParamPropDefault");
        {
            let mut v = np.val.lock();
            for (i, x) in def.iter().enumerate().take(v.len()) {
                v[i] = *x;
            }
        }
        if dp.kind == "OfxParamTypeString" {
            *np.text.lock() = cs(&dp.props.get_s("OfxParamPropDefault").into_iter().next().unwrap_or_default());
        }
        inst.params.params.lock().push(np);
    }
    inst
}

pub struct Pooled {
    inst: Box<Effect>,
    last: HashMap<String, (Vec<f64>, String)>,
}

fn prepare(inst: &Effect, plug: &Plugin, c: &FxCtx, w: usize, h: usize, src: usize, dst: usize) {
    let row = w * 4 * DEPTHS.iter().find(|d| d.0 == plug.depth).map(|d| d.1).unwrap_or(4);
    let r = c.r.max(1e-9);
    let (pw, ph) = (w as f64 / r, h as f64 / r);
    let frame = c.t * c.fps;
    let p = &inst.props;
    p.d("OfxImageEffectPropProjectSize", &[pw, ph]);
    p.d("OfxImageEffectPropProjectOffset", &[0.0, 0.0]);
    p.d("OfxImageEffectPropProjectExtent", &[pw, ph]);
    p.d("OfxImageEffectPropFrameRate", &[c.fps]);
    p.d("OfxImageEffectPropRenderScale", &[r, r]);
    for cl in inst.clips.lock().iter() {
        let q = &cl.props;
        q.d("OfxImageEffectPropFrameRate", &[c.fps]);
        q.d("OfxImageEffectPropUnmappedFrameRate", &[c.fps]);
        q.d("OfxImageEffectPropFrameRange", &[frame, frame]);
        q.d("OfxImageEffectPropUnmappedFrameRange", &[frame, frame]);
        q.d("OfxImageEffectPropRenderScale", &[r, r]);
        q.d("OfxImagePropRenderScale", &[r, r]);
        *cl.buf.lock() = match cl.name.as_str() {
            "Output" => Some((dst, w, h, row)),
            "Source" => Some((src, w, h, row)),
            _ => None,
        };
    }
}

fn apply_values(pooled: &mut Pooled, plug: &Plugin, c: &FxCtx, live: &Live) -> Vec<String> {
    let mut changed = vec![];
    let ps = pooled.inst.params.params.lock();
    for m in &plug.map {
        let Some(v) = c.p.get(m.id) else { continue };
        let Some(p) = ps.iter().find(|p| p.name == m.ofx) else { continue };
        let now = if m.kind == "OfxParamTypeString" {
            let s = if let Value::Str(s) = v { s.clone() } else { String::new() };
            *p.text.lock() = cs(&s);
            (vec![], s)
        } else {
            let vals = to_ofx(m, v, live);
            let mut pv = p.val.lock();
            for (i, x) in vals.iter().enumerate().take(pv.len()) {
                pv[i] = *x;
            }
            (pv.clone(), String::new())
        };
        if pooled.last.get(&m.ofx).is_some_and(|old| *old != now) {
            changed.push(m.ofx.clone());
        }
        pooled.last.insert(m.ofx.clone(), now);
    }
    changed
}

fn handle_of(inst: &Effect) -> *const c_void {
    inst as *const Effect as *const c_void
}

fn destroy(plug: &Plugin, pooled: Pooled) {
    call(plug.main, "OfxActionDestroyInstance", handle_of(&pooled.inst), None, None);
}

fn with_instance<R>(plug: &Plugin, c: &FxCtx, w: usize, h: usize, bufs: (usize, usize), live: &Live, f: impl FnOnce(*const c_void) -> Result<R, String>) -> Result<R, String> {
    let popped = plug.pool.lock().pop();
    let fresh = popped.is_none();
    let mut pooled = popped.unwrap_or_else(|| Pooled { inst: instance(plug), last: HashMap::new() });
    prepare(&pooled.inst, plug, c, w, h, bufs.0, bufs.1);
    let changed = apply_values(&mut pooled, plug, c, live);
    let handle = handle_of(&pooled.inst);
    let r = c.r.max(1e-9);
    let frame = c.t * c.fps;
    if fresh {
        let s = call(plug.main, "OfxActionCreateInstance", handle, None, None);
        if !ok(s) {
            return Err(format!("create instance failed ({s})"));
        }
    } else if !changed.is_empty() {
        let args = Props::default();
        args.s("OfxPropChangeReason", "OfxChangeUserEdited");
        call(plug.main, "OfxActionBeginInstanceChanged", handle, Some(&args), None);
        for name in &changed {
            let a = Props::default();
            a.s("OfxPropType", "OfxTypeParameter");
            a.s("OfxPropName", name);
            a.s("OfxPropChangeReason", "OfxChangeUserEdited");
            a.d("OfxPropTime", &[frame]);
            a.d("OfxImageEffectPropRenderScale", &[r, r]);
            call(plug.main, "OfxActionInstanceChanged", handle, Some(&a), None);
        }
        call(plug.main, "OfxActionEndInstanceChanged", handle, Some(&args), None);
    }
    let out = f(handle);
    let mut pool = plug.pool.lock();
    if out.is_ok() && pool.len() < cpus() as usize {
        pool.push(pooled);
    } else {
        drop(pool);
        destroy(plug, pooled);
    }
    out
}

fn live_for(plug: &Plugin, c: &FxCtx, w: usize, h: usize) -> Live {
    Live {
        fx: (c as *const FxCtx).cast(),
        ids: plug.map.iter().map(|m| (m.ofx.clone(), m.id)).collect(),
        maps: &plug.map as *const Vec<ParamMap>,
        w: w as f64,
        h: h as f64,
        depth: plug.depth,
        frame: c.t * c.fps,
        arena: Mutex::new(vec![]),
    }
}

fn render_marker() -> PathBuf {
    super::plugin::folder().join(".ofx-rendering")
}

pub fn run(plug: &Plugin, c: &FxCtx, img: &mut Img) -> Result<(), String> {
    let _serial = plug.unsafe_render.as_ref().map(|m| m.lock());
    let (w, h) = (img.w, img.h);
    let mut bufs = Bufs::new(plug.depth, &img.px);
    let ptrs = bufs.ptrs();
    let live = live_for(plug, c, w, h);
    let first = !plug.rendered.load(std::sync::atomic::Ordering::Relaxed);
    if first {
        let _ = std::fs::write(render_marker(), &plug.path);
    }
    let prev = RENDER.with(|r| r.replace(Some(&live as *const Live as usize)));
    let r = c.r.max(1e-9);
    let frame = c.t * c.fps;
    let result = with_instance(plug, c, w, h, ptrs, &live, |handle| {
        let ia = Props::default();
        ia.d("OfxPropTime", &[frame]);
        ia.s("OfxImageEffectPropFieldToRender", "OfxFieldNone");
        ia.i("OfxImageEffectPropRenderWindow", &[0, 0, w as c_int, h as c_int]);
        ia.d("OfxImageEffectPropRenderScale", &[r, r]);
        let io = Props::default();
        io.s("OfxPropName", "");
        io.d("OfxPropTime", &[frame]);
        if call(plug.main, "OfxImageEffectActionIsIdentity", handle, Some(&ia), Some(&io)) == OK {
            let clip = io.get_s("OfxPropName").into_iter().next().unwrap_or_default();
            let t = io.get_d("OfxPropTime").first().copied().unwrap_or(frame);
            if clip == "Source" {
                return Ok(Some(t));
            }
        }
        let seq = Props::default();
        seq.d("OfxImageEffectPropFrameRange", &[frame, frame]);
        seq.d("OfxImageEffectPropFrameStep", &[1.0]);
        seq.i("OfxPropIsInteractive", &[0]);
        seq.d("OfxImageEffectPropRenderScale", &[r, r]);
        seq.i("OfxImageEffectPropSequentialRenderStatus", &[0]);
        seq.i("OfxImageEffectPropInteractiveRenderStatus", &[0]);
        call(plug.main, "OfxImageEffectActionBeginSequenceRender", handle, Some(&seq), None);
        let ra = Props::default();
        ra.d("OfxPropTime", &[frame]);
        ra.s("OfxImageEffectPropFieldToRender", "OfxFieldNone");
        ra.i("OfxImageEffectPropRenderWindow", &[0, 0, w as c_int, h as c_int]);
        ra.d("OfxImageEffectPropRenderScale", &[r, r]);
        ra.i("OfxImageEffectPropSequentialRenderStatus", &[0]);
        ra.i("OfxImageEffectPropInteractiveRenderStatus", &[0]);
        ra.i("OfxImageEffectPropRenderQualityDraft", &[0]);
        let s = call(plug.main, "OfxImageEffectActionRender", handle, Some(&ra), None);
        call(plug.main, "OfxImageEffectActionEndSequenceRender", handle, Some(&seq), None);
        if s != OK {
            return Err(format!("render failed ({s})"));
        }
        Ok(None)
    });
    RENDER.with(|x| x.set(prev));
    if first {
        let _ = std::fs::remove_file(render_marker());
        plug.rendered.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    match result? {
        Some(t) if (t - frame).abs() < 1e-6 => {}
        Some(t) => {
            if let Some(o) = c.frame(t / c.fps.max(1e-6) - c.t).filter(|o| o.w == w && o.h == h) {
                img.px.copy_from_slice(&o.px);
            }
        }
        None => bufs.read(&mut img.px),
    }
    Ok(())
}

fn pad(c: &FxCtx) -> f64 {
    let Some(plug) = find(&c.s("plugin")) else { return 0.0 };
    let (w, h) = (c.size[0].max(1.0).round() as usize, c.size[1].max(1.0).round() as usize);
    let live = live_for(&plug, c, w, h);
    let prev = RENDER.with(|r| r.replace(Some(&live as *const Live as usize)));
    let frame = c.t * c.fps;
    let rod = with_instance(&plug, c, w, h, (0, 0), &live, |handle| {
        let ia = Props::default();
        ia.d("OfxPropTime", &[frame]);
        ia.d("OfxImageEffectPropRenderScale", &[1.0, 1.0]);
        let io = Props::default();
        io.d("OfxImageEffectPropRegionOfDefinition", &[0.0, 0.0, w as f64, h as f64]);
        let s = call(plug.main, "OfxImageEffectActionGetRegionOfDefinition", handle, Some(&ia), Some(&io));
        Ok(if s == OK { io.get_d("OfxImageEffectPropRegionOfDefinition") } else { vec![] })
    });
    RENDER.with(|x| x.set(prev));
    let Ok(v) = rod else { return 0.0 };
    if v.len() < 4 || v.iter().any(|x| !x.is_finite()) {
        return 0.0;
    }
    [-v[0], -v[1], v[2] - w as f64, v[3] - h as f64].into_iter().fold(0.0f64, f64::max).min(4096.0)
}

enum Bufs {
    F(Vec<f32>, Vec<f32>),
    S(Vec<u16>, Vec<u16>),
    B(Vec<u8>, Vec<u8>),
}

impl Bufs {
    fn new(depth: &str, px: &[f32]) -> Bufs {
        match depth {
            "OfxBitDepthShort" => Bufs::S(px.iter().map(|v| (v.clamp(0.0, 1.0) * 65535.0).round() as u16).collect(), vec![0; px.len()]),
            "OfxBitDepthByte" => Bufs::B(px.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect(), vec![0; px.len()]),
            _ => Bufs::F(px.to_vec(), vec![0.0; px.len()]),
        }
    }

    fn src(&self) -> usize {
        match self {
            Bufs::F(a, _) => a.as_ptr() as usize,
            Bufs::S(a, _) => a.as_ptr() as usize,
            Bufs::B(a, _) => a.as_ptr() as usize,
        }
    }

    fn ptrs(&mut self) -> (usize, usize) {
        match self {
            Bufs::F(a, b) => (a.as_ptr() as usize, b.as_mut_ptr() as usize),
            Bufs::S(a, b) => (a.as_ptr() as usize, b.as_mut_ptr() as usize),
            Bufs::B(a, b) => (a.as_ptr() as usize, b.as_mut_ptr() as usize),
        }
    }

    fn read(&self, px: &mut [f32]) {
        match self {
            Bufs::F(_, b) => {
                for (d, s) in px.iter_mut().zip(b) {
                    *d = if s.is_finite() { *s } else { 0.0 };
                }
            }
            Bufs::S(_, b) => {
                for (d, s) in px.iter_mut().zip(b) {
                    *d = *s as f32 / 65535.0;
                }
            }
            Bufs::B(_, b) => {
                for (d, s) in px.iter_mut().zip(b) {
                    *d = *s as f32 / 255.0;
                }
            }
        }
    }
}

fn render(c: &FxCtx, img: &mut Img) {
    let Some(plugin) = find(&c.s("plugin")) else { return };
    let r = run(&plugin, c, img);
    let mut f = failures().write();
    f.retain(|x| x.0 != plugin.id);
    if let Err(e) = r {
        f.push((plugin.id.clone(), e));
    }
}
