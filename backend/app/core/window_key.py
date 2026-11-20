import hmac
import json
import os
import secrets

HEADER = "x-formvfx-window-key"
STDOUT_ENV = "FORMVFX_WINDOW_KEY_STDOUT"
LINE = "formvfx-window-key: "
REFUSED = "Only the formvfx window can do this"
READS = ("GET", "HEAD", "OPTIONS")
OPEN = ("/mcp", "/api/window/key", "/api/rpc")
KEY = secrets.token_urlsafe(32)
STATE = {"claimed": False, "secondClaim": False}


def stdout_mode() -> bool:
    return os.environ.get(STDOUT_ENV) == "1"


def valid(value) -> bool:
    return isinstance(value, str) and bool(value) and hmac.compare_digest(value.encode("utf-8"), KEY.encode("utf-8"))


def claim() -> str | None:
    if STATE["claimed"]:
        STATE["secondClaim"] = True
        return None
    STATE["claimed"] = True
    return KEY


class WindowKeyGuard:
    def __init__(self, app) -> None:
        self.app = app

    async def __call__(self, scope, receive, send) -> None:
        if scope["type"] != "http" or scope.get("method") in READS or str(scope.get("path", "")) in OPEN:
            await self.app(scope, receive, send)
            return
        given = next((v.decode("latin-1") for k, v in scope.get("headers", []) if k.decode("latin-1").lower() == HEADER), None)
        if valid(given):
            await self.app(scope, receive, send)
            return
        body = json.dumps({"detail": REFUSED}).encode("utf-8")
        await send({"type": "http.response.start", "status": 403, "headers": [(b"content-type", b"application/json"), (b"content-length", str(len(body)).encode())]})
        await send({"type": "http.response.body", "body": body})
