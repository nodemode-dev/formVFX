use serde::{Deserialize, Serialize};

use crate::geom::Mat3;

const KAPPA: f64 = 0.552_284_749_830_793_4;

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct BezPath {
    pub vertices: Vec<[f64; 2]>,
    #[serde(rename = "in")]
    pub ins: Vec<[f64; 2]>,
    #[serde(rename = "out")]
    pub outs: Vec<[f64; 2]>,
    pub closed: bool,
}

impl BezPath {
    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    pub fn tangent_in(&self, i: usize) -> [f64; 2] {
        self.ins.get(i).copied().unwrap_or([0.0; 2])
    }

    pub fn tangent_out(&self, i: usize) -> [f64; 2] {
        self.outs.get(i).copied().unwrap_or([0.0; 2])
    }

    pub fn normalized(mut self) -> BezPath {
        let n = self.vertices.len();
        self.ins.resize(n, [0.0; 2]);
        self.outs.resize(n, [0.0; 2]);
        self
    }

    pub fn polygon(points: &[[f64; 2]], closed: bool) -> BezPath {
        BezPath { vertices: points.to_vec(), ins: vec![[0.0; 2]; points.len()], outs: vec![[0.0; 2]; points.len()], closed }
    }

    pub fn rect(cx: f64, cy: f64, w: f64, h: f64, roundness: f64) -> BezPath {
        let (hw, hh) = (w.abs() / 2.0, h.abs() / 2.0);
        let r = roundness.max(0.0).min(hw).min(hh);
        if r <= 0.0 {
            return BezPath::polygon(&[[cx + hw, cy - hh], [cx + hw, cy + hh], [cx - hw, cy + hh], [cx - hw, cy - hh]], true);
        }
        let k = r * KAPPA;
        let (l, t, rr, b) = (cx - hw, cy - hh, cx + hw, cy + hh);
        BezPath {
            vertices: vec![[rr, t + r], [rr, b - r], [rr - r, b], [l + r, b], [l, b - r], [l, t + r], [l + r, t], [rr - r, t]],
            ins: vec![[0.0, -k], [0.0, 0.0], [k, 0.0], [0.0, 0.0], [0.0, k], [0.0, 0.0], [-k, 0.0], [0.0, 0.0]],
            outs: vec![[0.0, 0.0], [0.0, k], [0.0, 0.0], [-k, 0.0], [0.0, 0.0], [0.0, -k], [0.0, 0.0], [k, 0.0]],
            closed: true,
        }
    }

