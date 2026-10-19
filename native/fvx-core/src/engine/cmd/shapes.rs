use serde_json::{Value as Json, json};

use super::{add_layer, select_only};
use crate::engine::{Engine, Res, pb, pf, ps, pu};
use crate::eval::Eval;
use crate::geom::Mat3;
use crate::model::path::BezPath;
use crate::model::prop::{self, Node};
use crate::model::value::{Gradient, parse_color};
use crate::model::{Group, GroupKind, Ids, ItemKind, Layer, LayerInit, LayerKind, MaskMode, Value, build_layer};
use crate::render::svg::{self, SvgDoc, SvgPaint};

fn set(g: &mut Group, id: &str, v: Value) {
    if let Some(p) = g.get_mut(id) {
        p.value = v;
    }
}

pub fn fill_item(ids: &mut Ids, color: [f64; 4], even_odd: bool, opacity: f64) -> Option<Group> {
    let mut f = prop::shape_item(ids, "fill", None)?;
    set(&mut f, "color", Value::Color([color[0], color[1], color[2], 1.0]));
    set(&mut f, "opacity", Value::Num(opacity * color[3] * 100.0));
    set(&mut f, "rule", Value::Enum(even_odd as u32));
    Some(f)
}

fn gradient_item(ids: &mut Ids, kind: &str, radial: bool, a: [f64; 2], b: [f64; 2], g: &Gradient, opacity: f64) -> Option<Group> {
    let mut f = prop::shape_item(ids, kind, None)?;
    set(&mut f, "type", Value::Enum(radial as u32));
    set(&mut f, "start", Value::Vec2(a));
    set(&mut f, "end", Value::Vec2(b));
    set(&mut f, "colors", Value::Gradient(g.clone()));
    set(&mut f, "opacity", Value::Num(opacity * 100.0));
    Some(f)
}

#[allow(clippy::too_many_arguments)]
pub fn stroke_item(ids: &mut Ids, color: [f64; 4], width: f64, cap: u32, join: u32, miter: f64, dash: &[f64], dash_offset: f64, opacity: f64) -> Option<Group> {
    let mut s = prop::shape_item(ids, "stroke", None)?;
    set(&mut s, "color", Value::Color([color[0], color[1], color[2], 1.0]));
    set(&mut s, "opacity", Value::Num(opacity * color[3] * 100.0));
    set(&mut s, "width", Value::Num(width));
    set(&mut s, "cap", Value::Enum(cap));
    set(&mut s, "join", Value::Enum(join));
    set(&mut s, "miter", Value::Num(miter));
    for (i, (dk, gk)) in [("dash", "gap"), ("dash2", "gap2"), ("dash3", "gap3")].iter().enumerate() {
        if let (Some(d), Some(g)) = (dash.get(i * 2), dash.get(i * 2 + 1)) {
            set(&mut s, dk, Value::Num(*d));
            set(&mut s, gk, Value::Num(*g));
        }
    }
    set(&mut s, "dashOffset", Value::Num(dash_offset));
    Some(s)
}

pub fn path_item(ids: &mut Ids, path: BezPath, name: &str) -> Option<Group> {
    let mut g = prop::shape_item(ids, "path", Some(name))?;
    set(&mut g, "path", Value::Path(path));
    Some(g)
}

pub fn group_item(ids: &mut Ids, name: &str, contents: Vec<Node>) -> Option<Group> {
    let mut g = prop::shape_item(ids, "group", Some(name))?;
    if let Some(c) = g.sub_mut("contents") {
        c.children = contents;
    }
    Some(g)
}

fn paint_item(ids: &mut Ids, p: &SvgPaint, opacity: f64, stroke: bool) -> Option<Group> {
    match p {
        SvgPaint::Solid(c) => {
            if stroke {
                stroke_item(ids, *c, 1.0, 0, 0, 4.0, &[], 0.0, opacity)
            } else {
                fill_item(ids, *c, false, opacity)
            }
        }
        SvgPaint::Linear(a, b, g) => gradient_item(ids, if stroke { "gradientStroke" } else { "gradientFill" }, false, *a, *b, g, opacity),
        SvgPaint::Radial(a, b, g) => gradient_item(ids, if stroke { "gradientStroke" } else { "gradientFill" }, true, *a, *b, g, opacity),
    }
}

