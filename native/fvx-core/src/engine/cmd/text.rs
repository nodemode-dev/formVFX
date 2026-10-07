use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, TextEdit, pb, ps, pu};
use crate::model::{LayerKind, Value};

fn current(e: &Engine) -> Result<(u64, u64, String), String> {
    let te = e.state.text_edit.clone().ok_or("no text is being edited (text.edit starts)")?;
    let cid = e.comp_id(None)?;
    let now = e.time_of(cid);
    let l = e.layer_ref(cid, te.layer)?;
    let lt = l.layer_time(now);
    let doc = l.text_prop().map(|p| p.at(lt)).and_then(|v| v.as_text().cloned()).ok_or("not a text layer")?;
    Ok((cid, te.layer, doc.text))
}

fn write(e: &mut Engine, cid: u64, lid: u64, text: String) -> Result<(), String> {
    let now = e.time_of(cid);
    let l = e.layer_mut(cid, lid)?;
    let lt = l.layer_time(now);
    let first_line: String = text.lines().next().unwrap_or("").chars().take(40).collect();
    let auto_name = l.name == "Text" || l.name.is_empty();
    let pr = l.props.sub_mut("text").and_then(|g| g.get_mut("sourceText")).ok_or("no text")?;
    let mut doc = pr.at(lt).as_text().cloned().ok_or("no text")?;
    doc.text = text;
    if pr.keys.is_empty() {
        pr.value = Value::Text(Box::new(doc));
    } else {
        let mut k = crate::model::Key::new(lt, Value::Text(Box::new(doc)), false);
        k.in_interp = crate::model::Interp::Hold;
        k.out_interp = crate::model::Interp::Hold;
        crate::model::key::insert(&mut pr.keys, k);
    }
    if auto_name && !first_line.trim().is_empty() {
        l.name = first_line;
    }
    Ok(())
}

pub fn edit(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let l = e.layer_ref(cid, lid)?;
    if l.kind != LayerKind::Text {
        return Err(format!("{} is not a text layer", l.name));
    }
    let n = l.text_prop().and_then(|p| p.value.as_text().map(|d| d.text.chars().count())).unwrap_or(0);
    let caret = pu(p, "caret").map(|x| x as usize).unwrap_or(n).min(n);
    let anchor = if pb(p, "selectAll").unwrap_or(true) && p.get("caret").is_none() { 0 } else { caret };
    e.state.text_edit = Some(TextEdit { layer: lid, anchor, caret });
    e.state.selected_layers = vec![lid];
    Ok(json!({"layer": lid, "caret": caret}))
}

pub fn end_edit(e: &mut Engine, _: &Json) -> Res {
    e.state.text_edit = None;
    e.history.merge = None;
    Ok(json!({"ok": true}))
}

pub fn insert(e: &mut Engine, p: &Json) -> Res {
    let ins = ps(p, "text").ok_or("pass `text`")?.to_string();
    let (cid, lid, text) = current(e)?;
    let te = e.state.text_edit.clone().unwrap_or_default();
    let chars: Vec<char> = text.chars().collect();
    let (a, b) = (te.anchor.min(te.caret).min(chars.len()), te.anchor.max(te.caret).min(chars.len()));
    let mut out: String = chars[..a].iter().collect();
    out.push_str(&ins);
    out.extend(chars[b..].iter());
    write(e, cid, lid, out)?;
    let c = a + ins.chars().count();
    e.state.text_edit = Some(TextEdit { layer: lid, anchor: c, caret: c });
    e.set_label("Edit Text");
    e.merge_hint = Some(format!("text-{lid}"));
    Ok(json!({"caret": c}))
}

pub fn delete(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, text) = current(e)?;
    let te = e.state.text_edit.clone().unwrap_or_default();
    let chars: Vec<char> = text.chars().collect();
    let back = ps(p, "direction").unwrap_or("backward") == "backward";
    let word = pb(p, "word").unwrap_or(false);
    let (mut a, mut b) = (te.anchor.min(te.caret).min(chars.len()), te.anchor.max(te.caret).min(chars.len()));
    if a == b {
        if back {
            if a == 0 {
                return Ok(json!({"caret": 0}));
            }
            a -= 1;
            if word {
                while a > 0 && !chars[a - 1].is_whitespace() {
                    a -= 1;
                }
            }
        } else {
            if b >= chars.len() {
                return Ok(json!({"caret": b}));
            }
            b += 1;
            if word {
                while b < chars.len() && !chars[b].is_whitespace() {
                    b += 1;
                }
            }
        }
    }
    let out: String = chars[..a].iter().chain(chars[b..].iter()).collect();
    write(e, cid, lid, out)?;
    e.state.text_edit = Some(TextEdit { layer: lid, anchor: a, caret: a });
    e.set_label("Edit Text");
    e.merge_hint = Some(format!("text-{lid}"));
    Ok(json!({"caret": a}))
}

pub fn move_caret(e: &mut Engine, p: &Json) -> Res {
    let (_, lid, text) = current(e)?;
    let te = e.state.text_edit.clone().unwrap_or_default();
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let extend = pb(p, "extend").unwrap_or(false);
    let line_start = |i: usize| chars[..i].iter().rposition(|c| *c == '\r' || *c == '\n').map(|x| x + 1).unwrap_or(0);
    let line_end = |i: usize| chars[i..].iter().position(|c| *c == '\r' || *c == '\n').map(|x| x + i).unwrap_or(n);
    let c = te.caret.min(n);
    let to = ps(p, "to").unwrap_or("right");
    let nc = match to {
        "left" => c.saturating_sub(1),
        "right" => (c + 1).min(n),
        "home" | "lineStart" => line_start(c),
        "end" | "lineEnd" => line_end(c),
        "start" => 0,
        "all" => n,
        "up" => {
            let s = line_start(c);
            if s == 0 {
                0
            } else {
                let ps_ = line_start(s - 1);
                (ps_ + (c - s)).min(s - 1)
            }
        }
        "down" => {
            let e_ = line_end(c);
            if e_ >= n {
                n
            } else {
                let ns = e_ + 1;
                (ns + (c - line_start(c))).min(line_end(ns))
            }
        }
        _ => c,
    };
    let anchor = if to == "all" { 0 } else if extend { te.anchor } else { nc };
    e.state.text_edit = Some(TextEdit { layer: lid, anchor, caret: nc });
    Ok(json!({"caret": nc, "anchor": anchor}))
}

pub fn fonts(_: &mut Engine, _: &Json) -> Res {
    let fams: Vec<Json> = crate::render::text::families().into_iter().map(|(f, s)| json!({"family": f, "styles": s})).collect();
    Ok(json!(fams))
}
