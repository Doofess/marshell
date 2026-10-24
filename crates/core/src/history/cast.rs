//! Reads asciicast v2 recordings written by `pty::record`: a summary of one file, and its events from a point.
//! Files are only read, in a stream, and may still be growing: an incomplete last line is ignored.

use anyhow::Context;
use marshell_protocol::history::CastEvent;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// True for a recording file name that is safe to join to the recordings folder: `<tab id>-<unix ms>.cast`.
/// Matches `^[A-Za-z0-9]{1,32}-[0-9]{1,16}\.cast$`.
pub fn valid_recording_name(name: &str) -> bool {
    let Some((tab, ms)) = name.strip_suffix(".cast").and_then(|stem| stem.split_once('-')) else {
        return false;
    };
    (1..=32).contains(&tab.len())
        && tab.bytes().all(|b| b.is_ascii_alphanumeric())
        && (1..=16).contains(&ms.len())
        && ms.bytes().all(|b| b.is_ascii_digit())
}

/// The start time (unix ms) a recording's file name carries.
pub fn started_ms(name: &str) -> Option<u64> {
    if !valid_recording_name(name) {
        return None;
    }
    name.strip_suffix(".cast")?.rsplit_once('-')?.1.parse().ok()
}

/// The 'o', 'r' or 'm' event on a complete line, with its time on the Unix clock. None for a line that isn't one.
fn parse_event(line: &[u8], started_ms: u64) -> Option<CastEvent> {
    let (t, code, data): (f64, String, String) = serde_json::from_slice(line).ok()?;
    if !t.is_finite() {
        return None;
    }
    Some(CastEvent {
        at_ms: started_ms + (t.max(0.0) * 1000.0).round() as u64,
        code,
        data,
    })
}

/// Reads complete lines (ending in a newline) with the byte offset each starts at. A last line with no
/// newline is not returned: the recorder may still be writing it.
struct Lines<R> {
    reader: R,
    offset: u64,
    buf: Vec<u8>,
}

impl<R: BufRead> Lines<R> {
    fn new(reader: R, offset: u64) -> Self {
        Lines {
            reader,
            offset,
            buf: Vec::new(),
        }
    }

    /// The next line without its newline, and where it starts. The slice lives until the next call.
    fn next_line(&mut self) -> std::io::Result<Option<(u64, &[u8])>> {
        self.buf.clear();
        let read = self.reader.read_until(b'\n', &mut self.buf)?;
        if self.buf.last() != Some(&b'\n') {
            return Ok(None);
        }
        let start = self.offset;
        self.offset += read as u64;
        self.buf.pop();
        Ok(Some((start, &self.buf)))
    }
}

/// Opens the file and reads its header line: the size and where the events start.
fn open(path: &Path) -> anyhow::Result<(Lines<BufReader<File>>, u16, u16)> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut lines = Lines::new(BufReader::with_capacity(64 * 1024, file), 0);
    let (_, line) = lines
        .next_line()
        .with_context(|| format!("reading {}", path.display()))?
        .with_context(|| format!("{} has no header line", path.display()))?;
    let header: serde_json::Value =
        serde_json::from_slice(line).with_context(|| format!("{} has a broken header line", path.display()))?;
    let dim = |key: &str| {
        header
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u16::try_from(n).ok())
            .filter(|n| *n > 0)
            .with_context(|| format!("{} has no usable {key} in its header", path.display()))
    };
    let (cols, rows) = (dim("width")?, dim("height")?);
    Ok((lines, cols, rows))
}

/// What one pass over a recording finds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastSummary {
    pub cols: u16,
    pub rows: u16,
    pub started_ms: u64,
    /// The last complete event's time, or `started_ms` when there is none.
    pub ended_ms: u64,
    /// Times of the `m` events labelled `prompt`.
    pub marks_ms: Vec<u64>,
    /// The text of the "Recording stopped:" marker, when the recording ends with one.
    pub stopped: Option<String>,
}

/// One streaming pass over the file; it never holds more than a line.
pub fn scan(path: &Path, started_ms: u64) -> anyhow::Result<CastSummary> {
    let (mut lines, cols, rows) = open(path)?;
    let mut summary = CastSummary {
        cols,
        rows,
        started_ms,
        ended_ms: started_ms,
        marks_ms: Vec::new(),
        stopped: None,
    };
    while let Some((_, line)) = lines
        .next_line()
        .with_context(|| format!("reading {}", path.display()))?
    {
        let Some(event) = parse_event(line, started_ms) else {
            continue;
        };
        summary.ended_ms = summary.ended_ms.max(event.at_ms);
        if event.code == "m" {
            if event.data == "prompt" {
                summary.marks_ms.push(event.at_ms);
            } else if event.data.starts_with(STOPPED) {
                summary.stopped = Some(event.data);
            }
        }
    }
    Ok(summary)
}

