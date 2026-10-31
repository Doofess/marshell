use super::AppState;
use crate::agents::claude::compact;
use crate::agents::claude::status::{fold_event, fold_statusline_with_window, mode_of};
use crate::agents::common::Gate;
use crate::agents::{antigravity, codex};
use crate::history::recorder::schedule_copy;
use crate::hooks::holds::Outcome;
use crate::hooks::log::HookEventRecord;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{LiveState, SessionMode};
use marshell_protocol::hook::{HEADER_AGENT, HEADER_ROLE, HEADER_TAB, HEADER_VERSION, HookRole};
use serde::Deserialize;

/// How long after the request arrived a note may still be answered. The bridge gives an event 400 ms for the whole
/// request (`budget` in `crates/hook/src/post.rs`), then prints nothing and lets the prompt go on as it is, so a
/// note that comes later reaches no agent. 350 ms leaves the rest for the connect and the response.
pub const CONTEXT_BUDGET: std::time::Duration = std::time::Duration::from_millis(350);

/// The note when it is ready in time to reach the agent, else None. The header chip shows what this returns, so it
/// never claims a note the bridge had already given up on.
fn note_in_budget(
    found: Option<marshell_protocol::api::ReferenceNote>,
    elapsed: std::time::Duration,
) -> Option<marshell_protocol::api::ReferenceNote> {
    found.filter(|_| elapsed <= CONTEXT_BUDGET)
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
}

