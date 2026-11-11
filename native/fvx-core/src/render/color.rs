use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use rayon::prelude::*;

use super::image::Img;
use super::ocio::{self, Fixed, Neg, Op, Proc};
use crate::model::{Footage, ProjectSettings};

pub type M3 = [[f64; 3]; 3];

pub const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prim {
    pub r: [f64; 2],
    pub g: [f64; 2],
    pub b: [f64; 2],
    pub w: [f64; 2],
}

pub const D65: [f64; 2] = [0.3127, 0.3290];
pub const D60: [f64; 2] = [0.32168, 0.33767];
pub const D50: [f64; 2] = [0.3457, 0.3585];
pub const DCI: [f64; 2] = [0.314, 0.351];

pub const REC709: Prim = Prim { r: [0.64, 0.33], g: [0.30, 0.60], b: [0.15, 0.06], w: D65 };
pub const REC2020: Prim = Prim { r: [0.708, 0.292], g: [0.170, 0.797], b: [0.131, 0.046], w: D65 };
pub const P3D65: Prim = Prim { r: [0.680, 0.320], g: [0.265, 0.690], b: [0.150, 0.060], w: D65 };
pub const P3DCI: Prim = Prim { w: DCI, ..P3D65 };
pub const P3D60: Prim = Prim { w: D60, ..P3D65 };
pub const AP0: Prim = Prim { r: [0.7347, 0.2653], g: [0.0, 1.0], b: [0.0001, -0.0770], w: D60 };
pub const AP1: Prim = Prim { r: [0.713, 0.293], g: [0.165, 0.830], b: [0.128, 0.044], w: D60 };
pub const OPRGB: Prim = Prim { r: [0.64, 0.33], g: [0.21, 0.71], b: [0.15, 0.06], w: D65 };
pub const PROPHOTO: Prim = Prim { r: [0.734699, 0.265301], g: [0.159597, 0.840403], b: [0.036598, 0.000105], w: D50 };
pub const XYZ: Prim = Prim { r: [1.0, 0.0], g: [0.0, 1.0], b: [0.0, 0.0], w: [1.0 / 3.0, 1.0 / 3.0] };

pub fn mul(a: &M3, b: &M3) -> M3 {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    o
}

pub fn apply(m: &M3, v: [f64; 3]) -> [f64; 3] {
    [m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2], m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2], m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2]]
}

