struct Comp {
    inv0: vec4<f32>,
    inv1: vec4<f32>,
    inv2: vec4<f32>,
    geo: vec4<f32>,
    size: vec4<u32>,
    reg: vec4<u32>,
    tile: vec4<i32>,
    flags: vec4<u32>,
};

@group(0) @binding(0) var<storage, read_write> dst: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> cu: Comp;

fn src_get(x: i32, y: i32) -> vec4<f32> {
    let sw = i32(cu.size.x);
    let sh = i32(cu.size.y);
    if (x < 0 || y < 0 || x >= sw || y >= sh) {
        return vec4<f32>(0.0);
    }
    return src[u32(y * sw + x)];
}

fn src_clamped(x: i32, y: i32) -> vec4<f32> {
    let sw = i32(cu.size.x);
    let sh = i32(cu.size.y);
    let cx = clamp(x, 0, sw - 1);
    let cy = clamp(y, 0, sh - 1);
    return src[u32(cy * sw + cx)];
}

fn lum3(c: vec3<f32>) -> f32 {
    return 0.3 * c.x + 0.59 * c.y + 0.11 * c.z;
}

fn clip_color(c: vec3<f32>) -> vec3<f32> {
    let l = lum3(c);
    let n = min(min(c.x, c.y), c.z);
    let x = max(max(c.x, c.y), c.z);
    var o = c;
    if (n < 0.0) {
        let d = l - n;
        if (d > 1e-6) {
            o = vec3<f32>(l) + (o - vec3<f32>(l)) * l / d;
        }
    }
    if (x > 1.0) {
        let d = x - l;
        if (d > 1e-6) {
            o = vec3<f32>(l) + (o - vec3<f32>(l)) * (1.0 - l) / d;
        }
    }
    return o;
}

fn set_lum(c: vec3<f32>, l: f32) -> vec3<f32> {
    let d = l - lum3(c);
    return clip_color(c + vec3<f32>(d));
}

fn sat3(c: vec3<f32>) -> f32 {
    return max(max(c.x, c.y), c.z) - min(min(c.x, c.y), c.z);
}

fn set_sat(c: vec3<f32>, s: f32) -> vec3<f32> {
    var ix = array<u32, 3>(0u, 1u, 2u);
    var v = array<f32, 3>(c.x, c.y, c.z);
    if (v[ix[1]] < v[ix[0]]) { let t = ix[0]; ix[0] = ix[1]; ix[1] = t; }
    if (v[ix[2]] < v[ix[1]]) { let t = ix[1]; ix[1] = ix[2]; ix[2] = t; }
    if (v[ix[1]] < v[ix[0]]) { let t = ix[0]; ix[0] = ix[1]; ix[1] = t; }
    var o = array<f32, 3>(0.0, 0.0, 0.0);
    let mn = ix[0];
    let md = ix[1];
    let mx = ix[2];
    if (v[mx] > v[mn]) {
        o[md] = (v[md] - v[mn]) * s / (v[mx] - v[mn]);
        o[mx] = s;
    }
    o[mn] = 0.0;
    return vec3<f32>(o[0], o[1], o[2]);
}

fn soft_d(x: f32) -> f32 {
    if (x <= 0.25) {
        return ((16.0 * x - 12.0) * x + 4.0) * x;
    }
    return sqrt(x);
}

fn burn(b: f32, s: f32) -> f32 {
    if (b >= 1.0) { return 1.0; }
    if (s <= 0.0) { return 0.0; }
    return 1.0 - min((1.0 - b) / s, 1.0);
}

fn dodge(b: f32, s: f32) -> f32 {
    if (b <= 0.0) { return 0.0; }
    if (s >= 1.0) { return 1.0; }
    return min(b / (1.0 - s), 1.0);
}

