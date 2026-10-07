use std::cell::RefCell;
use std::rc::Rc;

use super::host;
use super::interp::*;

type M = (&'static str, NativeImpl);

fn not_callable(_: &mut Interp, _: &V, _: Vec<V>) -> R {
    Err(type_err("this object is not a function"))
}

pub fn global(name: &str) -> Option<V> {
    let f: NativeImpl = match name {
        "Math" | "JSON" => not_callable,
        "Number" => g_number,
        "String" => g_string,
        "Boolean" => g_boolean,
        "Array" => g_array,
        "Object" => g_object,
        "RegExp" => g_regexp,
        "Error" | "TypeError" | "RangeError" | "SyntaxError" | "ReferenceError" => g_error,
        "parseInt" => g_parse_int,
        "parseFloat" => g_parse_float,
        "isNaN" => g_is_nan,
        "isFinite" => g_is_finite,
        "encodeURIComponent" | "encodeURI" | "escape" => g_identity_string,
        "decodeURIComponent" | "decodeURI" | "unescape" => g_identity_string,
        "linear" => ae_linear,
        "ease" => ae_ease,
        "easeIn" => ae_ease_in,
        "easeOut" => ae_ease_out,
        "clamp" => ae_clamp,
        "add" => ae_add,
        "sub" => ae_sub,
        "mul" => ae_mul,
        "div" => ae_div,
        "dot" => ae_dot,
        "cross" => ae_cross,
        "normalize" => ae_normalize,
        "length" => ae_length,
        "lookAt" => ae_look_at,
        "degreesToRadians" => ae_d2r,
        "radiansToDegrees" => ae_r2d,
        "rgbToHsl" => ae_rgb_to_hsl,
        "hslToRgb" => ae_hsl_to_rgb,
        "hexToRgb" => ae_hex_to_rgb,
        "random" => ae_random,
        "gaussRandom" => ae_gauss_random,
        "seedRandom" => ae_seed_random,
        "noise" => ae_noise,
        "timeToFrames" => ae_time_to_frames,
        "framesToTime" => ae_frames_to_time,
        "timeToTimecode" | "timeToCurrentFormat" | "timeToNTSCTimecode" => ae_time_to_timecode,
        "timeToFeetAndFrames" => ae_time_to_feet,
        "posterizeTime" => ae_posterize,
        "comp" => host::g_comp,
        "footage" => host::g_footage,
        "createPath" => host::g_create_path,
        "NaN" => return Some(V::Num(f64::NAN)),
        "Infinity" => return Some(V::Num(f64::INFINITY)),
        "globalThis" => return Some(obj(vec![])),
        _ => return None,
    };
    let name: &'static str = match name {
        "Math" => "Math",
        "JSON" => "JSON",
        "Number" => "Number",
        "String" => "String",
        "Boolean" => "Boolean",
        "Array" => "Array",
        "Object" => "Object",
        "RegExp" => "RegExp",
        "Error" => "Error",
        "TypeError" => "TypeError",
        "RangeError" => "RangeError",
        "SyntaxError" => "SyntaxError",
        "ReferenceError" => "ReferenceError",
        _ => "function",
    };
    Some(native(name, V::Undef, f))
}

pub fn ns_get(ns: &str, key: &str) -> Option<V> {
    let c = |x: f64| Some(V::Num(x));
    match ns {
        "Math" => match key {
            "PI" => c(std::f64::consts::PI),
            "E" => c(std::f64::consts::E),
            "LN2" => c(std::f64::consts::LN_2),
            "LN10" => c(std::f64::consts::LN_10),
            "LOG2E" => c(std::f64::consts::LOG2_E),
            "LOG10E" => c(std::f64::consts::LOG10_E),
            "SQRT2" => c(std::f64::consts::SQRT_2),
            "SQRT1_2" => c(std::f64::consts::FRAC_1_SQRT_2),
            _ => {
                let f: NativeImpl = match key {
                    "abs" | "acos" | "acosh" | "asin" | "asinh" | "atan" | "atanh" | "cbrt" | "ceil" | "cos" | "cosh" | "exp" | "expm1" | "floor" | "fround" | "log" | "log1p" | "log10" | "log2" | "round" | "sign" | "sin" | "sinh" | "sqrt" | "tan" | "tanh" | "trunc" => math1,
                    "atan2" => math_atan2,
                    "pow" => math_pow,
                    "max" => math_max,
                    "min" => math_min,
                    "hypot" => math_hypot,
                    "random" => math_random,
                    _ => return None,
                };
                Some(native(math_name(key), s(key), f))
            }
        },
        "JSON" => match key {
            "stringify" => Some(native("stringify", V::Undef, json_stringify)),
            "parse" => Some(native("parse", V::Undef, json_parse)),
            _ => None,
        },
        "Number" => match key {
            "EPSILON" => c(f64::EPSILON),
            "MAX_SAFE_INTEGER" => c(9_007_199_254_740_991.0),
            "MIN_SAFE_INTEGER" => c(-9_007_199_254_740_991.0),
            "MAX_VALUE" => c(f64::MAX),
            "MIN_VALUE" => c(5e-324),
            "POSITIVE_INFINITY" => c(f64::INFINITY),
            "NEGATIVE_INFINITY" => c(f64::NEG_INFINITY),
            "NaN" => c(f64::NAN),
            "isInteger" => Some(native("isInteger", V::Undef, num_is_integer)),
            "isFinite" => Some(native("isFinite", V::Undef, num_is_finite)),
            "isNaN" => Some(native("isNaN", V::Undef, num_is_nan)),
            "parseFloat" => Some(native("parseFloat", V::Undef, g_parse_float)),
            "parseInt" => Some(native("parseInt", V::Undef, g_parse_int)),
            _ => None,
        },
        "String" => match key {
            "fromCharCode" | "fromCodePoint" => Some(native("fromCharCode", V::Undef, str_from_char_code)),
            _ => None,
        },
        "Array" => match key {
            "isArray" => Some(native("isArray", V::Undef, arr_is_array)),
            "from" => Some(native("from", V::Undef, arr_from)),
            "of" => Some(native("of", V::Undef, arr_of)),
            _ => None,
        },
        "Object" => match key {
            "keys" => Some(native("keys", V::Undef, obj_keys)),
            "values" => Some(native("values", V::Undef, obj_values)),
            "entries" => Some(native("entries", V::Undef, obj_entries)),
            "assign" => Some(native("assign", V::Undef, obj_assign)),
            "freeze" | "seal" => Some(native("freeze", V::Undef, obj_identity)),
            _ => None,
        },
        _ => None,
    }
}

fn math_name(k: &str) -> &'static str {
    const NAMES: [&str; 34] = [
        "abs", "acos", "acosh", "asin", "asinh", "atan", "atanh", "cbrt", "ceil", "cos", "cosh", "exp", "expm1", "floor", "fround", "log", "log1p", "log10", "log2", "round", "sign", "sin", "sinh", "sqrt", "tan", "tanh", "trunc", "atan2", "pow", "max", "min", "hypot", "random", "function",
    ];
    NAMES.iter().find(|n| **n == k).copied().unwrap_or("function")
}