pub fn svg_nodes(doc: &SvgDoc, ids: &mut Ids, m: &Mat3) -> Vec<Node> {
    let mut out = vec![];
    for s in &doc.shapes {
        let mut contents: Vec<Node> = vec![];
        for (i, p) in s.paths.iter().enumerate() {
            if let Some(g) = path_item(ids, p.transformed(m), &format!("Path {}", i + 1)) {
                contents.push(Node::Group(g));
            }
        }
        let mut fill = s.fill.as_ref().and_then(|(p, o, eo)| {
            let mut g = paint_item(ids, &transform_paint(p, m), *o, false)?;
            set(&mut g, "rule", Value::Enum(*eo as u32));
            Some(g)
        });
        let stroke = s.stroke.as_ref().and_then(|st| {
            let mut g = paint_item(ids, &transform_paint(&st.paint, m), st.opacity, true)?;
            set(&mut g, "width", Value::Num(st.width * m.max_scale()));
            set(&mut g, "cap", Value::Enum(st.cap));
            set(&mut g, "join", Value::Enum(st.join));
            set(&mut g, "miter", Value::Num(st.miter));
            if st.dash.len() >= 2 {
                set(&mut g, "dash", Value::Num(st.dash[0]));
                set(&mut g, "gap", Value::Num(st.dash[1]));
            }
            set(&mut g, "dashOffset", Value::Num(st.dash_offset));
            Some(g)
        });
        if s.stroke_first {
            contents.extend(fill.take().map(Node::Group));
            contents.extend(stroke.map(Node::Group));
        } else {
            contents.extend(stroke.map(Node::Group));
            contents.extend(fill.take().map(Node::Group));
        }
        if let Some(g) = group_item(ids, &s.name, contents) {
            out.push(Node::Group(g));
        }
    }
    out
}

fn transform_paint(p: &SvgPaint, m: &Mat3) -> SvgPaint {
    match p {
        SvgPaint::Solid(c) => SvgPaint::Solid(*c),
        SvgPaint::Linear(a, b, g) => SvgPaint::Linear(m.apply(a[0], a[1]), m.apply(b[0], b[1]), g.clone()),
        SvgPaint::Radial(a, b, g) => SvgPaint::Radial(m.apply(a[0], a[1]), m.apply(b[0], b[1]), g.clone()),
    }
}

fn new_shape_layer(e: &mut Engine, cid: u64, name: &str) -> Result<Layer, String> {
    let c = e.project.comp(cid).ok_or("no composition")?;
    let (cw, ch, dur) = (c.width, c.height, c.duration);
    let name = super::unique_layer_name(c, name);
    Ok(build_layer(e.proj(), (cw, ch), LayerInit { kind: LayerKind::Shape, name, source: None, size: (cw as f64, ch as f64), start: 0.0, duration: dur }))
}

fn with_contents(e: &mut Engine, l: &mut Layer, f: impl FnOnce(&mut Ids) -> Vec<Node>) {
    let mut next = e.project.next_uid;
    let nodes = f(&mut Ids(&mut next));
    e.proj().next_uid = next;
    if let Some(c) = l.props.sub_mut("contents") {
        c.children = nodes;
    }
}

pub fn paste_svg(e: &mut Engine, p: &Json) -> Res {
    let text = ps(p, "svg").ok_or("pass `svg` (SVG markup)")?;
    let doc = svg::parse(text)?;
    if doc.shapes.is_empty() {
        return Err("the SVG has no paths to paste".into());
    }
    let cid = e.comp_id(p.get("comp"))?;
    let b = svg::bounds(&doc).unwrap_or([0.0, 0.0, doc.width, doc.height]);
    let centre = Mat3::translate(-(b[0] + b[2]) / 2.0, -(b[1] + b[3]) / 2.0);
    let mut l = new_shape_layer(e, cid, ps(p, "name").unwrap_or("Pasted Vector"))?;
    with_contents(e, &mut l, |ids| svg_nodes(&doc, ids, &centre));
    let id = add_layer(e, cid, l, None);
    select_only(e, &[id]);
    e.set_label("Paste Vector as Shapes");
    Ok(json!({"layer": id, "shapes": doc.shapes.len(), "skipped": []}))
}

fn copy_transform(e: &mut Engine, src: &Layer, dst: &mut Layer) {
    if let Some(t) = src.transform() {
        let mut t = t.clone();
        t.renumber(&mut e.proj().next_uid);
        if let Some(slot) = dst.props.children.iter_mut().find(|c| c.id() == "transform") {
            *slot = Node::Group(t);
        }
    }
    dst.switches.three_d = src.switches.three_d;
    dst.parent = src.parent;
    dst.in_point = src.in_point;
    dst.out_point = src.out_point;
    dst.start_time = src.start_time;
    dst.stretch = src.stretch;
}

