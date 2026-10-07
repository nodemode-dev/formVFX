use std::sync::OnceLock;

use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, TextEdit, pb, pf, ps};
use crate::model::key::{EASY_INFLUENCE, ease_dims, insert};
use crate::model::prop::{self, Expr, TEXT_ANIMATOR_KINDS, text_anim_props, text_kind_alias, text_selector};
use crate::model::{Ease, Group, Ids, Interp, Key, Layer, LayerKind, Node, Prop, Value};
use crate::render::layout::font_axes;
use crate::render::textanim::ensure_text_options;

static LIBRARY: OnceLock<Json> = OnceLock::new();

pub fn library() -> &'static Json {
    LIBRARY.get_or_init(|| serde_json::from_str(include_str!("../../../../../presets/text-animators.json")).unwrap_or(json!({"presets": []})))
}

fn find_preset(name: &str) -> Option<Json> {
    let n = name.to_ascii_lowercase();
    library()["presets"].as_array()?.iter().find(|p| p["id"].as_str().is_some_and(|s| s.to_ascii_lowercase() == n) || p["name"].as_str().is_some_and(|s| s.to_ascii_lowercase() == n)).cloned()
}

fn text_layer(e: &Engine, p: &Json) -> Result<(u64, u64), String> {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    if l.kind != LayerKind::Text {
        return Err(format!("{} is not a text layer", l.name));
    }
    Ok((cid, lid))
}

fn with_layer<R>(e: &mut Engine, cid: u64, lid: u64, f: impl FnOnce(&mut Layer, &mut Ids) -> Result<R, String>) -> Result<R, String> {
    let mut l = e.layer_ref(cid, lid)?.clone();
    let mut next = e.project.next_uid;
    let r = {
        let mut ids = Ids(&mut next);
        ensure_text_options(&mut l, &mut ids);
        f(&mut l, &mut ids)?
    };
    let pj = e.proj();
    pj.next_uid = next;
    if let Some(slot) = pj.comp_mut(cid).and_then(|c| c.layer_mut(lid)) {
        *slot = l;
    }
    Ok(r)
}

fn animators(l: &mut Layer) -> &mut Group {
    l.props.sub_mut("text").and_then(|t| t.sub_mut("animators")).expect("animators")
}

fn per_char_3d(l: &Layer) -> bool {
    l.props.sub("text").and_then(|t| t.get("perChar3d")).is_some_and(|p| p.value.boolean())
}

fn pick_animator(l: &mut Layer, sel: Option<&Json>) -> Option<usize> {
    let a = animators(l);
    let n = a.children.len();
    if n == 0 {
        return None;
    }
    match sel {
        Some(Json::Number(x)) => {
            let v = x.as_u64().unwrap_or(0);
            if let Some(i) = a.children.iter().position(|c| c.uid() == v) {
                return Some(i);
            }
            (v >= 1 && (v as usize) <= n).then(|| v as usize - 1)
        }
        Some(Json::String(s)) => a.children.iter().position(|c| c.name() == s).or_else(|| s.trim_start_matches('#').parse::<usize>().ok().filter(|i| *i >= 1 && *i <= n).map(|i| i - 1)),
        _ => Some(n - 1),
    }
}

fn kinds(p: &Json) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    if let Some(a) = p.get("properties").and_then(Json::as_array) {
        out.extend(a.iter().filter_map(Json::as_str).map(|s| text_kind_alias(s).to_string()));
    }
    for k in ["property", "kind"] {
        if let Some(s) = ps(p, k) {
            out.push(text_kind_alias(s).to_string());
        }
    }
    out
}

fn known(kind: &str) -> bool {
    TEXT_ANIMATOR_KINDS.iter().any(|k| k.0 == kind) || matches!(kind, "rotationX" | "rotationY")
}

fn push_props(group: &mut Group, props: Vec<Prop>) -> Vec<u64> {
    let mut added = vec![];
    for pr in props {
        if group.get(&pr.id).is_none() {
            added.push(pr.uid);
            group.children.push(Node::Prop(pr));
        }
    }
    added
}