    pub fn ellipse(cx: f64, cy: f64, w: f64, h: f64) -> BezPath {
        let (rx, ry) = (w.abs() / 2.0, h.abs() / 2.0);
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        BezPath {
            vertices: vec![[cx, cy - ry], [cx + rx, cy], [cx, cy + ry], [cx - rx, cy]],
            ins: vec![[-kx, 0.0], [0.0, -ky], [kx, 0.0], [0.0, ky]],
            outs: vec![[kx, 0.0], [0.0, ky], [-kx, 0.0], [0.0, -ky]],
            closed: true,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn star(cx: f64, cy: f64, points: f64, outer: f64, inner: f64, rotation: f64, outer_round: f64, inner_round: f64, polygon: bool) -> BezPath {
        let n = points.round().max(3.0) as usize;
        let mut vertices = vec![];
        let mut radii = vec![];
        let mut rounds = vec![];
        let steps = if polygon { n } else { n * 2 };
        for i in 0..steps {
            let outer_pt = polygon || i % 2 == 0;
            let r = if outer_pt { outer } else { inner };
            let a = (rotation - 90.0).to_radians() + std::f64::consts::TAU * i as f64 / steps as f64;
            vertices.push([cx + r * a.cos(), cy + r * a.sin()]);
            radii.push((r, a));
            rounds.push(if outer_pt { outer_round } else { inner_round });
        }
        let mut ins = vec![[0.0; 2]; steps];
        let mut outs = vec![[0.0; 2]; steps];
        for i in 0..steps {
            let (r, a) = radii[i];
            let rd = rounds[i] / 100.0;
            if rd != 0.0 {
                let len = r * rd * std::f64::consts::TAU / steps as f64 * 0.5;
                let tx = -a.sin() * len;
                let ty = a.cos() * len;
                outs[i] = [tx, ty];
                ins[i] = [-tx, -ty];
            }
        }
        BezPath { vertices, ins, outs, closed: true }
    }

    pub fn transformed(&self, m: &Mat3) -> BezPath {
        let mut out = self.clone().normalized();
        for i in 0..out.vertices.len() {
            let v = out.vertices[i];
            let a = m.apply(v[0] + out.ins[i][0], v[1] + out.ins[i][1]);
            let b = m.apply(v[0] + out.outs[i][0], v[1] + out.outs[i][1]);
            let p = m.apply(v[0], v[1]);
            out.vertices[i] = p;
            out.ins[i] = [a[0] - p[0], a[1] - p[1]];
            out.outs[i] = [b[0] - p[0], b[1] - p[1]];
        }
        out
    }

    pub fn segments(&self) -> usize {
        let n = self.vertices.len();
        if n < 2 {
            0
        } else if self.closed {
            n
        } else {
            n - 1
        }
    }

    pub fn segment(&self, i: usize) -> [[f64; 2]; 4] {
        let n = self.vertices.len();
        let j = (i + 1) % n;
        let a = self.vertices[i];
        let b = self.vertices[j];
        let o = self.tangent_out(i);
        let t = self.tangent_in(j);
        [a, [a[0] + o[0], a[1] + o[1]], [b[0] + t[0], b[1] + t[1]], b]
    }

    pub fn point_on_segment(&self, i: usize, t: f64) -> [f64; 2] {
        let s = self.segment(i);
        cubic(&s, t)
    }

    pub fn flatten(&self, tol: f64) -> Vec<[f64; 2]> {
        let mut pts = vec![];
        if self.vertices.is_empty() {
            return pts;
        }
        pts.push(self.vertices[0]);
        for i in 0..self.segments() {
            let s = self.segment(i);
            let straight = s[1] == s[0] && s[2] == s[3];
            let steps = if straight {
                1
            } else {
                let l = dist(s[0], s[1]) + dist(s[1], s[2]) + dist(s[2], s[3]);
                ((l / tol.max(0.05)).sqrt().ceil() as usize).clamp(2, 256)
            };
            for k in 1..=steps {
                pts.push(cubic(&s, k as f64 / steps as f64));
            }
        }
        pts
    }

    pub fn bounds(&self) -> Option<[f64; 4]> {
        let pts = self.flatten(1.0);
        bounds_of(&pts)
    }

    pub fn to_kurbo(&self) -> kurbo::BezPath {
        let mut p = kurbo::BezPath::new();
        if self.vertices.is_empty() {
            return p;
        }
        let v0 = self.vertices[0];
        p.move_to((v0[0], v0[1]));
        for i in 0..self.segments() {
            let s = self.segment(i);
            if s[1] == s[0] && s[2] == s[3] {
                p.line_to((s[3][0], s[3][1]));
            } else {
                p.curve_to((s[1][0], s[1][1]), (s[2][0], s[2][1]), (s[3][0], s[3][1]));
            }
        }
        if self.closed {
            p.close_path();
        }
        p
    }

    pub fn from_kurbo(bp: &kurbo::BezPath) -> Vec<BezPath> {
        let mut out = vec![];
        let mut cur: Option<BezPath> = None;
        for el in bp.elements() {
            match *el {
                kurbo::PathEl::MoveTo(p) => {
                    if let Some(c) = cur.take() {
                        out.push(c);
                    }
                    cur = Some(BezPath { vertices: vec![[p.x, p.y]], ins: vec![[0.0; 2]], outs: vec![[0.0; 2]], closed: false });
                }
                kurbo::PathEl::LineTo(p) => {
                    if let Some(c) = cur.as_mut() {
                        c.vertices.push([p.x, p.y]);
                        c.ins.push([0.0; 2]);
                        c.outs.push([0.0; 2]);
                    }
                }
                kurbo::PathEl::QuadTo(q, p) => {
                    if let Some(c) = cur.as_mut() {
                        let a = *c.vertices.last().unwrap();
                        let c1 = [a[0] + 2.0 / 3.0 * (q.x - a[0]), a[1] + 2.0 / 3.0 * (q.y - a[1])];
                        let c2 = [p.x + 2.0 / 3.0 * (q.x - p.x), p.y + 2.0 / 3.0 * (q.y - p.y)];
                        let last = c.outs.len() - 1;
                        c.outs[last] = [c1[0] - a[0], c1[1] - a[1]];
                        c.vertices.push([p.x, p.y]);
                        c.ins.push([c2[0] - p.x, c2[1] - p.y]);
                        c.outs.push([0.0; 2]);
                    }
                }
                kurbo::PathEl::CurveTo(c1, c2, p) => {
                    if let Some(c) = cur.as_mut() {
                        let a = *c.vertices.last().unwrap();
                        let last = c.outs.len() - 1;
                        c.outs[last] = [c1.x - a[0], c1.y - a[1]];
                        c.vertices.push([p.x, p.y]);
                        c.ins.push([c2.x - p.x, c2.y - p.y]);
                        c.outs.push([0.0; 2]);
                    }
                }
                kurbo::PathEl::ClosePath => {
                    if let Some(mut c) = cur.take() {
                        let n = c.vertices.len();
                        if n > 1 && dist(c.vertices[0], c.vertices[n - 1]) < 1e-9 {
                            let tin = c.ins[n - 1];
                            c.vertices.pop();
                            c.ins.pop();
                            c.outs.pop();
                            c.ins[0] = tin;
                        }
                        c.closed = true;
                        out.push(c);
                    }
                }
            }
        }
        if let Some(c) = cur {
            out.push(c);
        }
        out
    }

    pub fn lerp(&self, b: &BezPath, t: f64) -> BezPath {
        if self.vertices.len() != b.vertices.len() {
            return if t < 1.0 { self.clone() } else { b.clone() };
        }
        let a = self.clone().normalized();
        let b = b.clone().normalized();
        let mix = |x: &[[f64; 2]], y: &[[f64; 2]]| x.iter().zip(y).map(|(p, q)| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]).collect::<Vec<_>>();
        BezPath { vertices: mix(&a.vertices, &b.vertices), ins: mix(&a.ins, &b.ins), outs: mix(&a.outs, &b.outs), closed: a.closed }
    }
}

pub fn cubic(s: &[[f64; 2]; 4], t: f64) -> [f64; 2] {
    let u = 1.0 - t;
    let a = u * u * u;
    let b = 3.0 * u * u * t;
    let c = 3.0 * u * t * t;
    let d = t * t * t;
    [a * s[0][0] + b * s[1][0] + c * s[2][0] + d * s[3][0], a * s[0][1] + b * s[1][1] + c * s[2][1] + d * s[3][1]]
}

pub fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

pub fn bounds_of(pts: &[[f64; 2]]) -> Option<[f64; 4]> {
    let mut it = pts.iter();
    let f = it.next()?;
    let mut b = [f[0], f[1], f[0], f[1]];
    for p in it {
        b[0] = b[0].min(p[0]);
        b[1] = b[1].min(p[1]);
        b[2] = b[2].max(p[0]);
        b[3] = b[3].max(p[1]);
    }
    Some(b)
}

pub fn union_bounds(a: Option<[f64; 4]>, b: Option<[f64; 4]>) -> Option<[f64; 4]> {
    match (a, b) {
        (Some(a), Some(b)) => Some([a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]),
        (x, None) => x,
        (None, y) => y,
    }
}

pub fn skia_path(paths: &[BezPath]) -> Option<tiny_skia::Path> {
    let mut pb = tiny_skia::PathBuilder::new();
    for p in paths {
        if p.vertices.is_empty() {
            continue;
        }
        let v0 = p.vertices[0];
        pb.move_to(v0[0] as f32, v0[1] as f32);
        for i in 0..p.segments() {
            let s = p.segment(i);
            if s[1] == s[0] && s[2] == s[3] {
                pb.line_to(s[3][0] as f32, s[3][1] as f32);
            } else {
                pb.cubic_to(s[1][0] as f32, s[1][1] as f32, s[2][0] as f32, s[2][1] as f32, s[3][0] as f32, s[3][1] as f32);
            }
        }
        if p.closed {
            pb.close();
        }
    }
    pb.finish()
}