fn separable(mode: u32, b: f32, s: f32) -> f32 {
    switch mode {
        case 3u: { return min(b, s); }
        case 4u: { return b * s; }
        case 5u, 6u: { return burn(b, s); }
        case 7u: { return max(b + s - 1.0, 0.0); }
        case 9u, 14u: { return min(b + s, 1.0); }
        case 10u: { return max(b, s); }
        case 11u: { return b + s - b * s; }
        case 12u, 13u: { return dodge(b, s); }
        case 16u: {
            if (b <= 0.5) { return 2.0 * b * s; }
            return 1.0 - 2.0 * (1.0 - b) * (1.0 - s);
        }
        case 17u: {
            if (s <= 0.5) { return b - (1.0 - 2.0 * s) * b * (1.0 - b); }
            return b + (2.0 * s - 1.0) * (soft_d(b) - b);
        }
        case 18u: {
            if (s <= 0.5) { return 2.0 * b * s; }
            let s2 = 2.0 * s - 1.0;
            return b + s2 - b * s2;
        }
        case 19u: { return clamp(b + 2.0 * s - 1.0, 0.0, 1.0); }
        case 20u: {
            if (s <= 0.5) { return burn(b, 2.0 * s); }
            return dodge(b, 2.0 * (s - 0.5));
        }
        case 21u: {
            if (s <= 0.5) { return min(b, 2.0 * s); }
            return max(b, 2.0 * s - 1.0);
        }
        case 22u: {
            if (b + s >= 1.0) { return 1.0; }
            return 0.0;
        }
        case 23u, 24u: { return abs(b - s); }
        case 25u: { return b + s - 2.0 * b * s; }
        case 26u: { return max(b - s, 0.0); }
        case 27u: {
            if (s <= 0.0) { return 1.0; }
            return min(b / s, 1.0);
        }
        default: { return s; }
    }
}

fn blend_color(mode: u32, cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    switch mode {
        case 28u: { return set_lum(set_sat(cs, sat3(cb)), lum3(cb)); }
        case 29u: { return set_lum(set_sat(cb, sat3(cs)), lum3(cb)); }
        case 30u: { return set_lum(cs, lum3(cb)); }
        case 31u: { return set_lum(cb, lum3(cs)); }
        case 8u: {
            if (lum3(cs) < lum3(cb)) { return cs; }
            return cb;
        }
        case 15u: {
            if (lum3(cs) > lum3(cb)) { return cs; }
            return cb;
        }
        default: {
            return vec3<f32>(separable(mode, cb.x, cs.x), separable(mode, cb.y, cs.y), separable(mode, cb.z, cs.z));
        }
    }
}

fn blend_px(mode: u32, d: vec4<f32>, s: vec4<f32>) -> vec4<f32> {
    let sa = min(s.w, 1.0);
    let da = min(d.w, 1.0);
    switch mode {
        case 0u: { return vec4<f32>(s.xyz + d.xyz * (1.0 - sa), sa + da * (1.0 - sa)); }
        case 32u: { return vec4<f32>(d.xyz * sa, da * sa); }
        case 33u: {
            let l = lum3(s.xyz);
            return vec4<f32>(d.xyz * l, da * l);
        }
        case 34u: { return vec4<f32>(d.xyz * (1.0 - sa), da * (1.0 - sa)); }
        case 35u: {
            let l = lum3(s.xyz);
            return vec4<f32>(d.xyz * (1.0 - l), da * (1.0 - l));
        }
        case 36u: { return vec4<f32>(s.xyz + d.xyz * (1.0 - sa), min(sa + da, 1.0)); }
        case 37u: { return vec4<f32>(min(s.xyz + d.xyz, vec3<f32>(1.0)), sa + da * (1.0 - sa)); }
        default: {
            if ((mode == 9u || mode == 14u) && da >= 0.999) {
                return vec4<f32>(min(s.xyz + d.xyz, vec3<f32>(1.0)), 1.0);
            }
            var cs = vec3<f32>(0.0);
            var cb = vec3<f32>(0.0);
            if (sa > 1e-6) { cs = s.xyz / sa; }
            if (da > 1e-6) { cb = d.xyz / da; }
            let b = blend_color(mode, cb, cs);
            let o = (1.0 - sa) * d.xyz + (1.0 - da) * s.xyz + sa * da * b;
            return vec4<f32>(o, sa + da * (1.0 - sa));
        }
    }
}

fn quantize(v: vec4<f32>, q: f32) -> vec4<f32> {
    if (q <= 0.0) {
        return v;
    }
    return floor(clamp(v, vec4<f32>(0.0), vec4<f32>(1.0)) * q + vec4<f32>(0.5)) / q;
}

