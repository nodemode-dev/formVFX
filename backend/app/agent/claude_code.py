from __future__ import annotations

import json
import os
import shlex
import shutil
import subprocess
import time
from pathlib import Path

from . import consent, workspace

NOTE = (
    "You are running inside formvfx, the motion graphics and visual effects app. The formvfx MCP server is connected and "
    "is the way to read and change the open project; do not use Bash or the formvfx command line for that. Its tools are "
    "deferred: load them first with ToolSearch, query \"select:mcp__formvfx__status,mcp__formvfx__batch,"
    "mcp__formvfx__execute,mcp__formvfx__list_commands,mcp__formvfx__describe_command,mcp__formvfx__get_comp,"
    "mcp__formvfx__render_frame,mcp__formvfx__guide\" (others: \"+formvfx\"). Start with status, build with one batch so a "
    "multi-step edit undoes as one step, and look at the result with render_frame before saying an edit is done. Every "
    "change appears live in the person's window and is undoable."
)
CHAT_NOTE = (
    " You are answering in formvfx's Chat tab, which shows Markdown to people who may never have used a terminal: write short "
    "paragraphs, use **bold** for names of layers, effects and settings and a bulleted list when you changed several things, "
    "avoid tables and code blocks, say what you changed in a motion designer's words, and keep tool calls few by batching."
)
INSTALL = "Claude Code is not installed on this computer. Install it from https://claude.com/claude-code, run claude once in a terminal to sign in, then try again."


class NotInstalled(RuntimeError):
    pass



_KEEP = {"CLAUDE_CODE_OAUTH_TOKEN", "CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX", "CLAUDE_CONFIG_DIR"}
_cache: dict = {"at": 0.0, "info": None}


def _candidates() -> list[Path]:
    home = Path.home()
    names = ("claude.exe", "claude.cmd", "claude") if os.name == "nt" else ("claude",)
    dirs = [
        home / ".claude" / "local",
        home / ".local" / "bin",
        home / ".npm-global" / "bin",
        home / ".bun" / "bin",
        home / ".volta" / "bin",
        Path("/opt/homebrew/bin"),
        Path("/usr/local/bin"),
        Path("/usr/bin"),
    ]
    if os.name == "nt":
        appdata = os.environ.get("APPDATA")
        if appdata:
            dirs.insert(0, Path(appdata) / "npm")
    return [d / n for d in dirs for n in names]


def _login_shell_lookup() -> str | None:
    if os.name == "nt":
        return None
    shell = os.environ.get("SHELL") or "/bin/sh"
    try:
        out = subprocess.run([shell, "-l", "-c", "command -v claude"], capture_output=True, text=True, timeout=8)
    except (OSError, subprocess.SubprocessError):
        return None
    path = out.stdout.strip().splitlines()[-1] if out.stdout.strip() else ""
    return path if path and Path(path).is_file() else None


def cli() -> str | None:
    pref = os.environ.get("FORMVFX_CLAUDE_BIN", "").strip()
    if pref and Path(pref).expanduser().is_file():
        return str(Path(pref).expanduser())
    found = shutil.which("claude")
    if found:
        return found
    for c in _candidates():
        if c.is_file():
            return str(c)
    return _login_shell_lookup()


def info(fresh: bool = False) -> dict:
    now = time.monotonic()
    if not fresh and _cache["info"] is not None and now - _cache["at"] < 60:
        return _cache["info"]
    path = cli()
    version = None
    if path:
        try:
            out = subprocess.run([path, "--version"], capture_output=True, text=True, timeout=15)
            version = (out.stdout or out.stderr).strip().splitlines()[0] if (out.stdout or out.stderr).strip() else None
        except (OSError, subprocess.SubprocessError):
            version = None
    result = {"installed": bool(path), "path": path, "version": version, "folder": str(workspace.ensure())}
    _cache.update(at=now, info=result)
    return result


def mcp_config(base_url: str) -> str:
    return json.dumps({
        "mcpServers": {
            "formvfx": {
                "type": "http",
                "url": base_url.rstrip("/") + "/mcp",
                "headers": {"x-formvfx-agent-token": consent.TOKEN, "x-formvfx-source": "mcp:Claude Code in formvfx"},
            }
        }
    })


def child_env(base: dict) -> dict:
    env = {k: v for k, v in base.items() if not k.startswith("CLAUDE") or k in _KEEP}
    env.pop("FORMVFX_CALLER_DIR", None)
    return env


def _wrap(cmd: list[str]) -> list[str]:
    if os.name == "nt":
        return cmd
    shell = os.environ.get("SHELL") or "/bin/sh"
    return [shell, "-l", "-c", "exec " + shlex.join(cmd)]


def argv(base_url: str, prompt: str | None = None, resume: str | None = None) -> list[str]:
    path = cli()
    if not path:
        raise NotInstalled(INSTALL)
    cmd = [path, "--mcp-config", mcp_config(base_url), "--allowedTools", "mcp__formvfx", "ToolSearch", "--append-system-prompt", NOTE]
    if resume:
        cmd += ["--resume", resume]
    if prompt:
        cmd.append(prompt)
    return _wrap(cmd)


def headless(base_url: str, prompt: str, resume: str | None = None, note: str = NOTE) -> list[str]:
    path = cli()
    if not path:
        raise NotInstalled(INSTALL)
    cmd = [
        path, "-p", prompt, "--output-format", "stream-json", "--verbose", "--include-partial-messages",
        "--mcp-config", mcp_config(base_url), "--allowedTools", "mcp__formvfx", "ToolSearch", "--append-system-prompt", note,
    ]
    if resume:
        cmd += ["--resume", resume]
    return _wrap(cmd)
