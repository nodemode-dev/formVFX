from __future__ import annotations

import contextlib
import hashlib
import logging
import os
import platform
import shutil
import sys
import tarfile
import tempfile
import threading
from pathlib import Path

from .ffmpeg_record import FILES, RELEASE

SOURCE = "https://github.com/nodemode-dev/formcut-binaries/releases/download"
DOCUMENTS = ("LGPL-2.1.txt", "THIRD-PARTY-NOTICES.txt", "ffmpeg-licence.json")

log = logging.getLogger(__name__)


def host_triple() -> str:
    arch = "aarch64" if platform.machine().lower() in ("arm64", "aarch64") else "x86_64"
    if sys.platform == "darwin":
        return f"{arch}-apple-darwin"
    if os.name == "nt":
        return f"{arch}-pc-windows-msvc"
    return f"{arch}-unknown-linux-gnu"


def folder() -> Path:
    if sys.platform == "darwin":
        base = Path.home() / "Library" / "Application Support"
    elif os.name == "nt":
        base = Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData" / "Local")
    else:
        base = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local" / "share")
    return base / "formcut-binaries" / f"ffmpeg-{RELEASE}"


def source() -> str:
    base = os.environ.get("FORMCUT_FFMPEG_RELEASE") or f"{SOURCE}/ffmpeg-{RELEASE}"
    return f"{base.rstrip('/')}/ffmpeg-lgpl-{RELEASE}-{host_triple()}.tar.gz"


def _names(program: str) -> tuple[str, str]:
    suffix = ".exe" if os.name == "nt" else ""
    return f"{program}{suffix}", f"{program}-{host_triple()}{suffix}"


def _plain_names(where: Path) -> None:
    for program in ("ffmpeg", "ffprobe"):
        plain, tripled = (where / name for name in _names(program))
        if plain.exists() or plain.is_symlink() or not tripled.is_file():
            continue
        with contextlib.suppress(OSError):
            if os.name != "nt":
                plain.symlink_to(tripled.name)
            else:
                try:
                    os.link(tripled, plain)
                except OSError:
                    shutil.copy2(tripled, plain)


def prefer() -> Path | None:
    where = folder()
    if not (where / _names("ffmpeg")[1]).is_file():
        return None
    _plain_names(where)
    if not (where / _names("ffmpeg")[0]).is_file():
        return None
    first = [where]
    if os.name == "nt":
        first += sorted(d for d in where.glob("lib-*") if d.is_dir())
    parts = [p for p in os.environ.get("PATH", "").split(os.pathsep) if p]
    missing = [str(d) for d in first if str(d) not in parts]
    if missing:
        os.environ["PATH"] = os.pathsep.join(missing + parts)
    return where


def _digest(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def _download(url: str, target: Path) -> None:
    import httpx

    with httpx.stream("GET", url, follow_redirects=True, timeout=120) as response:
        response.raise_for_status()
        with target.open("wb") as out:
            for chunk in response.iter_bytes(1 << 20):
                out.write(chunk)


def _root(unpacked: Path, first: str) -> Path:
    for hit in unpacked.rglob(Path(first).name):
        candidate = hit
        for _ in Path(first).parts:
            candidate = candidate.parent
        if (candidate / first).is_file():
            return candidate
    raise ValueError(f"the archive has no {first}")


def _install(archive: Path, pins: tuple[tuple[str, str], ...], where: Path) -> None:
    where.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=where.parent) as scratch:
        unpacked = Path(scratch) / "archive"
        with tarfile.open(archive) as bundle:
            bundle.extractall(unpacked, filter="data")
        root = _root(unpacked, pins[0][0])
        for path, digest in pins:
            if not (root / path).is_file() or _digest(root / path) != digest:
                raise ValueError(f"{path} does not match the record, so nothing was installed")
        staged = Path(scratch) / "staged"
        for path, _ in pins:
            (staged / path).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(root / path, staged / path)
        for name in DOCUMENTS:
            if (root / name).is_file():
                shutil.copy2(root / name, staged / name)
        if not where.exists():
            with contextlib.suppress(OSError):
                os.replace(staged, where)
    if not (where / _names("ffmpeg")[1]).is_file():
        raise ValueError(f"ffmpeg could not be placed in {where}")


def fetch() -> Path | None:
    pins = FILES.get(host_triple())
    if not pins:
        raise ValueError(f"no ffmpeg build is recorded for {host_triple()}")
    with tempfile.TemporaryDirectory() as scratch:
        archive = Path(scratch) / "ffmpeg.tar.gz"
        _download(source(), archive)
        _install(archive, pins, folder())
    return prefer()


def _run() -> None:
    try:
        log.info("ffmpeg %s ready in %s", RELEASE, fetch())
    except Exception as error:
        log.warning("ffmpeg %s could not be fetched: %s", RELEASE, error)


def start() -> None:
    if prefer() or shutil.which("ffmpeg"):
        return
    if os.environ.get("FORM_FFMPEG_AUTO", "1" if getattr(sys, "frozen", False) else "0") == "0":
        return
    threading.Thread(target=_run, name="ffmpeg-fetch", daemon=True).start()
