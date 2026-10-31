//! Antigravity CLI's hook entries and status line, planned as config changes. Nothing here touches a
//! file: a plan is built from the text of `hooks.json`, `settings.json` and the chain file, and the
//! config service previews, snapshots and writes it. Every doubt fails closed: a file that does not
//! parse, or a value of the wrong shape, gives an error and no edits.
//!
//! Marshell's entries live under one named hook, `marshell`, in `~/.gemini/config/hooks.json`; the
//! person's own named hooks are never touched. The status line is `statusLine` in
//! `~/.gemini/antigravity-cli/settings.json`, chained the way Claude Code's is.

use crate::agents::common::{Bridge, PlanError, TAG, is_ours, statusline_left_note};
use crate::config::hash::hash_bytes;
use crate::config::jsonc::{parse_problem, read_value};
use crate::paths;
use crate::profiles::detect::posix_quote;
use marshell_protocol::agents::{HookInstall, HookPlanRequest};
use marshell_protocol::config::{Applies, ChangeSet, ConfigScope, EditOp, FileEdit};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// The one named hook in `hooks.json` that holds Marshell's entries.
pub const HOOK_NAME: &str = "marshell";

/// The events whose hooks carry live status, in the order they are installed. `PreToolUse` is
/// Marshell's approve entry, or with approve off an event entry, so it is not in this list.
pub const STATUS_EVENTS: [&str; 4] = ["PostToolUse", "PreInvocation", "PostInvocation", "Stop"];

const AGENT: &str = "antigravity";
const AGENT_NAME: &str = "Antigravity";
const PRE_TOOL: &str = "PreToolUse";
const HOOK_TIMEOUT: u64 = 5;
/// The bridge waits 590 s for the person's answer.
const PERMISSION_TIMEOUT: u64 = 600;
const REASON_PERCENT_OR_QUOTE: &str = "the path holds a % or a quote";
const REASON_NO_DRIVE: &str = "the path has no drive letter";

/// `<user home>/.gemini/config/hooks.json`.
pub fn hooks_path(user_home: &Path) -> PathBuf {
    user_home.join(".gemini").join("config").join("hooks.json")
}

/// `<user home>/.gemini/antigravity-cli/settings.json`.
pub fn settings_path(user_home: &Path) -> PathBuf {
    user_home.join(".gemini").join("antigravity-cli").join("settings.json")
}

// ---- the command ----

/// The bridge, written so the shell agy runs a hook through reads it as one word.
///
/// On Windows agy runs `cmd /c "<command>"` and escapes every `"` inside the command to `\"`, so a
/// command that starts with a quote fails. What runs, with a space and `& ( ) ; , = ^` in the path, is
/// the folder in quotes from just after the drive to just before the file name, with forward
/// slashes: `C:"/Users/Home Office/.marshell/bin"/marshell-hook.exe`. A `%` can't be protected from
/// cmd, and a `"` is safe only next to a separator, so a path with either is refused, and so is one
/// with no drive letter (a UNC path). A `\\?\C:\` prefix is dropped, and `\\?\UNC\...` refused as
/// any other UNC path. Elsewhere the path is single-quoted for `sh`.
///
/// In a distro agy runs the command through `sh`, so the Linux path is single-quoted.
fn bridge_prefix_of(bridge: &Bridge) -> Result<String, PlanError> {
    let Bridge::Local(exe) = bridge else {
        return Ok(posix_quote(&bridge.path()));
    };
    let forward = bridge.path();
    if cfg!(windows) {
        cmd_form(exe, &forward)
    } else {
        Ok(posix_quote(&forward))
    }
}

fn bridge_prefix(home: &Path) -> Result<String, PlanError> {
    bridge_prefix_of(&Bridge::local(home))
}

fn cmd_form(exe: &Path, forward: &str) -> Result<String, PlanError> {
    let refuse = |reason| PlanError::BridgePath {
        agent: AGENT_NAME,
        path: exe.to_path_buf(),
        reason,
    };
    if forward.contains(['%', '"']) {
        return Err(refuse(REASON_PERCENT_OR_QUOTE));
    }
    let path = forward.strip_prefix("//?/").unwrap_or(forward);
    let bytes = path.as_bytes();
    if !(bytes.len() > 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/') {
        return Err(refuse(REASON_NO_DRIVE));
    }
    let (drive, tail) = path.split_at(2);
    let (folder, file) = tail.rsplit_once('/').unwrap_or(("", tail));
    Ok(if folder.is_empty() {
        format!("{drive}/{file}")
    } else {
        format!("{drive}\"{folder}\"/{file}")
    })
}

/// The command string of a hook entry.
pub fn hook_command(home: &Path, role: &str, event: &str) -> Result<String, PlanError> {
    command_of(&Bridge::local(home), role, event)
}

fn command_of(bridge: &Bridge, role: &str, event: &str) -> Result<String, PlanError> {
    Ok(format!(
        "{} {role} --agent {AGENT} --event {event} --tag {TAG}",
        bridge_prefix_of(bridge)?
    ))
}

/// The command string of Marshell's status line. In a distro, which keeps no chain, it says `--no-chain`.
fn statusline_of(bridge: &Bridge) -> Result<String, PlanError> {
    let no_chain = if bridge.distro().is_some() { " --no-chain" } else { "" };
    Ok(format!(
        "{} statusline --agent {AGENT} --tag {TAG}{no_chain}",
        bridge_prefix_of(bridge)?
    ))
}

pub fn statusline_command(home: &Path) -> Result<String, PlanError> {
    statusline_of(&Bridge::local(home))
}

/// One event's value in the `marshell` hook. `PreToolUse` and `PostToolUse` take matcher groups, the
/// other events a flat list of entries. `permission` makes `PreToolUse` the approve entry.
fn event_value_of(bridge: &Bridge, event: &str, permission: bool) -> Result<Value, PlanError> {
    let (role, timeout) = if permission && event == PRE_TOOL {
        ("permission", PERMISSION_TIMEOUT)
    } else {
        ("event", HOOK_TIMEOUT)
    };
    let entry = json!({ "type": "command", "command": command_of(bridge, role, event)?, "timeout": timeout });
    Ok(match event {
        PRE_TOOL | "PostToolUse" => json!([{ "matcher": ".*", "hooks": [entry] }]),
        _ => json!([entry]),
    })
}

fn event_value(home: &Path, event: &str, permission: bool) -> Result<Value, PlanError> {
    event_value_of(&Bridge::local(home), event, permission)
}

// ---- reading the files ----

fn not_an_object(path: &Path, pointer: impl Into<String>) -> PlanError {
    PlanError::NotAnObject {
        path: path.to_path_buf(),
        pointer: pointer.into(),
    }
}

/// A file's text as a value (a missing file is `{}`) that is an object.
fn parse_object(path: &Path, text: Option<&str>) -> Result<Value, PlanError> {
    let Some(text) = text else { return Ok(json!({})) };
    let Ok(root) = read_value(text) else {
        let problem = parse_problem(text).unwrap_or((1, "The file does not parse.".to_string()));
        return Err(PlanError::DoesNotParse {
            path: path.to_path_buf(),
            problem,
        });
    };
    if !root.is_object() {
        return Err(not_an_object(path, ""));
    }
    Ok(root)
}

