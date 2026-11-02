//! The frames between the core and `marshell-host`, the helper that runs an elevated session.
//! Plain Rust, no ts-rs: the frontend never sees these.
//!
//! Wire format: `[kind u8][len u32 LE][payload]`.

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

/// The largest payload either side accepts.
pub const MAX_FRAME: usize = 1 << 20;

const SPAWN: u8 = 1;
const INPUT: u8 = 2;
const RESIZE: u8 = 3;
const STARTED: u8 = 0x81;
const FAILED: u8 = 0x82;
const OUTPUT: u8 = 0x83;
const EXIT: u8 = 0x84;

/// What the helper starts. `env` is the child's whole environment.
/// No `Debug`: it can hold a preset's key.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnRequest {
    pub argv: Vec<String>,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    pub cols: u16,
    pub rows: u16,
}

/// Core to helper.
#[derive(Clone, PartialEq, Eq)]
pub enum ToHost {
    Spawn(SpawnRequest),
    Input(Vec<u8>),
    Resize { cols: u16, rows: u16 },
}

/// Helper to core.
#[derive(Clone, PartialEq, Eq)]
pub enum FromHost {
    Started { pid: u32 },
    Failed(String),
    Output(Vec<u8>),
    Exit { code: i32 },
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

fn write_frame(w: &mut impl Write, kind: u8, payload: &[u8]) -> io::Result<()> {
    if payload.len() > MAX_FRAME {
        return Err(invalid("frame over the maximum size"));
    }
    let mut buf = Vec::with_capacity(5 + payload.len());
    buf.push(kind);
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(payload);
    w.write_all(&buf)?;
    w.flush()
}

/// `None` is a clean EOF at a frame boundary. EOF anywhere else is `InvalidData`.
fn read_frame(r: &mut impl Read) -> io::Result<Option<(u8, Vec<u8>)>> {
    let mut kind = [0u8; 1];
    loop {
        match r.read(&mut kind) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    let mut len = [0u8; 4];
    read_exact_frame(r, &mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(invalid("frame over the maximum size"));
    }
    let mut payload = vec![0u8; len];
    read_exact_frame(r, &mut payload)?;
    Ok(Some((kind[0], payload)))
}

fn read_exact_frame(r: &mut impl Read, buf: &mut [u8]) -> io::Result<()> {
    r.read_exact(buf).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            invalid("end of stream inside a frame")
        } else {
            e
        }
    })
}

fn fixed<const N: usize>(payload: &[u8]) -> io::Result<[u8; N]> {
    payload.try_into().map_err(|_| invalid("wrong payload length"))
}

pub fn write_to_host(w: &mut impl Write, m: &ToHost) -> io::Result<()> {
    match m {
        ToHost::Spawn(s) => {
            let json = serde_json::to_vec(s).map_err(|e| invalid(e.to_string()))?;
            write_frame(w, SPAWN, &json)
        }
        ToHost::Input(bytes) => write_frame(w, INPUT, bytes),
        ToHost::Resize { cols, rows } => {
            let mut p = [0u8; 4];
            p[..2].copy_from_slice(&cols.to_le_bytes());
            p[2..].copy_from_slice(&rows.to_le_bytes());
            write_frame(w, RESIZE, &p)
        }
    }
}

pub fn read_to_host(r: &mut impl Read) -> io::Result<Option<ToHost>> {
    let Some((kind, payload)) = read_frame(r)? else {
        return Ok(None);
    };
    let m = match kind {
        SPAWN => ToHost::Spawn(serde_json::from_slice(&payload).map_err(|e| invalid(e.to_string()))?),
        INPUT => ToHost::Input(payload),
        RESIZE => {
            let p: [u8; 4] = fixed(&payload)?;
            let cols = u16::from_le_bytes([p[0], p[1]]);
            let rows = u16::from_le_bytes([p[2], p[3]]);
            if !(1..=1000).contains(&cols) || !(1..=1000).contains(&rows) {
                return Err(invalid("resize out of bounds"));
            }
            ToHost::Resize { cols, rows }
        }
        k => return Err(invalid(format!("unknown frame kind {k}"))),
    };
    Ok(Some(m))
}

pub fn write_from_host(w: &mut impl Write, m: &FromHost) -> io::Result<()> {
    match m {
        FromHost::Started { pid } => write_frame(w, STARTED, &pid.to_le_bytes()),
        FromHost::Failed(text) => write_frame(w, FAILED, text.as_bytes()),
        FromHost::Output(bytes) => write_frame(w, OUTPUT, bytes),
        FromHost::Exit { code } => write_frame(w, EXIT, &code.to_le_bytes()),
    }
}

