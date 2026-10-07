use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, VertexRef, pb, pf, ps, pu};
use crate::model::key::{self, Key};
use crate::model::path::BezPath;
use crate::model::prop::{self, Node};
use crate::model::Value;

fn path_prop_uid(e: &Engine, cid: u64, lid: u64, group: u64) -> Result<u64, String> {
    let l = e.layer_ref(cid, lid)?;
    let g = l.props.find_group(group).ok_or("no such mask or path")?;
    g.get("path").map(|p| p.uid).ok_or_else(|| "that group has no path".into())
}

fn edit_path(e: &mut Engine, cid: u64, lid: u64, group: u64, f: impl FnOnce(&mut BezPath) -> Result<(), String>) -> Result<(), String> {
    let uid = path_prop_uid(e, cid, lid, group)?;
    let now = e.time_of(cid);
    let l = e.layer_mut(cid, lid)?;
    let lt = l.layer_time(now);
    let pr = l.props.find_mut(uid).ok_or("no path")?;
    let mut bp = pr.at(lt).as_path().cloned().unwrap_or_default().normalized();
    f(&mut bp)?;
    if pr.keys.is_empty() {
        pr.value = Value::Path(bp);
    } else {
        key::insert(&mut pr.keys, Key::new(lt, Value::Path(bp), false));
    }
    Ok(())
}

fn mask_of(e: &Engine, p: &Json) -> Result<(u64, u64, u64), String> {
    let (cid, lid) = e.layer_param(p)?;
    let m = pu(p, "mask").ok_or("pass `mask` (the mask or path uid)")?;
    Ok((cid, lid, m))
}

fn pt(p: &Json, k: &str) -> Option<[f64; 2]> {
    let a = p.get(k)?.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
}

pub fn set_vertex(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, m) = mask_of(e, p)?;
    let i = pu(p, "index").ok_or("pass `index`")? as usize;
    let (point, tin, tout) = (pt(p, "point"), pt(p, "in"), pt(p, "out"));
    edit_path(e, cid, lid, m, |bp| {
        if i >= bp.vertices.len() {
            return Err("no such vertex".into());
        }
        if let Some(v) = point {
            bp.vertices[i] = v;
        }
        if let Some(v) = tin {
            bp.ins[i] = v;
        }
        if let Some(v) = tout {
            bp.outs[i] = v;
        }
        Ok(())
    })?;
    e.set_label("Move Vertex");
    Ok(json!({"ok": true}))
}

fn vertex_list(e: &Engine, p: &Json) -> Vec<VertexRef> {
    match p.get("vertices").and_then(Json::as_array) {
        Some(a) => a.iter().filter_map(|v| Some(VertexRef { layer: pu(v, "layer")?, mask: pu(v, "mask")?, index: pu(v, "index")? as usize })).collect(),
        None => e.state.selected_vertices.clone(),
    }
}

pub fn move_vertices(e: &mut Engine, p: &Json) -> Res {
    let d = pt(p, "delta").ok_or("pass `delta` [dx, dy]")?;
    let cid = e.comp_id(p.get("comp"))?;
    let list = vertex_list(e, p);
    let mut groups: Vec<(u64, u64, Vec<usize>)> = vec![];
    for v in &list {
        match groups.iter_mut().find(|g| g.0 == v.layer && g.1 == v.mask) {
            Some(g) => g.2.push(v.index),
            None => groups.push((v.layer, v.mask, vec![v.index])),
        }
    }
    for (lid, m, idx) in groups {
        edit_path(e, cid, lid, m, |bp| {
            for i in idx {
                if let Some(v) = bp.vertices.get_mut(i) {
                    v[0] += d[0];
                    v[1] += d[1];
                }
            }
            Ok(())
        })?;
    }
    e.set_label("Move Vertices");
    Ok(json!({"moved": list.len()}))
}

pub fn select_vertices(e: &mut Engine, p: &Json) -> Res {
    let list = vertex_list(e, p);
    if pb(p, "add").unwrap_or(false) {
        for v in list {
            if let Some(i) = e.state.selected_vertices.iter().position(|x| *x == v) {
                e.state.selected_vertices.remove(i);
            } else {
                e.state.selected_vertices.push(v);
            }
        }
    } else {
        e.state.selected_vertices = list;
    }
    Ok(json!({"selected": e.state.selected_vertices.len()}))
}

