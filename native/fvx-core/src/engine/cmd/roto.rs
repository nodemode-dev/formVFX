use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use serde_json::{Value as Json, json};

use crate::effects::roto as fx;
use crate::engine::jobs::{self, Apply, Ctx};
use crate::engine::{Engine, Res, pb, pf, ps};
use crate::model::prop::{Group, GroupKind};
use crate::model::{Project, Value};
use crate::render::analysis::Source;
use crate::render::media::Media;
use crate::render::roto::{self, MOBILESAM, Mattes, ModelInfo, NEEDS, Stroke};
use crate::render::sam2;
use crate::render::vision::Gray;

const SIDE: f64 = 1024.0;

fn is_roto(g: &Group) -> bool {
    matches!(&g.kind, GroupKind::Effect { effect } if effect == fx::ID)
}

fn active_model(e: &Engine) -> u32 {
    match e.prefs.get("rotoModel").and_then(Json::as_str) {
        Some("mobilesam") => 1,
        Some("sam2") => 2,
        Some("sam2full") => 3,
        _ => 0,
    }
}

fn need_model(model: u32) -> Result<(), String> {
    match model {
        1 if !roto::installed() => Err(NEEDS.into()),
        2 | 3 if !roto::installed_of(&roto::SAM2) => Err(roto::NEEDS_SAM2.into()),
        _ => Ok(()),
    }
}

fn effect_uid(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<Option<u64>, String> {
    let l = e.layer_ref(cid, lid)?;
    let Some(fxg) = l.effects() else { return Ok(None) };
    match p.get("effect").and_then(Json::as_u64) {
        Some(u) => fxg.groups().find(|g| g.uid == u && is_roto(g)).map(|g| Some(g.uid)).ok_or_else(|| "no Roto Paint with that uid".into()),
        None => Ok(fxg.groups().filter(|g| is_roto(g)).last().map(|g| g.uid)),
    }
}

fn ensure(e: &mut Engine, cid: u64, lid: u64, p: &Json) -> Result<u64, String> {
    if let Some(u) = effect_uid(e, cid, lid, p)? {
        return Ok(u);
    }
    let r = super::effect::apply(e, &json!({"effect": fx::ID, "layers": [lid], "comp": cid}))?;
    let uid = r.get("applied").and_then(|a| a.get(0)).and_then(|a| a.get("uid")).and_then(Json::as_u64).ok_or("could not add the Roto Paint")?;
    let m = active_model(e);
    if let Some(pr) = e.layer_mut(cid, lid)?.props.find_group_mut(uid).and_then(|g| g.get_mut("model")) {
        pr.value = Value::Enum(m);
    }
    Ok(uid)
}

struct State {
    model: u32,
    strokes: Vec<Stroke>,
    span: Option<(i64, i64)>,
    mattes: Option<Mattes>,
    frozen: bool,
}

fn s_of(g: &Group, id: &str) -> String {
    match g.get(id).map(|p| &p.value) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

fn read(e: &Engine, cid: u64, lid: u64, uid: u64) -> Result<State, String> {
    let g = e.layer_ref(cid, lid)?.props.find_group(uid).ok_or("no Roto Paint")?;
    let span = s_of(g, "span").split_once(',').and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)));
    Ok(State {
        model: g.get("model").map(|p| p.value.index()).unwrap_or(0),
        strokes: roto::parse_strokes(&s_of(g, "strokes")),
        span,
        mattes: Mattes::decode(&s_of(g, "mattes")),
        frozen: !s_of(g, "frozen").is_empty(),
    })
}

fn write(e: &mut Engine, cid: u64, lid: u64, uid: u64, st: &State) -> Result<(), String> {
    let g = e.layer_mut(cid, lid)?.props.find_group_mut(uid).ok_or("no Roto Paint")?;
    let set = |g: &mut Group, id: &str, v: String| {
        if let Some(pr) = g.get_mut(id) {
            pr.value = Value::Str(v);
        }
    };
    set(g, "strokes", serde_json::to_string(&st.strokes).unwrap_or_else(|_| "[]".into()));
    set(g, "span", st.span.map(|(a, b)| format!("{a},{b}")).unwrap_or_default());
    set(g, "mattes", st.mattes.as_ref().map(|m| m.encode()).unwrap_or_default());
    set(g, "frozen", if st.frozen { "1".into() } else { String::new() });
    Ok(())
}

