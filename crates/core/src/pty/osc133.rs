//! Finds OSC 133 prompt and command marks (`ESC ] 133 ; X [; params] BEL|ST`) in shell output,
//! the typed command line a shell reports as `ESC ] 633 ; E ; <escaped text> BEL|ST`, and the folder it
//! reports as `ESC ] 9 ; 9 ; <path>` (PowerShell and cmd) or `ESC ] 7 ; file://<host>/<path>` (bash).
//! Also finds OSC 9 text, OSC 777 and kitty OSC 99 notifications, OSC 9;4 progress states, and bells
//! outside escape strings.
//! The scanner only reads: the bytes still go to the terminal unchanged, so xterm sees the marks too.

/// A shell integration mark, terminal notification, or bare bell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    PromptStart,
    CommandStart,
    Executed,
    Finished(Option<i32>),
    /// The command line the user entered, unescaped. Not yet trimmed or capped.
    CommandLine(String),
    /// The shell's current folder, with any quotes and `file://` host dropped.
    Cwd(String),
    /// Trimmed notification text, capped at 200 characters.
    Notify(Option<String>),
    /// An OSC 9;4 progress state, 0 to 4: 0 clear, 1 normal, 2 error, 3 indeterminate, 4 paused.
    Progress(u8),
    Bell,
}

const START: &[u8] = b"133;";
const START_LINE: &[u8] = b"633;E;";
const START_CWD9: &[u8] = b"9;9;";
const START_CWD7: &[u8] = b"7;";
const START_NOTIFY: &[u8] = b"777;notify;";
const START_PROGRESS: &[u8] = b"9;4;";
const START_KITTY: &[u8] = b"99;";
/// A mark is a few bytes; anything unterminated past this is not one of ours.
const MAX_CARRY: usize = 128;
/// A command line or folder can be long, but a body past this is dropped.
const MAX_LINE_BODY: usize = 4096;
const MAX_NOTIFY_CHARS: usize = 200;

#[derive(Clone, Copy, PartialEq, Eq)]
enum StringKind {
    Osc,
    Other,
}

/// String framing survives across reads even when the bounded payload has been dropped.
#[derive(Default)]
pub struct Osc133Scanner {
    string: Option<StringKind>,
    escaped: bool,
    body: OscBody,
    /// The kitty notification whose last chunk has not come yet. At most one: a new id replaces it.
    kitty: Option<KittyPending>,
}

impl Osc133Scanner {
    pub fn scan(&mut self, bytes: &[u8]) -> Vec<Mark> {
        let mut marks = Vec::new();
        for &byte in bytes {
            if matches!(byte, 0x18 | 0x1a) {
                self.string = None;
                self.escaped = false;
                self.body = OscBody::default();
                continue;
            }
            if self.escaped {
                self.escaped = false;
                if self.string.is_some() && byte == b'\\' {
                    if self.string == Some(StringKind::Osc) {
                        self.finish_body(&mut marks);
                    }
                    self.string = None;
                    continue;
                }
                if self.string == Some(StringKind::Other) {
                    // DCS/APC/PM/SOS only end at ST; other escapes are data.
                    self.escaped = byte == 0x1b;
                    continue;
                }
                // A non-ST escape aborts an OSC and may start another string.
                self.string = None;
                self.body = OscBody::default();
                match byte {
                    b']' => self.string = Some(StringKind::Osc),
                    b'P' | b'_' | b'^' | b'X' => self.string = Some(StringKind::Other),
                    0x1b => self.escaped = true,
                    0x07 => marks.push(Mark::Bell),
                    _ => {}
                }
                continue;
            }
            match byte {
                0x1b => self.escaped = true,
                0x07 if self.string == Some(StringKind::Osc) => {
                    self.finish_body(&mut marks);
                    self.string = None;
                }
                0x07 if self.string.is_none() => marks.push(Mark::Bell),
                _ if self.string == Some(StringKind::Osc) => self.body.push(byte),
                _ => {}
            }
        }
        marks
    }

    fn finish_body(&mut self, marks: &mut Vec<Mark>) {
        match std::mem::take(&mut self.body) {
            OscBody::Legacy { start, carry } if start == START_KITTY => {
                marks.extend(kitty_chunk(&mut self.kitty, &carry));
            }
            body => marks.extend(body.finish()),
        }
    }
}

