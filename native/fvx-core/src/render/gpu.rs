use std::collections::HashMap;
use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Wake, Waker};

use parking_lot::Mutex;
use wgpu::util::DeviceExt;

use super::image::Img;
use super::ocio::{Fixed, Interp, Neg, Op, Proc};
use crate::geom::Mat3;

struct ThreadWaker(std::thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = pin!(f);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::park_timeout(std::time::Duration::from_millis(5));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Backend {
    #[default]
    Auto,
    Gpu,
    Cpu,
}

impl Backend {
    pub fn parse(s: &str) -> Backend {
        match s.to_ascii_lowercase().as_str() {
            "gpu" => Backend::Gpu,
            "cpu" => Backend::Cpu,
            _ => Backend::Auto,
        }
    }
}

struct Pipes {
    composite: wgpu::ComputePipeline,
    box_h: wgpu::ComputePipeline,
    box_v: wgpu::ComputePipeline,
    half: wgpu::ComputePipeline,
    pointwise: wgpu::ComputePipeline,
}

#[derive(Default)]
struct Timing {
    cpu: Vec<f64>,
    gpu: Vec<f64>,
    frames: u64,
}

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pub adapter: String,
    pub backend: String,
    max_bytes: u64,
    pipes: Pipes,
    timing: Mutex<HashMap<u64, Timing>>,
}

impl std::fmt::Debug for Gpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Gpu({})", self.adapter)
    }
}

pub struct Buf {
    b: wgpu::Buffer,
    pub w: usize,
    pub h: usize,
}

#[derive(Clone, Debug)]
pub enum GpuFx {
    Boxes(Vec<(usize, bool)>, bool),
    Program(Vec<f32>, Vec<f32>),
}

static CURRENT: OnceLock<Mutex<Option<Arc<Gpu>>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<Arc<Gpu>>> {
    CURRENT.get_or_init(|| Mutex::new(None))
}

pub fn current() -> Option<Arc<Gpu>> {
    slot().lock().clone()
}

pub fn set_current(g: Option<Arc<Gpu>>) {
    *slot().lock() = g;
}

fn backends() -> wgpu::Backends {
    match std::env::var("FORMVFX_GPU_BACKEND").unwrap_or_default().to_ascii_lowercase().as_str() {
        "vulkan" => wgpu::Backends::VULKAN,
        "gl" | "gles" | "opengl" => wgpu::Backends::GL,
        "metal" => wgpu::Backends::METAL,
        "dx12" => wgpu::Backends::DX12,
        _ => wgpu::Backends::PRIMARY | wgpu::Backends::GL,
    }
}

pub fn open() -> Result<Gpu, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: backends(), ..wgpu::InstanceDescriptor::new_without_display_handle() });
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, force_fallback_adapter: false, compatible_surface: None, apply_limit_buckets: false })).map_err(|e| format!("no GPU adapter: {e}"))?;
    let info = adapter.get_info();
    let limits = adapter.limits();
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("formvfx"), required_features: wgpu::Features::empty(), required_limits: limits.clone(), ..wgpu::DeviceDescriptor::default() })).map_err(|e| format!("the GPU refused a device: {e}"))?;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("formvfx"), source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()) });
    let pipe = |entry: &str| device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(entry), layout: None, module: &module, entry_point: Some(entry), compilation_options: wgpu::PipelineCompilationOptions::default(), cache: None });
    let pipes = Pipes { composite: pipe("composite"), box_h: pipe("box_h"), box_v: pipe("box_v"), half: pipe("half"), pointwise: pipe("pointwise") };
    let max_bytes = (limits.max_storage_buffer_binding_size as u64).min(limits.max_buffer_size);
    Ok(Gpu { device, queue, adapter: format!("{} ({:?})", info.name, info.backend), backend: format!("{:?}", info.backend), max_bytes, pipes, timing: Mutex::new(HashMap::new()) })
}

pub fn probe() -> String {
    match open() {
        Ok(g) => {
            let mut img = Img::new(4, 4);
            img.px.iter_mut().enumerate().for_each(|(i, v)| *v = (i % 7) as f32 / 7.0);
            let ok = g.upload(&img).map(|b| g.download(&b).px == img.px).unwrap_or(false);
            serde_json::json!({"ok": ok, "built": true, "adapter": g.adapter, "backend": g.backend, "error": if ok { serde_json::Value::Null } else { "the GPU read back the wrong values".into() }}).to_string()
        }
        Err(e) => serde_json::json!({"ok": false, "built": true, "adapter": null, "error": e}).to_string(),
    }
}

