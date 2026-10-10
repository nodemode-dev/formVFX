use std::collections::BTreeMap;

use rayon::prelude::*;

use super::sam::{W, conv_transpose, gelu, gemm_packed, gemm_serial, get, pack_b, relu, Lin, Norm};

pub const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
pub const STD: [f32; 3] = [0.229, 0.224, 0.225];
const NUM_MASKMEM: usize = 7;
const MAX_PTRS: usize = 16;
const MEM_DIM: usize = 64;
const HID: usize = 256;

fn lin(w: &W, name: &str) -> Result<Lin, String> {
    Lin::new(w, name, true)
}

fn data(w: &W, name: &str) -> Result<Vec<f32>, String> {
    Ok(get(w, name)?.data.clone())
}

pub fn round_bf16(v: &mut [f32]) {
    for x in v.iter_mut() {
        let b = x.to_bits();
        let r = b.wrapping_add(0x7fff + ((b >> 16) & 1)) & 0xffff_0000;
        *x = f32::from_bits(r);
    }
}

fn add_into(a: &mut [f32], b: &[f32]) {
    a.par_iter_mut().zip(b.par_iter()).for_each(|(x, y)| *x += y);
}

fn softmax_row(r: &mut [f32]) {
    let mx = r.iter().copied().fold(f32::MIN, f32::max);
    let mut s = 0.0;
    for v in r.iter_mut() {
        *v = (*v - mx).exp();
        s += *v;
    }
    let k = 1.0 / s;
    for v in r.iter_mut() {
        *v *= k;
    }
}

fn split_head(x: &[f32], n: usize, d: usize, h: usize, hd: usize) -> Vec<f32> {
    let mut o = vec![0.0f32; n * hd];
    for i in 0..n {
        o[i * hd..(i + 1) * hd].copy_from_slice(&x[i * d + h * hd..i * d + (h + 1) * hd]);
    }
    o
}

fn attend(q: &[f32], k: &[f32], v: &[f32], nq: usize, nk: usize, heads: usize, d: usize) -> Vec<f32> {
    let hd = d / heads;
    let scale = 1.0 / (hd as f32).sqrt();
    let mut out = vec![0.0f32; nq * d];
    for h in 0..heads {
        let qh: Vec<f32> = split_head(q, nq, d, h, hd).into_iter().map(|x| x * scale).collect();
        let kh = split_head(k, nk, d, h, hd);
        let mut kt = vec![0.0f32; hd * nk];
        for j in 0..nk {
            for c in 0..hd {
                kt[c * nk + j] = kh[j * hd + c];
            }
        }
        let vh = split_head(v, nk, d, h, hd);
        let kp = pack_b(hd, nk, &kt);
        let vp = pack_b(nk, hd, &vh);
        let blk = 256usize;
        let mut i0 = 0;
        while i0 < nq {
            let rows = blk.min(nq - i0);
            let mut s = vec![0.0f32; rows * nk];
            gemm_packed(rows, &qh[i0 * hd..(i0 + rows) * hd], &kp, &mut s);
            s.par_chunks_mut(nk).for_each(softmax_row);
            let mut o = vec![0.0f32; rows * hd];
            gemm_packed(rows, &s, &vp, &mut o);
            for r in 0..rows {
                out[(i0 + r) * d + h * hd..(i0 + r) * d + (h + 1) * hd].copy_from_slice(&o[r * hd..(r + 1) * hd]);
            }
            i0 += rows;
        }
    }
    out
}

fn attend_small(q: &[f32], k: &[f32], v: &[f32], nq: usize, nk: usize, heads: usize, d: usize, out: &mut [f32]) {
    let hd = d / heads;
    let scale = 1.0 / (hd as f32).sqrt();
    let mut qh = vec![0.0f32; nq * hd];
    let mut kt = vec![0.0f32; hd * nk];
    let mut vh = vec![0.0f32; nk * hd];
    let mut s = vec![0.0f32; nq * nk];
    let mut o = vec![0.0f32; nq * hd];
    for h in 0..heads {
        for i in 0..nq {
            for c in 0..hd {
                qh[i * hd + c] = q[i * d + h * hd + c] * scale;
            }
        }
        for j in 0..nk {
            for c in 0..hd {
                kt[c * nk + j] = k[j * d + h * hd + c];
                vh[j * hd + c] = v[j * d + h * hd + c];
            }
        }
        s.iter_mut().for_each(|x| *x = 0.0);
        gemm_serial(nq, hd, nk, &qh, &kt, &mut s);
        s.chunks_mut(nk).for_each(softmax_row);
        o.iter_mut().for_each(|x| *x = 0.0);
        gemm_serial(nq, nk, hd, &s, &vh, &mut o);
        for i in 0..nq {
            out[i * d + h * hd..i * d + (h + 1) * hd].copy_from_slice(&o[i * hd..(i + 1) * hd]);
        }
    }
}

fn cubic_w(t: f32) -> [f32; 4] {
    let a = -0.75f32;
    let w = |x: f32| {
        let x = x.abs();
        if x <= 1.0 {
            ((a + 2.0) * x - (a + 3.0)) * x * x + 1.0
        } else if x < 2.0 {
            ((a * x - 5.0 * a) * x + 8.0 * a) * x - 4.0 * a
        } else {
            0.0
        }
    };
    [w(t + 1.0), w(t), w(1.0 - t), w(2.0 - t)]
}

fn bicubic(src: &[f32], c: usize, ih: usize, iw: usize, oh: usize, ow: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; oh * ow * c];
    let (sy, sx) = (ih as f32 / oh as f32, iw as f32 / ow as f32);
    for y in 0..oh {
        let fy = (y as f32 + 0.5) * sy - 0.5;
        let y0 = fy.floor();
        let wy = cubic_w(fy - y0);
        for x in 0..ow {
            let fx = (x as f32 + 0.5) * sx - 0.5;
            let x0 = fx.floor();
            let wx = cubic_w(fx - x0);
            for ch in 0..c {
                let mut acc = 0.0;
                for (a, wya) in wy.iter().enumerate() {
                    let yy = (y0 as isize + a as isize - 1).clamp(0, ih as isize - 1) as usize;
                    for (b, wxb) in wx.iter().enumerate() {
                        let xx = (x0 as isize + b as isize - 1).clamp(0, iw as isize - 1) as usize;
                        acc += wya * wxb * src[(yy * iw + xx) * c + ch];
                    }
                }
                out[(y * ow + x) * c + ch] = acc;
            }
        }
    }
    out
}

