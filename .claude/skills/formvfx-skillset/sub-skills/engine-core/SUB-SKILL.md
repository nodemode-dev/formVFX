---
name: engine-core
description: formvfx's compositing engine in native/fvx-core (the fvx_core Python extension). Use to add or change a command, a property, a layer kind, an effect, the renderer, footage and audio decoding, the Render Queue, the .fvxproj format or the .ecproj import, or to close a gap listed in docs/PARITY.md.
---

# engine-core

`native/fvx-core` is formvfx's compositing engine, written for formvfx in Rust. The engine service loads it as `fvx_core`; the interface and the agent tools reach it through the engine service's JSON-RPC.

## Files

| Path | Holds |
| --- | --- |
| `src/model/project.rs` | `Project`, items (folder, comp, footage, solid), `Comp`, `Layer` and its kinds (footage, solid, precomp, null, shape, text, camera, light), switches, blend modes, track mattes, the `formvfx.project/1` serde form |
| `src/model/prop.rs` | the property tree: `Prop` nodes with uids and match names, group kinds (transform, masks, effects, contents, text, audio), path lookup (`transform/position`, `effects/#1/hue`, `contents/Rectangle 1/fill/color`) |
| `src/model/value.rs`, `key.rs`, `path.rs` | values (number, vector, colour, path, text, bool, enum), keyframes in layer seconds with linear, Bezier and hold, speed and influence eases, spatial tangents and the arc-length table, roving times, Bezier paths |
| `src/eval.rs` | a property's value at a time, through `expr::evaluate` when the property has an enabled expression |
| `src/expr/` | the expression interpreter: `lex.rs`, `parse.rs` (AST in `ast.rs`, cached per text in `mod.rs`), `interp.rs` (values, scopes, operators with vector maths, step cap), `builtins.rs` (JavaScript built-ins and the After Effects helpers), `host.rs` (the object model: comp, layer, group, property, key, marker), `noise.rs`; `mod.rs` guards loops between expressions |
| `src/engine/cmd/expression.rs`, `src/effects/controls.rs` | `expr.errors`, `layer.expressions`, `prop.pickWhip`, `prop.convertExpressionToKeyframes`, `effect.editDropdown`; the eight Expression Controls |
| `src/geom.rs` | 2D and 3D matrices, the layer transform chain, camera projection |
| `src/engine/mod.rs` | `Engine`: undo snapshots (`Arc<Project>`, merge keys and `merge_hint`), gestures, events, `execute`, `batch` with `$N.key` references |
| `src/engine/registry.rs` | every command id with its label, menu, kind (`Undo`, `Plain`, `Query`, `Front` for interface-only commands, `Todo` for not yet) and parameter schema |
| `src/engine/cmd/*.rs` | command handlers by area: file, project, comp, layer, prop, keys, effect, mask, text, edit |
| `src/render/styles.rs`, `src/engine/cmd/styles.rs` | layer styles: the `layerStyles` group and its nine style groups, the pad they need and their render on the layer buffer after effects; the `layer.style.*` commands and Global Light |
| `src/engine/cmd/time.rs`, `keytools.rs`, `markers.rs` | time stretch, reverse, remapping, freeze frames, trim to work area, Sequence Layers, Responsive Design; the keyframe assistants (Wiggler, Smoother, Motion Sketch, Exponential Scale, audio to keys), key copy and paste, selection, labels, nudges, Separate Dimensions; marker settings and conversion |
| `src/engine/cmd/deps.rs`, `layermisc.rs`, `viewer.rs` | Collect Files, Consolidate, Remove Unused, Reduce Project, Find Missing, metadata and `.fvxpreset` animation presets; Layer Settings, Transform Value, Quality, the `layer.mask.*` family, Free Transform Points; snapshots, the locked viewer, guide files and shortcut presets |
| `src/engine/state.rs`, `menus.rs` | `ui_state` (the snapshot the interface store pulls, property trees on request), overlays, graph samples, the Flowchart, the menu bar |
| `src/engine/export.rs`, `rq.rs` | the Render Queue: items, the render thread, polling and post-render actions (`export.rs`); Render Settings and output module parameters, built-in and saved templates (`render-templates.json` in the data folder), the module writers (FFmpeg pipes for H.264, HEVC, AV1, ProRes, WebM and GIF; PNG, JPEG, TIFF and EXR sequences; WAV and AIFF), crop, resize, channels, field rendering and pulldown, logs (`rq.rs`) |
| `src/render/exr.rs`, `psd.rs`, `pdf.rs`, `src/engine/cmd/formats.rs` | OpenEXR through the `exr` crate; the Photoshop reader and layered writer; PDF pages through `hayro`; import probing (layers, pages, sequences), Photoshop and PDF as compositions, proxies (`file.setProxy`, `useProxy`, `interpretProxy`, `createProxy`) and Save Frame As Photoshop Layers and Layered EXR |
| `src/engine/lottie.rs` | Lottie export (keyframes with their eases, baked expressions, shapes, solids, images, text, precomps, parenting, masks and mattes; `ty` written first for strict players) and import, `.lottie` archives |
| `src/engine/ecproj.rs` | the `.ecproj` and `.ecprojx` import (read only): items, layers, properties, keys and the effects that map onto formvfx's |
| `src/engine/demo.rs` | the demo project, built through `batch` |
| `src/render/comp.rs` | the compositor: premultiplied float RGBA on the CPU, layer order, 3D depth sort, precomps, adjustment layers, track mattes, motion blur subframes; `render_comp` picks the GPU or CPU path per composition, `layer_parts` stops before the final warp so the GPU can take it |
| `src/render/gpu.rs`, `gpu.wgsl` | the wgpu device (probed and attached through `set_gpu`), storage-buffer images, the warp plan mirroring `image::warp`, the composite, box, half and pointwise kernels, GPU effect programs (`effect`, `CAPABLE`, `encode_proc`) and the per-composition timing that chooses GPU or CPU |
| `src/render/color.rs`, `ocio.rs`, `aces.rs`, `lut.rs`, `yaml.rs`, `src/engine/cmd/color.rs` | colour: profiles and the project pipeline (`Pipe`: quantisation, working space, authored colours, footage, display and output procs), the OCIO config reader, operators and builtins, the ACES output transforms, LUT readers; `view.displayColorManagement`, `view.simulateOutput`, `color.spaces`, `color.convert` |
| `src/render/diskcache.rs` | the disk cache (closure hash, PNG store, eviction) and the persisted engine preferences (`~/.formvfx/engine-prefs.json`, `FORMVFX_DATA`) |
| `src/render/image.rs`, `blend.rs` | the image type, warps with mipmaps, 39 blend modes |
| `src/render/vector.rs`, `shapes.rs`, `text.rs` | masks and shapes through tiny-skia (projected point by point when a matrix has perspective), shape operators, the font database and text fill and stroke ops |
| `src/render/shapeops.rs`, `src/render/svg.rs`, `src/engine/cmd/shapes.rs` | shape path operators (Merge and Offset through i_overlay, Pucker & Bloat, Twist, Zig Zag, Wiggle Paths and Transform), tapered and wavy stroke outlines; SVG reading (usvg) into vector footage and shape contents; paste as shapes, Layer > Create, the Pen tool's shape paths, stroke dashes, taper and wave commands |
| `src/render/layout.rs` | text layout: per-character styles from runs, OpenType features, variable axes, shaping (rustybuzz) by runs, paragraph wrapping and justification, the face and outline caches, `font_features`, `font_axes` |
| `src/render/textanim.rs`, `src/engine/cmd/textanim.rs` | text animators: range, wiggly and expression selectors, animator properties, anchor grouping, text on a mask path, per-character 3D; the animator, selector, per-character 3D, OpenType and preset commands, the compiled-in preset library |
| `src/render/media.rs`, `audio.rs` | footage: still images through `image` (and EXR, Photoshop and PDF through their modules, cached per layer and page), image sequences through the frame cache, video and audio through the FFmpeg program (probe, streaming decode, a frame cache) |
| `src/effects/*.rs` | the effect registry (`mod.rs`: id, name, category, parameters with nested groups, render function, the `FxHost` trait for other times, the original buffer, other layers, masks and audio) and 289 effects by category (`ocio.rs` holds the OCIO transforms and the shared LUT-file helper); `draw.rs` holds the shared path and segment rasterisers, `util.rs` the filters and meshes |
| `src/render/light.rs`, `mesh.rs`, `model3d.rs`, `three.rs` | 3D: lights, shading, shadows (layer casters ray-tested, model casters by shadow maps), environment maps, the triangle rasteriser, primitives, the glTF and OBJ readers (with a model cache in `Media`), depth of field and Advanced 3D depth compositing |
| `src/engine/cmd/view3d.rs`, `src/engine/cmd/three.rs` | the 3D views (kept per composition in `State.views3d`, orthographic for Front to Bottom), orbit, pan and dolly, Look at; the renderer switch, models, primitives, materials, camera and light settings and helpers |
| `src/render/vision.rs`, `analysis.rs`, `src/engine/jobs.rs` | image primitives for tracking (grey pyramids, NCC search, corners, pyramidal Lucas-Kanade, dense flow and Pixel Motion, model fits and RANSAC); `Source` renders a layer's frames before its effects; background analysis jobs (`jobs::run`, polled from `tick`, applied as one undo step through `Engine::apply_step`) |
| `src/engine/cmd/track.rs`, `src/effects/tracking.rs`, `src/render/face.rs` | trackers in the layer's `motionTrackers` group (`trackPoint1`, `trackPoint2` ...), analyze and apply, mask tracking from position to perspective, planar tracking with Corner Pin, face outline and detailed features with Face Track Points and Face Measurements (hidden effects, `effects::listed` keeps them out of menus) |
| `src/render/stabilize.rs`, `src/engine/cmd/warp.rs` | the Warp Stabilizer: per-frame motion stored in the effect's hidden `analysis`, the correction planned at render time by method, smoothness and framing; `track.warpStabilizer`, `warp.*` |
| `src/render/camsolve.rs`, `src/engine/cmd/camtrack.rs` | the 3D Camera Tracker: feature tracks, shot detection (tripod from rotation-consistent homographies, typical from the essential matrix, mostly flat from the homography decomposition), self-calibration, incremental resection and intersection with a final angle-of-view refinement; placing the solve in the composition, the ground plane and Create from Camera Solve; `track.camera`, `camera.*` |
| `src/render/maskfx.rs`, `src/engine/cmd/maskshape.rs` | mask feather points and falloff, mask motion blur samples, Smart Mask Interpolation (vertex matching and rigid or linear in-betweens); `mask.featherPoint.*`, `layer.mask.featherFalloff`, `layer.mask.motionBlur`, `mask.interpolate` |
| `src/render/roto.rs`, `sam.rs`, `ptload.rs`, `src/effects/roto.rs`, `src/engine/cmd/roto.rs` | Roto Brush (strokes, span and run-length mattes in the effect; Classic or MobileSAM segmentation; propagation by flow; guided-filter edges at render), the MobileSAM runtime and checkpoint reader, model install and download (checked against the official SHA-256, kept in `~/.formvfx/models` or `FORMVFX_MODELS`), Object Matte (`object_job` in `py.rs`) |
| `src/effects/paint.rs`, `src/engine/cmd/paint.rs` | the Paint effect: stroke groups `s0001` ... with Path, Stroke Options and Transform; dab rendering, clone sources, erase modes; paint options and brush presets in the engine prefs |
| `src/render/puppet.rs`, `src/effects/puppet.rs`, `src/engine/cmd/puppet.rs` | Puppet: meshes `m01` ... with Deform pins `p0001` ... (hidden rest positions), the grid mesh and the as-rigid-as-possible solve, overlap depth, recording and follow-through |
| `src/render/fill.rs`, `src/engine/cmd/fill.rs` | Content-Aware Fill: hole from the masked alpha, background alignment, temporal fill, seam blending, membrane fill; writes a PNG-coded movie and adds it as the fill layer |
| `src/effects/audiofx.rs`, `src/render/audio.rs` | the 13 audio effects: video passes through, the mixdown runs each layer's chain over a pre-rolled buffer so blocks join seamlessly; Backwards reverses the source, Tone makes any layer audible |
| `src/py.rs` | the `Engine` pyclass and `FrameJob`, `AudioJob`, `ThumbJob`, `PeaksJob`, `ObjectJob`; `encode_png`, `__version__` |

