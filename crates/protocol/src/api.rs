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
    /// A preset the agent starts with ("deepseek", on Claude Code only). Absent starts the agent as it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub preset: Option<String>,
    /// The terminal's colour scheme right now. Absent means the core sets no `COLORFGBG`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub color_scheme: Option<ColorScheme>,
    /// A tool preset to type into the shell. It takes no agent, prompt, mode or agent preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tool: Option<crate::tools::ToolLaunch>,
    /// A sign-in or update tab for a company CLI. It takes no shell, agent, prompt, mode, preset or tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cli_task: Option<CliTask>,
}

/// What a CLI-task tab runs. The core builds the command; the frontend never sends one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CliTaskKind {
    /// The CLI's own sign-in command.
    SignIn,
    /// The CLI's update command, by how it was installed.
    Update,
}

/// A sign-in or update tab for one company CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CliTask {
    /// "claude", "codex" or "antigravity".
    pub agent: String,
    pub kind: CliTaskKind,
}

/// How much an agent may do before it asks. The core turns it into the CLI's own flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum SessionMode {
    Manual,
    Plan,
    AutoEdit,
    FullAuto,
    Bypass,
    DontAsk,
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
    /// The program a plain shell session is running in front of its shell, found by the core.
    /// None at the prompt, and always None for a session launched as an agent.
    #[serde(default)]
    pub foreground: Option<Foreground>,
    /// What an agent is doing, folded by the core from its hook events and status line. None for a
    /// plain shell, and for an agent whose live status is unavailable (see `status_note`).
    #[serde(default)]
    pub live: Option<LiveStatus>,
    /// Output activity of an agent row that has no `live`: the pty fallback. Never Needs you, Done or Error.
    #[serde(default)]
    pub activity: Option<Activity>,
    /// Why this agent row has no `live`: an unknown version, or the per-agent switch being off.
    #[serde(default)]
    pub status_note: Option<String>,
    /// True while the user has muted this session's notifications.
    #[serde(default)]
    pub muted: bool,
    /// A terminal program asked for attention (OSC 9, OSC 777 or a bell) in a session without hook
    /// status. Cleared when the person types into the session or looks at it.
    #[serde(default)]
    pub attention: Option<Attention>,
    /// A hooked agent that repeats one tool call or goes quiet while Working. Never notifies.
    #[serde(default)]
    pub stuck: Option<Stuck>,
    /// The preset the agent was started with ("deepseek"), shown beside the agent. The row's `agent` stays "claude".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub preset: Option<String>,
    /// The local ports the tab's tools announced, at most 8. A chip opens its loopback URL.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<crate::tools::PortChip>>")]
    pub ports: Vec<crate::tools::PortChip>,
}

impl SessionInfo {
    /// True while the tab takes the person's text as a running session: its process runs and, on an agent row,
    /// the agent has not ended. An agent whose hooks reported SessionEnd counts as ended while its shell lives
    /// on; a later SessionStart (a `--resume` typed in the same tab) makes it running again. A plain shell, and an
    /// agent row with no `live`, follow the process. Whether the process itself runs (pausing, closing, the pty)
    /// is `state == SessionState::Running`, not this.
    pub fn running(&self) -> bool {
        self.state == SessionState::Running
            && !(self.agent.is_some() && self.live.as_ref().is_some_and(|l| l.state == LiveState::Ended))
    }
}

/// A terminal program's request for attention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Attention {
    /// The notification's text: trimmed, capped at 200 chars. None for a bare bell.
    pub message: Option<String>,
    /// When the first signal of this attention arrived. A flood keeps the first time.
    #[ts(type = "number")]
    pub since_ms: u64,
    /// True when the attention comes from a tool's prompt text matched on screen: a guess from text, not a hook.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(optional, as = "Option<bool>")]
    pub limited: bool,
}

/// Why an agent looks stuck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum StuckReason {
    /// The same tool call with identical input, 9 times in 4 minutes.
    Repeat,
    /// Working with no hook event for 10 minutes.
    Quiet,
}

/// An agent that looks stuck, and since when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Stuck {
    pub reason: StuckReason,
    #[ts(type = "number")]
    pub since_ms: u64,
}

/// What an agent session is doing, as its hooks report it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LiveState {
    Idle,
    Working,
    NeedsPermission,
    NeedsQuestion,
    Done,
    Error,
    Ended,
}

/// The last thing an agent said when it finished a turn, and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Recap {
    pub text: String,
    #[ts(type = "number")]
    pub at_ms: u64,
}

