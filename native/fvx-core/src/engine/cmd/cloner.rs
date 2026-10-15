use serde_json::{Value as Json, json};

use super::rig::{add_control, quoted};
use crate::engine::{Engine, Res, pf, ps, pu};
use crate::model::prop::Expr;
use crate::model::Value;

const HEAD: &str = "var __fvxCloner=";

fn header(ctl: u64, k: usize) -> String {
    format!("{HEAD}[{ctl},{k}];\n")
}

fn read(name: &str, param: &str, ctl: &str) -> String {
    format!("{ctl}.effect({})(\"{param}\").value", quoted(name))
}

fn position_text(mode: &str, ctl_ref: &str, k: usize, head: &str) -> String {
    let offset = read("Offset", "Point", ctl_ref);
    let count = read("Count", "Slider", ctl_ref);
    match mode {
        "grid" => {
            let cols = read("Columns", "Slider", ctl_ref);
            format!("{head}var o={offset};\nvar n=Math.max(1,Math.round({cols}));\nvar col={k}%n;\nvar row=Math.floor({k}/n);\nadd(value,[col*o[0],row*o[1]]);")
        }
        "radial" => {
            let radius = read("Radius", "Slider", ctl_ref);
            let start = read("Start Angle", "Angle", ctl_ref);
            format!("{head}var cnt=Math.max(1,Math.round({count}));\nvar a=({start}+{k}*360/cnt)*Math.PI/180;\nvar r={radius};\nadd(value,[r*Math.cos(a),r*Math.sin(a)]);")
        }
        _ => format!("{head}var o={offset};\nadd(value,mul(o,{k}));"),
    }
}

fn prop_mut<'a>(l: &'a mut crate::model::Layer, id: &str) -> Option<&'a mut crate::model::Prop> {
    if id == "timeRemap" { l.props.get_mut(id) } else { l.props.sub_mut("transform").and_then(|t| t.get_mut(id)) }
}

fn set_expr(l: &mut crate::model::Layer, id: &str, text: String) -> Result<(), String> {
    let name = l.name.clone();
    let pr = prop_mut(l, id).ok_or_else(|| format!("{name} has no {id} property"))?;
    pr.expr = Some(Expr { text, enabled: true });
    Ok(())
}

fn texts(mode: &str, ctl_name: &str, ctl: u64, k: usize, timed: bool) -> Vec<(&'static str, String)> {
    let c = format!("thisComp.layer({})", quoted(ctl_name));
    let head = header(ctl, k);
    let mut out = vec![
        ("position", position_text(mode, &c, k, &head)),
        ("rotation", format!("{head}value+{k}*{};", read("Rotation Step", "Angle", &c))),
        ("scale", format!("{head}mul(value,Math.pow({}/100,{k}));", read("Scale Step", "Slider", &c))),
        (
            "opacity",
            format!(
                "{head}var vis={k}<Math.round({})?1:0;\nMath.max(0,Math.min(100,value+{k}*{}))*vis;",
                read("Count", "Slider", &c),
                read("Opacity Step", "Slider", &c)
            ),
        ),
    ];
    if timed {
        out.push(("timeRemap", format!("{head}value-{k}*{};", read("Time Step", "Slider", &c))));
    }
    out
}

