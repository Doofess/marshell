use super::AppState;
use crate::hooks::log::HookEventRecord;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
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
) -> Result<StatusCode, (StatusCode, String)> {
    let role = header(&headers, HEADER_ROLE)
        .and_then(HookRole::parse)
        .ok_or((StatusCode::BAD_REQUEST, "unknown or missing role".to_string()))?;
    let agent = header(&headers, HEADER_AGENT)
        .ok_or((StatusCode::BAD_REQUEST, "missing agent".to_string()))?
        .to_string();
    let json: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| (StatusCode::BAD_REQUEST, format!("body is not JSON: {e}")))?;
    match role {
        HookRole::Permission => Err((StatusCode::BAD_REQUEST, "not yet".to_string())),
        HookRole::Context => Ok(StatusCode::NO_CONTENT),
        _ => {
            let at_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let rec = HookEventRecord {
                at_ms,
                role,
                agent,
                tab: header(&headers, HEADER_TAB).map(str::to_string),
                bridge_version: header(&headers, HEADER_VERSION).unwrap_or("").to_string(),
                body: json,
            };
            st.hook_log().push(rec, |tab| st.session(tab).is_some());
            Ok(StatusCode::NO_CONTENT)
        }
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
