from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request

from . import client

_name = {"value": os.environ.get("FORMVFX_AGENT_NAME", "")}


def _url() -> str | None:
    explicit = os.environ.get("FORMVFX_MCP_URL", "").strip()
    if explicit:
        return explicit
    base = client.engine_url()
    return f"{base}/mcp" if base else None


def forward(line: str) -> str | None:
    try:
        msg = json.loads(line)
    except ValueError:
        return json.dumps({"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "Parse error"}})
    params = msg.get("params") or {}
    if msg.get("method") == "initialize" and not _name["value"]:
        _name["value"] = "mcp:" + str((params.get("clientInfo") or {}).get("name") or "agent")
    version = (params.get("_meta") or {}).get("io.modelcontextprotocol/protocolVersion") or params.get("protocolVersion") or "2025-11-25"
    url = _url()
    if not url:
        if msg.get("id") is None:
            return None
        message = "The formvfx engine is not running and could not be started: run ./formvfx boot in the formvfx folder (or open the desktop app), then retry."
        if client.problem() != client.NOT_RUNNING:
            message = f"The formvfx engine cannot be reached: {client.problem()}."
        return json.dumps({"jsonrpc": "2.0", "id": msg.get("id"), "error": {"code": -32000, "message": message}})
    h = client.headers(_name["value"] or "mcp:agent")
    h["MCP-Protocol-Version"] = version
    req = urllib.request.Request(url, data=line.encode(), method="POST", headers=h)
    try:
        with urllib.request.urlopen(req, timeout=900) as r:
            body = r.read().decode()
    except urllib.error.HTTPError as exc:
        body = exc.read().decode() or ""
    except (urllib.error.URLError, OSError) as exc:
        print(f"[formvfx mcp] the engine is not answering at {url}: {exc}", file=sys.stderr, flush=True)
        if msg.get("id") is None:
            return None
        return json.dumps({"jsonrpc": "2.0", "id": msg.get("id"), "error": {"code": -32000, "message": f"The formvfx engine stopped answering ({exc}). Start it with ./formvfx boot and retry."}})
    if msg.get("id") is None:
        return None
    return body.strip() or None


def main() -> None:
    for line in sys.stdin:
        if not line.strip():
            continue
        out = forward(line)
        if out:
            sys.stdout.write(out + "\n")
            sys.stdout.flush()


if __name__ == "__main__":
    main()
