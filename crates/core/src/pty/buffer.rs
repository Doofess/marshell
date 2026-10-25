use std::collections::VecDeque;

/// Bytes of raw output kept per session for replay (roughly 10k+ lines).
pub const SCROLLBACK_BYTES: usize = 8 * 1024 * 1024;
/// Send output in frames of at most this size; also the "flush now" threshold.
pub const FLUSH_BYTES: usize = 64 * 1024;
/// Stop reading the pty when an attached client is this far behind.
pub const MAX_UNACKED: u64 = 1024 * 1024;

/// The last rendered rows, without formatting, trailing spaces or blank rows at the bottom.
pub(super) fn peek_rows(screen: &vt100::Screen, lines: usize) -> Vec<String> {
    let mut rows: Vec<String> = screen
        .rows(0, screen.size().1)
        .map(|row| row.trim_end_matches(' ').to_string())
        .collect();
    while rows.last().is_some_and(String::is_empty) {
        rows.pop();
    }
    rows.split_off(rows.len().saturating_sub(lines.clamp(1, 100)))
}

/// The rows a tool's prompt would be on: the screen's rows up to and including the cursor row, the last 3 non-empty
/// ones, right-trimmed and joined with '\n'.
pub(super) fn tail_rows(screen: &vt100::Screen) -> String {
    let cursor_row = usize::from(screen.cursor_position().0);
    let mut rows: Vec<String> = screen
        .rows(0, screen.size().1)
        .take(cursor_row + 1)
        .map(|row| row.trim_end().to_string())
        .filter(|row| !row.is_empty())
        .collect();
    rows.drain(..rows.len().saturating_sub(3));
    rows.join("\n")
}

/// A ring of output bytes addressed by absolute byte offset ("seq").
/// Old bytes fall off the front once `cap` is exceeded.
pub struct OutputBuffer {
    data: VecDeque<u8>,
    start: u64,
    cap: usize,
}

impl OutputBuffer {
    pub fn new(cap: usize) -> Self {
        Self {
            data: VecDeque::new(),
            start: 0,
            cap,
        }
    }
    pub fn start(&self) -> u64 {
        self.start
    }
    pub fn end(&self) -> u64 {
        self.start + self.data.len() as u64
    }
    pub fn push(&mut self, bytes: &[u8]) {
        self.data.extend(bytes);
        let excess = self.data.len().saturating_sub(self.cap);
        if excess > 0 {
            self.data.drain(..excess);
            self.start += excess as u64;
        }
    }
    pub fn read_from(&self, from: u64, max: usize) -> Option<Vec<u8>> {
        if from < self.start {
            return None;
        }
        let offset = ((from - self.start) as usize).min(self.data.len());
        let len = max.min(self.data.len() - offset);
        Some(self.data.range(offset..offset + len).copied().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peek_returns_the_last_rows_of_a_tui_redraw() {
        let mut parser = vt100::Parser::new(5, 20, 0);
        parser.process(b"a\r\nb\r\n\x1b[1;1Hz");
        assert_eq!(peek_rows(parser.screen(), 30), ["z", "b"]);
    }

    #[test]
    fn peek_clamps_lines() {
        let mut parser = vt100::Parser::new(110, 20, 0);
        let rows: Vec<String> = (0..105).map(|i| format!("row {i}")).collect();
        parser.process(rows.join("\r\n").as_bytes());
        assert_eq!(peek_rows(parser.screen(), 0), rows[104..]);
        assert_eq!(peek_rows(parser.screen(), 1), rows[104..]);
        assert_eq!(peek_rows(parser.screen(), 30), rows[75..]);
        assert_eq!(peek_rows(parser.screen(), 100), rows[5..]);
        assert_eq!(peek_rows(parser.screen(), usize::MAX), rows[5..]);
    }

    #[test]
    fn peek_trims_trailing_spaces_and_preserves_inner_blank_rows() {
        let mut parser = vt100::Parser::new(8, 20, 0);
        parser.process(b"old\r\n  \x1b[31mred\x1b[0m   \r\n   \r\nlast   \r\n   ");
        assert_eq!(peek_rows(parser.screen(), 3), ["  red", "", "last"]);
    }

    #[test]
    fn peek_blank_screen_has_no_rows() {
        let parser = vt100::Parser::new(5, 20, 0);
        assert!(peek_rows(parser.screen(), 30).is_empty());
    }

    #[test]
    fn the_tail_is_the_last_three_non_empty_rows_up_to_the_cursor_row() {
        let mut parser = vt100::Parser::new(8, 20, 0);
        parser.process(b"one\r\ntwo  \r\n\r\nthree\r\nfour   ok?");
        assert_eq!(tail_rows(parser.screen()), "two\nthree\nfour   ok?");
        // Rows below the cursor row are not part of it (a TUI that drew a footer, then moved up).
        parser.process(b"\x1b[8;1Hfooter\x1b[2;1H");
        assert_eq!(tail_rows(parser.screen()), "one\ntwo");
        // A blank screen has no tail.
        assert_eq!(tail_rows(vt100::Parser::new(5, 20, 0).screen()), "");
    }

    #[test]
    fn push_and_read() {
        let mut b = OutputBuffer::new(100);
        b.push(b"hello ");
        b.push(b"world");
        assert_eq!(b.end(), 11);
        assert_eq!(b.read_from(0, 100).unwrap(), b"hello world");
        assert_eq!(b.read_from(6, 3).unwrap(), b"wor");
    }

    #[test]
    fn read_at_end_is_empty_not_none() {
        let mut b = OutputBuffer::new(10);
        b.push(b"abc");
        assert_eq!(b.read_from(3, 10).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn eviction_moves_start_and_old_reads_return_none() {
        let mut b = OutputBuffer::new(4);
        b.push(b"abcdef");
        assert_eq!(b.start(), 2);
        assert_eq!(b.end(), 6);
        assert_eq!(b.read_from(2, 10).unwrap(), b"cdef");
        assert!(b.read_from(1, 10).is_none());
    }

    #[test]
    fn read_past_end_is_clamped_to_empty() {
        let mut b = OutputBuffer::new(10);
        b.push(b"ab");
        assert_eq!(b.read_from(9, 10).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn vt100_snapshot_reproduces_the_screen() {
        let mut p = vt100::Parser::new(5, 20, 0);
        p.process(b"hello\r\n\x1b[1mbold\x1b[0m world");
        let snapshot = p.screen().state_formatted();
        let mut q = vt100::Parser::new(5, 20, 0);
        q.process(&snapshot);
        assert_eq!(q.screen().contents(), p.screen().contents());
        assert_eq!(q.screen().cursor_position(), p.screen().cursor_position());
    }
}
