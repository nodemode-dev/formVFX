use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pb, pf};
use crate::eval::Eval;
use crate::model::prop::Expr;
use crate::model::Value;

const HEAD: &str = "var __fvxIK=";
const SWITCH: &str = "IK Enabled";

pub fn add_control(e: &mut Engine, cid: u64, lid: u64, effect: &str, name: &str, value: Option<Value>) -> Result<u64, String> {
    let out = super::effect::apply(e, &json!({"effect": effect, "layer": lid, "comp": cid}))?;
    let uid = out["applied"][0]["uid"].as_u64().ok_or("the control was not added")?;
    let l = e.layer_mut(cid, lid)?;
    let g = l.props.find_group_mut(uid).ok_or("no such control")?;
    g.name = name.to_string();
    let mut value = value;
    g.for_each_prop_mut(&mut |pr| {
        if let Some(v) = value.take() {
            pr.value = v;
        }
    });
    Ok(uid)
}

pub fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn has_effect(e: &Engine, cid: u64, lid: u64, name: &str) -> bool {
    e.layer_ref(cid, lid).ok().and_then(|l| l.effects()).is_some_and(|g| g.groups().any(|x| x.name == name))
}

fn transform_prop<'a>(l: &'a mut crate::model::Layer, id: &str) -> Result<&'a mut crate::model::Prop, String> {
    l.props.sub_mut("transform").and_then(|t| t.get_mut(id)).ok_or_else(|| format!("{} has no {id} property", l.name))
}

fn common(u: &str, lo: &str, en: &str, goal: &str, su: f64, sl: f64, stretch: bool) -> String {
    let mut s = format!(
        "var U={u};\nvar Lw={lo};\nvar En={en};\nvar gl={goal};\nvar Up=U.transform.position.value;\nvar A=Up;\nvar rp=0;\nvar ps=1;\nif(U.hasParent){{\nvar pl=U.parent;\nA=pl.toComp(Up);\nvar px=pl.toCompVec([1,0]);\nrp=Math.atan2(px[1],px[0]);\nps=Math.sqrt(px[0]*px[0]+px[1]*px[1]);\n}}\nvar T=gl.toComp(gl.transform.anchorPoint.value);\nvar v1=sub(Lw.transform.position.value,U.transform.anchorPoint.value);\nvar v2=sub(En.transform.position.value,Lw.transform.anchorPoint.value);\nvar a0=length(v1)*{su}*ps;\nvar b0=length(v2)*{sl}*{su}*ps;\nvar dx=T[0]-A[0];\nvar dy=T[1]-A[1];\nvar d=Math.sqrt(dx*dx+dy*dy);\nvar f=1;\n"
    );
    if stretch {
        s.push_str("if(a0+b0>0.001&&d>a0+b0){f=d/(a0+b0);}\n");
    }
    s.push_str("var a=a0*f;\nvar b=b0*f;\nvar ang=Math.atan2(dy,dx);\n");
    s
}

fn solve(bend: f64) -> String {
    format!(
        "var dm=Math.min(Math.max(d,Math.abs(a-b)+0.001),a+b-0.001);\nvar ca=(a*a+dm*dm-b*b)/(2*a*dm);\nca=Math.max(-1,Math.min(1,ca));\nvar al=ang-({bend})*Math.acos(ca);\nvar phi=Math.atan2(v1[1],v1[0]);\n"
    )
}

fn gate(goal: &str, body: &str) -> String {
    format!("var out=value;\nif({goal}.effect(\"{SWITCH}\")(\"Checkbox\").value){{\n{body}}}\nout;")
}

fn upper_text(head: &Json, goal: &str, lo: &str, en: &str, su: f64, sl: f64, stretch: bool, bend: f64) -> String {
    let mut body = common("thisLayer", lo, en, goal, su, sl, stretch);
    body.push_str(&solve(bend));
    body.push_str("if(a>0.001&&b>0.001){out=(al-rp-phi)*180/Math.PI;}\n");
    format!("{HEAD}{};\n{}", head, gate(goal, &body))
}

