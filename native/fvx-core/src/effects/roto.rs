use super::{EffectSpec, FxCtx, check, grp, hidden, no_pad, num, popup, slider};
use crate::render::image::Img;
use crate::render::roto;

pub const ID: &str = "fx.matte.rotoBrush";
pub const MODELS: [&str; 4] = ["Classic", "MobileSAM", "SAM 2", "SAM 2 Full Resolution"];

pub fn specs() -> Vec<EffectSpec> {
    vec![EffectSpec {
        id: ID,
        name: "Roto Brush & Refine Edge",
        category: "Matte",
        params: vec![
            popup("model", "Version", 0, &MODELS),
            check("renderRefineEdge", "Render Refine Edge", false),
            num("baseRefineRadius", "Base Refine Edge Radius", 2.0, 0.0, 100.0, 0.0, 30.0),
            grp("rotoMatte", "Roto Brush Matte"),
            num("rotoMatte/feather", "Feather", 0.0, 0.0, 100.0, 0.0, 100.0),
            num("rotoMatte/contrast", "Contrast", 0.0, 0.0, 100.0, 0.0, 100.0),
            num("rotoMatte/shiftEdge", "Shift Edge", 0.0, -100.0, 100.0, -100.0, 100.0),
            num("rotoMatte/reduceChatter", "Reduce Chatter", 0.0, 0.0, 100.0, 0.0, 100.0),
            grp("refineMatte", "Refine Edge Matte"),
            num("refineMatte/smooth", "Smooth", 0.0, 0.0, 100.0, 0.0, 100.0),
            num("refineMatte/feather", "Feather", 0.0, 0.0, 100.0, 0.0, 100.0),
            num("refineMatte/contrast", "Contrast", 50.0, 0.0, 100.0, 0.0, 100.0),
            num("refineMatte/shiftEdge", "Shift Edge", 0.0, -100.0, 100.0, -100.0, 100.0),
            check("refineMatte/decontaminate", "Decontaminate Edge Colors", true),
            slider("refineMatte/decontaminationAmount", "Decontamination Amount", 50.0, 0.0, 100.0, 1),
            hidden("strokes", "Strokes", "[]"),
            hidden("span", "Segmentation Span", ""),
            hidden("mattes", "Mattes", ""),
            hidden("frozen", "Frozen", ""),
        ],
        render,
        pad: no_pad,
    }]
}

fn matte_at(m: &roto::Mattes, f: i64, c: &FxCtx, w: usize, h: usize) -> Option<Vec<f32>> {
    let bin = m.get(f)?;
    use rayon::prelude::*;
    let mut out = vec![0.0f32; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, v) in row.iter_mut().enumerate() {
            let lp = [(x as f64 + 0.5) / c.r + c.origin[0], (y as f64 + 0.5) / c.r + c.origin[1]];
            let u = (lp[0] - m.origin[0]) * m.scale - 0.5;
            let vv = (lp[1] - m.origin[1]) * m.scale - 0.5;
            let (x0, y0) = (u.floor(), vv.floor());
            let (tx, ty) = ((u - x0) as f32, (vv - y0) as f32);
            let g = |xx: f64, yy: f64| -> f32 {
                let (xi, yi) = (xx.clamp(0.0, (m.w - 1) as f64) as usize, yy.clamp(0.0, (m.h - 1) as f64) as usize);
                if bin[yi * m.w + xi] { 1.0 } else { 0.0 }
            };
            let top = g(x0, y0) + (g(x0 + 1.0, y0) - g(x0, y0)) * tx;
            let bot = g(x0, y0 + 1.0) + (g(x0 + 1.0, y0 + 1.0) - g(x0, y0 + 1.0)) * tx;
            *v = top + (bot - top) * ty;
        }
    });
    Some(out)
}

fn render(c: &FxCtx, img: &mut Img) {
    let Some(m) = roto::parsed(&c.s("mattes")) else { return };
    let f = (c.t * c.fps).round() as i64;
    let (w, h) = (img.w, img.h);
    let Some(mut matte) = matte_at(&m, f, c, w, h) else { return };
    let chatter = c.f("rotoMatte/reduceChatter") / 100.0;
    if chatter > 0.0 {
        let mut n = 1.0f32;
        for d in [-1i64, 1] {
            if let Some(o) = matte_at(&m, f + d, c, w, h) {
                for (a, b) in matte.iter_mut().zip(&o) {
                    *a += b * chatter as f32;
                }
                n += chatter as f32;
            }
        }
        matte.iter_mut().for_each(|v| *v /= n);
    }
    let guide: Vec<f32> = img.px.chunks_exact(4).map(|p| 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2]).collect();
    let refine = c.b("renderRefineEdge");
    let base = c.f("baseRefineRadius").max(0.0);
    let radius = ((if refine { base * 3.0 + 2.0 } else { base.max(1.0) }) * c.r / m.scale.max(1e-6) * 0.5).clamp(1.0, 64.0) as usize;
    let mut soft = roto::guided(&matte, &guide, w, h, radius, if refine { 1e-4 } else { 1e-3 });
    let (fk, ck, sk) = if refine { ("refineMatte/feather", "refineMatte/contrast", "refineMatte/shiftEdge") } else { ("rotoMatte/feather", "rotoMatte/contrast", "rotoMatte/shiftEdge") };
    if refine && c.f("refineMatte/smooth") > 0.0 {
        let s = c.px(c.f("refineMatte/smooth") / 10.0);
        crate::effects::util::blur_scalar(&mut soft, w, h, s, s);
    }
    let feather = c.f(fk);
    if feather > 0.0 {
        let s = c.px(feather / 4.0);
        crate::effects::util::blur_scalar(&mut soft, w, h, s, s);
    }
    let contrast = c.f(ck) / 100.0;
    let shift = c.f(sk) / 100.0;
    for v in soft.iter_mut() {
        let mut x = *v + shift as f32 * 0.5;
        x = (x - 0.5) * (1.0 + contrast as f32 * 4.0) + 0.5;
        *v = x.clamp(0.0, 1.0);
    }
    let decon = refine && c.b("refineMatte/decontaminate");
    let amount = (c.f("refineMatte/decontaminationAmount") / 100.0) as f32;
    for (p, a) in img.px.chunks_exact_mut(4).zip(&soft) {
        if decon && *a > 0.0 && *a < 1.0 && p[3] > 1e-6 {
            let k = 1.0 + (1.0 / a.max(0.05) - 1.0) * amount * 0.25;
            for ch in 0..3 {
                p[ch] = (p[ch] * k).min(p[3]);
            }
        }
        for ch in 0..4 {
            p[ch] *= a;
        }
    }
}
