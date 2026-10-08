from __future__ import annotations

import json
import os

from ..core.config import data_dir

_FILE = "performance.json"
_DEFAULTS = {"multiFrame": True, "reservedCores": 0}


def cores() -> int:
    return max(1, os.cpu_count() or 1)


def load() -> dict:
    try:
        raw = json.loads((data_dir() / _FILE).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        raw = {}
    out = dict(_DEFAULTS)
    if isinstance(raw.get("multiFrame"), bool):
        out["multiFrame"] = raw["multiFrame"]
    if isinstance(raw.get("reservedCores"), int):
        out["reservedCores"] = max(0, min(cores() - 1, raw["reservedCores"]))
    return out


def save(settings: dict) -> dict:
    cur = load()
    if "multiFrame" in settings:
        cur["multiFrame"] = bool(settings["multiFrame"])
    if "reservedCores" in settings:
        cur["reservedCores"] = max(0, min(cores() - 1, int(settings["reservedCores"])))
    (data_dir() / _FILE).write_text(json.dumps(cur, indent=2), encoding="utf-8")
    return cur


def render_threads(settings: dict | None = None) -> int:
    s = settings or load()
    return max(1, cores() - s["reservedCores"])


def preview_frames(settings: dict | None = None) -> int:
    s = settings or load()
    return max(1, min(8, render_threads(s))) if s["multiFrame"] else 1


def apply_env() -> None:
    if not os.environ.get("RAYON_NUM_THREADS"):
        os.environ["RAYON_NUM_THREADS"] = str(render_threads())


def describe() -> dict:
    s = load()
    return {
        **s,
        "cores": cores(),
        "previewFrames": preview_frames(s),
        "renderThreads": render_threads(s),
        "activeRenderThreads": int(os.environ.get("RAYON_NUM_THREADS") or 0) or render_threads(s),
    }
