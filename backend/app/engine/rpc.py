from __future__ import annotations

import asyncio
import base64
import json
from typing import Any

from .. import __version__
from .host import EngineError, host


class RpcError(Exception):
    def __init__(self, message: str, code: int = -32000, data: Any = None):
        super().__init__(message)
        self.code = code
        self.data = data


_effects_cache: list | None = None


def _dumps(params: Any) -> str:
    return json.dumps(params if params is not None else {})


async def execute(command: str, params: Any = None, source: str = "ui") -> Any:
    if not isinstance(command, str) or not command:
        raise RpcError("missing `command`", -32602)
    if command == "renderQueue.render" and (params if isinstance(params, dict) else {}).get("wait", True) is not False:
        started = await execute(command, {**(params if isinstance(params, dict) else {}), "wait": False}, source)
        return await wait_for_render(started.get("started") or [])
    from ..agent.consent import query_command

    query = query_command(command)
    quiet = query and source == "ui"
    try:
        raw = await host.call(lambda e: e.execute(command, _dumps(params)), source, touches=not query)
    except ValueError as exc:
        if not quiet:
            host.note(source, command, False, str(exc))
        raise RpcError(str(exc)) from None
    except EngineError as exc:
        raise RpcError(str(exc), -32001) from None
    if not quiet:
        host.note(source, command, True)
    if command.startswith("effect.plugins."):
        global _effects_cache
        _effects_cache = None
    return json.loads(raw) if raw else None


async def render_queue() -> dict:
    return json.loads(await host.call(lambda e: e.execute("renderQueue.list", "{}")))


async def wait_for_render(ids: list, timeout: float | None = None, settle: float = 5.0) -> dict:
    loop = asyncio.get_running_loop()
    deadline = None if timeout is None else loop.time() + timeout
    idle_since = None
    while True:
        queue = await render_queue()
        done = {r.get("item"): r for r in queue.get("finished") or [] if r.get("item") in ids}
        if len(done) == len(ids) or (not queue.get("rendering") and idle_since is not None and loop.time() - idle_since > settle):
            return {"rendered": ids, "results": [done[i] for i in ids if i in done]}
        if deadline is not None and loop.time() >= deadline:
            return {"started": ids, "running": True, "items": [r for r in queue.get("items") or [] if r.get("id") in ids]}
        if not queue.get("rendering") and idle_since is None:
            idle_since = loop.time()
        await asyncio.sleep(0.25)


async def _native_edit(name: str, fn, source: str) -> Any:
    try:
        raw = await host.call(fn, source, touches=True)
    except ValueError as exc:
        host.note(source, name, False, str(exc))
        raise RpcError(str(exc)) from None
    host.note(source, name, True)
    return json.loads(raw) if raw else None


_NEEDS_SAM = "Object Matte uses the MobileSAM model: install it in Settings > Roto Paint (Download), or run roto.model.download {id: mobilesam}"


async def _object_matte(p: dict, source: str) -> dict:
    layer, point = p.get("layer"), p.get("point")
    if layer is None or not isinstance(point, list) or len(point) != 2:
        raise RpcError("pass `layer` and `point`: [x, y] in layer pixels on the object", -32602)
    if isinstance(layer, str):
        info = await execute("comp.info", {"comp": p["comp"]} if "comp" in p else {}, source)
        rows = info.get("layers", []) if isinstance(info, dict) else []
        found = next((r for r in rows if f"#{r.get('index')}" == layer or r.get("name") == layer), None)
        if not found:
            raise RpcError(f"no layer {layer!r}", -32602)
        layer = found["id"]
    models = (await execute("roto.models", {}, source) or {}).get("models") or []
    sam = next((m for m in models if m.get("id") == "mobilesam"), None)
    if not sam or not sam.get("installed"):
        raise RpcError(_NEEDS_SAM)
    if not sam.get("active"):
        await execute("roto.model.select", {"id": "mobilesam", "wait": True}, source)
    stroke = {"layer": layer, "kind": "fg", "points": [point, [point[0] + 1, point[1] + 1]], "radius": 3}
    if p.get("frame") is not None:
        stroke["frame"] = p["frame"]
    steps = [
        {"command": "roto.stroke", "params": stroke},
        {"command": "roto.span", "params": {"layer": layer, "effect": "$1.effect", "start": -1000000, "end": 1000000}},
    ]
    made = await execute("engine.batch", {"steps": steps, "label": "Object Matte"}, source)
    results = made.get("results") if isinstance(made, dict) else None
    first = (results or [{}])[0] if isinstance(results, list) else {}
    effect = first.get("effect") if isinstance(first, dict) else None
    args = {"layer": layer, "direction": p.get("direction") or "both", "wait": bool(p.get("wait"))}
    if effect is not None:
        args["effect"] = effect
    prop = await execute("roto.propagate", args, source)
    return {"layer": layer, "effect": effect, "span": (results[1] if isinstance(results, list) and len(results) > 1 else {}).get("span"), "frames": prop.get("frames"), "running": prop.get("running")}


