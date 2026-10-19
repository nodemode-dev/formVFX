# formvfx manual

formvfx is a motion graphics and visual effects compositor: a project of footage and compositions, layers on a timeline, properties you keyframe, effects stacked on a layer, and a Render Queue. This manual walks through the application by area. Menu paths are written as Menu > Item; every item is also a command you can search with Cmd or Ctrl+K and that agents can run (its id is in brackets where it helps).

## The window

- **Title bar** · the menus (File, Edit, Composition, Layer, Effect, Animation, View, Window, Help), the open project's name, the workspace menu, undo and redo, command search, the Agent button (the Terminal, Claude Code, Chat, Codex, Cursor and the Agent workspace) and Settings.
- **Toolbar** · selection, hand, zoom, orbit, pan behind, the shape tools (rectangle, rounded rectangle, ellipse, polygon, star), the pen, the type tool, brush, clone stamp and eraser, Roto Brush and the puppet tools. A shape tool with a non-shape layer selected draws a mask instead.
- **Panels** · Project, Composition (the viewer), Timeline, Effect Controls, Effects & Presets, Info, Preview, Align, Character, Paragraph, Tracker, Render Queue, Essential Graphics, Flowchart, History, Audio, Footage, Media Browser, Brushes, Paint, Content-Aware Fill, Color Scopes, Markers, Metadata, Motion Sketch, Smoother, Wiggler, Progress, Learn, Chat, Terminal, Agents and Dynamic Link. Window > Panels opens any of them; drag a tab onto another group to dock it there, or use Undock Panel from the tab's menu; the backtick key maximises the focused panel.
- **Workspaces** · Default, Animation, Effects, Text, Motion Tracking, Agent and Minimal (Window > Workspace or the title bar). Save Changes to this Workspace and Save as New Workspace keep your layout.
- **Language** · Settings > Interface > Language: 26 languages, English by default. The whole interface changes in place, including menus and effect names.
- **Shortcuts** · Edit > Keyboard Shortcuts shows the keyboard by modifier, searches, records new shortcuts, loads presets and imports or exports a set; conflicts are flagged.

## Projects and footage

- File > New > New Project, Open Project, Save, Save As, Increment and Save, Revert. Projects are `.fvxproj` files; File > Save As > Save a Copy As XML writes a readable `.fvxprojx` twin. Projects from earlier formvfx releases (`.ecproj`, `.ecprojx`) open as imports. Auto-save runs in the background.
- File > Import > File, Multiple Files, Placeholder, Solid, Lottie, Timeline (XML, FCPXML, OTIO, EDL), Vanishing Point, Motion Graphics Template and Data. Footage covers video and audio (through FFmpeg when it is installed), stills, numbered image sequences, OpenEXR (any layer), PSD documents (merged, one layer, or a composition with its groups as precomps), PDF and .ai pages, SVG and Lottie. `.aep` projects and `.mogrt` templates open too.
- Interpret Footage sets frame rate, alpha, loops and pixel aspect; proxies are set, used and created from File > Set Proxy, Use Proxy and Create Proxy.
- File > Dependencies collects files, consolidates footage, removes unused footage and reduces the project; the Find Missing command (Cmd or Ctrl+K) lists missing footage, effects and fonts.
- The Media Browser shows folders with numbered stills grouped as sequences; the Footage panel trims a clip and edits it into the active composition (Overlay and Ripple Insert).

## Compositions and layers

- Composition > New Composition sets size, frame rate, duration and background. Composition Settings changes them later; Duplicate Composition, Trim Comp to Work Area and the crop commands work on the active composition.
- Layer > New adds Text, Solid, Light, Camera, Null Object, Shape Layer, Adjustment Layer, Content-Aware Fill Layer and 3D Primitive (cube, sphere, plane, torus, cone, cylinder). Drag footage from the Project panel into the Timeline or viewer, or use File > Add Footage to Comp.
- Each layer has switches (video, audio, solo, lock, shy, collapse, quality, effects, frame blending, motion blur, adjustment, 3D), a blend mode (39 modes), a parent and a track matte. Layer > Pre-compose moves layers into a precomp.
- Layer > Time: time stretch, time remapping, freeze frames and time reversal. Layer > Frame Blending includes Pixel Motion.
- Layer > Layer Styles: the nine styles with Blending Options and Global Light.
- Edit > Split Layer, Lift Work Area and Extract Work Area cut layers by time. Layer > Markers and the Markers panel add markers with chapters, URLs, cue points and protected regions.

## The Flowchart

Composition > Composition Flowchart shows the composition as a graph and works as a node editor.

