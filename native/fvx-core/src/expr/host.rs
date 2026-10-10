use std::rc::Rc;

use super::interp::*;
use super::noise;
use crate::eval::Eval;
use crate::geom::{Mat3, Mat4};
use crate::model::{BezPath, Comp, Group, ItemKind, Layer, LayerKind, Node, Project, Prop, Ui, Value};

#[derive(Clone, Debug, PartialEq)]
pub enum H {
    Comp(u64),
    Footage(u64),
    Layer(u64, u64),
    Group(u64, u64, u64),
    Prop(u64, u64, u64),
    Key(u64, u64, u64, usize),
    Marker(u64, Option<u64>),
    MarkerKey(u64, Option<u64>, usize),
    App,
    Project,
    Items(Option<u64>),
    Folder(u64),
    Layers(u64),
    Queue,
    QueueItem(u64),
    Window(u64),
    Control(u64, u64),
}

impl H {
    pub fn callable(&self) -> bool {
        matches!(self, H::Layer(..) | H::Group(..))
    }

    pub fn describe(&self) -> String {
        if let Some(d) = super::script::describe(self) {
            return d;
        }
        match self {
            H::Comp(_) => "[object Comp]",
            H::Footage(_) => "[object Footage]",
            H::Layer(..) => "[object Layer]",
            H::Group(..) => "[object PropertyGroup]",
            H::Prop(..) => "[object Property]",
            H::Key(..) => "[object Key]",
            H::Marker(..) => "[object MarkerProperty]",
            H::MarkerKey(..) => "[object MarkerKey]",
            _ => "[object Object]",
        }
        .into()
    }
}

pub fn hv(h: H) -> V {
    V::Host(Rc::new(h))
}

pub struct Cx<'a> {
    pub project: &'a Project,
    pub comp_id: u64,
    pub comp: &'a Comp,
    pub layer: &'a Layer,
    pub prop: &'a Prop,
    pub time: f64,
    seed: f64,
    timeless: bool,
    count: u64,
    pub text: Option<[f64; 5]>,
}

impl<'a> Cx<'a> {
    pub fn new(project: &'a Project, comp_id: u64, comp: &'a Comp, layer: &'a Layer, prop: &'a Prop, time: f64) -> Cx<'a> {
        Cx { project, comp_id, comp, layer, prop, time, seed: 0.0, timeless: false, count: 0, text: None }
    }

    pub fn next_random(&mut self) -> f64 {
        self.count += 1;
        let frame = if self.timeless { 0 } else { (self.time * self.comp.frame_rate.max(1e-6) + 1e-6).floor() as i64 as u64 };
        match self.text {
            Some(t) => noise::unit(noise::hash(&[self.layer.id, self.prop.uid, self.seed.to_bits(), frame, self.count, t[0].to_bits()])),
            None => noise::unit(noise::hash(&[self.layer.id, self.prop.uid, self.seed.to_bits(), frame, self.count])),
        }
    }

    pub fn seed_random(&mut self, seed: f64, timeless: bool) {
        self.seed = seed;
        self.timeless = timeless;
        self.count = 0;
    }

    pub fn comp_rate_start(&self) -> (f64, f64) {
        (self.comp.frame_rate.max(1e-6), self.comp.display_start)
    }

    pub fn comp_of(&self, id: u64) -> Option<&'a Comp> {
        self.project.comp(id)
    }

    pub fn layer_of(&self, c: u64, l: u64) -> Option<(&'a Comp, &'a Layer)> {
        let comp = self.comp_of(c)?;
        Some((comp, comp.layer(l)?))
    }

    pub fn prop_of(&self, c: u64, l: u64, u: u64) -> Option<(&'a Comp, &'a Layer, &'a Prop)> {
        let (comp, layer) = self.layer_of(c, l)?;
        Some((comp, layer, layer.props.find(u)?))
    }

    pub fn group_of(&self, c: u64, l: u64, u: u64) -> Option<(&'a Comp, &'a Layer, &'a Group)> {
        let (comp, layer) = self.layer_of(c, l)?;
        Some((comp, layer, layer.props.find_group(u)?))
    }

    pub fn is_self(&self, c: u64, layer: &Layer, prop: &Prop) -> bool {
        c == self.comp_id && layer.id == self.layer.id && prop.uid == self.prop.uid
    }

    pub fn eval_prop(&self, c: u64, layer: &'a Layer, prop: &'a Prop, t: f64) -> Value {
        if self.is_self(c, layer, prop) {
            return prop.at(layer.layer_time(t));
        }
        match self.comp_of(c) {
            Some(comp) => Eval::new(self.project, c, comp, t).value(layer, prop),
            None => prop.at(layer.layer_time(t)),
        }
    }

    fn eval_at(&self, c: u64, t: f64) -> Option<Eval<'a>> {
        let comp = self.comp_of(c)?;
        Some(Eval::new(self.project, c, comp, t))
    }
}

fn missing() -> Flow {
    throw("Error", "the layer or property this refers to no longer exists")
}

fn dims_for(layer: &Layer, prop: &Prop) -> usize {
    match &prop.value {
        Value::Vec3(_) => {
            if matches!(prop.ui, Ui::Point3) || layer.is_3d() {
                3
            } else {
                2
            }
        }
        Value::Vec2(_) => 2,
        Value::Color(_) => 4,
        _ => 1,
    }
}

pub fn to_v(c: u64, layer: &Layer, prop: &Prop, v: &Value) -> V {
    match v {
        Value::Num(n) => V::Num(*n),
        Value::Vec2(a) => nums(a),
        Value::Vec3(a) => {
            if dims_for(layer, prop) == 3 {
                nums(a)
            } else {
                nums(&a[..2])
            }
        }
        Value::Color(col) => nums(col),
        Value::Bool(b) => V::Num(if *b { 1.0 } else { 0.0 }),
        Value::Enum(i) => V::Num(*i as f64 + 1.0),
        Value::Text(d) => s(&d.text),
        Value::Str(t) => s(t),
        Value::Layer(Some(id)) => hv(H::Layer(c, *id)),
        Value::Layer(None) => V::Null,
        Value::Path(p) => V::Path(Rc::new(p.clone())),
        Value::Gradient(_) => V::Null,
    }
}

fn components(it: &mut Interp, v: &V) -> Result<Vec<f64>, Flow> {
    match it.vector(v)? {
        Some(x) => Ok(x),
        None => Ok(vec![it.to_number(v)?]),
    }
}

