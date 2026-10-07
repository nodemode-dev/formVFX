# formvfx commands

Generated from the formvfx engine by `./formvfx agents sync`; do not edit by hand.

648 commands in 53 namespaces. Every menu item, button and drag in formvfx is one of these. Run one with `execute {command, params}` over MCP, inside `batch` for several as one undo step, or `formvfx exec ID 'JSON'` from a shell. `describe_command ID` returns the JSON Schema. In the parameter summaries `?` marks an optional parameter, `a|b` lists the accepted values, and `layers?`/`layer?` default to the selected layers. Times are composition seconds unless the summary says layer seconds.

## Contents

`file` (58) `edit` (26) `project` (11) `item` (1) `footage` (9) `comp` (25) `layer` (105) `prop` (16) `keys` (34) `anim` (7) `time` (13) `markers` (5) `effect` (16) `mask` (17) `path` (6) `paths` (3) `shape` (5) `text` (13) `camera` (19) `light` (3) `material` (4) `track` (22) `warp` (3) `roto` (14) `paint` (6) `puppet` (10) `liquify` (2) `contentFill` (2) `face` (5) `essential` (22) `expr` (2) `script` (1) `scriptui` (5) `render` (4) `renderQueue` (20) `playback` (6) `view` (60) `window` (8) `editor` (1) `scopes` (1) `cache` (1) `jobs` (3) `mediaBrowser` (7) `templates` (5) `prefs` (5) `shortcuts` (7) `storage` (3) `app` (12) `engine` (1) `command` (2) `motion` (1) `learn` (5) `help` (6)

## file

- `file.newProject` New Project · File > New · Cmd+Alt+N
- `file.openDemoProject` Open Demo Project · Help
- `file.open` Open Project · File · Cmd+O
  `{path}`
- `file.save` Save · File · Cmd+S
  `{path?}`
- `file.saveAs` Save As · File > Save As · Cmd+Shift+S
  `{path}`
- `file.incrementAndSave` Increment and Save · File · Cmd+Alt+Shift+S
- `file.revert` Revert · File
- `file.import` File · File > Import · Cmd+I
  `{paths: [string], importAs?: footage|composition|compositionLayerSizes (Photoshop, PDF, Illustrator and EPS files), layer?: name|index (footage of one Photoshop layer), page?: number from 1 (PDF / Illustrator page), drag?: bool (dropped files: Settings ▸ Import ▸ Default Drag Import As), addToComp?: bool (also add them to the active comp, at time?, index?, position? as in layer.addItem)}`
- `file.projectSettings` Project Settings · File · Cmd+Alt+Shift+K
  `{bitDepth?: 8|16|32, colorEngine?: adobe|ocio, workingSpace?: none|srgb|rec709|rec2020|p3|acescg|aces2065, linearize?, blendLinear?, hdr?: clip|compand|toneMap, outputSpace?: srgb|rec709|rec2020|p3|rec2100pq|rec2100hlg, renderer?: gpu|software, timeDisplay?: timecode|frames|feet35|feet16}`
- `file.cycleBitDepth` Cycle Project Bit Depth
- `file.close` Close · File · Cmd+W
  `{comp?}`
- `file.closeProject` Close Project · File
- `file.saveCopy` Save a Copy · File > Save As
  `{path}`
- `file.importMultiple` Multiple Files · File > Import · Cmd+Alt+I
  `{paths: [string]}`
- `file.importPlaceholder` Placeholder · File > Import
  `{name?, width?, height?, frameRate?, duration? (s)}`
- `file.importSolid` Solid · File > Import
  `{name?, color?, width?, height?}`
- `file.newCompFromSelection` New Comp from Selection · File · Cmd+Alt+\
  `{duration? (s, for stills), single?: bool (one comp for all), dimensionsFrom?: index, sequence?: bool, overlap?: bool, overlapDuration? (s), transition?: off|dissolveFront|crossDissolve, addToRenderQueue?: bool} → {comps}`
- `file.collectFiles` Collect Files · File > Dependencies
  `{folder}`
- `file.consolidateFootage` Consolidate All Footage · File > Dependencies
- `file.removeUnusedFootage` Remove Unused Footage · File > Dependencies
- `file.reduceProject` Reduce Project · File > Dependencies
  `{} (keeps the selected comps and what they use)`
- `file.findMissing` Find Missing
  `{what: footage|effects|fonts}`
- `file.runScript` Run Script File · File > Scripts
  `{path (.jsx/.js JavaScript, or a .jsonl/.json command script) | name (an installed or sample script, see file.scripts.list) | steps: [{command, params}]}`
- `file.interpretFootage` Main · File > Interpret Footage · Cmd+Alt+G
  `{items?, frameRate?: fps|"file", alpha?: straight|premultiplied|ignore|guess, guessAlpha?, matteColor?, invertAlpha?, loop?, pixelAspect?, fields?: off|upper|lower, colorProfile?: srgb|rec709|rec2020|p3|auto, linearLight?}`
- `file.rememberInterpretation` Remember Interpretation · File > Interpret Footage
  `{item?}`
- `file.applyInterpretation` Apply Interpretation · File > Interpret Footage · Cmd+Alt+V
  `{items?}`
- `file.replaceFootage` File · File > Replace Footage · Cmd+H
  `{path, item?}`
- `file.replaceWithPlaceholder` Placeholder · File > Replace Footage
  `{item?, width?, height?, frameRate?, duration?}`
- `file.replaceWithSolid` Solid · File > Replace Footage
  `{item?, color?, width?, height?}`
- `file.reloadFootage` Reload Footage · File · Cmd+Alt+L
  `{items?}`
- `file.revealInFinder` Reveal in Finder · File
  `{item?}`
- `file.exportLottie` Lottie JSON · File > Export
  `{comp?, path (.json or .lottie), includeExpressions?: bool, textAsShapes?: bool}`
- `file.importLottie` Lottie · File > Import
  `{path (.json or .lottie)}`
- `file.importTimeline` Adobe Premiere Pro Project · File > Import
  `{path (.xml Final Cut Pro XML from Premiere Pro or .fcpxml .otio .edl .aaf .omf), format?: auto|xml|fcpxml|otio|edl|aaf|omf, edlFrameRate?: number}`
- `file.exportTimeline` Adobe Premiere Pro Project · File > Export
  `{comp?, path (Final Cut Pro XML .xml for Premiere Pro or .fcpxml .otio .edl .aaf .omf), format?: xml|fcpxml|otio|edl|aaf|omf, prerender?: none|unsupported|all, precomps?: nest|prerender}`
- `file.timelineFormats` Timeline Interchange Formats
- `file.watchFolder` Watch Folder · File
  `{folder, stop?: bool} → watches the folder for .ecproj files with queued renders, renders them and writes `[project].status.json``
- `file.watchFolder.poll` Poll Watch Folder
  `{folder? (default: the watched one)} → renders new projects now: {watching, rendered: [{project, state: done|failed, items: [{comp, status, output}]}]}`
- `file.importVanishingPoint` Vanishing Point (.vpe) · File > Import
  `{} (always disabled: undocumented format)`
- `file.openRecent` Open Recent
  `{index? | path?}`
- `file.clearRecent` Clear Recent Projects · File > Open Recent
- `file.autoSave` Auto-Save Now
- `file.recoveryInfo` Auto-Save Status
- `file.importRecent` Import Recent Footage
  `{index? | path?}`
- `file.clearRecentFootage` Clear Recent Footage · File > Import Recent Footage
- `file.saveCopyAsXml` Save a Copy As XML · File > Save As
  `{path: .ecprojx}`
- `file.replaceWithLayeredComp` With Layered Comp · File > Replace Footage
  `{item?}`
- `file.executeFile` Execute File
  `{path, confirmed?}`
- `file.createProxy` Create Proxy
  `{kind: still|movie, comp?|item?, path?|output? (template), resolution? (default 0.5)}`
- `file.setProxy` File · File > Set Proxy
  `{path, items?|item?}`
- `file.setProxyNone` None · File > Set Proxy
  `{items?|item?}`
- `file.useProxy` Use Proxy
  `{items?|item?, on?}`
- `file.interpretProxy` Proxy · File > Interpret Footage
  `{items?|item?, alpha?, matteColor?, invertAlpha?, frameRate?, loop?, pixelAspect?, fields?, colorProfile?, linearLight?}`
- `file.installScript` Install Script File · File > Scripts
  `{path (.jsx / .js)} → copies it to the Scripts folder; it appears in File ▸ Scripts`
- `file.installScriptUIPanel` Install ScriptUI Panel · File > Scripts
  `{path (.jsx / .js)} → copies it to the ScriptUI Panels folder; it appears in the Window menu`
- `file.uninstallScript` Uninstall Script
  `{name}`
- `file.scripts.list` List Scripts
  `{} → [{name, panel, source: installed|sample}]`
- `file.newFromTemplate` New Project from Template · File > New
  `{} → the Home screen's Templates tab`

## edit

- `edit.undo` Undo · Edit · Cmd+Z
- `edit.redo` Redo · Edit · Cmd+Shift+Z
- `edit.cut` Cut · Edit · Cmd+X
  `{layers?} (keyframes when keys are selected)`
- `edit.copy` Copy · Edit · Cmd+C
  `{layers?} (keyframes when keys are selected)`
- `edit.copyWithPropertyLinks` Copy with Property Links · Edit · Cmd+Alt+C
  `{layers?} (selected properties, or layers, as expressions linking to the originals)`
- `edit.copyWithRelativePropertyLinks` Copy with Relative Property Links · Edit
  `{layers?} (like Copy with Property Links, using thisComp)`
- `edit.copyExpressionOnly` Copy Expression Only · Edit
- `edit.paste` Paste · Edit · Cmd+V
  `{} (layers, keyframes at the CTI, or property links / expressions)`
