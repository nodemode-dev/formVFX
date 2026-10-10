use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use super::comp::make_comp;
use crate::engine::{Engine, Res, pb, ps};
use crate::expr::script::{effect_alias, match_alias, new_uid, shape_kind, text_prop_alias};
use crate::model::key::{Ease, Interp, Key};
use crate::model::prop::{Group, GroupKind, Node, Ui};
use crate::model::value::{CharStyle, Justify, StyleRun, TextDoc};
use crate::model::{Footage, Item, ItemKind, LayerInit, LayerKind, Project, Solid, Value, build_layer};
use crate::render::aep;

pub struct Built {
    pub project: Project,
    pub comps: Vec<(u32, u64, String)>,
    pub layers: HashMap<(u64, u32), u64>,
    pub warnings: Vec<String>,
}

struct B<'a, 'e> {
    e: &'e mut Engine,
    a: &'a aep::Project,
    base: Option<PathBuf>,
    items: HashMap<u32, u64>,
    layers: HashMap<(u64, u32), u64>,
    warnings: Vec<String>,
    names: HashMap<String, String>,
    offset: f64,
}

#[derive(Clone, Copy, PartialEq)]
enum Domain {
    Layer,
    Effect,
    Shape,
    Text,
}

pub fn build(a: &aep::Project, base: Option<&Path>) -> Result<Built, String> {
    let mut names = HashMap::new();
    for (_, defs) in a.defs.values() {
        for d in defs {
            if !d.name.is_empty() {
                names.insert(d.mn.clone(), d.name.clone());
            }
        }
    }
    let mut side = Engine::new();
    let mut b = B { e: &mut side, a, base: base.map(Path::to_path_buf), items: HashMap::new(), layers: HashMap::new(), warnings: a.warnings.clone(), names, offset: 0.0 };
    b.e.batch_depth += 1;
    for it in &a.items {
        b.item(it)?;
    }
    for it in &a.items {
        if let aep::Kind::Comp(c) = &it.kind {
            let cid = b.items[&it.id];
            b.comp_layers(cid, c)?;
        }
    }
    b.e.batch_depth -= 1;
    let comps = a.items.iter().filter(|i| matches!(i.kind, aep::Kind::Comp(_))).map(|i| (i.id, b.items[&i.id], i.name.clone())).collect();
    let mut warnings = b.warnings;
    warnings.sort();
    warnings.dedup();
    let layers = b.layers;
    Ok(Built { project: (*side.project).clone(), comps, layers, warnings })
}

fn split_font(ps_name: &str) -> (String, String) {
    let (fam, style) = ps_name.split_once('-').unwrap_or((ps_name, "Regular"));
    let mut out = String::new();
    for (i, ch) in style.chars().enumerate() {
        if i > 0 && ch.is_ascii_uppercase() && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push(ch);
    }
    let style = if out.is_empty() || out == "Roman" { "Regular".to_string() } else { out };
    (fam.to_string(), style)
}

fn text_doc(d: &aep::TextDoc) -> TextDoc {
    let mut t = TextDoc { text: d.text.replace('\r', "\n"), ..TextDoc::default() };
    t.justify = match d.justify {
        1 => Justify::Right,
        2 => Justify::Center,
        3 => Justify::JustifyLeft,
        4 => Justify::JustifyRight,
        5 => Justify::JustifyCenter,
        6 => Justify::JustifyAll,
        _ => Justify::Left,
    };
    if let Some(r) = d.runs.first() {
        let (fam, style) = split_font(&r.font);
        t.font = fam;
        t.style = style;
        t.size = r.size;
        t.faux_bold = r.faux_bold;
        t.faux_italic = r.faux_italic;
        t.leading = (!r.auto_leading).then_some(r.leading);
        t.h_scale = r.h_scale * 100.0;
        t.v_scale = r.v_scale * 100.0;
        t.tracking = r.tracking;
        t.baseline_shift = r.baseline_shift;
        t.all_caps = r.caps == 2;
        t.small_caps = r.caps == 1;
        if let Some(f) = r.fill {
            t.fill = f;
        }
        if let Some(s) = r.stroke {
            t.stroke = s;
        }
        t.apply_fill = r.apply_fill && r.fill.is_some();
        t.apply_stroke = r.apply_stroke;
        t.stroke_over_fill = !r.fill_first;
        t.stroke_width = if r.apply_stroke { r.stroke_width } else { 0.0 };
    }
    if let Some(bx) = d.box_size {
        t.box_size = Some([0.0, 0.0, bx[0], bx[1]]);
    }
    let base = d.runs.first();
    let total = t.text.chars().count();
    let mut at = 0;
    for r in &d.runs {
        let end = (at + r.len).min(total);
        if let Some(b0) = base
            && at > 0
            && end > at
        {
            let mut cs = CharStyle::default();
            if r.font != b0.font {
                let (fam, style) = split_font(&r.font);
                cs.font = Some(fam);
                cs.style = Some(style);
            }
            if r.size != b0.size {
                cs.size = Some(r.size);
            }
            if r.fill != b0.fill {
                cs.fill = r.fill;
            }
            if r.tracking != b0.tracking {
                cs.tracking = Some(r.tracking);
            }
            if cs != CharStyle::default() {
                t.runs.push(StyleRun { start: at, end, style: cs });
            }
        }
        at = end;
    }
    t
}