pub fn from_v(it: &mut Interp, raw: &V, like: &Value) -> Result<Value, Flow> {
    if matches!(raw, V::Undef) {
        return Err(type_err("the expression gave no value (its last line should be the result)"));
    }
    let v = it.primitive(raw)?;
    let numeric = |x: &[f64]| -> Result<(), Flow> {
        if x.iter().any(|n| n.is_nan()) {
            return Err(type_err("the expression result is not a number (NaN)"));
        }
        Ok(())
    };
    let out = match like {
        Value::Num(_) => {
            let x = components(it, &v)?;
            let n = x.first().copied().unwrap_or(f64::NAN);
            numeric(&[n])?;
            Value::Num(n)
        }
        Value::Vec2(_) => {
            let x = components(it, &v)?;
            let a = [x.first().copied().unwrap_or(0.0), x.get(1).or(x.first()).copied().unwrap_or(0.0)];
            numeric(&a)?;
            Value::Vec2(a)
        }
        Value::Vec3(p) => {
            let x = components(it, &v)?;
            let a = match x.len() {
                0 => *p,
                1 => [x[0], x[0], p[2]],
                2 => [x[0], x[1], p[2]],
                _ => [x[0], x[1], x[2]],
            };
            numeric(&a)?;
            Value::Vec3(a)
        }
        Value::Color(_) => {
            let x = components(it, &v)?;
            let g = |i: usize, d: f64| x.get(i).copied().unwrap_or(d);
            let a = [g(0, 0.0), g(1, g(0, 0.0)), g(2, g(0, 0.0)), g(3, 1.0)];
            numeric(&a)?;
            Value::Color(a)
        }
        Value::Bool(_) => Value::Bool(truthy(&v)),
        Value::Enum(_) => {
            let n = it.to_number(&v)?;
            numeric(&[n])?;
            Value::Enum((n.round() - 1.0).max(0.0) as u32)
        }
        Value::Text(d) if super::textstyle::is_style(&v) => Value::Text(Box::new(super::textstyle::apply(it, &v, d)?)),
        Value::Text(d) => {
            let t = it.to_string(&v)?;
            let mut doc = (**d).clone();
            doc.text = t.replace("\r\n", "\n").replace('\r', "\n");
            Value::Text(Box::new(doc))
        }
        Value::Str(_) => Value::Str(it.to_string(&v)?),
        Value::Layer(_) => match &v {
            V::Host(h) => match &**h {
                H::Layer(_, id) => Value::Layer(Some(*id)),
                _ => return Err(type_err("a layer property needs a layer")),
            },
            V::Null | V::Undef => Value::Layer(None),
            other => {
                let n = it.to_number(other)?;
                let id = it.cx.comp.layers.get((n as usize).wrapping_sub(1)).map(|l| l.id);
                Value::Layer(id)
            }
        },
        Value::Path(_) => match &v {
            V::Path(p) => Value::Path((**p).clone()),
            _ => return Err(type_err("a path property needs a path (use createPath)")),
        },
        Value::Gradient(_) => like.clone(),
    };
    Ok(out)
}

pub fn context(it: &mut Interp, name: &str) -> Result<Option<V>, Flow> {
    let cx = &it.cx;
    Ok(Some(match name {
        "time" => V::Num(cx.time),
        "value" => to_v(cx.comp_id, cx.layer, cx.prop, &cx.prop.at(cx.layer.layer_time(cx.time))),
        "thisComp" => hv(H::Comp(cx.comp_id)),
        "thisLayer" => hv(H::Layer(cx.comp_id, cx.layer.id)),
        "thisProperty" => hv(H::Prop(cx.comp_id, cx.layer.id, cx.prop.uid)),
        "colorDepth" => V::Num(cx.project.settings.bit_depth as f64),
        "textIndex" if cx.text.is_some() => V::Num(cx.text.map(|t| t[0]).unwrap_or(0.0)),
        "textTotal" if cx.text.is_some() => V::Num(cx.text.map(|t| t[1]).unwrap_or(0.0)),
        "selectorValue" if cx.text.is_some() => {
            let t = cx.text.unwrap_or_default();
            crate::expr::interp::nums(&[t[2], t[3], t[4]])
        }
        _ => return Ok(None),
    }))
}

const PROP_IMPLICIT: [&str; 20] = [
    "wiggle", "temporalWiggle", "smooth", "loopIn", "loopOut", "loopInDuration", "loopOutDuration", "valueAtTime", "velocityAtTime", "speedAtTime", "velocity", "speed", "numKeys", "key", "nearestKey", "propertyGroup", "points", "inTangents", "outTangents", "isClosed",
];

pub fn implicit(it: &mut Interp, name: &str) -> Result<Option<V>, Flow> {
    let (c, l, u) = (it.cx.comp_id, it.cx.layer.id, it.cx.prop.uid);
    if PROP_IMPLICIT.contains(&name) {
        let h = Rc::new(H::Prop(c, l, u));
        return Ok(Some(get(it, &h, name)?));
    }
    let layer = it.cx.layer;
    layer_member(it, c, layer, name)
}

pub fn this_layer(it: &mut Interp) -> V {
    hv(H::Layer(it.cx.comp_id, it.cx.layer.id))
}

pub fn primitive(it: &mut Interp, h: &Rc<H>) -> R {
    match &**h {
        H::Prop(c, l, u) => {
            let (_, layer, prop) = it.cx.prop_of(*c, *l, *u).ok_or_else(missing)?;
            let v = it.cx.eval_prop(*c, layer, prop, it.cx.time);
            Ok(to_v(*c, layer, prop, &v))
        }
        H::Key(c, l, u, i) => {
            let (_, layer, prop) = it.cx.prop_of(*c, *l, *u).ok_or_else(missing)?;
            let k = prop.keys.get(*i).ok_or_else(missing)?;
            Ok(to_v(*c, layer, prop, &k.value))
        }
        _ => Ok(V::Host(h.clone())),
    }
}

pub fn get(it: &mut Interp, h: &Rc<H>, key: &str) -> R {
    match &**h {
        H::Comp(c) => comp_member(it, *c, key, h),
        H::Footage(id) => footage_member(it, *id, key),
        H::Layer(c, l) => {
            let (_, layer) = it.cx.layer_of(*c, *l).ok_or_else(missing)?;
            Ok(layer_member(it, *c, layer, key)?.unwrap_or(V::Undef))
        }
        H::Group(c, l, u) => group_member(it, *c, *l, *u, key, h),
        H::Prop(c, l, u) => prop_member(it, *c, *l, *u, key, h),
        H::Key(c, l, u, i) => {
            let (_, layer, prop) = it.cx.prop_of(*c, *l, *u).ok_or_else(missing)?;
            let k = prop.keys.get(*i).ok_or_else(missing)?;
            match key {
                "time" => Ok(V::Num(layer.comp_time(k.time))),
                "value" => Ok(to_v(*c, layer, prop, &k.value)),
                "index" => Ok(V::Num(*i as f64 + 1.0)),
                _ => {
                    let p = primitive(it, h)?;
                    it.get_member(&p, key)
                }
            }
        }
        H::Marker(c, l) => match key {
            "numKeys" => Ok(V::Num(markers(it, *c, *l).len() as f64)),
            "key" => Ok(native("key", V::Host(h.clone()), marker_key)),
            "nearestKey" => Ok(native("nearestKey", V::Host(h.clone()), marker_nearest)),
            _ => Ok(V::Undef),
        },
        H::MarkerKey(c, l, i) => {
            let ms = markers(it, *c, *l);
            let m = ms.get(*i).ok_or_else(missing)?;
            Ok(match key {
                "time" => V::Num(m.time),
                "duration" => V::Num(m.duration),
                "comment" => s(&m.comment),
                "chapter" => s(&m.chapter),
                "index" => V::Num(*i as f64 + 1.0),
                "label" => V::Num(m.label as f64),
                "protectedRegion" => V::Bool(m.protected),
                "url" | "frameTarget" | "cuePointName" => s(""),
                "eventCuePoint" => V::Bool(false),
                "parameters" => obj(vec![]),
                _ => V::Undef,
            })
        }
        _ => Ok(V::Undef),
    }
}

