use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    /// The mode an agent session starts in. Absent starts it with no mode flags. A plain shell ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub mode: Option<SessionMode>,
    /// The terminal's colour scheme right now. Absent means the core sets no `COLORFGBG`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub color_scheme: Option<ColorScheme>,
}

/// How much an agent may do before it asks. The core turns it into the CLI's own flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum SessionMode {
    Manual,
    Plan,
    AutoEdit,
    FullAuto,
    Bypass,
}

/// Whether a terminal's background is dark or light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ColorScheme {
    Dark,
    Light,
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
    /// A tab restored from layout.json whose process is not running: it ended in an earlier run of the app.
    Ended,
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
    /// The git repo root of `cwd` (the main checkout, for a task worktree), or `cwd` itself outside git.
    pub project: String,
    /// The mode the process started in. None for a plain shell, or an agent started with no mode.
    pub mode: Option<SessionMode>,
    pub state: SessionState,
    /// What the shell is doing, from its prompt marks. Unknown without shell integration.
    pub command: CommandState,
    /// True while the core has frozen the process tree. Always false once the session has ended.
    #[serde(default)]
    pub paused: bool,
}

/// A shell session's command state, folded from OSC 133 marks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum CommandState {
    /// No prompt mark seen: no integration, or the shell hasn't drawn its first prompt.
    Unknown,
    /// At the prompt. `last_exit` is the most recent finished command's code, if the shell reports one.
    Prompt { last_exit: Option<i32> },
    /// A command is running (an OSC 133 C mark arrived).
    Running,
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

/// One tab as layout.json remembers it. Enough to list it as Ended and to start it again with Resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SavedTab {
    pub tab_id: String,
    pub number: u32,
    pub name: String,
    /// "claude", "codex" or "antigravity". None means a plain shell.
    pub agent: Option<String>,
    pub profile_id: String,
    pub cwd: String,
    /// The mode it started in. A layout.json from before this field existed reads as None.
    #[serde(default)]
    pub mode: Option<SessionMode>,
}

/// How the window was laid out: which tabs sit in which panes, and the sidebar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ViewLayout {
    /// One entry per pane (1 to 4): the tab shown there, or None for an empty pane.
    pub panes: Vec<Option<String>>,
    /// An Arrangement name, or None for automatic.
    pub arrangement: Option<String>,
    /// Pane size ratios. Empty means even.
    pub ratios: Vec<f32>,
    /// Index of the focused pane.
    pub active: u32,
    /// Index of the zoomed pane, if one is zoomed.
    pub zoomed: Option<u32>,
    /// "expanded", "rail" or "focus".
    pub sidebar: String,
    pub sidebar_width: u32,
    pub shell_hidden: bool,
    pub grouped: bool,
}

impl Default for ViewLayout {
    fn default() -> Self {
        Self {
            panes: vec![None],
            arrangement: None,
            ratios: Vec::new(),
            active: 0,
            zoomed: None,
            sidebar: "expanded".into(),
            sidebar_width: 288,
            shell_hidden: false,
            grouped: false,
        }
    }
}

/// The last profile and agent picked for a folder in the launcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LauncherChoice {
    pub profile_id: String,
    pub agent: Option<String>,
    /// A counter that says which folder was used last: the newest choice has the highest number.
    /// A layout.json from before this field existed reads as 0.
    #[serde(default)]
    pub seq: u32,
    /// The mode last used here. Never Full auto or Bypass. A layout.json from before this field reads as None.
    #[serde(default)]
    pub mode: Option<SessionMode>,
}

/// The whole of layout.json.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LayoutDoc {
    pub tabs: Vec<SavedTab>,
    pub view: ViewLayout,
    /// Keyed by folder path.
    pub launcher: BTreeMap<String, LauncherChoice>,
}

/// The 409 body when Resume cannot start a tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResumeError {
    pub reason: String,
}

/// POST /v1/sessions/{tab}/restart body: start an agent session again in another mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RestartRequest {
    pub mode: SessionMode,
    pub cols: u16,
    pub rows: u16,
}

/// One task: a git worktree on its own branch, made by Marshell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub branch: String,
    pub base_branch: String,
    /// The main checkout the worktree belongs to.
    pub repo_root: String,
    /// The worktree folder.
    pub path: String,
    /// Milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub created_ms: u64,
    /// The worktree folder is gone (deleted by hand). Set by GET /v1/tasks when it answers; such a
    /// task can only be removed. Stored as false, and a file without it reads as false.
    #[serde(default)]
    pub folder_missing: bool,
}

