from __future__ import annotations

import asyncio
import base64
import json
import struct
import zlib
from pathlib import Path
from typing import Any

from .. import __version__
from ..engine import rpc
from ..engine.host import host
from ..engine.rpc import RpcError

INSTRUCTIONS = (
    "formvfx is a motion graphics and visual effects compositor (an After Effects alternative) running on this computer; the person sees "
    "its window and your edits appear there live, each one undoable. Start with `status`. Build with `batch` (several commands, one undo "
    "step, `$N.key` refers to step N's result, counting from 1). Every formvfx action is a command with an id: `list_commands` and `describe_command` find "
    "them, `execute` runs one. Look at your work with `render_frame` or `contact_sheet` after each meaningful change; never claim a visual "
    "result you did not render. Layers: id, `#n` (1 = top) or name. Times: composition seconds. Properties: paths such as "
    "`transform/position` or `effects/#1/blurriness` (`get_layer` lists them). The `guide` tool has recipes; get the `workflow` prompt "
    "for the full loop."
)

WORKFLOW = """How to build in formvfx with these tools.

1. Orient: status (project, active composition, selection, time). get_comp lists the layers of a composition with ids, timing and switches.
2. Compose: execute comp.new {name, width, height, frameRate, duration (s), background}. New layers: layer.newSolid, layer.newText, layer.newShape {kind: rect|rounded|ellipse|polygon|star}, layer.newNull, layer.newCamera, layer.newLight, layer.newAdjustment; footage: import_files then layer.addItem {item}. Do several in one batch.
3. Place and style: set_property for transform/position, transform/scale, transform/rotation, transform/opacity, transform/anchor; text with layer.setText {layer, text, font, size, fill, justify}; effects with apply_effect {layer, effect, params}.
4. Animate: set_keyframes {layer, path, keys: [{time, value, ease}]}; ease is linear, easyEase, easyEaseIn, easyEaseOut or hold. Expressions: execute prop.setExpression {layer, path, expression} (wiggle(2, 30), loopOut(), time*90).
5. Look: render_frame at the moments that matter; contact_sheet shows N frames in one image to check motion. Fix what is wrong before going on.
6. Deliver: project {action: save, path} to keep it editable (.fvxproj), render to write a movie or image sequence (h264, prores, hevc, av1, webm, gif, png, exr...). Lottie: execute file.exportLottie {comp, path}.
7. Mistakes: history {action: undo}. A failed tool says why; read the message, fix the params (describe_command shows the schema) and retry.
"""