- Turn on **Layers** to see each layer as a node. Drag a layer's link port onto another layer to parent it; drag its matte port onto another layer to use it as the track matte. Click a link to remove or change it.
- Turn on **Effects** to see each effect as its own node between a layer's source and the layer, in stack order. Drag an effect onto another effect of the same layer to reorder the stack. Right-click a layer for Add Effect; right-click an effect for Effect Enabled, Move Earlier, Move Later, Duplicate Effect and Remove Effect.
- An effect with a layer parameter (Displacement Map, Set Matte and others) has a link port: drag it onto a layer to use that layer, and remove it from the link's menu.

## Animation

- Select a layer and press P (position), S (scale), R (rotation), T (opacity) or A (anchor point); click a property's stopwatch to start keyframes and change the value at another time to add one. Keyframes take linear, Bezier, hold and roving interpolation, with speed and influence eases (Animation > Keyframe Interpolation and Keyframe Velocity).
- The Graph Editor (in the Timeline) edits value and speed curves. Separate Dimensions splits position into X, Y and Z.
- Animation > Keyframe Assistant: Convert Audio to Keyframes, Easy Ease, Exponential Scale, Sequence Layers, Smart Mask Interpolation, RPF Camera Import and Time-Reverse Keyframes. The Wiggler, Smoother and Motion Sketch panels do what their names say.
- Animation > Behaviors stacks procedural motion on a property (Wiggle, Oscillate, Spring, Bounce and three loops) without baking keyframes; Link Data to Property drives a property from an imported CSV or JSON column.
- Animation > Rigging > Create Two-Bone IK builds a limb with an FK switch; Animation > Physics > Bake Physics simulates 2D bodies and bakes keyframes. Layer > Cloner > Create Cloner repeats a layer with live per-copy offsets.
- Animation presets: Save Animation Preset and Apply Animation Preset (`.fvxpreset`, and `.ffx`), and Browse Presets.

## Text

- The type tool or Layer > New > Text makes point text; drag with the type tool for paragraph text. The Character and Paragraph panels set font, size, tracking, leading, colour, stroke, OpenType features, variable font axes and alignment, by character range.
- Animation > Animate Text adds animators (position, scale, rotation, opacity, colours, tracking, blur, character offset and value, line spacing, variable axes, and for extruded text Front, Bevel and Side Color); Add Text Selector adds range, wiggly and expression selectors. Text follows a mask path from Path Options, and Enable Per-character 3D gives each character its own 3D transform.
- Effects & Presets > Animation Presets has the 169 text presets.

## Shapes, masks and paths

- Shape layers hold groups of paths, fills and strokes; Add (in the Timeline) inserts every standard shape operator: Merge Paths, Offset Paths, Pucker & Bloat, Repeater, Round Corners, Trim Paths, Twist, Wiggle Paths, Wiggle Transform and Zig Zag. Strokes can be dashed, tapered and wavy.
- Paste SVG to get shape layers; Layer > Create makes shapes or masks from text and shapes from a vector layer; Layer > Auto-trace traces a layer's alpha into masks.
- Layer > Mask: new masks, modes, inversion, locking. Masks take variable feather points and their own motion blur. Window > Create Nulls from Paths links path points and nulls.

## Rotoscoping, paint and puppet

- Open a layer (Layer > Open Layer) and use the Roto Brush tool to stroke the foreground; propagation follows it through the layer, Classic, with MobileSAM, or with SAM 2.1 (which remembers earlier frames). Object Matte selects an object with one click.
- The brush, clone stamp and eraser paint on a layer; strokes keep their durations and blend modes. The Content-Aware Fill panel fills a masked area from the surrounding frames.
- The puppet tools place position, starch, overlap, bend and advanced pins; pins can be recorded.

## Effects

- Effect > [category] or Effects & Presets (search by name) applies one of 306 effects in 21 categories; Effect Controls edits its parameters; the effect's point parameters show handles in the viewer. Effect > Apply Last Effect and Remove All work on the selection.
- Effect > Load Plug-in... loads WebAssembly effect plug-ins and OpenFX plug-ins (`.ofx.bundle`, the effects other OpenFX hosts load); see [`PLUGINS.md`](PLUGINS.md). OpenFX effects that draw on-screen handles through the OpenFX draw suite show them in the viewer, and dragging them changes the effect as one undo step.

## 3D

