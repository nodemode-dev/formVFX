import pytest

from engine_support import Core

OPACITY = "transform/opacity"
POSITION = "transform/position"


@pytest.fixture
def core():
    core = Core()
    core.comp(duration=4.0)
    return core


def expr(core, layer, path, text):
    core.run("prop.setExpression", layer=layer, path=path, expression=text)


@pytest.mark.parametrize("time,expected", [(0.0, 0.0), (0.5, 5.0), (2.0, 20.0)])
def test_time_expression_evaluates_to_known_values(core, time, expected):
    layer = core.solid()["layer"]
    expr(core, layer, OPACITY, "time * 10")
    assert core.value(layer, OPACITY, time) == pytest.approx(expected, abs=1e-6)


@pytest.mark.parametrize("time,expected", [(0.0, 0.0), (0.25, 25.0), (1.0, 100.0), (3.0, 100.0)])
def test_linear_range_mapping(core, time, expected):
    layer = core.solid()["layer"]
    expr(core, layer, OPACITY, "linear(time, 0, 1, 0, 100)")
    assert core.value(layer, OPACITY, time) == pytest.approx(expected, abs=1e-6)


def test_value_keyword_adds_to_the_stored_value(core):
    layer = core.solid()["layer"]
    core.run("prop.set", layer=layer, path=OPACITY, value=40)
    expr(core, layer, OPACITY, "value + 5")
    assert core.value(layer, OPACITY, 1.0) == pytest.approx(45)


def test_vector_expression_returns_all_components(core):
    layer = core.solid()["layer"]
    expr(core, layer, POSITION, "[100, 200 + time * 10, 0]")
    assert core.value(layer, POSITION, 2.0) == pytest.approx([100, 220, 0], abs=1e-6)


def test_loop_out_repeats_the_keyed_animation(core):
    layer = core.solid()["layer"]
    core.run("prop.addKey", layer=layer, path=OPACITY, time=0.0, value=0)
    core.run("prop.addKey", layer=layer, path=OPACITY, time=1.0, value=100)
    expr(core, layer, OPACITY, 'loopOut("cycle")')
    assert core.value(layer, OPACITY, 1.5) == pytest.approx(50, abs=1e-6)
    assert core.value(layer, OPACITY, 2.25) == pytest.approx(25, abs=1e-6)


def test_expression_follows_another_layer(core):
    leader = core.solid("Leader")["layer"]
    follower = core.solid("Follower")["layer"]
    core.run("prop.set", layer=leader, path=OPACITY, value=70)
    expr(core, follower, OPACITY, 'thisComp.layer("Leader").transform.opacity')
    assert core.value(follower, OPACITY, 0.0) == pytest.approx(70)


def test_wiggle_is_repeatable_and_bounded(core):
    layer = core.solid()["layer"]
    core.run("prop.set", layer=layer, path=OPACITY, value=50)
    expr(core, layer, OPACITY, "wiggle(2, 10)")
    first = [core.value(layer, OPACITY, t / 4) for t in range(12)]
    second = [core.value(layer, OPACITY, t / 4) for t in range(12)]
    assert first == second
    assert len(set(round(v, 6) for v in first)) > 1
    assert all(abs(v - 50) <= 40 for v in first)


def test_a_broken_expression_is_listed_and_the_static_value_stays(core):
    layer = core.solid()["layer"]
    core.run("prop.set", layer=layer, path=OPACITY, value=33)
    expr(core, layer, OPACITY, "time * (")
    assert core.value(layer, OPACITY, 1.0) == pytest.approx(33)
    errors = core.run("expr.errors")
    assert errors["count"] == 1
    assert errors["errors"][0]["layer"] == layer


def test_a_reference_to_a_missing_layer_fails_safely(core):
    layer = core.solid()["layer"]
    core.run("prop.set", layer=layer, path=OPACITY, value=61)
    expr(core, layer, OPACITY, 'thisComp.layer("Nobody").transform.opacity')
    assert core.value(layer, OPACITY, 0.5) == pytest.approx(61)
    assert core.run("expr.errors")["count"] == 1


@pytest.mark.parametrize("text", ["while (true) {}", "function f() { return f(); } f()"])
def test_a_runaway_expression_ends_with_an_error(core, text):
    layer = core.solid()["layer"]
    core.run("prop.set", layer=layer, path=OPACITY, value=12)
    expr(core, layer, OPACITY, text)
    assert core.value(layer, OPACITY, 0.5) == pytest.approx(12)
    assert core.run("expr.errors")["count"] == 1


def test_the_engine_still_works_after_a_failing_expression(core):
    layer = core.solid()["layer"]
    expr(core, layer, OPACITY, "nonsense(")
    other = core.solid("Other")["layer"]
    expr(core, other, OPACITY, "time * 10")
    assert core.value(other, OPACITY, 1.0) == pytest.approx(10)
    assert core.run("expr.errors")["count"] == 1


def test_clearing_an_expression_restores_the_stored_value(core):
    layer = core.solid()["layer"]
    core.run("prop.set", layer=layer, path=OPACITY, value=80)
    expr(core, layer, OPACITY, "time * 10")
    expr(core, layer, OPACITY, "")
    assert core.value(layer, OPACITY, 2.0) == pytest.approx(80)
    assert core.run("expr.errors")["count"] == 0
