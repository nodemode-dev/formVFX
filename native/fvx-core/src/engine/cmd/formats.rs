use std::path::Path;

use serde_json::{Value as Json, json};

use super::comp::make_comp;
use super::{add_layer, footage_layer};
use crate::engine::{Engine, Res, pb, pf, ps};
use crate::model::{AlphaMode, BlendMode, Footage, FootageKind, Item, ItemKind, LayerKind, Proxy, Value};
use crate::render::{RenderOpts, Renderer, exr, media, pdf, psd};

pub const STILL_SECONDS: f64 = 10.0;
pub const IMPORT_RATE: f64 = 30.0;

pub fn import_as(p: &Json) -> Result<Option<bool>, String> {
    match ps(p, "importAs").unwrap_or("footage") {
        "footage" => Ok(None),
        "composition" | "comp" => Ok(Some(false)),
        "compositionLayerSizes" | "compositionRetainLayerSizes" | "layerSizes" => Ok(Some(true)),
        other => Err(format!("importAs: footage|composition|compositionLayerSizes, not `{other}`")),
    }
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path).to_string()
}

fn stem(path: &str) -> String {
    Path::new(path).file_stem().and_then(|n| n.to_str()).unwrap_or("Imported").to_string()
}

fn push_item(e: &mut Engine, name: &str, parent: Option<u64>, label: u8, kind: ItemKind) -> u64 {
    let pj = e.proj();
    let id = pj.new_id();
    pj.items.push(Item { id, name: name.to_string(), parent, label, comment: String::new(), kind, proxy: None });
    id
}

pub fn label_of(f: &Footage) -> u8 {
    match f.kind {
        FootageKind::Image | FootageKind::Sequence | FootageKind::Vector => 5,
        _ => 3,
    }
}

fn seq_name(files: &[String]) -> String {
    let first = Path::new(&files[0]);
    let st = first.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let digits = st.bytes().rev().take_while(u8::is_ascii_digit).count();
    let prefix = &st[..st.len() - digits];
    let num = |f: &String| -> String {
        let s = Path::new(f).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let d = s.bytes().rev().take_while(u8::is_ascii_digit).count();
        s[s.len() - d..].to_string()
    };
    let ext = first.extension().and_then(|e| e.to_str()).unwrap_or("");
    format!("{prefix}[{}-{}].{ext}", num(&files[0]), num(&files[files.len() - 1]))
}

pub fn probe(path: &str, p: &Json, warnings: &mut Vec<String>) -> Result<(Footage, String), String> {
    let mut f = media::probe(path)?;
    let mut name = file_name(path);
    if let Some(sel) = p.get("layer").filter(|v| !v.is_null() && v.as_str().is_none_or(|s| !s.trim().is_empty())) {
        match f.codec.as_str() {
            "PSD" => {
                let d = psd::open(path)?;
                let i = d.find(sel).ok_or_else(|| format!("{path}: no PSD layer {sel}"))?;
                let l = &d.layers[i];
                f.layer = Some(i as u32);
                f.layer_name = l.name.clone();
                name = format!("{}/{name}", l.name);
            }
            "EXR" => {
                let want = sel.as_str().map(str::to_string).unwrap_or_else(|| sel.to_string());
                let info = exr::info(path)?;
                if !info.layers.contains(&want) {
                    return Err(format!("{path}: no EXR layer `{want}` ({})", info.layers.iter().map(|l| if l.is_empty() { "(composite)" } else { l.as_str() }).collect::<Vec<_>>().join(", ")));
                }
                f.layer_name = want.clone();
                name = format!("{want}/{name}");
            }
            _ => {}
        }
    }
    if let Some(page) = p.get("page").and_then(|v| v.as_u64().or_else(|| v.as_str()?.trim().parse().ok()))
        && pdf::EXT.contains(&f.codec.to_ascii_lowercase().as_str())
    {
        let pages = pdf::pages(path)?;
        let k = (page.max(1) - 1) as usize;
        let (w, h) = *pages.get(k).ok_or_else(|| format!("{path}: no page {page} (it has {})", pages.len()))?;
        f.page = k as u32;
        f.width = w.round().max(1.0) as u32;
        f.height = h.round().max(1.0) as u32;
        if k > 0 {
            name = format!("{} (Page {page})", name);
        }
    }
    if f.kind == FootageKind::Image && f.layer.is_none() && pb(p, "sequence") != Some(false) {
        let files = media::sequence_files(path);
        if files.len() >= 2 {
            let fps = pf(p, "sequenceFrameRate").filter(|r| *r > 0.0).unwrap_or(IMPORT_RATE);
            if let Some(gap) = media::missing_frames(&files) {
                warnings.push(format!("{}: {gap}", seq_name(&files)));
            }
            name = seq_name(&files);
            f.kind = FootageKind::Sequence;
            f.frame_rate = fps;
            f.duration = files.len() as f64 / fps;
            f.sequence = files;
        }
    }
    Ok((f, name))
}

