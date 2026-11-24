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
    hooks_text(hooks)
}

/// The text `hooks_file` writes with no entries; what `is_marshells` compares an entry-less file with.
fn empty_hooks_text() -> String {
    hooks_text(serde_json::Map::new())
}

fn hooks_text(hooks: serde_json::Map<String, Value>) -> String {
    let mut text =
        serde_json::to_string_pretty(&json!({ "version": 1, "hooks": hooks })).expect("a JSON value always serializes");
    text.push('\n');
    text
}

/// The command string of Marshell's status line. In a distro, which keeps no chain file, it says `--no-chain`.
pub fn statusline_command(bridge: &Bridge) -> Result<String, PlanError> {
    let no_chain = if bridge.distro().is_some() { " --no-chain" } else { "" };
    Ok(format!(
        "{} statusline --agent {AGENT} --tag {TAG}{no_chain}",
        common::bridge_prefix_of(AGENT_NAME, bridge)?
    ))
}

/// The text of the three files a plan reads; `None` is a missing file.
pub struct Files<'a> {
    pub hooks_path: &'a Path,
    pub hooks_text: Option<&'a str>,
    pub settings_path: &'a Path,
    pub settings_text: Option<&'a str>,
    /// `None` in a distro, which keeps no chain.
    pub chain_path: Option<&'a Path>,
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

/// The file is Marshell's: it holds no key but `version` and `hooks`, it has a `hooks` object, and every event holds only
/// entries Marshell wrote (a key of the person's makes it theirs: a whole-file plan would take it away). A file with no
/// entry at all (every list empty, or no list) proves nothing by its entries, and deleting or overwriting it is a
/// write to a file someone else may have made: it is Marshell's only when its text is the one `hooks_file` writes with
/// the status off (line endings aside).
fn is_marshells(file: &Value, text: &str) -> bool {
    if file
        .as_object()
        .is_some_and(|root| root.keys().any(|k| k != "version" && k != "hooks"))
    {
        return false;
    }
    let Some(hooks) = file.get("hooks").and_then(Value::as_object) else {
        return false;
    };
    let ours = hooks
        .values()
        .all(|entries| entries.as_array().is_some_and(|e| e.iter().all(is_ours)));
    if !ours {
        return false;
    }
    entries_of(file).next().is_some() || unix_lines(text) == unix_lines(&empty_hooks_text())
}

fn unix_lines(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// What is wrong with a hooks file that is there: it doesn't parse, or it isn't Marshell's whole file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileProblem {
    DoesNotParse { line: u32, message: String },
    NotMarshells,
}

/// The hooks file's problem, if any; a missing file (`None`) has none.
pub fn file_problem(text: Option<&str>) -> Option<FileProblem> {
    let text = text?;
    let Ok(file) = read_value(text) else {
        let (line, message) = parse_problem(text).unwrap_or((1, "The file does not parse.".to_string()));
        return Some(FileProblem::DoesNotParse { line, message });
    };
    (!is_marshells(&file, text)).then_some(FileProblem::NotMarshells)
}

/// What a plan does with a hooks file that parses and isn't Marshell's.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Foreign {
    /// Plans nothing and says so.
    Refuse,
    /// Replaces the whole file with Marshell's hooks.
    Replace,
    /// The person keeps the file: only Marshell's own entries change.
    Keep,
}

/// The hooks file's text when it is there and Marshell's, and whether it is a foreign file a plan goes on with. A file
/// that doesn't parse is an error; one that isn't Marshell's is too, if `foreign` is `Refuse`.
fn marshells_hooks<'a>(files: &Files<'a>, foreign: Foreign) -> Result<(Option<&'a str>, bool), PlanError> {
    let Some(text) = files.hooks_text else {
        return Ok((None, false));
    };
    match file_problem(Some(text)) {
        None => Ok((Some(text), false)),
        Some(FileProblem::DoesNotParse { .. }) => Err(does_not_parse(files.hooks_path, text)),
        Some(FileProblem::NotMarshells) if foreign != Foreign::Refuse => Ok((Some(text), true)),
        Some(FileProblem::NotMarshells) => Err(PlanError::NotMarshells {
            path: files.hooks_path.to_path_buf(),
        }),
    }
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

    let ours = hooks
        .as_ref()
        .map_or(0, |file| entries_of(file).filter(|(_, e)| is_ours(e)).count());
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
        // Entries of the person's own (in a file they keep) don't count against it: Marshell's are what is checked.
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
///
/// In a distro (`bridge` is `Bridge::Distro`, and `files` holds no chain file) there is no chain: the person's own
/// status line stays as it is, with a note, and Marshell's own line says `--no-chain`.
pub fn install_plan(files: &Files, bridge: &Bridge, want: &HookPlanRequest) -> Result<ChangeSet, PlanError> {
    plan_install(files, bridge, want, Foreign::Refuse)
}

/// `install_plan`, except that a hooks file that parses and isn't Marshell's is replaced with Marshell's hooks
/// (`ReplaceText` against its present text). The Doctor's fix uses it, once the person has picked it. A file that
/// doesn't parse is still refused, and so is a foreign file when `want.status` is off, which would delete it.
pub fn install_plan_over(files: &Files, bridge: &Bridge, want: &HookPlanRequest) -> Result<ChangeSet, PlanError> {
    plan_install(files, bridge, want, Foreign::Replace)
}

/// `install_plan` for a hooks file the person keeps: over a file that parses and isn't Marshell's, only Marshell's own
/// entries change, by JSON edits that leave every other byte as it is (PLAN §9, decision 20). Each event of `EVENTS`
/// is left with exactly one entry of Marshell's, equal to the one `hooks_file` writes and in the place an entry of
/// Marshell's already had (at the end when there was none); entries of Marshell's under any
/// other event go; a missing `version` gets 1. With `want.status` off Marshell's entries go and the file stays. Over
/// Marshell's own file or no file it is `install_plan`. A hooks file that doesn't parse, or whose `hooks` (or an
/// event's list that would be edited) isn't an object (or a list), is refused.
pub fn install_plan_kept(files: &Files, bridge: &Bridge, want: &HookPlanRequest) -> Result<ChangeSet, PlanError> {
    plan_install(files, bridge, want, Foreign::Keep)
}

/// `remove_plan` for a hooks file the person keeps: Marshell's entries come out of it (highest index first) and the file
/// and the person's entries stay. The status line and chain are handled as `remove_plan` does.
pub fn remove_plan_kept(files: &Files, distro: Option<&str>) -> Result<RemovePlan, PlanError> {
    plan_remove(files, distro, Foreign::Keep)
}

/// `/` and `~` in an event's name, as a JSON pointer token.
fn esc(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn not_editable(path: &Path, pointer: &str) -> PlanError {
    PlanError::NotAnObject {
        path: path.to_path_buf(),
        pointer: pointer.to_string(),
    }
}

/// The ops that take Marshell's entries out of the entries of one event, highest index first.
fn remove_ours(event: &str, entries: &[Value]) -> impl Iterator<Item = EditOp> {
    let event = esc(event);
    entries
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, e)| is_ours(e))
        .map(move |(i, _)| EditOp::JsonRemove {
            pointer: format!("/hooks/{event}/{i}"),
        })
}

