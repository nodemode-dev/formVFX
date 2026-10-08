---
name: engine-service
description: The FastAPI engine service in backend/app. Use for the engine thread and its tick, the event bus and changed events, the JSON-RPC dispatcher, the live socket, binary viewer frames and range streaming, audio, file browsing, the PTY terminal, local-only security, discovery, configuration, ports, and the PyInstaller entry formvfx_engine.py.
---

# engine-service

## Map

| Route or module | File | Purpose |
| --- | --- | --- |
| Engine thread | `app/engine/host.py` | one `Engine`, a queue of callables, 100 ms tick (events, jobs, autosave), frame and job pools, `changed` publishing with `edited` |
| Bus | `app/engine/bus.py` | windows join and leave, `publish`, `ask_window` / `answer` for consent and UI requests |
| JSON-RPC | `app/engine/rpc.py` | `command.execute`, `command.batch`, `command.list`, `command.describe`, `ui.state`, `ui.overlays`, `ui.graph`, `ui.flowchart`, `engine.performance` (multi-frame rendering and the GPU: `host.attach_gpu`, the probe in `app/engine/performance.py`), `text.library`, `text.applyLibraryPreset`, `clipboard.pasteSvg`, `track.planar` (these through `_native_edit`), `roto.objectPreview` (a job on `host.jobs`), `roto.objectMatte` (stroke and span in one batch, then propagate; the library is `app/engine/text_presets.py`), `menu.bar`, `menu.titles`, `effects.catalog`, `gesture.begin`, `gesture.end`, `frame.png`, `engine.info`, `activity.list`, `project.json`, `state.touch` |
| `GET /api/health`, `/api/version`, `/api/system` | `app/routers/system.py` | liveness, version, system and capability report |
| `POST /api/rpc` | `app/routers/rpc.py` | JSON-RPC over HTTP (notifications get 204) |
| `WS /api/live` | `app/routers/live.py` | the interface's socket: calls, replies, server events |
| `WS /api/viewer` | `app/routers/viewer.py` | binary frames: header `<IIIIdI` (id, width, height, revision, time, kind) then RGBA; latest wins; range requests stream RAM previews |
| `/api/frame.png`, `/api/thumb/[item]`, `/api/audio`, `/api/peaks/[item]`, `/api/snapshot.png` (preview ranges render `host.preview_frames` at once, see `app/engine/performance.py`) | `app/routers/viewer.py` | single frames, project thumbnails, WAV for previews, waveform peaks (f32 quadruples min L, max L, min R, max R; `x-bins-per-sec`; cached by footage path or comp revision) |
| `/api/fs/roots`, `list`, `mkdir`, `upload`, `file` | `app/routers/files.py` | the in-app file browser and drops |
| `/api/link/info`, `/api/link/frame.png`, `/api/link/audio` | `app/routers/link.py`, `app/engine/link.py` | Dynamic Link: a comp of the open project (live) or of any saved `.ecproj` (side engine, reopened on mtime change) for another app; the `x-formvfx-local: 1` header on GET too; `link.export` writes the `.fvxlink` descriptor |
| `/api/terminal/status`, `WS /api/terminal/ws` | `app/routers/terminal.py` | PTY (pty.fork, pywinpty on Windows) in the agent workspace with the trust token in its environment |
| `/mcp`, `/api/agents/*` | `app/routers/mcp.py`, `app/routers/agents.py` | see `agent-substrate` |
| Security | `app/core/security.py` | `LocalOnly`: loopback clients and local origins only, `x-formvfx-local: 1` for non-browser callers |
| Config | `app/core/config.py` | ports (8230), data dir `~/Formvfx` or `FORMVFX_DATA_DIR`, `FORMVFX_DIST`, `FORMVFX_AGENTS`, frozen bundle paths |
| Discovery | `app/core/discovery.py` | `engine.json` so the CLI and the stdio relay find a running engine |
| Entry | `formvfx_engine.py` | `--selfcheck`, `--headless`, `--cli`, `--mcp`; binds a socket (port 0 in the desktop app) and prints `Uvicorn running on http://127.0.0.1:N` for the shell |

## Rules

- Never call the `Engine` outside `host.submit` / `host.call`.
- Commands that do not bump the revision but change what windows show must be submitted with `touches=True` (rpc does this for non-query commands) so a `changed` event with `edited: false` goes out.
- New HTTP routes go through `LocalOnly`; nothing binds anything but 127.0.0.1.
- `FORMVFX_OPEN` (set by the desktop shell for a double-clicked `.ecproj`) opens a project at start.

## Run

`./formvfx backend` (uvicorn with reload of `app/`), or `FORMVFX_DIST=frontend/dist ./formvfx backend` to serve the built interface too.
