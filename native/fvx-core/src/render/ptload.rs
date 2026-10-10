use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Tensor {
    pub shape: Vec<usize>,
    pub data: Vec<f32>,
}

impl Tensor {
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }
}

struct Entry {
    offset: usize,
    size: usize,
    method: u16,
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(i)?, *b.get(i + 1)?, *b.get(i + 2)?, *b.get(i + 3)?]))
}

fn u64_at(b: &[u8], i: usize) -> Option<u64> {
    let mut a = [0u8; 8];
    a.copy_from_slice(b.get(i..i + 8)?);
    Some(u64::from_le_bytes(a))
}

fn zip_entries(b: &[u8]) -> Result<HashMap<String, Entry>, String> {
    let bad = || "the model file is not a PyTorch checkpoint (zip)".to_string();
    let lo = b.len().saturating_sub(65_557);
    let eocd = (lo..b.len().saturating_sub(21)).rev().find(|&i| u32_at(b, i) == Some(0x0605_4b50)).ok_or_else(bad)?;
    let mut count = u16_at(b, eocd + 10).ok_or_else(bad)? as usize;
    let mut cd = u32_at(b, eocd + 16).ok_or_else(bad)? as usize;
    if (count == 0xffff || cd == 0xffff_ffff) && eocd >= 20 && u32_at(b, eocd - 20) == Some(0x0706_4b50) {
        let z64 = u64_at(b, eocd - 12).ok_or_else(bad)? as usize;
        if u32_at(b, z64) == Some(0x0606_4b50) {
            count = u64_at(b, z64 + 32).ok_or_else(bad)? as usize;
            cd = u64_at(b, z64 + 48).ok_or_else(bad)? as usize;
        }
    }
    let mut out = HashMap::new();
    let mut i = cd;
    for _ in 0..count {
        if u32_at(b, i) != Some(0x0201_4b50) {
            return Err(bad());
        }
        let method = u16_at(b, i + 10).ok_or_else(bad)?;
        let mut csize = u32_at(b, i + 20).ok_or_else(bad)? as u64;
        let full = u32_at(b, i + 24).ok_or_else(bad)?;
        let nlen = u16_at(b, i + 28).ok_or_else(bad)? as usize;
        let xlen = u16_at(b, i + 30).ok_or_else(bad)? as usize;
        let clen = u16_at(b, i + 32).ok_or_else(bad)? as usize;
        let mut local = u32_at(b, i + 42).ok_or_else(bad)? as u64;
        let name = String::from_utf8_lossy(b.get(i + 46..i + 46 + nlen).ok_or_else(bad)?).to_string();
        let mut x = i + 46 + nlen;
        let xend = x + xlen;
        while x + 4 <= xend {
            let id = u16_at(b, x).ok_or_else(bad)?;
            let sz = u16_at(b, x + 2).ok_or_else(bad)? as usize;
            if id == 1 {
                let mut p = x + 4;
                if full == 0xffff_ffff {
                    p += 8;
                }
                if csize == 0xffff_ffff {
                    csize = u64_at(b, p).ok_or_else(bad)?;
                    p += 8;
                }
                if local == 0xffff_ffff {
                    local = u64_at(b, p).ok_or_else(bad)?;
                }
            }
            x += 4 + sz;
        }
        let l = local as usize;
        let lname = u16_at(b, l + 26).ok_or_else(bad)? as usize;
        let lextra = u16_at(b, l + 28).ok_or_else(bad)? as usize;
        out.insert(name, Entry { offset: l + 30 + lname + lextra, size: csize as usize, method });
        i += 46 + nlen + xlen + clen;
    }
    Ok(out)
}

#[derive(Clone, Debug)]
enum V {
    None,
    Bool,
    Int(i64),
    Float,
    Str(String),
    Tuple(Vec<V>),
    List(Vec<V>),
    Dict(Vec<(V, V)>),
    Global(String),
    Pers(Box<V>),
    Tensor { key: String, offset: usize, size: Vec<usize>, stride: Vec<usize> },
    Mark,
}

fn as_usize(v: &V) -> Option<usize> {
    match v {
        V::Int(i) if *i >= 0 => Some(*i as usize),
        _ => None,
    }
}

fn tuple_usizes(v: &V) -> Option<Vec<usize>> {
    match v {
        V::Tuple(a) | V::List(a) => a.iter().map(as_usize).collect(),
        _ => None,
    }
}

