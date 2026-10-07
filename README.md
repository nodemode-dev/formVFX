# formvfx

Motion graphics and visual effects for the desktop and the browser, in the form family beside formvis and formcut. Compositions, layers, keyframes and the Graph Editor, 306 effects, text animators, shape layers, masks, Roto Brush, paint, puppet pins, 3D cameras and lights, tracking and stabilizing, expressions, scripting, Essential Graphics and a Render Queue, with an agent harness so Claude Code, Codex and Cursor can drive every tool from formvfx's embedded terminal.

Version 0.1.0 · Apache-2.0 · compositing engine: formvfx's own, in Rust (`native/fvx-core`)

## Quick start

Needs Node 20.19+ or 22.12+, Python 3.12+, and Rust 1.95+ (`https://rustup.rs`). `./formvfx doctor` checks them.

```bash
./formvfx install      # engine venv, the Rust compositing engine (the first build takes a few minutes), interface packages
./formvfx              # boots the engine on 127.0.0.1:8230 and the interface on http://localhost:5230
```

Windows: `formvfx.bat install`, then `formvfx.bat`. One process serving the built interface: `./formvfx serve` (http://127.0.0.1:8230).

Open http://localhost:5230, choose **Open the demo project** on the start screen, and work as in After Effects: Project, Effect Controls, Composition viewer, Timeline, Effects and Presets, Preview, Character, Render Queue. Cmd or Ctrl+K searches every command and effect.

## Agents

- **In formvfx**: the Terminal panel (bottom right, or the Agent button) opens a shell in formvfx's agent workspace. Its Claude Code, Codex and Cursor buttons start an agent that already has the `formvfx` MCP server and the `formvfx-harness` skill; it edits the open project live, every change undoable, without permission prompts.
- **From this repository**: `.mcp.json`, `.codex/config.toml` and `.cursor/mcp.json` run `./formvfx mcp`. The first change asks once in the formvfx window.
- **From anywhere**: `./formvfx agents install` (or the Agents panel) sets the three clients up for your account.
- **Command line**: `./formvfx state`, `commands FILTER`, `describe ID`, `exec ID 'JSON'`, `batch 'JSON'`, `frame OUT.png --time S`, `render --wait`.

## Commands

| Command | Does |
| --- | --- |
| `./formvfx` / `boot` | engine and interface for development |
| `install`, `install-native` | everything, or only the Rust core (`--dev`, `--gpu`) |
| `serve` | the built interface from the engine |
| `doctor` | checks the toolchain and ports |
| `mcp` | the MCP server over stdio |
| `agents install|sync|status` | client setup; regenerate the skill references from the engine |
| `clean [--all]` | remove build output |

## Layout

`native/fvx-core` (the compositing engine, Rust, PyO3) · `backend` (FastAPI engine service, MCP, terminal) · `frontend` (TypeScript and Vite interface) · `setup` (scripts) · `.claude/skills/formvfx-harness` (the agent skill, mirrored to `.agents/skills`).

## Status

0.1.0 is the first build: the editor, the engine service and the agent substrate. The desktop app (Tauri), the frozen engine sidecar and the release workflows land next.