fn fps_of(e: &Engine, cid: u64) -> Result<f64, String> {
    Ok(e.project.comp(cid).ok_or("no composition")?.frame_rate)
}

fn frame_range(e: &Engine, cid: u64, lid: u64) -> Result<(i64, i64), String> {
    let c = e.project.comp(cid).ok_or("no composition")?;
    let l = c.layer(lid).ok_or("no layer")?;
    let fps = c.frame_rate;
    let fd = c.frame_duration();
    Ok(((l.layer_time(l.in_point) * fps).round() as i64, (l.layer_time(l.out_point - fd) * fps).round() as i64))
}

fn current_frame(e: &Engine, cid: u64, lid: u64, p: &Json) -> Result<i64, String> {
    if let Some(f) = pf(p, "frame") {
        return Ok(f.round() as i64);
    }
    let fps = fps_of(e, cid)?;
    Ok((e.layer_ref(cid, lid)?.layer_time(e.time_of(cid)) * fps).round() as i64)
}

struct Seg {
    project: Arc<Project>,
    media: Arc<Media>,
    cid: u64,
    lid: u64,
    fps: f64,
    model: u32,
}

impl Seg {
    fn source(&self) -> Result<Source<'_>, String> {
        Source::new(&self.project, &self.media, self.cid, self.lid, SIDE).ok_or_else(|| "the layer is gone".into())
    }

    fn comp_time(&self, f: i64) -> f64 {
        self.project.comp(self.cid).and_then(|c| c.layer(self.lid)).map(|l| l.comp_time(f as f64 / self.fps)).unwrap_or(0.0)
    }

    fn frame(&self, src: &Source, f: i64, strokes: &[Stroke], prior: Option<&[f32]>, fixed: Option<&roto::Models>) -> Result<(Vec<bool>, Gray), String> {
        let img = src.image(self.comp_time(f)).ok_or("cannot read the layer's frames")?;
        let on: Vec<&Stroke> = strokes.iter().filter(|s| s.f == f).collect();
        let lab = roto::labels(&on, img.w, img.h, src.scale, [src.frame[0], src.frame[1]]);
        if self.model >= 2 {
            let m = roto::load_sam2()?;
            let s = self.side();
            let feats = m.encode(&sam2::prepare(&img.px, img.w, img.h, s), s);
            let (low, _) = m.segment(&feats, &sam2_points(&lab, img.w, img.h, s), s);
            return Ok((sam2_matte(&low, s, img.w, img.h, &lab), Gray::from_img(&img, 1)));
        }
        let soft = if self.model == 1 { roto::with_sam(roto::load_sam()?.as_ref(), &img, &lab, prior) } else { roto::classic(&img, &lab, prior, fixed) };
        Ok((roto::clean(&soft, &lab, img.w, img.h), Gray::from_img(&img, 1)))
    }

    fn side(&self) -> usize {
        if self.model == 3 { 1024 } else { 512 }
    }

    fn propagate_sam2(&self, ctx: Option<&Ctx>, st: &State, from: i64, to: i64) -> Result<Vec<(i64, Vec<bool>)>, String> {
        let src = self.source()?;
        let m = roto::load_sam2()?;
        let s = self.side();
        let base = self.mattes_for(&src, st.mattes.as_ref());
        let feats_of = |f: i64| -> Result<(sam2::Feats, usize, usize, Vec<u8>), String> {
            let img = src.image(self.comp_time(f)).ok_or("cannot read the layer's frames")?;
            let on: Vec<&Stroke> = st.strokes.iter().filter(|x| x.f == f).collect();
            let lab = roto::labels(&on, img.w, img.h, src.scale, [src.frame[0], src.frame[1]]);
            Ok((m.encode(&sam2::prepare(&img.px, img.w, img.h, s), s), img.w, img.h, lab))
        };
        let mut out = vec![];
        let (f0, w, h, lab0) = feats_of(from)?;
        let anchor = match base.get(from) {
            Some(a) => a,
            None => {
                let (low, _) = m.segment(&f0, &sam2_points(&lab0, w, h, s), s);
                let a = sam2_matte(&low, s, w, h, &lab0);
                out.push((from, a.clone()));
                a
            }
        };
        let mask: Vec<f32> = anchor.iter().map(|b| if *b { 1.0 } else { 0.0 }).collect();
        let mask: Vec<f32> = sam2::resample_aa(&mask, 1, h, w, s, s, false).into_iter().map(|v| if v >= 0.5 { 1.0 } else { 0.0 }).collect();
        let (_, mem) = m.condition(&f0, &mask, s);
        let mut mems: BTreeMap<i64, sam2::Memory> = BTreeMap::new();
        mems.insert(0, mem);
        let total = (to - from).abs();
        let step = if to >= from { 1 } else { -1 };
        for k in 1..=total {
            if let Some(c) = ctx {
                if c.stopped() {
                    return Err("stopped".into());
                }
                c.step(k as usize, total as usize, "Propagating");
            }
            let f = from + step * k;
            let (fe, w, h, lab) = feats_of(f)?;
            let (low, mem) = if lab.iter().any(|l| *l != 0) { m.segment(&fe, &sam2_points(&lab, w, h, s), s) } else { m.track(&fe, k, &mems, total + 1, false, s) };
            mems.insert(k, mem);
            mems.retain(|t, x| x.cond || *t + 16 > k);
            out.push((f, sam2_matte(&low, s, w, h, &lab)));
        }
        Ok(out)
    }

    fn anchor_models(&self, src: &Source, f: i64, strokes: &[Stroke], matte: &[bool]) -> Option<roto::Models> {
        let img = src.image(self.comp_time(f))?;
        let on: Vec<&Stroke> = strokes.iter().filter(|s| s.f == f).collect();
        let lab = roto::labels(&on, img.w, img.h, src.scale, [src.frame[0], src.frame[1]]);
        Some(roto::models(&img, &lab, Some(matte)))
    }

    fn mattes_for(&self, src: &Source, existing: Option<&Mattes>) -> Mattes {
        let (w, h) = src.size();
        match existing {
            Some(m) if m.w == w && m.h == h && (m.scale - src.scale).abs() < 1e-9 => m.clone(),
            _ => Mattes { w, h, scale: src.scale, origin: [src.frame[0], src.frame[1]], frames: Default::default() },
        }
    }

    fn propagate(&self, ctx: Option<&Ctx>, st: &State, from: i64, to: i64) -> Result<Vec<(i64, Vec<bool>)>, String> {
        if self.model >= 2 {
            return self.propagate_sam2(ctx, st, from, to);
        }
        let src = self.source()?;
        let mut out = vec![];
        let base = self.mattes_for(&src, st.mattes.as_ref());
        let (mut prev, mut prev_gray) = match base.get(from) {
            Some(m) => {
                let img = src.image(self.comp_time(from)).ok_or("cannot read the layer's frames")?;
                (m, Gray::from_img(&img, 1))
            }
            None => {
                let (m, g) = self.frame(&src, from, &st.strokes, None, None)?;
                out.push((from, m.clone()));
                (m, g)
            }
        };
        let fixed = if self.model == 0 { self.anchor_models(&src, from, &st.strokes, &prev) } else { None };
        let step = if to >= from { 1 } else { -1 };
        let total = (to - from).unsigned_abs() as usize;
        let mut f = from;
        let mut done = 0;
        while f != to {
            f += step;
            done += 1;
            if let Some(c) = ctx {
                if c.stopped() {
                    return Err("stopped".into());
                }
                c.step(done, total, "Propagating");
            }
            let img = src.image(self.comp_time(f)).ok_or("cannot read the layer's frames")?;
            let gray = Gray::from_img(&img, 1);
            let prior = roto::warp_prior(&prev, &prev_gray, &gray);
            let (m, _) = self.frame(&src, f, &st.strokes, Some(&prior), fixed.as_ref())?;
            let warped: Vec<bool> = prior.iter().map(|v| *v >= 0.5).collect();
            let m = if roto::iou(&m, &warped) < 0.4 && warped.iter().any(|b| *b) { roto::clean(&prior, &vec![0u8; prior.len()], img.w, img.h) } else { m };
            out.push((f, m.clone()));
            prev = m;
            prev_gray = gray;
        }
        Ok(out)
    }
}

