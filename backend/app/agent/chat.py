from __future__ import annotations

import base64
import copy
import json
import os
import re
import subprocess
import threading
import time
import uuid
from pathlib import Path
from typing import Any

from ..core import config
from ..engine.host import host
from . import claude_code, workspace

MAX_MESSAGES = 200
HIDDEN = {"ToolSearch"}
NOTE = claude_code.NOTE + claude_code.CHAT_NOTE
CONTEXT = re.compile(r"\n\n\[formvfx context: .*\]\s*$", re.S)
_lock = threading.RLock()


def _blank_usage() -> dict:
    return {"input": 0, "output": 0, "costUsd": 0.0, "context": 0, "window": 0, "model": ""}


STATE: dict[str, Any] = {"session": None, "proc": None, "busy": False, "cancelled": False, "run": 0, "messages": [], "usage": _blank_usage()}


def _emit(kind: str, **data: Any) -> None:
    host.bus.publish({"type": "chat", "kind": kind, **data}, ("ui",))


def _window(usage: dict) -> int:
    if usage["window"]:
        return usage["window"]
    model = str(usage["model"] or "")
    if not model:
        return 0
    return 1_000_000 if "1m" in model.lower() else 200_000


def _usage_view() -> dict:
    view = dict(STATE["usage"])
    view["window"] = _window(view)
    return view


def _tool_name(name: str) -> str:
    return name.split("__")[-1] if name.startswith("mcp__") else name


def _brief(value: Any) -> str:
    text = value if isinstance(value, str) else json.dumps(value, ensure_ascii=False, separators=(",", ":"), default=str)
    text = " ".join(text.split())
    return text if len(text) <= 160 else text[:157] + "..."


def _usage_of(u: dict) -> int:
    return int(u.get("input_tokens") or 0) + int(u.get("cache_creation_input_tokens") or 0) + int(u.get("cache_read_input_tokens") or 0)


def _save_image(block: dict) -> str | None:
    src = block.get("source") or {}
    if src.get("type") != "base64" or not src.get("data"):
        return None
    ext = {"image/png": "png", "image/webp": "webp", "image/gif": "gif"}.get(str(src.get("media_type")), "jpg")
    folder = config.data_dir() / "Chat"
    try:
        folder.mkdir(parents=True, exist_ok=True)
        path = folder / f"{uuid.uuid4().hex}.{ext}"
        path.write_bytes(base64.b64decode(src["data"]))
    except (OSError, ValueError):
        return None
    return str(path)


def state() -> dict:
    with _lock:
        out = {
            "busy": STATE["busy"],
            "session": STATE["session"] or "",
            "model": STATE["usage"]["model"],
            "messages": copy.deepcopy(STATE["messages"]),
            "usage": _usage_view(),
        }
    out["claude"] = claude_code.info()
    return out


def reset() -> dict:
    cancel()
    with _lock:
        STATE.update(session=None, messages=[], usage=_blank_usage(), busy=False, run=STATE["run"] + 1)
    _emit("reset")
    return state()


def cancel() -> bool:
    p = STATE["proc"]
    if p is None or p.poll() is not None:
        return False
    STATE["cancelled"] = True
    try:
        p.terminate()
    except OSError:
        return False
    return True


def send(base_url: str, text: str, context: str | None = None, note: str | None = None) -> dict:
    shown = (text or "").strip()
    if not shown:
        raise ValueError("Type a message first.")
    ctx = (context or "").strip()
    prompt = f"{shown}\n\n[formvfx context: {ctx}]" if ctx else shown
    argv = claude_code.headless(base_url, prompt, STATE["session"], NOTE)
    label = (note or "").strip()
    user = {"role": "user", "text": shown, "note": label}
    reply: dict[str, Any] = {"role": "assistant", "parts": [], "error": "", "meta": {}}
    with _lock:
        if STATE["busy"]:
            raise RuntimeError("Claude is still answering. Wait for it, or press Stop.")
        STATE.update(busy=True, cancelled=False, run=STATE["run"] + 1)
        token = STATE["run"]
        STATE["messages"] += [user, reply]
        del STATE["messages"][:-MAX_MESSAGES]
    _emit("start", text=shown, note=label)
    threading.Thread(target=_run, args=(argv, reply, token), daemon=True).start()
    return {"ok": True}


def _live(token: int) -> bool:
    return STATE["run"] == token


