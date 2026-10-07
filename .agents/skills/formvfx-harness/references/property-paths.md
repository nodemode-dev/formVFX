# formvfx property paths

Generated from the formvfx engine by `./formvfx agents sync`; do not edit by hand.

Every animatable value is a property with a path in its layer's tree. Use the path with `set_property`, `set_keyframes`, `get_property`, `prop.set`, `prop.addKey` and `prop.setExpression`. The lists below are the trees of fresh layers made by the commands shown, with their default values. Each segment of a path can be written three ways: the match name shown here (`masks/mask/feather`, a second one is `mask#2`), the 1-based index among its siblings (`masks/#2/feather`, `effects/#1/blurriness`, `text/animators/#1/selectors/#1/start`), or the display name (`masks/Mask 2/feather`, `effects/Gaussian Blur/blurriness`). `get_layer` on the real layer is always the authority.

## Contents

Solid with an effect and a mask · Text with an animator · Shape layer (rectangle) with Trim Paths and a Repeater · 3D layer · Camera · Spot light · Null

## Solid with an effect and a mask

Made with `layer.newSolid`, `effect.apply`, `layer.addMask`.

- `masks` · Masks (group)
- `masks/mask` · Mask 1 (group)
- `masks/mask/path` · Mask Path · path · ...
- `masks/mask/feather` · Mask Feather · vec2 · [0, 0]
- `masks/mask/opacity` · Mask Opacity · scalar · 100
- `masks/mask/expansion` · Mask Expansion · scalar · 0
- `effects` · Effects (group)
- `effects/#1` · Gaussian Blur (group)
- `effects/#1/blurriness` · Blurriness · scalar · 0
- `effects/#1/dimensions` · Blur Dimensions · enum · 0
- `effects/#1/repeatEdge` · Repeat Edge Pixels · bool · false
- `transform` · Transform (group)
- `transform/anchor` · Anchor Point · vec3 · [960, 540, 0]
- `transform/position` · Position · vec3 · [960, 540, 0]
- `transform/scale` · Scale · vec3 · [100, 100, 100]
- `transform/orientation` · Orientation · vec3 · [0, 0, 0]
- `transform/rotationX` · X Rotation · scalar · 0
- `transform/rotationY` · Y Rotation · scalar · 0
- `transform/rotation` · Rotation · scalar · 0
- `transform/opacity` · Opacity · scalar · 100
- `materialOptions` · Material Options (group)
- `materialOptions/castsShadows` · Casts Shadows · enum · 0
- `materialOptions/lightTransmission` · Light Transmission · scalar · 0
- `materialOptions/acceptsShadows` · Accepts Shadows · enum · 1
- `materialOptions/acceptsLights` · Accepts Lights · bool · true
- `materialOptions/ambient` · Ambient · scalar · 100
- `materialOptions/diffuse` · Diffuse · scalar · 50
- `materialOptions/specularIntensity` · Specular Intensity · scalar · 50
- `materialOptions/specularShininess` · Specular Shininess · scalar · 5
- `materialOptions/metal` · Metal · scalar · 100

## Text with an animator

Made with `layer.newText`, `layer.addTextAnimator`.

