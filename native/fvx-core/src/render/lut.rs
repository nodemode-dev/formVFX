use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

use super::color::M3;
use super::ocio::{Interp, Op};
use crate::engine::xmldoc::{self, El};

#[derive(Debug)]
pub struct Lut1 {
    pub data: Vec<[f32; 3]>,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Debug)]
pub struct Lut3 {
    pub n: usize,
    pub data: Vec<[f32; 3]>,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Lut1 {
    pub fn eval(&self, c: [f64; 3], interp: Interp) -> [f64; 3] {
        let n = self.data.len();
        let mut o = [0.0; 3];
        if n == 0 {
            return c;
        }
        for k in 0..3 {
            let span = (self.max[k] - self.min[k]) as f64;
            let t = if span.abs() < 1e-12 { 0.0 } else { ((c[k] - self.min[k] as f64) / span).clamp(0.0, 1.0) * (n - 1) as f64 };
            if interp == Interp::Nearest {
                o[k] = self.data[(t.round() as usize).min(n - 1)][k] as f64;
                continue;
            }
            let i = (t.floor() as usize).min(n.saturating_sub(2));
            let f = t - i as f64;
            let a = self.data[i][k] as f64;
            let b = self.data[(i + 1).min(n - 1)][k] as f64;
            o[k] = a + (b - a) * f;
        }
        o
    }

    pub fn inv(&self, c: [f64; 3]) -> [f64; 3] {
        let n = self.data.len();
        let mut o = [0.0; 3];
        if n < 2 {
            return c;
        }
        for k in 0..3 {
            let up = self.data[n - 1][k] >= self.data[0][k];
            let v = c[k];
            let at = |i: usize| self.data[i][k] as f64;
            let (lo, hi) = if up { (at(0), at(n - 1)) } else { (at(n - 1), at(0)) };
            let x = if v <= lo {
                if up { 0.0 } else { (n - 1) as f64 }
            } else if v >= hi {
                if up { (n - 1) as f64 } else { 0.0 }
            } else {
                let (mut a, mut b) = (0usize, n - 1);
                while b - a > 1 {
                    let m = (a + b) / 2;
                    if (at(m) <= v) == up {
                        a = m;
                    } else {
                        b = m;
                    }
                }
                let (va, vb) = (at(a), at(b));
                a as f64 + if (vb - va).abs() < 1e-30 { 0.0 } else { (v - va) / (vb - va) }
            };
            o[k] = self.min[k] as f64 + x / (n - 1) as f64 * (self.max[k] - self.min[k]) as f64;
        }
        o
    }
}

impl Lut3 {
    #[inline]
    fn at(&self, r: usize, g: usize, b: usize) -> [f64; 3] {
        let v = self.data[(b * self.n + g) * self.n + r];
        [v[0] as f64, v[1] as f64, v[2] as f64]
    }

