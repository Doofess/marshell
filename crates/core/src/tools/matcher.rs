//! Reads a tool's output: port lines from an escape-stripped line stream, and prompt text from the screen's rows.

use super::profile::Profile;
use marshell_protocol::tools::PortChip;
use std::collections::HashSet;
use std::sync::Arc;

/// What the matcher found in a chunk of output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolEvent {
    /// A prompt of a profiled tool shows; the text is the profile's own phrase.
    NeedsYou(String),
    /// The raised prompt is gone.
    Cleared,
    /// A new loopback port was announced.
    Port(PortChip),
}

/// Never logs, stores or returns a matched line, its neighbours or a captured secret. Only a normalized
/// `PortChip` and a profile's own phrase leave it.
pub struct Matcher {
    profiles: Vec<Arc<Profile>>,
    splitter: LineSplitter,
    raised: Option<String>,
    seen: HashSet<u16>,
}

impl Matcher {
    pub fn new(profiles: Vec<Arc<Profile>>) -> Matcher {
        Matcher {
            profiles,
            splitter: LineSplitter::new(),
            raised: None,
            seen: HashSet::new(),
        }
    }

    /// `pass` is what the terminal got; `tail` is the screen's last non-empty rows up to and including the
    /// cursor row, right-trimmed and joined with '\n', at most 3 rows.
    pub fn feed(&mut self, pass: &[u8], tail: &str) -> Vec<ToolEvent> {
        let mut events = Vec::new();
        for line in self.splitter.push(pass) {
            let line: String = line.chars().map(|c| if BOX.contains(&c) { ' ' } else { c }).collect();
            for profile in &self.profiles {
                for re in profile.port_regexes() {
                    let Some(chip) = re
                        .captures(&line)
                        .and_then(|c| c.get(1))
                        .and_then(|m| normalize_port(m.as_str()))
                    else {
                        continue;
                    };
                    if self.seen.insert(chip.port) {
                        events.push(ToolEvent::Port(chip));
                    }
                }
            }
        }

        let hit = self
            .profiles
            .iter()
            .flat_map(|p| p.needs_you_regexes())
            .find(|n| n.regex.is_match(tail))
            .map(|n| n.pattern.phrase.clone());
        match (&self.raised, hit) {
            (None, Some(phrase)) => {
                self.raised = Some(phrase.clone());
                events.push(ToolEvent::NeedsYou(phrase));
            }
            // New output below the prompt (a line break reached the terminal) ends it. A spinner that rewrites
            // its row with `\r` or cursor moves does not.
            (Some(_), None) if pass.contains(&b'\n') => {
                self.raised = None;
                events.push(ToolEvent::Cleared);
            }
            _ => {}
        }
        events
    }
}

/// Characters that draw a table, replaced by spaces before port patterns run.
const BOX: [char; 8] = [
    '\u{2502}', '\u{2503}', '|', '\u{256d}', '\u{256e}', '\u{2570}', '\u{256f}', '\u{2500}',
];

/// Turns an announced address into a loopback chip, or nothing. Accepts `scheme://host:port[/path]`,
/// `host:port`, `[v6]:port` and a bare port. The authority is parsed strictly: anything that is not exactly a
/// loopback host and a port gives `None`.
pub fn normalize_port(capture: &str) -> Option<PortChip> {
    let capture = capture.trim();
    let (scheme, rest, had_scheme) = match capture.split_once("://") {
        Some((s, rest)) => {
            let scheme = if s.eq_ignore_ascii_case("http") {
                "http"
            } else if s.eq_ignore_ascii_case("https") {
                "https"
            } else {
                return None;
            };
            (scheme, rest, true)
        }
        None => ("http", capture, false),
    };
    let (authority, after) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
    let path: String = if after.starts_with('/') {
        let end = after.find(['?', '#']).unwrap_or(after.len());
        after[..end]
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '~' | '-'))
            .collect()
    } else {
        String::new()
    };

    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        let (v6, port) = inner.split_once("]:")?;
        let host = match v6 {
            "::1" => "[::1]",
            "::" => "localhost",
            _ => return None,
        };
        (host, port)
    } else if !authority.is_empty() && authority.bytes().all(|b| b.is_ascii_digit()) {
        if had_scheme {
            return None;
        }
        ("localhost", authority)
    } else {
        let (host, port) = authority.rsplit_once(':')?;
        (loopback_host(host)?, port)
    };

    let port = parse_port(port)?;
    Some(PortChip {
        port,
        url: format!("{scheme}://{host}:{port}{path}"),
        announced: true,
    })
}

