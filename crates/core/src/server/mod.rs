mod activity;
pub(crate) mod agents;
mod attachments;
mod catalog;
mod config;
mod diagnostics;
mod events;
mod files;
mod fill;
mod folders;
mod foreground;
pub mod frames;
mod guard;
mod history;
mod hook;
mod layout;
mod outside;
mod permissions;
mod ports;
mod profiles;
mod projects;
mod pty_ws;
mod references;
mod reports;
mod scripts;
mod sessions;
mod settings;
mod shell_panel;
mod tasks;
mod tools;
mod view;

pub(crate) use files::existing_file as local_file;
/// The text check of `fill`, for the command line to run before it creates a session.
pub use fill::check_text;

use crate::profiles::detect::{Probe, detect_profiles};
use crate::registry::{Registry, Relaunch};
use crate::{auth, paths, private};
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, HeaderValue, Method, header};
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use marshell_protocol::api::{Endpoint, Health, LayoutDoc, Platform, Profile, ProfilesDoc, Settings};
use marshell_protocol::brand;
use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Asked to bring Marshell's window forward and select a session's tab.
pub type FocusSink = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Clone)]
pub struct CoreConfig {
    /// Usually `paths::home_dir()`; a temp dir in tests.
    pub home: PathBuf,
    /// Webview origins allowed to call the API, e.g. `http://tauri.localhost`.
    pub allowed_origins: Vec<String>,
    /// The folder holding marshell-hook and marshell, next to the app's exe. None in tests: set up then never
    /// copies them.
    pub programs_dir: Option<PathBuf>,
    /// Where the notifier hands what it decided (banners, sounds, flashes). None in tests that don't care:
    /// then no notifier runs.
    pub notify: Option<crate::notify::NotifySink>,
    /// Where `POST /v1/sessions/{tab}/focus` hands the tab. None in tests that don't care: the route then answers 503.
    pub focus: Option<FocusSink>,
    /// Shared by every core the supervisor starts; neither flag is saved to disk.
    pub notify_paused: Arc<AtomicBool>,
    pub window_hidden: Arc<AtomicBool>,
    /// The folders of Claude Code's and Codex's conversations, for the importer. None in tests: then no importer
    /// runs, so nothing reads the real `~/.claude` or `~/.codex`.
    pub import: Option<crate::history::import::ImportRoots>,
}

/// The view with a hidden window counted as unfocused: the shell's visibility wins over the webview's last report.
fn effective(reported: &marshell_protocol::api::UiView, hidden: bool) -> marshell_protocol::api::UiView {
    let mut view = reported.clone();
    if hidden {
        view.window_focused = false;
        view.visible_tabs.clear();
    }
    view
}

/// Runs `<exe> <args>` and returns its exit code and output; `None` when it can't start or times out.
pub type StatusRunner = fn(&Path, &[&str]) -> Option<(i32, String)>;

/// Shared server state. Cloning is cheap (one `Arc`).
#[derive(Clone)]
pub struct AppState(Arc<Inner>);

/// When `wsl.exe` last answered for an agent in a distro, and the version it printed (None when none could be read).
type WslVersion = (std::time::Instant, Option<String>);

/// Where Marshell's hooks go in a WSL distro: its home as a Windows (UNC) path the core can read and write, and the
/// bridge's path as the distro's Linux sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistroTarget {
    pub home: PathBuf,
    pub bridge: String,
}

/// The distros of the detected WSL profiles, spelt as the profiles name them.
fn wsl_distros(doc: &ProfilesDoc) -> Vec<String> {
    doc.profiles
        .iter()
        .filter(|p| matches!(p.family, marshell_protocol::api::ShellFamily::Wsl))
        .filter_map(|p| crate::wsl::distro_of_argv(&p.command))
        .collect()
}

