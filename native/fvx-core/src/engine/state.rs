use std::collections::HashSet;
use std::sync::Arc;

use serde_json::{Value as Json, json};

use super::{Engine, pf, ps};
use crate::eval::Eval;
use crate::geom::Mat3;
use crate::model::key::same_time;
use crate::model::path::BezPath;
use crate::model::prop::{Group, GroupKind, Node, Prop, Ui};
use crate::model::{Interp, Item, ItemKind, Layer, LayerKind, Marker, Project, label_name, label_rgb};
use crate::render::{Media, RenderOpts, Renderer, audio, content_bounds, shapes};

fn interp(i: Interp, auto: bool) -> &'static str {
    match i {
        Interp::Linear => "linear",
        Interp::Hold => "hold",
        Interp::Bezier if auto => "auto",
        Interp::Bezier => "bezier",
    }
}

pub fn marker_json(m: &Marker) -> Json {
    json!({
        "time": m.time, "duration": m.duration, "comment": m.comment, "label": label_name(m.label), "color": label_rgb(m.label), "chapter": m.chapter, "protected": m.protected,
        "url": m.url, "frameTarget": m.frame_target,
        "cuePoint": m.cue_point.as_ref().map(|c| json!({"name": c.name, "navigation": c.navigation, "params": c.params})),
    })
}

pub fn item_json(i: &Item) -> Json {
    let mut o = json!({"id": i.id, "name": i.name, "type": i.type_name(), "label": label_name(i.label), "color": label_rgb(i.label), "parent": i.parent, "comment": i.comment, "proxy": i.proxy.as_ref().is_some_and(|p| p.enabled)});
    if let Some(p) = &i.proxy {
        o["proxyInfo"] = json!({"path": p.footage.path, "enabled": p.enabled, "missing": p.footage.missing, "size": [p.footage.width, p.footage.height]});
    }
    if let Some((w, h)) = i.dimensions() {
        o["size"] = json!([w, h]);
    }
    if let Some(d) = i.duration() {
        o["duration"] = json!(d);
    }
    if let Some(fr) = i.frame_rate() {
        o["frameRate"] = json!(fr);
    }
    match &i.kind {
        ItemKind::Comp(c) => o["layers"] = json!(c.layers.len()),
        ItemKind::Footage(f) => {
            o["footage"] = json!({"path": f.path, "missing": f.missing, "hasVideo": f.has_video, "hasAudio": f.has_audio, "codec": f.codec, "kind": format!("{:?}", f.kind), "pixelAspect": f.pixel_aspect, "frames": f.sequence.len(), "layer": f.layer_name, "page": f.page + 1});
        }
        ItemKind::Solid(s) => o["solid"] = json!({"color": s.color}),
        ItemKind::Folder => {}
    }
    o
}

fn layer_has_audio(p: &Project, l: &Layer) -> bool {
    if audio::layer_makes_sound(l) {
        return true;
    }
    match l.source.and_then(|s| p.item(s)).map(|i| &i.kind) {
        Some(ItemKind::Footage(f)) => f.has_audio,
        Some(ItemKind::Comp(_)) => audio::comp_has_audio(p, l.source.unwrap_or(0)),
        _ => false,
    }
}

fn layer_row(p: &Project, ev: &Eval, l: &Layer, index: usize, selected: bool) -> Json {
    let src = l.source.and_then(|s| p.item(s));
    let effects: Vec<Json> = l.effects().map(|g| g.groups().map(|e| json!({"uid": e.uid, "name": e.name, "effect": match &e.kind { GroupKind::Effect { effect } => effect.as_str(), _ => "" }, "enabled": e.enabled})).collect()).unwrap_or_default();
    let has_video = match src.map(|i| &i.kind) {
        Some(ItemKind::Footage(f)) => f.has_video,
        Some(ItemKind::Comp(_)) => true,
        _ => false,
    };
    json!({
        "id": l.id, "index": index, "name": l.name, "sourceName": src.map(|i| i.name.clone()), "sourceItem": l.source,
        "type": if l.kind == LayerKind::Solid { "Solid" } else { l.type_name() }, "label": label_name(l.label), "color": label_rgb(l.label), "comment": l.comment,
        "in": l.in_point, "out": l.out_point, "start": l.start_time, "stretch": l.stretch,
        "switches": serde_json::to_value(&l.switches).unwrap_or_default(), "blendMode": l.blend.label(), "preserveTransparency": l.preserve_transparency,
        "trackMatte": l.track_matte.map(|m| json!({"layer": m.layer, "kind": m.kind.label()})), "parent": l.parent, "selected": selected,
        "active": l.is_active_at(ev.t), "hasVideo": has_video && l.has_av(), "hasAudio": layer_has_audio(p, l), "is3d": l.is_3d(),
        "camera": l.is_camera(), "light": l.is_light(), "markers": l.markers.iter().map(marker_json).collect::<Vec<_>>(),
        "effects": effects, "masks": l.masks().map(|m| m.children.len()).unwrap_or(0), "animated": l.props.any_animated(),
        "opacity": ev.tr_f(l, "opacity", 100.0), "autoOrient": format!("{:?}", l.auto_orient),
    })
}

struct Walk<'a, 'b> {
    ev: &'a Eval<'b>,
    layer: &'a Layer,
    sel_props: &'a HashSet<(u64, u64)>,
    sel_keys: &'a [super::KeyRef],
}

