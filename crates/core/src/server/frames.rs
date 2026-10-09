use marshell_protocol::api::Resize;
use marshell_protocol::frames as op;

pub fn encode_output(seq: u64, bytes: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(9 + bytes.len());
    f.push(op::OUTPUT);
    f.extend_from_slice(&seq.to_be_bytes());
    f.extend_from_slice(bytes);
    f
}

pub fn encode_reset(seq: u64, screen: &[u8]) -> Vec<u8> {
    let mut f = encode_output(seq, screen);
    f[0] = op::RESET;
    f
}

pub fn encode_exit(code: i32) -> Vec<u8> {
    let mut f = vec![op::EXIT];
    f.extend_from_slice(&code.to_be_bytes());
    f
}

#[derive(Debug, PartialEq)]
pub enum ClientFrame {
    Input(Vec<u8>),
    Resize(Resize),
    Ack(u64),
    Resume(u64),
}

/// Unknown or malformed frames return None and are ignored.
pub fn decode_client(frame: &[u8]) -> Option<ClientFrame> {
    let (&kind, rest) = frame.split_first()?;
    let u64_arg = |r: &[u8]| -> Option<u64> { Some(u64::from_be_bytes(r.get(..8)?.try_into().ok()?)) };
    match kind {
        op::INPUT => Some(ClientFrame::Input(rest.to_vec())),
        op::RESIZE => serde_json::from_slice(rest).ok().map(ClientFrame::Resize),
        op::ACK => u64_arg(rest).map(ClientFrame::Ack),
        op::RESUME => u64_arg(rest).map(ClientFrame::Resume),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_layout() {
        assert_eq!(encode_output(258, b"hi"), vec![1, 0, 0, 0, 0, 0, 0, 1, 2, b'h', b'i']);
    }

    #[test]
    fn exit_is_signed() {
        assert_eq!(encode_exit(-1), vec![2, 0xff, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn decodes_client_frames() {
        assert_eq!(decode_client(&[0x10, b'a']), Some(ClientFrame::Input(b"a".to_vec())));
        assert_eq!(
            decode_client(&[0x12, 0, 0, 0, 0, 0, 0, 0, 7]),
            Some(ClientFrame::Ack(7))
        );
        assert_eq!(
            decode_client(&[0x13, 0, 0, 0, 0, 0, 0, 1, 0]),
            Some(ClientFrame::Resume(256))
        );
        let mut resize = vec![0x11];
        resize.extend_from_slice(br#"{"cols":120,"rows":40}"#);
        assert_eq!(
            decode_client(&resize),
            Some(ClientFrame::Resize(Resize { cols: 120, rows: 40 }))
        );
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(decode_client(&[]), None);
        assert_eq!(decode_client(&[0x12, 1, 2]), None);
        assert_eq!(decode_client(&[0x11, b'{']), None);
        assert_eq!(decode_client(&[0x7f]), None);
    }
}
