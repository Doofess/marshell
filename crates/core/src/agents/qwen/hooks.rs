//! Qwen Code's hook entries and status line, planned as config changes in its user `settings.json` (PLAN §4, §6).
//! Nothing here touches a file: a plan is built from the text of the settings file and the chain file, and the
//! config service previews, snapshots and writes it. Every doubt fails closed: a file that does not parse, or a
//! `hooks` value of the wrong shape, gives an error and no edits.
//!
//! Qwen's hooks have Claude Code's layout (`hooks.<Event>[{matcher?, hooks:[{type, command, timeout}]}]`), and its
//! status line lives at `ui.statusLine`. The command string is the Windows `cmd` form (`common::cmd_form`), because
//! Qwen escapes `"` before it runs a hook through `cmd /d /s /c` (the 6l1 research note, Q2).

use crate::agents::common::{self, Bridge, PlanError, TAG, is_ours, statusline_left_note};
use crate::config::hash::hash_bytes;
use crate::config::jsonc::{parse_problem, read_value};
use crate::paths;
use marshell_protocol::agents::{HookInstall, HookPlanRequest};
use marshell_protocol::config::{Applies, ChangeSet, ConfigScope, EditOp, FileEdit};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub use crate::agents::claude::hooks::RemovePlan;

/// The events whose hooks carry live status, in the order they are installed. `PermissionRequest` is the approve
/// entry and comes after them.
pub const STATUS_EVENTS: [&str; 8] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

const AGENT: &str = "qwen";
const AGENT_NAME: &str = "Qwen Code";
const HOOK_TIMEOUT: u64 = 5;
const SESSION_END_TIMEOUT: u64 = 2;
/// The bridge waits 590 s for the person's answer.
const PERMISSION_TIMEOUT: u64 = 600;
const PERMISSION_EVENT: &str = "PermissionRequest";
const STATUSLINE_COMMAND_POINTER: &str = "/ui/statusLine/command";
const STATUSLINE_POINTER: &str = "/ui/statusLine";

/// `QWEN_HOME` when it is set and not blank, else `<user home>/.qwen`, joined with `settings.json`.
pub fn settings_path(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> PathBuf {
    super::home(env, user_home).join("settings.json")
}

// ---- the command ----

fn bridge_prefix_of(bridge: &Bridge) -> Result<String, PlanError> {
    common::bridge_prefix_of(AGENT_NAME, bridge)
}

fn command_of(bridge: &Bridge, role: &str) -> Result<String, PlanError> {
    Ok(format!(
        "{} {role} --agent {AGENT} --tag {TAG}",
        bridge_prefix_of(bridge)?
    ))
}

/// The command string of a hook entry: the bridge in the `cmd` form, then the role and the tag, unquoted.
pub fn hook_command(home: &Path, role: &str) -> Result<String, PlanError> {
    command_of(&Bridge::local(home), role)
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

fn role_of(event: &str) -> &'static str {
    if event == PERMISSION_EVENT {
        "permission"
    } else {
        "event"
    }
}

fn matcher_of(event: &str) -> Option<&'static str> {
    matches!(event, "PreToolUse" | "PostToolUse" | PERMISSION_EVENT).then_some(".*")
}

fn hook_entry_of(bridge: &Bridge, event: &str) -> Result<Value, PlanError> {
    Ok(json!({
        "type": "command",
        "command": command_of(bridge, role_of(event))?,
        "timeout": match event {
            PERMISSION_EVENT => PERMISSION_TIMEOUT,
            "SessionEnd" => SESSION_END_TIMEOUT,
            _ => HOOK_TIMEOUT,
        },
    }))
}

/// The group a plan appends to `event`: a `matcher` only where the table has one.
fn group_of(bridge: &Bridge, event: &str) -> Result<Value, PlanError> {
    let entry = hook_entry_of(bridge, event)?;
    Ok(match matcher_of(event) {
        Some(matcher) => json!({ "matcher": matcher, "hooks": [entry] }),
        None => json!({ "hooks": [entry] }),
    })
}