fn set_opacity(l: &mut crate::model::Layer, v: f64) {
    if let Some(pr) = l.props.sub_mut("transform").and_then(|t| t.get_mut("opacity")) {
        pr.keys.clear();
        pr.expr = None;
        pr.value = Value::Num(v);
    }
}

fn place(e: &mut Engine, cid: u64, item: u64, pos: Option<[f64; 2]>, name: &str, blend: BlendMode, opacity: f64, visible: bool, clip: bool) -> Result<u64, String> {
    let mut l = footage_layer(e, cid, item, 0.0, pos, Some(STILL_SECONDS))?;
    l.name = name.to_string();
    l.blend = blend;
    l.switches.video = visible;
    l.preserve_transparency = clip;
    set_opacity(&mut l, opacity);
    Ok(add_layer(e, cid, l, Some(0)))
}

fn build_psd(e: &mut Engine, doc: &psd::Doc, path: &str, group: Option<usize>, cid: u64, folder: u64, retain: bool, items: &mut Vec<u64>) -> Result<(), String> {
    let fname = file_name(path);
    let kids: Vec<usize> = (0..doc.layers.len()).filter(|i| doc.layers[*i].parent == group && doc.layers[*i].kind != psd::Kind::End).collect();
    for i in kids {
        let l = doc.layers[i].clone();
        let blend = psd::blend_of(&l.blend);
        let opacity = l.opacity as f64 * 100.0 / 255.0;
        if l.kind == psd::Kind::Group {
            let sub = make_comp(e, &l.name, doc.width as u32, doc.height as u32, IMPORT_RATE, STILL_SECONDS, [0.0; 3]);
            if let Some(it) = e.proj().item_mut(sub) {
                it.parent = Some(folder);
            }
            items.push(sub);
            build_psd(e, doc, path, Some(i), sub, folder, retain, items)?;
            place(e, cid, sub, None, &l.name, blend, opacity, l.visible, l.clipping)?;
            continue;
        }
        let (w, h) = ((l.right - l.left).max(0) as u32, (l.bottom - l.top).max(0) as u32);
        if w == 0 || h == 0 {
            continue;
        }
        let f = Footage {
            path: path.to_string(),
            kind: FootageKind::Image,
            width: if retain { w } else { doc.width as u32 },
            height: if retain { h } else { doc.height as u32 },
            has_video: true,
            codec: "PSD".into(),
            layer: Some(i as u32),
            layer_name: l.name.clone(),
            layer_size: retain,
            ..Footage::default()
        };
        let id = push_item(e, &format!("{}/{fname}", l.name), Some(folder), 5, ItemKind::Footage(f));
        items.push(id);
        let pos = retain.then(|| [l.left as f64 + w as f64 / 2.0, l.top as f64 + h as f64 / 2.0]);
        place(e, cid, id, pos, &l.name, blend, opacity, l.visible, l.clipping)?;
    }
    Ok(())
}

pub fn import_psd_comp(e: &mut Engine, path: &str, retain: bool, folder: Option<u64>) -> Result<(u64, Vec<u64>), String> {
    let doc = psd::open(path)?;
    let name = stem(path);
    let fid = push_item(e, &format!("{name} Layers"), folder, 0, ItemKind::Folder);
    let cid = make_comp(e, &name, doc.width.max(1) as u32, doc.height.max(1) as u32, IMPORT_RATE, STILL_SECONDS, [0.0; 3]);
    if let Some(it) = e.proj().item_mut(cid) {
        it.parent = folder;
    }
    let mut items = vec![fid, cid];
    build_psd(e, &doc, path, None, cid, fid, retain, &mut items)?;
    Ok((cid, items))
}

pub fn import_pdf_comp(e: &mut Engine, path: &str, p: &Json, folder: Option<u64>) -> Result<(u64, Vec<u64>), String> {
    let mut w = vec![];
    let (f, name) = probe(path, p, &mut w)?;
    let page = f.page + 1;
    let cname = if page > 1 { format!("{} (Page {page})", stem(path)) } else { stem(path) };
    let fid = push_item(e, &format!("{cname} Layers"), folder, 0, ItemKind::Folder);
    let (fw, fh) = (f.width, f.height);
    let item = push_item(e, &name, Some(fid), 5, ItemKind::Footage(f));
    let cid = make_comp(e, &cname, fw, fh, IMPORT_RATE, STILL_SECONDS, [0.0; 3]);
    if let Some(it) = e.proj().item_mut(cid) {
        it.parent = folder;
    }
    place(e, cid, item, None, &name, BlendMode::Normal, 100.0, true, false)?;
    Ok((cid, vec![fid, item, cid]))
}