GUIDES = {
    "index": "Topics: workflow, layers, animation, text, shapes, effects, masks, 3d, expressions, tracking, render, ui. Call guide {topic}.",
    "workflow": WORKFLOW,
    "layers": "Create: layer.newSolid {name, color, width, height}, layer.newText {text, size, fill}, layer.newShape {kind, size:[w,h], position:[x,y], fill, stroke, strokeWidth}, layer.newNull, layer.newAdjustment, layer.newCamera, layer.newLight {kind: point|spot|parallel|ambient}. Footage: import_files then layer.addItem {item, time, position}. Arrange: layer.arrange {layers, to: front|forward|backward|back, above}. Time: layer.timing {layers, in, out, start, delta}; layer.sequence {layers, overlap, duration, transition}. Parent: layer.setParent {layers, parent}. Switches: layer.setSwitch {layers, switch: video|audio|solo|lock|shy|collapse|fx|motionBlur|adjustment|threeD, value}. Blend: layer.setBlendMode {layers, mode}. Matte: layer.setTrackMatte {layer, matte, kind: alpha|alphaInverted|luma|lumaInverted}. Precompose: layer.precompose {layers, name}.",
    "animation": "set_keyframes converts composition seconds to layer time for you. Raw commands take LAYER seconds: prop.addKey {layer, path, time, value} adds a key, keys.set {layer, path, time, newTime?, value?} edits an existing one, keys.select {keys: [{layer, path, time}]} then keys.easyEase {which}, keys.interpolation {interpolation: linear|bezier|continuousBezier|autoBezier|hold}, keys.velocity, keys.move {delta}, keys.delete. Motion along a path: keyframe transform/position; layer.autoOrient {mode: alongPath}. Sequence layers: layer.sequence {layers, overlap, duration}. Wiggle keys: keys.wiggle. Time remap: layer.enableTimeRemap; speed: layer.timeStretch {percent}.",
    "text": "layer.newText {text, font, size, fill, justify, position}; edit with layer.setText {layer, text?, font?, size?, fill?, stroke?, strokeWidth?, tracking?, leading?, justify?, box?}. Fonts: execute text.fonts {query}. Animators: layer.addTextAnimator {layer, properties: [position|scale|rotation|opacity|fillColor|tracking|blur...]} then key its range selector (get_layer shows the paths). Presets: layer.applyTextPreset {layer, preset: typewriter|fadeUpCharacters|bounceInWords|trackingIn|scramble|blurIn|jitter|dropInLines}.",
    "shapes": "layer.newShape {kind: rect|rounded|ellipse|polygon|star, size, position, fill, stroke, strokeWidth}. Free paths: shape.newPath {vertices, inTangents, outTangents, closed, fill, stroke}. Add items and operators with layer.addShapeItem {layer, kind: group|rect|ellipse|star|polygon|fill|stroke|gfill|trim|repeater|round|offset|pucker|twist|zigzag|wiggle|merge, group?} (returns {uid, path}). Trim Paths end keyed 0 to 100 is a write-on.",
    "effects": "list_effects {query} finds effects (blur, color correction, distort, generate, keying, matte, noise, perspective, simulation, stylize, time, transition, audio...). apply_effect {layer, effect, params: {paramId: value}} adds and sets one; get_layer shows effects/#n/... paths. Common ids: fx.blur.gaussian (Gaussian Blur: blurriness), fx.stylize.glow (Glow), fx.generate.fill (Fill: color), fx.color.curves, fx.keying.keylight. Layer styles: execute layer.style.add {layers, style: dropShadow|outerGlow|stroke|bevelEmboss...}.",
    "masks": "layer.addMask {layer, shape: rect|ellipse, rect: [x, y, w, h] in layer pixels}; mask.new {layer, vertices: [[x,y]...]} for a polygon; mask properties: masks/#1/feather, masks/#1/opacity, masks/#1/expansion (or layer.mask.set {layer, mask, field, value}); mode with layer.mask.mode {layer, mask, mode: None|Add|Subtract|Intersect|Lighten|Darken|Difference}, invert with layer.mask.invert. Roto Brush: roto.stroke {layer, kind: fg|bg, points (layer px)} then roto.propagate {direction, wait: true}.",
    "3d": "layer.setSwitch {layers, switch: threeD, value: true} makes a layer 3D (transform/position becomes [x,y,z]; transform/orientation, xRotation, yRotation, zRotation). layer.newCamera {preset or zoom}, layer.newLight {kind, color, intensity}. Primitives: layer.new3dPrimitive {kind: cube|sphere|plane|torus|cone|cylinder}; models: layer.newModel {path}. Views: execute view.set3DView {view: activeCamera|front|top|left|custom1}. Depth of field: layer.cameraSettings {dof, focusDistance, aperture}.",
    "expressions": "prop.setExpression {layer, path, expression}. The language is JavaScript with the After Effects object model: time, value, wiggle(freq, amp), loopOut('cycle'), linear(t, a, b, c, d), ease(), thisComp.layer('Name').transform.position, effect('Slider Control')('Slider'). Errors: execute expr.errors. Scripts: run_script {code} runs After Effects scripting JavaScript (app.project, comp.layers.addText, property(...).setValueAtTime) as one undo step, for loops and data.",
    "tracking": "Point track: track.new {layer, kind: transform|stabilize|affine|perspective, target?} then track.analyze {layer, direction: forward, wait: true}, then track.apply {layer}. Stabilize: track.warpStabilizer {layer, wait: true}. 3D camera: track.camera {layer, wait: true}, then camera.createFromSolve {kind: text|solid|null|shadowCatcher|camera}. Masks: track.mask {layer, mask, method}. Planar surface (a screen, a sign): mask around it, then planar_track {layer, track: both, target}. Without wait, use the wait tool.",
    "render": "render {comp, format: h264|prores|hevc|av1|webm|gif|png|jpeg|tiff|exr|wav, output: absolute path, wait: true}. Or renderQueue.add {comp, format, output} then renderQueue.render {wait: true}. Single frame: execute comp.saveFrameAs {path, time}. Lottie: execute file.exportLottie {comp, path}.",
    "ui": "ui {action: state} reports what the person sees (panels, tool, zoom, workspace). ui {action: panel, panel: timeline|effectControls|project|composition|character|renderQueue|terminal...} shows a panel; ui {action: workspace, name}; ui {action: tool, tool: selection|pen|type|rectangle...}; ui {action: zoom, value: fit|100|200}. Selecting layers (layer.select) and moving the time (time.set {time}) also show in the window.",
}

PROMPTS = [{"name": "workflow", "title": "How to build in formvfx", "description": "The loop for compositions, layers, animation, effects, looking at the result and rendering.", "arguments": []}]

_LAYER = {"description": "Layer id, `#n` (1 = top) or name.", "anyOf": [{"type": "integer"}, {"type": "string"}]}
_COMP = {"description": "Composition id or name; omit for the active composition.", "anyOf": [{"type": "integer"}, {"type": "string"}]}
_TIME = {"type": "number", "description": "Composition time in seconds; omit for the current time."}


def _tool(name: str, description: str, props: dict, required: list[str] | None = None, **hints: bool) -> dict:
    return {
        "name": name,
        "description": description,
        "inputSchema": {"type": "object", "properties": props, "required": required or [], "additionalProperties": False},
        "annotations": {"readOnlyHint": hints.get("read", False), "destructiveHint": hints.get("destructive", False), "idempotentHint": hints.get("idempotent", False), "openWorldHint": False},
    }


