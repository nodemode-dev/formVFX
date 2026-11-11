use std::collections::HashMap;

use rayon::prelude::*;

use super::ptload::Tensor;

pub const SIZE: usize = 1024;
pub const EMB: usize = 64;
const MEAN: [f32; 3] = [123.675, 116.28, 103.53];
const STD: [f32; 3] = [58.395, 57.12, 57.375];

pub(crate) type W = HashMap<String, Tensor>;

pub(crate) fn get<'a>(w: &'a W, k: &str) -> Result<&'a Tensor, String> {
    w.get(k).ok_or_else(|| format!("the MobileSAM weights lack {k}"))
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn block_avx2(k: usize, n: usize, a: &[f32], b: &[f32], c: &mut [f32], rows: usize) {
    use std::arch::x86_64::*;
    let full = n / 16 * 16;
    let mut i = 0;
    while i + 4 <= rows {
        let mut j = 0;
        while j < full {
            unsafe {
                let cp = c.as_mut_ptr();
                let mut acc = [_mm256_setzero_ps(); 8];
                for r in 0..4 {
                    acc[r * 2] = _mm256_loadu_ps(cp.add((i + r) * n + j));
                    acc[r * 2 + 1] = _mm256_loadu_ps(cp.add((i + r) * n + j + 8));
                }
                let bp = b.as_ptr();
                let ap = a.as_ptr();
                for kk in 0..k {
                    let b0 = _mm256_loadu_ps(bp.add(kk * n + j));
                    let b1 = _mm256_loadu_ps(bp.add(kk * n + j + 8));
                    for r in 0..4 {
                        let av = _mm256_broadcast_ss(&*ap.add((i + r) * k + kk));
                        acc[r * 2] = _mm256_fmadd_ps(av, b0, acc[r * 2]);
                        acc[r * 2 + 1] = _mm256_fmadd_ps(av, b1, acc[r * 2 + 1]);
                    }
                }
                for r in 0..4 {
                    _mm256_storeu_ps(cp.add((i + r) * n + j), acc[r * 2]);
                    _mm256_storeu_ps(cp.add((i + r) * n + j + 8), acc[r * 2 + 1]);
                }
            }
            j += 16;
        }
        for r in i..i + 4 {
            for jj in full..n {
                let mut s = c[r * n + jj];
                for kk in 0..k {
                    s += a[r * k + kk] * b[kk * n + jj];
                }
                c[r * n + jj] = s;
            }
        }
        i += 4;
    }
    for r in i..rows {
        for kk in 0..k {
            let av = a[r * k + kk];
            for jj in 0..n {
                c[r * n + jj] += av * b[kk * n + jj];
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn block_avx512(k: usize, n: usize, a: &[f32], b: &[f32], c: &mut [f32], rows: usize) {
    use std::arch::x86_64::*;
    let full = n / 32 * 32;
    let mut i = 0;
    while i + 8 <= rows {
        let mut j = 0;
        while j < full {
            unsafe {
                let cp = c.as_mut_ptr();
                let mut acc = [_mm512_setzero_ps(); 16];
                for r in 0..8 {
                    acc[r * 2] = _mm512_loadu_ps(cp.add((i + r) * n + j));
                    acc[r * 2 + 1] = _mm512_loadu_ps(cp.add((i + r) * n + j + 16));
                }
                let bp = b.as_ptr();
                let ap = a.as_ptr();
                for kk in 0..k {
                    let b0 = _mm512_loadu_ps(bp.add(kk * n + j));
                    let b1 = _mm512_loadu_ps(bp.add(kk * n + j + 16));
                    for r in 0..8 {
                        let av = _mm512_set1_ps(*ap.add((i + r) * k + kk));
                        acc[r * 2] = _mm512_fmadd_ps(av, b0, acc[r * 2]);
                        acc[r * 2 + 1] = _mm512_fmadd_ps(av, b1, acc[r * 2 + 1]);
                    }
                }
                for r in 0..8 {
                    _mm512_storeu_ps(cp.add((i + r) * n + j), acc[r * 2]);
                    _mm512_storeu_ps(cp.add((i + r) * n + j + 16), acc[r * 2 + 1]);
                }
            }
            j += 32;
        }
        for r in i..i + 8 {
            for jj in full..n {
                let mut s = c[r * n + jj];
                for kk in 0..k {
                    s += a[r * k + kk] * b[kk * n + jj];
                }
                c[r * n + jj] = s;
            }
        }
        i += 8;
    }
    if i < rows {
        unsafe { block_avx2(k, n, &a[i * k..], b, &mut c[i * n..], rows - i) };
    }
}

pub(crate) struct Packed {
    k: usize,
    n: usize,
    panels: Vec<f32>,
    tail: Vec<f32>,
}

pub(crate) fn pack_b(k: usize, n: usize, b: &[f32]) -> Packed {
    let np = n / 32;
    let mut panels = vec![0.0f32; np * k * 32];
    panels.par_chunks_mut(k * 32).enumerate().for_each(|(p, pan)| {
        for kk in 0..k {
            pan[kk * 32..kk * 32 + 32].copy_from_slice(&b[kk * n + p * 32..kk * n + p * 32 + 32]);
        }
    });
    let rem = n - np * 32;
    let mut tail = vec![0.0f32; k * rem];
    for kk in 0..k {
        tail[kk * rem..(kk + 1) * rem].copy_from_slice(&b[kk * n + np * 32..(kk + 1) * n]);
    }
    Packed { k, n, panels, tail }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn panel_avx512(k: usize, n: usize, a: &[f32], pan: &[f32], c: &mut [f32], j: usize, rows: usize) {
    use std::arch::x86_64::*;
    let mut i = 0;
    while i + 8 <= rows {
        unsafe {
            let cp = c.as_mut_ptr();
            let mut acc = [_mm512_setzero_ps(); 16];
            for r in 0..8 {
                acc[r * 2] = _mm512_loadu_ps(cp.add((i + r) * n + j));
                acc[r * 2 + 1] = _mm512_loadu_ps(cp.add((i + r) * n + j + 16));
            }
            let bp = pan.as_ptr();
            let ap = a.as_ptr();
            for kk in 0..k {
                let b0 = _mm512_loadu_ps(bp.add(kk * 32));
                let b1 = _mm512_loadu_ps(bp.add(kk * 32 + 16));
                for r in 0..8 {
                    let av = _mm512_set1_ps(*ap.add((i + r) * k + kk));
                    acc[r * 2] = _mm512_fmadd_ps(av, b0, acc[r * 2]);
                    acc[r * 2 + 1] = _mm512_fmadd_ps(av, b1, acc[r * 2 + 1]);
                }
            }
            for r in 0..8 {
                _mm512_storeu_ps(cp.add((i + r) * n + j), acc[r * 2]);
                _mm512_storeu_ps(cp.add((i + r) * n + j + 16), acc[r * 2 + 1]);
            }
        }
        i += 8;
    }
    for r in i..rows {
        for kk in 0..k {
            let av = a[r * k + kk];
            for jj in 0..32 {
                c[r * n + j + jj] += av * pan[kk * 32 + jj];
            }
        }
    }
}

pub(crate) fn gemm_packed(m: usize, a: &[f32], b: &Packed, c: &mut [f32]) {
    let (k, n) = (b.k, b.n);
    let np = n / 32;
    let rem = n - np * 32;
    #[cfg(target_arch = "x86_64")]
    let wide = has_avx512() && has_avx2();
    c.par_chunks_mut(n * 16).enumerate().for_each(|(ci, cblk)| {
        let i0 = ci * 16;
        let rows = cblk.len() / n;
        let ab = &a[i0 * k..(i0 + rows) * k];
        for p in 0..np {
            let pan = &b.panels[p * k * 32..(p + 1) * k * 32];
            #[cfg(target_arch = "x86_64")]
            if wide {
                unsafe { panel_avx512(k, n, ab, pan, cblk, p * 32, rows) };
                continue;
            }
            for r in 0..rows {
                for kk in 0..k {
                    let av = ab[r * k + kk];
                    for jj in 0..32 {
                        cblk[r * n + p * 32 + jj] += av * pan[kk * 32 + jj];
                    }
                }
            }
        }
        if rem > 0 {
            for r in 0..rows {
                for kk in 0..k {
                    let av = ab[r * k + kk];
                    for jj in 0..rem {
                        cblk[r * n + np * 32 + jj] += av * b.tail[kk * rem + jj];
                    }
                }
            }
        }
    });
    let _ = m;
}

#[cfg(target_arch = "x86_64")]
fn has_avx512() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| is_x86_feature_detected!("avx512f"))
}

#[cfg(target_arch = "x86_64")]
fn has_avx2() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma"))
}

pub(crate) fn gemm_serial(m: usize, k: usize, n: usize, a: &[f32], b: &[f32], c: &mut [f32]) {
    #[cfg(target_arch = "x86_64")]
    if has_avx512() && has_avx2() {
        unsafe { block_avx512(k, n, &a[..m * k], b, &mut c[..m * n], m) };
        return;
    }
    #[cfg(target_arch = "x86_64")]
    if has_avx2() {
        unsafe { block_avx2(k, n, &a[..m * k], b, &mut c[..m * n], m) };
        return;
    }
    for r in 0..m {
        for kk in 0..k {
            let av = a[r * k + kk];
            if av == 0.0 {
                continue;
            }
            for (cv, bv) in c[r * n..(r + 1) * n].iter_mut().zip(&b[kk * n..(kk + 1) * n]) {
                *cv += av * bv;
            }
        }
    }
}

pub fn gemm(m: usize, k: usize, n: usize, a: &[f32], b: &[f32], c: &mut [f32]) {
    const RB: usize = 8;
    const NB: usize = 256;
    #[cfg(target_arch = "x86_64")]
    if has_avx2() && has_avx512() && m >= 64 && n >= 128 {
        gemm_packed(m, a, &pack_b(k, n, b), c);
        return;
    }
    #[cfg(target_arch = "x86_64")]
    if has_avx2() {
        let wide = has_avx512();
        c.par_chunks_mut(n * 16).enumerate().for_each(|(ci, cblk)| {
            let i0 = ci * 16;
            let rows = cblk.len() / n;
            if wide {
                unsafe { block_avx512(k, n, &a[i0 * k..(i0 + rows) * k], b, cblk, rows) };
            } else {
                unsafe { block_avx2(k, n, &a[i0 * k..(i0 + rows) * k], b, cblk, rows) };
            }
        });
        debug_assert_eq!(c.len(), m * n);
        return;
    }
    c.par_chunks_mut(n * RB).enumerate().for_each(|(ci, cblk)| {
        let i0 = ci * RB;
        let rows = cblk.len() / n;
        let mut nb = 0;
        while nb < n {
            let ne = (nb + NB).min(n);
            for kk in 0..k {
                let br = &b[kk * n + nb..kk * n + ne];
                for r in 0..rows {
                    let av = a[(i0 + r) * k + kk];
                    if av == 0.0 {
                        continue;
                    }
                    let cr = &mut cblk[r * n + nb..r * n + ne];
                    for (cv, bv) in cr.iter_mut().zip(br) {
                        *cv += av * bv;
                    }
                }
            }
            nb = ne;
        }
    });
    debug_assert_eq!(c.len(), m * n);
}

fn erf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let y = 1.0 - (((((1.061_405_4 * t - 1.453_152_1) * t) + 1.421_413_8) * t - 0.284_496_74) * t + 0.254_829_6) * t * (-x * x).exp();
    if x >= 0.0 { y } else { -y }
}

pub(crate) fn gelu(v: &mut [f32]) {
    v.par_iter_mut().for_each(|x| *x = 0.5 * *x * (1.0 + erf(*x * std::f32::consts::FRAC_1_SQRT_2)));
}

pub(crate) fn relu(v: &mut [f32]) {
    v.iter_mut().for_each(|x| *x = x.max(0.0));
}

#[derive(Clone)]
pub(crate) struct Lin {
    pub(crate) wt: Vec<f32>,
    pub(crate) b: Vec<f32>,
    pub(crate) inp: usize,
    pub(crate) out: usize,
}

impl Lin {
    pub(crate) fn new(w: &W, name: &str, bias: bool) -> Result<Lin, String> {
        let t = get(w, &format!("{name}.weight"))?;
        let (out, inp) = (t.shape[0], t.data.len() / t.shape[0]);
        let mut wt = vec![0.0f32; inp * out];
        for o in 0..out {
            for i in 0..inp {
                wt[i * out + o] = t.data[o * inp + i];
            }
        }
        let b = if bias { get(w, &format!("{name}.bias"))?.data.clone() } else { vec![0.0; out] };
        Ok(Lin { wt, b, inp, out })
    }

    fn conv_bn(w: &W, name: &str) -> Result<Lin, String> {
        let t = get(w, &format!("{name}.c.weight"))?;
        let out = t.shape[0];
        let inp = t.data.len() / out;
        let g = &get(w, &format!("{name}.bn.weight"))?.data;
        let be = &get(w, &format!("{name}.bn.bias"))?.data;
        let mu = &get(w, &format!("{name}.bn.running_mean"))?.data;
        let var = &get(w, &format!("{name}.bn.running_var"))?.data;
        let mut wt = vec![0.0f32; inp * out];
        let mut b = vec![0.0f32; out];
        for o in 0..out {
            let s = g[o] / (var[o] + 1e-5).sqrt();
            b[o] = be[o] - mu[o] * s;
            for i in 0..inp {
                wt[i * out + o] = t.data[o * inp + i] * s;
            }
        }
        Ok(Lin { wt, b, inp, out })
    }

    pub(crate) fn apply(&self, x: &[f32], rows: usize) -> Vec<f32> {
        let mut y = vec![0.0f32; rows * self.out];
        y.par_chunks_mut(self.out).for_each(|r| r.copy_from_slice(&self.b));
        gemm(rows, self.inp, self.out, x, &self.wt, &mut y);
        y
    }
}

struct Dw {
    w: Vec<f32>,
    b: Vec<f32>,
    c: usize,
    stride: usize,
}

impl Dw {
    fn conv_bn(w: &W, name: &str, stride: usize) -> Result<Dw, String> {
        let t = get(w, &format!("{name}.c.weight"))?;
        let c = t.shape[0];
        let g = &get(w, &format!("{name}.bn.weight"))?.data;
        let be = &get(w, &format!("{name}.bn.bias"))?.data;
        let mu = &get(w, &format!("{name}.bn.running_mean"))?.data;
        let var = &get(w, &format!("{name}.bn.running_var"))?.data;
        let mut wk = vec![0.0f32; 9 * c];
        let mut b = vec![0.0f32; c];
        for ch in 0..c {
            let s = g[ch] / (var[ch] + 1e-5).sqrt();
            b[ch] = be[ch] - mu[ch] * s;
            for k in 0..9 {
                wk[k * c + ch] = t.data[ch * 9 + k] * s;
            }
        }
        Ok(Dw { w: wk, b, c, stride })
    }

    fn apply(&self, x: &[f32], h: usize, wd: usize) -> (Vec<f32>, usize, usize) {
        let (oh, ow) = ((h - 1) / self.stride + 1, (wd - 1) / self.stride + 1);
        let c = self.c;
        let mut y = vec![0.0f32; oh * ow * c];
        y.par_chunks_mut(ow * c).enumerate().for_each(|(oy, row)| {
            for ox in 0..ow {
                let o = &mut row[ox * c..(ox + 1) * c];
                o.copy_from_slice(&self.b);
                for ky in 0..3 {
                    let iy = (oy * self.stride + ky) as isize - 1;
                    if iy < 0 || iy >= h as isize {
                        continue;
                    }
                    for kx in 0..3 {
                        let ix = (ox * self.stride + kx) as isize - 1;
                        if ix < 0 || ix >= wd as isize {
                            continue;
                        }
                        let src = &x[(iy as usize * wd + ix as usize) * c..(iy as usize * wd + ix as usize + 1) * c];
                        let wk = &self.w[(ky * 3 + kx) * c..(ky * 3 + kx + 1) * c];
                        for ((ov, sv), wv) in o.iter_mut().zip(src).zip(wk) {
                            *ov += sv * wv;
                        }
                    }
                }
            }
        });
        (y, oh, ow)
    }
}

struct Conv3 {
    lin: Lin,
    c: usize,
    stride: usize,
}

impl Conv3 {
    fn from(t: &Tensor, scale: Option<(Vec<f32>, Vec<f32>)>, stride: usize) -> Conv3 {
        let (out, c) = (t.shape[0], t.shape[1]);
        let mut wt = vec![0.0f32; 9 * c * out];
        let (sc, b) = scale.unwrap_or((vec![1.0; out], vec![0.0; out]));
        for o in 0..out {
            for ci in 0..c {
                for k in 0..9 {
                    wt[(k * c + ci) * out + o] = t.data[(o * c + ci) * 9 + k] * sc[o];
                }
            }
        }
        Conv3 { lin: Lin { wt, b, inp: 9 * c, out }, c, stride }
    }

    fn conv_bn(w: &W, name: &str, stride: usize) -> Result<Conv3, String> {
        let t = get(w, &format!("{name}.c.weight"))?;
        let g = &get(w, &format!("{name}.bn.weight"))?.data;
        let be = &get(w, &format!("{name}.bn.bias"))?.data;
        let mu = &get(w, &format!("{name}.bn.running_mean"))?.data;
        let var = &get(w, &format!("{name}.bn.running_var"))?.data;
        let out = t.shape[0];
        let s: Vec<f32> = (0..out).map(|o| g[o] / (var[o] + 1e-5).sqrt()).collect();
        let b: Vec<f32> = (0..out).map(|o| be[o] - mu[o] * s[o]).collect();
        Ok(Conv3::from(t, Some((s, b)), stride))
    }

    fn apply(&self, x: &[f32], h: usize, wd: usize) -> (Vec<f32>, usize, usize) {
        let (oh, ow) = ((h - 1) / self.stride + 1, (wd - 1) / self.stride + 1);
        let c = self.c;
        let mut cols = vec![0.0f32; oh * ow * 9 * c];
        cols.par_chunks_mut(ow * 9 * c).enumerate().for_each(|(oy, row)| {
            for ox in 0..ow {
                for ky in 0..3 {
                    let iy = (oy * self.stride + ky) as isize - 1;
                    for kx in 0..3 {
                        let ix = (ox * self.stride + kx) as isize - 1;
                        let dst = &mut row[(ox * 9 + ky * 3 + kx) * c..(ox * 9 + ky * 3 + kx + 1) * c];
                        if iy < 0 || iy >= h as isize || ix < 0 || ix >= wd as isize {
                            continue;
                        }
                        let s = (iy as usize * wd + ix as usize) * c;
                        dst.copy_from_slice(&x[s..s + c]);
                    }
                }
            }
        });
        (self.lin.apply(&cols, oh * ow), oh, ow)
    }
}

pub(crate) fn layer_norm(x: &mut [f32], c: usize, g: &[f32], b: &[f32], eps: f32) {
    x.par_chunks_mut(c).for_each(|r| {
        let m = r.iter().sum::<f32>() / c as f32;
        let v = r.iter().map(|x| (x - m) * (x - m)).sum::<f32>() / c as f32;
        let k = 1.0 / (v + eps).sqrt();
        for (i, x) in r.iter_mut().enumerate() {
            *x = (*x - m) * k * g[i] + b[i];
        }
    });
}

pub(crate) struct Norm {
    g: Vec<f32>,
    b: Vec<f32>,
    eps: f32,
}

impl Norm {
    pub(crate) fn new(w: &W, name: &str, eps: f32) -> Result<Norm, String> {
        Ok(Norm { g: get(w, &format!("{name}.weight"))?.data.clone(), b: get(w, &format!("{name}.bias"))?.data.clone(), eps })
    }

    pub(crate) fn apply(&self, x: &mut [f32]) {
        layer_norm(x, self.g.len(), &self.g, &self.b, self.eps);
    }
}

struct MbConv {
    c1: Lin,
    c2: Dw,
    c3: Lin,
}

struct Merge {
    c1: Lin,
    c2: Dw,
    c3: Lin,
}

impl Merge {
    fn apply(&self, x: &[f32], h: usize, w: usize) -> (Vec<f32>, usize, usize) {
        let mut a = self.c1.apply(x, h * w);
        gelu(&mut a);
        let (mut b, oh, ow) = self.c2.apply(&a, h, w);
        gelu(&mut b);
        (self.c3.apply(&b, oh * ow), oh, ow)
    }
}

struct Block {
    norm: Norm,
    qkv: Lin,
    proj: Lin,
    bias: Vec<f32>,
    heads: usize,
    ws: usize,
    local: Dw,
    mlp_norm: Norm,
    fc1: Lin,
    fc2: Lin,
}

impl Block {
    fn attn(&self, x: &[f32], n: usize, c: usize) -> Vec<f32> {
        let mut xn = x.to_vec();
        self.norm.apply(&mut xn);
        let qkv = self.qkv.apply(&xn, n);
        let kd = c / self.heads;
        let per = 3 * kd;
        let scale = 1.0 / (kd as f32).sqrt();
        let ws = self.ws;
        let nb = ws * ws;
        let mut out = vec![0.0f32; n * c];
        for h in 0..self.heads {
            let mut sc = vec![0.0f32; n];
            for i in 0..n {
                let q = &qkv[i * self.heads * per + h * per..i * self.heads * per + h * per + kd];
                let (iy, ix) = (i / ws, i % ws);
                let mut mx = f32::MIN;
                for j in 0..n {
                    let k = &qkv[j * self.heads * per + h * per + kd..j * self.heads * per + h * per + 2 * kd];
                    let (jy, jx) = (j / ws, j % ws);
                    let off = iy.abs_diff(jy) * ws + ix.abs_diff(jx);
                    let s = q.iter().zip(k).map(|(a, b)| a * b).sum::<f32>() * scale + self.bias[h * nb + off];
                    sc[j] = s;
                    mx = mx.max(s);
                }
                let mut sum = 0.0;
                for s in sc.iter_mut() {
                    *s = (*s - mx).exp();
                    sum += *s;
                }
                let o = &mut out[i * c + h * kd..i * c + (h + 1) * kd];
                for (j, s) in sc.iter().enumerate() {
                    let p = s / sum;
                    let v = &qkv[j * self.heads * per + h * per + 2 * kd..j * self.heads * per + h * per + 3 * kd];
                    for (ov, vv) in o.iter_mut().zip(v) {
                        *ov += p * vv;
                    }
                }
            }
        }
        self.proj.apply(&out, n)
    }

    fn apply(&self, x: &mut Vec<f32>, h: usize, w: usize, c: usize) {
        let ws = self.ws;
        let (ph, pw) = (h.div_ceil(ws) * ws, w.div_ceil(ws) * ws);
        let (nh, nw) = (ph / ws, pw / ws);
        let wins: Vec<(usize, usize)> = (0..nh).flat_map(|a| (0..nw).map(move |b| (a, b))).collect();
        let outs: Vec<Vec<f32>> = wins
            .par_iter()
            .map(|(wy, wx)| {
                let mut win = vec![0.0f32; ws * ws * c];
                for y in 0..ws {
                    for xx in 0..ws {
                        let (gy, gx) = (wy * ws + y, wx * ws + xx);
                        if gy < h && gx < w {
                            win[(y * ws + xx) * c..(y * ws + xx + 1) * c].copy_from_slice(&x[(gy * w + gx) * c..(gy * w + gx + 1) * c]);
                        }
                    }
                }
                self.attn(&win, ws * ws, c)
            })
            .collect();
        for ((wy, wx), o) in wins.iter().zip(outs) {
            for y in 0..ws {
                for xx in 0..ws {
                    let (gy, gx) = (wy * ws + y, wx * ws + xx);
                    if gy < h && gx < w {
                        let d = &mut x[(gy * w + gx) * c..(gy * w + gx + 1) * c];
                        for (dv, ov) in d.iter_mut().zip(&o[(y * ws + xx) * c..(y * ws + xx + 1) * c]) {
                            *dv += ov;
                        }
                    }
                }
            }
        }
        let (lc, _, _) = self.local.apply(x, h, w);
        *x = lc;
        let mut m = x.clone();
        self.mlp_norm.apply(&mut m);
        let mut a = self.fc1.apply(&m, h * w);
        gelu(&mut a);
        let b = self.fc2.apply(&a, h * w);
        x.par_iter_mut().zip(b.par_iter()).for_each(|(xv, bv)| *xv += bv);
    }
}

struct Attn {
    q: Lin,
    k: Lin,
    v: Lin,
    o: Lin,
    heads: usize,
}

impl Attn {
    fn new(w: &W, name: &str) -> Result<Attn, String> {
        Ok(Attn { q: Lin::new(w, &format!("{name}.q_proj"), true)?, k: Lin::new(w, &format!("{name}.k_proj"), true)?, v: Lin::new(w, &format!("{name}.v_proj"), true)?, o: Lin::new(w, &format!("{name}.out_proj"), true)?, heads: 8 })
    }

    fn apply(&self, q: &[f32], nq: usize, k: &[f32], nk: usize, v: &[f32]) -> Vec<f32> {
        let qp = self.q.apply(q, nq);
        let kp = self.k.apply(k, nk);
        let vp = self.v.apply(v, nk);
        let d = self.q.out;
        let hd = d / self.heads;
        let scale = 1.0 / (hd as f32).sqrt();
        let mut out = vec![0.0f32; nq * d];
        out.par_chunks_mut(d).enumerate().for_each(|(i, o)| {
            let mut sc = vec![0.0f32; nk];
            for h in 0..self.heads {
                let qi = &qp[i * d + h * hd..i * d + (h + 1) * hd];
                let mut mx = f32::MIN;
                for (j, s) in sc.iter_mut().enumerate() {
                    let kj = &kp[j * d + h * hd..j * d + (h + 1) * hd];
                    *s = qi.iter().zip(kj).map(|(a, b)| a * b).sum::<f32>() * scale;
                    mx = mx.max(*s);
                }
                let mut sum = 0.0;
                for s in sc.iter_mut() {
                    *s = (*s - mx).exp();
                    sum += *s;
                }
                let oh = &mut o[h * hd..(h + 1) * hd];
                for (j, s) in sc.iter().enumerate() {
                    let p = s / sum;
                    for (ov, vv) in oh.iter_mut().zip(&vp[j * d + h * hd..j * d + (h + 1) * hd]) {
                        *ov += p * vv;
                    }
                }
            }
        });
        self.o.apply(&out, nq)
    }
}

struct TwoWay {
    self_attn: Attn,
    n1: Norm,
    t2i: Attn,
    n2: Norm,
    l1: Lin,
    l2: Lin,
    n3: Norm,
    n4: Norm,
    i2t: Attn,
}

pub struct Sam {
    stem1: Conv3,
    stem2: Conv3,
    l0: Vec<MbConv>,
    merges: Vec<Merge>,
    stages: Vec<Vec<Block>>,
    neck1: Lin,
    neck_n1: Norm,
    neck2: Conv3,
    neck_n2: Norm,
    gauss: Vec<f32>,
    point_emb: Vec<Vec<f32>>,
    not_a_point: Vec<f32>,
    no_mask: Vec<f32>,
    md: [Lin; 3],
    md_n: [Norm; 2],
    layers: Vec<TwoWay>,
    final_attn: Attn,
    norm_final: Norm,
    iou_token: Vec<f32>,
    mask_tokens: Vec<f32>,
    up1: (Vec<f32>, Vec<f32>),
    up_n: Norm,
    up2: (Vec<f32>, Vec<f32>),
    hyper: Vec<[Lin; 3]>,
    iou_head: [Lin; 3],
}

fn lin3(w: &W, name: &str) -> Result<[Lin; 3], String> {
    Ok([Lin::new(w, &format!("{name}.layers.0"), true)?, Lin::new(w, &format!("{name}.layers.1"), true)?, Lin::new(w, &format!("{name}.layers.2"), true)?])
}

fn mlp3(m: &[Lin; 3], x: &[f32]) -> Vec<f32> {
    let mut a = m[0].apply(x, 1);
    relu(&mut a);
    let mut b = m[1].apply(&a, 1);
    relu(&mut b);
    m[2].apply(&b, 1)
}

fn convt(w: &W, name: &str) -> Result<(Vec<f32>, Vec<f32>), String> {
    Ok((get(w, &format!("{name}.weight"))?.data.clone(), get(w, &format!("{name}.bias"))?.data.clone()))
}

pub(crate) fn conv_transpose(x: &[f32], h: usize, w: usize, cin: usize, wt: &(Vec<f32>, Vec<f32>), cout: usize) -> Vec<f32> {
    let (oh, ow) = (h * 2, w * 2);
    let mut y = vec![0.0f32; oh * ow * cout];
    let mut wr = vec![0.0f32; 4 * cin * cout];
    for ci in 0..cin {
        for co in 0..cout {
            for k in 0..4 {
                wr[(k * cin + ci) * cout + co] = wt.0[(ci * cout + co) * 4 + k];
            }
        }
    }
    y.par_chunks_mut(ow * cout).enumerate().for_each(|(oy, row)| {
        let (iy, dy) = (oy / 2, oy % 2);
        for ox in 0..ow {
            let (ix, dx) = (ox / 2, ox % 2);
            let k = dy * 2 + dx;
            let o = &mut row[ox * cout..(ox + 1) * cout];
            o.copy_from_slice(&wt.1);
            let src = &x[(iy * w + ix) * cin..(iy * w + ix + 1) * cin];
            for (ci, sv) in src.iter().enumerate() {
                let wrow = &wr[(k * cin + ci) * cout..(k * cin + ci + 1) * cout];
                for (ov, wv) in o.iter_mut().zip(wrow) {
                    *ov += sv * wv;
                }
            }
        }
    });
    y
}

impl Sam {
    pub fn load(w: &W) -> Result<Sam, String> {
        let pe = "image_encoder";
        let stem1 = Conv3::conv_bn(w, &format!("{pe}.patch_embed.seq.0"), 2)?;
        let stem2 = Conv3::conv_bn(w, &format!("{pe}.patch_embed.seq.2"), 2)?;
        let mut l0 = vec![];
        for b in 0..2 {
            let n = format!("{pe}.layers.0.blocks.{b}");
            l0.push(MbConv { c1: Lin::conv_bn(w, &format!("{n}.conv1"))?, c2: Dw::conv_bn(w, &format!("{n}.conv2"), 1)?, c3: Lin::conv_bn(w, &format!("{n}.conv3"))? });
        }
        let mut merges = vec![];
        for (i, stride) in [(0, 2), (1, 2), (2, 1)] {
            let n = format!("{pe}.layers.{i}.downsample");
            merges.push(Merge { c1: Lin::conv_bn(w, &format!("{n}.conv1"))?, c2: Dw::conv_bn(w, &format!("{n}.conv2"), stride)?, c3: Lin::conv_bn(w, &format!("{n}.conv3"))? });
        }
        let mut stages = vec![];
        for (li, depth, heads, ws) in [(1, 2, 4, 7), (2, 6, 5, 14), (3, 2, 10, 7)] {
            let mut blocks = vec![];
            for b in 0..depth {
                let n = format!("{pe}.layers.{li}.blocks.{b}");
                blocks.push(Block {
                    norm: Norm::new(w, &format!("{n}.attn.norm"), 1e-5)?,
                    qkv: Lin::new(w, &format!("{n}.attn.qkv"), true)?,
                    proj: Lin::new(w, &format!("{n}.attn.proj"), true)?,
                    bias: get(w, &format!("{n}.attn.attention_biases"))?.data.clone(),
                    heads,
                    ws,
                    local: Dw::conv_bn(w, &format!("{n}.local_conv"), 1)?,
                    mlp_norm: Norm::new(w, &format!("{n}.mlp.norm"), 1e-5)?,
                    fc1: Lin::new(w, &format!("{n}.mlp.fc1"), true)?,
                    fc2: Lin::new(w, &format!("{n}.mlp.fc2"), true)?,
                });
            }
            stages.push(blocks);
        }
        let neck1 = Lin::new(w, &format!("{pe}.neck.0"), false)?;
        let neck2 = Conv3::from(get(w, &format!("{pe}.neck.2.weight"))?, None, 1);
        let pp = "prompt_encoder";
        let md = [Lin::new(w, &format!("{pp}.mask_downscaling.0"), true)?, Lin::new(w, &format!("{pp}.mask_downscaling.3"), true)?, Lin::new(w, &format!("{pp}.mask_downscaling.6"), true)?];
        let md_n = [Norm::new(w, &format!("{pp}.mask_downscaling.1"), 1e-6)?, Norm::new(w, &format!("{pp}.mask_downscaling.4"), 1e-6)?];
        let dd = "mask_decoder";
        let mut layers = vec![];
        for i in 0..2 {
            let n = format!("{dd}.transformer.layers.{i}");
            layers.push(TwoWay {
                self_attn: Attn::new(w, &format!("{n}.self_attn"))?,
                n1: Norm::new(w, &format!("{n}.norm1"), 1e-5)?,
                t2i: Attn::new(w, &format!("{n}.cross_attn_token_to_image"))?,
                n2: Norm::new(w, &format!("{n}.norm2"), 1e-5)?,
                l1: Lin::new(w, &format!("{n}.mlp.lin1"), true)?,
                l2: Lin::new(w, &format!("{n}.mlp.lin2"), true)?,
                n3: Norm::new(w, &format!("{n}.norm3"), 1e-5)?,
                n4: Norm::new(w, &format!("{n}.norm4"), 1e-5)?,
                i2t: Attn::new(w, &format!("{n}.cross_attn_image_to_token"))?,
            });
        }
        let mut hyper = vec![];
        for i in 0..4 {
            hyper.push(lin3(w, &format!("{dd}.output_hypernetworks_mlps.{i}"))?);
        }
        Ok(Sam {
            stem1,
            stem2,
            l0,
            merges,
            stages,
            neck1,
            neck_n1: Norm::new(w, &format!("{pe}.neck.1"), 1e-6)?,
            neck2,
            neck_n2: Norm::new(w, &format!("{pe}.neck.3"), 1e-6)?,
            gauss: get(w, &format!("{pp}.pe_layer.positional_encoding_gaussian_matrix"))?.data.clone(),
            point_emb: (0..4).map(|i| get(w, &format!("{pp}.point_embeddings.{i}.weight")).map(|t| t.data.clone())).collect::<Result<_, _>>()?,
            not_a_point: get(w, &format!("{pp}.not_a_point_embed.weight"))?.data.clone(),
            no_mask: get(w, &format!("{pp}.no_mask_embed.weight"))?.data.clone(),
            md,
            md_n,
            layers,
            final_attn: Attn::new(w, &format!("{dd}.transformer.final_attn_token_to_image"))?,
            norm_final: Norm::new(w, &format!("{dd}.transformer.norm_final_attn"), 1e-5)?,
            iou_token: get(w, &format!("{dd}.iou_token.weight"))?.data.clone(),
            mask_tokens: get(w, &format!("{dd}.mask_tokens.weight"))?.data.clone(),
            up1: convt(w, &format!("{dd}.output_upscaling.0"))?,
            up_n: Norm::new(w, &format!("{dd}.output_upscaling.1"), 1e-6)?,
            up2: convt(w, &format!("{dd}.output_upscaling.3"))?,
            hyper,
            iou_head: lin3(w, &format!("{dd}.iou_prediction_head"))?,
        })
    }

    pub fn embed(&self, rgb: &[f32], h: usize, w: usize) -> Vec<f32> {
        let mut x = vec![0.0f32; SIZE * SIZE * 3];
        for y in 0..h.min(SIZE) {
            for xx in 0..w.min(SIZE) {
                for c in 0..3 {
                    x[(y * SIZE + xx) * 3 + c] = (rgb[(y * w + xx) * 3 + c] - MEAN[c]) / STD[c];
                }
            }
        }
        let (mut x, h1, w1) = self.stem1.apply(&x, SIZE, SIZE);
        gelu(&mut x);
        let (mut x, mut h, mut w) = self.stem2.apply(&x, h1, w1);
        for b in &self.l0 {
            let mut a = b.c1.apply(&x, h * w);
            gelu(&mut a);
            let (mut d, _, _) = b.c2.apply(&a, h, w);
            gelu(&mut d);
            let e = b.c3.apply(&d, h * w);
            x.par_iter_mut().zip(e.par_iter()).for_each(|(xv, ev)| *xv = *xv + ev);
            gelu(&mut x);
        }
        let mut c;
        for (si, blocks) in self.stages.iter().enumerate() {
            let (nx, nh, nw) = self.merges[si].apply(&x, h, w);
            x = nx;
            h = nh;
            w = nw;
            c = self.merges[si].c3.out;
            for b in blocks {
                b.apply(&mut x, h, w, c);
            }
        }
        let mut y = self.neck1.apply(&x, h * w);
        self.neck_n1.apply(&mut y);
        let (mut y, _, _) = self.neck2.apply(&y, h, w);
        self.neck_n2.apply(&mut y);
        y
    }

    fn pe(&self, x: f32, y: f32) -> Vec<f32> {
        let (cx, cy) = (2.0 * x - 1.0, 2.0 * y - 1.0);
        let mut out = vec![0.0f32; 256];
        for j in 0..128 {
            let v = (cx * self.gauss[j] + cy * self.gauss[128 + j]) * std::f32::consts::TAU;
            out[j] = v.sin();
            out[128 + j] = v.cos();
        }
        out
    }

    pub fn dense_pe(&self) -> Vec<f32> {
        let mut out = vec![0.0f32; EMB * EMB * 256];
        out.par_chunks_mut(256).enumerate().for_each(|(i, o)| {
            let (y, x) = (i / EMB, i % EMB);
            o.copy_from_slice(&self.pe((x as f32 + 0.5) / EMB as f32, (y as f32 + 0.5) / EMB as f32));
        });
        out
    }

    pub fn decode(&self, emb: &[f32], points: &[([f32; 2], i32)], mask: Option<&[f32]>) -> (Vec<Vec<f32>>, Vec<f32>) {
        let mut sparse: Vec<f32> = vec![];
        for (p, l) in points {
            let mut e = self.pe((p[0] + 0.5) / SIZE as f32, (p[1] + 0.5) / SIZE as f32);
            match l {
                -1 => e = self.not_a_point.clone(),
                l if (0..4).contains(l) => {
                    for (a, b) in e.iter_mut().zip(&self.point_emb[*l as usize]) {
                        *a += b;
                    }
                }
                _ => {}
            }
            sparse.extend(e);
        }
        let dense: Vec<f32> = match mask {
            Some(m) => {
                let mut x: Vec<f32> = m.to_vec();
                let (mut h, mut w, mut c) = (256usize, 256usize, 1usize);
                for (i, out) in [4usize, 16].into_iter().enumerate() {
                    let lin = &self.md[i];
                    let mut y = vec![0.0f32; (h / 2) * (w / 2) * out];
                    for oy in 0..h / 2 {
                        for ox in 0..w / 2 {
                            let mut patch = vec![0.0f32; 4 * c];
                            for ky in 0..2 {
                                for kx in 0..2 {
                                    for ci in 0..c {
                                        patch[ci * 4 + ky * 2 + kx] = x[((oy * 2 + ky) * w + ox * 2 + kx) * c + ci];
                                    }
                                }
                            }
                            let r = lin.apply(&patch, 1);
                            y[(oy * (w / 2) + ox) * out..(oy * (w / 2) + ox + 1) * out].copy_from_slice(&r);
                        }
                    }
                    self.md_n[i].apply(&mut y);
                    gelu(&mut y);
                    x = y;
                    h /= 2;
                    w /= 2;
                    c = out;
                }
                self.md[2].apply(&x, h * w)
            }
            None => (0..EMB * EMB).flat_map(|_| self.no_mask.iter().copied()).collect(),
        };
        let nt = 5 + points.len();
        let mut tokens: Vec<f32> = vec![];
        tokens.extend(&self.iou_token);
        tokens.extend(&self.mask_tokens);
        tokens.extend(&sparse);
        let q_pe = tokens.clone();
        let mut keys: Vec<f32> = emb.iter().zip(&dense).map(|(a, b)| a + b).collect();
        let k_pe = self.dense_pe();
        let nk = EMB * EMB;
        let mut q = tokens.clone();
        let add = |a: &[f32], b: &[f32]| -> Vec<f32> { a.iter().zip(b).map(|(x, y)| x + y).collect() };
        for (li, l) in self.layers.iter().enumerate() {
            if li == 0 {
                q = l.self_attn.apply(&q, nt, &q, nt, &q);
            } else {
                let qq = add(&q, &q_pe);
                let a = l.self_attn.apply(&qq, nt, &qq, nt, &q);
                q = add(&q, &a);
            }
            l.n1.apply(&mut q);
            let qq = add(&q, &q_pe);
            let kk = add(&keys, &k_pe);
            let a = l.t2i.apply(&qq, nt, &kk, nk, &keys);
            q = add(&q, &a);
            l.n2.apply(&mut q);
            let mut m = l.l1.apply(&q, nt);
            relu(&mut m);
            let m = l.l2.apply(&m, nt);
            q = add(&q, &m);
            l.n3.apply(&mut q);
            let qq = add(&q, &q_pe);
            let kk = add(&keys, &k_pe);
            let a = l.i2t.apply(&kk, nk, &qq, nt, &q);
            keys = add(&keys, &a);
            l.n4.apply(&mut keys);
        }
        let qq = add(&q, &q_pe);
        let kk = add(&keys, &k_pe);
        let a = self.final_attn.apply(&qq, nt, &kk, nk, &keys);
        q = add(&q, &a);
        self.norm_final.apply(&mut q);
        let mut up = conv_transpose(&keys, EMB, EMB, 256, &self.up1, 64);
        self.up_n.apply(&mut up);
        gelu(&mut up);
        let mut up = conv_transpose(&up, EMB * 2, EMB * 2, 64, &self.up2, 32);
        gelu(&mut up);
        let n = 256 * 256;
        let masks: Vec<Vec<f32>> = (0..4)
            .map(|i| {
                let hv = mlp3(&self.hyper[i], &q[(1 + i) * 256..(2 + i) * 256]);
                let mut m = vec![0.0f32; n];
                m.par_iter_mut().enumerate().for_each(|(p, v)| {
                    *v = up[p * 32..(p + 1) * 32].iter().zip(&hv).map(|(a, b)| a * b).sum();
                });
                m
            })
            .collect();
        let iou = mlp3(&self.iou_head, &q[0..256]);
        (masks, iou)
    }
}

pub fn prepare(img: &super::image::Img) -> (Vec<f32>, usize, usize, f64) {
    let k = SIZE as f64 / img.w.max(img.h).max(1) as f64;
    let (w, h) = (((img.w as f64 * k).round() as usize).clamp(1, SIZE), ((img.h as f64 * k).round() as usize).clamp(1, SIZE));
    let mut out = vec![0.0f32; w * h * 3];
    let (sx, sy) = (img.w as f64 / w as f64, img.h as f64 / h as f64);
    out.par_chunks_mut(w * 3).enumerate().for_each(|(y, row)| {
        let (y0, y1) = (y as f64 * sy, (y + 1) as f64 * sy);
        for x in 0..w {
            let (x0, x1) = (x as f64 * sx, (x + 1) as f64 * sx);
            let mut acc = [0.0f64; 3];
            let mut wsum = 0.0;
            if sx > 1.0 || sy > 1.0 {
                let mut yy = y0.floor();
                while yy < y1 {
                    let wy = (yy + 1.0).min(y1) - yy.max(y0);
                    let mut xx = x0.floor();
                    while xx < x1 {
                        let wx = (xx + 1.0).min(x1) - xx.max(x0);
                        let p = img.get((xx as isize).min(img.w as isize - 1), (yy as isize).min(img.h as isize - 1));
                        for c in 0..3 {
                            acc[c] += p[c] as f64 * wx * wy;
                        }
                        wsum += wx * wy;
                        xx += 1.0;
                    }
                    yy += 1.0;
                }
            } else {
                let p = img.sample((x as f64 + 0.5) * sx, (y as f64 + 0.5) * sy, true);
                for c in 0..3 {
                    acc[c] = p[c] as f64;
                }
                wsum = 1.0;
            }
            for c in 0..3 {
                row[x * 3 + c] = (acc[c] / wsum.max(1e-12) * 255.0) as f32;
            }
        }
    });
    (out, w, h, k)
}
