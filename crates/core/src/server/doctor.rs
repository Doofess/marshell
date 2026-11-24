//! The Doctor's runs: `POST /v1/doctor/run` starts the checks (or joins the run that is going) and answers 202 with
//! the summary; `GET /v1/doctor/report` answers the last report, or 204 before the first run ends. The core also
//! runs once at startup (see `AppState::with_doctor_on_start`). A run reads the files of every CLI off the async
//! threads, builds the report with `doctor::checks` and publishes the summary on the events socket.

use super::AppState;
use super::agents::{
    Adapter, Refusal, adapter, agy_setup_of, changed_since, changes_of, codex_setup_of, copilot_hooks_file_is_kept,
    copilot_install_over_changes, copilot_kept_record, copilot_setup_of, hook_install_changes, qwen_setup_of,
    settings_file, setup_of,
};
use super::doctor_ext;
use super::doctor_usage;
use crate::agents::common::Gate;
use crate::agents::copilot::hooks::{FileProblem, file_problem};
use crate::agents::copilot::kept::{self, Kept};
use crate::config::guard::unverbatim;
use crate::config::hash::{hash_bytes, hash_file};
use crate::config::jsonc::{parse_problem, read_value};
use crate::config::managed::{self, ManagedEntry};
use crate::config::service::ApplyError;
use crate::doctor::cli::{CLI_DOCTOR_TIMEOUT_SECS, Ran, claude_items, codex_items, no_doctor_item};
use crate::doctor::ext::{self, Inventory, Switch};
use crate::doctor::{AgentFacts, FileState, PendingFix, checks};
use crate::paths;
use crate::profiles::detect::{agent_command, run_wsl_capture};
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
    DoctorAction, DoctorFixPlan, DoctorFixRequest, DoctorItem, DoctorKeepRequest, DoctorLastFix, DoctorReport,
    DoctorSummary,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

type Fail = (StatusCode, String);

/// The CLIs the Doctor checks, in the report's order.
const AGENTS: [&str; 5] = ["claude", "codex", "antigravity", "qwen", "copilot"];

pub async fn run(State(st): State<AppState>) -> (StatusCode, Json<DoctorSummary>) {
    request_run(&st);
    (StatusCode::ACCEPTED, Json(st.doctor().summary.borrow().clone()))
}

/// `POST /v1/doctor/cli`: runs the CLI doctors, or joins the run that is going, and answers 202 with the summary.
pub async fn cli_run(State(st): State<AppState>) -> (StatusCode, Json<DoctorSummary>) {
    request_cli_run(&st);
    (StatusCode::ACCEPTED, Json(st.doctor().summary.borrow().clone()))
}

/// `POST /v1/doctor/measure`: starts "Measure MCP servers", or joins the run that is going, and answers 202 with the
/// summary.
pub async fn measure_run(State(st): State<AppState>) -> (StatusCode, Json<DoctorSummary>) {
    super::doctor_measure::request_measure(&st);
    (StatusCode::ACCEPTED, Json(st.doctor().summary.borrow().clone()))
}

/// One CLI doctor to run: on Windows `exe` is the resolved program, in a distro it is the Linux program name.
pub(crate) struct DoctorCommand {
    pub distro: Option<String>,
    pub exe: PathBuf,
    pub args: Vec<&'static str>,
}

/// Runs one CLI doctor and returns its exit code and output; `None` when it can't start or isn't done in time.
pub(crate) type DoctorRunner = fn(&DoctorCommand) -> Ran;

/// The default `DoctorRunner`: a process on Windows, `wsl.exe --exec` in a distro. Both give up after the timeout.
pub(crate) fn run_doctor_command(cmd: &DoctorCommand) -> Ran {
    let timeout = std::time::Duration::from_secs(CLI_DOCTOR_TIMEOUT_SECS);
    match &cmd.distro {
        None => crate::agents::run_capture(&cmd.exe, &cmd.args, timeout),
        Some(distro) => {
            let argv = super::agents::wsl_exec_argv(distro, &cmd.exe.to_string_lossy(), &cmd.args);
            let args: Vec<&str> = argv.iter().map(String::as_str).collect();
            run_wsl_capture(&args, timeout).map(|(code, out)| (code, String::from_utf8_lossy(&out).into_owned()))
        }
    }
}

/// Starts the CLI doctors unless they are going, in which case the request joins that run: nothing runs twice. The
/// summary reads `cli_running` as soon as this returns. When they end, the findings are stored, a run of the checks
/// is asked for, and only then does `cli_running` clear, so the page that reloads on it finds the new items. Needs a
/// Tokio runtime.
pub(crate) fn request_cli_run(st: &AppState) {
    let doctor = st.doctor();
    {
        let mut cli = doctor.cli.lock().unwrap_or_else(|e| e.into_inner());
        if cli.running {
            return;
        }
        cli.running = true;
        doctor.summary.send_modify(|s| s.cli_running = true);
    }
    let st = st.clone();
    tokio::spawn(async move {
        let reader = st.clone();
        let found = tokio::task::spawn_blocking(move || cli_doctor_items(&reader)).await;
        let doctor = st.doctor();
        match found {
            Ok(items) => {
                let mut cli = doctor.cli.lock().unwrap_or_else(|e| e.into_inner());
                cli.items = items;
                cli.checked_at_ms = Some(now_ms());
            }
            // The old findings stay, and the flag still clears below.
            Err(e) => tracing::warn!("the CLI doctors' run failed: {e}"),
        }
        // `running` goes true here, before `cli_running` goes false.
        request_run(&st);
        let mut cli = doctor.cli.lock().unwrap_or_else(|e| e.into_inner());
        cli.running = false;
        doctor.summary.send_modify(|s| s.cli_running = false);
    });
}

/// The items of every CLI doctor: on Windows, then in each running managed distro. Runs processes, so callers go
/// through `spawn_blocking`.
fn cli_doctor_items(st: &AppState) -> Vec<DoctorItem> {
    let mut out = Vec::new();
    for agent in AGENTS {
        if let Some(exe) = st.agent_exe_of(agent) {
            out.extend(doctor_items_of(st, agent, None, exe));
        }
    }
    let entries = managed_entries(st);
    for distro in running_managed_distros(st, &entries) {
        for agent in AGENTS {
            if st.agent_version_in(&distro, agent).is_some() {
                // The program's name in the distro, as the version check runs it.
                let exe = PathBuf::from(agent_command(agent).unwrap_or(agent));
                out.extend(doctor_items_of(st, agent, Some(&distro), exe));
            }
        }
    }
    out
}

fn doctor_items_of(st: &AppState, agent: &str, distro: Option<&str>, exe: PathBuf) -> Vec<DoctorItem> {
    let run = |args: Vec<&'static str>| {
        let cmd = DoctorCommand {
            distro: distro.map(str::to_string),
            exe,
            args,
        };
        (st.0.doctor_runner)(&cmd)
    };
    match agent {
        "claude" => claude_items(&run(vec!["doctor"]), distro),
        "codex" => codex_items(&run(vec!["doctor", "--json"]), distro),
        _ => vec![no_doctor_item(agent, distro)],
    }
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
            let mut items = checks::items(&facts.facts, &facts.stopped, banners_on(st));
            // The last measurement is cloned under the lock, which is released at once.
            let measured = doctor.measure.lock().unwrap_or_else(|e| e.into_inner()).results.clone();
            let measured_at_ms = measured.as_ref().map(|m| m.at_ms);
            let signals = ext::Signals {
                usage: Some(&facts.usage),
                projects: &facts.projects,
                now_ms: now_ms(),
                measured: measured.as_ref(),
            };
            items.extend(ext::items(&facts.ext, &signals));
            // The CLI doctors' last findings ride along in every report. The lock is released at once.
            let (cli_items, cli_checked_at_ms) = {
                let cli = doctor.cli.lock().unwrap_or_else(|e| e.into_inner());
                (cli.items.clone(), cli.checked_at_ms)
            };
            items.extend(cli_items);
            let issues = checks::issues(&items);
            let now = now_ms();
            *doctor.report.lock().unwrap_or_else(|e| e.into_inner()) = Some(DoctorReport {
                items,
                checked_at_ms: now,
                duration_ms: started.elapsed().as_millis() as u64,
                cli_checked_at_ms,
                measured_at_ms,
            });
            Some((issues, now))
        }
        Err(e) => {
            tracing::warn!("the doctor's run failed: {e}");
            None
        }
    }
}

/// Marshell's banners are on for at least one of needs-you, done and error.
fn banners_on(st: &AppState) -> bool {
    let n = st.notify_settings();
    n.needs_you.banner || n.done.banner || n.error.banner
}