- Turn on a layer's 3D switch, add a camera and lights (parallel, spot, point, ambient, environment). Layer > Material sets how layers take light: shadows, light transmission, ambient, diffuse, specular, metal, reflections (intensity, sharpness, rolloff), transparency (with rolloff) and index of refraction.
- Composition Settings > 3D Renderer chooses Classic or Advanced. Advanced intersects layers per pixel, draws models, and extrudes: Layer > Geometry Options sets bevel style and depth, extrusion depth and the bevel and side colours of text and shape layers.
- Reflective surfaces show the environment (an Environment light or an environment layer, Layer > Environment Layer) and the other 3D layers that have Appears in Reflections on. Transparent surfaces bend what is behind them by their index of refraction.
- Import glTF, GLB or OBJ models; Layer > Material > Import MaterialX... gives a model, primitive or extruded layer a MaterialX material.
- View > Switch 3D View, Look at Selected Layers and the 1, 2 and 4-view layouts; Layer > Camera has depth of field links, Create Orbit Null and Create Stereo 3D Rig; Composition > VR creates 360 environments and extracts cubemaps.

## Tracking

- Animation > Track Motion, Stabilize Motion, Track Mask, Track this Property, Warp Stabilizer and Track Camera; the Tracker panel shows the analysis and applies tracks to a target. Planar tracking, face tracking (outlines, features and measurements) and the 3D Camera Tracker (which creates cameras, nulls, solids, text and a shadow catcher on its points) run as background jobs listed in the Progress panel.

## Expressions and scripting

- Alt-click a stopwatch (or use the property's menu) to write an expression; the pick whip links properties. Expressions run on formvfx's own interpreter with the standard expression object model, `sampleImage`, regular expressions and the text style API; errors show and fall back to the keyframed value.
- File > Scripts runs `.jsx` scripts against the standard compositor scripting object model and installs ScriptUI panels; Window > Extensions installs CEP extension panels.

## Preview

- Space plays a RAM preview with audio (Composition > Preview > Audio). While you work, the work area renders into the preview cache when idle (Composition > Preview > Cache Frames When Idle); a green strip on the time ruler marks cached frames, and scrubbing shows them at once.
- View > Fast Previews > Adaptive Resolution lowers the viewer's resolution while you drag the time indicator on a heavy composition and restores it when you let go; Draft, Fast Draft and Wireframe are there too. View > Take Snapshot, Show Snapshot, guides, grid, rulers, channels, exposure and the region of interest are available; Split with New Locked Viewer keeps one view fixed.
- Color Scopes shows waveform, parade, vectorscope and histogram.

## Rendering and export

- Composition > Add to Render Queue, then Render in the Render Queue panel. Render Settings set quality, resolution, proxies, effects, motion blur, frame blending, fields and time span; each item takes several output modules (H.264, HEVC, AV1, ProRes 422 to 4444 XQ, WebM, GIF, PNG, JPEG, TIFF and EXR sequences, WAV, AIFF) with templates. Renders run in the background.
- Composition > Save Frame As writes a still, PSD layers or a layered EXR; Pre-render and Save Current Preview render to a file. File > Export writes Lottie JSON, editor timelines (XML, FCPXML, OTIO, EDL) and Slices from Guides. File > Watch Folder renders projects dropped into a folder.
- Composition > Export as Motion Graphics Template and the Essential Graphics panel build templates (`.fvxtemplate` and `.mogrt`) with Essential Properties and Media Replacement.

## Colour

File > Project Settings sets 8, 16 or 32 bits per channel, the working space (sRGB to ACEScg) with Linearize Working Space and 1.0-gamma blending, and the colour engine (formvfx's own or OCIO with ACES 1.3 built in or any `.ocio` config). View > Use Display Color Management and Simulate Output check the result; Interpret Footage assigns footage profiles; output modules tag HDR (PQ, HLG) renders.

## Dynamic Link

In the Project panel, Export Dynamic Link Clip writes a `.fvxlink` for a composition; an editor such as formcut plays it live without rendering. Links survive a moved or renamed project and a renumbered composition. Window > Dynamic Link lists what is being served, relinks and errors.

## Agents

- Window > Start Claude Code (Shift+Cmd or Ctrl+J) runs Claude Code in the Terminal panel already connected to this window; the field under the terminal sends it a request in plain words. Window > Chat with Claude (Alt+Cmd or Ctrl+J) opens the Chat panel, which streams the answer with each tool it uses. Every change an agent makes is an ordinary undo step.
- Codex and Cursor start from the Terminal panel's buttons; the Agents panel sets the clients up for your account. Outside agents ask once in the window before their first change. See the wiki's Agents page and `.claude/skills/formvfx-harness`.

## Learning

Help > In-App Tutorials opens the Learn panel, whose tutorials advance as you do each step; Help > Open Demo Project opens a ready composition. Help > formvfx Help and formvfx Manual open these pages; [`PARITY.md`](PARITY.md) says, row by row, how formvfx compares with the reference compositor.
