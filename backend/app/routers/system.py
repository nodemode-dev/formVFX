import os
import platform

from fastapi import APIRouter

from .. import __version__, native
from ..core import config
from ..engine.host import host

router = APIRouter(tags=["system"])


@router.get("/api/health")
def health() -> dict:
    return {"status": "ok", "name": "formvfx", "version": __version__, "core": host.available, "error": host.error}


@router.get("/api/version")
def version() -> dict:
    ok, core = native.available()
    return {"version": __version__, "core": core if ok else None}


@router.get("/api/system")
def system() -> dict:
    return {
        "version": __version__,
        "platform": platform.system().lower(),
        "machine": platform.machine(),
        "python": platform.python_version(),
        "desktop": config.DESKTOP,
        "frozen": config.FROZEN,
        "dataDir": str(config.data_dir()),
        "home": os.path.expanduser("~"),
        "core": host.available,
        "error": host.error,
        "revision": host.revision,
        "busy": host.busy,
        "windows": len(host.bus.windows()),
        "agentsPolicy": config.agents_policy(),
    }
