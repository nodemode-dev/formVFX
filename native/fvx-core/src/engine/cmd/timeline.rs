use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use parking_lot::Mutex;
use serde_json::{Value as Json, json};

use super::{add_layer, footage_layer, formats};
use crate::engine::xmldoc::{self, El};
use crate::engine::{Engine, Res, pf, ps};
use crate::model::{Footage, FootageKind, Item, ItemKind, LayerKind, OutputModule, RenderItem};
use crate::render::media;

const FORMATS: [(&str, &str, &str, bool); 6] = [
    ("xml", "xmeml 5 XML (.xml)", "xml", true),
    ("fcpxml", "FCPXML 1.10", "fcpxml", true),
    ("otio", "OpenTimelineIO (.otio JSON)", "otio", true),
    ("edl", "CMX 3600 EDL", "edl", true),
    ("aaf", "Advanced Authoring Format", "aaf", false),
    ("omf", "Open Media Framework", "omf", false),
];

const BINARY: &str = "is a binary container (structured storage) formvfx does not read or write; export xmeml XML, FCPXML, OTIO or an EDL from the editor instead";

#[derive(Clone, Default)]
struct Clip {
    name: String,
    file: String,
    path: String,
    start: f64,
    dur: f64,
    src_in: f64,
    speed: f64,
    enabled: bool,
    length: f64,
    video: bool,
    audio: bool,
}

struct Track {
    name: String,
    clips: Vec<Clip>,
}

struct Timeline {
    name: String,
    fps: f64,
    width: u32,
    height: u32,
    video: Vec<Track>,
    audio: Vec<Track>,
    report: Vec<String>,
}

impl Timeline {
    fn new(name: &str, fps: f64) -> Timeline {
        Timeline { name: name.to_string(), fps, width: 0, height: 0, video: vec![], audio: vec![], report: vec![] }
    }

    fn end(&self) -> f64 {
        self.video.iter().chain(self.audio.iter()).flat_map(|t| t.clips.iter()).map(|c| c.start + c.dur).fold(0.0, f64::max)
    }

    fn clips(&self) -> usize {
        self.video.iter().chain(self.audio.iter()).map(|t| t.clips.len()).sum()
    }
}

pub fn formats(_: &mut Engine, _: &Json) -> Res {
    Ok(Json::Array(
        FORMATS
            .iter()
            .map(|(id, name, ext, ok)| json!({"id": id, "name": name, "extensions": [ext], "import": ok, "export": ok, "note": if *ok { Json::Null } else { json!(format!("{} {BINARY}", id.to_uppercase())) }}))
            .collect(),
    ))
}

fn find<'a>(el: &'a El, name: &str) -> Option<&'a El> {
    if el.name == name {
        return Some(el);
    }
    el.children().find_map(|c| find(c, name))
}

fn walk<'a>(el: &'a El, name: &str, out: &mut Vec<&'a El>) {
    if el.name == name {
        out.push(el);
    }
    for c in el.children() {
        walk(c, name, out);
    }
}

fn txt(el: &El, path: &str) -> Option<String> {
    let mut cur = el;
    for part in path.split('/') {
        cur = cur.child(part)?;
    }
    Some(cur.text().trim().to_string())
}

fn num(el: &El, path: &str) -> Option<f64> {
    txt(el, path)?.parse().ok()
}

fn timebase(fps: f64) -> (u32, bool) {
    (fps.round().max(1.0) as u32, (fps - fps.round()).abs() > 1e-3)
}

fn rate_of(base: u32, ntsc: bool) -> f64 {
    if ntsc { base as f64 * 1000.0 / 1001.0 } else { base as f64 }
}

fn frames(sec: f64, fps: f64) -> i64 {
    (sec * fps).round() as i64
}

fn rational(s: &str) -> f64 {
    let t = s.trim().trim_end_matches('s');
    match t.split_once('/') {
        Some((a, b)) => {
            let d: f64 = b.trim().parse().unwrap_or(1.0);
            if d.abs() < 1e-12 { 0.0 } else { a.trim().parse::<f64>().unwrap_or(0.0) / d }
        }
        None => t.parse().unwrap_or(0.0),
    }
}

fn fcp_time(sec: f64, fps: f64) -> String {
    let (base, ntsc) = timebase(fps);
    let n = frames(sec, fps);
    if n == 0 {
        "0s".into()
    } else if ntsc {
        format!("{}/{}s", n * 1001, base * 1000)
    } else {
        format!("{n}/{base}s")
    }
}

fn tc(n: i64, base: u32) -> String {
    let b = base.max(1) as i64;
    let f = n.max(0);
    format!("{:02}:{:02}:{:02}:{:02}", f / (3600 * b), f / (60 * b) % 60, f / b % 60, f % b)
}

fn is_tc(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 11 && [2, 5, 8].iter().all(|&i| matches!(b[i], b':' | b';' | b'.')) && [0, 1, 3, 4, 6, 7, 9, 10].iter().all(|&i| b[i].is_ascii_digit())
}