impl Walk<'_, '_> {
    fn group_visible(&self, g: &Group) -> bool {
        match g.id.as_str() {
            "masks" | "effects" | "animators" => !g.children.is_empty(),
            "materialOptions" => self.layer.is_3d(),
            "audio" => true,
            _ => true,
        }
    }

    fn prop_visible(&self, p: &Prop) -> bool {
        if matches!(p.ui, Ui::Hidden) {
            return false;
        }
        if p.three_d_only && !self.layer.is_3d() {
            return false;
        }
        if p.two_d_only && self.layer.is_3d() {
            return false;
        }
        if p.id == "poi" && self.layer.auto_orient != crate::model::AutoOrient::TowardsPointOfInterest {
            return false;
        }
        if p.id == "position" && self.layer.transform().is_some_and(|t| t.get("xPosition").is_some() && t.get("position").is_some_and(|x| x.uid == p.uid)) {
            return false;
        }
        true
    }

    fn group(&self, g: &Group) -> Json {
        let mut children = vec![];
        for c in &g.children {
            match c {
                Node::Prop(p) => {
                    if self.prop_visible(p) {
                        children.push(self.prop(p, g));
                    }
                }
                Node::Group(sub) => {
                    if self.group_visible(sub) {
                        let mut o = self.group(sub);
                        if matches!(g.kind, GroupKind::Indexed) && o.get("kind").is_none() {
                            o["kind"] = json!("indexed");
                        }
                        children.push(o);
                    }
                }
            }
        }
        let mut o = json!({"k": "g", "uid": g.uid, "match": g.id, "name": g.name, "enabled": g.enabled, "selected": self.sel_props.contains(&(self.layer.id, g.uid)), "children": children});
        match &g.kind {
            GroupKind::Plain | GroupKind::Indexed => {}
            GroupKind::Mask { mode, inverted, color, locked } => {
                o["kind"] = json!("mask");
                o["mask"] = json!({"mode": mode.label(), "inverted": inverted, "color": color, "locked": locked});
            }
            GroupKind::Effect { effect } => {
                o["kind"] = json!("effect");
                o["effect"] = json!(effect);
                o["effectName"] = json!(crate::effects::find(effect).map(|e| e.name).unwrap_or(""));
            }
            GroupKind::Tracker => o["kind"] = json!("tracker"),
            GroupKind::Essential => o["kind"] = json!("essential"),
        }
        o
    }

    fn prop(&self, p: &Prop, parent: &Group) -> Json {
        let l = self.layer;
        let v = self.ev.value(l, p);
        let keys: Vec<Json> = p
            .keys
            .iter()
            .map(|k| {
                let sel = self.sel_keys.iter().any(|s| s.layer == l.id && s.prop == p.uid && same_time(s.time, k.time));
                json!([l.comp_time(k.time), sel, interp(k.in_interp, k.auto_bezier), interp(k.out_interp, k.auto_bezier), k.time, k.roving, k.label])
            })
            .collect();
        let lt = l.layer_time(self.ev.t);
        let on_key = p.keys.iter().any(|k| same_time(k.time, lt));
        let dims = if parent.id == "transform" && matches!(p.id.as_str(), "anchor" | "position" | "scale") && !l.is_camera() && !l.is_light() {
            if l.is_3d() { 3 } else { 2 }
        } else {
            v.dims()
        };
        let mut o = json!({
            "k": "p", "uid": p.uid, "match": p.id, "name": if parent.id == "transform" && p.id == "rotation" && l.is_3d() && !l.is_camera() { "Z Rotation".to_string() } else { p.name.clone() },
            "vt": v.kind_name(), "v": v.to_json(), "ui": serde_json::to_value(&p.ui).unwrap_or_default(),
            "animated": p.is_animated(), "onKey": on_key, "keys": keys, "spatial": p.spatial, "static": p.static_only, "hold": p.hold_only, "dims": dims,
            "selected": self.sel_props.contains(&(l.id, p.uid)),
        });
        if parent.id == "pathOptions" && p.id == "path" {
            let mut opts = vec!["None".to_string()];
            opts.extend(l.masks().map(|m| m.groups().map(|g| g.name.clone()).collect::<Vec<_>>()).unwrap_or_default());
            o["ui"] = json!({"kind": "Popup", "options": opts});
        }
        if let Some(e) = &p.expr {
            o["expr"] = json!({"text": e.text, "enabled": e.enabled, "error": crate::expr::check(&self.ev, l, p)});
        }
        if let Some(td) = v.as_text() {
            o["doc"] = json!({
                "font": td.font, "style": td.style, "size": td.size, "fill": td.fill, "stroke": td.stroke, "strokeWidth": td.stroke_width,
                "applyFill": td.apply_fill, "applyStroke": td.apply_stroke, "tracking": td.tracking, "leading": td.leading.unwrap_or(td.size * 1.2),
                "justify": format!("{:?}", td.justify), "fauxBold": td.faux_bold, "fauxItalic": td.faux_italic, "allCaps": td.all_caps, "smallCaps": td.small_caps,
                "box": td.box_size, "indentLeft": td.indent_left, "indentRight": td.indent_right,
            });
        }
        o
    }
}

