# demos/motion · HASHOUT_LOG

Living context for demo 3. Code carries almost no comments; the reasoning lives here.

## Shape

- Demo 1's source in `../../demo/src` is reused by path and not modified: its model, effects registry, renderer, state, importer and every panel builder. Own code lives in `src/app.ts` (the shell, menus, dock, toolbar), `src/scene.ts` (the opening composition) and `src/ext/`.
- `index.html` is hand-written with the same pre-paint theme boot as demo 1 and a boot bar that is replaced once by the revealed app. The page is one classic IIFE (`assets/motion.js`) and `assets/motion.css`, fonts and logo inlined, so `file://` works. The vite config only builds this folder (`emptyOutDir` false).
- Media travels as JS data packs (`FVX_MEDIA[name]`) as in demo 1: one VP9 sky clip and three JPEG cards. The shell hides until the clip is decoded and the first frame rendered, so nothing shifts after reveal.

## 3D (ext/fx3d.ts)

- The app has no 3D layers, so Layer 3D and Camera are pseudo-effects pushed into demo 1's `EFFECTS` under a Perspective category. Their parameters get Effect Controls rows, stopwatches, keyframes, undo and the timeline keys with no further code.
- The camera is a null layer carrying the Camera effect. A wrapper over `renderer.render` renders each 3D layer flat into a texture and projects it through a look-at camera onto a subdivided triangle mesh, depth-sorts runs of 3D layers and draws the 2D runs over the accumulated frame by feeding it back as a footage layer. The viewer, thumbnails and every export call `renderer.render`, so all of them show 3D.
- Known gaps: no lights, shadows or intersection; depth sort is per layer, not per pixel.

## Expressions (ext/expr.ts)

- Stored per property on the layer (`ex`). A token allow-list check rejects anything outside a fixed vocabulary (no property access beyond a list, no bracketed strings), then the source is compiled and evaluated per frame into a resolved clone of the composition, so keyframes stay untouched.
- Errors are kept per layer and property, shown in the panel and the keyframed value is used.

## Template Controls (ext/tc.ts, ext/panels.ts)

- The controls live on the composition (`tc`): text, colour, slider and checkbox, each bound to one or more targets by path. Editing applies the value to the targets and `control("Label")` reads it from expressions. The panel is built from the app's own `fx-eg-*` classes so it matches the app's Essential-style panel under the approved Template Controls name.

## Opening project

- The composition is 9 s so the sky clip, camera and every animator end together. The camera moves 3 keyed positions and a roll; cards fly in from depth with Depth Fade; the title uses a text animator preset plus a wave animator. The counter is an expression on Source Text.

## Verification notes

- Quarter size is used for every render in verification on the shared two-core machine; the WebM export is 320x180, 9.0 s.
- Pressing a timeline row name is the reliable way to select a layer in a script; the stage click deselects.