- `edit.pasteReversedKeyframes` Paste Reversed Keyframes · Edit
  `{layers?, prop?|path?, time?}`
- `edit.clear` Clear · Edit · Delete
  `{layers?}`
- `edit.duplicate` Duplicate · Edit · Cmd+D
  `{layers?}`
- `edit.splitLayer` Split Layer · Edit · Cmd+Shift+D
  `{layers?}`
- `edit.liftWorkArea` Lift Work Area · Edit
  `{layers?}`
- `edit.extractWorkArea` Extract Work Area · Edit
  `{layers?}`
- `edit.selectAll` Select All · Edit · Cmd+A
- `edit.deselectAll` Deselect All · Edit · Cmd+Shift+A
- `edit.label` Label · Edit > Label
  `{label: Red|Yellow|Aqua|…, layers?, keys?, target?: layers}`
- `edit.selectLabelGroup` Select Label Group · Edit > Label
- `edit.purgeUndo` Undo · Edit > Purge
- `edit.purge` Purge
  `{what?: all|memoryAndDisk|memory|disk|3d|image|snapshot}`
- `edit.editOriginal` Edit Original · Edit · Cmd+E
- `edit.history.list` History
  `{} → {states: [{index, id, label, parent, depth, current, line, future}], current, branches}`
- `edit.history.goto` Go to History State
  `{index? (from edit.history.list) | id? | steps? (negative = back along the line)}`
- `edit.pasteTextMatchFormatting` Paste Text and Match Formatting · Edit
  `{text?} (the clipboard's text in the style at the caret)`
- `edit.pasteTextFormattingOnly` Paste Text Formatting Only · Edit
  `{layer?} (the copied text's formatting on the selected text or text layers)`
- `edit.history` History
  `{steps?: n (undo n steps), redo?: n}`

## project

- `project.select` Select Project Items
  `{items: [id|name], add?}`
- `project.rename` Rename Item
  `{item?: id|name (default: selected), name}`
- `project.move` Move to Folder
  `{items?: [id|name] (default: selected), folder: id|name|null (root)}`
- `project.setLabel` Item Label
  `{items?, label: name|index}`
- `project.setComment` Item Comment
  `{items?, comment}`
- `project.delete` Delete Project Items
  `{items?: [id|name] (default: selected)} (folders with their contents; layers using the items go too)`
- `project.usage` Project Item Usage
  `{items?: [id|name] (default: selected)} → {items, layers, comps} that deleting them removes`
- `project.duplicate` Duplicate Project Items
  `{items?: [id|name] (default: selected)} → {items}`
- `project.summary` Project Summary
- `project.newFolder` New Folder · File > New · Cmd+Alt+Shift+N
  `{name?, parent?}`
- `project.setProjectComment` Project Comment
  `{comment}`

## item

- `item.metadata` Metadata
  `{item?: id|name}`

## footage

- `footage.check` Check Footage
  `{items?: [id], wait?} · look for every footage file (in the background unless `wait`) and flag missing items`
- `footage.open` Open in Footage Panel
  `{item: id|name}`
- `footage.info` Footage Panel State
- `footage.setTime` Footage Panel Time
  `{time? (s, source) | frame?}`
- `footage.setIn` Set In Point
  `{time? | frame? (default: the panel's time)}`
- `footage.setOut` Set Out Point
  `{time? | frame? (default: the panel's time)}`
- `footage.clearInOut` Clear In and Out
- `footage.overlayEdit` Overlay Edit
  `{item?, comp?, in? (s), out? (s), at? (comp s; default the current time)}`
- `footage.rippleInsertEdit` Ripple Insert Edit
  `{item?, comp?, in? (s), out? (s), at? (comp s; default the current time)}`

## comp

- `comp.new` New Composition · Composition · Cmd+N
  `{name?, width?, height?, frameRate?, duration? (s), startTime? (s) | startTimecode?, background? [r,g,b]|#hex, pixelAspect?, shutterAngle?, shutterPhase?, motionBlurSamples?, adaptiveSampleLimit? (16–256), preserveFrameRate?: bool, preserveResolution?: bool, renderer?: classic3D|advanced3D, anchor?, open?}`
- `comp.settings` Composition Settings · Composition · Cmd+K
  `{comp?, name?, width?, height?, anchor? 0-8 (resize anchor, 4 = center), frameRate?, duration?, startTime? (s) | startTimecode?, background?, shutterAngle?, shutterPhase?, motionBlurSamples?, adaptiveSampleLimit? (16–256), preserveFrameRate?: bool (nested or in the render queue it shows only its own frames), preserveResolution?: bool (nested, it renders at full size), pixelAspect?, renderer?: classic3D|advanced3D}`
- `comp.setPosterTime` Set Poster Time · Composition
- `comp.trimToWorkArea` Trim Comp to Work Area · Composition · Cmd+Shift+X
  `{comp?}`
- `comp.open` Open Composition
  `{comp: id|name}`
- `comp.close` Close Composition
  `{comp?}`
- `comp.workArea` Set Work Area
  `{start?, end?, set?: begin|end (at CTI)}`
- `comp.setSwitch` Composition Switch
  `{switch: hideShy|motionBlur|frameBlending|draft3d, value?}`
- `comp.addMarker` Add Marker · Num*
  `{time?, comment?}`
- `comp.renderer` Renderer
  `{renderer?: advanced3d|classic3d (omit to read), value?}`
- `comp.info` Composition Info
  `{comp?}`
- `comp.revealInProject` Reveal Composition in Project · Layer > Reveal
  `{comp?}`
- `comp.cropToRegionOfInterest` Crop Comp to Region of Interest · Composition
  `{comp?}`
- `comp.cropToLayerBounds` Crop Comp to Selected Layer(s) Bounds · Composition
  `{layers?}`
- `comp.saveFrameAs` File · Composition > Save Frame As · Cmd+Alt+S
  `{path (.png; with queue: an output path or template), time?, scale?, queue?: bool (add a Render Queue item from the Frame Default templates instead of writing now)}`
- `comp.responsiveTime` Responsive Design · Time
  `{op: intro|outro|workArea}`
- `comp.saveFrameAsPsd` Photoshop Layers · Composition > Save Frame As
  `{path (.psd), comp?, time?, scale?} → a layered PSD: one layer per visible comp layer (blend mode, opacity) + the merged frame`
- `comp.saveFrameAsExr` ProEXR · Composition > Save Frame As
  `{path (.exr), comp?, time?, scale?} → a multi-layer OpenEXR: composite R,G,B,A + `[layer].R/G/B/A` per layer (linear, premultiplied)`
- `comp.vr.environments` VR Environments
  `{} → [{output, name, cubeMap, faces: [{face, comp, camera}], view: [x, y, z]}]`
- `comp.vr.setView` VR View Orientation
  `{comp? (an environment's output / cube map / face comp), orientation?: [x, y, z] degrees, pan?, tilt?, roll?} → turns the six face cameras together`
- `comp.flowchart` Composition Flowchart · Composition · Cmd+Shift+F11
- `comp.miniFlowchart` Composition Mini-Flowchart · Composition · Tab
- `comp.vr.createEnvironment` Create VR Environment · Composition > VR
  `{comp?, size?: face pixels (1024), position?: [x,y,z]}`
- `comp.vr.extractCubemap` Extract Cubemap · Composition > VR
  `{comp?, faceSize?}`
- `comp.openInEssentialGraphics` Open in Essential Graphics · Composition
  `{comp?} → makes it the Essential Graphics panel's Primary composition and shows the panel`

## layer

- `layer.newText` Text · Layer > New · Cmd+Alt+Shift+T
  `{text?, name? (layer name; default the text), position? [x,y], box? [x,y,w,h] (paragraph text, comp space), vertical?, edit? (start editing), font?, style?, size?, fill?, stroke?, applyFill?, applyStroke?, strokeWidth?, tracking?, leading?, baselineShift?, hScale?, vScale?, tsume?, fauxBold?, fauxItalic?, allCaps?, smallCaps?, baseline?, superscript?, subscript?, kerning?, ligatures?, justify?, indentLeft?, indentRight?, indentFirst?, spaceBefore?, spaceAfter?, direction?, composer?, hangingPunctuation?, strokeOverFill?} (attributes as layer.setText)`
- `layer.newSolid` Solid · Layer > New · Cmd+Y
  `{name?, color? #hex|[r,g,b], width?, height?, pixelAspect?}`
- `layer.newNull` Null Object · Layer > New · Cmd+Alt+Shift+Y
  `{name?}`
- `layer.newShape` Shape Layer · Layer > New
  `{kind?: rect|rounded|ellipse|star|polygon|none, name?, size?, fill?, stroke?, strokeWidth?, position?}`
- `layer.newAdjustment` Adjustment Layer · Layer > New · Cmd+Alt+Y
  `{name?}`
- `layer.settings` Layer Settings · Layer · Cmd+Shift+Y
  `{layer?, name?, color?, width?, height?, pixelAspect?, affectAll?: bool (default true; false gives this layer its own copy of a solid other layers use)} → {layer, solid?}`
- `layer.addItem` Add Footage to Comp · File · Cmd+/
  `{item: id|name, time? (s, the In point), index? (1-based stack position; default above the selected layer), position? ([x, y] comp px; default the centre), duration? (s, for a still)}`
- `layer.select` Select Layers
  `{layers: [id|name|#n], add?, toggle?}`
- `layer.selectNext` Select Next Layer · Cmd+ArrowDown
  `{add?}`
- `layer.selectPrevious` Select Previous Layer · Cmd+ArrowUp
  `{add?}`
