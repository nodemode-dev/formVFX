# Releasing

1. Land the work on main with CI green (interface build, engine selfcheck and MCP probe, skill references in step, text lint, shell syntax, `cargo check` of the desktop shell).
2. Bump the version in every site in one commit (see `.docs/BRANCHING.md`).
3. Push a branch named `compile_vX.Y.Z` from that commit, for example `git push origin HEAD:compile_v0.51.1`. CI runs on it. The name must match the version in `src-tauri/tauri.conf.json`, or formcompiler refuses it and says why.
4. formcompiler (`nodemode-ai/formcompiler`) finds the branch on its next watch run (every 8 hours), or at once when its watch is run by hand. Once CI has passed on that commit, it builds macOS arm64, Linux x64 and Windows x64 bundles and opens a draft release `formvfx-vX.Y.Z` on its release page.
5. Signing: the Apple Developer ID, Windows code signing and Tauri updater key secrets are set in formcompiler's repository, never in this one. Updater signatures also publish `latest.json` in the `formvfx-updates` feed. Set `plugins.updater.pubkey` in `src-tauri/tauri.conf.json` to the matching public key to turn update checks on.
6. Check the draft release's assets on formcompiler's release page, then publish it there. Compile branches are kept as the record of what was built.