    pub fn eval(&self, c: [f64; 3], interp: Interp) -> [f64; 3] {
        let n = self.n;
        if n < 2 {
            return c;
        }
        let mut idx = [0usize; 3];
        let mut f = [0.0f64; 3];
        for k in 0..3 {
            let span = (self.max[k] - self.min[k]) as f64;
            let t = if span.abs() < 1e-12 { 0.0 } else { ((c[k] - self.min[k] as f64) / span).clamp(0.0, 1.0) * (n - 1) as f64 };
            if interp == Interp::Nearest {
                idx[k] = (t.round() as usize).min(n - 1);
                continue;
            }
            idx[k] = (t.floor() as usize).min(n - 2);
            f[k] = t - idx[k] as f64;
        }
        if interp == Interp::Nearest {
            return self.at(idx[0], idx[1], idx[2]);
        }
        let (r, g, b) = (idx[0], idx[1], idx[2]);
        let (fx, fy, fz) = (f[0], f[1], f[2]);
        let c000 = self.at(r, g, b);
        let c111 = self.at(r + 1, g + 1, b + 1);
        let mix = |w: [(f64, [f64; 3]); 4]| {
            let mut o = [0.0; 3];
            for (k, c) in w {
                for i in 0..3 {
                    o[i] += k * c[i];
                }
            }
            o
        };
        if matches!(interp, Interp::Tetrahedral | Interp::Best) {
            return if fx > fy {
                if fy > fz {
                    mix([(1.0 - fx, c000), (fx - fy, self.at(r + 1, g, b)), (fy - fz, self.at(r + 1, g + 1, b)), (fz, c111)])
                } else if fx > fz {
                    mix([(1.0 - fx, c000), (fx - fz, self.at(r + 1, g, b)), (fz - fy, self.at(r + 1, g, b + 1)), (fy, c111)])
                } else {
                    mix([(1.0 - fz, c000), (fz - fx, self.at(r, g, b + 1)), (fx - fy, self.at(r + 1, g, b + 1)), (fy, c111)])
                }
            } else if fz > fy {
                mix([(1.0 - fz, c000), (fz - fy, self.at(r, g, b + 1)), (fy - fx, self.at(r, g + 1, b + 1)), (fx, c111)])
            } else if fz > fx {
                mix([(1.0 - fy, c000), (fy - fz, self.at(r, g + 1, b)), (fz - fx, self.at(r, g + 1, b + 1)), (fx, c111)])
            } else {
                mix([(1.0 - fy, c000), (fy - fx, self.at(r, g + 1, b)), (fx - fz, self.at(r + 1, g + 1, b)), (fz, c111)])
            };
        }
        let mut o = [0.0; 3];
        for dz in 0..2 {
            for dy in 0..2 {
                for dx in 0..2 {
                    let w = (if dx == 1 { fx } else { 1.0 - fx }) * (if dy == 1 { fy } else { 1.0 - fy }) * (if dz == 1 { fz } else { 1.0 - fz });
                    let v = self.at(r + dx, g + dy, b + dz);
                    for i in 0..3 {
                        o[i] += v[i] * w;
                    }
                }
            }
        }
        o
    }

    pub fn inv(&self, c: [f64; 3]) -> [f64; 3] {
        let mut x = c;
        for _ in 0..24 {
            let y = self.eval(x, Interp::Linear);
            let mut done = true;
            for k in 0..3 {
                let e = c[k] - y[k];
                if e.abs() > 1e-6 {
                    done = false;
                }
                x[k] = (x[k] + e).clamp(self.min[k] as f64, self.max[k] as f64);
            }
            if done {
                break;
            }
        }
        x
    }
}

fn nums(line: &str) -> Option<Vec<f32>> {
    let v: Result<Vec<f32>, _> = line.split_whitespace().map(str::parse::<f32>).collect();
    v.ok().filter(|v| !v.is_empty())
}

fn cube(text: &str) -> Result<Vec<Op>, String> {
    let (mut n1, mut n3) = (0usize, 0usize);
    let (mut dmin, mut dmax) = ([0.0f32; 3], [1.0f32; 3]);
    let (mut r1, mut r3): (Option<[f32; 2]>, Option<[f32; 2]>) = (None, None);
    let mut rows: Vec<[f32; 3]> = vec![];
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let mut parts = l.split_whitespace();
        let key = parts.next().unwrap_or("");
        let rest: Vec<f32> = parts.filter_map(|x| x.parse().ok()).collect();
        match key {
            "TITLE" => {}
            "LUT_1D_SIZE" => n1 = rest.first().copied().unwrap_or(0.0) as usize,
            "LUT_3D_SIZE" => n3 = rest.first().copied().unwrap_or(0.0) as usize,
            "DOMAIN_MIN" if rest.len() >= 3 => dmin = [rest[0], rest[1], rest[2]],
            "DOMAIN_MAX" if rest.len() >= 3 => dmax = [rest[0], rest[1], rest[2]],
            "LUT_1D_INPUT_RANGE" if rest.len() >= 2 => r1 = Some([rest[0], rest[1]]),
            "LUT_3D_INPUT_RANGE" if rest.len() >= 2 => r3 = Some([rest[0], rest[1]]),
            _ => {
                if let Some(v) = nums(l)
                    && v.len() >= 3
                {
                    rows.push([v[0], v[1], v[2]]);
                }
            }
        }
    }
    let mut ops = vec![];
    let mut at = 0;
    if n1 > 0 {
        if rows.len() < n1 {
            return Err(format!("the .cube says LUT_1D_SIZE {n1} but holds {} rows", rows.len()));
        }
        let (mn, mx) = match r1 {
            Some(r) => ([r[0]; 3], [r[1]; 3]),
            None => (dmin, dmax),
        };
        ops.push(Op::Lut1(Arc::new(Lut1 { data: rows[..n1].to_vec(), min: mn, max: mx }), Interp::Linear, false));
        at = n1;
    }
    if n3 > 0 {
        let need = n3 * n3 * n3;
        if rows.len() < at + need {
            return Err(format!("the .cube says LUT_3D_SIZE {n3} but holds {} rows", rows.len() - at));
        }
        let (mn, mx) = match r3 {
            Some(r) => ([r[0]; 3], [r[1]; 3]),
            None if n1 > 0 => ([0.0; 3], [1.0; 3]),
            None => (dmin, dmax),
        };
        ops.push(Op::Lut3(Arc::new(Lut3 { n: n3, data: rows[at..at + need].to_vec(), min: mn, max: mx }), Interp::Linear, false));
    }
    if ops.is_empty() {
        return Err("the .cube has no LUT_1D_SIZE or LUT_3D_SIZE".into());
    }
    Ok(ops)
}

