use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use super::path::BezPath;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Justify {
    #[default]
    Left,
    Center,
    Right,
    JustifyLeft,
    JustifyCenter,
    JustifyRight,
    JustifyAll,
}

impl Justify {
    pub fn parse(s: &str) -> Option<Justify> {
        Some(match s.to_ascii_lowercase().replace([' ', '_', '-'], "").as_str() {
            "left" => Justify::Left,
            "center" | "centre" => Justify::Center,
            "right" => Justify::Right,
            "justifyleft" => Justify::JustifyLeft,
            "justifycenter" => Justify::JustifyCenter,
            "justifyright" => Justify::JustifyRight,
            "justifyall" | "justify" => Justify::JustifyAll,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TextDoc {
    pub text: String,
    pub font: String,
    pub style: String,
    pub size: f64,
    pub fill: [f64; 3],
    pub stroke: [f64; 3],
    pub stroke_width: f64,
    pub apply_fill: bool,
    pub apply_stroke: bool,
    pub stroke_over_fill: bool,
    pub tracking: f64,
    pub leading: Option<f64>,
    pub justify: Justify,
    pub faux_bold: bool,
    pub faux_italic: bool,
    pub all_caps: bool,
    pub small_caps: bool,
    pub baseline_shift: f64,
    pub h_scale: f64,
    pub v_scale: f64,
    pub box_size: Option<[f64; 4]>,
    pub indent_left: f64,
    pub indent_right: f64,
    pub indent_first: f64,
    pub space_before: f64,
    pub space_after: f64,
    pub features: Vec<String>,
    pub variations: std::collections::BTreeMap<String, f64>,
    pub ligatures: bool,
    pub kerning: String,
    pub baseline: String,
    pub runs: Vec<StyleRun>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CharStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_fill: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_stroke: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_shift: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h_scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v_scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub faux_bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub faux_italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_caps: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_caps: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StyleRun {
    pub start: usize,
    pub end: usize,
    pub style: CharStyle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedStyle {
    pub font: String,
    pub style: String,
    pub size: f64,
    pub fill: [f64; 3],
    pub stroke: [f64; 3],
    pub stroke_width: f64,
    pub apply_fill: bool,
    pub apply_stroke: bool,
    pub tracking: f64,
    pub baseline_shift: f64,
    pub h_scale: f64,
    pub v_scale: f64,
    pub faux_bold: bool,
    pub faux_italic: bool,
    pub all_caps: bool,
    pub small_caps: bool,
    pub baseline: String,
    pub features: Vec<String>,
}

impl TextDoc {
    pub fn base_style(&self) -> ResolvedStyle {
        ResolvedStyle {
            font: self.font.clone(),
            style: self.style.clone(),
            size: self.size,
            fill: self.fill,
            stroke: self.stroke,
            stroke_width: self.stroke_width,
            apply_fill: self.apply_fill,
            apply_stroke: self.apply_stroke,
            tracking: self.tracking,
            baseline_shift: self.baseline_shift,
            h_scale: self.h_scale,
            v_scale: self.v_scale,
            faux_bold: self.faux_bold,
            faux_italic: self.faux_italic,
            all_caps: self.all_caps,
            small_caps: self.small_caps,
            baseline: self.baseline.clone(),
            features: self.features.clone(),
        }
    }

    pub fn style_at(&self, i: usize) -> ResolvedStyle {
        let mut s = self.base_style();
        for r in self.runs.iter().filter(|r| i >= r.start && i < r.end) {
            let c = &r.style;
            macro_rules! take {
                ($($f:ident),*) => { $( if let Some(v) = &c.$f { s.$f = v.clone(); } )* };
            }
            take!(font, style, size, fill, stroke, stroke_width, apply_fill, apply_stroke, tracking, baseline_shift, h_scale, v_scale, faux_bold, faux_italic, all_caps, small_caps, baseline);
            for f in &c.features {
                if !s.features.contains(f) {
                    s.features.push(f.clone());
                }
            }
        }
        s
    }

    pub fn clip_runs(&mut self) {
        let n = self.text.chars().count();
        for r in &mut self.runs {
            r.end = r.end.min(n);
        }
        self.runs.retain(|r| r.start < r.end);
    }
}

impl Default for TextDoc {
    fn default() -> Self {
        TextDoc {
            text: String::new(),
            font: "Inter".into(),
            style: "Regular".into(),
            size: 72.0,
            fill: [1.0, 1.0, 1.0],
            stroke: [0.0, 0.0, 0.0],
            stroke_width: 0.0,
            apply_fill: true,
            apply_stroke: false,
            stroke_over_fill: true,
            tracking: 0.0,
            leading: None,
            justify: Justify::Left,
            faux_bold: false,
            faux_italic: false,
            all_caps: false,
            small_caps: false,
            baseline_shift: 0.0,
            h_scale: 100.0,
            v_scale: 100.0,
            box_size: None,
            indent_left: 0.0,
            indent_right: 0.0,
            indent_first: 0.0,
            space_before: 0.0,
            space_after: 0.0,
            features: vec![],
            variations: Default::default(),
            ligatures: true,
            kerning: "metrics".into(),
            baseline: "normal".into(),
            runs: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradStop {
    pub pos: f64,
    pub color: [f64; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    pub stops: Vec<GradStop>,
}

impl Default for Gradient {
    fn default() -> Self {
        Gradient { stops: vec![GradStop { pos: 0.0, color: [1.0, 1.0, 1.0, 1.0] }, GradStop { pos: 1.0, color: [0.0, 0.0, 0.0, 1.0] }] }
    }
}

impl Gradient {
    pub fn sample(&self, t: f64) -> [f64; 4] {
        let s = &self.stops;
        if s.is_empty() {
            return [0.0, 0.0, 0.0, 1.0];
        }
        if t <= s[0].pos {
            return s[0].color;
        }
        for w in s.windows(2) {
            if t <= w[1].pos {
                let span = (w[1].pos - w[0].pos).max(1e-9);
                let u = (t - w[0].pos) / span;
                let mut c = [0.0; 4];
                for k in 0..4 {
                    c[k] = w[0].color[k] + (w[1].color[k] - w[0].color[k]) * u;
                }
                return c;
            }
        }
        s[s.len() - 1].color
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Value {
    Num(f64),
    Vec2([f64; 2]),
    Vec3([f64; 3]),
    Color([f64; 4]),
    Bool(bool),
    Enum(u32),
    Text(Box<TextDoc>),
    Path(BezPath),
    Layer(Option<u64>),
    Str(String),
    Gradient(Gradient),
}

impl Value {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Value::Num(_) => "scalar",
            Value::Vec2(_) => "vec2",
            Value::Vec3(_) => "vec3",
            Value::Color(_) => "color",
            Value::Bool(_) => "bool",
            Value::Enum(_) => "enum",
            Value::Text(_) => "text",
            Value::Path(_) => "path",
            Value::Layer(_) => "layer",
            Value::Str(_) => "string",
            Value::Gradient(_) => "gradient",
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Value::Num(_) | Value::Vec2(_) | Value::Vec3(_) | Value::Color(_))
    }

    pub fn components(&self) -> Vec<f64> {
        match self {
            Value::Num(x) => vec![*x],
            Value::Vec2(v) => v.to_vec(),
            Value::Vec3(v) => v.to_vec(),
            Value::Color(c) => c.to_vec(),
            Value::Bool(b) => vec![if *b { 1.0 } else { 0.0 }],
            Value::Enum(e) => vec![*e as f64],
            _ => vec![],
        }
    }

    pub fn dims(&self) -> usize {
        match self {
            Value::Num(_) | Value::Bool(_) | Value::Enum(_) => 1,
            Value::Vec2(_) => 2,
            Value::Vec3(_) => 3,
            Value::Color(_) => 4,
            _ => 1,
        }
    }

    pub fn with_components(&self, c: &[f64]) -> Value {
        let g = |i: usize, d: f64| c.get(i).copied().unwrap_or(d);
        match self {
            Value::Num(x) => Value::Num(g(0, *x)),
            Value::Vec2(v) => Value::Vec2([g(0, v[0]), g(1, v[1])]),
            Value::Vec3(v) => Value::Vec3([g(0, v[0]), g(1, v[1]), g(2, v[2])]),
            Value::Color(v) => Value::Color([g(0, v[0]), g(1, v[1]), g(2, v[2]), g(3, v[3])]),
            Value::Bool(b) => Value::Bool(g(0, if *b { 1.0 } else { 0.0 }) >= 0.5),
            Value::Enum(e) => Value::Enum(g(0, *e as f64).round().max(0.0) as u32),
            other => other.clone(),
        }
    }

    pub fn num(&self) -> f64 {
        match self {
            Value::Num(x) => *x,
            Value::Enum(e) => *e as f64,
            Value::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Vec2(v) => v[0],
            Value::Vec3(v) => v[0],
            Value::Color(c) => c[0],
            _ => 0.0,
        }
    }

    pub fn vec2(&self) -> [f64; 2] {
        match self {
            Value::Vec2(v) => *v,
            Value::Vec3(v) => [v[0], v[1]],
            Value::Num(x) => [*x, *x],
            _ => [0.0; 2],
        }
    }

    pub fn vec3(&self) -> [f64; 3] {
        match self {
            Value::Vec3(v) => *v,
            Value::Vec2(v) => [v[0], v[1], 0.0],
            Value::Num(x) => [*x, *x, *x],
            _ => [0.0; 3],
        }
    }

    pub fn color(&self) -> [f64; 4] {
        match self {
            Value::Color(c) => *c,
            Value::Vec3(v) => [v[0], v[1], v[2], 1.0],
            _ => [0.0, 0.0, 0.0, 1.0],
        }
    }

    pub fn boolean(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Num(x) => *x != 0.0,
            Value::Enum(e) => *e != 0,
            _ => false,
        }
    }

    pub fn index(&self) -> u32 {
        match self {
            Value::Enum(e) => *e,
            Value::Num(x) => x.round().max(0.0) as u32,
            Value::Bool(b) => *b as u32,
            _ => 0,
        }
    }

    pub fn as_path(&self) -> Option<&BezPath> {
        match self {
            Value::Path(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&TextDoc> {
        match self {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }

    pub fn as_gradient(&self) -> Option<&Gradient> {
        match self {
            Value::Gradient(g) => Some(g),
            _ => None,
        }
    }

    pub fn layer(&self) -> Option<u64> {
        match self {
            Value::Layer(l) => *l,
            _ => None,
        }
    }

    pub fn to_json(&self) -> Json {
        match self {
            Value::Num(x) => json!(x),
            Value::Vec2(v) => json!(v),
            Value::Vec3(v) => json!(v),
            Value::Color(c) => json!(c),
            Value::Bool(b) => json!(b),
            Value::Enum(e) => json!(e),
            Value::Text(t) => json!(t.text),
            Value::Path(p) => json!({"vertices": p.vertices, "in": p.ins, "out": p.outs, "closed": p.closed}),
            Value::Layer(l) => json!(l),
            Value::Str(s) => json!(s),
            Value::Gradient(g) => json!(g.stops.iter().map(|s| json!([s.pos, s.color[0], s.color[1], s.color[2], s.color[3]])).collect::<Vec<_>>()),
        }
    }

    pub fn lerp(&self, b: &Value, t: f64) -> Value {
        match (self, b) {
            (Value::Num(x), Value::Num(y)) => Value::Num(x + (y - x) * t),
            (Value::Vec2(x), Value::Vec2(y)) => Value::Vec2([x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t]),
            (Value::Vec3(x), Value::Vec3(y)) => Value::Vec3([x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t]),
            (Value::Color(x), Value::Color(y)) => Value::Color([x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t, x[3] + (y[3] - x[3]) * t]),
            (Value::Path(x), Value::Path(y)) => Value::Path(x.lerp(y, t)),
            (Value::Gradient(x), Value::Gradient(y)) if x.stops.len() == y.stops.len() => Value::Gradient(Gradient {
                stops: x
                    .stops
                    .iter()
                    .zip(&y.stops)
                    .map(|(a, b)| {
                        let mut c = [0.0; 4];
                        for k in 0..4 {
                            c[k] = a.color[k] + (b.color[k] - a.color[k]) * t;
                        }
                        GradStop { pos: a.pos + (b.pos - a.pos) * t, color: c }
                    })
                    .collect(),
            }),
            _ => {
                if t < 1.0 {
                    self.clone()
                } else {
                    b.clone()
                }
            }
        }
    }

    pub fn parse_like(&self, j: &Json, options: &[String]) -> Result<Value, String> {
        let nums = |j: &Json| -> Option<Vec<f64>> {
            match j {
                Json::Number(n) => n.as_f64().map(|x| vec![x]),
                Json::Array(a) => a.iter().map(Json::as_f64).collect(),
                _ => None,
            }
        };
        Ok(match self {
            Value::Num(_) => Value::Num(j.as_f64().or_else(|| nums(j).and_then(|v| v.first().copied())).ok_or("expected a number")?),
            Value::Vec2(cur) => {
                let v = nums(j).ok_or("expected [x, y]")?;
                let v = if v.len() == 1 { vec![v[0], v[0]] } else { v };
                Value::Vec2([v.first().copied().unwrap_or(cur[0]), v.get(1).copied().unwrap_or(cur[1])])
            }
            Value::Vec3(cur) => {
                let v = nums(j).ok_or("expected [x, y] or [x, y, z]")?;
                let v = if v.len() == 1 { vec![v[0], v[0], v[0]] } else { v };
                Value::Vec3([v.first().copied().unwrap_or(cur[0]), v.get(1).copied().unwrap_or(cur[1]), v.get(2).copied().unwrap_or(cur[2])])
            }
            Value::Color(cur) => {
                let c = parse_color(j).ok_or("expected a colour: [r, g, b] (0-1), [r, g, b, a] or #rrggbb")?;
                Value::Color([c[0], c[1], c[2], if j.as_array().is_some_and(|a| a.len() >= 4) { c[3] } else { cur[3] }])
            }
            Value::Bool(_) => Value::Bool(match j {
                Json::Bool(b) => *b,
                Json::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                Json::String(s) => matches!(s.to_ascii_lowercase().as_str(), "true" | "on" | "yes" | "1"),
                _ => return Err("expected true or false".into()),
            }),
            Value::Enum(_) => match j {
                Json::Number(n) => Value::Enum(n.as_f64().unwrap_or(0.0).round().max(0.0) as u32),
                Json::String(s) => {
                    let want = norm(s);
                    let i = options.iter().position(|o| norm(o) == want).ok_or_else(|| format!("`{s}` is not one of: {}", options.join(", ")))?;
                    Value::Enum(i as u32)
                }
                _ => return Err("expected an option index or name".into()),
            },
            Value::Text(cur) => match j {
                Json::String(s) => {
                    let mut d = (**cur).clone();
                    d.text = s.clone();
                    Value::Text(Box::new(d))
                }
                Json::Object(_) => {
                    let mut d = (**cur).clone();
                    apply_text_attrs(&mut d, j)?;
                    Value::Text(Box::new(d))
                }
                _ => return Err("expected text".into()),
            },
            Value::Path(_) => Value::Path(parse_path(j).ok_or("expected a path: {vertices: [[x, y]...], in?, out?, closed?}")?),
            Value::Layer(_) => match j {
                Json::Null => Value::Layer(None),
                Json::Number(n) => Value::Layer(n.as_u64()),
                _ => return Err("expected a layer id or null".into()),
            },
            Value::Str(_) => Value::Str(j.as_str().map(str::to_string).unwrap_or_else(|| j.to_string())),
            Value::Gradient(_) => Value::Gradient(parse_gradient(j).ok_or("expected gradient stops: [[pos, r, g, b, a]...]")?),
        })
    }
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

pub fn parse_color(j: &Json) -> Option<[f64; 4]> {
    match j {
        Json::String(s) => hex_color(s),
        Json::Array(a) => {
            let v: Vec<f64> = a.iter().filter_map(Json::as_f64).collect();
            if v.len() < 3 {
                return None;
            }
            let scale = if v.iter().take(3).any(|x| *x > 1.0) { 255.0 } else { 1.0 };
            Some([v[0] / scale, v[1] / scale, v[2] / scale, v.get(3).copied().map(|a| if a > 1.0 { a / 255.0 } else { a }).unwrap_or(1.0)])
        }
        _ => None,
    }
}

pub fn hex_color(s: &str) -> Option<[f64; 4]> {
    let h = s.trim().trim_start_matches('#');
    let h = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect::<String>() } else { h.to_string() };
    if h.len() != 6 && h.len() != 8 {
        return None;
    }
    let b = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|x| x as f64 / 255.0);
    Some([b(0)?, b(2)?, b(4)?, if h.len() == 8 { b(6)? } else { 1.0 }])
}

pub fn parse_path(j: &Json) -> Option<BezPath> {
    let pts = |v: Option<&Json>| -> Option<Vec<[f64; 2]>> {
        v?.as_array()?.iter().map(|p| {
            let a = p.as_array()?;
            Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
        }).collect()
    };
    let o = j.as_object()?;
    let vertices = pts(o.get("vertices").or_else(|| o.get("points")))?;
    let n = vertices.len();
    let ins = pts(o.get("in").or_else(|| o.get("inTangents"))).unwrap_or_else(|| vec![[0.0; 2]; n]);
    let outs = pts(o.get("out").or_else(|| o.get("outTangents"))).unwrap_or_else(|| vec![[0.0; 2]; n]);
    let closed = o.get("closed").and_then(Json::as_bool).unwrap_or(true);
    Some(BezPath { vertices, ins, outs, closed }.normalized())
}

pub fn parse_gradient(j: &Json) -> Option<Gradient> {
    let a = j.as_array()?;
    let mut stops = vec![];
    for s in a {
        let v: Vec<f64> = s.as_array()?.iter().filter_map(Json::as_f64).collect();
        if v.len() < 4 {
            return None;
        }
        stops.push(GradStop { pos: v[0], color: [v[1], v[2], v[3], v.get(4).copied().unwrap_or(1.0)] });
    }
    stops.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    Some(Gradient { stops })
}

pub fn apply_text_attrs(d: &mut TextDoc, p: &Json) -> Result<(), String> {
    let o = match p.as_object() {
        Some(o) => o,
        None => return Ok(()),
    };
    let f = |k: &str| o.get(k).and_then(Json::as_f64);
    let b = |k: &str| o.get(k).and_then(Json::as_bool);
    if let Some(t) = o.get("text").and_then(Json::as_str) {
        d.text = t.to_string();
        d.clip_runs();
    }
    if let Some(r) = o.get("range").and_then(Json::as_array) {
        let n = d.text.chars().count();
        let a = r.first().and_then(Json::as_f64).unwrap_or(0.0).max(0.0) as usize;
        let z = r.get(1).and_then(Json::as_f64).map(|x| x.max(0.0) as usize).unwrap_or(n).min(n);
        if a < z && !(a == 0 && z == n) {
            let style = char_style_from(p)?;
            if style != CharStyle::default() {
                d.runs.push(StyleRun { start: a, end: z, style });
            }
            apply_paragraph_attrs(d, p)?;
            return Ok(());
        }
    }
    apply_opentype(&mut d.features, p);
    if let Some(x) = b("ligatures") {
        d.ligatures = x;
    }
    if let Some(s) = o.get("kerning") {
        d.kerning = s.as_str().map(str::to_string).unwrap_or_else(|| s.to_string());
    }
    if let Some(s) = baseline_of(p) {
        d.baseline = s;
    }
    match o.get("variations") {
        Some(Json::Null) => d.variations.clear(),
        Some(Json::Object(m)) => {
            for (k, v) in m {
                match v.as_f64() {
                    Some(x) => {
                        d.variations.insert(k.clone(), x);
                    }
                    None => {
                        d.variations.remove(k);
                    }
                }
            }
        }
        _ => {}
    }
    if let Some(s) = o.get("font").and_then(Json::as_str) {
        d.font = s.to_string();
    }
    if let Some(s) = o.get("style").and_then(Json::as_str) {
        d.style = s.to_string();
    }
    if let Some(x) = f("size") {
        d.size = x.max(0.1);
    }
    if let Some(c) = o.get("fill").and_then(parse_color) {
        d.fill = [c[0], c[1], c[2]];
        if o.get("applyFill").is_none() {
            d.apply_fill = true;
        }
    }
    if let Some(c) = o.get("stroke").and_then(parse_color) {
        d.stroke = [c[0], c[1], c[2]];
        if o.get("applyStroke").is_none() {
            d.apply_stroke = true;
        }
        if d.stroke_width <= 0.0 && f("strokeWidth").is_none() {
            d.stroke_width = 2.0;
        }
    }
    if let Some(x) = f("strokeWidth") {
        d.stroke_width = x.max(0.0);
        if x > 0.0 && o.get("applyStroke").is_none() {
            d.apply_stroke = true;
        }
    }
    if let Some(x) = b("applyFill") {
        d.apply_fill = x;
    }
    if let Some(x) = b("applyStroke") {
        d.apply_stroke = x;
    }
    if let Some(x) = b("strokeOverFill") {
        d.stroke_over_fill = x;
    }
    if let Some(x) = f("tracking") {
        d.tracking = x;
    }
    match o.get("leading") {
        Some(Json::String(s)) if s == "auto" => d.leading = None,
        Some(Json::Null) => d.leading = None,
        Some(v) => {
            if let Some(x) = v.as_f64() {
                d.leading = Some(x);
            }
        }
        None => {}
    }
    if let Some(s) = o.get("justify").and_then(Json::as_str) {
        d.justify = Justify::parse(s).ok_or_else(|| format!("unknown justify `{s}`"))?;
    }
    if let Some(x) = b("fauxBold") {
        d.faux_bold = x;
    }
    if let Some(x) = b("fauxItalic") {
        d.faux_italic = x;
    }
    if let Some(x) = b("allCaps") {
        d.all_caps = x;
    }
    if let Some(x) = b("smallCaps") {
        d.small_caps = x;
    }
    if let Some(x) = f("baselineShift") {
        d.baseline_shift = x;
    }
    if let Some(x) = f("hScale") {
        d.h_scale = x;
    }
    if let Some(x) = f("vScale") {
        d.v_scale = x;
    }
    apply_paragraph_attrs(d, p)
}

pub fn baseline_of(p: &Json) -> Option<String> {
    if let Some(s) = p.get("baseline").and_then(Json::as_str) {
        return Some(s.to_string());
    }
    if p.get("superscript").and_then(Json::as_bool) == Some(true) {
        return Some("superscript".into());
    }
    if p.get("subscript").and_then(Json::as_bool) == Some(true) {
        return Some("subscript".into());
    }
    if p.get("superscript").and_then(Json::as_bool) == Some(false) || p.get("subscript").and_then(Json::as_bool) == Some(false) {
        return Some("normal".into());
    }
    None
}

pub const OT_KEYS: [(&str, &str); 10] = [
    ("discretionaryLigatures", "dlig"),
    ("contextualAlternates", "calt"),
    ("stylisticAlternates", "salt"),
    ("swash", "swsh"),
    ("titling", "titl"),
    ("ordinals", "ordn"),
    ("fractions", "frac"),
    ("allSmallCaps", "c2sc"),
    ("superiors", "sups"),
    ("inferiors", "subs"),
];

fn toggle_feature(list: &mut Vec<String>, tag: &str, on: bool) {
    list.retain(|f| f.trim_start_matches('-') != tag);
    list.push(if on { tag.to_string() } else { format!("-{tag}") });
}

pub fn apply_opentype(list: &mut Vec<String>, p: &Json) {
    let Some(o) = p.as_object() else { return };
    for (key, tag) in OT_KEYS {
        if let Some(on) = o.get(key).and_then(Json::as_bool) {
            toggle_feature(list, tag, on);
            if tag == "c2sc" {
                toggle_feature(list, "smcp", on);
            }
        }
    }
    for n in 1..=20 {
        if let Some(on) = o.get(&format!("ss{n:02}")).and_then(Json::as_bool) {
            toggle_feature(list, &format!("ss{n:02}"), on);
        }
    }
    if let Some(a) = o.get("stylisticSets").and_then(Json::as_array) {
        list.retain(|f| !f.trim_start_matches('-').starts_with("ss"));
        for n in a.iter().filter_map(Json::as_u64).filter(|n| (1..=20).contains(n)) {
            list.push(format!("ss{n:02}"));
        }
    }
    match o.get("figureStyle").and_then(Json::as_str) {
        Some("lining") => {
            toggle_feature(list, "lnum", true);
            list.retain(|f| f.trim_start_matches('-') != "onum");
        }
        Some("oldStyle") => {
            toggle_feature(list, "onum", true);
            list.retain(|f| f.trim_start_matches('-') != "lnum");
        }
        Some(_) => list.retain(|f| !matches!(f.trim_start_matches('-'), "lnum" | "onum")),
        None => {}
    }
    match o.get("figureWidth").and_then(Json::as_str) {
        Some("proportional") => {
            toggle_feature(list, "pnum", true);
            list.retain(|f| f.trim_start_matches('-') != "tnum");
        }
        Some("tabular") => {
            toggle_feature(list, "tnum", true);
            list.retain(|f| f.trim_start_matches('-') != "pnum");
        }
        Some(_) => list.retain(|f| !matches!(f.trim_start_matches('-'), "pnum" | "tnum")),
        None => {}
    }
    if let Some(a) = o.get("features").and_then(Json::as_array) {
        for t in a.iter().filter_map(Json::as_str) {
            let on = !t.starts_with('-');
            let tag = t.trim_start_matches(['-', '+']);
            if tag.len() == 4 {
                toggle_feature(list, tag, on);
            }
        }
    }
}

pub fn char_style_from(p: &Json) -> Result<CharStyle, String> {
    let mut c = CharStyle::default();
    let Some(o) = p.as_object() else { return Ok(c) };
    let f = |k: &str| o.get(k).and_then(Json::as_f64);
    let b = |k: &str| o.get(k).and_then(Json::as_bool);
    c.font = o.get("font").and_then(Json::as_str).map(str::to_string);
    c.style = o.get("style").and_then(Json::as_str).map(str::to_string);
    c.size = f("size").map(|x| x.max(0.1));
    c.fill = o.get("fill").and_then(parse_color).map(|x| [x[0], x[1], x[2]]);
    c.stroke = o.get("stroke").and_then(parse_color).map(|x| [x[0], x[1], x[2]]);
    c.stroke_width = f("strokeWidth").map(|x| x.max(0.0));
    c.apply_fill = b("applyFill").or(c.fill.map(|_| true));
    c.apply_stroke = b("applyStroke").or(c.stroke.map(|_| true)).or(c.stroke_width.map(|w| w > 0.0));
    c.tracking = f("tracking");
    c.baseline_shift = f("baselineShift");
    c.h_scale = f("hScale");
    c.v_scale = f("vScale");
    c.faux_bold = b("fauxBold");
    c.faux_italic = b("fauxItalic");
    c.all_caps = b("allCaps");
    c.small_caps = b("smallCaps");
    c.baseline = baseline_of(p);
    apply_opentype(&mut c.features, p);
    Ok(c)
}

fn apply_paragraph_attrs(d: &mut TextDoc, p: &Json) -> Result<(), String> {
    let Some(o) = p.as_object() else { return Ok(()) };
    let f = |k: &str| o.get(k).and_then(Json::as_f64);
    if let Some(s) = o.get("justify").and_then(Json::as_str) {
        d.justify = Justify::parse(s).ok_or_else(|| format!("unknown justify `{s}`"))?;
    }
    if let Some(x) = f("indentLeft").or_else(|| f("indent")) {
        d.indent_left = x;
    }
    if let Some(x) = f("indentRight") {
        d.indent_right = x;
    }
    if let Some(x) = f("indentFirst") {
        d.indent_first = x;
    }
    if let Some(x) = f("spaceBefore") {
        d.space_before = x;
    }
    if let Some(x) = f("spaceAfter") {
        d.space_after = x;
    }
    match o.get("leading") {
        Some(Json::String(s)) if s == "auto" => d.leading = None,
        Some(v) if !v.is_null() => {
            if let Some(x) = v.as_f64() {
                d.leading = Some(x);
            }
        }
        _ => {}
    }
    match o.get("box") {
        Some(Json::Null) => d.box_size = None,
        Some(Json::Array(a)) => {
            let v: Vec<f64> = a.iter().filter_map(Json::as_f64).collect();
            if v.len() == 4 {
                d.box_size = Some([v[0], v[1], v[2], v[3]]);
            }
        }
        _ => {}
    }
    Ok(())
}
