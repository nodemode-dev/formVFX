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


@router.post("/send")
async def send(request: Request) -> dict:
    body = await request.json()
    try:
        return chat.send(_base(request), str(body.get("text") or ""), body.get("context"))
    except ValueError as e:
        raise HTTPException(400, str(e)) from e
    except RuntimeError as e:
        raise HTTPException(409, str(e)) from e


@router.post("/cancel")
def cancel() -> dict:
    return {"cancelled": chat.cancel()}


@router.post("/reset")
def reset() -> dict:
    return chat.reset()
