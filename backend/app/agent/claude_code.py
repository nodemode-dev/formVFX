from __future__ import annotations

import json
import os
import secrets
import time
from pathlib import Path

from ..core import config
from . import agent_lookup, consent, workspace

TAB_ENTRY = "formvfx_tab"
NOTE = (
    "You are running inside formvfx, the motion graphics and visual effects app. The formvfx_tab MCP server is connected and "
    "is the way to read and change the open project; do not use Bash or the formvfx command line for that. Its tools are "
    "deferred: load them first with ToolSearch, query \"select:mcp__formvfx_tab__status,mcp__formvfx_tab__batch,"
    "mcp__formvfx_tab__execute,mcp__formvfx_tab__list_commands,mcp__formvfx_tab__describe_command,mcp__formvfx_tab__get_comp,"
    "mcp__formvfx_tab__render_frame,mcp__formvfx_tab__guide\" (others: \"+formvfx_tab\"). Start with status, build with one batch so a "
    "multi-step edit undoes as one step, and look at the result with render_frame before saying an edit is done. Every "
    "change appears live in the person's window and is undoable."
)
CHAT_NOTE = (
    " You are answering in formvfx's Chat tab, which shows Markdown to people who may never have used a terminal: write short "
    "paragraphs, use **bold** for names of layers, effects and settings and a bulleted list when you changed several things, "
    "avoid tables and code blocks, say what you changed in a motion designer's words, and keep tool calls few by batching."
)
INSTALL = "Claude Code is not installed on this computer. Install it from https://claude.com/claude-code, run claude once in a terminal to sign in, then try again."


OTHERWISE = ""
OVERRIDES = {"claude": ("FORMVFX_CLAUDE_BIN",), "codex": ("FORMVFX_CODEX_BIN",), "cursor": ("FORMVFX_CURSOR_BIN",), "antigravity": ("FORMVFX_ANTIGRAVITY_BIN",)}


class NotInstalled(RuntimeError):
    pass


_cache: dict = {"at": 0.0, "info": None}
_FILES: dict[str, Path] = {}


def setting(kind: str) -> str | None:
    return None


def located(kind: str, fresh: bool = False) -> agent_lookup.Found:
    found = agent_lookup.find(kind, setting=setting(kind), override_vars=OVERRIDES.get(kind, ()), shell_timeout=10.0, fresh=fresh)
    return agent_lookup.check(found, child_env(dict(os.environ), found.path), 15.0, fresh)


def said(found: agent_lookup.Found) -> str:
    return agent_lookup.sentence(found, agent_lookup.install(found.program), OTHERWISE)


def usable(kind: str) -> str:
    found = located(kind)
    if found.state != "runs":
        found = located(kind, fresh=True)
    if found.state != "runs" or not found.path:
        raise NotInstalled(said(found))
    return found.path


def cli() -> str | None:
    return located("claude").path


def info(fresh: bool = False) -> dict:
    now = time.monotonic()
    if not fresh and _cache["info"] is not None and now - _cache["at"] < 60:
        return _cache["info"]
    found = located("claude", fresh)
    result = agent_lookup.answer(found, agent_lookup.install("claude"), OTHERWISE) | {"folder": str(workspace.ensure())}
    _cache.update(at=now, info=result)
    return result


def mcp_config(base_url: str, token: str) -> str:
    return json.dumps({
        "mcpServers": {
            TAB_ENTRY: {
                "type": "http",
                "url": base_url.rstrip("/") + "/mcp",
                "headers": {"x-formvfx-agent-token": token, "x-formvfx-source": "mcp:Claude Code in formvfx"},
            }
        }
    })


def issue(base_url: str) -> tuple[str, str]:
    token = consent.issue()
    try:
        folder = config.data_dir() / "AgentTokens"
        folder.mkdir(mode=0o700, parents=True, exist_ok=True)
        path = folder / f"mcp-{secrets.token_hex(8)}.json"
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            f.write(mcp_config(base_url, token))
    except BaseException:
        consent.revoke(token)
        raise
    _FILES[token] = path
    return token, str(path)


def revoke(token: str) -> None:
    consent.revoke(token)
    path = _FILES.pop(token, None)
    if path is not None:
        try:
            path.unlink()
        except OSError:
            pass


def child_env(base: dict, path: str | None = None) -> dict:
    env = agent_lookup.child_env(path, base=base, front=[workspace.root() / "bin"], shell_timeout=10.0)
    env.pop("FORMVFX_CALLER_DIR", None)
    return env


def _wrap(cmd: list[str]) -> list[str]:
    return agent_lookup.start_argv(cmd)


def argv(mcp_file: str, prompt: str | None = None, resume: str | None = None) -> list[str]:
    path = usable("claude")
    cmd = [path, "--mcp-config", mcp_file, "--strict-mcp-config", "--allowedTools", "mcp__" + TAB_ENTRY, "ToolSearch", "--append-system-prompt", NOTE]
    if resume:
        cmd += ["--resume", resume]
    if prompt:
        cmd.append(prompt)
    return _wrap(cmd)


def headless(mcp_file: str, prompt: str, resume: str | None = None, note: str = NOTE) -> list[str]:
    path = usable("claude")
    cmd = [
        path, "-p", prompt, "--output-format", "stream-json", "--verbose", "--include-partial-messages",
        "--mcp-config", mcp_file, "--strict-mcp-config", "--allowedTools", "mcp__" + TAB_ENTRY, "ToolSearch", "--append-system-prompt", note,
    ]
    if resume:
        cmd += ["--resume", resume]
    return _wrap(cmd)
