//! Codex CLI's hook entries, planned as config changes. Nothing here touches a file: a plan is built
//! from the text of `hooks.json`, and the config service previews, snapshots and writes it. Every
//! doubt fails closed: a file that does not parse, or a `hooks` value of the wrong shape, gives an
//! error and no edits.

use crate::agents::common::{Bridge, PlanError, TAG, is_ours};
use crate::config::hash::hash_bytes;
use crate::config::jsonc::{parse_problem, read_value};
use crate::paths;
use crate::profiles::detect::{posix_quote, powershell_quote};
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

/// The bridge path quoted for the shell Codex runs a hook through: PowerShell on Windows, `sh`
/// elsewhere. Single quotes in both, so a `$` or a backtick in the path is never expanded.
fn quoted_bridge(home: &Path) -> String {
    if cfg!(windows) {
        powershell_quote(&bridge_path(home))
    } else {
        posix_quote(&bridge_path(home))
    }
}

/// What starts a hook command. Codex runs it through PowerShell on Windows, where a quoted path needs the `&`
/// call operator. In a distro it runs it through `sh`: the Linux path in POSIX single quotes.
fn bridge_command(bridge: &Bridge) -> String {
    match bridge.distro() {
        Some(_) => posix_quote(&bridge.path()),
        None => {
            let quoted = if cfg!(windows) {
                powershell_quote(&bridge.path())
            } else {
                posix_quote(&bridge.path())
            };
            format!("{}{quoted}", if cfg!(windows) { "& " } else { "" })
        }
    }
}

fn command_of(bridge: &Bridge, role: &str) -> String {
    format!("{} {role} --agent {AGENT} --tag {TAG}", bridge_command(bridge))
}

/// The command string of a hook entry: the single-quoted bridge path with forward slashes.
pub fn hook_command(home: &Path, role: &str) -> String {
    command_of(&Bridge::local(home), role)
}

fn entry_of(bridge: &Bridge, event: &str) -> Value {
    let (role, timeout) = match event {
        PERMISSION_EVENT => ("permission", PERMISSION_TIMEOUT),
        "SessionEnd" | "Interrupt" => ("event", SHORT_TIMEOUT),
        _ => ("event", HOOK_TIMEOUT),
    };
    json!({ "type": "command", "command": command_of(bridge, role), "timeout": timeout })
}

