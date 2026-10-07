use serde_json::{Map, Value as Json, json};

use super::interp::*;
use crate::model::{Justify, StyleRun, TextDoc};

const MARK: &str = "__textstyle";

macro_rules! setters {
    ($($f:ident => $field:expr),* $(,)?) => {
        $(fn $f(it: &mut Interp, this: &V, a: Vec<V>) -> R {
            set_named(it, this, a, $field)
        })*
    };
}

setters!(
    s_font => "font",
    s_font_size => "fontSize",
    s_fill => "fillColor",
    s_stroke => "strokeColor",
    s_stroke_width => "strokeWidth",
    s_apply_fill => "applyFill",
    s_apply_stroke => "applyStroke",
    s_tracking => "tracking",
    s_leading => "leading",
    s_auto_leading => "autoLeading",
    s_faux_bold => "isFauxBold",
    s_faux_italic => "isFauxItalic",
    s_all_caps => "isAllCaps",
    s_small_caps => "isSmallCaps",
    s_baseline => "baselineShift",
    s_h_scale => "horizontalScaling",
    s_v_scale => "verticalScaling",
    s_justify => "justification",
);

const FIELDS: [(&str, &str, &str, NativeImpl); 18] = [
    ("font", "setFont", "font", s_font),
    ("fontSize", "setFontSize", "size", s_font_size),
    ("fillColor", "setFillColor", "fill", s_fill),
    ("strokeColor", "setStrokeColor", "stroke", s_stroke),
    ("strokeWidth", "setStrokeWidth", "strokeWidth", s_stroke_width),
    ("applyFill", "setApplyFill", "applyFill", s_apply_fill),
    ("applyStroke", "setApplyStroke", "applyStroke", s_apply_stroke),
    ("tracking", "setTracking", "tracking", s_tracking),
    ("leading", "setLeading", "leading", s_leading),
    ("autoLeading", "setAutoLeading", "autoLeading", s_auto_leading),
    ("isFauxBold", "setFauxBold", "fauxBold", s_faux_bold),
    ("isFauxItalic", "setFauxItalic", "fauxItalic", s_faux_italic),
    ("isAllCaps", "setAllCaps", "allCaps", s_all_caps),
    ("isSmallCaps", "setSmallCaps", "smallCaps", s_small_caps),
    ("baselineShift", "setBaselineShift", "baselineShift", s_baseline),
    ("horizontalScaling", "setHorizontalScaling", "hScale", s_h_scale),
    ("verticalScaling", "setVerticalScaling", "vScale", s_v_scale),
    ("justification", "setJustification", "justify", s_justify),
];

pub fn style_v(d: &TextDoc) -> V {
    let props: Vec<(&str, V)> = vec![
        (MARK, V::Bool(true)),
        ("font", s(&d.font)),
        ("fontSize", V::Num(d.size)),
        ("fillColor", nums(&d.fill)),
        ("strokeColor", nums(&d.stroke)),
        ("strokeWidth", V::Num(d.stroke_width)),
        ("applyFill", V::Bool(d.apply_fill)),
        ("applyStroke", V::Bool(d.apply_stroke)),
        ("tracking", V::Num(d.tracking)),
        ("leading", V::Num(d.leading.unwrap_or(d.size * 1.2))),
        ("autoLeading", V::Bool(d.leading.is_none())),
        ("isFauxBold", V::Bool(d.faux_bold)),
        ("isFauxItalic", V::Bool(d.faux_italic)),
        ("isAllCaps", V::Bool(d.all_caps)),
        ("isSmallCaps", V::Bool(d.small_caps)),
        ("baselineShift", V::Num(d.baseline_shift)),
        ("horizontalScaling", V::Num(d.h_scale)),
        ("verticalScaling", V::Num(d.v_scale)),
        ("justification", s(match d.justify {
            Justify::Left => "LEFT_JUSTIFY",
            Justify::Center => "CENTER_JUSTIFY",
            Justify::Right => "RIGHT_JUSTIFY",
            Justify::JustifyLeft => "FULL_JUSTIFY_LASTLINE_LEFT",
            Justify::JustifyCenter => "FULL_JUSTIFY_LASTLINE_CENTER",
            Justify::JustifyRight => "FULL_JUSTIFY_LASTLINE_RIGHT",
            Justify::JustifyAll => "FULL_JUSTIFY_LASTLINE_FULL",
        })),
        ("__text", V::Undef),
        ("__runs", arr(vec![])),
        ("__changed", obj(vec![])),
    ];
    let o = obj(props);
    attach(&o);
    o
}

