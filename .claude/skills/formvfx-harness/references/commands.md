# formvfx commands

Generated from the formvfx engine by `./formvfx agents sync`; do not edit by hand.

508 commands in 45 namespaces. Every menu item, button and drag in formvfx is one of these. Run one with `execute {command, params}` over MCP, inside `batch` for several as one undo step, or `formvfx exec ID 'JSON'` from a shell. `describe_command ID` returns the JSON Schema. In the parameter summaries `?` marks an optional parameter, `a|b` lists the accepted values, and `layers?`/`layer?` default to the selected layers. Times are composition seconds unless the summary says layer seconds.

## Contents

`file` (34) `edit` (14) `project` (8) `item` (1) `comp` (17) `layer` (91) `prop` (15) `keys` (33) `anim` (7) `time` (8) `markers` (5) `effect` (12) `mask` (15) `path` (1) `shape` (6) `text` (15) `camera` (18) `light` (3) `material` (4) `track` (22) `warp` (3) `roto` (14) `paint` (6) `puppet` (10) `liquify` (2) `contentFill` (2) `face` (5) `essential` (14) `expr` (1) `script` (1) `scriptui` (4) `render` (3) `renderQueue` (22) `playback` (3) `view` (52) `window` (6) `cache` (2) `prefs` (3) `shortcuts` (7) `app` (12) `engine` (1) `command` (2) `motion` (1) `clipboard` (1) `color` (2)

## file

- `file.newProject` New Project · File > New · Cmd+Alt+N
- `file.open` Open Project · File · Cmd+O
  `{path (a .fvxproj, or an .ecproj to import)}`
- `file.openDemoProject` Open Demo Project · Help
- `file.closeProject` Close Project · File
- `file.save` Save · File · Cmd+S
  `{path? (.fvxproj; needed the first time)}`
- `file.saveAs` Save As · File > Save As · Cmd+Shift+S
  `{path (.fvxproj)}`
- `file.saveCopy` Save a Copy · File > Save As
  `{path (.fvxproj)}`
- `file.incrementAndSave` Increment and Save · File · Cmd+Alt+Shift+S
- `file.revert` Revert · File
- `file.import` File · File > Import · Cmd+I
  `{paths: [string] (absolute paths: video, stills, image sequences, EXR, PSD, PDF, Illustrator, SVG, audio, Lottie or a project to bring in), importAs?: footage|composition|compositionLayerSizes (Photoshop, PDF and Illustrator files), layer? (a Photoshop or EXR layer name or index: footage of that layer), page?: number from 1 (PDF / Illustrator page), sequence?: bool (numbered stills as one sequence, default true), sequenceFrameRate?: fps (default 30), addToComp?: bool, time? (s), position? [x, y]}`
- `file.importMultiple` Multiple Files · File > Import · Cmd+Alt+I
  `{paths: [string], importAs?: footage|composition|compositionLayerSizes, layer? (a Photoshop or EXR layer name or index), page?: number from 1, sequence?: bool, sequenceFrameRate?: fps}`
- `file.projectSettings` Project Settings · File · Cmd+Alt+Shift+K
  `{bitDepth?: 8|16|32, timeDisplay?: timecode|frames, frameStart?, audioRate?, colorEngine?: adobe|ocio, ocioConfig? (.ocio path, empty for the built-in ACES config), workingSpace? (none, srgb, rec709, rec2020, p3, acescg ... or an OCIO colour space), linearize?: bool, blendLinear?: bool, display?, view? (OCIO), hdr?: clip|toneMap, outputSpace?, renderer?: gpu|cpu}`
- `file.cycleBitDepth` Cycle Bit Depth
- `file.interpretFootage` Interpret Footage · File > Interpret Footage · Cmd+Alt+G
  `{items?: [id], frameRate?, alpha?: straight|premultiplied|ignore, loops?, pixelAspect?, colorProfile? (auto, a profile id or an OCIO colour space), preserveRgb?: bool}`
- `file.replaceFootage` Replace Footage · File > Replace Footage · Cmd+H
  `{path, items?: [id]}`
- `file.reloadFootage` Reload Footage · File · Cmd+Alt+L
  `{items?: [id]}`
- `file.newCompFromSelection` New Comp from Selection · File
  `{items?: [id]}`
- `file.importSolid` Solid · File > Import
  `{name?, color?, width?, height?}`
- `file.importLottie` Lottie · File > Import
  `{path (.json or .lottie)} -> a composition with shape, solid, image, text, null and precomp layers, keyframes and easing`
- `file.exportLottie` Lottie JSON · File > Export
  `{comp?, path (.json or .lottie), includeExpressions?: bool (otherwise expressions are baked to keyframes), textAsShapes?: bool}`
- `file.setProxy` File · File > Set Proxy
  `{path (a still, sequence or movie), items?|item? (default: the selected footage or compositions)}`
- `file.setProxyNone` None · File > Set Proxy
  `{items?|item?}`
- `file.useProxy` Use Proxy · File
  `{items?|item?, on?: bool (toggles)}`
- `file.interpretProxy` Proxy · File > Interpret Footage
  `{items?|item?, alpha?: straight|premultiplied|ignore, frameRate?, loop?, pixelAspect?, colorProfile?, preserveRgb?}`
- `file.createProxy` Create Proxy · File > Create Proxy
  `{kind: still|movie, comp?|item? (default: the selected item), path?|output?, resolution? (default 0.5), time? (s, stills)}`
- `file.runScript` Run Script File · File > Scripts
  `{path (.jsx or .js) | name (an installed script)}`
- `file.installScript` Install Script File · File > Scripts
  `{path (.jsx or .js)} · copies it into the Scripts folder so it appears in File > Scripts`
- `file.installScriptUIPanel` Install ScriptUI Panel · File > Scripts
  `{path (.jsx or .js)} · copies it into ScriptUI Panels so it appears in the Window menu`
- `file.scripts.list` Installed Scripts
- `file.collectFiles` Collect Files · File > Dependencies
  `{folder}`
- `file.consolidateFootage` Consolidate All Footage · File > Dependencies
- `file.removeUnusedFootage` Remove Unused Footage · File > Dependencies
- `file.reduceProject` Reduce Project · File > Dependencies
  `{} (keeps the selected comps and what they use)`
- `file.findMissing` Find Missing
  `{what: footage|effects|fonts}`

## edit

- `edit.copyExpressionOnly` Copy Expression Only · Edit
- `edit.undo` Undo · Edit · Cmd+Z
- `edit.redo` Redo · Edit · Cmd+Shift+Z
- `edit.history.list` History
- `edit.history.goto` Go to History State
  `{index? | steps?}`
- `edit.cut` Cut · Edit · Cmd+X
  `{layers?}`
- `edit.copy` Copy · Edit · Cmd+C
  `{layers?}`
- `edit.paste` Paste · Edit · Cmd+V
- `edit.clear` Clear · Edit · Delete
  `{layers?}`
- `edit.duplicate` Duplicate · Edit · Cmd+D
  `{layers?}`
- `edit.selectAll` Select All · Edit · Cmd+A
- `edit.deselectAll` Deselect All · Edit · Cmd+Shift+A
- `edit.label` Label
  `{layers?|items?, label: None|Red|Yellow|... or 0-16}`
- `edit.splitLayer` Split Layer
  `{layers?, time?}`

## project

- `project.summary` Project Summary
- `project.select` Select Project Items
  `{items: [id|name], add?: bool}`
- `project.rename` Rename Item
  `{item: id|name, name}`
- `project.delete` Delete Items
  `{items?: [id|name] (default: selected)}`
- `project.newFolder` New Folder · File > New · Cmd+Alt+Shift+N
  `{name?}`
- `project.move` Move to Folder
  `{items?: [id|name], folder: id|name|null}`
- `project.setComment` Set Comment
  `{items?: [id], comment}`
- `project.usage` Item Usage
  `{items?: [id]}`

## item

- `item.metadata` Metadata
  `{item?: id|name}`

## comp

- `comp.new` New Composition · Composition · Cmd+N
  `{name?, width?, height?, frameRate?, duration? (s), background? [r,g,b]|#hex, pixelAspect?, startTime? (s), shutterAngle?, shutterPhase?, motionBlurSamples?, open?: bool}`
- `comp.open` Open Composition
  `{comp: id|name}`
- `comp.close` Close Composition
  `{comp?}`
