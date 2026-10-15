use crate::{paths, store};
use std::io::Write;
use std::path::Path;

/// The log is moved to `crash.log.1` once it grows past this, so it never grows without bound.
const MAX_LOG_BYTES: u64 = 1024 * 1024;

/// Appends one line per panic to `<home>/logs/crash.log`, then runs the hook that was there before
/// (which prints the usual message to stderr). Writing the log never panics: errors are ignored.
pub fn install_panic_hook(home: &Path) {
    let log = paths::crash_log(home);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(non-string panic)".into());
        let place = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown place".into());
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let line = format!(
            "{seconds} thread={} at {place}: {message}\n",
            thread.name().unwrap_or("unnamed")
        );
        let _ = append(&log, &line);
        previous(info);
    }));
}

fn append(log: &Path, line: &str) -> anyhow::Result<()> {
    if let Some(dir) = log.parent() {
        store::create_private_dir(dir)?;
    }
    if std::fs::metadata(log).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        std::fs::rename(log, log.with_file_name("crash.log.1"))?;
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(log)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}
