---
name: effects-and-presets
description: Effects in formvfx. Use to find, apply, set, animate, reorder, copy or remove any of the 300 effects (blur, colour correction, distort, generate, keying, matte, noise, perspective, simulation, stylize, time, transition, the 3D Channel effects on EXR render passes, expression controls) or a WebAssembly plug-in, add layer styles such as drop shadow or stroke, grade colour, pull a key, or save and apply animation presets.
---

# effects-and-presets

## Find

- `list_effects {query}` searches names, ids and categories; add `detail: true` for each parameter's id, default and control. `references/effects.md` is the whole catalogue with parameter ids.
- Effect ids are `fx.[category].[name]`: `fx.blur.gaussian`, `fx.stylize.glow`, `fx.color.curves`, `fx.generate.gradientRamp`, `fx.keying.keylight`, `fx.control.slider`. The After Effects display name works too (`"Gaussian Blur"`).

## Apply and set

`apply_effect {layer, effect, params: {paramId: value}}` adds the effect at the bottom of the layer's stack and sets the parameters in one undo step. It returns the effect's path (`effects/#2`). A wrong parameter id still applies the effect and returns `paramErrors` with the `validParams`.

Afterwards each parameter is a property: `effects/#2/radius`, or `effects/Glow/radius`. Nested parameters keep their group: `effects/#1/basic/exposure` on Lumetri Color, `effects/#1/screenMatte/clipBlack` on Keylight. Animate them with `set_keyframes` like any property.

Values to know:

- Point parameters are fractions of the layer: `[0.5, 0.5]` is the centre.
- Menus take the 0-based option index (the catalogue lists the options in order).
- Colours take `"#rrggbb"`.
- Levels and similar input and output values run from 0 to 1, not 0 to 255.

## Manage

| Need | Command |
| --- | --- |
| Turn an effect off or on | `effect.toggle {layer, effect, enabled}` |
| Order (the stack renders top to bottom) | `effect.reorder {layer, effect, index}` (1-based) |
| Reset, remove | `effect.reset`, `effect.remove {layer, effect}`; `effect.removeAll {layers}` |
| Same effect on many layers | `effect.apply {effect, layers: [...]}` |
| Move effects between layers | `effect.copy {layer, effects}`, then `effect.paste {layers}`; `effect.duplicate {layer, effect}`; `effect.applyLast {layers}` repeats the last applied effect |
| A colour from the picture | `effect.pickColor {layer, effect, param, x, y, average}` |

## Curl Noise

`fx.noise.curlNoise` (Curl Noise Generator, Noise and Grain) is After Effects' Curl Noise: it generates swirling flow-line noise. Input Noise Internal (Speed and Direction move it) or This Layer (the layer's luminance is the field; on an adjustment layer that is everything below, which is how After Effects' Other Layer input is done here). Shape it with Scale, Swirl, Density, Smoothness and Vertical Bias, the lines with Sample Count, Sample Radius, Flow Softness and Edge Definition; View shows Input Noise, Curl Generation or Final Render, and Channel writes RGB, one colour channel or Alpha. Evolution, Turbulence Speed and Noise Speed move the flow over time; key Evolution for full control.

## Layer styles

`layer.style.add {layers, style: dropShadow|innerShadow|outerGlow|innerGlow|bevelEmboss|satin|colorOverlay|gradientOverlay|stroke}`. The style's properties appear under `layerStyles/` in `get_layer`; set them with `set_property`. `layer.style.remove`, `layer.style.toggle`, `layer.style.list`.

## 3D Channel effects (render passes)

They read the extra channels of OpenEXR footage from a 3D application: depth (`Z`, `depth.Z`), object and material ids (`objectId`, `IndexOB`, `materialId`), normals (`N.X/Y/Z`), UVs, coverage and Cryptomatte layers. Apply them to the EXR footage layer itself.

| Need | Effect and the parameters that matter |
| --- | --- |
| See a pass | 3D Channel Extract `fx.3d.channelExtract`: `channel` (0 Z-Depth, 1 Object ID, 2 UV, 3 Normals, 4 Coverage, 5 Background RGB, 6 Unclamped RGB, 7 Material ID), `blackPoint`, `whitePoint` |
| Cut by depth | Depth Matte `fx.3d.depthMatte`: `depth`, `feather`, `invert` (on keeps what is nearer) |
| Defocus by depth | Depth of Field `fx.3d.depthOfField`: `focalPlane` (in Z units), `maxRadius`, `thickness`, `bias` |
| Atmosphere | Fog 3D `fx.3d.fog3d`: `start`, `end`, `fogColor`, `opacity`, `density`, `foggyBackground`, `gradientLayer` with `contribution` |
| Isolate an object | ID Matte `fx.3d.idMatte`: `auxChannel` (0 object, 1 material), `idSelection`, `useCoverage`, `feather` |
| Any channel as RGBA | EXtractoR `fx.3d.extractor`: `red`, `green`, `blue`, `alpha` (channel names as in the file), `blackPoint`, `whitePoint` |
| Cryptomatte | Cryptomatte `fx.3d.cryptomatte`: `selection` (comma-separated names from the file's manifest, or `<hex>` ids), `display` (0 colours, 1 edges, 2 matte), `matteOnly`, `layer` (the Cryptomatte layer name when the file has several) |

## Plug-ins

WebAssembly effect plug-ins load from the Plug-ins folder at start or with `effect.plugins.load {path, install: true}`; `effect.plugins.list` names them with their parameters and any load or render errors. A loaded plug-in is an effect like any other (`effect.apply {effect: "Its Name"}`, its parameters under `effects/#n/`). `docs/PLUGINS.md` in the repository describes how to write one.

## Patterns

- **Glow on text or shapes**: Glow with `threshold` 40 to 70, `radius` 20 to 60, `intensity` 0.8 to 1.5. On a dark background a second, wider Glow at low intensity reads as bloom.
- **Background**: a full-frame solid with Gradient Ramp (`start`, `startColor`, `end`, `endColor`, `shape` 1 for radial) or 4-Color Gradient, then Noise at a low amount to break up banding.
- **Grade** the whole piece with an adjustment layer on top holding Lumetri Color, Curves, Levels or Hue/Saturation.
- **Depth**: Camera Lens Blur or Gaussian Blur on background layers, Drop Shadow (`fx.perspective.dropShadow`) for separation.
- **Keying green screen**: Keylight (`fx.keying.keylight`) with `screenColour` set to the background's colour (read it from a `render_frame`), then Key Cleaner and Advanced Spill Suppressor; check the matte with `render_frame {transparent: true}`.
- **Organic textures**: Fractal Noise with `evolution` keyed or driven by `time * 90`, Turbulent Displace for wobble.
- **Transitions**: Linear Wipe (`completion` 0 to 100), Venetian Blinds, Block Dissolve, keyed over the transition span.
- **Controls for expressions**: Slider, Angle, Checkbox, Color, Point and Layer Control effects (`fx.control.*`) give a rig a named knob; expressions read them as `effect("Slider Control")("Slider")`.

## Animation presets

`anim.savePreset {path, name}` saves the selected properties and effects; `anim.applyPreset {path, layers}` applies a saved preset. Text has built-in presets (see `text-and-shapes`).

## Check

`render_frame` after applying and after setting; effects that sample neighbours (blurs, glows) also change near the frame edges, so look at the whole frame, not only the subject.
