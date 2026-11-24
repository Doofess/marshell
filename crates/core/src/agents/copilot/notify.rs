//! What Copilot CLI notifies by itself, read from its settings and hooks files: its own `notifications` setting and the
//! person's own `notification` hooks. Nothing here touches a file.

use crate::agents::common::is_ours;
use serde_json::Value;
use std::path::PathBuf;

/// Copilot's own `notifications` setting is a literal `true`.
pub fn toasts_on(settings: &Value) -> bool {
    settings.get("notifications") == Some(&Value::Bool(true))
}

/// The variable that turns Copilot's own toasts off, whatever `settings.json` says, when it is exactly `1`.
pub const TOASTS_OFF_ENV: &str = "COPILOT_DISABLE_DESKTOP_NOTIFICATIONS";

/// `value` is that variable in the environment Copilot starts in.
pub fn toasts_off_by_env(value: Option<&std::ffi::OsStr>) -> bool {
    value == Some(std::ffi::OsStr::new("1"))
}

/// The command a hook entry runs, as text: the first of `powershell`, `command` and `bash`, else `exec` with its `args`
/// joined by spaces, else the `url` of an `http` entry. A `prompt` entry, and one with none of these, gives `None`.
fn command_of(entry: &Value) -> Option<String> {
    let text = |key: &str| {
        entry
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let kind = entry.get("type").and_then(Value::as_str);
    if kind == Some("prompt") {
        return None;
    }
    if let Some(c) = text("powershell").or_else(|| text("command")).or_else(|| text("bash")) {
        return Some(c.to_string());
    }
    if let Some(exec) = text("exec") {
        let mut parts = vec![exec.to_string()];
        parts.extend(
            entry
                .get("args")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string),
        );
        return Some(parts.join(" "));
    }
    if kind == Some("http") {
        return text("url").map(str::to_string);
    }
    None
}

/// The commands of the entries under `hooks.notification` of one file value that aren't Marshell's.
fn commands_in(value: &Value, out: &mut Vec<String>) {
    if value.get("disableAllHooks") == Some(&Value::Bool(true)) {
        return;
    }
    let Some(entries) = value.pointer("/hooks/notification").and_then(Value::as_array) else {
        return;
    };
    for entry in entries {
        // A Claude-style group holds its entries under `hooks`.
        let inner: Vec<&Value> = match entry.get("hooks").and_then(Value::as_array) {
            Some(group) => group.iter().collect(),
            None => vec![entry],
        };
        out.extend(inner.into_iter().filter(|e| !is_ours(e)).filter_map(command_of));
    }
}

