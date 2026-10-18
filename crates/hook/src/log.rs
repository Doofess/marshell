use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const LIMIT: u64 = 1_048_576;

/// Appends `<unix_ms> <role> <message>` to `<home>/logs/bridge.log`. A log over 1 MiB is first
/// renamed to `bridge.log.1`. Every error is swallowed: logging never fails a hook. Callers
/// must keep the token and the stdin body out of `msg`.
pub fn line(home: &Path, role: &str, msg: &str) {
    let dir = home.join("logs");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("bridge.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > LIMIT) {
        let _ = std::fs::rename(&path, dir.join("bridge.log.1"));
    }
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let msg = msg.replace(['\r', '\n'], " ");
    let text = format!("{ms} {role} {msg}\n");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(text.as_bytes());
    }
}
