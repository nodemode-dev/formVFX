use std::collections::HashMap;
use std::sync::OnceLock;

use crate::model::prop::{Group, GroupKind, Ids, Node, Ui};
use crate::model::Value;
use crate::render::image::Img;

pub mod audiofx;
pub mod auxfx;
pub mod blur;
pub mod blur2;
pub mod channel;
pub mod color;
pub mod color2;
pub mod controls;
pub mod curl;
pub mod distort;
pub mod distort2;
pub mod draw;
pub mod generate;
pub mod generate2;
pub mod keying;
pub mod motionfx;
pub mod stylize;
pub mod stylize2;
pub mod noise2;
pub mod ocio;
pub mod paint;
pub mod plugin;
pub mod puppet;
pub mod roto;
pub mod simulation;
pub mod timefx;
pub mod textutil;
pub mod tracking;
pub mod transition;
pub mod vr;
pub mod util;

#[derive(Clone, Debug)]
pub enum Init {
    V(Value),
    RelPoint(f64, f64),
    Group,
}

pub trait FxHost: Sync {
    fn frame(&self, dt: f64) -> Option<Img>;
    fn original(&self) -> Option<Img>;
    fn param_at(&self, id: &str, dt: f64) -> Option<Value>;
    fn layer(&self, id: u64, with_fx: bool, dt: f64) -> Option<(Img, [f64; 2])>;
    fn layer_in_comp(&self, id: u64, dt: f64) -> Option<Img>;
    fn mask(&self, index: usize) -> Option<crate::model::BezPath>;
    fn audio(&self, id: Option<u64>, t0: f64, t1: f64, rate: u32) -> Vec<f32>;
    fn comp_time(&self) -> f64;
    fn frame_rate(&self) -> f64;
    fn stretch(&self) -> f64;
    fn aux(&self) -> Option<std::sync::Arc<crate::render::exr::Aux>> {
        None
    }
    fn shutter_angle(&self) -> f64 {
        180.0
    }
}

#[derive(Clone, Debug)]
pub struct ParamSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub init: Init,
    pub ui: Ui,
}

pub type RenderFn = fn(&FxCtx, &mut Img);
pub type PadFn = fn(&FxCtx) -> f64;

pub struct EffectSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub params: Vec<ParamSpec>,
    pub render: RenderFn,
    pub pad: PadFn,
}

pub fn no_pad(_: &FxCtx) -> f64 {
    0.0
}

pub struct FxCtx<'a> {
    pub p: &'a HashMap<String, Value>,
    pub t: f64,
    pub r: f64,
    pub origin: [f64; 2],
    pub size: [f64; 2],
    pub fps: f64,
    pub host: Option<&'a dyn FxHost>,
    pub cconv: Option<&'a crate::render::ocio::Proc>,
    pub settings: Option<&'a crate::model::ProjectSettings>,
}

impl FxCtx<'_> {
    pub fn s(&self, id: &str) -> String {
        match self.p.get(id) {
            Some(Value::Str(s)) => s.clone(),
            _ => String::new(),
        }
    }
    pub fn layer_id(&self, id: &str) -> Option<u64> {
        self.p.get(id).and_then(|v| v.layer())
    }
    pub fn other(&self, id: &str, with_fx: bool) -> Option<(Img, [f64; 2])> {
        let lid = self.layer_id(id)?;
        self.host?.layer(lid, with_fx, 0.0)
    }
    pub fn other_fit(&self, id: &str, with_fx: bool, w: usize, h: usize, mode: u32) -> Option<Img> {
        let (img, size) = self.other(id, with_fx)?;
        Some(util::fit_layer(&img, size, self, w, h, mode))
    }
    pub fn frame(&self, dt: f64) -> Option<Img> {
        self.host?.frame(dt)
    }
    pub fn mask(&self, index: usize) -> Option<crate::model::BezPath> {
        if index == 0 {
            return None;
        }
        self.host?.mask(index)
    }
    pub fn pt_at(&self, id: &str, dt: f64) -> [f64; 2] {
        match self.host.and_then(|h| h.param_at(id, dt)) {
            Some(v) => self.to_buf(v.vec2()),
            None => self.pt(id),
        }
    }
    pub fn f_at(&self, id: &str, dt: f64) -> f64 {
        self.host.and_then(|h| h.param_at(id, dt)).map(|v| v.num()).unwrap_or_else(|| self.f(id))
    }
    pub fn comp_time(&self) -> f64 {
        self.host.map(|h| h.comp_time()).unwrap_or(self.t)
    }
    pub fn f(&self, id: &str) -> f64 {
        self.p.get(id).map(|v| v.num()).unwrap_or(0.0)
    }
    pub fn b(&self, id: &str) -> bool {
        self.p.get(id).map(|v| v.boolean()).unwrap_or(false)
    }
    pub fn e(&self, id: &str) -> u32 {
        self.p.get(id).map(|v| v.index()).unwrap_or(0)
    }
    pub fn c(&self, id: &str) -> [f32; 4] {
        let c = self.p.get(id).map(|v| v.color()).unwrap_or([1.0; 4]);
        let rgb = match self.cconv {
            Some(p) => p.apply([c[0], c[1], c[2]]),
            None => [c[0], c[1], c[2]],
        };
        [rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, c[3] as f32]
    }
    pub fn c_raw(&self, id: &str) -> [f32; 4] {
        let c = self.p.get(id).map(|v| v.color()).unwrap_or([1.0; 4]);
        [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32]
    }
    pub fn v2(&self, id: &str) -> [f64; 2] {
        self.p.get(id).map(|v| v.vec2()).unwrap_or([0.0; 2])
    }
    pub fn pt(&self, id: &str) -> [f64; 2] {
        let p = self.v2(id);
        self.to_buf(p)
    }
    pub fn to_buf(&self, p: [f64; 2]) -> [f64; 2] {
        [(p[0] - self.origin[0]) * self.r, (p[1] - self.origin[1]) * self.r]
    }
    pub fn px(&self, len: f64) -> f64 {
        len * self.r
    }
    pub fn layer_rect(&self) -> [f64; 4] {
        let a = self.to_buf([0.0, 0.0]);
        let b = self.to_buf(self.size);
        [a[0], a[1], b[0], b[1]]
    }
}