/// An agent session's live status. Percentages are whole numbers, 0 to 100; unknown is None.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LiveStatus {
    pub state: LiveState,
    /// Line 2 of the row, e.g. "Running `npm test`". Empty when there is nothing to say.
    pub phrase: String,
    /// When `state` last changed.
    #[ts(type = "number")]
    pub since_ms: u64,
    pub recap: Option<Recap>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub context_pct: Option<u8>,
    pub five_hour_pct: Option<u8>,
    /// When the 5-hour window resets, in Unix seconds as Claude Code sends it.
    #[ts(type = "number | null")]
    pub five_hour_resets_at: Option<u64>,
    pub seven_day_pct: Option<u8>,
    pub subagents: u32,
    /// How many subagents have stopped since this status began (a new process starts it again at 0). The core
    /// only: the notifier tells a finished subagent from the main turn's Stop clearing the set. Never sent.
    #[serde(skip)]
    #[ts(skip)]
    pub subagents_finished: u32,
}

/// A row's output activity when its hooks say nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Activity {
    Working,
    Idle,
}

/// What a shell session is running right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Foreground {
    /// The program's name, lowercase and without `.exe`, as the agent table knows it.
    pub program: String,
    /// The command line the shell reported: its first line, trimmed and capped at 200 chars.
    pub command_line: Option<String>,
    /// "claude", "codex" or "antigravity" when the program is a known agent CLI.
    pub agent: Option<String>,
    /// The id of the tool profile the running command matched ("vercel", "terraform"), if one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tool: Option<String>,
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
    /// Whether the user muted this session's notifications. A layout.json from before this field existed reads as false.
    #[serde(default)]
    pub muted: bool,
    /// The preset it started with. A layout.json from before this field existed reads as None. Never a key:
    /// a restart reads the key again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub preset: Option<String>,
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
    /// The preset last used here. A layout.json from before this field reads as None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub preset: Option<String>,
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

/// Which conversation a copied resume command continues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ResumeForm {
    /// One conversation, by the CLI's session id.
    ById,
    /// The CLI's own "last conversation in this folder": no id is known.
    ContinueLast,
    /// A new conversation: the session never wrote one, so there is nothing to continue.
    Fresh,
}

/// GET /v1/sessions/{tab}/resume-command and GET /v1/history/{agent}/{session_id}/resume-command reply: one line
/// that changes to the session's folder and starts its CLI again, for the person to paste into a terminal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResumeCommand {
    pub command: String,
    pub form: ResumeForm,
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

/// POST /v1/sessions/{tab}/fill body: text for the session's prompt line. Like `InputRequest`, it never presses
/// Enter, and it refuses every control character, so the text can't submit itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FillRequest {
    pub text: String,
    /// Hold the text back until the agent is ready for it (for a session that has only just started). It is
    /// refused with `ResumeError { reason: "not_ready" }` when that takes more than 30 s.
    pub wait: bool,
}

/// POST /v1/sessions/{tab}/resolve body: text that may name files, such as paths from terminal output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResolveRequest {
    /// At most 64, each at most 1024 characters.
    pub candidates: Vec<String>,
}

/// POST /v1/sessions/{tab}/resolve reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResolveResponse {
    /// One entry per candidate, in order: the absolute path of an existing regular file, or null.
    pub paths: Vec<Option<String>>,
}

/// GET /v1/files/stat reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FileStat {
    #[ts(type = "number")]
    pub size: u64,
    /// Last modification, in milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub modified_ms: u64,
}

/// POST /v1/sessions/{tab}/attachments reply: where the image was saved and how to name it to the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AttachmentSaved {
    /// The file name, `<unix ms>.png`, or `<unix ms>-2.png` and up when that name was taken.
    pub name: String,
    /// The absolute path of the saved file.
    pub path: String,
    /// The text to put in the agent's prompt so it reads the image.
    pub reference: String,
}

/// POST /v1/sessions/{tab}/paste body: paste a saved attachment's reference and a comment into the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PasteRequest {
    /// The `name` from `AttachmentSaved`.
    pub attachment: String,
    /// The comment to follow the reference; may be empty. No control characters, at most 2000 characters.
    pub text: String,
}

/// POST /v1/sessions/{tab}/paste-paths body: paste dropped files and folders into the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PastePathsRequest {
    /// 1 to 32 absolute paths that exist, each a file or a folder. No control characters and no `"`.
    pub paths: Vec<String>,
}

