# FormVFX demo 2: keying, masking and tracking

A working, offline demo of FormVFX's keying, rotoscoping and tracking in a browser tab. Nothing is uploaded and nothing is fetched. The interface is FormVFX's own (the Studio Dark tokens, the dock, the viewer, Effect Controls, the Timeline, the Tracker panel layout); the engine behind it is a compact compositor in TypeScript on Canvas 2D, because FormVFX's real engine is Rust and cannot run in a page. Demo 1 is at `../../demo/index.html`; the link row in the status bar moves between the demos.

## Open it

- Double-click `demos/keying/index.html` (it runs from `file://`), or serve the repo with any static server and open `demos/keying/`.
- The page reads the dusk footage that plays inside the phone from `../../demo/assets/media/`, so keep the `demo` folder beside `demos`.

## What it opens on

**Keying Lab**, three compositions on one real green-screen clip (a hand holding a phone, outdoors):

1. **Screen Key.** The green screen is keyed with Keylight, Advanced Spill Suppressor and Matte Choker; dusk footage plays behind it and is locked to the phone by a track; a pulsing callout is parented to the tracked corner. The View buttons in the toolbar switch Source, Final and Matte.
2. **Roto Isolate.** A hand-keyed Mask Path (100 points, 11 keyframes) cuts the hand and phone out; the background is desaturated, darkened and blurred beneath it.
3. **Lock Off.** Stabilize Motion applied to the same track: the phone holds still against the marker ring. Toggle the eye of "Phone (stabilized)" to see the raw handheld clip.

## What works

- **Keying.** Keylight (Screen Colour, Screen Gain, Screen Balance, Despill Bias and Amount, Clip Black and White, Shrink/Grow, Softness), Advanced Spill Suppressor (standard and ultra), Matte Choker, Simple Choker, Matte View. Chroma Key and Luma Key from demo 1 are there too.
- **Mask Paths.** The pen tool (G) draws a closed path on the selected layer: click points, Enter or double-click or a click on the first point closes it. Drag points to reshape. On an animated path a drag writes a keyframe at the playhead. Feather and Opacity are in Effect Controls; Mask Path has a stopwatch like the app's.
- **Tracker panel.** Track Motion or Stabilize Motion on the selected video layer, place the point with the track tool, set Feature Size and Search Size, Analyze Backward or Forward (with progress and Stop), choose a Motion Target, Apply. A transform track writes Position keyframes on the target; a stabilize track writes counter-motion on the layer and scales it up to hide the edges. The viewer draws the feature box, the search box and the trail.
- **Render Queue.** PNG, contact sheet, PNG sequence, GIF and WebM, at full, half or quarter size.
- **Your own file.** Open your file, File > Import Footage, or drop an image or video on the page; track it, mask it, key it.

## What it does not do

No GPU, Roto Paint propagation, face or planar tracking, Wrap Stabilization, 3D camera tracking or rotation and scale tracking: the tracker follows one point with template matching. Mask paths are straight vertices smoothed by the renderer, not Bezier handles. See `HASHOUT_LOG.md`.

## Rebuild it

The built page is committed (`index.html` and `assets/`). The build uses the interface's own Vite from `frontend/`; nothing is installed here.

```sh
cd demos/keying
node ../../frontend/node_modules/vite/bin/vite.js build --config ./vite.config.mjs
../../frontend/node_modules/.bin/tsc -p tsconfig.json
```
