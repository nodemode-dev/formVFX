from __future__ import annotations

import json
import os
import subprocess
import threading
import time
from collections import deque

from ..engine.host import host
from . import claude_code, workspace

STATE: dict = {"session": None, "model": None, "busy": False, "messages": [], "usage": None}
_proc: dict = {"p": None}
_lock = threading.Lock()
_HIDDEN = {"ToolSearch"}


def _emit(kind: str, **data) -> None:
    host.bus.publish({"type": "chat", "kind": kind, **data}, ("ui",))


def _push(message: dict) -> dict:
    STATE["messages"].append(message)
    if len(STATE["messages"]) > 200:
        del STATE["messages"][: len(STATE["messages"]) - 200]
    return message


def state() -> dict:
    return {
        "session": STATE["session"],
        "model": STATE["model"],
        "busy": STATE["busy"],
        "usage": STATE["usage"],
        "messages": STATE["messages"],
        "claude": claude_code.info(),
    }


def reset() -> dict:
    cancel()
    STATE.update(session=None, model=None, messages=[], usage=None)
    _emit("reset")
    return state()


def cancel() -> bool:
    p = _proc["p"]
    if p is None or p.poll() is not None:
        return False
    try:
        p.terminate()
    except OSError:
        return False
    return True


def send(base_url: str, text: str, context: str | None = None) -> dict:
    text = (text or "").strip()
    if not text:
        raise ValueError("Type a message first.")
    with _lock:
        if STATE["busy"]:
            raise RuntimeError("Claude is still answering. Wait for it, or press Stop.")
        prompt = f"{text}\n\n[formvfx context: {context}]" if context else text
        argv = claude_code.headless(base_url, prompt, STATE["session"])
        STATE["busy"] = True
    user = _push({"id": f"u{time.time_ns()}", "role": "user", "text": text})
    reply = _push({"id": f"a{time.time_ns()}", "role": "assistant", "text": "", "tools": [], "done": False, "error": None})
    _emit("user", message=user)
    _emit("start", message=reply)
    threading.Thread(target=_run, args=(argv, reply), daemon=True).start()
    return {"ok": True, "id": reply["id"]}


def _run(argv: list[str], reply: dict) -> None:
    tail: deque = deque(maxlen=40)
    error = None
    try:
        p = subprocess.Popen(
            argv,
            cwd=str(workspace.ensure()),
            env=claude_code.child_env(os.environ.copy()),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
    except OSError as e:
        _finish(reply, f"Claude Code could not start: {e}")
        return
    _proc["p"] = p

    def drain() -> None:
        for line in p.stderr:
            tail.append(line.rstrip())

    threading.Thread(target=drain, daemon=True).start()
    for line in p.stdout:
        line = line.strip()
        if not line:
            continue
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        error = _handle(ev, reply) or error
    p.wait()
    _proc["p"] = None
    if p.returncode and not error and not reply["text"]:
        error = "\n".join(list(tail)[-6:]) or f"Claude Code stopped (exit {p.returncode})."
    _finish(reply, error)


def _handle(ev: dict, reply: dict) -> str | None:
    if ev.get("parent_tool_use_id"):
        return None
    kind = ev.get("type")
    if kind == "system" and ev.get("subtype") == "init":
        STATE["session"] = ev.get("session_id") or STATE["session"]
        STATE["model"] = ev.get("model") or STATE["model"]
        _emit("init", session=STATE["session"], model=STATE["model"])
    elif kind == "stream_event":
        inner = ev.get("event") or {}
        if inner.get("type") == "content_block_delta":
            delta = inner.get("delta") or {}
            if delta.get("type") == "text_delta" and delta.get("text"):
                reply["text"] += delta["text"]
                _emit("delta", id=reply["id"], text=delta["text"])
    elif kind == "assistant":
        for block in (ev.get("message") or {}).get("content") or []:
            if block.get("type") == "tool_use" and block.get("name") not in _HIDDEN:
                name = str(block.get("name") or "tool").replace("mcp__formvfx__", "")
                tool = {"id": block.get("id"), "name": name, "status": "running"}
                reply["tools"].append(tool)
                _emit("tool", id=reply["id"], tool=tool)
    elif kind == "user":
        for block in (ev.get("message") or {}).get("content") or []:
            if isinstance(block, dict) and block.get("type") == "tool_result":
                for tool in reply["tools"]:
                    if tool["id"] == block.get("tool_use_id"):
                        tool["status"] = "error" if block.get("is_error") else "done"
                        _emit("toolDone", id=reply["id"], tool=tool)
    elif kind == "result":
        usage = ev.get("usage") or {}
        STATE["usage"] = {
            "input": usage.get("input_tokens"),
            "output": usage.get("output_tokens"),
            "cost": ev.get("total_cost_usd"),
            "turns": ev.get("num_turns"),
        }
        _emit("usage", usage=STATE["usage"])
        if ev.get("is_error"):
            return str(ev.get("result") or "Claude Code reported an error.")
        if not reply["text"] and isinstance(ev.get("result"), str):
            reply["text"] = ev["result"]
            _emit("delta", id=reply["id"], text=ev["result"])
    return None


def _finish(reply: dict, error: str | None) -> None:
    reply["done"] = True
    reply["error"] = error
    for tool in reply["tools"]:
        if tool["status"] == "running":
            tool["status"] = "error" if error else "done"
    STATE["busy"] = False
    _emit("done", id=reply["id"], error=error, session=STATE["session"])
