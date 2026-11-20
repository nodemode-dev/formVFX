import json
import logging
import os
import socket
import sys
from pathlib import Path

if sys.stdin is None:
    sys.stdin = open(os.devnull, encoding="utf-8")
if sys.stdout is None:
    sys.stdout = open(os.devnull, "w", encoding="utf-8")
if sys.stderr is None:
    sys.stderr = open(os.devnull, "w", encoding="utf-8")


def _bundle() -> Path:
    return Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parent))


def selfcheck() -> int:
    report = {"python": sys.version.split()[0], "frozen": bool(getattr(sys, "frozen", False))}
    try:
        from app import native

        core = native.module()
        caps = json.loads(core.Engine().capabilities())
        report.update({"core": core.__version__, "engine": native.engine_name(), "commands": caps.get("commands"), "effects": (caps.get("effects") or {}).get("total"), "ok": True})
    except BaseException as exc:
        report.update({"core": None, "error": repr(exc), "ok": False})
    report["interface"] = (_bundle() / "frontend_dist" / "index.html").is_file()
    report["skills"] = (_bundle() / "skills").is_dir()
    print(json.dumps(report))
    return 0 if report["ok"] else 1


def leave(data) -> None:
    try:
        from app.core import discovery, runtime

        if data is not None:
            runtime.release(data)
        discovery.withdraw()
    finally:
        os._exit(0)


def watch_shell(data) -> None:
    from app.core import startup

    shell = startup.shell_pid()
    if shell <= 1:
        return

    def gone() -> None:
        logging.getLogger("formvfx").info("the app that started this engine is gone (process %s), stopping", shell)
        leave(data)

    startup.watch(shell, gone)


def prepare() -> tuple[int, Path | None]:
    from app.core import config, runtime, startup

    try:
        data = config.data_dir()
        logs = runtime.start_logging(data).parent
    except OSError as err:
        print(f"formvfx cannot use its data folder: {err}", file=sys.stderr)
        return 5, None
    startup.keep_errors(logs)
    log = logging.getLogger("formvfx")
    if startup.alone():
        beside = startup.app_beside(Path(sys.executable))
        if beside is not None and startup.open_app(beside):
            log.info("started without its app, so it opened %s and stopped", beside)
            return 0, None
        log.error("started without its app, and no app program was found near %s", sys.executable)
        print("formvfx-engine is a part of FormVFX and serves nothing on its own. Open FormVFX instead.", file=sys.stderr)
        return 4, None
    try:
        runtime.claim(data)
    except runtime.AlreadyRunning as err:
        log.error("%s", err)
        print(err, file=sys.stderr)
        return 2, None
    return 0, data


def serve(headless: bool) -> int:
    port = int(os.environ.get("FORMVFX_PORT") or ("8250" if headless else "0"))
    data = None
    if not headless:
        os.environ["FORMVFX_DESKTOP"] = "1"
        code, data = prepare()
        if data is None:
            return code
    watch_shell(data)

    import uvicorn

    dist = _bundle() / "frontend_dist"
    if (dist / "index.html").is_file():
        os.environ.setdefault("FORMVFX_DIST", str(dist))
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    sock.bind(("127.0.0.1", port))
    sock.listen()
    port = sock.getsockname()[1]
    os.environ["FORMVFX_PORT"] = str(port)
    from app import __version__
    from app.core import config
    from app.core import discovery
    from app.core import window_key

    config.PORT = port
    from app.main import app

    if window_key.stdout_mode():
        print(window_key.LINE + window_key.KEY, flush=True)
    print(f"Uvicorn running on http://127.0.0.1:{port}", flush=True)
    discovery.publish(f"http://127.0.0.1:{port}", __version__)
    server = uvicorn.Server(uvicorn.Config(app, log_level="warning"))
    if data is None:
        server.run(sockets=[sock])
        return 0
    import signal

    from app.core import runtime

    runtime.start_logging(data)
    logging.getLogger("formvfx").info("serving on http://127.0.0.1:%s", port)
    signal.signal(signal.SIGTERM, lambda _signum, _frame: leave(data))
    try:
        server.run(sockets=[sock])
    finally:
        runtime.release(data)
    return 0


def main() -> int:
    args = sys.argv[1:]
    if args and args[0] == "--cli":
        from app.agent.cli import main as cli_main

        return cli_main(args[1:])
    if args and args[0] == "--mcp":
        from app.agent.stdio import main as stdio_main

        stdio_main()
        return 0
    if "--selfcheck" in args:
        return selfcheck()
    if "--gpu-probe" in args:
        from app import native

        print(native.module().gpu_probe(), flush=True)
        return 0
    try:
        return serve(headless="--headless" in args)
    except Exception:
        log = logging.getLogger("formvfx")
        if log.handlers:
            log.exception("the engine stopped on an error")
        raise


if __name__ == "__main__":
    raise SystemExit(main())
