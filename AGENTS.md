# formvfx

A motion graphics and visual effects compositor: vanilla TypeScript and Vite interface, Python FastAPI engine around formvfx's own Rust compositing engine, Tauri 2 desktop shell.

## Making motion graphics (natural language)

When the person wants a composition, animation, title, effect, 3D scene, track or render made in formvfx, use the `formvfx` MCP server's tools guided by the `formvfx-harness` skill in `.agents/skills/formvfx-harness` (its `references/tools.md` lists every tool; `references/commands.md` every command). Start with `status`, build with `batch`, check every visual change with `render_frame` or `contact_sheet`.

The MCP server is configured here in `.codex/config.toml` (Codex) and `.cursor/mcp.json` (Cursor); both run `./formvfx mcp`, which starts the engine when it is not running. From any other folder, `./formvfx agents install` sets Claude Code, Codex and Cursor up for the user account. Without MCP, the `./formvfx` command line has the same surface: `state`, `commands`, `describe`, `exec`, `batch`, `frame`, `render`.

## Changing formvfx's code

Roto Paint propagation and Selection-Aware-Fill (declined for patent risk, 2026-10-23): propagation stays global (whole-frame flow or SAM 2), with no per-window search regions along the contour sized by local motion; the fill adds no per-frame confidence scores, no prompts to fix or supply a specific frame, and no forward and backward flow weighting of boundary motion. See `CLAUDE.md` and "Declined" in `docs/PARITY.md`.

Read `CLAUDE.md` and `.claude/skills/formvfx-skillset/SKILL.md`, then `frontend/HASHOUT_LOG.md` and `backend/HASHOUT_LOG.md` and `native/fvx-core/HASHOUT_LOG.md` (the engine). After changing MCP tools or engine commands, run `./formvfx agents sync`.
