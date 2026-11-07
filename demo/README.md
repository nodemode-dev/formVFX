# FormVFX browser demo

A working, offline demo of FormVFX in a browser tab. Nothing is uploaded, nothing is fetched: every script, style, font and sample is in this folder.

## Open it

- Double-click `demo/index.html` (it runs from `file://`), or
- serve the folder with any static server, for example `python3 -m http.server -d demo 8000`.

## What it shows

The interface is FormVFX's own: the Studio Dark tokens and stylesheets, the inline icon module, the dock, the viewer bar, the timeline, Effect Controls, the Effects and Presets list, the Project panel and the Render Queue, all taken from `frontend/src`. The engine behind it is a compact compositor written in TypeScript on Canvas 2D, because FormVFX's real engine is Rust (`native/fvx-core`) behind a Python service and cannot run in a page.

Open your own file with **Open your file**, File > Import Footage, or by dropping it anywhere on the page:

- images (PNG, JPEG, WebP, GIF, AVIF, SVG) and video the browser can decode (WebM, MP4, and others) become footage and a layer;
- a `.fvxdemo.json` project saved from this demo opens again, with its images.

Then work on it:

1. **Composite.** Text, solid, rectangle, ellipse, star, polygon, adjustment and null layers; 17 blend modes; parenting; track mattes (alpha and luma, inverted or not); animated rectangle and ellipse masks with feather; layer in and out trimming, split and duplicate.
2. **Animate.** Keyframes on every transform, shape, text, mask and effect value, with Linear, Hold, Ease In, Ease Out, Easy Ease and Overshoot; drag a key in the timeline to retime it; text animation presets (Bounce In Characters, Fade Up, Typewriter, Scale Pop, Rotate In, Drop In, Track In, Wave); Wiggler and Smoother in the Animation menu.
3. **Effects.** 23 effects across Blur and Sharpen, Color Grade, Stylize, Distort, Generate and Keying (Gaussian Blur, Glow, Drop Shadow, Hue and Saturation, Gradient Ramp, Chroma Key, Luma Key, Wave Warp, Twirl and more), applied to a layer or, on an adjustment layer, to everything below it.
4. **Inspect.** Color Scopes (waveform, RGB parade, histogram, vectorscope) and the min, max and mean of the frame in the viewer; Info shows the comp, the selection and the last render time.
5. **Render.** The Render Queue writes a PNG still, a 12-frame contact sheet, a PNG sequence in a `.zip`, an animated GIF and a WebM video, at full, half or quarter size, for the work area or the whole comp, every Nth frame.

A generated **Launch Week** composition (text, a procedural sunset field, an orb, a ring and a grade) and an **Intro** composition (a text matte, a mask, a rotating star and an animated aberration) open first, so the demo works with no file at all.

Keys: Space plays, Left and Right step a frame (Shift ten), Home and End jump, B and N set the work area, U opens a layer's properties, Delete removes the layer, Cmd or Ctrl with Z, Shift+Z, D, Shift+D, S and I undo, redo, duplicate, split, save and import.

## What it does not do

The demo does not run FormVFX's Rust engine: no GPU renderer, OpenFX or WebAssembly plug-ins, 3D, tracking, Roto Paint, expressions, audio, `.aep`, `.mogrt` or `.fvxproj`. Effects are a faithful subset written for this page; see `HASHOUT_LOG.md`.

## Rebuild it

The built page is committed (`index.html` and `assets/`). To rebuild after changing `src/`:

```sh
cd frontend && npm ci          # the build uses the interface's own Vite and TypeScript
cd .. && node demo/build.mjs   # writes demo/assets/demo.js and demo/assets/demo.css
frontend/node_modules/.bin/tsc -p demo/tsconfig.json   # type check
```

`demo/vite.config.mjs` builds one classic script (IIFE) with the fonts and the logo inlined, so `file://` works. It sets `server.host: false` on port 5251 for `vite --config demo/vite.config.mjs` development.
