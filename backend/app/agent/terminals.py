from __future__ import annotations

import asyncio
import atexit
import codecs
import contextlib
import json
import os
import queue
import select
import signal
import struct
import subprocess
import threading
import time
from collections import deque
from pathlib import Path

from ..core import config
from ..engine.host import host
from . import claude_code, consent, workspace

POSIX = os.name == "posix"
SCROLLBACK = 256 * 1024
AGENTS = {"claude": "claude", "codex": "codex", "cursor": "cursor-agent", "antigravity": "agy"}
TITLES = {"claude": "Claude Code", "codex": "Codex", "cursor": "Cursor", "antigravity": "Antigravity"}
TAB_FILE = (".agents", "mcp_config.json")
TAB_ROOT = ("agent-tabs", "antigravity")
TAB_GUIDE = "AGENTS.md"
TAB_HEADER = "x-formvfx-agent-token"


class NotInstalled(RuntimeError):
    pass


def login_shell() -> list[str]:
    if POSIX:
        shell = os.environ.get("SHELL") or ("/bin/zsh" if os.path.exists("/bin/zsh") else "/bin/bash")
        return [shell, "-l"] if os.path.basename(shell) in ("bash", "zsh") else [shell]
    return [os.environ.get("COMSPEC", "cmd.exe")]


def environment(base_url: str) -> dict:
    ws = workspace.ensure()
    env = dict(os.environ)
    env["FORMVFX_URL"] = base_url
    env.pop("FORMVFX_AGENT_TOKEN", None)
    env["FORMVFX_AGENT_NAME"] = "terminal"
    env["FORMVFX_WORKSPACE"] = str(ws)
    env["PATH"] = str(ws / "bin") + os.pathsep + env.get("PATH", "")
    env.setdefault("TERM", "xterm-256color")
    env["COLORTERM"] = "truecolor"
    env.pop("FORMVFX_DESKTOP", None)
    env.pop("FORMVFX_CALLER_DIR", None)
    return env


def _clamp(cols: int, rows: int) -> tuple[int, int]:
    return max(20, min(500, int(cols))), max(5, min(300, int(rows)))


class Session:
    def __init__(self, sid: str, title: str, kind: str, cwd: str, argv: list[str], env: dict, cols: int, rows: int) -> None:
        self.id = sid
        self.title = title
        self.kind = kind
        self.cwd = cwd
        self.argv = argv
        self.env = env
        self.cols, self.rows = _clamp(cols, rows)
        self.alive = False
        self.gen = 0
        self.seq = 0
        self.chunks: deque[str] = deque()
        self.size = 0
        self.lock = threading.Lock()
        self.subscribers: set[asyncio.Queue] = set()
        self.pid: int | None = None
        self.fd: int | None = None
        self.winpty = None
        self.process = None
        self.pump: threading.Thread | None = None
        self.inbox: queue.SimpleQueue = queue.SimpleQueue()
        self.on_close: list = []

    def describe(self) -> dict:
        return {"id": self.id, "title": self.title, "kind": self.kind, "alive": self.alive}

    def keep(self, text: str) -> int:
        with self.lock:
            self.seq += 1
            self.chunks.append(text)
            self.size += len(text)
            while self.size > SCROLLBACK and len(self.chunks) > 1:
                self.size -= len(self.chunks.popleft())
            return self.seq

    def snapshot(self) -> tuple[str, int]:
        with self.lock:
            return "".join(self.chunks), self.seq

    def attach(self, queue: asyncio.Queue) -> tuple[str, int]:
        with self.lock:
            self.subscribers.add(queue)
            return "".join(self.chunks), self.seq

    def detach(self, queue: asyncio.Queue) -> None:
        with self.lock:
            self.subscribers.discard(queue)

    def clear(self) -> int:
        with self.lock:
            self.chunks.clear()
            self.size = 0
            self.seq += 1
            return self.seq

    def read_text(self, limit: int = 4000) -> str:
        return self.snapshot()[0][-limit:]


SESSIONS: dict[str, Session] = {}
_loop: asyncio.AbstractEventLoop | None = None
_counter = {"n": 0}
_guard = threading.Lock()


def bind(loop: asyncio.AbstractEventLoop) -> None:
    global _loop
    _loop = loop


