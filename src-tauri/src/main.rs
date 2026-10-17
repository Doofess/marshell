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

/// The page origin the dev server serves in a debug build: `build.devUrl` in `tauri.conf.json` (a test keeps them equal).
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
const DEV_ORIGIN: &str = "http://localhost:1420";

/// The scheme and authority of a page URI ("http://tauri.localhost/index.html" gives "http://tauri.localhost"), or None when it has
/// none or carries credentials. Scheme and host are case-insensitive, so they are lowercased.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn origin_of(uri: &str) -> Option<String> {
    let (scheme, rest) = uri.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if scheme.is_empty() || authority.is_empty() || authority.contains('@') {
        return None;
    }
    Some(format!(
        "{}://{}",
        scheme.to_ascii_lowercase(),
        authority.to_ascii_lowercase()
    ))
}

/// Whether a page at `uri` may read the clipboard: only the app's own origins, compared whole (never by prefix, so
/// "http://tauri.localhost.evil.test" is not "http://tauri.localhost"). The dev server's origin counts in debug builds only.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn clipboard_read_origin_allowed(uri: &str) -> bool {
    let Some(origin) = origin_of(uri) else { return false };
    origin == "http://tauri.localhost"
        || origin == "tauri://localhost"
        || (cfg!(debug_assertions) && origin == DEV_ORIGIN)
}

/// Capturing a screen region reads the clipboard image back with `navigator.clipboard.read()`, which WebView2 would otherwise ask
/// the person about. Allow exactly that permission for the app's own pages; every other request, and every other origin, is left
/// to WebView2's default (the handler does not touch it).
fn allow_clipboard_read(window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ,
            COREWEBVIEW2_PERMISSION_STATE_ALLOW,
        };
        use webview2_com::{PermissionRequestedEventHandler, take_pwstr};
        use windows_core::PWSTR;
        let result = window.with_webview(|webview| {
            let handler = PermissionRequestedEventHandler::create(Box::new(|_sender, args| {
                let Some(args) = args else { return Ok(()) };
                // SAFETY: COM calls on the event's own arguments, made on the webview's thread inside the event.
                unsafe {
                    let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                    args.PermissionKind(&mut kind)?;
                    if kind != COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ {
                        return Ok(());
                    }
                    let mut uri = PWSTR::null();
                    args.Uri(&mut uri)?;
                    if clipboard_read_origin_allowed(&take_pwstr(uri)) {
                        args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                    }
                }
                Ok(())
            }));
            // SAFETY: COM calls on the live controller, made on the webview's own thread by with_webview.
            let added = unsafe {
                let mut token = 0i64;
                webview
                    .controller()
                    .CoreWebView2()
                    .and_then(|core| core.add_PermissionRequested(&handler, &mut token))
            };
            if let Err(e) = added {
                eprintln!("Marshell: reading the clipboard image may ask first: {e}");
            }
        });
        if let Err(e) = result {
            eprintln!("Marshell: reading the clipboard image may ask first: {e}");
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
                allow_clipboard_read(&w);
            }
            app.manage(capture::CaptureState::default());
            let home = marshell_core::paths::home_dir()?;
            marshell_core::crash::install_panic_hook(&home);
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
            capture::cancel_capture
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

    #[test]
    fn the_clipboard_is_readable_only_from_the_apps_own_origins() {
        use super::clipboard_read_origin_allowed as ok;
        assert!(ok("http://tauri.localhost/"));
        assert!(ok("http://tauri.localhost/index.html?x=1#y"));
        assert!(ok("tauri://localhost"));
        assert!(ok("tauri://localhost/index.html"));
        // Not by prefix, not another scheme, not another port, not with credentials.
        assert!(!ok("http://tauri.localhost.evil.test/"));
        assert!(!ok("http://tauri.localhost@evil.test/"));
        assert!(!ok("https://tauri.localhost/"));
        assert!(!ok("http://tauri.localhost:8080/"));
        assert!(!ok("https://example.com/"));
        assert!(!ok("about:blank"));
        assert!(!ok(""));
        assert_eq!(ok("http://localhost:1420/"), cfg!(debug_assertions));
        assert!(!ok("http://localhost:1421/"));
    }

    /// The dev origin allowed in debug builds is the one the config serves.
    #[test]
    fn the_dev_origin_is_the_configs_dev_url() {
        let conf = include_str!("../tauri.conf.json");
        assert!(conf.contains(&format!("\"devUrl\": \"{}\"", super::DEV_ORIGIN)));
    }

    /// A window saved while it still had the system frame must not get it back: the config decides the frame.
    #[test]
    fn saved_window_state_leaves_the_frame_to_the_config() {
        use tauri_plugin_window_state::StateFlags;
        assert!(!super::window_state_flags().contains(StateFlags::DECORATIONS));
        assert!(super::window_state_flags().contains(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED));
    }
}
