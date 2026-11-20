//! The native window's theme.
//!
//! WebView2 draws what a page can't style, the right-click menu above all, in the theme of the window the webview is in.
//! A window left alone has the system's theme, so a light Marshell on a dark Windows showed a dark menu. The UI sends the
//! Appearance theme here on load and whenever it changes (PLAN §9, Soft surfaces decision 5).

use tauri::Theme;

/// The theme a name stands for: `None` follows the system.
fn theme_of(name: &str) -> Result<Option<Theme>, String> {
    match name {
        "light" => Ok(Some(Theme::Light)),
        "dark" => Ok(Some(Theme::Dark)),
        "system" => Ok(None),
        other => Err(format!("unknown theme: {other}")),
    }
}

/// Sets the calling window's theme to "light" or "dark", or back to the system's with "system". The window passes it on
/// to every webview in it.
#[tauri::command]
pub fn set_window_theme(window: tauri::Window, theme: String) -> Result<(), String> {
    window.set_theme(theme_of(&theme)?).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_theme_is_that_theme() {
        assert_eq!(theme_of("light"), Ok(Some(Theme::Light)));
        assert_eq!(theme_of("dark"), Ok(Some(Theme::Dark)));
    }

    #[test]
    fn system_follows_the_system() {
        assert_eq!(theme_of("system"), Ok(None));
    }

    /// Tauri's own parse reads anything it doesn't know as light.
    #[test]
    fn an_unknown_name_is_refused() {
        assert_eq!(theme_of("sepia"), Err("unknown theme: sepia".to_string()));
        assert!(theme_of("").is_err());
    }
}
