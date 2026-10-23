---
name: compositions-and-layers
description: Compositions and layers in formvfx. Use to create or change a composition (size, frame rate, duration, background, work area), add solids, text, shapes, nulls, adjustment layers or footage, import files, arrange and time layers, parent them, set track mattes, blend modes and switches, duplicate, split, delete or precompose layers, or build, use, export and import Template Controls templates.
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
| Trim to the work area | `comp.trimToWorkArea` |
| Crop the frame | `comp.cropToRegionOfInterest {comp, rect: [x, y, w, h]}` or `comp.cropToLayerBounds {comp, layers}`; layers keep their place on screen |

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

`import_files {paths: [absolute paths], addToComp}` imports video, image sequences, stills, audio, PSD, `.ai` and PDF files, SVG, Lottie and 3D models. With `addToComp: true` each becomes a layer named after its file. The raw command is `file.import {paths, importAs: footage|composition|compositionLayerSizes}`; a layered PSD file imported `as composition` becomes a composition with one layer per PSD layer.

Stand-ins and swaps: `file.importPlaceholder {name, width, height, frameRate, duration}` makes footage that shows colour bars until `file.replaceFootage {item, path}` gives it a file; `file.replaceWithPlaceholder`, `file.replaceWithSolid {item, color}` and `file.replaceWithLayeredComp {item}` (PSD, PDF, `.ai`) swap an item under every layer that uses it. `file.rememberInterpretation {item}` then `file.applyInterpretation {items}` copies frame rate, alpha, loops and colour profile. `project.duplicate`, `project.setLabel {items, label}`, `project.setProjectComment`, `comp.setPosterTime` (the frame thumbnails and templates show).

