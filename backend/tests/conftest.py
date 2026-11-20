import importlib
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

import pytest

BACKEND = Path(__file__).resolve().parents[1]
REPO = BACKEND.parent

APP_ID = "formvfx"
APP_MODULE = "app.main:app"
ENV_PREFIX = "FORMVFX"
DATA_ENV = "FORMVFX_DATA_DIR"
PORT_ENV = "FORMVFX_PORT"
POLICY_ENV = "FORMVFX_AGENTS"
ENGINE_PORT = 8250
HEALTH_PATH = "/api/health"
VERSION_PATH = "/api/version"
VERSION_KEY = "version"
LOCAL_HEADER = {"x-formvfx-local": "1"}
WINDOW_ORIGINS = ["http://localhost:5250", "http://127.0.0.1:5250", "http://localhost:5251", "tauri://localhost", "http://tauri.localhost"]
WS_PATH = "/api/live"
MCP_PATH = "/mcp"
STDIO_CMD = [sys.executable, str(REPO / "setup" / "mcp_stdio.py")]
READ_TOOL = ("status", {})
CHANGING_TOOL = ("execute", {"command": "comp.new", "params": {"name": "Family"}})
TOKEN_HEADER = "x-formvfx-agent-token"
VERSION_SITES = [
    ("README.md", r"^Version (\S+) ·"),
    ("backend/app/__init__.py", r'^__version__ = "([^"]+)"'),
    ("frontend/package.json", r'^  "version": "([^"]+)"'),
    ("frontend/package-lock.json", r'^  "version": "([^"]+)"'),
    ("frontend/src/version.ts", r"^export const VERSION = '([^']+)'"),
    ("native/fvx-core/Cargo.toml", r'^version = "([^"]+)"'),
    ("native/fvx-core/Cargo.lock", r'name = "fvx-core"\nversion = "([^"]+)"'),
    ("native/fvx-core/pyproject.toml", r'^version = "([^"]+)"'),
    ("src-tauri/Cargo.toml", r'^version = "([^"]+)"'),
    ("src-tauri/Cargo.lock", r'name = "formvfx"\nversion = "([^"]+)"'),
    ("src-tauri/tauri.conf.json", r'^  "version": "([^"]+)"'),
]
STRIP_EXEMPT = []


def tool_catalogue():
    return importlib.import_module("app.agent.tools").TOOLS


def consent_refusals():
    return {
        "no_window": "No formvfx window is open to ask the person for permission",
        "deny": "Agents are turned off in this formvfx",
    }


def set_policy(mode):
    os.environ[POLICY_ENV] = mode


def inapp_token():
    return importlib.import_module("app.agent.consent").issue()


_SANDBOX = Path(tempfile.mkdtemp(prefix=f"{APP_ID}-tests-"))
for _name in ("data", "home"):
    (_SANDBOX / _name).mkdir()
os.environ[DATA_ENV] = str(_SANDBOX / "data")
os.environ[PORT_ENV] = str(ENGINE_PORT)
os.environ["HOME"] = str(_SANDBOX / "home")
os.environ["USERPROFILE"] = str(_SANDBOX / "home")
for _key in [k for k in os.environ if k.startswith(f"{ENV_PREFIX}_") and k not in (DATA_ENV, PORT_ENV)]:
    os.environ.pop(_key)