/// GET /v1/sessions/{tab}/files reply: the files of the session's project, for quick open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FileList {
    /// The folder the paths are relative to.
    pub root: String,
    /// Relative paths with `/` separators.
    pub files: Vec<String>,
    /// True when the list was cut short at the file cap or the time limit.
    pub truncated: bool,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct Settings {
    pub appearance: Appearance,
    /// True once the user has seen the network warning that comes with the first Pause.
    pub pause_warned: bool,
    /// True once the user has been through (or dismissed) the first-run onboarding.
    pub onboarded: bool,
    /// Whether closing hides the window to the tray. None until the user chooses.
    pub close_to_tray: Option<bool>,
    /// Agent ids whose live status is switched off; their rows use the pty fallback.
    pub status_off: Vec<String>,
    /// Which events notify, and how.
    pub notifications: NotifySettings,
    /// Whether Marshell keeps its own copy of the conversations Claude Code and Codex ran outside it.
    pub history_import: bool,
    /// Recordings and images are kept up to this many GB (1024-based). None is no size limit.
    pub history_limit_gb: Option<u32>,
    /// Recordings and images older than this many days are removed. None keeps them forever.
    pub history_limit_days: Option<u32>,
    /// A folder that holds an exact copy of the history, in a "Marshell history" folder of its own. None is no backup.
    pub history_backup_dir: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            appearance: Appearance::default(),
            pause_warned: false,
            onboarded: false,
            close_to_tray: None,
            status_off: Vec::new(),
            notifications: NotifySettings::default(),
            history_import: true,
            history_limit_gb: None,
            history_limit_days: None,
            history_backup_dir: None,
        }
    }
}

/// How one kind of event notifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct NotifyChannel {
    pub banner: bool,
    pub sound: bool,
}

impl Default for NotifyChannel {
    fn default() -> Self {
        Self {
            banner: true,
            sound: true,
        }
    }
}

/// Hours in which notifications are held back, as "HH:MM" 24-hour strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct QuietHours {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

impl Default for QuietHours {
    fn default() -> Self {
        Self {
            enabled: false,
            start: "22:00".into(),
            end: "08:00".into(),
        }
    }
}

/// The notification settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct NotifySettings {
    pub needs_you: NotifyChannel,
    pub done: NotifyChannel,
    pub error: NotifyChannel,
    /// True when a subagent finishing plays a sound.
    pub subagent_sound: bool,
    pub quiet_hours: QuietHours,
    /// True when OSC 9, OSC 777 and bells from sessions without hooks raise a banner and sound.
    pub terminal_alerts: bool,
}

impl Default for NotifySettings {
    fn default() -> Self {
        let on = NotifyChannel::default();
        Self {
            needs_you: on,
            done: on,
            error: on,
            subagent_sound: false,
            quiet_hours: QuietHours::default(),
            terminal_alerts: true,
        }
    }
}

/// What the user is looking at, as the UI reports it to the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UiView {
    pub window_focused: bool,
    pub visible_tabs: Vec<String>,
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

/// Which of a repo's `marshell.json` scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ScriptKind {
    Setup,
    Run,
    Teardown,
}

/// A checkout's `marshell.json`, read and resolved for this OS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RepoScripts {
    /// The checkout's own root (a linked worktree's, not the main repo's).
    pub root: String,
    /// `<root>/marshell.json`.
    pub file: String,
    /// SHA-256 of the file's bytes. None when there is no file or it is too large to read.
    pub hash: Option<String>,
    pub setup: Option<String>,
    pub run: Option<String>,
    pub teardown: Option<String>,
    /// Whether the person allowed exactly this file for this repo.
    pub trusted: bool,
    /// Why the file can't be used. None when there is no file or it is fine.
    pub problem: Option<String>,
}

