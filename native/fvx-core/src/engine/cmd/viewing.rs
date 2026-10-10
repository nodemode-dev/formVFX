use serde_json::{Value as Json, json};

use crate::engine::state::FrameJob;
use crate::engine::{Engine, Res, pb, pf, ps, pu};

const FAST: [&str; 5] = ["off", "adaptive", "draft", "fastDraft", "wireframe"];

pub fn exposure(e: &mut Engine, p: &Json) -> Res {
    let v = match (pf(p, "value"), pf(p, "delta")) {
        (Some(v), _) => v,
        (None, Some(d)) => e.state.viewer.exposure + d,
        _ => return Err("pass `value` (stops) or `delta`".into()),
    };
    e.state.viewer.exposure = v.clamp(-40.0, 40.0);
    e.revision += 1;
    Ok(json!({"exposure": e.state.viewer.exposure}))
}

pub fn custom_rgb(e: &mut Engine, p: &Json) -> Res {
    let c = &mut e.state.viewer.custom_rgb;
    for (i, k) in ["red", "green", "blue"].iter().enumerate() {
        if let Some(b) = pb(p, k) {
            c[i] = b;
        }
    }
    let rgb = *c;
    if pb(p, "show").unwrap_or(true) {
        e.state.viewer.channel = "Custom".into();
    }
    e.revision += 1;
    Ok(json!({"channel": e.state.viewer.channel, "red": rgb[0], "green": rgb[1], "blue": rgb[2]}))
}

pub fn display_color(e: &mut Engine, _: &Json) -> Res {
    let v = &e.state.viewer;
    let s = &e.project.settings;
    Ok(json!({
        "channel": v.channel, "customRgb": {"red": v.custom_rgb[0], "green": v.custom_rgb[1], "blue": v.custom_rgb[2]}, "exposure": v.exposure,
        "displayColorManagement": v.display_cm, "simulateOutput": v.simulate, "preserveRgb": v.preserve_rgb, "fastPreviews": v.fast_previews,
        "workingSpace": s.working_space, "colorEngine": s.color_engine, "bitDepth": s.bit_depth,
    }))
}

pub fn extended_viewer(e: &mut Engine, p: &Json) -> Res {
    let on = pb(p, "value").unwrap_or(!e.state.viewer.extended);
    e.state.viewer.extended = on;
    e.frontend("view.extendedViewer", json!({"open": on}));
    Ok(json!({"extended": on}))
}

pub fn fast_preview_mode(e: &mut Engine, p: &Json) -> Res {
    let m = ps(p, "mode").ok_or("pass `mode` (off, adaptive, draft, fastDraft or wireframe)")?;
    let m = FAST.iter().find(|x| x.eq_ignore_ascii_case(m)).ok_or_else(|| format!("`{m}` is not a Fast Previews mode (off, adaptive, draft, fastDraft, wireframe)"))?;
    e.state.viewer.fast_previews = m.to_string();
    e.revision += 1;
    Ok(json!({"mode": m}))
}

fn guide_index(e: &Engine, cid: u64, p: &Json) -> Result<usize, String> {
    let n = e.project.comp(cid).map(|c| c.guides.len()).unwrap_or(0);
    let i = pu(p, "index").ok_or("pass `index` (0-based, as view.guides lists them)")? as usize;
    if i >= n {
        return Err(format!("there are {n} guides"));
    }
    Ok(i)
}

pub fn move_guide(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let i = guide_index(e, cid, p)?;
    let pos = pf(p, "position").ok_or("pass `position` (px)")?;
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    c.guides[i].position = pos;
    let g = &c.guides[i];
    let out = json!({"index": i, "vertical": g.vertical, "position": g.position});
    e.set_label("Move Guide");
    Ok(out)
}

pub fn remove_guide(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let i = guide_index(e, cid, p)?;
    let c = e.proj().comp_mut(cid).ok_or("no composition")?;
    c.guides.remove(i);
    let n = c.guides.len();
    e.set_label("Remove Guide");
    Ok(json!({"guides": n}))
}

