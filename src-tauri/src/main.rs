// No console window behind the app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;

use marshell_core::server::CoreConfig;
use marshell_core::supervisor::{CoreEvent, Supervisor};
use marshell_protocol::api::Endpoint;
#[cfg_attr(not(target_os = "linux"), allow(unused_imports))]
use marshell_protocol::brand;
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager};

/// Keeps the core running for the app's lifetime (restarting it if it fails); shut down on exit.
struct CoreHandle(Arc<Supervisor>);

#[tauri::command]
fn core_endpoint(core: tauri::State<'_, CoreHandle>) -> Result<Endpoint, String> {
    core.0.endpoint().ok_or_else(|| "the core is not running".to_string())
}

/// Tells the window when the core restarted (its sessions ended) or gave up.
fn emit_core_event(app: &tauri::AppHandle, event: CoreEvent) {
    let name = match event {
        CoreEvent::Restarted { .. } => "core-restarted",
        CoreEvent::Failed => "core-failed",
    };
    let sent = app.emit(name, event);
    if let Err(e) = sent {
        eprintln!("could not tell the window about the core: {e}");
    }
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

/// WebView2 runs its own shortcuts (Ctrl+Shift+I devtools, F5 and Ctrl+R reload, Ctrl+F find, Ctrl+P print)
/// before the page sees the key, so they would shadow app shortcuts. Off in debug builds too: the shortcut
/// must behave the same while developing. WebKit on mac and Linux has none of these, so they need nothing yet.
fn disable_browser_shortcuts(window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
        use windows_core::Interface;
        let result = window.with_webview(|webview| {
            // SAFETY: COM calls on the live controller, made on the webview's own thread by with_webview.
            let off = unsafe {
                webview
                    .controller()
                    .CoreWebView2()
                    .and_then(|core| core.Settings())
                    .and_then(|settings| settings.cast::<ICoreWebView2Settings3>())
                    .and_then(|settings| settings.SetAreBrowserAcceleratorKeysEnabled(false))
            };
            if let Err(e) = off {
                eprintln!("Marshell: the browser's own shortcuts stay on: {e}");
            }
        });
        if let Err(e) = result {
            eprintln!("Marshell: the browser's own shortcuts stay on: {e}");
        }
    }
    #[cfg(not(windows))]
    let _ = window;
}

/// What the window keeps between runs: everything but the frame, which the config sets (none on Windows, where the
/// app draws its own title bar). Restoring a frame saved before that would draw the system's bar over the app's.
fn window_state_flags() -> tauri_plugin_window_state::StateFlags {
    tauri_plugin_window_state::StateFlags::all() - tauri_plugin_window_state::StateFlags::DECORATIONS
}

fn main() {
    // First, so every process Marshell starts dies with it, even after a crash (Windows only).
    if let Err(e) = marshell_core::job::tie_children_to_app() {
        eprintln!("Marshell: children may outlive a crash: {e}");
    }
    linux_webview_workarounds();
    let app = tauri::Builder::default()
        // Must be the first plugin: a second launch focuses the existing window instead.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(window_state_flags())
                .build(),
        )
        // Opens a web link in the person's browser, only after the preview's confirm; the capability limits it to http(s).
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Some(w) = app.get_webview_window("main") {
                disable_browser_shortcuts(&w);
            }
            app.manage(capture::CaptureState::default());
            let home = marshell_core::paths::home_dir()?;
            marshell_core::crash::install_panic_hook(&home);
            // Off the launch path: copying the bridge into ~/.marshell/bin may wait on a running copy.
            let bridge_home = home.clone();
            std::thread::spawn(move || match std::env::current_exe() {
                Ok(exe) => match marshell_core::bridge::source_next_to(&exe) {
                    Some(source) => {
                        if let Err(e) = marshell_core::bridge::install(&source, &bridge_home) {
                            eprintln!("Marshell: the hook bridge was not installed: {e}");
                        }
                    }
                    None => eprintln!("Marshell: no hook bridge next to the app, hooks will not report"),
                },
                Err(e) => eprintln!("Marshell: could not find the app's own path for the hook bridge: {e}"),
            });
            let cfg = CoreConfig {
                home,
                allowed_origins: allowed_origins(),
            };
            let handle = app.handle().clone();
            let supervisor =
                tauri::async_runtime::block_on(Supervisor::start(cfg, move |event| emit_core_event(&handle, event)))?;
            // Inside block_on so the watch task spawns on Tauri's tokio runtime.
            tauri::async_runtime::block_on(async { supervisor.watch(Duration::from_secs(1)) });
            app.manage(CoreHandle(supervisor));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_endpoint,
            capture::capture_screen_region,
            capture::cancel_capture,
            capture::read_clipboard_image
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the app");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            handle.state::<CoreHandle>().0.shutdown();
            // The sessions have ended; anything else started from a shell (VS Code, a browser) keeps running.
            marshell_core::job::release_children();
        }
    });
}

#[cfg(test)]
mod tests {
    /// Debug builds inject Tauri's own Ctrl+Shift+I listener (toggle-devtools.js), which opens DevTools
    /// through a command, past the WebView2 shortcut switch. Denying the command leaves the key to focus mode.
    #[test]
    fn ctrl_shift_i_cannot_open_devtools() {
        let caps = include_str!("../capabilities/default.json");
        assert!(caps.contains("\"core:webview:deny-internal-toggle-devtools\""));
    }

    /// A window saved while it still had the system frame must not get it back: the config decides the frame.
    #[test]
    fn saved_window_state_leaves_the_frame_to_the_config() {
        use tauri_plugin_window_state::StateFlags;
        assert!(!super::window_state_flags().contains(StateFlags::DECORATIONS));
        assert!(super::window_state_flags().contains(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED));
    }
}
