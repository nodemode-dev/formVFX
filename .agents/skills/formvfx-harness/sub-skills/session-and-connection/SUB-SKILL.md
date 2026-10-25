---
name: session-and-connection
description: How an agent reaches formvfx. Use when the formvfx MCP server or its tools are missing, a tool reports that the engine is not running, a permission prompt or a refusal appears, the agent runs outside formvfx's Terminal panel, or the work should go through the formvfx command line instead of MCP.
---

# session-and-connection

## The pieces

- **The engine** is a local service (default `http://127.0.0.1:8250`) that holds the open project. The formvfx window, the MCP server and the command line all talk to the same engine, so they all see the same project.
- **The MCP server** `formvfx` is reachable two ways: `./formvfx mcp` (stdio; Claude Code, Codex and Cursor configs in this repository and in the agent workspace use it) and Streamable HTTP at `http://127.0.0.1:8250/mcp`. The stdio relay finds the running engine (the `FORMVFX_URL` variable, then the discovery record the engine writes, then the default port) and starts one in the background when none is running.
- **The command line** `formvfx` (or `./formvfx` in the repository) exposes the same surface: `formvfx state`, `formvfx commands FILTER`, `formvfx describe ID`, `formvfx exec ID 'JSON'`, `formvfx batch 'JSON'`, `formvfx frame OUT.png --time S`, `formvfx render --wait`. Use it when MCP tools are not loaded in this session.

## Where agents run

| Started from | Trust | Configuration |
| --- | --- | --- |
| formvfx's Terminal panel (Claude Code, Codex or Cursor buttons, or typed) | trusted: the shell carries the session token, nothing asks | the agent workspace: `CLAUDE.md`, `AGENTS.md`, `.mcp.json`, `.codex/config.toml`, `.cursor/mcp.json` and this skill |
| The formvfx repository | asks once per client in the window | `.mcp.json`, `.codex/config.toml`, `.cursor/mcp.json` at the repository root |
| Any other folder | asks once per client in the window | `./formvfx agents install` (or the Agents panel) adds the MCP server and the skill to the user's Claude Code, Codex and Cursor setup |

## Permission

- Reads (status, get_comp, get_layer, render_frame, contact_sheet, list_commands and every query command) never ask.
- The first change from an outside agent opens a dialog in the formvfx window: **Allow** (this session), **Allow Always** (remembered), **Don't Allow**. Tell the person a prompt is waiting in formvfx when you make the first change.
- `FORMVFX_AGENTS` on the engine sets the policy: `ask` (default), `allow` (never ask) or `deny` (refuse outside agents; Terminal agents still work).
- The Agents panel shows who is allowed and can forget them.

## When something is wrong

| Message | What to do |
| --- | --- |
| `The formvfx engine is not running and could not be started` | Ask the person to run `./formvfx boot` in the formvfx folder or open the desktop app, then retry. |
| `No formvfx window is open to ask the person for permission` | The policy is `ask` and no window is open. Ask the person to open formvfx (or start the agent from its Terminal panel), then retry. |
| `Nobody answered the permission request` | The dialog timed out. Ask the person to answer it in the formvfx window. |
| `Outside agents are turned off (FORMVFX_AGENTS=deny)` | Only Terminal agents may edit. Suggest starting the agent from the Terminal panel. |
| `The formvfx engine stopped answering` | The engine exited or is busy past the timeout. `status` again; if it fails, ask the person to restart formvfx. |

Background work (renders started with `wait: false`, tracking, roto, Selection-Aware-Fill, the watch folder, footage checks) is listed by `jobs.list`, waited for with `jobs.wait {timeout}` and stopped with `jobs.cancel {job}`. `editor.state` gives the whole editor at once (selection, open comps, Footage panel, jobs, undo). When a feature misbehaves on the person's machine, `help.systemReport {quiet: true}` checks FFmpeg, the GPU and the data folder, and `help.enableLogging {on: true}` records every command with its result in `logs/formvfx.log`.

## Several windows

More than one formvfx window can watch the same engine (`status` reports `windows`). They all show the same project and follow the same time and selection. Changes from any window, agent or command line appear in all of them.
