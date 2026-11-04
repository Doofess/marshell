//! The Doctor's runs: `POST /v1/doctor/run` starts the checks (or joins the run that is going) and answers 202 with
//! the summary; `GET /v1/doctor/report` answers the last report, or 204 before the first run ends. The core also
//! runs once at startup (see `AppState::with_doctor_on_start`). A run reads the files of every CLI off the async
//! threads, builds the report with `doctor::checks` and publishes the summary on the events socket.

use super::AppState;
use super::agents::{Adapter, agy_setup_of, codex_setup_of, qwen_setup_of, settings_file, setup_of};
use crate::agents::common::Gate;
use crate::config::guard::unverbatim;
use crate::config::jsonc::{parse_problem, read_value};
use crate::config::managed::{self, ManagedEntry};
use crate::doctor::{AgentFacts, FileState, checks};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::agents::{AgentSetup, SettingsState};
use marshell_protocol::doctor::{DoctorReport, DoctorSummary};
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
