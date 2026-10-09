//! ConPTY (portable-pty 0.9 sets PSEUDOCONSOLE_INHERIT_CURSOR) asks for the cursor position with
//! ESC[6n at startup and holds back all output until it gets an answer. While no renderer is
//! attached, the core answers instead and removes the query from the stream. That way a renderer
//! that attaches later never answers it a second time.

const DSR: &[u8] = b"\x1b[6n";
/// "Cursor is at row 1, column 1."
pub const DSR_REPLY: &[u8] = b"\x1b[1;1R";

#[derive(Default)]
pub struct DsrFilter {
    carry: Vec<u8>,
}

impl DsrFilter {
    /// Returns the bytes to pass on, and how many queries need an answer.
    pub fn filter(&mut self, input: &[u8]) -> (Vec<u8>, usize) {
        let mut buf = std::mem::take(&mut self.carry);
        buf.extend_from_slice(input);
        let mut out = Vec::with_capacity(buf.len());
        let mut answered = 0;
        let mut i = 0;
        while i < buf.len() {
            let rest = &buf[i..];
            if rest.starts_with(DSR) {
                answered += 1;
                i += DSR.len();
            } else if rest.len() < DSR.len() && DSR.starts_with(rest) {
                // Might be the start of a query; wait for the next read.
                self.carry = rest.to_vec();
                break;
            } else {
                out.push(buf[i]);
                i += 1;
            }
        }
        (out, answered)
    }

    /// Bytes held back because they might start a query. Call this when a renderer attaches.
    pub fn take_carry(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.carry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_and_counts_a_whole_query() {
        let mut f = DsrFilter::default();
        assert_eq!(f.filter(b"a\x1b[6nb"), (b"ab".to_vec(), 1));
    }

    #[test]
    fn handles_a_query_split_across_reads() {
        let mut f = DsrFilter::default();
        assert_eq!(f.filter(b"x\x1b["), (b"x".to_vec(), 0));
        assert_eq!(f.filter(b"6ny"), (b"y".to_vec(), 1));
    }

    #[test]
    fn passes_look_alike_sequences() {
        let mut f = DsrFilter::default();
        assert_eq!(f.filter(b"\x1b[6m\x1b[2J"), (b"\x1b[6m\x1b[2J".to_vec(), 0));
    }

    #[test]
    fn counts_several_queries() {
        let mut f = DsrFilter::default();
        assert_eq!(f.filter(b"\x1b[6n\x1b[6n"), (Vec::new(), 2));
    }

    #[test]
    fn carry_is_released_on_attach() {
        let mut f = DsrFilter::default();
        let _ = f.filter(b"z\x1b[6");
        assert_eq!(f.take_carry(), b"\x1b[6".to_vec());
    }
}
