use rayon::prelude::*;

const ADD: i32 = 0;
const CONCATENATION: i32 = 2;
const CONV_2D: i32 = 3;
const DEPTHWISE_CONV_2D: i32 = 4;
const DEQUANTIZE: i32 = 6;
const LOGISTIC: i32 = 14;
const MAX_POOL_2D: i32 = 17;
const RELU: i32 = 19;
const RESHAPE: i32 = 22;
const PAD: i32 = 34;
const PRELU: i32 = 54;

#[derive(Clone, Copy)]
struct Table<'a> {
    b: &'a [u8],
    pos: usize,
}

fn rd<const N: usize>(b: &[u8], at: usize) -> Option<[u8; N]> {
    b.get(at..at.checked_add(N)?)?.try_into().ok()
}

fn u16_at(b: &[u8], at: usize) -> Option<usize> {
    rd::<2>(b, at).map(|x| u16::from_le_bytes(x) as usize)
}

fn u32_at(b: &[u8], at: usize) -> Option<usize> {
    rd::<4>(b, at).map(|x| u32::from_le_bytes(x) as usize)
}

fn i32_at(b: &[u8], at: usize) -> Option<i32> {
    rd::<4>(b, at).map(i32::from_le_bytes)
}

impl<'a> Table<'a> {
    fn root(b: &'a [u8]) -> Option<Table<'a>> {
        Some(Table { b, pos: u32_at(b, 0)? })
    }

    fn field(&self, i: usize) -> Option<usize> {
        let vt = (self.pos as i64 - i32_at(self.b, self.pos)? as i64) as usize;
        let len = u16_at(self.b, vt)?;
        let off = 4 + 2 * i;
        if off + 2 > len {
            return None;
        }
        let f = u16_at(self.b, vt + off)?;
        (f != 0).then_some(self.pos + f)
    }

    fn u8(&self, i: usize, d: u8) -> u8 {
        self.field(i).and_then(|p| self.b.get(p).copied()).unwrap_or(d)
    }

    fn i32(&self, i: usize, d: i32) -> i32 {
        self.field(i).and_then(|p| i32_at(self.b, p)).unwrap_or(d)
    }

    fn table(&self, i: usize) -> Option<Table<'a>> {
        let p = self.field(i)?;
        Some(Table { b: self.b, pos: p + u32_at(self.b, p)? })
    }

    fn vector(&self, i: usize) -> Option<(usize, usize)> {
        let p = self.field(i)?;
        let v = p + u32_at(self.b, p)?;
        Some((v + 4, u32_at(self.b, v)?))
    }

    fn tables(&self, i: usize) -> Vec<Table<'a>> {
        let Some((s, n)) = self.vector(i) else { return vec![] };
        (0..n).filter_map(|k| Some(Table { b: self.b, pos: s + 4 * k + u32_at(self.b, s + 4 * k)? })).collect()
    }

    fn ints(&self, i: usize) -> Vec<i32> {
        let Some((s, n)) = self.vector(i) else { return vec![] };
        (0..n).filter_map(|k| i32_at(self.b, s + 4 * k)).collect()
    }

    fn bytes(&self, i: usize) -> &'a [u8] {
        let Some((s, n)) = self.vector(i) else { return &[] };
        self.b.get(s..s + n).unwrap_or(&[])
    }

    fn string(&self, i: usize) -> String {
        String::from_utf8_lossy(self.bytes(i)).to_string()
    }
}

fn f16(h: u16) -> f32 {
    let s = ((h >> 15) as u32) << 31;
    let e = ((h >> 10) & 0x1f) as u32;
    let m = (h & 0x3ff) as u32;
    let bits = match e {
        0 if m == 0 => s,
        0 => {
            let mut e = 127 - 15 + 1;
            let mut m = m;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            s | (e << 23) | ((m & 0x3ff) << 13)
        }
        31 => s | 0x7f80_0000 | (m << 13),
        _ => s | ((e + 127 - 15) << 23) | (m << 13),
    };
    f32::from_bits(bits)
}

#[derive(Clone, Debug, Default)]
pub struct Tensor {
    pub shape: Vec<usize>,
    pub f: Vec<f32>,
    pub i: Vec<i32>,
}

impl Tensor {
    fn len(&self) -> usize {
        self.shape.iter().product::<usize>().max(1)
    }
}

#[derive(Clone, Debug)]
struct Op {
    code: i32,
    inputs: Vec<i32>,
    outputs: Vec<i32>,
    padding: u8,
    stride: [usize; 2],
    dilation: [usize; 2],
    filter: [usize; 2],
    multiplier: usize,
    act: u8,
    axis: i32,
    new_shape: Vec<i32>,
}

