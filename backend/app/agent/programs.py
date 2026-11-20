from __future__ import annotations

import json
import os
import subprocess
import sys
import time

from . import agent_lookup, claude_code, terminals

INSTALL = {
    "claude": "irm https://claude.ai/install.ps1 | iex" if sys.platform == "win32" else "curl -fsSL https://claude.ai/install.sh | bash",
    "codex": "npm install -g @openai/codex",
    "cursor": "curl https://cursor.com/install -fsS | bash",
    "antigravity": agent_lookup.install("antigravity"),
}
_cache: dict = {"at": 0.0, "value": None}


def find(kind: str) -> str | None:
    return claude_code.located(kind).path


def _run(path: str, *args: str, timeout: float = 10) -> str | None:
    try:
        out = agent_lookup.run([path, *args], claude_code.child_env(dict(os.environ), path), timeout)
    except (OSError, subprocess.SubprocessError):
        return None
    return (out.stdout or out.stderr).strip()


def _signed_in(kind: str, path: str) -> bool | None:
    if kind != "claude":
        return None
    if os.environ.get("ANTHROPIC_API_KEY") or os.environ.get("CLAUDE_CODE_OAUTH_TOKEN"):
        return True
    try:
        data = json.loads(_run(path, "auth", "status", timeout=15) or "")
    except ValueError:
        return None
    return bool(data.get("loggedIn")) if isinstance(data, dict) and "loggedIn" in data else None


def status(fresh: bool = False) -> dict:
    now = time.monotonic()
    if not fresh and _cache["value"] is not None and now - _cache["at"] < 60:
        return _cache["value"]
    out = {}
    for kind, binary in terminals.AGENTS.items():
        found = claude_code.located(kind, fresh)
        out[kind] = agent_lookup.answer(found, INSTALL[kind], claude_code.OTHERWISE) | {
            "signedIn": _signed_in(kind, found.path) if found.state == "runs" and found.path else None,
            "install": INSTALL[kind],
            "program": binary,
            "title": terminals.TITLES[kind],
        }
    _cache.update(at=now, value=out)
    return out