TOOLS = [
    _tool("status", "Start here. The open project (path, unsaved changes), the active composition (id, name, size, frame rate, duration, layer count, current time), the selection, background jobs, how many formvfx windows are watching, the engine and the extension it saves projects with, and the command and effect counts.", {}, read=True, idempotent=True),
    _tool("list_commands", "formvfx's commands (every menu item, button and drag is one): id, label, menu path, parameter summary, enabled now. Filter by text or namespace (layer, comp, keys, effect, mask, text, shape, camera, track, renderQueue, view, file...).",
          {"filter": {"type": "string"}, "namespace": {"type": "string"}, "enabledOnly": {"type": "boolean", "default": False}, "limit": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 200}}, read=True, idempotent=True),
    _tool("describe_command", "One command in full: label, menu, parameter documentation and JSON Schema, whether it is undoable and enabled now.", {"command": {"type": "string"}}, ["command"], read=True, idempotent=True),
    _tool("execute", "Runs one formvfx command by id with its parameters (see describe_command) and returns its result. Undoable commands are one step in the person's History.",
          {"command": {"type": "string"}, "params": {"type": "object", "description": "The command's parameters.", "additionalProperties": True}}, ["command"]),
    _tool("batch", "Runs several commands as one undo step. Each step is {command, params}; a string \"$N.key\" in a later step's params is replaced by step N's result field, counting steps from 1, e.g. {\"layer\": \"$1.layer\"} is the layer the first step made. atomic (default true) undoes everything if one step fails.",
          {"steps": {"type": "array", "items": {"type": "object", "properties": {"command": {"type": "string"}, "params": {"type": "object", "additionalProperties": True}}, "required": ["command"]}, "minItems": 1, "maxItems": 500},
           "label": {"type": "string", "maxLength": 120}, "atomic": {"type": "boolean", "default": True}}, ["steps"]),
    _tool("get_comp", "A composition: size, frame rate, duration, work area, background, and its layers top to bottom (id, index, name, type, in and out points, start, stretch, switches, blend mode, parent, track matte). With flowchart, also the Flowchart graph: nodes (comps, footage, solids, layers, effects with their depth) and edges from each comp or layer to what it uses, nested comps followed.", {"comp": _COMP, "flowchart": {"type": "boolean", "default": False}}, read=True, idempotent=True),
    _tool("get_layer", "A layer's property tree: every property with its path (use it in set_property and set_keyframes), value at the time, keyframes and expression; masks, effects and text included.",
          {"layer": _LAYER, "comp": _COMP, "depth": {"type": "integer", "minimum": 1, "maximum": 32, "default": 16}, "time": _TIME}, ["layer"], read=True, idempotent=True),
    _tool("get_property", "One property's value (at a time) with its keyframes and expression.", {"layer": _LAYER, "path": {"type": "string"}, "comp": _COMP, "time": _TIME}, ["layer", "path"], read=True, idempotent=True),
    _tool("set_property", "Sets a property's value. On an animated property (it has keyframes) the value is keyed at time (default: the current time). Values: number, [x, y] or [x, y, z] (give scale all three, [sx, sy, 100], or z scale becomes 0), #rrggbb or [r, g, b(, a)] from 0 to 1 for colours, true or false, and the 0-based option index for menus (list_effects with detail shows the options in order).",
          {"layer": _LAYER, "path": {"type": "string", "description": "e.g. transform/position, transform/opacity, effects/#1/blurriness, masks/#1/feather."}, "value": {}, "comp": _COMP, "time": _TIME}, ["layer", "path", "value"], idempotent=True),
    _tool("set_keyframes", "Animates a property: one keyframe per entry at composition time (seconds) with an optional ease (linear, easyEase, easyEaseIn, easyEaseOut, hold). One undo step.",
          {"layer": _LAYER, "path": {"type": "string"}, "comp": _COMP,
           "keys": {"type": "array", "minItems": 1, "maxItems": 200, "items": {"type": "object", "properties": {"time": {"type": "number"}, "value": {}, "ease": {"type": "string", "enum": ["linear", "easyEase", "easyEaseIn", "easyEaseOut", "hold", "bezier", "autoBezier"]}}, "required": ["time", "value"]}}},
          ["layer", "path", "keys"]),
    _tool("apply_effect", "Adds an effect (id like fx.blur.gaussian or its After Effects name like Gaussian Blur) to a layer and sets its parameters by id ({\"blurriness\": 20}). Returns the effect's index for paths effects/#n/....",
          {"layer": _LAYER, "effect": {"type": "string"}, "params": {"type": "object", "additionalProperties": True}, "comp": _COMP}, ["layer", "effect"]),
    _tool("list_effects", "The effects by category with ids; with detail, their parameters (id, name, default, control). Filter by name, id or category.",
          {"query": {"type": "string"}, "category": {"type": "string"}, "detail": {"type": "boolean", "default": False}, "limit": {"type": "integer", "minimum": 1, "maximum": 400, "default": 60}}, read=True, idempotent=True),
    _tool("text_preset", "formvfx's text animation preset library (169 presets in 13 categories: Animate In, Animate Out, Blurs, Fill and Stroke, Lights and Optical, Mechanical, Multi-Line, Organic, Rotation, Scale, Tracking, 3D Text, Miscellaneous). list (filter by query or category) returns id, name, category, description; apply adds the preset's text animators to text layers starting at time (default the current time), one undo step. Reveal presets run about 1.5 s from that time; continuous ones loop.",
          {"action": {"type": "string", "enum": ["list", "apply"]}, "preset": {"type": "string"}, "query": {"type": "string"}, "category": {"type": "string"}, "layer": _LAYER, "layers": {"type": "array", "items": _LAYER}, "time": _TIME, "comp": _COMP}, ["action"]),
    _tool("shapes_from_svg", "Turns SVG markup into a shape layer (paths, fills, strokes, gradients, groups and clips become editable shape contents), centred in the composition, one undo step. Use it for logos, icons and vector art you write as SVG.",
          {"svg": {"type": "string"}, "name": {"type": "string"}, "comp": _COMP}, ["svg"]),
    _tool("planar_track", "Planar tracking: follows a flat surface (a screen, a sign, a wall, a tabletop) through footage and corner-pins another layer onto it. Draw a mask around the surface on the footage layer first (mask.new or layer.addMask, in layer pixels at the reference time). track forward, backward or both analyses the mask with the Perspective method from the reference time (a homography from features inside the mask, every frame); without track the mask must already be tracked. surface is the plane's four corners at the reference time (upper left, upper right, lower left, lower right, layer pixels; default the mask's four vertices or its bounding box). With target, a Corner Pin is keyed on that layer so its corners ride the surface; the result is also kept as a Perspective tracker on the footage for re-applying to other layers. One undo step for the corner pin (tracking is its own step).",
          {"layer": _LAYER, "mask": {"description": "Mask uid, 1-based index or name (default the first mask).", "anyOf": [{"type": "integer"}, {"type": "string"}]}, "target": _LAYER, "surface": {"type": "array", "items": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2}, "minItems": 4, "maxItems": 4}, "reference": _TIME, "track": {"type": "string", "enum": ["forward", "backward", "both"]}, "start": {"type": "number"}, "end": {"type": "number"}, "comp": _COMP}, ["layer"]),
    _tool("object_matte", "Object Matte: isolates one object (a person, a car, a product) in footage from one point on it, the way After Effects' Object Matte does. action preview segments the object at time and returns its bounding box, coverage and the model's confidence without changing anything; action select puts a Roto Brush on the layer seeded at that point (one undo step), spans the whole layer and propagates it (direction both by default; wait true blocks until done, otherwise call wait). The layer's alpha becomes the object's matte. Needs the MobileSAM model (roto.model.download {id: mobilesam}).",
          {"action": {"type": "string", "enum": ["preview", "select"]}, "layer": _LAYER, "point": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2, "description": "[x, y] in the layer's pixels"}, "time": _TIME, "direction": {"type": "string", "enum": ["both", "forward", "backward"]}, "wait": {"type": "boolean", "default": False}, "comp": _COMP}, ["action", "layer", "point"]),
    _tool("render_frame", "Renders one frame of a composition and returns it as an image so you can see it. Use it after changes and at the moments that matter. For a 3D comp, view renders from a 3D view (top, front, left, right, back, bottom, default, custom1-3) instead of the active camera, to check layout in space. backend auto (default) renders on the GPU or the CPU, whichever is faster for the comp; gpu or cpu forces one (status shows the GPU).",
          {"comp": _COMP, "time": _TIME, "max_size": {"type": "integer", "minimum": 64, "maximum": 2048, "default": 960}, "transparent": {"type": "boolean", "default": False}, "view": {"type": "string", "enum": ["activeCamera", "default", "front", "left", "top", "back", "right", "bottom", "custom1", "custom2", "custom3"]}, "backend": {"type": "string", "enum": ["auto", "gpu", "cpu"]}}, read=True, idempotent=True),
    _tool("contact_sheet", "Renders several frames across a time span into one image grid (each labelled with its time) to check motion and timing in one look.",
          {"comp": _COMP, "start": {"type": "number"}, "end": {"type": "number"}, "count": {"type": "integer", "minimum": 2, "maximum": 24, "default": 8}, "max_size": {"type": "integer", "minimum": 64, "maximum": 640, "default": 320}}, read=True, idempotent=True),
    _tool("render", "Renders a composition to a file through the Render Queue and waits for it: format h264, prores, hevc, av1, webm, gif, png, jpeg, tiff, exr, wav or aiff; output is an absolute path (default: the renders folder).",
          {"comp": _COMP, "format": {"type": "string", "default": "h264"}, "output": {"type": "string"}, "wait": {"type": "boolean", "default": True}}),
    _tool("dynamic_link", "Dynamic Link: another app (formcut, an editor, a script) uses a composition as a live clip with no render. export writes a .fvxlink descriptor beside the saved project (or at path) naming the project, the comp and the frame and audio URLs; the editor fetches GET /api/link/frame.png?project=&comp=&time= and /api/link/audio with the x-formvfx-local: 1 header, so every saved change shows up on its next frame (the open project serves its unsaved state too). info reads a comp's size, rate and duration from any saved project file, and frame renders one of its frames, both without opening that project in the window. Links survive a moved or renamed project (found again by its project id or file name beside the .fvxlink, in the project's old folder or among recent projects) and a renumbered comp (found again by name). check reads a .fvxlink (path), reports whether it resolves, was relinked, or was written by a newer formvfx, and with write: true rewrites it to the new location; render renders the linked comp to a movie beside the project (Render and Replace for the editor); status lists the projects being served and the recent link requests.",
          {"action": {"type": "string", "enum": ["export", "info", "frame", "check", "render", "status"]}, "project": {"type": "string", "description": "absolute project path, .fvxproj or .ecproj (default the open project)"}, "comp": _COMP, "path": {"type": "string", "description": "export: where to write the .fvxlink; check and render: the .fvxlink to read"}, "write": {"type": "boolean", "default": False}, "format": {"type": "string", "enum": ["prores", "h264", "hevc", "webm"], "default": "prores"}, "output": {"type": "string"}, "time": _TIME, "max_size": {"type": "integer", "minimum": 64, "maximum": 2048, "default": 960}}, ["action"]),
    _tool("project", "Project files: new, open (path), save (path for the first save), saveAs (path), close. formvfx projects are .fvxproj JSON; projects saved as .ecproj by earlier formvfx releases open as imports.",
          {"action": {"type": "string", "enum": ["new", "open", "save", "saveAs", "close", "demo"]}, "path": {"type": "string"}}, ["action"], destructive=True),
    _tool("import_files", "Imports footage, images, audio, PSD, SVG, PDF, Lottie or 3D models (absolute paths) into the project; addToComp also places them in the active composition.",
          {"paths": {"type": "array", "items": {"type": "string"}, "minItems": 1}, "addToComp": {"type": "boolean", "default": False}}, ["paths"]),
    _tool("run_script", "Runs an After Effects style JavaScript program against the project (app.project, items.addComp, CompItem, layers.addSolid/addText/addNull/addShape, layer.property('Transform').property('Position').setValueAtTime(t, v), effects through property('Effects').addProperty, File, app.settings, ScriptUI windows). The whole script is one undo step. Returns {ok, result, output (writeLn, alert and $.writeln), error: {message, line}}.", {"code": {"type": "string"}}, ["code"]),
    _tool("history", "Undo, redo, list the history, or go to a step.", {"action": {"type": "string", "enum": ["undo", "redo", "list", "goto"]}, "steps": {"type": "integer", "minimum": 1, "maximum": 200, "default": 1}, "index": {"type": "integer"}}, ["action"]),
    _tool("ui", "The formvfx window: state (what the person sees: panels, tool, zoom, workspace), or show a panel, switch workspace, choose a tool, zoom the viewer.",
          {"action": {"type": "string", "enum": ["state", "panel", "workspace", "tool", "zoom"]}, "panel": {"type": "string"}, "name": {"type": "string"}, "tool": {"type": "string"}, "value": {}}, ["action"]),
    _tool("wait", "Waits until background work (rendering, tracking, stabilizing, Roto Brush, Content-Aware Fill) has finished, up to timeout seconds.", {"timeout": {"type": "number", "minimum": 1, "maximum": 3600, "default": 120}}, read=True),
    _tool("guide", "formvfx know-how for agents: workflow, layers, animation, text, shapes, effects, masks, 3d, expressions, tracking, render, ui.", {"topic": {"type": "string"}}, read=True, idempotent=True),
]
NAMES = {t["name"] for t in TOOLS}
READ_ONLY = {t["name"] for t in TOOLS if t["annotations"]["readOnlyHint"]}