fn targets(e: &Engine, p: &Json) -> Result<Vec<u64>, String> {
    let mut ids: Vec<u64> = match (p.get("items"), p.get("item")) {
        (Some(Json::Array(a)), _) => a.iter().filter_map(Json::as_u64).collect(),
        (_, Some(v)) if v.is_u64() => vec![v.as_u64().unwrap_or(0)],
        _ => e.state.project_selection.clone(),
    };
    ids.retain(|i| e.project.item(*i).is_some_and(|it| matches!(it.kind, ItemKind::Footage(_) | ItemKind::Comp(_))));
    if ids.is_empty() {
        return Err("select footage or compositions in the Project panel, or pass `items`".into());
    }
    Ok(ids)
}

pub fn set_proxy(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (a still, sequence or movie)")?.to_string();
    let ids = targets(e, p)?;
    let mut w = vec![];
    let (f, _) = probe(&path, &json!({"sequence": pb(p, "sequence").unwrap_or(true)}), &mut w)?;
    for id in &ids {
        if let Some(it) = e.proj().item_mut(*id) {
            it.proxy = Some(Proxy { footage: f.clone(), enabled: true });
        }
    }
    e.media.forget(&path);
    e.set_label("Set Proxy");
    Ok(json!({"items": ids, "proxy": path, "size": [f.width, f.height], "warnings": w}))
}

pub fn set_proxy_none(e: &mut Engine, p: &Json) -> Res {
    let ids = targets(e, p)?;
    for id in &ids {
        if let Some(it) = e.proj().item_mut(*id) {
            it.proxy = None;
        }
    }
    e.set_label("Set Proxy None");
    Ok(json!({"items": ids}))
}

pub fn use_proxy(e: &mut Engine, p: &Json) -> Res {
    let ids = targets(e, p)?;
    let on = pb(p, "on");
    let mut out = vec![];
    for id in &ids {
        if let Some(px) = e.proj().item_mut(*id).and_then(|it| it.proxy.as_mut()) {
            px.enabled = on.unwrap_or(!px.enabled);
            out.push(json!({"item": id, "enabled": px.enabled}));
        }
    }
    if out.is_empty() {
        return Err("none of these items has a proxy (file.setProxy)".into());
    }
    e.set_label("Use Proxy");
    Ok(json!({"items": out}))
}

pub fn interpret_proxy(e: &mut Engine, p: &Json) -> Res {
    let ids = targets(e, p)?;
    if p.as_object().is_none_or(|o| o.keys().all(|k| k == "items" || k == "item")) {
        return Err("Interpret Proxy needs a setting: alpha, frameRate, loop, pixelAspect, colorProfile or preserveRgb".into());
    }
    let mut n = 0;
    for id in &ids {
        let Some(px) = e.proj().item_mut(*id).and_then(|it| it.proxy.as_mut()) else { continue };
        let f = &mut px.footage;
        if let Some(a) = ps(p, "alpha") {
            f.alpha = match a {
                "premultiplied" => AlphaMode::Premultiplied,
                "ignore" => AlphaMode::Ignore,
                _ => AlphaMode::Straight,
            };
        }
        if let Some(r) = pf(p, "frameRate").filter(|r| *r > 0.0) {
            if f.kind == FootageKind::Sequence {
                f.duration = f.sequence.len() as f64 / r;
            }
            f.frame_rate = r;
        }
        if let Some(l) = p.get("loop").or(p.get("loops")).and_then(Json::as_u64) {
            f.loops = l.clamp(1, 9999) as u32;
        }
        if let Some(pa) = pf(p, "pixelAspect").filter(|x| *x > 0.0) {
            f.pixel_aspect = pa;
        }
        if let Some(c) = ps(p, "colorProfile") {
            f.color_profile = if c == "auto" { String::new() } else { c.to_string() };
        }
        if let Some(b) = pb(p, "preserveRgb") {
            f.preserve_rgb = b;
        }
        n += 1;
    }
    if n == 0 {
        return Err("the selected items need a proxy first (file.setProxy)".into());
    }
    e.set_label("Interpret Proxy");
    Ok(json!({"items": ids}))
}