pub(super) fn now_ms() -> u64 {
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
        "qwen" => qwen_setup_of(st),
        "copilot" => copilot_setup_of(st),
        _ => Err((StatusCode::NOT_FOUND, "no such agent".to_string())),
    });
    let (distro, stopped, distro_homes) = distro_facts(st, &entries);
    let facts: Vec<AgentFacts> = facts.into_iter().chain(distro).collect();
    // The MCP servers and skills are read after the facts, so an agent whose facts failed is left out.
    let ext = doctor_ext::inventories(st, &facts, &distro_homes);
    let roots = doctor_ext::windows_roots(st);
    // What each CLI's history shows was used: the new lines are read now and kept in usage.json.
    let usage = doctor_usage::scan_all(st, &ext, roots.as_ref(), now_ms());
    let projects = if roots.is_some() {
        crate::projects::list(&st.0.home)
            .into_iter()
            .map(|p| PathBuf::from(p.path))
            .collect()
    } else {
        Vec::new()
    };
    let mut ext_homes: Vec<PathBuf> = roots.map(|r| r.home).into_iter().collect();
    ext_homes.extend(distro_homes.into_iter().map(|(_, home)| home));
    Gathered {
        facts,
        stopped,
        ext,
        ext_homes,
        usage,
        projects,
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
/// set up that aren't running, and their MCP servers and skills.
pub(crate) struct Gathered {
    pub facts: Vec<AgentFacts>,
    pub stopped: Vec<String>,
    /// Each CLI's MCP servers and skills, on Windows and in each running managed distro.
    pub ext: Vec<Inventory>,
    /// The user homes those were read under: Windows' and each distro's. A fix's edit is `User` scope under one.
    pub ext_homes: Vec<PathBuf>,
    /// What the histories showed was used, as of this read (Windows CLIs only).
    pub usage: crate::doctor::usage::UsageStore,
    /// Marshell's projects, which tell where a user-level item was used.
    pub projects: Vec<PathBuf>,
}

/// The distros Marshell set up: the detected ones that hold a path of `managed.json`. This reads no distro's file.
fn managed_distros(st: &AppState, entries: &[ManagedEntry]) -> Vec<String> {
    if entries.is_empty() {
        return Vec::new();
    }
    st.distros_of_paths(entries.iter().map(|e| Path::new(&e.path)))
}

/// The managed distros that are running, and those that aren't. `wsl.exe` is asked what runs only when there is a
/// managed distro.
fn partition_managed_distros(st: &AppState, entries: &[ManagedEntry]) -> (Vec<String>, Vec<String>) {
    let managed = managed_distros(st, entries);
    if managed.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let running = (st.0.running_distros)();
    let is_running = |d: &String| running.iter().any(|r| r.to_lowercase() == d.to_lowercase());
    managed.into_iter().partition(is_running)
}

/// The managed distros that are running: the only ones a CLI doctor may run in.
fn running_managed_distros(st: &AppState, entries: &[ManagedEntry]) -> Vec<String> {
    partition_managed_distros(st, entries).0
}

/// The facts of every running managed distro, and the managed distros that aren't running. `wsl.exe` is asked what
/// runs only when there is a managed distro, and a distro that isn't running is never asked for its target nor read,
/// since either would start it. Runs `wsl.exe` and reads shares, so callers go through `spawn_blocking`.
fn distro_facts(st: &AppState, entries: &[ManagedEntry]) -> (Vec<AgentFacts>, Vec<String>, Vec<(String, PathBuf)>) {
    let (up, stopped) = partition_managed_distros(st, entries);
    let kept = copilot_kept_record(&st.0.home);
    let mut facts = Vec::new();
    let mut homes = Vec::new();
    for distro in &up {
        match st.distro_target(distro) {
            Ok(target) => {
                homes.push((distro.clone(), target.home.clone()));
                for agent in AGENTS {
                    facts.push(distro_agent_facts(agent, distro, &target, entries, &kept));
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
    (facts, stopped, homes)
}

/// A CLI's facts when its reader failed: every field a default, and the reason.
fn failed_facts(agent: &'static str, distro: Option<String>, message: String) -> AgentFacts {
    AgentFacts {
        agent,
        distro,
        found: false,
        path: PathBuf::new(),
        file: FileState::Ok,
        problem_path: None,
        hooks: marshell_protocol::agents::HookInstall::None,
        approve_installed: false,
        has_own_statusline: false,
        chained: false,
        trust: None,
        set_up: false,
        kept: false,
        bridge: PathBuf::new(),
        bridge_exists: false,
        version_note: None,
        retention_unset: None,
        managed_settings: None,
        notify: Default::default(),
        error: Some(message),
    }
}

/// The `command` of every `hooks.Notification[*].hooks[*]` entry that isn't Marshell's, in file order. An entry without
/// a string `command` is skipped.
fn notification_hooks(value: &serde_json::Value) -> Vec<String> {
    let groups = value
        .pointer("/hooks/Notification")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten();
    groups
        .filter_map(|g| g.get("hooks").and_then(serde_json::Value::as_array))
        .flatten()
        .filter(|h| !crate::agents::common::is_ours(h))
        .filter_map(|h| h.get("command").and_then(serde_json::Value::as_str))
        .map(str::to_string)
        .collect()
}

/// Codex's top-level `notify` array of strings, joined with spaces. A missing file, one that doesn't parse, and a
/// `notify` that isn't an array of strings give `None`.
fn codex_notify_command(config: &Path) -> Option<String> {
    let text = std::fs::read_to_string(config).ok()?;
    let doc = text.parse::<toml_edit::DocumentMut>().ok()?;
    let items = doc.get("notify")?.as_array()?;
    let parts: Option<Vec<&str>> = items.iter().map(toml_edit::Value::as_str).collect();
    let parts = parts?;
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// What a CLI notifies by itself outside the terminal. `hooks` is the value its hooks file gave. Antigravity has
/// nothing here: its `notifications` is a bell and an OSC 9 in the terminal (see `checks::NotifyFacts`).
fn notify_facts(agent: &str, hooks_path: &Path, hooks: Option<&serde_json::Value>) -> checks::NotifyFacts {
    let mut n = checks::NotifyFacts {
        file: hooks_path.to_path_buf(),
        ..Default::default()
    };
    if agent == "antigravity" {
        return n;
    }
    if agent == "copilot" {
        // `hooks_path` is `<copilot home>/hooks/marshell.json`; Copilot's own toasts are read on Windows only. A tab
        // starts Copilot in the app's environment, so the variable that turns the toasts off is read from it.
        use crate::agents::copilot::notify::{TOASTS_OFF_ENV, toasts_off_by_env};
        let off = toasts_off_by_env(std::env::var_os(TOASTS_OFF_ENV).as_deref());
        return copilot_notify(hooks_path, !off);
    }
    if agent == "codex" {
        // Codex has no Notification hook event, so a group under it never runs: only its notify command counts.
        n.file = hooks_path.with_file_name("config.toml");
        n.command = codex_notify_command(&n.file);
        return n;
    }
    n.hook_commands = hooks.map(notification_hooks).unwrap_or_default();
    n
}

/// What Copilot CLI notifies by itself, from its `settings.json` and every `*.json` in the `hooks` folder beside Marshell's
/// file (`hooks_path`, which is read too: only the entries that aren't Marshell's count). A file that doesn't parse, or
/// is over the config size limit, is skipped. `toasts` is false in a distro, whatever `settings.json` says.
fn copilot_notify(hooks_path: &Path, toasts: bool) -> checks::NotifyFacts {
    use crate::agents::copilot::{hooks as copilot_hooks, notify};
    let folder = hooks_path.parent().unwrap_or(hooks_path);
    let settings_path = folder
        .parent()
        .map(copilot_hooks::settings_path)
        .unwrap_or_else(|| folder.join("settings.json"));
    let mut files: Vec<(PathBuf, serde_json::Value)> = Vec::new();
    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let path = entry.path();
        let is_json = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("json"));
        if !is_json || !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        // Any file can be dropped in the folder: one over the config size limit is not a hooks file Marshell reads.
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > crate::config::preview::MAX_CONFIG_BYTES) {
            continue;
        }
        if let Some(value) = std::fs::read_to_string(&path).ok().and_then(|t| read_value(&t).ok()) {
            files.push((path, value));
        }
    }
    let settings = std::fs::read_to_string(&settings_path)
        .ok()
        .and_then(|t| read_value(&t).ok());
    checks::NotifyFacts {
        file: hooks_path.to_path_buf(),
        hook_commands: notify::own_hooks(&files, settings.as_ref()),
        toasts: toasts && settings.as_ref().is_some_and(notify::toasts_on),
        settings_read_only: std::fs::metadata(&settings_path).is_ok_and(|m| m.permissions().readonly()),
        settings: settings_path,
        ..Default::default()
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
    kept: &Kept,
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
    if agent == "copilot" {
        return distro_copilot_facts(distro, target, entries, kept, &bridge);
    }
    // Antigravity's hooks are in `hooks.json`, with the status line in `settings.json`.
    let files: Vec<PathBuf> = match agent {
        "claude" => vec![home.join(".claude").join("settings.json")],
        "codex" => vec![home.join(".codex").join("hooks.json")],
        "antigravity" => vec![agy::hooks_path(home), agy::settings_path(home)],
        "qwen" => vec![home.join(".qwen").join("settings.json")],
        _ => return failed_facts(agent, Some(distro.to_string()), "no such agent".to_string()),
    };
    let path = files[0].clone();
    let mut facts = AgentFacts {
        agent,
        distro: Some(distro.to_string()),
        found: true,
        set_up: set_up_of(entries, &path),
        kept: false,
        path,
        file: FileState::Ok,
        problem_path: None,
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
        notify: Default::default(),
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
        "qwen" => {
            let f = qwen::hooks::inspect_for(first.unwrap_or(&none), None, &bridge);
            (f.install, f.approve_installed)
        }
        _ => return failed_facts(agent, Some(distro.to_string()), "no such agent".to_string()),
    };
    facts.hooks = install;
    facts.approve_installed = approve_installed;
    facts.notify = notify_facts(agent, &facts.path, first);
    facts
}

/// What Copilot CLI's hooks file is, read as text: its text when it is there, `Some(state)` when it doesn't parse, isn't
/// Marshell's, or exists and can't be read as UTF-8 text, and whether the person kept it. A file that isn't Marshell's
/// but still holds what the record says the person kept is kept: no problem, and Marshell changes only its own entries
/// in it. Any other file that isn't Marshell's is replaced by the Doctor's fix unless `cannot_fix` says why not: the file
/// or `settings.json` is read-only, or the file can't be shown in a preview because it isn't text. It can be kept when
/// it is text that parses to an object.
fn copilot_hooks_problem(
    hooks_path: &Path,
    settings_path: &Path,
    kept: &Kept,
) -> (Option<String>, Option<FileState>, bool) {
    use crate::agents::copilot::hooks::{FileProblem, file_problem};
    let no_preview = Some(FileState::NotOurs {
        cannot_fix: Some(NO_PREVIEW),
        keepable: false,
    });
    let text = match hooks_file_bytes(hooks_path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => return (None, no_preview, false),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (None, None, false),
        Err(_) => return (None, no_preview, false),
    };
    let read_only = |p: &Path| std::fs::metadata(p).is_ok_and(|m| m.permissions().readonly());
    let (problem, is_kept) = match file_problem(Some(&text)) {
        None => (None, false),
        Some(FileProblem::DoesNotParse { line, message }) => (Some(FileState::Unparsable { line, message }), false),
        Some(FileProblem::NotMarshells) if copilot_hooks_file_is_kept(kept, hooks_path, Some(&text)) => (None, true),
        Some(FileProblem::NotMarshells) => (
            Some(FileState::NotOurs {
                cannot_fix: (read_only(hooks_path) || read_only(settings_path)).then_some(READ_ONLY),
                keepable: read_value(&text).is_ok_and(|file| file.is_object()),
            }),
            false,
        ),
    };
    (Some(text), problem, is_kept)
}

/// A hooks file's bytes. Never more than the config size limit is read: a larger file is an error, as is one that
/// can't be read.
fn hooks_file_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
    use crate::config::preview::MAX_CONFIG_BYTES;
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the file is over the config size limit",
        ));
    }
    Ok(bytes)
}

use checks::READ_ONLY;
const NO_PREVIEW: &str = "Marshell can't show this file in a preview. Move it away, then run the check again.";

/// Copilot CLI's facts in a distro: its two files as text, read against the distro's bridge. A distro keeps no chain.
fn distro_copilot_facts(
    distro: &str,
    target: &super::DistroTarget,
    entries: &[ManagedEntry],
    kept: &Kept,
    bridge: &crate::agents::common::Bridge,
) -> AgentFacts {
    use crate::agents::copilot::hooks as copilot_hooks;
    let dir = target.home.join(".copilot");
    let (hooks_path, settings_path) = (copilot_hooks::hooks_path(&dir), copilot_hooks::settings_path(&dir));
    let mut facts = AgentFacts {
        agent: "copilot",
        distro: Some(distro.to_string()),
        found: true,
        set_up: set_up_of(entries, &hooks_path),
        kept: false,
        path: hooks_path.clone(),
        file: FileState::Ok,
        problem_path: None,
        hooks: marshell_protocol::agents::HookInstall::None,
        approve_installed: false,
        has_own_statusline: false,
        chained: false,
        trust: None,
        bridge: PathBuf::from(&target.bridge),
        bridge_exists: bridge.exists(),
        version_note: None,
        retention_unset: None,
        managed_settings: None,
        notify: Default::default(),
        error: None,
    };
    let settings_text = match std::fs::read_to_string(&settings_path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            facts.file = FileState::Unparsable {
                line: 1,
                message: e.to_string(),
            };
            facts.problem_path = Some(settings_path);
            return facts;
        }
    };
    if let Some((line, message)) = settings_text.as_deref().and_then(parse_problem) {
        facts.file = FileState::Unparsable { line, message };
        facts.problem_path = Some(settings_path);
        return facts;
    }
    let (hooks_text, problem, is_kept) = copilot_hooks_problem(&hooks_path, &settings_path, kept);
    facts.kept = is_kept;
    if let Some(problem) = problem {
        facts.file = problem;
        return facts;
    }
    if hooks_text.is_none() && settings_text.is_none() {
        facts.file = FileState::Missing;
        return facts;
    }
    let read_only = |p: &Path| std::fs::metadata(p).is_ok_and(|m| m.permissions().readonly());
    if read_only(&hooks_path) || read_only(&settings_path) {
        facts.file = FileState::ReadOnly;
    }
    let found = copilot_hooks::inspect(
        &copilot_hooks::Files {
            hooks_path: &hooks_path,
            hooks_text: hooks_text.as_deref(),
            settings_path: &settings_path,
            settings_text: settings_text.as_deref(),
            chain_path: None,
            chain_text: None,
        },
        bridge,
    );
    facts.hooks = found.install;
    facts.notify = copilot_notify(&hooks_path, false);
    facts
}

fn facts_from(st: &AppState, agent: &'static str, setup: AgentSetup, entries: &[ManagedEntry]) -> AgentFacts {
    // Antigravity's hooks are in `hooks.json`; the others keep them in the file their setup names.
    let path = PathBuf::from(setup.hooks_path.as_deref().unwrap_or(&setup.settings_path));
    let mut problem_path = None;
    let mut file = match setup.settings_state {
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
            if agent == "copilot" {
                // The hooks file is `path`; it is `settings.json` that doesn't parse.
                problem_path = Some(PathBuf::from(&setup.settings_path));
            }
            FileState::Unparsable { line, message }
        }
    };
    let mut kept = false;
    if agent == "copilot" && !matches!(file, FileState::Unparsable { .. }) {
        let record = copilot_kept_record(&st.0.home);
        let (_, problem, is_kept) = copilot_hooks_problem(&path, Path::new(&setup.settings_path), &record);
        kept = is_kept;
        if let Some(problem) = problem {
            file = problem;
        } else if std::fs::metadata(&path).is_ok_and(|m| m.permissions().readonly()) {
            file = FileState::ReadOnly;
        }
    }
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
    // Only a CLI on this machine is asked what it notifies by itself.
    let notify = if agent == "copilot" && matches!(file, FileState::NotOurs { .. } | FileState::Unparsable { .. }) {
        // A hooks file that isn't Marshell's or a settings file that doesn't parse says nothing about notifications.
        checks::NotifyFacts::default()
    } else if setup.found {
        let hooks = matches!(file, FileState::Ok | FileState::ReadOnly)
            .then(|| {
                let text = std::fs::read_to_string(&path).ok()?;
                read_value(&text).ok()
            })
            .flatten();
        notify_facts(agent, &path, hooks.as_ref())
    } else {
        checks::NotifyFacts::default()
    };
    AgentFacts {
        agent,
        distro: None,
        found: setup.found,
        set_up: set_up_of(entries, &path),
        kept,
        path,
        file,
        problem_path,
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
        notify,
        error: None,
    }
}

fn version_gate(agent: &str, version: Option<&str>) -> Gate {
    use crate::agents::{antigravity, claude, codex, qwen};
    match agent {
        "claude" => claude::version_gate(version),
        "codex" => codex::version_gate(version),
        "antigravity" => antigravity::version_gate(version),
        "qwen" => qwen::version_gate(version),
        "copilot" => crate::agents::copilot::version_gate(version),
        _ => Gate::Known,
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
    let mut now = checks::items(&gathered.facts, &gathered.stopped, banners_on(st));
    // The same signals as the run that made the report, so the ids the report holds are the ids found here.
    let measured = st
        .doctor()
        .measure
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .results
        .clone();
    let signals = ext::Signals {
        usage: Some(&gathered.usage),
        projects: &gathered.projects,
        now_ms: now_ms(),
        measured: measured.as_ref(),
    };
    now.extend(ext::items(&gathered.ext, &signals));
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
    let mut switches: Vec<Switch> = Vec::new();
    // Copilot CLI's settings files whose `/notifications` this fix turns off.
    let mut own_settings: Vec<String> = Vec::new();
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
            // Copilot CLI's hooks file holds what Marshell didn't write, and the person picked the fix that replaces it. The
            // plan is built against the text the file has now.
            Some("hooksfile") if facts.agent == "copilot" => {
                let want = HookPlanRequest {
                    status: true,
                    ..hooks_want_of(facts, &entries)
                };
                let changes = match &facts.distro {
                    Some(distro) => st.distro_copilot_install_over(distro, &want)?,
                    None => copilot_install_over_changes(st, &want)?,
                };
                edits.extend(changes.edits);
            }
            Some("retention") => edits.push(retention_edit(facts)?),
            // Copilot CLI's own `notifications` turned off, against the text `settings.json` has now.
            Some("toasts") if facts.agent == "copilot" => {
                let edit = toasts_edit(facts)?;
                own_settings.push(edit.path.clone());
                edits.push(edit);
            }
            // A server or a skill turned off: the switch is read again from the inventory just gathered. Two items for one
            // server (a failed measurement and an unused server) give the same switch, which is taken once.
            Some("mcp" | "skill" | "mcpfail" | "mcpunused" | "skillunused") => {
                let switch = ext::switch_of(&gathered.ext, &signals, &item.id).ok_or_else(cant_fix)?;
                if !switches.contains(&switch) {
                    switches.push(switch);
                }
            }
            _ => return Err(cant_fix()),
        }
    }
    // One edit per list and one per Codex file, whatever the number of items. They are the person's own settings.
    let ext_edits = doctor_ext::edits(&switches, &gathered.ext_homes)?;
    edits.extend(ext_edits.iter().cloned());
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
    // The retention setting and every server or skill turned off are the person's, so applying them records no key in
    // managed.json.
    let preview =
        st.0.config
            .preview_mixed(cs, &|edit| is_own_edit(edit, &own_settings) || ext_edits.contains(edit));
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

/// An edit of a setting that is the person's own: managed.json records no key for it.
/// `copilot_settings` are the paths of Copilot CLI's settings files this fix turns `/notifications` off in.
fn is_own_edit(edit: &FileEdit, copilot_settings: &[String]) -> bool {
    matches!(&edit.op, EditOp::JsonSet { pointer, .. }
        if pointer == RETENTION_POINTER || (pointer == NOTIFICATIONS_POINTER && copilot_settings.contains(&edit.path)))
}

const NOTIFICATIONS_POINTER: &str = "/notifications";