pub fn construct(it: &mut Interp, name: &str, args: Vec<V>) -> R {
    match name {
        "Error" | "TypeError" | "RangeError" | "SyntaxError" | "ReferenceError" => {
            let msg = match args.first() {
                Some(v) => it.to_string(v)?,
                None => String::new(),
            };
            Ok(err_obj(name, &msg))
        }
        "Array" => g_array(it, &V::Undef, args),
        "Object" => g_object(it, &V::Undef, args),
        "Number" => g_number(it, &V::Undef, args),
        "String" => g_string(it, &V::Undef, args),
        "Boolean" => g_boolean(it, &V::Undef, args),
        "RegExp" => super::regexp::construct(it, args),
        other => Err(type_err(format!("`new {other}` is not supported in expressions"))),
    }
}

fn g_regexp(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    super::regexp::construct(it, a)
}

fn n0(it: &mut Interp, a: &[V], i: usize) -> Result<f64, Flow> {
    it.to_number(&arg(a, i))
}

fn math1(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let x = n0(it, &a, 0)?;
    let key = plain_string(this);
    Ok(V::Num(math_apply(&key, x)))
}

fn math_apply(key: &str, x: f64) -> f64 {
    match key {
        "abs" => x.abs(),
        "acos" => x.acos(),
        "acosh" => x.acosh(),
        "asin" => x.asin(),
        "asinh" => x.asinh(),
        "atan" => x.atan(),
        "atanh" => x.atanh(),
        "cbrt" => x.cbrt(),
        "ceil" => x.ceil(),
        "cos" => x.cos(),
        "cosh" => x.cosh(),
        "exp" => x.exp(),
        "expm1" => x.exp_m1(),
        "floor" => x.floor(),
        "fround" => x as f32 as f64,
        "log" => x.ln(),
        "log1p" => x.ln_1p(),
        "log10" => x.log10(),
        "log2" => x.log2(),
        "round" => (x + 0.5).floor(),
        "sign" => {
            if x.is_nan() || x == 0.0 {
                x
            } else {
                x.signum()
            }
        }
        "sin" => x.sin(),
        "sinh" => x.sinh(),
        "sqrt" => x.sqrt(),
        "tan" => x.tan(),
        "tanh" => x.tanh(),
        "trunc" => x.trunc(),
        _ => f64::NAN,
    }
}

fn math_atan2(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let y = n0(it, &a, 0)?;
    let x = n0(it, &a, 1)?;
    Ok(V::Num(y.atan2(x)))
}

fn math_pow(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let x = n0(it, &a, 0)?;
    let y = n0(it, &a, 1)?;
    Ok(V::Num(x.powf(y)))
}

fn math_max(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let mut m = f64::NEG_INFINITY;
    for v in &a {
        let x = it.to_number(v)?;
        if x.is_nan() {
            return Ok(V::Num(f64::NAN));
        }
        m = m.max(x);
    }
    Ok(V::Num(m))
}

fn math_min(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let mut m = f64::INFINITY;
    for v in &a {
        let x = it.to_number(v)?;
        if x.is_nan() {
            return Ok(V::Num(f64::NAN));
        }
        m = m.min(x);
    }
    Ok(V::Num(m))
}

fn math_hypot(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let mut sum = 0.0;
    for v in &a {
        let x = it.to_number(v)?;
        sum += x * x;
    }
    Ok(V::Num(sum.sqrt()))
}

fn math_random(it: &mut Interp, _: &V, _: Vec<V>) -> R {
    Ok(V::Num(it.cx.next_random()))
}

fn g_number(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    if a.is_empty() {
        return Ok(V::Num(0.0));
    }
    Ok(V::Num(n0(it, &a, 0)?))
}

fn g_string(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    if a.is_empty() {
        return Ok(s(""));
    }
    let t = it.to_string(&a[0])?;
    Ok(s(&t))
}

fn g_boolean(_: &mut Interp, _: &V, a: Vec<V>) -> R {
    Ok(V::Bool(truthy(&arg(&a, 0))))
}

fn g_array(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    if a.len() == 1
        && let V::Num(n) = a[0]
    {
        if !(n >= 0.0 && n.fract() == 0.0 && n < 10_000_000.0) {
            return Err(throw("RangeError", "invalid array length"));
        }
        return Ok(arr(vec![V::Undef; n as usize]));
    }
    let _ = it;
    Ok(arr(a))
}

fn g_object(_: &mut Interp, _: &V, a: Vec<V>) -> R {
    match a.into_iter().next() {
        Some(v @ (V::Obj(_) | V::Arr(_))) => Ok(v),
        _ => Ok(obj(vec![])),
    }
}

fn g_error(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let _ = this;
    let msg = match a.first() {
        Some(v) => it.to_string(v)?,
        None => String::new(),
    };
    Ok(err_obj("Error", &msg))
}

fn g_parse_int(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let t = it.to_string(&arg(&a, 0))?;
    let radix = match a.get(1) {
        Some(v) => it.to_number(v)? as u32,
        None => 0,
    };
    let t = t.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let (radix, t) = if (radix == 0 || radix == 16) && (t.starts_with("0x") || t.starts_with("0X")) { (16, &t[2..]) } else { (if radix == 0 { 10 } else { radix }, t) };
    if !(2..=36).contains(&radix) {
        return Ok(V::Num(f64::NAN));
    }
    let digits: String = t.chars().take_while(|c| c.is_digit(radix)).collect();
    if digits.is_empty() {
        return Ok(V::Num(f64::NAN));
    }
    let mut v = 0.0f64;
    for c in digits.chars() {
        v = v * radix as f64 + c.to_digit(radix).unwrap_or(0) as f64;
    }
    Ok(V::Num(if neg { -v } else { v }))
}

fn g_parse_float(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let t = it.to_string(&arg(&a, 0))?;
    let t = t.trim_start();
    if t.starts_with("Infinity") || t.starts_with("+Infinity") {
        return Ok(V::Num(f64::INFINITY));
    }
    if t.starts_with("-Infinity") {
        return Ok(V::Num(f64::NEG_INFINITY));
    }
    let mut end = 0;
    let b = t.as_bytes();
    let mut seen_dot = false;
    let mut seen_e = false;
    while end < b.len() {
        let c = b[end];
        let ok = c.is_ascii_digit()
            || ((c == b'+' || c == b'-') && (end == 0 || matches!(b[end - 1], b'e' | b'E')))
            || (c == b'.' && !seen_dot && !seen_e)
            || ((c == b'e' || c == b'E') && !seen_e && end > 0);
        if !ok {
            break;
        }
        if c == b'.' {
            seen_dot = true;
        }
        if c == b'e' || c == b'E' {
            seen_e = true;
        }
        end += 1;
    }
    let mut slice = &t[..end];
    while !slice.is_empty() && slice.parse::<f64>().is_err() {
        slice = &slice[..slice.len() - 1];
    }
    Ok(V::Num(slice.parse().unwrap_or(f64::NAN)))
}

fn g_is_nan(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    Ok(V::Bool(n0(it, &a, 0)?.is_nan()))
}

fn g_is_finite(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    Ok(V::Bool(n0(it, &a, 0)?.is_finite()))
}

fn g_identity_string(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let t = it.to_string(&arg(&a, 0))?;
    Ok(s(&t))
}

fn num_is_integer(_: &mut Interp, _: &V, a: Vec<V>) -> R {
    Ok(V::Bool(matches!(arg(&a, 0), V::Num(n) if n.is_finite() && n.fract() == 0.0)))
}

fn num_is_finite(_: &mut Interp, _: &V, a: Vec<V>) -> R {
    Ok(V::Bool(matches!(arg(&a, 0), V::Num(n) if n.is_finite())))
}