def text(value: Any) -> dict:
    body = value if isinstance(value, str) else json.dumps(value, indent=1, ensure_ascii=False)
    return {"content": [{"type": "text", "text": body}]}


def fail(message: str) -> dict:
    return {"content": [{"type": "text", "text": message}], "isError": True}


def image(png: bytes, caption: str) -> dict:
    return {"content": [{"type": "image", "data": base64.b64encode(png).decode(), "mimeType": "image/png"}, {"type": "text", "text": caption}]}


async def ex(command: str, params: dict | None, source: str) -> Any:
    return await rpc.execute(command, params or {}, source)


def _comp_param(args: dict) -> dict:
    return {"comp": args["comp"]} if args.get("comp") is not None else {}


async def _timing(args: dict, source: str):
    info = await ex("comp.info", _comp_param(args), source)
    ref = args.get("layer")
    row = None
    for r in info.get("layers") or []:
        if ref == r.get("id") or (isinstance(ref, str) and (ref == r.get("name") or ref == f"#{r.get('index')}" or ref == str(r.get("id")))):
            row = r
            break
    if row is None:
        raise RpcError(f"no layer {ref!r} in the composition; get_comp lists them")
    fps = info.get("frameRate") or 30.0
    start = row.get("start") or 0.0
    stretch = row.get("stretch") or 100.0

    def layer_time(t: float) -> float:
        snapped = round(t * fps) / fps
        return (snapped - start) * 100.0 / stretch

    return layer_time


