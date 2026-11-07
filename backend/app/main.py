import asyncio
import mimetypes
import os
from contextlib import asynccontextmanager

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles

from . import __version__, ffmpeg_fetch
from .agent import workspace
from .core import config, discovery
from .core.security import LocalOnly
from .engine import rpc as engine_rpc
from .engine.host import host
from .engine import link as engine_link
from .routers import agents, chat, extensions, files, formext, link, live, mcp, rpc, system, terminal, viewer

mimetypes.add_type("text/javascript", ".js")
mimetypes.add_type("text/javascript", ".mjs")
mimetypes.add_type("text/css", ".css")
ffmpeg_fetch.start()

ROUTERS = (system.router, rpc.router, live.router, viewer.router, files.router, terminal.router, chat.router, mcp.router, agents.router, link.router, formext.router, extensions.router)


@asynccontextmanager
async def lifespan(app: FastAPI):
    loop = asyncio.get_running_loop()
    await loop.run_in_executor(None, host.start, loop)
    try:
        workspace.ensure()
    except OSError:
        pass
    opening = os.environ.get("FORMVFX_OPEN", "").strip()
    if opening:
        try:
            await engine_rpc.execute("file.open", {"path": opening}, "desktop")
        except Exception:
            pass
    if not config.DESKTOP:
        try:
            discovery.publish(f"http://127.0.0.1:{config.PORT}", __version__)
        except OSError:
            pass
    yield
    engine_link.stop_all()
    host.stop()
    discovery.withdraw()


app = FastAPI(title="formvfx engine", version=__version__, lifespan=lifespan)
app.add_middleware(CORSMiddleware, allow_origins=config.cors_origins(), allow_credentials=False, allow_methods=["*"], allow_headers=["*"])
app.add_middleware(LocalOnly)
for r in ROUTERS:
    app.include_router(r)

_dist = config.dist_dir()
if _dist:
    app.mount("/", StaticFiles(directory=str(_dist), html=True), name="interface")
