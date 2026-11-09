//! Spike 6q-0 (never merged): a WebView2 child webview over the preview slot. See
//! `docs/superpowers/plans/2026-11-07-phase-6q-0-browser-spike.md` and `docs/spikes/S12-browser-pane.md`.

#[cfg(windows)]
mod clip;
#[cfg(windows)]
mod com;
pub mod rules;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Url, WebviewBuilder, WebviewUrl};

/// The child's label. One page stands in for 6q-3's tabs.
pub const LABEL: &str = "spike-page";
/// wry's default arguments without `msSmartScreenProtection`, so SmartScreen stays on for the browser (research §3).
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI";

/// The placeholder's box and the window's inner size when it was measured, in CSS pixels.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
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
    /// FOLLOW, ANCHOR, PREDICT or RULE: who moves the page when the window resizes.
    mode: AtomicU8,
    layout: Mutex<rules::Layout>,
    rule: Mutex<rules::Rule>,
    /// Task 10's oversize and clip: the room ahead, in physical pixels, a growing page gets; 0 places the page as before.
    oversize: AtomicU32,
    /// With oversize on: the page is held against the pane's still edge, not its top-left corner (`clip::anchored`).
    clip_still: AtomicBool,
    log: Mutex<()>,
}

/// The front's reports move the page; nothing else does.
const FOLLOW: u8 = 0;
/// Task 4's anchor mode: a resize moves the page from the last report, keeping its width; the reports move it too.
const ANCHOR: u8 = 1;
/// Rust places the page from the layout it learned from the reports, on every resize and every report (finding 20).
const PREDICT: u8 = 2;
/// Rust places the page from the layout rule the front sends, on every resize and every report (finding 21).
const RULE: u8 = 3;

