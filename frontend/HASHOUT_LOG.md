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
- The timeline draws its ruler and tracks on two canvases; the layer and property rows are DOM, keyed by `L<id>` / group key / property path and patched in place. Row height is fixed at 24 px because the canvas maps rows by index (`ROW` in `panels/timeline.ts` must equal `--fx-row-h`).

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
