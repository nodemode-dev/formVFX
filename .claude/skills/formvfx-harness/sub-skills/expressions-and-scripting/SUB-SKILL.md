---
name: expressions-and-scripting
description: Procedural motion in formvfx. Use to write or fix expressions (wiggle, loopOut, linear and ease mapping, valueAtTime delays, links between layers, expression controls, random and posterized motion), to read expression errors, to run After Effects style JavaScript scripts against the project (loops, data, many layers, computed keys) and to build ScriptUI tool windows.
---

# expressions-and-scripting

## Expressions

Set one with `execute prop.setExpression {layer, path, expression}`; clear it with `expression: ""` or switch it off with `enabled: false`. The language is JavaScript with the After Effects expression object model. `get_property` returns both the stored `value` and the `evaluated` result at a time, which is the quickest test.

Checked against the engine:

| Expression | Effect |
| --- | --- |
| `wiggle(2, 30)` | random drift, 2 times a second, up to 30 units |
| `loopOut("cycle")`, `loopOut("pingpong")` | repeats the keyed animation after the last key |
| `linear(time, 0, 1, 0, 100)`, `ease(time, 0, 1, 0, 100)` | maps one range onto another, straight or eased |
| `value + [0, Math.sin(time * 3) * 20, 0]` | adds a bob to the keyed or static value |
| `thisComp.layer("Box").transform.position.valueAtTime(time - 0.5)` | follows another layer half a second late |
| `posterizeTime(4); seedRandom(index, false); random(-30, 30)` | a new random value 4 times a second, different per layer |
| `thisComp.layer("Box").transform.position.numKeys` | reads keyframe data |
| `effect("Slider Control")("Slider")` | reads an expression control on this layer |

Rules of thumb:

- Return the property's full shape: three values for transform position and scale (`[x, y, z]`), four for colours.
- Name layers and controls before referencing them; references are by name, and a rename breaks them.
- Rigs: add controls (`apply_effect` with `fx.control.slider`, `fx.control.angle`, `fx.control.checkbox`, `fx.control.color`, `fx.control.point`, `fx.control.point3d`, `fx.control.layer`, `fx.control.dropdown`) to a null named Controls, rename the effect if several (`prop.renameGroup`), and reference `thisComp.layer("Controls").effect("Speed")("Slider")`.
- `prop.pickWhip {layer, path, target: {layer, path}}` writes the reference expression for you.
- `prop.convertExpressionToKeyframes {layer, path}` bakes an expression into keys.
- `layer.expressions {layers, enabled: false}` switches every expression on the layers off, for speed or for debugging.

**Errors**: `execute expr.errors {}` lists every failing expression in the composition with its layer, property path and message (`Error at line 1: Effect "Amount" not found on layer "Spin"`). Run it after setting expressions; a failing expression leaves the property at its static value.

## Scripts

`run_script {code}` runs JavaScript against the project with the After Effects scripting object model: `app.project` (`items`, `item(i)`, `activeItem`, `items.addComp(name, w, h, pixelAspect, duration, fps)`, `items.addFolder`, `importFile(new ImportOptions(new File(path)))`, `renderQueue.items.add(comp)`), `CompItem` (`layers`, `layer(i or name)`, `numLayers`, `time`, `workAreaStart`), `comp.layers.addSolid([r, g, b], name, w, h, 1)`, `addText`, `addBoxText`, `addNull`, `addShape`, `addCamera`, `addLight`, `add(item)`, `precompose([indices], name)`, layers (`name`, `parent`, `enabled`, `threeDLayer`, `blendingMode = BlendingMode.SCREEN`, `inPoint`, `duplicate()`, `moveToEnd()`), properties (`property("Transform").property("Position")`, `setValue`, `setValueAtTime`, `setValuesAtTimes`, `keyValue`, `setInterpolationTypeAtKey`, `setTemporalEaseAtKey(i, [new KeyframeEase(0, 75)])`, `expression`), `property("Effects").addProperty("Gaussian Blur")` (After Effects match names such as `ADBE Gaussian Blur 2` work), shape contents through `addProperty("ADBE Vector Group")`, `TextDocument` values on Source Text, `File`, `Folder`, `app.settings`, `app.beginUndoGroup(name)`. It returns `{ok, result, output, error: {message, line}}`; `result` is the value of the last expression and `output` collects `writeLn`, `alert` and `$.writeln`.

Use a script for loops and data: fifty layers from a list, keys computed from a table, renaming by a pattern. The whole script is one undo step. For a handful of known operations, `batch` with commands is clearer.

Checked against the engine:

```javascript
app.beginUndoGroup("Build");
var comp = app.project.items.addComp("Scripted", 1280, 720, 1, 4, 30);
var t = comp.layers.addText("From script");
t.name = "Title";
var p = t.property("Transform").property("Position");
p.setValueAtTime(0, [200, 360]);
p.setValueAtTime(2, [1080, 360]);
p.setTemporalEaseAtKey(2, [new KeyframeEase(0, 75)]);
comp.layers.addSolid([0.1, 0.1, 0.2], "Back", 1280, 720, 1).moveToEnd();
app.endUndoGroup();
comp.numLayers;
```

`app.executeCommand(id, params)` runs any formvfx command from a script. Scripts read and write files the way After Effects scripts do, so they run with the person's consent like any edit.

## ScriptUI

`new Window("palette", "Title")` with `add("button" | "statictext" | "edittext" | "checkbox" | "radiobutton" | "slider" | "progressbar" | "dropdownlist" | "listbox" | "group" | "panel", ...)` and `onClick`, `onChange` or `onChanging` handlers, then `show()`, opens a tool window in the formvfx interface; each handler runs later, when the person uses the control, as its own undo step, and a palette's `show()` returns at once.

`new Window("dialog", ...)` is modal, as in After Effects: `show()` waits until the dialog is answered and returns 1 (OK), 2 (Cancel or the close box) or the number passed to `close(n)`, with the edited values in the controls; `confirm()` and `prompt()` wait the same way. While a script waits, `script.run` answers `{suspended: true, dialog}` and nothing it did yet is kept. To answer it yourself: `scriptui.get` lists the controls, `scriptui.set {control: name|text|id, text|value|selection}` fills them, `scriptui.click {control: "OK"}` presses a button, and the script then finishes as one undo step (its result is in `resumed`). Code before `show()` runs twice (once before the wait, again after), so keep file writes after it.

CEP extensions (a folder with `CSXS/manifest.xml`, or a `.zxp`) install with `extensions.install {path}` and open with `extensions.open {id}` as panels; `extensions.evalScript {script}` runs ExtendScript the way the panel's `CSInterface.evalScript` does and returns the result as a string. Scripts installed with `file.installScript` appear in File > Scripts; panels installed with `file.installScriptUIPanel` appear in the Window menu, where `this` is the Panel.

## Check

`expr.errors` must be empty, then `contact_sheet` over the span the expression drives, because an expression that evaluates without error can still move the wrong way.
