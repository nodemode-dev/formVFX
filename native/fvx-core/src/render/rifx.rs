use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct Chunk {
    pub id: String,
    pub list: Option<String>,
    pub data: Vec<u8>,
    pub kids: Vec<Chunk>,
}

const RAW_LISTS: [&str; 1] = ["btdk"];

pub fn parse(b: &[u8]) -> Result<(String, Vec<Chunk>), String> {
    if b.len() < 12 {
        return Err("the file is too short to be a RIFX container".into());
    }
    let big = match &b[0..4] {
        b"RIFX" => true,
        b"RIFF" => false,
        _ => return Err("not a RIFX container".into()),
    };
    let size = rd32(&b[4..8], big) as usize;
    let end = (8 + size).min(b.len());
    let form = String::from_utf8_lossy(&b[8..12]).to_string();
    Ok((form, chunks(b, 12, end, big, 0)?))
}

fn rd32(b: &[u8], big: bool) -> u32 {
    let a = [b[0], b[1], b[2], b[3]];
    if big { u32::from_be_bytes(a) } else { u32::from_le_bytes(a) }
}

fn chunks(b: &[u8], mut off: usize, end: usize, big: bool, depth: usize) -> Result<Vec<Chunk>, String> {
    if depth > 64 {
        return Err("RIFX lists nest too deeply".into());
    }
    let mut out = vec![];
    while off + 8 <= end {
        let id = String::from_utf8_lossy(&b[off..off + 4]).to_string();
        let size = rd32(&b[off + 4..off + 8], big) as usize;
        let body = off + 8;
        let stop = body.checked_add(size).filter(|s| *s <= end).ok_or_else(|| format!("chunk `{id}` runs past the end of its list"))?;
        if id == "LIST" && size >= 4 {
            let lt = String::from_utf8_lossy(&b[body..body + 4]).to_string();
            let (kids, data) = if RAW_LISTS.contains(&lt.as_str()) { (vec![], b[body + 4..stop].to_vec()) } else { (chunks(b, body + 4, stop, big, depth + 1)?, vec![]) };
            out.push(Chunk { id, list: Some(lt), data, kids });
        } else {
            out.push(Chunk { id, list: None, data: b[body..stop].to_vec(), kids: vec![] });
        }
        off = stop + (size & 1);
    }
    Ok(out)
}

impl Chunk {
    pub fn is(&self, id: &str) -> bool {
        self.list.is_none() && self.id == id
    }

    pub fn is_list(&self, lt: &str) -> bool {
        self.list.as_deref() == Some(lt)
    }

    pub fn kid(&self, id: &str) -> Option<&Chunk> {
        self.kids.iter().find(|k| k.is(id))
    }

    pub fn sub(&self, lt: &str) -> Option<&Chunk> {
        self.kids.iter().find(|k| k.is_list(lt))
    }

    pub fn subs<'a>(&'a self, lt: &'a str) -> impl Iterator<Item = &'a Chunk> + 'a {
        self.kids.iter().filter(move |k| k.is_list(lt))
    }

    pub fn utf8(&self) -> Option<String> {
        self.kid("Utf8").map(|k| String::from_utf8_lossy(&k.data).to_string())
    }

    pub fn utf8s(&self) -> Vec<String> {
        self.kids.iter().filter(|k| k.is("Utf8")).map(|k| String::from_utf8_lossy(&k.data).to_string()).collect()
    }

    pub fn find_all<'a>(&'a self, lt: &str, out: &mut Vec<&'a Chunk>) {
        for k in &self.kids {
            if k.is_list(lt) {
                out.push(k);
            }
            k.find_all(lt, out);
        }
    }
}

pub fn cstr(b: &[u8]) -> String {
    let n = b.iter().position(|c| *c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..n]).to_string()
}

pub fn be16(b: &[u8], at: usize) -> u16 {
    b.get(at..at + 2).map(|s| u16::from_be_bytes([s[0], s[1]])).unwrap_or(0)
}

pub fn be32(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]])).unwrap_or(0)
}

pub fn bei32(b: &[u8], at: usize) -> i32 {
    be32(b, at) as i32
}

pub fn bef64(b: &[u8], at: usize) -> f64 {
    b.get(at..at + 8).map(|s| f64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]])).unwrap_or(0.0)
}

pub fn bef32(b: &[u8], at: usize) -> f32 {
    b.get(at..at + 4).map(|s| f32::from_be_bytes([s[0], s[1], s[2], s[3]])).unwrap_or(0.0)
}

pub fn f64s(b: &[u8]) -> Vec<f64> {
    (0..b.len() / 8).map(|i| bef64(b, i * 8)).collect()
}

