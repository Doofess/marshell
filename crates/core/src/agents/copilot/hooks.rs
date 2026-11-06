//! Copilot CLI's hook entries and status line. Marshell writes three files and no others: its own hooks file
//! `<copilot home>/hooks/marshell.json` (the whole file), the `statusLine` key of `<copilot home>/settings.json`, and
//! the chain file that keeps the person's own status line. Nothing here touches a file: a plan is built from the
//! text of the three files, and the config service previews, snapshots and writes it. Every doubt fails closed.

use crate::agents::common::{self, Bridge, PlanError, TAG, chain_delete, is_ours};
use crate::config::hash::hash_bytes;
use crate::config::jsonc::{parse_problem, read_value};
use marshell_protocol::agents::{HookInstall, HookPlanRequest};
use marshell_protocol::config::{Applies, ChangeSet, ConfigScope, EditOp, FileEdit};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub use crate::agents::claude::hooks::RemovePlan;

/// Each hook event, the bridge role it runs and its `timeoutSec`, in the order they are written. There is no
/// `permissionRequest` entry: Copilot asks in its own terminal.
pub const EVENTS: [(&str, &str, u32); 9] = [
    ("sessionStart", "event", 5),
    ("userPromptSubmitted", "event", 5),
    ("preToolUse", "event", 5),
    ("postToolUse", "event", 5),
    ("postToolUseFailure", "event", 5),
    ("notification", "event", 5),
    ("agentStop", "event", 5),
    ("errorOccurred", "event", 5),
    ("sessionEnd", "event", 2),
];

/// `<copilot home>/hooks/marshell.json`.
pub fn hooks_path(copilot_home: &Path) -> PathBuf {
    copilot_home.join("hooks").join("marshell.json")
}

/// `<copilot home>/settings.json`.
pub fn settings_path(copilot_home: &Path) -> PathBuf {
    copilot_home.join("settings.json")
}

const AGENT: &str = "copilot";
const AGENT_NAME: &str = "Copilot CLI";
const STATUSLINE_POINTER: &str = "/statusLine";
const STATUSLINE_COMMAND_POINTER: &str = "/statusLine/command";

// ---- the entries ----

/// One hook entry: Copilot runs `exec` with `args` and no shell.
fn entry_of(bridge: &Bridge, event: &str, role: &str, timeout: u32) -> Value {
    json!({
        "type": "command",
        "exec": bridge.path(),
        "args": [role, "--agent", AGENT, "--event", event, "--tag", TAG],
        "timeoutSec": timeout,
    })
}

/// The whole text of `marshell.json`. With `want.status` false it holds no entries. `want.approve` changes nothing:
/// Copilot asks in its own terminal, so there is no permission entry.
pub fn hooks_file(bridge: &Bridge, want: &HookPlanRequest) -> String {
    let mut hooks = serde_json::Map::new();
    if want.status {
        for (event, role, timeout) in EVENTS {
            hooks.insert(event.to_string(), json!([entry_of(bridge, event, role, timeout)]));
        }
    }
    let mut text =
        serde_json::to_string_pretty(&json!({ "version": 1, "hooks": hooks })).expect("a JSON value always serializes");
    text.push('\n');
    text
}

/// The command string of Marshell's status line.
pub fn statusline_command(bridge: &Bridge) -> Result<String, PlanError> {
    Ok(format!(
        "{} statusline --agent {AGENT} --tag {TAG}",
        common::bridge_prefix_of(AGENT_NAME, bridge)?
    ))
}

/// The text of the three files a plan reads; `None` is a missing file.
pub struct Files<'a> {
    pub hooks_path: &'a Path,
    pub hooks_text: Option<&'a str>,
    pub settings_path: &'a Path,
    pub settings_text: Option<&'a str>,
    pub chain_path: &'a Path,
    pub chain_text: Option<&'a str>,
}

// ---- reading the files ----

fn does_not_parse(path: &Path, text: &str) -> PlanError {
    PlanError::DoesNotParse {
        path: path.to_path_buf(),
        problem: parse_problem(text).unwrap_or((1, "The file does not parse.".to_string())),
    }
}

/// Every hook entry of a hooks file as (event, entry); anything of an unexpected shape is skipped.
fn entries_of(file: &Value) -> impl Iterator<Item = (&str, &Value)> {
    file.get("hooks")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(event, entries)| entries.as_array().map(|e| (event.as_str(), e)))
        .flat_map(|(event, entries)| entries.iter().map(move |entry| (event, entry)))
}

/// The file is Marshell's: it has a `hooks` object, and every event holds only entries Marshell wrote.
fn is_marshells(file: &Value) -> bool {
    file.get("hooks").and_then(Value::as_object).is_some_and(|hooks| {
        hooks
            .values()
            .all(|entries| entries.as_array().is_some_and(|e| e.iter().all(is_ours)))
    })
}

/// The hooks file's text when it is there and Marshell's. A file that doesn't parse, or isn't Marshell's, is an error.
fn marshells_hooks<'a>(files: &Files<'a>) -> Result<Option<&'a str>, PlanError> {
    let Some(text) = files.hooks_text else { return Ok(None) };
    let Ok(file) = read_value(text) else {
        return Err(does_not_parse(files.hooks_path, text));
    };
    if !is_marshells(&file) {
        return Err(PlanError::NotMarshells {
            path: files.hooks_path.to_path_buf(),
        });
    }
    Ok(Some(text))
}

