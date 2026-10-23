use serde_json::{Value as Json, json};

use super::{Engine, ps, registry};
use crate::effects;
use crate::model::LABELS;

pub const TITLES: [&str; 9] = ["File", "Edit", "Composition", "Layer", "Effect", "Animation", "View", "Window", "Help"];

enum M {
    I(&'static str),
    P(&'static str, &'static str, Json),
    Owned(&'static str, String, Json),
    S,
    Sub(&'static str, Vec<M>),
}

const PANELS: [(&str, &str); 31] = [
    ("project", "Project"),
    ("footage", "Footage"),
    ("mediaBrowser", "Media Browser"),
    ("composition", "Composition"),
    ("timeline", "Timeline"),
    ("effectControls", "Effect Controls"),
    ("effectsPresets", "Effects & Presets"),
    ("essential", "Template Controls"),
    ("info", "Info"),
    ("preview", "Preview"),
    ("align", "Align"),
    ("character", "Character"),
    ("paragraph", "Paragraph"),
    ("tracker", "Tracker"),
    ("renderQueue", "Render Queue"),
    ("flowchart", "Flowchart"),
    ("history", "History"),
    ("audio", "Audio"),
    ("brushes", "Brushes"),
    ("paint", "Paint"),
    ("contentAwareFill", "Selection-Aware-Fill"),
    ("colorScopes", "Color Scopes"),
    ("markers", "Markers"),
    ("metadata", "Metadata"),
    ("motionSketch", "Motion Sketch"),
    ("smoother", "Smoother"),
    ("wiggler", "Wiggler"),
    ("progress", "Progress"),
    ("learn", "Learn"),
    ("chat", "Chat"),
    ("terminal", "Terminal"),
];

const WORKSPACES: [&str; 8] = ["Essentials", "Default","Animation", "Effects", "Motion Tracking", "Text", "Agent", "Minimal"];

fn recent_list(paths: &[String], run: &'static str, clear: &'static str) -> Vec<M> {
    let mut out: Vec<M> = paths.iter().enumerate().map(|(i, path)| M::Owned(run, std::path::Path::new(path).file_name().and_then(|x| x.to_str()).unwrap_or(path).to_string(), json!({"index": i}))).collect();
    if !out.is_empty() {
        out.push(M::S);
    }
    out.push(M::I(clear));
    out
}

fn tree(recent_paths: &[String], projects: &[String], footage: &[String]) -> Vec<M> {
    use M::*;
    let open_recent = recent_list(projects, "file.openRecent", "file.clearRecent");
    let import_recent = recent_list(footage, "file.importRecent", "file.clearRecentFootage");
    let mut recent: Vec<M> = recent_paths
        .iter()
        .enumerate()
        .map(|(i, path)| Owned("anim.applyRecentPreset", std::path::Path::new(path).file_stem().and_then(|x| x.to_str()).unwrap_or("Preset").to_string(), json!({"index": i})))
        .collect();
    if !recent.is_empty() {
        recent.push(S);
    }
    recent.push(I("anim.clearRecentPresets"));
    let mut scripts: Vec<M> = super::cmd::script::script_menu_items().into_iter().map(|(n, p)| Owned("file.runScript", n, p)).collect();
    if !scripts.is_empty() {
        scripts.push(S);
    }
    scripts.extend([I("file.runScript"), S, I("file.installScript"), I("file.installScriptUIPanel")]);
    let mut panels: Vec<M> = super::cmd::script::panel_menu_items().into_iter().map(|(n, p)| Owned("scriptui.openPanel", n, p)).collect();
    if !panels.is_empty() {
        panels.insert(0, S);
    }
    let mut extensions: Vec<M> = super::cmd::extensions::menu_items().into_iter().map(|(n, p)| Owned("extensions.open", n, p)).collect();
    if !extensions.is_empty() {
        extensions.push(S);
    }
    extensions.push(I("extensions.install"));
    let mut effect_menu = vec![I("effect.manage"), I("effect.applyLast"), I("effect.removeAll"), I("effect.plugins.load"), S];
    let mut cats: Vec<&str> = effects::every().into_iter().filter(|e| effects::listed(e)).map(|e| e.category).collect();
    cats.sort();
    cats.dedup();
    for cat in cats {
        let mut items: Vec<&effects::EffectSpec> = effects::every().into_iter().filter(|e| e.category == cat && effects::listed(e)).collect();
        items.sort_by_key(|e| e.name);
        effect_menu.push(Sub(cat, items.into_iter().map(|e| P("effect.apply", e.name, json!({"effect": e.id}))).collect()));
    }
    vec![
        Sub(
            "File",
            vec![
                Sub("New", vec![I("file.newProject"), I("project.newFolder"), I("file.newFromTemplate")]),
                I("file.open"),
                Sub("Open Recent", open_recent),
                S,
                I("file.close"),
                I("file.closeProject"),
                I("file.save"),
                Sub("Save As", vec![I("file.saveAs"), I("file.saveCopy"), I("file.saveCopyAsXml")]),
                I("file.incrementAndSave"),
                I("file.revert"),
                I("templates.saveAs"),
                S,
                Sub("Import", vec![I("file.import"), I("file.importMultiple"), I("file.importPlaceholder"), I("file.importSolid"), I("file.importLottie"), I("file.importTimeline"), I("file.importVanishingPoint"), I("essential.importTemplate"), I("data.import")]),
                Sub("Import Recent Footage", import_recent),
                Sub("Export", vec![I("file.exportLottie"), I("file.exportTimeline"), I("export.slicesFromGuides")]),
                S,
                I("layer.addItem"),
                I("file.newCompFromSelection"),
                S,
                Sub("Interpret Footage", vec![I("file.interpretFootage"), I("file.interpretProxy"), S, I("file.rememberInterpretation"), I("file.applyInterpretation")]),
                Sub("Replace Footage", vec![P("file.replaceFootage", "File...", Json::Null), I("file.replaceWithLayeredComp"), I("file.replaceWithPlaceholder"), I("file.replaceWithSolid")]),
                I("file.reloadFootage"),
                I("file.revealInFinder"),
                S,
                Sub("Set Proxy", vec![I("file.setProxy"), I("file.setProxyNone")]),
                Sub("Create Proxy", vec![P("file.createProxy", "Still...", json!({"kind": "still"})), P("file.createProxy", "Movie...", json!({"kind": "movie"}))]),
                I("file.useProxy"),
                S,
                Sub("Dependencies", vec![I("file.collectFiles"), I("file.consolidateFootage"), I("file.removeUnusedFootage"), I("file.reduceProject")]),
                I("file.watchFolder"),
                Sub("Scripts", scripts),
                I("app.find"),
                S,
                I("file.projectSettings"),
            ],
        ),
        Sub(
            "Edit",
            vec![
                I("edit.undo"),
                I("edit.redo"),
                I("edit.history"),
                S,
                I("edit.cut"),
                I("edit.copy"),
                I("edit.copyWithPropertyLinks"),
                I("edit.copyWithRelativePropertyLinks"),
                I("edit.paste"),
                Sub("Paste Text", vec![I("edit.pasteTextFormattingOnly"), I("edit.pasteTextMatchFormatting")]),
                I("edit.clear"),
                S,
                I("edit.duplicate"),
                I("layer.split"),
                I("edit.liftWorkArea"),
                I("edit.extractWorkArea"),
                S,
                I("edit.selectAll"),
                I("edit.deselectAll"),
                S,
                Sub("Label", LABELS.iter().enumerate().map(|(i, l)| P("edit.label", l.0, json!({"label": i}))).chain([S, I("edit.selectLabelGroup")]).collect()),
                Sub(
                    "Purge",
                    vec![
                        P("edit.purge", "All Memory", json!({"what": "memory"})),
                        I("edit.purgeUndo"),
                        P("edit.purge", "Image Cache Memory", json!({"what": "image"})),
                        P("edit.purge", "Snapshot", json!({"what": "snapshot"})),
                        P("edit.purge", "All Memory and Disk Cache", json!({"what": "memoryAndDisk"})),
                        I("cache.cleanDisk"),
                    ],
                ),
                S,
                I("edit.editOriginal"),
                S,
                I("app.keyboardShortcuts"),
                I("app.settings"),
            ],
        ),
        Sub(
            "Composition",
            vec![
                I("comp.new"),
                I("comp.settings"),
                I("comp.setPosterTime"),
                S,
                I("comp.trimToWorkArea"),
                S,
                I("renderQueue.add"),
                I("render.addOutputModule"),
                S,
                Sub("Save Frame As", vec![I("comp.saveFrameAs"), I("comp.saveFrameAsPsd"), I("comp.saveFrameAsExr")]),
                I("render.preRender"),
                I("render.saveCurrentPreview"),
                Sub("Preview", vec![I("playback.audio"), I("playback.scrubAudio"), I("playback.cacheWhenIdle")]),
                S,
                I("comp.openInEssentialGraphics"),
                I("essential.exportTemplate"),
                S,
                I("comp.cropToRegionOfInterest"),
                I("comp.cropToLayerBounds"),
                S,
                Sub("VR", vec![I("comp.vr.createEnvironment"), I("comp.vr.extractCubemap")]),
                S,
                I("comp.duplicate"),
                Sub(
                    "Responsive Design - Time",
                    vec![
                        P("comp.responsiveTime", "Create Intro", json!({"op": "intro"})),
                        P("comp.responsiveTime", "Create Outro", json!({"op": "outro"})),
                        P("comp.responsiveTime", "Create Protected Region from Work Area", json!({"op": "workArea"})),
                    ],
                ),
                S,
                I("comp.flowchart"),
                I("comp.miniFlowchart"),
            ],
        ),
        Sub(
            "Layer",
            vec![
                Sub(
                    "New",
                    vec![
                        I("layer.newText"),
                        I("layer.newSolid"),
                        I("layer.newLight"),
                        I("layer.newCamera"),
                        I("layer.newNull"),
                        I("layer.newShape"),
                        I("layer.newAdjustment"),
                        I("layer.newContentAwareFill"),
                        Sub("3D Primitive", crate::model::prop::PRIMITIVES.iter().map(|k| P("layer.new3dPrimitive", k, json!({"kind": k.to_ascii_lowercase()}))).collect()),
                    ],
                ),
                I("layer.settings"),
                I("layer.openLayer"),
                S,
                Sub(
                    "Mask",
                    vec![
                        I("layer.addMask"),
                        I("layer.mask.shape"),
                        I("layer.mask.reset"),
                        I("layer.mask.remove"),
                        I("layer.mask.removeAll"),
                        S,
                        Sub(
                            "Mode",
                            ["None", "Add", "Subtract", "Intersect", "Lighten", "Darken", "Difference"].iter().map(|m| P("layer.mask.mode", m, json!({"mode": m}))).collect(),
                        ),
                        I("layer.mask.invert"),
                        S,
                        I("layer.mask.lock"),
                        I("layer.mask.unlockAll"),
                        I("layer.mask.lockOthers"),
                        I("layer.mask.hideLocked"),
                    ],
                ),
                Sub("Mask and Shape Path", vec![I("path.rotoBezier"), I("path.setFirstVertex"), I("path.convertToBezier"), S, I("path.freeTransform")]),
                Sub("Track Matte", ["None", "Alpha", "Alpha Inverted", "Luma", "Luma Inverted", "Layer Above", "Layer Below"].iter().zip(["none", "alpha", "alphaInverted", "luma", "lumaInverted", "above", "below"]).map(|(l, op)| P("layer.trackMatte", l, json!({"op": op}))).collect()),
                Sub("Switches", vec![I("layer.hideOtherVideo"), I("layer.showAllVideo"), I("layer.unlockAll")]),
                Sub("Markers", vec![I("layer.markersLock"), I("layer.deleteAllMarkers"), I("layer.updateMarkersFromSource")]),
                Sub("Cloner", vec![I("cloner.create"), I("cloner.remove")]),
                I("layer.geometryOptions"),
                S,
                Sub(
                    "Layer Styles",
                    vec![
                        I("layer.style.convertToEditable"),
                        I("layer.style.showAll"),
                        I("layer.style.removeAll"),
                        S,
                        I("layer.style.dropShadow"),
                        I("layer.style.innerShadow"),
                        I("layer.style.outerGlow"),
                        I("layer.style.innerGlow"),
                        I("layer.style.bevelEmboss"),
                        I("layer.style.satin"),
                        I("layer.style.colorOverlay"),
                        I("layer.style.gradientOverlay"),
                        I("layer.style.stroke"),
                    ],
                ),
                Sub(
                    "Time",
                    vec![I("layer.enableTimeRemap"), I("layer.timeReverse"), I("layer.timeStretch"), I("layer.freezeFrame"), I("layer.freezeOnLastFrame"), S, I("layer.alignVideoToData")],
                ),
                Sub(
                    "Frame Blending",
                    vec![P("layer.frameBlending", "Off", json!({"mode": "off"})), P("layer.frameBlending", "Frame Mix", json!({"mode": "frameMix"})), P("layer.frameBlending", "Pixel Motion", json!({"mode": "pixelMotion"}))],
                ),
                Sub(
                    "Quality",
                    vec![
                        P("layer.quality", "Best", json!({"quality": "best"})),
                        P("layer.quality", "Draft", json!({"quality": "draft"})),
                        P("layer.quality", "Wireframe", json!({"quality": "wireframe"})),
                        S,
                        P("layer.sampling", "Bilinear", json!({"sampling": "bilinear"})),
                        P("layer.sampling", "Bicubic", json!({"sampling": "bicubic"})),
                    ],
                ),
                Sub(
                    "Create",
                    vec![
                        P("layer.create", "Create Shapes from Text", json!({"op": "shapesFromText"})),
                        P("layer.create", "Create Masks from Text", json!({"op": "masksFromText"})),
                        P("layer.create", "Create Shapes from Vector Layer", json!({"op": "shapesFromVector"})),
                    ],
                ),
                Sub(
                    "Camera",
                    vec![
                        I("layer.cameraSettings"),
                        I("camera.fromView"),
                        I("camera.orbitNull"),
                        I("camera.fromModel"),
                        S,
                        I("camera.linkFocusToPoi"),
                        I("camera.linkFocusToLayer"),
                        I("camera.setFocusToLayer"),
                        S,
                        I("camera.stereoRig"),
                    ],
                ),
                Sub("Light", vec![I("layer.lightSettings"), I("light.controlWithCamera"), I("light.fromModel"), I("light.environmentBackground")]),
                Sub("Material", vec![I("material.revealSource"), I("material.importMaterialX"), I("material.reset"), I("material.duplicateAssign")]),
                I("layer.environment"),
                S,
                Sub(
                    "Transform",
                    vec![
                        P("layer.transform", "Reset", json!({"op": "reset"})),
                        I("layer.centerAnchor"),
                        S,
                        P("layer.transform", "Flip Horizontal", json!({"op": "flipH"})),
                        P("layer.transform", "Flip Vertical", json!({"op": "flipV"})),
                        P("layer.transform", "Center in View", json!({"op": "center"})),
                        P("layer.transform", "Fit to Comp", json!({"op": "fit"})),
                        P("layer.transform", "Fit to Comp Width", json!({"op": "fitWidth"})),
                        P("layer.transform", "Fit to Comp Height", json!({"op": "fitHeight"})),
                        S,
                        I("layer.autoOrient"),
                    ],
                ),
                Sub("Arrange", vec![I("layer.bringToFront"), I("layer.bringForward"), I("layer.sendBackward"), I("layer.sendToBack")]),
                S,
                I("layer.openSource"),
                Sub("Reveal", vec![I("layer.revealSource"), I("layer.revealInFinder"), I("comp.revealInProject")]),
                S,
                I("path.groupShapes"),
                I("path.ungroupShapes"),
                S,
                I("layer.autoTrace"),
                I("layer.sceneEditDetection"),
                I("layer.precompose"),
            ],
        ),
        Sub("Effect", effect_menu),
        Sub(
            "Animation",
            vec![
                I("anim.savePreset"),
                I("anim.applyPreset"),
                Sub("Recent Animation Presets", recent),
                I("anim.browsePresets"),
                S,
                I("anim.addKeyframe"),
                I("keys.toggleHold"),
                I("keys.interpolation"),
                I("keys.velocity"),
                Sub(
                    "Keyframe Assistant",
                    vec![
                        I("keys.audioToKeyframes"),
                        I("keys.easyEase"),
                        I("keys.easyEaseIn"),
                        I("keys.easyEaseOut"),
                        I("keys.exponentialScale"),
                        I("layer.sequence"),
                        I("mask.interpolate"),
                        I("keys.rpfCameraImport"),
                        I("keys.timeReverse"),
                    ],
                ),
                Sub(
                    "Behaviors",
                    vec![
                        P("behavior.add", "Wiggle", json!({"kind": "wiggle"})),
                        P("behavior.add", "Oscillate", json!({"kind": "oscillate"})),
                        P("behavior.add", "Spring", json!({"kind": "spring"})),
                        P("behavior.add", "Bounce", json!({"kind": "bounce"})),
                        P("behavior.add", "Loop Cycle", json!({"kind": "loopCycle"})),
                        P("behavior.add", "Loop Ping Pong", json!({"kind": "loopPingPong"})),
                        P("behavior.add", "Loop Continue", json!({"kind": "loopContinue"})),
                        S,
                        I("behavior.remove"),
                        I("behavior.clear"),
                        S,
                        I("data.link"),
                    ],
                ),
                Sub("Rigging", vec![I("rig.ikCreate"), I("rig.ikRemove")]),
                Sub("Physics", vec![I("physics.bake")]),
                S,
                I("prop.separateDimensions"),
                I("essential.addProperty"),
                S,
                I("track.camera"),
                I("track.warpStabilizer"),
                I("track.motion"),
                I("track.stabilize"),
                I("track.mask"),
                I("track.property"),
                S,
                Sub(
                    "Animate Text",
                    crate::model::prop::TEXT_ANIMATOR_KINDS
                        .iter()
                        .filter(|k| k.0 != "fontAxis")
                        .map(|(k, l)| P("layer.addTextAnimator", l, json!({"properties": [k]})))
                        .chain([S, I("text.animatorFontAxes"), I("layer.enablePerChar3D")])
                        .collect(),
                ),
                Sub("Add Text Selector", vec![P("layer.addTextSelector", "Range", json!({"kind": "range"})), P("layer.addTextSelector", "Wiggly", json!({"kind": "wiggly"})), P("layer.addTextSelector", "Expression", json!({"kind": "expression"}))]),
                I("text.removeAllAnimators"),
            ],
        ),
        Sub(
            "View",
            vec![
                I("view.newViewer"),
                I("view.splitLockedViewer"),
                S,
                I("view.zoomIn"),
                I("view.zoomOut"),
                I("view.fit"),
                Sub("Resolution", vec![I("view.res.full"), I("view.res.half"), I("view.res.third"), I("view.res.quarter"), I("view.res.custom")]),
                S,
                I("view.rulers"),
                I("view.guides"),
                I("view.snapToGuides"),
                I("view.lockGuides"),
                I("view.clearGuides"),
                I("view.importGuides"),
                I("view.exportGuides"),
                S,
                I("view.grid"),
                I("view.snapToGrid"),
                S,
                I("view.options"),
                I("view.layerControls"),
                I("view.snapping"),
                S,
                I("view.takeSnapshot"),
                I("view.showSnapshot"),
                S,
                Sub(
                    "Fast Previews",
                    vec![
                        P("view.fastPreviewMode", "Off (Final Quality)", json!({"mode": "off"})),
                        P("view.fastPreviewMode", "Adaptive Resolution", json!({"mode": "adaptive"})),
                        P("view.fastPreviewMode", "Draft", json!({"mode": "draft"})),
                        P("view.fastPreviewMode", "Fast Draft", json!({"mode": "fastDraft"})),
                        P("view.fastPreviewMode", "Wireframe", json!({"mode": "wireframe"})),
                    ],
                ),
                I("view.extendedViewer"),
                I("view.shareViewOptions"),
                S,
                I("view.displayColorManagement"),
                Sub(
                    "Simulate Output",
                    vec![
                        P("view.simulateOutput", "No Output Simulation", json!({"profile": "none"})),
                        S,
                        P("view.simulateOutput", "sRGB IEC61966-2.1", json!({"profile": "srgb"})),
                        P("view.simulateOutput", "HDTV (Rec. 709)", json!({"profile": "rec709"})),
                        P("view.simulateOutput", "UHDTV (Rec. 2020)", json!({"profile": "rec2020"})),
                        P("view.simulateOutput", "Display P3", json!({"profile": "p3"})),
                        P("view.simulateOutput", "DCI-P3", json!({"profile": "dciP3"})),
                        P("view.simulateOutput", "Rec. 2100 PQ", json!({"profile": "rec2100pq"})),
                        P("view.simulateOutput", "Rec. 2100 HLG", json!({"profile": "rec2100hlg"})),
                        P("view.simulateOutput", "Legacy Macintosh RGB (Gamma 1.8)", json!({"profile": "mac18", "preserveRgb": true})),
                    ],
                ),
                S,
                Sub(
                    "Switch 3D View",
                    vec![
                        I("view.3d.activeCamera"),
                        I("view.3d.default"),
                        I("view.3d.front"),
                        I("view.3d.left"),
                        I("view.3d.top"),
                        I("view.3d.back"),
                        I("view.3d.right"),
                        I("view.3d.bottom"),
                        I("view.3d.custom1"),
                        I("view.3d.custom2"),
                        I("view.3d.custom3"),
                    ],
                ),
                I("view.3d.last"),
                I("view.reset3DView"),
                I("view.lookAtSelected"),
                I("view.lookAtAll"),
                S,
                I("time.set"),
                S,
                I("view.fullScreen"),
            ],
        ),
        Sub(
            "Window",
            vec![
                Sub(
                    "Workspace",
                    WORKSPACES
                        .iter()
                        .map(|w| P("window.workspace", w, json!({"name": w})))
                        .chain([S, I("window.resetWorkspace"), I("window.saveWorkspace"), I("window.saveWorkspaceAs"), I("window.editWorkspaces")])
                        .collect(),
                ),
                S,
                Sub("Panels", PANELS.iter().map(|(id, l)| P("window.panel", l, json!({"panel": id}))).collect()),
                Sub("Create Nulls from Paths", vec![I("paths.pointsFollowNulls"), I("paths.nullsFollowPoints"), I("paths.tracePath")]),
                Sub("Extensions", extensions),
                I("window.assignWorkspaceShortcut"),
            ]
            .into_iter()
            .chain(panels)
            .collect(),
        ),
        Sub("Help", vec![I("help.docs"), I("help.inAppTutorials"), I("file.openDemoProject"), S, I("help.systemReport"), I("help.enableLogging"), I("help.revealLogFile"), S, I("app.about")]),
    ]
}

fn footage_selected(e: &Engine, test: impl Fn(&crate::model::Footage) -> bool) -> bool {
    e.state.project_selection.iter().any(|i| matches!(e.project.item(*i).map(|it| &it.kind), Some(crate::model::ItemKind::Footage(f)) if test(f)))
}

fn item(e: &Engine, id: &str, label: Option<&str>, params: Json) -> Json {
    let c = registry::find(id);
    let lbl = label.map(str::to_string).or_else(|| c.map(|c| c.label.to_string())).unwrap_or_else(|| id.to_string());
    let shortcut = c.and_then(|c| e.shortcuts.get(c.id).and_then(|k| k.first().cloned()).or_else(|| (!c.key.is_empty() && params.is_null()).then(|| c.key.split(" or ").next().unwrap_or("").to_string())));
    let checked = match id {
        "view.snapping" => Json::Bool(e.state.snapping),
        "view.showSnapshot" => Json::Bool(e.state.show_snapshot),
        "view.displayColorManagement" => Json::Bool(e.state.viewer.display_cm),
        "view.simulateOutput" => Json::Bool(params.get("profile").and_then(Json::as_str) == Some(e.state.viewer.simulate.as_str())),
        "help.enableLogging" => Json::Bool(e.prefs.get("logging.enabled").and_then(Json::as_bool).unwrap_or(false)),
        "file.useProxy" => Json::Bool(e.state.project_selection.iter().any(|i| e.project.item(*i).and_then(|it| it.proxy.as_ref()).is_some_and(|p| p.enabled))),
        "layer.mask.hideLocked" => Json::Bool(e.state.hide_locked_masks),
        "view.extendedViewer" => Json::Bool(e.state.viewer.extended),
        "view.shareViewOptions" => Json::Bool(e.state.viewer.share_view_options),
        "playback.scrubAudio" => Json::Bool(e.state.viewer.scrub_audio),
        "view.fastPreviewMode" => Json::Bool(params.get("mode").and_then(Json::as_str) == Some(e.state.viewer.fast_previews.as_str())),
        "layer.markersLock" => Json::Bool(e.state.active_comp.and_then(|c| e.project.comp(c)).is_some_and(|c| !e.state.selected_layers.is_empty() && e.state.selected_layers.iter().all(|l| c.layer(*l).is_some_and(|l| l.switches.markers_locked)))),
        _ if id.starts_with("view.3d.") && id != "view.3d.last" => Json::Bool(e.state.active_comp.is_some_and(|c| super::cmd::view3d::current(e, c) == &id[8..])),
        _ => Json::Null,
    };
    let has_keys = !e.state.selected_keys.is_empty();
    let has_layers = !e.state.selected_layers.is_empty();
    let has_comp = e.state.active_comp.is_some();
    let enabled = match id {
        "edit.undo" => !e.history.undo.is_empty(),
        "edit.redo" => !e.history.redo.is_empty(),
        "file.save" | "file.saveAs" | "file.closeProject" => true,
        "comp.settings" | "renderQueue.add" | "comp.saveFrameAs" | "comp.duplicate" | "layer.newText" | "layer.newSolid" | "layer.newLight" | "layer.newCamera" | "layer.newNull" | "layer.newShape" | "layer.newAdjustment" => e.state.active_comp.is_some(),
        "effect.apply" | "layer.precompose" | "layer.addMask" | "edit.duplicate" | "layer.split" | "effect.removeAll" => has_layers,
        "keys.toggleHold" | "keys.timeReverse" | "keys.exponentialScale" | "keys.interpolation" | "keys.velocity" | "keys.easyEase" | "keys.easyEaseIn" | "keys.easyEaseOut" => has_keys,
        "anim.addKeyframe" => !e.state.selected_props.is_empty(),
        "anim.savePreset" => !e.state.selected_props.is_empty() || has_layers,
        "view.showSnapshot" => e.state.snapshot.is_some(),
        "view.displayColorManagement" | "view.simulateOutput" => e.project.settings.color_engine == "ocio" || crate::render::color::find(&e.project.settings.working_space).is_some(),
        "anim.clearRecentPresets" => !e.recent_presets.is_empty(),
        "file.clearRecent" => !e.session.recent_projects.is_empty(),
        "file.clearRecentFootage" => !e.session.recent_footage.is_empty(),
        "file.importVanishingPoint" => false,
        "layer.openSource" | "layer.revealInFinder" => has_layers,
        "file.close" | "comp.setPosterTime" | "comp.revealInProject" | "file.exportTimeline" | "comp.openInEssentialGraphics" | "comp.vr.extractCubemap" => has_comp,
        "comp.vr.createEnvironment" | "camera.stereoRig" => e.state.active_comp.and_then(|c| e.project.comp(c)).is_some_and(|c| c.vr.is_none() && c.stereo.is_none()),
        "comp.cropToRegionOfInterest" => has_comp && e.state.roi.is_some(),
        "comp.cropToLayerBounds" => has_layers,
        "templates.saveAs" => !e.project.items.is_empty(),
        "file.rememberInterpretation" | "file.replaceWithPlaceholder" | "file.replaceWithSolid" => footage_selected(e, |_| true),
        "file.applyInterpretation" => e.session.interpretation.is_some() && footage_selected(e, |_| true),
        "file.replaceWithLayeredComp" => footage_selected(e, |f| ["psd", "psb", "pdf", "ai"].iter().any(|x| f.path.to_ascii_lowercase().ends_with(&format!(".{x}")))),
        "file.revealInFinder" => e.path.is_some() || footage_selected(e, |f| !f.path.is_empty()),
        _ if id.starts_with("layer.style.") || id.starts_with("layer.mask.") || matches!(id, "layer.timeStretch" | "layer.timeReverse" | "layer.enableTimeRemap" | "layer.freezeFrame" | "layer.freezeOnLastFrame" | "layer.sequence" | "layer.settings" | "layer.revealSource" | "layer.quality" | "path.freeTransform" | "prop.separateDimensions" | "anim.applyPreset" | "anim.applyRecentPreset") => has_layers,
        "comp.trimToWorkArea" | "comp.responsiveTime" | "keys.audioToKeyframes" | "view.takeSnapshot" | "view.newViewer" | "view.splitLockedViewer" | "view.importGuides" | "view.exportGuides" | "comp.saveFrameAsPsd" | "comp.saveFrameAsExr" | "render.preRender" | "render.saveCurrentPreview" | "file.exportLottie" => has_comp,
        "render.addOutputModule" => !e.project.render_queue.is_empty(),
        "edit.copyWithPropertyLinks" | "edit.copyWithRelativePropertyLinks" | "edit.liftWorkArea" | "edit.extractWorkArea" | "edit.selectLabelGroup" | "layer.hideOtherVideo" | "layer.markersLock" | "layer.deleteAllMarkers" | "layer.updateMarkersFromSource" | "layer.trackMatte" | "layer.transform" | "layer.centerAnchor" | "layer.sampling" | "layer.alignVideoToData" | "layer.autoTrace" | "layer.sceneEditDetection" | "layer.geometryOptions" | "path.rotoBezier" | "path.setFirstVertex" | "path.convertToBezier" | "path.groupShapes" | "path.ungroupShapes" | "paths.pointsFollowNulls" | "paths.nullsFollowPoints" | "paths.tracePath" => has_layers,
        "effect.applyLast" => has_layers && e.session.last_effect.is_some(),
        "edit.pasteTextFormattingOnly" | "edit.pasteTextMatchFormatting" => has_layers && e.clipboard.is_some(),
        "edit.purgeUndo" => !e.history.undo.is_empty() || !e.history.redo.is_empty(),
        "layer.showAllVideo" | "layer.unlockAll" | "keys.rpfCameraImport" => has_comp,
        "edit.editOriginal" => has_layers || footage_selected(e, |f| !f.path.is_empty()),
        "file.setProxy" | "file.createProxy" => e.state.project_selection.iter().any(|i| e.project.item(*i).is_some_and(|it| matches!(it.kind, crate::model::ItemKind::Footage(_) | crate::model::ItemKind::Comp(_)))),
        "file.setProxyNone" | "file.useProxy" | "file.interpretProxy" => e.state.project_selection.iter().any(|i| e.project.item(*i).is_some_and(|it| it.proxy.is_some())),
        _ => true,
    };
    json!({"type": "item", "label": lbl, "command": id, "params": params, "shortcut": shortcut, "enabled": enabled, "checked": checked})
}

fn render(e: &Engine, nodes: &[M]) -> Vec<Json> {
    nodes
        .iter()
        .map(|n| match n {
            M::I(id) => item(e, id, None, Json::Null),
            M::P(id, label, params) => item(e, id, Some(label), params.clone()),
            M::Owned(id, label, params) => item(e, id, Some(label), params.clone()),
            M::S => json!({"type": "separator"}),
            M::Sub(label, children) => json!({"type": "submenu", "label": label, "children": render(e, children)}),
        })
        .collect()
}

impl Engine {
    pub fn menu_bar(&self, p: &Json) -> Json {
        let only = ps(p, "menu");
        let t = tree(&self.recent_presets, &self.session.recent_projects, &self.session.recent_footage);
        let picked: Vec<M> = t.into_iter().filter(|n| match (only, n) {
            (Some(m), M::Sub(label, _)) => *label == m,
            (Some(_), _) => false,
            _ => true,
        }).collect();
        Json::Array(render(self, &picked))
    }
}
