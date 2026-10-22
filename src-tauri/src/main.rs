// No console window behind the app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod attention;
mod capture;
mod diagnostics;
mod folder_picker;
mod notify;
mod tray;

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
        CoreEvent::Notify(request) => return notify::deliver(app, request),
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

/// What the close button does.
enum CloseAction {
    Quit,
    Hide,
    Ask,
}

/// Without a tray nothing could bring a hidden window back, so the close button quits whatever was saved.
/// With one it follows the saved choice, and asks the person once when there is none.
fn close_action(tray_ready: bool, close_to_tray: Option<bool>) -> CloseAction {
    match close_to_tray {
        None if tray_ready => CloseAction::Ask,
        choice => close_once_action(tray_ready, choice.unwrap_or(false)),
    }
}

/// Carries out a choice without saving it, with the same no-tray safeguard as a normal close.
fn close_once_action(tray_ready: bool, keep_in_tray: bool) -> CloseAction {
    if tray_ready && keep_in_tray {
        CloseAction::Hide
    } else {
        CloseAction::Quit
    }
}

fn hide_to_tray(window: &tauri::Window) {
    if let Some(core) = window.app_handle().try_state::<CoreHandle>() {
        core.0.set_window_hidden(true);
    }
    let _ = window.hide();
}

/// The dialog's fallback when saving failed or the core is unavailable. Never changes the saved choice.
#[tauri::command]
fn close_once(app: tauri::AppHandle, keep_in_tray: bool) {
    match close_once_action(tray::tray_ready(&app), keep_in_tray) {
        CloseAction::Hide => {
            if let Some(window) = app.get_webview_window("main") {
                hide_to_tray(&window.as_ref().window());
            }
        }
        CloseAction::Quit | CloseAction::Ask => app.exit(0),
    }
}

/// What the window keeps between runs: everything but the frame, which the config sets (none on Windows, where the
/// app draws its own title bar). Restoring a frame saved before that would draw the system's bar over the app's.
/// Not whether it was visible either: quitting from the tray while hidden must not start the next run hidden.
fn window_state_flags() -> tauri_plugin_window_state::StateFlags {
    use tauri_plugin_window_state::StateFlags;
    StateFlags::all() - StateFlags::DECORATIONS - StateFlags::VISIBLE
}

/// Where the Marshell home is, for the settings the close button reads on every request.
struct HomeDir(std::path::PathBuf);

/// The close button, from the title bar, Alt+F4 or the taskbar. Always held back from Tauri's own close, so a
/// live tray icon can never keep the process alive after a quit.
fn on_close_requested(window: &tauri::Window, api: &tauri::CloseRequestApi) {
    let app = window.app_handle();
    api.prevent_close();
    // Read fresh each time: the choice may have been saved since the window opened.
    let saved = app
        .try_state::<HomeDir>()
        .and_then(|home| marshell_core::settings::load(&home.0).close_to_tray);
    match close_action(tray::tray_ready(app), saved) {
        CloseAction::Quit => app.exit(0),
        CloseAction::Hide => hide_to_tray(window),
        CloseAction::Ask => {
            // The window stays open and in front; the webview shows its one-time dialog.
            tray::show_main(app);
            if let Err(e) = app.emit("close-choice", ()) {
                eprintln!("Marshell: could not ask the window about closing: {e}");
            }
        }
    }
}

