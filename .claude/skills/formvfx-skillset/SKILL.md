---
name: formvfx-skillset
description: Use for any change to formvfx's own code, the motion graphics and visual effects compositor in the form family. Covers formvfx's Rust compositing engine in native/fvx-core (project model, commands, renderer, effects, tracking, formats, the PyO3 surface with UI state, overlays and frame jobs), the FastAPI engine service in backend/app (engine thread, bus, JSON-RPC, live socket, viewer frames, terminal), the agent substrate (MCP tools, consent, agent workspace, CLI, the harness skill generator), the TypeScript interface in frontend/src (store, dock, panels, Timeline, viewer, palette, formvis design files, zero-flash rules), the Tauri desktop shell and PyInstaller sidecar, CI and releases, and verification in a headless browser. Trigger on formvfx source files, a panel or tool to add, a bug in the Timeline or viewer, an MCP tool change, a desktop build, or ./formvfx scripts. Not for making motion graphics with formvfx (use formvfx-harness).
---

# formvfx-skillset

Route every change to formvfx's code through this file. It maps the subsystems, states the rules that hold everywhere, and names the specialist.

## Subsystem map

| Subsystem | Root | Specialist |
| --- | --- | --- |
| Compositing engine | `native/fvx-core/src/` | `sub-skills/engine-core/SUB-SKILL.md` |
| Engine service | `backend/app/engine/`, `backend/app/routers/`, `backend/app/core/`, `backend/formvfx_engine.py` | `sub-skills/engine-service/SUB-SKILL.md` |
| Agent substrate | `backend/app/agent/`, `setup/agents.py`, `setup/mcp_stdio.py`, `setup/fx.py`, `.claude/skills/formvfx-harness` | `sub-skills/agent-substrate/SUB-SKILL.md` |
| Interface | `frontend/src/` | `sub-skills/interface/SUB-SKILL.md` |
| Desktop and release | `src-tauri/`, `setup/freeze_backend`, `setup/desktop_*`, `.github/workflows/` | `sub-skills/desktop-and-release/SUB-SKILL.md` |
| Verification | a procedure | `sub-skills/verification/SUB-SKILL.md` |

## How the pieces depend

```text
fvx_core (native/fvx-core: the compositing engine, PyO3 extension with Engine and render jobs)
  -> engine thread (backend/app/engine/host.py), one Engine, not Send
    -> rpc.dispatch (one JSON-RPC surface)
      -> /api/live socket  -> interface store, panels
      -> /api/rpc HTTP     -> CLI (setup/fx.py), scripts
      -> /mcp              -> MCP tools (backend/app/agent/tools.py) -> stdio relay -> agents
    -> /api/viewer frames, /api/audio, /api/terminal
desktop shell (Tauri) -> frozen engine sidecar (serves the built interface)
```

## Rules that hold everywhere

- **Commands are the API.** Any new capability is an engine command first (a row in `native/fvx-core/src/engine/registry.rs`, or a method on the PyO3 `Engine` for state shaping); the interface, the MCP tools and the CLI then reach it through `rpc.dispatch`. Never give the interface logic the engine does not have.
- **One engine thread.** Only callables submitted through `host.submit` / `host.call` touch the `Engine`. Renders run on `FrameJob` / `AudioJob` / `ThumbJob` snapshots off that thread.
- **Zero flash.** The interface mounts once and patches keyed nodes (`patchList`, `setText`, `setClass`); a panel never rebuilds its container to change one value. Every colour is a token, every z-index a `var(--z-*)`, every icon an inline `currentColor` SVG.
- **Keep the agents in step.** After changing an MCP tool, a guide text or anything that changes command or effect lists, run `./formvfx agents sync` and commit the regenerated references and the `.agents/skills` mirror; CI runs `sync --check`.
- **Text hygiene.** No pictographs anywhere and no em dashes in Markdown: `python3 setup/lint_text.py .` must report 0.
- **Logs over comments.** Code carries near-zero comments; reasoning goes in `frontend/HASHOUT_LOG.md`, `backend/HASHOUT_LOG.md` and `native/fvx-core/HASHOUT_LOG.md`.
- **Versions together.** `frontend/package.json` (and both fields in its lockfile), `frontend/src/version.ts`, `backend/app/__init__.py`, `native/fvx-core/Cargo.toml`, its `Cargo.lock`, `native/fvx-core/pyproject.toml`, `src-tauri/Cargo.toml`, its `Cargo.lock`, `src-tauri/tauri.conf.json` and the README version line always carry the same `v*.*.*`.

## Run it

```bash
./formvfx install        # venv, the compositing engine (release), interface packages
./formvfx                # engine 8250 + interface 5250
./formvfx install-native --dev   # quick engine rebuild after Rust changes
./formvfx serve          # the built interface from the engine
./formvfx desktop dev    # the Tauri shell with the frozen sidecar
```
