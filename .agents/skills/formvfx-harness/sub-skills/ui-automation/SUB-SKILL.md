---
name: ui-automation
description: Working with the formvfx window the person sees. Use to report what the window shows, open or focus a panel, switch workspace, choose a tool, zoom the viewer, move the current time or selection so the person can follow along, or drive the interface itself from a browser automation through window.formvfx and data-automation-id hooks.
---

# ui-automation

The project is the same whether or not a window is open; the window is where the person watches. Use these to show them what you did, not to edit: edits go through commands.

## From MCP

| Need | Call |
| --- | --- |
| What the person sees (workspace, visible panels, focused panel, tool, viewer zoom, Timeline state) | `ui {action: state}` |
| Show a panel | `ui {action: panel, panel}` with `composition`, `timeline`, `project`, `effectControls`, `effectsPresets`, `character`, `paragraph`, `align`, `preview`, `info`, `history`, `renderQueue`, `terminal`, `agents`, `tracker`, `markers`, `paint`, `essentialGraphics` and the other panel ids |
| Switch workspace | `ui {action: workspace, name: Default|Animation|Effects|Text|Motion Tracking|Agent|Minimal}` |
| Choose a tool | `ui {action: tool, tool: selection|hand|zoom|rotation|panBehind|rect|ellipse|pen|type|brush|clone|eraser|roto|puppet|orbit}` |
| Zoom the viewer | `ui {action: zoom, value: fit}` or a percentage such as `100` |
| Move the time indicator | `execute time.set {time}` (the window follows) |
| Select layers so they highlight | `execute layer.select {layers}` |
| Open a composition in the viewer | `execute comp.open {comp}` |

With no window open, `ui {action: state}` says so; nothing else is affected.

## From a browser automation

The interface exposes `window.formvfx`:

- `window.formvfx.state()` returns the engine state the window holds; `window.formvfx.execute(id, params)` and `window.formvfx.invoke(id)` run commands (invoke opens the command's form when it needs input).
- `window.formvfx.ui.state()`, `ui.elements()` (every interactive element with its `data-automation-id`), `ui.click(id)`, `ui.showPanel(id)`, `ui.workspace(name)`, `ui.tool(id)`.

Every control carries a `data-automation-id` such as `toolbar`, `panel.timeline`, `panel.tab.effectControls`, `timeline.rows`, `timeline.graphToggle`, `viewer.zoom`, `palette.input`, `workspace.select`, `menubar`.

## Extensions

Form extensions (installed through FormPortal or run from form.ext) add panels and commands to the window.

- `extensions_list` shows each one: id, name, version, source (`catalogue`, `sideload`, `development`), state (`ready`, `needs-permission`, `refused` with a `reason`), permissions, panels and commands. With no window open it lists what the window would load (`windows: 0`).
- `extension_command {id, command, args}` runs a contributed command in the window and returns what the extension reports. The first run of an extension version shows the person a permission dialog; Not Now fails the call, so tell them why you are asking before you run it. Its edits are ordinary undo steps.
- `ui {action: panel, panel: extensions}` shows the Extensions panel. The command line has `./formvfx extensions` and `./formvfx extension ID COMMAND [JSON]`.
- A refused extension cannot run; its reason (made for another app, a newer formvfx needed) is what to tell the person.

## Courtesy

Tell the person when you change their workspace or tool. Do not close panels they opened or switch workspaces repeatedly; one `ui` call to show the result of a step is plenty.