pub fn insert_vertex(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, m) = mask_of(e, p)?;
    let seg = pu(p, "segment").ok_or("pass `segment`")? as usize;
    let t = pf(p, "t").unwrap_or(0.5).clamp(0.0, 1.0);
    edit_path(e, cid, lid, m, |bp| {
        if seg >= bp.segments() {
            return Err("no such segment".into());
        }
        let s = bp.segment(seg);
        let lerp = |a: [f64; 2], b: [f64; 2]| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        let (p01, p12, p23) = (lerp(s[0], s[1]), lerp(s[1], s[2]), lerp(s[2], s[3]));
        let (p012, p123) = (lerp(p01, p12), lerp(p12, p23));
        let mid = lerp(p012, p123);
        let j = (seg + 1) % bp.vertices.len();
        let a = bp.vertices[seg];
        let b = bp.vertices[j];
        bp.outs[seg] = [p01[0] - a[0], p01[1] - a[1]];
        bp.ins[j] = [p23[0] - b[0], p23[1] - b[1]];
        let at = seg + 1;
        bp.vertices.insert(at, mid);
        bp.ins.insert(at, [p012[0] - mid[0], p012[1] - mid[1]]);
        bp.outs.insert(at, [p123[0] - mid[0], p123[1] - mid[1]]);
        Ok(())
    })?;
    e.set_label("Add Vertex");
    Ok(json!({"ok": true}))
}

pub fn delete_vertices(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let list = vertex_list(e, p);
    let mut groups: Vec<(u64, u64, Vec<usize>)> = vec![];
    for v in &list {
        match groups.iter_mut().find(|g| g.0 == v.layer && g.1 == v.mask) {
            Some(g) => g.2.push(v.index),
            None => groups.push((v.layer, v.mask, vec![v.index])),
        }
    }
    for (lid, m, mut idx) in groups {
        idx.sort_unstable();
        idx.dedup();
        edit_path(e, cid, lid, m, |bp| {
            for i in idx.into_iter().rev() {
                if i < bp.vertices.len() && bp.vertices.len() > 2 {
                    bp.vertices.remove(i);
                    bp.ins.remove(i);
                    bp.outs.remove(i);
                }
            }
            Ok(())
        })?;
    }
    e.state.selected_vertices.clear();
    e.set_label("Delete Vertices");
    Ok(json!({"deleted": list.len()}))
}

pub fn convert_vertex(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, m) = mask_of(e, p)?;
    let i = pu(p, "index").ok_or("pass `index`")? as usize;
    edit_path(e, cid, lid, m, |bp| {
        let n = bp.vertices.len();
        if i >= n {
            return Err("no such vertex".into());
        }
        if bp.ins[i] != [0.0; 2] || bp.outs[i] != [0.0; 2] {
            bp.ins[i] = [0.0; 2];
            bp.outs[i] = [0.0; 2];
        } else {
            let prev = bp.vertices[(i + n - 1) % n];
            let next = bp.vertices[(i + 1) % n];
            let d = [(next[0] - prev[0]) / 6.0, (next[1] - prev[1]) / 6.0];
            bp.ins[i] = [-d[0], -d[1]];
            bp.outs[i] = d;
        }
        Ok(())
    })?;
    e.set_label("Convert Vertex");
    Ok(json!({"ok": true}))
}

pub fn set_closed(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, m) = mask_of(e, p)?;
    let closed = pb(p, "closed").unwrap_or(true);
    edit_path(e, cid, lid, m, |bp| {
        bp.closed = closed;
        Ok(())
    })?;
    Ok(json!({"closed": closed}))
}