/// The ops over a kept hooks file that parses and isn't Marshell's: what leaves one entry of Marshell's per event of
/// `EVENTS` (with `install`), and none anywhere else. Any other key of the file, and the person's entries, stay.
fn kept_ops(path: &Path, file: &Value, install: Option<(&Bridge, &HookPlanRequest)>) -> Result<Vec<EditOp>, PlanError> {
    let Some(root) = file.as_object() else {
        return Err(not_editable(path, ""));
    };
    let hooks = match root.get("hooks") {
        None => None,
        Some(Value::Object(hooks)) => Some(hooks),
        Some(_) => return Err(not_editable(path, "/hooks")),
    };
    let wanted = install.filter(|(_, want)| want.status);
    let mut ops = Vec::new();
    if wanted.is_some() && !root.contains_key("version") {
        ops.push(set("/version", &json!(1)));
    }
    // Events outside `EVENTS`, and every event when nothing is wanted: Marshell's entries come out.
    for (event, entries) in hooks.into_iter().flatten() {
        let listed = EVENTS.iter().any(|(e, _, _)| e == event);
        if let Some(entries) = entries.as_array().filter(|_| !(listed && wanted.is_some())) {
            ops.extend(remove_ours(event, entries));
        }
    }
    let Some((bridge, _)) = wanted else {
        return Ok(ops);
    };
    for (event, role, timeout) in EVENTS {
        let entry = entry_of(bridge, event, role, timeout);
        let token = esc(event);
        match hooks.and_then(|h| h.get(event)) {
            None => ops.push(set(&format!("/hooks/{token}"), &json!([entry]))),
            Some(Value::Array(entries)) => {
                // The one kept is the exact entry when there is one, else the first of Marshell's.
                let mine: Vec<usize> = (0..entries.len()).filter(|i| is_ours(&entries[*i])).collect();
                let keep = mine
                    .iter()
                    .copied()
                    .find(|i| entries[*i] == entry)
                    .or(mine.first().copied());
                match keep {
                    None => ops.push(set(&format!("/hooks/{token}/-"), &entry)),
                    Some(i) if entries[i] != entry => ops.push(set(&format!("/hooks/{token}/{i}"), &entry)),
                    Some(_) => {}
                }
                ops.extend(
                    mine.into_iter()
                        .rev()
                        .filter(|i| Some(*i) != keep)
                        .map(|i| EditOp::JsonRemove {
                            pointer: format!("/hooks/{token}/{i}"),
                        }),
                );
            }
            Some(_) => return Err(not_editable(path, &format!("/hooks/{token}"))),
        }
    }
    Ok(ops)
}

