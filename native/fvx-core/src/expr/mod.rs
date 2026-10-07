pub mod ast;
mod builtins;
pub mod host;
pub mod interp;
mod lex;
pub mod noise;
pub mod parse;
pub mod regexp;
pub mod script;
pub mod scriptui;
pub mod textstyle;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

use crate::eval::Eval;
use crate::model::{Layer, Prop, Value};
use host::Cx;
use interp::{Flow, Interp};

type Parsed = Arc<Result<ast::Program, String>>;

static CACHE: OnceLock<Mutex<HashMap<String, Parsed>>> = OnceLock::new();
const MAX_NEST: usize = 32;

thread_local! {
    static STACK: RefCell<Vec<(u64, u64)>> = const { RefCell::new(Vec::new()) };
    static CYCLE: std::cell::Cell<Option<(u64, u64)>> = const { std::cell::Cell::new(None) };
}

const CYCLE_MSG: &str = "the expression refers back to itself through another expression";

pub fn program(text: &str) -> Parsed {
    let cache = CACHE.get_or_init(Default::default);
    if let Some(p) = cache.lock().get(text) {
        return p.clone();
    }
    let parsed = Arc::new(parse::parse_program(text));
    let mut c = cache.lock();
    if c.len() > 4096 {
        c.clear();
    }
    c.insert(text.to_string(), parsed.clone());
    parsed
}

pub fn active(prop: &Prop) -> bool {
    prop.expr.as_ref().is_some_and(|e| e.enabled && !e.text.trim().is_empty())
}

pub fn evaluate(ev: &Eval, layer: &Layer, prop: &Prop) -> Value {
    if !active(prop) {
        return prop.at(layer.layer_time(ev.t));
    }
    run(ev, layer, prop, None).unwrap_or_else(|_| prop.at(layer.layer_time(ev.t)))
}

pub fn evaluate_text(ev: &Eval, layer: &Layer, prop: &Prop, index: f64, total: f64, selector: [f64; 3]) -> Value {
    if !active(prop) {
        return prop.at(layer.layer_time(ev.t));
    }
    run(ev, layer, prop, Some([index, total, selector[0], selector[1], selector[2]])).unwrap_or_else(|_| prop.at(layer.layer_time(ev.t)))
}

pub fn check(ev: &Eval, layer: &Layer, prop: &Prop) -> Option<String> {
    if !active(prop) {
        return None;
    }
    let text = (prop.id == "amount" && prop.expr.as_ref().is_some_and(|e| e.text.contains("textIndex") || e.text.contains("selectorValue") || e.text.contains("textTotal"))).then_some([1.0, 1.0, 100.0, 100.0, 100.0]);
    run(ev, layer, prop, text).err()
}

fn message(it: &mut Interp, f: Flow) -> String {
    match f {
        Flow::Throw(v) => format!("{} (line {})", it.describe_error(&v), it.line),
        Flow::Fatal(m) => m,
        Flow::Return(_) | Flow::Break(_) | Flow::Continue(_) => format!("SyntaxError: misplaced return, break or continue (line {})", it.line),
    }
}

fn run(ev: &Eval, layer: &Layer, prop: &Prop, text_vars: Option<[f64; 5]>) -> Result<Value, String> {
    let text = prop.expr.as_ref().map(|e| e.text.as_str()).unwrap_or("");
    let key = (layer.id, prop.uid);
    let entered = STACK.with(|s| {
        let mut s = s.borrow_mut();
        if s.last() == Some(&key) {
            Err(false)
        } else if s.contains(&key) {
            Err(true)
        } else if s.len() >= MAX_NEST {
            Err(false)
        } else {
            s.push(key);
            Ok(())
        }
    });
    match entered {
        Err(true) => {
            CYCLE.with(|c| {
                if c.get().is_none() {
                    c.set(Some(key));
                }
            });
            return Err(CYCLE_MSG.into());
        }
        Err(false) => return Err("expressions refer to each other too deeply, or to themselves".into()),
        Ok(()) => {}
    }
    let parsed = program(text);
    let r = match &*parsed {
        Err(m) => Err(m.clone()),
        Ok(prog) => {
            let mut cx = Cx::new(ev.project, ev.comp_id, ev.comp, layer, prop, ev.t);
            cx.text = text_vars;
            let mut it = Interp::new(cx);
            match it.run(prog) {
                Ok(v) => {
                    let pre = prop.at(layer.layer_time(ev.t));
                    match host::from_v(&mut it, &v, &pre) {
                        Ok(out) => Ok(prop.ui.clamp(out)),
                        Err(f) => Err(message(&mut it, f)),
                    }
                }
                Err(m) => Err(m),
            }
        }
    };
    STACK.with(|s| {
        s.borrow_mut().pop();
    });
    let cycled = CYCLE.with(|c| match c.get() {
        Some(k) if k == key => {
            c.set(None);
            true
        }
        Some(_) => true,
        None => false,
    });
    if cycled {
        return Err(CYCLE_MSG.into());
    }
    r
}
