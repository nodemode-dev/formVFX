use serde_json::{Map, Value as Json, json};

use crate::effects::paint::{self, DURATIONS, ERASE_MODES, FOREVER, KINDS, NewStroke, PRESETS, Tip};
use crate::engine::{Engine, Res, pf, ps};
use crate::model::Value;
use crate::model::key::{self, Key};
use crate::model::prop::{Group, GroupKind, Node};
use crate::model::value::parse_color;

fn defaults() -> Json {
    json!({
        "color": [1.0, 1.0, 1.0, 1.0], "background": [0.0, 0.0, 0.0, 1.0], "opacity": 100.0, "flow": 100.0, "mode": 0, "channels": "RGBA",
        "durationMode": "constant", "customFrames": 1, "eraseMode": "layerSourceAndPaint",
        "diameter": 13.0, "angle": 0.0, "roundness": 100.0, "hardness": 100.0, "spacing": 25.0, "preset": 4,
        "sizePressure": false, "minSize": 1.0, "opacityPressure": false, "flowPressure": false,
        "clonePreset": 1, "cloneSource": null, "clonePoint": null, "cloneOffset": null, "aligned": true, "lockSourceTime": false, "cloneTimeShift": 0.0, "cloneTime": 0.0,
        "showOverlay": false, "overlayOpacity": 50.0, "overlayDifference": false,
    })
}

fn current(e: &Engine) -> Json {
    let mut d = defaults();
    if let (Some(o), Some(saved)) = (d.as_object_mut(), e.prefs.get("paintOptions").and_then(Json::as_object)) {
        for (k, v) in saved {
            o.insert(k.clone(), v.clone());
        }
    }
    d
}

fn save(e: &mut Engine, m: Map<String, Json>) {
    let mut cur = e.prefs.get("paintOptions").and_then(Json::as_object).cloned().unwrap_or_default();
    for (k, v) in m {
        cur.insert(k, v);
    }
    e.prefs.insert("paintOptions".into(), Json::Object(cur));
}

pub fn options(e: &mut Engine, p: &Json) -> Res {
    if p.get("reset").and_then(Json::as_bool).unwrap_or(false) {
        e.prefs.remove("paintOptions");
    }
    let known = defaults();
    let mut m = Map::new();
    if let Some(o) = p.as_object() {
        for (k, v) in o {
            if known.get(k).is_some() {
                m.insert(k.clone(), v.clone());
            }
        }
    }
    if !m.is_empty() {
        save(e, m);
    }
    Ok(current(e))
}

pub fn presets(_: &mut Engine, _: &Json) -> Res {
    Ok(json!({"presets": PRESETS.iter().enumerate().map(|(i, p)| json!({"index": i, "name": p.name, "diameter": p.diameter, "hardness": p.hardness})).collect::<Vec<_>>()}))
}

pub fn brush_preset(e: &mut Engine, p: &Json) -> Res {
    let i = match p.get("preset") {
        Some(Json::Number(n)) => n.as_u64().unwrap_or(0) as usize,
        Some(Json::String(s)) => PRESETS.iter().position(|x| x.name.eq_ignore_ascii_case(s)).ok_or_else(|| format!("no brush preset \"{s}\" (paint.presets)"))?,
        _ => 4,
    };
    let pr = PRESETS.get(i).ok_or("no such brush preset")?;
    let mut m = Map::new();
    m.insert("preset".into(), json!(i));
    m.insert("diameter".into(), json!(pr.diameter));
    m.insert("hardness".into(), json!(pr.hardness));
    m.insert("roundness".into(), json!(100.0));
    m.insert("angle".into(), json!(0.0));
    save(e, m);
    Ok(current(e))
}

pub fn set_clone_source(e: &mut Engine, p: &Json) -> Res {
    let (_, lid) = e.layer_param(p)?;
    let pt = p.get("point").and_then(Json::as_array).and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])).ok_or("pass `point`: [x, y] in layer pixels")?;
    let mut m = Map::new();
    m.insert("cloneSource".into(), json!(lid));
    m.insert("clonePoint".into(), json!(pt));
    m.insert("cloneOffset".into(), Json::Null);
    save(e, m);
    Ok(json!({"cloneSource": lid, "clonePoint": pt}))
}

