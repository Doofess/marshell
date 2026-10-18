//! Input the terminal sends on its own, not the person: focus changes, mouse movement, and replies
//! to the program's queries. Such input still reaches the program, but it must not count as typing.

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;

/// True when every byte of `input` belongs to a report the terminal sends by itself. A key, a paste,
/// a mouse button press, or a sequence cut off at the end makes it false.
pub fn is_passive_report(input: &[u8]) -> bool {
    let mut rest = input;
    while !rest.is_empty() {
        match passive_len(rest) {
            Some(n) => rest = &rest[n..],
            None => return false,
        }
    }
    true
}

/// The length of the passive report at the start of `input`, or `None` when it starts with anything else.
fn passive_len(input: &[u8]) -> Option<usize> {
    if input.first() != Some(&ESC) {
        return None;
    }
    match input.get(1)? {
        b'[' => csi_len(input),
        b']' => string_len(input, true),
        b'P' => string_len(input, false),
        _ => None,
    }
}

/// OSC and DCS replies run to ST (`ESC \`); an OSC reply may end with BEL instead.
fn string_len(input: &[u8], bel_ends: bool) -> Option<usize> {
    let mut i = 2;
    while i < input.len() {
        match input[i] {
            BEL if bel_ends => return Some(i + 1),
            ESC => return (input.get(i + 1) == Some(&b'\\')).then_some(i + 2),
            _ => i += 1,
        }
    }
    None
}

fn csi_len(input: &[u8]) -> Option<usize> {
    // Legacy mouse: ESC [ M and three bytes, each a value plus 32.
    if input.get(2) == Some(&b'M') {
        let button = input.get(3)?.checked_sub(32)?;
        input.get(5)?;
        return passive_mouse(button, false).then_some(6);
    }
    let mut i = 2;
    while matches!(input.get(i)?, 0x30..=0x3f) {
        i += 1;
    }
    let params = &input[2..i];
    let start = i;
    while matches!(input.get(i)?, 0x20..=0x2f) {
        i += 1;
    }
    let intermediates = &input[start..i];
    let last = *input.get(i)?;
    if !(0x40..=0x7e).contains(&last) {
        return None;
    }
    let prefix = params.first().copied().filter(|b| matches!(b, b'<'..=b'?'));
    let passive = match (prefix, intermediates, last) {
        // Focus in and out.
        (None, [], b'I' | b'O') => params.is_empty(),
        // SGR mouse: a release ends with `m`, a press or motion with `M`.
        (Some(b'<'), [], b'm') => true,
        (Some(b'<'), [], b'M') => first_param(&params[1..]).is_some_and(|b| passive_mouse(b, true)),
        // Cursor position, status, and window reports.
        (None, [], b'R' | b'n' | b't') => true,
        (Some(b'?'), [], b'n' | b'R') => true,
        // Primary and secondary device attributes.
        (Some(b'?' | b'>'), [], b'c') => true,
        // Mode reports.
        (None | Some(b'?'), [b'$'], b'y') => true,
        // Keyboard protocol flags.
        (Some(b'?'), [], b'u') => true,
        _ => false,
    };
    passive.then_some(i + 1)
}

fn first_param(params: &[u8]) -> Option<u8> {
    let digits = params.split(|&b| b == b';').next()?;
    std::str::from_utf8(digits).ok()?.parse().ok()
}

/// Motion (bit 32) and the wheel (64 and up) are passive. In the legacy encoding, a release is
/// button 3, since it has no release form of its own.
fn passive_mouse(button: u8, sgr: bool) -> bool {
    button & 32 != 0 || button >= 64 || (!sgr && button & 3 == 3)
}

#[cfg(test)]
mod tests {
    use super::is_passive_report;

    #[test]
    fn focus_reports_are_passive() {
        assert!(is_passive_report(b"\x1b[I"));
        assert!(is_passive_report(b"\x1b[O"));
        assert!(is_passive_report(b"\x1b[O\x1b[I"));
    }

    #[test]
    fn mouse_motion_wheel_and_release_are_passive() {
        assert!(is_passive_report(b"\x1b[<35;10;5M"), "motion, no button");
        assert!(is_passive_report(b"\x1b[<32;10;5M"), "drag with the left button");
        assert!(is_passive_report(b"\x1b[<64;10;5M"), "wheel up");
        assert!(is_passive_report(b"\x1b[<65;10;5M"), "wheel down");
        assert!(is_passive_report(b"\x1b[<0;10;5m"), "left button released");
        assert!(is_passive_report(b"\x1b[M#*%"), "legacy encoding, release");
        assert!(is_passive_report(b"\x1b[MC*%"), "legacy encoding, motion");
        assert!(is_passive_report(b"\x1b[<35;10;5M\x1b[<35;11;5M\x1b[<35;12;5M"));
    }

    #[test]
    fn mouse_button_presses_are_the_person() {
        assert!(!is_passive_report(b"\x1b[<0;10;5M"), "left press");
        assert!(!is_passive_report(b"\x1b[<2;10;5M"), "right press");
        assert!(!is_passive_report(b"\x1b[<16;10;5M"), "ctrl + left press");
        assert!(!is_passive_report(b"\x1b[M *%"), "legacy encoding, left press");
        assert!(
            !is_passive_report(b"\x1b[<35;10;5M\x1b[<0;12;5M"),
            "motion, then a press"
        );
    }

    #[test]
    fn replies_to_queries_are_passive() {
        assert!(is_passive_report(b"\x1b[12;40R"), "cursor position");
        assert!(is_passive_report(b"\x1b[?12;40R"), "private cursor position");
        assert!(is_passive_report(b"\x1b[?1;2c"), "primary device attributes");
        assert!(is_passive_report(b"\x1b[>0;276;0c"), "secondary device attributes");
        assert!(is_passive_report(b"\x1b[0n"), "status report");
        assert!(is_passive_report(b"\x1b[?2004;1$y"), "mode report");
        assert!(is_passive_report(b"\x1b[8;30;120t"), "window size");
        assert!(is_passive_report(b"\x1b[?0u"), "keyboard protocol flags");
        assert!(is_passive_report(b"\x1b]11;rgb:1e1e/1e1e/1e1e\x07"), "colour, BEL");
        assert!(is_passive_report(b"\x1b]10;rgb:ffff/ffff/ffff\x1b\\"), "colour, ST");
        assert!(is_passive_report(b"\x1bP1$r0m\x1b\\"), "setting report");
        assert!(is_passive_report(b""));
    }

    #[test]
    fn keys_and_pastes_are_the_person() {
        for key in [
            &b"4"[..],
            b"\r",
            b"\x1b",
            b"\x7f",
            b"\x1b[A",
            b"\x1b[1;5C",
            b"\x1b[15~",
            b"\x1b[97;5u",
            b"\x1ba",
            b"\x1bOP",
            b"\x1b[200~text\x1b[201~",
        ] {
            assert!(!is_passive_report(key), "{key:?}");
        }
    }

    #[test]
    fn a_report_followed_by_a_key_is_the_person() {
        assert!(!is_passive_report(b"\x1b[Iy"));
        assert!(!is_passive_report(b"\x1b[12;40R\r"));
    }

    #[test]
    fn cut_off_sequences_are_the_person() {
        assert!(!is_passive_report(b"\x1b["));
        assert!(!is_passive_report(b"\x1b[<35;10"));
        assert!(!is_passive_report(b"\x1b[M "));
        assert!(!is_passive_report(b"\x1b]11;rgb:1e1e"));
        assert!(!is_passive_report(b"\x1bP1$r0m"));
    }
}