fn spi1d(text: &str) -> Result<Vec<Op>, String> {
    let (mut from, mut comps, mut in_data) = ([0.0f32, 1.0], 1usize, false);
    let mut rows = vec![];
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if l.starts_with('{') {
            in_data = true;
            continue;
        }
        if l.starts_with('}') {
            break;
        }
        if in_data {
            if let Some(v) = nums(l) {
                rows.push(if comps >= 3 && v.len() >= 3 { [v[0], v[1], v[2]] } else { [v[0]; 3] });
            }
            continue;
        }
        let mut p = l.split_whitespace();
        match p.next() {
            Some("From") => {
                let v: Vec<f32> = p.filter_map(|x| x.parse().ok()).collect();
                if v.len() >= 2 {
                    from = [v[0], v[1]];
                }
            }
            Some("Components") => comps = p.next().and_then(|x| x.parse().ok()).unwrap_or(1),
            _ => {}
        }
    }
    if rows.len() < 2 {
        return Err("the .spi1d holds no data".into());
    }
    Ok(vec![Op::Lut1(Arc::new(Lut1 { data: rows, min: [from[0]; 3], max: [from[1]; 3] }), Interp::Linear, false)])
}

fn spi3d(text: &str) -> Result<Vec<Op>, String> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'));
    lines.next();
    lines.next();
    let size = lines.next().and_then(nums).ok_or("the .spi3d has no size line")?;
    let n = *size.first().ok_or("the .spi3d has no size")? as usize;
    if n < 2 || n > 256 {
        return Err("the .spi3d size is out of range".into());
    }
    let mut data = vec![[0.0f32; 3]; n * n * n];
    for l in lines {
        if let Some(v) = nums(l)
            && v.len() >= 6
        {
            let (i, j, k) = (v[0] as usize, v[1] as usize, v[2] as usize);
            if i < n && j < n && k < n {
                data[(k * n + j) * n + i] = [v[3], v[4], v[5]];
            }
        }
    }
    Ok(vec![Op::Lut3(Arc::new(Lut3 { n, data, min: [0.0; 3], max: [1.0; 3] }), Interp::Linear, false)])
}

fn spimtx(text: &str) -> Result<Vec<Op>, String> {
    let v: Vec<f64> = text.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    if v.len() < 12 {
        return Err("a .spimtx holds twelve numbers".into());
    }
    let m: M3 = [[v[0], v[1], v[2]], [v[4], v[5], v[6]], [v[8], v[9], v[10]]];
    Ok(vec![Op::Mat(m, [v[3] / 65535.0, v[7] / 65535.0, v[11] / 65535.0])])
}

