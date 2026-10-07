import os
import sys
from pathlib import Path

FRONTEND_PORT = 5230
BACKEND_PORT = 8230


def _load_env_file() -> None:
    env = Path(__file__).resolve().parents[2] / ".env"
    if not env.is_file():
        return
    for line in env.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key, value = key.strip(), value.strip()
        if key and value and key not in os.environ:
            os.environ[key] = value


_load_env_file()

HOST = os.environ.get("FORMVFX_HOST", "127.0.0.1")
PORT = int(os.environ.get("FORMVFX_PORT", str(BACKEND_PORT)) or BACKEND_PORT)
DESKTOP = os.environ.get("FORMVFX_DESKTOP") == "1"
FROZEN = bool(getattr(sys, "frozen", False))


def data_dir() -> Path:
    raw = os.environ.get("FORMVFX_DATA_DIR", "").strip()
    root = Path(raw).expanduser() if raw else Path.home() / "Formvfx"
    root.mkdir(parents=True, exist_ok=True)
    return root


def repo_root() -> Path | None:
    here = Path(__file__).resolve().parents[3]
    return here if (here / "formvfx").is_file() and (here / "backend").is_dir() else None


def bundle_dir() -> Path:
    base = getattr(sys, "_MEIPASS", None)
    return Path(base) if base else Path(__file__).resolve().parents[2]


def skills_source() -> Path | None:
    frozen = bundle_dir() / "skills"
    if frozen.is_dir():
        return frozen
    root = repo_root()
    if root and (root / ".claude" / "skills").is_dir():
        return root / ".claude" / "skills"
    return None


def dist_dir() -> Path | None:
    raw = os.environ.get("FORMVFX_DIST", "").strip()
    if raw:
        p = Path(raw)
        return p if (p / "index.html").is_file() else None
    frozen = bundle_dir() / "frontend_dist"
    return frozen if (frozen / "index.html").is_file() else None


def cors_origins() -> list[str]:
    raw = os.environ.get("FORMVFX_CORS_ORIGINS", "")
    if raw.strip():
        return [o.strip() for o in raw.split(",") if o.strip()]
    return [f"http://localhost:{FRONTEND_PORT}", f"http://127.0.0.1:{FRONTEND_PORT}", f"http://localhost:{FRONTEND_PORT + 1}"]


def agents_policy() -> str:
    value = os.environ.get("FORMVFX_AGENTS", "ask").strip().lower()
    return value if value in ("ask", "allow", "deny") else "ask"
