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

## Boot ground

- The critical style paints only `html` (`#0a0a0a`, then the boot script's profile ground); `body` is transparent until `tokens.css` gives it `--bg-primary`. Painting `body` too showed `#0a0a0a` for the first frames before the Studio ground `#111111`.
