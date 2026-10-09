# formvfx against After Effects

formvfx 0.23.0 measured line by line against Adobe After Effects 26.5 (September 2026), the way the form family measures itself: each capability is traced through formvfx's data model and engine command, the interface that exposes it, and the render or export path that applies it.

- **Full** · implemented and wired from the interface through to the render or export.
- **Partial** · present but narrower than After Effects, or reachable only as a command form where After Effects gives direct manipulation.
- **None** · nothing found.

Full counts 1, Partial one half, None 0. The platform rows at the end are shown but not scored. **Verified** says whether the capability was run end to end in this build (built through the MCP tools or the interface and checked on rendered frames or files); unverified rows are graded from the engine command and its interface path alone.

After Effects' side comes from Adobe's own documentation (What's new in After Effects, release notes through 26.5) and is Adobe's claim. formvfx's side was read from this tree at the version above; its engine is formvfx's own, written in Rust (`native/fvx-core`).

## Score

| | Rows | Full | Partial | None | Score |
| --- | ---: | ---: | ---: | ---: | ---: |
| formvfx 0.23.0 | 72 | 64 | 6 | 2 | 67 of 72 (93%) |

## Rows

### Project and footage

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 1 | Project panel: folders, labels, search, comments, usage | Full | `project.*`, Project panel (`frontend/src/panels/project.ts`) | yes |
| 2 | Import video, image sequences, stills, audio, PSD as composition, AI and PDF pages, SVG | Full | `file.import {importAs}`, `import_files`; Import dialog | mp4, png |
| 3 | Interpret Footage, proxies | Full | `file.interpretFootage`, `file.interpretProxy`, generated forms | no |
| 4 | Collect Files, Reduce Project, dependencies | Full | `file.collectFiles`, File > Dependencies menu | no |
| 5 | Dynamic Link with an editor | Partial | formvfx's side: a composition is served as a live clip, no render, from the open project (unsaved edits included) or from any saved project through a side engine that reopens the file when it changes (`GET /api/link/frame.png`, `/api/link/audio`, `/api/link/info`, `backend/app/engine/link.py`); Export Dynamic Link Clip in the Project panel, `link.export` and the `dynamic_link` MCP tool write the `.fvxlink` descriptor. The editor side (placing a `.fvxlink` on a formcut timeline) is not built yet | frames and audio, live and side engine, reload after save |

### Compositions

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 6 | Composition settings, nesting, precompose | Full | `comp.new`, `comp.settings`, `layer.precompose` | yes |
| 7 | Work area, composition and layer markers, chapters and cue points | Full | `comp.workArea`, `markers.*`; Timeline work area bar | work area |
| 8 | Flowchart | Full | `comp.flowchart` opens the Flowchart panel (`ui.flowchart`): nested comps, footage and solids, optionally layers and effects, left to right or top to bottom; double-click opens a comp, Cmd or Ctrl-click reveals; agents read it with `get_comp {flowchart: true}` | yes |
| 9 | Essential Graphics templates | Partial | `essential.*` and the Essential Graphics panel (Window menu): properties, comments and groups as controllers of a primary composition, renamed and reordered; each precomp layer of it gets Essential Properties that override per instance and follow the master until changed (Revert to Master); templates exported and imported as formvfx's own `.fvxtemplate` (the composition, what it uses, its media and a poster frame) and browsed in the panel; the scripting calls (`addToMotionGraphicsTemplate`, `motionGraphicsTemplateName`, `exportAsMotionGraphicsTemplate`, `essentialProperty`); not Adobe's `.mogrt` or Premiere Pro's side | a lower third with text, colour and opacity controllers on two instances: one overridden blue, the other following red then green, Revert to Master, rename, reorder, remove and undo reaching instances; exported with its image, imported into a fresh project with the image moved, rendered; the panel used under throttling |
| 10 | Responsive Design: Time | Full | `comp.responsiveTime` | no |

### Layers

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 11 | Solid, text, shape, null, adjustment, camera, light and 3D model layers | Full | `layer.new*`, Layer > New menu, toolbar tools | yes |
| 12 | Parenting and track mattes (any layer as the matte) | Full | `layer.setParent`, `layer.setTrackMatte`; Timeline Parent and Track Matte columns | yes |
| 13 | Blend modes | Full | `layer.setBlendMode`; Timeline Mode column | no |
| 14 | Layer switches (shy, solo, lock, collapse, quality, effects, frame blending, motion blur, adjustment, 3D) | Full | `layer.setSwitch`; Timeline switch columns | yes |
| 15 | Time stretch, time remapping, frame blending, freeze frame | Full | `layer.timeStretch`, `layer.enableTimeRemap`, `layer.frameBlending`, `layer.freezeFrame` | remap |
| 16 | Layer styles | Full | `layer.style.*`; Layer > Layer Styles | yes |
| 17 | Sequence layers, align and distribute | Full | `layer.sequence`, `layer.align`, `layer.distribute`; Align panel | yes |

### Animation

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 18 | Keyframes with linear, Bezier, continuous, auto Bezier, hold and roving interpolation | Full | `prop.addKey`, `keys.*`, `set_keyframes`; Timeline keyframes and context menus | yes |
| 19 | Graph Editor: value and speed graphs, influence handles | Full | `ui.graph` samples, `keys.transform`, `keys.setEase`; keys and their influence and speed handles drag in both graphs | handles |
| 20 | Motion paths in the viewer, auto-orient | Full | motion paths drawn from `overlays`; tangent handles drag through `keys.setSpatialTangents` (Alt breaks them); `layer.autoOrient` | tangents |
| 21 | Keyframe Assistant: easy ease, exponential scale, sequence, time-reverse, convert audio to keyframes | Full | `keys.easyEase*`, `keys.exponentialScale`, `keys.timeReverse`, `keys.audioToKeyframes` | easy ease |
| 22 | Wiggler, Smoother, Motion Sketch | Full | Wiggler, Smoother and Motion Sketch panels over `keys.wiggle`, `keys.smooth`, `motion.sketch`; Motion Sketch records the pointer in the Composition viewer with capture speed and smoothing | yes |
| 23 | Puppet pins (position, starch, bend, advanced, overlap), recording | Full | `puppet.*`; the Puppet tool places pins, the Selection and Puppet tools drag them in the viewer (`puppet.movePin`) | pin drag |
| 24 | Animation presets: save, apply, browse | Partial | `anim.savePreset`, `anim.applyPreset`; formvfx's own `.fvxpreset` document, no `.ffx` | no |

### Text

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 25 | Point and paragraph text, Character and Paragraph panels | Full | `layer.newText`, `layer.setText`; Character and Paragraph panels | yes |
| 26 | Text animators with range, wiggly and expression selectors | Full | `layer.addTextAnimator`, `layer.addTextSelector` | yes |
| 27 | Text on a path, per-character 3D | Full | `text/pathOptions`, `layer.enablePerChar3D` | path |
| 28 | OpenType features and variable font axes | Full | `layer.setText` OpenType keys and `variations`, `text.animatorFontAxes` | no |
| 29 | Text animation preset library | Full | 169 presets in 13 categories (Animate In and Out, Blurs, Fill and Stroke, Lights and Optical, Mechanical, Multi-Line, Organic, Rotation, Scale, Tracking, 3D Text, Miscellaneous) in `presets/text-animators.json`, applied as live text animators (`text.applyLibraryPreset`, `text_preset`), plus the engine's 8 (`layer.applyTextPreset`); Effects & Presets > Animation Presets, double-click or drag onto a text layer | all 169 swept on rendered frames, double-click, drag |

### Shapes

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 30 | Shape layers: groups, paths, fills, strokes, gradient fills | Full | `layer.newShape`, `shape.newPath`, `layer.addShapeItem` | yes |
| 31 | Shape operators: Trim Paths, Repeater, Offset, Pucker and Bloat, Twist, Zig Zag, Wiggle, Merge, Round Corners | Full | `layer.addShapeItem {kind}` | trim, repeater |
| 32 | Dashes, tapered and wavy strokes | Full | `shape.dashes.*`, `shape.stroke.taper`, `shape.stroke.wave` | no |
| 33 | Shapes and masks from text and from vector layers; Illustrator and SVG paste as shapes | Full | `layer.create {op}`; SVG markup on the clipboard pastes as one editable shape layer (Cmd+V, `clipboard.pasteSvg`, `shapes_from_svg`) from any app that copies SVG as text: Illustrator with Include SVG Code, Figma's Copy as SVG, a code editor | paste, MCP |

### Masks, rotoscoping and paint

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 34 | Masks: modes, feather, expansion, variable feather, animated paths, interpolation | Full | `layer.addMask`, `mask.*`, `layer.mask.*`; shape and pen tools draw masks | yes |
| 35 | Mask tracking | Full | `track.mask {method}` | no |
| 36 | Roto Brush and Refine Edge | Partial | `roto.*` with the Classic model or MobileSAM, propagated by optical flow; not Roto Brush 3's model | no |
| 37 | Object Matte (AI subject selection and tracking) | Full | the Object Matte tool (Alt+W): hovering a footage layer runs MobileSAM at the pointer and tints the object it would select, with an outline (`roto.objectPreview`, on formvfx's own MobileSAM encoder and decoder in `native/fvx-core`, which reproduces the reference model's embeddings to within 3e-6; the frame's embedding is kept, so moving the pointer costs only the decoder); a click seeds a Roto Brush there, spans the whole layer and propagates both ways as one "Object Matte" step (`roto.objectMatte`, MCP `object_matte`). The model is a 40 MB download (Settings > Roto Brush) | preview IoU 0.99 on a moving subject; hover and a click propagating 24 frames in the interface |
| 38 | Paint, Clone Stamp and Eraser with write-on | Full | `paint.*`; Brush, Clone and Eraser tools | brush, clone, the three erase modes, write-on |
| 39 | Content-Aware Fill | Full | `layer.newContentAwareFill`, `contentFill.*`: Object, Surface and Edge Blend fills, work area or whole layer, alpha expansion, lighting correction, reference frames, a Fill layer above the source | masked moving object: Object and Surface about 1.5 of 255 mean error in the hole against the clean plate; Edge Blend within 15 |

### Effects

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 40 | Effect catalogue breadth (307 effects in 21 categories, including the CC set) | Full | 304 effects: 300 in the Effect menu and Effects and Presets across the 20 categories (`effects.md`), with Face Track Points, Face Measurements, Paint and Roto Brush & Refine Edge added by their tools; the 3D Channel set (3D Channel Extract, Depth Matte, Depth of Field, EXtractoR, Fog 3D, ID Matte, IDentifier, Cryptomatte) reads depth, ids, normals, UVs, coverage and Cryptomatte layers from OpenEXR footage; Camera-Shake Deblur, Rolling Shutter Repair and Pixel Motion Blur on optical flow | all 300 listed applied and rendered, none failed; the 3D Channel set on an EXR with depth, ids, normals, coverage and a Cryptomatte layer (hashing checked against mmh3), the three flow effects on moving footage |
| 41 | Keying: Keylight, Key Cleaner, Advanced Spill Suppressor | Full | `fx.keying.*`, `fx.matte.*` | rendered |
| 42 | Colour: Lumetri Color, Curves, Levels, Hue/Saturation, OCIO transforms | Full | `fx.color.*`, the OCIO transforms | OCIO within 0.2% of OpenColorIO 2.6; Lumetri Look and Apply Color LUT |
| 43 | Distortion: Warp Stabilizer, Liquify, Mesh Warp, Turbulent Displace | Full | `fx.distort.*`, `track.warpStabilizer`, `liquify.*` | Warp Stabilizer 0.00 px against 42.5 px raw; Liquify stroke and clear |
| 44 | Simulation: particles, Shatter, Card Dance, CC Particle World | Full | `fx.sim.*` | contact sheet |
| 45 | Expression controls, including dropdown, layer and 3D point | Full | `fx.control.*` (slider, point, 3D point, angle, checkbox, colour, layer, dropdown) | slider |
| 46 | Third-party effect plug-ins | Partial | WebAssembly effect plug-ins (`effect.plugins.load`, `effect.plugins.list`, the Plug-ins folder, Effect > Load Plug-in...) with their own parameters in the Effect menu and Effects and Presets, sandboxed (memory limit, fuel limit, no file or network access); the ABI in `docs/PLUGINS.md` and an example in `native/plugins/radial-tint`; no After Effects SDK plug-ins | a Rust plug-in and a hand-written `.wat` plug-in loaded, applied, saved, reopened and rendered; an endless plug-in stopped and reported |
| 47 | Curl Noise (26.3) | Full | Curl Noise Generator (`fx.noise.curlNoise`): Input Noise Internal (Speed, Direction) or This Layer (Mirror at Edge), Transform (Scale, Uniform Scaling, Scale Width and Height, Offset, Rotation), Curl Generation (Evolution, Turbulence Speed, Noise Speed, Swirl, Density, Smoothness, Vertical Bias), Flow Options (Sample Count, Sample Radius, Flow Softness, Edge Definition, Flow Falloff), View, Contrast, Brightness, Clip HDR Results, Channel. Other Layer input is This Layer on an adjustment layer above the source | rendered |

### 3D

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 48 | Classic 3D and Advanced 3D renderers, cameras, lights, shadows | Full | `comp.renderer`, `layer.newCamera`, `layer.newLight`, `materialOptions` | yes |
| 49 | 3D models (glTF, GLB, OBJ), primitives, materials | Full | `layer.newModel`, `layer.new3dPrimitive`, `material.set` | no |
| 50 | Substance 3D materials (.sbsar) | None | not supported | |
| 51 | Depth of field | Full | `cameraOptions/dof`, `focusDistance`, `aperture`, `camera.linkFocusToLayer` | no |
| 52 | Environment lights and image-based lighting | Full | Environment light, `light.environmentBackground`, `layer.environment` | no |
| 53 | Cinema 4D renderer | None | not supported | |
| 54 | 3D views and view gizmos | Full | `view.set3DView`, `camera.orbit`; 1, 2 and 4-view layouts (`view.layout`, each extra view's 3D view chosen in place); the 3D axis gizmo drags a layer along its own X, Y or Z | 4 views, axis drag |

### Tracking

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 55 | Point, stabilize, affine and perspective trackers | Full | `track.*`; track points draw on footage and their feature, search and attach regions drag (`track.setPoint`); the Tracker panel lays the controls out as After Effects does (Track Motion, Stabilize Motion, Motion Source, Current Track, Track Type, Position / Rotation / Scale, Motion Target, Analyze both ways by frame or whole, Options, Reset, Apply, and Track Mask when a mask is selected, `frontend/src/panels/tracker-controls.ts`) | point track, drag, Tracker panel |
| 56 | Warp Stabilizer and 3D Camera Tracker | Full | `track.warpStabilizer`, `track.camera`, `camera.createFromSolve` | rendered scene: moving camera solved at 0.28 px mean error, path within 0.6% of the truth; tripod pan detected |
| 57 | Face tracking | Full | `track.mask {method: faceOutline|faceDetailed}`, `track.extractFaceMeasurements`, on the learned MediaPipe Face Landmarker (face detector and 478-point mesh, downloaded once and verified, `face.model.*`, Settings > Face Tracking) run by formvfx's own TensorFlow Lite interpreter; the classical detector without it | the interpreter matches LiteRT to 2e-4 on both networks; on MediaPipe's test portrait the 23 Face Track Points sit 1.8 px (2% of the eye distance) from MediaPipe's own expected landmarks; a face moving 126 px and turning 12 degrees is followed to within 2.2 px and 0.7 degrees |
| 58 | Planar tracking (Mocha AE) | Full | a mask around the surface is tracked with a homography from the features inside it (`track.mask {method: perspective}`), then every frame is re-aligned to the reference frame so error does not accumulate (`native/fvx-core/src/engine/cmd/track.rs`); the surface corners become a Perspective tracker and a Corner Pin (with Grow Bounds) on the insert layer, one undo step (`track.planar`, `planar_track`, Tracker panel > Planar Track) | a solid pinned on a synthetic clip, Corner Pin keyed on all 30 frames |

### Expressions and scripting

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 59 | JavaScript expressions on the After Effects object model, pick whip, expression errors | Full | `prop.setExpression`, `prop.pickWhip`, `expr.errors`; expression rows in the Timeline; formvfx's own interpreter with regular expressions (literals, `RegExp`, `match`, `replace`, `split`, `search`), `sampleImage` (before or after effects) and the text style API (`text.sourceText.style`, `getStyleAt`, `setFontSize` and the other setters, by character range too) | regular expressions, `sampleImage` on a solid before and after a Fill, styles by range |
| 60 | Scripting (`app.project`, layers, properties) | Full | `script.run`, `file.runScript`, File > Scripts, the `run_script` MCP tool: formvfx's own interpreter with the After Effects scripting object model (`app.project` items and folders, `CompItem`, `LayerCollection.add*`, layer attributes and methods, properties with keys, eases and expressions, effects and shape contents by match name, `TextDocument`, the render queue, `File`, `Folder`, `app.settings`), one undo step per script | 23 checks: comps, layers, keys, eases, text documents, effects, shapes, folders, render queue, files, settings, errors with line numbers |
| 61 | ScriptUI panels and extensions (CEP, UXP) | Partial | `new Window(...)` with buttons, text, edit fields, checkboxes, radio buttons, sliders, progress bars, lists, groups and panels, drawn as floating windows in the interface whose handlers run as undo steps (`scriptui.event`); installed ScriptUI panels in the Window menu (`scriptui.openPanel`); dialogs are not modal; no CEP or UXP | a palette made by a script, edited and clicked in the throttled interface, its handler adding a layer |

### Preview and performance

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 62 | RAM preview with audio, preview resolution, loop | Full | `/api/viewer` range streaming, Preview panel, `/api/audio` | yes |
| 63 | GPU acceleration | Full | formvfx's wgpu compositor (Metal, Vulkan, DirectX 12, OpenGL) is in the default build: layer warps, 36 blend modes and the GPU effects (blurs, exposure, brightness and contrast, tint, invert, OCIO transforms); the engine probes the adapter once in a separate process and attaches it, each comp then renders on the GPU or the CPU, whichever measures faster, and a project set to Mercury Software Only stays on the CPU. Settings > Memory & CPU shows the adapter and turns it off; `FORMVFX_GPU=0` too; `render_frame {backend}` forces either | Vulkan on Mesa's software rasteriser: GPU frames match the CPU within 1 of 255 on the demo project, 36 blend modes and eight GPU effects at 8, 16 and 32 bpc; no hardware GPU here |
| 64 | Multi-frame rendering, disk cache | Full | the Render Queue renders frames in parallel batches; previews render several frames at once; Settings > Memory & CPU: Enable Multi-Frame Rendering and CPU cores reserved (`engine.performance`); a disk cache that survives restarts (Settings > Disk & Cache, `cache.diskStats`, `cache.cleanDisk`) | the same 24 frames in order; a cached frame read back identical after a restart, missed after an edit |

### Render and export

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 65 | Render Queue: render settings, output modules, templates, several outputs | Full | `renderQueue.*`, `render.addOutputModule`; Render Queue panel | two output modules on one item, templates, 3:2 pulldown, Pre-render |
| 66 | H.264, HEVC, AV1, ProRes, WebM, GIF, PNG, TIFF, EXR and audio output | Full | `renderQueue.formats` | AV1, ProRes 4444 with alpha, HEVC Main 10, interlaced H.264, WebM, EXR and PNG sequences, AIFF, WAV |
| 67 | Lottie export (a plug-in in After Effects) | Full | `file.exportLottie`, `file.importLottie` | drawn by rlottie within a pixel of formvfx's own frames |
| 68 | Save frame as PSD or layered EXR | Full | `comp.saveFrameAsPsd`, `comp.saveFrameAsExr` | read back by psd-tools and OpenEXR |

### Colour, audio and interface

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 69 | 8, 16 and 32-bit projects, colour management, OCIO and ACES, HDR output | Full | `file.projectSettings {bitDepth, colorEngine, workingSpace, outputSpace}`, `renderQueue.setColor` | 41 colour checks; PQ code values in a 16-bit PNG |
| 70 | Audio: layers, levels, effects, waveforms in the Timeline | Full | audio layers and the 13 audio effects; Audio > Waveform rows in the Timeline (LL, or twirl open Audio) for footage and nested comps, from peak summaries (`/api/peaks`) | waveforms |
| 71 | Workspaces, docking, customisable shortcuts | Full | 7 workspaces, docking, maximise and floating panels (Undock Panel, saved with the workspace); Edit > Keyboard Shortcuts: a keyboard view by modifier, search, record, presets, import and export, conflicts | workspaces, float, shortcut record |
| 72 | Viewer: guides, grids, rulers, safe zones, channels, region of interest, snapshots, multiple viewers | Full | `view.*` commands; guides (also dragged from the rulers), grid, rulers, safe zones, channels, region of interest, Take and Show Snapshot; extra views and a locked viewer (`view.splitLockedViewer`, View > New Viewer) | rulers, guides, snapshot, locked viewer |

### Platform (not scored)

| Capability | After Effects 26.5 | formvfx 0.23.0 |
| --- | --- | --- |
| Desktop operating systems | macOS, Windows (x64, ARM) | macOS, Windows, Linux (Tauri) |
| In a browser | no | yes (`./formvfx serve`) |
| Agents drive every tool (MCP, command line, embedded terminal) | no | yes |
| Licence | proprietary subscription | Apache-2.0 |

## In brief

`native/fvx-core` is formvfx's compositing engine, written in Rust for formvfx. It saves `.fvxproj` and opens `.ecproj` and `.ecprojx` projects from earlier formvfx releases as imports. What the rows above add up to:

- **Full** · 1, 2, 3, 4, 6, 7, 8, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 37, 38, 39, 40, 41, 42, 43, 44, 45, 47, 48, 49, 51, 52, 54, 55, 56, 57, 58, 59, 60, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72: import of video, audio, stills, numbered image sequences (with missing-frame reports), OpenEXR (any layer), Photoshop documents as merged footage, one layer, or a composition with groups as precomps and blend modes, opacity and visibility kept (`file.import {importAs: composition|compositionLayerSizes, layer}`), PDF and Illustrator pages (`page`), SVG and Lottie; proxies for footage and compositions with Set Proxy, Use Proxy, Interpret Proxy and Create Proxy (still or movie), honoured by Render Settings Proxy Use (`file.setProxy`, `file.useProxy`, `file.interpretProxy`, `file.createProxy`); the Render Queue with Render Settings (quality, resolution, proxies, effects, solo, guides, colour depth, frame blending, field rendering and 3:2 pulldown, motion blur, time span, frame rate, skip existing files), several output modules per item (format, channels, straight or premultiplied colour, quality or bitrate, keyframe interval, ProRes profile, Main 10, crop, resize, audio rate, channels and bit depth, post-render actions: import, import and replace usage, set proxy), built-in and saved templates with defaults per kind, logs, notify, overflow folders, Pre-render and Save Current Preview (`renderQueue.*`, `render.*`); output to H.264, HEVC, AV1, ProRes 422 to 4444 XQ with alpha, WebM (VP9 or AV1), GIF, PNG, JPEG, TIFF and OpenEXR sequences, WAV and AIFF; Lottie export to `.json` and `.lottie` with eased keyframes, baked or kept expressions, shapes, solids, images, text or text outlines, precomps, parenting, masks and track mattes, checked against rlottie, and Lottie import (`file.exportLottie`, `file.importLottie`); Save Frame As layered Photoshop and layered EXR (`comp.saveFrameAsPsd`, `comp.saveFrameAsExr`); 8, 16 and 32-bit projects (8 and 16 bpc quantise after every effect and composite, 32 bpc keeps over-range values) with colour management: working spaces from sRGB and Rec. 2020 to ACEScg, Linearize Working Space and Blend Colors Using 1.0 Gamma, footage profiles (Rec. 2100 PQ, HLG and Rec. 2020 detected, Interpret Footage with Preserve RGB), display colour management with a display profile and Simulate Output, HDR Clip or Tone Map, and per-item output colour spaces with 16-bit PNG, TIFF and ProRes and 10-bit HEVC tagged for PQ and HLG (`file.projectSettings`, `renderQueue.setColor`, View menu); the OCIO colour engine on formvfx's own config reader and transforms with the ACES 1.x output transforms, a built-in ACES 1.3 config or any `.ocio` file, its forward transforms within 0.2% of OpenColorIO 2.6 on the ACES 1.3 CG config (`color.spaces`, `color.convert`), the OCIO Color Space, Display, Look, File and CDL Transforms, Lumetri Color's Input LUT and Creative Look and LUT files in cube, 3dl, spi1d, spi3d, CLF, CTF and CDL formats; GPU acceleration on wgpu (Metal, Vulkan, DirectX 12, OpenGL): layer warps, 36 blend modes and GPU effects (blurs, exposure, brightness and contrast, tint, invert, OCIO transforms), each composition on the GPU or the CPU, whichever measures faster, matching the CPU to 1 of 255 (Settings > Memory & CPU, Project Settings `renderer`); multi-frame rendering of Render Queue batches and a disk cache that survives restarts (Settings > Disk & Cache, `cache.*`); Object Matte from one point (hover previews the object, a click selects it and follows it through the layer; `roto.objectPreview`, `roto.objectMatte`) on formvfx's own implementation of the MobileSAM encoder and decoder, which reproduces the reference model's embeddings to within 3e-6; Paint with Brush, Clone Stamp and Eraser strokes kept in a Paint effect, write-on and the other durations, channels, blend modes and the three erase modes (`paint.*`); puppet pins of all five kinds on an as-rigid-as-possible mesh, with recording and follow-through (`puppet.*`); Content-Aware Fill with Object, Surface and Edge Blend fills, alpha expansion, lighting correction and a reference frame (`layer.newContentAwareFill`, `contentFill.*`); tracking as background analysis jobs that land as one undo step: point, stabilize, affine and perspective trackers with the Tracker panel and Apply to a target (`track.*`), mask tracking from position to perspective (`track.mask`), planar tracking with a Corner Pin on the insert layer (`track.planar`), the Warp Stabilizer with its four methods, smoothness, framing with crop, auto-scale and synthesized edges (`track.warpStabilizer`, `warp.*`), and the 3D Camera Tracker solving typical, mostly flat and tripod shots with a fixed, variable or specified angle of view, then creating the camera, text, solids, nulls and a shadow catcher on chosen points and setting the ground plane (`track.camera`, `camera.*`); variable mask feather with feather points, smooth or linear falloff, mask motion blur and Smart Mask Interpolation (`mask.featherPoint.*`, `layer.mask.featherFalloff`, `layer.mask.motionBlur`, `mask.interpolate`); Pixel Motion frame blending from dense optical flow (`layer.frameBlending`); 3D with Classic and Advanced renderers (`comp.renderer`; Advanced intersects layers and models per pixel), parallel, spot, point, ambient and environment lights with falloff, cones and soft shadows that take the caster's colour through Light Transmission, the Material Options (`material.set`), glTF, GLB and OBJ models and six 3D primitives with base colour, metallic, roughness and emissive (`layer.newModel`, `layer.new3dPrimitive`), cameras and lights from a model, depth of field with focus linked to the point of interest or a layer, image-based environment lights with a background layer (`light.environmentBackground`, `layer.environment`), and the eleven 3D views with orbit, pan, dolly, Look at Selected and the 1, 2 and 4-view layouts (`view.3d.*`, `camera.orbit`); keying with Keylight, Key Cleaner, Advanced Spill Suppressor and the Matte chokers; the Simulation category (CC Particle Systems II and Particle World, Particle Playground, Shatter, Card Dance, Caustics, Foam, Wave World and the CC simulations); the 13 audio effects applied in the mixdown that preview and export share, Tone making any layer audible, beside levels and waveform rows; the nine layer styles with Blending Options and Global Light (`layer.style.*`, Layer > Layer Styles); Collect Files, Consolidate, Remove Unused, Reduce Project and Find Missing (File > Dependencies); markers with chapters, URLs, frame targets, cue points and protected regions (`markers.*`) and Responsive Design: Time (`comp.responsiveTime`, protected regions keep their speed in a stretched precomp); every layer switch including Draft and Wireframe quality and frame blending; Sequence Layers with dissolves, align and distribute; roving keys; motion paths with auto-orient along the path; the Keyframe Assistant (Exponential Scale, Time-Reverse Keyframes, Convert Audio to Keyframes, Easy Ease, Sequence Layers); the Wiggler, Smoother and Motion Sketch; snapshots and a locked viewer beside guides, grids, rulers, channels and the region of interest; every shape operator (Merge Paths, Offset Paths, Pucker & Bloat, Repeater, Round Corners, Trim Paths, Twist, Wiggle Paths, Wiggle Transform, Zig Zag; the Timeline's Add menu, `shape.add`), dashes, tapered and wavy strokes (`shape.dashes.add`, `shape.stroke.taper`, `shape.stroke.wave`), SVG paste and import as shapes, Layer > Create shapes and masks from text and shapes from a vector layer (`clipboard.pasteSvg`, `layer.create`); the project panel, compositions and precomps, the Flowchart, parenting and track mattes, all 39 blend modes, the Graph Editor, point and paragraph text with character styles by range, text animators with range, wiggly and expression selectors (`layer.addTextAnimator`, `layer.addTextSelector`, the Timeline's Add Property and Add Selector), text on a mask path and per-character 3D (`text/pathOptions`, `layer.enablePerChar3D`), OpenType features and variable font axes (`layer.setText` keys and `variations`, `text.fontFeatures`, `text.animatorFontAxes`), the 169-preset text library (`text.applyLibraryPreset`, Effects & Presets > Animation Presets), shape layers, the eight Expression Controls (dropdown items editable), Curl Noise, previews and workspaces; expressions on formvfx's own interpreter with the After Effects object model, pick whip and expression errors, regular expressions, `sampleImage` and the text style API; scripting through `app.project` with items, layers, property groups, keyframes, the Render Queue, `File` and `Folder`, run from File > Scripts or `script.run`, each run one undo step; 304 effects across all 20 categories, the 3D Channel set reading depth, ids, normals and Cryptomatte from OpenEXR passes, Camera-Shake Deblur, Rolling Shutter Repair and Pixel Motion Blur; face tracking (outline, detailed features and Face Measurements) on the learned MediaPipe Face Landmarker run by formvfx's own TensorFlow Lite interpreter.
- **Partial** · 5, 9, 24, 36, 46, 61: Dynamic Link's serving side without an editor that places the clips; Essential Graphics templates in formvfx's own `.fvxtemplate` rather than `.mogrt`; animation presets in formvfx's own `.fvxpreset` document without `.ffx`; Roto Brush and Refine Edge with the Classic model or MobileSAM, propagated by optical flow (`roto.*`), not Roto Brush 3's own model; WebAssembly effect plug-ins rather than After Effects SDK plug-ins; ScriptUI windows, dialogs and dockable panels from Scripts/ScriptUI Panels (Window menu) with buttons, text, checkboxes, sliders, lists and groups, but no CEP or UXP panels and dialogs that do not block the script.
- **None** · 50, 53: Substance materials, the Cinema 4D renderer.

## Where formvfx trails, in order

1. Dynamic Link's editor side (5): formvfx serves linked comps; formcut does not place them yet.
2. Roto Brush 3's own model (36); formvfx runs MobileSAM.
3. Ecosystem: `.mogrt`, `.ffx`, After Effects SDK plug-ins, CEP and UXP panels, Cinema 4D, Substance (9, 24, 46, 50, 53, 61).

## Where formvfx leads

Lottie export without a plug-in, Linux and the browser, an Apache-2.0 licence, and an agent surface: every one of its 508 engine commands is reachable from Claude Code, Codex or Cursor through MCP or the command line, with a skill that teaches the workflow.

Sources: Adobe, [What's new in After Effects](https://helpx.adobe.com/after-effects/desktop/what-s-new/whats-new.html), read October 7, 2026.
