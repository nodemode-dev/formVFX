from __future__ import annotations

import json


def _core():
    import fvx_core

    return fvx_core


def available() -> tuple[bool, str]:
    try:
        core = _core()
        return True, core.__version__
    except BaseException as exc:
        return False, repr(exc)


def version() -> str:
    try:
        return _core().version()
    except BaseException:
        return json.dumps({"core": None})


def encode_png(width: int, height: int, rgba: bytes) -> bytes:
    return _core().encode_png(width, height, rgba)