async def _layer_time(args: dict, t: float, source: str) -> float:
    return (await _timing(args, source))(t)


def _png_grid(frames: list[tuple[int, int, bytes]], cols: int, pad: int = 4) -> tuple[int, int, bytes]:
    cw = max(w for w, _, _ in frames)
    ch = max(h for _, h, _ in frames)
    rows = (len(frames) + cols - 1) // cols
    W, H = cols * cw + (cols + 1) * pad, rows * ch + (rows + 1) * pad
    canvas = bytearray(bytes([24, 24, 26, 255]) * (W * H))
    for i, (w, h, px) in enumerate(frames):
        ox = pad + (i % cols) * (cw + pad)
        oy = pad + (i // cols) * (ch + pad)
        for y in range(h):
            start = ((oy + y) * W + ox) * 4
            canvas[start:start + w * 4] = px[y * w * 4:(y + 1) * w * 4]
    return W, H, bytes(canvas)


def _encode_png(w: int, h: int, px: bytes) -> bytes:
    try:
        from .. import native

        return native.encode_png(w, h, px)
    except BaseException:
        raw = b"".join(b"\x00" + px[y * w * 4:(y + 1) * w * 4] for y in range(h))

        def chunk(tag: bytes, data: bytes) -> bytes:
            return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

        return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b"")