Trimming before editing in: `footage.open {item}`, `footage.setIn {time}` and `footage.setOut {time}` (source seconds), then `footage.rippleInsertEdit {at}` (layers crossing `at` are split and everything after moves later by the clip's length; the composition grows if needed) or `footage.overlayEdit {at}` (on top, nothing moves). Both take `in` and `out` directly too. Finding media: `mediaBrowser.go {path}` lists a folder with numbered stills grouped as one `sequence` entry; `mediaBrowser.import {paths}` takes files, a sequence's first frame or a whole folder. `footage.check {wait: true}` marks footage whose files are gone.

## Arrange and time

| Need | Command |
| --- | --- |
| Stacking order | `layer.arrange {layers, to: front|forward|backward|back}` or `{layers, above: layer}` |
| In, out and start | `layer.timing {layers, in, out, start}`; `{layers, delta}` shifts by seconds |
| One after another | `layer.sequence {layers (in order), overlap, duration, transition: off|dissolveFront|crossDissolve}` |
| Speed | `layer.timeStretch {layers, percent}` (200 is half speed), `layer.timeReverse {layers}` |
| Rename | `layer.rename {layer, name}` |
| Align or distribute in the frame | `layer.align {edge: left|hcenter|right|top|vcenter|bottom, to: composition|selection, layers}`, `layer.distribute {mode, layers}` |
| Centre, fit, flip | `layer.transform {layers, op: reset|center|fit|fitWidth|fitHeight|flipH|flipV}`; `layer.centerAnchor {layers}` puts the anchor at the content's centre without moving it |
| Slip footage or a precomp under fixed in and out points | `layer.slip {layers, delta}` (seconds) or `{frames}` |
| Cut the work area out | `comp.workArea {start, end}`, then `edit.liftWorkArea {layers}` (leaves a gap) or `edit.extractWorkArea {layers}` (closes it) |
| Cuts in a clip | `layer.sceneEditDetection {layer, mode: markers|split|splitPrecompose, wait: true}` |
| Line a clip up with a data file's clock | `layer.alignVideoToData {layer, data (CSV or JSON), videoStart}` |
| Linked copies | `edit.copyWithPropertyLinks` (or `edit.copyWithRelativePropertyLinks`) then `edit.paste`: the copy's properties follow the original by expression |

## Selection-based edits

`edit.duplicate`, `edit.splitLayer`, `edit.clear` (delete), `edit.copy` and `edit.paste` act on the selection, as in the menus. Select first, in the same batch:

```json
{"steps": [{"command": "layer.select", "params": {"layers": [345]}}, {"command": "edit.duplicate"}]}
```

`edit.duplicate` returns the new layer ids. To delete layers: `layer.select` then `edit.clear`.

## Structure

- **Parenting**: `layer.setParent {layers, parent}` (`parent: null` to clear). The child keeps its place on screen and then follows the parent's position, rotation and scale. Nulls are the usual parents.
- **Track mattes**: `layer.setTrackMatte {layer, matte, kind: alpha|alphaInverted|luma|lumaInverted}` uses any layer as the matte; the matte layer stops rendering on its own. Example: footage seen through text is `{layer: "footage", matte: "Title", kind: "alpha"}`.
- **Blend modes**: `layer.setBlendMode {layers, mode}` with the standard mode names (Normal, Multiply, Screen, Overlay, Add, Soft Light, Color Dodge, Difference...).
- **Precompose**: `layer.precompose {layers, name, mode: move|leave}` returns `{comp, layer}`: the new composition and the layer that now holds it.
- **Switches**: `layer.setSwitch {layers, switch, value}` with `video` (visible), `audio`, `solo`, `lock`, `shy`, `collapse` (continuous rasterization for vector layers), `quality`, `fx`, `frameBlend`, `motionBlur`, `adjustment`, `threeD`, `guide`, `preserveTransparency`.
- **Solid settings**: `layer.settings {layer, name, color, width, height}` changes a solid's colour or size.
- **Labels and comments**: `edit.label {label: Red|Yellow|Aqua|..., layers}`; `project.setComment {items, comment}` on project items.

## Template Controls templates

A template is a composition with some of its properties exposed, so each use of it can be changed without opening it (a lower third whose name, colour and bar width differ per shot).

1. In the template composition: `essential.canAdd {comp, layer, path}` says whether a property can be exposed; `essential.addProperty {comp, layer, path, name}` for each one (`text/sourceText`, `effects/#1/color`, `transform/opacity`...). `essential.addMedia {comp, layer}` lets each use swap a footage or precomp layer's source (a photo or logo slot). `essential.addMirror {comp, controller}` shows a control twice; `essential.linkProperty {comp, controller, layer, path}` makes another property follow a control. `essential.addComment` and `essential.addGroup` organise the panel; `essential.rename`, `essential.move`, `essential.remove`; `essential.list {comp}` shows the controllers.
2. Use it: add the composition to other compositions (`layer.addItem {item}`). Each of those layers gets a Primary Properties group. `essential.set {comp, layer, controller, value}` overrides one instance (a media slot takes an item name or id); `essential.instance {comp, layer}` lists what is overridden; `essential.revert {layers}` drops the overrides; `essential.pushToComp {comp, layer}` makes one instance's overrides the template's own values.
3. Share it: `essential.exportTemplate {comp, path}` writes a `.fvxtemplate`, or a `.mogrt` when the path ends in `.mogrt` (the composition, everything it uses, its media and a poster frame); `essential.importTemplate {path, addToComp: true}` brings one into another project; `essential.installTemplate` puts it in the Template Controls panel's Browse tab. `essential.templateInfo {path}` reads any `.mogrt`: one that carries an `.aep` project opens (`opens: true`) through that project, and `essential.importTemplate` builds it with its controls linked; one without a project lists its controls but does not open. A bare `.aep` project imports with `file.importAep {path}`, and an `.ffx` preset applies with `anim.applyPreset {path, layers}`. Read the `warnings` either returns: they name what was not carried across (masks, time remapping, effects formvfx does not have).

## Check

`get_comp` after structural changes (order, parents, mattes), and `render_frame` to see that mattes and blend modes produce what was intended.
