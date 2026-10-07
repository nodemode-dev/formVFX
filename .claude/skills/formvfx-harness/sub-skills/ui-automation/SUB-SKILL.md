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

## Courtesy

Tell the person when you change their workspace or tool. Do not close panels they opened or switch workspaces repeatedly; one `ui` call to show the result of a step is plenty.