/// The commands of the person's own `notification` hooks: the files in file-name order, each in file order, then
/// `settings.hooks.notification`. Marshell's own entries are never listed. `disableAllHooks` in a hooks file turns off
/// that file's hooks, and in `settings.json` every file's.
pub fn own_hooks(files: &[(PathBuf, Value)], settings: Option<&Value>) -> Vec<String> {
    if settings.is_some_and(|s| s.get("disableAllHooks") == Some(&Value::Bool(true))) {
        return Vec::new();
    }
    let mut sorted: Vec<&(PathBuf, Value)> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.file_name().cmp(&b.0.file_name()));
    let mut out = Vec::new();
    for (_, value) in sorted {
        commands_in(value, &mut out);
    }
    if let Some(settings) = settings {
        commands_in(settings, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn file(name: &str, value: Value) -> (PathBuf, Value) {
        (PathBuf::from(format!("C:/h/hooks/{name}")), value)
    }

    #[test]
    fn toasts_are_on_only_for_a_literal_true() {
        assert!(toasts_on(&json!({"notifications": true})));
        for v in [
            json!({"notifications": "true"}),
            json!({"notifications": 1}),
            json!({"notifications": false}),
            json!({"notifications": null}),
            json!({}),
            json!([]),
        ] {
            assert!(!toasts_on(&v), "{v}");
        }
    }

    #[test]
    fn own_hooks_come_in_file_name_order_then_file_order() {
        let b = file(
            "b.json",
            json!({"hooks": {"notification": [{"type": "command", "command": "b1"}, {"type": "command", "bash": "b2"}]}}),
        );
        let a = file(
            "a.json",
            json!({"hooks": {"notification": [{"type": "command", "powershell": "a1", "command": "ignored"}]}}),
        );
        let settings = json!({"hooks": {"notification": [{"type": "command", "command": "s1"}]}});
        assert_eq!(own_hooks(&[b, a], Some(&settings)), ["a1", "b1", "b2", "s1"]);
    }

    #[test]
    fn a_group_and_a_flat_entry_both_count() {
        let f = file(
            "x.json",
            json!({"hooks": {"notification": [
                {"matcher": "*", "hooks": [{"type": "command", "command": "g1"}, {"type": "command", "command": "g2"}]},
                {"type": "command", "command": "flat"}
            ]}}),
        );
        assert_eq!(own_hooks(&[f], None), ["g1", "g2", "flat"]);
    }

    #[test]
    fn exec_with_args_and_http_urls_are_shown() {
        let f = file(
            "x.json",
            json!({"hooks": {"notification": [
                {"type": "command", "exec": "toast.exe", "args": ["a", "b", 3]},
                {"type": "command", "exec": "bare.exe"},
                {"type": "http", "url": "http://localhost:1/x"}
            ]}}),
        );
        assert_eq!(
            own_hooks(&[f], None),
            ["toast.exe a b", "bare.exe", "http://localhost:1/x"]
        );
    }

    #[test]
    fn prompt_entries_and_entries_with_no_command_are_skipped() {
        let f = file(
            "x.json",
            json!({"hooks": {"notification": [
                {"type": "prompt", "prompt": "hello", "command": "no"},
                {"type": "command"},
                {"type": "command", "command": "  "},
                {"type": "http"},
                "text",
                null
            ]}}),
        );
        assert!(own_hooks(&[f], None).is_empty());
    }

    #[test]
    fn marshells_own_entries_are_never_listed() {
        let ours = json!({"type": "command", "exec": "C:/m/bridge.exe",
            "args": ["event", "--agent", "copilot", "--event", "notification", "--tag", crate::agents::common::TAG]});
        let f = file(
            "marshell.json",
            json!({"version": 1, "hooks": {"notification": [ours, {"type": "command", "command": "mine"}]}}),
        );
        assert_eq!(own_hooks(&[f], None), ["mine"]);
    }

    #[test]
    fn disable_all_hooks_gives_none_and_other_events_are_ignored() {
        let off = file(
            "x.json",
            json!({"disableAllHooks": true, "hooks": {"notification": [{"type": "command", "command": "a"}]}}),
        );
        let other = file(
            "y.json",
            json!({"hooks": {"agentStop": [{"type": "command", "command": "b"}]}}),
        );
        let settings = json!({"disableAllHooks": true, "hooks": {"notification": [{"command": "c"}]}});
        assert!(own_hooks(&[off, other], Some(&settings)).is_empty());
    }

    #[test]
    fn disable_all_hooks_in_settings_turns_off_the_hooks_of_every_file() {
        let hook = json!({"hooks": {"notification": [{"type": "command", "command": "a"}]}});
        let off = json!({"disableAllHooks": true});
        assert!(own_hooks(&[file("x.json", hook.clone())], Some(&off)).is_empty());
        // Only a literal `true` is the switch.
        let text = json!({"disableAllHooks": "true"});
        assert_eq!(own_hooks(&[file("x.json", hook)], Some(&text)), ["a"]);
    }

    #[test]
    fn only_the_variable_set_to_exactly_1_turns_the_toasts_off() {
        use std::ffi::OsStr;
        assert!(toasts_off_by_env(Some(OsStr::new("1"))));
        for other in ["", "0", "true", " 1", "11"] {
            assert!(!toasts_off_by_env(Some(OsStr::new(other))), "{other:?}");
        }
        assert!(!toasts_off_by_env(None));
    }
}
