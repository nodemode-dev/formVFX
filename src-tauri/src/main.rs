#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod reveal;
mod traffic_lights;

use std::sync::Mutex;

use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_updater::UpdaterExt;

struct Backend(Mutex<Option<CommandChild>>);

const FALLBACK_LOGICAL: (f64, f64) = (1440.0, 900.0);
const MIN_LOGICAL: (f64, f64) = (1100.0, 700.0);
const SCREEN_FRACTION: f64 = 0.9;

fn kill_backend(handle: &tauri::AppHandle) {
    if let Some(child) = handle.state::<Backend>().0.lock().unwrap().take() {
        let _ = child.kill();
    }
}

fn close_splash(handle: &tauri::AppHandle) {
    if let Some(splash) = handle.get_webview_window("splash") {
        let _ = splash.close();
    }
}

fn check_for_updates(handle: tauri::AppHandle) {
    let configured = handle
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("pubkey"))
        .and_then(|key| key.as_str())
        .map_or(false, |key| !key.is_empty());
    if !configured {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let Ok(updater) = handle.updater() else { return };
        let Ok(Some(update)) = updater.check().await else { return };
        let install = handle
            .dialog()
            .message(format!("formvfx {} is available. Install it and restart?", update.version))
            .title("Update available")
            .buttons(MessageDialogButtons::OkCancel)
            .blocking_show();
        if install && update.download_and_install(|_, _| {}, || {}).await.is_ok() {
            kill_backend(&handle);
            handle.restart();
        }
    });
}

fn project_argument() -> Option<String> {
    std::env::args()
        .skip(1)
        .find(|arg| {
            let lower = arg.to_lowercase();
            (lower.ends_with(".fvxproj") || lower.ends_with(".ecproj")) && std::path::Path::new(arg).is_file()
        })
}

fn main() {
    reveal::mark_launch();
    let app = tauri::Builder::default()
        .on_window_event(|window, event| {
            if window.label() == "main"
                && matches!(
                    event,
                    WindowEvent::Resized(_)
                        | WindowEvent::Focused(_)
                        | WindowEvent::ThemeChanged(_)
                        | WindowEvent::ScaleFactorChanged { .. }
                )
            {
                traffic_lights::place(window);
            }
        })
        .invoke_handler(tauri::generate_handler![traffic_lights::set_window_title, reveal::app_ready])
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Backend(Mutex::new(None)))
        .setup(|app| {
            let handle = app.handle().clone();
            reveal::round_splash(&handle);
            let mut sidecar = app
                .shell()
                .sidecar("formvfx-engine")
                .expect("sidecar binary is bundled")
                .env("FORMVFX_PORT", "0")
                .env("FORMVFX_PARENT_PID", std::process::id().to_string());
            if let Some(project) = project_argument() {
                sidecar = sidecar.env("FORMVFX_OPEN", project);
            }
            let (mut rx, child) = sidecar.spawn().expect("the formvfx engine failed to spawn");
            app.state::<Backend>().0.lock().unwrap().replace(child);

            tauri::async_runtime::spawn(async move {
                let pattern = regex::Regex::new(r"http://127\.0\.0\.1:(\d+)").unwrap();
                let mut port: Option<u16> = None;
                while let Some(event) = rx.recv().await {
                    let line = match event {
                        CommandEvent::Stdout(bytes) | CommandEvent::Stderr(bytes) => {
                            String::from_utf8_lossy(&bytes).into_owned()
                        }
                        CommandEvent::Terminated(_) => break,
                        _ => continue,
                    };
                    if let Some(captures) = pattern.captures(&line) {
                        port = captures[1].parse().ok();
                        if port.is_some() {
                            break;
                        }
                    }
                }
                match port {
                    Some(found) => {
                        let url: tauri::Url = format!("http://127.0.0.1:{found}")
                            .parse()
                            .expect("loopback url");
                        let (width, height) = handle
                            .primary_monitor()
                            .ok()
                            .flatten()
                            .map(|monitor| {
                                let size = monitor.size();
                                let scale = monitor.scale_factor();
                                (
                                    (size.width as f64 / scale) * SCREEN_FRACTION,
                                    (size.height as f64 / scale) * SCREEN_FRACTION,
                                )
                            })
                            .unwrap_or(FALLBACK_LOGICAL);
                        let builder = WebviewWindowBuilder::new(&handle, "main", WebviewUrl::External(url))
                            .title("formvfx")
                            .visible(reveal::BUILD_VISIBLE)
                            .background_color(reveal::BACKGROUND)
                            .inner_size(width, height)
                            .min_inner_size(MIN_LOGICAL.0, MIN_LOGICAL.1)
                            .center();
                        #[cfg(target_os = "macos")]
                        let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
                        let builder = builder.on_page_load(|window, payload| {
                            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                                traffic_lights::place(&window.as_ref().window());
                            }
                        });
                        if let Ok(window) = builder.build() {
                            reveal::conceal(&window);
                            traffic_lights::place(&window.as_ref().window());
                        } else {
                            close_splash(&handle);
                        }
                        check_for_updates(handle.clone());
                    }
                    None => {
                        close_splash(&handle);
                        handle
                            .dialog()
                            .message("The formvfx engine failed to start. Please relaunch the app.")
                            .blocking_show();
                        kill_backend(&handle);
                        std::process::exit(1);
                    }
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("tauri app should build");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            kill_backend(handle);
        }
    });
}