def _deliver(session: Session, item: tuple[int, str | None]) -> None:
    for q in list(session.subscribers):
        if q.qsize() < 4000:
            q.put_nowait(item)


def _notify(session: Session, seq: int, text: str | None) -> None:
    loop = _loop or host.bus.loop
    if loop is not None and not loop.is_closed():
        with contextlib.suppress(RuntimeError):
            loop.call_soon_threadsafe(_deliver, session, (seq, text))


def _spawn(session: Session) -> None:
    cwd = session.cwd if os.path.isdir(session.cwd) else os.path.expanduser("~")
    cmd = session.argv
    handle: dict = {"fd": None, "pid": None, "winpty": None, "process": None}
    if POSIX:
        import pty

        pid, fd = pty.fork()
        if pid == 0:
            try:
                os.chdir(cwd)
            except OSError:
                os.chdir(os.path.expanduser("~"))
            try:
                os.execvpe(cmd[0], cmd, session.env)  # noqa: S606 the shell is the feature
            except OSError:
                os._exit(127)
        handle.update(fd=fd, pid=pid)
        _winsize(fd, session.cols, session.rows)
    else:
        try:
            from winpty import PtyProcess

            handle["winpty"] = PtyProcess.spawn(cmd, cwd=cwd, env=session.env, dimensions=(session.rows, session.cols))
        except Exception:
            handle["process"] = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=session.env, cwd=cwd)
    session.pid, session.fd = handle["pid"], handle["fd"]
    session.winpty, session.process = handle["winpty"], handle["process"]
    session.alive = True
    gen = session.gen
    session.pump = threading.Thread(target=_pump, args=(session, handle, gen), name=f"term-{session.id}-{gen}", daemon=True)
    session.pump.start()
    session.inbox = queue.SimpleQueue()
    threading.Thread(target=_writer, args=(session, handle, gen, session.inbox), name=f"term-in-{session.id}-{gen}", daemon=True).start()
    if handle["pid"]:
        threading.Thread(target=_reap, args=(session, handle, gen), name=f"reap-{session.id}-{gen}", daemon=True).start()


def _winsize(fd: int, cols: int, rows: int) -> None:
    import fcntl
    import termios

    with contextlib.suppress(OSError):
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))


def _chunk(handle: dict) -> bytes | str:
    if handle["fd"] is not None:
        return os.read(handle["fd"], 65536)
    if handle["winpty"] is not None:
        return handle["winpty"].read(65536)
    out = handle["process"].stdout
    return out.read1(65536) if hasattr(out, "read1") else out.readline()


def _pump(session: Session, handle: dict, gen: int) -> None:
    decoder = codecs.getincrementaldecoder("utf-8")("replace")
    while True:
        try:
            data = _chunk(handle)
        except (OSError, EOFError, ValueError):
            break
        if not data:
            break
        text = decoder.decode(data) if isinstance(data, bytes) else data
        if not text:
            continue
        if session.gen != gen:
            break
        _notify(session, session.keep(text), text)
    if handle["fd"] is not None:
        with contextlib.suppress(OSError):
            os.close(handle["fd"])
    _ended(session, gen)


def _write_fd(session: Session, fd: int, data: bytes, gen: int) -> None:
    while data and session.gen == gen and session.alive:
        try:
            ready = select.select([], [fd], [], 0.5)[1]
        except ValueError:
            ready = [fd]
        if ready:
            data = data[os.write(fd, data[:512]) :]


def _writer(session: Session, handle: dict, gen: int, inbox: queue.SimpleQueue) -> None:
    while session.gen == gen and session.alive:
        try:
            data = inbox.get(timeout=1.0)
        except queue.Empty:
            continue
        if session.gen != gen:
            break
        try:
            if handle["fd"] is not None:
                _write_fd(session, handle["fd"], data.encode("utf-8"), gen)
            elif handle["winpty"] is not None:
                handle["winpty"].write(data)
            elif handle["process"] is not None and handle["process"].stdin:
                handle["process"].stdin.write(data.replace("\r", "\r\n").encode("utf-8"))
                handle["process"].stdin.flush()
        except (OSError, ValueError):
            pass


