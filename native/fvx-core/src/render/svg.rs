use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use usvg::tiny_skia_path::{PathSegment, Transform};

use super::vector::{DrawOp, OpKind, Paint};
use crate::geom::Mat3;
use crate::model::path::BezPath;
use crate::model::value::{GradStop, Gradient};

#[derive(Clone, Debug)]
pub enum SvgPaint {
    Solid([f64; 4]),
    Linear([f64; 2], [f64; 2], Gradient),
    Radial([f64; 2], [f64; 2], Gradient),
}

#[derive(Clone, Debug)]
pub struct SvgStroke {
    pub paint: SvgPaint,
    pub opacity: f64,
    pub width: f64,
    pub cap: u32,
    pub join: u32,
    pub miter: f64,
    pub dash: Vec<f64>,
    pub dash_offset: f64,
}

#[derive(Clone, Debug)]
pub struct SvgShape {
    pub name: String,
    pub paths: Vec<BezPath>,
    pub fill: Option<(SvgPaint, f64, bool)>,
    pub stroke: Option<SvgStroke>,
    pub stroke_first: bool,
}

#[derive(Clone, Debug)]
pub struct SvgDoc {
    pub width: f64,
    pub height: f64,
    pub shapes: Vec<SvgShape>,
}

fn mat(t: &Transform) -> Mat3 {
    Mat3([[t.sx as f64, t.kx as f64, t.tx as f64], [t.ky as f64, t.sy as f64, t.ty as f64], [0.0, 0.0, 1.0]])
}

fn bez(data: &usvg::tiny_skia_path::Path, m: &Mat3) -> Vec<BezPath> {
    let mut out: Vec<BezPath> = vec![];
    let mut cur: Option<BezPath> = None;
    let push_cur = |cur: &mut Option<BezPath>, out: &mut Vec<BezPath>| {
        if let Some(c) = cur.take()
            && c.vertices.len() >= 2
        {
            out.push(c);
        }
    };
    for seg in data.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                push_cur(&mut cur, &mut out);
                cur = Some(BezPath { vertices: vec![[p.x as f64, p.y as f64]], ins: vec![[0.0; 2]], outs: vec![[0.0; 2]], closed: false });
            }
            PathSegment::LineTo(p) => {
                if let Some(c) = cur.as_mut() {
                    c.vertices.push([p.x as f64, p.y as f64]);
                    c.ins.push([0.0; 2]);
                    c.outs.push([0.0; 2]);
                }
            }
            PathSegment::QuadTo(q, p) => {
                if let Some(c) = cur.as_mut() {
                    let a = *c.vertices.last().unwrap();
                    let (qx, qy, px, py) = (q.x as f64, q.y as f64, p.x as f64, p.y as f64);
                    let li = c.outs.len() - 1;
                    c.outs[li] = [2.0 / 3.0 * (qx - a[0]), 2.0 / 3.0 * (qy - a[1])];
                    c.vertices.push([px, py]);
                    c.ins.push([2.0 / 3.0 * (qx - px), 2.0 / 3.0 * (qy - py)]);
                    c.outs.push([0.0; 2]);
                }
            }
            PathSegment::CubicTo(p1, p2, p) => {
                if let Some(c) = cur.as_mut() {
                    let a = *c.vertices.last().unwrap();
                    let li = c.outs.len() - 1;
                    c.outs[li] = [p1.x as f64 - a[0], p1.y as f64 - a[1]];
                    c.vertices.push([p.x as f64, p.y as f64]);
                    c.ins.push([p2.x as f64 - p.x as f64, p2.y as f64 - p.y as f64]);
                    c.outs.push([0.0; 2]);
                }
            }
            PathSegment::Close => {
                if let Some(mut c) = cur.take() {
                    let n = c.vertices.len();
                    if n > 2 {
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
                    if c.vertices.len() >= 2 {
                        out.push(c);
                    }
                }
            }
        }
    }
    push_cur(&mut cur, &mut out);
    out.into_iter().map(|p| p.transformed(m)).collect()
}

fn stops(s: &[usvg::Stop]) -> Gradient {
    Gradient {
        stops: s
            .iter()
            .map(|st| {
                let c = st.color();
                GradStop { pos: st.offset().get() as f64, color: [c.red as f64 / 255.0, c.green as f64 / 255.0, c.blue as f64 / 255.0, st.opacity().get() as f64] }
            })
            .collect(),
    }
}

fn paint(p: &usvg::Paint, m: &Mat3) -> Option<SvgPaint> {
    Some(match p {
        usvg::Paint::Color(c) => SvgPaint::Solid([c.red as f64 / 255.0, c.green as f64 / 255.0, c.blue as f64 / 255.0, 1.0]),
        usvg::Paint::LinearGradient(g) => {
            let gm = *m * mat(&g.transform());
            SvgPaint::Linear(gm.apply(g.x1() as f64, g.y1() as f64), gm.apply(g.x2() as f64, g.y2() as f64), stops(g.stops()))
        }
        usvg::Paint::RadialGradient(g) => {
            let gm = *m * mat(&g.transform());
            let c = gm.apply(g.cx() as f64, g.cy() as f64);
            let e = gm.apply(g.cx() as f64 + g.r().get() as f64, g.cy() as f64);
            SvgPaint::Radial(c, e, stops(g.stops()))
        }
        usvg::Paint::Pattern(_) => SvgPaint::Solid([0.5, 0.5, 0.5, 1.0]),
    })
}

