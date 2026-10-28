//! The CLI's client of the running app's loopback API. It finds the app through `<home>/run/endpoint.json`, as the
//! hook bridge does, and sends the token as a bearer. The token is never printed: it is replaced in any error text.

use marshell_protocol::api::{FillRequest, SessionInfo, SessionsSnapshot};
use std::io;
use std::time::Duration;

/// The app is closed: there is no `endpoint.json`, or it names a process that is gone.
#[derive(Debug, PartialEq, Eq)]
pub struct NotRunning;

/// Printed when the app is closed.
pub const NOT_RUNNING: &str = "Marshell isn't running.";

/// Why the core did not fill a prompt line. Each one prints as one line.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The core's own line (a text refusal, a 404, a paused session, or a failure to reach it): printed as it is.
    Line(String),
    /// The session has ended: the line names the session by number.
    Ended,
    /// The agent wasn't ready within the core's 30 s.
    NotReady,
}

pub struct Client {
    port: u16,
    token: String,
}

/// How long a request may take.
const TIMEOUT: Duration = Duration::from_secs(5);
/// `fill` with `wait` is held by the core for up to 30 s: it gets a little longer than that.
const WAIT_TIMEOUT: Duration = Duration::from_secs(35);

impl Client {
    pub fn connect(env: &dyn Fn(&str) -> Option<String>) -> Result<Client, NotRunning> {
        let (_, port, token) = marshell_hook::endpoint::discover(env).ok_or(NotRunning)?;
        Ok(Client { port, token })
    }

    /// `GET /v1/sessions`.
    pub fn sessions(&self) -> io::Result<SessionsSnapshot> {
        let (status, body) = self.request("/v1/sessions", None, TIMEOUT)?;
        if status != 200 {
            return Err(io::Error::other(format!("Marshell answered {status}: {}", body.trim())));
        }
        serde_json::from_str(&body).map_err(|e| io::Error::other(format!("Marshell's answer didn't parse: {e}")))
    }

    /// `POST /v1/sessions/{tab}/fill`.
    pub fn fill(&self, tab: &str, text: &str, wait: bool) -> Result<(), Refusal> {
        let body = serde_json::to_value(FillRequest {
            text: text.to_string(),
            wait,
        })
        .map_err(|e| Refusal::Line(e.to_string()))?;
        let timeout = if wait { WAIT_TIMEOUT } else { TIMEOUT };
        match self.request(&format!("/v1/sessions/{tab}/fill"), Some(&body), timeout) {
            Ok((204, _)) => Ok(()),
            Ok((409, body)) => Err(refusal(&body)),
            Ok((_, body)) => Err(Refusal::Line(body.trim().to_string())),
            Err(e) => Err(Refusal::Line(e.to_string())),
        }
    }

    /// POSTs `body` as JSON. Returns the status and the body text of the answer.
    #[allow(dead_code)] // `new` posts through this.
    pub fn post(&self, path: &str, body: &serde_json::Value) -> io::Result<(u16, String)> {
        self.request(path, Some(body), TIMEOUT)
    }

    /// A GET without a body, a POST with one.
    fn request(&self, path: &str, body: Option<&serde_json::Value>, timeout: Duration) -> io::Result<(u16, String)> {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            // Loopback only: no proxy from the environment, and never follow a redirect with a token.
            .proxy(None)
            .max_redirects(0)
            .build();
        let agent = ureq::Agent::new_with_config(config);
        let url = format!("http://127.0.0.1:{}{path}", self.port);
        let auth = format!("Bearer {}", self.token);
        let result = match body {
            Some(body) => agent
                .post(url)
                .header("Authorization", auth)
                .header("Content-Type", "application/json")
                .send(body.to_string().as_bytes()),
            None => agent.get(url).header("Authorization", auth).call(),
        };
        let mut resp = result.map_err(|e| self.failure(e))?;
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().map_err(|e| self.failure(e))?;
        Ok((status, text))
    }

    /// An `io::Error` for a failed request, with the token replaced in its text.
    fn failure(&self, e: ureq::Error) -> io::Error {
        let kind = match &e {
            ureq::Error::Io(io) => io.kind(),
            _ => io::ErrorKind::Other,
        };
        io::Error::new(kind, e.to_string().replace(&self.token, "***"))
    }
}

/// The open session `session` names: a number matches only `sessions` (open rows), and any other string must be a
/// tab id. A closed row never matches, as its number may belong to another session by now.
pub fn resolve<'a>(snapshot: &'a SessionsSnapshot, session: &str) -> Option<&'a SessionInfo> {
    if !session.is_empty() && session.bytes().all(|b| b.is_ascii_digit()) {
        let number: u32 = session.parse().ok()?;
        return snapshot.sessions.iter().find(|s| s.number == number);
    }
    snapshot.sessions.iter().find(|s| s.tab_id == session)
}

/// The refusal for a 409 body `{"reason": ..}`.
fn refusal(body: &str) -> Refusal {
    let reason = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("reason")?.as_str().map(str::to_string));
    match reason.as_deref() {
        Some("ended") => Refusal::Ended,
        Some("not_ready") => Refusal::NotReady,
        Some(other) => Refusal::Line(other.to_string()),
        None => Refusal::Line(body.trim().to_string()),
    }
}

#[cfg(test)]
pub fn sample(number: u32, tab: &str, state: marshell_protocol::api::SessionState) -> SessionInfo {
    let mut row: SessionInfo = serde_json::from_value(serde_json::json!({
        "tab_id": tab, "number": number, "name": format!("Session {number}"), "agent": "claude",
        "profile_id": "claude", "cwd": "C:\\work\\app", "project": "C:\\work", "mode": "plan",
        "state": { "kind": "running" }, "command": { "kind": "unknown" },
    }))
    .unwrap();
    row.state = state;
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::SessionState;

    #[test]
    fn resolve_matches_open_numbers_and_tab_ids() {
        let snapshot = SessionsSnapshot {
            sessions: vec![
                sample(1, "tab-a", SessionState::Running),
                sample(6, "tab-six", SessionState::Ended),
            ],
            // Number 2 was closed and may be handed out again.
            closed: vec![sample(2, "tab-closed", SessionState::Ended)],
        };
        assert_eq!(resolve(&snapshot, "1").unwrap().tab_id, "tab-a");
        assert_eq!(resolve(&snapshot, "6").unwrap().tab_id, "tab-six");
        assert_eq!(resolve(&snapshot, "tab-a").unwrap().number, 1);
        // A closed session matches neither by number nor by tab id.
        assert!(resolve(&snapshot, "2").is_none());
        assert!(resolve(&snapshot, "tab-closed").is_none());
        // No partial matches, no signs.
        assert!(resolve(&snapshot, "tab").is_none());
        assert!(resolve(&snapshot, "+1").is_none());
        assert!(resolve(&snapshot, "").is_none());
    }

    #[test]
    fn a_409_reason_becomes_a_refusal() {
        assert_eq!(refusal(r#"{"reason":"ended"}"#), Refusal::Ended);
        assert_eq!(refusal(r#"{"reason":"not_ready"}"#), Refusal::NotReady);
        assert_eq!(
            refusal(r#"{"reason":"It is paused."}"#),
            Refusal::Line("It is paused.".into())
        );
    }
}