fn main() {
    marshell_core::diagnostics::perf::mark_launch();
    // First, so every process Marshell starts dies with it, even after a crash (Windows only).
    if let Err(e) = marshell_core::job::tie_children_to_app() {
        eprintln!("Marshell: children may outlive a crash: {e}");
    }
    linux_webview_workarounds();
    let app = tauri::Builder::default()
        // Must be the first plugin: a second launch focuses the existing window instead.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            tray::show_main(app)
        }))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(window_state_flags())
                .build(),
        )
        // Opens a web link in the person's browser, only after the preview's confirm; the capability limits it to http(s).
        // No capability grants the dialog to the webview; only the `pick_folder` command opens it.
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // No capability grants notifications to the webview; only the shell shows banners.
        .plugin(tauri_plugin_notification::init())
        // The UI reports focus too, but a webview that died while focused cannot report losing it.
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Focused(false))
                && let Some(core) = window.try_state::<CoreHandle>()
            {
                core.0.window_blurred();
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
            {
                on_close_requested(window, api);
            }
            // The taskbar may have changed colour, and the wands must stay readable on it.
            if matches!(event, tauri::WindowEvent::ThemeChanged(_)) {
                tray::theme_changed(window.app_handle());
            }
        })
        .setup(|app| {
            if let Err(e) = notify::init(app.handle()) {
                eprintln!("Marshell: banners will not answer clicks: {e}");
            }
            if let Err(e) = tray::build(app.handle()) {
                eprintln!("Marshell: no tray icon this run: {e}");
            }
            if let Some(w) = app.get_webview_window("main") {
                disable_browser_shortcuts(&w);
            }
            app.manage(capture::CaptureState::default());
            let home = marshell_core::paths::home_dir()?;
            app.manage(HomeDir(home.clone()));
            marshell_core::crash::install_panic_hook(&home);
            if let Err(e) = marshell_core::diagnostics::log::init(&home) {
                eprintln!("Marshell: no log file this run: {e:#}");
            }
            // Off the launch path: copying the programs into ~/.marshell/bin may wait on a running copy.
            let bridge_home = home.clone();
            let exe = std::env::current_exe();
            let programs_dir = exe
                .as_ref()
                .ok()
                .and_then(|e| e.parent().map(std::path::Path::to_path_buf));
            std::thread::spawn(move || match exe {
                Ok(exe) => {
                    use marshell_core::bridge::{BRIDGE_EXE, CLI_EXE, ensure, source_next_to};
                    let Some(dir) = exe.parent() else {
                        eprintln!("Marshell: could not find the folder of the app for the hook bridge");
                        return;
                    };
                    match ensure(dir, &bridge_home) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            eprintln!("Marshell: no hook bridge next to the app, hooks will not report");
                        }
                        Err(e) => eprintln!("Marshell: {BRIDGE_EXE} or {CLI_EXE} was not installed: {e}"),
                    }
                    if source_next_to(&exe, CLI_EXE).is_none() {
                        eprintln!(
                            "Marshell: no marshell program next to the app, `marshell uninstall` will not be there"
                        );
                    }
                }
                Err(e) => eprintln!("Marshell: could not find the app's own path for the hook bridge: {e}"),
            });
            let cfg = CoreConfig {
                home,
                allowed_origins: allowed_origins(),
                programs_dir,
                notify: None,
                notify_paused: Default::default(),
                window_hidden: Default::default(),
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
            diagnostics::save_diagnostics,
            folder_picker::pick_folder,
            core_endpoint,
            close_once,
            capture::capture_screen_region,
            capture::cancel_capture,
            capture::read_clipboard_image,
            attention::set_badge,
            tray::set_tray
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
        // Quitting while hidden must not start the next run hidden.
        assert!(!super::window_state_flags().contains(StateFlags::VISIBLE));
        assert!(super::window_state_flags().contains(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED));
    }

    #[test]
    fn close_action_quits_without_a_tray_whatever_was_saved() {
        for saved in [None, Some(true), Some(false)] {
            assert!(matches!(super::close_action(false, saved), super::CloseAction::Quit));
        }
    }

    #[test]
    fn close_action_follows_the_saved_choice() {
        assert!(matches!(
            super::close_action(true, Some(true)),
            super::CloseAction::Hide
        ));
        assert!(matches!(
            super::close_action(true, Some(false)),
            super::CloseAction::Quit
        ));
    }

    #[test]
    fn close_action_asks_when_nothing_is_saved() {
        assert!(matches!(super::close_action(true, None), super::CloseAction::Ask));
    }

    #[test]
    fn close_once_action_follows_the_choice_with_a_tray() {
        assert!(matches!(super::close_once_action(true, true), super::CloseAction::Hide));
        assert!(matches!(
            super::close_once_action(true, false),
            super::CloseAction::Quit
        ));
    }

    #[test]
    fn close_once_action_quits_without_a_tray_whatever_was_chosen() {
        for keep_in_tray in [true, false] {
            assert!(matches!(
                super::close_once_action(false, keep_in_tray),
                super::CloseAction::Quit
            ));
        }
    }
}
