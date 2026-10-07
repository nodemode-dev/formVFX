use tiny_skia::{FillRule, GradientStop, LineCap, LineJoin, Pixmap, Point, Shader, SpreadMode, Stroke, StrokeDash, Transform};

use super::image::Img;
use crate::geom::Mat3;
use crate::model::path::{BezPath, skia_path, union_bounds};
use crate::model::Gradient;

#[derive(Clone, Debug)]
pub enum Paint {
    Solid([f32; 4]),
    Linear { start: [f64; 2], end: [f64; 2], grad: Gradient },
    Radial { center: [f64; 2], edge: [f64; 2], grad: Gradient },
}

#[derive(Clone, Debug)]
pub enum OpKind {
    Fill { even_odd: bool },
    Stroke { width: f64, cap: u32, join: u32, miter: f64, dash: Vec<f64>, dash_offset: f64 },
}

#[derive(Clone, Debug)]
pub struct DrawOp {
    pub paths: Vec<BezPath>,
    pub paint: Paint,
    pub opacity: f32,
    pub kind: OpKind,
    pub transform: Mat3,
}

impl DrawOp {
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b = None;
        for p in &self.paths {
            b = union_bounds(b, p.transformed(&self.transform).bounds());
        }
        let b = b?;
        let pad = match &self.kind {
            OpKind::Stroke { width, join, miter, .. } => {
                let s = self.transform.max_scale();
                width * s * if *join == 0 { miter.max(1.0) } else { 1.0 } / 2.0 + 1.0
            }
            _ => 1.0,
        };
        Some([b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad])
    }
}

pub fn ops_bounds(ops: &[DrawOp]) -> Option<[f64; 4]> {
    ops.iter().fold(None, |acc, o| union_bounds(acc, o.bounds()))
}

fn stops(g: &Gradient, opacity: f32) -> Vec<GradientStop> {
    g.stops
        .iter()
        .map(|s| GradientStop::new(s.pos.clamp(0.0, 1.0) as f32, tiny_skia::Color::from_rgba(s.color[0].clamp(0.0, 1.0) as f32, s.color[1].clamp(0.0, 1.0) as f32, s.color[2].clamp(0.0, 1.0) as f32, (s.color[3].clamp(0.0, 1.0) as f32 * opacity).clamp(0.0, 1.0)).unwrap_or(tiny_skia::Color::BLACK)))
        .collect()
}

fn shader(p: &Paint, opacity: f32, local: Transform) -> Option<Shader<'static>> {
    Some(match p {
        Paint::Solid(c) => {
            let a = (c[3] * opacity).clamp(0.0, 1.0);
            Shader::SolidColor(tiny_skia::Color::from_rgba(c[0].clamp(0.0, 1.0), c[1].clamp(0.0, 1.0), c[2].clamp(0.0, 1.0), a)?)
        }
        Paint::Linear { start, end, grad } => {
            let (s, e) = (Point::from_xy(start[0] as f32, start[1] as f32), Point::from_xy(end[0] as f32, end[1] as f32));
            if (start[0] - end[0]).abs() < 1e-6 && (start[1] - end[1]).abs() < 1e-6 {
                let c = grad.sample(1.0);
                return shader(&Paint::Solid([c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32]), opacity, local);
            }
            tiny_skia::LinearGradient::new(s, e, stops(grad, opacity), SpreadMode::Pad, local)?
        }
        Paint::Radial { center, edge, grad } => {
            let r = (edge[0] - center[0]).hypot(edge[1] - center[1]).max(0.01) as f32;
            let c = Point::from_xy(center[0] as f32, center[1] as f32);
            tiny_skia::RadialGradient::new(c, 0.0, c, r, stops(grad, opacity), SpreadMode::Pad, local)?
        }
    })
}