pub fn bilinear(src: &[f32], ih: usize, iw: usize, oh: usize, ow: usize) -> Vec<f32> {
    let (sy, sx) = (ih as f32 / oh as f32, iw as f32 / ow as f32);
    let mut out = vec![0.0f32; oh * ow];
    out.par_chunks_mut(ow).enumerate().for_each(|(y, row)| {
        let fy = ((y as f32 + 0.5) * sy - 0.5).max(0.0);
        let y0 = (fy as usize).min(ih - 1);
        let y1 = (y0 + 1).min(ih - 1);
        let ty = fy - y0 as f32;
        for (x, o) in row.iter_mut().enumerate() {
            let fx = ((x as f32 + 0.5) * sx - 0.5).max(0.0);
            let x0 = (fx as usize).min(iw - 1);
            let x1 = (x0 + 1).min(iw - 1);
            let tx = fx - x0 as f32;
            let a = src[y0 * iw + x0] * (1.0 - tx) + src[y0 * iw + x1] * tx;
            let b = src[y1 * iw + x0] * (1.0 - tx) + src[y1 * iw + x1] * tx;
            *o = a * (1.0 - ty) + b * ty;
        }
    });
    out
}

fn aa_weights(inn: usize, out: usize, cubic: bool) -> Vec<(usize, Vec<f32>)> {
    let scale = inn as f32 / out as f32;
    let support = if cubic { 2.0 } else { 1.0 } * scale.max(1.0);
    let fscale = scale.max(1.0);
    let filt = |x: f32| {
        let x = x.abs();
        if cubic {
            let a = -0.5f32;
            if x < 1.0 {
                ((a + 2.0) * x - (a + 3.0)) * x * x + 1.0
            } else if x < 2.0 {
                ((a * x - 5.0 * a) * x + 8.0 * a) * x - 4.0 * a
            } else {
                0.0
            }
        } else if x < 1.0 {
            1.0 - x
        } else {
            0.0
        }
    };
    (0..out)
        .map(|o| {
            let center = (o as f32 + 0.5) * scale;
            let xmin = ((center - support + 0.5).floor().max(0.0)) as usize;
            let xmax = ((center + support + 0.5).floor() as usize).min(inn);
            let mut w: Vec<f32> = (xmin..xmax).map(|j| filt((j as f32 - center + 0.5) / fscale)).collect();
            let t: f32 = w.iter().sum();
            if t != 0.0 {
                w.iter_mut().for_each(|v| *v /= t);
            }
            (xmin, w)
        })
        .collect()
}

pub fn resample_aa(src: &[f32], c: usize, ih: usize, iw: usize, oh: usize, ow: usize, cubic: bool) -> Vec<f32> {
    let wx = aa_weights(iw, ow, cubic);
    let wy = aa_weights(ih, oh, cubic);
    let mut tmp = vec![0.0f32; ih * ow * c];
    tmp.par_chunks_mut(ow * c).enumerate().for_each(|(y, row)| {
        for (x, (x0, ws)) in wx.iter().enumerate() {
            for ch in 0..c {
                let mut a = 0.0;
                for (k, w) in ws.iter().enumerate() {
                    a += w * src[(y * iw + x0 + k) * c + ch];
                }
                row[x * c + ch] = a;
            }
        }
    });
    let mut out = vec![0.0f32; oh * ow * c];
    out.par_chunks_mut(ow * c).enumerate().for_each(|(y, row)| {
        let (y0, ws) = &wy[y];
        for x in 0..ow {
            for ch in 0..c {
                let mut a = 0.0;
                for (k, w) in ws.iter().enumerate() {
                    a += w * tmp[((y0 + k) * ow + x) * c + ch];
                }
                row[x * c + ch] = a;
            }
        }
    });
    out
}

struct Conv {
    lin: Lin,
    cin: usize,
    k: usize,
    stride: usize,
    pad: usize,
}

impl Conv {
    fn new(w: &W, name: &str, stride: usize, pad: usize) -> Result<Conv, String> {
        let t = get(w, &format!("{name}.weight"))?;
        let (out, cin, k) = (t.shape[0], t.shape[1], t.shape[2]);
        let inp = cin * k * k;
        let mut wt = vec![0.0f32; inp * out];
        for o in 0..out {
            for ci in 0..cin {
                for ky in 0..k {
                    for kx in 0..k {
                        wt[((ky * k + kx) * cin + ci) * out + o] = t.data[((o * cin + ci) * k + ky) * k + kx];
                    }
                }
            }
        }
        Ok(Conv { lin: Lin { wt, b: data(w, &format!("{name}.bias"))?, inp, out }, cin, k, stride, pad })
    }

    fn apply(&self, x: &[f32], h: usize, w: usize) -> (Vec<f32>, usize, usize) {
        let (k, s, p, c) = (self.k, self.stride, self.pad, self.cin);
        let oh = (h + 2 * p - k) / s + 1;
        let ow = (w + 2 * p - k) / s + 1;
        if k == 1 && s == 1 {
            return (self.lin.apply(x, h * w), h, w);
        }
        let kk = k * k * c;
        let mut cols = vec![0.0f32; oh * ow * kk];
        cols.par_chunks_mut(ow * kk).enumerate().for_each(|(oy, row)| {
            for ox in 0..ow {
                for ky in 0..k {
                    let iy = (oy * s + ky) as isize - p as isize;
                    if iy < 0 || iy >= h as isize {
                        continue;
                    }
                    for kx in 0..k {
                        let ix = (ox * s + kx) as isize - p as isize;
                        if ix < 0 || ix >= w as isize {
                            continue;
                        }
                        let src = (iy as usize * w + ix as usize) * c;
                        let dst = ox * kk + (ky * k + kx) * c;
                        row[dst..dst + c].copy_from_slice(&x[src..src + c]);
                    }
                }
            }
        });
        (self.lin.apply(&cols, oh * ow), oh, ow)
    }
}

struct Ln2d {
    g: Vec<f32>,
    b: Vec<f32>,
}

impl Ln2d {
    fn new(w: &W, name: &str) -> Result<Ln2d, String> {
        Ok(Ln2d { g: data(w, &format!("{name}.weight"))?, b: data(w, &format!("{name}.bias"))? })
    }

    fn apply(&self, x: &mut [f32]) {
        super::sam::layer_norm(x, self.g.len(), &self.g, &self.b, 1e-6);
    }
}

struct Mlp {
    layers: Vec<Lin>,
}