fn paint_effect(e: &mut Engine, cid: u64, lid: u64) -> Result<u64, String> {
    let found = e.layer_ref(cid, lid)?.effects().and_then(|fx| fx.groups().find(|g| matches!(&g.kind, GroupKind::Effect { effect } if effect == paint::ID)).map(|g| g.uid));
    if let Some(u) = found {
        return Ok(u);
    }
    let r = super::effect::apply(e, &json!({"effect": paint::ID, "layers": [lid], "comp": cid}))?;
    r.get("applied").and_then(|a| a.get(0)).and_then(|a| a.get("uid")).and_then(Json::as_u64).ok_or_else(|| "could not add the Paint effect".into())
}

fn f(o: &Json, p: &Json, k: &str, d: f64) -> f64 {
    pf(p, k).or_else(|| o.get(k).and_then(Json::as_f64)).unwrap_or(d)
}

fn b(o: &Json, p: &Json, k: &str) -> bool {
    p.get(k).and_then(Json::as_bool).or_else(|| o.get(k).and_then(Json::as_bool)).unwrap_or(false)
}

fn s<'a>(o: &'a Json, p: &'a Json, k: &str, d: &'a str) -> &'a str {
    p.get(k).and_then(Json::as_str).or_else(|| o.get(k).and_then(Json::as_str)).unwrap_or(d)
}

fn mode_of(v: Option<&Json>) -> u32 {
    match v {
        Some(Json::Number(n)) => n.as_u64().unwrap_or(0) as u32,
        Some(Json::String(s)) => {
            let flat = |x: &str| x.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase();
            paint::MODES.iter().position(|m| flat(m.label()) == flat(s)).unwrap_or(0) as u32
        }
        _ => 0,
    }
}

