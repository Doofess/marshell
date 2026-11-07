//! Spike 6q-0: the raw WebView2 COM layer on the page's controller. Handlers only read their event and log, or post to
//! the main thread: they run inside WebView2's callback and must not call back into the app (research §2.3).

use super::rules::{KeyRoute, key_event, route_key};
use super::{LABEL, emit_app, log};
use serde_json::json;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Webview};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_KEY_EVENT_KIND, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_KEY_EVENT_KIND_KEY_UP,
    COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN, COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_UP,
    COREWEBVIEW2_MOVE_FOCUS_REASON, COREWEBVIEW2_MOVE_FOCUS_REASON_NEXT, COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS,
    COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC, COREWEBVIEW2_PHYSICAL_KEY_STATUS, ICoreWebView2Controller,
};
use webview2_com::{AcceleratorKeyPressedEventHandler, FocusChangedEventHandler};
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
