use std::sync::{Arc, OnceLock};

use rustybuzz::ttf_parser::OutlineBuilder;

use super::vector::{DrawOp, OpKind, Paint};
use crate::geom::Mat3;
use crate::model::TextDoc;
use crate::model::path::BezPath;

static DB: OnceLock<fontdb::Database> = OnceLock::new();

const FALLBACKS: [&str; 9] = ["Inter", "Helvetica Neue", "Helvetica", "Arial", "Segoe UI", "DejaVu Sans", "Liberation Sans", "Noto Sans", "FreeSans"];

pub fn db() -> &'static fontdb::Database {
    DB.get_or_init(|| {
        let mut d = fontdb::Database::new();
        d.load_system_fonts();
        if let Ok(dir) = std::env::var("FORMVFX_FONTS") {
            for p in std::env::split_paths(&dir) {
                d.load_fonts_dir(p);
            }
        }
        d
    })
}

pub fn families() -> Vec<(String, Vec<String>)> {
    let mut map: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for f in db().faces() {
        let Some((name, _)) = f.families.first() else { continue };
        let style = style_name(f.weight.0, f.style != fontdb::Style::Normal);
        let e = map.entry(name.clone()).or_default();
        if !e.contains(&style) {
            e.push(style);
        }
    }
    map.into_iter().collect()
}

fn style_name(weight: u16, italic: bool) -> String {
    let w = match weight {
        0..=150 => "Thin",
        151..=250 => "ExtraLight",
        251..=350 => "Light",
        351..=450 => "Regular",
        451..=550 => "Medium",
        551..=650 => "SemiBold",
        651..=750 => "Bold",
        751..=850 => "ExtraBold",
        _ => "Black",
    };
    match (w, italic) {
        ("Regular", true) => "Italic".into(),
        (w, true) => format!("{w} Italic"),
        (w, false) => w.into(),
    }
}

fn parse_style(style: &str) -> (u16, bool) {
    let s = style.to_ascii_lowercase().replace([' ', '-', '_'], "");
    let italic = s.contains("italic") || s.contains("oblique");
    let w = if s.contains("thin") || s.contains("hairline") {
        100
    } else if s.contains("extralight") || s.contains("ultralight") {
        200
    } else if s.contains("light") {
        300
    } else if s.contains("semibold") || s.contains("demibold") {
        600
    } else if s.contains("extrabold") || s.contains("ultrabold") {
        800
    } else if s.contains("black") || s.contains("heavy") {
        900
    } else if s.contains("bold") {
        700
    } else if s.contains("medium") {
        500
    } else {
        400
    };
    (w, italic)
}

pub fn resolve(family: &str, style: &str) -> Option<fontdb::ID> {
    let d = db();
    let (weight, italic) = parse_style(style);
    let q = |name: &str| {
        let fam = [fontdb::Family::Name(name)];
        d.query(&fontdb::Query { families: &fam, weight: fontdb::Weight(weight), stretch: fontdb::Stretch::Normal, style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal } })
    };
    let fam = family.trim();
    let base = fam.split(['-', ',']).next().unwrap_or(fam).trim();
    q(fam).or_else(|| q(base)).or_else(|| FALLBACKS.iter().find_map(|f| q(f))).or_else(|| d.faces().next().map(|f| f.id))
}

pub fn face_name(id: fontdb::ID) -> Option<(String, String)> {
    let f = db().face(id)?;
    Some((f.families.first()?.0.clone(), style_name(f.weight.0, f.style != fontdb::Style::Normal)))
}

pub struct Sink {
    pub paths: Vec<BezPath>,
    pub cur: Option<BezPath>,
}

impl Sink {
    fn last(&mut self) -> Option<&mut BezPath> {
        self.cur.as_mut()
    }
    fn push(&mut self, x: f32, y: f32) {
        if let Some(c) = self.cur.as_mut() {
            c.vertices.push([x as f64, y as f64]);
            c.ins.push([0.0; 2]);
            c.outs.push([0.0; 2]);
        }
    }
}

