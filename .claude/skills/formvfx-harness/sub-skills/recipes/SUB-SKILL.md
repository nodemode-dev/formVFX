---
name: recipes
description: Proven formvfx builds to start from. Use when the request matches a common motion graphics deliverable such as a lower third, a logo or title reveal with a per-character animator, a 3D card flip with camera, light and shadow, footage seen through text with a track matte and a rig, a write-on line, or a tracked label that follows footage; each recipe is the exact tool sequence that was run against the engine and checked on rendered frames.
---

# recipes

Each recipe below ran against the formvfx engine and was checked with `contact_sheet`. Change names, colours, sizes and times to fit the brief; keep the structure. After building, look with `contact_sheet` before reporting.

## Lower third (bar draws on, name slides in, title types on)

```json
["execute", {"command": "comp.new", "params": {"name": "Lower Third", "width": 1920, "height": 1080, "duration": 5, "frameRate": 30, "background": "#1d2230"}}]
["batch", {"label": "Lower third", "steps": [
  {"command": "layer.newShape", "params": {"kind": "rect", "name": "Bar", "size": [720, 120], "position": [560, 860], "fill": "#f97316"}},
  {"command": "layer.newText", "params": {"text": "Ada Lovelace", "name": "Name", "size": 64, "fill": "#ffffff", "position": [240, 850], "justify": "left"}},
  {"command": "layer.newText", "params": {"text": "Analytical Engine", "name": "Title", "size": 34, "fill": "#ffd9bf", "position": [242, 900], "justify": "left"}},
  {"command": "layer.addShapeItem", "params": {"layer": "$1.layer", "kind": "trim"}}]}]
["set_keyframes", {"layer": "Bar", "path": "contents/trim/end", "keys": [{"time": 0, "value": 0, "ease": "easyEaseOut"}, {"time": 0.6, "value": 100, "ease": "easyEase"}]}]
["set_keyframes", {"layer": "Name", "path": "transform/position", "keys": [{"time": 0.3, "value": [180, 850], "ease": "easyEase"}, {"time": 0.9, "value": [240, 850], "ease": "easyEase"}]}]
["set_keyframes", {"layer": "Name", "path": "transform/opacity", "keys": [{"time": 0.3, "value": 0}, {"time": 0.8, "value": 100}]}]
["execute", {"command": "layer.applyTextPreset", "params": {"layer": "Title", "preset": "typewriter"}}]
["execute", {"command": "layer.style.add", "params": {"layers": ["Name"], "style": "dropShadow"}}]
```

Exit: key the same properties back out (faster, 0.3 to 0.4 s) before the end of the shot.

## Logo reveal (radial gradient background, per-character fade, scale and blur, glow)

```json
["batch", {"label": "Logo reveal", "steps": [
  {"command": "comp.new", "params": {"name": "Logo Reveal", "width": 1280, "height": 720, "duration": 4, "frameRate": 30}},
  {"command": "layer.newSolid", "params": {"name": "Background", "color": "#000000"}},
  {"command": "effect.apply", "params": {"effect": "Gradient Ramp", "layers": ["$2.layer"]}},
  {"command": "layer.newText", "params": {"text": "NOVA", "name": "Logo", "size": 180, "fill": "#ffffff"}},
  {"command": "layer.addTextAnimator", "params": {"layer": "$4.layer", "properties": ["opacity", "scale", "blur"]}},
  {"command": "effect.apply", "params": {"effect": "Glow", "layers": ["$4.layer"]}}]}]
["set_property", {"layer": "Background", "path": "effects/#1/startColor", "value": "#1a1040"}]
["set_property", {"layer": "Background", "path": "effects/#1/endColor", "value": "#05050a"}]
["set_property", {"layer": "Background", "path": "effects/#1/shape", "value": 1}]
["set_property", {"layer": "Logo", "path": "text/animators/#1/properties/opacity", "value": 0}]
["set_property", {"layer": "Logo", "path": "text/animators/#1/properties/scale", "value": [300, 300, 100]}]
["set_property", {"layer": "Logo", "path": "text/animators/#1/properties/blur", "value": [20, 20]}]
["set_keyframes", {"layer": "Logo", "path": "text/animators/#1/selectors/#1/offset", "keys": [{"time": 0, "value": 0, "ease": "easyEase"}, {"time": 1.5, "value": 100, "ease": "easyEase"}]}]
["set_property", {"layer": "Logo", "path": "effects/Glow/radius", "value": 40}]
```

## 3D card flip (camera, spot and ambient light, shadow on a floor)

