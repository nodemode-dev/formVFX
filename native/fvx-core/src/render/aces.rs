use super::color::{self, AP0, AP1, M3};

pub const HALF_MIN: f64 = 5.960_464_48e-8;
pub const HALF_MAX: f64 = 65504.0;

const M: M3 = [[0.5, -1.0, 0.5], [-1.0, 1.0, 0.5], [0.5, 0.0, 0.0]];

#[derive(Clone, Debug)]
pub struct Spline {
    pub low: Vec<f64>,
    pub high: Vec<f64>,
    pub min: (f64, f64),
    pub mid: (f64, f64),
    pub max: (f64, f64),
    pub slope_low: f64,
    pub slope_high: f64,
}

fn quad(cf: &[f64], t: f64) -> f64 {
    let a = M[0][0] * cf[0] + M[1][0] * cf[1] + M[2][0] * cf[2];
    let b = M[0][1] * cf[0] + M[1][1] * cf[1] + M[2][1] * cf[2];
    let c = M[0][2] * cf[0] + M[1][2] * cf[1] + M[2][2] * cf[2];
    a * t * t + b * t + c
}

impl Spline {
    fn knots_low(&self) -> usize {
        self.low.len() - 2
    }

    fn knots_high(&self) -> usize {
        self.high.len() - 2
    }

    pub fn fwd(&self, x: f64) -> f64 {
        let logx = x.max(HALF_MIN).log10();
        let (lmin, lmid, lmax) = (self.min.0.log10(), self.mid.0.log10(), self.max.0.log10());
        let logy = if logx <= lmin {
            logx * self.slope_low + (self.min.1.log10() - self.slope_low * lmin)
        } else if logx < lmid {
            let kc = (self.knots_low() - 1) as f64 * (logx - lmin) / (lmid - lmin);
            let j = (kc as usize).min(self.knots_low() - 2);
            quad(&self.low[j..j + 3], kc - j as f64)
        } else if logx < lmax {
            let kc = (self.knots_high() - 1) as f64 * (logx - lmid) / (lmax - lmid);
            let j = (kc as usize).min(self.knots_high() - 2);
            quad(&self.high[j..j + 3], kc - j as f64)
        } else {
            logx * self.slope_high + (self.max.1.log10() - self.slope_high * lmax)
        };
        10f64.powf(logy)
    }

    fn rev_seg(coefs: &[f64], knots: usize, logy: f64, start: f64, inc: f64) -> f64 {
        let ky: Vec<f64> = (0..knots).map(|i| (coefs[i] + coefs[i + 1]) / 2.0).collect();
        let mut j = 0;
        for i in 0..knots - 1 {
            if logy > ky[i] && logy <= ky[i + 1] {
                j = i;
                break;
            }
            if i == knots - 2 {
                j = i;
            }
        }
        let cf = &coefs[j..j + 3];
        let a = M[0][0] * cf[0] + M[1][0] * cf[1] + M[2][0] * cf[2];
        let b = M[0][1] * cf[0] + M[1][1] * cf[1] + M[2][1] * cf[2];
        let c = M[0][2] * cf[0] + M[1][2] * cf[1] + M[2][2] * cf[2] - logy;
        let d = (b * b - 4.0 * a * c).max(0.0).sqrt();
        let t = if (-d - b).abs() < 1e-30 { 0.0 } else { (2.0 * c) / (-d - b) };
        start + (t + j as f64) * inc
    }

    pub fn rev(&self, y: f64) -> f64 {
        let logy = y.max(1e-10).log10();
        let (lmin, lmid, lmax) = (self.min.0.log10(), self.mid.0.log10(), self.max.0.log10());
        let logx = if logy <= self.min.1.log10() {
            if self.slope_low > 0.0 { (logy - (self.min.1.log10() - self.slope_low * lmin)) / self.slope_low } else { lmin }
        } else if logy <= self.mid.1.log10() {
            Self::rev_seg(&self.low, self.knots_low(), logy, lmin, (lmid - lmin) / (self.knots_low() - 1) as f64)
        } else if logy < self.max.1.log10() {
            Self::rev_seg(&self.high, self.knots_high(), logy, lmid, (lmax - lmid) / (self.knots_high() - 1) as f64)
        } else if self.slope_high > 0.0 {
            (logy - (self.max.1.log10() - self.slope_high * lmax)) / self.slope_high
        } else {
            lmax
        };
        10f64.powf(logx)
    }
}

