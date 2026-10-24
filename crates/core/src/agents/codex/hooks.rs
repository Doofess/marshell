//! Codex CLI's hook entries, planned as config changes. Nothing here touches a file: a plan is built
//! from the text of `hooks.json`, and the config service previews, snapshots and writes it. Every
//! doubt fails closed: a file that does not parse, or a `hooks` value of the wrong shape, gives an
//! error and no edits.

use crate::agents::common::{PlanError, TAG, is_ours};
use crate::config::hash::hash_bytes;
use crate::config::jsonc::{parse_problem, read_value};
use crate::paths;
use marshell_protocol::agents::{HookInstall, HookPlanRequest};
use marshell_protocol::config::{Applies, ChangeSet, ConfigScope, EditOp, FileEdit};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// The events whose hooks carry live status, in the order they are installed.
pub const STATUS_EVENTS: [&str; 7] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Stop",
    "Interrupt",
    "SessionEnd",
];

const AGENT: &str = "codex";
const HOOK_TIMEOUT: u64 = 5;
/// `SessionEnd` and `Interrupt` may take 3 s at most.
const SHORT_TIMEOUT: u64 = 2;
const PERMISSION_EVENT: &str = "PermissionRequest";
/// The bridge waits 590 s for the person's answer.
const PERMISSION_TIMEOUT: u64 = 600;

