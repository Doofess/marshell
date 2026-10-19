//! Claude Code's hook entries and status line, planned as config changes (PLAN §4, §6). Nothing here
//! touches a file: a plan is built from the text of the settings file and the chain file, and the
//! config service previews, snapshots and writes it. Every doubt fails closed: a file that does not
//! parse, or a `hooks` value of the wrong shape, gives an error and no edits.

use crate::config::hash::hash_bytes;
use crate::config::jsonc::{parse_problem, read_value};
use crate::paths;
use marshell_protocol::agents::{HookInstall, HookPlanRequest};
use marshell_protocol::config::{Applies, ChangeSet, ConfigScope, EditOp, FileEdit};
use serde_json::{Value, json};
use std::fmt;
use std::path::{Path, PathBuf};

/// What marks an entry or a status line as Marshell's.
pub const TAG: &str = "mshl1";

/// The events whose hooks carry live status, in the order they are installed.
pub const STATUS_EVENTS: [&str; 8] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Notification",
    "Stop",
    "SubagentStop",
    "SessionEnd",
];

const AGENT: &str = "claude";
const HOOK_TIMEOUT: u64 = 5;
/// `SessionEnd` may take 3 s at most.
const SESSION_END_TIMEOUT: u64 = 2;

#[derive(Debug, PartialEq)]
pub enum PlanError {
    /// The settings file is not JSON(C); `problem` is the line and what is wrong.
    DoesNotParse { path: PathBuf, problem: (u32, String) },
    /// A value Claude Code expects to be an object or array is something else, at `pointer`.
    NotAnObject { path: PathBuf, pointer: String },
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::DoesNotParse { path, problem } => {
                write!(
                    f,
                    "{} does not parse (line {}): {}",
                    path.display(),
                    problem.0,
                    problem.1
                )
            }
            PlanError::NotAnObject { path, pointer } => {
                write!(
                    f,
                    "{} has a value at {pointer} of a kind Claude Code does not use",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for PlanError {}

/// What removing the hooks would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovePlan {
    pub changes: ChangeSet,
    /// The person changed `statusLine` since the install; it is left alone.
    pub statusline_left: bool,
}

/// `MARSHELL_CLAUDE_SETTINGS` when it is set and not empty, else `<user home>/.claude/settings.json`.
pub fn settings_path(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> PathBuf {
    match env("MARSHELL_CLAUDE_SETTINGS") {
        Some(path) if !path.trim().is_empty() => PathBuf::from(path),
        _ => user_home.join(".claude").join("settings.json"),
    }
}

/// Which config layer a Claude settings file is: `settings.local.json` is Local, the one in
/// `<user home>/.claude` is User, any other is the project's.
pub fn scope_of(path: &Path, user_home: &Path) -> ConfigScope {
    if path
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("settings.local.json"))
    {
        return ConfigScope::Local;
    }
    let normal = |p: &Path| {
        let s = forward(p).trim_end_matches('/').to_string();
        if cfg!(windows) { s.to_lowercase() } else { s }
    };
    if normal(path) == normal(&user_home.join(".claude").join("settings.json")) {
        ConfigScope::User
    } else {
        ConfigScope::Project
    }
}

fn forward(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn bridge_path(home: &Path) -> String {
    forward(&paths::bridge_exe(home))
}

/// The status line command: the quoted bridge path, forward slashes (PLAN §4).
pub fn statusline_command(home: &Path) -> String {
    format!("\"{}\" statusline --agent {AGENT} --tag {TAG}", bridge_path(home))
}

fn hook_entry(home: &Path, event: &str) -> Value {
    json!({
        "type": "command",
        "command": bridge_path(home),
        "args": ["event", "--agent", AGENT, "--tag", TAG],
        "timeout": if event == "SessionEnd" { SESSION_END_TIMEOUT } else { HOOK_TIMEOUT },
    })
}

/// Marshell's own: `args` hold `--tag` then the tag, or the command string says so. The bridge's
/// path is not part of the match, so an orphan from another home is still found.
fn is_ours(entry: &Value) -> bool {
    let in_args = entry.get("args").and_then(Value::as_array).is_some_and(|args| {
        args.windows(2)
            .any(|w| w[0].as_str() == Some("--tag") && w[1].as_str() == Some(TAG))
    });
    let in_command = entry
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains(&format!("--tag {TAG}")));
    in_args || in_command
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
    if let Some(line) = root.get("statusLine")
        && !line.is_null()
        && !line.is_object()
    {
        return Err(not_an_object(path, "/statusLine"));
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

/// How many of Marshell's entries the settings hold, in any event.
fn ours_total(settings: &Value) -> usize {
    entries_of(settings).filter(|(_, e)| is_ours(e)).count()
}

/// Every hook entry as (event, entry); anything of an unexpected shape is skipped.
fn entries_of(settings: &Value) -> impl Iterator<Item = (&str, &Value)> {
    settings
        .get("hooks")
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

/// A command that is Marshell's status line.
fn line_is_ours(line: &Value) -> bool {
    line.get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains(&format!("--tag {TAG}")))
}

// ---- inspect ----

pub struct Found {
    pub install: HookInstall,
    /// A status line that is not Marshell's, or the one the chain keeps.
    pub has_own_statusline: bool,
    /// Ours is in place and the chain holds the user's.
    pub chained: bool,
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

/// What the settings (and the chain file's value, `{"statusLine": ...}`) say about the install.
pub fn inspect(settings: &Value, chain: Option<&Value>, home: &Path) -> Found {
    let line = settings.get("statusLine").filter(|l| l.is_object());
    let line_ours = line.is_some_and(line_is_ours);

    let chain_holds_users = chain_holds_users_line(chain);
    let chain_usable = chain_usable(chain);
    let line_current =
        line.is_some_and(|l| l.get("command").and_then(Value::as_str) == Some(&statusline_command(home)));

    let expected = bridge_path(home);
    let exact_events = STATUS_EVENTS
        .iter()
        .filter(|event| {
            let mut ours = entries_of(settings).filter(|(e, entry)| e == *event && is_ours(entry));
            match (ours.next(), ours.next()) {
                (Some((_, entry)), None) => {
                    entry["command"] == expected.as_str()
                        && entry["args"] == json!(["event", "--agent", AGENT, "--tag", TAG])
                }
                _ => false,
            }
        })
        .count();
    let total = ours_total(settings);

    let install = if total == 0 && !line_ours {
        HookInstall::None
    } else if !paths::bridge_exe(home).exists() {
        HookInstall::Orphaned
    } else if exact_events == STATUS_EVENTS.len()
        && total == exact_events
        && (!line_ours || (chain_usable && line_current))
    {
        HookInstall::Installed
    } else {
        HookInstall::Partial
    };
    Found {
        install,
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

fn remove_op(pointer: String) -> EditOp {
    EditOp::JsonRemove { pointer }
}

/// The plan that adds the 8 status hooks and, as asked, the status line.
///
/// Marshell's existing entries come out first (so a rerun never duplicates), then one group per
/// event goes in at the end, after the user's own. A status line that is already Marshell's keeps its
/// place and is only pointed at this home's bridge. The chain is written only when it has nothing usable,
/// and then as null, so the chain never holds Marshell's own command.
pub fn install_plan(
    settings_path: &Path,
    user_home: &Path,
    text: Option<&str>,
    chain_text: Option<&str>,
    home: &Path,
    want: &HookPlanRequest,
) -> Result<ChangeSet, PlanError> {
    let title = "Set up Claude Code hooks".to_string();
    if !want.status {
        return Ok(ChangeSet {
            title,
            reason: "Nothing was asked for, so nothing changes.".to_string(),
            edits: Vec::new(),
        });
    }
    let settings = parse_settings(settings_path, text)?;
    let scope = scope_of(settings_path, user_home);
    let mut ops: Vec<EditOp> = removal_pointers(&settings, false).into_iter().map(remove_op).collect();

    let existing = settings.get("hooks").and_then(Value::as_object);
    for event in STATUS_EVENTS {
        let group = json!({ "hooks": [hook_entry(home, event)] });
        if existing.is_some_and(|h| h.contains_key(event)) {
            ops.push(set(format!("/hooks/{}/-", esc(event)), &group));
        } else {
            // Setting `/-` under a missing event would make an object with a "-" key.
            ops.push(set(format!("/hooks/{}", esc(event)), &json!([group])));
        }
    }

    let ours = statusline_command(home);
    let mut chain_edit = None;
    let line = settings.get("statusLine").filter(|l| !l.is_null());
    let how = "Adds 8 hook entries so Marshell can show what Claude Code is doing";
    let mut tail = "";
    match line {
        Some(line) if line_is_ours(line) => {
            // Ours already: the user's line is never read back from it. A line that runs another home's bridge
            // is pointed at this one, and a chain with nothing to forward to says the user had none.
            if line.get("command").and_then(Value::as_str) != Some(&ours) {
                ops.push(set("/statusLine/command", &json!(ours)));
            }
            let saved = chain_text.and_then(|t| read_value(t).ok());
            if !chain_usable(saved.as_ref()) {
                chain_edit = Some(set("/statusLine", &Value::Null));
                tail = ", and runs Marshell's status line on its own, because the saved copy of yours is missing";
            }
        }
        Some(line) if want.statusline => {
            ops.push(set("/statusLine/command", &json!(ours)));
            chain_edit = Some(set("/statusLine", line));
            tail = ", and passes your status line through Marshell, which runs it as before";
        }
        Some(_) => {}
        None => {
            ops.push(set("/statusLine", &json!({ "type": "command", "command": ours })));
            chain_edit = Some(set("/statusLine", &Value::Null));
            tail = ", and adds a status line so context and plan data arrive";
        }
    }

    let mut edits: Vec<FileEdit> = ops.into_iter().map(|op| edit(settings_path, scope, op, text)).collect();
    if let Some(op) = chain_edit {
        let chain = paths::statusline_chain(home, AGENT);
        edits.push(edit(&chain, ConfigScope::Marshell, op, chain_text));
    }
    Ok(ChangeSet {
        title,
        reason: format!("{how}{tail}."),
        edits,
    })
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

fn saved_line(chain_text: Option<&str>) -> Saved {
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
/// The chain file is read and never changed. A status line that is no longer Marshell's was
/// changed by the person since the install and is left alone.
pub fn remove_plan(
    settings_path: &Path,
    user_home: &Path,
    text: Option<&str>,
    chain_text: Option<&str>,
    _home: &Path,
) -> Result<RemovePlan, PlanError> {
    let title = "Turn Claude Code hooks off".to_string();
    let settings = parse_settings(settings_path, text)?;
    let scope = scope_of(settings_path, user_home);
    let mut ops: Vec<EditOp> = removal_pointers(&settings, true).into_iter().map(remove_op).collect();
    let removed = ours_total(&settings);

    let mut statusline_left = false;
    let mut line_note = "";
    match settings.get("statusLine") {
        Some(line) if line.is_object() && line_is_ours(line) => match saved_line(chain_text) {
            Saved::Command(command) => {
                ops.push(set("/statusLine/command", &json!(command)));
                line_note = "puts your status line back";
            }
            Saved::Nothing => {
                ops.push(remove_op("/statusLine".to_string()));
                line_note = "removes the status line it added";
            }
            Saved::Unusable => {
                ops.push(remove_op("/statusLine".to_string()));
                line_note = "removes its status line, because the saved copy of yours is missing and there is nothing to put back";
            }
        },
        Some(line) if !line.is_null() && chain_text.is_some() => {
            statusline_left = true;
            line_note = "leaves your status line alone, because it was changed since the setup";
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
        (true, _) => format!("It {line_note}."),
    };
    Ok(RemovePlan {
        changes: ChangeSet {
            title,
            reason,
            edits: ops.into_iter().map(|op| edit(settings_path, scope, op, text)).collect(),
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
    };

    fn home() -> PathBuf {
        PathBuf::from(HOME)
    }
    fn user_home() -> PathBuf {
        PathBuf::from("C:/Users/test user")
    }
    fn sfile() -> PathBuf {
        PathBuf::from("C:/proj/.claude/settings.local.json")
    }
    fn chain_path() -> PathBuf {
        paths::statusline_chain(&home(), "claude")
    }
    fn bridge() -> String {
        format!("{HOME}/bin/marshell-hook{}", std::env::consts::EXE_SUFFIX)
    }
    fn ours_statusline() -> String {
        format!("\"{}\" statusline --agent claude --tag mshl1", bridge())
    }
    fn entry(event: &str) -> Value {
        json!({
            "type": "command",
            "command": bridge(),
            "args": ["event", "--agent", "claude", "--tag", "mshl1"],
            "timeout": if event == "SessionEnd" { 2 } else { 5 },
        })
    }
    fn fixture(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/claude/settings")
            .join(name);
        std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
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
            home,
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
            &home(),
        )
        .unwrap();
        let after = apply(&plan.changes, files);
        (plan, after)
    }

    fn pointers(cs: &ChangeSet, path: &Path) -> Vec<String> {
        cs.edits
            .iter()
            .filter(|e| e.path == path.to_string_lossy())
            .map(|e| match &e.op {
                EditOp::JsonSet { pointer, .. } | EditOp::JsonRemove { pointer } => pointer.clone(),
                other => panic!("unexpected op {other:?}"),
            })
            .collect()
    }
    /// How many hook entries carry the tag, wherever they sit.
    fn ours_count(v: &Value) -> usize {
        v["hooks"].to_string().matches("\"--tag\"").count()
    }
    fn tagged(v: &Value) -> bool {
        v.to_string().contains("--tag mshl1") || v.to_string().contains("\"--tag\",\"mshl1\"")
    }

    #[test]
    fn settings_path_prefers_a_non_empty_override() {
        let user = user_home();
        let default = user.join(".claude").join("settings.json");
        let with = |v: Option<&'static str>| {
            move |k: &str| (k == "MARSHELL_CLAUDE_SETTINGS").then(|| v.map(String::from)).flatten()
        };
        assert_eq!(
            super::settings_path(&with(Some("D:/x/settings.local.json")), &user),
            PathBuf::from("D:/x/settings.local.json")
        );
        assert_eq!(super::settings_path(&with(Some("")), &user), default);
        assert_eq!(super::settings_path(&with(None), &user), default);
    }

    #[test]
    fn scope_follows_the_file() {
        let user = user_home();
        assert_eq!(
            scope_of(Path::new("C:/p/.claude/settings.local.json"), &user),
            ConfigScope::Local
        );
        assert_eq!(
            scope_of(&user.join(".claude").join("settings.json"), &user),
            ConfigScope::User
        );
        assert_eq!(
            scope_of(Path::new("C:/p/.claude/settings.json"), &user),
            ConfigScope::Project
        );
        let cs = try_install(&Files::settings("{}\n"), BOTH, &home()).unwrap();
        let scopes: Vec<_> = cs.edits.iter().map(|e| (e.path.clone(), e.scope)).collect();
        assert!(scopes.contains(&(sfile().to_string_lossy().into_owned(), ConfigScope::Local)));
        assert!(scopes.contains(&(chain_path().to_string_lossy().into_owned(), ConfigScope::Marshell)));
    }

    #[test]
    fn statusline_command_is_quoted_with_forward_slashes() {
        assert_eq!(super::statusline_command(&home()), ours_statusline());
    }

    #[test]
    fn install_appends_one_group_per_event() {
        let (cs, after) = install(&Files::settings("{}\n"), BOTH);
        assert_eq!(cs.title, "Set up Claude Code hooks");
        let v = after.settings_value();
        let events: Vec<&str> = v["hooks"].as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(events, STATUS_EVENTS);
        for event in STATUS_EVENTS {
            assert_eq!(v["hooks"][event], json!([{ "hooks": [entry(event)] }]), "{event}");
        }
        assert_eq!(v["hooks"]["SessionEnd"][0]["hooks"][0]["timeout"], 2);
        assert_eq!(v["hooks"]["Stop"][0]["hooks"][0]["timeout"], 5);
    }

    #[test]
    fn install_creates_a_missing_settings_file() {
        let (cs, after) = install(&Files::default(), BOTH);
        assert!(cs.edits.iter().all(|e| e.expected_hash.is_none()));
        assert_eq!(ours_count(&after.settings_value()), 8);
    }

    #[test]
    fn install_with_status_off_has_no_edits() {
        let want = HookPlanRequest {
            status: false,
            statusline: true,
        };
        let cs = try_install(&Files::settings("{}\n"), want, &home()).unwrap();
        assert!(cs.edits.is_empty());
    }

    #[test]
    fn install_keeps_the_users_hooks_first() {
        let before = Files::settings(&fixture("user-hooks.json"));
        let (_, after) = install(&before, BOTH);
        let (b, a) = (before.settings_value(), after.settings_value());
        assert_eq!(a["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(a["hooks"]["Stop"][0], b["hooks"]["Stop"][0]);
        assert_eq!(a["hooks"]["Stop"][1], json!({ "hooks": [entry("Stop")] }));
        assert_eq!(a["hooks"]["PreToolUse"][0], b["hooks"]["PreToolUse"][0]);
        assert_eq!(a["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn install_chains_an_existing_status_line() {
        let original = json!({"type": "command", "command": "python ./line.py", "padding": 2, "refreshInterval": 5});
        let before = Files::settings(&json!({ "statusLine": original }).to_string());
        let (cs, after) = install(&before, BOTH);
        let status = after.settings_value()["statusLine"].clone();
        assert_eq!(status["command"], ours_statusline());
        assert_eq!(status["padding"], 2);
        assert_eq!(status["refreshInterval"], 5);
        assert_eq!(status["type"], "command");
        let ps = pointers(&cs, &sfile());
        assert!(ps.contains(&"/statusLine/command".to_string()) && !ps.contains(&"/statusLine".to_string()));
        assert_eq!(pointers(&cs, &chain_path()), ["/statusLine"]);
        assert_eq!(after.chain_value(), json!({ "statusLine": original }));
    }

    #[test]
    fn install_adds_a_status_line_when_there_is_none() {
        let (cs, after) = install(&Files::settings(&fixture("no-statusline.json")), BOTH);
        assert_eq!(
            after.settings_value()["statusLine"],
            json!({"type": "command", "command": ours_statusline()})
        );
        assert_eq!(after.chain_value(), json!({ "statusLine": null }));
        assert_eq!(pointers(&cs, &chain_path()), ["/statusLine"]);
        // The headline switch does not matter when there is nothing of the user's to keep.
        let off = HookPlanRequest {
            status: true,
            statusline: false,
        };
        let (_, after_off) = install(&Files::settings(&fixture("no-statusline.json")), off);
        assert_eq!(after_off.settings_value()["statusLine"]["command"], ours_statusline());
    }

    #[test]
    fn statusline_off_leaves_status_line_alone() {
        let before = Files::settings(&fixture("user-hooks.json"));
        let off = HookPlanRequest {
            status: true,
            statusline: false,
        };
        let (cs, after) = install(&before, off);
        assert!(pointers(&cs, &sfile()).iter().all(|p| !p.starts_with("/statusLine")));
        assert!(pointers(&cs, &chain_path()).is_empty());
        assert_eq!(after.chain, None);
        assert_eq!(
            after.settings_value()["statusLine"],
            before.settings_value()["statusLine"]
        );
        assert_eq!(ours_count(&after.settings_value()), 8);
    }

    #[test]
    fn reinstall_replaces_entries_and_keeps_the_chain() {
        let before = Files::settings(&fixture("user-hooks.json"));
        let (_, once) = install(&before, BOTH);
        let original_chain = once.chain_value();
        let (cs, twice) = install(&once, BOTH);
        let v = twice.settings_value();
        assert_eq!(ours_count(&v), 8);
        assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(v["hooks"]["Stop"][0], before.settings_value()["hooks"]["Stop"][0]);
        assert_eq!(v["statusLine"]["command"], ours_statusline());
        assert!(pointers(&cs, &chain_path()).is_empty(), "the chain is not rewritten");
        assert_eq!(twice.chain_value(), original_chain);
        assert!(!tagged(&original_chain));

        // Entries of another home's bridge are replaced, not kept beside the new ones.
        let other = PathBuf::from("D:/old home/.marshell");
        let cs = try_install(&before, BOTH, &other).unwrap();
        let other_files = Files {
            settings: Some(apply_for(&cs, &before.settings)),
            chain: None,
        };
        assert_eq!(ours_count(&other_files.settings_value()), 8);
        let other_files = Files {
            chain: Some(json!({ "statusLine": before.settings_value()["statusLine"] }).to_string()),
            ..other_files
        };
        let (cs, moved) = install(&other_files, BOTH);
        let v = moved.settings_value();
        assert_eq!(ours_count(&v), 8);
        assert!(v["hooks"].to_string().contains(&bridge()));
        assert!(!v["hooks"].to_string().contains("old home"));
        assert!(pointers(&cs, &chain_path()).is_empty());
        assert!(!tagged(&moved.chain_value()));
    }

    /// The settings text after the settings edits of `cs` (the chain edits are ignored).
    fn apply_for(cs: &ChangeSet, text: &Option<String>) -> String {
        let files = Files {
            settings: text.clone(),
            chain: None,
        };
        let only: Vec<FileEdit> = cs
            .edits
            .iter()
            .filter(|e| e.path == sfile().to_string_lossy())
            .cloned()
            .collect();
        let cs = ChangeSet {
            edits: only,
            ..cs.clone()
        };
        apply(&cs, &files).settings.unwrap()
    }

    #[test]
    fn install_never_writes_marshells_own_command_into_the_chain() {
        // Ours is in place and the chain file is gone: the user's line is not known, so the chain says
        // null. The status line command is never copied into it.
        let (_, once) = install(&Files::settings(&fixture("user-hooks.json")), BOTH);
        let lost = Files {
            chain: None,
            ..once.clone()
        };
        let (_, again) = install(&lost, BOTH);
        assert_eq!(again.chain_value(), json!({ "statusLine": null }));
        assert!(!tagged(&again.chain_value()));
    }

    #[test]
    fn reinstall_repairs_a_lost_chain() {
        let (_, once) = install(&Files::settings(&fixture("user-hooks.json")), BOTH);
        let lost = Files {
            chain: None,
            ..once.clone()
        };
        let (cs, again) = install(&lost, BOTH);
        assert_eq!(pointers(&cs, &chain_path()), ["/statusLine"]);
        let chain_edit = cs.edits.iter().find(|e| e.scope == ConfigScope::Marshell).unwrap();
        assert!(matches!(&chain_edit.op, EditOp::JsonSet { value, .. } if value == "null"));
        assert!(
            cs.reason
                .ends_with(", and runs Marshell's status line on its own, because the saved copy of yours is missing."),
            "{}",
            cs.reason
        );
        // The status line is already ours, so no edit touches it.
        assert!(pointers(&cs, &sfile()).iter().all(|p| p.starts_with("/hooks")));
        assert_eq!(
            again.settings_value()["statusLine"],
            once.settings_value()["statusLine"]
        );
        assert_eq!(again.chain_value(), json!({ "statusLine": null }));
    }

    #[test]
    fn reinstall_repoints_a_line_from_another_home() {
        let other = PathBuf::from("D:/old home/.marshell");
        let moved = installed_for(&fixture("user-hooks.json"), &other);
        let original_chain = moved.chain_value();
        assert_ne!(moved.settings_value()["statusLine"]["command"], ours_statusline());

        let (cs, again) = install(&moved, BOTH);
        let v = again.settings_value();
        assert_eq!(v["statusLine"]["command"], ours_statusline());
        assert_eq!(ours_count(&v), 8);
        assert!(pointers(&cs, &sfile()).contains(&"/statusLine/command".to_string()));
        // The chain already holds the user's line: it stays as it is.
        assert!(pointers(&cs, &chain_path()).is_empty());
        assert_eq!(again.chain_value(), original_chain);
    }

    #[test]
    fn install_fails_closed_on_a_hooks_array() {
        let cases = [
            (r#"{"hooks": []}"#, "/hooks"),
            (r#"{"hooks": null}"#, "/hooks"),
            (r#"{"hooks": {"Stop": {}}}"#, "/hooks/Stop"),
            (r#"{"hooks": {"Stop": [{"hooks": {}}]}}"#, "/hooks/Stop/0/hooks"),
        ];
        for (text, pointer) in cases {
            let files = Files::settings(text);
            match try_install(&files, BOTH, &home()) {
                Err(PlanError::NotAnObject { path, pointer: p }) => {
                    assert_eq!(path, sfile(), "{text}");
                    assert_eq!(p, pointer, "{text}");
                }
                other => panic!("{text}: {other:?}"),
            }
            let removed = remove_plan(&sfile(), &user_home(), Some(text), None, &home());
            assert!(matches!(removed, Err(PlanError::NotAnObject { .. })), "{text}");
        }
    }

    #[test]
    fn unparsable_settings_name_the_line() {
        let text = "{\n  \"a\": 1,\n  \"b\": }\n";
        match try_install(&Files::settings(text), BOTH, &home()) {
            Err(PlanError::DoesNotParse { path, problem }) => {
                assert_eq!(path, sfile());
                assert_eq!(problem.0, 3, "{problem:?}");
                assert!(!problem.1.is_empty());
            }
            other => panic!("{other:?}"),
        }
        let removed = remove_plan(&sfile(), &user_home(), Some(text), None, &home());
        assert!(matches!(removed, Err(PlanError::DoesNotParse { .. })));
    }

    #[test]
    fn remove_leaves_a_status_line_the_user_changed() {
        let (_, installed) = install(&Files::settings(&fixture("user-hooks.json")), BOTH);
        let edited = apply_ops(
            installed.settings.as_deref().unwrap(),
            &[JsonOp::Set {
                pointer: "/statusLine/command",
                value: &json!("node ./mine.js"),
            }],
        )
        .unwrap();
        let files = Files {
            settings: Some(edited),
            chain: installed.chain.clone(),
        };
        let (plan, after) = remove(&files);
        assert!(plan.statusline_left);
        assert!(
            pointers(&plan.changes, &sfile())
                .iter()
                .all(|p| !p.starts_with("/statusLine"))
        );
        assert!(pointers(&plan.changes, &chain_path()).is_empty());
        let v = after.settings_value();
        assert_eq!(v["statusLine"]["command"], "node ./mine.js");
        assert_eq!(ours_count(&v), 0);
        assert!(plan.changes.reason.contains("status line"), "{}", plan.changes.reason);
    }

    #[test]
    fn remove_restores_the_users_status_line_and_never_touches_the_chain() {
        let before = Files::settings(&fixture("user-hooks.json"));
        let (_, installed) = install(&before, BOTH);
        let (plan, after) = remove(&installed);
        assert!(!plan.statusline_left);
        assert_eq!(plan.changes.title, "Turn Claude Code hooks off");
        assert!(pointers(&plan.changes, &chain_path()).is_empty());
        assert_eq!(after.chain, installed.chain);
        assert_eq!(after.settings, before.settings);
    }

    #[test]
    fn remove_takes_out_a_status_line_we_added() {
        let (_, installed) = install(&Files::settings(&fixture("no-statusline.json")), BOTH);
        let (plan, after) = remove(&installed);
        assert!(after.settings_value().get("statusLine").is_none());
        assert!(!plan.statusline_left);
    }

    #[test]
    fn remove_without_a_chain_takes_our_status_line_out_and_says_so() {
        let (_, installed) = install(&Files::settings(&fixture("user-hooks.json")), BOTH);
        for chain in [
            None,
            Some("{ not json".to_string()),
            Some(json!({"statusLine": {"command": ours_statusline()}}).to_string()),
        ] {
            let files = Files {
                chain,
                ..installed.clone()
            };
            let (plan, after) = remove(&files);
            assert!(after.settings_value().get("statusLine").is_none());
            assert!(
                plan.changes.reason.contains("nothing to put back"),
                "{}",
                plan.changes.reason
            );
            assert_eq!(ours_count(&after.settings_value()), 0);
        }
    }

    #[test]
    fn remove_finds_orphans_by_tag() {
        let text = json!({
            "hooks": {
                "Stop": [
                    { "hooks": [{
                        "type": "command", "command": "C:/gone/bin/marshell-hook.exe",
                        "args": ["event", "--agent", "claude", "--tag", "mshl1"], "timeout": 5
                    }] }
                ],
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [
                        { "type": "command", "command": "python ./guard.py" },
                        { "type": "command", "command": "\"C:/gone/bin/marshell-hook.exe\" event --agent claude --tag mshl1" }
                    ] }
                ],
                "Notification": [
                    { "hooks": [{ "type": "command", "command": "echo --tag mshl2" }] }
                ]
            }
        })
        .to_string();
        let (_, after) = remove(&Files::settings(&text));
        let v = after.settings_value();
        assert!(v["hooks"].get("Stop").is_none(), "the emptied event is dropped");
        assert_eq!(
            v["hooks"]["PreToolUse"],
            json!([{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "python ./guard.py" }] }])
        );
        assert_eq!(v["hooks"]["Notification"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn remove_on_a_missing_or_clean_file_has_no_edits() {
        let (plan, _) = remove(&Files::default());
        assert!(plan.changes.edits.is_empty());
        let (plan, _) = remove(&Files::settings(&fixture("user-hooks.json")));
        assert!(plan.changes.edits.is_empty());
        assert!(!plan.statusline_left);
    }

    /// A bridge file under a scratch home, or none.
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
    /// The files an install into `text` leaves, planned for `home`.
    fn installed_for(text: &str, home: &Path) -> Files {
        let before = Files::settings(text);
        let cs = try_install(&before, BOTH, home).unwrap();
        let chain_edits: Vec<FileEdit> = cs
            .edits
            .iter()
            .filter(|e| e.scope == ConfigScope::Marshell)
            .cloned()
            .collect();
        let settings = apply_for(&cs, &before.settings);
        let chain = chain_edits.first().map(|e| match &e.op {
            EditOp::JsonSet { value, .. } => format!("{{\"statusLine\": {value}}}"),
            other => panic!("{other:?}"),
        });
        Files {
            settings: Some(settings),
            chain,
        }
    }

    #[test]
    fn inspect_states() {
        let (_keep, home) = scratch_home(true);
        let (_keep2, no_bridge) = scratch_home(false);

        let none = inspect(&json!({}), None, &home);
        assert_eq!(
            (none.install, none.has_own_statusline, none.chained),
            (HookInstall::None, false, false)
        );

        // Installed over the user's own status line: chained.
        let full = installed_for(&fixture("user-hooks.json"), &home);
        let chain = full.chain_value();
        let found = inspect(&full.settings_value(), Some(&chain), &home);
        assert_eq!(
            (found.install, found.has_own_statusline, found.chained),
            (HookInstall::Installed, true, true)
        );

        // Installed with nothing to chain: the chain says null.
        let plain = installed_for(&fixture("no-statusline.json"), &home);
        let chain = plain.chain_value();
        let found = inspect(&plain.settings_value(), Some(&chain), &home);
        assert_eq!(
            (found.install, found.has_own_statusline, found.chained),
            (HookInstall::Installed, false, false)
        );

        // Only 7 events.
        let seven = apply_ops(
            full.settings.as_deref().unwrap(),
            &[JsonOp::Remove {
                pointer: "/hooks/SessionEnd",
            }],
        )
        .unwrap();
        let chain = full.chain_value();
        let found = inspect(&read_value(&seven).unwrap(), Some(&chain), &home);
        assert_eq!(found.install, HookInstall::Partial);

        // The bridge file is gone.
        let found = inspect(&full.settings_value(), Some(&chain), &no_bridge);
        assert_eq!(found.install, HookInstall::Orphaned);

        // A different home's bridge in the entries.
        let moved = installed_for(&fixture("user-hooks.json"), &no_bridge);
        let found = inspect(&moved.settings_value(), Some(&chain), &home);
        assert_eq!(found.install, HookInstall::Partial);

        // Our status line with no chain to forward to.
        let found = inspect(&full.settings_value(), None, &home);
        assert_eq!(found.install, HookInstall::Partial);
        assert!(!found.chained);

        // The user's status line, not ours: reported, not chained.
        let own = json!({"statusLine": {"type": "command", "command": "python ./line.py"}});
        let found = inspect(&own, None, &home);
        assert_eq!(
            (found.install, found.has_own_statusline, found.chained),
            (HookInstall::None, true, false)
        );
    }

    /// The files after the install plan for `home` is applied: the settings edits through `apply_for`, and
    /// the chain, which is one whole-value edit, as `{"statusLine": <value>}`.
    fn installed_over(before: &Files, home: &Path) -> Files {
        let cs = try_install(before, BOTH, home).unwrap();
        let chain = cs
            .edits
            .iter()
            .find(|e| e.scope == ConfigScope::Marshell)
            .map(|e| match &e.op {
                EditOp::JsonSet { pointer, value } => {
                    assert_eq!(pointer, "/statusLine");
                    format!("{{\"statusLine\": {value}}}")
                }
                other => panic!("{other:?}"),
            })
            .or_else(|| before.chain.clone());
        Files {
            settings: Some(apply_for(&cs, &before.settings)),
            chain,
        }
    }

    #[test]
    fn every_partial_state_installs_to_installed() {
        let (_keep, home) = scratch_home(true);
        let other = PathBuf::from("D:/old home/.marshell");
        let other_line = statusline_command(&other);
        let other_bridge = bridge_path(&other);

        type Break = fn(&mut Value, &mut Option<String>, &str, &str);
        let breaks: [(&str, Break); 6] = [
            ("one event missing", |s, _, _, _| {
                s["hooks"].as_object_mut().unwrap().remove("SessionEnd");
            }),
            ("a duplicate entry", |s, _, _, _| {
                let groups = s["hooks"]["Stop"].as_array_mut().unwrap();
                let ours = groups.last().unwrap().clone();
                groups.push(ours);
            }),
            ("the wrong bridge in an entry", |s, _, _, bridge| {
                let groups = s["hooks"]["Stop"].as_array_mut().unwrap();
                groups.last_mut().unwrap()["hooks"][0]["command"] = json!(bridge);
            }),
            ("a lost chain", |_, chain, _, _| *chain = None),
            ("a chain that holds our own line", |s, chain, _, _| {
                *chain = Some(json!({ "statusLine": s["statusLine"] }).to_string());
            }),
            ("a status line from another home", |s, _, line, _| {
                s["statusLine"]["command"] = json!(line);
            }),
        ];
        for fixture_name in ["user-hooks.json", "no-statusline.json"] {
            let full = installed_over(&Files::settings(&fixture(fixture_name)), &home);
            let found = inspect(&full.settings_value(), Some(&full.chain_value()), &home);
            assert_eq!(found.install, HookInstall::Installed, "{fixture_name}: the baseline");
            for (name, break_it) in breaks {
                let mut settings = full.settings_value();
                let mut chain = full.chain.clone();
                break_it(&mut settings, &mut chain, &other_line, &other_bridge);
                let broken = Files {
                    settings: Some(settings.to_string()),
                    chain,
                };
                let chain_value = broken.chain.as_deref().and_then(|t| read_value(t).ok());
                let found = inspect(&broken.settings_value(), chain_value.as_ref(), &home);
                assert_eq!(found.install, HookInstall::Partial, "{fixture_name}: {name} is Partial");

                let fixed = installed_over(&broken, &home);
                let chain_value = fixed.chain.as_deref().and_then(|t| read_value(t).ok());
                let found = inspect(&fixed.settings_value(), chain_value.as_ref(), &home);
                assert_eq!(found.install, HookInstall::Installed, "{fixture_name}: {name} installs");
                assert!(
                    !tagged(&chain_value.unwrap()),
                    "{fixture_name}: {name} chain holds ours"
                );
            }
        }
    }

    #[test]
    fn byte_identity_install_then_remove() {
        let crlf = fixture("crlf.json").replace('\n', "\r\n");
        let bom = format!("\u{feff}{}", fixture("bom.json"));
        let cases: Vec<(&str, String, bool)> = vec![
            ("empty object", fixture("empty-object.json"), true),
            ("user hooks", fixture("user-hooks.json"), true),
            (
                "comments and trailing commas",
                fixture("comments-trailing-commas.json"),
                true,
            ),
            ("crlf", crlf, true),
            ("bom", bom, true),
            ("status line comment", fixture("statusline-comment.json"), true),
            ("no status line", fixture("no-statusline.json"), true),
            ("hooks already present", fixture("hooks-empty.json"), false),
        ];
        for (name, text, exact) in cases {
            let (_, installed) = install(&Files::settings(&text), BOTH);
            assert_ne!(
                installed.settings.as_deref(),
                Some(text.as_str()),
                "{name}: install changed nothing"
            );
            let (_, removed) = remove(&installed);
            let got = removed.settings.unwrap();
            if exact {
                assert_eq!(got, text, "{name}");
            } else {
                // Fallback for this fixture only: `"hooks": {}` was there before the install, but once
                // the entries are out an empty `hooks` is indistinguishable from one the install
                // created, and removing it is right for the other fixtures. So the value differs by
                // exactly that empty object.
                let mut want = read_value(&text).unwrap();
                want.as_object_mut().unwrap().shift_remove("hooks");
                assert_eq!(read_value(&got).unwrap(), want, "{name}: {got}");
            }
        }
    }
}