pub fn pascal_utf8(b: &[u8]) -> String {
    if b.len() >= 8 && &b[..4] == b"Utf8" {
        let n = be32(b, 4) as usize;
        return String::from_utf8_lossy(&b[8..(8 + n).min(b.len())]).to_string();
    }
    cstr(b)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Cos {
    Dict(BTreeMap<String, Cos>),
    Array(Vec<Cos>),
    Str(String),
    Name(String),
    Num(f64),
    Bool(bool),
    Nil,
}

impl Cos {
    pub fn get(&self, k: &str) -> Option<&Cos> {
        match self {
            Cos::Dict(d) => d.get(k),
            Cos::Array(a) => k.parse::<usize>().ok().and_then(|i| a.get(i)),
            _ => None,
        }
    }

    pub fn at(&self, path: &[&str]) -> Option<&Cos> {
        path.iter().try_fold(self, |c, k| c.get(k))
    }

    pub fn num(&self) -> Option<f64> {
        match self {
            Cos::Num(n) => Some(*n),
            Cos::Bool(b) => Some(*b as u8 as f64),
            _ => None,
        }
    }

    pub fn bool(&self) -> Option<bool> {
        match self {
            Cos::Bool(b) => Some(*b),
            Cos::Num(n) => Some(*n != 0.0),
            _ => None,
        }
    }

    pub fn str(&self) -> Option<&str> {
        match self {
            Cos::Str(s) | Cos::Name(s) => Some(s),
            _ => None,
        }
    }

    pub fn arr(&self) -> &[Cos] {
        match self {
            Cos::Array(a) => a,
            _ => &[],
        }
    }

    pub fn nums(&self) -> Vec<f64> {
        self.arr().iter().filter_map(Cos::num).collect()
    }
}

struct CosReader<'a> {
    b: &'a [u8],
    i: usize,
    depth: usize,
}

impl CosReader<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\r' | b'\n' | 0) {
            self.i += 1;
        }
    }

    fn peek(&self, s: &[u8]) -> bool {
        self.b[self.i..].starts_with(s)
    }

    fn token(&mut self) -> String {
        let j = self.i;
        while self.i < self.b.len() && !matches!(self.b[self.i], b' ' | b'\t' | b'\r' | b'\n' | b'/' | b'[' | b']' | b'<' | b'>' | b'(' | b')') {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.b[j..self.i]).to_string()
    }

    fn value(&mut self) -> Result<Cos, String> {
        self.ws();
        if self.i >= self.b.len() {
            return Err("text document ends early".into());
        }
        self.depth += 1;
        if self.depth > 256 {
            return Err("text document nests too deeply".into());
        }
        let v = if self.peek(b"<<") {
            self.i += 2;
            let mut d = BTreeMap::new();
            loop {
                self.ws();
                if self.i >= self.b.len() {
                    return Err("unterminated dictionary in a text document".into());
                }
                if self.peek(b">>") {
                    self.i += 2;
                    break;
                }
                let k = match self.value()? {
                    Cos::Name(n) => n,
                    Cos::Num(n) => format!("{n}"),
                    Cos::Str(s) => s,
                    _ => return Err("a dictionary key in a text document is not a name".into()),
                };
                let v = self.value()?;
                d.insert(k, v);
            }
            Cos::Dict(d)
        } else if self.peek(b"[") {
            self.i += 1;
            let mut a = vec![];
            loop {
                self.ws();
                if self.i >= self.b.len() {
                    return Err("unterminated array in a text document".into());
                }
                if self.peek(b"]") {
                    self.i += 1;
                    break;
                }
                a.push(self.value()?);
            }
            Cos::Array(a)
        } else if self.peek(b"(") {
            self.i += 1;
            let mut raw = vec![];
            while self.i < self.b.len() && self.b[self.i] != b')' {
                if self.b[self.i] == b'\\' && self.i + 1 < self.b.len() {
                    self.i += 1;
                }
                raw.push(self.b[self.i]);
                self.i += 1;
            }
            self.i += 1;
            Cos::Str(decode_text(&raw))
        } else if self.peek(b"/") {
            self.i += 1;
            Cos::Name(self.token())
        } else {
            let t = self.token();
            if t.is_empty() {
                self.i += 1;
                Cos::Nil
            } else {
                match t.as_str() {
                    "true" => Cos::Bool(true),
                    "false" => Cos::Bool(false),
                    _ => t.parse::<f64>().map(Cos::Num).unwrap_or(Cos::Name(t)),
                }
            }
        };
        self.depth -= 1;
        Ok(v)
    }
}

fn decode_text(raw: &[u8]) -> String {
    if raw.len() >= 2 && raw[0] == 0xfe && raw[1] == 0xff {
        let units: Vec<u16> = raw[2..].chunks(2).filter(|c| c.len() == 2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        return String::from_utf16_lossy(&units);
    }
    raw.iter().map(|c| *c as char).collect()
}

pub fn cos(b: &[u8]) -> Result<Cos, String> {
    let mut r = CosReader { b, i: 0, depth: 0 };
    let mut top = BTreeMap::new();
    loop {
        r.ws();
        if r.i >= b.len() {
            break;
        }
        let k = match r.value()? {
            Cos::Name(n) => n,
            Cos::Num(n) => format!("{n}"),
            _ => return Err("a text document key is not a name".into()),
        };
        let v = r.value()?;
        top.insert(k, v);
    }
    Ok(Cos::Dict(top))
}
