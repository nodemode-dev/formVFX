use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use super::ast::*;
use super::builtins;
use super::host::{self, Cx, H};
use crate::model::path::BezPath;

pub const MAX_STEPS: u64 = 4_000_000;
const MAX_CALL_DEPTH: u32 = 160;

#[derive(Clone)]
pub enum V {
    Undef,
    Null,
    Bool(bool),
    Num(f64),
    Str(Rc<str>),
    Arr(Rc<RefCell<Vec<V>>>),
    Obj(Rc<RefCell<Obj>>),
    Func(Rc<Closure>),
    Native(Rc<NativeFn>),
    Host(Rc<H>),
    Path(Rc<BezPath>),
}

#[derive(Default)]
pub struct Obj {
    pub props: Vec<(Rc<str>, V)>,
}

impl Obj {
    pub fn get(&self, k: &str) -> Option<&V> {
        self.props.iter().find(|(n, _)| &**n == k).map(|(_, v)| v)
    }

    pub fn set(&mut self, k: &str, v: V) {
        match self.props.iter_mut().find(|(n, _)| &**n == k) {
            Some(slot) => slot.1 = v,
            None => self.props.push((Rc::from(k), v)),
        }
    }

    pub fn remove(&mut self, k: &str) -> bool {
        let before = self.props.len();
        self.props.retain(|(n, _)| &**n != k);
        before != self.props.len()
    }
}

pub struct Closure {
    pub def: Arc<FuncDef>,
    pub env: Env,
    pub this: Option<V>,
}

pub type NativeImpl = fn(&mut Interp, &V, Vec<V>) -> R;

pub struct NativeFn {
    pub name: &'static str,
    pub this: V,
    pub f: NativeImpl,
}

pub enum Flow {
    Throw(V),
    Break(Option<String>),
    Continue(Option<String>),
    Return(V),
    Fatal(String),
}

pub type R = Result<V, Flow>;

pub struct Scope {
    vars: RefCell<Vec<(Rc<str>, V, bool)>>,
    parent: Option<Env>,
    func: bool,
}

pub type Env = Rc<Scope>;

impl Scope {
    pub fn new(parent: Option<Env>, func: bool) -> Env {
        Rc::new(Scope { vars: RefCell::new(vec![]), parent, func })
    }
}

fn find_var(env: &Env, name: &str) -> Option<V> {
    let mut cur = Some(env.clone());
    while let Some(s) = cur {
        if let Some((_, v, _)) = s.vars.borrow().iter().find(|(n, _, _)| &**n == name) {
            return Some(v.clone());
        }
        cur = s.parent.clone();
    }
    None
}

fn set_var(env: &Env, name: &str, v: V) -> Option<Result<(), ()>> {
    let mut cur = Some(env.clone());
    while let Some(s) = cur {
        {
            let mut vars = s.vars.borrow_mut();
            if let Some(slot) = vars.iter_mut().find(|(n, _, _)| &**n == name) {
                if slot.2 {
                    return Some(Err(()));
                }
                slot.1 = v;
                return Some(Ok(()));
            }
        }
        cur = s.parent.clone();
    }
    None
}

fn declare(env: &Env, name: &str, v: V, konst: bool) {
    let mut vars = env.vars.borrow_mut();
    match vars.iter_mut().find(|(n, _, _)| &**n == name) {
        Some(slot) => {
            slot.1 = v;
            slot.2 = konst;
        }
        None => vars.push((Rc::from(name), v, konst)),
    }
}

fn func_scope(env: &Env) -> Env {
    let mut cur = env.clone();
    loop {
        if cur.func {
            return cur;
        }
        match cur.parent.clone() {
            Some(p) => cur = p,
            None => return cur,
        }
    }
}

pub fn s(x: &str) -> V {
    V::Str(Rc::from(x))
}

pub fn arr(v: Vec<V>) -> V {
    V::Arr(Rc::new(RefCell::new(v)))
}

pub fn nums(v: &[f64]) -> V {
    arr(v.iter().map(|x| V::Num(*x)).collect())
}

pub fn obj(props: Vec<(&str, V)>) -> V {
    V::Obj(Rc::new(RefCell::new(Obj { props: props.into_iter().map(|(k, v)| (Rc::from(k), v)).collect() })))
}

pub fn native(name: &'static str, this: V, f: NativeImpl) -> V {
    V::Native(Rc::new(NativeFn { name, this, f }))
}

pub fn num_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if n == 0.0 {
        return "0".into();
    }
    let e = format!("{n:e}");
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let k = digits.len() as i32;
    let point = exp + 1;
    let body = if (1..=21).contains(&point) {
        if k <= point {
            format!("{}{}", digits, "0".repeat((point - k) as usize))
        } else {
            format!("{}.{}", &digits[..point as usize], &digits[point as usize..])
        }
    } else if point <= 0 && point > -6 {
        format!("0.{}{}", "0".repeat((-point) as usize), digits)
    } else {
        let m = if k == 1 { digits.clone() } else { format!("{}.{}", &digits[..1], &digits[1..]) };
        let pe = point - 1;
        format!("{m}e{}{}", if pe >= 0 { "+" } else { "-" }, pe.abs())
    };
    if neg { format!("-{body}") } else { body }
}

pub fn str_to_num(t: &str) -> f64 {
    let t = t.trim();
    if t.is_empty() {
        return 0.0;
    }
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u64::from_str_radix(h, 16).map(|v| v as f64).unwrap_or(f64::NAN);
    }
    match t {
        "Infinity" | "+Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ => {
            if t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-')) {
                t.parse().unwrap_or(f64::NAN)
            } else {
                f64::NAN
            }
        }
    }
}

pub fn truthy(v: &V) -> bool {
    match v {
        V::Undef | V::Null => false,
        V::Bool(b) => *b,
        V::Num(n) => *n != 0.0 && !n.is_nan(),
        V::Str(s) => !s.is_empty(),
        _ => true,
    }
}

pub fn type_of(v: &V) -> &'static str {
    match v {
        V::Undef => "undefined",
        V::Bool(_) => "boolean",
        V::Num(_) => "number",
        V::Str(_) => "string",
        V::Func(_) | V::Native(_) => "function",
        V::Host(h) if h.callable() => "function",
        _ => "object",
    }
}

pub fn err_obj(name: &str, msg: &str) -> V {
    obj(vec![("name", s(name)), ("message", s(msg))])
}