- `layer.setSwitch` Layer Switch
  `{layers?, switch: video|audio|solo|lock|shy|collapse|quality|fx|frameBlend|motionBlur|adjustment|threeD|guide|preserveTransparency, value?}`
- `layer.rename` Rename · Enter
  `{layer?, name}`
- `layer.setComment` Layer Comment
  `{layers?, comment}`
- `layer.setBlendMode` Blending Mode
  `{layers?, mode?: Normal|Multiply|Screen|…, step?: ±1}`
- `layer.setTrackMatte` Track Matte
  `{layer?, matte: layer|null, kind?: alpha|alphaInverted|luma|lumaInverted}`
- `layer.setParent` Parent
  `{layers?, parent: layer|null}`
- `layer.timing` Layer Timing
  `{layers?, op?: moveInToTime|moveOutToTime|trimInToTime|trimOutToTime, delta?, start?, in?, out?, merge?}`
- `layer.slip` Slip Edit
  `{layers?, delta? (seconds) | frames?, merge?}`
- `layer.slipBack` Slip Edit 1 Frame Earlier · Alt+PageUp
  `{layers?}`
- `layer.slipForward` Slip Edit 1 Frame Later · Alt+PageDown
  `{layers?}`
- `layer.arrange` Arrange · Layer > Arrange
  `{layers?, to: front|forward|backward|back, index?, above?: layer}`
- `layer.precompose` Pre-compose · Layer · Cmd+Shift+C
  `{layers?, name?, mode?: move|leave, adjustDuration?, open?}`
- `layer.addMask` New Mask · Layer > Mask · Cmd+Shift+N
  `{layer?, shape?: rect|ellipse, rect? [x,y,w,h], mode?}`
- `layer.setMask` Mask Mode
  `{layer?, mask: index|uid|name, mode?, inverted?}`
- `layer.addShapeItem` Add (Shape)
  `{layer?, kind: group|rect|ellipse|star|polygon|fill|stroke|gfill|trim|repeater|round|offset|pucker|twist|zigzag|wiggle|merge, group?: uid|path} → {uid, path}`
- `layer.setText` Edit Text
  `{layer?, range?: [start, end] (characters; default all), text?, font?, style?, size?, fill?, stroke?, applyFill?, applyStroke?, strokeWidth?, tracking?, leading?: px|"auto", baselineShift? px, hScale? %, vScale? %, tsume? %, fauxBold?, fauxItalic?, allCaps?, smallCaps?, baseline?: normal|superscript|subscript, superscript?, subscript?, kerning?: metrics|optical|number, ligatures?, OpenType: discretionaryLigatures?, contextualAlternates?, stylisticAlternates?, stylisticSets?: [1–20], ss01…ss20?, swash?, titling?, ordinals?, fractions?, allSmallCaps?, figureStyle?: default|lining|oldStyle, figureWidth?: default|proportional|tabular, figures?, variations?: {tag: value} (variable font axes; null resets), justify?: left|center|right|justifyLeft|justifyCenter|justifyRight|justifyAll, indentLeft?, indentRight?, indentFirst?, spaceBefore?, spaceAfter?, direction?: ltr|rtl, composer?: everyLine|singleLine, hangingPunctuation?, strokeOverFill?, box?: [x,y,w,h]|null (layer space), vertical?}`
- `layer.transform` Transform · Layer > Transform
  `{layers?, op: reset|center|fit|fitWidth|fitHeight|flipH|flipV}`
- `layer.style.convertToEditable` Convert to Editable Styles · Layer > Layer Styles
  `{layers?}`
- `layer.style.showAll` Show All · Layer > Layer Styles
  `{layers?}`
- `layer.style.removeAll` Remove All · Layer > Layer Styles
  `{layers?}`
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
- `layer.style.add` Add Layer Style
  `{style: dropShadow|innerShadow|outerGlow|innerGlow|bevelEmboss|satin|colorOverlay|gradientOverlay|stroke (or display name), layers?}`
- `layer.style.remove` Remove Layer Style
  `{layer?, style: id|name|uid|layerStyles}`
- `layer.style.toggle` Toggle Layer Style
  `{layer?, style: id|name|uid|layerStyles, value?}`
- `layer.style.globalLight` Global Light
  `{comp?, angle? (deg), altitude? (deg, 0..90), time? (s)} · no values returns the current setting`
- `layer.style.list` List Layer Styles
  `{layer?}`
- `layer.autoOrient` Auto-Orient · Layer > Transform · Cmd+Alt+O
  `{mode: off|alongPath|towardsCamera|towardsPointOfInterest, layers?}`
- `layer.newLight` Light · Layer > New · Cmd+Alt+Shift+L
  `{kind?: Parallel|Spot|Point|Ambient (default Spot), name?, color?, intensity?, coneAngle?, coneFeather?, falloff?: None|Smooth|Inverse Square Clamped, radius?, falloffDistance?, castsShadows?, shadowDarkness?, shadowDiffusion?, position? [x,y,z], poi? [x,y,z]}`
- `layer.newCamera` Camera · Layer > New · Cmd+Alt+Shift+C
  `{name?, type?: oneNode|twoNode (default twoNode), preset?: 15mm|20mm|24mm|28mm|35mm|50mm|80mm|135mm|200mm, zoom?, focalLength? mm, angleOfView? deg, dof?, focusDistance?, aperture?, fStop?, blurLevel?, lockToZoom?, position? [x,y,z], poi? [x,y,z]}`
- `layer.cameraSettings` Camera Settings · Layer > Camera
  `{layer?, name?, type?, preset?, zoom?, focalLength?, angleOfView?, dof?, focusDistance?, aperture?, fStop?, blurLevel?, position?, poi?}`
- `layer.lightSettings` Light Settings
  `{layer?, name?, kind?, color?, intensity?, coneAngle?, coneFeather?, falloff?, radius?, falloffDistance?, castsShadows?, shadowDarkness?, shadowDiffusion?, position?, poi?}`
- `layer.newModel` 3D Model Layer
  `{item?: id or name of a 3D model footage item, path?: a .gltf, .glb or .obj file to import, name?, time? (s), index? (1-based stack position), position? ([x, y] comp px)}`
- `layer.new3dPrimitive` 3D Primitive
  `{kind: cube|sphere|plane|torus|cone|cylinder, name?, width?, height?, depth?, radius?, tubeRadius?, segments?, rings?, position? [x,y,z], baseColor?, metallic?, roughness?, emissive?, castsShadows?, acceptsShadows?, acceptsLights?}`
- `layer.newPrimitive` New 3D Primitive
  `{kind: cube|sphere|plane|torus|cone|cylinder, name?, width?, height?, depth?, radius?, tubeRadius?, segments?, rings?, position? [x,y,z], baseColor?, metallic?, roughness?, emissive?, castsShadows?, acceptsShadows?, acceptsLights?}`
- `layer.environment` Environment Layer · Layer
  `{layers?, on?}`
- `layer.addTextAnimator` Animate Text
  `{layer?, properties: [anchor|position|scale|skew|rotation|opacity|transformAll|lineAnchor|lineSpacing|characterOffset|characterValue|blur|fillColor|fillHue|fillSaturation|fillBrightness|fillOpacity|strokeColor|strokeHue|strokeSaturation|strokeBrightness|strokeOpacity|strokeWidth|tracking|perChar3d], name?}`
- `layer.addTextAnimatorProperty` Add Property
  `{layer?, animator?: uid|index, property: (see layer.addTextAnimator)}`
- `layer.addTextSelector` Add Text Selector
  `{layer?, animator?: uid|index, kind: range|wiggly|expression}`
- `layer.enablePerChar3D` Enable Per-character 3D
  `{layer?, enabled?: bool, toggle?: bool}`
- `layer.applyTextPreset` Apply Text Animation Preset
  `{layer?, preset: typewriter|fadeUpCharacters|bounceInWords|trackingIn|scramble|blurIn|jitter|dropInLines}`
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
- `layer.updateMarkersFromSource` Update Markers From Source · Layer > Markers
  `{layers?}`
- `layer.mask.motionBlur` Motion Blur
  `{layer?, mask?, mode: sameAsLayer|on|off}`
- `layer.mask.featherFalloff` Feather Falloff
  `{layer?, mask?, mode: smooth|linear}`
- `layer.mask.hideLocked` Hide Locked Masks · Layer > Mask
  `{value?}`
- `layer.tree` Layer Property Tree
  `{layer?, comp?, depth?, time? (comp s)} → nodes with `path``
- `layer.quality` Quality
  `{layers?, quality: best|draft|wireframe}`
- `layer.sampling` Sampling
  `{layers?, sampling: bilinear|bicubic}`
- `layer.frameBlending` Frame Blending
  `{layers?, mode: off|frameMix|pixelMotion}`
- `layer.hideOtherVideo` Hide Other Video · Layer > Switches · Cmd+Alt+Shift+V
  `{layers?}`
- `layer.showAllVideo` Show All Video · Layer > Switches
  `{comp?}`
- `layer.unlockAll` Unlock All Layers · Layer > Switches · Cmd+Shift+L
  `{comp?}`
- `layer.expressions` Enable/Disable Expressions
  `{layers?, enabled: bool}`
- `layer.setTransform` Transform Value
  `{layers?, prop: anchor|position|scale|orientation|rotation|opacity, value}`
- `layer.centerAnchor` Center Anchor Point in Layer Content · Layer > Transform · Cmd+Alt+Home
  `{layers?}`
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
- `layer.addMarker` Add Marker · Layer > Markers
  `{layers?, time?, comment?}`
- `layer.markersLock` Lock Markers · Layer > Markers
  `{layers?, value?}`
- `layer.deleteAllMarkers` Delete All Markers · Layer > Markers
  `{layers?}`
