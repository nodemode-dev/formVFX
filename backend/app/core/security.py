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


def _origin_key(value: str) -> tuple[str, str, int | None] | None:
    try:
        parts = urlsplit(value.strip())
        port = parts.port
    except ValueError:
        return None
    if not parts.scheme or not parts.hostname:
        return None
    if port is None:
        port = {"http": 80, "https": 443}.get(parts.scheme)
    return parts.scheme.lower(), parts.hostname.lower(), port


def window_origin(origin: str, host: str) -> bool:
    from .config import cors_origins

    key = _origin_key(origin)
    if key is None:
        return False
    if key in {k for k in (_origin_key(o) for o in [*cors_origins(), "tauri://localhost", "http://tauri.localhost"]) if k is not None}:
        return True
    own = _origin_key(f"{key[0]}://{host}")
    return key[0] == "http" and own is not None and own == key


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
            allowed = loopback(client) and loopback(host) and (headers.get("origin") is None or window_origin(headers["origin"], headers.get("host", "")))
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
