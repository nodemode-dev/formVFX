use std::sync::Mutex;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use serde_json::{Value, json};

use crate::engine::state::{AudioJob, FrameJob, PEAK_BINS, PeaksJob, ThumbJob};
use crate::engine::{self, cmd};

fn parse(params: &str) -> PyResult<Value> {
    let t = params.trim();
    if t.is_empty() {
        return Ok(Value::Object(Default::default()));
    }
    serde_json::from_str(t).map_err(|e| PyValueError::new_err(format!("params are not JSON: {e}")))
}

fn err(e: String) -> PyErr {
    PyValueError::new_err(e)
}

#[pyclass(unsendable, module = "fvx_core")]
pub struct Engine {
    e: engine::Engine,
    gesture: u64,
}

#[pymethods]
impl Engine {
    #[new]
    fn new() -> Self {
        Engine { e: engine::Engine::new(), gesture: 0 }
    }

    fn execute(&mut self, id: &str, params: &str) -> PyResult<String> {
        let p = parse(params)?;
        self.e.execute(id, p).map(|v| v.to_string()).map_err(err)
    }

    fn execute_checked(&mut self, id: &str, params: &str) -> PyResult<String> {
        self.execute(id, params)
    }

    fn apply_text_preset(&mut self, params: &str) -> PyResult<String> {
        self.execute("text.applyLibraryPreset", params)
    }

    fn paste_svg(&mut self, params: &str) -> PyResult<String> {
        self.execute("clipboard.pasteSvg", params)
    }

    fn set_gpu(&mut self, py: Python<'_>, on: bool) -> String {
        use crate::render::gpu;
        if !on {
            gpu::set_current(None);
            return json!({"enabled": false, "built": true, "adapter": null}).to_string();
        }
        if let Some(g) = gpu::current() {
            return json!({"enabled": true, "built": true, "adapter": g.adapter}).to_string();
        }
        match py.detach(gpu::open) {
            Ok(g) => {
                let name = g.adapter.clone();
                gpu::set_current(Some(std::sync::Arc::new(g)));
                json!({"enabled": true, "built": true, "adapter": name}).to_string()
            }
            Err(e) => json!({"enabled": false, "built": true, "adapter": null, "error": e}).to_string(),
        }
    }

    fn gpu_stats(&self) -> String {
        crate::render::gpu::current().map(|g| g.stats()).unwrap_or(serde_json::Value::Null).to_string()
    }

    fn set_multi_frame(&mut self, on: bool) {
        self.e.multi_frame = on;
    }

    fn object_job(&self, params: &str) -> PyResult<PyObjectJob> {
        let job = cmd::roto::object_job(&self.e, &parse(params)?).map_err(err)?;
        Ok(PyObjectJob { job })
    }

    fn planar_corner_pin(&mut self, params: &str) -> PyResult<String> {
        cmd::track::planar(&mut self.e, &parse(params)?).map(|v| v.to_string()).map_err(err)
    }

    fn is_enabled(&self, id: &str) -> bool {
        engine::registry::find(id).is_some()
    }

    fn revision(&self) -> u64 {
        self.e.revision
    }

    fn dirty(&self) -> bool {
        self.e.is_dirty()
    }

    fn path(&self) -> Option<String> {
        self.e.path.clone()
    }

    fn drain_events(&mut self) -> String {
        Value::Array(self.e.drain_events()).to_string()
    }

    fn tick(&mut self, now: f64) -> String {
        self.e.tick(now).to_string()
    }

    fn begin_gesture(&mut self) -> String {
        self.gesture = self.gesture.wrapping_add(1);
        format!("g-{}", self.gesture)
    }

    fn end_gesture(&mut self) {
        self.e.end_gesture();
    }

    fn menu_bar(&self, params: &str) -> PyResult<String> {
        Ok(self.e.menu_bar(&parse(params)?).to_string())
    }

    fn menu_titles(&self) -> String {
        self.e.menu_titles().to_string()
    }

    fn ui_state(&self, params: &str) -> PyResult<String> {
        Ok(self.e.ui_state(&parse(params)?).to_string())
    }

    fn overlays(&self, params: &str) -> PyResult<String> {
        self.e.overlays(&parse(params)?).map(|v| v.to_string()).map_err(err)
    }

    fn graph(&self, params: &str) -> PyResult<String> {
        self.e.graph(&parse(params)?).map(|v| v.to_string()).map_err(err)
    }