fn new_animator(l: &mut Layer, ids: &mut Ids, kinds: &[String], name: Option<&str>, selector: bool) -> (u64, usize) {
    let three = per_char_3d(l);
    let mut props: Vec<Prop> = vec![];
    for k in kinds {
        for pr in text_anim_props(ids, k, three) {
            if !props.iter().any(|x| x.id == pr.id) {
                props.push(pr);
            }
        }
    }
    let a = animators(l);
    let name = name.map(str::to_string).unwrap_or_else(|| format!("Animator {}", a.children.len() + 1));
    let sel = selector.then(|| prop::range_selector(ids, "Range Selector 1"));
    let g = prop::text_animator(ids, &name, props, sel);
    let uid = g.uid;
    let a = animators(l);
    a.children.push(Node::Group(g));
    (uid, a.children.len())
}

pub fn add_animator(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = text_layer(e, p)?;
    let ks = kinds(p);
    if ks.is_empty() {
        return Err("pass `properties` (like [\"position\", \"opacity\"]) or `property`".into());
    }
    if let Some(bad) = ks.iter().find(|k| !known(k)) {
        return Err(format!("unknown text animator property `{bad}` (text.animatorProperties lists them)"));
    }
    let name = ps(p, "name").map(str::to_string);
    let (uid, index) = with_layer(e, cid, lid, |l, ids| Ok(new_animator(l, ids, &ks, name.as_deref(), true)))?;
    e.set_label("Animate Text");
    Ok(json!({"layer": lid, "animator": uid, "index": index}))
}

pub fn add_property(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = text_layer(e, p)?;
    let ks = kinds(p);
    if ks.is_empty() {
        return Err("pass `property`".into());
    }
    if let Some(bad) = ks.iter().find(|k| !known(k)) {
        return Err(format!("unknown text animator property `{bad}`"));
    }
    let which = p.get("animator").cloned();
    let added = with_layer(e, cid, lid, |l, ids| {
        let three = per_char_3d(l);
        let Some(i) = pick_animator(l, which.as_ref()) else {
            let (uid, _) = new_animator(l, ids, &ks, None, true);
            return Ok(json!({"animator": uid, "created": true}));
        };
        let props: Vec<Prop> = ks.iter().flat_map(|k| text_anim_props(ids, k, three)).collect();
        let Node::Group(an) = &mut animators(l).children[i] else { return Err("not an animator".into()) };
        let uid = an.uid;
        let pg = an.sub_mut("properties").ok_or("animator has no properties group")?;
        let added = push_props(pg, props);
        Ok(json!({"animator": uid, "added": added}))
    })?;
    e.set_label("Add Property");
    Ok(added)
}

pub fn add_selector(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = text_layer(e, p)?;
    let kind = ps(p, "kind").unwrap_or("range").to_string();
    let which = p.get("animator").cloned();
    let r = with_layer(e, cid, lid, |l, ids| {
        let Some(i) = pick_animator(l, which.as_ref()) else { return Err("the layer has no text animator (layer.addTextAnimator adds one)".into()) };
        let Node::Group(an) = &mut animators(l).children[i] else { return Err("not an animator".into()) };
        let sels = an.sub_mut("selectors").ok_or("animator has no selectors group")?;
        let label = match kind.as_str() {
            "range" => "Range Selector",
            "wiggly" => "Wiggly Selector",
            "expression" => "Expression Selector",
            _ => return Err(format!("unknown selector kind `{kind}` (range, wiggly or expression)")),
        };
        let n = sels.children.iter().filter(|c| c.name().starts_with(label)).count() + 1;
        let g = text_selector(ids, &kind, &format!("{label} {n}")).ok_or("unknown selector")?;
        let uid = g.uid;
        sels.children.push(Node::Group(g));
        Ok(json!({"selector": uid, "index": sels.children.len()}))
    })?;
    e.set_label("Add Text Selector");
    Ok(r)
}