- `comp.settings` Composition Settings · Composition
  `{comp?, name?, width?, height?, anchor? 0-8 (4 = centre), frameRate?, duration?, background?, pixelAspect?, startTime?, shutterAngle?, shutterPhase?, motionBlurSamples?, preserveFrameRate?: bool, preserveResolution?: bool}`
- `comp.info` Composition Info
  `{comp?}`
- `comp.workArea` Set Work Area
  `{start?, end?, set?: begin|end, merge?}`
- `comp.setSwitch` Composition Switch
  `{switch: motionBlur|frameBlending|draft3d|hideShy, value?: bool}`
- `comp.duplicate` Duplicate Composition · Edit
  `{comp?}`
- `comp.addMarker` Add Marker · Layer
  `{comp?, time?, comment?, duration?}`
- `comp.saveFrameAs` File · Composition > Save Frame As · Cmd+Alt+S
  `{path (.png|.jpg|.tif|.exr; with queue: an output path or template), time?, scale?, queue?: bool (add a Render Queue item from the still templates instead of writing now)}`
- `comp.saveFrameAsPsd` Photoshop Layers · Composition > Save Frame As
  `{path (.psd), comp?, time?, scale?} -> a layered PSD: one layer per visible comp layer (blend mode, opacity) and the merged frame`
- `comp.saveFrameAsExr` Layered EXR · Composition > Save Frame As
  `{path (.exr), comp?, time?, scale?, float?: bool (32-bit float, default half)} -> a multi-layer OpenEXR: the composite R, G, B, A and `[layer].R/G/B/A` per layer (scene-linear, premultiplied)`
- `comp.flowchart` Composition Flowchart · Composition · Cmd+Shift+F11
- `comp.renderer` Renderer
  `{renderer?: advanced3d|classic3d (omit to read)}`
- `comp.trimToWorkArea` Trim Comp to Work Area · Composition · Cmd+Shift+X
  `{comp?}`
- `comp.responsiveTime` Responsive Design
  `{op: intro|outro|workArea}`
- `comp.miniFlowchart` Composition Mini-Flowchart · Composition · Tab

## layer

- `layer.newSolid` Solid · Layer > New · Cmd+Y
  `{name?, color? #hex|[r,g,b], width?, height?}`
- `layer.newText` Text · Layer > New · Cmd+Alt+Shift+T
  `{text?, name?, position? [x,y], box? [x,y,w,h], font?, style?, size?, fill?, stroke?, strokeWidth?, applyFill?, applyStroke?, tracking?, leading?, justify?: left|center|right, fauxBold?, fauxItalic?, allCaps?, edit?: bool}`
- `layer.newNull` Null Object · Layer > New · Cmd+Alt+Shift+Y
  `{name?, position? [x,y]}`
- `layer.newShape` Shape Layer · Layer > New
  `{kind?: rect|rounded|ellipse|star|polygon|none, name?, size? [w,h], fill?, stroke?, strokeWidth?, position? [x,y]}`
- `layer.newAdjustment` Adjustment Layer · Layer > New · Cmd+Alt+Y
  `{name?}`
- `layer.newCamera` Camera · Layer > New · Cmd+Alt+Shift+C
  `{name?, focalLength?, zoom?, type?: twoNode|oneNode}`
- `layer.newLight` Light · Layer > New · Cmd+Alt+Shift+L
  `{kind?|type?: Parallel|Spot|Point|Ambient|Environment (default Spot), name?, color?, intensity?, coneAngle?, coneFeather?, falloff?: None|Smooth|Inverse Square Clamped, radius?, falloffDistance?, castsShadows?, shadowDarkness?, shadowDiffusion?, source? (Environment: the layer used as the environment image), environmentRotation?, position? [x,y,z], poi? [x,y,z]}`
- `layer.cameraSettings` Camera Settings · Layer > Camera
  `{layer?, name?, type?: oneNode|twoNode, preset?: 15mm..200mm, zoom?, focalLength? mm, angleOfView? deg, dof?, focusDistance?, aperture?, fStop?, blurLevel?, lockToZoom?, position?, poi?}`
- `layer.lightSettings` Light Settings · Layer > Light
  `{layer?, name?, kind?, color?, intensity?, coneAngle?, coneFeather?, falloff?, radius?, falloffDistance?, castsShadows?, shadowDarkness?, shadowDiffusion?, source?, environmentRotation?, position?, poi?}`
- `layer.newModel` 3D Model Layer
  `{item?: id or name of a 3D model footage item, path?: a .gltf, .glb or .obj file to import, name?, time? (s), index? (1-based stack position), position? ([x, y] comp px)}`
- `layer.new3dPrimitive` 3D Primitive · Layer > New
  `{kind: cube|sphere|plane|torus|cone|cylinder, name?, width?, height?, depth?, radius?, tubeRadius?, segments?, rings?, position? [x,y,z], baseColor?, metallic? (0..1 or %), roughness? (0..1 or %), emissive?, castsShadows?: off|on|only, acceptsShadows?: off|on|only, acceptsLights?}`
- `layer.newPrimitive` New 3D Primitive
  `{kind: cube|sphere|plane|torus|cone|cylinder, ... as layer.new3dPrimitive}`
- `layer.environment` Environment Layer · Layer
  `{layers?, on?} · the layer becomes the environment image for reflections and environment lights and is not drawn`
- `layer.addItem` Add Footage to Comp · File · Cmd+/
  `{item: id|name, time? (s), index? (1-based), position? [x,y], duration? (s, stills)}`
- `layer.select` Select Layers
  `{layers: [id|name|#n], add?: bool, toggle?: bool}`
- `layer.rename` Rename Layer
  `{layer?, name}`
- `layer.setComment` Layer Comment
  `{layers?, comment}`
- `layer.copyToComp` Copy to Composition
  `{layers?, target (comp id or name)} → {comp, layers}`
- `layer.setSwitch` Layer Switch
  `{layers?, switch: video|audio|solo|lock|shy|collapse|quality|fx|frameBlend|motionBlur|adjustment|threeD|guide|preserveTransparency, value?: bool}`
- `layer.setParent` Parent
  `{layers?, parent: layer|null}`
- `layer.setBlendMode` Blending Mode
  `{layers?, mode (Normal, Add, Multiply, Screen, Overlay...)}`
- `layer.setTrackMatte` Track Matte
  `{layer?, matte: layer|null, kind?: alpha|alphaInverted|luma|lumaInverted}`
- `layer.timing` Layer Timing
  `{layers?, op?: moveInToTime|moveOutToTime|trimInToTime|trimOutToTime, delta?, start?, in?, out?, stretch?, merge?}`
- `layer.addMask` New Mask · Layer > Mask · Cmd+Shift+N
  `{layer?, shape?: rect|ellipse, rect? [x,y,w,h] (layer px), path?, mode?: add|subtract|intersect|lighten|darken|difference|none}`
- `layer.setMask` Mask Mode
  `{layer?, mask: uid|index|name, mode?, inverted?: bool, locked?: bool, name?}`
- `layer.setText` Edit Text
  `{layer?, text?, font?, style?, size?, fill?, stroke?, strokeWidth?, applyFill?, applyStroke?, tracking?, leading?, justify?, fauxBold?, fauxItalic?, allCaps?, smallCaps?, baselineShift?, box?}`
- `layer.precompose` Pre-compose · Layer · Cmd+Shift+C
  `{layers?, name?, open?: bool}`
- `layer.arrange` Arrange
  `{layers?, to?: front|forward|backward|back, index?, above?: layer}`
- `layer.bringToFront` Bring Layer to Front · Layer > Arrange · Cmd+Shift+]
  `{layers?}`
- `layer.bringForward` Bring Layer Forward · Layer > Arrange · Cmd+]
  `{layers?}`
