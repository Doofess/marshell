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
- **Batch 2 design decisions (2026-10-10; details in `docs/PLAN.md`):**
  - **Terminals are not only AI agents.** A manual CLI is any program that is not an LLM agent. It gets its program's mark, never animates, and has no prompt line of ours (you type in the terminal).
  - **Session numbers.** Every open terminal has a number that is stable until it closes (a new one takes the lowest free number), shown on every surface, so "go talk to session 6" is unambiguous. Exposed to the session as `MARSHELL_SESSION_NUMBER`.
  - **Prompt line.** Every session on screen has its own flat command-line prompt (full under a single terminal, compact in each pane); the status strip moved to a status bar for the focused session.
  - **Split view** shows up to four terminals, chosen by what the window can fit (60 by 14 of terminal per pane).
  - **No scrolling in the main window** (the session list scrolls with a hidden bar; the drawer and the prompt text keep scrolling), and **no boxed outlines** (fills and edge bars).
  - **The accent follows the focused session's vendor.** OpenAI is pale yellow in dark and deep teal in light.
  - **Keyboard.** Plain Tab, Esc and arrows always reach the program. Navigation uses a held Ctrl+Shift chord (a setting): its Tab cycles only the sessions on screen, one direction only; with arrows it reaches the side list too; Backspace selects none. Shortcuts match physical keys, use no punctuation, and avoid OS clashes (see the audit in PLAN).
  - **Accessibility.** One screen-reader announcer for the window, announcing only what needs you; a safety net of shared blocks, rule tests, a forced-colours/zoom matrix and a screen-reader script.
  - **Platforms.** Windows 11 is the only tested platform; Linux and macOS support is a plan.
  - **Batch 3 (onboarding, Departures, doctor, settings, banners, tray).** One consent card per CLI with the exact diff, backup and undo path; the user's own status line is kept in a visible file when chained; unknown values are left out of rows and the strip and shown as an en dash in tables and forms; pages scroll in their body with the actions outside it; an Allow button in a banner only for a command shown whole on one line.
- **Decisions:**
  - pnpm
  - MIT open source
  - approve-from-anywhere on by default
  - project references off until enabled per project
  - keep all history, with the backup-folder mirror off
  - capture Claude's native recap (best effort)
  - soft original chimes
  - install Codex and Antigravity (`agy`) before phase 6
- **Gemini CLI is dropped; Google's Antigravity CLI (`agy`) replaces it (2026-10-10).** Its config is `~/.gemini/antigravity-cli/settings.json`, whose `statusLine` Marshell chains. Its hooks live in `~/.gemini/config/hooks.json`, and a `PreToolUse` hook can allow, deny or ask, so approve-from-anywhere works for it too (checked against agy 1.3.3 on 2026-10-15, `docs/research/2026-10-15-local-install-check.md`). Details and sources in `docs/PLAN.md`, "Corrections to the brief".
- **Session modes (2026-10-15).** Marshell sets the mode, the same five for every agent CLI: Manual, Plan, Auto-edit, Full auto, Bypass. With its permission hook on, a session starts strict and the hook answers by the mode, so most switches are instant; Plan and a CLI's own classifier use the CLI's mode. A risky request asks in every mode. A CLI setting that skips every prompt is offered for hand-over on the consent card. Details in `docs/PLAN.md`, "Session modes".
- **Context and preview (2026-10-15).** A new phase 2.5, taking ideas from the Codex desktop app's features (not its infrastructure):
  - A preview pane for code, Markdown, images, PDF and HTML. It opens from clickable paths, file drops or quick open, with no file tree. While it is open, the window shows one session.
  - Drag a box in a PDF or image to prompt from that region.
  - Paste, drop or capture a screenshot.
  - In phase 8, comments on diff lines are sent together as one message.
  - Every one of these only fills the prompt line, so the user still sends. Image references go through the adapter, so each feature works for every CLI. Details in `docs/PLAN.md`, phase 2.5.
- **Conversations, projects, accounts and doctor (2026-10-16).** Evidence and sources are in `docs/research/2026-10-16-accounts-cloud-extensions.md`; details in `docs/PLAN.md`, phases 2e, 5, 6 and 7.
  - **Conversations (phase 5).** View and manage every CLI's locally saved conversations (Claude Code, Codex CLI, others). A conversation that exists only in the cloud gets a cloud icon where the CLI lists it (`codex cloud list --json` does; Claude web sessions are reachable only through `--teleport` or claude.ai/code; neither claude.ai nor ChatGPT has a public history API). Transcript parsing is best effort and version-gated, because the formats are internal. Resume goes through each CLI's own resume flow.
  - **Projects (slice 2e+).** Projects are folders, so they work for every CLI. Projects plus task worktrees, as in the Codex desktop app: start a task in its own worktree, then review and merge it back in the app.
  - **Accounts (phase 6).** Settings → Accounts, one section per company. Sign-in runs the CLI's own login command in a terminal (for example `claude auth login`, `codex login`). Marshell never collects, stores or brokers OAuth tokens and never reads credential files. Status comes from the CLI's own status command (`claude auth status` JSON: `loggedIn`, `authMethod`, `email`, `subscriptionType`; `codex login status` text). It shows "Signed in as <email>, <plan>, checked <time> ago", and a Sign in button when signed out. There is no expiry countdown, because no CLI exposes one.
  - **Company tabs (phase 6).** A company's settings tab appears once its CLI is signed in by any method, an API key included. It edits that CLI's own settings (Claude Code `settings.json`, Codex `config.toml`), not Claude Desktop's. Every write goes through the config service (preview, backup, atomic write, undo).
  - **Extensions (phase 6).** One universal tab for MCP servers, skills, hooks and plugins, but only where they really port. Skills (`SKILL.md`) are the portable unit. MCP stdio entries can be translated between the three formats (Claude JSON, Codex TOML, `agy` JSON). Hooks, plugins and slash commands stay per CLI.
  - **CLI version and update (phase 6, settings).** Each CLI's `--version` and install type, and an update button (`claude update`, `codex update`) that shows the command's output. This pulls part of the v2 installer runner forward; installing a CLI stays in v2.
  - **Doctor (phase 7).** A one-click doctor per CLI (`claude doctor` text, `codex doctor --json`) that opens a summary window of suggested fixes. Every fix goes through preview, backup and undo; nothing is applied silently.
  - **Instruction-file check (phase 7, beside the doctor).** For CLAUDE.md, AGENTS.md, GEMINI.md and the like, checks per CLI for size, stale paths, contradictions and that CLI's conventions, and shows suggested edits with a preview. Nothing is written until the user accepts; backup and undo apply.
  - **Antigravity (`agy`) is terminal only.** No account status, doctor or automation, because it has no login or status command and Google's consumer terms ban wrapping it in other agents. The Accounts page says why.
  - **Order.** Slice 2c (shell integration) still ships first, unchanged.