fn num_is_nan(_: &mut Interp, _: &V, a: Vec<V>) -> R {
    Ok(V::Bool(matches!(arg(&a, 0), V::Num(n) if n.is_nan())))
}

fn str_from_char_code(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let mut out = String::new();
    for v in &a {
        let n = it.to_number(v)?;
        out.push(char::from_u32(n as u32).unwrap_or('\u{fffd}'));
    }
    Ok(s(&out))
}

fn arr_is_array(_: &mut Interp, _: &V, a: Vec<V>) -> R {
    Ok(V::Bool(matches!(arg(&a, 0), V::Arr(_))))
}

fn arr_from(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let src = arg(&a, 0);
    let items = match &src {
        V::Obj(o) => {
            let n = o.borrow().get("length").cloned().unwrap_or(V::Num(0.0));
            let n = it.to_number(&n)?.max(0.0).min(10_000_000.0) as usize;
            vec![V::Undef; n]
        }
        _ => it.iterate(&src)?,
    };
    match a.get(1) {
        Some(f @ (V::Func(_) | V::Native(_))) => {
            let mut out = Vec::with_capacity(items.len());
            for (i, x) in items.into_iter().enumerate() {
                out.push(it.call(f, V::Undef, vec![x, V::Num(i as f64)])?);
            }
            Ok(arr(out))
        }
        _ => Ok(arr(items)),
    }
}

fn arr_of(_: &mut Interp, _: &V, a: Vec<V>) -> R {
    Ok(arr(a))
}

fn obj_keys(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let k = it.keys_of(&arg(&a, 0))?;
    Ok(arr(k.iter().map(|x| s(x)).collect()))
}

fn obj_values(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let o = arg(&a, 0);
    let keys = it.keys_of(&o)?;
    let mut out = vec![];
    for k in keys {
        out.push(it.get_member(&o, &k)?);
    }
    Ok(arr(out))
}

fn obj_entries(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let o = arg(&a, 0);
    let keys = it.keys_of(&o)?;
    let mut out = vec![];
    for k in keys {
        let v = it.get_member(&o, &k)?;
        out.push(arr(vec![s(&k), v]));
    }
    Ok(arr(out))
}

fn obj_assign(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let target = arg(&a, 0);
    let V::Obj(t) = &target else { return Ok(target) };
    for src in a.iter().skip(1) {
        let keys = it.keys_of(src)?;
        for k in keys {
            let v = it.get_member(src, &k)?;
            t.borrow_mut().set(&k, v);
        }
    }
    Ok(target)
}

fn obj_identity(_: &mut Interp, _: &V, a: Vec<V>) -> R {
    Ok(arg(&a, 0))
}

pub fn has_own(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let k = it.to_string(&arg(&a, 0))?;
    Ok(V::Bool(match this {
        V::Obj(o) => o.borrow().get(&k).is_some(),
        V::Arr(x) => k.parse::<usize>().is_ok_and(|i| i < x.borrow().len()),
        _ => false,
    }))
}

pub fn to_string_method(it: &mut Interp, this: &V, _: Vec<V>) -> R {
    let t = it.to_string(this)?;
    Ok(s(&t))
}

pub fn fn_call(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let mut a = a.into_iter();
    let t = a.next().unwrap_or(V::Undef);
    it.call(this, t, a.collect())
}

pub fn fn_apply(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let t = arg(&a, 0);
    let list = match a.get(1) {
        Some(V::Undef | V::Null) | None => vec![],
        Some(v) => it.iterate(v)?,
    };
    it.call(this, t, list)
}

pub fn fn_bind(_: &mut Interp, this: &V, a: Vec<V>) -> R {
    let bound = arr(std::iter::once(this.clone()).chain(a).collect());
    Ok(native("bound", bound, bound_call))
}

fn bound_call(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let V::Arr(parts) = this else { return Ok(V::Undef) };
    let parts = parts.borrow().clone();
    let f = parts.first().cloned().unwrap_or(V::Undef);
    let t = parts.get(1).cloned().unwrap_or(V::Undef);
    let mut args: Vec<V> = parts.get(2..).map(|x| x.to_vec()).unwrap_or_default();
    args.extend(a);
    it.call(&f, t, args)
}

pub fn number_method(key: &str) -> Option<M> {
    Some(match key {
        "toFixed" => ("toFixed", num_to_fixed),
        "toPrecision" => ("toPrecision", num_to_precision),
        "toString" => ("toString", num_to_string_m),
        "toLocaleString" => ("toLocaleString", to_string_method),
        "valueOf" => ("valueOf", value_of),
        _ => return None,
    })
}

fn value_of(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    Ok(this.clone())
}

fn this_num(this: &V) -> f64 {
    if let V::Num(n) = this { *n } else { f64::NAN }
}

fn num_to_fixed(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let d = n0(it, &a, 0)?;
    let d = if d.is_nan() { 0 } else { d as i64 };
    if !(0..=100).contains(&d) {
        return Err(throw("RangeError", "toFixed() digits must be between 0 and 100"));
    }
    let x = this_num(this);
    if !x.is_finite() || x.abs() >= 1e21 {
        return Ok(s(&num_to_string(x)));
    }
    let scale = 10f64.powi(d as i32);
    let r = ((x.abs() * scale) + 0.5 - 1e-9 * scale.recip().min(1.0)).floor() / scale;
    let r = if x < 0.0 { -r } else { r };
    let t = format!("{:.*}", d as usize, r);
    let t = if t.starts_with('-') && t[1..].chars().all(|c| c == '0' || c == '.') { t[1..].to_string() } else { t };
    Ok(s(&t))
}

fn num_to_precision(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let x = this_num(this);
    if matches!(arg(&a, 0), V::Undef) || !x.is_finite() {
        return Ok(s(&num_to_string(x)));
    }
    let p = n0(it, &a, 0)? as i32;
    if !(1..=100).contains(&p) {
        return Err(throw("RangeError", "toPrecision() argument must be between 1 and 100"));
    }
    if x == 0.0 {
        return Ok(s(&format!("{:.*}", (p - 1) as usize, 0.0)));
    }
    let e = x.abs().log10().floor() as i32;
    if e < -6 || e >= p {
        let t = format!("{:.*e}", (p - 1) as usize, x);
        let (m, ex) = t.split_once('e').unwrap_or((&t, "0"));
        let ex: i32 = ex.parse().unwrap_or(0);
        return Ok(s(&format!("{m}e{}{}", if ex >= 0 { "+" } else { "-" }, ex.abs())));
    }
    Ok(s(&format!("{:.*}", (p - 1 - e).max(0) as usize, x)))
}

fn num_to_string_m(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let x = this_num(this);
    let radix = match a.first() {
        Some(V::Undef) | None => 10,
        Some(v) => it.to_number(v)? as u32,
    };
    if radix == 10 || !x.is_finite() || x.fract() != 0.0 {
        return Ok(s(&num_to_string(x)));
    }
    if !(2..=36).contains(&radix) {
        return Err(throw("RangeError", "toString() radix must be between 2 and 36"));
    }
    let mut n = x.abs() as u64;
    let mut digits = vec![];
    loop {
        digits.push(std::char::from_digit((n % radix as u64) as u32, radix).unwrap_or('0'));
        n /= radix as u64;
        if n == 0 {
            break;
        }
    }
    if x < 0.0 {
        digits.push('-');
    }
    Ok(s(&digits.iter().rev().collect::<String>()))
}