fn text_glyph_nodes(ids: &mut Ids, glyphs: &[crate::render::text::Glyph]) -> Vec<Node> {
    let mut out = vec![];
    for g in glyphs {
        if g.paths.is_empty() {
            continue;
        }
        let mut contents: Vec<Node> = vec![];
        for (i, p) in g.paths.iter().enumerate() {
            if let Some(pg) = path_item(ids, p.clone(), &format!("Path {}", i + 1)) {
                contents.push(Node::Group(pg));
            }
        }
        let st = &g.style;
        if st.apply_stroke && st.stroke_width > 0.0
            && let Some(s) = stroke_item(ids, [st.stroke[0], st.stroke[1], st.stroke[2], 1.0], st.stroke_width, 1, 1, 4.0, &[], 0.0, 1.0)
        {
            contents.push(Node::Group(s));
        }
        if st.apply_fill
            && let Some(f) = fill_item(ids, [st.fill[0], st.fill[1], st.fill[2], 1.0], false, 1.0)
        {
            contents.push(Node::Group(f));
        }
        let name = if g.ch.is_whitespace() { "Space".to_string() } else { g.ch.to_string() };
        if let Some(grp) = group_item(ids, &name, contents) {
            out.push(Node::Group(grp));
        }
    }
    out
}

pub fn layer_create(e: &mut Engine, p: &Json) -> Res {
    let op = ps(p, "op").ok_or("pass `op`: shapesFromText, masksFromText or shapesFromVector")?.to_string();
    let (cid, ids_) = e.layers_param(p)?;
    if ids_.is_empty() {
        return Err("select the layers to convert".into());
    }
    let now = e.time_of(cid);
    let mut made = vec![];
    for lid in ids_ {
        let src = e.layer_ref(cid, lid)?.clone();
        match (op.as_str(), src.kind) {
            ("shapesFromText", LayerKind::Text) => {
                let lt = src.layer_time(now);
                let Some(doc) = src.text_prop().map(|t| t.at(lt)).and_then(|v| v.as_text().cloned()) else { continue };
                let lay = crate::render::text::layout(&doc);
                let mut l = new_shape_layer(e, cid, &format!("{} Outlines", src.name))?;
                copy_transform(e, &src, &mut l);
                with_contents(e, &mut l, |ids| text_glyph_nodes(ids, &lay.glyphs));
                let at = e.project.comp(cid).and_then(|c| c.index_of(lid)).unwrap_or(0);
                let id = add_layer(e, cid, l, Some(at));
                if let Ok(s) = e.layer_mut(cid, lid) {
                    s.switches.video = false;
                }
                made.push(id);
            }
            ("masksFromText", LayerKind::Text) => {
                let (pj, comp) = (e.project.clone(), e.project.comp(cid).ok_or("no comp")?.clone());
                let ev = Eval::new(&pj, cid, &comp, now);
                let m = ev.layer_to_comp(&src);
                let lt = src.layer_time(now);
                let Some(doc) = src.text_prop().map(|t| t.at(lt)).and_then(|v| v.as_text().cloned()) else { continue };
                let lay = crate::render::text::layout(&doc);
                let (cw, ch) = (comp.width, comp.height);
                let name = format!("{} Outlines", src.name);
                let item = super::solid_item(e, &name, [0.5, 0.5, 0.5], cw, ch);
                let mut l = build_layer(e.proj(), (cw, ch), LayerInit { kind: LayerKind::Solid, name, source: Some(item), size: (cw as f64, ch as f64), start: src.start_time, duration: comp.duration });
                let mut next = e.project.next_uid;
                let mut masks = vec![];
                {
                    let mut ids = Ids(&mut next);
                    let mut k = 0usize;
                    for g in &lay.glyphs {
                        for p in &g.paths {
                            k += 1;
                            let mode = if k == 1 { MaskMode::Add } else { MaskMode::Difference };
                            let color = crate::model::label_rgb(((k % 15) + 1) as u8);
                            masks.push(Node::Group(prop::mask_group(&mut ids, &format!("{} {}", g.ch, k), p.transformed(&m), mode, color)));
                        }
                    }
                }
                e.proj().next_uid = next;
                if let Some(mg) = l.props.sub_mut("masks") {
                    mg.children = masks;
                }
                let at = e.project.comp(cid).and_then(|c| c.index_of(lid)).unwrap_or(0);
                made.push(add_layer(e, cid, l, Some(at)));
            }
            ("shapesFromVector", LayerKind::Footage) => {
                let Some(path) = src.source.and_then(|s| e.project.item(s)).and_then(|i| match &i.kind {
                    ItemKind::Footage(f) if f.kind == crate::model::FootageKind::Vector => Some(f.path.clone()),
                    _ => None,
                }) else {
                    continue;
                };
                let doc = svg::load(&path).ok_or_else(|| format!("{path} is not readable SVG"))?;
                let mut l = new_shape_layer(e, cid, &format!("{} Outlines", src.name))?;
                copy_transform(e, &src, &mut l);
                with_contents(e, &mut l, |ids| svg_nodes(&doc, ids, &Mat3::IDENTITY));
                let at = e.project.comp(cid).and_then(|c| c.index_of(lid)).unwrap_or(0);
                let id = add_layer(e, cid, l, Some(at));
                if let Ok(s) = e.layer_mut(cid, lid) {
                    s.switches.video = false;
                }
                made.push(id);
            }
            ("editableText", _) => return Err("Convert to Editable Text needs a PSD or .ai text layer, which formvfx does not import yet".into()),
            _ => {}
        }
    }
    if made.is_empty() {
        return Err(match op.as_str() {
            "shapesFromVector" => "select an SVG footage layer".into(),
            "shapesFromText" | "masksFromText" => "select a text layer".into(),
            _ => format!("unknown op `{op}`"),
        });
    }
    select_only(e, &made);
    e.set_label(match op.as_str() {
        "shapesFromText" => "Create Shapes from Text",
        "masksFromText" => "Create Masks from Text",
        _ => "Create Shapes from Vector Layer",
    });
    Ok(json!({"layers": made}))
}

