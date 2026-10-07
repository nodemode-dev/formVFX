# formvfx

Motion graphics and visual effects (an After Effects alternative): vanilla TypeScript and Vite interface, Python FastAPI engine around a Rust compositing core, Tauri 2 desktop shell.

## Making motion graphics (natural language)

When the person wants a composition, animation, title, effect, 3D scene, track or render made in formvfx, use the `formvfx` MCP server's tools guided by the `formvfx-harness` skill in `.agents/skills/formvfx-harness` (its `references/tools.md` lists every tool; `references/commands.md` every command). Start with `status`, build with `batch`, check every visual change with `render_frame` or `contact_sheet`.

The MCP server is configured here in `.codex/config.toml` (Codex) and `.cursor/mcp.json` (Cursor); both run `./formvfx mcp`, which starts the engine when it is not running. From any other folder, `./formvfx agents install` sets Claude Code, Codex and Cursor up for the user account. Without MCP, the `./formvfx` command line has the same surface: `state`, `commands`, `describe`, `exec`, `batch`, `frame`, `render`.

## Changing formvfx's code

Read `CLAUDE.md`, then `frontend/HASHOUT_LOG.md` and `backend/HASHOUT_LOG.md`. After changing MCP tools or engine commands, run `./formvfx agents sync`.
