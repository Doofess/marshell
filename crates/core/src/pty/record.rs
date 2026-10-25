//! Asciicast v2 recording of a session: output, resizes and prompt marks. Keystrokes are never written.

use crate::store::create_private_dir;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::time::{Duration, Instant};

/// What the first line of a recording says about the session.
pub struct CastHeader {
    pub cols: u16,
    pub rows: u16,
    pub started_unix: u64,
    pub shell: Option<String>,
}

/// The v2 header line, newline included. SHELL is left out when it is not known.
pub fn header_line(h: &CastHeader) -> String {
    let mut env = serde_json::json!({ "TERM": "xterm-256color" });
    if let Some(shell) = &h.shell {
        env["SHELL"] = shell.as_str().into();
    }
    let header = serde_json::json!({
        "version": 2,
        "width": h.cols,
        "height": h.rows,
        "timestamp": h.started_unix,
        "env": env,
    });
    format!("{header}\n")
}

/// Turns pty bytes into event lines. A character split across two reads is held back until it is whole.
#[derive(Default)]
pub struct CastEncoder {
    carry: Vec<u8>,
}

impl CastEncoder {
    /// An `"o"` event, or `None` when all that is left is the start of a character.
    pub fn output(&mut self, t: f64, bytes: &[u8]) -> Option<String> {
        self.carry.extend_from_slice(bytes);
        let cut = complete_prefix(&self.carry);
        let rest = self.carry.split_off(cut);
        let whole = std::mem::replace(&mut self.carry, rest);
        if whole.is_empty() {
            return None;
        }
        Some(event(t, "o", &String::from_utf8_lossy(&whole)))
    }

    pub fn resize(t: f64, cols: u16, rows: u16) -> String {
        event(t, "r", &format!("{cols}x{rows}"))
    }

    pub fn marker(t: f64, label: &str) -> String {
        event(t, "m", label)
    }
}

/// How much of `buf` can be decoded now: all of it, less an incomplete character at the end (at most 3 bytes).
/// Invalid bytes in the middle count as decodable; they become U+FFFD.
fn complete_prefix(buf: &[u8]) -> usize {
    let mut pos = 0;
    loop {
        match std::str::from_utf8(&buf[pos..]) {
            Ok(_) => return buf.len(),
            Err(e) => match e.error_len() {
                Some(n) => pos += e.valid_up_to() + n,
                None => return pos + e.valid_up_to(),
            },
        }
    }
}

fn event(t: f64, code: &str, data: &str) -> String {
    let data = serde_json::to_string(data).expect("a string always encodes");
    format!("[{t:.6},\"{code}\",{data}]\n")
}

/// A recording stops growing here; runaway output (`yes`) cannot fill the disk.
pub const MAX_RECORDING_BYTES: u64 = 256 * 1024 * 1024;

/// A 633 sequence longer than this is not followed any further.
const MAX_OSC633: usize = 64 * 1024;

const LIMIT_MARKER: &str = "Recording stopped: it reached the size limit.";

const BEHIND_MARKER: &str = "Recording stopped: it fell behind.";

/// Events waiting for the writer. A full queue means the disk is not keeping up.
const QUEUE: usize = 1024;

enum Event {
    Output(f64, Vec<u8>),
    Resize(f64, u16, u16),
    Marker(f64, String),
    Close,
}

