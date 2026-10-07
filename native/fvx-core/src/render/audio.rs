use super::media::Media;
use crate::effects::{self, audiofx};
use crate::eval::Eval;
use crate::model::{GroupKind, ItemKind, Layer, LayerKind, Project};

pub fn comp_has_audio(project: &Project, comp: u64) -> bool {
    fn walk(p: &Project, id: u64, depth: u32) -> bool {
        if depth > 24 {
            return false;
        }
        let Some(c) = p.comp(id) else { return false };
        c.layers.iter().any(|l| {
            layer_makes_sound(l)
                || match (l.kind, l.source.and_then(|s| p.item(s)).map(|i| &i.kind)) {
                    (LayerKind::Footage, Some(ItemKind::Footage(f))) => f.has_audio,
                    (LayerKind::Precomp, Some(ItemKind::Comp(_))) => walk(p, l.source.unwrap_or(0), depth + 1),
                    _ => false,
                }
        })
    }
    walk(project, comp, 0)
}

pub fn mix_comp(project: &Project, media: &Media, comp: u64, start: f64, frames: usize, rate: u32) -> Vec<f32> {
    let mut out = vec![0.0f32; frames * 2];
    mix_into(project, media, comp, start, rate, &mut out, 0, None);
    out
}

pub fn mix_layer(project: &Project, media: &Media, comp: u64, layer: u64, start: f64, frames: usize, rate: u32) -> Vec<f32> {
    let mut out = vec![0.0f32; frames * 2];
    mix_into(project, media, comp, start, rate, &mut out, 0, Some(layer));
    out
}

#[allow(clippy::too_many_arguments)]
fn mix_into(project: &Project, media: &Media, comp_id: u64, start: f64, rate: u32, out: &mut [f32], depth: u32, only: Option<u64>) {
    if depth > 24 {
        return;
    }
    let Some(comp) = project.comp(comp_id) else { return };
    let frames = out.len() / 2;
    let any_solo = comp.layers.iter().any(|l| l.switches.solo);
    let r = rate as f64;
    for l in &comp.layers {
        if only.is_some_and(|o| o != l.id) || (only.is_none() && (!l.switches.audio || (any_solo && !l.switches.solo))) {
            continue;
        }
        let chain = audio_chain(l);
        let item = l.source.and_then(|s| project.item(s));
        let has_src = match item.map(|i| &i.kind) {
            Some(ItemKind::Footage(f)) => f.has_audio && l.kind == LayerKind::Footage,
            Some(ItemKind::Comp(_)) => l.kind == LayerKind::Precomp,
            _ => false,
        };
        if !has_src && !chain.iter().any(|(_, id)| audiofx::makes_sound(id)) {
            continue;
        }
        if start + frames as f64 / r <= l.in_point || start >= l.out_point {
            continue;
        }
        let ev0 = Eval::new(project, comp_id, comp, start);
        let pre: f64 = chain.iter().map(|(g, id)| audiofx::preroll(id, &effects::params_at(&ev0, l, g))).sum::<f64>().min(8.0);
        let pre_n = (pre * r).ceil() as usize;
        let total = pre_n + frames;
        let t0 = start - pre_n as f64 / r;
        let mut raw = vec![0.0f32; total * 2];
        if has_src {
            let backwards = chain.iter().any(|(_, id)| *id == "fx.audio.backwards");
            let from = if backwards { l.in_point + l.out_point - (t0 + total as f64 / r) } else { t0 };
            layer_source(project, media, comp_id, l, from, rate, &mut raw, depth);
            if backwards {
                let frames_raw: Vec<[f32; 2]> = raw.chunks_exact(2).rev().map(|s| [s[0], s[1]]).collect();
                for (d, s) in raw.chunks_exact_mut(2).zip(frames_raw) {
                    d.copy_from_slice(&s);
                }
            }
        }
        let mut procs: Vec<(audiofx::Proc, &crate::model::Group)> = chain.iter().filter_map(|(g, id)| audiofx::Proc::new(id, rate).map(|p| (p, *g))).collect();
        let step = 256usize;
        let mut i = 0;
        while i < total && !procs.is_empty() {
            let len = step.min(total - i);
            let ct = t0 + i as f64 / r;
            let ev = Eval::new(project, comp_id, comp, ct);
            for (pr, g) in procs.iter_mut() {
                pr.block(&effects::params_at(&ev, l, g), &mut raw[i * 2..(i + len) * 2], l.layer_time(ct));
            }
            i += len;
        }
        let block = 1024usize;
        let mut i = 0;
        while i < frames {
            let len = block.min(frames - i);
            let ev = Eval::new(project, comp_id, comp, start + i as f64 / r);
            let lv = l.props.sub("audio").and_then(|g| g.get("levels")).map(|p| ev.value(l, p).vec2()).unwrap_or([0.0, 0.0]);
            let (gl, gr) = (10f32.powf(lv[0] as f32 / 20.0), 10f32.powf(lv[1] as f32 / 20.0));
            for k in i..i + len {
                if !l.is_active_at(start + k as f64 / r) {
                    continue;
                }
                out[k * 2] += raw[(pre_n + k) * 2] * gl;
                out[k * 2 + 1] += raw[(pre_n + k) * 2 + 1] * gr;
            }
            i += len;
        }
    }
}