- `layer.trackMatte` Track Matte
  `{layers?, op: none|alpha|alphaInverted|luma|lumaInverted|above|below}`
- `layer.openSource` Open Layer Source · Layer
  `{layer?}`
- `layer.revealInFinder` Reveal in Finder · Layer
  `{layer?}`
- `layer.revealSource` Reveal Layer Source in Project · Layer > Reveal
  `{layer?}`
- `layer.revealExpressionErrors` Reveal Expression Errors · Layer > Reveal
  `{comp?}`
- `layer.sequence` Sequence Layers · Animation > Keyframe Assistant
  `{layers? (in order), overlap?: bool, duration? (s), transition?: off|dissolveFront|crossDissolve}`
- `layer.create` Create
  `{op: editableText|shapesFromText|masksFromText|shapesFromVector, layers?}`
- `layer.style.options` Layer Style Options · Layer > Layer Styles
  `{layer?, style?: blendingOptions|dropShadow|…}`
- `layer.openLayer` Open Layer · Layer
  `{layer?}`
- `layer.autoTrace` Auto-trace · Layer
  `{layer?, timeSpan?: currentFrame|workArea, channel?: alpha|red|green|blue|luminance, invert?, blur? (px, 1), tolerance? (px, 1), threshold? (%, 50), minimumArea? (px, 10), cornerRoundness? (%, 50), applyToNewLayer?}`
- `layer.sceneEditDetection` Scene Edit Detection · Layer
  `{layer?, mode?: markers|split|splitPrecompose, threshold? (0…1, 0.25), wait?: bool}`
- `layer.alignVideoToData` Align Video to Data · Layer > Time
  `{layer?, data?: data footage id|name, key?: time field, videoStart?: ISO date-time | hh:mm:ss | timecode | seconds (default: file creation time), dataStart? (comp s of the first sample, 0)}`
- `layer.align` Align Layers
  `{edge: left|hcenter|right|top|vcenter|bottom, to?: composition|selection (default composition), layers?}`
- `layer.distribute` Distribute Layers
  `{mode: left|hcenter|right|top|vcenter|bottom (edges or centres), layers?}`
- `layer.newContentAwareFill` Content-Aware Fill Layer · Layer > New
  `{layer?, method?: object|surface|edgeBlend, range?: workArea|entire, alphaExpansion? (px), lightingCorrection?: none|subtle|moderate|strong, referenceLayer?, referenceTime? (s), outputDir?, wait?: bool}`

## prop

- `prop.get` Get Property
  `{layer?, path|prop, time? (comp s)}`
- `prop.set` Set Property Value
  `{layer?, path|prop, value, time?, merge?}`
- `prop.toggleAnimation` Toggle Stopwatch
  `{layer?, path|prop, value?}`
- `prop.addKey` Add Keyframe
  `{layer?, path|prop, time?, value?}`
- `prop.toggleKey` Add or Remove Keyframe at Current Time
  `{layer?, path|prop}`
- `prop.setExpression` Add Expression · Animation · Alt+Shift+=
  `{layer?, path|prop, expression?, enabled?}`
- `prop.reset` Reset Property
  `{layer?, path|prop, default?}`
- `prop.select` Select Property
  `{layer?, path|prop, add?, selectKeys?}`
- `prop.convertExpressionToKeyframes` Convert Expression to Keyframes · Animation > Keyframe Assistant
  `{layer?, path|prop}`
- `prop.renameGroup` Rename Property Group
  `{layer?, prop: uid, name}`
- `prop.setGroupEnabled` Enable Property Group
  `{layer?, prop: uid, value?}`
- `prop.removeGroup` Delete Property Group
  `{layer?, prop: uid}`
- `prop.duplicateGroup` Duplicate Property Group
  `{layer?, prop: uid} → {prop: new uid}`
- `prop.moveGroup` Reorder Property Group
  `{layer?, prop: uid, index (1-based among its siblings)}`
- `prop.separateDimensions` Separate Dimensions · Animation
  `{layer?, value?}`
- `prop.pickWhip` Pick Whip (Link Property)
  `{layer?, path|prop, target: {layer, path|prop}} → sets an AE reference expression`

## keys

- `keys.toggleTransform` Add or Remove Transform Keyframe
  `{layers?, prop: anchor|position|scale|rotation|opacity} (Alt+Shift+A/P/S/R/T)`
- `keys.select` Select Keyframes
  `{keys: [{layer, prop: uid | path (or `path`), time (layer s)}], add?, toggle?: bool (Shift+click: in or out of the selection)}`
- `keys.move` Move Keyframes
  `{delta (s), merge?}`
- `keys.delete` Delete Keyframes
- `keys.easyEase` Easy Ease · Animation > Keyframe Assistant · F9
  `{which?: both|in|out}`
- `keys.easyEaseIn` Easy Ease In · Animation > Keyframe Assistant · Shift+F9
- `keys.easyEaseOut` Easy Ease Out · Animation > Keyframe Assistant · Cmd+Shift+F9
- `keys.toggleHold` Toggle Hold Keyframe · Animation · Cmd+Alt+H
- `keys.interpolation` Keyframe Interpolation · Animation · Cmd+Alt+K
  `{interpolation?: linear|bezier|continuousBezier|autoBezier|hold, in?|out?: linear|bezier|hold, autoBezier?, continuous?, spatial?: linear|bezier|continuousBezier|autoBezier, roving?}`
- `keys.velocity` Keyframe Velocity · Animation · Cmd+Shift+K
  `{inSpeed?, inInfluence? %, outSpeed?, outInfluence? % (numbers or per-dimension arrays), continuous?}`
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
- `keys.set` Edit Keyframe
  `{layer?, path|prop, time (layer s), newTime?, value?, merge?}`
- `keys.selectEqual` Select Equal Keyframes
- `keys.selectPrevious` Select Previous Keyframes
- `keys.selectFollowing` Select Following Keyframes
- `keys.setEase` Set Keyframe Ease
  `{layer?, path|prop, time (layer s), side: in|out, dim?, speed?, influence? %, merge?}`
- `keys.timeReverse` Time-Reverse Keyframes · Animation > Keyframe Assistant
- `keys.info` Keyframe Info
  `{keys?: [{layer, prop, time}]}`
- `keys.wiggle` Wiggler
  `{apply?: spatial|temporal, noise?: smooth|jagged, dimensions?: one|same|independent, dimension? (index for `one`), frequency? (keys/s, 5), magnitude?, seed?}`
- `keys.smooth` Smoother
  `{tolerance? (property units, 1)}`
- `keys.exponentialScale` Exponential Scale · Animation > Keyframe Assistant
  `{} · two selected Scale keys`
- `keys.setLabel` Keyframe Label
  `{label: name|none|0–16}`
- `keys.selectLabelGroup` Select Keyframe Label Group
  `{scope: selected|all|visibleSelected|visibleAll, visible?: [prop uid]}`
- `keys.transform` Transform Keyframes
  `{timeScale?, timeAnchor? (comp s), timeOffset? (s), valueScale?, valueAnchor?, valueOffset?, dim? | dims?: [d…], merge?, fromStart?: bool (with merge: values are the whole transform since the drag started)}`
- `keys.setSpatialTangents` Edit Spatial Tangents
  `{layer?, path|prop, time (layer s), in?: [dx,dy,dz?], out?, break?, merge?}`
- `keys.audioToKeyframes` Convert Audio to Keyframes · Animation > Keyframe Assistant
  `{comp?}`
- `keys.rpfCameraImport` RPF Camera Import · Animation > Keyframe Assistant
  `{path: .json|.csv camera data, comp?}`

## anim

- `anim.savePreset` Save Animation Preset · Animation
  `{path (.ecpreset), name?} · saves the selected properties/effects`
- `anim.applyPreset` Apply Animation Preset · Animation
  `{path | preset, layers?}`
- `anim.addKeyframe` Add Keyframe · Animation
  `{} · keys the selected properties at the CTI`
- `anim.reveal` Reveal Properties
  `{kind: keyframes|animation|modified}`
- `anim.browsePresets` Browse Presets · Animation
- `anim.applyRecentPreset` Recent Animation Presets
  `{index? | path?, layers?}`
- `anim.clearRecentPresets` Clear Recent Presets · Animation > Recent Animation Presets

## time

- `time.set` Go to Time · View · Alt+Shift+J
  `{time? (s) | frame? | timecode?}`
- `time.nextFrame` Next Frame · PageDown
- `time.previousFrame` Previous Frame · PageUp
- `time.forward10` Forward 10 Frames · Shift+PageDown
- `time.back10` Back 10 Frames · Shift+PageUp
- `time.step` Step Frames
  `{frames}`
- `time.start` Go to Start · Home
- `time.end` Go to End · End
- `time.layerIn` Go to Layer In Point · I
- `time.layerOut` Go to Layer Out Point · O
- `time.nextKey` Go to Next Keyframe or Marker · K
  `{visible?: [{layer, prop}] (only these properties' keys, as the Timeline shows them)}`
- `time.previousKey` Go to Previous Keyframe or Marker · J
  `{visible?: [{layer, prop}]}`
- `time.go` Go To
  `{to: start|end|workStart|workEnd|layerIn|layerOut|nextKey|prevKey, prop?: uid, visible?: [{layer, prop}]}`

## markers

- `markers.list` List Markers
  `{layer? (omit for composition markers)}`
- `markers.nested` Nested Comp Markers
  `{layer: a precomp layer, comp?} · its comp's markers at their times in this comp (as on the layer bar)`
- `markers.set` Marker Settings
  `{layer? (omit: comp marker), index? (0-based) | at? (comp s) | new?: true, time? (comp s), duration? (s), comment?, chapter?, url?, frameTarget?, cuePoint?: {name, navigation?, params?: [[name, value]…]} | null, protected?, label?}`