fn markers<'a>(it: &Interp<'a>, c: u64, l: Option<u64>) -> &'a [crate::model::Marker] {
    match l {
        Some(l) => it.cx.layer_of(c, l).map(|(_, layer)| layer.markers.as_slice()).unwrap_or(&[]),
        None => it.cx.comp_of(c).map(|comp| comp.markers.as_slice()).unwrap_or(&[]),
    }
}

fn marker_key(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let V::Host(h) = this else { return Ok(V::Undef) };
    let H::Marker(c, l) = &**h else { return Ok(V::Undef) };
    let ms = markers(it, *c, *l);
    let i = match arg(&a, 0) {
        V::Str(name) => ms.iter().position(|m| *m.comment == *name).ok_or_else(|| throw("Error", format!("no marker named `{name}`")))?,
        v => {
            let n = it.to_number(&v)?;
            if n < 1.0 || n as usize > ms.len() {
                return Err(throw("RangeError", format!("marker index {} is out of range (1 to {})", num_to_string(n), ms.len())));
            }
            n as usize - 1
        }
    };
    Ok(hv(H::MarkerKey(*c, *l, i)))
}

fn marker_nearest(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let V::Host(h) = this else { return Ok(V::Undef) };
    let H::Marker(c, l) = &**h else { return Ok(V::Undef) };
    let t = match arg(&a, 0) {
        V::Undef => it.cx.time,
        v => it.to_number(&v)?,
    };
    let ms = markers(it, *c, *l);
    let best = ms.iter().enumerate().min_by(|x, y| (x.1.time - t).abs().total_cmp(&(y.1.time - t).abs())).map(|(i, _)| i);
    match best {
        Some(i) => Ok(hv(H::MarkerKey(*c, *l, i))),
        None => Err(throw("Error", "there are no markers")),
    }
}

fn comp_member(it: &mut Interp, c: u64, key: &str, h: &Rc<H>) -> R {
    let comp = it.cx.comp_of(c).ok_or_else(missing)?;
    let name = it.cx.project.item(c).map(|i| i.name.clone()).unwrap_or_default();
    Ok(match key {
        "name" => s(&name),
        "width" => V::Num(comp.width as f64),
        "height" => V::Num(comp.height as f64),
        "duration" => V::Num(comp.duration),
        "frameDuration" => V::Num(comp.frame_duration()),
        "frameRate" => V::Num(comp.frame_rate),
        "numLayers" => V::Num(comp.layers.len() as f64),
        "displayStartTime" => V::Num(comp.display_start),
        "pixelAspect" => V::Num(comp.pixel_aspect),
        "shutterAngle" => V::Num(comp.shutter_angle),
        "shutterPhase" => V::Num(comp.shutter_phase),
        "bgColor" => nums(&[comp.background[0] as f64, comp.background[1] as f64, comp.background[2] as f64, 1.0]),
        "id" => V::Num(c as f64),
        "layer" => native("layer", V::Host(h.clone()), comp_layer),
        "layerByComment" => native("layerByComment", V::Host(h.clone()), comp_layer_by_comment),
        "marker" => hv(H::Marker(c, None)),
        "activeCamera" => {
            let t = it.cx.time;
            match comp.layers.iter().find(|l| l.kind == LayerKind::Camera && l.switches.video && l.is_active_at(t)) {
                Some(l) => hv(H::Layer(c, l.id)),
                None => return Err(throw("Error", "this composition has no active camera")),
            }
        }
        _ => V::Undef,
    })
}

fn comp_layer(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let V::Host(h) = this else { return Ok(V::Undef) };
    let H::Comp(c) = &**h else { return Ok(V::Undef) };
    let comp = it.cx.comp_of(*c).ok_or_else(missing)?;
    let n = comp.layers.len();
    let by_index = |i: f64| -> R {
        if i < 1.0 || i.fract() != 0.0 || i as usize > n {
            return Err(throw("RangeError", format!("layer index {} is out of range (1 to {n})", num_to_string(i))));
        }
        Ok(hv(H::Layer(*c, comp.layers[i as usize - 1].id)))
    };
    match arg(&a, 0) {
        V::Str(name) => match comp.layers.iter().find(|l| *l.name == *name) {
            Some(l) => Ok(hv(H::Layer(*c, l.id))),
            None => Err(throw("Error", format!("there is no layer named `{name}` in this composition"))),
        },
        V::Host(lh) => match &*lh {
            H::Layer(_, lid) => {
                let base = comp.index_of(*lid).ok_or_else(missing)? as f64 + 1.0;
                let rel = match a.get(1) {
                    Some(v) => it.to_number(v)?,
                    None => 0.0,
                };
                by_index(base + rel)
            }
            _ => Err(type_err("layer() takes a name, an index or a layer")),
        },
        v => {
            let i = it.to_number(&v)?;
            by_index(i)
        }
    }
}

fn comp_layer_by_comment(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let V::Host(h) = this else { return Ok(V::Undef) };
    let H::Comp(c) = &**h else { return Ok(V::Undef) };
    let comp = it.cx.comp_of(*c).ok_or_else(missing)?;
    let want = it.to_string(&arg(&a, 0))?;
    match comp.layers.iter().find(|l| l.comment == want) {
        Some(l) => Ok(hv(H::Layer(*c, l.id))),
        None => Err(throw("Error", format!("there is no layer with the comment `{want}`"))),
    }
}

fn footage_member(it: &mut Interp, id: u64, key: &str) -> R {
    let item = it.cx.project.item(id).ok_or_else(missing)?;
    let (w, h) = item.dimensions().unwrap_or((0, 0));
    Ok(match key {
        "name" => s(&item.name),
        "width" => V::Num(w as f64),
        "height" => V::Num(h as f64),
        "duration" => V::Num(item.duration().unwrap_or(0.0)),
        "frameDuration" => V::Num(item.frame_rate().map(|f| 1.0 / f).unwrap_or(0.0)),
        "pixelAspect" => V::Num(match &item.kind {
            ItemKind::Footage(f) => f.pixel_aspect,
            ItemKind::Solid(sd) => sd.pixel_aspect,
            _ => 1.0,
        }),
        "sourceText" | "sourceData" | "dataValue" | "dataKeyCount" => return Err(type_err("data footage is not in formvfx yet")),
        _ => V::Undef,
    })
}

pub fn g_comp(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let name = it.to_string(&arg(&a, 0))?;
    match it.cx.project.items.iter().find(|i| i.name == name && matches!(i.kind, ItemKind::Comp(_))) {
        Some(i) => Ok(hv(H::Comp(i.id))),
        None => Err(throw("Error", format!("there is no composition named `{name}`"))),
    }
}

pub fn g_footage(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let name = it.to_string(&arg(&a, 0))?;
    match it.cx.project.items.iter().find(|i| i.name == name && matches!(i.kind, ItemKind::Footage(_) | ItemKind::Solid(_))) {
        Some(i) => Ok(hv(H::Footage(i.id))),
        None => Err(throw("Error", format!("there is no footage named `{name}`"))),
    }
}