fn tc_frames(code: &str, base: u32, drop: bool) -> Option<i64> {
    let parts: Vec<i64> = code.split([':', ';', '.']).map(|x| x.parse().ok()).collect::<Option<Vec<_>>>()?;
    if parts.len() != 4 {
        return None;
    }
    let b = base.max(1) as i64;
    let mut total = ((parts[0] * 60 + parts[1]) * 60 + parts[2]) * b + parts[3];
    if drop {
        let d = if b >= 60 { 4 } else { 2 };
        let m = parts[0] * 60 + parts[1];
        total -= d * (m - m / 10);
    }
    Some(total)
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = ((b[i + 1] as char).to_digit(16), (b[i + 2] as char).to_digit(16))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn url_to_path(u: &str) -> String {
    let s = u.trim();
    let s = s.strip_prefix("file://localhost").or_else(|| s.strip_prefix("file://")).unwrap_or(s);
    let d = decode(s);
    let b = d.as_bytes();
    if b.len() > 3 && b[0] == b'/' && b[2] == b':' && b[1].is_ascii_alphabetic() { d[1..].to_string() } else { d }
}

fn path_to_url(p: &str, host: &str) -> String {
    let p = p.replace('\\', "/");
    let p = if p.starts_with('/') { p } else { format!("/{p}") };
    let mut out = format!("file://{host}");
    for c in p.bytes() {
        if c.is_ascii_alphanumeric() || b"-._~/:".contains(&c) {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => o.push_str(&format!("&#x{:X};", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("").to_string()
}

fn read_xmeml(text: &str) -> Result<Timeline, String> {
    let root = xmldoc::parse(text)?;
    let seq = find(&root, "sequence").ok_or("no xmeml sequence in this file")?;
    let base = num(seq, "rate/timebase").unwrap_or(30.0).max(1.0) as u32;
    let ntsc = txt(seq, "rate/ntsc").is_some_and(|x| x.eq_ignore_ascii_case("true"));
    let fps = rate_of(base, ntsc);
    let mut files: HashMap<String, (String, String, f64)> = HashMap::new();
    let mut all = vec![];
    walk(&root, "file", &mut all);
    for f in all {
        if f.child("name").is_some() || f.child("pathurl").is_some() {
            let ff = num(f, "rate/timebase").map(|b| rate_of(b.max(1.0) as u32, txt(f, "rate/ntsc").is_some_and(|x| x.eq_ignore_ascii_case("true")))).unwrap_or(fps);
            files.insert(f.attr("id").unwrap_or("").to_string(), (txt(f, "name").unwrap_or_default(), url_to_path(&txt(f, "pathurl").unwrap_or_default()), num(f, "duration").unwrap_or(0.0) / ff));
        }
    }
    let mut tl = Timeline::new(&txt(seq, "name").filter(|s| !s.is_empty()).unwrap_or_else(|| "Imported XML".into()), fps);
    tl.width = num(seq, "media/video/format/samplecharacteristics/width").unwrap_or(0.0) as u32;
    tl.height = num(seq, "media/video/format/samplecharacteristics/height").unwrap_or(0.0) as u32;
    for (kind, audio) in [("video", false), ("audio", true)] {
        let Some(holder) = seq.child("media").and_then(|m| m.child(kind)) else { continue };
        for (n, lane) in holder.children().filter(|c| c.name == "track").enumerate() {
            let mut t = Track { name: format!("{}{}", if audio { "A" } else { "V" }, n + 1), clips: vec![] };
            for el in lane.children() {
                if matches!(el.name.as_str(), "transitionitem" | "generatoritem") {
                    let what = txt(el, "name").or_else(|| txt(el, "effect/name")).unwrap_or_default();
                    tl.report.push(format!("{}: a {} ({what}) is not carried; the cut stays", t.name, el.name));
                    continue;
                }
                if el.name != "clipitem" {
                    continue;
                }
                let cf = num(el, "rate/timebase").map(|b| rate_of(b.max(1.0) as u32, txt(el, "rate/ntsc").is_some_and(|x| x.eq_ignore_ascii_case("true")))).unwrap_or(fps);
                let (start, end) = (num(el, "start").unwrap_or(-1.0), num(el, "end").unwrap_or(-1.0));
                let sin = num(el, "in").unwrap_or(0.0);
                let len = num(el, "out").unwrap_or(sin) - sin;
                let rec = len * fps / cf;
                let (start, end) = match (start >= 0.0, end >= 0.0) {
                    (true, true) => (start, end),
                    (false, true) => (end - rec, end),
                    (true, false) => (start, start + rec),
                    _ => {
                        tl.report.push(format!("{}: {} sits between two transitions and is left out", t.name, txt(el, "name").unwrap_or_default()));
                        continue;
                    }
                };
                if end <= start {
                    continue;
                }
                let fid = el.child("file").and_then(|f| f.attr("id")).unwrap_or("");
                let (fname, path, flen) = files.get(fid).cloned().unwrap_or_else(|| (txt(el, "name").unwrap_or_default(), String::new(), 0.0));
                let (sdur, rdur) = (len / cf, (end - start) / fps);
                let speed = if sdur > 0.0 && (sdur - rdur).abs() > 0.5 / fps { sdur / rdur } else { 1.0 };
                t.clips.push(Clip {
                    name: txt(el, "name").filter(|s| !s.is_empty()).unwrap_or_else(|| fname.clone()),
                    file: fname,
                    path,
                    start: start / fps,
                    dur: rdur,
                    src_in: sin / cf,
                    speed,
                    enabled: txt(el, "enabled").is_none_or(|x| !x.eq_ignore_ascii_case("false")),
                    length: flen,
                    video: !audio,
                    audio,
                });
            }
            if audio { tl.audio.push(t) } else { tl.video.push(t) }
        }
    }
    Ok(tl)
}

type Asset = (String, String, f64, f64);

fn place(el: &El, at: f64, lane: i32, assets: &HashMap<String, Asset>, lanes: &mut BTreeMap<i32, Vec<Clip>>, report: &mut Vec<String>) {
    let start = el.attr("start").map(rational).unwrap_or(0.0);
    let (r, point) = match el.name.as_str() {
        "asset-clip" => (el.attr("ref"), start),
        "video" | "audio" if el.attr("ref").is_some() => (el.attr("ref"), start),
        "clip" => {
            let mut inner = vec![];
            walk(el, "video", &mut inner);
            walk(el, "audio", &mut inner);
            let Some(i) = inner.into_iter().find(|i| i.attr("ref").is_some()) else {
                report.push(format!("lane {lane}: the clip {} holds no media", el.attr("name").unwrap_or("")));
                return;
            };
            (i.attr("ref"), start - i.attr("offset").map(rational).unwrap_or(0.0) + i.attr("start").map(rational).unwrap_or(0.0))
        }
        "gap" | "spine" => return,
        "transition" => {
            report.push(format!("lane {lane}: a transition is not carried; the cut stays"));
            return;
        }
        other => {
            report.push(format!("lane {lane}: a {other} ({}) is not carried", el.attr("name").unwrap_or("")));
            return;
        }
    };
    let Some((name, path, astart, alen)) = r.and_then(|r| assets.get(r)).cloned() else {
        report.push(format!("{}: names a resource the file does not hold", el.attr("name").unwrap_or("a clip")));
        return;
    };
    if el.child("timeMap").is_some() {
        report.push(format!("{}: its retiming is not carried; it plays at normal speed", el.attr("name").unwrap_or(&name)));
    }
    lanes.entry(lane).or_default().push(Clip {
        name: el.attr("name").map(str::to_string).unwrap_or_else(|| name.clone()),
        file: name,
        path,
        start: at,
        dur: el.attr("duration").map(rational).unwrap_or(0.0),
        src_in: (point - astart).max(0.0),
        speed: 1.0,
        enabled: el.attr("enabled") != Some("0"),
        length: alen,
        video: lane >= 0,
        audio: lane < 0,
    });
}

fn read_fcpxml(text: &str) -> Result<Timeline, String> {
    let root = xmldoc::parse(text)?;
    let mut formats_: HashMap<String, (f64, u32, u32)> = HashMap::new();
    let mut list = vec![];
    walk(&root, "format", &mut list);
    for f in list {
        let fd = f.attr("frameDuration").map(rational).filter(|d| *d > 0.0);
        let dim = |k: &str| f.attr(k).and_then(|v| v.parse().ok()).unwrap_or(0);
        formats_.insert(f.attr("id").unwrap_or("").to_string(), (fd.map(|d| 1.0 / d).unwrap_or(0.0), dim("width"), dim("height")));
    }
    let mut assets: HashMap<String, Asset> = HashMap::new();
    let mut list = vec![];
    walk(&root, "asset", &mut list);
    for a in list {
        let src = a.child("media-rep").and_then(|r| r.attr("src")).or_else(|| a.attr("src")).unwrap_or("");
        assets.insert(a.attr("id").unwrap_or("").to_string(), (a.attr("name").unwrap_or("").to_string(), url_to_path(src), a.attr("start").map(rational).unwrap_or(0.0), a.attr("duration").map(rational).unwrap_or(0.0)));
    }
    let seq = find(&root, "sequence").ok_or("no FCPXML sequence in this file")?;
    let spine = seq.child("spine").ok_or("the FCPXML sequence has no spine")?;
    let (fps, w, h) = seq.attr("format").and_then(|f| formats_.get(f)).copied().unwrap_or((0.0, 0, 0));
    let fps = if fps > 0.0 { (fps * 1000.0).round() / 1000.0 } else { 30.0 };
    let tc0 = seq.attr("tcStart").map(rational).unwrap_or(0.0);
    let mut lanes: BTreeMap<i32, Vec<Clip>> = BTreeMap::new();
    let mut report = vec![];
    for item in spine.children() {
        let at = item.attr("offset").map(rational).unwrap_or(0.0) - tc0;
        place(item, at, 0, &assets, &mut lanes, &mut report);
        let own = item.attr("start").map(rational).unwrap_or(0.0);
        for child in item.children() {
            let Some(lane) = child.attr("lane").and_then(|l| l.parse::<i32>().ok()) else { continue };
            let cat = at + child.attr("offset").map(rational).unwrap_or(0.0) - own;
            if child.name == "spine" {
                let mut cur = cat;
                for c in child.children() {
                    place(c, cur, lane, &assets, &mut lanes, &mut report);
                    cur += c.attr("duration").map(rational).unwrap_or(0.0);
                }
            } else {
                place(child, cat, lane, &assets, &mut lanes, &mut report);
            }
        }
    }
    let name = find(&root, "project").and_then(|p| p.attr("name")).or_else(|| seq.attr("name")).unwrap_or("Imported FCPXML");
    let mut tl = Timeline::new(name, fps);
    tl.width = w;
    tl.height = h;
    tl.report = report;
    for (lane, mut clips) in lanes {
        clips.sort_by(|a, b| a.start.total_cmp(&b.start));
        if lane >= 0 {
            tl.video.push(Track { name: format!("V{}", lane + 1), clips });
        } else {
            tl.audio.insert(0, Track { name: format!("A{}", -lane), clips });
        }
    }
    Ok(tl)
}

fn schema(j: &Json) -> &str {
    j.get("OTIO_SCHEMA").and_then(Json::as_str).unwrap_or("")
}

fn secs(rt: &Json) -> f64 {
    let rate = rt.get("rate").and_then(Json::as_f64).unwrap_or(1.0);
    if rate <= 0.0 { 0.0 } else { rt.get("value").and_then(Json::as_f64).unwrap_or(0.0) / rate }
}

fn read_otio(text: &str) -> Result<Timeline, String> {
    let doc: Json = serde_json::from_str(text).map_err(|x| format!("not OpenTimelineIO JSON: {x}"))?;
    let tlj = if schema(&doc).starts_with("SerializableCollection") {
        doc.get("children").and_then(Json::as_array).and_then(|a| a.iter().find(|c| schema(c).starts_with("Timeline"))).cloned().unwrap_or(Json::Null)
    } else {
        doc
    };
    if !schema(&tlj).starts_with("Timeline") {
        return Err("no OpenTimelineIO timeline in this file".into());
    }
    let meta = tlj.get("metadata").and_then(|m| m.get("formvfx").or_else(|| m.get("formcut"))).cloned().unwrap_or(Json::Null);
    let mut fps = meta.get("fps").and_then(Json::as_f64).unwrap_or(0.0);
    let mut tl = Timeline::new(tlj.get("name").and_then(Json::as_str).filter(|s| !s.is_empty()).unwrap_or("Imported OTIO"), 30.0);
    tl.width = meta.get("width").and_then(Json::as_u64).unwrap_or(0) as u32;
    tl.height = meta.get("height").and_then(Json::as_u64).unwrap_or(0) as u32;
    let tracks = tlj.get("tracks").and_then(|s| s.get("children")).and_then(Json::as_array).cloned().unwrap_or_default();
    for (n, track) in tracks.iter().enumerate() {
        if !schema(track).starts_with("Track") {
            tl.report.push(format!("a {} in the stack is not a track; left out", schema(track)));
            continue;
        }
        let audio = track.get("kind").and_then(Json::as_str) == Some("Audio");
        let mut t = Track { name: track.get("name").and_then(Json::as_str).filter(|s| !s.is_empty()).map(str::to_string).unwrap_or_else(|| format!("{}{}", if audio { "A" } else { "V" }, n + 1)), clips: vec![] };
        let mut pos = 0.0;
        for item in track.get("children").and_then(Json::as_array).cloned().unwrap_or_default() {
            let sr = item.get("source_range").cloned().unwrap_or(Json::Null);
            let dur = sr.get("duration").map(secs).unwrap_or(0.0);
            if fps <= 0.0 {
                fps = sr.get("duration").and_then(|d| d.get("rate")).and_then(Json::as_f64).unwrap_or(0.0);
            }
            let sc = schema(&item);
            if sc.starts_with("Gap") {
                pos += dur;
            } else if sc.starts_with("Transition") {
                tl.report.push(format!("{}: a transition is not carried; the cut stays", t.name));
            } else if sc.starts_with("Clip") {
                let mut r = item.get("media_reference").cloned().unwrap_or(Json::Null);
                if r.is_null()
                    && let Some(m) = item.get("media_references")
                {
                    let key = item.get("active_media_reference_key").and_then(Json::as_str).unwrap_or("DEFAULT_MEDIA");
                    r = m.get(key).cloned().unwrap_or(Json::Null);
                }
                let avail = r.get("available_range").filter(|x| !x.is_null());
                let speed = item.get("effects").and_then(Json::as_array).and_then(|a| a.iter().find(|x| schema(x).starts_with("LinearTimeWarp"))).and_then(|x| x.get("time_scalar")).and_then(Json::as_f64).unwrap_or(1.0);
                let name = item.get("name").and_then(Json::as_str).unwrap_or("").to_string();
                let path = url_to_path(r.get("target_url").and_then(Json::as_str).unwrap_or(""));
                t.clips.push(Clip {
                    file: if path.is_empty() { name.clone() } else { file_name(&path) },
                    name,
                    path,
                    start: pos,
                    dur,
                    src_in: (sr.get("start_time").map(secs).unwrap_or(0.0) - avail.and_then(|a| a.get("start_time")).map(secs).unwrap_or(0.0)).max(0.0),
                    speed: if speed.abs() > 1e-6 { speed } else { 1.0 },
                    enabled: item.get("enabled").and_then(Json::as_bool).unwrap_or(true),
                    length: avail.and_then(|a| a.get("duration")).map(secs).unwrap_or(0.0),
                    video: !audio,
                    audio,
                });
                pos += dur;
            } else {
                tl.report.push(format!("{}: a {} is not carried; its time is kept as a gap", t.name, if sc.is_empty() { "nested item" } else { sc }));
                pos += dur;
            }
        }
        if audio { tl.audio.push(t) } else { tl.video.push(t) }
    }
    tl.fps = if fps > 0.0 { fps } else { 30.0 };
    Ok(tl)
}

fn read_edl(text: &str, fps: f64) -> Result<Timeline, String> {
    let (base, _) = timebase(fps);
    let upper = text.to_ascii_uppercase();
    let drop = upper.lines().any(|l| l.trim_start().starts_with("FCM:") && l.contains("DROP FRAME") && !l.contains("NON-DROP"));
    struct Ev {
        reel: String,
        channel: String,
        codes: [i64; 4],
        name: String,
        path: String,
        speed: Option<f64>,
    }
    let mut evs: Vec<Ev> = vec![];
    let mut title = None;
    let mut report = vec![];
    for line in text.lines() {
        let t = line.trim();
        if let Some(x) = t.strip_prefix("TITLE:") {
            title = Some(x.trim().to_string());
            continue;
        }
        if let Some(rest) = t.strip_prefix('*') {
            let r = rest.trim();
            if let Some(n) = r.strip_prefix("FROM CLIP NAME:") {
                if let Some(ev) = evs.last_mut() {
                    ev.name = n.trim().to_string();
                }
            } else if let Some(n) = r.strip_prefix("SOURCE FILE:").or_else(|| r.strip_prefix("FROM FILE:"))
                && let Some(ev) = evs.last_mut()
            {
                ev.path = n.trim().to_string();
            }
            continue;
        }
        let toks: Vec<&str> = t.split_whitespace().collect();
        if toks.first() == Some(&"M2") && toks.len() >= 4 {
            if let (Some(ev), Ok(s)) = (evs.last_mut(), toks[2].parse::<f64>()) {
                ev.speed = Some(s / fps);
            }
            continue;
        }
        if toks.len() >= 8 && (3..=6).contains(&toks[0].len()) && toks[0].bytes().all(|c| c.is_ascii_digit()) {
            let codes: Vec<&str> = toks[toks.len() - 4..].to_vec();
            if !codes.iter().all(|c| is_tc(c)) {
                continue;
            }
            let Some(parsed) = codes.iter().map(|c| tc_frames(c, base, drop)).collect::<Option<Vec<i64>>>() else { continue };
            if toks[3] != "C" {
                report.push(format!("event {}: a {} transition is not carried; it comes in as a cut", toks[0], toks[3]));
            }
            evs.push(Ev { reel: toks[1].to_string(), channel: toks[2].to_ascii_uppercase(), codes: [parsed[0], parsed[1], parsed[2], parsed[3]], name: toks[1].to_string(), path: String::new(), speed: None });
        }
    }
    if evs.is_empty() {
        return Err("no CMX 3600 events in this file".into());
    }
    let hour = 3600 * base as i64;
    let origin = evs.iter().map(|e| e.codes[2]).min().unwrap_or(0) / hour * hour;
    let mut tl = Timeline::new(title.as_deref().filter(|s| !s.is_empty()).unwrap_or("Imported EDL"), fps);
    tl.report = report;
    let mut video = Track { name: "V1".into(), clips: vec![] };
    let mut audio: BTreeMap<String, Track> = BTreeMap::new();
    for ev in evs {
        let vid = ev.channel.contains('V') || ev.channel == "B";
        let rec = (ev.codes[3] - ev.codes[2]).max(1);
        let src = ev.codes[1] - ev.codes[0];
        let speed = ev.speed.unwrap_or(if src > 0 && src != rec { src as f64 / rec as f64 } else { 1.0 });
        let clip = Clip {
            file: if ev.path.is_empty() { ev.reel.clone() } else { file_name(&ev.path) },
            name: ev.name,
            path: ev.path,
            start: (ev.codes[2] - origin) as f64 / fps,
            dur: rec as f64 / fps,
            src_in: ev.codes[0] as f64 / fps,
            speed,
            enabled: true,
            length: 0.0,
            video: vid,
            audio: !vid,
        };
        if vid {
            video.clips.push(clip);
        } else {
            let lane = if ev.channel.starts_with("A2") { "A2" } else { "A1" };
            audio.entry(lane.to_string()).or_insert_with(|| Track { name: lane.to_string(), clips: vec![] }).clips.push(clip);
        }
    }
    if !video.clips.is_empty() {
        tl.video.push(video);
    }
    tl.audio = audio.into_values().collect();
    Ok(tl)
}

fn sniff(path: &str, text: &str) -> &'static str {
    let ext = Path::new(path).extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "fcpxml" | "fcpxmld" => "fcpxml",
        "otio" => "otio",
        "edl" => "edl",
        "aaf" => "aaf",
        "omf" | "omfi" => "omf",
        _ if text.contains("<fcpxml") => "fcpxml",
        _ if text.contains("<xmeml") => "xml",
        _ if text.trim_start().starts_with('{') => "otio",
        _ => "edl",
    }
}

fn locate(c: &Clip, dir: &Path) -> Option<String> {
    if !c.path.is_empty() && Path::new(&c.path).is_file() {
        return Some(c.path.clone());
    }
    let name = if c.path.is_empty() { c.file.clone() } else { file_name(&c.path) };
    for d in [dir.to_path_buf(), dir.join("Footage"), dir.join("media")] {
        if !name.is_empty() && d.join(&name).is_file() {
            return Some(d.join(&name).to_string_lossy().to_string());
        }
    }
    let stems: Vec<String> = [&name, &c.name].iter().filter(|s| !s.is_empty()).map(|s| Path::new(s.as_str()).file_stem().and_then(|x| x.to_str()).unwrap_or(s).to_ascii_lowercase()).collect();
    for d in [dir.to_path_buf(), dir.join("Footage")] {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for ent in rd.flatten() {
            let p = ent.path();
            let stem = p.file_stem().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
            if p.is_file() && stems.contains(&stem) && media::is_media(&p.to_string_lossy()) {
                return Some(p.to_string_lossy().to_string());
            }
        }
    }
    None
}

pub fn placeholder(w: u32, h: u32, fps: f64, duration: f64) -> Footage {
    let still = duration <= 0.0;
    Footage { path: String::new(), kind: if still { FootageKind::Image } else { FootageKind::Video }, width: w.clamp(1, 30000), height: h.clamp(1, 30000), duration: duration.max(0.0), frame_rate: if still { 0.0 } else { fps.max(1.0) }, has_video: true, codec: "placeholder".into(), missing: true, ..Footage::default() }
}

fn item_for(e: &mut Engine, c: &Clip, dir: &Path, folder: u64, tl: &mut Timeline, cache: &mut HashMap<String, u64>) -> Result<(u64, bool), String> {
    if let Some(path) = locate(c, dir) {
        if let Some(id) = cache.get(&path) {
            return Ok((*id, false));
        }
        let existing = e.project.items.iter().find(|i| matches!(&i.kind, ItemKind::Footage(f) if f.path == path)).map(|i| i.id);
        let id = match existing {
            Some(id) => id,
            None => {
                let mut warnings = vec![];
                let (f, name) = formats::probe(&path, &json!({}), &mut warnings)?;
                tl.report.extend(warnings);
                let pj = e.proj();
                let id = pj.new_id();
                pj.items.push(Item { id, name, parent: Some(folder), label: formats::label_of(&f), comment: String::new(), kind: ItemKind::Footage(f), proxy: None });
                id
            }
        };
        cache.insert(path, id);
        return Ok((id, false));
    }
    let key = format!("\u{1}{}", if c.file.is_empty() { &c.name } else { &c.file });
    if let Some(id) = cache.get(&key) {
        return Ok((*id, true));
    }
    let named = e.project.items.iter().find(|i| matches!(&i.kind, ItemKind::Footage(_)) && (i.name == c.file || i.name == c.name)).map(|i| i.id);
    if let Some(id) = named {
        cache.insert(key, id);
        return Ok((id, false));
    }
    let label = if c.file.is_empty() { c.name.clone() } else { c.file.clone() };
    tl.report.push(format!("{label}: {} was not found; a placeholder stands in (File > Replace Footage)", if c.path.is_empty() { "the media file" } else { c.path.as_str() }));
    let (w, h) = (if tl.width > 0 { tl.width } else { 1920 }, if tl.height > 0 { tl.height } else { 1080 });
    let mut f = placeholder(w, h, tl.fps, c.length.max(c.src_in + c.dur * c.speed.abs()));
    f.has_audio = c.audio;
    let pj = e.proj();
    let id = pj.new_id();
    pj.items.push(Item { id, name: label, parent: Some(folder), label: 1, comment: String::new(), kind: ItemKind::Footage(f), proxy: None });
    cache.insert(key, id);
    Ok((id, true))
}

fn build(e: &mut Engine, mut tl: Timeline, dir: &Path) -> Res {
    if tl.clips() == 0 {
        return Err(format!("the timeline holds no clips formvfx can carry{}", if tl.report.is_empty() { String::new() } else { format!(": {}", tl.report.join("; ")) }));
    }
    let fps = tl.fps.clamp(1.0, 999.0);
    let dur = tl.end().max(1.0 / fps);
    let fname = e.project.unique_item_name(&tl.name);
    let folder = {
        let pj = e.proj();
        let id = pj.new_id();
        pj.items.push(Item { id, name: fname, parent: None, label: 0, comment: String::new(), kind: ItemKind::Folder, proxy: None });
        id
    };
    let mut cache = HashMap::new();
    let mut placed: Vec<(Clip, u64, bool, bool)> = vec![];
    let audio: Vec<Track> = std::mem::take(&mut tl.audio);
    let video: Vec<Track> = std::mem::take(&mut tl.video);
    let mut holes = 0;
    for (tracks, is_audio) in [(&audio, true), (&video, false)] {
        let ordered: Vec<&Track> = if is_audio { tracks.iter().rev().collect() } else { tracks.iter().collect() };
        for t in ordered {
            for c in &t.clips {
                let (item, hole) = item_for(e, c, dir, folder, &mut tl, &mut cache)?;
                holes += hole as usize;
                placed.push((c.clone(), item, is_audio, hole));
            }
        }
    }
    let (w, h) = if tl.width > 0 && tl.height > 0 {
        (tl.width, tl.height)
    } else {
        placed.iter().filter(|p| !p.2).find_map(|p| e.project.item(p.1).and_then(Item::dimensions)).unwrap_or((1920, 1080))
    };
    let cid = super::comp::make_comp(e, &tl.name, w, h, fps, dur, [0.0, 0.0, 0.0]);
    if let Some(it) = e.proj().item_mut(cid) {
        it.parent = Some(folder);
    }
    let heard: Vec<(String, f64, f64)> = placed.iter().filter(|p| p.2).map(|p| (p.0.path.clone(), p.0.start, p.0.dur)).collect();
    let mut layers = 0;
    for (c, item, is_audio, _) in &placed {
        let mut l = footage_layer(e, cid, *item, c.start, None, Some(c.dur))?;
        let s = if c.speed.abs() > 1e-6 { c.speed } else { 1.0 };
        l.name = c.name.clone();
        l.stretch = 100.0 / s;
        l.start_time = c.start - c.src_in / s;
        l.in_point = c.start;
        l.out_point = c.start + c.dur;
        if *is_audio {
            l.switches.video = false;
        } else if !c.path.is_empty() && heard.iter().any(|(p, st, d)| *p == c.path && (st - c.start).abs() < 1e-3 && (d - c.dur).abs() < 1e-3) {
            l.switches.audio = false;
        }
        if !c.enabled {
            l.switches.video = false;
            l.switches.audio = false;
        }
        add_layer(e, cid, l, Some(0));
        layers += 1;
    }
    e.state.project_selection = vec![cid];
    super::comp::open(e, &json!({"comp": cid}))?;
    e.set_label(format!("Import {}", tl.name));
    Ok(json!({"comp": cid, "folder": folder, "name": tl.name, "layers": layers, "placeholders": holes, "duration": dur, "frameRate": fps, "width": w, "height": h, "report": tl.report}))
}

pub fn import_timeline(e: &mut Engine, p: &Json) -> Res {
    let path = ps(p, "path").ok_or("pass `path` (.xml, .fcpxml, .otio or .edl)")?.to_string();
    let bytes = std::fs::read(&path).map_err(|x| format!("{path}: {x}"))?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let format = match ps(p, "format").unwrap_or("auto") {
        "auto" => sniff(&path, &text),
        "xml" => "xml",
        "fcpxml" => "fcpxml",
        "otio" => "otio",
        "edl" => "edl",
        "aaf" => "aaf",
        "omf" => "omf",
        other => return Err(format!("`{other}` is not a timeline format (file.timelineFormats lists them)")),
    };
    let tl = match format {
        "xml" => read_xmeml(&text),
        "fcpxml" => read_fcpxml(&text),
        "otio" => read_otio(&text),
        "edl" => read_edl(&text, pf(p, "edlFrameRate").filter(|f| *f > 0.0).unwrap_or(30.0)),
        other => Err(format!("{} {BINARY}", other.to_uppercase())),
    }
    .map_err(|x| format!("{path}: {x}"))?;
    let dir = Path::new(&path).parent().map(Path::to_path_buf).unwrap_or_default();
    let mut r = build(e, tl, &dir)?;
    r["format"] = json!(format);
    Ok(r)
}

fn safe(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' }).collect();
    if t.trim().is_empty() { "Layer".into() } else { t.trim().to_string() }
}

fn prerender(e: &Engine, cid: u64, lid: u64, dir: &Path) -> Result<(String, f64, f64), String> {
    let mut pj = (*e.project).clone();
    let c = pj.comp_mut(cid).ok_or("no composition")?;
    let fd = c.frame_duration();
    let l = c.layer(lid).ok_or("no such layer")?;
    let (start, end, name) = (l.in_point.max(0.0), l.out_point.min(c.duration), l.name.clone());
    for x in c.layers.iter_mut() {
        if x.id != lid {
            x.switches.video = false;
            x.switches.audio = false;
        }
    }
    std::fs::create_dir_all(dir).map_err(|x| format!("{}: {x}", dir.display()))?;
    let out = dir.join(format!("{lid} {}.mov", safe(&name)));
    let mut it = RenderItem { id: 0, comp: cid, start, end: end.max(start + fd), ..RenderItem::default() };
    it.settings = crate::engine::rq::settings_template("Best Settings").unwrap_or_default();
    it.modules.push(OutputModule { format: "prores".into(), prores_profile: "4444".into(), channels: "rgba".into(), output: out.to_string_lossy().to_string(), ..OutputModule::default() });
    let progress = Mutex::new(crate::engine::export::Progress::default());
    let stop = AtomicBool::new(false);
    let path = crate::engine::rq::render_item(&pj, &e.media, &it, &progress, &stop, rayon::current_num_threads().clamp(1, 16))?;
    Ok((path.lines().next().unwrap_or("").to_string(), start, end.max(start + fd)))
}

fn collect(e: &Engine, cid: u64, mode: &str, dir: &Path) -> Result<Timeline, String> {
    let comp = e.project.comp(cid).ok_or("no composition")?;
    let name = e.project.item(cid).map(|i| i.name.clone()).unwrap_or_else(|| "Composition".into());
    let mut tl = Timeline::new(&name, comp.frame_rate);
    tl.width = comp.width;
    tl.height = comp.height;
    for l in comp.layers.iter().rev() {
        if !l.switches.video && !l.switches.audio {
            tl.report.push(format!("{}: its video and audio are off; left out", l.name));
            continue;
        }
        let src = l.source.and_then(|s| e.project.item(s));
        let footage = match src.map(|i| &i.kind) {
            Some(ItemKind::Footage(f)) if l.kind == LayerKind::Footage && !f.missing && !f.path.is_empty() => Some(f),
            _ => None,
        };
        let clip = match footage {
            Some(f) if mode != "all" => {
                let s = if l.stretch.abs() < 1e-9 { 1.0 } else { 100.0 / l.stretch };
                if s < 0.0 {
                    tl.report.push(format!("{}: it plays reversed in formvfx; written forward", l.name));
                }
                if l.props.get("timeRemap").is_some() {
                    tl.report.push(format!("{}: time remapping is not carried", l.name));
                }
                if l.props.sub("effects").is_some_and(|g| !g.children.is_empty()) {
                    tl.report.push(format!("{}: its effects are not carried", l.name));
                }
                let path = if f.kind == FootageKind::Sequence { f.sequence.first().cloned().unwrap_or_else(|| f.path.clone()) } else { f.path.clone() };
                Clip {
                    name: l.name.clone(),
                    file: file_name(&path),
                    path,
                    start: l.in_point,
                    dur: l.out_point - l.in_point,
                    src_in: ((l.in_point - l.start_time) * s.abs()).max(0.0),
                    speed: s.abs(),
                    enabled: l.switches.video || l.switches.audio,
                    length: if f.is_still() { 0.0 } else { f.duration },
                    video: f.has_video && l.switches.video,
                    audio: f.has_audio && l.switches.audio,
                }
            }
            _ if mode == "unsupported" || mode == "all" => {
                let (path, start, end) = prerender(e, cid, l.id, dir)?;
                tl.report.push(format!("{}: pre-rendered to {path}", l.name));
                Clip { name: l.name.clone(), file: file_name(&path), path, start, dur: end - start, src_in: 0.0, speed: 1.0, enabled: true, length: end - start, video: true, audio: false }
            }
            _ => {
                let what = match l.kind {
                    LayerKind::Footage if footage.is_none() && src.is_some() => "missing or placeholder footage".to_string(),
                    k => format!("{k:?} layer").to_lowercase(),
                };
                tl.report.push(format!("{}: a {what} is not carried (prerender: unsupported renders it to a movie)", l.name));
                continue;
            }
        };
        if clip.dur <= 0.0 {
            continue;
        }
        let track = Track { name: String::new(), clips: vec![clip.clone()] };
        if clip.video {
            tl.video.push(Track { name: format!("V{}", tl.video.len() + 1), ..track });
        } else if clip.audio {
            tl.audio.push(Track { name: format!("A{}", tl.audio.len() + 1), ..track });
        }
    }
    Ok(tl)
}

fn write_xmeml(tl: &Timeline) -> String {
    let (base, ntsc) = timebase(tl.fps);
    let rate = format!("<rate><timebase>{base}</timebase><ntsc>{}</ntsc></rate>", if ntsc { "TRUE" } else { "FALSE" });
    let f = |s: f64| frames(s, tl.fps);
    let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>\n<xmeml version=\"5\">\n<sequence id=\"sequence-1\">\n<name>{}</name>\n<duration>{}</duration>\n{rate}\n", esc(&tl.name), f(tl.end()));
    s.push_str(&format!("<timecode>{rate}<string>00:00:00:00</string><frame>0</frame><displayformat>NDF</displayformat></timecode>\n<media>\n"));
    let mut files: HashMap<String, usize> = HashMap::new();
    let mut n = 0;
    for (kind, tracks) in [("video", &tl.video), ("audio", &tl.audio)] {
        s.push_str(&format!("<{kind}>\n"));
        if kind == "video" {
            s.push_str(&format!("<format><samplecharacteristics><width>{}</width><height>{}</height>{rate}</samplecharacteristics></format>\n", tl.width, tl.height));
        }
        for t in tracks.iter() {
            s.push_str("<track>\n");
            for c in &t.clips {
                n += 1;
                let sin = f(c.src_in);
                s.push_str(&format!(
                    "<clipitem id=\"clipitem-{n}\"><name>{}</name><enabled>{}</enabled><duration>{}</duration>{rate}<start>{}</start><end>{}</end><in>{sin}</in><out>{}</out>",
                    esc(&c.name),
                    if c.enabled { "TRUE" } else { "FALSE" },
                    f(if c.length > 0.0 { c.length } else { c.src_in + c.dur * c.speed }),
                    f(c.start),
                    f(c.start + c.dur),
                    sin + f(c.dur * c.speed)
                ));
                match files.get(&c.path) {
                    Some(id) => s.push_str(&format!("<file id=\"file-{id}\"/>")),
                    None => {
                        let id = files.len() + 1;
                        files.insert(c.path.clone(), id);
                        s.push_str(&format!(
                            "<file id=\"file-{id}\"><name>{}</name><pathurl>{}</pathurl>{rate}<duration>{}</duration><media>{}{}</media></file>",
                            esc(&c.file),
                            esc(&path_to_url(&c.path, "localhost")),
                            f(c.length),
                            if c.video { "<video/>" } else { "" },
                            if c.audio || !c.video { "<audio/>" } else { "" }
                        ));
                    }
                }
                s.push_str("</clipitem>\n");
            }
            s.push_str("</track>\n");
        }
        s.push_str(&format!("</{kind}>\n"));
    }
    s.push_str("</media>\n</sequence>\n</xmeml>\n");
    s
}

fn write_fcpxml(tl: &mut Timeline) -> String {
    let fps = tl.fps;
    let (base, ntsc) = timebase(fps);
    let t = |x: f64| fcp_time(x, fps);
    let end = tl.end();
    let mut assets: Vec<(String, String)> = vec![];
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut asset = |c: &Clip| -> String {
        if let Some(id) = ids.get(&c.path) {
            return id.clone();
        }
        let id = format!("r{}", ids.len() + 2);
        assets.push((
            id.clone(),
            format!(
                "<asset id=\"{id}\" name=\"{}\" start=\"0s\" duration=\"{}\" hasVideo=\"{}\" hasAudio=\"{}\" format=\"r1\"><media-rep kind=\"original-media\" src=\"{}\"/></asset>",
                esc(&c.file),
                t(c.length),
                c.video as u8,
                c.audio as u8,
                esc(&path_to_url(&c.path, ""))
            ),
        ));
        ids.insert(c.path.clone(), id.clone());
        id
    };
    for c in tl.video.iter().chain(tl.audio.iter()).flat_map(|tr| tr.clips.iter()) {
        if (c.speed - 1.0).abs() > 1e-6 {
            tl.report.push(format!("{}: its speed is not carried in FCPXML; it plays at normal speed", c.name));
        }
    }
    let clip_el = |c: &Clip, offset: f64, lane: Option<i32>, id: &str| -> String {
        format!(
            "<asset-clip ref=\"{id}\" name=\"{}\" offset=\"{}\" start=\"{}\" duration=\"{}\"{}{}{}",
            esc(&c.name),
            t(offset),
            t(c.src_in),
            t(c.dur),
            lane.map(|l| format!(" lane=\"{l}\"")).unwrap_or_default(),
            if c.video { " format=\"r1\"" } else { "" },
            if c.enabled { "" } else { " enabled=\"0\"" }
        )
    };
    struct Holder {
        at: f64,
        dur: f64,
        own: f64,
        head: String,
        tag: &'static str,
        kids: Vec<String>,
    }
    let mut spine: Vec<Holder> = vec![];
    let mut pos = 0.0;
    if let Some(first) = tl.video.first() {
        for c in &first.clips {
            if c.start > pos + 1e-6 {
                spine.push(Holder { at: pos, dur: c.start - pos, own: 0.0, head: format!("<gap name=\"Gap\" offset=\"{}\" start=\"0s\" duration=\"{}\"", t(pos), t(c.start - pos)), tag: "gap", kids: vec![] });
            }
            let id = asset(c);
            spine.push(Holder { at: c.start, dur: c.dur, own: c.src_in, head: clip_el(c, c.start, None, &id), tag: "asset-clip", kids: vec![] });
            pos = c.start + c.dur;
        }
    }
    if end > pos + 1e-6 {
        spine.push(Holder { at: pos, dur: end - pos, own: 0.0, head: format!("<gap name=\"Gap\" offset=\"{}\" start=\"0s\" duration=\"{}\"", t(pos), t(end - pos)), tag: "gap", kids: vec![] });
    }
    let lanes: Vec<(i32, &Track)> = tl.video.iter().skip(1).enumerate().map(|(i, tr)| (i as i32 + 1, tr)).chain(tl.audio.iter().enumerate().map(|(i, tr)| (-(i as i32) - 1, tr))).collect();
    for (lane, tr) in lanes {
        for c in &tr.clips {
            let Some(hi) = spine.iter().position(|h| h.at - 1e-6 <= c.start && c.start < h.at + h.dur - 1e-6).or(spine.len().checked_sub(1)) else { continue };
            let id = asset(c);
            let h = &spine[hi];
            let el = format!("{}/>", clip_el(c, h.own + (c.start - h.at), Some(lane), &id));
            spine[hi].kids.push(el);
        }
    }
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n<fcpxml version=\"1.10\">\n<resources>\n<format id=\"r1\" frameDuration=\"{}\" width=\"{}\" height=\"{}\"/>\n",
        if ntsc { format!("1001/{}s", base * 1000) } else { format!("1/{base}s") },
        tl.width,
        tl.height
    );
    for (_, a) in &assets {
        s.push_str(a);
        s.push('\n');
    }
    s.push_str(&format!("</resources>\n<library>\n<event name=\"formvfx\">\n<project name=\"{}\">\n<sequence format=\"r1\" duration=\"{}\" tcStart=\"0s\" tcFormat=\"NDF\">\n<spine>\n", esc(&tl.name), t(end)));
    for h in spine {
        if h.kids.is_empty() {
            s.push_str(&format!("{}/>\n", h.head));
        } else {
            s.push_str(&format!("{}>\n{}\n</{}>\n", h.head, h.kids.join("\n"), h.tag));
        }
    }
    s.push_str("</spine>\n</sequence>\n</project>\n</event>\n</library>\n</fcpxml>\n");
    s
}

fn rt(sec: f64, fps: f64) -> Json {
    json!({"OTIO_SCHEMA": "RationalTime.1", "rate": fps, "value": frames(sec, fps) as f64})
}

fn range(start: f64, dur: f64, fps: f64) -> Json {
    json!({"OTIO_SCHEMA": "TimeRange.1", "start_time": rt(start, fps), "duration": rt(dur, fps)})
}

fn write_otio(tl: &Timeline) -> String {
    let fps = tl.fps;
    let track = |tr: &Track, kind: &str| -> Json {
        let mut kids = vec![];
        let mut pos = 0.0;
        for c in &tr.clips {
            if c.start > pos + 1e-6 {
                kids.push(json!({"OTIO_SCHEMA": "Gap.1", "name": "", "source_range": range(0.0, c.start - pos, fps), "effects": [], "markers": [], "metadata": {}}));
            }
            let effects = if (c.speed - 1.0).abs() > 1e-6 { json!([{"OTIO_SCHEMA": "LinearTimeWarp.1", "name": "", "effect_name": "LinearTimeWarp", "time_scalar": c.speed, "metadata": {}}]) } else { json!([]) };
            kids.push(json!({
                "OTIO_SCHEMA": "Clip.1",
                "name": c.name,
                "source_range": range(c.src_in, c.dur, fps),
                "media_reference": {"OTIO_SCHEMA": "ExternalReference.1", "name": c.file, "target_url": path_to_url(&c.path, ""), "available_range": if c.length > 0.0 { range(0.0, c.length, fps) } else { Json::Null }, "metadata": {}},
                "effects": effects,
                "markers": [],
                "enabled": c.enabled,
                "metadata": {}
            }));
            pos = c.start + c.dur;
        }
        json!({"OTIO_SCHEMA": "Track.1", "name": tr.name, "kind": kind, "children": kids, "effects": [], "markers": [], "enabled": true, "metadata": {}})
    };
    let children: Vec<Json> = tl.video.iter().map(|tr| track(tr, "Video")).chain(tl.audio.iter().map(|tr| track(tr, "Audio"))).collect();
    let doc = json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": tl.name,
        "global_start_time": Json::Null,
        "tracks": {"OTIO_SCHEMA": "Stack.1", "name": "tracks", "children": children, "effects": [], "markers": [], "enabled": true, "metadata": {}},
        "metadata": {"formvfx": {"fps": fps, "width": tl.width, "height": tl.height}}
    });
    serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n"
}