fn bytes_of(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn groups(n: usize) -> u32 {
    n.div_ceil(16) as u32
}

pub struct Warp {
    pub inv: Mat3,
    pub lscale: f64,
    pub levels: u32,
    pub bbox: [i64; 4],
    pub nearest: bool,
}

pub fn plan(src_w: usize, src_h: usize, m: &Mat3, dst_w: usize, dst_h: usize, nearest: bool) -> Option<Warp> {
    if src_w == 0 || src_h == 0 {
        return None;
    }
    let (sw, sh) = (src_w as f64, src_h as f64);
    let corners = [m.apply(0.0, 0.0), m.apply(sw, 0.0), m.apply(sw, sh), m.apply(0.0, sh)];
    let affine = m.is_affine();
    if !affine {
        let w = &m.0[2];
        if ![(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)].iter().any(|(x, y)| w[0] * x + w[1] * y + w[2] > 0.0) {
            return None;
        }
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for c in corners {
        x0 = x0.min(c[0]);
        y0 = y0.min(c[1]);
        x1 = x1.max(c[0]);
        y1 = y1.max(c[1]);
    }
    if !affine {
        x0 = x0.max(0.0);
        y0 = y0.max(0.0);
        x1 = x1.min(dst_w as f64);
        y1 = y1.min(dst_h as f64);
    }
    let bx0 = (x0.floor() as i64 - 1).max(0);
    let by0 = (y0.floor() as i64 - 1).max(0);
    let bx1 = (x1.ceil() as i64 + 1).min(dst_w as i64);
    let by1 = (y1.ceil() as i64 + 1).min(dst_h as i64);
    if bx1 <= bx0 || by1 <= by0 {
        return None;
    }
    let inv = m.inverse()?;
    let (mut levels, mut lscale) = (0u32, 1.0f64);
    if affine && !nearest {
        let mut s = m.det().abs().sqrt();
        let (mut w, mut h) = (src_w, src_h);
        while s < 0.5 {
            if w <= 2 || h <= 2 {
                break;
            }
            w = (w / 2).max(1);
            h = (h / 2).max(1);
            levels += 1;
            lscale *= 0.5;
            s *= 2.0;
        }
    }
    Some(Warp { inv, lscale, levels, bbox: [bx0, by0, bx1, by1], nearest })
}

impl Gpu {
    fn storage(&self, w: usize, h: usize) -> Option<wgpu::Buffer> {
        let size = (w.max(1) * h.max(1) * 16) as u64;
        if size > self.max_bytes {
            return None;
        }
        Some(self.device.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }))
    }

    pub fn zeros(&self, w: usize, h: usize) -> Option<Buf> {
        let b = self.storage(w, h)?;
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        enc.clear_buffer(&b, 0, None);
        self.queue.submit([enc.finish()]);
        Some(Buf { b, w, h })
    }

    pub fn upload(&self, img: &Img) -> Option<Buf> {
        let b = self.storage(img.w, img.h)?;
        if !img.px.is_empty() {
            self.queue.write_buffer(&b, 0, &bytes_of(&img.px));
        }
        Some(Buf { b, w: img.w, h: img.h })
    }

    pub fn write(&self, buf: &Buf, img: &Img) {
        self.queue.write_buffer(&buf.b, 0, &bytes_of(&img.px));
    }

    pub fn download(&self, buf: &Buf) -> Img {
        let size = (buf.w.max(1) * buf.h.max(1) * 16) as u64;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        enc.copy_buffer_to_buffer(&buf.b, 0, &staging, 0, size);
        self.queue.submit([enc.finish()]);
        let slice = staging.slice(..);
        let done = Arc::new(Mutex::new(None));
        let d2 = done.clone();
        slice.map_async(wgpu::MapMode::Read, move |r| *d2.lock() = Some(r.is_ok()));
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let mut out = Img::new(buf.w, buf.h);
        if done.lock().unwrap_or(false)
            && let Ok(data) = slice.get_mapped_range()
        {
            for (o, c) in out.px.iter_mut().zip(data.chunks_exact(4)) {
                *o = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
            }
        }
        out
    }

    fn dispatch(&self, pipe: &wgpu::ComputePipeline, entries: &[wgpu::BindGroupEntry], gx: u32, gy: u32) {
        let layout = pipe.get_bind_group_layout(0);
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &layout, entries });
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(pipe);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(gx.max(1), gy.max(1), 1);
        }
        self.queue.submit([enc.finish()]);
    }

    fn uniform(&self, words: &[u32]) -> wgpu::Buffer {
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &bytes, usage: wgpu::BufferUsages::UNIFORM })
    }

    fn boxpass(&self, src: &Buf, r: usize, horizontal: bool, clamp: bool) -> Option<Buf> {
        let out = self.storage(src.w, src.h)?;
        let u = self.uniform(&[src.w as u32, src.h as u32, r as u32, clamp as u32]);
        let pipe = if horizontal { &self.pipes.box_h } else { &self.pipes.box_v };
        self.dispatch(pipe, &[wgpu::BindGroupEntry { binding: 0, resource: out.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: src.b.as_entire_binding() }, wgpu::BindGroupEntry { binding: 2, resource: u.as_entire_binding() }], groups(src.w), groups(src.h));
        Some(Buf { b: out, w: src.w, h: src.h })
    }

    pub fn half(&self, src: &Buf) -> Option<Buf> {
        let (w2, h2) = ((src.w / 2).max(1), (src.h / 2).max(1));
        let out = self.storage(w2, h2)?;
        let u = self.uniform(&[src.w as u32, src.h as u32, 0, 0]);
        self.dispatch(&self.pipes.half, &[wgpu::BindGroupEntry { binding: 0, resource: out.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: src.b.as_entire_binding() }, wgpu::BindGroupEntry { binding: 2, resource: u.as_entire_binding() }], groups(w2), groups(h2));
        Some(Buf { b: out, w: w2, h: h2 })
    }

    pub fn program(&self, buf: &Buf, code: &[f32], luts: &[f32]) {
        if code.is_empty() {
            return;
        }
        let prog = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &bytes_of(code), usage: wgpu::BufferUsages::STORAGE });
        let lut_data = if luts.is_empty() { vec![0.0f32; 4] } else { luts.to_vec() };
        let lb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &bytes_of(&lut_data), usage: wgpu::BufferUsages::STORAGE });
        let u = self.uniform(&[buf.w as u32, buf.h as u32, code.len() as u32, 0]);
        self.dispatch(
            &self.pipes.pointwise,
            &[wgpu::BindGroupEntry { binding: 0, resource: buf.b.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: prog.as_entire_binding() }, wgpu::BindGroupEntry { binding: 2, resource: u.as_entire_binding() }, wgpu::BindGroupEntry { binding: 3, resource: lb.as_entire_binding() }],
            groups(buf.w),
            groups(buf.h),
        );
    }

    pub fn effects(&self, mut buf: Buf, fx: &[GpuFx]) -> Option<Buf> {
        for f in fx {
            match f {
                GpuFx::Boxes(passes, clamp) => {
                    for (r, h) in passes {
                        buf = self.boxpass(&buf, *r, *h, *clamp)?;
                    }
                }
                GpuFx::Program(code, luts) => self.program(&buf, code, luts),
            }
        }
        Some(buf)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn composite(&self, dst: &Buf, src: &Buf, w: &Warp, mode: u32, opacity: f32, preserve: bool, quant: f32) {
        let stencil = mode == 32 || mode == 33;
        let (rx0, ry0, rw, rh) = if stencil { (0, 0, dst.w as u32, dst.h as u32) } else { (w.bbox[0] as u32, w.bbox[1] as u32, (w.bbox[2] - w.bbox[0]) as u32, (w.bbox[3] - w.bbox[1]) as u32) };
        let m = &w.inv.0;
        let f = |x: f64| (x as f32).to_bits();
        let words = [
            f(m[0][0]), f(m[0][1]), f(m[0][2]), 0,
            f(m[1][0]), f(m[1][1]), f(m[1][2]), 0,
            f(m[2][0]), f(m[2][1]), f(m[2][2]), 0,
            f(w.lscale), opacity.to_bits(), quant.to_bits(), 0,
            src.w as u32, src.h as u32, dst.w as u32, dst.h as u32,
            rx0, ry0, rw, rh,
            w.bbox[0] as i32 as u32, w.bbox[1] as i32 as u32, w.bbox[2] as i32 as u32, w.bbox[3] as i32 as u32,
            mode, preserve as u32, w.nearest as u32, w.inv.is_affine() as u32,
        ];
        let u = self.uniform(&words);
        self.dispatch(&self.pipes.composite, &[wgpu::BindGroupEntry { binding: 0, resource: dst.b.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: src.b.as_entire_binding() }, wgpu::BindGroupEntry { binding: 2, resource: u.as_entire_binding() }], groups(rw as usize), groups(rh as usize));
    }

    pub fn pick(&self, key: u64) -> Backend {
        let mut t = self.timing.lock();
        let e = t.entry(key).or_default();
        e.frames += 1;
        if e.frames % 240 == 0 {
            e.cpu.clear();
            e.gpu.clear();
        }
        if e.cpu.len() < 2 {
            return Backend::Cpu;
        }
        if e.gpu.len() < 2 {
            return Backend::Gpu;
        }
        let avg = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len() as f64;
        if avg(&e.gpu) < avg(&e.cpu) { Backend::Gpu } else { Backend::Cpu }
    }

    pub fn record(&self, key: u64, b: Backend, ms: f64) {
        let mut t = self.timing.lock();
        let e = t.entry(key).or_default();
        let v = if b == Backend::Gpu { &mut e.gpu } else { &mut e.cpu };
        if v.len() < 4 {
            v.push(ms);
        }
    }

    pub fn stats(&self) -> serde_json::Value {
        let t = self.timing.lock();
        let rows: Vec<serde_json::Value> = t
            .iter()
            .map(|(k, v)| {
                let avg = |v: &Vec<f64>| if v.is_empty() { serde_json::Value::Null } else { serde_json::json!(v.iter().sum::<f64>() / v.len() as f64) };
                serde_json::json!({"comp": k & 0xff_ffff_ffff, "scale": (k >> 40) as f64 / 1000.0, "cpuMs": avg(&v.cpu), "gpuMs": avg(&v.gpu), "frames": v.frames})
            })
            .collect();
        serde_json::json!({"adapter": self.adapter, "comps": rows})
    }
}

