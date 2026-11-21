//! The native open-file dialog, opened from Rust so the webview needs no dialog permission.

use crate::folder_picker::start_folder;
use tauri_plugin_dialog::DialogExt;

/// The file-type filter a `kind` asks for: a name and its extensions. Only "html" has one.
fn filter_for(kind: Option<&str>) -> Option<(&'static str, &'static [&'static str])> {
    match kind {
        Some("html") => Some(("HTML files", &["html", "htm"])),
        _ => None,
    }
}

/// Opens the native open-file dialog, modal to `window`, with the filter `kind` names (none for no kind). `None` when the person cancels.
#[tauri::command]
pub async fn pick_file(
    window: tauri::Window,
    start: Option<String>,
    kind: Option<String>,
) -> Result<Option<String>, String> {
    // The blocking call waits on a pool thread, so neither the UI thread nor the async runtime stalls.
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = window.dialog().file().set_parent(&window);
        if let Some(dir) = start_folder(start.as_deref()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some((name, extensions)) = filter_for(kind.as_deref()) {
            dialog = dialog.add_filter(name, extensions);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_gets_the_html_filter() {
        assert_eq!(filter_for(Some("html")), Some(("HTML files", &["html", "htm"][..])));
    }

    #[test]
    fn any_other_kind_gets_none() {
        assert_eq!(filter_for(None), None);
        assert_eq!(filter_for(Some("pdf")), None);
    }
}
