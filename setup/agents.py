#!/usr/bin/env python3
"""./formvfx agents: Claude Code, Codex and Cursor driving formvfx.

  sync [--check]    regenerate the formvfx-harness skill's generated references from the engine (commands, effects,
                    property paths, MCP tools) and mirror .claude/skills/formvfx-harness to .agents/skills;
                    --check exits 1 when anything drifted
  install [--dry-run] [claude|codex|cursor ...]
                    set the clients up for this user account: the MCP server and the skill (all three by default)
  status            which clients are on this computer and whether the skill is in step
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BACKEND = ROOT / "backend"
SKILL = ROOT / ".claude" / "skills" / "formvfx-harness"
MIRROR = ROOT / ".agents" / "skills" / "formvfx-harness"
GENERATED = ("commands.md", "effects.md", "property-paths.md", "tools.md")
ORDER = ["file", "edit", "project", "item", "footage", "comp", "layer", "prop", "keys", "anim", "time", "markers", "effect", "mask", "path",
         "paths", "shape", "text", "camera", "light", "material", "track", "warp", "roto", "paint", "puppet", "liquify", "contentFill", "face",
         "essential", "expr", "script", "scriptui", "render", "renderQueue", "playback", "view", "window", "editor", "scopes", "cache", "jobs",
         "mediaBrowser", "templates", "prefs", "shortcuts", "storage", "app", "engine", "command", "motion", "learn", "help"]
HEAD = "Generated from the formvfx engine by `./formvfx agents sync`; do not edit by hand."


def venv_python() -> Path | None:
    for p in (BACKEND / "venv" / "bin" / "python", BACKEND / "venv" / "Scripts" / "python.exe"):
        if p.exists():
            return p
    return None


def ensure_core() -> None:
    try:
        import fvx_core  # noqa: F401
    except ImportError:
        py = venv_python()
        if py and Path(sys.prefix).resolve() != (BACKEND / "venv").resolve() and not os.environ.get("FORMVFX_AGENTS_REEXEC"):
            os.environ["FORMVFX_AGENTS_REEXEC"] = "1"
            os.execv(str(py), [str(py), __file__, *sys.argv[1:]])
        sys.exit("agents: the compositing engine is not built; run ./formvfx install first")
    sys.path.insert(0, str(BACKEND))


class Engine:
    def __init__(self) -> None:
        import fvx_core
        self.e = fvx_core.Engine()

    def x(self, command: str, params: dict | None = None):
        return json.loads(self.e.execute(command, json.dumps(params or {})))

    def effects(self) -> list:
        return json.loads(self.e.effects())


def commands_md(eng: Engine) -> str:
    cmds = eng.x("command.list")
    groups: dict[str, list] = {}
    for c in cmds:
        groups.setdefault(c["id"].split(".")[0], []).append(c)
    names = [n for n in ORDER if n in groups] + sorted(n for n in groups if n not in ORDER)
    out = ["# formvfx commands", "", HEAD, "",
           f"{len(cmds)} commands in {len(groups)} namespaces. Every menu item, button and drag in formvfx is one of these. Run one with "
           "`execute {command, params}` over MCP, inside `batch` for several as one undo step, or `formvfx exec ID 'JSON'` from a shell. "
           "`describe_command ID` returns the JSON Schema. In the parameter summaries `?` marks an optional parameter, `a|b` lists the "
           "accepted values, and `layers?`/`layer?` default to the selected layers. Times are composition seconds unless the summary says "
           "layer seconds.", "", "## Contents", "",
           " ".join(f"`{n}` ({len(groups[n])})" for n in names), ""]
    for n in names:
        out += [f"## {n}", ""]
        for c in groups[n]:
            menu = " > ".join(c.get("menu") or [])
            keys = f" · {c['shortcut']}" if c.get("shortcut") else ""
            where = f" · {menu}" if menu else ""
            out.append(f"- `{c['id']}` {c['label'].rstrip('.')}{where}{keys}")
            params = (c.get("params") or "{}").strip()
            if params and params != "{}":
                out.append(f"  `{params}`")
        out.append("")
    return "\n".join(out)


def _ui(p: dict) -> str:
    ui = p.get("ui") or {}
    kind = ui.get("kind", "")
    if kind == "Popup":
        opts = ui.get("options") or []
        shown = "|".join(str(o) for o in opts[:10]) + ("|..." if len(opts) > 10 else "")
        return f"menu {shown}"
    if kind in ("Slider", "Angle", "Number"):
        lo, hi = ui.get("slider_min", ui.get("min")), ui.get("slider_max", ui.get("max"))
        return f"{kind.lower()} {lo:g}..{hi:g}" if isinstance(lo, (int, float)) and isinstance(hi, (int, float)) else kind.lower()
    return kind.lower() or "value"


def _default(v) -> str:
    if isinstance(v, float):
        return f"{v:g}"
    if isinstance(v, list):
        return "[" + ", ".join(_default(x) for x in v) + "]"
    return json.dumps(v)


def effects_md(eng: Engine) -> str:
    fx = eng.effects()
    cats: dict[str, list] = {}
    for f in fx:
        cats.setdefault(f["category"], []).append(f)
    out = ["# formvfx effects", "", HEAD, "",
           f"{len(fx)} effects in {len(cats)} categories. Apply with `apply_effect {{layer, effect, params}}` (the id or the name) or "
           "`execute effect.apply {effect, layers}`. Each parameter is listed as `id` = default (control). After applying, the "
           "parameter's property path is `effects/#N/<id>` where N is the effect's position on the layer (1 = top); `get_layer` shows "
           "the exact paths. GPU marks effects that render on the GPU when it is available; every effect also renders on the CPU.", "",
           "## Contents", "", " ".join(f"`{c}` ({len(v)})" for c, v in sorted(cats.items())), ""]
    for cat in sorted(cats):
        out += [f"## {cat}", ""]
        for f in sorted(cats[cat], key=lambda f: f["name"].lower()):
            flags = " · GPU" if f.get("gpu") else ""
            out.append(f"- **{f['name']}** `{f['id']}`{flags}")
            ps = [f"`{p['id']}` = {_default(p.get('default'))} ({_ui(p)})" for p in f.get("params", []) if p.get("id")]
            if ps:
                out.append("  " + "; ".join(ps))
        out.append("")
    return "\n".join(out)


SAMPLES = [
    ("Solid with an effect and a mask", [("layer.newSolid", {"name": "Solid", "color": "#3366ff"}), ("effect.apply", {"effect": "Gaussian Blur", "layers": ["$L"]}),
                                          ("layer.addMask", {"layer": "$L", "shape": "ellipse", "rect": [100, 100, 400, 300]})]),
    ("Text with an animator", [("layer.newText", {"text": "Title", "size": 96}), ("layer.addTextAnimator", {"layer": "$L", "properties": ["position", "opacity"]})]),
    ("Shape layer (rectangle) with Trim Paths and a Repeater", [("layer.newShape", {"kind": "rect", "size": [300, 200]}),
                                                                 ("layer.addShapeItem", {"layer": "$L", "kind": "trim"}), ("layer.addShapeItem", {"layer": "$L", "kind": "repeater"})]),
    ("3D layer", [("layer.newSolid", {"name": "Card", "width": 800, "height": 450}), ("layer.setSwitch", {"layers": ["$L"], "switch": "threeD", "value": True})]),
    ("Camera", [("layer.newCamera", {"preset": "35mm"})]),
    ("Spot light", [("layer.newLight", {"kind": "Spot"})]),
    ("Null", [("layer.newNull", {})]),
]


def _walk(node: dict, out: list) -> None:
    for c in node.get("children", []):
        if "children" in c:
            if c.get("path"):
                out.append(f"- `{c['path']}` · {c.get('name', '')} (group)")
            _walk(c, out)
        elif c.get("path"):
            v = c.get("value")
            shown = _default(v) if not isinstance(v, (dict, str)) or isinstance(v, str) and len(v) < 40 else "..."
            out.append(f"- `{c['path']}` · {c.get('name', '')} · {c.get('type', '')} · {shown}")


def paths_md(eng: Engine) -> str:
    eng.x("comp.new", {"name": "Paths", "width": 1920, "height": 1080, "duration": 5, "frameRate": 30})
    out = ["# formvfx property paths", "", HEAD, "",
           "Every animatable value is a property with a path in its layer's tree. Use the path with `set_property`, `set_keyframes`, "
           "`get_property`, `prop.set`, `prop.addKey` and `prop.setExpression`. The lists below are the trees of fresh layers made by "
           "the commands shown, with their default values. Each segment of a path can be written three ways: the match name shown here "
           "(`masks/mask/feather`, a second one is `mask#2`), the 1-based index among its siblings (`masks/#2/feather`, `effects/#1/blurriness`, "
           "`text/animators/#1/selectors/#1/start`), or the display name (`masks/Mask 2/feather`, `effects/Gaussian Blur/blurriness`). "
           "`get_layer` on the real layer is always the authority.", "", "## Contents", "",
           " · ".join(title for title, _ in SAMPLES), ""]
    for title, steps in SAMPLES:
        made = None
        for cmd, params in steps:
            bound = json.loads(json.dumps(params).replace('"$L"', json.dumps(made)))
            r = eng.x(cmd, bound)
            if made is None and isinstance(r, dict) and "layer" in r:
                made = r["layer"]
        tree = eng.x("layer.tree", {"layer": made, "depth": 12})
        cmds = ", ".join(f"`{c}`" for c, _ in steps)
        out += [f"## {title}", "", f"Made with {cmds}.", ""]
        lines: list[str] = []
        _walk(tree.get("properties", {}), lines)
        out += lines + [""]
    return "\n".join(out)


def tools_md() -> str:
    from app.agent.tools import GUIDES, TOOLS
    out = ["# formvfx MCP tools", "", HEAD, "",
           f"The `formvfx` MCP server has {len(TOOLS)} tools. `./formvfx mcp` serves them over stdio (it starts the engine when it is not "
           "running); the engine also serves them over Streamable HTTP at `http://127.0.0.1:8250/mcp`. The shell equivalents are in the "
           "last section.", "", "## Contents", "", " ".join(f"`{t['name']}`" for t in TOOLS), ""]
    for t in TOOLS:
        a = t["annotations"]
        marks = [m for m, on in (("read-only", a["readOnlyHint"]), ("replaces or removes", a["destructiveHint"])) if on]
        out += [f"## {t['name']}", "", t["description"], ""]
        props, required = t["inputSchema"]["properties"], set(t["inputSchema"]["required"])
        for k, v in props.items():
            kind = v.get("type") or ("id or name" if "anyOf" in v else "any")
            if "enum" in v:
                kind += " (" + " | ".join(map(str, v["enum"])) + ")"
            extra = "; ".join(x for x in (("required" if k in required else ""), (f"default {json.dumps(v['default'])}" if "default" in v else ""), v.get("description", "")) if x)
            out.append(f"- `{k}`: {kind}" + (f"; {extra}" if extra else ""))
        if not props:
            out.append("- no parameters")
        if marks:
            out += ["", "Note: " + ", ".join(marks) + "."]
        out.append("")
    out += ["## Guide topics", "", "The `guide` tool returns these texts.", ""]
    for k, v in GUIDES.items():
        if k in ("index", "workflow"):
            continue
        out += [f"### {k}", "", v, ""]
    out += ["## The same surface from a shell", "",
            "- `formvfx state` · the open project, active composition, selection and time (like `status`)",
            "- `formvfx commands FILTER` · list commands (like `list_commands`)",
            "- `formvfx describe ID` · one command with its schema (like `describe_command`)",
            "- `formvfx exec ID 'JSON'` · run one command (like `execute`)",
            "- `formvfx batch '[{\"command\": ID, \"params\": {}}]'` · several as one undo step (like `batch`)",
            "- `formvfx frame OUT.png --time S --comp C --max N` · save a rendered frame (like `render_frame`)",
            "- `formvfx render --wait` · render the Render Queue", ""]
    return "\n".join(out)


def generate() -> dict[str, str]:
    eng = Engine()
    docs = {"commands.md": commands_md(eng), "effects.md": effects_md(eng), "tools.md": tools_md()}
    docs["property-paths.md"] = paths_md(Engine())
    return {k: re.sub(r"<([A-Za-z#][^<>\n]{0,40})>", r"[\1]", v.replace(" — ", " · ").replace("—", "·")) for k, v in docs.items()}


def _files(folder: Path) -> dict:
    return {str(p.relative_to(folder)): p.read_bytes() for p in sorted(folder.rglob("*")) if p.is_file()}


def sync(check: bool) -> int:
    ensure_core()
    drift = []
    for name, text in generate().items():
        ref = SKILL / "references" / name
        if not ref.exists() or ref.read_text(encoding="utf-8") != text:
            drift.append(str(ref.relative_to(ROOT)))
            if not check:
                ref.parent.mkdir(parents=True, exist_ok=True)
                ref.write_text(text, encoding="utf-8")
    if not MIRROR.exists() or _files(MIRROR) != _files(SKILL):
        drift.append(str(MIRROR.relative_to(ROOT)))
        if not check:
            shutil.rmtree(MIRROR, ignore_errors=True)
            shutil.copytree(SKILL, MIRROR)
    if check:
        print("agents: in step" if not drift else "agents: drifted: " + ", ".join(drift) + " (run ./formvfx agents sync)")
        return 1 if drift else 0
    print("agents: " + ("updated " + ", ".join(drift) if drift else "already in step"))
    return 0


def install(args: list[str]) -> int:
    ensure_core()
    from app.agent import connect
    dry = "--dry-run" in args
    which = [a for a in args if not a.startswith("--")] or None
    for line in connect.install(which, dry):
        print(line)
    return 0


def status() -> int:
    ensure_core()
    from app.agent import connect
    for name, on in connect.clients().items():
        print(f"{name}: {'found' if on else 'not found'}")
    return subprocess.call([sys.executable, __file__, "sync", "--check"])


def main(argv: list[str]) -> int:
    if not argv or argv[0] in ("-h", "--help", "help"):
        print(__doc__)
        return 0
    if argv[0] == "sync":
        return sync("--check" in argv[1:])
    if argv[0] == "install":
        return install(argv[1:])
    if argv[0] == "status":
        return status()
    print(f"agents: unknown action {argv[0]}\n{__doc__}")
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
