# formvfx concepts for agents

The facts below were checked against the running engine. They are the things that most often decide whether a call works the first time.

## Addressing

- **Compositions**: the numeric id from `status` or a result (`{"comp": 158}`), or the name. Omit `comp` to act on the active composition. `comp.new` makes the new composition active; `comp.open {comp}` switches.
- **Layers**: the numeric id, `#n` for the n-th layer from the top (`#1` is the top layer), or the layer's name. Ids never change; `#n` changes when layers are added or reordered, so use ids inside long jobs.
- **Properties**: a path through the layer's tree. Each segment can be the match name (`masks/mask/feather`; a second one is `mask#2`), the 1-based index among its siblings (`masks/#2/feather`, `effects/#1/blurriness`) or the display name (`masks/Mask 2/feather`, `effects/Gaussian Blur/blurriness`). `get_layer` shows every path of a real layer; the property-paths reference shows the trees of fresh layers.
- **Selection**: commands whose layer parameter is optional (`layer?`, `layers?` in the summaries) act on the selected layers when it is omitted. Pass the layer explicitly; the selection belongs to the person.

## Time

- Tools and most commands take **composition seconds**. `time.set {time}` moves the current time indicator (the window follows).
- Raw keyframe commands take **layer seconds**: `prop.addKey`, `keys.set`, `keys.select`, `keys.setEase`, `keys.setSpatialTangents`. A layer whose start is 2 s has layer time 0 at composition time 2. `set_keyframes` converts and snaps to frames for you.
- Durations are seconds. Frame rates are frames per second; 29.97 is 30000/1001. `comp.new` without parameters gives 1920 x 1080, 29.97 fps, 10 s.

## Values

| Property type | Format | Notes |
| --- | --- | --- |
| scalar | number | opacity 0 to 100, rotation in degrees, percentages as 0 to 100 |
| vec2 / vec3 | `[x, y]` or `[x, y, z]` | transform properties are vec3 on every layer; for scale give all three (`[120, 120, 100]`), because a missing z becomes 0 |
| color | `"#rrggbb"` or `[r, g, b]` / `[r, g, b, a]` from 0 to 1 | reads back as `[r, g, b, a]` |
| enum (menus) | the 0-based option index | the effects catalogue lists the options in order |
| bool | `true` / `false` | |
| text | a string | `text/sourceText`; styling goes through `layer.setText` |
| path | vertices with tangents | set through `mask.*` and `shape.newPath`, not with plain numbers |

- **Positions** are composition pixels for transforms, from the top-left corner. A text layer's position is its baseline at the anchor (left, centre or right by its justification), so the glyphs sit above the point.
- **Effect point parameters** (centres, gradient start and end) are fractions of the layer: `[0, 0]` is the top-left, `[1, 1]` the bottom-right, `[0.5, 0.5]` the centre. Check the default in the effects catalogue.
- **Shape sizes** are pixels; a shape layer's contents are positioned relative to the layer's anchor.

## Results

| Command family | Typical result |
| --- | --- |
| `comp.new` | `{"comp": id}` |
| `layer.new*`, `layer.addItem` | `{"layer": id}` |
| `effect.apply` | `{"effect": "ec.blur.gaussian", "effects": [uid], "paths": ["effects/#1"]}` |
| `layer.addShapeItem` | `{"uid": n, "path": "contents/trim"}` |
| `layer.addTextAnimator` | `{"animator": uid}` |
| `prop.set` | the value now stored |
| `batch` | `{"steps": n, "results": [each step's result]}` |
| `import_files` | `{"items": [ids], "errors": [], "warnings": []}` |

Every tool result over MCP also carries `revision`, the project's change counter. A read returns the same revision; an edit raises it.

## Undo

- Each `execute`, `set_property`, `set_keyframes`, `apply_effect` and `batch` is one step in History. `history {action: undo}` goes back one; `history {action: list}` shows the steps.
- `batch` with `atomic: true` (the default) rolls everything back when a step fails, so a failed batch changed nothing.

## Errors

- A failed tool call says what was wrong and how to proceed: `no layer "Logo"`, `unknown effect "Gausian Blur"`, `"$1.layer" refers to step 1, but only 0 step(s) ran before`. `apply_effect` with a wrong parameter id still applies the effect and returns `paramErrors` with the list of `validParams`.
- Unknown parameter names in `execute` are ignored rather than rejected. If a call seemed to do nothing, compare the names with `describe_command`.
- `command ... is not available right now: no composition is open` means the command needs an active composition (or a selection): create or open one, or pass `comp` / `layer`.

## Permission

The first change an outside agent makes asks the person in the formvfx window (Allow, Allow Always, Don't Allow). Agents started from formvfx's Terminal panel are trusted and never ask. Reads never ask. If no window is open to ask, the error explains how to proceed; `session-and-connection` has the details.