- `markers.delete` Delete Marker
  `{layer?, index? | at? (comp s)}`
- `markers.convert` Convert Marker
  `{layer? (layer marker → comp marker), index? | at?, toLayer? (comp marker → layer marker)}`

## effect

- `effect.apply` Apply Effect
  `{effect: id|name (e.g. Gaussian Blur), layers?}`
- `effect.pickColor` Pick Effect Colour
  `{layer?, effect?: index|uid|name, param: id (e.g. screenColour) | prop: uid, x, y (layer px), average?: bool (5×5), time?} · the colour of the effect's input there`
- `effect.applyLast` Last Effect · Effect · Cmd+Alt+Shift+E
  `{layers?}`
- `effect.removeAll` Remove All · Effect · Cmd+Shift+E
  `{layers?}`
- `effect.remove` Remove Effect
  `{layer?, effect: index|uid|name}`
- `effect.toggle` Toggle Effect
  `{layer?, effect, value?}`
- `effect.reorder` Reorder Effect
  `{layer?, effect, index}`
- `effect.duplicate` Duplicate Effect
  `{layer?, effect}`
- `effect.copy` Copy Effects
  `{layer?, effect? | effects?: [uid|name|index]} (default: the selected effects)`
- `effect.paste` Paste Effects
  `{layers?} · adds the copied effects to the layers`
- `effect.reset` Reset Effect
  `{layer?, effect}`
- `effect.list` List Effects
  `{filter?}`
- `effect.plugins.list` List Effect Plug-ins
  `{} → {api, wasm, plugins: [{id, name, category, version, author, source, params}]}`
- `effect.plugins.load` Load Effect Plug-in
  `{path (.wasm / .wat plug-in, API v1) | folder (loads every .wasm in it)}`
- `effect.manage` Manage Effects · Effect
- `effect.editDropdown` Edit Dropdown Menu
  `{layer, path?|prop?, items: [string]}`

## mask

- `mask.new` New Mask from Points
  `{layer?, vertices: [[x,y]…], inTangents?, outTangents?, closed?, mode?}`
- `mask.addVertex` Add Mask Vertex
  `{layer?, mask, point: [x,y], in?, out?, index?}`
- `mask.setVertex` Set Mask Vertex
  `{layer?, mask, index, point?, in?, out?, merge?}`
- `mask.setClosed` Closed · Layer > Mask and Shape Path
  `{layer?, mask, closed?}`
- `mask.selectVertices` Select Mask Vertices
  `{vertices: [{layer, mask, index}], add?, toggle?}`
- `mask.moveVertices` Move Mask Vertices
  `{vertices?: [{layer, mask, index}], delta: [dx,dy], merge?}`
- `mask.deleteVertices` Delete Mask Vertices
  `{vertices?}`
- `mask.convertVertex` Convert Vertex
  `{layer?, mask: uid (mask or shape Path item), index, smooth?}`
- `mask.insertVertex` Add Vertex
  `{layer?, mask: uid (mask or shape Path item), segment, t?: 0..1}`
- `mask.featherPoint.add` Add Mask Feather Point
  `{layer?, mask, segment?, t?: 0..1, point?: [x,y] (layer space), radius? (px, negative = inner), tension? (%)} → {index}`
- `mask.featherPoint.set` Move Mask Feather Point
  `{layer?, mask, index, segment?, t?, point? (slide along the path), radius?, toward?: [x,y] (radius from the drag point, layer space), tension? (%), merge?}`
- `mask.featherPoint.remove` Delete Mask Feather Point
  `{layer?, mask, index? | all?}`
- `mask.featherPoint.list` Mask Feather Points
  `{layer?, mask}`
- `mask.remove` Remove Mask
  `{layer?, mask}`
- `mask.removeAll` Remove All Masks
  `{layer?}`
- `mask.interpolate` Apply Mask Interpolation
  `{layer?, mask?, times?: [from, to] (s; default the selected Mask Path keys), keyframeRate?: n|auto, keyframeFields?, linearVertexPaths?, bendingResistance? (0-100), quality? (0-100), addVertices?: n|false, addVerticesUnit?: pixels|total|percent, matchingMethod?: auto|curve|polyline, oneToOne?, firstVerticesMatch?}`
- `mask.interpolationOptions` Mask Interpolation Options
  `{keyframeRate?: n|auto, keyframeFields?, linearVertexPaths?, bendingResistance?, quality?, addVertices?: n|false, addVerticesUnit?: pixels|total|percent, matchingMethod?: auto|curve|polyline, oneToOne?, firstVerticesMatch?}`

## path

- `path.rotoBezier` RotoBezier · Layer > Mask and Shape Path
  `{layer?, mask?, value?}`
- `path.convertToBezier` Convert To Bezier Path · Layer > Mask and Shape Path
  `{layer?, mask?}`
- `path.groupShapes` Group Shapes · Layer · Cmd+G
  `{layer?, items?: [uid]}`
- `path.ungroupShapes` Ungroup Shapes · Layer · Cmd+Shift+G
  `{layer?, items?: [group uid]}`
- `path.setFirstVertex` Set First Vertex · Layer > Mask and Shape Path
  `{layer?, mask?: uid, index?}`
- `path.freeTransform` Free Transform Points · Layer > Mask and Shape Path
  `{layer?, mask?, scale?: % | [x%, y%], rotation?: deg, offset?: [dx, dy], anchor?: [x, y]}`

## paths

- `paths.pointsFollowNulls` Points Follow Nulls
  `{layer?, path?|prop?, pins?: [uid | name…]} → a null per vertex (or Position / Advanced puppet pin); the path (pins) follow them (expressions)`
- `paths.nullsFollowPoints` Nulls Follow Points
  `{layer?, path?|prop?, pins?: [uid | name…]} → a null per vertex (or puppet pin) following it (Position expressions)`
- `paths.tracePath` Trace Path
  `{layer?, path?|prop?, loop?: bool} → a null moving along the path (Progress slider)`

## shape

- `shape.newPath` Pen Tool (Shape Path)
  `{layer?, vertices: [[x,y]…], inTangents?, outTangents?, closed?, space?: comp|layer, fill?: [r,g,b]|#hex|false, stroke?, strokeWidth?, name?}`
- `shape.dashes.add` Add Dash or Gap
  `{layer?, prop?: stroke uid}`
- `shape.dashes.remove` Remove Dash or Gap
  `{layer?, prop?: stroke uid}`
- `shape.stroke.taper` Stroke Taper
  `{layer?, prop?: stroke uid, units?: pixels|percent, startLength?, endLength?, startWidth? (%), endWidth? (%), startEase? (%), endEase? (%)}`
- `shape.stroke.wave` Stroke Wave
  `{layer?, prop?: stroke uid, amount? (%), units?: pixels|cycles, wavelength?, cycles?, phase? (°)}`

## text

- `text.animatorFontAxes` Variable Font Axes · Animation > Animate Text
  `{layer?, axis?: tag|name (omit to list the font's axes), animator?: uid|index}`
- `text.presets` Text Animation Presets
- `text.animatorProperties` Text Animator Properties
- `text.edit` Edit Text
  `{layer?, select?: [anchor, caret]|"none", caret?, created?} → {layer, anchor, caret}`
- `text.endEdit` Exit Text Editing
- `text.fontFeatures` OpenType Features
  `{layer?, font?, style?} → {family, style, features: [tags], options: {smallCaps, superscript, stylisticSets: [n], fractions, …}} (what the font sets with its own glyphs)`
- `text.fonts` List Fonts
  `{query?, rescan?} → {count, families: [{family, styles, origin: bundled|system|user, nativeName?}], added} (rescan picks up fonts installed since launch)`
- `text.setSelection` Set Text Selection
  `{layer?, start, end?} | {anchor, caret} | {select: "all"} (character indices)`
- `text.moveCaret` Move Text Caret
  `{to: left|right|wordLeft|wordRight|lineStart|lineEnd|up|down|paraStart|paraEnd|start|end, extend?}`
- `text.insert` Type Text
  `{layer?, text, range?: [start, end], merge?} (replaces the selection)`
- `text.delete` Delete Text
  `{layer?, direction?: backward|forward, word?, range?: [start, end], merge?}`
- `text.addSelector` Add Text Selector
  `{layer?, animator?: uid|index, kind: range|wiggly|expression}`
- `text.removeAllAnimators` Remove All Text Animators · Animation
  `{layers?}`

## camera

- `camera.fromView` Create Camera from 3D View · Layer > Camera
  `{comp?}`
- `camera.orbit` Orbit Camera
  `{yaw|dx deg, pitch|dy deg, layer?, merge?} · current 3D view (camera layer in Active Camera)`
- `camera.pan` Pan Camera
  `{dx, dy (comp px), layer?, merge?}`
- `camera.dolly` Dolly Camera
  `{amount (px, + = forward), layer?, merge?}`
- `camera.stereoRig` Create Stereo 3D Rig · Layer > Camera
  `{comp?, configuration?: stereoPair|centerRight|centerLeft, sceneDepth? (% of comp width, default 3), convergence?: bool, convergenceOf?: poi|zoom, zOffset?, view3d?: 3D Glasses view index (default 5 Balanced Colored Red Blue)}`
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
  `{layer?, effect?: uid|name|index, wait?: block until done}`
- `camera.cancel` Cancel
- `camera.solveStatus` 3D Camera Tracker Status
  `{layer?, effect?}`
- `camera.points` 3D Camera Tracker Points
  `{layer?, effect?, time?: comp seconds} → visible solved points`
- `camera.selectPoints` Select Track Points
  `{points: [id], add?, toggle?}`