- `text` · Text (group)
- `text/sourceText` · Source Text · text · "Title"
- `text/perChar3d` · Per-character 3D · bool · false
- `text/pathOptions` · Path Options (group)
- `text/pathOptions/path` · Path · enum · 0
- `text/pathOptions/reversePath` · Reverse Path · bool · false
- `text/pathOptions/perpendicular` · Perpendicular To Path · bool · true
- `text/pathOptions/forceAlignment` · Force Alignment · bool · false
- `text/pathOptions/firstMargin` · First Margin · scalar · 0
- `text/pathOptions/lastMargin` · Last Margin · scalar · 0
- `text/moreOptions` · More Options (group)
- `text/moreOptions/anchorGrouping` · Anchor Point Grouping · enum · 0
- `text/moreOptions/groupingAlignment` · Grouping Alignment · vec2 · [0, 0]
- `text/moreOptions/fillStroke` · Fill & Stroke · enum · 0
- `text/moreOptions/interCharBlend` · Inter-Character Blending · enum · 0
- `text/animators` · Animators (group)
- `text/animators/animator` · Animator 1 (group)
- `text/animators/animator/selectors` · Selectors (group)
- `text/animators/animator/selectors/rangeSelector` · Range Selector 1 (group)
- `text/animators/animator/selectors/rangeSelector/start` · Start · scalar · 0
- `text/animators/animator/selectors/rangeSelector/end` · End · scalar · 100
- `text/animators/animator/selectors/rangeSelector/offset` · Offset · scalar · 0
- `text/animators/animator/selectors/rangeSelector/advanced` · Advanced (group)
- `text/animators/animator/selectors/rangeSelector/advanced/units` · Units · enum · 0
- `text/animators/animator/selectors/rangeSelector/advanced/basedOn` · Based On · enum · 0
- `text/animators/animator/selectors/rangeSelector/advanced/mode` · Mode · enum · 0
- `text/animators/animator/selectors/rangeSelector/advanced/amount` · Amount · scalar · 100
- `text/animators/animator/selectors/rangeSelector/advanced/shape` · Shape · enum · 0
- `text/animators/animator/selectors/rangeSelector/advanced/smoothness` · Smoothness · scalar · 100
- `text/animators/animator/selectors/rangeSelector/advanced/easeHigh` · Ease High · scalar · 0
- `text/animators/animator/selectors/rangeSelector/advanced/easeLow` · Ease Low · scalar · 0
- `text/animators/animator/selectors/rangeSelector/advanced/randomize` · Randomize Order · bool · false
- `text/animators/animator/selectors/rangeSelector/advanced/randomSeed` · Random Seed · scalar · 0
- `text/animators/animator/properties` · Properties (group)
- `text/animators/animator/properties/position` · Position · vec3 · [0, 0, 0]
- `text/animators/animator/properties/opacity` · Opacity · scalar · 100
- `masks` · Masks (group)
- `effects` · Effects (group)
- `transform` · Transform (group)
- `transform/anchor` · Anchor Point · vec3 · [0, 0, 0]
- `transform/position` · Position · vec3 · [960, 540, 0]
- `transform/scale` · Scale · vec3 · [100, 100, 100]
- `transform/orientation` · Orientation · vec3 · [0, 0, 0]
- `transform/rotationX` · X Rotation · scalar · 0
- `transform/rotationY` · Y Rotation · scalar · 0
- `transform/rotation` · Rotation · scalar · 0
- `transform/opacity` · Opacity · scalar · 100
- `materialOptions` · Material Options (group)
- `materialOptions/castsShadows` · Casts Shadows · enum · 0
- `materialOptions/lightTransmission` · Light Transmission · scalar · 0
- `materialOptions/acceptsShadows` · Accepts Shadows · enum · 1
- `materialOptions/acceptsLights` · Accepts Lights · bool · true
- `materialOptions/ambient` · Ambient · scalar · 100
- `materialOptions/diffuse` · Diffuse · scalar · 50
- `materialOptions/specularIntensity` · Specular Intensity · scalar · 50
- `materialOptions/specularShininess` · Specular Shininess · scalar · 5
- `materialOptions/metal` · Metal · scalar · 100

## Shape layer (rectangle) with Trim Paths and a Repeater

Made with `layer.newShape`, `layer.addShapeItem`, `layer.addShapeItem`.

