# formvfx

A motion graphics and visual effects compositor for the desktop and the browser, in the form family beside formvis and formcut. Compositions, layers, keyframes and the Graph Editor, 306 effects, WebAssembly effect plug-ins and OpenFX plug-ins (the effects other OpenFX hosts load), Essential Graphics templates (`.mogrt` files and `.aep` projects open), text animators with 169 presets, shape layers and SVG paste, masks, Roto Brush, paint, puppet pins, 3D cameras and lights, tracking and stabilizing, expressions, scripts and ScriptUI panels, colour management and a Render Queue, with an agent harness so Claude Code, Codex and Cursor can drive every tool from formvfx's embedded terminal.

Version 0.51.1 · Apache-2.0 · compositing engine: formvfx's own, in Rust (`native/fvx-core`)

## Quick start

Needs Node 20.19+ or 22.12+, Python 3.12+, and Rust 1.95+ (`https://rustup.rs`). `./formvfx doctor` checks them.

```bash
./formvfx install      # engine venv, the Rust compositing engine (the first build takes a few minutes), interface packages
./formvfx              # boots the engine on 127.0.0.1:8230 and the interface on http://localhost:5230
```

The GPU compositor is part of the default build and is used when the engine finds a working adapter (Settings > Memory & CPU shows which; `FORMVFX_GPU=0` keeps everything on the CPU). Windows: `formvfx.bat install`, then `formvfx.bat`. One process serving the built interface: `./formvfx serve` (http://127.0.0.1:8230).

Open http://localhost:5230, choose **Open the demo project** on the start screen, and work in the panels: Project, Effect Controls, Composition viewer, Timeline, Effects and Presets, Preview, Character, Render Queue. Cmd or Ctrl+K searches every command and effect. The interface comes in 26 languages (Settings > Interface > Language), English by default. The Flowchart (Composition > Composition Flowchart) is also a node editor: turn on Layers, then drag a layer's link or matte port onto another layer to parent it or set its track matte. With Effects on, each effect is its own node between a layer's source and the layer: drag it onto another effect to reorder the stack, right-click a layer to add an effect, and drag an effect's link port onto a layer to use that layer in it (Displacement Map Layer, for example). While you work, the work area renders into the preview cache on its own (a green strip on the time ruler marks what is ready), scrubbing shows cached frames at once, and View > Fast Previews > Adaptive Resolution keeps scrubbing light on heavy comps.

## The engine

`native/fvx-core` is formvfx's compositing engine, written for formvfx in Rust and loaded by the engine service as the `fvx_core` Python extension; `./formvfx install-native` rebuilds it. It saves projects as `.fvxproj` and opens `.ecproj` and `.ecprojx` projects from earlier formvfx releases as imports (they are never written). It renders on the GPU through wgpu or on the CPU, whichever is faster for each composition, reads footage through FFmpeg when FFmpeg is installed and stills, image sequences, OpenEXR, PSD (as footage, one layer or a composition), PDF and .ai pages and Lottie on its own, and covers compositions, precomps, solids, shapes, text with character styles, OpenType features and variable fonts, text animators with range, wiggly and expression selectors, text on a path and per-character 3D, the 169 text animation presets, shape layers with every standard operator and tapered, wavy and dashed strokes, SVG import and paste as shapes, shapes and masks from text, nulls, cameras with depth of field, lights with soft shadows, glTF, GLB and OBJ models and 3D primitives, MaterialX materials, extruded and bevelled text and shape layers with their own front, bevel and side colours (animatable per character), reflections of other layers and the environment, transparency and refraction, Classic and Advanced 3D, 360 VR environments and stereo 3D rigs, environment lights and the 3D views, point, planar and mask tracking, face tracking on the learned MediaPipe Face Landmarker (its own TensorFlow Lite interpreter), the Warp Stabilizer and the 3D Camera Tracker, Roto Brush and Object Matte (with formvfx's own MobileSAM and SAM 2 runtimes, SAM 2 keeping a memory of earlier frames), paint, puppet pins, Content-Aware Fill, masks with variable feather and Smart Mask Interpolation, Pixel Motion frame blending, track mattes, 39 blend modes, layer styles, keyframes with eases, roving keys and the Graph Editor, the Keyframe Assistant, Wiggler, Smoother and Motion Sketch, time stretch, time remapping and freeze frames, markers with chapters and cue points, Responsive Design: Time, Collect Files and the other dependency tools, animation presets (`.ffx` among them), snapshots and a locked viewer, expressions on its own interpreter (with regular expressions, `sampleImage` and the text style API), scripts against the standard compositor scripting object model with ScriptUI windows, modal dialogs and dockable panels, CEP extension panels (`CSXS/manifest.xml` folders and `.zxp` files, with `CSInterface.evalScript`), viewer exposure, custom channels, Fast Previews, an Extended Viewer window and the Color Scopes as numbers, Essential Graphics templates (`.fvxtemplate` and `.mogrt`) with per-instance Essential Properties and Media Replacement, `.aep` projects and the `.mogrt` templates built on them, read by its own RIFX reader into compositions, layers, keyframes, expressions, effects and text animators, 306 effects (with the Expression Controls) including the 3D Channel set on OpenEXR passes and Cryptomatte, keying, Color Grade, the OCIO transforms, the CC set, particles and simulations, immersive video and the 13 audio effects, Liquify strokes, 8, 16 and 32-bit projects with colour management, OCIO and ACES (its own config reader and transforms), a disk cache, proxies, the Render Queue with Render Settings, several output modules, templates, field rendering and pulldown, multi-frame rendering and HDR output to H.264, HEVC, AV1, ProRes 4444, WebM, GIF, PNG, TIFF, EXR, WAV and AIFF, layered PSD and EXR frames, Lottie export, editor timelines in and out (xmeml 5 XML (.xml), FCPXML, OpenTimelineIO and CMX 3600 EDL, with placeholders for media it cannot find), project templates, an XML twin of the project file (`.fvxprojx`), auto-save, recent projects and footage, a render Watch Folder, a Footage panel with Overlay and Ripple Insert Edit, a Media Browser that sees numbered stills as sequences, background jobs you can list and cancel and in-app tutorials, stackable procedural behaviours (wiggle, oscillate, spring, bounce, loops), two-bone IK rigs with an FK switch, 2D physics baked to keyframes, cloners with live per-copy offsets, data-driven animation from CSV and JSON, Slices from Guides, Scene Edit Detection, Auto-trace, Create Nulls from Paths, work-area lift and extract and property-link copies, and loads sandboxed WebAssembly effect plug-ins and OpenFX plug-ins with their on-screen handles ([`docs/PLUGINS.md`](docs/PLUGINS.md)); [`docs/MANUAL.md`](docs/MANUAL.md) walks through the application and [`docs/PARITY.md`](docs/PARITY.md) grades it and lists what it does not do yet.

