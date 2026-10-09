# formvfx property paths

Generated from the formvfx engine by `./formvfx agents sync`; do not edit by hand.

Every animatable value is a property with a path in its layer's tree. Use the path with `set_property`, `set_keyframes`, `get_property`, `prop.set`, `prop.addKey` and `prop.setExpression`. The lists below are the trees of fresh layers made by the commands shown, with their default values. Each segment of a path can be written three ways: the match name shown here (`masks/mask/feather`, a second one is `mask#2`), the 1-based index among its siblings (`masks/#2/feather`, `effects/#1/blurriness`, `text/animators/#1/selectors/#1/start`), or the display name (`masks/Mask 2/feather`, `effects/Gaussian Blur/blurriness`). `get_layer` on the real layer is always the authority.

## Contents

Solid with an effect and a mask · Text with an animator · Shape layer (rectangle) with Trim Paths and a Repeater · 3D layer · Camera · Spot light · Null

## Solid with an effect and a mask

Made with `layer.newSolid`, `effect.apply`, `layer.addMask`.


## Text with an animator

Made with `layer.newText`, `layer.addTextAnimator`.


## Shape layer (rectangle) with Trim Paths and a Repeater

Made with `layer.newShape`, `layer.addShapeItem`, `layer.addShapeItem`.


## 3D layer

Made with `layer.newSolid`, `layer.setSwitch`.


## Camera

Made with `layer.newCamera`.


## Spot light

Made with `layer.newLight`.


## Null

Made with `layer.newNull`.

