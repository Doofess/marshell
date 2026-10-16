//! Finds OSC 133 prompt and command marks (`ESC ] 133 ; X [; params] BEL|ST`) in shell output.
//! The scanner only reads: the bytes still go to the terminal unchanged, so xterm sees the marks too.

/// One OSC 133 mark: A, B, C and D[;exit code].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    PromptStart,
    CommandStart,
    Executed,
    Finished(Option<i32>),
}

const START: &[u8] = b"\x1b]133;";
/// A mark is a few bytes; anything unterminated past this is not one of ours.
const MAX_CARRY: usize = 128;

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
            if rest.len() < START.len() && START.starts_with(rest) {
                self.carry = rest.to_vec();
                break;
            }
            if !rest.starts_with(START) {
                i += 1;
                continue;
            }
            match terminator(&rest[START.len()..]) {
                End::Found { body_len, total } => {
                    if let Some(mark) = parse(&rest[START.len()..START.len() + body_len]) {
                        marks.push(mark);
                    }
                    i += START.len() + total;
                }
                End::Aborted(at) => i += START.len() + at,
                End::Incomplete => {
                    if rest.len() <= MAX_CARRY {
                        self.carry = rest.to_vec();
                        break;
                    }
                    // Too long to be a mark: skip its start and keep looking.
                    i += START.len();
                }
            }
        }
        marks
    }
}

enum End {
    /// The body is `body_len` bytes; the body plus its terminator is `total`.
    Found { body_len: usize, total: usize },
    /// Another escape sequence began at this offset, cancelling the OSC.
    Aborted(usize),
    Incomplete,
}

fn terminator(after: &[u8]) -> End {
    for (j, &b) in after.iter().enumerate() {
        match b {
            0x07 => return End::Found { body_len: j, total: j + 1 },
            0x1b => {
                return match after.get(j + 1) {
                    Some(b'\\') => End::Found { body_len: j, total: j + 2 },
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(bytes: &[u8]) -> Vec<Mark> {
        Osc133Scanner::default().scan(bytes)
    }

    #[test]
    fn finds_all_four_marks() {
        let out = scan(b"\x1b]133;A\x07$ \x1b]133;B\x1b\\ls\r\n\x1b]133;C\x07files\x1b]133;D;0\x1b\\");
        assert_eq!(out, [Mark::PromptStart, Mark::CommandStart, Mark::Executed, Mark::Finished(Some(0))]);
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
        assert!(scan(b"\x1b]7;file://h/tmp\x07\x1b]9;9;C:\\x\x1b\\\x1b]1337;A\x07\x1b]1330;A\x07").is_empty());
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
}
