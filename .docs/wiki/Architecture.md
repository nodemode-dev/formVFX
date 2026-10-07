# Architecture

```text
fvx_core                   native/fvx-core        the compositing engine, Rust and PyO3: project, commands, renderer, jobs
  -> engine service        backend/app            FastAPI on 127.0.0.1:8230, one engine thread
    -> /api/live, /api/viewer, /api/rpc, /mcp, /api/terminal
      -> interface         frontend/src           TypeScript and Vite, formvis Studio Dark
      -> agents            MCP tools, stdio relay, CLI
desktop shell              src-tauri              Tauri 2, runs the frozen engine as a sidecar
```

## The compositing core

`native/fvx-core` is formvfx's compositing engine, written in Rust for formvfx and loaded by `backend/app/native.py` as the `fvx_core` Python extension. It provides the project model, about 650 commands, 306 effects, the renderer, codecs and the expression and scripting engines, and its `Engine` class gives the engine service gestures for one-step drags, engine events, the UI snapshot the interface reads (including property trees), viewer overlays, graph samples, and render jobs that run without the Python lock.

## The engine service

One engine lives on one thread; everything that touches it is queued there. A tick drains engine events, polls background jobs and autosaves. Every surface (the interface socket, HTTP, MCP, the command line) goes through one JSON-RPC dispatcher, so all of them see the same commands and the same permission rules. Frames stream to the viewer as binary messages, latest request first.

## The interface

The interface renders engine state and sends commands; it holds no editing logic. A store pulls the UI snapshot when the revision changes and emits granular events, so each panel redraws only what changed. Panels are built once and patched in place; the design files come verbatim from formvis.

## Agents

The MCP tools are a thin, well-described layer over commands (plus rendering to images). Permission is a per-boot token for agents in the Terminal panel and an ask-once dialog for the rest.
