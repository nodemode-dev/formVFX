from __future__ import annotations

import asyncio
import concurrent.futures
import json
import os
import queue
import threading
import time
from collections import OrderedDict
from pathlib import Path
from typing import Any, Callable

from .. import __version__
from ..core import config
from .host import host
from .rpc import RpcError, execute

FORMAT = "formvfx.link/1"
SUFFIX = ".fvxlink"
PROJECT_EXT = (".fvxproj", ".ecproj", ".ecprojx")
_log: list[dict] = []


def _note(kind: str, project: str | None, comp: Any, status: str, message: str = "", resolved: str | None = None) -> None:
    _log.append({"time": time.time(), "kind": kind, "project": project, "comp": comp, "status": status, "message": message, "resolved": resolved})
    del _log[:-60]


_uids: dict[str, tuple[float, str | None]] = {}


def _peek_uid(path: Path) -> str | None:
    if path.suffix.lower() != ".fvxproj":
        return None
    try:
        mtime = path.stat().st_mtime
        hit = _uids.get(str(path))
        if hit and hit[0] == mtime:
            return hit[1]
        with path.open("r", encoding="utf-8") as fh:
            uid = json.load(fh).get("uid") or None
    except (OSError, ValueError):
        return None
    _uids[str(path)] = (mtime, uid)
    return uid


def _recent() -> list[str]:
    try:
        data = json.loads((config.data_dir() / "recent.json").read_text(encoding="utf-8"))
        return [p for p in data.get("projects", []) if isinstance(p, str)]
    except (OSError, ValueError):
        return []


async def resolve(project: str | None, uid: str | None = None, hint: str | None = None) -> tuple[str | None, str]:
    if not project and not uid:
        return None, "live"
    open_path = await _open_path()
    if project:
        want = _norm(project)
        if want == open_path:
            return want, "live"
        if os.path.isfile(want):
            if not uid or _peek_uid(Path(want)) in (None, uid):
                return want, "direct"
    if uid:
        summary = await execute("project.summary", {}, "link")
        if isinstance(summary, dict) and summary.get("uid") == uid and open_path:
            return open_path, "live"
    name = Path(project).name if project else None
    dirs: list[Path] = []
    for d in [Path(hint).expanduser().parent if hint else None, Path(project).expanduser().parent if project else None, Path(open_path).parent if open_path else None, *[Path(r).parent for r in _recent()]]:
        if d is not None and d.is_dir() and d not in dirs:
            dirs.append(d)
    files: list[Path] = []
    for d in dirs:
        try:
            files.extend(sorted(p for p in d.iterdir() if p.suffix.lower() in PROJECT_EXT and p.is_file()))
        except OSError:
            continue
    files.extend(Path(r) for r in _recent() if os.path.isfile(r))
    by_id = [f for f in files if uid and _peek_uid(f) == uid]
    if by_id:
        by_id.sort(key=lambda f: (f.name != name, -f.stat().st_mtime))
        return _norm(str(by_id[0])), "relinked"
    if name:
        by_name = [f for f in files if f.name == name]
        if by_name:
            by_name.sort(key=lambda f: -f.stat().st_mtime)
            return _norm(str(by_name[0])), "relinked"
    where = ", ".join(str(d) for d in dirs[:6]) or "no folders"
    raise RpcError(f"no project at {project or uid}: the linked project was moved or renamed and was not found in {where}. Open it in formvfx once (it then appears in Open Recent) or keep the .fvxlink beside it.", -32602)


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


async def _engine(project: str | None, uid: str | None = None, hint: str | None = None) -> tuple[Callable[[Callable[[Any], Any]], Any], bool, str | None, str]:
    if project and Path(project).suffix.lower() not in PROJECT_EXT:
        raise RpcError(f"{project} is not a formvfx project (.fvxproj or .ecproj)", -32602)
    want, how = await resolve(project, uid, hint)
    if how == "live" or want is None:
        return (lambda fn: host.call(fn, "link")), True, want, "live"
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
    return side.call, False, want, how


def _comp(comp: Any) -> dict:
    if comp is None or comp == "":
        return {}
    if isinstance(comp, str) and comp.isdigit():
        return {"comp": int(comp)}
    return {"comp": comp}