struct Inner {
    token: String,
    host: String,
    allowed_origins: Vec<String>,
    /// Where `profiles.json` lives.
    home: PathBuf,
    /// Where the shell integration scripts were written. None when writing them failed,
    /// so shells start without integration.
    shell_dir: Option<PathBuf>,
    /// The shells found at startup. Detection runs once, not per request.
    detected: Vec<Profile>,
    /// Detected profiles merged with `profiles.json`.
    profiles: Arc<Mutex<ProfilesDoc>>,
    /// The contents of `settings.json`, or the defaults.
    settings: Mutex<Settings>,
    /// The contents of `layout.json`, or the defaults.
    layout: Mutex<LayoutDoc>,
    /// Open and recently closed sessions. Shared with each session's exit callback.
    registry: Arc<Registry>,
    // Kept here so every handler reaches it through one State.
    sessions: Mutex<HashMap<String, Arc<crate::pty::Session>>>,
    /// Tabs a Resume or a Restart is starting a process for. One start per tab at a time, so two
    /// cannot both put a process in `sessions` and leave the first running unseen.
    starting: Mutex<HashSet<String>>,
    /// The sidebar's shell, while there is one. It is in `sessions` but has no registry row.
    shell_panel: Mutex<Option<shell_panel::PanelShell>>,
    /// Wakes the foreground watcher when a command starts, so it does not wait out its second.
    /// An `Arc` so a session's mark callback can hold it without holding the state.
    foreground_wake: Arc<tokio::sync::Notify>,
    /// Whether an agent's command (`claude`, `codex`, `agy`) runs from PATH. Tests swap it; without a swap it is looked up
    /// on the tab PATH (`AppState::agent_finder`).
    agent_on_path: Option<fn(&str) -> bool>,
    /// The file an agent's command runs from PATH, for its setup and version. Tests swap it; without a swap it is looked up
    /// on the tab PATH.
    agent_path: Option<fn(&str) -> Option<PathBuf>>,
    /// The PATH a session's program is looked up on (see `detect::tab_path`). Tests swap it.
    tab_path: fn() -> Option<std::ffi::OsString>,
    /// Opens "Open in system terminal"'s console (`outside::open_console`). Tests swap it, so none opens a window.
    open_console: crate::outside::ConsoleOpener,
    /// The consoles that opener started and that still run, per conversation (see `outside::Consoles`).
    consoles: crate::outside::Consoles,
    /// `DEEPSEEK_API_KEY`, read when a DeepSeek session starts or the catalog is built. Tests swap it. The value goes
    /// only into that child's spawn environment.
    deepseek_key: fn() -> Option<String>,
    /// Runs an agent's status command (`claude auth status`, `codex login status`) and returns its exit code and output.
    /// Tests swap it, so none runs a real CLI.
    status_runner: StatusRunner,
    /// `~/.marshell/agents.json`, read again when the file's modified time changes. `AppState::agents` adds the shipped
    /// manifests to the valid entries.
    own_agents: crate::agents::own::OwnCache,
    /// One save of `agents.json` at a time, so the changed-since-read check and the write are not interleaved.
    own_agents_write: Mutex<()>,
    /// `~/.marshell/tools.json`, read again when the file's modified time or length changes.
    own_tools: crate::tools::own::ToolsCache,
    /// One save of `tools.json` at a time, so the changed-since-read check and the write are not interleaved.
    own_tools_write: Mutex<()>,
    /// `--version` of the manifest agents' programs, for telling a program from another with the same name.
    agent_versions: Arc<crate::agents::identify::VersionCache>,
    /// The last events the bridge sent, per live tab.
    hook_log: crate::hooks::log::HookLog,
    /// The permission requests being held for a decision.
    holds: crate::hooks::holds::Holds,
    /// How long the core holds a permission request before it lets the agent ask.
    permission_hold: Duration,
    /// How long a Working agent can go without a hook event before it reads Stuck.
    stuck_quiet: Duration,
    /// How long to wait for Claude to resume after its last background subagent stops.
    subagent_wait: Duration,
    /// How long a Stop is held before the row reads Done.
    done_settle: Duration,
    /// Previews and applies changes to the CLIs' config files.
    config: crate::config::service::ConfigService,
    /// The sessions index and the transcript archive.
    history: Arc<crate::history::recorder::History>,
    /// Copies conversations run outside Marshell; started once, after the backfill.
    importer: std::sync::OnceLock<Arc<crate::history::import::Importer>>,
    /// Compresses finished recordings and keeps the history within its limits; started once, at startup.
    storage: std::sync::OnceLock<Arc<crate::history::storage::Storage>>,
    /// Claude Code's settings file when a test or a caller names one; None means
    /// `MARSHELL_CLAUDE_SETTINGS`, else `~/.claude/settings.json`.
    claude_settings: Option<PathBuf>,
    /// Codex's `hooks.json` when a test or a caller names one; None means `$CODEX_HOME`, else `~/.codex`.
    codex_hooks: Option<PathBuf>,
    /// The user home Antigravity's files are planned under when a test names one; None means the user's home.
    agy_home: Option<PathBuf>,
    /// The user home the `marshell` skill files are planned under when a test names one; None means the user's
    /// home, and `CLAUDE_CONFIG_DIR` from the environment. A named home also stops the environment from being read.
    skill_home: Option<PathBuf>,
    /// Home and managed settings paths for reading Claude's compact window; tests supply scratch paths.
    claude_compact_paths: Option<(PathBuf, Vec<PathBuf>)>,
    /// Where the app's own programs sit, so setting up hooks can put a missing bridge back. None in tests.
    programs_dir: Option<PathBuf>,
    /// `--version` of each agent executable with the file's modified time it was read at. A different
    /// time asks again, and a version that could not be read is not kept.
    versions: Mutex<HashMap<PathBuf, (Option<std::time::SystemTime>, String)>>,
    /// A Claude Code version a test pins, in place of asking the executable. `Some(None)` is a
    /// version that cannot be read.
    claude_version: Option<Option<String>>,
    /// A Codex version a test pins, in place of asking the executable. `Some(None)` cannot be read.
    codex_version: Option<Option<String>>,
    /// An Antigravity version a test pins, in place of asking the executable. `Some(None)` cannot be read.
    agy_version: Option<Option<String>>,
    /// The version of an agent inside a WSL distro that a test pins, in place of asking `wsl.exe`. `Some(None)` cannot
    /// be read.
    wsl_version: Option<Option<String>>,
    /// What `wsl.exe` last said of each `(distro lowercased, agent)`, and when. A version that could not be read is
    /// kept too, so a distro without the CLI is not asked on every hook.
    wsl_versions: Mutex<HashMap<(String, String), WslVersion>>,
    /// The target of a distro that a test pins, in place of asking `wsl.exe`. `Some(None)` is a distro that can't be
    /// reached.
    distro_target: Option<Option<DistroTarget>>,
    /// The previous process readings, so each `/v1/perf` call can tell the CPU used since the last one.
    sampler: Mutex<crate::diagnostics::perf::Sampler>,
    /// The diagnostics export waiting to be read and saved.
    previews: crate::diagnostics::export::Previews,
    /// What the UI last reported it shows, for the notifier to read.
    view: tokio::sync::watch::Sender<marshell_protocol::api::UiView>,
    notify_paused: Arc<AtomicBool>,
    window_hidden: Arc<AtomicBool>,
    focus: Option<FocusSink>,
}

/// How long a permission request is held when `MARSHELL_PERMISSION_HOLD_SECS` is not set.
pub const DEFAULT_PERMISSION_HOLD: Duration = Duration::from_secs(120);

/// How long a Working agent can go without a hook event before it reads Stuck, when `MARSHELL_STUCK_QUIET_SECS` is not set.
pub const DEFAULT_STUCK_QUIET: Duration = Duration::from_secs(600);

/// How long to wait for Claude to resume after its last background subagent stops.
pub const DEFAULT_SUBAGENT_WAIT: Duration = Duration::from_secs(30);

/// How long a Stop is held before the row reads Done, so a turn that resumes at once never shows it.
pub const DEFAULT_DONE_SETTLE: Duration = Duration::from_millis(1500);

