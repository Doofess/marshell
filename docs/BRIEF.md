# Plan-mode brief: Airport (now Marshell)

Oct 9, 2026

## Amendments (2026-10-09, from the approved plan in `docs/PLAN.md`)

Where the text below disagrees with this list, this list wins. `docs/PLAN.md` holds the details and sources.

- **Name.** The app is renamed **Marshell**, because Apple holds a live US AIRPORT trademark for software. Every name comes from one constants module:
  - `marshell` binary and CLI
  - `~/.marshell/` config dir
  - `MARSHELL_` env prefix
  - `marshell-hook` bridge
  - The Departures board keeps its name.
- **Icon.** The icon is two marshalling batons raised in a V, replacing the plane. The app icon is "Night shift": a charcoal tile, grey grips and orange lights. Its source is `assets/icon/app-night-shift.svg`.
- **Phase 0 decisions (2026-10-10):**
  - The default accent ("needs you" attention colour) is a signal amber, not the icon's orange and not Claude orange, so a Claude row's brand stripe never reads as an alert.
  - Mockups are Storybook stories in the repo, plus a published private preview for review.
  - Sign-off happens in three batches:
    1. tokens, glyphs, rows
    2. approve card, main window, launcher
    3. onboarding, departures, doctor, settings, notifications
- **Decisions:**
  - pnpm
  - MIT open source
  - approve-from-anywhere on by default
  - project references off until enabled per project
  - keep all history, with the backup-folder mirror off
  - capture Claude's native recap (best effort)
  - soft original chimes
  - install Codex and Gemini before phase 6
- **Gemini CLI:**
  - Hooks are switched by `hooksConfig.enabled` (on by default), not `hooks.enabled`.
  - It has no permission-grant hook, so its approve card focuses the tab.
  - Google replaced Gemini CLI with Antigravity CLI for free and Google One users on 2026-06-18. Decide the adapter target before phase 6.
- **Codex CLI:**
  - It has a `PermissionRequest` hook, so approve-from-anywhere works for Codex too.
  - Hooks must be trusted by hash in `/hooks` before they run.
  - `AGENTS.md` has no imports, so the instruction library inlines a tagged block (32 KiB limit).
- **Claude Code:**
  - `model` arrives only on `SessionStart`.
  - `cleanupPeriodDays` defaults to 30.
  - Session tokens are summed from transcript `message.usage`, deduped by `message.id`.
- **Shortcuts.**
  - App shortcuts are Cmd+key on macOS and Ctrl+Shift+key on Windows and Linux, because Ctrl+H/K/B/F/\ and Ctrl+/ are terminal control keys.
  - The needs-you lane is ordered oldest-waiting first.
- **Stack:**
  - xterm 6 removed the canvas renderer, so the fallback is the DOM renderer.
  - tauri-plugin-store is dropped; the core owns settings.
  - The config service core moves forward to phase 3.
  - The elevated helper ships in phase 6, with a separate elevated window until then.
- **UX.** The "UX and UI design" section of `docs/PLAN.md` is binding for phase 0 and every UI PR.

## How to use this

Everything from section 1 down is the prompt. Paste it whole into Claude Code once it is in plan mode.

Before pasting, fill in the placeholders in section 8: your main development OS and whether the repo is empty. The prompt is written for an empty folder.

It is written as plain text on purpose, with no diagrams, so it pastes cleanly into a terminal. If you would rather keep it in the repo, save this file as `docs/BRIEF.md` and tell Claude Code to read it. That also lets you reuse it in later sessions.

Treat it as a living spec. When the plan changes a decision, change it here too.

## 1. Role and goal

You are the lead engineer on a new desktop app. You are in plan mode: explore, ask me questions, and produce an implementation plan. Do not write code until I approve the plan.

Name: **Airport**, because it is the place with terminals. Use `airport` for the app binary and the config directory (`~/.airport/`), `AIRPORT_` as the prefix for env vars, and `airport-hook` for the hook bridge binary.

What we are building: a desktop terminal session manager for AI coding-agent CLIs (Claude Code, Codex CLI, Gemini CLI, and any other CLI). Each session is a real terminal tab running an agent. A side pane lists every session with live status, a one-line recap of what the agent is doing, its model, its effort level, how many subagents are running, and how full its context is. The app also gives one place to see everything connected to each agent (skills, MCP servers, hooks, plugins, instruction files, settings), a doctor that finds broken or wasteful config and fixes it, OS notifications when an agent finishes or needs me, and friendly editors for the config and markdown files.

The goal is to take the CLI agent coding experience to the next level: run several agents in parallel and only look when one needs you. UX and UI are the top priority. The app must feel polished and fast, not like a wrapper around a terminal.

About me: I know TypeScript and have shipped Electron apps. I am new to Rust. Keep the Rust surface small and simple.

## 2. Non-negotiables

1. **Windows, macOS and Linux are all first-class, from day one.** Every feature must work on all three. That means ConPTY on Windows and forkpty on macOS/Linux; PowerShell, cmd, Git Bash and WSL on Windows and zsh/bash/fish elsewhere; native notifications on each OS; WebView2 on Windows and WebKit on macOS/Linux. CI builds and tests all three. Call out every platform difference in the plan.
2. **Multi-CLI by design, not Claude-only.** All agent-specific code lives in adapters behind one interface. Claude Code is the first adapter, Codex and Gemini follow, and any other CLI or plain shell works with reduced status from the terminal stream alone.
3. **Never touch my config silently.** Show the exact change before making it, back the file up, tag every entry the app adds, install at user level (not project level), and make uninstall leave zero trace. Never modify managed or organization-level settings. Never run sudo, admin elevation, or security changes (like PowerShell execution policy) without showing the command and asking.
4. **UX first.** Design tokens and the key screens come before UI code. Keyboard-first, accessible, and status is never conveyed by color alone.
5. **Zero token cost by default.** Status, basic recaps, the doctor, the settings pane, the editors and history must work without calling any model. Model-backed features are opt-in and run through the user's own CLI in headless mode.
6. **Nothing is ever lost.** Airport keeps its own archive of every conversation and terminal recording, independent of each CLI's retention and cleanup, and never deletes the CLI's own files.
7. **Brand colors and text labels only.** Never use other companies' logos.

