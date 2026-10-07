from fastapi import APIRouter, Request

from ..agent import connect, consent, workspace
from ..engine.host import host

router = APIRouter(prefix="/api/agents", tags=["agents"])


@router.get("/status")
def status() -> dict:
    return {
        "workspace": workspace.describe(),
        "consent": consent.state(),
        "clients": connect.clients(),
        "windows": len(host.bus.windows()),
        "activity": list(host.activity)[-30:],
    }


@router.post("/install")
async def install(request: Request) -> dict:
    body = await request.json()
    which = body.get("clients") or ([body["client"]] if body.get("client") else None)
    return {"said": connect.install(which, dry=bool(body.get("dryRun")))}


@router.post("/forget")
def forget() -> dict:
    consent.forget_all()
    return consent.state()


@router.post("/workspace")
def ensure_workspace() -> dict:
    workspace.ensure()
    return workspace.describe()
