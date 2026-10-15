import asyncio
import contextlib
import json
import os
import shutil
import signal
import struct
import threading

from fastapi import APIRouter, WebSocket, WebSocketDisconnect

from ..agent import claude_code, consent, workspace
from ..core import config

router = APIRouter(prefix="/api/terminal", tags=["terminal"])
POSIX = os.name == "posix"
AGENTS = {"claude": "claude", "codex": "codex", "cursor": "cursor-agent"}


def _environment(base_url: str) -> dict:
    ws = workspace.ensure()
    env = dict(os.environ)
    env["FORMVFX_URL"] = base_url
    env["FORMVFX_AGENT_TOKEN"] = consent.TOKEN
    env["FORMVFX_AGENT_NAME"] = "terminal"
    env["FORMVFX_WORKSPACE"] = str(ws)
    env["PATH"] = str(ws / "bin") + os.pathsep + env.get("PATH", "")
    env.setdefault("TERM", "xterm-256color")
    env["COLORTERM"] = "truecolor"
    env.pop("FORMVFX_DESKTOP", None)
    return env


def _shell() -> list[str]:
    if POSIX:
        shell = os.environ.get("SHELL") or ("/bin/zsh" if os.path.exists("/bin/zsh") else "/bin/bash")
        return [shell, "-l"] if os.path.basename(shell) in ("bash", "zsh") else [shell]
    return [os.environ.get("COMSPEC", "cmd.exe")]


class Session:
    def __init__(self, cols: int, rows: int, env: dict, cwd: str, argv: list[str] | None = None):
        self.cols, self.rows = max(20, min(500, cols)), max(5, min(300, rows))
        self.pid = None
        self.fd = None
        self.process = None
        self.winpty = None
        cmd = argv or _shell()
        if POSIX:
            import pty

            pid, fd = pty.fork()
            if pid == 0:
                try:
                    os.chdir(cwd)
                except OSError:
                    os.chdir(os.path.expanduser("~"))
                os.execvpe(cmd[0], cmd, env)  # noqa: S606 the shell is the feature
            self.pid, self.fd = pid, fd
            self.resize(self.cols, self.rows)
            return
        try:
            from winpty import PtyProcess

            self.winpty = PtyProcess.spawn(cmd, cwd=cwd, env=env, dimensions=(self.rows, self.cols))
        except Exception:
            import subprocess

            self.process = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=env, cwd=cwd)

    def resize(self, cols: int, rows: int) -> None:
        self.cols, self.rows = max(20, min(500, cols)), max(5, min(300, rows))
        if POSIX and self.fd is not None:
            import fcntl
            import termios

            fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", self.rows, self.cols, 0, 0))
        elif self.winpty is not None:
            with contextlib.suppress(Exception):
                self.winpty.setwinsize(self.rows, self.cols)

    def write(self, data: str) -> None:
        if POSIX and self.fd is not None:
            os.write(self.fd, data.encode("utf-8"))
        elif self.winpty is not None:
            self.winpty.write(data)
        elif self.process and self.process.stdin:
            self.process.stdin.write(data.replace("\r", "\r\n").encode("utf-8"))
            self.process.stdin.flush()

    def read(self) -> str:
        if POSIX and self.fd is not None:
            return os.read(self.fd, 65536).decode("utf-8", errors="replace")
        if self.winpty is not None:
            return self.winpty.read(65536)
        if self.process and self.process.stdout:
            data = self.process.stdout.read1(65536) if hasattr(self.process.stdout, "read1") else self.process.stdout.readline()
            return data.decode("utf-8", errors="replace")
        return ""

    def close(self) -> None:
        if POSIX and self.pid:
            with contextlib.suppress(OSError):
                os.kill(self.pid, signal.SIGHUP)
            with contextlib.suppress(OSError):
                os.close(self.fd)
            with contextlib.suppress(OSError):
                os.waitpid(self.pid, os.WNOHANG)
        elif self.winpty is not None:
            with contextlib.suppress(Exception):
                self.winpty.terminate(force=True)
        elif self.process:
            self.process.kill()


@router.get("/status")
def status() -> dict:
    ws = workspace.ensure()
    return {
        "available": True,
        "pty": POSIX,
        "shell": _shell()[0],
        "cwd": str(ws),
        "agents": {name: (claude_code.cli() is not None) if name == "claude" else shutil.which(binary) is not None for name, binary in AGENTS.items()},
        "launch": AGENTS,
        "claude": claude_code.info(),
    }


@router.websocket("/ws")
async def terminal(socket: WebSocket) -> None:
    await socket.accept()
    try:
        cols = int(socket.query_params.get("cols", "100"))
        rows = int(socket.query_params.get("rows", "30"))
    except ValueError:
        cols, rows = 100, 30
    host = socket.headers.get("host") or f"127.0.0.1:{config.PORT}"
    base = f"http://127.0.0.1:{host.rsplit(':', 1)[-1]}" if ":" in host else f"http://127.0.0.1:{config.PORT}"
    if socket.query_params.get("engine"):
        base = socket.query_params["engine"]
    env = _environment(base)
    cwd = socket.query_params.get("cwd") or env["FORMVFX_WORKSPACE"]
    argv = None
    if socket.query_params.get("agent") == "claude":
        try:
            argv = claude_code.argv(base, socket.query_params.get("prompt") or None, socket.query_params.get("resume") or None)
            env = claude_code.child_env(env)
        except RuntimeError as e:
            await socket.send_text(f"\r\n\x1b[33m{e}\x1b[0m\r\n")
    session = Session(cols, rows, env, cwd, argv)
    loop = asyncio.get_running_loop()
    queue: asyncio.Queue = asyncio.Queue()
    stop = threading.Event()

    def pump() -> None:
        while not stop.is_set():
            try:
                data = session.read()
            except (OSError, EOFError):
                break
            if not data:
                break
            loop.call_soon_threadsafe(queue.put_nowait, data)
        loop.call_soon_threadsafe(queue.put_nowait, None)

    reader = threading.Thread(target=pump, daemon=True)
    reader.start()

    async def send() -> None:
        while True:
            data = await queue.get()
            if data is None:
                with contextlib.suppress(Exception):
                    await socket.send_text(json.dumps({"exit": True}))
                break
            await socket.send_text(data)

    sender = asyncio.create_task(send())
    launch = socket.query_params.get("launch")
    if launch in AGENTS:
        session.write(AGENTS[launch] + "\r")
    try:
        while True:
            message = await socket.receive_text()
            if message.startswith("\x00{"):
                try:
                    control = json.loads(message[1:])
                except ValueError:
                    control = {}
                if isinstance(control.get("resize"), list) and len(control["resize"]) == 2:
                    session.resize(int(control["resize"][0]), int(control["resize"][1]))
                if control.get("launch") in AGENTS:
                    session.write(AGENTS[control["launch"]] + "\r")
                continue
            session.write(message)
    except WebSocketDisconnect:
        pass
    finally:
        stop.set()
        session.close()
        sender.cancel()
