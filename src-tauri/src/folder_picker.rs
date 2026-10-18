//! The native folder picker, opened from Rust so the webview needs no dialog permission.

use std::path::{Path, PathBuf};
use tauri_plugin_dialog::DialogExt;

/// The folder the dialog opens in: `start`, only when it is an existing folder.
fn start_folder(start: Option<&str>) -> Option<PathBuf> {
    let path = Path::new(start?);
    path.is_dir().then(|| path.to_path_buf())
}

/// Opens the native folder picker, modal to `window`. `None` when the person cancels.
#[tauri::command]
pub async fn pick_folder(window: tauri::Window, start: Option<String>) -> Result<Option<String>, String> {
    // The blocking call waits on a pool thread, so neither the UI thread nor the async runtime stalls.
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = window.dialog().file().set_parent(&window);
        if let Some(dir) = start_folder(start.as_deref()) {
            dialog = dialog.set_directory(dir);
        }
        match dialog.blocking_pick_folder() {
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

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("marshell-picker-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_existing_folder_is_the_start() {
        let dir = scratch("dir");
        assert_eq!(start_folder(dir.to_str()), Some(dir.clone()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_folder_is_no_start() {
        let dir = scratch("missing");
        let gone = dir.join("nope");
        assert_eq!(start_folder(gone.to_str()), None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn no_start_is_no_start() {
        assert_eq!(start_folder(None), None);
    }

    #[test]
    fn a_file_is_no_start() {
        let dir = scratch("file");
        let file = dir.join("f.txt");
        std::fs::write(&file, "x").unwrap();
        assert_eq!(start_folder(file.to_str()), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
