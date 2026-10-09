//! Opcodes for the binary frames on `/v1/pty/{tab}`. All integers are big-endian.

/// Server -> client: `[OUTPUT][seq u64][bytes]`. `seq` is the byte offset of the first byte.
pub const OUTPUT: u8 = 0x01;
/// Server -> client: `[EXIT][code i32]`. Sent after all output has been sent.
pub const EXIT: u8 = 0x02;
/// Server -> client: `[RESET][seq u64][screen bytes]`. The client clears its terminal, writes the
/// screen, and continues the live stream from `seq`.
pub const RESET: u8 = 0x03;
/// Client -> server: `[INPUT][bytes]`.
pub const INPUT: u8 = 0x10;
/// Client -> server: `[RESIZE][json Resize]`.
pub const RESIZE: u8 = 0x11;
/// Client -> server: `[ACK][processed_up_to u64]`. The renderer has fully processed output before this offset.
pub const ACK: u8 = 0x12;
/// Client -> server: `[RESUME][from u64]`. First frame after connecting; 0 on a fresh terminal.
pub const RESUME: u8 = 0x13;
