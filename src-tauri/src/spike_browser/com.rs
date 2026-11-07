//! Spike 6q-0: the raw WebView2 COM layer on the page's controller. Handlers only read their event and log, or post to
//! the main thread: they run inside WebView2's callback and must not call back into the app (research §2.3).

use super::log;
use serde_json::json;
use tauri::{AppHandle, Webview};
use webview2_com::FocusChangedEventHandler;

pub fn install(app: &AppHandle, page: &Webview) {
    let app = app.clone();
    let result = page.with_webview(move |pv| {
        let controller = pv.controller();
        let mut token = 0i64;
        let on_focus = app.clone();
        let on_blur = app.clone();
        // SAFETY: COM calls on the live controller, on the webview's own thread (with_webview).
        let r = unsafe {
            controller
                .add_GotFocus(
                    &FocusChangedEventHandler::create(Box::new(move |_, _| {
                        log(&on_focus, "rust", "page-focus", json!({}));
                        Ok(())
                    })),
                    &mut token,
                )
                .and_then(|_| {
                    controller.add_LostFocus(
                        &FocusChangedEventHandler::create(Box::new(move |_, _| {
                            log(&on_blur, "rust", "page-blur", json!({}));
                            Ok(())
                        })),
                        &mut token,
                    )
                })
        };
        if let Err(e) = r {
            log(
                &app,
                "rust",
                "com-failed",
                json!({ "what": "focus handlers", "error": e.to_string() }),
            );
        }
    });
    if let Err(e) = result {
        eprintln!("spike: with_webview failed: {e}");
    }
}
