//! A Copilot CLI hooks file the person edited and told Marshell to keep (PLAN §9, slice 6n3, decision 20).
//!
//! Marshell remembers the person's part of the file: everything but the entries Marshell wrote. The record lives in
//! Marshell's own folder, so keeping writes nothing of Copilot's. Nothing here reads or writes a file: the server does,
//! and passes the texts in.

use crate::agents::common::is_ours;
use crate::config::hash::hash_bytes;
use crate::config::jsonc::read_value;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The hooks file without the entries `is_ours` claims, as compact JSON with the keys sorted at every level.
///
/// A file is the same file for this purpose whatever Marshell's entries do in it, so what Marshell's own writes add is
/// left out too: an event's array that holds nothing of the person's, a `hooks` object that ends up empty and a
/// `version` of 1 (a plan gives a file without one `version: 1`).
pub fn persons_part(file: &Value) -> String {
    let mut part = file.clone();
    if let Value::Object(root) = &mut part {
        if root.get("version") == Some(&json!(1)) {
            root.shift_remove("version");
        }
        let emptied = match root.get_mut("hooks") {
            Some(Value::Object(hooks)) => {
                for entries in hooks.values_mut() {
                    if let Value::Array(entries) = entries {
                        entries.retain(|entry| !is_ours(entry));
                    }
                }
                hooks.retain(|_, entries| !entries.as_array().is_some_and(Vec::is_empty));
                hooks.is_empty()
            }
            _ => false,
        };
        if emptied {
            root.shift_remove("hooks");
        }
    }
    sorted(&part).to_string()
}

fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let ordered: BTreeMap<&String, Value> = map.iter().map(|(k, v)| (k, sorted(v))).collect();
            Value::Object(ordered.into_iter().map(|(k, v)| (k.clone(), v)).collect::<Map<_, _>>())
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

/// What the record holds for a file: the hex hash of `persons_part`, hashed as the config service hashes a file.
pub fn mark(file: &Value) -> String {
    hash_bytes(persons_part(file).as_bytes())
}

/// The record's key for a hooks file: the path as displayed, `\` turned into `/`, no case change.
fn key_of(hooks_path: &Path) -> String {
    hooks_path.display().to_string().replace('\\', "/")
}

/// `<marshell home>/agents/copilot/kept-hooks.json`.
pub fn record_path(marshell_home: &Path) -> PathBuf {
    marshell_home.join("agents").join("copilot").join("kept-hooks.json")
}

/// The hooks files the person kept, each with the mark of its person's part.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Kept {
    files: BTreeMap<String, String>,
}

impl Kept {
    /// The record from its text. A missing record, one that doesn't parse and one of another shape are empty.
    pub fn parse(text: Option<&str>) -> Kept {
        let Some(Ok(record)) = text.map(read_value) else {
            return Kept::default();
        };
        if record.get("version") != Some(&json!(1)) {
            return Kept::default();
        }
        let Some(files) = record.get("files").and_then(Value::as_object) else {
            return Kept::default();
        };
        let mut kept = BTreeMap::new();
        for (path, mark) in files {
            let Some(mark) = mark.as_str() else {
                return Kept::default();
            };
            kept.insert(path.clone(), mark.to_string());
        }
        Kept { files: kept }
    }

    /// `{"version": 1, "files": {"<path>": "<mark>"}}`, pretty-printed with a newline at the end, keys sorted.
    pub fn to_text(&self) -> String {
        let files: Map<String, Value> = self.files.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        let mut text = serde_json::to_string_pretty(&json!({ "version": 1, "files": files }))
            .expect("a JSON value always serializes");
        text.push('\n');
        text
    }

    /// The person kept this file, and its person's part is still the one they kept.
    pub fn holds(&self, hooks_path: &Path, file: &Value) -> bool {
        self.files.get(&key_of(hooks_path)).is_some_and(|m| *m == mark(file))
    }

    /// Records the file's person's part as accepted.
    pub fn keep(&mut self, hooks_path: &Path, file: &Value) {
        self.files.insert(key_of(hooks_path), mark(file));
    }

