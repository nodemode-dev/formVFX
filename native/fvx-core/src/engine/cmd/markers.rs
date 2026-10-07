use serde_json::{Value as Json, json};

use crate::engine::state::marker_json;
use crate::engine::{Engine, Res, pb, pf, ps, pu};
use crate::model::{CuePoint, Marker, label_parse};

#[derive(Clone, Copy)]
enum Owner {
    Comp(u64),
    Layer(u64, u64),
}

fn owner(e: &Engine, p: &Json) -> Result<Owner, String> {
    let (cid, comp) = e.comp_of(p)?;
    match p.get("layer").filter(|v| !v.is_null()) {
        Some(v) => Ok(Owner::Layer(cid, e.layer_in(comp, v)?)),
        None => Ok(Owner::Comp(cid)),
    }
}

fn list_of(e: &Engine, o: Owner) -> Result<Vec<Marker>, String> {
    match o {
        Owner::Comp(c) => Ok(e.project.comp(c).ok_or("no composition")?.markers.clone()),
        Owner::Layer(c, l) => Ok(e.layer_ref(c, l)?.markers.clone()),
    }
}

fn list_mut(e: &mut Engine, o: Owner) -> Result<&mut Vec<Marker>, String> {
    match o {
        Owner::Comp(c) => Ok(&mut e.proj().comp_mut(c).ok_or("no composition")?.markers),
        Owner::Layer(c, l) => Ok(&mut e.layer_mut(c, l)?.markers),
    }
}

fn to_comp(e: &Engine, o: Owner, t: f64) -> f64 {
    match o {
        Owner::Layer(c, l) => e.layer_ref(c, l).map(|x| x.comp_time(t)).unwrap_or(t),
        Owner::Comp(_) => t,
    }
}

fn from_comp(e: &Engine, o: Owner, t: f64) -> f64 {
    match o {
        Owner::Layer(c, l) => e.layer_ref(c, l).map(|x| x.layer_time(t)).unwrap_or(t),
        Owner::Comp(_) => t,
    }
}

fn index_of(e: &Engine, o: Owner, p: &Json, list: &[Marker]) -> Result<usize, String> {
    if let Some(i) = pu(p, "index") {
        return if (i as usize) < list.len() { Ok(i as usize) } else { Err(format!("no marker {i}")) };
    }
    let at = pf(p, "at").ok_or("pass `index` (0-based) or `at` (comp s)")?;
    let fd = match o {
        Owner::Comp(c) | Owner::Layer(c, _) => e.project.comp(c).map(|x| x.frame_duration()).unwrap_or(1.0 / 30.0),
    };
    list.iter()
        .enumerate()
        .map(|(i, m)| (i, (to_comp(e, o, m.time) - at).abs()))
        .filter(|(_, d)| *d <= fd * 0.5 + 1e-6)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|x| x.0)
        .ok_or_else(|| format!("no marker at {at:.3} s"))
}

fn json_of(e: &Engine, o: Owner, i: usize, m: &Marker) -> Json {
    let mut j = marker_json(m);
    j["index"] = json!(i);
    j["compTime"] = json!(to_comp(e, o, m.time));
    j
}

pub fn list(e: &mut Engine, p: &Json) -> Res {
    let o = owner(e, p)?;
    let ms = list_of(e, o)?;
    Ok(json!(ms.iter().enumerate().map(|(i, m)| json_of(e, o, i, m)).collect::<Vec<_>>()))
}