fn sam2_points(lab: &[u8], w: usize, h: usize, s: usize) -> Vec<([f32; 2], i32)> {
    let (kx, ky) = (s as f32 / w.max(1) as f32, s as f32 / h.max(1) as f32);
    let mut pts = vec![];
    for p in roto::sample_points(lab, w, 1, 6) {
        pts.push(([p[0] * kx, p[1] * ky], 1));
    }
    for p in roto::sample_points(lab, w, 2, 6) {
        pts.push(([p[0] * kx, p[1] * ky], 0));
    }
    pts
}

fn sam2_matte(low: &[f32], s: usize, w: usize, h: usize, lab: &[u8]) -> Vec<bool> {
    let ms = s / 4;
    let up = sam2::bilinear(low, ms, ms, h, w);
    let soft: Vec<f32> = up.iter().map(|v| 1.0 / (1.0 + (-v).exp())).collect();
    roto::clean(&soft, lab, w, h)
}

fn seg_for(e: &Engine, cid: u64, lid: u64, model: u32) -> Result<Seg, String> {
    Ok(Seg { project: e.project.clone(), media: e.media.clone(), cid, lid, fps: fps_of(e, cid)?, model })
}

fn options(e: &Engine) -> Json {
    let mut d = json!({"diameter": 24.0, "refineDiameter": 12.0, "view": "alphaBoundary", "overlayColor": [1.0, 0.0, 0.0], "overlayOpacity": 50.0, "boundaryColor": [1.0, 0.0, 0.8], "autoPropagate": true});
    if let (Some(o), Some(s)) = (d.as_object_mut(), e.prefs.get("rotoOptions").and_then(Json::as_object)) {
        for (k, v) in s {
            o.insert(k.clone(), v.clone());
        }
    }
    d
}