def _run(argv: list[str], reply: dict, token: int) -> None:
    tail: list[str] = []
    started = time.time()
    result: dict | None = None
    error = ""
    try:
        proc = subprocess.Popen(
            argv,
            cwd=str(workspace.ensure()),
            env=claude_code.child_env(os.environ.copy()),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            encoding="utf-8",
            errors="replace",
        )
        STATE["proc"] = proc

        def drain() -> None:
            for line in proc.stderr:
                tail.append(line.rstrip())
                del tail[:-15]

        threading.Thread(target=drain, daemon=True).start()
        counters = {"done": 0, "call": 0}
        for line in proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                ev = json.loads(line)
            except ValueError:
                continue
            if _live(token):
                result = _handle(ev, reply, counters) or result
        proc.wait(timeout=30)
    except (OSError, subprocess.SubprocessError) as e:
        error = f"Claude Code could not run: {e}"
    except Exception as e:
        error = f"Claude Code chat failed: {e}"
    finally:
        if STATE["proc"] is not None and _live(token):
            STATE["proc"] = None
    if not _live(token):
        return
    if not error and result is not None and result.get("is_error"):
        error = str(result.get("result") or result.get("subtype") or "Claude Code reported an error.")
    elif not error and STATE["cancelled"]:
        error = "Stopped"
    elif not error and result is None:
        error = "Claude Code stopped before answering" + (f": {tail[-1]}" if tail else ".")
    totals = (result or {}).get("usage") or {}
    with _lock:
        usage = _usage_view()
        reply["error"] = error
        reply["meta"] = {
            "seconds": round(time.time() - started, 1),
            "turns": (result or {}).get("num_turns"),
            "costUsd": (result or {}).get("total_cost_usd"),
            "inputTokens": _usage_of(totals),
            "outputTokens": int(totals.get("output_tokens") or 0),
        }
        for part in reply["parts"]:
            if part["kind"] == "tool" and part["status"] == "running":
                part["status"] = "error" if error else "done"
        STATE["busy"] = False
        STATE["cancelled"] = False
        meta = dict(reply["meta"])
    _emit("done", error=error, meta=meta, usage=usage, session=STATE["session"] or "")


def _add_text(reply: dict, text: str) -> None:
    with _lock:
        if not reply["parts"] or reply["parts"][-1]["kind"] != "text":
            reply["parts"].append({"kind": "text", "text": ""})
        reply["parts"][-1]["text"] += text
    _emit("delta", text=text)


def _handle(ev: dict, reply: dict, counters: dict) -> dict | None:
    if ev.get("parent_tool_use_id"):
        return None
    kind = ev.get("type")
    usage = STATE["usage"]
    if kind == "system" and ev.get("subtype") == "init":
        with _lock:
            STATE["session"] = ev.get("session_id") or STATE["session"]
            usage["model"] = str(ev.get("model") or usage["model"])
    elif kind == "stream_event":
        inner = ev.get("event") or {}
        etype = inner.get("type")
        delta = inner.get("delta") or {}
        if etype in ("message_start", "message_delta"):
            raw = (inner.get("message") or {}).get("usage") if etype == "message_start" else inner.get("usage")
            if isinstance(raw, dict):
                if etype == "message_start":
                    counters["done"] += counters["call"]
                    counters["call"] = 0
                    usage["context"] = _usage_of(raw) or usage["context"]
                else:
                    counters["call"] = int(raw.get("output_tokens") or 0)
                _emit("usage", context=usage["context"], window=_window(usage), model=usage["model"], output=counters["done"] + counters["call"])
        elif etype == "content_block_delta" and delta.get("type") == "text_delta" and delta.get("text"):
            _add_text(reply, delta["text"])
    elif kind == "assistant":
        for block in (ev.get("message") or {}).get("content") or []:
            if isinstance(block, dict) and block.get("type") == "tool_use" and block.get("name") not in HIDDEN:
                part = {"kind": "tool", "id": block.get("id"), "name": _tool_name(str(block.get("name") or "tool")), "input": _brief(block.get("input") or {}), "status": "running", "images": []}
                with _lock:
                    reply["parts"].append(part)
                _emit("tool", id=part["id"], name=part["name"], input=part["input"])
    elif kind == "user":
        for block in (ev.get("message") or {}).get("content") or []:
            if not isinstance(block, dict) or block.get("type") != "tool_result":
                continue
            part = next((p for p in reply["parts"] if p["kind"] == "tool" and p["id"] == block.get("tool_use_id")), None)
            if part is None:
                continue
            content = block.get("content") if isinstance(block.get("content"), list) else []
            images = [p for p in (_save_image(c) for c in content if isinstance(c, dict) and c.get("type") == "image") if p]
            status = "error" if block.get("is_error") else "done"
            with _lock:
                part["status"] = status
                part["images"] = images
            _emit("toolDone", id=part["id"], status=status, images=images)
    elif kind == "result":
        totals = ev.get("usage") or {}
        with _lock:
            STATE["session"] = ev.get("session_id") or STATE["session"]
            usage["input"] += _usage_of(totals)
            usage["output"] += int(totals.get("output_tokens") or 0)
            usage["costUsd"] = round(usage["costUsd"] + float(ev.get("total_cost_usd") or 0), 6)
            per = ev.get("modelUsage") or {}
            entry = per.get(usage["model"]) or max(per.values(), key=lambda m: int(m.get("contextWindow") or 0), default={})
            usage["window"] = int(entry.get("contextWindow") or usage["window"] or 0)
        if not any(p["kind"] == "text" and p["text"] for p in reply["parts"]) and isinstance(ev.get("result"), str) and not ev.get("is_error") and ev["result"]:
            _add_text(reply, ev["result"])
        return ev
    return None


