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
    try:
        raw = await host.call(lambda e: e.execute(command, _dumps(params)), source)
    except ValueError as exc:
        host.note(source, command, False, str(exc))
        raise RpcError(str(exc)) from None
    except EngineError as exc:
        raise RpcError(str(exc), -32001) from None
    host.note(source, command, True)
    return json.loads(raw) if raw else None


async def render_png(params: dict, source: str = "ui") -> dict:
    from .. import native

    job = await host.call(lambda e: e.frame_job(_dumps(params)), source)
    loop = asyncio.get_running_loop()
    w, h, px = await loop.run_in_executor(host.frames, job.run)
    png = await loop.run_in_executor(host.frames, native.encode_png, w, h, px)
    return {"width": w, "height": h, "png": png}


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
        if method == "gesture.end":
            await host.call(lambda e: e.end_gesture(), source)
            return True
        if method == "frame.png":
            out = await render_png(p, source)
            return {"width": out["width"], "height": out["height"], "png": base64.b64encode(out["png"]).decode()}
        if method == "engine.info":
            caps = json.loads(await host.call(lambda e: e.capabilities(), source))
            from .. import native

            return {"name": "formvfx", "version": __version__, "core": json.loads(native.version()), "capabilities": caps, "revision": host.revision, "busy": host.busy}
        if method == "activity.list":
            n = int(p.get("limit") or 50)
            return list(host.activity)[-n:]
        if method == "project.json":
            return json.loads(await host.call(lambda e: e.project_json(), source))
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