pub const CAPABLE: [&str; 11] = [
    "fx.blur.gaussian",
    "fx.blur.box",
    "fx.color.exposure",
    "fx.color.brightnessContrast",
    "fx.color.tint",
    "fx.channel.invert",
    "fx.color.ocioColorSpace",
    "fx.color.ocioDisplay",
    "fx.color.ocioLook",
    "fx.color.ocioFile",
    "fx.color.ocioCdl",
];

pub fn effect(id: &str, c: &crate::effects::FxCtx, quant: f32) -> Option<Vec<GpuFx>> {
    let tail = |mut code: Vec<f32>| {
        if quant > 0.0 {
            code.extend([3.0, quant]);
        }
        code
    };
    let quant_only = || if quant > 0.0 { vec![GpuFx::Program(vec![3.0, quant], vec![])] } else { vec![] };
    let out = match id {
        "fx.blur.gaussian" => {
            let sigma = c.px(c.f("blurriness")) * 0.5;
            let d = c.e("dimensions");
            let (sx, sy) = (if d == 2 { 0.0 } else { sigma }, if d == 1 { 0.0 } else { sigma });
            let mut passes = vec![];
            if sx > 0.3 {
                passes.extend(crate::effects::util::box_sizes(sx).into_iter().filter(|r| *r > 0).map(|r| (r, true)));
            }
            if sy > 0.3 {
                passes.extend(crate::effects::util::box_sizes(sy).into_iter().filter(|r| *r > 0).map(|r| (r, false)));
            }
            let mut v = vec![GpuFx::Boxes(passes, c.b("repeatEdge"))];
            v.extend(quant_only());
            v
        }
        "fx.blur.box" => {
            let r = c.px(c.f("radius")).round() as usize;
            let n = c.f("iterations").round().clamp(1.0, 50.0) as usize;
            let d = c.e("dimensions");
            let mut passes = vec![];
            if r > 0 {
                for _ in 0..n {
                    if d != 2 {
                        passes.push((r, true));
                    }
                    if d != 1 {
                        passes.push((r, false));
                    }
                }
            }
            let mut v = vec![GpuFx::Boxes(passes, c.b("repeatEdge"))];
            v.extend(quant_only());
            v
        }
        "fx.color.exposure" => {
            let k = 2f32.powf(c.f("exposure") as f32);
            let g = 1.0 / c.f("gamma").max(0.01) as f32;
            vec![GpuFx::Program(tail(vec![1.0, 30.0, k, c.f("offset") as f32, g, 2.0]), vec![])]
        }
        "fx.color.brightnessContrast" => {
            let b = (c.f("brightness") / 150.0) as f32 * 0.5;
            let k = c.f("contrast") as f32 / 100.0;
            let k = if k >= 0.0 { 1.0 / (1.0 - k * 0.99) } else { 1.0 + k };
            vec![GpuFx::Program(tail(vec![1.0, 31.0, b, k, 2.0]), vec![])]
        }
        "fx.color.tint" => {
            let (b, w) = (c.c("black"), c.c("white"));
            vec![GpuFx::Program(tail(vec![1.0, 32.0, b[0], b[1], b[2], w[0], w[1], w[2], c.f("amount") as f32 / 100.0, 2.0]), vec![])]
        }
        "fx.channel.invert" => vec![GpuFx::Program(tail(vec![33.0, c.e("channel") as f32, c.f("blend") as f32 / 100.0]), vec![])],
        _ if id.starts_with("fx.color.ocio") => match crate::effects::ocio::proc_of(id, c) {
            None => quant_only(),
            Some(p) => {
                let (mut code, mut luts) = (vec![], vec![]);
                if !encode_proc(&p, &mut code, &mut luts) {
                    return None;
                }
                vec![GpuFx::Program(tail(code), luts)]
            }
        },
        _ => return None,
    };
    Some(out)
}