pub fn set_options(e: &mut Engine, p: &Json) -> Res {
    let mut cur = e.prefs.get("rotoOptions").and_then(Json::as_object).cloned().unwrap_or_default();
    if let Some(o) = p.as_object() {
        for k in ["diameter", "refineDiameter", "view", "overlayColor", "overlayOpacity", "boundaryColor", "autoPropagate"] {
            if let Some(v) = o.get(k) {
                cur.insert(k.into(), v.clone());
            }
        }
    }
    e.prefs.insert("rotoOptions".into(), Json::Object(cur));
    Ok(options(e))
}

fn start_propagate(e: &mut Engine, cid: u64, lid: u64, uid: u64, dir: &str, wait: bool) -> Res {
    let st = read(e, cid, lid, uid)?;
    let (a, b) = st.span.ok_or("set a segmentation span first (roto.span)")?;
    let mut anchors: Vec<i64> = st.strokes.iter().map(|s| s.f).filter(|f| *f >= a && *f <= b).collect();
    anchors.sort_unstable();
    anchors.dedup();
    if anchors.is_empty() {
        return Err("draw a Roto Paint stroke inside the span first".into());
    }
    need_model(st.model)?;
    let seg = seg_for(e, cid, lid, st.model)?;
    let fwd = dir != "backward";
    let back = dir != "forward";
    jobs::run(e, "roto", lid, "Propagate Roto Paint", wait, move |ctx| {
        let mut results: Vec<(i64, Vec<bool>)> = vec![];
        let mut cur = State { model: st.model, strokes: st.strokes.clone(), span: st.span, mattes: st.mattes.clone(), frozen: st.frozen };
        for (i, &f) in anchors.iter().enumerate() {
            let src = seg.source()?;
            let prior: Option<Vec<f32>> = cur.mattes.as_ref().and_then(|m| m.get(f)).map(|m| m.iter().map(|b| if *b { 1.0 } else { 0.0 }).collect());
            let (m, _) = seg.frame(&src, f, &cur.strokes, prior.as_deref(), None)?;
            results.push((f, m.clone()));
            let mut ms = seg.mattes_for(&src, cur.mattes.as_ref());
            ms.frames.insert(f, roto::encode_rle(&m));
            cur.mattes = Some(ms);
            if fwd {
                let end = anchors.get(i + 1).map(|n| n - 1).unwrap_or(b);
                if end > f {
                    let part = seg.propagate(Some(ctx), &cur, f, end)?;
                    results.extend(part);
                }
            }
            if back {
                let end = if i == 0 { a } else { anchors[i - 1] + 1 };
                if end < f {
                    let part = seg.propagate(Some(ctx), &cur, f, end)?;
                    results.extend(part);
                }
            }
        }
        let n = results.len();
        let apply: Apply = Box::new(move |e: &mut Engine| {
            let mut st = read(e, cid, lid, uid)?;
            let src_dims = st.mattes.clone();
            let mut ms = match src_dims {
                Some(m) => m,
                None => {
                    let seg = seg_for(e, cid, lid, st.model)?;
                    let src = seg.source()?;
                    seg.mattes_for(&src, None)
                }
            };
            for (f, m) in results {
                if m.len() == ms.w * ms.h {
                    ms.frames.insert(f, roto::encode_rle(&m));
                }
            }
            st.mattes = Some(ms);
            write(e, cid, lid, uid, &st)?;
            Ok(json!({"effect": uid, "frames": n}))
        });
        Ok(apply)
    })
}

