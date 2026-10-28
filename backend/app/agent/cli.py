from __future__ import annotations

import argparse
import base64
import json
import os
import sys
import time
from pathlib import Path

from . import client

RENDER = "renderQueue.render"
PATH_KEYS = ("output", "path")
POLL_SECONDS = 1.0
SETTLE_SECONDS = 5.0


def _load(raw: str | None) -> object:
    if raw is None:
        return {}
    p = Path(raw)
    text = p.read_text(encoding="utf-8") if not raw.lstrip().startswith(("{", "[")) and p.is_file() else raw
    try:
        return json.loads(text)
    except ValueError as exc:
        raise SystemExit(f"[formvfx] not JSON: {exc}") from None


def _print(value: object) -> None:
    print(json.dumps(value, indent=2, ensure_ascii=False))


def _ok(value: object) -> object:
    return {"ok": True, **value} if isinstance(value, dict) and "ok" not in value else value


def absolute_outputs(command: str, params: object) -> object:
    if not isinstance(params, dict) or not command.startswith(("renderQueue.", "render.")):
        return params
    here = Path(os.environ.get("FORMVFX_CALLER_DIR") or os.getcwd())
    return {k: str(here / v) if k in PATH_KEYS and isinstance(v, str) and v and not Path(v).is_absolute() else v for k, v in params.items()}


def _steps(steps: object) -> object:
    if not isinstance(steps, list):
        return steps
    return [{**s, "params": absolute_outputs(str(s.get("command") or ""), s.get("params"))} if isinstance(s, dict) else s for s in steps]


def _progress(queue: dict, last: str) -> str:
    row = next((r for r in queue.get("items") or [] if r.get("status") == "Rendering"), None)
    line = f"rendering {row.get('compName')}: {round(float(row.get('progress') or 0) * 100)}%" if row else ""
    if line and line != last:
        print(f"[formvfx] {line}", file=sys.stderr, flush=True)
    return line or last


def wait_for_render(ids: list) -> dict:
    last, idle = "", None
    while True:
        queue = client.rpc("render.status", timeout=60) or {}
        done = {r.get("item"): r for r in queue.get("finished") or [] if r.get("item") in ids}
        if len(done) == len(ids) or (idle is not None and time.monotonic() - idle > SETTLE_SECONDS):
            results = [done[i] for i in ids if i in done]
            return {"ok": len(results) == len(ids) and all(r.get("ok") for r in results), "rendered": ids, "results": results}
        if not queue.get("rendering") and idle is None:
            idle = time.monotonic()
        last = _progress(queue, last)
        time.sleep(POLL_SECONDS)


def render(params: dict, wait: bool) -> dict:
    started = client.rpc("command.execute", {"command": RENDER, "params": {**params, "wait": False}}) or {}
    if not wait:
        return _ok(started)
    try:
        return wait_for_render(started.get("started") or [])
    except KeyboardInterrupt:
        raise SystemExit("[formvfx] stopped waiting; the render goes on in the engine: ./formvfx render --status, ./formvfx render --stop") from None


def _render_cmd(a: argparse.Namespace) -> dict:
    if a.stop:
        return _ok(client.rpc("command.execute", {"command": "renderQueue.stop", "params": {}}))
    if a.status:
        return _ok(client.rpc("render.status"))
    return render({}, bool(a.wait))


def _exec(command: str, params: object) -> object:
    params = absolute_outputs(command, params)
    if command == RENDER and isinstance(params, dict) and params.get("wait", True) is not False:
        return render(params, True)
    return _ok(client.rpc("command.execute", {"command": command, "params": params}))


def _fail(message: str, as_json: bool) -> int:
    print(f"[formvfx] {message}", file=sys.stderr)
    if as_json:
        _print({"ok": False, "error": message})
    return 1


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv and argv[0] == "mcp":
        from . import stdio

        stdio.main()
        return 0
    ap = argparse.ArgumentParser(prog="formvfx", description="Drive the running formvfx from the command line. exec, batch and render print JSON with \"ok\": true on success and exit 0; on failure they print {\"ok\": false, \"error\": message} and exit 1. A field named error inside a successful result belongs to that command (prop.setExpression reports the expression's own error there, null when it is fine).")
    sub = ap.add_subparsers(dest="cmd", required=True)
    e = sub.add_parser("exec", help="run one command (renderQueue.render waits by polling unless wait is false; relative output paths resolve from the current folder)")
    e.add_argument("id")
    e.add_argument("params", nargs="?")
    b = sub.add_parser("batch", help="run several commands as one undo step")
    b.add_argument("steps", help="JSON list of {command, params}, or a file")
    b.add_argument("--label")
    c = sub.add_parser("commands", help="list commands")
    c.add_argument("filter", nargs="?")
    d = sub.add_parser("describe", help="one command with its schema")
    d.add_argument("id")
    sub.add_parser("state", help="project, active comp, selection, time")
    f = sub.add_parser("frame", help="save a rendered frame as PNG")
    f.add_argument("out")
    f.add_argument("--time", type=float)
    f.add_argument("--comp")
    f.add_argument("--max", type=int, default=1920)
    r = sub.add_parser("render", help="render the Render Queue in the background (--wait polls its progress until it is done)")
    r.add_argument("--wait", action="store_true")
    r.add_argument("--status", action="store_true", help="progress and finished results")
    r.add_argument("--stop", action="store_true", help="stop the running render")
    a = ap.parse_args(argv)
    as_json = a.cmd in ("exec", "batch", "render")
    try:
        if a.cmd == "exec":
            out = _exec(a.id, _load(a.params))
            _print(out)
            return 0 if not isinstance(out, dict) or out.get("ok") is not False else 1
        elif a.cmd == "batch":
            steps = _steps(_load(a.steps))
            params = {"steps": steps} if isinstance(steps, list) else steps
            if a.label:
                params["label"] = a.label
            _print(_ok(client.rpc("command.batch", params)))
        elif a.cmd == "commands":
            rows = client.rpc("command.list", {"filter": a.filter} if a.filter else {})
            for row in rows:
                print(f"{row['id']:<36} {row['label']:<40} {row.get('params') or ''}")
            print(f"{len(rows)} commands", file=sys.stderr)
        elif a.cmd == "describe":
            _print(client.rpc("command.describe", {"command": a.id}))
        elif a.cmd == "state":
            _print(client.rpc("command.execute", {"command": "project.summary", "params": {}}))
        elif a.cmd == "frame":
            params = {"maxSide": a.max, "display": False}
            if a.time is not None:
                params["time"] = a.time
            if a.comp:
                params["comp"] = int(a.comp) if a.comp.isdigit() else a.comp
            out = client.rpc("frame.png", params)
            Path(a.out).write_bytes(base64.b64decode(out["png"]))
            print(f"{a.out} · {out['width']} x {out['height']}")
        elif a.cmd == "render":
            out = _render_cmd(a)
            _print(out)
            return 0 if out.get("ok") is not False else 1
    except RuntimeError as exc:
        return _fail(str(exc), as_json)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
