---
name: compositions-and-layers
description: Compositions and layers in formvfx. Use to create or change a composition (size, frame rate, duration, background, work area), add solids, text, shapes, nulls, adjustment layers or footage, import files, arrange and time layers, parent them, set track mattes, blend modes and switches, duplicate, split, delete or precompose layers.
---

# compositions-and-layers

## Compositions

| Need | Command |
| --- | --- |
| New composition (becomes active) | `comp.new {name, width, height, frameRate, duration, background}` |
| Change one | `comp.settings {comp, name?, width?, height?, frameRate?, duration?, background?}` |
| Switch the active one | `comp.open {comp}` |
| Work area (what previews and renders cover) | `comp.workArea {start, end}` |
| Composition switches | `comp.setSwitch {switch: hideShy|motionBlur|frameBlending|draft3d, value}` |
| Read it | `get_comp {comp}` (layers top to bottom with ids, timing, switches, parent, matte) |
| Trim to the work area, crop to a region | `comp.trimToWorkArea`, `comp.cropToRegionOfInterest`, `comp.cropToLayerBounds {layers}` |

A composition background colour is not rendered into transparent formats; use a solid as the bottom layer when the background must be in the file.

## New layers

All return `{"layer": id}` and land above the selected layer (or on top).

| Layer | Command | Key parameters |
| --- | --- | --- |
| Solid | `layer.newSolid` | `name, color, width, height` (default: the composition size) |
| Text | `layer.newText` | `text, font, size, fill, stroke, strokeWidth, justify, position, box` |
| Shape | `layer.newShape` | `kind: rect|rounded|ellipse|star|polygon, size: [w, h], position, fill, stroke, strokeWidth` |
| Null | `layer.newNull` | `name` |
| Adjustment | `layer.newAdjustment` | `name` (its effects apply to every layer below it) |
| Camera, light | `layer.newCamera`, `layer.newLight` | see `camera-lights-3d` |
| 3D primitive or model | `layer.new3dPrimitive`, `layer.newModel` | see `camera-lights-3d` |
| Footage or another composition | `layer.addItem {item, time, index, position}` | `item` is a project item id or name |

## Footage

`import_files {paths: [absolute paths], addToComp}` imports video, image sequences, stills, audio, Photoshop, Illustrator and PDF files, SVG, Lottie and 3D models. With `addToComp: true` each becomes a layer named after its file. The raw command is `file.import {paths, importAs: footage|composition|compositionLayerSizes}`; a layered Photoshop file imported `as composition` becomes a composition with one layer per Photoshop layer.

## Arrange and time

| Need | Command |
| --- | --- |
| Stacking order | `layer.arrange {layers, to: front|forward|backward|back}` or `{layers, above: layer}` |
| In, out and start | `layer.timing {layers, in, out, start}`; `{layers, delta}` shifts by seconds |
| Slide footage inside its bar | `layer.slip {layers, delta}` |
| One after another | `layer.sequence {layers (in order), overlap, duration, transition: off|dissolveFront|crossDissolve}` |
| Speed | `layer.timeStretch {layers, percent}` (200 is half speed), `layer.timeReverse {layers}` |
| Rename | `layer.rename {layer, name}` |
| Align or distribute in the frame | `layer.align {edge: left|hcenter|right|top|vcenter|bottom, to: composition|selection, layers}`, `layer.distribute {mode, layers}` |
| Centre, fit, flip | `layer.transform {layers, op: center|fit|fitWidth|fitHeight|flipH|flipV|reset}`, `layer.centerAnchor {layers}` |

## Selection-based edits

`edit.duplicate`, `edit.splitLayer`, `edit.clear` (delete), `edit.copy` and `edit.paste` act on the selection, as in the menus. Select first, in the same batch:

```json
{"steps": [{"command": "layer.select", "params": {"layers": [345]}}, {"command": "edit.duplicate"}]}
```

`edit.duplicate` returns the new layer ids. To delete layers: `layer.select` then `edit.clear`.

## Structure

- **Parenting**: `layer.setParent {layers, parent}` (`parent: null` to clear). The child keeps its place on screen and then follows the parent's position, rotation and scale. Nulls are the usual parents.
- **Track mattes**: `layer.setTrackMatte {layer, matte, kind: alpha|alphaInverted|luma|lumaInverted}` uses any layer as the matte; the matte layer stops rendering on its own. Example: footage seen through text is `{layer: "footage", matte: "Title", kind: "alpha"}`.
- **Blend modes**: `layer.setBlendMode {layers, mode}` with the After Effects mode names (Normal, Multiply, Screen, Overlay, Add, Soft Light, Color Dodge, Difference...).
- **Precompose**: `layer.precompose {layers, name, mode: move|leave}` returns `{comp, layer}`: the new composition and the layer that now holds it.
- **Switches**: `layer.setSwitch {layers, switch, value}` with `video` (visible), `audio`, `solo`, `lock`, `shy`, `collapse` (continuous rasterization for vector layers), `quality`, `fx`, `frameBlend`, `motionBlur`, `adjustment`, `threeD`, `guide`, `preserveTransparency`.
- **Solid settings**: `layer.settings {layer, name, color, width, height}` changes a solid's colour or size.
- **Labels and comments**: `edit.label {label: Red|Yellow|Aqua|..., layers}`, `layer.setComment {layers, comment}`.

## Check

`get_comp` after structural changes (order, parents, mattes), and `render_frame` to see that mattes and blend modes produce what was intended.
