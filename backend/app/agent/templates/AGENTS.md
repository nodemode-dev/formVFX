# formvfx agent workspace

This folder is formvfx {{VERSION}}'s agent workspace. formvfx (a motion graphics and visual effects compositor) is running on this computer and the person sees its window; your edits appear there live and are undoable.

- Drive formvfx with the `formvfx` MCP server (`.codex/config.toml` and `.cursor/mcp.json` here run `{{MCP}}`), or with the `formvfx` command line (`formvfx exec ID JSON`, `formvfx batch JSON`, `formvfx commands FILTER`, `formvfx describe ID`, `formvfx state`, `formvfx frame OUT.png`).
- Read `.agents/skills/formvfx-harness/SKILL.md` first: it routes to the sub-skill for the job (compositions, animation, effects, text, shapes, masks, 3D, tracking, expressions, rendering, verification).
- Loop: `status`, then build with `batch`, then look with `render_frame` or `contact_sheet`, then fix. Never report a visual result you did not render.
- Layers by id, `#n` or name; times in composition seconds; property paths such as `transform/position` or `effects/#1/blurriness`.

Projects go in `{{WORKSPACE}}/projects`, renders in `{{WORKSPACE}}/renders`.