impl Mlp {
    fn new(w: &W, name: &str, n: usize) -> Result<Mlp, String> {
        Ok(Mlp { layers: (0..n).map(|i| lin(w, &format!("{name}.layers.{i}"))).collect::<Result<_, _>>()? })
    }

    fn apply(&self, x: &[f32], rows: usize, act_gelu: bool) -> Vec<f32> {
        let mut y = x.to_vec();
        let n = self.layers.len();
        for (i, l) in self.layers.iter().enumerate() {
            y = l.apply(&y, rows);
            if i + 1 < n {
                if act_gelu { gelu(&mut y) } else { relu(&mut y) }
            }
        }
        y
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
    fn new(w: &W, name: &str, heads: usize) -> Result<Attn, String> {
        Ok(Attn { q: lin(w, &format!("{name}.q_proj"))?, k: lin(w, &format!("{name}.k_proj"))?, v: lin(w, &format!("{name}.v_proj"))?, o: lin(w, &format!("{name}.out_proj"))?, heads })
    }

    fn apply(&self, q: &[f32], nq: usize, k: &[f32], nk: usize, v: &[f32]) -> Vec<f32> {
        let (qp, kp, vp) = (self.q.apply(q, nq), self.k.apply(k, nk), self.v.apply(v, nk));
        let a = attend(&qp, &kp, &vp, nq, nk, self.heads, self.q.out);
        self.o.apply(&a, nq)
    }

    fn apply_rope(&self, q: &[f32], nq: usize, k: &[f32], nk: usize, v: &[f32], rope: &Rope, exclude: usize) -> Vec<f32> {
        let (mut qp, mut kp, vp) = (self.q.apply(q, nq), self.k.apply(k, nk), self.v.apply(v, nk));
        let d = self.q.out;
        rope.apply(&mut qp, nq, d);
        let nr = nk - exclude;
        rope.apply(&mut kp[..nr * d], nr, d);
        let a = attend(&qp, &kp, &vp, nq, nk, self.heads, d);
        self.o.apply(&a, nq)
    }
}

struct Rope {
    cos: Vec<f32>,
    sin: Vec<f32>,
    n: usize,
    half: usize,
}

impl Rope {
    fn new(side: usize, dim: usize) -> Rope {
        let half = dim / 2;
        let q = dim / 4;
        let n = side * side;
        let (mut cos, mut sin) = (vec![0.0f32; n * half], vec![0.0f32; n * half]);
        for p in 0..n {
            let (tx, ty) = ((p % side) as f64, (p / side) as f64);
            for i in 0..q {
                let f = 1.0 / 10000f64.powf((4 * i) as f64 / dim as f64);
                let (ax, ay) = (tx * f, ty * f);
                cos[p * half + i] = ax.cos() as f32;
                sin[p * half + i] = ax.sin() as f32;
                cos[p * half + q + i] = ay.cos() as f32;
                sin[p * half + q + i] = ay.sin() as f32;
            }
        }
        Rope { cos, sin, n, half }
    }

