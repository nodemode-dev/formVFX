use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pu};
use crate::eval::Eval;
use crate::model::prop::Expr;

const HEAD: &str = "var __fvxBehaviors=";
const KINDS: [&str; 7] = ["wiggle", "oscillate", "spring", "bounce", "loopCycle", "loopPingPong", "loopContinue"];

pub fn locate(e: &Engine, p: &Json) -> Result<(u64, u64, u64), String> {
    let mut q = p.clone();
    if let Some(v) = p.get("property").filter(|v| !v.is_null()) {
        q["prop"] = v.clone();
    }
    if q.get("prop").is_none_or(Json::is_null) && q.get("path").is_none_or(Json::is_null) {
        let (cid, lid) = e.layer_param(&q)?;
        let uid = e.state.selected_props.iter().find(|(l, _)| *l == lid).map(|x| x.1).ok_or("pass `property` (like transform/position) or select a property")?;
        return Ok((cid, lid, uid));
    }
    super::prop::locate(e, &q)
}

fn parse(text: &str) -> Option<Vec<Json>> {
    let rest = text.strip_prefix(HEAD)?;
    let line = rest.lines().next()?;
    serde_json::from_str::<Vec<Json>>(line.strip_suffix(';')?).ok()
}

fn defaults(kind: &str) -> Json {
    match kind {
        "wiggle" => json!({"frequency": 2.0, "amplitude": 20.0, "octaves": 1.0, "multiplier": 0.5}),
        "oscillate" => json!({"frequency": 1.0, "amplitude": 50.0, "angle": 0.0, "phase": 0.0, "decay": 0.0, "start": 0.0, "waveform": "sine"}),
        "spring" => json!({"amplitude": 0.1, "frequency": 2.0, "decay": 2.0}),
        "bounce" => json!({"elasticity": 0.7, "gravity": 4000.0, "maxBounces": 8.0}),
        _ => json!({"direction": "out", "keyframes": 0.0}),
    }
}

fn merged(kind: &str, given: Option<&Json>) -> Result<Json, String> {
    let mut out = defaults(kind);
    let Some(g) = given.filter(|g| !g.is_null()) else { return Ok(out) };
    let obj = g.as_object().ok_or("`params` must be an object")?;
    let base = out.as_object_mut().ok_or("params")?;
    for (k, v) in obj {
        let cur = base.get(k).ok_or_else(|| format!("{kind} has no parameter `{k}` (it takes: {})", base.keys().cloned().collect::<Vec<_>>().join(", ")))?;
        if cur.is_string() {
            let s = v.as_str().ok_or_else(|| format!("`{k}` must be text"))?;
            base.insert(k.clone(), json!(s));
        } else {
            let x = v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| format!("`{k}` must be a number"))?;
            base.insert(k.clone(), json!(x));
        }
    }
    Ok(out)
}

fn num(p: &Json, k: &str) -> f64 {
    p.get(k).and_then(Json::as_f64).unwrap_or(0.0)
}

fn text<'a>(p: &'a Json, k: &str) -> &'a str {
    p.get(k).and_then(Json::as_str).unwrap_or("")
}

fn direction(angle_deg: f64, dims: usize) -> String {
    let a = angle_deg.to_radians();
    match dims {
        2 => format!("[{},{}]", a.cos(), a.sin()),
        3 => format!("[{},{},0]", a.cos(), a.sin()),
        4 => "[1,1,1,0]".to_string(),
        _ => String::new(),
    }
}