impl AppState {
    pub fn new(token: String, port: u16, allowed_origins: Vec<String>, home: PathBuf, detected: Vec<Profile>) -> Self {
        let profiles = crate::profiles::store::load(&home, detected.clone());
        let settings = crate::settings::load(&home);
        let layout = crate::layout::load(&home);
        // The tabs of the last run come back as Ended rows. Nothing is started, and this runs before
        // the saver subscribes, so seeding never writes the file.
        let registry = Arc::new(Registry::new());
        registry.seed_ended(&layout.tabs, |tab| Relaunch {
            profile_id: tab.profile_id.clone(),
            agent: tab.agent.clone(),
            cwd: tab.cwd.clone(),
            mode: tab.mode,
            name: tab.name.clone(),
            preset: tab.preset.clone(),
        });
        let shell_dir = match crate::shell::install(&home) {
            Ok(dir) => Some(dir),
            Err(err) => {
                tracing::warn!("shell integration scripts were not written: {err:#}");
                None
            }
        };
        // The config guard reads the same profiles to know which WSL distros are still detected.
        let shared_profiles = Arc::new(Mutex::new(profiles));
        let config = crate::config::service::ConfigService::with_distros(home.clone(), {
            let profiles = shared_profiles.clone();
            Arc::new(move || wsl_distros(&profiles.lock().unwrap_or_else(|e| e.into_inner())))
        });
        let history = Arc::new(crate::history::recorder::History::open(&home).with_registry(registry.clone()));
        Self(Arc::new(Inner {
            token,
            host: format!("127.0.0.1:{port}"),
            allowed_origins,
            home,
            shell_dir,
            detected,
            profiles: shared_profiles,
            settings: Mutex::new(settings),
            layout: Mutex::new(layout),
            registry,
            sessions: Mutex::new(HashMap::new()),
            starting: Mutex::new(HashSet::new()),
            shell_panel: Mutex::new(None),
            foreground_wake: Arc::new(tokio::sync::Notify::new()),
            agent_on_path: None,
            agent_path: None,
            tab_path: crate::profiles::detect::tab_path,
            open_console: crate::outside::open_console,
            consoles: crate::outside::Consoles::default(),
            deepseek_key: || std::env::var(crate::agents::presets::DEEPSEEK_KEY_VAR).ok(),
            status_runner: |exe, args| crate::agents::run_capture(exe, args, std::time::Duration::from_secs(10)),
            own_agents: crate::agents::own::OwnCache::default(),
            own_agents_write: Mutex::new(()),
            own_tools: crate::tools::own::ToolsCache::default(),
            own_tools_write: Mutex::new(()),
            agent_versions: Arc::new(crate::agents::identify::VersionCache::default()),
            hook_log: crate::hooks::log::HookLog::default(),
            holds: crate::hooks::holds::Holds::default(),
            permission_hold: DEFAULT_PERMISSION_HOLD,
            stuck_quiet: DEFAULT_STUCK_QUIET,
            subagent_wait: DEFAULT_SUBAGENT_WAIT,
            done_settle: DEFAULT_DONE_SETTLE,
            config,
            history,
            importer: std::sync::OnceLock::new(),
            storage: std::sync::OnceLock::new(),
            claude_settings: None,
            codex_hooks: None,
            agy_home: None,
            skill_home: None,
            claude_compact_paths: None,
            programs_dir: None,
            versions: Mutex::new(HashMap::new()),
            claude_version: None,
            codex_version: None,
            agy_version: None,
            wsl_version: None,
            wsl_versions: Mutex::new(HashMap::new()),
            distro_target: None,
            sampler: Mutex::new(crate::diagnostics::perf::Sampler::new()),
            previews: crate::diagnostics::export::Previews::default(),
            view: tokio::sync::watch::channel(marshell_protocol::api::UiView {
                window_focused: false,
                visible_tabs: Vec::new(),
            })
            .0,
            notify_paused: Arc::default(),
            window_hidden: Arc::default(),
            focus: None,
        }))
    }
    /// Names where the focus route sends a tab, in place of answering 503. Call it before the state is cloned.
    pub fn with_focus(mut self, sink: FocusSink) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_focus runs before the state is shared")
            .focus = Some(sink);
        self
    }
    pub(crate) fn focus_sink(&self) -> Option<FocusSink> {
        self.0.focus.clone()
    }
    /// Shares the supervisor's flags with this core, before the state is cloned.
    pub(crate) fn with_notification_flags(mut self, paused: Arc<AtomicBool>, hidden: Arc<AtomicBool>) -> Self {
        let inner = Arc::get_mut(&mut self.0).expect("with_notification_flags runs before the state is shared");
        inner.notify_paused = paused;
        inner.window_hidden = hidden;
        self
    }
    /// Names Claude Code's settings file, in place of the environment. Call it before the state is cloned.
    pub fn with_claude_settings(mut self, path: PathBuf) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_claude_settings runs before the state is shared")
            .claude_settings = Some(path);
        self
    }
    /// Names Codex's `hooks.json`, in place of the environment. Its `config.toml` is the file beside it. Call it
    /// before the state is cloned.
    pub fn with_codex_hooks(mut self, path: PathBuf) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_codex_hooks runs before the state is shared")
            .codex_hooks = Some(path);
        self
    }
    /// Names the user home under which Antigravity's `.gemini/config/hooks.json` and
    /// `.gemini/antigravity-cli/settings.json` are planned, in place of the environment. Call it before the state
    /// is cloned.
    pub fn with_agy_home(mut self, user_home: PathBuf) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_agy_home runs before the state is shared")
            .agy_home = Some(user_home);
        self
    }
    /// Names the user home under which the `marshell` skill files of all three CLIs are planned, in place of the
    /// environment. The process environment is not read either, so a `CLAUDE_CONFIG_DIR` of the developer's cannot
    /// move Claude Code's skill out of it. Call it before the state is cloned.
    pub fn with_skill_home(mut self, user_home: PathBuf) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_skill_home runs before the state is shared")
            .skill_home = Some(user_home);
        self
    }
    /// Names the user home and managed files used for Claude's compact window, before state is shared.
    pub fn with_claude_compact_paths(mut self, user_home: PathBuf, managed: Vec<PathBuf>) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_claude_compact_paths runs before the state is shared")
            .claude_compact_paths = Some((user_home, managed));
        self
    }
    /// Names the folder holding the app's own programs. Call it before the state is cloned.
    pub fn with_programs_dir(mut self, dir: PathBuf) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_programs_dir runs before the state is shared")
            .programs_dir = Some(dir);
        self
    }
    /// Pins the Claude Code version that the live status gate and the setup route see, so a test never runs a
    /// real `claude`. `None` is a version that cannot be read. Call it before the state is cloned.
    pub fn with_claude_version(mut self, version: Option<String>) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_claude_version runs before the state is shared")
            .claude_version = Some(version);
        self
    }
    /// Pins the Codex version the setup route sees, so a test never runs a real `codex`. `None` is a version that
    /// cannot be read. Call it before the state is cloned.
    pub fn with_codex_version(mut self, version: Option<String>) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_codex_version runs before the state is shared")
            .codex_version = Some(version);
        self
    }
    /// Pins the Antigravity version live status sees, so a test never runs a real `agy`. `None` is a version that
    /// cannot be read. Call it before the state is cloned.
    pub fn with_agy_version(mut self, version: Option<String>) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_agy_version runs before the state is shared")
            .agy_version = Some(version);
        self
    }
    /// Pins the version of an agent inside a WSL distro that live status sees, so a test never runs `wsl.exe`. `None` is
    /// a version that cannot be read. Call it before the state is cloned.
    pub fn with_wsl_version(mut self, version: Option<String>) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_wsl_version runs before the state is shared")
            .wsl_version = Some(version);
        self
    }
    /// Pins where hooks go in a WSL distro, so a test never runs `wsl.exe`. `None` is a distro that can't be reached.
    /// Call it before the state is cloned.
    pub fn with_distro_target(mut self, target: Option<DistroTarget>) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_distro_target runs before the state is shared")
            .distro_target = Some(target);
        self
    }
    /// Sets how long a permission request is held. Call it before the state is cloned.
    pub fn with_permission_hold(mut self, hold: Duration) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_permission_hold runs before the state is shared")
            .permission_hold = hold;
        self
    }
    pub fn permission_hold(&self) -> Duration {
        self.0.permission_hold
    }
    /// Sets the hook silence threshold. Call it before the state is cloned.
    pub fn with_stuck_quiet(mut self, quiet: Duration) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_stuck_quiet runs before the state is shared")
            .stuck_quiet = quiet;
        self
    }
    pub fn stuck_quiet(&self) -> Duration {
        self.0.stuck_quiet
    }
    /// Sets how long to wait after the last subagent stops. Call it before the state is cloned.
    pub fn with_subagent_wait(mut self, d: Duration) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_subagent_wait runs before the state is shared")
            .subagent_wait = d;
        self
    }
    pub fn subagent_wait(&self) -> Duration {
        self.0.subagent_wait
    }
    /// Sets how long a Stop is held before the row reads Done. Call it before the state is cloned.
    pub fn with_done_settle(mut self, d: Duration) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_done_settle runs before the state is shared")
            .done_settle = d;
        self
    }
    pub fn done_settle(&self) -> Duration {
        self.0.done_settle
    }
    pub fn holds(&self) -> &crate::hooks::holds::Holds {
        &self.0.holds
    }
    /// The cards of the held requests, oldest first. Each was built when its hold opened.
    pub fn permission_views(&self) -> Vec<marshell_protocol::hook::PermissionView> {
        self.0.holds.views()
    }
    /// Replaces the PATH check for agent commands. Call it before the state is cloned.
    pub fn with_agent_check(mut self, check: fn(&str) -> bool) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_agent_check runs before the state is shared")
            .agent_on_path = Some(check);
        self
    }
    /// Replaces the console "Open in system terminal" opens. Call it before the state is cloned.
    pub fn with_console(mut self, open: crate::outside::ConsoleOpener) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_console runs before the state is shared")
            .open_console = open;
        self
    }
    pub(crate) fn console_opener(&self) -> crate::outside::ConsoleOpener {
        self.0.open_console
    }
    /// Replaces the PATH lookup of an agent's file. Call it before the state is cloned.
    pub fn with_agent_path(mut self, find: fn(&str) -> Option<PathBuf>) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_agent_path runs before the state is shared")
            .agent_path = Some(find);
        self
    }
    /// How to find an agent's program: on the PATH a tab of that profile gets. Take one per request and use it for every
    /// command: the PATH is read once, and only when a lookup needs it. `profile_env` is the launching profile's own
    /// environment, whose PATH wins; the catalog has no profile and passes none.
    pub(crate) fn agent_finder(&self, profile_env: &[(String, String)]) -> AgentFinder {
        AgentFinder {
            path_of: self.0.agent_path,
            on_path: self.0.agent_on_path,
            profile_path: profile_env
                .iter()
                .rev()
                .find(|(name, _)| name.eq_ignore_ascii_case("PATH"))
                .map(|(_, value)| value.into()),
            read_tab_path: self.0.tab_path,
            tab_path: std::sync::OnceLock::new(),
        }
    }
    /// Replaces the PATH sessions are looked up on. Call it before the state is cloned.
    pub fn with_tab_path(mut self, read: fn() -> Option<std::ffi::OsString>) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_tab_path runs before the state is shared")
            .tab_path = read;
        self
    }
    /// Replaces the read of `DEEPSEEK_API_KEY`. Call it before the state is cloned.
    pub fn with_deepseek_key(mut self, read: fn() -> Option<String>) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_deepseek_key runs before the state is shared")
            .deepseek_key = read;
        self
    }
    /// Replaces the run of an agent's status command. Call it before the state is cloned.
    pub fn with_status_runner(mut self, run: StatusRunner) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_status_runner runs before the state is shared")
            .status_runner = run;
        self
    }
    /// Replaces the `--version` reader the manifest agents' programs are checked with. Call it before the state is cloned.
    pub fn with_agent_versions(mut self, versions: crate::agents::identify::VersionCache) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_agent_versions runs before the state is shared")
            .agent_versions = Arc::new(versions);
        self
    }
    /// The adapters' launch data, the shipped manifests and the valid own agents of `agents.json`. A saved or hand-edited
    /// file shows without a restart: the file's modified time is checked on each call.
    pub(crate) fn agents(&self) -> crate::agents::manifest::Agents {
        self.0.own_agents.get(&self.0.home).agents.clone()
    }
    /// The shipped tool profiles and the valid own ones of `tools.json`. A hand-edited file shows without a restart: the
    /// file's modified time and length are checked on each call.
    pub fn tools(&self) -> Arc<crate::tools::Tools> {
        self.0.own_tools.get(&self.0.home).tools.clone()
    }
    /// `tools` for a callback that outlives the call, such as a session's classifier. It holds a weak handle, not the
    /// state: the state owns the sessions that own the callback. It reads the cache at each call, so an edited
    /// `tools.json` applies to the next command. If the state is gone it answers with the shipped profiles.
    pub(crate) fn tools_reader(&self) -> impl Fn() -> Arc<crate::tools::Tools> + Send + Sync + 'static {
        let inner = Arc::downgrade(&self.0);
        move || match inner.upgrade() {
            Some(inner) => AppState(inner).tools(),
            None => Arc::new(crate::tools::Tools::shipped()),
        }
    }
    /// The sessions index and the transcript archive.
    pub fn history(&self) -> Arc<crate::history::recorder::History> {
        self.0.history.clone()
    }
    /// The storage thread. None until `start_storage`, and when history is off for this run.
    pub fn storage(&self) -> Option<Arc<crate::history::storage::Storage>> {
        self.0.storage.get().cloned()
    }
    /// Starts the storage thread. Does nothing when history is off for this run, or when it is already running.
    pub fn start_storage(&self) {
        self.start_storage_with(crate::history::storage::StorageTiming::default());
    }

    /// Like `start_storage`, with the given timing (tests shorten it).
    pub fn start_storage_with(&self, timing: crate::history::storage::StorageTiming) {
        if !self.0.history.is_on() {
            return;
        }
        // Weak, so the thread's source of what is in use doesn't keep the state alive.
        let weak = Arc::downgrade(&self.0);
        let open: crate::history::storage::OpenNow = Arc::new(move || {
            weak.upgrade()
                .map(|inner| {
                    let state = AppState(inner);
                    // The sidebar's tabs, Ended ones restored from the last run included, and the sessions
                    // (the sidebar's shell has no registry row). Each lock is released before the next.
                    let mut tabs: HashSet<String> = state.0.sessions.lock().unwrap().keys().cloned().collect();
                    tabs.extend(state.registry().saved_tabs().into_iter().map(|tab| tab.tab_id));
                    crate::history::storage::Open {
                        recordings: state.open_recordings(),
                        tabs,
                    }
                })
                .unwrap_or_default()
        });
        self.0.storage.get_or_init(|| {
            let home = self.0.history.home().to_path_buf();
            crate::history::storage::Storage::start_with(home, self.0.history.clone(), open, timing)
        });
    }
    /// Starts the importer on `roots`, with the switch from the settings. Does nothing when history is off for
    /// this run, or when one is already running.
    pub fn start_importer(&self, roots: crate::history::import::ImportRoots) {
        self.start_importer_with(roots, crate::history::import::Timing::default());
    }

    /// Like `start_importer`, with the given timing (tests shorten it).
    pub fn start_importer_with(
        &self,
        roots: crate::history::import::ImportRoots,
        timing: crate::history::import::Timing,
    ) {
        if !self.0.history.is_on() {
            return;
        }
        let on = self.settings_snapshot().history_import;
        self.0
            .importer
            .get_or_init(|| crate::history::import::Importer::start_with(self.0.history.clone(), roots, on, timing));
    }

    /// What `GET /v1/history/import` says. Without an importer: the switch from the settings and no work.
    pub fn import_status(&self) -> marshell_protocol::history::ImportStatus {
        match self.0.importer.get() {
            Some(importer) => importer.status(),
            None => marshell_protocol::history::ImportStatus {
                on: self.settings_snapshot().history_import,
                copying: false,
                done: 0,
                total: 0,
            },
        }
    }

    /// The folder holding `profiles.json`, `tasks.json` and the other files Marshell keeps.
    pub fn home(&self) -> &std::path::Path {
        &self.0.home
    }
    /// What the UI last reported it shows. Starts as an unfocused window with no visible tab.
    pub fn view(&self) -> tokio::sync::watch::Receiver<marshell_protocol::api::UiView> {
        self.0.view.subscribe()
    }
    /// Records what the UI reports it shows.
    pub fn report_view(&self, view: marshell_protocol::api::UiView) {
        self.0.view.send_replace(view);
        // What the person is looking at in a focused window has had their attention.
        self.registry().clear_attention_seen(&self.effective_view());
    }
    pub fn notify_paused(&self) -> bool {
        self.0.notify_paused.load(Ordering::SeqCst)
    }
    /// The shell's visibility wins over a hidden webview's last report.
    pub fn set_window_hidden(&self, hidden: bool) {
        self.0.window_hidden.store(hidden, Ordering::SeqCst);
    }
    pub(crate) fn effective_view(&self) -> marshell_protocol::api::UiView {
        effective(&self.0.view.borrow(), self.0.window_hidden.load(Ordering::SeqCst))
    }
    /// `effective_view` for a callback that outlives the call. It holds the channel and the flag, not
    /// the state: the state owns the sessions that own the callback.
    pub(crate) fn view_reader(&self) -> impl Fn() -> marshell_protocol::api::UiView + Send + Sync + 'static {
        let view = self.0.view.subscribe();
        let hidden = self.0.window_hidden.clone();
        move || effective(&view.borrow(), hidden.load(Ordering::SeqCst))
    }
    /// The window lost focus, as the shell saw it. A webview that died or hung while focused can no longer say
    /// so, and its last report would otherwise keep every notification back.
    pub fn window_blurred(&self) {
        self.0
            .view
            .send_if_modified(|v| std::mem::replace(&mut v.window_focused, false));
    }
    /// The open and recently closed sessions.
    pub fn registry(&self) -> &Registry {
        &self.0.registry
    }
    pub fn config(&self) -> &crate::config::service::ConfigService {
        &self.0.config
    }
    pub fn hook_log(&self) -> &crate::hooks::log::HookLog {
        &self.0.hook_log
    }
    pub fn token(&self) -> &str {
        &self.0.token
    }
    pub fn host(&self) -> &str {
        &self.0.host
    }
    pub fn session(&self, tab: &str) -> Option<Arc<crate::pty::Session>> {
        self.0.sessions.lock().unwrap().get(tab).cloned()
    }
    /// Files still held by session recording writers. Release the map lock before asking each session.
    pub fn open_recordings(&self) -> HashSet<PathBuf> {
        let sessions: Vec<_> = self.0.sessions.lock().unwrap().values().cloned().collect();
        sessions.iter().filter_map(|s| s.open_recording()).collect()
    }
    /// How many sessions have a running process. Ended tabs have none.
    pub fn live_session_count(&self) -> usize {
        self.0.sessions.lock().unwrap().len()
    }
    /// (tab id, process id) of each session that has a process.
    pub(crate) fn session_pids(&self) -> Vec<(String, u32)> {
        let sessions = self.0.sessions.lock().unwrap();
        sessions
            .iter()
            .filter_map(|(tab, s)| s.pid().map(|pid| (tab.clone(), pid)))
            .collect()
    }
    pub(crate) fn sampler(&self) -> std::sync::MutexGuard<'_, crate::diagnostics::perf::Sampler> {
        self.0.sampler.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub(crate) fn previews(&self) -> &crate::diagnostics::export::Previews {
        &self.0.previews
    }
    /// A copy of the notification settings, from memory: the notifier reads them every tick.
    pub(crate) fn notify_settings(&self) -> marshell_protocol::api::NotifySettings {
        self.0
            .settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .notifications
            .clone()
    }
    /// A copy of the current settings.
    pub(crate) fn settings_snapshot(&self) -> Settings {
        self.0.settings.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    /// A copy of the current profiles: detected ones merged with `profiles.json`.
    pub(crate) fn profiles_snapshot(&self) -> ProfilesDoc {
        self.0.profiles.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    /// The WSL distro the tab's shell runs in. None for a tab of any other profile, or an unknown tab.
    pub fn wsl_distro_of_tab(&self, tab: &str) -> Option<String> {
        let profile_id = self.registry().profile_of_tab(tab)?;
        let profile = self
            .profiles_snapshot()
            .profiles
            .into_iter()
            .find(|p| p.id == profile_id)?;
        if !matches!(profile.family, marshell_protocol::api::ShellFamily::Wsl) {
            return None;
        }
        crate::wsl::distro_of_argv(&profile.command).or_else(crate::wsl::default_distro)
    }
    pub fn insert_session(&self, session: Arc<crate::pty::Session>) {
        self.0
            .sessions
            .lock()
            .unwrap()
            .insert(session.id().to_string(), session);
    }
    /// Claims the tab for one Resume or Restart. None while another one is starting it.
    /// The claim ends when the returned value is dropped.
    pub fn claim_start(&self, tab: &str) -> Option<StartClaim> {
        if !self.0.starting.lock().unwrap().insert(tab.to_string()) {
            return None;
        }
        Some(StartClaim {
            state: self.clone(),
            tab: tab.to_string(),
        })
    }
    /// Forgets the session so no new socket can attach; the caller kills it. Its held permission
    /// requests and hook events go too.
    pub fn remove_session(&self, tab: &str) -> Option<Arc<crate::pty::Session>> {
        let removed = self.0.sessions.lock().unwrap().remove(tab);
        self.holds().release_tab(tab);
        self.hook_log().forget_tab(tab);
        removed
    }
    pub fn origin_allowed(&self, origin: &str) -> bool {
        self.0.allowed_origins.iter().any(|o| o == origin)
    }
}

/// A tab's claim on starting its process, from `AppState::claim_start`. Dropping it ends the claim.
pub struct StartClaim {
    state: AppState,
    tab: String,
}

impl Drop for StartClaim {
    fn drop(&mut self) {
        self.state.0.starting.lock().unwrap().remove(&self.tab);
    }
}

/// Finds an agent's program on the PATH its tab will get. A test's swap (`with_agent_path`, `with_agent_check`) answers
/// instead when it is set.
pub(crate) struct AgentFinder {
    path_of: Option<fn(&str) -> Option<PathBuf>>,
    on_path: Option<fn(&str) -> bool>,
    profile_path: Option<std::ffi::OsString>,
    read_tab_path: fn() -> Option<std::ffi::OsString>,
    tab_path: std::sync::OnceLock<Option<std::ffi::OsString>>,
}

impl AgentFinder {
    fn path_in_use(&self) -> Option<&std::ffi::OsStr> {
        self.profile_path
            .as_deref()
            .or_else(|| self.tab_path.get_or_init(self.read_tab_path).as_deref())
    }

    /// The file `command` runs from.
    pub(crate) fn path_of(&self, command: &str) -> Option<PathBuf> {
        match self.path_of {
            Some(swap) => swap(command),
            None => crate::profiles::detect::command_path_in(self.path_in_use()?, command),
        }
    }

    /// Whether `command` runs at all.
    pub(crate) fn on_path(&self, command: &str) -> bool {
        match self.on_path {
            Some(swap) => swap(command),
            None => self
                .path_in_use()
                .is_some_and(|path| crate::profiles::detect::command_path_in(path, command).is_some()),
        }
    }
}

pub fn router(state: AppState) -> Router {
    // Here and not in `AppState::new`, which `with_agent_check` needs to be the only owner of.
    layout::spawn_layout_saver(&state);
    foreground::spawn_watcher(&state);
    ports::spawn_watcher(&state);
    activity::spawn_watcher(&state);
    let origins: Vec<HeaderValue> = state.0.allowed_origins.iter().filter_map(|o| o.parse().ok()).collect();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        // The webview is another origin and reads this from a content reply.
        .expose_headers([HeaderName::from_static("x-modified-ms")]);
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/profiles", get(profiles::get).put(profiles::put))
        .route("/v1/profiles/detected", get(profiles::detected))
        .route("/v1/profiles/integration", post(profiles::integration))
        .route("/v1/settings", get(settings::get).put(settings::put))
        .route("/v1/notify/paused", get(get_notify_paused).put(put_notify_paused))
        .route("/v1/layout", get(layout::get))
        .route("/v1/layout/view", put(layout::put_view))
        .route("/v1/import/windows-terminal", get(settings::windows_terminal))
        .route("/v1/projects", get(projects::get))
        .route("/v1/folders/check", get(folders::check))
        .route("/v1/references", get(references::get).put(references::put))
        .route("/v1/scripts", get(scripts::get))
        .route("/v1/scripts/allow", post(scripts::allow))
        .route("/v1/scripts/run", post(scripts::run))
        .route("/v1/tasks", get(tasks::list).post(tasks::create))
        .route("/v1/tasks/plan", post(tasks::plan))
        .route("/v1/tasks/{id}/changes", get(tasks::changes))
        .route("/v1/tasks/{id}/merge/plan", post(tasks::merge_plan))
        .route("/v1/tasks/{id}/merge", post(tasks::merge))
        .route("/v1/tasks/{id}/remove/plan", post(tasks::remove_plan))
        .route("/v1/tasks/{id}/remove", post(tasks::remove))
        .route("/v1/sessions", get(sessions::list).post(sessions::create))
        .route("/v1/sessions/reorder", post(sessions::reorder))
        .route("/v1/sessions/reopen", post(sessions::reopen))
        .route("/v1/sessions/{tab}/resume", post(sessions::resume))
        .route("/v1/sessions/{tab}/bookmark", post(history::bookmark_now))
        .route("/v1/sessions/{tab}/restart", post(sessions::restart))
        .route("/v1/sessions/{tab}/muted", put(sessions::muted))
        .route(
            "/v1/ui/view",
            put(view::put).layer(DefaultBodyLimit::max(view::MAX_BODY * 4)),
        )
        .route("/v1/sessions/{tab}/pause", post(sessions::pause))
        .route("/v1/sessions/{tab}/continue", post(sessions::cont))
        .route("/v1/sessions/{tab}/input", post(sessions::input))
        .route("/v1/sessions/{tab}/fill", post(fill::fill))
        .route("/v1/sessions/{tab}/focus", post(sessions::focus))
        .route("/v1/sessions/{tab}/resume-command", get(outside::tab_resume_command))
        .route("/v1/sessions/{tab}/open-outside", post(outside::tab_open_outside))
        .route("/v1/sessions/{tab}/recording", get(sessions::recording))
        .route("/v1/sessions/{tab}/peek", get(sessions::peek))
        .route("/v1/sessions/{tab}/resolve", post(files::resolve))
        .route("/v1/sessions/{tab}/files", get(files::list))
        .route(
            "/v1/sessions/{tab}/attachments",
            post(attachments::save).layer(DefaultBodyLimit::max(files::IMAGE_CAP as usize + 1)),
        )
        .route("/v1/sessions/{tab}/paste", post(attachments::paste))
        .route("/v1/sessions/{tab}/paste-paths", post(attachments::paste_paths))
        .route("/v1/files/stat", get(files::stat))
        .route("/v1/files/content", get(files::content))
        .route("/v1/sessions/{tab}", patch(sessions::rename).delete(sessions::close))
        .route("/v1/shell-panel", post(shell_panel::open))
        .route("/v1/shell-panel/adopt", post(shell_panel::adopt))
        .route(
            "/v1/hook",
            post(hook::post).layer(DefaultBodyLimit::max(marshell_protocol::hook::HOOK_BODY_LIMIT)),
        )
        .route(
            "/v1/diagnostics/logging",
            get(diagnostics::get_logging).put(diagnostics::put_logging),
        )
        .route("/v1/diagnostics/export", post(diagnostics::post_export))
        .route("/v1/diagnostics/export/{id}/file", get(diagnostics::get_export_file))
        .route("/v1/diagnostics/export/{id}/zip", get(diagnostics::get_export_zip))
        .route("/v1/perf", get(diagnostics::get_perf))
        .route("/v1/hook/events", get(hook::events))
        .route("/v1/permissions", get(permissions::list))
        .route("/v1/permissions/{rid}/decision", post(permissions::decision))
        .route("/v1/permissions/{rid}/release", post(permissions::release))
        .route("/v1/config/file", get(config::file))
        .route("/v1/config/preview", post(config::preview))
        .route("/v1/config/preview/{id}", get(config::get_preview))
        .route("/v1/config/apply", post(config::apply))
        .route("/v1/snapshots", get(config::snapshots))
        .route("/v1/snapshots/{id}/restore", post(config::restore))
        .route("/v1/agents", get(agents::found))
        .route("/v1/agents/catalog", get(catalog::catalog))
        .route("/v1/agents/own", get(catalog::get_own).put(catalog::put_own))
        .route("/v1/tools", get(tools::catalog))
        .route("/v1/tools/own", get(tools::get_own).put(tools::put_own))
        .route("/v1/agents/{agent}/setup", get(agents::setup))
        .route("/v1/agents/{agent}/install", get(agents::install))
        .route("/v1/agents/{agent}/account", get(agents::account))
        .route("/v1/agents/{agent}/settings-view", get(agents::settings_view))
        .route("/v1/agents/{agent}/settings/plan", post(agents::settings_plan))
        .route("/v1/agents/{agent}/settings/undo", post(agents::settings_undo))
        .route("/v1/agents/{agent}/hooks/plan", post(agents::plan))
        .route("/v1/agents/{agent}/hooks/remove-plan", post(agents::remove_plan))
        .route("/v1/agents/{agent}/skill/plan", post(agents::skill_plan))
        .route("/v1/agents/{agent}/skill/remove-plan", post(agents::skill_remove_plan))
        .route("/v1/history", get(history::list))
        .route("/v1/history/import", get(history::import_status))
        .route("/v1/history/storage", get(history::storage_report))
        .route("/v1/history/storage/preview", post(history::storage_preview))
        .route("/v1/history/search", get(history::search))
        .route("/v1/history/bookmarks", get(history::bookmarks))
        .route(
            "/v1/history/{agent}/{session_id}",
            get(history::conversation)
                .patch(history::patch)
                .delete(history::delete_copy),
        )
        .route(
            "/v1/history/{agent}/{session_id}/bookmarks/{turn}",
            put(history::bookmark_turn).delete(history::unbookmark_turn),
        )
        .route("/v1/history/{agent}/{session_id}/resume", post(history::resume))
        .route("/v1/history/{agent}/{session_id}/fork", post(history::fork))
        .route(
            "/v1/history/{agent}/{session_id}/resume-command",
            get(outside::history_resume_command),
        )
        .route(
            "/v1/history/{agent}/{session_id}/open-outside",
            post(outside::history_open_outside),
        )
        .route("/v1/history/{agent}/{session_id}/export", get(history::export))
        .route("/v1/history/{agent}/{session_id}/recordings", get(history::recordings))
        .route(
            "/v1/history/{agent}/{session_id}/recordings/{part}/frames",
            get(history::recording_frames),
        )
        .route("/v1/events", get(events::handler))
        .route("/v1/pty/{tab}", get(pty_ws::handler))
        .layer(axum::middleware::from_fn_with_state(state.clone(), guard::auth_guard))
        .layer(cors)
        // Host is checked before CORS can answer a preflight.
        .layer(axum::middleware::from_fn_with_state(state.clone(), guard::host_guard))
        // Outside both guards and CORS, so even rejected requests have their final status logged.
        .layer(axum::middleware::from_fn(log_request))
        .with_state(state)
}

async fn log_request(req: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    let method = req.method().clone();
    // Never retain the full URI, headers or body: they can contain tokens and user content.
    let path = req.uri().path().to_owned();
    let started = std::time::Instant::now();
    let response = next.run(req).await;
    tracing::debug!(
        %method,
        %path,
        status = response.status().as_u16(),
        duration_ms = started.elapsed().as_secs_f64() * 1000.0,
        "API request"
    );
    response
}

#[derive(serde::Deserialize, serde::Serialize)]
struct NotifyPaused {
    paused: bool,
}

async fn get_notify_paused(axum::extract::State(st): axum::extract::State<AppState>) -> Json<NotifyPaused> {
    Json(NotifyPaused {
        paused: st.notify_paused(),
    })
}

async fn put_notify_paused(
    axum::extract::State(st): axum::extract::State<AppState>,
    Json(body): Json<NotifyPaused>,
) -> axum::http::StatusCode {
    st.0.notify_paused.store(body.paused, Ordering::SeqCst);
    axum::http::StatusCode::NO_CONTENT
}

async fn health() -> Json<Health> {
    Json(Health {
        app: brand::APP_NAME.into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

pub struct RunningCore {
    pub endpoint: Endpoint,
    pub state: AppState,
    /// Where `start` wrote the endpoint file, so `shutdown` can delete it.
    endpoint_file: PathBuf,
    shutdown: tokio::sync::oneshot::Sender<()>,
    serve: tokio::task::JoinHandle<()>,
    /// The notifier's task, when the config named a sink. Ended with the core.
    notifier: Option<tokio::task::JoinHandle<()>>,
    /// The stuck check's tick task. Ended with the core.
    stuck_tick: tokio::task::JoinHandle<()>,
}

impl RunningCore {
    /// False once the server task has stopped, or when a panic left the sessions lock poisoned.
    pub fn is_healthy(&self) -> bool {
        !self.serve.is_finished() && !self.state.0.sessions.is_poisoned()
    }

    /// Kills every session's process tree, deletes the endpoint file (unless a newer core has replaced it),
    /// then stops the server. Works on a poisoned lock too. Done here and not in a task, because the
    /// process exits right after this returns.
    pub fn shutdown(self) {
        let sessions: Vec<_> = self
            .state
            .0
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect();
        for s in sessions {
            s.force_kill();
        }
        // First, so it starts no new file. It is not waited for: the file it is copying is registered with
        // History, so the flush below waits for that copy, for 3 s at most; the flush's hurry also makes that copy skip its reindex.
        if let Some(importer) = self.state.0.importer.get() {
            importer.halt();
        }
        if let Some(storage) = self.state.0.storage.get() {
            storage.halt();
        }
        // After the kill, so Claude has stopped writing: a copy still waiting to settle runs now, and a reply
        // given just before closing is kept. It returns at once when nothing is pending.
        self.state.history().flush(Duration::from_secs(3));
        auth::remove_endpoint_file_if_ours(&self.endpoint_file, &self.endpoint.token);
        if let Some(notifier) = &self.notifier {
            notifier.abort();
        }
        self.stuck_tick.abort();
        let _ = self.shutdown.send(());
    }

    /// Panics in another thread while it holds the sessions lock, so the lock is poisoned.
    #[cfg(test)]
    pub(crate) fn poison_for_test(&self) {
        std::thread::scope(|s| {
            let _ = s
                .spawn(|| {
                    let _held = self.state.0.sessions.lock();
                    panic!("poisoning the sessions lock for a test");
                })
                .join();
        });
    }

    /// Stops the server task as a crash would; the sessions keep running.
    #[cfg(test)]
    pub(crate) fn stop_server_for_test(&self) {
        self.serve.abort();
    }
}

/// `MARSHELL_PERMISSION_HOLD_SECS` clamped to 1..=580 s, or the default when unset or not a number. The cap sits
/// below the bridge's 590 s `Permission` budget (`crates/hook/src/post.rs`), so the core's answer, a deny for
/// Antigravity, always arrives before the bridge gives up and prints nothing.
fn permission_hold_from_env(value: Option<&str>) -> Duration {
    match value.and_then(|v| v.trim().parse::<u64>().ok()) {
        Some(secs) => Duration::from_secs(secs.clamp(1, 580)),
        None => DEFAULT_PERMISSION_HOLD,
    }
}

/// `MARSHELL_STUCK_QUIET_SECS` clamped to 5..=3600 s, or the default when unset or not a number.
fn stuck_quiet_from_env(value: Option<&str>) -> Duration {
    match value.and_then(|v| v.trim().parse::<u64>().ok()) {
        Some(secs) => Duration::from_secs(secs.clamp(5, 3600)),
        None => DEFAULT_STUCK_QUIET,
    }
}

/// Binds 127.0.0.1 on a random port, writes `run/endpoint.json`, serves in the background.
pub async fn start(cfg: CoreConfig) -> anyhow::Result<RunningCore> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let endpoint = Endpoint {
        port,
        token: auth::new_token(),
        platform: Platform::current(),
    };
    // So no log line, export or error message can carry this launch's token.
    crate::diagnostics::redact::global().remember_token(&endpoint.token);
    // Looking for shells runs programs (wsl.exe can take up to 2 s): keep it off the async threads.
    let detected = tokio::task::spawn_blocking(|| detect_profiles(&Probe::system())).await?;
    // Before anything is written under the home. A failure is not fatal: the default home sits under the
    // user's profile, which is already private to the user. The first fix may walk the whole home, so it
    // runs off the async threads.
    let home = cfg.home.clone();
    match tokio::task::spawn_blocking(move || private::secure_home(&home)).await? {
        Ok(private::HomeAcl::Fixed) => tracing::info!("made the Marshell home owner-only"),
        Ok(private::HomeAcl::AlreadyPrivate) => {}
        Err(e) => tracing::warn!("could not make the Marshell home owner-only: {e:#}"),
    }
    let endpoint_file = paths::endpoint_file(&cfg.home);
    let mut state = AppState::new(endpoint.token.clone(), port, cfg.allowed_origins, cfg.home, detected)
        .with_notification_flags(cfg.notify_paused, cfg.window_hidden)
        .with_permission_hold(permission_hold_from_env(
            std::env::var("MARSHELL_PERMISSION_HOLD_SECS").ok().as_deref(),
        ))
        .with_stuck_quiet(stuck_quiet_from_env(
            std::env::var("MARSHELL_STUCK_QUIET_SECS").ok().as_deref(),
        ));
    if let Some(dir) = cfg.programs_dir {
        state = state.with_programs_dir(dir);
    }
    if let Some(sink) = cfg.focus {
        state = state.with_focus(sink);
    }
    // Last, so the endpoint is only advertised once the state is ready to serve.
    auth::write_endpoint_file(&endpoint_file, &endpoint)?;
    let app = router(state.clone());
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let serve = tokio::spawn(async move {
        let serve = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = rx.await;
        });
        if let Err(e) = serve.await {
            tracing::error!("core server stopped: {e}");
        }
    });
    // The server is listening: index the conversations archived before search existed, one at a time off the
    // async threads. Dropping the handle lets it run on its own.
    drop(state.history().backfill());
    if let Some(roots) = cfg.import {
        state.start_importer(roots);
    }
    state.start_storage();
    let notifier = cfg.notify.map(|sink| crate::notify::spawn(state.clone(), sink));
    let stuck_state = state.clone();
    let stuck_tick = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        let quiet_ms = stuck_state.stuck_quiet().as_millis() as u64;
        loop {
            interval.tick().await;
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            stuck_state.registry().check_stuck(now_ms, quiet_ms);
        }
    });
    tracing::info!(app_version = env!("CARGO_PKG_VERSION"), port, "core started");
    Ok(RunningCore {
        endpoint,
        state,
        endpoint_file,
        shutdown: tx,
        serve,
        notifier,
        stuck_tick,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stuck_quiet_env_is_clamped() {
        assert_eq!(stuck_quiet_from_env(None), Duration::from_secs(600));
        assert_eq!(stuck_quiet_from_env(Some("60")), Duration::from_secs(60));
        assert_eq!(stuck_quiet_from_env(Some("1")), Duration::from_secs(5));
        assert_eq!(stuck_quiet_from_env(Some("99999")), Duration::from_secs(3600));
        assert_eq!(stuck_quiet_from_env(Some("x")), Duration::from_secs(600));
    }

    #[test]
    fn permission_hold_env_is_clamped() {
        assert_eq!(permission_hold_from_env(None), DEFAULT_PERMISSION_HOLD);
        assert_eq!(permission_hold_from_env(Some("abc")), DEFAULT_PERMISSION_HOLD);
        assert_eq!(permission_hold_from_env(Some("0")), Duration::from_secs(1));
        assert_eq!(permission_hold_from_env(Some("45")), Duration::from_secs(45));
        assert_eq!(permission_hold_from_env(Some("9999")), Duration::from_secs(580));
    }

    #[test]
    fn the_hold_clamp_ends_before_the_bridge_budget() {
        // The bridge's `Permission` budget (`crates/hook/src/post.rs`, `budget`). The core can't import the
        // hook crate, so it is written out here; change both together.
        const BRIDGE_PERMISSION_BUDGET: Duration = Duration::from_secs(590);
        let longest = permission_hold_from_env(Some("9999"));
        assert_eq!(longest, Duration::from_secs(580));
        assert!(longest < BRIDGE_PERMISSION_BUDGET);
        assert!(DEFAULT_PERMISSION_HOLD < longest);
    }

    #[test]
    fn a_wsl_tab_names_its_distro_and_no_other_tab_does() {
        use marshell_protocol::api::{Profile, ShellFamily};
        let profile = |id: &str, command: &[&str], family| Profile {
            id: id.into(),
            name: id.into(),
            command: command.iter().map(|s| s.to_string()).collect(),
            family,
            cwd: None,
            env: Vec::new(),
            shell_integration: true,
            terminal_theme: None,
            detected: true,
        };
        let home = tempfile::tempdir().unwrap();
        let st = AppState::new(
            "t".into(),
            1,
            Vec::new(),
            home.path().into(),
            vec![
                profile("wsl-my", &["wsl.exe", "-d", "My Distro"], ShellFamily::Wsl),
                profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell),
            ],
        );
        for (tab, profile_id) in [("a", "wsl-my"), ("b", "pwsh"), ("c", "gone")] {
            st.registry().reserve(
                tab,
                crate::registry::Relaunch {
                    profile_id: profile_id.into(),
                    agent: None,
                    cwd: "C:/work".into(),
                    mode: None,
                    name: tab.into(),
                    preset: None,
                },
            );
        }
        assert_eq!(st.wsl_distro_of_tab("a").as_deref(), Some("My Distro"));
        assert_eq!(st.wsl_distro_of_tab("b"), None);
        assert_eq!(st.wsl_distro_of_tab("c"), None);
        assert_eq!(st.wsl_distro_of_tab("missing"), None);
    }
}