pub fn string_method(key: &str) -> Option<M> {
    Some(match key {
        "charAt" => ("charAt", str_char_at),
        "charCodeAt" | "codePointAt" => ("charCodeAt", str_char_code_at),
        "indexOf" => ("indexOf", str_index_of),
        "lastIndexOf" => ("lastIndexOf", str_last_index_of),
        "includes" => ("includes", str_includes),
        "startsWith" => ("startsWith", str_starts_with),
        "endsWith" => ("endsWith", str_ends_with),
        "slice" => ("slice", str_slice),
        "substring" => ("substring", str_substring),
        "substr" => ("substr", str_substr),
        "toUpperCase" | "toLocaleUpperCase" => ("toUpperCase", str_upper),
        "toLowerCase" | "toLocaleLowerCase" => ("toLowerCase", str_lower),
        "trim" => ("trim", str_trim),
        "trimStart" | "trimLeft" => ("trimStart", str_trim_start),
        "trimEnd" | "trimRight" => ("trimEnd", str_trim_end),
        "padStart" => ("padStart", str_pad_start),
        "padEnd" => ("padEnd", str_pad_end),
        "repeat" => ("repeat", str_repeat),
        "split" => ("split", str_split),
        "replace" => ("replace", str_replace),
        "replaceAll" => ("replaceAll", str_replace_all),
        "concat" => ("concat", str_concat),
        "at" => ("at", str_at),
        "localeCompare" => ("localeCompare", str_locale_compare),
        "toString" | "valueOf" => ("toString", value_of),
        "match" => ("match", super::regexp::str_match),
        "matchAll" => ("matchAll", super::regexp::str_match_all),
        "search" => ("search", super::regexp::str_search),
        _ => return None,
    })
}

fn this_chars(this: &V) -> Vec<char> {
    if let V::Str(t) = this { t.chars().collect() } else { vec![] }
}

fn this_str(this: &V) -> String {
    if let V::Str(t) = this { t.to_string() } else { String::new() }
}

fn rel_index(it: &mut Interp, v: &V, len: usize, default: usize) -> Result<usize, Flow> {
    if matches!(v, V::Undef) {
        return Ok(default);
    }
    let n = it.to_number(v)?;
    let n = if n.is_nan() { 0.0 } else { n.trunc() };
    Ok(if n < 0.0 { (len as f64 + n).max(0.0) as usize } else { (n as usize).min(len) })
}

fn clamp_index(it: &mut Interp, v: &V, len: usize, default: usize) -> Result<usize, Flow> {
    if matches!(v, V::Undef) {
        return Ok(default);
    }
    let n = it.to_number(v)?;
    let n = if n.is_nan() { 0.0 } else { n.trunc() };
    Ok(n.clamp(0.0, len as f64) as usize)
}

fn str_char_at(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = this_chars(this);
    let i = n0(it, &a, 0)?;
    let i = if i.is_nan() { 0 } else { i as i64 };
    Ok(s(&if i >= 0 { c.get(i as usize).map(|x| x.to_string()).unwrap_or_default() } else { String::new() }))
}

fn str_char_code_at(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = this_chars(this);
    let i = n0(it, &a, 0)?;
    let i = if i.is_nan() { 0 } else { i as i64 };
    Ok(V::Num(if i >= 0 { c.get(i as usize).map(|x| *x as u32 as f64).unwrap_or(f64::NAN) } else { f64::NAN }))
}

fn find_from(hay: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(from.min(hay.len()));
    }
    (from..hay.len()).find(|&i| hay[i..].starts_with(needle))
}

fn str_index_of(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let h = this_chars(this);
    let n: Vec<char> = it.to_string(&arg(&a, 0))?.chars().collect();
    let from = clamp_index(it, &arg(&a, 1), h.len(), 0)?;
    Ok(V::Num(find_from(&h, &n, from).map(|i| i as f64).unwrap_or(-1.0)))
}

fn str_last_index_of(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let h = this_chars(this);
    let n: Vec<char> = it.to_string(&arg(&a, 0))?.chars().collect();
    if n.len() > h.len() {
        return Ok(V::Num(-1.0));
    }
    let last = (0..=h.len() - n.len()).rev().find(|&i| h[i..].starts_with(&n));
    Ok(V::Num(last.map(|i| i as f64).unwrap_or(-1.0)))
}

fn str_includes(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let n = it.to_string(&arg(&a, 0))?;
    Ok(V::Bool(this_str(this).contains(&n)))
}

fn str_starts_with(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let n = it.to_string(&arg(&a, 0))?;
    Ok(V::Bool(this_str(this).starts_with(&n)))
}

fn str_ends_with(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let n = it.to_string(&arg(&a, 0))?;
    Ok(V::Bool(this_str(this).ends_with(&n)))
}

fn str_slice(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = this_chars(this);
    let b = rel_index(it, &arg(&a, 0), c.len(), 0)?;
    let e = rel_index(it, &arg(&a, 1), c.len(), c.len())?;
    Ok(s(&if b < e { c[b..e].iter().collect::<String>() } else { String::new() }))
}

fn str_substring(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = this_chars(this);
    let b = clamp_index(it, &arg(&a, 0), c.len(), 0)?;
    let e = clamp_index(it, &arg(&a, 1), c.len(), c.len())?;
    let (b, e) = if b > e { (e, b) } else { (b, e) };
    Ok(s(&c[b..e].iter().collect::<String>()))
}

fn str_substr(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = this_chars(this);
    let b = rel_index(it, &arg(&a, 0), c.len(), 0)?;
    let n = match arg(&a, 1) {
        V::Undef => c.len(),
        v => it.to_number(&v)?.max(0.0) as usize,
    };
    let e = (b + n).min(c.len());
    Ok(s(&c[b.min(e)..e].iter().collect::<String>()))
}

fn str_upper(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    Ok(s(&this_str(this).to_uppercase()))
}

fn str_lower(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    Ok(s(&this_str(this).to_lowercase()))
}

fn str_trim(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    Ok(s(this_str(this).trim()))
}

fn str_trim_start(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    Ok(s(this_str(this).trim_start()))
}

fn str_trim_end(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    Ok(s(this_str(this).trim_end()))
}

fn pad(it: &mut Interp, this: &V, a: &[V], start: bool) -> R {
    let t = this_str(this);
    let target = n0(it, a, 0)?.max(0.0) as usize;
    let fill = match arg(a, 1) {
        V::Undef => " ".to_string(),
        v => it.to_string(&v)?,
    };
    let len = t.chars().count();
    if target <= len || fill.is_empty() || target > 1_000_000 {
        return Ok(s(&t));
    }
    let need = target - len;
    let padding: String = fill.chars().cycle().take(need).collect();
    Ok(s(&if start { padding + &t } else { t + &padding }))
}

fn str_pad_start(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    pad(it, this, &a, true)
}

fn str_pad_end(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    pad(it, this, &a, false)
}

fn str_repeat(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let n = n0(it, &a, 0)?;
    if !(n >= 0.0) || n.is_infinite() {
        return Err(throw("RangeError", "invalid repeat count"));
    }
    let t = this_str(this);
    if t.len() * n as usize > 10_000_000 {
        return Err(throw("RangeError", "the repeated string is too long"));
    }
    Ok(s(&t.repeat(n as usize)))
}

