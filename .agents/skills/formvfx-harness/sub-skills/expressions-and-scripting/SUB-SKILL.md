---
name: expressions-and-scripting
description: Procedural motion in formvfx. Use to write or fix expressions (wiggle, loopOut, linear and ease mapping, valueAtTime delays, links between layers, expression controls, random and posterized motion), to read expression errors, to run After Effects style JavaScript scripts against the project, and to build Essential Graphics templates with exposed controls.
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
- Rigs: add controls (`apply_effect` with `ec.control.slider`, `ec.control.angle`, `ec.control.checkbox`, `ec.control.color`, `ec.control.point`, `ec.control.layer`) to a null named Controls, rename the effect if several (`prop.renameGroup`), and reference `thisComp.layer("Controls").effect("Speed")("Slider")`.
- `prop.pickWhip {layer, path, target: {layer, path}}` writes the reference expression for you.
- `prop.convertExpressionToKeyframes {layer, path}` bakes an expression into keys.
- `layer.expressions {layers, enabled: false}` switches every expression on the layers off, for speed or for debugging.

**Errors**: `execute expr.errors {}` lists every failing expression in the composition with its layer, property path and message (`Error at line 1: Effect "Amount" not found on layer "Spin"`). Run it after setting expressions; a failing expression leaves the property at its static value.

## Scripts

`run_script {code}` runs JavaScript against the project with the After Effects scripting object model: `app.project`, `app.project.activeItem` (the active composition), `comp.layers.addText(...)`, `addSolid`, `addNull`, `layer.property("Transform").property("Position").setValueAtTime(t, v)`, `setValue`, `numLayers`, `layer(i)` and the rest. It returns `{ok, result, output, error: {message, line, column}}`; `result` is the value of the last expression.

Use a script for loops and data: fifty layers from a list, keys computed from a table, renaming by a pattern. The whole script is one undo step. For a handful of known operations, `batch` with commands is clearer.

Example that adds and animates a text layer:

```javascript
var c = app.project.activeItem;
var t = c.layers.addText("From script");
t.name = "Scripted";
var p = t.property("Transform").property("Position");
p.setValueAtTime(0, [200, 200]);
p.setValueAtTime(1, [600, 200]);
c.numLayers;
```

## Essential Graphics

Expose a composition's properties as a template others can edit without opening it:

1. `essential.setPrimary {comp}` makes it the template composition.
2. `essential.addProperty {comp, layer, path, name}` exposes a property (Source Text, a colour, a slider); `essential.addGroup`, `essential.addComment`, `essential.move`, `essential.rename` organise the panel.
3. Instances: put the template in another composition (`layer.addItem`), then `essential.set {layer, control, value}` sets a control on that instance; `essential.list {comp}` shows the controls.
4. `essential.exportTemplate {comp, path}` writes an `.ectemplate`; `essential.importTemplate {path}` brings one in.

## Check

`expr.errors` must be empty, then `contact_sheet` over the span the expression drives, because an expression that evaluates without error can still move the wrong way.
