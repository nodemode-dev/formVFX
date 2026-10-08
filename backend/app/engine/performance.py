from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

from ..core.config import data_dir

_FILE = "performance.json"
_PROBE = "gpu-probe.json"
_DEFAULTS = {"multiFrame": True, "reservedCores": 0, "gpu": True}


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
    if isinstance(raw.get("gpu"), bool):
        out["gpu"] = raw["gpu"]
    return out


def save(settings: dict) -> dict:
    cur = load()
    if "multiFrame" in settings:
        cur["multiFrame"] = bool(settings["multiFrame"])
    if "reservedCores" in settings:
        cur["reservedCores"] = max(0, min(cores() - 1, int(settings["reservedCores"])))
    if "gpu" in settings:
        cur["gpu"] = bool(settings["gpu"])
    (data_dir() / _FILE).write_text(json.dumps(cur, indent=2), encoding="utf-8")
    return cur


def gpu_allowed() -> bool:
    return os.environ.get("FORMVFX_GPU", "").strip().lower() not in ("0", "off", "false", "no")


def gpu_probe() -> dict:
    from .. import native

    core = native.module()
    stamp = f"{native.engine_name()} {core.__version__}"
    path = data_dir() / _PROBE
    try:
        cached = json.loads(path.read_text(encoding="utf-8"))
        if cached.get("core") == stamp and cached.get("platform") == sys.platform:
            return cached
    except (OSError, ValueError):
        pass
    if getattr(sys, "frozen", False):
        cmd = [sys.executable, "--gpu-probe"]
    else:
        cmd = [sys.executable, str(Path(__file__).resolve().parents[2] / "formvfx_engine.py"), "--gpu-probe"]
    try:
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=90)
        lines = out.stdout.strip().splitlines()
        result = json.loads(lines[-1]) if lines else {"ok": False, "error": (out.stderr or "").strip()[-300:] or f"the probe exited with {out.returncode}"}
    except (OSError, subprocess.TimeoutExpired, ValueError) as exc:
        result = {"ok": False, "error": str(exc)}
    result.update({"core": stamp, "platform": sys.platform})
    path.write_text(json.dumps(result), encoding="utf-8")
    return result


def gpu_status(probe: dict | None, attached: dict | None) -> dict:
    if not gpu_allowed():
        return {"enabled": False, "adapter": None, "reason": "FORMVFX_GPU turns the GPU off"}
    if probe is not None and not probe.get("ok"):
        return {"enabled": False, "adapter": None, "reason": probe.get("error") or ("no usable GPU adapter" if probe.get("built", True) else "the core was built without the GPU compositor")}
    return {"enabled": bool(attached and attached.get("enabled")), "adapter": (attached or {}).get("adapter") or (probe or {}).get("adapter"), "reason": None}


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