pub fn rasterize(ops: &[DrawOp], m: &Mat3, w: usize, h: usize, x0: i64, y0: i64) -> Option<Img> {
    if w == 0 || h == 0 || ops.is_empty() {
        return None;
    }
    let mut pm = Pixmap::new(w as u32, h as u32)?;
    let base = Mat3::translate(-(x0 as f64), -(y0 as f64)) * *m;
    for op in ops {
        if op.opacity <= 0.0 {
            continue;
        }
        let full = base * op.transform;
        let (path, tr, wscale) = if full.is_affine() {
            let Some(path) = skia_path(&op.paths) else { continue };
            (path, full.to_skia(), 1.0)
        } else {
            let Some((flat, k)) = projected(&op.paths, &full) else { continue };
            let Some(path) = skia_path(&flat) else { continue };
            (path, Transform::identity(), k)
        };
        let mut paint = tiny_skia::Paint::default();
        paint.anti_alias = true;
        let Some(sh) = shader(&op.paint, op.opacity, Transform::identity()) else { continue };
        paint.shader = sh;
        match &op.kind {
            OpKind::Fill { even_odd } => {
                pm.fill_path(&path, &paint, if *even_odd { FillRule::EvenOdd } else { FillRule::Winding }, tr, None);
            }
            OpKind::Stroke { width, cap, join, miter, dash, dash_offset } => {
                if *width <= 0.0 {
                    continue;
                }
                let stroke = Stroke {
                    width: (*width * wscale) as f32,
                    miter_limit: miter.max(1.0) as f32,
                    line_cap: match cap {
                        1 => LineCap::Round,
                        2 => LineCap::Square,
                        _ => LineCap::Butt,
                    },
                    line_join: match join {
                        1 => LineJoin::Round,
                        2 => LineJoin::Bevel,
                        _ => LineJoin::Miter,
                    },
                    dash: if dash.len() >= 2 && dash.iter().any(|d| *d > 0.0) { StrokeDash::new(dash.iter().map(|d| d.max(0.01) as f32).collect(), *dash_offset as f32) } else { None },
                };
                pm.stroke_path(&path, &paint, &stroke, tr, None);
            }
        }
    }
    Some(Img::from_pixmap(&pm))
}

pub fn projected(paths: &[BezPath], m: &Mat3) -> Option<(Vec<BezPath>, f64)> {
    let b = paths.iter().fold(None, |acc, p| union_bounds(acc, p.bounds()))?;
    let c = [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0];
    let p0 = m.apply(c[0], c[1]);
    let px = m.apply(c[0] + 1.0, c[1]);
    let py = m.apply(c[0], c[1] + 1.0);
    let k = (((px[0] - p0[0]) * (py[1] - p0[1]) - (px[1] - p0[1]) * (py[0] - p0[0])).abs()).sqrt().max(1e-6);
    let wr = &m.0[2];
    let mut out = vec![];
    for p in paths {
        let pts = p.flatten(0.35 / k);
        if pts.iter().any(|q| wr[0] * q[0] + wr[1] * q[1] + wr[2] <= 1e-9) {
            continue;
        }
        let tp: Vec<[f64; 2]> = pts.iter().map(|q| m.apply(q[0], q[1])).collect();
        out.push(BezPath::polygon(&tp, p.closed));
    }
    if out.is_empty() { None } else { Some((out, k)) }
}

pub fn coverage(paths: &[BezPath], m: &Mat3, w: usize, h: usize, x0: i64, y0: i64, even_odd: bool, stroke: f64) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    if w == 0 || h == 0 {
        return out;
    }
    let Some(mut pm) = Pixmap::new(w as u32, h as u32) else { return out };
    let Some(path) = skia_path(paths) else { return out };
    let tr = (Mat3::translate(-(x0 as f64), -(y0 as f64)) * *m).to_skia();
    let mut paint = tiny_skia::Paint::default();
    paint.anti_alias = true;
    paint.shader = Shader::SolidColor(tiny_skia::Color::WHITE);
    if stroke > 0.0 {
        let s = Stroke { width: stroke as f32, line_join: LineJoin::Round, line_cap: LineCap::Round, ..Stroke::default() };
        pm.stroke_path(&path, &paint, &s, tr, None);
    } else {
        pm.fill_path(&path, &paint, if even_odd { FillRule::EvenOdd } else { FillRule::Winding }, tr, None);
    }
    for (o, px) in out.iter_mut().zip(pm.data().chunks_exact(4)) {
        *o = px[3] as f32 / 255.0;
    }
    out
}
