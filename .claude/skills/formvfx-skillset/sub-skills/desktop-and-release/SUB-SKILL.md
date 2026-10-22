---
name: desktop-and-release
description: The formvfx desktop app and releases. Use for the Tauri 2 shell in src-tauri (splash, sidecar launch, the main window, file associations, updater, capabilities), freezing the engine with PyInstaller, desktop builds for macOS, Windows and Linux, CI and the release workflow, icons, signing secrets, and version bumps.
---

# desktop-and-release

## The shell

- `src-tauri/src/main.rs` spawns the sidecar `binaries/formvfx-engine-[target triple]` with `FORMVFX_PORT=0` (and `FORMVFX_OPEN` when launched with an `.fvxproj` or `.ecproj`), reads `http://127.0.0.1:N` from its output, opens the main window on that URL sized to 90% of the screen, closes the splash (`frontend/public/splash.html`) and checks for updates when `plugins.updater.pubkey` is set.
- `capabilities/default.json` grants `core:default` and `dialog:default` to the splash and main windows, with `remote.urls` `http://127.0.0.1:*` so the interface served by the sidecar can use native file dialogs (`ui/file-dialog.ts` detects `__TAURI_INTERNALS__`).
- `tauri.conf.json`: identifier `ai.nodemode.formvfx`, the `.fvxproj` association (editor) and the `.ecproj` one for projects from earlier releases (viewer, opened as imports), bundle icons from `assets/formvfx_icon.png` (`tauri icon`), macOS hardened runtime with `entitlements.plist` (PyInstaller needs library validation off).

## The sidecar

`./formvfx freeze` (`setup/freeze_backend`, `.bat` twin) runs PyInstaller `--onefile` on `backend/formvfx_engine.py` with the built interface (`frontend_dist`), the agent templates (`agent_templates`), the skills (`skills`) and `--collect-all fvx_core`, copies it to `src-tauri/binaries/` with the host triple and runs `--selfcheck` (engine, command and effect counts, interface, skills). Freeze only a release build of the engine.

## Commands

| Command | Does |
| --- | --- |
| `./formvfx install-desktop` | PyInstaller, the Tauri CLI, checks Rust and (Linux) webkit2gtk |
| `./formvfx desktop dev` | freezes if needed, `tauri dev` |
| `./formvfx desktop build` | builds the interface, freezes, `tauri build`; bundles in `src-tauri/target/release/bundle` |

Linux needs `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`.

## CI and release

- `.github/workflows/ci.yml` on push (main and `compile_v*` branches) and pull request: interface build; engine (dev build of `native/fvx-core`, compile, selfcheck, `agents sync --check`, boot and probe HTTP and MCP); scripts (bash syntax, `lint_text.py`, Python syntax); desktop shell (`cargo check --locked` with a placeholder sidecar).
- Releases are built by formcompiler (`nodemode-ai/formcompiler`), not here. It watches this repo every 8 hours, or when run by hand, for a branch `compile_vX.Y.Z`; once CI has passed on that commit it builds macOS arm64, Linux x64 and Windows x64 bundles and drafts `formvfx-vX.Y.Z` on its own release page, with `latest.json` in the `formvfx-updates` feed. The signing secrets live only in formcompiler.

## Version bump

Hand-edit every site in one commit, last in the series: `frontend/package.json` and both fields in `frontend/package-lock.json`, `frontend/src/version.ts`, `backend/app/__init__.py`, `native/fvx-core/Cargo.toml` and `pyproject.toml`, the package's own entry in each of the two `Cargo.lock` files, `src-tauri/Cargo.toml`, `tauri.conf.json` and the version line in `README.md`. Push a `compile_vX.Y.Z` branch from that commit to release.
