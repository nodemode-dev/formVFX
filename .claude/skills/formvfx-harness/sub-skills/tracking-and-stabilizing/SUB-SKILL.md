---
name: tracking-and-stabilizing
description: Motion tracking in formvfx. Use to point-track footage and attach text, graphics or nulls to it, track position, rotation and scale or corner-pin a screen with a perspective track, stabilize shaky footage with Warp Stabilizer or a stabilize track, solve a 3D camera from footage and place layers in the solved scene, track masks, or track faces.
---

# tracking-and-stabilizing

All analyses run in the background. Pass `wait: true` (the call returns when the analysis is done) or call the `wait` tool before reading results. Long clips take time; tell the person when an analysis is running.

## Point tracking (Tracker panel)

Verified sequence, attaching a null to a moving feature:

```json
{"command": "track.new", "params": {"layer": "clip.mp4", "kind": "transform"}}
{"command": "track.setPoint", "params": {"layer": "clip.mp4", "point": 1, "center": [200, 270], "featureSize": [60, 60], "searchSize": [160, 160]}}
{"command": "track.analyze", "params": {"layer": "clip.mp4", "direction": "forward", "wait": true}}
{"command": "layer.newNull", "params": {"name": "Follower"}}
{"command": "track.setTarget", "params": {"layer": "clip.mp4", "target": "Follower"}}
{"command": "track.apply", "params": {"layer": "clip.mp4"}}
```

- `kind`: `transform` (position, and with `rotation` / `scale` true, two points for rotation and scale), `stabilize` (moves the footage to cancel the motion), `affine` (three points), `perspective` (four corners, a corner pin onto the target), `raw` (data only).
- Choose the feature on the frame where the track starts (`time.set` first): a high-contrast corner or dot. `featureSize` should hug the feature; `searchSize` must cover the farthest it moves between frames.
- `track.status {layer}` shows the points and how many frames have keys. `track.options {layer, channel: rgb|luminance|saturation, adaptEveryFrame, threshold, action: continue|stop|extrapolate|adapt}` tunes a track that drifts.
- `track.apply` writes position (and rotation, scale) keys on the target, one per frame. Parent graphics to that null rather than applying to them directly, so they can be offset and animated on top of the track.
- Screen replacement: `kind: perspective`, set four points on the screen corners, analyze, target the insert layer, apply.

## Stabilizing

- Warp Stabilizer: `track.warpStabilizer {layer, wait: true}` applies the effect and analyzes. Its settings (smoothness, method, framing) are properties under the effect (`get_layer`).
- Point stabilize: `track.new {layer, kind: stabilize}`, set the point, analyze, `track.apply` (the layer itself moves).

## 3D camera tracking

1. `track.camera {layer, shotType: fixed|variable, solveMethod: auto|typical|flat|tripod, wait: true}` applies the 3D Camera Tracker and solves.
2. `camera.solveStatus {layer}` reports the solve. `camera.points {layer, time}` lists solved points visible at a time.
3. `camera.selectPoints {points}` then `camera.createFromSolve {kind: text|solid|null|shadowCatcher|camera}` places a layer on those points and creates the solved camera. `camera.setGroundPlane {points}` orients the scene.

## Masks and faces

- `track.mask {layer, mask, method: position|positionScale|positionScaleRotation|positionScaleRotationSkew|perspective|faceOutline|faceDetailed, direction, wait: true}` keys the mask path to follow the footage.
- Face tracking uses `method: faceOutline` or `faceDetailed`; `track.extractFaceMeasurements {layer}` turns the detailed points into keys on a Face Measurements effect for expressions.

## Check

`get_property` on the target's `transform/position` lists the tracked keys; compare the last key with where the feature ends. Then `contact_sheet` over the clip with the attached graphic visible: drift shows as the graphic sliding off the feature.
