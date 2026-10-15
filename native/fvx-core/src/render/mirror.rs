use super::image::Img;
use super::light::{add, cross, dot, norm, scale, sub};
use super::mesh::{MatEval, Tri};
use crate::geom::Mat4;

pub struct Plane {
    pub id: u64,
    pub inv: Mat4,
    pub world: Mat4,
    pub img: Img,
    pub rect: [f64; 2],
    pub r: f64,
}

pub struct Hit {
    pub t: f64,
    pub p: [f64; 3],
    pub n: [f64; 3],
    pub color: [f32; 4],
    pub mesh: Option<(usize, u16)>,
}

struct Node {
    lo: [f64; 3],
    hi: [f64; 3],
    start: usize,
    count: usize,
    left: usize,
}

pub struct Mirror {
    pub planes: Vec<Plane>,
    tris: Vec<Tri>,
    owner: Vec<usize>,
    pub meshes: Vec<(u64, Vec<MatEval>)>,
    nodes: Vec<Node>,
}

fn bounds(tris: &[Tri], idx: &[usize]) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for &i in idx {
        for v in &tris[i].v {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
    }
    (lo, hi)
}

fn centroid(t: &Tri, k: usize) -> f64 {
    (t.v[0][k] + t.v[1][k] + t.v[2][k]) / 3.0
}

impl Mirror {
    pub fn new(planes: Vec<Plane>, meshes: Vec<(u64, Vec<Tri>, Vec<MatEval>)>) -> Mirror {
        let mut tris = vec![];
        let mut owner = vec![];
        let mut mats = vec![];
        for (mi, (id, ts, ms)) in meshes.into_iter().enumerate() {
            owner.extend(std::iter::repeat_n(mi, ts.len()));
            tris.extend(ts);
            mats.push((id, ms));
        }
        let mut idx: Vec<usize> = (0..tris.len()).collect();
        let mut nodes = vec![];
        if !idx.is_empty() {
            let n = idx.len();
            build(&tris, &mut idx, 0, n, &mut nodes);
        }
        let tris: Vec<Tri> = idx.iter().map(|&i| tris[i]).collect();
        let owner: Vec<usize> = idx.iter().map(|&i| owner[i]).collect();
        Mirror { planes, tris, owner, meshes: mats, nodes }
    }

    pub fn is_empty(&self) -> bool {
        self.planes.is_empty() && self.tris.is_empty()
    }

    pub fn trace(&self, o: [f64; 3], d: [f64; 3], skip: u64) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        for pl in &self.planes {
            if pl.id == skip {
                continue;
            }
            let a = pl.inv.apply(o);
            let b = pl.inv.apply(add(o, d));
            let dz = b[2] - a[2];
            if dz.abs() < 1e-12 {
                continue;
            }
            let t = -a[2] / dz;
            if t <= 1e-3 || best.as_ref().is_some_and(|h| h.t <= t) {
                continue;
            }
            let hit = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            let s = pl.img.sample((hit[0] - pl.rect[0]) * pl.r, (hit[1] - pl.rect[1]) * pl.r, false);
            if s[3] <= 1e-3 {
                continue;
            }
            let n = norm(sub(pl.world.apply([0.0, 0.0, -1.0]), pl.world.apply([0.0; 3])));
            let color = [s[0] / s[3], s[1] / s[3], s[2] / s[3], s[3]];
            best = Some(Hit { t, p: add(o, scale(d, t)), n, color, mesh: None });
        }
        if self.nodes.is_empty() {
            return best;
        }
        let inv = [1.0 / d[0], 1.0 / d[1], 1.0 / d[2]];
        let mut stack = vec![0usize];
        while let Some(ni) = stack.pop() {
            let nd = &self.nodes[ni];
            let limit = best.as_ref().map(|h| h.t).unwrap_or(f64::MAX);
            if !slab(o, inv, nd.lo, nd.hi, limit) {
                continue;
            }
            if nd.count > 0 {
                for i in nd.start..nd.start + nd.count {
                    let mi = self.owner[i];
                    if self.meshes[mi].0 == skip {
                        continue;
                    }
                    let tr = &self.tris[i];
                    let Some((t, u, v)) = intersect(o, d, tr) else { continue };
                    if t <= 1e-3 || best.as_ref().is_some_and(|h| h.t <= t) {
                        continue;
                    }
                    let w = 1.0 - u - v;
                    let n = [0, 1, 2].map(|k| (tr.n[0][k] * w as f32 + tr.n[1][k] * u as f32 + tr.n[2][k] * v as f32) as f64);
                    let uv = [0, 1].map(|k| tr.uv[0][k] * w as f32 + tr.uv[1][k] * u as f32 + tr.uv[2][k] * v as f32);
                    let color = self.meshes[mi].1.get(tr.mat as usize).map(|m| m.color(uv)).unwrap_or([1.0; 4]);
                    best = Some(Hit { t, p: add(o, scale(d, t)), n: norm(n), color, mesh: Some((mi, tr.mat)) });
                }
            } else {
                stack.push(nd.left);
                stack.push(nd.left + 1);
            }
        }
        best
    }
}

fn build(tris: &[Tri], idx: &mut [usize], start: usize, end: usize, nodes: &mut Vec<Node>) {
    nodes.push(Node { lo: [0.0; 3], hi: [0.0; 3], start: 0, count: 0, left: 0 });
    build_into(tris, idx, start, end, nodes, 0);
}

fn build_into(tris: &[Tri], idx: &mut [usize], start: usize, end: usize, nodes: &mut Vec<Node>, slot: usize) {
    let (lo, hi) = bounds(tris, &idx[start..end]);
    let n = end - start;
    nodes[slot] = Node { lo, hi, start, count: 0, left: 0 };
    if n <= 4 {
        nodes[slot].count = n;
        return;
    }
    let ext = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let k = if ext[0] >= ext[1] && ext[0] >= ext[2] { 0 } else if ext[1] >= ext[2] { 1 } else { 2 };
    let mid = start + n / 2;
    idx[start..end].select_nth_unstable_by(n / 2, |a, b| centroid(&tris[*a], k).total_cmp(&centroid(&tris[*b], k)));
    let left = nodes.len();
    nodes[slot].left = left;
    nodes.push(Node { lo, hi, start: 0, count: 0, left: 0 });
    nodes.push(Node { lo, hi, start: 0, count: 0, left: 0 });
    build_into(tris, idx, start, mid, nodes, left);
    build_into(tris, idx, mid, end, nodes, left + 1);
}

fn slab(o: [f64; 3], inv: [f64; 3], lo: [f64; 3], hi: [f64; 3], limit: f64) -> bool {
    let mut t0 = 0.0f64;
    let mut t1 = limit;
    for k in 0..3 {
        let a = (lo[k] - o[k]) * inv[k];
        let b = (hi[k] - o[k]) * inv[k];
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        if a.is_nan() || b.is_nan() {
            continue;
        }
        t0 = t0.max(a);
        t1 = t1.min(b);
        if t0 > t1 {
            return false;
        }
    }
    true
}

fn intersect(o: [f64; 3], d: [f64; 3], t: &Tri) -> Option<(f64, f64, f64)> {
    let e1 = sub(t.v[1], t.v[0]);
    let e2 = sub(t.v[2], t.v[0]);
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(o, t.v[0]);
    let u = dot(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(d, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some((dot(e2, q) * inv, u, v))
}
