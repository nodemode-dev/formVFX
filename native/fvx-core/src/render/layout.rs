use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use rustybuzz::ttf_parser::{GlyphId, Tag};

use super::text::{Sink, db, resolve};
use crate::geom::Mat3;
use crate::model::path::BezPath;
use crate::model::{Justify, ResolvedStyle, TextDoc};

type FaceData = Arc<(Vec<u8>, u32)>;
static DATA: OnceLock<Mutex<HashMap<fontdb::ID, FaceData>>> = OnceLock::new();
type Outlines = Mutex<HashMap<(fontdb::ID, u16, u64), Arc<Vec<BezPath>>>>;
static OUTLINES: OnceLock<Outlines> = OnceLock::new();

pub fn face_data(id: fontdb::ID) -> Option<FaceData> {
    let cache = DATA.get_or_init(Default::default);
    if let Some(d) = cache.lock().get(&id) {
        return Some(d.clone());
    }
    let d = db().with_face_data(id, |data, index| Arc::new((data.to_vec(), index)))?;
    cache.lock().insert(id, d.clone());
    Some(d)
}

pub fn var_key(vars: &[(String, f64)]) -> u64 {
    if vars.is_empty() {
        return 0;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (t, v) in vars {
        t.hash(&mut h);
        v.to_bits().hash(&mut h);
    }
    h.finish() | 1
}

pub fn face<'a>(data: &'a FaceData, vars: &[(String, f64)]) -> Option<rustybuzz::Face<'a>> {
    let mut f = rustybuzz::Face::from_slice(&data.0, data.1)?;
    if !vars.is_empty() {
        let v: Vec<rustybuzz::Variation> = vars.iter().filter_map(|(t, x)| Some(rustybuzz::Variation { tag: Tag::from_bytes_lossy(t.as_bytes()), value: *x as f32 })).collect();
        f.set_variations(&v);
    }
    Some(f)
}

pub fn outline_of(id: fontdb::ID, gid: u16, vars: &[(String, f64)]) -> Arc<Vec<BezPath>> {
    let key = (id, gid, var_key(vars));
    let cache = OUTLINES.get_or_init(Default::default);
    if let Some(v) = cache.lock().get(&key) {
        return v.clone();
    }
    let mut sink = Sink { paths: vec![], cur: None };
    if let Some(data) = face_data(id)
        && let Some(f) = face(&data, vars)
    {
        f.outline_glyph(GlyphId(gid), &mut sink);
    }
    if let Some(c) = sink.cur.take() {
        sink.paths.push(c);
    }
    let v = Arc::new(sink.paths);
    let mut c = cache.lock();
    if c.len() > 200_000 {
        c.clear();
    }
    c.insert(key, v.clone());
    v
}

pub fn font_features(id: fontdb::ID) -> Vec<String> {
    let Some(data) = face_data(id) else { return vec![] };
    let Some(f) = rustybuzz::Face::from_slice(&data.0, data.1) else { return vec![] };
    let mut out: Vec<String> = vec![];
    for table in [f.tables().gsub, f.tables().gpos].into_iter().flatten() {
        for i in 0..table.features.len() {
            if let Some(feat) = table.features.get(i) {
                let t = feat.tag.to_string();
                if !out.contains(&t) {
                    out.push(t);
                }
            }
        }
    }
    out.sort();
    out
}

#[derive(Clone, Debug)]
pub struct Axis {
    pub tag: String,
    pub name: String,
    pub min: f64,
    pub default: f64,
    pub max: f64,
}

