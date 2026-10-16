//! Asciicast v2 recording of a session: output, resizes and prompt marks. Keystrokes are never written.

/// What the first line of a recording says about the session.
pub struct CastHeader {
    pub cols: u16,
    pub rows: u16,
    pub started_unix: u64,
    pub shell: Option<String>,
}

/// The v2 header line, newline included. SHELL is left out when it is not known.
pub fn header_line(h: &CastHeader) -> String {
    let mut env = serde_json::json!({ "TERM": "xterm-256color" });
    if let Some(shell) = &h.shell {
        env["SHELL"] = shell.as_str().into();
    }
    let header = serde_json::json!({
        "version": 2,
        "width": h.cols,
        "height": h.rows,
        "timestamp": h.started_unix,
        "env": env,
    });
    format!("{header}\n")
}

/// Turns pty bytes into event lines. A character split across two reads is held back until it is whole.
#[derive(Default)]
pub struct CastEncoder {
    carry: Vec<u8>,
}

impl CastEncoder {
    /// An `"o"` event, or `None` when all that is left is the start of a character.
    pub fn output(&mut self, t: f64, bytes: &[u8]) -> Option<String> {
        self.carry.extend_from_slice(bytes);
        let cut = complete_prefix(&self.carry);
        let rest = self.carry.split_off(cut);
        let whole = std::mem::replace(&mut self.carry, rest);
        if whole.is_empty() {
            return None;
        }
        Some(event(t, "o", &String::from_utf8_lossy(&whole)))
    }

    pub fn resize(t: f64, cols: u16, rows: u16) -> String {
        event(t, "r", &format!("{cols}x{rows}"))
    }

    pub fn marker(t: f64, label: &str) -> String {
        event(t, "m", label)
    }
}

/// How much of `buf` can be decoded now: all of it, less an incomplete character at the end (at most 3 bytes).
/// Invalid bytes in the middle count as decodable; they become U+FFFD.
fn complete_prefix(buf: &[u8]) -> usize {
    let mut pos = 0;
    loop {
        match std::str::from_utf8(&buf[pos..]) {
            Ok(_) => return buf.len(),
            Err(e) => match e.error_len() {
                Some(n) => pos += e.valid_up_to() + n,
                None => return pos + e.valid_up_to(),
            },
        }
    }
}

fn event(t: f64, code: &str, data: &str) -> String {
    let data = serde_json::to_string(data).expect("a string always encodes");
    format!("[{t:.6},\"{code}\",{data}]\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(line: &str) -> String {
        let v: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
        v[2].as_str().unwrap().to_owned()
    }

    #[test]
    fn header_has_size_time_and_term() {
        let line = header_line(&CastHeader {
            cols: 80,
            rows: 24,
            started_unix: 1_700_000_000,
            shell: None,
        });
        assert!(line.ends_with('\n'));
        let v: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(v["version"], 2);
        assert_eq!(v["width"], 80);
        assert_eq!(v["height"], 24);
        assert_eq!(v["timestamp"], 1_700_000_000u64);
        assert_eq!(v["env"]["TERM"], "xterm-256color");
        assert!(v["env"].get("SHELL").is_none());
        let with = header_line(&CastHeader {
            cols: 1,
            rows: 1,
            started_unix: 0,
            shell: Some("pwsh".into()),
        });
        assert!(with.contains("\"SHELL\":\"pwsh\""));
    }

    #[test]
    fn output_escapes_control_bytes() {
        let line = CastEncoder::default().output(1.5, b"a\x1b[0m\r\n");
        assert_eq!(line.as_deref(), Some("[1.500000,\"o\",\"a\\u001b[0m\\r\\n\"]\n"));
    }

    #[test]
    fn split_utf8_is_carried() {
        let mut enc = CastEncoder::default();
        assert_eq!(data(&enc.output(0.1, &[b'x', 0xC3]).unwrap()), "x");
        let second = data(&enc.output(0.2, &[0xA9]).unwrap());
        assert_eq!(second, "é");
        assert!(!second.contains('\u{FFFD}'));
    }

    #[test]
    fn invalid_bytes_become_replacement() {
        let line = CastEncoder::default().output(0.0, &[0xFF, b'a']).unwrap();
        assert_eq!(data(&line), "\u{FFFD}a");
    }

    #[test]
    fn lone_partial_returns_none() {
        let mut enc = CastEncoder::default();
        assert_eq!(enc.output(0.0, &[0xE2, 0x82]), None);
        assert_eq!(data(&enc.output(0.1, &[0xAC]).unwrap()), "€");
    }

    #[test]
    fn resize_and_marker_lines() {
        assert_eq!(CastEncoder::resize(2.0, 120, 30), "[2.000000,\"r\",\"120x30\"]\n");
        assert_eq!(CastEncoder::marker(3.0, "prompt"), "[3.000000,\"m\",\"prompt\"]\n");
    }
}