pub fn enable_per_char_3d(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = text_layer(e, p)?;
    let toggle = pb(p, "toggle").unwrap_or(false);
    let want = pb(p, "enabled");
    let on = with_layer(e, cid, lid, |l, ids| {
        let cur = per_char_3d(l);
        let on = if toggle { !cur } else { want.unwrap_or(true) };
        if let Some(pr) = l.props.sub_mut("text").and_then(|t| t.get_mut("perChar3d")) {
            pr.value = Value::Bool(on);
        }
        if on {
            l.switches.three_d = true;
            let anims = animators(l);
            for c in anims.children.iter_mut() {
                let Node::Group(an) = c else { continue };
                let Some(pg) = an.sub_mut("properties") else { continue };
                if pg.get("rotation").is_some() && pg.get("rotationX").is_none() {
                    let x = ids.prop("rotationX", "X Rotation", Value::Num(0.0), crate::model::Ui::Angle);
                    let y = ids.prop("rotationY", "Y Rotation", Value::Num(0.0), crate::model::Ui::Angle);
                    let at = pg.children.iter().position(|c| c.id() == "rotation").unwrap_or(0);
                    pg.children.insert(at, Node::Prop(y));
                    pg.children.insert(at, Node::Prop(x));
                }
            }
        }
        Ok(on)
    })?;
    e.set_label(if on { "Enable Per-character 3D" } else { "Disable Per-character 3D" });
    Ok(json!({"layer": lid, "perChar3d": on}))
}

pub fn remove_all(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    let mut n = 0;
    for lid in ids {
        if e.layer_ref(cid, lid)?.kind != LayerKind::Text {
            continue;
        }
        n += with_layer(e, cid, lid, |l, _| {
            let a = animators(l);
            let k = a.children.len();
            a.children.clear();
            Ok(k)
        })?;
    }
    e.set_label("Remove All Text Animators");
    Ok(json!({"removed": n}))
}

pub fn animator_properties(_: &mut Engine, _: &Json) -> Res {
    Ok(json!(TEXT_ANIMATOR_KINDS.iter().map(|(k, l)| json!({"property": k, "label": l})).collect::<Vec<_>>()))
}

fn doc_of(e: &Engine, cid: u64, lid: u64) -> Result<crate::model::TextDoc, String> {
    let l = e.layer_ref(cid, lid)?;
    let lt = l.layer_time(e.time_of(cid));
    l.text_prop().map(|p| p.at(lt)).and_then(|v| v.as_text().cloned()).ok_or_else(|| "not a text layer".to_string())
}

pub fn font_features(e: &mut Engine, p: &Json) -> Res {
    let (font, style) = match (ps(p, "font"), ps(p, "style")) {
        (Some(f), s) => (f.to_string(), s.unwrap_or("Regular").to_string()),
        _ => {
            let (cid, lid) = text_layer(e, p)?;
            let d = doc_of(e, cid, lid)?;
            (d.font, d.style)
        }
    };
    let id = crate::render::text::resolve(&font, &style).ok_or("no fonts are installed")?;
    let (family, st) = crate::render::text::face_name(id).unwrap_or((font, style));
    let feats = crate::render::layout::font_features(id);
    let has = |t: &str| feats.iter().any(|x| x == t);
    let sets: Vec<u32> = (1..=20).filter(|n| has(&format!("ss{n:02}"))).collect();
    let axes: Vec<Json> = font_axes(id).into_iter().map(|a| json!({"tag": a.tag, "name": a.name, "min": a.min, "default": a.default, "max": a.max})).collect();
    Ok(json!({
        "family": family, "style": st, "features": feats, "axes": axes,
        "options": {
            "smallCaps": has("smcp"), "allSmallCaps": has("c2sc"), "superscript": has("sups"), "subscript": has("subs"), "stylisticSets": sets,
            "fractions": has("frac"), "ordinals": has("ordn"), "swash": has("swsh"), "titling": has("titl"), "discretionaryLigatures": has("dlig"),
            "contextualAlternates": has("calt"), "stylisticAlternates": has("salt"), "ligatures": has("liga"), "oldStyle": has("onum"), "lining": has("lnum"),
            "tabular": has("tnum"), "proportional": has("pnum"), "kerning": has("kern"),
        }
    }))
}