fn alias(key: &str) -> &str {
    match key {
        "anchorPoint" => "anchor",
        "zRotation" => "rotation",
        "xRotation" => "rotationX",
        "yRotation" => "rotationY",
        "pointOfInterest" => "poi",
        "maskPath" => "path",
        "maskFeather" => "feather",
        "maskOpacity" => "opacity",
        "maskExpansion" => "expansion",
        other => other,
    }
}

fn node_v(c: u64, l: u64, n: &Node) -> V {
    match n {
        Node::Prop(p) => hv(H::Prop(c, l, p.uid)),
        Node::Group(g) => hv(H::Group(c, l, g.uid)),
    }
}

fn child_of(it: &mut Interp, c: u64, l: u64, g: &Group, key: &V) -> R {
    let node = match key {
        V::Num(n) => {
            if *n < 1.0 || n.fract() != 0.0 {
                return Err(throw("RangeError", format!("property index {} is out of range (1 to {})", num_to_string(*n), g.children.len())));
            }
            g.children.get(*n as usize - 1)
        }
        other => {
            let name = it.to_string(other)?;
            g.child(alias(&name))
        }
    };
    match node {
        Some(n) => Ok(node_v(c, l, n)),
        None => {
            let what = it.to_string(key)?;
            Err(throw("Error", format!("`{}` has no property `{what}`", g.name)))
        }
    }
}

const XFORM_KEYS: [&str; 10] = ["anchorPoint", "position", "scale", "rotation", "zRotation", "xRotation", "yRotation", "orientation", "opacity", "pointOfInterest"];

pub fn layer_member(it: &mut Interp, c: u64, layer: &Layer, key: &str) -> Result<Option<V>, Flow> {
    let comp = it.cx.comp_of(c).ok_or_else(missing)?;
    let lh = || hv(H::Layer(c, layer.id));
    let source = layer.source.and_then(|s| it.cx.project.item(s));
    let t = it.cx.time;
    let group = |id: &str| layer.props.sub(id).map(|g| hv(H::Group(c, layer.id, g.uid)));
    Ok(Some(match key {
        "name" => s(&layer.name),
        "index" => V::Num(comp.index_of(layer.id).map(|i| i + 1).unwrap_or(0) as f64),
        "inPoint" => V::Num(layer.in_point),
        "outPoint" => V::Num(layer.out_point),
        "startTime" => V::Num(layer.start_time),
        "width" | "height" => {
            let (w, h) = source.and_then(|i| i.dimensions()).unwrap_or((comp.width, comp.height));
            V::Num(if key == "width" { w } else { h } as f64)
        }
        "hasParent" => V::Bool(layer.parent.is_some()),
        "parent" => match layer.parent {
            Some(p) => hv(H::Layer(c, p)),
            None => return Err(type_err(format!("`{}` has no parent layer", layer.name))),
        },
        "hasVideo" => V::Bool(!matches!(layer.kind, LayerKind::Camera | LayerKind::Light | LayerKind::Null) && source.is_none_or(|i| !matches!(&i.kind, ItemKind::Footage(f) if !f.has_video))),
        "hasAudio" => V::Bool(layer.has_av() && source.is_some_and(|i| matches!(&i.kind, ItemKind::Footage(f) if f.has_audio) || matches!(i.kind, ItemKind::Comp(_)))),
        "active" => V::Bool(layer.switches.video && layer.is_active_at(t)),
        "enabled" => V::Bool(layer.switches.video),
        "audioActive" => V::Bool(layer.switches.audio && layer.is_active_at(t)),
        "source" => match source {
            Some(i) if matches!(i.kind, ItemKind::Comp(_)) => hv(H::Comp(i.id)),
            Some(i) => hv(H::Footage(i.id)),
            None => return Err(type_err(format!("`{}` has no source", layer.name))),
        },
        "transform" => group("transform").unwrap_or(V::Undef),
        k if XFORM_KEYS.contains(&k) => match layer.transform().and_then(|g| g.child(alias(k))) {
            Some(n) => node_v(c, layer.id, n),
            None => return Err(type_err(format!("`{}` has no `{k}`", layer.name))),
        },
        "effect" => native("effect", lh(), layer_effect),
        "mask" => native("mask", lh(), layer_mask),
        "content" => native("content", lh(), layer_content),
        "text" => match group("text") {
            Some(g) => g,
            None => return Err(type_err(format!("`{}` is not a text layer", layer.name))),
        },
        "marker" => hv(H::Marker(c, Some(layer.id))),
        "audioLevels" => match layer.props.sub("audio").and_then(|g| g.get("levels")) {
            Some(p) => hv(H::Prop(c, layer.id, p.uid)),
            None => return Err(type_err(format!("`{}` has no audio", layer.name))),
        },
        "cameraOption" => group("cameraOptions").unwrap_or(V::Undef),
        "lightOption" => group("lightOptions").unwrap_or(V::Undef),
        "toComp" => native("toComp", lh(), to_comp),
        "fromComp" => native("fromComp", lh(), from_comp),
        "toWorld" => native("toWorld", lh(), to_world),
        "fromWorld" => native("fromWorld", lh(), from_world),
        "toCompVec" | "toWorldVec" => native("toCompVec", lh(), to_comp_vec),
        "fromCompVec" | "fromWorldVec" => native("fromCompVec", lh(), from_comp_vec),
        "sourceRectAtTime" => native("sourceRectAtTime", lh(), source_rect),
        "sampleImage" => native("sampleImage", lh(), sample_image),
        "timeRemap" => match layer.props.get("timeRemap") {
            Some(p) => hv(H::Prop(c, layer.id, p.uid)),
            None => return Err(type_err("time remapping is not enabled on this layer")),
        },
        "numProperties" => V::Num(layer.props.children.len() as f64),
        "label" => V::Num(layer.label as f64),
        "id" => V::Num(layer.id as f64),
        "hasTrackMatte" => V::Bool(layer.track_matte.is_some()),
        "isTrackMatte" => V::Bool(comp.layers.iter().any(|o| o.track_matte.is_some_and(|m| m.layer == layer.id))),
        "trackMatteLayer" => match layer.track_matte {
            Some(m) => hv(H::Layer(c, m.layer)),
            None => V::Null,
        },
        _ => return Ok(None),
    }))
}

fn this_layer_of<'a>(it: &Interp<'a>, this: &V) -> Result<(u64, &'a Comp, &'a Layer), Flow> {
    let V::Host(h) = this else { return Err(missing()) };
    let H::Layer(c, l) = &**h else { return Err(missing()) };
    let (comp, layer) = it.cx.layer_of(*c, *l).ok_or_else(missing)?;
    Ok((*c, comp, layer))
}

fn layer_effect(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, _, layer) = this_layer_of(it, this)?;
    let g = layer.effects().ok_or_else(|| throw("Error", format!("`{}` has no effects", layer.name)))?;
    let key = arg(&a, 0);
    if let V::Str(name) = &key
        && let Some(n) = g.children.iter().find(|n| n.name() == &**name).or_else(|| g.children.iter().find(|n| matches!(n, Node::Group(eg) if matches!(&eg.kind, crate::model::GroupKind::Effect { effect } if crate::effects::find(effect).is_some_and(|sp| sp.name == &**name)))))
    {
        return Ok(node_v(c, layer.id, n));
    }
    child_of(it, c, layer.id, g, &key).map_err(|_| throw("Error", format!("`{}` has no effect `{}`", layer.name, plain_string(&key))))
}