pub fn stroke(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let kind = match ps(p, "kind").unwrap_or("fg") {
        "fg" => 0u8,
        "bg" => 1,
        "refine" => 2,
        "refineErase" => 3,
        other => return Err(format!("kind is fg, bg, refine or refineErase, not {other}")),
    };
    let pts: Vec<[f32; 2]> = p.get("points").and_then(Json::as_array).ok_or("pass `points`: [[x, y], ...] in layer pixels")?.iter().filter_map(|q| {
        let a = q.as_array()?;
        Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32])
    }).collect();
    if pts.is_empty() {
        return Err("a stroke needs points".into());
    }
    let opts = options(e);
    let radius = pf(p, "radius").unwrap_or_else(|| opts.get(if kind >= 2 { "refineDiameter" } else { "diameter" }).and_then(Json::as_f64).unwrap_or(24.0) / 2.0) as f32;
    let f = current_frame(e, cid, lid, p)?;
    let uid = ensure(e, cid, lid, p)?;
    let mut st = read(e, cid, lid, uid)?;
    need_model(st.model)?;
    let (lo, hi) = frame_range(e, cid, lid)?;
    st.span = Some(match st.span {
        Some((a, b)) => (a.min(f), b.max(f)),
        None => ((f - 20).max(lo), (f + 20).min(hi).max(f)),
    });
    st.strokes.push(Stroke { f, k: kind, r: radius, p: pts });
    let seg = seg_for(e, cid, lid, st.model)?;
    let src = seg.source()?;
    let prior: Option<Vec<f32>> = st.mattes.as_ref().and_then(|m| m.get(f)).map(|m| m.iter().map(|b| if *b { 1.0 } else { 0.0 }).collect());
    let (m, _) = seg.frame(&src, f, &st.strokes, prior.as_deref(), None)?;
    let coverage = m.iter().filter(|b| **b).count() as f64 / m.len().max(1) as f64;
    let mut ms = seg.mattes_for(&src, st.mattes.as_ref());
    ms.frames.insert(f, roto::encode_rle(&m));
    st.mattes = Some(ms);
    write(e, cid, lid, uid, &st)?;
    e.set_label("Roto Paint Stroke");
    let auto = opts.get("autoPropagate").and_then(Json::as_bool).unwrap_or(true) && !pb(p, "noPropagate").unwrap_or(false);
    let job = if auto { start_propagate(e, cid, lid, uid, "both", false).ok() } else { None };
    Ok(json!({"layer": lid, "effect": uid, "frame": f, "coverage": coverage, "span": st.span.map(|(a, b)| [a, b]), "propagating": job.is_some()}))
}