pub fn rrt_spline() -> Spline {
    Spline {
        low: vec![-4.0, -4.0, -3.157_376_577_3, -0.485_249_995_8, 1.847_732_470_6, 1.847_732_470_6],
        high: vec![-0.718_548_242_5, 2.081_030_717_2, 3.668_124_123_7, 4.0, 4.0, 4.0],
        min: (0.18 * 2f64.powf(-15.0), 0.0001),
        mid: (0.18, 4.8),
        max: (0.18 * 2f64.powf(18.0), 10000.0),
        slope_low: 0.0,
        slope_high: 0.0,
    }
}

pub fn odt48_spline() -> Spline {
    let r = rrt_spline();
    Spline {
        low: vec![-1.698_970_004_3, -1.698_970_004_3, -1.4779, -1.2291, -0.8648, -0.448, 0.00518, 0.451_108_033_4, 0.911_374_441_4, 0.911_374_441_4],
        high: vec![0.515_438_696_5, 0.847_043_778_3, 1.1358, 1.3802, 1.5197, 1.5985, 1.6467, 1.674_609_135_7, 1.687_873_339, 1.687_873_339],
        min: (r.fwd(0.18 * 2f64.powf(-6.5)), 0.02),
        mid: (r.fwd(0.18), 4.8),
        max: (r.fwd(0.18 * 2f64.powf(6.5)), 48.0),
        slope_low: 0.0,
        slope_high: 0.04,
    }
}

fn interp1(t: [[f64; 2]; 2], x: f64) -> f64 {
    if x <= t[0][0] {
        return t[0][1];
    }
    if x >= t[1][0] {
        return t[1][1];
    }
    t[0][1] + (x - t[0][0]) / (t[1][0] - t[0][0]) * (t[1][1] - t[0][1])
}

pub fn ssts_params(min_lum: f64, max_lum: f64, exp_shift: f64) -> Spline {
    let (min_stop_sdr, max_stop_sdr, min_stop_rrt, max_stop_rrt) = (-6.5, 6.5, -15.0, 18.0);
    let (min_lum_sdr, max_lum_sdr, min_lum_rrt, max_lum_rrt) = (0.02f64, 48.0f64, 0.0001f64, 10000.0f64);
    let aces_min = 0.18 * 2f64.powf(interp1([[min_lum_rrt.log10(), min_stop_rrt], [min_lum_sdr.log10(), min_stop_sdr]], min_lum.log10()));
    let aces_max = 0.18 * 2f64.powf(interp1([[max_lum_sdr.log10(), max_stop_sdr], [max_lum_rrt.log10(), max_stop_rrt]], max_lum.log10()));
    let (lo, mid, hi) = ((aces_min, min_lum, 0.0f64), (0.18f64, 4.8f64, 1.55f64), (aces_max, max_lum, 0.0f64));
    let inc_low = (mid.0.log10() - lo.0.log10()) / 3.0;
    let mut cl = [0.0; 5];
    cl[0] = lo.2 * (lo.0.log10() - 0.5 * inc_low) + (lo.1.log10() - lo.2 * lo.0.log10());
    cl[1] = lo.2 * (lo.0.log10() + 0.5 * inc_low) + (lo.1.log10() - lo.2 * lo.0.log10());
    cl[3] = mid.2 * (mid.0.log10() - 0.5 * inc_low) + (mid.1.log10() - mid.2 * mid.0.log10());
    cl[4] = mid.2 * (mid.0.log10() + 0.5 * inc_low) + (mid.1.log10() - mid.2 * mid.0.log10());
    let pct_low = interp1([[min_stop_rrt, 0.18], [min_stop_sdr, 0.35]], (lo.0 / 0.18).log2());
    cl[2] = lo.1.log10() + pct_low * (mid.1.log10() - lo.1.log10());
    let inc_high = (hi.0.log10() - mid.0.log10()) / 3.0;
    let mut ch = [0.0; 5];
    ch[0] = mid.2 * (mid.0.log10() - 0.5 * inc_high) + (mid.1.log10() - mid.2 * mid.0.log10());
    ch[1] = mid.2 * (mid.0.log10() + 0.5 * inc_high) + (mid.1.log10() - mid.2 * mid.0.log10());
    ch[3] = hi.2 * (hi.0.log10() - 0.5 * inc_high) + (hi.1.log10() - hi.2 * hi.0.log10());
    ch[4] = hi.2 * (hi.0.log10() + 0.5 * inc_high) + (hi.1.log10() - hi.2 * hi.0.log10());
    let pct_high = interp1([[max_stop_sdr, 0.89], [max_stop_rrt, 0.90]], (hi.0 / 0.18).log2());
    ch[2] = mid.1.log10() + pct_high * (hi.1.log10() - mid.1.log10());
    let shift = |x: f64| 2f64.powf(x.log2() - exp_shift);
    Spline {
        low: vec![cl[0], cl[1], cl[2], cl[3], cl[4], cl[4]],
        high: vec![ch[0], ch[1], ch[2], ch[3], ch[4], ch[4]],
        min: (shift(aces_min), min_lum),
        mid: (shift(0.18), 4.8),
        max: (shift(aces_max), max_lum),
        slope_low: 0.0,
        slope_high: 0.0,
    }
}

