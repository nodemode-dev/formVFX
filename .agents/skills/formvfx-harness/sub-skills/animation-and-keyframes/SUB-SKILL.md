---
name: animation-and-keyframes
description: Keyframe animation in formvfx. Use to animate any property, choose easing, shape speed with influence and velocity as in the Graph Editor, build motion paths and auto-orient, stagger or sequence layers, loop, hold, wiggle or smooth keyframes, record motion sketches, or remap and stretch time.
---

# animation-and-keyframes

## The main tool

`set_keyframes {layer, path, keys: [{time, value, ease}]}` writes all the keys of one property in one undo step. Times are composition seconds, snapped to frames. `ease` per key:

| ease | Result |
| --- | --- |
| `linear` | constant speed through the key |
| `easyEase` | eases in and out (F9) |
| `easyEaseIn` / `easyEaseOut` | eases only arriving / only leaving |
| `hold` | the value jumps at the next key |
| `bezier`, `autoBezier` | smooth, with the default influence |

Example, a title rising into place while fading in:

```json
{"layer": "Title", "path": "transform/position", "keys": [{"time": 0.4, "value": [960, 620, 0], "ease": "easyEase"}, {"time": 1.1, "value": [960, 560, 0], "ease": "easyEase"}]}
{"layer": "Title", "path": "transform/opacity", "keys": [{"time": 0.4, "value": 0}, {"time": 0.8, "value": 100}]}
```

`set_property` on a property that already has keys writes a key at `time` (default: the current time); on a static property it changes the value.

## Speed and influence (the Graph Editor)

The raw keyframe commands use **layer seconds** (equal to composition seconds when the layer starts at 0).

| Need | Command |
| --- | --- |
| Influence or speed on one side of one key | `keys.setEase {layer, path, time, side: in|out, influence (%), speed}` |
| Strong ease out of the first key (a snappy start) | `keys.setEase {... time: first, side: out, influence: 80}` |
| Select keys | `keys.select {keys: [{layer, path, time}]}`, `keys.selectAll {layers}` |
| Act on the selected keys | `keys.easyEase {which: both|in|out}`, `keys.interpolation {interpolation: linear|bezier|continuousBezier|autoBezier|hold}`, `keys.velocity {inSpeed, inInfluence, outSpeed, outInfluence}`, `keys.move {delta}`, `keys.delete`, `keys.copy` then `keys.paste {layers, path, time}` |
| Scale or offset keys in time or value | `keys.transform {timeScale, timeAnchor, timeOffset, valueScale, valueOffset}` |
| Read keys with their ease | `keys.info {}` on the selection, or `get_property` |

Influence 33% is the default ease. 60 to 85% on the arriving side gives the long, soft landing of modern UI motion; a strong ease out of the first key with a strong ease into the last gives a confident move.

## Motion paths

- Keyframing `transform/position` draws a motion path. Curves come from spatial interpolation: `keys.interpolation {spatial: linear|bezier|continuousBezier|autoBezier}` on the selected keys, or exact tangents with `keys.setSpatialTangents {layer, path, time, in: [dx, dy], out: [dx, dy]}`.
- `layer.autoOrient {layers, mode: alongPath}` turns the layer to follow its path.
- `paths.tracePath {layer, path}` makes a null that travels along a mask or shape path (it has a Progress slider to key).
- `motion.sketch {layer, points: [[t, x, y], ...]}` turns a recorded pointer path into position keys.

## Rhythm across layers

- Stagger: offset each layer's keys by a constant (0.03 to 0.08 s reads as a cascade). Either key each layer with shifted times, or key one and move the others' timing with `layer.timing {layers, delta}`.
- Back to back: `layer.sequence {layers, overlap, duration, transition}`.
- Repeat forever: an expression on the property, `loopOut("cycle")` or `loopOut("pingpong")` (see `expressions-and-scripting`).

## Organic motion

- `keys.wiggle {frequency, magnitude, noise: smooth|jagged, dimensions: one|same|independent}` adds random keys between the selected keys.
- `keys.smooth {tolerance}` thins hand-made or tracked keys.
- For continuous wiggle without keys, use the expression `wiggle(freq, amount)`.

## Time

- `layer.enableTimeRemap {layers, value: true}` adds `timeRemap` keys at the layer's start and end; key `timeRemap` (seconds of source time) to freeze, slow, reverse or speed footage and precomps.
- `layer.timeStretch {layers, percent}`, `layer.timeReverse {layers}`, `layer.freezeFrame {layers}`.
- `comp.setSwitch {switch: frameBlending}` with `layer.frameBlending {layers, mode: frameMix|pixelMotion}` smooths retimed footage.

## Check

`contact_sheet {start, end, count}` across each animated span: positions should progress smoothly with ease visible as tighter spacing near the keys. For timing questions, `get_property` lists the keys with times and values.