pub struct Model {
    shapes: Vec<Vec<usize>>,
    names: Vec<String>,
    consts: Vec<Option<Tensor>>,
    ops: Vec<Op>,
    pub inputs: Vec<usize>,
    pub outputs: Vec<usize>,
}

pub fn load(b: &[u8]) -> Result<Model, String> {
    let bad = || "not a readable TensorFlow Lite model".to_string();
    if b.len() < 8 || &b[4..8] != b"TFL3" {
        return Err(bad());
    }
    let m = Table::root(b).ok_or_else(bad)?;
    let codes: Vec<i32> = m.tables(1).iter().map(|c| c.i32(3, 0).max(c.u8(0, 0) as i32)).collect();
    let buffers: Vec<&[u8]> = m.tables(4).iter().map(|t| t.bytes(0)).collect();
    let g = m.tables(2).into_iter().next().ok_or_else(bad)?;
    let mut shapes = vec![];
    let mut names = vec![];
    let mut consts = vec![];
    for t in g.tables(0) {
        let shape: Vec<usize> = t.ints(0).into_iter().map(|x| x.max(0) as usize).collect();
        let ty = t.u8(1, 0);
        let data = buffers.get(t.i32(2, 0) as usize).copied().unwrap_or(&[]);
        let c = (!data.is_empty()).then(|| {
            let mut out = Tensor { shape: shape.clone(), ..Default::default() };
            match ty {
                0 => out.f = data.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect(),
                1 => out.f = data.chunks_exact(2).map(|c| f16(u16::from_le_bytes([c[0], c[1]]))).collect(),
                2 => out.i = data.chunks_exact(4).map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect(),
                _ => {}
            }
            out
        });
        shapes.push(shape);
        names.push(t.string(3));
        consts.push(c);
    }
    let mut ops = vec![];
    for o in g.tables(3) {
        let code = *codes.get(o.i32(0, 0) as usize).ok_or_else(bad)?;
        let opt = o.table(4);
        let oi = |i: usize, d: i32| opt.map(|t| t.i32(i, d)).unwrap_or(d);
        let ob = |i: usize, d: u8| opt.map(|t| t.u8(i, d)).unwrap_or(d);
        let mut op = Op { code, inputs: o.ints(1), outputs: o.ints(2), padding: 0, stride: [1, 1], dilation: [1, 1], filter: [1, 1], multiplier: 1, act: 0, axis: 0, new_shape: vec![] };
        match code {
            CONV_2D => {
                op.padding = ob(0, 0);
                op.stride = [oi(2, 1) as usize, oi(1, 1) as usize];
                op.act = ob(3, 0);
                op.dilation = [oi(5, 1) as usize, oi(4, 1) as usize];
            }
            DEPTHWISE_CONV_2D => {
                op.padding = ob(0, 0);
                op.stride = [oi(2, 1) as usize, oi(1, 1) as usize];
                op.multiplier = oi(3, 1).max(1) as usize;
                op.act = ob(4, 0);
                op.dilation = [oi(6, 1) as usize, oi(5, 1) as usize];
            }
            MAX_POOL_2D => {
                op.padding = ob(0, 0);
                op.stride = [oi(2, 1) as usize, oi(1, 1) as usize];
                op.filter = [oi(4, 1) as usize, oi(3, 1) as usize];
                op.act = ob(5, 0);
            }
            ADD => op.act = ob(0, 0),
            CONCATENATION => {
                op.axis = oi(0, 0);
                op.act = ob(1, 0);
            }
            RESHAPE => op.new_shape = opt.map(|t| t.ints(0)).unwrap_or_default(),
            DEQUANTIZE | RELU | PAD | PRELU | LOGISTIC => {}
            other => return Err(format!("TensorFlow Lite operator {other} is not supported")),
        }
        ops.push(op);
    }
    let idx = |v: Vec<i32>| v.into_iter().map(|x| x.max(0) as usize).collect::<Vec<_>>();
    Ok(Model { shapes, names, consts, ops, inputs: idx(g.ints(1)), outputs: idx(g.ints(2)) })
}

fn activate(v: &mut [f32], act: u8) {
    match act {
        1 => v.iter_mut().for_each(|x| *x = x.max(0.0)),
        2 => v.iter_mut().for_each(|x| *x = x.clamp(-1.0, 1.0)),
        3 => v.iter_mut().for_each(|x| *x = x.clamp(0.0, 6.0)),
        _ => {}
    }
}

fn hw(shape: &[usize]) -> Result<(usize, usize, usize), String> {
    match shape {
        [_, h, w, c] => Ok((*h, *w, *c)),
        _ => Err(format!("expected an image tensor, got shape {shape:?}")),
    }
}

fn out_size(n: usize, k: usize, s: usize, d: usize, same: bool) -> (usize, usize) {
    let ke = (k - 1) * d + 1;
    if same {
        let o = n.div_ceil(s);
        let total = ((o - 1) * s + ke).saturating_sub(n);
        (o, total / 2)
    } else {
        ((n.saturating_sub(ke)) / s + 1, 0)
    }
}