pub fn animator_font_axes(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = text_layer(e, p)?;
    let d = doc_of(e, cid, lid)?;
    let id = crate::render::text::resolve(&d.font, &d.style).ok_or("no fonts are installed")?;
    let axes = font_axes(id);
    let (family, style) = crate::render::text::face_name(id).unwrap_or((d.font.clone(), d.style.clone()));
    let Some(want) = ps(p, "axis") else {
        return Ok(json!({"family": family, "style": style, "axes": axes.iter().map(|a| json!({"tag": a.tag, "name": a.name, "min": a.min, "default": a.default, "max": a.max})).collect::<Vec<_>>()}));
    };
    if axes.is_empty() {
        return Err(format!("{family} is not a variable font (try a font with axes, such as one ending in Variable)"));
    }
    let ax = axes.iter().find(|a| a.tag == want || a.name.eq_ignore_ascii_case(want)).cloned().ok_or_else(|| format!("{family} has no axis `{want}` ({})", axes.iter().map(|a| a.tag.as_str()).collect::<Vec<_>>().join(", ")))?;
    let which = p.get("animator").cloned();
    let r = with_layer(e, cid, lid, |l, ids| {
        let pr = prop::font_axis_prop(ids, &ax.tag, &ax.name, ax.default, ax.min, ax.max);
        let i = match pick_animator(l, which.as_ref()) {
            Some(i) => i,
            None => {
                new_animator(l, ids, &[], None, true);
                animators(l).children.len() - 1
            }
        };
        let Node::Group(an) = &mut animators(l).children[i] else { return Err("not an animator".into()) };
        let uid = an.uid;
        let pg = an.sub_mut("properties").ok_or("animator has no properties group")?;
        let added = push_props(pg, vec![pr]);
        Ok(json!({"animator": uid, "axis": ax.tag, "added": added}))
    })?;
    e.set_label("Add Variable Font Axis");
    Ok(r)
}

pub fn set_selection(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = text_layer(e, p).or_else(|_| e.state.text_edit.as_ref().map(|t| (0, t.layer)).ok_or("no text is being edited".to_string()).and_then(|(_, l)| Ok((e.comp_id(None)?, l))))?;
    let n = doc_of(e, cid, lid)?.text.chars().count();
    let (a, c) = if ps(p, "select") == Some("all") {
        (0, n)
    } else if let (Some(a), Some(c)) = (pf(p, "anchor"), pf(p, "caret")) {
        (a as usize, c as usize)
    } else {
        let s = pf(p, "start").unwrap_or(0.0) as usize;
        (s, pf(p, "end").map(|x| x as usize).unwrap_or(s))
    };
    e.state.text_edit = Some(TextEdit { layer: lid, anchor: a.min(n), caret: c.min(n) });
    Ok(json!({"layer": lid, "anchor": a.min(n), "caret": c.min(n)}))
}

fn key_at(pr: &mut Prop, t: f64, v: Value, ease: bool) {
    let mut k = Key::new(t, v, pr.spatial);
    if ease {
        let d = ease_dims(&k.value, pr.spatial);
        k.in_interp = Interp::Bezier;
        k.out_interp = Interp::Bezier;
        k.in_ease = vec![Ease { speed: 0.0, influence: EASY_INFLUENCE }; d];
        k.out_ease = k.in_ease.clone();
    }
    insert(&mut pr.keys, k);
}

fn set_path(g: &mut Group, path: &str, val: &Json) -> Result<(), String> {
    let Some(uid) = g.resolve_uid(path).or_else(|| path.strip_prefix("advanced/").and_then(|p| g.resolve_uid(p))) else { return Ok(()) };
    let pr = g.find_mut(uid).ok_or_else(|| format!("`{path}` is a group"))?;
    pr.value = pr.ui.clamp(pr.value.parse_like(val, pr.ui.options())?);
    Ok(())
}

