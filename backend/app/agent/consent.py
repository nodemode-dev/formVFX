from __future__ import annotations

import json
import secrets
import time

from ..core import config
from ..engine.host import host

TOKEN = secrets.token_urlsafe(24)
_session: set[str] = set()
_declined: dict[str, float] = {}
_QUERY_IDS = {
    "app.capabilities", "cache.diskStats", "camera.points", "camera.solveStatus", "command.describe", "command.list", "comp.info",
    "effect.list", "expr.errors", "item.metadata", "keys.info", "layer.style.list", "layer.tree", "markers.list",
    "mask.featherPoint.list", "paint.presets", "prefs.get", "prefs.pages",
    "project.summary", "project.usage", "prop.get", "puppet.info", "renderQueue.formats", "renderQueue.list",
    "renderQueue.templates", "shortcuts.conflicts", "shortcuts.list", "text.animatorProperties", "text.presets",
    "track.status", "warp.status", "text.fonts", "text.fontFeatures", "view.get3D",
}
_READ_RPC = {"command.list", "command.describe", "ui.state", "ui.overlays", "ui.graph", "ui.flowchart", "text.library", "roto.objectPreview", "menu.bar", "menu.titles", "effects.catalog", "frame.png", "engine.info", "activity.list", "project.json", "link.info"}


def _store():
    return config.data_dir() / "agents.json"


def _always() -> set[str]:
    try:
        return set(json.loads(_store().read_text(encoding="utf-8")).get("allowed", []))
    except (OSError, ValueError):
        return set()


def _remember(client: str) -> None:
    allowed = _always() | {client}
    _store().write_text(json.dumps({"allowed": sorted(allowed)}, indent=2), encoding="utf-8")


def forget_all() -> None:
    _session.clear()
    try:
        _store().unlink()
    except OSError:
        pass


def query_command(command: str | None) -> bool:
    return bool(command) and command in _QUERY_IDS


def mutating_rpc(message) -> bool:
    if isinstance(message, list):
        return any(mutating_rpc(m) for m in message)
    if not isinstance(message, dict):
        return False
    method = message.get("method", "")
    if method in _READ_RPC:
        return False
    if method == "command.execute":
        return not query_command((message.get("params") or {}).get("command"))
    return True


def client_key(source: str) -> str:
    return source.split(":", 1)[-1].strip().lower() or "agent"


def state() -> dict:
    return {"policy": config.agents_policy(), "session": sorted(_session), "always": sorted(_always())}


async def gate(source: str, token: str | None, mutating: bool, detail: str = "") -> str | None:
    if token and secrets.compare_digest(token, TOKEN):
        return None
    policy = config.agents_policy()
    if policy == "allow" or not mutating:
        return None
    if policy == "deny":
        return "Outside agents are turned off (FORMVFX_AGENTS=deny). Start the agent from formvfx's Terminal panel instead."
    key = client_key(source)
    if key in _session or key in _always():
        return None
    if _declined.get(key, 0) > time.monotonic():
        return "The person declined edits from this agent a moment ago. Ask them in chat before trying again."
    if not host.bus.windows():
        return ("No formvfx window is open to ask the person for permission. Open formvfx (./formvfx boot, then http://localhost:5230, "
                "or the desktop app) and allow this agent, start the agent from formvfx's Terminal panel, or set FORMVFX_AGENTS=allow.")
    reply = await host.bus.ask_window({"type": "consent", "client": source, "detail": detail}, timeout=180)
    if not reply:
        return "Nobody answered the permission request in the formvfx window. Ask the person to allow the agent there, then retry."
    if reply.get("allow"):
        if reply.get("remember") == "always":
            _remember(key)
        _session.add(key)
        return None
    _declined[key] = time.monotonic() + 600
    return "The person declined edits from this agent. Ask them in chat what they want instead."
