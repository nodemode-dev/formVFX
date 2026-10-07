#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Num(f64),
    Str(String),
    Tmpl(Vec<String>, Vec<(String, u32)>),
    Regex(String, String),
    Ident(String),
    P(&'static str),
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub nl: bool,
    pub line: u32,
}

const PUNCTS: [&str; 52] = [
    ">>>=", "...", "===", "!==", "**=", "<<=", ">>=", ">>>", "&&=", "||=", "??=", "=>", "==", "!=", "<=", ">=", "&&", "||", "??", "?.", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "**", "<<", ">>", "{", "}", "(", ")", "[", "]", ";", ",", "<", ">", "+", "-", "*", "/", "%", "&", "|", "^", "!",
];
const SINGLE: [&str; 5] = ["~", "?", ":", "=", "."];

pub struct Lexer<'a> {
    src: &'a [u8],
    text: &'a str,
    i: usize,
    line: u32,
}

fn is_id_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

fn is_id_part(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

fn regex_allowed(prev: Option<&Token>) -> bool {
    match prev.map(|t| &t.tok) {
        None => true,
        Some(Tok::Num(_) | Tok::Str(_) | Tok::Tmpl(..) | Tok::Regex(..)) => false,
        Some(Tok::Ident(n)) => matches!(n.as_str(), "return" | "typeof" | "instanceof" | "in" | "of" | "new" | "delete" | "void" | "throw" | "case" | "do" | "else" | "yield" | "await"),
        Some(Tok::P(p)) => !matches!(*p, ")" | "]" | "++" | "--"),
        Some(Tok::Eof) => true,
    }
}

impl<'a> Lexer<'a> {
    pub fn new(text: &'a str, line: u32) -> Lexer<'a> {
        Lexer { src: text.as_bytes(), text, i: 0, line }
    }

    fn peek(&self, k: usize) -> u8 {
        self.src.get(self.i + k).copied().unwrap_or(0)
    }

    fn char_at(&self) -> Option<char> {
        self.text[self.i..].chars().next()
    }

    fn err(&self, msg: &str) -> String {
        format!("SyntaxError: {msg} (line {})", self.line)
    }

    fn skip_space(&mut self) -> Result<bool, String> {
        let mut nl = false;
        loop {
            match self.peek(0) {
                b'\n' => {
                    nl = true;
                    self.line += 1;
                    self.i += 1;
                }
                b' ' | b'\t' | b'\r' | 0x0b | 0x0c => self.i += 1,
                b'/' if self.peek(1) == b'/' => {
                    while self.i < self.src.len() && self.peek(0) != b'\n' {
                        self.i += 1;
                    }
                }
                b'/' if self.peek(1) == b'*' => {
                    self.i += 2;
                    loop {
                        if self.i >= self.src.len() {
                            return Err(self.err("a comment is never closed"));
                        }
                        if self.peek(0) == b'*' && self.peek(1) == b'/' {
                            self.i += 2;
                            break;
                        }
                        if self.peek(0) == b'\n' {
                            nl = true;
                            self.line += 1;
                        }
                        self.i += 1;
                    }
                }
                0xc2 if self.peek(1) == 0xa0 => self.i += 2,
                0xef if self.peek(1) == 0xbb && self.peek(2) == 0xbf => self.i += 3,
                _ => return Ok(nl),
            }
        }
    }

    fn number(&mut self) -> Result<f64, String> {
        let start = self.i;
        if self.peek(0) == b'0' && matches!(self.peek(1), b'x' | b'X' | b'o' | b'O' | b'b' | b'B') {
            let radix = match self.peek(1) {
                b'x' | b'X' => 16,
                b'o' | b'O' => 8,
                _ => 2,
            };
            self.i += 2;
            let ds = self.i;
            while self.peek(0).is_ascii_alphanumeric() || self.peek(0) == b'_' {
                self.i += 1;
            }
            let digits: String = self.text[ds..self.i].chars().filter(|c| *c != '_').collect();
            return u64::from_str_radix(&digits, radix).map(|v| v as f64).map_err(|_| self.err("a malformed number"));
        }
        while self.peek(0).is_ascii_digit() || self.peek(0) == b'_' {
            self.i += 1;
        }
        if self.peek(0) == b'.' {
            self.i += 1;
            while self.peek(0).is_ascii_digit() || self.peek(0) == b'_' {
                self.i += 1;
            }
        }
        if matches!(self.peek(0), b'e' | b'E') && (self.peek(1).is_ascii_digit() || (matches!(self.peek(1), b'+' | b'-') && self.peek(2).is_ascii_digit())) {
            self.i += 2;
            while self.peek(0).is_ascii_digit() {
                self.i += 1;
            }
        }
        let s: String = self.text[start..self.i].chars().filter(|c| *c != '_').collect();
        if self.char_at().is_some_and(is_id_start) {
            return Err(self.err("an identifier starts right after a number"));
        }
        s.parse::<f64>().map_err(|_| self.err("a malformed number"))
    }

    fn escape(&mut self, out: &mut String) -> Result<(), String> {
        self.i += 1;
        let c = self.char_at().ok_or_else(|| self.err("a string ends inside an escape"))?;
        self.i += c.len_utf8();
        match c {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'v' => out.push('\u{b}'),
            '0' if !self.peek(0).is_ascii_digit() => out.push('\0'),
            '\n' => self.line += 1,
            '\r' => {
                if self.peek(0) == b'\n' {
                    self.i += 1;
                }
                self.line += 1;
            }
            'x' => {
                let h = self.text.get(self.i..self.i + 2).ok_or_else(|| self.err("a short \\x escape"))?;
                let v = u32::from_str_radix(h, 16).map_err(|_| self.err("a malformed \\x escape"))?;
                out.push(char::from_u32(v).unwrap_or('\u{fffd}'));
                self.i += 2;
            }
            'u' => {
                let v = if self.peek(0) == b'{' {
                    let end = self.text[self.i..].find('}').ok_or_else(|| self.err("an unclosed \\u{} escape"))?;
                    let h = &self.text[self.i + 1..self.i + end];
                    self.i += end + 1;
                    u32::from_str_radix(h, 16).map_err(|_| self.err("a malformed \\u escape"))?
                } else {
                    let h = self.text.get(self.i..self.i + 4).ok_or_else(|| self.err("a short \\u escape"))?;
                    self.i += 4;
                    u32::from_str_radix(h, 16).map_err(|_| self.err("a malformed \\u escape"))?
                };
                out.push(char::from_u32(v).unwrap_or('\u{fffd}'));
            }
            other => out.push(other),
        }
        Ok(())
    }

    fn string(&mut self, q: u8) -> Result<String, String> {
        self.i += 1;
        let mut out = String::new();
        loop {
            match self.peek(0) {
                0 if self.i >= self.src.len() => return Err(self.err("a string is never closed")),
                b'\n' => return Err(self.err("a string runs past the end of its line")),
                b'\\' => self.escape(&mut out)?,
                c if c == q => {
                    self.i += 1;
                    return Ok(out);
                }
                _ => {
                    let c = self.char_at().unwrap_or('\u{fffd}');
                    out.push(c);
                    self.i += c.len_utf8();
                }
            }
        }
    }

    fn template(&mut self) -> Result<Tok, String> {
        self.i += 1;
        let mut quasis = vec![];
        let mut exprs = vec![];
        let mut cur = String::new();
        loop {
            match self.peek(0) {
                0 if self.i >= self.src.len() => return Err(self.err("a template string is never closed")),
                b'`' => {
                    self.i += 1;
                    quasis.push(cur);
                    return Ok(Tok::Tmpl(quasis, exprs));
                }
                b'\\' => self.escape(&mut cur)?,
                b'$' if self.peek(1) == b'{' => {
                    self.i += 2;
                    let start = self.i;
                    let line = self.line;
                    let mut depth = 0;
                    loop {
                        match self.peek(0) {
                            0 if self.i >= self.src.len() => return Err(self.err("a ${ in a template is never closed")),
                            b'{' => depth += 1,
                            b'}' if depth == 0 => break,
                            b'}' => depth -= 1,
                            b'\n' => self.line += 1,
                            q @ (b'"' | b'\'') => {
                                self.string(q)?;
                                continue;
                            }
                            _ => {}
                        }
                        self.i += 1;
                    }
                    exprs.push((self.text[start..self.i].to_string(), line));
                    self.i += 1;
                    quasis.push(std::mem::take(&mut cur));
                }
                b'\n' => {
                    self.line += 1;
                    cur.push('\n');
                    self.i += 1;
                }
                _ => {
                    let c = self.char_at().unwrap_or('\u{fffd}');
                    cur.push(c);
                    self.i += c.len_utf8();
                }
            }
        }
    }

    fn regex(&mut self) -> Result<Tok, String> {
        self.i += 1;
        let start = self.i;
        let mut in_class = false;
        loop {
            match self.peek(0) {
                0 if self.i >= self.src.len() => return Err(self.err("a regular expression is never closed")),
                b'\n' => return Err(self.err("a regular expression runs past the end of its line")),
                b'\\' => {
                    self.i += 1;
                    self.i += self.char_at().map(char::len_utf8).unwrap_or(1);
                }
                b'[' => {
                    in_class = true;
                    self.i += 1;
                }
                b']' => {
                    in_class = false;
                    self.i += 1;
                }
                b'/' if !in_class => break,
                _ => self.i += self.char_at().map(char::len_utf8).unwrap_or(1),
            }
        }
        let body = self.text[start..self.i].to_string();
        self.i += 1;
        let fs = self.i;
        while self.peek(0).is_ascii_alphabetic() {
            self.i += 1;
        }
        Ok(Tok::Regex(body, self.text[fs..self.i].to_string()))
    }

    pub fn tokens(mut self) -> Result<Vec<Token>, String> {
        let mut out = vec![];
        loop {
            let nl = self.skip_space()?;
            let line = self.line;
            if self.i >= self.src.len() {
                out.push(Token { tok: Tok::Eof, nl: true, line });
                return Ok(out);
            }
            let c = self.peek(0);
            let tok = if c.is_ascii_digit() || (c == b'.' && self.peek(1).is_ascii_digit()) {
                Tok::Num(self.number()?)
            } else if c == b'"' || c == b'\'' {
                Tok::Str(self.string(c)?)
            } else if c == b'`' {
                self.template()?
            } else if c == b'/' && regex_allowed(out.last()) && self.peek(1) != b'/' && self.peek(1) != b'*' {
                self.regex()?
            } else if self.char_at().is_some_and(is_id_start) || c == b'\\' {
                let start = self.i;
                while let Some(ch) = self.char_at() {
                    if !is_id_part(ch) {
                        break;
                    }
                    self.i += ch.len_utf8();
                }
                if self.i == start {
                    return Err(self.err("an unexpected backslash"));
                }
                Tok::Ident(self.text[start..self.i].to_string())
            } else {
                let rest = &self.text[self.i..];
                let found = PUNCTS.iter().chain(SINGLE.iter()).find(|p| rest.starts_with(**p)).copied();
                match found {
                    Some("?.") if self.peek(2).is_ascii_digit() => {
                        self.i += 1;
                        Tok::P("?")
                    }
                    Some(p) => {
                        self.i += p.len();
                        Tok::P(p)
                    }
                    None => {
                        let ch = self.char_at().unwrap_or('?');
                        return Err(self.err(&format!("the character `{ch}` is not valid here")));
                    }
                }
            };
            out.push(Token { tok, nl, line });
        }
    }
}
