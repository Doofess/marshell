use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Keeps a name safe to use as a file or folder name.
fn clean(s: &str) -> String {
    let s: String = s
        .chars()
        .take(64)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() { "unknown".into() } else { s }
}

/// The folder a recording goes in: `MARSHELL_RECORD_DIR`, else `<home>/fixtures/<agent>`.
pub fn dir(env: &dyn Fn(&str) -> Option<String>, home: &Path, agent: &str) -> PathBuf {
    match env(&marshell_protocol::brand::env_var("RECORD_DIR")).filter(|d| !d.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => home.join("fixtures").join(clean(agent)),
    }
}

/// Writes `body` as `<hook_event_name or role>.json` in `dir`. The second save of a name becomes
/// `<name>-2.json`, and so on. Never overwrites a file.
pub fn save(dir: &Path, role: &str, body: &[u8]) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let name = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("hook_event_name")?.as_str().map(clean))
        .unwrap_or_else(|| clean(role));
    for n in 1..=10_000u32 {
        let file = if n == 1 {
            format!("{name}.json")
        } else {
            format!("{name}-{n}.json")
        };
        let path = dir.join(file);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                f.write_all(body)?;
                return Ok(path);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("too many recordings with one name"))
}