impl Engine {
    pub fn ui_state(&self, p: &Json) -> Json {
        let trees: Vec<u64> = p.get("trees").and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_u64).collect()).unwrap_or_default();
        let pj = &self.project;
        let mut out = json!({
            "revision": self.revision, "dirty": self.is_dirty(), "path": self.path,
            "bitDepth": format!("{} bpc", pj.settings.bit_depth), "settings": serde_json::to_value(&pj.settings).unwrap_or_default(),
            "undo": self.history.undo.last().map(|s| s.label.clone()), "redo": self.history.redo.last().map(|s| s.label.clone()),
            "activeComp": self.state.active_comp, "openComps": self.state.open_comps, "projectSelection": self.state.project_selection,
            "items": pj.items.iter().map(item_json).collect::<Vec<_>>(),
            "selection": {
                "layers": self.state.selected_layers, "props": self.state.selected_props.iter().map(|(l, u)| json!([l, u])).collect::<Vec<_>>(),
                "vertices": self.state.selected_vertices.iter().map(|v| json!({"layer": v.layer, "mask": v.mask, "index": v.index})).collect::<Vec<_>>(),
            },
            "snapping": self.state.snapping, "roi": self.state.roi, "viewLayout": self.state.view_layout, "snapshot": super::cmd::viewer::snapshot_json(self), "shareViewOptions": self.state.viewer.share_view_options,
            "lockedViewer": super::cmd::viewer::locked_json(self),
            "viewer": {"channel": self.state.viewer.channel, "colorized": self.state.viewer.colorized, "exposure": self.state.viewer.exposure, "show_snapshot": self.state.show_snapshot, "custom_rgb": self.state.viewer.custom_rgb, "fast_previews": self.state.viewer.fast_previews, "extended": self.state.viewer.extended, "scrub_audio": self.state.viewer.scrub_audio, "playback": self.state.viewer.playback},
            "textEdit": self.state.text_edit.as_ref().map(|t| json!({"layer": t.layer, "anchor": t.anchor, "caret": t.caret})),
            "rendering": self.render.busy(), "comp": null, "layers": [], "trees": {}, "engine": "fvx",
        });
        let Some(cid) = self.state.active_comp else { return out };
        let Some(comp) = pj.comp(cid) else { return out };
        let t = self.time_of(cid);
        let ev = Eval::new(pj, cid, comp, t);
        let name = pj.item(cid).map(|i| i.name.clone()).unwrap_or_default();
        out["time"] = json!(t);
        out["comp"] = json!({
            "id": cid, "name": name, "width": comp.width, "height": comp.height, "pixelAspect": comp.pixel_aspect, "frameRate": comp.frame_rate,
            "frameDuration": comp.frame_duration(), "duration": comp.duration, "displayStart": comp.display_start, "background": comp.background,
            "workArea": comp.work_area, "hideShy": comp.hide_shy, "motionBlur": comp.motion_blur, "frameBlending": comp.frame_blending, "draft3d": comp.draft_3d,
            "renderer": if comp.advanced_3d() { "Advanced3D" } else { "Classic3D" }, "has3d": comp.has_3d(), "view3d": super::cmd::view3d::state_name(super::cmd::view3d::current(self, cid)), "markers": comp.markers.iter().map(marker_json).collect::<Vec<_>>(),
            "guides": comp.guides.iter().map(|g| json!({"vertical": g.vertical, "position": g.position})).collect::<Vec<_>>(),
            "hasAudio": audio::comp_has_audio(pj, cid),
        });
        let sel: HashSet<u64> = self.state.selected_layers.iter().copied().collect();
        let sel_props: HashSet<(u64, u64)> = self.state.selected_props.iter().copied().collect();
        out["selection"]["keys"] = json!(self.state.selected_keys.iter().filter_map(|k| comp.layer(k.layer).map(|l| json!({"layer": k.layer, "prop": k.prop, "time": l.comp_time(k.time), "layerTime": k.time}))).collect::<Vec<_>>());
        out["layers"] = json!(comp.layers.iter().enumerate().map(|(i, l)| layer_row(pj, &ev, l, i + 1, sel.contains(&l.id))).collect::<Vec<_>>());
        let mut tm = serde_json::Map::new();
        for id in trees {
            if let Some(l) = comp.layer(id) {
                let w = Walk { ev: &ev, layer: l, sel_props: &sel_props, sel_keys: &self.state.selected_keys };
                tm.insert(id.to_string(), w.group(&l.props));
            }
        }
        out["trees"] = Json::Object(tm);
        out
    }

    pub fn overlays(&self, p: &Json) -> Result<Json, String> {
        let cid = self.comp_id(p.get("comp"))?;
        let pj = &self.project;
        let comp = pj.comp(cid).ok_or("no composition")?;
        let t = pf(p, "time").unwrap_or_else(|| self.time_of(cid));
        let ev = Eval::new(pj, cid, comp, t);
        let view = ps(p, "view3d").or_else(|| ps(p, "view")).and_then(super::cmd::view3d::normalize).unwrap_or_else(|| super::cmd::view3d::current(self, cid));
        let cam = comp.has_3d().then(|| super::cmd::view3d::view_camera(self, cid, view).unwrap_or_else(|| ev.camera()));
        let sel: HashSet<u64> = self.state.selected_layers.iter().copied().collect();
        let any_solo = comp.layers.iter().any(|l| l.switches.solo);
        let mut layers = vec![];
        let mut paths = vec![];
        let mut motion = vec![];
        let mut rigs = vec![];
        let (cw, ch) = (comp.width as f64, comp.height as f64);
        for l in &comp.layers {
            let active = l.is_active_at(t);
            if !active && !sel.contains(&l.id) {
                continue;
            }
            let visible = (l.switches.video && (!any_solo || l.switches.solo)) || l.is_camera() || l.is_light();
            let m = match &cam {
                Some(c) if l.is_3d() && !l.is_camera() && !l.is_light() => ev.layer_to_screen(l, c),
                _ => ev.layer_to_comp(l),
            };
            let quad = content_bounds(&ev, l).map(|b| [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]].map(|q| m.apply(q[0], q[1])));
            let anchor = l.transform().map(|_| {
                let a = ev.tr_v3(l, "anchor", [0.0; 3]);
                m.apply(a[0], a[1])
            });
            let position = l.transform().map(|_| {
                let p3 = ev.tr_v3(l, "position", [0.0; 3]);
                match &cam {
                    Some(c) if l.is_3d() => {
                        let w = ev.parent_matrix(l).apply(p3);
                        c.project(cw, ch, w).unwrap_or([p3[0], p3[1]])
                    }
                    _ => {
                        let pm = ev.parent_to_comp(l);
                        pm.apply(p3[0], p3[1])
                    }
                }
            });
            let selected = sel.contains(&l.id);
            let mut axes = Json::Null;
            if l.is_3d()
                && selected
                && let Some(c) = &cam
            {
                let a = ev.tr_v3(l, "anchor", [0.0; 3]);
                let w = ev.world_matrix(l);
                let len = pf(p, "axisLength").unwrap_or(60.0);
                let o = w.apply(a);
                let proj = |q: [f64; 3]| c.project(cw, ch, q);
                let ends: Option<Vec<[f64; 2]>> = [[len, 0.0, 0.0], [0.0, len, 0.0], [0.0, 0.0, len]].iter().map(|d| proj(w.apply([a[0] + d[0], a[1] + d[1], a[2] + d[2]]))).collect();
                if let (Some(origin), Some(ends)) = (proj(o), ends) {
                    axes = json!({"origin": origin, "ends": ends, "parent": [[1, 0, 0], [0, 1, 0], [0, 0, 1]], "position": ev.tr_v3(l, "position", [0.0; 3])});
                }
            }
            let to_parent = if l.is_3d() { None } else { ev.parent_to_comp(l).inverse().map(|i| i.0) };
            layers.push(json!({
                "id": l.id, "toLocal": m.inverse().map(|i| i.0), "toParent": to_parent, "quad": quad, "anchor": anchor, "position": position,
                "selected": selected, "locked": l.switches.locked, "visible": visible && active, "is3d": l.is_3d(), "camera": l.is_camera(), "light": l.is_light(),
                "label": label_rgb(l.label), "axes": axes, "features": null,
            }));
            if !selected {
                continue;
            }
            if let Some(masks) = l.masks() {
                for g in masks.groups() {
                    let GroupKind::Mask { color, locked, mode, inverted } = &g.kind else { continue };
                    if *locked && self.state.hide_locked_masks {
                        continue;
                    }
                    let Some(bp) = g.get("path").map(|pr| ev.value(l, pr)).and_then(|v| v.as_path().cloned()) else { continue };
                    paths.push(path_json(&bp, &m, l.id, g.uid, &g.name, true, *color, &self.state.selected_vertices, json!({"mode": mode.label(), "inverted": inverted, "locked": locked})));
                }
            }
            if l.kind == LayerKind::Shape {
                for (uid, bp) in shapes::shape_paths(&ev, l) {
                    let name = l.props.find_group(uid).map(|g| g.name.clone()).unwrap_or_default();
                    paths.push(path_json(&bp, &m, l.id, uid, &name, false, label_rgb(l.label), &self.state.selected_vertices, Json::Null));
                }
            }
            if let Some(pos) = l.transform().and_then(|tr| tr.get("position"))
                && pos.keys.len() >= 2
                && !l.is_3d()
            {
                motion.push(motion_json(&ev, l, pos, &self.state.selected_keys));
            }
            if (l.is_camera() || l.is_light())
                && let Some(c) = &cam
            {
                let w = ev.world_matrix(l);
                let eye = w.apply([0.0; 3]);
                let fwd = w.apply([0.0, 0.0, if l.is_camera() { 400.0 } else { 200.0 }]);
                rigs.push(json!({"layer": l.id, "kind": if l.is_camera() { "camera" } else { "light" }, "eye": c.project(cw, ch, eye), "aim": c.project(cw, ch, fwd)}));
            }
        }
        Ok(json!({
            "comp": cid, "time": t, "width": comp.width, "height": comp.height, "pixelAspect": comp.pixel_aspect, "view": view,
            "layers": layers, "paths": paths, "motion": motion, "rigs": rigs,
            "guides": comp.guides.iter().map(|g| json!({"vertical": g.vertical, "position": g.position})).collect::<Vec<_>>(), "roi": self.state.roi,
        }))
    }

    pub fn graph(&self, p: &Json) -> Result<Json, String> {
        let cid = self.comp_id(p.get("comp"))?;
        let comp = self.project.comp(cid).ok_or("no composition")?;
        let lid = p.get("layer").and_then(Json::as_u64).ok_or("missing layer")?;
        let l = comp.layer(lid).ok_or("no such layer")?;
        let t0 = pf(p, "t0").unwrap_or(0.0);
        let t1 = pf(p, "t1").unwrap_or(comp.duration);
        let n = p.get("samples").and_then(Json::as_u64).unwrap_or(200).clamp(2, 4000) as usize;
        let uids: Vec<u64> = p.get("props").and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_u64).collect()).unwrap_or_default();
        let mut out = vec![];
        for uid in uids {
            let Some(pr) = l.props.find(uid) else { continue };
            let dims = pr.value.dims().max(1);
            let mut values = Vec::with_capacity(n);
            let mut speeds = Vec::with_capacity(n);
            let h = (t1 - t0).abs() / n as f64 * 0.5 + 1e-4;
            for i in 0..n {
                let ct = t0 + (t1 - t0) * i as f64 / (n - 1) as f64;
                let ev = Eval::new(&self.project, cid, comp, ct);
                let v = ev.value(l, pr).components();
                let mut row = vec![ct];
                row.extend(v.iter().take(dims));
                values.push(row);
                let a = Eval::new(&self.project, cid, comp, ct - h).value(l, pr);
                let b = Eval::new(&self.project, cid, comp, ct + h).value(l, pr);
                let (ca, cb) = (a.components(), b.components());
                let mut srow = vec![ct];
                if pr.spatial {
                    let d: f64 = ca.iter().zip(&cb).map(|(x, y)| (y - x) * (y - x)).sum::<f64>().sqrt();
                    srow.push(d / (2.0 * h));
                } else {
                    srow.extend(ca.iter().zip(&cb).take(dims).map(|(x, y)| (y - x) / (2.0 * h)));
                }
                speeds.push(srow);
            }
            let sel: Vec<f64> = self.state.selected_keys.iter().filter(|k| k.layer == lid && k.prop == uid).map(|k| k.time).collect();
            let edims = crate::model::key::ease_dims(&pr.value, pr.spatial);
            let keys: Vec<Json> = pr
                .keys
                .iter()
                .enumerate()
                .map(|(i, k)| {
                    let side = |d: usize, out: bool| crate::model::key::side_ease(&pr.keys, i, d, pr.spatial, out).map(|e| json!({"speed": e.speed, "influence": e.influence})).unwrap_or(Json::Null);
                    json!({
                        "time": l.comp_time(k.time), "layerTime": k.time, "value": k.value.components(),
                        "in": interp(k.in_interp, k.auto_bezier), "out": interp(k.out_interp, k.auto_bezier),
                        "inEase": (0..edims).map(|d| side(d, false)).collect::<Vec<_>>(), "outEase": (0..edims).map(|d| side(d, true)).collect::<Vec<_>>(),
                        "selected": sel.iter().any(|s| same_time(*s, k.time)),
                    })
                })
                .collect();
            out.push(json!({"uid": uid, "name": pr.name, "dims": dims, "spatial": pr.spatial, "values": values, "speeds": speeds, "keys": keys}));
        }
        Ok(json!({"layer": lid, "t0": t0, "t1": t1, "props": out}))
    }

    pub fn flowchart(&self, p: &Json) -> Result<Json, String> {
        let root = self.comp_id(p.get("comp"))?;
        let b = |k: &str, d: bool| p.get(k).and_then(Json::as_bool).unwrap_or(d);
        let (with_layers, with_fx, solids) = (b("layers", false), b("effects", false), b("solids", true));
        let mut nodes = vec![];
        let mut edges = vec![];
        let mut seen = HashSet::new();
        let mut stack = vec![root];
        while let Some(cid) = stack.pop() {
            if !seen.insert(cid) {
                continue;
            }
            let Some(item) = self.project.item(cid) else { continue };
            let ItemKind::Comp(c) = &item.kind else { continue };
            nodes.push(json!({"id": format!("i{cid}"), "kind": "comp", "item": cid, "name": item.name}));
            for l in &c.layers {
                let src = l.source.and_then(|s| self.project.item(s));
                let from = if with_layers {
                    let lid = format!("l{}", l.id);
                    let fx: Vec<String> = if with_fx { l.effects().map(|g| g.groups().map(|e| e.name.clone()).collect()).unwrap_or_default() } else { vec![] };
                    nodes.push(json!({"id": lid, "kind": "layer", "layer": l.id, "comp": cid, "name": l.name, "type": l.type_name(), "effects": fx}));
                    edges.push(json!({"from": lid, "to": format!("i{cid}")}));
                    lid
                } else {
                    format!("i{cid}")
                };
                if let Some(s) = src {
                    if matches!(s.kind, ItemKind::Solid(_)) && !solids {
                        continue;
                    }
                    let sid = format!("i{}", s.id);
                    if !nodes.iter().any(|n| n["id"] == json!(sid)) && !matches!(s.kind, ItemKind::Comp(_)) {
                        nodes.push(json!({"id": sid, "kind": match s.kind { ItemKind::Solid(_) => "solid", _ => "footage" }, "item": s.id, "name": s.name}));
                    }
                    edges.push(json!({"from": sid, "to": from}));
                    if matches!(s.kind, ItemKind::Comp(_)) {
                        stack.push(s.id);
                    }
                }
            }
        }
        Ok(json!({"root": root, "nodes": nodes, "edges": edges}))
    }

    pub fn menu_titles(&self) -> Json {
        json!(super::menus::TITLES)
    }
}

