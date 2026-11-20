import asyncio
import contextlib
import json

from fastapi import APIRouter, HTTPException, Request, WebSocket, WebSocketDisconnect

from ..agent import claude_code, programs, terminals, workspace
from ..core import window_key

router = APIRouter(prefix="/api/terminal", tags=["terminal"])
AGENTS = terminals.AGENTS


async def _off(fn, *args):
    return await asyncio.get_running_loop().run_in_executor(None, fn, *args)


def _base(request: Request) -> str:
    return f"http://127.0.0.1:{request.url.port or 80}"


async def _body(request: Request) -> dict:
    try:
        body = await request.json()
    except ValueError:
        return {}
    return body if isinstance(body, dict) else {}


def _size(body: dict) -> tuple[int, int]:
    try:
        return int(body.get("cols") or 100), int(body.get("rows") or 30)
    except (TypeError, ValueError):
        return 100, 30


def _session(sid: object) -> terminals.Session:
    try:
        return terminals.get(str(sid or ""))
    except KeyError:
        raise HTTPException(404, "No such terminal.") from None


@router.get("/status")
def status() -> dict:
    ws = workspace.ensure()
    return {
        "available": True,
        "pty": terminals.POSIX,
        "shell": terminals.login_shell()[0],
        "cwd": str(ws),
        "agents": {name: programs.find(name) is not None for name in AGENTS},
        "launch": AGENTS,
        "claude": claude_code.info(),
    }


@router.get("/agents")
def agents(fresh: bool = False) -> dict:
    return programs.status(fresh=fresh)


@router.get("/list")
def listing() -> dict:
    return {"terminals": terminals.listing()}


@router.post("/open")
async def open_shell(request: Request) -> dict:
    cols, rows = _size(await _body(request))
    session = await _off(terminals.open_shell, _base(request), cols, rows)
    return session.describe()


@router.post("/claude")
async def open_claude(request: Request) -> dict:
    body = await _body(request)
    cols, rows = _size(body)
    try:
        session = await _off(terminals.open_claude, _base(request), str(body.get("prompt") or ""), str(body.get("resume") or ""), cols, rows)
        return session.describe()
    except terminals.NotInstalled as e:
        raise HTTPException(400, str(e)) from e


@router.post("/launch")
async def launch(request: Request) -> dict:
    body = await _body(request)
    cols, rows = _size(body)
    try:
        session = await _off(terminals.open_agent, str(body.get("agent") or ""), _base(request), cols, rows)
        return session.describe()
    except terminals.NotInstalled as e:
        raise HTTPException(400, str(e)) from e
    except ValueError as e:
        raise HTTPException(400, str(e)) from e


@router.post("/restart")
async def restart(request: Request) -> dict:
    session = _session((await _body(request)).get("id"))
    await _off(terminals.restart, session)
    return {"ok": True}


@router.post("/close")
async def close(request: Request) -> dict:
    session = _session((await _body(request)).get("id"))
    await _off(terminals.close, session.id)
    return {"ok": True}


@router.websocket("/{sid}/ws")
async def attach(socket: WebSocket, sid: str) -> None:
    await socket.accept()
    try:
        first = json.loads(await asyncio.wait_for(socket.receive_text(), 10))
    except (asyncio.TimeoutError, ValueError, WebSocketDisconnect, RuntimeError):
        first = None
    if not isinstance(first, dict) or first.get("type") != "key" or not window_key.valid(first.get("key")):
        with contextlib.suppress(Exception):
            await socket.close(code=4403, reason=window_key.REFUSED)
        return
    try:
        session = terminals.get(sid)
    except KeyError:
        await socket.close(code=4404)
        return
    queue: asyncio.Queue = asyncio.Queue()
    backlog, last = session.attach(queue)
    exit_frame = json.dumps({"type": "exit"})
    typed = first.get("control") == "binary"

    async def control(frame: str) -> None:
        if typed:
            await socket.send_bytes(frame.encode("utf-8"))
        else:
            await socket.send_text(frame)

    async def outbound() -> None:
        if typed:
            await control(json.dumps({"type": "hello", "id": session.id, "kind": session.kind, "alive": session.alive}))
        if backlog:
            await socket.send_text(backlog)
        if not session.alive:
            await control(exit_frame)
        while True:
            seq, text = await queue.get()
            if text is None:
                await control(exit_frame)
                if terminals.SESSIONS.get(sid) is not session:
                    break
            elif seq > last:
                await socket.send_text(text)

    sender = asyncio.create_task(outbound())
    receiver = asyncio.create_task(socket.receive_text())
    try:
        while True:
            done, _ = await asyncio.wait({sender, receiver}, return_when=asyncio.FIRST_COMPLETED)
            if sender in done:
                break
            try:
                raw = receiver.result()
            except WebSocketDisconnect:
                break
            receiver = asyncio.create_task(socket.receive_text())
            try:
                message = json.loads(raw)
            except ValueError:
                continue
            if not isinstance(message, dict):
                continue
            if message.get("type") == "input":
                terminals.write(session, str(message.get("data", "")))
            elif message.get("type") == "resize":
                with contextlib.suppress(TypeError, ValueError):
                    terminals.resize(session, int(message.get("cols", 80)), int(message.get("rows", 24)))
    finally:
        session.detach(queue)
        sender.cancel()
        receiver.cancel()
        with contextlib.suppress(Exception):
            await socket.close()