fn layer_mask(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, _, layer) = this_layer_of(it, this)?;
    let g = layer.masks().ok_or_else(|| throw("Error", format!("`{}` has no masks", layer.name)))?;
    let key = arg(&a, 0);
    child_of(it, c, layer.id, g, &key).map_err(|_| throw("Error", format!("`{}` has no mask `{}`", layer.name, plain_string(&key))))
}

fn layer_content(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, _, layer) = this_layer_of(it, this)?;
    let g = layer.contents().ok_or_else(|| throw("Error", format!("`{}` is not a shape layer", layer.name)))?;
    let key = arg(&a, 0);
    child_of(it, c, layer.id, g, &key)
}

fn point_arg(it: &mut Interp, a: &[V]) -> Result<(Vec<f64>, f64), Flow> {
    let p = components(it, &arg(a, 0))?;
    let t = match arg(a, 1) {
        V::Undef => it.cx.time,
        v => it.to_number(&v)?,
    };
    Ok((p, t))
}

fn out_dims(p: &[f64], r: [f64; 3]) -> V {
    if p.len() >= 3 { nums(&r) } else { nums(&r[..2]) }
}

fn world(it: &Interp, c: u64, layer: &Layer, t: f64) -> Option<Mat4> {
    let ev = it.cx.eval_at(c, t)?;
    Some(ev.world_matrix(layer))
}

fn p3(p: &[f64]) -> [f64; 3] {
    [p.first().copied().unwrap_or(0.0), p.get(1).copied().unwrap_or(0.0), p.get(2).copied().unwrap_or(0.0)]
}

fn to_world(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, _, layer) = this_layer_of(it, this)?;
    let (p, t) = point_arg(it, &a)?;
    let m = world(it, c, layer, t).ok_or_else(missing)?;
    Ok(out_dims(&p, m.apply(p3(&p))))
}

fn from_world(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, _, layer) = this_layer_of(it, this)?;
    let (p, t) = point_arg(it, &a)?;
    let m = world(it, c, layer, t).and_then(|m| m.inverse()).ok_or_else(|| type_err("the layer cannot be inverted (zero scale)"))?;
    Ok(out_dims(&p, m.apply(p3(&p))))
}

fn to_comp(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, comp, layer) = this_layer_of(it, this)?;
    let (p, t) = point_arg(it, &a)?;
    let ev = it.cx.eval_at(c, t).ok_or_else(missing)?;
    let w = ev.world_matrix(layer).apply(p3(&p));
    if layer.is_3d() {
        let cam = ev.camera();
        let (cw, ch) = (comp.width as f64, comp.height as f64);
        if let Some(s2) = cam.project(cw, ch, w) {
            return Ok(out_dims(&p, [s2[0], s2[1], 0.0]));
        }
    }
    Ok(out_dims(&p, w))
}

fn from_comp(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, comp, layer) = this_layer_of(it, this)?;
    let (p, t) = point_arg(it, &a)?;
    let ev = it.cx.eval_at(c, t).ok_or_else(missing)?;
    if layer.is_3d() {
        let cam = ev.camera();
        let m: Mat3 = ev.layer_to_screen(layer, &cam);
        let inv = m.inverse().ok_or_else(|| type_err("the layer is edge-on to the camera"))?;
        let r = inv.apply(p[0], p.get(1).copied().unwrap_or(0.0));
        let _ = comp;
        return Ok(out_dims(&p, [r[0], r[1], 0.0]));
    }
    let inv = ev.world_matrix(layer).inverse().ok_or_else(|| type_err("the layer cannot be inverted (zero scale)"))?;
    Ok(out_dims(&p, inv.apply(p3(&p))))
}

fn to_comp_vec(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, _, layer) = this_layer_of(it, this)?;
    let (p, t) = point_arg(it, &a)?;
    let m = world(it, c, layer, t).ok_or_else(missing)?;
    let o = m.apply([0.0; 3]);
    let q = m.apply(p3(&p));
    Ok(out_dims(&p, [q[0] - o[0], q[1] - o[1], q[2] - o[2]]))
}

fn from_comp_vec(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, _, layer) = this_layer_of(it, this)?;
    let (p, t) = point_arg(it, &a)?;
    let m = world(it, c, layer, t).and_then(|m| m.inverse()).ok_or_else(|| type_err("the layer cannot be inverted (zero scale)"))?;
    let o = m.apply([0.0; 3]);
    let q = m.apply(p3(&p));
    Ok(out_dims(&p, [q[0] - o[0], q[1] - o[1], q[2] - o[2]]))
}

fn source_rect(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, comp, layer) = this_layer_of(it, this)?;
    let t = match arg(&a, 0) {
        V::Undef => it.cx.time,
        v => it.to_number(&v)?,
    };
    let rect = |l: f64, tp: f64, w: f64, h: f64| obj(vec![("left", V::Num(l)), ("top", V::Num(tp)), ("width", V::Num(w)), ("height", V::Num(h))]);
    let ev = it.cx.eval_at(c, t).ok_or_else(missing)?;
    let bounds = match layer.kind {
        LayerKind::Text => {
            let doc = layer.text_prop().map(|p| ev.value(layer, p)).and_then(|v| v.as_text().cloned());
            doc.and_then(|d| crate::render::vector::ops_bounds(&crate::render::text::text_ops(&d)))
        }
        LayerKind::Shape => crate::render::vector::ops_bounds(&crate::render::shapes::shape_ops(&ev, layer)),
        _ => {
            let (w, h) = layer.source.and_then(|s| it.cx.project.item(s)).and_then(|i| i.dimensions()).unwrap_or((comp.width, comp.height));
            Some([0.0, 0.0, w as f64, h as f64])
        }
    };
    Ok(match bounds {
        Some(b) => rect(b[0], b[1], b[2] - b[0], b[3] - b[1]),
        None => rect(0.0, 0.0, 0.0, 0.0),
    })
}

thread_local! {
    static SAMPLING: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn sample_image(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, comp, layer) = this_layer_of(it, this)?;
    let p = components(it, &arg(&a, 0))?;
    let rad = match arg(&a, 1) {
        V::Undef => vec![0.5, 0.5],
        v => components(it, &v)?,
    };
    let post = !matches!(arg(&a, 2), V::Bool(false));
    let t = match arg(&a, 3) {
        V::Undef => it.cx.time,
        v => it.to_number(&v)?,
    };
    if SAMPLING.with(|d| d.get()) >= 2 {
        return Err(type_err("sampleImage refers back to itself through another expression"));
    }
    let media = crate::render::media::shared().ok_or_else(|| type_err("sampleImage has no footage cache to render from"))?;
    let rd = crate::render::comp::Renderer::new(it.cx.project, &media, crate::render::comp::RenderOpts::default());
    let ev = Eval::new(it.cx.project, c, comp, t);
    let at = [p.first().copied().unwrap_or(0.0), p.get(1).copied().unwrap_or(0.0)];
    let radius = [rad.first().copied().unwrap_or(0.5), rad.get(1).or(rad.first()).copied().unwrap_or(0.5)];
    SAMPLING.with(|d| d.set(d.get() + 1));
    let px = rd.sample_layer(&ev, layer, at, radius, post);
    SAMPLING.with(|d| d.set(d.get() - 1));
    Ok(nums(&px.unwrap_or([0.0; 4])))
}