async def call(name: str, args: dict, source: str) -> dict:
    try:
        return await _call(name, args, source)
    except RpcError as exc:
        return fail(f"{exc}. Check the parameters with describe_command, or get_comp / get_layer for ids and paths.")
    except Exception as exc:
        return fail(f"formvfx could not do that: {exc}")


async def _call(name: str, a: dict, source: str) -> dict:
    if name == "status":
        info = await rpc.dispatch("engine.info", {}, source)
        summary = await ex("project.summary", {}, source)
        comp = None
        if summary.get("activeComp") is not None:
            ci = await ex("comp.info", {}, source)
            comp = {k: ci.get(k) for k in ("id", "name", "width", "height", "frameRate", "duration", "workArea", "background")}
            comp["layers"] = len(ci.get("layers") or [])
        items = summary.get("items") or []
        caps = info.get("capabilities") or {}
        return text({
            "formvfx": __version__,
            "engine": caps.get("engine") or "fvx",
            "projectExtension": caps.get("projectExtension") or "fvxproj",
            "project": {"path": summary.get("path"), "unsaved": summary.get("dirty"), "items": len(items), "compositions": [{"id": i["id"], "name": i["name"]} for i in items if i.get("type") == "Composition"][:50]},
            "activeComp": comp,
            "time": summary.get("time"),
            "selectedLayers": summary.get("selectedLayers"),
            "busy": host.busy,
            "windows": len(host.bus.windows()),
            "commands": caps.get("commands"),
            "effects": (caps.get("effects") or {}).get("total"),
            "gpu": info.get("gpu"),
            "export": caps.get("export"),
            "next": "No composition yet: execute comp.new." if comp is None else "get_comp for the layers; render_frame to look.",
        })
    if name == "list_commands":
        rows = await ex("command.list", {"filter": a.get("filter") or None, "enabledOnly": bool(a.get("enabledOnly"))} if a.get("filter") else {"enabledOnly": bool(a.get("enabledOnly"))}, source)
        ns = (a.get("namespace") or "").strip().rstrip(".")
        if ns:
            rows = [r for r in rows if r["id"].startswith(ns + ".")]
        limit = int(a.get("limit") or 200)
        out = [{"id": r["id"], "label": r["label"], "menu": "/".join(r.get("menu") or []), "params": r.get("params"), "enabled": r.get("enabled")} for r in rows[:limit]]
        return text({"total": len(rows), "shown": len(out), "commands": out})
    if name == "describe_command":
        return text(await ex("command.describe", {"command": a.get("command")}, source))
    if name == "execute":
        result = await ex(a.get("command", ""), a.get("params") or {}, source)
        return text({"result": result, "revision": host.revision})
    if name == "batch":
        args = {"steps": a.get("steps") or [], "atomic": a.get("atomic", True)}
        if a.get("label"):
            args["label"] = a["label"]
        result = await rpc.dispatch("command.batch", args, source)
        return text({"result": result, "revision": host.revision})
    if name == "get_comp":
        info = await ex("comp.info", _comp_param(a), source)
        if a.get("flowchart"):
            flow = await rpc.dispatch("ui.flowchart", {**_comp_param(a), "layers": True, "effects": True}, source)
            info = {**info, "flowchart": flow} if isinstance(info, dict) else {"comp": info, "flowchart": flow}
        return text(info)
    if name == "get_layer":
        p = {"layer": a["layer"], "depth": a.get("depth", 16), **_comp_param(a)}
        if a.get("time") is not None:
            p["time"] = a["time"]
        return text(await ex("layer.tree", p, source))
    if name == "get_property":
        p = {"layer": a["layer"], "path": a["path"], **_comp_param(a)}
        if a.get("time") is not None:
            p["time"] = a["time"]
        return text(await ex("prop.get", p, source))
    if name == "set_property":
        p = {"layer": a["layer"], "path": a["path"], "value": a.get("value"), **_comp_param(a)}
        if a.get("time") is not None:
            p["time"] = await _layer_time(a, float(a["time"]), source)
        return text({"result": await ex("prop.set", p, source), "revision": host.revision})
    if name == "set_keyframes":
        steps = []
        keys = a.get("keys") or []
        to_layer = await _timing(a, source)
        for k in keys:
            lt = to_layer(float(k["time"]))
            steps.append({"command": "prop.addKey", "params": {"layer": a["layer"], "path": a["path"], "time": lt, "value": k.get("value"), **_comp_param(a)}})
        eases = {k.get("ease") for k in keys if k.get("ease")}
        if eases:
            for k in keys:
                ease = k.get("ease")
                if not ease or ease == "linear":
                    continue
                lt = to_layer(float(k["time"]))
                steps.append({"command": "keys.select", "params": {"keys": [{"layer": a["layer"], "path": a["path"], "time": lt}]}})
                if ease in ("easyEase", "easyEaseIn", "easyEaseOut"):
                    steps.append({"command": f"keys.{ease}", "params": {}})
                elif ease == "hold":
                    steps.append({"command": "keys.interpolation", "params": {"interpolation": "hold"}})
                else:
                    steps.append({"command": "keys.interpolation", "params": {"interpolation": ease}})
        result = await rpc.dispatch("command.batch", {"steps": steps, "label": f"Keyframe {a['path']}", "atomic": True}, source)
        return text({"keys": len(keys), "result": result, "revision": host.revision})
    if name == "apply_effect":
        layer = a["layer"]
        before = await ex("layer.tree", {"layer": layer, "depth": 3, **_comp_param(a)}, source)
        count = _effect_count(before)
        result = await ex("effect.apply", {"effect": a["effect"], "layers": [layer], **_comp_param(a)}, source)
        index = count + 1
        set_errors = []
        for pid, value in (a.get("params") or {}).items():
            try:
                await ex("prop.set", {"layer": layer, "path": f"effects/#{index}/{pid}", "value": value, **_comp_param(a)}, source)
            except RpcError as exc:
                set_errors.append(f"{pid}: {exc}")
        valid = None
        if set_errors:
            cat = await rpc.dispatch("effects.catalog", {}, source)
            spec = next((e for e in cat if e["id"] == result.get("effect")), None) if isinstance(result, dict) else None
            valid = [p["id"] for p in spec["params"]] if spec else None
        return text({"result": result, "effectIndex": index, "path": f"effects/#{index}", "paramErrors": set_errors or None, "validParams": valid, "revision": host.revision})
    if name == "list_effects":
        cat = await rpc.dispatch("effects.catalog", {}, source)
        q = (a.get("query") or "").lower()
        c = (a.get("category") or "").lower()
        rows = [e for e in cat if (not q or q in e["name"].lower() or q in e["id"].lower()) and (not c or c in e["category"].lower())]
        limit = int(a.get("limit") or 60)
        if a.get("detail"):
            out = rows[:limit]
        else:
            out = [{"id": e["id"], "name": e["name"], "category": e["category"], "params": [p["id"] for p in e["params"]]} for e in rows[:limit]]
        cats = sorted({e["category"] for e in cat})
        return text({"total": len(rows), "shown": len(out), "effects": out, "categories": cats if not (q or c) else None})
    if name == "text_preset":
        if a.get("action") == "list":
            rows = await rpc.dispatch("text.library", {"query": a.get("query"), "category": a.get("category")}, source)
            return text({"total": len(rows), "presets": rows})
        args = {k: a[k] for k in ("preset", "layer", "layers", "time", "comp") if a.get(k) is not None}
        return text(await rpc.dispatch("text.applyLibraryPreset", args, source))
    if name == "shapes_from_svg":
        args = {k: a[k] for k in ("svg", "name", "comp") if a.get(k) is not None}
        return text(await rpc.dispatch("clipboard.pasteSvg", args, source))
    if name == "object_matte":
        layer = a.get("layer")
        if isinstance(layer, str):
            ci = await ex("comp.info", _comp_param(a), source)
            row = next((r for r in ci.get("layers") or [] if f"#{r.get('index')}" == layer or r.get("name") == layer), None)
            if not row:
                return fail(f"no layer {layer!r}")
            layer = row["id"]
        if a.get("action") == "preview":
            r = await rpc.dispatch("roto.objectPreview", {"layer": layer, "point": a.get("point"), **({"time": a["time"]} if a.get("time") is not None else {}), **_comp_param(a)}, source)
            return text({k: r.get(k) for k in ("layer", "time", "bbox", "coverage", "score")})
        if a.get("time") is not None:
            await ex("time.set", {"time": a["time"], **_comp_param(a)}, source)
        args = {"layer": layer, "point": a.get("point"), "direction": a.get("direction") or "both", "wait": bool(a.get("wait"))}
        return text(await rpc.dispatch("roto.objectMatte", {**args, **_comp_param(a)}, source))
    if name == "planar_track":
        args = {k: a[k] for k in ("layer", "mask", "target", "surface", "reference", "track", "start", "end", "comp") if a.get(k) is not None}
        return text(await rpc.dispatch("track.planar", args, source))
    if name == "dynamic_link":
        from ..engine import link

        act = a.get("action")
        if act == "export":
            return text(await rpc.dispatch("link.export", {k: a[k] for k in ("comp", "path") if a.get(k) is not None}, source))
        if act == "info":
            return text(await link.info(a.get("project"), a.get("comp")))
        if act == "status":
            return text(link.status())
        if act in ("check", "render"):
            p = a.get("path")
            if not p:
                return fail("pass `path`, the .fvxlink file")
            try:
                descriptor = json.loads(Path(p).expanduser().read_text(encoding="utf-8"))
            except (OSError, ValueError) as exc:
                return fail(f"{p} is not a readable .fvxlink: {exc}")
            if act == "check":
                return text(await link.check(descriptor, p, bool(a.get("write"))))
            return text(await link.render(descriptor, a.get("output"), str(a.get("format") or "prores")))
        w, h, png = await link.frame(a.get("project"), a.get("comp"), float(a.get("time") or 0.0), int(a.get("max_size") or 960), False)
        return image(png, f"{w} x {h} at {a.get('time') or 0} s")
    if name == "render_frame":
        p = {"maxSide": int(a.get("max_size") or 960), "transparent": bool(a.get("transparent")), "display": False, **_comp_param(a)}
        if a.get("time") is not None:
            p["time"] = a["time"]
        if a.get("view"):
            p["view3d"] = a["view"]
        if a.get("backend") in ("auto", "gpu", "cpu"):
            p["backend"] = a["backend"]
        out = await rpc.render_png(p, source)
        t = a.get("time")
        return image(out["png"], f"{out['width']} x {out['height']}" + (f" at {t} s" if t is not None else " at the current time"))
    if name == "contact_sheet":
        ci = await ex("comp.info", _comp_param(a), source)
        start = float(a.get("start") if a.get("start") is not None else (ci.get("workArea") or [0, 0])[0])
        end = float(a.get("end") if a.get("end") is not None else (ci.get("workArea") or [0, ci.get("duration", 1)])[1])
        n = int(a.get("count") or 8)
        fd = 1.0 / (ci.get("frameRate") or 30)
        end = max(start, end - fd)
        times = [round(start + (end - start) * i / (n - 1), 4) for i in range(n)]
        loop = asyncio.get_running_loop()
        frames = []
        for t in times:
            job = await host.call(lambda e, t=t: e.frame_job(json.dumps({"time": t, "maxSide": int(a.get("max_size") or 320), "display": False, **_comp_param(a)})), source)
            frames.append(await loop.run_in_executor(host.frames, job.run))
        cols = 2 if n == 4 else 4 if n > 9 or n in (7, 8) else min(n, 3)
        W, H, px = await loop.run_in_executor(host.frames, _png_grid, frames, cols)
        png = await loop.run_in_executor(host.frames, _encode_png, W, H, px)
        return image(png, "Frames left to right, top to bottom at " + ", ".join(f"{t:g} s" for t in times))
    if name == "render":
        from .workspace import ensure

        fmt = a.get("format") or "h264"
        params = {"format": fmt, **_comp_param(a)}
        output = a.get("output")
        if not output:
            ci = await ex("comp.info", _comp_param(a), source)
            ext = {"h264": "mp4", "hevc": "mp4", "av1": "mp4", "prores": "mov", "webm": "webm", "gif": "gif", "png": "png", "jpeg": "jpg", "tiff": "tif", "exr": "exr", "wav": "wav", "aiff": "aif"}.get(fmt, "mp4")
            safe = "".join(ch if ch.isalnum() or ch in "-_ " else "_" for ch in str(ci.get("name") or "render")).strip() or "render"
            output = str(ensure() / "renders" / f"{safe}.{ext}")
        params["output"] = output
        added = await ex("renderQueue.add", params, source)
        rendered = await ex("renderQueue.render", {"wait": bool(a.get("wait", True))}, source)
        return text({"output": output, "queued": added, "result": rendered})
    if name == "project":
        action = a.get("action")
        path = a.get("path")
        cmd = {"new": "file.newProject", "open": "file.open", "save": "file.save", "saveAs": "file.saveAs", "close": "file.closeProject", "demo": "file.openDemoProject"}[action]
        p = {"path": path} if path else {}
        if action in ("open", "saveAs") and not path:
            return fail(f"{action} needs an absolute path")
        return text({"result": await ex(cmd, p, source), "revision": host.revision})
    if name == "import_files":
        p = {"paths": a.get("paths") or []}
        if a.get("addToComp"):
            p["addToComp"] = True
        return text({"result": await ex("file.import", p, source), "revision": host.revision})
    if name == "run_script":
        return text(await ex("script.run", {"code": a.get("code", "")}, source))
    if name == "history":
        action = a.get("action")
        if action == "list":
            return text(await ex("edit.history.list", {}, source))
        if action == "goto":
            return text(await ex("edit.history.goto", {"index": a.get("index", 0)}, source))
        steps = int(a.get("steps") or 1)
        out = None
        for _ in range(steps):
            out = await ex("edit.undo" if action == "undo" else "edit.redo", {}, source)
        return text({"result": out, "revision": host.revision})
    if name == "ui":
        action = a.get("action")
        if action == "state":
            reply = await host.bus.ask_window({"type": "uiRequest", "what": "state"}, timeout=5)
            return text(reply or {"windows": 0, "note": "No formvfx window is open; the project is still editable."})
        if action == "panel":
            return text(await ex("window.panel", {"panel": a.get("panel") or a.get("name")}, source))
        if action == "workspace":
            return text(await ex("window.workspace", {"name": a.get("name")}, source))
        host.bus.publish({"type": "uiAction", "action": action, "tool": a.get("tool"), "value": a.get("value"), "source": source}, ("ui",))
        return text({"sent": action, "windows": len(host.bus.windows())})
    if name == "wait":
        timeout = float(a.get("timeout") or 120)
        loop = asyncio.get_running_loop()
        deadline = loop.time() + timeout
        while host.busy and loop.time() < deadline:
            await asyncio.sleep(0.25)
        return text({"done": not host.busy, "busy": host.busy})
    if name == "guide":
        topic = (a.get("topic") or "index").lower()
        return text(GUIDES.get(topic, GUIDES["index"]))
    return fail(f"no tool named {name}")


def _effect_count(tree: dict) -> int:
    props = tree.get("properties") or {}
    for child in props.get("children") or []:
        if child.get("match") == "effects":
            return len(child.get("children") or [])
    return 0
