import json

from fastapi import APIRouter, Request, Response

from ..agent import consent
from ..engine import rpc

router = APIRouter(tags=["rpc"])


@router.post("/api/rpc")
async def call(request: Request) -> Response:
    try:
        message = json.loads(await request.body())
    except ValueError:
        return Response(json.dumps({"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "Parse error"}}), media_type="application/json", status_code=400)
    source = request.headers.get("x-formvfx-source", "cli")[:60] or "cli"
    if source != "ui":
        denied = await consent.gate(source, request.headers.get("x-formvfx-agent-token"), mutating=consent.mutating_rpc(message))
        if denied:
            rid = message.get("id") if isinstance(message, dict) else None
            return Response(json.dumps({"jsonrpc": "2.0", "id": rid, "error": {"code": -32003, "message": denied}}), media_type="application/json")
    out = await rpc.handle(message, source)
    if out is None:
        return Response(status_code=204)
    return Response(json.dumps(out), media_type="application/json")