fn lower_text(head: &Json, goal: &str, en: &str, su: f64, sl: f64, stretch: bool, bend: f64) -> String {
    let mut body = common("thisLayer.parent", "thisLayer", en, goal, su, sl, stretch);
    body.push_str(&solve(bend));
    body.push_str("if(a>0.001&&b>0.001){\nvar ex=A[0]+a*Math.cos(al);\nvar ey=A[1]+a*Math.sin(al);\nvar tx=A[0]+dm*Math.cos(ang);\nvar ty=A[1]+dm*Math.sin(ang);\nvar al2=Math.atan2(ty-ey,tx-ex);\nvar ux=U.toCompVec([1,0]);\nvar ru=Math.atan2(ux[1],ux[0]);\nvar phi2=Math.atan2(v2[1],v2[0]);\nout=(al2-ru-phi2)*180/Math.PI;\n}\n");
    format!("{HEAD}{};\n{}", head, gate(goal, &body))
}

fn scale_text(head: &Json, goal: &str, lo: &str, en: &str, su: f64, sl: f64) -> String {
    let mut body = common("thisLayer", lo, en, goal, su, sl, true);
    body.push_str("out=mul(value,f);\n");
    format!("{HEAD}{};\n{}", head, gate(goal, &body))
}

fn head_of(text: &str) -> Option<Json> {
    let rest = text.strip_prefix(HEAD)?;
    serde_json::from_str(rest.lines().next()?.strip_suffix(';')?).ok()
}

pub fn ik_create(e: &mut Engine, p: &Json) -> Res {
    let (cid, comp) = e.comp_of(p)?;
    let pick = |k: &str| -> Result<u64, String> { e.layer_in(comp, p.get(k).ok_or_else(|| format!("pass `{k}` (a layer id, name or #index)"))?) };
    let (upper, lower, end) = (pick("upper")?, pick("lower")?, pick("end")?);
    if upper == lower || lower == end || upper == end {
        return Err("upper, lower and end must be three different layers".into());
    }
    for id in [upper, lower, end] {
        let l = comp.layer(id).ok_or("no layer")?;
        if l.is_3d() {
            return Err(format!("{} is 3D; two-bone IK works on 2D layers", l.name));
        }
        if l.props.sub("transform").and_then(|t| t.get("position")).is_none() {
            return Err(format!("{} has separate position dimensions; join them first", l.name));
        }
    }
    let bend = match pf(p, "bend").unwrap_or(1.0) {
        x if x >= 0.0 => 1.0,
        _ => -1.0,
    };
    let stretch = pb(p, "stretch").unwrap_or(false);
    let existing = match p.get("goal").filter(|g| !g.is_null()) {
        Some(g) => Some(e.layer_in(comp, g)?),
        None => None,
    };
    for id in [upper, lower] {
        let pr = comp.layer(id).and_then(|l| l.props.sub("transform")).and_then(|t| t.get("rotation")).ok_or("no rotation property")?;
        if pr.expr.is_some() {
            return Err(format!("{} already has a rotation expression; remove it first", comp.layer(id).map(|l| l.name.as_str()).unwrap_or("")));
        }
    }
    if stretch && comp.layer(upper).and_then(|l| l.props.sub("transform")).and_then(|t| t.get("scale")).is_some_and(|s| s.expr.is_some()) {
        return Err("the upper layer already has a scale expression; remove it first".into());
    }
    if comp.layer(lower).is_some_and(|l| l.parent != Some(upper)) {
        super::layer::set_parent(e, &json!({"comp": cid, "layers": [lower], "parent": upper}))?;
    }
    if e.layer_ref(cid, end)?.parent != Some(lower) {
        super::layer::set_parent(e, &json!({"comp": cid, "layers": [end], "parent": lower}))?;
    }
    let (goal, created, spot) = {
        let comp = e.project.comp(cid).ok_or("no composition")?;
        let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
        let el = comp.layer(end).ok_or("no layer")?;
        let anchor = ev.tr_v3(el, "anchor", [0.0; 3]);
        let m = ev.layer_to_comp(el);
        let spot = m.apply(anchor[0], anchor[1]);
        (existing, existing.is_none(), spot)
    };
    let goal = match goal {
        Some(g) => g,
        None => {
            let r = super::layer::new_null(e, &json!({"comp": cid, "name": "IK Goal", "position": [spot[0], spot[1]]}))?;
            r["layer"].as_u64().ok_or("the goal was not created")?
        }
    };
    if goal == upper || goal == lower || goal == end {
        return Err("the goal must be a separate layer".into());
    }
    let fx_created = !has_effect(e, cid, goal, SWITCH);
    if fx_created {
        add_control(e, cid, goal, "fx.control.checkbox", SWITCH, Some(Value::Bool(true)))?;
    }
    let (su, sl, names) = {
        let comp = e.project.comp(cid).ok_or("no composition")?;
        let ev = Eval::new(&e.project, cid, comp, e.time_of(cid));
        let scale_of = |id: u64| comp.layer(id).map(|l| ev.tr_v3(l, "scale", [100.0; 3])[0].abs() / 100.0).unwrap_or(1.0);
        let name_of = |id: u64| comp.layer(id).map(|l| l.name.clone()).unwrap_or_default();
        (scale_of(upper), scale_of(lower), [name_of(upper), name_of(lower), name_of(end), name_of(goal)])
    };
    let layer_ref = |n: &str| format!("thisComp.layer({})", quoted(n));
    let (lo_ref, en_ref, goal_ref) = (layer_ref(&names[1]), layer_ref(&names[2]), layer_ref(&names[3]));
    let head = json!({"upper": upper, "lower": lower, "end": end, "goal": goal, "goalCreated": created, "switchCreated": fx_created, "stretch": stretch, "bend": bend});
    let rot_u = upper_text(&head, &goal_ref, &lo_ref, &en_ref, su, sl, stretch, bend);
    let rot_l = lower_text(&head, &goal_ref, &en_ref, su, sl, stretch, bend);
    let sc_u = stretch.then(|| scale_text(&head, &goal_ref, &lo_ref, &en_ref, su, sl));
    {
        let l = e.layer_mut(cid, upper)?;
        transform_prop(l, "rotation")?.expr = Some(Expr { text: rot_u, enabled: true });
        if let Some(t) = sc_u {
            transform_prop(l, "scale")?.expr = Some(Expr { text: t, enabled: true });
        }
        let l = e.layer_mut(cid, lower)?;
        transform_prop(l, "rotation")?.expr = Some(Expr { text: rot_l, enabled: true });
    }
    let t = e.time_of(cid);
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let ev = Eval::new(&e.project, cid, comp, t);
    for id in [upper, lower] {
        let l = comp.layer(id).ok_or("no layer")?;
        for pid in ["rotation", "scale"] {
            if let Some(pr) = l.props.sub("transform").and_then(|g| g.get(pid))
                && let Some(err) = crate::expr::check(&ev, l, pr)
            {
                return Err(format!("{}: {err}", l.name));
            }
        }
    }
    e.set_label("Create IK");
    Ok(json!({"goal": goal, "upper": upper, "lower": lower, "end": end, "switch": SWITCH, "goalCreated": created}))
}

