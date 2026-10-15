use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pf, ps, pu};
use crate::model::DataTable;
use crate::model::key::{self, Key};
use crate::model::prop::Expr;

fn csv_rows(src: &str) -> Vec<Vec<String>> {
    let first = src.lines().next().unwrap_or("");
    let delim = [',', ';', '\t'].into_iter().max_by_key(|d| first.matches(*d).count()).unwrap_or(',');
    let mut rows: Vec<Vec<String>> = vec![];
    let mut row: Vec<String> = vec![];
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = src.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cell.push('"');
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
        } else if c == '"' {
            quoted = true;
        } else if c == delim {
            row.push(std::mem::take(&mut cell));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut cell));
            rows.push(std::mem::take(&mut row));
        } else {
            cell.push(c);
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|c| !c.trim().is_empty()));
    rows
}

fn numeric(s: &str) -> bool {
    s.trim().parse::<f64>().is_ok()
}

fn with_header(mut rows: Vec<Vec<String>>) -> (Vec<String>, Vec<Vec<String>>) {
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    for r in rows.iter_mut() {
        r.resize(width, String::new());
    }
    if rows.is_empty() {
        return (vec![], rows);
    }
    let headed = rows[0].iter().any(|c| !numeric(c) && !c.trim().is_empty());
    if headed {
        let head = rows.remove(0).into_iter().enumerate().map(|(i, c)| if c.trim().is_empty() { format!("Column {}", i + 1) } else { c.trim().to_string() }).collect();
        (head, rows)
    } else {
        ((0..width).map(|i| format!("Column {}", i + 1)).collect(), rows)
    }
}

fn cell(v: &Json) -> String {
    match v {
        Json::Null => String::new(),
        Json::String(s) => s.clone(),
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

fn json_table(v: &Json) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    match v {
        Json::Array(items) if items.iter().all(Json::is_object) && !items.is_empty() => {
            let mut cols: Vec<String> = vec![];
            for it in items {
                for k in it.as_object().map(|o| o.keys()).into_iter().flatten() {
                    if !cols.contains(k) {
                        cols.push(k.clone());
                    }
                }
            }
            let rows = items.iter().map(|it| cols.iter().map(|c| it.get(c).map(cell).unwrap_or_default()).collect()).collect();
            Ok((cols, rows))
        }
        Json::Array(items) if items.iter().all(Json::is_array) && !items.is_empty() => {
            let rows = items.iter().map(|r| r.as_array().map(|a| a.iter().map(cell).collect()).unwrap_or_default()).collect();
            Ok(with_header(rows))
        }
        Json::Array(items) => Ok((vec!["value".into()], items.iter().map(|x| vec![cell(x)]).collect())),
        Json::Object(o) if o.len() == 1 && o.values().all(Json::is_array) => json_table(o.values().next().ok_or("empty object")?),
        Json::Object(o) if !o.is_empty() && o.values().all(Json::is_array) => {
            let cols: Vec<String> = o.keys().cloned().collect();
            let n = o.values().map(|a| a.as_array().map(Vec::len).unwrap_or(0)).max().unwrap_or(0);
            let rows = (0..n).map(|i| o.values().map(|a| a.get(i).map(cell).unwrap_or_default()).collect()).collect();
            Ok((cols, rows))
        }
        Json::Object(o) => Ok((o.keys().cloned().collect(), vec![o.values().map(cell).collect()])),
        _ => Err("the JSON must be an array or an object".into()),
    }
}

pub fn import(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (a .csv, .tsv or .json file)")?.to_string();
    let src = std::fs::read_to_string(&path).map_err(|err| format!("{path}: {err}"))?;
    let lower = path.to_ascii_lowercase();
    let (columns, rows) = if lower.ends_with(".json") {
        let v: Json = serde_json::from_str(src.trim_start_matches('\u{feff}')).map_err(|err| format!("{path}: {err}"))?;
        json_table(&v)?
    } else {
        with_header(csv_rows(&src))
    };
    if columns.is_empty() || rows.is_empty() {
        return Err(format!("{path} holds no rows"));
    }
    let stem = std::path::Path::new(&path).file_stem().and_then(|s| s.to_str()).unwrap_or("Data").to_string();
    let name = ps(p, "name").map(str::to_string).unwrap_or(stem);
    let pj = e.proj();
    let id = pj.new_id();
    let (n, c) = (rows.len(), columns.len());
    pj.data.push(DataTable { id, name: name.clone(), path, columns: columns.clone(), rows });
    e.set_label("Import Data");
    Ok(json!({"item": id, "name": name, "columns": columns, "rows": n, "width": c}))
}

pub fn list(e: &mut Engine, p: &Json) -> Res {
    let preview = pu(p, "rows").unwrap_or(0) as usize;
    let pick = p.get("item").filter(|v| !v.is_null());
    let out: Vec<Json> = e
        .project
        .data
        .iter()
        .filter(|t| match pick {
            Some(Json::Number(n)) => n.as_u64() == Some(t.id),
            Some(Json::String(s)) => *s == t.name || s.parse::<u64>().ok() == Some(t.id),
            _ => true,
        })
        .map(|t| json!({"item": t.id, "name": t.name, "path": t.path, "columns": t.columns, "rows": t.rows.len(), "preview": t.rows.iter().take(preview).collect::<Vec<_>>()}))
        .collect();
    Ok(json!(out))
}

fn table<'a>(e: &'a Engine, v: &Json) -> Result<&'a DataTable, String> {
    e.project
        .data
        .iter()
        .find(|t| match v {
            Json::Number(n) => n.as_u64() == Some(t.id),
            Json::String(s) => *s == t.name || s.parse::<u64>().ok() == Some(t.id),
            _ => false,
        })
        .ok_or_else(|| "no such data item (data.list shows them)".to_string())
}