fn key_path(g: &mut Group, path: &str, keys: &Json, t0: f64, ease: bool) -> Result<(), String> {
    let uid = g.resolve_uid(path).ok_or_else(|| format!("no `{path}` in {}", g.name))?;
    let pr = g.find_mut(uid).ok_or_else(|| format!("`{path}` is a group"))?;
    for k in keys.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        let Some(a) = k.as_array() else { continue };
        let t = a.first().and_then(Json::as_f64).unwrap_or(0.0);
        let Some(v) = a.get(1) else { continue };
        let val = pr.ui.clamp(pr.value.parse_like(v, pr.ui.options())?);
        key_at(pr, t0 + t, val, ease);
    }
    Ok(())
}

fn express_path(g: &mut Group, path: &str, text: &str) -> Result<(), String> {
    let uid = g.resolve_uid(path).ok_or_else(|| format!("no `{path}` in {}", g.name))?;
    let pr = g.find_mut(uid).ok_or_else(|| format!("`{path}` is a group"))?;
    pr.expr = Some(Expr { text: text.to_string(), enabled: true });
    Ok(())
}

fn build_from_preset(l: &mut Layer, ids: &mut Ids, preset: &Json, t0: f64, based_override: Option<u32>) -> Result<usize, String> {
    if preset.get("perChar3d").and_then(Json::as_bool) == Some(true) {
        if let Some(pr) = l.props.sub_mut("text").and_then(|t| t.get_mut("perChar3d")) {
            pr.value = Value::Bool(true);
        }
        l.switches.three_d = true;
    }
    let three = per_char_3d(l);
    let specs = preset.get("animators").and_then(Json::as_array).cloned().unwrap_or_default();
    for spec in &specs {
        let props_spec = spec.get("properties").and_then(Json::as_object).cloned().unwrap_or_default();
        let mut props: Vec<Prop> = vec![];
        for (kind, val) in &props_spec {
            let kind = text_kind_alias(kind);
            let made = text_anim_props(ids, kind, three);
            for mut pr in made {
                if props.iter().any(|x| x.id == pr.id) {
                    continue;
                }
                if pr.id == kind {
                    pr.value = pr.ui.clamp(pr.value.parse_like(val, pr.ui.options())?);
                }
                props.push(pr);
            }
        }
        let name = spec.get("name").and_then(Json::as_str).map(str::to_string).unwrap_or_else(|| format!("Animator {}", animators(l).children.len() + 1));
        let mut an = prop::text_animator(ids, &name, props, None);
        let ease = spec.get("ease").and_then(Json::as_bool).unwrap_or(false);
        if let Some(pg) = an.sub_mut("properties") {
            if let Some(keys) = spec.get("keys").and_then(Json::as_object) {
                for (k, v) in keys {
                    key_path(pg, text_kind_alias(k), v, t0, ease)?;
                }
            }
            if let Some(ex) = spec.get("expressions").and_then(Json::as_object) {
                for (k, v) in ex {
                    if let Some(t) = v.as_str() {
                        express_path(pg, text_kind_alias(k), t)?;
                    }
                }
            }
        }
        let sel_specs = spec.get("selectors").and_then(Json::as_array).cloned().unwrap_or_default();
        let mut counts: std::collections::HashMap<String, usize> = Default::default();
        let mut sels: Vec<Group> = vec![];
        for ss in &sel_specs {
            let kind = ss.get("kind").and_then(Json::as_str).unwrap_or("range");
            let label = match kind {
                "wiggly" => "Wiggly Selector",
                "expression" => "Expression Selector",
                _ => "Range Selector",
            };
            let n = counts.entry(label.to_string()).or_default();
            *n += 1;
            let mut g = text_selector(ids, kind, &format!("{label} {n}")).ok_or_else(|| format!("unknown selector kind `{kind}`"))?;
            if let Some(set) = ss.get("set").and_then(Json::as_object) {
                for (path, v) in set {
                    set_path(&mut g, path, v)?;
                }
            }
            if let Some(b) = based_override {
                let path = if kind == "range" { "advanced/basedOn" } else { "basedOn" };
                set_path(&mut g, path, &json!(b))?;
            }
            let sease = ss.get("ease").and_then(Json::as_bool).unwrap_or(false);
            if let Some(keys) = ss.get("keys").and_then(Json::as_object) {
                for (path, v) in keys {
                    key_path(&mut g, path, v, t0, sease)?;
                }
            }
            if let Some(ex) = ss.get("expressions").and_then(Json::as_object) {
                for (path, v) in ex {
                    if let Some(t) = v.as_str() {
                        express_path(&mut g, path, t)?;
                    }
                }
            }
            sels.push(g);
        }
        if let Some(sg) = an.sub_mut("selectors") {
            sg.children = sels.into_iter().map(Node::Group).collect();
        }
        animators(l).children.push(Node::Group(an));
    }
    Ok(specs.len())
}