async def _comp_params(call: Callable[[Callable[[Any], Any]], Any], comp: Any, comp_name: str | None) -> dict:
    params = _comp(comp)
    if not params or not comp_name:
        return params
    try:
        await call(lambda e: e.execute("comp.info", json.dumps(params)))
        return params
    except ValueError:
        return {"comp": comp_name}


async def info(project: str | None, comp: Any, uid: str | None = None, comp_name: str | None = None, hint: str | None = None) -> dict:
    try:
        call, live, path, how = await _engine(project, uid, hint)
        params = await _comp_params(call, comp, comp_name)
        raw = await call(lambda e: e.execute("comp.info", json.dumps(params)))
        rev = await call(lambda e: e.revision())
    except ValueError as exc:
        _note("info", project, comp, "error", str(exc))
        raise RpcError(str(exc)) from None
    except RpcError as exc:
        _note("info", project, comp, "error", str(exc))
        raise
    c = json.loads(raw) if raw else {}
    keep = ("id", "name", "width", "height", "frameRate", "duration", "workArea", "background")
    moved = how == "relinked" or (comp is not None and str(c.get("id")) != str(comp))
    _note("info", project, comp, "relinked" if moved else "ok", "", path)
    return {"format": FORMAT, "formvfx": __version__, "project": path or project, "requested": project, "relinked": how == "relinked", "live": live, "revision": rev, "comp": {k: c.get(k) for k in keep}}


async def frame(project: str | None, comp: Any, time: float, max_side: int, transparent: bool, uid: str | None = None, comp_name: str | None = None, hint: str | None = None) -> tuple[int, int, bytes]:
    from .. import native

    try:
        call, _, path, how = await _engine(project, uid, hint)
        params = {"maxSide": max_side, "transparent": transparent, "display": False, "time": time, **(await _comp_params(call, comp, comp_name))}
        job = await call(lambda e: e.frame_job(json.dumps(params)))
    except ValueError as exc:
        _note("frame", project, comp, "error", str(exc))
        raise RpcError(str(exc)) from None
    except RpcError as exc:
        _note("frame", project, comp, "error", str(exc))
        raise
    if how == "relinked" and (not _log or _log[-1].get("resolved") != path or _log[-1]["kind"] != "frame"):
        _note("frame", project, comp, "relinked", "", path)
    loop = asyncio.get_running_loop()
    w, h, px = await loop.run_in_executor(host.frames, job.run)
    png = await loop.run_in_executor(host.frames, native.encode_png, w, h, px)
    return w, h, png


async def audio(project: str | None, comp: Any, start: float, seconds: float, rate: int, uid: str | None = None, comp_name: str | None = None, hint: str | None = None) -> bytes:
    try:
        call, _, _path, _how = await _engine(project, uid, hint)
        params = {"start": start, "seconds": seconds, "rate": rate, **(await _comp_params(call, comp, comp_name))}
        job = await call(lambda e: e.audio_job(json.dumps(params)))
    except ValueError as exc:
        _note("audio", project, comp, "error", str(exc))
        raise RpcError(str(exc)) from None
    return await asyncio.get_running_loop().run_in_executor(host.jobs, job.run)


async def export(comp: Any, path: str | None, source: str) -> dict:
    summary = await execute("project.summary", {}, source)
    project = summary.get("path") if isinstance(summary, dict) else None
    if not project:
        raise RpcError("save the project first: a Live Link clip points at the saved project file")
    data = await info(project, comp)
    name = (data["comp"].get("name") or "Comp").replace("/", "-").replace("\\", "-")
    out = Path(path).expanduser() if path else Path(project).with_name(f"{Path(project).stem} - {name}{SUFFIX}")
    if out.suffix != SUFFIX:
        out = out.with_suffix(SUFFIX)
    descriptor = {
        "format": FORMAT,
        "formvfx": __version__,
        "project": str(Path(project).resolve()),
        "projectId": summary.get("uid") if isinstance(summary, dict) else None,
        "comp": {"id": data["comp"].get("id"), "name": data["comp"].get("name")},
        "width": data["comp"].get("width"),
        "height": data["comp"].get("height"),
        "frameRate": data["comp"].get("frameRate"),
        "duration": data["comp"].get("duration"),
        "frames": "/api/link/frame.png?project={project}&projectId={projectId}&comp={comp}&compName={compName}&time={seconds}",
        "audio": "/api/link/audio?project={project}&projectId={projectId}&comp={comp}&compName={compName}&start={seconds}&seconds={length}",
        "check": "/api/link/check",
        "render": "/api/link/render",
    }
    out.write_text(json.dumps(descriptor, indent=2), encoding="utf-8")
    _note("export", project, data["comp"].get("id"), "ok", str(out), project)
    return {"path": str(out), "link": descriptor}


