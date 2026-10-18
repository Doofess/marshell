//! One module per agent CLI. Each holds that agent's own JSON; the code around it stays vendor-neutral.

pub mod claude;

use marshell_protocol::hook::PermissionDecision;
use serde_json::Value;

/// The hook output that carries `decision` back to `agent`, or why it can't.
pub fn permission_output(agent: &str, decision: &PermissionDecision, request: &Value) -> Result<Value, &'static str> {
    match agent {
        "claude" => claude::permission_output(decision, request),
        _ => Err("This agent can't take a decision yet."),
    }
}