    fn apply(&self, x: &mut [f32], rows: usize, d: usize) {
        x.par_chunks_mut(d).enumerate().take(rows).for_each(|(r, row)| {
            let p = r % self.n;
            for j in 0..self.half {
                let (c, s) = (self.cos[p * self.half + j], self.sin[p * self.half + j]);
                let (a, b) = (row[2 * j], row[2 * j + 1]);
                row[2 * j] = a * c - b * s;
                row[2 * j + 1] = a * s + b * c;
            }
        });
    }
}

struct HBlock {
    dim: usize,
    out: usize,
    heads: usize,
    window: usize,
    pool: bool,
    norm1: Norm,
    qkv: Lin,
    proj_attn: Lin,
    norm2: Norm,
    mlp: Mlp,
    proj: Option<Lin>,
}

fn maxpool2(x: &[f32], h: usize, w: usize, c: usize) -> Vec<f32> {
    let (oh, ow) = (h / 2, w / 2);
    let mut o = vec![0.0f32; oh * ow * c];
    o.par_chunks_mut(ow * c).enumerate().for_each(|(y, row)| {
        for xx in 0..ow {
            for ch in 0..c {
                let a = x[((2 * y) * w + 2 * xx) * c + ch];
                let b = x[((2 * y) * w + 2 * xx + 1) * c + ch];
                let cc = x[((2 * y + 1) * w + 2 * xx) * c + ch];
                let d = x[((2 * y + 1) * w + 2 * xx + 1) * c + ch];
                row[xx * c + ch] = a.max(b).max(cc).max(d);
            }
        }
    });
    o
}

impl HBlock {
    fn apply(&self, x: &[f32], h: usize, w: usize) -> (Vec<f32>, usize, usize) {
        let (c, co) = (self.dim, self.out);
        let mut xn = x.to_vec();
        self.norm1.apply(&mut xn);
        let (h2, w2) = if self.pool { (h / 2, w / 2) } else { (h, w) };
        let shortcut = match &self.proj {
            Some(p) => {
                let y = p.apply(&xn, h * w);
                if self.pool { maxpool2(&y, h, w, co) } else { y }
            }
            None => x.to_vec(),
        };
        let qkv = self.qkv.apply(&xn, h * w);
        let attn = if self.window == 0 {
            let n = h * w;
            let mut q = vec![0.0f32; n * co];
            let mut k = vec![0.0f32; n * co];
            let mut v = vec![0.0f32; n * co];
            for i in 0..n {
                q[i * co..(i + 1) * co].copy_from_slice(&qkv[i * 3 * co..i * 3 * co + co]);
                k[i * co..(i + 1) * co].copy_from_slice(&qkv[i * 3 * co + co..i * 3 * co + 2 * co]);
                v[i * co..(i + 1) * co].copy_from_slice(&qkv[i * 3 * co + 2 * co..(i + 1) * 3 * co]);
            }
            attend(&q, &k, &v, n, n, self.heads, co)
        } else {
            let ws = self.window;
            let hp = h.div_ceil(ws) * ws;
            let wp = w.div_ceil(ws) * ws;
            let (ny, nx) = (hp / ws, wp / ws);
            let ws2 = if self.pool { ws / 2 } else { ws };
            let (hp2, wp2) = (ny * ws2, nx * ws2);
            let bias = &self.qkv.b;
            let nk = ws * ws;
            let nq = ws2 * ws2;
            let mut out = vec![0.0f32; hp2 * wp2 * co];
            let wins: Vec<Vec<f32>> = (0..ny * nx)
                .into_par_iter()
                .map(|wi| {
                    let (wy, wx) = (wi / nx, wi % nx);
                    let mut q = vec![0.0f32; nk * co];
                    let mut k = vec![0.0f32; nk * co];
                    let mut v = vec![0.0f32; nk * co];
                    for ty in 0..ws {
                        for tx in 0..ws {
                            let (yy, xx) = (wy * ws + ty, wx * ws + tx);
                            let t = ty * ws + tx;
                            let src: &[f32] = if yy < h && xx < w { &qkv[(yy * w + xx) * 3 * co..(yy * w + xx + 1) * 3 * co] } else { bias };
                            q[t * co..(t + 1) * co].copy_from_slice(&src[..co]);
                            k[t * co..(t + 1) * co].copy_from_slice(&src[co..2 * co]);
                            v[t * co..(t + 1) * co].copy_from_slice(&src[2 * co..]);
                        }
                    }
                    let q = if self.pool { maxpool2(&q, ws, ws, co) } else { q };
                    let mut o = vec![0.0f32; nq * co];
                    attend_small(&q, &k, &v, nq, nk, self.heads, co, &mut o);
                    o
                })
                .collect();
            for (wi, o) in wins.iter().enumerate() {
                let (wy, wx) = (wi / nx, wi % nx);
                for ty in 0..ws2 {
                    for tx in 0..ws2 {
                        let (yy, xx) = (wy * ws2 + ty, wx * ws2 + tx);
                        let t = ty * ws2 + tx;
                        out[(yy * wp2 + xx) * co..(yy * wp2 + xx + 1) * co].copy_from_slice(&o[t * co..(t + 1) * co]);
                    }
                }
            }
            let mut crop = vec![0.0f32; h2 * w2 * co];
            for y in 0..h2 {
                crop[y * w2 * co..(y + 1) * w2 * co].copy_from_slice(&out[y * wp2 * co..(y * wp2 + w2) * co]);
            }
            crop
        };
        let _ = c;
        let a = self.proj_attn.apply(&attn, h2 * w2);
        let mut x = shortcut;
        add_into(&mut x, &a);
        let mut m = x.clone();
        self.norm2.apply(&mut m);
        let m = self.mlp.apply(&m, h2 * w2, true);
        add_into(&mut x, &m);
        (x, h2, w2)
    }
}

pub struct Feats {
    pub side: usize,
    pub hr0: Vec<f32>,
    pub hr1: Vec<f32>,
    pub vis: Vec<f32>,
}

#[derive(Clone)]
pub struct Memory {
    pub feat: Vec<f32>,
    pub ptr: Vec<f32>,
    pub cond: bool,
    pub appears: bool,
}

pub struct Sam2 {
    patch: Conv,
    pos_embed: Vec<f32>,
    pos_window: Vec<f32>,
    blocks: Vec<HBlock>,
    stage_ends: Vec<usize>,
    neck: Vec<Lin>,
    conv_s0: Lin,
    conv_s1: Lin,
    gauss: Vec<f32>,
    point_emb: Vec<Vec<f32>>,
    not_a_point: Vec<f32>,
    no_mask: Vec<f32>,
    md_c0: Conv,
    md_n0: Ln2d,
    md_c1: Conv,
    md_n1: Ln2d,
    md_c2: Lin,
    tw: Vec<TwoWay>,
    final_attn: Attn,
    norm_final: Norm,
    obj_token: Vec<f32>,
    iou_token: Vec<f32>,
    mask_tokens: Vec<f32>,
    up1: (Vec<f32>, Vec<f32>),
    up_n: Ln2d,
    up2: (Vec<f32>, Vec<f32>),
    hyper: Vec<Mlp>,
    iou_head: Mlp,
    obj_head: Mlp,
    mask_downsample: Conv,
    obj_ptr_proj: Mlp,
    obj_ptr_tpos: Lin,
    no_obj_ptr: Vec<f32>,
    no_mem_embed: Vec<f32>,
    no_obj_spatial: Vec<f32>,
    maskmem_tpos: Vec<f32>,
    me_convs: Vec<(Conv, Ln2d)>,
    me_last: Lin,
    me_pix: Lin,
    me_fuser: Vec<CxBlock>,
    me_out: Lin,
    ma_layers: Vec<MaLayer>,
    ma_norm: Norm,
}

struct TwoWay {
    self_attn: Attn,
    n1: Norm,
    t2i: Attn,
    n2: Norm,
    mlp: Mlp,
    n3: Norm,
    n4: Norm,
    i2t: Attn,
}

struct CxBlock {
    dw: Vec<f32>,
    dwb: Vec<f32>,
    norm: Ln2d,
    pw1: Lin,
    pw2: Lin,
    gamma: Vec<f32>,
}

impl CxBlock {
    fn apply(&self, x: &mut [f32], h: usize, w: usize, c: usize) {
        let mut y = vec![0.0f32; h * w * c];
        y.par_chunks_mut(w * c).enumerate().for_each(|(oy, row)| {
            for ox in 0..w {
                let o = &mut row[ox * c..(ox + 1) * c];
                o.copy_from_slice(&self.dwb);
                for ky in 0..7 {
                    let iy = oy as isize + ky as isize - 3;
                    if iy < 0 || iy >= h as isize {
                        continue;
                    }
                    for kx in 0..7 {
                        let ix = ox as isize + kx as isize - 3;
                        if ix < 0 || ix >= w as isize {
                            continue;
                        }
                        let src = &x[(iy as usize * w + ix as usize) * c..(iy as usize * w + ix as usize + 1) * c];
                        let wk = &self.dw[(ky * 7 + kx) * c..(ky * 7 + kx + 1) * c];
                        for ((ov, sv), wv) in o.iter_mut().zip(src).zip(wk) {
                            *ov += sv * wv;
                        }
                    }
                }
            }
        });
        self.norm.apply(&mut y);
        let mut a = self.pw1.apply(&y, h * w);
        gelu(&mut a);
        let mut b = self.pw2.apply(&a, h * w);
        b.par_chunks_mut(c).for_each(|r| r.iter_mut().zip(&self.gamma).for_each(|(v, g)| *v *= g));
        add_into(x, &b);
    }
}

struct MaLayer {
    self_attn: Attn,
    cross: Attn,
    l1: Lin,
    l2: Lin,
    n1: Norm,
    n2: Norm,
    n3: Norm,
}

pub fn sine_pe(side: usize, feats: usize) -> Vec<f32> {
    let npf = feats / 2;
    let scale = std::f32::consts::TAU;
    let mut out = vec![0.0f32; side * side * feats];
    let dim_t: Vec<f32> = (0..npf).map(|j| 10000f32.powf(2.0 * (j / 2) as f32 / npf as f32)).collect();
    for y in 0..side {
        for x in 0..side {
            let ye = (y + 1) as f32 / (side as f32 + 1e-6) * scale;
            let xe = (x + 1) as f32 / (side as f32 + 1e-6) * scale;
            let o = &mut out[(y * side + x) * feats..(y * side + x + 1) * feats];
            for j in 0..npf {
                let (py, px) = (ye / dim_t[j], xe / dim_t[j]);
                o[j] = if j % 2 == 0 { py.sin() } else { py.cos() };
                o[npf + j] = if j % 2 == 0 { px.sin() } else { px.cos() };
            }
        }
    }
    out
}

fn sine_1d(pos: f32, dim: usize) -> Vec<f32> {
    let pe = dim / 2;
    let mut out = vec![0.0f32; dim];
    for j in 0..pe {
        let t = 10000f32.powf(2.0 * (j / 2) as f32 / pe as f32);
        out[j] = (pos / t).sin();
        out[pe + j] = (pos / t).cos();
    }
    out
}

impl Sam2 {
    pub fn load(w: &W) -> Result<Sam2, String> {
        let t = "image_encoder.trunk";
        let stages = [1usize, 2, 7, 2];
        let windows = [8usize, 4, 14, 7];
        let global = [5usize, 7, 9];
        let ends: Vec<usize> = (1..=4).map(|i| stages[..i].iter().sum::<usize>() - 1).collect();
        let pool_blocks: Vec<usize> = ends[..3].iter().map(|e| e + 1).collect();
        let mut blocks = vec![];
        let (mut dim, mut heads, mut stage) = (96usize, 1usize, 1usize);
        for i in 0..stages.iter().sum::<usize>() {
            let mut out = dim;
            let mut window = windows[stage - 1];
            if global.contains(&i) {
                window = 0;
            }
            if i >= 1 && ends.contains(&(i - 1)) {
                out = dim * 2;
                heads *= 2;
                stage += 1;
            }
            let n = format!("{t}.blocks.{i}");
            blocks.push(HBlock {
                dim,
                out,
                heads,
                window,
                pool: pool_blocks.contains(&i),
                norm1: Norm::new(w, &format!("{n}.norm1"), 1e-6)?,
                qkv: lin(w, &format!("{n}.attn.qkv"))?,
                proj_attn: lin(w, &format!("{n}.attn.proj"))?,
                norm2: Norm::new(w, &format!("{n}.norm2"), 1e-6)?,
                mlp: Mlp::new(w, &format!("{n}.mlp"), 2)?,
                proj: if dim != out { Some(lin(w, &format!("{n}.proj"))?) } else { None },
            });
            dim = out;
        }
        let neck = (0..4).map(|i| lin(w, &format!("image_encoder.neck.convs.{i}.conv"))).collect::<Result<Vec<_>, _>>()?;
        let pp = "sam_prompt_encoder";
        let dd = "sam_mask_decoder";
        let mut tw = vec![];
        for i in 0..2 {
            let n = format!("{dd}.transformer.layers.{i}");
            tw.push(TwoWay {
                self_attn: Attn::new(w, &format!("{n}.self_attn"), 8)?,
                n1: Norm::new(w, &format!("{n}.norm1"), 1e-5)?,
                t2i: Attn::new(w, &format!("{n}.cross_attn_token_to_image"), 8)?,
                n2: Norm::new(w, &format!("{n}.norm2"), 1e-5)?,
                mlp: Mlp::new(w, &format!("{n}.mlp"), 2)?,
                n3: Norm::new(w, &format!("{n}.norm3"), 1e-5)?,
                n4: Norm::new(w, &format!("{n}.norm4"), 1e-5)?,
                i2t: Attn::new(w, &format!("{n}.cross_attn_image_to_token"), 8)?,
            });
        }
        let me = "memory_encoder";
        let mut me_convs = vec![];
        for (ci, ni) in [(0, 1), (3, 4), (6, 7), (9, 10)] {
            me_convs.push((Conv::new(w, &format!("{me}.mask_downsampler.encoder.{ci}"), 2, 1)?, Ln2d::new(w, &format!("{me}.mask_downsampler.encoder.{ni}"))?));
        }
        let mut me_fuser = vec![];
        for i in 0..2 {
            let n = format!("{me}.fuser.layers.{i}");
            let dwt = get(w, &format!("{n}.dwconv.weight"))?;
            let c = dwt.shape[0];
            let mut dw = vec![0.0f32; 49 * c];
            for ch in 0..c {
                for k in 0..49 {
                    dw[k * c + ch] = dwt.data[ch * 49 + k];
                }
            }
            me_fuser.push(CxBlock { dw, dwb: data(w, &format!("{n}.dwconv.bias"))?, norm: Ln2d::new(w, &format!("{n}.norm"))?, pw1: lin(w, &format!("{n}.pwconv1"))?, pw2: lin(w, &format!("{n}.pwconv2"))?, gamma: data(w, &format!("{n}.gamma"))? });
        }
        let mut ma_layers = vec![];
        for i in 0..4 {
            let n = format!("memory_attention.layers.{i}");
            ma_layers.push(MaLayer {
                self_attn: Attn::new(w, &format!("{n}.self_attn"), 1)?,
                cross: Attn::new(w, &format!("{n}.cross_attn_image"), 1)?,
                l1: lin(w, &format!("{n}.linear1"))?,
                l2: lin(w, &format!("{n}.linear2"))?,
                n1: Norm::new(w, &format!("{n}.norm1"), 1e-5)?,
                n2: Norm::new(w, &format!("{n}.norm2"), 1e-5)?,
                n3: Norm::new(w, &format!("{n}.norm3"), 1e-5)?,
            });
        }
        let pos_embed = get(w, &format!("{t}.pos_embed"))?;
        let pe_hwc = |tt: &super::ptload::Tensor| -> Vec<f32> {
            let (c, h, ww) = (tt.shape[1], tt.shape[2], tt.shape[3]);
            let mut o = vec![0.0f32; h * ww * c];
            for ch in 0..c {
                for y in 0..h {
                    for x in 0..ww {
                        o[(y * ww + x) * c + ch] = tt.data[(ch * h + y) * ww + x];
                    }
                }
            }
            o
        };
        Ok(Sam2 {
            patch: Conv::new(w, &format!("{t}.patch_embed.proj"), 4, 3)?,
            pos_embed: pe_hwc(pos_embed),
            pos_window: pe_hwc(get(w, &format!("{t}.pos_embed_window"))?),
            blocks,
            stage_ends: ends,
            neck,
            conv_s0: lin(w, &format!("{dd}.conv_s0"))?,
            conv_s1: lin(w, &format!("{dd}.conv_s1"))?,
            gauss: data(w, &format!("{pp}.pe_layer.positional_encoding_gaussian_matrix"))?,
            point_emb: (0..4).map(|i| data(w, &format!("{pp}.point_embeddings.{i}.weight"))).collect::<Result<_, _>>()?,
            not_a_point: data(w, &format!("{pp}.not_a_point_embed.weight"))?,
            no_mask: data(w, &format!("{pp}.no_mask_embed.weight"))?,
            md_c0: Conv::new(w, &format!("{pp}.mask_downscaling.0"), 2, 0)?,
            md_n0: Ln2d::new(w, &format!("{pp}.mask_downscaling.1"))?,
            md_c1: Conv::new(w, &format!("{pp}.mask_downscaling.3"), 2, 0)?,
            md_n1: Ln2d::new(w, &format!("{pp}.mask_downscaling.4"))?,
            md_c2: lin(w, &format!("{pp}.mask_downscaling.6"))?,
            tw,
            final_attn: Attn::new(w, &format!("{dd}.transformer.final_attn_token_to_image"), 8)?,
            norm_final: Norm::new(w, &format!("{dd}.transformer.norm_final_attn"), 1e-5)?,
            obj_token: data(w, &format!("{dd}.obj_score_token.weight"))?,
            iou_token: data(w, &format!("{dd}.iou_token.weight"))?,
            mask_tokens: data(w, &format!("{dd}.mask_tokens.weight"))?,
            up1: (data(w, &format!("{dd}.output_upscaling.0.weight"))?, data(w, &format!("{dd}.output_upscaling.0.bias"))?),
            up_n: Ln2d::new(w, &format!("{dd}.output_upscaling.1"))?,
            up2: (data(w, &format!("{dd}.output_upscaling.3.weight"))?, data(w, &format!("{dd}.output_upscaling.3.bias"))?),
            hyper: (0..4).map(|i| Mlp::new(w, &format!("{dd}.output_hypernetworks_mlps.{i}"), 3)).collect::<Result<_, _>>()?,
            iou_head: Mlp::new(w, &format!("{dd}.iou_prediction_head"), 3)?,
            obj_head: Mlp::new(w, &format!("{dd}.pred_obj_score_head"), 3)?,
            mask_downsample: Conv::new(w, "mask_downsample", 4, 0)?,
            obj_ptr_proj: Mlp::new(w, "obj_ptr_proj", 3)?,
            obj_ptr_tpos: lin(w, "obj_ptr_tpos_proj")?,
            no_obj_ptr: data(w, "no_obj_ptr")?,
            no_mem_embed: data(w, "no_mem_embed")?,
            no_obj_spatial: data(w, "no_obj_embed_spatial")?,
            maskmem_tpos: data(w, "maskmem_tpos_enc")?,
            me_convs,
            me_last: lin(w, &format!("{me}.mask_downsampler.encoder.12"))?,
            me_pix: lin(w, &format!("{me}.pix_feat_proj"))?,
            me_fuser,
            me_out: lin(w, &format!("{me}.out_proj"))?,
            ma_layers,
            ma_norm: Norm::new(w, "memory_attention.norm", 1e-5)?,
        })
    }

