# formvfx MCP tools

Generated from the formvfx engine by `./formvfx agents sync`; do not edit by hand.

The `formvfx` MCP server has 26 tools. `./formvfx mcp` serves them over stdio (it starts the engine when it is not running); the engine also serves them over Streamable HTTP at `http://127.0.0.1:8230/mcp`. The shell equivalents are in the last section.

## Contents

`status` `list_commands` `describe_command` `execute` `batch` `get_comp` `get_layer` `get_property` `set_property` `set_keyframes` `apply_effect` `list_effects` `text_preset` `shapes_from_svg` `planar_track` `object_matte` `render_frame` `contact_sheet` `render` `project` `import_files` `history` `run_script` `ui` `wait` `guide`

## status

Start here. The open project (path, unsaved changes), the active composition (id, name, size, frame rate, duration, layer count, current time), the selection, background jobs, how many formvfx windows are watching, and the command and effect counts.

- no parameters

Note: read-only.

## list_commands

formvfx's commands (every menu item, button and drag is one): id, label, menu path, parameter summary, enabled now. Filter by text or namespace (layer, comp, keys, effect, mask, text, shape, camera, track, renderQueue, view, file...).

- `filter`: string
- `namespace`: string
- `enabledOnly`: boolean; default false
- `limit`: integer; default 200

Note: read-only.

## describe_command

One command in full: label, menu, parameter documentation and JSON Schema, whether it is undoable and enabled now.

- `command`: string; required

Note: read-only.

## execute

Runs one formvfx command by id with its parameters (see describe_command) and returns its result. Undoable commands are one step in the person's History.

- `command`: string; required
- `params`: object; The command's parameters.

## batch

Runs several commands as one undo step. Each step is {command, params}; a string "$N.key" in a later step's params is replaced by step N's result field, counting steps from 1, e.g. {"layer": "$1.layer"} is the layer the first step made. atomic (default true) undoes everything if one step fails.

- `steps`: array; required
- `label`: string
- `atomic`: boolean; default true

## get_comp

A composition: size, frame rate, duration, work area, background, and its layers top to bottom (id, index, name, type, in and out points, start, stretch, switches, blend mode, parent, track matte). With flowchart, also the Flowchart graph: nodes (comps, footage, solids, layers, effects with their depth) and edges from each comp or layer to what it uses, nested comps followed.

- `comp`: id or name; Composition id or name; omit for the active composition.
- `flowchart`: boolean; default false

Note: read-only.

## get_layer

A layer's property tree: every property with its path (use it in set_property and set_keyframes), value at the time, keyframes and expression; masks, effects and text included.

- `layer`: id or name; required; Layer id, `#n` (1 = top) or name.
- `comp`: id or name; Composition id or name; omit for the active composition.
- `depth`: integer; default 16
- `time`: number; Composition time in seconds; omit for the current time.

Note: read-only.

## get_property

One property's value (at a time) with its keyframes and expression.

- `layer`: id or name; required; Layer id, `#n` (1 = top) or name.
- `path`: string; required
- `comp`: id or name; Composition id or name; omit for the active composition.
- `time`: number; Composition time in seconds; omit for the current time.

Note: read-only.

## set_property

Sets a property's value. On an animated property (it has keyframes) the value is keyed at time (default: the current time). Values: number, [x, y] or [x, y, z] (give scale all three, [sx, sy, 100], or z scale becomes 0), #rrggbb or [r, g, b(, a)] from 0 to 1 for colours, true or false, and the 0-based option index for menus (list_effects with detail shows the options in order).

- `layer`: id or name; required; Layer id, `#n` (1 = top) or name.
- `path`: string; required; e.g. transform/position, transform/opacity, effects/#1/blurriness, masks/#1/feather.
- `value`: any; required
- `comp`: id or name; Composition id or name; omit for the active composition.
- `time`: number; Composition time in seconds; omit for the current time.

## set_keyframes

Animates a property: one keyframe per entry at composition time (seconds) with an optional ease (linear, easyEase, easyEaseIn, easyEaseOut, hold). One undo step.

- `layer`: id or name; required; Layer id, `#n` (1 = top) or name.
- `path`: string; required
- `comp`: id or name; Composition id or name; omit for the active composition.
- `keys`: array; required

## apply_effect

Adds an effect (id like ec.blur.gaussian or its After Effects name like Gaussian Blur) to a layer and sets its parameters by id ({"blurriness": 20}). Returns the effect's index for paths effects/#n/....