fn plan_install(
    files: &Files,
    bridge: &Bridge,
    want: &HookPlanRequest,
    foreign_file: Foreign,
) -> Result<ChangeSet, PlanError> {
    let title = "Set up Copilot CLI hooks".to_string();
    let (existing, foreign) = marshells_hooks(files, foreign_file)?;
    if foreign && !want.status && foreign_file == Foreign::Replace {
        return Err(PlanError::NotMarshells {
            path: files.hooks_path.to_path_buf(),
        });
    }
    let kept = foreign && foreign_file == Foreign::Keep;
    let mut edits: Vec<FileEdit> = Vec::new();
    let mut how = String::new();
    if let (Some(text), true) = (existing, kept) {
        let file = read_value(text).map_err(|_| does_not_parse(files.hooks_path, text))?;
        let ops = kept_ops(files.hooks_path, &file, Some((bridge, want)))?;
        edits.extend(
            ops.into_iter()
                .map(|op| edit(files.hooks_path, ConfigScope::User, op, Some(text))),
        );
    } else {
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
        edits.extend(hooks_edit);
        how = match edits.first().map(|e| &e.op) {
            Some(EditOp::DeleteFile) => "Removes Marshell's hooks file".to_string(),
            Some(_) => format!(
                "Writes Marshell's own hooks file with {} entries so Marshell can show what {AGENT_NAME} is doing",
                EVENTS.len()
            ),
            None => String::new(),
        };
    }
    let hooks_changed_in_kept = kept && !edits.is_empty();

    let mut left_alone = None;
    if want.status && want.statusline {
        let settings = parse_settings(files.settings_path, files.settings_text)?;
        let ours = statusline_command(bridge)?;
        let (ops, chain, tail): (Vec<EditOp>, Option<Value>, &str) = match line_of(&settings) {
            Line::Theirs(_) if bridge.distro().is_some() => {
                left_alone = bridge.distro().map(common::statusline_left_note);
                (Vec::new(), None, "")
            }
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
        let chain = chain.filter(|_| bridge.distro().is_none());
        if let (Some(chain), Some(chain_path)) = (chain, files.chain_path) {
            edits.push(edit(
                chain_path,
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

    let mut reason = if how.is_empty() {
        if want.status {
            "Marshell's hooks are already in place, so nothing changes.".to_string()
        } else {
            "Nothing was asked for, so nothing changes.".to_string()
        }
    } else {
        format!("{how}.")
    };
    if hooks_changed_in_kept {
        let note = format!("Changes only Marshell's hooks in {}.", files.hooks_path.display());
        reason = if how.is_empty() {
            note
        } else {
            format!("{note} {reason}")
        };
    } else if foreign && !kept {
        reason = format!(
            "{reason} Replaces {} with Marshell's hooks.",
            files.hooks_path.display()
        );
    }
    if let Some(note) = left_alone {
        reason = format!("{reason} {note}");
    }
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

fn saved_line(chain_text: Option<&str>, in_distro: bool) -> Saved {
    // A distro keeps no chain: the line Marshell added there went over none.
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

/// The plan that deletes Marshell's hooks file and puts the person's status line back.
///
/// The chain file is read, then deleted in the same change set, so one undo brings back all three files. While
/// `statusLine.command` is Marshell's, the chain's command goes back in that key (or the key goes, when the chain
/// holds `null`); a command the person changed since the setup is left alone. `settings.json` is never deleted.
///
/// In a distro (`distro` is its name) there is no chain: a status line of Marshell's was added over none, and the key
/// goes, and any other is the person's, left alone.
pub fn remove_plan(files: &Files, distro: Option<&str>) -> Result<RemovePlan, PlanError> {
    plan_remove(files, distro, Foreign::Refuse)
}

fn plan_remove(files: &Files, distro: Option<&str>, foreign_file: Foreign) -> Result<RemovePlan, PlanError> {
    let title = "Turn Copilot CLI hooks off".to_string();
    let (hooks, foreign) = marshells_hooks(files, foreign_file)?;
    let settings = parse_settings(files.settings_path, files.settings_text)?;

    let mut statusline_left = false;
    let mut line_note = "";
    let mut ops: Vec<EditOp> = Vec::new();
    match line_of(&settings) {
        Line::Ours(_) => match saved_line(files.chain_text, distro.is_some()) {
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
        Line::Theirs(_) | Line::Other if files.chain_text.is_some() || distro.is_some() => {
            statusline_left = true;
            line_note = if distro.is_some() {
                "leaves your status line alone, because it is not Marshell's"
            } else {
                "leaves your status line alone, because it was changed since the setup"
            };
        }
        _ => {}
    }

    let kept = foreign && foreign_file == Foreign::Keep;
    let mut edits: Vec<FileEdit> = Vec::new();
    match hooks {
        Some(text) if kept => {
            let file = read_value(text).map_err(|_| does_not_parse(files.hooks_path, text))?;
            edits.extend(
                kept_ops(files.hooks_path, &file, None)?
                    .into_iter()
                    .map(|op| edit(files.hooks_path, ConfigScope::User, op, Some(text))),
            );
        }
        Some(text) => edits.push(edit(
            files.hooks_path,
            ConfigScope::User,
            EditOp::DeleteFile,
            Some(text),
        )),
        None => {}
    }
    let hooks_gone = !edits.is_empty();
    edits.extend(
        ops.into_iter()
            .map(|op| edit(files.settings_path, ConfigScope::User, op, files.settings_text)),
    );
    let chain_edit = chain_delete(files.chain_path, files.chain_text, distro);
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
        (true, false) if kept => format!(
            "Removes Marshell's hooks from {} and keeps the rest. It {line_note}.",
            files.hooks_path.display()
        ),
        (true, true) if kept => format!(
            "Removes Marshell's hooks from {} and keeps the rest.",
            files.hooks_path.display()
        ),
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
            chain_path: Some(&cp),
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
        let plan = with_files(state, |files| remove_plan(files, None)).unwrap();
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
            assert_eq!(with_files(&state, |files| remove_plan(files, None)).err(), Some(want));
        }
        assert_eq!(
            PlanError::NotMarshells { path: path.clone() }.to_string(),
            format!(
                "{} holds hooks Marshell didn't write, so Marshell leaves it as it is.",
                path.display()
            )
        );
    }

    /// What `hooks_file` writes with the status off: no entries.
    fn own_empty_text() -> String {
        hooks_file(&bridge(), &HookPlanRequest { status: false, ..BOTH })
    }

    #[test]
    fn an_empty_hooks_file_someone_else_wrote_is_not_marshells() {
        let path = hooks_path(&copilot_home());
        let own = own_empty_text();
        let with_a_key = own.replace("\"hooks\": {}", "\"hooks\": {},\n  \"mine\": true");
        assert_ne!(with_a_key, own);
        for text in [
            r#"{"hooks":{}}"#,
            r#"{"version":1,"hooks":{"sessionStart":[]}}"#,
            with_a_key.as_str(),
        ] {
            let state = State {
                hooks: Some(text.to_string()),
                ..State::default()
            };
            let want = || PlanError::NotMarshells { path: path.clone() };
            assert_eq!(try_install(&state, &bridge(), &BOTH), Err(want()), "{text}");
            assert_eq!(
                try_install(&state, &bridge(), &HookPlanRequest { status: false, ..BOTH }),
                Err(want()),
                "{text}"
            );
            assert_eq!(
                with_files(&state, |files| remove_plan(files, None)).err(),
                Some(want()),
                "{text}"
            );
        }
    }

    #[test]
    fn file_problem_tells_what_is_wrong_with_a_hooks_file() {
        assert_eq!(file_problem(None), None);
        let own = hooks_file(&bridge(), &BOTH);
        assert_eq!(file_problem(Some(&own)), None);
        assert_eq!(file_problem(Some(&own_empty_text())), None);
        for text in [
            "{}",
            r#"{"hooks":{"sessionStart":[{"type":"command","command":"mine"}]}}"#,
        ] {
            assert_eq!(file_problem(Some(text)), Some(FileProblem::NotMarshells), "{text}");
        }
        assert!(matches!(
            file_problem(Some("{\"hooks\": ")),
            Some(FileProblem::DoesNotParse { line, .. }) if line >= 1
        ));
    }

    #[test]
    fn a_file_with_a_key_besides_version_and_hooks_is_not_marshells_even_with_only_its_entries() {
        let mut file: Value = serde_json::from_str(&hooks_file(&bridge(), &BOTH)).unwrap();
        assert_eq!(file_problem(Some(&file.to_string())), None);
        file["disableAllHooks"] = json!(true);
        assert_eq!(file_problem(Some(&file.to_string())), Some(FileProblem::NotMarshells));
        // A whole-file plan over it is refused, and a replace is the only way to overwrite it.
        let state = State {
            hooks: Some(file.to_string()),
            ..State::default()
        };
        assert_eq!(
            try_install(&state, &bridge(), &BOTH),
            Err(PlanError::NotMarshells {
                path: hooks_path(&copilot_home())
            })
        );
    }

    fn try_over(state: &State, b: &Bridge, want: &HookPlanRequest) -> Result<ChangeSet, PlanError> {
        with_files(state, |files| install_plan_over(files, b, want))
    }

    #[test]
    fn install_plan_over_replaces_a_hooks_file_that_is_not_marshells() {
        let path = hooks_path(&copilot_home());
        let theirs = r#"{"version":1,"hooks":{"preToolUse":[{"type":"command","exec":"mine.exe","args":["a"]}]}}"#;
        let (reference, _) = install(&State::default());
        for text in ["{}", theirs] {
            let state = State {
                hooks: Some(text.to_string()),
                ..State::default()
            };
            // install_plan still refuses it.
            assert_eq!(
                try_install(&state, &bridge(), &BOTH),
                Err(PlanError::NotMarshells { path: path.clone() }),
                "{text}"
            );
            let (cs, after) = {
                let cs = try_over(&state, &bridge(), &BOTH).unwrap();
                let after = apply(&cs, &state);
                (cs, after)
            };
            let EditOp::ReplaceText { content } = &cs.edits[0].op else {
                panic!("{:?}", cs.edits[0].op)
            };
            assert_eq!(content, &hooks_file(&bridge(), &BOTH), "{text}");
            assert_eq!(cs.edits[0].path, path.to_string_lossy());
            assert_eq!(cs.edits[0].expected_hash, Some(hash_bytes(text.as_bytes())));
            assert_eq!(after.hooks.as_deref(), Some(expected_hooks_text(BRIDGE_PATH).as_str()));
            // The settings edits are the ones a missing hooks file gets.
            assert_eq!(cs.edits[1..], reference.edits[1..], "{text}");
            assert!(
                cs.reason
                    .contains(&format!("Replaces {} with Marshell's hooks.", path.display())),
                "{}",
                cs.reason
            );
        }
    }

    #[test]
    fn install_plan_over_is_install_plan_for_marshells_own_file_and_no_file() {
        let (_, installed) = install(&State::default());
        for state in [State::default(), installed.clone()] {
            assert_eq!(
                try_over(&state, &bridge(), &BOTH),
                try_install(&state, &bridge(), &BOTH)
            );
        }
        let moved = Bridge::Local(PathBuf::from("D:/Other Home/.marshell/bin/marshell-hook.exe"));
        let (a, b) = (
            try_over(&installed, &moved, &BOTH).unwrap(),
            try_install(&installed, &moved, &BOTH).unwrap(),
        );
        assert_eq!(a, b);
        assert!(!a.reason.contains("Replaces"), "{}", a.reason);
    }

    #[test]
    fn install_plan_over_never_replaces_what_does_not_parse_nor_deletes_a_foreign_file() {
        let path = hooks_path(&copilot_home());
        let broken = State {
            hooks: Some("{ \"hooks\": ".to_string()),
            ..State::default()
        };
        match try_over(&broken, &bridge(), &BOTH) {
            Err(PlanError::DoesNotParse { path: p, .. }) => assert_eq!(p, path),
            other => panic!("{other:?}"),
        }
        // With the status off a foreign file would be deleted: that stays refused.
        let foreign = State {
            hooks: Some("{}".to_string()),
            ..State::default()
        };
        assert_eq!(
            try_over(&foreign, &bridge(), &HookPlanRequest { status: false, ..BOTH }),
            Err(PlanError::NotMarshells { path })
        );
    }

    #[test]
    fn marshells_own_empty_file_still_is() {
        let own = own_empty_text();
        // A file kept by an editor that saved it with Windows line endings is still the same file.
        for text in [own.clone(), own.replace('\n', "\r\n")] {
            assert!(is_marshells(&value(&text), &text), "{text:?}");
            let state = State {
                hooks: Some(text.clone()),
                ..State::default()
            };
            assert!(try_install(&state, &bridge(), &BOTH).is_ok(), "{text:?}");
        }
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
        match with_files(&state, |files| remove_plan(files, None)) {
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
        match with_files(&state, |files| remove_plan(files, None)) {
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
        let plan = with_files(&State::settings(&their_settings()), |files| remove_plan(files, None)).unwrap();
        assert!(plan.changes.edits.is_empty());
        assert!(!plan.statusline_left);
    }

    // ---- a hooks file the person keeps ----

    /// A person's hooks file with a comment, tabs and uneven indentation.
    const PERSONS: &str = "// my hooks\n{\n\t\"version\": 1, // the version\n   \"hooks\": {\n\t\"sessionStart\": [\n\t\t  {\"type\": \"command\", \"bash\": \"echo mine\"}   // mine\n\t]\n  }\n}\n";

    fn real_bridge() -> (tempfile::TempDir, Bridge) {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("bin").join("marshell-hook.exe");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"x").unwrap();
        (dir, Bridge::Local(exe))
    }
    fn with_hooks(text: &str) -> State {
        State {
            hooks: Some(text.to_string()),
            ..State::default()
        }
    }
    fn try_kept(state: &State, b: &Bridge, want: &HookPlanRequest) -> Result<ChangeSet, PlanError> {
        with_files(state, |files| install_plan_kept(files, b, want))
    }
    fn kept_install(state: &State, b: &Bridge, want: &HookPlanRequest) -> (ChangeSet, State) {
        let cs = try_kept(state, b, want).unwrap();
        let after = apply(&cs, state);
        (cs, after)
    }
    fn kept_remove(state: &State) -> (RemovePlan, State) {
        let plan = with_files(state, |files| remove_plan_kept(files, None)).unwrap();
        let after = apply(&plan.changes, state);
        (plan, after)
    }
    fn hooks_edits(cs: &ChangeSet) -> Vec<&FileEdit> {
        let path = hooks_path(&copilot_home()).to_string_lossy().into_owned();
        cs.edits.iter().filter(|e| e.path == path).collect()
    }
    fn no_whole_file_op(cs: &ChangeSet) {
        for e in hooks_edits(cs) {
            assert!(
                matches!(e.op, EditOp::JsonSet { .. } | EditOp::JsonRemove { .. }),
                "{:?}",
                e.op
            );
            assert!(e.expected_hash.is_some());
        }
    }
    fn ours_under<'a>(file: &'a Value, event: &str) -> Vec<&'a Value> {
        file["hooks"][event]
            .as_array()
            .map(|a| a.iter().filter(|e| is_ours(e)).collect())
            .unwrap_or_default()
    }
    fn found(state: &State, b: &Bridge) -> HookInstall {
        with_files(state, |files| inspect(files, b)).install
    }
    fn part(state: &State) -> String {
        crate::agents::copilot::kept::persons_part(&value(state.hooks.as_deref().unwrap()))
    }

    #[test]
    fn a_kept_install_adds_marshells_entries_and_keeps_the_persons_bytes() {
        let (_dir, real) = real_bridge();
        let state = with_hooks(PERSONS);
        let (cs, after) = kept_install(&state, &real, &BOTH);
        no_whole_file_op(&cs);
        assert!(
            cs.reason.starts_with(&format!(
                "Changes only Marshell's hooks in {}.",
                hooks_path(&copilot_home()).display()
            )),
            "{}",
            cs.reason
        );
        let text = after.hooks.as_deref().unwrap();
        // The comments, the tabs and the uneven indentation stand byte for byte up to the end of the person's entry; the
        // appended entry follows it, and the person's trailing comment stays after the array's last element.
        let entry_end = PERSONS.find("echo mine\"}").unwrap() + "echo mine\"}".len();
        assert!(text.starts_with(&PERSONS[..entry_end]), "{text}");
        assert!(text[entry_end..].starts_with(','), "{text}");
        assert!(text.contains("   // mine\n\t]"), "{text}");
        assert!(text.contains("// my hooks\n{\n\t\"version\": 1, // the version\n   \"hooks\": {\n"));
        let file = value(text);
        assert_eq!(
            file["hooks"]["sessionStart"][0],
            json!({"type": "command", "bash": "echo mine"})
        );
        for (event, role, timeout) in EVENTS {
            let ours = ours_under(&file, event);
            assert_eq!(ours.len(), 1, "{event}");
            assert_eq!(*ours[0], entry_of(&real, event, role, timeout), "{event}");
        }
        assert_eq!(file["hooks"].as_object().unwrap().len(), EVENTS.len());
        assert_eq!(found(&after, &real), HookInstall::Installed);
        assert_eq!(part(&after), part(&state));
        // The person's entry stays first under sessionStart, Marshell's is appended.
        assert_eq!(file["hooks"]["sessionStart"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_kept_install_uses_appends_sets_and_removes_by_pointer() {
        let (_dir, real) = real_bridge();
        let cs = try_kept(&with_hooks(PERSONS), &real, &BOTH).unwrap();
        let pointers: Vec<&str> = hooks_edits(&cs)
            .iter()
            .map(|e| match &e.op {
                EditOp::JsonSet { pointer, .. } => pointer.as_str(),
                other => panic!("{other:?}"),
            })
            .collect();
        // Appended to an event that has an array, set whole under one that has none.
        assert!(pointers.contains(&"/hooks/sessionStart/-"), "{pointers:?}");
        assert!(pointers.contains(&"/hooks/preToolUse"), "{pointers:?}");
        assert!(!pointers.contains(&"/version"), "{pointers:?}");
        for e in hooks_edits(&cs) {
            assert_eq!(e.expected_hash, Some(hash_bytes(PERSONS.as_bytes())));
            assert_eq!(e.scope, ConfigScope::User);
        }
    }

    #[test]
    fn a_kept_install_sets_a_changed_entry_in_place_and_adds_none_twice() {
        let (_dir, real) = real_bridge();
        let old = Bridge::Local(PathBuf::from("D:/Old Home/.marshell/bin/marshell-hook.exe"));
        let (_, installed) = kept_install(&with_hooks(PERSONS), &old, &BOTH);
        let (cs, after) = kept_install(&installed, &real, &BOTH);
        no_whole_file_op(&cs);
        let sets: Vec<&str> = hooks_edits(&cs)
            .iter()
            .filter_map(|e| match &e.op {
                EditOp::JsonSet { pointer, .. } => Some(pointer.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(sets.len(), EVENTS.len(), "{sets:?}");
        // Marshell's entry sits at index 1 under sessionStart (after the person's) and 0 elsewhere.
        assert!(sets.contains(&"/hooks/sessionStart/1"), "{sets:?}");
        assert!(sets.contains(&"/hooks/agentStop/0"), "{sets:?}");
        let file = value(after.hooks.as_deref().unwrap());
        for (event, role, timeout) in EVENTS {
            let ours = ours_under(&file, event);
            assert_eq!(ours.len(), 1, "{event}");
            assert_eq!(*ours[0], entry_of(&real, event, role, timeout), "{event}");
        }
        assert_eq!(found(&after, &real), HookInstall::Installed);
        assert_eq!(part(&after), part(&installed));
        assert!(after.hooks.as_deref().unwrap().starts_with("// my hooks\n"));
    }

    #[test]
    fn a_kept_install_leaves_one_of_two_entries_of_marshells() {
        let (_dir, real) = real_bridge();
        let (_, installed) = kept_install(&with_hooks(PERSONS), &real, &BOTH);
        // A second one of Marshell's, with another bridge, and a third; the person's stays between.
        let mut file = value(installed.hooks.as_deref().unwrap());
        let stale = entry_of(&bridge(), "preToolUse", "event", 5);
        let arr = file["hooks"]["preToolUse"].as_array_mut().unwrap();
        arr.push(json!({"type": "command", "bash": "echo between"}));
        arr.push(stale.clone());
        arr.push(stale);
        let doubled = with_hooks(&serde_json::to_string_pretty(&file).unwrap());
        assert_eq!(found(&doubled, &real), HookInstall::Partial);
        let (cs, after) = kept_install(&doubled, &real, &BOTH);
        no_whole_file_op(&cs);
        // The extras go highest index first.
        let removes: Vec<&str> = hooks_edits(&cs)
            .iter()
            .filter_map(|e| match &e.op {
                EditOp::JsonRemove { pointer } => Some(pointer.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(removes, ["/hooks/preToolUse/3", "/hooks/preToolUse/2"], "{removes:?}");
        let file = value(after.hooks.as_deref().unwrap());
        assert_eq!(ours_under(&file, "preToolUse").len(), 1);
        assert_eq!(
            *ours_under(&file, "preToolUse")[0],
            entry_of(&real, "preToolUse", "event", 5)
        );
        assert_eq!(
            file["hooks"]["preToolUse"][1],
            json!({"type": "command", "bash": "echo between"})
        );
        assert_eq!(found(&after, &real), HookInstall::Installed);
        assert_eq!(part(&after), part(&doubled));
    }

    #[test]
    fn a_kept_install_removes_marshells_entries_under_other_events() {
        let (_dir, real) = real_bridge();
        let text = "{\"version\": 1, \"hooks\": {\"preCompact\": [{\"bash\": \"mine\"}, {\"type\": \"command\", \"exec\": \"x.exe\", \"args\": [\"--tag\", \"mshl1\"]}]}}";
        let (cs, after) = kept_install(&with_hooks(text), &real, &BOTH);
        no_whole_file_op(&cs);
        let file = value(after.hooks.as_deref().unwrap());
        assert_eq!(file["hooks"]["preCompact"], json!([{"bash": "mine"}]));
        assert_eq!(found(&after, &real), HookInstall::Installed);
    }

    #[test]
    fn a_kept_install_over_an_empty_object_adds_the_version_and_the_hooks() {
        let (_dir, real) = real_bridge();
        let (cs, after) = kept_install(&with_hooks("{}"), &real, &BOTH);
        no_whole_file_op(&cs);
        let file = value(after.hooks.as_deref().unwrap());
        assert_eq!(file["version"], 1);
        for (event, role, timeout) in EVENTS {
            assert_eq!(
                file["hooks"][event],
                json!([entry_of(&real, event, role, timeout)]),
                "{event}"
            );
        }
        assert_eq!(found(&after, &real), HookInstall::Installed);
        // A hooks object with no arrays, and a version already there.
        let (_, after) = kept_install(&with_hooks("{\"version\": 2, \"hooks\": {}}"), &real, &BOTH);
        assert_eq!(value(after.hooks.as_deref().unwrap())["version"], 2);
        assert_eq!(found(&after, &real), HookInstall::Installed);
    }

    #[test]
    fn a_kept_install_run_twice_plans_no_hooks_edit_the_second_time() {
        let (_dir, real) = real_bridge();
        for text in [PERSONS, "{\"disableAllHooks\": false}"] {
            let (_, once) = kept_install(&with_hooks(text), &real, &BOTH);
            let again = try_kept(&once, &real, &BOTH).unwrap();
            assert!(hooks_edits(&again).is_empty(), "{text}: {:?}", again.edits);
            assert!(again.edits.is_empty(), "{text}: {:?}", again.edits);
            assert_eq!(part(&once), part(&with_hooks(text)), "{text}");
        }
    }

    #[test]
    fn a_kept_install_with_the_status_off_removes_marshells_entries_and_keeps_the_file() {
        let (_dir, real) = real_bridge();
        let (_, installed) = kept_install(&with_hooks(PERSONS), &real, &BOTH);
        let off = HookPlanRequest { status: false, ..BOTH };
        let (cs, after) = kept_install(&installed, &real, &off);
        no_whole_file_op(&cs);
        // Only the hooks file's edits: the status line is left to the status flag.
        assert!(
            cs.edits
                .iter()
                .all(|e| e.path == hooks_path(&copilot_home()).to_string_lossy())
        );
        let file = value(after.hooks.as_deref().unwrap());
        for (event, _, _) in EVENTS {
            assert!(ours_under(&file, event).is_empty(), "{event}");
        }
        assert_eq!(
            file["hooks"]["sessionStart"],
            json!([{"type": "command", "bash": "echo mine"}])
        );
        assert!(
            after
                .hooks
                .as_deref()
                .unwrap()
                .starts_with("// my hooks\n{\n\t\"version\": 1, // the version\n")
        );
        // The status line is still Marshell's, so the install is partial, not gone.
        assert_eq!(found(&after, &real), HookInstall::Partial);
        assert_eq!(part(&after), part(&installed));
        // Nothing of Marshell's: nothing to plan, and no refusal.
        assert!(try_kept(&with_hooks(PERSONS), &real, &off).unwrap().edits.is_empty());
    }

    #[test]
    fn a_kept_install_builds_the_same_settings_edits_as_install_plan() {
        let (_dir, real) = real_bridge();
        let state = State {
            hooks: Some(PERSONS.to_string()),
            settings: Some(their_settings()),
            chain: None,
        };
        let kept = try_kept(&state, &real, &BOTH).unwrap();
        let plain = try_install(
            &State {
                hooks: None,
                ..state.clone()
            },
            &real,
            &BOTH,
        )
        .unwrap();
        let not_hooks = |cs: &ChangeSet| -> Vec<FileEdit> {
            cs.edits
                .iter()
                .filter(|e| e.path != hooks_path(&copilot_home()).to_string_lossy())
                .cloned()
                .collect()
        };
        assert_eq!(not_hooks(&kept), not_hooks(&plain));
        assert!(!not_hooks(&kept).is_empty());
        let after = apply(&kept, &state);
        assert!(
            after
                .settings
                .as_deref()
                .unwrap()
                .contains("statusline --agent copilot")
        );
        assert!(after.chain.is_some());
    }

    #[test]
    fn a_kept_removal_removes_only_marshells_entries() {
        let (_dir, real) = real_bridge();
        let (_, installed) = kept_install(
            &State {
                settings: Some(their_settings()),
                ..with_hooks(PERSONS)
            },
            &real,
            &BOTH,
        );
        let (plan, removed) = kept_remove(&installed);
        no_whole_file_op(&plan.changes);
        assert_eq!(plan.changes.title, "Turn Copilot CLI hooks off");
        assert!(
            plan.changes.reason.starts_with(&format!(
                "Removes Marshell's hooks from {} and keeps the rest.",
                hooks_path(&copilot_home()).display()
            )),
            "{}",
            plan.changes.reason
        );
        // Highest index first within an event: the removals under one event never go up.
        let mut last: std::collections::HashMap<String, usize> = Default::default();
        for e in hooks_edits(&plan.changes) {
            let EditOp::JsonRemove { pointer } = &e.op else {
                panic!("{:?}", e.op)
            };
            let (event, index) = pointer.rsplit_once('/').unwrap();
            let index: usize = index.parse().unwrap();
            if let Some(before) = last.insert(event.to_string(), index) {
                assert!(index < before, "{pointer}");
            }
        }
        let text = removed.hooks.as_deref().unwrap();
        assert!(text.starts_with("// my hooks\n{\n\t\"version\": 1, // the version\n   \"hooks\": {\n"));
        let file = value(text);
        for (event, _, _) in EVENTS {
            assert!(ours_under(&file, event).is_empty(), "{event}");
        }
        assert_eq!(
            file["hooks"]["sessionStart"],
            json!([{"type": "command", "bash": "echo mine"}])
        );
        assert_eq!(part(&removed), part(&installed));
        // The status line goes as remove_plan puts it back, and the chain goes.
        assert_eq!(removed.settings.as_deref(), Some(their_settings().as_str()));
        assert_eq!(removed.chain, None);
        assert!(!plan.statusline_left);
        assert_eq!(found(&removed, &real), HookInstall::None);
    }

    #[test]
    fn a_kept_removal_removes_two_of_marshells_in_one_event_highest_first() {
        let (_dir, real) = real_bridge();
        let ours = |n: u32| json!({"type": "command", "exec": "x.exe", "args": ["--tag", "mshl1", n.to_string()]});
        let file = json!({"version": 1, "hooks": {"preToolUse": [ours(0), {"bash": "mine"}, ours(2), ours(3)]}});
        let state = with_hooks(&serde_json::to_string(&file).unwrap());
        let (plan, removed) = kept_remove(&state);
        let removes: Vec<String> = hooks_edits(&plan.changes)
            .iter()
            .map(|e| match &e.op {
                EditOp::JsonRemove { pointer } => pointer.clone(),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            removes,
            ["/hooks/preToolUse/3", "/hooks/preToolUse/2", "/hooks/preToolUse/0"]
        );
        assert_eq!(
            value(removed.hooks.as_deref().unwrap())["hooks"]["preToolUse"],
            json!([{"bash": "mine"}])
        );
        let _ = real;
    }

    #[test]
    fn the_kept_plans_equal_the_plain_ones_over_marshells_own_file_and_no_file() {
        let (_, installed) = install(&State::default());
        let moved = Bridge::Local(PathBuf::from("D:/Other Home/.marshell/bin/marshell-hook.exe"));
        for state in [State::default(), installed.clone()] {
            for b in [bridge(), moved.clone()] {
                assert_eq!(try_kept(&state, &b, &BOTH), try_install(&state, &b, &BOTH));
            }
            let plain = with_files(&state, |files| remove_plan(files, None));
            let kept = with_files(&state, |files| remove_plan_kept(files, None));
            assert_eq!(
                kept.map(|p| (p.changes, p.statusline_left)),
                plain.map(|p| (p.changes, p.statusline_left))
            );
        }
        // Over a file of Marshell's with no entries, too.
        let empty = with_hooks(&own_empty_text());
        assert_eq!(
            try_kept(&empty, &bridge(), &BOTH),
            try_install(&empty, &bridge(), &BOTH)
        );
    }

    #[test]
    fn the_kept_plans_refuse_a_hooks_file_that_does_not_parse_or_has_a_shape_they_cannot_edit() {
        let path = hooks_path(&copilot_home());
        let broken = with_hooks("{ \"hooks\": ");
        match try_kept(&broken, &bridge(), &BOTH) {
            Err(PlanError::DoesNotParse { path: p, .. }) => assert_eq!(p, path),
            other => panic!("{other:?}"),
        }
        match with_files(&broken, |files| remove_plan_kept(files, None)) {
            Err(PlanError::DoesNotParse { path: p, .. }) => assert_eq!(p, path),
            other => panic!("{:?}", other.map(|p| p.changes)),
        }
        let refused = |pointer: &str| PlanError::NotAnObject {
            path: path.clone(),
            pointer: pointer.to_string(),
        };
        for (text, pointer) in [
            ("[1]", ""),
            ("{\"hooks\": []}", "/hooks"),
            ("{\"hooks\": null}", "/hooks"),
            ("{\"hooks\": {\"sessionStart\": {\"a\": 1}}}", "/hooks/sessionStart"),
            ("{\"hooks\": {\"sessionEnd\": \"x\"}}", "/hooks/sessionEnd"),
        ] {
            assert_eq!(
                try_kept(&with_hooks(text), &bridge(), &BOTH),
                Err(refused(pointer)),
                "{text}"
            );
        }
        // Removing needs only the shapes it reads: a bad `hooks` is refused, an event Marshell has no entry in is not.
        assert_eq!(
            with_files(&with_hooks("{\"hooks\": []}"), |files| remove_plan_kept(files, None)).err(),
            Some(refused("/hooks"))
        );
        assert!(
            with_files(&with_hooks("{\"hooks\": {\"x\": 5}}"), |files| remove_plan_kept(
                files, None
            ))
            .is_ok()
        );
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

    // ---- a WSL distro: no chain file ----

    const LINUX_BRIDGE: &str = "/mnt/c/Users/Jane Doe/.marshell/bin/marshell-hook.exe";

    fn distro_bridge() -> Bridge {
        Bridge::Distro {
            name: "Ubuntu".into(),
            path: LINUX_BRIDGE.into(),
        }
    }

    /// The distro's two files under its own `~/.copilot`, and no chain.
    fn with_distro_files<T>(state: &State, f: impl FnOnce(&Files) -> T) -> T {
        let home = PathBuf::from("/home/u/.copilot");
        let (hp, sp) = (hooks_path(&home), settings_path(&home));
        f(&Files {
            hooks_path: &hp,
            hooks_text: state.hooks.as_deref(),
            settings_path: &sp,
            settings_text: state.settings.as_deref(),
            chain_path: None,
            chain_text: None,
        })
    }

    fn distro_paths(cs: &ChangeSet) -> Vec<String> {
        cs.edits.iter().map(|e| e.path.replace("\\", "/")).collect()
    }

    #[test]
    fn a_distro_install_writes_two_files_and_a_status_line_that_runs_without_the_chain() {
        let cs = with_distro_files(&State::default(), |files| install_plan(files, &distro_bridge(), &BOTH)).unwrap();
        let mut paths = distro_paths(&cs);
        paths.dedup();
        assert_eq!(
            paths,
            ["/home/u/.copilot/hooks/marshell.json", "/home/u/.copilot/settings.json"]
        );
        let EditOp::CreateFile { content } = &cs.edits[0].op else {
            panic!("{:?}", cs.edits[0].op)
        };
        assert_eq!(content, &expected_hooks_text(LINUX_BRIDGE));
        let EditOp::JsonSet { value, .. } = &cs.edits[1].op else {
            panic!("{:?}", cs.edits[1].op)
        };
        let line: Value = serde_json::from_str(value).unwrap();
        assert_eq!(
            line["command"],
            format!("'{LINUX_BRIDGE}' statusline --agent copilot --tag mshl1 --no-chain")
        );
        // The command on Windows has no such flag.
        assert!(!ours_line().contains("--no-chain"));
    }

    #[test]
    fn a_distro_install_leaves_their_status_line_and_says_so() {
        let state = State::settings(&their_settings());
        let cs = with_distro_files(&state, |files| install_plan(files, &distro_bridge(), &BOTH)).unwrap();
        assert_eq!(
            distro_paths(&cs),
            ["/home/u/.copilot/hooks/marshell.json"],
            "{:?}",
            cs.edits
        );
        assert!(
            cs.reason.ends_with(
                "Ubuntu already has a status line, so Marshell leaves it. The row won't show the model or context use."
            ),
            "{}",
            cs.reason
        );
    }

    #[test]
    fn a_distro_remove_takes_out_the_line_it_added_and_leaves_theirs() {
        let bridge = distro_bridge();
        let ours = with_distro_files(&State::default(), |files| install_plan(files, &bridge, &BOTH)).unwrap();
        let hooks_text = match &ours.edits[0].op {
            EditOp::CreateFile { content } => content.clone(),
            op => panic!("{op:?}"),
        };
        let line = statusline_command(&bridge).unwrap();
        let added = State {
            hooks: Some(hooks_text.clone()),
            settings: Some(format!(
                "{{\"statusLine\": {{\"type\": \"command\", \"command\": {}}}}}",
                serde_json::to_string(&line).unwrap()
            )),
            ..State::default()
        };
        let plan = with_distro_files(&added, |files| remove_plan(files, Some("Ubuntu"))).unwrap();
        assert!(!plan.statusline_left);
        assert_eq!(
            distro_paths(&plan.changes),
            ["/home/u/.copilot/hooks/marshell.json", "/home/u/.copilot/settings.json"]
        );
        assert!(
            matches!(&plan.changes.edits[1].op, EditOp::JsonRemove { pointer } if pointer == "/statusLine"),
            "{:?}",
            plan.changes.edits
        );

        let theirs = State {
            hooks: Some(hooks_text),
            ..State::settings(&their_settings())
        };
        let plan = with_distro_files(&theirs, |files| remove_plan(files, Some("Ubuntu"))).unwrap();
        assert!(plan.statusline_left);
        assert_eq!(distro_paths(&plan.changes), ["/home/u/.copilot/hooks/marshell.json"]);
    }
}