async def render_png(params: dict, source: str = "ui") -> dict:
    from .. import native

    job = await host.call(lambda e: e.frame_job(_dumps(params)), source)
    loop = asyncio.get_running_loop()
    w, h, px = await loop.run_in_executor(host.frames, job.run)
    png = await loop.run_in_executor(host.frames, native.encode_png, w, h, px)
    return {"width": w, "height": h, "png": png}


async def formext_list() -> dict:
    reply = await host.bus.ask_window({"type": "uiRequest", "what": "extensions.list"}, timeout=5)
    if reply is not None:
        return reply
    from ..routers import formext

    found = formext.listing()
    rows = [{k: e[k] for k in ("id", "name", "version", "source", "state", "reason", "permissions")} | {"commands": [{"id": c["id"], "title": c["title"]} for c in e["commands"]]} for e in found["extensions"]]
    return {"windows": 0, "note": "No FormVFX window is open: extensions run in the window, so their commands wait until one is open.", "extensions": rows}


async def formext_command(ext_id: Any, command: Any, args: Any) -> Any:
    if not isinstance(ext_id, str) or not isinstance(command, str) or not ext_id or not command:
        raise RpcError("`id` and `command` are required (extensions_list shows them)", -32602)
    if not host.bus.windows():
        raise RpcError("No FormVFX window is open, and extensions run in the window: open FormVFX, then retry")
    reply = await host.bus.ask_window({"type": "uiRequest", "what": "extensions.command", "id": ext_id, "command": command, "args": args if isinstance(args, dict) else {}}, timeout=330)
    if reply is None:
        raise RpcError(f"the FormVFX window did not finish {ext_id}: {command} in time")
    if not reply.get("ok"):
        raise RpcError(str(reply.get("error") or "the extension command failed"), int(reply.get("code") or -32004))
    return reply.get("result")