- `camera.createFromSolve` Create from Camera Solve
  `{kind: text|solid|null|shadowCatcher|camera, points?: [id] (default: selected), target?: {center, normal, size?} (comp world), multiple?, layer?, effect?}`
- `camera.create` Create Camera
  `{layer?, effect?}`
- `camera.setGroundPlane` Set Ground Plane and Origin
  `{points?: [id] (default: selected), layer?, effect?}`
- `camera.deletePoints` Delete Selected Points
  `{points?: [id] (default: selected), layer?, effect?, wait?}`

## light

- `light.fromModel` Create Lights from 3D Model · Layer > Light
  `{comp?, layers?: 3D model layers (default selected)}`
- `light.controlWithCamera` Control Light with Camera · Layer > Light
  `{comp?, layers?: lights (default selected), camera? (default active), on?: bool (default toggle)}`
- `light.environmentBackground` Create Environment Light Background Layer · Layer > Light
  `{comp?, layer?: environment light (default selected / first)}`

## material

- `material.set` Material Options
  `{layers?, castsShadows?: off|on|only, acceptsShadows?: off|on|only, acceptsLights?, appearsInReflections?, lightTransmission?, ambient?, diffuse?, specularIntensity?, specularShininess?, metal?, baseColor?, metallic?, roughness?, emissive?, time?}`
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
  `{layer?, tracker?, point: n (1-based), center?: [x,y], featureSize?: [w,h], searchOffset?: [x,y], searchSize?: [w,h], attachOffset?: [x,y], move?: [dx,dy], time? (s)}`
- `track.analyze` Analyze
  `{layer?, tracker?, direction?: forward|backward|frameForward|frameBackward, start? (s), end? (s), wait?: block until done}`
- `track.stop` Stop Analysis
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
- `track.mask` Track Mask · Animation
  `{layer?, mask?: uid|index|name, method?: position|positionScale|positionScaleRotation|positionScaleRotationSkew|perspective|faceOutline|faceDetailed, direction?: forward|backward|frameForward|frameBackward, start? (s), end? (s), wait?: block until done}`
- `track.maskMethod` Mask Tracking Method
  `{method: position|positionScale|positionScaleRotation|positionScaleRotationSkew|perspective|faceOutline|faceDetailed}`
- `track.extractFaceMeasurements` Extract & Copy Face Measurements
  `{layer?} → keys a Face Measurements effect from the layer's Face Track Points (one key per tracked frame) and copies those keys`
- `track.warpStabilizer` Warp Stabilizer VFX · Animation
  `{layer?, wait?}`
- `track.camera` Track Camera · Animation
  `{layer?, shotType?: fixed|variable|specify, aov?: degrees, solveMethod?: auto|typical|flat|tripod, detailed?, lensDistortion?: solve k1/k2, undistort?: render undistorted, wait?}`

## warp

- `warp.analyze` Analyze
  `{layer?, effect?: uid|name|index, wait?: block until done}`
- `warp.cancel` Cancel
- `warp.status` Warp Stabilizer Status
  `{layer?, effect?}`

## roto

- `roto.models` Roto Brush Models
- `roto.model.select` Use Roto Brush Model
  `{id: classical|mobilesam, wait?} (Roto Brush 2.0 / 3.0 use it; wait: loaded before returning)`
- `roto.model.install` Install Roto Brush Model
  `{path: weights file, id?, wait?} · verified against the registry's SHA-256`
- `roto.model.download` Download Roto Brush Model
  `{id, wait?} · the official weights, verified (desktop; uses the system curl)`
- `roto.model.remove` Remove Roto Brush Model
  `{id}`
- `roto.stroke` Roto Brush Stroke
  `{layer?, kind?: fg|bg|refine|refineErase, points: [[x, y], …] (layer pixels), frame? (layer frame; default current), radius? (layer px; default the tool's diameter / 2), effect?}`
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
  `{layer?, kind?: brush|clone|eraser, points: [[x,y,pressure?],…] (layer space), time? (s) | frame?, duration? (s, drawing time for Write On), color?: [r,g,b,a], diameter?, angle?, hardness?, roundness?, spacing?, opacity?, flow?, mode?, channels?: RGBA|RGB|Alpha, durationMode?: constant|writeOn|singleFrame|custom, customFrames?, eraseMode?: layerSourceAndPaint|paintOnly|lastStrokeOnly, cloneSource?: layer id, clonePosition?: [x,y], cloneTimeShift? (s), lockSourceTime?, cloneTime? (s), sizePressure?, minSize?, opacityPressure?, flowPressure?}`
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
  `{layer?, pin: uid | name, position?, scale? (%), rotation? (°), amount? (%), extent? (px), inFront?, time? | frame?, merge?}`
- `puppet.removePin` Delete Puppet Pin
  `{layer?, pin?: uid | name, pins?: [uid | name…]} (default: the selected pins)`
- `puppet.selectPins` Select Puppet Pins
  `{layer?, pins: [uid | name…] ([] deselects), add?, toggle?}`
- `puppet.mesh` Puppet Mesh Options
  `{layer?, mesh?: uid, density?, expansion?, triangles?, showMesh?, time?, merge?}`
- `puppet.info` Puppet Mesh Info
  `{layer?, time? | frame?}`
- `puppet.recordPin` Record Puppet Pin
  `{layer?, pin: uid | name, samples: [[t, x, y]…] (t = seconds since the drag began, layer space), pins?: [uid | name…] (more pins that move by the same displacement), start? (s, default current time), speed? (%, Record Options), smoothing? (Record Options)}`
- `puppet.follow` Follow-Through
  `{layer?, leader?: uid | name (default: the first selected pin), pins?: [uid | name…] (default: the other selected pins), delay? (s, 0.1), amount? (%, 100), cascade? (true: the k-th nearest pin trails k × delay)}`
- `puppet.recordOptions` Record Options
  `{speed? (%, 100), smoothing? (0-100), useDraftDeformation?, showMesh?}`

## liquify

- `liquify.stroke` Liquify Stroke
  `{layer?, effect?: uid, tool?: warp|turbulence|twirlClockwise|twirlCounterclockwise|pucker|bloat|shiftPixels|reflection|clone|reconstruction|freeze|thaw, points: [[x,y],…] (layer space), size?, pressure? (1-100), jitter? (1-100), cloneOffset?: [dx,dy], time? (s) | frame?}`
- `liquify.clear` Clear Liquify Mesh
  `{layer?, effect?: uid}`

## contentFill

- `contentFill.set` Content-Aware Fill Settings
  `{method?: object|surface|edgeBlend, range?: workArea|entire, alphaExpansion? (px), lightingCorrection?: none|subtle|moderate|strong, referenceLayer?: layer|null, referenceTime? (s)}`
- `contentFill.generate` Generate Fill Layer
  `{layer?, method?: object|surface|edgeBlend, range?: workArea|entire, alphaExpansion? (px), lightingCorrection?: none|subtle|moderate|strong, referenceLayer?, referenceTime? (s), outputDir?, wait?: bool}`

## face

- `face.models` Face Tracking Models
- `face.model.select` Use Face Tracking Model
  `{id: classical|mediapipe-face, wait?} (face tracking uses it; wait: loaded before returning)`
- `face.model.install` Install Face Tracking Model
  `{path: model file, id?, wait?} · verified against the registry's SHA-256`
- `face.model.download` Download Face Tracking Model
  `{id, wait?} · the official model, verified (desktop; uses the system curl)`
- `face.model.remove` Remove Face Tracking Model
  `{id}`

## essential

- `essential.setPrimary` Primary Composition
  `{comp}`
- `essential.setName` Essential Graphics Name
  `{comp?, name}`
- `essential.addProperty` Add Property to Essential Graphics · Animation
  `{comp?, layer?, path?|prop?: uid (default: the selected properties), name?, group?: control id, index?, as?: font (Source Text font family/style/size) | scale (uniform Scale), mirror?: bool (a property already present is added as a mirror; default true)}`
- `essential.addMirror` Add Mirror
  `{comp?, control, name?, group?, index?}`
- `essential.linkProperty` Link Property to Control
  `{comp?, control, layer?, path?|prop? (default: the selected property)}`
- `essential.unlinkProperty` Unlink Property
  `{comp?, control, layer?, path?|prop? (default: the selected property)}`
- `essential.addMedia` Add Media Replacement
  `{layer, name?, group?}`
- `essential.addGroup` Add Group
  `{comp?, name?, index?}`
- `essential.addComment` Add Comment
  `{comp?, text?, group?}`
- `essential.rename` Rename Control
  `{comp?, control, name}`
- `essential.remove` Remove Control
  `{comp?, control}`
- `essential.move` Move Control
  `{comp?, control, group?: id (null = top level), index?}`
- `essential.soloSupported` Solo Supported Properties
  `{on?}`
- `essential.exportTemplate` Essential Graphics Template · File > Export
  `{comp?, path (.ectemplate), name?}`
- `essential.importTemplate` Essential Graphics Template · File > Import
  `{path (.ectemplate), comp?, addToComp?: bool}`
- `essential.set` Set Essential Property
  `{layer, control: id|name, value | item (media)}`
- `essential.pushToComp` Push Override Values to Source
  `{layer, control? (default: all overridden)}`
- `essential.revert` Revert
  `{layer, control? (default: all overridden)}`
- `essential.list` Essential Graphics controls
  `{comp?, supported?: bool}`
- `essential.canAdd` Can Add Property to Essential Graphics
  `{layer, path|prop, as?} → {ok, type?, reason?}`
- `essential.instance` Essential Properties of a precomp layer
  `{layer}`
- `essential.templateInfo` Read a template's manifest
  `{path}`

## expr

- `expr.errors` Expression errors (the error bar)
  `{comp?, time? (s)}`
