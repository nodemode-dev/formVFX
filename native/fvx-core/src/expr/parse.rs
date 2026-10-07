use std::sync::Arc;

use super::ast::*;
use super::lex::{Lexer, Tok, Token};

const KEYWORDS: [&str; 28] = [
    "var", "let", "const", "function", "if", "else", "for", "while", "do", "break", "continue", "return", "throw", "try", "catch", "finally", "switch", "case", "default", "new", "typeof", "void", "delete", "instanceof", "in", "this", "class", "with",
];

pub struct Parser {
    toks: Vec<Token>,
    i: usize,
}

pub fn parse_program(src: &str) -> Result<Program, String> {
    let toks = Lexer::new(src, 1).tokens()?;
    let mut p = Parser { toks, i: 0 };
    let mut body = vec![];
    while !p.at_eof() {
        body.push(p.statement()?);
    }
    Ok(Program { body })
}

fn parse_fragment(src: &str, line: u32) -> Result<Expr, String> {
    let toks = Lexer::new(src, line).tokens()?;
    let mut p = Parser { toks, i: 0 };
    let e = p.expression()?;
    if !p.at_eof() {
        return Err(p.unexpected());
    }
    Ok(e)
}

fn bin_prec(t: &Tok) -> Option<(u8, BinOp)> {
    let Tok::P(p) = t else {
        return match t {
            Tok::Ident(s) if s == "instanceof" => Some((9, BinOp::InstanceOf)),
            Tok::Ident(s) if s == "in" => Some((9, BinOp::In)),
            _ => None,
        };
    };
    Some(match *p {
        "|" => (5, BinOp::BitOr),
        "^" => (6, BinOp::BitXor),
        "&" => (7, BinOp::BitAnd),
        "==" => (8, BinOp::Eq),
        "!=" => (8, BinOp::Ne),
        "===" => (8, BinOp::StrictEq),
        "!==" => (8, BinOp::StrictNe),
        "<" => (9, BinOp::Lt),
        ">" => (9, BinOp::Gt),
        "<=" => (9, BinOp::Le),
        ">=" => (9, BinOp::Ge),
        "<<" => (10, BinOp::Shl),
        ">>" => (10, BinOp::Shr),
        ">>>" => (10, BinOp::UShr),
        "+" => (11, BinOp::Add),
        "-" => (11, BinOp::Sub),
        "*" => (12, BinOp::Mul),
        "/" => (12, BinOp::Div),
        "%" => (12, BinOp::Mod),
        "**" => (13, BinOp::Pow),
        _ => return None,
    })
}

