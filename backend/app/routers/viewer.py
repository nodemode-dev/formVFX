import asyncio
import json
import struct
from collections import OrderedDict

from fastapi import APIRouter, Query, Response, WebSocket, WebSocketDisconnect

from .. import native
from ..engine.host import host

router = APIRouter(tags=["viewer"])
HEADER = struct.Struct("<IIIIdI")


async def _render(params: dict):
    job = await host.call(lambda e: e.frame_job(json.dumps(params)))
    loop = asyncio.get_running_loop()
    w, h, px = await loop.run_in_executor(host.frames, job.run)
    return w, h, job.rev, px


@router.websocket("/api/viewer")
async def viewer(socket: WebSocket) -> None:
    await socket.accept()
    latest: dict = {}
    wake = asyncio.Event()
    state = {"range": None, "range_id": 0, "next": 0}

    async def send(rid: int, params: dict, kind: int) -> None:
        try:
            w, h, rev, px = await _render(params)
        except Exception as exc:
            await socket.send_text(json.dumps({"id": rid, "error": str(exc)}))
            return
        t = float(params.get("time") or 0.0)
        await socket.send_bytes(HEADER.pack(rid, w, h, rev, t, kind) + px)

    async def worker() -> None:
        while True:
            await wake.wait()
            wake.clear()
            if latest:
                req = dict(latest)
                latest.clear()
                await send(int(req.get("id") or 0), req, 0)
                if state["range"]:
                    wake.set()
                continue
            rng = state["range"]
            if rng:
                rid = state["range_id"]
                times = rng.get("times") or []
                base = {k: v for k, v in rng.items() if k not in ("times", "type")}
                while state["range_id"] == rid and not latest and state["next"] < len(times):
                    i = state["next"]
                    state["next"] = i + 1
                    await send(rid, {**base, "time": times[i], "index": i}, 1)
                if state["range_id"] == rid and state["next"] >= len(times):
                    state["range"] = None
                    await socket.send_text(json.dumps({"id": rid, "done": True}))
                if latest:
                    wake.set()

    task = asyncio.create_task(worker())
    try:
        while True:
            text = await socket.receive_text()
            try:
                req = json.loads(text)
            except ValueError:
                continue
            kind = req.get("type", "frame")
            if kind == "frame":
                latest.clear()
                latest.update(req)
                wake.set()
            elif kind == "range":
                state["range_id"] = int(req.get("id") or 0)
                state["range"] = req
                state["next"] = 0
                wake.set()
            elif kind == "cancel":
                state["range_id"] = -1
                state["range"] = None
    except WebSocketDisconnect:
        pass
    finally:
        task.cancel()


@router.get("/api/frame.png")
async def frame_png(comp: str | None = None, time: float | None = None, max: int = 1024, transparent: bool = False) -> Response:
    params: dict = {"maxSide": max, "transparent": transparent, "display": False}
    if comp is not None:
        params["comp"] = int(comp) if comp.isdigit() else comp
    if time is not None:
        params["time"] = time
    w, h, _, px = await _render(params)
    loop = asyncio.get_running_loop()
    png = await loop.run_in_executor(host.frames, native.encode_png, w, h, px)
    return Response(png, media_type="image/png", headers={"cache-control": "no-store"})


@router.get("/api/thumb/{item}")
async def thumb(item: int, time: float = 0.0, max: int = Query(160, ge=16, le=1024)) -> Response:
    job = await host.call(lambda e: e.thumbnail_job(item, time, max))
    if job is None:
        return Response(status_code=204)
    loop = asyncio.get_running_loop()
    out = await loop.run_in_executor(host.frames, job.run)
    if not out:
        return Response(status_code=204)
    w, h, px = out
    png = await loop.run_in_executor(host.frames, native.encode_png, w, h, px)
    return Response(png, media_type="image/png", headers={"cache-control": "max-age=5"})


_peaks: OrderedDict[str, bytes] = OrderedDict()
_peaks_running: dict[str, asyncio.Task] = {}


@router.get("/api/peaks/{item}")
async def peaks(item: int) -> Response:
    job = await host.call(lambda e: e.peaks_job(item))
    if job is None:
        return Response(status_code=204)
    key = job.key
    data = _peaks.get(key)
    if data is None:
        task = _peaks_running.get(key)
        if task is None:
            task = asyncio.ensure_future(asyncio.get_running_loop().run_in_executor(host.jobs, job.run))
            _peaks_running[key] = task
            task.add_done_callback(lambda _t: _peaks_running.pop(key, None))
        data = await task
        _peaks[key] = data
        while len(_peaks) > 48:
            _peaks.popitem(last=False)
    _peaks.move_to_end(key)
    return Response(data, media_type="application/octet-stream", headers={"cache-control": "no-store", "x-bins-per-sec": str(job.bins_per_sec)})


@router.get("/api/audio")
async def audio(comp: str | None = None, start: float = 0.0, seconds: float = 2.0, rate: int = 48000) -> Response:
    params: dict = {"start": start, "seconds": seconds, "rate": rate}
    if comp is not None:
        params["comp"] = int(comp) if comp.isdigit() else comp
    job = await host.call(lambda e: e.audio_job(json.dumps(params)))
    loop = asyncio.get_running_loop()
    pcm = await loop.run_in_executor(host.jobs, job.run)
    frames = len(pcm) // 8
    header = b"RIFF" + struct.pack("<I", 36 + len(pcm)) + b"WAVE" + b"fmt " + struct.pack("<IHHIIHH", 16, 3, 2, rate, rate * 8, 8, 32) + b"data" + struct.pack("<I", len(pcm))
    return Response(header + pcm, media_type="audio/wav", headers={"cache-control": "no-store", "x-frames": str(frames)})