#[allow(clippy::too_many_arguments)]
fn path_json(bp: &BezPath, m: &Mat3, layer: u64, uid: u64, name: &str, is_mask: bool, color: [u8; 3], sel: &[super::VertexRef], extra: Json) -> Json {
    let bp = bp.clone().normalized();
    let ap = |q: [f64; 2]| m.apply(q[0], q[1]);
    let pts: Vec<[f64; 2]> = bp.flatten(0.5).into_iter().map(ap).collect();
    let pts = if bp.closed && !bp.vertices.is_empty() {
        let mut v = pts;
        v.push(ap(bp.vertices[0]));
        v
    } else {
        pts
    };
    let vertices: Vec<[f64; 2]> = bp.vertices.iter().map(|v| ap(*v)).collect();
    let ins: Vec<Option<[f64; 2]>> = bp.vertices.iter().enumerate().map(|(i, v)| (bp.ins[i] != [0.0; 2]).then(|| ap([v[0] + bp.ins[i][0], v[1] + bp.ins[i][1]]))).collect();
    let outs: Vec<Option<[f64; 2]>> = bp.vertices.iter().enumerate().map(|(i, v)| (bp.outs[i] != [0.0; 2]).then(|| ap([v[0] + bp.outs[i][0], v[1] + bp.outs[i][1]]))).collect();
    let selected: Vec<usize> = sel.iter().filter(|x| x.layer == layer && x.mask == uid).map(|x| x.index).collect();
    json!({"layer": layer, "uid": uid, "name": name, "mask": is_mask, "color": color, "closed": bp.closed, "pts": pts, "vertices": vertices, "in": ins, "out": outs, "selected": selected, "toLocal": m.inverse().map(|i| i.0), "toComp": m.0, "info": extra})
}