fn write_edl(tl: &mut Timeline) -> String {
    let fps = tl.fps;
    let (base, ntsc) = timebase(fps);
    if ntsc {
        tl.report.push(format!("{fps:.3} fps is written as {base} fps non-drop timecode"));
    }
    let mut events: Vec<(Clip, &str)> = tl.video.iter().flat_map(|t| t.clips.iter().map(|c| (c.clone(), "V"))).chain(tl.audio.iter().flat_map(|t| t.clips.iter().map(|c| (c.clone(), "A")))).collect();
    events.sort_by(|a, b| a.0.start.total_cmp(&b.0.start));
    let mut vend = f64::NEG_INFINITY;
    let mut s = format!("TITLE: {}\nFCM: NON-DROP FRAME\n\n", tl.name.chars().take(70).collect::<String>());
    let hour = 3600 * base as i64;
    let f = |x: f64| frames(x, fps);
    for (n, (c, ch)) in events.iter().enumerate() {
        if *ch == "V" {
            if c.start < vend - 1e-6 {
                tl.report.push(format!("{}: overlaps the clip before it; an EDL holds one video track", c.name));
            }
            vend = vend.max(c.start + c.dur);
        }
        let sin = f(c.src_in);
        s.push_str(&format!("{:03}  AX       {:<5} C        {} {} {} {}\n", n + 1, ch, tc(sin, base), tc(sin + f(c.dur * c.speed), base), tc(hour + f(c.start), base), tc(hour + f(c.start + c.dur), base)));
        if (c.speed - 1.0).abs() > 1e-6 {
            s.push_str(&format!("M2   AX       {:05.1}                {}\n", fps * c.speed, tc(sin, base)));
        }
        s.push_str(&format!("* FROM CLIP NAME: {}\n* SOURCE FILE: {}\n\n", if c.file.is_empty() { &c.name } else { &c.file }, c.path));
    }
    s
}

