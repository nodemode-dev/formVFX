# Getting Started

## Requirements

Node 20.19+ or 22.12+, Python 3.12+, Rust 1.95+ (`https://rustup.rs`). `./formvfx doctor` (or `formvfx.bat doctor`) checks them and the ports.

## Install and run

```bash
./formvfx install    # engine venv, the Rust compositing core, interface packages
./formvfx            # engine on 127.0.0.1:8230, interface on http://localhost:5230
```

The first install compiles the compositing engine (`native/fvx-core`) in release mode and takes a few minutes. `./formvfx install-native --dev` rebuilds it quickly while developing.

## First composition

1. Open http://localhost:5230. The start screen offers New Composition, Open Project, Import and the demo project.
2. New Composition asks for the size, frame rate and duration.
3. Add layers from Layer > New or the toolbar: the rectangle and ellipse tools draw shape layers, the type tool makes text.
4. Select a layer and press P (position), S (scale), R (rotation), T (opacity) or A (anchor); click the stopwatch to start keyframes; move the time indicator and change the value to add another key.
5. Space plays a RAM preview with audio.
6. Composition > Add to Render Queue, then Render in the Render Queue panel.

Cmd or Ctrl+K searches every command and effect.

## Workspaces

The title bar's workspace menu switches between Default, Animation, Effects, Text, Motion Tracking, Agent and Minimal. Drag a panel tab onto another group to move it; the backtick key maximises the focused panel and restores it again.
