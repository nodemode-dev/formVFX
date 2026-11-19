import json
import os
import stat
import threading
import time

import pytest
from starlette.websockets import WebSocketDisconnect

from conftest import LOCAL_HEADER, MCP_PATH

READS = {"get", "head", "options"}


@pytest.fixture
def key():
    from app.core import window_key

    return window_key.KEY


@pytest.fixture
def keyed(key):
    from app.core import window_key

    return {**LOCAL_HEADER, window_key.HEADER: key}


def test_every_write_route_needs_the_window_key_unless_it_is_on_the_open_list(app, client):
    from app.core import window_key

    answered = []
    for path, ops in app.openapi()["paths"].items():
        for method in ops:
            if method in READS or path in window_key.OPEN:
                continue
            url = path.replace("{", "").replace("}", "")
            r = client.request(method.upper(), url, content="{}", headers={**LOCAL_HEADER, "content-type": "application/json"})
            if r.status_code != 403 or r.json().get("detail") != window_key.REFUSED:
                answered.append(f"{method.upper()} {path} answered {r.status_code}")
    assert answered == []


def test_a_wrong_window_key_is_refused_and_the_right_one_is_taken(client, keyed):
    from app.core import window_key

    assert client.post("/api/chat/reset", headers={**LOCAL_HEADER, window_key.HEADER: "x" * 43}).status_code == 403
    assert client.post("/api/chat/reset", headers=keyed).status_code == 200


def test_the_development_bootstrap_answers_once_then_refuses(client, monkeypatch):
    from app.core import window_key

    monkeypatch.setitem(window_key.STATE, "claimed", False)
    monkeypatch.setitem(window_key.STATE, "secondClaim", False)
    first = client.post("/api/window/key", headers=LOCAL_HEADER)
    assert first.status_code == 200 and first.json()["key"] == window_key.KEY
    assert client.post("/api/window/key", headers=LOCAL_HEADER).status_code == 409
    assert client.get("/api/system").json()["secondWindowClaim"] is True


def test_the_bootstrap_does_not_exist_when_the_shell_reads_the_key(client, monkeypatch):
    from app.core import window_key

    monkeypatch.setenv(window_key.STDOUT_ENV, "1")
    monkeypatch.setitem(window_key.STATE, "claimed", False)
    assert client.post("/api/window/key", headers=LOCAL_HEADER).status_code == 404


def test_the_rpc_route_never_skips_consent_on_a_header_alone(client):
    body = {"jsonrpc": "2.0", "id": 3, "method": "command.execute", "params": {"command": "comp.new", "params": {"name": "Claimed"}}}
    out = client.post("/api/rpc", json=body, headers={**LOCAL_HEADER, "x-formvfx-source": "ui"}).json()
    assert out["error"]["code"] == -32003


def test_a_live_client_without_the_key_is_not_a_window_and_cannot_change_anything(client):
    from app.engine.host import host

    with client.websocket_connect("/api/live?kind=ui") as ws:
        ws.receive_text()
        ws.send_text(json.dumps({"method": "window", "params": {"key": "not-the-key"}}))
        ws.send_text(json.dumps({"jsonrpc": "2.0", "id": 4, "method": "command.execute", "params": {"command": "comp.new", "params": {}}}))
        reply = json.loads(ws.receive_text())
        assert reply["id"] == 4 and reply["error"]["code"] == -32003
        assert host.bus.windows() == []


