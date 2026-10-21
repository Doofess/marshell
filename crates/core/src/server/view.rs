use super::AppState;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use marshell_protocol::api::UiView;

/// The UI reports a handful of visible tabs (at most four panes); more than this is a broken caller.
const MAX_VISIBLE_TABS: usize = 16;
/// The largest body the route reads; the router's body limit sits above it so the refusal is a 400.
pub(super) const MAX_BODY: usize = 4096;

/// Records what the UI shows (window focus, tabs in view) for the notifier to read.
pub async fn put(State(st): State<AppState>, body: Bytes) -> Result<StatusCode, (StatusCode, String)> {
    if body.len() > MAX_BODY {
        return Err((StatusCode::BAD_REQUEST, "The body is over 4 KiB.".into()));
    }
    let view: UiView = serde_json::from_slice(&body).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    if view.visible_tabs.len() > MAX_VISIBLE_TABS {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("At most {MAX_VISIBLE_TABS} visible tabs."),
        ));
    }
    st.report_view(view);
    Ok(StatusCode::NO_CONTENT)
}