fn str_split(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let t = this_str(this);
    let limit = match arg(&a, 1) {
        V::Undef => usize::MAX,
        v => it.to_number(&v)? as usize,
    };
    let parts: Vec<V> = match arg(&a, 0) {
        V::Undef => vec![s(&t)],
        sep if super::regexp::parts(&sep).is_some() => return super::regexp::split(it, &t, &sep, limit),
        sep => {
            let sep = it.to_string(&sep)?;
            if sep.is_empty() { t.chars().map(|c| s(&c.to_string())).collect() } else { t.split(sep.as_str()).map(s).collect() }
        }
    };
    Ok(arr(parts.into_iter().take(limit).collect()))
}

fn replace_impl(it: &mut Interp, this: &V, a: &[V], all: bool) -> R {
    let t = this_str(this);
    if super::regexp::parts(&arg(a, 0)).is_some() {
        return super::regexp::replace(it, &t, &arg(a, 0), &arg(a, 1), all);
    }
    let pat = it.to_string(&arg(a, 0))?;
    let rep = arg(a, 1);
    let mut out = String::new();
    let mut rest = t.as_str();
    let mut offset = 0usize;
    loop {
        let found = if pat.is_empty() { Some(0) } else { rest.find(pat.as_str()) };
        let Some(i) = found else { break };
        out.push_str(&rest[..i]);
        let r = match &rep {
            V::Func(_) | V::Native(_) => {
                let pos = t[..offset + i].chars().count();
                let v = it.call(&rep, V::Undef, vec![s(&pat), V::Num(pos as f64), s(&t)])?;
                it.to_string(&v)?
            }
            v => it.to_string(v)?.replace("$&", &pat),
        };
        out.push_str(&r);
        let adv = i + pat.len();
        offset += adv;
        rest = &rest[adv..];
        if !all {
            break;
        }
        if pat.is_empty() {
            match rest.chars().next() {
                Some(c) => {
                    out.push(c);
                    rest = &rest[c.len_utf8()..];
                    offset += c.len_utf8();
                }
                None => break,
            }
        }
    }
    out.push_str(rest);
    Ok(s(&out))
}

fn str_replace(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    replace_impl(it, this, &a, false)
}

fn str_replace_all(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    replace_impl(it, this, &a, true)
}

fn str_concat(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let mut t = this_str(this);
    for v in &a {
        t.push_str(&it.to_string(v)?);
    }
    Ok(s(&t))
}

fn str_at(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let c = this_chars(this);
    let i = n0(it, &a, 0)?.trunc();
    let i = if i < 0.0 { c.len() as f64 + i } else { i };
    Ok(if i >= 0.0 { c.get(i as usize).map(|x| s(&x.to_string())).unwrap_or(V::Undef) } else { V::Undef })
}

fn str_locale_compare(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let o = it.to_string(&arg(&a, 0))?;
    Ok(V::Num(match this_str(this).cmp(&o) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    }))
}

pub fn array_method(key: &str) -> Option<M> {
    Some(match key {
        "push" => ("push", arr_push),
        "pop" => ("pop", arr_pop),
        "shift" => ("shift", arr_shift),
        "unshift" => ("unshift", arr_unshift),
        "slice" => ("slice", arr_slice),
        "splice" => ("splice", arr_splice),
        "concat" => ("concat", arr_concat),
        "join" => ("join", arr_join),
        "indexOf" => ("indexOf", arr_index_of),
        "lastIndexOf" => ("lastIndexOf", arr_last_index_of),
        "includes" => ("includes", arr_includes),
        "reverse" => ("reverse", arr_reverse),
        "sort" => ("sort", arr_sort),
        "map" => ("map", arr_map),
        "filter" => ("filter", arr_filter),
        "reduce" => ("reduce", arr_reduce),
        "reduceRight" => ("reduceRight", arr_reduce_right),
        "forEach" => ("forEach", arr_for_each),
        "some" => ("some", arr_some),
        "every" => ("every", arr_every),
        "find" => ("find", arr_find),
        "findIndex" => ("findIndex", arr_find_index),
        "fill" => ("fill", arr_fill),
        "flat" => ("flat", arr_flat),
        "at" => ("at", arr_at),
        "toString" => ("toString", to_string_method),
        "valueOf" => ("valueOf", value_of),
        _ => return None,
    })
}

fn this_arr(this: &V) -> Rc<RefCell<Vec<V>>> {
    match this {
        V::Arr(a) => a.clone(),
        _ => Rc::new(RefCell::new(vec![])),
    }
}

fn arr_push(_: &mut Interp, this: &V, a: Vec<V>) -> R {
    let x = this_arr(this);
    x.borrow_mut().extend(a);
    let n = x.borrow().len();
    Ok(V::Num(n as f64))
}

fn arr_pop(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    Ok(this_arr(this).borrow_mut().pop().unwrap_or(V::Undef))
}

fn arr_shift(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    let x = this_arr(this);
    let mut b = x.borrow_mut();
    Ok(if b.is_empty() { V::Undef } else { b.remove(0) })
}

fn arr_unshift(_: &mut Interp, this: &V, a: Vec<V>) -> R {
    let x = this_arr(this);
    let mut b = x.borrow_mut();
    for (i, v) in a.into_iter().enumerate() {
        b.insert(i, v);
    }
    Ok(V::Num(b.len() as f64))
}

fn arr_slice(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let x = this_arr(this).borrow().clone();
    let b = rel_index(it, &arg(&a, 0), x.len(), 0)?;
    let e = rel_index(it, &arg(&a, 1), x.len(), x.len())?;
    Ok(arr(if b < e { x[b..e].to_vec() } else { vec![] }))
}

fn arr_splice(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let x = this_arr(this);
    let len = x.borrow().len();
    let start = rel_index(it, &arg(&a, 0), len, 0)?;
    let count = match a.get(1) {
        None => len - start,
        Some(v) => (it.to_number(v)?.max(0.0) as usize).min(len - start),
    };
    let insert: Vec<V> = a.get(2..).map(|s| s.to_vec()).unwrap_or_default();
    let removed: Vec<V> = x.borrow_mut().splice(start..start + count, insert).collect();
    Ok(arr(removed))
}

fn arr_concat(_: &mut Interp, this: &V, a: Vec<V>) -> R {
    let mut out = this_arr(this).borrow().clone();
    for v in a {
        match v {
            V::Arr(o) => out.extend(o.borrow().iter().cloned()),
            other => out.push(other),
        }
    }
    Ok(arr(out))
}

fn arr_join(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let sep = match arg(&a, 0) {
        V::Undef => ",".to_string(),
        v => it.to_string(&v)?,
    };
    let items = this_arr(this).borrow().clone();
    let mut parts = vec![];
    for v in &items {
        parts.push(if matches!(v, V::Undef | V::Null) { String::new() } else { it.to_string(v)? });
    }
    Ok(s(&parts.join(&sep)))
}

fn arr_index_of(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let items = this_arr(this).borrow().clone();
    let from = clamp_index(it, &arg(&a, 1), items.len(), 0)?;
    let t = arg(&a, 0);
    Ok(V::Num(items.iter().enumerate().skip(from).find(|(_, v)| strict_eq(v, &t)).map(|(i, _)| i as f64).unwrap_or(-1.0)))
}

fn arr_last_index_of(_: &mut Interp, this: &V, a: Vec<V>) -> R {
    let items = this_arr(this).borrow().clone();
    let t = arg(&a, 0);
    Ok(V::Num(items.iter().rposition(|v| strict_eq(v, &t)).map(|i| i as f64).unwrap_or(-1.0)))
}