fn hook_entry(home: &Path, event: &str) -> Value {
    entry_of(&Bridge::local(home), event)
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

/// A group made only of Marshell's entries, with nothing else in it.
fn is_whole(group: &Value) -> bool {
    let Some(entries) = group.get("hooks").and_then(Value::as_array) else {
        return false;
    };
    !entries.is_empty() && entries.iter().all(is_ours) && group.as_object().is_some_and(|o| o.len() == 1)
}

/// What happens to one event's groups. Codex keys its trust by group position, so a group may only
/// leave when nothing after it stays: otherwise every later group would shift, and Codex would ask
/// the person to trust them again. A Marshell group with a group after it that stays becomes
/// `{"hooks": []}`, which Codex accepts and counts. With `wanted`, the first Marshell group is set
/// to it where it stands. An entry sharing a group with the user's own comes out alone, and the
/// group stays.
struct EventChange {
    /// Highest group first, so no edit shifts one that follows.
    ops: Vec<EditOp>,
    found: bool,
    groups_gone: usize,
    /// `wanted` was set at an existing Marshell group.
    replaced: bool,
}

fn event_change(event_pointer: &str, groups: &[Value], wanted: Option<&Value>) -> EventChange {
    let target = wanted.and_then(|_| groups.iter().position(is_whole));
    let mut change = EventChange {
        ops: Vec::new(),
        found: false,
        groups_gone: 0,
        replaced: false,
    };
    // Nothing after the group being looked at stays.
    let mut tail_clear = true;
    for (gi, group) in groups.iter().enumerate().rev() {
        let entries = group
            .get("hooks")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        let ours: Vec<usize> = (0..entries.len()).filter(|&i| is_ours(&entries[i])).collect();
        if ours.is_empty() {
            tail_clear = false;
            continue;
        }
        change.found = true;
        let pointer = format!("{event_pointer}/{gi}");
        if !is_whole(group) {
            change
                .ops
                .extend(ours.iter().rev().map(|ei| remove_op(format!("{pointer}/hooks/{ei}"))));
        } else if Some(gi) == target {
            if let Some(wanted) = wanted.filter(|w| *w != group) {
                change.ops.push(set(pointer, wanted));
            }
            change.replaced = true;
        } else if tail_clear {
            change.ops.push(remove_op(pointer));
            change.groups_gone += 1;
            continue;
        } else {
            change.ops.push(set(pointer, &json!({ "hooks": [] })));
        }
        tail_clear = false;
    }
    change
}

/// The edits that take Marshell's entries out of the events `in_scope` names. `wanted` gives an
/// event's group to set at its first Marshell group instead; the events where it did are returned.
/// With `prune`, an event whose groups all went goes, and `/hooks` itself when every event in it went.
fn removal_ops(
    root: &Value,
    prune: bool,
    in_scope: &dyn Fn(&str) -> bool,
    wanted: &dyn Fn(&str) -> Option<Value>,
) -> (Vec<EditOp>, Vec<String>) {
    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else {
        return (Vec::new(), Vec::new());
    };
    let mut out = Vec::new();
    let mut replaced = Vec::new();
    let mut events_gone = 0;
    for (event, groups) in hooks {
        let Some(groups) = groups.as_array().filter(|_| in_scope(event)) else {
            continue;
        };
        let event_pointer = format!("/hooks/{}", esc(event));
        let change = event_change(&event_pointer, groups, wanted(event).as_ref());
        if !change.found {
            continue;
        }
        if change.replaced {
            replaced.push(event.clone());
        }
        if prune && change.groups_gone == groups.len() {
            out.push(remove_op(event_pointer));
            events_gone += 1;
        } else {
            out.extend(change.ops);
        }
    }
    if prune && events_gone > 0 && events_gone == hooks.len() {
        return (vec![remove_op("/hooks".to_string())], replaced);
    }
    (out, replaced)
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
    // The first 6a wrote the path in double quotes; those entries still point at the bridge, so
    // they are not orphaned, only not exact (a reinstall sets the single-quoted form in place).
    let quoted = quoted_bridge(home);
    let old_quoted = format!("\"{}\"", bridge_path(home));
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
                .is_some_and(|c| c.contains(&quoted) || c.contains(&old_quoted))
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
/// Where an event already has a Marshell group, the wanted group is set at that position and the
/// user's groups stay where they are (Codex keys its trust by position). Other Marshell entries come
/// out as in `remove_plan`, and an event without a Marshell group gets one at the end, after the
/// user's own. A file that already holds exactly what is asked for gets no edits.
///
/// In a distro (`Bridge::Distro`) the entries run the Linux path of the bridge, and the reason sends the person
/// to trust them in Codex's `/hooks` inside that distro.
pub fn install_plan(
    path: &Path,
    text: Option<&str>,
    bridge: &Bridge,
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
        .all(|(e, entry)| events.contains(e) && **entry == entry_of(bridge, e))
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

    let group_of = |event: &str| json!({ "hooks": [entry_of(bridge, event)] });
    let (mut ops, replaced_in_place) = removal_ops(&root, false, &replaced, &|event| {
        events.contains(&event).then(|| group_of(event))
    });
    let existing = root.get("hooks").and_then(Value::as_object);
    for event in events.iter().filter(|e| !replaced_in_place.iter().any(|r| r == **e)) {
        let group = group_of(event);
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
    // A distro's Codex is the one that asks, so the sentence names the distro.
    let trust = match bridge.distro() {
        Some(distro) => format!("Codex inside {distro} asks you to trust them in /hooks."),
        None => "Codex asks you to trust them in /hooks.".to_string(),
    };
    let mut reason = match (want.status, approval.is_empty()) {
        (true, true) => format!(
            "Adds {} hook entries so Marshell can show what Codex is doing. {trust}",
            STATUS_EVENTS.len()
        ),
        (true, false) => format!(
            "Adds {} hook entries so Marshell can show what Codex is doing. {approval}. {trust}",
            STATUS_EVENTS.len()
        ),
        (false, true) => "Nothing was asked for, so nothing changes.".to_string(),
        (false, false) => format!("{approval}."),
    };
    if bridge.distro().is_some() && !want.status && want.approve {
        reason = format!("{reason} {trust}");
    }
    Ok(ChangeSet {
        title,
        reason,
        edits: ops.into_iter().map(|op| edit(path, op, text)).collect(),
    })
}

/// The plan that takes Marshell's entries out and leaves the user's own, each at the position it
/// had: a Marshell group with one of the user's after it becomes an empty group.
pub fn remove_plan(path: &Path, text: Option<&str>, _home: &Path) -> Result<ChangeSet, PlanError> {
    let title = "Turn Codex hooks off".to_string();
    let root = parse_hooks(path, text)?;
    let (ops, _) = removal_ops(&root, true, &|_| true, &|_| None);
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
        skill: false,
        distro: None,
    };
    const BOTH: HookPlanRequest = HookPlanRequest {
        status: true,
        statusline: true,
        approve: true,
        skill: false,
        distro: None,
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
        let cs = install_plan(&file(), text, &Bridge::local(home), want).unwrap();
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
            assert!(command.contains(&format!("' {role} --agent")), "{event}: {command}");
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
        assert_eq!(command, format!("{prefix}'{bridge}' event --agent codex --tag mshl1"));
    }

    #[test]
    fn hook_command_never_lets_the_shell_expand_the_path() {
        let home = PathBuf::from("C:/Users/a$b `c` it's/.marshell");
        let command = hook_command(&home, "event");
        let bridge = format!(
            "C:/Users/a$b `c` it's/.marshell/bin/marshell-hook{}",
            std::env::consts::EXE_SUFFIX
        );
        let quoted = if cfg!(windows) {
            // PowerShell: single quotes, a quote inside doubled.
            bridge.replace('\'', "''")
        } else {
            // sh: a quote inside closes, escapes and reopens.
            bridge.replace('\'', "'\\''")
        };
        let prefix = if cfg!(windows) { "& " } else { "" };
        assert_eq!(command, format!("{prefix}'{quoted}' event --agent codex --tag mshl1"));
        assert!(!command.contains('"'), "{command}");
    }

    /// The command the first 6a builds wrote: the path in double quotes.
    fn old_command(home: &Path, role: &str) -> String {
        let call = if cfg!(windows) { "& " } else { "" };
        format!("{call}\"{}\" {role} --agent codex --tag mshl1", bridge_path(home))
    }

    fn old_form_file(home: &Path) -> String {
        let entry = |role: &str, timeout: u64| json!({ "hooks": [{ "type": "command", "command": old_command(home, role), "timeout": timeout }] });
        json!({ "hooks": {
            "Stop": [{ "hooks": [{ "type": "command", "command": "echo mine" }] }, entry("event", 5)],
            "SessionEnd": [entry("event", 2)],
        } })
        .to_string()
    }

    #[test]
    fn double_quoted_entries_are_still_ours_and_get_repaired() {
        let home = real_home();
        let before = old_form_file(&home);
        // Recognised as ours and not orphaned: the bridge is there, the quoting is just old.
        assert_eq!(inspect(&value(&before), &home).install, HookInstall::Partial);
        // A reinstall sets the new form where the old entry stands.
        let (_, after) = install(Some(&before), &home, &STATUS);
        let v = value(&after.unwrap());
        assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(v["hooks"]["Stop"][1]["hooks"][0], hook_entry(&home, "Stop"));
        assert_eq!(v["hooks"]["SessionEnd"][0]["hooks"][0], hook_entry(&home, "SessionEnd"));
        assert!(!v.to_string().contains(&old_command(&home, "event")));
    }

    #[test]
    fn removal_takes_double_quoted_entries_too() {
        let home = real_home();
        let before = old_form_file(&home);
        let cs = remove_plan(&file(), Some(&before), &home).unwrap();
        let after = apply(&cs, Some(&before)).unwrap();
        assert!(!after.contains("mshl1"), "{after}");
        assert_eq!(
            value(&after)["hooks"]["Stop"],
            json!([{ "hooks": [{ "type": "command", "command": "echo mine" }] }])
        );
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
        let cs = install_plan(&file(), Some(&first), &Bridge::local(&home), &BOTH).unwrap();
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

    /// Everything of the user's in the fixture up to the end of their `Stop` group, and what
    /// follows their groups to the end of the file.
    fn user_head_and_tail(text: &str) -> (&str, &str) {
        let at = text.find(USER_GROUP).unwrap() + USER_GROUP.len();
        let tail = if text.contains("/* mine */") {
            "\n  },\n}\n"
        } else {
            "\n  }\n}\n"
        };
        (&text[..at], tail)
    }

    #[test]
    fn install_leaves_every_byte_outside_ours_alone() {
        for text in [plain(), bom(), crlf(), comments()] {
            let crlf = text.contains("\r\n");
            let (head, tail) = user_head_and_tail(&text);
            let tail = if crlf {
                tail.replace('\n', "\r\n")
            } else {
                tail.to_string()
            };
            let (_, after) = install(Some(&text), &fake_home(), &STATUS);
            let after = after.unwrap();
            // The BOM, the line endings, the comments and the user's group, byte for byte, then
            // the comma that now separates it from Marshell's group.
            assert!(after.starts_with(&format!("{head},")), "{after:?}");
            assert!(after.ends_with(&tail), "{after:?}");
            if text.contains("/* mine */") {
                assert!(after.contains("}], /* mine */\n    \"SessionStart\""), "{after:?}");
            }
            // The line endings stay all one kind.
            assert_eq!(
                after.matches("\r\n").count(),
                if crlf { after.matches('\n').count() } else { 0 }
            );
        }
    }

    #[test]
    #[ignore = "issue #63: removing a trailing Marshell group leaves a newline before `]` and drops a trailing comma"]
    fn removing_a_trailing_group_restores_the_original_bytes() {
        for text in [plain(), bom(), crlf(), comments()] {
            let home = fake_home();
            let (_, installed) = install(Some(&text), &home, &BOTH);
            let installed = installed.unwrap();
            let cs = remove_plan(&file(), Some(&installed), &home).unwrap();
            assert_eq!(apply(&cs, Some(&installed)).unwrap(), text);
        }
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

    /// Marshell's `Stop` group, as compact JSON.
    fn ours_stop(home: &Path) -> String {
        json!({ "hooks": [hook_entry(home, "Stop")] }).to_string()
    }

    #[test]
    fn removal_keeps_a_later_user_group_in_place() {
        for text in [plain(), bom(), crlf(), comments()] {
            let home = fake_home();
            let before = text.replacen(USER_GROUP, &format!("{},{USER_GROUP}", ours_stop(&home)), 1);
            let cs = remove_plan(&file(), Some(&before), &home).unwrap();
            let after = apply(&cs, Some(&before)).unwrap();
            let v = value(&after);
            assert_eq!(v["hooks"]["Stop"], json!([{ "hooks": [] }, value(USER_GROUP)]));
            assert!(after.contains(USER_GROUP));
            assert_eq!(after.starts_with('\u{feff}'), text.starts_with('\u{feff}'));
            assert_eq!(after.contains("\r\n"), text.contains("\r\n"));
            // Only Marshell's group changed, into the empty placeholder; the text on both sides is
            // the user's, byte for byte.
            let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
            let (head, tail) = before.split_once(&ours_stop(&home)).unwrap();
            let placeholder = format!("{{{eol}        \"hooks\": []{eol}      }}");
            assert_eq!(after, format!("{head}{placeholder}{tail}"));
        }
    }

    #[test]
    fn removal_drops_a_trailing_group() {
        let home = fake_home();
        let before = plain().replacen(USER_GROUP, &format!("{USER_GROUP},{}", ours_stop(&home)), 1);
        let cs = remove_plan(&file(), Some(&before), &home).unwrap();
        let after = apply(&cs, Some(&before)).unwrap();
        assert_eq!(value(&after)["hooks"]["Stop"], json!([value(USER_GROUP)]));
    }

    #[test]
    fn an_event_with_a_placeholder_left_stays() {
        let home = fake_home();
        let ours = ours_stop(&home);
        let before = format!(r#"{{"hooks":{{"Stop":[{ours},{USER_GROUP}],"SessionEnd":[{ours}]}}}}"#);
        let cs = remove_plan(&file(), Some(&before), &home).unwrap();
        let v = value(&apply(&cs, Some(&before)).unwrap());
        assert_eq!(v["hooks"]["Stop"], json!([{ "hooks": [] }, value(USER_GROUP)]));
        assert!(v["hooks"].get("SessionEnd").is_none());
    }

    #[test]
    fn reinstall_replaces_ours_where_it_is() {
        let home = fake_home();
        let mut old = hook_entry(&home, "Stop");
        old["timeout"] = json!(9);
        let before = plain().replacen(USER_GROUP, &format!("{},{USER_GROUP}", json!({ "hooks": [old] })), 1);
        let (_, after) = install(Some(&before), &home, &STATUS);
        let after = after.unwrap();
        let stop = value(&after)["hooks"]["Stop"].clone();
        assert_eq!(
            stop,
            json!([{ "hooks": [hook_entry(&home, "Stop")] }, value(USER_GROUP)])
        );
        assert!(after.contains(USER_GROUP));
    }

    #[test]
    fn install_after_a_placeholder_appends() {
        let home = fake_home();
        let before = plain().replacen(USER_GROUP, &format!(r#"{{"hooks":[]}},{USER_GROUP}"#), 1);
        let (_, after) = install(Some(&before), &home, &STATUS);
        let stop = value(&after.unwrap())["hooks"]["Stop"].clone();
        assert_eq!(
            stop,
            json!([{ "hooks": [] }, value(USER_GROUP), { "hooks": [hook_entry(&home, "Stop")] }])
        );
    }

    #[test]
    fn remove_from_a_file_without_ours_changes_nothing() {
        let cs = remove_plan(&file(), Some(&plain()), &fake_home()).unwrap();
        assert!(cs.edits.is_empty());
    }

    #[test]
    fn an_unparsable_file_is_refused_with_its_line() {
        let bad = "{\n  \"hooks\": {\n    \"Stop\": [\n";
        match install_plan(&file(), Some(bad), &Bridge::local(&fake_home()), &BOTH) {
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
            install_plan(&file(), Some(bad), &Bridge::local(&fake_home()), &BOTH),
            Err(PlanError::NotAnObject { .. })
        ));
    }

    fn distro_bridge(path: &str) -> Bridge {
        Bridge::Distro {
            name: "Ubuntu".into(),
            path: path.into(),
        }
    }

    #[test]
    fn a_distro_plan_writes_the_linux_bridge() {
        let linux = "/mnt/c/Users/x y/.marshell/bin/marshell-hook.exe";
        let cs = install_plan(&file(), None, &distro_bridge(linux), &BOTH).unwrap();
        let after = value(&apply(&cs, None).unwrap());
        for event in STATUS_EVENTS.into_iter().chain(["PermissionRequest"]) {
            let command = after["hooks"][event][0]["hooks"][0]["command"].as_str().unwrap();
            let role = if event == "PermissionRequest" {
                "permission"
            } else {
                "event"
            };
            // A shell string for `sh`: the path in single quotes, no PowerShell call operator.
            assert_eq!(
                command,
                format!("'{linux}' {role} --agent codex --tag mshl1"),
                "{event}"
            );
        }
        assert!(
            cs.reason
                .ends_with("Codex inside Ubuntu asks you to trust them in /hooks."),
            "{}",
            cs.reason
        );
        // One trust sentence, not the Windows one and a second for the distro.
        assert_eq!(cs.reason.matches("trust them").count(), 1, "{}", cs.reason);

        // A quote in the path is closed, escaped and opened again.
        let cs = install_plan(&file(), None, &distro_bridge("/mnt/c/it's/marshell-hook.exe"), &STATUS).unwrap();
        let after = value(&apply(&cs, None).unwrap());
        assert_eq!(
            after["hooks"]["Stop"][0]["hooks"][0]["command"],
            r"'/mnt/c/it'\''s/marshell-hook.exe' event --agent codex --tag mshl1"
        );
    }

    #[test]
    fn a_windows_plan_does_not_send_the_person_into_a_distro() {
        let cs = install_plan(&file(), None, &Bridge::local(&fake_home()), &BOTH).unwrap();
        assert!(!cs.reason.contains("inside"), "{}", cs.reason);
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