def _project_dir() -> Path | None:
    root = Path(os.environ.get("CLAUDE_CONFIG_DIR") or Path.home() / ".claude").expanduser()
    folder = workspace.ensure()
    for candidate in (folder, folder.resolve()):
        found = root / "projects" / re.sub(r"[^A-Za-z0-9]", "-", str(candidate))
        if found.is_dir():
            return found
    return None


def _user_text(ev: dict) -> str:
    content = (ev.get("message") or {}).get("content")
    texts = [content] if isinstance(content, str) else [b.get("text", "") for b in content or [] if isinstance(b, dict) and b.get("type") == "text"]
    return CONTEXT.sub("", "\n\n".join(t for t in texts if t and not t.lstrip().startswith("<"))).strip()


def _events(path: Path):
    try:
        with path.open(encoding="utf-8", errors="replace") as f:
            for line in f:
                try:
                    ev = json.loads(line)
                except ValueError:
                    continue
                if isinstance(ev, dict) and ev.get("type") in ("user", "assistant") and not ev.get("isSidechain") and not ev.get("isMeta"):
                    yield ev
    except OSError:
        return


def _first_question(path: Path) -> str:
    for ev in _events(path):
        if ev["type"] == "user":
            text = _user_text(ev)
            if text:
                return text
    return ""


def _transcript(path: Path) -> tuple[list[dict], int, str]:
    out: list[dict] = []
    tools: dict[str, dict] = {}
    context, model = 0, ""
    for ev in _events(path):
        message = ev.get("message") or {}
        content = message.get("content")
        if ev["type"] == "user":
            text = _user_text(ev)
            if text:
                out.append({"role": "user", "text": text, "note": "from Claude Code"})
            for b in content if isinstance(content, list) else []:
                if isinstance(b, dict) and b.get("type") == "tool_result" and b.get("tool_use_id") in tools and b.get("is_error"):
                    tools[b["tool_use_id"]]["status"] = "error"
            continue
        if message.get("model") == "<synthetic>":
            continue
        parts: list[dict] = []
        for b in content if isinstance(content, list) else []:
            if not isinstance(b, dict):
                continue
            if b.get("type") == "text" and (b.get("text") or "").strip():
                parts.append({"kind": "text", "text": b["text"]})
            elif b.get("type") == "tool_use" and b.get("name") not in HIDDEN:
                part = {"kind": "tool", "id": b.get("id"), "name": _tool_name(str(b.get("name") or "tool")), "input": _brief(b.get("input") or {}), "status": "done", "images": []}
                tools[str(b.get("id"))] = part
                parts.append(part)
        if isinstance(message.get("usage"), dict):
            context = _usage_of(message["usage"]) or context
        model = str(message.get("model") or model)
        if not parts:
            continue
        if out and out[-1]["role"] == "assistant":
            out[-1]["parts"] += parts
        else:
            out.append({"role": "assistant", "parts": parts, "error": "", "meta": {}})
    return out[-MAX_MESSAGES:], context, model


def sessions() -> dict:
    folder = _project_dir()
    items: list[dict] = []
    if folder is not None:
        files = sorted(folder.glob("*.jsonl"), key=lambda p: p.stat().st_mtime, reverse=True)
        for f in files:
            title = _first_question(f)
            if title:
                items.append({"id": f.stem, "title": title[:90], "modified": int(f.stat().st_mtime), "current": f.stem == STATE["session"]})
            if len(items) >= 20:
                break
    return {"sessions": items}


def adopt(session: str) -> dict:
    folder = _project_dir()
    session = (session or "").strip()
    if not re.fullmatch(r"[A-Za-z0-9_-]+", session):
        raise ValueError("Pick a Claude Code conversation first.")
    path = folder / f"{session}.jsonl" if folder is not None else None
    if path is None or not path.is_file():
        raise LookupError("That Claude Code conversation was not found in formvfx's agent folder.")
    with _lock:
        if STATE["busy"]:
            raise RuntimeError("Claude is still answering. Wait for it, or press Stop.")
    messages, context, model = _transcript(path)
    with _lock:
        if STATE["busy"]:
            raise RuntimeError("Claude is still answering. Wait for it, or press Stop.")
        usage = _blank_usage()
        usage.update(context=context, model=model)
        STATE.update(session=session, messages=messages, usage=usage, run=STATE["run"] + 1)
    _emit("adopted", session=session, count=len(messages))
    return {"session": session, "messages": len(messages)}