pub fn add_shape(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let kind = prop::shape_kind_alias(ps(p, "kind").ok_or("pass `kind`: group|rect|ellipse|star|polygon|path|fill|stroke|gradientFill|gradientStroke|trim|repeater|roundCorners|offsetPaths|twist|wiggle|mergePaths|puckerBloat|zigZag|wiggleTransform")?).to_string();
    let target = pu(p, "group");
    let l = e.layer_ref(cid, lid)?;
    if l.contents().is_none() {
        return Err(format!("{} is not a shape layer", l.name));
    }
    let (g, extra_fill) = {
        let pj = e.proj();
        let mut ids = pj.ids();
        let pk = if kind == "polygon" { "polygon" } else { kind.as_str() };
        let g = prop::shape_item(&mut ids, pk, ps(p, "name")).ok_or_else(|| format!("unknown shape item `{kind}`"))?;
        let fill = if kind == "mergePaths" { prop::shape_item(&mut ids, "fill", None) } else { None };
        (g, fill)
    };
    let mut g = g;
    if let Some(path) = p.get("path").and_then(crate::model::value::parse_path)
        && let Some(pr) = g.get_mut("path")
    {
        pr.value = Value::Path(path);
    }
    let uid = g.uid;
    let l = e.layer_mut(cid, lid)?;
    let contents = match target {
        Some(gid) => l.props.find_group_mut(gid).and_then(|grp| grp.sub_mut("contents")).ok_or("no such shape group")?,
        None => l.props.sub_mut("contents").ok_or("no contents")?,
    };
    let at = if prop::SHAPE_OPERATORS.contains(&kind.as_str()) { contents.children.len() } else { 0 };
    let name = {
        let base = g.name.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end().to_string();
        let n = contents.children.iter().filter(|c| c.name().starts_with(&base)).count() + 1;
        format!("{base} {n}")
    };
    g.name = name;
    contents.children.insert(at, Node::Group(g));
    if let Some(fill) = extra_fill {
        contents.children.push(Node::Group(fill));
    }
    e.set_label(format!("Add {kind}"));
    Ok(json!({"layer": lid, "uid": uid}))
}

pub fn mask_new(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let pts: Vec<[f64; 2]> = p.get("vertices").and_then(Json::as_array).map(|a| a.iter().filter_map(|v| Some([v.get(0)?.as_f64()?, v.get(1)?.as_f64()?])).collect()).unwrap_or_default();
    if pts.is_empty() {
        return Err("pass `vertices` [[x, y]...] in layer pixels".into());
    }
    let closed = pb(p, "closed").unwrap_or(pts.len() > 2 && p.get("closed").is_some());
    let mut path = BezPath::polygon(&pts, closed);
    if let Some(a) = p.get("in").and_then(Json::as_array) {
        for (i, v) in a.iter().enumerate().take(path.ins.len()) {
            path.ins[i] = [v.get(0).and_then(Json::as_f64).unwrap_or(0.0), v.get(1).and_then(Json::as_f64).unwrap_or(0.0)];
        }
    }
    if let Some(a) = p.get("out").and_then(Json::as_array) {
        for (i, v) in a.iter().enumerate().take(path.outs.len()) {
            path.outs[i] = [v.get(0).and_then(Json::as_f64).unwrap_or(0.0), v.get(1).and_then(Json::as_f64).unwrap_or(0.0)];
        }
    }
    let mode = ps(p, "mode").and_then(prop::MaskMode::parse).unwrap_or(prop::MaskMode::Add);
    let count = e.layer_ref(cid, lid)?.masks().map(|m| m.children.len()).ok_or("this layer cannot have masks")?;
    let color = crate::model::label_rgb(((count % 15) + 1) as u8);
    let name = ps(p, "name").map(str::to_string).unwrap_or_else(|| format!("Mask {}", count + 1));
    let g = {
        let pj = e.proj();
        let mut ids = pj.ids();
        prop::mask_group(&mut ids, &name, path, mode, color)
    };
    let uid = g.uid;
    let l = e.layer_mut(cid, lid)?;
    l.props.sub_mut("masks").ok_or("this layer cannot have masks")?.children.push(Node::Group(g));
    e.set_label("New Mask");
    Ok(json!({"layer": lid, "mask": uid, "index": count + 1}))
}

pub fn add_vertex(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, m) = mask_of(e, p)?;
    let v = pt(p, "point").ok_or("pass `point` [x, y] in layer pixels")?;
    let at = pu(p, "index").map(|x| x as usize);
    let mut idx = 0;
    edit_path(e, cid, lid, m, |bp| {
        let i = at.unwrap_or(bp.vertices.len()).min(bp.vertices.len());
        bp.vertices.insert(i, v);
        bp.ins.insert(i, pt(p, "in").unwrap_or([0.0; 2]));
        bp.outs.insert(i, pt(p, "out").unwrap_or([0.0; 2]));
        idx = i;
        Ok(())
    })?;
    e.set_label("Add Vertex");
    Ok(json!({"index": idx}))
}