impl SpikeState {
    pub fn new(home: PathBuf) -> Self {
        Self {
            home,
            rect: Mutex::new(None),
            mode: AtomicU8::new(FOLLOW),
            layout: Mutex::new(rules::Layout::default()),
            rule: Mutex::new(rules::Rule::default()),
            oversize: AtomicU32::new(0),
            clip_still: AtomicBool::new(false),
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
    #[cfg(windows)]
    {
        let state = app.state::<SpikeState>();
        let ahead = state.oversize.load(Ordering::SeqCst);
        if ahead > 0 {
            let still = state.clip_still.load(Ordering::SeqCst);
            return clip::place(app, &page, [x, y, width, height], ahead, still, why);
        }
    }
    let started = std::time::Instant::now();
    page.set_bounds(tauri::Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(width.max(0.0), height.max(0.0)).into(),
    })
    .map_err(|e| e.to_string())?;
    let set_ms = started.elapsed().as_secs_f64() * 1000.0;
    let scale = page.window().scale_factor().unwrap_or(1.0);
    let actual = page.bounds().ok().map(|b| {
        let p = b.position.to_physical::<f64>(scale);
        let s = b.size.to_physical::<f64>(scale);
        json!({ "x": p.x, "y": p.y, "width": s.width, "height": s.height })
    });
    let total_ms = started.elapsed().as_secs_f64() * 1000.0;
    log(
        app,
        "rust",
        "bounds",
        json!({ "why": why, "requested": { "x": x, "y": y, "width": width, "height": height }, "actualPhysical": actual, "scale": scale, "setMs": set_ms, "totalMs": total_ms }),
    );
    Ok(())
}

/// Anchor and predict modes: when the app's window is resized, the page moves at once, without waiting for the front.
/// This runs on the main thread inside the window's own resize, even during an edge drag's sizing loop, so the page moves
/// in the same instant as the window. The window comes from the event: once the page exists,
/// `get_webview_window("main")` returns None (S12 finding 16).
pub fn on_resized(window: &tauri::Window, size: tauri::PhysicalSize<u32>) {
    let app = window.app_handle();
    let Some(state) = app.try_state::<SpikeState>() else {
        return;
    };
    // The lag check's timeline of the window's own size, in the CSS pixels the front measures in.
    let size = size.to_logical::<f64>(window.scale_factor().unwrap_or(1.0));
    log(
        app,
        "rust",
        "win-resized",
        json!({ "width": size.width, "height": size.height }),
    );
    let to = match state.mode.load(Ordering::SeqCst) {
        ANCHOR => state
            .rect
            .lock()
            .ok()
            .and_then(|r| *r)
            .map(|rect| (rules::anchored(rect, size.width, size.height), "anchor")),
        PREDICT => state
            .layout
            .lock()
            .ok()
            .and_then(|l| l.place(size.width, size.height))
            .map(|r| (r, "predict-resize")),
        RULE => state
            .rule
            .lock()
            .ok()
            .and_then(|r| r.place(size.width, size.height))
            .map(|r| (r, "rule-resize")),
        _ => None,
    };
    if let Some((r, why)) = to {
        let _ = set_bounds(app, r.x, r.y, r.width, r.height, why);
    }
}

/// A report from the front, applied on the main thread, where the window's size is read without a round trip and the
/// move can't land between a resize and its own move. Predict and rule modes place the page for the window's size now,
/// which a report measured a frame ago may already be behind. Rule mode takes the report itself when it was measured at
/// the window's size now: a divider or sidebar drag changes the layout with the window still, and the report is newer
/// than the rule sent ahead of it (the rule is read when the layout changes, the report when its turn comes). Without a
/// rule yet, rule mode follows the report.
fn place_reported(app: &AppHandle, rect: PaneRect) {
    let state = app.state::<SpikeState>();
    let mode = state.mode.load(Ordering::SeqCst);
    let size = || {
        let window = app.get_window("main")?;
        let size = window.inner_size().ok()?;
        Some(size.to_logical::<f64>(window.scale_factor().unwrap_or(1.0)))
    };
    let placed = match mode {
        PREDICT => size()
            .and_then(|s| state.layout.lock().ok()?.place(s.width, s.height))
            .map(|r| (r, "predict-report")),
        RULE => size().and_then(|s| {
            if rules::same_size(s.width, s.height, rect) {
                let r = rules::LogicalRect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                };
                return Some((r, "rule-measured"));
            }
            state
                .rule
                .lock()
                .ok()?
                .place(s.width, s.height)
                .map(|r| (r, "rule-report"))
        }),
        _ => None,
    };
    match placed {
        Some((r, why)) => {
            let _ = set_bounds(app, r.x, r.y, r.width, r.height, why);
        }
        None if mode == PREDICT => {}
        None => {
            let _ = set_bounds(app, rect.x, rect.y, rect.width, rect.height, "front");
        }
    }
}

/// Rule mode's check: where the rule puts the pane for the size a report was measured at, against where the front
/// measured it. Logged only when they differ by more than half a pixel.
fn check_rule(app: &AppHandle, rect: PaneRect) {
    let state = app.state::<SpikeState>();
    let Some(at) = state
        .rule
        .lock()
        .ok()
        .and_then(|r| r.place(rect.win_width, rect.win_height))
    else {
        return;
    };
    let off = [
        at.x - rect.x,
        at.y - rect.y,
        at.width - rect.width,
        at.height - rect.height,
    ];
    if off.iter().any(|d| d.abs() > 0.5) {
        log(
            app,
            "rust",
            "rule-miss",
            json!({ "winWidth": rect.win_width, "winHeight": rect.win_height, "off": off }),
        );
    }
}

