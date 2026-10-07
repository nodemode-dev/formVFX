use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use fancy_regex::Regex;

use super::interp::*;

thread_local! {
    static CACHE: RefCell<HashMap<String, Rc<Regex>>> = RefCell::new(HashMap::new());
}

const MARK: &str = "__regexp";

pub fn compile(pattern: &str, flags: &str) -> Result<Rc<Regex>, Flow> {
    let key = format!("{flags}/{pattern}");
    if let Some(r) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Ok(r);
    }
    let mut inline = String::new();
    for (f, c) in [('i', 'i'), ('m', 'm'), ('s', 's')] {
        if flags.contains(f) {
            inline.push(c);
        }
    }
    let src = translate(pattern);
    let full = if inline.is_empty() { src } else { format!("(?{inline}){src}") };
    let re = Regex::new(&full).map_err(|e| throw("SyntaxError", format!("invalid regular expression /{pattern}/: {e}")))?;
    let re = Rc::new(re);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 256 {
            c.clear();
        }
        c.insert(key, re.clone());
    });
    Ok(re)
}

fn translate(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    let chars: Vec<char> = p.chars().collect();
    let mut i = 0;
    let mut in_class = false;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if i + 1 < chars.len() => {
                let n = chars[i + 1];
                match n {
                    'd' | 'D' | 'w' | 'W' | 's' | 'S' | 'b' | 'B' => {
                        out.push('\\');
                        out.push(n);
                    }
                    '/' => out.push('/'),
                    _ => {
                        out.push('\\');
                        out.push(n);
                    }
                }
                i += 2;
                continue;
            }
            '[' if !in_class => {
                in_class = true;
                if chars.get(i + 1) == Some(&']') {
                    out.push_str("[^\\s\\S]");
                    i += 2;
                    in_class = false;
                    continue;
                }
                out.push('[');
            }
            ']' if in_class => {
                in_class = false;
                out.push(']');
            }
            '[' if in_class => out.push_str("\\["),
            '(' if !in_class && chars.get(i + 1) == Some(&'?') && chars.get(i + 2) == Some(&'<') && chars.get(i + 3).is_some_and(|x| *x != '=' && *x != '!') => {
                out.push_str("(?P<");
                i += 3;
                continue;
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out
}

pub fn make(pattern: &str, flags: &str) -> V {
    let o = obj(vec![
        (MARK, V::Bool(true)),
        ("source", s(pattern)),
        ("flags", s(flags)),
        ("global", V::Bool(flags.contains('g'))),
        ("ignoreCase", V::Bool(flags.contains('i'))),
        ("multiline", V::Bool(flags.contains('m'))),
        ("sticky", V::Bool(flags.contains('y'))),
        ("unicode", V::Bool(flags.contains('u'))),
        ("lastIndex", V::Num(0.0)),
    ]);
    if let V::Obj(ob) = &o {
        let mut b = ob.borrow_mut();
        b.set("test", native("test", o.clone(), re_test));
        b.set("exec", native("exec", o.clone(), re_exec));
        b.set("toString", native("toString", o.clone(), re_to_string));
    }
    o
}

pub fn construct(it: &mut Interp, a: Vec<V>) -> R {
    let first = arg(&a, 0);
    if let Some((p, f)) = parts(&first) {
        let flags = match arg(&a, 1) {
            V::Undef => f,
            v => it.to_string(&v)?,
        };
        compile(&p, &flags)?;
        return Ok(make(&p, &flags));
    }
    let pattern = match first {
        V::Undef => "(?:)".to_string(),
        v => it.to_string(&v)?,
    };
    let flags = match arg(&a, 1) {
        V::Undef => String::new(),
        v => it.to_string(&v)?,
    };
    if let Some(bad) = flags.chars().find(|c| !"gimsuyd".contains(*c)) {
        return Err(throw("SyntaxError", format!("invalid regular expression flag `{bad}`")));
    }
    compile(&pattern, &flags)?;
    Ok(make(&pattern, &flags))
}

pub fn parts(v: &V) -> Option<(String, String)> {
    let V::Obj(o) = v else { return None };
    let b = o.borrow();
    b.get(MARK)?;
    Some((plain_string(b.get("source")?), plain_string(b.get("flags")?)))
}

fn last_index(v: &V) -> usize {
    match v {
        V::Obj(o) => match o.borrow().get("lastIndex") {
            Some(V::Num(n)) if *n >= 0.0 => *n as usize,
            _ => 0,
        },
        _ => 0,
    }
}

fn set_last_index(v: &V, n: usize) {
    if let V::Obj(o) = v {
        o.borrow_mut().set("lastIndex", V::Num(n as f64));
    }
}

fn utf16_to_byte(t: &str, u: usize) -> usize {
    let mut count = 0;
    for (b, ch) in t.char_indices() {
        if count >= u {
            return b;
        }
        count += ch.len_utf16();
    }
    t.len()
}

fn byte_to_utf16(t: &str, b: usize) -> usize {
    t[..b.min(t.len())].encode_utf16().count()
}

struct Hit {
    start: usize,
    end: usize,
    groups: Vec<Option<(usize, usize)>>,
    names: Vec<Option<String>>,
}

fn find_at(re: &Regex, t: &str, from: usize) -> Result<Option<Hit>, Flow> {
    if from > t.len() {
        return Ok(None);
    }
    let caps = re.captures_from_pos(t, from).map_err(|e| throw("Error", format!("regular expression failed: {e}")))?;
    Ok(caps.map(|c| {
        let m = c.get(0).map(|m| (m.start(), m.end())).unwrap_or((from, from));
        let names: Vec<Option<String>> = re.capture_names().map(|n| n.map(str::to_string)).collect();
        Hit { start: m.0, end: m.1, groups: (1..c.len()).map(|i| c.get(i).map(|g| (g.start(), g.end()))).collect(), names }
    }))
}

fn hit_array(t: &str, h: &Hit) -> V {
    let mut items = vec![s(&t[h.start..h.end])];
    for g in &h.groups {
        items.push(match g {
            Some((a, b)) => s(&t[*a..*b]),
            None => V::Undef,
        });
    }
    arr(items)
}

fn re_test(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let r = re_exec(it, this, a)?;
    Ok(V::Bool(!matches!(r, V::Null)))
}

fn re_exec(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (p, f) = parts(this).ok_or_else(|| type_err("exec needs a regular expression"))?;
    let re = compile(&p, &f)?;
    let t = it.to_string(&arg(&a, 0))?;
    let stateful = f.contains('g') || f.contains('y');
    let from = if stateful { utf16_to_byte(&t, last_index(this)) } else { 0 };
    let hit = find_at(&re, &t, from)?;
    match hit {
        Some(h) if !f.contains('y') || h.start == from => {
            if stateful {
                let next = if h.end == h.start { h.end + t[h.end..].chars().next().map(char::len_utf8).unwrap_or(1) } else { h.end };
                set_last_index(this, byte_to_utf16(&t, next));
            }
            Ok(hit_array(&t, &h))
        }
        _ => {
            if stateful {
                set_last_index(this, 0);
            }
            Ok(V::Null)
        }
    }
}

fn re_to_string(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    let (p, f) = parts(this).unwrap_or_default();
    Ok(s(&format!("/{p}/{f}")))
}

fn all_hits(re: &Regex, t: &str) -> Result<Vec<Hit>, Flow> {
    let mut out = vec![];
    let mut from = 0;
    while from <= t.len() {
        let Some(h) = find_at(re, t, from)? else { break };
        from = if h.end == h.start { h.end + t[h.end..].chars().next().map(char::len_utf8).unwrap_or(1) } else { h.end };
        out.push(h);
        if out.len() > 100_000 {
            break;
        }
    }
    Ok(out)
}

pub fn str_match(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let t = plain_string(this);
    let (p, f) = match parts(&arg(&a, 0)) {
        Some(x) => x,
        None => (regex_escape(&it.to_string(&arg(&a, 0))?), String::new()),
    };
    let re = compile(&p, &f)?;
    if f.contains('g') {
        let hits = all_hits(&re, &t)?;
        if hits.is_empty() {
            return Ok(V::Null);
        }
        return Ok(arr(hits.iter().map(|h| s(&t[h.start..h.end])).collect()));
    }
    Ok(match find_at(&re, &t, 0)? {
        Some(h) => hit_array(&t, &h),
        None => V::Null,
    })
}

pub fn str_match_all(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let t = plain_string(this);
    let (p, f) = match parts(&arg(&a, 0)) {
        Some(x) => x,
        None => (regex_escape(&it.to_string(&arg(&a, 0))?), "g".to_string()),
    };
    let re = compile(&p, &f)?;
    Ok(arr(all_hits(&re, &t)?.iter().map(|h| hit_array(&t, h)).collect()))
}

pub fn str_search(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let t = plain_string(this);
    let (p, f) = match parts(&arg(&a, 0)) {
        Some(x) => x,
        None => (regex_escape(&it.to_string(&arg(&a, 0))?), String::new()),
    };
    let re = compile(&p, &f)?;
    Ok(V::Num(match find_at(&re, &t, 0)? {
        Some(h) => byte_to_utf16(&t, h.start) as f64,
        None => -1.0,
    }))
}

fn regex_escape(t: &str) -> String {
    let mut out = String::new();
    for c in t.chars() {
        if "\\^$.|?*+()[]{}/".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn expand(rep: &str, t: &str, h: &Hit) -> String {
    let chars: Vec<char> = rep.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let group = |n: usize| -> &str { h.groups.get(n.wrapping_sub(1)).copied().flatten().map(|(a, b)| &t[a..b]).unwrap_or("") };
    while i < chars.len() {
        let c = chars[i];
        if c != '$' || i + 1 >= chars.len() {
            out.push(c);
            i += 1;
            continue;
        }
        let n = chars[i + 1];
        match n {
            '$' => {
                out.push('$');
                i += 2;
            }
            '&' => {
                out.push_str(&t[h.start..h.end]);
                i += 2;
            }
            '`' => {
                out.push_str(&t[..h.start]);
                i += 2;
            }
            '\'' => {
                out.push_str(&t[h.end..]);
                i += 2;
            }
            '<' => {
                let close = chars[i + 2..].iter().position(|x| *x == '>');
                match close {
                    Some(k) => {
                        let name: String = chars[i + 2..i + 2 + k].iter().collect();
                        let idx = h.names.iter().position(|x| x.as_deref() == Some(name.as_str()));
                        if let Some(ix) = idx {
                            out.push_str(group(ix));
                        }
                        i += 3 + k;
                    }
                    None => {
                        out.push('$');
                        i += 1;
                    }
                }
            }
            d if d.is_ascii_digit() => {
                let mut num = d.to_digit(10).unwrap_or(0) as usize;
                let mut used = 2;
                if let Some(d2) = chars.get(i + 2).and_then(|x| x.to_digit(10)) {
                    let two = num * 10 + d2 as usize;
                    if two >= 1 && two <= h.groups.len() {
                        num = two;
                        used = 3;
                    }
                }
                if num >= 1 && num <= h.groups.len() {
                    out.push_str(group(num));
                } else {
                    out.push('$');
                    out.push(d);
                }
                i += used;
            }
            _ => {
                out.push('$');
                i += 1;
            }
        }
    }
    out
}

pub fn replace(it: &mut Interp, t: &str, re_v: &V, rep: &V, force_all: bool) -> R {
    let (p, f) = parts(re_v).ok_or_else(|| type_err("replace needs a regular expression"))?;
    let re = compile(&p, &f)?;
    let hits = if f.contains('g') || force_all {
        all_hits(&re, t)?
    } else {
        find_at(&re, t, 0)?.into_iter().collect()
    };
    let mut out = String::new();
    let mut last = 0;
    for h in &hits {
        out.push_str(&t[last..h.start]);
        let r = match rep {
            V::Func(_) | V::Native(_) => {
                let mut args = vec![s(&t[h.start..h.end])];
                for g in &h.groups {
                    args.push(match g {
                        Some((a, b)) => s(&t[*a..*b]),
                        None => V::Undef,
                    });
                }
                args.push(V::Num(byte_to_utf16(t, h.start) as f64));
                args.push(s(t));
                let v = it.call(rep, V::Undef, args)?;
                it.to_string(&v)?
            }
            other => {
                let r = it.to_string(other)?;
                expand(&r, t, h)
            }
        };
        out.push_str(&r);
        last = h.end;
    }
    out.push_str(&t[last..]);
    Ok(s(&out))
}

pub fn split(it: &mut Interp, t: &str, re_v: &V, limit: usize) -> R {
    let _ = it;
    let (p, f) = parts(re_v).ok_or_else(|| type_err("split needs a regular expression"))?;
    let re = compile(&p, &f)?;
    if t.is_empty() {
        return Ok(if find_at(&re, t, 0)?.is_some() { arr(vec![]) } else { arr(vec![s("")]) });
    }
    let mut out = vec![];
    let mut last = 0;
    let mut from = 0;
    while from < t.len() && out.len() < limit {
        let Some(h) = find_at(&re, t, from)? else { break };
        if h.end == h.start && h.start == last {
            from = h.end + t[h.end..].chars().next().map(char::len_utf8).unwrap_or(1);
            if h.start >= t.len() {
                break;
            }
            continue;
        }
        if h.start >= t.len() {
            break;
        }
        out.push(s(&t[last..h.start]));
        for g in &h.groups {
            out.push(match g {
                Some((a, b)) => s(&t[*a..*b]),
                None => V::Undef,
            });
        }
        last = h.end;
        from = if h.end == h.start { h.end + t[h.end..].chars().next().map(char::len_utf8).unwrap_or(1) } else { h.end };
    }
    if out.len() < limit {
        out.push(s(&t[last..]));
    }
    out.truncate(limit);
    Ok(arr(out))
}