fn motion_json(ev: &Eval, l: &Layer, pos: &Prop, sel: &[super::KeyRef]) -> Json {
    let p2c = ev.parent_to_comp(l);
    let ap = |v: [f64; 3]| p2c.apply(v[0], v[1]);
    let fd = ev.comp.frame_duration();
    let first = pos.keys.first().map(|k| k.time).unwrap_or(0.0);
    let last = pos.keys.last().map(|k| k.time).unwrap_or(0.0);
    let mut pts = vec![];
    let mut lt = first;
    while lt <= last && pts.len() < 20_000 {
        pts.push(ap(pos.at(lt).vec3()));
        lt += fd;
    }
    pts.push(ap(pos.at(last).vec3()));
    let keys: Vec<Json> = pos
        .keys
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let v = k.value.vec3();
            let (tin, tout) = crate::model::key::spatial_tangents(&pos.keys, i);
            let has_in = i > 0 && (tin[0] != 0.0 || tin[1] != 0.0);
            let has_out = i + 1 < pos.keys.len() && (tout[0] != 0.0 || tout[1] != 0.0);
            json!({
                "time": k.time, "compTime": l.comp_time(k.time), "p": ap(v), "value": v,
                "selected": sel.iter().any(|s| s.layer == l.id && s.prop == pos.uid && same_time(s.time, k.time)),
                "in": if has_in { json!(ap([v[0] + tin[0], v[1] + tin[1], v[2] + tin[2]])) } else { Json::Null },
                "out": if has_out { json!(ap([v[0] + tout[0], v[1] + tout[1], v[2] + tout[2]])) } else { Json::Null },
            })
        })
        .collect();
    json!({"layer": l.id, "prop": pos.uid, "pts": pts, "keys": keys, "toParent": p2c.inverse().map(|i| i.0)})
}