pub fn ssts_for(min_lum: f64, mid_lum: f64, max_lum: f64) -> Spline {
    let def = ssts_params(min_lum, max_lum, 0.0);
    let shift = def.rev(mid_lum).log2() - 0.18f64.log2();
    ssts_params(min_lum, max_lum, shift)
}

fn rgb_2_saturation(c: [f64; 3]) -> f64 {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    (mx.max(1e-5) - mn.max(1e-5)) / mx.max(1e-2)
}

fn rgb_2_yc(c: [f64; 3]) -> f64 {
    let (r, g, b) = (c[0], c[1], c[2]);
    let chroma = (b * (b - g) + g * (g - r) + r * (r - b)).max(0.0).sqrt();
    (b + g + r + 1.75 * chroma) / 3.0
}

fn sigmoid_shaper(x: f64) -> f64 {
    let t = (1.0 - (x / 2.0).abs()).max(0.0);
    let y = 1.0 + x.signum() * (1.0 - t * t);
    y / 2.0
}

fn glow_fwd(yc: f64, gain: f64, mid: f64) -> f64 {
    if yc <= 2.0 / 3.0 * mid {
        gain
    } else if yc >= 2.0 * mid {
        0.0
    } else {
        gain * (mid / yc - 0.5)
    }
}

fn glow_inv(yc: f64, gain: f64, mid: f64) -> f64 {
    if yc <= (1.0 + gain) * 2.0 / 3.0 * mid {
        -gain / (1.0 + gain)
    } else if yc >= 2.0 * mid {
        0.0
    } else {
        gain * (mid / yc - 0.5) / (gain / 2.0 - 1.0)
    }
}

fn rgb_2_hue(c: [f64; 3]) -> f64 {
    if c[0] == c[1] && c[1] == c[2] {
        return 0.0;
    }
    let h = (3f64.sqrt() * (c[1] - c[2])).atan2(2.0 * c[0] - c[1] - c[2]).to_degrees();
    if h < 0.0 { h + 360.0 } else { h }
}

fn center_hue(h: f64, c: f64) -> f64 {
    let x = h - c;
    if x < -180.0 {
        x + 360.0
    } else if x > 180.0 {
        x - 360.0
    } else {
        x
    }
}

fn cubic_basis_shaper(x: f64, w: f64) -> f64 {
    let m: [[f64; 4]; 4] = [[-1.0 / 6.0, 3.0 / 6.0, -3.0 / 6.0, 1.0 / 6.0], [3.0 / 6.0, -6.0 / 6.0, 3.0 / 6.0, 0.0], [-3.0 / 6.0, 0.0, 3.0 / 6.0, 0.0], [1.0 / 6.0, 4.0 / 6.0, 1.0 / 6.0, 0.0]];
    let knots = [-w / 2.0, -w / 4.0, 0.0, w / 4.0, w / 2.0];
    let mut y = 0.0;
    if x > knots[0] && x < knots[4] {
        let kc = (x - knots[0]) * 4.0 / w;
        let j = kc as i64;
        let t = kc - j as f64;
        let mono = [t * t * t, t * t, t, 1.0];
        let col = match j {
            3 => 0,
            2 => 1,
            1 => 2,
            0 => 3,
            _ => return 0.0,
        };
        y = mono[0] * m[0][col] + mono[1] * m[1][col] + mono[2] * m[2][col] + mono[3] * m[3][col];
    }
    y * 1.5
}

