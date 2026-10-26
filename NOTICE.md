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
| fancy-regex, regex-automata, regex-syntax, aho-corasick, bit-set, bit-vec | regular expressions in expressions and scripts in `native/fvx-core` | MIT, MIT OR Apache-2.0, Unlicense OR MIT |
| i_overlay, i_float, i_shape, i_tree, i_key_sort | polygon boolean operations and offsets (Merge Paths, Offset Paths) in `native/fvx-core` | MIT OR Apache-2.0, MIT |
| usvg, svgtypes, roxmltree, simplecss, data-url, float-cmp, imagesize, pico-args, siphasher | SVG reading (SVG footage, paste as shapes) in `native/fvx-core` | MIT OR Apache-2.0, MIT |
| exr, half, lebe, bit_field, zune-inflate | OpenEXR reading and writing (EXR footage, sequences, layered frames) in `native/fvx-core` | BSD-3-Clause, MIT OR Apache-2.0 |
| hayro, hayro-syntax, hayro-interpret and their decoders, vello_cpu, skrifa, read-fonts, pic-scale, moxcms | PDF and .ai page rendering in `native/fvx-core` | Apache-2.0 OR MIT, MIT OR Apache-2.0, BSD-3-Clause OR Apache-2.0 |
| Foxit standard fonts (through hayro-interpret, from PDFium; Copyright 2014 PDFium Authors, original code Copyright 2014 Foxit Software Inc.) | the 14 standard PDF fonts | BSD-3-Clause |
| CGATS001Compat and LAB ICC profiles (through hayro-interpret) | CMYK and Lab colour in PDF pages | CC0-1.0 |
| miniz_oxide, brotli-decompressor | zip data in PSD documents and `.lottie` archives; PDF streams | MIT OR Zlib OR Apache-2.0, BSD-3-Clause/MIT |
| FFmpeg (not bundled: run as a separate program when it is installed) | video and audio decoding and movie encoding in `native/fvx-core` | LGPL-2.1 or GPL, as built by its distributor |
| SAM 2.1 tiny weights (`sam2.1_hiera_tiny.pt`), by Meta (not bundled: downloaded on request from `dl.fbaipublicfiles.com` and checked against their SHA-256) | roto and Object Matte segmentation with a memory of earlier frames, run by formvfx's own implementation | Apache-2.0 (the SAM 2 repository states that its model checkpoints are licensed under Apache 2.0) |
| MobileSAM weights, by Chaoning Zhang et al. (not bundled: downloaded on request and checked against their SHA-256) | Roto Paint and Object Matte segmentation, run by formvfx's own implementation | Apache-2.0 |
| MediaPipe Face Landmarker (BlazeFace short-range detector and Face Mesh V2), by Google (not bundled: downloaded on request and checked against its SHA-256) | face tracking, run by formvfx's own TensorFlow Lite interpreter | Apache-2.0 |
| wasmi, wasmi_core, wasmi_ir, wasmi_collections, wasmparser, wat, wast, wasm-encoder, spin, libm, string-interner, hashbrown, foldhash, bumpalo, leb128fmt, unicode-width | the WebAssembly runtime for effect plug-ins in `native/fvx-core` | MIT OR Apache-2.0, Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT, MIT, Zlib |
| libloading | loading OpenFX plug-in binaries in `native/fvx-core` | ISC |
| OpenFX image effect API, by the OpenFX project (the suite layouts and property names from its published headers, declared again in Rust; no header is shipped) | the OpenFX host in `native/fvx-core/src/effects/ofx.rs` | BSD-3-Clause |
| formvis design system (NodeMode Studio Dark tokens, shell, panel and dialog styles, theme, DOM, dialog and toast helpers) | `frontend/src/styles`, `frontend/src/ui` | Apache-2.0 |
| Remix Icon paths in `frontend/src/ui/icons.ts` (via formvis) | inline SVG icons | Apache-2.0 |
| Inter and JetBrains Mono (`@fontsource`) | interface fonts | SIL Open Font License 1.1 |
| xterm.js and its fit addon | the embedded terminal | MIT |
| PyO3, maturin | the Python extension | MIT OR Apache-2.0 |
| FastAPI, Starlette, Uvicorn, python-multipart | the engine service | MIT (FastAPI), BSD-3-Clause (Starlette, Uvicorn), Apache-2.0 (python-multipart) |
| pywinpty (Windows only) | the terminal on Windows | MIT |
| Vite, TypeScript, Tauri API and plugins | build and desktop | MIT, Apache-2.0 |
| pytest 9.1.1 (with iniconfig, packaging, pluggy, pygments) and httpx 0.28.1 | runs `backend/tests` in CI (`backend/requirements-test.txt`); development only, never installed in the app | MIT (pytest, iniconfig, pluggy), Apache-2.0 OR BSD-2-Clause (packaging), BSD-2-Clause (pygments), BSD-3-Clause (httpx) |

## Python packages frozen into the engine

The engine is frozen with PyInstaller `--onefile` (`setup/freeze_backend`). These packages are installed in `backend/venv` from `backend/requirements.txt` and are collected by the freeze, directly or as dependencies of fastapi and uvicorn[standard]; each licence is read from the package's installed metadata. "Frozen in" is inferred from the freeze flags and imports, not from a built bundle. maturin is a build tool and is not frozen.

| Package | Licence |
| --- | --- |
| CPython runtime and standard library | PSF-2.0 |
| typing_extensions | PSF-2.0 |
| PyInstaller 6.22.3 (its bootloader is the executable stub of the frozen engine) | GPL-2.0-or-later with the PyInstaller exception that permits distributing the frozen program under another licence (PyPI metadata for 6.22.3) |
| pydantic, pydantic_core, annotated-types, annotated-doc, typing-inspection, anyio, h11, httptools, PyYAML, watchfiles | MIT |
| uvloop | MIT or Apache-2.0 |
| click, idna, python-dotenv, websockets | BSD-3-Clause |
| fvx-core (formvfx's own compositing engine) | Apache-2.0 |

certifi is not installed in the engine's environment and is not frozen.

## Desktop shell: crates under other licences

Tauri links these crates unedited (no `[patch]` and no vendored copy):

| Crate | Licence | Why it is there |
| --- | --- | --- |
| cssparser, selectors, dtoa-short, option-ext | MPL-2.0 | Tauri's HTML handling (tauri-utils); used unmodified, source at crates.io |
| ring | Apache-2.0 AND ISC | TLS for the updater (rustls) |
| webpki-root-certs | CDLA-Permissive-2.0 | root certificates for the updater's TLS |
| r-efi | MIT OR Apache-2.0 OR LGPL-2.1-or-later | used under MIT |

The Linux AppImage also carries the build machine's WebKitGTK, GTK and GLib (LGPL-2.1-or-later) as separate shared libraries.

## Trademarks and other products' files

formvfx names no other product or company (`setup/lint_text.py` holds that); file formats are named by their extension. Any product names a person may associate with those formats are the trademarks of their owners, and formvfx is not affiliated with or endorsed by any of them. It opens the user's own `.aep` projects, `.mogrt` templates and `.ffx` presets with its own readers, runs scripts and CEP extension panels against interfaces it implements itself (the `CSInterface` shim in `backend/app/routers/extensions.py` is formvfx's code), and contains no other vendor's code, presets, templates or sample media.
