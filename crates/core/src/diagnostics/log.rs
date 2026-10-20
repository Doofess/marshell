//! Marshell's own log: `<home>/logs/marshell.log`, redacted, capped at 5 files of 2 MiB.

use crate::diagnostics::redact::{self, Redactor};
use crate::{paths, store};
use anyhow::Context;
use marshell_protocol::diagnostics::LoggingState;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Layer, Registry, fmt, reload};

/// One log file's size cap, and how many files (the log and its numbered older copies) are kept.
const MAX_BYTES: u64 = 2 * 1024 * 1024;
const KEEP: usize = 5;
const ROTATION_RETRY_BYTES: u64 = 64 * 1024;

struct Current {
    file: Option<File>,
    len: u64,
    retry_at: u64,
    recover: bool,
}

/// A log file that moves to `<name>.1`, `.2` … once the next line would take it past `max_bytes`, and drops
/// the oldest copy so at most `keep` files exist. A line goes whole into one file.
pub struct RotatingFile {
    path: PathBuf,
    max_bytes: u64,
    keep: usize,
    current: Mutex<Current>,
}

impl RotatingFile {
    pub fn new(path: PathBuf, max_bytes: u64, keep: usize) -> RotatingFile {
        RotatingFile {
            path,
            max_bytes,
            keep: keep.max(1),
            current: Mutex::new(Current {
                file: None,
                len: 0,
                retry_at: 0,
                recover: true,
            }),
        }
    }

    fn numbered(&self, n: usize) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".{n}"));
        self.path.with_file_name(name)
    }

    fn open(&self) -> io::Result<(File, u64)> {
        if let Some(dir) = self.path.parent() {
            store::create_private_dir(dir).map_err(io::Error::other)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        let len = file.metadata()?.len();
        Ok((file, len))
    }

    fn rotating(&self) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(".rotating");
        self.path.with_file_name(name)
    }

    /// Finishes a staged rotation, including one left by a crash. Never append to the staged file.
    fn finish_rotation(&self) -> io::Result<()> {
        let rotating = self.rotating();
        if !rotating.try_exists()? {
            return Ok(());
        }
        if self.keep > 1 {
            let oldest = self.numbered(self.keep - 1);
            if oldest.exists() {
                std::fs::remove_file(&oldest)?;
            }
            for n in (1..self.keep - 1).rev() {
                let from = self.numbered(n);
                if from.exists() {
                    std::fs::rename(&from, self.numbered(n + 1))?;
                }
            }
            std::fs::rename(rotating, self.numbered(1))?;
        } else {
            std::fs::remove_file(rotating)?;
        }
        Ok(())
    }

    /// Stages the current log before touching any older copy. The caller holds the lock and has
    /// finished any pending rotation, so a crash's staged log cannot be overwritten.
    fn rotate(&self, current: &mut Current) -> io::Result<()> {
        current.file = None;
        std::fs::rename(&self.path, self.rotating())?;
        self.finish_rotation()
    }

    /// Appends a whole line, cutting lines longer than the cap. A refused rotation temporarily allows
    /// the current log past the cap; retry after another 64 KiB, without dropping any incoming lines.
    pub fn write_line(&self, line: &[u8]) -> io::Result<()> {
        let mut line = line;
        let cap = usize::try_from(self.max_bytes).unwrap_or(usize::MAX);
        let cut;
        if line.len() > cap {
            let mut kept = line[..cap.saturating_sub(1)].to_vec();
            kept.push(b'\n');
            cut = kept;
            line = &cut;
        }
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if current.file.is_none() {
            let (file, len) = self.open()?;
            current.file = Some(file);
            current.len = len;
        }
        let full = current.len > 0 && current.len + line.len() as u64 > self.max_bytes;
        if (current.recover || full) && current.len >= current.retry_at {
            let result = self
                .finish_rotation()
                .and_then(|()| if full { self.rotate(&mut current) } else { Ok(()) });
            // Reopen even when renaming failed: retaining the old length and continuing to append
            // is essential when a reader has opened the log without FILE_SHARE_DELETE.
            if current.file.is_none() {
                let (file, len) = self.open()?;
                current.file = Some(file);
                current.len = len;
            }
            current.recover = result.is_err();
            current.retry_at = if result.is_err() {
                current.len.saturating_add(ROTATION_RETRY_BYTES)
            } else {
                0
            };
        }
        let file = current.file.as_mut().expect("the log file was just opened");
        file.write_all(line)?;
        current.len += line.len() as u64;
        Ok(())
    }
}