/// Writes one session's recording from its own thread, so disk I/O never runs on the pty reader
/// thread or under the output lock. If the disk fails the recording stops and the session goes on.
pub struct Recorder {
    path: PathBuf,
    started: Instant,
    tx: SyncSender<Event>,
    /// Set by the writer when an I/O error stopped it, or by a sender that found the writer gone.
    failed: Arc<AtomicBool>,
    /// Set by a sender that found the queue full. Nothing more is sent; the writer ends the file with a marker.
    behind: Arc<AtomicBool>,
    /// Set before sending Close. Also lets the writer close when the queue has no room for it.
    closing: Arc<AtomicBool>,
    /// Set by the writer only after it has dropped the file.
    closed: Arc<AtomicBool>,
    #[cfg(test)]
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Recorder {
    pub fn start(path: PathBuf, header: CastHeader) -> anyhow::Result<Recorder> {
        Self::start_with_limit(path, header, MAX_RECORDING_BYTES)
    }

    pub fn start_with_limit(path: PathBuf, header: CastHeader, limit: u64) -> anyhow::Result<Recorder> {
        Self::start_inner(path, header, limit, QUEUE, None)
    }

    /// `gate`, in tests only, holds the writer back until it receives, as a disk that hangs would.
    fn start_inner(
        path: PathBuf,
        header: CastHeader,
        limit: u64,
        queue: usize,
        gate: Option<Receiver<()>>,
    ) -> anyhow::Result<Recorder> {
        if let Some(dir) = path.parent() {
            create_private_dir(dir)?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = BufWriter::new(options.open(&path)?);
        let head = header_line(&header);
        file.write_all(head.as_bytes())?;
        file.flush()?;
        let (tx, rx) = sync_channel::<Event>(queue);
        let failed = Arc::new(AtomicBool::new(false));
        let behind = Arc::new(AtomicBool::new(false));
        let closing = Arc::new(AtomicBool::new(false));
        let closed = Arc::new(AtomicBool::new(false));
        let writer = Writer {
            file,
            written: head.len() as u64,
            limit,
            limited: false,
            behind: behind.clone(),
            closing: closing.clone(),
            last_t: 0.0,
            encoder: CastEncoder::default(),
        };
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let thread_failed = failed.clone();
        let thread_path = path.clone();
        let thread_closed = closed.clone();
        let _handle = std::thread::Builder::new().name(format!("rec-{stem}")).spawn(move || {
            if let Some(gate) = gate {
                let _ = gate.recv();
            }
            if let Err(e) = writer.run(rx) {
                thread_failed.store(true, Ordering::SeqCst);
                tracing::warn!("recording {} stopped: {e}", thread_path.display());
            }
            thread_closed.store(true, Ordering::SeqCst);
        })?;
        Ok(Recorder {
            path,
            started: Instant::now(),
            tx,
            failed,
            behind,
            closing,
            closed,
            #[cfg(test)]
            thread: Some(_handle),
        })
    }

    pub fn output(&self, bytes: &[u8]) {
        self.send(Event::Output(self.now(), bytes.to_vec()));
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        self.send(Event::Resize(self.now(), cols, rows));
    }

    pub fn marker(&self, label: &str) {
        self.send(Event::Marker(self.now(), label.to_owned()));
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// False once an I/O error stopped the recording. A recording that hit the size limit is still kept.
    pub fn is_recording(&self) -> bool {
        !self.failed.load(Ordering::SeqCst)
    }

    /// Requests a flush and close after the queued events, without waiting for the disk.
    pub fn close(&self) {
        if !self.closing.swap(true, Ordering::SeqCst) {
            // A full queue still closes: the writer sees `closing` when it has drained it.
            // Unlike a dropped output event, this does not mean the recording fell behind.
            let _ = self.tx.try_send(Event::Close);
        }
    }

    /// True once the writer has flushed (or failed) and dropped its file.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// Never waits: a disk that hangs must not hold up the session. A full queue stops the recording
    /// rather than dropping single events, which would leave a gap in the replay.
    fn send(&self, event: Event) {
        if self.closing.load(Ordering::SeqCst)
            || self.failed.load(Ordering::Relaxed)
            || self.behind.load(Ordering::Relaxed)
        {
            return;
        }
        self.enqueue(event);
    }

    /// The nonblocking send after the initial flag check; close may race with this step.
    fn enqueue(&self, event: Event) {
        match self.tx.try_send(event) {
            Ok(()) => {}
            // Close can fill the queue or drop the receiver after `send` checked the flag.
            Err(_) if self.closing.load(Ordering::SeqCst) => {}
            Err(TrySendError::Full(_)) => self.behind.store(true, Ordering::SeqCst),
            Err(TrySendError::Disconnected(_)) => self.failed.store(true, Ordering::SeqCst),
        }
    }

    /// Closes the channel and waits for the writer to flush.
    #[cfg(test)]
    fn finish(mut self) {
        let thread = self.thread.take();
        drop(self);
        if let Some(thread) = thread {
            thread.join().unwrap();
        }
    }
}

struct Writer {
    file: BufWriter<std::fs::File>,
    written: u64,
    limit: u64,
    limited: bool,
    behind: Arc<AtomicBool>,
    closing: Arc<AtomicBool>,
    /// Events come from more than one thread; a time never goes backwards in the file.
    last_t: f64,
    encoder: CastEncoder,
}

impl Writer {
    /// Runs until close or every sender is gone. Flushes about once a second, so a crash loses at most that much.
    fn run(mut self, rx: Receiver<Event>) -> std::io::Result<()> {
        loop {
            // Observe close before checking for an empty queue: a close arriving during a
            // flush may have queued output before it, and that output must still be drained.
            let closing = self.closing.load(Ordering::SeqCst);
            let timeout = if closing {
                Duration::ZERO
            } else {
                Duration::from_secs(1)
            };
            match rx.recv_timeout(timeout) {
                Ok(Event::Close) => {
                    self.stop_if_behind()?;
                    return self.file.flush();
                }
                Ok(event) => self.write(event)?,
                Err(RecvTimeoutError::Timeout) => {
                    self.stop_if_behind()?;
                    self.file.flush()?;
                    if closing {
                        return Ok(());
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.stop_if_behind()?;
                    return self.file.flush();
                }
            }
        }
    }

    /// Once the sender gave up, the queue holds only what came before; when it is empty the file ends here.
    fn stop_if_behind(&mut self) -> std::io::Result<()> {
        if self.limited || !self.behind.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.limited = true;
        self.file
            .write_all(CastEncoder::marker(self.last_t, BEHIND_MARKER).as_bytes())
    }

    fn write(&mut self, event: Event) -> std::io::Result<()> {
        if self.limited {
            return Ok(());
        }
        let line = match event {
            Event::Output(t, bytes) => {
                let t = self.time(t);
                match self.encoder.output(t, &bytes) {
                    Some(line) => line,
                    None => return Ok(()),
                }
            }
            Event::Resize(t, cols, rows) => CastEncoder::resize(self.time(t), cols, rows),
            Event::Marker(t, label) => CastEncoder::marker(self.time(t), &label),
            Event::Close => unreachable!("close is handled by the writer loop"),
        };
        if self.written + line.len() as u64 > self.limit {
            self.limited = true;
            let stop = CastEncoder::marker(self.last_t, LIMIT_MARKER);
            self.file.write_all(stop.as_bytes())?;
            return self.file.flush();
        }
        self.written += line.len() as u64;
        self.file.write_all(line.as_bytes())
    }

    fn time(&mut self, t: f64) -> f64 {
        self.last_t = self.last_t.max(t);
        self.last_t
    }
}

/// Drops every OSC 633 sequence (`ESC ] 633 ; ... BEL` or `... ESC \`) from the bytes going to a recording. The
/// shell integration's 633;E carries the typed command line, unscrubbed, and the player has no use for it.
/// A sequence cut by the end of a chunk is followed across chunks.
#[derive(Default)]
pub struct Osc633Strip {
    state: StripState,
    /// The bytes of a possible `ESC ] 633 ;` read so far.
    held: Vec<u8>,
}

#[derive(Default, Clone, Copy)]
enum StripState {
    #[default]
    Text,
    /// `held` is a proper prefix of `ESC ] 633 ;`.
    Prefix,
    /// Inside a sequence; `esc` is set when the last byte was ESC.
    Body { len: usize, esc: bool },
}

const OSC633_START: &[u8] = b"]633;";

impl Osc633Strip {
    /// `bytes` without its OSC 633 sequences. A prefix that is not one comes out whole, with the next chunk.
    pub fn filter(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(bytes.len());
        for &b in bytes {
            loop {
                match self.state {
                    StripState::Text => {
                        if b == 0x1b {
                            self.held.push(b);
                            self.state = StripState::Prefix;
                        } else {
                            out.push(b);
                        }
                    }
                    StripState::Prefix => {
                        if b == OSC633_START[self.held.len()] {
                            self.held.push(b);
                            if self.held.len() == OSC633_START.len() {
                                self.held.clear();
                                self.state = StripState::Body { len: 0, esc: false };
                            }
                        } else {
                            out.append(&mut self.held);
                            self.state = StripState::Text;
                            continue;
                        }
                    }
                    StripState::Body { len, esc } => {
                        // BEL, or ST (ESC followed by a backslash, 0x5c), ends it.
                        if b == 0x07 || (esc && b == 0x5c) {
                            self.state = StripState::Text;
                        } else if len >= MAX_OSC633 {
                            // Give up on a sequence that never ends, so the recording goes on.
                            self.state = StripState::Text;
                            out.push(b);
                        } else {
                            self.state = StripState::Body {
                                len: len + 1,
                                esc: b == 0x1b,
                            };
                        }
                    }
                }
                break;
            }
        }
        out
    }
}

#[cfg(test)]
mod strip_tests {
    use super::*;

    fn run(chunks: &[&[u8]]) -> Vec<u8> {
        let mut f = Osc633Strip::default();
        chunks.iter().flat_map(|c| f.filter(c)).collect()
    }

    #[test]
    fn osc633_never_reaches_the_recording() {
        let e: &[u8] = b"a\x1b]633;E;docker login -p hunter2\x1b\\b\x1b]633;E;x\x07c";
        assert_eq!(run(&[e]), b"abc");
        assert_eq!(run(&[b"\x1b]633;A\x07\x1b]633;D;0\x07"]), b"");
        // Other sequences and lone escapes stay.
        let keep: &[u8] = b"\x1b[31mred\x1b[0m \x1b]133;A\x07 \x1b]0;title\x07 \x1b] \x1b";
        assert_eq!(run(&[keep]), &keep[..keep.len() - 1]);
        // Cut at every byte of the sequence, in every place.
        for cut in 0..=e.len() {
            assert_eq!(run(&[&e[..cut], &e[cut..]]), b"abc", "cut at {cut}");
        }
        // Three chunks, the terminator split from its ESC.
        assert_eq!(run(&[b"x\x1b]633;E;sec", b"ret\x1b", b"\\y"]), b"xy");
        // A prefix that turns out to be something else comes out whole.
        assert_eq!(run(&[b"\x1b]63", b"4;z"]), b"\x1b]634;z");
        assert_eq!(run(&[b"\x1b\x1b]633;E;s\x07"]), b"\x1b");
    }

    #[test]
    fn an_unterminated_osc633_is_dropped_up_to_its_bound() {
        let mut f = Osc633Strip::default();
        let mut got = f.filter(b"ok \x1b]633;E;");
        let junk = vec![b's'; MAX_OSC633 + 10];
        got.extend(f.filter(&junk));
        // Past the bound the filter gives up and lets text through again.
        got.extend(f.filter(b" back"));
        assert!(got.starts_with(b"ok "));
        assert!(got.ends_with(b" back"));
        assert!(got.len() < 3 + 20 + 5 + 10, "the sequence body was recorded");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(line: &str) -> String {
        let v: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
        v[2].as_str().unwrap().to_owned()
    }

    #[test]
    fn header_has_size_time_and_term() {
        let line = header_line(&CastHeader {
            cols: 80,
            rows: 24,
            started_unix: 1_700_000_000,
            shell: None,
        });
        assert!(line.ends_with('\n'));
        let v: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(v["version"], 2);
        assert_eq!(v["width"], 80);
        assert_eq!(v["height"], 24);
        assert_eq!(v["timestamp"], 1_700_000_000u64);
        assert_eq!(v["env"]["TERM"], "xterm-256color");
        assert!(v["env"].get("SHELL").is_none());
        let with = header_line(&CastHeader {
            cols: 1,
            rows: 1,
            started_unix: 0,
            shell: Some("pwsh".into()),
        });
        assert!(with.contains("\"SHELL\":\"pwsh\""));
    }

    #[test]
    fn output_escapes_control_bytes() {
        let line = CastEncoder::default().output(1.5, b"a\x1b[0m\r\n");
        assert_eq!(line.as_deref(), Some("[1.500000,\"o\",\"a\\u001b[0m\\r\\n\"]\n"));
    }

    #[test]
    fn split_utf8_is_carried() {
        let mut enc = CastEncoder::default();
        assert_eq!(data(&enc.output(0.1, &[b'x', 0xC3]).unwrap()), "x");
        let second = data(&enc.output(0.2, &[0xA9]).unwrap());
        assert_eq!(second, "é");
        assert!(!second.contains('\u{FFFD}'));
    }

    #[test]
    fn invalid_bytes_become_replacement() {
        let line = CastEncoder::default().output(0.0, &[0xFF, b'a']).unwrap();
        assert_eq!(data(&line), "\u{FFFD}a");
    }

    #[test]
    fn lone_partial_returns_none() {
        let mut enc = CastEncoder::default();
        assert_eq!(enc.output(0.0, &[0xE2, 0x82]), None);
        assert_eq!(data(&enc.output(0.1, &[0xAC]).unwrap()), "€");
    }

    #[test]
    fn resize_and_marker_lines() {
        assert_eq!(CastEncoder::resize(2.0, 120, 30), "[2.000000,\"r\",\"120x30\"]\n");
        assert_eq!(CastEncoder::marker(3.0, "prompt"), "[3.000000,\"m\",\"prompt\"]\n");
    }

    fn header() -> CastHeader {
        CastHeader {
            cols: 80,
            rows: 24,
            started_unix: 0,
            shell: None,
        }
    }

    fn event_of(line: &str) -> (f64, String) {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        (v[0].as_f64().unwrap(), v[1].as_str().unwrap().to_owned())
    }

    #[test]
    fn recorder_writes_header_and_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rec").join("t-1.cast");
        let rec = Recorder::start(path.clone(), header()).unwrap();
        rec.output(b"hi");
        rec.resize(100, 40);
        rec.marker("prompt");
        rec.finish();
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4);
        let h: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(h["version"], 2);
        let events: Vec<(f64, String)> = lines[1..].iter().map(|l| event_of(l)).collect();
        let codes: Vec<&str> = events.iter().map(|(_, c)| c.as_str()).collect();
        assert_eq!(codes, ["o", "r", "m"]);
        assert!(events.windows(2).all(|w| w[0].0 <= w[1].0));
    }

    #[test]
    fn recorder_stops_at_the_size_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t-2.cast");
        let rec = Recorder::start_with_limit(path.clone(), header(), 200).unwrap();
        for _ in 0..50 {
            rec.output(&[b'y'; 20]);
        }
        rec.finish();
        let text = std::fs::read_to_string(&path).unwrap();
        let last = text.lines().last().unwrap();
        let v: serde_json::Value = serde_json::from_str(last).unwrap();
        assert_eq!(v[1], "m");
        assert_eq!(v[2], "Recording stopped: it reached the size limit.");
        assert!(text.len() <= 200 + last.len() + 1, "{} bytes", text.len());
    }

    #[test]
    fn a_stalled_disk_never_blocks_the_session() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t-3.cast");
        // The writer waits on `gate` before reading anything: a disk that hangs without failing.
        let (open, gate) = sync_channel::<()>(0);
        let rec = Recorder::start_inner(path.clone(), header(), MAX_RECORDING_BYTES, 4, Some(gate)).unwrap();
        let (done_tx, done) = sync_channel::<Recorder>(1);
        std::thread::spawn(move || {
            for _ in 0..100 {
                rec.output(b"y\r\n");
            }
            done_tx.send(rec).unwrap();
        });
        let rec = done
            .recv_timeout(Duration::from_secs(5))
            .expect("output blocked behind a stalled recording");
        assert!(rec.is_recording(), "the file is still valid, so its path stays offered");
        open.send(()).unwrap();
        rec.finish();
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let v: serde_json::Value = serde_json::from_str(lines.last().unwrap()).unwrap();
        assert_eq!(v[1], "m");
        assert_eq!(v[2], "Recording stopped: it fell behind.");
        // Only the events queued before the drop are written: no gap inside the replay.
        assert_eq!(lines.len(), 1 + 4 + 1, "{text}");
    }

    #[test]
    fn close_flushes_and_closes_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t-close.cast");
        let rec = Recorder::start(path.clone(), header()).unwrap();
        rec.output(b"last");
        rec.close();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !rec.is_closed() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(rec.is_closed(), "the writer did not close its file");
        let text = std::fs::read_to_string(&path).unwrap();
        let last: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
        assert_eq!(last[1], "o");
        assert_eq!(last[2], "last");
        rec.output(b"after");
        rec.resize(100, 40);
        rec.marker("after");
        rec.close();
        assert!(rec.is_recording(), "a finished recording keeps its path");
        rec.finish();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        assert!(!text.contains("after"));
    }