pub fn stroke(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let o = current(e);
    let kind = KINDS.iter().position(|k| *k == ps(p, "kind").unwrap_or("brush")).ok_or("kind is brush, clone or eraser")?;
    let pts: Vec<[f64; 3]> = p
        .get("points")
        .and_then(Json::as_array)
        .ok_or("pass `points`: [[x, y, pressure?], ...] in layer pixels")?
        .iter()
        .filter_map(|q| {
            let a = q.as_array()?;
            Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2).and_then(Json::as_f64).unwrap_or(1.0)])
        })
        .collect();
    if pts.is_empty() {
        return Err("a stroke needs at least one point".into());
    }
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let fd = comp.frame_duration();
    let ct = match (pf(p, "time"), pf(p, "frame")) {
        (Some(t), _) => t,
        (None, Some(fr)) => fr * fd,
        _ => e.time_of(cid),
    };
    let lt = e.layer_ref(cid, lid)?.layer_time(ct);
    let dur_mode = DURATIONS.iter().position(|d| *d == s(&o, p, "durationMode", "constant")).unwrap_or(0);
    let out = match dur_mode {
        2 => lt + fd,
        3 => lt + fd * f(&o, p, "customFrames", 1.0).max(1.0),
        _ => FOREVER,
    };
    let channels = match s(&o, p, "channels", "RGBA") {
        "RGB" => 1,
        "Alpha" => 2,
        _ => 0,
    };
    let color = p.get("color").and_then(parse_color).or_else(|| o.get("color").and_then(parse_color)).unwrap_or([1.0; 4]);
    let tip = Tip {
        color,
        diameter: f(&o, p, "diameter", 13.0).max(0.1),
        angle: f(&o, p, "angle", 0.0),
        hardness: f(&o, p, "hardness", 100.0).clamp(0.0, 100.0),
        roundness: f(&o, p, "roundness", 100.0).clamp(1.0, 100.0),
        spacing: f(&o, p, "spacing", 25.0).clamp(1.0, 1000.0),
        opacity: f(&o, p, "opacity", 100.0).clamp(0.0, 100.0),
        flow: f(&o, p, "flow", 100.0).clamp(0.0, 100.0),
        channels,
        mode: mode_of(p.get("mode").or_else(|| o.get("mode"))),
    };
    let first = [pts[0][0], pts[0][1]];
    let mut save_offset = None;
    let clone = (kind == 1).then(|| {
        let src = p.get("cloneSource").and_then(Json::as_u64).or_else(|| o.get("cloneSource").and_then(Json::as_u64));
        let pos = match p.get("clonePosition").and_then(Json::as_array).and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])) {
            Some(v) => v,
            None => {
                let off = o.get("cloneOffset").and_then(Json::as_array).and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]));
                let point = o.get("clonePoint").and_then(Json::as_array).and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])).unwrap_or(first);
                match off {
                    Some(d) if b(&o, p, "aligned") => [first[0] + d[0], first[1] + d[1]],
                    _ => point,
                }
            }
        };
        save_offset = Some([pos[0] - first[0], pos[1] - first[1]]);
        let src = src.filter(|s| *s != lid);
        (src, pos, f(&o, p, "cloneTimeShift", 0.0), b(&o, p, "lockSourceTime"), f(&o, p, "cloneTime", 0.0))
    });
    let erase_mode = ERASE_MODES.iter().position(|m| *m == s(&o, p, "eraseMode", "layerSourceAndPaint")).unwrap_or(0) as u32;
    let dynamics = [b(&o, p, "sizePressure") as u8 as f64, f(&o, p, "minSize", 1.0), b(&o, p, "opacityPressure") as u8 as f64, b(&o, p, "flowPressure") as u8 as f64];
    let ns = NewStroke { kind, points: pts, tip, in_time: lt, out_time: out, erase_mode, dynamics, clone };
    let write_on = dur_mode == 1;
    let duration = pf(p, "duration").unwrap_or(0.0).max(fd);
    let fx = paint_effect(e, cid, lid)?;
    let (gid, name) = {
        let g = e.layer_ref(cid, lid)?.props.find_group(fx).ok_or("no Paint effect")?;
        let strokes: Vec<&Group> = g.groups().collect();
        let n_kind = strokes.iter().filter(|s| s.get("kind").is_some_and(|k| matches!(&k.value, Value::Str(x) if x == KINDS[kind]))).count();
        let next = strokes.iter().filter_map(|s| s.id.strip_prefix('s').and_then(|x| x.parse::<u32>().ok())).max().unwrap_or(0) + 1;
        let label = match kind {
            1 => "Clone",
            2 => "Eraser",
            _ => "Brush",
        };
        (format!("s{next:04}"), format!("{label} {}", n_kind + 1))
    };
    let mut group = {
        let pj = e.proj();
        let mut ids = pj.ids();
        paint::stroke_group(&mut ids, &gid, &name, &ns)
    };
    if write_on
        && let Some(end) = group.sub_mut("strokeOptions").and_then(|g| g.get_mut("end"))
    {
        key::insert(&mut end.keys, Key::new(lt, Value::Num(0.0), false));
        key::insert(&mut end.keys, Key::new(lt + duration, Value::Num(100.0), false));
    }
    let uid = group.uid;
    e.layer_mut(cid, lid)?.props.find_group_mut(fx).ok_or("no Paint effect")?.children.push(Node::Group(group));
    if let Some(off) = save_offset {
        let mut m = Map::new();
        m.insert("cloneOffset".into(), json!(off));
        save(e, m);
    }
    e.set_label(format!("Paint Stroke: {name}"));
    Ok(json!({"layer": lid, "effect": fx, "stroke": uid, "name": name, "inTime": lt, "outTime": if out >= FOREVER { Json::Null } else { json!(out) }}))
}

pub fn remove_stroke(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let target = p.get("stroke").ok_or("pass `stroke`: a uid or name")?.clone();
    let removed = {
        let l = e.layer_mut(cid, lid)?;
        let fx = l.props.sub_mut("effects").ok_or("the layer has no paint")?;
        let mut out = None;
        for g in fx.groups_mut() {
            if !matches!(&g.kind, GroupKind::Effect { effect } if effect == paint::ID) {
                continue;
            }
            let at = g.children.iter().position(|n| match n {
                Node::Group(s) => target.as_u64() == Some(s.uid) || target.as_str() == Some(s.name.as_str()),
                _ => false,
            });
            if let Some(i) = at {
                out = Some(g.children.remove(i).uid());
                break;
            }
        }
        out.ok_or("no such paint stroke")?
    };
    e.set_label("Delete Paint Stroke");
    Ok(json!({"removed": removed}))
}