/// `localhost`, `127.0.0.1` and `[::1]` keep their names; `0.0.0.0` and the rest of `127.0.0.0/8` become `localhost`.
fn loopback_host(host: &str) -> Option<&'static str> {
    if host.eq_ignore_ascii_case("localhost") {
        return Some("localhost");
    }
    let mut octets = [0u8; 4];
    let mut parts = host.split('.');
    for octet in &mut octets {
        let part = parts.next()?;
        let plain = !part.is_empty()
            && part.len() <= 3
            && part.bytes().all(|b| b.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'));
        if !plain {
            return None;
        }
        *octet = part.parse().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    match octets {
        [127, 0, 0, 1] => Some("127.0.0.1"),
        [127, ..] | [0, 0, 0, 0] => Some("localhost"),
        _ => None,
    }
}

/// 1-65535, digits only, no sign and no leading zero.
fn parse_port(s: &str) -> Option<u16> {
    if s.is_empty() || s.len() > 5 || s.starts_with('0') || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok().filter(|p| *p != 0)
}

/// A carried partial line is dropped past this many characters.
const LINE_CAP: usize = 4096;
/// A cursor-forward is clamped, so a hostile `ESC[999999999C` costs nothing.
const FORWARD_CAP: usize = 1000;
/// A string sequence (OSC, DCS, ...) that never ends is abandoned after this many bytes.
const STRING_CAP: usize = 4096;
/// CSI parameter bytes kept; the rest are counted out.
const PARAMS_CAP: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    Esc,
    /// After `ESC` and an intermediate byte (`ESC ( B`), waiting for the final byte.
    EscInter,
    Csi,
    /// An OSC, DCS, SOS, PM or APC string, ended by BEL or `ESC \`.
    Str,
    StrEsc,
}

/// Escape-stripped lines from a byte stream. Carries a partial line (and a partial UTF-8 character) between
/// chunks. `\r` returns to the line's start and later text overwrites; a cursor-forward counts as spaces; a
/// cursor move to another row ends the line. Holds a screen's text only for as long as it takes to yield it.
struct LineSplitter {
    state: State,
    params: Vec<u8>,
    string_len: usize,
    utf8: Vec<u8>,
    utf8_need: usize,
    line: Vec<char>,
    col: usize,
    /// The carried line passed `LINE_CAP`: the rest of it is dropped.
    dropping: bool,
}

impl LineSplitter {
    fn new() -> Self {
        LineSplitter {
            state: State::Ground,
            params: Vec::new(),
            string_len: 0,
            utf8: Vec::new(),
            utf8_need: 0,
            line: Vec::new(),
            col: 0,
            dropping: false,
        }
    }

    /// How many characters of a partial line are carried.
    #[cfg(test)]
    fn carried(&self) -> usize {
        self.line.len()
    }

    fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        for &b in bytes {
            self.byte(b, &mut out);
        }
        out
    }

    fn byte(&mut self, b: u8, out: &mut Vec<String>) {
        match self.state {
            State::Ground => self.ground(b, out),
            State::Esc => match b {
                b'[' => {
                    self.params.clear();
                    self.state = State::Csi;
                }
                b']' | b'P' | b'X' | b'^' | b'_' => {
                    self.string_len = 0;
                    self.state = State::Str;
                }
                0x20..=0x2f => self.state = State::EscInter,
                0x1b => {}
                _ => self.state = State::Ground,
            },
            State::EscInter => match b {
                0x20..=0x2f => {}
                0x1b => self.state = State::Esc,
                _ => self.state = State::Ground,
            },
            State::Csi => match b {
                0x20..=0x3f => {
                    if self.params.len() < PARAMS_CAP {
                        self.params.push(b);
                    }
                }
                0x40..=0x7e => {
                    self.csi(b, out);
                    self.state = State::Ground;
                }
                0x1b => self.state = State::Esc,
                _ => {}
            },
            State::Str => {
                self.string_len += 1;
                match b {
                    0x07 => self.state = State::Ground,
                    0x1b => self.state = State::StrEsc,
                    _ if self.string_len > STRING_CAP => self.state = State::Ground,
                    _ => {}
                }
            }
            State::StrEsc => match b {
                b'\\' => self.state = State::Ground,
                0x1b => {}
                _ => {
                    self.state = State::Esc;
                    self.byte(b, out);
                }
            },
        }
    }

    fn ground(&mut self, b: u8, out: &mut Vec<String>) {
        if b >= 0x80 {
            self.utf8_byte(b);
            return;
        }
        if !self.utf8.is_empty() {
            self.flush_utf8();
        }
        match b {
            0x1b => self.state = State::Esc,
            b'\n' => self.end_line(out),
            b'\r' => self.col = 0,
            0x08 => self.col = self.col.saturating_sub(1),
            b'\t' => self.put(' '),
            0x20..=0x7e => self.put(b as char),
            _ => {}
        }
    }

    fn utf8_byte(&mut self, b: u8) {
        if b & 0xc0 == 0x80 {
            if self.utf8.is_empty() {
                self.put('\u{fffd}');
                return;
            }
            self.utf8.push(b);
        } else {
            self.flush_utf8();
            self.utf8_need = match b {
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => {
                    self.put('\u{fffd}');
                    return;
                }
            };
            self.utf8.push(b);
        }
        if self.utf8.len() == self.utf8_need {
            let c = std::str::from_utf8(&self.utf8)
                .ok()
                .and_then(|s| s.chars().next())
                .unwrap_or('\u{fffd}');
            self.utf8.clear();
            self.put(c);
        }
    }

    /// An incomplete multi-byte character becomes one replacement character.
    fn flush_utf8(&mut self) {
        if !self.utf8.is_empty() {
            self.utf8.clear();
            self.put('\u{fffd}');
        }
    }

    fn csi(&mut self, final_byte: u8, out: &mut Vec<String>) {
        // Only plain numeric parameters: `ESC[?25l` and the like are dropped.
        if !self.params.iter().all(|b| b.is_ascii_digit() || *b == b';') {
            return;
        }
        match final_byte {
            // CUF, which ConPTY uses for a run of spaces.
            b'C' => {
                let first = self.params.split(|b| *b == b';').next().unwrap_or(&[]);
                let n = std::str::from_utf8(first)
                    .ok()
                    .and_then(|s| s.parse::<usize>().ok())
                    .filter(|n| *n > 0)
                    .unwrap_or(1)
                    .min(FORWARD_CAP);
                for _ in 0..n {
                    if self.col < self.line.len() {
                        self.col += 1;
                    } else {
                        self.put(' ');
                    }
                }
            }
            // CUP, HVP, VPA and CUD move to another row.
            b'H' | b'f' | b'd' | b'B' => self.end_line(out),
            _ => {}
        }
    }

    fn put(&mut self, c: char) {
        if self.dropping {
            return;
        }
        if self.col < self.line.len() {
            self.line[self.col] = c;
        } else if self.line.len() >= LINE_CAP {
            self.line.clear();
            self.col = 0;
            self.dropping = true;
            return;
        } else {
            self.line.push(c);
        }
        self.col += 1;
    }

    fn end_line(&mut self, out: &mut Vec<String>) {
        if !self.dropping && self.line.iter().any(|c| !c.is_whitespace()) {
            out.push(self.line.iter().collect());
        }
        self.line.clear();
        self.col = 0;
        self.dropping = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tools;

    fn matcher(ids: &[&str]) -> Matcher {
        let tools = Tools::shipped();
        Matcher::new(ids.iter().map(|id| tools.profile(id).unwrap().clone()).collect())
    }

    fn chip(port: u16, url: &str) -> ToolEvent {
        ToolEvent::Port(PortChip {
            port,
            url: url.into(),
            announced: true,
        })
    }

    fn splitter() -> LineSplitter {
        LineSplitter::new()
    }

    #[test]
    fn vite_bold_port_is_read_after_stripping() {
        let mut m = matcher(&["vite"]);
        let out = m.feed(
            b"  \x1b[32m\xe2\x9e\x9c\x1b[39m  \x1b[1mLocal\x1b[22m:   \x1b[36mhttp://localhost:\x1b[1m5173\x1b[22m/\x1b[39m\n",
            "",
        );
        assert_eq!(out, [chip(5173, "http://localhost:5173/")]);
    }

    #[test]
    fn a_line_split_across_two_chunks_is_read_once() {
        let mut m = matcher(&["vite"]);
        assert!(m.feed("  \u{279c}  Local:   http://local".as_bytes(), "").is_empty());
        let out = m.feed(b"host:5173/\r\n", "");
        assert_eq!(out, [chip(5173, "http://localhost:5173/")]);
        // The same line again is not a new port.
        assert!(
            m.feed("  \u{279c}  Local:   http://localhost:5173/\r\n".as_bytes(), "")
                .is_empty()
        );
        // A multi-byte character cut between two chunks survives.
        let mut m = matcher(&["vite"]);
        let bytes = "  \u{279c}  Local:   http://localhost:5174/\n".as_bytes();
        let cut = bytes.iter().position(|b| *b == 0xe2).unwrap() + 1;
        assert!(m.feed(&bytes[..cut], "").is_empty());
        assert_eq!(m.feed(&bytes[cut..], ""), [chip(5174, "http://localhost:5174/")]);
    }

    #[test]
    fn conpty_cursor_forward_counts_as_spaces() {
        let mut m = matcher(&["vite"]);
        let out = m.feed(b"  \xe2\x9e\x9c  Local:\x1b[3Chttp://localhost:5173/\r\n", "");
        assert_eq!(out, [chip(5173, "http://localhost:5173/")]);
        let mut m = matcher(&["next"]);
        let out = m.feed(b"- Local:\x1b[8Chttp://localhost:3000\r\n", "");
        assert_eq!(out, [chip(3000, "http://localhost:3000")]);
        // A cursor-forward with no number moves one cell.
        let mut s = splitter();
        assert_eq!(s.push(b"a\x1b[Cb\n"), ["a b"]);
        // A move to another row ends the line, like a newline.
        assert_eq!(s.push(b"first\x1b[5;1Hsecond\n"), ["first", "second"]);
        assert_eq!(s.push(b"one\x1b[7dtwo\x1b[2Bthree\n"), ["one", "two", "three"]);
    }

    /// What PowerShell's `Write-Host` actually sends through ConPTY (captured on Windows 11, 100 columns): the
    /// text with its spaces, SGR resets around each row, terminal queries, and a CUP to the home cell.
    #[test]
    fn conpty_output_as_captured_gives_both_ports() {
        let raw = b"\x1b[1t\x1b[c\x1b[?1004h\x1b[?9001h\x1b[1;1H\x1b[0m\x1b[0m  Local:   http://localhost:5173/\x1b[0m\x1b[0m\r\n\x1b[0m\x1b[0m- Local:        http://localhost:3000\x1b[0m\x1b[0m\r\n\x1b[?1004l\x1b[?9001l";
        let mut m = matcher(&["next"]);
        assert_eq!(
            m.feed(raw, ""),
            [
                chip(5173, "http://localhost:5173/"),
                chip(3000, "http://localhost:3000")
            ]
        );
    }

    #[test]
    fn osc8_links_and_box_characters_are_stripped() {
        let mut m = matcher(&["firebase"]);
        let row = "\u{2502} Authentication \u{2502} \x1b]8;;http://127.0.0.1:9099\x1b\\127.0.0.1:9099\x1b]8;;\x1b\\ \u{2502} \x1b]8;;http://127.0.0.1:4000/auth\x07http://127.0.0.1:4000/auth\x1b]8;;\x07 \u{2502}\r\n";
        let out = m.feed(row.as_bytes(), "");
        assert_eq!(out, [chip(9099, "http://127.0.0.1:9099")]);
        // The ASCII pipe is a box character too.
        let mut m = matcher(&["firebase"]);
        let out = m.feed(
            b"| Firestore | 127.0.0.1:8080 | http://127.0.0.1:4000/firestore |\r\n",
            "",
        );
        assert_eq!(out, [chip(8080, "http://127.0.0.1:8080")]);
    }

    #[test]
    fn kubectl_forwarding_lines_give_one_chip() {
        let mut m = matcher(&["kubectl"]);
        let out = m.feed(
            b"Forwarding from 127.0.0.1:8080 -> 80\r\nForwarding from [::1]:8080 -> 80\r\n",
            "",
        );
        assert_eq!(out, [chip(8080, "http://127.0.0.1:8080")]);
    }

    #[test]
    fn a_port_capture_that_is_not_loopback_gives_no_chip() {
        for bad in [
            "http://localhost:3000@evil.example/",
            "http://user:pw@localhost:3000/",
            "http://localhost.evil.example:80",
            "http://192.168.1.5:8787",
            "localhost:0",
            "localhost:70000",
            "localhost:3000.evil",
            "127.0.0.1.evil.example:80",
            "localhost.:3000",
            "http://localhostevil:3000",
            "ftp://localhost:3000",
            "http://localhost",
            "http://[::2]:3000",
            "http://[fe80::1]:3000",
            "::1:3000",
            "http://127.0.0.1:3000\\@evil.example",
            "http://128.0.0.1:3000",
            "http://127.0.0.1.1:3000",
            "http://127.0.0.256:3000",
            "http://5173",
            "",
            "0",
            "99999",
        ] {
            assert_eq!(normalize_port(bad), None, "{bad}");
        }
    }

    #[test]
    fn loopback_captures_are_normalized() {
        let got = |s: &str| normalize_port(s).map(|c| (c.port, c.url));
        let some = |p: u16, u: &str| Some((p, u.to_string()));
        assert_eq!(got("http://localhost:5173/"), some(5173, "http://localhost:5173/"));
        assert_eq!(got("127.0.0.1:9099"), some(9099, "http://127.0.0.1:9099"));
        assert_eq!(got("[::1]:8080"), some(8080, "http://[::1]:8080"));
        assert_eq!(got("[::]:8080"), some(8080, "http://localhost:8080"));
        assert_eq!(got("0.0.0.0:4000"), some(4000, "http://localhost:4000"));
        assert_eq!(got("127.1.2.3:4000"), some(4000, "http://localhost:4000"));
        assert_eq!(got("2222"), some(2222, "http://localhost:2222"));
        assert_eq!(
            got("https://LOCALHOST:8443/a/b"),
            some(8443, "https://localhost:8443/a/b")
        );
        // Query and fragment are dropped, other path characters too.
        assert_eq!(
            got("http://localhost:3000/app?token=abc#frag"),
            some(3000, "http://localhost:3000/app")
        );
        assert_eq!(
            got("http://localhost:3000/a%20b"),
            some(3000, "http://localhost:3000/a20b")
        );
        assert_eq!(got("http://localhost:3000?x=1"), some(3000, "http://localhost:3000"));
        assert!(normalize_port("localhost:3000").unwrap().announced);
    }

    #[test]
    fn terraform_value_prompt_raises_and_new_output_clears() {
        let mut m = matcher(&["terraform"]);
        let tail = "Only 'yes' will be accepted to approve.\n\n  Enter a value:";
        let out = m.feed(b"\r\n  Enter a value: ", tail);
        assert_eq!(out, [ToolEvent::NeedsYou("Asks for a value".into())]);
        // Still showing: nothing new.
        assert!(m.feed(b"", tail).is_empty());
        // The answer's echo and the next output clear it.
        let out = m.feed(b"\r\nApply complete!\r\n", "Apply complete!");
        assert_eq!(out, [ToolEvent::Cleared]);
        // Nothing raised, nothing to clear.
        assert!(m.feed(b"more\r\n", "more").is_empty());
    }

    #[test]
    fn a_spinner_rewriting_the_matched_row_keeps_needs_you() {
        let mut m = matcher(&["vercel"]);
        let out = m.feed(
            "Waiting for authentication...".as_bytes(),
            "Waiting for authentication...",
        );
        assert_eq!(out, [ToolEvent::NeedsYou("Waiting for sign-in in the browser".into())]);
        for _ in 0..10 {
            let out = m.feed(
                "\r\x1b[2K\u{280b} Waiting for authentication...".as_bytes(),
                "\u{280b} Waiting for authentication...",
            );
            assert!(out.is_empty(), "{out:?}");
        }
        // A frame that doesn't match, redrawn in place, still isn't new output.
        assert!(m.feed(b"\r\x1b[2K\xe2\xa0\xb9 working", "working").is_empty());
        // A new row does clear it.
        assert_eq!(m.feed(b"\r\nDone\r\n", "Done"), [ToolEvent::Cleared]);
    }

    #[test]
    fn npx_ok_to_proceed_is_matched_through_the_launcher_profile() {
        let mut m = matcher(&["npx", "vite"]);
        let tail = "Need to install the following packages:\nvite@6.0.0\nOk to proceed? ";
        let out = m.feed(b"Ok to proceed? ", tail);
        assert_eq!(out, [ToolEvent::NeedsYou("Asks to continue".into())]);
    }

    #[test]
    fn a_runaway_line_is_dropped_not_grown() {
        let mut s = splitter();
        assert!(s.push(&vec![b'x'; 10_000]).is_empty());
        assert!(s.carried() <= 4096);
        // The rest of that line is dropped; the next line reads normally.
        assert_eq!(s.push(b"tail\nnext\n"), ["next"]);
        assert!(s.push(&vec![b'y'; 3_000]).is_empty());
        assert_eq!(s.carried(), 3_000);
    }

    #[test]
    fn return_overwrites_and_other_escapes_are_dropped() {
        let mut s = splitter();
        assert_eq!(s.push(b"abcdef\rXY\n"), ["XYcdef"]);
        assert_eq!(
            s.push(b"\x1b7\x1b(Bplain\x1b[?25l \x1b[38;5;12mblue\x1b[0m\x1bP1;2|data\x1b\\!\n"),
            ["plain blue!"]
        );
        assert_eq!(s.push(b"\x1b]0;a title\x07after\n"), ["after"]);
    }
}