pub fn p(id: &'static str, name: &'static str, v: Value, ui: Ui) -> ParamSpec {
    ParamSpec { id, name, init: Init::V(v), ui }
}

pub fn num(id: &'static str, name: &'static str, v: f64, min: f64, max: f64, smin: f64, smax: f64) -> ParamSpec {
    p(id, name, Value::Num(v), Ui::slider(min, max, smin, smax, 1))
}

pub fn pct(id: &'static str, name: &'static str, v: f64) -> ParamSpec {
    p(id, name, Value::Num(v), Ui::slider(0.0, 100.0, 0.0, 100.0, 1))
}

pub fn angle(id: &'static str, name: &'static str, v: f64) -> ParamSpec {
    p(id, name, Value::Num(v), Ui::Angle)
}

pub fn check(id: &'static str, name: &'static str, v: bool) -> ParamSpec {
    p(id, name, Value::Bool(v), Ui::Checkbox)
}

pub fn color(id: &'static str, name: &'static str, c: [f64; 3]) -> ParamSpec {
    p(id, name, Value::Color([c[0], c[1], c[2], 1.0]), Ui::Color)
}

pub fn popup(id: &'static str, name: &'static str, v: u32, opts: &[&str]) -> ParamSpec {
    p(id, name, Value::Enum(v), Ui::popup(opts))
}

pub fn point(id: &'static str, name: &'static str, fx: f64, fy: f64) -> ParamSpec {
    ParamSpec { id, name, init: Init::RelPoint(fx, fy), ui: Ui::Point }
}

pub fn layer_param(id: &'static str, name: &'static str) -> ParamSpec {
    p(id, name, Value::Layer(None), Ui::Layer)
}

pub fn grp(id: &'static str, name: &'static str) -> ParamSpec {
    ParamSpec { id, name, init: Init::Group, ui: Ui::Plain }
}

pub fn text(id: &'static str, name: &'static str, v: &str) -> ParamSpec {
    p(id, name, Value::Str(v.to_string()), Ui::Plain)
}

pub fn hidden(id: &'static str, name: &'static str, v: &str) -> ParamSpec {
    p(id, name, Value::Str(v.to_string()), Ui::Hidden)
}

pub fn mask_param(id: &'static str, name: &'static str) -> ParamSpec {
    p(id, name, Value::Num(0.0), Ui::Mask)
}

pub fn slider(id: &'static str, name: &'static str, v: f64, min: f64, max: f64, decimals: u8) -> ParamSpec {
    p(id, name, Value::Num(v), Ui::slider(min, max, min, max, decimals))
}

pub fn v2(id: &'static str, name: &'static str, v: [f64; 2]) -> ParamSpec {
    p(id, name, Value::Vec2(v), Ui::Plain)
}

static ALL: OnceLock<Vec<EffectSpec>> = OnceLock::new();