fn snippet(i: usize, kind: &str, p: &Json, dims: usize) -> Result<String, String> {
    Ok(match kind {
        "wiggle" => format!("v=add(v,sub(wiggle({},{},{},{}),value));\n", num(p, "frequency"), num(p, "amplitude"), num(p, "octaves").max(1.0), num(p, "multiplier")),
        "oscillate" => {
            let wave = match text(p, "waveform") {
                "sine" => format!("Math.sin(w{i})"),
                "triangle" => format!("(2/Math.PI)*Math.asin(Math.sin(w{i}))"),
                "square" => format!("(Math.sin(w{i})>=0?1:-1)"),
                "saw" => format!("(2*(w{i}/(2*Math.PI)-Math.floor(w{i}/(2*Math.PI)+0.5)))"),
                other => return Err(format!("unknown waveform `{other}` (sine, triangle, square or saw)")),
            };
            let apply = if dims == 1 { format!("v=v+o{i};\n") } else { format!("v=add(v,mul({},o{i}));\n", direction(num(p, "angle"), dims)) };
            format!(
                "var t{i}=Math.max(0,time-{});\nvar w{i}=2*Math.PI*{}*t{i}+{}*Math.PI/180;\nvar o{i}={}*Math.exp(-{}*t{i})*{wave};\n{apply}",
                num(p, "start"),
                num(p, "frequency"),
                num(p, "phase"),
                num(p, "amplitude"),
                num(p, "decay")
            )
        }
        "spring" => format!(
            "var n{i}=numKeys;\nvar a{i}=0;\nif(n{i}>0){{\na{i}=nearestKey(time).index;\nif(key(a{i}).time>time){{a{i}=a{i}-1;}}\n}}\nif(a{i}>0){{\nvar t{i}=time-key(a{i}).time;\nv=add(v,mul(velocityAtTime(key(a{i}).time-0.001),{}*Math.sin({}*t{i}*2*Math.PI)/Math.exp({}*t{i})));\n}}\n",
            num(p, "amplitude"),
            num(p, "frequency"),
            num(p, "decay")
        ),
        "bounce" => {
            if num(p, "gravity") <= 0.0 {
                return Err("gravity must be above 0".into());
            }
            format!(
                "var n{i}=numKeys;\nif(n{i}>0){{\nvar a{i}=nearestKey(time).index;\nif(key(a{i}).time>time){{a{i}=a{i}-1;}}\nif(a{i}==n{i}){{\nvar vel{i}=velocityAtTime(key(a{i}).time-0.001);\nvar sp{i}=length(vel{i})*{e};\nif(sp{i}>0){{\nvar u{i}=mul(vel{i},-1/length(vel{i}));\nvar tau{i}=time-key(a{i}).time;\nvar s{i}=0;\nvar k{i}=0;\nvar d{i}=0;\nvar h{i}=0;\nwhile(d{i}==0&&k{i}<{m}&&sp{i}>0.001){{\nh{i}=2*sp{i}/{g};\nif(tau{i}<h{i}){{\ns{i}=sp{i}*tau{i}-0.5*{g}*tau{i}*tau{i};\nd{i}=1;\n}}else{{\ntau{i}=tau{i}-h{i};\nsp{i}=sp{i}*{e};\nk{i}=k{i}+1;\n}}\n}}\nv=add(v,mul(u{i},s{i}));\n}}\n}}\n}}\n",
                e = num(p, "elasticity").clamp(0.0, 1.0),
                g = num(p, "gravity"),
                m = num(p, "maxBounces").max(1.0).round()
            )
        }
        _ => {
            let mode = match kind {
                "loopCycle" => "cycle",
                "loopPingPong" => "pingpong",
                _ => "continue",
            };
            let n = num(p, "keyframes").max(0.0).round();
            let args = if mode == "continue" { format!("\"{mode}\"") } else { format!("\"{mode}\",{n}") };
            match text(p, "direction") {
                "out" => format!("if(numKeys>1){{v=loopOut({args});}}\n"),
                "in" => format!("if(numKeys>1){{v=loopIn({args});}}\n"),
                "both" => format!("if(numKeys>1){{\nif(time<key(1).time){{v=loopIn({args});}}else{{v=loopOut({args});}}\n}}\n"),
                other => return Err(format!("unknown direction `{other}` (out, in or both)")),
            }
        }
    })
}

