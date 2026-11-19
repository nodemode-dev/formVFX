import os

import pytest


@pytest.fixture
def programs():
    from app.agent import programs as module

    module._cache.update(at=0.0, value=None)
    yield module
    module._cache.update(at=0.0, value=None)


@pytest.fixture
def stand_ins(tmp_path, monkeypatch):
    scripts = {
        "claude": 'if [ "$1" = "--version" ]; then echo "9.9.9 (stand-in)"; exit 0; fi\nif [ "$1" = "auth" ]; then echo \'{"loggedIn": true}\'; exit 0; fi',
        "codex": 'if [ "$1" = "--version" ]; then echo "codex stand-in 0.1"; exit 0; fi',
        "cursor-agent": 'if [ "$1" = "--version" ]; then echo "cursor stand-in 0.2"; exit 0; fi',
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
    assert set(found) == {"claude", "codex", "cursor"}
    assert found["claude"] == {"installed": True, "version": "9.9.9 (stand-in)", "signedIn": True, "install": programs.INSTALL["claude"],
                               "path": str(stand_ins / "claude"), "program": "claude", "title": "Claude Code"}
    assert found["codex"]["installed"] is True and found["codex"]["version"] == "codex stand-in 0.1" and found["codex"]["signedIn"] is None
    assert found["cursor"]["path"] == str(stand_ins / "cursor-agent") and found["cursor"]["program"] == "cursor-agent"


def test_a_missing_program_comes_with_the_line_that_installs_it(client, programs, monkeypatch):
    monkeypatch.setattr(programs, "find", lambda kind: None)
    found = client.get("/api/terminal/agents").json()
    for kind in ("claude", "codex", "cursor"):
        assert found[kind]["installed"] is False and found[kind]["version"] is None and found[kind]["signedIn"] is None and found[kind]["path"] is None
    assert found["codex"]["install"] == "npm install -g @openai/codex"
    assert found["cursor"]["install"] == "curl https://cursor.com/install -fsS | bash"
    assert "claude.ai/install" in found["claude"]["install"]


@pytest.mark.skipif(os.name == "nt", reason="the stand-ins are shell scripts")
def test_the_answer_is_kept_for_a_minute_unless_a_fresh_one_is_asked(client, programs, stand_ins, monkeypatch):
    assert client.get("/api/terminal/agents").json()["codex"]["installed"] is True
    monkeypatch.setattr(programs, "find", lambda kind: None)
    assert client.get("/api/terminal/agents").json()["codex"]["installed"] is True
    assert client.get("/api/terminal/agents?fresh=1").json()["codex"]["installed"] is False


def test_asking_for_the_status_needs_no_key_and_is_a_read(app):
    assert list(app.openapi()["paths"]["/api/terminal/agents"]) == ["get"]
