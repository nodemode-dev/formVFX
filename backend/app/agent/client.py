from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

from ..core import discovery

_started = False
HEALTH_RETRY_SECONDS = 10.0


_problem = {"text": ""}
NOT_RUNNING = "no engine is running and it could not be started: run ./formvfx boot (or open the desktop app)"


def problem() -> str:
    return _problem["text"] or NOT_RUNNING


def engine_url(autostart: bool = True) -> str | None:
    explicit = os.environ.get("FORMVFX_URL", "").strip().rstrip("/")
    url = discovery.running_engine()
    if url:
        return url
    held = discovery.held_port()
    if held:
        if discovery.healthy(held, HEALTH_RETRY_SECONDS):
            return held
        port = held.rsplit(":", 1)[-1]
        _problem["text"] = (f"something is listening on {held} but does not answer {held}/api/health within {HEALTH_RETRY_SECONDS:g} s: the engine there may be stuck. "
                            f"A second engine was not started because the port is taken; find the process with lsof -nP -iTCP:{port} -sTCP:LISTEN and stop it, "
                            "or set FORMVFX_PORT to a free port, then retry")
        return None
    if explicit:
        _problem["text"] = f"no engine answers at FORMVFX_URL={explicit}"
        return None
    if autostart and start_engine():
        return discovery.running_engine(timeout=2)
    return None


def start_engine() -> bool:
    global _started
    if _started or os.environ.get("FORMVFX_AUTOSTART", "1") == "0":
        return False
    _started = True
    if getattr(sys, "frozen", False):
        command = [sys.executable, "--headless"]
        cwd = None
    else:
        root = Path(__file__).resolve().parents[3]
        launcher = root / ("formvfx.bat" if os.name == "nt" else "formvfx")
        if not launcher.is_file():
            return False
        built = (root / "frontend" / "dist" / "index.html").is_file()
        command = [str(launcher), "serve" if built else "backend"]
        cwd = str(root)
    log = open(os.path.join(tempfile.gettempdir(), "formvfx-engine.log"), "ab")
    print(f"[formvfx] starting the engine ({' '.join(command[-2:])}); log: {log.name}", file=sys.stderr, flush=True)
    kwargs = {"start_new_session": True} if os.name != "nt" else {"creationflags": 0x00000008}
    subprocess.Popen(command, cwd=cwd, stdin=subprocess.DEVNULL, stdout=log, stderr=log, **kwargs)
    for _ in range(240):
        time.sleep(0.5)
        if discovery.running_engine(timeout=1):
            return True
    return False


def headers(source: str | None = None) -> dict:
    h = {"Content-Type": "application/json", "Accept": "application/json", "x-formvfx-local": "1"}
    h["x-formvfx-source"] = source or os.environ.get("FORMVFX_AGENT_NAME") or "cli"
    token = os.environ.get("FORMVFX_AGENT_TOKEN")
    if token:
        h["x-formvfx-agent-token"] = token
    return h


def rpc(method: str, params: dict | None = None, source: str | None = None, timeout: float = 900) -> object:
    url = engine_url()
    if not url:
        raise SystemExit(f"[formvfx] {problem()}")
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}).encode()
    req = urllib.request.Request(f"{url}/api/rpc", data=body, method="POST", headers=headers(source))
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            reply = json.loads(r.read().decode() or "{}")
    except urllib.error.HTTPError as exc:
        raise SystemExit(f"[formvfx] engine refused the request ({exc.code}): {exc.read().decode()[:400]}") from None
    if "error" in reply:
        raise RuntimeError(reply["error"].get("message", "error"))
    return reply.get("result")
