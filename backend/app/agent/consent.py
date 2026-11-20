from __future__ import annotations

import asyncio
import json
import secrets
import time

from ..core import config, seal
from ..engine.host import host

TOKENS: set[str] = set()
_session: set[str] = set()
_declined: dict[str, float] = {}
_QUERY_IDS = {
    "app.capabilities", "cache.diskStats", "camera.points", "camera.solveStatus", "command.describe", "command.list", "comp.info", "comp.list",
    "effect.list", "expr.errors", "item.metadata", "keys.info", "layer.style.list", "layer.tree", "markers.list",
    "mask.featherPoint.list", "paint.presets", "prefs.get", "prefs.pages",
    "project.summary", "project.usage", "prop.get", "puppet.info", "renderQueue.formats", "renderQueue.list",
    "renderQueue.templates", "shortcuts.conflicts", "shortcuts.list", "text.animatorProperties", "text.presets",
    "track.status", "warp.status", "text.fonts", "text.fontFeatures", "view.get3D",
    "essential.list", "essential.templates", "effect.plugins.list", "face.models", "file.scripts.list", "scriptui.list",
}
_READ_RPC = {"command.list", "command.describe", "ui.state", "ui.overlays", "ui.graph", "ui.flowchart", "text.library", "roto.objectPreview", "menu.bar", "menu.titles", "effects.catalog", "frame.png", "engine.info", "activity.list", "project.json", "link.info", "render.status", "formext.list"}
ASK_SECONDS = 180
HANGUP_POLL = 0.5
GONE = "The caller went away before the person answered."
ANSWERS = ["deny", "once", "session", "always"]
SCOPE = "agent"
BLOCKED = ("Agents are turned off in this formvfx (FORMVFX_AGENTS=deny), its own Chat and agent tabs included. "
           "Tell the person: they can make the change in the window, or start formvfx again without that setting.")
_PLAIN = {"cli": "The formvfx command line", "terminal": "A program in a formvfx terminal tab"}


def _store():
    return config.data_dir() / "agents.json"


def issue() -> str:
    token = secrets.token_urlsafe(32)
    TOKENS.add(token)
    return token


def revoke(token: str) -> None:
    TOKENS.discard(token)


def in_app(value: str | None) -> bool:
    if not isinstance(value, str) or not value:
        return False
    return any(secrets.compare_digest(value.encode("utf-8"), t.encode("utf-8")) for t in list(TOKENS))


def _stored() -> dict:
    try:
        raw = json.loads(_store().read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    return raw if isinstance(raw, dict) else {}


def _checked() -> dict:
    stored = _stored()
    allowed = stored.get("allowed")
    return seal.effective({**stored, "allowed": sorted(str(a) for a in allowed) if isinstance(allowed, list) else []})


def _always() -> set[str]:
    return set(_checked()["allowed"])


def _remember(client: str) -> None:
    allowed = _always() | {client}
    _store().write_text(json.dumps(seal.sign({"allowed": sorted(allowed)}), indent=2), encoding="utf-8")


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


def agent_name(source: str) -> str:
    name = source.split(":", 1)[-1].strip()
    return _PLAIN.get(name.lower()) or name or "An outside agent"


def asked_tool(name: str, args: dict) -> str:
    command = args.get("command") if name == "execute" else None
    return f"Run the command {command} (tool {name})" if isinstance(command, str) and command else f"Use the tool {name}"


def asked_rpc(message) -> str:
    calls = [m for m in (message if isinstance(message, list) else [message]) if isinstance(m, dict) and mutating_rpc(m)]
    if not calls:
        return ""
    first = calls[0]
    command = (first.get("params") or {}).get("command") if first.get("method") == "command.execute" else None
    what = f"Run the command {command}" if isinstance(command, str) and command else f"Call {first.get('method') or 'the engine'}"
    return what if len(calls) == 1 else f"{what} and {len(calls) - 1} more"


def answer_of(reply: dict | None) -> str | None:
    if not reply:
        return None
    if not reply.get("allow"):
        return "deny"
    remember = reply.get("remember")
    return remember if remember in ("once", "always") else "session"


def state() -> dict:
    checked = _checked()
    return {"policy": config.agents_policy(), "session": sorted(_session), "always": sorted(checked["allowed"]), "seal": checked["seal"], "sealBroken": checked["sealBroken"],
            "answers": list(ANSWERS), "scope": SCOPE, "timeoutSec": ASK_SECONDS}


class _Gone(Exception):
    pass


async def _held(asking, gone):
    if gone is None:
        return await asking
    task = asyncio.ensure_future(asking)
    try:
        while True:
            done, _ = await asyncio.wait({task}, timeout=HANGUP_POLL)
            if done:
                return task.result()
            if await gone():
                raise _Gone()
    finally:
        if not task.done():
            task.cancel()
            await asyncio.gather(task, return_exceptions=True)


async def gate(source: str, token: str | None, mutating: bool, detail: str = "", what: str = "", gone=None) -> str | None:
    if not mutating:
        return None
    if config.agents_policy() == "deny":
        return BLOCKED
    if in_app(token):
        return None
    key = client_key(source)
    if key in _session or key in _always():
        return None
    if _declined.get(key, 0) > time.monotonic():
        return "The person declined edits from this agent a moment ago. Ask them in chat before trying again."
    if not host.bus.windows():
        return ("No formvfx window is open to ask the person for permission. Open formvfx (./formvfx boot, then http://localhost:5250, "
                "or the desktop app) and allow this agent, or start the agent from formvfx's Terminal panel.")
    rid = host.bus.ticket()
    request = {"type": "consent", "client": source, "detail": detail, "requestId": rid, "agent": agent_name(source), "agentKey": key,
               "what": what or "Change the project", "answers": list(ANSWERS), "scope": SCOPE, "timeoutSec": ASK_SECONDS,
               "expiresAt": int((time.time() + ASK_SECONDS) * 1000)}
    reply, reason = None, "cancelled"
    try:
        reply = await _held(host.bus.ask_window(request, timeout=ASK_SECONDS), gone)
        reason = "expired" if reply is None else "answered"
    except _Gone:
        return GONE
    finally:
        host.bus.publish({"type": "consentClosed", "requestId": rid, "reason": reason, "answer": answer_of(reply), "agentKey": key}, kinds=("ui",))
    if not reply:
        return "Nobody answered the permission request in the formvfx window. Ask the person to allow the agent there, then retry."
    if reply.get("allow"):
        if reply.get("remember") == "always":
            _remember(key)
        if reply.get("remember") != "once":
            _session.add(key)
        return None
    _declined[key] = time.monotonic() + 600
    return "The person declined edits from this agent. Ask them in chat what they want instead."
