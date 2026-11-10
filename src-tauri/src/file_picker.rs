//! The native open-file dialog, opened from Rust so the webview needs no dialog permission.

use crate::folder_picker::start_folder;
use tauri_plugin_dialog::DialogExt;

/// Opens the native open-file dialog, modal to `window`, with no file-type filter. `None` when the person cancels.
#[tauri::command]
pub async fn pick_file(window: tauri::Window, start: Option<String>) -> Result<Option<String>, String> {
    // The blocking call waits on a pool thread, so neither the UI thread nor the async runtime stalls.
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = window.dialog().file().set_parent(&window);
        if let Some(dir) = start_folder(start.as_deref()) {
            dialog = dialog.set_directory(dir);
        }
        match dialog.blocking_pick_file() {
            None => Ok(None),
            Some(p) => p
                .into_path()
                .map(|p| Some(p.to_string_lossy().into_owned()))
                .map_err(|e| e.to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
