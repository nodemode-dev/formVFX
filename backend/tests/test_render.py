import hashlib
import json
import shutil
import struct
import subprocess
import time

import pytest

from engine_support import Core

FPS = 10
SECONDS = 1.0
FRAMES = 10
WIDTH, HEIGHT = 64, 48

needs_ffmpeg = pytest.mark.skipif(not (shutil.which("ffmpeg") and shutil.which("ffprobe")), reason="ffmpeg and ffprobe are not installed")


@pytest.fixture
def core():
    return Core()


def build(core):
    core.comp(width=WIDTH, height=HEIGHT, fps=FPS, duration=SECONDS, background="#102030")
    core.solid("Red", "#ff0000", width=32, height=24)
    box = core.run("layer.newShape", kind="rect", name="Box", size=[20, 20], fill="#00ff00", position=[10, 10])["layer"]
    core.run("prop.addKey", layer=box, path="transform/position", time=0.0, value=[10, 10, 0])
    core.run("prop.addKey", layer=box, path="transform/position", time=SECONDS, value=[50, 30, 0])
    return box


def digest(frame):
    return hashlib.sha256(frame[2]).hexdigest()


def pixel(frame, x, y):
    width, _, data = frame
    i = (y * width + x) * 4
    return tuple(data[i:i + 4])


def probe(path, *entries):
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-count_frames", "-select_streams", "v:0", "-show_entries", "stream=" + ",".join(entries) + ":format=duration", "-of", "json", str(path)],
        capture_output=True, text=True, check=True,
    ).stdout
    return json.loads(out)


def test_a_frame_has_the_composition_size(core):
    build(core)
    width, height, data = core.frame(0.0)
    assert (width, height) == (WIDTH, HEIGHT)
    assert len(data) == WIDTH * HEIGHT * 4


def test_a_solid_renders_its_colour_and_the_background_shows_around_it(core):
    core.comp(width=WIDTH, height=HEIGHT, fps=FPS, duration=SECONDS, background="#000000")
    core.solid("Red", "#ff0000", width=WIDTH, height=HEIGHT)
    frame = core.frame(0.0)
    assert pixel(frame, 5, 5)[:3] == (255, 0, 0)
    assert pixel(frame, WIDTH - 1, HEIGHT - 1)[:3] == (255, 0, 0)


def test_the_same_frame_renders_to_the_same_pixels_twice(core):
    build(core)
    assert digest(core.frame(0.5)) == digest(core.frame(0.5))


def test_the_same_frame_is_identical_in_a_second_engine(core):
    build(core)
    first = digest(core.frame(0.7))
    other = Core()
    build(other)
    assert digest(other.frame(0.7)) == first


def test_a_moving_layer_changes_the_frame_over_time(core):
    build(core)
    assert digest(core.frame(0.0)) != digest(core.frame(0.9))


def test_a_changed_composition_renders_differently(core):
    box = build(core)
    before = digest(core.frame(0.5))
    core.run("prop.set", layer=box, path="transform/opacity", value=10)
    assert digest(core.frame(0.5)) != before


def test_the_cpu_path_renders_without_a_gpu(core):
    build(core)
    state = json.loads(core.engine.set_gpu(False))
    assert state["enabled"] is False
    assert digest(core.frame(0.5)) == digest(core.frame(0.5))


def queue(core, fmt, output):
    core.run("renderQueue.add", format=fmt, output=str(output))
    return core.run("renderQueue.render", wait=True)


def test_png_sequence_has_one_valid_file_per_frame(core, tmp_path):
    build(core)
    result = queue(core, "png", tmp_path / "shot_[#####].png")
    assert result["rendered"]
    files = sorted(tmp_path.glob("shot_*.png"))
    assert [f.name for f in files] == [f"shot_{i:05d}.png" for i in range(FRAMES)]
    for f in (files[0], files[-1]):
        head = f.read_bytes()[:24]
        assert head[:8] == b"\x89PNG\r\n\x1a\n"
        assert struct.unpack(">II", head[16:24]) == (WIDTH, HEIGHT)


def test_png_sequence_frames_differ_as_the_layer_moves(core, tmp_path):
    build(core)
    queue(core, "png", tmp_path / "shot_[#####].png")
    first = (tmp_path / "shot_00000.png").read_bytes()
    last = (tmp_path / "shot_00009.png").read_bytes()
    assert first != last


