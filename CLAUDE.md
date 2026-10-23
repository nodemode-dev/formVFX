# formvfx

A motion graphics and visual effects compositor in the form family beside formvis and formcut: a vanilla TypeScript and Vite interface, a Python FastAPI engine service around formvfx's own Rust compositing engine (`fvx_core`, wrapped with PyO3), and a Tauri 2 desktop shell.

## Two kinds of work

**Making motion graphics with formvfx** (a title, an animation, an effect, a render): use the `formvfx` MCP server (`.mcp.json` runs `./formvfx mcp`, which starts the engine when needed) with the `formvfx-harness` skill in `.claude/skills/formvfx-harness`. Start with `status`, build with `batch`, look with `render_frame` and `contact_sheet`. The first change asks the person once in the formvfx window; agents started from formvfx's Terminal panel never ask.

**Changing formvfx's code**: route through `.claude/skills/formvfx-skillset/SKILL.md` (subsystem map, rules, specialists) and read the living logs `frontend/HASHOUT_LOG.md`, `backend/HASHOUT_LOG.md` and `native/fvx-core/HASHOUT_LOG.md` before editing.

## Layout

- `native/fvx-core/` · the compositing engine (`fvx_core`), written for formvfx in Rust: project model, commands, undo, renderer (CPU and GPU), effects, colour management, tracking, roto, formats and the Render Queue, exposed to Python with PyO3. `.fvxproj` is the project file; `.ecproj` and `.ecprojx` projects from earlier formvfx releases open as imports. Build with `./formvfx install-native`; read `native/fvx-core/HASHOUT_LOG.md` and the `engine-core` specialist before changing it.
- `backend/app/` · the engine service: `engine/` (the engine thread, the event bus, JSON-RPC), `routers/` (live socket, viewer frames, files, terminal, MCP, agents), `agent/` (MCP tools, consent, workspace, CLI).
- `frontend/src/` · the interface: `engine/` (client, store, frames, playback; no DOM), `ui/` (dock, menus, palette, widgets, formvis design files), `panels/`, `app/`.
- `setup/` · install, boot, build, freeze and desktop scripts behind the `./formvfx` dispatcher; `setup/agents.py` regenerates the harness skill's references from the engine.
- Planning documents (DEV_PLAN, BUILD_NOTES, TEST_PLAN, the root HASHOUT_LOG) ship in the planning sidecar zip, not in this tree.

## Roto Paint propagation and Selection-Aware-Fill (declined for patent risk, 2026-10-23)

- Roto propagation stays global: one whole-frame optical flow warps the previous matte, or the SAM 2 path carries it. Never add search windows or regions placed along the object contour whose size or search radius follows the local motion, and never blend such windows into a map that drives the next frame's search.
- Selection-Aware-Fill never gets a per-frame confidence score, a threshold that flags a frame, or any prompt or suggestion that asks the person to fix, modify or supply a specific frame as the reference. The reference stays an optional choice the person makes unprompted. Never weight boundary motion by a forward and backward flow consistency check, or by any sigmoid of it.
- See "Declined" in `docs/PARITY.md`.

## After changing the MCP tools or the engine's commands

Run `./formvfx agents sync` so `.claude/skills/formvfx-harness/references/` and the `.agents/skills` mirror follow; `./formvfx agents sync --check` fails while they drift.

## Trademarks, copyright and licences

- No other product or company is named anywhere in the tree (`setup/lint_text.py` fails on a listed name; see the root HASHOUT_LOG, "Names and declined features"); formats are named by extension (`.aep`, `.mogrt`, `.ffx`) and the scripting model as the standard compositor scripting object model. Other products' feature names are not formvfx command, panel, effect, tool or agent tool names either. The approved label renames are Template Controls (Essential Graphics), Live Link (Dynamic Link), Selection-Aware-Fill (Content-Aware Fill), Roto Paint (Roto Brush) and Wrap Stabilization (Warp Stabilizer); Liquify stays. Command ids keep their names. `NOTICE.md` says formvfx is not affiliated with the owners of the formats it opens.
- Never copy vendor help or documentation text into effect names, parameter descriptions or the manual; never import or trace vendor icons; never bundle vendor presets, Motion Graphics templates, animation presets or sample projects. The user's own files are fine.
- FFmpeg stays outside the bundle, run as a separate program when the person has it.
- Every new dependency, binary, model or data file gets a row in `NOTICE.md` before it ships. GPL stays outside the distributed bundle, AGPL nowhere, LGPL run as a separate program or dynamically linked and replaceable; model weights with gated or non-commercial terms stay optional downloads checked against a pinned SHA-256.