fn apply_preset(e: &mut Engine, p: &Json, preset: &Json, based: Option<u32>, label: &str) -> Res {
    let (cid, mut layers) = e.layers_param(p)?;
    layers.retain(|id| e.layer_ref(cid, *id).is_ok_and(|l| l.kind == LayerKind::Text));
    if layers.is_empty() {
        return Err("select a text layer (or pass `layers`)".into());
    }
    let now = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let mut added = 0;
    for lid in &layers {
        let t0 = e.layer_ref(cid, *lid)?.layer_time(now);
        added += with_layer(e, cid, *lid, |l, ids| build_from_preset(l, ids, preset, t0, based))?;
    }
    e.set_label(label);
    Ok(json!({"layers": layers, "animators": added, "preset": preset.get("id").cloned().unwrap_or(Json::Null)}))
}

pub fn apply_library_preset(e: &mut Engine, p: &Json) -> Res {
    let preset = match p.get("preset") {
        Some(Json::Object(_)) => p["preset"].clone(),
        Some(Json::String(s)) => find_preset(s).ok_or_else(|| format!("no text preset `{s}` (text.library lists them)"))?,
        _ => return Err("pass `preset` (an id from text.library)".into()),
    };
    apply_preset(e, p, &preset, None, "Apply Text Preset")
}

pub const BUILTIN: [(&str, &str, Option<u32>); 8] = [
    ("typewriter", "typewriterCharacters", None),
    ("fadeUpCharacters", "fadeUpCharacters", None),
    ("bounceInWords", "bounceInCharacters", Some(2)),
    ("trackingIn", "trackingInTight", None),
    ("scramble", "scrambleIn", None),
    ("blurIn", "blurInCharacters", None),
    ("jitter", "jitter", None),
    ("dropInLines", "dropInLines", None),
];

pub fn apply_builtin(e: &mut Engine, p: &Json) -> Res {
    let name = ps(p, "preset").ok_or("pass `preset` (text.presets lists them)")?;
    let (_, id, based) = BUILTIN.iter().find(|b| b.0.eq_ignore_ascii_case(name)).copied().unwrap_or((name, name, None));
    let preset = find_preset(id).ok_or_else(|| format!("no text preset `{name}`"))?;
    apply_preset(e, p, &preset, based, "Apply Text Preset")
}

pub fn presets(_: &mut Engine, _: &Json) -> Res {
    Ok(json!({
        "builtin": BUILTIN.iter().map(|b| b.0).collect::<Vec<_>>(),
        "library": library()["presets"].as_array().map(|a| a.len()).unwrap_or(0),
    }))
}

pub fn library_list(_: &mut Engine, p: &Json) -> Res {
    let q: Vec<String> = ps(p, "query").unwrap_or("").to_ascii_lowercase().split_whitespace().map(str::to_string).collect();
    let cat = ps(p, "category").map(str::to_ascii_lowercase);
    let list: Vec<Json> = library()["presets"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or(&[])
        .iter()
        .filter(|x| cat.as_ref().is_none_or(|c| x["category"].as_str().is_some_and(|s| s.to_ascii_lowercase() == *c)))
        .filter(|x| {
            let hay = format!("{} {} {}", x["name"].as_str().unwrap_or(""), x["category"].as_str().unwrap_or(""), x["description"].as_str().unwrap_or("")).to_ascii_lowercase();
            q.iter().all(|w| hay.contains(w))
        })
        .map(|x| json!({"id": x["id"], "name": x["name"], "category": x["category"], "description": x["description"], "perChar3d": x.get("perChar3d").and_then(Json::as_bool).unwrap_or(false)}))
        .collect();
    Ok(json!(list))
}
