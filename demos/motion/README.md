# FormVFX demo 3: motion design

A working, offline demo of FormVFX motion design in a browser tab: kinetic type with text animators, animated shapes, expressions, Template Controls, 3D layers with a camera move, and the Render Queue. Nothing is uploaded and nothing is fetched. The interface is FormVFX's own (the Studio Dark tokens, the dock, the viewer, Effect Controls, the Timeline, the Template Controls and Expressions panels); the engine behind it is a compact compositor in TypeScript on Canvas 2D, because FormVFX's real engine is Rust and cannot run in a page. Demo 1 is at `../../demo/index.html` and demo 2 at `../keying/index.html`; the link row in the toolbar moves between the demos.

## Open it

- Double-click `demos/motion/index.html` (it runs from `file://`), or serve the repo with any static server and open `demos/motion/`.
- The page reads its code and media from this folder and from nothing else. Opening takes a few seconds on a slow machine while the sky clip is decoded; the window appears once, finished.

## What it opens on

**Spring Architecture Week**, one 9 s composition at 1280x720, 30 fps:

- A blue cloud timelapse as the plate, graded.
- Three architecture photos hung in space as 3D cards, with a camera that dollies in, orbits and settles while the cards fly in with depth fade.
- A kinetic title: SPRING with a tracking animator, ARCHITECTURE with Bounce In and a wave animator, a subtitle that fades up.
- A ring, a bar, a star and a hexagon animated with eased and overshoot keyframes; a counter text driven by an expression; three dots that pop in.
- A Finish adjustment layer with saturation, glow and vignette.
- Template Controls for the title, kicker, subtitle, accent colour, wiggle amount and the counter.

## What works

- **Text animators.** Layer > Text Animation Preset lists the 169 presets; Text Animator in Effect Controls adds one; Start, Delay per Character and the rest are properties with stopwatches.
- **Shape animation.** Rectangle, ellipse, star and polygon layers with keyframes and the interpolation menu (Animation > Keyframe Interpolation), Wiggler and Smoother.
- **Expressions.** The Expressions panel writes one expression per property (`wiggle`, `loopOut`, `valueAtTime`, `linear`, `ease`, `clamp`, `noise`, `layer("Name")`, `control("Label")`, `time`, `value`, `index`). A mistake shows its message and the keyframed value is used.
- **Template Controls.** The panel edits text, colour, slider and checkbox controls bound to layers; edits show in the viewer at once and `control("Label")` reads them from expressions.
- **3D.** Layer > Make Layer 3D adds Position Z, X and Y Orientation and Depth Fade to a layer; Layer > New > Camera adds a null with position, point of interest, zoom and roll, all keyframeable.
- **Render Queue.** PNG, contact sheet, PNG sequence, GIF and WebM, at full, half or quarter size. Viewer, thumbnails and every export go through the same renderer, so they all show the 3D.
- **Your own file.** Open your file, File > Import Footage, or drop an image or video on the page.

## What it does not do

3D layers are flat layers projected by a look-at camera: no lights, shadows, extrusion or 3D text, and no depth intersection between layers. Expressions are a safe subset (no free JavaScript, no property links by pickwhip). Template Controls are bound inside the page, not exported as a motion graphics template. See `HASHOUT_LOG.md`.

## Rebuild it

The built page is committed (`index.html` and `assets/`). The build uses the interface's own Vite from `frontend/`; nothing is installed here.

```sh
cd demos/motion
node ../../frontend/node_modules/vite/bin/vite.js build --config ./vite.config.mjs
../../frontend/node_modules/.bin/tsc -p tsconfig.json
```
