# formvfx

Motion graphics and visual effects for the desktop and the browser, in the form family beside formvis and formcut. Compositions, layers, keyframes and the Graph Editor, 307 effects, text animators with 177 presets, shape layers and SVG paste, masks, Roto Brush, paint, puppet pins, 3D cameras and lights, tracking and stabilizing, expressions, scripting, Essential Graphics and a Render Queue, with an agent harness so Claude Code, Codex and Cursor can drive every tool from formvfx's embedded terminal.

Version 0.11.0 · Apache-2.0 · compositing engine: formvfx's own, in Rust (`native/fvx-core`)

## Quick start

Needs Node 20.19+ or 22.12+, Python 3.12+, and Rust 1.95+ (`https://rustup.rs`). `./formvfx doctor` checks them.

```bash
./formvfx install      # engine venv, the Rust compositing engine (the first build takes a few minutes), interface packages
./formvfx              # boots the engine on 127.0.0.1:8230 and the interface on http://localhost:5230
```

The GPU compositor is part of the default build and is used when the engine finds a working adapter (Settings > Memory & CPU shows which; `FORMVFX_GPU=0` keeps everything on the CPU). Windows: `formvfx.bat install`, then `formvfx.bat`. One process serving the built interface: `./formvfx serve` (http://127.0.0.1:8230).

Open http://localhost:5230, choose **Open the demo project** on the start screen, and work as in After Effects: Project, Effect Controls, Composition viewer, Timeline, Effects and Presets, Preview, Character, Render Queue. Cmd or Ctrl+K searches every command and effect.

## Agents

- **In formvfx**: the Terminal panel (bottom right, or the Agent button) opens a shell in formvfx's agent workspace. Its Claude Code, Codex and Cursor buttons start an agent that already has the `formvfx` MCP server and the `formvfx-harness` skill; it edits the open project live, every change undoable, without permission prompts.
- **From this repository**: `.mcp.json`, `.codex/config.toml` and `.cursor/mcp.json` run `./formvfx mcp`. The first change asks once in the formvfx window.
- **From anywhere**: `./formvfx agents install` (or the Agents panel) sets the three clients up for your account.
- **Command line**: `./formvfx state`, `commands FILTER`, `describe ID`, `exec ID 'JSON'`, `batch 'JSON'`, `frame OUT.png --time S`, `render --wait`.

## Desktop

```bash
./formvfx install-desktop   # PyInstaller and the Tauri CLI (Linux also needs the webkit2gtk-4.1 packages)
./formvfx desktop dev       # the app, with the engine frozen as its sidecar
./formvfx desktop build     # DMG, AppImage, deb, MSI and setup bundles in src-tauri/target/release/bundle
```

Tagging `vX.Y.Z` builds macOS, Linux and Windows bundles in GitHub Actions and opens a draft release.

## Commands

| Command | Does |
| --- | --- |
| `./formvfx` / `boot` | engine and interface for development |
| `install`, `install-native` | everything, or only the Rust engine (`--dev`; `--cpu` leaves out the GPU compositor) |
| `serve` | the built interface from the engine |
| `install-desktop`, `freeze`, `desktop dev`, `desktop build` | the desktop app |
| `doctor` | checks the toolchain and ports |
| `mcp` | the MCP server over stdio |
| `agents install|sync|status` | client setup; regenerate the skill references from the engine |
| `counts [--check]` | the code size table below |
| `presets [--check]` | regenerate the text animation preset library (`presets/text-animators.json`) |
| `clean [--all]` | remove build output |

## Layout

`native/fvx-core` (the compositing engine, Rust, PyO3) · `backend` (FastAPI engine service, MCP, terminal) · `frontend` (TypeScript and Vite interface) · `setup` (scripts) · `presets` (the text animation preset library) · `.claude/skills/formvfx-harness` (the agent skill, mirrored to `.agents/skills`).

## Parity

[`docs/PARITY.md`](docs/PARITY.md) grades formvfx against After Effects 26.5 on 72 traced capability rows: 64 Full, 6 Partial, 2 None, a score of 93%. It lists where formvfx trails, in priority order, and where it leads. The wiki (`.docs/wiki`) covers getting started, agents, the architecture, the desktop app and releasing.

## Code size

| Area | Files | Lines |
| --- | ---: | ---: |
| Compositing core (Rust, PyO3) | 142 | 69,371 |
| Engine service (Python) | 33 | 2,521 |
| Interface (TypeScript) | 44 | 10,437 |
| Interface styles (CSS) | 8 | 1,780 |
| Desktop shell (Rust, Tauri) | 1 | 138 |
| Setup and CLI | 35 | 1,095 |
| Agent skill (Markdown, hand-written) | 16 | 737 |
| Total | 279 | 86,079 |

Non-blank lines in tracked files; generated skill references excluded. `./formvfx counts` prints this table.