- `layer.sendBackward` Send Layer Backward · Layer > Arrange · Cmd+[
  `{layers?}`
- `layer.sendToBack` Send Layer to Back · Layer > Arrange · Cmd+Shift+[
  `{layers?}`
- `layer.align` Align Layers
  `{edge: left|hcenter|right|top|vcenter|bottom, to?: composition|selection, layers?}`
- `layer.distribute` Distribute Layers
  `{mode: left|hcenter|right|top|vcenter|bottom, layers?}`
- `layer.split` Split Layer · Edit · Cmd+Shift+D
  `{layers?, time?}`
- `layer.solidSettings` Solid Settings · Layer · Cmd+Shift+Y
  `{layer?, name?, color?, width?, height?}`
- `layer.autoOrient` Auto-Orient · Layer > Transform · Cmd+Alt+O
  `{layers?, mode: off|alongPath|towardsCamera|towardsPointOfInterest}`
- `layer.tree` Layer Property Tree
  `{layer?, comp?, depth?, time? (comp s)}`
- `layer.addMarker` Add Layer Marker
  `{layer?, time?, comment?}`
- `layer.expressions` Enable/Disable Expressions
  `{layers?, enabled: bool}`
- `layer.revealExpressionErrors` Reveal Expression Errors
  `{comp?}`
- `layer.newContentAwareFill` Content-Aware Fill Layer · Layer > New
  `{layer?, method?: object|surface|edgeBlend, range?: workArea|entire, alphaExpansion? (px), lightingCorrection?: none|subtle|moderate|strong, referenceLayer?, referenceTime? (s), outputDir?, wait?: bool}`
- `layer.mask.featherFalloff` Mask Feather Falloff
  `{layer?, mask?, falloff: smooth|linear}`
- `layer.mask.motionBlur` Mask Motion Blur
  `{layer?, mask?, mode: sameAsLayer|on|off}`
- `layer.addShapeItem` Add Shape Item
  `{layer?, kind: (see shape.add), group? (uid), name?, path?}`
- `layer.create` Create · Layer > Create
  `{op: shapesFromText|masksFromText|shapesFromVector|editableText, layers?}`
- `layer.addTextAnimator` Animate Text · Animation
  `{layer?, properties: [anchor|position|scale|skew|rotation|opacity|transformAll|lineAnchor|lineSpacing|characterOffset|characterValue|blur|fillColor|fillHue|fillSaturation|fillBrightness|fillOpacity|strokeColor|strokeHue|strokeSaturation|strokeBrightness|strokeOpacity|strokeWidth|tracking] | property, name?}`
- `layer.addTextAnimatorProperty` Add Property
  `{layer?, animator?: uid|index, property: (see layer.addTextAnimator)}`
- `layer.addTextSelector` Add Text Selector
  `{layer?, animator?: uid|index, kind: range|wiggly|expression}`
- `layer.enablePerChar3D` Enable Per-character 3D · Animation
  `{layer?, enabled?: bool, toggle?: bool}`
- `layer.applyTextPreset` Apply Text Animation Preset
  `{layer?, layers?, preset: typewriter|fadeUpCharacters|bounceInWords|trackingIn|scramble|blurIn|jitter|dropInLines}`
- `layer.style.add` Add Layer Style
  `{style: dropShadow|innerShadow|outerGlow|innerGlow|bevelEmboss|satin|colorOverlay|gradientOverlay|stroke (or display name), layers?}`
- `layer.style.dropShadow` Drop Shadow · Layer > Layer Styles
  `{layers?}`
- `layer.style.innerShadow` Inner Shadow · Layer > Layer Styles
  `{layers?}`
- `layer.style.outerGlow` Outer Glow · Layer > Layer Styles
  `{layers?}`
- `layer.style.innerGlow` Inner Glow · Layer > Layer Styles
  `{layers?}`
- `layer.style.bevelEmboss` Bevel and Emboss · Layer > Layer Styles
  `{layers?}`
- `layer.style.satin` Satin · Layer > Layer Styles
  `{layers?}`
- `layer.style.colorOverlay` Color Overlay · Layer > Layer Styles
  `{layers?}`
- `layer.style.gradientOverlay` Gradient Overlay · Layer > Layer Styles
  `{layers?}`
- `layer.style.stroke` Stroke · Layer > Layer Styles
  `{layers?}`
- `layer.style.remove` Remove Layer Style
  `{layer?, style: id|name|uid|layerStyles}`
- `layer.style.removeAll` Remove All · Layer > Layer Styles
  `{layers?}`
- `layer.style.toggle` Toggle Layer Style
  `{layer?, style: id|name|uid|layerStyles, value?}`
- `layer.style.showAll` Show All · Layer > Layer Styles
  `{layers?}`
- `layer.style.convertToEditable` Convert to Editable Styles · Layer > Layer Styles
  `{layers?}`
- `layer.style.globalLight` Global Light
  `{comp?, angle? (deg), altitude? (deg, 0..90)} · no values returns the current setting`
- `layer.style.list` List Layer Styles
  `{layer?}`
- `layer.style.options` Layer Style Options · Layer > Layer Styles
  `{layer?, style?: blendingOptions|dropShadow|…}`
- `layer.timeStretch` Time Stretch · Layer > Time
  `{layers?, percent?|duration? (s), hold?: in|current|out, op?: reverse}`
- `layer.timeReverse` Time-Reverse Layer · Layer > Time · Cmd+Alt+R
  `{layers?}`
- `layer.enableTimeRemap` Enable Time Remapping · Layer > Time · Cmd+Alt+T
  `{layers?, value?}`
- `layer.freezeFrame` Freeze Frame · Layer > Time
  `{layers?}`
- `layer.freezeOnLastFrame` Freeze On Last Frame · Layer > Time
  `{layers?}`
- `layer.sequence` Sequence Layers · Animation > Keyframe Assistant
  `{layers? (in order), overlap?: bool, duration? (s), transition?: off|dissolveFront|crossDissolve}`
- `layer.setTransform` Transform Value
  `{layers?, prop: anchor|position|scale|orientation|rotation|opacity, value}`
- `layer.settings` Layer Settings · Layer
  `{layer?, name?, color?, width?, height?, pixelAspect?, affectAll?: bool (default true; false gives this layer its own copy of a solid other layers use)} → {layer, solid?}`
- `layer.revealSource` Reveal Layer Source in Project · Layer > Reveal
  `{layer?}`
- `layer.quality` Quality
  `{layers?, quality: best|draft|wireframe}`
- `layer.frameBlending` Frame Blending
  `{layers?, mode: off|frameMix|pixelMotion} (turns the composition's Frame Blending switch on)`
- `layer.mask.shape` Mask Shape · Layer > Mask · Cmd+Shift+M
  `{layer?, mask?, rect: [x, y, w, h], shape?: rect|ellipse}`
- `layer.mask.set` Mask Settings
  `{layer?, mask?, field: feather|opacity|expansion, value}`
- `layer.mask.reset` Reset Mask · Layer > Mask
  `{layer?, mask?}`
- `layer.mask.remove` Remove Mask · Layer > Mask
  `{layer?, mask?}`
- `layer.mask.removeAll` Remove All Masks · Layer > Mask
  `{layers?}`
- `layer.mask.mode` Mask Mode
  `{layer?, mask?, mode: None|Add|Subtract|Intersect|Lighten|Darken|Difference}`
- `layer.mask.invert` Inverted · Layer > Mask · Cmd+Shift+I
  `{layer?, mask?, value?}`
- `layer.mask.lock` Locked · Layer > Mask
  `{layer?, mask?, value?}`
- `layer.mask.unlockAll` Unlock All Masks · Layer > Mask
  `{layers?}`
- `layer.mask.lockOthers` Lock Other Masks · Layer > Mask
  `{layer?, mask?}`
- `layer.openLayer` Open Layer · Layer
  `{layer?}`

## prop

- `prop.get` Get Property
  `{layer?, path|prop, time? (comp s)}`
- `prop.set` Set Property Value
  `{layer?, path|prop, value, time? (layer s; sets a key when animated), merge?}`
- `prop.addKey` Add Keyframe
  `{layer?, path|prop, time? (layer s), value?}`
- `prop.removeKey` Remove Keyframe
  `{layer?, path|prop, time (layer s)}`
- `prop.toggleKey` Add or Remove Keyframe · Animation
  `{layer?, prop}`
- `prop.toggleAnimation` Toggle Stopwatch
  `{layer?, prop}`
- `prop.reset` Reset
  `{layer?, prop}`
- `prop.select` Select Property
  `{layer?, path|prop, add?: bool, selectKeys?: bool}`
- `prop.setExpression` Expression · Animation
  `{layer?, prop, expression?, enabled?: bool}`
- `prop.pickWhip` Pick Whip
  `{layer?, path|prop, target: {layer, path|prop}} → sets an AE reference expression`
- `prop.convertExpressionToKeyframes` Convert Expression to Keyframes · Animation > Keyframe Assistant
  `{layer?, path|prop}`
- `prop.removeGroup` Remove
  `{layer?, prop (a mask, effect, shape item or animator uid)}`
- `prop.duplicateGroup` Duplicate
  `{layer?, prop}`
- `prop.renameGroup` Rename
  `{layer?, prop, name}`
- `prop.separateDimensions` Separate Dimensions · Animation
  `{layer?, value?}`

## keys

- `keys.select` Select Keyframes
  `{keys: [{layer, prop|path, time (layer s)}], add?: bool, toggle?: bool}`
- `keys.set` Edit Keyframe
  `{layer?, path|prop, time (layer s), newTime?, value?, merge?}`
- `keys.move` Move Keyframes
  `{delta (s), merge?}`
- `keys.transform` Transform Keyframes
  `{timeOffset?, valueOffset?, dim?, merge?}`
- `keys.delete` Delete Keyframes
- `keys.easyEase` Easy Ease · Animation > Keyframe Assistant · F9
  `{which?: both|in|out}`
- `keys.easyEaseIn` Easy Ease In · Animation > Keyframe Assistant · Shift+F9
- `keys.easyEaseOut` Easy Ease Out · Animation > Keyframe Assistant · Cmd+Shift+F9
- `keys.interpolation` Keyframe Interpolation · Animation · Cmd+Alt+K
  `{interpolation?: linear|bezier|continuousBezier|autoBezier|hold, in?, out?, spatial?: linear|autoBezier, roving?: bool}`
- `keys.velocity` Keyframe Velocity · Animation · Cmd+Shift+K
  `{inSpeed?, inInfluence?, outSpeed?, outInfluence?}`
- `keys.setEase` Set Keyframe Ease
  `{layer?, path|prop, time (layer s), side: in|out, dim?, speed?, influence?, merge?}`
- `keys.setSpatialTangents` Set Spatial Tangents
  `{layer?, path|prop, time (layer s), in? [x,y,z], out? [x,y,z], merge?}`
- `keys.info` Keyframe Info
- `keys.toggleHold` Toggle Hold Keyframe · Animation · Cmd+Alt+H
- `keys.copy` Copy Keyframes
- `keys.paste` Paste Keyframes
  `{layers?, prop?|path?, time?}`
- `keys.selectAll` Select All Keyframes · Cmd+Alt+A
  `{layers?, visible?: [{layer, prop}] (every key of these properties)}`
- `keys.nudge` Nudge Keyframes
  `{frames, merge?}`
- `keys.nudgeForward` Move Keyframes 1 Frame Later · Alt+ArrowRight
- `keys.nudgeBackward` Move Keyframes 1 Frame Earlier · Alt+ArrowLeft
- `keys.nudgeForward10` Move Keyframes 10 Frames Later · Alt+Shift+ArrowRight
- `keys.nudgeBackward10` Move Keyframes 10 Frames Earlier · Alt+Shift+ArrowLeft
- `keys.selectEqual` Select Equal Keyframes
- `keys.selectPrevious` Select Previous Keyframes
- `keys.selectFollowing` Select Following Keyframes
- `keys.timeReverse` Time-Reverse Keyframes · Animation > Keyframe Assistant
- `keys.wiggle` Wiggler
  `{apply?: spatial|temporal, noise?: smooth|jagged, dimensions?: one|same|independent, dimension? (index for `one`), frequency? (keys/s, 5), magnitude?, seed?}`
- `keys.smooth` Smoother
  `{tolerance? (property units, 1)}`
- `keys.exponentialScale` Exponential Scale · Animation > Keyframe Assistant
  `{} · two selected Scale keys`
- `keys.setLabel` Keyframe Label
  `{label: name|none|0-16}`
- `keys.selectLabelGroup` Select Keyframe Label Group
  `{scope: selected|all|visibleSelected|visibleAll, visible?: [prop uid]}`
- `keys.audioToKeyframes` Convert Audio to Keyframes · Animation > Keyframe Assistant
  `{comp?}`
- `keys.toggleTransform` Add or Remove Transform Keyframe
  `{layers?, prop: anchor|position|scale|rotation|opacity}`

## anim

- `anim.addKeyframe` Add Keyframe · Animation
  `{} · keys the selected properties at the current time`
- `anim.savePreset` Save Animation Preset · Animation
  `{path, name?} · saves the selected properties, effects and layer styles`
- `anim.applyPreset` Apply Animation Preset · Animation
  `{path | preset (a preset document), layers?}`
- `anim.applyRecentPreset` Recent Animation Presets
  `{index? | path?, layers?}`
- `anim.clearRecentPresets` Clear Recent Presets
- `anim.recentPresets` Recent Animation Preset List
- `anim.browsePresets` Browse Presets · Animation

## time

- `time.set` Go to Time · View · Alt+Shift+J
  `{time? (s) | frame? | timecode?}`
- `time.go` Go To
  `{to: start|end|workStart|workEnd|layerIn|layerOut|nextKey|prevKey, layer?, prop?}`
- `time.nextFrame` Next Frame · PageDown
  `{frames?}`
- `time.previousFrame` Previous Frame · PageUp
  `{frames?}`
- `time.forward10` Forward 10 Frames · Shift+PageDown
- `time.back10` Back 10 Frames · Shift+PageUp
- `time.start` Go to Start · Home
- `time.end` Go to End · End

## markers

- `markers.list` Markers
  `{layer? (omit for composition markers), comp?}`
- `markers.nested` Nested Comp Markers
  `{layer: a precomp layer, comp?}`
- `markers.set` Marker Settings
  `{layer? (omit: comp marker), index? (0-based) | at? (comp s) | new?: true, time? (comp s), duration? (s), comment?, chapter?, url?, frameTarget?, cuePoint?: {name, navigation?, params?: [[name, value]…]} | null, protected?, label?}`
- `markers.delete` Delete Marker
  `{layer?, index? | at? (comp s)}`
- `markers.convert` Convert Marker
  `{layer? (layer marker to comp marker), index? | at?, toLayer? (comp marker to layer marker)}`

## effect

- `effect.editDropdown` Edit Dropdown Menu
  `{layer, path?|prop?, items: [string]}`
- `effect.apply` Apply Effect
  `{effect: id|name (e.g. Gaussian Blur), layers?}`
- `effect.plugins.list` Effect Plug-ins
  `{} -> loaded WebAssembly plug-ins (id, name, category, vendor, version, path, params), load errors and the Plug-ins folder`
- `effect.plugins.load` Load Plug-in
  `{path (.wasm or .wat), install?: bool (copy it into the Plug-ins folder so it loads at every start)} · a WebAssembly effect plug-in (ABI in docs/PLUGINS.md); its effect appears in the Effect menu and Effects & Presets`
- `effect.plugins.reload` Reload Plug-ins
  `{} · loads the Plug-ins folder again`
- `effect.remove` Remove Effect
  `{layer?, effect: uid|index|name}`
- `effect.removeAll` Remove All · Effect · Cmd+Alt+Shift+E
  `{layers?}`
- `effect.reset` Reset Effect
  `{layer?, effect}`
- `effect.toggle` Effect On/Off
  `{layer?, effect, enabled?: bool}`
- `effect.reorder` Reorder Effect
  `{layer?, effect, index (1-based)}`
- `effect.list` Effects
  `{filter?}`
- `effect.manage` Manage Effects · Effect

## mask

- `mask.setVertex` Set Mask Vertex
  `{layer?, mask, index, point?, in?, out?, merge?}`
- `mask.moveVertices` Move Mask Vertices
  `{vertices?: [{layer, mask, index}], delta: [dx,dy], merge?}`
- `mask.selectVertices` Select Vertices
  `{vertices: [{layer, mask, index}], add?: bool}`
- `mask.insertVertex` Add Vertex
  `{layer?, mask, segment, t?: 0..1}`
- `mask.deleteVertices` Delete Vertices
  `{vertices?: [{layer, mask, index}]}`
- `mask.convertVertex` Convert Vertex
  `{layer?, mask, index}`
- `mask.setClosed` Closed
  `{layer?, mask, closed: bool}`
- `mask.featherPoint.add` Add Feather Point
  `{layer?, mask, segment (0-based), t?: 0..1, radius? (px, + outward, - inward), tension? (%)} → {index}`
- `mask.featherPoint.set` Move Feather Point
  `{layer?, mask, index, segment?, t?, radius?, tension?, merge?}`
- `mask.featherPoint.remove` Delete Feather Point
  `{layer?, mask, index | all: true}`
- `mask.featherPoint.list` Feather Points
  `{layer?, mask}`
- `mask.interpolate` Smart Mask Interpolation · Animation > Keyframe Assistant
  `{layer?, mask?, from? (layer s), to? (layer s), rate? (keys/s, default comp rate), linearVertexPaths?, bendingResistance? (0..100), addVertices?, firstVerticesMatch?} → keys between two Mask Path keyframes`
- `mask.interpolationOptions` Smart Mask Interpolation Options
  `{rate?, keyframeFields?, linearVertexPaths?, bendingResistance?, quality?, addVertices?, vertexSpacing?, firstVerticesMatch?} → the options (sets the given ones)`
- `mask.new` New Mask (Pen)
  `{layer?, vertices: [[x,y]...] (layer px), in?, out?, closed?, mode?, name?} → {mask}`
- `mask.addVertex` Add Vertex
  `{layer?, mask (mask or shape path uid), point [x,y], index?, in?, out?} → {index}`

## path

- `path.freeTransform` Free Transform Points · Layer > Mask and Shape Path
  `{layer?, mask? (a mask or shape path group uid), scale?: % | [x%, y%], rotation?: deg, offset?: [dx, dy], anchor?: [x, y]}`

## shape

- `shape.add` Add Shape Item
  `{layer?, kind: group|rect|ellipse|star|polygon|path|fill|stroke|gradientFill|gradientStroke|trim|repeater|roundCorners|offsetPaths|twist|wiggle|mergePaths|puckerBloat|zigZag|wiggleTransform, group? (uid), name?, path?}`
- `shape.newPath` Pen Tool (Shape Path)
  `{layer?, vertices: [[x,y]…], inTangents?, outTangents?, closed?, space?: comp|layer, fill?: [r,g,b]|#hex|false, stroke?, strokeWidth?, name?} → {layer, path}`
- `shape.dashes.add` Add Dash or Gap
  `{layer?, prop?: stroke uid}`
- `shape.dashes.remove` Remove Dash or Gap
  `{layer?, prop?: stroke uid}`
- `shape.stroke.taper` Stroke Taper
  `{layer?, prop?: stroke uid, units?: pixels|percent, startLength?, endLength?, startWidth? (%), endWidth? (%), startEase? (%), endEase? (%)}`
- `shape.stroke.wave` Stroke Wave
  `{layer?, prop?: stroke uid, amount? (%), units?: pixels|cycles, wavelength?, cycles?, phase? (°)}`

## text

- `text.edit` Edit Text
  `{layer?, caret?}`
- `text.endEdit` End Text Editing
- `text.insert` Type
  `{text}`
- `text.delete` Delete Characters
  `{direction?: backward|forward, word?: bool}`
- `text.moveCaret` Move Caret
  `{to: left|right|up|down|home|end|start|all, extend?: bool}`
- `text.fonts` Fonts
- `text.addSelector` Add Text Selector
  `{layer?, animator?: uid|index, kind: range|wiggly|expression}`
- `text.removeAllAnimators` Remove All Text Animators · Animation
  `{layers?}`
- `text.animatorProperties` Text Animator Properties
- `text.fontFeatures` OpenType Features
  `{layer?, font?, style?} → {family, style, features: [tags], axes, options: {smallCaps, superscript, stylisticSets: [n], fractions, …}}`
- `text.animatorFontAxes` Variable Font Axes · Animation
  `{layer?, axis?: tag|name (omit to list the font's axes), animator?: uid|index}`
- `text.setSelection` Set Text Selection
  `{layer?, start, end?} | {anchor, caret} | {select: "all"} (character indices)`
- `text.presets` Text Animation Presets
- `text.library` Text Animation Preset Library
  `{category?, query?}`
- `text.applyLibraryPreset` Apply Text Preset
  `{preset: id|name (text.library), layers?, layer?, time? (s)}`

## camera

- `camera.fromView` Create Camera from 3D View · Layer > Camera
  `{comp?, switch?: bool (default true)}`
- `camera.orbit` Orbit Camera
  `{yaw|dx deg, pitch|dy deg, layer?, merge?} · current 3D view (the camera layer in Active Camera)`
- `camera.pan` Pan Camera
  `{dx, dy (comp px), layer?, merge?}`
- `camera.dolly` Dolly Camera
  `{amount (px, + = forward), layer?, merge?}`
- `camera.orbitNull` Create Orbit Null · Layer > Camera
  `{comp?, layer?: camera (default selected/active)}`
- `camera.fromModel` Create Cameras from 3D Model · Layer > Camera
  `{comp?, layers?: 3D model layers (default selected)}`
- `camera.linkFocusToPoi` Link Focus Distance to Point of Interest · Layer > Camera
  `{comp?, camera?}`
- `camera.linkFocusToLayer` Link Focus Distance to Layer · Layer > Camera
  `{comp?, camera?, layer?}`
- `camera.setFocusToLayer` Set Focus Distance to Layer · Layer > Camera
  `{comp?, camera?, layer?}`
- `camera.analyze` Analyze
  `{layer?, effect?: uid, wait?}`
- `camera.cancel` Cancel
- `camera.solveStatus` 3D Camera Tracker Status
  `{layer?, effect?}`
- `camera.points` 3D Camera Tracker Points
  `{layer?, effect?, time? (comp s)} → the solved points visible at that time, in comp px`
- `camera.selectPoints` Select Track Points
  `{points: [id], add?, toggle?}`
- `camera.createFromSolve` Create from Camera Solve
  `{kind: text|solid|null|shadowCatcher|camera, points?: [id] (default selected), target?: {center, normal, size?} (comp world), multiple?, layer?, effect?}`
- `camera.create` Create Camera
  `{layer?, effect?}`
- `camera.setGroundPlane` Set Ground Plane and Origin
  `{points?: [id] (default selected), layer?, effect?}`
- `camera.deletePoints` Delete Selected Points
  `{points?: [id] (default selected), layer?, effect?, wait?}`

## light

- `light.fromModel` Create Lights from 3D Model · Layer > Light
  `{comp?, layers?: 3D model layers (default selected)}`
- `light.controlWithCamera` Control Light with Camera · Layer > Light
  `{comp?, layers?: lights (default selected), camera? (default active), on?: bool (default toggle)}`
- `light.environmentBackground` Create Environment Light Background Layer · Layer > Light
  `{comp?, layer?: environment light (default selected / first)}`

## material

- `material.set` Material Options
  `{layers?, castsShadows?: off|on|only, acceptsShadows?: off|on|only, acceptsLights?, appearsInReflections?, lightTransmission?, ambient?, diffuse?, specularIntensity?, specularShininess?, metal?, reflectionIntensity?, overrideMaterial?, baseColor?, metallic?, roughness?, emissive?, time?}`
- `material.revealSource` Reveal Material Source in Project · Layer > Material
  `{layer?}`
- `material.reset` Reset Material · Layer > Material
  `{layers?}`
- `material.duplicateAssign` Duplicate and Assign Material · Layer > Material
  `{layers? (first = source)}`

## track

- `track.motion` Track Motion · Animation
  `{layer?}`
- `track.stabilize` Stabilize Motion · Animation
  `{layer?}`
- `track.new` New Tracker
  `{layer?, kind?: transform|stabilize|affine|perspective|raw, position?, rotation?, scale?, target?: layer}`
- `track.property` Track this Property · Animation
  `{layer?, source?: layer}`
- `track.select` Current Track
  `{layer?, tracker?: uid|name|#n}`
- `track.setType` Track Type
  `{layer?, tracker?, kind?: transform|stabilize|affine|perspective|raw, position?, rotation?, scale?}`
- `track.setTarget` Edit Target
  `{layer?, tracker?, target: layer|null}`
- `track.options` Motion Tracker Options
  `{layer?, tracker?, name?, channel?: rgb|luminance|saturation, blur? (px, 0 = off), enhance?, subpixel?, adaptEveryFrame?, threshold? (%), action?: continue|stop|extrapolate|adapt, trackShape?}`
- `track.setPoint` Move Track Point
  `{layer?, tracker?, point: n (1-based), center?: [x,y], featureSize?: [w,h], searchOffset?: [x,y], searchSize?: [w,h], attachOffset?: [x,y], move?: [dx,dy], time? (s), merge?}`
- `track.analyze` Analyze
  `{layer?, tracker?, direction?: forward|backward|frameForward|frameBackward, start? (s), end? (s), wait?: block until done}`
- `track.stop` Stop Analysis
  `{kind?: track|mask|warp|camera}`
- `track.apply` Apply
  `{layer?, tracker?, dimensions?: xy|x|y}`
- `track.reset` Reset
  `{layer?, tracker?}`
- `track.delete` Delete Tracker
  `{layer?, tracker?}`
- `track.editTargetDialog` Edit Target
- `track.optionsDialog` Options
- `track.status` Tracker Status
  `{layer?, tracker?}`
- `track.warpStabilizer` Warp Stabilizer · Animation
  `{layer?, wait?: block until analyzed} → adds a Warp Stabilizer (or reuses the layer's) and analyzes the footage`
- `track.camera` Track Camera · Animation
  `{layer?, shotType?: fixed|variable|specify, aov?: degrees, solveMethod?: auto|typical|flat|tripod, detailed?, wait?} → adds a 3D Camera Tracker (or reuses the layer's), tracks features and solves the camera`
- `track.mask` Track Mask · Animation
  `{layer?, mask?: uid|index|name, method?: position|positionScale|positionScaleRotation|positionScaleRotationSkew|perspective|faceOutline|faceDetailed, direction?: forward|backward|frameForward|frameBackward, start? (s), end? (s), wait?: block until done}`
- `track.maskMethod` Mask Tracking Method
  `{method: position|positionScale|positionScaleRotation|positionScaleRotationSkew|perspective|faceOutline|faceDetailed}`
- `track.extractFaceMeasurements` Extract & Copy Face Measurements
  `{layer?} → keys a Face Measurements effect from the layer's Face Track Points (one key per tracked frame) and copies those keys`

## warp

- `warp.analyze` Analyze
  `{layer?, effect?: uid, wait?}`
- `warp.cancel` Cancel
- `warp.status` Warp Stabilizer Status
  `{layer?, effect?: uid}`

## roto

- `roto.models` Roto Brush Models
- `roto.model.select` Use Roto Brush Model
  `{id: classical|mobilesam, wait?} (Roto Brush and Object Matte use it; wait: loaded before returning)`
- `roto.model.install` Install Roto Brush Model
  `{path: weights file, id?, wait?} · verified against the official SHA-256`
- `roto.model.download` Download Roto Brush Model
  `{id, wait?} · the official weights, verified (uses the system curl)`
- `roto.model.remove` Remove Roto Brush Model
  `{id}`
- `roto.stroke` Roto Brush Stroke
  `{layer?, kind?: fg|bg|refine|refineErase, points: [[x, y], ...] (layer pixels), frame? (layer frame; default current), radius? (layer px; default the tool's diameter / 2), effect?, noPropagate?}`
- `roto.propagate` Propagate Roto Brush
  `{layer?, effect?, direction?: forward|backward|both, to? (layer frame: extend the span), wait?: block until done}`
- `roto.span` Set Segmentation Span
  `{layer?, effect?, start?, end? (layer frames)}`
- `roto.freeze` Freeze
  `{layer?, effect?, wait?}`
- `roto.unfreeze` Unfreeze
  `{layer?, effect?}`
- `roto.clearStrokes` Remove Roto Brush Strokes
  `{layer?, effect?, frame? (only this layer frame), kind?}`
- `roto.cancel` Stop Roto Brush
- `roto.options` Roto Brush Options
  `{diameter?, refineDiameter?, view?: alphaBoundary|alpha|alphaOverlay|none, overlayColor?: [r,g,b], overlayOpacity?, boundaryColor?: [r,g,b], autoPropagate?}`
- `roto.status` Roto Brush Status
  `{layer?, effect?, frame? (layer frame: also report its matte), matte?: include the matte (RLE, base64), compute?: compute the frame if needed, compareTo?: RLE matte to report the IoU against}`

## paint

- `paint.stroke` Paint Stroke
  `{layer?, kind?: brush|clone|eraser, points: [[x,y,pressure?],...] (layer space), time? (s) | frame?, duration? (s, drawing time for Write On), color?: [r,g,b,a], diameter?, angle?, hardness?, roundness?, spacing?, opacity?, flow?, mode?, channels?: RGBA|RGB|Alpha, durationMode?: constant|writeOn|singleFrame|custom, customFrames?, eraseMode?: layerSourceAndPaint|paintOnly|lastStrokeOnly, cloneSource?: layer id, clonePosition?: [x,y], cloneTimeShift? (s), lockSourceTime?, cloneTime? (s), sizePressure?, minSize?, opacityPressure?, flowPressure?}`
- `paint.options` Paint Options
  `{color?, background?, opacity?, flow?, mode?, channels?, durationMode?, customFrames?, eraseMode?, diameter?, angle?, roundness?, hardness?, spacing?, preset?, sizePressure?, minSize?, opacityPressure?, flowPressure?, clonePreset?, cloneSource?, clonePoint?, cloneOffset?, aligned?, lockSourceTime?, cloneTimeShift?, cloneTime?, showOverlay?, overlayOpacity?, overlayDifference?, reset?}`
- `paint.brushPreset` Brush Preset
  `{preset?: index | name}`
- `paint.setCloneSource` Set Clone Source
  `{layer?, point: [x,y]}`
- `paint.removeStroke` Delete Paint Stroke
  `{layer?, stroke: uid | name}`
- `paint.presets` Brush Presets

## puppet

- `puppet.addPin` Add Puppet Pin
  `{layer?, kind?: position|advanced|bend|starch|overlap, position: [x,y] (layer space), time? (s) | frame?, mesh?: uid, newMesh?, density?, expansion?, triangles?}`
- `puppet.movePin` Move Puppet Pin
  `{layer?, pin: uid | name, position: [x,y], time? | frame?, merge?}`
- `puppet.setPin` Edit Puppet Pin
  `{layer?, pin: uid | name, position?, scale? (%), rotation? (deg), amount? (%), extent? (px), inFront?, time? | frame?, merge?}`
- `puppet.removePin` Delete Puppet Pin
  `{layer?, pin?: uid | name, pins?: [uid | name...]} (default: the selected pins)`
- `puppet.selectPins` Select Puppet Pins
  `{layer?, pins: [uid | name...] ([] deselects), add?, toggle?}`
- `puppet.mesh` Puppet Mesh Options
  `{layer?, mesh?: uid, density?, expansion?, triangles?, showMesh?, time?, merge?}`
- `puppet.info` Puppet Mesh Info
  `{layer?, time? | frame?}`
- `puppet.recordPin` Record Puppet Pin
  `{layer?, pin: uid | name, samples: [[t, x, y]...] (t = seconds since the drag began, layer space), pins?: [uid | name...] (more pins that move by the same displacement), start? (s, default current time), speed? (%, Record Options), smoothing? (Record Options)}`
- `puppet.follow` Follow-Through
  `{layer?, leader?: uid | name (default: the first selected pin), pins?: [uid | name...] (default: the other selected pins), delay? (s, 0.1), amount? (%, 100), cascade? (true: the k-th nearest pin trails k x delay)}`
- `puppet.recordOptions` Record Options
  `{speed? (%, 100), smoothing? (0-100), useDraftDeformation?, showMesh?}`

## liquify

- `liquify.stroke` Liquify Stroke
  `{layer?, effect?, points: [[x, y]] (layer px), tool?: warp|turbulence|twirlClockwise|twirlCounterclockwise|pucker|bloat|shiftPixels|reflection|clone|reconstruction, brushSize?, brushPressure?, jitter?, cloneOffset? [dx, dy]}`
- `liquify.clear` Clear Liquify Mesh
  `{layer?, effect?}`

## contentFill

- `contentFill.set` Content-Aware Fill Settings
  `{method?: object|surface|edgeBlend, range?: workArea|entire, alphaExpansion? (px), lightingCorrection?: none|subtle|moderate|strong, referenceLayer?: layer|null, referenceTime? (s)}`
- `contentFill.generate` Generate Fill Layer
  `{layer?, method?: object|surface|edgeBlend, range?: workArea|entire, alphaExpansion? (px), lightingCorrection?: none|subtle|moderate|strong, referenceLayer?, referenceTime? (s), outputDir?, wait?: bool}`

## face

- `face.models` Face Tracking Models
  `{} -> the classical detector and the learned MediaPipe Face Landmarker (detector and 478-point mesh), which is installed and which one face tracking uses`
- `face.model.select` Use Face Tracking Model
  `{id: classical|facelandmarker, wait?}`
- `face.model.install` Install Face Tracking Model
  `{path: face_landmarker.task, id?} · verified against the official SHA-256`
- `face.model.download` Download Face Tracking Model
  `{id?: facelandmarker} · the official Face Landmarker bundle (Apache-2.0), verified against its SHA-256 (uses the system curl)`
- `face.model.remove` Remove Face Tracking Model
  `{id?}`

## essential

- `essential.addProperty` Add Property to Essential Graphics · Animation
  `{layer?, path|prop, comp?, name? (controller name, default the property's), group? (a group controller id)} · the property becomes a controller of the composition's template; every precomp layer of the composition gets it under Essential Properties`
- `essential.addComment` Add Comment
  `{comp?, text}`
- `essential.addGroup` Add Group
  `{comp?, name}`
- `essential.remove` Remove Controller
  `{comp?, controller (id, name or 1-based index)}`
- `essential.rename` Rename Controller
  `{comp?, controller, name?, text? (a comment's text)}`
- `essential.move` Move Controller
  `{comp?, controller, index (1-based), group? (a group controller id, or "" for none)}`
- `essential.setTemplateName` Set Template Name
  `{comp?, name}`
- `essential.open` Open in Essential Graphics · Composition
  `{comp?}`
- `essential.list` Essential Graphics Controllers
  `{comp? (default: the composition open in Essential Graphics, else the active one)} -> {comp, templateName, controllers: [{id, name, kind: property|comment|group, group, layer, path, value, ui}], instances}`
- `essential.revertToMaster` Revert to Master
  `{layers?, controller? (default: all)} · precomp layers: Essential Properties follow the master values again`
- `essential.exportTemplate` Export as Motion Graphics Template · Composition
  `{comp?, path? (.fvxtemplate), folder? (default the Templates folder; the file is named after the template), name?, overwrite?: bool} · formvfx's own template: the composition, what it uses, its media and a poster frame in one file`
- `essential.importTemplate` Motion Graphics Template · File > Import
  `{path (.fvxtemplate), addToComp?: bool, comp?, time?} -> {comp, folder, layer}`
- `essential.templates` Installed Templates
  `{} -> templates in the Templates folder with name, size, duration, controller count and poster`
- `essential.installTemplate` Install Motion Graphics Template
  `{path (.fvxtemplate)} · copies it into the Templates folder`

## expr

- `expr.errors` Expression errors (the error bar)
  `{comp?, time? (s)}`

## script

- `script.run` Run Script
  `{code (JavaScript with the After Effects scripting object model: app.project, CompItem, layers, properties, File, ScriptUI), file? (shown as $.fileName)} → {ok, result, output, error: {message, line}}; the whole script is one undo step`

## scriptui

- `scriptui.list` ScriptUI Windows
- `scriptui.event` ScriptUI Event
  `{window, control, event: click|change|changing|doubleclick, text?, value?, selection?} · runs the control's handler`
- `scriptui.close` Close ScriptUI Window
  `{window}`
- `scriptui.openPanel` Open ScriptUI Panel
  `{name (an installed ScriptUI panel)}`

## render

- `render.addOutputModule` Add Output Module · Composition
  `{item?|index? (default: the last item), format?, output?, channels?, quality?, bitrate?, proresProfile?, audio?, template?}`
- `render.preRender` Pre-render · Composition
  `{comp?, output?: path|template, format?} -> a Render Queue item with the Pre-render templates that imports the result and replaces the composition's usage`
- `render.saveCurrentPreview` Save Current Preview · Composition
  `{comp?, path (.mp4 or .mov), resolution?} -> renders the work area now`

## renderQueue

- `renderQueue.add` Add to Render Queue · Composition · Cmd+M
  `{comp?, template? (Render Settings template), outputModule? (Output Module template), format?: h264|hevc|av1|prores|webm|gif|png|jpeg|tiff|exr|wav|aiff, output? (path or template like [compName].[fileExtension]), log?: errorsOnly|plusSettings|plusPerFrameInfo, quality?: best|draft or 1-100, resolution?: full|half|third|quarter|scale, timeSpan?: workArea|comp|custom, start? (s), end? (s), duration? (s), frameRate?, colorDepth?, motionBlur?, channels?: rgb|rgba|alpha, bitrate? (kbps), proresProfile?, audio?: auto|on|off, outputSpace?, depth?: 8|16|32 (values as in setRenderSettings and setOutputModule)}`
- `renderQueue.list` Render Queue Items
- `renderQueue.remove` Remove from Render Queue
  `{item?|index?}`
- `renderQueue.setOutput` Output To
  `{item?|index?, module?: n (1 = first), path (or a template like [compName]_[#####].[fileExtension])}`
- `renderQueue.setRender` Render Checkbox
  `{item?|index?, render?: bool (toggles)}`
- `renderQueue.setFormat` Output Module Format
  `{item?|index?, module?, format}`
- `renderQueue.setRenderSettings` Render Settings
  `{item?|index?, template?: name, quality?: best|draft|current, resolution?: full|half|third|quarter|scale, proxyUse?: current|all|comp|none, effects?: current|allOn|allOff, solo?: current|allOff, guideLayers?: current|allOff, colorDepth?: current|8|16|32, frameBlending?: current|onForChecked|offForAll, fieldRender?: off|upper|lower, pulldown?: off|WSSWW|SSWWW|SWWWS|WWWSS|WWSSW, motionBlur?: bool|current|onForChecked|offForAll, timeSpan?: workArea|comp|custom, start?: s, end?: s, duration?: s, frameRate?: fps|null, skipExisting?: bool, storageOverflow?: bool}`
- `renderQueue.setOutputModule` Output Module Settings
  `{item?|index?, module?: n (1 = first), template?: name, format?: h264|hevc|av1|prores|webm|png|jpeg|tiff|exr|gif|wav|aiff, channels?: rgb|rgba|alpha, color?: straight|premultiplied, quality?: 1-100, bitrate?: kbps, webmBitrate?: bool, keyframeInterval?: frames (0 = auto), proresProfile?: proxy|lt|standard|hq|4444|4444xq, profile?: main|main10|high, rateControl?: bitrate|quality, webmCodec?: vp9|av1, audioBitrate?: kbps, crop?: bool|{top?, left?, bottom?, right?}, cropTop?, cropLeft?, cropBottom?, cropRight?, resize?: bool|{width, height}, resizeWidth?, resizeHeight?, postRenderAction?: none|import|importAndReplace|setProxy, audio?: auto|on|off, sampleRate?, audioChannels?: mono|stereo, audioFormat?: 16|24|32, outputSpace?, depth?: 8|16|32, loop?: bool, output?}`
- `renderQueue.move` Move in Render Queue
  `{item?|index?, to: n (1-based)}`
- `renderQueue.duplicate` Duplicate Render Item
  `{item?|index?}`
- `renderQueue.templates` Render Templates
- `renderQueue.saveTemplate` Save Template
  `{kind: renderSettings|outputModule, name, item?|index? (save from this item), from?: template to copy, module?, params?: {setRenderSettings / setOutputModule keys}}`
- `renderQueue.deleteTemplate` Delete Template
  `{kind: renderSettings|outputModule, name}`
- `renderQueue.setTemplateDefault` Set Template Default
  `{kind: renderSettings|outputModule, slot: movie|still|preRender|movieProxy|stillProxy, name}`
- `renderQueue.applyTemplate` Apply Template
  `{item?|index?, renderSettings?: template name, outputModule?: template name, module?: n}`
- `renderQueue.setLog` Render Queue: Log
  `{item?|index?, log: errorsOnly|plusSettings|plusPerFrameInfo}`
- `renderQueue.setNotify` Notify When Done
  `{notify?: bool (toggles)}`
- `renderQueue.setOverflowFolders` Storage Overflow Folders
  `{folders: [path, ...] (used in order when the output volume fails)}`
- `renderQueue.render` Render
  `{wait?: bool}`
- `renderQueue.stop` Stop Rendering
- `renderQueue.formats` Output Formats
- `renderQueue.setColor` Output Module Color
  `{item, outputSpace? (working, srgb, rec709, rec2020, p3, rec2100pq, rec2100hlg ... or an OCIO colour space or `Display | View`), depth?: 8|16 (16-bit PNG, TIFF, ProRes and 10-bit HEVC)}`

## playback

- `playback.toggle` Play / Stop · Space
- `playback.audio` Audio · Composition > Preview
  `{value?: include audio in previews}`
- `playback.cacheWhenIdle` Cache Frames When Idle · Composition > Preview
  `{value?}`

## view

- `view.channel` Show Channel
  `{channel: RGB|Red|Green|Blue|Alpha}`
- `view.snapping` Snapping · View
  `{value?: bool}`
- `view.layout` View Layout
  `{views: 1|2|4}`
- `view.set3DView` Switch 3D View
  `{view: activeCamera|default|front|left|top|back|right|bottom|custom1|custom2|custom3, comp?}`
- `view.3d.activeCamera` Active Camera · View > Switch 3D View · F12
  `{comp?}`
- `view.3d.default` Default · View > Switch 3D View
  `{comp?}`
- `view.3d.front` Front · View > Switch 3D View · F10
  `{comp?}`
- `view.3d.left` Left · View > Switch 3D View
  `{comp?}`
- `view.3d.top` Top · View > Switch 3D View
  `{comp?}`
- `view.3d.back` Back · View > Switch 3D View
  `{comp?}`
- `view.3d.right` Right · View > Switch 3D View
  `{comp?}`
- `view.3d.bottom` Bottom · View > Switch 3D View
  `{comp?}`
- `view.3d.custom1` Custom View 1 · View > Switch 3D View · F11
  `{comp?}`
- `view.3d.custom2` Custom View 2 · View > Switch 3D View
  `{comp?}`
- `view.3d.custom3` Custom View 3 · View > Switch 3D View
  `{comp?}`
- `view.3d.last` Switch to Last 3D View · View
  `{comp?}`
- `view.reset3DView` Reset 3D View · View
  `{comp?, view?}`
- `view.set3DViewCamera` Set 3D View Camera
  `{view?, eye? [x,y,z], poi? [x,y,z], zoom? (orthographic views: scale)}`
- `view.get3D` 3D View State
  `{comp?} → current 3D view, view camera, active camera layer, lights`
- `view.lookAtSelected` Look at Selected Layers · View · Cmd+Alt+Shift+\
  `{comp?, layers?}`
- `view.lookAtAll` Look at All Layers · View
  `{comp?}`
- `view.setRegionOfInterest` Region of Interest
  `{rect? [x,y,w,h]|null}`
- `view.resetExposure` Reset Exposure
  `{value?}`
- `view.addGuide` Add Guide
  `{orientation: horizontal|vertical, position}`
- `view.clearGuides` Clear Guides · View
- `view.zoomIn` Zoom In · View · .
- `view.zoomOut` Zoom Out · View · ,
- `view.fit` Resolution and Fit · View · Shift+/
- `view.fullScreen` Full Screen · View
- `view.newViewer` New Viewer · View · Alt+Shift+N
  `{} · another Composition viewer of the active comp; the one in use is locked`
- `view.splitLockedViewer` Split with New Locked Viewer · View
  `{comp?, view?: 3D view id}`
- `view.closeLockedViewer` Close Locked Viewer
- `view.takeSnapshot` Take Snapshot · View · Shift+F5
  `{comp?, scale?: 0.05..1}`
- `view.showSnapshot` Show Snapshot · View
  `{value?}`
- `view.displayColorManagement` Use Display Color Management · View
  `{value?: bool}`
- `view.simulateOutput` Simulate Output · View
  `{profile: none|srgb|rec709|rec2020|p3|dciP3|mac18|rec2100pq|rec2100hlg|..., preserveRgb?: bool}`
- `view.exportGuides` Export Guides · View
  `{path}`
- `view.importGuides` Import Guides · View
  `{path}`
- `view.grid` Show Grid · View · Cmd+'
  `{value?}`
- `view.guides` Show Guides · View · Cmd+;
  `{value?}`
- `view.lockGuides` Lock Guides · View · Cmd+Alt+Shift+;
  `{value?}`
- `view.snapToGrid` Snap to Grid · View · Cmd+Shift+'
  `{value?}`
- `view.snapToGuides` Snap to Guides · View · Cmd+Shift+;
  `{value?}`
- `view.rulers` Show Rulers · View · Cmd+R
  `{value?}`
- `view.options` View Options · View · Cmd+Alt+U
- `view.layerControls` Show Layer Controls · View · Cmd+Shift+H
  `{value?}`
- `view.panelBackground` Panel Background Color
  `{color?: black|darkGray|mediumGray|lightGray|white|custom|#hex, pick?: true}`
- `view.res.full` Full · View > Resolution · Cmd+J
- `view.res.half` Half · View > Resolution · Cmd+Shift+J
- `view.res.third` Third · View > Resolution
- `view.res.quarter` Quarter · View > Resolution · Cmd+Alt+Shift+J
- `view.res.custom` Custom · View > Resolution
  `{factor?: 1..40 (render every n-th pixel)}`

## window

- `window.panel` Show Panel
  `{panel}`
- `window.workspace` Workspace
  `{name}`
- `window.resetWorkspace` Reset Workspace · Window > Workspace
- `window.saveWorkspace` Save Changes to this Workspace · Window > Workspace
- `window.saveWorkspaceAs` Save as New Workspace · Window > Workspace
  `{name}`
- `window.editWorkspaces` Edit Workspaces · Window > Workspace
  `{name, rename?: new name, delete?: bool}`

## cache

- `cache.diskStats` Disk Cache Statistics
  `{} · folder, size, frame count, limit, hits`
- `cache.cleanDisk` Empty Disk Cache · Edit > Purge

## prefs

- `prefs.pages` Settings Pages
- `prefs.get` Get Setting
  `{key}`
- `prefs.set` Change Setting
  `{key, value}`

## shortcuts

- `shortcuts.list` Keyboard Shortcuts
- `shortcuts.set` Set Shortcut
  `{command, keys: [string]}`
- `shortcuts.reset` Reset Shortcut
  `{command?}`
- `shortcuts.conflicts` Shortcut Conflicts
- `shortcuts.preset` Keyboard Shortcut Preset
  `{op: select|new|duplicate|delete|rename, name?, from?, newName?}`
- `shortcuts.export` Export Keyboard Shortcuts
  `{preset?, path?}`
- `shortcuts.import` Import Keyboard Shortcuts
  `{path? | preset?: exported document, name?}`

## app

- `app.about` About formvfx · Help
- `app.settings` Settings · Edit · Cmd+,
  `{page?}`
- `app.commandPalette` Command Palette · Window
  `{query?}`
- `app.keyboardShortcuts` Keyboard Shortcuts · Edit · Cmd+Alt+'
- `app.capabilities` App Capabilities
- `app.find` Find · File · Cmd+F
  `{query?}`
- `app.quit` Quit formvfx · Cmd+Q
- `app.gpuInfo` GPU Information
- `app.hide` Hide formvfx
- `app.hideOthers` Hide Others
- `app.showAll` Show All
- `app.templates` Templates
  `{kind: renderSettings|outputModule}`

## engine

- `engine.batch` Run Commands (Batch)
  `{steps: [{command, params?}], label?, atomic?: bool} (a string "$N" or "$N.key" is step N's result, 1-based)`

## command

- `command.list` Commands
  `{filter?, schemas?: bool}`
- `command.describe` Describe Command
  `{command}`

## motion

- `motion.sketch` Motion Sketch
  `{layer?, points: [[t, x, y]…] (t = capture seconds) | [[x, y]…] (one per frame), start? (s), captureSpeed? (%, 100), smoothing? (px, 1)}`

## clipboard

- `clipboard.pasteSvg` Paste Vector as Shapes
  `{svg (markup), name?, comp?} → {layer, shapes}`

## color

- `color.spaces` Colour Spaces
  `{config? (.ocio path; empty for the project's)} · working spaces, profiles, OCIO colour spaces, displays, views, looks and roles`
- `color.convert` Convert Colours
  `{values: [[r,g,b]...], src?, dst?, display?, view?, looks?, inverse?: bool, config?} · converts through formvfx's profiles or the OCIO config`