pub struct FrameJob {
    project: Arc<Project>,
    media: Arc<Media>,
    comp: u64,
    t: f64,
    scale: f64,
    bg: [f32; 3],
    transparent: bool,
    guides: bool,
    channel: String,
    exposure: f64,
    custom_rgb: [bool; 3],
    fast: String,
    view: Option<crate::eval::Camera>,
    display: Option<Arc<crate::render::ocio::Proc>>,
    cache_key: Option<String>,
    backend: crate::render::gpu::Backend,
    pub rev: u64,
}

impl FrameJob {
    pub fn new(e: &Engine, comp: u64, t: f64, scale: f64, transparent: bool, guides: bool) -> FrameJob {
        let pipe = crate::render::color::Pipe::of(&e.project.settings);
        let mut bg = e.project.comp(comp).map(|c| c.background).unwrap_or_default();
        if let Some(a) = &pipe.authored {
            bg = a.apply32(bg);
        }
        let display = pipe.display_proc(&crate::engine::cmd::color::display_of(e));
        FrameJob { project: e.project.clone(), media: e.media.clone(), comp, t, scale, bg, transparent, guides, channel: "RGB".into(), exposure: 0.0, custom_rgb: [true; 3], fast: "off".into(), view: None, display, cache_key: None, backend: crate::render::gpu::Backend::Auto, rev: e.revision }
    }

