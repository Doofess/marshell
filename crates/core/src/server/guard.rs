use super::AppState;
use crate::auth;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use marshell_protocol::brand;

/// Outermost check, runs before CORS so even preflights need a good `Host`.
/// `Host` must be present exactly once and equal `127.0.0.1:<port>` (blocks DNS rebinding).
pub async fn host_guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let hosts = req.headers().get_all(header::HOST);
    let mut values = hosts.iter();
    let host_ok = match (values.next(), values.next()) {
        (Some(h), None) => h.to_str().ok() == Some(st.host()),
        _ => false,
    };
    if !host_ok {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(req).await
}

/// Inside CORS. `Origin`, when present, must appear once and be on the allowlist (blocks other
/// web pages). Then the per-launch token must be present and correct (Bearer header, or
/// `token.<t>` WebSocket subprotocol).
pub async fn auth_guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    if headers.get_all(header::ORIGIN).iter().count() > 1 {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Some(origin) = headers.get(header::ORIGIN) {
        let allowed = origin.to_str().map(|o| st.origin_allowed(o)).unwrap_or(false);
        if !allowed {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    let ok = request_token(headers).is_some_and(|t| auth::tokens_match(&t, st.token()));
    if !ok {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(req).await
}

/// The token from `Authorization: Bearer <t>` or from a `token.<t>` entry in `Sec-WebSocket-Protocol`.
pub fn request_token(headers: &HeaderMap) -> Option<String> {
    if let Some(t) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        return Some(t.trim().to_string());
    }
    let protocols = headers.get(header::SEC_WEBSOCKET_PROTOCOL)?.to_str().ok()?;
    protocols
        .split(',')
        .map(str::trim)
        .find_map(|p| p.strip_prefix(brand::TOKEN_SUBPROTOCOL_PREFIX))
        .map(str::to_string)
}