fn parse_hooks(path: &Path, text: Option<&str>) -> Result<Value, PlanError> {
    let root = parse_object(path, text)?;
    if root.get(HOOK_NAME).is_some_and(|h| !h.is_object()) {
        return Err(not_an_object(path, format!("/{HOOK_NAME}")));
    }
    Ok(root)
}

fn parse_settings(path: &Path, text: Option<&str>) -> Result<Value, PlanError> {
    let root = parse_object(path, text)?;
    if root.get("statusLine").is_some_and(|l| !l.is_null() && !l.is_object()) {
        return Err(not_an_object(path, "/statusLine"));
    }
    Ok(root)
}

/// Every hook entry of the `marshell` hook as (event, entry): inside a matcher group's `hooks`, or
/// the event's own list item. Anything of another shape is skipped.
fn entries_of(hook: &Value) -> Vec<(&str, &Value)> {
    let mut out = Vec::new();
    for (event, items) in hook.as_object().into_iter().flatten() {
        for item in items.as_array().into_iter().flatten() {
            match item.get("hooks").and_then(Value::as_array) {
                Some(inner) => out.extend(inner.iter().map(|e| (event.as_str(), e))),
                None => out.push((event.as_str(), item)),
            }
        }
    }
    out
}

fn ours_in(hooks: &Value) -> Vec<(&str, &Value)> {
    hooks
        .get(HOOK_NAME)
        .map(entries_of)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, e)| is_ours(e))
        .collect()
}

fn is_permission(entry: &Value) -> bool {
    entry
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains(" permission --agent "))
}

/// A command that is Marshell's status line.
fn line_is_ours(line: &Value) -> bool {
    line.get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains(&format!("--tag {TAG}")))
}

/// The chain file's value, `{"statusLine": ...}`, holds a status line of the user's own.
fn chain_holds_users_line(chain: Option<&Value>) -> bool {
    chain
        .and_then(|c| c.get("statusLine"))
        .is_some_and(|s| s.is_object() && !line_is_ours(s))
}

/// The chain can be forwarded to: it holds the user's line, or says they had none.
fn chain_usable(chain: Option<&Value>) -> bool {
    chain_holds_users_line(chain) || chain.and_then(|c| c.get("statusLine")).is_some_and(Value::is_null)
}

// ---- inspect ----

pub struct Found {
    pub install: HookInstall,
    /// The `PreToolUse` approve hook is in place.
    pub approve_installed: bool,
    /// A status line that is not Marshell's, or the one the chain keeps.
    pub has_own_statusline: bool,
    /// Ours is in place and the chain holds the user's.
    pub chained: bool,
}

/// What the files (as values, `None` for a missing file) say about the install. The chain is
/// `{"statusLine": ...}`.
pub fn inspect(hooks: Option<&Value>, settings: Option<&Value>, chain: Option<&Value>, home: &Path) -> Found {
    let none = json!({});
    let hooks = hooks.unwrap_or(&none);
    let hook = hooks.get(HOOK_NAME).filter(|h| h.is_object());
    let ours = ours_in(hooks);
    let line = settings.and_then(|s| s.get("statusLine")).filter(|l| l.is_object());
    let line_ours = line.is_some_and(line_is_ours);
    let bridge_exists = paths::bridge_exe(home).exists();

    let event_is = |event: &str, permission: bool| {
        event_value(home, event, permission)
            .ok()
            .is_some_and(|want| hook.and_then(|h| h.get(event)) == Some(&want))
    };
    let permission_exact = event_is(PRE_TOOL, true);
    let hooks_exact = hook.is_some_and(|h| {
        h.get("enabled") == Some(&Value::Bool(true))
            && h.as_object().is_some_and(|o| o.len() == STATUS_EVENTS.len() + 2)
    }) && (permission_exact || event_is(PRE_TOOL, false))
        && STATUS_EVENTS.iter().all(|e| event_is(e, false));

    let prefix = bridge_prefix(home).ok();
    let from_this_home =
        |command: Option<&str>| matches!((command, &prefix), (Some(c), Some(p)) if c.starts_with(p.as_str()));
    let line_command = line.and_then(|l| l.get("command")).and_then(Value::as_str);
    let line_current = statusline_command(home)
        .ok()
        .is_some_and(|c| line_command == Some(c.as_str()));
    let chain_holds_users = chain_holds_users_line(chain);

    let install = if ours.is_empty() && !line_ours {
        HookInstall::None
    } else if !bridge_exists
        || ours
            .iter()
            .any(|(_, e)| !from_this_home(e.get("command").and_then(Value::as_str)))
        || (line_ours && !from_this_home(line_command))
    {
        HookInstall::Orphaned
    } else if hooks_exact && (!line_ours || (chain_usable(chain) && line_current)) {
        HookInstall::Installed
    } else {
        HookInstall::Partial
    };
    Found {
        install,
        approve_installed: permission_exact && bridge_exists,
        has_own_statusline: (line.is_some() && !line_ours) || (line_ours && chain_holds_users),
        chained: line_ours && chain_holds_users,
    }
}

// ---- plans ----

