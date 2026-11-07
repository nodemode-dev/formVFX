from __future__ import annotations

import base64
import binascii
import os
from pathlib import Path

from fastapi import APIRouter, Request
from fastapi.responses import FileResponse, JSONResponse, Response

from .. import __version__
from ..services import formext_host as fx

PRODUCT = "formvfx"
APP = "FormVFX"
BASE = "/api/extensions/files"
SAVE_CAP = 200 * 1024 * 1024

router = APIRouter(tags=["formext"])
_state: dict = {"stamp": None, "extensions": []}


def _resolve(force: bool = False) -> list[fx.Extension]:
    stamp = fx.stamp(PRODUCT)
    if force or _state["stamp"] != stamp:
        _state["extensions"] = fx.resolve(PRODUCT, __version__, APP)
        _state["stamp"] = stamp
    return _state["extensions"]


def listing(force: bool = False) -> dict:
    found = _resolve(force)
    return {
        "product": PRODUCT,
        "app": APP,
        "appVersion": __version__,
        "api": fx.API,
        "root": str(fx.portal_root()),
        "stamp": _token(_state["stamp"]),
        "extensions": [e.public(BASE) for e in found],
    }


def _token(stamp: tuple[float, float]) -> str:
    return ",".join(repr(v) for v in stamp)


@router.get("/api/extensions")
def extensions(since: str | None = None, reload: bool = False) -> dict:
    if since is not None and not reload:
        now = fx.stamp(PRODUCT)
        if since == _token(now) and _state["stamp"] == now:
            return {"stamp": since, "unchanged": True}
    return listing(force=reload)


def _missing() -> Response:
    return Response(status_code=404, headers=fx.headers_for("text/plain", ""))


@router.api_route(BASE + "/{ext_id}/{version}/{path:path}", methods=["GET", "HEAD"])
def extension_file(ext_id: str, version: str, path: str) -> Response:
    hit = fx.file_for(_resolve(), ext_id, version, path)
    if hit is None:
        hit = fx.file_for(_resolve(force=True), ext_id, version, path)
    if hit is None:
        return _missing()
    target, kind = hit
    return FileResponse(target, media_type=kind, headers=fx.headers_for(kind, ""))


@router.post("/api/extensions/save")
async def save(request: Request) -> Response:
    try:
        body = await request.json()
        target = Path(os.path.expanduser(str(body.get("path") or "")))
        data = base64.b64decode(str(body.get("data") or ""), validate=True)
    except (ValueError, binascii.Error):
        return JSONResponse({"error": "the file data is not base64"}, status_code=400)
    if not target.is_absolute() or not target.parent.is_dir():
        return JSONResponse({"error": "choose a folder that exists"}, status_code=400)
    if len(data) > SAVE_CAP:
        return JSONResponse({"error": "the file is larger than 200 MB"}, status_code=400)
    try:
        target.write_bytes(data)
    except OSError as exc:
        return JSONResponse({"error": f"could not save {target.name}: {exc.strerror or exc}"}, status_code=400)
    return JSONResponse({"saved": True, "name": target.name, "path": str(target)})
