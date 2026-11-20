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


def test_allow_policy_no_longer_skips_the_ask(client):
    set_policy("allow")
    reply = call(client, CHANGING_TOOL)
    assert refused(reply, "no_window")
    assert reply["result"].get("isError")


def test_unknown_policy_value_falls_back_to_ask(client):
    set_policy("whatever")
    assert refused(call(client, CHANGING_TOOL), "no_window")


def test_inapp_token_skips_the_ask(client):
    reply = call(client, CHANGING_TOOL, {TOKEN_HEADER: inapp_token()})
    assert not refused(reply, "no_window")
    assert not reply["result"].get("isError")


def test_a_deny_policy_stops_the_apps_own_tabs_too(client):
    set_policy("deny")
    token = {TOKEN_HEADER: inapp_token()}
    reply = call(client, CHANGING_TOOL, token)
    assert refused(reply, "deny") and reply["result"].get("isError")
    assert not refused(call(client, READ_TOOL, token), "deny")


@pytest.mark.parametrize("token", ["", "wrong-token", "x" * 64])
def test_wrong_inapp_token_does_not_skip_the_ask(client, token):
    assert refused(call(client, CHANGING_TOOL, {TOKEN_HEADER: token}), "no_window")


def test_rpc_route_refuses_a_changing_command_with_no_window(client):
    body = {"jsonrpc": "2.0", "id": 5, "method": "command.execute", "params": {"command": "comp.new", "params": {"name": "X"}}}
    out = client.post("/api/rpc", json=body, headers={**LOCAL_HEADER, "x-formvfx-source": "cli"}).json()
    assert out["error"]["code"] == -32003
    assert consent_refusals()["no_window"] in out["error"]["message"]


def test_rpc_route_lets_the_window_itself_through(client):
    from app.core import window_key

    body = {"jsonrpc": "2.0", "id": 6, "method": "command.execute", "params": {"command": "comp.new", "params": {"name": "Window"}}}
    out = client.post("/api/rpc", json=body, headers={**LOCAL_HEADER, window_key.HEADER: window_key.KEY}).json()
    assert "error" not in out


def test_rpc_route_does_not_take_a_window_claim_without_the_key(client):
    body = {"jsonrpc": "2.0", "id": 7, "method": "command.execute", "params": {"command": "comp.new", "params": {"name": "Claimed"}}}
    out = client.post("/api/rpc", json=body, headers={**LOCAL_HEADER, "x-formvfx-source": "ui"}).json()
    assert out["error"]["code"] == -32003
    assert consent_refusals()["no_window"] in out["error"]["message"]


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


def passed_the_gate(reply):
    text = text_of(reply)
    return not any(said in text for said in (*consent_refusals().values(), "declined", "Nobody answered"))


def test_allow_once_runs_that_call_and_the_next_call_asks_again(client, window):
    from app.agent import consent

    fake = window({"allow": True, "remember": "once"})
    assert passed_the_gate(call(client, CHANGING_TOOL, agent="once-agent"))
    assert passed_the_gate(call(client, CHANGING_TOOL, agent="once-agent"))
    assert len(fake.asked) == 2
    assert consent.state()["session"] == [] and consent.state()["always"] == []


def test_the_question_says_who_asks_for_what_and_which_answers_stand(client, window):
    import time

    from app.agent import consent

    fake = window({"allow": False})
    before = time.time() * 1000
    call(client, CHANGING_TOOL, agent="Named Agent")
    asked = fake.asked[0]
    assert asked["type"] == "consent" and asked["client"] == "mcp:Named Agent" and asked["detail"] == "execute"
    assert isinstance(asked["requestId"], int)
    assert asked["agent"] == "Named Agent" and asked["agentKey"] == "named agent"
    assert asked["what"] == "Run the command comp.new (tool execute)"
    assert asked["answers"] == ["deny", "once", "session", "always"] and asked["scope"] == "agent"
    assert asked["timeoutSec"] == consent.ASK_SECONDS
    assert before + consent.ASK_SECONDS * 1000 - 1000 <= asked["expiresAt"] <= time.time() * 1000 + consent.ASK_SECONDS * 1000


def test_the_question_from_the_command_line_names_it_in_plain_words(client, window):
    fake = window({"allow": True, "remember": "once"})
    body = {"jsonrpc": "2.0", "id": 8, "method": "command.execute", "params": {"command": "comp.new", "params": {"name": "X"}}}
    client.post("/api/rpc", json=body, headers={**LOCAL_HEADER, "x-formvfx-source": "cli"})
    asked = fake.asked[0]
    assert asked["agent"] == "The formvfx command line" and asked["agentKey"] == "cli"
    assert asked["what"] == "Run the command comp.new" and asked["detail"] == ""


def test_the_state_says_which_answers_the_engine_takes(client):
    state = client.get("/api/agents/consent").json()
    assert state["answers"] == ["deny", "once", "session", "always"] and state["scope"] == "agent" and state["timeoutSec"] == 180


def test_a_window_that_opens_while_a_question_waits_is_asked_too_and_may_answer():
    import asyncio

    from app.engine.bus import Bus

    async def run():
        bus = Bus()
        first = bus.join("ui")
        asking = asyncio.ensure_future(bus.ask_window({"type": "consent", "requestId": bus.ticket(), "agent": "Late"}, timeout=5))
        await asyncio.sleep(0)
        sent = first.queue.get_nowait()
        late = bus.join("watch")
        assert bus.answer(sent["requestId"], {"allow": True}, late) is False
        late.kind = "ui"
        assert bus.answer(sent["requestId"], {"allow": True}, late) is False
        assert bus.admit(late) == 1 and late.queue.get_nowait() == sent
        assert bus.admit(late) == 0
        other = asyncio.ensure_future(bus.ask_window({"type": "uiRequest", "what": "state"}, timeout=5))
        await asyncio.sleep(0)
        fresh = bus.join("ui")
        assert bus.admit(fresh) == 1 and fresh.queue.get_nowait() == sent
        assert bus.answer(sent["requestId"], {"allow": True, "remember": "once"}, late) is True
        assert await asking == {"allow": True, "remember": "once"}
        other.cancel()
        await asyncio.gather(other, return_exceptions=True)
        assert bus.admit(bus.join("ui")) == 0

    asyncio.run(run())


def test_a_caller_that_hangs_up_ends_its_question_and_is_not_barred(monkeypatch):
    import asyncio

    from app.agent import consent
    from app.engine.bus import Bus
    from app.engine.host import host

    async def run():
        bus = Bus()
        bus.bind(asyncio.get_running_loop())
        window = bus.join("ui")
        monkeypatch.setattr(host, "bus", bus)
        monkeypatch.setattr(consent, "HANGUP_POLL", 0.01)
        gone = {"now": False}

        async def hung_up():
            return gone["now"]

        waiting = asyncio.ensure_future(consent.gate("mcp:Hang Up", None, True, detail="execute", what="Use the tool execute", gone=hung_up))
        await asyncio.sleep(0.05)
        asked = window.queue.get_nowait()
        assert asked["type"] == "consent" and not waiting.done()
        gone["now"] = True
        assert await asyncio.wait_for(waiting, 2) == consent.GONE
        assert window.queue.get_nowait() == {"type": "consentClosed", "requestId": asked["requestId"], "reason": "cancelled", "answer": None, "agentKey": "hang up"}
        assert bus.admit(bus.join("ui")) == 0
        assert consent._declined == {} and consent.state()["session"] == []

    asyncio.run(run())