fn lut3dl(text: &str) -> Result<Vec<Op>, String> {
    let rows: Vec<Vec<f32>> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('<')).filter_map(|l| if l.starts_with("Mesh") || l.starts_with("LUT") { None } else { nums(l) }).collect();
    let shaper = rows.iter().find(|r| r.len() > 3).cloned().ok_or("the .3dl has no input shaper line")?;
    let n = shaper.len();
    let trip: Vec<&Vec<f32>> = rows.iter().filter(|r| r.len() == 3).collect();
    if trip.len() < n * n * n {
        return Err(format!("the .3dl needs {} rows and holds {}", n * n * n, trip.len()));
    }
    let maxv = trip.iter().flat_map(|r| r.iter().copied()).fold(0.0f32, f32::max);
    let scale = [1023.0f32, 4095.0, 65535.0].into_iter().find(|s| maxv <= *s).unwrap_or(maxv.max(1.0));
    let mut data = vec![[0.0f32; 3]; n * n * n];
    for (i, r) in trip.iter().take(n * n * n).enumerate() {
        let (ri, gi, bi) = (i / (n * n), (i / n) % n, i % n);
        data[(bi * n + gi) * n + ri] = [r[0] / scale, r[1] / scale, r[2] / scale];
    }
    Ok(vec![Op::Lut3(Arc::new(Lut3 { n, data, min: [0.0; 3], max: [1.0; 3] }), Interp::Linear, false)])
}

fn depth_max(s: Option<&str>) -> f64 {
    match s.unwrap_or("32f") {
        "8i" => 255.0,
        "10i" => 1023.0,
        "12i" => 4095.0,
        "16i" => 65535.0,
        _ => 1.0,
    }
}

fn floats(el: &El) -> Vec<f64> {
    el.text().split_whitespace().filter_map(|x| x.parse().ok()).collect()
}

fn three(el: Option<&El>, def: f64) -> [f64; 3] {
    let v = el.map(floats).unwrap_or_default();
    match v.len() {
        0 => [def; 3],
        1 | 2 => [v[0]; 3],
        _ => [v[0], v[1], v[2]],
    }
}

fn cdl_node(cc: &El, clamp: bool, inv: bool) -> Op {
    let sop = cc.child("SOPNode");
    let sat = cc.child("SatNode").and_then(|s| s.child("Saturation")).map(floats).and_then(|v| v.first().copied()).unwrap_or(1.0);
    Op::Cdl { s: three(sop.and_then(|n| n.child("Slope")), 1.0), o: three(sop.and_then(|n| n.child("Offset")), 0.0), p: three(sop.and_then(|n| n.child("Power")), 1.0), sat, clamp, inv }
}

fn find_cc<'a>(root: &'a El, id: &str) -> Option<&'a El> {
    let mut all: Vec<&El> = vec![];
    fn walk<'b>(e: &'b El, out: &mut Vec<&'b El>) {
        if e.name == "ColorCorrection" {
            out.push(e);
        }
        for k in e.children() {
            walk(k, out);
        }
    }
    walk(root, &mut all);
    if id.is_empty() {
        return all.first().copied();
    }
    all.iter().copied().find(|e| e.attr("id") == Some(id)).or_else(|| id.parse::<usize>().ok().and_then(|i| all.get(i).copied()))
}

fn log_params(node: &El, ch: usize) -> (f64, [f64; 4], Option<f64>, Option<f64>) {
    let mut base = node.attr("base").and_then(|v| v.parse().ok()).unwrap_or(2.0);
    let mut v = [1.0, 0.0, 1.0, 0.0];
    let (mut brk, mut lin_slope) = (None, None);
    for p in node.children().filter(|e| e.name == "LogParams") {
        let c = p.attr("channel");
        let mine = c.is_none() || c == Some(["R", "G", "B"][ch]);
        if !mine {
            continue;
        }
        let f = |k: &str| p.attr(k).and_then(|x| x.parse::<f64>().ok());
        if let Some(b) = f("base") {
            base = b;
        }
        v = [f("logSideSlope").unwrap_or(1.0), f("logSideOffset").unwrap_or(0.0), f("linSideSlope").unwrap_or(1.0), f("linSideOffset").unwrap_or(0.0)];
        brk = f("linSideBreak");
        lin_slope = f("linearSlope");
    }
    (base, v, brk, lin_slope)
}

