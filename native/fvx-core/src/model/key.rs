use serde::{Deserialize, Serialize};

use super::value::Value;

pub const TIME_EPS: f64 = 1e-6;
pub const DEFAULT_INFLUENCE: f64 = 16.666_667;
pub const EASY_INFLUENCE: f64 = 33.333_333;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interp {
    #[default]
    Linear,
    Bezier,
    Hold,
}

impl Interp {
    pub fn parse(s: &str) -> Option<Interp> {
        Some(match s.to_ascii_lowercase().as_str() {
            "linear" => Interp::Linear,
            "bezier" | "continuousbezier" | "autobezier" | "auto" => Interp::Bezier,
            "hold" => Interp::Hold,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ease {
    pub speed: f64,
    pub influence: f64,
}

impl Default for Ease {
    fn default() -> Self {
        Ease { speed: 0.0, influence: DEFAULT_INFLUENCE }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub time: f64,
    pub value: Value,
    #[serde(default)]
    pub in_interp: Interp,
    #[serde(default)]
    pub out_interp: Interp,
    #[serde(default)]
    pub in_ease: Vec<Ease>,
    #[serde(default)]
    pub out_ease: Vec<Ease>,
    #[serde(default)]
    pub auto_bezier: bool,
    #[serde(default)]
    pub continuous: bool,
    #[serde(default)]
    pub spatial_in: [f64; 3],
    #[serde(default)]
    pub spatial_out: [f64; 3],
    #[serde(default)]
    pub spatial_auto: bool,
    #[serde(default)]
    pub roving: bool,
    #[serde(default)]
    pub label: u8,
}

impl Key {
    pub fn new(time: f64, value: Value, spatial: bool) -> Key {
        Key {
            time,
            value,
            in_interp: Interp::Linear,
            out_interp: Interp::Linear,
            in_ease: vec![],
            out_ease: vec![],
            auto_bezier: false,
            continuous: false,
            spatial_in: [0.0; 3],
            spatial_out: [0.0; 3],
            spatial_auto: spatial,
            roving: false,
            label: 0,
        }
    }
}

pub fn same_time(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-4
}

pub fn ease_dims(v: &Value, spatial: bool) -> usize {
    if spatial || !v.is_numeric() { 1 } else { v.dims() }
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn len3(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

pub fn spatial_tangents(keys: &[Key], i: usize) -> ([f64; 3], [f64; 3]) {
    let k = &keys[i];
    if !k.spatial_auto {
        return (k.spatial_in, k.spatial_out);
    }
    if i == 0 || i + 1 >= keys.len() {
        return ([0.0; 3], [0.0; 3]);
    }
    let p0 = keys[i - 1].value.vec3();
    let p1 = k.value.vec3();
    let p2 = keys[i + 1].value.vec3();
    let d = sub3(p2, p0);
    let la = len3(sub3(p1, p0));
    let lb = len3(sub3(p2, p1));
    let s = la + lb;
    if s < 1e-9 {
        return ([0.0; 3], [0.0; 3]);
    }
    let fo = lb / s / 3.0;
    let fi = la / s / 3.0;
    ([-d[0] * fi, -d[1] * fi, -d[2] * fi], [d[0] * fo, d[1] * fo, d[2] * fo])
}

fn cubic3(p0: [f64; 3], p1: [f64; 3], p2: [f64; 3], p3: [f64; 3], u: f64) -> [f64; 3] {
    let v = 1.0 - u;
    let (a, b, c, d) = (v * v * v, 3.0 * v * v * u, 3.0 * v * u * u, u * u * u);
    [a * p0[0] + b * p1[0] + c * p2[0] + d * p3[0], a * p0[1] + b * p1[1] + c * p2[1] + d * p3[1], a * p0[2] + b * p1[2] + c * p2[2] + d * p3[2]]
}

struct Arc {
    pts: [[f64; 3]; 4],
    lut: Vec<(f64, f64)>,
    length: f64,
}

impl Arc {
    fn new(p0: [f64; 3], p1: [f64; 3], p2: [f64; 3], p3: [f64; 3]) -> Arc {
        let n = 48;
        let mut lut = Vec::with_capacity(n + 1);
        let mut acc = 0.0;
        let mut prev = p0;
        lut.push((0.0, 0.0));
        for i in 1..=n {
            let u = i as f64 / n as f64;
            let p = cubic3(p0, p1, p2, p3, u);
            acc += len3(sub3(p, prev));
            prev = p;
            lut.push((u, acc));
        }
        Arc { pts: [p0, p1, p2, p3], lut, length: acc }
    }

    fn at_length(&self, d: f64) -> [f64; 3] {
        if self.length < 1e-12 {
            return self.pts[0];
        }
        let d = d.clamp(0.0, self.length);
        let i = self.lut.partition_point(|x| x.1 < d).clamp(1, self.lut.len() - 1);
        let (u0, l0) = self.lut[i - 1];
        let (u1, l1) = self.lut[i];
        let u = if l1 - l0 > 1e-12 { u0 + (u1 - u0) * (d - l0) / (l1 - l0) } else { u1 };
        let [a, b, c, e] = self.pts;
        cubic3(a, b, c, e, u)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn ease_curve(t: f64, t0: f64, t1: f64, y0: f64, y1: f64, out: Option<Ease>, inn: Option<Ease>) -> f64 {
    ease_between(t, t0, t1, y0, y1, out, inn, false)
}

#[allow(clippy::too_many_arguments)]
fn ease_between(t: f64, t0: f64, t1: f64, y0: f64, y1: f64, out: Option<Ease>, inn: Option<Ease>, monotone: bool) -> f64 {
    let dt = t1 - t0;
    if dt <= 0.0 {
        return y1;
    }
    let u_lin = ((t - t0) / dt).clamp(0.0, 1.0);
    if out.is_none() && inn.is_none() {
        return y0 + (y1 - y0) * u_lin;
    }
    let avg = (y1 - y0) / dt;
    let (s0, i0) = match out {
        Some(e) => (e.speed, (e.influence / 100.0).clamp(0.001, 1.0)),
        None => (avg, 1.0 / 3.0),
    };
    let (s1, i1) = match inn {
        Some(e) => (e.speed, (e.influence / 100.0).clamp(0.001, 1.0)),
        None => (avg, 1.0 / 3.0),
    };
    let x1 = i0;
    let x2 = 1.0 - i1;
    let yy0 = y0;
    let (lo, hi) = (y0.min(y1), y0.max(y1));
    let keep = |v: f64| if monotone { v.clamp(lo, hi) } else { v };
    let yy1 = keep(y0 + s0 * i0 * dt);
    let yy2 = keep(y1 - s1 * i1 * dt);
    let yy3 = y1;
    let bx = |u: f64| {
        let v = 1.0 - u;
        3.0 * v * v * u * x1 + 3.0 * v * u * u * x2 + u * u * u
    };
    let dbx = |u: f64| {
        let v = 1.0 - u;
        3.0 * v * v * x1 + 6.0 * v * u * (x2 - x1) + 3.0 * u * u * (1.0 - x2)
    };
    let target = u_lin;
    let mut u = target;
    for _ in 0..8 {
        let e = bx(u) - target;
        if e.abs() < 1e-9 {
            break;
        }
        let d = dbx(u);
        if d.abs() < 1e-9 {
            break;
        }
        u = (u - e / d).clamp(0.0, 1.0);
    }
    if (bx(u) - target).abs() > 1e-6 {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..60 {
            u = (lo + hi) / 2.0;
            if bx(u) < target {
                lo = u;
            } else {
                hi = u;
            }
        }
    }
    let v = 1.0 - u;
    v * v * v * yy0 + 3.0 * v * v * u * yy1 + 3.0 * v * u * u * yy2 + u * u * u * yy3
}

fn auto_speed(keys: &[Key], i: usize, dim: usize, spatial: bool) -> f64 {
    let comp = |k: &Key| -> f64 {
        if spatial {
            0.0
        } else {
            k.value.components().get(dim).copied().unwrap_or(0.0)
        }
    };
    if spatial {
        let p = |j: usize| keys[j].value.vec3();
        if i > 0 && i + 1 < keys.len() {
            let l = len3(sub3(p(i), p(i - 1))) + len3(sub3(p(i + 1), p(i)));
            return l / (keys[i + 1].time - keys[i - 1].time).max(1e-9);
        }
        if i + 1 < keys.len() {
            return len3(sub3(p(i + 1), p(i))) / (keys[i + 1].time - keys[i].time).max(1e-9);
        }
        if i > 0 {
            return len3(sub3(p(i), p(i - 1))) / (keys[i].time - keys[i - 1].time).max(1e-9);
        }
        return 0.0;
    }
    if i > 0 && i + 1 < keys.len() {
        return (comp(&keys[i + 1]) - comp(&keys[i - 1])) / (keys[i + 1].time - keys[i - 1].time).max(1e-9);
    }
    if i + 1 < keys.len() {
        return (comp(&keys[i + 1]) - comp(&keys[i])) / (keys[i + 1].time - keys[i].time).max(1e-9);
    }
    if i > 0 {
        return (comp(&keys[i]) - comp(&keys[i - 1])) / (keys[i].time - keys[i - 1].time).max(1e-9);
    }
    0.0
}

pub fn side_ease(keys: &[Key], i: usize, dim: usize, spatial: bool, out: bool) -> Option<Ease> {
    let k = &keys[i];
    let interp = if out { k.out_interp } else { k.in_interp };
    if interp != Interp::Bezier {
        return None;
    }
    if k.auto_bezier {
        return Some(Ease { speed: auto_speed(keys, i, dim, spatial), influence: DEFAULT_INFLUENCE });
    }
    let list = if out { &k.out_ease } else { &k.in_ease };
    Some(list.get(dim).or_else(|| list.first()).copied().unwrap_or_default())
}

fn segment(keys: &[Key], t: f64) -> Option<usize> {
    if keys.len() < 2 || t <= keys[0].time || t >= keys[keys.len() - 1].time {
        return None;
    }
    let i = keys.partition_point(|k| k.time <= t);
    Some(i.saturating_sub(1).min(keys.len() - 2))
}

pub fn has_roving(keys: &[Key]) -> bool {
    keys.len() > 2 && keys[1..keys.len() - 1].iter().any(|k| k.roving)
}

pub fn roving_times(keys: &[Key]) -> Vec<Key> {
    let mut out = keys.to_vec();
    let n = keys.len();
    if n < 3 {
        return out;
    }
    let seg_len = |i: usize| -> f64 {
        let (_, to) = spatial_tangents(keys, i);
        let (ti, _) = spatial_tangents(keys, i + 1);
        let p0 = keys[i].value.vec3();
        let p3 = keys[i + 1].value.vec3();
        Arc::new(p0, [p0[0] + to[0], p0[1] + to[1], p0[2] + to[2]], [p3[0] + ti[0], p3[1] + ti[1], p3[2] + ti[2]], p3).length
    };
    let mut a = 0;
    while a < n - 1 {
        let mut b = a + 1;
        while b < n - 1 && keys[b].roving {
            b += 1;
        }
        if b > a + 1 {
            let lens: Vec<f64> = (a..b).map(seg_len).collect();
            let total: f64 = lens.iter().sum();
            if total > 1e-9 {
                let (t0, t1) = (keys[a].time, keys[b].time);
                let mut acc = 0.0;
                for (k, l) in lens.iter().enumerate().take(b - a - 1) {
                    acc += l;
                    out[a + k + 1].time = t0 + (t1 - t0) * acc / total;
                }
            }
        }
        a = b;
    }
    out
}

pub fn value_at(keys: &[Key], t: f64, spatial: bool) -> Option<Value> {
    if spatial && has_roving(keys) {
        return value_at_plain(&roving_times(keys), t, spatial);
    }
    value_at_plain(keys, t, spatial)
}

fn value_at_plain(keys: &[Key], t: f64, spatial: bool) -> Option<Value> {
    let first = keys.first()?;
    if keys.len() == 1 || t <= first.time {
        return Some(first.value.clone());
    }
    let last = &keys[keys.len() - 1];
    if t >= last.time {
        return Some(last.value.clone());
    }
    let i = segment(keys, t)?;
    let a = &keys[i];
    let b = &keys[i + 1];
    if a.out_interp == Interp::Hold {
        return Some(a.value.clone());
    }
    let both_linear = a.out_interp == Interp::Linear && b.in_interp == Interp::Linear;
    if spatial && matches!(a.value, Value::Vec2(_) | Value::Vec3(_)) {
        let (_, to) = spatial_tangents(keys, i);
        let (ti, _) = spatial_tangents(keys, i + 1);
        let p0 = a.value.vec3();
        let p3 = b.value.vec3();
        let p1 = [p0[0] + to[0], p0[1] + to[1], p0[2] + to[2]];
        let p2 = [p3[0] + ti[0], p3[1] + ti[1], p3[2] + ti[2]];
        let arc = Arc::new(p0, p1, p2, p3);
        let d = if both_linear {
            arc.length * (t - a.time) / (b.time - a.time)
        } else {
            ease_between(t, a.time, b.time, 0.0, arc.length, side_ease(keys, i, 0, true, true), side_ease(keys, i + 1, 0, true, false), true)
        };
        let p = arc.at_length(d);
        return Some(match a.value {
            Value::Vec2(_) => Value::Vec2([p[0], p[1]]),
            _ => Value::Vec3(p),
        });
    }
    if a.value.is_numeric() {
        let ca = a.value.components();
        let cb = b.value.components();
        let mut out = Vec::with_capacity(ca.len());
        for d in 0..ca.len() {
            let y1 = cb.get(d).copied().unwrap_or(ca[d]);
            if both_linear {
                out.push(ca[d] + (y1 - ca[d]) * (t - a.time) / (b.time - a.time));
            } else {
                out.push(ease_curve(t, a.time, b.time, ca[d], y1, side_ease(keys, i, d, false, true), side_ease(keys, i + 1, d, false, false)));
            }
        }
        return Some(a.value.with_components(&out));
    }
    match (&a.value, &b.value) {
        (Value::Path(_), Value::Path(_)) | (Value::Gradient(_), Value::Gradient(_)) => {
            let p = if both_linear {
                (t - a.time) / (b.time - a.time)
            } else {
                let dt = b.time - a.time;
                let scale = |e: Option<Ease>| e.map(|e| Ease { speed: e.speed * dt / 100.0, influence: e.influence });
                ease_curve(t, a.time, b.time, 0.0, 1.0, scale(side_ease(keys, i, 0, false, true)), scale(side_ease(keys, i + 1, 0, false, false)))
            };
            Some(a.value.lerp(&b.value, p))
        }
        _ => Some(a.value.clone()),
    }
}

pub fn insert(keys: &mut Vec<Key>, k: Key) -> usize {
    if let Some(i) = keys.iter().position(|x| same_time(x.time, k.time)) {
        let old = &keys[i];
        let mut k = k;
        k.in_interp = old.in_interp;
        k.out_interp = old.out_interp;
        k.in_ease = old.in_ease.clone();
        k.out_ease = old.out_ease.clone();
        k.auto_bezier = old.auto_bezier;
        k.continuous = old.continuous;
        k.spatial_in = old.spatial_in;
        k.spatial_out = old.spatial_out;
        k.spatial_auto = old.spatial_auto;
        k.label = old.label;
        keys[i] = k;
        return i;
    }
    let i = keys.partition_point(|x| x.time < k.time);
    keys.insert(i, k);
    i
}

pub fn sort(keys: &mut [Key]) {
    keys.sort_by(|a, b| a.time.total_cmp(&b.time));
}
