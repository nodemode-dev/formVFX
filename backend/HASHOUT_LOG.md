# backend · HASHOUT_LOG

Living context for the engine service. Code carries almost no comments; the reasoning lives here.

## Shape

- `app/engine/host.py` owns the one `fvx_core.Engine` on a dedicated thread (the engine is not `Send`). Everything that touches it is submitted as a callable to that thread; a 100 ms tick drains engine events, polls background jobs and autosaves.
- Frame, audio and thumbnail renders are built on the engine thread as `FrameJob` / `AudioJob` / `ThumbJob` snapshots (an `Arc` of the project) and run on thread pools with the GIL released, so rendering never blocks editing.
- `changed` events go to every window over the bus with `revision`, `edited` (did the project change, which purges RAM previews) and `source`. Commands that do not bump the revision (time.set, layer.select) still publish with `edited: false` so every window follows the time and selection.
- `app/engine/rpc.py` is the single JSON-RPC dispatcher behind `/api/rpc`, the `/api/live` socket, the MCP tools and the CLI. `engine.batch` takes `$N.key` references counted from 1.

## Agent substrate

- `app/agent/tools.py`: the MCP tools and the guide texts. The tool catalogue is the source the harness skill's `references/tools.md` is generated from (`./formvfx agents sync`).
- Consent (`app/agent/consent.py`): a per-boot token in the Terminal panel's environment marks trusted agents; others ask once in a window (Allow, Allow Always, Don't Allow) unless `FORMVFX_AGENTS` is `allow` or `deny`. Reads never ask.
- `app/agent/workspace.py` writes the agent workspace (`~/Formvfx/agent-workspace`): CLAUDE.md, AGENTS.md, MCP configs for the three clients, a `formvfx` shim on PATH and the harness skill.
- `app/agent/stdio.py` is the stdio relay `./formvfx mcp` runs: it finds the engine (FORMVFX_URL, the discovery record, the default port) and starts one headless when none answers.

## Facts that bit

- Unknown parameters in `execute` are ignored by the engine, not rejected.
- `prop.set` takes menu values as 0-based indices only; option names fail.
- Giving a vec3 transform property two values sets z to 0 (scale z 0 matters once a layer turns 3D).
- Keyframe commands (`prop.addKey`, `keys.set`, `keys.select`) take layer seconds; `set_keyframes` converts and snaps.
- `layer.newText` positions are baselines.

## Known gaps

- The desktop sidecar freeze (`setup/freeze_backend`) and the Tauri shell are not in this tree yet.