fn conv(x: &Tensor, w: &Tensor, b: Option<&Tensor>, op: &Op, depthwise: bool) -> Result<Tensor, String> {
    let (ih, iw, ic) = hw(&x.shape)?;
    let (kh, kw) = (w.shape[1], w.shape[2]);
    let oc = if depthwise { w.shape[3] } else { w.shape[0] };
    let same = op.padding == 0;
    let (oh, pt) = out_size(ih, kh, op.stride[0], op.dilation[0], same);
    let (ow, pl) = out_size(iw, kw, op.stride[1], op.dilation[1], same);
    let mut out = vec![0f32; oh * ow * oc];
    let wf = &w.f;
    let bias = b.map(|t| t.f.as_slice());
    let m = op.multiplier.max(1);
    out.par_chunks_mut(ow * oc).enumerate().for_each(|(oy, row)| {
        for ox in 0..ow {
            let cell = &mut row[ox * oc..ox * oc + oc];
            if let Some(bv) = bias {
                cell.copy_from_slice(&bv[..oc]);
            }
            for ky in 0..kh {
                let iy = (oy * op.stride[0] + ky * op.dilation[0]) as isize - pt as isize;
                if iy < 0 || iy >= ih as isize {
                    continue;
                }
                for kx in 0..kw {
                    let ix = (ox * op.stride[1] + kx * op.dilation[1]) as isize - pl as isize;
                    if ix < 0 || ix >= iw as isize {
                        continue;
                    }
                    let px = &x.f[(iy as usize * iw + ix as usize) * ic..][..ic];
                    if depthwise {
                        let wk = &wf[(ky * kw + kx) * oc..][..oc];
                        for (o, c) in cell.iter_mut().enumerate() {
                            *c += px[o / m] * wk[o];
                        }
                    } else {
                        for (o, c) in cell.iter_mut().enumerate() {
                            let wk = &wf[((o * kh + ky) * kw + kx) * ic..][..ic];
                            let mut acc = 0f32;
                            for i in 0..ic {
                                acc += px[i] * wk[i];
                            }
                            *c += acc;
                        }
                    }
                }
            }
        }
        activate(row, op.act);
    });
    Ok(Tensor { shape: vec![1, oh, ow, oc], f: out, i: vec![] })
}

fn max_pool(x: &Tensor, op: &Op) -> Result<Tensor, String> {
    let (ih, iw, c) = hw(&x.shape)?;
    let same = op.padding == 0;
    let (oh, pt) = out_size(ih, op.filter[0], op.stride[0], 1, same);
    let (ow, pl) = out_size(iw, op.filter[1], op.stride[1], 1, same);
    let mut out = vec![f32::NEG_INFINITY; oh * ow * c];
    for oy in 0..oh {
        for ox in 0..ow {
            let cell = &mut out[(oy * ow + ox) * c..][..c];
            for ky in 0..op.filter[0] {
                let iy = (oy * op.stride[0] + ky) as isize - pt as isize;
                if iy < 0 || iy >= ih as isize {
                    continue;
                }
                for kx in 0..op.filter[1] {
                    let ix = (ox * op.stride[1] + kx) as isize - pl as isize;
                    if ix < 0 || ix >= iw as isize {
                        continue;
                    }
                    let px = &x.f[(iy as usize * iw + ix as usize) * c..][..c];
                    for (o, v) in cell.iter_mut().zip(px) {
                        *o = o.max(*v);
                    }
                }
            }
        }
    }
    activate(&mut out, op.act);
    Ok(Tensor { shape: vec![1, oh, ow, c], f: out, i: vec![] })
}

fn broadcast(a: &Tensor, b: &Tensor, f: impl Fn(f32, f32) -> f32) -> Result<Tensor, String> {
    let (big, small, swap) = if a.f.len() >= b.f.len() { (a, b, false) } else { (b, a, true) };
    let n = small.f.len().max(1);
    if big.f.len() % n != 0 {
        return Err(format!("cannot broadcast {:?} with {:?}", a.shape, b.shape));
    }
    let out = big.f.iter().enumerate().map(|(i, v)| {
        let s = small.f[i % n];
        if swap { f(s, *v) } else { f(*v, s) }
    });
    Ok(Tensor { shape: big.shape.clone(), f: out.collect(), i: vec![] })
}

impl Model {
    pub fn input_shape(&self, k: usize) -> Vec<usize> {
        self.inputs.get(k).map(|i| self.shapes[*i].clone()).unwrap_or_default()
    }

    pub fn output_names(&self) -> Vec<String> {
        self.outputs.iter().map(|i| self.names[*i].clone()).collect()
    }

