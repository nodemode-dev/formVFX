---
name: verification
description: Proving formvfx work is right before reporting it. Use after any visual change, before saying a piece is done, when a result looks wrong, or when the person asks whether something worked; covers render_frame, contact_sheet, property reads, keyframe reads, expression errors, render output checks and the common failure patterns.
---

# verification

Tools report what they did; only pixels show what it looks like. Every claim about how a composition looks or moves rests on a frame you rendered and looked at in this session.

## The checks

| Question | Check |
| --- | --- |
| Does this moment look right? | `render_frame {time, max_size: 960}` |
| Does the motion read? | `contact_sheet {start, end, count: 6 to 12}` across the span |
| Is the alpha right (mattes, keys, masks)? | `render_frame {transparent: true}` |
| What value does a property have at a time? | `get_property {layer, path, time}`; with an expression, `evaluated` is the result |
| Where are the keys and how do they ease? | `get_property` lists keys; `keys.select` then `keys.info` shows influence and speed |
| What is in the composition? | `get_comp` (order, timing, parents, mattes, switches) |
| Do expressions run? | `execute expr.errors {}` must report `count: 0` |
| Is background work finished? | `wait`, then `status` (`busy` empty) |
| Did the render land? | the `render` result's output path and frame count; the file exists |

## A pass before saying done

1. `expr.errors` is empty.
2. `contact_sheet` of the whole work area at 8 to 12 frames.
3. `render_frame` at each beat's settled moment, at full width when text must be legible.
4. For 3D: the start, middle and end of every camera move.
5. The render result matches the requested format, size and length.

## Reading a frame

Look for: text cut by the frame edge or overlapping other text; elements at the wrong layer order; a layer missing because it is outside its in and out points, invisible (`video` switch off), behind the camera, or used as a track matte; colours that are flat because a glow or grade is too strong; motion that pops because a key is `hold` or linear where an ease was intended.

## When a frame is wrong

- Nothing visible: `get_comp` (is the layer in range and on?), then `get_layer` (opacity 0, scale 0, z scale 0 on a newly 3D layer, a mask in Subtract).
- Wrong place: positions are composition pixels from the top-left; text sits on its baseline; parented layers use the parent's space.
- Effect does nothing: `get_layer` to check the effect is enabled and its parameters; point parameters are fractions of the layer, not pixels.
- An edit reverted: an expression on the property overrides its keys and value; read `expression` in `get_property`.

Fix, then render the same moment again and compare.
