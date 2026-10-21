//! OS banners. Every banner is silent (the core picks a sound and the shell plays it), never raises the window,
//! and a click only focuses the window and tells the webview which session to select.

use std::collections::HashMap;
use tauri::{Emitter, Manager};
use tauri_plugin_notification::{Action, ActionType, NotificationExt};

/// The action types a banner can carry; their actions become its buttons.
const OPEN: &str = "marshell-open";
const REVIEW: &str = "marshell-review";

/// Registers the buttons and the click handler. Banners shown before this ran report nothing.
pub fn init(app: &tauri::AppHandle) -> tauri_plugin_notification::Result<()> {
    let n = app.notification();
    n.register_action_types(vec![
        ActionType::builder(OPEN)
            .actions(vec![Action::builder("open", "Open").build()])
            .build(),
        ActionType::builder(REVIEW)
            .actions(vec![Action::builder("review", "Review").build()])
            .build(),
    ])?;
    let handle = app.clone();
    n.on_action(move |performed| {
        let tab_id = performed
            .notification()
            .and_then(|n| n.extra().get("tab_id"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        eprintln!("Marshell: banner clicked ({}), tab {tab_id:?}", performed.action_id());
        if let Some(w) = handle.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
        let _ = handle.emit("focus-session", HashMap::from([("tab_id", tab_id)]));
    })
}