fn arr_includes(_: &mut Interp, this: &V, a: Vec<V>) -> R {
    let items = this_arr(this).borrow().clone();
    let t = arg(&a, 0);
    Ok(V::Bool(items.iter().any(|v| strict_eq(v, &t) || matches!((v, &t), (V::Num(x), V::Num(y)) if x.is_nan() && y.is_nan()))))
}

fn arr_reverse(_: &mut Interp, this: &V, _: Vec<V>) -> R {
    this_arr(this).borrow_mut().reverse();
    Ok(this.clone())
}

fn compare(it: &mut Interp, f: &V, x: &V, y: &V) -> Result<bool, Flow> {
    if matches!(x, V::Undef) {
        return Ok(false);
    }
    if matches!(y, V::Undef) {
        return Ok(true);
    }
    if matches!(f, V::Undef) {
        let a = it.to_string(x)?;
        let b = it.to_string(y)?;
        return Ok(a <= b);
    }
    let r = it.call(f, V::Undef, vec![x.clone(), y.clone()])?;
    let n = it.to_number(&r)?;
    Ok(n <= 0.0 || n.is_nan())
}

fn arr_sort(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let f = arg(&a, 0);
    let items = this_arr(this).borrow().clone();
    let mut v = items;
    let mut width = 1;
    let n = v.len();
    while width < n {
        let mut out = Vec::with_capacity(n);
        let mut i = 0;
        while i < n {
            let mid = (i + width).min(n);
            let end = (i + 2 * width).min(n);
            let (mut l, mut r) = (i, mid);
            while l < mid && r < end {
                if compare(it, &f, &v[l], &v[r])? {
                    out.push(v[l].clone());
                    l += 1;
                } else {
                    out.push(v[r].clone());
                    r += 1;
                }
            }
            out.extend_from_slice(&v[l..mid]);
            out.extend_from_slice(&v[r..end]);
            i = end;
        }
        v = out;
        width *= 2;
    }
    *this_arr(this).borrow_mut() = v;
    Ok(this.clone())
}

fn each<F>(it: &mut Interp, this: &V, a: &[V], mut f: F) -> Result<(), Flow>
where
    F: FnMut(&mut Interp, usize, V, V) -> Result<bool, Flow>,
{
    let cb = arg(a, 0);
    if !matches!(cb, V::Func(_) | V::Native(_)) {
        return Err(type_err("the callback is not a function"));
    }
    let items = this_arr(this).borrow().clone();
    for (i, x) in items.into_iter().enumerate() {
        let r = it.call(&cb, V::Undef, vec![x.clone(), V::Num(i as f64), this.clone()])?;
        if !f(it, i, x, r)? {
            break;
        }
    }
    Ok(())
}

fn arr_map(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let mut out = vec![];
    each(it, this, &a, |_, _, _, r| {
        out.push(r);
        Ok(true)
    })?;
    Ok(arr(out))
}

fn arr_filter(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let mut out = vec![];
    each(it, this, &a, |_, _, x, r| {
        if truthy(&r) {
            out.push(x);
        }
        Ok(true)
    })?;
    Ok(arr(out))
}

fn arr_for_each(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    each(it, this, &a, |_, _, _, _| Ok(true))?;
    Ok(V::Undef)
}

fn arr_some(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let mut found = false;
    each(it, this, &a, |_, _, _, r| {
        found = truthy(&r);
        Ok(!found)
    })?;
    Ok(V::Bool(found))
}

fn arr_every(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let mut all = true;
    each(it, this, &a, |_, _, _, r| {
        all = truthy(&r);
        Ok(all)
    })?;
    Ok(V::Bool(all))
}

fn arr_find(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let mut out = V::Undef;
    each(it, this, &a, |_, _, x, r| {
        if truthy(&r) {
            out = x;
            return Ok(false);
        }
        Ok(true)
    })?;
    Ok(out)
}

fn arr_find_index(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let mut out = -1.0;
    each(it, this, &a, |_, i, _, r| {
        if truthy(&r) {
            out = i as f64;
            return Ok(false);
        }
        Ok(true)
    })?;
    Ok(V::Num(out))
}

fn reduce_impl(it: &mut Interp, this: &V, a: &[V], right: bool) -> R {
    let cb = arg(a, 0);
    let mut items = this_arr(this).borrow().clone();
    let mut idx: Vec<usize> = (0..items.len()).collect();
    if right {
        items.reverse();
        idx.reverse();
    }
    let mut iter = items.into_iter().zip(idx);
    let mut acc = if a.len() >= 2 {
        a[1].clone()
    } else {
        match iter.next() {
            Some((v, _)) => v,
            None => return Err(type_err("reduce of an empty array with no initial value")),
        }
    };
    for (x, i) in iter {
        acc = it.call(&cb, V::Undef, vec![acc, x, V::Num(i as f64), this.clone()])?;
    }
    Ok(acc)
}

fn arr_reduce(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    reduce_impl(it, this, &a, false)
}

fn arr_reduce_right(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    reduce_impl(it, this, &a, true)
}

fn arr_fill(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let x = this_arr(this);
    let len = x.borrow().len();
    let b = rel_index(it, &arg(&a, 1), len, 0)?;
    let e = rel_index(it, &arg(&a, 2), len, len)?;
    let v = arg(&a, 0);
    for slot in x.borrow_mut().iter_mut().take(e).skip(b) {
        *slot = v.clone();
    }
    Ok(this.clone())
}

fn flatten(v: &[V], depth: i32, out: &mut Vec<V>) {
    for x in v {
        match x {
            V::Arr(inner) if depth > 0 => flatten(&inner.borrow(), depth - 1, out),
            other => out.push(other.clone()),
        }
    }
}

fn arr_flat(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let d = match arg(&a, 0) {
        V::Undef => 1,
        v => it.to_number(&v)? as i32,
    };
    let mut out = vec![];
    flatten(&this_arr(this).borrow(), d, &mut out);
    Ok(arr(out))
}

fn arr_at(it: &mut Interp, this: &V, a: Vec<V>) -> R {
    let items = this_arr(this).borrow().clone();
    let i = n0(it, &a, 0)?.trunc();
    let i = if i < 0.0 { items.len() as f64 + i } else { i };
    Ok(if i >= 0.0 { items.get(i as usize).cloned().unwrap_or(V::Undef) } else { V::Undef })
}

fn json_quote(t: &str) -> String {
    serde_json::to_string(t).unwrap_or_else(|_| "\"\"".into())
}