pub fn propagate(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = effect_uid(e, cid, lid, p)?.ok_or("the layer has no Roto Paint")?;
    if let Some(to) = pf(p, "to") {
        let mut st = read(e, cid, lid, uid)?;
        let to = to.round() as i64;
        st.span = st.span.map(|(a, b)| (a.min(to), b.max(to)));
        write(e, cid, lid, uid, &st)?;
    }
    let mut r = start_propagate(e, cid, lid, uid, ps(p, "direction").unwrap_or("both"), pb(p, "wait").unwrap_or(false))?;
    if let Some(o) = r.as_object_mut() {
        o.insert("effect".into(), json!(uid));
    }
    Ok(r)
}

pub fn span(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = ensure(e, cid, lid, p)?;
    let (lo, hi) = frame_range(e, cid, lid)?;
    let mut st = read(e, cid, lid, uid)?;
    let cur = st.span.unwrap_or((lo, hi));
    let a = pf(p, "start").map(|v| v.round() as i64).unwrap_or(cur.0).clamp(lo, hi);
    let b = pf(p, "end").map(|v| v.round() as i64).unwrap_or(cur.1).clamp(lo, hi);
    st.span = Some((a.min(b), a.max(b)));
    write(e, cid, lid, uid, &st)?;
    e.set_label("Set Segmentation Span");
    Ok(json!({"effect": uid, "span": [a.min(b), a.max(b)]}))
}

pub fn freeze(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = effect_uid(e, cid, lid, p)?.ok_or("the layer has no Roto Paint")?;
    let mut st = read(e, cid, lid, uid)?;
    let (a, b) = st.span.ok_or("nothing to freeze")?;
    let missing = (a..=b).any(|f| st.mattes.as_ref().is_none_or(|m| !m.frames.contains_key(&f)));
    st.frozen = true;
    write(e, cid, lid, uid, &st)?;
    e.set_label("Freeze");
    if missing {
        let r = start_propagate(e, cid, lid, uid, "both", pb(p, "wait").unwrap_or(false))?;
        return Ok(json!({"effect": uid, "frozen": true, "analysis": r}));
    }
    Ok(json!({"effect": uid, "frozen": true}))
}

pub fn unfreeze(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = effect_uid(e, cid, lid, p)?.ok_or("the layer has no Roto Paint")?;
    let mut st = read(e, cid, lid, uid)?;
    st.frozen = false;
    write(e, cid, lid, uid, &st)?;
    e.set_label("Unfreeze");
    Ok(json!({"effect": uid, "frozen": false}))
}

pub fn clear_strokes(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let uid = effect_uid(e, cid, lid, p)?.ok_or("the layer has no Roto Paint")?;
    let mut st = read(e, cid, lid, uid)?;
    let frame = pf(p, "frame").map(|v| v.round() as i64);
    let kind = ps(p, "kind").map(|k| match k {
        "bg" => 1u8,
        "refine" => 2,
        "refineErase" => 3,
        _ => 0,
    });
    let before = st.strokes.len();
    st.strokes.retain(|s| !(frame.is_none_or(|f| s.f == f) && kind.is_none_or(|k| s.k == k)));
    let removed = before - st.strokes.len();
    if let Some(m) = st.mattes.as_mut() {
        match frame {
            Some(f) => {
                m.frames.remove(&f);
            }
            None => m.frames.clear(),
        }
    }
    write(e, cid, lid, uid, &st)?;
    e.set_label("Remove Roto Paint Strokes");
    Ok(json!({"effect": uid, "removed": removed}))
}

pub fn cancel(e: &mut Engine, _: &Json) -> Res {
    Ok(json!({"stopped": e.jobs.stop(Some("roto"))}))
}