- `layer`: id or name; required; Layer id, `#n` (1 = top) or name.
- `effect`: string; required
- `params`: object
- `comp`: id or name; Composition id or name; omit for the active composition.

## list_effects

The effects (307) by category with ids; with detail, their parameters (id, name, default, control). Filter by name, id or category.

- `query`: string
- `category`: string
- `detail`: boolean; default false
- `limit`: integer; default 60

Note: read-only.

## text_preset

formvfx's text animation preset library (169 presets in 13 categories: Animate In, Animate Out, Blurs, Fill and Stroke, Lights and Optical, Mechanical, Multi-Line, Organic, Rotation, Scale, Tracking, 3D Text, Miscellaneous). list (filter by query or category) returns id, name, category, description; apply adds the preset's text animators to text layers starting at time (default the current time), one undo step. Reveal presets run about 1.5 s from that time; continuous ones loop.

- `action`: string (list | apply); required
- `preset`: string
- `query`: string
- `category`: string
- `layer`: id or name; Layer id, `#n` (1 = top) or name.
- `layers`: array
- `time`: number; Composition time in seconds; omit for the current time.
- `comp`: id or name; Composition id or name; omit for the active composition.

## shapes_from_svg

Turns SVG markup into a shape layer (paths, fills, strokes, gradients, groups and clips become editable shape contents), centred in the composition, one undo step. Use it for logos, icons and vector art you write as SVG.

- `svg`: string; required
- `name`: string
- `comp`: id or name; Composition id or name; omit for the active composition.

## planar_track

Planar tracking: follows a flat surface (a screen, a sign, a wall, a tabletop) through footage and corner-pins another layer onto it. Draw a mask around the surface on the footage layer first (mask.new or layer.addMask, in layer pixels at the reference time). track forward, backward or both analyses the mask with the Perspective method from the reference time (a homography from features inside the mask, every frame); without track the mask must already be tracked. surface is the plane's four corners at the reference time (upper left, upper right, lower left, lower right, layer pixels; default the mask's four vertices or its bounding box). With target, a Corner Pin is keyed on that layer so its corners ride the surface; the result is also kept as a Perspective tracker on the footage for re-applying to other layers. One undo step for the corner pin (tracking is its own step).

- `layer`: id or name; required; Layer id, `#n` (1 = top) or name.
- `mask`: id or name; Mask uid, 1-based index or name (default the first mask).
- `target`: id or name; Layer id, `#n` (1 = top) or name.
- `surface`: array
- `reference`: number; Composition time in seconds; omit for the current time.
- `track`: string (forward | backward | both)
- `start`: number
- `end`: number
- `comp`: id or name; Composition id or name; omit for the active composition.

## object_matte

Object Matte: isolates one object (a person, a car, a product) in footage from one point on it, the way After Effects' Object Matte does. action preview segments the object at time and returns its bounding box, coverage and the model's confidence without changing anything; action select puts a Roto Brush on the layer seeded at that point (one undo step), spans the whole layer and propagates it (direction both by default; wait true blocks until done, otherwise call wait). The layer's alpha becomes the object's matte. Needs the MobileSAM model (roto.model.download {id: mobilesam}).

- `action`: string (preview | select); required
- `layer`: id or name; required; Layer id, `#n` (1 = top) or name.
- `point`: array; required; [x, y] in the layer's pixels
- `time`: number; Composition time in seconds; omit for the current time.
- `direction`: string (both | forward | backward)
- `wait`: boolean; default false
- `comp`: id or name; Composition id or name; omit for the active composition.

## render_frame

Renders one frame of a composition and returns it as an image so you can see it. Use it after changes and at the moments that matter. For a 3D comp, view renders from a 3D view (top, front, left, right, back, bottom, default, custom1-3) instead of the active camera, to check layout in space. backend auto (default) renders on the GPU or the CPU, whichever is faster for the comp; gpu or cpu forces one (status shows the GPU).

- `comp`: id or name; Composition id or name; omit for the active composition.
- `time`: number; Composition time in seconds; omit for the current time.
- `max_size`: integer; default 960
- `transparent`: boolean; default false
- `view`: string (activeCamera | default | front | left | top | back | right | bottom | custom1 | custom2 | custom3)
- `backend`: string (auto | gpu | cpu)

Note: read-only.

## contact_sheet

