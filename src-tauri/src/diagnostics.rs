//! Saves a diagnostics export where the person chooses. The core builds the zip in memory; only this
//! command writes it, through the native Save dialog opened from Rust, so the webview needs no file or
//! dialog permission and no route takes a path to write.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tauri::ipc::{InvokeBody, Request};
use tauri_plugin_dialog::DialogExt;

const NOT_AN_EXPORT: &str = "Not a diagnostics export.";

/// The name is `marshell-diagnostics-YYYY-MM-DD.zip` and the bytes start like a zip.
fn check(name: &str, bytes: &[u8]) -> Result<(), String> {
    let date = name
        .strip_prefix("marshell-diagnostics-")
        .and_then(|rest| rest.strip_suffix(".zip"));
    let date_ok = date.is_some_and(|d| {
        d.len() == 10
            && d.bytes().enumerate().all(|(i, b)| {
                if i == 4 || i == 7 {
                    b == b'-'
                } else {
                    b.is_ascii_digit()
                }
            })
    });
    if date_ok && bytes.starts_with(b"PK\x03\x04") {
        Ok(())
    } else {
        Err(NOT_AN_EXPORT.to_string())
    }
}

/// Writes `bytes` to `<path>.part` beside `path`, then renames it over `path`. A failed write leaves no
/// `.part` behind.
pub(crate) fn write_export(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut part = path.as_os_str().to_os_string();
    part.push(".part");
    let part = PathBuf::from(part);
    let written = (|| {
        let mut file = std::fs::File::create(&part)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&part, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    written
}

/// Asks where to save the export (the zip bytes, named by the `x-file-name` header) and writes it there.
/// `None` when the person cancels.
#[tauri::command]
pub async fn save_diagnostics(window: tauri::Window, request: Request<'_>) -> Result<Option<String>, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(NOT_AN_EXPORT.to_string());
    };
    let name = request
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    check(&name, bytes)?;
    let bytes = bytes.clone();
    // The blocking call waits on a pool thread, so neither the UI thread nor the async runtime stalls.
    tauri::async_runtime::spawn_blocking(move || {
        let picked = window
            .dialog()
            .file()
            .set_parent(&window)
            .set_file_name(&name)
            .add_filter("Zip archive", &["zip"])
            .blocking_save_file();
        let Some(picked) = picked else {
            return Ok(None);
        };
        let path = picked.into_path().map_err(|e| e.to_string())?;
        write_export(&path, &bytes).map_err(|e| format!("The export could not be saved: {e}"))?;
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("marshell-export-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn checks_name_and_zip_magic() {
        let zip = b"PK\x03\x04rest";
        assert_eq!(check("marshell-diagnostics-2026-10-20.zip", zip), Ok(()));
        for name in [
            "",
            "marshell-diagnostics-2026-10-20.zip.exe",
            "marshell-diagnostics-2026-1-20.zip",
            "marshell-diagnostics-2026-10-2a.zip",
            "../marshell-diagnostics-2026-10-20.zip",
            r"..\marshell-diagnostics-2026-10-20.zip",
            "Marshell-diagnostics-2026-10-20.zip",
            "marshell-diagnostics-2026-10-20.ZIP",
            "marshell-diagnostics-２０２６-10-20.zip",
        ] {
            assert_eq!(check(name, zip), Err(NOT_AN_EXPORT.to_string()), "{name}");
        }
        for bytes in [&b""[..], b"PK", b"PK\x05\x06", b"MZ\x90\x00"] {
            assert_eq!(
                check("marshell-diagnostics-2026-10-20.zip", bytes),
                Err(NOT_AN_EXPORT.to_string())
            );
        }
    }

    #[test]
    fn write_export_replaces_and_leaves_no_part() {
        let dir = scratch("replace");
        let target = dir.join("marshell-diagnostics-2026-10-20.zip");
        std::fs::write(&target, b"old").unwrap();
        write_export(&target, b"PK\x03\x04new").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"PK\x03\x04new");
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![target.file_name().unwrap().to_os_string()]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn write_export_failure_leaves_nothing() {
        let dir = scratch("fail");
        let not_a_folder = dir.join("file");
        std::fs::write(&not_a_folder, b"x").unwrap();
        let target = not_a_folder.join("marshell-diagnostics-2026-10-20.zip");
        assert!(write_export(&target, b"PK\x03\x04").is_err());
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("file")]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