- **Appearance (slice 2d, 2026-10-16).** Settings live in a real store, `~/.marshell/settings.json`. Theme import takes Windows Terminal, iTerm2 and VS Code files, and lists the schemes in the user's Windows Terminal settings (read only). High contrast is Follow system, On or Off. Text size has three steps (Default, Large, Larger). Details in `docs/PLAN.md`, phase 2.
- **Layout, restore and Resume (slice 2e, 2026-10-16).** Split view shows up to four panes. Focus mode (Ctrl+Shift+I), the sidebar shell panel (Ctrl+Shift+S), project grouping and `layout.json` restore ship in 2e. A relaunch or core restart brings tabs back as Ended with Resume, which runs the CLI's own continue-last-here (`claude --continue`, `codex resume --last`, plain `agy`); exact-id resume is phase 3.
  - Resume is refused, with a reason, when the profile or folder is gone. Projects and task worktrees move to slice 2e+. "More than two panes" is no longer deferred. The slice's decisions are in `docs/PLAN.md`, phase 2.
- **Task worktrees (slice 2e+, 2026-10-16).** Task worktrees are created by Marshell's own git commands (shown first), live under `~/.marshell/worktrees`, and merge back as one squashed commit; projects are git repository roots. Review changes opens with Ctrl+Shift+R (Cmd+R on macOS). Details in `docs/PLAN.md`, phase 2.
- **Codex CLI:**
  - It has a `PermissionRequest` hook, so approve-from-anywhere works for Codex too.
  - Non-managed hooks need user review and trust in `/hooks`, bound to the current hook hash; hook definitions load additively across active config sources. See the current implementation notes in `docs/research/codex-cli.md`.
  - Codex discovers chained `AGENTS.md` files up to a default 32 KiB combined limit (`project_doc_max_bytes`). The instruction library uses a tagged inline block with a size warning and must preserve user-authored files.
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

Body updated 2026-10-10 to match the Amendments and docs/PLAN.md; PLAN is the detailed source.

## How to use this

Everything from section 1 down is the prompt. Paste it whole into Claude Code once it is in plan mode.

Before pasting, fill in the placeholders in section 8: your main development OS and whether the repo is empty. The prompt is written for an empty folder.

It is written as plain text on purpose, with no diagrams, so it pastes cleanly into a terminal. If you would rather keep it in the repo, save this file as `docs/BRIEF.md` and tell Claude Code to read it. That also lets you reuse it in later sessions.

Treat it as a living spec. When the plan changes a decision, change it here too.

## 1. Role and goal

You are the lead engineer on a new desktop app. You are in plan mode: explore, ask me questions, and produce an implementation plan. Do not write code until I approve the plan.

Name: **Marshell** (marshal + shell; renamed from Airport because Apple holds a live US AIRPORT trademark for software). Use `marshell` for the app binary and the config directory (`~/.marshell/`), `MARSHELL_` as the prefix for env vars, and `marshell-hook` for the hook bridge binary. All names live in one constants module (`crates/protocol/src/brand.rs` plus `src/brand.ts`) so a rename is one edit. The Departures board keeps its name: it is a feature, not the brand.

What we are building: a desktop terminal session manager for AI coding-agent CLIs (Claude Code, Codex CLI, Antigravity CLI, and any other CLI). Each session is a real terminal tab running an agent. A terminal does not have to be an AI agent: a **manual CLI** (any program that is not an LLM agent: a shell, a dev tool, a remote tool) is a session too, with its own mark and no agent status. Every open terminal has a **session number** that stays with it until it closes, so "talk to session 6" is unambiguous. A side pane lists every session with live status, a one-line recap of what the agent is doing, its model, its effort level, how many subagents are running, and how full its context is. The app also gives one place to see everything connected to each agent (skills, MCP servers, hooks, plugins, instruction files, settings), a doctor that finds broken or wasteful config and fixes it, OS notifications when an agent finishes or needs me, and friendly editors for the config and markdown files.

The goal is to take the CLI agent coding experience to the next level: run several agents in parallel and only look when one needs you. UX and UI are the top priority. The app must feel polished and fast, not like a wrapper around a terminal.

About me: I know TypeScript and have shipped Electron apps. I am new to Rust. Keep the Rust surface small and simple.

## 2. Non-negotiables

