use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::{Value as Json, json};

use super::host::{H, hv};
use super::interp::*;
use crate::engine::Engine;

#[derive(Clone)]
struct Ctl {
    id: u64,
    parent: u64,
    kind: String,
    text: String,
    value: f64,
    min: f64,
    max: f64,
    items: Vec<String>,
    selection: Option<usize>,
    enabled: bool,
    visible: bool,
    name: String,
    multiline: bool,
    handlers: BTreeMap<String, V>,
    extra: BTreeMap<String, V>,
}

impl Ctl {
    fn new(id: u64, parent: u64, kind: &str) -> Ctl {
        Ctl {
            id,
            parent,
            kind: kind.to_string(),
            text: String::new(),
            value: 0.0,
            min: 0.0,
            max: 100.0,
            items: vec![],
            selection: None,
            enabled: true,
            visible: true,
            name: String::new(),
            multiline: false,
            handlers: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }
}

struct Win {
    kind: String,
    title: String,
    panel: String,
    ctls: Vec<Ctl>,
    next: u64,
    shown: bool,
    dirty: bool,
    closed: bool,
}

#[derive(Default)]
struct Store {
    next: u64,
    wins: BTreeMap<u64, Win>,
}

thread_local! {
    static STORE: RefCell<Store> = RefCell::new(Store::default());
}

const CONTAINERS: [&str; 5] = ["window", "group", "panel", "tabbedpanel", "tab"];
const LISTS: [&str; 3] = ["dropdownlist", "listbox", "treeview"];
const KINDS: [&str; 18] = [
    "button", "iconbutton", "statictext", "edittext", "checkbox", "radiobutton", "slider", "scrollbar", "progressbar", "dropdownlist", "listbox", "treeview", "group", "panel", "tabbedpanel", "tab", "image", "flashplayer",
];

pub fn exists(w: u64) -> bool {
    STORE.with(|s| s.borrow().wins.get(&w).is_some_and(|x| !x.closed))
}

pub fn is_panel(h: &H) -> bool {
    let H::Window(w) = h else { return false };
    STORE.with(|s| s.borrow().wins.get(w).is_some_and(|x| x.kind == "panel"))
}

pub fn global(name: &str) -> Result<Option<V>, Flow> {
    Ok(Some(match name {
        "ScriptUI" => obj(vec![
            ("version", s("formvfx")),
            ("environment", obj(vec![("keyboardState", obj(vec![("shiftKey", V::Bool(false)), ("altKey", V::Bool(false)), ("ctrlKey", V::Bool(false)), ("metaKey", V::Bool(false))]))])),
            ("newFont", native("newFont", V::Undef, g_obj)),
            ("newImage", native("newImage", V::Undef, g_obj)),
            ("FontStyle", obj(vec![("REGULAR", s("REGULAR")), ("BOLD", s("BOLD")), ("ITALIC", s("ITALIC")), ("BOLDITALIC", s("BOLDITALIC"))])),
        ]),
        _ => return Ok(None),
    }))
}

fn g_obj(_: &mut Interp, _: &V, _: Vec<V>) -> R {
    Ok(obj(vec![]))
}

fn new_window(kind: &str, title: &str, panel: &str) -> u64 {
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        s.next += 1;
        let id = s.next;
        let mut root = Ctl::new(0, 0, "window");
        root.text = title.to_string();
        root.extra.insert("orientation".into(), V::Str(Rc::from("column")));
        s.wins.insert(id, Win { kind: kind.to_string(), title: title.to_string(), panel: panel.to_string(), ctls: vec![root], next: 1, shown: false, dirty: true, closed: false });
        id
    })
}

pub fn construct(it: &mut Interp, name: &str, a: Vec<V>) -> R {
    let kind = match arg(&a, 0) {
        V::Undef => if name == "Panel" { "panel".to_string() } else { "palette".to_string() },
        v => it.to_string(&v)?.to_ascii_lowercase(),
    };
    let kind = match kind.as_str() {
        "dialog" | "palette" | "window" | "panel" => kind,
        _ => "palette".to_string(),
    };
    let title = match arg(&a, 1) {
        V::Undef | V::Null => String::new(),
        v => it.to_string(&v)?,
    };
    let id = new_window(&kind, &title, "");
    Ok(hv(H::Window(id)))
}