pub fn new_path(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let pts: Vec<[f64; 2]> = p.get("vertices").and_then(Json::as_array).map(|a| a.iter().filter_map(|v| Some([v.get(0)?.as_f64()?, v.get(1)?.as_f64()?])).collect()).unwrap_or_default();
    if pts.is_empty() {
        return Err("pass `vertices` [[x, y]...]".into());
    }
    let tan = |k: &str| -> Vec<[f64; 2]> { p.get(k).and_then(Json::as_array).map(|a| a.iter().map(|v| [v.get(0).and_then(Json::as_f64).unwrap_or(0.0), v.get(1).and_then(Json::as_f64).unwrap_or(0.0)]).collect()).unwrap_or_default() };
    let mut bp = BezPath { vertices: pts.clone(), ins: tan("inTangents"), outs: tan("outTangents"), closed: pb(p, "closed").unwrap_or(false) }.normalized();
    let existing = match p.get("layer") {
        Some(v) if !v.is_null() => Some(e.layer_param(p)?.1),
        _ => None,
    };
    let comp_space = ps(p, "space").unwrap_or("comp") == "comp";
    let fill = match p.get("fill") {
        None => Some([0.98, 0.45, 0.09, 1.0]),
        Some(Json::Null) | Some(Json::Bool(false)) => None,
        Some(v) => parse_color(v),
    };
    let stroke = match p.get("stroke") {
        None | Some(Json::Null) | Some(Json::Bool(false)) => None,
        Some(v) => parse_color(v),
    };
    let sw = pf(p, "strokeWidth").unwrap_or(2.0);
    let (lid, mut l) = match existing {
        Some(lid) => {
            let l = e.layer_ref(cid, lid)?.clone();
            if l.kind != LayerKind::Shape {
                return Err(format!("{} is not a shape layer", l.name));
            }
            (Some(lid), l)
        }
        None => {
            let n = e.project.comp(cid).map(|c| c.layers.iter().filter(|l| l.kind == LayerKind::Shape).count()).unwrap_or(0) + 1;
            (None, new_shape_layer(e, cid, &format!("Shape Layer {n}"))?)
        }
    };
    if comp_space {
        let pj = e.project.clone();
        let comp = pj.comp(cid).ok_or("no comp")?;
        let ev = Eval::new(&pj, cid, comp, e.time_of(cid));
        let inv = ev.layer_to_comp(&l).inverse().unwrap_or(Mat3::IDENTITY);
        bp = bp.transformed(&inv);
    }
    let mut next = e.project.next_uid;
    let (group, path_uid) = {
        let mut ids = Ids(&mut next);
        let n = l.contents().map(|c| c.children.len()).unwrap_or(0) + 1;
        let pi = path_item(&mut ids, bp, "Path 1").ok_or("no path")?;
        let path_uid = pi.uid;
        let mut contents = vec![Node::Group(pi)];
        if let Some(s) = stroke {
            contents.extend(stroke_item(&mut ids, s, sw, 0, 0, 4.0, &[], 0.0, 1.0).map(Node::Group));
        }
        if let Some(f) = fill {
            contents.extend(fill_item(&mut ids, f, false, 1.0).map(Node::Group));
        }
        (group_item(&mut ids, &ps(p, "name").map(str::to_string).unwrap_or_else(|| format!("Shape {n}")), contents).ok_or("no group")?, path_uid)
    };
    e.proj().next_uid = next;
    if let Some(c) = l.props.sub_mut("contents") {
        c.children.insert(0, Node::Group(group));
    }
    let id = match lid {
        Some(lid) => {
            *e.layer_mut(cid, lid)? = l;
            lid
        }
        None => add_layer(e, cid, l, None),
    };
    select_only(e, &[id]);
    e.set_label("Pen Tool");
    Ok(json!({"layer": id, "path": path_uid}))
}