def _reap(session: Session, handle: dict, gen: int) -> None:
    with contextlib.suppress(OSError, ChildProcessError):
        os.waitpid(handle["pid"], 0)
    pump = session.pump
    if pump is not None and session.gen == gen:
        pump.join(1.0)
        _ended(session, gen)


def _ended(session: Session, gen: int) -> None:
    with session.lock:
        if session.gen != gen or not session.alive:
            return
        session.alive = False
    _notify(session, session.seq, None)


def _stop(session: Session) -> None:
    pid, wp, proc = session.pid, session.winpty, session.process
    session.pid = session.fd = session.winpty = session.process = None
    if POSIX and pid:
        for sig in (signal.SIGHUP, signal.SIGTERM):
            with contextlib.suppress(OSError):
                os.killpg(pid, sig)
            with contextlib.suppress(OSError):
                os.kill(pid, sig)

        def hard() -> None:
            time.sleep(1.5)
            try:
                if os.waitpid(pid, os.WNOHANG)[0] != 0:
                    return
            except ChildProcessError:
                return
            with contextlib.suppress(OSError):
                os.killpg(pid, signal.SIGKILL)
            with contextlib.suppress(OSError):
                os.kill(pid, signal.SIGKILL)

        threading.Thread(target=hard, daemon=True).start()
    elif wp is not None:
        with contextlib.suppress(Exception):
            wp.terminate(force=True)
    elif proc is not None:
        with contextlib.suppress(Exception):
            proc.kill()


def _title(kind: str, title: str | None) -> str:
    if title:
        return title
    if kind == "shell":
        with _guard:
            _counter["n"] += 1
            return f"Terminal {_counter['n']}"
    return TITLES.get(kind, kind.title())


def open_session(kind: str, argv: list[str], env: dict, cols: int = 100, rows: int = 30, title: str | None = None, cwd: str | None = None) -> Session:
    session = Session(os.urandom(6).hex(), _title(kind, title), kind, cwd or str(workspace.ensure()), argv, env, cols, rows)
    _spawn(session)
    SESSIONS[session.id] = session
    return session


def get(sid: str) -> Session:
    session = SESSIONS.get(sid)
    if session is None:
        raise KeyError(sid)
    return session


def write(session: Session, data: str) -> None:
    if session.alive and data:
        session.inbox.put(data)


def resize(session: Session, cols: int, rows: int) -> None:
    session.cols, session.rows = _clamp(cols, rows)
    if POSIX and session.fd is not None:
        _winsize(session.fd, session.cols, session.rows)
    elif session.winpty is not None:
        with contextlib.suppress(Exception):
            session.winpty.setwinsize(session.rows, session.cols)


def restart(session: Session) -> None:
    with session.lock:
        session.gen += 1
        session.alive = False
    _stop(session)
    _notify(session, session.clear(), "\x1bc")
    _spawn(session)


def close(sid: str) -> bool:
    session = SESSIONS.pop(sid, None)
    if session is None:
        return False
    with session.lock:
        session.gen += 1
        session.alive = False
    _stop(session)
    for done in session.on_close:
        with contextlib.suppress(Exception):
            done()
    _notify(session, session.seq, None)
    return True


def close_all() -> None:
    for sid in list(SESSIONS):
        close(sid)


def listing() -> list[dict]:
    return [s.describe() for s in SESSIONS.values()]


def open_shell(base_url: str, cols: int, rows: int) -> Session:
    return open_session("shell", login_shell(), environment(base_url), cols, rows)


def feed(session: Session, text: str) -> None:
    write(session, text)
    threading.Timer(0.2, write, args=(session, "\r")).start()


def open_claude(base_url: str, prompt: str | None, resume: str | None, cols: int, rows: int) -> Session:
    prompt = (prompt or "").strip() or None
    resume = (resume or "").strip() or None
    if prompt and not resume:
        running = next((s for s in SESSIONS.values() if s.kind == "claude" and s.alive), None)
        if running is not None:
            feed(running, prompt)
            return running
    agent, mcp_file = claude_code.issue(base_url)
    try:
        argv = claude_code.argv(mcp_file, prompt, resume)
        env = claude_code.child_env(environment(base_url), claude_code.cli())
        session = open_session("claude", argv, env, cols, rows, "Claude Code (from Chat)" if resume else None)
    except claude_code.NotInstalled as e:
        claude_code.revoke(agent)
        raise NotInstalled(str(e)) from e
    except BaseException:
        claude_code.revoke(agent)
        raise
    session.on_close.append(lambda: claude_code.revoke(agent))
    return session