pub fn panel_window(name: &str) -> u64 {
    new_window("panel", name, name)
}

fn with_ctl<T>(w: u64, c: u64, f: impl FnOnce(&mut Ctl) -> T) -> Result<T, Flow> {
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        let win = s.wins.get_mut(&w).ok_or_else(|| throw("Error", "this window has been closed"))?;
        let ctl = win.ctls.iter_mut().find(|x| x.id == c).ok_or_else(|| throw("Error", "this control has been removed"))?;
        Ok(f(ctl))
    })
}

fn touch(w: u64) {
    STORE.with(|s| {
        if let Some(win) = s.borrow_mut().wins.get_mut(&w) {
            win.dirty = true;
        }
    });
}

fn ids(h: &H) -> Option<(u64, u64)> {
    match h {
        H::Window(w) => Some((*w, 0)),
        H::Control(w, c) => Some((*w, *c)),
        _ => None,
    }
}

fn ctl_v(w: u64, c: u64) -> V {
    if c == 0 { hv(H::Window(w)) } else { hv(H::Control(w, c)) }
}

fn list_item(text: &str, index: usize) -> V {
    obj(vec![("text", s(text)), ("index", V::Num(index as f64)), ("selected", V::Bool(false)), ("type", s("item"))])
}

fn m(h: &Rc<H>, name: &'static str, f: NativeImpl) -> V {
    native(name, V::Host(h.clone()), f)
}

pub fn get(it: &mut Interp, h: &Rc<H>, key: &str) -> Result<Option<V>, Flow> {
    let _ = it;
    let Some((w, c)) = ids(h) else { return Ok(None) };
    if c == 0 {
        let win = STORE.with(|s| s.borrow().wins.get(&w).map(|x| (x.kind.clone(), x.title.clone(), x.shown)));
        let Some((kind, title, shown)) = win else { return Err(throw("Error", "this window has been closed")) };
        match key {
            "show" => return Ok(Some(m(h, "show", w_show))),
            "close" | "hide" => return Ok(Some(m(h, "close", w_close))),
            "center" | "update" => return Ok(Some(m(h, "noop", g_noop))),
            "layout" => {
                let o = obj(vec![]);
                if let V::Obj(ob) = &o {
                    ob.borrow_mut().set("layout", native("layout", V::Undef, g_noop));
                    ob.borrow_mut().set("resize", native("resize", V::Undef, g_noop));
                }
                return Ok(Some(o));
            }
            "type" => return Ok(Some(s(&kind))),
            "text" | "title" => return Ok(Some(s(&title))),
            "visible" => return Ok(Some(V::Bool(shown))),
            "findElement" => return Ok(Some(m(h, "findElement", w_find))),
            "frameLocation" | "location" => return Ok(Some(nums(&[0.0, 0.0]))),
            _ => {}
        }
    }
    let ctl = STORE.with(|st| st.borrow().wins.get(&w).and_then(|x| x.ctls.iter().find(|y| y.id == c).cloned()));
    let Some(ctl) = ctl else { return Err(throw("Error", "this control has been removed")) };
    if let Some(hd) = ctl.handlers.get(key) {
        return Ok(Some(hd.clone()));
    }
    Ok(Some(match key {
        "type" => s(&ctl.kind),
        "text" => s(&ctl.text),
        "value" => match ctl.kind.as_str() {
            "checkbox" | "radiobutton" => V::Bool(ctl.value != 0.0),
            _ => V::Num(ctl.value),
        },
        "minvalue" => V::Num(ctl.min),
        "maxvalue" => V::Num(ctl.max),
        "items" => arr(ctl.items.iter().enumerate().map(|(i, t)| list_item(t, i)).collect()),
        "selection" => match ctl.selection {
            Some(i) if i < ctl.items.len() => list_item(&ctl.items[i], i),
            _ => V::Null,
        },
        "enabled" => V::Bool(ctl.enabled),
        "visible" => V::Bool(ctl.visible),
        "name" => s(&ctl.name),
        "parent" => {
            if c == 0 {
                V::Null
            } else {
                ctl_v(w, ctl.parent)
            }
        }
        "window" => hv(H::Window(w)),
        "children" => {
            let kids: Vec<V> = STORE.with(|st| st.borrow().wins.get(&w).map(|x| x.ctls.iter().filter(|y| y.id != 0 && y.parent == c).map(|y| ctl_v(w, y.id)).collect()).unwrap_or_default());
            arr(kids)
        }
        "add" => m(h, "add", c_add),
        "remove" => m(h, "remove", c_remove),
        "removeAll" => m(h, "removeAll", c_remove_all),
        "notify" => m(h, "notify", c_notify),
        "find" => m(h, "find", c_find_item),
        "graphics" => {
            let o = obj(vec![("font", obj(vec![]))]);
            for n in ["newBrush", "newPen", "newFont", "drawString", "rectPath", "fillPath", "strokePath", "newPath", "moveTo", "lineTo", "closePath"] {
                if let V::Obj(ob) = &o {
                    ob.borrow_mut().set(n, native("graphics", V::Undef, g_noop));
                }
            }
            o
        }
        "properties" => obj(vec![("multiline", V::Bool(ctl.multiline))]),
        "active" | "justify" | "indent" | "characters" | "orientation" | "alignChildren" | "alignment" | "helpTip" | "preferredSize" | "size" | "spacing" | "margins" | "bounds" | "minimumSize" | "maximumSize" | "image" => {
            ctl.extra.get(key).cloned().unwrap_or(V::Undef)
        }
        _ => ctl.extra.get(key).cloned().unwrap_or(V::Undef),
    }))
}