@compute @workgroup_size(16, 16)
fn composite(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cu.reg.z || gid.y >= cu.reg.w) {
        return;
    }
    let x = i32(cu.reg.x + gid.x);
    let y = i32(cu.reg.y + gid.y);
    let cw = i32(cu.size.z);
    let mode = cu.flags.x;
    let stencil = mode == 32u || mode == 33u;
    var s = vec4<f32>(0.0);
    if (x >= cu.tile.x && y >= cu.tile.y && x < cu.tile.z && y < cu.tile.w) {
        let px = f32(x) + 0.5;
        let py = f32(y) + 0.5;
        var sx = cu.inv0.x * px + cu.inv0.y * py + cu.inv0.z;
        var sy = cu.inv1.x * px + cu.inv1.y * py + cu.inv1.z;
        var ok = true;
        if (cu.flags.w == 0u) {
            let w = cu.inv2.x * px + cu.inv2.y * py + cu.inv2.z;
            if (w <= 0.0) {
                ok = false;
            } else if (w != 1.0 && abs(w) > 1e-12) {
                sx = sx / w;
                sy = sy / w;
            }
        }
        sx = sx * cu.geo.x;
        sy = sy * cu.geo.x;
        let sw = f32(cu.size.x);
        let sh = f32(cu.size.y);
        if (ok && !(sx < -1.0 || sy < -1.0 || sx > sw + 1.0 || sy > sh + 1.0)) {
            if (cu.flags.z == 1u) {
                if (!(sx < 0.0 || sy < 0.0 || sx >= sw || sy >= sh)) {
                    s = src_clamped(i32(floor(sx)), i32(floor(sy)));
                }
            } else {
                let fx = sx - 0.5;
                let fy = sy - 0.5;
                let x0 = floor(fx);
                let y0 = floor(fy);
                let tx = fx - x0;
                let ty = fy - y0;
                let xi = i32(x0);
                let yi = i32(y0);
                let a = src_get(xi, yi);
                let b = src_get(xi + 1, yi);
                let c = src_get(xi, yi + 1);
                let d = src_get(xi + 1, yi + 1);
                let top = a + (b - a) * tx;
                let bot = c + (d - c) * tx;
                s = top + (bot - top) * ty;
            }
        }
    } else if (!stencil) {
        return;
    }
    if (s.w <= 0.0 && !stencil) {
        return;
    }
    let idx = u32(y * cw + x);
    let d = dst[idx];
    let sv = s * cu.geo.y;
    var o = blend_px(mode, d, sv);
    if (cu.flags.y == 1u) {
        var k = 0.0;
        if (o.w > 1e-6) {
            k = d.w / o.w;
        }
        o = vec4<f32>(o.xyz * k, d.w);
    }
    dst[idx] = quantize(o, cu.geo.z);
}

struct Box {
    size: vec4<u32>,
};

@group(0) @binding(0) var<storage, read_write> bout: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> bin: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> bu: Box;

fn box_at(x: i32, y: i32) -> vec4<f32> {
    let w = i32(bu.size.x);
    let h = i32(bu.size.y);
    if (x < 0 || y < 0 || x >= w || y >= h) {
        if (bu.size.w == 1u) {
            return bin[u32(clamp(y, 0, h - 1) * w + clamp(x, 0, w - 1))];
        }
        return vec4<f32>(0.0);
    }
    return bin[u32(y * w + x)];
}

@compute @workgroup_size(16, 16)
fn box_h(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= bu.size.x || gid.y >= bu.size.y) {
        return;
    }
    let r = i32(bu.size.z);
    var acc = vec4<f32>(0.0);
    for (var k = -r; k <= r; k = k + 1) {
        acc = acc + box_at(i32(gid.x) + k, i32(gid.y));
    }
    bout[gid.y * bu.size.x + gid.x] = acc / f32(2 * r + 1);
}

@compute @workgroup_size(16, 16)
fn box_v(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= bu.size.x || gid.y >= bu.size.y) {
        return;
    }
    let r = i32(bu.size.z);
    var acc = vec4<f32>(0.0);
    for (var k = -r; k <= r; k = k + 1) {
        acc = acc + box_at(i32(gid.x), i32(gid.y) + k);
    }
    bout[gid.y * bu.size.x + gid.x] = acc / f32(2 * r + 1);
}