fn is_identifier(value: &str) -> bool {
    value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// What the bridge posts. The body is the agent's hook JSON.
pub async fn post(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, (StatusCode, String)> {
    // The bridge's clock for this request started about now, so the note's budget is counted from here.
    let started = std::time::Instant::now();
    let role = header(&headers, HEADER_ROLE)
        .and_then(HookRole::parse)
        .ok_or((StatusCode::BAD_REQUEST, "unknown or missing role".to_string()))?;
    let agent = header(&headers, HEADER_AGENT)
        .ok_or((StatusCode::BAD_REQUEST, "missing agent".to_string()))?
        .to_string();
    let mut json: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| (StatusCode::BAD_REQUEST, format!("body is not JSON: {e}")))?;
    // A prompt or an email is personal: it goes before anything else sees the body (the log, the fold,
    // the history, a hold), so the core keeps neither.
    // The prompt of a turn is kept in this local for the references resolver alone. It is taken out of the body
    // here, so the log, the fold, the history and a hold never see it.
    let mut prompt = None;
    if agent == "antigravity" {
        prompt = json
            .get("lastUserInput")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        antigravity::strip_personal(&mut json);
    } else if matches!(agent.as_str(), "claude" | "codex")
        && let Some(object) = json.as_object_mut()
    {
        prompt = match object.remove("prompt") {
            Some(serde_json::Value::String(text)) => Some(text),
            _ => None,
        };
    }
    let tab = header(&headers, HEADER_TAB).map(str::to_string);
    // Both come from the sender, so only a short identifier is logged: never free text a payload could carry.
    tracing::debug!(
        event_name = json
            .get("hook_event_name")
            .and_then(serde_json::Value::as_str)
            .filter(|v| is_identifier(v)),
        tab_id = tab.as_deref().filter(|v| is_identifier(v)),
        "hook event received"
    );
    if role == HookRole::Context {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    let at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let rec = HookEventRecord {
        at_ms,
        role,
        agent: agent.clone(),
        tab: tab.clone(),
        bridge_version: header(&headers, HEADER_VERSION).unwrap_or("").to_string(),
        body: json.clone(),
    };
    st.hook_log().push(rec, |tab| st.session(tab).is_some());
    // A WSL tab's processes are Linux ones the foreground watcher can't read: its hooks say which agent runs there. The
    // mark comes first, so the check below sees it and an event of another agent in that tab is dropped.
    let mut left_wsl = false;
    if matches!(agent.as_str(), "claude" | "codex" | "antigravity")
        && let Some(tab) = tab.as_deref()
        && in_wsl(&st, tab).await
    {
        if json.get("hook_event_name").and_then(serde_json::Value::as_str) == Some("SessionEnd") {
            // The agent is gone from the row: nothing is left to fold into a status, even when no earlier hook marked it.
            st.registry().clear_hook_agent(tab, &agent);
            left_wsl = true;
        } else if matches!(role, HookRole::Event | HookRole::Permission | HookRole::Statusline) {
            st.registry().mark_hook_agent(tab, &agent);
        }
    }
    // Grok Build runs Claude Code's hooks too. A tab that runs another agent takes no event or hold
    // from this one, so its own prompt applies.
    if let Some(tab) = tab.as_deref()
        && st.registry().effective_agent(tab).is_some_and(|a| a != agent)
    {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    // Whether an Antigravity request waits for a card. Every other call passes at once (204): agy's own mode
    // decides, and it asks in its terminal where it would.
    let mut held = false;
    if agent == "antigravity" && role == HookRole::Permission {
        // An Antigravity IDE started from a plain shell tab inherits the tab's id and reads the same
        // hooks.json: only a tab that runs agy has a say, and anything else passes before it folds anything.
        let Some(tab) = tab
            .as_deref()
            .filter(|t| st.registry().is_open(t) && st.registry().effective_agent(t).as_deref() == Some("antigravity"))
        else {
            return Ok(StatusCode::NO_CONTENT.into_response());
        };
        // agy's `workspacePaths` is empty, so the risk rules judge paths against the tab's folder.
        if json.get("cwd").is_none()
            && let Some(cwd) = st.registry().cwd_of_tab(tab)
            && let Some(object) = json.as_object_mut()
        {
            object.insert("cwd".into(), cwd.into());
        }
        let mode = st.registry().mode_of_open_tab(tab).unwrap_or(SessionMode::Manual);
        held = antigravity::policy::holds(mode, antigravity::request::risk_of(&json).is_some());
    }
    // A prompt that names an alias gets the note as context. Every prompt of a tab that Marshell opened sets the
    // header chip, so a prompt that names nothing clears it. Antigravity has no prompt event: each model call of a
    // prompt is a `PreInvocation`, and the note a call adds stays in the model's context for the rest of the prompt,
    // so only the first call (`invocationNum` 0) is answered and the later ones leave the note and the chip alone.
    let event = json.get("hook_event_name").and_then(serde_json::Value::as_str);
    let starts_prompt = match agent.as_str() {
        "claude" | "codex" => event == Some("UserPromptSubmit"),
        "antigravity" => {
            event == Some("PreInvocation") && json.get("invocationNum").and_then(serde_json::Value::as_u64) == Some(0)
        }
        _ => false,
    };
    let note_tab = tab.as_deref().filter(|tab| {
        role == HookRole::Event
            && starts_prompt
            && st.registry().is_open(tab)
            && st.registry().effective_agent(tab).as_deref() == Some(agent.as_str())
    });
    // The lookup runs beside the fold and the history (the history's index lock can be held by a search) under what
    // is left of the budget, so a slow fold can't make the answer later than the bridge waits.
    let fold = async {
        if matches!(agent.as_str(), "claude" | "codex" | "antigravity")
            && let Some(tab) = tab.as_deref()
        {
            if !left_wsl {
                fold_live_with(&st, &agent, role, tab, &json, at_ms, held).await;
            }
            if role == HookRole::Event && st.registry().is_open(tab) {
                record_history(&st, &agent, tab, &json, at_ms).await;
            }
        }
    };
    let lookup = async {
        let tab = note_tab?;
        let left = CONTEXT_BUDGET.saturating_sub(started.elapsed());
        tokio::time::timeout(left, references_for(&st, tab, prompt))
            .await
            .unwrap_or(None)
    };
    let ((), found) = tokio::join!(fold, lookup);
    if let Some(tab) = note_tab {
        let found = note_in_budget(found, started.elapsed());
        st.registry().set_references(tab, found.clone());
        if let Some(found) = found {
            let output = match agent.as_str() {
                "codex" => codex::context_output(&found.note),
                "antigravity" => antigravity::context_output(&found.note),
                _ => crate::agents::claude::context_output(&found.note),
            };
            return Ok(Json(output).into_response());
        }
    }
    if role == HookRole::Permission {
        // Without a card or an open tab, the agent must ask the person itself.
        if !crate::agents::supports_permissions(&agent) || (agent == "antigravity" && !held) {
            return Ok(StatusCode::NO_CONTENT.into_response());
        }
        let Some(tab) = tab.filter(|t| st.registry().is_open(t)) else {
            return Ok(StatusCode::NO_CONTENT.into_response());
        };
        return Ok(hold_permission(&st, Some(tab), agent, json).await);
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Whether the tab's shell runs in a WSL distro. The first answer for a profile without `-d` asks `wsl.exe` for the default
/// distro, so it runs off the async workers.
async fn in_wsl(st: &AppState, tab: &str) -> bool {
    let (asker, tab) = (st.clone(), tab.to_string());
    tokio::task::spawn_blocking(move || asker.wsl_distro_of_tab(&tab).is_some())
        .await
        .unwrap_or(false)
}

/// The note for the aliases `prompt` names in the project of the folder the agent runs in, with paths in that
/// folder's own checkout. That folder is the one the tab's shell last reported (a shell opened in repo A where the
/// person ran `cd ..\B; claude` is in B), and the tab's start folder when the shell reported none.
/// None when there is no prompt, the references are off, the file can't be read, or nothing matches: the prompt
/// then goes on as it is. The read is one small file, off the async workers.
async fn references_for(
    st: &AppState,
    tab: &str,
    prompt: Option<String>,
) -> Option<marshell_protocol::api::ReferenceNote> {
    let prompt = prompt?;
    let reported = st
        .session(tab)
        .and_then(|s| s.current_cwd())
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|cwd| !cwd.is_empty());
    let cwd = reported.or_else(|| st.registry().cwd_of_tab(tab).filter(|cwd| !cwd.is_empty()))?;
    let (home, asker, tab) = (st.home().to_path_buf(), st.clone(), tab.to_string());
    tokio::task::spawn_blocking(move || {
        // A WSL tab's folder is a Linux path (or the start folder, a Windows one): the files are read as Windows
        // paths, and the note spells them the way the distro reads them.
        let distro = asker.wsl_distro_of_tab(&tab);
        let cwd = match &distro {
            Some(distro) if cwd.starts_with('/') => {
                crate::wsl::to_windows(distro, &cwd)?.to_string_lossy().into_owned()
            }
            Some(_) if !std::path::Path::new(&cwd).is_absolute() => return None,
            _ => cwd,
        };
        let root = std::path::PathBuf::from(crate::projects::project_of(&cwd));
        let checkout =
            crate::scripts::checkout_root(std::path::Path::new(&cwd)).unwrap_or_else(|| std::path::PathBuf::from(&cwd));
        let doc = crate::references::load(&home, &root).ok()?;
        match distro {
            Some(distro) => crate::references::resolve(&doc, &checkout, &prompt, &|p| crate::wsl::to_linux(&distro, p)),
            None => crate::references::resolve(&doc, &checkout, &prompt, &|p| Some(p.display().to_string())),
        }
    })
    .await
    .unwrap_or(None)
}

/// Notes the event in the history. The index update is short, so it is awaited; the copy it may ask for
/// runs on its own and the response does not wait for it.
async fn record_history(st: &AppState, agent: &str, tab: &str, body: &serde_json::Value, at_ms: u64) {
    let history = st.history();
    let recording = st
        .session(tab)
        .and_then(|s| s.recording_path())
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()));
    let (agent, tab, body) = (agent.to_string(), tab.to_string(), body.clone());
    let noter = history.clone();
    let meta = tokio::task::spawn_blocking(move || noter.note_event(&agent, &tab, recording.as_deref(), &body, at_ms))
        .await
        .unwrap_or(None);
    if let Some(meta) = meta {
        schedule_copy(history, meta);
    }
}

/// Folds what an agent (Claude Code, Codex or Antigravity) reported into the tab's live status. A permission
/// request counts as an event (it folds to Needs permission). Without a known version, or with the switch off,
/// the row gets a note saying why and keeps no status. Claude Code and Antigravity send a status line.
#[cfg(test)]
async fn fold_live(st: &AppState, agent: &str, role: HookRole, tab: &str, body: &serde_json::Value, at_ms: u64) {
    fold_live_with(st, agent, role, tab, body, at_ms, false).await;
}

/// `fold_live`, where `held` says an Antigravity permission request waits for a card (it folds to Needs permission).
async fn fold_live_with(
    st: &AppState,
    agent: &str,
    role: HookRole,
    tab: &str,
    body: &serde_json::Value,
    at_ms: u64,
    held: bool,
) {
    if !matches!(role, HookRole::Event | HookRole::Permission | HookRole::Statusline)
        || (role == HookRole::Statusline && !matches!(agent, "claude" | "antigravity"))
        || !st.registry().is_open(tab)
    {
        return;
    }
    let (name, gate): (&str, fn(Option<&str>) -> Gate) = match agent {
        "claude" => ("Claude Code", crate::agents::claude::version_gate),
        "codex" => ("Codex", codex::version_gate),
        "antigravity" => ("Antigravity", antigravity::version_gate),
        _ => return,
    };
    let off = st.0.settings.lock().unwrap().status_off.iter().any(|a| a == agent);
    if off {
        st.registry()
            .set_status_note(tab, Some(format!("Live status is off for {name}")));
        return;
    }
    // A cold cache runs `<agent> --version` (inside the distro for a WSL tab, whose agent is not the Windows one): keep
    // that, and the distro lookup, off the async workers.
    let (asker, tab_id) = (st.clone(), tab.to_string());
    let which = agent.to_string();
    let (version, distro) = tokio::task::spawn_blocking(move || match asker.wsl_distro_of_tab(&tab_id) {
        Some(distro) => (asker.agent_version_in(&distro, &which), Some(distro)),
        None => (asker.agent_version(&which, asker.agent_exe_of(&which).as_deref()), None),
    })
    .await
    .unwrap_or((None, None));
    match gate(version.as_deref()) {
        Gate::Unknown(_) if version.is_none() && distro.is_some() => st.registry().set_status_note(
            tab,
            Some(format!("Can't read {name}'s version in {}", distro.unwrap_or_default())),
        ),
        Gate::Unknown(note) => st.registry().set_status_note(tab, Some(note)),
        Gate::Known if agent == "codex" => {
            // No Done hold, subagent count or stuck check: those follow Claude Code's events.
            st.registry()
                .update_live(tab, |prev| codex::status::fold_event(prev, body, at_ms));
            if let Some(mode) = codex::status::mode_of(body) {
                st.registry().set_live_mode(tab, mode);
            }
            st.registry().set_status_note(tab, None);
        }
        Gate::Known if agent == "antigravity" => {
            if role == HookRole::Statusline {
                // The status line is folded first: a change to the status takes attention, so a prompt
                // that is still showing is raised again after it.
                st.registry().update_live(tab, |prev| {
                    Some(antigravity::status::fold_statusline(prev, body, at_ms))
                });
                if let Some(pending) = antigravity::status::confirmation_pending(body) {
                    st.registry().agent_prompt(tab, pending);
                }
            } else {
                // Any hook event means agy went on: its own prompt, if one was showing, is answered.
                st.registry().agent_prompt(tab, false);
                st.registry()
                    .update_live(tab, |prev| antigravity::status::fold_event(prev, body, at_ms, held));
            }
            st.registry().set_status_note(tab, None);
        }
        Gate::Known => {
            if role == HookRole::Statusline {
                // Read settings off the async workers and before taking the registry lock.
                let paths =
                    st.0.claude_compact_paths
                        .clone()
                        .or_else(|| super::files::home_dir().map(|home| (home, compact::managed_settings_paths())));
                let settings_body = body.clone();
                let window = tokio::task::spawn_blocking(move || {
                    let (home, managed) = paths?;
                    let project = settings_body
                        .pointer("/workspace/project_dir")
                        .and_then(serde_json::Value::as_str)
                        .or_else(|| settings_body.get("cwd").and_then(serde_json::Value::as_str))
                        .filter(|path| !super::files::refused(path))
                        .map(std::path::Path::new)
                        .filter(|path| path.is_absolute());
                    let files = compact::read_settings(&home, project, &managed);
                    let model = settings_body.pointer("/model/id").and_then(serde_json::Value::as_str);
                    compact::compact_window(model, &files)
                })
                .await
                .unwrap_or(None);
                st.registry()
                    .update_live(tab, |prev| Some(fold_statusline_with_window(prev, body, at_ms, window)));
            } else {
                // A Stop that would read Done is held for `done_settle`: a turn that resumes within it
                // (the next event cancels the hold) never shows Done.
                let mut done = None;
                st.registry()
                    .update_live(tab, |prev| match fold_event(prev, body, at_ms) {
                        Some(next) if is_stop(body) && next.state == LiveState::Done => {
                            done = Some(next);
                            None
                        }
                        other => other,
                    });
                st.registry()
                    .stuck_event(tab, crate::agents::claude::stuck::stuck_signal(body), at_ms);
                st.registry().update_subagents(tab, body);
                // Cancels a hold from an earlier event, so it runs before this Stop's own.
                st.registry().follow_subagent_wait(tab, body);
                if let Some(done) = done {
                    if st.registry().hold_done(tab, done.clone()) {
                        let st = st.clone();
                        let tab = tab.to_string();
                        tokio::spawn(async move {
                            tokio::time::sleep(st.done_settle()).await;
                            st.registry().settle_done(&tab);
                        });
                    } else {
                        // Not a row that holds (another agent's): show it at once, as before.
                        st.registry().update_live(tab, |_| Some(done));
                    }
                }
                if st.registry().awaits_subagents_end(tab) {
                    let st = st.clone();
                    let tab = tab.to_string();
                    tokio::spawn(async move {
                        tokio::time::sleep(st.subagent_wait()).await;
                        st.registry().settle_subagent_wait(&tab);
                    });
                }
                if let Some(mode) = mode_of(body) {
                    st.registry().set_live_mode(tab, mode);
                }
            }
            st.registry().set_status_note(tab, None);
        }
    }
}

/// Whether the body is the main turn's Stop.
fn is_stop(body: &serde_json::Value) -> bool {
    body.get("hook_event_name").and_then(serde_json::Value::as_str) == Some("Stop")
}

/// Holds the bridge's request until a decision, a release, the hold running out, or the bridge
/// leaving. 200 with the agent's JSON for a decision; for a release or the hold running out, 200 with the agent's
/// release answer (Antigravity's deny) or else 204, so the agent asks the person itself.
///
/// This future owns the `Hold`: when the bridge disconnects axum drops the future, and the guard
/// takes the request out of the pending list.
async fn hold_permission(st: &AppState, tab: Option<String>, agent: String, request: serde_json::Value) -> Response {
    let (_hold, mut rx) = st
        .holds()
        .open_for(tab, agent.clone(), request.clone(), st.permission_hold());
    let outcome = match tokio::time::timeout(st.permission_hold(), &mut rx).await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(_)) => Outcome::Released,
        Err(_) => {
            // Close first so nothing can settle it now; a decision that got in just before is kept.
            rx.close();
            rx.try_recv().unwrap_or(Outcome::Released)
        }
    };
    let Outcome::Decided(decision) = outcome else {
        return unanswered(&agent, &request);
    };
    match crate::agents::permission_output(&agent, &decision, &request) {
        Ok(output) => Json(output).into_response(),
        // The decision route checked this already; if it still fails, the agent gets what a release gets.
        Err(_) => unanswered(&agent, &request),
    }
}

