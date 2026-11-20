import hashlib
import importlib.util
import json
import re
import shutil

from conftest import REPO

DOCK = REPO / "frontend" / "src" / "dock"
SOURCE = REPO / "frontend" / "src"


def _agents():
    spec = importlib.util.spec_from_file_location("formvfx_setup_agents", REPO / "setup" / "agents.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _imports(path):
    return set(re.findall(r"(?:from|import)\s*\(?'([^']+)'", path.read_text(encoding="utf-8")))


def test_the_shared_dock_files_match_their_manifest():
    kept = json.loads((DOCK / "MANIFEST.json").read_text(encoding="utf-8"))
    assert kept["module"] == "dock" and kept["version"] == "2.2.1"
    now = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in DOCK.iterdir() if p.is_file() and p.name != "MANIFEST.json"}
    assert kept["files"] == now
    assert f"Module version {kept['version']}." in (DOCK / "CONTRACT.md").read_text(encoding="utf-8")
    assert _agents().dock_problems() == []


def test_agents_sync_names_a_shared_file_edited_in_place(tmp_path, monkeypatch, capsys):
    agents = _agents()
    copy = tmp_path / "frontend" / "src" / "dock"
    shutil.copytree(DOCK, copy)
    monkeypatch.setattr(agents, "ROOT", tmp_path)
    monkeypatch.setattr(agents, "DOCK_FOLDER", copy)
    assert agents.dock_problems() == []
    with open(copy / "dock.ts", "a", encoding="utf-8") as f:
        f.write("\n")
    said = "frontend/src/dock/dock.ts: does not match MANIFEST.json (the shared dock module changes only as a whole new version)"
    assert agents.dock_problems() == [said]
    for check in (True, False):
        assert agents.sync(check, manifest_only=True) == 1
        assert said in capsys.readouterr().out
    (copy / "extra.ts").write_text("export {};\n", encoding="utf-8")
    assert "frontend/src/dock/extra.ts" in " ".join(agents.dock_problems())
    (copy / "MANIFEST.json").unlink()
    assert agents.dock_problems()[0].startswith("frontend/src/dock/MANIFEST.json: missing or unreadable")


LOOKUP = "backend/app/agent/agent_lookup.py"


def test_the_shared_finder_matches_the_hash_recorded_for_its_version():
    agents = _agents()
    version = re.search(r'^VERSION = "([^"]+)"$', (REPO / LOOKUP).read_text(encoding="utf-8"), re.M).group(1)
    assert agents.LOOKUP_HASHES[version] == hashlib.sha256((REPO / LOOKUP).read_bytes()).hexdigest()
    assert agents.lookup_problems() == []


def test_agents_sync_names_the_shared_finder_edited_in_place_or_missing(tmp_path, monkeypatch, capsys):
    agents = _agents()
    copy = tmp_path / LOOKUP
    copy.parent.mkdir(parents=True)
    shutil.copy(REPO / LOOKUP, copy)
    monkeypatch.setattr(agents, "LOOKUP", copy)
    assert agents.lookup_problems() == []
    with open(copy, "a", encoding="utf-8") as f:
        f.write("\n")
    version = re.search(r'^VERSION = "([^"]+)"$', (REPO / LOOKUP).read_text(encoding="utf-8"), re.M).group(1)
    said = f"{copy.as_posix()}: does not match the hash recorded for version {version} (the shared finder changes only as a whole new version)"
    assert agents.lookup_problems() == [said]
    for check in (True, False):
        assert agents.sync(check, manifest_only=True) == 1
        assert said in capsys.readouterr().out
    copy.unlink()
    assert agents.lookup_problems() == [f"{copy.as_posix()}: missing (the shared finder for the agent programs)"]


def test_the_shared_finder_imports_nothing_from_the_app():
    imports = set(re.findall(r"^\s*(?:from|import)\s+([\w.]+)", (REPO / LOOKUP).read_text(encoding="utf-8"), re.M))
    assert imports == {"__future__", "dataclasses", "os", "pathlib", "re", "shutil", "signal", "subprocess", "sys", "threading", "time", "winreg"}


def test_the_shared_dock_imports_nothing_from_the_app():
    outside = sorted({i for p in DOCK.glob("*.ts") for i in _imports(p) if not i.startswith(("./", "@xterm/"))})
    assert outside == [], outside


def test_only_the_lazily_loaded_file_imports_the_emulator():
    assert {p.name for p in DOCK.glob("*.ts") if any(i.startswith("@xterm/") for i in _imports(p))} == {"terminal.ts"}
    dock = (DOCK / "dock.ts").read_text(encoding="utf-8")
    assert re.findall(r"^import (?!type)[^;]*'\./terminal'", dock, re.M) == [] and "import('./terminal')" in dock
    app = [p for p in SOURCE.rglob("*.ts") if DOCK not in p.parents]
    assert [p.name for p in app if any(i.startswith("@xterm/") for i in _imports(p))] == []


def test_the_page_reserves_the_dock_from_the_key_the_adapter_uses_and_reads_the_old_height():
    adapter = (SOURCE / "dock-adapter.ts").read_text(encoding="utf-8")
    page = (REPO / "frontend" / "index.html").read_text(encoding="utf-8")
    key = re.search(r"DOCK_KEY = '([^']+)'", adapter).group(1)
    old = re.search(r"DOCK_OLD_HEIGHT = '([^']+)'", adapter).group(1)
    assert (key, old) == ("formvfx.termdock", "formvfx.termdock.h")
    assert f"localStorage.getItem('{key}')" in page and f"localStorage.getItem('{old}')" in page
    panels = re.search(r"STORE_KEY = '([^']+)'", (SOURCE / "ui" / "dock.ts").read_text(encoding="utf-8")).group(1)
    assert panels != key and not key.startswith(panels + ".")
    for name in ("data-ft-dock", "data-ft-dock-side", "data-ft-dock-h", "data-ft-dock-w", "--ft-dock-size", "--ft-dock-wsize"):
        assert f"'{name}'" in page, name
    assert not (SOURCE / "ui" / "terminal-dock.ts").exists()


def test_the_agent_manifest_says_where_the_dock_can_sit():
    dock = json.loads((REPO / "agents" / "manifest.json").read_text(encoding="utf-8"))["dock"]
    assert dock["present"] is True and dock["positions"] == ["bottom", "right"]