/// The settings as a value (a missing file is `{}`). Only a file that doesn't parse, or isn't an object, is refused.
fn parse_settings(path: &Path, text: Option<&str>) -> Result<Value, PlanError> {
    let Some(text) = text else { return Ok(json!({})) };
    let Ok(settings) = read_value(text) else {
        return Err(does_not_parse(path, text));
    };
    if !settings.is_object() {
        return Err(PlanError::NotAnObject {
            path: path.to_path_buf(),
            pointer: String::new(),
        });
    }
    Ok(settings)
}

/// What `statusLine` is.
enum Line<'a> {
    /// No `statusLine` key.
    Missing,
    /// `null`.
    Null,
    /// A command that is Marshell's.
    Ours(&'a Value),
    /// A command of the person's own.
    Theirs(&'a Value),
    /// Anything else (no string `command`, or not an object): left as it is.
    Other,
}

fn command_text(line: &Value) -> Option<&str> {
    line.get("command").and_then(Value::as_str)
}

fn line_is_ours(line: &Value) -> bool {
    command_text(line).is_some_and(|c| c.contains(&format!("--tag {TAG}")))
}

fn line_of(settings: &Value) -> Line<'_> {
    match settings.get("statusLine") {
        None => Line::Missing,
        Some(Value::Null) => Line::Null,
        Some(line) if line.is_object() && line_is_ours(line) => Line::Ours(line),
        Some(line) if line.is_object() && command_text(line).is_some() => Line::Theirs(line),
        Some(_) => Line::Other,
    }
}

/// The chain's `statusLine` is a command of the person's own.
fn chain_holds_theirs(chain: Option<&Value>) -> bool {
    chain
        .and_then(|c| c.get("statusLine"))
        .is_some_and(|s| s.is_object() && command_text(s).is_some() && !line_is_ours(s))
}

// ---- inspect ----

pub struct Found {
    pub install: HookInstall,
    /// A status line command that is not Marshell's, or the one the chain keeps.
    pub has_own_statusline: bool,
    /// Ours is in place and the chain holds the person's.
    pub chained: bool,
}

