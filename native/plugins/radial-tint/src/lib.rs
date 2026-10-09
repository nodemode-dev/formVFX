use std::alloc::{Layout, alloc};

static DESCRIPTION: &str = r#"{
  "id": "examples.radialTint",
  "name": "Radial Tint",
  "category": "Plug-ins",
  "vendor": "formvfx examples",
  "version": "1.0.0",
  "params": [
    {"id": "amount", "name": "Amount", "type": "slider", "min": 0, "max": 100, "default": 50},
    {"id": "color", "name": "Color", "type": "color", "default": [1, 0.5, 0]},
    {"id": "center", "name": "Center", "type": "point", "default": [0, 0]},
    {"id": "radius", "name": "Radius", "type": "slider", "min": 1, "max": 4000, "default": 400},
    {"id": "invert", "name": "Invert Falloff", "type": "checkbox", "default": false}
  ]
}"#;

#[unsafe(no_mangle)]
pub extern "C" fn fvx_abi() -> i32 {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn fvx_describe() -> i64 {
    ((DESCRIPTION.as_ptr() as i64) << 32) | DESCRIPTION.len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn fvx_alloc(size: i32) -> i32 {
    match Layout::from_size_align(size.max(1) as usize, 16) {
        Ok(l) => unsafe { alloc(l) as i32 },
        Err(_) => 0,
    }
}

fn find<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let tag = format!("\"{key}\":");
    let at = json.find(&tag)? + tag.len();
    Some(json[at..].trim_start())
}

fn number(json: &str, key: &str) -> Option<f32> {
    let s = find(json, key)?;
    if s.starts_with("true") {
        return Some(1.0);
    }
    if s.starts_with("false") {
        return Some(0.0);
    }
    let end = s.find(|c: char| !(c.is_ascii_digit() || "+-.eE".contains(c))).unwrap_or(s.len());
    s[..end].parse().ok()
}

fn numbers(json: &str, key: &str) -> Vec<f32> {
    let Some(s) = find(json, key).and_then(|s| s.strip_prefix('[')) else { return vec![] };
    let end = s.find(']').unwrap_or(s.len());
    s[..end].split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

#[unsafe(no_mangle)]
pub extern "C" fn fvx_render(px: i32, w: i32, h: i32, params: i32, len: i32, _time: f64) -> i32 {
    let (w, h) = (w.max(0) as usize, h.max(0) as usize);
    let pixels = unsafe { std::slice::from_raw_parts_mut(px as *mut f32, w * h * 4) };
    let text = unsafe { std::slice::from_raw_parts(params as *const u8, len.max(0) as usize) };
    let Ok(json) = std::str::from_utf8(text) else { return 1 };
    let Some(values) = json.find("\"values\"").map(|i| &json[i..]) else { return 2 };
    let scale = number(json, "scale").unwrap_or(1.0);
    let origin = numbers(json, "origin");
    let (ox, oy) = (origin.first().copied().unwrap_or(0.0), origin.get(1).copied().unwrap_or(0.0));
    let amount = number(values, "amount").unwrap_or(50.0) / 100.0;
    let color = numbers(values, "color");
    let tint = [color.first().copied().unwrap_or(1.0), color.get(1).copied().unwrap_or(0.5), color.get(2).copied().unwrap_or(0.0)];
    let center = numbers(values, "center");
    let (cx, cy) = ((center.first().copied().unwrap_or(0.0) - ox) * scale, (center.get(1).copied().unwrap_or(0.0) - oy) * scale);
    let radius = (number(values, "radius").unwrap_or(400.0) * scale).max(1e-3);
    let invert = number(values, "invert").unwrap_or(0.0) > 0.5;
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt() / radius;
            let fall = (1.0 - d).clamp(0.0, 1.0);
            let k = amount * if invert { 1.0 - fall } else { fall };
            let p = &mut pixels[(y * w + x) * 4..][..4];
            let a = p[3];
            for c in 0..3 {
                p[c] += (tint[c] * a - p[c]) * k;
            }
        }
    }
    0
}
