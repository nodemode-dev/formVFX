from __future__ import annotations

import asyncio
import itertools
from dataclasses import dataclass, field


@dataclass(eq=False)
class Client:
    kind: str
    queue: asyncio.Queue = field(default_factory=lambda: asyncio.Queue(maxsize=2000))
    name: str = ""
    id: int = 0


class Bus:
    def __init__(self) -> None:
        self.loop: asyncio.AbstractEventLoop | None = None
        self.clients: set[Client] = set()
        self._ids = itertools.count(1)
        self._replies: dict[int, asyncio.Future] = {}

    def bind(self, loop: asyncio.AbstractEventLoop) -> None:
        self.loop = loop

    def join(self, kind: str, name: str = "") -> Client:
        c = Client(kind=kind, name=name, id=next(self._ids))
        self.clients.add(c)
        return c

    def leave(self, c: Client) -> None:
        self.clients.discard(c)

    def windows(self) -> list[Client]:
        return [c for c in self.clients if c.kind == "ui"]

    def _deliver(self, message: dict, kinds: tuple[str, ...] | None) -> None:
        for c in list(self.clients):
            if kinds and c.kind not in kinds:
                continue
            try:
                c.queue.put_nowait(message)
            except asyncio.QueueFull:
                pass

    def publish(self, message: dict, kinds: tuple[str, ...] | None = None) -> None:
        loop = self.loop
        if loop is None:
            return
        try:
            running = asyncio.get_running_loop()
        except RuntimeError:
            running = None
        if running is loop:
            self._deliver(message, kinds)
        else:
            loop.call_soon_threadsafe(self._deliver, message, kinds)

    async def ask_window(self, message: dict, timeout: float) -> dict | None:
        windows = self.windows()
        if not windows:
            return None
        rid = next(self._ids)
        fut: asyncio.Future = asyncio.get_running_loop().create_future()
        self._replies[rid] = fut
        payload = {**message, "requestId": rid}
        for w in windows:
            try:
                w.queue.put_nowait(payload)
            except asyncio.QueueFull:
                pass
        try:
            return await asyncio.wait_for(fut, timeout)
        except asyncio.TimeoutError:
            return None
        finally:
            self._replies.pop(rid, None)

    def answer(self, rid: int, value: dict) -> bool:
        fut = self._replies.get(rid)
        if fut and not fut.done():
            fut.set_result(value)
            return True
        return False
