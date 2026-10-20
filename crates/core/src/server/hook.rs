use super::AppState;
use crate::agents::claude::status::{fold_event, fold_statusline};
use crate::agents::claude::{Gate, version_gate};
use crate::hooks::holds::Outcome;
use crate::hooks::log::HookEventRecord;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use marshell_protocol::hook::{HEADER_AGENT, HEADER_ROLE, HEADER_TAB, HEADER_VERSION, HookRole};
use serde::Deserialize;

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
}

/// What the bridge posts. The body is the agent's hook JSON.
pub async fn post(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, (StatusCode, String)> {
    let role = header(&headers, HEADER_ROLE)
        .and_then(HookRole::parse)
        .ok_or((StatusCode::BAD_REQUEST, "unknown or missing role".to_string()))?;
    let agent = header(&headers, HEADER_AGENT)
        .ok_or((StatusCode::BAD_REQUEST, "missing agent".to_string()))?
        .to_string();
    let json: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| (StatusCode::BAD_REQUEST, format!("body is not JSON: {e}")))?;
    if role == HookRole::Context {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    let tab = header(&headers, HEADER_TAB).map(str::to_string);
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
    if agent == "claude"
        && let Some(tab) = tab.as_deref()
    {
        fold_live(&st, role, tab, &json, at_ms).await;
    }
    if role == HookRole::Permission {
        // Without a card or an open tab, the agent must ask the person itself.
        if !crate::agents::supports_permissions(&agent) {
            return Ok(StatusCode::NO_CONTENT.into_response());
        }
        let Some(tab) = tab.filter(|t| st.registry().is_open(t)) else {
            return Ok(StatusCode::NO_CONTENT.into_response());
        };
        return Ok(hold_permission(&st, Some(tab), agent, json).await);
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Folds what Claude Code reported into the tab's live status. A permission request counts as an event
/// (it folds to Needs permission). Without a known version, or with the switch off, the row gets a
/// note saying why and keeps no status.
async fn fold_live(st: &AppState, role: HookRole, tab: &str, body: &serde_json::Value, at_ms: u64) {
    if !matches!(role, HookRole::Event | HookRole::Permission | HookRole::Statusline) || !st.registry().is_open(tab) {
        return;
    }
    let off = st.0.settings.lock().unwrap().status_off.iter().any(|a| a == "claude");
    if off {
        st.registry()
            .set_status_note(tab, Some("Live status is off for Claude Code".to_string()));
        return;
    }
    // A cold cache runs `claude --version`: keep that off the async workers.
    let asker = st.clone();
    let version = tokio::task::spawn_blocking(move || asker.claude_version(asker.claude_exe().as_deref()))
        .await
        .unwrap_or(None);
    match version_gate(version.as_deref()) {
        Gate::Unknown(note) => st.registry().set_status_note(tab, Some(note)),
        Gate::Known => {
            if role == HookRole::Statusline {
                st.registry()
                    .update_live(tab, |prev| Some(fold_statusline(prev, body, at_ms)));
            } else {
                st.registry().update_live(tab, |prev| fold_event(prev, body, at_ms));
            }
            st.registry().set_status_note(tab, None);
        }
    }
}

/// Holds the bridge's request until a decision, a release, the hold running out, or the bridge
/// leaving. 200 with the agent's JSON for a decision, else 204 so the agent asks the person itself.
///
/// This future owns the `Hold`: when the bridge disconnects axum drops the future, and the guard
/// takes the request out of the pending list.
async fn hold_permission(st: &AppState, tab: Option<String>, agent: String, request: serde_json::Value) -> Response {
    let (_hold, mut rx) = st.holds().open(tab, agent.clone(), request.clone());
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
        return StatusCode::NO_CONTENT.into_response();
    };
    match crate::agents::permission_output(&agent, &decision, &request) {
        Ok(output) => Json(output).into_response(),
        // The decision route checked this already; if it still fails, let the agent ask.
        Err(_) => StatusCode::NO_CONTENT.into_response(),
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