/// Copilot CLI's `notifications` set to `false` in its `settings.json`, against the text of the file as it is now.
fn toasts_edit(facts: &AgentFacts) -> Result<FileEdit, Fail> {
    let path = &facts.notify.settings;
    let text = std::fs::read_to_string(path).map_err(|_| cant_fix())?;
    Ok(FileEdit {
        path: path.to_string_lossy().into_owned(),
        scope: ConfigScope::User,
        op: EditOp::JsonSet {
            pointer: NOTIFICATIONS_POINTER.to_string(),
            value: "false".to_string(),
        },
        expected_hash: Some(hash_bytes(text.as_bytes())),
        applies: Applies::NewSessions,
    })
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
    // Copilot CLI's status line is in `settings.json`, beside the `hooks` folder, and a distro install writes one too.
    let copilot_line = facts.agent == "copilot"
        && (facts.chained
            || facts
                .path
                .parent()
                .and_then(Path::parent)
                .map(|dir| dir.join("settings.json"))
                .is_some_and(|settings| managed_pointer(entries, &settings, |p| under(p, "/statusLine"))));
    let statusline = copilot_line
        || chains
            && facts.agent != "copilot"
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

/// `POST /v1/doctor/keep`: "Keep my file" (`keep: true`) or "Stop keeping" (`keep: false`) on the row with `id`. The facts
/// are read again, and the row must be in them with that action: anything else is a 422 that writes nothing. It writes
/// Marshell's own record of kept hooks files and nothing of a CLI's, then asks for a new run of the checks, as a fix
/// does, and answers 202 with the summary.
pub async fn keep(State(st): State<AppState>, Json(req): Json<DoctorKeepRequest>) -> Response {
    let work = {
        let st = st.clone();
        move || keep_of(&st, &req)
    };
    match tokio::task::spawn_blocking(work).await {
        Ok(Ok(())) => {
            request_run(&st);
            (StatusCode::ACCEPTED, Json(st.doctor().summary.borrow().clone())).into_response()
        }
        Ok(Err((status, message))) => (status, message).into_response(),
        Err(e) => server_error(e),
    }
}

fn keep_of(st: &AppState, req: &DoctorKeepRequest) -> Result<(), Fail> {
    let action = if req.keep {
        DoctorAction::Keep
    } else {
        DoctorAction::StopKeeping
    };
    // The files as they are now: the report may be minutes old.
    let gathered = facts_of(st);
    let now = checks::items(&gathered.facts, &gathered.stopped, banners_on(st));
    let item = now
        .iter()
        .find(|i| i.id == req.id && i.action == Some(action))
        .ok_or_else(cant_fix)?;
    let facts = gathered
        .facts
        .iter()
        .find(|f| Some(f.agent) == item.agent.as_deref() && f.distro == item.distro)
        .filter(|f| f.agent == "copilot")
        .ok_or_else(cant_fix)?;
    let record_path = kept::record_path(&st.0.home);
    let mut record = copilot_kept_record(&st.0.home);
    if req.keep {
        // The person's part is read again, so what is recorded is what is in the file now.
        let bytes = hooks_file_bytes(&facts.path).map_err(|_| cant_fix())?;
        let text = String::from_utf8(bytes).map_err(|_| cant_fix())?;
        let file = read_value(&text)
            .ok()
            .filter(serde_json::Value::is_object)
            .ok_or_else(cant_fix)?;
        if file_problem(Some(&text)) != Some(FileProblem::NotMarshells) {
            return Err(cant_fix());
        }
        record.keep(&facts.path, &file);
    } else if !record.drop_path(&facts.path) {
        return Err(cant_fix());
    }
    store::write_bytes_atomic(&record_path, record.to_text().as_bytes()).map_err(|e| {
        tracing::warn!("the doctor could not write the kept hooks record: {e:#}");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
    })
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
pub(in crate::server) mod tests {
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
            not_found_note: None,
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
        assert_eq!(facts.len(), 5);
        let items = checks::items(&facts, &[], true);
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        let failed = items.iter().find(|i| i.id == "error.codex").unwrap();
        assert_eq!(failed.group, DoctorGroup::Info);
        assert_eq!(failed.detail, "the home folder is not known");
        // The others were read and found, with no file: each gets its setup line.
        for agent in ["claude", "antigravity", "qwen", "copilot"] {
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
                ext: Vec::new(),
                ext_homes: Vec::new(),
                usage: Default::default(),
                projects: Vec::new(),
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

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn notify_facts_are_read_for_each_cli() {
        let dir = tempfile::tempdir().unwrap();
        let st = state(dir.path());
        let root = dir.path();

        // Antigravity's `notifications` stays in the terminal, so it gives no fact.
        let agy_settings = root.join("agy").join("settings.json");
        write(&agy_settings, r#"{"notifications": true}"#);
        let mut s = setup("antigravity");
        s.settings_path = agy_settings.to_string_lossy().into_owned();
        s.hooks_path = Some(root.join("agy").join("hooks.json").to_string_lossy().into_owned());
        s.settings_state = SettingsState::Ok;
        let f = facts_from(&st, "antigravity", s, &[]);
        assert_eq!(f.notify.command, None);
        assert!(f.notify.hook_commands.is_empty());

        let hooks = root.join("codex").join("hooks.json");
        let config = root.join("codex").join("config.toml");
        write(
            &hooks,
            r#"{"hooks": {"Notification": [{"hooks": [{"type": "command", "command": "toast.exe"}]}]}}"#,
        );
        write(&config, "notify = [\"a.exe\", \"x\"]\n");
        let mut s = setup("codex");
        s.settings_path = hooks.to_string_lossy().into_owned();
        s.hooks_path = Some(hooks.to_string_lossy().into_owned());
        s.settings_state = SettingsState::Ok;
        let f = facts_from(&st, "codex", s.clone(), &[]);
        // Codex has no Notification hook event (docs/research/codex-cli.md), so a group under it never runs.
        assert_eq!(
            f.notify,
            checks::NotifyFacts {
                file: config.clone(),
                command: Some("a.exe x".into()),
                hook_commands: Vec::new(),
                ..Default::default()
            }
        );
        // A config.toml that doesn't parse gives no command.
        write(&config, "notify = [");
        assert_eq!(facts_from(&st, "codex", s, &[]).notify.command, None);

        let claude = root.join("claude").join("settings.json");
        write(
            &claude,
            r#"{"hooks": {"Notification": [
                {"hooks": [
                    {"type": "command", "command": "toast.exe a"},
                    {"type": "command", "command": "bridge.exe", "args": ["--tag", "mshl1"]},
                    {"type": "command", "command": "bridge.exe --tag mshl1 notification"},
                    {"type": "http"}
                ]},
                {"hooks": [{"type": "command", "command": "b.sh"}]}
            ]}}"#,
        );
        let mut s = setup("claude");
        s.settings_path = claude.to_string_lossy().into_owned();
        s.settings_state = SettingsState::Ok;
        let f = facts_from(&st, "claude", s, &[]);
        assert_eq!(f.notify.file, claude);
        assert_eq!(f.notify.hook_commands, ["toast.exe a", "b.sh"]);
        assert_eq!(f.notify.command, None);
    }

    /// A state over folders under `root` with every path pinned, as the route tests do.
    pub(in crate::server) fn pinned(root: &Path) -> AppState {
        pinned_with(root, Vec::new())
    }

    pub(in crate::server) fn pinned_with(root: &Path, profiles: Vec<marshell_protocol::api::Profile>) -> AppState {
        use crate::projects::{canonical, record};
        let root = canonical(root);
        let home = root.join("home");
        let project = root.join("project");
        for d in ["home", "project/codex-home", "project/agy-home", "project/qwen-home"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        record(&home, &project, 1).unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.clone(), profiles)
            .with_claude_settings(project.join("settings.json"))
            .with_codex_hooks(project.join("codex-home").join("hooks.json"))
            .with_agy_home(project.join("agy-home"))
            .with_qwen_home(project.join("qwen-home"))
            .with_copilot_home(root.join("copilot-home"))
            .with_copilot_version(Some("1.0.106".into()))
            .with_qwen_system_settings(root.join("qwen-system.json"))
            .with_skill_home(root.join("skill-home"))
            .with_claude_compact_paths(root.join("claude-home"), vec![root.join("claude-managed.json")])
            .with_claude_version(Some("2.1.302".into()))
            .with_codex_version(Some("0.163.0".into()))
            .with_agy_version(Some("1.0.0".into()))
            .with_qwen_version(Some("0.26.5".into()))
            .with_agent_path(|name| Some(PathBuf::from(format!(r"Z:\nowhere\{name}.exe"))));
        let bridge = paths::bridge_exe(&home);
        std::fs::create_dir_all(bridge.parent().unwrap()).unwrap();
        std::fs::write(&bridge, "").unwrap();
        st
    }

    /// Antigravity's `notifications` rings the bell and sends OSC 9 into the terminal (agy 1.4.12 has no toast code),
    /// which Marshell ignores on a hooked row: nothing arrives twice, so there is no item and no fix (7b decision 4,
    /// amended after the binary was read).
    #[tokio::test]
    async fn antigravity_notifications_give_no_item_and_no_fix() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let agy_home = crate::projects::canonical(dir.path()).join("project").join("agy-home");
        let settings = crate::agents::antigravity::hooks::settings_path(&agy_home);
        write(&settings, "{\n  \"notifications\": true\n}\n");

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = st.doctor().report.lock().unwrap().clone().unwrap();
        assert!(
            !report.items.iter().any(|i| i.id.starts_with("notify.antigravity")),
            "{:?}",
            report.items
        );
        let (status, _) = fix_of(&st, vec!["notify.antigravity".into()]).expect_err("refused");
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn a_notify_fix_for_codex_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let codex_home = crate::projects::canonical(dir.path())
            .join("project")
            .join("codex-home");
        write(&codex_home.join("config.toml"), "notify = [\"a.exe\"]\n");

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = st.doctor().report.lock().unwrap().clone().unwrap();
        let item = report.items.iter().find(|i| i.id == "notify.codex").expect("the item");
        assert!(!item.fixable);
        let (status, _) = fix_of(&st, vec!["notify.codex".into()]).expect_err("refused");
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn banners_off_in_settings_gives_no_notify_item() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let codex_home = crate::projects::canonical(dir.path())
            .join("project")
            .join("codex-home");
        write(&codex_home.join("config.toml"), "notify = [\"a.exe\"]\n");
        assert!(banners_on(&st));
        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = st.doctor().report.lock().unwrap().clone().unwrap();
        assert!(
            report.items.iter().any(|i| i.id == "notify.codex"),
            "on: {:?}",
            report.items
        );
        st.0.settings.lock().unwrap().notifications = marshell_protocol::api::NotifySettings {
            needs_you: marshell_protocol::api::NotifyChannel {
                banner: false,
                sound: true,
            },
            done: marshell_protocol::api::NotifyChannel {
                banner: false,
                sound: true,
            },
            error: marshell_protocol::api::NotifyChannel {
                banner: false,
                sound: true,
            },
            ..Default::default()
        };
        assert!(!banners_on(&st));
        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = st.doctor().report.lock().unwrap().clone().unwrap();
        assert!(!report.items.iter().any(|i| i.id.starts_with("notify.")));
    }

    // The CLI doctors. Every runner is a pinned `fn`, so none runs a real CLI or `wsl.exe`; `pinned` finds every CLI at
    // a path that doesn't exist.

    const CODEX_FAIL: &str = r#"{"schemaVersion":1,"checks":{"auth":{"id":"auth","status":"fail","summary":"No login","remediation":"Run codex login"}}}"#;
    const CODEX_OK: &str = r#"{"schemaVersion":1,"checks":{"auth":{"id":"auth","status":"ok","summary":"Logged in"}}}"#;
    const CLAUDE_OK: &str = "No installation issues found.\n";

    fn is_codex(cmd: &DoctorCommand) -> bool {
        cmd.args.contains(&"--json")
    }

    fn claude_ok_codex_fails(cmd: &DoctorCommand) -> Ran {
        Some(if is_codex(cmd) {
            (1, CODEX_FAIL.to_string())
        } else {
            (0, CLAUDE_OK.to_string())
        })
    }

    /// Ends when no run of either kind is going.
    async fn wait_idle(st: &AppState) {
        let mut rx = st.doctor().summary.subscribe();
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            rx.wait_for(|s| !s.running && !s.cli_running),
        )
        .await
        .expect("the runs end")
        .unwrap();
    }

    fn last_report(st: &AppState) -> DoctorReport {
        st.doctor().report.lock().unwrap().clone().expect("a report")
    }

    #[tokio::test]
    async fn cli_doctors_fold_into_the_report() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path()).with_doctor_runner(claude_ok_codex_fails);
        request_cli_run(&st);
        wait_idle(&st).await;
        let report = last_report(&st);
        let broken = report
            .items
            .iter()
            .find(|i| i.id == "cli.codex.auth")
            .expect("the item");
        assert_eq!(broken.group, DoctorGroup::Broken);
        assert!(
            !broken.fixable && !broken.open_agents && broken.path.is_none(),
            "{broken:?}"
        );
        assert!(report.cli_checked_at_ms.is_some());
        // Claude was fine; Antigravity and Qwen Code have no doctor.
        assert!(!report.items.iter().any(|i| i.id == "cli.claude"));
        assert!(report.items.iter().any(|i| i.id == "cli.antigravity"));
        assert!(report.items.iter().any(|i| i.id == "cli.qwen"));
        let summary = st.doctor().summary.borrow().clone();
        assert_eq!(summary.issues, checks::issues(&report.items));
        let without: Vec<DoctorItem> = report
            .items
            .iter()
            .filter(|i| !i.id.starts_with("cli."))
            .cloned()
            .collect();
        assert_eq!(summary.issues, checks::issues(&without) + 1);
    }

    fn claude_times_out(cmd: &DoctorCommand) -> Ran {
        if is_codex(cmd) {
            Some((0, CODEX_OK.to_string()))
        } else {
            None
        }
    }

    #[tokio::test]
    async fn a_cli_doctor_that_times_out_leaves_the_others() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path()).with_doctor_runner(claude_times_out);
        request_cli_run(&st);
        wait_idle(&st).await;
        let report = last_report(&st);
        let ids: Vec<&str> = report
            .items
            .iter()
            .filter(|i| i.id.starts_with("cli."))
            .map(|i| i.id.as_str())
            .collect();
        assert!(ids.contains(&"cli.claude"), "{ids:?}");
        assert!(!ids.iter().any(|i| i.starts_with("cli.codex")), "{ids:?}");
        let item = report.items.iter().find(|i| i.id == "cli.claude").unwrap();
        assert!(item.title.contains("didn't give a result"), "{}", item.title);
        assert!(report.cli_checked_at_ms.is_some());
    }

    static JOIN_CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    static JOIN_HOLD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

    fn held_runner(cmd: &DoctorCommand) -> Ran {
        use std::sync::atomic::Ordering;
        JOIN_CALLS.fetch_add(1, Ordering::SeqCst);
        while JOIN_HOLD.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        claude_ok_codex_fails(cmd)
    }

    #[tokio::test]
    async fn a_second_request_joins_the_cli_run() {
        use std::sync::atomic::Ordering;
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path()).with_doctor_runner(held_runner);
        request_cli_run(&st);
        assert!(st.doctor().summary.borrow().cli_running);
        // Wait for the first CLI to be running, then ask again, twice.
        while JOIN_CALLS.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        request_cli_run(&st);
        request_cli_run(&st);
        assert!(st.doctor().summary.borrow().cli_running);
        JOIN_HOLD.store(false, Ordering::SeqCst);
        wait_idle(&st).await;
        // One call per CLI that has a doctor, whatever the number of requests.
        assert_eq!(JOIN_CALLS.load(Ordering::SeqCst), 2);
        assert!(!st.doctor().summary.borrow().cli_running);
        assert!(last_report(&st).items.iter().any(|i| i.id == "cli.codex.auth"));
    }

    #[tokio::test]
    async fn cli_items_survive_a_background_run() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path()).with_doctor_runner(claude_ok_codex_fails);
        request_cli_run(&st);
        wait_idle(&st).await;
        let before = last_report(&st);
        request_run(&st);
        wait_idle(&st).await;
        let after = last_report(&st);
        assert!(after.checked_at_ms >= before.checked_at_ms);
        assert!(after.items.iter().any(|i| i.id == "cli.codex.auth"));
        assert_eq!(after.cli_checked_at_ms, before.cli_checked_at_ms);
    }

    /// A thread watches the summary as fast as it can while the runs go on other threads. The first time it sees
    /// neither flag set, the report must already hold the new items: reading the summary first and the report after
    /// can only make the report newer, so a failure here is real.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn running_is_set_before_cli_running_clears() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path()).with_doctor_runner(claude_ok_codex_fails);
        request_cli_run(&st);
        let watched = st.clone();
        let seen = std::thread::spawn(move || {
            let started = Instant::now();
            loop {
                let s = watched.doctor().summary.borrow().clone();
                if !s.running && !s.cli_running {
                    let report = watched.doctor().report.lock().unwrap().clone();
                    return report.is_some_and(|r| r.items.iter().any(|i| i.id == "cli.codex.auth"));
                }
                assert!(started.elapsed().as_secs() < 10, "the runs did not end");
                std::thread::yield_now();
            }
        });
        assert!(
            seen.join().unwrap(),
            "a frame with neither flag set came before the new items"
        );
    }

    static SEEN: std::sync::Mutex<Vec<(Option<String>, String)>> = std::sync::Mutex::new(Vec::new());

    fn recording_runner(cmd: &DoctorCommand) -> Ran {
        SEEN.lock().unwrap().push((
            cmd.distro.clone(),
            cmd.exe.file_stem().unwrap().to_string_lossy().into_owned(),
        ));
        claude_ok_codex_fails(cmd)
    }

    #[tokio::test]
    async fn cli_doctors_run_only_in_running_managed_distros() {
        use marshell_protocol::api::{Profile, ShellFamily};
        let profile = |name: &str| Profile {
            id: name.to_lowercase(),
            name: name.into(),
            command: vec!["wsl.exe".into(), "-d".into(), name.into()],
            family: ShellFamily::Wsl,
            cwd: None,
            env: Vec::new(),
            shell_integration: false,
            terminal_theme: None,
            detected: true,
        };
        let dir = tempfile::tempdir().unwrap();
        let st = pinned_with(dir.path(), vec![profile("Ubuntu"), profile("Debian")])
            .with_wsl_version(Some("1.0.0".into()))
            .with_running_distros(|| vec!["Ubuntu".to_string()])
            .with_distro_target(Some(super::super::DistroTarget {
                home: dir.path().join("ubuntu-home"),
                bridge: "/mnt/c/marshell-hook.exe".into(),
            }))
            .with_doctor_runner(recording_runner);
        // Marshell set up both distros; only Ubuntu runs.
        let entry = |distro: &str| ManagedEntry {
            path: format!(r"\\wsl.localhost\{distro}\home\u\.claude\settings.json"),
            pointer: Some("/hooks/Stop/-".into()),
            changeset_id: "c".into(),
            applied_ms: 1,
        };
        crate::config::managed::save(&st.0.home, &[entry("Ubuntu"), entry("Debian")]).unwrap();

        request_cli_run(&st);
        wait_idle(&st).await;
        let seen = SEEN.lock().unwrap().clone();
        assert!(seen.contains(&(Some("Ubuntu".into()), "claude".into())), "{seen:?}");
        assert!(seen.contains(&(Some("Ubuntu".into()), "codex".into())), "{seen:?}");
        assert!(seen.contains(&(None, "claude".into())), "{seen:?}");
        assert!(!seen.iter().any(|(d, _)| d.as_deref() == Some("Debian")), "{seen:?}");
        let report = last_report(&st);
        assert!(report.items.iter().any(|i| i.id == "cli.codex.auth@Ubuntu"));
        assert!(report.items.iter().any(|i| i.id == "cli.antigravity@Ubuntu"));
        assert!(
            !report
                .items
                .iter()
                .any(|i| i.id.ends_with("@Debian") && i.id.starts_with("cli."))
        );
    }

    // Copilot CLI (slice 6n3): its facts, its hooks item and its plain lines. Every file is under the test's temp folder.

    fn copilot_files(dir: &Path) -> (PathBuf, PathBuf) {
        let copilot = crate::projects::canonical(dir).join("copilot-home").join(".copilot");
        (
            copilot.join("hooks").join("marshell.json"),
            copilot.join("settings.json"),
        )
    }

    /// Sets Copilot CLI's hooks up through its own plan, as "Set up" does.
    fn install_copilot(st: &AppState) {
        let want = HookPlanRequest {
            status: true,
            statusline: true,
            approve: false,
            skill: false,
            distro: None,
        };
        let cs = hook_install_changes(st, Adapter::Copilot, &want).expect("a plan");
        let preview = st.0.config.preview_mixed(cs, &|_| false);
        let req = ApplyRequest {
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
        st.0.config.apply(&req, now_ms()).expect("applied");
    }

    fn copilot_facts(st: &AppState) -> AgentFacts {
        facts_of(st)
            .facts
            .into_iter()
            .find(|f| f.agent == "copilot" && f.distro.is_none())
            .expect("copilot's facts")
    }

    async fn report_of(st: &AppState) -> DoctorReport {
        run_once(st, Arc::new(facts_of)).await.expect("a run");
        last_report(st)
    }

    fn has_item(report: &DoctorReport, id: &str) -> bool {
        report.items.iter().any(|i| i.id == id)
    }

    #[test]
    fn copilot_is_the_fifth_fact() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, _) = copilot_files(dir.path());
        let facts = facts_of(&st).facts;
        assert_eq!(facts.len(), 5);
        assert_eq!(facts[4].agent, "copilot");
        assert!(!facts[4].set_up);
        install_copilot(&st);
        let f = copilot_facts(&st);
        assert_eq!(f.path, hooks);
        assert!(f.set_up, "{f:?}");
        assert_eq!(f.file, FileState::Ok);
        assert_eq!(f.hooks, marshell_protocol::agents::HookInstall::Installed);
    }

    #[tokio::test]
    async fn copilot_hooks_gone_are_fixed_with_the_status_line() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, settings) = copilot_files(dir.path());
        install_copilot(&st);
        assert!(settings.exists());
        std::fs::remove_file(&hooks).unwrap();
        let report = report_of(&st).await;
        assert!(has_item(&report, "hooks.copilot"), "{:?}", report.items);
        let want = hooks_want_of(&copilot_facts(&st), &managed_entries(&st));
        assert!(want.statusline, "the line Marshell set is kept");

        // The person also took the status line out by hand: the fix puts both back.
        write(&settings, "{}");
        let report = report_of(&st).await;
        assert!(has_item(&report, "hooks.copilot"), "{:?}", report.items);
        let plan = fix_of(&st, vec!["hooks.copilot".into()]).expect("a plan");
        let paths: Vec<&str> = plan.preview.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.iter().any(|p| p.ends_with("marshell.json")), "{paths:?}");
        assert!(paths.iter().any(|p| p.ends_with("settings.json")), "{paths:?}");
        assert!(
            paths
                .iter()
                .all(|p| p.contains("copilot-home") || p.ends_with("statusline-chain.json")),
            "{paths:?}"
        );
    }

    /// Applies a fix plan the way the route does, and waits for the run that follows.
    async fn apply_fix_plan(st: &AppState, plan: &DoctorFixPlan) {
        let req = ApplyRequest {
            changeset_id: plan.preview.changeset_id.clone(),
            expected_hashes: plan
                .preview
                .files
                .iter()
                .map(|f| ExpectedHash {
                    path: f.path.clone(),
                    hash: f.before_hash.clone(),
                })
                .collect(),
        };
        let applied = fix_apply(State(st.clone()), Json(req)).await;
        assert_eq!(applied.status(), StatusCode::OK);
        wait_idle(st).await;
    }

    #[allow(clippy::permissions_set_readonly_false)]
    fn set_read_only(path: &Path, on: bool) {
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_readonly(on);
        std::fs::set_permissions(path, perms).unwrap();
    }

    #[tokio::test]
    async fn a_hooks_file_that_is_not_marshells_is_fixed_when_picked_and_undone() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, settings) = copilot_files(dir.path());
        install_copilot(&st);
        let own = std::fs::read_to_string(&hooks).unwrap();
        let theirs = r#"{"version":1,"hooks":{"preToolUse":[{"type":"command","exec":"mine.exe","args":["a"]}]}}"#;
        for text in ["{}", theirs] {
            std::fs::write(&hooks, text).unwrap();
            let settings_before = std::fs::read(&settings).unwrap();
            let report = report_of(&st).await;
            let item = report
                .items
                .iter()
                .find(|i| i.id == "hooksfile.copilot")
                .expect("the item");
            assert!(
                item.fixable && item.unchecked && item.group == DoctorGroup::Broken,
                "{item:?}"
            );
            assert_eq!(item.cannot_fix, None);
            assert!(!has_item(&report, "hooks.copilot"), "{:?}", report.items);

            let plan = fix_of(&st, vec!["hooksfile.copilot".into()]).expect("a plan");
            assert_eq!(plan.item_ids, ["hooksfile.copilot"]);
            let paths: Vec<&str> = plan.preview.files.iter().map(|f| f.path.as_str()).collect();
            assert_eq!(paths.len(), 1, "{paths:?}");
            assert!(paths[0].ends_with("marshell.json"), "{paths:?}");
            // Planning writes nothing.
            assert_eq!(std::fs::read_to_string(&hooks).unwrap(), text);

            apply_fix_plan(&st, &plan).await;
            assert_eq!(std::fs::read_to_string(&hooks).unwrap(), own, "Marshell's hooks again");
            assert_eq!(
                std::fs::read(&settings).unwrap(),
                settings_before,
                "the status line stays in place"
            );
            let report = report_of(&st).await;
            assert!(!has_item(&report, "hooksfile.copilot"), "{:?}", report.items);
            assert!(!has_item(&report, "hooks.copilot"), "{:?}", report.items);
            assert!(copilot_facts(&st).set_up);

            let undone = undo(State(st.clone())).await;
            assert_eq!(undone.status(), StatusCode::OK);
            assert_eq!(std::fs::read_to_string(&hooks).unwrap(), text, "byte-equal after Undo");
        }
    }

    #[tokio::test]
    async fn the_agents_page_still_refuses_a_hooks_file_that_is_not_marshells() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, _) = copilot_files(dir.path());
        install_copilot(&st);
        std::fs::write(&hooks, "{}").unwrap();
        let want = hooks_want_of(&copilot_facts(&st), &managed_entries(&st));
        assert!(hook_install_changes(&st, Adapter::Copilot, &want).is_err());
        assert!(copilot_install_over_changes(&st, &want).is_ok());
        // A file that doesn't parse is refused by both.
        std::fs::write(&hooks, "{ \"hooks\": ").unwrap();
        assert!(hook_install_changes(&st, Adapter::Copilot, &want).is_err());
        assert!(copilot_install_over_changes(&st, &want).is_err());
    }

    #[tokio::test]
    async fn a_hooks_file_that_is_not_text_is_reported_and_not_fixed() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, _) = copilot_files(dir.path());
        // Another CLI has something to fix meanwhile: Claude Code's retention.
        write(&dir.path().join("project").join("settings.json"), "{}");
        install_copilot(&st);
        std::fs::write(&hooks, [0xff, 0xfe, 0x7b, 0x7d]).unwrap();
        let report = report_of(&st).await;
        assert!(!has_item(&report, "hooks.copilot"), "{:?}", report.items);
        let item = report
            .items
            .iter()
            .find(|i| i.id == "hooksfile.copilot")
            .expect("the item");
        assert!(!item.fixable && !item.unchecked && item.group == DoctorGroup::Broken);
        assert_eq!(
            item.cannot_fix.as_deref(),
            Some("Marshell can't show this file in a preview. Move it away, then run the check again.")
        );
        assert!(fix_of(&st, vec!["hooksfile.copilot".into()]).is_err());
        assert!(fix_of(&st, vec!["retention.claude".into()]).is_ok());
        assert_eq!(std::fs::read(&hooks).unwrap(), [0xff, 0xfe, 0x7b, 0x7d]);
    }

    #[tokio::test]
    async fn a_hooks_file_that_is_not_marshells_beside_no_settings_file_is_still_fixable() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, settings) = copilot_files(dir.path());
        write(&dir.path().join("project").join("settings.json"), "{}");
        install_copilot(&st);
        std::fs::write(&hooks, "{}").unwrap();
        std::fs::remove_file(&settings).unwrap();
        let report = report_of(&st).await;
        let item = report
            .items
            .iter()
            .find(|i| i.id == "hooksfile.copilot")
            .expect("the item");
        assert!(item.fixable, "{item:?}");
        assert!(!has_item(&report, "hooks.copilot"), "{:?}", report.items);
        assert!(fix_of(&st, vec!["retention.claude".into()]).is_ok());
        assert!(fix_of(&st, vec!["hooksfile.copilot".into()]).is_ok());
    }

    #[tokio::test]
    async fn a_read_only_file_beside_a_hooks_file_that_is_not_marshells_makes_the_item_unfixable() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, settings) = copilot_files(dir.path());
        write(&dir.path().join("project").join("settings.json"), "{}");
        install_copilot(&st);
        std::fs::write(&hooks, "{}").unwrap();
        for file in [&hooks, &settings] {
            set_read_only(file, true);
            let report = report_of(&st).await;
            let outcome = report
                .items
                .iter()
                .find(|i| i.id == "hooksfile.copilot")
                .map(|i| (i.fixable, i.cannot_fix.clone()));
            let refused = fix_of(&st, vec!["hooksfile.copilot".into()]).is_err();
            let others = fix_of(&st, vec!["retention.claude".into()]).is_ok();
            set_read_only(file, false);
            assert_eq!(
                outcome,
                Some((false, Some("The file is read-only.".to_string()))),
                "{file:?}"
            );
            assert!(refused && others, "{file:?}");
            assert!(!has_item(&report, "hooks.copilot"), "{:?}", report.items);
        }
    }

    #[tokio::test]
    async fn a_distro_hooks_file_that_is_not_marshells_is_fixed_when_picked_and_undone() {
        use marshell_protocol::api::{Profile, ShellFamily};
        let dir = tempfile::tempdir().unwrap();
        let root = crate::projects::canonical(dir.path());
        let ubuntu = Profile {
            id: "ubuntu".into(),
            name: "Ubuntu".into(),
            command: vec!["wsl.exe".into(), "-d".into(), "Ubuntu".into()],
            family: ShellFamily::Wsl,
            cwd: None,
            env: Vec::new(),
            shell_integration: false,
            terminal_theme: None,
            detected: true,
        };
        let ubuntu_home = root.join("ubuntu-home");
        let st = pinned_with(&root, vec![ubuntu])
            .with_wsl_version(Some("1.0.0".into()))
            .with_running_distros(|| vec!["Ubuntu".to_string()])
            .with_distro_target(Some(super::super::DistroTarget {
                home: ubuntu_home.clone(),
                bridge: mnt(&paths::bridge_exe(&root.join("home"))),
            }));
        let hooks = ubuntu_home.join(".copilot").join("hooks").join("marshell.json");
        let want = HookPlanRequest {
            status: true,
            statusline: true,
            approve: false,
            skill: false,
            distro: Some("Ubuntu".into()),
        };
        let cs = st.distro_install(Adapter::Copilot, "Ubuntu", &want).expect("a plan");
        let preview = st.0.config.preview_mixed(cs, &|_| false);
        let req = ApplyRequest {
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
        st.0.config.apply(&req, now_ms()).expect("applied");
        let own = std::fs::read_to_string(&hooks).unwrap();

        std::fs::write(&hooks, "{}").unwrap();
        let report = report_of(&st).await;
        let item = report
            .items
            .iter()
            .find(|i| i.id == "hooksfile.copilot@Ubuntu")
            .unwrap_or_else(|| panic!("{:?}", report.items));
        assert!(item.fixable && item.unchecked, "{item:?}");
        assert!(!has_item(&report, "hooks.copilot@Ubuntu"), "{:?}", report.items);

        let plan = fix_of(&st, vec!["hooksfile.copilot@Ubuntu".into()]).expect("a plan");
        assert!(
            plan.preview.files.iter().all(|f| f.path.contains("ubuntu-home")),
            "{:?}",
            plan.preview.files
        );
        apply_fix_plan(&st, &plan).await;
        assert_eq!(std::fs::read_to_string(&hooks).unwrap(), own);
        let report = report_of(&st).await;
        assert!(!has_item(&report, "hooksfile.copilot@Ubuntu"), "{:?}", report.items);
        assert!(!has_item(&report, "hooks.copilot@Ubuntu"), "{:?}", report.items);

        let undone = undo(State(st.clone())).await;
        assert_eq!(undone.status(), StatusCode::OK);
        assert_eq!(std::fs::read_to_string(&hooks).unwrap(), "{}");
    }

    // Copilot CLI's own notifications and notification hooks (slice 6n3, decisions 2, 8 and 14).

    /// Settings text as a person's editor could leave it: a BOM, a comment, CRLF, a trailing comma, and `notifications`
    /// set to `value` (JSON text). Built on what "Set up" wrote, so the status line stays.
    fn settings_with_notifications(settings: &Path, value: &str) -> String {
        let text = std::fs::read_to_string(settings).unwrap();
        let body = text.trim_end().strip_suffix('}').unwrap().trim_end().to_string();
        let text = format!("\u{feff}// mine\n{body},\n  \"notifications\": {value}, // toasts\n}}\n");
        let text = text.replace('\n', "\r\n");
        std::fs::write(settings, &text).unwrap();
        text
    }

    #[test]
    fn copilots_own_hooks_and_toasts_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, settings) = copilot_files(dir.path());
        install_copilot(&st);
        settings_with_notifications(&settings, "true");
        let folder = hooks.parent().unwrap();
        write(
            &folder.join("b.json"),
            r#"{"hooks": {"notification": [{"type": "command", "command": "b1"}]}}"#,
        );
        write(
            &folder.join("a.json"),
            r#"{"hooks": {"notification": [{"type": "command", "bash": "a1"}]}}"#,
        );
        write(&folder.join("bad.json"), "{not json");
        write(&folder.join("notes.txt"), "notification");
        let f = copilot_facts(&st);
        assert!(f.notify.toasts, "{f:?}");
        assert_eq!(f.notify.settings, settings);
        assert_eq!(
            f.notify.hook_commands,
            ["a1", "b1"],
            "Marshell's own entries are never listed"
        );
        assert!(!f.notify.settings_read_only);
    }

    #[tokio::test]
    async fn a_hooks_file_of_the_persons_own_entries_is_read_when_kept() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, _) = foreign_copilot(&st, dir.path());
        edit_hooks(&hooks, |file| {
            file["hooks"]["notification"]
                .as_array_mut()
                .unwrap()
                .insert(0, serde_json::json!({"type": "command", "command": "mine.exe"}));
        });
        // Not kept yet: the file isn't Marshell's, so no notify fact.
        report_of(&st).await;
        assert!(copilot_facts(&st).notify.hook_commands.is_empty());
        assert_eq!(press(&st, "hooksfile.copilot", true).await.0, StatusCode::ACCEPTED);
        let f = copilot_facts(&st);
        assert!(f.kept);
        assert_eq!(f.notify.hook_commands, ["mine.exe"], "{f:?}");
    }

    #[test]
    fn a_distros_toasts_are_always_off_but_its_hooks_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".copilot");
        write(&home.join("settings.json"), r#"{"notifications": true}"#);
        write(
            &home.join("hooks").join("mine.json"),
            r#"{"hooks": {"notification": [{"type": "command", "command": "x"}]}}"#,
        );
        let hooks = home.join("hooks").join("marshell.json");
        let distro = copilot_notify(&hooks, false);
        assert!(!distro.toasts);
        assert_eq!(distro.hook_commands, ["x"]);
        assert!(copilot_notify(&hooks, true).toasts);
    }

    #[test]
    fn a_hooks_file_over_the_size_limit_is_not_read_for_its_notification_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".copilot");
        let small = r#"{"hooks": {"notification": [{"type": "command", "command": "small"}]}}"#;
        write(&home.join("hooks").join("mine.json"), small);
        let mut big = r#"{"hooks": {"notification": [{"type": "command", "command": "big"}]}}"#.to_string();
        big.push_str(&" ".repeat(crate::config::preview::MAX_CONFIG_BYTES as usize));
        write(&home.join("hooks").join("export.json"), &big);
        let facts = copilot_notify(&home.join("hooks").join("marshell.json"), true);
        assert_eq!(facts.hook_commands, ["small"]);
    }

    #[test]
    fn marshells_hooks_file_over_the_size_limit_is_not_read_and_offers_no_keep() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".copilot");
        let hooks = home.join("hooks").join("marshell.json");
        let mut big = r#"{"hooks": {"notification": [{"type": "command", "command": "mine"}]}}"#.to_string();
        big.push_str(&" ".repeat(crate::config::preview::MAX_CONFIG_BYTES as usize));
        write(&hooks, &big);
        let (text, problem, is_kept) = copilot_hooks_problem(&hooks, &home.join("settings.json"), &Kept::parse(None));
        assert_eq!(text, None);
        assert_eq!(
            problem,
            Some(FileState::NotOurs {
                cannot_fix: Some(NO_PREVIEW),
                keepable: false
            })
        );
        assert!(!is_kept);
    }

    #[test]
    fn only_a_literal_true_gives_a_toast_fact() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (_, settings) = copilot_files(dir.path());
        install_copilot(&st);
        for (value, on) in [("true", true), ("\"true\"", false), ("1", false), ("false", false)] {
            settings_with_notifications(&settings, value);
            assert_eq!(copilot_facts(&st).notify.toasts, on, "{value}");
        }
    }

    #[tokio::test]
    async fn copilots_toast_item_is_fixed_by_one_edit_that_keeps_every_other_byte() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (_, settings) = copilot_files(dir.path());
        install_copilot(&st);
        let before = settings_with_notifications(&settings, "true");
        let report = report_of(&st).await;
        let item = item_of(&report, "toasts.copilot").unwrap_or_else(|| panic!("{:?}", report.items));
        assert!(item.fixable && !item.unchecked, "{item:?}");

        let plan = fix_of(&st, vec!["toasts.copilot".into()]).expect("a plan");
        assert_eq!(plan.item_ids, ["toasts.copilot"]);
        assert_eq!(plan.preview.files.len(), 1, "{:?}", plan.preview.files);
        apply_fix_plan(&st, &plan).await;
        let after = std::fs::read_to_string(&settings).unwrap();
        assert_eq!(
            after,
            before.replacen("\"notifications\": true", "\"notifications\": false", 1)
        );
        assert!(
            !format!("{:?}", managed_entries(&st)).contains("/notifications"),
            "managed.json must not name the person's own setting"
        );
        let report = report_of(&st).await;
        assert!(!has_item(&report, "toasts.copilot"), "{:?}", report.items);

        let undone = undo(State(st.clone())).await;
        assert_eq!(undone.status(), StatusCode::OK);
        assert_eq!(std::fs::read(&settings).unwrap(), before.as_bytes());
    }

    #[tokio::test]
    async fn the_toast_fix_and_the_hooks_fix_apply_together() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, settings) = copilot_files(dir.path());
        install_copilot(&st);
        settings_with_notifications(&settings, "true");
        std::fs::remove_file(&hooks).unwrap();
        let report = report_of(&st).await;
        assert!(has_item(&report, "hooks.copilot"), "{:?}", report.items);
        // The status line is still Marshell's, so its hooks count as set up and both items are there.
        assert!(has_item(&report, "toasts.copilot"), "{:?}", report.items);
        let plan = fix_of(&st, vec!["toasts.copilot".into(), "hooks.copilot".into()]).expect("a plan");
        apply_fix_plan(&st, &plan).await;
        assert!(
            std::fs::read_to_string(&settings)
                .unwrap()
                .contains("\"notifications\": false")
        );
        assert_eq!(
            copilot_facts(&st).hooks,
            marshell_protocol::agents::HookInstall::Installed
        );
    }

    #[tokio::test]
    async fn a_string_true_gives_no_toast_item() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (_, settings) = copilot_files(dir.path());
        install_copilot(&st);
        settings_with_notifications(&settings, "\"true\"");
        let report = report_of(&st).await;
        assert!(!has_item(&report, "toasts.copilot"), "{:?}", report.items);
    }

    // "Keep my file" and "Stop keeping" (slice 6n3, decision 20).

    fn persons_entry() -> serde_json::Value {
        serde_json::json!({"type": "command", "bash": "echo mine"})
    }

    /// Changes the hooks file as a person with an editor would: the text is parsed, changed and written back.
    fn edit_hooks(path: &Path, change: impl FnOnce(&mut serde_json::Value)) {
        let mut file: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        change(&mut file);
        std::fs::write(path, serde_json::to_string_pretty(&file).unwrap()).unwrap();
    }

    /// The person's own entry at the head of Marshell's `preToolUse` list.
    fn add_persons_entry(file: &mut serde_json::Value) {
        file["hooks"]["preToolUse"]
            .as_array_mut()
            .unwrap()
            .insert(0, persons_entry());
    }

    fn item_of<'a>(report: &'a DoctorReport, id: &str) -> Option<&'a DoctorItem> {
        report.items.iter().find(|i| i.id == id)
    }

    /// Presses a Doctor row's button, and waits for the run that follows a success. The status and the body.
    async fn press(st: &AppState, id: &str, keep_it: bool) -> (StatusCode, String) {
        let answer = keep(
            State(st.clone()),
            Json(DoctorKeepRequest {
                id: id.into(),
                keep: keep_it,
            }),
        )
        .await;
        let status = answer.status();
        let body = axum::body::to_bytes(answer.into_body(), usize::MAX).await.unwrap();
        wait_idle(st).await;
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    /// The hooks file after a person added an entry of their own, and the settings file, as bytes.
    fn foreign_copilot(st: &AppState, dir: &Path) -> (PathBuf, PathBuf) {
        let (hooks, settings) = copilot_files(dir);
        install_copilot(st);
        edit_hooks(&hooks, add_persons_entry);
        (hooks, settings)
    }

    fn record_holds(st: &AppState, hooks: &Path) -> bool {
        let text = std::fs::read_to_string(kept::record_path(&st.0.home)).ok();
        let file = serde_json::from_str(&std::fs::read_to_string(hooks).unwrap()).unwrap();
        Kept::parse(text.as_deref()).holds(hooks, &file)
    }

    #[tokio::test]
    async fn keep_my_file_writes_only_the_record_and_the_fix_then_changes_only_marshells_entries() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, settings) = foreign_copilot(&st, dir.path());
        let report = report_of(&st).await;
        let row = item_of(&report, "hooksfile.copilot").expect("the item");
        assert_eq!(row.action, Some(DoctorAction::Keep), "{row:?}");

        // Keep: the record is written, and neither of Copilot's files changes.
        let (hooks_before, settings_before) = (std::fs::read(&hooks).unwrap(), std::fs::read(&settings).unwrap());
        let record = kept::record_path(&st.0.home);
        assert!(!record.exists());
        assert_eq!(press(&st, "hooksfile.copilot", true).await.0, StatusCode::ACCEPTED);
        assert_eq!(std::fs::read(&hooks).unwrap(), hooks_before);
        assert_eq!(std::fs::read(&settings).unwrap(), settings_before);
        assert!(record_holds(&st, &hooks));
        let leftovers: Vec<_> = std::fs::read_dir(record.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        let report = last_report(&st);
        let row = item_of(&report, "hookskept.copilot").expect("the kept row");
        assert_eq!(row.action, Some(DoctorAction::StopKeeping));
        assert_eq!(row.group, DoctorGroup::Info);
        assert!(
            !has_item(&report, "hooksfile.copilot") && !has_item(&report, "hooks.copilot"),
            "{:?}",
            report.items
        );
        assert!(copilot_facts(&st).kept);

        // One of Marshell's entries is deleted by hand: the hooks fix puts back only it.
        edit_hooks(&hooks, |f| {
            f["hooks"].as_object_mut().unwrap().shift_remove("sessionStart");
        });
        let report = report_of(&st).await;
        let row = item_of(&report, "hooks.copilot").expect("the hooks item is back");
        assert!(row.fixable && row.unchecked, "{row:?}");
        assert!(has_item(&report, "hookskept.copilot") && !has_item(&report, "hooksfile.copilot"));
        let want = hooks_want_of(&copilot_facts(&st), &managed_entries(&st));
        let changes = hook_install_changes(&st, Adapter::Copilot, &want).expect("a plan");
        let on_hooks: Vec<_> = changes
            .edits
            .iter()
            .filter(|e| e.path == hooks.to_string_lossy())
            .collect();
        assert!(!on_hooks.is_empty());
        assert!(
            on_hooks
                .iter()
                .all(|e| matches!(e.op, EditOp::JsonSet { .. } | EditOp::JsonRemove { .. })),
            "{on_hooks:?}"
        );
        let plan = fix_of(&st, vec!["hooks.copilot".into()]).expect("a plan");
        let before_fix = std::fs::read(&hooks).unwrap();
        apply_fix_plan(&st, &plan).await;
        let after: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&hooks).unwrap()).unwrap();
        assert_eq!(after["hooks"]["preToolUse"][0], persons_entry());
        assert!(after["hooks"]["sessionStart"].is_array());
        let report = last_report(&st);
        assert!(!has_item(&report, "hooks.copilot") && has_item(&report, "hookskept.copilot"));
        assert_eq!(
            copilot_facts(&st).hooks,
            marshell_protocol::agents::HookInstall::Installed
        );
        assert!(record_holds(&st, &hooks), "the file is still kept");
        let undone = undo(State(st.clone())).await;
        assert_eq!(undone.status(), StatusCode::OK);
        assert_eq!(std::fs::read(&hooks).unwrap(), before_fix, "byte-equal after Undo");

        // The person's own entry changes: the item comes back and the kept row goes.
        edit_hooks(&hooks, |f| {
            f["hooks"]["preToolUse"][0]["bash"] = serde_json::json!("echo other")
        });
        let report = report_of(&st).await;
        assert!(has_item(&report, "hooksfile.copilot"), "{:?}", report.items);
        assert!(!has_item(&report, "hookskept.copilot"), "{:?}", report.items);
        assert!(!record_holds(&st, &hooks));
        // A fix of the hooks does not take what the person wrote away: the file is not kept now, so it is refused.
        let want = hooks_want_of(&copilot_facts(&st), &managed_entries(&st));
        assert!(hook_install_changes(&st, Adapter::Copilot, &want).is_err());
    }

    #[tokio::test]
    async fn stop_keeping_brings_the_item_back_and_the_replace_fix_works_again() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, _) = foreign_copilot(&st, dir.path());
        let foreign = std::fs::read_to_string(&hooks).unwrap();
        assert_eq!(press(&st, "hooksfile.copilot", true).await.0, StatusCode::ACCEPTED);
        assert!(has_item(&last_report(&st), "hookskept.copilot"));

        // The wrong button on a row: refused, nothing is written.
        let record = kept::record_path(&st.0.home);
        let record_before = std::fs::read(&record).unwrap();
        assert_eq!(
            press(&st, "hookskept.copilot", true).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(std::fs::read(&record).unwrap(), record_before);

        assert_eq!(press(&st, "hookskept.copilot", false).await.0, StatusCode::ACCEPTED);
        assert_eq!(
            std::fs::read_to_string(&hooks).unwrap(),
            foreign,
            "the file is not touched"
        );
        assert!(!record_holds(&st, &hooks));
        let report = last_report(&st);
        assert!(has_item(&report, "hooksfile.copilot"), "{:?}", report.items);
        assert!(!has_item(&report, "hookskept.copilot"), "{:?}", report.items);
        assert_eq!(
            item_of(&report, "hooksfile.copilot").unwrap().action,
            Some(DoctorAction::Keep)
        );

        // The replace fix works as before, and Undo gives the person's file back.
        let plan = fix_of(&st, vec!["hooksfile.copilot".into()]).expect("a plan");
        apply_fix_plan(&st, &plan).await;
        let report = last_report(&st);
        assert!(
            !has_item(&report, "hooksfile.copilot") && !has_item(&report, "hooks.copilot"),
            "{:?}",
            report.items
        );
        let undone = undo(State(st.clone())).await;
        assert_eq!(undone.status(), StatusCode::OK);
        assert_eq!(std::fs::read_to_string(&hooks).unwrap(), foreign);
    }

    #[tokio::test]
    async fn keep_refuses_what_the_fresh_items_do_not_offer_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, _) = copilot_files(dir.path());
        install_copilot(&st);
        let record = kept::record_path(&st.0.home);
        let cant = "This item can't be fixed any more. Run the check again.";
        report_of(&st).await;

        // Marshell's own file has no item; an id that isn't in the items; the other button.
        for (id, keep_it) in [
            ("hooksfile.copilot", true),
            ("hookskept.copilot", false),
            ("nothing.copilot", true),
            ("hooks.claude", true),
            ("", false),
        ] {
            let (status, body) = press(&st, id, keep_it).await;
            assert_eq!(
                (status, body.as_str()),
                (StatusCode::UNPROCESSABLE_ENTITY, cant),
                "{id} {keep_it}"
            );
        }
        assert!(!record.exists());

        // A foreign file: Stop keeping is not offered, and a file that is not text has no Keep.
        edit_hooks(&hooks, add_persons_entry);
        report_of(&st).await;
        let (status, body) = press(&st, "hooksfile.copilot", false).await;
        assert_eq!((status, body.as_str()), (StatusCode::UNPROCESSABLE_ENTITY, cant));
        std::fs::write(&hooks, [0xff, 0xfe, 0x7b, 0x7d]).unwrap();
        let report = report_of(&st).await;
        assert_eq!(item_of(&report, "hooksfile.copilot").unwrap().action, None);
        let (status, body) = press(&st, "hooksfile.copilot", true).await;
        assert_eq!((status, body.as_str()), (StatusCode::UNPROCESSABLE_ENTITY, cant));
        assert_eq!(std::fs::read(&hooks).unwrap(), [0xff, 0xfe, 0x7b, 0x7d]);
        // Text that parses to something other than an object has no Keep either.
        std::fs::write(&hooks, "[1]").unwrap();
        let report = report_of(&st).await;
        assert_eq!(item_of(&report, "hooksfile.copilot").unwrap().action, None);
        assert_eq!(
            press(&st, "hooksfile.copilot", true).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert!(!record.exists(), "nothing was written");
    }

    #[tokio::test]
    async fn a_read_only_hooks_file_can_be_kept_and_reads_as_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, _) = foreign_copilot(&st, dir.path());
        set_read_only(&hooks, true);
        let report = report_of(&st).await;
        let row = item_of(&report, "hooksfile.copilot").expect("the item");
        let shown = (row.fixable, row.action, row.cannot_fix.clone());
        let (status, _) = press(&st, "hooksfile.copilot", true).await;
        let (file, kept_now) = {
            let f = copilot_facts(&st);
            (f.file, f.kept)
        };
        set_read_only(&hooks, false);
        assert_eq!(
            shown,
            (
                false,
                Some(DoctorAction::Keep),
                Some("The file is read-only.".to_string())
            )
        );
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(file, FileState::ReadOnly);
        assert!(kept_now);
    }

    #[tokio::test]
    async fn a_kept_file_in_a_distro_is_kept_fixed_entry_by_entry_and_not_kept_any_more() {
        use marshell_protocol::api::{Profile, ShellFamily};
        let dir = tempfile::tempdir().unwrap();
        let root = crate::projects::canonical(dir.path());
        let ubuntu = Profile {
            id: "ubuntu".into(),
            name: "Ubuntu".into(),
            command: vec!["wsl.exe".into(), "-d".into(), "Ubuntu".into()],
            family: ShellFamily::Wsl,
            cwd: None,
            env: Vec::new(),
            shell_integration: false,
            terminal_theme: None,
            detected: true,
        };
        let ubuntu_home = root.join("ubuntu-home");
        let st = pinned_with(&root, vec![ubuntu])
            .with_wsl_version(Some("1.0.0".into()))
            .with_running_distros(|| vec!["Ubuntu".to_string()])
            .with_distro_target(Some(super::super::DistroTarget {
                home: ubuntu_home.clone(),
                bridge: mnt(&paths::bridge_exe(&root.join("home"))),
            }));
        let distro_hooks = ubuntu_home.join(".copilot").join("hooks").join("marshell.json");
        let want = HookPlanRequest {
            status: true,
            statusline: true,
            approve: false,
            skill: false,
            distro: Some("Ubuntu".into()),
        };
        let cs = st.distro_install(Adapter::Copilot, "Ubuntu", &want).expect("a plan");
        let preview = st.0.config.preview_mixed(cs, &|_| false);
        let req = ApplyRequest {
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
        st.0.config.apply(&req, now_ms()).expect("applied");
        // Windows' own file is set up and kept too.
        let (hooks, _) = foreign_copilot(&st, dir.path());
        for file in [&hooks, &distro_hooks] {
            edit_hooks(file, add_persons_entry);
        }
        let report = report_of(&st).await;
        for id in ["hooksfile.copilot", "hooksfile.copilot@Ubuntu"] {
            assert_eq!(item_of(&report, id).unwrap().action, Some(DoctorAction::Keep), "{id}");
        }
        assert_eq!(
            press(&st, "hooksfile.copilot@Ubuntu", true).await.0,
            StatusCode::ACCEPTED
        );
        assert_eq!(press(&st, "hooksfile.copilot", true).await.0, StatusCode::ACCEPTED);
        let report = last_report(&st);
        for id in ["hookskept.copilot", "hookskept.copilot@Ubuntu"] {
            assert!(has_item(&report, id), "{id}: {:?}", report.items);
        }
        assert!(record_holds(&st, &distro_hooks) && record_holds(&st, &hooks));

        // A hooks fix in the distro puts one entry back and leaves the rest.
        edit_hooks(&distro_hooks, |f| {
            f["hooks"].as_object_mut().unwrap().shift_remove("sessionStart");
        });
        let report = report_of(&st).await;
        assert!(has_item(&report, "hooks.copilot@Ubuntu"), "{:?}", report.items);
        assert!(!has_item(&report, "hooks.copilot"), "{:?}", report.items);
        let plan = fix_of(&st, vec!["hooks.copilot@Ubuntu".into()]).expect("a plan");
        apply_fix_plan(&st, &plan).await;
        let after: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&distro_hooks).unwrap()).unwrap();
        assert_eq!(after["hooks"]["preToolUse"][0], persons_entry());
        assert!(after["hooks"]["sessionStart"].is_array());
        let report = last_report(&st);
        assert!(has_item(&report, "hookskept.copilot@Ubuntu") && !has_item(&report, "hooks.copilot@Ubuntu"));

        // Stop keeping in the distro leaves Windows' file kept.
        assert_eq!(
            press(&st, "hookskept.copilot@Ubuntu", false).await.0,
            StatusCode::ACCEPTED
        );
        let report = last_report(&st);
        assert!(has_item(&report, "hooksfile.copilot@Ubuntu"), "{:?}", report.items);
        assert!(has_item(&report, "hookskept.copilot"), "{:?}", report.items);
        assert!(!record_holds(&st, &distro_hooks) && record_holds(&st, &hooks));
    }

    #[tokio::test]
    async fn a_read_only_copilot_file_makes_its_hooks_item_unfixable() {
        use crate::agents::common::Bridge;
        use crate::agents::copilot::hooks as copilot_hooks;
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (hooks, settings) = copilot_files(dir.path());
        install_copilot(&st);
        // Marshell's own file with no entry: the hooks read as incomplete.
        let own_empty = copilot_hooks::hooks_file(
            &Bridge::local(&st.0.home),
            &HookPlanRequest {
                status: false,
                statusline: false,
                approve: false,
                skill: false,
                distro: None,
            },
        );
        std::fs::write(&hooks, own_empty).unwrap();
        for file in [&hooks, &settings] {
            let mut perms = std::fs::metadata(file).unwrap().permissions();
            perms.set_readonly(true);
            std::fs::set_permissions(file, perms).unwrap();
            let report = report_of(&st).await;
            let item = report.items.iter().find(|i| i.id == "hooks.copilot").expect("the item");
            assert!(!item.fixable, "{item:?}");
            assert_eq!(item.cannot_fix.as_deref(), Some("The file is read-only."));
            #[allow(clippy::permissions_set_readonly_false)]
            {
                let mut perms = std::fs::metadata(file).unwrap().permissions();
                perms.set_readonly(false);
                std::fs::set_permissions(file, perms).unwrap();
            }
        }
    }

    #[test]
    fn an_unparsable_copilot_settings_file_is_the_one_named() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path());
        let (_, settings) = copilot_files(dir.path());
        install_copilot(&st);
        write(&settings, "{ \"statusLine\": ");
        let f = copilot_facts(&st);
        assert!(matches!(f.file, FileState::Unparsable { .. }), "{:?}", f.file);
        assert_eq!(f.problem_path, Some(settings.clone()));
        let items = checks::items(&[f], &[], true);
        let parse = items.iter().find(|i| i.id == "parse.copilot").expect("the item");
        assert!(
            parse.title.starts_with(&settings.display().to_string()),
            "{}",
            parse.title
        );
        assert_eq!(parse.path.as_deref(), Some(settings.display().to_string().as_str()));
    }

    /// A distro's `/mnt/<drive>/...` form of a Windows path.
    fn mnt(path: &Path) -> String {
        let text = path.to_string_lossy().replace('\\', "/");
        let text = text.strip_prefix("//?/").unwrap_or(&text).to_string();
        format!("/mnt/{}{}", text[..1].to_ascii_lowercase(), &text[2..])
    }

    #[test]
    fn copilot_in_a_distro_reads_its_two_files_as_text() {
        use crate::agents::common::Bridge;
        use crate::agents::copilot::hooks as copilot_hooks;
        let dir = tempfile::tempdir().unwrap();
        let root = crate::projects::canonical(dir.path());
        let bridge_exe = root.join("bridge").join("marshell-hook.exe");
        write(&bridge_exe, "");
        let target = super::super::DistroTarget {
            home: root.join("ubuntu-home"),
            bridge: mnt(&bridge_exe),
        };
        let dot = target.home.join(".copilot");
        let (hooks, settings) = (dot.join("hooks").join("marshell.json"), dot.join("settings.json"));

        let none = distro_agent_facts("copilot", "Ubuntu", &target, &[], &Kept::default());
        assert_eq!(none.file, FileState::Missing);
        assert_eq!(none.path, hooks);

        let bridge = Bridge::Distro {
            name: "Ubuntu".into(),
            path: target.bridge.clone(),
        };
        let want = HookPlanRequest {
            status: true,
            statusline: false,
            approve: false,
            skill: false,
            distro: Some("Ubuntu".into()),
        };
        write(&hooks, &copilot_hooks::hooks_file(&bridge, &want));
        let command = copilot_hooks::statusline_command(&bridge).unwrap();
        write(
            &settings,
            &serde_json::json!({"statusLine": {"type": "command", "command": command}}).to_string(),
        );
        let entries = [entry(&hooks.to_string_lossy(), None)];
        let f = distro_agent_facts("copilot", "Ubuntu", &target, &entries, &Kept::default());
        assert_eq!(f.file, FileState::Ok, "{f:?}");
        assert_eq!(f.hooks, marshell_protocol::agents::HookInstall::Installed, "{f:?}");
        assert!(f.set_up);
        assert_eq!(f.distro.as_deref(), Some("Ubuntu"));

        write(&hooks, "{}");
        let f = distro_agent_facts("copilot", "Ubuntu", &target, &entries, &Kept::default());
        assert_eq!(
            f.file,
            FileState::NotOurs {
                cannot_fix: None,
                keepable: true
            }
        );

        // The status line is asked for when managed.json says Marshell set it; a Qwen Code fact never is.
        let mut with_line = entries.to_vec();
        with_line.push(entry(&settings.to_string_lossy(), Some("/statusLine")));
        write(&hooks, &copilot_hooks::hooks_file(&bridge, &want));
        let f = distro_agent_facts("copilot", "Ubuntu", &target, &with_line, &Kept::default());
        assert!(hooks_want_of(&f, &with_line).statusline);
        assert!(!hooks_want_of(&f, &entries).statusline);
        let q = distro_agent_facts("qwen", "Ubuntu", &target, &with_line, &Kept::default());
        assert!(!hooks_want_of(&q, &with_line).statusline);
    }

    #[test]
    fn an_unknown_agent_gets_failed_facts() {
        let dir = tempfile::tempdir().unwrap();
        let target = super::super::DistroTarget {
            home: dir.path().join("h"),
            bridge: "/mnt/c/x.exe".into(),
        };
        let f = distro_agent_facts("nobody", "Ubuntu", &target, &[], &Kept::default());
        assert!(f.error.is_some());
    }

    #[test]
    fn copilot_has_no_doctor_command() {
        let dir = tempfile::tempdir().unwrap();
        let st = pinned(dir.path()).with_doctor_runner(claude_ok_codex_fails);
        let items = cli_doctor_items(&st);
        assert!(
            items.iter().any(|i| i.title == "Copilot CLI has no doctor command."),
            "{items:?}"
        );
    }

    // MCP servers and skills (slice 7c1). `with_ext_home` points the readers at a fake home, and a pinned PATH check
    // makes `ghost` the one command that isn't there.

    fn ext_state(root: &Path) -> (AppState, PathBuf) {
        let ext = crate::projects::canonical(root).join("ext-home");
        std::fs::create_dir_all(&ext).unwrap();
        let st = pinned(root)
            .with_ext_home(ext.clone())
            .with_agent_check(|command| command != "ghost");
        (st, ext)
    }

    fn broken_ext_ids(report: &DoctorReport) -> Vec<String> {
        report
            .items
            .iter()
            .filter(|i| i.group == DoctorGroup::Broken && (i.id.starts_with("mcp.") || i.id.starts_with("skill.")))
            .map(|i| i.id.clone())
            .collect()
    }

    fn bytes(path: &Path) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }

    fn managed_pairs(st: &AppState) -> Vec<(String, Option<String>)> {
        managed_entries(st).into_iter().map(|e| (e.path, e.pointer)).collect()
    }

    #[tokio::test]
    async fn fix_apply_and_undo_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        let project = crate::projects::canonical(dir.path()).join("project");
        let codex_cfg = ext.join(".codex").join("config.toml");
        let qwen_cfg = ext.join(".qwen").join("settings.json");
        let skill = ext.join(".qwen").join("skills").join("nodesc").join("SKILL.md");
        let shared = project.join(".mcp.json");
        let local = project.join(".claude").join("settings.local.json");
        write(&codex_cfg, "# mine\n[mcp_servers.ghostly]\ncommand = \"ghost\"\n");
        write(&qwen_cfg, "{\n  \"theme\": \"dark\"\n}\n");
        write(&skill, "---\nname: nodesc\n---\nbody\n");
        write(&shared, r#"{"mcpServers":{"proj-ghost":{"command":"ghost"}}}"#);
        let before: Vec<Option<Vec<u8>>> = [&codex_cfg, &qwen_cfg, &shared, &local]
            .iter()
            .map(|p| bytes(p))
            .collect();
        let managed_before = managed_pairs(&st);

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = last_report(&st);
        let ids = broken_ext_ids(&report);
        // Claude Code, Qwen Code and Copilot CLI all read the project's `.mcp.json`, so its server is three items,
        // beside Codex's server and Qwen Code's skill.
        assert_eq!(ids.len(), 5, "{:?}", report.items);
        assert!(
            report.items.iter().filter(|i| ids.contains(&i.id)).all(|i| i.fixable),
            "{:?}",
            report.items
        );

        let plan = fix_of(&st, ids.clone()).expect("a plan");
        assert!(plan.preview.can_apply, "{:?}", plan.preview);
        let req = ApplyRequest {
            changeset_id: plan.preview.changeset_id.clone(),
            expected_hashes: plan
                .preview
                .files
                .iter()
                .map(|f| ExpectedHash {
                    path: f.path.clone(),
                    hash: f.before_hash.clone(),
                })
                .collect(),
        };
        let applied = fix_apply(State(st.clone()), Json(req)).await;
        assert_eq!(applied.status(), StatusCode::OK);
        wait_idle(&st).await;

        let codex_after = std::fs::read_to_string(&codex_cfg).unwrap();
        assert!(codex_after.contains("# mine"), "{codex_after}");
        let doc: toml::Table = codex_after.parse().unwrap();
        assert_eq!(doc["mcp_servers"]["ghostly"]["enabled"].as_bool(), Some(false));
        let qwen_after: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&qwen_cfg).unwrap()).unwrap();
        assert_eq!(qwen_after["skills"]["disabled"], serde_json::json!(["nodesc"]));
        assert_eq!(qwen_after["mcp"]["excluded"], serde_json::json!(["proj-ghost"]));
        assert_eq!(qwen_after["theme"], "dark");
        let local_after: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&local).unwrap()).unwrap();
        assert_eq!(local_after["disabledMcpjsonServers"], serde_json::json!(["proj-ghost"]));
        // The project's shared file is never written, and the person's own edits are not Marshell's entries.
        assert_eq!(bytes(&shared), before[2]);
        assert_eq!(managed_pairs(&st), managed_before);
        // The next report has none of the three.
        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        assert_eq!(broken_ext_ids(&last_report(&st)), Vec::<String>::new());

        let undone = undo(State(st.clone())).await;
        assert_eq!(undone.status(), StatusCode::OK);
        wait_idle(&st).await;
        let after: Vec<Option<Vec<u8>>> = [&codex_cfg, &qwen_cfg, &shared, &local]
            .iter()
            .map(|p| bytes(p))
            .collect();
        assert_eq!(after, before);
    }

    fn ext_item<'a>(report: &'a DoctorReport, title_part: &str) -> &'a DoctorItem {
        report
            .items
            .iter()
            .find(|i| (i.id.starts_with("mcp.") || i.id.starts_with("skill.")) && i.title.contains(title_part))
            .unwrap_or_else(|| panic!("no item for {title_part}: {:?}", report.items))
    }

    fn cant_edit_copy(file: &Path, kind: &str) -> String {
        format!(
            "Marshell can't edit {} safely, so it leaves the file alone. Turn the {kind} off in Qwen Code.",
            file.display()
        )
    }

    #[tokio::test]
    async fn a_switch_in_a_file_that_does_not_parse_is_report_only() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        let qwen_cfg = ext.join(".qwen").join("settings.json");
        write(&qwen_cfg, "{");
        write(
            &ext.join(".qwen").join("skills").join("nodesc").join("SKILL.md"),
            "---\nname: nodesc\n---\nbody\n",
        );
        write(
            &ext.join(".codex").join("config.toml"),
            "[mcp_servers.ghostly]\ncommand = \"ghost\"\n",
        );

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = last_report(&st);
        let skill = ext_item(&report, "nodesc");
        assert_eq!(skill.group, DoctorGroup::Broken);
        assert!(!skill.fixable, "{skill:?}");
        assert_eq!(skill.cannot_fix, Some(cant_edit_copy(&qwen_cfg, "skill")));
        let codex = ext_item(&report, "ghostly");
        assert!(codex.fixable, "{codex:?}");
        fix_of(&st, vec![codex.id.clone()]).expect("the Codex fix alone");
        let all: Vec<String> = report
            .items
            .iter()
            .filter(|i| i.fixable)
            .map(|i| i.id.clone())
            .collect();
        assert!(all.contains(&codex.id) && !all.contains(&skill.id), "{all:?}");
        fix_of(&st, all).expect("every fixable id");
    }

    #[tokio::test]
    async fn a_list_that_is_not_an_array_makes_its_server_report_only() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        let qwen_cfg = ext.join(".qwen").join("settings.json");
        write(
            &qwen_cfg,
            r#"{"mcp":{"excluded":"x"},"mcpServers":{"ghostly":{"command":"ghost"}}}"#,
        );
        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = last_report(&st);
        let server = ext_item(&report, "ghostly");
        assert_eq!(server.group, DoctorGroup::Broken);
        assert!(!server.fixable, "{server:?}");
        assert_eq!(server.cannot_fix, Some(cant_edit_copy(&qwen_cfg, "server")));
    }

    #[tokio::test]
    async fn report_only_items_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        let project = crate::projects::canonical(dir.path()).join("project");
        // A user-level Claude Code server, a server only the project's shared file can turn off, and a plugin's.
        write(
            &ext.join(".claude.json"),
            r#"{"mcpServers":{"user-ghost":{"command":"ghost"}}}"#,
        );
        write(
            &project.join(".codex").join("config.toml"),
            "[mcp_servers.shared-ghost]\ncommand = \"ghost\"\n",
        );
        let plugin = ext.join("plugin-install");
        write(
            &plugin.join(".mcp.json"),
            r#"{"mcpServers":{"plug-ghost":{"command":"ghost"}}}"#,
        );
        // Claude Code's user settings are the pinned file (`pinned` names `<project>/settings.json`).
        write(&project.join("settings.json"), r#"{"enabledPlugins":{"p@m":true}}"#);
        write(
            &ext.join(".claude").join("plugins").join("installed_plugins.json"),
            &serde_json::json!({"plugins": {"p@m": [{"installPath": plugin.to_string_lossy()}]}}).to_string(),
        );

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = last_report(&st);
        let ghosts: Vec<&DoctorItem> = report.items.iter().filter(|i| i.id.starts_with("mcp.")).collect();
        assert_eq!(ghosts.len(), 3, "{:?}", report.items);
        for item in ghosts {
            assert!(!item.fixable && item.cannot_fix.is_some(), "{item:?}");
            let (status, _) = fix_of(&st, vec![item.id.clone()]).expect_err("refused");
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{item:?}");
        }
    }

    #[tokio::test]
    async fn an_agent_that_failed_to_read_has_no_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let ext = dir.path().join("ext-home");
        let st = state(dir.path()).with_ext_home(ext.clone());
        write(
            &ext.join(".codex").join("config.toml"),
            "[mcp_servers.g]\ncommand = \"ghost\"\n",
        );
        let facts = facts_with(&st, |_, agent| {
            if agent == "codex" {
                Err((StatusCode::INTERNAL_SERVER_ERROR, "the home folder is not known".into()))
            } else {
                Ok(setup(agent))
            }
        });
        let inv = doctor_ext::inventories(&st, &facts, &[]);
        let mut agents: Vec<&str> = inv.iter().map(|i| i.agent).collect();
        agents.sort();
        assert_eq!(agents, ["antigravity", "claude", "copilot", "qwen"]);
    }

    #[tokio::test]
    async fn distro_inventory_has_no_projects_and_no_command_check() {
        use crate::doctor::ext::{CommandState, Origin, Transport};
        use marshell_protocol::api::{Profile, ShellFamily};
        let ubuntu = Profile {
            id: "ubuntu".into(),
            name: "Ubuntu".into(),
            command: vec!["wsl.exe".into(), "-d".into(), "Ubuntu".into()],
            family: ShellFamily::Wsl,
            cwd: None,
            env: Vec::new(),
            shell_integration: false,
            terminal_theme: None,
            detected: true,
        };
        let dir = tempfile::tempdir().unwrap();
        let ubuntu_home = dir.path().join("ubuntu-home");
        let project = crate::projects::canonical(dir.path()).join("project");
        let st = pinned_with(dir.path(), vec![ubuntu])
            .with_ext_home(dir.path().join("ext-home"))
            .with_wsl_version(Some("1.0.0".into()))
            .with_running_distros(|| vec!["Ubuntu".to_string()])
            .with_distro_target(Some(super::super::DistroTarget {
                home: ubuntu_home.clone(),
                bridge: "/mnt/c/marshell-hook.exe".into(),
            }));
        crate::config::managed::save(
            &st.0.home,
            &[ManagedEntry {
                path: r"\\wsl.localhost\Ubuntu\home\u\.claude\settings.json".into(),
                pointer: Some("/hooks/Stop/-".into()),
                changeset_id: "c".into(),
                applied_ms: 1,
            }],
        )
        .unwrap();
        write(
            &ubuntu_home.join(".qwen").join("settings.json"),
            r#"{"mcpServers":{"in-distro":{"command":"ghost"}}}"#,
        );
        write(
            &ubuntu_home.join(".copilot").join("mcp-config.json"),
            r#"{"mcpServers":{"in-distro-copilot":{"command":"ghost"}}}"#,
        );
        // A project of Windows holds a server too; the distro never reads it.
        write(
            &project.join(".mcp.json"),
            r#"{"mcpServers":{"proj":{"command":"ghost"}}}"#,
        );

        let gathered = facts_of(&st);
        let in_distro: Vec<_> = gathered
            .ext
            .iter()
            .filter(|i| i.distro.as_deref() == Some("Ubuntu"))
            .collect();
        assert_eq!(in_distro.len(), 5, "{:?}", gathered.ext);
        let qwen = in_distro.iter().find(|i| i.agent == "qwen").unwrap();
        assert_eq!(qwen.servers.len(), 1);
        assert_eq!(
            qwen.servers[0].transport,
            Transport::Stdio {
                command: "ghost".into(),
                state: CommandState::NotChecked
            }
        );
        // Copilot CLI's distro inventory is the distro's own `~/.copilot`, at user level, with no command checked.
        let copilot = in_distro.iter().find(|i| i.agent == "copilot").unwrap();
        let mut names: Vec<&str> = copilot.servers.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["github-mcp-server", "in-distro-copilot"]);
        let own = copilot.servers.iter().find(|s| s.name == "in-distro-copilot").unwrap();
        assert_eq!(own.origin, Origin::User);
        assert_eq!(own.file, ubuntu_home.join(".copilot").join("mcp-config.json"));
        assert_eq!(
            own.transport,
            Transport::Stdio {
                command: "ghost".into(),
                state: CommandState::NotChecked
            }
        );
        for inv in &in_distro {
            assert!(
                inv.servers
                    .iter()
                    .all(|s| !matches!(s.origin, Origin::Project(_)) && s.file.starts_with(&ubuntu_home)),
                "{inv:?}"
            );
        }
        // On Windows the project's server was read.
        assert!(
            gathered
                .ext
                .iter()
                .any(|i| i.distro.is_none() && i.servers.iter().any(|s| s.name == "proj")),
            "{:?}",
            gathered.ext
        );
    }

    /// The ids of the Broken Copilot server items in the report.
    fn copilot_broken_ids(report: &DoctorReport) -> Vec<String> {
        report
            .items
            .iter()
            .filter(|i| i.group == DoctorGroup::Broken && i.id.starts_with("mcp.copilot."))
            .map(|i| i.id.clone())
            .collect()
    }

    fn apply_plan(plan: &DoctorFixPlan) -> ApplyRequest {
        ApplyRequest {
            changeset_id: plan.preview.changeset_id.clone(),
            expected_hashes: plan
                .preview
                .files
                .iter()
                .map(|f| ExpectedHash {
                    path: f.path.clone(),
                    hash: f.before_hash.clone(),
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn a_broken_copilot_server_is_turned_off_in_a_settings_file_that_keeps_its_other_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        let mcp = ext.join(".copilot").join("mcp-config.json");
        let settings = ext.join(".copilot").join("settings.json");
        write(
            &mcp,
            "// mine\n{\"mcpServers\": {\"ghostly\": {\"command\": \"ghost\"}, \"ghosty\": {\"command\": \"ghost\"}, \"fine\": {\"command\": \"node\"}}}\n",
        );
        let original = "\u{feff}{\r\n  // my settings\r\n  \"theme\": \"dark\", // keep\r\n}\r\n";
        write(&settings, original);

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = last_report(&st);
        let gathered = facts_of(&st);
        let copilot = gathered
            .ext
            .iter()
            .find(|i| i.agent == "copilot" && i.distro.is_none())
            .expect("a Copilot inventory");
        assert_eq!(
            copilot.servers.iter().filter(|s| s.name != "github-mcp-server").count(),
            3
        );

        // Two picked servers are one edit of the list, in the settings file.
        let ids = copilot_broken_ids(&report);
        assert_eq!(ids.len(), 2, "{:?}", report.items);
        assert!(report.items.iter().filter(|i| ids.contains(&i.id)).all(|i| i.fixable));
        let plan = fix_of(&st, ids.clone()).expect("a plan");
        assert!(plan.preview.can_apply, "{:?}", plan.preview);
        assert_eq!(plan.preview.files.len(), 1, "two servers are one edit of the list");
        assert_eq!(Path::new(&plan.preview.files[0].path), settings);
        let applied = fix_apply(State(st.clone()), Json(apply_plan(&plan))).await;
        assert_eq!(applied.status(), StatusCode::OK);
        wait_idle(&st).await;

        let after = std::fs::read_to_string(&settings).unwrap();
        assert!(
            after.starts_with("\u{feff}{\r\n  // my settings\r\n  \"theme\": \"dark\", // keep\r\n"),
            "{after:?}"
        );
        let value = crate::config::jsonc::read_value(&after).unwrap();
        assert_eq!(value["theme"], "dark");
        let mut list: Vec<&str> = value["disabledMcpServers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        list.sort();
        assert_eq!(list, ["ghostly", "ghosty"]);
        // The server files are never written.
        assert!(std::fs::read_to_string(&mcp).unwrap().starts_with("// mine\n"));

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        assert!(copilot_broken_ids(&last_report(&st)).is_empty());

        let undone = undo(State(st.clone())).await;
        assert_eq!(undone.status(), StatusCode::OK);
        wait_idle(&st).await;
        assert_eq!(std::fs::read(&settings).unwrap(), original.as_bytes());
    }

    #[tokio::test]
    async fn a_copilot_settings_file_that_is_missing_is_created_and_undo_removes_it() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        let settings = ext.join(".copilot").join("settings.json");
        write(
            &ext.join(".copilot").join("mcp-config.json"),
            r#"{"mcpServers": {"ghostly": {"command": "ghost"}}}"#,
        );

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let ids = copilot_broken_ids(&last_report(&st));
        assert_eq!(ids.len(), 1);
        let plan = fix_of(&st, ids).expect("a plan");
        let applied = fix_apply(State(st.clone()), Json(apply_plan(&plan))).await;
        assert_eq!(applied.status(), StatusCode::OK);
        wait_idle(&st).await;
        let value = crate::config::jsonc::read_value(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(value["disabledMcpServers"], serde_json::json!(["ghostly"]));

        let undone = undo(State(st.clone())).await;
        assert_eq!(undone.status(), StatusCode::OK);
        wait_idle(&st).await;
        assert!(!settings.exists());
    }

    #[tokio::test]
    async fn a_copilot_list_that_is_not_an_array_makes_its_servers_report_only() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        let settings = ext.join(".copilot").join("settings.json");
        write(
            &ext.join(".copilot").join("mcp-config.json"),
            r#"{"mcpServers": {"ghostly": {"command": "ghost"}}}"#,
        );
        write(&settings, "{\"disabledMcpServers\": \"ghostly\"}");
        let before = std::fs::read(&settings).unwrap();

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = last_report(&st);
        let item = report
            .items
            .iter()
            .find(|i| i.id.starts_with("mcp.copilot."))
            .unwrap_or_else(|| panic!("no item: {:?}", report.items));
        assert!(!item.fixable);
        assert_eq!(
            item.cannot_fix.as_deref(),
            Some(
                format!(
                    "Marshell can't edit {} safely, so it leaves the file alone. Turn the server off in Copilot CLI.",
                    settings.display()
                )
                .as_str()
            )
        );
        assert_eq!(std::fs::read(&settings).unwrap(), before);
    }

    /// The ids of the Broken Copilot skill items in the report.
    fn copilot_skill_ids(report: &DoctorReport) -> Vec<String> {
        report
            .items
            .iter()
            .filter(|i| i.group == DoctorGroup::Broken && i.id.starts_with("skill.copilot."))
            .map(|i| i.id.clone())
            .collect()
    }

    fn copilot_skill(ext: &Path, folder: &str, text: &str) {
        write(&ext.join(".copilot").join("skills").join(folder).join("SKILL.md"), text);
    }

    #[tokio::test]
    async fn a_copilot_skill_over_the_description_limit_is_turned_off_and_one_without_a_description_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        let settings = ext.join(".copilot").join("settings.json");
        copilot_skill(
            &ext,
            "longdesc",
            &format!("---\nname: longdesc\ndescription: {}\n---\nbody\n", "d".repeat(1025)),
        );
        copilot_skill(&ext, "nodesc", "---\nname: nodesc\n---\nA first line.\n");
        copilot_skill(
            &ext,
            "fine",
            &format!("---\nname: fine\ndescription: {}\n---\n", "d".repeat(1024)),
        );

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = last_report(&st);
        let ids = copilot_skill_ids(&report);
        assert_eq!(ids.len(), 1, "{:?}", report.items);
        let item = report.items.iter().find(|i| i.id == ids[0]).unwrap();
        assert_eq!(item.title, "Copilot CLI's skill longdesc doesn't load");
        assert_eq!(
            item.detail,
            "Its description is over 1,024 characters. The fix turns it off and doesn't delete it."
        );
        assert!(item.fixable, "{item:?}");

        let plan = fix_of(&st, ids).expect("a plan");
        assert!(plan.preview.can_apply, "{:?}", plan.preview);
        assert_eq!(plan.preview.files.len(), 1);
        assert_eq!(Path::new(&plan.preview.files[0].path), settings);
        let applied = fix_apply(State(st.clone()), Json(apply_plan(&plan))).await;
        assert_eq!(applied.status(), StatusCode::OK);
        wait_idle(&st).await;
        let value = crate::config::jsonc::read_value(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(value["disabledSkills"], serde_json::json!(["longdesc"]));
        // The skill's own file is never written.
        assert!(
            std::fs::read_to_string(ext.join(".copilot").join("skills").join("longdesc").join("SKILL.md"))
                .unwrap()
                .starts_with("---\nname: longdesc\n")
        );

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        assert!(copilot_skill_ids(&last_report(&st)).is_empty());

        let undone = undo(State(st.clone())).await;
        assert_eq!(undone.status(), StatusCode::OK);
        wait_idle(&st).await;
        assert!(!settings.exists());
    }

    #[tokio::test]
    async fn a_picked_copilot_server_and_a_picked_skill_are_one_edit_per_list_in_one_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        let settings = ext.join(".copilot").join("settings.json");
        write(
            &ext.join(".copilot").join("mcp-config.json"),
            r#"{"mcpServers": {"ghostly": {"command": "ghost"}}}"#,
        );
        copilot_skill(&ext, "bad", "---\nname: bad\ndescription: d\n---\nbody\n");
        copilot_skill(&ext, "unclosed", "---\nname: unclosed\ndescription: d\n");
        let original = "\u{feff}{\r\n  // mine\r\n  \"theme\": \"dark\",\r\n  \"disabledSkills\": [\"old\"],\r\n}\r\n";
        write(&settings, original);

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = last_report(&st);
        let mut ids = copilot_broken_ids(&report);
        let skills = copilot_skill_ids(&report);
        assert_eq!((ids.len(), skills.len()), (1, 1), "{:?}", report.items);
        ids.extend(skills);
        let plan = fix_of(&st, ids).expect("a plan");
        assert!(plan.preview.can_apply, "{:?}", plan.preview);
        assert_eq!(plan.preview.files.len(), 1, "one settings file");
        assert_eq!(Path::new(&plan.preview.files[0].path), settings);
        let applied = fix_apply(State(st.clone()), Json(apply_plan(&plan))).await;
        assert_eq!(applied.status(), StatusCode::OK);
        wait_idle(&st).await;

        let after = std::fs::read_to_string(&settings).unwrap();
        assert!(
            after.starts_with("\u{feff}{\r\n  // mine\r\n  \"theme\": \"dark\",\r\n"),
            "{after:?}"
        );
        let value = crate::config::jsonc::read_value(&after).unwrap();
        assert_eq!(value["disabledMcpServers"], serde_json::json!(["ghostly"]));
        assert_eq!(value["disabledSkills"], serde_json::json!(["old", "unclosed"]));

        let undone = undo(State(st.clone())).await;
        assert_eq!(undone.status(), StatusCode::OK);
        wait_idle(&st).await;
        assert_eq!(std::fs::read(&settings).unwrap(), original.as_bytes());
    }

    #[tokio::test]
    async fn a_copilot_skill_list_that_is_not_an_array_makes_its_broken_skills_report_only() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        let settings = ext.join(".copilot").join("settings.json");
        copilot_skill(&ext, "unclosed", "---\nname: unclosed\ndescription: d\n");
        write(&settings, "{\"disabledSkills\": \"unclosed\"}");
        let before = std::fs::read(&settings).unwrap();

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = last_report(&st);
        let ids = copilot_skill_ids(&report);
        assert_eq!(ids.len(), 1, "{:?}", report.items);
        let item = report.items.iter().find(|i| i.id == ids[0]).unwrap();
        assert!(!item.fixable);
        assert_eq!(
            item.cannot_fix.as_deref(),
            Some(
                format!(
                    "Marshell can't edit {} safely, so it leaves the file alone. Turn the skill off in Copilot CLI.",
                    settings.display()
                )
                .as_str()
            )
        );
        assert_eq!(std::fs::read(&settings).unwrap(), before);
    }

    #[tokio::test]
    async fn skills_past_copilots_listing_budget_are_one_report_only_item_with_a_count() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        for i in 0..200 {
            copilot_skill(
                &ext,
                &format!("skill{i:03}"),
                &format!("---\nname: skill{i:03}\ndescription: {}\n---\n", "d".repeat(15)),
            );
        }

        run_once(&st, Arc::new(facts_of)).await.expect("a run");
        let report = last_report(&st);
        let item = report
            .items
            .iter()
            .find(|i| i.id == "skilllisting.copilot")
            .unwrap_or_else(|| panic!("no listing item: {:?}", report.items));
        assert_eq!(item.group, DoctorGroup::Waste);
        assert_eq!(item.title, "Copilot CLI lists 69 skills by name only");
        assert!(!item.fixable);
        assert_eq!(
            item.cannot_fix.as_deref(),
            Some("Shorten the longest descriptions, or turn off skills you don't use.")
        );
        assert!(copilot_skill_ids(&report).is_empty());
    }
}