    pub fn encode(&self, img: &[f32], s: usize) -> Feats {
        let (mut x, mut h, mut w) = self.patch.apply(img, s, s);
        let c0 = 96;
        let pe = bicubic(&self.pos_embed, c0, 7, 7, h, w);
        x.par_chunks_mut(w * c0).enumerate().for_each(|(y, row)| {
            for xx in 0..w {
                for ch in 0..c0 {
                    row[xx * c0 + ch] += pe[(y * w + xx) * c0 + ch] + self.pos_window[((y % 8) * 8 + xx % 8) * c0 + ch];
                }
            }
        });
        let mut feats: Vec<(Vec<f32>, usize, usize, usize)> = vec![];
        for (i, b) in self.blocks.iter().enumerate() {
            let (nx, nh, nw) = b.apply(&x, h, w);
            x = nx;
            h = nh;
            w = nw;
            if self.stage_ends.contains(&i) {
                feats.push((x.clone(), h, w, b.out));
            }
        }
        let lat = |i: usize, j: usize| self.neck[j].apply(&feats[i].0, feats[i].1 * feats[i].2);
        let top = lat(3, 0);
        let (h3, w3) = (feats[3].1, feats[3].2);
        let mut l2 = lat(2, 1);
        let (h2, w2) = (feats[2].1, feats[2].2);
        l2.par_chunks_mut(HID).enumerate().for_each(|(i, r)| {
            let (y, xx) = (i / w2, i % w2);
            let src = &top[((y / 2).min(h3 - 1) * w3 + (xx / 2).min(w3 - 1)) * HID..][..HID];
            r.iter_mut().zip(src).for_each(|(a, b)| *a += b);
        });
        let l1 = lat(1, 2);
        let l0 = lat(0, 3);
        Feats { side: h2, hr0: self.conv_s0.apply(&l0, feats[0].1 * feats[0].2), hr1: self.conv_s1.apply(&l1, feats[1].1 * feats[1].2), vis: l2 }
    }

