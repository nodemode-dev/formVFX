import asyncio
import contextlib
import json

from fastapi import APIRouter, WebSocket, WebSocketDisconnect

from .. import __version__
from ..engine import rpc
from ..engine.host import host

router = APIRouter(tags=["live"])
_SLOW = {"frame.png", "engine.info"}


@router.websocket("/api/live")
async def live(socket: WebSocket) -> None:
    await socket.accept()
    kind = socket.query_params.get("kind", "ui")
    client = host.bus.join("ui" if kind == "ui" else "watch", socket.query_params.get("name", ""))

    async def pump() -> None:
        while True:
            message = await client.queue.get()
            await socket.send_text(json.dumps(message))

    def put(message: dict) -> None:
        with contextlib.suppress(asyncio.QueueFull):
            client.queue.put_nowait(message)

    async def slow(message: dict) -> None:
        reply = await rpc.handle(message, "ui")
        if reply:
            put(reply)

    sender = asyncio.create_task(pump())
    put({"type": "hello", "version": __version__, "revision": host.revision, "busy": host.busy, "core": host.available, "error": host.error, "client": client.id})
    tasks: set[asyncio.Task] = set()
    try:
        while True:
            text = await socket.receive_text()
            try:
                message = json.loads(text)
            except ValueError:
                continue
            if not isinstance(message, dict):
                continue
            if message.get("method") == "reply":
                params = message.get("params") or {}
                host.bus.answer(int(params.get("requestId") or 0), params.get("value") or {})
                continue
            if message.get("method") in _SLOW:
                t = asyncio.create_task(slow(message))
                tasks.add(t)
                t.add_done_callback(tasks.discard)
                continue
            reply = await rpc.handle(message, "ui")
            if reply:
                put(reply)
    except WebSocketDisconnect:
        pass
    finally:
        host.bus.leave(client)
        sender.cancel()
        for t in tasks:
            t.cancel()