async def dispatch(method: str, params: Any, source: str = "ui") -> Any:
    global _effects_cache
    p = params if isinstance(params, dict) else {}
    try:
        if method == "command.execute":
            return await execute(p.get("command"), p.get("params") or {}, source)
        if method == "command.batch":
            steps = p.get("steps")
            if not isinstance(steps, list) or not steps:
                raise RpcError("`steps` must be a non-empty list of {command, params}", -32602)
            args = {"steps": steps}
            if p.get("label"):
                args["label"] = p["label"]
            if "atomic" in p:
                args["atomic"] = bool(p["atomic"])
            return await execute("engine.batch", args, source)
        if method == "command.list":
            return await execute("command.list", {k: v for k, v in p.items() if k in ("filter", "enabledOnly", "schemas")}, source)
        if method == "command.describe":
            return await execute("command.describe", {"command": p.get("command") or p.get("id")}, source)
        if method == "ui.state":
            return json.loads(await host.call(lambda e: e.ui_state(_dumps(p)), source))
        if method == "ui.overlays":
            return json.loads(await host.call(lambda e: e.overlays(_dumps(p)), source))
        if method == "ui.graph":
            return json.loads(await host.call(lambda e: e.graph(_dumps(p)), source))
        if method == "ui.flowchart":
            return json.loads(await host.call(lambda e: e.flowchart(_dumps(p)), source))
        if method == "menu.bar":
            return json.loads(await host.call(lambda e: e.menu_bar(_dumps(p)), source))
        if method == "menu.titles":
            return json.loads(await host.call(lambda e: e.menu_titles(), source))
        if method == "effects.catalog":
            if _effects_cache is None:
                _effects_cache = json.loads(await host.call(lambda e: e.effects(), source))
            return _effects_cache
        if method == "gesture.begin":
            return await host.call(lambda e: e.begin_gesture(), source)
        if method == "state.touch":
            await host.call(lambda e: None, source, touches=True)
            return True
        if method == "gesture.end":
            await host.call(lambda e: e.end_gesture(), source)
            return True
        if method == "frame.png":
            out = await render_png(p, source)
            return {"width": out["width"], "height": out["height"], "png": base64.b64encode(out["png"]).decode()}
        if method == "engine.info":
            caps = json.loads(await host.call(lambda e: e.capabilities(), source))
            from .. import native

            return {"name": "formvfx", "version": __version__, "core": json.loads(native.version()), "capabilities": caps, "gpu": host.gpu, "revision": host.revision, "busy": host.busy}
        if method == "text.library":
            from . import text_presets

            return text_presets.listing(p.get("category"), p.get("query"))
        if method == "text.applyLibraryPreset":
            from . import text_presets

            preset = text_presets.find(str(p.get("preset") or ""))
            if preset is None:
                raise RpcError(f"no text preset {p.get('preset')!r} (see text.library)", -32602)
            args = {k: v for k, v in p.items() if k in ("layer", "layers", "comp", "time")}
            refs = args.get("layers") if isinstance(args.get("layers"), list) else [args["layer"]] if "layer" in args else []
            if any(isinstance(r, str) and r.startswith("#") for r in refs):
                info = await execute("comp.info", {"comp": args["comp"]} if "comp" in args else {}, source)
                rows = info.get("layers", []) if isinstance(info, dict) else []
                by_index = {f"#{lr.get('index')}": lr.get("id") for lr in rows}
                args.pop("layer", None)
                args["layers"] = [by_index.get(r, r) if isinstance(r, str) else r for r in refs]
            args["preset"] = preset
            return await _native_edit("text.applyLibraryPreset", lambda e: e.apply_text_preset(_dumps(args)), source)
        if method == "clipboard.pasteSvg":
            args = {k: v for k, v in p.items() if k in ("svg", "name", "comp")}
            return await _native_edit("clipboard.pasteSvg", lambda e: e.paste_svg(_dumps(args)), source)
        if method == "roto.objectPreview":
            args = {k: v for k, v in p.items() if k in ("layer", "point", "time", "comp")}
            try:
                job = await host.call(lambda e: e.object_job(_dumps(args)), source)
                return json.loads(await asyncio.get_running_loop().run_in_executor(host.jobs, job.run))
            except ValueError as exc:
                raise RpcError(str(exc)) from None
        if method == "roto.objectMatte":
            return await _object_matte(p, source)
        if method == "track.planar":
            args = {k: v for k, v in p.items() if k in ("layer", "mask", "target", "surface", "reference", "track", "start", "end", "comp")}
            return await _native_edit("track.planar", lambda e: e.planar_corner_pin(_dumps(args)), source)
        if method == "engine.performance":
            from . import performance

            if any(k in p for k in ("multiFrame", "reservedCores", "gpu")):
                saved = performance.save(p)
                host.set_preview_frames(performance.preview_frames(saved))
                if "multiFrame" in p:
                    await host.call(lambda e: host.apply_multi_frame(), source)
                if "gpu" in p:
                    await host.call(lambda e: host.attach_gpu(), source, touches=True)
            return {**performance.describe(), "gpuStatus": host.gpu}
        if method == "render.status":
            return await render_queue()
        if method == "activity.list":
            n = int(p.get("limit") or 50)
            return list(host.activity)[-n:]
        if method == "project.json":
            return json.loads(await host.call(lambda e: e.project_json(), source))
        if method == "link.export":
            from . import link

            return await link.export(p.get("comp"), p.get("path"), source)
        if method == "link.info":
            from . import link

            return await link.info(p.get("project"), p.get("comp"))
        if method == "formext.list":
            return await formext_list()
        if method == "formext.command":
            return await formext_command(p.get("id"), p.get("command"), p.get("args"))
    except RpcError:
        raise
    except ValueError as exc:
        raise RpcError(str(exc)) from None
    except EngineError as exc:
        raise RpcError(str(exc), -32001) from None
    raise RpcError(f"unknown method {method}", -32601)


async def handle(message: Any, source: str = "ui") -> dict | list | None:
    if isinstance(message, list):
        out = [r for r in [await handle(m, source) for m in message] if r is not None]
        return out or None
    if not isinstance(message, dict):
        return {"jsonrpc": "2.0", "id": None, "error": {"code": -32600, "message": "Invalid request"}}
    rid = message.get("id")
    try:
        result = await dispatch(message.get("method", ""), message.get("params"), source)
    except RpcError as exc:
        return None if rid is None else {"jsonrpc": "2.0", "id": rid, "error": {"code": exc.code, "message": str(exc), **({"data": exc.data} if exc.data is not None else {})}}
    return None if rid is None else {"jsonrpc": "2.0", "id": rid, "result": result}