    fn pe(&self, x: f32, y: f32) -> Vec<f32> {
        let (cx, cy) = (2.0 * x - 1.0, 2.0 * y - 1.0);
        let mut out = vec![0.0f32; HID];
        for j in 0..128 {
            let v = (cx * self.gauss[j] + cy * self.gauss[128 + j]) * std::f32::consts::TAU;
            out[j] = v.sin();
            out[128 + j] = v.cos();
        }
        out
    }

    fn dense_pe(&self, side: usize) -> Vec<f32> {
        let mut out = vec![0.0f32; side * side * HID];
        out.par_chunks_mut(HID).enumerate().for_each(|(i, o)| {
            let (y, x) = (i / side, i % side);
            o.copy_from_slice(&self.pe((x as f32 + 0.5) / side as f32, (y as f32 + 0.5) / side as f32));
        });
        out
    }

    fn heads(&self, f: &Feats, pix: &[f32], points: &[([f32; 2], i32)], mask: Option<&[f32]>, multimask: bool, s: usize) -> (Vec<f32>, Vec<f32>, f32) {
        let side = f.side;
        let mut sparse: Vec<f32> = vec![];
        let mut pts = points.to_vec();
        pts.push(([0.0, 0.0], -1));
        for (p, l) in &pts {
            let mut e = self.pe((p[0] + 0.5) / s as f32, (p[1] + 0.5) / s as f32);
            if *l == -1 {
                e = self.not_a_point.clone();
            } else if (0..4).contains(l) {
                e.iter_mut().zip(&self.point_emb[*l as usize]).for_each(|(a, b)| *a += b);
            }
            sparse.extend(e);
        }
        let dense: Vec<f32> = match mask {
            Some(m) => {
                let ms = side * 4;
                let (mut y, h1, w1) = self.md_c0.apply(m, ms, ms);
                self.md_n0.apply(&mut y);
                gelu(&mut y);
                let (mut y, h2, w2) = self.md_c1.apply(&y, h1, w1);
                self.md_n1.apply(&mut y);
                gelu(&mut y);
                self.md_c2.apply(&y, h2 * w2)
            }
            None => (0..side * side).flat_map(|_| self.no_mask.iter().copied()).collect(),
        };
        let mut tokens: Vec<f32> = vec![];
        tokens.extend(&self.obj_token);
        tokens.extend(&self.iou_token);
        tokens.extend(&self.mask_tokens);
        tokens.extend(&sparse);
        let nt = tokens.len() / HID;
        let q_pe = tokens.clone();
        let mut keys: Vec<f32> = pix.iter().zip(&dense).map(|(a, b)| a + b).collect();
        let k_pe = self.dense_pe(side);
        let nk = side * side;
        let add = |a: &[f32], b: &[f32]| -> Vec<f32> { a.iter().zip(b).map(|(x, y)| x + y).collect() };
        let mut q = tokens.clone();
        for (li, l) in self.tw.iter().enumerate() {
            if li == 0 {
                q = l.self_attn.apply(&q, nt, &q, nt, &q);
            } else {
                let qq = add(&q, &q_pe);
                let a = l.self_attn.apply(&qq, nt, &qq, nt, &q);
                q = add(&q, &a);
            }
            l.n1.apply(&mut q);
            let a = l.t2i.apply(&add(&q, &q_pe), nt, &add(&keys, &k_pe), nk, &keys);
            q = add(&q, &a);
            l.n2.apply(&mut q);
            let m = l.mlp.apply(&q, nt, false);
            q = add(&q, &m);
            l.n3.apply(&mut q);
            let a = l.i2t.apply(&add(&keys, &k_pe), nk, &add(&q, &q_pe), nt, &q);
            keys = add(&keys, &a);
            l.n4.apply(&mut keys);
        }
        let a = self.final_attn.apply(&add(&q, &q_pe), nt, &add(&keys, &k_pe), nk, &keys);
        q = add(&q, &a);
        self.norm_final.apply(&mut q);
        let mut up = conv_transpose(&keys, side, side, HID, &self.up1, 64);
        add_into(&mut up, &f.hr1);
        self.up_n.apply(&mut up);
        gelu(&mut up);
        let mut up = conv_transpose(&up, side * 2, side * 2, 64, &self.up2, 32);
        add_into(&mut up, &f.hr0);
        gelu(&mut up);
        let n = (side * 4) * (side * 4);
        let masks: Vec<Vec<f32>> = (0..4)
            .map(|i| {
                let hv = self.hyper[i].apply(&q[(2 + i) * HID..(3 + i) * HID], 1, false);
                let mut m = vec![0.0f32; n];
                m.par_iter_mut().enumerate().for_each(|(p, v)| *v = up[p * 32..(p + 1) * 32].iter().zip(&hv).map(|(a, b)| a * b).sum());
                m
            })
            .collect();
        let iou: Vec<f32> = self.iou_head.apply(&q[HID..2 * HID], 1, false).into_iter().map(|v| 1.0 / (1.0 + (-v).exp())).collect();
        let obj = self.obj_head.apply(&q[0..HID], 1, false)[0];
        let pick = if multimask {
            (1..4).max_by(|a, b| iou[*a].total_cmp(&iou[*b])).unwrap_or(1)
        } else {
            let m0 = &masks[0];
            let inter = m0.iter().filter(|v| **v > 0.05).count();
            let uni = m0.iter().filter(|v| **v > -0.05).count();
            let stab = if uni > 0 { inter as f32 / uni as f32 } else { 1.0 };
            if stab >= 0.98 { 0 } else { (1..4).max_by(|a, b| iou[*a].total_cmp(&iou[*b])).unwrap_or(1) }
        };
        let token_idx = if multimask { pick } else { 0 };
        let mut low = masks[pick].clone();
        let appears = obj > 0.0;
        if !appears {
            low.iter_mut().for_each(|v| *v = -1024.0);
        }
        let tok = &q[(2 + token_idx) * HID..(3 + token_idx) * HID];
        let mut ptr = self.obj_ptr_proj.apply(tok, 1, false);
        if !appears {
            ptr = self.no_obj_ptr.clone();
        }
        (low, ptr, obj)
    }

