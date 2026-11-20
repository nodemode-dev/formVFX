import json
import subprocess
import sys

from conftest import REPO, tool_catalogue


def test_the_committed_agent_manifest_is_current():
    done = subprocess.run([sys.executable, "-B", str(REPO / "setup" / "agents.py"), "sync", "--check", "--manifest-only"], capture_output=True, text=True)
    assert done.returncode == 0, done.stdout + done.stderr


def test_the_manifest_lists_every_tool_the_server_lists():
    manifest = json.loads((REPO / "agents" / "manifest.json").read_text(encoding="utf-8"))
    names = [t["name"] for t in tool_catalogue()]
    assert manifest["mcp"]["tools"] == names
    assert set(manifest["mcp"]["read_only"]) | set(manifest["mcp"]["approval_gated"]) == set(names)
    assert not set(manifest["mcp"]["read_only"]) & set(manifest["mcp"]["approval_gated"])