/// What the three files say about the install, for the bridge a setup would use. Text that doesn't parse reads as
/// holding nothing.
pub fn inspect(files: &Files, bridge: &Bridge) -> Found {
    let hooks = files.hooks_text.and_then(|t| read_value(t).ok());
    let settings = files
        .settings_text
        .and_then(|t| read_value(t).ok())
        .unwrap_or_else(|| json!({}));
    let chain = files.chain_text.and_then(|t| read_value(t).ok());
    let line = line_of(&settings);
    let line_ours = matches!(line, Line::Ours(_));
    let holds_theirs = chain_holds_theirs(chain.as_ref());
    let line_current = match &line {
        Line::Ours(l) => statusline_command(bridge).ok().as_deref() == command_text(l),
        _ => false,
    };

    let (all, ours) = hooks.as_ref().map_or((0, 0), |file| {
        let ours = entries_of(file).filter(|(_, e)| is_ours(e)).count();
        (entries_of(file).count(), ours)
    });
    let exact_events = hooks.as_ref().map_or(0, |file| {
        EVENTS
            .iter()
            .filter(|(event, role, timeout)| {
                let mut found = entries_of(file).filter(|(e, entry)| e == event && is_ours(entry));
                match (found.next(), found.next()) {
                    (Some((_, entry)), None) => *entry == entry_of(bridge, event, role, *timeout),
                    _ => false,
                }
            })
            .count()
    });

    let install = if ours == 0 && !line_ours {
        HookInstall::None
    } else if !bridge.exists() {
        HookInstall::Orphaned
    } else if exact_events == EVENTS.len()
        && all == EVENTS.len()
        && ours == EVENTS.len()
        // A line that isn't Marshell's is the person's to keep: a setup with the status line off leaves it (as Qwen's).
        && (!line_ours || line_current)
    {
        HookInstall::Installed
    } else {
        HookInstall::Partial
    };
    Found {
        install,
        has_own_statusline: matches!(line, Line::Theirs(_)) || (line_ours && holds_theirs),
        chained: line_ours && holds_theirs,
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

fn set(pointer: &str, value: &Value) -> EditOp {
    EditOp::JsonSet {
        pointer: pointer.to_string(),
        value: value.to_string(),
    }
}

/// The plan for Marshell's hooks file and the status line. Nothing is written when the hooks file isn't Marshell's
/// or either file doesn't parse.
///
/// The hooks file is made, replaced against its hash when it differs, or (with `want.status` false) removed. The
/// status line, with `want.status` and `want.statusline` both on, by what it is:
/// - a command of the person's own: the chain keeps the whole object, and only its `command` is replaced;
/// - already Marshell's: pointed at this bridge, and nothing is saved;
/// - missing or `null`: becomes Marshell's command, and the chain says the person had none;
/// - anything else: left as it is, no chain.
pub fn install_plan(files: &Files, bridge: &Bridge, want: &HookPlanRequest) -> Result<ChangeSet, PlanError> {
    let title = "Set up Copilot CLI hooks".to_string();
    let existing = marshells_hooks(files)?;
    let hooks_edit = match (existing, want.status.then(|| hooks_file(bridge, want))) {
        (None, None) => None,
        (None, Some(content)) => Some(edit(
            files.hooks_path,
            ConfigScope::User,
            EditOp::CreateFile { content },
            None,
        )),
        (Some(text), Some(content)) if text == content => None,
        (Some(text), Some(content)) => Some(edit(
            files.hooks_path,
            ConfigScope::User,
            EditOp::ReplaceText { content },
            Some(text),
        )),
        (Some(text), None) => Some(edit(
            files.hooks_path,
            ConfigScope::User,
            EditOp::DeleteFile,
            Some(text),
        )),
    };
    let mut edits: Vec<FileEdit> = hooks_edit.into_iter().collect();
    let mut how = match edits.first().map(|e| &e.op) {
        Some(EditOp::DeleteFile) => "Removes Marshell's hooks file".to_string(),
        Some(_) => format!(
            "Writes Marshell's own hooks file with {} entries so Marshell can show what {AGENT_NAME} is doing",
            EVENTS.len()
        ),
        None => String::new(),
    };

    if want.status && want.statusline {
        let settings = parse_settings(files.settings_path, files.settings_text)?;
        let ours = statusline_command(bridge)?;
        let (ops, chain, tail): (Vec<EditOp>, Option<Value>, &str) = match line_of(&settings) {
            Line::Ours(line) if command_text(line) != Some(&ours) => (
                vec![set(STATUSLINE_COMMAND_POINTER, &json!(ours))],
                None,
                "points your status line's Marshell command at this bridge",
            ),
            Line::Theirs(line) => (
                vec![set(STATUSLINE_COMMAND_POINTER, &json!(ours))],
                Some(line.clone()),
                "passes your status line through Marshell, which runs it as before",
            ),
            Line::Missing | Line::Null => (
                vec![set(STATUSLINE_POINTER, &json!({ "type": "command", "command": ours }))],
                Some(Value::Null),
                "adds a status line so the model and context use arrive",
            ),
            Line::Ours(_) | Line::Other => (Vec::new(), None, ""),
        };
        edits.extend(
            ops.into_iter()
                .map(|op| edit(files.settings_path, ConfigScope::User, op, files.settings_text)),
        );
        if let Some(chain) = chain {
            edits.push(edit(
                files.chain_path,
                ConfigScope::Marshell,
                set(STATUSLINE_POINTER, &chain),
                files.chain_text,
            ));
        }
        if !tail.is_empty() {
            how = if how.is_empty() {
                format!("It {tail}")
            } else {
                format!("{how}, and {tail}")
            };
        }
    }

    let reason = if how.is_empty() {
        if want.status {
            "Marshell's hooks are already in place, so nothing changes.".to_string()
        } else {
            "Nothing was asked for, so nothing changes.".to_string()
        }
    } else {
        format!("{how}.")
    };
    Ok(ChangeSet { title, reason, edits })
}

/// What the chain file holds for the status line.
enum Saved {
    /// The person's command.
    Command(String),
    /// The person had no status line.
    Nothing,
    /// Missing, unreadable, or naming Marshell's own command.
    Unusable,
}

fn saved_line(chain_text: Option<&str>) -> Saved {
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

/// The plan that deletes Marshell's hooks file and puts the person's status line back.
///
/// The chain file is read, then deleted in the same change set, so one undo brings back all three files. While
/// `statusLine.command` is Marshell's, the chain's command goes back in that key (or the key goes, when the chain
/// holds `null`); a command the person changed since the setup is left alone. `settings.json` is never deleted.
pub fn remove_plan(files: &Files) -> Result<RemovePlan, PlanError> {
    let title = "Turn Copilot CLI hooks off".to_string();
    let hooks = marshells_hooks(files)?;
    let settings = parse_settings(files.settings_path, files.settings_text)?;

    let mut statusline_left = false;
    let mut line_note = "";
    let mut ops: Vec<EditOp> = Vec::new();
    match line_of(&settings) {
        Line::Ours(_) => match saved_line(files.chain_text) {
            Saved::Command(command) => {
                ops.push(set(STATUSLINE_COMMAND_POINTER, &json!(command)));
                line_note = "puts your status line back";
            }
            Saved::Nothing => {
                ops.push(EditOp::JsonRemove {
                    pointer: STATUSLINE_POINTER.to_string(),
                });
                line_note = "removes the status line it added";
            }
            Saved::Unusable => {
                ops.push(EditOp::JsonRemove {
                    pointer: STATUSLINE_POINTER.to_string(),
                });
                line_note = "removes its status line, because the saved copy of yours is missing and there is nothing to put back";
            }
        },
        Line::Theirs(_) | Line::Other if files.chain_text.is_some() => {
            statusline_left = true;
            line_note = "leaves your status line alone, because it was changed since the setup";
        }
        _ => {}
    }

    let mut edits: Vec<FileEdit> = hooks
        .map(|text| edit(files.hooks_path, ConfigScope::User, EditOp::DeleteFile, Some(text)))
        .into_iter()
        .collect();
    let hooks_gone = !edits.is_empty();
    edits.extend(
        ops.into_iter()
            .map(|op| edit(files.settings_path, ConfigScope::User, op, files.settings_text)),
    );
    let chain_edit = chain_delete(Some(files.chain_path), files.chain_text, None);
    if edits.is_empty() && chain_edit.is_none() {
        return Ok(RemovePlan {
            changes: ChangeSet {
                title,
                reason: "Marshell has no hooks file or status line here, so nothing changes.".to_string(),
                edits,
            },
            statusline_left,
        });
    }
    let mut reason = match (hooks_gone, line_note.is_empty()) {
        (true, false) => format!("Deletes Marshell's hooks file and {line_note}."),
        (true, true) => "Deletes Marshell's hooks file.".to_string(),
        (false, false) => format!("It {line_note}."),
        (false, true) => String::new(),
    };
    if let Some(chain_edit) = chain_edit {
        edits.push(chain_edit);
        let note = "Deletes the saved copy of the status line in Marshell's own folder.";
        reason = if reason.is_empty() {
            note.to_string()
        } else {
            format!("{reason} {note}")
        };
    }
    Ok(RemovePlan {
        changes: ChangeSet { title, reason, edits },
        statusline_left,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::jsonc::{JsonOp, apply_ops};

    const BRIDGE_PATH: &str = "C:/Users/Jane Doe/.marshell/bin/marshell-hook.exe";
    const BOTH: HookPlanRequest = HookPlanRequest {
        status: true,
        statusline: true,
        approve: false,
        skill: false,
        distro: None,
    };

    fn bridge() -> Bridge {
        Bridge::Local(PathBuf::from(BRIDGE_PATH))
    }
    fn copilot_home() -> PathBuf {
        PathBuf::from("C:/Users/Jane Doe/.copilot")
    }
    fn chain_path() -> PathBuf {
        PathBuf::from("C:/Users/Jane Doe/.marshell/agents/copilot/statusline-chain.json")
    }
    fn ours_line() -> String {
        statusline_command(&bridge()).unwrap()
    }
    fn json_escaped(text: &str) -> String {
        let quoted = serde_json::to_string(text).unwrap();
        quoted[1..quoted.len() - 1].to_string()
    }
    fn value(text: &str) -> Value {
        read_value(text).unwrap()
    }

    /// The three files, as text; `None` is a missing file.
    #[derive(Clone, Default)]
    struct State {
        hooks: Option<String>,
        settings: Option<String>,
        chain: Option<String>,
    }
    impl State {
        fn settings(text: &str) -> Self {
            State {
                settings: Some(text.to_string()),
                ..State::default()
            }
        }
    }

    /// Plans against `state`, with the three files at their usual paths.
    fn with_files<T>(state: &State, f: impl FnOnce(&Files) -> T) -> T {
        let (hp, sp, cp) = (
            hooks_path(&copilot_home()),
            settings_path(&copilot_home()),
            chain_path(),
        );
        f(&Files {
            hooks_path: &hp,
            hooks_text: state.hooks.as_deref(),
            settings_path: &sp,
            settings_text: state.settings.as_deref(),
            chain_path: &cp,
            chain_text: state.chain.as_deref(),
        })
    }
    fn try_install(state: &State, b: &Bridge, want: &HookPlanRequest) -> Result<ChangeSet, PlanError> {
        with_files(state, |files| install_plan(files, b, want))
    }
    fn install_with(state: &State, b: &Bridge, want: &HookPlanRequest) -> (ChangeSet, State) {
        let cs = try_install(state, b, want).unwrap();
        let after = apply(&cs, state);
        (cs, after)
    }
    fn install(state: &State) -> (ChangeSet, State) {
        install_with(state, &bridge(), &BOTH)
    }
    fn remove(state: &State) -> (RemovePlan, State) {
        let plan = with_files(state, remove_plan).unwrap();
        let after = apply(&plan.changes, state);
        (plan, after)
    }

    /// Applies a ChangeSet the way the config service does: per path, in order, a missing file starting as `{}`.
    /// Every edit must carry the hash of the text the plan was built from.
    fn apply(cs: &ChangeSet, before: &State) -> State {
        let hooks = hooks_path(&copilot_home()).to_string_lossy().into_owned();
        let settings = settings_path(&copilot_home()).to_string_lossy().into_owned();
        let chain = chain_path().to_string_lossy().into_owned();
        let mut after = before.clone();
        for e in &cs.edits {
            assert_eq!(e.applies, Applies::NewSessions);
            let (original, slot, scope) = if e.path == hooks {
                (&before.hooks, &mut after.hooks, ConfigScope::User)
            } else if e.path == settings {
                (&before.settings, &mut after.settings, ConfigScope::User)
            } else if e.path == chain {
                (&before.chain, &mut after.chain, ConfigScope::Marshell)
            } else {
                panic!("unexpected path {}", e.path)
            };
            assert_eq!(e.scope, scope, "scope of {}", e.path);
            assert_eq!(
                e.expected_hash,
                original.as_ref().map(|t| hash_bytes(t.as_bytes())),
                "expected_hash of {}",
                e.path
            );
            match &e.op {
                EditOp::CreateFile { content } => {
                    assert!(slot.is_none(), "CreateFile over {}", e.path);
                    *slot = Some(content.clone());
                }
                EditOp::ReplaceText { content } => {
                    assert!(slot.is_some() && e.expected_hash.is_some(), "ReplaceText of {}", e.path);
                    *slot = Some(content.clone());
                }
                EditOp::DeleteFile => {
                    assert!(slot.is_some() && e.expected_hash.is_some(), "DeleteFile of {}", e.path);
                    *slot = None;
                }
                EditOp::JsonSet { pointer, value } => {
                    let v: Value = serde_json::from_str(value).unwrap();
                    let text = slot.as_deref().unwrap_or("{}\n");
                    *slot = Some(apply_ops(text, &[JsonOp::Set { pointer, value: &v }]).unwrap());
                }
                EditOp::JsonRemove { pointer } => {
                    if let Some(text) = slot.as_deref() {
                        *slot = Some(apply_ops(text, &[JsonOp::Remove { pointer }]).unwrap());
                    }
                }
                other => panic!("unexpected op {other:?}"),
            }
        }
        after
    }

    /// The text `hooks_file` makes for `exec`, written out by hand.
    fn expected_hooks_text(exec: &str) -> String {
        let mut out = String::from("{\n  \"version\": 1,\n  \"hooks\": {\n");
        for (i, (event, _, timeout)) in EVENTS.iter().enumerate() {
            out.push_str(&format!(
                "    \"{event}\": [\n      {{\n        \"type\": \"command\",\n        \"exec\": \"{exec}\",\n        \"args\": [\n          \"event\",\n          \"--agent\",\n          \"copilot\",\n          \"--event\",\n          \"{event}\",\n          \"--tag\",\n          \"mshl1\"\n        ],\n        \"timeoutSec\": {timeout}\n      }}\n    ]{}\n",
                if i + 1 < EVENTS.len() { "," } else { "" }
            ));
        }
        out.push_str("  }\n}\n");
        out
    }

    const THEIR_LINE: &str = "{\r\n    \"type\": \"command\",\r\n    \"command\": \"my-line.cmd\",\r\n    \"padding\": 2,\r\n    \"refreshInterval\": 5\r\n  }";
    fn their_settings() -> String {
        format!(
            "\u{feff}// my settings\r\n{{\r\n  \"model\": \"gpt-5\",\r\n  \"statusLine\": {THEIR_LINE},\r\n  \"theme\": \"github\" // keep\r\n}}\r\n"
        )
    }

    // ---- the hooks file ----

    #[test]
    fn install_into_an_empty_home_creates_the_file_and_the_status_line() {
        let (cs, after) = install(&State::default());
        assert_eq!(cs.title, "Set up Copilot CLI hooks");
        assert_eq!(cs.edits.len(), 3, "{:?}", cs.edits);

        assert_eq!(cs.edits[0].path, hooks_path(&copilot_home()).to_string_lossy());
        let EditOp::CreateFile { content } = &cs.edits[0].op else {
            panic!("{:?}", cs.edits[0].op)
        };
        assert_eq!(content, &expected_hooks_text(BRIDGE_PATH));
        assert_eq!(cs.edits[0].expected_hash, None);

        assert_eq!(cs.edits[1].path, settings_path(&copilot_home()).to_string_lossy());
        assert_eq!(cs.edits[1].scope, ConfigScope::User);
        assert_eq!(
            after.settings.as_deref().map(value),
            Some(json!({"statusLine": {"type": "command", "command": ours_line()}}))
        );

        assert_eq!(cs.edits[2].path, chain_path().to_string_lossy());
        assert_eq!(after.chain.as_deref().map(value), Some(json!({"statusLine": null})));
        assert_eq!(after.hooks.as_deref(), Some(expected_hooks_text(BRIDGE_PATH).as_str()));
    }

    #[test]
    fn the_status_line_command_is_the_bridge_then_statusline_agent_and_tag() {
        let command = ours_line();
        assert!(
            command.ends_with(" statusline --agent copilot --tag mshl1"),
            "{command}"
        );
        assert!(command.contains("marshell-hook.exe"), "{command}");
    }

    #[test]
    fn each_entry_runs_the_bridge_by_exec_with_its_event_and_tag() {
        let text = hooks_file(&bridge(), &BOTH);
        assert!(text.ends_with("}\n"));
        let file = value(&text);
        assert_eq!(file["version"], 1);
        let hooks = file["hooks"].as_object().unwrap();
        let events: Vec<&str> = hooks.keys().map(String::as_str).collect();
        assert_eq!(events, EVENTS.map(|(event, _, _)| event));
        for (event, role, timeout) in EVENTS {
            let entries = hooks[event].as_array().unwrap();
            assert_eq!(entries.len(), 1, "{event}");
            assert_eq!(
                entries[0],
                json!({
                    "type": "command",
                    "exec": BRIDGE_PATH,
                    "args": [role, "--agent", "copilot", "--event", event, "--tag", "mshl1"],
                    "timeoutSec": timeout,
                }),
                "{event}"
            );
            let keys: Vec<&str> = entries[0].as_object().unwrap().keys().map(String::as_str).collect();
            assert_eq!(keys, ["type", "exec", "args", "timeoutSec"], "{event}");
            for absent in ["command", "bash", "powershell", "matcher"] {
                assert!(entries[0].get(absent).is_none(), "{event} has {absent}");
            }
            assert!(is_ours(&entries[0]), "{event}");
        }
        assert_eq!(hooks["sessionEnd"][0]["timeoutSec"], 2);
        assert_eq!(hooks["preToolUse"][0]["timeoutSec"], 5);
    }

    #[test]
    fn want_picks_the_entries() {
        // Status off: no hooks file, and nothing about the status line either.
        let off = HookPlanRequest { status: false, ..BOTH };
        assert!(
            try_install(&State::default(), &bridge(), &off)
                .unwrap()
                .edits
                .is_empty()
        );
        assert_eq!(value(&hooks_file(&bridge(), &off)), json!({"version": 1, "hooks": {}}));

        // Status line off: the hooks file and nothing in settings or the chain.
        let no_line = HookPlanRequest {
            statusline: false,
            ..BOTH
        };
        let cs = try_install(&State::settings(&their_settings()), &bridge(), &no_line).unwrap();
        assert_eq!(cs.edits.len(), 1);
        assert_eq!(cs.edits[0].path, hooks_path(&copilot_home()).to_string_lossy());

        // Approve changes nothing.
        for state in [State::default(), State::settings(&their_settings())] {
            let on = try_install(&state, &bridge(), &HookPlanRequest { approve: true, ..BOTH }).unwrap();
            let not = try_install(&state, &bridge(), &HookPlanRequest { approve: false, ..BOTH }).unwrap();
            assert_eq!(on, not);
        }
        assert_eq!(
            hooks_file(&bridge(), &HookPlanRequest { approve: true, ..BOTH }),
            hooks_file(&bridge(), &BOTH)
        );

        // Status off over Marshell's file plans its removal.
        let (_, installed) = install(&State::default());
        let cs = try_install(&installed, &bridge(), &off).unwrap();
        assert_eq!(cs.edits.len(), 1);
        assert_eq!(cs.edits[0].op, EditOp::DeleteFile);
        assert_eq!(apply(&cs, &installed).hooks, None);
    }

    #[test]
    fn no_entry_is_a_permission_request() {
        assert_eq!(EVENTS.len(), 9);
        for (event, role, _) in EVENTS {
            assert!(!event.to_lowercase().contains("permission"), "{event}");
            assert_ne!(role, "permission");
        }
        for approve in [false, true] {
            let text = hooks_file(&bridge(), &HookPlanRequest { approve, ..BOTH });
            assert!(!text.to_lowercase().contains("permission"), "approve={approve}");
        }
    }

    #[test]
    fn reinstall_with_the_same_bridge_plans_nothing() {
        let (_, installed) = install(&State::default());
        let again = try_install(&installed, &bridge(), &BOTH).unwrap();
        assert!(again.edits.is_empty(), "{:?}", again.edits);

        let (_, theirs) = install(&State::settings(&their_settings()));
        let again = try_install(&theirs, &bridge(), &BOTH).unwrap();
        assert!(again.edits.is_empty(), "{:?}", again.edits);
    }

    #[test]
    fn a_moved_bridge_replaces_the_file_against_its_hash() {
        let (_, installed) = install(&State::default());
        let moved = Bridge::Local(PathBuf::from("D:/Other Home/.marshell/bin/marshell-hook.exe"));
        let (cs, after) = install_with(&installed, &moved, &BOTH);
        let hooks: Vec<&FileEdit> = cs
            .edits
            .iter()
            .filter(|e| e.path == hooks_path(&copilot_home()).to_string_lossy())
            .collect();
        assert_eq!(hooks.len(), 1);
        assert!(matches!(hooks[0].op, EditOp::ReplaceText { .. }));
        assert_eq!(
            hooks[0].expected_hash,
            Some(hash_bytes(installed.hooks.as_deref().unwrap().as_bytes()))
        );
        assert_eq!(
            after.hooks.as_deref(),
            Some(expected_hooks_text("D:/Other Home/.marshell/bin/marshell-hook.exe").as_str())
        );
        // The status line follows the bridge, and the chain is left as it is.
        assert_eq!(
            value(after.settings.as_deref().unwrap())["statusLine"]["command"],
            statusline_command(&moved).unwrap().as_str()
        );
        assert_eq!(after.chain, installed.chain);
    }

    #[test]
    fn a_marshell_json_the_person_wrote_is_refused() {
        let path = hooks_path(&copilot_home());
        let theirs = r#"{"version":1,"hooks":{"preToolUse":[{"type":"command","exec":"mine.exe","args":["a"]}]}}"#;
        let (_, installed) = install(&State::default());
        let mixed = installed.hooks.as_deref().unwrap().replace(
            "\"hooks\": {",
            "\"hooks\": {\n    \"preCompact\": [{\"type\": \"command\", \"bash\": \"echo mine\"}],",
        );
        for text in [theirs, r#"{"version":1}"#, r#"{"hooks": []}"#, mixed.as_str()] {
            let state = State {
                hooks: Some(text.to_string()),
                ..State::default()
            };
            let want = PlanError::NotMarshells { path: path.clone() };
            assert_eq!(
                try_install(&state, &bridge(), &BOTH),
                Err(PlanError::NotMarshells { path: path.clone() }),
                "{text}"
            );
            assert_eq!(
                try_install(&state, &bridge(), &HookPlanRequest { status: false, ..BOTH }),
                Err(PlanError::NotMarshells { path: path.clone() })
            );
            assert_eq!(with_files(&state, remove_plan).err(), Some(want));
        }
        assert_eq!(
            PlanError::NotMarshells { path: path.clone() }.to_string(),
            format!(
                "{} holds hooks Marshell didn't write, so Marshell leaves it as it is.",
                path.display()
            )
        );
    }

    #[test]
    fn a_hooks_file_that_does_not_parse_plans_nothing() {
        let state = State {
            hooks: Some("{ \"hooks\": ".to_string()),
            ..State::default()
        };
        let path = hooks_path(&copilot_home());
        match try_install(&state, &bridge(), &BOTH) {
            Err(PlanError::DoesNotParse { path: p, .. }) => assert_eq!(p, path),
            other => panic!("{other:?}"),
        }
        match with_files(&state, remove_plan) {
            Err(PlanError::DoesNotParse { path: p, .. }) => assert_eq!(p, path),
            other => panic!("{:?}", other.map(|p| p.changes)),
        }
    }

    // ---- the status line ----

    #[test]
    fn install_keeps_the_persons_settings_bytes() {
        let before = their_settings();
        let (cs, after) = install(&State::settings(&before));
        // The hooks file, the settings command and the chain.
        assert_eq!(cs.edits.len(), 3);
        let expected = before.replace("my-line.cmd", &json_escaped(&ours_line()));
        assert_eq!(after.settings.as_deref(), Some(expected.as_str()));
        assert!(after.settings.as_deref().unwrap().starts_with('\u{feff}'));
        let line = &value(after.settings.as_deref().unwrap())["statusLine"];
        assert_eq!(line["padding"], 2);
        assert_eq!(line["refreshInterval"], 5);
        assert_eq!(line["type"], "command");
    }

    #[test]
    fn install_chains_their_command_status_line() {
        let (_, after) = install(&State::settings(&their_settings()));
        assert_eq!(
            after.chain.as_deref().map(value),
            Some(json!({"statusLine": {
                "type": "command",
                "command": "my-line.cmd",
                "padding": 2,
                "refreshInterval": 5,
            }}))
        );
        // Ours already: nothing is saved over the chain.
        let (cs, _) = install_with(&after, &bridge(), &BOTH);
        assert!(cs.edits.is_empty());
    }

    #[test]
    fn install_over_no_status_line_chains_null() {
        for text in [
            "{\n  \"model\": \"gpt-5\"\n}\n",
            "{\n  \"model\": \"gpt-5\",\n  \"statusLine\": null\n}\n",
            "{}",
        ] {
            let (_, after) = install(&State::settings(text));
            assert_eq!(
                after.chain.as_deref().map(value),
                Some(json!({"statusLine": null})),
                "{text}"
            );
            let settings = value(after.settings.as_deref().unwrap());
            assert_eq!(
                settings["statusLine"],
                json!({"type": "command", "command": ours_line()}),
                "{text}"
            );
        }
        // A blank settings file is filled with only that key.
        let (_, after) = install(&State::settings("  \n"));
        assert_eq!(
            after.settings.as_deref().map(value),
            Some(json!({"statusLine": {"type": "command", "command": ours_line()}}))
        );
    }

    #[test]
    fn a_status_line_without_a_command_is_left() {
        for line in [
            r#"{"type": "preset"}"#,
            r#"{"type": "command", "command": 5}"#,
            r#""text""#,
            "5",
            "[]",
        ] {
            let text = format!("{{\n  \"statusLine\": {line}\n}}\n");
            let state = State::settings(&text);
            let (cs, after) = install(&state);
            assert_eq!(cs.edits.len(), 1, "{line}");
            assert_eq!(
                cs.edits[0].path,
                hooks_path(&copilot_home()).to_string_lossy(),
                "{line}"
            );
            assert_eq!(after.settings.as_deref(), Some(text.as_str()), "{line}");
            assert_eq!(after.chain, None, "{line}");
        }
    }

    #[test]
    fn settings_that_do_not_parse_plan_nothing() {
        let state = State::settings("{ \"statusLine\": ");
        let path = settings_path(&copilot_home());
        match try_install(&state, &bridge(), &BOTH) {
            Err(PlanError::DoesNotParse { path: p, .. }) => assert_eq!(p, path),
            other => panic!("{other:?}"),
        }
        match with_files(&state, remove_plan) {
            Err(PlanError::DoesNotParse { path: p, .. }) => assert_eq!(p, path),
            other => panic!("{:?}", other.map(|p| p.changes)),
        }
        // Not an object.
        assert_eq!(
            try_install(&State::settings("[1]"), &bridge(), &BOTH),
            Err(PlanError::NotAnObject {
                path,
                pointer: String::new()
            })
        );
        // With the status line off, the settings are not read at all.
        let no_line = HookPlanRequest {
            statusline: false,
            ..BOTH
        };
        assert_eq!(try_install(&state, &bridge(), &no_line).unwrap().edits.len(), 1);
    }

    // ---- removal ----

    #[test]
    fn removal_restores_the_original_bytes() {
        let cases = [
            ("their own line, bom and crlf", their_settings()),
            ("their own line, lf", their_settings().replace("\r\n", "\n")),
            ("no line", "{\n  \"model\": \"gpt-5\"\n}\n".to_string()),
            (
                "no line, crlf",
                "\u{feff}{\r\n  \"model\": \"gpt-5\"\r\n}\r\n".to_string(),
            ),
            (
                "no line, comments",
                "// mine\n{\n  \"model\": \"gpt-5\", // keep\n  \"theme\": \"github\"\n}\n".to_string(),
            ),
            ("empty object", "{}\n".to_string()),
        ];
        for (name, text) in cases {
            let (_, installed) = install(&State::settings(&text));
            assert_ne!(
                installed.settings.as_deref(),
                Some(text.as_str()),
                "{name}: install changed nothing"
            );
            assert!(installed.hooks.is_some(), "{name}");
            let (plan, removed) = remove(&installed);
            assert_eq!(removed.settings.as_deref(), Some(text.as_str()), "{name}");
            assert_eq!(removed.hooks, None, "{name}");
            assert_eq!(removed.chain, None, "{name}");
            assert!(!plan.statusline_left, "{name}");
            assert_eq!(plan.changes.title, "Turn Copilot CLI hooks off");
            let hooks = plan
                .changes
                .edits
                .iter()
                .find(|e| e.path == hooks_path(&copilot_home()).to_string_lossy())
                .unwrap();
            assert_eq!(hooks.op, EditOp::DeleteFile);
        }
    }

    #[test]
    fn removal_from_a_created_settings_file_leaves_the_file() {
        let (_, installed) = install(&State::default());
        let (_, removed) = remove(&installed);
        assert_eq!(removed.settings.as_deref().map(value), Some(json!({})));
        assert_eq!(removed.hooks, None);
        assert_eq!(removed.chain, None);
    }

    #[test]
    fn removal_leaves_a_status_line_the_person_changed() {
        let (_, installed) = install(&State::settings(&their_settings()));
        let changed = State {
            settings: Some(
                installed
                    .settings
                    .as_deref()
                    .unwrap()
                    .replace(&json_escaped(&ours_line()), "newer.cmd"),
            ),
            ..installed.clone()
        };
        let (plan, removed) = remove(&changed);
        assert!(plan.statusline_left);
        assert_eq!(removed.settings, changed.settings);
        assert_eq!(removed.chain, None);
        assert_eq!(removed.hooks, None);
    }

    #[test]
    fn removal_with_a_lost_chain_removes_the_line_it_added() {
        let (_, installed) = install(&State::settings("{\n  \"model\": \"gpt-5\"\n}\n"));
        let lost = State {
            chain: None,
            ..installed
        };
        let (plan, removed) = remove(&lost);
        assert!(!plan.statusline_left);
        assert_eq!(removed.settings.as_deref().map(value), Some(json!({"model": "gpt-5"})));
    }

    #[test]
    fn removal_with_nothing_of_marshells_plans_nothing() {
        let plan = with_files(&State::settings(&their_settings()), remove_plan).unwrap();
        assert!(plan.changes.edits.is_empty());
        assert!(!plan.statusline_left);
    }

    // ---- inspect ----

    #[test]
    fn inspect_reports_installed_partial_orphaned_and_none() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("bin").join("marshell-hook.exe");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"x").unwrap();
        let real = Bridge::Local(exe);
        let found = |state: &State, b: &Bridge| with_files(state, |files| inspect(files, b));

        // None: no file.
        let none = found(&State::settings(&their_settings()), &real);
        assert_eq!(none.install, HookInstall::None);
        assert!(none.has_own_statusline && !none.chained);
        assert_eq!(found(&State::default(), &real).install, HookInstall::None);

        // Installed.
        let (_, installed) = install_with(&State::settings(&their_settings()), &real, &BOTH);
        let f = found(&installed, &real);
        assert_eq!(f.install, HookInstall::Installed);
        assert!(f.has_own_statusline && f.chained);
        let (_, bare) = install_with(&State::default(), &real, &BOTH);
        let f = found(&bare, &real);
        assert_eq!(f.install, HookInstall::Installed);
        assert!(!f.has_own_statusline && !f.chained);

        // Installed, with a status line left by the "any other shape" rule.
        let (_, other) = install_with(&State::settings(r#"{"statusLine": {"type": "preset"}}"#), &real, &BOTH);
        assert_eq!(found(&other, &real).install, HookInstall::Installed);

        // Orphaned: the bridge the entries run is gone.
        assert_eq!(found(&installed, &bridge()).install, HookInstall::Orphaned);

        // Partial: an entry missing, an entry changed, the status line not Marshell's or not current, extra entries.
        let mut file = value(installed.hooks.as_deref().unwrap());
        file["hooks"].as_object_mut().unwrap().remove("agentStop");
        let missing = State {
            hooks: Some(serde_json::to_string_pretty(&file).unwrap()),
            ..installed.clone()
        };
        assert_eq!(found(&missing, &real).install, HookInstall::Partial);

        let mut file = value(installed.hooks.as_deref().unwrap());
        file["hooks"]["preToolUse"][0]["timeoutSec"] = json!(99);
        let changed = State {
            hooks: Some(serde_json::to_string_pretty(&file).unwrap()),
            ..installed.clone()
        };
        assert_eq!(found(&changed, &real).install, HookInstall::Partial);

        let mut file = value(installed.hooks.as_deref().unwrap());
        let extra = file["hooks"]["preToolUse"][0].clone();
        file["hooks"]["preToolUse"].as_array_mut().unwrap().push(extra);
        let twice = State {
            hooks: Some(serde_json::to_string_pretty(&file).unwrap()),
            ..installed.clone()
        };
        assert_eq!(found(&twice, &real).install, HookInstall::Partial);

        // Installed: a setup with the status line off leaves the person's own line, or none, as it is (as Qwen's).
        let no_line = HookPlanRequest {
            statusline: false,
            ..BOTH
        };
        let (_, file_only) = install_with(&State::settings(&their_settings()), &real, &no_line);
        assert_eq!(file_only.settings.as_deref(), Some(their_settings().as_str()));
        let f = found(&file_only, &real);
        assert_eq!(f.install, HookInstall::Installed);
        assert!(f.has_own_statusline && !f.chained);
        let (_, bare_file_only) = install_with(&State::default(), &real, &no_line);
        assert_eq!(found(&bare_file_only, &real).install, HookInstall::Installed);

        let stale_line = State {
            settings: installed.settings.as_deref().map(|t| {
                t.replace(
                    &json_escaped(&ours_line_for(&real)),
                    "other-bridge statusline --agent copilot --tag mshl1",
                )
            }),
            ..installed.clone()
        };
        assert_eq!(found(&stale_line, &real).install, HookInstall::Partial);

        // A status line of Marshell's with no hooks file is still Marshell's to remove.
        let line_only = State {
            hooks: None,
            ..installed.clone()
        };
        assert_eq!(found(&line_only, &real).install, HookInstall::Partial);
    }

    fn ours_line_for(b: &Bridge) -> String {
        statusline_command(b).unwrap()
    }
}