fn g_noop(_: &mut Interp, _: &V, _: Vec<V>) -> R {
    Ok(V::Undef)
}

pub fn set(it: &mut Interp, h: &Rc<H>, key: &str, v: V) -> Result<(), Flow> {
    let Some((w, c)) = ids(h) else { return Ok(()) };
    if key.starts_with("on") {
        with_ctl(w, c, |ctl| {
            ctl.handlers.insert(key.to_string(), v);
        })?;
        return Ok(());
    }
    match key {
        "text" | "title" => {
            let t = it.to_string(&v)?;
            if c == 0 {
                STORE.with(|st| {
                    if let Some(win) = st.borrow_mut().wins.get_mut(&w) {
                        win.title = t.clone();
                    }
                });
            }
            with_ctl(w, c, |ctl| ctl.text = t)?;
        }
        "value" => {
            let n = match &v {
                V::Bool(b) => *b as u8 as f64,
                other => it.to_number(other)?,
            };
            with_ctl(w, c, |ctl| {
                ctl.value = if ctl.kind == "slider" || ctl.kind == "progressbar" || ctl.kind == "scrollbar" { n.clamp(ctl.min.min(ctl.max), ctl.max.max(ctl.min)) } else { n };
            })?;
        }
        "minvalue" => {
            let n = it.to_number(&v)?;
            with_ctl(w, c, |ctl| ctl.min = n)?;
        }
        "maxvalue" => {
            let n = it.to_number(&v)?;
            with_ctl(w, c, |ctl| ctl.max = n)?;
        }
        "selection" => {
            let sel = match &v {
                V::Null | V::Undef => None,
                V::Num(n) => Some(*n as usize),
                V::Obj(o) => o.borrow().get("index").and_then(|x| if let V::Num(n) = x { Some(*n as usize) } else { None }),
                other => Some(it.to_number(other)? as usize),
            };
            with_ctl(w, c, |ctl| ctl.selection = sel.filter(|i| *i < ctl.items.len()))?;
        }
        "enabled" => with_ctl(w, c, |ctl| ctl.enabled = truthy(&v))?,
        "visible" => with_ctl(w, c, |ctl| ctl.visible = truthy(&v))?,
        "name" => {
            let t = it.to_string(&v)?;
            with_ctl(w, c, |ctl| ctl.name = t)?;
        }
        _ => with_ctl(w, c, |ctl| {
            ctl.extra.insert(key.to_string(), v);
        })?,
    }
    touch(w);
    Ok(())
}