def test_rendering_twice_writes_identical_files(core, tmp_path):
    build(core)
    queue(core, "png", tmp_path / "a" / "f_[#####].png")
    other = Core()
    build(other)
    queue(other, "png", tmp_path / "b" / "f_[#####].png")
    for i in (0, 4, 9):
        assert (tmp_path / "a" / f"f_{i:05d}.png").read_bytes() == (tmp_path / "b" / f"f_{i:05d}.png").read_bytes()


@needs_ffmpeg
def test_mp4_has_the_right_frame_count_size_and_duration(core, tmp_path):
    build(core)
    out = tmp_path / "clip.mp4"
    queue(core, "h264", out)
    assert out.is_file() and out.stat().st_size > 0
    info = probe(out, "codec_name", "width", "height", "nb_read_frames", "r_frame_rate")
    stream = info["streams"][0]
    assert stream["codec_name"] == "h264"
    assert (stream["width"], stream["height"]) == (WIDTH, HEIGHT)
    assert int(stream["nb_read_frames"]) == FRAMES
    assert float(info["format"]["duration"]) == pytest.approx(SECONDS, abs=1.5 / FPS)


@needs_ffmpeg
def test_mp4_follows_a_shorter_work_area(core, tmp_path):
    build(core)
    core.run("comp.workArea", start=0.0, end=0.5)
    out = tmp_path / "short.mp4"
    queue(core, "h264", out)
    info = probe(out, "nb_read_frames")
    assert int(info["streams"][0]["nb_read_frames"]) == 5
    assert float(info["format"]["duration"]) == pytest.approx(0.5, abs=1.5 / FPS)


def test_light_sweep_lights_a_band_not_the_whole_layer(core):
    core.comp(width=200, height=100, fps=FPS, duration=SECONDS, background="#000000")
    layer = core.solid("Red", "#ff0000", width=200, height=100)["layer"]
    core.run("effect.apply", effect="fx.generate.ccLightSweep", layers=[layer])
    for shape in (0, 1, 2):
        core.run("prop.set", layer=layer, path="effects/#1/shape", value=shape)
        frame = core.frame(0.0)
        assert pixel(frame, 100, 50)[1] > 60, shape
        assert pixel(frame, 2, 2)[:3] == (255, 0, 0), shape
        assert pixel(frame, 197, 97)[:3] == (255, 0, 0), shape


def test_a_relative_output_resolves_from_the_saved_project_folder(core, tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    build(core)
    project = tmp_path / "projects" / "shot.fvxproj"
    project.parent.mkdir()
    core.run("file.saveAs", path=str(project))
    result = queue(core, "png", "plates/f_[#####].png")
    expected = project.parent / "plates"
    assert result["results"][0]["output"] == str(expected / "f_[#####].png")
    assert len(list(expected.glob("f_*.png"))) == FRAMES
    assert not (tmp_path / "plates").exists()


def test_a_relative_output_without_a_saved_project_is_refused(core, tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    build(core)
    core.run("renderQueue.add", format="png", output="plates/f_[#####].png")
    assert "absolute output path" in core.fails("renderQueue.render", wait=True)
    assert not (tmp_path / "plates").exists()


def test_sequence_files_are_numbered_by_composition_frame(core, tmp_path):
    build(core)
    core.run("renderQueue.add", format="png", output=str(tmp_path / "f_[#####].png"), timeSpan="custom", start=0.5, end=0.8)
    core.run("renderQueue.render", wait=True)
    assert [f.name for f in sorted(tmp_path.glob("f_*.png"))] == ["f_00005.png", "f_00006.png", "f_00007.png"]


def test_comp_list_names_every_composition(core):
    first = core.comp(name="One")
    second = core.comp(name="Two", width=32, height=32)
    comps = core.run("comp.list")["comps"]
    assert [(c["id"], c["name"]) for c in comps] == [(first, "One"), (second, "Two")]
    assert comps[1]["width"] == 32 and comps[1]["active"] is True


def test_a_background_render_keeps_its_results_for_polling(core, tmp_path):
    build(core)
    core.run("renderQueue.add", format="png", output=str(tmp_path / "f_[#####].png"))
    item = core.run("renderQueue.render", wait=False)["started"][0]
    deadline = time.monotonic() + 120
    finished = []
    while time.monotonic() < deadline and not finished:
        core.engine.tick(time.time())
        listing = core.run("renderQueue.list")
        finished = [r for r in listing["finished"] if r["item"] == item]
        time.sleep(0.05)
    assert finished and finished[0]["ok"] is True, listing
    assert len(list(tmp_path.glob("f_*.png"))) == FRAMES