/// `$CODEX_HOME/hooks.json` when `CODEX_HOME` is set and not empty, else `<user home>/.codex/hooks.json`.
pub fn hooks_path(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> PathBuf {
    match env("CODEX_HOME") {
        Some(dir) if !dir.trim().is_empty() => PathBuf::from(dir).join("hooks.json"),
        _ => user_home.join(".codex").join("hooks.json"),
    }
}

fn bridge_path(home: &Path) -> String {
    paths::bridge_exe(home).to_string_lossy().replace('\\', "/")
}

/// The command string of a hook entry: the quoted bridge path with forward slashes. Codex runs it
/// through PowerShell on Windows, where a quoted path needs the `&` call operator.
pub fn hook_command(home: &Path, role: &str) -> String {
    let call = if cfg!(windows) { "& " } else { "" };
    format!("{call}\"{}\" {role} --agent {AGENT} --tag {TAG}", bridge_path(home))
}

fn hook_entry(home: &Path, event: &str) -> Value {
    let (role, timeout) = match event {
        PERMISSION_EVENT => ("permission", PERMISSION_TIMEOUT),
        "SessionEnd" | "Interrupt" => ("event", SHORT_TIMEOUT),
        _ => ("event", HOOK_TIMEOUT),
    };
    json!({ "type": "command", "command": hook_command(home, role), "timeout": timeout })
}

/// An RFC 6901 token.
fn esc(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn not_an_object(path: &Path, pointer: impl Into<String>) -> PlanError {
    PlanError::NotAnObject {
        path: path.to_path_buf(),
        pointer: pointer.into(),
    }
}

/// The file as a value (a missing file is `{}`), checked for the shapes a plan relies on.
fn parse_hooks(path: &Path, text: Option<&str>) -> Result<Value, PlanError> {
    let Some(text) = text else { return Ok(json!({})) };
    let Ok(root) = read_value(text) else {
        let problem = parse_problem(text).unwrap_or((1, "The file does not parse.".to_string()));
        return Err(PlanError::DoesNotParse {
            path: path.to_path_buf(),
            problem,
        });
    };
    let Some(map) = root.as_object() else {
        return Err(not_an_object(path, ""));
    };
    if let Some(hooks) = map.get("hooks") {
        let Some(hooks) = hooks.as_object() else {
            return Err(not_an_object(path, "/hooks"));
        };
        for (event, groups) in hooks {
            let Some(groups) = groups.as_array() else {
                return Err(not_an_object(path, format!("/hooks/{}", esc(event))));
            };
            for (i, group) in groups.iter().enumerate() {
                if group.get("hooks").is_some_and(|h| !h.is_array()) {
                    return Err(not_an_object(path, format!("/hooks/{}/{i}/hooks", esc(event))));
                }
            }
        }
    }
    Ok(root)
}

/// Every hook entry as (event, entry); anything of an unexpected shape is skipped.
fn entries_of(root: &Value) -> impl Iterator<Item = (&str, &Value)> {
    root.get("hooks")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(event, groups)| groups.as_array().map(|g| (event.as_str(), g)))
        .flat_map(|(event, groups)| {
            groups
                .iter()
                .filter_map(|g| g.get("hooks").and_then(Value::as_array))
                .flatten()
                .map(move |entry| (event, entry))
        })
}

fn ours_of(root: &Value) -> Vec<(&str, &Value)> {
    entries_of(root).filter(|(_, e)| is_ours(e)).collect()
}

/// The pointers that take Marshell's entries out, highest index first so none shifts another.
/// A group holding nothing else goes whole. With `prune`, an event left without groups goes, and
/// `/hooks` itself when every event in it went.
fn removal_pointers(root: &Value, prune: bool) -> Vec<String> {
    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut events_gone = 0;
    for (event, groups) in hooks {
        let Some(groups) = groups.as_array() else { continue };
        let event_pointer = format!("/hooks/{}", esc(event));
        let mut ops = Vec::new();
        let mut groups_gone = 0;
        let mut found = false;
        for (gi, group) in groups.iter().enumerate().rev() {
            let Some(entries) = group.get("hooks").and_then(Value::as_array) else {
                continue;
            };
            let ours: Vec<usize> = (0..entries.len()).filter(|&i| is_ours(&entries[i])).collect();
            if ours.is_empty() {
                continue;
            }
            found = true;
            if ours.len() == entries.len() && group.as_object().is_some_and(|o| o.len() == 1) {
                ops.push(format!("{event_pointer}/{gi}"));
                groups_gone += 1;
            } else {
                ops.extend(ours.iter().rev().map(|ei| format!("{event_pointer}/{gi}/hooks/{ei}")));
            }
        }
        if !found {
            continue;
        }
        if prune && groups_gone == groups.len() {
            out.push(event_pointer);
            events_gone += 1;
        } else {
            out.extend(ops);
        }
    }
    if prune && events_gone > 0 && events_gone == hooks.len() {
        return vec!["/hooks".to_string()];
    }
    out
}

// ---- inspect ----

pub struct Found {
    pub install: HookInstall,
    /// The PermissionRequest hook is in place.
    pub approve_installed: bool,
}

/// What the file (as a value) says about the install.
pub fn inspect(hooks: &Value, home: &Path) -> Found {
    let ours = ours_of(hooks);
    let bridge_exists = paths::bridge_exe(home).exists();
    let quoted = format!("\"{}\"", bridge_path(home));
    let exact_one = |event: &str| {
        let mut it = ours.iter().filter(|(e, _)| *e == event);
        matches!((it.next(), it.next()), (Some((_, entry)), None) if **entry == hook_entry(home, event))
    };
    let exact_events = STATUS_EVENTS.iter().filter(|e| exact_one(e)).count();
    let permission_exact = exact_one(PERMISSION_EVENT);

    let install = if ours.is_empty() {
        HookInstall::None
    } else if !bridge_exists
        || ours.iter().any(|(_, e)| {
            !e.get("command")
                .and_then(Value::as_str)
                .is_some_and(|c| c.contains(&quoted))
        })
    {
        HookInstall::Orphaned
    } else if exact_events == STATUS_EVENTS.len() && ours.len() == exact_events + usize::from(permission_exact) {
        HookInstall::Installed
    } else {
        HookInstall::Partial
    };
    Found {
        install,
        approve_installed: permission_exact && bridge_exists,
    }
}

// ---- plans ----

fn edit(path: &Path, op: EditOp, text: Option<&str>) -> FileEdit {
    FileEdit {
        path: path.to_string_lossy().into_owned(),
        scope: ConfigScope::User,
        op,
        expected_hash: text.map(|t| hash_bytes(t.as_bytes())),
        applies: Applies::NewSessions,
    }
}

fn set(pointer: impl Into<String>, value: &Value) -> EditOp {
    EditOp::JsonSet {
        pointer: pointer.into(),
        value: value.to_string(),
    }
}

fn remove_op(pointer: String) -> EditOp {
    EditOp::JsonRemove { pointer }
}

/// The plan for status hooks and optional approval hooks. Status off leaves status alone, and
/// `want.statusline` is ignored, because Codex has no status line.
///
/// Marshell's existing entries come out first (so a rerun never duplicates), then one group per
/// event goes in at the end, after the user's own. A file that already holds exactly what is asked
/// for gets no edits.
pub fn install_plan(
    path: &Path,
    text: Option<&str>,
    home: &Path,
    want: &HookPlanRequest,
) -> Result<ChangeSet, PlanError> {
    let title = "Set up Codex hooks".to_string();
    let root = parse_hooks(path, text)?;
    let events: Vec<&str> = STATUS_EVENTS
        .into_iter()
        .filter(|_| want.status)
        .chain(want.approve.then_some(PERMISSION_EVENT))
        .collect();

    // Already as asked: one exact entry per wanted event and no other entry of ours that the
    // request would replace.
    let replaced = |event: &str| want.status || event == PERMISSION_EVENT;
    let ours = ours_of(&root);
    let in_place = ours
        .iter()
        .filter(|(e, _)| replaced(e))
        .all(|(e, entry)| events.contains(e) && **entry == hook_entry(home, e))
        && events
            .iter()
            .all(|ev| ours.iter().filter(|(e, _)| e == ev).count() == 1);
    let had_permission = ours.iter().any(|(e, _)| *e == PERMISSION_EVENT);
    if in_place && (want.status || want.approve == had_permission) {
        return Ok(ChangeSet {
            title,
            reason: "The Codex hooks Marshell needs are already in place, so nothing changes.".to_string(),
            edits: Vec::new(),
        });
    }

    let mut ops: Vec<EditOp> = removal_pointers(&root, false)
        .into_iter()
        .filter(|p| want.status || p.starts_with("/hooks/PermissionRequest/"))
        .map(remove_op)
        .collect();
    let existing = root.get("hooks").and_then(Value::as_object);
    for event in &events {
        let group = json!({ "hooks": [hook_entry(home, event)] });
        if existing.is_some_and(|h| h.contains_key(*event)) {
            ops.push(set(format!("/hooks/{}/-", esc(event)), &group));
        } else {
            // Setting `/-` under a missing event would make an object with a "-" key.
            ops.push(set(format!("/hooks/{}", esc(event)), &json!([group])));
        }
    }

    let approval = if want.approve {
        "Adds the PermissionRequest hook for Approve from anywhere"
    } else if had_permission {
        "Removes Marshell's PermissionRequest hook"
    } else {
        ""
    };
    let reason = match (want.status, approval.is_empty()) {
        (true, true) => format!(
            "Adds {} hook entries so Marshell can show what Codex is doing. Codex asks you to trust them in /hooks.",
            STATUS_EVENTS.len()
        ),
        (true, false) => format!(
            "Adds {} hook entries so Marshell can show what Codex is doing. {approval}. Codex asks you to trust them in /hooks.",
            STATUS_EVENTS.len()
        ),
        (false, true) => "Nothing was asked for, so nothing changes.".to_string(),
        (false, false) => format!("{approval}."),
    };
    Ok(ChangeSet {
        title,
        reason,
        edits: ops.into_iter().map(|op| edit(path, op, text)).collect(),
    })
}

/// The plan that takes Marshell's entries out and leaves the user's own.
pub fn remove_plan(path: &Path, text: Option<&str>, _home: &Path) -> Result<ChangeSet, PlanError> {
    let title = "Turn Codex hooks off".to_string();
    let root = parse_hooks(path, text)?;
    let ops: Vec<EditOp> = removal_pointers(&root, true).into_iter().map(remove_op).collect();
    if ops.is_empty() {
        return Ok(ChangeSet {
            title,
            reason: "Marshell has no hook entries in this file, so nothing changes.".to_string(),
            edits: Vec::new(),
        });
    }
    let reason = match ours_of(&root).len() {
        1 => "Removes the hook entry Marshell added.".to_string(),
        n => format!("Removes the {n} hook entries Marshell added."),
    };
    Ok(ChangeSet {
        title,
        reason,
        edits: ops.into_iter().map(|op| edit(path, op, text)).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::hash::hash_bytes;
    use crate::config::jsonc::{JsonOp, apply_ops, read_value};
    use crate::paths;
    use marshell_protocol::agents::{HookInstall, HookPlanRequest};
    use marshell_protocol::config::{Applies, ChangeSet, ConfigScope, EditOp};
    use serde_json::{Value, json};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    const STATUS: HookPlanRequest = HookPlanRequest {
        status: true,
        statusline: true,
        approve: false,
    };
    const BOTH: HookPlanRequest = HookPlanRequest {
        status: true,
        statusline: true,
        approve: true,
    };
    const USER_GROUP: &str = r#"{"hooks":[{"type":"command","command":"echo mine"}]}"#;

    fn file() -> PathBuf {
        PathBuf::from("C:/Users/test user/.codex/hooks.json")
    }
    fn fake_home() -> PathBuf {
        PathBuf::from("C:/Users/test user/.marshell")
    }
    /// A scratch Marshell home whose bridge exists.
    fn real_home() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let home = std::env::temp_dir().join(format!(
            "marshell-codex-hooks-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let bridge = paths::bridge_exe(&home);
        std::fs::create_dir_all(bridge.parent().unwrap()).unwrap();
        std::fs::write(bridge, b"").unwrap();
        home
    }
    fn plain() -> String {
        format!("{{\n  \"hooks\": {{\n    \"Stop\": [{USER_GROUP}]\n  }}\n}}\n")
    }
    fn bom() -> String {
        format!("\u{feff}{}", plain())
    }
    fn crlf() -> String {
        plain().replace('\n', "\r\n")
    }
    fn comments() -> String {
        format!("{{\n  // my hooks\n  \"hooks\": {{\n    \"Stop\": [{USER_GROUP},], /* mine */\n  }},\n}}\n")
    }

    fn apply(cs: &ChangeSet, before: Option<&str>) -> Option<String> {
        let mut text = before.map(str::to_string);
        for e in &cs.edits {
            assert_eq!(e.applies, Applies::NewSessions);
            assert_eq!(e.scope, ConfigScope::User);
            assert_eq!(e.path, file().to_string_lossy());
            assert_eq!(e.expected_hash, before.map(|t| hash_bytes(t.as_bytes())));
            let next = match &e.op {
                EditOp::JsonSet { pointer, value } => {
                    let v: Value = serde_json::from_str(value).unwrap();
                    apply_ops(text.as_deref().unwrap_or("{}\n"), &[JsonOp::Set { pointer, value: &v }]).unwrap()
                }
                EditOp::JsonRemove { pointer } => match &text {
                    None => continue,
                    Some(t) => apply_ops(t, &[JsonOp::Remove { pointer }]).unwrap(),
                },
                other => panic!("unexpected op {other:?}"),
            };
            text = Some(next);
        }
        text
    }
    fn install(text: Option<&str>, home: &Path, want: &HookPlanRequest) -> (ChangeSet, Option<String>) {
        let cs = install_plan(&file(), text, home, want).unwrap();
        let after = apply(&cs, text);
        (cs, after)
    }
    fn value(text: &str) -> Value {
        read_value(text).unwrap()
    }

    #[test]
    fn install_into_a_missing_file_adds_every_event() {
        let home = fake_home();
        let (cs, after) = install(None, &home, &BOTH);
        assert_eq!(cs.title, "Set up Codex hooks");
        let v = value(&after.unwrap());
        for event in STATUS_EVENTS.into_iter().chain(["PermissionRequest"]) {
            let groups = v["hooks"][event].as_array().unwrap();
            assert_eq!(groups.len(), 1, "{event}");
            let entry = &groups[0]["hooks"][0];
            assert_eq!(entry["type"], "command");
            let command = entry["command"].as_str().unwrap();
            assert!(command.contains("--agent codex --tag mshl1"), "{command}");
            let (role, timeout) = match event {
                "PermissionRequest" => ("permission", 600),
                "SessionEnd" | "Interrupt" => ("event", 2),
                _ => ("event", 5),
            };
            assert!(command.contains(&format!("\" {role} --agent")), "{event}: {command}");
            assert_eq!(entry["timeout"], timeout, "{event}");
        }
    }

    #[test]
    fn hook_command_quotes_the_bridge_with_forward_slashes() {
        let command = hook_command(&fake_home(), "event");
        let bridge = format!(
            "C:/Users/test user/.marshell/bin/marshell-hook{}",
            std::env::consts::EXE_SUFFIX
        );
        let prefix = if cfg!(windows) { "& " } else { "" };
        assert_eq!(command, format!("{prefix}\"{bridge}\" event --agent codex --tag mshl1"));
    }

    #[test]
    fn status_only_adds_no_permission_entry() {
        let (_, after) = install(None, &fake_home(), &STATUS);
        let v = value(&after.unwrap());
        assert!(v["hooks"].get("PermissionRequest").is_none());
        assert_eq!(v["hooks"].as_object().unwrap().len(), STATUS_EVENTS.len());
    }

    #[test]
    fn install_keeps_the_users_own_hooks() {
        let before = plain();
        let (_, after) = install(Some(&before), &fake_home(), &BOTH);
        let after = after.unwrap();
        let v = value(&after);
        let stop = v["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0], value(USER_GROUP));
        assert!(after.contains(USER_GROUP));
    }

    #[test]
    fn install_on_an_installed_file_changes_nothing() {
        let home = real_home();
        let (_, first) = install(Some(&plain()), &home, &BOTH);
        let first = first.unwrap();
        let cs = install_plan(&file(), Some(&first), &home, &BOTH).unwrap();
        assert!(cs.edits.is_empty(), "{:?}", cs.edits);
        let found = inspect(&value(&first), &home);
        assert_eq!(found.install, HookInstall::Installed);
        assert!(found.approve_installed);
    }

    #[test]
    fn a_rerun_that_drops_approval_removes_only_the_permission_entry() {
        let home = real_home();
        let (_, first) = install(None, &home, &BOTH);
        let (_, second) = install(first.as_deref(), &home, &STATUS);
        let v = value(&second.unwrap());
        // Like Claude's plan, this leaves an emptied event as `[]`.
        assert!(v["hooks"]["PermissionRequest"].as_array().is_none_or(|a| a.is_empty()));
        assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn an_entry_from_another_home_is_orphaned() {
        let other = real_home();
        let (_, text) = install(None, &other, &BOTH);
        let mine = real_home();
        let found = inspect(&value(&text.unwrap()), &mine);
        assert_eq!(found.install, HookInstall::Orphaned);
        assert!(!found.approve_installed);
        assert_eq!(inspect(&json!({}), &mine).install, HookInstall::None);
    }

    #[test]
    fn remove_takes_only_ours() {
        for text in [plain(), bom(), crlf(), comments()] {
            let home = fake_home();
            let (_, installed) = install(Some(&text), &home, &BOTH);
            let installed = installed.unwrap();
            let cs = remove_plan(&file(), Some(&installed), &home).unwrap();
            assert_eq!(cs.title, "Turn Codex hooks off");
            let after = apply(&cs, Some(&installed)).unwrap();
            assert_eq!(value(&after), value(&text));
            assert!(after.contains(USER_GROUP));
            assert_eq!(after.starts_with('\u{feff}'), text.starts_with('\u{feff}'));
            assert_eq!(after.contains("\r\n"), text.contains("\r\n"));
        }
    }

    #[test]
    fn remove_from_a_file_without_ours_changes_nothing() {
        let cs = remove_plan(&file(), Some(&plain()), &fake_home()).unwrap();
        assert!(cs.edits.is_empty());
    }

    #[test]
    fn an_unparsable_file_is_refused_with_its_line() {
        let bad = "{\n  \"hooks\": {\n    \"Stop\": [\n";
        match install_plan(&file(), Some(bad), &fake_home(), &BOTH) {
            Err(PlanError::DoesNotParse { problem, .. }) => assert!(problem.0 >= 1),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            remove_plan(&file(), Some(bad), &fake_home()),
            Err(PlanError::DoesNotParse { .. })
        ));
    }

    #[test]
    fn a_hooks_value_of_the_wrong_shape_is_refused() {
        let bad = r#"{"hooks": {"Stop": {}}}"#;
        assert!(matches!(
            install_plan(&file(), Some(bad), &fake_home(), &BOTH),
            Err(PlanError::NotAnObject { .. })
        ));
    }

    #[test]
    fn codex_home_names_the_file() {
        let home = Path::new("C:/Users/test user");
        let with =
            |v: Option<&'static str>| move |k: &str| (k == "CODEX_HOME").then(|| v.map(str::to_string)).flatten();
        assert_eq!(
            hooks_path(&with(Some("D:/cx")), home),
            Path::new("D:/cx").join("hooks.json")
        );
        let default = home.join(".codex").join("hooks.json");
        assert_eq!(hooks_path(&with(Some("")), home), default);
        assert_eq!(hooks_path(&with(None), home), default);
    }
}