const GLOW_GAIN: f64 = 0.05;
const GLOW_MID: f64 = 0.08;
const RED_SCALE: f64 = 0.82;
const RED_PIVOT: f64 = 0.03;
const RED_HUE: f64 = 0.0;
const RED_WIDTH: f64 = 135.0;

pub fn glow(c: [f64; 3], inverse: bool) -> [f64; 3] {
    let s = sigmoid_shaper((rgb_2_saturation(c) - 0.4) / 0.2);
    let yc = rgb_2_yc(c);
    let k = 1.0 + if inverse { glow_inv(yc, GLOW_GAIN * s, GLOW_MID) } else { glow_fwd(yc, GLOW_GAIN * s, GLOW_MID) };
    [c[0] * k, c[1] * k, c[2] * k]
}

pub fn red_mod(c: [f64; 3], inverse: bool) -> [f64; 3] {
    let hue = rgb_2_hue(c);
    let ch = center_hue(hue, RED_HUE);
    let w = cubic_basis_shaper(ch, RED_WIDTH);
    let mut o = c;
    if !inverse {
        let sat = rgb_2_saturation(c);
        o[0] = c[0] + w * sat * (RED_PIVOT - c[0]) * (1.0 - RED_SCALE);
        return o;
    }
    let min_chan = if ch < 0.0 { c[1] } else { c[2] };
    let a = w * (1.0 - RED_SCALE) - 1.0;
    let b = c[0] - w * (RED_PIVOT + min_chan) * (1.0 - RED_SCALE);
    let cc = w * RED_PIVOT * min_chan * (1.0 - RED_SCALE);
    if a.abs() > 1e-12 {
        o[0] = (-b - (b * b - 4.0 * a * cc).max(0.0).sqrt()) / (2.0 * a);
    }
    o
}

pub fn sat_adjust(c: [f64; 3], sat: f64, w: [f64; 3]) -> [f64; 3] {
    let y = w[0] * c[0] + w[1] * c[1] + w[2] * c[2];
    [y + sat * (c[0] - y), y + sat * (c[1] - y), y + sat * (c[2] - y)]
}

pub fn dark_to_dim(c: [f64; 3], gamma: f64, w: [f64; 3]) -> [f64; 3] {
    let y = (w[0] * c[0] + w[1] * c[1] + w[2] * c[2]).max(1e-10);
    let k = y.powf(gamma - 1.0);
    [c[0] * k, c[1] * k, c[2] * k]
}

pub struct Mats {
    pub ap0_ap1: M3,
    pub ap1_ap0: M3,
    pub ap1_xyz: M3,
    pub xyz_ap1: M3,
    pub ap1_y: [f64; 3],
}

pub fn mats() -> &'static Mats {
    static M: std::sync::OnceLock<Mats> = std::sync::OnceLock::new();
    M.get_or_init(|| {
        let ap0_ap1 = color::convert(&AP0, &AP1, false);
        let ap1_xyz = color::to_xyz(&AP1);
        Mats { ap1_ap0: color::inv(&ap0_ap1), ap0_ap1, xyz_ap1: color::inv(&ap1_xyz), ap1_y: ap1_xyz[1], ap1_xyz }
    })
}

pub fn sweeteners(aces: [f64; 3]) -> [f64; 3] {
    let m = mats();
    let c = red_mod(glow(aces, false), false);
    let c = [c[0].clamp(0.0, f64::MAX), c[1].clamp(0.0, f64::MAX), c[2].clamp(0.0, f64::MAX)];
    let p = color::apply(&m.ap0_ap1, c);
    let p = [p[0].clamp(0.0, HALF_MAX), p[1].clamp(0.0, HALF_MAX), p[2].clamp(0.0, HALF_MAX)];
    sat_adjust(p, 0.96, m.ap1_y)
}

pub fn sweeteners_inv(ap1: [f64; 3]) -> [f64; 3] {
    let m = mats();
    let p = sat_adjust(ap1, 1.0 / 0.96, m.ap1_y);
    let p = [p[0].clamp(0.0, HALF_MAX), p[1].clamp(0.0, HALF_MAX), p[2].clamp(0.0, HALF_MAX)];
    let c = color::apply(&m.ap1_ap0, p).map(|v| v.max(0.0));
    glow(red_mod(c, true), true)
}

