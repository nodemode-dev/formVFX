from __future__ import annotations

import asyncio
import concurrent.futures
import json
import os
import queue
import threading
from collections import OrderedDict
from pathlib import Path
from typing import Any, Callable

from .host import host
from .rpc import RpcError, execute

FORMAT = "formvfx.link/1"
SUFFIX = ".fvxlink"


class _Side:
    def __init__(self, path: str):
        self.path = path
        self.mtime = -1.0
        self._q: queue.Queue = queue.Queue()
        self._thread = threading.Thread(target=self._run, name="formvfx-link", daemon=True)
        self._thread.start()

    def _run(self) -> None:
        from .. import native

        engine = native.module().Engine()
        while True:
            fn, fut = self._q.get()
            if fn is None:
                break
            if fut.set_running_or_notify_cancel():
                try:
                    fut.set_result(fn(engine))
                except BaseException as exc:
                    fut.set_exception(exc)

    async def call(self, fn: Callable[[Any], Any]) -> Any:
        fut: concurrent.futures.Future = concurrent.futures.Future()
        self._q.put((fn, fut))
        return await asyncio.wrap_future(fut)

    def stop(self) -> None:
        self._q.put((None, None))


_sides: OrderedDict[str, _Side] = OrderedDict()
_lock = asyncio.Lock()


def _norm(path: str) -> str:
    return os.path.normcase(str(Path(path).expanduser().resolve()))


async def _open_path() -> str | None:
    summary = await execute("project.summary", {}, "link")
    path = summary.get("path") if isinstance(summary, dict) else None
    return _norm(path) if path else None


async def _engine(project: str | None) -> tuple[Callable[[Callable[[Any], Any]], Any], bool]:
    if not project:
        return (lambda fn: host.call(fn, "link")), True
    want = _norm(project)
    if want == await _open_path():
        return (lambda fn: host.call(fn, "link")), True
    if Path(want).suffix.lower() not in (".fvxproj", ".ecproj", ".ecprojx"):
        raise RpcError(f"{project} is not a formvfx project (.fvxproj or .ecproj)", -32602)
    if not os.path.isfile(want):
        raise RpcError(f"no project at {project}", -32602)
    async with _lock:
        side = _sides.get(want)
        if side is None:
            side = _Side(want)
            _sides[want] = side
            while len(_sides) > 2:
                _, old = _sides.popitem(last=False)
                old.stop()
        _sides.move_to_end(want)
        mtime = os.path.getmtime(want)
        if mtime != side.mtime:
            raw = await side.call(lambda e: e.execute("file.open", json.dumps({"path": want})))
            side.mtime = mtime
            del raw
    return side.call, False


def _comp(comp: Any) -> dict:
    if comp is None or comp == "":
        return {}
    if isinstance(comp, str) and comp.isdigit():
        return {"comp": int(comp)}
    return {"comp": comp}


async def info(project: str | None, comp: Any) -> dict:
    call, live = await _engine(project)
    try:
        raw = await call(lambda e: e.execute("comp.info", json.dumps(_comp(comp))))
        rev = await call(lambda e: e.revision())
    except ValueError as exc:
        raise RpcError(str(exc)) from None
    c = json.loads(raw) if raw else {}
    keep = ("id", "name", "width", "height", "frameRate", "duration", "workArea", "background")
    return {"format": FORMAT, "project": project, "live": live, "revision": rev, "comp": {k: c.get(k) for k in keep}}


async def frame(project: str | None, comp: Any, time: float, max_side: int, transparent: bool) -> tuple[int, int, bytes]:
    from .. import native

    call, _ = await _engine(project)
    params = {"maxSide": max_side, "transparent": transparent, "display": False, "time": time, **_comp(comp)}
    try:
        job = await call(lambda e: e.frame_job(json.dumps(params)))
    except ValueError as exc:
        raise RpcError(str(exc)) from None
    loop = asyncio.get_running_loop()
    w, h, px = await loop.run_in_executor(host.frames, job.run)
    png = await loop.run_in_executor(host.frames, native.encode_png, w, h, px)
    return w, h, png


async def audio(project: str | None, comp: Any, start: float, seconds: float, rate: int) -> bytes:
    call, _ = await _engine(project)
    params = {"start": start, "seconds": seconds, "rate": rate, **_comp(comp)}
    try:
        job = await call(lambda e: e.audio_job(json.dumps(params)))
    except ValueError as exc:
        raise RpcError(str(exc)) from None
    return await asyncio.get_running_loop().run_in_executor(host.jobs, job.run)


async def export(comp: Any, path: str | None, source: str) -> dict:
    summary = await execute("project.summary", {}, source)
    project = summary.get("path") if isinstance(summary, dict) else None
    if not project:
        raise RpcError("save the project first: a Dynamic Link clip points at the saved project file")
    data = await info(project, comp)
    name = (data["comp"].get("name") or "Comp").replace("/", "-").replace("\\", "-")
    out = Path(path).expanduser() if path else Path(project).with_name(f"{Path(project).stem} - {name}{SUFFIX}")
    if out.suffix != SUFFIX:
        out = out.with_suffix(SUFFIX)
    descriptor = {
        "format": FORMAT,
        "project": str(Path(project).resolve()),
        "comp": {"id": data["comp"].get("id"), "name": data["comp"].get("name")},
        "width": data["comp"].get("width"),
        "height": data["comp"].get("height"),
        "frameRate": data["comp"].get("frameRate"),
        "duration": data["comp"].get("duration"),
        "frames": "/api/link/frame.png?project={project}&comp={comp}&time={seconds}",
        "audio": "/api/link/audio?project={project}&comp={comp}&start={seconds}&seconds={length}",
    }
    out.write_text(json.dumps(descriptor, indent=2), encoding="utf-8")
    return {"path": str(out), "link": descriptor}


def stop_all() -> None:
    for side in _sides.values():
        side.stop()
    _sides.clear()