pub fn style_at_v(d: &TextDoc, i: usize) -> V {
    let r = d.style_at(i);
    let mut x = d.clone();
    x.font = r.font;
    x.style = r.style;
    x.size = r.size;
    x.fill = r.fill;
    x.stroke = r.stroke;
    x.stroke_width = r.stroke_width;
    x.apply_fill = r.apply_fill;
    x.apply_stroke = r.apply_stroke;
    x.tracking = r.tracking;
    x.baseline_shift = r.baseline_shift;
    x.h_scale = r.h_scale;
    x.v_scale = r.v_scale;
    x.faux_bold = r.faux_bold;
    x.faux_italic = r.faux_italic;
    x.all_caps = r.all_caps;
    x.small_caps = r.small_caps;
    style_v(&x)
}

fn attach(o: &V) {
    let V::Obj(ob) = o else { return };
    let mut b = ob.borrow_mut();
    for (_, setter, _, f) in FIELDS {
        b.set(setter, native(setter, o.clone(), f));
    }
    b.set("setText", native("setText", o.clone(), set_text));
}

fn copy(o: &V) -> V {
    let V::Obj(ob) = o else { return o.clone() };
    let props: Vec<(std::rc::Rc<str>, V)> = ob
        .borrow()
        .props
        .iter()
        .filter(|(_, v)| !matches!(v, V::Native(_)))
        .map(|(k, v)| {
            let v = match v {
                V::Obj(inner) => V::Obj(std::rc::Rc::new(std::cell::RefCell::new(Obj { props: inner.borrow().props.clone() }))),
                V::Arr(a) => arr(a.borrow().clone()),
                other => other.clone(),
            };
            (k.clone(), v)
        })
        .collect();
    let n = V::Obj(std::rc::Rc::new(std::cell::RefCell::new(Obj { props })));
    attach(&n);
    n
}

fn set_named(it: &mut Interp, this: &V, a: Vec<V>, field: &str) -> R {
    let value = arg(&a, 0);
    let n = copy(this);
    let V::Obj(ob) = &n else { return Ok(n) };
    if a.len() >= 2 {
        let start = arg(&a, 1);
        let end = arg(&a, 2);
        let key = FIELDS.iter().find(|f| f.0 == field).map(|f| f.2).unwrap_or(field);
        let val = if field == "justification" { s(justify_key(&plain_string(&value))) } else { value };
        let run = obj(vec![("start", start), ("end", end), ("attrs", obj(vec![(key, val)]))]);
        let runs = ob.borrow().get("__runs").cloned();
        if let Some(V::Arr(list)) = runs {
            list.borrow_mut().push(run);
        }
        let _ = it;
        return Ok(n);
    }
    ob.borrow_mut().set(field, value);
    let changed = ob.borrow().get("__changed").cloned();
    if let Some(V::Obj(ch)) = changed {
        ch.borrow_mut().set(field, V::Bool(true));
    }
    Ok(n)
}

fn set_text(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let t = it.to_string(&arg(&a, 0))?;
    let n = copy(this);
    if let V::Obj(ob) = &n {
        ob.borrow_mut().set("__text", s(&t));
    }
    Ok(n)
}

pub fn is_style(v: &V) -> bool {
    matches!(v, V::Obj(o) if o.borrow().get(MARK).is_some())
}