/// Hands `tracing` a writer for each event. The event is collected whole, passed through the redactor, and
/// written as one line when the writer is dropped.
pub struct RedactingWriter {
    sink: Arc<RotatingFile>,
    redactor: &'static Redactor,
}

impl RedactingWriter {
    pub fn new(sink: Arc<RotatingFile>, redactor: &'static Redactor) -> RedactingWriter {
        RedactingWriter { sink, redactor }
    }
}

pub struct EventWriter {
    buf: Vec<u8>,
    sink: Arc<RotatingFile>,
    redactor: &'static Redactor,
}

impl Write for EventWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for EventWriter {
    fn drop(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        let text = String::from_utf8_lossy(&self.buf);
        let mut line = self.redactor.redact(&text).into_owned();
        if !line.ends_with('\n') {
            line.push('\n');
        }
        // A failing log must never take the app down, and there is nowhere to report it.
        let _ = self.sink.write_line(line.as_bytes());
    }
}

impl<'a> MakeWriter<'a> for RedactingWriter {
    type Writer = EventWriter;

    fn make_writer(&'a self) -> EventWriter {
        EventWriter {
            buf: Vec::new(),
            sink: self.sink.clone(),
            redactor: self.redactor,
        }
    }
}

type FilterHandle = reload::Handle<Targets, Registry>;

static HANDLE: OnceLock<FilterHandle> = OnceLock::new();
static DETAILED: AtomicBool = AtomicBool::new(false);

/// Marshell's crates log at info (debug when detailed); everything else only at warn.
fn targets(detailed: bool) -> Targets {
    let level = if detailed {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };
    Targets::new()
        .with_target("marshell_core", level)
        .with_target("marshell_app", level)
        .with_default(LevelFilter::WARN)
}

/// Installs the global `tracing` subscriber, writing to `<home>/logs/marshell.log`. Errors when it already
/// ran, or when something else installed a subscriber first.
pub fn init(home: &Path) -> anyhow::Result<()> {
    if HANDLE.get().is_some() {
        anyhow::bail!("logging is already running");
    }
    let sink = Arc::new(RotatingFile::new(paths::marshell_log(home), MAX_BYTES, KEEP));
    let writer = RedactingWriter::new(sink, redact::global());
    let (filter, handle) = reload::Layer::new(targets(false));
    let layer = fmt::layer().with_ansi(false).with_writer(writer).with_filter(filter);
    tracing_subscriber::registry()
        .with(layer)
        .try_init()
        .context("installing the log")?;
    DETAILED.store(false, Ordering::Relaxed);
    HANDLE
        .set(handle)
        .map_err(|_| anyhow::anyhow!("logging is already running"))
}

/// Switches Marshell's crates to debug (or back to info). False when `init` never ran.
pub fn set_detailed(on: bool) -> bool {
    let Some(handle) = HANDLE.get() else {
        return false;
    };
    if handle.reload(targets(on)).is_err() {
        return false;
    }
    DETAILED.store(on, Ordering::Relaxed);
    true
}

