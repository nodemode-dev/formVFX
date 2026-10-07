use serde::{Deserialize, Serialize};

use super::key::{self, Key};
use super::path::BezPath;
use super::value::{Gradient, TextDoc, Value};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Ui {
    Slider { min: f64, max: f64, slider_min: f64, slider_max: f64, decimals: u8 },
    Percent,
    Angle,
    Pixels,
    Point,
    Point3,
    Color,
    Checkbox,
    Popup { options: Vec<String> },
    Mask,
    Layer,
    Text,
    Path,
    Gradient,
    Plain,
    Hidden,
}

impl Ui {
    pub fn slider(min: f64, max: f64, smin: f64, smax: f64, decimals: u8) -> Ui {
        Ui::Slider { min, max, slider_min: smin, slider_max: smax, decimals }
    }

    pub fn popup(opts: &[&str]) -> Ui {
        Ui::Popup { options: opts.iter().map(|s| s.to_string()).collect() }
    }

    pub fn options(&self) -> &[String] {
        match self {
            Ui::Popup { options } => options,
            _ => &[],
        }
    }

    pub fn clamp(&self, v: Value) -> Value {
        match (self, v) {
            (Ui::Slider { min, max, .. }, Value::Num(x)) => Value::Num(x.clamp(*min, *max)),
            (Ui::Popup { options }, Value::Enum(e)) if !options.is_empty() => Value::Enum(e.min(options.len() as u32 - 1)),
            (_, v) => v,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Expr {
    pub text: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prop {
    pub uid: u64,
    #[serde(rename = "match")]
    pub id: String,
    pub name: String,
    pub value: Value,
    pub default: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<Key>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<Expr>,
    pub ui: Ui,
    #[serde(default)]
    pub spatial: bool,
    #[serde(default)]
    pub static_only: bool,
    #[serde(default)]
    pub hold_only: bool,
    #[serde(default)]
    pub three_d_only: bool,
    #[serde(default)]
    pub two_d_only: bool,
}

impl Prop {
    pub fn new(uid: u64, id: &str, name: &str, value: Value, ui: Ui) -> Prop {
        Prop {
            uid,
            id: id.to_string(),
            name: name.to_string(),
            default: value.clone(),
            value,
            keys: vec![],
            expr: None,
            ui,
            spatial: false,
            static_only: false,
            hold_only: false,
            three_d_only: false,
            two_d_only: false,
        }
    }

    pub fn spatial(mut self) -> Prop {
        self.spatial = true;
        self
    }

    pub fn only_3d(mut self) -> Prop {
        self.three_d_only = true;
        self
    }

    pub fn only_2d(mut self) -> Prop {
        self.two_d_only = true;
        self
    }

    pub fn fixed(mut self) -> Prop {
        self.static_only = true;
        self
    }

    pub fn is_animated(&self) -> bool {
        !self.keys.is_empty()
    }

    pub fn at(&self, t: f64) -> Value {
        if self.keys.is_empty() {
            return self.value.clone();
        }
        key::value_at(&self.keys, t, self.spatial).unwrap_or_else(|| self.value.clone())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MaskMode {
    None,
    #[default]
    Add,
    Subtract,
    Intersect,
    Lighten,
    Darken,
    Difference,
}

impl MaskMode {
    pub const ALL: [MaskMode; 7] = [MaskMode::None, MaskMode::Add, MaskMode::Subtract, MaskMode::Intersect, MaskMode::Lighten, MaskMode::Darken, MaskMode::Difference];

    pub fn label(&self) -> &'static str {
        match self {
            MaskMode::None => "None",
            MaskMode::Add => "Add",
            MaskMode::Subtract => "Subtract",
            MaskMode::Intersect => "Intersect",
            MaskMode::Lighten => "Lighten",
            MaskMode::Darken => "Darken",
            MaskMode::Difference => "Difference",
        }
    }

    pub fn parse(s: &str) -> Option<MaskMode> {
        MaskMode::ALL.iter().copied().find(|m| m.label().eq_ignore_ascii_case(s))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GroupKind {
    Plain,
    Indexed,
    Mask { mode: MaskMode, inverted: bool, color: [u8; 3], locked: bool },
    Effect { effect: String },
    Tracker,
    Essential,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub uid: u64,
    #[serde(rename = "match")]
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub kind: GroupKind,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Node {
    Prop(Prop),
    Group(Group),
}

impl Node {
    pub fn uid(&self) -> u64 {
        match self {
            Node::Prop(p) => p.uid,
            Node::Group(g) => g.uid,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Node::Prop(p) => &p.id,
            Node::Group(g) => &g.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Node::Prop(p) => &p.name,
            Node::Group(g) => &g.name,
        }
    }
}

pub enum NodeRef<'a> {
    Prop(&'a Prop),
    Group(&'a Group),
}

impl Group {
    pub fn new(uid: u64, id: &str, name: &str, kind: GroupKind) -> Group {
        Group { uid, id: id.to_string(), name: name.to_string(), enabled: true, kind, children: vec![] }
    }

    pub fn with(mut self, children: Vec<Node>) -> Group {
        self.children = children;
        self
    }

    pub fn get(&self, id: &str) -> Option<&Prop> {
        self.children.iter().find_map(|c| match c {
            Node::Prop(p) if p.id == id => Some(p),
            _ => None,
        })
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Prop> {
        self.children.iter_mut().find_map(|c| match c {
            Node::Prop(p) if p.id == id => Some(p),
            _ => None,
        })
    }

    pub fn sub(&self, id: &str) -> Option<&Group> {
        self.children.iter().find_map(|c| match c {
            Node::Group(g) if g.id == id => Some(g),
            _ => None,
        })
    }

    pub fn sub_mut(&mut self, id: &str) -> Option<&mut Group> {
        self.children.iter_mut().find_map(|c| match c {
            Node::Group(g) if g.id == id => Some(g),
            _ => None,
        })
    }

    pub fn groups(&self) -> impl Iterator<Item = &Group> {
        self.children.iter().filter_map(|c| match c {
            Node::Group(g) => Some(g),
            _ => None,
        })
    }

    pub fn groups_mut(&mut self) -> impl Iterator<Item = &mut Group> {
        self.children.iter_mut().filter_map(|c| match c {
            Node::Group(g) => Some(g),
            _ => None,
        })
    }

    pub fn group(&self, path: &str) -> Option<&Group> {
        let mut g = self;
        for part in path.split('/').filter(|s| !s.is_empty()) {
            g = g.sub(part)?;
        }
        Some(g)
    }

    pub fn group_mut(&mut self, path: &str) -> Option<&mut Group> {
        let mut g = self;
        for part in path.split('/').filter(|s| !s.is_empty()) {
            g = g.sub_mut(part)?;
        }
        Some(g)
    }

    pub fn prop(&self, path: &str) -> Option<&Prop> {
        let (head, last) = match path.rsplit_once('/') {
            Some((h, l)) => (h, l),
            None => ("", path),
        };
        self.group(head)?.get(last)
    }

    pub fn find(&self, uid: u64) -> Option<&Prop> {
        for c in &self.children {
            match c {
                Node::Prop(p) if p.uid == uid => return Some(p),
                Node::Group(g) => {
                    if let Some(p) = g.find(uid) {
                        return Some(p);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub fn find_mut(&mut self, uid: u64) -> Option<&mut Prop> {
        for c in &mut self.children {
            match c {
                Node::Prop(p) if p.uid == uid => return Some(p),
                Node::Group(g) => {
                    if let Some(p) = g.find_mut(uid) {
                        return Some(p);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub fn find_group(&self, uid: u64) -> Option<&Group> {
        if self.uid == uid {
            return Some(self);
        }
        self.groups().find_map(|g| g.find_group(uid))
    }

    pub fn find_group_mut(&mut self, uid: u64) -> Option<&mut Group> {
        if self.uid == uid {
            return Some(self);
        }
        for c in &mut self.children {
            if let Node::Group(g) = c
                && let Some(x) = g.find_group_mut(uid)
            {
                return Some(x);
            }
        }
        None
    }

    pub fn parent_of(&self, uid: u64) -> Option<&Group> {
        if self.children.iter().any(|c| c.uid() == uid) {
            return Some(self);
        }
        self.groups().find_map(|g| g.parent_of(uid))
    }

    pub fn parent_of_mut(&mut self, uid: u64) -> Option<&mut Group> {
        if self.children.iter().any(|c| c.uid() == uid) {
            return Some(self);
        }
        for c in &mut self.children {
            if let Node::Group(g) = c
                && let Some(x) = g.parent_of_mut(uid)
            {
                return Some(x);
            }
        }
        None
    }

    pub fn path_of(&self, uid: u64) -> Option<String> {
        for (i, c) in self.children.iter().enumerate() {
            let seg = if matches!(self.kind, GroupKind::Indexed) { format!("#{}", i + 1) } else { c.id().to_string() };
            if c.uid() == uid {
                return Some(seg);
            }
            if let Node::Group(g) = c
                && let Some(rest) = g.path_of(uid)
            {
                return Some(format!("{seg}/{rest}"));
            }
        }
        None
    }

    pub fn resolve(&self, path: &str) -> Option<NodeRef<'_>> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return Some(NodeRef::Group(self));
        }
        let mut g = self;
        for (i, part) in parts.iter().enumerate() {
            let node = g.child(part)?;
            if i + 1 == parts.len() {
                return Some(match node {
                    Node::Prop(p) => NodeRef::Prop(p),
                    Node::Group(sub) => NodeRef::Group(sub),
                });
            }
            match node {
                Node::Group(sub) => g = sub,
                Node::Prop(_) => return None,
            }
        }
        None
    }

    pub fn resolve_uid(&self, path: &str) -> Option<u64> {
        self.resolve(path).map(|n| match n {
            NodeRef::Prop(p) => p.uid,
            NodeRef::Group(g) => g.uid,
        })
    }

    pub fn child(&self, seg: &str) -> Option<&Node> {
        if let Some(n) = seg.strip_prefix('#').and_then(|s| s.parse::<usize>().ok()) {
            return self.children.get(n.checked_sub(1)?);
        }
        if let Some(c) = self.children.iter().find(|c| c.id() == seg) {
            return Some(c);
        }
        let lower = seg.to_ascii_lowercase();
        if let Some(c) = self.children.iter().find(|c| c.name().to_ascii_lowercase() == lower) {
            return Some(c);
        }
        let flat = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase();
        let want = flat(seg);
        self.children.iter().find(|c| flat(c.id()) == want || flat(c.name()) == want)
    }

    pub fn any_animated(&self) -> bool {
        self.children.iter().any(|c| match c {
            Node::Prop(p) => p.is_animated() || p.expr.as_ref().is_some_and(|e| e.enabled),
            Node::Group(g) => g.any_animated(),
        })
    }

    pub fn for_each_prop<'a>(&'a self, f: &mut dyn FnMut(&'a Prop)) {
        for c in &self.children {
            match c {
                Node::Prop(p) => f(p),
                Node::Group(g) => g.for_each_prop(f),
            }
        }
    }

    pub fn for_each_prop_mut(&mut self, f: &mut dyn FnMut(&mut Prop)) {
        for c in &mut self.children {
            match c {
                Node::Prop(p) => f(p),
                Node::Group(g) => g.for_each_prop_mut(f),
            }
        }
    }

    pub fn renumber(&mut self, next: &mut u64) {
        *next += 1;
        self.uid = *next;
        for c in &mut self.children {
            match c {
                Node::Prop(p) => {
                    *next += 1;
                    p.uid = *next;
                }
                Node::Group(g) => g.renumber(next),
            }
        }
    }
}

pub struct Ids<'a>(pub &'a mut u64);

impl Ids<'_> {
    pub fn next(&mut self) -> u64 {
        *self.0 += 1;
        *self.0
    }

    pub fn prop(&mut self, id: &str, name: &str, value: Value, ui: Ui) -> Prop {
        Prop::new(self.next(), id, name, value, ui)
    }

    pub fn group(&mut self, id: &str, name: &str, kind: GroupKind, children: Vec<Node>) -> Group {
        Group::new(self.next(), id, name, kind).with(children)
    }
}

pub fn p(x: Prop) -> Node {
    Node::Prop(x)
}

pub fn g(x: Group) -> Node {
    Node::Group(x)
}

pub fn transform_group(ids: &mut Ids, anchor: [f64; 3], position: [f64; 3]) -> Group {
    let children = vec![
        p(ids.prop("anchor", "Anchor Point", Value::Vec3(anchor), Ui::Point).spatial()),
        p(ids.prop("position", "Position", Value::Vec3(position), Ui::Point).spatial()),
        p(ids.prop("scale", "Scale", Value::Vec3([100.0, 100.0, 100.0]), Ui::Percent)),
        p(ids.prop("orientation", "Orientation", Value::Vec3([0.0; 3]), Ui::Point3).only_3d()),
        p(ids.prop("rotationX", "X Rotation", Value::Num(0.0), Ui::Angle).only_3d()),
        p(ids.prop("rotationY", "Y Rotation", Value::Num(0.0), Ui::Angle).only_3d()),
        p(ids.prop("rotation", "Rotation", Value::Num(0.0), Ui::Angle)),
        p(ids.prop("opacity", "Opacity", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
    ];
    ids.group("transform", "Transform", GroupKind::Plain, children)
}

pub fn camera_transform_group(ids: &mut Ids, poi: [f64; 3], position: [f64; 3]) -> Group {
    let children = vec![
        p(ids.prop("poi", "Point of Interest", Value::Vec3(poi), Ui::Point3).spatial()),
        p(ids.prop("position", "Position", Value::Vec3(position), Ui::Point3).spatial()),
        p(ids.prop("orientation", "Orientation", Value::Vec3([0.0; 3]), Ui::Point3)),
        p(ids.prop("rotationX", "X Rotation", Value::Num(0.0), Ui::Angle)),
        p(ids.prop("rotationY", "Y Rotation", Value::Num(0.0), Ui::Angle)),
        p(ids.prop("rotation", "Z Rotation", Value::Num(0.0), Ui::Angle)),
    ];
    ids.group("transform", "Transform", GroupKind::Plain, children)
}

pub fn camera_options(ids: &mut Ids, zoom: f64) -> Group {
    let children = vec![
        p(ids.prop("zoom", "Zoom", Value::Num(zoom), Ui::slider(1.0, 100_000.0, 1.0, 10_000.0, 1))),
        p(ids.prop("dof", "Depth of Field", Value::Bool(false), Ui::Checkbox)),
        p(ids.prop("focusDistance", "Focus Distance", Value::Num(zoom), Ui::slider(1.0, 100_000.0, 1.0, 10_000.0, 1))),
        p(ids.prop("aperture", "Aperture", Value::Num(25.0), Ui::slider(0.0, 10_000.0, 0.0, 500.0, 1))),
        p(ids.prop("blurLevel", "Blur Level", Value::Num(100.0), Ui::slider(0.0, 1000.0, 0.0, 300.0, 0))),
    ];
    ids.group("cameraOptions", "Camera Options", GroupKind::Plain, children)
}

pub const LIGHT_TYPES: [&str; 5] = ["Parallel", "Spot", "Point", "Ambient", "Environment"];
pub const SHADOW_MODES: [&str; 3] = ["Off", "On", "Only"];

pub fn light_options(ids: &mut Ids, kind: u32, color: [f64; 4]) -> Group {
    let children = vec![
        p(ids.prop("lightType", "Light Type", Value::Enum(kind), Ui::popup(&LIGHT_TYPES)).fixed()),
        p(ids.prop("intensity", "Intensity", Value::Num(100.0), Ui::slider(-100_000.0, 100_000.0, 0.0, 400.0, 0))),
        p(ids.prop("color", "Color", Value::Color(color), Ui::Color)),
        p(ids.prop("coneAngle", "Cone Angle", Value::Num(90.0), Ui::slider(0.0, 180.0, 0.0, 180.0, 1))),
        p(ids.prop("coneFeather", "Cone Feather", Value::Num(50.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
        p(ids.prop("falloff", "Falloff", Value::Enum(0), Ui::popup(&["None", "Smooth", "Inverse Square Clamped"]))),
        p(ids.prop("radius", "Radius", Value::Num(500.0), Ui::slider(0.0, 100_000.0, 0.0, 2000.0, 0))),
        p(ids.prop("falloffDistance", "Falloff Distance", Value::Num(500.0), Ui::slider(0.0, 100_000.0, 0.0, 2000.0, 0))),
        p(ids.prop("castsShadows", "Casts Shadows", Value::Bool(false), Ui::Checkbox)),
        p(ids.prop("shadowDarkness", "Shadow Darkness", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
        p(ids.prop("shadowDiffusion", "Shadow Diffusion", Value::Num(0.0), Ui::slider(0.0, 1000.0, 0.0, 100.0, 1))),
        p(ids.prop("source", "Source", Value::Layer(None), Ui::Layer)),
        p(ids.prop("environmentRotation", "Environment Rotation", Value::Num(0.0), Ui::Angle)),
    ];
    ids.group("lightOptions", "Light Options", GroupKind::Plain, children)
}

pub fn material_options(ids: &mut Ids, model: bool) -> Group {
    let mut children = vec![
        p(ids.prop("castsShadows", "Casts Shadows", Value::Enum(0), Ui::popup(&SHADOW_MODES))),
        p(ids.prop("lightTransmission", "Light Transmission", Value::Num(0.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
        p(ids.prop("acceptsShadows", "Accepts Shadows", Value::Enum(1), Ui::popup(&SHADOW_MODES))),
        p(ids.prop("acceptsLights", "Accepts Lights", Value::Bool(true), Ui::Checkbox)),
        p(ids.prop("appearsInReflections", "Appears in Reflections", Value::Bool(true), Ui::Checkbox)),
        p(ids.prop("ambient", "Ambient", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
        p(ids.prop("diffuse", "Diffuse", Value::Num(50.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
        p(ids.prop("specularIntensity", "Specular Intensity", Value::Num(50.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
        p(ids.prop("specularShininess", "Specular Shininess", Value::Num(5.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
        p(ids.prop("metal", "Metal", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
        p(ids.prop("reflectionIntensity", "Reflection Intensity", Value::Num(0.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
    ];
    if model {
        children.push(p(ids.prop("overrideMaterial", "Override Material", Value::Bool(false), Ui::Checkbox)));
        children.push(p(ids.prop("baseColor", "Base Color", Value::Color([1.0, 1.0, 1.0, 1.0]), Ui::Color)));
        children.push(p(ids.prop("metallic", "Metallic", Value::Num(0.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))));
        children.push(p(ids.prop("roughness", "Roughness", Value::Num(50.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))));
        children.push(p(ids.prop("emissive", "Emissive", Value::Color([0.0, 0.0, 0.0, 1.0]), Ui::Color)));
    }
    ids.group("materialOptions", "Material Options", GroupKind::Plain, children)
}

pub const PRIMITIVES: [&str; 6] = ["Cube", "Sphere", "Plane", "Torus", "Cone", "Cylinder"];

pub fn primitive_group(ids: &mut Ids, kind: u32, size: f64) -> Group {
    let children = vec![
        p(ids.prop("kind", "Shape", Value::Enum(kind), Ui::popup(&PRIMITIVES)).fixed()),
        p(ids.prop("width", "Width", Value::Num(size), Ui::slider(0.0, 100_000.0, 0.0, 2000.0, 1))),
        p(ids.prop("height", "Height", Value::Num(size), Ui::slider(0.0, 100_000.0, 0.0, 2000.0, 1))),
        p(ids.prop("depth", "Depth", Value::Num(size), Ui::slider(0.0, 100_000.0, 0.0, 2000.0, 1))),
        p(ids.prop("radius", "Radius", Value::Num(size / 2.0), Ui::slider(0.0, 100_000.0, 0.0, 1000.0, 1))),
        p(ids.prop("tubeRadius", "Tube Radius", Value::Num(size / 6.0), Ui::slider(0.0, 100_000.0, 0.0, 500.0, 1))),
        p(ids.prop("segments", "Segments", Value::Num(48.0), Ui::slider(3.0, 256.0, 3.0, 128.0, 0))),
        p(ids.prop("rings", "Rings", Value::Num(24.0), Ui::slider(2.0, 256.0, 2.0, 128.0, 0))),
    ];
    ids.group("geometry", "Geometry", GroupKind::Plain, children)
}

pub fn audio_group(ids: &mut Ids) -> Group {
    let children = vec![p(ids.prop("levels", "Audio Levels", Value::Vec2([0.0, 0.0]), Ui::Plain))];
    ids.group("audio", "Audio", GroupKind::Plain, children)
}

pub fn mask_group(ids: &mut Ids, name: &str, path: BezPath, mode: MaskMode, color: [u8; 3]) -> Group {
    let children = vec![
        p(ids.prop("path", "Mask Path", Value::Path(path), Ui::Path)),
        p(ids.prop("feather", "Mask Feather", Value::Vec2([0.0, 0.0]), Ui::Pixels)),
        p(ids.prop("opacity", "Mask Opacity", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
        p(ids.prop("expansion", "Mask Expansion", Value::Num(0.0), Ui::slider(-10_000.0, 10_000.0, -100.0, 100.0, 1))),
    ];
    ids.group("mask", name, GroupKind::Mask { mode, inverted: false, color, locked: false }, children)
}

pub fn text_group(ids: &mut Ids, doc: TextDoc) -> Group {
    let mut source = ids.prop("sourceText", "Source Text", Value::Text(Box::new(doc)), Ui::Text);
    source.hold_only = true;
    let children = vec![
        p(source),
        p(ids.prop("perChar3d", "Per-character 3D", Value::Bool(false), Ui::Hidden).fixed()),
        g(path_options(ids)),
        g(more_options(ids)),
        g(ids.group("animators", "Animators", GroupKind::Indexed, vec![])),
    ];
    ids.group("text", "Text", GroupKind::Plain, children)
}

pub fn path_options(ids: &mut Ids) -> Group {
    let children = vec![
        p(ids.prop("path", "Path", Value::Enum(0), Ui::Plain).fixed()),
        p(ids.prop("reversePath", "Reverse Path", Value::Bool(false), Ui::Checkbox)),
        p(ids.prop("perpendicular", "Perpendicular To Path", Value::Bool(true), Ui::Checkbox)),
        p(ids.prop("forceAlignment", "Force Alignment", Value::Bool(false), Ui::Checkbox)),
        p(ids.prop("firstMargin", "First Margin", Value::Num(0.0), Ui::Pixels)),
        p(ids.prop("lastMargin", "Last Margin", Value::Num(0.0), Ui::Pixels)),
    ];
    ids.group("pathOptions", "Path Options", GroupKind::Plain, children)
}

pub const INTER_CHAR_BLEND: [&str; 6] = ["Normal", "Multiply", "Screen", "Overlay", "Add", "Darken"];

pub fn more_options(ids: &mut Ids) -> Group {
    let children = vec![
        p(ids.prop("anchorGrouping", "Anchor Point Grouping", Value::Enum(0), Ui::popup(&["Character", "Word", "Line", "All"])).fixed()),
        p(ids.prop("groupingAlignment", "Grouping Alignment", Value::Vec2([0.0, 0.0]), Ui::Percent)),
        p(ids.prop("fillStroke", "Fill & Stroke", Value::Enum(0), Ui::popup(&["Per Character Palette", "All Fills Over All Strokes", "All Strokes Over All Fills"])).fixed()),
        p(ids.prop("interCharBlend", "Inter-Character Blending", Value::Enum(0), Ui::popup(&INTER_CHAR_BLEND)).fixed()),
    ];
    ids.group("moreOptions", "More Options", GroupKind::Plain, children)
}

pub const BASED_ON: [&str; 4] = ["Characters", "Characters Excluding Spaces", "Words", "Lines"];
pub const SELECTOR_MODES: [&str; 6] = ["Add", "Subtract", "Intersect", "Min", "Max", "Difference"];
pub const SELECTOR_SHAPES: [&str; 6] = ["Square", "Ramp Up", "Ramp Down", "Triangle", "Round", "Smooth"];
pub const EXPRESSION_SELECTOR_TEXT: &str = "selectorValue * textIndex / textTotal";

pub fn range_selector(ids: &mut Ids, name: &str) -> Group {
    let adv = vec![
        p(ids.prop("units", "Units", Value::Enum(0), Ui::popup(&["Percentage", "Index"])).fixed()),
        p(ids.prop("basedOn", "Based On", Value::Enum(0), Ui::popup(&BASED_ON)).fixed()),
        p(ids.prop("mode", "Mode", Value::Enum(0), Ui::popup(&SELECTOR_MODES)).fixed()),
        p(ids.prop("amount", "Amount", Value::Num(100.0), Ui::slider(-100.0, 100.0, -100.0, 100.0, 0))),
        p(ids.prop("shape", "Shape", Value::Enum(0), Ui::popup(&SELECTOR_SHAPES)).fixed()),
        p(ids.prop("smoothness", "Smoothness", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
        p(ids.prop("easeHigh", "Ease High", Value::Num(0.0), Ui::slider(-100.0, 100.0, -100.0, 100.0, 0))),
        p(ids.prop("easeLow", "Ease Low", Value::Num(0.0), Ui::slider(-100.0, 100.0, -100.0, 100.0, 0))),
        p(ids.prop("randomize", "Randomize Order", Value::Bool(false), Ui::Checkbox).fixed()),
        p(ids.prop("randomSeed", "Random Seed", Value::Num(0.0), Ui::slider(0.0, 10_000.0, 0.0, 100.0, 0))),
    ];
    let advanced = g(ids.group("advanced", "Advanced", GroupKind::Plain, adv));
    let children = vec![
        p(ids.prop("start", "Start", Value::Num(0.0), Ui::Percent)),
        p(ids.prop("end", "End", Value::Num(100.0), Ui::Percent)),
        p(ids.prop("offset", "Offset", Value::Num(0.0), Ui::Percent)),
        advanced,
    ];
    ids.group("rangeSelector", name, GroupKind::Plain, children)
}

pub fn wiggly_selector(ids: &mut Ids, name: &str) -> Group {
    let children = vec![
        p(ids.prop("mode", "Mode", Value::Enum(2), Ui::popup(&SELECTOR_MODES)).fixed()),
        p(ids.prop("maxAmount", "Max Amount", Value::Num(100.0), Ui::slider(-100.0, 100.0, -100.0, 100.0, 0))),
        p(ids.prop("minAmount", "Min Amount", Value::Num(-100.0), Ui::slider(-100.0, 100.0, -100.0, 100.0, 0))),
        p(ids.prop("basedOn", "Based On", Value::Enum(0), Ui::popup(&BASED_ON)).fixed()),
        p(ids.prop("wigglesPerSecond", "Wiggles/Second", Value::Num(2.0), Ui::slider(0.0, 1000.0, 0.0, 10.0, 1))),
        p(ids.prop("correlation", "Correlation", Value::Num(50.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
        p(ids.prop("temporalPhase", "Temporal Phase", Value::Num(0.0), Ui::Angle)),
        p(ids.prop("spatialPhase", "Spatial Phase", Value::Num(0.0), Ui::Angle)),
        p(ids.prop("lockDimensions", "Lock Dimensions", Value::Bool(false), Ui::Checkbox)),
        p(ids.prop("randomSeed", "Random Seed", Value::Num(0.0), Ui::slider(0.0, 10_000.0, 0.0, 100.0, 0))),
    ];
    ids.group("wigglySelector", name, GroupKind::Plain, children)
}

pub fn expression_selector(ids: &mut Ids, name: &str) -> Group {
    let mut amount = ids.prop("amount", "Amount", Value::Vec3([100.0; 3]), Ui::Percent);
    amount.expr = Some(Expr { text: EXPRESSION_SELECTOR_TEXT.into(), enabled: true });
    let children = vec![p(ids.prop("basedOn", "Based On", Value::Enum(0), Ui::popup(&BASED_ON)).fixed()), p(amount)];
    ids.group("expressionSelector", name, GroupKind::Plain, children)
}

pub fn text_selector(ids: &mut Ids, kind: &str, name: &str) -> Option<Group> {
    Some(match kind {
        "range" => range_selector(ids, name),
        "wiggly" => wiggly_selector(ids, name),
        "expression" => expression_selector(ids, name),
        _ => return None,
    })
}

pub const TEXT_ANIMATOR_KINDS: [(&str, &str); 25] = [
    ("anchor", "Anchor Point"),
    ("position", "Position"),
    ("scale", "Scale"),
    ("skew", "Skew"),
    ("rotation", "Rotation"),
    ("opacity", "Opacity"),
    ("transformAll", "All Transform Properties"),
    ("lineAnchor", "Line Anchor"),
    ("lineSpacing", "Line Spacing"),
    ("characterOffset", "Character Offset"),
    ("characterValue", "Character Value"),
    ("blur", "Blur"),
    ("fillColor", "Fill Color: RGB"),
    ("fillHue", "Fill Color: Hue"),
    ("fillSaturation", "Fill Color: Saturation"),
    ("fillBrightness", "Fill Color: Brightness"),
    ("fillOpacity", "Fill Color: Opacity"),
    ("strokeColor", "Stroke Color: RGB"),
    ("strokeHue", "Stroke Color: Hue"),
    ("strokeSaturation", "Stroke Color: Saturation"),
    ("strokeBrightness", "Stroke Color: Brightness"),
    ("strokeOpacity", "Stroke Color: Opacity"),
    ("strokeWidth", "Stroke Width"),
    ("tracking", "Tracking"),
    ("fontAxis", "Variable Font Axis"),
];

pub fn text_kind_alias(kind: &str) -> &str {
    match kind {
        "anchorPoint" | "anchor point" => "anchor",
        "fill" | "fillRGB" => "fillColor",
        "stroke" | "strokeRGB" => "strokeColor",
        "trackingAmount" => "tracking",
        "all" | "allTransform" => "transformAll",
        other => other,
    }
}

fn pct(ids: &mut Ids, id: &str, name: &str, v: f64, min: f64) -> Prop {
    ids.prop(id, name, Value::Num(v), Ui::slider(min, 100.0, min, 100.0, 0))
}

pub fn text_anim_props(ids: &mut Ids, kind: &str, three_d: bool) -> Vec<Prop> {
    let kind = text_kind_alias(kind);
    match kind {
        "anchor" => vec![ids.prop("anchor", "Anchor Point", Value::Vec3([0.0; 3]), Ui::Point)],
        "position" => vec![ids.prop("position", "Position", Value::Vec3([0.0; 3]), Ui::Point)],
        "scale" => vec![ids.prop("scale", "Scale", Value::Vec3([100.0; 3]), Ui::Percent)],
        "skew" => vec![ids.prop("skew", "Skew", Value::Num(0.0), Ui::slider(-85.0, 85.0, -70.0, 70.0, 1)), ids.prop("skewAxis", "Skew Axis", Value::Num(0.0), Ui::Angle)],
        "rotation" => {
            if three_d {
                vec![ids.prop("rotationX", "X Rotation", Value::Num(0.0), Ui::Angle), ids.prop("rotationY", "Y Rotation", Value::Num(0.0), Ui::Angle), ids.prop("rotation", "Z Rotation", Value::Num(0.0), Ui::Angle)]
            } else {
                vec![ids.prop("rotation", "Rotation", Value::Num(0.0), Ui::Angle)]
            }
        }
        "rotationX" => vec![ids.prop("rotationX", "X Rotation", Value::Num(0.0), Ui::Angle)],
        "rotationY" => vec![ids.prop("rotationY", "Y Rotation", Value::Num(0.0), Ui::Angle)],
        "opacity" => vec![pct(ids, "opacity", "Opacity", 100.0, 0.0)],
        "transformAll" => ["anchor", "position", "scale", "skew", "rotation", "opacity"].iter().flat_map(|k| text_anim_props(ids, k, three_d)).collect(),
        "lineAnchor" => vec![ids.prop("lineAnchor", "Line Anchor", Value::Num(0.0), Ui::Percent)],
        "lineSpacing" => vec![ids.prop("lineSpacing", "Line Spacing", Value::Vec2([0.0, 0.0]), Ui::Point)],
        "characterOffset" => vec![
            ids.prop("characterAlignment", "Character Alignment", Value::Enum(1), Ui::popup(&["Left or Top", "Center", "Right or Bottom", "Adjust Kerning"])).fixed(),
            ids.prop("characterRange", "Character Range", Value::Enum(0), Ui::popup(&["Preserve Case & Digits", "Full Unicode"])).fixed(),
            ids.prop("characterOffset", "Character Offset", Value::Num(0.0), Ui::slider(-10_000.0, 10_000.0, -100.0, 100.0, 0)),
        ],
        "characterValue" => vec![
            ids.prop("characterRange", "Character Range", Value::Enum(0), Ui::popup(&["Preserve Case & Digits", "Full Unicode"])).fixed(),
            ids.prop("characterValue", "Character Value", Value::Num(0.0), Ui::slider(0.0, 1_114_111.0, 0.0, 255.0, 0)),
        ],
        "blur" => vec![ids.prop("blur", "Blur", Value::Vec2([0.0, 0.0]), Ui::Pixels)],
        "fillColor" => vec![ids.prop("fillColor", "Fill Color", Value::Color([1.0, 0.0, 0.0, 1.0]), Ui::Color)],
        "fillHue" => vec![ids.prop("fillHue", "Fill Hue", Value::Num(0.0), Ui::Angle)],
        "fillSaturation" => vec![pct(ids, "fillSaturation", "Fill Saturation", 0.0, -100.0)],
        "fillBrightness" => vec![pct(ids, "fillBrightness", "Fill Brightness", 0.0, -100.0)],
        "fillOpacity" => vec![pct(ids, "fillOpacity", "Fill Opacity", 100.0, 0.0)],
        "strokeColor" => vec![ids.prop("strokeColor", "Stroke Color", Value::Color([1.0, 0.0, 0.0, 1.0]), Ui::Color)],
        "strokeHue" => vec![ids.prop("strokeHue", "Stroke Hue", Value::Num(0.0), Ui::Angle)],
        "strokeSaturation" => vec![pct(ids, "strokeSaturation", "Stroke Saturation", 0.0, -100.0)],
        "strokeBrightness" => vec![pct(ids, "strokeBrightness", "Stroke Brightness", 0.0, -100.0)],
        "strokeOpacity" => vec![pct(ids, "strokeOpacity", "Stroke Opacity", 100.0, 0.0)],
        "strokeWidth" => vec![ids.prop("strokeWidth", "Stroke Width", Value::Num(0.0), Ui::Pixels)],
        "tracking" => vec![ids.prop("trackingType", "Tracking Type", Value::Enum(0), Ui::popup(&["Before & After", "Before", "After"])).fixed(), ids.prop("tracking", "Tracking Amount", Value::Num(0.0), Ui::slider(-10_000.0, 10_000.0, -200.0, 200.0, 0))],
        _ => vec![],
    }
}

pub fn font_axis_prop(ids: &mut Ids, tag: &str, name: &str, default: f64, min: f64, max: f64) -> Prop {
    ids.prop(&format!("axis_{tag}"), name, Value::Num(default), Ui::slider(min, max, min, max, 1))
}

pub fn text_animator(ids: &mut Ids, name: &str, props: Vec<Prop>, selector: Option<Group>) -> Group {
    let sels = g(ids.group("selectors", "Selectors", GroupKind::Indexed, selector.into_iter().map(g).collect()));
    let pg = g(ids.group("properties", "Properties", GroupKind::Plain, props.into_iter().map(p).collect()));
    ids.group("animator", name, GroupKind::Plain, vec![sels, pg])
}

pub fn shape_transform(ids: &mut Ids) -> Group {
    let children = vec![
        p(ids.prop("anchor", "Anchor Point", Value::Vec2([0.0, 0.0]), Ui::Point)),
        p(ids.prop("position", "Position", Value::Vec2([0.0, 0.0]), Ui::Point)),
        p(ids.prop("scale", "Scale", Value::Vec2([100.0, 100.0]), Ui::Percent)),
        p(ids.prop("skew", "Skew", Value::Num(0.0), Ui::slider(-85.0, 85.0, -85.0, 85.0, 1))),
        p(ids.prop("skewAxis", "Skew Axis", Value::Num(0.0), Ui::Angle)),
        p(ids.prop("rotation", "Rotation", Value::Num(0.0), Ui::Angle)),
        p(ids.prop("opacity", "Opacity", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
    ];
    ids.group("transform", "Transform", GroupKind::Plain, children)
}

pub const SHAPE_KINDS: [&str; 19] = ["group", "rect", "ellipse", "star", "path", "fill", "stroke", "gradientFill", "gradientStroke", "trim", "repeater", "roundCorners", "offsetPaths", "twist", "wiggle", "mergePaths", "puckerBloat", "zigZag", "wiggleTransform"];

pub const SHAPE_OPERATORS: [&str; 14] = ["fill", "stroke", "gradientFill", "gradientStroke", "trim", "repeater", "roundCorners", "offsetPaths", "twist", "wiggle", "mergePaths", "puckerBloat", "zigZag", "wiggleTransform"];

pub const MERGE_MODES: [&str; 5] = ["Merge", "Add", "Subtract", "Intersect", "Exclude Intersections"];

pub fn shape_kind_alias(kind: &str) -> &str {
    match kind {
        "rectangle" => "rect",
        "polystar" => "star",
        "trimPaths" => "trim",
        "wigglePaths" => "wiggle",
        "round" => "roundCorners",
        "offset" => "offsetPaths",
        "merge" => "mergePaths",
        "pucker" | "puckerAndBloat" => "puckerBloat",
        "zigzag" => "zigZag",
        "gfill" => "gradientFill",
        "gstroke" => "gradientStroke",
        other => other,
    }
}

pub fn stroke_taper(ids: &mut Ids) -> Group {
    let c = vec![
        p(ids.prop("units", "Length Units", Value::Enum(0), Ui::popup(&["Pixels", "Percent"]))),
        p(ids.prop("startLength", "Start Length", Value::Num(0.0), Ui::slider(0.0, 100_000.0, 0.0, 500.0, 1))),
        p(ids.prop("endLength", "End Length", Value::Num(0.0), Ui::slider(0.0, 100_000.0, 0.0, 500.0, 1))),
        p(ids.prop("startWidth", "Start Width", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 1))),
        p(ids.prop("endWidth", "End Width", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 1))),
        p(ids.prop("startEase", "Start Ease", Value::Num(0.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 1))),
        p(ids.prop("endEase", "End Ease", Value::Num(0.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 1))),
    ];
    ids.group("taper", "Taper", GroupKind::Plain, c)
}

pub fn stroke_wave(ids: &mut Ids) -> Group {
    let c = vec![
        p(ids.prop("amount", "Amount", Value::Num(0.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 1))),
        p(ids.prop("units", "Units", Value::Enum(0), Ui::popup(&["Pixels", "Cycles"]))),
        p(ids.prop("wavelength", "Wavelength", Value::Num(20.0), Ui::slider(0.1, 100_000.0, 1.0, 500.0, 1))),
        p(ids.prop("cycles", "Cycles", Value::Num(10.0), Ui::slider(0.0, 1000.0, 0.0, 50.0, 1))),
        p(ids.prop("phase", "Phase", Value::Num(0.0), Ui::Angle)),
    ];
    ids.group("wave", "Wave", GroupKind::Plain, c)
}

pub const LINE_CAPS: [&str; 3] = ["Butt Cap", "Round Cap", "Projecting Cap"];
pub const LINE_JOINS: [&str; 3] = ["Miter Join", "Round Join", "Bevel Join"];

pub fn shape_item(ids: &mut Ids, kind: &str, name: Option<&str>) -> Option<Group> {
    let nm = |d: &str| name.unwrap_or(d).to_string();
    let grp = |ids: &mut Ids, id: &str, n: String, children: Vec<Node>| ids.group(id, &n, GroupKind::Plain, children);
    Some(match kind {
        "group" => {
            let contents = g(ids.group("contents", "Contents", GroupKind::Indexed, vec![]));
            let tr = g(shape_transform(ids));
            grp(ids, "group", nm("Group 1"), vec![contents, tr])
        }
        "rect" => {
            let c = vec![
                p(ids.prop("size", "Size", Value::Vec2([100.0, 100.0]), Ui::Pixels)),
                p(ids.prop("position", "Position", Value::Vec2([0.0, 0.0]), Ui::Point)),
                p(ids.prop("roundness", "Roundness", Value::Num(0.0), Ui::slider(0.0, 100_000.0, 0.0, 200.0, 1))),
            ];
            grp(ids, "rect", nm("Rectangle Path 1"), c)
        }
        "ellipse" => {
            let c = vec![
                p(ids.prop("size", "Size", Value::Vec2([100.0, 100.0]), Ui::Pixels)),
                p(ids.prop("position", "Position", Value::Vec2([0.0, 0.0]), Ui::Point)),
            ];
            grp(ids, "ellipse", nm("Ellipse Path 1"), c)
        }
        "star" | "polygon" => {
            let poly = kind == "polygon";
            let c = vec![
                p(ids.prop("type", "Type", Value::Enum(if poly { 1 } else { 0 }), Ui::popup(&["Star", "Polygon"]))),
                p(ids.prop("points", "Points", Value::Num(5.0), Ui::slider(3.0, 100.0, 3.0, 20.0, 0))),
                p(ids.prop("position", "Position", Value::Vec2([0.0, 0.0]), Ui::Point)),
                p(ids.prop("rotation", "Rotation", Value::Num(0.0), Ui::Angle)),
                p(ids.prop("innerRadius", "Inner Radius", Value::Num(25.0), Ui::slider(0.0, 100_000.0, 0.0, 500.0, 1))),
                p(ids.prop("outerRadius", "Outer Radius", Value::Num(50.0), Ui::slider(0.0, 100_000.0, 0.0, 500.0, 1))),
                p(ids.prop("innerRoundness", "Inner Roundness", Value::Num(0.0), Ui::slider(-1000.0, 1000.0, -100.0, 100.0, 0))),
                p(ids.prop("outerRoundness", "Outer Roundness", Value::Num(0.0), Ui::slider(-1000.0, 1000.0, -100.0, 100.0, 0))),
            ];
            grp(ids, "star", nm(if poly { "Polystar Path 1" } else { "Polystar Path 1" }), c)
        }
        "path" => {
            let c = vec![p(ids.prop("path", "Path", Value::Path(BezPath::default()), Ui::Path))];
            grp(ids, "path", nm("Path 1"), c)
        }
        "fill" => {
            let c = vec![
                p(ids.prop("rule", "Fill Rule", Value::Enum(0), Ui::popup(&["Non-Zero Winding", "Even-Odd"]))),
                p(ids.prop("color", "Color", Value::Color([1.0, 0.0, 0.0, 1.0]), Ui::Color)),
                p(ids.prop("opacity", "Opacity", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
            ];
            grp(ids, "fill", nm("Fill 1"), c)
        }
        "stroke" => {
            let c = vec![
                p(ids.prop("color", "Color", Value::Color([1.0, 1.0, 1.0, 1.0]), Ui::Color)),
                p(ids.prop("opacity", "Opacity", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
                p(ids.prop("width", "Stroke Width", Value::Num(2.0), Ui::slider(0.0, 100_000.0, 0.0, 100.0, 1))),
                p(ids.prop("cap", "Line Cap", Value::Enum(0), Ui::popup(&LINE_CAPS))),
                p(ids.prop("join", "Line Join", Value::Enum(0), Ui::popup(&LINE_JOINS))),
                p(ids.prop("miter", "Miter Limit", Value::Num(4.0), Ui::slider(1.0, 100.0, 1.0, 10.0, 1))),
                p(ids.prop("dash", "Dash", Value::Num(0.0), Ui::slider(0.0, 10_000.0, 0.0, 200.0, 1))),
                p(ids.prop("gap", "Gap", Value::Num(0.0), Ui::slider(0.0, 10_000.0, 0.0, 200.0, 1))),
                p(ids.prop("dash2", "Dash 2", Value::Num(0.0), Ui::slider(0.0, 10_000.0, 0.0, 200.0, 1))),
                p(ids.prop("gap2", "Gap 2", Value::Num(0.0), Ui::slider(0.0, 10_000.0, 0.0, 200.0, 1))),
                p(ids.prop("dash3", "Dash 3", Value::Num(0.0), Ui::slider(0.0, 10_000.0, 0.0, 200.0, 1))),
                p(ids.prop("gap3", "Gap 3", Value::Num(0.0), Ui::slider(0.0, 10_000.0, 0.0, 200.0, 1))),
                p(ids.prop("dashOffset", "Offset", Value::Num(0.0), Ui::slider(-10_000.0, 10_000.0, -200.0, 200.0, 1))),
                g(stroke_taper(ids)),
                g(stroke_wave(ids)),
            ];
            grp(ids, "stroke", nm("Stroke 1"), c)
        }
        "gradientFill" | "gradientStroke" => {
            let stroke = kind == "gradientStroke";
            let mut c = vec![
                p(ids.prop("type", "Type", Value::Enum(0), Ui::popup(&["Linear", "Radial"]))),
                p(ids.prop("start", "Start Point", Value::Vec2([0.0, 0.0]), Ui::Point)),
                p(ids.prop("end", "End Point", Value::Vec2([100.0, 0.0]), Ui::Point)),
                p(ids.prop("colors", "Colors", Value::Gradient(Gradient::default()), Ui::Gradient)),
                p(ids.prop("opacity", "Opacity", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
            ];
            if stroke {
                c.push(p(ids.prop("width", "Stroke Width", Value::Num(2.0), Ui::slider(0.0, 100_000.0, 0.0, 100.0, 1))));
                c.push(p(ids.prop("cap", "Line Cap", Value::Enum(0), Ui::popup(&LINE_CAPS))));
                c.push(p(ids.prop("join", "Line Join", Value::Enum(0), Ui::popup(&LINE_JOINS))));
                c.push(p(ids.prop("miter", "Miter Limit", Value::Num(4.0), Ui::slider(1.0, 100.0, 1.0, 10.0, 1))));
                c.push(p(ids.prop("dash", "Dash", Value::Num(0.0), Ui::slider(0.0, 10_000.0, 0.0, 200.0, 1))));
                c.push(p(ids.prop("gap", "Gap", Value::Num(0.0), Ui::slider(0.0, 10_000.0, 0.0, 200.0, 1))));
                c.push(p(ids.prop("dashOffset", "Offset", Value::Num(0.0), Ui::slider(-10_000.0, 10_000.0, -200.0, 200.0, 1))));
                c.push(g(stroke_taper(ids)));
                c.push(g(stroke_wave(ids)));
            } else {
                c.insert(0, p(ids.prop("rule", "Fill Rule", Value::Enum(0), Ui::popup(&["Non-Zero Winding", "Even-Odd"]))));
            }
            grp(ids, kind, nm(if stroke { "Gradient Stroke 1" } else { "Gradient Fill 1" }), c)
        }
        "trim" => {
            let c = vec![
                p(ids.prop("start", "Start", Value::Num(0.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 1))),
                p(ids.prop("end", "End", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 1))),
                p(ids.prop("offset", "Offset", Value::Num(0.0), Ui::Angle)),
                p(ids.prop("mode", "Trim Multiple Shapes", Value::Enum(0), Ui::popup(&["Simultaneously", "Individually"]))),
            ];
            grp(ids, "trim", nm("Trim Paths 1"), c)
        }
        "repeater" => {
            let tr = vec![
                p(ids.prop("anchor", "Anchor Point", Value::Vec2([0.0, 0.0]), Ui::Point)),
                p(ids.prop("position", "Position", Value::Vec2([100.0, 0.0]), Ui::Point)),
                p(ids.prop("scale", "Scale", Value::Vec2([100.0, 100.0]), Ui::Percent)),
                p(ids.prop("rotation", "Rotation", Value::Num(0.0), Ui::Angle)),
                p(ids.prop("startOpacity", "Start Opacity", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
                p(ids.prop("endOpacity", "End Opacity", Value::Num(100.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
            ];
            let trg = g(ids.group("transform", "Transform: Repeater", GroupKind::Plain, tr));
            let c = vec![
                p(ids.prop("copies", "Copies", Value::Num(3.0), Ui::slider(0.0, 1000.0, 0.0, 50.0, 1))),
                p(ids.prop("offset", "Offset", Value::Num(0.0), Ui::slider(-1000.0, 1000.0, -10.0, 10.0, 1))),
                p(ids.prop("composite", "Composite", Value::Enum(0), Ui::popup(&["Below", "Above"]))),
                trg,
            ];
            grp(ids, "repeater", nm("Repeater 1"), c)
        }
        "roundCorners" => {
            let c = vec![p(ids.prop("radius", "Radius", Value::Num(10.0), Ui::slider(0.0, 100_000.0, 0.0, 200.0, 1)))];
            grp(ids, "roundCorners", nm("Round Corners 1"), c)
        }
        "offsetPaths" => {
            let c = vec![
                p(ids.prop("amount", "Amount", Value::Num(10.0), Ui::slider(-10_000.0, 10_000.0, -100.0, 100.0, 1))),
                p(ids.prop("join", "Line Join", Value::Enum(0), Ui::popup(&LINE_JOINS))),
                p(ids.prop("miter", "Miter Limit", Value::Num(4.0), Ui::slider(1.0, 100.0, 1.0, 10.0, 1))),
                p(ids.prop("copies", "Copies", Value::Num(1.0), Ui::slider(0.0, 100.0, 0.0, 20.0, 0))),
                p(ids.prop("copyOffset", "Copy Offset", Value::Num(0.0), Ui::slider(-100.0, 100.0, -10.0, 10.0, 1))),
            ];
            grp(ids, "offsetPaths", nm("Offset Paths 1"), c)
        }
        "mergePaths" => {
            let c = vec![p(ids.prop("mode", "Mode", Value::Enum(0), Ui::popup(&MERGE_MODES)))];
            grp(ids, "mergePaths", nm("Merge Paths 1"), c)
        }
        "puckerBloat" => {
            let c = vec![p(ids.prop("amount", "Amount", Value::Num(0.0), Ui::slider(-100.0, 100.0, -100.0, 100.0, 1)))];
            grp(ids, "puckerBloat", nm("Pucker & Bloat 1"), c)
        }
        "zigZag" => {
            let c = vec![
                p(ids.prop("size", "Size", Value::Num(10.0), Ui::slider(-10_000.0, 10_000.0, 0.0, 100.0, 1))),
                p(ids.prop("ridges", "Ridges per segment", Value::Num(5.0), Ui::slider(0.0, 1000.0, 0.0, 50.0, 0))),
                p(ids.prop("points", "Points", Value::Enum(0), Ui::popup(&["Corner", "Smooth"]))),
            ];
            grp(ids, "zigZag", nm("Zig Zag 1"), c)
        }
        "wiggleTransform" => {
            let tr = vec![
                p(ids.prop("anchor", "Anchor Point", Value::Vec2([0.0, 0.0]), Ui::Point)),
                p(ids.prop("position", "Position", Value::Vec2([0.0, 0.0]), Ui::Point)),
                p(ids.prop("scale", "Scale", Value::Vec2([0.0, 0.0]), Ui::Percent)),
                p(ids.prop("rotation", "Rotation", Value::Num(0.0), Ui::Angle)),
            ];
            let trg = g(ids.group("transform", "Transform", GroupKind::Plain, tr));
            let c = vec![
                p(ids.prop("wigglesPerSecond", "Wiggles/Second", Value::Num(2.0), Ui::slider(0.0, 100.0, 0.0, 10.0, 1))),
                p(ids.prop("correlation", "Correlation", Value::Num(50.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
                p(ids.prop("temporalPhase", "Temporal Phase", Value::Num(0.0), Ui::Angle)),
                p(ids.prop("spatialPhase", "Spatial Phase", Value::Num(0.0), Ui::Angle)),
                p(ids.prop("seed", "Random Seed", Value::Num(0.0), Ui::slider(0.0, 10_000.0, 0.0, 100.0, 0))),
                trg,
            ];
            grp(ids, "wiggleTransform", nm("Wiggle Transform 1"), c)
        }
        "twist" => {
            let c = vec![
                p(ids.prop("angle", "Angle", Value::Num(45.0), Ui::Angle)),
                p(ids.prop("center", "Center", Value::Vec2([0.0, 0.0]), Ui::Point)),
            ];
            grp(ids, "twist", nm("Twist 1"), c)
        }
        "wiggle" => {
            let c = vec![
                p(ids.prop("size", "Size", Value::Num(10.0), Ui::slider(0.0, 10_000.0, 0.0, 100.0, 1))),
                p(ids.prop("detail", "Detail", Value::Num(10.0), Ui::slider(0.0, 1000.0, 0.0, 50.0, 1))),
                p(ids.prop("points", "Points", Value::Enum(1), Ui::popup(&["Corner", "Smooth"]))),
                p(ids.prop("wigglesPerSecond", "Wiggles/Second", Value::Num(2.0), Ui::slider(0.0, 100.0, 0.0, 10.0, 1))),
                p(ids.prop("correlation", "Correlation", Value::Num(50.0), Ui::slider(0.0, 100.0, 0.0, 100.0, 0))),
                p(ids.prop("temporalPhase", "Temporal Phase", Value::Num(0.0), Ui::Angle)),
                p(ids.prop("spatialPhase", "Spatial Phase", Value::Num(0.0), Ui::Angle)),
                p(ids.prop("seed", "Random Seed", Value::Num(0.0), Ui::slider(0.0, 10_000.0, 0.0, 100.0, 0))),
            ];
            grp(ids, "wiggle", nm("Wiggle Paths 1"), c)
        }
        _ => return None,
    })
}
