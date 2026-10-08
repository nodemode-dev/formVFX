from __future__ import annotations

import json
from pathlib import Path

from ..core.config import bundle_dir, repo_root

_cache: dict | None = None


def _file() -> Path | None:
    for base in (repo_root(), bundle_dir()):
        if base and (base / "presets" / "text-animators.json").is_file():
            return base / "presets" / "text-animators.json"
    return None


def library() -> dict:
    global _cache
    if _cache is None:
        f = _file()
        _cache = json.loads(f.read_text(encoding="utf-8")) if f else {"categories": [], "presets": []}
    return _cache


def listing(category: str | None = None, query: str | None = None) -> list[dict]:
    q = (query or "").lower().split()
    out = []
    for p in library()["presets"]:
        if category and p["category"].lower() != category.lower():
            continue
        hay = f"{p['name']} {p['category']} {p['description']}".lower()
        if q and not all(w in hay for w in q):
            continue
        out.append({"id": p["id"], "name": p["name"], "category": p["category"], "description": p["description"], "perChar3d": bool(p.get("perChar3d"))})
    return out


def find(ref: str) -> dict | None:
    r = (ref or "").strip().lower()
    for p in library()["presets"]:
        if p["id"].lower() == r or p["name"].lower() == r:
            return p
    return None