fn audio_chain(l: &Layer) -> Vec<(&crate::model::Group, &'static str)> {
    if !l.switches.effects {
        return vec![];
    }
    l.effects()
        .map(|g| {
            g.groups()
                .filter(|e| e.enabled)
                .filter_map(|e| match &e.kind {
                    GroupKind::Effect { effect } if audiofx::is_audio(effect) => effects::find(effect).map(|s| (e, s.id)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn layer_makes_sound(l: &Layer) -> bool {
    audio_chain(l).iter().any(|(_, id)| audiofx::makes_sound(id))
}

#[allow(clippy::too_many_arguments)]
fn layer_source(project: &Project, media: &Media, comp_id: u64, l: &Layer, start: f64, rate: u32, out: &mut [f32], depth: u32) {
    let Some(comp) = project.comp(comp_id) else { return };
    let Some(item) = l.source.and_then(|s| project.item(s)) else { return };
    let frames = out.len() / 2;
    let r = rate as f64;
    match &item.kind {
        ItemKind::Footage(f) if f.has_audio => {
            let Some(pcm) = media.audio(&f.path, rate) else { return };
            let n = pcm.len() / 2;
            let block = 1024usize;
            let mut i = 0;
            while i < frames {
                let len = block.min(frames - i);
                let ct = start + i as f64 / r;
                let remap = l.props.get("timeRemap").map(|_| (Eval::new(project, comp_id, comp, ct).source_time(l), Eval::new(project, comp_id, comp, ct + len as f64 / r).source_time(l)));
                for k in 0..len {
                    let t = start + (i + k) as f64 / r;
                    if !l.is_active_at(t) {
                        continue;
                    }
                    let lt = match remap {
                        Some((a, b)) => a + (b - a) * k as f64 / len as f64,
                        None => l.layer_time(t),
                    };
                    if lt < 0.0 {
                        continue;
                    }
                    let si = (lt * r) as usize;
                    if si >= n {
                        continue;
                    }
                    out[(i + k) * 2] = pcm[si * 2];
                    out[(i + k) * 2 + 1] = pcm[si * 2 + 1];
                }
                i += len;
            }
        }
        ItemKind::Comp(_) => {
            let t1 = start + frames as f64 / r;
            if (l.stretch - 100.0).abs() < 1e-9 {
                mix_into(project, media, item.id, l.layer_time(start), rate, out, depth + 1, None);
            } else {
                let need = ((frames as f64) * (100.0 / l.stretch).abs()).ceil() as usize + 2;
                let mut raw = vec![0.0f32; need * 2];
                let lt0 = l.layer_time(start).min(l.layer_time(t1));
                mix_into(project, media, item.id, lt0, rate, &mut raw, depth + 1, None);
                for k in 0..frames {
                    let lt = l.layer_time(start + k as f64 / r);
                    let si = ((lt - lt0) * r) as usize;
                    if si < need {
                        out[k * 2] = raw[si * 2];
                        out[k * 2 + 1] = raw[si * 2 + 1];
                    }
                }
            }
        }
        _ => {}
    }
}

pub fn peak_summary(buf: &[f32], bin: usize) -> Vec<[f32; 4]> {
    let frames = buf.len() / 2;
    let mut out = Vec::with_capacity(frames / bin.max(1) + 1);
    let mut i = 0;
    while i < frames {
        let n = bin.min(frames - i);
        let mut p = [f32::MAX, f32::MIN, f32::MAX, f32::MIN];
        for k in 0..n {
            let l = buf[(i + k) * 2];
            let r = buf[(i + k) * 2 + 1];
            p[0] = p[0].min(l);
            p[1] = p[1].max(l);
            p[2] = p[2].min(r);
            p[3] = p[3].max(r);
        }
        out.push(p);
        i += n;
    }
    out
}