## 3. Tech stack (decided; challenge it in the plan if you find a blocker)

- **Base: Tauri 2 with a Rust backend.** Reasons: small installer, fast startup, and Rust fits the backend jobs (ptys, a local server, config parsing). Conductor, one of the most polished apps in this category, is built on Tauri with a Rust backend, which is reassuring for macOS and Windows. The known risk is Linux: WebKitGTK is the only webview Tauri has there, and terminal projects report it holds back repaints while the window is idle (output painted only after a keystroke) plus dead-key input quirks on both WebKitGTK and WKWebView. So the window shell is kept swappable (see protocol-first core) and phase 1 has a go/no-go gate. If the gate fails without a clean fix, switch the shell to Electron and keep everything else.
- **Protocol-first core.** The Rust core exposes a localhost HTTP and WebSocket API (`axum`) and the UI talks to it only through that API: pty input and output over WebSocket, everything else over HTTP, a per-launch token on both, bound to 127.0.0.1 only. In v1 the core runs as a thread inside the app process. v2 can move it to a separate `airportd` process so sessions survive UI restarts and crashes. Tauri itself is used only for the window, notifications, tray, badge and dialogs. Precedent: TermFlow and Baton, both Tauri terminals that keep throughput out of the webview hot path.
- **Rust style:** structs and enums, `Result` with `?`, `serde`, `Arc<Mutex<...>>` for shared state, `tokio` for async. Avoid heavy trait and lifetime gymnastics. I am learning Rust through this project.
- **Rust crates (verify current versions):** `portable-pty` for ptys on all three OSes, `axum` with its WebSocket feature for the core server, `serde` and `serde_json` with `preserve_order`, `toml_edit` for TOML edits that keep comments, `notify` for file watching, `rodio` for sound playback, `tokio`.
- **Tauri plugins:** notification, store, single-instance, window-state, opener. Verify current APIs.
- **Frontend:** React and TypeScript with Vite. `@xterm/xterm` with the fit, WebGL, search and web-links addons (fall back to the canvas renderer where WebGL is unavailable). CodeMirror 6 for editors. Accessible component primitives such as Radix, with CSS variables for design tokens. Keep state management simple. Config files are never written from the frontend; the Rust config service is the only write path.
- **Transport:** WebSocket for pty streams, HTTP for commands and queries, a second WebSocket for status updates. Tauri events only for OS-level things. Generate shared TypeScript types from the Rust types (`ts-rs` or `specta`; your pick).
- **Session persistence:** live processes do not survive an app restart in v1. Restore the layout and relaunch each agent with its CLI's resume feature (for example `claude --resume <id>`). The `airportd` daemon in v2 makes sessions survive restarts; design the protocol so that move is a packaging change, not a rewrite.
- **Repo:** a single repo with `crates/core` (server, pty manager, adapters, config service), `crates/hook` (the bridge binary), `src-tauri/` (the thin window shell) and `src/` (the frontend).

## 4. Architecture

### Rust core modules

