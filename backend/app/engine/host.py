from __future__ import annotations

import asyncio
import collections
import concurrent.futures
import json
import queue
import threading
import time
from typing import Any, Callable

from .bus import Bus

_STOP = object()


class EngineError(Exception):
    pass


class EngineHost:
    def __init__(self) -> None:
        self.bus = Bus()
        self.engine: Any = None
        self.error: str | None = None
        self.revision = 0
        self.busy: list[str] = []
        self.activity: collections.deque = collections.deque(maxlen=300)
        self._q: queue.Queue = queue.Queue()
        self._ready = threading.Event()
        self._thread: threading.Thread | None = None
        self._source = "ui"
        self._touched = False
        self.frames = concurrent.futures.ThreadPoolExecutor(max_workers=2, thread_name_prefix="formvfx-frame")
        self.jobs = concurrent.futures.ThreadPoolExecutor(max_workers=2, thread_name_prefix="formvfx-job")

    def start(self, loop: asyncio.AbstractEventLoop) -> None:
        self.bus.bind(loop)
        if self._thread:
            return
        self._thread = threading.Thread(target=self._run, name="formvfx-engine", daemon=True)
        self._thread.start()
        self._ready.wait(120)

    def stop(self) -> None:
        if self._thread:
            self._q.put(_STOP)
            self._thread.join(timeout=5)
        self.frames.shutdown(wait=False, cancel_futures=True)
        self.jobs.shutdown(wait=False, cancel_futures=True)

    @property
    def available(self) -> bool:
        return self.engine is not None

    def _run(self) -> None:
        try:
            import fvx_core

            self.engine = fvx_core.Engine()
            self.revision = self.engine.revision()
        except BaseException as exc:
            self.error = f"the compositing core did not load: {exc!r}. Run ./formvfx install-native"
            self._ready.set()
            return
        self._ready.set()
        last_tick = 0.0
        while True:
            try:
                item = self._q.get(timeout=0.1)
            except queue.Empty:
                item = None
            if item is _STOP:
                break
            if item is not None:
                fn, fut, source, touches = item
                if fut.set_running_or_notify_cancel():
                    self._source = source
                    try:
                        fut.set_result(fn(self.engine))
                    except BaseException as exc:
                        fut.set_exception(exc)
                    if touches:
                        self._touched = True
            now = time.time()
            if now - last_tick >= 0.1:
                last_tick = now
                try:
                    report = json.loads(self.engine.tick(now))
                except BaseException:
                    report = {"busy": []}
                busy = report.get("busy") or []
                if busy != self.busy:
                    self.busy = busy
                    self.bus.publish({"type": "busy", "busy": busy})
                if report.get("autosave"):
                    self.bus.publish({"type": "autosave", **report["autosave"]})
            self._flush()

    def _flush(self) -> None:
        try:
            events = json.loads(self.engine.drain_events())
            rev = self.engine.revision()
        except BaseException:
            return
        for ev in events:
            if ev.get("type") == "projectChanged":
                continue
            ev["source"] = self._source
            self.bus.publish(ev)
        if rev != self.revision or self._touched:
            edited = rev != self.revision
            self.revision = rev
            self._touched = False
            self.bus.publish({"type": "changed", "revision": rev, "edited": edited, "source": self._source})

    def submit(self, fn: Callable[[Any], Any], source: str = "ui", touches: bool = False) -> concurrent.futures.Future:
        fut: concurrent.futures.Future = concurrent.futures.Future()
        if self.engine is None:
            fut.set_exception(EngineError(self.error or "the compositing core is not loaded"))
            return fut
        self._q.put((fn, fut, source, touches))
        return fut

    async def call(self, fn: Callable[[Any], Any], source: str = "ui", touches: bool = False) -> Any:
        return await asyncio.wrap_future(self.submit(fn, source, touches))

    def call_sync(self, fn: Callable[[Any], Any], source: str = "ui", timeout: float = 600) -> Any:
        return self.submit(fn, source).result(timeout)

    def note(self, source: str, command: str, ok: bool, error: str | None = None, label: str | None = None) -> None:
        entry = {"time": time.time(), "source": source, "command": command, "ok": ok, "error": error, "label": label}
        self.activity.append(entry)
        if source != "ui":
            self.bus.publish({"type": "activity", **entry})


host = EngineHost()