pub fn throw(name: &str, msg: impl Into<String>) -> Flow {
    Flow::Throw(err_obj(name, &msg.into()))
}

pub fn type_err(msg: impl Into<String>) -> Flow {
    throw("TypeError", msg)
}

pub fn arg(a: &[V], i: usize) -> V {
    a.get(i).cloned().unwrap_or(V::Undef)
}

pub struct Interp<'a> {
    pub cx: Cx<'a>,
    pub steps: u64,
    pub line: u32,
    depth: u32,
    pub this: V,
    pub script: bool,
    pub max_steps: u64,
}

enum Ref {
    Var(String),
    Member(V, String),
}

impl<'a> Interp<'a> {
    pub fn new(cx: Cx<'a>) -> Interp<'a> {
        Interp { cx, steps: 0, line: 1, depth: 0, this: V::Undef, script: false, max_steps: MAX_STEPS }
    }

    fn tick(&mut self) -> Result<(), Flow> {
        self.steps += 1;
        if self.steps > self.max_steps {
            let what = if self.script { "the script" } else { "the expression" };
            return Err(Flow::Fatal(format!("{what} ran too long (an endless loop?)")));
        }
        Ok(())
    }

    pub fn run(&mut self, prog: &Program) -> Result<V, String> {
        let env = Scope::new(None, true);
        let out = self.exec_body(&prog.body, &env, true);
        match out {
            Ok(v) => Ok(v.unwrap_or(V::Undef)),
            Err(Flow::Return(v)) => Ok(v),
            Err(Flow::Throw(v)) => Err(format!("{} (line {})", self.describe_error(&v), self.line)),
            Err(Flow::Fatal(m)) => Err(m),
            Err(Flow::Break(_)) | Err(Flow::Continue(_)) => Err(format!("SyntaxError: break or continue outside a loop (line {})", self.line)),
        }
    }

    pub fn describe_error(&mut self, v: &V) -> String {
        if let V::Obj(o) = v {
            let o = o.borrow();
            if let (Some(n), Some(m)) = (o.get("name"), o.get("message")) {
                return format!("{}: {}", plain_string(n), plain_string(m));
            }
        }
        format!("Uncaught {}", self.to_string(v).unwrap_or_default())
    }

    fn hoist(&mut self, stmts: &[Stmt], env: &Env, vars: bool) {
        for st in stmts {
            if let StmtKind::Func(def) = &st.kind
                && let Some(n) = &def.name
            {
                let f = V::Func(Rc::new(Closure { def: def.clone(), env: env.clone(), this: None }));
                declare(env, n, f, false);
            }
        }
        if vars {
            let mut names = vec![];
            collect_vars(stmts, &mut names);
            for n in names {
                if !env.vars.borrow().iter().any(|(k, _, _)| &**k == n.as_str()) {
                    declare(env, &n, V::Undef, false);
                }
            }
        }
    }

    fn exec_body(&mut self, stmts: &[Stmt], env: &Env, fn_level: bool) -> Result<Option<V>, Flow> {
        self.hoist(stmts, env, fn_level);
        let mut last = None;
        for st in stmts {
            if let Some(v) = self.exec(st, env)? {
                last = Some(v);
            }
        }
        Ok(last)
    }

    fn block(&mut self, stmts: &[Stmt], env: &Env) -> Result<Option<V>, Flow> {
        let needs = stmts.iter().any(|s| matches!(s.kind, StmtKind::Decl(DeclKind::Let | DeclKind::Const, _) | StmtKind::Func(_)));
        if needs {
            let inner = Scope::new(Some(env.clone()), false);
            self.exec_body(stmts, &inner, false)
        } else {
            let mut last = None;
            for st in stmts {
                if let Some(v) = self.exec(st, env)? {
                    last = Some(v);
                }
            }
            Ok(last)
        }
    }

    fn bind(&mut self, pat: &Pattern, v: V, env: &Env, kind: Option<DeclKind>) -> Result<(), Flow> {
        match pat {
            Pattern::Name(n) => match kind {
                Some(DeclKind::Var) => {
                    let fs = func_scope(env);
                    declare(&fs, n, v, false);
                    Ok(())
                }
                Some(k) => {
                    declare(env, n, v, k == DeclKind::Const);
                    Ok(())
                }
                None => self.assign_name(n, v, env),
            },
            Pattern::Array(items, rest) => {
                let list = self.iterate(&v)?;
                for (i, it) in items.iter().enumerate() {
                    if let Some((p, d)) = it {
                        let mut x = list.get(i).cloned().unwrap_or(V::Undef);
                        if matches!(x, V::Undef)
                            && let Some(d) = d
                        {
                            x = self.eval(d, env)?;
                        }
                        self.bind(p, x, env, kind)?;
                    }
                }
                if let Some(r) = rest {
                    let tail = list.get(items.len()..).map(|s| s.to_vec()).unwrap_or_default();
                    self.bind(r, arr(tail), env, kind)?;
                }
                Ok(())
            }
            Pattern::Object(props) => {
                for (k, p, d) in props {
                    let mut x = self.get_member(&v, k)?;
                    if matches!(x, V::Undef)
                        && let Some(d) = d
                    {
                        x = self.eval(d, env)?;
                    }
                    self.bind(p, x, env, kind)?;
                }
                Ok(())
            }
        }
    }

    fn assign_name(&mut self, n: &str, v: V, env: &Env) -> Result<(), Flow> {
        match set_var(env, n, v.clone()) {
            Some(Ok(())) => Ok(()),
            Some(Err(())) => Err(type_err(format!("`{n}` is a const and cannot be assigned"))),
            None => {
                if n == "time" {
                    if let V::Num(t) = v {
                        self.cx.time = t;
                    }
                    return Ok(());
                }
                let mut root = env.clone();
                while let Some(p) = root.parent.clone() {
                    root = p;
                }
                declare(&root, n, v, false);
                Ok(())
            }
        }
    }

    fn exec(&mut self, st: &Stmt, env: &Env) -> Result<Option<V>, Flow> {
        self.line = st.line;
        self.tick()?;
        match &st.kind {
            StmtKind::Expr(e) => Ok(Some(self.eval(e, env)?)),
            StmtKind::Decl(kind, decls) => {
                for (pat, init) in decls {
                    let v = match init {
                        Some(e) => self.eval(e, env)?,
                        None => {
                            if *kind == DeclKind::Var {
                                continue;
                            }
                            V::Undef
                        }
                    };
                    self.bind(pat, v, env, Some(*kind))?;
                }
                Ok(None)
            }
            StmtKind::Func(_) | StmtKind::Empty => Ok(None),
            StmtKind::If(c, a, b) => {
                let cv = self.eval(c, env)?;
                let r = if truthy(&cv) {
                    self.exec_scoped(a, env)?
                } else if let Some(b) = b {
                    self.exec_scoped(b, env)?
                } else {
                    None
                };
                Ok(Some(r.unwrap_or(V::Undef)))
            }
            StmtKind::Block(b) => self.block(b, env),
            StmtKind::Labeled(label, inner) => match self.exec_labeled(inner, env, Some(label)) {
                Err(Flow::Break(Some(l))) if &l == label => Ok(Some(V::Undef)),
                other => other,
            },
            StmtKind::For { .. } | StmtKind::ForEach { .. } | StmtKind::While(..) | StmtKind::DoWhile(..) => self.exec_labeled(st, env, None),
            StmtKind::Break(l) => Err(Flow::Break(l.clone())),
            StmtKind::Continue(l) => Err(Flow::Continue(l.clone())),
            StmtKind::Return(e) => {
                let v = match e {
                    Some(e) => self.eval(e, env)?,
                    None => V::Undef,
                };
                Err(Flow::Return(v))
            }
            StmtKind::Throw(e) => {
                let v = self.eval(e, env)?;
                Err(Flow::Throw(v))
            }
            StmtKind::Try { block, param, handler, finally } => {
                let r = self.block(block, env);
                let r = match (r, handler) {
                    (Err(Flow::Throw(v)), Some(h)) => {
                        let inner = Scope::new(Some(env.clone()), false);
                        if let Some(p) = param {
                            self.bind(p, v, &inner, Some(DeclKind::Let))?;
                        }
                        self.exec_body(h, &inner, false)
                    }
                    (r, _) => r,
                };
                if let Some(f) = finally {
                    self.block(f, env)?;
                }
                r.map(|v| Some(v.unwrap_or(V::Undef)))
            }
            StmtKind::Switch(d, cases) => {
                let dv = self.eval(d, env)?;
                let inner = Scope::new(Some(env.clone()), false);
                let mut start = None;
                for (i, (test, _)) in cases.iter().enumerate() {
                    if let Some(t) = test {
                        let tv = self.eval(t, &inner)?;
                        if strict_eq(&dv, &tv) {
                            start = Some(i);
                            break;
                        }
                    }
                }
                if start.is_none() {
                    start = cases.iter().position(|(t, _)| t.is_none());
                }
                let mut last = None;
                if let Some(s0) = start {
                    for (_, body) in &cases[s0..] {
                        for st in body {
                            match self.exec(st, &inner) {
                                Ok(Some(v)) => last = Some(v),
                                Ok(None) => {}
                                Err(Flow::Break(None)) => return Ok(Some(last.unwrap_or(V::Undef))),
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
                Ok(Some(last.unwrap_or(V::Undef)))
            }
        }
    }

    fn exec_scoped(&mut self, st: &Stmt, env: &Env) -> Result<Option<V>, Flow> {
        self.exec(st, env)
    }

    fn loop_flow(&self, r: Result<Option<V>, Flow>, label: Option<&String>, last: &mut Option<V>) -> Result<bool, Flow> {
        match r {
            Ok(v) => {
                if v.is_some() {
                    *last = v;
                }
                Ok(true)
            }
            Err(Flow::Break(None)) => Ok(false),
            Err(Flow::Break(Some(l))) if Some(&l) == label => Ok(false),
            Err(Flow::Continue(None)) => Ok(true),
            Err(Flow::Continue(Some(l))) if Some(&l) == label => Ok(true),
            Err(e) => Err(e),
        }
    }

    fn exec_labeled(&mut self, st: &Stmt, env: &Env, label: Option<&String>) -> Result<Option<V>, Flow> {
        self.line = st.line;
        let mut last = None;
        match &st.kind {
            StmtKind::While(c, body) => {
                loop {
                    self.tick()?;
                    let cv = self.eval(c, env)?;
                    if !truthy(&cv) {
                        break;
                    }
                    let r = self.exec(body, env);
                    if !self.loop_flow(r, label, &mut last)? {
                        break;
                    }
                }
                Ok(Some(last.unwrap_or(V::Undef)))
            }
            StmtKind::DoWhile(body, c) => {
                loop {
                    self.tick()?;
                    let r = self.exec(body, env);
                    if !self.loop_flow(r, label, &mut last)? {
                        break;
                    }
                    let cv = self.eval(c, env)?;
                    if !truthy(&cv) {
                        break;
                    }
                }
                Ok(Some(last.unwrap_or(V::Undef)))
            }
            StmtKind::For { init, test, update, body } => {
                let scope = Scope::new(Some(env.clone()), false);
                let per_iter = matches!(init.as_deref().map(|s| &s.kind), Some(StmtKind::Decl(DeclKind::Let | DeclKind::Const, _)));
                if let Some(i) = init {
                    self.exec(i, &scope)?;
                }
                let mut cur = scope;
                loop {
                    self.tick()?;
                    if let Some(t) = test {
                        let tv = self.eval(t, &cur)?;
                        if !truthy(&tv) {
                            break;
                        }
                    }
                    let r = self.exec(body, &cur);
                    if !self.loop_flow(r, label, &mut last)? {
                        break;
                    }
                    if per_iter {
                        let next = Scope::new(cur.parent.clone(), false);
                        for (n, v, k) in cur.vars.borrow().iter() {
                            next.vars.borrow_mut().push((n.clone(), v.clone(), *k));
                        }
                        cur = next;
                    }
                    if let Some(u) = update {
                        self.eval(u, &cur)?;
                    }
                }
                Ok(Some(last.unwrap_or(V::Undef)))
            }
            StmtKind::ForEach { kind, pat, of, obj, body } => {
                let o = self.eval(obj, env)?;
                let items: Vec<V> = if *of {
                    self.iterate(&o)?
                } else {
                    self.keys_of(&o)?.into_iter().map(|k| s(&k)).collect()
                };
                for it in items {
                    self.tick()?;
                    let scope = Scope::new(Some(env.clone()), false);
                    self.bind(pat, it, &scope, *kind)?;
                    let r = self.exec(body, &scope);
                    if !self.loop_flow(r, label, &mut last)? {
                        break;
                    }
                }
                Ok(Some(last.unwrap_or(V::Undef)))
            }
            _ => self.exec(st, env),
        }
    }

    pub fn iterate(&mut self, v: &V) -> Result<Vec<V>, Flow> {
        match v {
            V::Arr(a) => Ok(a.borrow().clone()),
            V::Str(st) => Ok(st.chars().map(|c| s(&c.to_string())).collect()),
            V::Host(_) => {
                let p = self.primitive(v)?;
                if matches!(p, V::Host(_)) {
                    return Err(type_err("this object cannot be iterated"));
                }
                self.iterate(&p)
            }
            V::Undef | V::Null => Err(type_err(format!("{} is not iterable", self.to_string(v).unwrap_or_default()))),
            _ => Err(type_err("this value is not iterable")),
        }
    }

    pub fn keys_of(&mut self, v: &V) -> Result<Vec<String>, Flow> {
        Ok(match v {
            V::Obj(o) => o.borrow().props.iter().map(|(k, _)| k.to_string()).collect(),
            V::Arr(a) => (0..a.borrow().len()).map(|i| i.to_string()).collect(),
            V::Str(st) => (0..st.chars().count()).map(|i| i.to_string()).collect(),
            _ => vec![],
        })
    }

    pub fn primitive(&mut self, v: &V) -> R {
        match v {
            V::Host(h) => host::primitive(self, h),
            _ => Ok(v.clone()),
        }
    }

    pub fn to_number(&mut self, v: &V) -> Result<f64, Flow> {
        Ok(match v {
            V::Undef => f64::NAN,
            V::Null => 0.0,
            V::Bool(b) => *b as u8 as f64,
            V::Num(n) => *n,
            V::Str(st) => str_to_num(st),
            V::Arr(a) => {
                let a = a.borrow();
                match a.len() {
                    0 => 0.0,
                    1 => {
                        let x = a[0].clone();
                        drop(a);
                        self.to_number(&x)?
                    }
                    _ => f64::NAN,
                }
            }
            V::Host(_) => {
                let p = self.primitive(v)?;
                if matches!(p, V::Host(_)) { f64::NAN } else { self.to_number(&p)? }
            }
            _ => f64::NAN,
        })
    }

    pub fn to_string(&mut self, v: &V) -> Result<String, Flow> {
        Ok(match v {
            V::Undef => "undefined".into(),
            V::Null => "null".into(),
            V::Bool(b) => b.to_string(),
            V::Num(n) => num_to_string(*n),
            V::Str(st) => st.to_string(),
            V::Arr(a) => {
                let items = a.borrow().clone();
                let mut parts = Vec::with_capacity(items.len());
                for x in &items {
                    parts.push(if matches!(x, V::Undef | V::Null) { String::new() } else { self.to_string(x)? });
                }
                parts.join(",")
            }
            V::Obj(o) => {
                let b = o.borrow();
                match (b.get("name"), b.get("message")) {
                    (Some(n), Some(m)) => format!("{}: {}", plain_string(n), plain_string(m)),
                    _ => "[object Object]".into(),
                }
            }
            V::Func(c) => format!("function {}() {{ [code] }}", c.def.name.clone().unwrap_or_default()),
            V::Native(n) => format!("function {}() {{ [native code] }}", n.name),
            V::Host(h) => {
                let p = self.primitive(v)?;
                if matches!(p, V::Host(_)) { h.describe() } else { self.to_string(&p)? }
            }
            V::Path(_) => "[object Path]".into(),
        })
    }

    fn lookup(&mut self, name: &str, env: &Env) -> R {
        if let Some(v) = find_var(env, name) {
            return Ok(v);
        }
        if self.script {
            if let Some(v) = super::script::global(self, name)? {
                return Ok(v);
            }
            if let Some(v) = builtins::global(name) {
                return Ok(v);
            }
            return Err(throw("ReferenceError", format!("`{name}` is not defined")));
        }
        if let Some(v) = host::context(self, name)? {
            return Ok(v);
        }
        if let Some(v) = builtins::global(name) {
            return Ok(v);
        }
        if let Some(v) = host::implicit(self, name)? {
            return Ok(v);
        }
        Err(throw("ReferenceError", format!("`{name}` is not defined")))
    }

    pub fn get_member(&mut self, o: &V, key: &str) -> R {
        match o {
            V::Undef | V::Null => Err(type_err(format!("cannot read `{key}` of {}", if matches!(o, V::Null) { "null" } else { "undefined" }))),
            V::Str(st) => {
                if key == "length" {
                    return Ok(V::Num(st.encode_utf16().count() as f64));
                }
                if let Ok(i) = key.parse::<usize>() {
                    return Ok(st.chars().nth(i).map(|c| s(&c.to_string())).unwrap_or(V::Undef));
                }
                Ok(builtins::string_method(key).map(|f| native(f.0, o.clone(), f.1)).unwrap_or(V::Undef))
            }
            V::Num(_) => Ok(builtins::number_method(key).map(|f| native(f.0, o.clone(), f.1)).unwrap_or(V::Undef)),
            V::Bool(_) => Ok(match key {
                "toString" | "valueOf" => native("toString", o.clone(), builtins::to_string_method),
                _ => V::Undef,
            }),
            V::Arr(a) => {
                if key == "length" {
                    return Ok(V::Num(a.borrow().len() as f64));
                }
                if let Ok(i) = key.parse::<usize>() {
                    return Ok(a.borrow().get(i).cloned().unwrap_or(V::Undef));
                }
                Ok(builtins::array_method(key).map(|f| native(f.0, o.clone(), f.1)).unwrap_or(V::Undef))
            }
            V::Obj(ob) => {
                if let Some(v) = ob.borrow().get(key) {
                    return Ok(v.clone());
                }
                Ok(match key {
                    "hasOwnProperty" => native("hasOwnProperty", o.clone(), builtins::has_own),
                    "toString" => native("toString", o.clone(), builtins::to_string_method),
                    _ => V::Undef,
                })
            }
            V::Func(c) => Ok(match key {
                "name" => s(&c.def.name.clone().unwrap_or_default()),
                "length" => V::Num(c.def.params.iter().filter(|p| p.default.is_none() && !p.rest).count() as f64),
                "call" => native("call", o.clone(), builtins::fn_call),
                "apply" => native("apply", o.clone(), builtins::fn_apply),
                "bind" => native("bind", o.clone(), builtins::fn_bind),
                _ => V::Undef,
            }),
            V::Native(n) => Ok(match key {
                "name" => s(n.name),
                "call" => native("call", o.clone(), builtins::fn_call),
                "apply" => native("apply", o.clone(), builtins::fn_apply),
                _ => {
                    if self.script
                        && let Some(v) = super::script::static_get(n.name, key)
                    {
                        return Ok(v);
                    }
                    builtins::ns_get(n.name, key).unwrap_or(V::Undef)
                }
            }),
            V::Host(h) => {
                if self.script
                    && let Some(v) = super::script::get(self, h, key)?
                {
                    return Ok(v);
                }
                host::get(self, h, key)
            }
            V::Path(_) => Ok(host::path_method(key).map(|f| native(f.0, o.clone(), f.1)).unwrap_or(V::Undef)),
        }
    }

    fn get_index(&mut self, o: &V, k: &V) -> R {
        if let V::Num(n) = k {
            let i = *n;
            match o {
                V::Arr(a) => {
                    return Ok(if i >= 0.0 && i.fract() == 0.0 { a.borrow().get(i as usize).cloned().unwrap_or(V::Undef) } else { V::Undef });
                }
                V::Host(_) => {
                    let p = self.primitive(o)?;
                    if !matches!(p, V::Host(_)) {
                        return self.get_index(&p, k);
                    }
                }
                _ => {}
            }
        }
        let key = self.to_string(k)?;
        self.get_member(o, &key)
    }

    fn set_member(&mut self, o: &V, key: &str, v: V) -> Result<(), Flow> {
        match o {
            V::Obj(ob) => {
                ob.borrow_mut().set(key, v);
                Ok(())
            }
            V::Arr(a) => {
                if key == "length" {
                    let n = self.to_number(&v)?;
                    if !(n >= 0.0 && n.fract() == 0.0 && n <= 10_000_000.0) {
                        return Err(throw("RangeError", "invalid array length"));
                    }
                    a.borrow_mut().resize(n as usize, V::Undef);
                    return Ok(());
                }
                if let Ok(i) = key.parse::<usize>() {
                    if i > 10_000_000 {
                        return Err(throw("RangeError", "array index too large"));
                    }
                    let mut b = a.borrow_mut();
                    if i >= b.len() {
                        b.resize(i + 1, V::Undef);
                    }
                    b[i] = v;
                }
                Ok(())
            }
            V::Undef | V::Null => Err(type_err(format!("cannot set `{key}` of {}", if matches!(o, V::Null) { "null" } else { "undefined" }))),
            V::Host(h) if self.script => super::script::set(self, h, key, v),
            V::Host(_) => Err(type_err(format!("`{key}` is read-only in an expression"))),
            _ => Ok(()),
        }
    }

    fn reference(&mut self, e: &Expr, env: &Env) -> Result<Ref, Flow> {
        match e {
            Expr::Ident(n) => Ok(Ref::Var(n.clone())),
            Expr::Member(o, k, _) => Ok(Ref::Member(self.eval(o, env)?, k.clone())),
            Expr::Index(o, k, _) => {
                let ov = self.eval(o, env)?;
                let kv = self.eval(k, env)?;
                let key = self.to_string(&kv)?;
                Ok(Ref::Member(ov, key))
            }
            _ => Err(throw("SyntaxError", "invalid assignment target")),
        }
    }

    fn read_ref(&mut self, r: &Ref, env: &Env) -> R {
        match r {
            Ref::Var(n) => self.lookup(n, env),
            Ref::Member(o, k) => self.get_member(o, k),
        }
    }

    fn write_ref(&mut self, r: &Ref, v: V, env: &Env) -> Result<(), Flow> {
        match r {
            Ref::Var(n) => self.assign_name(n, v, env),
            Ref::Member(o, k) => self.set_member(o, k, v),
        }
    }

    fn eval_args(&mut self, args: &[Arg], env: &Env) -> Result<Vec<V>, Flow> {
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            match a {
                Arg::Plain(e) => out.push(self.eval(e, env)?),
                Arg::Spread(e) => {
                    let v = self.eval(e, env)?;
                    out.extend(self.iterate(&v)?);
                }
            }
        }
        Ok(out)
    }

    pub fn call(&mut self, f: &V, this: V, args: Vec<V>) -> R {
        self.tick()?;
        match f {
            V::Func(c) => {
                if self.depth >= MAX_CALL_DEPTH {
                    return Err(throw("RangeError", "too much recursion"));
                }
                self.depth += 1;
                let r = self.call_closure(c, this, args);
                self.depth -= 1;
                r
            }
            V::Native(n) => (n.f)(self, &n.this, args),
            V::Host(h) if h.callable() => host::call(self, h, args),
            _ => Err(type_err(format!("{} is not a function", self.to_string(f).unwrap_or_else(|_| "this value".into())))),
        }
    }

    fn call_closure(&mut self, c: &Rc<Closure>, this: V, args: Vec<V>) -> R {
        let env = Scope::new(Some(c.env.clone()), true);
        let def = c.def.clone();
        if !def.arrow {
            declare(&env, "arguments", arr(args.clone()), false);
            if let Some(n) = &def.name
                && find_var(&env, n).is_none()
            {
                declare(&env, n, V::Func(c.clone()), false);
            }
        }
        for (i, p) in def.params.iter().enumerate() {
            let v = if p.rest {
                arr(args.get(i..).map(|s| s.to_vec()).unwrap_or_default())
            } else {
                let mut v = args.get(i).cloned().unwrap_or(V::Undef);
                if matches!(v, V::Undef)
                    && let Some(d) = &p.default
                {
                    v = self.eval(d, &env)?;
                }
                v
            };
            self.bind(&p.pat, v, &env, Some(DeclKind::Let))?;
        }
        let saved_this = std::mem::replace(&mut self.this, if def.arrow { c.this.clone().unwrap_or(V::Undef) } else { this });
        let saved_line = self.line;
        let r = match &def.body {
            Body::Expr(e) => self.eval(e, &env),
            Body::Block(stmts) => match self.exec_body(stmts, &env, true) {
                Ok(_) => Ok(V::Undef),
                Err(Flow::Return(v)) => Ok(v),
                Err(e) => Err(e),
            },
        };
        self.this = saved_this;
        if r.is_ok() {
            self.line = saved_line;
        }
        r
    }

    pub fn construct(&mut self, f: &V, args: Vec<V>) -> R {
        match f {
            V::Native(n) if self.script && super::script::is_class(n.name) => super::script::construct(self, n.name, args),
            V::Native(n) => builtins::construct(self, n.name, args),
            V::Func(_) => {
                let o = obj(vec![]);
                let r = self.call(f, o.clone(), args)?;
                Ok(if matches!(r, V::Obj(_) | V::Arr(_)) { r } else { o })
            }
            _ => Err(type_err("this value is not a constructor")),
        }
    }

    pub fn eval(&mut self, e: &Expr, env: &Env) -> R {
        match e {
            Expr::Num(n) => Ok(V::Num(*n)),
            Expr::Str(st) => Ok(s(st)),
            Expr::Bool(b) => Ok(V::Bool(*b)),
            Expr::Null => Ok(V::Null),
            Expr::Undefined => Ok(V::Undef),
            Expr::Tmpl(quasis, exprs) => {
                let mut out = String::new();
                for (i, q) in quasis.iter().enumerate() {
                    out.push_str(q);
                    if let Some(x) = exprs.get(i) {
                        let v = self.eval(x, env)?;
                        out.push_str(&self.to_string(&v)?);
                    }
                }
                Ok(s(&out))
            }
            Expr::Ident(n) => self.lookup(n, env),
            Expr::This => Ok(match &self.this {
                V::Undef => host::this_layer(self),
                v => v.clone(),
            }),
            Expr::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for it in items {
                    match it {
                        None => out.push(V::Undef),
                        Some(Arg::Plain(x)) => out.push(self.eval(x, env)?),
                        Some(Arg::Spread(x)) => {
                            let v = self.eval(x, env)?;
                            out.extend(self.iterate(&v)?);
                        }
                    }
                }
                Ok(arr(out))
            }
            Expr::Object(entries) => {
                let mut o = Obj::default();
                for en in entries {
                    match en {
                        ObjEntry::Pair(k, x) => {
                            let key = match k {
                                PropKey::Name(n) => n.clone(),
                                PropKey::Computed(c) => {
                                    let kv = self.eval(c, env)?;
                                    self.to_string(&kv)?
                                }
                            };
                            let v = self.eval(x, env)?;
                            o.set(&key, v);
                        }
                        ObjEntry::Spread(x) => {
                            let v = self.eval(x, env)?;
                            if let V::Obj(src) = &v {
                                for (k, val) in src.borrow().props.iter() {
                                    o.set(k, val.clone());
                                }
                            } else if let V::Arr(a) = &v {
                                for (i, val) in a.borrow().iter().enumerate() {
                                    o.set(&i.to_string(), val.clone());
                                }
                            }
                        }
                    }
                }
                Ok(V::Obj(Rc::new(RefCell::new(o))))
            }
            Expr::Func(def) => Ok(V::Func(Rc::new(Closure { def: def.clone(), env: env.clone(), this: if def.arrow { Some(self.this.clone()) } else { None } }))),
            Expr::Unary(op, x) => {
                if *op == UnOp::TypeOf
                    && let Expr::Ident(n) = &**x
                {
                    return Ok(match self.lookup(n, env) {
                        Ok(v) => s(type_of(&v)),
                        Err(Flow::Throw(_)) => s("undefined"),
                        Err(f) => return Err(f),
                    });
                }
                if *op == UnOp::Delete {
                    return match &**x {
                        Expr::Member(o, k, _) => {
                            let ov = self.eval(o, env)?;
                            if let V::Obj(ob) = &ov {
                                ob.borrow_mut().remove(k);
                            }
                            Ok(V::Bool(true))
                        }
                        Expr::Index(o, k, _) => {
                            let ov = self.eval(o, env)?;
                            let kv = self.eval(k, env)?;
                            let key = self.to_string(&kv)?;
                            if let V::Obj(ob) = &ov {
                                ob.borrow_mut().remove(&key);
                            }
                            Ok(V::Bool(true))
                        }
                        _ => Ok(V::Bool(true)),
                    };
                }
                let v = self.eval(x, env)?;
                match op {
                    UnOp::Neg => {
                        let p = self.primitive(&v)?;
                        if let V::Arr(a) = &p {
                            let items = a.borrow().clone();
                            let mut out = vec![];
                            for it in &items {
                                out.push(V::Num(-self.to_number(it)?));
                            }
                            return Ok(arr(out));
                        }
                        Ok(V::Num(-self.to_number(&p)?))
                    }
                    UnOp::Plus => Ok(V::Num(self.to_number(&v)?)),
                    UnOp::Not => Ok(V::Bool(!truthy(&v))),
                    UnOp::BitNot => Ok(V::Num(!(to_i32(self.to_number(&v)?)) as f64)),
                    UnOp::TypeOf => Ok(s(type_of(&v))),
                    UnOp::Void => Ok(V::Undef),
                    UnOp::Delete => Ok(V::Bool(true)),
                }
            }
            Expr::Update { inc, prefix, target } => {
                let r = self.reference(target, env)?;
                let old = self.read_ref(&r, env)?;
                let n = self.to_number(&old)?;
                let new = if *inc { n + 1.0 } else { n - 1.0 };
                self.write_ref(&r, V::Num(new), env)?;
                Ok(V::Num(if *prefix { new } else { n }))
            }
            Expr::Binary(op, a, b) => {
                let av = self.eval(a, env)?;
                let bv = self.eval(b, env)?;
                self.binary(*op, &av, &bv)
            }
            Expr::Logic(op, a, b) => {
                let av = self.eval(a, env)?;
                match op {
                    LogicOp::And => {
                        if truthy(&av) {
                            self.eval(b, env)
                        } else {
                            Ok(av)
                        }
                    }
                    LogicOp::Or => {
                        if truthy(&av) {
                            Ok(av)
                        } else {
                            self.eval(b, env)
                        }
                    }
                    LogicOp::Nullish => {
                        if matches!(av, V::Undef | V::Null) {
                            self.eval(b, env)
                        } else {
                            Ok(av)
                        }
                    }
                }
            }
            Expr::Cond(c, a, b) => {
                let cv = self.eval(c, env)?;
                if truthy(&cv) { self.eval(a, env) } else { self.eval(b, env) }
            }
            Expr::Assign(op, target, value) => {
                let r = self.reference(target, env)?;
                let v = match op {
                    None => self.eval(value, env)?,
                    Some(bop) => {
                        let old = self.read_ref(&r, env)?;
                        let rhs = self.eval(value, env)?;
                        self.binary(*bop, &old, &rhs)?
                    }
                };
                self.write_ref(&r, v.clone(), env)?;
                Ok(v)
            }
            Expr::LogicAssign(op, target, value) => {
                let r = self.reference(target, env)?;
                let old = self.read_ref(&r, env).unwrap_or(V::Undef);
                let go = match op {
                    LogicOp::And => truthy(&old),
                    LogicOp::Or => !truthy(&old),
                    LogicOp::Nullish => matches!(old, V::Undef | V::Null),
                };
                if !go {
                    return Ok(old);
                }
                let v = self.eval(value, env)?;
                self.write_ref(&r, v.clone(), env)?;
                Ok(v)
            }
            Expr::DestructAssign(pat, value) => {
                let v = self.eval(value, env)?;
                self.bind(pat, v.clone(), env, None)?;
                Ok(v)
            }
            Expr::Member(o, k, opt) => {
                let ov = self.eval(o, env)?;
                if *opt && matches!(ov, V::Undef | V::Null) {
                    return Ok(V::Undef);
                }
                self.get_member(&ov, k)
            }
            Expr::Index(o, k, opt) => {
                let ov = self.eval(o, env)?;
                if *opt && matches!(ov, V::Undef | V::Null) {
                    return Ok(V::Undef);
                }
                let kv = self.eval(k, env)?;
                self.get_index(&ov, &kv)
            }
            Expr::Call(callee, args, opt) => {
                let (f, this) = match &**callee {
                    Expr::Member(o, k, mopt) => {
                        let ov = self.eval(o, env)?;
                        if *mopt && matches!(ov, V::Undef | V::Null) {
                            return Ok(V::Undef);
                        }
                        let f = self.get_member(&ov, k)?;
                        if matches!(f, V::Undef) && !*opt {
                            let what = self.to_string(&ov).map(|t| if t.len() > 40 { format!("{}...", &t[..t.char_indices().nth(40).map(|x| x.0).unwrap_or(t.len())]) } else { t }).unwrap_or_default();
                            return Err(type_err(format!("`{k}` is not a function of {what}")));
                        }
                        (f, ov)
                    }
                    Expr::Index(o, k, _) => {
                        let ov = self.eval(o, env)?;
                        let kv = self.eval(k, env)?;
                        (self.get_index(&ov, &kv)?, ov)
                    }
                    other => (self.eval(other, env)?, V::Undef),
                };
                if *opt && matches!(f, V::Undef | V::Null) {
                    return Ok(V::Undef);
                }
                let a = self.eval_args(args, env)?;
                if let (Expr::Ident(n), V::Undef | V::Null) = (&**callee, &f) {
                    return Err(type_err(format!("`{n}` is not a function")));
                }
                self.call(&f, this, a)
            }
            Expr::New(callee, args) => {
                let f = self.eval(callee, env)?;
                let a = self.eval_args(args, env)?;
                self.construct(&f, a)
            }
            Expr::Seq(items) => {
                let mut last = V::Undef;
                for it in items {
                    last = self.eval(it, env)?;
                }
                Ok(last)
            }
        }
    }

    pub fn vector(&mut self, v: &V) -> Result<Option<Vec<f64>>, Flow> {
        let p = self.primitive(v)?;
        if let V::Arr(a) = &p {
            let items = a.borrow().clone();
            let mut out = Vec::with_capacity(items.len());
            for it in &items {
                out.push(self.to_number(it)?);
            }
            return Ok(Some(out));
        }
        Ok(None)
    }

    pub fn binary(&mut self, op: BinOp, a: &V, b: &V) -> R {
        match op {
            BinOp::StrictEq => return Ok(V::Bool(strict_eq(a, b))),
            BinOp::StrictNe => return Ok(V::Bool(!strict_eq(a, b))),
            BinOp::InstanceOf => {
                return Ok(V::Bool(match (a, b) {
                    (V::Arr(_), V::Native(n)) => n.name == "Array",
                    (V::Obj(o), V::Native(n)) => n.name == "Object" || o.borrow().get("name").is_some_and(|x| plain_string(x) == n.name),
                    (V::Func(_) | V::Native(_), V::Native(n)) => n.name == "Function" || n.name == "Object",
                    (V::Host(h), V::Native(n)) if self.script => super::script::instance_of(self, h, n.name),
                    _ => false,
                }));
            }
            BinOp::In => {
                let k = self.to_string(a)?;
                return Ok(V::Bool(match b {
                    V::Obj(o) => o.borrow().get(&k).is_some(),
                    V::Arr(x) => k == "length" || k.parse::<usize>().is_ok_and(|i| i < x.borrow().len()),
                    _ => return Err(type_err("`in` needs an object on the right")),
                }));
            }
            _ => {}
        }
        let pa = self.primitive(a)?;
        let pb = self.primitive(b)?;
        match op {
            BinOp::Eq => return Ok(V::Bool(self.loose_eq(&pa, &pb)?)),
            BinOp::Ne => return Ok(V::Bool(!self.loose_eq(&pa, &pb)?)),
            _ => {}
        }
        let va = if let V::Arr(_) = pa { self.vector(&pa)? } else { None };
        let vb = if let V::Arr(_) = pb { self.vector(&pb)? } else { None };
        if matches!(op, BinOp::Add | BinOp::Sub) && (va.is_some() || vb.is_some()) && !matches!(pa, V::Str(_)) && !matches!(pb, V::Str(_)) {
            let x = match &va {
                Some(v) => v.clone(),
                None => vec![self.to_number(&pa)?],
            };
            let y = match &vb {
                Some(v) => v.clone(),
                None => vec![self.to_number(&pb)?],
            };
            let n = x.len().max(y.len());
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let p = x.get(i).copied().unwrap_or(0.0);
                let q = y.get(i).copied().unwrap_or(0.0);
                out.push(if op == BinOp::Add { p + q } else { p - q });
            }
            return Ok(nums(&out));
        }
        if matches!(op, BinOp::Mul | BinOp::Div) {
            match (&va, &vb) {
                (Some(x), None) => {
                    let k = self.to_number(&pb)?;
                    return Ok(nums(&x.iter().map(|p| if op == BinOp::Mul { p * k } else { p / k }).collect::<Vec<_>>()));
                }
                (None, Some(y)) if op == BinOp::Mul => {
                    let k = self.to_number(&pa)?;
                    return Ok(nums(&y.iter().map(|p| p * k).collect::<Vec<_>>()));
                }
                (Some(x), Some(y)) if op == BinOp::Mul => {
                    let n = x.len().min(y.len());
                    return Ok(nums(&(0..n).map(|i| x[i] * y[i]).collect::<Vec<_>>()));
                }
                _ => {}
            }
        }
        if op == BinOp::Add && (matches!(pa, V::Str(_) | V::Arr(_) | V::Obj(_)) || matches!(pb, V::Str(_) | V::Arr(_) | V::Obj(_))) {
            let x = self.to_string(&pa)?;
            let y = self.to_string(&pb)?;
            return Ok(s(&(x + &y)));
        }
        if matches!(op, BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge)
            && let (V::Str(x), V::Str(y)) = (&pa, &pb)
        {
            return Ok(V::Bool(match op {
                BinOp::Lt => x < y,
                BinOp::Gt => x > y,
                BinOp::Le => x <= y,
                _ => x >= y,
            }));
        }
        let x = self.to_number(&pa)?;
        let y = self.to_number(&pb)?;
        Ok(match op {
            BinOp::Add => V::Num(x + y),
            BinOp::Sub => V::Num(x - y),
            BinOp::Mul => V::Num(x * y),
            BinOp::Div => V::Num(x / y),
            BinOp::Mod => V::Num(x % y),
            BinOp::Pow => V::Num(x.powf(y)),
            BinOp::Lt => V::Bool(x < y),
            BinOp::Gt => V::Bool(x > y),
            BinOp::Le => V::Bool(x <= y),
            BinOp::Ge => V::Bool(x >= y),
            BinOp::BitAnd => V::Num((to_i32(x) & to_i32(y)) as f64),
            BinOp::BitOr => V::Num((to_i32(x) | to_i32(y)) as f64),
            BinOp::BitXor => V::Num((to_i32(x) ^ to_i32(y)) as f64),
            BinOp::Shl => V::Num(to_i32(x).wrapping_shl(to_u32(y) & 31) as f64),
            BinOp::Shr => V::Num(to_i32(x).wrapping_shr(to_u32(y) & 31) as f64),
            BinOp::UShr => V::Num(to_u32(x).wrapping_shr(to_u32(y) & 31) as f64),
            _ => V::Undef,
        })
    }

    fn loose_eq(&mut self, a: &V, b: &V) -> Result<bool, Flow> {
        Ok(match (a, b) {
            (V::Undef | V::Null, V::Undef | V::Null) => true,
            (V::Undef | V::Null, _) | (_, V::Undef | V::Null) => false,
            (V::Num(_) | V::Str(_) | V::Bool(_), V::Num(_) | V::Str(_) | V::Bool(_)) => {
                if let (V::Str(x), V::Str(y)) = (a, b) {
                    x == y
                } else {
                    self.to_number(a)? == self.to_number(b)?
                }
            }
            (V::Arr(_), V::Num(_) | V::Str(_) | V::Bool(_)) => {
                let t = self.to_string(a)?;
                self.loose_eq(&s(&t), b)?
            }
            (V::Num(_) | V::Str(_) | V::Bool(_), V::Arr(_)) => {
                let t = self.to_string(b)?;
                self.loose_eq(a, &s(&t))?
            }
            _ => strict_eq(a, b),
        })
    }
}

pub fn plain_string(v: &V) -> String {
    match v {
        V::Str(s) => s.to_string(),
        V::Num(n) => num_to_string(*n),
        V::Bool(b) => b.to_string(),
        V::Null => "null".into(),
        V::Undef => "undefined".into(),
        _ => String::new(),
    }
}

pub fn strict_eq(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::Undef, V::Undef) | (V::Null, V::Null) => true,
        (V::Bool(x), V::Bool(y)) => x == y,
        (V::Num(x), V::Num(y)) => x == y,
        (V::Str(x), V::Str(y)) => x == y,
        (V::Arr(x), V::Arr(y)) => Rc::ptr_eq(x, y),
        (V::Obj(x), V::Obj(y)) => Rc::ptr_eq(x, y),
        (V::Func(x), V::Func(y)) => Rc::ptr_eq(x, y),
        (V::Native(x), V::Native(y)) => Rc::ptr_eq(x, y) || (x.name == y.name && x.f as usize == y.f as usize),
        (V::Host(x), V::Host(y)) => **x == **y,
        (V::Path(x), V::Path(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

pub fn to_i32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    (x.trunc() as i64 as u64 as u32) as i32
}

pub fn to_u32(x: f64) -> u32 {
    if !x.is_finite() {
        return 0;
    }
    x.trunc() as i64 as u64 as u32
}

fn collect_vars(stmts: &[Stmt], out: &mut Vec<String>) {
    for st in stmts {
        collect_stmt(st, out);
    }
}

fn pattern_names(p: &Pattern, out: &mut Vec<String>) {
    match p {
        Pattern::Name(n) => out.push(n.clone()),
        Pattern::Array(items, rest) => {
            for (p, _) in items.iter().flatten() {
                pattern_names(p, out);
            }
            if let Some(r) = rest {
                pattern_names(r, out);
            }
        }
        Pattern::Object(props) => {
            for (_, p, _) in props {
                pattern_names(p, out);
            }
        }
    }
}

fn collect_stmt(st: &Stmt, out: &mut Vec<String>) {
    match &st.kind {
        StmtKind::Decl(DeclKind::Var, d) => {
            for (p, _) in d {
                pattern_names(p, out);
            }
        }
        StmtKind::If(_, a, b) => {
            collect_stmt(a, out);
            if let Some(b) = b {
                collect_stmt(b, out);
            }
        }
        StmtKind::Block(b) => collect_vars(b, out),
        StmtKind::For { init, body, .. } => {
            if let Some(i) = init {
                collect_stmt(i, out);
            }
            collect_stmt(body, out);
        }
        StmtKind::ForEach { kind, pat, body, .. } => {
            if *kind == Some(DeclKind::Var) {
                pattern_names(pat, out);
            }
            collect_stmt(body, out);
        }
        StmtKind::While(_, b) | StmtKind::DoWhile(b, _) | StmtKind::Labeled(_, b) => collect_stmt(b, out),
        StmtKind::Try { block, handler, finally, .. } => {
            collect_vars(block, out);
            if let Some(h) = handler {
                collect_vars(h, out);
            }
            if let Some(f) = finally {
                collect_vars(f, out);
            }
        }
        StmtKind::Switch(_, cases) => {
            for (_, b) in cases {
                collect_vars(b, out);
            }
        }
        _ => {}
    }
}