1. **Windows, macOS and Linux are all first-class, from day one.** Every feature must work on all three. That means ConPTY on Windows and forkpty on macOS/Linux; PowerShell, cmd, Git Bash and WSL on Windows and zsh/bash/fish elsewhere; native notifications on each OS; WebView2 on Windows and WebKit on macOS/Linux. CI builds and tests all three. Call out every platform difference in the plan. **Platform status (2026-10-10): Windows 11 is the only platform tested so far; Linux and macOS support is a plan, not yet verified.**
2. **Multi-CLI by design, not Claude-only.** All agent-specific code lives in adapters behind one interface. Claude Code is the first adapter, Codex and Antigravity follow, and any other CLI or plain shell works with reduced status from the terminal stream alone.
3. **Never touch my config silently.** Show the exact change before making it, back the file up, tag every entry the app adds, install at user level (not project level), and make uninstall leave zero trace. Never modify managed or organization-level settings. Never run sudo, admin elevation, or security changes (like PowerShell execution policy) without showing the command and asking.
4. **UX first.** Design tokens and the key screens come before UI code. Keyboard-first, accessible, and status is never conveyed by color alone.
5. **Zero token cost by default.** Status, basic recaps, the doctor, the settings pane, the editors and history must work without calling any model. Model-backed features are opt-in and run through the user's own CLI in headless mode.
6. **Nothing is ever lost.** Marshell keeps its own archive of every conversation and terminal recording, independent of each CLI's retention and cleanup, and never deletes the CLI's own files.
7. **Vendor marks only to say which program a session runs.** Brand colors and the vendors' own marks identify a session's CLI (the way a file manager shows an application's icon); they are never used as decoration or to suggest endorsement. Marks are sourced and checked against each owner's terms before release. A program with no known mark gets a neutral terminal mark with its own name.

## 3. Tech stack (decided; challenge it in the plan if you find a blocker)

- **Base: Tauri 2 with a Rust backend.** Reasons: small installer, fast startup, and Rust fits the backend jobs (ptys, a local server, config parsing). Conductor, one of the most polished apps in this category, is built on Tauri with a Rust backend, which is reassuring for macOS and Windows. The known risk is Linux: WebKitGTK is the only webview Tauri has there, and terminal projects report it holds back repaints while the window is idle (output painted only after a keystroke) plus dead-key input quirks on both WebKitGTK and WKWebView. So the window shell is kept swappable (see protocol-first core) and phase 1 has a go/no-go gate. If the gate fails without a clean fix, switch the shell to Electron and keep everything else.
- **Protocol-first core.** The Rust core exposes a localhost HTTP and WebSocket API (`axum`) and the UI talks to it only through that API: pty input and output over WebSocket, everything else over HTTP, a per-launch token on both, bound to 127.0.0.1 only. In v1 the core runs as a thread inside the app process. v2 can move it to a separate `marshelld` process so sessions survive UI restarts and crashes. Tauri itself is used only for the window, notifications, tray, badge and dialogs. Precedent: TermFlow and Baton, both Tauri terminals that keep throughput out of the webview hot path.
- **Rust style:** structs and enums, `Result` with `?`, `serde`, `Arc<Mutex<...>>` for shared state, `tokio` for async. Avoid heavy trait and lifetime gymnastics. I am learning Rust through this project.
- **Rust crates (verify current versions):** `portable-pty` for ptys on all three OSes, `axum` with its WebSocket feature for the core server, `serde` and `serde_json` with `preserve_order`, `toml_edit` for TOML edits that keep comments, `notify` for file watching, `rodio` for sound playback, `tokio`.
- **Tauri plugins:** notification, single-instance, window-state, opener. The store plugin is dropped: the Rust core owns all app settings in `~/.marshell/`, which the v2 daemon needs. Verify current APIs.
- **Frontend:** React and TypeScript with Vite. `@xterm/xterm` with the fit, WebGL, search and web-links addons (xterm 6 removed the canvas renderer, so the fallback where WebGL is unavailable is the DOM renderer). CodeMirror 6 for editors. Accessible component primitives such as Radix, with CSS variables for design tokens. Keep state management simple. Config files are never written from the frontend; the Rust config service is the only write path.
- **Transport:** WebSocket for pty streams, HTTP for commands and queries, a second WebSocket for status updates. Tauri events only for OS-level things. Generate shared TypeScript types from the Rust types (`ts-rs` or `specta`; your pick).
- **Session persistence:** live processes do not survive an app restart in v1. Restore the layout and relaunch each agent with its CLI's resume feature (for example `claude --resume <id>`). The `marshelld` daemon in v2 makes sessions survive restarts; design the protocol so that move is a packaging change, not a rewrite.
- **Repo:** a single repo with `crates/protocol` (the shared types and `brand.rs`), `crates/core` (server, pty manager, adapters, config service), `crates/bridge` (the `marshell-hook` bridge and the `marshell` CLI), `crates/host` (the elevated helper), `crates/fake-agent` (a test-only agent that emulates a CLI), `src-tauri/` (the thin window shell) and `src/` (the frontend). `docs/PLAN.md` is the detailed source for the layout.

## 4. Architecture

### Rust core modules

1. **PTY manager.** Spawns each session through a shell profile (for example `pwsh -NoExit -Command claude` or `zsh -lc claude`), keeps a map of tab id to process, forwards input and resize events, and streams output to the frontend over the pty WebSocket. It sets `MARSHELL_TAB_ID`, `MARSHELL_AGENT` and `MARSHELL_SESSION_NUMBER` env vars on every spawn. Keep the hot path in Rust: coalesce output (flush every 8 ms or at 64 KB), apply backpressure (stop reading the pty when more than 1 MiB is unacknowledged by the renderer), and keep a scrollback ring buffer per session that can replay on reconnect and feed previews. It also parses escape sequences in the output stream: OSC 7 (current directory), OSC 133 (command start and end), OSC 9 and OSC 777 (terminal notifications), and BEL.
2. **Hook bridge.** A tiny binary (`marshell-hook`) that every CLI's hooks call. The app copies it to a stable path, `~/.marshell/bin/marshell-hook`, on launch and keeps it current, so settings never point at an app bundle path that changes with versions. It reads the event JSON on stdin and the tab id from the env, adds which agent sent it, and POSTs to the core server. It must exit fast and never block the agent: on any failure, exit 0 with no output. Two special roles: as Claude Code's status line command in chaining mode (forward the JSON, then run the user's original status line command and print its output), and as a `PermissionRequest` hook (forward the request, wait for the user's decision from the app up to the hook timeout, return `decision.behavior`; if no decision arrives, exit 0 with no output so the normal terminal prompt applies).
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

Session state per tab: session number, agent (or, for a manual CLI, the program), name, cwd, project (folder name, repo name and git branch, derived by the app from the cwd so it works for every tab including plain shells), status, `model: Option`, `effort: Option`, `permission_mode: Option` (Claude Code: default, plan, acceptEdits, auto, dontAsk, bypassPermissions; other CLIs map their approval modes onto a shared set of manual, plan, auto-edit and full-auto, with the CLI's own label kept for the tooltip), `subagents: HashSet<id>` (a set of ids, never a counter, so one missed stop event cannot leave it off by one), `context: Option<{ tokens, window_size, pct }>`, `session_tokens: Option<{ input, output, cache_read, cache_write }>`, `cost_usd: Option`, recap text and its age, last event time, needs-attention flag, muted flag. Unknown values display as a dash, never a guess. A manual CLI has only the states at the prompt, running a command and exited (with its exit code); it has no thinking animation, model, effort, mode or context.

### Claude Code adapter (first; verify everything against the current docs before coding)

- Hooks: command hooks receive JSON on stdin with `session_id`, `transcript_path`, `cwd`, `permission_mode`, `effort.level`, `hook_event_name`, plus `agent_id` and `agent_type` inside subagents. HTTP hooks also exist, but use the command bridge for consistency with the other CLIs. Events to use: `SessionStart` (has `model` and `source`), `UserPromptSubmit`, `PreToolUse` and `PostToolUse` (`tool_name`, `tool_input`), `Notification` (matchers `permission_prompt`, `idle_prompt`, `elicitation_dialog`, `agent_needs_input`), `SubagentStart` and `SubagentStop` (`agent_id`), `Stop` (`last_assistant_message`), `StopFailure` (`rate_limit`, `overloaded`, and others), `PostModelSwitch` (`from_model`, `to_model`), `SessionEnd`.
- Permission mode: `permission_mode` arrives as a common field on most hook events (`default`, `plan`, `acceptEdits`, `auto`, `dontAsk`, `bypassPermissions`; the mode labeled Manual arrives as `default`). There is no dedicated mode-changed event, so update the mode from whichever event arrives next. The status line re-runs when the mode changes, which is a cheap trigger to refresh.
- Status line JSON: `model.display_name`, `effort.level`, `session_name`, `workspace.project_dir`, `workspace.repo.name` and `owner`, `workspace.git_worktree`, `context_window.context_window_size` (200000 by default, 1000000 for extended-context models), `context_window.total_input_tokens` and `total_output_tokens` (tokens currently in the context window from the last API response, not a running total), `context_window.used_percentage`, `cost.total_cost_usd` (estimated at list price), `cost.total_lines_added` and `removed`, `rate_limits.five_hour` and `seven_day` (`used_percentage`, `resets_at`; Pro and Max subscribers only, after the first API response). A separate `subagentStatusLine` setting receives a `tasks` array with status, model, effort and token count per subagent.
- Session tokens so far: the status line has no running total, so sum the per-message `usage` entries in the transcript JSONL (input, output, cache read, cache write) and refresh on each `Stop`. Verify the transcript format first.
- Headless: `claude -p --model <small model> --settings '{"disableAllHooks": true}'` for smart recaps. Disabling hooks prevents a feedback loop into our own server and also suppresses the status line.
- Native recap: Claude Code prints a recap after about three minutes unfocused (with at least three turns) and on `/recap`; it is capped at 400 characters and can be turned off with `awaySummaryEnabled`. A literal `Recap:` prefix is unverified. Capturing it from the pty stream is optional and the format is not a stable API.
- Verified corrections (see `docs/PLAN.md`, "Corrections to the brief"): `model` arrives only on `SessionStart`; `effort.level` only on tool-context events; a `PermissionRequest` hook has a 600 s default timeout and only a `decision` object grants or denies (exit code 2 is ignored); `additionalContext` is capped at 10,000 characters; `cleanupPeriodDays` defaults to 30; the transcript format is internal and changes between versions, and one API message spans several JSONL lines, so dedupe by `message.id`.
- Config locations: `~/.claude/settings.json` (user), `.claude/settings.json` (project), `.claude/settings.local.json` (local); hooks also come from plugins and from skill and subagent frontmatter; skills in `~/.claude/skills` and `.claude/skills`; instruction files `CLAUDE.md` and `.claude/rules/*.md`. List MCP servers with `claude mcp list` rather than parsing files, because file locations change between versions.

### Codex CLI and Antigravity CLI adapters (verify current docs)

- Codex: current hook docs and implementation details are in `docs/research/codex-cli.md`. Hooks can be defined in `~/.codex/hooks.json` or event-nested hook tables in `~/.codex/config.toml`; hook sources are additive, and non-managed definitions require review and hash-bound trust in `/hooks`. `PermissionRequest` supports explicit allow/deny; returning no decision leaves Codex's normal prompt. Hook inputs document the active model and permission mode. Codex chains discovered `AGENTS.md` files with a default combined 32 KiB cap; the official docs describe discovery/chaining but no import syntax, so the instruction library uses a previewed tagged inline block. App-server exposes additional model/usage/rate-limit APIs, with version, payload, authentication, and session association limits recorded in the research handoff.
- Antigravity (`agy`): config in `~/.gemini/antigravity-cli/settings.json`. Its `statusLine` command gets JSON on stdin (model, context window, quota, agent state, whether a tool waits for confirmation), so model, context % and the plan meter come through the chained status line. Plugins (skills, agents, rules, MCP, hooks) live in `~/.gemini/antigravity-cli/plugins/`. Hooks go in `~/.gemini/config/hooks.json` (events `PreToolUse`, `PostToolUse`, `PreInvocation`, `PostInvocation`, `Stop`; camelCase payloads with no event name); `PreToolUse` is its permission hook (allow, deny or ask). The consent card adds its hook entries there and chains the status line in `settings.json`. Antigravity has no effort setting; show a dash. Gemini CLI is dropped (2026-10-10).
- Any AI CLI without hooks: terminal heuristics only. No output for a few seconds means idle or waiting; BEL, OSC 9 or OSC 777 means it wants attention; OSC 133 gives command state for plain shells. A manual CLI (not an LLM agent) has only these terminal signals and the foreground process's name, which picks its mark.

## 5. Features

### 5.1 Session sidebar (the core)

- Each row: a thin brand stripe or dot (defaults: Claude orange, Antigravity cyan, OpenAI pale yellow in the dark theme and deep teal in the light one, generic gray; editable per agent and per session) and the vendor's mark (a manual CLI shows its program's mark, still, in a calm tone), the session number, the session name (custom title, else the project name), the project and git branch ("my-app · feat/auth"), a status indicator, and a context ring.
- Status indicators use shape and motion, never color alone: working = gently pulsing dot, done = solid check, needs you = a distinct badge (the only element allowed to grab attention), error = its own icon.
- Second line (on hover or when expanded, with compact and comfortable density settings): model, effort, permission mode, and subagent count. Plan mode gets its own pill. Modes that act without asking (`auto`, `dontAsk`, `bypassPermissions`) get a small caution mark so they are always visible. Third line: the one-line recap.
- Expanded details and the ring tooltip: context as "84k / 200k (42%)", session tokens used so far as "312k in · 48k out" with cache reads and writes in the tooltip, and the estimated cost where the CLI reports one.
- Fixed order that the user can drag. No auto-sorting; rows must not jump while the user reaches for them. Add a "jump to next session that needs me" shortcut instead. `Ctrl+Shift+1` to `9` (`Cmd+1` to `9` on macOS) switch to that session number, and the palette's "Go to session…" reaches higher numbers. Optional grouping by project. The list never shows a scrollbar: it scrolls with a soft fade at its edges, and the rail (a 52 px collapsed sidebar) shows the same sessions.
- **Session numbers.** Each open terminal has a number: stable until it closes, and a new terminal takes the lowest free number. The number is shown on every surface that names a session (rows, rail, pane headers, lane cards, menus, palette) and exported to the session as `MARSHELL_SESSION_NUMBER`, so the `marshell` CLI and agents can address "session 6".
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
- **Broken:** hook script missing or not executable (the CLI treats this as a non-blocking error, so a safety hook can silently stop working), MCP server failing to connect or auth expired, config that does not parse, skill frontmatter missing or malformed, the same MCP server defined in two scopes, outdated CLI version, hooks configured in a CLI whose hook engine is off (Claude Code's `disableAllHooks`), our status line chain broken, a CLI sending its own desktop notifications in addition to ours.
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

- Instruction files: a tree by scope covering `CLAUDE.md`, `.claude/rules`, `AGENTS.md`, Antigravity's rules, skills, subagents and commands. CodeMirror markdown editor with live preview. Frontmatter shown as a validated form above the body, reusing doctor checks. A token estimate per file ("~1.2k tokens, loaded every session"). A "what loads here" view: pick a folder and see the merged, ordered list of instruction files a session there would load. Templates for a new skill, subagent, or command.
- Settings: form mode (toggles, dropdowns, lists) and raw mode (JSON or TOML with schema validation; Claude Code's settings have a published JSON schema, verify on SchemaStore). Show the effective value with a scope badge such as "set in project settings, overrides your user setting". Friendly builders for permission rules (tool picker plus pattern field), hooks (event, matcher, command), and environment variables (masked). App-managed entries are locked.
- Shared safety layer: minimal edits that preserve formatting and key order, all through the Rust config service (`serde_json with preserve_order` for JSON, `toml_edit` for TOML), validate then write atomically (temp file plus rename), snapshot with a diff and undo (the same system the doctor uses), watch for outside changes while a file is open and offer a merge instead of overwriting, and say whether a change applies live or needs a session restart.
- Optional, opt-in "Improve this file" that runs the user's own CLI headless and shows a diff to accept or reject.

**Instruction library (many .md files, at user and project level):**

- **A set of named docs per scope, not one file.** Besides `CLAUDE.md`, `AGENTS.md` and Antigravity's rules, manage `SECURITY.md`, `SCOPE.md`, `CONVENTIONS.md`, `ARCHITECTURE.md`, `TESTING.md` and any custom name. Two scopes: user level (applies to every project; lives in `~/.marshell/instructions/`) and project level (lives in the repo and is committed, so a team shares it). Each doc has a description, an enabled toggle per project, an agent filter (all CLIs or only some), and optional path globs so it loads only when the agent touches matching files.
- **Write once, wired into every CLI.** Marshell keeps each doc once and wires it into each CLI's own loading mechanism, tagging everything it writes: Claude Code through `.claude/rules/<name>.md` with `paths:` frontmatter for conditional docs, and the user-level rules directory for user docs; Codex through a tagged block inlined into `AGENTS.md` (current docs describe file discovery/chaining, not imports); Antigravity through its rules (a plugin component; the file layout is verified when the adapter is built). Verify each CLI's current rules and import mechanism before wiring. Use copies with a content hash rather than symlinks (symlinks on Windows need developer mode), and have the doctor flag drift between the library and the wired copies.
- **Templates and profiles.** Starter templates for each common doc: `SECURITY.md` covers secrets handling, destructive commands and dependency policy; `SCOPE.md` covers what the agent may and may not touch; `TESTING.md` covers how to run and write tests. A project profile bundles docs, settings and hooks so a new project is set up in one click.
- **Budget and precedence.** The "what loads here" view lists user docs, project docs and conditional docs in load order, with a token count per doc and a total, and warns when the always-loaded total passes a threshold (default 4k tokens). Every doc keeps snapshot history with restore, the same as settings.

### 5.7 Terminal and shell settings

- Layer 1, app-owned terminal settings: font family, size and weight, ligatures, theme, cursor style and blink, padding, background opacity where the OS supports it, scrollback, copy on select, paste warnings, bell behavior. Global defaults plus per-profile overrides; an optional brand theme per agent.
- Layer 2, shell profiles: auto-detect installed shells (Windows: PowerShell 7, Windows PowerShell, cmd, Git Bash, WSL distros; macOS and Linux: `/etc/shells`). A profile is shell, args, starting directory, env vars, name, color and icon. Agents always launch through a profile so they get the user's normal PATH and the tab stays usable after the agent exits.
- Profile settings page: one page per profile with sections Appearance, Behavior, Shell and Startup, so a user changes PowerShell's look and feel from the app without touching files. The app-managed init script is shown and editable there.
- Layer 3, the shell's own settings: never edit `$PROFILE`, `.bashrc` or `.zshrc` directly. Each session runs a small app-owned init script that loads the user's profile first, then applies app settings on top. For PowerShell: PSReadLine options (prediction view, edit mode, colors), prompt style with Starship or oh-my-posh integration, module auto-load. For bash, zsh and fish: aliases, prompt and completion settings. The same script enables shell integration (OSC 7 for cwd, OSC 133 for command state). A "make permanent" action writes the change into the user's own profile through the show, back up, tag and undo flow. Security settings such as execution policy are shown read-only with a confirm-to-run command.
- Elevated sessions: a launcher option that starts a profile with administrator rights. Windows: launch a small `marshell-host --elevated` helper through ShellExecute with the `runas` verb, which shows the UAC prompt; the helper owns the elevated ConPTY and connects back to the core over the localhost protocol with the per-launch token, so the tab renders like any other. Windows integrity levels prevent a non-elevated process from hosting an elevated pty directly, which is why the helper exists. The helper accepts only pty input and resize from the core, nothing else, because a lower-integrity process steering an elevated one is an escalation surface; if the spike shows it cannot be made safe, fall back to Windows Terminal's behavior and open elevated sessions in a separate elevated Marshell window. macOS: `sudo -i` in the session, with the password or Touch ID prompt in the terminal. Linux: `pkexec` or `sudo -i`. Elevated tabs show a shield badge and an "Administrator" label in the header for the whole session, start in manual permission mode, and an agent in an elevated session cannot be switched to bypass mode without an extra confirmation. Elevation is never remembered; every elevated session prompts again.

### 5.8 Usage rings

- A context ring on each session row showing how full its context is, with the tokens and window size in the tooltip ("84k / 200k"). Claude Code: the `context_window` fields from the status line. Others: computed from session logs and the model's window size. Hide the ring when unknown rather than showing a fake 0%.
- Session tokens used so far are a number, not a ring, because there is no ceiling to fill against. Show them in the expanded row and in a per-session details panel, with cost where available.
- A plan-usage ring in the app header showing the 5-hour limit, with the 7-day limit and both reset times in a tooltip. Claude Pro and Max only; hidden when absent.
- Green under 70%, yellow 70 to 89%, red at 90% and above, always with the number next to it. An SVG circle driven by `stroke-dasharray` is enough.

### 5.9 Installer (v2)

- A catalog as data (JSON or TOML, updatable without an app release): per tool and per OS, the official install command, version check, update check, and login command.
- Every install runs in a visible terminal tab. Show the exact command before running it. Curated catalog and official sources only. No silent elevation. Run the version check afterwards, then offer the login flow. The version check also powers "update available" in the settings pane.

### 5.10 History (nothing is ever lost)

- **Three layers captured for every session, automatically.** (a) The CLI's own structured transcript, copied incrementally into Marshell's archive (`~/.marshell/history/`, compressed) so it survives the CLI's own cleanup. Claude Code deletes old transcripts after a retention period by default (verify the current setting and its default), so Marshell's copy is the durable one. (b) A raw terminal recording in asciicast v2 format, written from the pty stream with timestamps. (c) Marshell's own event stream and recaps. Marshell never modifies or deletes the CLI's files.
- **Sessions started outside Marshell are indexed too.** Watch each CLI's transcript directory and import sessions run from a plain terminal, so history stays complete even when the user does not use the app.
- **Index:** SQLite (`rusqlite`, bundled) with FTS5 full-text search over prompts, assistant text, tool calls and file paths. Fields: agent, project, branch, title, started, ended, duration, turns, tokens, cost, model, outcome (done, failed, abandoned), tags, pinned, bookmarks, continuation chain.
- **Continuation chains:** compaction, `/clear`, resume and fork create new session ids in the CLIs. Link them (Claude Code: `SessionStart` with `source` resume, fork or compact, plus `PreCompact` and `PostCompact`) so one task reads as one thread.
- **History view (`Ctrl+Shift+Y`, `Cmd+Y` on macOS):** a list grouped by project, newest first, with filters (agent, project, branch, date, model, outcome, tag) and search (`Ctrl+Shift+Alt+F` from anywhere; results show highlighted snippets and jump to that turn). A live session and its history entry are the same object; ending a session only makes it past.
- **Conversation view:** render the transcript as a readable conversation, not raw terminal text: user prompts, assistant text as markdown, tool calls collapsed into cards ("Edited src/auth.ts, 12 added, 3 removed", "Ran npm test, passed"), subagent runs as collapsible sections, compaction points marked, and a timeline scrubber. A second tab replays the terminal recording with a player. A third tab lists the files changed.
- **Actions:** Resume (opens a new tab with the CLI's resume command; the main way to go back), Fork where the CLI supports it (`claude --resume <id> --fork-session`), Rename, Tag, Pin, Bookmark (`Ctrl+Shift+B` during a live session marks the current turn), Copy as markdown, Export (markdown, JSON, asciicast), Archive, and Delete Marshell's copy (with confirmation; the CLI's own files are never deleted unless the user asks explicitly).
- **Titles:** use the CLI's session name or AI-generated title when present, else the first prompt shortened. The user can rename, and the name is kept across resumes.
- **Retention and backup:** keep everything by default; optional size or age limits the user sets; a backup folder option (any synced folder) that mirrors the archive; disk usage shown in settings. Exports can redact common secret patterns (API keys, tokens) with a toggle, because transcripts often contain them.
- The adapter additions this needs are listed in section 4.

### 5.11 Project references (an app-level skill for every agent)

- **Goal:** when the user says "the login button", "the pty manager" or "that migration from yesterday", every agent finds the right thing on the first try instead of searching the repo. It works for every CLI and model because Marshell supplies it through the hook bridge, not the agent.
- **Project map:** for each repo Marshell maintains a compact map (under 300 tokens): top-level layout, entry points, naming conventions, and an alias dictionary the user curates ("money app" = a repo path, "login button" = `src/components/auth/LoginButton.tsx`). Stored in `~/.marshell/projects/<id>/`, optionally also committed as `.airport/references.md` so a team shares it.
- **Resolver at prompt time:** the bridge receives each prompt (`UserPromptSubmit` on Claude Code and Codex; Antigravity's equivalent is verified when the adapter is built), matches it against aliases, file and directory names, recent history and, in v2, a local symbol index built with tree-sitter or ctags. It returns `additionalContext` only when something matched, for example "References: 'login button' = src/components/auth/LoginButton.tsx. 'Yesterday's migration' = session 'Add JWT migration' (Oct 8), files: src/db/migrations/0012_jwt.sql." No match means no injection, so the token cost stays near zero. The doctor reports how many tokens the resolver injected.
- **Learns from history:** the History index knows which files each session touched and what the user called them. Marshell suggests aliases for names it sees repeated ("you have said 'the pty manager' four times; map it to crates/core/src/pty.rs?") and resolves "the bug from yesterday" to a past session's recap and files.
- **Manual references:** typing `@` in a session opens Marshell's picker (files, aliases, past sessions, branches) and inserts the concrete path or session id in the form that CLI understands (Claude Code already accepts `@file` references). This intercepts keystrokes, so keep it optional and move it to v2 if it proves fragile.
- **Rules:** never more than about 300 tokens per injection; plain factual statements, not instructions, so the CLI's prompt-injection defenses do not fire; a per-project toggle; the map is rebuilt in the background on file changes and never blocks a prompt for more than 100 ms.

## 6. UX and UI principles (treat these as requirements)

1. **The core loop is glance, jump, act.** The sidebar's one job is to answer "who needs me?" in under a second. Show little by default; details on hover or expand.
2. **Status never by color alone.** Brand colors are accents only; Claude's orange and OpenAI's yellow sit too close to warning amber. Status gets shapes and motion, which also makes the app usable for colorblind users.
3. **The terminal is the hero.** Minimal window chrome, collapsible sidebar, focus mode. A clean UI font for the app and a good monospace font for terminals, kept visibly separate.
4. **Keyboard-first, mouse-friendly.** A command palette (`Ctrl+Shift+K`, `Cmd+K` on macOS) that reaches every action: new session, switch session (by name or by number: type `6`), run doctor, open a setting. Every mouse action has a shortcut, and shortcuts are discoverable. Plain Tab, Esc and the arrows always reach the terminal program; navigation between terminals uses a held Ctrl+Shift chord, matches physical keys (so it works on every keyboard layout) and uses no punctuation. The full shortcut audit, with the reasons for each clash avoided, is in `docs/PLAN.md` ("Keyboard navigation and shortcuts").
5. **Every state is designed.** First run: detect installed CLIs, explain in plain words what the app will change (hooks, status line), ask permission, then start the first session. Empty state points to "start one". A missing CLI offers the installer; a broken hook links to the doctor. On restart, restore the layout and sessions exactly. Confirm before closing a session that is mid-task.
6. **Speed is part of UX.** Typing latency in the terminal is the first thing people notice: use the WebGL renderer, keep startup fast, never block the UI on file scans or the doctor, and measure input latency.
7. **Settings people do not fear.** Search across all settings, plain-language descriptions, and an always-visible undo. Raw JSON is available but never required.
8. **Consistency from day one.** Define design tokens first (light and dark colors, spacing, type scale, radii). Build on accessible primitives so keyboard navigation and screen reader labels come for free. Subtle, purposeful motion that respects the OS reduced-motion setting.
9. **Native feel on each OS.** Follow macOS, Windows and Linux conventions for menus, window controls, and modifier keys.

### Design philosophy and themes

- **Apple-like: simple, clean, content first.** The terminal is the content and the chrome recedes. Few type sizes, generous whitespace in the app chrome (dense inside the terminal), native-feeling controls, hairline separators, no decoration and no gradients. Every screen should look obvious at first glance.
- **Three themes: Light, Dark and System.** Dark is true black: `#000000` base surfaces for OLED screens, raised surfaces from `#0A0A0A` to `#141414`, hairline borders as white at 8 to 12 percent opacity, body text near `#E5E5E5` rather than pure white to avoid halation. Light uses white and warm grays. The terminal background follows the app theme by default and can be overridden per profile.
- **User-chosen accent color.** A preset palette of about eight accents plus a custom hex, applied to selection, focus rings, toggles, links, the active tab marker and the plan ring. Agent brand colors stay on session stripes and dots only, so the accent never fights them. Default to the OS accent where the OS exposes it. **While a session is focused, the window's accent follows that session's vendor colour**, and the attention ("needs you") colour stays a fixed signal amber. Check every pairing for WCAG AA contrast in both themes.
- **Platform fit.** Follow each OS for window controls, menus, modifier keys and title bar style: one unified 40 px top bar on every OS (macOS: traffic lights inset; Windows: its own window controls at the right; Linux: native decorations plus a header). The OS window title follows the focused session, for example "billing — Marshell".

### Signature moves (what makes Marshell stand out)

1. **Approve from anywhere.** (Claude Code and Codex have a `PermissionRequest` hook; Antigravity's `PreToolUse` hook can allow, deny or ask.) When a session asks for permission, show the request as a card with Allow, Deny and Allow always, in the sidebar, in the needs-you lane, and in the OS notification where the OS supports action buttons. For Claude Code, implement it through the `PermissionRequest` hook as described in section 4, so there is no keystroke injection. For CLIs without such a hook, the card focuses the tab instead. Handling five agents' prompts from one place is the single biggest win.
2. **Needs-you lane.** A small strip above the fixed session list showing only sessions waiting on you, oldest-waiting first (first in, first out), each with its request or question text and the approve card. The main list never reorders. `Ctrl+Shift+N` jumps to the next one.
3. **Peek.** Hover a row, or press a key on it, to see a live read-only preview of that terminal's last 30 lines in a popover, rendered from the scrollback the core already holds. Check on a session without leaving the one you are typing in.
4. **Departures board.** A full-window overview mode (opened from the palette; it has no default key yet, because `Ctrl+Shift+D` is Split) listing every session like a flight: name, project and branch, agent, status, waiting time, context fill, last recap. Split-flap style transitions on status changes, static when reduced motion is on. This is the screen for running ten agents, and the app's visual signature.
5. **Launcher.** One keyboard-driven screen to start a session: agent, project (recent folders with their git branch), permission mode (plan, manual, auto), model and effort presets such as Quick and Deep, an optional new git worktree (pass the CLI's own flag, for example `claude --worktree`), and an optional first prompt. Remembers the last choice per project.
6. **Command palette with broadcast.** `Ctrl+Shift+K` reaches every session, action and setting. Includes "send to": type a prompt once and send it to several sessions, for example "run the tests and report". Includes switch mode and set effort for the current session.
7. **Clickable paths and a diff drawer.** File paths in agent output (`src/auth.ts:42`) open in the user's editor (configurable: VS Code, Cursor, Zed, Vim). A per-session diff drawer shows files changed since your last prompt with plus and minus counts, built from edit events and `git status`; click a file for its diff.
8. **Attention choreography.** A status change flashes the row once with the accent color, then settles. The needs-you badge bounces once and stays. Done draws a check in 200 ms. Sounds are one soft, original family (D major pentatonic, a sine plus a felt-bell timbre): a soft two-note chime for needs-you, a short arrival tone for done, a quiet click for subagent done, a low tone for error. The window title and the dock or taskbar badge carry the needs-you count. All of it respects reduced motion and quiet hours.
9. **Split view.** Up to four terminals on screen at once (`Ctrl+Shift+D` adds a pane; a layout menu picks the arrangement, and the window shows only as many as it can fit at a usable size), for typing in one while watching others. **Every AI session on screen has its own prompt line**, so a line of text always goes to the session of its own pane (a manual CLI has none: you type in its terminal). Zoom a pane with `Ctrl+Shift+P`.
10. **Escape hatches build trust.** Every session has "Open in system terminal" (runs the CLI's resume command) and "Copy resume command". Permission mode is always visible in the terminal header, and bypass and auto modes mark the header red (a tint and a bar on its start edge, not a box outline) for the whole session.

Optional delight, default off: an airport mode that relabels statuses as Boarding, In flight, Holding, Landed and Diverted. (Deferred beyond v1; see `docs/PLAN.md`.)

### Borrowed from the field

Study these before planning and take what fits. Superset is the closest competitor (any CLI agent in parallel worktrees, built-in terminal, diff viewer, in-app browser, automations, a CLI, free), Conductor is the most polished (macOS only, Tauri, review and merge, PR flow), Crystal is open source (Electron, per-iteration commits, git operations), and Claude Code's own desktop app runs parallel sessions with automatic worktrees. Marshell's position is what none of them combine: real terminals for any CLI on Windows, macOS and Linux, the needs-you workflow with approve-from-anywhere, config management with a doctor, and terminal settings.

v1 borrowings (cheap, high leverage):

- **Repo scripts** (Superset, Conductor): an optional `marshell.json` committed to the repo with `setup` (runs when a session or worktree starts, for example install dependencies and copy `.env`), `run` (dev server) and `teardown` scripts, shared with the team.
- **Custom agents** (Superset): add any terminal command as an agent with a name, color and launch args; it appears like a built-in. This is the data-configured generic adapter from section 4.
- **A `marshell` CLI** (Superset): `marshell new --agent claude --cwd . --prompt "..."`, `marshell list`, `marshell focus <id>`, `marshell send <id> "..."`; `<id>` accepts a session number, so `marshell send 6 "..."` reaches session 6. Scripts and agents themselves can then start and steer sessions, and the user can drive the app from any terminal. Ship a small built-in skill so an agent knows how to use it.
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

- Scrollback search (`Ctrl+Shift+F`), command markers from OSC 133 with jump between prompts, clickable links, copy on select and bracketed paste, ligatures, per-tab zoom, a smooth-scrolling toggle, cursor options, and at least 10k lines of scrollback held in Rust so it survives renderer reloads.
- Theme import from Windows Terminal, iTerm2 and VS Code theme files. App chrome follows the OS light or dark setting. A high-contrast mode.
- Inline rename with `F2`, drag to reorder, close with confirmation while working, reopen a closed session.
- A shortcut cheat sheet overlay on `Ctrl+Shift+O` (`Cmd+O` on macOS).

### Information architecture and visual system

- Sidebar on the left: the needs-you lane, then the fixed session list, then a footer with the plan ring and the doctor status.
- Main area: the terminal with a slim header showing session number and name, project and branch, mode chip, model and effort, subagent count, context ring, port chips. Below the terminal, a flat command-line prompt for an AI session (one per pane in split view), then a thin status bar for the focused session (model, context, plan limits, cost, branch and similar facts; unknown ones are left out). The main window never shows scrollbars except the session list's hidden-bar scroll, the right drawer and a long prompt.
- Right drawer, toggled: recap timeline for this turn, diff drawer, session details (tokens, cost, resume command).
- Settings is a page, not a dialog: left navigation (General, Appearance, Terminal, Shells, Agents, Notifications, Doctor, Advanced) with search on top.
- Visual system: 8 px grid, 13 px UI type with 1.5 line height, sidebar rows 36 px compact and 48 px comfortable, one accent per agent, status by shape, 6 px radius, no gradients, motion 150 to 200 ms ease-out.
- Performance budget: typing latency under 30 ms end to end, 60 fps while an agent streams, sidebar updates batched at 100 ms. Include a hidden performance panel.
- Accessibility: one screen reader live region for the whole window that announces what needs you ("session 6, billing needs you"), never progress, visible focus rings, a large-text mode, xterm's accessibility mode on request. Selection and focus are visible without colour and in Windows High Contrast; a safety net of shared blocks, rule tests, a forced-colours/zoom matrix and a screen-reader script (see `docs/PLAN.md`) catches regressions.

Process: before writing UI code, produce mockups of the main window with the sidebar, every sidebar row state, the doctor report, the settings editor, and first-run onboarding, as component stories plus a published private review page, and get my approval on them, in three sign-off batches (see the Amendments).

### Brand and icon

The app is called Marshell (marshal + shell). The icon is two marshalling batons raised in a V ("Night shift": a charcoal tile, grey grips and orange lights), replacing the earlier plane: one bold glyph on a rounded-square tile, flat colors with at most two tones, in the style of current terminal app icons such as the PowerShell 7 icon. The source is `assets/icon/app-night-shift.svg`. It must read clearly at 16 px and 32 px, work on light and dark backgrounds, and have a monochrome variant for the menu bar and system tray. Draw an original mark and do not copy any existing app's icon. Deliver it as an SVG plus a 1024 px PNG, and run it through Tauri's icon generator to produce every platform size. In the window the logo and a lowercase "marshell" wordmark sit in the top bar and open the app menu.

## 7. Phased roadmap (v1 is phases 1 to 9)

0. **Design (blocks UI work).** Tokens, status glyphs, sidebar rows, the approve card, the main window (split view, focus mode, rail, prompt line, status bar), the launcher and command palette, then onboarding, Departures, doctor, settings, notification mocks and the tray, as stories plus a published review page, in three sign-off batches. Sign-off includes a keyboard-only walkthrough, reduced motion, high contrast and a screen-reader pass.
1. **Scaffold, one live tab, and the webview gate.** Tauri project, one pty tab working end to end on Windows, macOS and Linux through the core server's WebSocket, xterm with the fit and WebGL addons, design tokens, and the empty layout shell. The gate, on Linux (WebKitGTK) and macOS (WKWebView): (a) output paints while the window is idle with no keyboard input; (b) dead keys and IME input work; (c) `yes | head -n 2000000` and a 50 MB file streamed with `cat` do not stall the UI and typing stays responsive; (d) the WebGL renderer works, or the DOM renderer is acceptable. Any failure without a clean fix means switching the window shell to Electron and keeping the core and frontend. Exit criteria: typing feels instant, resize works, CI builds all three OSes.
2. **Multiple tabs and the sidebar.** Shell profile detection, the launcher (agent, project, mode, model and effort presets, worktree through the CLI's flag, elevated session), profile settings pages, themes and accent colors, split view (up to four panes, a prompt line in each), session numbers and manual CLI marks, clickable paths, session persistence and restore, raw terminal recording to disk, command palette basics, brand colors.
3. **Hook bridge, core server and the Claude Code adapter.** Status indicators, event-built recap, status line chaining, pty heuristics fallback, the context ring and the plan ring, approve-from-anywhere through `PermissionRequest`, the needs-you lane, peek, transcript capture and the history index with Resume.
4. **Notifications and attention choreography.** Plus the departures board and stuck detection.
5. **History view.** Search, conversation view, recording replay, bookmarks, tags, export, and import of sessions run outside Marshell.
6. **Settings pane (read-only), the Codex and Antigravity adapters, custom agents, repo scripts, port chips, the `marshell` CLI, and project references.**
7. **Doctor** with the Fix button, snapshots and undo.
8. **Editors** for the instruction library and settings on the same safety layer, opt-in smart recaps and native recap capture, the diff drawer, the step timeline, saved prompts and broadcast.
9. **Release.** Signed and notarized builds, the updater, installers that unhook on uninstall, and the formal trademark search.

**v2 (after v1):** installer catalog, the `marshelld` daemon so sessions survive restarts, review to merge, start from an issue, per-turn checkpoints, the file tree drawer, the `@` picker if it did not ship earlier, phone check-in, cost tracking, more than four panes.

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
11. Risks and open questions, including a name check: the original name Airport conflicted with Apple's live AIRPORT trademark for software, which is why the app is now Marshell (a web search found no software of that name); run a formal trademark search before the first public release.

**Verify before you finalize.** These CLIs change monthly. Confirm against the official docs, and cite the URLs in the plan: hook event names, fields and config locations for Claude Code, Codex CLI and Antigravity CLI; the Claude Code status line fields; Tauri 2 plugin APIs; crate and package versions.

**Ask me these before finishing the plan** (use your question tool): my main development OS (`<PRIMARY_OS>`; all three remain targets); whether the repo is empty (`<REPO_STATE>`); package manager (pnpm, npm or bun); license and whether it will be open source; which CLIs I have installed today; whether to capture Claude Code's native recap; the style of notification sounds I want; whether approve-from-anywhere is on by default; history retention (my default is keep everything) and whether to mirror the archive to a backup folder; whether project references are on by default for every project.
