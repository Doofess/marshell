use super::AppState;
use crate::hooks::holds::PendingPermission;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use marshell_protocol::hook::PermissionDecision;

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
    crate::agents::permission_output(&pending.agent, &decision, &pending.request)
        .map_err(|text| (StatusCode::CONFLICT, text.to_string()))?;
    if st.holds().decide(&rid, decision) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found())
    }
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