fn stroke_uid(e: &Engine, p: &Json) -> Result<(u64, u64, u64), String> {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    let uid = match pu(p, "prop") {
        Some(u) => u,
        None => {
            let mut found = None;
            if let Some(c) = l.contents() {
                fn find(g: &Group, out: &mut Option<u64>) {
                    for c in g.groups() {
                        if out.is_some() {
                            return;
                        }
                        if c.id == "stroke" || c.id == "gradientStroke" {
                            *out = Some(c.uid);
                        } else {
                            find(c, out);
                        }
                    }
                }
                find(c, &mut found);
            }
            found.ok_or("the layer has no stroke (pass `prop`, a stroke uid)")?
        }
    };
    Ok((cid, lid, uid))
}

fn edit_stroke(e: &mut Engine, p: &Json, sub: Option<&str>, f: impl FnOnce(&mut Group) -> Result<(), String>) -> Res {
    let (cid, lid, uid) = stroke_uid(e, p)?;
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no such stroke")?;
    if !matches!(g.id.as_str(), "stroke" | "gradientStroke") || !matches!(g.kind, GroupKind::Plain) {
        return Err("that is not a stroke".into());
    }
    match sub {
        Some(s) => f(g.sub_mut(s).ok_or("this stroke predates taper and wave; add a new stroke")?)?,
        None => f(g)?,
    }
    Ok(json!({"stroke": uid}))
}

fn num(g: &mut Group, p: &Json, key: &str, id: &str) {
    if let Some(x) = pf(p, key) {
        set(g, id, Value::Num(x));
    }
}

pub fn stroke_taper(e: &mut Engine, p: &Json) -> Res {
    let r = edit_stroke(e, p, Some("taper"), |g| {
        if let Some(u) = ps(p, "units") {
            set(g, "units", Value::Enum((u == "percent") as u32));
        }
        for (k, id) in [("startLength", "startLength"), ("endLength", "endLength"), ("startWidth", "startWidth"), ("endWidth", "endWidth"), ("startEase", "startEase"), ("endEase", "endEase")] {
            num(g, p, k, id);
        }
        Ok(())
    })?;
    e.set_label("Stroke Taper");
    Ok(r)
}

pub fn stroke_wave(e: &mut Engine, p: &Json) -> Res {
    let r = edit_stroke(e, p, Some("wave"), |g| {
        if let Some(u) = ps(p, "units") {
            set(g, "units", Value::Enum((u == "cycles") as u32));
        }
        for k in ["amount", "wavelength", "cycles", "phase"] {
            num(g, p, k, k);
        }
        Ok(())
    })?;
    e.set_label("Stroke Wave");
    Ok(r)
}

pub fn dashes_add(e: &mut Engine, p: &Json) -> Res {
    let r = edit_stroke(e, p, None, |g| {
        for (dk, gk) in [("dash", "gap"), ("dash2", "gap2"), ("dash3", "gap3")] {
            let d = g.get(dk).map(|x| x.value.num()).unwrap_or(0.0);
            if d <= 0.0 {
                set(g, dk, Value::Num(10.0));
                set(g, gk, Value::Num(10.0));
                return Ok(());
            }
        }
        Err("a stroke has at most three dash and gap pairs".into())
    })?;
    e.set_label("Add Dash");
    Ok(r)
}

pub fn dashes_remove(e: &mut Engine, p: &Json) -> Res {
    let r = edit_stroke(e, p, None, |g| {
        for (dk, gk) in [("dash3", "gap3"), ("dash2", "gap2"), ("dash", "gap")] {
            let d = g.get(dk).map(|x| x.value.num()).unwrap_or(0.0);
            if d > 0.0 {
                set(g, dk, Value::Num(0.0));
                set(g, gk, Value::Num(0.0));
                return Ok(());
            }
        }
        Ok(())
    })?;
    e.set_label("Remove Dash");
    Ok(r)
}
