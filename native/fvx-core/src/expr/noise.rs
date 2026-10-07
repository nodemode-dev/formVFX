pub fn mix64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

pub fn hash(parts: &[u64]) -> u64 {
    let mut h = 0x9E37_79B9_7F4A_7C15u64;
    for p in parts {
        h = mix64(h ^ p.wrapping_mul(0xD6E8_FEB8_6659_FD93));
    }
    h
}

pub fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn grad(h: u64, x: f64, y: f64, z: f64) -> f64 {
    match h % 12 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        _ => -y - z,
    }
}

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

pub fn perlin3(x: f64, y: f64, z: f64, seed: u64) -> f64 {
    let (xi, yi, zi) = (x.floor(), y.floor(), z.floor());
    let (xf, yf, zf) = (x - xi, y - yi, z - zi);
    let (u, v, w) = (fade(xf), fade(yf), fade(zf));
    let c = |dx: i64, dy: i64, dz: i64| {
        let h = hash(&[seed, (xi as i64 + dx) as u64, (yi as i64 + dy) as u64, (zi as i64 + dz) as u64]);
        grad(h, xf - dx as f64, yf - dy as f64, zf - dz as f64)
    };
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), u);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), u);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), u);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), u);
    let y0 = lerp(x00, x10, v);
    let y1 = lerp(x01, x11, v);
    (lerp(y0, y1, w) * 0.97).clamp(-1.0, 1.0)
}

pub fn wiggle1(t: f64, freq: f64, octaves: f64, amp_mult: f64, seed: u64) -> f64 {
    let full = octaves.floor().max(0.0) as u32;
    let frac = octaves - full as f64;
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut f = freq;
    for o in 0..=full.min(16) {
        let weight = if o < full { 1.0 } else { frac };
        if weight <= 0.0 {
            break;
        }
        sum += weight * amp * perlin3(t * f, 0.5 + o as f64 * 7.13, 0.25, seed);
        amp *= amp_mult;
        f *= 2.0;
    }
    sum
}