fn json_write(it: &mut Interp, v: &V, indent: &str, level: usize, out: &mut String) -> Result<bool, Flow> {
    let nl = |lvl: usize| if indent.is_empty() { String::new() } else { format!("\n{}", indent.repeat(lvl)) };
    let p = it.primitive(v)?;
    match &p {
        V::Undef | V::Func(_) | V::Native(_) => return Ok(false),
        V::Null => out.push_str("null"),
        V::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        V::Num(n) => out.push_str(&if n.is_finite() { num_to_string(*n) } else { "null".into() }),
        V::Str(t) => out.push_str(&json_quote(t)),
        V::Arr(a) => {
            let items = a.borrow().clone();
            out.push('[');
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&nl(level + 1));
                if !json_write(it, x, indent, level + 1, out)? {
                    out.push_str("null");
                }
            }
            if !items.is_empty() {
                out.push_str(&nl(level));
            }
            out.push(']');
        }
        V::Obj(o) => {
            let props = o.borrow().props.clone();
            out.push('{');
            let mut first = true;
            for (k, x) in props.iter() {
                let mut piece = String::new();
                if !json_write(it, x, indent, level + 1, &mut piece)? {
                    continue;
                }
                if !first {
                    out.push(',');
                }
                first = false;
                out.push_str(&nl(level + 1));
                out.push_str(&json_quote(k));
                out.push(':');
                if !indent.is_empty() {
                    out.push(' ');
                }
                out.push_str(&piece);
            }
            if !first {
                out.push_str(&nl(level));
            }
            out.push('}');
        }
        V::Host(h) => out.push_str(&json_quote(&h.describe())),
        V::Path(_) => out.push_str("{}"),
    }
    Ok(true)
}

fn json_stringify(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let indent = match arg(&a, 2) {
        V::Num(n) => " ".repeat(n.clamp(0.0, 10.0) as usize),
        V::Str(t) => t.chars().take(10).collect(),
        _ => String::new(),
    };
    let mut out = String::new();
    if !json_write(it, &arg(&a, 0), &indent, 0, &mut out)? {
        return Ok(V::Undef);
    }
    Ok(s(&out))
}

pub fn from_json(j: &serde_json::Value) -> V {
    match j {
        serde_json::Value::Null => V::Null,
        serde_json::Value::Bool(b) => V::Bool(*b),
        serde_json::Value::Number(n) => V::Num(n.as_f64().unwrap_or(f64::NAN)),
        serde_json::Value::String(t) => s(t),
        serde_json::Value::Array(a) => arr(a.iter().map(from_json).collect()),
        serde_json::Value::Object(o) => V::Obj(Rc::new(RefCell::new(Obj { props: o.iter().map(|(k, v)| (Rc::from(k.as_str()), from_json(v))).collect() }))),
    }
}

fn json_parse(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let t = it.to_string(&arg(&a, 0))?;
    let j: serde_json::Value = serde_json::from_str(&t).map_err(|e| throw("SyntaxError", format!("JSON.parse: {e}")))?;
    Ok(from_json(&j))
}

pub fn num_or_vec(it: &mut Interp, v: &V) -> Result<Result<f64, Vec<f64>>, Flow> {
    if let Some(vec) = it.vector(v)? {
        return Ok(Err(vec));
    }
    Ok(Ok(it.to_number(v)?))
}

fn mix(it: &mut Interp, a: &V, b: &V, u: f64) -> R {
    match (num_or_vec(it, a)?, num_or_vec(it, b)?) {
        (Ok(x), Ok(y)) => Ok(V::Num(x + (y - x) * u)),
        (_, y) => {
            let x = vec_of(it, a)?;
            let y = match y {
                Ok(n) => vec![n; x.len().max(1)],
                Err(v) => v,
            };
            let x = if x.len() == 1 && y.len() > 1 { vec![x[0]; y.len()] } else { x };
            let n = x.len().max(y.len());
            Ok(nums(&(0..n).map(|i| {
                let p = x.get(i).copied().unwrap_or(0.0);
                let q = y.get(i).copied().unwrap_or(0.0);
                p + (q - p) * u
            }).collect::<Vec<_>>()))
        }
    }
}

fn interp_args(it: &mut Interp, a: &[V]) -> Result<(f64, V, V), Flow> {
    let t = n0(it, a, 0)?;
    if a.len() <= 3 {
        return Ok((t.clamp(0.0, 1.0), arg(a, 1), arg(a, 2)));
    }
    let lo = n0(it, a, 1)?;
    let hi = n0(it, a, 2)?;
    let u = if (hi - lo).abs() < 1e-12 { if t < lo { 0.0 } else { 1.0 } } else { ((t - lo) / (hi - lo)).clamp(0.0, 1.0) };
    Ok((u, arg(a, 3), arg(a, 4)))
}

fn ae_linear(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let (u, x, y) = interp_args(it, &a)?;
    mix(it, &x, &y, u)
}

fn ae_ease(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let (u, x, y) = interp_args(it, &a)?;
    mix(it, &x, &y, u * u * (3.0 - 2.0 * u))
}

fn ae_ease_in(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let (u, x, y) = interp_args(it, &a)?;
    mix(it, &x, &y, 2.0 * u * u - u * u * u)
}

fn ae_ease_out(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let (u, x, y) = interp_args(it, &a)?;
    mix(it, &x, &y, u + u * u - u * u * u)
}

fn vec_of(it: &mut Interp, v: &V) -> Result<Vec<f64>, Flow> {
    Ok(match num_or_vec(it, v)? {
        Ok(n) => vec![n],
        Err(v) => v,
    })
}

fn ae_clamp(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let v = num_or_vec(it, &arg(&a, 0))?;
    let lo = vec_of(it, &arg(&a, 1))?;
    let hi = vec_of(it, &arg(&a, 2))?;
    let cl = |x: f64, i: usize| {
        let l = lo.get(i).or(lo.first()).copied().unwrap_or(f64::NEG_INFINITY);
        let h = hi.get(i).or(hi.first()).copied().unwrap_or(f64::INFINITY);
        if l > h { x.clamp(h, l) } else { x.clamp(l, h) }
    };
    Ok(match v {
        Ok(n) => V::Num(cl(n, 0)),
        Err(v) => nums(&v.iter().enumerate().map(|(i, x)| cl(*x, i)).collect::<Vec<_>>()),
    })
}

fn ae_add(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    it.binary(super::ast::BinOp::Add, &arg(&a, 0), &arg(&a, 1))
}

fn ae_sub(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    it.binary(super::ast::BinOp::Sub, &arg(&a, 0), &arg(&a, 1))
}

fn ae_mul(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    it.binary(super::ast::BinOp::Mul, &arg(&a, 0), &arg(&a, 1))
}

fn ae_div(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    it.binary(super::ast::BinOp::Div, &arg(&a, 0), &arg(&a, 1))
}

fn ae_dot(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let x = vec_of(it, &arg(&a, 0))?;
    let y = vec_of(it, &arg(&a, 1))?;
    Ok(V::Num(x.iter().zip(&y).map(|(p, q)| p * q).sum()))
}

fn ae_cross(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let x = vec_of(it, &arg(&a, 0))?;
    let y = vec_of(it, &arg(&a, 1))?;
    let g = |v: &Vec<f64>, i: usize| v.get(i).copied().unwrap_or(0.0);
    Ok(nums(&[g(&x, 1) * g(&y, 2) - g(&x, 2) * g(&y, 1), g(&x, 2) * g(&y, 0) - g(&x, 0) * g(&y, 2), g(&x, 0) * g(&y, 1) - g(&x, 1) * g(&y, 0)]))
}

fn ae_normalize(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let x = vec_of(it, &arg(&a, 0))?;
    let l = x.iter().map(|v| v * v).sum::<f64>().sqrt();
    if l < 1e-300 {
        return Ok(nums(&x));
    }
    Ok(nums(&x.iter().map(|v| v / l).collect::<Vec<_>>()))
}

fn ae_length(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let x = vec_of(it, &arg(&a, 0))?;
    let x = if a.len() >= 2 {
        let y = vec_of(it, &arg(&a, 1))?;
        let n = x.len().max(y.len());
        (0..n).map(|i| x.get(i).copied().unwrap_or(0.0) - y.get(i).copied().unwrap_or(0.0)).collect()
    } else {
        x
    };
    Ok(V::Num(x.iter().map(|v| v * v).sum::<f64>().sqrt()))
}