#[derive(Clone, Debug)]
pub enum Output {
    Sdr { dim: bool, limit: Option<color::Prim> },
    Hdr { min: f64, mid: f64, max: f64, limit: color::Prim, ssts: Spline },
}

#[derive(Clone, Debug)]
pub struct OutputTransform {
    pub kind: Output,
    rrt: Spline,
    odt: Spline,
    cat: M3,
}

impl OutputTransform {
    pub fn sdr(dim: bool, limit: Option<color::Prim>) -> OutputTransform {
        OutputTransform { kind: Output::Sdr { dim, limit }, rrt: rrt_spline(), odt: odt48_spline(), cat: color::bradford(color::D60, color::D65) }
    }

    pub fn hdr(min: f64, mid: f64, max: f64, limit: color::Prim) -> OutputTransform {
        OutputTransform { kind: Output::Hdr { min, mid, max, limit, ssts: ssts_for(min, mid, max) }, rrt: rrt_spline(), odt: odt48_spline(), cat: color::bradford(color::D60, color::D65) }
    }

    pub fn fwd(&self, aces: [f64; 3]) -> [f64; 3] {
        let m = mats();
        let pre = sweeteners(aces);
        match &self.kind {
            Output::Sdr { dim, limit } => {
                let post = pre.map(|v| self.rrt.fwd(v));
                let y = post.map(|v| self.odt.fwd(v));
                let mut lin = y.map(|v| (v - 0.02) / (48.0 - 0.02));
                if *dim {
                    lin = dark_to_dim(lin, 0.9811, m.ap1_y);
                    lin = sat_adjust(lin, 0.93, m.ap1_y);
                }
                let xyz = color::apply(&self.cat, color::apply(&m.ap1_xyz, lin));
                match limit {
                    Some(lp) => {
                        let rgb = color::apply(&color::from_xyz(lp), xyz).map(|v| v.clamp(0.0, 1.0));
                        color::apply(&color::to_xyz(lp), rgb)
                    }
                    None => xyz,
                }
            }
            Output::Hdr { min, max, limit, ssts, .. } => {
                let post = pre.map(|v| ssts.fwd(v));
                let lin = post.map(|v| (v - min) / (max - min));
                let xyz = color::apply(&m.ap1_xyz, lin);
                let to = color::from_xyz(limit);
                let rgb = color::apply(&to, xyz).map(|v| v.clamp(0.0, 1.0));
                let xyz = color::apply(&color::inv(&to), rgb);
                let xyz = color::apply(&self.cat, xyz);
                xyz.map(|v| v * max / 100.0)
            }
        }
    }

    pub fn inv(&self, xyz: [f64; 3]) -> [f64; 3] {
        let m = mats();
        let icat = color::inv(&self.cat);
        match &self.kind {
            Output::Sdr { dim, limit } => {
                let xyz = match limit {
                    Some(lp) => {
                        let rgb = color::apply(&color::from_xyz(lp), xyz).map(|v| v.clamp(0.0, 1.0));
                        color::apply(&color::to_xyz(lp), rgb)
                    }
                    None => xyz,
                };
                let xyz = color::apply(&icat, xyz);
                let mut lin = color::apply(&m.xyz_ap1, xyz);
                if *dim {
                    lin = sat_adjust(lin, 1.0 / 0.93, m.ap1_y);
                    lin = dark_to_dim(lin, 1.0 / 0.9811, m.ap1_y);
                }
                let y = lin.map(|v| v * (48.0 - 0.02) + 0.02);
                let post = y.map(|v| self.odt.rev(v));
                let pre = post.map(|v| self.rrt.rev(v));
                sweeteners_inv(pre)
            }
            Output::Hdr { min, max, ssts, limit, .. } => {
                let xyz = color::apply(&icat, xyz.map(|v| v * 100.0 / max));
                let rgb = color::apply(&color::from_xyz(limit), xyz).map(|v| v.clamp(0.0, 1.0));
                let xyz = color::apply(&color::to_xyz(limit), rgb);
                let lin = color::apply(&m.xyz_ap1, xyz);
                let post = lin.map(|v| v * (max - min) + min);
                let pre = post.map(|v| ssts.rev(v));
                sweeteners_inv(pre)
            }
        }
    }
}

