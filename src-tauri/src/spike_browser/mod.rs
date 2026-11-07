//! Spike 6q-0 (never merged): a WebView2 child webview over the preview slot. See
//! `docs/superpowers/plans/2026-11-07-phase-6q-0-browser-spike.md` and `docs/spikes/S12-browser-pane.md`.

#[cfg(windows)]
mod com;
pub mod rules;

use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Url, WebviewBuilder, WebviewUrl};

/// The child's label. One page stands in for 6q-3's tabs.
pub const LABEL: &str = "spike-page";
/// wry's default arguments without `msSmartScreenProtection`, so SmartScreen stays on for the browser (research §3).
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI";

/// The placeholder's box and the window's inner size when it was measured, in CSS pixels.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaneRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub win_width: f64,
    pub win_height: f64,
}

pub struct SpikeState {
    home: PathBuf,
    rect: Mutex<Option<PaneRect>>,
    anchor: AtomicBool,
    log: Mutex<()>,
}

impl SpikeState {
    pub fn new(home: PathBuf) -> Self {
        Self {
            home,
            rect: Mutex::new(None),
            anchor: AtomicBool::new(false),
            log: Mutex::new(()),
        }
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// One JSONL line in `<home>/spike-browser.jsonl`, the record every check reads.
pub fn log(app: &AppHandle, by: &str, event: &str, data: Value) {
    let Some(state) = app.try_state::<SpikeState>() else {
        return;
    };
    let mut line = json!({ "t": now_ms() as u64, "by": by, "event": event });
    if let (Some(obj), Value::Object(extra)) = (line.as_object_mut(), data) {
        obj.extend(extra);
    }
    let _guard = state.log.lock();
    let path = state.home.join("spike-browser.jsonl");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

/// Emits to the app's own webview only: the page has no listeners and no IPC.
pub fn emit_app(app: &AppHandle, event: &str, payload: Value) {
    let _ = app.emit_to("main", event, payload);
}

fn page(app: &AppHandle) -> Result<tauri::Webview, String> {
    app.get_webview(LABEL).ok_or_else(|| "no page is open".to_string())
}

fn refuse(app: &AppHandle, url: &str) {
    log(app, "rust", "refused", json!({ "url": url }));
    let app2 = app.clone();
    let url = url.to_string();
    let _ = app.run_on_main_thread(move || emit_app(&app2, "spike://refused", json!({ "url": url })));
}

pub fn set_bounds(app: &AppHandle, x: f64, y: f64, width: f64, height: f64, why: &str) -> Result<(), String> {
    let page = page(app)?;
    page.set_bounds(tauri::Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(width.max(0.0), height.max(0.0)).into(),
    })
    .map_err(|e| e.to_string())?;
    let scale = page.window().scale_factor().unwrap_or(1.0);
    let actual = page.bounds().ok().map(|b| {
        let p = b.position.to_physical::<f64>(scale);
        let s = b.size.to_physical::<f64>(scale);
        json!({ "x": p.x, "y": p.y, "width": s.width, "height": s.height })
    });
    log(
        app,
        "rust",
        "bounds",
        json!({ "why": why, "requested": { "x": x, "y": y, "width": width, "height": height }, "actualPhysical": actual, "scale": scale }),
    );
    Ok(())
}

#[tauri::command]
pub async fn spike_browser_open(app: AppHandle, url: String, rect: PaneRect) -> Result<(), String> {
    let url: Url = url.parse().map_err(|e| format!("not a URL: {e}"))?;
    if !rules::allowed_navigation(&url) {
        refuse(&app, url.as_str());
        return Err(format!("refused: {url}"));
    }
    if let Ok(page) = page(&app) {
        log(&app, "rust", "navigate", json!({ "url": url.as_str() }));
        return page.navigate(url).map_err(|e| e.to_string());
    }
    let state = app.state::<SpikeState>();
    let main = app.get_webview_window("main").ok_or("no main window")?;
    let window = main.as_ref().window();
    let on_nav = app.clone();
    let builder = WebviewBuilder::new(LABEL, WebviewUrl::External(url.clone()))
        .data_directory(state.home.join("browser-profile"))
        .additional_browser_args(BROWSER_ARGS)
        .devtools(false)
        .on_navigation(move |u| {
            let ok = rules::allowed_navigation(u);
            if !ok {
                refuse(&on_nav, u.as_str());
            } else {
                log(&on_nav, "rust", "navigation", json!({ "url": u.as_str() }));
            }
            ok
        });
    let page = window
        .add_child(
            builder,
            LogicalPosition::new(rect.x, rect.y),
            LogicalSize::new(rect.width, rect.height),
        )
        .map_err(|e| e.to_string())?;
    *state.rect.lock().map_err(|e| e.to_string())? = Some(rect);
    log(
        &app,
        "rust",
        "opened",
        json!({ "url": url.as_str(), "rect": { "x": rect.x, "y": rect.y, "width": rect.width, "height": rect.height } }),
    );
    #[cfg(windows)]
    com::install(&app, &page);
    #[cfg(not(windows))]
    let _ = page;
    Ok(())
}

#[tauri::command]
pub async fn spike_browser_rect(app: AppHandle, rect: PaneRect) -> Result<(), String> {
    *app.state::<SpikeState>().rect.lock().map_err(|e| e.to_string())? = Some(rect);
    set_bounds(&app, rect.x, rect.y, rect.width, rect.height, "front")
}

#[tauri::command]
pub async fn spike_browser_mode(app: AppHandle, anchor: bool) -> Result<(), String> {
    app.state::<SpikeState>().anchor.store(anchor, Ordering::SeqCst);
    log(&app, "rust", "mode", json!({ "anchor": anchor }));
    Ok(())
}

#[tauri::command]
pub async fn spike_browser_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    let page = page(&app)?;
    if visible { page.show() } else { page.hide() }.map_err(|e| e.to_string())?;
    log(&app, "rust", if visible { "shown" } else { "hidden" }, json!({}));
    Ok(())
}

#[tauri::command]
pub async fn spike_browser_close(app: AppHandle) -> Result<(), String> {
    page(&app)?.close().map_err(|e| e.to_string())?;
    log(&app, "rust", "closed", json!({}));
    Ok(())
}

/// `reason`: "next", "previous" or "programmatic" (COREWEBVIEW2_MOVE_FOCUS_REASON_*).
#[tauri::command]
pub async fn spike_browser_focus(app: AppHandle, reason: String) -> Result<(), String> {
    log(&app, "rust", "focus-page", json!({ "reason": reason }));
    #[cfg(windows)]
    return com::focus_page(&app, &reason);
    #[cfg(not(windows))]
    return page(&app)?.set_focus().map_err(|e| e.to_string());
}

#[tauri::command]
pub async fn spike_log(app: AppHandle, event: String, data: Value) -> Result<(), String> {
    log(&app, "front", &event, data);
    Ok(())
}