@compute @workgroup_size(16, 16)
fn half(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w2 = max(bu.size.x / 2u, 1u);
    let h2 = max(bu.size.y / 2u, 1u);
    if (gid.x >= w2 || gid.y >= h2) {
        return;
    }
    let w = i32(bu.size.x);
    let h = i32(bu.size.y);
    var acc = vec4<f32>(0.0);
    for (var dy = 0; dy < 2; dy = dy + 1) {
        for (var dx = 0; dx < 2; dx = dx + 1) {
            let sx = clamp(i32(gid.x) * 2 + dx, 0, w - 1);
            let sy = clamp(i32(gid.y) * 2 + dy, 0, h - 1);
            acc = acc + bin[u32(sy * w + sx)];
        }
    }
    bout[gid.y * w2 + gid.x] = acc * 0.25;
}

struct Prog {
    size: vec4<u32>,
};

@group(0) @binding(0) var<storage, read_write> pbuf: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> prog: array<f32>;
@group(0) @binding(2) var<uniform> pu: Prog;
@group(0) @binding(3) var<storage, read> luts: array<f32>;

fn negw_pow(x: f32, e: f32, neg: u32) -> f32 {
    if (neg == 1u) {
        return sign(x) * pow(abs(x), e);
    }
    if (neg == 2u && x < 0.0) {
        return x;
    }
    if (neg == 0u || neg == 3u) {
        let v = max(x, 0.0);
        if (v == 0.0) {
            if (e == 0.0) { return 1.0; }
            return 0.0;
        }
        return pow(v, e);
    }
    return pow(x, e);
}

fn mon_slope(g: f32, o: f32) -> f32 {
    return ((g - 1.0) / o) * pow((o * g) / ((g - 1.0) * (1.0 + o)), g);
}

fn mon_dec(x: f32, g: f32, o: f32) -> f32 {
    if (o <= 0.0 || g <= 1.0) {
        return pow(max(x, 0.0), g);
    }
    if (x > o / (g - 1.0)) {
        return pow((x + o) / (1.0 + o), g);
    }
    return x * mon_slope(g, o);
}

fn mon_enc(y: f32, g: f32, o: f32) -> f32 {
    if (o <= 0.0 || g <= 1.0) {
        return pow(max(y, 0.0), 1.0 / g);
    }
    let s = mon_slope(g, o);
    let brk = (o / (g - 1.0)) * s;
    if (y > brk) {
        return (1.0 + o) * pow(y, 1.0 / g) - o;
    }
    return y / s;
}

fn mon(x: f32, g: f32, o: f32, neg: u32, enc: bool) -> f32 {
    var v = x;
    if (neg == 1u) {
        v = abs(x);
    } else if (neg == 2u && x < 0.0) {
        return x;
    } else if (neg == 0u) {
        v = max(x, 0.0);
    }
    var r = 0.0;
    if (enc) {
        r = mon_enc(v, g, o);
    } else {
        r = mon_dec(v, g, o);
    }
    if (neg == 1u) {
        return sign(x) * r;
    }
    return r;
}

fn logb(x: f32, b: f32) -> f32 {
    return log(max(x, 1.17549435e-38)) / log(b);
}

fn lut_at(off: u32, i: u32) -> vec3<f32> {
    let k = off + i * 3u;
    return vec3<f32>(luts[k], luts[k + 1u], luts[k + 2u]);
}

fn lut1(c: vec3<f32>, off: u32, n: u32, mn: vec3<f32>, mx: vec3<f32>, interp: u32) -> vec3<f32> {
    var o = vec3<f32>(0.0);
    for (var k = 0; k < 3; k = k + 1) {
        let span = mx[k] - mn[k];
        var t = 0.0;
        if (abs(span) >= 1e-12) {
            t = clamp((c[k] - mn[k]) / span, 0.0, 1.0) * f32(n - 1u);
        }
        if (interp == 0u) {
            o[k] = lut_at(off, min(u32(round(t)), n - 1u))[k];
        } else {
            let i = min(u32(floor(t)), n - 2u);
            let f = t - f32(i);
            let a = lut_at(off, i)[k];
            let b = lut_at(off, min(i + 1u, n - 1u))[k];
            o[k] = a + (b - a) * f;
        }
    }
    return o;
}

fn l3(off: u32, n: u32, r: u32, g: u32, b: u32) -> vec3<f32> {
    return lut_at(off, (b * n + g) * n + r);
}