- `contents` · Contents (group)
- `contents/group` · Rectangle 1 (group)
- `contents/group/blend` · Blend Mode · enum · 0
- `contents/group/contents` · Contents (group)
- `contents/group/contents/rect` · Rectangle Path 1 (group)
- `contents/group/contents/rect/direction` · Path Direction · enum · 0
- `contents/group/contents/rect/size` · Size · vec2 · [300, 200]
- `contents/group/contents/rect/position` · Position · vec2 · [0, 0]
- `contents/group/contents/rect/roundness` · Roundness · scalar · 0
- `contents/group/contents/fill` · Fill 1 (group)
- `contents/group/contents/fill/blend` · Blend Mode · enum · 0
- `contents/group/contents/fill/composite` · Composite · enum · 0
- `contents/group/contents/fill/rule` · Fill Rule · enum · 0
- `contents/group/contents/fill/color` · Color · color · [0.25, 0.55, 1, 1]
- `contents/group/contents/fill/opacity` · Opacity · scalar · 100
- `contents/group/transform` · Transform (group)
- `contents/group/transform/anchor` · Anchor Point · vec2 · [0, 0]
- `contents/group/transform/position` · Position · vec2 · [0, 0]
- `contents/group/transform/scale` · Scale · vec2 · [100, 100]
- `contents/group/transform/skew` · Skew · scalar · 0
- `contents/group/transform/skewAxis` · Skew Axis · scalar · 0
- `contents/group/transform/rotation` · Rotation · scalar · 0
- `contents/group/transform/opacity` · Opacity · scalar · 100
- `contents/trim` · Trim Paths 1 (group)
- `contents/trim/start` · Start · scalar · 0
- `contents/trim/end` · End · scalar · 100
- `contents/trim/offset` · Offset · scalar · 0
- `contents/trim/mode` · Trim Multiple Shapes · enum · 0
- `contents/repeater` · Repeater 1 (group)
- `contents/repeater/copies` · Copies · scalar · 3
- `contents/repeater/offset` · Offset · scalar · 0
- `contents/repeater/composite` · Composite · enum · 0
- `contents/repeater/transform` · Transform (group)
- `contents/repeater/transform/anchor` · Anchor Point · vec2 · [0, 0]
- `contents/repeater/transform/position` · Position · vec2 · [100, 0]
- `contents/repeater/transform/scale` · Scale · vec2 · [100, 100]
- `contents/repeater/transform/rotation` · Rotation · scalar · 0
- `contents/repeater/transform/startOpacity` · Start Opacity · scalar · 100
- `contents/repeater/transform/endOpacity` · End Opacity · scalar · 100
- `masks` · Masks (group)
- `effects` · Effects (group)
- `transform` · Transform (group)
- `transform/anchor` · Anchor Point · vec3 · [0, 0, 0]
- `transform/position` · Position · vec3 · [960, 540, 0]
- `transform/scale` · Scale · vec3 · [100, 100, 100]
- `transform/orientation` · Orientation · vec3 · [0, 0, 0]
- `transform/rotationX` · X Rotation · scalar · 0
- `transform/rotationY` · Y Rotation · scalar · 0
- `transform/rotation` · Rotation · scalar · 0
- `transform/opacity` · Opacity · scalar · 100
- `materialOptions` · Material Options (group)
- `materialOptions/castsShadows` · Casts Shadows · enum · 0
- `materialOptions/lightTransmission` · Light Transmission · scalar · 0
- `materialOptions/acceptsShadows` · Accepts Shadows · enum · 1
- `materialOptions/acceptsLights` · Accepts Lights · bool · true
- `materialOptions/ambient` · Ambient · scalar · 100
- `materialOptions/diffuse` · Diffuse · scalar · 50
- `materialOptions/specularIntensity` · Specular Intensity · scalar · 50
- `materialOptions/specularShininess` · Specular Shininess · scalar · 5
- `materialOptions/metal` · Metal · scalar · 100

## 3D layer

Made with `layer.newSolid`, `layer.setSwitch`.

- `masks` · Masks (group)
- `effects` · Effects (group)
- `transform` · Transform (group)
- `transform/anchor` · Anchor Point · vec3 · [400, 225, 0]
- `transform/position` · Position · vec3 · [960, 540, 0]
- `transform/scale` · Scale · vec3 · [100, 100, 100]
- `transform/orientation` · Orientation · vec3 · [0, 0, 0]
- `transform/rotationX` · X Rotation · scalar · 0
- `transform/rotationY` · Y Rotation · scalar · 0
- `transform/rotation` · Rotation · scalar · 0
- `transform/opacity` · Opacity · scalar · 100
- `materialOptions` · Material Options (group)
- `materialOptions/castsShadows` · Casts Shadows · enum · 0
- `materialOptions/lightTransmission` · Light Transmission · scalar · 0
- `materialOptions/acceptsShadows` · Accepts Shadows · enum · 1
- `materialOptions/acceptsLights` · Accepts Lights · bool · true
- `materialOptions/ambient` · Ambient · scalar · 100
- `materialOptions/diffuse` · Diffuse · scalar · 50
- `materialOptions/specularIntensity` · Specular Intensity · scalar · 50
- `materialOptions/specularShininess` · Specular Shininess · scalar · 5
- `materialOptions/metal` · Metal · scalar · 100