AGENT_STAND_INS = ("claude", "codex", "cursor-agent", "agent", "agy")
AGENT_CALLS = _SANDBOX / "agent-calls.log"
AGENT_SHELL = """#!/bin/sh
login=0; inter=0; script=""
while [ $# -gt 0 ]; do
  case "$1" in -l) login=1 ;; -i) inter=1 ;; -c) shift; script="$1" ;; esac
  shift
done
if [ "$login" = 1 ] && [ -f "$HOME/.zprofile-path" ]; then PATH="$(cat "$HOME/.zprofile-path"):$PATH"; fi
if [ "$inter" = 1 ] && [ -f "$HOME/.zshrc-path" ]; then PATH="$(cat "$HOME/.zshrc-path"):$PATH"; fi
export PATH
if [ -n "$script" ]; then exec /bin/sh -c "$script"; fi
exec /bin/sh
"""
_agent_calls: list[str] = []
if os.name != "nt":
    for _name in ("bin", "shell"):
        (_SANDBOX / _name).mkdir()
    for _name in AGENT_STAND_INS:
        (_SANDBOX / "bin" / _name).write_text(f'#!/bin/sh\necho "{_name} $*" >> "{AGENT_CALLS}"\nif [ "$1" = "--version" ]; then echo "0.0.0 (stand-in)"; exit 0; fi\nexit 1\n')
        (_SANDBOX / "bin" / _name).chmod(0o755)
    (_SANDBOX / "shell" / "zsh").write_text(AGENT_SHELL)
    (_SANDBOX / "shell" / "zsh").chmod(0o755)
    os.environ["SHELL"] = str(_SANDBOX / "shell" / "zsh")
    _kept = [d for d in os.environ.get("PATH", "").split(os.pathsep) if d and not any((Path(d) / n).exists() for n in AGENT_STAND_INS)]
    os.environ["PATH"] = os.pathsep.join([str(_SANDBOX / "bin"), *_kept])
    for _key in ("NPM_CONFIG_PREFIX", "npm_config_prefix", "PNPM_HOME", "VOLTA_HOME", "BUN_INSTALL", "NVM_SYMLINK"):
        os.environ.pop(_key, None)
sys.path.insert(0, str(BACKEND))


@pytest.fixture(autouse=True)
def only_stand_in_agent_programs(monkeypatch):
    agent_lookup = importlib.import_module("app.agent.agent_lookup")
    monkeypatch.setattr(agent_lookup, "system_folders", lambda platform=None: [])


def pytest_sessionfinish(session, exitstatus):
    try:
        _agent_calls.extend(AGENT_CALLS.read_text(encoding="utf-8").splitlines())
    except OSError:
        pass
    shutil.rmtree(_SANDBOX, ignore_errors=True)


def pytest_terminal_summary(terminalreporter):
    terminalreporter.write_line(f"agent program stand-ins started {len(_agent_calls)} time(s): " + (", ".join(sorted(set(_agent_calls))) or "none"))


def own_origin():
    return f"http://127.0.0.1:{ENGINE_PORT}"


@pytest.fixture(scope="session")
def sandbox():
    return _SANDBOX


@pytest.fixture(scope="session")
def app():
    module, _, attr = APP_MODULE.partition(":")
    return getattr(importlib.import_module(module), attr)


@pytest.fixture(scope="session")
def client(app):
    from fastapi.testclient import TestClient

    with TestClient(app, base_url=own_origin()) as c:
        yield c


@pytest.fixture(scope="session")
def repo_versions():
    found = {}
    for rel, pattern in VERSION_SITES:
        text = (REPO / rel).read_text(encoding="utf-8")
        match = re.search(pattern, text, re.MULTILINE)
        found[rel] = match.group(1) if match else None
    return found


@pytest.fixture(scope="session")
def repo_version(repo_versions):
    return next(iter(repo_versions.values()))


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def get_json(url, timeout=2.0):
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return r.status, json.loads(r.read().decode("utf-8") or "null")


@pytest.fixture(scope="session")
def engine():
    port = free_port()
    data = _SANDBOX / "engine-data"
    data.mkdir()
    env = dict(os.environ, **{DATA_ENV: str(data), PORT_ENV: str(port)})
    proc = subprocess.Popen(
        [sys.executable, "-m", "uvicorn", APP_MODULE, "--host", "127.0.0.1", "--port", str(port)],
        cwd=BACKEND, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
    )
    base = f"http://127.0.0.1:{port}"
    deadline = time.monotonic() + 90
    ready = False
    while time.monotonic() < deadline and proc.poll() is None:
        try:
            status, _ = get_json(base + HEALTH_PATH)
            ready = status == 200
        except OSError:
            ready = False
        if ready:
            break
        time.sleep(0.25)
    if not ready:
        proc.kill()
        out = proc.communicate(timeout=5)[0].decode("utf-8", "replace")
        raise RuntimeError(f"engine did not answer {HEALTH_PATH} within 90 s:\n{out[-4000:]}")
    yield {"base": base, "port": port, "data": data, "env": env, "proc": proc}
    proc.terminate()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
