use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use rayon::prelude::*;

use super::aces::{self, OutputTransform};
use super::color::{self, M3, Prim};
use super::image::Img;
use super::lut::{self, Lut1, Lut3};
use super::yaml::{self, Y};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interp {
    Nearest,
    Linear,
    Tetrahedral,
    Best,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Neg {
    Clamp,
    Mirror,
    PassThru,
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fixed {
    Glow10,
    RedMod10,
    DarkToDim,
    GamutComp13,
    Surround(f64),
    RgbToHsv,
    XyzToXyy,
    XyzToUvy,
    XyzToLuv,
    Pq,
    HlgOetf,
    Cct,
    Cc,
    AppleLog,
    Rolloff,
}

#[derive(Clone, Debug)]
pub enum Op {
    Mat(M3, [f64; 3]),
    Pow([f64; 3], Neg),
    Moncurve { g: [f64; 3], o: [f64; 3], neg: Neg, encode: bool },
    Log { base: f64, inv: bool },
    LogAff { base: f64, ls: [f64; 3], lo: [f64; 3], ns: [f64; 3], no: [f64; 3], brk: Option<[f64; 3]>, lin: Option<[f64; 3]>, inv: bool },
    Cdl { s: [f64; 3], o: [f64; 3], p: [f64; 3], sat: f64, clamp: bool, inv: bool },
    Range { scale: f64, off: f64, lo: Option<f64>, hi: Option<f64>, lo_in: Option<f64>, hi_in: Option<f64> },
    Lut1(Arc<Lut1>, Interp, bool),
    Lut3(Arc<Lut3>, Interp, bool),
    Fixed(Fixed, bool),
    Aces(Arc<OutputTransform>, bool),
}

pub fn log_aff(base: f64, ls: [f64; 3], lo: [f64; 3], ns: [f64; 3], no: [f64; 3], brk: Option<[f64; 3]>, lin: Option<[f64; 3]>, inv: bool) -> Op {
    Op::LogAff { base, ls, lo, ns, no, brk, lin, inv }
}

pub fn range_op(min_in: Option<f64>, max_in: Option<f64>, min_out: Option<f64>, max_out: Option<f64>, clamp: bool) -> Op {
    let (scale, off) = match (min_in, max_in, min_out, max_out) {
        (Some(a), Some(b), Some(c), Some(d)) if (b - a).abs() > 1e-30 => {
            let s = (d - c) / (b - a);
            (s, c - s * a)
        }
        (Some(a), _, Some(c), _) => (1.0, c - a),
        (_, Some(b), _, Some(d)) => (1.0, d - b),
        _ => (1.0, 0.0),
    };
    if clamp { Op::Range { scale, off, lo: min_out, hi: max_out, lo_in: min_in, hi_in: max_in } } else { Op::Range { scale, off, lo: None, hi: None, lo_in: None, hi_in: None } }
}

const LUMA709: [f64; 3] = [0.2126, 0.7152, 0.0722];
const LUMA2020: [f64; 3] = [0.2627, 0.6780, 0.0593];

fn mon_slope(g: f64, o: f64) -> f64 {
    ((g - 1.0) / o) * ((o * g) / ((g - 1.0) * (1.0 + o))).powf(g)
}

fn mon_dec(x: f64, g: f64, o: f64) -> f64 {
    if o <= 0.0 || g <= 1.0 {
        return x.max(0.0).powf(g);
    }
    if x > o / (g - 1.0) { ((x + o) / (1.0 + o)).powf(g) } else { x * mon_slope(g, o) }
}

fn mon_enc(y: f64, g: f64, o: f64) -> f64 {
    if o <= 0.0 || g <= 1.0 {
        return y.max(0.0).powf(1.0 / g);
    }
    let s = mon_slope(g, o);
    let brk = (o / (g - 1.0)) * s;
    if y > brk { (1.0 + o) * y.powf(1.0 / g) - o } else { y / s }
}

fn neg_wrap(x: f64, neg: Neg, f: impl Fn(f64) -> f64) -> f64 {
    match neg {
        Neg::Mirror => f(x.abs()).copysign(x),
        Neg::PassThru if x < 0.0 => x,
        Neg::Clamp => f(x.max(0.0)),
        _ => f(x),
    }
}

fn log_b(x: f64, base: f64) -> f64 {
    x.max(f32::MIN_POSITIVE as f64).ln() / base.ln()
}

fn hsv(c: [f64; 3], inv: bool) -> [f64; 3] {
    if !inv {
        let (r, g, b) = (c[0], c[1], c[2]);
        let mx = r.max(g).max(b);
        let mn = r.min(g).min(b);
        let d = mx - mn;
        let mut h = if d <= 0.0 {
            0.0
        } else if mx == r {
            ((g - b) / d) / 6.0
        } else if mx == g {
            ((b - r) / d + 2.0) / 6.0
        } else {
            ((r - g) / d + 4.0) / 6.0
        };
        if h < 0.0 {
            h += 1.0;
        }
        let s = if mx != 0.0 { d / mx.abs() } else { 0.0 };
        return [h, s, mx];
    }
    let (h, s, v) = ((c[0] - c[0].floor()) * 6.0, c[1], c[2]);
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i as i64 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

fn xyz_fn(c: [f64; 3], kind: Fixed, inv: bool) -> [f64; 3] {
    match (kind, inv) {
        (Fixed::XyzToXyy, false) => {
            let s = c[0] + c[1] + c[2];
            if s == 0.0 { [0.0, 0.0, c[1]] } else { [c[0] / s, c[1] / s, c[1]] }
        }
        (Fixed::XyzToXyy, true) => {
            if c[1] == 0.0 { [0.0, 0.0, 0.0] } else { [c[0] * c[2] / c[1], c[2], (1.0 - c[0] - c[1]) * c[2] / c[1]] }
        }
        (Fixed::XyzToUvy, false) => {
            let d = c[0] + 15.0 * c[1] + 3.0 * c[2];
            if d == 0.0 { [0.0, 0.0, c[1]] } else { [4.0 * c[0] / d, 9.0 * c[1] / d, c[1]] }
        }
        (Fixed::XyzToUvy, true) => {
            if c[1] == 0.0 {
                [0.0, c[2], 0.0]
            } else {
                let y = c[2];
                let x = 9.0 * c[0] * y / (4.0 * c[1]);
                [x, y, (12.0 - 3.0 * c[0] - 20.0 * c[1]) * y / (4.0 * c[1])]
            }
        }
        (Fixed::XyzToLuv, false) => {
            let wn = color::white_xyz(color::D65);
            let un = 4.0 * wn[0] / (wn[0] + 15.0 * wn[1] + 3.0 * wn[2]);
            let vn = 9.0 * wn[1] / (wn[0] + 15.0 * wn[1] + 3.0 * wn[2]);
            let d = c[0] + 15.0 * c[1] + 3.0 * c[2];
            let (u, v) = if d == 0.0 { (un, vn) } else { (4.0 * c[0] / d, 9.0 * c[1] / d) };
            let yr = c[1];
            let l = if yr <= (6.0f64 / 29.0).powi(3) { yr * (29.0f64 / 3.0).powi(3) / 100.0 } else { (1.16 * yr.cbrt()) - 0.16 };
            [l, 13.0 * l * (u - un), 13.0 * l * (v - vn)]
        }
        (Fixed::XyzToLuv, true) => {
            let wn = color::white_xyz(color::D65);
            let un = 4.0 * wn[0] / (wn[0] + 15.0 * wn[1] + 3.0 * wn[2]);
            let vn = 9.0 * wn[1] / (wn[0] + 15.0 * wn[1] + 3.0 * wn[2]);
            let l = c[0];
            if l <= 0.0 {
                return [0.0; 3];
            }
            let u = c[1] / (13.0 * l) + un;
            let v = c[2] / (13.0 * l) + vn;
            let y = if l <= 0.08 { l * 100.0 * (3.0f64 / 29.0).powi(3) } else { ((l + 0.16) / 1.16).powi(3) };
            let x = y * 9.0 * u / (4.0 * v);
            [x, y, y * (12.0 - 3.0 * u - 20.0 * v) / (4.0 * v)]
        }
        _ => c,
    }
}

fn apple_log(v: f64, inv: bool) -> f64 {
    let (r0, rt, c, b, g, beta) = (-0.05641088, 0.01, 47.28711236, 0.00964052, 0.08550479, 0.69336945);
    if inv {
        if v >= rt {
            g * (v + b).log2() + beta
        } else if v >= r0 {
            c * (v - r0) * (v - r0)
        } else {
            0.0
        }
    } else {
        let yt = g * (rt + b).log2() + beta;
        if v >= yt {
            2f64.powf((v - beta) / g) - b
        } else if v >= 0.0 {
            (v / c).sqrt() + r0
        } else {
            r0
        }
    }
}

fn fixed(f: Fixed, c: [f64; 3], inv: bool) -> [f64; 3] {
    match f {
        Fixed::Glow10 => aces::glow(c, inv),
        Fixed::RedMod10 => aces::red_mod(c, inv),
        Fixed::DarkToDim => aces::dark_to_dim(c, if inv { 1.0 / 0.9811 } else { 0.9811 }, aces::mats().ap1_y),
        Fixed::GamutComp13 => aces::gamut_compress(c, inv),
        Fixed::Surround(g) => {
            let g = if inv { 1.0 / g } else { g };
            let y = (LUMA2020[0] * c[0] + LUMA2020[1] * c[1] + LUMA2020[2] * c[2]).max(1e-4);
            let k = y.powf(g - 1.0);
            [c[0] * k, c[1] * k, c[2] * k]
        }
        Fixed::RgbToHsv => hsv(c, inv),
        Fixed::XyzToXyy | Fixed::XyzToUvy | Fixed::XyzToLuv => xyz_fn(c, f, inv),
        Fixed::Pq => c.map(|v| if inv { aces::pq_decode(v) / 100.0 } else { aces::pq_encode(v * 100.0) }),
        Fixed::HlgOetf => c.map(|v| if inv { aces::hlg_oetf_inv(v.abs()).copysign(v) } else { aces::hlg_oetf(v.abs()).copysign(v) }),
        Fixed::Cct => c.map(|v| if inv { aces::lin_to_acescct(v) } else { aces::acescct_to_lin(v) }),
        Fixed::Cc => c.map(|v| if inv { aces::lin_to_acescc(v) } else { aces::acescc_to_lin(v) }),
        Fixed::AppleLog => c.map(|v| apple_log(v, inv)),
        Fixed::Rolloff => {
            let k = 0.8;
            let m = c[0].max(c[1]).max(c[2]);
            if m <= k {
                return c;
            }
            let span = 4.0 * (1.0 - k);
            let t = if inv {
                if m >= 1.0 - 1e-9 {
                    return c;
                }
                let u = (m - k) / (1.0 - k);
                k + span * u / (1.0 - u)
            } else {
                let x = (m - k) / span;
                k + (1.0 - k) * x / (1.0 + x)
            };
            let s = t / m;
            [c[0] * s, c[1] * s, c[2] * s]
        }
    }
}

fn cdl(c: [f64; 3], s: &[f64; 3], o: &[f64; 3], p: &[f64; 3], sat: f64, clamp: bool, inv: bool) -> [f64; 3] {
    let cl = |v: f64| if clamp { v.clamp(0.0, 1.0) } else { v };
    let pw = |v: f64, e: f64| if v > 0.0 || clamp { v.max(0.0).powf(e) } else { v };
    if !inv {
        let mut x = [0.0; 3];
        for k in 0..3 {
            x[k] = pw(cl(c[k] * s[k] + o[k]), p[k]);
        }
        let y = LUMA709[0] * x[0] + LUMA709[1] * x[1] + LUMA709[2] * x[2];
        return x.map(|v| cl(y + sat * (v - y)));
    }
    let x = c.map(cl);
    let y = LUMA709[0] * x[0] + LUMA709[1] * x[1] + LUMA709[2] * x[2];
    let k = if sat.abs() > 1e-12 { 1.0 / sat } else { 0.0 };
    let x = x.map(|v| cl(y + k * (v - y)));
    let mut out = [0.0; 3];
    for i in 0..3 {
        let v = pw(x[i], if p[i].abs() > 1e-12 { 1.0 / p[i] } else { 1.0 });
        out[i] = if s[i].abs() > 1e-12 { (v - o[i]) / s[i] } else { 0.0 };
    }
    out
}

fn log_aff_apply(c: [f64; 3], base: f64, ls: &[f64; 3], lo: &[f64; 3], ns: &[f64; 3], no: &[f64; 3], brk: &Option<[f64; 3]>, lin: &Option<[f64; 3]>, inv: bool) -> [f64; 3] {
    let mut out = [0.0; 3];
    for k in 0..3 {
        let x = c[k];
        let seg = brk.map(|b| {
            let b = b[k];
            let lb = ls[k] * log_b(ns[k] * b + no[k], base) + lo[k];
            let slope = lin.map(|l| l[k]).unwrap_or_else(|| ls[k] * ns[k] / ((ns[k] * b + no[k]) * base.ln()));
            (b, lb, slope, lb - slope * b)
        });
        out[k] = if !inv {
            match seg {
                Some((b, _, slope, off)) if x <= b => slope * x + off,
                _ => ls[k] * log_b(ns[k] * x + no[k], base) + lo[k],
            }
        } else {
            match seg {
                Some((_, lb, slope, off)) if x <= lb => (x - off) / slope,
                _ => (base.powf((x - lo[k]) / ls[k]) - no[k]) / ns[k],
            }
        };
    }
    out
}

impl Op {
    #[inline]
    pub fn apply(&self, c: [f64; 3]) -> [f64; 3] {
        match self {
            Op::Mat(m, o) => {
                let v = color::apply(m, c);
                [v[0] + o[0], v[1] + o[1], v[2] + o[2]]
            }
            Op::Pow(p, neg) => [neg_wrap(c[0], *neg, |x| x.powf(p[0])), neg_wrap(c[1], *neg, |x| x.powf(p[1])), neg_wrap(c[2], *neg, |x| x.powf(p[2]))],
            Op::Moncurve { g, o, neg, encode } => {
                let mut out = [0.0; 3];
                for k in 0..3 {
                    out[k] = neg_wrap(c[k], *neg, |x| if *encode { mon_enc(x, g[k], o[k]) } else { mon_dec(x, g[k], o[k]) });
                }
                out
            }
            Op::Log { base, inv } => c.map(|x| if *inv { base.powf(x) } else { log_b(x, *base) }),
            Op::LogAff { base, ls, lo, ns, no, brk, lin, inv } => log_aff_apply(c, *base, ls, lo, ns, no, brk, lin, *inv),
            Op::Cdl { s, o, p, sat, clamp, inv } => cdl(c, s, o, p, *sat, *clamp, *inv),
            Op::Range { scale, off, lo, hi, .. } => c.map(|x| {
                let mut v = x * scale + off;
                if let Some(l) = lo {
                    v = v.max(*l);
                }
                if let Some(h) = hi {
                    v = v.min(*h);
                }
                v
            }),
            Op::Lut1(l, i, inv) => {
                if *inv {
                    l.inv(c)
                } else {
                    l.eval(c, *i)
                }
            }
            Op::Lut3(l, i, inv) => {
                if *inv {
                    l.inv(c)
                } else {
                    l.eval(c, *i)
                }
            }
            Op::Fixed(f, inv) => fixed(*f, c, *inv),
            Op::Aces(a, inv) => {
                if *inv {
                    a.inv(c)
                } else {
                    a.fwd(c)
                }
            }
        }
    }

    pub fn inverse(&self) -> Op {
        match self {
            Op::Mat(m, o) => {
                let mi = color::inv(m);
                let t = color::apply(&mi, *o);
                Op::Mat(mi, [-t[0], -t[1], -t[2]])
            }
            Op::Pow(p, n) => Op::Pow(p.map(|x| if x.abs() > 1e-12 { 1.0 / x } else { 1.0 }), *n),
            Op::Moncurve { g, o, neg, encode } => Op::Moncurve { g: *g, o: *o, neg: *neg, encode: !encode },
            Op::Log { base, inv } => Op::Log { base: *base, inv: !inv },
            Op::LogAff { base, ls, lo, ns, no, brk, lin, inv } => Op::LogAff { base: *base, ls: *ls, lo: *lo, ns: *ns, no: *no, brk: *brk, lin: *lin, inv: !inv },
            Op::Cdl { s, o, p, sat, clamp, inv } => Op::Cdl { s: *s, o: *o, p: *p, sat: *sat, clamp: *clamp, inv: !inv },
            Op::Range { scale, off, lo, hi, lo_in, hi_in } => {
                let s = if scale.abs() > 1e-30 { 1.0 / scale } else { 1.0 };
                Op::Range { scale: s, off: -off * s, lo: *lo_in, hi: *hi_in, lo_in: *lo, hi_in: *hi }
            }
            Op::Lut1(l, i, inv) => Op::Lut1(l.clone(), *i, !inv),
            Op::Lut3(l, i, inv) => Op::Lut3(l.clone(), *i, !inv),
            Op::Fixed(f, inv) => Op::Fixed(*f, !inv),
            Op::Aces(a, inv) => Op::Aces(a.clone(), !inv),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Proc {
    pub ops: Vec<Op>,
}

impl Proc {
    pub fn new(ops: Vec<Op>) -> Proc {
        Proc { ops }.optimize()
    }

    pub fn is_identity(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn then(mut self, other: &Proc) -> Proc {
        self.ops.extend(other.ops.iter().cloned());
        self.optimize()
    }

    pub fn inverse(&self) -> Proc {
        Proc { ops: self.ops.iter().rev().map(Op::inverse).collect() }
    }

    fn optimize(self) -> Proc {
        let mut out: Vec<Op> = Vec::with_capacity(self.ops.len());
        for op in self.ops {
            if let Op::Mat(m, o) = &op {
                if let Some(Op::Mat(pm, po)) = out.last() {
                    let nm = color::mul(m, pm);
                    let t = color::apply(m, *po);
                    let no = [t[0] + o[0], t[1] + o[1], t[2] + o[2]];
                    out.pop();
                    out.push(Op::Mat(nm, no));
                } else {
                    out.push(op);
                }
                if let Some(Op::Mat(m, o)) = out.last()
                    && (0..3).all(|i| (0..3).all(|j| (m[i][j] - if i == j { 1.0 } else { 0.0 }).abs() < 1e-12) && o[i].abs() < 1e-12)
                {
                    out.pop();
                }
                continue;
            }
            out.push(op);
        }
        Proc { ops: out }
    }

    #[inline]
    pub fn apply(&self, c: [f64; 3]) -> [f64; 3] {
        let mut v = c;
        for op in &self.ops {
            v = op.apply(v);
        }
        v
    }

    #[inline]
    pub fn apply32(&self, c: [f32; 3]) -> [f32; 3] {
        let v = self.apply([c[0] as f64, c[1] as f64, c[2] as f64]);
        [v[0] as f32, v[1] as f32, v[2] as f32]
    }

    pub fn apply_img(&self, img: &mut Img) {
        if self.ops.is_empty() {
            return;
        }
        img.px.par_chunks_mut(4).for_each(|p| {
            let a = p[3];
            if a <= 1e-6 {
                return;
            }
            let o = self.apply32([p[0] / a, p[1] / a, p[2] / a]);
            p[0] = o[0] * a;
            p[1] = o[1] * a;
            p[2] = o[2] * a;
        });
    }

    pub fn apply_straight(&self, px: &mut [f32]) {
        if self.ops.is_empty() {
            return;
        }
        px.par_chunks_mut(4).for_each(|p| {
            let o = self.apply32([p[0], p[1], p[2]]);
            p[0] = o[0];
            p[1] = o[1];
            p[2] = o[2];
        });
    }

    pub fn bake3(&self, n: usize) -> Vec<[f32; 4]> {
        let n = n.max(2);
        let k = (n - 1) as f64;
        (0..n * n * n)
            .into_par_iter()
            .map(|i| {
                let (r, g, b) = (i % n, (i / n) % n, i / (n * n));
                let o = self.apply([r as f64 / k, g as f64 / k, b as f64 / k]);
                [o[0] as f32, o[1] as f32, o[2] as f32, 1.0]
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub enum Tr {
    Ops(Vec<Op>),
    File { src: String, cccid: String, interp: Interp, inv: bool },
    Group(Vec<Tr>, bool),
    Cs { src: String, dst: String, inv: bool },
    Dv { src: String, display: String, view: String, inv: bool },
    Look { src: String, dst: String, looks: String, inv: bool },
    Bad(String),
}

fn v3(y: Option<&Y>, def: f64) -> [f64; 3] {
    let v = y.map(Y::nums).unwrap_or_default();
    match v.len() {
        0 => [def; 3],
        1 | 2 => [v[0]; 3],
        _ => [v[0], v[1], v[2]],
    }
}

fn neg_style(s: Option<&str>, def: Neg) -> Neg {
    match s.unwrap_or("").to_ascii_lowercase().as_str() {
        "clamp" => Neg::Clamp,
        "mirror" => Neg::Mirror,
        "pass_thru" | "passthru" => Neg::PassThru,
        "linear" => Neg::Linear,
        _ => def,
    }
}

fn maybe_inv(ops: Vec<Op>, inv: bool) -> Vec<Op> {
    if inv { ops.iter().rev().map(Op::inverse).collect() } else { ops }
}

pub fn parse_tr(y: &Y) -> Result<Tr, String> {
    if let Y::Seq(items) = y.untag()
        && y.tag().is_none()
    {
        return Ok(Tr::Group(items.iter().map(parse_tr).collect::<Result<_, _>>()?, false));
    }
    let tag = y.tag().unwrap_or("");
    let inv = y.get("direction").and_then(Y::str).is_some_and(|d| d.eq_ignore_ascii_case("inverse"));
    let s = |k: &str| y.get(k).and_then(Y::str).unwrap_or("").to_string();
    let ops = match tag {
        "MatrixTransform" => {
            let m = y.get("matrix").map(Y::nums).unwrap_or_default();
            let o = y.get("offset").map(Y::nums).unwrap_or_default();
            let mm: M3 = if m.len() >= 16 { [[m[0], m[1], m[2]], [m[4], m[5], m[6]], [m[8], m[9], m[10]]] } else { color::IDENTITY };
            let oo = if o.len() >= 3 { [o[0], o[1], o[2]] } else { [0.0; 3] };
            vec![Op::Mat(mm, oo)]
        }
        "ExponentTransform" => vec![Op::Pow(v3(y.get("value"), 1.0), neg_style(y.get("style").and_then(Y::str), Neg::Clamp))],
        "ExponentWithLinearTransform" => vec![Op::Moncurve { g: v3(y.get("gamma"), 1.0), o: v3(y.get("offset"), 0.0), neg: neg_style(y.get("negative_style").and_then(Y::str), Neg::Linear), encode: false }],
        "LogTransform" => vec![Op::Log { base: y.get("base").and_then(Y::num).unwrap_or(2.0), inv: false }],
        "LogAffineTransform" | "LogCameraTransform" => {
            let base = y.get("base").and_then(Y::num).unwrap_or(2.0);
            let brk = if tag == "LogCameraTransform" { Some(v3(y.get("lin_side_break"), 0.0)) } else { None };
            let lin = y.get("linear_slope").map(|v| v3(Some(v), 1.0));
            vec![log_aff(base, v3(y.get("log_side_slope"), 1.0), v3(y.get("log_side_offset"), 0.0), v3(y.get("lin_side_slope"), 1.0), v3(y.get("lin_side_offset"), 0.0), brk, lin, false)]
        }
        "CDLTransform" => {
            let clamp = !s("style").eq_ignore_ascii_case("noclamp");
            vec![Op::Cdl { s: v3(y.get("slope"), 1.0), o: v3(y.get("offset"), 0.0), p: v3(y.get("power"), 1.0), sat: y.get("sat").and_then(Y::num).unwrap_or(1.0), clamp, inv: false }]
        }
        "RangeTransform" => {
            let f = |k: &str| y.get(k).and_then(Y::num);
            vec![range_op(f("min_in_value"), f("max_in_value"), f("min_out_value"), f("max_out_value"), !s("style").eq_ignore_ascii_case("noclamp"))]
        }
        "BuiltinTransform" => match builtin(&s("style")) {
            Ok(o) => o,
            Err(e) => return Ok(Tr::Bad(e)),
        },
        "FixedFunctionTransform" => {
            let st = s("style");
            let params = y.get("params").map(Y::nums).unwrap_or_default();
            let (f, i) = match st.to_ascii_lowercase().as_str() {
                "aces_redmod10" => (Fixed::RedMod10, false),
                "aces_glow10" => (Fixed::Glow10, false),
                "aces_darktodim10" => (Fixed::DarkToDim, false),
                "aces_gamutcomp13" => (Fixed::GamutComp13, false),
                "rec2100_surround" => (Fixed::Surround(params.first().copied().unwrap_or(1.0)), false),
                "rgb_to_hsv" => (Fixed::RgbToHsv, false),
                "xyz_to_xyy" => (Fixed::XyzToXyy, false),
                "xyz_to_uvy" => (Fixed::XyzToUvy, false),
                "xyz_to_luv" => (Fixed::XyzToLuv, false),
                "lin_to_pq" => (Fixed::Pq, false),
                _ => return Ok(Tr::Bad(format!("FixedFunctionTransform style `{st}` is not supported"))),
            };
            vec![Op::Fixed(f, i)]
        }
        "AllocationTransform" => vec![],
        "GroupTransform" => {
            let kids = y.get("children").map(Y::seq).unwrap_or(&[]);
            return Ok(Tr::Group(kids.iter().map(parse_tr).collect::<Result<_, _>>()?, inv));
        }
        "FileTransform" => {
            let interp = match s("interpolation").to_ascii_lowercase().as_str() {
                "nearest" => Interp::Nearest,
                "tetrahedral" => Interp::Tetrahedral,
                "best" => Interp::Best,
                _ => Interp::Linear,
            };
            return Ok(Tr::File { src: s("src"), cccid: s("cccid"), interp, inv });
        }
        "ColorSpaceTransform" => return Ok(Tr::Cs { src: s("src"), dst: s("dst"), inv }),
        "DisplayViewTransform" => return Ok(Tr::Dv { src: s("src"), display: s("display"), view: s("view"), inv }),
        "LookTransform" => return Ok(Tr::Look { src: s("src"), dst: s("dst"), looks: s("looks"), inv }),
        other => return Ok(Tr::Bad(format!("`{other}` transforms are not supported"))),
    };
    Ok(Tr::Ops(maybe_inv(ops, inv)))
}

fn display_ops(p: &Prim, adapt: bool, curve: Op) -> Vec<Op> {
    let m = if adapt { color::mul(&color::from_xyz(p), &color::bradford(color::D65, p.w)) } else { color::from_xyz(p) };
    vec![Op::Mat(m, [0.0; 3]), curve]
}

fn gamma_rev(g: f64, mirror: bool) -> Op {
    Op::Pow([1.0 / g; 3], if mirror { Neg::Mirror } else { Neg::Clamp })
}

fn srgb_rev(mirror: bool) -> Op {
    Op::Moncurve { g: [2.4; 3], o: [0.055; 3], neg: if mirror { Neg::Mirror } else { Neg::Linear }, encode: true }
}

pub fn builtin(style: &str) -> Result<Vec<Op>, String> {
    let st = style.trim();
    let key = st.to_ascii_uppercase();
    let mirror = key.ends_with("MIRROR NEGS");
    let base = key.trim_end_matches(" - MIRROR NEGS").to_string();
    let ap0_ap1 = aces::mats().ap0_ap1;
    let ap1_ap0 = aces::mats().ap1_ap0;
    let z = [0.0; 3];
    let ops = match base.as_str() {
        "IDENTITY" => vec![],
        "UTILITY - ACES-AP0_TO_CIE-XYZ-D65_BFD" => vec![Op::Mat(color::mul(&color::bradford(color::D60, color::D65), &color::to_xyz(&color::AP0)), z)],
        "UTILITY - ACES-AP1_TO_CIE-XYZ-D65_BFD" => vec![Op::Mat(color::mul(&color::bradford(color::D60, color::D65), &color::to_xyz(&color::AP1)), z)],
        "UTILITY - ACES-AP1_TO_LINEAR-REC709_BFD" => vec![Op::Mat(color::convert(&color::AP1, &color::REC709, true), z)],
        "ACESCG_TO_ACES2065-1" => vec![Op::Mat(ap1_ap0, z)],
        "ACESCCT_TO_ACES2065-1" => vec![Op::Fixed(Fixed::Cct, false), Op::Mat(ap1_ap0, z)],
        "ACESCC_TO_ACES2065-1" => vec![range_op(Some(-0.36), Some(1.5), Some(-0.36), Some(1.5), true), Op::Fixed(Fixed::Cc, false), Op::Mat(ap1_ap0, z), range_op(Some(0.0), None, Some(0.0), None, true)],
        "APPLE_LOG_TO_ACES2065-1" => vec![Op::Fixed(Fixed::AppleLog, false), Op::Mat(color::convert(&color::REC2020, &color::AP0, true), z)],
        "CURVE - APPLE_LOG_TO_LINEAR" => vec![Op::Fixed(Fixed::AppleLog, false)],
        "CURVE - LINEAR_TO_ST-2084" => vec![Op::Fixed(Fixed::Pq, false)],
        "ACES-LMT - ACES 1.3 REFERENCE GAMUT COMPRESSION" => vec![Op::Fixed(Fixed::GamutComp13, false)],
        "DISPLAY - CIE-XYZ-D65_TO_SRGB" => display_ops(&color::REC709, false, srgb_rev(mirror)),
        "DISPLAY - CIE-XYZ-D65_TO_DISPLAYP3" => display_ops(&color::P3D65, false, srgb_rev(true)),
        "DISPLAY - CIE-XYZ-D65_TO_REC.1886-REC.709" => display_ops(&color::REC709, false, gamma_rev(2.4, mirror)),
        "DISPLAY - CIE-XYZ-D65_TO_REC.1886-REC.2020" => display_ops(&color::REC2020, false, gamma_rev(2.4, mirror)),
        "DISPLAY - CIE-XYZ-D65_TO_G2.2-REC.709" => display_ops(&color::REC709, false, gamma_rev(2.2, mirror)),
        "DISPLAY - CIE-XYZ-D65_TO_G2.6-P3-D65" => display_ops(&color::P3D65, false, gamma_rev(2.6, mirror)),
        "DISPLAY - CIE-XYZ-D65_TO_G2.6-P3-DCI-BFD" => display_ops(&color::P3DCI, true, gamma_rev(2.6, mirror)),
        "DISPLAY - CIE-XYZ-D65_TO_G2.6-P3-D60-BFD" => display_ops(&color::P3D60, true, gamma_rev(2.6, mirror)),
        "DISPLAY - CIE-XYZ-D65_TO_REC.2100-PQ" => display_ops(&color::REC2020, false, Op::Fixed(Fixed::Pq, false)),
        "DISPLAY - CIE-XYZ-D65_TO_ST2084-P3-D65" => display_ops(&color::P3D65, false, Op::Fixed(Fixed::Pq, false)),
        "DISPLAY - CIE-XYZ-D65_TO_REC.2100-HLG-1000NIT" => {
            let m = color::from_xyz(&color::REC2020).map(|r| r.map(|v| v * 0.1));
            vec![Op::Mat(m, z), Op::Fixed(Fixed::Surround(1.0 / 1.2), false), Op::Fixed(Fixed::HlgOetf, false)]
        }
        _ if base.starts_with("ACES-OUTPUT - ACES2065-1_TO_CIE-XYZ-D65 - ") => {
            let rest = &base["ACES-OUTPUT - ACES2065-1_TO_CIE-XYZ-D65 - ".len()..];
            let ot = match rest {
                "SDR-VIDEO_1.0" => OutputTransform::sdr(true, None),
                "SDR-VIDEO-REC709LIM_1.1" => OutputTransform::sdr(true, Some(color::REC709)),
                "SDR-VIDEO-P3LIM_1.1" => OutputTransform::sdr(true, Some(color::P3D65)),
                "SDR-CINEMA_1.0" => OutputTransform::sdr(false, None),
                "SDR-CINEMA-REC709LIM_1.1" => OutputTransform::sdr(false, Some(color::REC709)),
                _ => {
                    let parts: Vec<&str> = rest.split('-').collect();
                    let nits = |s: &str| s.trim_end_matches("NIT").parse::<f64>().ok();
                    match parts.as_slice() {
                        ["HDR", kind, max, mid, lim] if (*kind == "VIDEO" || *kind == "CINEMA") => {
                            let (Some(max), Some(mid)) = (nits(max), nits(mid)) else { return Err(format!("unknown BuiltinTransform `{st}`")) };
                            let limit = if lim.starts_with("REC2020") { color::REC2020 } else if lim.starts_with("P3") { color::P3D65 } else { color::REC709 };
                            OutputTransform::hdr(0.0001, mid, max, limit)
                        }
                        _ => return Err(format!("unknown BuiltinTransform `{st}`")),
                    }
                }
            };
            vec![Op::Aces(Arc::new(ot), false)]
        }
        _ => {
            let _ = ap0_ap1;
            return Err(format!("unknown BuiltinTransform `{st}`"));
        }
    };
    Ok(ops)
}

#[derive(Clone, Debug)]
pub struct Space {
    pub name: String,
    pub aliases: Vec<String>,
    pub family: String,
    pub description: String,
    pub data: bool,
    pub encoding: String,
    pub categories: Vec<String>,
    pub display: bool,
    pub to_ref: Option<Tr>,
    pub from_ref: Option<Tr>,
}

#[derive(Clone, Debug)]
pub struct View {
    pub name: String,
    pub cs: Option<String>,
    pub vt: Option<String>,
    pub dcs: Option<String>,
    pub looks: String,
}

#[derive(Clone, Debug)]
pub struct ViewTransform {
    pub name: String,
    pub to_scene: Option<Tr>,
    pub from_scene: Option<Tr>,
    pub to_display: Option<Tr>,
    pub from_display: Option<Tr>,
}

#[derive(Clone, Debug)]
pub struct LookDef {
    pub name: String,
    pub space: String,
    pub tr: Option<Tr>,
    pub inv_tr: Option<Tr>,
}

#[derive(Debug, Default)]
pub struct Config {
    pub name: String,
    pub version: f64,
    pub dir: PathBuf,
    pub search: Vec<String>,
    pub env: Vec<(String, String)>,
    pub roles: Vec<(String, String)>,
    pub spaces: Vec<Space>,
    pub displays: Vec<(String, Vec<View>)>,
    pub vts: Vec<ViewTransform>,
    pub looks: Vec<LookDef>,
    pub active_displays: Vec<String>,
    pub active_views: Vec<String>,
    pub default_vt: Option<String>,
    pub inactive: Vec<String>,
    cache: Mutex<HashMap<String, Arc<Proc>>>,
    pub serial: u64,
}

fn opt_tr(y: Option<&Y>) -> Result<Option<Tr>, String> {
    match y {
        Some(Y::Null) | None => Ok(None),
        Some(v) => parse_tr(v).map(Some),
    }
}

fn parse_view(y: &Y) -> View {
    let s = |k: &str| y.get(k).and_then(Y::str).map(str::to_string);
    View { name: s("name").unwrap_or_default(), cs: s("colorspace"), vt: s("view_transform"), dcs: s("display_colorspace"), looks: s("looks").unwrap_or_default() }
}

fn parse_space(y: &Y, display: bool) -> Result<Space, String> {
    let s = |k: &str| y.get(k).and_then(Y::str).unwrap_or("").to_string();
    let name = s("name");
    let to = if display { opt_tr(y.get("to_display_reference"))? } else { opt_tr(y.get("to_scene_reference").or_else(|| y.get("to_reference")))? };
    let from = if display { opt_tr(y.get("from_display_reference"))? } else { opt_tr(y.get("from_scene_reference").or_else(|| y.get("from_reference")))? };
    Ok(Space {
        aliases: y.get("aliases").map(Y::strs).unwrap_or_default(),
        family: s("family"),
        description: s("description").trim().to_string(),
        data: y.get("isdata").and_then(Y::bool).unwrap_or(false),
        encoding: s("encoding"),
        categories: y.get("categories").map(Y::strs).unwrap_or_default(),
        display,
        to_ref: to,
        from_ref: from,
        name,
    })
}

impl Config {
    pub fn parse(text: &str, dir: &Path) -> Result<Config, String> {
        let y = yaml::parse(text)?;
        let version = y.get("ocio_profile_version").and_then(Y::num).unwrap_or(1.0);
        let mut c = Config { name: y.get("name").and_then(Y::str).unwrap_or("").to_string(), version, dir: dir.to_path_buf(), ..Config::default() };
        c.search = match y.get("search_path") {
            Some(v) if !v.seq().is_empty() => v.strs(),
            Some(v) => v.str().unwrap_or("").split(':').map(str::to_string).filter(|s| !s.is_empty()).collect(),
            None => vec![],
        };
        if c.search.is_empty() {
            c.search.push(".".into());
        }
        c.env = y.get("environment").map(|e| e.map().iter().map(|(k, v)| (k.clone(), v.str().unwrap_or("").to_string())).collect()).unwrap_or_default();
        c.roles = y.get("roles").map(|r| r.map().iter().map(|(k, v)| (k.clone(), v.str().unwrap_or("").to_string())).collect()).unwrap_or_default();
        for cs in y.get("colorspaces").map(Y::seq).unwrap_or(&[]) {
            c.spaces.push(parse_space(cs, false)?);
        }
        for cs in y.get("display_colorspaces").map(Y::seq).unwrap_or(&[]) {
            c.spaces.push(parse_space(cs, true)?);
        }
        let shared: Vec<View> = y.get("shared_views").map(Y::seq).unwrap_or(&[]).iter().map(parse_view).collect();
        for (dname, views) in y.get("displays").map(Y::map).unwrap_or(&[]) {
            let mut list = vec![];
            for v in views.seq() {
                if v.tag() == Some("Views") {
                    for n in v.strs() {
                        if let Some(sv) = shared.iter().find(|s| s.name == n) {
                            let mut sv = sv.clone();
                            if sv.dcs.as_deref() == Some("<USE_DISPLAY_NAME>") {
                                sv.dcs = Some(dname.clone());
                            }
                            list.push(sv);
                        }
                    }
                } else {
                    let mut pv = parse_view(v);
                    if pv.dcs.as_deref() == Some("<USE_DISPLAY_NAME>") {
                        pv.dcs = Some(dname.clone());
                    }
                    list.push(pv);
                }
            }
            c.displays.push((dname.clone(), list));
        }
        for vt in y.get("view_transforms").map(Y::seq).unwrap_or(&[]) {
            c.vts.push(ViewTransform {
                name: vt.get("name").and_then(Y::str).unwrap_or("").to_string(),
                to_scene: opt_tr(vt.get("to_scene_reference").or_else(|| vt.get("to_reference")))?,
                from_scene: opt_tr(vt.get("from_scene_reference").or_else(|| vt.get("from_reference")))?,
                to_display: opt_tr(vt.get("to_display_reference"))?,
                from_display: opt_tr(vt.get("from_display_reference"))?,
            });
        }
        for lk in y.get("looks").map(Y::seq).unwrap_or(&[]) {
            c.looks.push(LookDef { name: lk.get("name").and_then(Y::str).unwrap_or("").to_string(), space: lk.get("process_space").and_then(Y::str).unwrap_or("").to_string(), tr: opt_tr(lk.get("transform"))?, inv_tr: opt_tr(lk.get("inverse_transform"))? });
        }
        c.active_displays = y.get("active_displays").map(Y::strs).unwrap_or_default();
        c.active_views = y.get("active_views").map(Y::strs).unwrap_or_default();
        c.inactive = y.get("inactive_colorspaces").map(Y::strs).unwrap_or_default();
        c.default_vt = y.get("default_view_transform").and_then(Y::str).map(str::to_string).or_else(|| c.vts.first().map(|v| v.name.clone()));
        if c.spaces.is_empty() {
            return Err("the config has no colour spaces".into());
        }
        Ok(c)
    }

    pub fn role(&self, r: &str) -> Option<&str> {
        self.roles.iter().find(|(k, _)| k.eq_ignore_ascii_case(r)).map(|(_, v)| v.as_str())
    }

    pub fn space(&self, name: &str) -> Option<&Space> {
        let n = name.trim();
        let by = |n: &str| self.spaces.iter().find(|s| s.name.eq_ignore_ascii_case(n) || s.aliases.iter().any(|a| a.eq_ignore_ascii_case(n)));
        by(n).or_else(|| self.role(n).and_then(by))
    }

    pub fn space_names(&self, include_display: bool) -> Vec<String> {
        self.spaces.iter().filter(|s| include_display || !s.display).filter(|s| !self.inactive.iter().any(|i| i == &s.name)).map(|s| s.name.clone()).collect()
    }

    pub fn display_names(&self) -> Vec<String> {
        let all: Vec<String> = self.displays.iter().map(|d| d.0.clone()).collect();
        if self.active_displays.is_empty() {
            return all;
        }
        let mut out: Vec<String> = self.active_displays.iter().filter(|d| all.contains(d)).cloned().collect();
        if out.is_empty() {
            out = all;
        }
        out
    }

    pub fn views(&self, display: &str) -> Vec<String> {
        let Some((_, vs)) = self.displays.iter().find(|d| d.0.eq_ignore_ascii_case(display)) else { return vec![] };
        let mut out: Vec<String> = vs.iter().map(|v| v.name.clone()).collect();
        if !self.active_views.is_empty() {
            let ordered: Vec<String> = self.active_views.iter().filter(|a| out.contains(a)).cloned().collect();
            if !ordered.is_empty() {
                out = ordered;
            }
        }
        out
    }

    pub fn look_names(&self) -> Vec<String> {
        self.looks.iter().map(|l| l.name.clone()).collect()
    }

    fn expand(&self, s: &str) -> String {
        let mut out = s.to_string();
        for _ in 0..4 {
            let Some(at) = out.find('$') else { break };
            let rest = &out[at + 1..];
            let (name, len) = if let Some(r) = rest.strip_prefix('{') {
                let (e, close) = match r.find('}') {
                    Some(e) => (e, 1),
                    None => (r.len(), 0),
                };
                (r[..e].to_string(), e + 1 + close)
            } else {
                let e = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
                (rest[..e].to_string(), e)
            };
            let val = std::env::var(&name).ok().or_else(|| self.env.iter().find(|(k, _)| *k == name).map(|(_, v)| v.clone())).unwrap_or_default();
            out = format!("{}{}{}", &out[..at], val, &out[at + 1 + len..]);
        }
        out
    }

    fn find_file(&self, src: &str) -> Result<String, String> {
        let s = self.expand(src);
        let p = Path::new(&s);
        if p.is_absolute() {
            return if p.exists() { Ok(s) } else { Err(format!("the LUT `{s}` was not found")) };
        }
        for sp in &self.search {
            let d = self.expand(sp);
            let dir = if Path::new(&d).is_absolute() { PathBuf::from(&d) } else { self.dir.join(&d) };
            let f = dir.join(&s);
            if f.exists() {
                return Ok(f.to_string_lossy().to_string());
            }
        }
        Err(format!("the LUT `{s}` was not found on the config's search path"))
    }

    fn compile(&self, t: &Tr, depth: u32) -> Result<Proc, String> {
        if depth > 24 {
            return Err("the config's transforms nest too deeply".into());
        }
        Ok(match t {
            Tr::Ops(o) => Proc { ops: o.clone() },
            Tr::Group(kids, inv) => {
                let mut p = Proc::default();
                for k in kids {
                    p = p.then(&self.compile(k, depth + 1)?);
                }
                if *inv { p.inverse() } else { p }
            }
            Tr::File { src, cccid, interp, inv } => {
                let path = self.find_file(src)?;
                let ops: Vec<Op> = lut::load(&path, cccid)?
                    .iter()
                    .map(|o| match o {
                        Op::Lut1(l, _, i) => Op::Lut1(l.clone(), *interp, *i),
                        Op::Lut3(l, _, i) => Op::Lut3(l.clone(), *interp, *i),
                        x => x.clone(),
                    })
                    .collect();
                let p = Proc { ops };
                if *inv { p.inverse() } else { p }
            }
            Tr::Cs { src, dst, inv } => {
                let p = self.convert_d(src, dst, depth + 1)?;
                if *inv { p.inverse() } else { p }
            }
            Tr::Dv { src, display, view, inv } => self.display_view_d(src, display, view, None, *inv, depth + 1)?,
            Tr::Look { src, dst, looks, inv } => self.look_d(src, dst, looks, *inv, depth + 1)?,
            Tr::Bad(e) => return Err(e.clone()),
        })
    }

    fn to_ref(&self, s: &Space, depth: u32) -> Result<Proc, String> {
        match (&s.to_ref, &s.from_ref) {
            (Some(t), _) => self.compile(t, depth),
            (None, Some(f)) => Ok(self.compile(f, depth)?.inverse()),
            _ => Ok(Proc::default()),
        }
    }

    fn from_ref(&self, s: &Space, depth: u32) -> Result<Proc, String> {
        match (&s.from_ref, &s.to_ref) {
            (Some(f), _) => self.compile(f, depth),
            (None, Some(t)) => Ok(self.compile(t, depth)?.inverse()),
            _ => Ok(Proc::default()),
        }
    }

    fn vt_scene_to_display(&self, vt: &ViewTransform, depth: u32) -> Result<Proc, String> {
        match (&vt.from_scene, &vt.to_scene) {
            (Some(f), _) => self.compile(f, depth),
            (None, Some(t)) => Ok(self.compile(t, depth)?.inverse()),
            _ => match (&vt.to_display, &vt.from_display) {
                (Some(t), _) => self.compile(t, depth),
                (None, Some(f)) => Ok(self.compile(f, depth)?.inverse()),
                _ => Ok(Proc::default()),
            },
        }
    }

    fn default_vt(&self) -> Option<&ViewTransform> {
        let n = self.default_vt.as_deref()?;
        self.vts.iter().find(|v| v.name == n)
    }

    fn cached(&self, key: String, f: impl FnOnce() -> Result<Proc, String>) -> Result<Arc<Proc>, String> {
        if let Some(p) = self.cache.lock().get(&key) {
            return Ok(p.clone());
        }
        let p = Arc::new(f()?);
        self.cache.lock().insert(key, p.clone());
        Ok(p)
    }

    fn convert_d(&self, src: &str, dst: &str, depth: u32) -> Result<Proc, String> {
        let a = self.space(src).ok_or_else(|| format!("no colour space `{src}` in the config"))?;
        let b = self.space(dst).ok_or_else(|| format!("no colour space `{dst}` in the config"))?;
        if a.data || b.data || a.name == b.name {
            return Ok(Proc::default());
        }
        let mut p = self.to_ref(a, depth)?;
        if a.display != b.display {
            let vt = self.default_vt().ok_or("the config has no default view transform to cross between scene and display spaces")?;
            let s2d = self.vt_scene_to_display(vt, depth)?;
            p = p.then(&if a.display { s2d.inverse() } else { s2d });
        }
        Ok(p.then(&self.from_ref(b, depth)?))
    }

    pub fn convert(&self, src: &str, dst: &str) -> Result<Arc<Proc>, String> {
        self.cached(format!("cs\u{1}{src}\u{1}{dst}"), || self.convert_d(src, dst, 0))
    }

    fn looks_d(&self, src: &str, looks: &str, depth: u32) -> Result<(Proc, String), String> {
        let mut p = Proc::default();
        let mut cur = src.to_string();
        for item in looks.split([',', ':']).map(str::trim).filter(|s| !s.is_empty()) {
            let first = item.split('|').next().unwrap_or(item).trim();
            let (inv, name) = match first.strip_prefix('-') {
                Some(n) => (true, n.trim()),
                None => (false, first.trim_start_matches('+').trim()),
            };
            let ld = self.looks.iter().find(|l| l.name.eq_ignore_ascii_case(name)).ok_or_else(|| format!("no look `{name}` in the config"))?;
            p = p.then(&self.convert_d(&cur, &ld.space, depth + 1)?);
            cur = ld.space.clone();
            let fwd = match (&ld.tr, &ld.inv_tr) {
                (Some(t), _) => self.compile(t, depth + 1)?,
                (None, Some(t)) => self.compile(t, depth + 1)?.inverse(),
                _ => Proc::default(),
            };
            p = p.then(&if inv { fwd.inverse() } else { fwd });
        }
        Ok((p, cur))
    }

    fn look_d(&self, src: &str, dst: &str, looks: &str, inverse: bool, depth: u32) -> Result<Proc, String> {
        let (p, cur) = self.looks_d(src, looks, depth)?;
        let p = p.then(&self.convert_d(&cur, dst, depth + 1)?);
        Ok(if inverse { p.inverse() } else { p })
    }

    pub fn look(&self, src: &str, dst: &str, looks: &str, inverse: bool) -> Result<Arc<Proc>, String> {
        self.cached(format!("lk\u{1}{src}\u{1}{dst}\u{1}{looks}\u{1}{inverse}"), || self.look_d(src, dst, looks, inverse, 0))
    }

    fn display_view_d(&self, src: &str, display: &str, view: &str, looks: Option<&str>, inverse: bool, depth: u32) -> Result<Proc, String> {
        let (dname, vs) = self.displays.iter().find(|d| d.0.eq_ignore_ascii_case(display)).ok_or_else(|| format!("no display `{display}` in the config"))?;
        let v = if view.is_empty() { vs.first() } else { vs.iter().find(|v| v.name.eq_ignore_ascii_case(view)) }.ok_or_else(|| format!("no view `{view}` for `{dname}`"))?;
        let look_str = looks.unwrap_or(&v.looks).to_string();
        let (mut p, cur) = self.looks_d(src, &look_str, depth)?;
        if let Some(vt_name) = &v.vt {
            let vt = self.vts.iter().find(|t| &t.name == vt_name).ok_or_else(|| format!("no view transform `{vt_name}` in the config"))?;
            let a = self.space(&cur).ok_or_else(|| format!("no colour space `{cur}` in the config"))?;
            if !a.data {
                let dcs_name = v.dcs.clone().unwrap_or_else(|| dname.clone());
                let b = self.space(&dcs_name).ok_or_else(|| format!("no display colour space `{dcs_name}` in the config"))?;
                let mut to_scene = self.to_ref(a, depth)?;
                if a.display {
                    to_scene = to_scene.then(&self.vt_scene_to_display(self.default_vt().ok_or("no default view transform")?, depth)?.inverse());
                }
                p = p.then(&to_scene).then(&self.vt_scene_to_display(vt, depth)?).then(&self.from_ref(b, depth)?);
            }
        } else {
            let cs = v.cs.clone().ok_or_else(|| format!("the view `{}` names no colour space", v.name))?;
            p = p.then(&self.convert_d(&cur, &cs, depth + 1)?);
        }
        Ok(if inverse { p.inverse() } else { p })
    }

    pub fn display_view(&self, src: &str, display: &str, view: &str, looks: Option<&str>, inverse: bool) -> Result<Arc<Proc>, String> {
        self.cached(format!("dv\u{1}{src}\u{1}{display}\u{1}{view}\u{1}{looks:?}\u{1}{inverse}"), || self.display_view_d(src, display, view, looks, inverse, 0))
    }

    pub fn is_display_space(&self, name: &str) -> bool {
        self.space(name).is_some_and(|s| s.display)
    }
}

fn sp(name: &str, aliases: &[&str], family: &str, encoding: &str, to_ref: Vec<Op>) -> Space {
    Space { name: name.into(), aliases: aliases.iter().map(|s| s.to_string()).collect(), family: family.into(), description: String::new(), data: false, encoding: encoding.into(), categories: vec![], display: false, to_ref: Some(Tr::Ops(to_ref)), from_ref: None }
}

fn dsp(name: &str, aliases: &[&str], style: &str) -> Space {
    Space { name: name.into(), aliases: aliases.iter().map(|s| s.to_string()).collect(), family: "Display".into(), description: String::new(), data: false, encoding: "sdr-video".into(), categories: vec![], display: true, to_ref: None, from_ref: Some(Tr::Ops(builtin(style).unwrap_or_default())) }
}

pub const LOOKS: [(&str, [f64; 3], [f64; 3], [f64; 3], f64); 6] = [
    ("Warm", [1.06, 1.0, 0.92], [0.01, 0.0, -0.01], [1.0, 1.0, 1.0], 1.0),
    ("Cool", [0.94, 1.0, 1.08], [-0.01, 0.0, 0.01], [1.0, 1.0, 1.0], 1.0),
    ("High Contrast", [1.25, 1.25, 1.25], [-0.12, -0.12, -0.12], [1.0, 1.0, 1.0], 1.05),
    ("Low Contrast", [0.8, 0.8, 0.8], [0.1, 0.1, 0.1], [1.0, 1.0, 1.0], 0.95),
    ("Desaturated", [1.0, 1.0, 1.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], 0.6),
    ("Vivid", [1.05, 1.05, 1.05], [-0.02, -0.02, -0.02], [1.0, 1.0, 1.0], 1.35),
];

fn builtin_config() -> Config {
    let to_ap0 = |p: &Prim| color::convert(p, &color::AP0, true);
    let z = [0.0; 3];
    let srgb = Op::Moncurve { g: [2.4; 3], o: [0.055; 3], neg: Neg::Linear, encode: false };
    let g = |v: f64| Op::Pow([v; 3], Neg::PassThru);
    let spaces = vec![
        sp("ACES2065-1", &["aces2065_1", "aces", "lin_ap0", "aces2065"], "ACES", "scene-linear", vec![]),
        sp("ACEScg", &["lin_ap1", "acescg"], "ACES", "scene-linear", vec![Op::Mat(aces::mats().ap1_ap0, z)]),
        sp("ACEScct", &["acescct", "acescct_ap1"], "ACES", "log", builtin("ACEScct_to_ACES2065-1").unwrap_or_default()),
        sp("ACEScc", &["acescc", "acescc_ap1"], "ACES", "log", builtin("ACEScc_to_ACES2065-1").unwrap_or_default()),
        sp("Linear Rec.709 (sRGB)", &["lin_rec709", "lin_srgb", "linearSrgb", "Utility - Linear - sRGB"], "Utility", "scene-linear", vec![Op::Mat(to_ap0(&color::REC709), z)]),
        sp("sRGB Encoded Rec.709 (sRGB)", &["srgb_tx", "srgb_texture", "sRGB - Texture", "Utility - sRGB - Texture", "srgb"], "Utility", "sdr-video", vec![srgb.clone(), Op::Mat(to_ap0(&color::REC709), z)]),
        sp("Gamma 1.8 Encoded Rec.709", &["g18_rec709"], "Utility", "sdr-video", vec![g(1.8), Op::Mat(to_ap0(&color::REC709), z)]),
        sp("Gamma 2.2 Encoded Rec.709", &["g22_rec709"], "Utility", "sdr-video", vec![g(2.2), Op::Mat(to_ap0(&color::REC709), z)]),
        sp("Gamma 2.4 Encoded Rec.709", &["g24_rec709", "rec709", "rec709_display"], "Utility", "sdr-video", vec![g(2.4), Op::Mat(to_ap0(&color::REC709), z)]),
        sp("Linear Rec.2020", &["lin_rec2020"], "Utility", "scene-linear", vec![Op::Mat(to_ap0(&color::REC2020), z)]),
        sp("Gamma 2.4 Encoded Rec.2020", &["g24_rec2020", "rec2020"], "Utility", "sdr-video", vec![g(2.4), Op::Mat(to_ap0(&color::REC2020), z)]),
        sp("Linear P3-D65", &["lin_p3d65", "lin_displayp3"], "Utility", "scene-linear", vec![Op::Mat(to_ap0(&color::P3D65), z)]),
        sp("sRGB Encoded P3-D65", &["srgb_p3d65", "srgb_displayp3", "p3"], "Utility", "sdr-video", vec![srgb.clone(), Op::Mat(to_ap0(&color::P3D65), z)]),
        sp("Linear opRGB", &["lin_oprgb"], "Utility", "scene-linear", vec![Op::Mat(to_ap0(&color::OPRGB), z)]),
        sp("Gamma 2.2 Encoded opRGB", &["g22_oprgb", "oprgb"], "Utility", "sdr-video", vec![g(2.19921875), Op::Mat(to_ap0(&color::OPRGB), z)]),
        sp("CIE XYZ-D65 - Scene-referred", &["lin_ciexyzd65_scene"], "Utility", "scene-linear", vec![Op::Mat(color::mul(&color::from_xyz(&color::AP0), &color::bradford(color::D65, color::D60)), z)]),
        Space { name: "Raw".into(), aliases: vec!["raw".into(), "data".into()], family: "Utility".into(), description: String::new(), data: true, encoding: String::new(), categories: vec![], display: false, to_ref: None, from_ref: None },
        Space { name: "CIE XYZ-D65 - Display-referred".into(), aliases: vec!["cie_xyz_d65_display".into()], family: String::new(), description: String::new(), data: false, encoding: "display-linear".into(), categories: vec![], display: true, to_ref: None, from_ref: None },
        dsp("sRGB - Display", &["srgb_display"], "DISPLAY - CIE-XYZ-D65_to_sRGB"),
        dsp("Display P3 - Display", &["displayp3_display"], "DISPLAY - CIE-XYZ-D65_to_DisplayP3"),
        dsp("Rec.1886 Rec.709 - Display", &["rec1886_rec709_display"], "DISPLAY - CIE-XYZ-D65_to_REC.1886-REC.709"),
        dsp("Rec.1886 Rec.2020 - Display", &["rec1886_rec2020_display"], "DISPLAY - CIE-XYZ-D65_to_REC.1886-REC.2020"),
        dsp("Rec.2100-PQ - Display", &["rec2100_pq_display"], "DISPLAY - CIE-XYZ-D65_to_REC.2100-PQ"),
        dsp("Rec.2100-HLG - Display", &["rec2100_hlg_display"], "DISPLAY - CIE-XYZ-D65_to_REC.2100-HLG-1000nit"),
        dsp("P3-D65 - Display", &["p3d65_display"], "DISPLAY - CIE-XYZ-D65_to_G2.6-P3-D65"),
    ];
    let vt = |name: &str, style: &str| ViewTransform { name: name.into(), to_scene: None, from_scene: Some(Tr::Ops(builtin(style).unwrap_or_default())), to_display: None, from_display: None };
    let vts = vec![
        vt("ACES 1.0 - SDR Video", "ACES-OUTPUT - ACES2065-1_to_CIE-XYZ-D65 - SDR-VIDEO_1.0"),
        vt("ACES 1.1 - HDR Video (1000 nits & Rec.2020 lim)", "ACES-OUTPUT - ACES2065-1_to_CIE-XYZ-D65 - HDR-VIDEO-1000nit-15nit-REC2020lim_1.1"),
        vt("ACES 1.1 - HDR Video (1000 nits & P3 lim)", "ACES-OUTPUT - ACES2065-1_to_CIE-XYZ-D65 - HDR-VIDEO-1000nit-15nit-P3lim_1.1"),
        vt("ACES 1.0 - SDR Cinema", "ACES-OUTPUT - ACES2065-1_to_CIE-XYZ-D65 - SDR-CINEMA_1.0"),
        vt("Un-tone-mapped", "UTILITY - ACES-AP0_to_CIE-XYZ-D65_BFD"),
    ];
    let v = |name: &str, vt: &str, d: &str| View { name: name.into(), cs: None, vt: Some(vt.into()), dcs: Some(d.into()), looks: String::new() };
    let raw = || View { name: "Raw".into(), cs: Some("Raw".into()), vt: None, dcs: None, looks: String::new() };
    let sdr = |d: &str| vec![v("ACES 1.0 - SDR Video", "ACES 1.0 - SDR Video", d), v("Un-tone-mapped", "Un-tone-mapped", d), raw()];
    let hdr = |d: &str, vt: &str| vec![v(vt, vt, d), v("Un-tone-mapped", "Un-tone-mapped", d), raw()];
    let displays = vec![
        ("sRGB - Display".to_string(), sdr("sRGB - Display")),
        ("Display P3 - Display".to_string(), sdr("Display P3 - Display")),
        ("Rec.1886 Rec.709 - Display".to_string(), sdr("Rec.1886 Rec.709 - Display")),
        ("Rec.1886 Rec.2020 - Display".to_string(), sdr("Rec.1886 Rec.2020 - Display")),
        ("Rec.2100-PQ - Display".to_string(), hdr("Rec.2100-PQ - Display", "ACES 1.1 - HDR Video (1000 nits & Rec.2020 lim)")),
        ("Rec.2100-HLG - Display".to_string(), hdr("Rec.2100-HLG - Display", "ACES 1.1 - HDR Video (1000 nits & Rec.2020 lim)")),
        ("P3-D65 - Display".to_string(), vec![v("ACES 1.0 - SDR Cinema", "ACES 1.0 - SDR Cinema", "P3-D65 - Display"), v("Un-tone-mapped", "Un-tone-mapped", "P3-D65 - Display"), raw()]),
    ];
    let mut looks = vec![LookDef { name: "ACES 1.3 Reference Gamut Compression".into(), space: "ACES2065-1".into(), tr: Some(Tr::Ops(vec![Op::Fixed(Fixed::GamutComp13, false)])), inv_tr: None }];
    for (name, s, o, p, sat) in LOOKS {
        looks.push(LookDef { name: name.into(), space: "ACEScct".into(), tr: Some(Tr::Ops(vec![Op::Cdl { s, o, p, sat, clamp: false, inv: false }])), inv_tr: None });
    }
    let roles = [
        ("aces_interchange", "ACES2065-1"),
        ("cie_xyz_d65_interchange", "CIE XYZ-D65 - Display-referred"),
        ("color_picking", "sRGB Encoded Rec.709 (sRGB)"),
        ("color_timing", "ACEScct"),
        ("compositing_log", "ACEScct"),
        ("data", "Raw"),
        ("default", "sRGB Encoded Rec.709 (sRGB)"),
        ("matte_paint", "ACEScct"),
        ("scene_linear", "ACEScg"),
        ("texture_paint", "sRGB Encoded Rec.709 (sRGB)"),
    ];
    Config {
        name: "formvfx ACES 1.3".into(),
        version: 2.4,
        search: vec![".".into()],
        roles: roles.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
        spaces,
        displays,
        vts,
        looks,
        default_vt: Some("Un-tone-mapped".into()),
        ..Config::default()
    }
}

pub fn builtin_cfg() -> Arc<Config> {
    static C: OnceLock<Arc<Config>> = OnceLock::new();
    C.get_or_init(|| Arc::new(builtin_config())).clone()
}

type CfgCache = Mutex<HashMap<String, (Option<std::time::SystemTime>, Arc<Config>)>>;

pub fn load(path: &str) -> Result<Arc<Config>, String> {
    let path = path.trim();
    if path.is_empty() || path.eq_ignore_ascii_case("builtin") {
        return Ok(builtin_cfg());
    }
    static C: OnceLock<CfgCache> = OnceLock::new();
    let cache = C.get_or_init(|| Mutex::new(HashMap::new()));
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    if let Some((t, c)) = cache.lock().get(path)
        && *t == mtime
    {
        return Ok(c.clone());
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let dir = Path::new(path).parent().map(Path::to_path_buf).unwrap_or_default();
    static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let mut cfg = Config::parse(&text, &dir).map_err(|e| format!("{path}: {e}"))?;
    cfg.serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cfg = Arc::new(cfg);
    cache.lock().insert(path.to_string(), (mtime, cfg.clone()));
    Ok(cfg)
}
