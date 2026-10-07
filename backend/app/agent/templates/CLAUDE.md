# formvfx agent workspace

You are running inside formvfx {{VERSION}}, an After Effects alternative, in its embedded terminal. formvfx is open on this computer and the person is watching its window: every change you make through formvfx's tools appears there live, as one undoable step, and the person can undo it with Cmd or Ctrl+Z.

## How to drive formvfx

- Use the `formvfx` MCP server (configured in `.mcp.json` here; it runs `{{MCP}}`). Agents started from this terminal are trusted; nothing asks for permission.
- If MCP is unavailable, the same surface is on the command line: `formvfx exec ID '{"json": "params"}'`, `formvfx batch '[...]'`, `formvfx commands FILTER`, `formvfx describe ID`, `formvfx state`, `formvfx frame out.png --time 1.5`.
- Read `.claude/skills/formvfx-harness/SKILL.md` before a non-trivial job. It routes to sub-skills for compositions, animation, effects, text and shapes, masks and roto, 3D, tracking, expressions, rendering and verifying.

## The loop that works

1. `status` to see the open project, the active composition and the selection.
2. Plan the composition, then build it with `batch` (one undo step; `"$1.layer"` is the layer the first step made, steps count from 1).
3. Look at the result with `render_frame` or `contact_sheet` after every meaningful change. Never claim a visual result you did not render and look at.
4. Fix, then render to a file with `render` when the person wants a file. Outputs go in `{{WORKSPACE}}/renders` unless they say otherwise.

## Facts that save time

- Every menu item, button and drag in formvfx is a command with an id (about 655). `list_commands` filters them; `describe_command` gives the JSON Schema.
- Layers are addressed by id, `#n` (1 is the top layer) or name; compositions by id or name. Times are seconds in composition time unless a tool says layer time.
- Properties are paths in the layer's tree: `transform/position`, `transform/scale`, `transform/opacity`, `effects/#1/blurriness`, `masks/#1/feather`, `text/sourceText`. `get_layer` lists them with values.
- Colours are `#rrggbb` or `[r, g, b]` from 0 to 1. Positions are composition pixels.

Projects: `{{WORKSPACE}}/projects` · renders: `{{WORKSPACE}}/renders` · formvfx data: `{{DATA}}`.