impl OutlineBuilder for Sink {
    fn move_to(&mut self, x: f32, y: f32) {
        if let Some(c) = self.cur.take() {
            self.paths.push(c);
        }
        self.cur = Some(BezPath { vertices: vec![[x as f64, y as f64]], ins: vec![[0.0; 2]], outs: vec![[0.0; 2]], closed: false });
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.push(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let Some(c) = self.last() else { return };
        let a = *c.vertices.last().unwrap();
        let (qx, qy) = (x1 as f64, y1 as f64);
        let (px, py) = (x as f64, y as f64);
        let li = c.outs.len() - 1;
        c.outs[li] = [2.0 / 3.0 * (qx - a[0]), 2.0 / 3.0 * (qy - a[1])];
        c.vertices.push([px, py]);
        c.ins.push([2.0 / 3.0 * (qx - px), 2.0 / 3.0 * (qy - py)]);
        c.outs.push([0.0; 2]);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let Some(c) = self.last() else { return };
        let a = *c.vertices.last().unwrap();
        let li = c.outs.len() - 1;
        c.outs[li] = [x1 as f64 - a[0], y1 as f64 - a[1]];
        c.vertices.push([x as f64, y as f64]);
        c.ins.push([x2 as f64 - x as f64, y2 as f64 - y as f64]);
        c.outs.push([0.0; 2]);
    }
    fn close(&mut self) {
        if let Some(mut c) = self.cur.take() {
            let n = c.vertices.len();
            if n > 1 {
                let f = c.vertices[0];
                let l = c.vertices[n - 1];
                if (f[0] - l[0]).abs() < 1e-6 && (f[1] - l[1]).abs() < 1e-6 {
                    let tin = c.ins[n - 1];
                    c.vertices.pop();
                    c.ins.pop();
                    c.outs.pop();
                    c.ins[0] = tin;
                }
            }
            c.closed = true;
            self.paths.push(c);
        }
    }
}

pub use super::layout::{CharInfo, Glyph, Layout, Mods, layout, layout_with};

pub fn text_ops(doc: &TextDoc) -> Vec<DrawOp> {
    let lay = layout(doc);
    glyph_ops(doc, &lay.glyphs)
}

pub struct GlyphPaint {
    pub fill: [f64; 3],
    pub stroke: [f64; 3],
    pub fill_opacity: f64,
    pub stroke_opacity: f64,
    pub stroke_width: f64,
    pub opacity: f64,
}

pub fn style_paint(g: &Glyph) -> GlyphPaint {
    let st = &g.style;
    GlyphPaint {
        fill: st.fill,
        stroke: st.stroke,
        fill_opacity: if st.apply_fill { 1.0 } else { 0.0 },
        stroke_opacity: if st.apply_stroke { 1.0 } else { 0.0 },
        stroke_width: st.stroke_width,
        opacity: 1.0,
    }
}

pub fn paint_ops(doc: &TextDoc, paths: Vec<BezPath>, g: &Glyph, gp: &GlyphPaint, fills: &mut Vec<DrawOp>, strokes: &mut Vec<DrawOp>) {
    if paths.is_empty() || gp.opacity <= 0.0 {
        return;
    }
    let fill = [gp.fill[0] as f32, gp.fill[1] as f32, gp.fill[2] as f32, 1.0];
    let stroke = [gp.stroke[0] as f32, gp.stroke[1] as f32, gp.stroke[2] as f32, 1.0];
    let fo = (gp.fill_opacity * gp.opacity) as f32;
    let so = (gp.stroke_opacity * gp.opacity) as f32;
    if fo > 0.0 {
        fills.push(DrawOp { paths: paths.clone(), paint: Paint::Solid(fill), opacity: fo, kind: OpKind::Fill { even_odd: false }, transform: Mat3::IDENTITY });
        if g.style.faux_bold {
            fills.push(DrawOp { paths: paths.clone(), paint: Paint::Solid(fill), opacity: fo, kind: OpKind::Stroke { width: g.size * 0.03, cap: 1, join: 1, miter: 4.0, dash: vec![], dash_offset: 0.0 }, transform: Mat3::IDENTITY });
        }
    }
    if so > 0.0 && gp.stroke_width > 0.0 {
        strokes.push(DrawOp { paths, paint: Paint::Solid(stroke), opacity: so, kind: OpKind::Stroke { width: gp.stroke_width, cap: 1, join: 1, miter: 4.0, dash: vec![], dash_offset: 0.0 }, transform: Mat3::IDENTITY });
    }
    let _ = doc;
}

pub fn order_ops(doc: &TextDoc, per_glyph: Vec<(Vec<DrawOp>, Vec<DrawOp>)>, mode: u32) -> Vec<DrawOp> {
    let stroke_first = !doc.stroke_over_fill;
    match mode {
        1 => {
            let (f, s): (Vec<_>, Vec<_>) = per_glyph.into_iter().unzip();
            s.into_iter().flatten().chain(f.into_iter().flatten()).collect()
        }
        2 => {
            let (f, s): (Vec<_>, Vec<_>) = per_glyph.into_iter().unzip();
            f.into_iter().flatten().chain(s.into_iter().flatten()).collect()
        }
        _ => per_glyph.into_iter().flat_map(|(f, s)| if stroke_first { s.into_iter().chain(f) } else { f.into_iter().chain(s) }).collect(),
    }
}

pub fn glyph_ops(doc: &TextDoc, glyphs: &[Glyph]) -> Vec<DrawOp> {
    let mut groups: Vec<(Vec<DrawOp>, Vec<DrawOp>)> = vec![];
    let mut i = 0;
    while i < glyphs.len() {
        let mut j = i + 1;
        while j < glyphs.len() && Arc::ptr_eq(&glyphs[j].style, &glyphs[i].style) {
            j += 1;
        }
        let paths: Vec<BezPath> = glyphs[i..j].iter().flat_map(|g| g.paths.iter().cloned()).collect();
        let (mut f, mut s) = (vec![], vec![]);
        paint_ops(doc, paths, &glyphs[i], &style_paint(&glyphs[i]), &mut f, &mut s);
        groups.push((f, s));
        i = j;
    }
    order_ops(doc, groups, 0)
}