pub fn create(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let mode = ps(p, "mode").unwrap_or("linear");
    if !["linear", "grid", "radial"].contains(&mode) {
        return Err("mode is linear, grid or radial".into());
    }
    let count = pu(p, "count").ok_or("pass `count` (the number of copies including the original)")? as usize;
    if !(2..=2000).contains(&count) {
        return Err("count is between 2 and 2000".into());
    }
    let (orig_name, timed, orig_pos) = {
        let l = e.layer_ref(cid, lid)?;
        if l.is_3d() {
            return Err("the cloner works on 2D layers".into());
        }
        let tr = l.props.sub("transform").ok_or("no transform")?;
        let pos = tr.get("position").ok_or("separate position dimensions are not supported; join them first")?;
        for id in ["position", "rotation", "scale", "opacity"] {
            if tr.get(id).is_some_and(|x| x.expr.is_some()) {
                return Err(format!("{} has an expression on {id}; remove it first", l.name));
            }
        }
        let timed = l.props.get("timeRemap").is_some_and(|x| x.expr.is_none());
        (l.name.clone(), timed, pos.value.vec3())
    };
    let off = p.get("offset").filter(|o| o.is_object()).cloned().unwrap_or(json!({}));
    let ofs_pos = super::vec2_param(&off, "position").unwrap_or(if mode == "grid" { [100.0, 100.0] } else { [100.0, 0.0] });
    let columns = pu(p, "columns").map(|c| c as f64).unwrap_or_else(|| match pu(p, "rows") {
        Some(r) if r > 0 => (count as f64 / r as f64).ceil(),
        _ => (count as f64).sqrt().ceil(),
    });
    let ctl_name = {
        let comp = e.project.comp(cid).ok_or("no composition")?;
        super::unique_layer_name(comp, "Cloner Controller")
    };
    let made = super::layer::new_null(e, &json!({"comp": cid, "name": ctl_name, "position": [orig_pos[0], orig_pos[1]]}))?;
    let ctl = made["layer"].as_u64().ok_or("the controller was not created")?;
    add_control(e, cid, ctl, "fx.control.slider", "Count", Some(Value::Num(count as f64)))?;
    match mode {
        "grid" => {
            add_control(e, cid, ctl, "fx.control.slider", "Columns", Some(Value::Num(columns.max(1.0))))?;
            add_control(e, cid, ctl, "fx.control.point", "Offset", Some(Value::Vec2(ofs_pos)))?;
        }
        "radial" => {
            add_control(e, cid, ctl, "fx.control.slider", "Radius", Some(Value::Num(pf(p, "radius").unwrap_or(200.0))))?;
            add_control(e, cid, ctl, "fx.control.angle", "Start Angle", Some(Value::Num(-90.0)))?;
        }
        _ => {
            add_control(e, cid, ctl, "fx.control.point", "Offset", Some(Value::Vec2(ofs_pos)))?;
        }
    }
    add_control(e, cid, ctl, "fx.control.angle", "Rotation Step", Some(Value::Num(pf(&off, "rotation").unwrap_or(0.0))))?;
    add_control(e, cid, ctl, "fx.control.slider", "Scale Step", Some(Value::Num(pf(&off, "scale").unwrap_or(100.0))))?;
    add_control(e, cid, ctl, "fx.control.slider", "Opacity Step", Some(Value::Num(pf(&off, "opacity").unwrap_or(0.0))))?;
    if timed {
        add_control(e, cid, ctl, "fx.control.slider", "Time Step", Some(Value::Num(pf(&off, "time").unwrap_or(0.0))))?;
    }
    let mut ids = vec![lid];
    for _ in 1..count {
        let r = super::layer::duplicate(e, &json!({"comp": cid, "layer": lid}))?;
        ids.push(r["layers"][0].as_u64().ok_or("the copy was not created")?);
    }
    for (k, id) in ids.iter().enumerate() {
        for (prop, text) in texts(mode, &ctl_name, ctl, k, timed) {
            set_expr(e.layer_mut(cid, *id)?, prop, text)?;
        }
    }
    super::select_only(e, &ids);
    e.set_label("Create Cloner");
    Ok(json!({"controller": ctl, "layers": ids, "mode": mode, "count": count, "source": orig_name, "timeOffset": timed}))
}

fn tag(l: &crate::model::Layer) -> Option<(u64, usize)> {
    let text = &l.props.sub("transform")?.get("position")?.expr.as_ref()?.text;
    let rest = text.strip_prefix(HEAD)?;
    let list: Vec<u64> = serde_json::from_str(rest.lines().next()?.strip_suffix(';')?).ok()?;
    Some((*list.first()?, *list.get(1)? as usize))
}

pub fn remove(e: &mut Engine, p: &Json) -> Res {
    let (cid, comp) = e.comp_of(p)?;
    let ctl = e.layer_in(comp, p.get("controller").ok_or("pass `controller` (the controller layer)")?)?;
    let members: Vec<(u64, usize)> = comp.layers.iter().filter_map(|l| tag(l).filter(|t| t.0 == ctl).map(|t| (l.id, t.1))).collect();
    if members.is_empty() {
        return Err("that layer is not a cloner controller (cloner.create)".into());
    }
    let clones: Vec<u64> = members.iter().filter(|m| m.1 > 0).map(|m| m.0).collect();
    for (id, k) in &members {
        if *k > 0 {
            continue;
        }
        let l = e.layer_mut(cid, *id)?;
        for pid in ["position", "rotation", "scale", "opacity", "timeRemap"] {
            if let Some(pr) = prop_mut(l, pid)
                && pr.expr.as_ref().is_some_and(|x| x.text.starts_with(&header(ctl, 0)))
            {
                pr.expr = None;
            }
        }
    }
    let mut gone = clones.clone();
    gone.push(ctl);
    let n = super::layer::delete_layers(e, cid, &gone)?;
    e.set_label("Remove Cloner");
    Ok(json!({"deleted": n, "clones": clones}))
}