fn this_ids(this: &V) -> Result<(u64, u64), Flow> {
    match this {
        V::Host(h) => ids(h).ok_or_else(|| type_err("this method belongs to a ScriptUI control")),
        _ => Err(type_err("this method belongs to a ScriptUI control")),
    }
}

fn w_show(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    let (w, _) = this_ids(this)?;
    STORE.with(|s| {
        if let Some(win) = s.borrow_mut().wins.get_mut(&w) {
            win.shown = true;
            win.dirty = true;
        }
    });
    Ok(V::Num(1.0))
}

fn w_close(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let (w, _) = this_ids(this)?;
    let handler = STORE.with(|s| {
        let mut s = s.borrow_mut();
        let win = s.wins.get_mut(&w)?;
        if win.closed {
            return None;
        }
        win.closed = true;
        win.dirty = true;
        win.ctls.first().and_then(|r| r.handlers.get("onClose").cloned())
    });
    if let Some(f) = handler {
        it.call(&f, this.clone(), vec![])?;
    }
    Ok(V::Undef)
}

fn w_find(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (w, _) = this_ids(this)?;
    let name = it.to_string(&arg(&a, 0))?;
    let found = STORE.with(|s| s.borrow().wins.get(&w).and_then(|x| x.ctls.iter().find(|y| y.name == name).map(|y| y.id)));
    Ok(found.map(|c| ctl_v(w, c)).unwrap_or(V::Null))
}