## Agents

- **In formvfx**: Claude Code starts inside the app already connected, with no setup step: Window > Start Claude Code (Shift+Cmd or Ctrl+J) runs it in the Terminal panel, the Ask Claude Code field there sends it a request, and Window > Chat with Claude (Alt+Cmd or Ctrl+J) opens a Chat panel that runs it headless, streams the answer with each tool it uses, and hands the conversation to the terminal. It edits the open project live, every change undoable, without permission prompts. Claude Code is found wherever it is installed, also from the packaged app. Codex and Cursor start from the Terminal panel's buttons with the `formvfx` MCP server and the `formvfx-harness` skill.
- **From this repository**: `.mcp.json`, `.codex/config.toml` and `.cursor/mcp.json` run `./formvfx mcp`. The first change asks once in the formvfx window.
- **From anywhere**: `./formvfx agents install` (or the Agents panel) sets the three clients up for your account.
- **Dynamic Link**: Export Dynamic Link Clip (Project panel) writes a `.fvxlink` that an editor such as formcut plays live with no render. Links keep working when the project is moved or renamed (found again by its project id beside the `.fvxlink`, in its old folder or among recent projects) and when a comp is renumbered (found by name). `POST /api/link/check` reports and fixes a link, `POST /api/link/render` renders the comp for Render and Replace, and Window > Dynamic Link shows what is being served.
- **Command line**: `./formvfx state`, `commands FILTER`, `describe ID`, `exec ID 'JSON'`, `batch 'JSON'`, `frame OUT.png --time S`, `render --wait`.