pub fn share_view_options(e: &mut Engine, p: &Json) -> Res {
    let on = pb(p, "value").unwrap_or(!e.state.viewer.share_view_options);
    e.state.viewer.share_view_options = on;
    e.revision += 1;
    Ok(json!({"shareViewOptions": on}))
}

fn assign(e: &mut Engine, key: &str, command: &str, params: Json) {
    for v in e.shortcuts.values_mut() {
        v.retain(|k| k != key);
    }
    e.session.param_shortcuts.retain(|x| x.0 != key);
    e.session.param_shortcuts.push((key.to_string(), command.to_string(), params));
}

pub fn assign_3d_shortcut(e: &mut Engine, p: &Json) -> Res {
    let view = ps(p, "view").ok_or("pass `view` (front, left, top, back, right, bottom, custom1, custom2, custom3, activeCamera)")?;
    let view = super::view3d::normalize(view).ok_or_else(|| format!("`{view}` is not a 3D view"))?;
    let key = ps(p, "key").unwrap_or("F10");
    if !matches!(key, "F10" | "F11" | "F12") {
        return Err("3D views take F10, F11 or F12".into());
    }
    let id = format!("view.3d.{view}");
    if crate::engine::registry::find(&id).is_none() {
        return Err(format!("no command for the {view} view"));
    }
    let defaults = |c: &crate::engine::registry::Cmd| -> Vec<String> { if c.key.is_empty() { vec![] } else { c.key.split(" or ").map(str::to_string).collect() } };
    for c in crate::engine::registry::all().iter().filter(|c| c.id.starts_with("view.3d.") && c.id != id) {
        let keys = e.shortcuts.get(c.id).cloned().unwrap_or_else(|| defaults(c));
        if keys.iter().any(|k| k == key) {
            e.shortcuts.insert(c.id.to_string(), keys.into_iter().filter(|k| k != key).collect());
        }
    }
    let target = crate::engine::registry::find(&id).ok_or("no such view")?;
    let mut keys = e.shortcuts.get(&id).cloned().unwrap_or_else(|| defaults(target));
    if !keys.iter().any(|k| k == key) {
        keys.push(key.to_string());
    }
    e.shortcuts.insert(id.clone(), keys);
    e.session.param_shortcuts.retain(|x| x.0 != key);
    Ok(json!({"key": key, "command": id}))
}

pub fn assign_workspace_shortcut(e: &mut Engine, p: &Json) -> Res {
    let name = ps(p, "name").ok_or("pass `name` (a workspace)")?.to_string();
    let key = ps(p, "key").unwrap_or("Shift+F10");
    if !matches!(key, "Shift+F10" | "Shift+F11" | "Shift+F12") {
        return Err("workspaces take Shift+F10, Shift+F11 or Shift+F12".into());
    }
    assign(e, key, "window.workspace", json!({"name": name}));
    Ok(json!({"key": key, "workspace": name}))
}

pub fn scrub_audio(e: &mut Engine, p: &Json) -> Res {
    let on = pb(p, "value").unwrap_or(!e.state.viewer.scrub_audio);
    e.state.viewer.scrub_audio = on;
    e.revision += 1;
    Ok(json!({"scrubAudio": on}))
}

pub fn playback_get(e: &mut Engine, _: &Json) -> Res {
    let mut s = e.state.viewer.playback.clone();
    s["scrubAudio"] = json!(e.state.viewer.scrub_audio);
    s["fastPreviews"] = json!(e.state.viewer.fast_previews);
    Ok(s)
}