    /// Forgets the file; whether the record held it.
    pub fn drop_path(&mut self, hooks_path: &Path) -> bool {
        self.files.remove(&key_of(hooks_path)).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ours(event: &str, exec: &str) -> Value {
        json!({"type": "command", "exec": exec, "args": ["event", "--agent", "copilot", "--event", event, "--tag", "mshl1"], "timeoutSec": 5})
    }
    fn theirs() -> Value {
        json!({"type": "command", "bash": "echo mine"})
    }
    fn own_file() -> Value {
        json!({"version": 1, "hooks": {
            "sessionStart": [ours("sessionStart", "a.exe")],
            "agentStop": [ours("agentStop", "a.exe")],
        }})
    }
    fn with_theirs() -> Value {
        json!({"version": 1, "hooks": {
            "sessionStart": [theirs(), ours("sessionStart", "a.exe")],
            "agentStop": [ours("agentStop", "a.exe")],
        }})
    }

    #[test]
    fn marshells_own_file_has_no_persons_part() {
        assert_eq!(
            persons_part(&own_file()),
            persons_part(&json!({"version": 1, "hooks": {}}))
        );
    }

    #[test]
    fn persons_part_ignores_what_marshell_does_with_its_entries() {
        let base = persons_part(&with_theirs());
        // Added to, removed from and reordered.
        let added = json!({"version": 1, "hooks": {
            "sessionStart": [theirs(), ours("sessionStart", "a.exe")],
            "agentStop": [ours("agentStop", "a.exe")],
            "sessionEnd": [ours("sessionEnd", "a.exe")],
        }});
        let removed = json!({"version": 1, "hooks": {"sessionStart": [theirs()]}});
        let reordered = json!({"hooks": {
            "agentStop": [ours("agentStop", "b.exe")],
            "sessionStart": [ours("sessionStart", "a.exe"), theirs()],
        }, "version": 1});
        let no_version = json!({"hooks": {"sessionStart": [theirs()]}});
        for file in [added, removed, reordered, no_version] {
            assert_eq!(persons_part(&file), base, "{file}");
        }
        assert_eq!(mark(&with_theirs()), mark(&removed_of(&with_theirs())));
    }

    fn removed_of(file: &Value) -> Value {
        json!({"version": 1, "hooks": {"sessionStart": [file["hooks"]["sessionStart"][0].clone()]}})
    }

    #[test]
    fn persons_part_differs_when_the_persons_part_does() {
        let base = persons_part(&with_theirs());
        let mut changed = with_theirs();
        changed["hooks"]["sessionStart"][0]["bash"] = json!("echo other");
        let mut key = with_theirs();
        key["mine"] = json!(true);
        let mut disabled = with_theirs();
        disabled["disableAllHooks"] = json!(true);
        let mut second = with_theirs();
        second["hooks"]["preToolUse"] = json!([theirs()]);
        for file in [changed, key, disabled, second] {
            assert_ne!(persons_part(&file), base, "{file}");
        }
    }

    #[test]
    fn persons_part_is_compact_and_sorted() {
        let file = json!({"version": 2, "b": {"y": 1, "x": [ {"d": 1, "c": 2} ]}, "a": null});
        assert_eq!(
            persons_part(&file),
            r#"{"a":null,"b":{"x":[{"c":2,"d":1}],"y":1},"version":2}"#
        );
    }

    #[test]
    fn kept_holds_a_file_until_its_persons_part_changes() {
        let path = Path::new("C:\\Users\\Jane Doe\\.copilot\\hooks\\marshell.json");
        let file = with_theirs();
        let mut kept = Kept::default();
        assert!(!kept.holds(path, &file));
        kept.keep(path, &file);
        assert!(kept.holds(path, &file));
        // Marshell's entries come and go.
        assert!(kept.holds(path, &removed_of(&file)));
        // The same path with a slash, the other way round.
        assert!(kept.holds(Path::new("C:/Users/Jane Doe/.copilot/hooks/marshell.json"), &file));
        // Not folded to another case.
        assert!(!kept.holds(Path::new("c:/users/jane doe/.copilot/hooks/marshell.json"), &file));
        let mut changed = file.clone();
        changed["hooks"]["sessionStart"][0]["bash"] = json!("echo other");
        assert!(!kept.holds(path, &changed));
        assert!(kept.drop_path(path));
        assert!(!kept.holds(path, &file));
        assert!(!kept.drop_path(path));
    }

    #[test]
    fn kept_paths_are_independent() {
        let (a, b) = (
            Path::new("C:/a/hooks/marshell.json"),
            Path::new("//wsl$/Ubuntu/home/u/.copilot/hooks/marshell.json"),
        );
        let file = with_theirs();
        let mut kept = Kept::default();
        kept.keep(a, &file);
        kept.keep(b, &file);
        assert!(kept.drop_path(a));
        assert!(!kept.holds(a, &file));
        assert!(kept.holds(b, &file));
    }

    #[test]
    fn the_record_round_trips_as_sorted_pretty_text() {
        let mut kept = Kept::default();
        kept.keep(Path::new("D:/z/marshell.json"), &with_theirs());
        kept.keep(Path::new("C:/a/marshell.json"), &json!({}));
        let text = kept.to_text();
        assert!(text.ends_with("}\n"));
        assert!(
            text.starts_with("{\n  \"version\": 1,\n  \"files\": {\n    \"C:/a/marshell.json\""),
            "{text}"
        );
        assert_eq!(Kept::parse(Some(&text)), kept);
        assert_eq!(Kept::default().to_text(), "{\n  \"version\": 1,\n  \"files\": {}\n}\n");
    }

    #[test]
    fn a_missing_broken_or_odd_record_is_empty() {
        for text in [
            None,
            Some(""),
            Some("{ \"files\": "),
            Some("[1]"),
            Some(r#"{"files": {"C:/a": "x"}}"#),
            Some(r#"{"version": 2, "files": {"C:/a": "x"}}"#),
            Some(r#"{"version": 1, "files": []}"#),
            Some(r#"{"version": 1, "files": {"C:/a": 5}}"#),
        ] {
            assert_eq!(Kept::parse(text), Kept::default(), "{text:?}");
        }
    }

    #[test]
    fn the_record_lives_in_marshells_own_folder() {
        assert_eq!(
            record_path(Path::new("C:/Users/Jane Doe/.marshell")),
            Path::new("C:/Users/Jane Doe/.marshell")
                .join("agents")
                .join("copilot")
                .join("kept-hooks.json")
        );
    }
}