fn edit(path: &Path, scope: ConfigScope, op: EditOp, text: Option<&str>) -> FileEdit {
    FileEdit {
        path: path.to_string_lossy().into_owned(),
        scope,
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

fn remove_op(pointer: &str) -> EditOp {
    EditOp::JsonRemove {
        pointer: pointer.to_string(),
    }
}

/// What the `marshell` hook should hold for this request. With status off the existing status
/// events stay as they are, and only `PreToolUse` follows `approve`.
fn desired_hook(bridge: &Bridge, want: &HookPlanRequest, existing: Option<&Value>) -> Result<Value, PlanError> {
    let kept = |event: &str| existing.and_then(|h| h.get(event));
    let mut hook = Map::new();
    hook.insert("enabled".into(), Value::Bool(true));
    if want.approve || want.status {
        hook.insert(PRE_TOOL.into(), event_value_of(bridge, PRE_TOOL, want.approve)?);
    } else if let Some(old) = kept(PRE_TOOL)
        && !entries_of(&json!({ PRE_TOOL: old }))
            .iter()
            .any(|(_, e)| is_permission(e))
    {
        hook.insert(PRE_TOOL.into(), old.clone());
    }
    for event in STATUS_EVENTS {
        if want.status {
            hook.insert(event.into(), event_value_of(bridge, event, false)?);
        } else if let Some(old) = kept(event) {
            hook.insert(event.into(), old.clone());
        }
    }
    Ok(Value::Object(hook))
}

/// The edit that takes the `marshell` hook out of `hooks.json`: the file goes when nothing else is in it.
fn hook_removal(hooks: &Value, path: &Path, text: Option<&str>) -> Option<FileEdit> {
    hooks.get(HOOK_NAME)?;
    let only_ours = hooks.as_object().is_some_and(|o| o.len() == 1);
    let op = if only_ours {
        EditOp::DeleteFile
    } else {
        remove_op(&format!("/{HOOK_NAME}"))
    };
    Some(edit(path, ConfigScope::User, op, text))
}

/// The plan for the hook entries, the status line and optional approval. Status off leaves the
/// status events and the status line alone.
///
/// The `marshell` hook is set whole, so a rerun never duplicates an entry, and `hooks.json` is
/// created when it is missing. The status line follows Claude Code's chain: the person's own is kept
/// whole in the chain file, and Marshell's keeps their `padding` and `stack_with_default`. With none
/// of their own, `stack_with_default` is on, so agy's own line shows under the bridge's empty output.
/// `dangerouslySkipPermissions` is never touched.
///
/// `chain` is the chain file, `None` in a distro (see `Bridge::Distro`): there the chain is neither read nor
/// written, a status line that is not Marshell's stays as it is, and Marshell's own line says `--no-chain`.
pub fn install_plan(
    user_home: &Path,
    hooks_text: Option<&str>,
    settings_text: Option<&str>,
    chain_text: Option<&str>,
    chain: Option<&Path>,
    bridge: &Bridge,
    want: &HookPlanRequest,
) -> Result<ChangeSet, PlanError> {
    let title = "Set up Antigravity hooks".to_string();
    let (hooks_file, settings_file) = (hooks_path(user_home), settings_path(user_home));
    let hooks = parse_hooks(&hooks_file, hooks_text)?;
    let settings = parse_settings(&settings_file, settings_text)?;
    let existing = hooks.get(HOOK_NAME);
    let desired = desired_hook(bridge, want, existing)?;
    let wanted_events = desired.as_object().is_some_and(|o| o.len() > 1);

    let mut edits = Vec::new();
    if existing != Some(&desired) {
        if !wanted_events {
            edits.extend(hook_removal(&hooks, &hooks_file, hooks_text));
        } else if hooks_text.is_none() {
            let content = serde_json::to_string_pretty(&json!({ HOOK_NAME: desired })).unwrap_or_default() + "\n";
            edits.push(edit(
                &hooks_file,
                ConfigScope::User,
                EditOp::CreateFile { content },
                None,
            ));
        } else {
            edits.push(edit(
                &hooks_file,
                ConfigScope::User,
                set(format!("/{HOOK_NAME}"), &desired),
                hooks_text,
            ));
        }
    }

    let had_permission = ours_in(&hooks).iter().any(|(_, e)| is_permission(e));
    let approval = if want.approve {
        "Adds the PreToolUse hook for Approve from anywhere"
    } else if had_permission {
        "Removes Marshell's PreToolUse approval hook"
    } else {
        ""
    };

    let mut tail = "";
    let mut left_alone = None;
    if want.status {
        let ours = statusline_of(bridge)?;
        let mut ops = Vec::new();
        let mut chain_op = None;
        let line = settings.get("statusLine").filter(|l| !l.is_null());
        match line {
            Some(line) if line_is_ours(line) => {
                // Ours already: the user's line is never read back from it. A line that runs another
                // home's bridge is pointed at this one, and a chain with nothing to forward to says
                // the user had none.
                if line.get("command").and_then(Value::as_str) != Some(&ours) {
                    ops.push(set("/statusLine/command", &json!(ours)));
                }
                let saved = chain_text.and_then(|t| read_value(t).ok());
                if chain.is_some() && !chain_usable(saved.as_ref()) {
                    chain_op = Some(set("/statusLine", &Value::Null));
                    if line.get("stack_with_default") != Some(&Value::Bool(true)) {
                        ops.push(set("/statusLine/stack_with_default", &Value::Bool(true)));
                    }
                    tail = ", and runs Marshell's status line on its own, because the saved copy of yours is missing";
                }
            }
            Some(_) if chain.is_none() => left_alone = bridge.distro().map(statusline_left_note),
            Some(line) if want.statusline => {
                ops.push(set("/statusLine/command", &json!(ours)));
                chain_op = Some(set("/statusLine", line));
                tail = ", and passes your status line through Marshell, which runs it as before";
            }
            Some(_) => {}
            None => {
                ops.push(set(
                    "/statusLine",
                    &json!({ "type": "command", "command": ours, "stack_with_default": true }),
                ));
                chain_op = Some(set("/statusLine", &Value::Null));
                tail = ", and adds a status line so context data arrives";
            }
        }
        edits.extend(
            ops.into_iter()
                .map(|op| edit(&settings_file, ConfigScope::User, op, settings_text)),
        );
        if let (Some(op), Some(chain)) = (chain_op, chain) {
            edits.push(edit(chain, ConfigScope::Marshell, op, chain_text));
        }
    }

    let mut reason = if edits.is_empty() {
        if want.status || want.approve || existing.is_some() {
            "The Antigravity hooks Marshell needs are already in place, so nothing changes.".to_string()
        } else {
            "Nothing was asked for, so nothing changes.".to_string()
        }
    } else if want.status {
        let how =
            format!("Adds the {HOOK_NAME} hook to hooks.json so Marshell can show what Antigravity is doing{tail}");
        if approval.is_empty() {
            format!("{how}.")
        } else {
            format!("{how}. {approval}.")
        }
    } else {
        format!("{approval}.")
    };
    if let Some(note) = left_alone {
        reason = format!("{reason} {note}");
    }
    Ok(ChangeSet { title, reason, edits })
}

/// What the chain file holds for the status line.
enum Saved {
    /// The user's command.
    Command(String),
    /// The user had no status line.
    Nothing,
    /// Missing, unreadable, or naming Marshell's own command.
    Unusable,
}

fn saved_line(chain_text: Option<&str>, in_distro: bool) -> Saved {
    if in_distro {
        return Saved::Nothing;
    }
    let Some(chain) = chain_text.and_then(|t| read_value(t).ok()) else {
        return Saved::Unusable;
    };
    match chain.get("statusLine") {
        Some(Value::Null) => Saved::Nothing,
        Some(line) if line.is_object() && !line_is_ours(line) => match line.get("command").and_then(Value::as_str) {
            Some(command) => Saved::Command(command.to_string()),
            None => Saved::Unusable,
        },
        _ => Saved::Unusable,
    }
}

/// The plan that takes Marshell's entries out and puts the user's status line back.
///
/// `hooks.json` is deleted when nothing but the `marshell` hook is in it. The chain file is read and
/// never changed. A status line that is no longer Marshell's was changed by the person since the
/// install and is left alone.
///
/// In a distro (`distro` is its name) there is no chain: a status line of Marshell's was added by it, and goes,
/// and any other is the person's, left alone.
pub fn remove_plan(
    user_home: &Path,
    hooks_text: Option<&str>,
    settings_text: Option<&str>,
    chain_text: Option<&str>,
    distro: Option<&str>,
) -> Result<ChangeSet, PlanError> {
    let title = "Turn Antigravity hooks off".to_string();
    let (hooks_file, settings_file) = (hooks_path(user_home), settings_path(user_home));
    let hooks = parse_hooks(&hooks_file, hooks_text)?;
    let settings = parse_settings(&settings_file, settings_text)?;

    let mut edits = Vec::new();
    let hook_edit = hook_removal(&hooks, &hooks_file, hooks_text);
    let file_deleted = hook_edit.as_ref().is_some_and(|e| e.op == EditOp::DeleteFile);
    let removed = ours_in(&hooks).len();
    edits.extend(hook_edit);

    let mut line_note = "";
    let mut line_op = None;
    match settings.get("statusLine") {
        Some(line) if line.is_object() && line_is_ours(line) => match saved_line(chain_text, distro.is_some()) {
            Saved::Command(command) => {
                line_op = Some(set("/statusLine/command", &json!(command)));
                line_note = "puts your status line back";
            }
            Saved::Nothing => {
                line_op = Some(remove_op("/statusLine"));
                line_note = "removes the status line it added";
            }
            Saved::Unusable => {
                line_op = Some(remove_op("/statusLine"));
                line_note = "removes its status line, because the saved copy of yours is missing and there is nothing to put back";
            }
        },
        Some(line) if !line.is_null() && (chain_text.is_some() || distro.is_some()) => {
            line_note = if distro.is_some() {
                "leaves your status line alone, because it is not Marshell's"
            } else {
                "leaves your status line alone, because it was changed since the setup"
            };
        }
        _ => {}
    }
    edits.extend(line_op.map(|op| edit(&settings_file, ConfigScope::User, op, settings_text)));

    if edits.is_empty() {
        return Ok(ChangeSet {
            title,
            reason: "Marshell has no hook entries or status line in these files, so nothing changes.".to_string(),
            edits,
        });
    }
    let entries = match (removed, file_deleted) {
        (0, _) if edits.iter().all(|e| e.path != hooks_file.to_string_lossy()) => String::new(),
        (n, true) => format!(
            "Removes the {n} hook {} Marshell added and deletes hooks.json, which holds nothing else",
            if n == 1 { "entry" } else { "entries" }
        ),
        (1, false) => "Removes the hook entry Marshell added".to_string(),
        (n, false) => format!("Removes the {n} hook entries Marshell added"),
    };
    let reason = match (entries.is_empty(), line_note.is_empty()) {
        (false, false) => format!("{entries} and {line_note}."),
        (false, true) => format!("{entries}."),
        (true, _) => format!("It {line_note}."),
    };
    Ok(ChangeSet { title, reason, edits })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::jsonc::{JsonOp, apply_ops};
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
    const USER_HOOK: &str = r#""mine": { "enabled": true, "Stop": [{ "type": "command", "command": "echo mine" }] }"#;

    fn user_home() -> PathBuf {
        PathBuf::from("C:/Users/test user")
    }
    fn fake_home() -> PathBuf {
        PathBuf::from("C:/Users/test user/.marshell")
    }
    fn hooks_file() -> PathBuf {
        hooks_path(&user_home())
    }
    fn settings_file() -> PathBuf {
        settings_path(&user_home())
    }
    fn chain_file(home: &Path) -> PathBuf {
        paths::statusline_chain(home, "antigravity")
    }
    /// A scratch Marshell home whose bridge exists.
    fn real_home() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let home = std::env::temp_dir().join(format!(
            "marshell-agy-hooks-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let bridge = paths::bridge_exe(&home);
        std::fs::create_dir_all(bridge.parent().unwrap()).unwrap();
        std::fs::write(bridge, b"").unwrap();
        home
    }
    fn plain_hooks() -> String {
        format!("{{\n  {USER_HOOK}\n}}\n")
    }
    fn bom_hooks() -> String {
        format!("\u{feff}{}", plain_hooks())
    }
    fn crlf_hooks() -> String {
        plain_hooks().replace('\n', "\r\n")
    }
    fn comments_hooks() -> String {
        format!("{{\n  // my hooks\n  {USER_HOOK}, /* mine */\n}}\n")
    }
    fn hooks_variants() -> [String; 4] {
        [plain_hooks(), bom_hooks(), crlf_hooks(), comments_hooks()]
    }
    fn fixture(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/claude/settings")
            .join(name);
        std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
    }
    fn value(text: &str) -> Value {
        read_value(text).unwrap()
    }

    /// The three files a plan touches, as text; `None` is a missing file.
    #[derive(Clone, Default)]
    struct Files {
        hooks: Option<String>,
        settings: Option<String>,
        chain: Option<String>,
    }
    impl Files {
        fn hooks_value(&self) -> Value {
            value(self.hooks.as_deref().unwrap())
        }
        fn settings_value(&self) -> Value {
            value(self.settings.as_deref().unwrap())
        }
        fn chain_value(&self) -> Value {
            value(self.chain.as_deref().unwrap())
        }
    }

    /// Applies a ChangeSet the way `preview` does: per path, in order, a missing file starting as
    /// `{}`. Every edit must carry the hash of the text the plan was built from.
    fn apply(cs: &ChangeSet, before: &Files, home: &Path) -> Files {
        let mut after = before.clone();
        for e in &cs.edits {
            assert_eq!(e.applies, Applies::NewSessions);
            let (slot, original, scope) = if e.path == hooks_file().to_string_lossy() {
                (&mut after.hooks, &before.hooks, ConfigScope::User)
            } else if e.path == settings_file().to_string_lossy() {
                (&mut after.settings, &before.settings, ConfigScope::User)
            } else if e.path == chain_file(home).to_string_lossy() {
                (&mut after.chain, &before.chain, ConfigScope::Marshell)
            } else {
                panic!("unexpected path {}", e.path)
            };
            assert_eq!(e.scope, scope, "scope of {}", e.path);
            let want_hash = match (&e.op, original) {
                (EditOp::CreateFile { .. }, _) => None,
                (_, original) => original.as_ref().map(|t| hash_bytes(t.as_bytes())),
            };
            assert_eq!(e.expected_hash, want_hash, "expected_hash of {}", e.path);
            let next = match &e.op {
                EditOp::JsonSet { pointer, value } => {
                    let v: Value = serde_json::from_str(value).unwrap();
                    Some(apply_ops(slot.as_deref().unwrap_or("{}\n"), &[JsonOp::Set { pointer, value: &v }]).unwrap())
                }
                EditOp::JsonRemove { pointer } => match slot {
                    None => continue,
                    Some(text) => Some(apply_ops(text, &[JsonOp::Remove { pointer }]).unwrap()),
                },
                EditOp::CreateFile { content } => {
                    assert!(slot.is_none(), "CreateFile over an existing {}", e.path);
                    Some(content.clone())
                }
                EditOp::DeleteFile => {
                    assert!(slot.is_some(), "DeleteFile of a missing {}", e.path);
                    assert!(e.expected_hash.is_some(), "DeleteFile needs the hash");
                    None
                }
                other => panic!("unexpected op {other:?}"),
            };
            *slot = next;
        }
        after
    }

    fn try_install(files: &Files, want: &HookPlanRequest, home: &Path) -> Result<ChangeSet, PlanError> {
        install_plan(
            &user_home(),
            files.hooks.as_deref(),
            files.settings.as_deref(),
            files.chain.as_deref(),
            Some(&paths::statusline_chain(home, AGENT)),
            &Bridge::local(home),
            want,
        )
    }
    fn install(files: &Files, want: &HookPlanRequest, home: &Path) -> (ChangeSet, Files) {
        let cs = try_install(files, want, home).unwrap();
        let after = apply(&cs, files, home);
        (cs, after)
    }
    fn remove(files: &Files, home: &Path) -> (ChangeSet, Files) {
        let cs = remove_plan(
            &user_home(),
            files.hooks.as_deref(),
            files.settings.as_deref(),
            files.chain.as_deref(),
            None,
        )
        .unwrap();
        let after = apply(&cs, files, home);
        (cs, after)
    }
    fn only_settings(text: &str) -> Files {
        Files {
            settings: Some(text.to_string()),
            ..Files::default()
        }
    }
    fn only_hooks(text: &str) -> Files {
        Files {
            hooks: Some(text.to_string()),
            ..Files::default()
        }
    }
    fn command_of(entry: &Value) -> &str {
        entry["command"].as_str().unwrap()
    }

    #[test]
    fn the_files_are_agys() {
        assert_eq!(
            hooks_path(Path::new("/h")),
            Path::new("/h").join(".gemini").join("config").join("hooks.json")
        );
        assert_eq!(
            settings_path(Path::new("/h")),
            Path::new("/h")
                .join(".gemini")
                .join("antigravity-cli")
                .join("settings.json")
        );
    }

    #[test]
    fn install_into_missing_files_adds_the_hook_and_the_line() {
        let home = fake_home();
        let (cs, after) = install(&Files::default(), &BOTH, &home);
        assert_eq!(cs.title, "Set up Antigravity hooks");
        // hooks.json is created, not edited into existence.
        let hooks_edits: Vec<_> = cs
            .edits
            .iter()
            .filter(|e| e.path == hooks_file().to_string_lossy())
            .collect();
        assert_eq!(hooks_edits.len(), 1);
        assert!(matches!(hooks_edits[0].op, EditOp::CreateFile { .. }));

        let hooks = after.hooks_value();
        let hook = &hooks[HOOK_NAME];
        assert_eq!(hook["enabled"], true);
        assert_eq!(hook.as_object().unwrap().len(), 2 + STATUS_EVENTS.len());
        for event in std::iter::once(PRE_TOOL).chain(STATUS_EVENTS) {
            let list = hook[event].as_array().unwrap();
            assert_eq!(list.len(), 1, "{event}");
            let entry = if event == PRE_TOOL || event == "PostToolUse" {
                assert_eq!(list[0]["matcher"], ".*", "{event}");
                assert_eq!(list[0].as_object().unwrap().len(), 2, "{event}");
                &list[0]["hooks"][0]
            } else {
                &list[0]
            };
            assert_eq!(entry["type"], "command");
            let (role, timeout) = if event == PRE_TOOL {
                ("permission", 600)
            } else {
                ("event", 5)
            };
            assert_eq!(command_of(entry), hook_command(&home, role, event).unwrap(), "{event}");
            assert!(
                command_of(entry).ends_with(&format!(" {role} --agent antigravity --event {event} --tag mshl1")),
                "{}",
                command_of(entry)
            );
            assert_eq!(entry["timeout"], timeout, "{event}");
        }

        let line = &after.settings_value()["statusLine"];
        assert_eq!(line["type"], "command");
        assert_eq!(command_of(line), statusline_command(&home).unwrap());
        assert!(command_of(line).ends_with(" statusline --agent antigravity --tag mshl1"));
        assert_eq!(after.chain_value(), json!({ "statusLine": null }));
        for e in &cs.edits {
            assert_eq!(
                e.scope == ConfigScope::Marshell,
                e.path == chain_file(&home).to_string_lossy()
            );
        }
    }

    #[test]
    fn status_only_puts_an_event_entry_on_pre_tool_use() {
        let (_, after) = install(&Files::default(), &STATUS, &fake_home());
        let entry = &after.hooks_value()[HOOK_NAME][PRE_TOOL][0]["hooks"][0];
        assert_eq!(entry["timeout"], 5);
        assert!(command_of(entry).contains(" event --agent antigravity --event PreToolUse "));
        assert!(!command_of(entry).contains("permission"));
    }

    #[test]
    fn approve_without_status_installs_only_the_approve_entry() {
        let want = HookPlanRequest {
            status: false,
            statusline: false,
            approve: true,
            skill: false,
            distro: None,
        };
        let (cs, after) = install(&Files::default(), &want, &fake_home());
        let hook = &after.hooks_value()[HOOK_NAME];
        assert_eq!(hook.as_object().unwrap().len(), 2);
        assert_eq!(hook[PRE_TOOL][0]["hooks"][0]["timeout"], 600);
        assert!(after.settings.is_none() && after.chain.is_none());
        assert!(cs.edits.iter().all(|e| e.path == hooks_file().to_string_lossy()));
    }

    #[test]
    fn approve_off_without_status_turns_the_approve_entry_into_nothing_else() {
        let home = fake_home();
        let (_, installed) = install(&Files::default(), &BOTH, &home);
        // Status off and approve off: the status events stay, the approve entry goes.
        let want = HookPlanRequest {
            status: false,
            statusline: false,
            approve: false,
            skill: false,
            distro: None,
        };
        let (cs, after) = install(&installed, &want, &home);
        assert!(
            cs.reason.contains("Removes Marshell's PreToolUse approval hook"),
            "{}",
            cs.reason
        );
        let hook = &after.hooks_value()[HOOK_NAME];
        assert!(hook.get(PRE_TOOL).is_none());
        for event in STATUS_EVENTS {
            assert!(hook.get(event).is_some(), "{event}");
        }
        assert_eq!(after.settings, installed.settings);
    }

    #[test]
    fn install_keeps_the_users_own_named_hook() {
        for text in hooks_variants() {
            let (_, after) = install(&only_hooks(&text), &STATUS, &fake_home());
            let after = after.hooks.unwrap();
            let user_part = text.trim_end_matches(['\r', '\n']).trim_end_matches('}').trim_end();
            // The BOM, the line endings, the comments and the user's hook stay byte for byte; ours
            // follows after a comma.
            let user_part = user_part.trim_end_matches(',');
            assert!(after.starts_with(user_part), "{after:?}");
            assert_eq!(after.starts_with('\u{feff}'), text.starts_with('\u{feff}'));
            assert_eq!(
                after.matches("\r\n").count(),
                if text.contains("\r\n") {
                    after.matches('\n').count()
                } else {
                    0
                },
                "{after:?}"
            );
            let v = value(&after);
            assert_eq!(v["mine"], value(&text)["mine"]);
            assert!(v[HOOK_NAME].is_object());
        }
    }

    #[test]
    fn install_on_an_installed_file_changes_nothing() {
        for approve in [false, true] {
            let home = real_home();
            let want = HookPlanRequest { approve, ..STATUS };
            let (_, installed) = install(&Files::default(), &want, &home);
            let again = try_install(&installed, &want, &home).unwrap();
            assert!(again.edits.is_empty(), "approve={approve}: {:?}", again.edits);
            assert!(again.reason.contains("already in place"), "{}", again.reason);

            let found = inspect(
                Some(&installed.hooks_value()),
                Some(&installed.settings_value()),
                Some(&installed.chain_value()),
                &home,
            );
            assert_eq!(found.install, HookInstall::Installed, "approve={approve}");
            assert_eq!(found.approve_installed, approve);
            assert!(!found.has_own_statusline);
            assert!(!found.chained);
        }
    }

    #[test]
    fn inspect_sees_nothing_in_missing_files() {
        let found = inspect(None, None, None, &real_home());
        assert_eq!(found.install, HookInstall::None);
        assert!(!found.approve_installed);
    }

    #[test]
    fn inspect_calls_a_hook_without_a_line_partial() {
        let home = real_home();
        let (_, installed) = install(&Files::default(), &STATUS, &home);
        let mut hooks = installed.hooks_value();
        hooks[HOOK_NAME].as_object_mut().unwrap().shift_remove("Stop");
        let found = inspect(Some(&hooks), None, None, &home);
        assert_eq!(found.install, HookInstall::Partial);
    }

    #[test]
    fn an_entry_from_another_home_is_orphaned() {
        let elsewhere = fake_home();
        let (_, installed) = install(&Files::default(), &BOTH, &elsewhere);
        let here = real_home();
        let found = inspect(
            Some(&installed.hooks_value()),
            Some(&installed.settings_value()),
            Some(&installed.chain_value()),
            &here,
        );
        assert_eq!(found.install, HookInstall::Orphaned);
        assert!(!found.approve_installed);
        // The home whose bridge is gone is orphaned too.
        let (_, mine) = install(&Files::default(), &BOTH, &here);
        std::fs::remove_file(paths::bridge_exe(&here)).unwrap();
        let found = inspect(Some(&mine.hooks_value()), None, None, &here);
        assert_eq!(found.install, HookInstall::Orphaned);
    }

    #[test]
    fn reinstalling_from_another_home_points_every_entry_at_this_one() {
        let (_, old) = install(&Files::default(), &BOTH, &fake_home());
        let here = PathBuf::from("C:/Users/test user/.other");
        let (_, after) = install(&old, &BOTH, &here);
        assert_eq!(
            after.hooks_value()[HOOK_NAME],
            install(&Files::default(), &BOTH, &here).1.hooks_value()[HOOK_NAME]
        );
        assert_eq!(
            command_of(&after.settings_value()["statusLine"]),
            statusline_command(&here).unwrap()
        );
    }

    #[test]
    fn remove_with_only_ours_deletes_hooks_json() {
        let home = fake_home();
        let (_, installed) = install(&Files::default(), &BOTH, &home);
        let (cs, after) = remove(&installed, &home);
        assert_eq!(cs.title, "Turn Antigravity hooks off");
        assert!(after.hooks.is_none());
        let delete = cs
            .edits
            .iter()
            .find(|e| e.path == hooks_file().to_string_lossy())
            .unwrap();
        assert_eq!(delete.op, EditOp::DeleteFile);
        assert_eq!(
            delete.expected_hash.as_deref(),
            Some(hash_bytes(installed.hooks.as_deref().unwrap().as_bytes()).as_str())
        );
        assert!(cs.reason.contains("deletes hooks.json"), "{}", cs.reason);
        // No line of their own: the whole status line goes.
        assert!(after.settings_value().get("statusLine").is_none());
    }

    #[test]
    fn remove_with_a_users_hook_keeps_the_file() {
        for text in hooks_variants() {
            let home = fake_home();
            let (_, installed) = install(&only_hooks(&text), &STATUS, &home);
            let (cs, after) = remove(&installed, &home);
            assert!(cs.edits.iter().all(|e| e.op != EditOp::DeleteFile));
            assert_eq!(after.hooks.as_deref(), Some(text.as_str()), "{text:?}");
        }
    }

    #[test]
    fn remove_without_anything_of_ours_changes_nothing() {
        let files = Files {
            hooks: Some(plain_hooks()),
            settings: Some(fixture("user-hooks.json")),
            chain: None,
        };
        let (cs, after) = remove(&files, &fake_home());
        assert!(cs.edits.is_empty());
        assert_eq!(after.hooks, files.hooks);
        assert_eq!(after.settings, files.settings);
        let (cs, _) = remove(&Files::default(), &fake_home());
        assert!(cs.edits.is_empty());
    }

    #[test]
    fn the_users_status_line_is_chained_and_put_back() {
        let home = fake_home();
        let settings = fixture("user-hooks.json");
        let own = value(&settings)["statusLine"].clone();
        let (cs, installed) = install(&only_settings(&settings), &STATUS, &home);
        let line = &installed.settings_value()["statusLine"];
        assert_eq!(command_of(line), statusline_command(&home).unwrap());
        // Their padding stays, and their line is kept whole in the chain file.
        assert_eq!(line["padding"], 1);
        assert!(line.get("stack_with_default").is_none());
        assert_eq!(installed.chain_value(), json!({ "statusLine": own }));
        assert!(
            cs.reason.contains("passes your status line through Marshell"),
            "{}",
            cs.reason
        );

        let (cs, removed) = remove(&installed, &home);
        assert_eq!(removed.settings.as_deref(), Some(settings.as_str()));
        assert!(cs.reason.contains("puts your status line back"), "{}", cs.reason);
        // The chain file is read and never changed.
        assert!(cs.edits.iter().all(|e| e.path != chain_file(&home).to_string_lossy()));
    }

    #[test]
    fn their_stack_with_default_stays_theirs() {
        let home = fake_home();
        let settings = r#"{"statusLine":{"type":"command","command":"mine","stack_with_default":false,"padding":2}}"#;
        let (_, installed) = install(&only_settings(settings), &STATUS, &home);
        let line = &installed.settings_value()["statusLine"];
        assert_eq!(line["stack_with_default"], false);
        assert_eq!(line["padding"], 2);
        assert_eq!(installed.chain_value()["statusLine"]["command"], "mine");
    }

    #[test]
    fn a_status_line_changed_since_the_install_is_left_alone() {
        let home = fake_home();
        let (_, mut installed) = install(&only_settings(&fixture("user-hooks.json")), &STATUS, &home);
        let changed = r#"{"statusLine":{"type":"command","command":"python ./new.py"}}"#;
        installed.settings = Some(changed.to_string());
        let (cs, removed) = remove(&installed, &home);
        assert_eq!(removed.settings.as_deref(), Some(changed));
        assert!(cs.reason.contains("leaves your status line alone"), "{}", cs.reason);
        assert!(cs.edits.iter().all(|e| e.path != settings_file().to_string_lossy()));
    }

    #[test]
    fn with_no_line_of_their_own_the_line_stacks_with_agys() {
        let home = fake_home();
        for text in [None, Some(fixture("no-statusline.json"))] {
            let files = Files {
                settings: text,
                ..Files::default()
            };
            let (_, after) = install(&files, &STATUS, &home);
            let line = &after.settings_value()["statusLine"];
            assert_eq!(line["stack_with_default"], true);
            assert_eq!(command_of(line), statusline_command(&home).unwrap());
            assert_eq!(after.chain_value(), json!({ "statusLine": null }));
        }
    }

    #[test]
    fn removal_with_no_line_of_their_own_removes_the_line() {
        let home = fake_home();
        let settings = fixture("no-statusline.json");
        let (_, installed) = install(&only_settings(&settings), &STATUS, &home);
        let (cs, removed) = remove(&installed, &home);
        assert_eq!(removed.settings.as_deref(), Some(settings.as_str()));
        assert!(cs.reason.contains("removes the status line it added"), "{}", cs.reason);
    }

    #[test]
    fn a_missing_chain_removes_the_line_and_says_why() {
        let home = fake_home();
        let (_, mut installed) = install(&Files::default(), &STATUS, &home);
        installed.chain = None;
        let (cs, removed) = remove(&installed, &home);
        assert!(removed.settings_value().get("statusLine").is_none());
        assert!(cs.reason.contains("saved copy of yours is missing"), "{}", cs.reason);
    }

    #[test]
    fn a_line_of_ours_with_no_chain_gets_a_null_chain_and_stacks() {
        let home = fake_home();
        let (_, mut installed) = install(&Files::default(), &STATUS, &home);
        installed.chain = None;
        let mut settings = installed.settings_value();
        settings["statusLine"]
            .as_object_mut()
            .unwrap()
            .shift_remove("stack_with_default");
        installed.settings = Some(settings.to_string());
        let (_, after) = install(&installed, &STATUS, &home);
        assert_eq!(after.chain_value(), json!({ "statusLine": null }));
        assert_eq!(after.settings_value()["statusLine"]["stack_with_default"], true);
    }

    #[test]
    fn skip_all_in_settings_is_left_alone() {
        let home = fake_home();
        let settings = "{\r\n  \"dangerouslySkipPermissions\": true,\r\n  \"theme\": \"dark\"\r\n}\r\n";
        for want in [STATUS, BOTH] {
            let (_, installed) = install(&only_settings(settings), &want, &home);
            let after = installed.settings_value();
            assert_eq!(after["dangerouslySkipPermissions"], true);
            assert_eq!(after["theme"], "dark");
            let (_, removed) = remove(&installed, &home);
            assert_eq!(removed.settings.as_deref(), Some(settings));
        }
    }

    #[test]
    fn settings_bytes_survive_install_then_remove() {
        let crlf = fixture("crlf.json").replace('\n', "\r\n");
        let bom = format!("\u{feff}{}", fixture("bom.json"));
        let cases = [
            ("empty object", fixture("empty-object.json")),
            ("user hooks", fixture("user-hooks.json")),
            ("comments and trailing commas", fixture("comments-trailing-commas.json")),
            ("crlf", crlf),
            ("bom", bom),
            ("status line comment", fixture("statusline-comment.json")),
            ("no status line", fixture("no-statusline.json")),
        ];
        let home = fake_home();
        for (name, text) in cases {
            for want in [STATUS, BOTH] {
                let (_, installed) = install(&only_settings(&text), &want, &home);
                assert_ne!(installed.settings.as_deref(), Some(text.as_str()), "{name}");
                let (_, removed) = remove(&installed, &home);
                assert_eq!(removed.settings.as_deref(), Some(text.as_str()), "{name}");
            }
        }
    }

    #[test]
    fn hooks_bytes_survive_install_then_remove() {
        let home = fake_home();
        for text in hooks_variants() {
            for want in [STATUS, BOTH] {
                let (_, installed) = install(
                    &Files {
                        hooks: Some(text.clone()),
                        ..Files::default()
                    },
                    &want,
                    &home,
                );
                let (_, removed) = remove(&installed, &home);
                assert_eq!(removed.hooks.as_deref(), Some(text.as_str()), "{text:?}");
            }
        }
    }

    #[test]
    fn nothing_asked_for_changes_nothing() {
        let want = HookPlanRequest {
            status: false,
            statusline: false,
            approve: false,
            skill: false,
            distro: None,
        };
        let cs = try_install(&Files::default(), &want, &fake_home()).unwrap();
        assert!(cs.edits.is_empty());
        assert!(cs.reason.contains("Nothing was asked for"), "{}", cs.reason);
    }

    #[cfg(windows)]
    #[test]
    fn the_command_quotes_after_the_drive() {
        let home = PathBuf::from(r"C:\Users\Home Office\.marshell");
        assert_eq!(
            hook_command(&home, "permission", "PreToolUse").unwrap(),
            r#"C:"/Users/Home Office/.marshell/bin"/marshell-hook.exe permission --agent antigravity --event PreToolUse --tag mshl1"#
        );
        assert_eq!(
            statusline_command(&home).unwrap(),
            r#"C:"/Users/Home Office/.marshell/bin"/marshell-hook.exe statusline --agent antigravity --tag mshl1"#
        );
        // `& ( ) ; , = ^` are safe inside the quotes.
        let odd = PathBuf::from(r"D:\a&b (1);x,y=z^q\.marshell");
        assert_eq!(
            hook_command(&odd, "event", "Stop").unwrap(),
            r#"D:"/a&b (1);x,y=z^q/.marshell/bin"/marshell-hook.exe event --agent antigravity --event Stop --tag mshl1"#
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_verbatim_drive_prefix_is_dropped() {
        let home = PathBuf::from(r"\\?\C:\Users\Home Office\.marshell");
        assert_eq!(
            hook_command(&home, "event", "Stop").unwrap(),
            r#"C:"/Users/Home Office/.marshell/bin"/marshell-hook.exe event --agent antigravity --event Stop --tag mshl1"#
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_home_with_a_percent_is_refused() {
        let home = PathBuf::from(r"C:\Users\100%\.marshell");
        let err = hook_command(&home, "event", "Stop").unwrap_err();
        assert_eq!(
            err.to_string(),
            r"Antigravity can't run Marshell's hook from C:\Users\100%\.marshell\bin\marshell-hook.exe: the path holds a % or a quote"
        );
        assert!(matches!(err, PlanError::BridgePath { .. }));
        assert_eq!(try_install(&Files::default(), &STATUS, &home).unwrap_err(), err);
        assert_eq!(statusline_command(&home).unwrap_err(), err);
    }

    #[cfg(windows)]
    #[test]
    fn a_home_with_a_quote_is_refused() {
        let home = PathBuf::from("C:\\Users\\a\"b\\.marshell");
        let err = hook_command(&home, "event", "Stop").unwrap_err();
        assert!(err.to_string().ends_with(": the path holds a % or a quote"), "{err}");
        assert!(
            err.to_string()
                .starts_with("Antigravity can't run Marshell's hook from ")
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_unc_home_is_refused() {
        for home in [
            r"\\server\share\.marshell",
            r"\\?\UNC\server\share\.marshell",
            r"relative\.marshell",
        ] {
            let err = hook_command(Path::new(home), "event", "Stop").unwrap_err();
            let exe = paths::bridge_exe(Path::new(home));
            assert_eq!(
                err.to_string(),
                format!(
                    "Antigravity can't run Marshell's hook from {}: the path has no drive letter",
                    exe.display()
                ),
                "{home}"
            );
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn elsewhere_the_bridge_is_single_quoted() {
        let home = PathBuf::from("/home/it's me/.marshell");
        assert_eq!(
            hook_command(&home, "event", "Stop").unwrap(),
            "'/home/it'\\''s me/.marshell/bin/marshell-hook' event --agent antigravity --event Stop --tag mshl1"
        );
    }

    #[test]
    fn an_unparsable_file_is_refused_with_its_line() {
        let home = fake_home();
        let bad = "{\n  \"a\": 1,\n  \"b\": \n";
        let want_err = |path: PathBuf, err: PlanError| match err {
            PlanError::DoesNotParse { path: p, problem } => {
                assert_eq!(p, path);
                assert!(problem.0 >= 1);
            }
            other => panic!("{other:?}"),
        };
        for (files, path) in [(only_hooks(bad), hooks_file()), (only_settings(bad), settings_file())] {
            want_err(path.clone(), try_install(&files, &STATUS, &home).unwrap_err());
            want_err(
                path,
                remove_plan(
                    &user_home(),
                    files.hooks.as_deref(),
                    files.settings.as_deref(),
                    None,
                    None,
                )
                .unwrap_err(),
            );
        }
    }

    #[test]
    fn a_value_of_the_wrong_shape_is_refused() {
        let home = fake_home();
        let cases = [
            (only_hooks("[]"), hooks_file(), String::new()),
            (only_hooks(r#"{"marshell": []}"#), hooks_file(), "/marshell".to_string()),
            (only_settings("[]"), settings_file(), String::new()),
            (
                only_settings(r#"{"statusLine": "x"}"#),
                settings_file(),
                "/statusLine".to_string(),
            ),
        ];
        for (files, path, pointer) in cases {
            assert_eq!(
                try_install(&files, &STATUS, &home).unwrap_err(),
                PlanError::NotAnObject { path, pointer }
            );
        }
    }

    // ---- inside a WSL distro ----

    const LINUX_BRIDGE: &str = "/mnt/c/Users/x y/.marshell/bin/marshell-hook.exe";
    const THEIRS: &str = r#"{"statusLine":{"type":"command","command":"echo mine"}}"#;

    fn distro_bridge(path: &str) -> Bridge {
        Bridge::Distro {
            name: "Ubuntu".into(),
            path: path.into(),
        }
    }
    fn distro_user_home() -> PathBuf {
        PathBuf::from(r"\\wsl.localhost\Ubuntu\home\me")
    }
    fn distro_install(hooks: Option<&str>, settings: Option<&str>, want: &HookPlanRequest) -> ChangeSet {
        install_plan(
            &distro_user_home(),
            hooks,
            settings,
            None,
            None,
            &distro_bridge(LINUX_BRIDGE),
            want,
        )
        .unwrap()
    }
    /// What the edits set, by file name and pointer.
    fn set_values(cs: &ChangeSet) -> Vec<(String, String, Value)> {
        cs.edits
            .iter()
            .filter_map(|e| {
                let name = e.path.rsplit(['\\', '/']).next().unwrap().to_string();
                match &e.op {
                    EditOp::JsonSet { pointer, value } => {
                        Some((name, pointer.clone(), serde_json::from_str(value).unwrap()))
                    }
                    EditOp::CreateFile { content } => {
                        Some((name, String::new(), serde_json::from_str(content).unwrap()))
                    }
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn a_distro_plan_writes_the_linux_bridge() {
        let cs = distro_install(None, None, &BOTH);
        // A shell string for `sh`: the path in single quotes, then the rest of the command.
        let quoted = format!("'{LINUX_BRIDGE}'");
        let values = set_values(&cs);
        let hooks = values.iter().find(|(name, ..)| name == "hooks.json").unwrap();
        let commands: Vec<String> = entries_of(&hooks.2[HOOK_NAME])
            .iter()
            .map(|(_, e)| e["command"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(commands.len(), STATUS_EVENTS.len() + 1);
        for command in &commands {
            assert!(command.starts_with(&format!("{quoted} ")), "{command}");
        }
        assert!(
            commands
                .iter()
                .any(|c| c.contains(" permission --agent antigravity --event PreToolUse "))
        );
        let line = values.iter().find(|(_, pointer, _)| pointer == "/statusLine").unwrap();
        assert_eq!(
            line.2["command"],
            format!("{quoted} statusline --agent antigravity --tag mshl1 --no-chain")
        );
        // Both files are the distro's.
        assert!(
            cs.edits
                .iter()
                .all(|e| e.path.starts_with(r"\\wsl.localhost\Ubuntu\home\me"))
        );

        // A quote in the path is closed, escaped and opened again.
        let cs = install_plan(
            &distro_user_home(),
            None,
            None,
            None,
            None,
            &distro_bridge("/mnt/c/it's/marshell-hook.exe"),
            &STATUS,
        )
        .unwrap();
        let line = set_values(&cs)
            .into_iter()
            .find(|(_, pointer, _)| pointer == "/statusLine")
            .unwrap();
        assert!(
            line.2["command"]
                .as_str()
                .unwrap()
                .starts_with(r"'/mnt/c/it'\''s/marshell-hook.exe' statusline"),
            "{}",
            line.2["command"]
        );
    }

    #[test]
    fn a_distro_plan_leaves_an_existing_statusline() {
        for statusline in [true, false] {
            let cs = distro_install(None, Some(THEIRS), &HookPlanRequest { statusline, ..BOTH });
            assert!(
                set_values(&cs).iter().all(|(name, ..)| name == "hooks.json"),
                "statusline={statusline}: the person's status line is not touched"
            );
            assert!(cs.reason.ends_with(
                "Ubuntu already has a status line, so Marshell leaves it. The row won't show the model or context use."
            ));
        }
    }

    #[test]
    fn a_distro_plan_writes_no_chain_file() {
        // Nothing there, the person's own line, and a line of Marshell's from a Windows setup.
        let windows_line = statusline_command(&fake_home()).unwrap();
        let ours = json!({ "statusLine": { "type": "command", "command": windows_line } }).to_string();
        for settings in [None, Some(THEIRS.to_string()), Some(ours.clone())] {
            let cs = distro_install(None, settings.as_deref(), &BOTH);
            assert!(
                cs.edits.iter().all(|e| e.scope != ConfigScope::Marshell),
                "{settings:?}"
            );
            assert!(
                cs.edits.iter().all(|e| !e.path.contains("statusline-chain")),
                "{settings:?}"
            );
        }
        // The line from the Windows setup is pointed at the distro's bridge, and says it has no chain.
        let cs = distro_install(None, Some(&ours), &BOTH);
        let line = set_values(&cs)
            .into_iter()
            .find(|(_, pointer, _)| pointer == "/statusLine/command")
            .unwrap();
        assert!(line.2.as_str().unwrap().ends_with(" --no-chain"));
    }

    #[test]
    fn a_distro_remove_takes_out_its_own_line_and_leaves_the_persons() {
        let installed = distro_install(None, None, &BOTH);
        let line = set_values(&installed)
            .into_iter()
            .find(|(_, pointer, _)| pointer == "/statusLine")
            .unwrap()
            .2;
        let ours = json!({ "statusLine": line }).to_string();
        let cs = remove_plan(&distro_user_home(), None, Some(&ours), None, Some("Ubuntu")).unwrap();
        assert!(
            cs.edits
                .iter()
                .any(|e| matches!(&e.op, EditOp::JsonRemove { pointer } if pointer == "/statusLine"))
        );

        let cs = remove_plan(&distro_user_home(), None, Some(THEIRS), None, Some("Ubuntu")).unwrap();
        assert!(cs.edits.is_empty(), "{cs:?}");
    }
}