Renders several frames across a time span into one image grid (each labelled with its time) to check motion and timing in one look.

- `comp`: id or name; Composition id or name; omit for the active composition.
- `start`: number
- `end`: number
- `count`: integer; default 8
- `max_size`: integer; default 320

Note: read-only.

## render

Renders a composition to a file through the Render Queue and waits for it: format h264, prores, hevc, av1, webm, gif, png, jpeg, tiff, exr, wav or aiff; output is an absolute path (default: the renders folder).

- `comp`: id or name; Composition id or name; omit for the active composition.
- `format`: string; default "h264"
- `output`: string
- `wait`: boolean; default true

## project

Project files: new, open (path), save (path for the first save), saveAs (path), close. formvfx projects are .ecproj JSON.

- `action`: string (new | open | save | saveAs | close | demo); required
- `path`: string

Note: replaces or removes.

## import_files

Imports footage, images, audio, PSD, SVG, PDF, Lottie or 3D models (absolute paths) into the project; addToComp also places them in the active composition.

- `paths`: array; required
- `addToComp`: boolean; default false

## history

Undo, redo, list the history, or go to a step.

- `action`: string (undo | redo | list | goto); required
- `steps`: integer; default 1
- `index`: integer

## run_script

Runs an ExtendScript-style JavaScript program against the project (app.project, CompItem, layers, properties, as in After Effects scripting). Returns its result and console output.

- `code`: string; required

## ui

The formvfx window: state (what the person sees: panels, tool, zoom, workspace), or show a panel, switch workspace, choose a tool, zoom the viewer.

- `action`: string (state | panel | workspace | tool | zoom); required
- `panel`: string
- `name`: string
- `tool`: string
- `value`: any

## wait

Waits until background work (rendering, tracking, stabilizing, Roto Brush, Content-Aware Fill) has finished, up to timeout seconds.

- `timeout`: number; default 120

Note: read-only.

## guide

formvfx know-how for agents: workflow, layers, animation, text, shapes, effects, masks, 3d, expressions, tracking, render, ui.

- `topic`: string

Note: read-only.

## Guide topics

The `guide` tool returns these texts.

### layers

Create: layer.newSolid {name, color, width, height}, layer.newText {text, size, fill}, layer.newShape {kind, size:[w,h], position:[x,y], fill, stroke, strokeWidth}, layer.newNull, layer.newAdjustment, layer.newCamera, layer.newLight {kind: point|spot|parallel|ambient}. Footage: import_files then layer.addItem {item, time, position}. Arrange: layer.arrange {layers, to: front|forward|backward|back, above}. Time: layer.timing {layers, in, out, start, delta}; layer.sequence {layers, overlap, duration, transition}. Parent: layer.setParent {layers, parent}. Switches: layer.setSwitch {layers, switch: video|audio|solo|lock|shy|collapse|fx|motionBlur|adjustment|threeD, value}. Blend: layer.setBlendMode {layers, mode}. Matte: layer.setTrackMatte {layer, matte, kind: alpha|alphaInverted|luma|lumaInverted}. Precompose: layer.precompose {layers, name}.

### animation

set_keyframes converts composition seconds to layer time for you. Raw commands take LAYER seconds: prop.addKey {layer, path, time, value} adds a key, keys.set {layer, path, time, newTime?, value?} edits an existing one, keys.select {keys: [{layer, path, time}]} then keys.easyEase {which}, keys.interpolation {interpolation: linear|bezier|continuousBezier|autoBezier|hold}, keys.velocity, keys.move {delta}, keys.delete. Motion along a path: keyframe transform/position; layer.autoOrient {mode: alongPath}. Sequence layers: layer.sequence {layers, overlap, duration}. Wiggle keys: keys.wiggle. Time remap: layer.enableTimeRemap; speed: layer.timeStretch {percent}.

### text

layer.newText {text, font, size, fill, justify, position}; edit with layer.setText {layer, text?, font?, size?, fill?, stroke?, strokeWidth?, tracking?, leading?, justify?, box?}. Fonts: execute text.fonts {query}. Animators: layer.addTextAnimator {layer, properties: [position|scale|rotation|opacity|fillColor|tracking|blur...]} then key its range selector (get_layer shows the paths). Presets: layer.applyTextPreset {layer, preset: typewriter|fadeUpCharacters|bounceInWords|trackingIn|scramble|blurIn|jitter|dropInLines}.

### shapes

