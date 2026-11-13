# Extensions in FormVFX

FormVFX loads extensions that follow the Form Extension Contract, API 1, beside its own effect plug-ins (`docs/PLUGINS.md`), scripts and ScriptUI panels, and HTML panels installed under Window > Extensions. This page is what an extension author needs for FormVFX: the document snapshot, the commands, the events and where things appear. The protocol itself (`host.hello`, `ui.notify`, `storage.*`, `ext.activate` and the rest) is the contract's.

- Product id: `formvfx`. `host.hello` answers `app: "FormVFX"`, `appVersion` (this release, 0.55.4), `api: "1"`.
- Set `"minimum_app": "0.54.0"` in your manifest. A source checkout reports its last release (0.53.3) until the 0.54.0 release commit, so leave `minimum_app` out of a test copy run against an unreleased build.
- Consent: the first time a version is opened or run, FormVFX lists its permissions and the person chooses Allow (stored for that id, version and permission set) or Not Now (not stored; nothing opens and a command fails with -32001). The frame is created only after Allow, so your first `host.hello` already carries the granted permissions.

## document.read

`document.read { what? }` needs the `document` permission. `what` is one of:

| what | Returns |
| --- | --- |
| `summary` (the default) | the project, the active composition, its layers top to bottom, the time and the selection, from the window's own state (no engine round trip) |
| `project` | the engine's `project.summary`: path, unsaved changes, every item, the active composition, the time, the selected layers |
| `comp` | the engine's `comp.info` for the active composition (or `comp`: an id or name): size, rate, duration, work area, background and every layer with its timing and switches |
| `layers` | the full property tree (`layer.tree`) of each selected layer, at most 20: every property with its path, value at the current time, keyframes and expression |
| `selection` | the selected layer ids, properties and mask vertices, and the rows of the selected layers |
| `file` | the open project as it would be saved now, unsaved changes included: `{ name, type: "application/json", size, data }` with `data` base64 (the `.fvxproj` JSON), at most 200 MB |

Any other `what` answers -32602.

A `summary` from a running FormVFX with one composition and two layers (the solid selected):

```json
{
  "product": "formvfx",
  "revision": 6,
  "project": { "path": null, "dirty": true, "undo": "New Text Layer", "redo": null, "items": 2, "compositions": [{ "id": 1, "name": "Title" }] },
  "comp": { "id": 1, "name": "Title", "width": 1920, "height": 1080, "pixelAspect": 1, "frameRate": 30, "duration": 5, "workArea": [0, 5], "background": [0, 0, 0] },
  "time": 0,
  "layers": [
    { "id": 3, "index": 1, "name": "Hello", "type": "Text", "in": 0, "out": 5, "start": 0, "stretch": 100, "parent": null, "selected": false, "is3d": false, "blendMode": "Normal", "effects": [], "masks": 0 },
    { "id": 2, "index": 2, "name": "Backdrop", "type": "Solid", "in": 0, "out": 5, "start": 0, "stretch": 100, "parent": null, "selected": true, "is3d": false, "blendMode": "Normal", "effects": [], "masks": 0 }
  ],
  "selection": { "layers": [2], "props": [] }
}
```

`comp` is `null` and `layers` is empty when no composition is open. Times are composition seconds; layer ids are stable for the life of the project.

## document.apply and app.run

`document.apply { label, ops: [{ command, args }] }` runs the ops through FormVFX's own command surface (the commands its menus, palette, command line and MCP tools use) as one step in History, named `label`. It answers `{ applied, results }`, one result per op. If an op fails, the steps before it are undone and the call fails with -32004 and the engine's message. A string `"$N.key"` in a later op's args is replaced by op N's result field, counting from 1 (`{ "layer": "$1.layer" }` is the layer the first op made).

`app.run { command, args }` runs one command and answers its result. An undoable command is one step in History. `app.commands` lists every command an extension may run: `{ id, title, args, menu }`, where `args` is the parameter summary.

Extensions reach the document, never the disk, the network or the app itself: these are refused with -32004 and the reason. Every command in the `file.`, `script.`, `scriptui.`, `extensions.`, `effect.plugins.`, `app.`, `prefs.`, `shortcuts.`, `help.`, `agents.`, `renderQueue.`, `render.`, `templates.`, `mediaBrowser.`, `storage.`, `material.`, `roto.model.`, `face.model.`, `track.faceModel.`, `export.`, `data.`, `link.`, `engine.` and `command.` namespaces, and the commands that read or write a file by path (Save Frame As, template and animation preset import and export, guides import and export, 3D model import, Align Video to Data, camera data import, cubemap extraction, Edit Original, colour configuration lookups, emptying the disk cache). For files, ask with `files.open` and `files.save`; `app.commands` lists only what is allowed.

Useful commands (all layers are an id, `#n` with 1 the top layer, or a name; omit a layer to use the selection):