pub fn export_timeline(e: &mut Engine, p: &Json) -> Res {
    let cid = e.comp_id(p.get("comp"))?;
    let path = ps(p, "path").ok_or("pass `path` (.xml, .fcpxml, .otio or .edl)")?.to_string();
    let format = match ps(p, "format").unwrap_or("auto") {
        "auto" => match Path::new(&path).extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
            "fcpxml" => "fcpxml",
            "otio" => "otio",
            "edl" => "edl",
            "aaf" => "aaf",
            "omf" => "omf",
            _ => "xml",
        },
        "xml" => "xml",
        "fcpxml" => "fcpxml",
        "otio" => "otio",
        "edl" => "edl",
        "aaf" | "omf" => return Err(format!("{} {BINARY}", ps(p, "format").unwrap_or("").to_uppercase())),
        other => return Err(format!("`{other}` is not a timeline format (file.timelineFormats lists them)")),
    };
    if format == "aaf" || format == "omf" {
        return Err(format!("{} {BINARY}", format.to_uppercase()));
    }
    let mode = match ps(p, "prerender").unwrap_or("none") {
        m @ ("none" | "unsupported" | "all") => m,
        other => return Err(format!("prerender is none, unsupported or all, not `{other}`")),
    };
    let mode = if ps(p, "precomps") == Some("prerender") && mode == "none" { "unsupported" } else { mode };
    let target = PathBuf::from(&path);
    let dir = target.parent().map(Path::to_path_buf).unwrap_or_default().join(format!("{} Prerenders", target.file_stem().and_then(|s| s.to_str()).unwrap_or("Timeline")));
    let mut tl = collect(e, cid, mode, &dir)?;
    let text = match format {
        "fcpxml" => write_fcpxml(&mut tl),
        "otio" => write_otio(&tl),
        "edl" => write_edl(&mut tl),
        _ => write_xmeml(&tl),
    };
    if let Some(d) = target.parent() {
        std::fs::create_dir_all(d).map_err(|x| format!("{}: {x}", d.display()))?;
    }
    std::fs::write(&target, text).map_err(|x| format!("{path}: {x}"))?;
    Ok(json!({"path": path, "format": format, "clips": tl.clips(), "videoTracks": tl.video.len(), "audioTracks": tl.audio.len(), "duration": tl.end(), "report": tl.report}))
}