fn proxy_path(name: &str, ext: &str) -> String {
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into());
    let d = std::env::var("FORMVFX_RENDERS").map(std::path::PathBuf::from).unwrap_or_else(|_| Path::new(&home).join("Formvfx").join("Renders")).join("Proxies");
    let _ = std::fs::create_dir_all(&d);
    d.join(format!("{safe}_proxy.{ext}")).to_string_lossy().to_string()
}

pub fn create_proxy(e: &mut Engine, p: &Json) -> Res {
    let kind = ps(p, "kind").unwrap_or("still");
    let id = match p.get("comp").or(p.get("item")) {
        Some(v) if !v.is_null() => {
            if let Some(n) = v.as_u64() {
                n
            } else {
                e.comp_id(Some(v))?
            }
        }
        _ => targets(e, p)?.first().copied().ok_or("select a composition or footage item")?,
    };
    let res = pf(p, "resolution").unwrap_or(0.5).clamp(0.05, 1.0);
    let it = e.project.item(id).ok_or("no such item")?.clone();
    let out = ps(p, "path").or(ps(p, "output")).map(str::to_string);
    match (&it.kind, kind) {
        (ItemKind::Comp(_), "movie") => {
            let output = out.unwrap_or_else(|| proxy_path(&it.name, "mov"));
            let r = crate::engine::export::add(e, &json!({"comp": id, "format": "prores", "proresProfile": "4444", "channels": "rgba", "output": output, "resolution": res, "postRenderAction": "setProxy"}))?;
            e.set_label("Create Proxy");
            Ok(json!({"queued": r["item"], "output": output, "note": "render the queue (renderQueue.render); the proxy is set when it finishes"}))
        }
        (ItemKind::Comp(c), _) => {
            let output = out.unwrap_or_else(|| proxy_path(&it.name, "png"));
            let t = pf(p, "time").unwrap_or_else(|| e.time_of(id));
            let r = Renderer::new(&e.project, &e.media, RenderOpts { scale: res, guides: false, ..RenderOpts::default() });
            let img = r.comp_frame(id, t.clamp(0.0, c.duration));
            write_png(&r, &img, &output)?;
            set_proxy(e, &json!({"items": [id], "path": output, "sequence": false}))
        }
        (ItemKind::Footage(f), _) => {
            let video = matches!(f.kind, FootageKind::Video);
            let (w, h) = (((f.width as f64 * res).round() as u32).max(1), ((f.height as f64 * res).round() as u32).max(1));
            if video && kind == "movie" {
                let output = out.unwrap_or_else(|| proxy_path(&it.name, "mov"));
                let st = std::process::Command::new(media::tool("ffmpeg"))
                    .args(["-y", "-v", "error", "-i", &f.path, "-vf", &format!("scale={w}:{h}"), "-c:v", "prores_ks", "-profile:v", "0", "-c:a", "pcm_s16le", &output])
                    .status()
                    .map_err(|_| "a movie proxy needs FFmpeg on PATH".to_string())?;
                if !st.success() {
                    return Err("FFmpeg could not write the proxy".into());
                }
                return set_proxy(e, &json!({"items": [id], "path": output, "sequence": false}));
            }
            let output = out.unwrap_or_else(|| proxy_path(&it.name, "png"));
            let t = pf(p, "time").unwrap_or(0.0);
            let src = e.media.frame(f, t).ok_or("the footage has no frame to make a proxy from")?;
            let small = scale_img(&src, w as usize, h as usize);
            let buf = image::RgbaImage::from_raw(w, h, small.to_rgba8()).ok_or("proxy frame")?;
            buf.save(&output).map_err(|err| format!("{output}: {err}"))?;
            set_proxy(e, &json!({"items": [id], "path": output, "sequence": false}))
        }
        _ => Err("Create Proxy works on compositions and footage".into()),
    }
}

fn scale_img(src: &crate::render::Img, w: usize, h: usize) -> crate::render::Img {
    let mut out = crate::render::Img::new(w, h);
    let (kx, ky) = (src.w as f64 / w as f64, src.h as f64 / h as f64);
    for y in 0..h {
        for x in 0..w {
            let (x0, y0) = ((x as f64 * kx) as usize, (y as f64 * ky) as usize);
            let (x1, y1) = (((x + 1) as f64 * kx).ceil() as usize, ((y + 1) as f64 * ky).ceil() as usize);
            let mut acc = [0.0f32; 4];
            let mut n = 0.0;
            for yy in y0..y1.min(src.h).max(y0 + 1) {
                for xx in x0..x1.min(src.w).max(x0 + 1) {
                    let i = (yy.min(src.h - 1) * src.w + xx.min(src.w - 1)) * 4;
                    for k in 0..4 {
                        acc[k] += src.px[i + k];
                    }
                    n += 1.0;
                }
            }
            let o = (y * w + x) * 4;
            for k in 0..4 {
                out.px[o + k] = acc[k] / n;
            }
        }
    }
    out
}