fn c_add(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (w, c) = this_ids(this)?;
    let kind = it.to_string(&arg(&a, 0))?.to_ascii_lowercase();
    let parent_kind = with_ctl(w, c, |ctl| ctl.kind.clone())?;
    if kind == "item" || (LISTS.contains(&parent_kind.as_str()) && !CONTAINERS.contains(&kind.as_str())) {
        let text = match arg(&a, 1) {
            V::Undef | V::Null => String::new(),
            v => it.to_string(&v)?,
        };
        let idx = with_ctl(w, c, |ctl| {
            ctl.items.push(text.clone());
            ctl.items.len() - 1
        })?;
        touch(w);
        return Ok(list_item(&text, idx));
    }
    if !CONTAINERS.contains(&parent_kind.as_str()) {
        return Err(type_err(format!("a {parent_kind} cannot hold other controls")));
    }
    let kind = if KINDS.contains(&kind.as_str()) { kind } else { return Err(type_err(format!("`{kind}` is not a ScriptUI control type"))) };
    let mut ctl = Ctl::new(0, c, &kind);
    for x in a.iter().skip(1) {
        match x {
            V::Str(t) => ctl.text = t.to_string(),
            V::Arr(items) if LISTS.contains(&kind.as_str()) => {
                let list = items.borrow().clone();
                for i in &list {
                    ctl.items.push(it.to_string(i)?);
                }
            }
            V::Obj(o) => {
                let props = o.borrow().props.clone();
                for (k, v) in props {
                    match &*k {
                        "name" => ctl.name = it.to_string(&v)?,
                        "multiline" => ctl.multiline = truthy(&v),
                        "items" => {
                            for i in it.iterate(&v)? {
                                ctl.items.push(it.to_string(&i)?);
                            }
                        }
                        _ => {
                            ctl.extra.insert(k.to_string(), v.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if kind == "slider" || kind == "scrollbar" || kind == "progressbar" {
        let nums_after: Vec<f64> = a.iter().skip(2).filter_map(|x| if let V::Num(n) = x { Some(*n) } else { None }).collect();
        if let Some(v) = nums_after.first() {
            ctl.value = *v;
        }
        if let Some(v) = nums_after.get(1) {
            ctl.min = *v;
        }
        if let Some(v) = nums_after.get(2) {
            ctl.max = *v;
        }
    }
    if matches!(kind.as_str(), "group" | "panel" | "tab") {
        ctl.extra.entry("orientation".into()).or_insert(V::Str(Rc::from(if kind == "group" { "row" } else { "column" })));
    }
    let id = STORE.with(|s| {
        let mut s = s.borrow_mut();
        let win = s.wins.get_mut(&w)?;
        let id = win.next;
        win.next += 1;
        ctl.id = id;
        win.ctls.push(ctl);
        win.dirty = true;
        Some(id)
    });
    let id = id.ok_or_else(|| throw("Error", "this window has been closed"))?;
    Ok(hv(H::Control(w, id)))
}

fn c_remove(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (w, c) = this_ids(this)?;
    let target = arg(&a, 0);
    let list = with_ctl(w, c, |ctl| LISTS.contains(&ctl.kind.as_str()))?;
    if list {
        let idx = match &target {
            V::Num(n) => Some(*n as usize),
            V::Obj(o) => o.borrow().get("index").and_then(|x| if let V::Num(n) = x { Some(*n as usize) } else { None }),
            V::Str(t) => with_ctl(w, c, |ctl| ctl.items.iter().position(|x| x == &**t))?,
            _ => None,
        };
        if let Some(i) = idx {
            with_ctl(w, c, |ctl| {
                if i < ctl.items.len() {
                    ctl.items.remove(i);
                    if ctl.selection == Some(i) {
                        ctl.selection = None;
                    }
                }
            })?;
        }
        touch(w);
        return Ok(V::Undef);
    }
    let (tw, tc) = match &target {
        V::Host(h) => ids(h).ok_or_else(|| type_err("remove() takes a control"))?,
        V::Num(n) => {
            let kids: Vec<u64> = STORE.with(|s| s.borrow().wins.get(&w).map(|x| x.ctls.iter().filter(|y| y.id != 0 && y.parent == c).map(|y| y.id).collect()).unwrap_or_default());
            (w, kids.get(*n as usize).copied().ok_or_else(|| throw("RangeError", "no child at that index"))?)
        }
        _ => {
            let _ = it;
            return Err(type_err("remove() takes a control"));
        }
    };
    STORE.with(|s| {
        if let Some(win) = s.borrow_mut().wins.get_mut(&tw) {
            let mut gone = vec![tc];
            let mut changed = true;
            while changed {
                changed = false;
                for x in &win.ctls {
                    if !gone.contains(&x.id) && gone.contains(&x.parent) && x.id != 0 {
                        gone.push(x.id);
                        changed = true;
                    }
                }
            }
            win.ctls.retain(|x| !gone.contains(&x.id));
            win.dirty = true;
        }
    });
    Ok(V::Undef)
}

fn c_remove_all(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    let (w, c) = this_ids(this)?;
    with_ctl(w, c, |ctl| {
        ctl.items.clear();
        ctl.selection = None;
    })?;
    touch(w);
    Ok(V::Undef)
}

fn c_find_item(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (w, c) = this_ids(this)?;
    let t = it.to_string(&arg(&a, 0))?;
    let found = with_ctl(w, c, |ctl| ctl.items.iter().position(|x| *x == t))?;
    Ok(found.map(|i| list_item(&t, i)).unwrap_or(V::Null))
}

fn c_notify(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let (w, c) = this_ids(this)?;
    let ev = match arg(&a, 0) {
        V::Undef => {
            let kind = with_ctl(w, c, |ctl| ctl.kind.clone())?;
            if matches!(kind.as_str(), "button" | "iconbutton" | "checkbox" | "radiobutton") { "onClick".to_string() } else { "onChange".to_string() }
        }
        v => it.to_string(&v)?,
    };
    let h = with_ctl(w, c, |ctl| ctl.handlers.get(&ev).cloned())?;
    if let Some(f) = h {
        it.call(&f, this.clone(), vec![])?;
    }
    Ok(V::Undef)
}

fn tree(w: u64, win: &Win) -> Json {
    let ctls: Vec<Json> = win
        .ctls
        .iter()
        .filter(|c| c.id != 0)
        .map(|c| {
            let ex = |k: &str| c.extra.get(k).map(plain_string);
            json!({
                "id": c.id,
                "parent": c.parent,
                "type": c.kind,
                "text": c.text,
                "value": c.value,
                "min": c.min,
                "max": c.max,
                "items": c.items,
                "selection": c.selection,
                "enabled": c.enabled,
                "visible": c.visible,
                "name": c.name,
                "multiline": c.multiline,
                "orientation": ex("orientation"),
                "helpTip": ex("helpTip"),
                "events": c.handlers.keys().collect::<Vec<_>>(),
            })
        })
        .collect();
    let root = win.ctls.first();
    json!({
        "window": w,
        "kind": win.kind,
        "title": win.title,
        "panel": win.panel,
        "orientation": root.and_then(|r| r.extra.get("orientation")).map(plain_string).unwrap_or_else(|| "column".into()),
        "controls": ctls,
    })
}

pub fn flush(e: &mut Engine) {
    let mut out = vec![];
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        let mut gone = vec![];
        for (id, win) in s.wins.iter_mut() {
            if win.closed {
                if win.shown {
                    out.push(("scriptui.close", json!({"window": id})));
                }
                gone.push(*id);
            } else if win.dirty && win.shown {
                out.push(("scriptui.show", tree(*id, win)));
                win.dirty = false;
            }
        }
        for id in gone {
            s.wins.remove(&id);
        }
    });
    for (cmd, p) in out {
        e.frontend(cmd, p);
    }
}

pub fn list() -> Json {
    STORE.with(|s| {
        let s = s.borrow();
        Json::Array(s.wins.iter().filter(|(_, w)| !w.closed && w.shown).map(|(id, w)| tree(*id, w)).collect())
    })
}

pub fn handler(w: u64, c: u64, event: &str) -> Option<V> {
    STORE.with(|s| s.borrow().wins.get(&w).and_then(|x| x.ctls.iter().find(|y| y.id == c)).and_then(|y| y.handlers.get(event).cloned()))
}

pub fn update(w: u64, c: u64, p: &Json) -> Result<(), String> {
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        let win = s.wins.get_mut(&w).ok_or("that ScriptUI window is closed")?;
        let ctl = win.ctls.iter_mut().find(|x| x.id == c).ok_or("that ScriptUI control is gone")?;
        if let Some(t) = p.get("text").and_then(Json::as_str) {
            ctl.text = t.to_string();
        }
        if let Some(v) = p.get("value") {
            if let Some(n) = v.as_f64() {
                ctl.value = n;
            } else if let Some(b) = v.as_bool() {
                ctl.value = b as u8 as f64;
            }
        }
        match p.get("selection") {
            Some(Json::Null) => ctl.selection = None,
            Some(v) => {
                if let Some(i) = v.as_u64() {
                    ctl.selection = Some(i as usize).filter(|i| *i < ctl.items.len());
                }
            }
            None => {}
        }
        if ctl.kind == "radiobutton" && ctl.value != 0.0 {
            let (parent, me) = (ctl.parent, ctl.id);
            for o in win.ctls.iter_mut() {
                if o.kind == "radiobutton" && o.parent == parent && o.id != me {
                    o.value = 0.0;
                }
            }
            win.dirty = true;
        }
        Ok(())
    })
}

pub fn control(w: u64, c: u64) -> V {
    ctl_v(w, c)
}

pub fn close(w: u64) -> Option<V> {
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        let win = s.wins.get_mut(&w)?;
        win.closed = true;
        win.ctls.first().and_then(|r| r.handlers.get("onClose").cloned())
    })
}

pub fn show(w: u64) {
    STORE.with(|s| {
        if let Some(win) = s.borrow_mut().wins.get_mut(&w) {
            win.shown = true;
            win.dirty = true;
        }
    });
}