pub fn status(e: &mut Engine, p: &Json) -> Res {
    let (cid, lid) = e.layer_param(p)?;
    let running = e.jobs.running_for(lid, "roto");
    let progress = e.jobs.progress(lid, "roto");
    let Some(uid) = effect_uid(e, cid, lid, p)? else {
        return Ok(json!({"layer": lid, "effect": null, "running": running, "progress": progress}));
    };
    let st = read(e, cid, lid, uid)?;
    let mut out = json!({
        "layer": lid, "effect": uid, "model": fx::MODELS[st.model.min(3) as usize], "span": st.span.map(|(a, b)| [a, b]),
        "strokes": st.strokes.len(), "frames": st.mattes.as_ref().map(|m| m.frames.len()).unwrap_or(0),
        "frozen": st.frozen, "running": running, "progress": progress,
    });
    if let Some(f) = pf(p, "frame").map(|v| v.round() as i64) {
        let mut m = st.mattes.as_ref().and_then(|ms| ms.get(f));
        let dims = st.mattes.as_ref().map(|ms| (ms.w, ms.h));
        if m.is_none() && pb(p, "compute").unwrap_or(false) {
            let seg = seg_for(e, cid, lid, st.model)?;
            let src = seg.source()?;
            m = Some(seg.frame(&src, f, &st.strokes, None, None)?.0);
        }
        if let Some(m) = m {
            let (w, h) = dims.unwrap_or((0, 0));
            let cov = m.iter().filter(|b| **b).count() as f64 / m.len().max(1) as f64;
            if let Some(o) = out.as_object_mut() {
                o.insert("frame".into(), json!(f));
                o.insert("coverage".into(), json!(cov));
                o.insert("w".into(), json!(w));
                o.insert("h".into(), json!(h));
                if pb(p, "matte").unwrap_or(false) {
                    o.insert("matte".into(), json!(roto::encode_rle(&m)));
                }
                if let Some(other) = ps(p, "compareTo") {
                    let b = roto::decode_rle(other, m.len());
                    o.insert("iou".into(), json!(roto::iou(&m, &b)));
                }
            }
        }
    }
    Ok(out)
}

pub fn models(e: &mut Engine, _: &Json) -> Res {
    let active = active_model(e);
    let entry = |m: &ModelInfo, on: bool| json!({"id": m.id, "name": m.name, "installed": roto::installed_of(m), "active": on, "size": m.size, "licence": m.licence, "url": m.url, "sha256": m.sha256, "path": roto::path_of(m).to_string_lossy()});
    let name = ["classical", "mobilesam", "sam2", "sam2full"][active as usize];
    Ok(json!({"models": [
        {"id": "classical", "name": "Classic (built in)", "installed": true, "active": active == 0, "size": 0},
        entry(&MOBILESAM, active == 1),
        entry(&roto::SAM2, active >= 2),
    ], "active": name, "fullResolution": active == 3, "dir": roto::models_dir().to_string_lossy()}))
}

pub fn model_select(e: &mut Engine, p: &Json) -> Res {
    match ps(p, "id").unwrap_or("classical") {
        "classical" | "classic" => {
            e.prefs.insert("rotoModel".into(), json!("classical"));
            Ok(json!({"active": "classical"}))
        }
        "mobilesam" => {
            if !roto::installed() {
                return Err(NEEDS.into());
            }
            if pb(p, "wait").unwrap_or(false) {
                roto::load_sam()?;
            }
            e.prefs.insert("rotoModel".into(), json!("mobilesam"));
            Ok(json!({"active": "mobilesam"}))
        }
        id @ ("sam2" | "sam2full") => {
            need_model(2)?;
            if pb(p, "wait").unwrap_or(false) {
                roto::load_sam2()?;
            }
            e.prefs.insert("rotoModel".into(), json!(id));
            Ok(json!({"active": id}))
        }
        other => Err(format!("no Roto Paint model {other} (classical, mobilesam, sam2 or sam2full)")),
    }
}

fn which(p: &Json) -> Result<&'static ModelInfo, String> {
    let id = ps(p, "id").unwrap_or(MOBILESAM.id);
    roto::model_info(if id == "sam2full" { "sam2" } else { id }).ok_or_else(|| format!("no downloadable Roto Paint model {id} (mobilesam or sam2)"))
}

