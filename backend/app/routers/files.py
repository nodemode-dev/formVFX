import os
import re
import shutil
from pathlib import Path

from fastapi import APIRouter, HTTPException, Request, UploadFile
from fastapi.responses import FileResponse

from ..agent import workspace
from ..core import config

router = APIRouter(prefix="/api/fs", tags=["files"])
_SAFE = re.compile(r"[^A-Za-z0-9._ -]+")


def _resolve(raw: str | None) -> Path:
    if not raw:
        return Path.home()
    p = Path(os.path.expanduser(raw))
    if not p.is_absolute():
        raise HTTPException(400, "paths must be absolute")
    return p


@router.get("/roots")
def roots() -> dict:
    data = config.data_dir()
    ws = workspace.ensure()
    places = [
        {"name": "Home", "path": str(Path.home())},
        {"name": "formvfx", "path": str(data)},
        {"name": "Projects", "path": str(ws / "projects")},
        {"name": "Renders", "path": str(ws / "renders")},
        {"name": "Agent workspace", "path": str(ws)},
    ]
    for name in ("Desktop", "Documents", "Downloads", "Movies", "Videos", "Pictures", "Music"):
        p = Path.home() / name
        if p.is_dir():
            places.append({"name": name, "path": str(p)})
    root = config.repo_root()
    if root:
        places.append({"name": "formvfx repo", "path": str(root)})
    return {"places": places, "home": str(Path.home()), "sep": os.sep}


@router.get("/list")
def listing(path: str | None = None, hidden: bool = False) -> dict:
    p = _resolve(path)
    if not p.is_dir():
        raise HTTPException(404, f"not a folder: {p}")
    entries = []
    try:
        with os.scandir(p) as it:
            for e in it:
                if not hidden and e.name.startswith("."):
                    continue
                try:
                    is_dir = e.is_dir()
                    st = e.stat()
                except OSError:
                    continue
                entries.append({"name": e.name, "path": str(Path(p) / e.name), "dir": is_dir, "size": 0 if is_dir else st.st_size, "mtime": st.st_mtime})
    except PermissionError:
        raise HTTPException(403, f"cannot read {p}") from None
    entries.sort(key=lambda x: (not x["dir"], x["name"].lower()))
    parent = str(p.parent) if p.parent != p else None
    return {"path": str(p), "parent": parent, "entries": entries}


@router.post("/mkdir")
async def mkdir(request: Request) -> dict:
    body = await request.json()
    p = _resolve(body.get("path"))
    p.mkdir(parents=True, exist_ok=True)
    return {"path": str(p)}


@router.post("/upload")
async def upload(files: list[UploadFile]) -> dict:
    dest = config.data_dir() / "imports"
    dest.mkdir(parents=True, exist_ok=True)
    paths = []
    for f in files:
        name = _SAFE.sub("_", Path(f.filename or "file").name) or "file"
        target = dest / name
        stem, suffix, n = target.stem, target.suffix, 1
        while target.exists():
            target = dest / f"{stem} {n}{suffix}"
            n += 1
        with target.open("wb") as out:
            shutil.copyfileobj(f.file, out)
        paths.append(str(target))
    return {"paths": paths}


@router.get("/file")
def file(path: str) -> FileResponse:
    p = _resolve(path)
    if not p.is_file():
        raise HTTPException(404, f"no file at {p}")
    return FileResponse(p, filename=p.name)
