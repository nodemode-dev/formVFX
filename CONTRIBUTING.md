# Contributing to FormVFX

Thank you for helping. FormVFX aims to be a free, local-first motion graphics and
visual effects compositor, and it gets there through real use and real contributions.

## Where to start

- **Use it and report what goes wrong.** A clear bug report, with steps, the version
  and your platform, is one of the most useful things you can give.
- **Look at the open issues.** Comment on one before starting larger work, so nobody
  builds the same thing twice.
- **Translations** are welcome: FormVFX ships in 26 languages, with the tables in
  `frontend/src/ui/i18n/tables`.

## Setting up

Clone the repository and run `./formvfx install`, then `./formvfx` (or
`formvfx.bat install` and `formvfx.bat` on Windows). The first sets up the engine,
builds the Rust compositing engine and installs the interface; the second starts the
engine and the interface. `./formvfx doctor` checks Node, Python, the Rust version and
the ports, and the README's Quick start has the details and the requirements.

The code lives in:

| Folder | What is there |
| --- | --- |
| `native/fvx-core` | the compositing engine (Rust, loaded from Python with PyO3) |
| `backend/app` | the engine service (Python, FastAPI): engine thread, routers, agent tools |
| `frontend/src` | the interface (TypeScript, Vite) |
| `src-tauri` | the desktop shell (Rust, Tauri) |
| `setup` | install, boot, build, freeze, desktop and agent scripts |
| `backend/tests`, `frontend/tests/window` | the engine tests and the window smoke test |
| `.docs/wiki` | the wiki's source, published on push |

## Before you open a pull request

Run the same checks CI runs:

```sh
(cd frontend && npm ci && npm audit --audit-level=low && npm run build)
(cd backend && ./oc_install) && ./formvfx install-native --dev
(cd backend && venv/bin/python -m compileall -q app formvfx_engine.py && venv/bin/python formvfx_engine.py --selfcheck)
./formvfx agents sync --check
(cd backend && venv/bin/pip install -r requirements-test.txt && venv/bin/python -m pytest tests -q)
backend/venv/bin/python -m pytest frontend/tests/window -q -p no:cacheprovider
python3 setup/lint_text.py .
python3 -m py_compile setup/*.py
python3 setup/text_presets.py --check
(cd src-tauri && cargo check --locked)
```

The window smoke test needs the built window and Playwright's Chromium, and the
movie render test needs FFmpeg. CI also checks the shell scripts with `bash -n`.
`setup/lint_text.py` refuses pictographs, an em dash, and the names of other products
anywhere in the tree.

- Keep each pull request to one change, with tests for what it adds or fixes.
- Write commit subjects the way the history does, in Conventional Commits:
  `feat(engine): ...`, `fix(interface): ...`, `docs(...)` (see
  [.docs/BRANCHING.md](.docs/BRANCHING.md)). Leave the version alone; the maintainer
  bumps it when releasing.
- A change to the engine's commands, effects or MCP tools is followed in the same
  commit by `./formvfx agents sync`.
- FFmpeg stays outside the bundle and runs as a separate program.
- Every new dependency, binary, model or data file gets a row in `NOTICE.md` before
  it ships.
- Formats are named by their extension. Never copy vendor help text into effect names,
  parameter descriptions or the manual, and never bundle vendor icons, presets,
  templates or sample projects.

## How changes land

Only the maintainer pushes to `main`. Everyone else forks the repository and opens a pull request into `main`. A pull request merges when all six checks pass on an up-to-date branch and the maintainer has approved it after its last push.

## Releases

A release is a version, and only the maintainer makes one: after the version is bumped on
`main`, the maintainer cuts a `compile_vX.Y.Z` branch, and formcompiler
(`nodemode-dev/formcompiler`) builds every platform and publishes them together. No
push from a contributor starts a release. See [.docs/BRANCHING.md](.docs/BRANCHING.md)
and the Releasing page of the wiki.

## Licensing

FormVFX is licensed under the Apache License, Version 2.0 ([LICENSE](LICENSE), SPDX
`Apache-2.0`). By contributing, you agree that your contribution is licensed under
the same terms. Only add code you have the right to contribute, and name any
third-party code and its licence in the pull request.

## Patents and other people's IP

A feature may be held back or removed if it would infringe someone else's patent or
other intellectual property. The maintainers decide.

## Conduct

Everyone taking part follows the [code of conduct](CODE_OF_CONDUCT.md). Security
problems go through [SECURITY.md](SECURITY.md), never a public issue.