fn lut3(c: vec3<f32>, off: u32, n: u32, mn: vec3<f32>, mx: vec3<f32>, interp: u32) -> vec3<f32> {
    var idx = vec3<u32>(0u);
    var f = vec3<f32>(0.0);
    for (var k = 0; k < 3; k = k + 1) {
        let span = mx[k] - mn[k];
        var t = 0.0;
        if (abs(span) >= 1e-12) {
            t = clamp((c[k] - mn[k]) / span, 0.0, 1.0) * f32(n - 1u);
        }
        if (interp == 0u) {
            idx[k] = min(u32(round(t)), n - 1u);
        } else {
            idx[k] = min(u32(floor(t)), n - 2u);
            f[k] = t - f32(idx[k]);
        }
    }
    if (interp == 0u) {
        return l3(off, n, idx.x, idx.y, idx.z);
    }
    let r = idx.x;
    let g = idx.y;
    let b = idx.z;
    let fx = f.x;
    let fy = f.y;
    let fz = f.z;
    let c000 = l3(off, n, r, g, b);
    let c111 = l3(off, n, r + 1u, g + 1u, b + 1u);
    if (interp >= 2u) {
        if (fx > fy) {
            if (fy > fz) {
                return (1.0 - fx) * c000 + (fx - fy) * l3(off, n, r + 1u, g, b) + (fy - fz) * l3(off, n, r + 1u, g + 1u, b) + fz * c111;
            } else if (fx > fz) {
                return (1.0 - fx) * c000 + (fx - fz) * l3(off, n, r + 1u, g, b) + (fz - fy) * l3(off, n, r + 1u, g, b + 1u) + fy * c111;
            }
            return (1.0 - fz) * c000 + (fz - fx) * l3(off, n, r, g, b + 1u) + (fx - fy) * l3(off, n, r + 1u, g, b + 1u) + fy * c111;
        }
        if (fz > fy) {
            return (1.0 - fz) * c000 + (fz - fy) * l3(off, n, r, g, b + 1u) + (fy - fx) * l3(off, n, r, g + 1u, b + 1u) + fx * c111;
        } else if (fz > fx) {
            return (1.0 - fy) * c000 + (fy - fz) * l3(off, n, r, g + 1u, b) + (fz - fx) * l3(off, n, r, g + 1u, b + 1u) + fx * c111;
        }
        return (1.0 - fy) * c000 + (fy - fx) * l3(off, n, r, g + 1u, b) + (fx - fz) * l3(off, n, r + 1u, g + 1u, b) + fz * c111;
    }
    var o = vec3<f32>(0.0);
    for (var dz = 0u; dz < 2u; dz = dz + 1u) {
        for (var dy = 0u; dy < 2u; dy = dy + 1u) {
            for (var dx = 0u; dx < 2u; dx = dx + 1u) {
                var w = 1.0;
                if (dx == 1u) { w = w * fx; } else { w = w * (1.0 - fx); }
                if (dy == 1u) { w = w * fy; } else { w = w * (1.0 - fy); }
                if (dz == 1u) { w = w * fz; } else { w = w * (1.0 - fz); }
                o = o + l3(off, n, r + dx, g + dy, b + dz) * w;
            }
        }
    }
    return o;
}

fn pq_enc(nits: f32) -> f32 {
    let m1 = 0.1593017578125;
    let m2 = 78.84375;
    let c1 = 0.8359375;
    let c2 = 18.8515625;
    let c3 = 18.6875;
    let l = pow(abs(nits) / 10000.0, m1);
    return sign(nits) * pow((c1 + c2 * l) / (1.0 + c3 * l), m2);
}

fn pq_dec(v: f32) -> f32 {
    let m1 = 0.1593017578125;
    let m2 = 78.84375;
    let c1 = 0.8359375;
    let c2 = 18.8515625;
    let c3 = 18.6875;
    let p = pow(min(abs(v), 1.0), 1.0 / m2);
    return sign(v) * pow(max(p - c1, 0.0) / (c2 - c3 * p), 1.0 / m1) * 10000.0;
}

fn hlg(e: f32) -> f32 {
    let a = 0.17883277;
    let b = 0.28466892;
    let c = 0.55991073;
    let v = max(e, 0.0);
    if (v <= 1.0 / 12.0) {
        return sqrt(3.0 * v);
    }
    return a * log(12.0 * v - b) + c;
}

