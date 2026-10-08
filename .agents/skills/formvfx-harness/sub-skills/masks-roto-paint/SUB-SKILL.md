---
name: masks-roto-paint
description: Isolating and drawing on layers in formvfx. Use for masks (rectangles, ellipses, free shapes, feather, expansion, opacity, modes, inverting, animating the mask path), Roto Brush segmentation and propagation, one-click Object Matte, paint, clone and eraser strokes with write-on, Puppet pins, Liquify, Auto-trace and Content-Aware Fill for removing objects from footage.
---

# masks-roto-paint

## Masks

| Need | Command |
| --- | --- |
| Rectangle or ellipse | `layer.addMask {layer, shape: rect|ellipse, rect: [x, y, w, h], mode}` |
| Any shape | `mask.new {layer, vertices: [[x, y], ...], inTangents, outTangents, closed, mode}` |
| Feather, opacity, expansion | `set_property` on `masks/#1/feather` (`[x, y]` pixels), `masks/#1/opacity`, `masks/#1/expansion`, or `layer.mask.set {layer, mask, field: feather|opacity|expansion, value}` |
| Mode | `layer.mask.mode {layer, mask, mode: None|Add|Subtract|Intersect|Lighten|Darken|Difference}` |
| Invert, lock, remove | `layer.mask.invert`, `layer.mask.lock`, `layer.mask.remove`, `layer.mask.removeAll` |
| Edit vertices | `mask.setVertex`, `mask.addVertex`, `mask.moveVertices {vertices, delta}`, `mask.convertVertex`, `path.freeTransform {layer, mask, scale, rotation, offset}` |
| Variable feather | `mask.featherPoint.add {layer, mask, segment, t, radius}` |

Coordinates are the layer's own pixels: for a full-frame solid that equals composition pixels; for text the origin is the text's anchor point; for a shape layer, its anchor.

**Animate a mask** by keying `masks/#1/path` with `set_keyframes`. A path value is:

```json
{"closed": true, "vertices": [[100, 100], [900, 100], [900, 600], [100, 600]], "in_tangents": [[0, 0], [0, 0], [0, 0], [0, 0]], "out_tangents": [[0, 0], [0, 0], [0, 0], [0, 0]]}
```

Tangents are relative to their vertex. Keep the same vertex count on every key; `mask.interpolate` (Smart Mask Interpolation) can fill in keys between two shapes.

A mask with mode `None` changes nothing by itself and is how a path is given to Stroke, text on a path or other effects that read a mask.

## Roto Brush

For separating a subject from footage:

1. `roto.stroke {layer, kind: fg, points: [[x, y], ...], frame}` paints over the subject on one frame (layer pixels). `kind: bg` subtracts.
2. `roto.propagate {layer, direction: forward|backward|both, wait: true}` carries the segmentation through the span.
3. Refine with more strokes on frames where it drifted, then `roto.freeze {layer, wait: true}` to lock the result.
4. `roto.status {layer, frame}` reports progress and can return the matte; `roto.options {view: alphaOverlay}` changes what the window shows.

Roto Brush 2 and 3 use a model chosen in Settings (`roto.models`, `roto.model.select`).

## Object Matte (one click)

For one whole object (a person, a car, a product), skip the strokes: `object_matte {action: "preview", layer, point: [x, y], time}` shows what one point on it selects (bounding box, coverage, the model's confidence) without changing anything; move the point if the box is wrong. Then `object_matte {action: "select", layer, point, time, wait: true}` seeds a Roto Brush at that point, spans the whole layer and propagates both ways; the layer's alpha is the object. It needs the MobileSAM model (`roto.models` shows it; `roto.model.download {id: mobilesam, wait: true}` installs it, about 40 MB). Correct a drifted frame with `roto.stroke` as above. People do the same with the Object Matte tool (Alt+W cycles to it): hover shows the object tinted, click selects it.

## Paint

`paint.stroke {layer, kind: brush|clone|eraser, points: [[x, y, pressure?], ...], color: [r, g, b, a], diameter, hardness, opacity, durationMode: constant|writeOn|singleFrame|custom, duration, time}` in layer space. `durationMode: writeOn` with `duration` draws the stroke on over that many seconds, which is the quick way to a hand-drawn reveal. Clone strokes take `cloneSource`, `clonePosition` and `cloneTimeShift`. Strokes are listed under the layer's Paint effect; `paint.removeStroke` deletes one.

## Puppet

1. `puppet.addPin {layer, position: [x, y]}` (layer space) for each pin; the first pin creates the mesh. `kind: starch|bend|advanced|overlap` for the other pin types.
2. `puppet.movePin {layer, pin, position, time}` keys a pin's position at a time; animate several pins to make the layer bend and walk.
3. `puppet.mesh {layer, density, expansion}` adjusts the mesh; `puppet.info {layer, time}` reads pins and the deformed mesh.
4. `puppet.recordPin` records a drag; `puppet.follow` makes pins trail a leader.

## Liquify, Auto-trace, Content-Aware Fill

- `liquify.stroke {layer, tool: warp|turbulence|twirlClockwise|pucker|bloat|..., points, size, pressure, time}` after applying the Liquify effect.
- `layer.autoTrace {layer, channel: alpha|luminance, threshold, timeSpan}` turns a layer's alpha or luma into masks.
- Object removal: mask the object on the footage layer (mode Subtract or an animated mask), then `layer.newContentAwareFill {layer, method: object|surface|edgeBlend, range: workArea, wait: true}`. It renders a fill layer above the footage.

## Check

`render_frame {transparent: true}` shows the alpha a mask or matte leaves. For Roto Brush and Content-Aware Fill, check several frames across the span with `contact_sheet`, because drift shows up over time, not on the frame you stroked.