pub fn playback_set(e: &mut Engine, p: &Json) -> Res {
    let o = p.as_object().ok_or("pass the settings to change")?;
    for (k, v) in o {
        match (k.as_str(), v) {
            ("includeAudio" | "loop" | "cacheBeforePlayback" | "fullScreen", Json::Bool(_)) => e.state.viewer.playback[k] = v.clone(),
            ("range", Json::String(s)) if matches!(s.as_str(), "workArea" | "workAreaExtended" | "comp" | "aroundCurrent" | "playFromCurrent") => e.state.viewer.playback[k] = v.clone(),
            ("resolution", Json::String(s)) if matches!(s.as_str(), "auto" | "full" | "half" | "third" | "quarter") => e.state.viewer.playback[k] = v.clone(),
            ("frameRate", Json::String(s)) if s == "auto" => e.state.viewer.playback[k] = v.clone(),
            ("frameRate", Json::Number(n)) if n.as_f64().is_some_and(|x| x > 0.0 && x <= 240.0) => e.state.viewer.playback[k] = v.clone(),
            ("skip", Json::Number(n)) if n.as_u64().is_some_and(|x| x <= 30) => e.state.viewer.playback[k] = v.clone(),
            ("scrubAudio", Json::Bool(b)) => e.state.viewer.scrub_audio = *b,
            (k, v) => return Err(format!("`{k}` cannot be {v} (includeAudio, loop, cacheBeforePlayback, fullScreen: bool; range: workArea|workAreaExtended|comp|aroundCurrent|playFromCurrent; resolution: auto|full|half|third|quarter; frameRate: auto or fps; skip: 0 to 30; scrubAudio: bool)")),
        }
    }
    e.revision += 1;
    playback_get(e, &Json::Null)
}

pub fn render_backend(e: &mut Engine, p: &Json) -> Res {
    let gpu = crate::render::gpu::current().map(|g| format!("{} ({})", g.adapter, g.backend));
    if let Some(b) = ps(p, "backend") {
        let v = match b.to_ascii_lowercase().as_str() {
            "gpu" | "mercury gpu" | "auto" => "gpu",
            "cpu" | "software" | "mercury software only" => "cpu",
            other => return Err(format!("`{other}` is not a backend (gpu or cpu)")),
        };
        e.proj().settings.renderer = v.into();
        e.set_label("Video Rendering and Effects");
    }
    let r = &e.project.settings.renderer;
    Ok(json!({"backend": r, "label": if r == "cpu" { "Software Only" } else { "GPU Acceleration" }, "gpu": gpu, "gpuAvailable": gpu.is_some()}))
}