def _major(fmt: Any) -> int | None:
    if not isinstance(fmt, str) or not fmt.startswith("formvfx.link/"):
        return None
    try:
        return int(fmt.split("/", 1)[1].split(".")[0])
    except ValueError:
        return None


async def check(descriptor: dict, path: str | None = None, write: bool = False) -> dict:
    mine = _major(FORMAT) or 1
    theirs = _major(descriptor.get("format"))
    if theirs is None:
        raise RpcError("this is not a formvfx Live Link descriptor (its format should be formvfx.link/1)", -32602)
    if theirs > mine:
        raise RpcError(f"this link was written by a newer formvfx ({descriptor.get('formvfx') or 'format ' + str(theirs)}); this formvfx {__version__} reads format {mine}. Update formvfx to open it.", -32602)
    comp = descriptor.get("comp") or {}
    data = await info(descriptor.get("project"), comp.get("id"), descriptor.get("projectId"), comp.get("name"), path)
    warnings = []
    if descriptor.get("formvfx") and str(descriptor["formvfx"]) != __version__:
        warnings.append(f"written by formvfx {descriptor['formvfx']}, served by {__version__}")
    for k in ("width", "height", "frameRate"):
        if descriptor.get(k) is not None and data["comp"].get(k) is not None and descriptor[k] != data["comp"][k]:
            warnings.append(f"the composition's {k} changed from {descriptor[k]} to {data['comp'][k]}")
    moved = data["relinked"] or str(data["comp"].get("id")) != str(comp.get("id"))
    fixed = None
    if moved and write and path:
        new = {**descriptor, "project": data["project"], "comp": {"id": data["comp"].get("id"), "name": data["comp"].get("name")}, "formvfx": __version__}
        Path(path).expanduser().write_text(json.dumps(new, indent=2), encoding="utf-8")
        fixed = new
    return {"ok": True, "relinked": moved, "project": data["project"], "comp": data["comp"], "warnings": warnings, "rewritten": fixed is not None, "link": fixed or descriptor}


async def render(descriptor: dict, output: str | None = None, fmt: str = "prores") -> dict:
    comp = descriptor.get("comp") or {}
    data = await info(descriptor.get("project"), comp.get("id"), descriptor.get("projectId"), comp.get("name"))
    project = data["project"]
    if not project:
        raise RpcError("the linked project has never been saved", -32602)
    name = (data["comp"].get("name") or "Comp").replace("/", "-").replace("\\", "-")
    ext = {"prores": ".mov", "h264": ".mp4", "hevc": ".mp4", "webm": ".webm", "exr": ".exr", "png": ".png"}.get(fmt, ".mov")
    out = Path(output).expanduser() if output else Path(project).with_name(f"{Path(project).stem} - {name} (rendered){ext}")
    warnings = []
    if data["live"]:
        summary = await execute("project.summary", {}, "link")
        if isinstance(summary, dict) and summary.get("dirty"):
            warnings.append("the project has unsaved changes; the render uses the saved file")
    side = _sides.get(project) or _Side(project)
    _sides[project] = side
    try:
        await side.call(lambda e: e.execute("file.open", json.dumps({"path": project})))
        side.mtime = os.path.getmtime(project)
        params = {"comp": data["comp"].get("id"), "format": fmt, "output": str(out)}
        await side.call(lambda e: e.execute("renderQueue.add", json.dumps(params)))
        await side.call(lambda e: e.execute("renderQueue.render", json.dumps({"wait": True})))
    except ValueError as exc:
        _note("render", project, comp.get("id"), "error", str(exc))
        raise RpcError(str(exc)) from None
    _note("render", project, comp.get("id"), "ok", str(out), project)
    return {"path": str(out), "exists": out.exists(), "warnings": warnings, "project": project, "comp": data["comp"]}


def status() -> dict:
    served = [{"project": p, "loaded": s.mtime, "exists": os.path.isfile(p)} for p, s in _sides.items()]
    return {"format": FORMAT, "formvfx": __version__, "served": served, "recent": list(reversed(_log[-40:]))}


def stop_all() -> None:
    for side in _sides.values():
        side.stop()
    _sides.clear()