    pub fn run(&self, inputs: Vec<Vec<f32>>) -> Result<Vec<Tensor>, String> {
        let mut live: Vec<Option<Tensor>> = self.consts.clone();
        for (k, data) in inputs.into_iter().enumerate() {
            let at = *self.inputs.get(k).ok_or("too many inputs")?;
            let t = Tensor { shape: self.shapes[at].clone(), f: data, i: vec![] };
            if t.f.len() != t.len() {
                return Err(format!("input {k} needs {} values, got {}", t.len(), t.f.len()));
            }
            live[at] = Some(t);
        }
        for op in &self.ops {
            let get = |k: usize| -> Result<&Tensor, String> {
                let i = *op.inputs.get(k).ok_or("missing operator input")?;
                if i < 0 {
                    return Err("optional operator input".into());
                }
                live.get(i as usize).and_then(Option::as_ref).ok_or_else(|| format!("tensor {i} has no value yet"))
            };
            let out = match op.code {
                DEQUANTIZE => get(0)?.clone(),
                CONV_2D => conv(get(0)?, get(1)?, op.inputs.get(2).filter(|i| **i >= 0).and(get(2).ok()), op, false)?,
                DEPTHWISE_CONV_2D => conv(get(0)?, get(1)?, op.inputs.get(2).filter(|i| **i >= 0).and(get(2).ok()), op, true)?,
                RELU => {
                    let mut t = get(0)?.clone();
                    activate(&mut t.f, 1);
                    t
                }
                PRELU => broadcast(get(0)?, get(1)?, |x, a| if x >= 0.0 { x } else { x * a })?,
                LOGISTIC => {
                    let mut t = get(0)?.clone();
                    t.f.iter_mut().for_each(|v| *v = 1.0 / (1.0 + (-*v).exp()));
                    t
                }
                ADD => {
                    let mut t = broadcast(get(0)?, get(1)?, |x, y| x + y)?;
                    activate(&mut t.f, op.act);
                    t
                }
                MAX_POOL_2D => max_pool(get(0)?, op)?,
                PAD => {
                    let x = get(0)?;
                    let p = &get(1)?.i;
                    let (ih, iw, c) = hw(&x.shape)?;
                    let g = |k: usize| p.get(k).copied().unwrap_or(0).max(0) as usize;
                    let (t, bt, l, r, c0, c1) = (g(2), g(3), g(4), g(5), g(6), g(7));
                    let (oh, ow, oc) = (ih + t + bt, iw + l + r, c + c0 + c1);
                    let mut out = vec![0f32; oh * ow * oc];
                    for y in 0..ih {
                        for xx in 0..iw {
                            let src = &x.f[(y * iw + xx) * c..][..c];
                            let d = ((y + t) * ow + xx + l) * oc + c0;
                            out[d..d + c].copy_from_slice(src);
                        }
                    }
                    Tensor { shape: vec![1, oh, ow, oc], f: out, i: vec![] }
                }
                RESHAPE => {
                    let x = get(0)?;
                    let want = if op.new_shape.is_empty() { get(1).map(|t| t.i.clone()).unwrap_or_default() } else { op.new_shape.clone() };
                    let target = self.shapes[op.outputs[0] as usize].clone();
                    let shape = if target.iter().product::<usize>() == x.f.len() {
                        target
                    } else {
                        let known: usize = want.iter().filter(|v| **v > 0).map(|v| *v as usize).product::<usize>().max(1);
                        want.iter().map(|v| if *v < 0 { x.f.len() / known } else { *v as usize }).collect()
                    };
                    Tensor { shape, f: x.f.clone(), i: vec![] }
                }
                CONCATENATION => {
                    let parts: Vec<&Tensor> = (0..op.inputs.len()).map(&get).collect::<Result<_, _>>()?;
                    let rank = parts[0].shape.len();
                    let axis = if op.axis < 0 { (op.axis + rank as i32) as usize } else { op.axis as usize };
                    let outer: usize = parts[0].shape[..axis].iter().product::<usize>().max(1);
                    let mut shape = parts[0].shape.clone();
                    shape[axis] = parts.iter().map(|p| p.shape[axis]).sum();
                    let mut out = Vec::with_capacity(shape.iter().product());
                    for o in 0..outer {
                        for p in &parts {
                            let inner: usize = p.shape[axis..].iter().product::<usize>().max(1);
                            out.extend_from_slice(&p.f[o * inner..(o + 1) * inner]);
                        }
                    }
                    let mut t = Tensor { shape, f: out, i: vec![] };
                    activate(&mut t.f, op.act);
                    t
                }
                other => return Err(format!("operator {other} is not supported")),
            };
            live[op.outputs[0] as usize] = Some(out);
        }
        self.outputs.iter().map(|o| live[*o].clone().ok_or_else(|| "an output was not computed".to_string())).collect()
    }
}