/// An RFC 6901 token.
fn esc(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

// ---- reading the settings ----

fn not_an_object(path: &Path, pointer: impl Into<String>) -> PlanError {
    PlanError::NotAnObject {
        path: path.to_path_buf(),
        pointer: pointer.into(),
    }
}

/// The settings as a value (a missing file is `{}`), checked for the shapes a plan relies on.
fn parse_settings(path: &Path, text: Option<&str>) -> Result<Value, PlanError> {
    let Some(text) = text else { return Ok(json!({})) };
    let Ok(settings) = read_value(text) else {
        let problem = parse_problem(text).unwrap_or((1, "The file does not parse.".to_string()));
        return Err(PlanError::DoesNotParse {
            path: path.to_path_buf(),
            problem,
        });
    };
    let Some(root) = settings.as_object() else {
        return Err(not_an_object(path, ""));
    };
    if let Some(ui) = root.get("ui") {
        let Some(ui) = ui.as_object() else {
            return Err(not_an_object(path, "/ui"));
        };
        if let Some(line) = ui.get("statusLine")
            && !line.is_null()
            && !line.is_object()
        {
            return Err(not_an_object(path, STATUSLINE_POINTER));
        }
    }
    if let Some(hooks) = root.get("hooks") {
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
    Ok(settings)
}

/// Every hook entry as (event, group, entry); anything of an unexpected shape is skipped.
fn entries_of(settings: &Value) -> impl Iterator<Item = (&str, &Value, &Value)> {
    settings
        .get("hooks")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(event, groups)| groups.as_array().map(|g| (event.as_str(), g)))
        .flat_map(|(event, groups)| {
            groups.iter().flat_map(move |group| {
                group
                    .get("hooks")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(move |entry| (event, group, entry))
            })
        })
}

/// How many of Marshell's entries the settings hold, in any event.
fn ours_total(settings: &Value) -> usize {
    entries_of(settings).filter(|(_, _, e)| is_ours(e)).count()
}

/// A group Marshell made whole: only `hooks`, and the `.*` matcher it gives the tool events.
fn is_our_group_shape(group: &Value) -> bool {
    group.as_object().is_some_and(|o| {
        o.iter()
            .all(|(k, v)| k == "hooks" || (k == "matcher" && v.as_str() == Some(".*")))
    })
}

/// The pointers that take Marshell's entries out, highest index first so none shifts another.
/// A group holding nothing else goes whole. With `prune`, an event left without groups goes, and
/// `/hooks` itself when every event in it went.
fn removal_pointers(settings: &Value, prune: bool) -> Vec<String> {
    let Some(hooks) = settings.get("hooks").and_then(Value::as_object) else {
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
            if ours.len() == entries.len() && is_our_group_shape(group) {
                ops.push(format!("{event_pointer}/{gi}"));
                groups_gone += 1;
            } else {
                ops.extend(ours.iter().rev().map(|ei| format!("{event_pointer}/{gi}/hooks/{ei}")));
            }
        }
        if !found && !(event == PERMISSION_EVENT && groups.is_empty()) {
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

/// What `ui.statusLine` is.
enum Line<'a> {
    /// No `ui`, or a `ui` with no `statusLine`: Qwen shows its default preset.
    Missing,
    /// `null`: no status line at all.
    Null,
    /// A command that is Marshell's.
    Ours(&'a Value),
    /// A command of the person's own.
    Theirs(&'a Value),
    /// An object that is not a command: a preset, which Marshell leaves as it is.
    Preset,
}

fn command_text(line: &Value) -> Option<&str> {
    line.get("command").and_then(Value::as_str)
}

/// A command that is Marshell's status line.
fn line_is_ours(line: &Value) -> bool {
    command_text(line).is_some_and(|c| c.contains(&format!("--tag {TAG}")))
}

fn line_of(settings: &Value) -> Line<'_> {
    match settings.get("ui").and_then(|ui| ui.get("statusLine")) {
        None => Line::Missing,
        Some(Value::Null) => Line::Null,
        Some(line) if line_is_ours(line) => Line::Ours(line),
        Some(line) if line.get("type").and_then(Value::as_str) == Some("command") && command_text(line).is_some() => {
            Line::Theirs(line)
        }
        Some(_) => Line::Preset,
    }
}

// ---- inspect ----

pub struct Found {
    pub install: HookInstall,
    /// The PermissionRequest hook is in place.
    pub approve_installed: bool,
    /// A status line command that is not Marshell's, or the one the chain keeps.
    pub has_own_statusline: bool,
    /// Ours is in place and the chain holds the user's.
    pub chained: bool,
    /// The status line is a preset (or missing, which is Qwen's default preset): Marshell leaves it as it is.
    pub preset_statusline: bool,
}

/// The chain file's value, `{"statusLine": ...}`, holds a status line command of the user's own.
fn chain_holds_users_line(chain: Option<&Value>) -> bool {
    chain
        .and_then(|c| c.get("statusLine"))
        .is_some_and(|s| s.is_object() && command_text(s).is_some() && !line_is_ours(s))
}

/// The chain can be forwarded to: it holds the user's line, or says they had none.
fn chain_usable(chain: Option<&Value>) -> bool {
    chain_holds_users_line(chain) || chain.and_then(|c| c.get("statusLine")).is_some_and(Value::is_null)
}

/// What the settings (and the chain file's value, `{"statusLine": ...}`) say about the install.
pub fn inspect(settings: &Value, chain: Option<&Value>, home: &Path) -> Found {
    let line = line_of(settings);
    let line_ours = matches!(line, Line::Ours(_));
    let chain_holds_users = chain_holds_users_line(chain);
    let chain_usable = chain_usable(chain);
    let line_current = match &line {
        Line::Ours(l) => statusline_command(home).ok().as_deref() == command_text(l),
        _ => false,
    };

    // A bridge path no command can be built from (a `%` in it) makes nothing exact.
    let bridge = Bridge::local(home);
    let exact_of = |event: &str| {
        let mut ours = entries_of(settings).filter(|(e, _, entry)| *e == event && is_ours(entry));
        match (ours.next(), ours.next(), hook_entry_of(&bridge, event)) {
            (Some((_, group, entry)), None, Ok(expected)) => {
                *entry == expected && group.get("matcher").and_then(Value::as_str) == matcher_of(event)
            }
            _ => false,
        }
    };
    let exact_events = STATUS_EVENTS.iter().filter(|event| exact_of(event)).count();
    let permission_exact = exact_of(PERMISSION_EVENT);
    let total = ours_total(settings);

    let install = if total == 0 && !line_ours {
        HookInstall::None
    } else if !paths::bridge_exe(home).exists() {
        HookInstall::Orphaned
    } else if exact_events == STATUS_EVENTS.len()
        && total == exact_events + usize::from(permission_exact)
        && (!line_ours || (chain_usable && line_current))
    {
        HookInstall::Installed
    } else {
        HookInstall::Partial
    };
    Found {
        install,
        approve_installed: permission_exact && paths::bridge_exe(home).exists(),
        has_own_statusline: matches!(line, Line::Theirs(_)) || (line_ours && chain_holds_users),
        chained: line_ours && chain_holds_users,
        preset_statusline: matches!(line, Line::Missing | Line::Preset),
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

fn chain_edit_of(path: &Path, op: EditOp, text: Option<&str>) -> FileEdit {
    FileEdit {
        scope: ConfigScope::Marshell,
        ..edit(path, op, text)
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

/// The plan for status hooks, the status line and optional approval. Status off leaves status alone.
///
/// Marshell's existing entries come out first (so a rerun never duplicates), then one group per event goes in at
/// the end, after the person's own. The status line, by what it is:
/// - a command of the person's own: the chain keeps the whole object, and only its `command` is replaced;
/// - already Marshell's: pointed at this home's bridge, and the chain is written (as null) only when it has
///   nothing usable;
/// - `null`: becomes Marshell's command, and the chain says the person had none;
/// - a preset, or missing (Qwen's default preset): left as it is, no chain.
///
/// `chain` is the chain file, `None` in a distro (see `Bridge::Distro`): there the chain is neither read nor
/// written, a status line that is not Marshell's stays as it is, and Marshell's own line says `--no-chain`.
pub fn install_plan(
    settings_path: &Path,
    _user_home: &Path,
    text: Option<&str>,
    chain_text: Option<&str>,
    chain: Option<&Path>,
    bridge: &Bridge,
    want: &HookPlanRequest,
) -> Result<ChangeSet, PlanError> {
    let title = "Set up Qwen Code hooks".to_string();
    let settings = parse_settings(settings_path, text)?;
    let mut ops: Vec<EditOp> = removal_pointers(&settings, false)
        .into_iter()
        .filter(|p| want.status || p.starts_with("/hooks/PermissionRequest/"))
        .map(remove_op)
        .collect();

    // Status arrays stay in place for the appends below. Only an approval array we no longer need is pruned.
    let permission_pointer = format!("/hooks/{PERMISSION_EVENT}");
    if !want.approve
        && removal_pointers(&settings, true)
            .iter()
            .any(|p| p == &permission_pointer || p == "/hooks")
    {
        ops.retain(|op| !matches!(op, EditOp::JsonRemove { pointer } if pointer.starts_with(&format!("{permission_pointer}/"))));
        if settings.pointer(&permission_pointer).is_some() {
            ops.push(remove_op(permission_pointer));
        }
    }

    let existing = settings.get("hooks").and_then(Value::as_object);
    let events = STATUS_EVENTS
        .into_iter()
        .filter(|_| want.status)
        .chain(want.approve.then_some(PERMISSION_EVENT));
    for event in events {
        let group = group_of(bridge, event)?;
        if existing.is_some_and(|h| h.contains_key(event)) {
            ops.push(set(format!("/hooks/{}/-", esc(event)), &group));
        } else {
            // Setting `/-` under a missing event would make an object with a "-" key.
            ops.push(set(format!("/hooks/{}", esc(event)), &json!([group])));
        }
    }

    let approval = if want.approve {
        "Adds the PermissionRequest hook for Approve from anywhere"
    } else if entries_of(&settings).any(|(event, _, entry)| event == PERMISSION_EVENT && is_ours(entry)) {
        "Removes Marshell's PermissionRequest hook"
    } else {
        ""
    };
    if !want.status {
        return Ok(ChangeSet {
            title,
            reason: if approval.is_empty() {
                "Nothing was asked for, so nothing changes.".to_string()
            } else {
                format!("{approval}.")
            },
            edits: ops.into_iter().map(|op| edit(settings_path, op, text)).collect(),
        });
    }

    let ours = statusline_of(bridge)?;
    let mut chain_edit = None;
    let how = format!(
        "Adds {} hook entries so Marshell can show what {AGENT_NAME} is doing",
        STATUS_EVENTS.len()
    );
    let mut tail = "";
    let mut left_alone = None;
    match line_of(&settings) {
        Line::Ours(line) => {
            // Ours already: the person's line is never read back from it. A line that runs another home's bridge
            // is pointed at this one, and a chain with nothing to forward to says the person had none.
            if command_text(line) != Some(&ours) {
                ops.push(set(STATUSLINE_COMMAND_POINTER, &json!(ours)));
            }
            let saved = chain_text.and_then(|t| read_value(t).ok());
            if chain.is_some() && !chain_usable(saved.as_ref()) {
                chain_edit = Some(set("/statusLine", &Value::Null));
                tail = ", and runs Marshell's status line on its own, because the saved copy of yours is missing";
            }
        }
        Line::Theirs(_) if chain.is_none() => left_alone = bridge.distro().map(statusline_left_note),
        Line::Theirs(line) if want.statusline => {
            // Only the command is replaced, so their `refreshInterval`, `respectUserColors` and the rest stay.
            ops.push(set(STATUSLINE_COMMAND_POINTER, &json!(ours)));
            chain_edit = Some(set("/statusLine", line));
            tail = ", and passes your status line through Marshell, which runs it as before";
        }
        Line::Theirs(_) => {}
        Line::Null => {
            ops.push(set(STATUSLINE_POINTER, &json!({ "type": "command", "command": ours })));
            chain_edit = Some(set("/statusLine", &Value::Null));
            tail = ", and adds a status line so the model and context use arrive";
        }
        Line::Missing | Line::Preset => {
            tail = ", and leaves your status line as it is, because it is a preset";
        }
    }

    let mut edits: Vec<FileEdit> = ops.into_iter().map(|op| edit(settings_path, op, text)).collect();
    if let (Some(op), Some(chain)) = (chain_edit, chain) {
        edits.push(chain_edit_of(chain, op, chain_text));
    }
    let mut reason = if approval.is_empty() {
        format!("{how}{tail}.")
    } else {
        format!("{how}{tail}. {approval}.")
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
        Some(line) if line.is_object() && !line_is_ours(line) => match command_text(line) {
            Some(command) => Saved::Command(command.to_string()),
            None => Saved::Unusable,
        },
        _ => Saved::Unusable,
    }
}

/// The plan that takes Marshell's entries out and puts the user's status line back.
///
/// The chain file is read and never changed. While `ui.statusLine.command` is Marshell's, the chain's command goes
/// back in that key (or `null` back as `ui.statusLine`); a status line the person changed since the install is
/// left alone.
///
/// In a distro (`distro` is its name) there is no chain: a status line of Marshell's was added over a `null`,
/// and goes back to `null`, and any other is the person's, left alone.
pub fn remove_plan(
    settings_path: &Path,
    _user_home: &Path,
    text: Option<&str>,
    chain_text: Option<&str>,
    distro: Option<&str>,
) -> Result<RemovePlan, PlanError> {
    let title = "Turn Qwen Code hooks off".to_string();
    let settings = parse_settings(settings_path, text)?;
    let mut ops: Vec<EditOp> = removal_pointers(&settings, true).into_iter().map(remove_op).collect();
    let removed = ours_total(&settings);

    let mut statusline_left = false;
    let mut line_note = "";
    match line_of(&settings) {
        Line::Ours(_) => match saved_line(chain_text, distro.is_some()) {
            Saved::Command(command) => {
                ops.push(set(STATUSLINE_COMMAND_POINTER, &json!(command)));
                line_note = "puts your status line back";
            }
            Saved::Nothing => {
                ops.push(set(STATUSLINE_POINTER, &Value::Null));
                line_note = "puts back the status line setting of null that it replaced";
            }
            Saved::Unusable => {
                ops.push(remove_op(STATUSLINE_POINTER.to_string()));
                line_note = "removes its status line, because the saved copy of yours is missing and there is nothing to put back";
            }
        },
        Line::Theirs(_) | Line::Preset if chain_text.is_some() || distro.is_some() => {
            statusline_left = true;
            line_note = if distro.is_some() {
                "leaves your status line alone, because it is not Marshell's"
            } else {
                "leaves your status line alone, because it was changed since the setup"
            };
        }
        _ => {}
    }

    if ops.is_empty() {
        return Ok(RemovePlan {
            changes: ChangeSet {
                title,
                reason: "Marshell has no hook entries or status line in this file, so nothing changes.".to_string(),
                edits: Vec::new(),
            },
            statusline_left,
        });
    }
    let entries = match removed {
        0 => String::new(),
        1 => "Removes the hook entry Marshell added".to_string(),
        n => format!("Removes the {n} hook entries Marshell added"),
    };
    let reason = match (entries.is_empty(), line_note.is_empty()) {
        (false, false) => format!("{entries} and {line_note}."),
        (false, true) => format!("{entries}."),
        (true, true) => "Removes the empty PermissionRequest hook list left by an earlier Marshell.".to_string(),
        (true, _) => format!("It {line_note}."),
    };
    Ok(RemovePlan {
        changes: ChangeSet {
            title,
            reason,
            edits: ops.into_iter().map(|op| edit(settings_path, op, text)).collect(),
        },
        statusline_left,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::jsonc::{JsonOp, apply_ops};

    const HOME: &str = "C:/Users/test user/.marshell";
    const BOTH: HookPlanRequest = HookPlanRequest {
        status: true,
        statusline: true,
        approve: false,
        skill: false,
        distro: None,
    };

    fn home() -> PathBuf {
        PathBuf::from(HOME)
    }
    fn user_home() -> PathBuf {
        PathBuf::from("C:/Users/test user")
    }
    fn sfile() -> PathBuf {
        PathBuf::from("C:/Users/test user/.qwen/settings.json")
    }
    fn chain_path() -> PathBuf {
        paths::statusline_chain(&home(), "qwen")
    }
    fn ours_statusline() -> String {
        statusline_command(&home()).unwrap()
    }
    /// The entry a plan appends for `event`.
    fn entry(event: &str) -> Value {
        hook_entry_of(&Bridge::local(&home()), event).unwrap()
    }
    fn group(event: &str) -> Value {
        group_of(&Bridge::local(&home()), event).unwrap()
    }

    /// The two files a plan touches, as text; `None` is a missing file.
    #[derive(Clone, Default)]
    struct Files {
        settings: Option<String>,
        chain: Option<String>,
    }
    impl Files {
        fn settings(text: &str) -> Self {
            Files {
                settings: Some(text.to_string()),
                chain: None,
            }
        }
        fn settings_value(&self) -> Value {
            read_value(self.settings.as_deref().unwrap()).unwrap()
        }
        fn chain_value(&self) -> Value {
            read_value(self.chain.as_deref().unwrap()).unwrap()
        }
    }

    /// Applies a ChangeSet the way `preview` does: per path, in order, a missing file starting as
    /// `{}`. Every edit must carry the hash of the text the plan was built from.
    fn apply(cs: &ChangeSet, before: &Files) -> Files {
        let mut after = before.clone();
        for e in &cs.edits {
            assert_eq!(e.applies, Applies::NewSessions);
            let is_settings = e.path == sfile().to_string_lossy();
            let is_chain = e.path == chain_path().to_string_lossy();
            assert!(is_settings || is_chain, "unexpected path {}", e.path);
            assert_eq!(e.scope == ConfigScope::Marshell, is_chain, "scope of {}", e.path);
            if is_settings {
                assert_eq!(e.scope, ConfigScope::User);
            }
            let original = if is_settings { &before.settings } else { &before.chain };
            assert_eq!(
                e.expected_hash,
                original.as_ref().map(|t| hash_bytes(t.as_bytes())),
                "expected_hash of {}",
                e.path
            );
            let slot = if is_settings {
                &mut after.settings
            } else {
                &mut after.chain
            };
            let next = match &e.op {
                EditOp::JsonSet { pointer, value } => {
                    let v: Value = serde_json::from_str(value).unwrap();
                    apply_ops(slot.as_deref().unwrap_or("{}\n"), &[JsonOp::Set { pointer, value: &v }]).unwrap()
                }
                EditOp::JsonRemove { pointer } => match slot {
                    None => continue,
                    Some(text) => apply_ops(text, &[JsonOp::Remove { pointer }]).unwrap(),
                },
                other => panic!("unexpected op {other:?}"),
            };
            *slot = Some(next);
        }
        after
    }

    fn try_install(files: &Files, want: HookPlanRequest, home: &Path) -> Result<ChangeSet, PlanError> {
        install_plan(
            &sfile(),
            &user_home(),
            files.settings.as_deref(),
            files.chain.as_deref(),
            Some(&paths::statusline_chain(home, AGENT)),
            &Bridge::local(home),
            &want,
        )
    }
    fn install(files: &Files, want: HookPlanRequest) -> (ChangeSet, Files) {
        let cs = try_install(files, want, &home()).unwrap();
        let after = apply(&cs, files);
        (cs, after)
    }
    fn remove(files: &Files) -> (RemovePlan, Files) {
        let plan = remove_plan(
            &sfile(),
            &user_home(),
            files.settings.as_deref(),
            files.chain.as_deref(),
            None,
        )
        .unwrap();
        let after = apply(&plan.changes, files);
        (plan, after)
    }
    fn ours_count(v: &Value) -> usize {
        entries_of(v).filter(|(_, _, e)| is_ours(e)).count()
    }
    fn scratch_home(with_bridge: bool) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        if with_bridge {
            let exe = paths::bridge_exe(&home);
            std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
            std::fs::write(exe, b"").unwrap();
        }
        (dir, home)
    }

    const USER_GROUP: &str =
        r#"{ "matcher": "run_shell_command", "hooks": [{ "type": "command", "command": "python ./guard.py" }] }"#;
    const THEIR_LINE: &str =
        r#"{ "type": "command", "command": "python ./line.py", "refreshInterval": 5, "respectUserColors": true }"#;

    /// `text` as it reads inside a JSON string (the `cmd` form holds quotes).
    fn json_escaped(text: &str) -> String {
        let quoted = serde_json::to_string(text).unwrap();
        quoted[1..quoted.len() - 1].to_string()
    }

    fn with_ui(line: &str) -> String {
        format!(
            "{{\n  \"$version\": 4,\n  \"ui\": {{\n    \"theme\": \"dark\",\n    \"statusLine\": {line}\n  }}\n}}\n"
        )
    }
    fn user_hooks() -> String {
        format!(
            "{{\n  \"$version\": 4,\n  \"ui\": {{\n    \"theme\": \"dark\"\n  }},\n  \"hooks\": {{\n    \"PreToolUse\": [\n      {USER_GROUP}\n    ]\n  }}\n}}\n"
        )
    }
    /// Every shape of settings file the byte-identity tests walk.
    fn cases() -> Vec<(&'static str, String)> {
        let plain = user_hooks();
        vec![
            ("empty object", "{}\n".to_string()),
            ("version key only", "{\n  \"$version\": 4\n}\n".to_string()),
            ("user hooks", plain.clone()),
            ("bom", format!("\u{feff}{plain}")),
            ("crlf", plain.replace('\n', "\r\n")),
            (
                "comments and trailing commas",
                format!(
                    "{{\n  // my hooks\n  \"$version\": 4, /* qwen's */\n  \"hooks\": {{\n    \"Stop\": [{{\"hooks\":[{{\"type\":\"command\",\"command\":\"echo mine\"}}]}},], /* mine */\n  }},\n  \"ui\": {{\n    \"statusLine\": {THEIR_LINE}, // my bar\n  }},\n}}\n"
                ),
            ),
            ("their command status line", with_ui(THEIR_LINE)),
            (
                "their command status line, crlf",
                with_ui(THEIR_LINE).replace('\n', "\r\n"),
            ),
            (
                "a preset status line",
                with_ui(r#"{ "type": "preset", "items": ["model", "context"] }"#),
            ),
            ("a null status line", with_ui("null")),
            (
                "ui with only a theme",
                "{\n  \"ui\": {\n    \"theme\": \"dark\"\n  }\n}\n".to_string(),
            ),
        ]
    }

    // ---- paths and commands ----

    #[test]
    fn the_settings_path_follows_qwen_home() {
        let user = user_home();
        let env = |k: &str| (k == "QWEN_HOME").then(|| "D:/q/home".to_string());
        assert_eq!(
            settings_path(&env, &user),
            PathBuf::from("D:/q/home").join("settings.json")
        );
        assert_eq!(
            settings_path(&|_| None, &user),
            user.join(".qwen").join("settings.json")
        );
    }

    #[test]
    fn a_blank_qwen_home_is_ignored() {
        let user = user_home();
        for blank in ["", "   "] {
            let env = |k: &str| (k == "QWEN_HOME").then(|| blank.to_string());
            assert_eq!(
                settings_path(&env, &user),
                user.join(".qwen").join("settings.json"),
                "{blank:?}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn the_hook_command_uses_the_cmd_form() {
        let home = PathBuf::from(r"C:\Users\Home Office\.marshell");
        assert_eq!(
            hook_command(&home, "event").unwrap(),
            r#"C:"/Users/Home Office/.marshell/bin"/marshell-hook.exe event --agent qwen --tag mshl1"#
        );
        assert_eq!(
            hook_command(&home, "permission").unwrap(),
            r#"C:"/Users/Home Office/.marshell/bin"/marshell-hook.exe permission --agent qwen --tag mshl1"#
        );
        assert_eq!(
            statusline_command(&home).unwrap(),
            r#"C:"/Users/Home Office/.marshell/bin"/marshell-hook.exe statusline --agent qwen --tag mshl1"#
        );
    }

    #[test]
    fn the_hook_command_ends_with_the_role_agent_and_tag() {
        let command = hook_command(&home(), "event").unwrap();
        assert!(command.ends_with(" event --agent qwen --tag mshl1"), "{command}");
        assert!(!command.contains(" \"event"), "{command}");
        assert!(ours_statusline().ends_with(" statusline --agent qwen --tag mshl1"));
    }

    #[cfg(windows)]
    #[test]
    fn a_bridge_path_with_a_quote_or_percent_is_refused_naming_qwen_code() {
        let home = PathBuf::from(r"C:\Users\100%\.marshell");
        let err = hook_command(&home, "event").unwrap_err();
        assert_eq!(
            err.to_string(),
            r"Qwen Code can't run Marshell's hook from C:\Users\100%\.marshell\bin\marshell-hook.exe: the path holds a % or a quote"
        );
        assert!(matches!(err, PlanError::BridgePath { .. }));
        assert_eq!(try_install(&Files::default(), BOTH, &home).unwrap_err(), err);
        assert_eq!(statusline_command(&home).unwrap_err(), err);

        let quoted = PathBuf::from("C:\\Users\\a\"b\\.marshell");
        let err = hook_command(&quoted, "event").unwrap_err();
        assert!(err.to_string().starts_with("Qwen Code can't run Marshell's hook from "));
        assert!(err.to_string().ends_with(": the path holds a % or a quote"), "{err}");

        let unc = hook_command(Path::new(r"\\server\share\.marshell"), "event").unwrap_err();
        assert!(unc.to_string().ends_with(": the path has no drive letter"), "{unc}");
    }

    // ---- install ----

    #[test]
    fn install_into_a_missing_file_writes_every_event() {
        let (cs, after) = install(&Files::default(), BOTH);
        assert_eq!(cs.title, "Set up Qwen Code hooks");
        let v = after.settings_value();
        let events: Vec<&str> = v["hooks"].as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(events, STATUS_EVENTS);
        for event in STATUS_EVENTS {
            let groups = v["hooks"][event].as_array().unwrap();
            assert_eq!(groups.len(), 1, "{event}");
            let tool = matches!(event, "PreToolUse" | "PostToolUse");
            assert_eq!(groups[0].get("matcher").is_some(), tool, "{event} matcher");
            if tool {
                assert_eq!(groups[0]["matcher"], ".*");
            }
            let hook = &groups[0]["hooks"][0];
            assert_eq!(hook["type"], "command");
            assert_eq!(hook["timeout"], if event == "SessionEnd" { 2 } else { 5 }, "{event}");
            assert_eq!(hook["command"], hook_command(&home(), "event").unwrap().as_str());
            assert_eq!(groups[0], group(event), "{event}");
        }
        // Qwen's default preset is left as it is: no ui.statusLine, no chain.
        assert!(v.get("ui").is_none());
        assert!(after.chain.is_none());
    }

    #[test]
    fn install_with_approve_adds_permission_request_at_600() {
        let (_, after) = install(&Files::default(), HookPlanRequest { approve: true, ..BOTH });
        let v = after.settings_value();
        let permission = &v["hooks"]["PermissionRequest"];
        assert_eq!(permission, &json!([group("PermissionRequest")]));
        assert_eq!(permission[0]["matcher"], ".*");
        assert_eq!(permission[0]["hooks"][0]["timeout"], 600);
        assert_eq!(
            permission[0]["hooks"][0]["command"],
            hook_command(&home(), "permission").unwrap().as_str()
        );
        let (_, again) = install(&after, HookPlanRequest { approve: true, ..BOTH });
        assert_eq!(again.settings_value(), after.settings_value());

        // Without approve the entry goes.
        let (_, off) = install(&after, BOTH);
        let off = off.settings_value();
        assert_eq!(ours_count(&off), STATUS_EVENTS.len());
        assert!(off.pointer("/hooks/PermissionRequest").is_none());
    }

    #[test]
    fn approvals_off_then_hooks_off_leaves_no_empty_permission_request() {
        let (_, on) = install(&Files::default(), HookPlanRequest { approve: true, ..BOTH });
        let (_, approvals_off) = install(&on, BOTH);
        assert!(
            approvals_off
                .settings_value()
                .pointer("/hooks/PermissionRequest")
                .is_none()
        );
        let (_, hooks_off) = remove(&approvals_off);
        assert!(hooks_off.settings_value().get("hooks").is_none());
    }

    #[test]
    fn an_empty_permission_request_left_by_an_older_marshell_is_removed_with_the_hooks() {
        let before = Files::settings(r#"{"hooks":{"PermissionRequest":[],"Stop":[]}}"#);
        let (_, after) = remove(&before);
        let value = after.settings_value();
        assert!(value.pointer("/hooks/PermissionRequest").is_none());
        assert_eq!(value.pointer("/hooks/Stop"), Some(&json!([])));
        let (_, after) = remove(&Files::settings(r#"{"hooks":{"PermissionRequest":[]}}"#));
        assert!(after.settings_value().get("hooks").is_none());
    }

    #[test]
    fn a_persons_non_empty_permission_request_group_stays() {
        let text = format!(r#"{{"hooks":{{"PermissionRequest":[{USER_GROUP}]}}}}"#);
        let before = Files::settings(&text);
        let (_, on) = install(&before, HookPlanRequest { approve: true, ..BOTH });
        let (_, approvals_off) = install(&on, BOTH);
        assert_eq!(
            approvals_off.settings_value()["hooks"]["PermissionRequest"],
            json!([read_value(USER_GROUP).unwrap()])
        );
        let (_, hooks_off) = remove(&approvals_off);
        assert_eq!(hooks_off.settings_value(), before.settings_value());
    }

    #[test]
    fn install_keeps_the_persons_own_hooks_and_bytes() {
        for (name, text) in cases() {
            let (_, after) = install(&Files::settings(&text), BOTH);
            let got = after.settings.unwrap();
            assert_ne!(got, text, "{name}: install changed nothing");
            assert_eq!(got.starts_with('\u{feff}'), text.starts_with('\u{feff}'), "{name}: BOM");
            if text.contains("\r\n") {
                assert!(!got.replace("\r\n", "").contains('\n'), "{name}: a lone LF");
            }
            for needle in [
                "// my hooks",
                "/* mine */",
                "// my bar",
                "\"$version\": 4",
                "\"theme\": \"dark\"",
            ] {
                if text.contains(needle) {
                    assert!(got.contains(needle), "{name}: lost {needle}");
                }
            }
            let v = read_value(&got).unwrap();
            if text.contains("python ./guard.py") {
                let groups = v["hooks"]["PreToolUse"].as_array().unwrap();
                assert_eq!(groups.len(), 2, "{name}");
                assert_eq!(
                    groups[0],
                    read_value(USER_GROUP).unwrap(),
                    "{name}: the person's group is first"
                );
                assert_eq!(groups[1], group("PreToolUse"), "{name}: ours is appended after");
            }
            assert_eq!(ours_count(&v), STATUS_EVENTS.len(), "{name}");
            // The exact original bytes come back out.
            let (_, removed) = remove(&Files {
                settings: Some(got),
                chain: after.chain,
            });
            assert_eq!(removed.settings.as_deref(), Some(text.as_str()), "{name}");
        }
    }

    #[test]
    fn install_keeps_qwens_version_key() {
        let text = "{\n  \"$version\": 4,\n  \"general\": {\n    \"enableAutoUpdate\": false\n  }\n}\n";
        let (_, after) = install(&Files::settings(text), BOTH);
        let got = after.settings.as_deref().unwrap();
        assert!(got.starts_with("{\n  \"$version\": 4,\n  \"general\": {\n    \"enableAutoUpdate\": false\n  }"));
        assert_eq!(after.settings_value()["$version"], 4);
        let (_, removed) = remove(&after);
        assert_eq!(removed.settings.as_deref(), Some(text));
    }

    #[test]
    fn install_chains_their_command_status_line() {
        let text = with_ui(THEIR_LINE);
        let (cs, after) = install(&Files::settings(&text), BOTH);
        let v = after.settings_value();
        let line = &v["ui"]["statusLine"];
        assert_eq!(line["command"], ours_statusline().as_str());
        // Their other keys stay, byte for byte.
        assert_eq!(line["type"], "command");
        assert_eq!(line["refreshInterval"], 5);
        assert_eq!(line["respectUserColors"], true);
        assert_eq!(v["ui"]["theme"], "dark");
        let got = after.settings.as_deref().unwrap();
        assert!(
            got.contains(r#""refreshInterval": 5, "respectUserColors": true }"#),
            "{got}"
        );
        // The chain holds their whole object under `statusLine`, which is where the bridge reads `/statusLine/command`.
        assert_eq!(
            after.chain_value(),
            json!({ "statusLine": read_value(THEIR_LINE).unwrap() })
        );
        assert_eq!(
            after.chain_value().pointer("/statusLine/command").unwrap(),
            "python ./line.py"
        );
        // Only the command is replaced in the settings.
        let pointers: Vec<String> = cs
            .edits
            .iter()
            .filter(|e| e.path == sfile().to_string_lossy())
            .filter_map(|e| match &e.op {
                EditOp::JsonSet { pointer, .. } => Some(pointer.clone()),
                _ => None,
            })
            .collect();
        assert!(pointers.iter().any(|p| p == "/ui/statusLine/command"), "{pointers:?}");
        assert!(!pointers.iter().any(|p| p == "/ui/statusLine"), "{pointers:?}");
        assert!(
            cs.reason.contains("passes your status line through Marshell"),
            "{}",
            cs.reason
        );

        let (_, home_check) = (0, scratch_home(true));
        let found = inspect(&after.settings_value(), Some(&after.chain_value()), &home_check.1);
        assert!(found.chained && found.has_own_statusline && !found.preset_statusline);
    }

    #[test]
    fn install_without_the_statusline_choice_leaves_their_command_alone() {
        let text = with_ui(THEIR_LINE);
        let (_, after) = install(
            &Files::settings(&text),
            HookPlanRequest {
                statusline: false,
                ..BOTH
            },
        );
        assert_eq!(
            after.settings_value()["ui"]["statusLine"],
            read_value(THEIR_LINE).unwrap()
        );
        assert!(after.chain.is_none());
    }

    #[test]
    fn install_leaves_a_preset_status_line() {
        let preset = r#"{ "type": "preset", "items": ["model", "context"] }"#;
        let text = with_ui(preset);
        let (cs, after) = install(&Files::settings(&text), BOTH);
        let v = after.settings_value();
        assert_eq!(v["ui"]["statusLine"], read_value(preset).unwrap());
        assert!(after.chain.is_none(), "no chain for a preset");
        assert!(!cs.edits.iter().any(|e| e.path == chain_path().to_string_lossy()));
        assert_eq!(ours_count(&v), STATUS_EVENTS.len());
        let (_keep, home) = scratch_home(true);
        let found = inspect(&v, None, &home);
        assert!(found.preset_statusline && !found.has_own_statusline && !found.chained);
    }

    #[test]
    fn install_leaves_the_default_preset_when_there_is_no_status_line() {
        let only_theme = "{\n  \"ui\": {\n    \"theme\": \"dark\"\n  }\n}\n";
        for text in ["{\n  \"$version\": 4\n}\n", only_theme] {
            let (_, after) = install(&Files::settings(text), BOTH);
            let v = after.settings_value();
            assert!(v.pointer("/ui/statusLine").is_none(), "{text}");
            assert_eq!(v.pointer("/ui/theme").is_some(), text == only_theme);
            assert!(after.chain.is_none(), "{text}");
            let (_keep, home) = scratch_home(true);
            assert!(inspect(&v, None, &home).preset_statusline);
        }
    }

    #[test]
    fn install_over_a_null_status_line_restores_null() {
        let text = with_ui("null");
        let (_, after) = install(&Files::settings(&text), BOTH);
        let v = after.settings_value();
        assert_eq!(
            v["ui"]["statusLine"],
            json!({ "type": "command", "command": ours_statusline() })
        );
        assert_eq!(after.chain_value(), json!({ "statusLine": null }));
        let (_, removed) = remove(&after);
        assert_eq!(removed.settings.as_deref(), Some(text.as_str()));
        assert_eq!(removed.settings_value()["ui"]["statusLine"], Value::Null);
    }

    #[test]
    fn reinstall_replaces_entries_in_place() {
        let text = user_hooks();
        let (_, once) = install(&Files::settings(&text), HookPlanRequest { approve: true, ..BOTH });
        let (_, twice) = install(&once, HookPlanRequest { approve: true, ..BOTH });
        assert_eq!(twice.settings_value(), once.settings_value());
        assert_eq!(twice.settings, once.settings);
        assert_eq!(ours_count(&twice.settings_value()), STATUS_EVENTS.len() + 1);

        // Over their command line: nothing is saved twice, and the chain keeps their line, not ours.
        let (_, chained) = install(&Files::settings(&with_ui(THEIR_LINE)), BOTH);
        let (cs, again) = install(&chained, BOTH);
        assert!(
            !cs.edits.iter().any(|e| e.path == chain_path().to_string_lossy()),
            "nothing saved"
        );
        assert_eq!(again.chain, chained.chain);
        assert_eq!(again.settings_value(), chained.settings_value());
    }

    #[test]
    fn reinstall_repairs_a_lost_chain_and_repoints_a_line_from_another_home() {
        let (_, chained) = install(&Files::settings(&with_ui(THEIR_LINE)), BOTH);
        let lost = Files {
            settings: chained.settings.clone(),
            chain: None,
        };
        let (_, repaired) = install(&lost, BOTH);
        assert_eq!(repaired.chain_value(), json!({ "statusLine": null }));
        assert_eq!(repaired.settings_value(), chained.settings_value());

        let elsewhere = chained.settings.as_deref().unwrap().replace(
            &json_escaped(&ours_statusline()),
            "other-bridge statusline --agent qwen --tag mshl1",
        );
        assert_ne!(elsewhere, chained.settings.clone().unwrap());
        let (_, pointed) = install(
            &Files {
                settings: Some(elsewhere),
                chain: chained.chain.clone(),
            },
            BOTH,
        );
        assert_eq!(
            pointed.settings_value()["ui"]["statusLine"]["command"],
            ours_statusline().as_str()
        );
    }

    // ---- removal ----

    #[test]
    fn removal_restores_the_original_bytes() {
        for (name, text) in cases() {
            for approve in [false, true] {
                let (_, installed) = install(&Files::settings(&text), HookPlanRequest { approve, ..BOTH });
                assert_ne!(
                    installed.settings.as_deref(),
                    Some(text.as_str()),
                    "{name}: install changed nothing"
                );
                let (plan, removed) = remove(&installed);
                assert_eq!(
                    removed.settings.as_deref(),
                    Some(text.as_str()),
                    "{name}, approve={approve}"
                );
                assert!(!plan.statusline_left, "{name}");
                assert_eq!(plan.changes.title, "Turn Qwen Code hooks off");
                // The chain is read and never changed.
                assert_eq!(removed.chain, installed.chain, "{name}");
            }
        }
    }

    #[test]
    fn removal_leaves_a_status_line_the_person_changed() {
        let (_, installed) = install(&Files::settings(&with_ui(THEIR_LINE)), BOTH);
        let changed = installed
            .settings
            .as_deref()
            .unwrap()
            .replace(&json_escaped(&ours_statusline()), "python ./newer.py");
        let before = Files {
            settings: Some(changed),
            chain: installed.chain.clone(),
        };
        let (plan, after) = remove(&before);
        assert!(plan.statusline_left);
        let v = after.settings_value();
        assert_eq!(v["ui"]["statusLine"]["command"], "python ./newer.py");
        assert_eq!(ours_count(&v), 0);
        assert!(v.get("hooks").is_none());
        assert_eq!(after.chain, installed.chain);
        assert!(
            plan.changes.reason.contains("leaves your status line alone"),
            "{}",
            plan.changes.reason
        );
    }

    #[test]
    fn removal_with_a_lost_chain_removes_the_line_it_added() {
        let (_, installed) = install(&Files::settings(&with_ui("null")), BOTH);
        let lost = Files {
            settings: installed.settings.clone(),
            chain: None,
        };
        let (plan, after) = remove(&lost);
        assert!(!plan.statusline_left);
        assert!(after.settings_value()["ui"].get("statusLine").is_none());
    }

    #[test]
    fn removal_keeps_the_persons_own_hooks_and_prunes_what_it_added() {
        let text = user_hooks();
        let (_, installed) = install(&Files::settings(&text), BOTH);
        let (_, removed) = remove(&installed);
        assert_eq!(removed.settings.as_deref(), Some(text.as_str()));

        let own_in_our_event = r#"{"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "echo mine"}]}]}}"#;
        let (_, installed) = install(&Files::settings(own_in_our_event), BOTH);
        let (_, removed) = remove(&installed);
        assert_eq!(removed.settings_value(), read_value(own_in_our_event).unwrap());

        // A group the person gave their own entry beside ours loses only ours.
        let mixed = json!({"hooks": {"Stop": [{"hooks": [
            {"type": "command", "command": "echo mine"},
            entry("Stop"),
        ]}]}})
        .to_string();
        let (_, removed) = remove(&Files::settings(&mixed));
        assert_eq!(
            removed.settings_value()["hooks"]["Stop"],
            json!([{"hooks": [{"type": "command", "command": "echo mine"}]}])
        );
    }

    #[test]
    fn remove_on_a_missing_or_clean_file_has_no_edits() {
        let (plan, _) = remove(&Files::default());
        assert!(plan.changes.edits.is_empty());
        let (plan, _) = remove(&Files::settings(&user_hooks()));
        assert!(plan.changes.edits.is_empty());
        assert!(!plan.statusline_left);
        // The person's preset with no chain: nothing to leave, nothing to put back.
        let (plan, _) = remove(&Files::settings(&with_ui(r#"{ "type": "preset" }"#)));
        assert!(plan.changes.edits.is_empty());
        assert!(!plan.statusline_left);
    }

    // ---- inspect ----

    #[test]
    fn inspect_reports_installed_partial_orphaned_and_none() {
        let (_keep, home) = scratch_home(true);
        let install_for = |text: &str, approve: bool| {
            let before = Files::settings(text);
            let cs = try_install(&before, HookPlanRequest { approve, ..BOTH }, &home).unwrap();
            // Plans for this home are written to its own chain path, so apply through it.
            let mut files = before.clone();
            for e in &cs.edits {
                let slot = if e.scope == ConfigScope::Marshell {
                    &mut files.chain
                } else {
                    &mut files.settings
                };
                let EditOp::JsonSet { pointer, value } = &e.op else {
                    let EditOp::JsonRemove { pointer } = &e.op else {
                        panic!()
                    };
                    *slot = Some(apply_ops(slot.as_deref().unwrap(), &[JsonOp::Remove { pointer }]).unwrap());
                    continue;
                };
                let v: Value = serde_json::from_str(value).unwrap();
                *slot =
                    Some(apply_ops(slot.as_deref().unwrap_or("{}\n"), &[JsonOp::Set { pointer, value: &v }]).unwrap());
            }
            files
        };

        let none = json!({ "ui": { "theme": "dark" } });
        let found = inspect(&none, None, &home);
        assert_eq!(found.install, HookInstall::None);
        assert!(!found.approve_installed && !found.chained);

        let files = install_for(&with_ui(THEIR_LINE), true);
        let (settings, chain) = (files.settings_value(), files.chain_value());
        let found = inspect(&settings, Some(&chain), &home);
        assert_eq!(found.install, HookInstall::Installed);
        assert!(found.approve_installed && found.chained && found.has_own_statusline);

        let files = install_for(&with_ui("null"), false);
        let found = inspect(&files.settings_value(), Some(&files.chain_value()), &home);
        assert_eq!(found.install, HookInstall::Installed);
        assert!(!found.approve_installed && !found.chained && !found.has_own_statusline);

        // An event missing is Partial; so is a status line with no chain to forward to.
        let mut partial = files.settings_value();
        partial["hooks"].as_object_mut().unwrap().shift_remove("StopFailure");
        assert_eq!(
            inspect(&partial, Some(&files.chain_value()), &home).install,
            HookInstall::Partial
        );
        assert_eq!(
            inspect(&files.settings_value(), None, &home).install,
            HookInstall::Partial
        );

        // A changed timeout, or a doubled entry, is not exact.
        let mut changed = files.settings_value();
        changed["hooks"]["Stop"][0]["hooks"][0]["timeout"] = json!(9);
        assert_eq!(
            inspect(&changed, Some(&files.chain_value()), &home).install,
            HookInstall::Partial
        );
        let mut doubled = files.settings_value();
        let again = doubled["hooks"]["Stop"][0].clone();
        doubled["hooks"]["Stop"].as_array_mut().unwrap().push(again);
        assert_eq!(
            inspect(&doubled, Some(&files.chain_value()), &home).install,
            HookInstall::Partial
        );

        // The bridge not existing is Orphaned.
        let (_gone, no_bridge) = scratch_home(false);
        let orphan = inspect(&files.settings_value(), Some(&files.chain_value()), &no_bridge);
        assert_eq!(orphan.install, HookInstall::Orphaned);
        assert!(!orphan.approve_installed);
    }

    // ---- fail closed ----

    #[test]
    fn a_file_that_does_not_parse_plans_nothing() {
        let broken = Files::settings("{ \"hooks\": ");
        let err = try_install(&broken, BOTH, &home()).unwrap_err();
        assert!(matches!(err, PlanError::DoesNotParse { .. }), "{err}");
        assert!(remove_plan(&sfile(), &user_home(), broken.settings.as_deref(), None, None).is_err());
        for text in [
            "[]",
            "\"x\"",
            "{ \"ui\": [] }",
            "{ \"ui\": { \"statusLine\": \"echo\" } }",
        ] {
            assert!(try_install(&Files::settings(text), BOTH, &home()).is_err(), "{text}");
        }
    }

    #[test]
    fn a_hooks_value_of_the_wrong_shape_plans_nothing() {
        for text in [
            r#"{ "hooks": [] }"#,
            r#"{ "hooks": { "PreToolUse": {} } }"#,
            r#"{ "hooks": { "PreToolUse": [{ "hooks": {} }] } }"#,
        ] {
            let err = try_install(&Files::settings(text), BOTH, &home()).unwrap_err();
            assert!(matches!(err, PlanError::NotAnObject { .. }), "{text}: {err}");
            assert!(
                remove_plan(&sfile(), &user_home(), Some(text), None, None).is_err(),
                "{text}"
            );
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
        let cs = try_install(&Files::default(), want, &home()).unwrap();
        assert!(cs.edits.is_empty());
        assert!(cs.reason.contains("Nothing was asked for"), "{}", cs.reason);
    }

    // ---- inside a WSL distro ----

    #[test]
    fn a_distro_plan_keeps_no_chain_and_leaves_their_line() {
        let bridge = Bridge::Distro {
            name: "Ubuntu".into(),
            path: "/mnt/c/Users/x y/.marshell/bin/marshell-hook.exe".into(),
        };
        let settings = PathBuf::from(r"\\wsl.localhost\Ubuntu\home\me\.qwen\settings.json");
        let plan = |text: &str| {
            install_plan(
                &settings,
                Path::new(r"\\wsl.localhost\Ubuntu\home\me"),
                Some(text),
                None,
                None,
                &bridge,
                &BOTH,
            )
            .unwrap()
        };
        let cs = plan(&with_ui(THEIR_LINE));
        assert!(cs.edits.iter().all(|e| !e.path.contains("statusline-chain")));
        assert!(
            cs.edits
                .iter()
                .all(|e| !matches!(&e.op, EditOp::JsonSet { pointer, .. } if pointer.starts_with("/ui"))),
            "their line is not touched"
        );
        assert!(cs.reason.contains("Ubuntu already has a status line"), "{}", cs.reason);
        let cs = plan(&with_ui("null"));
        let line = cs
            .edits
            .iter()
            .find_map(|e| match &e.op {
                EditOp::JsonSet { pointer, value } if pointer == "/ui/statusLine" => Some(value.clone()),
                _ => None,
            })
            .unwrap();
        assert!(
            line.contains("statusline --agent qwen --tag mshl1 --no-chain"),
            "{line}"
        );
        assert!(
            line.contains("'/mnt/c/Users/x y/.marshell/bin/marshell-hook.exe'"),
            "{line}"
        );
    }
}
