import asyncio

from fastapi import APIRouter, HTTPException, Request

from ..agent import chat, claude_code

router = APIRouter(prefix="/api/chat", tags=["chat"])


def _base(request: Request) -> str:
    return f"http://127.0.0.1:{request.url.port or 80}"


@router.get("/state")
def state() -> dict:
    return chat.state()


@router.get("/claude")
def claude(fresh: bool = False) -> dict:
    return claude_code.info(fresh)


async def _body(request: Request) -> dict:
    try:
        body = await request.json()
    except ValueError:
        return {}
    return body if isinstance(body, dict) else {}


@router.get("/sessions")
def sessions() -> dict:
    return chat.sessions()


@router.post("/adopt")
async def adopt(request: Request) -> dict:
    body = await _body(request)
    try:
        return await asyncio.get_running_loop().run_in_executor(None, chat.adopt, str(body.get("session") or ""))
    except ValueError as e:
        raise HTTPException(400, str(e)) from e
    except LookupError as e:
        raise HTTPException(404, str(e)) from e
    except RuntimeError as e:
        raise HTTPException(409, str(e)) from e


@router.post("/send")
async def send(request: Request) -> dict:
    body = await _body(request)
    args = (_base(request), str(body.get("text") or ""), str(body.get("context") or ""), str(body.get("note") or ""))
    try:
        return await asyncio.get_running_loop().run_in_executor(None, chat.send, *args)
    except ValueError as e:
        raise HTTPException(400, str(e)) from e
    except claude_code.NotInstalled as e:
        raise HTTPException(400, str(e)) from e
    except RuntimeError as e:
        raise HTTPException(409, str(e)) from e


@router.post("/cancel")
def cancel() -> dict:
    return {"cancelled": chat.cancel()}


@router.post("/reset")
def reset() -> dict:
    return chat.reset()
