# demo · HASHOUT_LOG

Living context for the browser demo. Code carries almost no comments; the reasoning lives here.

## Shape

- `index.html` is hand-written and committed, with the theme boot script copied from `frontend/index.html` so the persisted profile and accent are on the document before first paint. It loads one classic `assets/demo.js` and `assets/demo.css`.
- `src/` is built by the interface's Vite (`build.mjs` calls `frontend/node_modules/.bin/vite`; the config is a plain object so it needs no `vite` import from this folder). Library mode, IIFE, `assetsInlineLimit` high enough to inline the Inter and JetBrains Mono subsets and the logo. Module scripts and font files are blocked or flaky on `file://`, which is why nothing is emitted as a module or a separate font.
- Reused as is from `frontend/src`: `styles/*`, `ui/icons.ts`, `ui/icons-vfx.ts`, `ui/themes.ts`, `ui/dom.ts`, `ui/toast.ts`, `ui/popup.ts`, `ui/dialogs.ts`, `engine/time.ts`, `version.ts`. `demo/src/demo.css` only adds the few `dm-`, `vw-`, `tl-`, `rq-` rules the demo needs; every colour is a token and every z-index a `var(--z-*)`.

## Engine (src/engine)

- Real engine is Rust; the demo carries a TypeScript compositor for the operations it shows. Names and ids mirror the app (layers, effects, text animators, work area), nothing else is shared.
- Render order mirrors the stack: bottom to top, each layer drawn into a comp-sized buffer under its matrix (position, rotation, scale, anchor, parent chain), then masks, then effects, then blended with `globalCompositeOperation`. Effects run after the transform, in comp space, which differs from the app (layer space); it is invisible for the demoed effects except Gaussian Blur on a rotated layer.
- Track mattes read the layer directly above and hide it. An adjustment layer copies the comp so far, runs its effects, masks the result with its own region and composites it back.
- Effects work on premultiplied floats for blurs (three box passes per axis, the Kutskir edge-clamped form) and straight RGBA bytes elsewhere. Each layer's effects run only inside the layer's quad grown by a margin from its blur-like parameters (`effectRect`), which cut a Half-size frame from about 400 ms to about 250 ms in software headless Chrome. `Env.oy` keeps Wave Warp's phase stable when that rectangle moves. Vignette, Twirl and Gradient Ramp are relative to the rectangle, exact for full-frame layers and adjustment layers.
- Text animators are a stagger model (start, delay per character, duration, ease, deltas) plus a wave kind, not a range selector. The presets are this demo's own; no vendor preset is reproduced.
- Keyframes evaluate with the departing key's ease; Overshoot is an ease-out-back. Editing a value on an animated property writes a key at the playhead; the stopwatch toggles animation.
- History is a JSON snapshot of the compositions taken by `touch()` after each committed edit (pointer up, commit, menu action); drags mutate without touching so a drag is one undo step.
- Footage: images keep a data URL so a saved project reopens with them; video keeps only a blob URL, so a reopened project shows "Missing media" for video. Video frames come from a hidden `video` element seeked per render (`syncVideos` for the viewer, awaited `prepare` for export).

## Exporters (engine/export.ts)

- PNG and contact sheet: one canvas, `toBlob`. Sequence: PNG per frame stored in a store-method ZIP written here (CRC32 table, no dependency).
- GIF: a 256 colour palette from the 15-bit colour histogram of up to six sampled frames, nearest colour cached per 15-bit key, LZW with the 4096 code limit and clear codes, NETSCAPE loop. No dithering, so smooth gradients band.
- WebM: `canvas.captureStream(0)` and `requestFrame()` per rendered frame, paced to the frame rate with `setTimeout`, so it runs in real time. WebCodecs is not used.

## Interface

- Dock, tabs, viewer bar, timeline, Effect Controls and Project reuse the app's class names. Lists are reconciled with `patchList`; values are patched in place; the viewer frame, the overlay and both timeline canvases are sized before draw and drawn in the same task as any resize, so nothing blanks.
- The viewer overlay handles selection (corner scale, drag move, anchor, mask outlines). Hit testing uses layer quads, not pixels.
- Menus use the app's `PopupMenu`; dialogs use `Dialog`. The Wiggler seeds its generator from the key count, so a repeat gives the same curve.

## Real media (0.53.1)

- The opening project is Coastline (`src/engine/coastline.ts`), built from five Pexels files in `assets/media/*.js` (sources in `CREDITS.md`). The generated Launch Week and Intro compositions stay as the offline fallback and under File > Open Generated Sample.
- Packs, not files: a `file://` page cannot fetch local files, and a video or image served from a file URL taints the canvas so the effects' `getImageData` fails. Each pack is `window.FVX_MEDIA[name] = "data:...;base64,..."`, loaded by injecting a script tag (`src/media.ts`), decoded with `atob` into a Blob for video (blob URL) or used as the image source directly. Video is VP9 WebM with no sound (the compositor has no audio), GOP 12 so the per-frame seeks stay cheap.
- Zero flash: the page shows a static boot line from `index.html` (hidden by `body.app-ready` in CSS, no script) while the packs decode. The project loads, the playhead is set to 2.8 s and `Renderer.prepare` seeks every video to that frame before `revealApp`, so the first visible frame is the real one and nothing is swapped in afterwards.
- `vite.config.mjs` no longer empties `assets/` on build, which would delete the packs; the build overwrites `demo.js` and `demo.css` in place.
- Chroma Key on the gull uses a teal key (#3593a8, tolerance 30, softness 14); the sky is a gradient, so a flat key colour is a compromise that keeps the bird's wing tips clean.
- The closing title is a text layer above a footage layer with an alpha track matte; the matte text is hidden by the compositor and the cliff video shows through its letters. Matte and effects order follows the app: effects run before the matte, so a shadow on the matted layer would be cut away.
- A saved `.fvxdemo.json` keeps the two photos but not the three videos (blob URLs), as before.

## Known gaps

- Single-line text; no audio; no 3D; no per-property graph editor; the Timeline shows transform and animated rows only.
- File names saved as `.fvxdemo.json` are the demo's own format, not `.fvxproj`.
- Software rendering is slow at Full size with several full-frame effects; use Half or Quarter for playback.
