---
name: agent-substrate
description: How agents reach formvfx. Use to add or change an MCP tool or guide text, change consent and trust, the agent workspace files, the stdio relay, the formvfx command line, client installation for Claude Code, Codex and Cursor, or the formvfx-harness skill and its generated references.
---

# agent-substrate

## Pieces

| Piece | File | Notes |
| --- | --- | --- |
| MCP tools, guides, instructions, prompt | `backend/app/agent/tools.py` | `TOOLS` (schemas and annotations), `GUIDES`, `INSTRUCTIONS`, `call()` |
| MCP transport | `backend/app/routers/mcp.py` | Streamable HTTP at `/mcp`, protocol versions 2025-03-26 to 2026-07-28 |
| stdio relay | `backend/app/agent/stdio.py`, `setup/mcp_stdio.py` | `./formvfx mcp`; finds or starts the engine, forwards each line |
| Consent | `backend/app/agent/consent.py` | per-boot `TOKEN` (Terminal agents), `FORMVFX_AGENTS` policy, ask-once dialog through the bus, remembered clients |
| Workspace | `backend/app/agent/workspace.py`, `templates/` | `~/Formvfx/agent-workspace`: CLAUDE.md, AGENTS.md, `.mcp.json`, `.claude/settings.json`, `.codex/config.toml`, `.cursor/mcp.json`, `bin/formvfx`, the harness skill |
| Client setup | `backend/app/agent/connect.py`, `setup/agents.py install` | user-scope MCP entries and skill copies, backups before changes |
| CLI | `backend/app/agent/cli.py`, `setup/fx.py` | `state`, `commands`, `describe`, `exec`, `batch`, `frame`, `render` |
| Skill generator | `setup/agents.py sync` | writes `references/commands.md`, `effects.md`, `property-paths.md`, `tools.md` from the native engine and `TOOLS`, mirrors to `.agents/skills` |

## Adding a tool

1. Add the schema to `TOOLS` with `_tool(name, description, props, required, read=..., destructive=..., idempotent=...)`. Write the description for a model choosing among tools: what it returns, units, defaults.
2. Handle it in `call()`; reuse `ex()` (one command) and `rpc.dispatch` rather than touching the engine directly. Return `text(...)`, `image(...)` or `fail(...)` with a message that says how to recover.
3. Read-only tools set `read=True` so consent never asks.
4. `./formvfx agents sync`, then add or adjust the sub-skill in `.claude/skills/formvfx-harness` that teaches it, and run the god-tier validator over the pack.
5. Exercise it through the stdio relay against a running engine before committing.

## Facts to keep straight

- `engine.batch` references count from 1 (`"$1.layer"`).
- Keyframe commands take layer seconds; `set_keyframes` converts.
- `prop.set` takes menu indices, not names.
- Every claim in the harness skill is checked against the engine; recipes are sequences that ran.