- `expr.languageMenu` Expression Language menu

## script

- `script.run` Run Script
  `{code (JavaScript: app, app.project, CompItem, Layer, Property… like After Effects scripting), name?, console? (Script Console context)} → {ok, result, output, error: {message, line, column} | null}`

## scriptui

- `scriptui.list` List Script Windows
  `{} → [{window, title, kind: dialog|palette|window|panel, script, modal, size}]`
- `scriptui.get` Script Window Controls
  `{window?: id | title} → {id, title, kind, root: {id, type, name, text, value, checked, items, selection, bounds, enabled, draw (onDraw paint list), children…}}`
- `scriptui.click` Click Script Window Control
  `{window?: id | title, widget: id | "#id" | properties.name | text} (buttons, checkboxes, radio buttons, tabs)`
- `scriptui.set` Set Script Window Control
  `{window?, widget, value: text | number | bool | item index | item text, changing?: bool (a live update: each keystroke / slider step; fires onChanging only)} (edit text, sliders, checkboxes, lists) → fires onChanging / onChange`
- `scriptui.close` Close Script Window
  `{window?, result? (a dialog's show() returns it; default 2 = Cancel)}`

## render

- `render.backend` Video Rendering and Effects
  `{backend?: gpu|cpu}`
- `render.addOutputModule` Add Output Module · Composition
  `{item?|index? (default: the last item), format?, output?, channels?, quality?, bitrate?, proresProfile?, audio?}`
- `render.preRender` Pre-render · Composition
  `{comp?, output?: path|template, format?}`
- `render.saveCurrentPreview` Save Current Preview · Composition
  `{comp?, path}`

## renderQueue

- `renderQueue.add` Add to Render Queue · Composition · Cmd+M
  `{comp?: id|name, template?: Render Settings template, format?: h264|hevc|av1|prores|webm|png|jpeg|tiff|exr|gif|wav|aiff, output?: path|template, log?: errorsOnly|plusSettings|plusPerFrameInfo, quality?: best|draft|1-100 (jpeg, webm), resolution?: full|half|third|quarter|scale, proxyUse?, effects?, solo?, guideLayers?, colorDepth?, frameBlending?, fieldRender?, pulldown?, motionBlur?, timeSpan?: workArea|comp|custom, start?: s, end?: s, duration?: s, frameRate?: fps|null, skipExisting?: bool, storageOverflow?: bool, channels?: rgb|rgba|alpha, color?, bitrate?: kbps, webmBitrate?, keyframeInterval?, proresProfile?, profile?, level?, rateControl?, webmCodec?, audioBitrate?, opusApplication?, crop?, resize?, includeProjectLink?, audio?: auto|on|off, sampleRate?, audioChannels?, audioFormat?, loop?: bool (values as in setRenderSettings / setOutputModule)}`
- `renderQueue.remove` Remove from Render Queue
  `{item?: id, index?: n}`
- `renderQueue.setRender` Render Queue: Render Checkbox
  `{item?|index?, render?: bool (toggles)}`
- `renderQueue.setRenderSettings` Render Settings
  `{item?|index?, template?: name, quality?: best|draft, resolution?: full|half|third|quarter|scale, proxyUse?: current|all|comp|none, effects?: current|allOn|allOff, solo?: current|allOff, guideLayers?: current|allOff, colorDepth?: current|8|16|32, frameBlending?: current|onForChecked|offForAll, fieldRender?: off|upper|lower, pulldown?: off|WSSWW|SSWWW|SWWWS|WWWSS|WWSSW, motionBlur?: bool|current|onForChecked|offForAll, timeSpan?: workArea|comp|custom, start?: s, end?: s, duration?: s, frameRate?: fps|null, skipExisting?: bool, storageOverflow?: bool}`
- `renderQueue.setOutputModule` Output Module Settings
  `{item?|index?, module?: n (1 = first), template?: name, format?: h264|hevc|av1|prores|webm|png|jpeg|tiff|exr|gif|wav|aiff, channels?: rgb|rgba|alpha, color?: straight|premultiplied, quality?: 1-100, bitrate?: kbps, webmBitrate?: bool, keyframeInterval?: frames (0 = auto), proresProfile?: proxy|lt|standard|hq|4444|4444xq, profile?: main|main10 (HEVC/AV1), level?: auto|4.1, rateControl?: bitrate|quality, webmCodec?: vp9|av1, audioBitrate?: Opus kbps, opusApplication?: audio|voice, crop?: bool|{useRoi?, roi?, top?, left?, bottom?, right?}, cropTop?, cropLeft?, cropBottom?, cropRight?, resize?: bool|{preset?, width?, height?, lockAspect?, quality?: low|high}, resizeWidth?, resizeHeight?, postRenderAction?: none|import|importAndReplace|setProxy, includeProjectLink?: bool, audio?: auto|on|off, sampleRate?, audioChannels?: mono|stereo, audioFormat?: 16|24|32, loop?: bool, output?}`
- `renderQueue.setOutput` Output To
  `{item?|index?, module?: n, path: file path or template like [compName].[fileExtension]}`
- `renderQueue.move` Move in Render Queue
  `{item?|index?, to: n (1-based)}`
- `renderQueue.duplicate` Duplicate Render Item
  `{item?|index?}`
- `renderQueue.render` Render
  `{wait?: bool (default true; the UI renders in the background)}`
- `renderQueue.stop` Stop Rendering
- `renderQueue.list` Render Queue Items
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
  `{folders: [path, …] (used in order when the output volume is full)}`
- `renderQueue.formats` Output Formats

## playback

- `playback.toggle` Play Current Preview · Composition > Preview · Space
  `{shortcut?: spacebar|shiftSpacebar|numpad0|shiftNumpad0|altNumpad0 (whose Preview panel options to use; default spacebar)}`
- `playback.cacheWhenIdle` Cache Frames When Idle · Composition > Preview
  `{value?}`
- `playback.audio` Audio · Composition > Preview
  `{value?: include audio in previews}`
- `playback.scrubAudio` Scrub Audio
  `{time? (s, default the current time)} · plays one frame of the comp's audio there (Ctrl/Cmd-drag the current time)`
- `playback.settings.get` Preview Settings
  `{shortcut?: spacebar|shiftSpacebar|numpad0|shiftNumpad0|altNumpad0 (default: the one shown in the Preview panel)} → options + plan {start, end, first, step, fps}`
- `playback.settings.set` Change Preview Settings
  `{shortcut?, current?: shortcut shown in the panel, reset?: bool, values?: {…}, includeVideo?, includeAudio?, includeOverlays?, includeLayerControls?, loop?, cacheBeforePlayback?, range?: workArea|workAreaExtended|entireDuration|aroundCurrentTime, preRoll?, postRoll?: seconds, playFrom?: rangeStart|currentTime, frameRate?: fps|"auto", skip?, resolution?: auto|full|half|third|quarter|custom, customResolution?, fullScreen?, playCachedFrames?, moveTimeToPreviewTime?}`

## view

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
- `view.lookAtSelected` Look at Selected Layers · View · Cmd+Alt+Shift+\
  `{comp?, layers?}`
- `view.lookAtAll` Look at All Layers · View
  `{comp?}`
- `view.reset3DView` Reset 3D View · View
  `{comp?}`
- `view.set3DViewCamera` Set 3D View Camera
  `{view, eye? [x,y,z], poi? [x,y,z], zoom?}`
- `view.get3D` 3D View State
  `{comp?} → current 3D view, view camera, active camera layer, lights`
- `view.addGuide` Add Guide · View
  `{orientation?: vertical|horizontal, position? (comp px)}`
- `view.clearGuides` Clear Guides · View
  `{comp?}`
- `view.importGuides` Import Guides · View
  `{path}`
- `view.exportGuides` Export Guides · View
  `{path}`
- `view.layout` Switch View Layout
  `{views: 1|2|4}`
- `view.shareViewOptions` Share View Options · View > Switch View Layout
  `{value?}`
- `view.extendedViewer` Extended Viewer
  `{value?}`
- `view.setRegionOfInterest` Region of Interest
  `{rect?: [x, y, w, h] | null}`
- `view.snapping` Snapping · View
  `{value?}`
- `view.displayColorManagement` Use Display Color Management · View
  `{value?}`
- `view.simulateOutput` Simulate Output
  `{profile: none|rec709|ntsc|pal|mac18|srgb|rec2020|p3|linear|myCustom|custom, preserveRgb?, space? (custom)}`
- `view.customRgb` My Custom RGB
  `{name?, red?/green?/blue?/white?: [x, y] (CIE xy), gamma?, srgbCurve?: bool, icc?: path (RGB ICC profile, matrix/TRC or LUT-based A2B0: read when it changes, "" clears; a LUT profile simulates through its tables, `lutProfile` in the reply; typed-in numbers replace it), reload?, from?: a Simulate Output profile to start from, reset?, preserveRgb?, apply?: bool (default true: simulate it)} → the definition (kept in Settings)`
- `view.splitLockedViewer` Split with New Locked Viewer · View
  `{comp?, view?: 3D view id}`
- `view.closeLockedViewer` Close Locked Viewer
- `view.displayColor` Viewer Color State
  `{} → display colour management, output simulation, display profile, locked viewer`
- `view.channel` Show Channel
  `{channel: rgb|red|green|blue|alpha|rgbStraight, colorized?, toggle?}`
- `view.exposure` Adjust Exposure
  `{stops? | delta?}`
- `view.resetExposure` Reset Exposure
- `view.takeSnapshot` Take Snapshot · Shift+F5
  `{comp?, scale?: 0.05..1}`
- `view.showSnapshot` Show Snapshot
  `{value?}`
- `view.fastPreviewMode` Fast Previews
  `{mode: off|adaptive|draft|fastDraft|wireframe}`
