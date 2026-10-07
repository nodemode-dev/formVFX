# formvfx

Motion graphics and visual effects (an After Effects alternative) in the form family beside formvis and formcut: a vanilla TypeScript and Vite interface, a Python FastAPI engine service around formvfx's own Rust compositing engine (`fvx_core`, wrapped with PyO3), and a Tauri 2 desktop shell.

## Two kinds of work

**Making motion graphics with formvfx** (a title, an animation, an effect, a render): use the `formvfx` MCP server (`.mcp.json` runs `./formvfx mcp`, which starts the engine when needed) with the `formvfx-harness` skill in `.claude/skills/formvfx-harness`. Start with `status`, build with `batch`, look with `render_frame` and `contact_sheet`. The first change asks the person once in the formvfx window; agents started from formvfx's Terminal panel never ask.

**Changing formvfx's code**: read the living logs `frontend/HASHOUT_LOG.md` and `backend/HASHOUT_LOG.md` before editing.

## Layout

- `native/fvx-core/` · the compositing engine (`fvx_core`), written for formvfx in Rust: project model, commands, undo, renderer, effects, gestures, events, UI state, overlays, graph samples and frame, audio and thumbnail jobs, exposed to Python with PyO3. `.fvxproj` is the project file. Build with `./formvfx install-native`; read `native/fvx-core/HASHOUT_LOG.md` before changing it.
- `backend/app/` · the engine service: `engine/` (the engine thread, the event bus, JSON-RPC), `routers/` (live socket, viewer frames, files, terminal, MCP, agents), `agent/` (MCP tools, consent, workspace, CLI).
- `frontend/src/` · the interface: `engine/` (client, store, frames, playback; no DOM), `ui/` (dock, menus, palette, widgets, formvis design files), `panels/`, `app/`.
- `setup/` · install, boot, build, freeze and desktop scripts behind the `./formvfx` dispatcher; `setup/agents.py` regenerates the harness skill's references from the engine.
- Planning documents (DEV_PLAN, BUILD_NOTES, TEST_PLAN, the root HASHOUT_LOG) ship in the planning sidecar zip, not in this tree.

## After changing the MCP tools or the engine's commands

Run `./formvfx agents sync` so `.claude/skills/formvfx-harness/references/` and the `.agents/skills` mirror follow; `./formvfx agents sync --check` fails while they drift.