def find_agent(binary: str) -> str | None:
    kind = next((k for k, name in AGENTS.items() if name == binary), None)
    return claude_code.located(kind).path if kind else None


def tab_folder(guide: Path) -> Path:
    folder = config.data_dir().joinpath(*TAB_ROOT, os.urandom(6).hex())
    folder.mkdir(parents=True)
    with contextlib.suppress(OSError):
        (folder / TAB_GUIDE).write_bytes(guide.read_bytes())
    return folder


def drop_tab_folder(folder: Path, token: str, guide: Path) -> None:
    remove_tab_entry(folder, token)
    path = folder.joinpath(*TAB_FILE)
    with contextlib.suppress(OSError, ValueError):
        if _tab_servers(path) == {"mcpServers": {}}:
            path.unlink()
    with contextlib.suppress(OSError):
        if (folder / TAB_GUIDE).read_bytes() == guide.read_bytes():
            (folder / TAB_GUIDE).unlink()
    for emptied in (path.parent, folder):
        with contextlib.suppress(OSError):
            emptied.rmdir()


def _tab_servers(path: Path) -> dict:
    try:
        data = json.loads(path.read_text(encoding="utf-8") or "{}") if path.exists() else {}
    except ValueError:
        data = None
    if not isinstance(data, dict) or not isinstance(data.get("mcpServers", {}), dict):
        raise OSError(f"{path} is not an MCP servers file formvfx can read, so it was left alone")
    return data


def _tab_replace(path: Path, data: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    part = path.with_name(path.name + f".part-{os.getpid()}")
    with os.fdopen(os.open(part, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600), "w", encoding="utf-8") as f:
        f.write(json.dumps(data, indent=2) + "\n")
    os.replace(part, path)


def write_tab_entry(folder: Path, base_url: str, token: str) -> None:
    path = folder.joinpath(*TAB_FILE)
    data = _tab_servers(path)
    entry = {"serverUrl": base_url.rstrip("/") + "/mcp", "headers": {TAB_HEADER: token, "x-formvfx-source": "mcp:Antigravity in formvfx"}}
    data.setdefault("mcpServers", {})[claude_code.TAB_ENTRY] = entry
    _tab_replace(path, data)


def remove_tab_entry(folder: Path, token: str) -> None:
    path = folder.joinpath(*TAB_FILE)
    try:
        data = _tab_servers(path)
        entry = data.get("mcpServers", {}).get(claude_code.TAB_ENTRY)
        if not isinstance(entry, dict) or (entry.get("headers") or {}).get(TAB_HEADER) != token:
            return
        del data["mcpServers"][claude_code.TAB_ENTRY]
        _tab_replace(path, data)
    except (OSError, ValueError):
        return


def open_agent(kind: str, base_url: str, cols: int, rows: int) -> Session:
    binary = AGENTS.get(kind)
    if kind == "claude" or binary is None:
        raise ValueError(f"No agent called {kind}")
    try:
        path = claude_code.usable(kind)
    except claude_code.NotInstalled as e:
        raise NotInstalled(str(e)) from e
    argv = claude_code._wrap([path])
    agent = consent.issue()
    env = claude_code.child_env(environment(base_url), path)
    env["FORMVFX_AGENT_TOKEN"] = agent
    guide = workspace.root() / TAB_GUIDE
    folder = None
    try:
        if kind == "antigravity":
            folder = tab_folder(guide)
            write_tab_entry(folder, base_url, agent)
        session = open_session(kind, argv, env, cols, rows, cwd=str(folder) if folder else None)
    except BaseException:
        consent.revoke(agent)
        if folder is not None:
            drop_tab_folder(folder, agent, guide)
        raise
    session.on_close.append(lambda: consent.revoke(agent))
    if folder is not None:
        session.on_close.append(lambda: drop_tab_folder(folder, agent, guide))
    return session


atexit.register(close_all)