fn write_png(r: &Renderer, img: &crate::render::Img, path: &str) -> Result<(), String> {
    let mut shown = img.clone();
    if let Some(pr) = r.pipe.output(&r.project.settings.output_space)? {
        pr.apply_img(&mut shown);
    }
    if let Some(dir) = Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let buf = image::RgbaImage::from_raw(shown.w as u32, shown.h as u32, shown.to_rgba8()).ok_or("frame")?;
    buf.save(path).map_err(|err| format!("{path}: {err}"))
}

struct Frame {
    merged: crate::render::Img,
    layers: Vec<(String, crate::render::Img, BlendMode, f32, bool)>,
}

fn layered(e: &Engine, p: &Json) -> Result<(Frame, std::sync::Arc<crate::model::Project>, u64), String> {
    let cid = e.comp_id(p.get("comp"))?;
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let scale = pf(p, "scale").unwrap_or(1.0).clamp(0.05, 4.0);
    let project = e.project.clone();
    let comp = project.comp(cid).ok_or("no composition")?;
    let opts = RenderOpts { scale, guides: false, ..RenderOpts::default() };
    let merged = Renderer::new(&project, &e.media, opts.clone()).comp_frame(cid, t);
    let ev = crate::eval::Eval::new(&project, cid, comp, t);
    let mut layers = vec![];
    let flat = pb(p, "flat").unwrap_or(false);
    for l in comp.layers.iter().rev() {
        if flat || !l.switches.video || !l.is_active_at(t) || l.switches.guide || l.switches.adjustment || matches!(l.kind, LayerKind::Camera | LayerKind::Light | LayerKind::Null) {
            continue;
        }
        let opacity = ev.opacity(l) as f32;
        let mut pj = (*project).clone();
        let Some(c) = pj.comp_mut(cid) else { continue };
        for x in c.layers.iter_mut() {
            x.switches.solo = x.id == l.id;
            if x.id == l.id {
                x.blend = BlendMode::Normal;
                set_opacity(x, 100.0);
            }
        }
        let img = Renderer::new(&pj, &e.media, opts.clone()).comp_frame(cid, t);
        layers.push((l.name.clone(), img, l.blend, opacity, true));
    }
    Ok((Frame { merged, layers }, project, cid))
}

pub fn save_frame_psd(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (.psd)")?.to_string();
    let (mut fr, project, _) = layered(e, p)?;
    let r = Renderer::new(&project, &e.media, RenderOpts::default());
    if let Some(pr) = r.pipe.output(&project.settings.output_space)? {
        pr.apply_img(&mut fr.merged);
        for l in fr.layers.iter_mut() {
            pr.apply_img(&mut l.1);
        }
    }
    let out: Vec<psd::OutLayer> = fr.layers.iter().map(|l| psd::OutLayer { name: l.0.clone(), img: &l.1, blend: l.2, opacity: l.3, visible: l.4 }).collect();
    if let Some(dir) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    psd::write(&path, &fr.merged, &out)?;
    Ok(json!({"path": path, "width": fr.merged.w, "height": fr.merged.h, "layers": fr.layers.iter().map(|l| l.0.clone()).collect::<Vec<_>>()}))
}

pub fn save_frame_exr(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (.exr)")?.to_string();
    let (mut fr, project, _) = layered(e, p)?;
    let r = Renderer::new(&project, &e.media, RenderOpts::default());
    if let Some(pr) = r.pipe.scene_linear() {
        pr.apply_img(&mut fr.merged);
        for l in fr.layers.iter_mut() {
            pr.apply_img(&mut l.1);
        }
    }
    let mut names: Vec<String> = vec![];
    let mut parts: Vec<(String, &crate::render::Img)> = vec![(String::new(), &fr.merged)];
    for l in fr.layers.iter().rev() {
        let base: String = l.0.chars().map(|c| if c == '.' { '_' } else { c }).collect();
        let mut n = base.clone();
        let mut k = 2;
        while names.contains(&n) {
            n = format!("{base} {k}");
            k += 1;
        }
        names.push(n.clone());
        parts.push((n, &l.1));
    }
    if let Some(dir) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    exr::write(&path, &parts, !pb(p, "float").unwrap_or(false))?;
    Ok(json!({"path": path, "width": fr.merged.w, "height": fr.merged.h, "layers": names}))
}