fn group_member(it: &mut Interp, c: u64, l: u64, u: u64, key: &str, h: &Rc<H>) -> R {
    let (_, layer, g) = it.cx.group_of(c, l, u).ok_or_else(missing)?;
    Ok(match key {
        "name" => s(&g.name),
        "numProperties" => V::Num(g.children.len() as f64),
        "active" | "enabled" => V::Bool(g.enabled),
        "propertyIndex" => V::Num(index_in_parent(layer, u) as f64),
        "propertyGroup" => native("propertyGroup", V::Host(h.clone()), prop_group),
        "animator" if g.sub("animators").is_some() => native("animator", V::Host(h.clone()), sub_call_animators),
        "selector" if g.sub("selectors").is_some() => native("selector", V::Host(h.clone()), sub_call_selectors),
        "property" if g.sub("properties").is_some() => match g.sub("properties") {
            Some(pg) => hv(H::Group(c, l, pg.uid)),
            None => V::Undef,
        },
        "param" | "property" | "content" => native("param", V::Host(h.clone()), group_call),
        "mask" if g.id == "masks" => native("mask", V::Host(h.clone()), group_call),
        "effect" if g.id == "effects" => native("effect", V::Host(h.clone()), group_call),
        _ => match g.child(alias(key)) {
            Some(n) => node_v(c, l, n),
            None => V::Undef,
        },
    })
}

fn index_in_parent(layer: &Layer, uid: u64) -> usize {
    layer.props.parent_of(uid).and_then(|p| p.children.iter().position(|n| n.uid() == uid)).map(|i| i + 1).unwrap_or(0)
}

fn sub_call(it: &mut Interp, this: &V, a: Vec<V>, id: &str) -> R {
    let V::Host(h) = this else { return Ok(V::Undef) };
    let H::Group(c, l, u) = &**h else { return Ok(V::Undef) };
    let (c, l, u) = (*c, *l, *u);
    let sub = {
        let (_, _, g) = it.cx.group_of(c, l, u).ok_or_else(missing)?;
        g.sub(id).map(|s| s.uid)
    };
    let Some(su) = sub else { return Ok(V::Undef) };
    let (_, _, sg) = it.cx.group_of(c, l, su).ok_or_else(missing)?;
    let sg = sg.clone();
    child_of(it, c, l, &sg, &arg(&a, 0))
}

fn sub_call_animators(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    sub_call(it, this, a, "animators")
}

fn sub_call_selectors(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    sub_call(it, this, a, "selectors")
}

fn group_call(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let V::Host(h) = this else { return Ok(V::Undef) };
    call(it, h, a)
}

pub fn call(it: &mut Interp, h: &Rc<H>, a: Vec<V>) -> R {
    let key = arg(&a, 0);
    match &**h {
        H::Layer(c, l) => {
            let (_, layer) = it.cx.layer_of(*c, *l).ok_or_else(missing)?;
            if let V::Str(name) = &key {
                let lower = name.to_ascii_lowercase();
                let id = match lower.as_str() {
                    "effects" => Some("effects"),
                    "masks" => Some("masks"),
                    "transform" => Some("transform"),
                    "contents" => Some("contents"),
                    "text" => Some("text"),
                    "audio" => Some("audio"),
                    _ => None,
                };
                if let Some(g) = id.and_then(|i| layer.props.sub(i)) {
                    return Ok(hv(H::Group(*c, *l, g.uid)));
                }
            }
            child_of(it, *c, *l, &layer.props, &key)
        }
        H::Group(c, l, u) => {
            let (_, _, g) = it.cx.group_of(*c, *l, *u).ok_or_else(missing)?;
            child_of(it, *c, *l, g, &key)
        }
        _ => Err(type_err("this object is not a function")),
    }
}

fn prop_group(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let V::Host(h) = this else { return Ok(V::Undef) };
    let (c, l, mut uid) = match &**h {
        H::Prop(c, l, u) | H::Group(c, l, u) => (*c, *l, *u),
        _ => return Ok(V::Undef),
    };
    let (_, layer) = it.cx.layer_of(c, l).ok_or_else(missing)?;
    let n = match arg(&a, 0) {
        V::Undef => 1,
        v => it.to_number(&v)?.max(1.0) as usize,
    };
    for _ in 0..n {
        match layer.props.parent_of(uid) {
            Some(p) if p.uid != layer.props.uid => uid = p.uid,
            Some(_) => return Ok(hv(H::Layer(c, l))),
            None => return Ok(hv(H::Layer(c, l))),
        }
    }
    Ok(hv(H::Group(c, l, uid)))
}

fn this_prop<'a>(it: &Interp<'a>, this: &V) -> Result<(u64, &'a Layer, &'a Prop), Flow> {
    let V::Host(h) = this else { return Err(missing()) };
    let H::Prop(c, l, u) = &**h else { return Err(missing()) };
    let (_, layer, prop) = it.cx.prop_of(*c, *l, *u).ok_or_else(missing)?;
    Ok((*c, layer, prop))
}

fn value_v<'a>(it: &Interp<'a>, c: u64, layer: &'a Layer, prop: &'a Prop, t: f64) -> V {
    let v = it.cx.eval_prop(c, layer, prop, t);
    to_v(c, layer, prop, &v)
}

fn time_arg(it: &mut Interp, a: &[V], i: usize) -> Result<f64, Flow> {
    match arg(a, i) {
        V::Undef => Ok(it.cx.time),
        v => it.to_number(&v),
    }
}

fn opt_num(it: &mut Interp, a: &[V], i: usize, d: f64) -> Result<f64, Flow> {
    match arg(a, i) {
        V::Undef => Ok(d),
        v => it.to_number(&v),
    }
}

fn p_style_at(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, layer, prop) = this_prop(it, this)?;
    let i = it.to_number(&arg(&a, 0))?.max(0.0) as usize;
    let t = time_arg(it, &a, 1)?;
    match it.cx.eval_prop(c, layer, prop, t) {
        Value::Text(d) => Ok(super::textstyle::style_at_v(&d, i)),
        _ => Ok(V::Undef),
    }
}

fn p_value_at_time(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, layer, prop) = this_prop(it, this)?;
    let t = time_arg(it, &a, 0)?;
    Ok(value_v(it, c, layer, prop, t))
}

fn velocity_at<'a>(it: &mut Interp<'a>, c: u64, layer: &'a Layer, prop: &'a Prop, t: f64) -> Result<Vec<f64>, Flow> {
    let h = 1e-3;
    let a = value_v(it, c, layer, prop, t - h);
    let b = value_v(it, c, layer, prop, t + h);
    let x = components(it, &a)?;
    let y = components(it, &b)?;
    Ok(x.iter().zip(&y).map(|(p, q)| (q - p) / (2.0 * h)).collect())
}