enum OscBody {
    Prefix(Vec<u8>),
    Legacy { start: &'static [u8], carry: Vec<u8> },
    Notify(NotificationText),
    Title(NotificationText),
    Ignore,
}

impl Default for OscBody {
    fn default() -> Self {
        Self::Prefix(Vec::new())
    }
}

impl OscBody {
    fn push(&mut self, byte: u8) {
        match self {
            Self::Prefix(prefix) => {
                if prefix == b"9;" && !byte.is_ascii_digit() {
                    let mut text = NotificationText::default();
                    text.push(byte);
                    *self = Self::Notify(text);
                    return;
                }
                prefix.push(byte);
                let starts = [
                    START,
                    START_LINE,
                    START_CWD9,
                    START_CWD7,
                    START_NOTIFY,
                    START_PROGRESS,
                    START_KITTY,
                ];
                if let Some(start) = starts.into_iter().find(|start| *start == prefix) {
                    *self = if start == START_NOTIFY {
                        Self::Title(NotificationText::default())
                    } else {
                        Self::Legacy {
                            start,
                            carry: Vec::new(),
                        }
                    };
                } else if !starts.iter().any(|start| start.starts_with(prefix)) {
                    *self = Self::Ignore;
                }
            }
            Self::Legacy { start, carry } => {
                let limit = if *start == START || *start == START_PROGRESS {
                    MAX_CARRY
                } else {
                    MAX_LINE_BODY
                };
                if carry.len() < limit {
                    carry.push(byte);
                } else {
                    *self = Self::Ignore;
                }
            }
            Self::Title(text) if byte == b';' => {
                // A blank title leaves just the body.
                if text.chars > 0 {
                    text.push(b':');
                    text.push(b' ');
                }
                *self = Self::Notify(std::mem::take(text));
            }
            Self::Title(text) => text.push(byte),
            Self::Notify(text) => text.push(byte),
            Self::Ignore => {}
        }
    }

    fn finish(self) -> Option<Mark> {
        let text = match self {
            Self::Legacy { start, carry } => {
                return if start == START {
                    parse(&carry)
                } else if start == START_LINE {
                    Some(Mark::CommandLine(unescape(&carry)))
                } else if start == START_CWD9 {
                    cwd_from_osc9(&carry)
                } else if start == START_PROGRESS {
                    progress(&carry)
                } else if start == START_KITTY {
                    // Kitty chunks need the scanner's pending notification: `finish_body` takes them.
                    None
                } else {
                    cwd_from_osc7(&carry)
                };
            }
            Self::Prefix(prefix) if prefix == b"9;" => String::new(),
            Self::Notify(text) => text.finish(),
            _ => return None,
        };
        Some(Mark::Notify((!text.is_empty()).then_some(text)))
    }
}

/// Keeps only the first 200 trimmed characters and an incomplete UTF-8 character.
#[derive(Default)]
struct NotificationText {
    text: String,
    chars: usize,
    trimmed_end: usize,
    pending: Vec<u8>,
}

impl NotificationText {
    fn push(&mut self, byte: u8) {
        if self.pending.is_empty() && byte.is_ascii() {
            self.push_char(char::from(byte));
            return;
        }
        self.pending.push(byte);
        while !self.pending.is_empty() {
            let (character, consumed) = match std::str::from_utf8(&self.pending) {
                Ok(text) => {
                    let character = text.chars().next().unwrap();
                    (character, character.len_utf8())
                }
                Err(error) => match error.error_len() {
                    Some(len) => ('\u{fffd}', len),
                    None => break,
                },
            };
            self.pending.drain(..consumed);
            self.push_char(character);
        }
    }

    fn push_char(&mut self, character: char) {
        if self.chars == 0 && character.is_whitespace() {
            return;
        }
        if self.chars < MAX_NOTIFY_CHARS {
            self.text.push(character);
            self.chars += 1;
        }
        // A non-space beyond the cap makes any retained spaces internal, not trailing.
        if !character.is_whitespace() {
            self.trimmed_end = self.text.len();
        }
    }

