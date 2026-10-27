use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Whether an agent CLI is signed in, as its own status command says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SignIn {
    SignedIn,
    SignedOut,
    /// The CLI wasn't found, or its status command gave no answer.
    Unknown,
    /// The CLI has no status command (Antigravity).
    NoCommand,
}

/// `GET /v1/agents/{agent}/account`: the sign-in state of one agent CLI. The CLI's own output never goes into it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AccountStatus {
    pub agent: String,
    pub sign_in: SignIn,
    /// The signed-in account's email, when the CLI reports one.
    pub email: Option<String>,
    /// The plan (Claude's `subscriptionType`), when the CLI reports one.
    pub plan: Option<String>,
    /// How it is signed in: "ChatGPT" or "an API key" (Codex).
    pub method: Option<String>,
    /// When the check finished, in milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub checked_ms: u64,
}