const GC_LIM: [f64; 3] = [1.147, 1.264, 1.312];
const GC_THR: [f64; 3] = [0.815, 0.803, 0.880];
const GC_PWR: f64 = 1.2;

fn gc_scale(lim: f64, thr: f64) -> f64 {
    (lim - thr) / (((1.0 - thr) / (lim - thr)).powf(-GC_PWR) - 1.0).powf(1.0 / GC_PWR)
}

fn gc_compress(d: f64, lim: f64, thr: f64, inverse: bool) -> f64 {
    if d < thr {
        return d;
    }
    let s = gc_scale(lim, thr);
    let nd = (d - thr) / s;
    if !inverse {
        return thr + s * nd / (1.0 + nd.powf(GC_PWR)).powf(1.0 / GC_PWR);
    }
    let p = nd.powf(GC_PWR);
    if p >= 1.0 {
        return d;
    }
    thr + s * (-(p / (p - 1.0))).powf(1.0 / GC_PWR)
}

pub fn gamut_compress(aces: [f64; 3], inverse: bool) -> [f64; 3] {
    let m = mats();
    let lin = color::apply(&m.ap0_ap1, aces);
    let ach = lin[0].max(lin[1]).max(lin[2]);
    if ach == 0.0 {
        return aces;
    }
    let mut out = [0.0; 3];
    for i in 0..3 {
        let d = (ach - lin[i]) / ach.abs();
        let cd = gc_compress(d, GC_LIM[i], GC_THR[i], inverse);
        out[i] = ach - cd * ach.abs();
    }
    color::apply(&m.ap1_ap0, out)
}

pub fn acescct_to_lin(v: f64) -> f64 {
    let (y_brk, a, b) = (0.155_251_141_552_511, 10.540_237_741_654_5, 0.072_905_534_195_835_5);
    if v <= y_brk { (v - b) / a } else { 2f64.powf(v * 17.52 - 9.72) }
}

pub fn lin_to_acescct(v: f64) -> f64 {
    let (x_brk, a, b) = (0.0078125, 10.540_237_741_654_5, 0.072_905_534_195_835_5);
    if v <= x_brk { a * v + b } else { (v.log2() + 9.72) / 17.52 }
}

pub fn acescc_to_lin(v: f64) -> f64 {
    if v < (9.72 - 15.0) / 17.52 { (2f64.powf(v * 17.52 - 9.72) - 2f64.powf(-16.0)) * 2.0 } else { 2f64.powf(v * 17.52 - 9.72) }
}

pub fn lin_to_acescc(v: f64) -> f64 {
    if v <= 0.0 {
        (2f64.powf(-16.0).log2() + 9.72) / 17.52
    } else if v < 2f64.powf(-15.0) {
        ((2f64.powf(-16.0) + v * 0.5).log2() + 9.72) / 17.52
    } else {
        (v.log2() + 9.72) / 17.52
    }
}

pub fn pq_encode(nits: f64) -> f64 {
    let (m1, m2, c1, c2, c3) = (0.159_301_757_812_5, 78.84375, 0.8359375, 18.8515625, 18.6875);
    let l = (nits.abs() / 10000.0).powf(m1);
    let v = ((c1 + c2 * l) / (1.0 + c3 * l)).powf(m2);
    v.copysign(nits)
}

pub fn pq_decode(v: f64) -> f64 {
    let (m1, m2, c1, c2, c3) = (0.159_301_757_812_5, 78.84375, 0.8359375, 18.8515625, 18.6875);
    let p = v.abs().min(1.0).powf(1.0 / m2);
    let l = ((p - c1).max(0.0) / (c2 - c3 * p)).powf(1.0 / m1);
    (l * 10000.0).copysign(v)
}

const HLG_A: f64 = 0.178_832_77;
const HLG_B: f64 = 0.284_668_92;
const HLG_C: f64 = 0.559_910_73;

pub fn hlg_oetf(e: f64) -> f64 {
    let e = e.max(0.0);
    if e <= 1.0 / 12.0 { (3.0 * e).sqrt() } else { HLG_A * (12.0 * e - HLG_B).ln() + HLG_C }
}

pub fn hlg_oetf_inv(v: f64) -> f64 {
    let v = v.max(0.0);
    if v <= 0.5 { v * v / 3.0 } else { (((v - HLG_C) / HLG_A).exp() + HLG_B) / 12.0 }
}
