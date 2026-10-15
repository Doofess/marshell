use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Linux
        }
    }
}

/// What the window needs to reach the core. Handed over by the Tauri command `core_endpoint`.
#[derive(Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Endpoint {
    pub port: u16,
    pub token: String,
    pub platform: Platform,
}

/// Written by hand so the token never lands in a log or a panic message.
impl std::fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Endpoint")
            .field("port", &self.port)
            .field("token", &"<redacted>")
            .field("platform", &self.platform)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateSessionRequest {
    /// Full argv. Absent means the platform default shell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub shell: Option<Vec<String>>,
    /// Absent means the user's home directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
    /// The profile this session was launched from. Absent means the default profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub profile_id: Option<String>,
    /// "claude", "codex" or "antigravity". Absent means a plain shell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub agent: Option<String>,
    /// Display name for the session. Absent means the core picks one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    /// Text typed into the agent once it starts. Absent means nothing is typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateSessionResponse {
    pub tab_id: String,
    /// The session number the core assigned (lowest free slot).
    pub number: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum SessionState {
    Running,
    Exited {
        /// The process exit code.
        code: i32,
    },
}

/// One session as the core lists it. The core owns number, name and order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionInfo {
    pub tab_id: String,
    /// Stable while the session lives; never renumbered on reorder.
    pub number: u32,
    pub name: String,
    /// "claude", "codex" or "antigravity". None means a plain shell.
    pub agent: Option<String>,
    pub profile_id: String,
    pub cwd: String,
    pub state: SessionState,
}

/// GET /v1/sessions body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionsSnapshot {
    /// Open sessions in tab order.
    pub sessions: Vec<SessionInfo>,
    /// Recently closed sessions, newest first.
    pub closed: Vec<SessionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RenameRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReorderRequest {
    /// Every open tab id, in the new order.
    pub tab_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ShellFamily {
    PowerShell,
    Cmd,
    Posix,
    Wsl,
}

/// A shell the user can launch sessions in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// Full argv.
    pub command: Vec<String>,
    pub family: ShellFamily,
    /// Folder the shell starts in. None means the user's home directory.
    pub cwd: Option<String>,
    /// Extra environment variables as (name, value) pairs.
    pub env: Vec<(String, String)>,
    /// True for profiles found on this machine and not edited by the user.
    pub detected: bool,
}

/// GET/PUT /v1/profiles body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProfilesDoc {
    pub profiles: Vec<Profile>,
    pub default_id: String,
}

/// A project folder offered by the launcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
    pub branch: Option<String>,
    /// Milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub last_used_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Resize {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Health {
    pub app: String,
    pub version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_debug_redacts_the_token() {
        let ep = Endpoint {
            port: 1234,
            token: "secret-token".into(),
            platform: Platform::Linux,
        };
        let s = format!("{ep:?}");
        assert!(!s.contains("secret-token"), "{s}");
        assert!(s.contains("token: \"<redacted>\""), "{s}");
        assert!(s.contains("port: 1234"), "{s}");
    }

    #[test]
    fn session_state_serializes_tagged() {
        let json = serde_json::to_string(&SessionState::Exited { code: 2 }).unwrap();
        assert_eq!(json, r#"{"kind":"exited","code":2}"#);
        let json = serde_json::to_string(&SessionState::Running).unwrap();
        assert_eq!(json, r#"{"kind":"running"}"#);
    }

    #[test]
    fn create_request_old_shape_still_parses() {
        let req: CreateSessionRequest = serde_json::from_str(r#"{"cols":80,"rows":24}"#).unwrap();
        assert_eq!((req.cols, req.rows), (80, 24));
        assert!(req.shell.is_none() && req.cwd.is_none());
        assert!(req.profile_id.is_none() && req.agent.is_none());
        assert!(req.name.is_none() && req.prompt.is_none());
    }
}
