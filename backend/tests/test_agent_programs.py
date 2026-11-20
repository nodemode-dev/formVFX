import os

import pytest


@pytest.fixture
def programs():
    from app.agent import programs as module

    module._cache.update(at=0.0, value=None)
    module.agent_lookup.forget()
    yield module
    module._cache.update(at=0.0, value=None)
    module.agent_lookup.forget()


def _nothing_found(programs, monkeypatch):
    lookup = programs.agent_lookup
    monkeypatch.setattr(programs.claude_code, "located", lambda kind, fresh=False: lookup.Found(kind, lookup.PROGRAMS[kind]["title"], looked=["PATH (empty)"]))


@pytest.fixture
def stand_ins(tmp_path, monkeypatch):
    scripts = {
        "claude": 'if [ "$1" = "--version" ]; then echo "9.9.9 (stand-in)"; exit 0; fi\nif [ "$1" = "auth" ]; then echo \'{"loggedIn": true}\'; exit 0; fi',
        "codex": 'if [ "$1" = "--version" ]; then echo "codex stand-in 0.1"; exit 0; fi',
        "cursor-agent": 'if [ "$1" = "--version" ]; then echo "cursor stand-in 0.2"; exit 0; fi',
        "agy": 'if [ "$1" = "--version" ]; then echo "agy stand-in 0.3"; exit 0; fi',
    }
    for name, body in scripts.items():
        script = tmp_path / name
        script.write_text(f"#!/bin/sh\n{body}\n")
        script.chmod(0o755)
    monkeypatch.setenv("PATH", f"{tmp_path}{os.pathsep}{os.environ.get('PATH', '')}")
    monkeypatch.delenv("ANTHROPIC_API_KEY", raising=False)
    monkeypatch.delenv("CLAUDE_CODE_OAUTH_TOKEN", raising=False)
    monkeypatch.delenv("FORMVFX_CLAUDE_BIN", raising=False)
    return tmp_path


@pytest.mark.skipif(os.name == "nt", reason="the stand-ins are shell scripts")
def test_one_call_says_which_agent_programs_are_found(client, programs, stand_ins):
    found = client.get("/api/terminal/agents").json()
    assert set(found) == {"claude", "codex", "cursor", "antigravity"}
    where = str(stand_ins / "claude")
    assert found["claude"] == {"installed": True, "version": "9.9.9", "signedIn": True, "install": programs.INSTALL["claude"],
                               "path": where, "program": "claude", "title": "Claude Code", "finding": "runs", "how": "path",
                               "looked": [], "error": "", "said": f"Claude Code is at {where} (the engine's PATH)."}
    assert found["codex"]["installed"] is True and found["codex"]["version"] == "0.1" and found["codex"]["signedIn"] is None
    assert found["cursor"]["path"] == str(stand_ins / "cursor-agent") and found["cursor"]["program"] == "cursor-agent"
    assert found["antigravity"]["path"] == str(stand_ins / "agy") and found["antigravity"]["program"] == "agy"
    assert found["antigravity"]["title"] == "Antigravity" and found["antigravity"]["finding"] == "runs" and found["antigravity"]["signedIn"] is None


def test_a_missing_program_comes_with_the_line_that_installs_it(client, programs, monkeypatch):
    _nothing_found(programs, monkeypatch)
    found = client.get("/api/terminal/agents").json()
    for kind in ("claude", "codex", "cursor", "antigravity"):
        assert found[kind]["installed"] is False and found[kind]["version"] is None and found[kind]["signedIn"] is None and found[kind]["path"] is None
        assert found[kind]["finding"] == "missing" and found[kind]["install"] in found[kind]["said"]
    assert found["codex"]["install"] == "npm install -g @openai/codex"
    assert found["cursor"]["install"] == "curl https://cursor.com/install -fsS | bash"
    assert "claude.ai/install" in found["claude"]["install"]
    assert found["antigravity"]["install"] == "curl -fsSL https://antigravity.google/cli/install.sh | bash"
    assert found["antigravity"]["said"].startswith("Antigravity was not found. Looked in: PATH (empty). Install it with: ")


@pytest.mark.skipif(os.name == "nt", reason="the stand-ins are shell scripts")
def test_the_answer_is_kept_for_a_minute_unless_a_fresh_one_is_asked(client, programs, stand_ins, monkeypatch):
    assert client.get("/api/terminal/agents").json()["codex"]["installed"] is True
    _nothing_found(programs, monkeypatch)
    assert client.get("/api/terminal/agents").json()["codex"]["installed"] is True
    assert client.get("/api/terminal/agents?fresh=1").json()["codex"]["installed"] is False


def test_asking_for_the_status_needs_no_key_and_is_a_read(app):
    assert list(app.openapi()["paths"]["/api/terminal/agents"]) == ["get"]


def test_connecting_antigravity_writes_one_entry_and_keeps_the_rest(tmp_path, monkeypatch):
    import json

    from app.agent import connect

    monkeypatch.setenv("HOME", str(tmp_path))
    cfg = tmp_path / ".gemini" / "config" / "mcp_config.json"
    cfg.parent.mkdir(parents=True)
    cfg.write_text(json.dumps({"mcpServers": {"other": {"command": "x"}}, "keep": 1}), encoding="utf-8")
    assert connect.install(["antigravity"], dry=True) == ["Antigravity:", f"  MCP server formvfx -> {cfg}"]
    assert json.loads(cfg.read_text(encoding="utf-8")) == {"mcpServers": {"other": {"command": "x"}}, "keep": 1}
    assert connect.install(["antigravity"]) == ["Antigravity:", f"  MCP server formvfx -> {cfg}"]
    now = json.loads(cfg.read_text(encoding="utf-8"))
    assert now["keep"] == 1 and now["mcpServers"]["other"] == {"command": "x"}
    assert now["mcpServers"]["formvfx"] == {"command": connect.launch(), "args": ["mcp"]}
    assert connect.install(["antigravity"]) == ["Antigravity:", f"  MCP server formvfx already in {cfg}"]
    now["mcpServers"]["formvfx"] = {"command": "/elsewhere/tool", "args": ["serve"]}
    cfg.write_text(json.dumps(now), encoding="utf-8")
    assert connect.install(["antigravity"])[1].endswith("has a server named formvfx that formvfx did not write; left alone")
    assert json.loads(cfg.read_text(encoding="utf-8"))["mcpServers"]["formvfx"] == {"command": "/elsewhere/tool", "args": ["serve"]}
    assert connect.install(["nope"]) == ["unknown client nope (claude, codex, cursor or antigravity)"]