/// Task 8: every file drag over any webview, with the webview's label. The paths stay out of the log; their count and
/// extensions say enough.
pub fn on_drag_drop(webview: &tauri::Webview, event: &tauri::DragDropEvent) {
    let files = |paths: &[PathBuf]| -> Vec<String> {
        paths
            .iter()
            .map(|p| {
                p.extension()
                    .map(|e| e.to_string_lossy().into_owned())
                    .unwrap_or_default()
            })
            .collect()
    };
    let (kind, data) = match event {
        tauri::DragDropEvent::Enter { paths, position } => (
            "enter",
            json!({ "files": files(paths), "x": position.x, "y": position.y }),
        ),
        tauri::DragDropEvent::Drop { paths, position } => (
            "drop",
            json!({ "files": files(paths), "x": position.x, "y": position.y }),
        ),
        tauri::DragDropEvent::Leave => ("leave", json!({})),
        // Over comes on every mouse move; enter, drop and leave say what happened.
        _ => return,
    };
    let mut line = data;
    line["label"] = webview.label().into();
    line["kind"] = kind.into();
    log(webview.app_handle(), "rust", "drag-drop", line);
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
    let window = app.get_window("main").ok_or("no main window")?;
    let on_nav = app.clone();
    let on_popup = app.clone();
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
        })
        // A link or script asking for a new window opens in the pane instead; one page stands in for "a popup opens as a
        // new tab" (6q decision 3). The navigation goes through on_navigation's check like any other.
        .on_new_window(move |u, _features| {
            log(&on_popup, "rust", "popup", json!({ "url": u.as_str() }));
            let app2 = on_popup.clone();
            let _ = on_popup.run_on_main_thread(move || {
                emit_app(&app2, "spike://popup", json!({ "url": u.as_str() }));
                if let Ok(page) = page(&app2) {
                    let _ = page.navigate(u);
                }
            });
            tauri::webview::NewWindowResponse::Deny
        });
    let page = window
        .add_child(
            builder,
            LogicalPosition::new(rect.x, rect.y),
            LogicalSize::new(rect.width, rect.height),
        )
        .map_err(|e| e.to_string())?;
    *state.rect.lock().map_err(|e| e.to_string())? = Some(rect);
    state.layout.lock().map_err(|e| e.to_string())?.learn(rect);
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

/// `sent_at` (epoch ms, when the front measured) and `queued` (the front's commands not done yet) time the report's trip
/// for the lag check. The move itself goes to the main thread and the command returns at once, so the front's next
/// report doesn't wait for it.
#[tauri::command]
pub async fn spike_browser_rect(
    app: AppHandle,
    rect: PaneRect,
    sent_at: Option<f64>,
    queued: Option<u32>,
) -> Result<(), String> {
    let state = app.state::<SpikeState>();
    log(
        &app,
        "rust",
        "rect-in",
        json!({ "rect": rect, "sentAt": sent_at, "queued": queued }),
    );
    *state.rect.lock().map_err(|e| e.to_string())? = Some(rect);
    state.layout.lock().map_err(|e| e.to_string())?.learn(rect);
    if state.mode.load(Ordering::SeqCst) == RULE {
        check_rule(&app, rect);
    }
    let on_main = app.clone();
    app.run_on_main_thread(move || place_reported(&on_main, rect))
        .map_err(|e| e.to_string())
}

/// The front's layout as straight pieces over the window's width, sent before the report whenever the layout changes
/// (the divider, the sidebar's width or mode, the drawer, the bars above the pane).
#[tauri::command]
pub async fn spike_browser_rule(app: AppHandle, rule: rules::Rule) -> Result<(), String> {
    let rule = rule.sorted();
    log(
        &app,
        "rust",
        "rule",
        json!({ "pieces": rule.pieces.len(), "top": rule.top, "bottom": rule.bottom }),
    );
    *app.state::<SpikeState>().rule.lock().map_err(|e| e.to_string())? = rule;
    Ok(())
}

/// `follow`, `anchor`, `predict` or `rule`.
#[tauri::command]
pub async fn spike_browser_mode(app: AppHandle, mode: String) -> Result<(), String> {
    let value = match mode.as_str() {
        "follow" => FOLLOW,
        "anchor" => ANCHOR,
        "predict" => PREDICT,
        "rule" => RULE,
        other => return Err(format!("no mode {other}")),
    };
    app.state::<SpikeState>().mode.store(value, Ordering::SeqCst);
    log(&app, "rust", "mode", json!({ "mode": mode }));
    Ok(())
}

#[tauri::command]
pub async fn spike_browser_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    let page = page(&app)?;
    if visible { page.show() } else { page.hide() }.map_err(|e| e.to_string())?;
    log(&app, "rust", if visible { "shown" } else { "hidden" }, json!({}));
    Ok(())
}

