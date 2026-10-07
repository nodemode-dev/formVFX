---
name: text-and-shapes
description: Type and vector graphics in formvfx. Use for text layers (point or paragraph text, fonts, size, tracking, leading, fill, stroke, justification), per-character animation with text animators, selectors and the built-in text presets, text on a path, shape layers (rectangles, ellipses, stars, polygons, free paths), fills and strokes, Trim Paths write-ons, repeaters and the other shape operators, and converting text to shapes or masks.
---

# text-and-shapes

## Text layers

- Create: `layer.newText {text, name, font, style, size, fill, stroke, strokeWidth, tracking, leading, justify: left|center|right, position}`. `box: [x, y, w, h]` makes paragraph text that wraps inside the box.
- Restyle: `layer.setText {layer, text?, font?, style?, size?, fill?, stroke?, strokeWidth?, tracking?, leading?, justify?, allCaps?, smallCaps?, fauxBold?, fauxItalic?, baselineShift?, ...}`. `range: [start, end]` styles only those characters (indices into the text).
- Fonts: `execute text.fonts {query}` lists the families and styles available. Inter, JetBrains Mono and Noto Serif are always bundled; system fonts are listed when present. Use a family name exactly as listed.
- The text value is the property `text/sourceText`; key it with `set_keyframes` (hold keys) to change words over time.
- Position is the baseline at the anchor: left-justified text grows to the right of its position, centred text around it. Default position is the composition centre.
- Text on a path: add a mask to the text layer with mode None so it guides without cutting (`layer.addMask {layer, shape: ellipse, rect: [-r, -r, 2r, 2r], mode: "None"}`; a text layer's own space has its origin at the text's anchor point), then set `text/pathOptions/path` to the mask's number (0 is None, 1 the first mask). `text/pathOptions/firstMargin` slides the text along the path, `reversePath` flips its direction, `perpendicular` stands the characters on it.

## Per-character animation

1. `layer.addTextAnimator {layer, properties: [...]}` with any of `position, scale, rotation, opacity, fillColor, strokeColor, tracking, blur, skew, anchor, characterOffset, lineSpacing, perChar3d...`. It returns the animator, whose paths are `text/animators/#1/...`.
2. Set the animator's property values to the **offset** state, for example `text/animators/#1/properties/opacity` to 0 and `text/animators/#1/properties/position` to `[0, 40, 0]`.
3. Key the range selector: `text/animators/#1/selectors/#1/offset` from 0 to 100 (or `start` / `end`) over the reveal. Characters inside the range get the offset values; as the range moves, characters settle.
4. Shape the cascade with `text/animators/#1/selectors/#1/advanced/shape` (0 Square, 1 Ramp Up, 2 Ramp Down, 3 Triangle, 4 Round, 5 Smooth), `easeHigh`, `easeLow`, `basedOn` (characters, words, lines) and `randomize`.
5. More selectors: `layer.addTextSelector {layer, animator, kind: range|wiggly|expression}`.

Built-in presets do all of this in one command: `layer.applyTextPreset {layer, preset: typewriter|fadeUpCharacters|bounceInWords|trackingIn|scramble|blurIn|jitter|dropInLines}`. They key from the layer's in point; move the keys or the layer to retime.

## Shape layers

- Parametric: `layer.newShape {kind: rect|rounded|ellipse|star|polygon, name, size: [w, h], position, fill, stroke, strokeWidth}`. The shape's own group is `contents/group`, its path `contents/group/contents/rect` (`size`, `roundness`, `position`), its fill `contents/group/contents/fill/color`.
- Free path: `shape.newPath {vertices: [[x, y], ...], inTangents, outTangents, closed, fill (false for none), stroke, strokeWidth}` in composition pixels. The stroke is `contents/group/contents/stroke` (`color`, `width`, `opacity`).
- Add items and operators: `layer.addShapeItem {layer, kind, group?}` returns `{uid, path}`. Kinds: `group, rect, ellipse, star, polygon, fill, stroke, gfill` (gradient fill), `trim, repeater, round` (round corners), `offset, pucker, twist, zigzag, wiggle, merge`. Without `group` the item goes at the top level of `contents`, where operators act on every group above them.
- Dashes and tapers: `shape.dashes.add {layer}`, `shape.stroke.taper {layer, startWidth, endWidth}`, `shape.stroke.wave {layer, amount, wavelength}`.

## Patterns

- **Write-on line or outline**: a stroked path, `layer.addShapeItem {kind: trim}`, then key `contents/trim/end` from 0 to 100 with easy ease. Key `start` a little later to make the line travel and disappear.
- **Wipe a filled bar on**: Trim Paths on a filled rectangle trims the fill along the outline; for a clean left-to-right wipe, animate `contents/group/contents/rect/size` from `[0, h]` with the group's anchor at the left edge, or scale the layer from 0 on X with `transform/anchor` at the left edge.
- **Radial burst or dotted row**: one small shape plus a Repeater; set `contents/repeater/copies` and `contents/repeater/transform/position` (row) or `.../transform/rotation` with the anchor offset (ring). Key `contents/repeater/offset` to make it march.
- **Logo outline to fill**: `layer.create {op: shapesFromText}` or import the vector logo, stroke it, write it on with Trim Paths, then fade the fill in.

## Check

Type must be read on screen: `render_frame` at the moment each line is fully in, and check that nothing is cut by the frame edge or overlapping. For per-character work use `contact_sheet` across the reveal.