def test_only_the_asked_window_can_answer_a_consent_prompt(client, key):
    from app.agent import consent
    from app.engine.host import host

    os.environ.pop("FORMVFX_AGENTS", None)
    consent.forget_all()
    with client.websocket_connect("/api/live") as other, client.websocket_connect("/api/live") as window:
        other.receive_text()
        window.receive_text()
        window.send_text(json.dumps({"method": "window", "params": {"key": key}}))
        deadline = time.monotonic() + 5
        while not host.bus.windows() and time.monotonic() < deadline:
            time.sleep(0.05)
        assert len(host.bus.windows()) == 1
        result = {}
        body = {"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "execute", "arguments": {"command": "comp.new", "params": {"name": "Asked"}}}}
        worker = threading.Thread(target=lambda: result.update(reply=client.post(MCP_PATH, json=body, headers={"x-formvfx-source": "mcp:asker"}).json()))
        worker.start()
        while True:
            message = json.loads(window.receive_text())
            if message.get("type") == "consent":
                break
        other.send_text(json.dumps({"method": "reply", "params": {"requestId": message["requestId"], "value": {"allow": True, "remember": "always"}}}))
        time.sleep(0.4)
        assert worker.is_alive()
        assert consent.state()["always"] == []
        window.send_text(json.dumps({"method": "reply", "params": {"requestId": message["requestId"], "value": {"allow": False}}}))
        worker.join(10)
    assert "declined" in result["reply"]["result"]["content"][0]["text"]
    consent.forget_all()
    consent._declined.clear()


def test_a_terminal_socket_without_the_key_is_closed(client, keyed):
    from app.agent import terminals

    session = client.post("/api/terminal/open", headers=keyed, json={"cols": 80, "rows": 24}).json()
    try:
        with pytest.raises(WebSocketDisconnect) as closed:
            with client.websocket_connect(f"/api/terminal/{session['id']}/ws") as ws:
                ws.send_text(json.dumps({"type": "input", "data": "echo hi\r"}))
                ws.receive_text()
        assert closed.value.code == 4403
    finally:
        terminals.close(session["id"])


def test_a_plain_terminal_tab_carries_no_agent_token(monkeypatch):
    from app.agent import terminals

    monkeypatch.setenv("FORMVFX_AGENT_TOKEN", "inherited")
    assert "FORMVFX_AGENT_TOKEN" not in terminals.environment("http://127.0.0.1:1")


def test_a_claude_code_tab_gets_its_own_token_in_a_private_file_that_dies_with_the_tab(tmp_path, monkeypatch):
    from app.agent import consent, terminals

    fake = tmp_path / "claude"
    fake.write_text("#!/bin/sh\nsleep 30\n")
    fake.chmod(0o755)
    monkeypatch.setenv("FORMVFX_CLAUDE_BIN", str(fake))
    session = terminals.open_claude("http://127.0.0.1:1", None, None, 80, 24)
    try:
        assert session.argv[-1].count("--mcp-config") == 1
        config_file = session.argv[-1].split("--mcp-config ", 1)[1].split(" ", 1)[0]
        token = json.loads(open(config_file, encoding="utf-8").read())["mcpServers"]["formvfx"]["headers"]["x-formvfx-agent-token"]
        assert stat.S_IMODE(os.stat(config_file).st_mode) == 0o600
        assert token not in " ".join(session.argv) and token not in session.env.values()
        assert consent.in_app(token)
    finally:
        terminals.close(session.id)
    assert not os.path.exists(config_file)
    assert not consent.in_app(token)


class _Keychain:
    def __init__(self):
        import keyring.backend

        class Store(keyring.backend.KeyringBackend):
            priority = 1
            values: dict = {}

            def get_password(self, service, user):
                return self.values.get((service, user))

            def set_password(self, service, user, password):
                self.values[(service, user)] = password

            def delete_password(self, service, user):
                self.values.pop((service, user), None)

        self.backend = Store()


@pytest.fixture
def sealed(monkeypatch):
    import keyring

    from app.agent import consent
    from app.core import seal

    before = keyring.get_keyring()
    keyring.set_keyring(_Keychain().backend)
    seal.reset()
    consent.forget_all()
    yield consent
    consent.forget_all()
    keyring.set_keyring(before)
    seal.reset()


def test_a_hand_edit_of_the_allow_list_is_caught(sealed):
    import asyncio

    from app.core import config

    sealed._remember("alpha")
    assert sealed.state()["always"] == ["alpha"] and sealed.state()["seal"] == "ok"
    store = config.data_dir() / "agents.json"
    data = json.loads(store.read_text(encoding="utf-8"))
    data["allowed"].append("mallory")
    store.write_text(json.dumps(data), encoding="utf-8")
    state = sealed.state()
    assert state["always"] == [] and state["sealBroken"] is True
    assert asyncio.run(sealed.gate("mcp:mallory", None, True)) is not None


def test_with_no_keychain_the_allow_list_stands_and_the_seal_is_unavailable(monkeypatch):
    import keyring
    import keyring.backends.fail

    from app.agent import consent
    from app.core import seal

    before = keyring.get_keyring()
    keyring.set_keyring(keyring.backends.fail.Keyring())
    seal.reset()
    consent.forget_all()
    try:
        consent._remember("beta")
        state = consent.state()
        assert state["always"] == ["beta"] and state["seal"] == "unavailable" and state["sealBroken"] is False
    finally:
        consent.forget_all()
        keyring.set_keyring(before)
        seal.reset()