/// Task 10: the room ahead a growing page gets, in physical pixels; 0 turns oversize and clip off and gives the page the
/// pane's size at once.
#[tauri::command]
pub async fn spike_browser_oversize(app: AppHandle, px: u32, still: Option<bool>) -> Result<(), String> {
    let still = still.unwrap_or(false);
    let state = app.state::<SpikeState>();
    state.oversize.store(px, Ordering::SeqCst);
    state.clip_still.store(still, Ordering::SeqCst);
    log(&app, "rust", "oversize", json!({ "px": px, "still": still }));
    #[cfg(windows)]
    if px == 0 {
        let app2 = app.clone();
        app.run_on_main_thread(move || clip::settle(&app2, None))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// `#rrggbb`, or `none` for no colour.
fn parse_color(s: &str) -> Result<Option<tauri::window::Color>, String> {
    if s == "none" {
        return Ok(None);
    }
    let hex = s
        .strip_prefix('#')
        .filter(|h| h.len() == 6)
        .ok_or_else(|| format!("{s}: a colour is #rrggbb or none"))?;
    let v = u32::from_str_radix(hex, 16).map_err(|e| format!("{s}: {e}"))?;
    Ok(Some(tauri::window::Color(
        (v >> 16) as u8,
        (v >> 8) as u8,
        v as u8,
        255,
    )))
}

/// Task 10: the main window's background (tao fills the client area with it on WM_ERASEBKGND) and the app webview's
/// (WebView2's DefaultBackgroundColor), each `#rrggbb` or `none`; a layer left out is unchanged. Runtime only.
#[tauri::command]
pub async fn spike_bg(app: AppHandle, window: Option<String>, webview: Option<String>) -> Result<(), String> {
    if let Some(c) = &window {
        let color = parse_color(c)?;
        app.get_window("main")
            .ok_or("no main window")?
            .set_background_color(color)
            .map_err(|e| e.to_string())?;
    }
    if let Some(c) = &webview {
        let color = parse_color(c)?;
        app.get_webview("main")
            .ok_or("no main webview")?
            .set_background_color(color)
            .map_err(|e| e.to_string())?;
    }
    log(&app, "rust", "bg", json!({ "window": window, "webview": webview }));
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

/// Runs a script in the page, for the checks CDP can't reach: the page has no CDP of its own (S12 finding 2).
#[tauri::command]
pub async fn spike_browser_eval(app: AppHandle, script: String) -> Result<(), String> {
    log(&app, "rust", "eval", json!({ "script": script }));
    page(&app)?.eval(script).map_err(|e| e.to_string())
}

/// `action`: "go" (with `url`), "back", "forward" or "reload".
#[tauri::command]
pub async fn spike_browser_nav(app: AppHandle, action: String, url: Option<String>) -> Result<(), String> {
    log(&app, "rust", "nav", json!({ "action": action, "url": url }));
    match action.as_str() {
        "go" => {
            let url: Url = url
                .ok_or("go needs a URL")?
                .parse()
                .map_err(|e| format!("not a URL: {e}"))?;
            if !rules::allowed_navigation(&url) {
                refuse(&app, url.as_str());
                return Err(format!("refused: {url}"));
            }
            page(&app)?.navigate(url).map_err(|e| e.to_string())
        }
        #[cfg(windows)]
        "back" | "forward" | "reload" => com::nav(&app, &action),
        _ => Err(format!("no such action: {action}")),
    }
}

/// The page's picture for the overlay freeze, as a `data:` URL (the CSP's `img-src` allows `data:`).
#[tauri::command]
pub async fn spike_browser_capture(app: AppHandle) -> Result<String, String> {
    #[cfg(windows)]
    {
        let started = std::time::Instant::now();
        log(&app, "rust", "capture-start", json!({}));
        let png = com::capture(&app)
            .await
            .inspect_err(|e| log(&app, "rust", "capture-failed", json!({ "error": e })))?;
        log(
            &app,
            "rust",
            "captured",
            json!({ "bytes": png.len(), "ms": started.elapsed().as_millis() as u64 }),
        );
        Ok(format!("data:image/png;base64,{}", rules::base64(&png)))
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("the spike captures the page on Windows only".into())
    }
}

#[tauri::command]
pub async fn spike_log(app: AppHandle, event: String, data: Value) -> Result<(), String> {
    log(&app, "front", &event, data);
    Ok(())
}
