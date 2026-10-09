use std::collections::VecDeque;

/// Bytes of raw output kept per session for replay (roughly 10k+ lines).
pub const SCROLLBACK_BYTES: usize = 8 * 1024 * 1024;
/// Send output in frames of at most this size; also the "flush now" threshold.
pub const FLUSH_BYTES: usize = 64 * 1024;
/// Stop reading the pty when an attached client is this far behind.
pub const MAX_UNACKED: u64 = 1024 * 1024;

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