fn interp(code: u8) -> Interp {
    match code {
        2 => Interp::Bezier,
        3 => Interp::Hold,
        _ => Interp::Linear,
    }
}

fn percent_like(ui: &Ui) -> bool {
    match ui {
        Ui::Percent => true,
        Ui::Slider { max, slider_max, .. } => (*max - 100.0).abs() < 1e-9 || (*slider_max - 100.0).abs() < 1e-9,
        _ => false,
    }
}

const FRACTION: [&str; 9] = ["ADBE Opacity", "ADBE Scale", "ADBE Text Opacity", "ADBE Text Scale 3D", "ADBE Text Fill Opacity", "ADBE Text Stroke Opacity", "ADBE Mask Opacity", "ADBE Vector Repeater Opacity 1", "ADBE Vector Repeater Opacity 2"];

fn factor(src: &aep::Prop, ui: &Ui, domain: Domain) -> f64 {
    if FRACTION.contains(&src.mn.as_str()) && domain != Domain::Effect {
        return 100.0;
    }
    if domain == Domain::Effect && percent_like(ui) {
        match src.max {
            Some(m) if (m - 255.0).abs() < 1e-9 => return 100.0 / 255.0,
            Some(m) if (m - 1.0).abs() < 1e-9 => return 100.0,
            _ => {}
        }
    }
    1.0
}

