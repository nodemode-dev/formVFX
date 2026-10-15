import json
import struct
from pathlib import Path

from fastapi import APIRouter, Depends, HTTPException, Query, Request, Response

from ..core.security import LOCAL_HEADER
from ..engine import link
from ..engine.rpc import RpcError


def _local(request: Request) -> None:
    if request.headers.get(LOCAL_HEADER) != "1":
        raise HTTPException(status_code=403, detail=f"send the {LOCAL_HEADER}: 1 header")


router = APIRouter(prefix="/api/link", tags=["link"], dependencies=[Depends(_local)])


def _fail(exc: RpcError) -> HTTPException:
    return HTTPException(status_code=404 if "no project" in str(exc) else 400, detail=str(exc))


@router.get("/info")
async def info(project: str | None = None, comp: str | None = None, projectId: str | None = None, compName: str | None = None, link_path: str | None = Query(None, alias="link")) -> dict:
    try:
        return await link.info(project, comp, projectId or None, compName or None, link_path)
    except RpcError as exc:
        raise _fail(exc) from None


@router.get("/status")
async def status() -> dict:
    return link.status()


@router.post("/check")
async def check(request: Request) -> dict:
    body = await request.json()
    descriptor = body.get("link") if isinstance(body.get("link"), dict) else None
    path = body.get("path")
    if descriptor is None and path:
        try:
            descriptor = json.loads(Path(path).expanduser().read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            raise HTTPException(status_code=400, detail=f"{path} is not a readable .fvxlink: {exc}") from None
    if not isinstance(descriptor, dict):
        raise HTTPException(status_code=400, detail="pass `link` (the descriptor) or `path` (a .fvxlink file)")
    try:
        return await link.check(descriptor, path, bool(body.get("write")))
    except RpcError as exc:
        raise _fail(exc) from None


@router.post("/render")
async def render(request: Request) -> dict:
    body = await request.json()
    descriptor = body.get("link") if isinstance(body.get("link"), dict) else None
    if descriptor is None and body.get("path"):
        try:
            descriptor = json.loads(Path(body["path"]).expanduser().read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            raise HTTPException(status_code=400, detail=f"{body['path']} is not a readable .fvxlink: {exc}") from None
    if not isinstance(descriptor, dict):
        raise HTTPException(status_code=400, detail="pass `link` (the descriptor) or `path` (a .fvxlink file)")
    try:
        return await link.render(descriptor, body.get("output"), str(body.get("format") or "prores"))
    except RpcError as exc:
        raise _fail(exc) from None


@router.get("/frame.png")
async def frame(project: str | None = None, comp: str | None = None, time: float = 0.0, max: int = Query(1920, ge=16, le=8192), transparent: bool = False, projectId: str | None = None, compName: str | None = None, link_path: str | None = Query(None, alias="link")) -> Response:
    try:
        w, h, png = await link.frame(project, comp, time, max, transparent, projectId or None, compName or None, link_path)
    except RpcError as exc:
        raise _fail(exc) from None
    return Response(png, media_type="image/png", headers={"cache-control": "no-store", "x-width": str(w), "x-height": str(h)})


@router.get("/audio")
async def audio(project: str | None = None, comp: str | None = None, start: float = 0.0, seconds: float = Query(2.0, gt=0, le=600), rate: int = 48000, projectId: str | None = None, compName: str | None = None, link_path: str | None = Query(None, alias="link")) -> Response:
    try:
        pcm = await link.audio(project, comp, start, seconds, rate, projectId or None, compName or None, link_path)
    except RpcError as exc:
        raise _fail(exc) from None
    header = b"RIFF" + struct.pack("<I", 36 + len(pcm)) + b"WAVE" + b"fmt " + struct.pack("<IHHIIHH", 16, 3, 2, rate, rate * 8, 8, 32) + b"data" + struct.pack("<I", len(pcm))
    return Response(header + pcm, media_type="audio/wav", headers={"cache-control": "no-store"})