fn build(list: &[Json], dims: usize) -> Result<String, String> {
    let mut out = format!("{HEAD}{};\nvar v=value;\n", serde_json::to_string(list).map_err(|x| x.to_string())?);
    for (i, b) in list.iter().enumerate() {
        let kind = b.get("kind").and_then(Json::as_str).ok_or("behavior without a kind")?;
        let params = b.get("params").cloned().unwrap_or(json!({}));
        out.push_str(&snippet(i, kind, &params, dims)?);
    }
    out.push_str("v;");
    Ok(out)
}

fn is_loop(b: &Json) -> bool {
    b.get("kind").and_then(Json::as_str).is_some_and(|k| k.starts_with("loop"))
}

fn state(e: &Engine, cid: u64, lid: u64, uid: u64) -> Result<(Vec<Json>, usize), String> {
    let l = e.layer_ref(cid, lid)?;
    let pr = l.props.find(uid).ok_or("that is a group, not a property")?;
    if !pr.value.is_numeric() {
        return Err(format!("{} is not a number, point or colour property", pr.name));
    }
    let list = match &pr.expr {
        None => vec![],
        Some(x) => parse(&x.text).ok_or_else(|| format!("{} has its own expression; remove it before adding behaviors", pr.name))?,
    };
    Ok((list, pr.value.dims()))
}

fn store(e: &mut Engine, cid: u64, lid: u64, uid: u64, list: &[Json], dims: usize, label: &str) -> Result<Option<String>, String> {
    let body = if list.is_empty() { None } else { Some(build(list, dims)?) };
    let l = e.layer_mut(cid, lid)?;
    let pr = l.props.find_mut(uid).ok_or("no property")?;
    pr.expr = body.map(|text| Expr { text, enabled: true });
    e.set_label(label);
    let t = e.time_of(cid);
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let l = comp.layer(lid).ok_or("no layer")?;
    Ok(l.props.find(uid).and_then(|pr| crate::expr::check(&Eval::new(&e.project, cid, comp, t), l, pr)))
}

fn listing(list: &[Json]) -> Json {
    json!(list.iter().enumerate().map(|(i, b)| json!({"index": i, "kind": b["kind"], "params": b["params"]})).collect::<Vec<_>>())
}

pub fn add(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let kind = crate::engine::ps(p, "kind").ok_or("pass `kind`")?;
    if !KINDS.contains(&kind) {
        return Err(format!("unknown kind `{kind}` ({})", KINDS.join(", ")));
    }
    let params = merged(kind, p.get("params"))?;
    let (mut list, dims) = state(e, cid, lid, uid)?;
    let entry = json!({"kind": kind, "params": params});
    let at = if is_loop(&entry) { list.iter().position(|b| !is_loop(b)).unwrap_or(list.len()) } else { list.len() };
    list.insert(at, entry);
    if let Some(err) = store(e, cid, lid, uid, &list, dims, "Add Behavior")? {
        return Err(err);
    }
    Ok(json!({"index": at, "behaviors": listing(&list)}))
}

pub fn list(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let (list, _) = state(e, cid, lid, uid)?;
    Ok(listing(&list))
}

pub fn remove(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let i = pu(p, "index").ok_or("pass `index` (0-based, behavior.list shows them)")? as usize;
    let (mut list, dims) = state(e, cid, lid, uid)?;
    if i >= list.len() {
        return Err(format!("no behavior {i}"));
    }
    list.remove(i);
    if let Some(err) = store(e, cid, lid, uid, &list, dims, "Remove Behavior")? {
        return Err(err);
    }
    Ok(json!({"removed": i, "behaviors": listing(&list)}))
}

pub fn clear(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = locate(e, p)?;
    let (list, dims) = state(e, cid, lid, uid)?;
    let n = list.len();
    store(e, cid, lid, uid, &[], dims, "Clear Behaviors")?;
    Ok(json!({"removed": n}))
}