fn apply(m: &mut Marker, p: &Json, t: Option<f64>) -> Result<(), String> {
    if let Some(t) = t {
        m.time = t;
    }
    if let Some(d) = pf(p, "duration") {
        m.duration = d.max(0.0);
    }
    for (k, f) in [("comment", &mut m.comment), ("chapter", &mut m.chapter), ("url", &mut m.url), ("frameTarget", &mut m.frame_target)] {
        if let Some(s) = ps(p, k) {
            *f = s.to_string();
        }
    }
    if let Some(b) = pb(p, "protected") {
        m.protected = b;
    }
    match p.get("label") {
        Some(Json::Number(n)) => m.label = n.as_u64().unwrap_or(0).min(16) as u8,
        Some(Json::String(s)) => m.label = label_parse(s).ok_or_else(|| format!("unknown label `{s}`"))?,
        _ => {}
    }
    match p.get("cuePoint") {
        Some(Json::Null) => m.cue_point = None,
        Some(c @ Json::Object(_)) => {
            let params = c
                .get("params")
                .and_then(Json::as_array)
                .map(|a| a.iter().filter_map(|x| Some((x.get(0)?.as_str()?.to_string(), x.get(1).map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))?))).collect())
                .unwrap_or_default();
            m.cue_point = Some(CuePoint { name: c.get("name").and_then(Json::as_str).unwrap_or("").to_string(), navigation: c.get("navigation").and_then(Json::as_bool).unwrap_or(false), params });
        }
        _ => {}
    }
    Ok(())
}

pub fn set(e: &mut Engine, p: &Json) -> Res {
    let o = owner(e, p)?;
    let list = list_of(e, o)?;
    let t = pf(p, "time").map(|t| from_comp(e, o, t));
    let fresh = pb(p, "new").unwrap_or(false);
    let now = from_comp(e, o, e.time_of(match o {
        Owner::Comp(c) | Owner::Layer(c, _) => c,
    }));
    let (mut m, idx) = if fresh {
        (Marker { time: now, ..Marker::default() }, None)
    } else {
        let i = index_of(e, o, p, &list)?;
        (list[i].clone(), Some(i))
    };
    apply(&mut m, p, t)?;
    let ms = list_mut(e, o)?;
    match idx {
        Some(i) => ms[i] = m.clone(),
        None => ms.push(m.clone()),
    }
    ms.sort_by(|a, b| a.time.total_cmp(&b.time));
    let i = ms.iter().position(|x| *x == m).unwrap_or(0);
    e.set_label("Marker Settings");
    Ok(json_of(e, o, i, &m))
}

pub fn delete(e: &mut Engine, p: &Json) -> Res {
    let o = owner(e, p)?;
    let list = list_of(e, o)?;
    let i = index_of(e, o, p, &list)?;
    list_mut(e, o)?.remove(i);
    e.set_label("Delete Marker");
    Ok(json!({"deleted": i}))
}

pub fn convert(e: &mut Engine, p: &Json) -> Res {
    let (cid, comp) = e.comp_of(p)?;
    if let Some(v) = p.get("toLayer").filter(|v| !v.is_null()) {
        let lid = e.layer_in(comp, v)?;
        let list = comp.markers.clone();
        let i = index_of(e, Owner::Comp(cid), p, &list)?;
        let mut m = list[i].clone();
        m.time = e.layer_ref(cid, lid)?.layer_time(m.time);
        e.proj().comp_mut(cid).ok_or("no composition")?.markers.remove(i);
        let l = e.layer_mut(cid, lid)?;
        l.markers.push(m);
        l.markers.sort_by(|a, b| a.time.total_cmp(&b.time));
        e.set_label("Convert to Layer Marker");
        return Ok(json!({"layer": lid}));
    }
    let o = owner(e, p)?;
    let Owner::Layer(_, lid) = o else { return Err("pass `layer` (layer marker to comp marker) or `toLayer` (comp marker to layer marker)".into()) };
    let list = list_of(e, o)?;
    let i = index_of(e, o, p, &list)?;
    let mut m = list[i].clone();
    m.time = to_comp(e, o, m.time);
    e.layer_mut(cid, lid)?.markers.remove(i);
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    c.markers.push(m);
    c.markers.sort_by(|a, b| a.time.total_cmp(&b.time));
    e.set_label("Convert to Composition Marker");
    Ok(json!({"comp": cid}))
}

pub fn nested(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    let sub = l.source.and_then(|s| e.project.comp(s)).ok_or_else(|| format!("{} is not a precomp layer", l.name))?;
    let out: Vec<Json> = sub
        .markers
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let mut j = marker_json(m);
            j["index"] = json!(i);
            j["compTime"] = json!(l.comp_time(m.time));
            j["visible"] = json!(l.is_active_at(l.comp_time(m.time)));
            j
        })
        .collect();
    Ok(json!(out))
}