    fn encode_memory(&self, f: &Feats, high: &[f32], s: usize, binarize: bool, appears: bool) -> Vec<f32> {
        let m: Vec<f32> = high.iter().map(|v| if binarize { if *v > 0.0 { 10.0 } else { -10.0 } } else { 1.0 / (1.0 + (-v).exp()) * 20.0 - 10.0 }).collect();
        let (mut x, mut h, mut w) = (m, s, s);
        for (c, n) in &self.me_convs {
            let (y, nh, nw) = c.apply(&x, h, w);
            x = y;
            h = nh;
            w = nw;
            n.apply(&mut x);
            gelu(&mut x);
        }
        let md = self.me_last.apply(&x, h * w);
        let mut y = self.me_pix.apply(&f.vis, f.side * f.side);
        add_into(&mut y, &md);
        for b in &self.me_fuser {
            b.apply(&mut y, f.side, f.side, HID);
        }
        let mut out = self.me_out.apply(&y, f.side * f.side);
        if !appears {
            out.par_chunks_mut(MEM_DIM).for_each(|r| r.iter_mut().zip(&self.no_obj_spatial).for_each(|(a, b)| *a += b));
        }
        round_bf16(&mut out);
        out
    }

    pub fn segment(&self, f: &Feats, points: &[([f32; 2], i32)], s: usize) -> (Vec<f32>, Memory) {
        let pix: Vec<f32> = f.vis.par_chunks(HID).flat_map_iter(|r| r.iter().zip(&self.no_mem_embed).map(|(a, b)| a + b)).collect();
        let (low, ptr, obj) = self.heads(f, &pix, points, None, points.len() <= 1, s);
        let ms = f.side * 4;
        let up = bilinear(&low, ms, ms, s, s);
        let appears = obj > 0.0;
        let feat = self.encode_memory(f, &up, s, true, appears);
        (low, Memory { feat, ptr, cond: true, appears })
    }