## Desktop

```bash
./build_app                 # from a fresh clone to an installed app: tools check, install, interface build, engine freeze, bundle
./build_app --open          # the same, then open it (--clean rebuilds from nothing; Linux: --install installs the .deb)
./formvfx desktop dev       # the app, with the engine frozen as its sidecar
```

`./formvfx desktop build` is the same as `./build_app`; on Windows run `build_app.bat`. On macOS the app is copied to Applications and a DMG is packed beside the bundle, and the desktop app puts formvfx's menus in the macOS menu bar.

Pushing a `compile_vX.Y.Z` branch has formcompiler (`nodemode-ai/formcompiler`) build macOS, Linux and Windows bundles and open a draft release `formvfx-vX.Y.Z` on its release page.

## Commands

| Command | Does |
| --- | --- |
| `./formvfx` / `boot` | engine and interface for development |
| `install`, `install-native` | everything, or only the Rust compositing engine (`--dev` for a quicker debug build) |
| `serve` | the built interface from the engine |
| `install-desktop`, `freeze`, `desktop dev`, `desktop build` | the desktop app (`desktop build` runs `./build_app`) |
| `doctor` | checks the toolchain and ports |
| `claude [ARGS]` | Claude Code in this folder, connected to formvfx |
| `mcp` | the MCP server over stdio |
| `agents install|sync|status` | client setup; regenerate the skill references from the engine |
| `counts [--check]` | the code size table below |
| `presets [--check]` | regenerate the text animation preset library (`presets/text-animators.json`) |
| `clean [--all]` | remove build output |

## Layout

`native/fvx-core` (the compositing engine, Rust, PyO3) · `native/plugins` (an example WebAssembly effect plug-in) · `backend` (FastAPI engine service, MCP, terminal) · `frontend` (TypeScript and Vite interface) · `setup` (scripts) · `presets` (the text animation preset library) · `.claude/skills/formvfx-harness` (the agent skill, mirrored to `.agents/skills`).

## Parity

[`docs/PARITY.md`](docs/PARITY.md) grades formvfx against the reference compositor on 72 traced capability rows: 68 Full, 3 Partial, 1 None, a score of 69.5 of 72 (97%). It lists where formvfx trails, in priority order, and where it leads. The wiki (`.docs/wiki`) covers getting started, agents, the architecture, the desktop app and releasing.

## Code size

| Area | Files | Lines |
| --- | ---: | ---: |
| Compositing engine (Rust, fvx-core) | 170 | 84,535 |
| Engine service (Python) | 38 | 3,602 |
| Interface (TypeScript) | 60 | 14,263 |
| Interface styles (CSS) | 11 | 2,088 |
| Desktop shell (Rust, Tauri) | 1 | 141 |
| Setup and CLI | 35 | 1,121 |
| Agent skill (Markdown, hand-written) | 16 | 786 |
| Total | 331 | 106,536 |

Non-blank lines in tracked files; generated skill references excluded. `./formvfx counts` prints this table.

## Trademarks

Adobe and After Effects are either registered trademarks or trademarks of Adobe in the United States and/or other countries. Other product names are the trademarks of their owners. formvfx is not affiliated with or endorsed by any of them; they are named only to identify file formats and compatibility. See `NOTICE.md`.