fn column(t: &DataTable, v: &Json) -> Result<usize, String> {
    let i = match v {
        Json::Number(n) => n.as_u64().map(|x| x as usize),
        Json::String(s) => t.columns.iter().position(|c| c.eq_ignore_ascii_case(s)).or_else(|| s.parse::<usize>().ok()),
        _ => None,
    };
    i.filter(|i| *i < t.columns.len()).ok_or_else(|| format!("no column {v} (columns: {})", t.columns.join(", ")))
}

pub fn link(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid, uid) = super::behavior::locate(e, p)?;
    let t = table(e, p.get("item").ok_or("pass `item` (data.list shows them)")?)?.clone();
    let wanted = p.get("column").ok_or("pass `column` (a name or index, or a list of them for point properties)")?;
    let cols: Vec<usize> = match wanted {
        Json::Array(a) => a.iter().map(|c| column(&t, c)).collect::<Result<_, _>>()?,
        one => vec![column(&t, one)?],
    };
    let mode = ps(p, "mode").unwrap_or("keyframes");
    if !["keyframes", "expression"].contains(&mode) {
        return Err("mode is keyframes or expression".into());
    }
    let (fps, duration) = {
        let c = e.project.comp(cid).ok_or("no composition")?;
        (c.frame_rate, c.duration)
    };
    let start = pf(p, "start").unwrap_or(0.0);
    let rate = pf(p, "rate").unwrap_or(fps);
    if rate <= 0.0 {
        return Err("rate must be above 0".into());
    }
    let (scale, offset) = (pf(p, "scale").unwrap_or(1.0), pf(p, "offset").unwrap_or(0.0));
    let l = e.layer_ref(cid, lid)?;
    let pr = l.props.find(uid).ok_or("that is a group, not a property")?;
    if !pr.value.is_numeric() {
        return Err(format!("{} is not a number, point or colour property", pr.name));
    }
    let dims = pr.value.dims();
    if cols.len() > dims {
        return Err(format!("{} takes {dims} value(s) but {} columns were given", pr.name, cols.len()));
    }
    if pr.expr.is_some() {
        return Err(format!("{} already has an expression; remove it first", pr.name));
    }
    let series: Vec<Vec<Option<f64>>> = cols.iter().map(|c| t.rows.iter().map(|r| r.get(*c).and_then(|s| s.trim().parse::<f64>().ok()).map(|x| x * scale + offset)).collect()).collect();
    if series.iter().all(|s| s.iter().all(Option::is_none)) {
        return Err("the column holds no numbers".into());
    }
    let rows = t.rows.len();
    match mode {
        "keyframes" => {
            let fresh: Vec<Key> = {
                let l = e.layer_ref(cid, lid)?;
                let pr = l.props.find(uid).ok_or("no property")?;
                let mut keys = vec![];
                for i in 0..rows {
                    let ct = start + i as f64 / rate;
                    if ct > duration + 1.0 / fps {
                        break;
                    }
                    let lt = l.layer_time(ct);
                    let mut comps = pr.at(lt).components();
                    let mut any = false;
                    for (d, s) in series.iter().enumerate() {
                        if let Some(x) = s[i] {
                            comps[d] = x;
                            any = true;
                        }
                    }
                    if any {
                        keys.push(Key::new(lt, pr.value.with_components(&comps), pr.spatial));
                    }
                }
                keys
            };
            let n = fresh.len();
            let l = e.layer_mut(cid, lid)?;
            let pr = l.props.find_mut(uid).ok_or("no property")?;
            pr.keys.clear();
            for mut k in fresh {
                k.spatial_auto = false;
                key::insert(&mut pr.keys, k);
            }
            e.set_label("Link Data to Property");
            Ok(json!({"mode": mode, "keys": n, "rows": rows}))
        }
        _ => {
            let mut data = String::from("[");
            for (d, s) in series.iter().enumerate() {
                if d > 0 {
                    data.push(',');
                }
                data.push('[');
                let mut last = 0.0;
                for (i, x) in s.iter().enumerate() {
                    if i > 0 {
                        data.push(',');
                    }
                    last = x.unwrap_or(last);
                    data.push_str(&last.to_string());
                }
                data.push(']');
            }
            data.push(']');
            let pick = |d: usize| if d < cols.len() { format!("s({d})") } else { format!("value[{d}]") };
            let result = if dims == 1 { pick(0) } else { format!("[{}]", (0..dims).map(pick).collect::<Vec<_>>().join(",")) };
            let text = format!(
                "var __fvxData={data};\nvar f=(time-{start})*{rate};\nvar i0=Math.max(0,Math.min({last}-1,Math.floor(f)));\nvar i1=Math.min({last}-1,i0+1);\nvar u=Math.max(0,Math.min(1,f-i0));\nfunction s(c){{\nvar a=__fvxData[c][i0];\nvar b=__fvxData[c][i1];\nreturn a+(b-a)*u;\n}}\n{result};",
                last = rows
            );
            let l = e.layer_mut(cid, lid)?;
            l.props.find_mut(uid).ok_or("no property")?.expr = Some(Expr { text, enabled: true });
            e.set_label("Link Data to Property");
            let t_now = e.time_of(cid);
            let comp = e.project.comp(cid).ok_or("no composition")?;
            let l = comp.layer(lid).ok_or("no layer")?;
            if let Some(err) = l.props.find(uid).and_then(|pr| crate::expr::check(&crate::eval::Eval::new(&e.project, cid, comp, t_now), l, pr)) {
                return Err(err);
            }
            Ok(json!({"mode": mode, "rows": rows, "source": t.name}))
        }
    }
}
