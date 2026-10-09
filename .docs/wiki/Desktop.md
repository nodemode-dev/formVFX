# Desktop

The desktop app is a Tauri 2 shell that runs the engine as a sidecar and shows the interface it serves.

## Build locally

```bash
./formvfx install            # if not done
./formvfx install-desktop    # PyInstaller, the Tauri CLI; checks Rust and the Linux webview packages
./formvfx desktop dev        # freeze the engine if needed, then run the app
./formvfx desktop build      # bundles in src-tauri/target/release/bundle
```

On Debian or Ubuntu the shell needs `libwebkit2gtk-4.1-dev build-essential libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev patchelf`. On Windows use `formvfx.bat` with the same verbs.

## How it starts

1. The splash window appears.
2. The shell starts `formvfx-engine` with port 0; the engine picks a free port, prints its URL and serves the built interface from inside the binary.
3. The shell opens the main window on that URL and closes the splash. Opening an `.fvxproj` file (or an `.ecproj` from an earlier release) passes it to the engine, which opens it.
4. Quitting the app stops the engine.

## The frozen engine

`./formvfx freeze` builds `src-tauri/binaries/formvfx-engine-[target triple]` with PyInstaller: the engine service, the compositing engine, the built interface, the agent templates and the skills, in one file. It runs `--selfcheck` after building. Freeze a release build of the engine; the development build carries about a gigabyte of debug information.