fn clf(text: &str) -> Result<Vec<Op>, String> {
    let root = xmldoc::parse(text)?;
    let mut ops = vec![];
    for node in root.children() {
        let (inm, outm) = (depth_max(node.attr("inBitDepth")), depth_max(node.attr("outBitDepth")));
        let style = node.attr("style").unwrap_or("");
        match node.name.as_str() {
            "Matrix" => {
                let a = node.child("Array").ok_or("a Matrix has no Array")?;
                let v = floats(a);
                let (m, o) = match v.len() {
                    9 => ([[v[0], v[1], v[2]], [v[3], v[4], v[5]], [v[6], v[7], v[8]]], [0.0; 3]),
                    12 => ([[v[0], v[1], v[2]], [v[4], v[5], v[6]], [v[8], v[9], v[10]]], [v[3], v[7], v[11]]),
                    16 => ([[v[0], v[1], v[2]], [v[4], v[5], v[6]], [v[8], v[9], v[10]]], [v[3], v[7], v[11]]),
                    _ => return Err("a Matrix Array holds 9 or 12 values".into()),
                };
                let k = inm / outm;
                ops.push(Op::Mat(m.map(|r| r.map(|x| x * k)), o.map(|x| x / outm)));
            }
            "LUT1D" => {
                let a = node.child("Array").ok_or("a LUT1D has no Array")?;
                let dims: Vec<usize> = a.attr("dim").unwrap_or("").split_whitespace().filter_map(|x| x.parse().ok()).collect();
                let v = floats(a);
                let comps = dims.get(1).copied().unwrap_or(1).max(1);
                let rows: Vec<[f32; 3]> = v.chunks(comps).map(|c| if comps >= 3 { [(c[0] / outm) as f32, (c[1] / outm) as f32, (c[2] / outm) as f32] } else { [(c[0] / outm) as f32; 3] }).collect();
                ops.push(Op::Lut1(Arc::new(Lut1 { data: rows, min: [0.0; 3], max: [1.0; 3] }), Interp::Linear, false));
            }
            "LUT3D" => {
                let a = node.child("Array").ok_or("a LUT3D has no Array")?;
                let dims: Vec<usize> = a.attr("dim").unwrap_or("").split_whitespace().filter_map(|x| x.parse().ok()).collect();
                let n = *dims.first().ok_or("a LUT3D Array has no dim")?;
                let v = floats(a);
                if v.len() < n * n * n * 3 {
                    return Err("a LUT3D Array is short".into());
                }
                let mut data = vec![[0.0f32; 3]; n * n * n];
                for i in 0..n * n * n {
                    let (ri, gi, bi) = (i / (n * n), (i / n) % n, i % n);
                    data[(bi * n + gi) * n + ri] = [(v[i * 3] / outm) as f32, (v[i * 3 + 1] / outm) as f32, (v[i * 3 + 2] / outm) as f32];
                }
                let interp = if node.attr("interpolation") == Some("tetrahedral") { Interp::Tetrahedral } else { Interp::Linear };
                ops.push(Op::Lut3(Arc::new(Lut3 { n, data, min: [0.0; 3], max: [1.0; 3] }), interp, false));
            }
            "Range" => {
                let f = |k: &str| node.child(k).map(floats).and_then(|v| v.first().copied());
                let (a, b, c, d) = (f("minInValue").map(|x| x / inm), f("maxInValue").map(|x| x / inm), f("minOutValue").map(|x| x / outm), f("maxOutValue").map(|x| x / outm));
                let clamp = !style.eq_ignore_ascii_case("noClamp");
                ops.push(super::ocio::range_op(a, b, c, d, clamp));
            }
            "Log" => {
                let (base, v, brk, ls) = log_params(node, 0);
                let op = match style {
                    "log10" => Op::Log { base: 10.0, inv: false },
                    "log2" => Op::Log { base: 2.0, inv: false },
                    "antiLog10" => Op::Log { base: 10.0, inv: true },
                    "antiLog2" => Op::Log { base: 2.0, inv: true },
                    "linToLog" | "logToLin" => super::ocio::log_aff(base, [v[0]; 3], [v[1]; 3], [v[2]; 3], [v[3]; 3], None, None, style == "logToLin"),
                    "cameraLinToLog" | "cameraLogToLin" => super::ocio::log_aff(base, [v[0]; 3], [v[1]; 3], [v[2]; 3], [v[3]; 3], brk.map(|b| [b; 3]), ls.map(|s| [s; 3]), style == "cameraLogToLin"),
                    _ => return Err(format!("unknown Log style `{style}`")),
                };
                ops.push(op);
            }
            "Exponent" => {
                let p = node.children().find(|e| e.name == "ExponentParams");
                let e = p.and_then(|p| p.attr("exponent")).and_then(|x| x.parse::<f64>().ok()).unwrap_or(1.0);
                let off = p.and_then(|p| p.attr("offset")).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0);
                let neg = if style.contains("Mirror") { super::ocio::Neg::Mirror } else if style.contains("PassThru") { super::ocio::Neg::PassThru } else { super::ocio::Neg::Clamp };
                let rev = style.ends_with("Rev");
                let op = if style.starts_with("monCurve") {
                    Op::Moncurve { g: [e; 3], o: [off; 3], neg: if neg == super::ocio::Neg::Mirror { neg } else { super::ocio::Neg::Linear }, encode: rev }
                } else {
                    Op::Pow(if rev { [1.0 / e; 3] } else { [e; 3] }, neg)
                };
                ops.push(op);
            }
            "ASC_CDL" => {
                let clamp = !style.contains("NoClamp");
                let inv = style.starts_with("Rev");
                ops.push(cdl_node(node, clamp, inv));
            }
            "Description" | "InputDescriptor" | "OutputDescriptor" | "Info" => {}
            other => return Err(format!("the CLF holds a `{other}` node formvfx does not read")),
        }
    }
    Ok(ops)
}

