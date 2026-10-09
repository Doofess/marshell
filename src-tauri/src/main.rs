// No console window behind the app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use marshell_core::server::{CoreConfig, RunningCore};
use marshell_protocol::api::Endpoint;
#[cfg_attr(not(target_os = "linux"), allow(unused_imports))]
use marshell_protocol::brand;
use std::sync::Mutex;
use tauri::Manager;

/// Keeps the core alive for the app's lifetime; taken on exit to shut it down.
struct CoreHandle(Mutex<Option<RunningCore>>);

#[tauri::command]
fn core_endpoint(core: tauri::State<'_, CoreHandle>) -> Result<Endpoint, String> {
    core.0
        .lock()
        .unwrap()
        .as_ref()
        .map(|c| c.endpoint.clone())
        .ok_or_else(|| "the core is not running".to_string())
}

fn allowed_origins() -> Vec<String> {
    let mut origins = vec![
        "tauri://localhost".to_string(),
        "http://tauri.localhost".to_string(),
        "https://tauri.localhost".to_string(),
    ];
    if cfg!(debug_assertions) {
        origins.push("http://localhost:1420".to_string());
    }
    origins
}

/// Spike S1 tries WebKitGTK workarounds one at a time through the `brand::env_var("LINUX_GFX")` env var.
/// Never overrides a value the user already set.
fn linux_webview_workarounds() {
    #[cfg(target_os = "linux")]
    {
        let mode = std::env::var(brand::env_var("LINUX_GFX")).unwrap_or_default();
        let vars: &[&str] = match mode.as_str() {
            "no-dmabuf" => &["WEBKIT_DISABLE_DMABUF_RENDERER"],
            "no-compositing" => &["WEBKIT_DISABLE_COMPOSITING_MODE"],
            "both" => &["WEBKIT_DISABLE_DMABUF_RENDERER", "WEBKIT_DISABLE_COMPOSITING_MODE"],
            _ => &[],
        };
        for v in vars {
            if std::env::var_os(v).is_none() {
                // SAFETY: runs first thing in main, before any other thread exists.
                unsafe { std::env::set_var(v, "1") };
            }
        }
    }
}

fn main() {
    linux_webview_workarounds();
    let app = tauri::Builder::default()
        // Must be the first plugin: a second launch focuses the existing window instead.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            let home = marshell_core::paths::home_dir()?;
            let cfg = CoreConfig {
                home,
                allowed_origins: allowed_origins(),
            };
            let core = tauri::async_runtime::block_on(marshell_core::server::start(cfg))?;
            app.manage(CoreHandle(Mutex::new(Some(core))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![core_endpoint])
        .build(tauri::generate_context!())
        .expect("failed to build the app");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event
            && let Some(core) = handle.state::<CoreHandle>().0.lock().unwrap().take()
        {
            core.shutdown();
        }
    });
}
