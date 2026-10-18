use std::io::Read;
use std::time::Duration;

use marshell_protocol::hook::{HEADER_AGENT, HEADER_ROLE, HEADER_TAB, HEADER_VERSION, HOOK_BODY_LIMIT, HookRole};

use crate::args::Args;

pub const CONNECT_TIMEOUT: Duration = Duration::from_millis(50);

/// How long a whole request may take, by role.
pub fn budget(role: HookRole) -> Duration {
    match role {
        HookRole::Event | HookRole::Record => Duration::from_millis(400),
        HookRole::Context => Duration::from_millis(150),
        HookRole::Statusline => Duration::from_millis(100),
        HookRole::Permission => Duration::from_secs(590),
    }
}

pub enum PostError {
    /// Nothing is listening: the app is closed, which is normal and not logged.
    Closed,
    Failed(String),
}

/// POSTs `body` to the core. Returns the status and the (capped) response body.
pub fn post(
    port: u16,
    token: &str,
    args: &Args,
    tab: Option<&str>,
    body: &[u8],
    budget: Duration,
) -> Result<(u16, Vec<u8>), PostError> {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(budget))
        .http_status_as_error(false)
        // Loopback only: no proxy from the environment, and never follow a redirect with a token.
        .proxy(None)
        .max_redirects(0)
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let mut req = agent
        .post(format!("http://127.0.0.1:{port}/v1/hook"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header(HEADER_ROLE, args.role.as_str())
        .header(HEADER_AGENT, args.agent.as_str())
        .header(HEADER_VERSION, env!("CARGO_PKG_VERSION"));
    if let Some(tab) = tab {
        req = req.header(HEADER_TAB, tab);
    }
    let mut resp = req.send(body).map_err(classify)?;
    let status = resp.status().as_u16();
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(HOOK_BODY_LIMIT as u64)
        .read_to_vec()
        .map_err(classify)?;
    Ok((status, bytes))
}

fn classify(e: ureq::Error) -> PostError {
    match &e {
        ureq::Error::Io(io) if io.kind() == std::io::ErrorKind::ConnectionRefused => PostError::Closed,
        // Windows retries a refused loopback connect for about a second; the 50 ms limit cuts it.
        ureq::Error::Timeout(ureq::Timeout::Connect) => PostError::Closed,
        _ => PostError::Failed(format!("request failed: {e}")),
    }
}

/// Reads at most `HOOK_BODY_LIMIT` bytes. `None` means the input was larger.
pub fn read_capped(stdin: &mut dyn Read) -> std::io::Result<Option<Vec<u8>>> {
    let mut buf = Vec::new();
    stdin.take(HOOK_BODY_LIMIT as u64 + 1).read_to_end(&mut buf)?;
    Ok((buf.len() <= HOOK_BODY_LIMIT).then_some(buf))
}
