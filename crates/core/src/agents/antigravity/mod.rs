//! Antigravity CLI (`agy`): its hook entries and status line, planned as config changes, and its hook events and
//! status line folded into live status.

pub mod hooks;
pub mod status;

use serde_json::Value;

pub use super::common::Gate;

/// The Antigravity versions whose hook events and status line are recorded in
/// `tests/fixtures/antigravity/{hooks,statusline}/<version>/`. One entry per minor line: its oldest recorded patch.
pub const FIXTURE_VERSIONS: &[(u32, u32, u32)] = &[(1, 4, 7)];

/// Known: the same minor line as a recorded fixture set, and not older than its oldest patch.
pub fn version_gate(version: Option<&str>) -> Gate {
    super::common::version_gate("Antigravity", FIXTURE_VERSIONS, version)
}

/// Takes out what the core never keeps: a hook body's `lastUserInput` (the person's prompt) and a status line's
/// `email`. Called on a body as it arrives, before any log, store, fold or hold sees it.
pub fn strip_personal(body: &mut Value) {
    if let Some(object) = body.as_object_mut() {
        object.remove("lastUserInput");
        object.remove("email");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn antigravity_1_4_7_is_known() {
        for v in ["1.4.7", "1.4.9"] {
            assert_eq!(version_gate(Some(v)), Gate::Known, "{v}");
        }
    }

    #[test]
    fn a_later_version_is_not_recorded() {
        assert_eq!(
            version_gate(Some("1.5.0")),
            Gate::Unknown("Antigravity 1.5.0 is not one Marshell has recorded yet".to_string())
        );
    }

    #[test]
    fn an_earlier_version_is_older() {
        assert_eq!(
            version_gate(Some("1.4.6")),
            Gate::Unknown("Antigravity 1.4.6 is older than the versions Marshell knows".to_string())
        );
        assert_eq!(
            version_gate(Some("1.3.9")),
            Gate::Unknown("Antigravity 1.3.9 is older than the versions Marshell knows".to_string())
        );
    }

    #[test]
    fn an_unreadable_version_says_so() {
        assert_eq!(
            version_gate(None),
            Gate::Unknown("Antigravity's version could not be read".to_string())
        );
    }

    #[test]
    fn the_version_is_read_from_the_bare_output() {
        let read = crate::agents::parse_any_version("1.4.7\r\n");
        assert_eq!(version_gate(read.as_deref()), Gate::Known);
    }

    #[test]
    fn strip_personal_removes_the_last_user_input_and_the_email() {
        let mut stop: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/antigravity/hooks/1.4.7/Stop.json"
        ))
        .unwrap();
        assert!(stop.get("lastUserInput").is_some());
        strip_personal(&mut stop);
        assert!(stop.get("lastUserInput").is_none());
        assert!(!stop.to_string().contains("Run the shell command"));
        // Nothing else goes.
        assert_eq!(stop["conversationId"], "00000000-0000-4000-8000-000000000002");
        assert_eq!(stop["terminationReason"], "NO_TOOL_CALL");

        let mut line: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/antigravity/statusline/1.4.7/working.json"
        ))
        .unwrap();
        assert!(line.get("email").is_some());
        strip_personal(&mut line);
        assert!(line.get("email").is_none());
        assert!(!line.to_string().contains("user@example.com"));
        assert_eq!(line["agent_state"], "working");
    }

    #[test]
    fn strip_personal_leaves_other_bodies_alone() {
        for mut body in [
            json!(null),
            json!("x"),
            json!([1]),
            json!({}),
            json!({"a": {"email": "keep"}}),
        ] {
            let before = body.clone();
            strip_personal(&mut body);
            assert_eq!(body, before);
        }
    }
}