fn vel_v(v: Vec<f64>) -> V {
    if v.len() == 1 { V::Num(v[0]) } else { nums(&v) }
}

fn p_velocity_at_time(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, layer, prop) = this_prop(it, this)?;
    let t = time_arg(it, &a, 0)?;
    Ok(vel_v(velocity_at(it, c, layer, prop, t)?))
}

fn p_speed_at_time(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, layer, prop) = this_prop(it, this)?;
    let t = time_arg(it, &a, 0)?;
    let v = velocity_at(it, c, layer, prop, t)?;
    Ok(V::Num(v.iter().map(|x| x * x).sum::<f64>().sqrt()))
}

fn seed_for(layer: &Layer, prop: &Prop, d: usize) -> u64 {
    noise::hash(&[layer.id, prop.uid, d as u64])
}

fn p_wiggle(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, layer, prop) = this_prop(it, this)?;
    let freq = opt_num(it, &a, 0, 1.0)?;
    let amp_v = arg(&a, 1);
    let amp = components(it, &amp_v)?;
    let oct = opt_num(it, &a, 2, 1.0)?;
    let mult = opt_num(it, &a, 3, 0.5)?;
    let t = time_arg(it, &a, 4)?;
    let base = value_v(it, c, layer, prop, t);
    let is_vec = it.vector(&base)?.is_some();
    let x = components(it, &base)?;
    let out: Vec<f64> = x
        .iter()
        .enumerate()
        .map(|(d, v)| {
            let am = amp.get(d).or(amp.first()).copied().unwrap_or(0.0);
            v + am * 1.5 * noise::wiggle1(t, freq, oct, mult, seed_for(layer, prop, d))
        })
        .collect();
    Ok(if is_vec { nums(&out) } else { V::Num(out[0]) })
}

fn p_temporal_wiggle(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, layer, prop) = this_prop(it, this)?;
    let freq = opt_num(it, &a, 0, 1.0)?;
    let amp = opt_num(it, &a, 1, 0.0)?;
    let oct = opt_num(it, &a, 2, 1.0)?;
    let mult = opt_num(it, &a, 3, 0.5)?;
    let t = time_arg(it, &a, 4)?;
    let dt = amp * 1.5 * noise::wiggle1(t, freq, oct, mult, seed_for(layer, prop, 99));
    Ok(value_v(it, c, layer, prop, t + dt))
}

fn p_smooth(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, layer, prop) = this_prop(it, this)?;
    let width = opt_num(it, &a, 0, 0.2)?;
    let samples = opt_num(it, &a, 1, 5.0)?.clamp(1.0, 64.0) as usize;
    let t = time_arg(it, &a, 2)?;
    if samples <= 1 {
        return Ok(value_v(it, c, layer, prop, t));
    }
    let mut acc: Vec<f64> = vec![];
    let mut is_vec = false;
    for i in 0..samples {
        let tt = t + (i as f64 / (samples - 1) as f64 - 0.5) * width;
        let v = value_v(it, c, layer, prop, tt);
        is_vec |= it.vector(&v)?.is_some();
        let x = components(it, &v)?;
        if acc.is_empty() {
            acc = vec![0.0; x.len()];
        }
        for (k, val) in x.iter().enumerate().take(acc.len()) {
            acc[k] += val;
        }
    }
    let out: Vec<f64> = acc.iter().map(|v| v / samples as f64).collect();
    Ok(if is_vec { nums(&out) } else { V::Num(out.first().copied().unwrap_or(0.0)) })
}

fn key_times(layer: &Layer, prop: &Prop) -> Vec<f64> {
    prop.keys.iter().map(|k| layer.comp_time(k.time)).collect()
}

fn loop_type(it: &mut Interp, a: &[V]) -> Result<String, Flow> {
    Ok(match arg(a, 0) {
        V::Undef => "cycle".into(),
        v => it.to_string(&v)?.to_ascii_lowercase(),
    })
}

fn add_scaled(it: &mut Interp, base: &V, d: &V, k: f64) -> R {
    let scaled = it.binary(super::ast::BinOp::Mul, d, &V::Num(k))?;
    it.binary(super::ast::BinOp::Add, base, &scaled)
}

fn loop_impl(it: &mut Interp, this: &V, a: &[V], out: bool, by_duration: bool) -> R {
    let (c, layer, prop) = this_prop(it, this)?;
    let kind = loop_type(it, a)?;
    let amount = opt_num(it, a, 1, 0.0)?;
    let t = it.cx.time;
    let times = key_times(layer, prop);
    let n = times.len();
    if n < 2 {
        return Ok(value_v(it, c, layer, prop, t));
    }
    if !matches!(kind.as_str(), "cycle" | "pingpong" | "offset" | "continue") {
        return Err(type_err(format!("`{kind}` is not a loop type (cycle, pingpong, offset or continue)")));
    }
    if out {
        let t_end = times[n - 1];
        if t <= t_end {
            return Ok(value_v(it, c, layer, prop, t));
        }
        let t_start = if by_duration {
            if amount <= 0.0 { times[0] } else { (t_end - amount).max(times[0]) }
        } else if amount <= 0.0 {
            times[0]
        } else {
            times[(n - 1).saturating_sub(amount as usize)]
        };
        let span = t_end - t_start;
        if kind == "continue" {
            let v_end = value_v(it, c, layer, prop, t_end);
            let vel = velocity_at(it, c, layer, prop, t_end - 2e-3)?;
            return add_scaled(it, &v_end, &vel_v(vel), t - t_end);
        }
        if span <= 1e-9 {
            return Ok(value_v(it, c, layer, prop, t_end));
        }
        let d = t - t_end;
        let cycles = (d / span).floor();
        let phase = d - cycles * span;
        match kind.as_str() {
            "cycle" => Ok(value_v(it, c, layer, prop, t_start + phase)),
            "pingpong" => {
                let tt = if cycles as i64 % 2 == 0 { t_end - phase } else { t_start + phase };
                Ok(value_v(it, c, layer, prop, tt))
            }
            _ => {
                let v0 = value_v(it, c, layer, prop, t_start);
                let v1 = value_v(it, c, layer, prop, t_end);
                let delta = it.binary(super::ast::BinOp::Sub, &v1, &v0)?;
                let base = value_v(it, c, layer, prop, t_start + phase);
                add_scaled(it, &base, &delta, cycles + 1.0)
            }
        }
    } else {
        let t_first = times[0];
        if t >= t_first {
            return Ok(value_v(it, c, layer, prop, t));
        }
        let t_seg = if by_duration {
            if amount <= 0.0 { times[n - 1] } else { (t_first + amount).min(times[n - 1]) }
        } else if amount <= 0.0 {
            times[n - 1]
        } else {
            times[(amount as usize).min(n - 1)]
        };
        let span = t_seg - t_first;
        if kind == "continue" {
            let v_first = value_v(it, c, layer, prop, t_first);
            let vel = velocity_at(it, c, layer, prop, t_first + 2e-3)?;
            return add_scaled(it, &v_first, &vel_v(vel), t - t_first);
        }
        if span <= 1e-9 {
            return Ok(value_v(it, c, layer, prop, t_first));
        }
        let d = t_first - t;
        let cycles = (d / span).floor();
        let phase = d - cycles * span;
        match kind.as_str() {
            "cycle" => Ok(value_v(it, c, layer, prop, t_seg - phase)),
            "pingpong" => {
                let tt = if cycles as i64 % 2 == 0 { t_first + phase } else { t_seg - phase };
                Ok(value_v(it, c, layer, prop, tt))
            }
            _ => {
                let v0 = value_v(it, c, layer, prop, t_first);
                let v1 = value_v(it, c, layer, prop, t_seg);
                let delta = it.binary(super::ast::BinOp::Sub, &v1, &v0)?;
                let base = value_v(it, c, layer, prop, t_seg - phase);
                add_scaled(it, &base, &delta, -(cycles + 1.0))
            }
        }
    }
}

