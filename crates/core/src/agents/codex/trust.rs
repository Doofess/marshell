//! Whether Codex trusts the hook entries Marshell added. Codex records its trust decisions in
//! `config.toml`, under `[hooks.state]`, as a hash of each hook's identity. This module only reads:
//! it recomputes those hashes the way Codex does and compares them. Marshell never writes the file.

use crate::agents::common::is_ours;
use marshell_protocol::agents::HookTrust;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;

/// What Codex assumes when an entry sets no `timeout`.
const DEFAULT_TIMEOUT: u64 = 600;
/// `SessionEnd` and `Interrupt` are capped at this.
const SHORT_TIMEOUT_CAP: u64 = 3;

/// `PreToolUse` as `pre_tool_use`, the form Codex puts in its identity JSON and in its keys.
pub fn snake_case(event: &str) -> String {
    let mut out = String::with_capacity(event.len() + 4);
    for (i, c) in event.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The hash Codex records as `trusted_hash`: `sha256:` and the lowercase hex of SHA-256 over the
/// identity JSON of one hook. Source: openai/codex `rust-v0.163.0`, `hooks/src/engine/discovery.rs`
/// (`hook_hash`) and `config/src/fingerprint.rs`. The JSON is compact with sorted keys and
/// non-ASCII text unescaped. Each object is built in sorted key order, so the bytes do not depend
/// on serde_json's `preserve_order`.
pub fn hook_hash(event: &str, group: &Value, entry: &Value) -> String {
    let short = matches!(event, "SessionEnd" | "Interrupt");
    let timeout = entry.get("timeout").and_then(Value::as_u64).unwrap_or(DEFAULT_TIMEOUT);
    let timeout = if short { timeout.min(SHORT_TIMEOUT_CAP) } else { timeout }.max(1);

    let mut hook = Map::new();
    hook.insert("async".into(), json!(false));
    hook.insert(
        "command".into(),
        json!(entry.get("command").and_then(Value::as_str).unwrap_or("")),
    );
    if let Some(message) = entry.get("statusMessage").and_then(Value::as_str) {
        hook.insert("statusMessage".into(), json!(message));
    }
    hook.insert("timeout".into(), json!(timeout));
    hook.insert("type".into(), json!("command"));

    let mut identity = Map::new();
    identity.insert("event_name".into(), json!(snake_case(event)));
    identity.insert("hooks".into(), json!([Value::Object(hook)]));
    // These three events have no matcher.
    if !matches!(event, "UserPromptSubmit" | "Stop" | "Interrupt")
        && let Some(matcher) = group.get("matcher").and_then(Value::as_str)
    {
        identity.insert("matcher".into(), json!(matcher));
    }
    let text = Value::Object(identity).to_string();
    format!("sha256:{}", hex::encode(Sha256::digest(text.as_bytes())))
}

/// Marshell's entries as (event, group index, group, handler index, entry). Codex keys its trust by
/// the two indexes, so a user's group ahead of ours counts. Anything of an unexpected shape is skipped.
fn ours_with_indexes(root: &Value) -> Vec<(&str, usize, &Value, usize, &Value)> {
    let mut out = Vec::new();
    let events = root.get("hooks").and_then(Value::as_object).into_iter().flatten();
    for (event, groups) in events {
        for (gi, group) in groups.as_array().into_iter().flatten().enumerate() {
            let entries = group.get("hooks").and_then(Value::as_array).into_iter().flatten();
            for (hi, entry) in entries.enumerate() {
                if is_ours(entry) {
                    out.push((event.as_str(), gi, group, hi, entry));
                }
            }
        }
    }
    out
}

/// A path as compared between Codex's key and ours: ASCII case and `/` against `\` do not count.
fn normalized(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

/// A `[hooks.state]` key as (path, event, group index, handler index). The path holds `:` itself
/// (`C:`), so the key is split from the right.
fn split_key(key: &str) -> Option<(&str, &str, usize, usize)> {
    let mut parts = key.rsplitn(4, ':');
    let handler = parts.next()?.parse().ok()?;
    let group = parts.next()?.parse().ok()?;
    let event = parts.next()?;
    Some((parts.next()?, event, group, handler))
}

/// What `config.toml` says about Marshell's entries in the hooks file at `hooks_path`. `None` when
/// `hooks` holds no entry of ours or `config_toml` does not parse. A missing `config.toml` is
/// `Untrusted`. Every entry of ours needs a recorded hash that matches and is not disabled: any
/// entry whose recorded hash differs makes the answer `Changed`, else any entry without a hash, a
/// record, or with `enabled = false` makes it `Untrusted`.
pub fn state(hooks_path: &Path, hooks: &Value, config_toml: Option<&str>) -> Option<HookTrust> {
    let ours = ours_with_indexes(hooks);
    if ours.is_empty() {
        return None;
    }
    let config = match config_toml {
        Some(text) => Some(text.parse::<toml::Table>().ok()?),
        None => None,
    };
    let recorded = config
        .as_ref()
        .and_then(|c| c.get("hooks"))
        .and_then(|h| h.get("state"))
        .and_then(toml::Value::as_table);
    let path = normalized(&hooks_path.to_string_lossy());

    let (mut changed, mut untrusted) = (false, false);
    for (event, gi, group, hi, entry) in ours {
        let snake = snake_case(event);
        let row = recorded.into_iter().flatten().find_map(|(key, row)| {
            let (key_path, key_event, key_gi, key_hi) = split_key(key)?;
            (normalized(key_path) == path && key_event == snake && key_gi == gi && key_hi == hi).then_some(row)
        });
        let Some(hash) = row.and_then(|r| r.get("trusted_hash")).and_then(toml::Value::as_str) else {
            untrusted = true;
            continue;
        };
        if hash != hook_hash(event, group, entry) {
            changed = true;
        } else if row.and_then(|r| r.get("enabled")).and_then(toml::Value::as_bool) == Some(false) {
            untrusted = true;
        }
    }
    Some(match (changed, untrusted) {
        (true, _) => HookTrust::Changed,
        (false, true) => HookTrust::Untrusted,
        (false, false) => HookTrust::Trusted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::agents::HookTrust;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::path::Path;

    const CMD: &str = r#"& "C:/Users/someone/.marshell/bin/marshell-hook.exe" event --agent codex --tag mshl1"#;
    const HOOKS_PATH: &str = r"C:\Users\x\.codex\hooks.json";

    fn sha(identity: &str) -> String {
        format!("sha256:{}", hex::encode(Sha256::digest(identity.as_bytes())))
    }
    fn ours(timeout: u64) -> Value {
        json!({ "type": "command", "command": CMD, "timeout": timeout })
    }
    fn users() -> Value {
        json!({ "hooks": [{ "type": "command", "command": "echo mine" }] })
    }
    /// Marshell's `Stop` (5 s) and `SessionEnd` (2 s) entries, after whatever groups come first.
    fn hooks_after(stop_before: Vec<Value>) -> Value {
        let mut stop = stop_before;
        stop.push(json!({ "hooks": [ours(5)] }));
        json!({ "hooks": { "Stop": stop, "SessionEnd": [{ "hooks": [ours(2)] }] } })
    }
    fn hooks() -> Value {
        hooks_after(Vec::new())
    }
    fn good_hash(event: &str, timeout: u64) -> String {
        hook_hash(event, &json!({}), &ours(timeout))
    }
    /// A `[hooks.state]` table; a row is (key, trusted_hash, enabled).
    fn config(rows: &[(String, Option<String>, Option<bool>)]) -> String {
        let mut out = String::from("model = \"x\"\n\n");
        for (key, hash, enabled) in rows {
            out.push_str(&format!("[hooks.state.'{key}']\n"));
            if let Some(hash) = hash {
                out.push_str(&format!("trusted_hash = \"{hash}\"\n"));
            }
            if let Some(enabled) = enabled {
                out.push_str(&format!("enabled = {enabled}\n"));
            }
            out.push('\n');
        }
        out
    }
    fn trusted_rows() -> Vec<(String, Option<String>, Option<bool>)> {
        vec![
            (format!("{HOOKS_PATH}:stop:0:0"), Some(good_hash("Stop", 5)), None),
            (
                format!("{HOOKS_PATH}:session_end:0:0"),
                Some(good_hash("SessionEnd", 2)),
                None,
            ),
        ]
    }
    fn state_of(hooks: &Value, config: Option<&str>) -> Option<HookTrust> {
        state(Path::new(HOOKS_PATH), hooks, config)
    }

    // The vectors are confirmed against real Codex 0.163 in `docs/research/2026-10-24-codex-hooks.md`.
    #[test]
    fn hash_matches_codex_for_the_recorded_vectors() {
        let entry = |t: u64| json!({ "type": "command", "command": CMD, "timeout": t });
        assert_eq!(
            hook_hash("Stop", &json!({}), &entry(5)),
            "sha256:eae810b59ee8e55e5a7a3169f12f3216e3be79d801f5259f1259fec5c558ab56"
        );
        assert_eq!(
            hook_hash("SessionEnd", &json!({}), &entry(2)),
            "sha256:6da5391ccb8c4e99a5fb910f7bc2b0dec6a6b0c9ac476f61d69c1cdb0ae97131"
        );
    }

    #[test]
    fn an_entry_without_a_timeout_hashes_with_600() {
        let entry = json!({ "type": "command", "command": CMD });
        let identity = format!(
            r#"{{"event_name":"pre_tool_use","hooks":[{{"async":false,"command":{},"timeout":600,"type":"command"}}]}}"#,
            json!(CMD)
        );
        assert_eq!(hook_hash("PreToolUse", &json!({}), &entry), sha(&identity));
    }

    #[test]
    fn a_matcher_counts_for_permission_requests_but_not_for_stop() {
        let group = json!({ "matcher": "Bash" });
        let hook = format!(
            r#"[{{"async":false,"command":{},"timeout":600,"type":"command"}}]"#,
            json!(CMD)
        );
        let with = format!(r#"{{"event_name":"permission_request","hooks":{hook},"matcher":"Bash"}}"#);
        assert_eq!(hook_hash("PermissionRequest", &group, &ours(600)), sha(&with));
        let without = format!(r#"{{"event_name":"stop","hooks":{hook}}}"#);
        assert_eq!(
            hook_hash("Stop", &group, &json!({ "type": "command", "command": CMD })),
            sha(&without)
        );
    }

    #[test]
    fn a_missing_config_is_untrusted() {
        assert_eq!(state_of(&hooks(), None), Some(HookTrust::Untrusted));
        assert_eq!(state_of(&hooks(), Some("[hooks.state]\n")), Some(HookTrust::Untrusted));
    }

    #[test]
    fn every_entry_trusted_is_trusted() {
        assert_eq!(
            state_of(&hooks(), Some(&config(&trusted_rows()))),
            Some(HookTrust::Trusted)
        );
    }

    #[test]
    fn the_key_path_ignores_case_and_slash_direction() {
        let rows: Vec<_> = trusted_rows()
            .into_iter()
            .map(|(k, h, e)| (k.replace('\\', "/").replace("C:", "c:"), h, e))
            .collect();
        assert_eq!(state_of(&hooks(), Some(&config(&rows))), Some(HookTrust::Trusted));
    }

    #[test]
    fn one_altered_hash_is_changed() {
        let mut rows = trusted_rows();
        rows[1].1 = Some(good_hash("SessionEnd", 3));
        assert_eq!(state_of(&hooks(), Some(&config(&rows))), Some(HookTrust::Changed));
        // Changed outranks untrusted.
        rows.remove(0);
        assert_eq!(state_of(&hooks(), Some(&config(&rows))), Some(HookTrust::Changed));
    }

    #[test]
    fn an_entry_without_state_is_untrusted() {
        let mut rows = trusted_rows();
        rows.remove(1);
        assert_eq!(state_of(&hooks(), Some(&config(&rows))), Some(HookTrust::Untrusted));
        rows[0].1 = None;
        assert_eq!(state_of(&hooks(), Some(&config(&rows))), Some(HookTrust::Untrusted));
    }

    #[test]
    fn a_disabled_entry_is_untrusted() {
        let mut rows = trusted_rows();
        rows[0].2 = Some(false);
        assert_eq!(state_of(&hooks(), Some(&config(&rows))), Some(HookTrust::Untrusted));
        rows[0].2 = Some(true);
        assert_eq!(state_of(&hooks(), Some(&config(&rows))), Some(HookTrust::Trusted));
    }

    #[test]
    fn the_key_counts_the_users_groups_first() {
        let hooks = hooks_after(vec![users()]);
        // Keyed as if Marshell's `Stop` group were first: nothing is recorded for `:1:0`.
        assert_eq!(
            state_of(&hooks, Some(&config(&trusted_rows()))),
            Some(HookTrust::Untrusted)
        );
        let mut rows = trusted_rows();
        rows[0].0 = format!("{HOOKS_PATH}:stop:1:0");
        assert_eq!(state_of(&hooks, Some(&config(&rows))), Some(HookTrust::Trusted));
    }

    #[test]
    fn a_placeholder_counts_as_a_group() {
        // Marshell's `Stop` entry sits after an empty group, so Codex keys it as `stop:1:0`.
        let hooks = hooks_after(vec![json!({ "hooks": [] })]);
        assert_eq!(
            state_of(&hooks, Some(&config(&trusted_rows()))),
            Some(HookTrust::Untrusted)
        );
        let mut rows = trusted_rows();
        rows[0].0 = format!("{HOOKS_PATH}:stop:1:0");
        assert_eq!(state_of(&hooks, Some(&config(&rows))), Some(HookTrust::Trusted));
        // An empty group is not Marshell's.
        let only_placeholder = json!({ "hooks": { "Stop": [{ "hooks": [] }] } });
        assert_eq!(state_of(&only_placeholder, Some(&config(&rows))), None);
    }

    #[test]
    fn a_config_that_does_not_parse_gives_no_answer() {
        assert_eq!(state_of(&hooks(), Some("[hooks.state\nnot toml")), None);
    }

    #[test]
    fn no_entries_of_ours_gives_no_answer() {
        let only_users = json!({ "hooks": { "Stop": [users()] } });
        assert_eq!(state_of(&only_users, Some(&config(&trusted_rows()))), None);
        assert_eq!(state_of(&json!({}), None), None);
    }
}