fn call(f: &V, args: Vec<V>) -> Result<V, String> {
    match f {
        V::Global(n) if n == "_rebuild_tensor_v2" || n == "_rebuild_tensor" => {
            let key = match args.first() {
                Some(V::Pers(p)) => match p.as_ref() {
                    V::Tuple(t) => match t.get(2) {
                        Some(V::Str(s)) => s.clone(),
                        _ => return Err("unexpected storage reference".into()),
                    },
                    _ => return Err("unexpected storage reference".into()),
                },
                _ => return Err("unexpected tensor storage".into()),
            };
            let offset = args.get(1).and_then(as_usize).ok_or("bad tensor offset")?;
            let size = args.get(2).and_then(tuple_usizes).ok_or("bad tensor size")?;
            let stride = args.get(3).and_then(tuple_usizes).ok_or("bad tensor stride")?;
            Ok(V::Tensor { key, offset, size, stride })
        }
        V::Global(n) if n == "_rebuild_parameter" => args.into_iter().next().ok_or_else(|| "bad parameter".into()),
        V::Global(n) if n == "OrderedDict" => Ok(V::Dict(vec![])),
        V::Global(n) => Ok(V::Global(n.clone())),
        _ => Ok(V::None),
    }
}

fn unpickle(b: &[u8]) -> Result<V, String> {
    let mut st: Vec<V> = vec![];
    let mut memo: HashMap<usize, V> = HashMap::new();
    let mut i = 0usize;
    let rd = |i: &mut usize, n: usize| -> Result<&[u8], String> {
        let s = b.get(*i..*i + n).ok_or("the checkpoint index is truncated")?;
        *i += n;
        Ok(s)
    };
    let pop_mark = |st: &mut Vec<V>| -> Vec<V> {
        let at = st.iter().rposition(|v| matches!(v, V::Mark)).unwrap_or(0);
        let items: Vec<V> = st.drain(at..).skip(1).collect();
        items
    };
    loop {
        let op = *b.get(i).ok_or("the checkpoint index ended early")?;
        i += 1;
        match op {
            0x80 => {
                rd(&mut i, 1)?;
            }
            0x95 => {
                rd(&mut i, 8)?;
            }
            b'}' => st.push(V::Dict(vec![])),
            b']' => st.push(V::List(vec![])),
            b')' => st.push(V::Tuple(vec![])),
            b'N' => st.push(V::None),
            0x88 | 0x89 => st.push(V::Bool),
            b'(' => st.push(V::Mark),
            b'q' => {
                let k = rd(&mut i, 1)?[0] as usize;
                memo.insert(k, st.last().cloned().unwrap_or(V::None));
            }
            b'r' => {
                let k = u32::from_le_bytes(rd(&mut i, 4)?.try_into().map_err(|_| "bad memo")?) as usize;
                memo.insert(k, st.last().cloned().unwrap_or(V::None));
            }
            0x94 => {
                let k = memo.len();
                memo.insert(k, st.last().cloned().unwrap_or(V::None));
            }
            b'h' => {
                let k = rd(&mut i, 1)?[0] as usize;
                st.push(memo.get(&k).cloned().ok_or("bad memo reference")?);
            }
            b'j' => {
                let k = u32::from_le_bytes(rd(&mut i, 4)?.try_into().map_err(|_| "bad memo")?) as usize;
                st.push(memo.get(&k).cloned().ok_or("bad memo reference")?);
            }
            b'X' => {
                let n = u32::from_le_bytes(rd(&mut i, 4)?.try_into().map_err(|_| "bad string")?) as usize;
                st.push(V::Str(String::from_utf8_lossy(rd(&mut i, n)?).to_string()));
            }
            0x8c => {
                let n = rd(&mut i, 1)?[0] as usize;
                st.push(V::Str(String::from_utf8_lossy(rd(&mut i, n)?).to_string()));
            }
            b'c' => {
                let line = |i: &mut usize| -> Result<String, String> {
                    let s = *i;
                    while *b.get(*i).ok_or("bad global")? != b'\n' {
                        *i += 1;
                    }
                    let out = String::from_utf8_lossy(&b[s..*i]).to_string();
                    *i += 1;
                    Ok(out)
                };
                line(&mut i)?;
                let n = line(&mut i)?;
                st.push(V::Global(n));
            }
            0x93 => {
                let n = st.pop().ok_or("bad stack global")?;
                let m = st.pop().ok_or("bad stack global")?;
                match (m, n) {
                    (V::Str(_), V::Str(n)) => st.push(V::Global(n)),
                    _ => return Err("bad stack global".into()),
                }
            }
            b'Q' => {
                let pid = st.pop().ok_or("bad persistent id")?;
                st.push(V::Pers(Box::new(pid)));
            }
            b'K' => st.push(V::Int(rd(&mut i, 1)?[0] as i64)),
            b'M' => st.push(V::Int(u16::from_le_bytes(rd(&mut i, 2)?.try_into().map_err(|_| "bad int")?) as i64)),
            b'J' => st.push(V::Int(i32::from_le_bytes(rd(&mut i, 4)?.try_into().map_err(|_| "bad int")?) as i64)),
            0x8a => {
                let n = rd(&mut i, 1)?[0] as usize;
                let bytes = rd(&mut i, n)?;
                let mut v: i64 = 0;
                for (k, byte) in bytes.iter().enumerate().take(8) {
                    v |= (*byte as i64) << (8 * k);
                }
                if n > 0 && n < 8 && bytes[n - 1] & 0x80 != 0 {
                    v -= 1i64 << (8 * n);
                }
                st.push(V::Int(v));
            }
            b'G' => {
                rd(&mut i, 8)?;
                st.push(V::Float);
            }
            b't' => {
                let items = pop_mark(&mut st);
                st.push(V::Tuple(items));
            }
            0x85 | 0x86 | 0x87 => {
                let n = (op - 0x84) as usize;
                if st.len() < n {
                    return Err("bad tuple".into());
                }
                let items = st.split_off(st.len() - n);
                st.push(V::Tuple(items));
            }
            b'R' => {
                let args = st.pop().ok_or("bad reduce")?;
                let f = st.pop().ok_or("bad reduce")?;
                let args = match args {
                    V::Tuple(a) => a,
                    other => vec![other],
                };
                st.push(call(&f, args)?);
            }
            0x81 => {
                let args = st.pop().ok_or("bad newobj")?;
                let cls = st.pop().ok_or("bad newobj")?;
                let args = match args {
                    V::Tuple(a) => a,
                    other => vec![other],
                };
                st.push(call(&cls, args)?);
            }
            b'b' => {
                st.pop();
            }
            b's' => {
                let v = st.pop().ok_or("bad setitem")?;
                let k = st.pop().ok_or("bad setitem")?;
                if let Some(V::Dict(d)) = st.last_mut() {
                    d.push((k, v));
                }
            }
            b'u' => {
                let items = pop_mark(&mut st);
                if let Some(V::Dict(d)) = st.last_mut() {
                    let mut it = items.into_iter();
                    while let (Some(k), Some(v)) = (it.next(), it.next()) {
                        d.push((k, v));
                    }
                }
            }
            b'a' => {
                let v = st.pop().ok_or("bad append")?;
                if let Some(V::List(l)) = st.last_mut() {
                    l.push(v);
                }
            }
            b'e' => {
                let items = pop_mark(&mut st);
                if let Some(V::List(l)) = st.last_mut() {
                    l.extend(items);
                }
            }
            b'.' => return st.pop().ok_or_else(|| "empty checkpoint index".into()),
            other => return Err(format!("unsupported pickle opcode 0x{other:02x} in the checkpoint")),
        }
    }
}

