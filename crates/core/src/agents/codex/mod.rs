//! Codex CLI's side: its rollout files, read into a common conversation, its hook entries and whether Codex trusts them, and its hook events folded into live status.

pub mod hooks;
pub mod request;
pub mod rollout;
pub mod status;
pub mod trust;

pub use super::common::Gate;

/// The Codex versions whose hook events are recorded in `tests/fixtures/codex/hooks/<version>/`.
/// One entry per minor line: its oldest recorded patch.
pub const FIXTURE_VERSIONS: &[(u32, u32, u32)] = &[(0, 163, 0)];

/// Known: the same minor line as a recorded fixture set, and not older than its oldest patch.
pub fn version_gate(version: Option<&str>) -> Gate {
    super::common::version_gate("Codex", FIXTURE_VERSIONS, version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::parse_any_version;

    #[test]
    fn the_version_is_read_after_codex_cli() {
        let read = parse_any_version("codex-cli 0.163.0\r\n");
        assert_eq!(read.as_deref(), Some("0.163.0"));
        assert_eq!(version_gate(read.as_deref()), Gate::Known);
    }

    #[test]
    fn codex_0_163_is_known() {
        for v in ["0.163.0", "0.163.4"] {
            assert_eq!(version_gate(Some(v)), Gate::Known, "{v}");
        }
    }

    #[test]
    fn codex_0_170_is_not_recorded() {
        assert_eq!(
            version_gate(Some("0.170.0")),
            Gate::Unknown("Codex 0.170.0 is not one Marshell has recorded yet".to_string())
        );
    }

    #[test]
    fn codex_0_162_is_older() {
        assert_eq!(
            version_gate(Some("0.162.9")),
            Gate::Unknown("Codex 0.162.9 is older than the versions Marshell knows".to_string())
        );
    }

    #[test]
    fn a_prerelease_of_a_newer_line_is_not_recorded() {
        assert_eq!(
            version_gate(Some("0.164.0-alpha.1")),
            Gate::Unknown("Codex 0.164.0-alpha.1 is not one Marshell has recorded yet".to_string())
        );
    }

    #[test]
    fn a_prerelease_is_never_known() {
        assert_eq!(
            version_gate(Some("0.163.0-alpha.1")),
            Gate::Unknown("Codex 0.163.0-alpha.1 is older than the versions Marshell knows".to_string())
        );
        assert_eq!(
            version_gate(Some("0.163.4-pre")),
            Gate::Unknown("Codex 0.163.4-pre is not one Marshell has recorded yet".to_string())
        );
        assert_eq!(
            version_gate(Some("0.100.0-beta")),
            Gate::Unknown("Codex 0.100.0-beta is older than the versions Marshell knows".to_string())
        );
    }

    #[test]
    fn build_metadata_is_the_same_release() {
        assert_eq!(version_gate(Some("0.163.0+build")), Gate::Known);
        assert_eq!(version_gate(Some("0.163.2+build.7")), Gate::Known);
    }

    #[test]
    fn an_unreadable_codex_version_says_so() {
        let unread = Gate::Unknown("Codex's version could not be read".to_string());
        assert_eq!(version_gate(None), unread);
        assert_eq!(version_gate(Some("garbage")), unread);
    }
}