## Camera

Made with `layer.newCamera`.

- `transform` · Transform (group)
- `transform/poi` · Point of Interest · vec3 · [960, 540, 0]
- `transform/position` · Position · vec3 · [960, 540, -2666.67]
- `transform/orientation` · Orientation · vec3 · [0, 0, 0]
- `transform/rotationX` · X Rotation · scalar · 0
- `transform/rotationY` · Y Rotation · scalar · 0
- `transform/rotation` · Z Rotation · scalar · 0
- `cameraOptions` · Camera Options (group)
- `cameraOptions/zoom` · Zoom · scalar · 1866.67
- `cameraOptions/dof` · Depth of Field · bool · false
- `cameraOptions/focusDistance` · Focus Distance · scalar · 1866.67
- `cameraOptions/aperture` · Aperture · scalar · 25.3
- `cameraOptions/blurLevel` · Blur Level · scalar · 100
- `cameraOptions/irisShape` · Iris Shape · enum · 0
- `cameraOptions/irisRotation` · Iris Rotation · scalar · 0
- `cameraOptions/irisRoundness` · Iris Roundness · scalar · 0
- `cameraOptions/irisAspectRatio` · Iris Aspect Ratio · scalar · 1
- `cameraOptions/irisDiffractionFringe` · Iris Diffraction Fringe · scalar · 0
- `cameraOptions/highlightGain` · Highlight Gain · scalar · 0
- `cameraOptions/highlightThreshold` · Highlight Threshold · scalar · 255
- `cameraOptions/highlightSaturation` · Highlight Saturation · scalar · 0

## Spot light

Made with `layer.newLight`.

- `transform` · Transform (group)
- `transform/poi` · Point of Interest · vec3 · [960, 540, 0]
- `transform/position` · Position · vec3 · [700, 280, -440]
- `transform/orientation` · Orientation · vec3 · [0, 0, 0]
- `transform/rotationX` · X Rotation · scalar · 0
- `transform/rotationY` · Y Rotation · scalar · 0
- `transform/rotation` · Z Rotation · scalar · 0
- `lightOptions` · Light Options (group)
- `lightOptions/intensity` · Intensity · scalar · 100
- `lightOptions/color` · Color · color · [1, 1, 1, 1]
- `lightOptions/coneAngle` · Cone Angle · scalar · 90
- `lightOptions/coneFeather` · Cone Feather · scalar · 50
- `lightOptions/falloff` · Falloff · enum · 0
- `lightOptions/radius` · Radius · scalar · 500
- `lightOptions/falloffDistance` · Falloff Distance · scalar · 500
- `lightOptions/castsShadows` · Casts Shadows · bool · false
- `lightOptions/shadowDarkness` · Shadow Darkness · scalar · 100
- `lightOptions/shadowDiffusion` · Shadow Diffusion · scalar · 0

## Null

Made with `layer.newNull`.

- `effects` · Effects (group)
- `transform` · Transform (group)
- `transform/anchor` · Anchor Point · vec3 · [50, 50, 0]
- `transform/position` · Position · vec3 · [960, 540, 0]
- `transform/scale` · Scale · vec3 · [100, 100, 100]
- `transform/orientation` · Orientation · vec3 · [0, 0, 0]
- `transform/rotationX` · X Rotation · scalar · 0
- `transform/rotationY` · Y Rotation · scalar · 0
- `transform/rotation` · Rotation · scalar · 0
- `transform/opacity` · Opacity · scalar · 100