pub fn read(bytes: &[u8]) -> Result<HashMap<String, Tensor>, String> {
    let entries = zip_entries(bytes)?;
    let pkl = entries.iter().find(|(k, _)| k.ends_with("/data.pkl") || k.as_str() == "data.pkl").ok_or("the checkpoint has no data.pkl")?;
    let prefix = pkl.0.strip_suffix("data.pkl").unwrap_or("").to_string();
    if pkl.1.method != 0 {
        return Err("compressed checkpoints are not supported".into());
    }
    let index = bytes.get(pkl.1.offset..pkl.1.offset + pkl.1.size).ok_or("the checkpoint index is truncated")?;
    let root = unpickle(index)?;
    let V::Dict(mut items) = root else { return Err("the checkpoint is not a state dict".into()) };
    if let Some(pos) = items.iter().position(|(k, v)| matches!((k, v), (V::Str(n), V::Dict(_)) if n == "model")) {
        if let (_, V::Dict(inner)) = items.swap_remove(pos) {
            items = inner;
        }
    }
    let mut out = HashMap::new();
    for (k, v) in items {
        let (V::Str(name), V::Tensor { key, offset, size, stride }) = (k, v) else { continue };
        if name.ends_with("num_batches_tracked") {
            continue;
        }
        let Some(e) = entries.get(&format!("{prefix}data/{key}")) else { continue };
        if e.method != 0 {
            return Err("compressed checkpoints are not supported".into());
        }
        let raw = bytes.get(e.offset..e.offset + e.size).ok_or("a checkpoint tensor is truncated")?;
        if raw.len() % 4 != 0 {
            continue;
        }
        let numel: usize = size.iter().product();
        let floats: Vec<f32> = raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let mut data = vec![0.0f32; numel];
        let dims = size.len();
        let mut idx = vec![0usize; dims];
        for (n, d) in data.iter_mut().enumerate() {
            let mut rem = n;
            for a in (0..dims).rev() {
                idx[a] = rem % size[a].max(1);
                rem /= size[a].max(1);
            }
            let src = offset + idx.iter().zip(&stride).map(|(i, s)| i * s).sum::<usize>();
            *d = floats.get(src).copied().unwrap_or(0.0);
        }
        out.insert(name, Tensor { shape: size, data });
    }
    Ok(out)
}
