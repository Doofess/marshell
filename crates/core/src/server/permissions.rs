use super::AppState;
use crate::hooks::holds::PendingPermission;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use marshell_protocol::api::LiveState;
use marshell_protocol::hook::{PermissionDecision, PermissionScope};

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The requests being held, oldest first.
pub async fn list(State(st): State<AppState>) -> Json<Vec<PendingPermission>> {
    Json(st.holds().pending())
}

/// Answers the held hook with a decision. The agent's output is built first: when it can't be
/// (a scope with no rule to add), the answer is 409 and the request stays pending.
pub async fn decision(
    State(st): State<AppState>,
    Path(rid): Path<String>,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let decision: PermissionDecision =
        serde_json::from_slice(&body).map_err(|e| (StatusCode::BAD_REQUEST, format!("not a decision: {e}")))?;
    let pending = st.holds().get(&rid).ok_or(not_found())?;
    // An administrator tab is answered only from Marshell's own window (`Trusted::decide`).
    if pending.tab.as_deref().is_some_and(|tab| st.tab_is_elevated(tab)) {
        return Err((StatusCode::CONFLICT, crate::pty::elevated::API_WRITE_COPY.to_string()));
    }
    decide(&st, &rid, decision)?;
    Ok(StatusCode::NO_CONTENT)
}

/// The body of an answer, shared by the route and the window's trusted door: refuses what can't be answered,
/// then settles the request and updates the row.
pub(super) fn decide(st: &AppState, rid: &str, decision: PermissionDecision) -> Result<(), (StatusCode, String)> {
    let pending = st.holds().get(rid).ok_or(not_found())?;
    // Marshell can't keep a rule for an administrator session.
    if decision.scope == PermissionScope::Project && pending.tab.as_deref().is_some_and(|tab| st.tab_is_elevated(tab)) {
        return Err((StatusCode::CONFLICT, crate::pty::elevated::ALWAYS_COPY.to_string()));
    }
    crate::agents::permission_output(&pending.agent, &decision, &pending.request)
        .map_err(|text| (StatusCode::CONFLICT, text.to_string()))?;
    let answered = st.holds().decide_and_update(rid, decision, |pending, waiting| {
        // Claude carries on after either answer, so the row stops asking at once.
        if let Some(tab) = pending.tab.as_deref()
            && !waiting
        {
            st.registry().update_live(tab, |prev| {
                let prev = prev?;
                if prev.state == LiveState::Working && prev.phrase == "Thinking" {
                    return None;
                }
                let mut next = prev.clone();
                if prev.state != LiveState::Working {
                    next.since_ms = now_ms();
                }
                next.state = LiveState::Working;
                next.phrase = "Thinking".to_string();
                Some(next)
            });
        }
    });
    if answered { Ok(()) } else { Err(not_found()) }
}

/// Lets the agent ask the person itself.
pub async fn release(State(st): State<AppState>, Path(rid): Path<String>) -> Result<StatusCode, (StatusCode, String)> {
    if st.holds().release(&rid) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found())
    }
}

fn not_found() -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, "no such request".to_string())
}