pub fn apply(it: &mut Interp, v: &V, base: &TextDoc) -> Result<TextDoc, Flow> {
    let mut d = base.clone();
    let V::Obj(ob) = v else { return Ok(d) };
    let (text, changed, runs) = {
        let b = ob.borrow();
        (b.get("__text").cloned(), b.get("__changed").cloned(), b.get("__runs").cloned())
    };
    if let Some(V::Str(t)) = text {
        d.text = t.to_string();
        d.clip_runs();
    }
    if let Some(V::Obj(ch)) = changed {
        let keys: Vec<std::rc::Rc<str>> = ch.borrow().props.iter().map(|(k, _)| k.clone()).collect();
        let mut attrs = Map::new();
        for k in keys {
            let val = ob.borrow().get(&k).cloned().unwrap_or(V::Undef);
            if &*k == "justification" {
                let j = plain_string(&val);
                attrs.insert("justify".into(), json!(justify_key(&j)));
            } else if let Some((_, _, key, _)) = FIELDS.iter().find(|f| f.0 == &*k) {
                attrs.insert((*key).into(), json_of(it, &val)?);
            }
        }
        if attrs.get("autoLeading") == Some(&json!(true)) {
            attrs.insert("leading".into(), json!("auto"));
        }
        attrs.remove("autoLeading");
        crate::model::value::apply_text_attrs(&mut d, &Json::Object(attrs)).map_err(|m| type_err(m))?;
    }
    if let Some(V::Arr(list)) = runs {
        let items = list.borrow().clone();
        let n = d.text.chars().count();
        for r in items {
            let V::Obj(ro) = &r else { continue };
            let (start, end, attrs) = {
                let b = ro.borrow();
                (b.get("start").cloned(), b.get("end").cloned(), b.get("attrs").cloned())
            };
            let a0 = it.to_number(&start.unwrap_or(V::Num(0.0)))?.max(0.0) as usize;
            let z0 = match end {
                Some(V::Num(e)) => (e.max(0.0) as usize).min(n),
                _ => n,
            };
            if a0 >= z0 {
                continue;
            }
            let aj = json_of(it, &attrs.unwrap_or(V::Undef))?;
            let style = crate::model::value::char_style_from(&aj).map_err(|m| type_err(m))?;
            d.runs.push(StyleRun { start: a0, end: z0, style });
        }
    }
    Ok(d)
}

fn justify_key(j: &str) -> &'static str {
    match j {
        "left" | "center" | "right" | "justifyLeft" | "justifyCenter" | "justifyRight" | "justifyAll" => match j {
            "center" => "center",
            "right" => "right",
            "justifyLeft" => "justifyLeft",
            "justifyCenter" => "justifyCenter",
            "justifyRight" => "justifyRight",
            "justifyAll" => "justifyAll",
            _ => "left",
        },
        "CENTER_JUSTIFY" => "center",
        "RIGHT_JUSTIFY" => "right",
        "FULL_JUSTIFY_LASTLINE_LEFT" => "justifyLeft",
        "FULL_JUSTIFY_LASTLINE_CENTER" => "justifyCenter",
        "FULL_JUSTIFY_LASTLINE_RIGHT" => "justifyRight",
        "FULL_JUSTIFY_LASTLINE_FULL" => "justifyAll",
        _ => "left",
    }
}

fn json_of(it: &mut Interp, v: &V) -> Result<Json, Flow> {
    Ok(match v {
        V::Num(n) => json!(n),
        V::Bool(b) => json!(b),
        V::Str(t) => json!(&**t),
        V::Arr(a) => {
            let items = a.borrow().clone();
            let mut out = vec![];
            for x in &items {
                out.push(json!(it.to_number(x)?));
            }
            Json::Array(out)
        }
        V::Obj(o) => {
            let props = o.borrow().props.clone();
            let mut m = Map::new();
            for (k, x) in props {
                if !matches!(x, V::Native(_) | V::Func(_)) {
                    m.insert(k.to_string(), json_of(it, &x)?);
                }
            }
            Json::Object(m)
        }
        _ => Json::Null,
    })
}
