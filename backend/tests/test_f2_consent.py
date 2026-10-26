import os

import pytest

from conftest import CHANGING_TOOL, LOCAL_HEADER, MCP_PATH, POLICY_ENV, READ_TOOL, TOKEN_HEADER, consent_refusals, inapp_token, set_policy


def rpc(client, method, params=None, headers=None, rid=1):
    r = client.post(MCP_PATH, json={"jsonrpc": "2.0", "id": rid, "method": method, "params": params or {}}, headers=headers or {})
    assert r.status_code == 200, r.text
    return r.json()


def call(client, tool, headers=None, agent="family-test-agent"):
    name, args = tool
    hdrs = {"x-formvfx-source": f"mcp:{agent}", **(headers or {})}
    return rpc(client, "tools/call", {"name": name, "arguments": args}, hdrs, rid=2)


def text_of(reply):
    result = reply.get("result") or {}
    return " ".join(c.get("text", "") for c in result.get("content", []) if isinstance(c, dict))


def refused(reply, kind):
    return consent_refusals()[kind] in text_of(reply)


class Window:
    def __init__(self, answer):
        self.answer = answer
        self.asked = []

    def windows(self):
        return [object()]

    async def ask_window(self, message, timeout=0):
        self.asked.append(message)
        return self.answer


@pytest.fixture
def window(monkeypatch):
    from app.engine.host import host

    def attach(answer):
        fake = Window(answer)
        monkeypatch.setattr(host.bus, "windows", fake.windows)
        monkeypatch.setattr(host.bus, "ask_window", fake.ask_window)
        return fake

    return attach


@pytest.fixture(autouse=True)
def policy_reset():
    from app.agent import consent

    os.environ.pop(POLICY_ENV, None)
    consent.forget_all()
    consent._declined.clear()
    yield
    os.environ.pop(POLICY_ENV, None)
    consent.forget_all()
    consent._declined.clear()


def test_read_only_tool_never_asks(client):
    reply = call(client, READ_TOOL)
    assert "error" not in reply
    assert not refused(reply, "no_window")
    assert not reply["result"].get("isError")


def test_a_query_command_through_execute_never_asks(client):
    reply = call(client, ("execute", {"command": "app.capabilities", "params": {}}))
    assert not refused(reply, "no_window")


def test_changing_tool_with_no_window_is_refused(client):
    reply = call(client, CHANGING_TOOL)
    assert refused(reply, "no_window")
    assert reply["result"].get("isError")


def test_deny_policy_refuses_a_changing_tool(client):
    set_policy("deny")
    reply = call(client, CHANGING_TOOL)
    assert refused(reply, "deny")
    assert reply["result"].get("isError")


def test_deny_policy_still_lets_a_read_only_tool_through(client):
    set_policy("deny")
    assert not call(client, READ_TOOL)["result"].get("isError")


def test_allow_policy_lets_a_changing_tool_through(client):
    set_policy("allow")
    reply = call(client, CHANGING_TOOL)
    assert not refused(reply, "no_window")
    assert not reply["result"].get("isError")


def test_unknown_policy_value_falls_back_to_ask(client):
    set_policy("whatever")
    assert refused(call(client, CHANGING_TOOL), "no_window")


def test_inapp_token_skips_the_ask(client):
    reply = call(client, CHANGING_TOOL, {TOKEN_HEADER: inapp_token()})
    assert not refused(reply, "no_window")
    assert not reply["result"].get("isError")


def test_inapp_token_skips_a_deny_policy(client):
    set_policy("deny")
    assert not refused(call(client, CHANGING_TOOL, {TOKEN_HEADER: inapp_token()}), "deny")


@pytest.mark.parametrize("token", ["", "wrong-token", "x" * 64])
def test_wrong_inapp_token_does_not_skip_the_ask(client, token):
    assert refused(call(client, CHANGING_TOOL, {TOKEN_HEADER: token}), "no_window")


def test_rpc_route_refuses_a_changing_command_with_no_window(client):
    body = {"jsonrpc": "2.0", "id": 5, "method": "command.execute", "params": {"command": "comp.new", "params": {"name": "X"}}}
    out = client.post("/api/rpc", json=body, headers={**LOCAL_HEADER, "x-formvfx-source": "cli"}).json()
    assert out["error"]["code"] == -32003
    assert consent_refusals()["no_window"] in out["error"]["message"]


def test_rpc_route_lets_the_window_itself_through(client):
    body = {"jsonrpc": "2.0", "id": 6, "method": "command.execute", "params": {"command": "app.capabilities", "params": {}}}
    out = client.post("/api/rpc", json=body, headers={**LOCAL_HEADER, "x-formvfx-source": "ui"}).json()
    assert "error" not in out


def test_a_decline_blocks_the_agent_for_the_cooldown(client, window):
    fake = window({"allow": False})
    first = call(client, CHANGING_TOOL, agent="declined-agent")
    assert first["result"].get("isError") and len(fake.asked) == 1
    fake.answer = {"allow": True}
    second = call(client, CHANGING_TOOL, agent="declined-agent")
    assert second["result"].get("isError")
    assert "declined" in text_of(second)
    assert len(fake.asked) == 1


def test_a_decline_does_not_block_another_agent(client, window):
    window({"allow": False})
    call(client, CHANGING_TOOL, agent="declined-agent")
    window({"allow": True})
    assert not call(client, CHANGING_TOOL, agent="other-agent")["result"].get("isError")


def test_an_allowed_agent_is_not_asked_again_this_session(client, window):
    fake = window({"allow": True})
    assert not call(client, CHANGING_TOOL, agent="trusted-agent")["result"].get("isError")
    assert not call(client, CHANGING_TOOL, agent="trusted-agent")["result"].get("isError")
    assert len(fake.asked) == 1


def test_always_is_remembered_in_the_data_folder(client, window, sandbox):
    window({"allow": True, "remember": "always"})
    call(client, CHANGING_TOOL, agent="remembered-agent")
    stored = (sandbox / "data" / "agents.json").read_text(encoding="utf-8")
    assert "remembered-agent" in stored
