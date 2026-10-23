//! OS banners. Every banner is silent (the core picks a sound and the shell plays it), never raises the window,
//! and a click only focuses the window and tells the webview which session to select.

use crate::attention;
#[cfg(not(windows))]
use crate::tray;
#[cfg(not(windows))]
use marshell_core::notify::BannerAction;
use marshell_core::notify::{Banner, NotifyRequest};
#[cfg(not(windows))]
use std::collections::HashMap;
#[cfg(not(windows))]
use tauri::Emitter;
#[cfg(not(windows))]
use tauri_plugin_notification::{Action, ActionType, NotificationExt};

/// The action types a banner can carry; their actions become its buttons.
#[cfg(not(windows))]
const OPEN: &str = "marshell-open";
#[cfg(not(windows))]
const REVIEW: &str = "marshell-review";

/// On Windows the shell builds its toasts itself (toast.rs), so there is nothing to register.
#[cfg(windows)]
pub fn init(_app: &tauri::AppHandle) -> tauri_plugin_notification::Result<()> {
    Ok(())
}

/// Registers the buttons and the click handler. Banners shown before this ran report nothing.
#[cfg(not(windows))]
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
        tray::show_main(&handle);
        let _ = handle.emit("focus-session", HashMap::from([("tab_id", tab_id)]));
    })
}

/// Tells the user what the core decided: the banner, then the sound and the flash unless Windows asks apps to
/// keep quiet. Called from the core's notifier task; nothing here waits on the user.
pub fn deliver(app: &tauri::AppHandle, request: NotifyRequest) {
    if let Some(banner) = &request.banner
        && let Err(e) = show(app, banner)
    {
        eprintln!("Marshell: could not show a banner: {e}");
    }
    if request.sound.is_none() && !request.flash {
        return;
    }
    if attention::os_is_quiet() {
        return;
    }
    if let Some(kind) = request.sound {
        attention::play_sound(kind);
    }
    if request.flash {
        attention::flash(app);
    }
}

/// The banner's second line: what happened, then which session. The crate sets both as text, never markup.
pub(crate) fn body_of(banner: &Banner) -> String {
    format!(
        "{}
{}",
        banner.body, banner.meta
    )
}

/// Windows banners are Marshell's own toasts, which carry the session's link.
#[cfg(windows)]
fn show(app: &tauri::AppHandle, banner: &Banner) -> windows::core::Result<()> {
    crate::toast::show(app, banner)
}

#[cfg(not(windows))]
fn show(app: &tauri::AppHandle, banner: &Banner) -> tauri_plugin_notification::Result<()> {
    let action = match banner.action {
        BannerAction::Open => OPEN,
        BannerAction::Review => REVIEW,
    };
    let mut b = app
        .notification()
        .builder()
        .title(&banner.title)
        .body(body_of(banner))
        .action_type_id(action);
    if let Some(tab) = &banner.tab_id {
        b = b.extra("tab_id", tab);
    }
    b.show()
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_core::notify::BannerAction;

    #[test]
    fn the_body_names_the_session_under_what_happened() {
        let banner = Banner {
            title: "api needs you".into(),
            body: "Wants to run `npm test` <&\"'>".into(),
            meta: "Session 3 · api".into(),
            action: BannerAction::Open,
            tab_id: Some("t3".into()),
        };
        // Handed over as is: the crate writes it into the toast with SetInnerText, so nothing is escaped twice.
        assert_eq!(
            body_of(&banner),
            "Wants to run `npm test` <&\"'>
Session 3 · api"
        );
    }
}
