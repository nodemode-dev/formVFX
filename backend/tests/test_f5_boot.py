import concurrent.futures
import json
import socket
import time
import urllib.request
from pathlib import Path

import pytest

from conftest import APP_ID, HEALTH_PATH, LOCAL_HEADER, VERSION_KEY, VERSION_PATH, get_json


def test_engine_boots_on_a_free_port_with_an_empty_data_folder(engine):
    status, body = get_json(engine["base"] + HEALTH_PATH)
    assert status == 200
    assert body["status"] == "ok"
    assert engine["proc"].poll() is None


def test_engine_loads_the_compositing_core(engine):
    _, body = get_json(engine["base"] + HEALTH_PATH)
    assert body["core"] is True, body["error"]


def test_engine_reports_the_repository_version(engine, repo_version):
    status, body = get_json(engine["base"] + VERSION_PATH)
    assert status == 200
    assert str(body[VERSION_KEY]).lstrip("v") == repo_version.lstrip("v")


def test_core_reports_the_repository_version(engine, repo_version):
    _, body = get_json(engine["base"] + VERSION_PATH)
    assert body["core"] == repo_version


def test_engine_uses_the_data_folder_it_was_given(engine):
    assert (engine["data"] / "engine.json").is_file()
    home = engine["data"].parent / "home"
    assert not [p.name for p in home.iterdir() if p.name.lower().startswith(APP_ID.lower())]


def rpc(base, method, params=None, timeout=600):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}).encode()
    req = urllib.request.Request(base + "/api/rpc", data=body, method="POST", headers={**LOCAL_HEADER, "Content-Type": "application/json", "x-formvfx-source": "ui"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        reply = json.loads(r.read().decode("utf-8"))
    assert "error" not in reply, reply
    return reply["result"]


def execute(base, command, **params):
    return rpc(base, "command.execute", {"command": command, "params": params})


def test_health_answers_while_a_long_render_runs(engine, tmp_path):
    base = engine["base"]
    execute(base, "comp.new", name="Long", width=960, height=540, frameRate=30, duration=4.0, background="#203040")
    for i in range(3):
        execute(base, "engine.batch", steps=[{"command": "layer.newSolid", "params": {"name": f"S{i}", "color": "#ff8000", "width": 400, "height": 300}}, {"command": "effect.apply", "params": {"effect": "fx.blur.gaussian", "layers": ["$1.layer"]}}])
        execute(base, "prop.set", layer=f"S{i}", path="effects/#1/blurriness", value=60)
    execute(base, "renderQueue.add", format="png", output=str(tmp_path / "f_[#####].png"))
    with concurrent.futures.ThreadPoolExecutor(1) as pool:
        render = pool.submit(execute, base, "renderQueue.render", wait=True)
        answered = 0
        while not render.done():
            status, body = get_json(base + HEALTH_PATH, timeout=1)
            assert status == 200 and body["status"] == "ok"
            answered += 1
            time.sleep(0.1)
        result = render.result()
    assert answered >= 1, answered
    assert result["results"] and all(r["ok"] for r in result["results"]), result
    assert len(list(tmp_path.glob("f_*.png"))) == 120


def test_a_held_port_is_reported_and_no_second_engine_starts(monkeypatch):
    from app.agent import client

    with socket.socket() as held:
        held.bind(("127.0.0.1", 0))
        held.listen(8)
        port = held.getsockname()[1]
        monkeypatch.delenv("FORMVFX_URL", raising=False)
        monkeypatch.setenv("FORMVFX_PORT", str(port))
        monkeypatch.setattr(client.discovery, "record_path", lambda: Path("/nonexistent/engine.json"))
        monkeypatch.setattr(client, "HEALTH_RETRY_SECONDS", 0.5)
        monkeypatch.setattr(client, "start_engine", lambda: pytest.fail("started a second engine on a held port"))
        assert client.engine_url() is None
    assert f"127.0.0.1:{port}" in client.problem() and "second engine was not started" in client.problem()