    fn finish(mut self) -> String {
        if !self.pending.is_empty() {
            self.push_char('\u{fffd}');
        }
        self.text.truncate(self.trimmed_end);
        self.text
    }
}

fn parse(body: &[u8]) -> Option<Mark> {
    let (&kind, rest) = body.split_first()?;
    if !rest.is_empty() && rest[0] != b';' {
        return None;
    }
    match kind {
        b'A' => Some(Mark::PromptStart),
        b'B' => Some(Mark::CommandStart),
        b'C' => Some(Mark::Executed),
        b'D' => {
            let code = rest
                .get(1..)
                .and_then(|p| p.split(|&b| b == b';').next())
                .and_then(|p| std::str::from_utf8(p).ok())
                .and_then(|p| p.parse::<i32>().ok());
            Some(Mark::Finished(code))
        }
        _ => None,
    }
}

/// The state of `9;4;<state>[;<progress>]`: a single digit 0 to 4. Anything else is not a mark.
fn progress(body: &[u8]) -> Option<Mark> {
    let state = body.split(|&b| b == b';').next()?;
    match state {
        [digit @ b'0'..=b'4'] => Some(Mark::Progress(digit - b'0')),
        _ => None,
    }
}

/// A kitty notification still waiting for its last chunk.
struct KittyPending {
    id: Vec<u8>,
    title: NotificationText,
    body: NotificationText,
}

fn split_at_byte(bytes: &[u8], separator: u8) -> Option<(&[u8], &[u8])> {
    let at = bytes.iter().position(|&b| b == separator)?;
    Some((&bytes[..at], &bytes[at + 1..]))
}

/// Folds one `99;<metadata>;<payload>` body into the pending notification. The chunk that is not
/// `d=0` emits it. A chunk with another id replaces the pending one when it is not final, and is
/// emitted alone when it is. An invalid chunk drops its own id's pending text.
fn kitty_chunk(pending: &mut Option<KittyPending>, body: &[u8]) -> Option<Mark> {
    let (metadata, payload) = split_at_byte(body, b';')?;
    let (mut id, mut more, mut is_body, mut base64) = (&b""[..], false, false, false);
    for pair in metadata.split(|&b| b == b':') {
        match split_at_byte(pair, b'=') {
            Some((b"i", value)) => id = value,
            Some((b"d", value)) => more = value == b"0",
            Some((b"e", value)) => base64 = value == b"1",
            Some((b"p", b"title")) => is_body = false,
            Some((b"p", b"body")) => is_body = true,
            // Close, query, icon and every other payload kind is not text for us.
            Some((b"p", _)) => return None,
            _ => {}
        }
    }
    let decoded;
    let payload = if base64 {
        let Some(bytes) = decode_base64(payload) else {
            if pending.as_ref().is_some_and(|note| note.id == id) {
                *pending = None;
            }
            return None;
        };
        decoded = bytes;
        &decoded[..]
    } else {
        payload
    };
    let mut note = match pending.take() {
        Some(note) if note.id == id => note,
        other => {
            // Another id is pending. A final chunk is its own notification and leaves it be.
            if !more {
                *pending = other;
            }
            KittyPending {
                id: id.to_vec(),
                title: NotificationText::default(),
                body: NotificationText::default(),
            }
        }
    };
    let part = if is_body { &mut note.body } else { &mut note.title };
    payload.iter().for_each(|&byte| part.push(byte));
    if more {
        *pending = Some(note);
        return None;
    }
    let (title, body) = (note.title.finish(), note.body.finish());
    let mut text = match (title.is_empty(), body.is_empty()) {
        (false, false) => format!("{title}: {body}"),
        (true, _) => body,
        (_, true) => title,
    };
    if let Some((end, _)) = text.char_indices().nth(MAX_NOTIFY_CHARS) {
        text.truncate(end);
        text.truncate(text.trim_end().len());
    }
    Some(Mark::Notify((!text.is_empty()).then_some(text)))
}

/// Standard base64, with or without `=` padding. None for any other byte or an impossible length.
fn decode_base64(text: &[u8]) -> Option<Vec<u8>> {
    let text = text
        .strip_suffix(b"==")
        .or_else(|| text.strip_suffix(b"="))
        .unwrap_or(text);
    if text.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut bits, mut count) = (0u32, 0u32);
    for &byte in text {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        bits = bits << 6 | u32::from(value);
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    Some(out)
}

/// The body of `9;9;<path>`, without the quotes some shells put around it.
fn cwd_from_osc9(body: &[u8]) -> Option<Mark> {
    let body = body.strip_prefix(b"\"").unwrap_or(body);
    let body = body.strip_suffix(b"\"").unwrap_or(body);
    (!body.is_empty()).then(|| Mark::Cwd(String::from_utf8_lossy(body).into_owned()))
}

/// The body of `7;file://<host>/<path>`: the host is dropped and the path percent-decoded. A drive
/// path (`/C:/x`) loses its leading slash.
fn cwd_from_osc7(body: &[u8]) -> Option<Mark> {
    let after_host = body.strip_prefix(b"file://")?;
    let path = &after_host[after_host.iter().position(|&b| b == b'/')?..];
    let mut out = Vec::with_capacity(path.len());
    let mut i = 0;
    while i < path.len() {
        if path[i] == b'%'
            && let Some(hex) = path.get(i + 1..i + 3)
            && hex.iter().all(u8::is_ascii_hexdigit)
            && let Some(byte) = std::str::from_utf8(hex)
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(path[i]);
        i += 1;
    }
    if out.len() >= 3 && out[0] == b'/' && out[1].is_ascii_alphabetic() && out[2] == b':' {
        out.remove(0);
    }
    (!out.is_empty()).then(|| Mark::Cwd(String::from_utf8_lossy(&out).into_owned()))
}

/// Undoes the shell's escaping: `\\` is a backslash and `\xHH` is the byte HH. Any other
/// backslash stays as it is. Invalid UTF-8 is decoded lossily.
fn unescape(body: &[u8]) -> String {
    let mut out = Vec::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        if body[i] == b'\\' {
            if body.get(i + 1) == Some(&b'\\') {
                out.push(b'\\');
                i += 2;
                continue;
            }
            if body.get(i + 1) == Some(&b'x')
                && let Some(hex) = body.get(i + 2..i + 4)
                && hex.iter().all(u8::is_ascii_hexdigit)
                && let Some(byte) = std::str::from_utf8(hex)
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(byte);
                i += 4;
                continue;
            }
        }
        out.push(body[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(bytes: &[u8]) -> Vec<Mark> {
        Osc133Scanner::default().scan(bytes)
    }

    #[test]
    fn osc9_text_is_a_notification() {
        for bytes in [&b"\x1b]9;Build done\x07"[..], &b"\x1b]9;Build done\x1b\\"[..]] {
            assert_eq!(scan(bytes), [Mark::Notify(Some("Build done".into()))]);
        }
        assert_eq!(scan(b"\x1b]9;  done \t\x07"), [Mark::Notify(Some("done".into()))]);
        assert_eq!(scan(b"\x1b]9; \t\x07"), [Mark::Notify(None)]);
        assert_eq!(scan(b"\x1b]9;a\xffb\x07"), [Mark::Notify(Some("a\u{fffd}b".into()))]);
    }

    #[test]
    fn osc9_9_and_osc9_4_are_not_notifications() {
        assert_eq!(scan(b"\x1b]9;9;C:\\work\x07"), [Mark::Cwd("C:\\work".into())]);
        for digit in b'0'..=b'9' {
            let bytes = format!("\x1b]9;{};\x07", char::from(digit));
            assert!(scan(bytes.as_bytes()).is_empty());
        }
    }

    #[test]
    fn osc_9_4_progress_is_a_mark() {
        assert_eq!(scan(b"\x1b]9;4;1;50\x07"), [Mark::Progress(1)]);
        assert_eq!(scan(b"\x1b]9;4;0\x07"), [Mark::Progress(0)]);
        assert_eq!(scan(b"\x1b]9;4;3\x1b\\"), [Mark::Progress(3)]);
        for (tail, state) in [("2;10", 2), ("4;", 4), ("1;", 1)] {
            assert_eq!(scan(format!("\x1b]9;4;{tail}\x07").as_bytes()), [Mark::Progress(state)]);
        }
        for bad in ["9", "5", "10", "x", "", "-1", "1x;50"] {
            assert!(scan(format!("\x1b]9;4;{bad}\x07").as_bytes()).is_empty(), "{bad:?}");
        }
        // A bare 9;4 is not progress, and 9 free text is still a notification.
        assert!(scan(b"\x1b]9;4\x07").is_empty());
        assert_eq!(
            scan(b"\x1b]9;Build done\x07"),
            [Mark::Notify(Some("Build done".into()))]
        );
    }

    #[test]
    fn osc_9_4_progress_survives_split_reads() {
        assert_split(b"a\x1b]9;4;1;50\x07b", &[Mark::Progress(1)]);
        assert_split(b"a\x1b]9;4;0\x1b\\b", &[Mark::Progress(0)]);
        assert_split(b"\x1b]9;4;7\x07\x1b]9;4;4\x07", &[Mark::Progress(4)]);
        // An oversized body is dropped and the scanner recovers.
        let mut s = Osc133Scanner::default();
        let mut long = b"\x1b]9;4;1;".to_vec();
        long.extend(std::iter::repeat_n(b'5', 1000));
        assert!(s.scan(&long).is_empty());
        assert_eq!(s.scan(b"\x07\x1b]133;A\x07"), [Mark::PromptStart]);
    }

    #[test]
    fn osc_99_notifications() {
        // A single chunk is a title.
        for end in ["\x07", "\x1b\\"] {
            assert_eq!(
                scan(format!("\x1b]99;;Build done{end}").as_bytes()),
                [Mark::Notify(Some("Build done".into()))]
            );
        }
        assert_eq!(
            scan(b"\x1b]99;i=1:d=1:p=body;only body\x07"),
            [Mark::Notify(Some("only body".into()))]
        );
        // A d=0 chunk waits for the final one, which joins title and body.
        let mut s = Osc133Scanner::default();
        assert!(s.scan(b"\x1b]99;i=7:d=0:p=title;Tests\x1b\\").is_empty());
        assert!(s.scan(b"\x1b]9;4;9\x07").is_empty());
        assert_eq!(
            s.scan(b"\x1b]99;i=7:p=body;All green\x1b\\"),
            [Mark::Notify(Some("Tests: All green".into()))]
        );
        // The pending notification is gone once emitted.
        assert_eq!(s.scan(b"\x1b]99;i=7;again\x07"), [Mark::Notify(Some("again".into()))]);
        // Title chunks accumulate, and the body may come first.
        assert_eq!(
            scan(b"\x1b]99;i=2:d=0:p=body;Done\x07\x1b]99;i=2:d=0;Te\x07\x1b]99;i=2;sts\x07"),
            [Mark::Notify(Some("Tests: Done".into()))]
        );
        // A base64 payload is decoded, with or without padding.
        assert_eq!(scan(b"\x1b]99;e=1;SGVsbG8=\x07"), [Mark::Notify(Some("Hello".into()))]);
        assert_eq!(scan(b"\x1b]99;e=1;SGVsbG8\x07"), [Mark::Notify(Some("Hello".into()))]);
        assert_eq!(
            scan(b"\x1b]99;e=1:p=body;8J+mgA==\x07"),
            [Mark::Notify(Some("\u{1f980}".into()))]
        );
        // Invalid base64, a p value that is not title or body, and no separator give nothing.
        assert!(scan(b"\x1b]99;e=1;SGV$bG8=\x07").is_empty());
        assert!(scan(b"\x1b]99;e=1;S\x07").is_empty());
        assert!(scan(b"\x1b]99;p=close;x\x07").is_empty());
        assert!(scan(b"\x1b]99;p=?;x\x07").is_empty());
        assert!(scan(b"\x1b]99;no separator\x07").is_empty());
    }

    #[test]
    fn osc_99_keeps_one_pending_notification_and_caps_the_text() {
        // A new id replaces the pending one, so the first id's final chunk stands alone.
        let mut s = Osc133Scanner::default();
        assert!(s.scan(b"\x1b]99;i=1:d=0:p=title;one\x07").is_empty());
        assert!(s.scan(b"\x1b]99;i=2:d=0:p=title;two\x07").is_empty());
        assert_eq!(
            s.scan(b"\x1b]99;i=1:p=body;late\x07"),
            [Mark::Notify(Some("late".into()))]
        );
        assert_eq!(
            s.scan(b"\x1b]99;i=2:p=body;b\x07"),
            [Mark::Notify(Some("two: b".into()))]
        );
        // Chunks that never end cannot grow the text past the cap.
        let mut s = Osc133Scanner::default();
        for _ in 0..50 {
            assert!(
                s.scan(format!("\x1b]99;i=1:d=0;{}\x07", "x".repeat(100)).as_bytes())
                    .is_empty()
            );
        }
        assert_eq!(
            s.scan(b"\x1b]99;i=1:p=body;b\x07"),
            [Mark::Notify(Some("x".repeat(200)))]
        );
        for character in ["x", "界", "🦀"] {
            let bytes = format!("\x1b]99;;{}\x07", character.repeat(500));
            assert_split(bytes.as_bytes(), &[Mark::Notify(Some(character.repeat(200)))]);
        }
    }

    #[test]
    fn osc_99_survives_split_reads() {
        assert_split(b"a\x1b]99;;Build done\x07b", &[Mark::Notify(Some("Build done".into()))]);
        assert_split(b"\x1b]99;e=1;SGVsbG8=\x1b\\", &[Mark::Notify(Some("Hello".into()))]);
        assert_split(
            b"\x1b]99;i=3:d=0:p=title;Tests\x07\x1b]99;i=3:p=body;Green\x1b\\",
            &[Mark::Notify(Some("Tests: Green".into()))],
        );
    }

    #[test]
    fn osc777_notify_joins_title_and_body() {
        assert_eq!(
            scan(b"\x1b]777;notify;Tests;All green\x07"),
            [Mark::Notify(Some("Tests: All green".into()))]
        );
        assert_eq!(
            scan(b"\x1b]777;notify;;All;green\x1b\\"),
            [Mark::Notify(Some("All;green".into()))]
        );
        assert_eq!(scan(b"\x1b]777;notify;; \x07"), [Mark::Notify(None)]);
        assert!(scan(b"\x1b]777;notify;missing body\x07").is_empty());
    }

    #[test]
    fn bare_bel_is_a_bell() {
        assert_eq!(scan(b"abc\x07def"), [Mark::Bell]);
        assert_eq!(scan(b"\x07\x07"), [Mark::Bell, Mark::Bell]);
    }

    #[test]
    fn bel_ending_a_title_osc_is_not_a_bell() {
        assert!(scan(b"\x1b]0;title\x07\x1b]2;t\x07").is_empty());
        assert!(scan(b"\x1b]8;;https://x\x07text\x1b]8;;\x07").is_empty());
    }

    #[test]
    fn bel_inside_a_dcs_is_not_a_bell() {
        for start in *b"P_^X" {
            let bytes = [0x1b, start, b'a', 0x07, b'b', 0x1b, b'\\'];
            assert!(scan(&bytes).is_empty());
            let mut bytes = vec![0x1b, start];
            bytes.extend_from_slice(b"\x1b]9;still data\x07\x1b\x1b\\\x07");
            assert_split(&bytes, &[Mark::Bell]);
        }
    }

    fn assert_split(bytes: &[u8], expected: &[Mark]) {
        for cut in 0..=bytes.len() {
            let mut s = Osc133Scanner::default();
            let mut out = s.scan(&bytes[..cut]);
            out.extend(s.scan(&bytes[cut..]));
            assert_eq!(out, expected, "cut at {cut}");
        }
        let mut s = Osc133Scanner::default();
        let out: Vec<_> = bytes
            .iter()
            .flat_map(|byte| s.scan(std::slice::from_ref(byte)))
            .collect();
        assert_eq!(out, expected, "one byte at a time");
    }

    #[test]
    fn sequence_split_across_reads_is_seen_once() {
        for end in ["\x07", "\x1b\\"] {
            assert_split(
                format!("\x1b]9;Build done{end}").as_bytes(),
                &[Mark::Notify(Some("Build done".into()))],
            );
            assert_split(format!("\x1b]0;t{end}\x07").as_bytes(), &[Mark::Bell]);
            assert_split(format!("\x1b]0;{}{end}\x07", "x".repeat(500)).as_bytes(), &[Mark::Bell]);
            assert_split(
                format!("\x1b]8;;https://{}{end}\x07", "x".repeat(500)).as_bytes(),
                &[Mark::Bell],
            );
        }
        for start in ['P', '_', '^', 'X'] {
            assert_split(
                format!("\x1b{start}{}\x07data\x1b\\\x07", "x".repeat(500)).as_bytes(),
                &[Mark::Bell],
            );
        }
        assert_split(
            "\x1b]777;notify;Tests;All;green\x1b\\".as_bytes(),
            &[Mark::Notify(Some("Tests: All;green".into()))],
        );
    }

    #[test]
    fn notification_text_is_capped() {
        for character in ["x", "界", "🦀"] {
            let bytes = format!("\x1b]9;{}\x07", character.repeat(500));
            assert_split(bytes.as_bytes(), &[Mark::Notify(Some(character.repeat(200)))]);
        }
        let bytes = format!("\x1b]777;notify;{};{}\x1b\\", "界".repeat(100), "🦀".repeat(500));
        assert_split(
            bytes.as_bytes(),
            &[Mark::Notify(Some(format!("{}: {}", "界".repeat(100), "🦀".repeat(98))))],
        );
        let bytes = format!("\x1b]777;notify;{};body\x07", "界".repeat(500));
        assert_split(bytes.as_bytes(), &[Mark::Notify(Some("界".repeat(200)))]);
    }

    #[test]
    fn notification_trimming_precedes_the_character_cap() {
        let text = format!("{}{}z", "x".repeat(199), " ".repeat(500));
        let expected = [Mark::Notify(Some(format!("{} ", "x".repeat(199))))];
        assert_split(format!("\x1b]9;{text}\x07").as_bytes(), &expected);
        assert_split(format!("\x1b]777;notify;;{text}\x07").as_bytes(), &expected);
        assert_split(
            b"\x1b]777;notify; Tests ; All green \x07",
            &[Mark::Notify(Some("Tests :  All green".into()))],
        );
    }

    #[test]
    fn notification_utf8_is_decoded_lossily_across_reads() {
        for body in [&b"a\xffb"[..], &b"\xf0\x9f"[..], &b"\xe2\x82x\xff\xc0\xaf"[..]] {
            let mut bytes = b"\x1b]9;".to_vec();
            bytes.extend_from_slice(body);
            bytes.push(0x07);
            assert_split(
                &bytes,
                &[Mark::Notify(Some(String::from_utf8_lossy(body).into_owned()))],
            );
        }
        assert_split(
            "\x1b]9;\u{2003}🦀界\u{2003}\x07".as_bytes(),
            &[Mark::Notify(Some("🦀界".into()))],
        );
    }

    #[test]
    fn long_strings_keep_framing_and_notifications_stay_bounded() {
        for start in ["\x1b]0;", "\x1b]8;;", "\x1bP", "\x1b_", "\x1b^", "\x1bX"] {
            let bytes = format!("{start}{}\x07\x1b\\\x07", "x".repeat(10_000));
            for size in [1, 127, 4096, bytes.len()] {
                let mut s = Osc133Scanner::default();
                let out: Vec<_> = bytes.as_bytes().chunks(size).flat_map(|chunk| s.scan(chunk)).collect();
                assert_eq!(out, [Mark::Bell]);
            }
        }
        for start in ["\x1b]9;", "\x1b]777;notify;;"] {
            let bytes = format!("{start}{}{}\x07", " ".repeat(10_000), "🦀".repeat(10_000));
            let mut s = Osc133Scanner::default();
            let out: Vec<_> = bytes.as_bytes().chunks(127).flat_map(|chunk| s.scan(chunk)).collect();
            assert_eq!(out, [Mark::Notify(Some("🦀".repeat(200)))]);
        }
    }

    #[test]
    fn legacy_payload_limits_survive_split_terminators() {
        for (start, body, expected) in [
            ("133;", format!("A;{}", "x".repeat(MAX_CARRY - 2)), Mark::PromptStart),
            (
                "633;E;",
                "x".repeat(MAX_LINE_BODY),
                Mark::CommandLine("x".repeat(MAX_LINE_BODY)),
            ),
            ("9;9;", "x".repeat(MAX_LINE_BODY), Mark::Cwd("x".repeat(MAX_LINE_BODY))),
            (
                "7;",
                format!("file://h/{}", "x".repeat(MAX_LINE_BODY - 9)),
                Mark::Cwd(format!("/{}", "x".repeat(MAX_LINE_BODY - 9))),
            ),
        ] {
            let mut s = Osc133Scanner::default();
            assert!(s.scan(format!("\x1b]{start}{body}\x1b").as_bytes()).is_empty());
            assert_eq!(s.scan(b"\\"), [expected]);
            assert!(scan(format!("\x1b]{start}{body}x\x07").as_bytes()).is_empty());
        }
    }

    #[test]
    fn strings_abort_and_the_scanner_recovers() {
        assert_split(b"\x1b]9;discard\x1b]133;A\x07", &[Mark::PromptStart]);
        for start in [']', 'P', '_', '^', 'X'] {
            for abort in ['\x18', '\x1a'] {
                assert_split(
                    format!("\x1b{start}discard{abort}\x07\x1b]133;A\x07").as_bytes(),
                    &[Mark::Bell, Mark::PromptStart],
                );
            }
        }
    }

    #[test]
    fn finds_all_four_marks() {
        let out = scan(b"\x1b]133;A\x07$ \x1b]133;B\x1b\\ls\r\n\x1b]133;C\x07files\x1b]133;D;0\x1b\\");
        assert_eq!(
            out,
            [
                Mark::PromptStart,
                Mark::CommandStart,
                Mark::Executed,
                Mark::Finished(Some(0))
            ]
        );
    }

    #[test]
    fn d_with_and_without_code() {
        assert_eq!(scan(b"\x1b]133;D;1\x07"), [Mark::Finished(Some(1))]);
        assert_eq!(scan(b"\x1b]133;D\x07"), [Mark::Finished(None)]);
        assert_eq!(scan(b"\x1b]133;D;x\x07"), [Mark::Finished(None)]);
        assert_eq!(scan(b"\x1b]133;D;0;aid=3\x07"), [Mark::Finished(Some(0))]);
        assert_eq!(scan(b"\x1b]133;D;-1\x07"), [Mark::Finished(Some(-1))]);
    }

    #[test]
    fn split_across_chunks() {
        let all: &[u8] = b"a\x1b]133;D;2\x07b";
        for cut in 0..=all.len() {
            let mut s = Osc133Scanner::default();
            let mut out = s.scan(&all[..cut]);
            out.extend(s.scan(&all[cut..]));
            assert_eq!(out, [Mark::Finished(Some(2))], "cut at {cut}");
        }
    }

    #[test]
    fn split_at_the_st_escape() {
        let mut s = Osc133Scanner::default();
        assert!(s.scan(b"\x1b]133;A\x1b").is_empty());
        assert_eq!(s.scan(b"\\x"), [Mark::PromptStart]);
    }

    #[test]
    fn a_mark_starting_after_another_escape_at_the_end_is_kept() {
        let mut s = Osc133Scanner::default();
        assert!(s.scan(b"\x1b[0m\x1b[\x1b]13").is_empty());
        assert_eq!(s.scan(b"3;C\x07"), [Mark::Executed]);
    }

    #[test]
    fn ignores_other_osc() {
        assert!(scan(b"\x1b]1337;A\x07\x1b]1330;A\x07\x1b]9;4;9\x07\x1b]9;9\x07").is_empty());
        assert!(scan(b"\x1b]133;Z\x07").is_empty());
    }

    #[test]
    fn oversized_carry_is_dropped() {
        let mut s = Osc133Scanner::default();
        let mut long = b"\x1b]133;".to_vec();
        long.extend(std::iter::repeat_n(b'x', 1000));
        assert!(s.scan(&long).is_empty());
        assert_eq!(s.scan(b"\x07 \x1b]133;A\x07"), [Mark::PromptStart]);
    }

    #[test]
    fn command_line_ends_with_bel_or_st() {
        assert_eq!(
            scan(b"\x1b]633;E;git status\x07"),
            [Mark::CommandLine("git status".into())]
        );
        assert_eq!(
            scan(b"\x1b]633;E;git status\x1b\\"),
            [Mark::CommandLine("git status".into())]
        );
    }

    #[test]
    fn command_line_escapes() {
        assert_eq!(
            scan(b"\x1b]633;E;a\\x3bb\\\\c\x07"),
            [Mark::CommandLine("a;b\\c".into())]
        );
        assert_eq!(
            scan(b"\x1b]633;E;a\\x0ab\\x41\x07"),
            [Mark::CommandLine("a\nbA".into())]
        );
        // Invalid UTF-8 is decoded lossily.
        assert_eq!(
            scan(b"\x1b]633;E;a\\xffb\x07"),
            [Mark::CommandLine("a\u{fffd}b".into())]
        );
    }

    #[test]
    fn other_633_letters_are_ignored() {
        assert!(scan(b"\x1b]633;A\x07").is_empty());
        assert!(scan(b"\x1b]633;D;0\x07\x1b]633;P;Cwd=x\x07").is_empty());
    }

    #[test]
    fn command_line_split_across_chunks() {
        let all: &[u8] = b"a\x1b]633;E;ls -la\x1b\\b";
        for cut in 0..=all.len() {
            let mut s = Osc133Scanner::default();
            let mut out = s.scan(&all[..cut]);
            out.extend(s.scan(&all[cut..]));
            assert_eq!(out, [Mark::CommandLine("ls -la".into())], "cut at {cut}");
        }
    }

    #[test]
    fn command_line_and_133_marks_mix() {
        let out = scan(b"\x1b]633;E;x\x07\x1b]133;C\x07\x1b]133;D;0\x07");
        assert_eq!(
            out,
            [Mark::CommandLine("x".into()), Mark::Executed, Mark::Finished(Some(0))]
        );
    }

    #[test]
    fn oversized_command_line_is_dropped_and_the_scanner_recovers() {
        let mut s = Osc133Scanner::default();
        let mut long = b"\x1b]633;E;".to_vec();
        long.extend(std::iter::repeat_n(b'x', 5000));
        assert!(s.scan(&long).is_empty());
        assert_eq!(s.scan(b"\x07 \x1b]133;A\x07"), [Mark::PromptStart]);
        // Terminated in the same read: still no mark.
        long.push(0x07);
        assert!(s.scan(&long).is_empty());
        assert_eq!(s.scan(b"\x1b]633;E;ok\x07"), [Mark::CommandLine("ok".into())]);
    }

    #[test]
    fn a_long_command_line_under_the_limit_survives_a_split() {
        let mut s = Osc133Scanner::default();
        let mut first = b"\x1b]633;E;".to_vec();
        first.extend(std::iter::repeat_n(b'y', 3000));
        assert!(s.scan(&first).is_empty());
        let out = s.scan(b"z\x07");
        assert_eq!(out.len(), 1);
        let Mark::CommandLine(text) = &out[0] else { panic!() };
        assert_eq!(text.len(), 3001);
    }

    #[test]
    fn osc_9_9_gives_the_folder_with_st_or_bel() {
        assert_eq!(scan(b"\x1b]9;9;C:\\x\\y\x1b\\"), [Mark::Cwd("C:\\x\\y".into())]);
        assert_eq!(scan(b"\x1b]9;9;C:\\x\\y\x07"), [Mark::Cwd("C:\\x\\y".into())]);
        assert_eq!(scan(b"\x1b]9;9;\"C:\\a b\"\x07"), [Mark::Cwd("C:\\a b".into())]);
    }

    #[test]
    fn osc_7_gives_the_decoded_path_without_the_host() {
        assert_eq!(
            scan(b"\x1b]7;file://host/home/u/a%20b\x07"),
            [Mark::Cwd("/home/u/a b".into())]
        );
        assert_eq!(scan(b"\x1b]7;file:///C:/x\x07"), [Mark::Cwd("C:/x".into())]);
        assert_eq!(scan(b"\x1b]7;file:///home/u\x1b\\"), [Mark::Cwd("/home/u".into())]);
        assert!(scan(b"\x1b]7;http://h/x\x07").is_empty());
    }

    #[test]
    fn a_folder_split_across_chunks_is_found() {
        for all in [&b"a\x1b]9;9;C:\\x\x1b\\b"[..], &b"a\x1b]7;file://h/x%20y\x07b"[..]] {
            for cut in 0..=all.len() {
                let mut s = Osc133Scanner::default();
                let mut out = s.scan(&all[..cut]);
                out.extend(s.scan(&all[cut..]));
                assert_eq!(out.len(), 1, "cut at {cut}: {out:?}");
                assert!(matches!(out[0], Mark::Cwd(_)));
            }
        }
    }

    #[test]
    fn an_oversized_folder_is_dropped_and_the_scanner_recovers() {
        let mut s = Osc133Scanner::default();
        let mut long = b"\x1b]9;9;".to_vec();
        long.extend(std::iter::repeat_n(b'x', 5000));
        assert!(s.scan(&long).is_empty());
        assert_eq!(s.scan(b"\x07 \x1b]133;A\x07"), [Mark::PromptStart]);
        // Terminated in the same read: still no mark.
        long.push(0x07);
        assert!(s.scan(&long).is_empty());
        assert_eq!(s.scan(b"\x1b]9;9;ok\x07"), [Mark::Cwd("ok".into())]);
    }
}
