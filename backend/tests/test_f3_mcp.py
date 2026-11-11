import json
import os
import subprocess
import sys

from conftest import MCP_PATH, READ_TOOL, REPO, STDIO_CMD, TOKEN_HEADER, inapp_token, tool_catalogue


def initialize(client, headers=None):
    body = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "family-test", "version": "1"}}}
    return client.post(MCP_PATH, json=body, headers=headers or {})


def test_initialize_without_origin_answers(client):
    r = initialize(client)
    assert r.status_code == 200
    result = r.json()["result"]
    assert result["protocolVersion"]
    assert result["serverInfo"]["name"] == "formvfx"
    assert "tools" in result["capabilities"]


def test_initialize_from_a_foreign_origin_is_refused(client):
    assert initialize(client, {"origin": "http://evil.example"}).status_code == 403


def test_tools_list_matches_the_catalogue(client):
    listed = client.post(MCP_PATH, json={"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}).json()["result"]["tools"]
    catalogue = tool_catalogue()
    assert len(listed) == len(catalogue)
    assert sorted(t["name"] for t in listed) == sorted(t["name"] for t in catalogue)
    for tool in listed:
        assert tool["description"].strip()
        assert tool["inputSchema"]["type"] == "object"


def test_read_only_tool_returns_a_valid_result(client):
    name, args = READ_TOOL
    reply = client.post(MCP_PATH, json={"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": name, "arguments": args}}).json()
    result = reply["result"]
    assert not result.get("isError")
    assert result["content"] and all(c["type"] in ("text", "image", "resource", "resource_link") for c in result["content"])


def test_unknown_method_is_a_jsonrpc_error(client):
    reply = client.post(MCP_PATH, json={"jsonrpc": "2.0", "id": 4, "method": "no/such/method", "params": {}}).json()
    assert reply["error"]["code"] == -32601


def test_unknown_tool_is_an_invalid_params_error(client):
    reply = client.post(MCP_PATH, json={"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "no_such_tool", "arguments": {}}}).json()
    assert reply["error"]["code"] == -32602


def test_stdio_relay_finds_the_engine_through_engine_json(engine):
    discovery = engine["data"] / "engine.json"
    assert discovery.is_file()
    assert json.loads(discovery.read_text(encoding="utf-8"))["url"] == engine["base"]
    env = dict(engine["env"])
    env.pop("FORMVFX_PORT", None)
    env["FORMVFX_AUTOSTART"] = "0"
    lines = [
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "family-test", "version": "1"}}},
        {"jsonrpc": "2.0", "method": "notifications/initialized"},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}},
    ]
    proc = subprocess.run(STDIO_CMD, input="".join(json.dumps(m) + "\n" for m in lines), capture_output=True, text=True, timeout=60, env=env)
    replies = {m.get("id"): m for m in (json.loads(l) for l in proc.stdout.splitlines() if l.strip().startswith("{"))}
    assert 2 in replies, f"relay gave no tools/list reply: {proc.stdout[-2000:]} {proc.stderr[-2000:]}"
    assert len(replies[2]["result"]["tools"]) == len(tool_catalogue())


CLI = [sys.executable, str(REPO / "setup" / "fx.py")]


def cli(engine, *args):
    env = dict(engine["env"], FORMVFX_URL=engine["base"], FORMVFX_AUTOSTART="0")
    return subprocess.run([*CLI, *args], capture_output=True, text=True, timeout=120, env=env)


def test_cli_success_says_ok_and_lists_compositions(engine):
    proc = cli(engine, "exec", "comp.list")
    assert proc.returncode == 0, proc.stderr
    out = json.loads(proc.stdout)
    assert out["ok"] is True and isinstance(out["comps"], list)


def test_cli_failure_says_not_ok_with_the_error(engine):
    proc = cli(engine, "exec", "comp.info", '{"comp": "no such comp"}')
    assert proc.returncode == 1
    out = json.loads(proc.stdout)
    assert out["ok"] is False and out["error"]


def test_cli_resolves_relative_render_outputs_from_the_current_folder(tmp_path, monkeypatch):
    from app.agent.cli import absolute_outputs

    monkeypatch.chdir(tmp_path)
    params = absolute_outputs("renderQueue.add", {"output": "plates/f_[#####].png", "comp": "Main"})
    assert params == {"output": str(tmp_path / "plates" / "f_[#####].png"), "comp": "Main"}
    assert absolute_outputs("renderQueue.setOutput", {"path": "/abs/out.mp4"}) == {"path": "/abs/out.mp4"}
    assert absolute_outputs("layer.newText", {"path": "x"}) == {"path": "x"}
    monkeypatch.setenv("FORMVFX_CALLER_DIR", str(tmp_path / "caller"))
    assert absolute_outputs("renderQueue.setOutput", {"path": "out.mp4"}) == {"path": str(tmp_path / "caller" / "out.mp4")}


def test_terminal_panel_shells_and_agents_resolve_relative_paths_from_their_own_folder(tmp_path, monkeypatch):
    from app.agent import claude_code, terminals

    monkeypatch.setenv("FORMVFX_CALLER_DIR", str(tmp_path / "launched-from"))
    shell = terminals.environment("http://127.0.0.1:1")
    assert "FORMVFX_CALLER_DIR" not in shell and shell["FORMVFX_AGENT_NAME"] == "terminal"
    assert "FORMVFX_CALLER_DIR" not in claude_code.child_env(shell)
    assert "FORMVFX_CALLER_DIR" not in claude_code.child_env(dict(os.environ))
    monkeypatch.delenv("FORMVFX_CALLER_DIR")
    folder = tmp_path / "typed-in"
    folder.mkdir()
    probe = "import os, sys; sys.path.insert(0, os.environ['BACKEND']); from app.agent.cli import _here; print(_here('out.png'))"
    out = subprocess.run([sys.executable, "-c", probe], cwd=folder, env={**shell, "BACKEND": str(REPO / "backend")}, capture_output=True, text=True, check=True)
    assert out.stdout.strip() == str(folder.resolve() / "out.png")

def test_the_render_tool_renders_in_the_background_and_reports_the_output(client, tmp_path):
    token = {TOKEN_HEADER: inapp_token()}
    call = lambda name, args: client.post(MCP_PATH, json={"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {"name": name, "arguments": args}}, headers=token).json()["result"]
    assert not call("execute", {"command": "comp.new", "params": {"name": "Tool render", "width": 32, "height": 32, "frameRate": 10, "duration": 0.5}}).get("isError")
    result = call("render", {"format": "png", "output": str(tmp_path / "t_[#####].png")})
    assert not result.get("isError"), result
    body = json.loads(result["content"][0]["text"])
    assert body["result"]["results"][0]["ok"] is True, body
    assert len(list(tmp_path.glob("t_*.png"))) == 5
