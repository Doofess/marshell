//! The Doctor's runs: `POST /v1/doctor/run` starts the checks (or joins the run that is going) and answers 202 with
//! the summary; `GET /v1/doctor/report` answers the last report, or 204 before the first run ends. The core also
//! runs once at startup (see `AppState::with_doctor_on_start`). A run reads the files of every CLI off the async
//! threads, builds the report with `doctor::checks` and publishes the summary on the events socket.

use super::AppState;
use super::agents::{
    Adapter, Refusal, adapter, agy_setup_of, changed_since, changes_of, codex_setup_of, hook_install_changes,
    qwen_setup_of, settings_file, setup_of,
};
use crate::agents::common::Gate;
use crate::config::guard::unverbatim;
use crate::config::hash::{hash_bytes, hash_file};
use crate::config::jsonc::{parse_problem, read_value};
use crate::config::managed::{self, ManagedEntry};
use crate::config::service::ApplyError;
use crate::doctor::{AgentFacts, FileState, PendingFix, checks};
use crate::paths;
use crate::store::{self, Versioned};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::agents::{AgentSetup, HookPlanRequest, SettingsState};
use marshell_protocol::api::SessionState;
use marshell_protocol::config::{
    Applies, ApplyConflict, ApplyRequest, ApplyResult, ChangeSet, ConfigScope, EditOp, ExpectedHash, FileEdit,
    SnapshotFileInfo,
};
use marshell_protocol::doctor::{
    DoctorFixPlan, DoctorFixRequest, DoctorItem, DoctorLastFix, DoctorReport, DoctorSummary,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

type Fail = (StatusCode, String);

/// The CLIs the Doctor checks, in the report's order.
const AGENTS: [&str; 4] = ["claude", "codex", "antigravity", "qwen"];

pub async fn run(State(st): State<AppState>) -> (StatusCode, Json<DoctorSummary>) {
    request_run(&st);
    (StatusCode::ACCEPTED, Json(st.doctor().summary.borrow().clone()))
}

pub async fn report(State(st): State<AppState>) -> Response {
    // Cloned under the lock, which is released before the reply is built.
    let report = st.doctor().report.lock().unwrap_or_else(|e| e.into_inner()).clone();
    match report {
        Some(report) => Json(report).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

/// Starts a run of the checks unless one is going, in which case the request joins that run. The summary reads
/// `running` as soon as this returns. Needs a Tokio runtime.
pub(crate) fn request_run(st: &AppState) {
    request_run_with(st, Arc::new(facts_of));
}

/// Reads what the checks need, off the async threads. A test swaps it to hold a run.
type ReadFacts = Arc<dyn Fn(&AppState) -> Gathered + Send + Sync>;

fn request_run_with(st: &AppState, read: ReadFacts) {
    let doctor = st.doctor();
    {
        // `running` and `again` change together under one lock (see `RunFlags`), and the summary is updated under it
        // too, so the end of a run and a new request cannot write `running` in the wrong order. Nothing here awaits.
        let mut flags = doctor.run.lock().unwrap_or_else(|e| e.into_inner());
        if flags.running {
            // A fix may have been applied after this run read its files: one more run follows it, however many
            // requests arrive meanwhile.
            flags.again = true;
            return;
        }
        flags.running = true;
        doctor.summary.send_modify(|s| s.running = true);
    }
    let st = st.clone();
    tokio::spawn(async move {
        loop {
            let finished = run_once(&st, read.clone()).await;
            let doctor = st.doctor();
            let mut flags = doctor.run.lock().unwrap_or_else(|e| e.into_inner());
            let again = std::mem::take(&mut flags.again);
            // `running` stays true in the summary while another run follows.
            doctor.summary.send_modify(|s| {
                s.running = again;
                if let Some((issues, now)) = finished {
                    s.issues = issues;
                    s.checked_at_ms = Some(now);
                }
            });
            if !again {
                flags.running = false;
                return;
            }
        }
    });
}

/// One run, answering the issue count and the time of its report, or `None` when it failed. It is the only writer of
/// the report.
async fn run_once(st: &AppState, read: ReadFacts) -> Option<(u32, u64)> {
    let started = Instant::now();
    let reader = st.clone();
    let facts = tokio::task::spawn_blocking(move || read(&reader)).await;
    let doctor = st.doctor();
    match facts {
        Ok(facts) => {
            let items = checks::items(&facts.facts, &facts.stopped);
            let issues = checks::issues(&items);
            let now = now_ms();
            *doctor.report.lock().unwrap_or_else(|e| e.into_inner()) = Some(DoctorReport {
                items,
                checked_at_ms: now,
                duration_ms: started.elapsed().as_millis() as u64,
            });
            Some((issues, now))
        }
        Err(e) => {
            tracing::warn!("the doctor's run failed: {e}");
            None
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// What the checks need to know about each CLI. Reads files, so callers go through `spawn_blocking`.
pub(crate) fn facts_of(st: &AppState) -> Gathered {
    // managed.json is read once, for the Windows CLIs and for the distros. One that can't be read names no file, so no
    // CLI reads as set up and no distro as managed.
    let entries = managed_entries(st);
    let facts = windows_facts(st, &entries, |st, agent| match agent {
        "claude" => setup_of(st),
        "codex" => codex_setup_of(st),
        "antigravity" => agy_setup_of(st),
        _ => qwen_setup_of(st),
    });
    let (distro, stopped) = distro_facts(st, &entries);
    Gathered {
        facts: facts.into_iter().chain(distro).collect(),
        stopped,
    }
}

fn managed_entries(st: &AppState) -> Vec<ManagedEntry> {
    managed::list(&st.0.home).unwrap_or_else(|e| {
        tracing::warn!("the doctor could not read managed.json: {e:#}");
        Vec::new()
    })
}

/// The Windows CLIs' facts over any reader of a CLI's setup, so a test can make one of them fail.
#[cfg(test)]
fn facts_with(st: &AppState, read: impl Fn(&AppState, &'static str) -> Result<AgentSetup, Fail>) -> Vec<AgentFacts> {
    windows_facts(st, &managed_entries(st), read)
}

fn windows_facts(
    st: &AppState,
    entries: &[ManagedEntry],
    read: impl Fn(&AppState, &'static str) -> Result<AgentSetup, Fail>,
) -> Vec<AgentFacts> {
    AGENTS
        .into_iter()
        .map(|agent| match read(st, agent) {
            Ok(setup) => facts_from(st, agent, setup, entries),
            Err((_, message)) => failed_facts(agent, None, message),
        })
        .collect()
}

/// What one run reads: the facts of every CLI on Windows and in each running distro Marshell set up, and the distros it
/// set up that aren't running.
pub(crate) struct Gathered {
    pub facts: Vec<AgentFacts>,
    pub stopped: Vec<String>,
}

/// The distros Marshell set up: the detected ones that hold a path of `managed.json`. This reads no distro's file.
fn managed_distros(st: &AppState, entries: &[ManagedEntry]) -> Vec<String> {
    if entries.is_empty() {
        return Vec::new();
    }
    st.distros_of_paths(entries.iter().map(|e| Path::new(&e.path)))
}

/// The facts of every running managed distro, and the managed distros that aren't running. `wsl.exe` is asked what
/// runs only when there is a managed distro, and a distro that isn't running is never asked for its target nor read,
/// since either would start it. Runs `wsl.exe` and reads shares, so callers go through `spawn_blocking`.
fn distro_facts(st: &AppState, entries: &[ManagedEntry]) -> (Vec<AgentFacts>, Vec<String>) {
    let managed = managed_distros(st, entries);
    if managed.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let running = (st.0.running_distros)();
    let is_running = |d: &String| running.iter().any(|r| r.to_lowercase() == d.to_lowercase());
    let (up, stopped): (Vec<String>, Vec<String>) = managed.into_iter().partition(is_running);
    let mut facts = Vec::new();
    for distro in &up {
        match st.distro_target(distro) {
            Ok(target) => {
                for agent in AGENTS {
                    facts.push(distro_agent_facts(agent, distro, &target, entries));
                }
            }
            // It was running a moment ago and gave no home: each CLI says it couldn't be checked.
            Err((_, message)) => {
                for agent in AGENTS {
                    facts.push(failed_facts(agent, Some(distro.clone()), message.clone()));
                }
            }
        }
    }
    (facts, stopped)
}

/// A CLI's facts when its reader failed: every field a default, and the reason.
fn failed_facts(agent: &'static str, distro: Option<String>, message: String) -> AgentFacts {
    AgentFacts {
        agent,
        distro,
        found: false,
        path: PathBuf::new(),
        file: FileState::Ok,
        hooks: marshell_protocol::agents::HookInstall::None,
        approve_installed: false,
        has_own_statusline: false,
        chained: false,
        trust: None,
        set_up: false,
        bridge: PathBuf::new(),
        bridge_exists: false,
        version_note: None,
        retention_unset: None,
        managed_settings: None,
        error: Some(message),
    }
}

/// One file of a distro as a value and whether it is read-only: `Ok(None)` when it isn't there, and the line and
/// message when it can't be read or doesn't parse.
fn read_json(path: &Path) -> Result<Option<(serde_json::Value, bool)>, (u32, String)> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err((1, e.to_string())),
    };
    match (parse_problem(&text), read_value(&text)) {
        (None, Ok(value)) => {
            let read_only = std::fs::metadata(path).is_ok_and(|m| m.permissions().readonly());
            Ok(Some((value, read_only)))
        }
        (problem, _) => Err(problem.unwrap_or((1, "the file doesn't parse".to_string()))),
    }
}

/// What a CLI's hooks files say in a distro, judged against the distro's bridge. A distro keeps no chain file, and its
/// version, retention and organisation settings are not checked.
fn distro_agent_facts(
    agent: &'static str,
    distro: &str,
    target: &super::DistroTarget,
    entries: &[ManagedEntry],
) -> AgentFacts {
    use crate::agents::antigravity::hooks as agy;
    use crate::agents::common::Bridge;
    use crate::agents::{claude, codex, qwen};
    use marshell_protocol::agents::HookInstall;

    let home = &target.home;
    let bridge = Bridge::Distro {
        name: distro.to_string(),
        path: target.bridge.clone(),
    };
    // Antigravity's hooks are in `hooks.json`, with the status line in `settings.json`.
    let files: Vec<PathBuf> = match agent {
        "claude" => vec![home.join(".claude").join("settings.json")],
        "codex" => vec![home.join(".codex").join("hooks.json")],
        "antigravity" => vec![agy::hooks_path(home), agy::settings_path(home)],
        _ => vec![home.join(".qwen").join("settings.json")],
    };
    let path = files[0].clone();
    let mut facts = AgentFacts {
        agent,
        distro: Some(distro.to_string()),
        found: true,
        set_up: set_up_of(entries, &path),
        path,
        file: FileState::Ok,
        hooks: HookInstall::None,
        approve_installed: false,
        // A distro keeps no chain: its own status line is left alone, so nothing here says it isn't chained.
        has_own_statusline: false,
        chained: false,
        trust: None,
        bridge: PathBuf::from(&target.bridge),
        bridge_exists: bridge.exists(),
        version_note: None,
        retention_unset: None,
        managed_settings: None,
        error: None,
    };

    let mut values = Vec::new();
    let mut read_only = false;
    for file in &files {
        match read_json(file) {
            Ok(Some((value, ro))) => {
                read_only |= ro;
                values.push(Some(value));
            }
            Ok(None) => values.push(None),
            Err((line, message)) => {
                // With two files, the message names the one that failed, as on Windows.
                let message = if files.len() > 1 {
                    format!("{}: {message}", file.display())
                } else {
                    message
                };
                facts.file = FileState::Unparsable { line, message };
                return facts;
            }
        }
    }
    if values.iter().all(Option::is_none) {
        facts.file = FileState::Missing;
        return facts;
    }
    if read_only {
        facts.file = FileState::ReadOnly;
    }
    let none = serde_json::json!({});
    let first = values[0].as_ref();
    let (install, approve_installed) = match agent {
        "claude" => {
            let f = claude::hooks::inspect_for(first.unwrap_or(&none), None, &bridge);
            (f.install, f.approve_installed)
        }
        "codex" => {
            let hooks = first.unwrap_or(&none);
            let f = codex::hooks::inspect_for(hooks, &bridge);
            // Codex keys its trust by the hooks file's path as the distro sees it.
            let linux = crate::wsl::to_linux(distro, &facts.path).map_or_else(|| facts.path.clone(), PathBuf::from);
            let config_toml = std::fs::read_to_string(facts.path.with_file_name("config.toml")).ok();
            facts.trust = codex::trust::state(&linux, hooks, config_toml.as_deref());
            (f.install, f.approve_installed)
        }
        "antigravity" => {
            let f = agy::inspect_for(first, values[1].as_ref(), None, &bridge);
            (f.install, f.approve_installed)
        }
        _ => {
            let f = qwen::hooks::inspect_for(first.unwrap_or(&none), None, &bridge);
            (f.install, f.approve_installed)
        }
    };
    facts.hooks = install;
    facts.approve_installed = approve_installed;
    facts
}

fn facts_from(st: &AppState, agent: &'static str, setup: AgentSetup, entries: &[ManagedEntry]) -> AgentFacts {
    // Antigravity's hooks are in `hooks.json`; the others keep them in the file their setup names.
    let path = PathBuf::from(setup.hooks_path.as_deref().unwrap_or(&setup.settings_path));
    let file = match setup.settings_state {
        SettingsState::Ok => FileState::Ok,
        SettingsState::Missing => FileState::Missing,
        SettingsState::ReadOnly => FileState::ReadOnly,
        SettingsState::Unparsable => {
            let (line, message) = setup
                .parse_error
                .as_ref()
                .map_or((1, "the file can't be read".to_string()), |p| {
                    (p.line, p.message.clone())
                });
            FileState::Unparsable { line, message }
        }
    };
    let version_note = if setup.found {
        match version_gate(agent, setup.version.as_deref()) {
            Gate::Unknown(note) => Some(note),
            Gate::Known => None,
        }
    } else {
        None
    };
    // Only Claude Code deletes transcripts by age, and only a file that was read says whether the key is there.
    let retention_unset = (agent == "claude" && setup.found && matches!(file, FileState::Ok | FileState::ReadOnly))
        .then(|| cleanup_period_unset(&setup.settings_path))
        .flatten();
    let managed_settings = match agent {
        "claude" => managed_file_of(st, Adapter::Claude),
        "qwen" => managed_file_of(st, Adapter::Qwen),
        _ => None,
    };
    AgentFacts {
        agent,
        distro: None,
        found: setup.found,
        set_up: set_up_of(entries, &path),
        path,
        file,
        hooks: setup.hooks,
        approve_installed: setup.approve_installed,
        has_own_statusline: setup.has_own_statusline,
        chained: setup.chained,
        trust: setup.trust,
        bridge: PathBuf::from(&setup.bridge_path),
        bridge_exists: Path::new(&setup.bridge_path).exists(),
        version_note,
        retention_unset,
        managed_settings,
        error: None,
    }
}

fn version_gate(agent: &str, version: Option<&str>) -> Gate {
    use crate::agents::{antigravity, claude, codex, qwen};
    match agent {
        "claude" => claude::version_gate(version),
        "codex" => codex::version_gate(version),
        "antigravity" => antigravity::version_gate(version),
        _ => qwen::version_gate(version),
    }
}

/// `Some(true)` when the settings file has no top-level `cleanupPeriodDays`, `Some(false)` when it has one, `None`
/// when the file can't be read.
fn cleanup_period_unset(settings_path: &str) -> Option<bool> {
    let text = std::fs::read_to_string(settings_path).ok()?;
    let value = read_value(&text).ok()?;
    Some(value.get("cleanupPeriodDays").is_none())
}

/// The organisation's settings file that applies on top of the person's, with the paths the settings view reads.
fn managed_file_of(st: &AppState, kind: Adapter) -> Option<PathBuf> {
    let (_, managed) = settings_file(st, kind).ok()?;
    managed.map(|(path, _)| path)
}

/// managed.json names hooks Marshell wrote in `path`: an entry for that file whose pointer is under `/hooks` (Claude,
/// Codex and Qwen Code) or under `/marshell`, the one named hook Antigravity's `hooks.json` holds Marshell's entries
/// in. An entry without a pointer is a file Marshell created. The paths are compared after `unverbatim`, ignoring case
/// on Windows. Own edits record no keys, and a status line entry (`/statusLine`) is not a hook.
pub(crate) fn set_up_of(entries: &[ManagedEntry], path: &Path) -> bool {
    let want = comparable(path);
    entries.iter().any(|e| {
        comparable(Path::new(&e.path)) == want
            && e.pointer
                .as_deref()
                .is_none_or(|p| under(p, "/hooks") || under(p, "/marshell"))
    })
}

fn under(pointer: &str, root: &str) -> bool {
    pointer == root || pointer.strip_prefix(root).is_some_and(|rest| rest.starts_with('/'))
}

fn comparable(path: &Path) -> String {
    let text = unverbatim(path).to_string_lossy().into_owned();
    if cfg!(windows) {
        text.replace('/', "\\").trim_end_matches('\\').to_lowercase()
    } else {
        text.trim_end_matches('/').to_string()
    }
}

// ---- fixing the checked items and undoing the last fix ----

/// Why an id can't be fixed: it isn't in the last report, or the report or a fresh read says it can't be fixed.
const CANT_FIX: &str = "This item can't be fixed any more. Run the check again.";

fn cant_fix() -> Fail {
    (StatusCode::UNPROCESSABLE_ENTITY, CANT_FIX.to_string())
}

fn server_error(err: impl std::fmt::Display) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
}

/// Plans one fix for the items: every hooks item as "Set up" plans it, and the retention item as one edit of Claude
/// Code's settings. The facts are read again here, so an item that was fixed or changed since the report is a 422.
pub async fn fix(State(st): State<AppState>, Json(req): Json<DoctorFixRequest>) -> Response {
    match tokio::task::spawn_blocking(move || fix_of(&st, req.item_ids)).await {
        Ok(Ok(plan)) => Json(plan).into_response(),
        Ok(Err((status, message))) => (status, message).into_response(),
        Err(e) => server_error(e),
    }
}

fn fix_of(st: &AppState, requested: Vec<String>) -> Result<DoctorFixPlan, Fail> {
    let mut ids: Vec<String> = Vec::new();
    for id in requested {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        return Err(cant_fix());
    }
    let reported = st
        .doctor()
        .report
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or_else(cant_fix)?;
    // The files as they are now: the report may be minutes old.
    let gathered = facts_of(st);
    let now = checks::items(&gathered.facts, &gathered.stopped);
    let entries = managed_entries(st);

    let mut picked: Vec<(DoctorItem, &AgentFacts)> = Vec::new();
    for id in &ids {
        let fixable = |items: &[DoctorItem]| items.iter().find(|i| i.id == *id && i.fixable).cloned();
        let (Some(_), Some(item)) = (fixable(&reported.items), fixable(&now)) else {
            return Err(cant_fix());
        };
        let facts = gathered
            .facts
            .iter()
            .find(|f| Some(f.agent) == item.agent.as_deref() && f.distro == item.distro)
            .ok_or_else(cant_fix)?;
        picked.push((item, facts));
    }

    let mut edits: Vec<FileEdit> = Vec::new();
    for (item, facts) in &picked {
        match item.id.split('.').next() {
            Some("hooks") => {
                let adapter = adapter(facts.agent)?;
                let want = hooks_want_of(facts, &entries);
                let changes = match &facts.distro {
                    Some(distro) => st.distro_install(adapter, distro, &want)?,
                    None => hook_install_changes(st, adapter, &want)?,
                };
                edits.extend(changes.edits);
            }
            Some("retention") => edits.push(retention_edit(facts)?),
            _ => return Err(cant_fix()),
        }
    }
    if edits.is_empty() {
        return Err(cant_fix());
    }

    let count = picked.len();
    let cs = ChangeSet {
        title: if count == 1 {
            "Fix 1 issue".to_string()
        } else {
            format!("Fix {count} issues")
        },
        reason: picked
            .iter()
            .map(|(item, _)| item.title.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        edits,
    };
    // The retention setting is the person's, so applying it records no key in managed.json.
    let preview = st.0.config.preview_mixed(cs, &is_retention_edit);
    let restart = restart_list(st, &picked);
    *st.doctor().pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(PendingFix {
        changeset_id: preview.changeset_id.clone(),
        item_ids: ids.clone(),
        restart: restart.clone(),
    });
    Ok(DoctorFixPlan {
        preview,
        item_ids: ids,
        restart,
    })
}

const RETENTION_POINTER: &str = "/cleanupPeriodDays";

fn is_retention_edit(edit: &FileEdit) -> bool {
    matches!(&edit.op, EditOp::JsonSet { pointer, .. } if pointer == RETENTION_POINTER)
}

/// Claude Code's `cleanupPeriodDays` set to a year, against the text of the file as it is now.
fn retention_edit(facts: &AgentFacts) -> Result<FileEdit, Fail> {
    let text = std::fs::read_to_string(&facts.path).map_err(|_| cant_fix())?;
    Ok(FileEdit {
        path: facts.path.to_string_lossy().into_owned(),
        scope: ConfigScope::User,
        op: EditOp::JsonSet {
            pointer: RETENTION_POINTER.to_string(),
            value: "365".to_string(),
        },
        expected_hash: Some(hash_bytes(text.as_bytes())),
        applies: Applies::NewSessions,
    })
}

/// What "Set up" would ask for, from what is there now: approval stays when it is installed or managed.json says
/// Marshell added a `PermissionRequest` hook, and the chain stays when the status line is chained or managed.json says
/// Marshell set the status line. Antigravity's approval is its `PreToolUse` entry, which managed.json can't tell from
/// the status hooks, so only what the file shows keeps it. The skill is not part of a fix.
fn hooks_want_of(facts: &AgentFacts, entries: &[ManagedEntry]) -> HookPlanRequest {
    let approve =
        facts.approve_installed || managed_pointer(entries, &facts.path, |p| p.contains("/PermissionRequest"));
    // A distro keeps no chain, and Codex has no status line.
    let chains = facts.distro.is_none() && facts.agent != "codex";
    let statusline = chains
        && (facts.chained || {
            // Antigravity keeps its status line in `settings.json`, beside the `config` folder of its hooks.
            let settings = (facts.agent == "antigravity")
                .then(|| {
                    facts
                        .path
                        .parent()
                        .and_then(Path::parent)
                        .map(|gemini| gemini.join("antigravity-cli").join("settings.json"))
                })
                .flatten();
            [Some(facts.path.clone()), settings]
                .into_iter()
                .flatten()
                .any(|path| managed_pointer(entries, &path, |p| under(p, "/statusLine")))
        });
    HookPlanRequest {
        status: true,
        statusline,
        approve,
        skill: false,
        distro: facts.distro.clone(),
    }
}

/// managed.json has an entry for `path` whose pointer `matches`.
fn managed_pointer(entries: &[ManagedEntry], path: &Path, matches: impl Fn(&str) -> bool) -> bool {
    let want = comparable(path);
    entries
        .iter()
        .any(|e| comparable(Path::new(&e.path)) == want && e.pointer.as_deref().is_some_and(&matches))
}

/// The running sessions of the fixed items' agents, on Windows or in the item's distro, in session-number order.
fn restart_list(st: &AppState, picked: &[(DoctorItem, &AgentFacts)]) -> Vec<String> {
    let mut sessions: Vec<_> = st
        .registry()
        .snapshot()
        .sessions
        .into_iter()
        .filter(|s| s.state == SessionState::Running)
        .filter_map(|s| {
            let agent = s.agent.clone()?;
            let mut wanted = picked.iter().filter(|(_, f)| f.agent == agent).peekable();
            wanted.peek()?;
            // Asked of the tab only when an item of its agent could match.
            let distro = st.wsl_distro_of_tab(&s.tab_id);
            let matches = wanted.any(|(_, f)| match (&f.distro, &distro) {
                (None, None) => true,
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                _ => false,
            });
            matches.then_some((s.number, s.tab_id))
        })
        .collect();
    sessions.sort();
    sessions.into_iter().map(|(_, tab)| tab).collect()
}

/// What `<home>/doctor/last-fix.json` keeps of a fix: the restart list and whether the files match are not kept.
#[derive(Serialize, Deserialize)]
struct LastFixFile {
    snapshot_id: String,
    fixed_at_ms: u64,
    item_ids: Vec<String>,
    count: u32,
}

impl Versioned for LastFixFile {
    const VERSION: u32 = 1;
}

/// Applies the fix whose preview was shown, with the hashes that were shown. A file changed since is a 409 that
/// writes nothing. On success the fix is recorded for "Undo last fix", and the checks run again.
pub async fn fix_apply(State(st): State<AppState>, Json(req): Json<ApplyRequest>) -> Response {
    let pending = st.doctor().pending.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let Some(pending) = pending.filter(|p| p.changeset_id == req.changeset_id) else {
        return (StatusCode::NOT_FOUND, "no such preview").into_response();
    };
    let work = {
        let st = st.clone();
        let pending = pending.clone();
        move || -> Result<DoctorLastFix, ApplyError> {
            let at = now_ms();
            let result = st.0.config.apply(&req, at)?;
            let record = DoctorLastFix {
                snapshot_id: result.snapshot_id,
                fixed_at_ms: at,
                count: pending.item_ids.len() as u32,
                item_ids: pending.item_ids,
                restart: pending.restart,
                files_match: true,
            };
            // The files are written; a record that can't be kept only costs the undo after a restart.
            let kept = LastFixFile {
                snapshot_id: record.snapshot_id.clone(),
                fixed_at_ms: record.fixed_at_ms,
                item_ids: record.item_ids.clone(),
                count: record.count,
            };
            if let Err(e) = store::write(&paths::doctor_last_fix(&st.0.home), &kept) {
                tracing::warn!("the doctor could not keep the last fix: {e:#}");
            }
            Ok(record)
        }
    };
    let outcome = tokio::task::spawn_blocking(work).await;
    let doctor = st.doctor();
    let forget_pending = || {
        let mut slot = doctor.pending.lock().unwrap_or_else(|e| e.into_inner());
        if slot.as_ref().is_some_and(|p| p.changeset_id == pending.changeset_id) {
            *slot = None;
        }
    };
    match outcome {
        Ok(Ok(record)) => {
            *doctor.last_fix.lock().unwrap_or_else(|e| e.into_inner()) = Some(record.clone());
            forget_pending();
            request_run(&st);
            Json(record).into_response()
        }
        Ok(Err(ApplyError::UnknownChangeSet)) => {
            forget_pending();
            (StatusCode::NOT_FOUND, "no such preview").into_response()
        }
        Ok(Err(ApplyError::Conflict { reason, paths })) => {
            (StatusCode::CONFLICT, Json(ApplyConflict { reason, paths })).into_response()
        }
        Ok(Err(ApplyError::Failed(e))) => server_error(format!("{e:#}")),
        Err(e) => server_error(e),
    }
}

/// The last fix in this run of the app, else the one on disk (which has no restart list).
fn current_record(st: &AppState) -> Option<DoctorLastFix> {
    if let Some(record) = st.doctor().last_fix.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Some(record);
    }
    match store::read::<LastFixFile>(&paths::doctor_last_fix(&st.0.home)) {
        Ok(Some(kept)) => Some(DoctorLastFix {
            snapshot_id: kept.snapshot_id,
            fixed_at_ms: kept.fixed_at_ms,
            item_ids: kept.item_ids,
            count: kept.count,
            restart: Vec::new(),
            files_match: false,
        }),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("the doctor could not read the last fix: {e:#}");
            None
        }
    }
}

/// Drops the record, in memory and on disk.
fn forget_record(st: &AppState) {
    *st.doctor().last_fix.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let file = paths::doctor_last_fix(&st.0.home);
    for path in [store::bak_path(&file), file] {
        if let Err(e) = std::fs::remove_file(&path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("the doctor could not delete {}: {e}", path.display());
        }
    }
}

/// The file isn't what the fix left: another hash, or present where the fix removed it.
fn differs_from_fix(file: &SnapshotFileInfo) -> bool {
    match hash_file(Path::new(&file.path)) {
        Ok(now) => now != file.after_hash,
        Err(_) => true,
    }
}

/// `GET /v1/doctor/undo`: the last fix, or 204. `files_match` is read without starting a distro: a file in a managed
/// distro that isn't running is left out of it.
pub async fn last_fix(State(st): State<AppState>) -> Response {
    let found = tokio::task::spawn_blocking(move || {
        let mut record = current_record(&st)?;
        record.files_match = match st.0.config.snapshots().get(&record.snapshot_id) {
            Ok(Some(info)) => {
                let mut running: Option<Vec<String>> = None;
                info.files.iter().all(|file| {
                    let in_distro = st.distros_of_paths([Path::new(&file.path)]).into_iter().next();
                    if let Some(distro) = in_distro {
                        let up = running.get_or_insert_with(|| (st.0.running_distros)());
                        if !up.iter().any(|d| d.eq_ignore_ascii_case(&distro)) {
                            return true;
                        }
                    }
                    !differs_from_fix(file)
                })
            }
            _ => false,
        };
        Some(record)
    })
    .await;
    match found {
        Ok(Some(record)) => Json(record).into_response(),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => server_error(e),
    }
}

/// `POST /v1/doctor/undo`: puts the files back as the last fix found them, if each is still as the fix left it.
/// Starting a distro is allowed here: the person clicked.
pub async fn undo(State(st): State<AppState>) -> Response {
    let work = {
        let st = st.clone();
        move || undo_of(&st)
    };
    match tokio::task::spawn_blocking(work).await {
        Ok(Ok(Some(result))) => {
            request_run(&st);
            Json(result).into_response()
        }
        Ok(Ok(None)) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(refusal)) => refusal.into_response(),
        Err(e) => server_error(e),
    }
}

fn undo_of(st: &AppState) -> Result<Option<ApplyResult>, Refusal> {
    let Some(record) = current_record(st) else {
        return Ok(None);
    };
    let info = match st.0.config.snapshots().get(&record.snapshot_id) {
        Ok(Some(info)) => info,
        // The backup is gone, so the record can never be undone.
        Ok(None) => {
            forget_record(st);
            return Ok(None);
        }
        Err(e) => return Err(Refusal::Plain(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))),
    };
    // After a restart the config guard knows no distro's folders until its home is looked up.
    st.record_distros_of(info.files.iter().map(|f| Path::new(&f.path)));
    let changed: Vec<String> = info
        .files
        .iter()
        .filter(|f| differs_from_fix(f))
        .map(|f| f.path.clone())
        .collect();
    if !changed.is_empty() {
        return Err(changed_since(changed));
    }
    let cs = ChangeSet {
        title: format!("Restore: {}", info.title),
        reason: "Puts the files back as they were before this change.".to_string(),
        edits: info
            .files
            .iter()
            .map(|file| FileEdit {
                path: file.path.clone(),
                scope: file.scope,
                op: EditOp::Restore {
                    snapshot_id: info.id.clone(),
                },
                expected_hash: file.after_hash.clone(),
                applies: file.applies,
            })
            .collect(),
    };
    let preview = changes_of(st.0.config.preview(cs))?;
    let apply = ApplyRequest {
        changeset_id: preview.changeset_id.clone(),
        expected_hashes: preview
            .files
            .iter()
            .map(|f| ExpectedHash {
                path: f.path.clone(),
                hash: f.before_hash.clone(),
            })
            .collect(),
    };
    match st.0.config.apply(&apply, now_ms()) {
        Ok(result) => {
            forget_record(st);
            Ok(Some(result))
        }
        Err(ApplyError::Conflict { paths, .. }) => Err(changed_since(paths)),
        Err(ApplyError::UnknownChangeSet) => Err(changed_since(vec![])),
        Err(ApplyError::Failed(e)) => Err(Refusal::Plain(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::doctor::DoctorGroup;

    fn entry(path: &str, pointer: Option<&str>) -> ManagedEntry {
        ManagedEntry {
            path: path.into(),
            pointer: pointer.map(str::to_string),
            changeset_id: "c".into(),
            applied_ms: 1,
        }
    }

    #[test]
    fn set_up_is_true_for_a_hooks_entry_of_that_file() {
        let file = Path::new("C:/Users/me/.claude/settings.json");
        assert!(set_up_of(
            &[entry("C:/Users/me/.claude/settings.json", Some("/hooks/Stop/-"))],
            file
        ));
        assert!(set_up_of(
            &[entry("C:/Users/me/.claude/settings.json", Some("/hooks"))],
            file
        ));
        // A file Marshell created.
        assert!(set_up_of(&[entry("C:/Users/me/.claude/settings.json", None)], file));
        // Antigravity's one named hook.
        let agy = Path::new("C:/Users/me/.gemini/config/hooks.json");
        assert!(set_up_of(
            &[entry("C:/Users/me/.gemini/config/hooks.json", Some("/marshell"))],
            agy
        ));
    }

    #[test]
    fn set_up_is_false_for_a_status_line_alone_and_for_another_file() {
        let file = Path::new("C:/Users/me/.claude/settings.json");
        assert!(!set_up_of(
            &[entry("C:/Users/me/.claude/settings.json", Some("/statusLine"))],
            file
        ));
        assert!(!set_up_of(
            &[entry("C:/Users/me/.claude/settings.json", Some("/hooksX/a"))],
            file
        ));
        assert!(!set_up_of(
            &[entry("C:/Users/other/.claude/settings.json", Some("/hooks/Stop"))],
            file
        ));
        assert!(!set_up_of(&[], file));
    }

    #[cfg(windows)]
    #[test]
    fn set_up_ignores_case_slashes_and_the_verbatim_prefix_on_windows() {
        let file = Path::new(r"C:\Users\Me\.claude\settings.json");
        assert!(set_up_of(
            &[entry("c:/users/me/.CLAUDE/settings.json", Some("/hooks/Stop"))],
            file
        ));
        assert!(set_up_of(
            &[entry(r"C:\Users\Me\.claude\settings.json", Some("/hooks/Stop"))],
            file
        ));
        let share = Path::new(r"\\wsl.localhost\Ubuntu\home\me\.claude\settings.json");
        assert!(set_up_of(
            &[entry(
                r"\\?\UNC\wsl.localhost\Ubuntu\home\me\.claude\settings.json",
                Some("/hooks")
            )],
            share
        ));
    }

    fn state(dir: &Path) -> AppState {
        AppState::new("t".into(), 1, Vec::new(), dir.join("home"), Vec::new())
    }

    fn setup(agent: &str) -> AgentSetup {
        AgentSetup {
            agent: agent.into(),
            admin_unavailable: None,
            found: true,
            version: None,
            exe_path: None,
            settings_path: "Z:/none/settings.json".into(),
            skips_all: None,
            hooks_path: None,
            settings_state: SettingsState::Missing,
            parse_error: None,
            hooks: marshell_protocol::agents::HookInstall::None,
            approve_installed: false,
            has_own_statusline: false,
            chained: false,
            statusline_preset: false,
            needs_trust: false,
            trust: None,
            bridge_path: "Z:/none/bridge.exe".into(),
            snapshots_dir: String::new(),
            skill: marshell_protocol::agents::SkillInstall::None,
            skill_path: String::new(),
            skill_problem: None,
        }
    }

    #[test]
    fn one_cli_failing_to_read_leaves_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let st = state(dir.path());
        let facts = facts_with(&st, |_, agent| {
            if agent == "codex" {
                Err((StatusCode::INTERNAL_SERVER_ERROR, "the home folder is not known".into()))
            } else {
                Ok(setup(agent))
            }
        });
        assert_eq!(facts.len(), 4);
        let items = checks::items(&facts, &[]);
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        let failed = items.iter().find(|i| i.id == "error.codex").unwrap();
        assert_eq!(failed.group, DoctorGroup::Info);
        assert_eq!(failed.detail, "the home folder is not known");
        // The others were read and found, with no file: each gets its setup line.
        for agent in ["claude", "antigravity", "qwen"] {
            assert!(ids.contains(&format!("setup.{agent}").as_str()), "{ids:?}");
        }
        assert!(!ids.contains(&"setup.codex"), "{ids:?}");
    }

    #[tokio::test]
    async fn a_request_during_a_run_runs_again_after_it() {
        use std::sync::Mutex;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;

        let dir = tempfile::tempdir().unwrap();
        let st = state(dir.path());
        let calls = Arc::new(AtomicUsize::new(0));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel::<()>();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let started_tx = Mutex::new(Some(started_tx));
        let release_rx = Mutex::new(release_rx);
        let counter = calls.clone();
        // The first read blocks until the test has made its further requests; each read names its own number.
        let read: ReadFacts = Arc::new(move |st| {
            let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
            if n == 1 {
                if let Some(tx) = started_tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
                release_rx.lock().unwrap().recv().unwrap();
            }
            Gathered {
                facts: facts_with(st, move |_, _| {
                    Err((StatusCode::INTERNAL_SERVER_ERROR, format!("read {n}")))
                }),
                stopped: Vec::new(),
            }
        });

        let mut summary = st.doctor().summary.subscribe();
        request_run_with(&st, read.clone());
        started_rx.await.unwrap();
        // Two requests during one run still cause one more run.
        request_run_with(&st, read.clone());
        request_run_with(&st, read.clone());
        release_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(10), summary.wait_for(|s| !s.running))
            .await
            .expect("the runs end")
            .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let report = st.doctor().report.lock().unwrap().clone().unwrap();
        let failed = report.items.iter().find(|i| i.id == "error.claude").unwrap();
        assert_eq!(failed.detail, "read 2");
        assert!(!st.doctor().summary.borrow().running);
    }

    #[test]
    fn antigravity_reads_its_hooks_file_and_an_unparsable_file_keeps_its_line() {
        let dir = tempfile::tempdir().unwrap();
        let st = state(dir.path());
        let mut s = setup("antigravity");
        s.hooks_path = Some("Z:/none/hooks.json".into());
        s.settings_state = SettingsState::Unparsable;
        s.parse_error = Some(marshell_protocol::agents::ParseProblem {
            line: 4,
            message: "bad".into(),
        });
        let f = facts_from(&st, "antigravity", s, &[]);
        assert_eq!(f.path, PathBuf::from("Z:/none/hooks.json"));
        assert_eq!(
            f.file,
            FileState::Unparsable {
                line: 4,
                message: "bad".into()
            }
        );
        assert_eq!(f.retention_unset, None);
    }

    #[test]
    fn retention_is_read_for_claude_only() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let st = state(dir.path());
        let mut s = setup("claude");
        s.settings_path = file.to_string_lossy().into_owned();
        s.settings_state = SettingsState::Ok;

        std::fs::write(&file, r#"{"theme": "dark"}"#).unwrap();
        assert_eq!(facts_from(&st, "claude", s.clone(), &[]).retention_unset, Some(true));
        std::fs::write(&file, r#"{"cleanupPeriodDays": 30}"#).unwrap();
        assert_eq!(facts_from(&st, "claude", s.clone(), &[]).retention_unset, Some(false));
        // A nested key is not the top-level setting.
        std::fs::write(&file, r#"{"general": {"cleanupPeriodDays": 30}}"#).unwrap();
        assert_eq!(facts_from(&st, "claude", s.clone(), &[]).retention_unset, Some(true));
        // Not found, a missing file, and the other CLIs.
        let mut gone = s.clone();
        gone.found = false;
        assert_eq!(facts_from(&st, "claude", gone, &[]).retention_unset, None);
        let mut missing = s.clone();
        missing.settings_state = SettingsState::Missing;
        assert_eq!(facts_from(&st, "claude", missing, &[]).retention_unset, None);
        for agent in ["codex", "antigravity", "qwen"] {
            let mut other = s.clone();
            other.agent = agent.into();
            assert_eq!(facts_from(&st, agent_static(agent), other, &[]).retention_unset, None);
        }
    }

    fn agent_static(agent: &str) -> &'static str {
        AGENTS.into_iter().find(|a| *a == agent).unwrap()
    }

    #[test]
    fn a_found_cli_with_an_unreadable_version_gets_the_version_note() {
        let dir = tempfile::tempdir().unwrap();
        let st = state(dir.path());
        let f = facts_from(&st, "codex", setup("codex"), &[]);
        assert_eq!(f.version_note.as_deref(), Some("Codex's version could not be read"));
        let mut gone = setup("codex");
        gone.found = false;
        assert_eq!(facts_from(&st, "codex", gone, &[]).version_note, None);
    }
}