/// A folder the launcher was given as text, as the core found it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FolderInfo {
    /// The folder's canonical path.
    pub path: String,
    pub name: String,
    pub branch: Option<String>,
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
    fn notify_settings_and_ui_view_round_trip() {
        let mut n = NotifySettings::default();
        n.done.sound = false;
        n.subagent_sound = true;
        n.quiet_hours.enabled = true;
        let json = serde_json::to_string(&n).unwrap();
        assert_eq!(serde_json::from_str::<NotifySettings>(&json).unwrap(), n);
        assert_eq!(
            serde_json::from_str::<NotifySettings>("{}").unwrap(),
            NotifySettings::default()
        );
        let view = UiView {
            window_focused: true,
            visible_tabs: vec!["a".into(), "b".into()],
        };
        let json = serde_json::to_string(&view).unwrap();
        assert_eq!(serde_json::from_str::<UiView>(&json).unwrap(), view);
    }

    #[test]
    fn session_info_without_attention_or_stuck_deserializes() {
        let json = r#"{"tab_id":"a","number":1,"name":"A","agent":null,"profile_id":"cmd","cwd":"C:/w","project":"C:/w","mode":null,"state":{"kind":"running"},"command":{"kind":"unknown"}}"#;
        let info = serde_json::from_str::<SessionInfo>(json).unwrap();
        assert_eq!(info.attention, None);
        assert_eq!(info.stuck, None);
    }

    #[test]
    fn stuck_reason_serializes_snake_case() {
        let s = Stuck {
            reason: StuckReason::Repeat,
            since_ms: 5,
        };
        assert_eq!(
            serde_json::to_string(&s).unwrap(),
            r#"{"reason":"repeat","since_ms":5}"#
        );
    }

    #[test]
    fn saved_tab_without_muted_deserializes_false() {
        let json = r#"{"tab_id":"a","number":1,"name":"A","agent":null,"profile_id":"cmd","cwd":"C:/w"}"#;
        assert!(!serde_json::from_str::<SavedTab>(json).unwrap().muted);
    }

    #[test]
    fn session_info_without_muted_deserializes_false() {
        let info = SessionInfo {
            tab_id: "a".into(),
            number: 1,
            name: "n".into(),
            agent: None,
            profile_id: "p".into(),
            cwd: "c".into(),
            project: "c".into(),
            mode: None,
            state: SessionState::Running,
            command: CommandState::Unknown,
            paused: false,
            foreground: None,
            live: None,
            activity: None,
            status_note: None,
            muted: true,
            attention: None,
            stuck: None,
            preset: None,
            ports: Vec::new(),
        };
        let mut value = serde_json::to_value(&info).unwrap();
        value.as_object_mut().unwrap().remove("muted");
        let back: SessionInfo = serde_json::from_value(value).unwrap();
        assert!(!back.muted);
    }

    #[test]
    fn running_follows_the_process_and_an_ended_agent() {
        let row = |state, agent: Option<&str>, live: Option<LiveState>| {
            let mut v = serde_json::json!({
                "tab_id": "a", "number": 1, "name": "n", "agent": agent, "profile_id": "p", "cwd": "c",
                "project": "c", "mode": null, "state": { "kind": "running" }, "command": { "kind": "unknown" },
            });
            if let Some(l) = live {
                v["live"] = serde_json::json!({
                    "state": l, "phrase": "", "since_ms": 0, "recap": null, "model": null, "effort": null,
                    "context_pct": null, "five_hour_pct": null, "five_hour_resets_at": null,
                    "seven_day_pct": null, "subagents": 0, "subagents_finished": 0,
                });
            }
            let mut info: SessionInfo = serde_json::from_value(v).unwrap();
            info.state = state;
            info
        };
        assert!(row(SessionState::Running, Some("claude"), None).running());
        assert!(!row(SessionState::Running, Some("claude"), Some(LiveState::Ended)).running());
        assert!(row(SessionState::Running, Some("claude"), Some(LiveState::Idle)).running());
        assert!(row(SessionState::Running, None, None).running());
        assert!(!row(SessionState::Exited { code: 0 }, Some("claude"), Some(LiveState::Idle)).running());
        assert!(!row(SessionState::Ended, None, None).running());
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
        assert!(req.cli_task.is_none());
    }

    #[test]
    fn cli_task_is_snake_case() {
        let req: CreateSessionRequest =
            serde_json::from_str(r#"{"cols":80,"rows":24,"cli_task":{"agent":"claude","kind":"sign_in"}}"#).unwrap();
        assert_eq!(
            req.cli_task,
            Some(CliTask {
                agent: "claude".into(),
                kind: CliTaskKind::SignIn
            })
        );
        assert_eq!(serde_json::to_string(&CliTaskKind::Update).unwrap(), r#""update""#);
    }

    #[test]
    fn session_mode_is_kebab_case() {
        let req: CreateSessionRequest = serde_json::from_str(r#"{"cols":80,"rows":24,"mode":"auto-edit"}"#).unwrap();
        assert_eq!(req.mode, Some(SessionMode::AutoEdit));
        assert_eq!(serde_json::to_string(&SessionMode::FullAuto).unwrap(), r#""full-auto""#);
        assert_eq!(serde_json::to_string(&SessionMode::DontAsk).unwrap(), r#""dont-ask""#);
        assert_eq!(
            serde_json::from_str::<SessionMode>(r#""dont-ask""#).unwrap(),
            SessionMode::DontAsk
        );
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