fn hlg_inv(x: f32) -> f32 {
    let a = 0.17883277;
    let b = 0.28466892;
    let c = 0.55991073;
    let v = max(x, 0.0);
    if (v <= 0.5) {
        return v * v / 3.0;
    }
    return (exp((v - c) / a) + b) / 12.0;
}

fn fixed1(kind: u32, inv: bool, x: f32) -> f32 {
    switch kind {
        case 1u: {
            if (inv) { return pq_dec(x) / 100.0; }
            return pq_enc(x * 100.0);
        }
        case 2u: {
            if (inv) { return sign(x) * hlg_inv(abs(x)); }
            return sign(x) * hlg(abs(x));
        }
        case 3u: {
            if (inv) {
                if (x <= 0.0078125) { return 10.5402377416545 * x + 0.0729055341958355; }
                return (log2(x) + 9.72) / 17.52;
            }
            if (x <= 0.155251141552511) { return (x - 0.0729055341958355) / 10.5402377416545; }
            return exp2(x * 17.52 - 9.72);
        }
        case 4u: {
            if (inv) {
                if (x <= 0.0) { return (-16.0 + 9.72) / 17.52; }
                if (x < exp2(-15.0)) { return (log2(exp2(-16.0) + x * 0.5) + 9.72) / 17.52; }
                return (log2(x) + 9.72) / 17.52;
            }
            if (x < (9.72 - 15.0) / 17.52) { return (exp2(x * 17.52 - 9.72) - exp2(-16.0)) * 2.0; }
            return exp2(x * 17.52 - 9.72);
        }
        default: { return x; }
    }
}

fn op_lin(x: f32, k: u32, i: u32, inv: bool) -> f32 {
    let base = prog[i + 1u];
    let ls = prog[i + 2u + k];
    let lo = prog[i + 5u + k];
    let ns = prog[i + 8u + k];
    let no = prog[i + 11u + k];
    let has_brk = prog[i + 14u] > 0.5;
    let b = prog[i + 15u + k];
    let has_lin = prog[i + 18u] > 0.5;
    var slope = 0.0;
    var off = 0.0;
    var lb = 0.0;
    if (has_brk) {
        lb = ls * logb(ns * b + no, base) + lo;
        if (has_lin) {
            slope = prog[i + 19u + k];
        } else {
            slope = ls * ns / ((ns * b + no) * log(base));
        }
        off = lb - slope * b;
    }
    if (!inv) {
        if (has_brk && x <= b) {
            return slope * x + off;
        }
        return ls * logb(ns * x + no, base) + lo;
    }
    if (has_brk && x <= lb) {
        return (x - off) / slope;
    }
    return (pow(base, (x - lo) / ls) - no) / ns;
}

fn cdl(c: vec3<f32>, i: u32) -> vec3<f32> {
    let s = vec3<f32>(prog[i + 1u], prog[i + 2u], prog[i + 3u]);
    let o = vec3<f32>(prog[i + 4u], prog[i + 5u], prog[i + 6u]);
    let p = vec3<f32>(prog[i + 7u], prog[i + 8u], prog[i + 9u]);
    let sat = prog[i + 10u];
    let clampit = prog[i + 11u] > 0.5;
    let inv = prog[i + 12u] > 0.5;
    let lw = vec3<f32>(0.2126, 0.7152, 0.0722);
    if (!inv) {
        var x = c * s + o;
        if (clampit) { x = clamp(x, vec3<f32>(0.0), vec3<f32>(1.0)); }
        for (var k = 0; k < 3; k = k + 1) {
            if (x[k] > 0.0 || clampit) {
                x[k] = pow(max(x[k], 0.0), p[k]);
            }
        }
        let y = dot(lw, x);
        var r = vec3<f32>(y) + sat * (x - vec3<f32>(y));
        if (clampit) { r = clamp(r, vec3<f32>(0.0), vec3<f32>(1.0)); }
        return r;
    }
    var x = c;
    if (clampit) { x = clamp(x, vec3<f32>(0.0), vec3<f32>(1.0)); }
    let y = dot(lw, x);
    var k2 = 0.0;
    if (abs(sat) > 1e-12) { k2 = 1.0 / sat; }
    x = vec3<f32>(y) + k2 * (x - vec3<f32>(y));
    if (clampit) { x = clamp(x, vec3<f32>(0.0), vec3<f32>(1.0)); }
    var out = vec3<f32>(0.0);
    for (var k = 0; k < 3; k = k + 1) {
        var e = 1.0;
        if (abs(p[k]) > 1e-12) { e = 1.0 / p[k]; }
        var v = x[k];
        if (v > 0.0 || clampit) { v = pow(max(v, 0.0), e); }
        if (abs(s[k]) > 1e-12) { out[k] = (v - o[k]) / s[k]; }
    }
    return out;
}

