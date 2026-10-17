//! Finds OSC 133 prompt and command marks (`ESC ] 133 ; X [; params] BEL|ST`) in shell output,
//! the typed command line a shell reports as `ESC ] 633 ; E ; <escaped text> BEL|ST`, and the folder it
//! reports as `ESC ] 9 ; 9 ; <path>` (PowerShell and cmd) or `ESC ] 7 ; file://<host>/<path>` (bash).
//! The scanner only reads: the bytes still go to the terminal unchanged, so xterm sees the marks too.

/// One mark: OSC 133 A, B, C and D[;exit code], the command line from OSC 633 E, or the folder from OSC 9;9 or 7.
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
}

const START: &[u8] = b"\x1b]133;";
const START_LINE: &[u8] = b"\x1b]633;E;";
const START_CWD9: &[u8] = b"\x1b]9;9;";
const START_CWD7: &[u8] = b"\x1b]7;";
/// A mark is a few bytes; anything unterminated past this is not one of ours.
const MAX_CARRY: usize = 128;
/// A command line or folder can be long, but a body past this is dropped.
const MAX_LINE_BODY: usize = 4096;

/// Remembers a mark cut off at the end of one read until the next read completes it.
#[derive(Default)]
pub struct Osc133Scanner {
    carry: Vec<u8>,
}

impl Osc133Scanner {
    pub fn scan(&mut self, bytes: &[u8]) -> Vec<Mark> {
        let mut buf = std::mem::take(&mut self.carry);
        buf.extend_from_slice(bytes);
        let mut marks = Vec::new();
        let mut i = 0;
        while let Some(off) = buf[i..].iter().position(|&b| b == 0x1b) {
            i += off;
            let rest = &buf[i..];
            if [START, START_LINE, START_CWD9, START_CWD7]
                .iter()
                .any(|s| rest.len() < s.len() && s.starts_with(rest))
            {
                self.carry = rest.to_vec();
                break;
            }
            let (start, max_body) = if rest.starts_with(START) {
                (START, MAX_CARRY)
            } else if rest.starts_with(START_LINE) {
                (START_LINE, MAX_LINE_BODY)
            } else if rest.starts_with(START_CWD9) {
                (START_CWD9, MAX_LINE_BODY)
            } else if rest.starts_with(START_CWD7) {
                (START_CWD7, MAX_LINE_BODY)
            } else {
                i += 1;
                continue;
            };
            match terminator(&rest[start.len()..]) {
                End::Found { body_len, total } => {
                    let body = &rest[start.len()..start.len() + body_len];
                    let mark = if start == START {
                        parse(body)
                    } else if body_len > max_body {
                        None
                    } else if start == START_LINE {
                        Some(Mark::CommandLine(unescape(body)))
                    } else if start == START_CWD9 {
                        cwd_from_osc9(body)
                    } else {
                        cwd_from_osc7(body)
                    };
                    marks.extend(mark);
                    i += start.len() + total;
                }
                End::Aborted(at) => i += start.len() + at,
                End::Incomplete => {
                    if rest.len() <= start.len() + max_body {
                        self.carry = rest.to_vec();
                        break;
                    }
                    // Too long to be a mark: skip its start and keep looking.
                    i += start.len();
                }
            }
        }
        marks
    }
}

enum End {
    /// The body is `body_len` bytes; the body plus its terminator is `total`.
    Found {
        body_len: usize,
        total: usize,
    },
    /// Another escape sequence began at this offset, cancelling the OSC.
    Aborted(usize),
    Incomplete,
}

fn terminator(after: &[u8]) -> End {
    for (j, &b) in after.iter().enumerate() {
        match b {
            0x07 => {
                return End::Found {
                    body_len: j,
                    total: j + 1,
                };
            }
            0x1b => {
                return match after.get(j + 1) {
                    Some(b'\\') => End::Found {
                        body_len: j,
                        total: j + 2,
                    },
                    Some(_) => End::Aborted(j),
                    None => End::Incomplete,
                };
            }
            _ => {}
        }
    }
    End::Incomplete
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
        assert!(scan(b"\x1b]1337;A\x07\x1b]1330;A\x07\x1b]9;4;3\x07\x1b]9;9\x07").is_empty());
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
