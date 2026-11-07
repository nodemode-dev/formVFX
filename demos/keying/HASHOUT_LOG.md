# demos/keying · HASHOUT_LOG

Living context for demo 2. Code carries almost no comments; the reasoning lives here.

## Shape

- Demo 1's source in `../../demo/src` is reused by path and not modified. Only what had to change lives here: `src/app.ts` (the shell, tabs, menus, toolbar), `src/engine/{keying,roto,tracker,project}.ts`, `src/ui/{overlay,trackpanel,tools}.ts`, `src/shim/model.ts`, `src/media.ts`.
- `vite.config.mjs` carries one plugin, `keying-model-shim`. Any import of `demo/src/engine/model.ts` from a file outside this folder resolves to `src/shim/model.ts`, which re-exports the original and replaces only `layerProps`, so demo 1's controls and timeline list a path mask's rows without a change. Files in this folder import the original directly, so there is one instance of the model module.
- `index.html` is hand-written with the same pre-paint theme boot as demo 1; the page is one classic IIFE (`assets/keying.js`) and `assets/keying.css`, fonts and logo inlined, so `file://` works.
- Media travels as JS data packs (`FVX_MEDIA[name]`), as in demo 1. The dusk footage that plays in the phone is demo 1's pack, loaded by relative path rather than copied (it is 3.6 MB).

## Effects (engine/keying.ts)

- They push into demo 1's `EFFECTS` and `CATEGORIES` arrays before the app starts, so the Effect menu and Effects and Presets list them with no further code.
- Keylight: the screen channel is the largest of the key colour; the reference is a blend of the other two by Screen Balance; matte = 1 - gain * (screen - reference) / the key colour's own difference; Clip Black and White stretch it; Shrink/Grow is a separable min or max filter on the alpha plane; Softness blurs it. Despill limits the screen channel to a blend of the others set by Despill Bias.
- Matte View turns alpha into an opaque grey. The toolbar's View buttons toggle effects on the keyed layers (Source: key effects off; Final; Matte: Matte View on), so the chokers show in the matte.

## Mask paths (engine/roto.ts)

- A path mask is a normal `Mask` with an extra `path` property, a `Prop` whose value is a flat list of x,y numbers. Keyframes of that Prop interpolate component by component, which is exactly what a point-for-point animated path needs; the Timeline already draws its keys.
- `installPathMasks` replaces `Renderer.prototype.maskCanvas` so a path is filled as a closed curve through the midpoints of its vertices. It repeats the rectangle and ellipse branches unchanged.
- The baked outline has 100 points per keyframe, resampled by arc length from the contour point nearest the frame's bottom left corner, so every keyframe has the same vertex order. Radial sampling cut the thumb gap and was dropped.

## Tracker (engine/tracker.ts, ui/trackpanel.ts, ui/overlay.ts)

- Analysis uses its own hidden video element per footage, so scrubbing and playback never fight it. Each frame is seeked, drawn at half size, turned to grey, and the feature box from the start frame is matched by sum of absolute differences inside the search box around the last position, with a parabola for the sub-pixel part. "Adapt Feature on Every Frame" replaces the template each frame.
- Apply is the same as the app's: keyframes. Transform writes Position on the Motion Target as the tracked comp point plus the offset the target had at the start frame. Stabilize writes the layer's Position minus the drift and multiplies Scale by the margin needed.
- The track of the opening project is not run at load (240 seeks). It was produced once by this tracker in a headless run, stored as `assets/media/track-corner.js`, and Apply runs at load for the three targets.
- The overlay is a canvas inside the viewer frame. It takes pointer events only while the pen or track tool is on and stops them before the viewer's own handlers.

## Known gaps

- One point, translation only; no search regions that follow motion, no confidence score (see CLAUDE.md on declined features).
- Mask Path has no Bezier handles, expansion or variable feather.
- The Source view is a toggle of effects, not Keylight's own view menu.