layer.newShape {kind: rect|rounded|ellipse|polygon|star, size, position, fill, stroke, strokeWidth}. Free paths: shape.newPath {vertices, inTangents, outTangents, closed, fill, stroke}. Add items and operators with layer.addShapeItem {layer, kind: group|rect|ellipse|star|polygon|fill|stroke|gfill|trim|repeater|round|offset|pucker|twist|zigzag|wiggle|merge, group?} (returns {uid, path}). Trim Paths end keyed 0 to 100 is a write-on.

### effects

list_effects {query} finds effects (307: blur, color correction, distort, generate, keying, matte, noise, perspective, simulation, stylize, time, transition, audio...). apply_effect {layer, effect, params: {paramId: value}} adds and sets one; get_layer shows effects/#n/... paths. Common ids: ec.blur.gaussian (Gaussian Blur: blurriness), ec.stylize.glow (Glow), ec.generate.fill (Fill: color), ec.color.curves, ec.keying.keylight. Layer styles: execute layer.style.add {layers, style: dropShadow|outerGlow|stroke|bevelEmboss...}.

### masks

layer.addMask {layer, shape: rect|ellipse, rect: [x, y, w, h] in layer pixels}; mask.new {layer, vertices: [[x,y]...]} for a polygon; mask properties: masks/#1/feather, masks/#1/opacity, masks/#1/expansion (or layer.mask.set {layer, mask, field, value}); mode with layer.mask.mode {layer, mask, mode: None|Add|Subtract|Intersect|Lighten|Darken|Difference}, invert with layer.mask.invert. Roto Brush: roto.stroke {layer, kind: fg|bg, points (layer px)} then roto.propagate {direction, wait: true}.

### 3d

layer.setSwitch {layers, switch: threeD, value: true} makes a layer 3D (transform/position becomes [x,y,z]; transform/orientation, xRotation, yRotation, zRotation). layer.newCamera {preset or zoom}, layer.newLight {kind, color, intensity}. Primitives: layer.new3dPrimitive {kind: cube|sphere|plane|torus|cone|cylinder}; models: layer.newModel {path}. Views: execute view.set3DView {view: activeCamera|front|top|left|custom1}. Depth of field: layer.cameraSettings {dof, focusDistance, aperture}.

### expressions

prop.setExpression {layer, path, expression}. The language is JavaScript with the After Effects object model: time, value, wiggle(freq, amp), loopOut('cycle'), linear(t, a, b, c, d), ease(), thisComp.layer('Name').transform.position, effect('Slider Control')('Slider'). Errors: execute expr.errors.

### tracking

Point track: track.new {layer, kind: transform|stabilize|affine|perspective, target?} then track.analyze {layer, direction: forward, wait: true}, then track.apply {layer}. Stabilize: track.warpStabilizer {layer, wait: true}. 3D camera: track.camera {layer, wait: true}, then camera.createFromSolve {kind: text|solid|null|shadowCatcher|camera}. Masks: track.mask {layer, mask, method}. Planar surface (a screen, a sign): mask around it, then planar_track {layer, track: both, target}. Without wait, use the wait tool.

### render

render {comp, format: h264|prores|hevc|av1|webm|gif|png|jpeg|tiff|exr|wav, output: absolute path, wait: true}. Or renderQueue.add {comp, format, output} then renderQueue.render {wait: true}. Single frame: execute comp.saveFrameAs {path, time}. Lottie: execute file.exportLottie {comp, path}.

### ui

ui {action: state} reports what the person sees (panels, tool, zoom, workspace). ui {action: panel, panel: timeline|effectControls|project|composition|character|renderQueue|terminal...} shows a panel; ui {action: workspace, name}; ui {action: tool, tool: selection|pen|type|rectangle...}; ui {action: zoom, value: fit|100|200}. Selecting layers (layer.select) and moving the time (time.set {time}) also show in the window.

## The same surface from a shell

- `formvfx state` · the open project, active composition, selection and time (like `status`)
- `formvfx commands FILTER` · list commands (like `list_commands`)
- `formvfx describe ID` · one command with its schema (like `describe_command`)
- `formvfx exec ID 'JSON'` · run one command (like `execute`)
- `formvfx batch '[{"command": ID, "params": {}}]'` · several as one undo step (like `batch`)
- `formvfx frame OUT.png --time S --comp C --max N` · save a rendered frame (like `render_frame`)
- `formvfx render --wait` · render the Render Queue