pub fn all() -> &'static [EffectSpec] {
    ALL.get_or_init(|| {
        let mut v = vec![];
        v.extend(blur::specs());
        v.extend(color::specs());
        v.extend(stylize::specs());
        v.extend(generate::specs());
        v.extend(distort::specs());
        v.extend(curl::specs());
        v.extend(keying::specs());
        v.extend(color2::specs());
        v.extend(ocio::specs());
        v.extend(channel::specs());
        v.extend(blur2::specs());
        v.extend(distort2::specs());
        v.extend(generate2::specs());
        v.extend(stylize2::specs());
        v.extend(noise2::specs());
        v.extend(timefx::specs());
        v.extend(transition::specs());
        v.extend(textutil::specs());
        v.extend(vr::specs());
        v.extend(simulation::specs());
        v.extend(audiofx::specs());
        v.extend(auxfx::specs());
        v.extend(motionfx::specs());
        v.extend(tracking::specs());
        v.extend(paint::specs());
        v.extend(puppet::specs());
        v.extend(roto::specs());
        v.extend(controls::specs());
        v
    })
}

pub fn passive(id: &str) -> bool {
    controls::is_control(id) || audiofx::is_audio(id) || id.starts_with("fx.face.")
}

pub fn listed(e: &EffectSpec) -> bool {
    e.category != tracking::CAT && e.id != paint::ID && e.id != roto::ID
}

fn flat(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

pub fn every() -> Vec<&'static EffectSpec> {
    let mut v: Vec<&'static EffectSpec> = all().iter().collect();
    v.extend(plugin::specs());
    v
}

pub fn find(key: &str) -> Option<&'static EffectSpec> {
    let a = every();
    if let Some(e) = a.iter().find(|e| e.id == key) {
        return Some(e);
    }
    let k = flat(key);
    a.into_iter().find(|e| flat(e.name) == k || flat(e.id) == k || flat(e.id.rsplit('.').next().unwrap_or("")) == k)
}

fn place(children: &mut Vec<Node>, path: &[&str], node: Node, ids: &mut Ids, names: &HashMap<&str, &str>, prefix: String) {
    if path.len() == 1 {
        children.push(node);
        return;
    }
    let seg = path[0];
    let full = if prefix.is_empty() { seg.to_string() } else { format!("{prefix}/{seg}") };
    let at = children.iter().position(|c| matches!(c, Node::Group(g) if g.id == seg));
    let i = match at {
        Some(i) => i,
        None => {
            let name = names.get(full.as_str()).copied().unwrap_or(seg);
            children.push(Node::Group(ids.group(seg, name, GroupKind::Plain, vec![])));
            children.len() - 1
        }
    };
    if let Node::Group(g) = &mut children[i] {
        place(&mut g.children, &path[1..], node, ids, names, full);
    }
}

pub fn instance(spec: &EffectSpec, ids: &mut Ids, name: &str, size: [f64; 2]) -> Group {
    let names: HashMap<&str, &str> = spec.params.iter().filter(|ps| matches!(ps.init, Init::Group)).map(|ps| (ps.id, ps.name)).collect();
    let mut children = vec![];
    for ps in &spec.params {
        let v = match &ps.init {
            Init::V(v) => v.clone(),
            Init::RelPoint(fx, fy) => Value::Vec2([size[0] * fx, size[1] * fy]),
            Init::Group => continue,
        };
        let path: Vec<&str> = ps.id.split('/').collect();
        let leaf = path[path.len() - 1];
        let node = Node::Prop(ids.prop(leaf, ps.name, v, ps.ui.clone()));
        place(&mut children, &path, node, ids, &names, String::new());
    }
    ids.group(spec.id, name, GroupKind::Effect { effect: spec.id.to_string() }, children)
}

fn collect(ev: &crate::eval::Eval, layer: &crate::model::Layer, g: &Group, prefix: &str, m: &mut HashMap<String, Value>) {
    for c in &g.children {
        match c {
            Node::Prop(pr) => {
                let k = if prefix.is_empty() { pr.id.clone() } else { format!("{prefix}/{}", pr.id) };
                m.insert(k, ev.value(layer, pr));
            }
            Node::Group(sub) => {
                let k = if prefix.is_empty() { sub.id.clone() } else { format!("{prefix}/{}", sub.id) };
                collect(ev, layer, sub, &k, m);
            }
        }
    }
}

pub fn params_at(ev: &crate::eval::Eval, layer: &crate::model::Layer, g: &Group) -> HashMap<String, Value> {
    let mut m = HashMap::new();
    collect(ev, layer, g, "", &mut m);
    m
}