pub fn read_from_host(r: &mut impl Read) -> io::Result<Option<FromHost>> {
    let Some((kind, payload)) = read_frame(r)? else {
        return Ok(None);
    };
    let m = match kind {
        STARTED => FromHost::Started {
            pid: u32::from_le_bytes(fixed(&payload)?),
        },
        FAILED => FromHost::Failed(String::from_utf8(payload).map_err(|_| invalid("failure text is not UTF-8"))?),
        OUTPUT => FromHost::Output(payload),
        EXIT => FromHost::Exit {
            code: i32::from_le_bytes(fixed(&payload)?),
        },
        k => return Err(invalid(format!("unknown frame kind {k}"))),
    };
    Ok(Some(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn() -> SpawnRequest {
        SpawnRequest {
            argv: vec!["cmd.exe".into(), "/k".into()],
            cwd: Some("C:/w".into()),
            env: vec![("A".into(), "b".into())],
            cols: 120,
            rows: 30,
        }
    }

    fn kind_of(e: io::Result<impl Sized>) -> io::ErrorKind {
        e.err().expect("an error").kind()
    }

    #[test]
    fn every_message_round_trips() {
        let to = [
            ToHost::Spawn(spawn()),
            ToHost::Input(b"ls\r".to_vec()),
            ToHost::Input(Vec::new()),
            ToHost::Resize { cols: 1, rows: 1000 },
        ];
        let mut buf = Vec::new();
        for m in &to {
            write_to_host(&mut buf, m).unwrap();
        }
        let mut r = buf.as_slice();
        for m in &to {
            assert!(read_to_host(&mut r).unwrap().unwrap() == *m);
        }
        assert!(read_to_host(&mut r).unwrap().is_none());

        let from = [
            FromHost::Started { pid: 4242 },
            FromHost::Failed("no such folder".into()),
            FromHost::Output(vec![0, 1, 255]),
            FromHost::Exit { code: -1 },
        ];
        let mut buf = Vec::new();
        for m in &from {
            write_from_host(&mut buf, m).unwrap();
        }
        let mut r = buf.as_slice();
        for m in &from {
            assert!(read_from_host(&mut r).unwrap().unwrap() == *m);
        }
        assert!(read_from_host(&mut r).unwrap().is_none());
    }

    #[test]
    fn eof_at_a_boundary_is_none_and_mid_frame_is_an_error() {
        assert!(read_to_host(&mut &b""[..]).unwrap().is_none());
        // A header cut short.
        assert_eq!(
            kind_of(read_to_host(&mut &[INPUT, 3, 0][..])),
            io::ErrorKind::InvalidData
        );
        // A payload cut short.
        assert_eq!(
            kind_of(read_to_host(&mut &[INPUT, 3, 0, 0, 0, b'a'][..])),
            io::ErrorKind::InvalidData
        );
        assert_eq!(kind_of(read_from_host(&mut &[OUTPUT][..])), io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_frame_over_max_is_refused_before_reading_its_payload() {
        let mut header = vec![INPUT];
        header.extend_from_slice(&((MAX_FRAME + 1) as u32).to_le_bytes());
        assert_eq!(
            kind_of(read_to_host(&mut header.as_slice())),
            io::ErrorKind::InvalidData
        );
        let mut header = vec![OUTPUT];
        header.extend_from_slice(&((MAX_FRAME + 1) as u32).to_le_bytes());
        assert_eq!(
            kind_of(read_from_host(&mut header.as_slice())),
            io::ErrorKind::InvalidData
        );
        // The writer refuses too.
        let mut sink = Vec::new();
        assert_eq!(
            kind_of(write_to_host(&mut sink, &ToHost::Input(vec![0; MAX_FRAME + 1]))),
            io::ErrorKind::InvalidData
        );
        // Exactly MAX_FRAME is fine.
        write_to_host(&mut sink, &ToHost::Input(vec![0; MAX_FRAME])).unwrap();
        assert!(read_to_host(&mut sink.as_slice()).unwrap().is_some());
    }

    #[test]
    fn unknown_kind_and_bad_resize_are_invalid() {
        assert_eq!(
            kind_of(read_to_host(&mut &[9, 0, 0, 0, 0][..])),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            kind_of(read_from_host(&mut &[9, 0, 0, 0, 0][..])),
            io::ErrorKind::InvalidData
        );
        let resize = |cols: u16, rows: u16| {
            let mut f = vec![RESIZE, 4, 0, 0, 0];
            f.extend_from_slice(&cols.to_le_bytes());
            f.extend_from_slice(&rows.to_le_bytes());
            f
        };
        for (c, r) in [(0, 10), (10, 0), (1001, 10), (10, 1001)] {
            assert_eq!(
                kind_of(read_to_host(&mut resize(c, r).as_slice())),
                io::ErrorKind::InvalidData
            );
        }
        assert!(read_to_host(&mut resize(1000, 1).as_slice()).unwrap().is_some());
        // A wrong fixed length.
        assert_eq!(
            kind_of(read_to_host(&mut &[RESIZE, 3, 0, 0, 0, 1, 0, 1][..])),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            kind_of(read_from_host(&mut &[STARTED, 2, 0, 0, 0, 1, 0][..])),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            kind_of(read_from_host(&mut &[EXIT, 5, 0, 0, 0, 0, 0, 0, 0, 0][..])),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn a_from_host_kind_is_not_a_to_host_message() {
        let mut buf = Vec::new();
        write_from_host(&mut buf, &FromHost::Started { pid: 1 }).unwrap();
        assert_eq!(kind_of(read_to_host(&mut buf.as_slice())), io::ErrorKind::InvalidData);
        let mut buf = Vec::new();
        write_to_host(&mut buf, &ToHost::Input(vec![1])).unwrap();
        assert_eq!(kind_of(read_from_host(&mut buf.as_slice())), io::ErrorKind::InvalidData);
    }
}
