---
name: formvfx-harness
description: Drive formvfx, the motion graphics and visual effects app (an After Effects alternative), through its formvfx MCP server or the formvfx command line. Use whenever the person wants a composition, animation, title, lower third, logo reveal, kinetic type, transition, shape animation, 3D scene, camera move, light, effect, glow, blur, colour grade, keying, mask, rotoscoping, paint, puppet pins, tracking, stabilizing, face tracking, expression, script, ScriptUI tool, Essential Graphics template, effect plug-in, EXR render pass or a render to MP4, ProRes, GIF, PNG sequence, EXR or Lottie made or changed in formvfx; when formvfx tools such as status, batch, set_keyframes, apply_effect, render_frame or contact_sheet are available; when the person describes an After Effects technique and wants it done here; or when a formvfx tool fails and the next step is unclear. Not for editing formvfx's own source code (use formvfx-skillset for that).
---

# formvfx harness

formvfx is a compositor running on this computer, with the person watching its window. Its MCP server, named `formvfx`, exposes the whole application: every menu item, button, drag and panel action in formvfx is a command with an id, and the tools run those commands against the open project. Each change appears in the window as it happens and is one step in the person's History, so they can see it and undo it.

These tools are the hands. This skill is the knowledge of how a motion designer uses them: what to build first, which command does what, where every value lives, and how to prove the result looks right before saying it does.

## The model

- A **project** holds items: compositions, footage, solids and folders. One composition is active, the one the viewer and Timeline show.
- A **composition** has a size, frame rate, duration, work area and background, and a stack of **layers**. Layer 1 is on top.
- A **layer** is a solid, text, shape, null, adjustment, camera, light, footage, precomposition or 3D model. It has an in and out point, switches (visible, solo, lock, shy, 3D, motion blur, adjustment...), a blend mode, an optional parent and an optional track matte.
- Every value a layer has is a **property** in its tree, addressed by a path: `transform/position`, `effects/#1/blurriness`, `masks/#2/feather`, `text/animators/#1/selectors/#1/offset`, `contents/trim/end`. Properties are static, keyframed or driven by an expression.
- Every action is a **command** (about 500). Commands that change the project are undoable; `batch` makes several one step.

`references/concepts.md` has the details that save the most time: how layers, compositions and properties are addressed, composition time versus layer time, value formats, colours, effect point parameters, and the result each kind of command returns.

## Start every job

1. `status`. The open project, the active composition, the selection, the time, and whether background work is running. It ends with the sensible next step.
2. No composition? Create one with the size, frame rate and duration the destination needs (`execute comp.new`). A composition already open? `get_comp` lists its layers with ids.
3. Look before touching: `render_frame` at the current time.

## The loop

1. **Plan** the composition the way a motion designer would: what is on screen, in which order, how long each beat lasts, how it eases. Write the plan down for anything longer than a few seconds.
2. **Build** with `batch`: one batch per coherent piece (the background, the title block, the transition). A string `"$1.layer"` in a later step means the `layer` field of the first step's result; steps count from 1. A failed step rolls the whole batch back and says which step and why.
3. **Animate** with `set_keyframes`: composition seconds, one ease per key (`linear`, `easyEase`, `easyEaseIn`, `easyEaseOut`, `hold`). Expressions for motion that is a rule rather than a performance.
4. **Look** with `render_frame` at the moments that matter and `contact_sheet` across a span to check motion and timing in one image. Never describe a visual result you did not render and look at.
5. **Fix** what is wrong, then look again.
6. **Deliver**: `render` to a file at an absolute path, and save the project if the person wants to keep editing.

## Routing

Load the sub-skill for the part of the job in front of you. For a whole piece (a title sequence, an explainer, a promo), start with the director.

