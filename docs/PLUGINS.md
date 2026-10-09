# Effect plug-ins

formvfx loads effect plug-ins written as WebAssembly modules. A plug-in is one `.wasm` (or `.wat`) file that describes its effect and its parameters as JSON and processes the layer's pixels in place. It runs in a sandbox: it can see only the buffers formvfx hands it, has no file or network access, is limited to 2 GB of memory, and is stopped when it runs for too long, so a faulty plug-in cannot hang or crash formvfx. After Effects SDK plug-ins are native code for another host and do not load.

## Installing

- Put the file in the Plug-ins folder (`~/.formvfx/Plug-ins`, or `$FORMVFX_DATA/Plug-ins`); every plug-in there loads when formvfx starts.
- Or choose Effect > Load Plug-in... (`effect.plugins.load {path, install?}`); `install: true` copies it into the Plug-ins folder.
- `effect.plugins.list` shows what loaded, the parameters of each and any errors; `effect.plugins.reload` loads the folder again.

A loaded plug-in appears in the Effect menu and Effects & Presets under its category (Plug-ins by default) and is applied, keyframed, saved and rendered like any built-in effect. Its id is `plugin.` followed by the id it declares. A project that uses a plug-in which is not loaded opens normally; the effect is listed by File > Dependencies > Find Missing Effects and is skipped in renders until the plug-in is loaded.

## The ABI (version 1)

The module exports its linear memory as `memory` and these functions:

| Export | Signature | Does |
| --- | --- | --- |
| `fvx_abi` | `() -> i32` | optional; returns 1 |
| `fvx_describe` | `() -> i64` | the description JSON: pointer in the high 32 bits, byte length in the low 32 bits |
| `fvx_alloc` | `(size: i32) -> i32` | returns a pointer to `size` writable bytes |
| `fvx_render` | `(px: i32, w: i32, h: i32, params: i32, len: i32, time: f64) -> i32` | processes the image in place; returns 0 on success |

It may import `env.fvx_log(ptr: i32, len: i32)` to write a UTF-8 line, which the engine prints to its standard error prefixed with the plug-in's id. A render that traps, runs out of time or returns non-zero leaves the layer as it was and is reported by `effect.plugins.list` until the next good render. A fresh instance is made for every render, so a plug-in keeps no state between frames.

### The description

```json
{
  "id": "acme.radialTint",
  "name": "Radial Tint",
  "category": "Acme",
  "vendor": "Acme",
  "version": "1.0.0",
  "params": [
    {"id": "amount", "name": "Amount", "type": "slider", "min": 0, "max": 100, "default": 50, "decimals": 1},
    {"id": "color", "name": "Color", "type": "color", "default": [1, 0.5, 0]},
    {"id": "center", "name": "Center", "type": "point", "default": [0, 0]},
    {"id": "angle", "name": "Angle", "type": "angle", "default": 0},
    {"id": "invert", "name": "Invert", "type": "checkbox", "default": false},
    {"id": "mode", "name": "Mode", "type": "popup", "options": ["Soft", "Hard"], "default": 0},
    {"id": "source", "name": "Source Layer", "type": "layer"}
  ]
}
```

`id` is letters, digits, dots, dashes and underscores. A `name` may not repeat a built-in effect's name. Parameter ids may not contain `/` and may not be `plugin`. Slider parameters also take `sliderMin` and `sliderMax`.

### Rendering

`px` points at `w * h` pixels of four little-endian 32-bit floats each, red, green, blue and alpha, premultiplied by alpha, in the project's working colour space (values above 1.0 are kept in 32 bpc projects). The plug-in writes its result into the same buffer.

`params` points at `len` bytes of JSON:

```json
{"time": 1.25, "scale": 0.5, "origin": [0, 0], "layerSize": [1920, 1080], "fps": 24, "values": {"amount": 50, "color": [1, 0.5, 0, 1], "center": [960, 540], "invert": false}}
```

`time` is the layer time in seconds. Point parameters are in layer pixels; the buffer covers the layer at `scale` (0.5 for a half-resolution preview) starting at `origin`, so a point lands at `((x - origin[0]) * scale, (y - origin[1]) * scale)` in the buffer. Lengths in layer pixels are multiplied by `scale` the same way. Colours are `[r, g, b, a]` from 0 to 1, angles are degrees, checkboxes are booleans and popups are 0-based indices.

## Example

`native/plugins/radial-tint` is a complete plug-in in Rust with no dependencies. Build it with:

```bash
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown --manifest-path native/plugins/radial-tint/Cargo.toml
```

and load `native/plugins/radial-tint/target/wasm32-unknown-unknown/release/radial_tint.wasm`. Any language that compiles to WebAssembly works; a plug-in can also be written by hand in the text format (`.wat`).