1. **PTY manager.** Spawns each session through a shell profile (for example `pwsh -NoExit -Command claude` or `zsh -lc claude`), keeps a map of tab id to process, forwards input and resize events, and streams output to the frontend over the pty WebSocket. It sets `AIRPORT_TAB_ID` and `AIRPORT_AGENT` env vars on every spawn. Keep the hot path in Rust: coalesce output (flush every 8 ms or at 64 KB), apply backpressure (stop reading the pty when more than 1 MiB is unacknowledged by the renderer), and keep a scrollback ring buffer per session that can replay on reconnect and feed previews. It also parses escape sequences in the output stream: OSC 7 (current directory), OSC 133 (command start and end), OSC 9 and OSC 777 (terminal notifications), and BEL.
2. **Hook bridge.** A tiny binary (`airport-hook`, or an `airport --hook` subcommand) that every CLI's hooks call. The app copies it to a stable path, `~/.airport/bin/airport-hook`, on launch and keeps it current, so settings never point at an app bundle path that changes with versions. It reads the event JSON on stdin and the tab id from the env, adds which agent sent it, and POSTs to the core server. It must exit fast and never block the agent: on any failure, exit 0 with no output. Two special roles: as Claude Code's status line command in chaining mode (forward the JSON, then run the user's original status line command and print its output), and as a `PermissionRequest` hook (forward the request, wait for the user's decision from the app up to the hook timeout, return `decision.behavior`; if no decision arrives, exit 0 with no output so the normal terminal prompt applies).
3. **Core server.** `axum` on 127.0.0.1 with a random port and a per-launch token written to a known file, so other local processes cannot spoof events. Serves the hook endpoint, the pty WebSocket, the status WebSocket, and the command API. Raw hook events go to the right adapter to normalize, then update session state and fan out to the UI.
4. **Adapter registry.** One adapter per CLI (see below), plus a generic adapter configured by data for custom agents.
5. **Config service.** The only code path that writes config and instruction files: scope resolution, snapshots, atomic writes (temp file plus rename), and watching for outside changes. JSON through `serde_json` with `preserve_order` (verify whether any CLI's settings file allows comments; if so, use a formatting-preserving editor for that file). TOML through `toml_edit`. Shared by the settings pane, the doctor, the editors and hook installation.
6. **Recap worker, doctor engine, notification service, installer runner** (installer is v2).

### Adapter interface (sketch; refine it in the plan)

- `id`, display name, default brand color
- `detect()`: installed, version, binary path
- `launch_command(profile, cwd, options)`: options include permission mode, model, effort, worktree and first prompt where the CLI supports them
- `install_hooks()`, `uninstall_hooks()`, `hooks_installed()`: tagged entries at user level
- `normalize(raw_event) -> Vec<SessionEvent>`
- `permission_decision(request, decision)`: how to return an approve or deny for CLIs that support it
- `context_injection()`: which hook carries `additionalContext` per prompt and per session start, used by project references
- `config_sources()`: instruction files, settings files, skills, MCP servers, hooks, plugins, by scope
- `doctor_checks()`: adapter-specific checks
- `headless_summarize(transcript)`: the command for an opt-in smart recap, with that CLI's hooks disabled for the call
- `usage_source()`: where context and plan usage come from
- `history_sources()`: where the CLI stores transcripts and how to watch that location
- `parse_transcript(path)`: into the common conversation model used by History
- `resume_command(session_id)` and `fork_command(session_id)` where supported

### Shared event model

The UI never sees raw CLI events, only these: `SessionStarted { model? }`, `PromptSubmitted`, `Working`, `ToolUse { name, target? }`, `NeedsInput { kind: permission | question | elicitation | idle }`, `SubagentStarted { id }`, `SubagentStopped { id }`, `TurnDone`, `TurnFailed { reason }`, `ModelChanged { model }`, `EffortChanged { level }`, `PermissionModeChanged { mode }`, `ContextUsage { tokens_in_context, window_size, pct }`, `SessionUsage { input_tokens, output_tokens, cache_read, cache_write, cost_usd? }`, `PlanUsage { five_hour_pct, seven_day_pct, resets_at }`, `CwdChanged { path }`, `CommandStarted`, `CommandFinished { exit_code }`, `SessionEnded`.

Session state per tab: agent, name, cwd, project (folder name, repo name and git branch, derived by the app from the cwd so it works for every tab including plain shells), status, `model: Option`, `effort: Option`, `permission_mode: Option` (Claude Code: default, plan, acceptEdits, auto, dontAsk, bypassPermissions; other CLIs map their approval modes onto a shared set of manual, plan, auto-edit and full-auto, with the CLI's own label kept for the tooltip), `subagents: HashSet<id>` (a set of ids, never a counter, so one missed stop event cannot leave it off by one), `context: Option<{ tokens, window_size, pct }>`, `session_tokens: Option<{ input, output, cache_read, cache_write }>`, `cost_usd: Option`, recap text and its age, last event time, needs-attention flag, muted flag. Unknown values display as a dash, never a guess.

### Claude Code adapter (first; verify everything against the current docs before coding)

- Hooks: command hooks receive JSON on stdin with `session_id`, `transcript_path`, `cwd`, `permission_mode`, `effort.level`, `hook_event_name`, plus `agent_id` and `agent_type` inside subagents. HTTP hooks also exist, but use the command bridge for consistency with the other CLIs. Events to use: `SessionStart` (has `model` and `source`), `UserPromptSubmit`, `PreToolUse` and `PostToolUse` (`tool_name`, `tool_input`), `Notification` (matchers `permission_prompt`, `idle_prompt`, `elicitation_dialog`, `agent_needs_input`), `SubagentStart` and `SubagentStop` (`agent_id`), `Stop` (`last_assistant_message`), `StopFailure` (`rate_limit`, `overloaded`, and others), `PostModelSwitch` (`from_model`, `to_model`), `SessionEnd`.
- Permission mode: `permission_mode` arrives as a common field on most hook events (`default`, `plan`, `acceptEdits`, `auto`, `dontAsk`, `bypassPermissions`; the mode labeled Manual arrives as `default`). There is no dedicated mode-changed event, so update the mode from whichever event arrives next. The status line re-runs when the mode changes, which is a cheap trigger to refresh.
- Status line JSON: `model.display_name`, `effort.level`, `session_name`, `workspace.project_dir`, `workspace.repo.name` and `owner`, `workspace.git_worktree`, `context_window.context_window_size` (200000 by default, 1000000 for extended-context models), `context_window.total_input_tokens` and `total_output_tokens` (tokens currently in the context window from the last API response, not a running total), `context_window.used_percentage`, `cost.total_cost_usd` (estimated at list price), `cost.total_lines_added` and `removed`, `rate_limits.five_hour` and `seven_day` (`used_percentage`, `resets_at`; Pro and Max subscribers only, after the first API response). A separate `subagentStatusLine` setting receives a `tasks` array with status, model, effort and token count per subagent.
- Session tokens so far: the status line has no running total, so sum the per-message `usage` entries in the transcript JSONL (input, output, cache read, cache write) and refresh on each `Stop`. Verify the transcript format first.
- Headless: `claude -p --model <small model> --settings '{"disableAllHooks": true}'` for smart recaps. Disabling hooks prevents a feedback loop into our own server and also suppresses the status line.
- Native recap: Claude Code prints a line starting with `Recap:` after about three minutes unfocused (with at least three turns) and on `/recap`. Capturing it from the pty stream is optional and the format is not a stable API.
- Config locations: `~/.claude/settings.json` (user), `.claude/settings.json` (project), `.claude/settings.local.json` (local); hooks also come from plugins and from skill and subagent frontmatter; skills in `~/.claude/skills` and `.claude/skills`; instruction files `CLAUDE.md` and `.claude/rules/*.md`. List MCP servers with `claude mcp list` rather than parsing files, because file locations change between versions.

### Codex CLI and Gemini CLI adapters (verify current docs)

- Codex: hooks are generally available (May 2026). Config in `~/.codex/hooks.json` or a `[hooks]` table in `~/.codex/config.toml`. Events include `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop` and more. Project-level hooks go through a trust model, so install at user level. Instruction file is `AGENTS.md`.
- Gemini: hooks live in `~/.gemini/settings.json` under `hooks` and must be enabled with `"hooks": { "enabled": true }`. Events: `SessionStart`, `SessionEnd`, `BeforeAgent`, `AfterAgent`, `BeforeModel`, `AfterModel`, `BeforeTool`, `AfterTool`. Project hooks are fingerprinted, so install at user level. Instruction file is `GEMINI.md`. Gemini has no effort setting; show a dash.
- Any CLI without hooks: terminal heuristics only. No output for a few seconds means idle or waiting; BEL, OSC 9 or OSC 777 means it wants attention; OSC 133 gives command state for plain shells.

## 5. Features

### 5.1 Session sidebar (the core)

- Each row: a thin brand stripe or dot (defaults: Claude orange, Gemini purple, OpenAI light yellow, generic gray; editable per agent and per session), the session name (custom title, else the project name), the project and git branch ("my-app · feat/auth"), a status indicator, and a context ring.
- Status indicators use shape and motion, never color alone: working = gently pulsing dot, done = solid check, needs you = a distinct badge (the only element allowed to grab attention), error = its own icon.
- Second line (on hover or when expanded, with compact and comfortable density settings): model, effort, permission mode, and subagent count. Plan mode gets its own pill. Modes that act without asking (`auto`, `dontAsk`, `bypassPermissions`) get a small caution mark so they are always visible. Third line: the one-line recap.
- Expanded details and the ring tooltip: context as "84k / 200k (42%)", session tokens used so far as "312k in · 48k out" with cache reads and writes in the tooltip, and the estimated cost where the CLI reports one.
- Fixed order that the user can drag. No auto-sorting; rows must not jump while the user reaches for them. Add a "jump to next session that needs me" shortcut instead. `Ctrl+1` to `9` switch sessions. Optional grouping by project.
- Per-session mute. Collapsible sidebar and a focus mode.
- Show a dash for anything unknown; never guess a model, mode or effort.

### 5.2 Live status and recap

- Live status comes from events: "Editing src/auth.ts", "Running npm test", "Waiting for permission", "Needs an answer".
- Recap has three tiers. (a) Event-built, free, the default: compose one line from the last events, such as "Edited 4 files in src/auth, ran tests, waiting for you". (b) Smart recap, opt-in: on `TurnDone`, run the adapter's headless summarize command with hooks disabled, using a small model, at most once per turn, debounced. (c) Native recap capture: when Claude Code prints its own `Recap:` line, show it. Optional and best-effort.
- Show how old the recap is.

### 5.3 Settings pane

- Per agent and per scope (user, project, local, plugin, managed): skills, subagents, custom commands, MCP servers with connection status (via the CLI's own listing command), hooks with their source, plugins, instruction files, environment variables (values masked).
- Read-only in its first version. Editing arrives with the editors in 5.6. Entries the app itself manages (our hooks, the status line chain) are shown as locked.

### 5.4 Doctor

- Runs entirely locally with zero tokens, in the background, never blocking the UI. Each adapter contributes checks. Also run each CLI's own doctor command (for example `claude doctor`) and fold its output in.
- **Broken:** hook script missing or not executable (the CLI treats this as a non-blocking error, so a safety hook can silently stop working), MCP server failing to connect or auth expired, config that does not parse, skill frontmatter missing or malformed, the same MCP server defined in two scopes, outdated CLI version, hooks configured in a CLI whose hook engine is off (Gemini), our status line chain broken, a CLI sending its own desktop notifications in addition to ours.
- **Waste:** MCP servers and skills unused in the last 30 days (scan transcripts: MCP tool calls carry the server name, skill use appears as Skill tool calls), token cost estimate per server (tool definition characters divided by 4, adjusted for whether that CLI loads tool definitions lazily, so the number is not exaggerated), skill descriptions that are too long (suggest shortening rather than removing), scope too broad (a user-level server used in one project only).
- **Info:** everything else worth knowing.
- Report UI: three groups with checkboxes. Broken items are pre-checked; waste items are unchecked because usage stats can mislead. A button at the bottom reads "Fix X issues" where X counts only checked, auto-fixable items. Items that need the user (re-login) open a terminal tab and are not counted.
- Fix rules: disable, never delete. Snapshot every touched file before fixing. "Undo last fix" is one click. Show which running sessions must restart to pick up the change. Managed and organization settings are reported, never touched.
- Fix actions are a small closed set: disable an MCP server, remove a hook entry, apply a config edit, run a command in a tab.

### 5.5 Notifications

- Four categories mapped from shared events: needs you (`NeedsInput`), session done (`TurnDone`), subagent done (`SubagentStopped`), error (`TurnFailed`). Session done and subagent done must have different sounds.
- Send OS banners silent and play our own sounds from Rust with `rodio`, because OS notification sounds are inconsistent across platforms. One sound per category, per-category volume and on/off, royalty-free or original sound files.
- Rules: no notification when the app is focused and that tab is active; batch events within a few seconds ("3 sessions need you"); subagent sounds are subtle and separately toggleable; escalate an unacknowledged needs-you once after about a minute (Claude's `idle_prompt` is a natural trigger); quiet hours, and respect OS do-not-disturb for our own sounds too; per-session mute.
- Clicking a banner focuses the app and jumps to that session. Verify click handling on each OS early; fall back to per-OS crates if the Tauri plugin falls short.
- Badge with the needs-you count on the dock and taskbar icon; taskbar flash on Windows.
- Fallback for CLIs without hooks: BEL, OSC 9 and OSC 777 from the pty stream.
- The doctor offers to disable each CLI's own desktop notifications so nothing arrives twice.

### 5.6 Editors (instruction files and settings)

- Instruction files: a tree by scope covering `CLAUDE.md`, `.claude/rules`, `AGENTS.md`, `GEMINI.md`, skills, subagents and commands. CodeMirror markdown editor with live preview. Frontmatter shown as a validated form above the body, reusing doctor checks. A token estimate per file ("~1.2k tokens, loaded every session"). A "what loads here" view: pick a folder and see the merged, ordered list of instruction files a session there would load. Templates for a new skill, subagent, or command.
- Settings: form mode (toggles, dropdowns, lists) and raw mode (JSON or TOML with schema validation; Claude Code's settings have a published JSON schema, verify on SchemaStore). Show the effective value with a scope badge such as "set in project settings, overrides your user setting". Friendly builders for permission rules (tool picker plus pattern field), hooks (event, matcher, command), and environment variables (masked). App-managed entries are locked.
- Shared safety layer: minimal edits that preserve formatting and key order, all through the Rust config service (`serde_json with preserve_order` for JSON, `toml_edit` for TOML), validate then write atomically (temp file plus rename), snapshot with a diff and undo (the same system the doctor uses), watch for outside changes while a file is open and offer a merge instead of overwriting, and say whether a change applies live or needs a session restart.
- Optional, opt-in "Improve this file" that runs the user's own CLI headless and shows a diff to accept or reject.

**Instruction library (many .md files, at user and project level):**

- **A set of named docs per scope, not one file.** Besides `CLAUDE.md`, `AGENTS.md` and `GEMINI.md`, manage `SECURITY.md`, `SCOPE.md`, `CONVENTIONS.md`, `ARCHITECTURE.md`, `TESTING.md` and any custom name. Two scopes: user level (applies to every project; lives in `~/.airport/instructions/`) and project level (lives in the repo and is committed, so a team shares it). Each doc has a description, an enabled toggle per project, an agent filter (all CLIs or only some), and optional path globs so it loads only when the agent touches matching files.
- **Write once, wired into every CLI.** Airport keeps each doc once and wires it into each CLI's own loading mechanism, tagging everything it writes: Claude Code through `.claude/rules/<name>.md` with `paths:` frontmatter for conditional docs, and the user-level rules directory for user docs; Codex through a managed import block in `AGENTS.md`; Gemini through `@` imports in `GEMINI.md`. Verify each CLI's current rules and import mechanism before wiring. Use copies with a content hash rather than symlinks (symlinks on Windows need developer mode), and have the doctor flag drift between the library and the wired copies.
- **Templates and profiles.** Starter templates for each common doc: `SECURITY.md` covers secrets handling, destructive commands and dependency policy; `SCOPE.md` covers what the agent may and may not touch; `TESTING.md` covers how to run and write tests. A project profile bundles docs, settings and hooks so a new project is set up in one click.
- **Budget and precedence.** The "what loads here" view lists user docs, project docs and conditional docs in load order, with a token count per doc and a total, and warns when the always-loaded total passes a threshold (default 4k tokens). Every doc keeps snapshot history with restore, the same as settings.

### 5.7 Terminal and shell settings

- Layer 1, app-owned terminal settings: font family, size and weight, ligatures, theme, cursor style and blink, padding, background opacity where the OS supports it, scrollback, copy on select, paste warnings, bell behavior. Global defaults plus per-profile overrides; an optional brand theme per agent.
- Layer 2, shell profiles: auto-detect installed shells (Windows: PowerShell 7, Windows PowerShell, cmd, Git Bash, WSL distros; macOS and Linux: `/etc/shells`). A profile is shell, args, starting directory, env vars, name, color and icon. Agents always launch through a profile so they get the user's normal PATH and the tab stays usable after the agent exits.
- Profile settings page: one page per profile with sections Appearance, Behavior, Shell and Startup, so a user changes PowerShell's look and feel from the app without touching files. The app-managed init script is shown and editable there.
- Layer 3, the shell's own settings: never edit `$PROFILE`, `.bashrc` or `.zshrc` directly. Each session runs a small app-owned init script that loads the user's profile first, then applies app settings on top. For PowerShell: PSReadLine options (prediction view, edit mode, colors), prompt style with Starship or oh-my-posh integration, module auto-load. For bash, zsh and fish: aliases, prompt and completion settings. The same script enables shell integration (OSC 7 for cwd, OSC 133 for command state). A "make permanent" action writes the change into the user's own profile through the show, back up, tag and undo flow. Security settings such as execution policy are shown read-only with a confirm-to-run command.
- Elevated sessions: a launcher option that starts a profile with administrator rights. Windows: launch a small `airport-host --elevated` helper through ShellExecute with the `runas` verb, which shows the UAC prompt; the helper owns the elevated ConPTY and connects back to the core over the localhost protocol with the per-launch token, so the tab renders like any other. Windows integrity levels prevent a non-elevated process from hosting an elevated pty directly, which is why the helper exists. The helper accepts only pty input and resize from the core, nothing else, because a lower-integrity process steering an elevated one is an escalation surface; if the spike shows it cannot be made safe, fall back to Windows Terminal's behavior and open elevated sessions in a separate elevated Airport window. macOS: `sudo -i` in the session, with the password or Touch ID prompt in the terminal. Linux: `pkexec` or `sudo -i`. Elevated tabs show a shield badge and an "Administrator" label in the header for the whole session, start in manual permission mode, and an agent in an elevated session cannot be switched to bypass mode without an extra confirmation. Elevation is never remembered; every elevated session prompts again.

### 5.8 Usage rings

- A context ring on each session row showing how full its context is, with the tokens and window size in the tooltip ("84k / 200k"). Claude Code: the `context_window` fields from the status line. Others: computed from session logs and the model's window size. Hide the ring when unknown rather than showing a fake 0%.
- Session tokens used so far are a number, not a ring, because there is no ceiling to fill against. Show them in the expanded row and in a per-session details panel, with cost where available.
- A plan-usage ring in the app header showing the 5-hour limit, with the 7-day limit and both reset times in a tooltip. Claude Pro and Max only; hidden when absent.
- Green under 70%, yellow 70 to 89%, red at 90% and above, always with the number next to it. An SVG circle driven by `stroke-dasharray` is enough.

### 5.9 Installer (v2)

- A catalog as data (JSON or TOML, updatable without an app release): per tool and per OS, the official install command, version check, update check, and login command.
- Every install runs in a visible terminal tab. Show the exact command before running it. Curated catalog and official sources only. No silent elevation. Run the version check afterwards, then offer the login flow. The version check also powers "update available" in the settings pane.

### 5.10 History (nothing is ever lost)

- **Three layers captured for every session, automatically.** (a) The CLI's own structured transcript, copied incrementally into Airport's archive (`~/.airport/history/`, compressed) so it survives the CLI's own cleanup. Claude Code deletes old transcripts after a retention period by default (verify the current setting and its default), so Airport's copy is the durable one. (b) A raw terminal recording in asciicast v2 format, written from the pty stream with timestamps. (c) Airport's own event stream and recaps. Airport never modifies or deletes the CLI's files.
- **Sessions started outside Airport are indexed too.** Watch each CLI's transcript directory and import sessions run from a plain terminal, so history stays complete even when the user does not use the app.
- **Index:** SQLite (`rusqlite`, bundled) with FTS5 full-text search over prompts, assistant text, tool calls and file paths. Fields: agent, project, branch, title, started, ended, duration, turns, tokens, cost, model, outcome (done, failed, abandoned), tags, pinned, bookmarks, continuation chain.
- **Continuation chains:** compaction, `/clear`, resume and fork create new session ids in the CLIs. Link them (Claude Code: `SessionStart` with `source` resume, fork or compact, plus `PreCompact` and `PostCompact`) so one task reads as one thread.
- **History view (`Ctrl+H`):** a list grouped by project, newest first, with filters (agent, project, branch, date, model, outcome, tag) and search (`Ctrl+Shift+F` from anywhere; results show highlighted snippets and jump to that turn). A live session and its history entry are the same object; ending a session only makes it past.
- **Conversation view:** render the transcript as a readable conversation, not raw terminal text: user prompts, assistant text as markdown, tool calls collapsed into cards ("Edited src/auth.ts, 12 added, 3 removed", "Ran npm test, passed"), subagent runs as collapsible sections, compaction points marked, and a timeline scrubber. A second tab replays the terminal recording with a player. A third tab lists the files changed.
- **Actions:** Resume (opens a new tab with the CLI's resume command; the main way to go back), Fork where the CLI supports it (`claude --resume <id> --fork-session`), Rename, Tag, Pin, Bookmark (`Ctrl+B` during a live session marks the current turn), Copy as markdown, Export (markdown, JSON, asciicast), Archive, and Delete Airport's copy (with confirmation; the CLI's own files are never deleted unless the user asks explicitly).
- **Titles:** use the CLI's session name or AI-generated title when present, else the first prompt shortened. The user can rename, and the name is kept across resumes.
- **Retention and backup:** keep everything by default; optional size or age limits the user sets; a backup folder option (any synced folder) that mirrors the archive; disk usage shown in settings. Exports can redact common secret patterns (API keys, tokens) with a toggle, because transcripts often contain them.
- The adapter additions this needs are listed in section 4.

### 5.11 Project references (an app-level skill for every agent)

- **Goal:** when the user says "the login button", "the pty manager" or "that migration from yesterday", every agent finds the right thing on the first try instead of searching the repo. It works for every CLI and model because Airport supplies it through the hook bridge, not the agent.
- **Project map:** for each repo Airport maintains a compact map (under 300 tokens): top-level layout, entry points, naming conventions, and an alias dictionary the user curates ("money app" = a repo path, "login button" = `src/components/auth/LoginButton.tsx`). Stored in `~/.airport/projects/<id>/`, optionally also committed as `.airport/references.md` so a team shares it.
- **Resolver at prompt time:** the bridge receives each prompt (`UserPromptSubmit` on Claude Code and Codex, `BeforeAgent` on Gemini), matches it against aliases, file and directory names, recent history and, in v2, a local symbol index built with tree-sitter or ctags. It returns `additionalContext` only when something matched, for example "References: 'login button' = src/components/auth/LoginButton.tsx. 'Yesterday's migration' = session 'Add JWT migration' (Oct 8), files: src/db/migrations/0012_jwt.sql." No match means no injection, so the token cost stays near zero. The doctor reports how many tokens the resolver injected.
- **Learns from history:** the History index knows which files each session touched and what the user called them. Airport suggests aliases for names it sees repeated ("you have said 'the pty manager' four times; map it to crates/core/src/pty.rs?") and resolves "the bug from yesterday" to a past session's recap and files.
- **Manual references:** typing `@` in a session opens Airport's picker (files, aliases, past sessions, branches) and inserts the concrete path or session id in the form that CLI understands (Claude Code already accepts `@file` references). This intercepts keystrokes, so keep it optional and move it to v2 if it proves fragile.
- **Rules:** never more than about 300 tokens per injection; plain factual statements, not instructions, so the CLI's prompt-injection defenses do not fire; a per-project toggle; the map is rebuilt in the background on file changes and never blocks a prompt for more than 100 ms.

## 6. UX and UI principles (treat these as requirements)

1. **The core loop is glance, jump, act.** The sidebar's one job is to answer "who needs me?" in under a second. Show little by default; details on hover or expand.
2. **Status never by color alone.** Brand colors are accents only; Claude's orange and OpenAI's yellow sit too close to warning amber. Status gets shapes and motion, which also makes the app usable for colorblind users.
3. **The terminal is the hero.** Minimal window chrome, collapsible sidebar, focus mode. A clean UI font for the app and a good monospace font for terminals, kept visibly separate.
4. **Keyboard-first, mouse-friendly.** A command palette (`Ctrl+K`, `Cmd+K` on macOS) that reaches every action: new session, switch session, run doctor, open a setting. Every mouse action has a shortcut, and shortcuts are discoverable.
5. **Every state is designed.** First run: detect installed CLIs, explain in plain words what the app will change (hooks, status line), ask permission, then start the first session. Empty state points to "start one". A missing CLI offers the installer; a broken hook links to the doctor. On restart, restore the layout and sessions exactly. Confirm before closing a session that is mid-task.
6. **Speed is part of UX.** Typing latency in the terminal is the first thing people notice: use the WebGL renderer, keep startup fast, never block the UI on file scans or the doctor, and measure input latency.
7. **Settings people do not fear.** Search across all settings, plain-language descriptions, and an always-visible undo. Raw JSON is available but never required.
8. **Consistency from day one.** Define design tokens first (light and dark colors, spacing, type scale, radii). Build on accessible primitives so keyboard navigation and screen reader labels come for free. Subtle, purposeful motion that respects the OS reduced-motion setting.
9. **Native feel on each OS.** Follow macOS, Windows and Linux conventions for menus, window controls, and modifier keys.

### Design philosophy and themes

- **Apple-like: simple, clean, content first.** The terminal is the content and the chrome recedes. Few type sizes, generous whitespace in the app chrome (dense inside the terminal), native-feeling controls, hairline separators, no decoration and no gradients. Every screen should look obvious at first glance.
- **Three themes: Light, Dark and System.** Dark is true black: `#000000` base surfaces for OLED screens, raised surfaces from `#0A0A0A` to `#141414`, hairline borders as white at 8 to 12 percent opacity, body text near `#E5E5E5` rather than pure white to avoid halation. Light uses white and warm grays. The terminal background follows the app theme by default and can be overridden per profile.
- **User-chosen accent color.** A preset palette of about eight accents plus a custom hex, applied to selection, focus rings, toggles, links, the active tab marker and the plan ring. Agent brand colors stay on session stripes and dots only, so the accent never fights them. Default to the OS accent where the OS exposes it. Check every pairing for WCAG AA contrast in both themes.
- **Platform fit.** Follow each OS for window controls, menus, modifier keys and title bar style: native title bar with traffic lights on macOS, a custom Fluent-like bar on Windows, sensible defaults on Linux.

### Signature moves (what makes Airport stand out)

1. **Approve from anywhere.** When a session asks for permission, show the request as a card with Allow, Deny and Allow always, in the sidebar, in the needs-you lane, and in the OS notification where the OS supports action buttons. For Claude Code, implement it through the `PermissionRequest` hook as described in section 4, so there is no keystroke injection. For CLIs without such a hook, the card focuses the tab instead. Handling five agents' prompts from one place is the single biggest win.
2. **Needs-you lane.** A small strip above the fixed session list showing only sessions waiting on you, newest first, each with its request or question text and the approve card. The main list never reorders. `Ctrl+Shift+N` jumps to the next one.
3. **Peek.** Hover a row, or press a key on it, to see a live read-only preview of that terminal's last 30 lines in a popover, rendered from the scrollback the core already holds. Check on a session without leaving the one you are typing in.
4. **Departures board.** A full-window overview mode (`Ctrl+Shift+D`) listing every session like a flight: name, project and branch, agent, status, waiting time, context fill, last recap. Split-flap style transitions on status changes, static when reduced motion is on. This is the screen for running ten agents, and the app's visual signature.
5. **Launcher.** One keyboard-driven screen to start a session: agent, project (recent folders with their git branch), permission mode (plan, manual, auto), model and effort presets such as Quick and Deep, an optional new git worktree (pass the CLI's own flag, for example `claude --worktree`), and an optional first prompt. Remembers the last choice per project.
6. **Command palette with broadcast.** `Ctrl+K` reaches every session, action and setting. Includes "send to": type a prompt once and send it to several sessions, for example "run the tests and report". Includes switch mode and set effort for the current session.
7. **Clickable paths and a diff drawer.** File paths in agent output (`src/auth.ts:42`) open in the user's editor (configurable: VS Code, Cursor, Zed, Vim). A per-session diff drawer shows files changed since your last prompt with plus and minus counts, built from edit events and `git status`; click a file for its diff.
8. **Attention choreography.** A status change flashes the row once with the accent color, then settles. The needs-you badge bounces once and stays. Done draws a check in 200 ms. Sounds are a small family built on the airport theme: a soft two-note chime for needs-you, a short arrival tone for done, a quiet click for subagent done, a low tone for error. The window title and the dock or taskbar badge carry the needs-you count. All of it respects reduced motion and quiet hours.
9. **Split view.** Two sessions side by side (`Ctrl+\`), for typing in one while watching another. More than two panes is v2.
10. **Escape hatches build trust.** Every session has "Open in system terminal" (runs the CLI's resume command) and "Copy resume command". Permission mode is always visible in the terminal header, and bypass and auto modes outline the header in red for the whole session.

Optional delight, default off: an airport mode that relabels statuses as Boarding, In flight, Holding, Landed and Diverted.

### Borrowed from the field

Study these before planning and take what fits. Superset is the closest competitor (any CLI agent in parallel worktrees, built-in terminal, diff viewer, in-app browser, automations, a CLI, free), Conductor is the most polished (macOS only, Tauri, review and merge, PR flow), Crystal is open source (Electron, per-iteration commits, git operations), and Claude Code's own desktop app runs parallel sessions with automatic worktrees. Airport's position is what none of them combine: real terminals for any CLI on Windows, macOS and Linux, the needs-you workflow with approve-from-anywhere, config management with a doctor, and terminal settings.

v1 borrowings (cheap, high leverage):

- **Repo scripts** (Superset, Conductor): an optional `airport.json` committed to the repo with `setup` (runs when a session or worktree starts, for example install dependencies and copy `.env`), `run` (dev server) and `teardown` scripts, shared with the team.
- **Custom agents** (Superset): add any terminal command as an agent with a name, color and launch args; it appears like a built-in. This is the data-configured generic adapter from section 4.
- **An `airport` CLI** (Superset): `airport new --agent claude --cwd . --prompt "..."`, `airport list`, `airport focus <id>`, `airport send <id> "..."`. Scripts and agents themselves can then start and steer sessions, and the user can drive the app from any terminal. Ship a small built-in skill so an agent knows how to use it.
- **Port chips** (Superset): detect ports opened by processes in a session, show `localhost:3000` chips in the header, click to open in the browser.
- **Stuck detection** (Codey): the same tool repeating many times, or no progress for several minutes while "working", raises a soft alert in the row rather than a notification.
- **Step timeline** (Codey): click a recap to expand this turn's tool calls with durations and, where the CLI reports it, token cost per step.
- **Saved prompts** (Warp notebooks): a small library of reusable prompts with variables, sent with one keystroke or broadcast.

v2 borrowings:

- **Review to merge** (Conductor, Crystal): diff viewer with per-file comments that can be sent back to the agent as a prompt, run checks, open a PR through `gh`, merge, then archive the session and its worktree.
- **Start from an issue** (Conductor): paste a GitHub or Linear issue URL to start a session with the issue text as the first prompt and a branch named after it.
- **Per-turn checkpoints** (Crystal): optional commit after each agent turn in worktree sessions, so a turn can be rolled back. Use the CLI's own checkpoint feature where it has one.
- **File tree and rendered markdown** (Claude Code desktop): a right drawer with the project tree and rendered markdown for files the agent wrote, such as plans.
- **Phone check-in** (Superset): a read-only departures board with approve and deny, served by the core over the LAN or a tunnel, with a proper security review first.

### Terminal essentials (table stakes, do not skip)

- Scrollback search (`Ctrl+F`), command markers from OSC 133 with jump between prompts, clickable links, copy on select and bracketed paste, ligatures, per-tab zoom, a smooth-scrolling toggle, cursor options, and at least 10k lines of scrollback held in Rust so it survives renderer reloads.
- Theme import from Windows Terminal, iTerm2 and VS Code theme files. App chrome follows the OS light or dark setting. A high-contrast mode.
- Inline rename with `F2`, drag to reorder, close with confirmation while working, reopen a closed session.
- A shortcut cheat sheet overlay on `Ctrl+/`.

### Information architecture and visual system

- Sidebar on the left: the needs-you lane, then the fixed session list, then a footer with the plan ring and the doctor status.
- Main area: the terminal with a slim header showing session name, project and branch, mode chip, model and effort, subagent count, context ring, port chips.
- Right drawer, toggled: recap timeline for this turn, diff drawer, session details (tokens, cost, resume command).
- Settings is a page, not a dialog: left navigation (General, Appearance, Terminal, Shells, Agents, Notifications, Doctor, Advanced) with search on top.
- Visual system: 8 px grid, 13 px UI type with 1.5 line height, sidebar rows 36 px compact and 48 px comfortable, one accent per agent, status by shape, 6 px radius, no gradients, motion 150 to 200 ms ease-out.
- Performance budget: typing latency under 30 ms end to end, 60 fps while an agent streams, sidebar updates batched at 100 ms. Include a hidden performance panel.
- Accessibility: a screen reader live region that announces "session X needs you", visible focus rings, a large-text mode, xterm's accessibility mode on request.

Process: before writing UI code, produce mockups of the main window with the sidebar, every sidebar row state, the doctor report, the settings editor, and first-run onboarding, as static pages or component stories, and get my approval on them.

### Brand and icon

The app is called Airport because that is where terminals are. The icon is a simple plane: one bold glyph on a rounded-square tile, flat colors with at most two tones, in the style of current terminal app icons such as the PowerShell 7 icon. It must read clearly at 16 px and 32 px, work on light and dark backgrounds, and have a monochrome variant for the menu bar and system tray. Draw an original mark and do not copy any existing app's icon. Deliver it as an SVG plus a 1024 px PNG, and run it through Tauri's icon generator to produce every platform size.

## 7. Phased roadmap (v1 is phases 1 to 8)

1. **Scaffold, one live tab, and the webview gate.** Tauri project, one pty tab working end to end on Windows, macOS and Linux through the core server's WebSocket, xterm with the fit and WebGL addons, design tokens, and the empty layout shell. The gate, on Linux (WebKitGTK) and macOS (WKWebView): (a) output paints while the window is idle with no keyboard input; (b) dead keys and IME input work; (c) `yes | head -n 2000000` and a 50 MB file streamed with `cat` do not stall the UI and typing stays responsive; (d) the WebGL renderer works, or the canvas fallback is acceptable. Any failure without a clean fix means switching the window shell to Electron and keeping the core and frontend. Exit criteria: typing feels instant, resize works, CI builds all three OSes.
2. **Multiple tabs and the sidebar.** Shell profile detection, the launcher (agent, project, mode, model and effort presets, worktree through the CLI's flag, elevated session), profile settings pages, themes and accent colors, split view, clickable paths, session persistence and restore, raw terminal recording to disk, command palette basics, brand colors.
3. **Hook bridge, core server and the Claude Code adapter.** Status indicators, event-built recap, status line chaining, pty heuristics fallback, the context ring and the plan ring, approve-from-anywhere through `PermissionRequest`, the needs-you lane, peek, transcript capture and the history index with Resume.
4. **Notifications and attention choreography.** Plus the departures board and stuck detection.
5. **History view.** Search, conversation view, recording replay, bookmarks, tags, export, and import of sessions run outside Airport.
6. **Settings pane (read-only), the Codex and Gemini adapters, custom agents, repo scripts, port chips, the `airport` CLI, and project references.**
7. **Doctor** with the Fix button, snapshots and undo.
8. **Editors** for the instruction library and settings on the same safety layer, opt-in smart recaps and native recap capture, the diff drawer, the step timeline, saved prompts and broadcast.
9. **v2:** installer catalog, the `airportd` daemon so sessions survive restarts, review to merge, start from an issue, per-turn checkpoints, the file tree drawer, the `@` picker if it did not ship earlier, phone check-in, cost tracking, more than two panes.

## 8. What I want from your plan

1. The repo structure, with `crates/` for the hook bridge and adapters, and how shared TypeScript types are generated from Rust.
2. The adapter trait and the `SessionEvent` enum, written out in Rust.
3. The IPC contract: commands, channels and events, with their types.
4. The data model and where state lives. Rust is the source of truth; the frontend is a view.
5. The hook bridge protocol: port and token discovery, headers, timeouts, failure behavior, and how it chains the user's existing status line.
6. The config safety layer: snapshots, atomic writes, tagging, watching, undo.
7. Per-OS notes for Windows, macOS and Linux: ptys, shells, notifications, webview differences, config paths, packaging and code signing.
8. Spikes to run first, each with a pass/fail criterion: the Linux and macOS webview gate from section 7; ConPTY and PowerShell quirks on Windows; notification click-to-focus on each OS; status line chaining; whether switching tabs inside the app triggers Claude Code's native recap; WebSocket throughput under heavy pty output with coalescing and backpressure on; SQLite FTS5 availability through rusqlite's bundled build on all three OSes; the elevated session helper on Windows (UAC prompt, integrity levels, token-protected loopback).
9. A milestone plan with exit criteria that maps to the phases in section 7. Keep phase 1 small.
10. A testing strategy: Rust unit tests for adapters and normalizers using recorded fixture events from each CLI, component tests for the frontend, and at least one end-to-end test per OS in CI.
11. Risks and open questions, including a name check: Apple sold Wi-Fi products under the AirPort name for years, so look into trademark and app-store conflicts before publishing under this name.

**Verify before you finalize.** These CLIs change monthly. Confirm against the official docs, and cite the URLs in the plan: hook event names, fields and config locations for Claude Code, Codex CLI and Gemini CLI; the Claude Code status line fields; Tauri 2 plugin APIs; crate and package versions.

**Ask me these before finishing the plan** (use your question tool): my main development OS (`<PRIMARY_OS>`; all three remain targets); whether the repo is empty (`<REPO_STATE>`); package manager (pnpm, npm or bun); license and whether it will be open source; which CLIs I have installed today; whether to capture Claude Code's native recap; the style of notification sounds I want; whether approve-from-anywhere is on by default; history retention (my default is keep everything) and whether to mirror the archive to a backup folder; whether project references are on by default for every project.