fn walk(g: &usvg::Group, opacity: f64, out: &mut Vec<SvgShape>) {
    let op = opacity * g.opacity().get() as f64;
    for n in g.children() {
        match n {
            usvg::Node::Group(sub) => walk(sub, op, out),
            usvg::Node::Path(p) => {
                if !p.is_visible() {
                    continue;
                }
                let m = mat(&p.abs_transform());
                let paths = bez(p.data(), &m);
                if paths.is_empty() {
                    continue;
                }
                let fill = p.fill().and_then(|f| Some((paint(f.paint(), &m)?, f.opacity().get() as f64 * op, matches!(f.rule(), usvg::FillRule::EvenOdd))));
                let k = m.max_scale();
                let stroke = p.stroke().and_then(|s| {
                    Some(SvgStroke {
                        paint: paint(s.paint(), &m)?,
                        opacity: s.opacity().get() as f64 * op,
                        width: s.width().get() as f64 * k,
                        cap: match s.linecap() {
                            usvg::LineCap::Round => 1,
                            usvg::LineCap::Square => 2,
                            _ => 0,
                        },
                        join: match s.linejoin() {
                            usvg::LineJoin::Round => 1,
                            usvg::LineJoin::Bevel => 2,
                            _ => 0,
                        },
                        miter: s.miterlimit().get() as f64,
                        dash: s.dasharray().map(|d| d.iter().map(|x| *x as f64 * k).collect()).unwrap_or_default(),
                        dash_offset: s.dashoffset() as f64 * k,
                    })
                });
                let name = if p.id().is_empty() { format!("Path {}", out.len() + 1) } else { p.id().to_string() };
                out.push(SvgShape { name, paths, fill, stroke, stroke_first: matches!(p.paint_order(), usvg::PaintOrder::StrokeAndFill) });
            }
            usvg::Node::Image(_) | usvg::Node::Text(_) => {}
        }
    }
}

pub fn parse(text: &str) -> Result<SvgDoc, String> {
    let tree = usvg::Tree::from_str(text, &usvg::Options::default()).map_err(|e| format!("not readable SVG: {e}"))?;
    let mut shapes = vec![];
    walk(tree.root(), 1.0, &mut shapes);
    let s = tree.size();
    Ok(SvgDoc { width: s.width() as f64, height: s.height() as f64, shapes })
}

type Cache = Mutex<HashMap<String, (std::time::SystemTime, Arc<SvgDoc>)>>;
static CACHE: OnceLock<Cache> = OnceLock::new();

pub fn load(path: &str) -> Option<Arc<SvgDoc>> {
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    let c = CACHE.get_or_init(Default::default);
    if let Some((t, d)) = c.lock().get(path)
        && *t == mtime
    {
        return Some(d.clone());
    }
    let text = std::fs::read_to_string(path).ok()?;
    let doc = Arc::new(parse(&text).ok()?);
    c.lock().insert(path.to_string(), (mtime, doc.clone()));
    Some(doc)
}

fn to_paint(p: &SvgPaint) -> Paint {
    match p {
        SvgPaint::Solid(c) => Paint::Solid([c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32]),
        SvgPaint::Linear(a, b, g) => Paint::Linear { start: *a, end: *b, grad: g.clone() },
        SvgPaint::Radial(a, b, g) => Paint::Radial { center: *a, edge: *b, grad: g.clone() },
    }
}

pub fn ops(doc: &SvgDoc) -> Vec<DrawOp> {
    let mut out = vec![];
    for s in &doc.shapes {
        let fill = s.fill.as_ref().map(|(p, o, eo)| DrawOp { paths: s.paths.clone(), paint: to_paint(p), opacity: *o as f32, kind: OpKind::Fill { even_odd: *eo }, transform: Mat3::IDENTITY });
        let stroke = s.stroke.as_ref().map(|st| DrawOp {
            paths: s.paths.clone(),
            paint: to_paint(&st.paint),
            opacity: st.opacity as f32,
            kind: OpKind::Stroke { width: st.width, cap: st.cap, join: st.join, miter: st.miter, dash: st.dash.clone(), dash_offset: st.dash_offset },
            transform: Mat3::IDENTITY,
        });
        if s.stroke_first {
            out.extend(stroke);
            out.extend(fill);
        } else {
            out.extend(fill);
            out.extend(stroke);
        }
    }
    out
}

pub fn bounds(doc: &SvgDoc) -> Option<[f64; 4]> {
    doc.shapes.iter().flat_map(|s| s.paths.iter()).fold(None, |acc, p| crate::model::path::union_bounds(acc, p.bounds()))
}