fn cdl_file(text: &str, cccid: &str) -> Result<Vec<Op>, String> {
    let root = xmldoc::parse(text)?;
    let cc = if root.name == "ColorCorrection" { Some(&root) } else { find_cc(&root, cccid) };
    let cc = cc.ok_or_else(|| if cccid.is_empty() { "no ColorCorrection in the file".to_string() } else { format!("no ColorCorrection `{cccid}` in the file") })?;
    Ok(vec![cdl_node(cc, true, false)])
}

pub fn parse(path: &Path, text: &str, cccid: &str) -> Result<Vec<Op>, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "cube" => cube(text),
        "spi1d" => spi1d(text),
        "spi3d" => spi3d(text),
        "spimtx" => spimtx(text),
        "3dl" => lut3dl(text),
        "clf" | "ctf" => clf(text),
        "cc" | "ccc" | "cdl" => cdl_file(text, cccid),
        _ => Err(format!("formvfx does not read .{ext} LUTs (cube, spi1d, spi3d, spimtx, 3dl, clf, ctf, cc, ccc, cdl)")),
    }
}

type Cache = Mutex<HashMap<(String, String), (Option<std::time::SystemTime>, Arc<Vec<Op>>)>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn load(path: &str, cccid: &str) -> Result<Arc<Vec<Op>>, String> {
    let p = Path::new(path);
    let mtime = std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let key = (path.to_string(), cccid.to_string());
    if let Some((t, ops)) = cache().lock().get(&key)
        && *t == mtime
    {
        return Ok(ops.clone());
    }
    let text = std::fs::read_to_string(p).map_err(|e| format!("{path}: {e}"))?;
    let ops = Arc::new(parse(p, &text, cccid)?);
    cache().lock().insert(key, (mtime, ops.clone()));
    Ok(ops)
}
