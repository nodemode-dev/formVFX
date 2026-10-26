import json

import pytest

from engine_support import Core


@pytest.fixture
def core():
    return Core()


def build(core):
    core.comp("Main", width=96, height=54, fps=24, duration=3.0, background="#203040")
    core.solid("Back", "#336699")
    text = core.run("layer.newText", text="Round trip", name="Title")["layer"]
    core.run("prop.addKey", layer=text, path="transform/position", time=0.0, value=[10, 20, 0])
    core.run("prop.addKey", layer=text, path="transform/position", time=2.0, value=[80, 40, 0])
    core.run("prop.setExpression", layer=text, path="transform/opacity", expression="time * 10")
    core.run("layer.newNull", name="Rig")
    return text


def canonical(core):
    project = json.loads(core.engine.project_json())
    project.pop("savedBy", None)
    return project


def test_saved_project_is_formvfx_json(core, tmp_path):
    build(core)
    path = tmp_path / "a.fvxproj"
    core.run("file.saveAs", path=str(path))
    saved = json.loads(path.read_text(encoding="utf-8"))
    assert saved["format"] == "formvfx.project/1"


def test_open_then_save_again_gives_the_same_file(core, tmp_path):
    build(core)
    first = tmp_path / "first.fvxproj"
    core.run("file.saveAs", path=str(first))
    reopened = Core()
    reopened.run("file.open", path=str(first))
    second = tmp_path / "second.fvxproj"
    reopened.run("file.saveAs", path=str(second))
    assert second.read_bytes() == first.read_bytes()


def test_the_loaded_project_equals_the_original(core, tmp_path):
    build(core)
    before = canonical(core)
    path = tmp_path / "p.fvxproj"
    core.run("file.saveAs", path=str(path))
    reopened = Core()
    reopened.run("file.open", path=str(path))
    assert canonical(reopened) == before


def test_keys_and_expressions_survive_the_round_trip(core, tmp_path):
    text = build(core)
    path = tmp_path / "k.fvxproj"
    core.run("file.saveAs", path=str(path))
    reopened = Core()
    reopened.run("file.open", path=str(path))
    position = reopened.prop(text, "transform/position")
    assert [k["time"] for k in position["keys"]] == [0.0, 2.0]
    assert reopened.value(text, "transform/position", 1.0) == pytest.approx([45, 30, 0], abs=1e-6)
    assert reopened.prop(text, "transform/opacity")["expression"]["text"] == "time * 10"


def test_a_reloaded_project_renders_the_same_frame(core, tmp_path):
    build(core)
    path = tmp_path / "r.fvxproj"
    core.run("file.saveAs", path=str(path))
    reopened = Core()
    reopened.run("file.open", path=str(path))
    assert reopened.frame(1.0)[2] == core.frame(1.0)[2]


def test_a_file_that_is_not_a_project_is_refused_and_changes_nothing(core, tmp_path):
    build(core)
    before = canonical(core)
    bad = tmp_path / "bad.fvxproj"
    bad.write_text("{ not json", encoding="utf-8")
    assert core.fails("file.open", path=str(bad))
    assert canonical(core) == before
