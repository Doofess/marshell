//! Spike 6q-0: the raw WebView2 COM layer on the page's controller. Handlers only read their event and log, or post to
//! the main thread: they run inside WebView2's callback and must not call back into the app (research §2.3).

use super::rules::{KeyRoute, focus_out_direction, key_event, route_key};
use super::{LABEL, emit_app, log};
use serde_json::json;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, Webview};
use webview2_com::CapturePreviewCompletedHandler;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG, COREWEBVIEW2_KEY_EVENT_KIND, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN,
    COREWEBVIEW2_KEY_EVENT_KIND_KEY_UP, COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN,
    COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_UP, COREWEBVIEW2_MOVE_FOCUS_REASON, COREWEBVIEW2_MOVE_FOCUS_REASON_NEXT,
    COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS, COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC,
    COREWEBVIEW2_PHYSICAL_KEY_STATUS, ICoreWebView2, ICoreWebView2Controller,
};
use webview2_com::{
    AcceleratorKeyPressedEventHandler, DocumentTitleChangedEventHandler, FocusChangedEventHandler,
    HistoryChangedEventHandler, MoveFocusRequestedEventHandler, SourceChangedEventHandler, take_pwstr,
};
use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
use windows::Win32::System::Com::{IStream, STREAM_SEEK_SET};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};

static KEY_SEQ: AtomicU64 = AtomicU64::new(0);

fn down(vk: i32) -> bool {
    // SAFETY: reads this thread's key state; the handler runs on the thread that took the key message.
    unsafe { GetKeyState(vk) < 0 }
}

fn com_failed(app: &AppHandle, what: &str, e: windows_core::Error) {
    log(
        app,
        "rust",
        "com-failed",
        json!({ "what": what, "error": e.to_string() }),
    );
}

pub fn install(app: &AppHandle, page: &Webview) {
    let app = app.clone();
    let result = page.with_webview(move |pv| {
        let controller = pv.controller();
        if let Err(e) = focus_handlers(&app, &controller) {
            com_failed(&app, "focus handlers", e);
        }
        if let Err(e) = key_handler(&app, &controller) {
            com_failed(&app, "key handler", e);
        }
        if let Err(e) = focus_out_handler(&app, &controller) {
            com_failed(&app, "focus-out handler", e);
        }
        if let Err(e) = history_handlers(&app, &controller) {
            com_failed(&app, "history handlers", e);
        }
        // F12 opens DevTools on the page only: the builder turned them off, as a release build has them (research §2.3).
        // SAFETY: COM calls on the live controller, on the webview's own thread (with_webview).
        let on = unsafe {
            controller
                .CoreWebView2()
                .and_then(|w| w.Settings())
                .and_then(|s| s.SetAreDevToolsEnabled(true))
        };
        if let Err(e) = on {
            com_failed(&app, "devtools on", e);
        }
    });
    if let Err(e) = result {
        eprintln!("spike: with_webview failed: {e}");
    }
}

fn focus_handlers(app: &AppHandle, controller: &ICoreWebView2Controller) -> windows_core::Result<()> {
    let mut token = 0i64;
    let on_focus = app.clone();
    let on_blur = app.clone();
    // SAFETY: as in `install`.
    unsafe {
        controller.add_GotFocus(
            &FocusChangedEventHandler::create(Box::new(move |_, _| {
                log(&on_focus, "rust", "page-focus", json!({}));
                // The pane keeps track of who holds the keyboard, to give it back to the page after Alt+Tab.
                let app = on_focus.clone();
                let _ = on_focus.run_on_main_thread(move || emit_app(&app, "spike://page-focus", json!({})));
                Ok(())
            })),
            &mut token,
        )?;
        controller.add_LostFocus(
            &FocusChangedEventHandler::create(Box::new(move |_, _| {
                log(&on_blur, "rust", "page-blur", json!({}));
                Ok(())
            })),
            &mut token,
        )
    }
}

/// Tab past the page's last element, or Shift+Tab before its first, hands focus back to the app, which moves it to the
/// element on that side of the page. Unhandled, WebView2 would cycle focus inside the page.
fn focus_out_handler(app: &AppHandle, controller: &ICoreWebView2Controller) -> windows_core::Result<()> {
    let app = app.clone();
    let mut token = 0i64;
    let handler = MoveFocusRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let mut reason = COREWEBVIEW2_MOVE_FOCUS_REASON::default();
        // SAFETY: reading the event's own arguments inside its callback.
        unsafe { args.Reason(&mut reason)? };
        let Some(direction) = focus_out_direction(reason.0) else {
            return Ok(());
        };
        // SAFETY: as above.
        unsafe { args.SetHandled(true)? };
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(main) = app2.get_webview("main") {
                let _ = main.set_focus();
            }
            log(&app2, "rust", "focus-out", json!({ "direction": direction }));
            emit_app(&app2, "spike://focus-out", json!({ "direction": direction }));
        });
        Ok(())
    }));
    // SAFETY: as in `install`.
    unsafe { controller.add_MoveFocusRequested(&handler, &mut token) }
}