@compute @workgroup_size(16, 16)
fn pointwise(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= pu.size.x || gid.y >= pu.size.y) {
        return;
    }
    let idx = gid.y * pu.size.x + gid.x;
    var p = pbuf[idx];
    var c = vec3<f32>(0.0);
    var skip = true;
    var i = 0u;
    let n = pu.size.z;
    loop {
        if (i >= n) {
            break;
        }
        let op = u32(prog[i]);
        switch op {
            case 1u: {
                skip = p.w <= 1e-6;
                if (!skip) { c = p.xyz / p.w; }
                i = i + 1u;
            }
            case 2u: {
                if (!skip) { p = vec4<f32>(c * p.w, p.w); }
                i = i + 1u;
            }
            case 3u: {
                p = quantize(p, prog[i + 1u]);
                i = i + 2u;
            }
            case 10u: {
                if (!skip) {
                    let r0 = vec3<f32>(prog[i + 1u], prog[i + 2u], prog[i + 3u]);
                    let r1 = vec3<f32>(prog[i + 4u], prog[i + 5u], prog[i + 6u]);
                    let r2 = vec3<f32>(prog[i + 7u], prog[i + 8u], prog[i + 9u]);
                    c = vec3<f32>(dot(r0, c), dot(r1, c), dot(r2, c)) + vec3<f32>(prog[i + 10u], prog[i + 11u], prog[i + 12u]);
                }
                i = i + 13u;
            }
            case 11u: {
                if (!skip) {
                    let neg = u32(prog[i + 4u]);
                    c = vec3<f32>(negw_pow(c.x, prog[i + 1u], neg), negw_pow(c.y, prog[i + 2u], neg), negw_pow(c.z, prog[i + 3u], neg));
                }
                i = i + 5u;
            }
            case 12u: {
                if (!skip) {
                    let neg = u32(prog[i + 7u]);
                    let enc = prog[i + 8u] > 0.5;
                    c = vec3<f32>(mon(c.x, prog[i + 1u], prog[i + 4u], neg, enc), mon(c.y, prog[i + 2u], prog[i + 5u], neg, enc), mon(c.z, prog[i + 3u], prog[i + 6u], neg, enc));
                }
                i = i + 9u;
            }
            case 13u: {
                if (!skip) {
                    let b = prog[i + 1u];
                    if (prog[i + 2u] > 0.5) {
                        c = vec3<f32>(pow(b, c.x), pow(b, c.y), pow(b, c.z));
                    } else {
                        c = vec3<f32>(logb(c.x, b), logb(c.y, b), logb(c.z, b));
                    }
                }
                i = i + 3u;
            }
            case 14u: {
                if (!skip) {
                    let inv = prog[i + 22u] > 0.5;
                    c = vec3<f32>(op_lin(c.x, 0u, i, inv), op_lin(c.y, 1u, i, inv), op_lin(c.z, 2u, i, inv));
                }
                i = i + 23u;
            }
            case 15u: {
                if (!skip) { c = cdl(c, i); }
                i = i + 13u;
            }
            case 16u: {
                if (!skip) {
                    c = c * prog[i + 1u] + vec3<f32>(prog[i + 2u]);
                    if (prog[i + 3u] > 0.5) { c = max(c, vec3<f32>(prog[i + 4u])); }
                    if (prog[i + 5u] > 0.5) { c = min(c, vec3<f32>(prog[i + 6u])); }
                }
                i = i + 7u;
            }
            case 17u, 18u: {
                if (!skip) {
                    let off = u32(prog[i + 1u]);
                    let ln = u32(prog[i + 2u]);
                    let mn = vec3<f32>(prog[i + 3u], prog[i + 4u], prog[i + 5u]);
                    let mx = vec3<f32>(prog[i + 6u], prog[i + 7u], prog[i + 8u]);
                    let interp = u32(prog[i + 9u]);
                    if (op == 17u) {
                        c = lut1(c, off, ln, mn, mx, interp);
                    } else {
                        c = lut3(c, off, ln, mn, mx, interp);
                    }
                }
                i = i + 10u;
            }
            case 19u: {
                if (!skip) {
                    let kind = u32(prog[i + 1u]);
                    let inv = prog[i + 2u] > 0.5;
                    let prm = prog[i + 3u];
                    if (kind == 5u) {
                        var g = prm;
                        if (inv) { g = 1.0 / prm; }
                        let y = max(0.2627 * c.x + 0.6780 * c.y + 0.0593 * c.z, 1e-4);
                        c = c * pow(y, g - 1.0);
                    } else if (kind == 6u) {
                        let k = 0.8;
                        let m = max(max(c.x, c.y), c.z);
                        if (m > k) {
                            let span = 4.0 * (1.0 - k);
                            var t = 0.0;
                            if (inv) {
                                if (m < 1.0 - 1e-9) {
                                    let u = (m - k) / (1.0 - k);
                                    t = k + span * u / (1.0 - u);
                                    c = c * (t / m);
                                }
                            } else {
                                let xx = (m - k) / span;
                                t = k + (1.0 - k) * xx / (1.0 + xx);
                                c = c * (t / m);
                            }
                        }
                    } else {
                        c = vec3<f32>(fixed1(kind, inv, c.x), fixed1(kind, inv, c.y), fixed1(kind, inv, c.z));
                    }
                }
                i = i + 4u;
            }
            case 30u: {
                if (!skip) {
                    let k = prog[i + 1u];
                    let o = prog[i + 2u];
                    let g = prog[i + 3u];
                    c = vec3<f32>(negw_pow(max(c.x * k + o, 0.0), g, 0u), negw_pow(max(c.y * k + o, 0.0), g, 0u), negw_pow(max(c.z * k + o, 0.0), g, 0u));
                }
                i = i + 4u;
            }
            case 31u: {
                if (!skip) {
                    let b = prog[i + 1u];
                    let k = prog[i + 2u];
                    c = clamp((c + vec3<f32>(b - 0.5)) * k + vec3<f32>(0.5), vec3<f32>(0.0), vec3<f32>(1.0));
                }
                i = i + 3u;
            }
            case 32u: {
                if (!skip) {
                    let bl = vec3<f32>(prog[i + 1u], prog[i + 2u], prog[i + 3u]);
                    let wh = vec3<f32>(prog[i + 4u], prog[i + 5u], prog[i + 6u]);
                    let k = prog[i + 7u];
                    let l = 0.2126 * c.x + 0.7152 * c.y + 0.0722 * c.z;
                    let t = bl + (wh - bl) * l;
                    c = c + (t - c) * k;
                }
                i = i + 8u;
            }
            case 33u: {
                let ch = u32(prog[i + 1u]);
                let blend = prog[i + 2u];
                let orig = p;
                if (ch == 4u) {
                    let a = p.w;
                    let na = 1.0 - a;
                    var k = 0.0;
                    if (a > 1e-6) { k = na / a; }
                    p = vec4<f32>(p.xyz * k, na);
                } else if (p.w > 1e-6) {
                    var v = p.xyz / p.w;
                    if (ch == 1u) {
                        v.x = 1.0 - v.x;
                    } else if (ch == 2u) {
                        v.y = 1.0 - v.y;
                    } else if (ch == 3u) {
                        v.z = 1.0 - v.z;
                    } else if (ch == 5u) {
                        let l = 0.2126 * v.x + 0.7152 * v.y + 0.0722 * v.z;
                        v = clamp(v + vec3<f32>(1.0 - 2.0 * l), vec3<f32>(0.0), vec3<f32>(1.0));
                    } else {
                        v = vec3<f32>(1.0) - v;
                    }
                    p = vec4<f32>(v * p.w, p.w);
                }
                if (blend > 0.0) {
                    p = p + (orig - p) * min(blend, 1.0);
                }
                i = i + 3u;
            }
            default: {
                i = n;
            }
        }
    }
    pbuf[idx] = p;
}
