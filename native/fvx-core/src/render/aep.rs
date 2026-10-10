use std::collections::HashMap;

use super::rifx::{self, be16, be32, bef32, bef64, bei32, cstr, f64s, pascal_utf8, Chunk, Cos};

#[derive(Clone, Debug, Default)]
pub struct ParDef {
    pub mn: String,
    pub name: String,
    pub kind: u8,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Key {
    pub time: f64,
    pub in_interp: u8,
    pub out_interp: u8,
    pub flags: u8,
    pub value: Vec<f64>,
    pub in_ease: Vec<(f64, f64)>,
    pub out_ease: Vec<(f64, f64)>,
    pub tan_in: Vec<f64>,
    pub tan_out: Vec<f64>,
    pub text: Option<TextDoc>,
}

#[derive(Clone, Debug, Default)]
pub struct Prop {
    pub mn: String,
    pub name: Option<String>,
    pub hidden: bool,
    pub dims: usize,
    pub spatial: bool,
    pub value: Vec<f64>,
    pub keys: Vec<Key>,
    pub expr: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub layer_ref: Option<u32>,
    pub text: Option<TextDoc>,
}

#[derive(Clone, Debug, Default)]
pub struct Group {
    pub mn: String,
    pub name: Option<String>,
    pub hidden: bool,
    pub kids: Vec<Node>,
    pub defs: Vec<ParDef>,
    pub effect_name: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Node {
    Group(Group),
    Prop(Prop),
}

impl Node {
    pub fn mn(&self) -> &str {
        match self {
            Node::Group(g) => &g.mn,
            Node::Prop(p) => &p.mn,
        }
    }
}

impl Group {
    pub fn get(&self, mn: &str) -> Option<&Node> {
        self.kids.iter().find(|k| k.mn() == mn)
    }

    pub fn group(&self, mn: &str) -> Option<&Group> {
        match self.get(mn) {
            Some(Node::Group(g)) => Some(g),
            _ => None,
        }
    }