/// POST /v1/tasks/plan body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskPlanRequest {
    /// A folder inside the repository.
    pub cwd: String,
    /// Raw text or a prompt; the core makes the task name from it.
    pub name: String,
}

/// POST /v1/tasks body: the plan request plus the commands the user was shown. The core plans again and refuses
/// when its commands differ, so only what was shown runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskCreateRequest {
    /// A folder inside the repository.
    pub cwd: String,
    /// Raw text or a prompt; the core makes the task name from it.
    pub name: String,
    /// The commands from the plan the user confirmed.
    pub commands: Vec<String>,
}

/// What creating a task would do, shown before it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskPlan {
    pub name: String,
    pub branch: String,
    pub base_branch: String,
    pub repo_root: String,
    pub path: String,
    /// The git commands, as the user reads them.
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChangedFile {
    pub path: String,
    /// "added", "modified", "deleted", "renamed" or "untracked".
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskChanges {
    pub files: Vec<ChangedFile>,
    pub diff: String,
    pub truncated: bool,
    pub ahead: u32,
    pub uncommitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MergeRequest {
    pub message: String,
    pub remove: bool,
    /// The commands the user was shown. A merge refuses to run when planning again gives others.
    /// Ignored when only planning.
    pub commands: Vec<String>,
    /// The warnings the user was shown, under the same rule (the sessions a removal closes).
    /// Ignored when only planning.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MergePlan {
    pub commands: Vec<String>,
    /// What the merge would close or lose, shown with the commands: the sessions a removal closes.
    pub warnings: Vec<String>,
    pub blocker: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum MergeOutcome {
    /// `removal_note`: why the worktree was kept when a removal was asked for, in one sentence.
    Merged {
        removed: bool,
        removal_note: Option<String>,
    },
    Conflict {
        files: Vec<String>,
    },
    Failed {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RemovePlan {
    pub commands: Vec<String>,
    pub warnings: Vec<String>,
    pub sessions: u32,
}

/// POST /v1/tasks/{id}/remove body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RemoveRequest {
    /// The commands the user was shown. Removing refuses to run when planning again gives others.
    pub commands: Vec<String>,
    /// The warnings the user was shown, under the same rule: what would be lost is part of what
    /// they agreed to.
    pub warnings: Vec<String>,
}

/// POST /v1/sessions/{tab}/input body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct InputRequest {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RenameRequest {
    pub name: String,
}

/// POST /v1/sessions/reopen body. The whole body is optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReopenRequest {
    /// The closed session to reopen. Absent means the most recently closed one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tab_id: Option<String>,
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
    /// Start plain shell sessions through Marshell's init script (prompt marks, command status).
    #[serde(default = "default_true")]
    pub shell_integration: bool,
    /// This profile's terminal theme: "dark", "light" or an imported theme's id.
    /// Absent means the app's terminal theme setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub terminal_theme: Option<String>,
    /// True for profiles found on this machine and not edited by the user.
    pub detected: bool,
}

fn default_true() -> bool {
    true
}

/// POST /v1/profiles/integration body: what integration would do for this profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct IntegrationInfo {
    pub applies: bool,
    /// Why it doesn't apply; None when it does.
    pub reason: Option<String>,
    /// Where the script lives, for the read-only view.
    pub script_path: Option<String>,
    /// The script text (for cmd, the PROMPT value; for WSL, the dispatcher).
    pub script: Option<String>,
}

/// GET/PUT /v1/profiles body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProfilesDoc {
    pub profiles: Vec<Profile>,
    pub default_id: String,
}

/// The app's light or dark look. Follow system tracks the operating system.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum AppTheme {
    #[default]
    FollowSystem,
    Dark,
    Light,
}

/// High contrast: follow the system setting, or force it on or off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ContrastMode {
    #[default]
    FollowSystem,
    On,
    Off,
}

/// The size of the app's own text (the terminal font size is separate).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum TextSize {
    #[default]
    Default,
    Large,
    Larger,
}

/// The terminal's font. `family: None` means the bundled font.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct TerminalFont {
    pub family: Option<String>,
    pub size: u16,
}

impl Default for TerminalFont {
    fn default() -> Self {
        Self { family: None, size: 13 }
    }
}

/// A terminal colour scheme. Every colour is `#rrggbb`, lower case.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct TerminalColors {
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub selection: String,
    /// The 16 ANSI colours, black first.
    pub ansi: Vec<String>,
}

