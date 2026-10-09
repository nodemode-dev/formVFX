---
name: verification
description: Verifying a change to formvfx before it ships. Use after any interface, engine, agent or desktop change to run the builds, boot the stack, drive it in headless Chromium under CPU and network throttling, replay MCP tool sequences, and check the text, skill and version gates.
---

# verification

## Gates

| Gate | Command | Passes when |
| --- | --- | --- |
| Interface types and bundle | `cd frontend && npm run build` | no errors |
| Text hygiene | `python3 setup/lint_text.py .` | 0 findings |
| Agent references | `./formvfx agents sync --check` | in step |
| Skills | the god-tier `validate_skill.py` on `.claude/skills/formvfx-harness` and `formvfx-skillset` | 0 FAIL |
| Engine | `backend/venv/bin/python backend/formvfx_engine.py --selfcheck` | `"ok": true` with the engine's command and effect counts |
| Sidecar | `./formvfx freeze` | the frozen binary's selfcheck reports the engine, interface and skills |
| Shell | `cd src-tauri && cargo check` | compiles |

## Interface under throttling

1. Build the interface and serve it from the engine: `FORMVFX_DIST=frontend/dist ./formvfx backend`, then open the demo (`./formvfx exec file.openDemoProject '{}'`).
2. Drive Chromium with Playwright (`/opt/pw-browsers` in the cloud workspace): CDP `Emulation.setCPUThrottlingRate` 4, `Network.emulateNetworkConditions` (120 ms, 1.5 MB/s), cache disabled, a `layout-shift` PerformanceObserver injected before load.
3. Screenshot at 0.15, 0.4, 0.8, 1.5, 2.5 and 4 s after navigation: the page must be one uniform background until the shell reveals complete, media must fill reserved boxes, and the observer must record no shifts.
4. Exercise every path the change touched (clicks, keys, drags) and screenshot the moment of change, not only the end state. Collect `pageerror` and console errors; there must be none.

## Agent paths

Replay tool sequences through `setup/mcp_stdio.py` against a running engine (`FORMVFX_AGENTS=allow` for unattended runs, `ask` to test the consent dialog with a browser open). Save the images `render_frame` and `contact_sheet` return and look at them. A change to a tool is verified only when its output was looked at.

## Report

State in the turn ledger: what was verified under throttling, what without and why, and what was not verified.