fn place(m: &ModelInfo, bytes: &[u8]) -> Result<PathBuf, String> {
    if roto::sha256_hex(bytes) != m.sha256 {
        return Err(format!("that file is not the official {} weights (checksum mismatch)", m.name));
    }
    let dir = roto::models_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot write {}: {e}", dir.display()))?;
    let path = roto::path_of(m);
    std::fs::write(&path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

pub fn model_install(_: &mut Engine, p: &Json) -> Res {
    let m = which(p)?;
    let path = ps(p, "path").ok_or_else(|| format!("pass `path`: the {} file", m.file))?;
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let out = place(m, &bytes)?;
    Ok(json!({"installed": m.id, "path": out.to_string_lossy()}))
}

pub fn model_download(_: &mut Engine, p: &Json) -> Res {
    let m = which(p)?;
    let tmp = std::env::temp_dir().join(format!("formvfx-{}-{}.pt", m.id, std::process::id()));
    let ok = Command::new("curl").args(["-fsSL", "-o"]).arg(&tmp).arg(m.url).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("the download failed; fetch {} yourself and use roto.model.install {{id: {}, path}}", m.url, m.id));
    }
    let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp);
    let out = place(m, &bytes)?;
    Ok(json!({"installed": m.id, "path": out.to_string_lossy()}))
}

pub fn model_remove(e: &mut Engine, p: &Json) -> Res {
    let m = which(p)?;
    let path = roto::path_of(m);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    let a = active_model(e);
    if (m.id == MOBILESAM.id && a == 1) || (m.id == roto::SAM2.id && a >= 2) {
        e.prefs.insert("rotoModel".into(), json!("classical"));
    }
    Ok(json!({"removed": m.id}))
}

pub struct ObjectJob {
    project: Arc<Project>,
    media: Arc<Media>,
    cid: u64,
    lid: u64,
    t: f64,
    point: [f64; 2],
}

pub fn object_job(e: &Engine, p: &Json) -> Result<ObjectJob, String> {
    let (cid, comp) = e.comp_of(p)?;
    let lid = e.layer_in(comp, p.get("layer").ok_or("pass `layer`, the id of a footage layer")?)?;
    let point = p.get("point").and_then(Json::as_array).and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])).ok_or("pass `point`: [x, y] in layer pixels")?;
    if !roto::installed() {
        return Err(NEEDS.into());
    }
    Ok(ObjectJob { project: e.project.clone(), media: e.media.clone(), cid, lid, t: pf(p, "time").unwrap_or_else(|| e.time_of(cid)), point })
}

impl ObjectJob {
    pub fn run(&self) -> Result<Json, String> {
        let model = roto::load_sam()?;
        let src = Source::new(&self.project, &self.media, self.cid, self.lid, SIDE).ok_or("the layer is gone")?;
        let img = src.image(self.t).ok_or("cannot read the layer at that time")?;
        let q = src.to_buf(self.point);
        let pv = roto::preview(&model, &img, q);
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
        let mut n = 0usize;
        for (i, on) in pv.mask.iter().enumerate() {
            if *on {
                n += 1;
                x0 = x0.min(i % pv.w);
                y0 = y0.min(i / pv.w);
                x1 = x1.max(i % pv.w);
                y1 = y1.max(i / pv.w);
            }
        }
        let step = 1.0 / src.scale;
        let bbox = if n > 0 { json!([x0 as f64 * step + src.frame[0], y0 as f64 * step + src.frame[1], (x1 + 1) as f64 * step + src.frame[0], (y1 + 1) as f64 * step + src.frame[1]]) } else { Json::Null };
        Ok(json!({
            "layer": self.lid, "time": self.t, "w": pv.w, "h": pv.h, "step": step, "origin": [src.frame[0], src.frame[1]],
            "runs": roto::runs(&pv.mask), "bbox": bbox, "coverage": n as f64 / pv.mask.len().max(1) as f64, "score": pv.score,
        }))
    }
}