fn rewrite_expr(src: &str, names: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(i) = rest.find("\"ADBE ") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('"') {
            Some(j) => {
                let mn = &after[..j];
                match names.get(mn) {
                    Some(n) => {
                        out.push('"');
                        out.push_str(n);
                        out.push('"');
                    }
                    None => {
                        out.push('"');
                        out.push_str(mn);
                        out.push('"');
                    }
                }
                rest = &after[j + 1..];
            }
            None => {
                out.push_str(&rest[i..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn candidates(mn: &str, display: Option<&str>) -> Vec<String> {
    let mut out = vec![];
    let alias = match_alias(mn);
    if alias != mn {
        out.push(alias.to_string());
    }
    if let Some(d) = display.filter(|d| !d.is_empty()) {
        out.push(d.to_string());
    }
    let words: Vec<&str> = mn.strip_prefix("ADBE ").unwrap_or(mn).split(' ').collect();
    for i in 0..words.len() {
        out.push(words[i..].join(" "));
    }
    out
}

fn child_uid(g: &Group, mn: &str, display: Option<&str>) -> Option<(u64, bool)> {
    for c in candidates(mn, display) {
        if let Some(n) = g.child(&c) {
            return Some(match n {
                Node::Prop(p) => (p.uid, true),
                Node::Group(gr) => (gr.uid, false),
            });
        }
    }
    None
}

impl B<'_, '_> {
    fn item(&mut self, it: &aep::Item) -> Result<(), String> {
        let parent = it.parent.and_then(|p| self.items.get(&p).copied());
        let id = match &it.kind {
            aep::Kind::Folder => self.push(it, parent, ItemKind::Folder),
            aep::Kind::Solid { width, height, color } => self.push(it, parent, ItemKind::Solid(Solid { color: *color, width: *width, height: *height, pixel_aspect: 1.0 })),
            aep::Kind::Footage { path, width, height } => {
                let found = self.locate(path);
                let f = match found.as_deref().map(|p| super::formats::probe(p, &json!({}), &mut self.warnings)) {
                    Some(Ok((f, _))) => f,
                    _ => {
                        self.warnings.push(format!("footage `{}` was not found; a placeholder stands in", it.name));
                        Footage { path: String::new(), width: (*width).max(1), height: (*height).max(1), missing: true, has_video: true, frame_rate: 30.0, duration: 10.0, ..Footage::default() }
                    }
                };
                self.push(it, parent, ItemKind::Footage(f))
            }
            aep::Kind::Comp(c) => {
                let bg = [c.background[0] as f32, c.background[1] as f32, c.background[2] as f32];
                let id = make_comp(&mut self.e, &it.name, c.width, c.height, c.fps, c.duration, bg);
                if let Some(i) = self.e.proj().item_mut(id) {
                    i.parent = parent;
                    i.name = it.name.clone();
                    if let ItemKind::Comp(cc) = &mut i.kind {
                        cc.pixel_aspect = c.pixel_aspect;
                        cc.work_area = c.work;
                        cc.motion_blur = c.motion_blur;
                        if let Some(n) = &c.template_name {
                            cc.template_name = n.clone();
                        }
                    }
                }
                id
            }
            aep::Kind::Other => {
                self.warnings.push(format!("project item `{}` is of a kind formvfx does not read yet", it.name));
                return Ok(());
            }
        };
        self.items.insert(it.id, id);
        Ok(())
    }

    fn push(&mut self, it: &aep::Item, parent: Option<u64>, kind: ItemKind) -> u64 {
        let pj = self.e.proj();
        let id = pj.new_id();
        pj.items.push(Item { id, name: it.name.clone(), parent, label: 0, comment: String::new(), kind, proxy: None });
        id
    }

    fn locate(&self, path: &str) -> Option<String> {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
        let name = path.rsplit(['/', '\\']).next()?;
        let base = self.base.as_ref()?;
        [base.join(name), base.join("(Footage)").join(name)].into_iter().find(|p| p.exists()).map(|p| p.to_string_lossy().to_string())
    }

    fn comp_layers(&mut self, cid: u64, c: &aep::Comp) -> Result<(), String> {
        let mut order = vec![];
        for ly in &c.layers {
            match self.layer(cid, c, ly) {
                Ok(id) => {
                    self.layers.insert((cid, ly.id), id);
                    order.push(id);
                }
                Err(err) => self.warnings.push(format!("layer `{}` was not imported: {err}", ly.name)),
            }
        }
        if let Some(comp) = self.e.proj().comp_mut(cid) {
            comp.layers.sort_by_key(|l| order.iter().position(|o| *o == l.id).unwrap_or(usize::MAX));
        }
        for ly in &c.layers {
            let (Some(&lid), Some(par)) = (self.layers.get(&(cid, ly.id)), ly.parent) else { continue };
            let pid = self.layers.get(&(cid, par)).copied();
            if let Ok(l) = self.e.layer_mut(cid, lid) {
                l.parent = pid;
            }
        }
        for ly in &c.layers {
            if let Some(&lid) = self.layers.get(&(cid, ly.id)) {
                self.props(cid, lid, ly);
            }
        }
        Ok(())
    }

    fn layer(&mut self, cid: u64, c: &aep::Comp, ly: &aep::Layer) -> Result<u64, String> {
        let src = ly.source.and_then(|s| self.items.get(&s).copied());
        let src_item = ly.source.and_then(|s| self.a.item(s));
        let mut name = ly.name.clone();
        if name.is_empty() {
            name = src_item.map(|i| i.name.clone()).unwrap_or_else(|| "Layer".into());
        }
        let dur = c.duration;
        let id = match ly.kind {
            2 | 1 => {
                let cmd = if ly.kind == 2 { "layer.newCamera" } else { "layer.newLight" };
                let r = self.e.execute(cmd, json!({"comp": cid, "name": name}))?;
                r.get("layer").and_then(Json::as_u64).ok_or("the camera or light was not created")?
            }
            _ => {
                let (kind, source, size) = match ly.kind {
                    3 => (LayerKind::Text, None, (c.width as f64, c.height as f64)),
                    4 => (LayerKind::Shape, None, (c.width as f64, c.height as f64)),
                    _ if ly.null => (LayerKind::Null, None, (100.0, 100.0)),
                    _ => match src_item.map(|i| &i.kind) {
                        Some(aep::Kind::Solid { width, height, .. }) => (LayerKind::Solid, src, (*width as f64, *height as f64)),
                        Some(aep::Kind::Comp(sc)) => (LayerKind::Precomp, src, (sc.width as f64, sc.height as f64)),
                        Some(aep::Kind::Footage { .. }) => {
                            let dims = src.and_then(|s| self.e.project.item(s)).and_then(|i| i.dimensions()).unwrap_or((c.width, c.height));
                            (LayerKind::Footage, src, (dims.0 as f64, dims.1 as f64))
                        }
                        _ => return Err("its source is not in the project".into()),
                    },
                };
                let l = build_layer(self.e.proj(), (c.width, c.height), LayerInit { kind, name: name.clone(), source, size, start: 0.0, duration: dur });
                let id = l.id;
                if let Some(comp) = self.e.proj().comp_mut(cid) {
                    comp.layers.push(l);
                }
                id
            }
        };
        let l = self.e.layer_mut(cid, id)?;
        l.name = name;
        l.start_time = ly.start;
        l.in_point = ly.in_point;
        l.out_point = ly.out_point.max(ly.in_point);
        l.stretch = if ly.stretch.abs() > 1e-9 { ly.stretch } else { 100.0 };
        l.label = ly.label;
        let s = &mut l.switches;
        s.video = ly.video;
        s.audio = ly.audio;
        s.effects = ly.effects;
        s.motion_blur = ly.motion_blur;
        s.collapse = ly.collapse;
        s.locked = ly.locked;
        s.shy = ly.shy;
        s.solo = ly.solo;
        s.adjustment = ly.adjustment;
        if !matches!(l.kind, LayerKind::Camera | LayerKind::Light) {
            s.three_d = ly.three_d;
        }
        Ok(id)
    }

    fn props(&mut self, cid: u64, lid: u64, ly: &aep::Layer) {
        for n in &ly.props.kids {
            let aep::Node::Group(g) = n else {
                if let aep::Node::Prop(p) = n
                    && p.mn == "ADBE Time Remapping"
                    && (!p.keys.is_empty() || p.expr.is_some())
                {
                    self.warnings.push(format!("time remapping on `{}` is not imported yet", ly.name));
                }
                continue;
            };
            match g.mn.as_str() {
                "ADBE Transform Group" => {
                    if let Some(u) = self.sub_uid(cid, lid, "transform") {
                        self.fill(cid, lid, u, g, Domain::Layer);
                    }
                }
                "ADBE Effect Parade" => self.effects(cid, lid, g),
                "ADBE Text Properties" => self.text(cid, lid, g),
                "ADBE Root Vectors Group" => {
                    if let Some(u) = self.sub_uid(cid, lid, "contents") {
                        self.shapes(cid, lid, u, g, true);
                    }
                }
                "ADBE Mask Parade" => {
                    if !g.kids.is_empty() {
                        self.warnings.push(format!("masks on `{}` are not imported yet", ly.name));
                    }
                }
                "ADBE Camera Options Group" | "ADBE Light Options Group" | "ADBE Material Options Group" | "ADBE Audio Group" => {
                    let id = match_alias(&g.mn).to_string();
                    if let Some(u) = self.sub_uid(cid, lid, &id) {
                        self.fill(cid, lid, u, g, Domain::Layer);
                    }
                }
                _ => {}
            }
        }
    }

    fn sub_uid(&mut self, cid: u64, lid: u64, id: &str) -> Option<u64> {
        self.e.layer_mut(cid, lid).ok()?.props.sub(id).map(|g| g.uid)
    }

    fn group_of(&mut self, cid: u64, lid: u64, uid: u64) -> Option<Group> {
        self.e.layer_mut(cid, lid).ok()?.props.find_group(uid).cloned()
    }

    fn fill(&mut self, cid: u64, lid: u64, uid: u64, src: &aep::Group, domain: Domain) {
        for n in &src.kids {
            let Some(target) = self.group_of(cid, lid, uid) else { return };
            match n {
                aep::Node::Prop(p) => {
                    if p.hidden && p.keys.is_empty() && p.expr.is_none() {
                        continue;
                    }
                    let display = if domain == Domain::Effect { self.names.get(&p.mn).cloned() } else { p.name.clone() };
                    match child_uid(&target, &p.mn, display.as_deref()) {
                        Some((pu, true)) => self.write(cid, lid, pu, p, domain),
                        _ => {
                            if p.expr.is_some() || !p.keys.is_empty() {
                                self.warnings.push(format!("`{}` has no counterpart in formvfx yet", display.unwrap_or_else(|| p.mn.clone())));
                            }
                        }
                    }
                }
                aep::Node::Group(g) => {
                    if g.hidden {
                        continue;
                    }
                    if let Some((gu, false)) = child_uid(&target, &g.mn, g.name.as_deref()) {
                        self.fill(cid, lid, gu, g, domain);
                    }
                }
            }
        }
    }

    fn write(&mut self, cid: u64, lid: u64, uid: u64, src: &aep::Prop, domain: Domain) {
        let layers = self.layers.clone();
        let names = self.names.clone();
        let Ok(l) = self.e.layer_mut(cid, lid) else { return };
        let Some(p) = l.props.find_mut(uid) else { return };
        let f = factor(src, &p.ui, domain);
        let popup = matches!(p.ui, Ui::Popup { .. });
        let conv = |v: &[f64], cur: &Value| -> Option<Value> {
            let g = |i: usize| v.get(i).copied().unwrap_or(0.0) * f;
            Some(match cur {
                Value::Num(_) => Value::Num(g(0)),
                Value::Vec2(_) => Value::Vec2([g(0), g(1)]),
                Value::Vec3(_) => Value::Vec3([g(0), g(1), if v.len() > 2 { g(2) } else { 0.0 }]),
                Value::Color(_) if v.len() >= 4 => {
                    let k = if v.iter().any(|x| *x > 1.0) { 255.0 } else { 1.0 };
                    Value::Color([v[1] / k, v[2] / k, v[3] / k, v[0] / k])
                }
                Value::Bool(_) => Value::Bool(v.first().copied().unwrap_or(0.0) != 0.0),
                Value::Enum(_) => Value::Enum(if popup { (v.first().copied().unwrap_or(1.0) - 1.0).max(0.0) as u32 } else { v.first().copied().unwrap_or(0.0).max(0.0) as u32 }),
                _ => return None,
            })
        };
        if let Value::Layer(_) = p.value {
            let r = src.layer_ref.filter(|r| *r != 0).and_then(|r| layers.get(&(cid, r)).copied());
            p.value = Value::Layer(r);
        } else if let Some(doc) = &src.text {
            p.value = Value::Text(Box::new(text_doc(doc)));
        } else if !src.value.is_empty()
            && let Some(v) = conv(&src.value, &p.value)
        {
            p.value = p.ui.clamp(v);
        }
        if !src.keys.is_empty() {
            let mut keys = vec![];
            for k in &src.keys {
                let value = if let Some(doc) = &k.text {
                    Value::Text(Box::new(text_doc(doc)))
                } else {
                    match conv(&k.value, &p.value) {
                        Some(v) => v,
                        None => continue,
                    }
                };
                let ease = |list: &[(f64, f64)]| list.iter().map(|(s, i)| Ease { speed: s * f, influence: (i * 100.0).clamp(0.1, 100.0) }).collect::<Vec<_>>();
                let tan = |t: &[f64]| [t.first().copied().unwrap_or(0.0), t.get(1).copied().unwrap_or(0.0), t.get(2).copied().unwrap_or(0.0)];
                let mut nk = Key::new(k.time + self.offset, value, p.spatial);
                nk.in_interp = interp(k.in_interp);
                nk.out_interp = interp(k.out_interp);
                nk.in_ease = ease(&k.in_ease);
                nk.out_ease = ease(&k.out_ease);
                nk.continuous = k.flags & 0x08 != 0;
                nk.auto_bezier = false;
                nk.roving = false;
                if p.spatial {
                    nk.spatial_in = tan(&k.tan_in);
                    nk.spatial_out = tan(&k.tan_out);
                    nk.spatial_auto = false;
                }
                keys.push(nk);
            }
            keys.sort_by(|a, b| a.time.total_cmp(&b.time));
            p.keys = keys;
        }
        if let Some(x) = &src.expr {
            p.expr = Some(crate::model::prop::Expr { text: rewrite_expr(x, &names), enabled: true });
        }
    }

    fn effects(&mut self, cid: u64, lid: u64, g: &aep::Group) {
        for n in &g.kids {
            let aep::Node::Group(fx) = n else { continue };
            let alias = effect_alias(&fx.mn);
            let display = fx.effect_name.clone().unwrap_or_else(|| alias.clone());
            let known = crate::effects::find(&alias).or_else(|| crate::effects::find(&display));
            let Some(spec) = known else {
                self.warnings.push(format!("effect `{display}` ({}) is not in formvfx yet", fx.mn));
                continue;
            };
            let r = match self.e.execute("effect.apply", json!({"comp": cid, "layers": [lid], "effect": spec.id})) {
                Ok(r) => r,
                Err(err) => {
                    self.warnings.push(format!("effect `{display}` could not be applied: {err}"));
                    continue;
                }
            };
            let Some(uid) = new_uid(&r) else { continue };
            let name = fx.name.clone().unwrap_or(display);
            if let Ok(l) = self.e.layer_mut(cid, lid)
                && let Some(gr) = l.props.find_group_mut(uid)
            {
                gr.name = name;
                gr.enabled = !fx.hidden;
            }
            for d in &fx.defs {
                if !d.name.is_empty() {
                    self.names.entry(d.mn.clone()).or_insert_with(|| d.name.clone());
                }
            }
            self.fill(cid, lid, uid, fx, Domain::Effect);
        }
    }

    fn text(&mut self, cid: u64, lid: u64, g: &aep::Group) {
        let Some(tu) = self.sub_uid(cid, lid, "text") else { return };
        for n in &g.kids {
            match n {
                aep::Node::Prop(p) if p.mn == "ADBE Text Document" => {
                    if let Some(target) = self.group_of(cid, lid, tu)
                        && let Some((pu, true)) = child_uid(&target, "ADBE Text Document", None)
                    {
                        self.write(cid, lid, pu, p, Domain::Text);
                    }
                }
                aep::Node::Group(a) if a.mn == "ADBE Text Animators" => {
                    for an in &a.kids {
                        if let aep::Node::Group(anim) = an {
                            self.animator(cid, lid, anim);
                        }
                    }
                }
                aep::Node::Group(other) if other.mn == "ADBE Text More Options" || other.mn == "ADBE Text Path Options" => {
                    if let Some(target) = self.group_of(cid, lid, tu)
                        && let Some((gu, false)) = child_uid(&target, &other.mn, None)
                    {
                        self.fill(cid, lid, gu, other, Domain::Text);
                    }
                }
                _ => {}
            }
        }
    }

    fn animator(&mut self, cid: u64, lid: u64, anim: &aep::Group) {
        let mut kinds: Vec<String> = vec![];
        if let Some(props) = anim.group("ADBE Text Animator Properties") {
            for n in &props.kids {
                if let aep::Node::Prop(p) = n
                    && !p.hidden
                {
                    let k = text_prop_alias(&p.mn);
                    if !kinds.contains(&k) {
                        kinds.push(k);
                    }
                }
            }
        }
        if kinds.is_empty() {
            kinds.push("opacity".into());
        }
        let r = match self.e.execute("layer.addTextAnimator", json!({"comp": cid, "layer": lid, "properties": kinds})) {
            Ok(r) => r,
            Err(err) => {
                self.warnings.push(format!("a text animator could not be added: {err}"));
                return;
            }
        };
        let Some(au) = r.get("animator").and_then(Json::as_u64).or_else(|| new_uid(&r)) else { return };
        if let Some(name) = &anim.name
            && let Ok(l) = self.e.layer_mut(cid, lid)
            && let Some(gr) = l.props.find_group_mut(au)
        {
            gr.name = name.clone();
        }
        if let Some(props) = anim.group("ADBE Text Animator Properties") {
            if let Some(target) = self.group_of(cid, lid, au)
                && let Some((pu, false)) = child_uid(&target, "properties", Some("Properties"))
            {
                self.fill(cid, lid, pu, props, Domain::Text);
            }
        }
        if let Some(sels) = anim.group("ADBE Text Selectors") {
            for n in &sels.kids {
                let aep::Node::Group(sel) = n else { continue };
                let kind = match sel.mn.as_str() {
                    "ADBE Text Wiggly Selector" => "wiggly",
                    "ADBE Text Expressible Selector" => "expression",
                    _ => "range",
                };
                let Ok(r) = self.e.execute("layer.addTextSelector", json!({"comp": cid, "layer": lid, "animator": au, "kind": kind})) else { continue };
                if let Some(su) = r.get("selector").and_then(Json::as_u64).or_else(|| new_uid(&r)) {
                    self.fill(cid, lid, su, sel, Domain::Text);
                }
            }
        }
    }

    fn shapes(&mut self, cid: u64, lid: u64, parent: u64, g: &aep::Group, root: bool) {
        for n in &g.kids {
            let aep::Node::Group(item) = n else { continue };
            let Some(kind) = shape_kind(&item.mn) else {
                if item.mn != "ADBE Vector Transform Group" && item.mn != "ADBE Vector Materials Group" {
                    self.warnings.push(format!("shape item `{}` is not in formvfx yet", item.mn));
                }
                continue;
            };
            let group = if root { Json::Null } else { json!(parent) };
            let r = match self.e.execute("layer.addShapeItem", json!({"comp": cid, "layer": lid, "kind": kind, "group": group})) {
                Ok(r) => r,
                Err(err) => {
                    self.warnings.push(format!("shape item `{}` could not be added: {err}", item.mn));
                    continue;
                }
            };
            let Some(uid) = new_uid(&r) else { continue };
            if let Some(name) = &item.name
                && let Ok(l) = self.e.layer_mut(cid, lid)
                && let Some(gr) = l.props.find_group_mut(uid)
            {
                gr.name = name.clone();
            }
            if kind == "group" {
                if let Some(contents) = item.group("ADBE Vectors Group") {
                    self.shapes(cid, lid, uid, contents, false);
                }
                if let Some(xf) = item.group("ADBE Vector Transform Group")
                    && let Some(target) = self.group_of(cid, lid, uid)
                    && let Some((tu, false)) = child_uid(&target, "ADBE Vector Transform Group", Some("Transform"))
                {
                    self.fill(cid, lid, tu, xf, Domain::Shape);
                }
            } else {
                self.fill(cid, lid, uid, item, Domain::Shape);
            }
        }
    }
}

pub fn essential_from(e: &mut Engine, cid: u64, comp: &aep::Comp, layers: &HashMap<u32, u64>, def: Option<&Json>) -> Vec<String> {
    let mut warnings = vec![];
    let mut order: Vec<(String, Option<String>)> = vec![];
    let mut groups: HashMap<String, String> = HashMap::new();
    if let Some(ctrls) = def.and_then(|d| d.get("clientControls")).and_then(Json::as_array) {
        let mut parent_of: HashMap<String, String> = HashMap::new();
        for c in ctrls {
            if c.get("type").and_then(Json::as_u64) == Some(10)
                && let (Some(id), Some(kids)) = (c.get("id").and_then(Json::as_str), c.get("value").and_then(Json::as_array))
            {
                for k in kids.iter().filter_map(Json::as_str) {
                    parent_of.insert(k.to_string(), id.to_string());
                }
            }
        }
        for c in ctrls {
            let Some(id) = c.get("id").and_then(Json::as_str) else { continue };
            if c.get("type").and_then(Json::as_u64) == Some(10) {
                let name = c.get("uiName").and_then(super::essential::localized).unwrap_or_else(|| "Group".into());
                if let Ok(r) = e.execute("essential.addGroup", json!({"comp": cid, "name": name}))
                    && let Some(gid) = r.get("controller").and_then(Json::as_str).or_else(|| r.get("id").and_then(Json::as_str))
                {
                    groups.insert(id.to_string(), gid.to_string());
                }
                continue;
            }
            order.push((id.to_string(), parent_of.get(id).cloned()));
        }
    } else {
        order = comp.controls.iter().map(|c| (c.id.clone(), None)).collect();
    }
    for (id, parent) in order {
        let Some(ctl) = comp.controls.iter().find(|c| c.id == id) else { continue };
        let Some(&lid) = layers.get(&ctl.layer) else {
            warnings.push(format!("control `{}` points at a layer that was not imported", ctl.name));
            continue;
        };
        let Some(uid) = resolve_path(e, cid, lid, &ctl.path) else {
            warnings.push(format!("control `{}` could not be linked to its property", ctl.name));
            continue;
        };
        let mut params = json!({"comp": cid, "layer": lid, "prop": uid, "name": ctl.name});
        if let Some(g) = parent.and_then(|p| groups.get(&p)) {
            params["group"] = json!(g);
        }
        if let Err(err) = e.execute("essential.addProperty", params) {
            warnings.push(format!("control `{}` was not added: {err}", ctl.name));
        }
    }
    warnings
}

fn resolve_path(e: &mut Engine, cid: u64, lid: u64, path: &[(String, i64)]) -> Option<u64> {
    let l = e.layer_mut(cid, lid).ok()?;
    let mut g: &Group = &l.props;
    let mut last: Option<u64> = None;
    for (i, (mn, idx)) in path.iter().enumerate() {
        let param = mn.rsplit_once('-').is_some_and(|(_, t)| t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()));
        let node = if param && matches!(g.kind, GroupKind::Effect { .. }) {
            let n = if *idx >= 1 { *idx as usize } else { mn.rsplit_once('-')?.1.parse().ok()? };
            g.children.get(n.checked_sub(1)?)
        } else if (g.id == "effects" || matches!(g.kind, GroupKind::Indexed)) && *idx >= 0 {
            g.children.get(*idx as usize)
        } else {
            candidates(mn, None).iter().find_map(|c| g.child(c))
        }?;
        match node {
            Node::Group(gr) => {
                g = gr;
                last = Some(gr.uid);
            }
            Node::Prop(p) => {
                return (i + 1 == path.len()).then_some(p.uid);
            }
        }
    }
    last
}

fn bring_in(e: &mut Engine, project: &Project, proj_path: &Path, name: &str) -> Result<u64, String> {
    std::fs::write(proj_path, serde_json::to_string(project).map_err(|x| x.to_string())?).map_err(|x| format!("{}: {x}", proj_path.display()))?;
    let folder = super::file::import_project(e, &proj_path.to_string_lossy(), None);
    let _ = std::fs::remove_file(proj_path);
    let folder = folder?;
    if let Some(f) = e.proj().items.iter_mut().find(|i| i.id == folder) {
        f.name = name.to_string();
    }
    Ok(folder)
}

fn in_folder(e: &Engine, id: u64, folder: u64) -> bool {
    let mut cur = e.project.item(id).and_then(|x| x.parent);
    let mut hops = 0;
    while let Some(p) = cur {
        if p == folder {
            return true;
        }
        hops += 1;
        if hops > 64 {
            break;
        }
        cur = e.project.item(p).and_then(|x| x.parent);
    }
    false
}

pub fn adobe_mogrt(e: &mut Engine, path: &str) -> Result<Option<(u64, u64, String, Vec<String>)>, String> {
    let bytes = std::fs::read(path).map_err(|x| format!("{path}: {x}"))?;
    let outer = crate::engine::lottie::unzip(&bytes).map_err(|x| format!("{path}: {x}"))?;
    let Some(graphic) = outer.iter().find(|(k, _)| k.to_ascii_lowercase().ends_with(".aegraphic")).map(|(_, v)| v) else { return Ok(None) };
    let def: Option<Json> = outer.get("definition.json").and_then(|b| serde_json::from_slice(b).ok());
    let inner = crate::engine::lottie::unzip(graphic).map_err(|x| format!("{path}: the .aegraphic inside could not be read: {x}"))?;
    let aep_bytes = inner.iter().find(|(k, _)| k.to_ascii_lowercase().ends_with(".aep")).map(|(_, v)| v).ok_or_else(|| format!("{path}: the .aegraphic holds no .aep project"))?;
    let a = aep::read(aep_bytes).map_err(|x| format!("{path}: {x}"))?;
    let mut built = build(&a, Path::new(path).parent())?;
    let capsule = def.as_ref().and_then(|d| d.get("capsuleNameLocalized").and_then(super::essential::localized).or_else(|| d.get("capsuleName").and_then(Json::as_str).map(str::to_string)));
    let primary = a
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            aep::Kind::Comp(c) => Some((i, c)),
            _ => None,
        })
        .max_by_key(|(i, c)| (capsule.as_deref() == Some(i.name.as_str()) || c.template_name.as_deref() == capsule.as_deref(), c.controls.len()))
        .ok_or_else(|| format!("{path}: the project has no composition"))?;
    let (pitem, pcomp) = primary;
    let cid = built.comps.iter().find(|c| c.0 == pitem.id).map(|c| c.1).ok_or("the primary composition was not built")?;
    let lmap: HashMap<u32, u64> = built.layers.iter().filter(|((c, _), _)| *c == cid).map(|((_, a2), l)| (*a2, *l)).collect();
    let mut side = Engine::new();
    side.project = std::sync::Arc::new(built.project.clone());
    side.batch_depth += 1;
    let w = essential_from(&mut side, cid, pcomp, &lmap, def.as_ref());
    side.batch_depth -= 1;
    built.warnings.extend(w);
    let name = capsule.clone().unwrap_or_else(|| pitem.name.clone());
    if let Some(ItemKind::Comp(c)) = std::sync::Arc::make_mut(&mut side.project).item_mut(cid).map(|i| &mut i.kind) {
        c.template_name = name.clone();
    }
    built.project = (*side.project).clone();
    let dir = std::env::temp_dir().join(format!("formvfx-mogrt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|x| format!("{}: {x}", dir.display()))?;
    let folder = bring_in(e, &built.project, &dir.join("template.fvxproj"), &name)?;
    let comp = e
        .project
        .items
        .iter()
        .filter(|i| in_folder(e, i.id, folder))
        .find(|i| matches!(&i.kind, ItemKind::Comp(c) if !c.essential.is_empty()) || i.name == pitem.name)
        .map(|i| i.id)
        .ok_or("the template's primary composition was not found after import")?;
    Ok(Some((comp, folder, name, built.warnings)))
}

pub fn apply_ffx(e: &mut Engine, path: &str, p: &Json) -> Res {
    let bytes = std::fs::read(path).map_err(|x| format!("{path}: {x}"))?;
    let (groups, proj) = aep::read_preset(&bytes).map_err(|x| format!("{path}: {x}"))?;
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("select a layer to apply the preset to".into());
    }
    let now = e.time_of(cid);
    let mut names = HashMap::new();
    for (_, defs) in proj.defs.values() {
        for d in defs {
            if !d.name.is_empty() {
                names.insert(d.mn.clone(), d.name.clone());
            }
        }
    }
    let mut warnings = proj.warnings.clone();
    for lid in &ids {
        let lt = e.project.comp(cid).and_then(|c| c.layer(*lid)).map(|l| l.layer_time(now)).unwrap_or(0.0);
        let mut b = B { e: &mut *e, a: &proj, base: None, items: HashMap::new(), layers: HashMap::new(), warnings: vec![], names: names.clone(), offset: lt };
        let ly = aep::Layer { props: aep::Group { kids: groups.iter().cloned().map(aep::Node::Group).collect(), ..aep::Group::default() }, ..aep::Layer::default() };
        b.props(cid, *lid, &ly);
        warnings.extend(b.warnings);
    }
    warnings.sort();
    warnings.dedup();
    e.set_label("Apply Animation Preset");
    Ok(json!({"layers": ids, "groups": groups.iter().map(|g| g.mn.clone()).collect::<Vec<_>>(), "warnings": warnings}))
}

pub fn import(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (an After Effects project, .aep)")?.to_string();
    let bytes = std::fs::read(&path).map_err(|x| format!("{path}: {x}"))?;
    let a = aep::read(&bytes).map_err(|x| format!("{path}: {x}"))?;
    let base = Path::new(&path).parent();
    let built = build(&a, base)?;
    let name = Path::new(&path).file_stem().and_then(|n| n.to_str()).unwrap_or("Imported").to_string();
    let dir = std::env::temp_dir().join(format!("formvfx-aep-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|x| format!("{}: {x}", dir.display()))?;
    let proj_path = dir.join(format!("{name}.fvxproj"));
    let mut built = built;
    let mut comps = built.comps.clone();
    for (aid, cid, _) in comps.iter_mut() {
        let lmap: HashMap<u32, u64> = built.layers.iter().filter(|((c, _), _)| c == cid).map(|((_, a2), l)| (*a2, *l)).collect();
        if let Some(aep::Kind::Comp(ac)) = a.item(*aid).map(|i| &i.kind)
            && !ac.controls.is_empty()
        {
            let mut side = Engine::new();
            side.project = std::sync::Arc::new(built.project.clone());
            side.batch_depth += 1;
            let w = essential_from(&mut side, *cid, ac, &lmap, None);
            side.batch_depth -= 1;
            built.warnings.extend(w);
            built.project = (*side.project).clone();
        }
    }
    let folder = bring_in(e, &built.project, &proj_path, &format!("{name}.aep"))?;
    e.set_label("Import After Effects Project");
    let made: Vec<Json> = e
        .project
        .items
        .iter()
        .filter(|i| matches!(i.kind, ItemKind::Comp(_)))
        .filter(|i| in_folder(e, i.id, folder))
        .map(|i| json!({"id": i.id, "name": i.name}))
        .collect();
    if pb(p, "open").unwrap_or(false)
        && let Some(first) = made.first().and_then(|m| m["id"].as_u64())
    {
        let _ = e.execute("comp.open", json!({"comp": first}));
    }
    Ok(json!({"folder": folder, "comps": made, "warnings": built.warnings}))
}