    fn keyed(mut self, e: &Engine) -> FrameJob {
        if crate::render::diskcache::enabled() {
            let base = crate::render::diskcache::closure_hash(&self.project, self.comp);
            let disp = format!("{:?}|{}", crate::engine::cmd::color::display_of(e), e.project.settings.renderer);
            let text = format!("{base}|{:.6}|{:.5}|{}|{}|{:?}|{}|{:?}|{disp}|{}|{:?}|{}", self.t, self.scale, self.guides, self.transparent, self.bg, self.channel, self.view, self.exposure, self.custom_rgb, self.fast);
            self.cache_key = Some(crate::render::roto::sha256_hex(text.as_bytes()));
        }
        self
    }

    pub fn from_params(e: &Engine, p: &Json) -> Result<FrameJob, String> {
        let cid = e.comp_id(p.get("comp"))?;
        let comp = e.project.comp(cid).ok_or("no composition")?;
        let t = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
        let max_side = p.get("maxSide").and_then(Json::as_f64).unwrap_or(0.0);
        let mut scale = pf(p, "scale").unwrap_or(1.0).clamp(0.01, 1.0);
        let long = comp.width.max(comp.height).max(1) as f64;
        if max_side > 0.0 {
            scale = scale.min(max_side / long);
        }
        let display = p.get("display").and_then(Json::as_bool).unwrap_or(true);
        let mut j = FrameJob::new(e, cid, t, scale, p.get("transparent").and_then(Json::as_bool).unwrap_or(false), p.get("guides").and_then(Json::as_bool).unwrap_or(true));
        if display {
            j.channel = e.state.viewer.channel.clone();
            j.exposure = e.state.viewer.exposure;
            j.custom_rgb = e.state.viewer.custom_rgb;
            j.fast = ps(p, "fastPreviews").map(str::to_string).unwrap_or_else(|| e.state.viewer.fast_previews.clone());
        } else {
            let plain = crate::render::color::Display { profile: "srgb".into(), simulate: "none".into(), ..Default::default() };
            j.display = crate::render::color::Pipe::of(&e.project.settings).display_proc(&plain);
        }
        let view = match ps(p, "view3d").or_else(|| ps(p, "view")) {
            Some(v) => crate::engine::cmd::view3d::normalize(v).unwrap_or("activeCamera"),
            None if display => crate::engine::cmd::view3d::current(e, cid),
            None => "activeCamera",
        };
        if comp.has_3d() {
            j.view = crate::engine::cmd::view3d::view_camera(e, cid, view);
        }
        if let Some(b) = ps(p, "backend") {
            j.backend = crate::render::gpu::Backend::parse(b);
        }
        let cached = p.get("cache").and_then(Json::as_bool).unwrap_or(true) && j.backend == crate::render::gpu::Backend::Auto;
        Ok(if cached { j.keyed(e) } else { j })
    }

    pub fn run(&self) -> (u32, u32, Vec<u8>) {
        if let Some(k) = &self.cache_key
            && let Some(hit) = crate::render::diskcache::get(k)
        {
            return hit;
        }
        let started = std::time::Instant::now();
        let out = self.render();
        if let Some(k) = &self.cache_key
            && started.elapsed().as_millis() >= 40
        {
            crate::render::diskcache::put(k, out.0, out.1, &out.2);
        }
        out
    }

    fn render(&self) -> (u32, u32, Vec<u8>) {
        let draft = matches!(self.fast.as_str(), "draft" | "fastDraft");
        let wire;
        let project: &Project = if self.fast == "wireframe" {
            let mut pj = (*self.project).clone();
            for it in pj.items.iter_mut() {
                if let crate::model::ItemKind::Comp(c) = &mut it.kind {
                    for l in c.layers.iter_mut() {
                        l.switches.quality = crate::model::Quality::Wireframe;
                    }
                }
            }
            wire = pj;
            &wire
        } else {
            &self.project
        };
        let r = Renderer::new(project, &self.media, RenderOpts { scale: self.scale, guides: self.guides, draft, motion_blur: self.fast != "fastDraft", roi: None, view: self.view, backend: self.backend });
        let mut img = r.comp_frame(self.comp, self.t);
        if self.exposure.abs() > 1e-9 {
            let k = 2f32.powf(self.exposure as f32);
            for c in img.px.chunks_exact_mut(4) {
                c[0] *= k;
                c[1] *= k;
                c[2] *= k;
            }
        }
        let mut px = match &self.display {
            None if self.transparent => img.to_rgba8(),
            None => img.to_rgba8_over(self.bg),
            Some(d) => {
                let mut shown = if self.transparent { img.clone() } else { img.flattened(self.bg) };
                d.apply_img(&mut shown);
                shown.to_rgba8()
            }
        };
        match self.channel.as_str() {
            "Red" | "Green" | "Blue" => {
                let k = match self.channel.as_str() {
                    "Red" => 0,
                    "Green" => 1,
                    _ => 2,
                };
                for c in px.chunks_exact_mut(4) {
                    let v = c[k];
                    c[0] = v;
                    c[1] = v;
                    c[2] = v;
                    c[3] = 255;
                }
            }
            "Alpha" => {
                let a: Vec<u8> = img.px.chunks_exact(4).map(|p| crate::render::image::to8(p[3])).collect();
                for (c, v) in px.chunks_exact_mut(4).zip(a) {
                    c[0] = v;
                    c[1] = v;
                    c[2] = v;
                    c[3] = 255;
                }
            }
            "Custom" => {
                for c in px.chunks_exact_mut(4) {
                    for k in 0..3 {
                        if !self.custom_rgb[k] {
                            c[k] = 0;
                        }
                    }
                }
            }
            _ => {}
        }
        (img.w as u32, img.h as u32, px)
    }
}