    pub fn condition(&self, f: &Feats, mask: &[f32], s: usize) -> (Vec<f32>, Memory) {
        let high: Vec<f32> = mask.iter().map(|v| v * 20.0 - 10.0).collect();
        let ms = s / 4;
        let low = resample_aa(&high, 1, s, s, ms, ms, false);
        let appears = mask.iter().any(|v| *v > 0.0);
        let (md, _, _) = self.mask_downsample.apply(mask, s, s);
        let (_, mut ptr, _) = self.heads(f, &f.vis, &[([0.0, 0.0], -1)], Some(&md), false, s);
        if !appears {
            ptr = self.no_obj_ptr.clone();
        }
        let up = bilinear(&low, ms, ms, s, s);
        let feat = self.encode_memory(f, &up, s, true, appears);
        (low, Memory { feat, ptr, cond: true, appears })
    }

    pub fn track(&self, f: &Feats, frame: i64, mems: &BTreeMap<i64, Memory>, num_frames: i64, reverse: bool, s: usize) -> (Vec<f32>, Memory) {
        let tgt = self.conditioned(f, frame, mems, num_frames, reverse);
        let (low, ptr, obj) = self.heads(f, &tgt, &[([0.0, 0.0], -1)], None, true, s);
        let ms = f.side * 4;
        let up = bilinear(&low, ms, ms, s, s);
        let appears = obj > 0.0;
        let feat = self.encode_memory(f, &up, s, false, appears);
        (low, Memory { feat, ptr, cond: false, appears })
    }

    pub fn conditioned(&self, f: &Feats, frame: i64, mems: &BTreeMap<i64, Memory>, num_frames: i64, reverse: bool) -> Vec<f32> {
        let side = f.side;
        let n = side * side;
        let sign = if reverse { -1 } else { 1 };
        let mut memory: Vec<f32> = vec![];
        let mut mpos: Vec<f32> = vec![];
        let pos64 = sine_pe(side, MEM_DIM);
        let tpos = |idx: usize| &self.maskmem_tpos[idx * MEM_DIM..(idx + 1) * MEM_DIM];
        let push_mem = |m: &Memory, t_pos: usize, memory: &mut Vec<f32>, mpos: &mut Vec<f32>| {
            memory.extend_from_slice(&m.feat);
            let tp = tpos(NUM_MASKMEM - t_pos - 1);
            for i in 0..n {
                for c in 0..MEM_DIM {
                    mpos.push(pos64[i * MEM_DIM + c] + tp[c]);
                }
            }
        };
        let conds: Vec<(i64, &Memory)> = mems.iter().filter(|(_, m)| m.cond).map(|(k, m)| (*k, m)).collect();
        for (_, m) in &conds {
            push_mem(m, 0, &mut memory, &mut mpos);
        }
        for t_pos in 1..NUM_MASKMEM {
            let t_rel = (NUM_MASKMEM - t_pos) as i64;
            let prev = frame - sign * t_rel;
            if let Some(m) = mems.get(&prev).filter(|m| !m.cond) {
                push_mem(m, t_pos, &mut memory, &mut mpos);
            }
        }
        let max_ptrs = (num_frames.max(1) as usize).min(MAX_PTRS);
        let mut ptrs: Vec<(f32, &Memory)> = conds.iter().filter(|(t, _)| if reverse { *t >= frame } else { *t <= frame }).map(|(t, m)| (((frame - t) * sign) as f32, *m)).collect();
        for d in 1..max_ptrs as i64 {
            let t = frame - sign * d;
            if t < 0 || t >= num_frames {
                break;
            }
            if let Some(m) = mems.get(&t).filter(|m| !m.cond) {
                ptrs.push((d as f32, m));
            }
        }
        let mut n_ptr_tokens = 0;
        let tmax = (max_ptrs.max(2) - 1) as f32;
        for (p, m) in &ptrs {
            let pe = self.obj_ptr_tpos.apply(&sine_1d(p / tmax, HID), 1);
            for k in 0..HID / MEM_DIM {
                memory.extend_from_slice(&m.ptr[k * MEM_DIM..(k + 1) * MEM_DIM]);
                mpos.extend_from_slice(&pe);
                n_ptr_tokens += 1;
            }
        }
        let nm = memory.len() / MEM_DIM;
        let cur_pos = sine_pe(side, HID);
        let rope = Rope::new(side, HID);
        let mut tgt: Vec<f32> = f.vis.iter().zip(&cur_pos).map(|(a, b)| a + 0.1 * b).collect();
        let kmem: Vec<f32> = memory.iter().zip(&mpos).map(|(a, b)| a + b).collect();
        for l in &self.ma_layers {
            let mut t2 = tgt.clone();
            l.n1.apply(&mut t2);
            let a = l.self_attn.apply_rope(&t2, n, &t2, n, &t2, &rope, 0);
            add_into(&mut tgt, &a);
            let mut t2 = tgt.clone();
            l.n2.apply(&mut t2);
            let a = l.cross.apply_rope(&t2, n, &kmem, nm, &memory, &rope, n_ptr_tokens);
            add_into(&mut tgt, &a);
            let mut t2 = tgt.clone();
            l.n3.apply(&mut t2);
            let mut h = l.l1.apply(&t2, n);
            relu(&mut h);
            let a = l.l2.apply(&h, n);
            add_into(&mut tgt, &a);
        }
        self.ma_norm.apply(&mut tgt);
        tgt
    }
}

pub fn prepare(px: &[f32], w: usize, h: usize, s: usize) -> Vec<f32> {
    let mut rgb = vec![0.0f32; w * h * 3];
    rgb.par_chunks_mut(3).enumerate().for_each(|(i, o)| {
        let a = px[i * 4 + 3];
        for c in 0..3 {
            let v = if a > 1e-6 { px[i * 4 + c] / a } else { 0.0 };
            o[c] = (v.clamp(0.0, 1.0) * 255.0).round() / 255.0;
        }
    });
    let mut out = resample_aa(&rgb, 3, h, w, s, s, true);
    out.par_chunks_mut(3).for_each(|o| {
        for c in 0..3 {
            o[c] = ((o[c] * 255.0).round().clamp(0.0, 255.0) / 255.0 - MEAN[c]) / STD[c];
        }
    });
    out
}