    #[test]
    fn close_with_a_full_queue_still_closes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t-close-full.cast");
        let (open, gate) = sync_channel::<()>(0);
        let rec = Recorder::start_inner(path.clone(), header(), MAX_RECORDING_BYTES, 2, Some(gate)).unwrap();
        let (done_tx, done) = sync_channel::<Recorder>(1);
        std::thread::spawn(move || {
            rec.output(b"first");
            rec.output(b"last");
            rec.output(b"dropped");
            rec.close();
            done_tx.send(rec).unwrap();
        });
        let rec = done
            .recv_timeout(Duration::from_secs(2))
            .expect("close blocked behind a stalled recording");
        assert!(!rec.is_closed(), "the writer still holds its file");
        open.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !rec.is_closed() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(rec.is_closed(), "the full queue prevented close");
        assert!(rec.is_recording());
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 1 + 2 + 1, "{text}");
        let last: serde_json::Value = serde_json::from_str(lines.last().unwrap()).unwrap();
        assert_eq!(last[1], "m");
        assert_eq!(last[2], "Recording stopped: it fell behind.");
        assert!(!text.contains("dropped"));
        rec.finish();
    }

    #[test]
    fn close_with_a_full_queue_preserves_every_queued_event() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t-close-drain.cast");
        let (open, gate) = sync_channel::<()>(0);
        let rec = Recorder::start_inner(path.clone(), header(), MAX_RECORDING_BYTES, 2, Some(gate)).unwrap();
        let (done_tx, done) = sync_channel::<Recorder>(1);
        std::thread::spawn(move || {
            rec.output(b"first");
            rec.output(b"last");
            rec.close();
            rec.output(b"after");
            rec.resize(100, 40);
            rec.marker("after");
            done_tx.send(rec).unwrap();
        });
        let rec = done.recv_timeout(Duration::from_secs(2)).expect("close blocked");
        assert!(!rec.is_closed());
        open.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !rec.is_closed() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(rec.is_closed());
        assert!(rec.is_recording());
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 3, "{text}");
        let last: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
        assert_eq!(last[1], "o");
        assert_eq!(last[2], "last");
        assert!(!text.contains("after"));
        rec.finish();
    }

    #[test]
    fn a_send_already_in_flight_when_close_finishes_keeps_the_recording_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t-close-race.cast");
        let rec = Recorder::start(path.clone(), header()).unwrap();
        rec.output(b"last");
        rec.close();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !rec.is_closed() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(rec.is_closed());
        let text = std::fs::read_to_string(&path).unwrap();
        // Resume a sender after its initial flag check: close has since dropped the receiver.
        rec.enqueue(Event::Marker(rec.now(), "after".into()));
        assert!(rec.is_recording(), "a successful close is not an I/O failure");
        rec.finish();
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    }

    #[test]
    fn start_fails_cleanly_on_a_bad_path() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Recorder::start(dir.path().to_path_buf(), header()).is_err());
    }
}