pub fn ik_remove(e: &mut Engine, p: &Json) -> Res {
    let (cid, comp) = e.comp_of(p)?;
    let upper = e.layer_in(comp, p.get("upper").ok_or("pass `upper`")?)?;
    let head = comp
        .layer(upper)
        .and_then(|l| l.props.sub("transform"))
        .and_then(|t| t.get("rotation"))
        .and_then(|r| r.expr.as_ref())
        .and_then(|x| head_of(&x.text))
        .ok_or_else(|| format!("{} has no IK (rig.ikCreate)", comp.layer(upper).map(|l| l.name.as_str()).unwrap_or("that layer")))?;
    let id = |k: &str| head.get(k).and_then(Json::as_u64).unwrap_or(0);
    let (lower, goal) = (id("lower"), id("goal"));
    for (lid, props) in [(upper, vec!["rotation", "scale"]), (lower, vec!["rotation"])] {
        if let Ok(l) = e.layer_mut(cid, lid) {
            for pid in props {
                if let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut(pid))
                    && pr.expr.as_ref().is_some_and(|x| x.text.starts_with(HEAD))
                {
                    pr.expr = None;
                }
            }
        }
    }
    if head.get("goalCreated").and_then(Json::as_bool).unwrap_or(false) {
        super::layer::delete_layers(e, cid, &[goal])?;
    } else if head.get("switchCreated").and_then(Json::as_bool).unwrap_or(false)
        && let Ok(l) = e.layer_mut(cid, goal)
        && let Some(fx) = l.props.sub_mut("effects")
    {
        fx.children.retain(|c| c.name() != SWITCH);
    }
    e.set_label("Remove IK");
    Ok(json!({"upper": upper, "lower": lower, "goalRemoved": head.get("goalCreated").and_then(Json::as_bool).unwrap_or(false)}))
}