/// A colour scheme the user imported from another terminal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct ImportedTheme {
    pub id: String,
    pub name: String,
    /// "windows-terminal", "iterm2" or "vscode".
    pub source: String,
    pub colors: TerminalColors,
}

/// How the app and its terminals look.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct Appearance {
    pub theme: AppTheme,
    /// A preset name (sand, amber, blue, indigo, violet, magenta, cyan, teal, slate) or `#rrggbb`.
    pub accent: String,
    pub contrast: ContrastMode,
    pub text_size: TextSize,
    /// "follow-app", "dark", "light" or the id of an imported theme.
    pub terminal_theme: String,
    pub terminal_font: TerminalFont,
    /// Lift terminal text that would be hard to read against its background.
    pub contrast_protection: bool,
    pub imported_themes: Vec<ImportedTheme>,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: AppTheme::default(),
            accent: "sand".into(),
            contrast: ContrastMode::default(),
            text_size: TextSize::default(),
            terminal_theme: "follow-app".into(),
            terminal_font: TerminalFont::default(),
            contrast_protection: true,
            imported_themes: Vec::new(),
        }
    }
}

/// GET/PUT /v1/settings body.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct Settings {
    pub appearance: Appearance,
    /// True once the user has seen the network warning that comes with the first Pause.
    pub pause_warned: bool,
}

/// A file the core found for the user to import from (the core only reads it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FoundFile {
    pub path: String,
    pub text: String,
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
    fn ended_state_serializes_as_kind_ended() {
        assert_eq!(
            serde_json::to_string(&SessionState::Ended).unwrap(),
            r#"{"kind":"ended"}"#
        );
    }

    #[test]
    fn view_layout_default_matches_plan() {
        let view = ViewLayout::default();
        assert_eq!(view.sidebar_width, 288);
        assert_eq!(view.panes, vec![None::<String>]);
        assert!(!view.grouped);
    }

    #[test]
    fn profile_without_shell_integration_loads_as_on() {
        let p: Profile = serde_json::from_str(
            r#"{"id":"pwsh","name":"PowerShell","command":["pwsh.exe"],"family":"power_shell","cwd":null,"env":[],"detected":true}"#,
        )
        .unwrap();
        assert!(p.shell_integration);
    }

    #[test]
    fn command_state_serializes_tagged() {
        let json = serde_json::to_string(&CommandState::Prompt { last_exit: Some(1) }).unwrap();
        assert_eq!(json, r#"{"kind":"prompt","last_exit":1}"#);
        let json = serde_json::to_string(&CommandState::Running).unwrap();
        assert_eq!(json, r#"{"kind":"running"}"#);
    }

    #[test]
    fn create_request_old_shape_still_parses() {
        let req: CreateSessionRequest = serde_json::from_str(r#"{"cols":80,"rows":24}"#).unwrap();
        assert_eq!((req.cols, req.rows), (80, 24));
        assert!(req.shell.is_none() && req.cwd.is_none());
        assert!(req.profile_id.is_none() && req.agent.is_none());
        assert!(req.name.is_none() && req.prompt.is_none());
        assert!(req.mode.is_none());
    }

    #[test]
    fn session_mode_is_kebab_case() {
        let req: CreateSessionRequest = serde_json::from_str(r#"{"cols":80,"rows":24,"mode":"auto-edit"}"#).unwrap();
        assert_eq!(req.mode, Some(SessionMode::AutoEdit));
        assert_eq!(serde_json::to_string(&SessionMode::FullAuto).unwrap(), r#""full-auto""#);
    }

    #[test]
    fn create_request_color_scheme_is_kebab_case() {
        let req: CreateSessionRequest =
            serde_json::from_str(r#"{"cols":80,"rows":24,"color_scheme":"light"}"#).unwrap();
        assert_eq!(req.color_scheme, Some(ColorScheme::Light));
        let req: CreateSessionRequest = serde_json::from_str(r#"{"cols":80,"rows":24}"#).unwrap();
        assert_eq!(req.color_scheme, None);
    }

    #[test]
    fn profile_terminal_theme_is_optional() {
        let p: Profile = serde_json::from_str(
            r#"{"id":"pwsh","name":"PowerShell","command":["pwsh.exe"],"family":"power_shell","cwd":null,"env":[],"detected":true}"#,
        )
        .unwrap();
        assert_eq!(p.terminal_theme, None);
        // Absent stays absent when written back.
        assert!(!serde_json::to_string(&p).unwrap().contains("terminal_theme"));
    }
}