```json
["batch", {"label": "3D card", "steps": [
  {"command": "comp.new", "params": {"name": "Card Flip", "width": 1280, "height": 720, "duration": 4, "frameRate": 30, "background": "#101418"}},
  {"command": "layer.newSolid", "params": {"name": "Floor", "color": "#2a3340", "width": 3000, "height": 3000}},
  {"command": "layer.newSolid", "params": {"name": "Card", "color": "#f97316", "width": 480, "height": 300}},
  {"command": "layer.setSwitch", "params": {"layers": ["$2.layer", "$3.layer"], "switch": "threeD", "value": true}},
  {"command": "layer.newCamera", "params": {"preset": "35mm"}},
  {"command": "layer.newLight", "params": {"kind": "Spot", "intensity": 140, "castsShadows": true}},
  {"command": "layer.newLight", "params": {"kind": "Ambient", "intensity": 25}}]}]
["set_property", {"layer": "Floor", "path": "transform/orientation", "value": [90, 0, 0]}]
["set_property", {"layer": "Floor", "path": "transform/position", "value": [640, 560, 0]}]
["set_property", {"layer": "Card", "path": "materialOptions/castsShadows", "value": 1}]
["set_keyframes", {"layer": "Card", "path": "transform/rotationY", "keys": [{"time": 0, "value": 0, "ease": "easyEase"}, {"time": 2, "value": 180, "ease": "easyEase"}]}]
["execute", {"command": "prop.setExpression", "params": {"layer": "Card", "path": "transform/position", "expression": "value + [0, Math.sin(time * 3) * 20, 0]"}}]
```

## Footage through text (track matte, a null rig, a dotted row)

```json
["execute", {"command": "comp.new", "params": {"name": "Matte", "width": 1280, "height": 720, "duration": 3, "frameRate": 24}}]
["import_files", {"paths": ["/absolute/path/to/footage_or_image.png"], "addToComp": true}]
["batch", {"label": "Matte", "steps": [
  {"command": "layer.newText", "params": {"text": "MATTE", "name": "Word", "size": 260, "fill": "#ffffff"}},
  {"command": "layer.setTrackMatte", "params": {"layer": "footage_or_image.png", "matte": "$1.layer", "kind": "alpha"}},
  {"command": "layer.newNull", "params": {"name": "Rig"}},
  {"command": "layer.setParent", "params": {"layers": ["Word"], "parent": "$3.layer"}},
  {"command": "layer.newShape", "params": {"kind": "ellipse", "name": "Dots", "size": [40, 40], "position": [100, 640], "fill": "#f97316"}},
  {"command": "layer.addShapeItem", "params": {"layer": "$5.layer", "kind": "repeater"}}]}]
["set_property", {"layer": "Dots", "path": "contents/repeater/copies", "value": 12}]
["set_property", {"layer": "Dots", "path": "contents/repeater/transform/position", "value": [90, 0]}]
["execute", {"command": "prop.setExpression", "params": {"layer": "Rig", "path": "transform/rotation", "expression": "wiggle(2, 4)"}}]
```

The imported layer is named after its file; use that name as the matted layer.

## Write-on line

```json
["execute", {"command": "shape.newPath", "params": {"vertices": [[100, 500], [600, 300], [1100, 500]], "closed": false, "fill": false, "stroke": "#ffffff", "strokeWidth": 8, "name": "Line"}}]
["execute", {"command": "layer.addShapeItem", "params": {"layer": "Line", "kind": "trim"}}]
["set_keyframes", {"layer": "Line", "path": "contents/trim/end", "keys": [{"time": 0, "value": 0, "ease": "easyEaseOut"}, {"time": 1, "value": 100, "ease": "easyEase"}]}]
```

A hand-drawn alternative: `paint.stroke {layer, points, color, diameter, durationMode: writeOn, duration}` on a solid.

## Label that follows footage (point track)

The full sequence is in `tracking-and-stabilizing`: `track.new`, `track.setPoint` on the feature, `track.analyze {wait: true}`, a null as `track.setTarget`, `track.apply`, then parent the label to the null with an offset.

## Text on a circle

```json
["batch", {"steps": [
  {"command": "layer.newText", "params": {"text": "TEXT ON A CIRCLE", "name": "Ring", "size": 40, "fill": "#f97316"}},
  {"command": "layer.addMask", "params": {"layer": "$1.layer", "shape": "ellipse", "rect": [-200, -200, 400, 400], "mode": "None"}}]}]
["set_property", {"layer": "Ring", "path": "text/pathOptions/path", "value": 1}]
["set_property", {"layer": "Ring", "path": "text/pathOptions/reversePath", "value": true}]
```

Key `text/pathOptions/firstMargin` to run the text around the circle.