/// The answer for a hold that ended with no decision: the agent's own release answer (Antigravity's deny, since
/// under its Bypass flag an empty answer runs the call), else 204 so the agent asks the person itself.
fn unanswered(agent: &str, request: &serde_json::Value) -> Response {
    match crate::agents::release_output(agent, request) {
        Some(output) => Json(output).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

#[derive(Deserialize)]
pub struct EventsQuery {
    tab: Option<String>,
}

/// The kept events, oldest first: a tab's, or the untabbed ones without `?tab=`.
pub async fn events(State(st): State<AppState>, Query(q): Query<EventsQuery>) -> Json<Vec<HookEventRecord>> {
    Json(st.hook_log().recent(q.tab.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::{Stuck, StuckReason};
    use serde_json::json;
    use std::time::Duration;

    fn note() -> marshell_protocol::api::ReferenceNote {
        marshell_protocol::api::ReferenceNote {
            names: vec!["login button".into()],
            note: "References: 'login button' = /x/login.rs.".into(),
        }
    }

    #[test]
    fn a_note_inside_the_budget_is_kept_and_a_late_one_is_dropped() {
        assert_eq!(note_in_budget(Some(note()), Duration::from_millis(10)), Some(note()));
        assert_eq!(note_in_budget(Some(note()), CONTEXT_BUDGET), Some(note()));
        // The bridge has given up by then and printed nothing: the chip must not claim a note the agent never got.
        assert_eq!(
            note_in_budget(Some(note()), CONTEXT_BUDGET + Duration::from_millis(1)),
            None
        );
        assert_eq!(note_in_budget(Some(note()), Duration::from_millis(500)), None);
        assert_eq!(note_in_budget(None, Duration::from_millis(1)), None);
    }

    #[tokio::test]
    async fn stuck_statusline_neither_resets_repeats_nor_counts_as_progress() {
        let home = tempfile::tempdir().unwrap();
        let st = AppState::new("test-token".into(), 5555, vec![], home.path().into(), vec![])
            .with_claude_version(Some("2.1.302".into()))
            .with_claude_compact_paths(home.path().join("user"), vec![]);
        st.registry().reserve(
            "a",
            crate::registry::Relaunch {
                profile_id: "test".into(),
                agent: Some("claude".into()),
                cwd: "C:/work".into(),
                mode: None,
                name: "A".into(),
                preset: None,
            },
        );
        let call = json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "pwd"}});
        for ms in 1..=9 {
            fold_live(&st, "claude", HookRole::Event, "a", &call, ms * 10).await;
            fold_live(
                &st,
                "claude",
                HookRole::Event,
                "a",
                &json!({"hook_event_name": "PostToolUse"}),
                ms * 10 + 1,
            )
            .await;
            fold_live(
                &st,
                "claude",
                HookRole::Event,
                "a",
                &json!({"hook_event_name": "Notification", "notification_type": "other"}),
                ms * 10 + 2,
            )
            .await;
            fold_live(
                &st,
                "claude",
                HookRole::Statusline,
                "a",
                &json!({"model": {"id": "test"}}),
                ms * 10 + 3,
            )
            .await;
        }
        assert_eq!(
            st.registry().snapshot().sessions[0].stuck,
            Some(Stuck {
                reason: StuckReason::Repeat,
                since_ms: 90
            })
        );
        fold_live(
            &st,
            "claude",
            HookRole::Event,
            "a",
            &json!({"hook_event_name": "UserPromptSubmit"}),
            100,
        )
        .await;
        fold_live(&st, "claude", HookRole::Statusline, "a", &json!({}), 600_099).await;
        st.registry().check_stuck(600_100, 600_000);
        assert_eq!(
            st.registry().snapshot().sessions[0].stuck,
            Some(Stuck {
                reason: StuckReason::Quiet,
                since_ms: 600_100
            })
        );
    }
}