pub fn font_axes(id: fontdb::ID) -> Vec<Axis> {
    let Some(data) = face_data(id) else { return vec![] };
    let Some(f) = rustybuzz::Face::from_slice(&data.0, data.1) else { return vec![] };
    f.variation_axes()
        .into_iter()
        .map(|a| {
            let tag = a.tag.to_string();
            let name = f
                .names()
                .into_iter()
                .filter(|n| n.name_id == a.name_id)
                .find_map(|n| n.to_string())
                .unwrap_or_else(|| match tag.as_str() {
                    "wght" => "Weight".into(),
                    "wdth" => "Width".into(),
                    "slnt" => "Slant".into(),
                    "ital" => "Italic".into(),
                    "opsz" => "Optical Size".into(),
                    _ => tag.clone(),
                });
            Axis { tag, name, min: a.min_value as f64, default: a.def_value as f64, max: a.max_value as f64 }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct Glyph {
    pub paths: Vec<BezPath>,
    pub char_index: usize,
    pub line: usize,
    pub x: f64,
    pub baseline: f64,
    pub advance: f64,
    pub ch: char,
    pub size: f64,
    pub font: Option<fontdb::ID>,
    pub gid: u16,
    pub place: Mat3,
    pub vars: Vec<(String, f64)>,
    pub style: Arc<ResolvedStyle>,
}

#[derive(Clone, Debug)]
pub struct CharInfo {
    pub ch: char,
    pub line: usize,
    pub word: Option<usize>,
    pub space: bool,
    pub newline: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Mods {
    pub chars: Option<Vec<char>>,
    pub track_before: Vec<f64>,
    pub track_after: Vec<f64>,
}

pub struct Layout {
    pub glyphs: Vec<Glyph>,
    pub ascent: f64,
    pub descent: f64,
    pub leading: f64,
    pub lines: Vec<(f64, f64, f64)>,
    pub chars: Vec<CharInfo>,
    pub words: usize,
}

struct Eff {
    font: Option<fontdb::ID>,
    size: f64,
    shift: f64,
    ch: char,
    features: Vec<String>,
    style: Arc<ResolvedStyle>,
}

struct Sg {
    gid: u16,
    ci: usize,
    adv: f64,
    dx: f64,
    dy: f64,
    k: f64,
}

fn has_feature(id: Option<fontdb::ID>, tag: &str) -> bool {
    type Feats = Mutex<HashMap<fontdb::ID, Arc<Vec<String>>>>;
    static FEATS: OnceLock<Feats> = OnceLock::new();
    let Some(id) = id else { return false };
    let c = FEATS.get_or_init(Default::default);
    let f = {
        let g = c.lock().get(&id).cloned();
        match g {
            Some(f) => f,
            None => {
                let f = Arc::new(font_features(id));
                c.lock().insert(id, f.clone());
                f
            }
        }
    };
    f.iter().any(|t| t == tag)
}

fn effective(doc: &TextDoc, chars: &[char], styles: &[Arc<ResolvedStyle>]) -> Vec<Eff> {
    let mut font_cache: HashMap<(String, String), Option<fontdb::ID>> = HashMap::new();
    chars
        .iter()
        .zip(styles)
        .map(|(c, st)| {
            let font = *font_cache.entry((st.font.clone(), st.style.clone())).or_insert_with(|| resolve(&st.font, &st.style));
            let mut size = st.size;
            let mut ch = if st.all_caps { c.to_uppercase().next().unwrap_or(*c) } else { *c };
            let mut features = st.features.clone();
            if !doc.ligatures {
                features.push("-liga".into());
                features.push("-clig".into());
            }
            if matches!(doc.kerning.as_str(), "0" | "none" | "off") {
                features.push("-kern".into());
            }
            if st.small_caps && ch.is_lowercase() {
                if has_feature(font, "smcp") {
                    features.push("smcp".into());
                } else {
                    ch = ch.to_uppercase().next().unwrap_or(ch);
                    size *= 0.75;
                }
            }
            let mut shift = st.baseline_shift;
            match st.baseline.as_str() {
                "superscript" => {
                    if has_feature(font, "sups") {
                        features.push("sups".into());
                    } else {
                        shift += st.size * 0.333;
                        size *= 0.583;
                    }
                }
                "subscript" => {
                    if has_feature(font, "subs") {
                        features.push("subs".into());
                    } else {
                        shift -= st.size * 0.333;
                        size *= 0.583;
                    }
                }
                _ => {}
            }
            Eff { font, size, shift, ch, features, style: st.clone() }
        })
        .collect()
}

fn same_seg(a: &Eff, b: &Eff) -> bool {
    a.font == b.font && a.size == b.size && a.features == b.features && a.style.h_scale == b.style.h_scale && a.style.v_scale == b.style.v_scale
}

fn shape_range(eff: &[Eff], a: usize, b: usize, vars: &[(String, f64)]) -> Vec<Sg> {
    let mut out = vec![];
    let mut i = a;
    while i < b {
        let mut j = i + 1;
        while j < b && same_seg(&eff[i], &eff[j]) {
            j += 1;
        }
        let e = &eff[i];
        let seg: String = eff[i..j].iter().map(|x| x.ch).collect();
        let byte_to_char: HashMap<usize, usize> = seg.char_indices().enumerate().map(|(ci, (bi, _))| (bi, ci)).collect();
        let shaped = e.font.and_then(face_data).and_then(|data| {
            let f = face(&data, vars)?;
            let upem = f.units_per_em() as f64;
            let feats: Vec<rustybuzz::Feature> = e.features.iter().filter_map(|t| rustybuzz::Feature::from_str(t).ok()).collect();
            let mut buf = rustybuzz::UnicodeBuffer::new();
            buf.push_str(&seg);
            let res = rustybuzz::shape(&f, &feats, buf);
            let k = e.size / upem.max(1.0);
            Some(
                res.glyph_infos()
                    .iter()
                    .zip(res.glyph_positions())
                    .map(|(gi, p)| Sg { gid: gi.glyph_id as u16, ci: i + byte_to_char.get(&(gi.cluster as usize)).copied().unwrap_or(0), adv: p.x_advance as f64 * k, dx: p.x_offset as f64 * k, dy: p.y_offset as f64 * k, k })
                    .collect::<Vec<_>>(),
            )
        });
        match shaped {
            Some(v) => out.extend(v),
            None => {
                for c in i..j {
                    out.push(Sg { gid: 0, ci: c, adv: eff[c].size * 0.5, dx: 0.0, dy: 0.0, k: 0.0 });
                }
            }
        }
        i = j;
    }
    out
}

fn track_of(eff: &[Eff], mods: &Mods, ci: usize) -> (f64, f64) {
    let base = eff[ci].style.tracking / 1000.0 * eff[ci].style.size;
    (mods.track_before.get(ci).copied().unwrap_or(0.0), base + mods.track_after.get(ci).copied().unwrap_or(0.0))
}

fn ascent_of(e: &Eff, vars: &[(String, f64)]) -> (f64, f64) {
    e.font
        .and_then(face_data)
        .and_then(|d| {
            let f = face(&d, vars)?;
            let k = e.size / (f.units_per_em() as f64).max(1.0) * e.style.v_scale / 100.0;
            Some((f.ascender() as f64 * k, -(f.descender() as f64) * k))
        })
        .unwrap_or((e.size * 0.8, e.size * 0.2))
}

pub fn layout(doc: &TextDoc) -> Layout {
    layout_with(doc, &Mods::default())
}

pub fn doc_vars(doc: &TextDoc) -> Vec<(String, f64)> {
    doc.variations.iter().map(|(k, v)| (k.clone(), *v)).collect()
}

pub fn layout_with(doc: &TextDoc, mods: &Mods) -> Layout {
    let src: Vec<char> = doc.text.chars().collect();
    let n = src.len();
    let styles: Vec<Arc<ResolvedStyle>> = if doc.runs.is_empty() {
        let b = Arc::new(doc.base_style());
        vec![b; n]
    } else {
        (0..n).map(|i| Arc::new(doc.style_at(i))).collect()
    };
    let chars: Vec<char> = match &mods.chars {
        Some(c) if c.len() == n => c.clone(),
        _ => src.clone(),
    };
    let vars = doc_vars(doc);
    let eff = effective(doc, &chars, &styles);
    let mut paras: Vec<(usize, usize)> = vec![];
    let mut s = 0usize;
    let mut i = 0usize;
    while i < n {
        if chars[i] == '\r' || chars[i] == '\n' {
            paras.push((s, i));
            if chars[i] == '\r' && i + 1 < n && chars[i + 1] == '\n' {
                i += 1;
            }
            s = i + 1;
        }
        i += 1;
    }
    paras.push((s, n));
    let base_size = doc.size;
    let line_width = |a: usize, b: usize| -> f64 {
        let sg = shape_range(&eff, a, b, &vars);
        let mut w = 0.0;
        let mut last_after = 0.0;
        let mut seen = usize::MAX;
        for g in &sg {
            let hs = eff[g.ci].style.h_scale / 100.0;
            let (tb, ta) = track_of(&eff, mods, g.ci);
            if g.ci != seen {
                w += tb;
            }
            w += g.adv * hs + ta;
            last_after = ta;
            seen = g.ci;
        }
        w - last_after
    };
    let mut lines: Vec<(usize, usize, usize, bool, bool)> = vec![];
    for (pi, &(a, b)) in paras.iter().enumerate() {
        let first_in_para = lines.len();
        match doc.box_size {
            Some(bx) => {
                let maxw = (bx[2] - doc.indent_left - doc.indent_right).max(1.0);
                let mut ls = a;
                let mut k = a;
                while k < b {
                    let mut we = k;
                    while we < b && !chars[we].is_whitespace() {
                        we += 1;
                    }
                    while we < b && chars[we].is_whitespace() {
                        we += 1;
                    }
                    let mut trimmed = we;
                    while trimmed > ls && chars[trimmed - 1].is_whitespace() {
                        trimmed -= 1;
                    }
                    let indent = if ls == a { doc.indent_first } else { 0.0 };
                    if k > ls && line_width(ls, trimmed) + indent > maxw {
                        lines.push((pi, ls, k, ls == a, false));
                        ls = k;
                    }
                    k = we.max(k + 1);
                }
                lines.push((pi, ls, b, ls == a, true));
            }
            None => lines.push((pi, a, b, true, true)),
        }
        if lines.len() == first_in_para {
            lines.push((pi, a, b, true, true));
        }
    }
    let mut glyphs = vec![];
    let mut line_info = vec![];
    let mut char_line = vec![0usize; n];
    let mut ascent0 = base_size * 0.8;
    let mut descent0 = base_size * 0.2;
    let mut baseline = 0.0;
    let mut prev_para = usize::MAX;
    for (li, &(pi, a, b, para_first, para_last)) in lines.iter().enumerate() {
        let max_size = (a..b).map(|c| eff[c].size).fold(0.0f64, f64::max).max(if a == b { eff.get(a).map(|e| e.size).unwrap_or(base_size) } else { 0.0 });
        let max_size = if max_size <= 0.0 { base_size } else { max_size };
        let lead = doc.leading.unwrap_or(max_size * 1.2);
        let probe = if a < b { &eff[a] } else if a < n { &eff[a] } else if n > 0 { &eff[n - 1] } else { &Eff { font: resolve(&doc.font, &doc.style), size: base_size, shift: 0.0, ch: ' ', features: vec![], style: Arc::new(doc.base_style()) } };
        let (asc, desc) = ascent_of(probe, &vars);
        if li == 0 {
            ascent0 = asc;
            descent0 = desc;
            baseline = match doc.box_size {
                Some(bx) => bx[1] + asc,
                None => 0.0,
            };
        } else {
            baseline += lead;
            if pi != prev_para {
                baseline += doc.space_after + doc.space_before;
            }
        }
        prev_para = pi;
        for c in char_line.iter_mut().take(b.min(n)).skip(a) {
            *c = li;
        }
        if b < n {
            char_line[b] = li;
        }
        let mut end = b;
        while end > a && chars[end - 1].is_whitespace() {
            end -= 1;
        }
        let sg = shape_range(&eff, a, b, &vars);
        let lw = line_width(a, end);
        let indent = if para_first { doc.indent_first } else { 0.0 };
        let justify_extra = match (doc.box_size, doc.justify) {
            (Some(bx), Justify::JustifyLeft | Justify::JustifyCenter | Justify::JustifyRight) if !para_last => Some(bx),
            (Some(bx), Justify::JustifyAll) => Some(bx),
            _ => None,
        }
        .map(|bx| {
            let avail = bx[2] - doc.indent_left - doc.indent_right - indent;
            let spaces = (a..end).filter(|c| chars[*c] == ' ').count();
            if spaces > 0 && avail > lw { (avail - lw) / spaces as f64 } else { 0.0 }
        })
        .unwrap_or(0.0);
        let lw_j = lw + justify_extra * (a..end).filter(|c| chars[*c] == ' ').count() as f64;
        let x0 = match doc.box_size {
            Some(bx) => {
                let left = bx[0] + doc.indent_left + indent;
                let right = bx[0] + bx[2] - doc.indent_right;
                match doc.justify {
                    Justify::Center => (left + right) / 2.0 - lw_j / 2.0,
                    Justify::Right => right - lw_j,
                    Justify::JustifyCenter if para_last => (left + right) / 2.0 - lw_j / 2.0,
                    Justify::JustifyRight if para_last => right - lw_j,
                    _ => left,
                }
            }
            None => match doc.justify {
                Justify::Center | Justify::JustifyCenter => -lw_j / 2.0,
                Justify::Right | Justify::JustifyRight => -lw_j,
                _ => 0.0,
            },
        };
        line_info.push((x0, baseline, lw_j));
        let mut pen = x0;
        let mut seen = usize::MAX;
        for g in &sg {
            let e = &eff[g.ci];
            let hs = e.style.h_scale / 100.0;
            let vs = e.style.v_scale / 100.0;
            let (tb, ta) = track_of(&eff, mods, g.ci);
            if g.ci != seen {
                pen += tb;
            }
            seen = g.ci;
            let adv = g.adv * hs;
            let ox = pen + g.dx * hs;
            let oy = baseline - e.shift - g.dy * vs;
            let kk = g.k;
            let place = Mat3([[kk * hs, if e.style.faux_italic { -0.21 * kk * vs } else { 0.0 }, ox], [0.0, -kk * vs, oy], [0.0, 0.0, 1.0]]);
            let paths: Vec<BezPath> = match e.font {
                Some(id) if kk > 0.0 => outline_of(id, g.gid, &vars).iter().map(|p| p.transformed(&place)).collect(),
                _ => vec![],
            };
            glyphs.push(Glyph {
                paths,
                char_index: g.ci,
                line: li,
                x: pen,
                baseline: baseline - e.shift,
                advance: adv,
                ch: chars[g.ci],
                size: e.size,
                font: e.font,
                gid: g.gid,
                place,
                vars: vars.clone(),
                style: e.style.clone(),
            });
            pen += adv + ta;
            if chars[g.ci] == ' ' && g.ci < end {
                pen += justify_extra;
            }
        }
    }
    let mut infos = Vec::with_capacity(n);
    let mut word = 0usize;
    let mut in_word = false;
    let mut any_word = false;
    for (ci, c) in chars.iter().enumerate() {
        let nl = *c == '\r' || *c == '\n';
        let sp = c.is_whitespace();
        let w = if sp {
            in_word = false;
            None
        } else {
            if !in_word {
                if any_word {
                    word += 1;
                }
                in_word = true;
                any_word = true;
            }
            Some(word)
        };
        infos.push(CharInfo { ch: *c, line: char_line.get(ci).copied().unwrap_or(0), word: w, space: sp && !nl, newline: nl });
    }
    let words = if any_word { word + 1 } else { 0 };
    Layout { glyphs, ascent: ascent0, descent: descent0, leading: doc.leading.unwrap_or(base_size * 1.2), lines: line_info, chars: infos, words }
}
