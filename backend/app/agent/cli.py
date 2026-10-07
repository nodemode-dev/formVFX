from __future__ import annotations

import argparse
import base64
import json
import sys
from pathlib import Path

from . import client


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


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv and argv[0] == "mcp":
        from . import stdio

        stdio.main()
        return 0
    ap = argparse.ArgumentParser(prog="formvfx", description="Drive the running formvfx from the command line.")
    sub = ap.add_subparsers(dest="cmd", required=True)
    e = sub.add_parser("exec", help="run one command")
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
    r = sub.add_parser("render", help="render the Render Queue")
    r.add_argument("--wait", action="store_true")
    a = ap.parse_args(argv)
    try:
        if a.cmd == "exec":
            _print(client.rpc("command.execute", {"command": a.id, "params": _load(a.params)}))
        elif a.cmd == "batch":
            steps = _load(a.steps)
            params = {"steps": steps} if isinstance(steps, list) else steps
            if a.label:
                params["label"] = a.label
            _print(client.rpc("command.batch", params))
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
            _print(client.rpc("command.execute", {"command": "renderQueue.render", "params": {"wait": bool(a.wait)}}))
    except RuntimeError as exc:
        print(f"[formvfx] {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