## Contracts

- The Python surface in `src/py.rs` is what `backend/app` calls; change both together. Something the engine cannot do yet raises a plain sentence ("... is not in formvfx yet"), never a panic.
- `capabilities()` reports `engine: "fvx"`, `projectExtension: "fvxproj"` and `importProjects`; the interface reads them through `engine.info` to pick the save extension.
- `file.save` and `file.saveAs` write `.fvxproj` only; a path ending `.ecproj` is refused with a sentence. `file.open` of an `.ecproj` imports it and leaves the project without a path.
- Command ids and parameter names are stable; renaming one means changing the interface, the MCP tools and the harness references in the same commit.

## Writing it

- formvfx's own code only. Standard algorithms (Gaussian kernels, Porter-Duff, Bezier evaluation, colour space formulas) are written from their definitions.
- A new dependency is vetted first (permissive or MPL unmodified) and gets a row in `NOTICE.md`.
- A new effect is a function plus one registry entry in `src/effects/mod.rs`; a new command is a handler plus one `registry.rs` row.
- Reasoning goes in `native/fvx-core/HASHOUT_LOG.md`, not in comments.

## Build and run

```bash
./formvfx install-native --dev             # the engine, dev profile
./formvfx                                  # the app
backend/venv/bin/python backend/formvfx_engine.py --selfcheck
```

After changing commands or effects, regrade the affected rows in `docs/PARITY.md` and run `./formvfx agents sync` so the harness references follow.