fn neg_code(n: Neg) -> f32 {
    match n {
        Neg::Clamp => 0.0,
        Neg::Mirror => 1.0,
        Neg::PassThru => 2.0,
        Neg::Linear => 3.0,
    }
}

fn interp_code(i: Interp) -> f32 {
    match i {
        Interp::Nearest => 0.0,
        Interp::Linear => 1.0,
        Interp::Tetrahedral | Interp::Best => 2.0,
    }
}

pub fn encode_proc(p: &Proc, code: &mut Vec<f32>, luts: &mut Vec<f32>) -> bool {
    code.push(1.0);
    for op in &p.ops {
        match op {
            Op::Mat(m, o) => {
                code.push(10.0);
                for r in m {
                    code.extend(r.iter().map(|v| *v as f32));
                }
                code.extend(o.iter().map(|v| *v as f32));
            }
            Op::Pow(e, n) => {
                code.push(11.0);
                code.extend(e.iter().map(|v| *v as f32));
                code.push(neg_code(*n));
            }
            Op::Moncurve { g, o, neg, encode } => {
                code.push(12.0);
                code.extend(g.iter().map(|v| *v as f32));
                code.extend(o.iter().map(|v| *v as f32));
                code.push(neg_code(*neg));
                code.push(*encode as u8 as f32);
            }
            Op::Log { base, inv } => code.extend([13.0, *base as f32, *inv as u8 as f32]),
            Op::LogAff { base, ls, lo, ns, no, brk, lin, inv } => {
                code.push(14.0);
                code.push(*base as f32);
                for a in [ls, lo, ns, no] {
                    code.extend(a.iter().map(|v| *v as f32));
                }
                code.push(brk.is_some() as u8 as f32);
                code.extend(brk.unwrap_or([0.0; 3]).iter().map(|v| *v as f32));
                code.push(lin.is_some() as u8 as f32);
                code.extend(lin.unwrap_or([0.0; 3]).iter().map(|v| *v as f32));
                code.push(*inv as u8 as f32);
            }
            Op::Cdl { s, o, p, sat, clamp, inv } => {
                code.push(15.0);
                for a in [s, o, p] {
                    code.extend(a.iter().map(|v| *v as f32));
                }
                code.extend([*sat as f32, *clamp as u8 as f32, *inv as u8 as f32]);
            }
            Op::Range { scale, off, lo, hi, .. } => code.extend([16.0, *scale as f32, *off as f32, lo.is_some() as u8 as f32, lo.unwrap_or(0.0) as f32, hi.is_some() as u8 as f32, hi.unwrap_or(0.0) as f32]),
            Op::Lut1(l, i, false) => {
                let off = luts.len() as f32;
                for v in &l.data {
                    luts.extend_from_slice(v);
                }
                code.extend([17.0, off, l.data.len() as f32]);
                code.extend(l.min);
                code.extend(l.max);
                code.push(interp_code(*i));
            }
            Op::Lut3(l, i, false) => {
                let off = luts.len() as f32;
                for v in &l.data {
                    luts.extend_from_slice(v);
                }
                code.extend([18.0, off, l.n as f32]);
                code.extend(l.min);
                code.extend(l.max);
                code.push(interp_code(*i));
            }
            Op::Fixed(f, inv) => {
                let (k, prm) = match f {
                    Fixed::Pq => (1.0, 0.0),
                    Fixed::HlgOetf => (2.0, 0.0),
                    Fixed::Cct => (3.0, 0.0),
                    Fixed::Cc => (4.0, 0.0),
                    Fixed::Surround(g) => (5.0, *g as f32),
                    Fixed::Rolloff => (6.0, 0.0),
                    _ => return false,
                };
                code.extend([19.0, k, *inv as u8 as f32, prm]);
            }
            _ => return false,
        }
    }
    code.push(2.0);
    true
}