fn assign_op(p: &str) -> Option<Option<BinOp>> {
    Some(match p {
        "=" => None,
        "+=" => Some(BinOp::Add),
        "-=" => Some(BinOp::Sub),
        "*=" => Some(BinOp::Mul),
        "/=" => Some(BinOp::Div),
        "%=" => Some(BinOp::Mod),
        "**=" => Some(BinOp::Pow),
        "&=" => Some(BinOp::BitAnd),
        "|=" => Some(BinOp::BitOr),
        "^=" => Some(BinOp::BitXor),
        "<<=" => Some(BinOp::Shl),
        ">>=" => Some(BinOp::Shr),
        ">>>=" => Some(BinOp::UShr),
        _ => return None,
    })
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Num(n) => format!("the number {n}"),
        Tok::Str(_) => "a string".into(),
        Tok::Tmpl(..) => "a template string".into(),
        Tok::Regex(..) => "a regular expression".into(),
        Tok::Ident(s) => format!("`{s}`"),
        Tok::P(p) => format!("`{p}`"),
        Tok::Eof => "the end of the expression".into(),
    }
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.i].tok
    }

    fn peek_at(&self, k: usize) -> &Tok {
        &self.toks[(self.i + k).min(self.toks.len() - 1)].tok
    }

    fn line(&self) -> u32 {
        self.toks[self.i].line
    }

    fn nl_before(&self) -> bool {
        self.toks[self.i].nl
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), Tok::Eof)
    }

    fn next(&mut self) -> Tok {
        let t = self.toks[self.i].tok.clone();
        if self.i + 1 < self.toks.len() {
            self.i += 1;
        }
        t
    }

    fn is_p(&self, p: &str) -> bool {
        matches!(self.peek(), Tok::P(x) if *x == p)
    }

    fn is_kw(&self, k: &str) -> bool {
        matches!(self.peek(), Tok::Ident(x) if x == k)
    }

    fn eat_p(&mut self, p: &str) -> bool {
        if self.is_p(p) {
            self.next();
            true
        } else {
            false
        }
    }

    fn eat_kw(&mut self, k: &str) -> bool {
        if self.is_kw(k) {
            self.next();
            true
        } else {
            false
        }
    }

    fn unexpected(&self) -> String {
        format!("SyntaxError: unexpected {} (line {})", describe(self.peek()), self.line())
    }

    fn expect_p(&mut self, p: &str) -> Result<(), String> {
        if self.eat_p(p) {
            Ok(())
        } else {
            Err(format!("SyntaxError: expected `{p}` but found {} (line {})", describe(self.peek()), self.line()))
        }
    }

    fn ident(&mut self) -> Result<String, String> {
        match self.peek().clone() {
            Tok::Ident(s) if !KEYWORDS.contains(&s.as_str()) => {
                self.next();
                Ok(s)
            }
            _ => Err(format!("SyntaxError: expected a name but found {} (line {})", describe(self.peek()), self.line())),
        }
    }

    fn prop_name(&mut self) -> Result<String, String> {
        match self.next() {
            Tok::Ident(s) => Ok(s),
            Tok::Str(s) => Ok(s),
            Tok::Num(n) => Ok(super::interp::num_to_string(n)),
            t => Err(format!("SyntaxError: expected a property name but found {} (line {})", describe(&t), self.line())),
        }
    }

    fn semi(&mut self) -> Result<(), String> {
        if self.eat_p(";") || self.is_p("}") || self.at_eof() || self.nl_before() {
            Ok(())
        } else {
            Err(self.unexpected())
        }
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        let line = self.line();
        let kind = self.statement_kind()?;
        Ok(Stmt { line, kind })
    }

    fn block_body(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect_p("{")?;
        let mut out = vec![];
        while !self.is_p("}") {
            if self.at_eof() {
                return Err(format!("SyntaxError: a block is never closed with `}}` (line {})", self.line()));
            }
            out.push(self.statement()?);
        }
        self.next();
        Ok(out)
    }

    fn decl_kind(&self) -> Option<DeclKind> {
        match self.peek() {
            Tok::Ident(s) if s == "var" => Some(DeclKind::Var),
            Tok::Ident(s) if s == "let" => Some(DeclKind::Let),
            Tok::Ident(s) if s == "const" => Some(DeclKind::Const),
            _ => None,
        }
    }

    fn declarators(&mut self, kind: DeclKind) -> Result<Vec<(Pattern, Option<Expr>)>, String> {
        let mut out = vec![];
        loop {
            let pat = self.binding_pattern()?;
            let init = if self.eat_p("=") { Some(self.assign()?) } else { None };
            if kind == DeclKind::Const && init.is_none() && !(self.is_kw("of") || self.is_kw("in")) {
                return Err(format!("SyntaxError: a const needs a value (line {})", self.line()));
            }
            out.push((pat, init));
            if !self.eat_p(",") {
                return Ok(out);
            }
        }
    }

    fn binding_pattern(&mut self) -> Result<Pattern, String> {
        if self.eat_p("[") {
            let mut items = vec![];
            let mut rest = None;
            loop {
                if self.eat_p("]") {
                    break;
                }
                if self.is_p(",") {
                    self.next();
                    items.push(None);
                    continue;
                }
                if self.eat_p("...") {
                    rest = Some(Box::new(self.binding_pattern()?));
                    self.expect_p("]")?;
                    break;
                }
                let p = self.binding_pattern()?;
                let d = if self.eat_p("=") { Some(self.assign()?) } else { None };
                items.push(Some((p, d)));
                if !self.eat_p(",") {
                    self.expect_p("]")?;
                    break;
                }
            }
            return Ok(Pattern::Array(items, rest));
        }
        if self.eat_p("{") {
            let mut props = vec![];
            loop {
                if self.eat_p("}") {
                    break;
                }
                let key = self.prop_name()?;
                let pat = if self.eat_p(":") { self.binding_pattern()? } else { Pattern::Name(key.clone()) };
                let d = if self.eat_p("=") { Some(self.assign()?) } else { None };
                props.push((key, pat, d));
                if !self.eat_p(",") {
                    self.expect_p("}")?;
                    break;
                }
            }
            return Ok(Pattern::Object(props));
        }
        Ok(Pattern::Name(self.ident()?))
    }

    fn statement_kind(&mut self) -> Result<StmtKind, String> {
        if self.eat_p(";") {
            return Ok(StmtKind::Empty);
        }
        if self.is_p("{") {
            return Ok(StmtKind::Block(self.block_body()?));
        }
        if let Some(kind) = self.decl_kind() {
            self.next();
            let d = self.declarators(kind)?;
            self.semi()?;
            return Ok(StmtKind::Decl(kind, d));
        }
        let Tok::Ident(word) = self.peek().clone() else {
            let e = self.expression()?;
            self.semi()?;
            return Ok(StmtKind::Expr(e));
        };
        if matches!(self.peek_at(1), Tok::P(":")) && !KEYWORDS.contains(&word.as_str()) {
            self.next();
            self.next();
            return Ok(StmtKind::Labeled(word, Box::new(self.statement()?)));
        }
        match word.as_str() {
            "function" => {
                self.next();
                let f = self.function_rest(true)?;
                Ok(StmtKind::Func(f))
            }
            "if" => {
                self.next();
                self.expect_p("(")?;
                let c = self.expression()?;
                self.expect_p(")")?;
                let then = Box::new(self.statement()?);
                let other = if self.eat_kw("else") { Some(Box::new(self.statement()?)) } else { None };
                Ok(StmtKind::If(c, then, other))
            }
            "for" => {
                self.next();
                self.for_rest()
            }
            "while" => {
                self.next();
                self.expect_p("(")?;
                let c = self.expression()?;
                self.expect_p(")")?;
                Ok(StmtKind::While(c, Box::new(self.statement()?)))
            }
            "do" => {
                self.next();
                let body = Box::new(self.statement()?);
                if !self.eat_kw("while") {
                    return Err(self.unexpected());
                }
                self.expect_p("(")?;
                let c = self.expression()?;
                self.expect_p(")")?;
                self.eat_p(";");
                Ok(StmtKind::DoWhile(body, c))
            }
            "break" | "continue" => {
                self.next();
                let label = match self.peek().clone() {
                    Tok::Ident(l) if !self.nl_before() && !KEYWORDS.contains(&l.as_str()) => {
                        self.next();
                        Some(l)
                    }
                    _ => None,
                };
                self.semi()?;
                Ok(if word == "break" { StmtKind::Break(label) } else { StmtKind::Continue(label) })
            }
            "return" => {
                self.next();
                if self.eat_p(";") || self.is_p("}") || self.at_eof() || self.nl_before() {
                    return Ok(StmtKind::Return(None));
                }
                let e = self.expression()?;
                self.semi()?;
                Ok(StmtKind::Return(Some(e)))
            }
            "throw" => {
                self.next();
                let e = self.expression()?;
                self.semi()?;
                Ok(StmtKind::Throw(e))
            }
            "try" => {
                self.next();
                let block = self.block_body()?;
                let mut param = None;
                let mut handler = None;
                if self.eat_kw("catch") {
                    if self.eat_p("(") {
                        param = Some(self.binding_pattern()?);
                        self.expect_p(")")?;
                    }
                    handler = Some(self.block_body()?);
                }
                let finally = if self.eat_kw("finally") { Some(self.block_body()?) } else { None };
                if handler.is_none() && finally.is_none() {
                    return Err(format!("SyntaxError: try needs catch or finally (line {})", self.line()));
                }
                Ok(StmtKind::Try { block, param, handler, finally })
            }
            "switch" => {
                self.next();
                self.expect_p("(")?;
                let d = self.expression()?;
                self.expect_p(")")?;
                self.expect_p("{")?;
                let mut cases = vec![];
                while !self.eat_p("}") {
                    let test = if self.eat_kw("case") {
                        Some(self.expression()?)
                    } else if self.eat_kw("default") {
                        None
                    } else {
                        return Err(self.unexpected());
                    };
                    self.expect_p(":")?;
                    let mut body = vec![];
                    while !self.is_kw("case") && !self.is_kw("default") && !self.is_p("}") {
                        if self.at_eof() {
                            return Err(self.unexpected());
                        }
                        body.push(self.statement()?);
                    }
                    cases.push((test, body));
                }
                Ok(StmtKind::Switch(d, cases))
            }
            "class" | "with" | "import" | "export" | "async" | "await" | "yield" => Err(format!("SyntaxError: `{word}` is not supported in expressions (line {})", self.line())),
            _ => {
                let e = self.expression()?;
                self.semi()?;
                Ok(StmtKind::Expr(e))
            }
        }
    }

    fn for_rest(&mut self) -> Result<StmtKind, String> {
        self.expect_p("(")?;
        let line = self.line();
        let mut init = None;
        if let Some(kind) = self.decl_kind() {
            self.next();
            let pat = self.binding_pattern()?;
            if self.is_kw("of") || self.is_kw("in") {
                let of = self.is_kw("of");
                self.next();
                let obj = if of { self.assign()? } else { self.expression()? };
                self.expect_p(")")?;
                let body = Box::new(self.statement()?);
                return Ok(StmtKind::ForEach { kind: Some(kind), pat, of, obj, body });
            }
            let first_init = if self.eat_p("=") { Some(self.assign_no_in()?) } else { None };
            let mut decls = vec![(pat, first_init)];
            while self.eat_p(",") {
                let p = self.binding_pattern()?;
                let i = if self.eat_p("=") { Some(self.assign_no_in()?) } else { None };
                decls.push((p, i));
            }
            init = Some(Box::new(Stmt { line, kind: StmtKind::Decl(kind, decls) }));
        } else if !self.is_p(";") {
            if let Tok::Ident(name) = self.peek().clone()
                && matches!(self.peek_at(1), Tok::Ident(w) if w == "of" || w == "in")
                && !KEYWORDS.contains(&name.as_str())
            {
                self.next();
                let of = self.is_kw("of");
                self.next();
                let obj = if of { self.assign()? } else { self.expression()? };
                self.expect_p(")")?;
                let body = Box::new(self.statement()?);
                return Ok(StmtKind::ForEach { kind: None, pat: Pattern::Name(name), of, obj, body });
            }
            let e = self.assign_no_in()?;
            init = Some(Box::new(Stmt { line, kind: StmtKind::Expr(e) }));
        }
        self.expect_p(";")?;
        let test = if self.is_p(";") { None } else { Some(self.expression()?) };
        self.expect_p(";")?;
        let update = if self.is_p(")") { None } else { Some(self.expression()?) };
        self.expect_p(")")?;
        let body = Box::new(self.statement()?);
        Ok(StmtKind::For { init, test, update, body })
    }

    fn assign_no_in(&mut self) -> Result<Expr, String> {
        let mut out = self.assign()?;
        while self.eat_p(",") {
            let next = self.assign()?;
            out = match out {
                Expr::Seq(mut v) => {
                    v.push(next);
                    Expr::Seq(v)
                }
                e => Expr::Seq(vec![e, next]),
            };
        }
        Ok(out)
    }

    fn function_rest(&mut self, needs_name: bool) -> Result<Arc<FuncDef>, String> {
        if self.is_p("*") {
            return Err(format!("SyntaxError: generator functions are not supported (line {})", self.line()));
        }
        let name = if matches!(self.peek(), Tok::Ident(_)) { Some(self.ident()?) } else { None };
        if needs_name && name.is_none() {
            return Err(format!("SyntaxError: a function declaration needs a name (line {})", self.line()));
        }
        self.expect_p("(")?;
        let params = self.params()?;
        let body = Body::Block(self.block_body()?);
        Ok(Arc::new(FuncDef { name, params, body, arrow: false }))
    }

    fn params(&mut self) -> Result<Vec<Param>, String> {
        let mut out = vec![];
        loop {
            if self.eat_p(")") {
                return Ok(out);
            }
            let rest = self.eat_p("...");
            let pat = self.binding_pattern()?;
            let default = if self.eat_p("=") { Some(self.assign()?) } else { None };
            out.push(Param { pat, default, rest });
            if !self.eat_p(",") {
                self.expect_p(")")?;
                return Ok(out);
            }
        }
    }

    fn arrow_ahead(&self) -> bool {
        if !self.is_p("(") {
            return false;
        }
        let mut depth = 0i32;
        let mut k = self.i;
        while k < self.toks.len() {
            match &self.toks[k].tok {
                Tok::P("(") | Tok::P("[") | Tok::P("{") => depth += 1,
                Tok::P(")") | Tok::P("]") | Tok::P("}") => {
                    depth -= 1;
                    if depth == 0 {
                        return matches!(self.toks.get(k + 1).map(|t| &t.tok), Some(Tok::P("=>")));
                    }
                }
                Tok::Eof => return false,
                _ => {}
            }
            k += 1;
        }
        false
    }

    fn arrow_body(&mut self, params: Vec<Param>) -> Result<Expr, String> {
        let body = if self.is_p("{") { Body::Block(self.block_body()?) } else { Body::Expr(Box::new(self.assign()?)) };
        Ok(Expr::Func(Arc::new(FuncDef { name: None, params, body, arrow: true })))
    }

    pub fn expression(&mut self) -> Result<Expr, String> {
        let first = self.assign()?;
        if !self.is_p(",") {
            return Ok(first);
        }
        let mut v = vec![first];
        while self.eat_p(",") {
            v.push(self.assign()?);
        }
        Ok(Expr::Seq(v))
    }

    fn assign(&mut self) -> Result<Expr, String> {
        if let Tok::Ident(name) = self.peek().clone()
            && matches!(self.peek_at(1), Tok::P("=>"))
            && !KEYWORDS.contains(&name.as_str())
        {
            self.next();
            self.next();
            return self.arrow_body(vec![Param { pat: Pattern::Name(name), default: None, rest: false }]);
        }
        if self.arrow_ahead() {
            self.next();
            let params = self.params()?;
            self.expect_p("=>")?;
            return self.arrow_body(params);
        }
        let left = self.conditional()?;
        if let Tok::P(p) = self.peek().clone() {
            if let Some(op) = assign_op(p) {
                self.next();
                let right = self.assign()?;
                return match (&left, op) {
                    (Expr::Ident(_) | Expr::Member(..) | Expr::Index(..), _) => Ok(Expr::Assign(op, Box::new(left), Box::new(right))),
                    (Expr::Array(_) | Expr::Object(_), None) => Ok(Expr::DestructAssign(self.to_pattern(left)?, Box::new(right))),
                    _ => Err(format!("SyntaxError: the left side of `{p}` cannot be assigned to (line {})", self.line())),
                };
            }
            let logic = match p {
                "&&=" => Some(LogicOp::And),
                "||=" => Some(LogicOp::Or),
                "??=" => Some(LogicOp::Nullish),
                _ => None,
            };
            if let Some(op) = logic {
                self.next();
                let right = self.assign()?;
                return Ok(Expr::LogicAssign(op, Box::new(left), Box::new(right)));
            }
        }
        Ok(left)
    }

    fn to_pattern(&self, e: Expr) -> Result<Pattern, String> {
        match e {
            Expr::Ident(n) => Ok(Pattern::Name(n)),
            Expr::Array(items) => {
                let mut out = vec![];
                let mut rest = None;
                for it in items {
                    match it {
                        None => out.push(None),
                        Some(Arg::Spread(e)) => rest = Some(Box::new(self.to_pattern(e)?)),
                        Some(Arg::Plain(Expr::Assign(None, t, d))) => out.push(Some((self.to_pattern(*t)?, Some(*d)))),
                        Some(Arg::Plain(e)) => out.push(Some((self.to_pattern(e)?, None))),
                    }
                }
                Ok(Pattern::Array(out, rest))
            }
            Expr::Object(entries) => {
                let mut out = vec![];
                for en in entries {
                    match en {
                        ObjEntry::Pair(PropKey::Name(k), Expr::Assign(None, t, d)) => out.push((k, self.to_pattern(*t)?, Some(*d))),
                        ObjEntry::Pair(PropKey::Name(k), v) => out.push((k, self.to_pattern(v)?, None)),
                        _ => return Err(format!("SyntaxError: this destructuring is not supported (line {})", self.line())),
                    }
                }
                Ok(Pattern::Object(out))
            }
            _ => Err(format!("SyntaxError: the left side cannot be assigned to (line {})", self.line())),
        }
    }

    fn conditional(&mut self) -> Result<Expr, String> {
        let c = self.logical_or()?;
        if self.eat_p("?") {
            let a = self.assign()?;
            self.expect_p(":")?;
            let b = self.assign()?;
            return Ok(Expr::Cond(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(c)
    }

    fn logical_or(&mut self) -> Result<Expr, String> {
        let mut left = self.logical_and()?;
        loop {
            let op = if self.is_p("||") {
                LogicOp::Or
            } else if self.is_p("??") {
                LogicOp::Nullish
            } else {
                return Ok(left);
            };
            self.next();
            let right = self.logical_and()?;
            left = Expr::Logic(op, Box::new(left), Box::new(right));
        }
    }

    fn logical_and(&mut self) -> Result<Expr, String> {
        let mut left = self.binary(5)?;
        while self.eat_p("&&") {
            let right = self.binary(5)?;
            left = Expr::Logic(LogicOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn binary(&mut self, min: u8) -> Result<Expr, String> {
        let mut left = self.unary()?;
        loop {
            let Some((prec, op)) = bin_prec(self.peek()) else { return Ok(left) };
            if prec < min {
                return Ok(left);
            }
            self.next();
            let right = if op == BinOp::Pow { self.binary(prec)? } else { self.binary(prec + 1)? };
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        let op = match self.peek() {
            Tok::P("-") => Some(UnOp::Neg),
            Tok::P("+") => Some(UnOp::Plus),
            Tok::P("!") => Some(UnOp::Not),
            Tok::P("~") => Some(UnOp::BitNot),
            Tok::Ident(s) if s == "typeof" => Some(UnOp::TypeOf),
            Tok::Ident(s) if s == "void" => Some(UnOp::Void),
            Tok::Ident(s) if s == "delete" => Some(UnOp::Delete),
            _ => None,
        };
        if let Some(op) = op {
            self.next();
            let e = self.unary()?;
            if self.is_p("**") {
                return Err(format!("SyntaxError: wrap the unary expression before `**` in parentheses (line {})", self.line()));
            }
            return Ok(Expr::Unary(op, Box::new(e)));
        }
        if self.is_p("++") || self.is_p("--") {
            let inc = self.is_p("++");
            self.next();
            let t = self.unary()?;
            return Ok(Expr::Update { inc, prefix: true, target: Box::new(t) });
        }
        let e = self.postfix()?;
        Ok(e)
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let e = self.call_member()?;
        if (self.is_p("++") || self.is_p("--")) && !self.nl_before() {
            let inc = self.is_p("++");
            self.next();
            return Ok(Expr::Update { inc, prefix: false, target: Box::new(e) });
        }
        Ok(e)
    }

    fn args(&mut self) -> Result<Vec<Arg>, String> {
        let mut out = vec![];
        loop {
            if self.eat_p(")") {
                return Ok(out);
            }
            if self.eat_p("...") {
                out.push(Arg::Spread(self.assign()?));
            } else {
                out.push(Arg::Plain(self.assign()?));
            }
            if !self.eat_p(",") {
                self.expect_p(")")?;
                return Ok(out);
            }
        }
    }

    fn call_member(&mut self) -> Result<Expr, String> {
        let mut e = if self.eat_kw("new") {
            let callee = self.new_callee()?;
            let args = if self.eat_p("(") { self.args()? } else { vec![] };
            Expr::New(Box::new(callee), args)
        } else {
            self.primary()?
        };
        loop {
            if self.eat_p(".") {
                let name = self.prop_name()?;
                e = Expr::Member(Box::new(e), name, false);
            } else if self.is_p("?.") {
                self.next();
                if self.eat_p("(") {
                    let a = self.args()?;
                    e = Expr::Call(Box::new(e), a, true);
                } else if self.eat_p("[") {
                    let k = self.expression()?;
                    self.expect_p("]")?;
                    e = Expr::Index(Box::new(e), Box::new(k), true);
                } else {
                    let name = self.prop_name()?;
                    e = Expr::Member(Box::new(e), name, true);
                }
            } else if self.is_p("[") {
                self.next();
                let k = self.expression()?;
                self.expect_p("]")?;
                e = Expr::Index(Box::new(e), Box::new(k), false);
            } else if self.is_p("(") {
                self.next();
                let a = self.args()?;
                e = Expr::Call(Box::new(e), a, false);
            } else if matches!(self.peek(), Tok::Tmpl(..)) {
                return Err(format!("SyntaxError: tagged templates are not supported (line {})", self.line()));
            } else {
                return Ok(e);
            }
        }
    }

    fn new_callee(&mut self) -> Result<Expr, String> {
        let mut e = self.primary()?;
        loop {
            if self.eat_p(".") {
                let name = self.prop_name()?;
                e = Expr::Member(Box::new(e), name, false);
            } else if self.is_p("[") {
                self.next();
                let k = self.expression()?;
                self.expect_p("]")?;
                e = Expr::Index(Box::new(e), Box::new(k), false);
            } else {
                return Ok(e);
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let line = self.line();
        match self.next() {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Regex(p, f) => Ok(Expr::New(Box::new(Expr::Ident("RegExp".into())), vec![Arg::Plain(Expr::Str(p)), Arg::Plain(Expr::Str(f))])),
            Tok::Tmpl(quasis, srcs) => {
                let mut exprs = vec![];
                for (s, l) in srcs {
                    exprs.push(parse_fragment(&s, l)?);
                }
                Ok(Expr::Tmpl(quasis, exprs))
            }
            Tok::Ident(s) => match s.as_str() {
                "true" => Ok(Expr::Bool(true)),
                "false" => Ok(Expr::Bool(false)),
                "null" => Ok(Expr::Null),
                "undefined" => Ok(Expr::Undefined),
                "this" => Ok(Expr::This),
                "function" => Ok(Expr::Func(self.function_rest(false)?)),
                k if KEYWORDS.contains(&k) => Err(format!("SyntaxError: unexpected `{k}` (line {line})")),
                _ => Ok(Expr::Ident(s)),
            },
            Tok::P("(") => {
                let e = self.expression()?;
                self.expect_p(")")?;
                Ok(e)
            }
            Tok::P("[") => {
                let mut items = vec![];
                loop {
                    if self.eat_p("]") {
                        break;
                    }
                    if self.is_p(",") {
                        self.next();
                        items.push(None);
                        continue;
                    }
                    let it = if self.eat_p("...") { Arg::Spread(self.assign()?) } else { Arg::Plain(self.assign()?) };
                    items.push(Some(it));
                    if !self.eat_p(",") {
                        self.expect_p("]")?;
                        break;
                    }
                }
                Ok(Expr::Array(items))
            }
            Tok::P("{") => {
                let mut entries = vec![];
                loop {
                    if self.eat_p("}") {
                        break;
                    }
                    if self.eat_p("...") {
                        entries.push(ObjEntry::Spread(self.assign()?));
                    } else if self.eat_p("[") {
                        let k = self.assign()?;
                        self.expect_p("]")?;
                        if self.eat_p("(") {
                            let params = self.params()?;
                            let body = Body::Block(self.block_body()?);
                            entries.push(ObjEntry::Pair(PropKey::Computed(k), Expr::Func(Arc::new(FuncDef { name: None, params, body, arrow: false }))));
                        } else {
                            self.expect_p(":")?;
                            entries.push(ObjEntry::Pair(PropKey::Computed(k), self.assign()?));
                        }
                    } else {
                        let is_ident = matches!(self.peek(), Tok::Ident(_));
                        let key = self.prop_name()?;
                        if (key == "get" || key == "set") && is_ident && !self.is_p(":") && !self.is_p("(") && !self.is_p(",") && !self.is_p("}") {
                            return Err(format!("SyntaxError: getters and setters are not supported (line {})", self.line()));
                        }
                        if self.eat_p(":") {
                            entries.push(ObjEntry::Pair(PropKey::Name(key), self.assign()?));
                        } else if self.eat_p("(") {
                            let params = self.params()?;
                            let body = Body::Block(self.block_body()?);
                            entries.push(ObjEntry::Pair(PropKey::Name(key.clone()), Expr::Func(Arc::new(FuncDef { name: Some(key), params, body, arrow: false }))));
                        } else if is_ident {
                            if self.eat_p("=") {
                                let d = self.assign()?;
                                entries.push(ObjEntry::Pair(PropKey::Name(key.clone()), Expr::Assign(None, Box::new(Expr::Ident(key)), Box::new(d))));
                            } else {
                                entries.push(ObjEntry::Pair(PropKey::Name(key.clone()), Expr::Ident(key)));
                            }
                        } else {
                            return Err(self.unexpected());
                        }
                    }
                    if !self.eat_p(",") {
                        self.expect_p("}")?;
                        break;
                    }
                }
                Ok(Expr::Object(entries))
            }
            Tok::P("/") | Tok::P("/=") => Err(format!("SyntaxError: regular expressions are not supported yet (line {line})")),
            t => Err(format!("SyntaxError: unexpected {} (line {line})", describe(&t))),
        }
    }
}