const LANG: &[(&str, &[&str])] = &[
    ("Global", &["comp(name)", "footage(name)", "thisComp", "thisLayer", "thisProperty", "time", "colorDepth", "posterizeTime(updatesPerSecond)", "value"]),
    ("Vector Math", &["add(vec1, vec2)", "sub(vec1, vec2)", "mul(vec, amount)", "div(vec, amount)", "clamp(value, limit1, limit2)", "dot(vec1, vec2)", "cross(vec1, vec2)", "normalize(vec)", "length(vec)", "length(point1, point2)", "lookAt(fromPoint, atPoint)"]),
    ("Random Numbers", &["seedRandom(offset, timeless)", "random()", "random(maxValOrArray)", "random(minValOrArray, maxValOrArray)", "gaussRandom()", "noise(valOrArray)"]),
    ("Interpolation", &["linear(t, tMin, tMax, value1, value2)", "linear(t, value1, value2)", "ease(t, tMin, tMax, value1, value2)", "easeIn(t, tMin, tMax, value1, value2)", "easeOut(t, tMin, tMax, value1, value2)"]),
    ("Color Conversion", &["rgbToHsl(rgbaArray)", "hslToRgb(hslaArray)", "hexToRgb(hexString)"]),
    ("Other Math", &["degreesToRadians(degrees)", "radiansToDegrees(radians)"]),
    ("Time Conversion", &["timeToFrames(t, fps, isDuration)", "framesToTime(frames, fps)", "timeToTimecode(t, timecodeBase, isDuration)", "timeToCurrentFormat(t, fps, isDuration)"]),
    ("Comp", &["layer(index)", "layer(name)", "marker", "numLayers", "activeCamera", "width", "height", "duration", "frameDuration", "bgColor", "name"]),
    ("Layer General", &["width", "height", "index", "parent", "hasParent", "inPoint", "outPoint", "startTime", "hasVideo", "hasAudio", "active", "enabled", "audioActive", "sampleImage(point, radius, postEffect, t)"]),
    ("Layer Properties", &["anchorPoint", "position", "scale", "rotation", "opacity", "audioLevels", "timeRemap", "marker", "name"]),
    ("Layer 3D", &["orientation", "rotationX", "rotationY", "rotationZ", "lightTransmission", "castsShadows", "acceptsShadows", "acceptsLights", "ambient", "diffuse", "specular", "shininess", "metal"]),
    ("Layer Space Transforms", &["toComp(point, t)", "fromComp(point, t)", "toWorld(point, t)", "fromWorld(point, t)", "toCompVec(vec, t)", "fromCompVec(vec, t)", "toWorldVec(vec, t)", "fromWorldVec(vec, t)", "fromCompToSurface(point, t)"]),
    ("Camera", &["pointOfInterest", "zoom", "depthOfField", "focusDistance", "aperture", "blurLevel", "active"]),
    ("Light", &["pointOfInterest", "intensity", "color", "coneAngle", "coneFeather", "shadowDarkness", "shadowDiffusion"]),
    ("Effect", &["active", "param(name)", "param(index)", "name"]),
    ("Path Property", &["points(t)", "inTangents(t)", "outTangents(t)", "isClosed()", "pointOnPath(percentage, t)", "tangentOnPath(percentage, t)", "normalOnPath(percentage, t)", "createPath(points, inTangents, outTangents, is_closed)"]),
    ("Property", &["value", "valueAtTime(t)", "velocity", "velocityAtTime(t)", "speed", "speedAtTime(t)", "wiggle(freq, amp, octaves, amp_mult, t)", "temporalWiggle(freq, amp, octaves, amp_mult, t)", "smooth(width, samples, t)", "loopIn(type, numKeyframes)", "loopOut(type, numKeyframes)", "loopInDuration(type, duration)", "loopOutDuration(type, duration)", "key(index)", "key(markerName)", "nearestKey(t)", "numKeys", "propertyGroup(countUp)", "propertyIndex", "name"]),
    ("Key", &["value", "time", "index"]),
    ("Marker Key", &["comment", "chapter", "url", "frameTarget", "eventCuePoint", "cuePointName", "parameters", "protectedRegion", "duration"]),
    ("Project", &["fullPath", "bitsPerChannel", "linearBlending"]),
    ("Text", &["sourceText", "text.sourceText.style", "getStyleAt(charIndex, t)", "createStyle()", "setFontSize(value)", "setFont(name)", "setFillColor(rgb)", "setText(value)", "setTracking(value)", "setLeading(value)", "setApplyFill(bool)", "setApplyStroke(bool)", "setStrokeColor(rgb)", "setStrokeWidth(value)"]),
    ("JavaScript Math", &["Math.cos(value)", "Math.sin(value)", "Math.tan(value)", "Math.acos(value)", "Math.asin(value)", "Math.atan(value)", "Math.atan2(y, x)", "Math.sqrt(value)", "Math.exp(value)", "Math.pow(value, exponent)", "Math.log(value)", "Math.abs(value)", "Math.round(value)", "Math.ceil(value)", "Math.floor(value)", "Math.min(value1, value2)", "Math.max(value1, value2)", "Math.PI", "Math.E"]),
];

pub fn language_menu(_: &mut Engine, _: &Json) -> Res {
    let cats: Vec<Json> = LANG.iter().map(|(name, items)| json!({"name": name, "items": items})).collect();
    Ok(json!({"engine": "formvfx expressions (JavaScript)", "categories": cats}))
}

fn luma(r: f64, g: f64, b: f64) -> f64 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