/// The page's address, title and history, read from the sender inside its own event.
fn history_of(w: &ICoreWebView2) -> windows_core::Result<serde_json::Value> {
    let (mut url, mut title) = (windows_core::PWSTR::null(), windows_core::PWSTR::null());
    let (mut back, mut forward) = (windows_core::BOOL::default(), windows_core::BOOL::default());
    // SAFETY: getters on the live webview, on its own thread; take_pwstr frees each string WebView2 allocated.
    unsafe {
        w.Source(&mut url)?;
        w.DocumentTitle(&mut title)?;
        w.CanGoBack(&mut back)?;
        w.CanGoForward(&mut forward)?;
    }
    Ok(
        json!({ "url": take_pwstr(url), "title": take_pwstr(title), "canBack": back.as_bool(), "canForward": forward.as_bool() }),
    )
}

fn send_history(app: &AppHandle, sender: Option<ICoreWebView2>, why: &'static str) -> windows_core::Result<()> {
    let Some(w) = sender else { return Ok(()) };
    let state = history_of(&w)?;
    let mut line = state.clone();
    line["why"] = why.into();
    log(app, "rust", "history", line);
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || emit_app(&app2, "spike://history", state));
    Ok(())
}

/// The bar follows the page: its address, its title and whether Back and Forward can go anywhere. The first state is
/// sent at once, since the first navigation may have started before these handlers were added.
fn history_handlers(app: &AppHandle, controller: &ICoreWebView2Controller) -> windows_core::Result<()> {
    let mut token = 0i64;
    let (a, b, c) = (app.clone(), app.clone(), app.clone());
    // SAFETY: as in `install`.
    unsafe {
        let w = controller.CoreWebView2()?;
        w.add_HistoryChanged(
            &HistoryChangedEventHandler::create(Box::new(move |s, _| send_history(&a, s, "history"))),
            &mut token,
        )?;
        w.add_SourceChanged(
            &SourceChangedEventHandler::create(Box::new(move |s, _| send_history(&b, s, "source"))),
            &mut token,
        )?;
        w.add_DocumentTitleChanged(
            &DocumentTitleChangedEventHandler::create(Box::new(move |s, _| send_history(&c, s, "title"))),
            &mut token,
        )?;
        send_history(app, Some(w), "installed")
    }
}

/// Back, Forward and Reload, on the page's own history.
pub fn nav(app: &AppHandle, action: &str) -> Result<(), String> {
    let page = app.get_webview(LABEL).ok_or("no page is open")?;
    let app2 = app.clone();
    let action = action.to_string();
    page.with_webview(move |pv| {
        // SAFETY: as in `install`.
        let done = unsafe {
            pv.controller().CoreWebView2().and_then(|w| match action.as_str() {
                "back" => w.GoBack(),
                "forward" => w.GoForward(),
                _ => w.Reload(),
            })
        };
        if let Err(e) = done {
            com_failed(&app2, &format!("nav {action}"), e);
        }
    })
    .map_err(|e| e.to_string())
}

