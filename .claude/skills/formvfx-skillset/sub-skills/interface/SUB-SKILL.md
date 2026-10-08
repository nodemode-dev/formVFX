---
name: interface
description: The formvfx interface in frontend/src. Use to add or change a panel, the dock and workspaces, menus, the command palette, keyboard handling, the Composition viewer and its tools, the Timeline, the Graph Editor, property editors and scrubbable values, dialogs and generated command forms, preferences, the Terminal and Agents panels, styles in the formvis design language, or the window.formvfx automation surface.
---

# interface

## Map

| Area | Files |
| --- | --- |
| Engine side, no DOM | `engine/client.ts` (live socket, `call`, `exec`), `engine/store.ts` (ui.state pulls, granular events, `watch`), `engine/frames.ts` (viewer frames), `engine/playback.ts` (RAM preview, audio), `engine/time.ts`, `engine/geom.ts`, `engine/types.ts` |
| App | `app/ctx.ts` (`run`, `invoke` with pickers and forms, gestures, tool state), `app/app.ts` (shell, toolbar, status, home, engine event wiring, consent, automation) |
| UI kit | `ui/dock.ts` (groups, tabs, splits, workspaces), `ui/menubar.ts`, `ui/popup.ts`, `ui/palette.ts`, `ui/keys.ts`, `ui/forms.ts`, `ui/prefs.ts`, `ui/widgets.ts` (Scrub, Swatch), `ui/prop-editor.ts`, `ui/file-dialog.ts`, `ui/icons-vfx.ts`; formvis files `ui/dom.ts`, `ui/themes.ts`, `ui/icons.ts`, `ui/dialogs.ts`, `ui/toast.ts` |
| Panels | `panels/viewer.ts`, `panels/timeline.ts`, `panels/graph.ts`, `panels/project.ts`, `panels/effect-controls.ts`, `panels/effects-presets.ts`, `panels/small.ts` (Info, Preview, Align, Character, Paragraph, History, Render Queue, command panels), `panels/terminal.ts`, `panels/agents.ts` |
| Styles | formvis `styles/tokens.css`, `shell.css`, `panels.css`, `dialogs.css`, `light-mode.css`, `scrollbars.css` (last); formvfx's own `styles/vfx.css` (`fx-*`) |

## Adding a panel

1. Implement `Panel` (`id`, `title`, `el`, optional `shown`, `hidden`, `resized`, `menu`). Build the element once in the constructor.
2. Subscribe with `ctx.store.watch(fn)` (replays the current state) or a granular event; patch keyed children with `patchList` (it calls `update` after `create`), `setText`, `setClass`.
3. Register it in `App` (`dock.register(id, title, factory)`) and, if it belongs in a workspace, in `WORKSPACES` in `ui/dock.ts`.
4. Style it in `styles/vfx.css` with tokens only; give interactive elements `data-automation-id`.
5. Every action is `ctx.run(command, params)` or `ctx.invoke(command)`; a drag takes one key from `ctx.gesture()` on press and sends it as `merge` with every write, so the drag undoes as one step.
6. Async pointer handlers keep the press promise (`this.pressing = this.down(e)`) and run the release after it (`this.pressing.then(() => this.up())`); otherwise a quick click releases before the press has set its drag and the next move edits with a stale drag.

## Rules

- No rebuilds of live containers; no `innerHTML` on a populated list; reserve space for anything that loads (the viewer fills the comp rectangle before the first frame).
- The Timeline's row height (`ROW` in `panels/timeline.ts`) equals `--fx-row-h`; the track canvas maps rows by index.
- `npm run build` (tsc with `noUnusedLocals`, then Vite) must pass; Vite keeps `server.host: false` and port 5230.
