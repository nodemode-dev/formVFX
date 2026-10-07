# Releasing

1. Land the work on main with CI green (interface build, engine selfcheck and MCP probe, skill references in step, text lint, shell syntax, `cargo check` of the desktop shell).
2. Bump the version in every site in one commit (see `.docs/BRANCHING.md`).
3. Tag `vX.Y.Z` and push the tag. `.github/workflows/desktop.yml` builds macOS arm64, Linux x64 and Windows x64 bundles and opens a draft release with them.
4. Optional signing: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` for macOS; `TAURI_SIGNING_PRIVATE_KEY` and its password for updater signatures, which also publish `latest.json`. Set `plugins.updater.pubkey` in `src-tauri/tauri.conf.json` to the matching public key to turn update checks on.
5. Review the draft release and publish it.