/// Marshell's keys and F12 are taken from the page (6q decision 4). The key-up of a taken key is taken too, by its
/// virtual key, so a modifier let go first can't leak the release into the page.
fn key_handler(app: &AppHandle, controller: &ICoreWebView2Controller) -> windows_core::Result<()> {
    let app = app.clone();
    let taken: Arc<Mutex<HashSet<u32>>> = Arc::default();
    let mut token = 0i64;
    let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
        let mut vk = 0u32;
        let mut status = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
        // SAFETY: reading the event's own arguments inside its callback.
        unsafe {
            args.KeyEventKind(&mut kind)?;
            args.VirtualKey(&mut vk)?;
            args.PhysicalKeyStatus(&mut status)?;
        }
        let is_down =
            kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN || kind == COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN;
        let is_up = kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_UP || kind == COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_UP;
        let mut taken = taken.lock().unwrap_or_else(|p| p.into_inner());
        if is_up {
            if taken.remove(&vk) {
                // SAFETY: as above.
                unsafe { args.SetHandled(true)? };
            }
            return Ok(());
        }
        if !is_down {
            return Ok(());
        }
        let (ctrl, shift, alt) = (down(VK_CONTROL as i32), down(VK_SHIFT as i32), down(VK_MENU as i32));
        let route = route_key(vk, ctrl, shift, alt);
        if route == KeyRoute::Page {
            return Ok(());
        }
        taken.insert(vk);
        // SAFETY: as above.
        unsafe { args.SetHandled(true)? };
        let event = key_event(vk, ctrl, shift, alt, status.WasKeyDown.as_bool());
        let n = KEY_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || match route {
            KeyRoute::App => {
                if let Some(main) = app2.get_webview("main") {
                    let _ = main.set_focus();
                }
                log(&app2, "rust", "key-forwarded", json!({ "code": event.code, "ctrl": event.ctrl, "shift": event.shift, "alt": event.alt, "repeat": event.repeat, "n": n }));
                emit_app(&app2, "spike://key", json!({ "code": event.code, "ctrl": event.ctrl, "shift": event.shift, "alt": event.alt, "repeat": event.repeat, "n": n }));
            }
            KeyRoute::DevTools => open_devtools(&app2),
            KeyRoute::Page => {}
        });
        Ok(())
    }));
    // SAFETY: as in `install`.
    unsafe { controller.add_AcceleratorKeyPressed(&handler, &mut token) }
}

fn open_devtools(app: &AppHandle) {
    let Some(page) = app.get_webview(LABEL) else { return };
    let app2 = app.clone();
    let _ = page.with_webview(move |pv| {
        // SAFETY: as in `install`.
        let r = unsafe { pv.controller().CoreWebView2().and_then(|w| w.OpenDevToolsWindow()) };
        match r {
            Ok(()) => log(&app2, "rust", "devtools-opened", json!({})),
            Err(e) => com_failed(&app2, "open devtools", e),
        }
    });
}

/// "0x80070005 Access is denied." A failed capture's HRESULT is what Task 5 (d) records.
fn hr_text(e: &windows_core::Error) -> String {
    format!("{:#010x} {}", e.code().0 as u32, e.message())
}

fn read_all(stream: &IStream) -> windows_core::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0u8; 64 * 1024];
    // SAFETY: a stream this module created, read on the thread that owns it into a buffer of the length given.
    unsafe {
        stream.Seek(0, STREAM_SEEK_SET, None)?;
        loop {
            let mut read = 0u32;
            stream
                .Read(buf.as_mut_ptr().cast(), buf.len() as u32, Some(&mut read))
                .ok()?;
            if read == 0 {
                break;
            }
            out.extend_from_slice(&buf[..read as usize]);
        }
    }
    Ok(out)
}

/// The page as WebView2 last drew it, as PNG bytes: `CapturePreview` into a stream in memory. The call is made on the
/// webview's thread and completes there; the command's thread only waits for the bytes.
pub async fn capture(app: &AppHandle) -> Result<Vec<u8>, String> {
    let page = app.get_webview(LABEL).ok_or("no page is open")?;
    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<u8>, String>>();
    page.with_webview(move |pv| {
        let done = tx.clone();
        // SAFETY: COM calls on the live controller, on the webview's own thread (with_webview).
        let started = unsafe {
            CreateStreamOnHGlobal(HGLOBAL::default(), true).and_then(|stream| {
                let read = stream.clone();
                let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
                    let _ = done.send(result.and_then(|()| read_all(&read)).map_err(|e| hr_text(&e)));
                    Ok(())
                }));
                pv.controller().CoreWebView2()?.CapturePreview(
                    COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                    &stream,
                    &handler,
                )
            })
        };
        if let Err(e) = started {
            let _ = tx.send(Err(hr_text(&e)));
        }
    })
    .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| Err("the capture did not finish in 5 s".into()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Moves keyboard focus into the page: from the placeholder going forward or back, or from code.
pub fn focus_page(app: &AppHandle, reason: &str) -> Result<(), String> {
    let reason: COREWEBVIEW2_MOVE_FOCUS_REASON = match reason {
        "next" => COREWEBVIEW2_MOVE_FOCUS_REASON_NEXT,
        "previous" => COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS,
        _ => COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC,
    };
    let page = app.get_webview(LABEL).ok_or("no page is open")?;
    let app2 = app.clone();
    page.with_webview(move |pv| {
        // SAFETY: as in `install`.
        if let Err(e) = unsafe { pv.controller().MoveFocus(reason) } {
            com_failed(&app2, "move focus", e);
        }
    })
    .map_err(|e| e.to_string())
}