/// How the recorder's own markers that end a recording begin.
const STOPPED: &str = "Recording stopped:";

/// Where `frames` starts reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameStart {
    /// The first event at or after this time (unix ms), with the screen as it was just before.
    At(u64),
    /// A byte offset an earlier answer gave; the screen is not sent again.
    Offset(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frames {
    pub cols: u16,
    pub rows: u16,
    /// The screen at the start, as escape sequences. Only with `FrameStart::At`.
    pub screen: Option<String>,
    pub events: Vec<CastEvent>,
    /// Where to continue, or None at `to_ms` or the end of the file.
    pub next_offset: Option<u64>,
}

#[derive(Debug)]
pub enum FramesError {
    NotABoundary,
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for FramesError {
    fn from(err: anyhow::Error) -> Self {
        Self::Failed(err)
    }
}

impl std::fmt::Display for FramesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotABoundary => f.write_str("Not a frame boundary."),
            Self::Failed(err) => write!(f, "{err:#}"),
        }
    }
}

/// Applies an `r` event's `COLSxROWS` to the size, ignoring one that doesn't parse.
fn resized(data: &str, size: &mut (u16, u16)) {
    let parsed = data.split_once('x').and_then(|(cols, rows)| {
        let (cols, rows) = (cols.parse::<u16>().ok()?, rows.parse::<u16>().ok()?);
        (cols > 0 && rows > 0).then_some((cols, rows))
    });
    if let Some(new) = parsed {
        *size = new;
    }
}

