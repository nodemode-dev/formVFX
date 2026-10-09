---
name: effects-and-presets
description: Effects in formvfx. Use to find, apply, set, animate, reorder, copy or remove any of the 289 effects (blur, colour correction, distort, generate, keying, matte, noise, perspective, simulation, stylize, time, transition, expression controls), add layer styles such as drop shadow or stroke, grade colour, pull a key, or save and apply animation presets.
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

## Curl Noise

`fx.noise.curlNoise` (Curl Noise Generator, Noise and Grain) is After Effects' Curl Noise: it generates swirling flow-line noise. Input Noise Internal (Speed and Direction move it) or This Layer (the layer's luminance is the field; on an adjustment layer that is everything below, which is how After Effects' Other Layer input is done here). Shape it with Scale, Swirl, Density, Smoothness and Vertical Bias, the lines with Sample Count, Sample Radius, Flow Softness and Edge Definition; View shows Input Noise, Curl Generation or Final Render, and Channel writes RGB, one colour channel or Alpha. Evolution, Turbulence Speed and Noise Speed move the flow over time; key Evolution for full control.

## Layer styles

`layer.style.add {layers, style: dropShadow|innerShadow|outerGlow|innerGlow|bevelEmboss|satin|colorOverlay|gradientOverlay|stroke}`. The style's properties appear under `layerStyles/` in `get_layer`; set them with `set_property`. `layer.style.remove`, `layer.style.toggle`, `layer.style.list`.

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