pub fn scopes(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let side = pf(p, "maxSide").unwrap_or(512.0).clamp(16.0, 4096.0);
    let job = FrameJob::from_params(e, &json!({"comp": cid, "time": t, "maxSide": side, "display": false, "guides": false, "cache": false}))?;
    let (w, h, px) = job.run();
    let (w, h) = (w as usize, h as usize);
    let cols = pu(p, "columns").unwrap_or(128).clamp(8, 1024) as usize;
    let levels = pu(p, "levels").unwrap_or(64).clamp(8, 256) as usize;
    let vs = pu(p, "vectorscopeSize").unwrap_or(64).clamp(8, 256) as usize;
    let mut hist = vec![vec![0u64; 256]; 4];
    let mut wave = vec![vec![0u32; levels]; cols];
    let mut parade = vec![vec![vec![0u32; levels]; cols]; 3];
    let mut vec_scope = vec![vec![0u32; vs]; vs];
    let (mut lo, mut hi, mut sum) = (f64::MAX, f64::MIN, [0.0f64; 4]);
    let (mut black, mut white) = (0u64, 0u64);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let (r, g, b) = (px[i] as f64 / 255.0, px[i + 1] as f64 / 255.0, px[i + 2] as f64 / 255.0);
            let yv = luma(r, g, b);
            let yb = (yv * 255.0).round().clamp(0.0, 255.0) as usize;
            hist[0][px[i] as usize] += 1;
            hist[1][px[i + 1] as usize] += 1;
            hist[2][px[i + 2] as usize] += 1;
            hist[3][yb] += 1;
            lo = lo.min(yv);
            hi = hi.max(yv);
            for (k, v) in [r, g, b, yv].iter().enumerate() {
                sum[k] += v;
            }
            if yv <= 1.0 / 255.0 {
                black += 1;
            }
            if yv >= 254.0 / 255.0 {
                white += 1;
            }
            let col = x * cols / w.max(1);
            let lev = |v: f64| ((v * (levels - 1) as f64).round() as usize).min(levels - 1);
            wave[col][lev(yv)] += 1;
            for (k, v) in [r, g, b].iter().enumerate() {
                parade[k][col][lev(*v)] += 1;
            }
            let cb = (b - yv) / 1.8556;
            let cr = (r - yv) / 1.5748;
            let vx = (((cb + 0.5) * vs as f64) as isize).clamp(0, vs as isize - 1) as usize;
            let vy = (((0.5 - cr) * vs as f64) as isize).clamp(0, vs as isize - 1) as usize;
            vec_scope[vy][vx] += 1;
        }
    }
    let n = (w * h).max(1) as f64;
    let want = ps(p, "scope").unwrap_or("all");
    let mut out = json!({
        "comp": cid, "time": t, "width": w, "height": h, "pixels": w * h, "standard": "Rec. 709",
        "luma": {"min": lo, "max": hi, "mean": sum[3] / n, "clippedBlack": black as f64 / n, "clippedWhite": white as f64 / n},
        "mean": {"red": sum[0] / n, "green": sum[1] / n, "blue": sum[2] / n},
    });
    if matches!(want, "all" | "histogram") {
        out["histogram"] = json!({"red": hist[0], "green": hist[1], "blue": hist[2], "luma": hist[3]});
    }
    if matches!(want, "all" | "waveform") {
        out["waveform"] = json!({"columns": cols, "levels": levels, "data": wave});
    }
    if matches!(want, "all" | "parade") {
        out["parade"] = json!({"columns": cols, "levels": levels, "red": parade[0], "green": parade[1], "blue": parade[2]});
    }
    if matches!(want, "all" | "vectorscope") {
        out["vectorscope"] = json!({"size": vs, "data": vec_scope, "axes": "x: Cb from -0.5 to 0.5, y: Cr from 0.5 (top) to -0.5"});
    }
    if !matches!(want, "all" | "histogram" | "waveform" | "parade" | "vectorscope") {
        return Err(format!("`{want}` is not a scope (all, histogram, waveform, parade, vectorscope)"));
    }
    Ok(out)
}
