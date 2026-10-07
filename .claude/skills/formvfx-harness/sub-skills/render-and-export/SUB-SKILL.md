---
name: render-and-export
description: Getting work out of formvfx. Use to render a composition to H.264, HEVC, AV1, ProRes, WebM or GIF, to PNG, JPEG, TIFF or EXR sequences, to audio, to export single frames, layered PSD or multi-layer EXR stills, or Lottie JSON for the web; to manage the Render Queue, render settings, output modules and templates; and to save, open or version projects.
---

# render-and-export

## The quick path

`render {comp, format, output, wait: true}` adds the composition to the Render Queue, renders it, and returns when the file is written. Formats: `h264` (default), `hevc`, `av1`, `prores`, `webm`, `gif`, `png`, `jpeg`, `tiff`, `exr`, `wav`, `aiff`. `output` is an absolute path; for image sequences put a frame-number template in it, `.../shot_[#####].png` writes `shot_00000.png` onwards. Without `output` the file goes to the agent workspace's `renders` folder.

The work area decides the frames: `comp.workArea {start, end}` first if only part should render.

| Destination | Format and notes |
| --- | --- |
| Web, social, review | `h264` |
| Smaller files, modern players | `hevc` or `av1` |
| Editing and finishing, with alpha | `prores`; `renderQueue.setOutputModule {proresProfile: proxy|lt|standard|hq|4444|4444xq, channels: rgba}` (4444 keeps alpha) |
| Browser with alpha | `webm` |
| Animated preview, chat | `gif` (keep it short and small) |
| Compositing elsewhere, alpha, stills | `png` sequence; `exr` for linear high bit depth |
| Audio only | `wav` or `aiff` |

`execute renderQueue.formats {}` lists every format with its options and whether it is available on this machine.

## The Render Queue in detail

| Need | Command |
| --- | --- |
| Queue a composition | `renderQueue.add {comp, format, output, template, quality}` |
| Render settings (resolution, quality, effects, time span) | `renderQueue.setRenderSettings {item, resolution: full|half|third|quarter, quality: best|draft, ...}` |
| Output module (codec options, channels, bitrate, audio) | `renderQueue.setOutputModule {item, format, channels: rgb|rgba|alpha, quality, bitrate, proresProfile, audio}` |
| A second output from the same render | `render.addOutputModule {item, format, output}` |
| Output path | `renderQueue.setOutput {item, path}` |
| Render everything queued | `renderQueue.render {wait: true}` |
| Inspect, reorder, remove | `renderQueue.list`, `renderQueue.move`, `renderQueue.remove`, `renderQueue.setRender {item, render: false}` |
| Templates | `renderQueue.templates`, `renderQueue.saveTemplate`, `renderQueue.applyTemplate` |

Alpha needs both a format that carries it (PNG, ProRes 4444, WebM, EXR, TIFF) and `channels: rgba`; the composition background colour is not rendered when alpha is on.

## Stills and other exports

- One frame: `comp.saveFrameAs {path, time, scale}` (PNG).
- Layered stills: `comp.saveFrameAsPsd {path, time}`, `comp.saveFrameAsExr {path, time}` (one EXR layer per composition layer).
- Lottie: `file.exportLottie {comp, path (.json or .lottie), includeExpressions, textAsShapes}`. It reports warnings for anything Lottie cannot carry (most effects, expressions unless included); read them and tell the person.
- A frame for yourself to look at: `render_frame` (returns the image, writes nothing).

## Projects

- `project {action: new|open|save|saveAs|close|demo, path}`. Projects are `.ecproj` JSON. `save` needs a `path` the first time.
- `file.incrementAndSave` saves a numbered copy next to the current file, good before risky changes.
- `file.collectFiles {folder}` gathers the project and its footage into one folder.
- Do not overwrite a project the person opened without asking; save new work under a new name unless told otherwise.

## Check

After a render, confirm the output exists and is the expected size and length (for sequences, the frame count): the `render` result reports the output path and frame count. For a movie the person will watch, `contact_sheet` of the composition right before rendering is the last visual check.