pub fn inv(m: &M3) -> M3 {
    let d = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if d.abs() < 1e-300 {
        return IDENTITY;
    }
    let k = 1.0 / d;
    [
        [(m[1][1] * m[2][2] - m[1][2] * m[2][1]) * k, (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * k, (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * k],
        [(m[1][2] * m[2][0] - m[1][0] * m[2][2]) * k, (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * k, (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * k],
        [(m[1][0] * m[2][1] - m[1][1] * m[2][0]) * k, (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * k, (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * k],
    ]
}

pub fn white_xyz(w: [f64; 2]) -> [f64; 3] {
    [w[0] / w[1], 1.0, (1.0 - w[0] - w[1]) / w[1]]
}

pub fn to_xyz(p: &Prim) -> M3 {
    if *p == XYZ {
        return IDENTITY;
    }
    let col = |c: [f64; 2]| [c[0] / c[1], 1.0, (1.0 - c[0] - c[1]) / c[1]];
    let (r, g, b) = (col(p.r), col(p.g), col(p.b));
    let pm = [[r[0], g[0], b[0]], [r[1], g[1], b[1]], [r[2], g[2], b[2]]];
    let s = apply(&inv(&pm), white_xyz(p.w));
    [[pm[0][0] * s[0], pm[0][1] * s[1], pm[0][2] * s[2]], [pm[1][0] * s[0], pm[1][1] * s[1], pm[1][2] * s[2]], [pm[2][0] * s[0], pm[2][1] * s[1], pm[2][2] * s[2]]]
}

pub fn from_xyz(p: &Prim) -> M3 {
    inv(&to_xyz(p))
}

const BRADFORD: M3 = [[0.8951, 0.2664, -0.1614], [-0.7502, 1.7135, 0.0367], [0.0389, -0.0685, 1.0296]];

pub fn bradford(src: [f64; 2], dst: [f64; 2]) -> M3 {
    if (src[0] - dst[0]).abs() < 1e-9 && (src[1] - dst[1]).abs() < 1e-9 {
        return IDENTITY;
    }
    let s = apply(&BRADFORD, white_xyz(src));
    let d = apply(&BRADFORD, white_xyz(dst));
    let sc = [[d[0] / s[0], 0.0, 0.0], [0.0, d[1] / s[1], 0.0], [0.0, 0.0, d[2] / s[2]]];
    mul(&inv(&BRADFORD), &mul(&sc, &BRADFORD))
}

pub fn convert(src: &Prim, dst: &Prim, adapt: bool) -> M3 {
    let cat = if adapt { bradford(src.w, dst.w) } else { IDENTITY };
    mul(&from_xyz(dst), &mul(&cat, &to_xyz(src)))
}

pub fn luma(p: &Prim) -> [f64; 3] {
    to_xyz(p)[1]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Linear,
    Srgb,
    Gamma(f64),
    Pq,
    Hlg,
    Cct,
}

#[derive(Clone, Copy, Debug)]
pub struct ASpace {
    pub id: &'static str,
    pub name: &'static str,
    pub prim: Prim,
    pub curve: Curve,
    pub working: bool,
}

pub const SPACES: [ASpace; 17] = [
    ASpace { id: "srgb", name: "sRGB IEC61966-2.1", prim: REC709, curve: Curve::Srgb, working: true },
    ASpace { id: "rec709", name: "HDTV (Rec. 709)", prim: REC709, curve: Curve::Gamma(2.4), working: true },
    ASpace { id: "rec2020", name: "UHDTV (Rec. 2020)", prim: REC2020, curve: Curve::Gamma(2.4), working: true },
    ASpace { id: "p3", name: "Display P3", prim: P3D65, curve: Curve::Srgb, working: true },
    ASpace { id: "p3d65", name: "P3-D65 (Gamma 2.6)", prim: P3D65, curve: Curve::Gamma(2.6), working: true },
    ASpace { id: "dciP3", name: "DCI-P3 (Gamma 2.6)", prim: P3DCI, curve: Curve::Gamma(2.6), working: true },
    ASpace { id: "opRgb", name: "opRGB (1998)", prim: OPRGB, curve: Curve::Gamma(2.199_218_75), working: true },
    ASpace { id: "proPhoto", name: "ProPhoto RGB", prim: PROPHOTO, curve: Curve::Gamma(1.8), working: true },
    ASpace { id: "linearSrgb", name: "Linear sRGB (Rec. 709)", prim: REC709, curve: Curve::Linear, working: true },
    ASpace { id: "linearRec2020", name: "Linear Rec. 2020", prim: REC2020, curve: Curve::Linear, working: true },
    ASpace { id: "linearP3", name: "Linear P3-D65", prim: P3D65, curve: Curve::Linear, working: true },
    ASpace { id: "acescg", name: "ACEScg", prim: AP1, curve: Curve::Linear, working: true },
    ASpace { id: "aces2065", name: "ACES2065-1", prim: AP0, curve: Curve::Linear, working: true },
    ASpace { id: "acescct", name: "ACEScct", prim: AP1, curve: Curve::Cct, working: true },
    ASpace { id: "rec2100pq", name: "Rec. 2100 PQ", prim: REC2020, curve: Curve::Pq, working: false },
    ASpace { id: "rec2100hlg", name: "Rec. 2100 HLG", prim: REC2020, curve: Curve::Hlg, working: false },
    ASpace { id: "mac18", name: "Legacy Mac (Gamma 1.8)", prim: REC709, curve: Curve::Gamma(1.8), working: false },
];

pub fn find(id: &str) -> Option<&'static ASpace> {
    let k = id.trim();
    let k = match k.to_ascii_lowercase().as_str() {
        "linear" | "linear srgb" | "lin_srgb" => "linearSrgb",
        "hdtv" | "rec.709" | "rec. 709" => "rec709",
        "rec.2020" | "rec. 2020" | "uhdtv" => "rec2020",
        "display p3" | "displayp3" => "p3",
        "op rgb" | "oprgb" => "opRgb",
        "aces" | "aces2065-1" => "aces2065",
        _ => crate::legacy::alias(k).unwrap_or(k),
    };
    SPACES.iter().find(|s| s.id.eq_ignore_ascii_case(k) || s.name.eq_ignore_ascii_case(k))
}

const SDR_WHITE_NITS: f64 = 203.0;
const HLG_REF: f64 = 0.264_9;

fn scale(k: f64) -> Op {
    Op::Mat([[k, 0.0, 0.0], [0.0, k, 0.0], [0.0, 0.0, k]], [0.0; 3])
}

pub fn decode_ops(c: Curve) -> Vec<Op> {
    match c {
        Curve::Linear => vec![],
        Curve::Srgb => vec![Op::Moncurve { g: [2.4; 3], o: [0.055; 3], neg: Neg::Mirror, encode: false }],
        Curve::Gamma(g) => vec![Op::Pow([g; 3], Neg::Mirror)],
        Curve::Pq => vec![Op::Fixed(Fixed::Pq, true), scale(100.0 / SDR_WHITE_NITS)],
        Curve::Hlg => vec![Op::Fixed(Fixed::HlgOetf, true), scale(1.0 / HLG_REF)],
        Curve::Cct => vec![Op::Fixed(Fixed::Cct, false)],
    }
}

pub fn encode_ops(c: Curve) -> Vec<Op> {
    decode_ops(c).iter().rev().map(Op::inverse).collect()
}

pub fn convert_spaces(a: &ASpace, a_lin: bool, b: &ASpace, b_lin: bool, tonemap: bool) -> Proc {
    let mut ops = if a_lin { vec![] } else { decode_ops(a.curve) };
    if tonemap && !matches!(b.curve, Curve::Pq | Curve::Hlg) {
        ops.push(Op::Fixed(Fixed::Rolloff, false));
    }
    if a.prim != b.prim {
        ops.push(Op::Mat(convert(&a.prim, &b.prim, true), [0.0; 3]));
    }
    if !b_lin {
        ops.extend(encode_ops(b.curve));
    }
    Proc::new(ops)
}

#[derive(Clone, Debug, Default)]
pub struct Display {
    pub off: bool,
    pub profile: String,
    pub simulate: String,
    pub preserve: bool,
}

#[derive(Clone, Default)]
pub struct Pipe {
    pub managed: bool,
    pub ocio: bool,
    pub depth: u8,
    pub linear: bool,
    pub authored: Option<Arc<Proc>>,
    pub blend: Option<(Arc<Proc>, Arc<Proc>)>,
    pub working: String,
    pub cfg: Option<Arc<ocio::Config>>,
    pub tonemap: bool,
    pub display: String,
    pub view: String,
    pub error: Option<String>,
    space: Option<&'static ASpace>,
}

fn arc(p: Proc) -> Option<Arc<Proc>> {
    if p.is_identity() { None } else { Some(Arc::new(p)) }
}

fn proc_cache() -> &'static Mutex<HashMap<String, Option<Arc<Proc>>>> {
    static C: OnceLock<Mutex<HashMap<String, Option<Arc<Proc>>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

impl Pipe {
    pub fn of(s: &ProjectSettings) -> Pipe {
        let mut p = Pipe { depth: s.bit_depth, tonemap: s.hdr == "toneMap" && s.bit_depth == 32, display: s.display.clone(), view: s.view.clone(), ..Pipe::default() };
        if s.color_engine.eq_ignore_ascii_case("ocio") {
            let cfg = match ocio::load(&s.ocio_config) {
                Ok(c) => c,
                Err(e) => {
                    p.error = Some(e);
                    return p;
                }
            };
            let ws = cfg.space(&s.working_space).filter(|w| !w.display && !w.data).or_else(|| cfg.role("scene_linear").and_then(|r| cfg.space(r))).or_else(|| cfg.spaces.iter().find(|x| !x.display && !x.data));
            let Some(ws) = ws else { return p };
            p.managed = true;
            p.ocio = true;
            p.working = ws.name.clone();
            p.linear = ws.encoding == "scene-linear" || ws.name.to_ascii_lowercase().contains("lin") || ws.name.eq_ignore_ascii_case("ACEScg") || ws.name.eq_ignore_ascii_case("ACES2065-1");
            if let Some(pick) = cfg.role("color_picking")
                && cfg.space(pick).is_some_and(|x| x.name != ws.name)
            {
                p.authored = cfg.convert(pick, &ws.name).ok().filter(|x| !x.is_identity());
            }
            if s.blend_linear && !p.linear
                && let Some(lin) = cfg.role("scene_linear")
                && let (Ok(a), Ok(b)) = (cfg.convert(&ws.name, lin), cfg.convert(lin, &ws.name))
            {
                p.blend = Some((a, b));
            }
            p.cfg = Some(cfg);
            return p;
        }
        let Some(ws) = find(&s.working_space).filter(|w| w.working) else { return p };
        p.managed = true;
        p.space = Some(ws);
        p.working = ws.id.to_string();
        p.linear = s.linearize || ws.curve == Curve::Linear;
        if s.linearize && ws.curve != Curve::Linear {
            p.authored = arc(Proc::new(decode_ops(ws.curve)));
        }
        if s.blend_linear && !p.linear {
            p.blend = Some((Arc::new(Proc::new(decode_ops(ws.curve))), Arc::new(Proc::new(encode_ops(ws.curve)))));
        }
        p
    }

    fn key(&self) -> String {
        format!("{}|{}|{}|{}|{}", self.ocio, self.working, self.linear, self.cfg.as_ref().map(|c| format!("{}#{}", c.name, c.serial)).unwrap_or_default(), self.tonemap)
    }

    fn cached(&self, k: String, f: impl FnOnce() -> Option<Arc<Proc>>) -> Option<Arc<Proc>> {
        let key = format!("{}#{k}", self.key());
        if let Some(v) = proc_cache().lock().get(&key) {
            return v.clone();
        }
        let v = f();
        let mut c = proc_cache().lock();
        if c.len() > 512 {
            c.clear();
        }
        c.insert(key, v.clone());
        v
    }

    pub fn footage(&self, f: &Footage) -> Option<Arc<Proc>> {
        if !self.managed {
            if f.transfer == "linearSrgb" && !f.preserve_rgb {
                return self.cached("ft:unmanaged-linear".into(), || arc(Proc::new(encode_ops(Curve::Srgb))));
            }
            return None;
        }
        if f.preserve_rgb {
            return self.authored.clone();
        }
        let profile = if !f.color_profile.is_empty() { f.color_profile.clone() } else if !f.transfer.is_empty() { f.transfer.clone() } else { String::new() };
        self.cached(format!("ft:{profile}"), || {
            if let Some(cfg) = &self.cfg {
                let src = if profile.is_empty() { None } else { cfg.space(&profile).map(|s| s.name.clone()) }
                    .or_else(|| find(&profile).and_then(|a| ocio_name_for(cfg, a)))
                    .or_else(|| cfg.role("texture_paint").or(cfg.role("color_picking")).or(cfg.role("default")).map(str::to_string))?;
                return cfg.convert(&src, &self.working).ok().filter(|p| !p.is_identity());
            }
            let ws = self.space?;
            let src = find(&profile).unwrap_or(&SPACES[0]);
            arc(convert_spaces(src, false, ws, self.linear, false))
        })
    }

    pub fn scene_linear(&self) -> Option<Arc<Proc>> {
        if !self.managed {
            return self.cached("sl:unmanaged".into(), || arc(Proc::new(decode_ops(Curve::Srgb))));
        }
        if let Some(cfg) = &self.cfg {
            let lin = cfg.role("scene_linear").map(str::to_string)?;
            return self.cached(format!("sl:{lin}"), || cfg.convert(&self.working, &lin).ok().filter(|p| !p.is_identity()));
        }
        let ws = self.space?;
        if self.linear || ws.curve == Curve::Linear {
            return None;
        }
        self.cached("sl:profile".into(), || arc(Proc::new(decode_ops(ws.curve))))
    }

    pub fn display_proc(&self, d: &Display) -> Option<Arc<Proc>> {
        if !self.managed {
            return None;
        }
        if let Some(cfg) = &self.cfg {
            let disp = if self.display.is_empty() { cfg.display_names().first().cloned().unwrap_or_default() } else { self.display.clone() };
            let view = if self.view.is_empty() { cfg.views(&disp).first().cloned().unwrap_or_default() } else { self.view.clone() };
            if d.off {
                return None;
            }
            return self.cached(format!("dv:{disp}:{view}"), || cfg.display_view(&self.working, &disp, &view, None, false).ok().filter(|p| !p.is_identity()));
        }
        let ws = self.space?;
        let key = format!("ad:{}:{}:{}:{}", d.off, d.profile, d.simulate, d.preserve);
        self.cached(key, || {
            if d.off {
                return if self.linear && ws.curve != Curve::Linear { arc(Proc::new(encode_ops(ws.curve))) } else { None };
            }
            let disp = find(&d.profile).unwrap_or(&SPACES[0]);
            if let Some(out) = find(&d.simulate) {
                let to_out = convert_spaces(ws, self.linear, out, false, self.tonemap);
                if d.preserve {
                    return arc(to_out);
                }
                return arc(to_out.then(&convert_spaces(out, false, disp, false, false)));
            }
            arc(convert_spaces(ws, self.linear, disp, false, self.tonemap))
        })
    }

    pub fn output(&self, space: &str) -> Result<Option<Arc<Proc>>, String> {
        if !self.managed {
            return Ok(None);
        }
        let space = space.trim();
        if let Some(cfg) = &self.cfg {
            if space.is_empty() || space.eq_ignore_ascii_case("working") {
                return Ok(None);
            }
            if cfg.space(space).is_some() {
                let p = cfg.convert(&self.working, space)?;
                return Ok(if p.is_identity() { None } else { Some(p) });
            }
            if let Some((d, v)) = space.split_once(" | ").or_else(|| space.split_once('|')) {
                return Ok(Some(cfg.display_view(&self.working, d.trim(), v.trim(), None, false)?));
            }
            return Err(format!("no colour space or display `{space}` in the OCIO config"));
        }
        let Some(ws) = self.space else { return Ok(None) };
        if space.is_empty() || space.eq_ignore_ascii_case("working") {
            return Ok(if self.linear && ws.curve != Curve::Linear { arc(Proc::new(encode_ops(ws.curve))) } else { None });
        }
        let out = find(space).ok_or_else(|| format!("unknown output colour profile `{space}` ({})", SPACES.iter().map(|s| s.id).collect::<Vec<_>>().join(", ")))?;
        Ok(arc(convert_spaces(ws, self.linear, out, false, self.tonemap)))
    }

    pub fn quantize(&self, img: &mut Img) {
        let k = match self.depth {
            8 => 255.0f32,
            16 => 32768.0,
            _ => return,
        };
        img.px.par_iter_mut().for_each(|v| *v = (v.clamp(0.0, 1.0) * k + 0.5).floor() / k);
    }
}

fn ocio_name_for(cfg: &ocio::Config, a: &ASpace) -> Option<String> {
    let names: &[&str] = match a.id {
        "srgb" => &["sRGB Encoded Rec.709 (sRGB)", "srgb_tx", "sRGB - Texture", "Utility - sRGB - Texture", "sRGB"],
        "rec709" => &["Gamma 2.4 Encoded Rec.709", "g24_rec709", "rec709"],
        "rec2020" => &["Gamma 2.4 Encoded Rec.2020", "rec2020"],
        "p3" => &["sRGB Encoded P3-D65", "srgb_p3d65"],
        "linearSrgb" => &["Linear Rec.709 (sRGB)", "lin_rec709", "lin_srgb"],
        "linearRec2020" => &["Linear Rec.2020", "lin_rec2020"],
        "linearP3" => &["Linear P3-D65", "lin_p3d65"],
        "acescg" => &["ACEScg"],
        "aces2065" => &["ACES2065-1"],
        "acescct" => &["ACEScct"],
        "opRgb" => &["Gamma 2.2 Encoded opRGB", "oprgb"],
        _ => &[],
    };
    names.iter().find_map(|n| cfg.space(n).map(|s| s.name.clone())).or_else(|| {
        cfg.spaces.iter().find(|s| std::iter::once(&s.name).chain(s.aliases.iter()).any(|n| crate::legacy::alias(n).is_some_and(|c| names.iter().any(|w| w.eq_ignore_ascii_case(c))))).map(|s| s.name.clone())
    })
}