    fn snapshot<'py>(&self, py: Python<'py>) -> Option<(u32, u32, Bound<'py, PyBytes>)> {
        let s = self.e.state.snapshot.as_ref()?;
        Some((s.width, s.height, PyBytes::new(py, &s.px)))
    }

    fn flowchart(&self, params: &str) -> PyResult<String> {
        self.e.flowchart(&parse(params)?).map(|v| v.to_string()).map_err(err)
    }

    fn frame_job(&self, params: &str) -> PyResult<PyFrameJob> {
        let job = FrameJob::from_params(&self.e, &parse(params)?).map_err(err)?;
        Ok(PyFrameJob { rev: job.rev, job })
    }

    fn audio_job(&self, params: &str) -> PyResult<PyAudioJob> {
        let job = AudioJob::from_params(&self.e, &parse(params)?).map_err(err)?;
        Ok(PyAudioJob { job })
    }

    fn thumbnail_job(&self, item: u64, time: f64, max_side: u32) -> Option<PyThumbJob> {
        ThumbJob::new(&self.e, item, time, max_side).map(|j| PyThumbJob { job: Mutex::new(Some(j)) })
    }

    fn peaks_job(&self, item: u64) -> Option<PyPeaksJob> {
        PeaksJob::new(&self.e, item).map(|job| PyPeaksJob { key: job.key.clone(), job })
    }

    fn effects(&self) -> String {
        cmd::effect::catalog().to_string()
    }

    fn capabilities(&mut self) -> PyResult<String> {
        Ok(self.e.capabilities().to_string())
    }

    fn project_json(&self) -> String {
        serde_json::to_string(&*self.e.project).unwrap_or_default()
    }
}

#[pyclass(module = "fvx_core", name = "FrameJob")]
pub struct PyFrameJob {
    job: FrameJob,
    #[pyo3(get)]
    rev: u64,
}

#[pymethods]
impl PyFrameJob {
    fn run<'py>(&self, py: Python<'py>) -> (u32, u32, Bound<'py, PyBytes>) {
        let job = &self.job;
        let (w, h, px) = py.detach(|| job.run());
        (w, h, PyBytes::new(py, &px))
    }
}

#[pyclass(module = "fvx_core", name = "ObjectJob")]
pub struct PyObjectJob {
    job: cmd::roto::ObjectJob,
}

#[pymethods]
impl PyObjectJob {
    fn run(&self, py: Python<'_>) -> PyResult<String> {
        let job = &self.job;
        py.detach(|| job.run()).map(|v| v.to_string()).map_err(err)
    }
}

#[pyclass(module = "fvx_core", name = "AudioJob")]
pub struct PyAudioJob {
    job: AudioJob,
}

#[pymethods]
impl PyAudioJob {
    fn run<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let job = &self.job;
        let samples = py.detach(|| job.run());
        let mut bytes = Vec::with_capacity(samples.len() * 4);
        for s in samples {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        PyBytes::new(py, &bytes)
    }
}

#[pyclass(module = "fvx_core", name = "ThumbJob")]
pub struct PyThumbJob {
    job: Mutex<Option<ThumbJob>>,
}

#[pymethods]
impl PyThumbJob {
    fn run<'py>(&self, py: Python<'py>) -> Option<(u32, u32, Bound<'py, PyBytes>)> {
        let job = self.job.lock().ok()?.take()?;
        let (w, h, px) = py.detach(move || job.run())?;
        Some((w, h, PyBytes::new(py, &px)))
    }
}

#[pyclass(module = "fvx_core", name = "PeaksJob")]
pub struct PyPeaksJob {
    job: PeaksJob,
    #[pyo3(get)]
    key: String,
}

#[pymethods]
impl PyPeaksJob {
    #[getter]
    fn bins_per_sec(&self) -> u32 {
        PEAK_BINS
    }

    fn run<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let job = &self.job;
        let peaks = py.detach(|| job.run());
        let mut bytes = Vec::with_capacity(peaks.len() * 16);
        for p in peaks {
            for v in p {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        PyBytes::new(py, &bytes)
    }
}

#[pyfunction]
fn encode_png<'py>(py: Python<'py>, width: u32, height: u32, rgba: &[u8]) -> PyResult<Bound<'py, PyBytes>> {
    let out = py
        .detach(|| {
            use image::ImageEncoder;
            let mut out = Vec::new();
            image::codecs::png::PngEncoder::new(&mut out).write_image(rgba, width, height, image::ExtendedColorType::Rgba8).map(|_| out).map_err(|e| e.to_string())
        })
        .map_err(err)?;
    Ok(PyBytes::new(py, &out))
}

#[pyfunction]
fn version() -> String {
    json!({"core": env!("CARGO_PKG_VERSION"), "engine": "fvx"}).to_string()
}

#[pyfunction]
fn gpu_probe(py: Python<'_>) -> String {
    py.detach(crate::render::gpu::probe)
}

#[pymodule]
fn fvx_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    m.add_class::<PyFrameJob>()?;
    m.add_class::<PyAudioJob>()?;
    m.add_class::<PyThumbJob>()?;
    m.add_class::<PyPeaksJob>()?;
    m.add_class::<PyObjectJob>()?;
    m.add_function(wrap_pyfunction!(encode_png, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(gpu_probe, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