- `view.moveGuide` Move Guide
  `{comp?, index, position (comp px), merge?}`
- `view.removeGuide` Remove Guide
  `{comp?, index}`
- `view.zoomIn` Zoom In · View · .
- `view.zoomOut` Zoom Out · View · ,
- `view.res.full` Full · View > Resolution · Cmd+J
- `view.res.half` Half · View > Resolution · Cmd+Shift+J
- `view.res.third` Third · View > Resolution
- `view.res.quarter` Quarter · View > Resolution · Cmd+Alt+Shift+J
- `view.res.custom` Custom · View > Resolution
  `{factor?: 1..40 (render every n-th pixel)}`
- `view.rulers` Show Rulers · View · Cmd+R
  `{value?}`
- `view.panelBackground` Panel Background Color
  `{color?: black|darkGray|mediumGray|lightGray|white|custom|#hex, pick?: true}`
- `view.guides` Show Guides · View · Cmd+;
  `{value?}`
- `view.snapToGuides` Snap to Guides · View · Cmd+Shift+;
  `{value?}`
- `view.lockGuides` Lock Guides · View · Cmd+Alt+Shift+;
  `{value?}`
- `view.grid` Show Grid · View · Cmd+'
  `{value?}`
- `view.snapToGrid` Snap to Grid · View · Cmd+Shift+'
  `{value?}`
- `view.options` View Options · View · Cmd+Alt+U
- `view.layerControls` Show Layer Controls · View · Cmd+Shift+H
  `{value?}`
- `view.fullScreen` Enter Full Screen · View · Ctrl+Cmd+F
- `view.newViewer` New Viewer · View · Alt+Shift+N
  `{} · another Composition viewer of the active comp; the one in use is locked`
- `view.assign3dShortcut` Assign Shortcut to 3D View
  `{slot: F10|F11|F12, comp?}`

## window

- `window.panel` Show Panel
  `{panel: project|effectControls|composition|layer|timeline|info|audio|preview|effectsPresets|properties|character|paragraph|align|tracker|wiggler|smoother|motionSketch|paint|brushes|renderQueue|flowchart|history|markers|tools|lumetriScopes|footage|mediaBrowser|metadata|progress|contentAwareFill|createNullsFromPaths|vrCompEditor}`
- `window.workspace` Workspace
  `{name: Default|Standard|Small Screen|Animation|Effects|Motion Tracking|Paint|Text|Minimal|All Panels}`
- `window.saveWorkspace` Save Changes to this Workspace · Window > Workspace
- `window.saveWorkspaceAs` Save as New Workspace · Window > Workspace
  `{name}`
- `window.editWorkspaces` Edit Workspaces · Window > Workspace
  `{name, rename?: new name, delete?: bool}`
- `window.resetWorkspace` Reset to Saved Layout · Window > Workspace
- `window.assignWorkspaceShortcut` Assign Shortcut to Workspace
  `{slot: Shift+F10|Shift+F11|Shift+F12, workspace?}`
- `window.scriptPanel` ScriptUI Panel
  `{name (a script in the ScriptUI Panels folder, e.g. `Layer Tools.jsx`)} → opens it as a dockable panel`

## editor

- `editor.state` Editor State

## scopes

- `scopes.analyze` Lumetri Scopes
  `{comp?, time?|frame?, scope?: waveformRgb|waveformLuma|waveformYc|vectorscopeYuv|vectorscopeHls|histogram|paradeRgb|paradeYuv, standard?: rec601|rec709|rec2020, float?, clamp?, size?}`

## cache

- `cache.diskStats` Disk Cache Statistics

## jobs

- `jobs.list` Background Jobs
- `jobs.cancel` Cancel Job
  `{job?: render|track|maskTrack|warp|camera|roto|task:[n]|all}`
- `jobs.wait` Wait for Background Jobs

## mediaBrowser

- `mediaBrowser.list` List Folder
  `{path?, importableOnly?}`
- `mediaBrowser.go` Go to Folder
  `{path?: folder | "..", importableOnly?}`
- `mediaBrowser.addFavorite` Add to Favorites
  `{path?}`
- `mediaBrowser.removeFavorite` Remove from Favorites
  `{path?}`
- `mediaBrowser.import` Import
  `{paths, addToComp?, time?, index?, position? (where the layers go, as in layer.addItem)}`
- `mediaBrowser.action` Media Browser Action
  `{action}: an action from mediaBrowser.list's `actions` (web: openFolder, addFiles, uploadFolder)`
- `mediaBrowser.fileInfo` File Info
  `{path}`

## templates

- `templates.list` Project Templates
  `{thumbnails?: bool} → [{id, name, description, category, builtin, width, height, frameRate, duration, controls, thumbnail? (hex RGB 256×144)}]`
- `templates.thumbnail` Template Thumbnail
  `{id} → {id, width, height, rgb (hex)}`
- `templates.create` New Project from Template
  `{id (templates.list) | path (.ectemplate), projectPath? (save the new project there; embedded footage goes to `[stem] Footage/` next to it), footageDir?} → an untitled copy (embedded footage extracted: `footage` lists the files)`
- `templates.saveAs` Save as Template · File
  `{name, description?, category?, overwrite?: bool (default true), embedFootage?: bool (default true: the footage files go into the template), embedLimitMB? (default 256: footage beyond it stays linked, with a warning)} → {id, path, embedded, embeddedBytes, linked: [{item, name, reason}], warning?}`
- `templates.delete` Delete Template
  `{id: user/[file]}`

## prefs

- `prefs.get` Get Setting
  `{key?: e.g. general.undoLevels (omit for all settings)}`
- `prefs.set` Change Setting
  `{key, value, values?: {key: value}}`
- `prefs.reset` Reset Settings
  `{page?: general|labels|project|…}`
- `prefs.open` Open Settings
  `{page?: general|startup|project|composition|previews|appearance|grids|labels|type|import|export|audio|disk|memory|video|3d|scripting}`
- `prefs.pages` Settings Pages

## shortcuts

- `shortcuts.list` List Keyboard Shortcuts
  `{query?, keys?, assigned?: bool}`
- `shortcuts.set` Set Keyboard Shortcut
  `{command, params?, keys: "Cmd+Shift+K" | [keys] | null}`
- `shortcuts.reset` Reset Keyboard Shortcuts
  `{command?, params?}`
- `shortcuts.preset` Keyboard Shortcut Preset
  `{op: select|new|duplicate|delete|rename, name?, from?, newName?}`
- `shortcuts.export` Export Keyboard Shortcuts
  `{preset?, path?}`
- `shortcuts.import` Import Keyboard Shortcuts
  `{path? | preset?: exported document, name?}`
- `shortcuts.conflicts` Keyboard Shortcut Conflicts

## storage

- `storage.info` Browser Storage
  `{} → {available, backend, usage, quota, persisted, files: {count, bytes, media, projects, autoSaves}, diskCache: {enabled, entries, bytes, maxBytes, hits, misses, writes, evictions}}`
- `storage.persist` Request Persistent Storage
  `{} → {requested, persisted}`
- `storage.clear` Clear Browser Storage
  `{what: diskCache|media|projects|autoSaves|all} → {cleared, entries, bytes}`

## app

- `app.capabilities` App Capabilities
  `{} → version, command/effect counts, export formats, parity summary`
- `app.about` About formvfx
- `app.settings` Settings
  `{page?: general|startup|project|composition|previews|appearance|grids|labels|type|import|export|audio|disk|memory|video|3d|scripting}`
- `app.gpuInfo` GPU Information
- `app.hide` Hide formvfx
- `app.hideOthers` Hide Others
- `app.showAll` Show All
- `app.quit` Quit formvfx · Cmd+Q
- `app.commandPalette` Quick Apply · Edit · Cmd+Shift+Space
  `{query?}`
- `app.keyboardShortcuts` Keyboard Shortcuts · Edit · Cmd+Alt+'
- `app.templates` Templates
  `{kind: renderSettings|outputModule}`
- `app.find` Find · File · Cmd+F
  `{query?}`

## engine

- `engine.batch` Run Commands (Batch)
  `{steps: [{command, params?}], label? (undo step name, default Batch), atomic?: bool (default true: a failing step rolls the whole batch back)} → {steps, results: [each step's result]}; a string param "$N" or "$N.key.0" is step N's result (1-based), e.g. {"layer": "$1.layer"}`

## command

- `command.list` List Commands
  `{filter?, enabledOnly?, schemas? (add each command's params JSON Schema)}`
- `command.describe` Describe Command
  `{command} → id, label, menu, shortcut, params doc, JSON `schema`, enabled`

## motion

- `motion.sketch` Motion Sketch
  `{layer?, points: [[t, x, y]…] (t = capture seconds) | [[x, y]…] (one per frame), start? (s, default current time), captureSpeed? (%, 100), smoothing? (px, 1)}`

## learn

- `learn.list` Learn Tutorials
  `{} → [{id, title, summary, minutes, steps}]`
- `learn.state` Tutorial State
  `{} → {tutorial, step, steps, done, current: {title, text, target, accepts}} | null`
- `learn.start` Start Tutorial
  `{id}`
- `learn.step` Tutorial Step
  `{action?: next|back|showMe|goto, index?}`
- `learn.stop` Close Tutorial

## help

- `help.docs` formvfx Help · F1
  `{page?: help|scripting|expressions|effects|agents}`
- `help.inAppTutorials` In-App Tutorials · Help
  `{} → the Home screen's Learn tab`
- `help.enableLogging` Enable Logging · Help
  `{on?}`
- `help.revealLogFile` Reveal Logging File · Help
- `help.systemReport` System Compatibility Report · Help
  `{quiet?}`
- `help.systemInfo` System Information
