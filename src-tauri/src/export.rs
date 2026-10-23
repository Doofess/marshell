//! Saves a conversation export where the person chooses. The core builds the text; only this command writes
//! it, through the native Save dialog opened from Rust, as `save_diagnostics` does, so the webview needs no
//! file or dialog permission and no route takes a path to write.

use crate::diagnostics::write_export;
use tauri::ipc::{InvokeBody, Request};
use tauri_plugin_dialog::DialogExt;

const NOT_AN_EXPORT: &str = "Not a conversation export.";
const MAX_NAME: usize = 60;
const MAX_BYTES: usize = 64 * 1024 * 1024;

/// The file's extension when the name is `<words>-YYYY-MM-DD.md` or `.json`: lower-case letters and digits
/// in words joined by single hyphens, then the date. At most 60 characters.
fn extension_of(name: &str) -> Option<&'static str> {
    if name.len() > MAX_NAME {
        return None;
    }
    let (stem, extension) = if let Some(stem) = name.strip_suffix(".md") {
        (stem, "md")
    } else {
        (name.strip_suffix(".json")?, "json")
    };
    // `words-YYYY-MM-DD`: the date is the last 10 bytes, after a hyphen that follows the words.
    let split = stem.len().checked_sub(11)?;
    let (words, date) = (stem.get(..split)?, stem.get(split..)?.strip_prefix('-')?);
    let date_ok = date.bytes().enumerate().all(|(i, b)| {
        if i == 4 || i == 7 {
            b == b'-'
        } else {
            b.is_ascii_digit()
        }
    });
    let words_ok = words
        .split('-')
        .all(|word| !word.is_empty() && word.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()));
    (date_ok && words_ok).then_some(extension)
}

/// The name is a conversation export's and the bytes are UTF-8 text of at most 64 MiB. Answers the
/// extension the name carries.
fn check(name: &str, bytes: &[u8]) -> Result<&'static str, String> {
    let extension = extension_of(name).ok_or_else(|| NOT_AN_EXPORT.to_string())?;
    if bytes.len() > MAX_BYTES || std::str::from_utf8(bytes).is_err() {
        return Err(NOT_AN_EXPORT.to_string());
    }
    Ok(extension)
}

/// Asks where to save the export (UTF-8 text, named by the `x-file-name` header) and writes it there.
/// `None` when the person cancels.
#[tauri::command]
pub async fn save_conversation(window: tauri::Window, request: Request<'_>) -> Result<Option<String>, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(NOT_AN_EXPORT.to_string());
    };
    let name = request
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let extension = check(&name, bytes)?;
    let label = if extension == "md" { "Markdown" } else { "JSON" };
    let bytes = bytes.clone();
    // The blocking call waits on a pool thread, so neither the UI thread nor the async runtime stalls.
    tauri::async_runtime::spawn_blocking(move || {
        let picked = window
            .dialog()
            .file()
            .set_parent(&window)
            .set_file_name(&name)
            .add_filter(label, &[extension])
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

    #[test]
    fn save_name_check_table() {
        let text = "# Chat\n".as_bytes();
        assert_eq!(check("my-chat-2026-10-23.md", text), Ok("md"));
        assert_eq!(check("conversation-2026-10-23.json", b"{}"), Ok("json"));
        assert_eq!(check("a-2026-10-23.md", b""), Ok("md"));
        // The longest name that is allowed, and one character more.
        let long = format!("{}-2026-10-23.md", "a".repeat(MAX_NAME - 14));
        assert_eq!(long.len(), MAX_NAME);
        assert_eq!(check(&long, text), Ok("md"));
        let too_long = format!("{}-2026-10-23.md", "a".repeat(MAX_NAME - 13));
        assert_eq!(too_long.len(), 61);
        let refused = [
            "",
            "../x-2026-10-23.md",
            r"..\x-2026-10-23.md",
            "a/x-2026-10-23.md",
            "x-2026-10-23.md.exe",
            "x-2026-10-23.txt",
            "x-2026-10-23.zip",
            "X-2026-10-23.md",
            "x-2026-10-23.MD",
            "x-2026-10-23",
            "-2026-10-23.md",
            "2026-10-23.md",
            "x--y-2026-10-23.md",
            "x-y--2026-10-23.md",
            "-x-2026-10-23.md",
            "x_y-2026-10-23.md",
            "x y-2026-10-23.md",
            "x-2026-1-23.md",
            "x-2026-10-2a.md",
            "x-２０２６-10-23.md",
            "é-2026-10-23.md",
            "x-2026-10-23.md\0",
            "con-2026-10-23.md\n",
            too_long.as_str(),
        ];
        for name in refused {
            assert_eq!(check(name, text), Err(NOT_AN_EXPORT.to_string()), "{name:?}");
        }
        // Bytes that are not UTF-8 text, and more than 64 MiB.
        for bytes in [&[0xff, 0xfe, 0x41][..], b"ok \xc3", b"\x80"] {
            assert_eq!(
                check("my-chat-2026-10-23.md", bytes),
                Err(NOT_AN_EXPORT.to_string()),
                "{bytes:?}"
            );
        }
        let big = vec![b'a'; MAX_BYTES + 1];
        assert_eq!(check("my-chat-2026-10-23.md", &big), Err(NOT_AN_EXPORT.to_string()));
        assert_eq!(check("my-chat-2026-10-23.md", &big[..MAX_BYTES]), Ok("md"));
    }
}
