---
name: tracking-and-stabilizing
description: Motion tracking in formvfx. Use to point-track footage and attach text, graphics or nulls to it, track position, rotation and scale or corner-pin a screen, sign or wall with the planar tracker, stabilize shaky footage with Warp Stabilizer or a stabilize track, solve a 3D camera from footage and place layers in the solved scene, track masks, or track faces.
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
- Screen replacement: use the planar tracker below. A `kind: perspective` point track (four points on the screen corners, analyze, target, apply) works when all four corners stay visible and distinct.
- People run the same steps from the Tracker panel (`ui {action: panel, panel: tracker}`), which offers Track Camera, Warp Stabilize, Track Motion, Stabilize Motion; Motion Source, Current Track, Track Type, Position / Rotation / Scale, Motion Target, Options, Analyze (one frame or all, either way), Stop, Reset, Apply. With a mask on the source selected, the panel tracks the mask instead (Mask Method shows). Point the person at it when they want to adjust a track by hand.

## Planar tracking (screens, signs, walls, tabletops)

For a flat surface prefer the planar tracker over four point tracks: it follows the whole textured area inside a mask, survives corners leaving the frame or being covered, and has no drift on long shots.

1. `time.set` to a frame where the surface is clear, then draw a mask around it on the footage: `mask.new {layer, vertices: [[x, y], ...]}` in layer pixels. Four vertices on the surface's corners make those corners the pinned surface.
2. `planar_track {layer, track: "both", target: "Insert", reference: t}`. It tracks the mask with a perspective solve both ways from the reference frame, re-aligns every frame to the reference (so error does not accumulate), sets the mask to mode None so the footage stays whole, and keys a Corner Pin (after a Grow Bounds) on the target so its corners ride the surface. `surface` overrides the corners when the mask is not the surface itself (four points: upper left, upper right, lower left, lower right).
   The target's own frame is what gets pinned, so use footage, an image, a solid or a precomp sized to the insert; precompose text and shape layers first (their layer frame is the whole comp).
3. The result is also a Perspective tracker on the footage (`Planar Tracker 1`): re-apply it to another layer with `track.setTarget` then `track.apply`.

Check: `contact_sheet` across the shot with the insert visible; the insert's edges should sit on the surface's edges in every cell.

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
- Face tracking is best with the learned model: `face.models` says whether the MediaPipe Face Landmarker is installed; `face.model.download` fetches it once (3.8 MB, verified). With it the mask becomes the 36-point face oval and the Face Track Points sit on a 478-point face mesh that holds through head turns and tilts; without it a classical skin and feature detector is used. Draw the starting mask roughly around the head on the first frame.

## Check

`get_property` on the target's `transform/position` lists the tracked keys; compare the last key with where the feature ends. Then `contact_sheet` over the clip with the attached graphic visible: drift shows as the graphic sliding off the feature.
