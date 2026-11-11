import pytest

from engine_support import Core

BAD = ["#zz0000", "red", [1], {"r": 1}, 7]


@pytest.fixture
def core():
    return Core()


@pytest.mark.parametrize("colour", BAD)
def test_a_solid_with_a_colour_that_is_not_one_is_refused_and_adds_nothing(core, colour):
    core.comp()
    assert "expected a colour" in core.fails("layer.newSolid", color=colour)
    assert core.solid(color="#ff0000")["index"] == 1


@pytest.mark.parametrize("colour", BAD)
def test_an_imported_or_replacing_solid_with_a_bad_colour_is_refused(core, colour):
    assert "expected a colour" in core.fails("file.importSolid", color=colour)
    item = core.run("file.importSolid", color="#00ff00")["item"]
    assert "expected a colour" in core.fails("file.replaceWithSolid", item=item, color=colour)


def test_a_solid_with_no_colour_is_still_grey(core):
    core.comp()
    assert core.run("layer.newSolid")["index"] == 1
    assert core.run("file.importSolid", width=8, height=8)["item"]
