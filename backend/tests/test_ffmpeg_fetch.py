import hashlib
import io
import tarfile

import pytest

from app import ffmpeg_fetch

GOOD = {"ffmpeg": b"engine-bytes", "ffprobe": b"probe-bytes"}


def archive(files):
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as bundle:
        for name, data in files.items():
            info = tarfile.TarInfo(f"pack/{name}")
            info.size = len(data)
            info.mode = 0o755
            bundle.addfile(info, io.BytesIO(data))
    return buf.getvalue()


@pytest.fixture
def world(tmp_path, monkeypatch):
    triple = ffmpeg_fetch.host_triple()
    names = {f"{program}-{triple}": data for program, data in GOOD.items()}
    monkeypatch.setitem(ffmpeg_fetch.FILES, triple, tuple((name, hashlib.sha256(data).hexdigest()) for name, data in names.items()))
    monkeypatch.setattr(ffmpeg_fetch, "folder", lambda: tmp_path / "shared" / "ffmpeg")
    monkeypatch.delenv("FORMCUT_FFMPEG_RELEASE", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path / "empty"))
    served = {}
    asked = []

    def download(url, target):
        asked.append(url)
        if url not in served:
            raise OSError(f"404 for {url}")
        target.write_bytes(served[url])

    monkeypatch.setattr(ffmpeg_fetch, "_download", download)
    return {"names": names, "served": served, "asked": asked, "where": tmp_path / "shared" / "ffmpeg"}


def tampered(world):
    return archive({name: data + b"-changed" for name, data in world["names"].items()})


def test_the_new_address_is_tried_first_and_the_old_one_second():
    first, second = ffmpeg_fetch.sources()
    assert "/nodemode-dev/formai-binaries/releases/download/" in first
    assert "/nodemode-dev/formcut-binaries/releases/download/" in second
    assert first.rsplit("/", 2)[1:] == second.rsplit("/", 2)[1:]


def test_the_override_is_the_only_source(monkeypatch):
    monkeypatch.setenv("FORMCUT_FFMPEG_RELEASE", "http://127.0.0.1:9/mirror/")
    assert ffmpeg_fetch.sources() == [f"http://127.0.0.1:9/mirror/ffmpeg-lgpl-{ffmpeg_fetch.RELEASE}-{ffmpeg_fetch.host_triple()}.tar.gz"]


def test_the_first_address_answering_is_the_only_one_asked(world):
    first, _ = ffmpeg_fetch.sources()
    world["served"][first] = archive(world["names"])
    assert ffmpeg_fetch.fetch() == world["where"]
    assert world["asked"] == [first]


def test_a_missing_new_address_falls_back_to_the_old_one(world):
    first, second = ffmpeg_fetch.sources()
    world["served"][second] = archive(world["names"])
    assert ffmpeg_fetch.fetch() == world["where"]
    assert world["asked"] == [first, second]


def test_bytes_that_do_not_match_the_record_fall_back_to_the_old_address(world):
    first, second = ffmpeg_fetch.sources()
    world["served"][first] = tampered(world)
    world["served"][second] = archive(world["names"])
    assert ffmpeg_fetch.fetch() == world["where"]
    assert (world["where"] / next(iter(world["names"]))).read_bytes() == GOOD["ffmpeg"]


def test_nothing_is_installed_when_no_address_matches(world):
    first, second = ffmpeg_fetch.sources()
    world["served"][first] = tampered(world)
    world["served"][second] = tampered(world)
    with pytest.raises(ValueError) as failed:
        ffmpeg_fetch.fetch()
    assert first in str(failed.value) and second in str(failed.value)
    assert not world["where"].exists()


def test_the_override_is_never_followed_by_the_built_in_addresses(world, monkeypatch):
    monkeypatch.setenv("FORMCUT_FFMPEG_RELEASE", "http://127.0.0.1:9/mirror")
    with pytest.raises(ValueError):
        ffmpeg_fetch.fetch()
    assert world["asked"] == ffmpeg_fetch.sources()
    assert not world["where"].exists()


def test_the_download_needs_no_extra_package(tmp_path, monkeypatch):
    import http.server
    import sys
    import threading
    import urllib.error

    body = b"x" * (3 << 20)

    class Serve(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path != "/archive.tar.gz":
                self.send_response(404)
                self.end_headers()
                return
            self.send_response(200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *args):
            pass

    monkeypatch.setitem(sys.modules, "httpx", None)
    monkeypatch.setenv("NO_PROXY", "127.0.0.1")
    server = http.server.HTTPServer(("127.0.0.1", 0), Serve)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{server.server_address[1]}"
    try:
        ffmpeg_fetch._download(f"{base}/archive.tar.gz", tmp_path / "a.tar.gz")
        assert (tmp_path / "a.tar.gz").read_bytes() == body
        with pytest.raises(urllib.error.HTTPError):
            ffmpeg_fetch._download(f"{base}/missing.tar.gz", tmp_path / "b.tar.gz")
    finally:
        server.shutdown()
        server.server_close()