/// The events from `from` up to `to_ms` (exclusive), at most `budget` bytes of their data (one event always
/// goes out, however large). `At` replays everything before it through a terminal emulator and returns that
/// screen; `Offset` continues where an earlier answer stopped and returns no screen. With `Offset` the size
/// is the header's: the caller already has the live one, and `r` events arrive in the events.
pub fn frames(
    path: &Path,
    started_ms: u64,
    from: FrameStart,
    to_ms: u64,
    budget: usize,
) -> Result<Frames, FramesError> {
    let (mut lines, mut cols, mut rows) = open(path)?;
    let read_err = |err: std::io::Error| FramesError::Failed(anyhow::Error::new(err).context("reading the recording"));
    let mut screen = None;
    // The first event not yet used, when a replay already read it.
    let mut pending: Option<(u64, CastEvent)> = None;

    match from {
        FrameStart::At(at) => {
            let mut parser = vt100::Parser::new(rows, cols, 0);
            let mut size = (cols, rows);
            while let Some((offset, line)) = lines.next_line().map_err(read_err)? {
                let Some(event) = parse_event(line, started_ms) else {
                    continue;
                };
                if event.at_ms >= at {
                    pending = Some((offset, event));
                    break;
                }
                match event.code.as_str() {
                    "o" => parser.process(event.data.as_bytes()),
                    "r" => {
                        resized(&event.data, &mut size);
                        parser.screen_mut().set_size(size.1, size.0);
                    }
                    _ => {}
                }
            }
            (cols, rows) = size;
            screen = Some(String::from_utf8_lossy(&parser.screen().state_formatted()).into_owned());
        }
        FrameStart::Offset(offset) => {
            let header_end = lines.offset;
            if offset > header_end {
                let mut file = lines.reader.into_inner();
                file.seek(SeekFrom::Start(offset - 1)).map_err(read_err)?;
                let mut before = [0u8; 1];
                match file.read_exact(&mut before) {
                    Ok(()) if before[0] != b'\n' => return Err(FramesError::NotABoundary),
                    Ok(()) => {}
                    // Past the end of the file: nothing there yet.
                    Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {}
                    Err(err) => return Err(read_err(err)),
                }
                lines = Lines::new(BufReader::with_capacity(64 * 1024, file), offset);
            } else if offset != 0 && offset != header_end {
                return Err(FramesError::NotABoundary);
            }
        }
    }

    let mut events = Vec::new();
    let mut used = 0usize;
    let mut next_offset = None;
    loop {
        let (offset, event) = match pending.take() {
            Some(found) => found,
            None => match lines.next_line().map_err(read_err)? {
                Some((offset, line)) => match parse_event(line, started_ms) {
                    Some(event) => (offset, event),
                    None => continue,
                },
                None => break,
            },
        };
        if event.at_ms >= to_ms {
            break;
        }
        if !events.is_empty() && used + event.data.len() > budget {
            next_offset = Some(offset);
            break;
        }
        used += event.data.len();
        events.push(event);
    }
    Ok(Frames {
        cols,
        rows,
        screen,
        events,
        next_offset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::record::{CastEncoder, CastHeader, header_line};

    const START: u64 = 1_761_300_000_000;

    fn header(cols: u16, rows: u16) -> String {
        header_line(&CastHeader {
            cols,
            rows,
            started_unix: START / 1000,
            shell: None,
        })
    }

    fn out(t: f64, text: &str) -> String {
        CastEncoder::default().output(t, text.as_bytes()).unwrap()
    }

    fn write(dir: &Path, lines: &[String]) -> std::path::PathBuf {
        let path = dir.join("t0a1b2c3d4e5f-1761300000000.cast");
        std::fs::write(&path, lines.concat()).unwrap();
        path
    }

    /// Every event of the file with its offset, by the reader's own rules, for comparing chained calls.
    fn all_events(path: &Path) -> Vec<CastEvent> {
        let mut found = Vec::new();
        let mut at = FrameStart::At(0);
        loop {
            let got = frames(path, START, at, u64::MAX, 1 << 20).unwrap();
            found.extend(got.events);
            match got.next_offset {
                Some(o) => at = FrameStart::Offset(o),
                None => return found,
            }
        }
    }

    #[test]
    fn recording_name_check_table() {
        assert!(valid_recording_name("t0a1b2c3d4e5f-1761300000000.cast"));
        for bad in [
            r"..\x.cast",
            r"C:\x.cast",
            "a/b-1.cast",
            "t1-12.cast.exe",
            "t1.cast",
            "t1-.cast",
            "-12.cast",
            "t1-1-2.cast",
            "t1-12.CAST",
            "",
        ] {
            assert!(!valid_recording_name(bad), "{bad:?}");
        }
        assert!(!valid_recording_name(&format!("{}-1.cast", "a".repeat(200))));
        assert!(!valid_recording_name(&format!("{}.cast", "1".repeat(200))));
        assert!(!valid_recording_name(&format!("t-{}.cast", "1".repeat(17))));
        assert_eq!(started_ms("t0a1b2c3d4e5f-1761300000000.cast"), Some(START));
        assert_eq!(started_ms("a/b-1.cast"), None);
    }

    #[test]
    fn scan_reads_size_end_marks_and_stop() {
        let dir = tempfile::tempdir().unwrap();
        let stop = "Recording stopped: it reached the size limit.";
        let path = write(
            dir.path(),
            &[
                header(80, 24),
                out(0.5, "hello"),
                CastEncoder::marker(1.0, "prompt"),
                out(1.5, "world"),
                CastEncoder::marker(2.0, "prompt"),
                CastEncoder::marker(2.0, stop),
            ],
        );
        let got = scan(&path, START).unwrap();
        assert_eq!((got.cols, got.rows), (80, 24));
        assert_eq!(got.started_ms, START);
        assert_eq!(got.ended_ms, START + 2000);
        assert_eq!(got.marks_ms, vec![START + 1000, START + 2000]);
        assert_eq!(got.stopped.as_deref(), Some(stop));
    }

    #[test]
    fn scan_ignores_an_incomplete_last_line() {
        let dir = tempfile::tempdir().unwrap();
        for tail in [r#"[9.0,"o","par"#, r#"[9.0,"o","whole"]"#] {
            let path = write(dir.path(), &[header(80, 24), out(0.5, "hello"), tail.to_string()]);
            let got = scan(&path, START).unwrap();
            assert_eq!(got.ended_ms, START + 500, "{tail}");
            assert_eq!(got.stopped, None);
        }
    }

    #[test]
    fn scan_of_a_header_only_file_has_zero_length() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), &[header(120, 40)]);
        let got = scan(&path, START).unwrap();
        assert_eq!((got.cols, got.rows), (120, 40));
        assert_eq!(got.ended_ms, START);
        assert!(got.marks_ms.is_empty());
        assert!(scan(&dir.path().join("gone.cast"), START).is_err());
    }

    /// A fixture with cursor moves, a full-screen redraw and a clear, ten events 100 ms apart.
    fn redraw_fixture() -> Vec<String> {
        let mut lines = vec![header(40, 10)];
        let texts = [
            "one\r\ntwo\r\nthree\r\n",
            "\x1b[2J\x1b[Hscreen A\r\nrow 2",
            "\x1b[5;3Hmoved\x1b[1;1HX",
            "\x1b[1mbold\x1b[0m plain",
            "\x1b[2J\x1b[Hscreen B",
            "\x1b[3;1Hmiddle\x1b[K",
            "\r\nnext",
            "\x1b[s\x1b[10;1Hbottom\x1b[u",
            "\x1b[H\x1b[2Kfirst",
            "tail",
        ];
        for (i, text) in texts.iter().enumerate() {
            lines.push(out(i as f64 / 10.0, text));
        }
        lines
    }

    #[test]
    fn frames_at_matches_a_full_replay() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), &redraw_fixture());
        // 40% of the way: the events at 0.0 to 0.3 s come before.
        let at = START + 400;
        let got = frames(&path, START, FrameStart::At(at), u64::MAX, 1 << 20).unwrap();
        let mut full = vt100::Parser::new(10, 40, 0);
        for event in all_events(&path).iter().filter(|e| e.at_ms < at) {
            full.process(event.data.as_bytes());
        }
        let mut seeded = vt100::Parser::new(got.rows, got.cols, 0);
        seeded.process(got.screen.as_deref().unwrap().as_bytes());
        assert_eq!(seeded.screen().contents(), full.screen().contents());
        assert_eq!(seeded.screen().cursor_position(), full.screen().cursor_position());
        assert!(got.events.iter().all(|e| e.at_ms >= at));
        assert_eq!(got.events.len(), 6);
        assert_eq!(got.next_offset, None);
    }

    #[test]
    fn frames_at_uses_the_size_after_a_resize() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            &[
                header(80, 24),
                out(0.5, "hello"),
                CastEncoder::resize(1.0, 100, 30),
                out(2.0, "later"),
            ],
        );
        let before = frames(&path, START, FrameStart::At(START + 900), u64::MAX, 1 << 20).unwrap();
        assert_eq!((before.cols, before.rows), (80, 24));
        let after = frames(&path, START, FrameStart::At(START + 1500), u64::MAX, 1 << 20).unwrap();
        assert_eq!((after.cols, after.rows), (100, 30));
        assert_eq!(after.events.len(), 1);
    }

    #[test]
    fn frames_continue_from_next_offset_without_gaps() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), &redraw_fixture());
        let whole = frames(&path, START, FrameStart::At(0), u64::MAX, 1 << 20).unwrap();
        assert_eq!(whole.events.len(), 10);
        assert_eq!(whole.next_offset, None);

        let mut seen = Vec::new();
        let mut got = frames(&path, START, FrameStart::At(0), u64::MAX, 64).unwrap();
        assert!(got.screen.is_some());
        let mut calls = 1;
        loop {
            assert!(!got.events.is_empty());
            seen.extend(got.events.clone());
            let Some(next) = got.next_offset else { break };
            got = frames(&path, START, FrameStart::Offset(next), u64::MAX, 64).unwrap();
            assert_eq!(got.screen, None);
            calls += 1;
        }
        assert!(calls > 1);
        assert_eq!(seen, whole.events);
    }

    #[test]
    fn frames_stop_at_to_ms_and_handle_times_outside_the_part() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), &redraw_fixture());
        let cut = frames(&path, START, FrameStart::At(START + 200), START + 500, 1 << 20).unwrap();
        assert_eq!(cut.events.len(), 3);
        assert_eq!(cut.next_offset, None);
        // Before the part's start: the part's start.
        let early = frames(&path, START, FrameStart::At(0), u64::MAX, 1 << 20).unwrap();
        assert_eq!(early.events.len(), 10);
        // After its end: an empty frame, nothing to continue.
        let late = frames(&path, START, FrameStart::At(START + 60_000), u64::MAX, 1 << 20).unwrap();
        assert!(late.events.is_empty());
        assert_eq!(late.next_offset, None);
    }

    #[test]
    fn frames_refuse_an_offset_inside_a_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), &redraw_fixture());
        let head = header(40, 10).len() as u64;
        assert!(matches!(
            frames(&path, START, FrameStart::Offset(head + 3), u64::MAX, 1 << 20),
            Err(FramesError::NotABoundary)
        ));
        assert!(matches!(
            frames(&path, START, FrameStart::Offset(3), u64::MAX, 1 << 20),
            Err(FramesError::NotABoundary)
        ));
        let ok = frames(&path, START, FrameStart::Offset(head), u64::MAX, 1 << 20).unwrap();
        assert_eq!(ok.events.len(), 10);
        let zero = frames(&path, START, FrameStart::Offset(0), u64::MAX, 1 << 20).unwrap();
        assert_eq!(zero.events.len(), 10);
    }

    #[test]
    fn frames_ignore_an_incomplete_last_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            &[header(80, 24), out(0.5, "hello"), r#"[0.9,"o","hal"#.to_string()],
        );
        let got = frames(&path, START, FrameStart::At(0), u64::MAX, 1 << 20).unwrap();
        assert_eq!(got.events.len(), 1);
        assert_eq!(got.next_offset, None);
    }
}
