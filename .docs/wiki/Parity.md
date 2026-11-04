# Parity

formvfx is measured against the reference compositor row by row in [`docs/PARITY.md`](https://github.com/nodemode-dev/formVFX/blob/main/docs/PARITY.md): 72 capability rows across project and footage, compositions, layers, animation, text, shapes, masks and rotoscoping, effects, 3D, tracking, expressions and scripting, preview, rendering, colour, audio and the interface. Each row is traced through the engine command, the interface that exposes it and the render or export path, and marked Full, Partial or None, with whether it was run end to end in that build.

The page also lists where formvfx trails, in order of priority, and where it leads.

## Declined

Content-Aware Scale (seam-carving, content-aware resizing of images and layers) and its skin-tone protection option are declined and will not be built, because the techniques are covered by patents. Requests for them are answered with this note.