pub fn state() -> LoggingState {
    LoggingState {
        available: HANDLE.get().is_some(),
        detailed: DETAILED.load(Ordering::Relaxed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Arc;
    use tracing_subscriber::fmt::MakeWriter;

    fn files_in(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut found: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("marshell.log"))
            .collect();
        found.sort();
        found
    }

    #[test]
    fn writes_lines_and_rotates_at_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        let log = RotatingFile::new(dir.path().join("marshell.log"), 100, 5);
        for i in 0..30 {
            // 20 bytes with the newline.
            let line = format!("line {i:02} xxxxxxxxxxx\n");
            assert_eq!(line.len(), 20);
            log.write_line(line.as_bytes()).unwrap();
        }
        let files = files_in(dir.path());
        let names: Vec<_> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            [
                "marshell.log",
                "marshell.log.1",
                "marshell.log.2",
                "marshell.log.3",
                "marshell.log.4"
            ]
        );
        for f in &files {
            assert!(std::fs::metadata(f).unwrap().len() <= 100, "{f:?}");
        }
        let newest = std::fs::read_to_string(dir.path().join("marshell.log")).unwrap();
        assert_eq!(newest.lines().last().unwrap(), "line 29 xxxxxxxxxxx");
    }

    #[test]
    #[cfg(windows)]
    fn a_refused_rotation_keeps_older_files_and_every_line() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("marshell.log");
        let log = RotatingFile::new(path.clone(), 128 * 1024, 5);
        for n in 1..5 {
            std::fs::write(log.numbered(n), format!("older {n}\n")).unwrap();
        }
        let held = OpenOptions::new()
            .create(true)
            .append(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(&path)
            .unwrap();
        let lines: Vec<_> = (0..194).map(|i| format!("{i:03} {}\n", "x".repeat(1019))).collect();
        for line in &lines[..132] {
            assert_eq!(line.len(), 1024);
            log.write_line(line.as_bytes()).unwrap();
        }
        for n in 1..5 {
            assert_eq!(
                std::fs::read_to_string(log.numbered(n)).unwrap(),
                format!("older {n}\n")
            );
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), lines[..132].concat());
        drop(held);
        // A successful append does not immediately retry the refused rotation.
        log.write_line(lines[132].as_bytes()).unwrap();
        assert_eq!(std::fs::read_to_string(log.numbered(1)).unwrap(), "older 1\n");
        for line in &lines[133..] {
            log.write_line(line.as_bytes()).unwrap();
        }
        let files = files_in(dir.path());
        assert_eq!(files.len(), 5);
        assert!(!dir.path().join("marshell.log.rotating").exists());
        let text = files
            .iter()
            .map(|p| std::fs::read_to_string(p).unwrap())
            .collect::<String>();
        for line in &lines {
            assert_eq!(text.lines().filter(|found| *found == line.trim_end()).count(), 1);
        }
        assert!(std::fs::read_to_string(log.numbered(1)).unwrap().starts_with(&lines[0]));
    }

    #[test]
    fn a_leftover_rotation_is_recovered_before_appending() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("marshell.log");
        std::fs::write(dir.path().join("marshell.log.rotating"), "before crash\n").unwrap();
        std::fs::write(&path, "after crash\n").unwrap();
        let log = RotatingFile::new(path.clone(), 100, 5);
        for n in 1..5 {
            std::fs::write(log.numbered(n), format!("older {n}\n")).unwrap();
        }
        log.write_line(b"new line\n").unwrap();
        assert_eq!(std::fs::read_to_string(log.numbered(1)).unwrap(), "before crash\n");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "after crash\nnew line\n");
        assert_eq!(files_in(dir.path()).len(), 5);
    }

    #[test]
    fn rotation_keeps_five_whole_files_under_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        let log = Arc::new(RotatingFile::new(dir.path().join("marshell.log"), 4096, 5));
        let threads: Vec<_> = (0..8)
            .map(|n| {
                let log = log.clone();
                std::thread::spawn(move || {
                    for i in 0..500 {
                        let line = format!("t{n} {i} {}\n", "x".repeat(40));
                        log.write_line(line.as_bytes()).unwrap();
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        let files = files_in(dir.path());
        assert!(files.len() <= 5, "{files:?}");
        let whole = regex::Regex::new(r"^t\d+ \d+ x{40}$").unwrap();
        for f in &files {
            assert!(std::fs::metadata(f).unwrap().len() <= 4096, "{f:?}");
            for line in std::fs::read_to_string(f).unwrap().lines() {
                assert!(whole.is_match(line), "torn line {line:?} in {f:?}");
            }
        }
    }

    #[test]
    fn log_lines_are_redacted() {
        let dir = tempfile::tempdir().unwrap();
        let file = Arc::new(RotatingFile::new(dir.path().join("marshell.log"), 4096, 5));
        let redactor: &'static crate::diagnostics::redact::Redactor =
            Box::leak(Box::new(crate::diagnostics::redact::Redactor::new(None, None, None)));
        let make = RedactingWriter::new(file, redactor);
        let mut w = make.make_writer();
        w.write_all(b"key sk-ant-abcdefghij").unwrap();
        drop(w);
        let text = std::fs::read_to_string(dir.path().join("marshell.log")).unwrap();
        assert_eq!(text, "key <anthropic-key>\n");
    }
}
