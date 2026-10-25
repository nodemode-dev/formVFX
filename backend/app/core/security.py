import ipaddress
from urllib.parse import urlsplit

from starlette.responses import PlainTextResponse

LOCAL_HEADER = "x-formvfx-local"


def loopback(value: str) -> bool:
    if value in ("localhost", "testclient", "testserver", "tauri.localhost"):
        return True
    try:
        return ipaddress.ip_address(value).is_loopback
    except ValueError:
        return False


def local_origin(origin: str | None) -> bool:
    if not origin:
        return True
    if origin.startswith("tauri://"):
        return True
    host = urlsplit(origin).hostname or ""
    return loopback(host)


class LocalOnly:
    def __init__(self, app):
        self.app = app

    async def __call__(self, scope, receive, send):
        if scope["type"] not in ("http", "websocket"):
            return await self.app(scope, receive, send)
        headers = {k.decode().lower(): v.decode() for k, v in scope.get("headers", [])}
        client = (scope.get("client") or ("",))[0]
        try:
            host = urlsplit("http://" + headers.get("host", "")).hostname or ""
            allowed = loopback(client) and loopback(host) and local_origin(headers.get("origin"))
            if "origin" not in headers and headers.get("sec-fetch-site", "").strip().lower() == "cross-site":
                allowed = False
        except ValueError:
            allowed = False
        path = scope.get("path", "")
        if (
            allowed
            and scope["type"] == "http"
            and scope.get("method") in ("POST", "PUT", "PATCH", "DELETE")
            and path.startswith("/api/")
        ):
            allowed = headers.get(LOCAL_HEADER) == "1"
        if not allowed:
            if scope["type"] == "websocket":
                return await send({"type": "websocket.close", "code": 1008})
            return await PlainTextResponse("Local API request refused", status_code=403)(scope, receive, send)

        async def safe_send(message):
            if message["type"] == "http.response.start":
                message.setdefault("headers", []).append((b"x-content-type-options", b"nosniff"))
            await send(message)

        await self.app(scope, receive, safe_send)
