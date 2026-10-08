# formvfx against After Effects

formvfx 0.4.0 measured line by line against Adobe After Effects 26.5 (September 2026), the way the form family measures itself: each capability is traced through formvfx's data model and engine command, the interface that exposes it, and the render or export path that applies it.

- **Full** · implemented and wired from the interface through to the render or export.
- **Partial** · present but narrower than After Effects, or reachable only as a command form where After Effects gives direct manipulation.
- **None** · nothing found.

Full counts 1, Partial one half, None 0. The platform rows at the end are shown but not scored. **Verified** says whether the capability was run end to end in this build (built through the MCP tools or the interface and checked on rendered frames or files); unverified rows are graded from the engine command and its interface path alone.

After Effects' side comes from Adobe's own documentation (What's new in After Effects, release notes through 26.5) and is Adobe's claim. formvfx's side was read from this tree at the version above; its engine is formvfx's own, written in Rust (`native/fvx-core`).

## Score

| | Rows | Full | Partial | None | Score |
| --- | ---: | ---: | ---: | ---: | ---: |
| formvfx 0.2.0 | 72 | 46 | 22 | 4 | 57 of 72 (79%) |
| formvfx 0.3.0 | 72 | 50 | 18 | 4 | 59 of 72 (82%) |
| formvfx 0.4.0 | 72 | 53 | 15 | 4 | 60.5 of 72 (84%) |

## Rows

### Project and footage

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 1 | Project panel: folders, labels, search, comments, usage | Full | `project.*`, Project panel (`frontend/src/panels/project.ts`) | yes |
| 2 | Import video, image sequences, stills, audio, PSD as composition, AI and PDF pages, SVG | Full | `file.import {importAs}`, `import_files`; Import dialog | mp4, png |
| 3 | Interpret Footage, proxies | Full | `file.interpretFootage`, `file.interpretProxy`, generated forms | no |
| 4 | Collect Files, Reduce Project, dependencies | Full | `file.collectFiles`, File > Dependencies menu | no |
| 5 | Dynamic Link with an editor | None | formcut handoff not built | |

### Compositions

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 6 | Composition settings, nesting, precompose | Full | `comp.new`, `comp.settings`, `layer.precompose` | yes |
| 7 | Work area, composition and layer markers, chapters and cue points | Full | `comp.workArea`, `markers.*`; Timeline work area bar | work area |
| 8 | Flowchart | Full | `comp.flowchart` opens the Flowchart panel (`ui.flowchart`): nested comps, footage and solids, optionally layers and effects, left to right or top to bottom; double-click opens a comp, Cmd or Ctrl-click reveals; agents read it with `get_comp {flowchart: true}` | yes |
| 9 | Essential Graphics templates | Partial | `essential.*`, `.ectemplate`; not Adobe's `.mogrt` | no |
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
| 22 | Wiggler, Smoother, Motion Sketch | Partial | `keys.wiggle`, `keys.smooth`, `motion.sketch {points}`; command panels, no live pointer capture | wiggle |
| 23 | Puppet pins (position, starch, bend, advanced, overlap), recording | Full | `puppet.*`; the Puppet tool places pins, the Selection and Puppet tools drag them in the viewer (`puppet.movePin`) | pin drag |
| 24 | Animation presets: save, apply, browse | Partial | `anim.savePreset`, `anim.applyPreset`; `.ecpreset`, no `.ffx` | no |

### Text

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 25 | Point and paragraph text, Character and Paragraph panels | Full | `layer.newText`, `layer.setText`; Character and Paragraph panels | yes |
| 26 | Text animators with range, wiggly and expression selectors | Full | `layer.addTextAnimator`, `layer.addTextSelector` | yes |
| 27 | Text on a path, per-character 3D | Full | `text/pathOptions`, `layer.enablePerChar3D` | path |
| 28 | OpenType features and variable font axes | Full | `layer.setText` OpenType keys and `variations`, `text.animatorFontAxes` | no |
| 29 | Text animation preset library | Partial | `layer.applyTextPreset` with 8 presets; After Effects ships hundreds | typewriter |

### Shapes

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 30 | Shape layers: groups, paths, fills, strokes, gradient fills | Full | `layer.newShape`, `shape.newPath`, `layer.addShapeItem` | yes |
| 31 | Shape operators: Trim Paths, Repeater, Offset, Pucker and Bloat, Twist, Zig Zag, Wiggle, Merge, Round Corners | Full | `layer.addShapeItem {kind}` | trim, repeater |
| 32 | Dashes, tapered and wavy strokes | Full | `shape.dashes.*`, `shape.stroke.taper`, `shape.stroke.wave` | no |
| 33 | Shapes and masks from text and from vector layers; Illustrator and SVG paste as shapes | Partial | `layer.create {op}`; no clipboard paste of vector content | no |

### Masks, rotoscoping and paint

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 34 | Masks: modes, feather, expansion, variable feather, animated paths, interpolation | Full | `layer.addMask`, `mask.*`, `layer.mask.*`; shape and pen tools draw masks | yes |
| 35 | Mask tracking | Full | `track.mask {method}` | no |
| 36 | Roto Brush and Refine Edge | Partial | `roto.*` with classical and MobileSAM models; not Roto Brush 3's model | no |
| 37 | Object Matte (AI subject selection and tracking) | Partial | `roto.stroke` and `roto.propagate` approximate it; no one-click subject pick | no |
| 38 | Paint, Clone Stamp and Eraser with write-on | Full | `paint.*`; Brush, Clone and Eraser tools | write-on |
| 39 | Content-Aware Fill | Partial | `layer.newContentAwareFill`, `contentFill.*`; methods present, quality untested | no |

### Effects

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 40 | Effect catalogue breadth (306 effects in 21 categories, including the CC set) | Full | `effects.md`; Effects and Presets panel, Effect menu | 8 effects |
| 41 | Keying: Keylight, Key Cleaner, Advanced Spill Suppressor | Full | `ec.keying.*`, `ec.key.*` | no |
| 42 | Colour: Lumetri Color, Curves, Levels, Hue/Saturation, OCIO transforms | Full | `ec.color.*` | no |
| 43 | Distortion: Warp Stabilizer, Liquify, Mesh Warp, Turbulent Displace | Full | `ec.distort.*`, `track.warpStabilizer`, `liquify.*` | no |
| 44 | Simulation: particles, Shatter, Card Dance, CC Particle World | Full | `ec.sim.*` | no |
| 45 | Expression controls, including dropdown, layer and 3D point | Full | `ec.control.*` | slider |
| 46 | Third-party effect plug-ins | Partial | WebAssembly plug-ins (`effect.plugins.load`); no After Effects SDK plug-ins | no |
| 47 | Curl Noise (26.3) | None | not in the catalogue | |

### 3D

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 48 | Classic 3D and Advanced 3D renderers, cameras, lights, shadows | Full | `comp.renderer`, `layer.newCamera`, `layer.newLight`, `materialOptions` | yes |
| 49 | 3D models (glTF, GLB, OBJ), primitives, materials | Full | `layer.newModel`, `layer.new3dPrimitive`, `material.set` | no |
| 50 | Substance 3D materials (.sbsar) | None | not supported | |
| 51 | Depth of field | Full | `cameraOptions/dof`, `focusDistance`, `aperture`, `camera.linkFocusToLayer` | no |
| 52 | Environment lights and image-based lighting | Full | Environment light, `light.environmentBackground`, `layer.environment` | no |
| 53 | Cinema 4D renderer | None | not supported | |
| 54 | 3D views and view gizmos | Partial | `view.set3DView`, `camera.orbit`; one viewer, no transform gizmo | no |

### Tracking

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 55 | Point, stabilize, affine and perspective trackers | Full | `track.*`; track points draw on footage and their feature, search and attach regions drag (`track.setPoint`) | point track, drag |
| 56 | Warp Stabilizer and 3D Camera Tracker | Full | `track.warpStabilizer`, `track.camera`, `camera.createFromSolve` | no |
| 57 | Face tracking | Full | `track.mask {method: faceOutline|faceDetailed}`, `track.extractFaceMeasurements` | no |
| 58 | Planar tracking (Mocha AE) | Partial | `perspective` tracker corner-pins; no planar solver | no |

### Expressions and scripting

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 59 | JavaScript expressions on the After Effects object model, pick whip, expression errors | Full | `prop.setExpression`, `prop.pickWhip`, `expr.errors`; expression rows in the Timeline | yes |
| 60 | Scripting (`app.project`, layers, properties) | Full | `script.run`, `run_script` | yes |
| 61 | ScriptUI panels and extensions (CEP, UXP) | Partial | `scriptui.*` drives ScriptUI dialogs; no CEP or UXP | no |

### Preview and performance

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 62 | RAM preview with audio, preview resolution, loop | Full | `/api/viewer` range streaming, Preview panel, `/api/audio` | yes |
| 63 | GPU acceleration | Partial | GPU paths behind the `gpu` build feature (`./formvfx install-native --gpu`); the default build renders on the CPU | no |
| 64 | Multi-frame rendering, disk cache | Partial | renders run off the edit thread on a pool; `cache.diskStats`; no multi-frame render setting | no |

### Render and export

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 65 | Render Queue: render settings, output modules, templates, several outputs | Full | `renderQueue.*`, `render.addOutputModule`; Render Queue panel | yes |
| 66 | H.264, HEVC, AV1, ProRes, WebM, GIF, PNG, TIFF, EXR and audio output | Full | `renderQueue.formats` | H.264, ProRes, GIF, PNG |
| 67 | Lottie export (a plug-in in After Effects) | Full | `file.exportLottie` | yes |
| 68 | Save frame as PSD or layered EXR | Full | `comp.saveFrameAsPsd`, `comp.saveFrameAsExr` | PNG frame |

### Colour, audio and interface

| # | Capability | formvfx | Trace | Verified |
| ---: | --- | --- | --- | --- |
| 69 | 8, 16 and 32-bit projects, colour management, OCIO and ACES, HDR output | Full | `file.projectSettings {bitDepth, colorEngine, workingSpace, outputSpace}` | no |
| 70 | Audio: layers, levels, effects, waveforms in the Timeline | Full | audio layers and the 13 audio effects; Audio > Waveform rows in the Timeline (LL, or twirl open Audio) for footage and nested comps, from peak summaries (`/api/peaks`) | waveforms |
| 71 | Workspaces, docking, customisable shortcuts | Full | 7 workspaces, docking, maximise and floating panels (Undock Panel, saved with the workspace); Edit > Keyboard Shortcuts: a keyboard view by modifier, search, record, presets, import and export, conflicts, over 1,144 bindable commands | workspaces, float, shortcut record |
| 72 | Viewer: guides, grids, rulers, safe zones, channels, region of interest, snapshots, multiple viewers | Partial | `view.*` commands; one Composition viewer with guides, grid, channels and safe zones | channels |

### Platform (not scored)

| Capability | After Effects 26.5 | formvfx 0.4.0 |
| --- | --- | --- |
| Desktop operating systems | macOS, Windows (x64, ARM) | macOS, Windows, Linux (Tauri) |
| In a browser | no | yes (`./formvfx serve`) |
| Agents drive every tool (MCP, command line, embedded terminal) | no | yes |
| Licence | proprietary subscription | Apache-2.0 |

## Where formvfx trails, in order

1. GPU rendering on by default (63) and multi-frame rendering (64).
2. AI and model-backed tools: Roto Brush 3 quality, Object Matte (36, 37).
3. Several Composition viewers at once (72).
4. Ecosystem: `.mogrt`, `.ffx`, After Effects SDK plug-ins, Cinema 4D, Substance, Curl Noise (9, 24, 46, 47, 50, 53).

## Where formvfx leads

Lottie export without a plug-in, Linux and the browser, an Apache-2.0 licence, and an agent surface: every one of its 655 commands is reachable from Claude Code, Codex or Cursor through MCP or the command line, with a skill that teaches the workflow.

Sources: Adobe, [What's new in After Effects](https://helpx.adobe.com/after-effects/desktop/what-s-new/whats-new.html), read October 7, 2026.
