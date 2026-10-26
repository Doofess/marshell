//! Which Antigravity tool requests are held for a card. Everything else passes at once.

use marshell_protocol::api::SessionMode;

/// Whether a request waits for a card. Only a risky one in Bypass does: under `--dangerously-skip-permissions`
/// agy asks nothing itself, so the card is the only chance to stop it. In every other mode agy's own prompt
/// decides, in its terminal, and Marshell passes the call (204: the bridge prints nothing).
pub fn holds(mode: SessionMode, risky: bool) -> bool {
    mode == SessionMode::Bypass && risky
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_risky_command_is_held_in_bypass() {
        assert!(holds(SessionMode::Bypass, true));
    }

    #[test]
    fn a_safe_command_passes_in_bypass() {
        assert!(!holds(SessionMode::Bypass, false));
    }

    #[test]
    fn nothing_is_held_outside_bypass() {
        for mode in [
            SessionMode::Manual,
            SessionMode::Plan,
            SessionMode::AutoEdit,
            SessionMode::FullAuto,
            SessionMode::DontAsk,
        ] {
            for risky in [true, false] {
                assert!(!holds(mode, risky), "{mode:?} risky={risky}");
            }
        }
    }
}