fn p_loop_out(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    loop_impl(it, this, &a, true, false)
}

fn p_loop_in(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    loop_impl(it, this, &a, false, false)
}

fn p_loop_out_duration(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    loop_impl(it, this, &a, true, true)
}

fn p_loop_in_duration(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    loop_impl(it, this, &a, false, true)
}

fn p_key(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, layer, prop) = this_prop(it, this)?;
    let n = prop.keys.len();
    let i = it.to_number(&arg(&a, 0))?;
    if i < 1.0 || i.fract() != 0.0 || i as usize > n {
        return Err(throw("RangeError", format!("key index {} is out of range (1 to {n})", num_to_string(i))));
    }
    Ok(hv(H::Key(c, layer.id, prop.uid, i as usize - 1)))
}

fn p_nearest_key(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (c, layer, prop) = this_prop(it, this)?;
    let t = time_arg(it, &a, 0)?;
    let times = key_times(layer, prop);
    let best = times.iter().enumerate().min_by(|x, y| (x.1 - t).abs().total_cmp(&(y.1 - t).abs())).map(|(i, _)| i);
    match best {
        Some(i) => Ok(hv(H::Key(c, layer.id, prop.uid, i))),
        None => Err(throw("Error", format!("`{}` has no keyframes", prop.name))),
    }
}

fn path_of(it: &mut Interp, this: &V, a: &[V]) -> Result<BezPath, Flow> {
    if let V::Path(p) = this {
        return Ok((**p).clone());
    }
    let (c, layer, prop) = this_prop(it, this)?;
    let t = time_arg(it, a, 0)?;
    match value_v(it, c, layer, prop, t) {
        V::Path(p) => Ok((*p).clone()),
        _ => Err(type_err(format!("`{}` is not a path", prop.name))),
    }
}

fn pts(v: &[[f64; 2]]) -> V {
    arr(v.iter().map(|p| nums(p)).collect())
}

fn p_points(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    Ok(pts(&path_of(it, this, &a)?.vertices))
}

fn p_in_tangents(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    Ok(pts(&path_of(it, this, &a)?.normalized().ins))
}

fn p_out_tangents(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    Ok(pts(&path_of(it, this, &a)?.normalized().outs))
}

fn p_is_closed(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    Ok(V::Bool(path_of(it, this, &a)?.closed))
}

pub fn path_method(key: &str) -> Option<(&'static str, NativeImpl)> {
    Some(match key {
        "points" => ("points", p_points),
        "inTangents" => ("inTangents", p_in_tangents),
        "outTangents" => ("outTangents", p_out_tangents),
        "isClosed" => ("isClosed", p_is_closed),
        _ => return None,
    })
}

fn point_list(it: &mut Interp, v: &V) -> Result<Vec<[f64; 2]>, Flow> {
    if matches!(v, V::Undef | V::Null) {
        return Ok(vec![]);
    }
    let items = it.iterate(v)?;
    let mut out = Vec::with_capacity(items.len());
    for p in &items {
        let x = components(it, p)?;
        out.push([x.first().copied().unwrap_or(0.0), x.get(1).copied().unwrap_or(0.0)]);
    }
    Ok(out)
}

pub fn g_create_path(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let vertices = point_list(it, &arg(&a, 0))?;
    if vertices.is_empty() {
        return Ok(V::Path(Rc::new(BezPath::default())));
    }
    let ins = point_list(it, &arg(&a, 1))?;
    let outs = point_list(it, &arg(&a, 2))?;
    let closed = match arg(&a, 3) {
        V::Undef => true,
        v => truthy(&v),
    };
    let n = vertices.len();
    let fit = |mut v: Vec<[f64; 2]>| {
        v.resize(n, [0.0; 2]);
        v
    };
    Ok(V::Path(Rc::new(BezPath { vertices, ins: fit(ins), outs: fit(outs), closed })))
}

fn prop_member(it: &mut Interp, c: u64, l: u64, u: u64, key: &str, h: &Rc<H>) -> R {
    let (_, layer, prop) = it.cx.prop_of(c, l, u).ok_or_else(missing)?;
    let m = |name: &'static str, f: NativeImpl| native(name, V::Host(h.clone()), f);
    Ok(match key {
        "value" => primitive(it, h)?,
        "valueAtTime" => m("valueAtTime", p_value_at_time),
        "velocityAtTime" => m("velocityAtTime", p_velocity_at_time),
        "speedAtTime" => m("speedAtTime", p_speed_at_time),
        "velocity" => {
            let t = it.cx.time;
            vel_v(velocity_at(it, c, layer, prop, t)?)
        }
        "speed" => {
            let t = it.cx.time;
            let v = velocity_at(it, c, layer, prop, t)?;
            V::Num(v.iter().map(|x| x * x).sum::<f64>().sqrt())
        }
        "wiggle" => m("wiggle", p_wiggle),
        "temporalWiggle" => m("temporalWiggle", p_temporal_wiggle),
        "smooth" => m("smooth", p_smooth),
        "loopOut" => m("loopOut", p_loop_out),
        "loopIn" => m("loopIn", p_loop_in),
        "loopOutDuration" => m("loopOutDuration", p_loop_out_duration),
        "loopInDuration" => m("loopInDuration", p_loop_in_duration),
        "key" => m("key", p_key),
        "nearestKey" => m("nearestKey", p_nearest_key),
        "numKeys" => V::Num(prop.keys.len() as f64),
        "propertyGroup" => m("propertyGroup", prop_group),
        "points" => m("points", p_points),
        "inTangents" => m("inTangents", p_in_tangents),
        "outTangents" => m("outTangents", p_out_tangents),
        "isClosed" => m("isClosed", p_is_closed),
        "createPath" => native("createPath", V::Undef, g_create_path),
        "style" if matches!(prop.value, Value::Text(_)) => match it.cx.eval_prop(c, layer, prop, it.cx.time) {
            Value::Text(d) => super::textstyle::style_v(&d),
            _ => V::Undef,
        },
        "getStyleAt" if matches!(prop.value, Value::Text(_)) => m("getStyleAt", p_style_at),
        "name" => s(&prop.name),
        "propertyIndex" => V::Num(index_in_parent(layer, u) as f64),
        "active" | "enabled" => V::Bool(true),
        _ => {
            let p = primitive(it, h)?;
            it.get_member(&p, key)?
        }
    })
}