    pub fn prop(&self, mn: &str) -> Option<&Prop> {
        match self.get(mn) {
            Some(Node::Prop(p)) => Some(p),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Run {
    pub len: usize,
    pub font: String,
    pub size: f64,
    pub faux_bold: bool,
    pub faux_italic: bool,
    pub auto_leading: bool,
    pub leading: f64,
    pub h_scale: f64,
    pub v_scale: f64,
    pub tracking: f64,
    pub baseline_shift: f64,
    pub caps: u8,
    pub fill: Option<[f64; 3]>,
    pub stroke: Option<[f64; 3]>,
    pub apply_fill: bool,
    pub apply_stroke: bool,
    pub fill_first: bool,
    pub stroke_width: f64,
}

#[derive(Clone, Debug, Default)]
pub struct TextDoc {
    pub text: String,
    pub justify: u8,
    pub runs: Vec<Run>,
    pub box_size: Option<[f64; 2]>,
}

#[derive(Clone, Debug, Default)]
pub struct Layer {
    pub id: u32,
    pub name: String,
    pub kind: u32,
    pub source: Option<u32>,
    pub start: f64,
    pub in_point: f64,
    pub out_point: f64,
    pub stretch: f64,
    pub parent: Option<u32>,
    pub label: u8,
    pub quality: u16,
    pub three_d: bool,
    pub null: bool,
    pub adjustment: bool,
    pub video: bool,
    pub audio: bool,
    pub effects: bool,
    pub motion_blur: bool,
    pub collapse: bool,
    pub locked: bool,
    pub shy: bool,
    pub solo: bool,
    pub props: Group,
}

#[derive(Clone, Debug, Default)]
pub struct Control {
    pub id: String,
    pub name: String,
    pub kind: u32,
    pub layer: u32,
    pub path: Vec<(String, i64)>,
}

#[derive(Clone, Debug, Default)]
pub struct Comp {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub pixel_aspect: f64,
    pub duration: f64,
    pub work: [f64; 2],
    pub background: [f64; 3],
    pub motion_blur: bool,
    pub poster: Option<f64>,
    pub layers: Vec<Layer>,
    pub controls: Vec<Control>,
    pub template_name: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Kind {
    Folder,
    Comp(Box<Comp>),
    Solid { width: u32, height: u32, color: [f64; 3] },
    Footage { path: String, width: u32, height: u32 },
    Other,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: u32,
    pub name: String,
    pub parent: Option<u32>,
    pub kind: Kind,
}

#[derive(Clone, Debug, Default)]
pub struct Project {
    pub items: Vec<Item>,
    pub defs: HashMap<String, (String, Vec<ParDef>)>,
    pub warnings: Vec<String>,
}

impl Project {
    pub fn item(&self, id: u32) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }
}

fn mn_of(c: &Chunk) -> String {
    cstr(&c.data)
}

fn user_name(c: &Chunk) -> Option<String> {
    let n = c.kid("tdsn").map(|k| pascal_utf8(&k.data))?;
    if n.is_empty() || n == "-_0_/-" { None } else { Some(n) }
}

fn hidden(c: &Chunk) -> bool {
    c.kid("tdsb").is_some_and(|k| be32(&k.data, 0) & 0x02 != 0)
}

pub fn read(bytes: &[u8]) -> Result<Project, String> {
    let (form, top) = rifx::parse(bytes)?;
    if form != "Egg!" {
        return Err(format!("not an After Effects project (RIFX form `{form}`)"));
    }
    let mut p = Project::default();
    for c in &top {
        if c.is_list("EfdG") {
            for df in c.subs("EfDf") {
                if let (Some(m), Some(sp)) = (df.kid("tdmn"), df.sub("sspc")) {
                    let display = sp.kid("fnam").map(|f| pascal_utf8(&f.data)).unwrap_or_default();
                    let defs = sp.sub("parT").map(par_defs).unwrap_or_default();
                    p.defs.insert(mn_of(m), (display, defs));
                }
            }
        }
    }
    for c in &top {
        if c.is_list("Fold") {
            folder(c, None, &mut p)?;
        }
    }
    Ok(p)
}

fn par_defs(part: &Chunk) -> Vec<ParDef> {
    let mut out = vec![];
    let mut mn = String::new();
    for k in &part.kids {
        if k.is("tdmn") {
            mn = mn_of(k);
        } else if k.is("pard") {
            let d = &k.data;
            out.push(ParDef { mn: mn.clone(), name: cstr(d.get(16..48).unwrap_or(&[])), kind: d.get(15).copied().unwrap_or(0), options: vec![] });
        } else if k.is("pdnm")
            && let Some(last) = out.last_mut()
        {
            let s = pascal_utf8(&k.data);
            if last.kind == 7 {
                last.options = s.split('|').map(str::to_string).collect();
            } else if last.name.is_empty() {
                last.name = s;
            }
        }
    }
    out
}

fn folder(c: &Chunk, parent: Option<u32>, p: &mut Project) -> Result<(), String> {
    for it in c.subs("Item") {
        let Some(idta) = it.kid("idta") else { continue };
        let ty = be16(&idta.data, 0);
        let id = be32(&idta.data, 16);
        let name = it.utf8().unwrap_or_default();
        match ty {
            1 => {
                p.items.push(Item { id, name, parent, kind: Kind::Folder });
                if let Some(sub) = it.sub("Sfdr") {
                    folder(sub, Some(id), p)?;
                }
            }
            4 => {
                let comp = comp(it, p)?;
                p.items.push(Item { id, name, parent, kind: Kind::Comp(Box::new(comp)) });
            }
            7 => {
                let kind = footage(it);
                let name = if name.is_empty() {
                    match &kind {
                        Kind::Solid { .. } => it.sub("Pin ").and_then(|pin| pin.kid("opti")).map(|o| cstr(o.data.get(26..).unwrap_or(&[]))).unwrap_or_default(),
                        Kind::Footage { path, .. } => std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("").to_string(),
                        _ => String::new(),
                    }
                } else {
                    name
                };
                p.items.push(Item { id, name, parent, kind });
            }
            _ => p.items.push(Item { id, name, parent, kind: Kind::Other }),
        }
    }
    Ok(())
}

fn footage(it: &Chunk) -> Kind {
    let Some(pin) = it.sub("Pin ") else { return Kind::Other };
    let sspc = pin.kid("sspc").map(|k| k.data.as_slice()).unwrap_or(&[]);
    let (w, h) = (be32(sspc, 30), be32(sspc, 34));
    if let Some(opti) = pin.kid("opti")
        && opti.data.starts_with(b"Soli")
    {
        let f = |o| bef32(&opti.data, o) as f64;
        return Kind::Solid { width: w.max(1), height: h.max(1), color: [f(14), f(18), f(22)] };
    }
    let mut path = String::new();
    let mut lists = vec![];
    pin.find_all("Als2", &mut lists);
    for a in lists {
        if let Some(alas) = a.kid("alas")
            && let Ok(j) = serde_json::from_slice::<serde_json::Value>(&alas.data)
            && let Some(fp) = j.get("fullpath").and_then(serde_json::Value::as_str)
        {
            path = fp.to_string();
        }
    }
    if path.is_empty() {
        return Kind::Other;
    }
    Kind::Footage { path, width: w, height: h }
}

fn rational(d: &[u8], at: usize) -> f64 {
    let s = be32(d, at + 4);
    if s == 0 { 0.0 } else { bei32(d, at) as f64 / s as f64 }
}

fn comp(it: &Chunk, p: &mut Project) -> Result<Comp, String> {
    let d = &it.kid("cdta").ok_or("a composition has no settings chunk")?.data;
    let scale = be32(d, 8).max(1) as f64;
    let fps = be32(d, 0x9c) as f64 / 65536.0;
    let mut c = Comp {
        width: be16(d, 0x8c) as u32,
        height: be16(d, 0x8e) as u32,
        fps: if fps > 0.0 { fps } else { 30.0 },
        pixel_aspect: {
            let (n, m) = (be32(d, 0x90), be32(d, 0x94));
            if n > 0 && m > 0 { n as f64 / m as f64 } else { 1.0 }
        },
        duration: rational(d, 0x2c),
        work: [rational(d, 0x1c), rational(d, 0x1c) + rational(d, 0x24)],
        background: [d.get(0x34).copied().unwrap_or(0) as f64 / 255.0, d.get(0x35).copied().unwrap_or(0) as f64 / 255.0, d.get(0x36).copied().unwrap_or(0) as f64 / 255.0],
        motion_blur: d.get(0x37).is_some_and(|b| b & 0x08 != 0),
        poster: it.kid("CPTm").map(|k| rational(&k.data, 0)),
        ..Comp::default()
    };
    if c.width == 0 || c.height == 0 {
        return Err("a composition has no size".into());
    }
    if c.duration <= 0.0 {
        c.duration = 10.0;
    }
    if c.work[1] <= c.work[0] || c.work[1] > c.duration + 1e-6 {
        c.work = [0.0, c.duration];
    }
    for l in it.subs("Layr") {
        c.layers.push(layer(l, scale, p));
    }
    if let Some(cifo) = it.sub("CIFO") {
        if let Some(caps) = cifo.sub("CapS") {
            c.template_name = caps.utf8();
        }
        for ctl in cifo.subs("CCtl") {
            let name = ctl.sub("CapS").and_then(|k| k.utf8()).unwrap_or_default();
            let id = ctl.utf8().unwrap_or_default();
            let kind = ctl.kid("CTyp").map(|k| be32(&k.data, 0)).unwrap_or(0);
            let Some(cp) = ctl.sub("CPrp") else {
                c.controls.push(Control { id, name, kind, layer: 0, path: vec![] });
                continue;
            };
            let layer = cp.kid("CLId").map(|k| be32(&k.data, 0)).unwrap_or(0);
            let mut path = vec![];
            if let Some(js) = cp.utf8()
                && let Ok(serde_json::Value::Object(m)) = serde_json::from_str::<serde_json::Value>(&js)
            {
                let mut steps: Vec<(usize, String, i64)> = m
                    .iter()
                    .filter_map(|(k, v)| {
                        let i = k.parse::<usize>().ok()?;
                        let mn = v.get("matchName")?.as_str()?.to_string();
                        let idx = v.get("index").and_then(serde_json::Value::as_i64).unwrap_or(-1);
                        Some((i, mn, if idx == 4294967295 { -1 } else { idx }))
                    })
                    .collect();
                steps.sort_by_key(|s| s.0);
                path = steps.into_iter().map(|(_, mn, i)| (mn, i)).collect();
            }
            c.controls.push(Control { id, name, kind, layer, path });
        }
    }
    Ok(c)
}

fn layer(l: &Chunk, scale_hint: f64, p: &mut Project) -> Layer {
    let d = l.kid("ldta").map(|k| k.data.clone()).unwrap_or_default();
    let scale = be32(&d, 0x10).max(1) as f64;
    let scale = if scale > 1.0 { scale } else { scale_hint };
    let b = |i: usize| d.get(i).copied().unwrap_or(0);
    let source = be32(&d, 0x28);
    let parent = be32(&d, 0x84);
    let num = be32(&d, 0x08);
    let den = be32(&d, 0x6c);
    let mut ly = Layer {
        id: be32(&d, 0),
        name: l.utf8().unwrap_or_default(),
        kind: be32(&d, 0x80),
        source: (source != 0).then_some(source),
        start: rational(&d, 0x0c),
        in_point: rational(&d, 0x14),
        out_point: rational(&d, 0x1c),
        stretch: if num > 0 && den > 0 { num as f64 / den as f64 * 100.0 } else { 100.0 },
        parent: (parent != 0).then_some(parent),
        label: b(0x3d),
        quality: be16(&d, 4),
        null: b(0x26) & 0x80 != 0,
        three_d: b(0x26) & 0x04 != 0,
        adjustment: b(0x26) & 0x02 != 0,
        video: b(0x27) & 0x01 != 0,
        audio: b(0x27) & 0x02 != 0,
        effects: b(0x27) & 0x04 != 0,
        motion_blur: b(0x27) & 0x08 != 0,
        locked: b(0x27) & 0x20 != 0,
        shy: b(0x27) & 0x40 != 0,
        collapse: b(0x27) & 0x80 != 0,
        solo: b(0x26) & 0x08 != 0,
        props: Group::default(),
    };
    if let Some(root) = l.sub("tdgp") {
        ly.props = group(root, String::new(), scale, p);
    }
    ly
}

fn group(g: &Chunk, mn: String, scale: f64, p: &mut Project) -> Group {
    let mut out = Group { mn, name: user_name(g), hidden: hidden(g), ..Group::default() };
    let mut pending: Option<String> = None;
    for k in &g.kids {
        if k.is("tdmn") {
            let m = mn_of(k);
            if m == "ADBE Group End" {
                pending = None;
                continue;
            }
            pending = Some(m);
            continue;
        }
        let Some(m) = pending.clone() else { continue };
        match k.list.as_deref() {
            Some("tdgp") => {
                out.kids.push(Node::Group(group(k, m, scale, p)));
                pending = None;
            }
            Some("tdbs") => {
                out.kids.push(Node::Prop(prop(k, m, scale, p)));
                pending = None;
            }
            Some("sspc") => {
                out.kids.push(Node::Group(effect(k, m, scale, p)));
                pending = None;
            }
            Some("btds") => {
                if let Some(tb) = k.sub("tdbs") {
                    let mut pr = prop(tb, m, scale, p);
                    let mut docs = vec![];
                    k.find_all("btdk", &mut docs);
                    let parsed: Vec<TextDoc> = docs.iter().filter_map(|c| match rifx::cos(&c.data) {
                        Ok(cos) => text_doc(&cos),
                        Err(e) => {
                            p.warnings.push(format!("a text document could not be read: {e}"));
                            None
                        }
                    }).collect();
                    if pr.keys.is_empty() {
                        pr.text = parsed.into_iter().next();
                    } else {
                        for (key, doc) in pr.keys.iter_mut().zip(parsed) {
                            key.text = Some(doc);
                        }
                        pr.text = pr.keys.first().and_then(|k| k.text.clone());
                    }
                    out.kids.push(Node::Prop(pr));
                }
                pending = None;
            }
            Some("otst") => {
                if let Some(tb) = k.sub("tdbs") {
                    let mut pr = prop(tb, m, scale, p);
                    if let Some(ky) = k.sub("otky") {
                        let vals: Vec<Vec<f64>> = ky.kids.iter().filter(|c| c.is("otda")).map(|c| f64s(&c.data)).collect();
                        if pr.keys.is_empty() {
                            if let Some(v) = vals.first() {
                                pr.value = v.clone();
                            }
                        } else {
                            for (key, v) in pr.keys.iter_mut().zip(vals) {
                                key.value = v;
                            }
                        }
                    }
                    pr.dims = 3;
                    out.kids.push(Node::Prop(pr));
                }
                pending = None;
            }
            Some("om-s") => {
                if let Some(tb) = k.sub("tdbs") {
                    let pr = prop(tb, m, scale, p);
                    p.warnings.push(format!("shape path `{}` was read without its vertices", pr.mn));
                    out.kids.push(Node::Prop(pr));
                }
                pending = None;
            }
            Some(other) => {
                if !matches!(other, "list") && !(other == "OvG2" && k.kids.iter().all(|c| c.list.is_none())) {
                    p.warnings.push(format!("`{m}` ({other}) is not read yet"));
                }
                pending = None;
            }
            None => {}
        }
    }
    out
}

fn effect(sp: &Chunk, mn: String, scale: f64, p: &mut Project) -> Group {
    let mut g = sp.sub("tdgp").map(|v| group(v, mn.clone(), scale, p)).unwrap_or(Group { mn: mn.clone(), ..Group::default() });
    g.defs = sp.sub("parT").map(par_defs).filter(|d| !d.is_empty()).or_else(|| p.defs.get(&mn).map(|d| d.1.clone())).unwrap_or_default();
    g.effect_name = sp.kid("fnam").map(|f| pascal_utf8(&f.data)).or_else(|| p.defs.get(&mn).map(|d| d.0.clone()));
    g
}

fn prop(t: &Chunk, mn: String, scale: f64, p: &mut Project) -> Prop {
    let tdb4 = t.kid("tdb4").map(|k| k.data.as_slice()).unwrap_or(&[]);
    let scale = if scale > 1.0 { scale } else { (be32(tdb4, 12) as f64).max(1.0) };
    let dims = (be16(tdb4, 2) as usize).max(1);
    let spatial = dims >= 2 && tdb4.get(7).is_some_and(|b| b & 0x02 != 0);
    let cdat = t.kid("cdat").map(|k| f64s(&k.data)).unwrap_or_default();
    let mut pr = Prop {
        mn,
        name: user_name(t),
        hidden: hidden(t),
        dims,
        spatial,
        value: cdat.into_iter().take(dims).collect(),
        expr: t.kid("Utf8").map(|k| String::from_utf8_lossy(&k.data).to_string()).filter(|s| !s.trim().is_empty()),
        min: t.kid("tdum").map(|k| bef64(&k.data, 0)),
        max: t.kid("tduM").map(|k| bef64(&k.data, 0)),
        layer_ref: t.kid("tdpi").map(|k| be32(&k.data, 0)),
        ..Prop::default()
    };
    if let Some(l) = t.sub("list") {
        pr.keys = keys(l, dims, spatial, scale, &pr.mn, p);
    }
    pr
}

fn keys(l: &Chunk, dims: usize, spatial: bool, scale: f64, mn: &str, p: &mut Project) -> Vec<Key> {
    let Some(h) = l.kid("lhd3") else { return vec![] };
    let count = be32(&h.data, 8) as usize;
    let size = be32(&h.data, 16) as usize;
    let Some(dat) = l.kid("ldat") else { return vec![] };
    if size < 8 || dat.data.len() < count * size {
        p.warnings.push(format!("the keyframes of `{mn}` were not read"));
        return vec![];
    }
    let n = (size - 8) / 8;
    let mut out = vec![];
    for i in 0..count {
        let r = &dat.data[i * size..(i + 1) * size];
        let v: Vec<f64> = (0..n).map(|j| bef64(r, 8 + j * 8)).collect();
        let mut k = Key { time: bei32(r, 0) as f64 / scale, in_interp: r[4], out_interp: r[5], flags: r[7], ..Key::default() };
        if spatial && n == 6 + 3 * dims {
            k.in_ease = vec![(v[2], v[3])];
            k.out_ease = vec![(v[4], v[5])];
            k.value = v[6..6 + dims].to_vec();
            k.tan_in = v[6 + dims..6 + 2 * dims].to_vec();
            k.tan_out = v[6 + 2 * dims..6 + 3 * dims].to_vec();
        } else if n == 5 * dims {
            k.value = v[0..dims].to_vec();
            k.in_ease = (0..dims).map(|d| (v[dims + d], v[2 * dims + d])).collect();
            k.out_ease = (0..dims).map(|d| (v[3 * dims + d], v[4 * dims + d])).collect();
        } else if dims == 4 && n >= 9 {
            let o = n - 4;
            k.in_ease = vec![(v[o - 4], v[o - 3])];
            k.out_ease = vec![(v[o - 2], v[o - 1])];
            k.value = v[o..].to_vec();
        } else if n >= 5 {
            k.in_ease = vec![(v[1], v[2])];
            k.out_ease = vec![(v[3], v[4])];
        } else {
            p.warnings.push(format!("the keyframes of `{mn}` have an unknown layout"));
            return vec![];
        }
        out.push(k);
    }
    out
}

fn paint(c: Option<&Cos>) -> Option<[f64; 3]> {
    let v = c?.at(&["0", "1"])?.nums();
    if v.len() >= 4 { Some([v[1], v[2], v[3]]) } else { None }
}

fn run_of(st: &Cos, fonts: &[String]) -> Run {
    let n = |k: &str, d: f64| st.get(k).and_then(Cos::num).unwrap_or(d);
    let b = |k: &str, d: bool| st.get(k).and_then(Cos::bool).unwrap_or(d);
    Run {
        len: 0,
        font: fonts.get(n("0", 0.0) as usize).cloned().unwrap_or_default(),
        size: n("1", 12.0),
        faux_bold: b("2", false),
        faux_italic: b("3", false),
        auto_leading: b("4", true),
        leading: n("5", 0.0),
        h_scale: n("6", 1.0),
        v_scale: n("7", 1.0),
        tracking: n("8", 0.0),
        baseline_shift: n("10", 0.0),
        caps: n("12", 0.0) as u8,
        fill: paint(st.get("53")),
        stroke: paint(st.get("54")),
        apply_fill: b("56", true),
        apply_stroke: b("57", false),
        fill_first: b("58", false),
        stroke_width: n("63", 1.0),
    }
}

pub fn text_doc(c: &Cos) -> Option<TextDoc> {
    let fonts: Vec<String> = c.at(&["0", "1", "0"]).map(|a| a.arr().iter().filter_map(|f| f.at(&["0", "0", "0"]).and_then(Cos::str).map(str::to_string)).collect()).unwrap_or_default();
    let body = c.at(&["1", "1", "0", "0"]).or_else(|| c.at(&["1", "0", "0"]))?;
    let text = body.get("0").and_then(Cos::str).unwrap_or("").to_string();
    let mut doc = TextDoc { text: text.trim_end_matches('\r').to_string(), ..TextDoc::default() };
    if let Some(p) = body.at(&["5", "0", "0", "0", "0", "5"]) {
        doc.justify = p.get("0").and_then(Cos::num).unwrap_or(0.0) as u8;
    }
    for r in body.at(&["6", "0"]).map(Cos::arr).unwrap_or(&[]) {
        let Some(st) = r.at(&["0", "0", "6"]) else { continue };
        let mut run = run_of(st, &fonts);
        run.len = r.get("1").and_then(Cos::num).unwrap_or(0.0) as usize;
        doc.runs.push(run);
    }
    if let Some(bx) = c.at(&["1", "1", "2", "0"]).and_then(|f| f.get("6")).map(Cos::nums)
        && bx.len() >= 2
        && bx[0] > 0.0
    {
        doc.box_size = Some([bx[0], bx[1]]);
    }
    Some(doc)
}

pub const PRESET_GROUPS: [&str; 7] = ["ADBE Effect Parade", "ADBE Transform Group", "ADBE Text Properties", "ADBE Root Vectors Group", "ADBE Mask Parade", "ADBE Layer Styles", "ADBE Marker"];

pub fn read_preset(bytes: &[u8]) -> Result<(Vec<Group>, Project), String> {
    let (form, top) = rifx::parse(bytes)?;
    if form != "FaFX" {
        return Err(format!("not an After Effects animation preset (RIFX form `{form}`)"));
    }
    let mut p = Project::default();
    for c in &top {
        if c.is_list("EfdG") {
            for df in c.subs("EfDf") {
                if let (Some(m), Some(sp)) = (df.kid("tdmn"), df.sub("sspc")) {
                    let display = sp.kid("fnam").map(|f| pascal_utf8(&f.data)).unwrap_or_default();
                    p.defs.insert(mn_of(m), (display, sp.sub("parT").map(par_defs).unwrap_or_default()));
                }
            }
        }
    }
    let mut out = vec![];
    let root = Chunk { id: "LIST".into(), list: Some("root".into()), data: vec![], kids: top };
    collect_groups(&root, &mut p, &mut out, 0);
    if out.is_empty() {
        return Err("the preset holds no properties, effects or text animators formvfx can read".into());
    }
    Ok((out, p))
}

fn collect_groups(c: &Chunk, p: &mut Project, out: &mut Vec<Group>, depth: usize) {
    if depth > 32 {
        return;
    }
    if c.kids.iter().any(|k| k.is("tdmn") && PRESET_GROUPS.contains(&mn_of(k).as_str())) {
        let g = group(c, String::new(), 0.0, p);
        for k in g.kids {
            if let Node::Group(gr) = k
                && PRESET_GROUPS.contains(&gr.mn.as_str())
            {
                out.push(gr);
            }
        }
        return;
    }
    for k in &c.kids {
        if k.list.is_some() {
            collect_groups(k, p, out, depth + 1);
        }
    }
}

pub fn effect_param_name(g: &Group, mn: &str) -> Option<String> {
    g.defs.iter().find(|d| d.mn == mn).map(|d| d.name.clone()).filter(|n| !n.is_empty())
}
