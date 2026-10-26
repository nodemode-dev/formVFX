import pytest

from engine_support import Core


@pytest.fixture
def core():
    return Core()


def animated(core, values, interpolation, path="transform/opacity"):
    core.comp(duration=4.0)
    layer = core.solid()["layer"]
    for time, value in values:
        core.run("prop.addKey", layer=layer, path=path, time=time, value=value)
    if interpolation:
        core.run("keys.select", keys=[{"layer": layer, "path": path, "time": t} for t, _ in values])
        core.run("keys.interpolation", interpolation=interpolation)
    return layer


@pytest.mark.parametrize("time,expected", [(0.0, 0.0), (0.5, 25.0), (1.0, 50.0), (1.5, 75.0), (2.0, 100.0)])
def test_linear_keys_interpolate_in_a_straight_line(core, time, expected):
    layer = animated(core, [(0.0, 0), (2.0, 100)], "linear")
    assert core.value(layer, "transform/opacity", time) == pytest.approx(expected, abs=1e-6)


def test_values_outside_the_keys_hold_the_end_keys(core):
    layer = animated(core, [(1.0, 20), (2.0, 80)], "linear")
    assert core.value(layer, "transform/opacity", 0.0) == pytest.approx(20)
    assert core.value(layer, "transform/opacity", 3.5) == pytest.approx(80)


def test_linear_vectors_interpolate_per_component(core):
    layer = animated(core, [(0.0, [0, 0, 0]), (2.0, [200, 100, 40])], "linear", path="transform/position")
    assert core.value(layer, "transform/position", 1.0) == pytest.approx([100, 50, 20], abs=1e-6)


def test_hold_keeps_the_first_value_until_the_next_key(core):
    layer = animated(core, [(0.0, 10), (1.0, 90)], "hold")
    assert core.value(layer, "transform/opacity", 0.0) == pytest.approx(10)
    assert core.value(layer, "transform/opacity", 0.99) == pytest.approx(10)
    assert core.value(layer, "transform/opacity", 1.0) == pytest.approx(90)


def test_hold_and_linear_disagree_between_the_keys(core):
    held = animated(core, [(0.0, 0), (1.0, 100)], "hold")
    assert core.value(held, "transform/opacity", 0.5) == pytest.approx(0)
    linear = animated(core, [(0.0, 0), (1.0, 100)], "linear")
    assert core.value(linear, "transform/opacity", 0.5) == pytest.approx(50)


def test_easy_ease_is_symmetric_and_slow_at_both_ends(core):
    layer = animated(core, [(0.0, 0), (1.0, 100)], None)
    core.run("keys.select", keys=[{"layer": layer, "path": "transform/opacity", "time": t} for t in (0.0, 1.0)])
    core.run("keys.easyEase", which="both")
    at = lambda t: core.value(layer, "transform/opacity", t)
    assert at(0.0) == pytest.approx(0, abs=1e-6)
    assert at(1.0) == pytest.approx(100, abs=1e-6)
    assert at(0.5) == pytest.approx(50, abs=1e-3)
    assert 0 < at(0.1) < 10
    assert at(0.25) < 25
    assert at(0.25) + at(0.75) == pytest.approx(100, abs=1e-3)
    values = [at(i / 20) for i in range(21)]
    assert values == sorted(values)


def test_a_longer_influence_makes_a_slower_start(core):
    layer = animated(core, [(0.0, 0), (1.0, 100)], "bezier")
    core.run("keys.setEase", layer=layer, path="transform/opacity", time=0.0, side="out", influence=33.3, speed=0)
    gentle = core.value(layer, "transform/opacity", 0.25)
    core.run("keys.setEase", layer=layer, path="transform/opacity", time=0.0, side="out", influence=90, speed=0)
    slow = core.value(layer, "transform/opacity", 0.25)
    assert 0 < slow < gentle < 25


def test_keys_report_their_interpolation(core):
    layer = animated(core, [(0.0, 0), (1.0, 100)], "hold")
    keys = core.prop(layer, "transform/opacity")["keys"]
    assert [k["out"] for k in keys] == ["hold", "hold"]
    core.run("keys.select", keys=[{"layer": layer, "path": "transform/opacity", "time": 0.0}])
    core.run("keys.interpolation", interpolation="bezier")
    assert core.prop(layer, "transform/opacity")["keys"][0]["out"] == "bezier"


def test_a_key_moved_with_its_layer_keeps_its_place_in_layer_time(core):
    layer = animated(core, [(0.0, 0), (2.0, 100)], "linear")
    core.run("layer.timing", layers=[layer], delta=1.0)
    assert core.value(layer, "transform/opacity", 2.0) == pytest.approx(50, abs=1e-6)
    assert core.value(layer, "transform/opacity", 3.0) == pytest.approx(100, abs=1e-6)