pub struct AudioJob {
    project: Arc<Project>,
    media: Arc<Media>,
    comp: u64,
    start: f64,
    frames: usize,
    rate: u32,
}

impl AudioJob {
    pub fn from_params(e: &Engine, p: &Json) -> Result<AudioJob, String> {
        let cid = e.comp_id(p.get("comp"))?;
        let rate = p.get("rate").and_then(Json::as_u64).unwrap_or(48000).clamp(8000, 192000) as u32;
        let seconds = pf(p, "seconds").unwrap_or(1.0).clamp(0.0, 600.0);
        Ok(AudioJob { project: e.project.clone(), media: e.media.clone(), comp: cid, start: pf(p, "start").unwrap_or(0.0), frames: (seconds * rate as f64) as usize, rate })
    }

    pub fn run(&self) -> Vec<f32> {
        audio::mix_comp(&self.project, &self.media, self.comp, self.start, self.frames, self.rate)
    }
}

pub struct ThumbJob {
    project: Arc<Project>,
    media: Arc<Media>,
    item: u64,
    time: f64,
    max_side: u32,
}

impl ThumbJob {
    pub fn new(e: &Engine, item: u64, time: f64, max_side: u32) -> Option<ThumbJob> {
        let it = e.project.item(item)?;
        if matches!(it.kind, ItemKind::Folder) {
            return None;
        }
        if let ItemKind::Footage(f) = &it.kind
            && !f.has_video
        {
            return None;
        }
        Some(ThumbJob { project: e.project.clone(), media: e.media.clone(), item, time, max_side: max_side.max(8) })
    }

    pub fn run(&self) -> Option<(u32, u32, Vec<u8>)> {
        let it = self.project.item(self.item)?;
        let img = match &it.kind {
            ItemKind::Comp(c) => {
                let s = (self.max_side as f64 / c.width.max(c.height) as f64).min(1.0);
                let r = Renderer::new(&self.project, &self.media, RenderOpts { scale: s, guides: false, ..RenderOpts::default() });
                let img = r.comp_frame(self.item, self.time);
                return Some((img.w as u32, img.h as u32, img.to_rgba8_over(c.background)));
            }
            ItemKind::Footage(f) => self.media.frame(f, self.time)?,
            ItemKind::Solid(s) => {
                let c = [s.color[0] as f32, s.color[1] as f32, s.color[2] as f32, 1.0];
                let k = (self.max_side as f64 / s.width.max(s.height).max(1) as f64).min(1.0);
                let (w, h) = (((s.width as f64 * k) as usize).max(1), ((s.height as f64 * k) as usize).max(1));
                let img = crate::render::Img::filled(w, h, c);
                return Some((w as u32, h as u32, img.to_rgba8()));
            }
            ItemKind::Folder => return None,
        };
        let k = (self.max_side as f64 / img.w.max(img.h).max(1) as f64).min(1.0);
        let m = Mat3::scale(k, k);
        let (w, h) = (((img.w as f64 * k).round() as usize).max(1), ((img.h as f64 * k).round() as usize).max(1));
        let t = crate::render::image::warp(&img, &m, w, h, true)?;
        let mut full = crate::render::Img::new(w, h);
        for yy in 0..t.img.h {
            let y = t.y0 as usize + yy;
            if y >= h {
                continue;
            }
            let len = t.img.w.min(w - t.x0 as usize) * 4;
            let di = (y * w + t.x0 as usize) * 4;
            full.px[di..di + len].copy_from_slice(&t.img.px[yy * t.img.w * 4..yy * t.img.w * 4 + len]);
        }
        Some((w as u32, h as u32, full.to_rgba8()))
    }
}

pub const PEAK_BINS: u32 = 200;

pub struct PeaksJob {
    project: Arc<Project>,
    media: Arc<Media>,
    item: u64,
    pub key: String,
}

impl PeaksJob {
    pub fn new(e: &Engine, item: u64) -> Option<PeaksJob> {
        let it = e.project.item(item)?;
        let key = match &it.kind {
            ItemKind::Footage(f) if f.has_audio && !f.missing => format!("f{item}:{}", f.path),
            ItemKind::Comp(_) if audio::comp_has_audio(&e.project, item) => format!("c{item}:{}", e.revision),
            _ => return None,
        };
        Some(PeaksJob { project: e.project.clone(), media: e.media.clone(), item, key })
    }

    pub fn run(&self) -> Vec<[f32; 4]> {
        let Some(it) = self.project.item(self.item) else { return vec![] };
        let rate = 48000u32;
        let bin = (rate / PEAK_BINS) as usize;
        match &it.kind {
            ItemKind::Footage(f) => match self.media.audio(&f.path, rate) {
                Some(pcm) => audio::peak_summary(&pcm, bin),
                None => vec![],
            },
            ItemKind::Comp(c) => {
                let frames = (c.duration * rate as f64) as usize;
                let buf = audio::mix_comp(&self.project, &self.media, self.item, 0.0, frames, rate);
                audio::peak_summary(&buf, bin)
            }
            _ => vec![],
        }
    }
}
