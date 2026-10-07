#[derive(Clone, Debug, PartialEq)]
pub enum Y {
    Null,
    Str(String),
    Seq(Vec<Y>),
    Map(Vec<(String, Y)>),
    Tagged(String, Box<Y>),
}

impl Y {
    pub fn untag(&self) -> &Y {
        match self {
            Y::Tagged(_, v) => v.untag(),
            v => v,
        }
    }

    pub fn tag(&self) -> Option<&str> {
        match self {
            Y::Tagged(t, _) => Some(t.as_str()),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Y> {
        match self.untag() {
            Y::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn str(&self) -> Option<&str> {
        match self.untag() {
            Y::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn num(&self) -> Option<f64> {
        self.str().and_then(|s| s.trim().parse::<f64>().ok())
    }

    pub fn bool(&self) -> Option<bool> {
        match self.str()?.trim() {
            "true" | "True" | "TRUE" | "yes" | "on" => Some(true),
            "false" | "False" | "FALSE" | "no" | "off" => Some(false),
            _ => None,
        }
    }

    pub fn seq(&self) -> &[Y] {
        match self.untag() {
            Y::Seq(v) => v,
            _ => &[],
        }
    }

    pub fn map(&self) -> &[(String, Y)] {
        match self.untag() {
            Y::Map(m) => m,
            _ => &[],
        }
    }

    pub fn nums(&self) -> Vec<f64> {
        match self.untag() {
            Y::Seq(v) => v.iter().filter_map(Y::num).collect(),
            Y::Str(_) => self.num().into_iter().collect(),
            _ => vec![],
        }
    }

    pub fn strs(&self) -> Vec<String> {
        match self.untag() {
            Y::Seq(v) => v.iter().filter_map(|x| x.str().map(str::to_string)).collect(),
            Y::Str(s) => s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
            _ => vec![],
        }
    }
}

struct Line {
    no: usize,
    indent: usize,
    text: String,
}

struct Doc {
    lines: Vec<Line>,
    raw: Vec<String>,
    at: usize,
}

fn strip_comment(s: &str) -> &str {
    let b = s.as_bytes();
    let (mut sq, mut dq) = (false, false);
    for i in 0..b.len() {
        match b[i] {
            b'\'' if !dq => sq = !sq,
            b'"' if !sq => dq = !dq,
            b'#' if !sq && !dq && (i == 0 || b[i - 1] == b' ' || b[i - 1] == b'\t') => return s[..i].trim_end(),
            _ => {}
        }
    }
    s.trim_end()
}

fn key_split(s: &str) -> Option<(String, &str)> {
    let b = s.as_bytes();
    if b.is_empty() || matches!(b[0], b'[' | b'{' | b'!' | b'|' | b'>' | b'#') || s == "-" || s.starts_with("- ") {
        return None;
    }
    if b[0] == b'"' || b[0] == b'\'' {
        let q = b[0];
        let mut i = 1;
        while i < b.len() {
            if b[i] == q {
                if q == b'\'' && i + 1 < b.len() && b[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                if q == b'"' && b[i - 1] == b'\\' {
                    i += 1;
                    continue;
                }
                break;
            }
            i += 1;
        }
        let rest = s.get(i + 1..)?.trim_start();
        let rest = rest.strip_prefix(':')?;
        if !rest.is_empty() && !rest.starts_with(' ') {
            return None;
        }
        return Some((unquote(&s[..=i]), rest.trim_start()));
    }
    let mut i = 0;
    while i < b.len() {
        if b[i] == b':' && (i + 1 == b.len() || b[i + 1] == b' ' || b[i + 1] == b'\t') {
            return Some((s[..i].trim().to_string(), s[i + 1..].trim_start()));
        }
        if matches!(b[i], b'[' | b'{') {
            return None;
        }
        i += 1;
    }
    None
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
        return t[1..t.len() - 1].replace("''", "'");
    }
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        let inner = &t[1..t.len() - 1];
        let mut out = String::with_capacity(inner.len());
        let mut it = inner.chars();
        while let Some(c) = it.next() {
            if c == '\\' {
                match it.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some(o) => {
                        out.push('\\');
                        out.push(o);
                    }
                    None => {}
                }
            } else {
                out.push(c);
            }
        }
        return out;
    }
    t.to_string()
}

fn scalar(s: &str) -> Y {
    let t = s.trim();
    if t.is_empty() || t == "~" || t == "null" || t == "Null" || t == "NULL" {
        return Y::Null;
    }
    Y::Str(unquote(t))
}

fn split_tag(s: &str) -> (Option<String>, &str) {
    let t = s.trim_start();
    if let Some(r) = t.strip_prefix("!<")
        && let Some(e) = r.find('>')
    {
        return (Some(r[..e].to_string()), r[e + 1..].trim_start());
    }
    if let Some(r) = t.strip_prefix('!') {
        let e = r.find([' ', '{', '[']).unwrap_or(r.len());
        return (Some(r[..e].to_string()), r[e..].trim_start());
    }
    (None, t)
}

fn wrap(tag: Option<String>, v: Y) -> Y {
    match tag {
        Some(t) => Y::Tagged(t, Box::new(v)),
        None => v,
    }
}

struct Flow<'a> {
    b: &'a [u8],
    s: &'a str,
    i: usize,
}

impl Flow<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn value(&mut self, in_map_key: bool) -> Result<Y, String> {
        self.ws();
        let mut tag = None;
        if self.i < self.b.len() && self.b[self.i] == b'!' {
            let (t, _) = split_tag(&self.s[self.i..]);
            if self.s[self.i..].starts_with("!<") {
                let e = self.s[self.i..].find('>').ok_or("an unclosed tag")?;
                self.i += e + 1;
            } else {
                while self.i < self.b.len() && !matches!(self.b[self.i], b' ' | b'{' | b'[' | b',' | b']' | b'}') {
                    self.i += 1;
                }
            }
            tag = t;
            self.ws();
        }
        if self.i >= self.b.len() {
            return Ok(wrap(tag, Y::Null));
        }
        let v = match self.b[self.i] {
            b'[' => {
                self.i += 1;
                let mut out = vec![];
                loop {
                    self.ws();
                    if self.i >= self.b.len() {
                        return Err("a flow sequence is never closed".into());
                    }
                    if self.b[self.i] == b']' {
                        self.i += 1;
                        break;
                    }
                    out.push(self.value(false)?);
                    self.ws();
                    if self.i < self.b.len() && self.b[self.i] == b',' {
                        self.i += 1;
                    }
                }
                Y::Seq(out)
            }
            b'{' => {
                self.i += 1;
                let mut out = vec![];
                loop {
                    self.ws();
                    if self.i >= self.b.len() {
                        return Err("a flow mapping is never closed".into());
                    }
                    if self.b[self.i] == b'}' {
                        self.i += 1;
                        break;
                    }
                    let k = match self.value(true)? {
                        Y::Str(s) => s,
                        Y::Null => String::new(),
                        _ => return Err("a flow mapping key is not a string".into()),
                    };
                    self.ws();
                    let v = if self.i < self.b.len() && self.b[self.i] == b':' {
                        self.i += 1;
                        self.value(false)?
                    } else {
                        Y::Null
                    };
                    out.push((k, v));
                    self.ws();
                    if self.i < self.b.len() && self.b[self.i] == b',' {
                        self.i += 1;
                    }
                }
                Y::Map(out)
            }
            b'"' | b'\'' => {
                let q = self.b[self.i];
                let st = self.i;
                self.i += 1;
                while self.i < self.b.len() {
                    if self.b[self.i] == b'\\' && q == b'"' {
                        self.i += 2;
                        continue;
                    }
                    if self.b[self.i] == q {
                        if q == b'\'' && self.i + 1 < self.b.len() && self.b[self.i + 1] == b'\'' {
                            self.i += 2;
                            continue;
                        }
                        break;
                    }
                    self.i += 1;
                }
                self.i = (self.i + 1).min(self.b.len());
                Y::Str(unquote(&self.s[st..self.i]))
            }
            _ => {
                let st = self.i;
                while self.i < self.b.len() {
                    let c = self.b[self.i];
                    if matches!(c, b',' | b']' | b'}') {
                        break;
                    }
                    if c == b':' && (in_map_key || self.i + 1 >= self.b.len() || matches!(self.b[self.i + 1], b' ' | b'\n')) {
                        break;
                    }
                    self.i += 1;
                }
                scalar(&self.s[st..self.i])
            }
        };
        Ok(wrap(tag, v))
    }
}

pub fn parse_flow(s: &str) -> Result<Y, String> {
    let mut f = Flow { b: s.as_bytes(), s, i: 0 };
    f.value(false)
}

fn depth_delta(s: &str) -> i32 {
    let (mut d, mut sq, mut dq) = (0, false, false);
    let b = s.as_bytes();
    for i in 0..b.len() {
        match b[i] {
            b'\'' if !dq => sq = !sq,
            b'"' if !sq && (i == 0 || b[i - 1] != b'\\') => dq = !dq,
            b'[' | b'{' if !sq && !dq => d += 1,
            b']' | b'}' if !sq && !dq => d -= 1,
            _ => {}
        }
    }
    d
}

impl Doc {
    fn peek(&self) -> Option<&Line> {
        self.lines.get(self.at)
    }

    fn flow_from(&mut self, first: &str) -> Result<Y, String> {
        let mut text = first.to_string();
        let mut d = depth_delta(first);
        while d > 0 {
            self.at += 1;
            let Some(l) = self.lines.get(self.at) else { return Err("a flow collection is never closed".into()) };
            text.push('\n');
            text.push_str(&l.text);
            d += depth_delta(&l.text);
        }
        parse_flow(&text)
    }

    fn block_scalar(&mut self, header: &str, parent: usize) -> Y {
        let fold = header.starts_with('>');
        let keep = header.contains('+');
        let strip = header.contains('-');
        let start_no = self.lines[self.at].no;
        self.at += 1;
        let mut end_no = start_no;
        while let Some(l) = self.lines.get(self.at) {
            if l.indent <= parent {
                break;
            }
            end_no = l.no;
            self.at += 1;
        }
        let body: Vec<&str> = self.raw[start_no + 1..=end_no.max(start_no)].iter().map(String::as_str).collect();
        let ind = body.iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
        let lines: Vec<String> = body.iter().map(|l| if l.len() >= ind { l[ind..].to_string() } else { String::new() }).collect();
        let mut out = if fold {
            let mut s = String::new();
            for (i, l) in lines.iter().enumerate() {
                if i > 0 {
                    s.push(if l.is_empty() || lines[i - 1].is_empty() { '\n' } else { ' ' });
                }
                s.push_str(l);
            }
            s
        } else {
            lines.join("\n")
        };
        if strip {
            out = out.trim_end().to_string();
        } else if !keep {
            out = out.trim_end().to_string();
            out.push('\n');
        }
        Y::Str(out)
    }

    fn inline_value(&mut self, rest: &str, parent: usize) -> Result<Y, String> {
        let (tag, body) = split_tag(rest);
        if body.is_empty() {
            self.at += 1;
            let nested = match self.peek() {
                Some(l) if l.indent > parent => {
                    let ind = l.indent;
                    self.block(ind)?
                }
                Some(l) if l.indent == parent && (l.text == "-" || l.text.starts_with("- ")) && tag.is_none() => self.block(parent)?,
                _ => Y::Null,
            };
            return Ok(wrap(tag, nested));
        }
        if body.starts_with('|') || body.starts_with('>') {
            return Ok(wrap(tag, self.block_scalar(body, parent)));
        }
        if body.starts_with('[') || body.starts_with('{') {
            let v = self.flow_from(body)?;
            self.at += 1;
            return Ok(wrap(tag, v));
        }
        let mut text = body.to_string();
        if !(body.starts_with('"') || body.starts_with('\'')) {
            while let Some(n) = self.lines.get(self.at + 1) {
                if n.indent > parent && key_split(&n.text).is_none() && !n.text.starts_with("- ") {
                    text.push(' ');
                    text.push_str(&n.text);
                    self.at += 1;
                } else {
                    break;
                }
            }
        }
        self.at += 1;
        Ok(wrap(tag, scalar(&text)))
    }

    fn block(&mut self, indent: usize) -> Result<Y, String> {
        let Some(first) = self.peek() else { return Ok(Y::Null) };
        if first.text == "-" || first.text.starts_with("- ") {
            let mut out = vec![];
            while let Some(l) = self.peek() {
                if l.indent != indent || !(l.text == "-" || l.text.starts_with("- ")) {
                    break;
                }
                let rest = l.text[1..].trim_start().to_string();
                let col = indent + (l.text.len() - rest.len());
                if rest.is_empty() {
                    out.push(self.inline_value("", indent)?);
                    continue;
                }
                let (tag, body) = split_tag(&rest);
                if key_split(body).is_some() {
                    let idx = self.at;
                    self.lines[idx].indent = col + (rest.len() - body.len());
                    self.lines[idx].text = body.to_string();
                    let ind = self.lines[idx].indent;
                    out.push(wrap(tag, self.block(ind)?));
                    continue;
                }
                out.push(self.inline_value(&rest, indent)?);
            }
            return Ok(Y::Seq(out));
        }
        if key_split(&first.text).is_some() {
            let mut out: Vec<(String, Y)> = vec![];
            while let Some(l) = self.peek() {
                if l.indent != indent {
                    if l.indent > indent {
                        return Err(format!("line {}: unexpected indentation", l.no + 1));
                    }
                    break;
                }
                let Some((k, rest)) = key_split(&l.text) else { return Err(format!("line {}: expected `key: value`", l.no + 1)) };
                let rest = rest.to_string();
                let v = self.inline_value(&rest, indent)?;
                out.push((k, v));
            }
            return Ok(Y::Map(out));
        }
        let text = first.text.clone();
        self.inline_value(&text, indent.saturating_sub(1))
    }
}

pub fn parse(text: &str) -> Result<Y, String> {
    let raw: Vec<String> = text.trim_start_matches('\u{feff}').lines().map(|l| l.replace('\t', "    ")).collect();
    let mut lines = vec![];
    for (no, r) in raw.iter().enumerate() {
        let t = r.trim_start();
        if t.is_empty() || t.starts_with('#') || t == "---" || t.starts_with("%YAML") || t == "..." {
            continue;
        }
        let indent = r.len() - t.len();
        lines.push(Line { no, indent, text: strip_comment(t).to_string() });
    }
    let mut d = Doc { lines, raw, at: 0 };
    let Some(first) = d.peek() else { return Ok(Y::Null) };
    let ind = first.indent;
    let v = d.block(ind)?;
    if let Some(l) = d.peek() {
        return Err(format!("line {}: unexpected content", l.no + 1));
    }
    Ok(v)
}
