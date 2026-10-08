---
name: camera-lights-3d
description: 3D in formvfx. Use to make layers 3D, build scenes with cameras (one-node or two-node, presets, depth of field), lights (spot, point, parallel, ambient, environment) and shadows, set material options, add 3D primitives or imported glTF and OBJ models, animate camera moves such as push-ins, orbits and fly-throughs, switch 3D views, and use per-character 3D text.
---

# camera-lights-3d

## Make it 3D

- `layer.setSwitch {layers, switch: threeD, value: true}`. A 3D layer has `transform/position` and `transform/anchor` with a z, `transform/orientation` and `transform/rotationX`, `rotationY`, `rotation` (z).
- +z points away from the viewer. A layer at z = 0 sits on the composition plane; negative z comes toward the camera.
- Text layers can be 3D per character: `layer.enablePerChar3D {layer, enabled: true}`, then text animators gain 3D position and rotation.
- `comp.renderer {renderer: advanced3d|classic3d}` picks the 3D renderer; Advanced 3D adds environment lights, reflections and models.

## Camera

- `layer.newCamera {name, type: oneNode|twoNode, preset: 15mm|20mm|24mm|28mm|35mm|50mm|80mm|135mm|200mm, dof, focusDistance, aperture, position, poi}`.
- A two-node camera aims at its point of interest `transform/poi`; a one-node camera aims with its orientation and rotation. The default camera sits at `[w/2, h/2, -zoom]` looking at the centre, which frames the composition exactly as 2D.
- Depth of field: `cameraOptions/dof` true, `cameraOptions/focusDistance` (pixels from the camera), `cameraOptions/aperture` or `blurLevel`. `camera.linkFocusToLayer {camera, layer}` keeps a layer in focus.
- Settings in one call: `layer.cameraSettings {layer, preset, zoom, dof, focusDistance, aperture, position, poi}`.

Camera moves are keyframes on `transform/position` and `transform/poi`:

| Move | Keys |
| --- | --- |
| Push in | position z from -2666 to -1600, poi fixed |
| Truck | position and poi x change together |
| Orbit | parent the camera to a null at the subject (`camera.orbitNull {layer}` makes one) and key the null's `transform/rotationY` |
| Fly-through | position along z through layers spaced in depth, poi leading it |

Ease every camera key; linear camera moves read as mechanical.

## Lights

- `layer.newLight {kind: Spot|Point|Parallel|Ambient, color, intensity, coneAngle, coneFeather, falloff: None|Smooth|Inverse Square Clamped, radius, falloffDistance, castsShadows, shadowDarkness, shadowDiffusion, position, poi}`.
- Properties: `lightOptions/intensity`, `lightOptions/color`, `lightOptions/coneAngle`, `lightOptions/castsShadows`, `lightOptions/shadowDarkness`, `lightOptions/shadowDiffusion`.
- Shadows need three things: a light with `castsShadows`, a layer whose `materialOptions/castsShadows` is 1 (On; 2 is Only), and a layer that accepts them (`materialOptions/acceptsShadows` 1, the default).
- A scene with only spot lights is dark outside the cones; add an Ambient light at 20 to 30% to lift the shadows.
- Environment lighting (Advanced 3D): an Environment light with a footage source, `light.environmentBackground` shows it behind the scene.

## Materials

`material.set {layers, castsShadows: off|on|only, acceptsShadows, acceptsLights, ambient, diffuse, specularIntensity, specularShininess, metal, baseColor, metallic, roughness}`. The same values are under `materialOptions/` on each 3D layer.

## Primitives and models

- `layer.new3dPrimitive {kind: cube|sphere|plane|torus|cone|cylinder, width, height, depth, radius, position: [x, y, z], baseColor, metallic, roughness, emissive}`.
- `layer.newModel {path}` imports a `.gltf`, `.glb` or `.obj` and adds it as a layer; `camera.fromModel` and `light.fromModel` bring in the model's own cameras and lights.

## Views

The renders always use the active camera. In the window, `execute view.set3DView {view: activeCamera|front|left|top|back|right|bottom|custom1|custom2|custom3}` changes the viewer's view, and `camera.orbit`, `camera.pan`, `camera.dolly` move that view without keyframing. `view.get3D` reports the current view and the active camera.

## A dependable scene

```json
{"steps": [
  {"command": "comp.new", "params": {"name": "Scene", "width": 1920, "height": 1080, "frameRate": 30, "duration": 6, "background": "#0d1016"}},
  {"command": "layer.newSolid", "params": {"name": "Floor", "color": "#262e3a", "width": 4000, "height": 4000}},
  {"command": "layer.newSolid", "params": {"name": "Card", "color": "#f97316", "width": 640, "height": 400}},
  {"command": "layer.setSwitch", "params": {"layers": ["$2.layer", "$3.layer"], "switch": "threeD", "value": true}},
  {"command": "layer.newCamera", "params": {"preset": "35mm"}},
  {"command": "layer.newLight", "params": {"kind": "Spot", "intensity": 140, "castsShadows": true}},
  {"command": "layer.newLight", "params": {"kind": "Ambient", "intensity": 25}}
]}
```

Then `transform/orientation` `[90, 0, 0]` and a y below the card on the floor, `materialOptions/castsShadows` 1 on the card, and keys on the card or the camera.

## Check

Render frames at the start, middle and end of every camera move; perspective errors (a layer behind the camera, a floor seen edge-on, clipping) only show from the camera's point of view. To check where layers sit in space, `render_frame {view: "top"}` (or `front`, `right`) renders an orthographic view without moving the camera; the person can set the same views side by side with `view.layout {views: 2|4}`.
