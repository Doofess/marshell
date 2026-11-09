//! The native window's background colour.
//!
//! Windows fills a resized window with it before the webview has drawn at the new size. The default is white, which shows as
//! a strip at the moving edge of a dark app, so the UI sends its own background here on load and whenever its theme changes
//! (PLAN §9, 6q-0 decisions 8 and 10).

use tauri::window::Color;

/// Sets the calling window's background to an opaque sRGB colour.
#[tauri::command]
pub fn set_window_background(window: tauri::Window, rgb: [u8; 3]) -> Result<(), String> {
    let [r, g, b] = rgb;
    window
        .set_background_color(Some(Color(r, g, b, 255)))
        .map_err(|e| e.to_string())
}