| Command | Args |
| --- | --- |
| `comp.new` | `{ name?, width?, height?, frameRate?, duration? (s), background? "#hex" or [r, g, b] }` |
| `comp.open` | `{ comp: id or name }` |
| `layer.newSolid` | `{ name?, color? "#hex", width?, height? }` |
| `layer.newText` | `{ text?, name?, position? [x, y], font?, size?, fill?, justify? }` |
| `layer.setText` | `{ layer?, text?, font?, size?, fill?, stroke?, strokeWidth?, tracking?, leading?, justify? }` |
| `layer.newShape` | `{ kind?: rect, rounded, ellipse, star, polygon, name?, size? [w, h], fill?, stroke?, strokeWidth?, position? [x, y] }` |
| `layer.newNull` | `{ name?, position? [x, y] }` |
| `layer.select` | `{ layers: [layer], add?, toggle? }` |
| `layer.rename` | `{ layer?, name }` |
| `layer.timing` | `{ layers?, start?, in?, out?, stretch?, delta? }` |
| `layer.setParent` | `{ layers?, parent: layer or null }` |
| `layer.setBlendMode` | `{ layers?, mode: Normal, Add, Multiply, Screen, Overlay... }` |
| `layer.addMask` | `{ layer?, shape?: rect or ellipse, rect? [x, y, w, h] (layer pixels), mode? }` |
| `layer.addMarker` | `{ layer?, time?, comment? }` |
| `layer.setTransform` | `{ layers?, prop: anchor, position, scale, orientation, rotation or opacity, value }` |
| `prop.set` | `{ layer?, path, value, time? (layer s) }` (paths such as `transform/position`, `effects/#1/blurriness`; `layers` in `document.read` lists them) |
| `prop.addKey` | `{ layer?, path, time? (layer s), value? }` |
| `prop.setExpression` | `{ layer?, prop, expression?, enabled? }` |
| `effect.apply` | `{ effect: id or name (Gaussian Blur), layers? }` |
| `keys.easyEase` | `{ which?: both, in or out }` (on the selected keyframes) |
| `edit.duplicate`, `edit.clear` | `{ layers? }` |
| `time.set` | `{ time? (s) or frame? }` |
| `comp.info`, `layer.tree`, `markers.list`, `project.summary` | queries, also through `app.run` |

`app.commands` has the rest (about 900), with their parameter summaries.

## Events

`ext.event` sends:

| name | data |
| --- | --- |
| `document.changed` | `{ revision, undo, dirty }` after any change to the project (an edit, an undo, a redo, an agent's change) |
| `selection.changed` | `{ layers, props }` when the selected layers or properties change |
| `time.changed` | `{ time }` when the current time moves (not during playback) |
| `comp.changed` | `{ comp }` when another composition becomes active |

## Files and network

- `files.open` (permission `files-read`) shows FormVFX's own open dialog (the system dialog in the desktop app, FormVFX's file browser in a browser window), filtered by `accept`, and answers the chosen files as base64, at most 200 MB in total.
- `files.save` (permission `files-write`) shows FormVFX's save dialog with the name you give and writes the data where the person chooses.
- Choosing Cancel in either dialog fails the call with -32002.
- `net.fetch` is not available in FormVFX (-32003).
- `storage.get` and `storage.set` keep up to 256 KB per extension in the window.

## Where it appears

- **Extensions menu**, top level in the menu bar between Window and Help (and in the macOS app menu bar): one item per contributed command, `Name: Title`, then Extensions Panel, Reload Extensions and Manage in FormPortal.
- **Extensions panel** (Extensions > Extensions Panel, or Show Extensions in the command palette): one row per extension with its version, source (Catalogue, File, Development), permission chips and state (Ready, Needs Permission, Refused with the reason), Open for its panels and the folder of a development extension. Below the rows it lists what FormVFX loads on its own (HTML panels from Window > Extensions and effect plug-ins), so everything that extends FormVFX is in one place.
- **Contributed panels**: a `dock` panel opens as a tab in the right-hand panel group and can be moved, undocked and closed like any FormVFX panel; its tab says `Name: Panel title` (or just the name when they are the same). A `dialog` panel opens as a FormVFX dialog over the window.
- **Command palette** (Cmd+K): every contributed command as `Name: Title` and every contributed panel as `Show Name: Panel title`, under the path Extensions.
- **Agents**: the MCP tools `extensions_list` and `extension_command`, and `./formvfx extensions` and `./formvfx extension ID COMMAND [JSON]` on the command line. They run in the open window, with the same consent.

The frame is sandboxed (`allow-scripts` only); its files come from `/api/extensions/files/{id}/{version}/{path}` on the engine, with the contract's CSP.

## Run your extension in FormVFX

1. In form.ext, choose Run in App with FormVFX. It writes `extensions/formvfx/dev.json` under FormPortal's folder (`~/.local/share/formportal` on Linux, `~/Library/Application Support/studio.formai.portal` on macOS, `%LOCALAPPDATA%\FormPortal` on Windows, or `FORMPORTAL_ROOT`) with the path of your extension's folder.
2. FormVFX checks that file every 2 seconds while its window has focus, and on Extensions > Reload Extensions. The extension appears as a Development row; edit its files and choose Reload Extensions (or reopen the panel) to see the change.
3. Without form.ext, write the file yourself:

```json
{ "format": "formext-dev-extensions", "format_version": 1, "product": "formvfx", "extensions": [{ "id": "my-extension", "path": "/absolute/path/to/my-extension", "added": "2026-11-07T12:00:00Z" }] }
```

To try it against a source checkout: `FORMPORTAL_ROOT=/tmp/portal ./formvfx` (engine on 8250, interface on http://localhost:5250).

## Installed through FormPortal

FormPortal installs a `.formpkg` from the catalogue (or a file you choose), unpacks it and lists it in `extensions/formvfx/enabled.json`. FormVFX picks the change up within 2 seconds and shows the row as Catalogue or File. Manage in FormPortal (in the Extensions menu and panel) opens `formai://extensions?product=formvfx`.