fn ae_look_at(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let f = vec_of(it, &arg(&a, 0))?;
    let t = vec_of(it, &arg(&a, 1))?;
    let g = |v: &Vec<f64>, i: usize| v.get(i).copied().unwrap_or(0.0);
    let d = [g(&t, 0) - g(&f, 0), g(&t, 1) - g(&f, 1), g(&t, 2) - g(&f, 2)];
    let horiz = (d[0] * d[0] + d[2] * d[2]).sqrt();
    let rx = -(d[1].atan2(horiz)).to_degrees();
    let ry = d[0].atan2(d[2]).to_degrees();
    Ok(nums(&[rx, ry, 0.0]))
}

fn ae_d2r(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    Ok(V::Num(n0(it, &a, 0)?.to_radians()))
}

fn ae_r2d(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    Ok(V::Num(n0(it, &a, 0)?.to_degrees()))
}

fn ae_rgb_to_hsl(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let c = vec_of(it, &arg(&a, 0))?;
    let g = |i: usize| c.get(i).copied().unwrap_or(if i == 3 { 1.0 } else { 0.0 });
    let (r, gg, b) = (g(0), g(1), g(2));
    let mx = r.max(gg).max(b);
    let mn = r.min(gg).min(b);
    let l = (mx + mn) / 2.0;
    let (h, sat) = if (mx - mn).abs() < 1e-12 {
        (0.0, 0.0)
    } else {
        let d = mx - mn;
        let sat = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
        let h = if mx == r {
            (gg - b) / d + if gg < b { 6.0 } else { 0.0 }
        } else if mx == gg {
            (b - r) / d + 2.0
        } else {
            (r - gg) / d + 4.0
        };
        (h / 6.0, sat)
    };
    Ok(nums(&[h, sat, l, g(3)]))
}

fn hue(p: f64, q: f64, t: f64) -> f64 {
    let t = t.rem_euclid(1.0);
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 0.5 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

fn ae_hsl_to_rgb(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let c = vec_of(it, &arg(&a, 0))?;
    let g = |i: usize| c.get(i).copied().unwrap_or(if i == 3 { 1.0 } else { 0.0 });
    let (h, sat, l) = (g(0), g(1), g(2));
    if sat <= 0.0 {
        return Ok(nums(&[l, l, l, g(3)]));
    }
    let q = if l < 0.5 { l * (1.0 + sat) } else { l + sat - l * sat };
    let p = 2.0 * l - q;
    Ok(nums(&[hue(p, q, h + 1.0 / 3.0), hue(p, q, h), hue(p, q, h - 1.0 / 3.0), g(3)]))
}

fn ae_hex_to_rgb(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let t = it.to_string(&arg(&a, 0))?;
    let c = crate::model::value::hex_color(t.trim()).ok_or_else(|| type_err(format!("`{t}` is not a hex colour")))?;
    Ok(nums(&c))
}

fn rand_range(it: &mut Interp, a: &[V], u: f64) -> R {
    match a.len() {
        0 => Ok(V::Num(u)),
        1 => match num_or_vec(it, &a[0])? {
            Ok(m) => Ok(V::Num(u * m)),
            Err(v) => {
                let mut out = vec![];
                for (i, m) in v.iter().enumerate() {
                    let r = if i == 0 { u } else { it.cx.next_random() };
                    out.push(r * m);
                }
                Ok(nums(&out))
            }
        },
        _ => {
            let lo = num_or_vec(it, &a[0])?;
            let hi = num_or_vec(it, &a[1])?;
            match (lo, hi) {
                (Ok(l), Ok(h)) => Ok(V::Num(l + (h - l) * u)),
                (_, h) => {
                    let lo = vec_of(it, &a[0])?;
                    let hi = match h {
                        Ok(n) => vec![n; lo.len().max(1)],
                        Err(v) => v,
                    };
                    let n = lo.len().max(hi.len());
                    let mut out = vec![];
                    for i in 0..n {
                        let r = if i == 0 { u } else { it.cx.next_random() };
                        let p = lo.get(i).or(lo.first()).copied().unwrap_or(0.0);
                        let q = hi.get(i).or(hi.first()).copied().unwrap_or(0.0);
                        out.push(p + (q - p) * r);
                    }
                    Ok(nums(&out))
                }
            }
        }
    }
}

fn ae_random(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let u = it.cx.next_random();
    rand_range(it, &a, u)
}

fn ae_gauss_random(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let u1 = it.cx.next_random().max(1e-12);
    let u2 = it.cx.next_random();
    let g = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
    let u = (0.5 + g / 6.0).clamp(-0.5, 1.5);
    rand_range(it, &a, u)
}

fn ae_seed_random(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let seed = n0(it, &a, 0)?;
    let timeless = truthy(&arg(&a, 1));
    it.cx.seed_random(seed, timeless);
    Ok(V::Undef)
}

fn ae_noise(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let v = vec_of(it, &arg(&a, 0))?;
    let g = |i: usize| v.get(i).copied().unwrap_or(0.0);
    Ok(V::Num(super::noise::perlin3(g(0), g(1), g(2), 0)))
}

fn ae_time_to_frames(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let (fps, start) = it.cx.comp_rate_start();
    let t = match arg(&a, 0) {
        V::Undef => it.cx.time + start,
        v => it.to_number(&v)?,
    };
    let fps = match arg(&a, 1) {
        V::Undef => fps,
        v => it.to_number(&v)?,
    };
    Ok(V::Num((t * fps + 1e-6).floor()))
}

fn ae_frames_to_time(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let (fps, _) = it.cx.comp_rate_start();
    let f = n0(it, &a, 0)?;
    let fps = match arg(&a, 1) {
        V::Undef => fps,
        v => it.to_number(&v)?,
    };
    Ok(V::Num(f / fps))
}

fn ae_time_to_timecode(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let (fps, start) = it.cx.comp_rate_start();
    let t = match arg(&a, 0) {
        V::Undef => it.cx.time + start,
        v => it.to_number(&v)?,
    };
    let base = match arg(&a, 1) {
        V::Undef => fps.round().max(1.0),
        v => it.to_number(&v)?.max(1.0),
    };
    let neg = t < 0.0;
    let total = (t.abs() * base + 1e-6).floor() as i64;
    let b = base as i64;
    let ff = total % b;
    let secs = total / b;
    let txt = format!("{}{}:{:02}:{:02}:{:02}", if neg { "-" } else { "" }, secs / 3600, (secs / 60) % 60, secs % 60, ff);
    Ok(s(&txt))
}

fn ae_time_to_feet(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let (fps, start) = it.cx.comp_rate_start();
    let t = match arg(&a, 0) {
        V::Undef => it.cx.time + start,
        v => it.to_number(&v)?,
    };
    let frames = (t * fps + 1e-6).floor() as i64;
    Ok(s(&format!("{}+{:02}", frames / 16, frames % 16)))
}

fn ae_posterize(it: &mut Interp, _: &V, a: Vec<V>) -> R {
    let fps = n0(it, &a, 0)?;
    if fps > 0.0 && fps.is_finite() {
        it.cx.time = (it.cx.time * fps + 1e-9).floor() / fps;
    }
    Ok(V::Num(it.cx.time))
}
