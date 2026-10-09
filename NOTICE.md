# formvfx notices

formvfx is licensed under the Apache License 2.0 (`LICENSE`). It includes or depends on the following work.

| Component | Use | License |
| --- | --- | --- |
| wgpu, naga and their backends | GPU rendering on Metal, Vulkan, DirectX 12 and OpenGL in `native/fvx-core` | MIT OR Apache-2.0 |
| rayon | parallel loops in `native/fvx-core` | MIT OR Apache-2.0 |
| tiny-skia | path filling, stroking and gradients in `native/fvx-core` | BSD-3-Clause |
| kurbo | Bezier arc length and subdivision (Trim Paths) in `native/fvx-core` | MIT OR Apache-2.0 |
| rustybuzz, ttf-parser, fontdb | text shaping, glyph outlines and system font lookup in `native/fvx-core` | MIT, MIT OR Apache-2.0 |
| image, parking_lot, serde, serde_json | image files, locks and project files in `native/fvx-core` | MIT OR Apache-2.0 |
| i_overlay, i_float, i_shape, i_tree, i_key_sort | polygon boolean operations and offsets (Merge Paths, Offset Paths) in `native/fvx-core` | MIT OR Apache-2.0, MIT |
| usvg, svgtypes, roxmltree, simplecss, data-url, float-cmp, imagesize, pico-args, siphasher | SVG reading (SVG footage, paste as shapes) in `native/fvx-core` | MIT OR Apache-2.0, MIT |
| exr, half, lebe, bit_field, zune-inflate | OpenEXR reading and writing (EXR footage, sequences, layered frames) in `native/fvx-core` | BSD-3-Clause, MIT OR Apache-2.0 |
| hayro, hayro-syntax, hayro-interpret and their decoders, vello_cpu, skrifa, read-fonts, pic-scale, moxcms | PDF and Illustrator page rendering in `native/fvx-core` | Apache-2.0 OR MIT, MIT OR Apache-2.0, BSD-3-Clause OR Apache-2.0 |
| Foxit standard fonts (through hayro-interpret, from PDFium; Copyright 2014 PDFium Authors, original code Copyright 2014 Foxit Software Inc.) | the 14 standard PDF fonts | BSD-3-Clause |
| CGATS001Compat and LAB ICC profiles (through hayro-interpret) | CMYK and Lab colour in PDF pages | CC0-1.0 |
| miniz_oxide, brotli-decompressor | zip data in Photoshop documents and `.lottie` archives; PDF streams | MIT OR Zlib OR Apache-2.0, BSD-3-Clause/MIT |
| FFmpeg (not bundled: run as a separate program when it is installed) | video and audio decoding and movie encoding in `native/fvx-core` | LGPL-2.1 or GPL, as built by its distributor |
| MobileSAM weights, by Chaoning Zhang et al. (not bundled: downloaded on request and checked against their SHA-256) | Roto Brush and Object Matte segmentation, run by formvfx's own implementation | Apache-2.0 |
| formvis design system (NodeMode Studio Dark tokens, shell, panel and dialog styles, theme, DOM, dialog and toast helpers) | `frontend/src/styles`, `frontend/src/ui` | Apache-2.0 |
| Remix Icon paths in `frontend/src/ui/icons.ts` (via formvis) | inline SVG icons | Apache-2.0 |
| Inter and JetBrains Mono (`@fontsource`) | interface fonts | SIL Open Font License 1.1 |
| xterm.js and its fit addon | the embedded terminal | MIT |
| PyO3, maturin | the Python extension | MIT OR Apache-2.0 |
| FastAPI, Starlette, Uvicorn, python-multipart | the engine service | MIT, BSD-3-Clause |
| pywinpty (Windows only) | the terminal on Windows | MIT |
| Vite, TypeScript, Tauri API and plugins | build and desktop | MIT, Apache-2.0 |

