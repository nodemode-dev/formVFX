# frontend · HASHOUT_LOG

Living context for the interface. Code carries almost no comments; the reasoning lives here.

## Shape of the interface

- Vanilla TypeScript, no framework. `src/app/app.ts` mounts the shell once (title bar, toolbar, dock, status bar, home) and everything after that is patched in place.
- The engine is the source of truth. `engine/store.ts` pulls `ui.state` from the engine (coalesced, one request in flight) and emits granular events (`items`, `comp`, `layers`, `selection`, `time`, `trees`, `history`, `viewer`, `project`, `state`) by comparing JSON signatures, so a panel only redraws what changed.
- `store.watch(fn)` subscribes to `state` and replays the current state on the next microtask. Panels are built lazily when a workspace first shows them, so a panel created after the first pull still renders immediately.
- Every user action is an engine command: `ctx.run(id, params)` for direct calls, `ctx.invoke(id)` when the command may need a form (generated from the command's parameter docs in `ui/forms.ts`) or a file picker (`PICKERS` in `app/ctx.ts`).
- Drags are gestures: `gesture.begin` before the first write, `gesture.end` after the last, so a whole drag is one undo step (the engine merges by key until the gesture ends).

## Rendering

- The Composition viewer is one canvas. Frames come over `/api/viewer` as binary messages (header `<IIIIdI` then RGBA) and are drawn under the overlays (layer bounds, anchor, paths, motion paths, guides). Latest wins: a newer request cancels an older one.
- Before the first frame arrives, the comp rectangle is filled with the comp background so the frame lands in a reserved box.
- RAM preview (`engine/playback.ts`) caches ImageBitmaps for the work area; the cache purges on any `changed` event with `edited: true`. Audio plays from `/api/audio` WAV through an AudioContext.
- The timeline draws its ruler and tracks on two canvases; the layer and property rows are DOM, keyed by `L<id>` / group key / property path and patched in place. Row height is 24 px (`ROW` in `panels/timeline.ts` must equal `--fx-row-h`); since 0.4.0 waveform rows are three rows tall, and the canvas maps y through `tops` (prefix sums of `rowH`) instead of dividing by `ROW`.

## Design language

- formvis Studio Dark, copied verbatim: `styles/tokens.css`, `shell.css`, `panels.css`, `dialogs.css`, `light-mode.css`, `scrollbars.css`, plus `ui/themes.ts`, `ui/dom.ts`, `ui/icons.ts`, `ui/dialogs.ts`, `ui/toast.ts`.
- `styles/vfx.css` is formvfx's own sheet: `fx-*` classes only, every colour a token, every z-index a `var(--z-*)`. App-owned media colours are declared once at the top: `--fx-pasteboard`, `--fx-track-bg`, `--fx-ruler-bg`, `--fx-workarea`, `--fx-cti` (current time indicator, blue as motion tools use), `--fx-hot` (scrubbable values), `--fx-guide`, `--fx-expr` (values driven by an expression).
- formvis puts the options row inside its main area (`.cv-app` has three rows); formvfx keeps it as its own row, so `.fx-app` restores the four-row grid.
- `scrollbars.css` stays the last stylesheet in `index.html`.

## Divergences from the formvis files

- `ui/dom.ts` `patchList` now calls `update` right after `create`, so `create` builds structure and `update` fills it. formvis callers that fully render in `create` are unaffected because `update` is idempotent.

## Agent surface

- `window.formvfx` exposes `execute`, `invoke`, `state`, and `ui.{state, elements, click, showPanel, workspace, tool}`. Every interactive element carries `data-automation-id`, which `ui.elements()` lists, so an agent can drive the visible interface as well as the engine.
- The engine can ask the interface for its state (`uiRequest`) and to perform UI actions (`uiAction`); the reply goes back over the live socket.
- Consent: an agent that is not started from the Terminal panel triggers a dialog (Allow, Allow Always, Don't Allow). Terminal agents carry the per-boot token and never ask.

## Known gaps

- The timeline's left column width is fixed (`--fx-tl-left`); no splitter yet.
- Tracker, Wiggler, Smoother, Motion Sketch, Paint, Brushes, Content-Aware Fill, Markers, Audio, Progress, Metadata, Media Browser, Footage and Layer are generic command panels (`COMMAND_PANELS` in `app/app.ts`): every command they need is there as a button with a generated form, but none has a dedicated view yet.
- Paint strokes preview as a polyline while drawing; brush tip size and hardness are not drawn.

## Direct manipulation (0.3.0)

- Pointer race: `down()` is async (it awaits hit tests and the gesture key), so a fast click could fire `pointerup` first and leave a drag armed for the next move. Viewer, Graph Editor and Timeline keep `pressing` (the press promise) and run `up()` after it.
- Gesture keys are minted on the client (`ctx.gesture()`: a per-load tag plus a counter). The engine's `begin_gesture` only minted a key too, so the round trip is gone; every write in a drag carries the same `merge` key.
- Graph Editor handles: drawn for selected Bezier-family keys. Value mode: one handle per dimension, x = key time plus or minus span times influence, y from the speed; speed mode: the handle sits on the speed curve. Dragging sends `keys.setEase` with influence in percent (clamped 0.1 to 100) and speed in units per second.
- Motion path tangents: dragging a tangent handle sends `keys.setSpatialTangents`; the opposite side mirrors unless Alt breaks it. Alt on a key without tangents pulls handles out.
- Viewer extras (`loadExtras` in `panels/viewer.ts`): for the first selected layer, `track.status` and `puppet.info` give track points and pins in layer space; `geom.invert` of `toLocal` draws them in comp space. Skipped during playback. Track point boxes edit locally while dragging and commit one `track.setPoint` on release; pins send `puppet.movePin` on every move.
- `track.status` and `puppet.info` are query commands, so the engine does not log them in Activity when the interface asks (`execute` quiet for ui-sourced queries).

## Audio, Flowchart, shortcuts, floating panels (0.4.0)

- Waveforms: `engine/peaks.ts` (`PeakCache`) fetches `/api/peaks/{item}` once per footage item and once per revision for a nested comp (the comp's mix changes with edits), keeps drawing the last summary while a newer one loads, and asks the Timeline to redraw when one lands. A wave row appears under an open Audio group or with LL (L twice within 350 ms); it draws left over right, columns aggregated min and max through the layer's start and stretch (time remapping is not followed).
- Flowchart (`panels/flowchart.ts`): `ui.flowchart` graph, nodes as absolutely placed divs (patched by id, moved by transform), edges in one SVG keyed by from and to. Back edges (to a shallower node: a shared item or a cycle) run from the source's near side to the target's far side so they do not loop. Loads only while visible.
- Keyboard Shortcuts (`ui/shortcut-editor.ts`): one dialog built once; the list is patched by binding key, the keyboard repainted by class. Changing the built-in preset makes the engine create "Custom" and toast it. After every change the editor calls `Keys.load()` so the new keys work at once.
- `comboOf` now matches the engine's names: Ctrl before Cmd, `Escape` (was `Esc`), numpad keys as `Num0`, `Num*`, `Num+`, `Num-`, `Num.`, `Num/` (they were `Numpad0` and never matched).
- Floating panels: a fifth dock area, `float`, in `.fx-float-layer` (one `--z-companion` stacking context with `isolation`). Floating groups order themselves with small inline z-index values inside that context (`Dock.raise`); geometry is saved in the workspace layout (`float: [{panels, active, x, y, w, h}]`). Undock Panel and Dock Panel are in the group menu; the header drags, the corner grip resizes.

## Views, gizmo, assistants, rulers, snapshots (0.5.0)

- View layouts and the locked viewer (`panels/viewer-aux.ts`): the viewer's stage sits in `.fx-viewer-main`, a CSS grid whose `data-layout` (1, 2, 4) places up to three extra panes beside it; `.fx-viewer-views` adds the locked pane as a left column. Panes are built once and only hidden, each with its own `FrameStream` (its own socket, connected the first time it shows) asking for draft frames with `view3d`; a pane's 3D view is local (select in its label), double-click makes it the main view. A layout change refits the main view on the next frame (the stage is the same element, so its tools and overlays keep working). Panes do not update during playback.
- 3D axis gizmo: `ui.overlays` gives, per selected 3D layer, `axes.parent` (the parent-space displacement of each 60-unit local axis) and `axes.position`. Dragging an arrow projects the pointer movement on the arrow's screen direction and sets position to `position + t * parent[i]`, one `prop.set` per move with one gesture key. The anchor area (9 px) is left to the move drag.
- Motion Sketch: the panel arms the viewer through `ui.emit('viewer', {command: 'motionSketch.arm'})`; the next press records `[seconds since press, comp x, comp y]` until release, draws the trail, then runs `motion.sketch` and emits `motionSketch.done`. Wiggler and Smoother panels enable Apply from `selection.keys` (two keys of a property, three for the Smoother).
- Rulers (`view.rulers`, Cmd+R, bar button): drawn on the viewer canvas in an 18 px band (`RULER`), steps of 1, 2 or 5 times a power of ten at 50 px or more; a press in a ruler starts a new guide (left ruler vertical, top ruler horizontal), added with `view.addGuide` when released over the stage.
- Snapshots: `ui.state.snapshot` carries an id per taken snapshot; the viewer loads `/api/snapshot.png` once per id and draws it in place of the frame while `viewer.show_snapshot` is on.
- `window.formvfx.ui.state().viewer` now also reports `origin` (stage pixels of comp 0,0), `views` (layout, extra views, locked viewer) and `motionSketch`.

## Text presets and SVG paste (0.6.0)

- Effects & Presets loads `text.library` once on first show; the list is one keyed row set (`c:*presets` the root, `c:pc:<category>` at depth 1, `p:<id>` at depth 2, then the effect categories), patched in place, so twirling a category only inserts or removes its rows. Open state keys are `*presets` and `pc:<category>`. A preset applies by double-click to the selected text layers or by dragging (`application/x-formvfx-text-preset`) onto a text layer's row in the Timeline.
- Paste: `App.paste` handles the document `paste` event outside fields, the terminal and dialogs. Clipboard text that starts as an SVG document goes to `clipboard.pasteSvg`; anything else runs `edit.paste`. `Keys` does not run `edit.paste` on Cmd+V itself: it waits 120 ms for the paste event (`ui.emit('pasted')` cancels the wait), so the layer paste still runs in webviews that send no paste event for non-editable content, and never runs twice.

## Planar Track section (0.7.0)

- `panels/tracker.ts`: `TrackerPanel` extends the generic `CommandPanel` for `tracker` and prepends one Planar Track section, built once. The mask and target selects are refilled only when their option lists change (a JSON signature); the target defaults to the layer above the source each time the source changes. Mask names come from the source layer's property tree (`store.wantTrees('planar', [id])`), else Mask 1..n. The run calls `track.planar` and keeps its message in the status line until the source changes.

## Settings pages (0.8.0)

- `Preferences.show` builds a page off the DOM and swaps it in with one `replaceChildren` once its values (and, for Memory & CPU, `engine.performance`) have arrived; the old page stays until then, and a page left before its values arrive is dropped. It used to empty the pane first and append the Multi-Frame Rendering rows after a second request, so the page blinked empty and then grew.
- Memory & CPU > GPU Acceleration: the checkbox sends `engine.performance {gpu}` and the note shows the adapter or the reason the CPU is used.

## Object Matte tool (0.9.0)

- `objectMatte` is the third tool of the Alt+W group. Hover (no drag) over the selected layer, or the layer under the pointer, when it has video: after 90 ms still, one `roto.objectPreview` is in flight at a time and the latest point waits behind it. The run lengths are decoded into a canvas the size of the returned mask (accent at alpha 110 inside, white at 235 on its edge), drawn in `draw()` through an affine transform from three mapped points (mask pixel to layer pixel to comp to screen), only while the tool is active and the time is the one it was computed for. A click calls `roto.objectMatte` and drops the preview.

## Tracker panel and Dynamic Link (0.10.0)

- `panels/tracker-controls.ts`: `MotionTracker` is the After Effects Tracker panel's control set (Track Camera, Warp Stabilize, Track Motion, Stabilize Motion; Motion Source, Current Track, Track Type, Position / Rotation / Scale, Motion Target, Analyze, Options, Reset, Apply), prepended above the Planar Track section and built once. `refresh()` is debounced on `store.watch` and polls `track.status` every 400 ms only while a track runs; selects are refilled only when their option signature changes. `track.status` carries `layer`, `tracker` and `trackers` only when the source has a tracker, and `mask` is mask-tracking progress, not the selection.
- Mask mode: when the selection is a mask on the source (`selection.vertices`, or a selected prop whose uid is one of the source's mask groups in `store.tree`), the Track Type and Target rows hide, Mask Method shows, and Analyze runs `track.mask`. `.fx-assist [hidden]` is needed because the rows are grids.
- `trackerOptions` and `editTarget` are the two dialogs; the engine's `track.optionsDialog` and `track.editTargetDialog` frontend events open them too (they were unhandled before).
- Project panel: a composition's context menu has Export Dynamic Link Clip (`link.export`), with the result path or the reason in a toast.

## Project files (0.11.0)

- `Ctx.projectExt` and `Ctx.engineName` come from `engine.info` (`capabilities.projectExtension`, `capabilities.engine`) when the socket opens, defaulting to `fvxproj` and `fvx`. The save pickers (Save, Save As, Save a Copy) offer `.fvxproj` with `Untitled Project.fvxproj`; `invoke` passes `PICKERS[command]` through unchanged.
- Open Project and Import accept `.fvxproj`, `.ecproj` and `.ecprojx`; the last two are projects from earlier formvfx releases, opened as imports.

## Text animators in the Timeline (0.13.0)

- `panels/timeline.ts` group context menu: an `animator` group offers Add Property (the animator property kinds, `layer.addTextAnimatorProperty` with the animator uid) and Add Selector (range, wiggly, expression, `layer.addTextSelector`). Text and Animators keep Add Text Animator. The menu is the shared popup; rows appear through the normal keyed patch on the next state pull.
- The engine's tree marks list items (animators, selectors, masks, effects, shape items) with `kind: "indexed"`, which is what offers Rename, Duplicate and Delete; the list groups themselves no longer carry it.

## Animation preset files (0.15.0)

- `Ctx.presetExt` comes from `capabilities.presetExtension` the same way as `projectExt`, defaulting to `fvxpreset`. Save Animation Preset offers `.fvxpreset` with `Preset.fvxpreset`; Apply Animation Preset takes `.fvxpreset` or `.json`. No DOM change: the picker is the shared file dialog.

## Import options and the Render Queue row (0.21.0)

- `invoke` for `file.import` and `file.importMultiple`: when the picked files include `.psd`, `.psb`, `.pdf` or `.ai`, the command's own form opens with the paths filled in and Import As set to Footage, so Composition, Retain Layer Sizes, a layer or a page can be chosen; other files import straight away as before. The form is the shared `commandForm`, built from the engine's parameter summary (which names `layer` without an option list on purpose, so the field is free text).
- The Render Queue row gains two links between the name and the colour label: the Render Settings template and the Output Module template (with `(+N)` when an item has more output modules). Each opens the command's form through the engine's "needs a setting" answer, the way the colour label does. The row's grid now has a column per child (it had five columns for six in-flow children, so the remove button wrapped). Both labels are created once in `create` and only their text changes in `update`.
- The Project panel's info lines name the proxy file, with `(off)` when Use Proxy is off.

## ScriptUI windows (0.22.0)

- `ui/scriptui.ts` draws the windows a script builds. The engine sends the whole tree with `scriptui.show`; the first show mounts a floating section in the overlay root (so it stacks on `--z-companion`, above the dock, below menus) and every later show patches it: controls are keyed by id, a control whose type is unchanged keeps its node and only its text, value, checked state or enabled state is written, and a dropdown or list rebuilds its options only when the item list changed. A button that renames itself in its own handler therefore keeps focus and position.
- Events go back as `scriptui.event` with the control id and the new value; the handler runs on the engine and its changes come back as one more `scriptui.show`. Windows outlive a reload: on each socket open `restore` asks `scriptui.list` and mounts what the engine still holds.
- The window is `position: fixed` and the head drags it by `left` and `top`, so dragging never moves anything around it; new windows cascade by 24 pixels.

## Essential Graphics panel and plug-ins (0.23.0)

- `panels/essential.ts` has an Edit tab (primary composition, template name, the controllers) and a Browse tab (installed templates as poster cards that import into the active composition). Controller rows are keyed by controller id and patched in place: a rename writes only the input's value, so typing in one row never rebuilds another; a move reorders the existing nodes. The list is fetched with `essential.list` only when the revision changes and the panel is showing, so playback (which updates the time, not the revision) costs nothing.
- Rows are a small grid: the layer name on top, the name field with move and remove buttons, and the value under them, so the panel stays usable at the narrow width of the right column.
- `Dock.home` opens Essential Graphics beside Effects & Presets (the tallest right-hand group in the built-in workspaces) instead of the first right-hand group, which is Info's short strip.
- Loading a plug-in emits the frontend command `effects.refresh`: the context drops its effect catalogue and Effects & Presets re-reads it if it was ever shown; the engine service drops its cached catalogue whenever an `effect.plugins.*` command runs.

## Boot ground

- The critical style paints only `html` (`#0a0a0a`, then the boot script's profile ground); `body` is transparent until `tokens.css` gives it `--bg-primary`. Painting `body` too showed `#0a0a0a` for the first frames before the Studio ground `#111111`.
