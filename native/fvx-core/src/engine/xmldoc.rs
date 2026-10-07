use serde_json::{Map, Number, Value as Json};

pub struct El {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub kids: Vec<Node>,
}

impl El {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        text(self)
    }

    pub fn children(&self) -> impl Iterator<Item = &El> {
        elements(self)
    }

    pub fn child(&self, name: &str) -> Option<&El> {
        elements(self).find(|e| e.name == name)
    }
}

pub fn parse(text: &str) -> Result<El, String> {
    let mut r = Reader { s: text.trim_start_matches('\u{feff}'), i: 0 };
    r.skip_misc()?;
    r.element()
}

pub enum Node {
    El(El),
    Text(String),
}

struct Reader<'a> {
    s: &'a str,
    i: usize,
}

fn unescape(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';') else {
            out.push_str(rest);
            return out;
        };
        let ent = &rest[1..end];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&rest[..=end]),
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

impl<'a> Reader<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }

    fn skip_ws(&mut self) {
        let r = self.rest();
        self.i += r.len() - r.trim_start().len();
    }

    fn skip_until(&mut self, end: &str) -> Result<(), String> {
        match self.rest().find(end) {
            Some(at) => {
                self.i += at + end.len();
                Ok(())
            }
            None => Err(format!("the XML ends inside a section that needs `{end}`")),
        }
    }

    fn skip_misc(&mut self) -> Result<(), String> {
        loop {
            self.skip_ws();
            let r = self.rest();
            if r.starts_with("<?") {
                self.skip_until("?>")?;
            } else if r.starts_with("<!--") {
                self.skip_until("-->")?;
            } else if r.starts_with("<!") {
                self.skip_until(">")?;
            } else {
                return Ok(());
            }
        }
    }

    fn name(&mut self) -> String {
        let r = self.rest();
        let n = r.find(|c: char| c.is_whitespace() || c == '>' || c == '/' || c == '=').unwrap_or(r.len());
        self.i += n;
        r[..n].to_string()
    }

    fn element(&mut self) -> Result<El, String> {
        if !self.rest().starts_with('<') {
            return Err(format!("expected an element at byte {}", self.i));
        }
        self.i += 1;
        let name = self.name();
        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            let r = self.rest();
            if r.starts_with("/>") {
                self.i += 2;
                return Ok(El { name, attrs, kids: Vec::new() });
            }
            if r.starts_with('>') {
                self.i += 1;
                break;
            }
            if r.is_empty() {
                return Err(format!("the XML ends inside <{name}>"));
            }
            let key = self.name();
            self.skip_ws();
            if !self.rest().starts_with('=') {
                return Err(format!("attribute `{key}` of <{name}> has no value"));
            }
            self.i += 1;
            self.skip_ws();
            let q = self.rest().chars().next().ok_or("the XML ends in an attribute")?;
            if q != '"' && q != '\'' {
                return Err(format!("attribute `{key}` of <{name}> is not quoted"));
            }
            self.i += 1;
            let r = self.rest();
            let end = r.find(q).ok_or_else(|| format!("attribute `{key}` of <{name}> is not closed"))?;
            attrs.push((key, unescape(&r[..end])));
            self.i += end + 1;
        }
        let mut kids = Vec::new();
        loop {
            let r = self.rest();
            if r.is_empty() {
                return Err(format!("<{name}> is never closed"));
            }
            if r.starts_with("</") {
                self.i += 2;
                let close = self.name();
                if close != name {
                    return Err(format!("<{name}> is closed by </{close}>"));
                }
                self.skip_until(">")?;
                return Ok(El { name, attrs, kids });
            }
            if r.starts_with("<!--") {
                self.skip_until("-->")?;
            } else if r.starts_with("<![CDATA[") {
                self.i += 9;
                let r = self.rest();
                let end = r.find("]]>").ok_or("a CDATA section is never closed")?;
                kids.push(Node::Text(r[..end].to_string()));
                self.i += end + 3;
            } else if r.starts_with("<?") {
                self.skip_until("?>")?;
            } else if r.starts_with('<') {
                kids.push(Node::El(self.element()?));
            } else {
                let end = r.find('<').unwrap_or(r.len());
                kids.push(Node::Text(unescape(&r[..end])));
                self.i += end;
            }
        }
    }
}

fn text(el: &El) -> String {
    el.kids.iter().filter_map(|k| if let Node::Text(t) = k { Some(t.as_str()) } else { None }).collect()
}

fn elements(el: &El) -> impl Iterator<Item = &El> {
    el.kids.iter().filter_map(|k| if let Node::El(e) = k { Some(e) } else { None })
}

fn number(raw: &str) -> Result<Json, String> {
    let t = raw.trim();
    if let Ok(i) = t.parse::<i64>() {
        return Ok(Json::Number(i.into()));
    }
    if let Ok(u) = t.parse::<u64>() {
        return Ok(Json::Number(u.into()));
    }
    let f: f64 = t.parse().map_err(|_| format!("`{t}` is not a number"))?;
    Ok(Number::from_f64(f).map(Json::Number).unwrap_or(Json::Null))
}

fn value(el: &El) -> Result<Json, String> {
    match el.name.as_str() {
        "null" => Ok(Json::Null),
        "bool" => Ok(Json::Bool(text(el).trim() == "true")),
        "num" => number(&text(el)),
        "str" => Ok(Json::String(text(el))),
        "arr" => elements(el).map(value).collect::<Result<Vec<_>, _>>().map(Json::Array),
        "obj" => {
            let mut m = Map::new();
            for member in elements(el) {
                if member.name != "m" {
                    return Err(format!("<obj> holds <{}> where a member <m> belongs", member.name));
                }
                let key = member.attrs.iter().find(|(k, _)| k == "n").map(|(_, v)| v.clone()).ok_or("a member <m> has no `n` name")?;
                let v = match elements(member).next() {
                    Some(inner) => value(inner)?,
                    None => Json::Null,
                };
                m.insert(key, v);
            }
            Ok(Json::Object(m))
        }
        other => Err(format!("<{other}> is not a value element")),
    }
}

pub fn looks_like_xml(text: &str) -> bool {
    text.trim_start_matches('\u{feff}').trim_start().starts_with('<')
}

pub fn to_json(text: &str) -> Result<Json, String> {
    let mut r = Reader { s: text.trim_start_matches('\u{feff}'), i: 0 };
    r.skip_misc()?;
    let root = r.element()?;
    let body = elements(&root).next().ok_or_else(|| format!("<{}> holds no project", root.name))?;
    value(body)
}