| The job involves | Load |
| --- | --- |
| A whole piece, a brief, anything broad or unscoped | `sub-skills/harness-director/SUB-SKILL.md` |
| Connecting: the MCP server, the command line, permission prompts, the engine not running | `sub-skills/session-and-connection/SUB-SKILL.md` |
| Compositions, layers, footage, precomps, parenting, track mattes, blend modes, switches, timing, Essential Graphics templates | `sub-skills/compositions-and-layers/SUB-SKILL.md` |
| Keyframes, easing, the Graph Editor, motion paths, sequencing, time remapping, wiggle | `sub-skills/animation-and-keyframes/SUB-SKILL.md` |
| Effects, their parameters, layer styles, animation presets, colour work, keying, the 3D Channel effects on EXR passes, WebAssembly effect plug-ins | `sub-skills/effects-and-presets/SUB-SKILL.md` |
| Text layers, type styling, text animators, text presets, shape layers, Trim Paths, repeaters | `sub-skills/text-and-shapes/SUB-SKILL.md` |
| Masks, feathering, Roto Brush, paint strokes, puppet pins, Content-Aware Fill | `sub-skills/masks-roto-paint/SUB-SKILL.md` |
| 3D layers, cameras, lights, shadows, materials, primitives, 3D models, views | `sub-skills/camera-lights-3d/SUB-SKILL.md` |
| Point tracking, stabilizing, the 3D camera tracker, mask tracking, face tracking | `sub-skills/tracking-and-stabilizing/SUB-SKILL.md` |
| Expressions, the expression language, control rigs, scripts and ScriptUI windows | `sub-skills/expressions-and-scripting/SUB-SKILL.md` |
| Rendering movies, image sequences, frames, GIF, Lottie, the Render Queue, saving projects, project templates, editor timelines (XML, FCPXML, OTIO, EDL), watch folders | `sub-skills/render-and-export/SUB-SKILL.md` |
| Checking work: frames, contact sheets, property reads, expression errors, sanity checks | `sub-skills/verification/SUB-SKILL.md` |
| The formvfx window itself: panels, workspaces, tools, zoom, what the person sees | `sub-skills/ui-automation/SUB-SKILL.md` |
| A known deliverable (lower third, logo reveal, kinetic type, 3D card, matte reveal...) | `sub-skills/recipes/SUB-SKILL.md` |

## Rules that hold everywhere

- **Look before you claim.** The tools report what they did; only a rendered frame shows whether it looks right. Check every visual change with `render_frame` or `contact_sheet`.
- **Never guess an id.** Command ids come from `list_commands` or `references/commands.md`; effect ids and parameter ids from `list_effects` (with `detail`) or `references/effects.md`; property paths from `get_layer` or `references/property-paths.md`. When a call fails, the error says what was wrong and often lists the valid values; read it, fix the parameter, retry.
- **One idea, one undo step.** Group the steps of one change in `batch`, so the person can undo it as a unit.
- **Composition seconds by default.** Tools take composition time. Raw keyframe commands (`prop.addKey`, `keys.set`, `keys.select`) take layer time; `set_keyframes` converts for you, so prefer it.
- **Absolute paths for files.** Imports, saves and renders take absolute paths. Renders default to the workspace's `renders` folder.
- **Background work is asynchronous.** Rendering, tracking, stabilizing, Roto Brush propagation and Content-Aware Fill run in the background. Pass `wait: true` where a command accepts it, or call `wait`, before reading results.
- **Respect the person's project.** Do not save over a project file, close a project with unsaved changes, or delete layers you did not create without saying so first. Prefer building in a new composition when the request is additive.

## References

| File | What it holds |
| --- | --- |
| `references/concepts.md` | Addressing, time, values, results, errors: read once per session |
| `references/tools.md` | The MCP tools with every parameter, the guide topics, and the shell equivalents (generated) |
| `references/commands.md` | Every command id with its menu path, parameters and shortcut (generated) |
| `references/effects.md` | Every effect id with its parameter ids, defaults and controls (generated) |
| `references/property-paths.md` | The property trees of fresh solids, text, shapes, 3D layers, cameras, lights and nulls (generated) |

The generated files come from the engine itself (`./formvfx agents sync`), so they match the formvfx that is running.
