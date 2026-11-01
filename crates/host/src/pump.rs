//! The frame loop after the first Spawn: only Input and Resize are accepted. Kept free of
//! Windows types so it can be tested anywhere.

use marshell_protocol::host::{ToHost, read_to_host};
use std::io::{self, Read};

/// Where the core's input and resizes go: the pty.
pub trait PtySink {
    fn input(&mut self, b: &[u8]);
    fn resize(&mut self, cols: u16, rows: u16);
}

/// Reads frames until a clean EOF (`Ok`) or a protocol error (`Err`). A second Spawn is an error.
pub fn serve_frames(mut down: impl Read, sink: &mut dyn PtySink) -> io::Result<()> {
    while let Some(frame) = read_to_host(&mut down)? {
        match frame {
            ToHost::Input(bytes) => sink.input(&bytes),
            ToHost::Resize { cols, rows } => sink.resize(cols, rows),
            ToHost::Spawn(_) => {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "a second Spawn frame"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::host::{SpawnRequest, write_to_host};

    #[derive(Default)]
    struct Rec(Vec<String>);

    impl PtySink for Rec {
        fn input(&mut self, b: &[u8]) {
            self.0.push(format!("in:{}", String::from_utf8_lossy(b)));
        }
        fn resize(&mut self, cols: u16, rows: u16) {
            self.0.push(format!("resize:{cols}x{rows}"));
        }
    }

    fn spawn() -> ToHost {
        ToHost::Spawn(SpawnRequest {
            argv: vec!["cmd.exe".into()],
            cwd: None,
            env: vec![],
            cols: 80,
            rows: 24,
        })
    }

    fn bytes(frames: &[ToHost]) -> Vec<u8> {
        let mut buf = Vec::new();
        for f in frames {
            write_to_host(&mut buf, f).unwrap();
        }
        buf
    }

    #[test]
    fn input_and_resize_reach_the_sink_in_order() {
        let buf = bytes(&[
            ToHost::Input(b"a".to_vec()),
            ToHost::Resize { cols: 100, rows: 40 },
            ToHost::Input(b"b".to_vec()),
        ]);
        let mut sink = Rec::default();
        serve_frames(buf.as_slice(), &mut sink).unwrap();
        assert_eq!(sink.0, ["in:a", "resize:100x40", "in:b"]);
    }

    #[test]
    fn a_second_spawn_stops_the_loop_with_an_error() {
        let buf = bytes(&[ToHost::Input(b"a".to_vec()), spawn(), ToHost::Input(b"b".to_vec())]);
        let mut sink = Rec::default();
        let err = serve_frames(buf.as_slice(), &mut sink).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert_eq!(sink.0, ["in:a"]);
    }

    #[test]
    fn an_unknown_kind_stops_the_loop() {
        let mut buf = bytes(&[ToHost::Input(b"a".to_vec())]);
        buf.extend_from_slice(&[9, 0, 0, 0, 0]);
        buf.extend(bytes(&[ToHost::Input(b"b".to_vec())]));
        let mut sink = Rec::default();
        let err = serve_frames(buf.as_slice(), &mut sink).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert_eq!(sink.0, ["in:a"]);
    }

    #[test]
    fn eof_ends_the_loop_cleanly() {
        let mut sink = Rec::default();
        serve_frames(&b""[..], &mut sink).unwrap();
        assert!(sink.0.is_empty());
    }
}
