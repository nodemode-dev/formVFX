# Getting Started

## Requirements

Node 20.19+ or 22.12+, Python 3.12+, Rust 1.95+ (`https://rustup.rs`). `./formvfx doctor` (or `formvfx.bat doctor`) checks them and the ports.

## Install and run

```bash
./formvfx install    # engine venv, the Rust compositing engine, interface packages
./formvfx            # engine on 127.0.0.1:8250, interface on http://localhost:5250
```

The first install compiles the compositing engine (`native/fvx-core`) in release mode and takes a few minutes. `./formvfx install-native --dev` rebuilds it quickly while developing.

Projects save as `.fvxproj`; `.ecproj` projects from earlier formvfx releases open as imports. Video and audio footage and movie renders use FFmpeg: an installed one is used, and the desktop app downloads its own LGPL build on first run when none is found.

## First composition

1. Open http://localhost:5250. The start screen offers New Composition, Open Project, Import and the demo project.
2. New Composition asks for the size, frame rate and duration.
3. Add layers from Layer > New or the toolbar: the rectangle and ellipse tools draw shape layers, the type tool makes text.
4. Select a layer and press P (position), S (scale), R (rotation), T (opacity) or A (anchor); click the stopwatch to start keyframes; move the time indicator and change the value to add another key.
5. Space plays a RAM preview with audio.
6. Composition > Add to Render Queue, then Render in the Render Queue panel.

Cmd or Ctrl+K searches every command and effect.

## Workspaces

The title bar's workspace menu switches between Default, Animation, Effects, Text, Motion Tracking, Agent and Minimal. Drag a panel tab onto another group to move it; the backtick key maximises the focused panel and restores it again. Window > Workspace has the same list, and Window > Panels opens any panel.

## Language

Settings > Interface > Language switches the whole interface, effect names included, between 26 languages; English is the default.

The [Manual](Manual) goes through the rest of the application area by area.
