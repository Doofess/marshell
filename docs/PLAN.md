# Marshell: implementation plan (formerly "Airport")

## Context

`C:\dev\Airport\BRIEF.md` specifies a cross-platform desktop terminal session manager for AI coding-agent CLIs. It runs real terminal tabs for Claude Code, Codex, Antigravity or any other CLI, alongside:
- a live-status sidebar with context rings
- approve-from-anywhere permission cards
- a config doctor
- editors for config and instruction files
- a durable history archive

The folder is empty: no git repo, only the brief. This plan answers the brief's section 8.

Every CLI fact below was checked against current docs on 2026-10-09; sources are cited inline and in the appendix.

**Rename.** Apple holds a live US AIRPORT registration in class 9 (software), registration 2640080. The app is now **Marshell** (marshal + shell); a web search found no software of that name.
- Names: `marshell` app/CLI, `~/.marshell/` config dir, `MARSHELL_` env prefix, `marshell-hook` bridge.
- Every name lives in one constants module (`crates/protocol/src/brand.rs` plus `src/brand.ts`), so a future rename is one edit.
- The Departures board keeps its name; it is a feature, not the brand.

**Decisions taken in this session:**

| Decision | Choice |
|---|---|
| Primary dev OS | Windows 11. Home path `C:\Users\Home Office` contains a space, which is a real test case. |
| Repo | Empty |
| Package manager | pnpm (via corepack) |
| License | Open source, MIT |
| Installed CLIs | Claude Code, Codex 0.162.1 (desktop-app install) and Antigravity `agy` 1.3.3, checked read-only on 2026-10-15 (`docs/research/2026-10-15-local-install-check.md`). Fixtures are recorded from the real CLIs, in a throwaway config, before phase 6. |
| Approve-from-anywhere | On by default |
| Project references | Off until enabled per project |
| History | Keep everything; backup-folder mirror available but off |
| Claude native recap capture | Yes, best-effort |
| Sounds | Soft original chimes |

Toolchain present: Rust 1.98.1, Node 24, gh, pwsh 7.6, WSL 3.

The user already has a Claude status line (`python ~/.claude/statusline.py`) and a `PreToolUse` hook, so status line chaining must work from the start.

## Corrections to the brief (verified)

1. **Antigravity CLI (`agy`) is Google's agent; Gemini CLI is dropped.** Decided 2026-10-10 ("scratch gemini, google is migrating to agy"). Gemini CLI is intentionally gone from Marshell: no adapter, mark, colour, fixture, launcher entry or copy. This paragraph is the git-greppable note; do not add Gemini back.
   - Google replaced Gemini CLI with **Antigravity CLI** for free-tier and Google One users on 2026-06-18.
   - Binary `agy`. The installer (`irm https://antigravity.google/cli/install.ps1 | iex` on Windows) puts it in `%LOCALAPPDATA%\agy\bin`; `~/.local/bin/agy` on macOS and Linux.
   - Config `~/.gemini/antigravity-cli/settings.json` (the directory name is Google's, kept from Gemini). Its `statusLine` is `{ "type": "command", "command": "<script>" }` with optional `padding`, `enabled` and `stack_with_default`. The command gets JSON on stdin: `model`, `context_window` (`used_percentage`), `quota` (one bucket per model: `remaining_fraction`, `reset_time`), `agent_state` (idle, thinking, working, tool_use), `tool_confirmation_pending`, `pending_input_count`, `execution_mode`, `transcript_path`, `version`. Marshell chains it the way it chains Claude's, so model, context % and the plan meter come through the status line.
   - Plugins (skills, agents, rules, MCP, hooks) live in `~/.gemini/antigravity-cli/plugins/<name>/`; `agy plugin list`.
   - **Hooks, checked against agy 1.3.3 and its bundled hooks doc on 2026-10-15** (`docs/research/2026-10-15-local-install-check.md`). Global hooks live in `~/.gemini/config/hooks.json` (workspace ones in `<repo>/.agents/hooks.json`), never in `settings.json`. The file holds named hooks, each `{ "enabled": true, <Event>: [...] }`. Events: `PreToolUse` and `PostToolUse` (matcher groups, `matcher` a regex on the tool name), `PreInvocation`, `PostInvocation` and `Stop` (flat handler lists). Handlers are `{ "type": "command", "command", "timeout" }`, seconds, default 30; Windows runs them through `cmd /c`. Payloads are camelCase (`conversationId`, `transcriptPath`, `modelName`, `toolCall`) and carry **no event name**, so each Marshell command passes `--event <Name>`. `PreToolUse` answers `decision: allow | deny | ask | force_ask`, so approve-from-anywhere works for Antigravity: Marshell's entry is the permission role with a 600 s timeout, and a release answers `ask` so Antigravity's own prompt takes over (to confirm in the capture). `Stop` can answer `decision: continue` and `PreInvocation` can inject steps; the bridge never sends either. The consent card adds 5 entries (4 status, 1 approve) in a new `hooks.json` under one hook named `marshell`, and chains the status line in `settings.json`; undo removes that hook and deletes `hooks.json` only if nothing else is in it.
   - Still open (needs one live capture in a throwaway profile; ask the owner first): whether an empty `PreToolUse` answer leaves the native prompt, the status line's exact keys, the transcript schema (`brain/<conversationId>/.system_generated/logs/transcript.jsonl`), the undocumented `deny_unless_prior_grant` decision. A `dangerouslySkipPermissions: true` setting (on, on the dev machine) is handled by "Session modes": the consent card offers to hand the mode to Marshell.
2. **Codex CLI**
   - It **does** have a `PermissionRequest` hook (allow or deny), so approve-from-anywhere works for Codex too.
   - Current official docs (checked 2026-10-10) specify the exact permission output contract, hook sources, event payloads, `/hooks` trust-by-current-hash flow, and the 32 KiB default **combined** instruction limit. See `docs/research/codex-cli.md` for the implementation handoff and primary links.
   - Hook integration is opt-in and user-level by default. Do not edit managed/org policy or project `.codex/` files silently. A file containing a hook is not proof the hook is trusted or running; the user must inspect and trust the current definition in `/hooks`.
   - Install into one hook representation per config layer. Codex discovers `hooks.json` and inline TOML hook tables, loads matching sources additively, and warns if both representations appear at one layer. Project hooks also require the project `.codex/` layer to be trusted.
   - `PermissionRequest` must answer with `hookSpecificOutput.hookEventName = "PermissionRequest"` and `decision.behavior = "allow" | "deny"`; no decision leaves Codex's normal prompt in place. Any deny wins across matching hooks. `PreToolUse` is not the permission grant path.
   - Hook JSON arrives on stdin. Documented events include `SessionStart`, `SessionEnd`, `SubagentStart`, `SubagentStop`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, `PreCompact`, `PostCompact`, `UserPromptSubmit`, `Stop`, and `Interrupt`. Use documented fields only and preserve raw fixtures. Most hooks default to 600 s; `SessionEnd` and `Interrupt` have a 1 s default and 3 s maximum. Set explicit bounded timeouts.
   - Codex reads one global instruction file, then at most one file per directory from project root to cwd; `AGENTS.override.md` has precedence at each level. The default 32 KiB (`project_doc_max_bytes`) cap is combined across discovered instructions. For the instruction library, inline only a previewed, tagged managed block and warn against the effective combined cap. Never replace user-authored root or nested `AGENTS.md`, or rename/convert Claude instruction files.
   - Launch the interactive `codex` CLI in Marshell's PTY. App-server is a separate JSON-RPC integration with a documented remote-TUI pairing, not an attach-to-existing-process API in the docs; do not treat its thread id as the PTY session id. App-server documents model/effort discovery, per-thread token-usage updates, and ChatGPT account rate-limit methods, but the usage payload and authentication-dependent availability require versioned verification. Real CLI fixtures must settle Windows command quoting, trust UX, transcript schema, context denominator, and resume behavior before claiming full Codex support.
3. **Claude Code**
   - `model` arrives only on `SessionStart`, not on every event.
   - `effort.level` appears only on tool-context events. Values: low, medium, high, xhigh, max; the CLI also accepts `ultracode`.
   - `PermissionRequest` has a 600 s default timeout. Exit code 2 is ignored; only a `decision` object grants or denies.
   - `additionalContext` is capped at 10,000 characters.
   - `cleanupPeriodDays` defaults to **30**, so the Marshell archive copy really matters.
   - The transcript format is officially "internal, changes between versions".
   - Inspecting a local transcript confirmed `message.usage.{input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens}`. One API message spans several JSONL lines, so **dedupe by `message.id`** before summing.
   - Native recap appears only when the terminal is unfocused. It can be turned off with `awaySummaryEnabled`, and it is capped at 400 characters. A literal `Recap:` prefix is unverified.
4. **xterm.js 6.0** removed the canvas renderer, so the fallback is the **DOM renderer**.
5. **portable-pty 0.9.0** sets `PSEUDOCONSOLE_INHERIT_CURSOR`. ConPTY then holds back all output until a cursor-position report (`ESC[6n`) is answered.
   - The core answers that query itself, until the renderer attaches.
   - Pinning 0.8.1 is the fallback.
   - Child processes also need Job Objects (kill-on-close) so that killing a session, or a crash, kills its children; phase 1 used `taskkill /T /F`, the Job Object lands in phase 2.
6. **Tauri notifications**
   - `tauri-plugin-notification` has **no action buttons on desktop** and no documented click callback.
   - On Windows, toasts only work properly for installed apps.
   - `setBadgeCount` is unsupported on Windows; use `setOverlayIcon` there.
   - Plan: per-OS notifier modules (see "Per-OS notes").
7. **tauri-plugin-store is dropped.** The core owns all app settings in `~/.marshell/`, which is required for the v2 daemon. window-state, single-instance, opener and notification are kept.

## UX and UI design (expert pass, binding for phase 0 and every UI PR)

### Phase 0: what to mock, in order, and what "approved" means

Each step builds on the one before it, so do them in this order. All of them are Storybook stories, shown in dark and light.

| # | Deliverable | Contents |
|---|---|---|
| 1 | Tokens + type specimen | Colours, spacing, radii. AA matrix for the 8 accents in both themes. Type scale with real strings. |
| 2 | Status glyph set + motion | Every glyph at 12/16 px. Live animations, plus their reduced-motion versions. |
| 3 | Sidebar row matrix | Every state in the row table below, in comfortable and expanded (there is no compact density). Truncation at a 200 px sidebar. |
| 4 | Approve card | Safe Bash, risky Bash, Edit with diff, Write of a new file, MCP tool, a 40-line command, question card, after-decision receipt, "Answered in terminal", released on timeout. |
| 5 | Main window composite | The **5-agent scenario**: 2 cards in the lane, 1 working, 1 done-unseen, 1 error. Also split view, focus mode, collapsed rail, the 720 px minimum width, and the terminal theme switching between dark and light (see "Terminal theme"). |
| 6 | Launcher + command palette | Empty, filtering, and "remembered for this project" states. |
| 7 | Onboarding | 4 screens, including the consent diff, plus every empty state. |
| 8 | Departures board | Static design plus a 10 s motion clip. |
| 9 | Doctor | Report, fix sheet, undo toast. |
| 10 | Settings page + editor | Form mode, raw mode, scope badges. |
| 11 | OS notification mocks + icon | Banners per OS. The icon at 16/32 px on light and dark backgrounds. |

Rows 7 to 11 are designed in batch 3; the decisions are recorded in "Batch 3 design decisions" below.

Stories are fed by `fake-agent` scenario scripts, so the same 5-agent run drives the stories, the E2E tests and demo mode. Motion prototypes and sound drafts are part of phase 0.

**"Approved" means all of these pass:**
- You sign off on Windows at 100% and 150% display scaling.
- A keyboard-only walkthrough of the 5-agent loop works, with no mouse.
- The **5-second test**: shown the composite for 5 s, a viewer can name which sessions need them and why.
- Reduced motion and high contrast both pass.
- Approved stories become the Playwright screenshot baseline.

**PR review gate.** Put this checklist in the PR template; each item is yes/no.
1. Is every status readable without colour (glyph plus text)?
2. Is no keyboard focus moved and no window raised by anything except a user action?
3. Does no row change height or position unless the user did something?
4. Is every new action in the palette, with a shortcut shown and rebindable?
5. Are unknown values left out of rows and the status strip (the context ring, usage line, model, effort and subagent count simply do not appear) and shown as an en dash in tables and forms (Departures, doctor, settings), with nothing guessed?
6. Is every state covered: empty, loading (over 150 ms only), error, offline core?
7. Does every motion have a reduced-motion version and use tokens only?
8. Is the copy clean: sentence case, no "successfully", no "Oops", undo path named?
9. Are the latency budgets met?
10. Does the axe check pass in Storybook?

### Layout

| Element | Spec |
|---|---|
| Window | Minimum 720×480. Below 960 px wide the sidebar auto-collapses to the rail. |
| Top bar | **One unified 40 px bar**; no separate title bar. |
| Top bar, sidebar column | macOS: traffic lights, inset 78 px. Windows: the app icon and the lowercase "marshell" wordmark (vector, Space Grotesk SemiBold) as one button that opens the app menu, then a palette button, then the sidebar toggle (folds the sidebar to the rail and opens it again). Both stay in the bar in the rail: the palette button is icon-only there and the column sizes to its content. In a window under 960 px the sidebar cannot open, so the toggle is aria-disabled and says "Widen the window to open the sidebar". |
| Top bar, main column | Session header: name · project/branch · mode chip · model/effort · subagents · context ring · port chips. Hidden while two or more panes are on screen: each pane's header names its session, and a header above the first pane read as that pane's title. A zoomed pane, or a split with room for one, keeps it. |
| Window controls | Windows: 46×40 controls at the right. Linux: native decorations plus a 36 px header. |
| Sidebar | Default 288 px, minimum 200, maximum 400, rail 52, focus mode 0. Drag the edge to resize; double-click resets. The top bar's sidebar toggle (or the "sidebar" palette command) folds it to the rail. |
| Sidebar footer | 40 px: plan ring, 5h %, and doctor glyph with issue count. |
| Needs-you lane | The header is **always present** at 28 px ("Needs you · 2" or "All clear"), so it never pops in or out. |
| Lane cards | Accordion: only the targeted card (oldest waiting) is expanded, at about 250 px (measured: 246 for a question, 255 for a command with its Always and terminal links). The others are 36 px one-liners. |
| Lane height | At most 40% of sidebar height, in whole rows, and it **never scrolls**: it shows the targeted card and as many 36 px one-liners as fit, and a "+N more" button in its header opens a menu with every waiting request. If even one 256 px card does not fit in 40% (a sidebar under about 640 px tall), it shows one-liners only. |
| Sidebar list and rail | The **session list scrolls** (the sidebar list and the rail list), with **no visible scrollbar**: wheel, touch and keys, and a soft fade at an edge only while more rows continue that way. Everything else in the main window never scrolls; the lane keeps its "+N more" button, and the all-requests menu may scroll with the themed scrollbar. |
| Shell panel | A plain shell (PowerShell on Windows, zsh on macOS, bash on Linux) in the expanded sidebar between the list and the footer, for running manual commands. It is not tied to any agent: no vendor colour, no prompt line, typing there never reaches a session. Header 32 px with the shell name, the focused session's folder, "Open in a pane" and hide/show; body 30% of the sidebar (120 to 280 px), a real terminal about 34 columns wide. It folds to its header by itself when the list would be left with under two rows, and the user's choice to hide it always wins. Ctrl+Shift+S (Cmd+S) shows or focuses it. Not in the rail, focus mode or below 960 px. |
| Right drawer | Default 360 px, minimum 300, maximum 560. It pushes the terminal, and overlays instead when the terminal would drop below 720 px. |
| Split view | **Up to four terminals on screen at once** (any number can run; this is only how many are visible). The layout button in the top bar opens a menu: 1 to 4 panes, an arrangement per count (2: side by side or stacked; 3: one main beside two, one main on top, side by side or stacked; 4: grid, side by side or stacked) and an Automatic switch. Automatic picks the arrangement that keeps the smallest pane closest to 80 by 24, judged on even panes so dragging a divider never flips it. **Every pane for an AI agent carries its own prompt line** (see Prompt line; a pane for a manual CLI has none and gives those rows to its terminal), so each such pane's terminal is its height minus the 28 px header, its prompt line and the padding. Dividers are a 1 px hairline with an 8 px hit area that overhangs the panes, so a divider costs one pixel of terminal. Each pane header (28 px) has the status glyph (the vendor mark moving while working), name, project, and zoom and remove buttons; a pane that needs you keeps a vendor tint at full strength while other inactive headers sit at 60%. **Four only when the window is big enough to use them:** a count is offered only if some arrangement gives every pane at least 60 columns by 14 rows **of terminal** (each pane's header and own prompt line come on top, and the status bar's height counts). Below that the layout shows fewer (the smallest window shows one), the layout menu greys out the counts that do not fit and says "Showing 2 of 3", and the sessions not shown keep running (the focused pane's header says "N more running"). A pane under 80 columns says its size ("39×11"). Zoom (Ctrl+Shift+P, Cmd+P on macOS) fills the view and the others keep running. A new pane starts empty and offers the sessions not on screen. In split view the focused pane has a soft ring in its focus colour around the whole pane and a raised header; the other panes sit flat. Focus crossfades in 120 ms (none under reduced motion). Terminal text is **never** dimmed. **A pane header is that pane's title bar:** the number and the name are the last things to give way (the project shrinks first). The app window's own title in the OS title bar and taskbar follows the focused session ("billing — Marshell", or "Marshell" when none is focused). The window has **one** screen-reader live region, for the focused session (empty on first render, announces only a change, never a repeat); the prompt lines in the panes are plain text. |
| Focus mode | Sidebar and header go to 0. The header reveals on an 8 px top-edge hover. A floating pill at top right reads "2 need you ⌃⇧N" and stays silent apart from its glyph bounce. |
| Prompt line | Every session on screen has its **own** prompt line, so a line of text always goes to the session of the pane it was typed in. A single terminal has one **full** line under it (**66 px**); in split view every pane has a **compact** line docked at its foot (**42 px**), including a zoomed pane. An empty pane has none, and **neither does a manual CLI** (a shell or dev tool is a plain terminal: you type in the terminal, see Session numbers and manual CLIs). The look is a **flat command-line**, not a floating card or a rounded composer: a docked bar with a prompt marker and a field, separated from the terminal by a hairline and a vendor-coloured top edge on the focused one, so it reads as part of the terminal rather than a chat box. The height is fixed in a slot, so the terminals never resize when a session starts or stops working; a long prompt grows the line upward over its own terminal while it is written, then scrolls with its scrollbar hidden. It takes its session's vendor colour. It is blocked while that session waits on the user or has ended, so typed text can never answer a request by accident; the reason sits in the line ("billing is waiting for your approval"). Only that pane's line is off: the others stay live. Enter sends (Queue while the session is working), Shift+Enter adds a line, Ctrl+Shift+L (Cmd+L on macOS) puts the cursor in the focused pane's prompt line without changing which pane is focused. Hidden in focus mode. The terminal itself stays typeable. |
| Status bar | One **24 px** bar at the foot of the centre column, the same in a single window and in split view. It carries the status strip (inspired by the user's statusline.py) for the **focused** pane's session: model and effort, context % with a tiny bar and used/size, the 5-hour and 7-day limits with "resets in", cost, session time, uncommitted changes, MCP health, version, skills, venv and ports. Unknown values are left out of the strip, never shown as a dash; items drop by priority as the window narrows (the model, context and 5-hour limit last) and never wrap; an MCP problem replaces the healthy count and outranks everything. Hidden in focus mode. Which items every CLI can supply is in `docs/research/2026-10-10-cli-data-sources.md`: only project, branch, elapsed time, ports, venv and version are universal. **It starts with the focused session's number and name** (never dropped, ellipsized only at its own 24-character cap), so the facts always say whose they are; the strip fits in what is left. |
| Rail (52 px) | Brand stripe and the agent mark per session, with a small status glyph under it (no letters). While a session works, its vendor mark carries the working loop in place of any pulsing dot. Needs-you shows as a badge stack with a count at the top. It shows the items that fit and a "+N" button for the rest. The palette and sidebar buttons stay in the top bar. |

**Terminal theme.** The terminal has its own palette, separate from the app theme.
- Setting: **Follow app** (default), **Always dark**, **Always light**, or an imported theme; a profile can override it. Switching applies to every open terminal at once through xterm's live `options.theme`, keeping scrollback.
- Switching changes the default foreground and background, cursor, selection and the 16 ANSI colours. Colours a program painted in 24-bit RGB stay as drawn, so `minimumContrastRatio` (default 4.5) is on to keep them readable.
- Programs that read the background once at startup keep their old look until restarted. Marshell helps in three ways: it answers OSC 10/11 colour queries with the current theme, sets `COLORFGBG` at launch, and sends the colour-scheme-change notice (mode 2031) to programs that opt in. The setting's help text says what a running program may not pick up.
- xterm.js answers the OSC 10/11 queries while a renderer is attached. Before one attaches, or with none attached, nothing answers, so a program that asks at that moment gets no reply.

**Hover-freeze rule.** While the pointer is inside the sidebar, lane height changes are deferred until the pointer leaves, with a 3 s cap. Meanwhile a "1 new" chip appears in the lane header.

### Session numbers and manual CLIs

**Session numbers.** Every open terminal has a number, so "go talk to session 6" names one terminal.
- **Stable, lowest free.** A session keeps its number until it closes. A new session takes the lowest number nobody holds (`nextSessionNumber`), so with 1, 2, 3, 5 and 6 open the next one is 4, and a number never moves while its session lives. Numbers are never renumbered when the list is reordered.
- **Shown on every surface that names a session:** the sidebar row (a quiet tabular numeral between the mark and the name), the rail item (a small numeral on the mark), the session header, every pane header, the lane card ("1 · billing") and so the drawer, the all-sessions menu, and the palette and launcher results. It is a numeral on a quiet fill, never an outline, and it never decides truncation: the name keeps its room. Accessible names start with it: "Session 6, billing, ...". Its tooltip teaches the phrase: "Say 'session 6' to an agent and Marshell knows which terminal you mean."
- **Keys.** Ctrl+Shift+1 to 9 (Cmd+1 to 9 on macOS) go to sessions 1 to 9, as written in section 7. A higher number is "Go to session..." in the palette (type 12).
- **Data contract (design level).** The core owns the number (`crates/protocol`, later): it is part of the session record, assigned when the session starts and released when it closes. Each session's process gets `MARSHELL_SESSION_NUMBER` (and `MARSHELL_TAB_ID`) in its environment. The future Marshell skill, plugin and `marshell` CLI resolve "session 6" through `marshell list` and `marshell send 6 ...`, so an agent in one terminal can message another.

**Manual CLIs.** A terminal is not always an AI agent. A manual CLI is **any program that is not an LLM agent**: a shell, a dev tool, a remote tool, or anything else you run by hand. The named marks below are only the ones with a known mark; every other program is still a manual CLI and gets the neutral mark with its own name.
- **Mark.** Marshell picks the mark from the session's foreground process (its executable name) and draws it the way an AI vendor's is drawn: shells (PowerShell, Bash, Zsh, Command Prompt, Git Bash, WSL), dev tools (Git, Docker, kubectl, Node.js, Python, Cargo, GitHub CLI, Terraform), remote and cloud tools (SSH, AWS, Google Cloud, Azure, Supabase, Vercel), and a neutral "terminal program" mark for anything else, which shows the executable's name. The mark changes when the foreground command changes (PowerShell, then ssh). The core finds the foreground program once a second while a command runs (not at the prompt), and PowerShell's integration reports the typed command line at once, so a known agent CLI started in a shell is promoted to its own agent row, and one started in the shell panel moves to the session list. The Phase 0 marks are simple glyphs drawn here; the programs' own logos are sourced and checked before release.
- **Still.** No thinking motion at all: the mark never animates and `working` means nothing to it. No context ring, no model or effort, no mode chip, no approve card, no plan meters. The only states are *at the prompt* (idle), *running a command* (a still glyph and "Running docker compose up") and *exited* ("Exited · code 255"). The strip and header show only what is known (project, branch, uncommitted changes, elapsed time, ports, environment), as everywhere else.
- **Colour.** One calm, low-chroma tone per group (shells, dev tools, remote tools) so a plain terminal never competes with an agent for attention; the accent still follows the focused session, and every tone holds AA like the vendor colours.
- **No prompt line.** It is a plain terminal, so the terminal itself is where you type. A manual CLI has no Marshell prompt line: no full line under a single terminal, no compact line in its pane (its pane's terminal takes those 42 px back, and the pane math counts it per pane).

### Sidebar rows

**Comfortable row, 56 px** (there is no compact density; the two modes are comfortable and expanded):
- 3 px brand stripe flush left
- 14 px padding, then the **agent mark** in a 16 px box: the vendor logo (Claude, OpenAI, Antigravity, Copilot, Cursor and others, a generic terminal mark for custom CLIs) in the agent's brand colour
- 8 px gap, then the **name** (13/500)
- 6 px gap, then project · branch (12, secondary)
- flexible space, then the right cluster: caution 12, mute 12, the **status glyph** 16, wait time or age (11, tabular), context ring 16
- 12 px right padding
- a second line: the **model** (12/500, e.g. "Opus 5.5", left out when the CLI does not report one) · the live status phrase in 12 px secondary, e.g. "Editing src/auth.ts · 2 subagents"

**Truncation order:**
1. The branch truncates first, in the middle (`feat/au…-flow`).
2. Then the project truncates, at the end.
3. Then the name truncates at the end, keeping at least 8 characters.
4. The right cluster never truncates.

The full text is in the tooltip and the screen-reader label.

**Hover never changes row height.** Hover shows the ring tooltip, and Peek after 350 ms. **Expanded** (opened with → or the disclosure control, remembered per row) adds:
- a meta line: model · effort · `Plan` pill · mode caution · subagents
- the recap with its age
- `84k / 200k · 312k in · 48k out · $1.20`

| State | Glyph | Motion (reduced-motion) | Text (line 2 / screen reader) | Row treatment |
|---|---|---|---|---|
| Idle | Hollow circle | — | "Idle" | — |
| Working | The vendor mark itself moves, one loop per vendor, signed off in phase 0 (Claude breathes: the tips draw in more than the centre; Codex turns steadily; Antigravity swims up like a jellyfish, the bell squeezing and the tentacles trailing; Copilot stays still while parts of its face flash dark, each on its own tempo; Cursor's cube stays still while the pointer cut out of it slides in, clicks and slides back; opencode steps; Amp's main arrow stays while the two secondary arrows are fired away one after another and slide back; Grok and Qwen turn back slowly; DeepSeek swims; custom agents blink only the underscore, hard on and off); no glyph in the cluster | Mark stays still; the phrase and screen-reader label carry the state | "Running npm test" | — |
| Needs you: permission | Filled rounded-square badge in the vendor's colour with a padlock (locked until you decide); "?" for a question | One bounce | "Wants to run `npm test`" | 12% tint in the vendor's colour, reason line in full-contrast text, wait timer |
| Needs you: question | Badge in the vendor's colour with "?" | One bounce | "Has a question" | Same |
| Done, unseen | Check in the vendor's colour, drawn in 200 ms | Draw once | "Done · edited 4 files" | Name at 600 weight, plus a 6 px unread dot |
| Done, seen | Thinner check in the vendor's colour | — | "Done 12m ago" | Normal |
| Error | Filled disc in the vendor's colour with a cut-out ✕ | One flash | "Stopped: rate limit" | 12% tint in the vendor's colour, reason line in full-contrast text |
| Stuck | Dashed ring | — | "Same command 9× in 4 min" | No notification |
| Muted | Bell-slash in the cluster | Badge does not bounce | suffix "muted" | Glyphs still shown |
| Elevated | Shield before the name | — | "Administrator" | Header label for the whole session |
| Auto / dontAsk / bypass | ⚠, **always visible** | — | "Full auto" | Header tinted red with a red bar on its start edge (bypass, auto); in split view each pane header carries the same tint and bar |
| Ended | Hollow square | — | "Ended · exit 0" | Text 60% |
| Limited (no hooks) | Dotted circle | — | "Status limited · set up" | Links to setup |
| Paused | Two vertical bars | — | no phrase | A frozen process tree (Pause); Continue thaws it |
| Unknown values | Nothing in a row or the strip: the ring, usage line, model, effort and subagent count are left out, not shown as a dash. (A table or a form shows an en dash instead; see Batch 3 design decisions.) | — | — | Ring hidden |

### The approve card

Content, top to bottom:
1. **Who:** badge, session name, wait time, overflow menu.
2. **What, as a verb phrase:** "Run a command", "Edit src/auth.ts (+12 −3)", "Create .env.example", "Fetch api.github.com", "Use github · create_pr".
3. **Exact payload:**
   - 12 px mono on a raised surface, wrapped and never cut in the middle.
   - After 4 lines it fades and shows "Show all 23 lines (Space)".
   - Edits show the first hunk. The cwd is shown when it differs from the session's. MCP arguments are shown as key: value pairs.
   - The full view opens in the right drawer.
4. **Risk line**, only when risky: ⚠ plus the plain reason, e.g. "Deletes files recursively outside this project".
5. **Actions:** `Deny  N`, then `Allow once  Y` as the primary, at the right on every OS. Below them is a text button that **spells out the exact rule** it would add: "Always allow `Bash(npm test:*)` in this project  A". Then "Answer in terminal  T".

**Keyboard safety:**
- The card is reachable only by deliberate focus: Ctrl+Shift+N (Cmd+N), a click, or Tab inside the lane. Terminal keystrokes can never reach it.
- **Enter never allows.**
- Buttons stay inert for 500 ms after the card gains focus or its content changes.
- There is **no global approve shortcut**.

**Risky requests:**
- Classified deterministically in the core: recursive delete, force push, `reset --hard`, curl/iwr piped into sh/iex, sudo or elevation, writes outside the cwd or to `.env`/`.ssh`/dotfiles, non-localhost network.
- Allow becomes **hold Y for 600 ms**, with a progress ring on the button.
- "Always" is hidden.
- A truncated payload must be expanded before Allow arms.
- OS notifications for risky requests show only "Review".

**After a decision:**
- The card becomes a one-line receipt ("Allowed · npm test") for 1.5 s, then exits in 140 ms. There is no undo, and the copy never implies one.
- Focus **returns to wherever you were typing**.
- Enter on a card jumps to the session instead.

### Session modes

Decided 2026-10-15: **Marshell sets the mode**, the same five for every agent CLI, instead of each CLI keeping its own. The logic is `src/features/modes/sessionModes.ts`; the flags were read from `--help` of Claude Code, codex-cli 0.162.1 and agy 1.3.3.

| Mode | What asks | Claude Code | Codex | Antigravity |
|---|---|---|---|---|
| Manual | every change | `--permission-mode manual` | `--sandbox read-only --ask-for-approval on-request` | default |
| Plan | everything; the agent only reads and plans | `--permission-mode plan` | no plan flag: read-only sandbox stands in (same start as Manual) | `--mode plan` |
| Auto-edit | everything but edits inside the project | `acceptEdits` | `--sandbox workspace-write --ask-for-approval on-request` | `--mode accept-edits` |
| Full auto | what a safety check holds back | `--permission-mode auto` (its classifier) | `--approve-for-me` (automatic review) | `--mode auto` (agy 1.4.0); Marshell's risk check still stands in for a classifier |
| Bypass | only risky requests | `bypassPermissions` | `--dangerously-bypass-approvals-and-sandbox` | `--dangerously-skip-permissions` |

- **Hybrid.** With Marshell's permission hook on, Manual, Auto-edit and Bypass start the CLI in its strictest native mode, so every request reaches the hook, and the hook answers by the mode (`decide`). Plan and Full auto keep the CLI's own mode, because their meaning lives in the CLI (planning, a classifier). With the hook off, each mode is the CLI's own flag.
- **Guard rail.** A risky request (the deterministic list in "The approve card") gets a card with hold-to-allow in every mode, Bypass included. Full auto and Bypass differ in the middle: in Full auto, what the CLI's classifier holds back comes to you; in Bypass, only the risky list does. For Antigravity both use Marshell's risk check, so they behave the same until it has a classifier.
- **Switching mid-session.** From the mode chip in the session header (a button), the row menu's Mode ▸, or the palette (`Mode: …`, unbound until a person picks keys); all three open the same inline bar. The mode a session shows is the one it **started in** (the chip says so); a change made inside the CLI is not shown until the hook reports it. When only Marshell's answer changes (`switchKind` says "instant"), it applies at once, with no keystrokes sent to the terminal. When the CLI's own mode must change (into or out of Plan, or a classifier), or when there is no hook, Marshell restarts the agent in the new mode, in the same row: a bar replaces the pane's header (the top bar when one pane is on screen) and asks "Restart 3 billing in Bypass? It ends and runs claude --continue --permission-mode bypassPermissions." (plus "may be mid-task" while it is working), with Cancel focused first, Esc cancelling, and focus returning to the terminal. The CLI resumes its last conversation (`--continue`, `codex resume --last`, `agy --continue`). The core refuses a plain shell, a removed profile, a missing folder and an agent that is not installed, and says why in a toast; it never types into the terminal for you.
- **Default and memory.** A new session starts in Manual unless the launcher remembers another mode for that project.
- **A CLI setting that skips every prompt** (Antigravity's `dangerouslySkipPermissions: true`, which is on on the dev machine; the same idea as Claude's `permissions.defaultMode: "bypassPermissions"` or Codex's `approval_policy = "never"`) makes every mode below Bypass impossible. The consent card offers "Let Marshell set the mode": the diff removes that one line, with the backup and the undo ("turns dangerouslySkipPermissions back on"). If the person keeps it, those sessions show as Bypass and the mode chip says why it can't go lower.
- **To verify in the capture** (ask first; throwaway config): that a strict start plus an `allow` answer suppresses each CLI's own prompt; that Codex's read-only sandbox raises a `PermissionRequest` for edits, so Auto-edit can allow them; that Antigravity's `force_ask` reaches a person while skip-all is on; and Codex's plan mode, if it has one.

### Key flows

**First run (4 screens, under 60 s):**
1. **Detected:** CLIs found, with versions, and "Not installed" rows.
2. **Consent, one card per CLI**, in plain words. Example: "Add 10 hook entries and chain your status line in `C:\Users\Home Office\.claude\settings.json`."
   - Toggles per capability: status, approve-from-anywhere, status line.
   - "View exact changes" opens the unified diff.
   - The backup path is shown.
   - The removal path is shown.
   - Codex adds a "Trust in `/hooks`" step.
   - The secondary option is "Skip: use limited status".
3. **Notifications:** request OS permission, with ▶ previews of each sound.
4. **Launcher:** pre-filled with the most recent git folder. Enter starts the first session.

**Launch a session:**
- Ctrl+Shift+T opens a 560 px sheet at 20% from the top.
- It reads as one sentence of chips: `[Claude] in [my-app · main] · [Plan] · [Quick] · [worktree ☐]`, followed by a prompt box.
- Typing filters projects. Tab moves between chips, and letters pick values.
- The folder field also takes a full path in the running OS's syntax, offered as a "Use <path>" row. A "Browse…" row at the end of the list, and Ctrl+O in the field, open the OS folder picker (decided 2026-10-18; built in slice 3f).
- Enter launches. The tab appears **in under 100 ms** showing "Starting Claude Code in my-app…" and never waits for the process.

**5 parallel agents (glance → jump → act):**
- **Glance:** the taskbar badge reads "2", and the lane reads "Needs you · 2".
- **Jump:** Ctrl+Shift+N focuses the oldest card.
- **Act:** Y. The next card auto-targets but does **not** take focus, so pressing Ctrl+Shift+N again is a conscious choice.
- Space opens Peek on any row, and Esc returns you to your terminal.

**Doctor fix and undo:**
1. "Fix 4 issues" opens a sheet with the combined diff, grouped by file.
2. Confirm, then rows tick off inline.
3. A toast appears: "Fixed 4 issues. 2 sessions pick this up after a restart." with [Restart them] [Undo]. It stays 10 s, then remains in the Doctor header as "Undo last fix".

**Closing a busy session:**
- Ctrl+Shift+W on an idle or done session closes it at once, with Reopen in the toast.
- If it is working or needs you, an inline header bar appears instead of a modal: "Claude is mid-task in my-app (editing 3 files)." [Cancel, which has focus] [Close when done] [Close now].

### Visual system

**Type:**

| Item | Choice |
|---|---|
| UI font | Segoe UI Variable (Windows), system-ui / SF (macOS), bundled Inter Variable (Linux) |
| Scale | 11 / 12 / **13** / 15 / 20 / 28. Weights 400/500/600. Tabular figures for every number. |
| Mono font | **Import the user's existing terminal font** from Windows Terminal or iTerm settings, so Nerd Font prompts keep working. Otherwise use bundled JetBrains Mono, 13 px. A **terminal font choice** (added 2026-10-18; proposed, to confirm when the slice is planned) lets the user pick one of three bundled fonts instead: see "Terminal font setting" below. |

**Terminal font setting** (added 2026-10-18; the option list is proposed, to confirm when the slice is planned). It governs the terminal (xterm.js) font only. The UI font stays as in the table above, and the terminal font size stays its own setting (8–32) and the Text size setting does not move it. It sits in the Appearance dialog and the settings page, next to the size. Options, all bundled with the app under the SIL Open Font License because the webview's CSP allows only the app and loopback (no web-hosted fonts), which also removes any dependency on OS fonts on Linux and macOS:
- **Match my terminal** (default): today's rule above, the user's Windows Terminal or iTerm font if found, else JetBrains Mono.
- **JetBrains Mono** (SIL OFL, bundled): the current bundled font.
- **Cascadia Mono** (SIL OFL, bundled; also ships with Windows 11): the Windows Terminal look.
- **Atkinson Hyperlegible Mono** (SIL OFL, bundled): drawn for low vision, with distinct 0/O and 1/l/I.
Every choice is monospace and falls back to the system monospace font and then to the imported Nerd Font if one exists, so powerline and icon glyphs keep working; the WebGL renderer draws box-drawing and block characters itself. The picker is a keyboard-operable list that shows each name in its own font plus a sample line, works in forced colours (it uses the system colours there, and the terminal keeps the user's font) and announces the change through the `Announcer`. Bundling adds font files only, not a code dependency, but the files and their licences need the user's approval when the slice is planned.

**Colour tokens:** the source of truth is `src/styles/tokens.css` (oklch, `light-dark()`), `accents.css` and the Storybook Colours story; this plan holds no hex values. Rules that stay binding:
- Surfaces: true-black dark, warm-white light. Hover and selected are low-alpha overlays of the text colour. Text has three levels, and `--text-3` must pass AA on every surface.
- **The accent follows the vendor.** Everything in a session's row, card or pane (its header, composer and status bar too) takes that CLI's signal colour; the window's own controls (the layout menu, the palette, the shell panel, the navigation cursor) keep the app accent: focus rings, the active-pane ring, primary buttons, the needs-you tint. Where no session is in play (onboarding, settings, doctor) the user's chosen accent applies; the default is sand, a warm dark grey in light mode and a beige in dark mode, kept apart from Codex's neutral grey and OpenAI's pale yellow (9 accents: sand, amber, blue, indigo, violet, magenta, cyan, teal, slate). Vendor colours are checked for 4.5:1 text on their button fill and 3:1 for rings and bars on every surface (`accentFollowsVendor.test.ts`).
- **No boxed outlines.** Cards, buttons, chips and keys are told apart by fill and tint, never by a 1 px box; a risky request is a tinted card with a red bar on its start edge. Hairlines stay as dividers between regions, and the keyboard focus ring stays. `noBoxedOutlines.test.ts` enforces it.
- **Scrollbars:** the main window never shows a scrollbar: only the terminal conversation and the session list scroll, both by wheel, keys and touch, with the bar invisible. Menus, settings and the request-details drawer, which may scroll, keep a quiet 12 px pill that follows the theme and strengthens on hover and drag. `noScrollInMainWindow.test.ts` scans every feature and component stylesheet.
- Status colours (error, ok, caution) are separate from the accent. Caution is yellow (dark) or olive (light) so it stays distinct from amber; `contrast.test.ts` checks AA and CIEDE2000 distance for every pair, including amber vs caution.
- Brand colours are for marks, stripes and dots and, as the accent, for the focused session's rings, bars and primary buttons (see the accent rule above). Claude's is a muted terracotta, never a status colour; Codex and Antigravity have their own tokens (OpenAI is a pale yellow in dark mode and a deep teal in light; a yellow that passes 3:1 on white looked like muddy gold). All are user-changeable.
- Custom accent hex: validated at pick time against both themes (AA for `--accent-ink`), rejected with the reason if it fails.

**Motion:**

| Interaction | Duration / easing | Reduced motion |
|---|---|---|
| Row status flash | 600 ms, accent 12%→0, ease-out | Instant tint, 300 ms |
| Needs-you bounce | Scale 1→1.2→1, 280 ms, spring, once | none |
| Done check | 200 ms draw | Static |
| Lane card enter / exit | 180 ms `cubic-bezier(.2,.8,.2,1)` / 140 ms ease-in | 80 ms fade |
| Peek / palette | 120 ms fade + 2 px / 100 ms scale .98→1 | Fade only |
| Drawer | 200 ms | Instant |

Pause all looping animation when the window is hidden.

**Sound brief:**
- One family in D major pentatonic: soft sine plus a felt-bell timbre, normalised to −20 LUFS.
- Needs you: rising D5→A5, 420 ms. The escalation adds a third note.
- Done: a resolving 3-note arrival, 600 ms.
- Subagent done: a wood tick, 80 ms, at −30 LUFS.
- Error: a falling minor third, low register, 500 ms.
- At most one sound per category per 3 s.

**Departures board:**
- True black with 44 px rows, in both themes (it is a sign, not a page).
- Columns: STATUS · SESSION · PROJECT/BRANCH · AGENT (text label in brand colour) · WAITING · CTX · RECAP.
- Uppercase mono for status and times.
- Tiles are `#0F0F0F` with a hairline split at 50% height. No yellow Solari pastiche.
- Only the status and waiting cells flip: 60 ms per character, 15 ms stagger, 400 ms cap. Reduced motion uses a crossfade.
- A "Gate calls" group sits at the top. Rows never move while the pointer is over the board.
- Arrow keys move, Enter jumps, Y/N opens an inline card.

### Microcopy

**Tone rules:**
- Sentence case, with the subject first.
- Use the agent's name, not "the AI".
- Use concrete numbers.
- Say what changes and where to undo it.
- No "!", "successfully" or "Oops".

**Examples:**
- "No sessions yet. Start one with Ctrl+Shift+T."
- "All clear"
- "Wants to run `npm test`"
- "api-server has waited 1 min to run a command"
- "3 sessions need you"
- "Stopped: rate limit reached. Resets at 3:40 pm."
- "Marshell adds 9 entries to your Claude user settings so it can see status and approvals. Your status line keeps working. A backup is saved first."
- "Claude is still working in my-app. You can resume it from History."
- "Limited status. Codex hooks aren't set up. Set up…"
- "Same command 9 times in 4 min. Might be stuck."

### UX risks

| Risk | Mitigation |
|---|---|
| Accidental approval | Focus-gated card, 500 ms arming delay, Enter never allows, hold-to-allow for risky requests, no global approve key |
| Focus stealing | Only user input moves focus. Banners never raise the window. Focus returns after acting from the lane. |
| Sidebar jitter | Fixed order, permanent lane header, hover-freeze, rows never resize on hover |
| Notification fatigue | Suppressed when focused, 3 s batching, a single escalation, subagent sounds off by default, "done" banners only for sessions in the background for more than 30 s |
| Terminal key conflicts | Ctrl+Shift layer; Ctrl+C copies only when text is selected |
| Stale or false status | Show the age of every recap. Use the limited-status glyph rather than guessing "idle". |
| Permission hold hides the native prompt | The card says "Also answerable in terminal". Typing in that tab releases the hold, and the card shows "Answered in terminal". |

### Latency budgets (part of every phase's exit criteria)

| Interaction | Budget |
|---|---|
| Typing, keypress to paint | ≤ 30 ms |
| Hook to visible card | ≤ 150 ms |
| Decision to bridge return | ≤ 50 ms |
| Tab switch | ≤ 50 ms. Keep the 4 most recent xterm instances mounted; seed the others from a vt100 snapshot. |
| Palette and launcher open | ≤ 50 ms |
| Peek | ≤ 100 ms |
| Cold start to an interactive layout skeleton (from `layout.json`) | ≤ 800 ms |

## 1. Repo structure and type generation

```
marshell/
  Cargo.toml                  # workspace
  package.json                # pnpm, single frontend package
  crates/
    protocol/                 # serde types shared by everything + ts-rs derives; brand.rs
    core/                     # the engine (lib): server, pty, sessions, adapters, config, history, doctor, notify
      src/server/  pty/  session/  adapters/{claude,codex,antigravity,generic}/  config/
          history/  doctor/  notify/  shells/  recap/  references/  ports/
      src/settings.rs         # the settings store (~/.marshell/settings.json) and its validation
      src/wt.rs               # finds Windows Terminal settings files (read only)
      src/layout.rs           # the layout store (~/.marshell/layout.json): open tabs, panes, sidebar, last launcher choice per folder
      src/server/layout.rs    # GET/PUT /v1/layout
      src/server/shell_panel.rs  # the sidebar shell panel's own pty session, kept out of the registry
    bridge/                   # two tiny console bins sharing a client module:
                              #   marshell-hook (hook + status line bridge), marshell (CLI: new/list/focus/send/uninstall)
    host/                     # marshell-host: Windows elevated pty helper (spike S8 in phase 1, ships in phase 6)
    fake-agent/               # test-only binary that emulates an agent: prints output, calls the bridge
  src-tauri/                  # thin window shell: starts core thread, window, tray, notifier, badge
  src/                        # React + TS frontend
    generated/                # ts-rs output, committed, CI checks it is fresh
    lib/api/                  # http client, pty socket, status socket
    tokens/                   # tokens.css (design tokens), themes
    styles/appearance.css     # root attributes for theme, contrast and text size
    styles/fonts.css          # the bundled JetBrains Mono
    components/               # primitives over radix-ui
    features/                 # sidebar, terminal, launcher, palette, departures, history, doctor, settings, editors
      appearance/             # Appearance dialog, provider, theme import parsers, custom accent check
      sessions/ResumeBar.tsx  # the Resume view for Ended tabs and the bar under an Exited terminal
      sidebar/grouping.ts     # project grouping of the session list (pure)
      window/viewState.ts     # split, focus mode, zoom and sidebar state, and what goes to layout.json (pure)
  stories/                    # Storybook mockups (approval gate before UI code)
  assets/{icon,sounds}/
  docs/                       # BRIEF.md (updated living spec), adr/, fixtures notes
  .github/workflows/ci.yml    # Win/Linux matrix; macOS paused until there is a Mac to test on
```

- **Type generation:** use ts-rs 12, not specta (specta 2 is still at rc.25, and its strength is tauri-specta command bindings, which we don't use).
  - Apply `#[derive(TS)] #[ts(export)]` with the `serde-compat` feature to every type in `crates/protocol`.
  - `cargo test -p protocol` writes `src/generated/*.ts`, and CI fails if `git diff` shows they changed.
  - Set `TS_RS_EXPORT_DIR=src/generated` in `.cargo/config.toml` `[env]`; ts-rs writes to `./bindings` by default.
  - ts-rs maps `u64`/`i64` to `bigint`, but JSON sends plain numbers. Every such field (seq, at_ms, durations, token counts, resets_at) gets `#[ts(type = "number")]`. A unit test fails if any exported `.ts` file contains `bigint`.
- **Why the frontend talks only to the core API:** the core can later move into `marshelld` without touching the UI.
- The hook bridge depends only on `protocol` and a blocking HTTP client (`ureq`), with no tokio. That keeps startup in milliseconds.

## 2. Adapter trait and SessionEvent (Rust)

Plain sync trait objects (`Box<dyn Adapter>`), no lifetimes or async in the trait. Slow work runs in `spawn_blocking`.

```rust
pub trait Adapter: Send + Sync {
    fn id(&self) -> AgentId;                         // "claude" | "codex" | "antigravity" | "custom:<slug>"
    fn display_name(&self) -> &str;
    fn brand_color(&self) -> &str;                   // default; user-overridable
    fn capabilities(&self) -> Capabilities;          // hooks, permission_hook, context_injection, resume, fork, effort, statusline …
    fn detect(&self, env: &ResolvedEnv) -> Detection;          // installed, version, path
    fn launch_command(&self, req: &LaunchRequest) -> LaunchSpec; // mode, model, effort, worktree, first prompt
    fn resume_command(&self, session_id: &str) -> Option<LaunchSpec>;
    fn fork_command(&self, session_id: &str) -> Option<LaunchSpec>;
    fn hook_install_plan(&self, bridge: &Path) -> Result<ChangeSet>;   // returns a plan; never writes
    fn hook_uninstall_plan(&self) -> Result<ChangeSet>;
    fn hooks_installed(&self) -> HookInstallState;                     // Installed | Partial | Missing | Untrusted
    fn normalize(&self, raw: &RawHookEvent) -> Vec<SessionEvent>;
    fn permission_response(&self, d: &PermissionDecision) -> Option<serde_json::Value>;
    fn context_response(&self, at: ContextPoint, text: &str) -> Option<serde_json::Value>; // PromptSubmit | SessionStart
    fn config_sources(&self, cwd: Option<&Path>) -> Vec<ConfigSource>;
    fn doctor_checks(&self) -> Vec<CheckSpec>;
    fn headless_summarize(&self, transcript: &Path, model: &str) -> Option<LaunchSpec>;
    fn usage_source(&self) -> UsageSource;           // StatusLine | Transcript | None
    fn history_sources(&self) -> Vec<WatchRoot>;
    fn parse_transcript(&self, path: &Path) -> Result<Conversation>;
}

#[derive(Serialize, Deserialize, TS, Clone)]
#[serde(tag = "type")]
pub enum SessionEvent {
    SessionStarted { agent_session_id: Option<String>, model: Option<String>, source: Option<StartSource> },
    SessionLinked { previous: String, reason: LinkReason },          // resume | fork | compact | clear
    PromptSubmitted { text: Option<String> },
    Working,
    ToolUse { name: String, target: Option<String>, phase: ToolPhase, ok: Option<bool>, duration_ms: Option<u64> },
    NeedsInput { kind: NeedsInputKind, text: Option<String> },      // Permission | Question | Elicitation | Idle
    PermissionRequested { request_id: RequestId, tool: String, detail: PermissionDetail,  // Bash{cmd,cwd} | Edit{path,hunks,add,del} | Write{path,preview} | Fetch{url} | Mcp{server,tool,args}
                          risk: Risk /* {level, reasons} */, always_rule: Option<String> /* exact rule text shown on the card */ },
    PermissionResolved { request_id: RequestId, by: ResolvedBy },  // App | Terminal | Timeout
    SubagentStarted { id: String, kind: Option<String> },
    SubagentStopped { id: String },
    TurnDone { last_message: Option<String> },
    TurnFailed { reason: String },
    ModelChanged { model: String },
    EffortChanged { level: String },
    PermissionModeChanged { mode: PermissionMode, raw: String },   // shared set + CLI's own label
    TitleChanged { title: String },
    RecapLine { text: String, source: RecapSource },              // Events | Smart | Native
    ContextUsage { tokens_in_context: u64, window_size: u64, pct: f32 },
    SessionUsage { input_tokens: u64, output_tokens: u64, cache_read: u64, cache_write: u64, cost_usd: Option<f64> },
    PlanUsage { five_hour_pct: Option<f32>, seven_day_pct: Option<f32>, resets_at: Option<i64> },
    CwdChanged { path: String },
    CommandStarted,
    CommandFinished { exit_code: Option<i32> },
    Attention { via: AttentionVia },                               // Bel | Osc9 | Osc777
    SessionEnded { reason: Option<String> },
}
// Envelope: EventEnvelope { tab_id: Option<TabId>, agent: AgentId, at_ms: u64, source: Hook|StatusLine|Pty|Transcript|Core, event }
```

`PermissionMode` is the shared set {Manual, Plan, AutoEdit, FullAuto, Bypass} that Marshell sets (see "Session modes"); the CLI's own label is kept in `raw`.

Session state is exactly as in brief §4, plus `unseen_done`, `waiting_since_ms` and `seen_at`. These live in Rust and are driven by `ClientState`. `subagents` is a `HashSet<String>`, and unknown values are `None`, which the UI leaves out.

## 3. IPC contract

- **Endpoint:** `http://127.0.0.1:<random>/v1`. Every request carries `Authorization: Bearer <token>`.
  - Browsers can't set headers on a WebSocket, so the WS token travels as the subprotocol `marshell.v1, token.<t>`.
- **The window gets the port and token** from a single Tauri command, `core_endpoint()`.
- **Bridge and CLI** read `~/.marshell/run/endpoint.json`, which holds `{port, token, pid, started_at}` and is readable by the user only (0600 on Unix, owner-only ACL on Windows).

**HTTP**

| Area | Routes |
|---|---|
| Sessions | `GET/POST /sessions` · `PATCH /sessions/:id` (rename, mute, color) · `POST /sessions/reorder` · `DELETE /sessions/:id?force` · `POST /sessions/:id/input` (broadcast, CLI send) · `GET /sessions/:id/peek?lines=30` |
| Permissions | `POST /permissions/:rid/decision {behavior, scope: once\|session\|project}` · `POST /permissions/:rid/release` (answer in terminal) |
| Hooks | `POST /hook` (bridge only; see §5) |
| Agents | `GET /agents` · `GET /agents/:id/hooks/plan` · `POST /agents/:id/hooks/apply {plan_hash}` |
| Shells / settings | `GET/PUT /profiles` · `GET/PUT /app-settings` |
| Config | `GET /config/sources?agent&cwd` · `GET /config/file?path` · `POST /config/preview` (ChangeSet → diffs) · `POST /config/apply {changeset_id, expected_hashes}` · `GET /snapshots` · `POST /snapshots/:id/restore` |
| Doctor | `POST /doctor/run` · `GET /doctor/report` · `POST /doctor/fix {item_ids}` · `POST /doctor/undo` |
| History | `GET /history?filters` · `GET /history/search?q` · `GET /history/:id` · `GET /history/:id/recording` · `POST /history/:id/resume\|fork\|export` · `PATCH /history/:id` |
| References | `GET/PUT /projects/:id/references` |

**WebSockets**
- **`/v1/pty/:tab`**, binary frames. `seq` is the **byte offset** of the stream, so the 1 MiB unacknowledged limit is simply `sent − acked`.
  - Server → client: `[0x01][seq u64][bytes]` for output, `[0x02]` for exit, `[0x03][seq]` + screen snapshot for a reset.
  - Client → server: `[0x10][bytes]` for input, `[0x11]` + JSON `{cols, rows}` for resize, `[0x12][acked_seq u64]` for ack, `[0x13][resume_from u64]` for resume.
  - On resume:
    - If the position is still in the ring buffer, the core replays bytes from there.
    - Otherwise it sends a reset: the current screen rendered from the session's `vt100::Parser`, then the live stream. This avoids broken escape-sequence state from replaying mid-stream.
- **Browser rules for the loopback server.** The webview origin (`tauri://localhost` / `http://tauri.localhost`) differs from `127.0.0.1`. Therefore:
  - add a `tower-http` `CorsLayer` that allows only the Tauri origins;
  - set CSP `connect-src http://127.0.0.1:* ws://127.0.0.1:*`;
  - reject any `Host` header that isn't `127.0.0.1:<port>`, to block DNS rebinding;
  - make the WebSocket upgrade echo the `marshell.v1` subprotocol (`WebSocketUpgrade::protocols`), or browsers drop the connection.
- **`/v1/status`**, JSON.
  - It opens with a `Snapshot {sessions, lane, plan_usage, doctor_summary}`.
  - After that, the core sends a `Patch[]` batch every 100 ms: `SessionPatch`, `PermissionCard`, `NotifyRequest`, `DoctorStatus`. `PermissionCard` and needs-you patches **skip the batch** and go out immediately, to meet the 150 ms hook-to-card budget.
  - The client sends `ClientState {focused: bool, active_tab, visible_tabs}`, which the notification rules and the native-recap focus trick both need.
- **Tauri events** are OS-level only: notification activated → `{tab_id}`, single-instance argv, tray menu actions.
- **The window shell subscribes to `/v1/status` like any other client** and turns `NotifyRequest` into banners and badges. This keeps v2's daemon move a packaging change.

## 4. Data model: where state lives

The Rust core is the only source of truth.
- `CoreState` lives behind `Arc<Mutex<…>>`, with a `tokio::broadcast` channel for patches.
- The frontend uses one Zustand store that mirrors snapshot plus patches. Local UI-only state (collapsed panes, the active drawer) stays in React.

```
~/.marshell/
  run/endpoint.json         bin/marshell-hook(.exe), bin/marshell(.exe)   # copied from the bundle on launch, hash-checked
  settings.json             # app settings (theme, accent, notif rules, sounds, quiet hours)
  layout.json               # tabs, order, groups, split, last launcher choice per project
  profiles.json  agents.json (custom agents)
  managed.json              # manifest of every entry Marshell wrote into any CLI config (path, pointer, tag, hash)
  agents/claude/statusline-chain.json   # the user's original statusLine, verbatim
  snapshots/<id>/…  snapshots/index.json
  history/index.sqlite      history/archive/<agent>/<session>.jsonl.zst   history/recordings/<tab>-<ts>.cast.zst
  instructions/             # user-level instruction library
  projects/<id>/            # project map, aliases
  shell/                    # init scripts: marshell.ps1, .bash, .zsh, .fish
  logs/
```

- **SQLite** (rusqlite 0.40, `bundled`; FTS5 is enabled in the bundled build).
  - Tables: sessions, chains, turns, tool_calls, files_touched, tags, bookmarks, and an FTS5 table over prompts, assistant text, tool calls and paths.
- **Scrollback:** each session keeps two things in Rust.
  - A byte ring buffer sized for at least 10k lines, used for replay.
  - A `vt100::Parser` holding the screen state. Peek reads its last 30 rendered rows, which works even for TUIs like Claude's that redraw with cursor moves. Reconnect resets also come from it.
- **Concurrency rule (simple Rust).**
  - Lock, change, drop the guard, then `.await` or do I/O. Never hold a `MutexGuard` across `.await` or I/O.
  - Blocking portable-pty reads run on a plain `std::thread` per session, which forwards over a tokio channel.
  - Prefer a few small mutexes (sessions, permissions, settings) over one giant one.

## 5. Hook bridge protocol

- **Invocation.** The bridge is written as `"<home>/.marshell/bin/marshell-hook" <role> --agent claude --tag mshl1`. Forward slashes; always quoted because the home path has a space.
  - Roles: `event`, `context`, `permission`, `statusline`, `record` (fixture capture).
- **Install location and updates.**
  - The app copies the bridge to `~/.marshell/bin` on launch.
  - A running `marshell-hook.exe` is locked on Windows, for example during a permission hold or a status line run. So the copy writes `marshell-hook.new.exe`, renames the old file aside (Windows allows renaming a running exe), renames the new file into place, and deletes leftovers on the next launch.
- **Discovery.** `MARSHELL_HOME` (tests only), otherwise `~/.marshell/run/endpoint.json`.
  - If the file is missing or the connection is refused, exit 0 immediately with no output. This is the case when an agent runs while the app is closed.
  - There is no pid check: a refused loopback connection already fails instantly, and pids get reused.
  - Tab id comes from `MARSHELL_TAB_ID`. If it is absent (a session started outside the app), the core uses the event only for history linking.
- **Request.** `POST /v1/hook` with headers `Authorization`, `X-Marshell-Agent`, `X-Marshell-Tab`, `X-Marshell-Role`, `X-Marshell-Version`. The body is raw stdin, capped at 1 MiB.
- **Timeouts.** Connect timeout is 50 ms on every role; it is loopback, so a connection succeeds or is refused at once.

  | Role | Budget | Behaviour |
  |---|---|---|
  | `event` | 400 ms | Response ignored. |
  | `context` (UserPromptSubmit / BeforeAgent) | 150 ms (resolver must answer in 100 ms) | Prints the adapter's `additionalContext` JSON only when there is a match. |
  | `permission` | Configurable hold, default 120 s; the hook's own timeout is set to 600 s | Returns `hookSpecificOutput.decision`. |

- **Permission scopes** map to Claude `updatedPermissions`:
  - "Allow always (this project)" → `addRules` to `localSettings`.
  - "Allow for session" → `addRules` to `session`.
  - The card always says which.
- **Permission releases.** If the hold expires, the user clicks "Answer in terminal", or the user starts typing in that tab, the core releases the request. The bridge then exits 0 with no output and the native prompt takes over. Typing means keys, pastes and mouse button presses. Reports the terminal sends by itself (focus changes, mouse motion, the wheel, replies to queries) don't release, because Claude enables focus and any-motion mouse reporting (S9, 2026-10-18).
- **Failure rule.** On any error the bridge exits 0 with empty stdout, never blocks, and appends a single line to `logs/bridge.log`.
- **Status line chaining.**
  - On install, the user's `statusLine` object is saved verbatim to `statusline-chain.json`.
  - `statusLine.command` becomes `"C:/Users/Home Office/.marshell/bin/marshell-hook.exe" statusline --agent claude --tag mshl1`: the full quoted path, like every hook command.
  - The bridge:
    1. forwards stdin to the core on a background thread with a 100 ms cap;
    2. runs the saved original command through the same shell Claude Code uses (determined in the spike), piping the same stdin;
    3. prints its stdout unchanged;
    4. on any failure, still prints the original's output.
  - Uninstall restores the saved object byte-for-byte.
  - The doctor checks that the chain is intact.
  - Fallback if the spike shows quoting problems: `marshell-hook tee | <original>` in the command string itself.
- **Session tokens.** On each `Stop`, the core tail-reads the transcript, dedupes by `message.id`, and sums the usage fields.

## 6. Config safety layer (`crates/core/src/config/`)

- **ChangeSet.** `{id, title, reason, edits: Vec<FileEdit>}`.
  - Each `FileEdit` is `{path, scope, op, expected_hash, applies: Live | NewSessions | RestartSession}`.
  - `op` is one of: `JsonSet/JsonRemove` (by pointer), `TomlSet/TomlRemove`, `MarkdownBlock{tag}`, `CreateFile`, `Restore`.
- **Preview before write.** Every write path (installer, doctor, editors, settings) goes `preview → user sees unified diff → apply`.
- **Minimal edits.**
  - JSON: CST edits only, through `jsonc-parser` (cst feature), so formatting and key order survive. There is no whole-file re-serialize fallback. If the CST can't parse a file, fail closed: report it as "config doesn't parse" in the doctor and don't edit it. `serde_json` with `preserve_order` is used for reading only.
  - TOML: `toml_edit`.
  - Markdown: tagged blocks, `<!-- marshell:begin id=… hash=… -->` … `<!-- marshell:end -->`.
- **Apply steps.**
  1. Take a per-path lock.
  2. Check that the current hash equals `expected_hash`; otherwise conflict and re-preview.
  3. Snapshot the original bytes.
  4. Write the temp file in the same directory and fsync.
  5. Rename (retry with backoff on Windows sharing violations).
  6. Record the new hash so the watcher ignores our own write.
  7. Update `managed.json`.
- **Tagging.**
  - JSON has no comments, so hook entries are identified by the bridge path plus `--tag mshl1` in the command string.
  - `managed.json` is the uninstall source of truth, with a tag scan as a fallback.
- **Guards.**
  - Managed and organization paths per OS are hard-rejected by `apply`.
  - Elevation commands are never run by this layer.
  - Codex hook installs end with a "trust these hooks in `/hooks`" step.
- **Watching.** `notify` with a 200 ms debounce. If an open editor buffer is dirty, offer a 3-way merge (`diffy`) instead of overwriting.
- **Undo.** Every apply is one snapshot; "Undo last fix" restores it if the file still matches the post-write hash, otherwise it shows a diff. Snapshot history is browsable per file.
- **Uninstall** (`marshell uninstall` and Settings → Advanced): run every adapter's unhook plan, restore the status line, delete `~/.marshell/bin`, and ask whether to keep the archive.

## 7. Per-OS notes

**Platform status.** Windows 11 is the only platform hand-tested for now. Linux and macOS are planned and designed for (the table below, the Mac shortcut labels in the stories, the three-OS CI matrix), but nothing is signed off on them until the owner has hardware to test on. Treat every macOS and Linux row as a plan, not a verified fact.

| Area | Windows | macOS | Linux |
|---|---|---|---|
| PTY | ConPTY via portable-pty 0.9. Answer DSR; Job Object kills the process tree; treat exit code 259 as "still running" properly; sideloaded `conpty.dll` + OpenConsole next to the exe (ships in phase 2: about 4x throughput on short-line floods, MIT notice required). | forkpty; kill the process group | forkpty; kill the process group |
| Shells | pwsh 7, Windows PowerShell, cmd, Git Bash (registry `GitForWindows\InstallPath`), WSL distros (`wsl -l -q` outputs UTF-16) | `/etc/shells`, login shell | `/etc/shells`, login shell |
| PATH | Inherited | GUI apps lack the shell PATH, so resolve `$SHELL -lic env` once for `detect()` and headless runs | Same as macOS |
| WSL agents | Separate config root per distro. Hooks inside WSL call the Windows bridge through interop (`/mnt/c/.../marshell-hook.exe`), with `WSLENV=MARSHELL_TAB_ID/u:MARSHELL_AGENT/u`. Spike. | n/a | n/a |
| Notifications | Banners are always sent **silent**; our own rodio sounds play instead. First evaluate one cross-platform crate with actions and a click callback (e.g. `user-notify`; verify it in S3), and write per-OS code only where it falls short. Per-OS fallback: `tauri-winrt-notification` toasts with buttons and an activation callback. Needs an AUMID, which the installed app has (dev builds show as PowerShell). Overlay icon for the count; `requestUserAttention` to flash. | Per-OS fallback: `UNUserNotificationCenter` with Allow/Deny categories and a click callback. Needs a signed bundle. Dock badge via `setBadgeCount`. | Per-OS fallback: `notify-rust` over D-Bus with actions and a default action (GNOME/KDE). Badge only where the Unity LauncherEntry API exists. |
| Fallback for all three | Activation through a `marshell://focus/<tab>` deep link plus single-instance | Same | Same |
| DND for our sounds | `SHQueryUserNotificationState` | Best effort (no public Focus API without an entitlement) | GNOME `show-banners` / KDE `Inhibited` |
| Webview | WebView2 (bootstrapper embedded) | WKWebView. Dead-key bug xterm #5894 needs a host-side dead-key shim. | WebKitGTK 4.1. WebGL `preserveDrawingBuffer:true`; set `WEBKIT_DISABLE_COMPOSITING_MODE` / `WEBKIT_DISABLE_DMABUF_RENDERER` in `main()` only when the user hasn't set them; `transparent:false`; IME commit guard; build the AppImage on debian:12. |
| Title bar | Custom Fluent-like | Native traffic lights (Overlay) | Native decorations |
| Claude paths | `%USERPROFILE%\.claude`; managed `C:\Program Files\ClaudeCode\managed-settings.json` | `~/.claude`; managed `/Library/Application Support/ClaudeCode/` | `~/.claude`; managed `/etc/claude-code/` |
| Codex paths | `%USERPROFILE%\.codex` (or `CODEX_HOME`): `config.toml`, `hooks.json`, `AGENTS.md`, `sessions/YYYY/MM/DD/rollout-*.jsonl[.zst]`; admin `requirements.toml` (on Windows `%ProgramData%\OpenAI\Codex\requirements.toml`) is read-only to us; `notify` may already be taken (the desktop app uses it), so never claim it | `~/.codex`, same layout | `~/.codex`, same layout |
| Antigravity paths | `%USERPROFILE%\.gemini\antigravity-cli\settings.json`, `plugins\<name>\`; binary `%LOCALAPPDATA%\agy\bin\agy.exe`; hooks `%USERPROFILE%\.gemini\config\hooks.json`; transcripts `antigravity-cli\brain\<conversationId>\.system_generated\logs\transcript.jsonl`, but read the path from the payload (`transcriptPath` in hooks, `transcript_path` in the status line) | `~/.gemini/antigravity-cli`, same layout; `~/.local/bin/agy` | same as macOS |
| Packaging | NSIS per-user (no admin) + MSI | dmg, universal binary | AppImage, deb, rpm |
| Signing | Azure Artifact Signing | Developer ID + notarization, hardened runtime. Sidecar bins (`marshell-hook`, `marshell`, `marshell-host`) are signed too. | AppImage GPG, rpm key |
| Elevated sessions | `marshell-host --elevated` via ShellExecute `runas`; token loopback; accepts only input and resize | `sudo -i` in the tab | `pkexec` / `sudo -i` |
| Sound | rodio 0.22 (WASAPI) | rodio (CoreAudio) | rodio; ALSA build dependency |

Tray mouse events are not emitted on Linux, so on Linux the tray is menu-only.

**Keyboard routing (a recorded deviation from the brief).** Several brief shortcuts are terminal control keys: Ctrl+H is Backspace, Ctrl+K kills a line, Ctrl+B and Ctrl+F move the cursor, and Ctrl+\ is SIGQUIT. So:
- Every key goes through xterm's `attachCustomKeyEventHandler` and a single keymap.
- App shortcuts are **Cmd+key on macOS** and **Ctrl+Shift+key on Windows and Linux**, as in Windows Terminal and VS Code's terminal. Examples: palette Ctrl+Shift+K, history Ctrl+Shift+Y, search in scrollback Ctrl+Shift+F, split Ctrl+Shift+D, bookmark Ctrl+Shift+B, launcher Ctrl+Shift+T, reopen closed Ctrl+Shift+Alt+T, close Ctrl+Shift+W.
- Cheat sheet: Ctrl+Shift+O (Cmd+O on macOS). It was Ctrl+Shift+/, but punctuation keys sit elsewhere (or behind AltGr) on other layouts, so no shortcut uses punctuation.
- Sessions 1 to 9 are Ctrl+Shift+1 to 9 (Cmd+1 to 9 on macOS), not Ctrl+1 to 9: terminals read Ctrl+2 to Ctrl+7 as control codes (NUL, ESC, FS, GS, RS, and US, which is readline's undo) and vim uses Ctrl+6. F2 stays as written. Peek is Space on a focused row.
- The approve-card keys Y/N/A/T are active only while the card has focus.
- Global search moves to Ctrl+Shift+Alt+F.
- Shift+F10 or the Menu key opens the focused session row's menu, as in Windows Explorer.
- Ctrl+Space on a focused session row (and Ctrl+click, Cmd+click on macOS) adds that session in a pane, or removes its pane if it is on screen.
- The cheat sheet shows the real bindings per OS, and all bindings are rebindable.
- **The needs-you lane is ordered oldest-waiting first** (a deviation from the brief's "newest first"), so waiting is first-in, first-out; Ctrl+Shift+N follows that order.

### Keyboard navigation and shortcuts

Two layers, so that nothing the app does ever takes a key a terminal program needs.

**1. The app layer** is one chord per command: **Ctrl+Shift+key** on Windows and Linux, **Cmd+key** on macOS. Keys are matched by **physical position** (`KeyboardEvent.code`: `KeyK`, `Digit1`, `F2`), never by the character they type, so a shortcut sits on the same key of a French, German, Dvorak, Hebrew or Cyrillic keyboard, and a Hebrew layout cannot make "K" disappear. **No shortcut uses punctuation**, because punctuation moves between layouts or needs AltGr. Bindings are rebindable and every rebinding is checked by a pure function against the other bindings and a reserved list (below), which gives the reason in a sentence.

**2. The navigation layer** moves the selection. It lives on a **navigation modifier that terminals never need: Ctrl+Shift, on every system** (shown as ⌃⇧ on macOS). It is a setting; the settings page shows each choice with the systems where it clashes:

| Modifier | Windows | macOS | Linux |
|---|---|---|---|
| **Ctrl+Shift** (default) | Free. Ctrl+Shift alone changes the input language only when released with no other key, so holding it with Tab is safe. | Free. Cmd+Tab is the system app switcher and never reaches an app, so the chord cannot use Cmd. | Free. |
| Ctrl+Alt | Taken: Windows reads it as AltGr, which types @ { [ on many layouts. | Taken when VoiceOver is on (Ctrl+Option is its key). | Taken: workspace switching on GNOME and KDE. |
| Alt+Shift | Taken: switches the keyboard layout. | Taken: types symbols. | Taken: switches the keyboard layout. |

How it works (a pure, tested state machine; the window only measures rectangles and applies effects):
- **Terminals are selected, not just focused.** Exactly one terminal on screen is selected, or none. Selecting means it gets the keyboard, its pane takes the accent, the header and the status bar show it. Plain keys always go to the program in the terminal; Tab, Esc and the arrows are never taken there.
- **Modifier+Tab** selects the next terminal **on screen**, in order, and wraps. Empty slots are skipped (nothing to select in them). It never reaches the sidebar, however often it is pressed. The modifier already holds Shift, so Tab cannot also mean "backwards"; the arrows go the other way.
- **Hold the modifier and use the arrows** to move a **cursor** anywhere by where things are on screen: across the (up to four) panes, down the sidebar list, up to the top bar. The cursor is a tinted fill with a bar on its start edge and a one-line hint ("Release Ctrl+Shift to select · Backspace for none"). **Releasing the modifier selects what the cursor is on.** Leaving a pane to the left lands on that pane's own session in the list. A session that is not on screen is **brought into the focused pane** (the first empty slot when none is focused, the first slot when all are taken), then selected; the session it replaces keeps running. A top-bar control is activated, and the selection stays.
- **Only the modifier and the arrows reach the sidebar list**, or the mouse. Tab alone never does. Inside a terminal or the prompt line, Ctrl+Shift+arrow selects a word, so the arrows start navigation only after a Tab has been pressed in that hold; on a button or on nothing, they start it directly.
- **Select none:** **Modifier+Backspace** (works everywhere: Ctrl+Shift+Esc is Windows Task Manager and never arrives). Plain **Esc** also selects none when focus is on a control or nothing, never inside a terminal (Esc interrupts Claude Code, closes vim and fzf). While a cursor is out, Esc (macOS, Linux) cancels the move and keeps the selection; on Windows, move the cursor back or release on the pane you came from.
- **With nothing selected:** no pane is lit, the accent is neutral, typing goes nowhere, and the status bar says "No session selected · Tab to select a terminal". Plain Tab on nothing selects the first terminal; plain Tab on a button keeps moving through the page, so it stays keyboard-reachable.
- **Screen readers:** one polite live region says "Selected session 6, billing", "Cursor on session 3, api-server", "No session selected". Nothing is announced when the window opens.
- Losing window focus while the modifier is down cancels the cursor (the key-up never arrives).
- The prerequisites: the webview's own accelerators (reload, print, find, devtools, zoom) are switched off, so no browser shortcut shadows an app one (Tauri/WebView2 `AreBrowserAcceleratorKeysEnabled = false`; the equivalents on WKWebView and WebKitGTK).

**The audit.** Every command checked against the system, the webview, text boxes and terminal programs. Moved commands are in bold.

| Command | Was | Now (Win/Linux · macOS) | Why |
|---|---|---|---|
| Prompt line focus | M | **L** · ⌘L | ⌘M is Minimize on macOS. |
| History | H | **Y** · ⌘Y | ⌘H is Hide on macOS (and Y matches browsers' history key). |
| Zoom pane | Z | **P** · ⌘P (Pane) | ⌘Z is Undo and Ctrl+Shift+Z is Redo in the prompt line. E was rejected: Ctrl+Shift+E is the IBus emoji picker on Linux. |
| Shell panel | J | **S** · ⌘S (Shell) | Ctrl+Shift+J opens the devtools console in a development webview. |
| Split | \ | **D** · ⌘D | Punctuation. D matches iTerm2's split. |
| Focus mode | (none) | **I** · ⌘I | Decided 2026-10-16. M, H, Q and A are window or app actions on macOS, X is cut, and U, E, C, V, Z and J are taken as above. Grouping by project has no key: it is a palette command. |
| Review task | (none) | **R** · ⌘R | Decided 2026-10-16; the only free letter left. Its keydown is always prevented, so it never reloads the webview. |
| Cheat sheet | / | **O** · ⌘O (Overview) | Punctuation. |
| Sessions 1 to 9 | Ctrl+1 to 9 | **Ctrl+Shift+1 to 9** · ⌘1 to 9 | Terminals read Ctrl+2 to Ctrl+7 as control codes; vim uses Ctrl+6. |
| Next / previous pane | ] [ | **Modifier+Tab** (no previous: the arrows) | Punctuation; needs AltGr on many keyboards. |
| Start session, close, reopen, find, global search, bookmark, palette, next waiting, rename (F2) | unchanged | unchanged | No clash found. Ctrl+Shift+U and Ctrl+Shift+E (GTK/IBus), Ctrl+Shift+C and V (terminal copy and paste), Ctrl+Shift+Esc and Z were avoided on purpose. |

Known soft spots, left as they are: on a Mac laptop F2 needs Fn unless "use F-keys as standard" is set (the palette has Rename); Ctrl+Shift+N is "new incognito window" in browsers but the webview has no such command.


## 8. Spikes first (each has pass/fail)

| # | Spike | Pass when |
|---|---|---|
| S1 | Webview gate, Linux + macOS | (a) output paints while idle with no input; (b) dead keys and IME work (with shims); (c) `yes \| head -n 2000000` and `cat` of a 50 MB file don't stall the UI, and typing stays under 30 ms; (d) WebGL works or the DOM renderer is acceptable. **Fail without a clean fix → Electron shell.** |
| S2 | ConPTY + PowerShell on Windows | pwsh, cmd, Git Bash and WSL all render; resize reflows; DSR is answered with no hang on 0.9; killing a tab leaves no orphans (Job Object); PSReadLine predictions render; ESC/Alt keys work. |
| S3 | Notification click-to-focus, all three OSes | Clicking a banner focuses the window and selects the tab; Allow/Deny buttons work on Windows and macOS, or the deep-link fallback works. |
| S4 | Status line chaining | The user's real `statusline.py` output is unchanged through the chain on Windows (Git Bash or pwsh resolution), macOS and Linux; the core receives every update; uninstall restores the original byte-for-byte. |
| S5 | Native recap trigger | Sending focus-out (`CSI O`) to an inactive tab's pty (Claude enables DECSET 1004) yields the native recap after 3 minutes and 3 turns; record its exact format. |
| S6 | WS throughput | 8 ms / 64 KB coalescing plus 1 MiB backpressure: 50 MB `cat` is under 2 s end to end, memory stays bounded, and typing in another tab stays under 30 ms. |
| S7 | FTS5 | rusqlite `bundled` creates an FTS5 table and matches on all three CI runners. |
| S8 | Elevated helper, Windows | A UAC prompt appears; the helper connects back with the token; a non-elevated core can only send input and resize; the elevated tab renders like any other. If it can't be made safe → separate elevated window. |
| S9 | Permission hold vs terminal prompt | While the bridge holds a `PermissionRequest`, check whether Claude's own dialog is visible. Then confirm the release path (exit 0, no output) brings the native prompt back immediately. This sets the default hold time. |
| S10 | Windows hook path with spaces | A quoted bridge path under `C:\Users\Home Office` runs from Claude's hook runner and status line on this machine. |
| S11 | WSL bridge | A hook from Claude inside WSL reaches the Windows core through interop, with the tab id carried by `WSLENV`. |

S10, S4 and S9 ran on 2026-10-18 against Claude Code 2.1.302 on Windows and passed (`docs/spikes/`). Hooks install in exec form, the status line chains with no fallback, and the hold stays at 120 s because Claude's own dialog stays visible during it. S4's uninstall row and the macOS and Linux rows are still open.

## 9. Milestones (phases 1–9 = v1)

Decisions from the 2026-10-10 coverage audit (`docs/superpowers/plan-coverage-audit.md`) are folded in below and in "Cross-cutting requirements".

0. **Design (blocks UI work).** Three sign-off batches, each reviewed in Storybook and a published private preview:
   - Batch 1: tokens + type, status glyphs + motion, sidebar rows (deliverables 1–3).
   - Batch 2: approve card, main window composite, launcher + palette (4–6).
   - Batch 3: onboarding + empty states, Departures, doctor, settings + editor, OS notification mocks + icon, tray menu (7–11).
   - Also: sound drafts, the PR template file (10-item gate), demo mode as a `fake-agent` scenario, Playwright baseline from approved stories.
   - Icon done 2026-10-09 ("Night shift"; `assets/icon/app-night-shift.svg`, alternates `alt-daylight.svg`, `alt-signal-tile.svg`, mono tray `tray-mono.svg`).
   - Exit: your approval of each batch.
1. **Scaffold + one live tab + gate.** (merged, PR #1)
   - Scope: workspace, core thread, axum server with token, CORS/CSP/Host checks, pty WebSocket with coalescing, backpressure and the vt100 screen, xterm in a bare window, CI on all three OSes, `brand.rs`, `fake-agent`.
   - Added by the audit: README, CONTRIBUTING, SECURITY.md, issue and PR templates; `docs/adr/` with a GLOSSARY; macOS and Linux smoke jobs in CI; a date for the S1 Mac/Linux runs and a written cost estimate for the Electron fallback.
   - Spikes S1, S2, S6, S7 and S8 ran here. Open: S1 Mac/Linux rows. Exit: typing feels instant, resize works, the gate passes, all three OSes build in CI.
2. **Tabs + sidebar + shell integration.**
   - Scope:
     - shell detection and profiles, the launcher, profile settings pages
     - app-owned init scripts (OSC 133 prompt and command marks; OSC 7 from the POSIX shells). "Make permanent" moves to phase 3 with the config safety core; PSReadLine and prompt options come later
     - themes, accent, high-contrast and large-text modes, theme import (Windows Terminal, iTerm2, VS Code)
     - **terminal theme** (below): follow-app default, per-profile override, live switch, contrast protection, background-colour answers
     - split view, clickable paths, layout restore, F2 rename, reopen a closed session, drag reorder and project grouping, focus mode and rail
     - asciicast v2 recording, palette basics, the keyboard routing above, the cheat sheet, brand colors
     - the structural UX rules, which are hard to retrofit: focus restoration, the hover-freeze rule, the permanent lane header, rows that never resize
     - **Windows process hygiene (from S2):** Job Object with kill-on-close so a crash leaves no orphans; sideloaded `conpty.dll` + `OpenConsole.exe` with the MIT notice.
     - **Resilience:** a core panic hook and supervisor with a "core restarted" UI state; every file in `~/.marshell` carries a `version` field, is written atomically and keeps a `.bak`; migrations run on load.
     - **Restart behaviour:** on relaunch tabs come back as "Ended" with a one-click Resume (the CLI's own continue-last-here; see slice 2e below). Nothing relaunches itself or spends tokens unasked.
   - Slices: 2a hygiene and resilience; 2b tabs, sidebar, profiles and launcher (agents run inside a shell profile; status limited until phase 3); 2c shell integration; 2d themes; 2e split up to four panes, focus mode, the sidebar shell panel, project grouping, `layout.json` restore, and Ended tabs with Resume; 2e+ projects and task worktrees (below); 2f recording, palette and cheat sheet (built; decisions below).
   - Slice 2c (shell integration) ships first and is unchanged by the roadmap additions of 2026-10-16.
   - **Slice 2d (themes) is built.** User decisions of 2026-10-16:
     - Settings live in a real store, `~/.marshell/settings.json`, with `GET`/`PUT /v1/settings`. Only the Appearance fields are wired so far.
     - Theme import takes Windows Terminal, iTerm2 (`.itermcolors`) and VS Code theme files from a file picker. It also lists the schemes found in the user's Windows Terminal settings, read only and never written.
     - High contrast is Follow system (`prefers-contrast: more`), On or Off.
     - Text size is Default, Large or Larger, which move the UI type scale up one or two steps (13, 15, 17 px body). The terminal font size is its own setting (8–32).
     - What shipped: the Appearance dialog from the palette ("Appearance…"); presets plus a custom accent checked against both themes (3:1 on every surface, 4.5:1 for text); terminal theme Follow app, Always dark, Always light or imported, with a per-profile override in Shell profiles; contrast protection (`minimumContrastRatio`); `COLORFGBG` set at session start from the scheme the terminal will show; terminal font from your setting, else Windows Terminal's default profile font, else the bundled JetBrains Mono.
   - **Projects and task worktrees (slice 2e+, widened 2026-10-16; research in `docs/research/2026-10-16-accounts-cloud-extensions.md`).** A project is a folder, so projects are universal across CLIs and need no adapter. Projects are listed and grouped in the sidebar. A task can start in its own git worktree, as in the Codex desktop app, and Marshell then lets the user review its changes and merge it back in the app. This pulls the v2 "review-to-merge" item forward for worktree tasks; start-from-issue stays in v2. Worktree creation, merge and removal are git operations that show their command first and never run silently. **2e+** covers create, review the diff, merge back and remove; every git command is shown first.
   - A core restart ends the running sessions. A restart or a relaunch brings the tabs back as **Ended** with a Resume button. Resume runs the CLI's own continue-last-here in the tab's folder: `claude --continue`, `codex resume --last` and `agy --continue`, followed by the flags of the mode the session was last started in (the Resume bar shows them). A plain shell just restarts. Resume is refused, with the reason shown, if the profile was removed or the folder is missing. Exact-id resume is phase 3.
   - **Slice 2e decisions (2026-10-16).**
     1. The closed list is not saved. Reopen covers this run only, and only open tabs go to `layout.json`.
     2. Focus mode is Ctrl+Shift+I (Cmd+I on macOS). The shell panel is Ctrl+Shift+S, split is Ctrl+Shift+D and zoom pane is Ctrl+Shift+P. Grouping by project is an unbound palette command.
     3. Ended means "restored from the last run, nothing has run here yet": no terminal, only the Resume view. Exited keeps its terminal so the output can be read, with the same Resume bar under it. Resume replaces the old "Enter starts a new shell" text.
     4. Antigravity resumes with `agy --continue` (agy 1.4.0 `--help`, 2026-10-17). Claude uses `--continue` and Codex uses `resume --last`.
     5. If a restored tab's profile was deleted or its folder is gone, Resume is disabled and says why. It never falls back to another profile or folder.
     6. The shell panel is its own pty session, kept out of the registry, so typing there can never reach a tab. It starts in the focused session's folder, never types `cd` into itself, and uses the default shell profile. "Open in a pane" adopts that pty as a normal tab with its history, and the panel starts a fresh shell the next time it is shown. It folds itself when the sidebar is short.
     7. Panes remount when the layout changes. The core replays the buffered output, so scrollback is kept but the scroll position resets. Terminals never remount when the selection moves inside a layout.
     8. The arrow navigation cursor moves to 2f with the palette and cheat sheet. The Tab and Backspace chords ship in 2e.
     9. The focus-mode pill and next-waiting (N) stay hidden or unbound until phase 3 supplies real needs-you status.
     10. With grouping on, drag and Alt+Shift+arrow move a session only within its project group. The order inside a group is the core's order.
     11. The launcher remembers the last profile and agent per folder (the 32 most recent folders) in `layout.json`.
   - **Slice 2e+ decisions (2026-10-16).**
     1. Marshell runs git itself (`git worktree add -b …`) and shows every command first. This works for any CLI and for plain shells. No CLI's own `--worktree` flag is used.
     2. Worktrees live at `~/.marshell/worktrees/<repo-name>/<task-name>`, never inside the repo.
     3. Merge back commits any uncommitted changes in the worktree first. Then `git merge --squash <branch>` runs in the base checkout, followed by one commit on the base branch with a message the user can edit.
     4. A project is the git repository root. Subfolders and task worktrees group under their main repo. A folder outside git is its own project.
     5. Before merging, the base checkout must have no tracked changes. On a conflict the merge is undone so the base is left as it was, and the conflicting files are listed. The sheet offers to type a prompt asking the task's agent to fix them. The prompt is typed without Enter, so the user reads it and presses Enter.
     6. After a merge, "Remove the worktree and branch afterwards" is ticked by default. The removal commands are shown next to the merge commands.
     7. The task name is a slug of the first prompt, editable before launch. The branch is `marshell/<name>`. With no usable prompt (Hebrew, emoji) the name is `task-<n>`.
     8. "Review changes" is a sheet opened from a task tab's sidebar row, the palette, or Ctrl+Shift+R (Cmd+R on macOS). It holds the changed files, the diff against the base, Merge back and Remove.
     9. **Hidden git calls.** Only read-only calls run without being shown: `merge-base`, `diff`, `status`, `rev-list`, `rev-parse --verify` and `ls-files`. Every call sets `GIT_OPTIONAL_LOCKS=0`, so reads never take `index.lock` while an agent runs git. The `reset --merge` restores that follow a failed shown command are also not shown, because they only undo it.
     10. **Shown equals run.** Merge and remove send back the exact commands and warnings the user saw. If the core's fresh plan differs, it refuses with "The task changed while you were reading the commands. Review them again." and the sheet reloads the commands. Merge stays disabled while the commands are being refetched.
     11. When git no longer lists a task's worktree but its folder is still there, the plan shows `git -C "<repo>" worktree prune` as a step and warns that the folder is left on disk. Marshell never deletes a folder outside a shown git command.
     12. Remove closes the task's sessions first, including the shell-panel session and any session whose folder is inside the task folder, because a running shell holds the folder on Windows. It then re-plans, and refuses if the branch gained commits after the user saw the sheet. If git can't tell whether the branch has unmerged commits, the plan warns "Removing deletes the branch."
     13. Remove only accepts branches named `marshell/…`.
     14. If the merge succeeded but the removal after it failed, the result is "Merged" with removal not done. The merge is never reported as failed. Merging a task twice, or one with nothing to merge, fails with "<branch> has no changes to merge."
   - **Slice 2f decisions (2026-10-17).**
     1. Every session writes an asciicast v2 recording to `~/.marshell/history/recordings/<tab>-<unix ms>.cast`. It holds output, resizes and the OSC 133 prompt marks (as `m` events), never keystrokes, so a typed password is not written by the recorder. The palette's "Copy recording path" copies the focused session's file.
     2. Recordings are plain `.cast` files for now. zstd compression and retention (how many to keep, for how long) wait for phase 5.
     3. A recording stops at 256 MiB, or when the disk falls behind (the writer's queue fills), with a marker saying which. The session never waits for the disk. The file stays valid and replayable, and its path is still offered. Only a write error drops the path.
     4. The cheat sheet (Ctrl+Shift+O) reads the live keymap and lists only the keys the window actually runs, so it can never show a binding that does nothing.
     5. The navigation cursor ships to the full spec: hold Ctrl+Shift, Tab starts it, the arrows move it across panes, the session list and the top bar, and releasing selects or brings that session into the focused pane. Ctrl+Shift+Backspace selects nothing. Until Tab starts navigation, Ctrl+Shift+arrow still reaches the terminal (word selection in the shell). Blur or Esc cancels with nothing selected.
     6. On Windows the webview's own browser shortcuts (Ctrl+Shift+I devtools, F5 and Ctrl+R reload, Ctrl+F find, Ctrl+P print) are switched off in every build, debug included, so app shortcuts behave the same while developing. WebKit on macOS and Linux has none of these.
     15. A failed git call shows git's last `error:` or `fatal:` line after "Git said: ", else its last non-empty line. If the base can't be put back after a failed merge, the message adds the `git reset --merge` command to run.
     16. Large diffs are cut at 4 MiB on a file boundary. Binary and large untracked files show as a one-line note, not as text.
     17. The "Review changes" button sits inside the sidebar row. Ctrl+Shift+R and the palette are the keyboard and screen reader routes. A by-eye and NVDA check of the button is still open.
     - Routes (all under `/v1`):

       | Route | Does |
       |---|---|
       | `GET /tasks` | List the tasks. |
       | `POST /tasks/plan` | Show the commands that would create a task. |
       | `POST /tasks` | Create a task and its worktree. |
       | `GET /tasks/{id}/changes` | The changed files and the diff against the base. |
       | `POST /tasks/{id}/merge/plan` | Show the merge commands, with or without removal. |
       | `POST /tasks/{id}/merge` | Run the merge. Needs the commands the user saw. |
       | `POST /tasks/{id}/remove/plan` | Show the removal commands and warnings. |
       | `POST /tasks/{id}/remove` | Run the removal. Needs the commands and warnings the user saw. |
       | `POST /sessions/{tab}/input` | Type text into a session with no Enter. Used for the conflict prompt. |
       | `POST /sessions/{tab}/attachments` | Save a region's PNG under the session and return its path and the agent's reference for it. |
       | `POST /sessions/{tab}/paste` | Paste a saved attachment's reference and a one-line comment as bracketed pastes, never Enter. |
   - **Slice 2.5a decisions (2026-10-17).**
     1. The preview toggles with Ctrl+Shift+Q. On macOS, ⌘Q is Quit, so a binding may carry a `mac` override and the preview uses ⌘⌥Q there (macOS is untested). Ctrl+click is ⌘+click on macOS.
     2. A path opens with Ctrl+click, and only a path that exists becomes a link. Quick open lists the paths seen in the session's output first, for keyboard use.
     3. Quick open is a mixed "Files in <folder>" group in the palette after 2 or more characters, with "Seen in output" before it, plus an "Open file…" command that shows files only.
     4. The preview reads any file the user can read, through the token-checked API. Regular files only; the caps are 2 MB for text and 20 MB for images.
     5. File drop moved to slice 2.5c.
     6. A relative path resolves against the shell's current folder, then the launch folder, then the project root. A session in a worktree lists and resolves against its own worktree, not the main repo.
     7. A changed file reloads automatically and keeps its scroll position.
     8. Links in documents: a local link opens in the preview with Back, a web link asks before it opens the browser, and an anchor scrolls.
   - **Slice 2.5b decisions (2026-10-17).**
     1. Delivery is a bracketed paste into the agent's terminal input, never Enter. The image's quoted path goes as its own paste so Codex attaches it, then ` <file>, page <n>: <comment>` as a second one. A session with no prompt line gets `"<path>" <label>: <comment>` on the clipboard and a notice. This stands in for "fills the prompt line" until the phase 3 composer exists; without phase 3 status a paste can still land on an agent's approval prompt.
     2. A PDF scrolls continuously and renders pages lazily, with a "Page n of N" field, fit width, zoom steps from 50% to 300%, and selectable text. pdf.js runs without eval or WebAssembly, so the CSP is unchanged; files up to 50 MB.
     3. The comment is a small popover anchored to the box that flips to stay on screen: Add (Enter) and Cancel (Esc).
     4. Keyboard: "Select region" or R while the preview has focus. Arrows move the box 10 px, Ctrl+arrows 1 px, Shift+arrows resize; Enter opens the comment, Esc cancels. With the mouse, drag in select mode.
     5. After Add, focus goes to the session's terminal and the preview stays open.
     6. One region per Add. The comment is one line of at most 2000 characters with no control characters; the core refuses anything else.
     7. Crops go in `<home>/attachments/<tab>/<unix ms>.png` with owner-only permissions, at most 20 MB, rendered at up to 3× and at most 4096 px on the longest side. Every agent's `attachment_ref` is the quoted absolute path for now (docs/research/attachment-ref.md).
   - **Slice 2.5c decisions (2026-10-18).**
     1. Capture (Ctrl+Shift+X, ⌘⌥X on macOS) opens the Windows snipping tool, waits up to 60 s for an image on the clipboard, and delivers it like a region to the focused pane's session. Esc in the snipping tool cancels. The image is read in the Rust shell, since the page has no focus while the snipping overlay is up. Off Windows the command says it isn't available yet.
     2. An image-only paste with Ctrl+Shift+V is saved and its path pasted. Ctrl+V stays the CLI's own, so Codex keeps attaching clipboard images natively.
     3. A drop on a session's terminal pastes the quoted path or paths with no Enter, each followed by a space, and focuses that terminal. Up to 32 paths go in at a time. A name the session's shell would act on inside double quotes (`$` or a backtick, PowerShell's typographic quotes, a POSIX `!`) is pasted in the shell's own single quotes instead, so a dropped name can't run as code even after an agent has exited to its shell. A drop anywhere else opens the file in the preview; a folder can't be shown there, and the preview can't tell a folder from a missing file, so both read as a folder.
     4. Native drop is on, and the sidebar reorder is rewritten with pointer events, since native drop turns off HTML5 drag and drop on Windows. The click that ends a drag past 4 px is swallowed, as with HTML5 drag.
     5. A plain shell's terminal, the shell panel included, gets the dropped path pasted too, as Windows Terminal does.
     6. A pasted or captured image goes to any live session, agent or not, as its saved path; the §2.5 clipboard fallback is not used for it.
     7. The live check and its fixes are in docs/research/drop-paste-capture.md.
   - **Window frame (2026-10-17).** Windows runs frameless with the system shadow and rounded corners, and the 40 px bar is the title bar: its empty space drags the window, double-click maximizes or restores, and Maximize becomes Restore while maximized. Snap Layouts' hover flyout on Maximize is a follow-up, because the buttons are HTML and Windows does not see a maximize button; Win+Z and edge snapping work. macOS and Linux keep their native frames.
   - **Session row menu (2026-10-17).**
     1. Right-clicking a session row, or Shift+F10 or the Menu key on a focused row, opens its menu: Rename (F2), Split into a pane, Resume, Review changes (Ctrl+Shift+R), then Close (Ctrl+Shift+W) after a separator. On macOS the shortcuts show as Cmd+key.
     2. An item that doesn't apply is hidden, never disabled: Split only for a session not already in a pane on screen while a pane is free, Resume only on an exited or ended row, Review changes only on a task row.
     3. Opening the menu does not select the row, so Split can bring a session that isn't shown into a pane. Every item acts on the row the menu was opened from. Close on an unselected agent that needs the confirm bar selects it first.
     4. Arrows move with wrap-around, Home and End jump, Enter or Space runs, typing a letter jumps to the item that starts with it. Esc closes and returns focus to the row; Tab or a click outside closes without moving focus. The menu closes by itself if its row goes away. It stays fully on screen near the window's edges.
     5. Mode ▸ (restart the agent in another mode, after an inline confirm) is done in the modes-at-launch PR. Pause and Continue (freeze and thaw the whole process tree, a Paused mark, a first-time network warning, and a bar when typing into a paused session) are done in the pause PR. Later: Bookmark with phase 5.
   - **Sidebar pane marks (2026-10-17).**
     1. The session list marks every session that is visible: `aria-selected` and the selected fill on each row, and the list is `aria-multiselectable`. The focused pane's row also gets a 3px accent bar on its inline-end edge, and keeps the single tab stop. While a pane is zoomed only its session is visible; sessions in panes cut off by a small window, and empty panes, are not. The rail marks the same sessions.
     2. A plain click on a session that isn't visible fills the first empty visible pane and focuses it, or else replaces the focused pane's session. While zoomed it replaces the zoomed pane's session. A click on a visible session focuses its pane.
     3. Ctrl+click (Cmd+click on macOS, where Ctrl+click is a right-click), or Ctrl+Space on a focused row on every OS, toggles a pane: on a visible session it removes that pane, unless it is the only one; on any other session it shows it the way Split into a pane does. When the window fits no more panes nothing changes and the announcer says "No room for another pane."
     4. In forced colors a visible row has a 1px dashed outline, the focused row a 2px solid one and the keyboard focus a 3px double one, so they differ in shape and not only in colour. The rail's items follow the same rules, Ctrl+Space included.
   - Exit: 5 tabs across 3 shells; restart restores the layout; kill the app mid-task, relaunch, and every tab is back with Resume; no orphan processes after a crash; a recording replays in asciinema-player; OSC 133 prompt jumps work.
2.5. **Context and preview** (added 2026-10-15, inspired by the features of the Codex desktop app, not its infrastructure; numbered 2.5 so later phases keep their numbers).
   - Scope:
     - **Preview pane.** Read-only preview of code (syntax highlighted), Markdown (rendered, with a source toggle), images, PDF and HTML.
       - Opened from a clickable path in terminal output, by dropping a file on the window, or with quick open (fuzzy find in the focused session's folder). Its toggle is Ctrl+Shift+Q (⌘⌥Q on macOS, since ⌘Q is Quit). Dropping a file on the window is done in slice 2.5c. There is no file tree; that stays in v2.
       - Layout: the preview takes the space split view would use. Opening it shows the focused session and the preview side by side, and closing it brings the split back. This is the user's decision (2026-10-15): one session at a time while previewing.
     - **Region to prompt.** In a PDF or image preview, drag a box and type a comment. Marshell saves the crop as a PNG and puts the image reference, the file name and page, and the comment into the session's prompt line. Done in slice 2.5b, delivered by a paste with no Enter until phase 3 (see the slice 2.5b decisions).
     - **Screenshot and image drop.** Paste or drop an image on a session, or capture a screen region. Marshell saves it and puts its reference into the prompt line. Done in slice 2.5c (see the slice 2.5c decisions).
     - **Delivery rule.** Every feature here only fills the prompt line; the user edits it and presses Enter. Marshell still never types into a terminal on its own. A manual CLI has no prompt line, so the reference is copied to the clipboard and a toast says so. While a session waits on the user, the prompt line keeps its block, and the filled text waits until it can be sent.
     - **Multi-CLI.** How an image is referenced is an adapter method (`attachment_ref(path) -> String`). The default is the quoted absolute path. Claude Code and Codex each get theirs after their current docs are checked. A CLI that cannot read images gets the path and a note in the toast.
     - **Storage.** Crops and screenshots go in `~/.marshell/attachments/<session>/`, with owner-only permissions. They are kept with the session's history and follow its retention (phase 5).
     - **Security.** The core reads files for the preview only through the token-checked API. HTML previews render in a sandboxed frame without scripts or network access.
     - **New dependencies** (the concrete need is this phase): a PDF renderer (pdf.js), a syntax highlighter, and a Markdown renderer that sanitizes its output. Each is chosen and justified in the phase's plan.
   - Exit:
     - A path clicked in Claude Code output opens in the preview within 200 ms.
     - A PDF region reaches the prompt line, and the agent reads the crop, in both Claude Code and Codex.
     - A manual shell gets the clipboard fallback.
     - The whole flow works with the keyboard alone (open, select a region with the arrow keys, comment, fill).
2.6. **Project docs for people and agents** (added 2026-10-18 and done before slice 2.5c; numbered 2.6 so later phases keep their numbers).
   - Scope:
     - `AGENTS.md` holds the instructions every coding agent shares: the project map, commands, verification, working rules and platform. It is worded for no one CLI.
     - `CLAUDE.md` starts with `@AGENTS.md`, because Claude Code stops reading `AGENTS.md` by itself once a `CLAUDE.md` exists. It adds only Claude Code specifics: subagents and models, the slice workflow, where plans and ledgers live, the live-check recipe and compact instructions. The two together stay under 200 lines.
     - `SCOPE.md` says what an agent may change freely, what needs the user first and what it never touches. It also holds the secrets, destructive-command and dependency rules that the brief's `SECURITY.md` template lists (§5.6). Both instruction files point to it.
     - `SECURITY.md` is GitHub's security policy: how to report privately, supported versions, and the security model as built today.
     - `README.md` is the public front page: what Marshell is, its status, platforms, building from source, the docs and the licence.
     - From the phase-1 audit list, README and SECURITY.md are done here. CONTRIBUTING, the issue and PR templates and `docs/adr/` stay open.
   - Decisions (2026-10-18): `SECURITY.md` is for GitHub and `SCOPE.md` is for agents; `AGENTS.md` is the shared core that `CLAUDE.md` imports. The old "don't run tests unless asked" rule became "run the checks that cover your change before calling it done", which every agent now follows.
   - Exit: a fresh Claude Code session in the repo has `CLAUDE.md` and `AGENTS.md` loaded. GitHub shows the security policy, and private vulnerability reporting is on.
3. **Config safety core + bridge + Claude adapter.**
   - Scope:
     - The **config service core**, built here because hook install depends on it: ChangeSet, preview diff, jsonc CST edits, atomic write, snapshot and restore, `managed.json`, watching, managed-path guard.
     - Hook install flow, status, event recap, status line chain (S4, S10), pty heuristics fallback, the bridge's locked-exe rename update.
     - Context ring and plan ring (green/yellow/red at 70% and 90%), approve-from-anywhere (S9), needs-you lane plus the screen-reader live region, Peek.
     - Transcript archive copy, history index (SQLite `integrity_check` on start; rebuild from the archive if corrupt), Resume.
     - Pulled forward so the core loop can be used daily from phase 3: the onboarding consent flow, the taskbar/dock badge and flash, and the needs-you sound. Banners, batching and the full choreography stay in phase 4.
     - **Security:** an ADR threat model (loopback server, `/sessions/:id/input`, the `marshell://` link, instruction-library injection); owner-only permissions on all of `~/.marshell`; the deep link carries no authority (it only focuses); a security review gate before approve-from-anywhere ships.
     - **Diagnostics:** `tracing` logs with rotation, a size cap and redaction; "Export diagnostics" in Settings → Advanced; the hidden perf panel.
     - **Version drift:** adapters are version-gated; an unknown CLI version shows "Status limited" and a doctor note, with a per-CLI switch to turn normalisation off.
     - `marshell uninstall` and, per CLI, turning Marshell's hooks off in Settings (each removes the hook entries Marshell added and puts the user's own status line back from that CLI's `statusline-chain.json`; nothing else in the file is touched); an "orphaned hooks" check for when the bridge is gone.
     - Start code-signing procurement (Azure Artifact Signing, Apple Developer ID).
   - **Slices (decided 2026-10-18).** Each is its own plan and PR, in this order:
     - **3a Bridge and spikes.** The `marshell-hook` binary (roles, discovery, the exit-0 failure rule, `record`), `POST /v1/hook`, the permission hold with its decision and release routes, the copy to `~/.marshell/bin` with the locked-exe rename, and spikes S10, S4 and S9 against a real Claude Code.
     - **3b Config safety core.** ChangeSet, JSONC CST edits, preview diff, atomic write, snapshots and restore, `managed.json`, the managed-path guard, watching with the 3-way merge, owner-only permissions on `~/.marshell`, and the ADR threat model.
     - **3c Claude adapter, live.** The install plan through the consent card, events normalised into live status, the context and plan rings, the recap, the status line chain, version gating ("Status limited"), the pty fallback, `marshell uninstall`, "hooks off" and the orphaned-hooks check. Split on 2026-10-19 into two plans and PRs:
       - **3c1 Install and remove.** The Claude adapter's install and uninstall plans, the consent card wired to the config routes, the status line chain, `marshell uninstall`, hooks off, the orphaned-hooks check, and the bridge fixes carried from 3a.
       - **3c2 Live status.** Events normalised into live status, the context and plan rings, the event recap, version gating and the pty fallback.
     - **3d Approve from anywhere.** Live approve cards with the risk check, the needs-you lane and live region, Peek, the taskbar badge and flash, the needs-you sound, and the security review gate.
     - **3e History and diagnostics.** The transcript archive, the SQLite index with its check and rebuild on start, Resume, `tracing` logs with rotation and redaction, "Export diagnostics", the perf panel, and the code-signing checklist.
       - **3e1 History.** The transcript archive, the SQLite sessions index with its check and rebuild on start, and Resume by session id.
       - **3e2 Diagnostics.** `tracing` logs with rotation and redaction, "Export diagnostics", the perf panel, and the code-signing checklist.
     - **3f Folder picker** (added 2026-10-18, may run any time after 3a). The launcher's folder field accepts a typed path and opens the OS folder picker.
     - **3g Exit gaps** (added 2026-10-21, before the phase 3 exit check). The subagent count and the live permission mode, the two exit criteria nothing fed yet.
   - **Slice 3a decisions (2026-10-18).**
     1. The spikes reach a real Claude Code through a scratch folder's `.claude/settings.local.json`, so `~/.claude` and its login stay untouched. Trusting that folder is part of the check, in a session started for it.
     2. S4 wraps the user's real `python ~/.claude/statusline.py`, which is only run, never written.
     3. Hook events show nowhere in the UI in 3a. The core keeps the last 200 raw events per tab in memory behind a token-checked GET, and the bridge logs its failures to `logs/bridge.log`. 3c turns them into status.
     4. A held permission request is answered through the real `POST /v1/permissions/:rid/decision` and `/release` routes, driven by the spike. The card comes in 3d.
     5. Recorded fixtures come from made-up prompts in a scratch folder. Before they are committed, a scrub step replaces the home path, the user name, session ids and transcript paths with placeholders, and a test checks that no fixture still holds them.
     6. Code signing: a checklist (`docs/research/code-signing.md`, eligibility, cost and the steps to apply) is written in 3e. Buying and identity checks are the user's; wiring signing into CI waits for the release phase.
   - **Carried from 3a's live check into 3c** (details in `docs/spikes/S10-hook-path-spaces.md` and `S9-permission-hold.md`):
     1. The status line posts every second and floods the 200-event ring. Keep only the latest status line per tab, or give it its own ring.
     2. A session inherits `CLAUDE_CODE_*` variables from whatever started Marshell, and `CLAUDE_CODE_CHILD_SESSION` turns Claude's transcript saving off. Drop them from the session environment.
     3. Some requests offer only an `addDirectories` suggestion, so a scoped allow is refused. Decide whether a scope may use it, or the card hides the scopes then (3c/3d).
   - **Carried from 3a's final review into 3c** (before 3c installs the hooks and the status line chain):
     1. The bridge runs the user's status line command with no time limit, and on Windows its shell outlives the bridge when Claude kills it. Run the command in a kill-on-close Job Object (as `crates/core/src/job.rs` does) with a timeout.
     2. Nothing clears a tab's state when its session ends: its event ring stays, and its held requests wait out the hold. Release the tab's requests and drop its ring when the session closes or exits.
     3. A permission request with no tab is held for the full 120 s, and nothing in the pane can release it. Decide whether it is answered at once with 204 (3c/3d).
   - **Slice 3b decisions (2026-10-18).**
     1. New dependencies (approved): `jsonc-parser` with its `cst` feature for comment-preserving JSON edits, `notify` for watching, `diffy` for the preview diff and the 3-way merge, and `sha2` as a direct dependency for the SHA-256 file hashes.
     2. Edit kinds: 3b builds JsonSet, JsonRemove, CreateFile and Restore. TomlSet and TomlRemove come with the Codex adapter, and MarkdownBlock with the instruction files in phase 7.
     3. Permissions: on Windows, `~/.marshell` gets a protected ACL that grants only the current user and SYSTEM, inherited by everything inside it, and it is checked and fixed on every launch. On Unix, folders are 0700 and files 0600. Changing them was approved as part of this slice.
     4. Snapshots: the last 20 per file are kept, and the oldest is deleted when a 21st is taken.
     5. Write guard: the config service writes only under the user's home folder or a project folder opened in Marshell, and it always refuses the known managed and organization paths (Claude's managed settings, Codex's `/etc` and `managed_config`, Program Files, ProgramData), even inside an allowed folder.
     6. Watching: the files Marshell manages are watched, its own writes are ignored by hash, and a change made outside Marshell marks an open preview stale. The 3-way merge is a tested core function with no UI; the phase 6 editors use it.
     7. No UI in 3b: the core, the PLAN §3 config and snapshot routes, and their tests. The first screen that shows a diff is 3c's consent card. The live check drives the API against scratch files. `GET /config/sources` needs the adapter's list of config files and moves to 3c; `POST /snapshots/:id/restore` returns a preview that is applied through `/config/apply`, so a restore is shown before it is written.
   - **Slice 3c decisions (2026-10-19).**
     1. 3c ships as two plans and PRs, 3c1 (install and remove) and then 3c2 (live status), each with its own live check and review.
     2. The product installs into `~/.claude/settings.json`. Tests and live checks set `MARSHELL_CLAUDE_SETTINGS` to a scratch project's `.claude/settings.local.json` instead, so the real `~/.claude` is never written and Claude keeps its own login.
     3. The consent card offers status and the status line only. The approve-from-anywhere toggle, and so the permission hook, comes in 3d with the card that answers requests. The carried decisions on `addDirectories`-only requests and on a held request with no tab move to 3d with it.
     4. Version gate: a Claude Code version on the same minor line as a recorded fixture set (2.1.x), and not older than the oldest one, is known. Anything else shows "Status limited" with a doctor note until fixtures for it are recorded. Normalisation also has a per-CLI off switch (3c2).
     5. Entry points: on first launch, onboarding shows its Detected and Consent screens (the notification and launcher screens come later). A "Status limited · set up" row link opens the same card. A minimal Settings → Agents section shows the install state, with Set up, Hooks off (preview, then apply) and the orphaned-hooks warning.
     6. `marshell uninstall` is a standalone `marshell` binary that links the core, so it works with the app closed or broken. It prints the unified diff of every file it would change and asks y/N (`--yes` skips the prompt). It applies through the config service with snapshots, restores the status line byte for byte, and then removes `~/.marshell/bin`. The rest of `~/.marshell` stays.
     7. The CLI parses its arguments by hand (`uninstall [--yes]`, `--help`, `--version`); no new dependency. Phase 6 revisits this when it adds `list`, `send` and `new`.
     8. The status line flood (carried item 1) is fixed in 3c1: the core keeps only the latest status line per tab, outside the 200-event ring.
   - **Slice 3c2 decisions (2026-10-19).**
     1. The core normalises hook events into a per-tab status record (state, phrase, since, recap) and pushes changes over the existing socket. The UI only renders it. 3d's lane, Peek and taskbar badge read the same record.
     2. The context ring reads the latest status line's context window used percentage, and the plan ring its 5-hour rate limit (the sidebar footer ring). Green below 70%, yellow from 70%, red from 90%. A field the status line lacks hides its ring; no dash and no zero. There is no transcript-derived fallback.
     3. The version gate is decision 4 of slice 3c. A gated, unreadable or switched-off session shows "Status limited" with a doctor note naming the version, and takes the pty fallback.
     4. The pty fallback gives Working (output activity), Idle (quiet) and Ended (process exit) only. It never guesses Needs you, Done or Error. A Claude row without hooks still reads "Status limited · set up".
     5. The rings use colour and shape together: green below 70%, yellow from 70% and thicker, red from 90% with a centre dot, so the level still reads in forced colours and without colour (2026-10-19, after the review).
     6. A Done row in the comfortable density reads "Done · <the recap's first line>", with the age in the right cluster. The expanded row keeps the recap with its age (2026-10-19, after the live check).
     7. A fallback row reads "Working · Status limited" or "Idle · Status limited" on line 2, and its accessible name adds the note ("Status limited: Live status is off for Claude Code."). The tooltip keeps the full note (2026-10-19, after the review).
     8. A sidebar row's age reads "<1m" under a minute, then "1m", "2m"…, on screen and in the row's accessible name, so a focused row is not re-read every second and the name still contains what the row shows (2026-10-19, after the review).
   - **Slice 3d decisions (2026-10-19).**
     1. Sessions keep starting with each CLI's own mode flag. A live card is shown for every request the CLI itself asks about, with the risk check (hold-to-allow, no Always, expand before Allow arms). The hybrid start, where the hook answers by mode and the guard rail also covers Bypass, becomes its own later slice.
     2. A request whose suggestions hold no rule (only `addDirectories` or `setMode`) gets no Always button. The card offers Deny, Allow once and Answer in terminal.
     3. A permission request with no tab (a Claude run outside Marshell) is answered with 204 at once, so the CLI's own prompt handles it and an outside session is never slowed.
     4. The needs-you sound uses rodio (approved as a new dependency) with one bundled sound. It plays when a session starts needing you while Marshell isn't the focused window, and a Settings switch mutes it. (Superseded by 9.)
     5. The consent card's "Approve from anywhere" toggle, which installs the PermissionRequest hook, is on by default. An install without it shows "Approve from anywhere: off · Set up" in Settings → Agents, which opens the same card with its diff. Nothing is added silently.
     6. The security review gate is a dedicated Opus security review of the branch against ADR 0001, separate from the whole-branch review. Its findings are fixed with a test that fails first, and ADR 0001 gains a "Reviewed for 3d" section.
     7. Peek shows the session's last 30 rows from the core's screen parser as plain monospace text, without colours, through a new token-guarded route.
     8. The taskbar overlay shows the needs-you count whenever it is above 0. The window flashes and the sound plays only when a session starts needing you while Marshell isn't focused. Muted sessions don't flash or sound.
     9. The needs-you sound is Windows' own notification sound (`PlaySound` with the `Notification.Default` alias and no fallback beep), not a tone of Marshell's own: the synthesised chime sounded harsh in the live check. It follows the user's sound scheme and volume, stays on by default, and the Settings switch still mutes it. rodio is dropped from 3d; macOS and Linux get their system sound when those platforms are built (2026-10-19, after the live check).
   - **Slice 3e decisions (2026-10-20).**
     1. 3e is split into 3e1 History and 3e2 Diagnostics, each with its own plan and PR.
     2. The archive copies Claude Code transcripts of sessions run in a Marshell tab, found through the `transcript_path` and `session_id` its hooks send. The zstd copy is refreshed on every `Stop` and at `SessionEnd`, so a crash loses at most the turn in progress, and the copy outlives Claude's own cleanup after `cleanupPeriodDays`. Codex and other agents follow when their adapters land.
     3. In 3e the index holds sessions and their chains only (agent, session id, tab, folder, times, archive path, the tab's previous session). Parsing turns, tool calls, files touched and the FTS5 search waits for phase 5, which has the UI that uses them.
     4. Resume and Restart run `claude --resume <session id>` when the tab's session id is known, so two sessions in one folder each resume their own conversation. Without one they keep `--continue`.
     5. Each archive copy has a `<session>.meta.json` sidecar beside it, and the index is rebuilt from the sidecars, never by parsing the transcript, whose format is internal to Claude Code.
   - **Slice 3e2 decisions (2026-10-20).**
     1. Logs: `tracing-subscriber` (approved as a new dependency) writes `<home>/logs/marshell.log` through Marshell's own size-rotating writer, 2 MiB per file and 5 files kept (10 MiB at most). Info level by default; a "Detailed logs" switch in Settings → Advanced turns on debug until restart. Every line passes the redaction filter before it is written.
     2. "Export diagnostics" holds logs and setup only: Marshell's logs, `crash.log`, `bridge.log`, versions (app, OS, each CLI), settings and profiles with secrets removed, the hook install state per CLI, and one perf snapshot. No transcripts, recordings, prompts or hook event bodies. Everything is redacted on the way out (API keys, tokens, the home path and the user name).
     3. The export is one `.zip`, previewed first: the dialog lists every file going in with its size, each redacted file can be opened before saving, and a Save dialog writes `marshell-diagnostics-<date>.zip`. The `zip` crate is approved with deflate only.
     4. The perf panel gains resource numbers: a token-checked core route gives startup time, the core's memory and CPU, and each session's process-tree memory and CPU, sampled once a second while the panel is open. The overlay shows them beside the keypress→paint p50/p95, and the export takes one snapshot. Budget numbers are set later from what it shows.
     5. "Detailed logs" and "Export diagnostics" live in a "Diagnostics…" dialog opened from the palette (Tools group, beside Agents… and Appearance…), because the Settings page is not live yet. When it is, the same component becomes its Advanced section.
   - **Slice 3f decisions (2026-10-18).**
     1. One click on the folder chip opens the field as now. Typing filters known folders and also accepts a full path in the running OS's syntax, offered as a "Use <path>" row. A "Browse…" row at the end of the list and Ctrl+O in the field open the native picker, so mouse and keyboard reach it alike (double-click was considered and dropped: the first click already swaps the chip for a field, and it has no keyboard route).
     2. The native dialog comes from `tauri-plugin-dialog` (approved as a new dependency), with a capability limited to the folder-open dialog. Linux and macOS get it from the same plugin later.
     3. Built as its own slice and PR after 3a merges, with a live check of the dialog. How a typed path that doesn't exist is handled is decided when 3f is planned.
     4. A typed path is checked by the core as you type. An existing folder gets "Use <path>"; a missing one gets a disabled "No folder at <path>" row, so Enter can't launch into it (decided 2026-10-19).
     5. A full path is an absolute path in the running OS's syntax (`C:\x`, `\\server\share`, `/x` on Unix) or one starting with `~` for the home folder. Quotes pasted around it are stripped, and anything else only filters the known folders (2026-10-19).
     6. The dialog opens through Marshell's own `pick_folder` Tauri command, which calls the plugin from Rust. The webview gets no dialog permission and no npm package is added (2026-10-19).
     7. A folder chosen by typing or Browse is remembered after launch like any other, except a drive root (`C:\`, `/`), which is never recorded, as the home folder isn't. So the config write guard never allows a whole drive (2026-10-19).
   - **Slice 3g decisions (2026-10-21).**
     1. The subagent count comes from hooks. `SubagentStart` joins the installed status events; the core keeps the running `agent_id`s per tab, which Start adds, `SubagentStop` removes, and the main turn's `Stop`, `SessionEnd` or a new process clear, so a missed Stop can't leave a stale count. An install from before reads Partial, and Set up again adds the one entry. Amended by the live check (`docs/research/2026-10-21-exit-gaps-live-check.md`): Claude Code 2.1.305 runs subagents in the background past the main `Stop`, so that `Stop` keeps the subagents its `background_tasks` lists as running and clears only when the list is missing; a subagent's own events don't move the mode; a comfortable row shows the count after its phrase.
     2. The mode chip follows the `permission_mode` Claude Code reports on its hook events: `default`→Manual, `plan`→Plan, `acceptEdits`→Auto-edit, `auto`→Full auto, `bypassPermissions`→Bypass (with its red tint), `dontAsk`→a new "Don't ask" mode, which is reported and never offered in the launcher or Mode ▸. Until a hook reports, the launch mode shows. Resume and Restart start in the mode last seen.
     3. 3g runs as its own plan and PR with a live check, then one integrated phase 3 exit check runs on main, its findings in `docs/research/`.
   - **3g follow-up decisions (2026-10-21).**
     1. While the main turn is over but subagents still run (a `Stop` whose `background_tasks` lists a running subagent), the row stays Working with the phrase "Waiting for subagents", and its count follows. There is no Done sound or notice yet.
     2. The row turns Done at Claude Code's own final `Stop`, which lists no running subagent. If the last subagent has ended and Claude Code hasn't resumed the turn 30 s later, the row turns Done anyway.
     3. An agent that ended inside a shell that still runs (#39) offers no Resume: no bar, palette command or menu item. The person types in the shell as usual, and Resume comes back once the shell exits.
     4. The Resume bar may name `--continue` while the core runs `--resume <id>` (#41): not a problem.
   - **Phase 3 exit check decisions (2026-10-21).** From the integrated check on main (`docs/research/2026-10-21-phase-3-exit-check.md`), where every criterion passed except the context ring:
     1. The context ring matches Claude Code's `/context`. It is the status line's `context_window.total_input_tokens` divided by the effective window: the smaller of `context_window_size` and the `autoCompactWindow` in Claude's settings (user, project and local, read only). Without that setting it stays `used_percentage`. This amends slice 3c2 decision 2.
     2. Mode ▸ follows the Resume rule of the 3g follow-up decision 3: while an agent has ended inside a shell that still runs, the row menu, the chip and the palette offer no Mode ▸. It comes back once the shell exits.
     3. `marshell uninstall` deletes its own exe once it has exited, through a short hidden helper that waits for the process and then removes the exe and the empty `bin` folder, so nothing is left to delete by hand.
   - Exit: a real Claude session shows correct model, effort, mode, subagents and context; approval from a card works; uninstall leaves zero byte diff; corrupting `index.sqlite` is repaired on start.
4. **Notifications + choreography + Departures board + stuck detection** (S3).
   - **Phase 4 decisions (2026-10-21).**
     1. Phase 4 is four slices, each with its own plan and PR, in this order: 4a notifications (spike S3, the notification engine in the core, Windows banners, sounds, Settings → Notifications); 4b the tray, the close button and the needs-you count in the window title; 4c the Departures board and the attention choreography; 4d stuck detection and the BEL, OSC 9 and OSC 777 fallback.
     2. Each category plays a Windows system sound of its own through `PlaySound` with a sound alias, as slice 3d decision 9 did for needs-you, so the sounds follow the user's sound scheme and volume. Each category has its own on/off switch; there is no per-category volume. This amends brief §5.5 (rodio, per-category volume) and the sound brief above; macOS and Linux get their system sounds when those platforms are built.
     3. With a tray icon, the close button hides the window to the tray: sessions keep running and banners and the tray still work. Quit is in the tray menu. A one-time note says where the window went, and a setting makes the close button quit instead.
     4. The exit is verified on Windows 11. The macOS and Linux parts sit behind `cfg` stubs (a no-op or a best effort), are marked unverified, and are checked when there is hardware to test on.
   - **Slice 4a decisions (2026-10-21).**
     1. 4a's banners offer Open, or Review for a risky request, and nothing else. Allow and Deny on a banner (batch 3 design decisions) come in their own small slice after 4a, gated by an Opus security review against ADR 0001 as slice 3d was. A `marshell://` link only ever focuses a session and never answers a request.
     2. A main `Stop` is held for 1.5 s before it counts as Done: the row stays Working, and a `UserPromptSubmit` or `PreToolUse` in that time cancels it (phase 3 exit check finding 4). Only a settled Done draws the check and reaches the sound and banner rules.
     3. Quiet hours are off by default and prefilled 22:00–08:00 when turned on. They silence Marshell's sounds and the taskbar flash only: banners still arrive (silent, as always) and the badge count stays. Windows do-not-disturb silences Marshell's sounds the same way, as far as `SHQueryUserNotificationState` reports it (checked in spike S3; a known limit if it doesn't).
   - Exit: every rule in brief §5.5 is verified on Windows 11 (phase 4 decision 4). Every sound has a visual equivalent.
5. **History view and conversations.**
   - Conversations (added 2026-10-16; evidence in `docs/research/2026-10-16-accounts-cloud-extensions.md`): the history view also lists and manages every CLI's locally saved conversations (Claude Code, Codex CLI, others), not only sessions Marshell ran.
     - **Cloud-only conversations.** Where a CLI can list them, they show with a cloud icon (`codex cloud list --json` exists). Claude web sessions are reachable only through `--teleport` or claude.ai/code. There is no public API for claude.ai or ChatGPT history, so Marshell does not try to read one.
     - **Parsing.** Transcript parsing is best effort and version-gated, because each CLI's file format is internal. An unknown version shows "Status limited", as the adapters do.
     - **Resume.** It goes through each CLI's own resume flow. Marshell never rewrites a CLI's conversation files.
   - Scope: search, conversation view, files-changed tab, recording replay, bookmarks, tags, archive and delete-copy, export with redaction, import of sessions run outside the app, retention limits (size and age), the optional backup-folder mirror, a disk-usage view, recording compression and cleanup.
   - Exit: a 1000-session index searches in under 100 ms; recordings stay within a stated MB-per-hour budget.
6. **Agents, integration and the elevated helper.**
   - Accounts, company tabs, extensions and CLI versions (added 2026-10-16; evidence in `docs/research/2026-10-16-accounts-cloud-extensions.md`):
     - **Accounts** (Settings → Accounts, one section per company). Sign in runs the CLI's own login command in a terminal (`claude auth login`, `codex login`). Marshell never collects, stores or brokers OAuth tokens and never reads credential files. Status comes from the CLI's status command: `claude auth status` (JSON: `loggedIn`, `authMethod`, `email`, `subscriptionType`) and `codex login status` (text). The row reads "Signed in as <email>, <plan>, checked <time> ago", with a Sign in button when signed out. There is no expiry countdown, because no CLI exposes one. A value the command does not give is left out (nothing guessed).
     - **Antigravity is terminal only.** It has no login or status command, and Google's consumer terms ban wrapping it in other agents, so it has no account status, doctor or automation. Its Accounts section says why.
     - **Company tabs.** A company's settings tab appears once its CLI is signed in by any method, an API key included. It edits that CLI's own settings (Claude Code `settings.json`, Codex `config.toml`), not Claude Desktop's. Every write goes through the config service (preview, backup, atomic write, undo; section 6).
     - **Extensions** (a universal tab). It covers MCP servers, skills, hooks and plugins, but only where they port. Skills (`SKILL.md`) are the portable unit. MCP stdio entries can be translated between the Claude JSON, Codex TOML and `agy` JSON formats, with a preview. Hooks, plugins and slash commands stay per CLI.
     - **CLI version and update** (in settings). Each CLI's `--version` and install type, and an update button (`claude update`, `codex update`) with the command's output shown. This pulls part of the v2 installer runner forward; installing a CLI stays in v2.
   - Scope: settings pane (read-only until the company tabs above add editing), the terminal font setting (added 2026-10-18; three bundled fonts and a "Match my terminal" default, specified under "Visual system"; the option list is proposed, to confirm when the slice is planned; licence notices go in with phase 9), Codex + Antigravity adapters (install both CLIs first; the Antigravity adapter verifies its hook events and any permission hook against a real install), custom agents, repo scripts (`marshell.json`), port chips, the `marshell` CLI and its built-in skill (when the app is closed, `marshell new` says so and offers to start it), project references (`@` picker, alias learning), WSL (S11), "open in system terminal" and "copy resume command", and the `marshell-host` elevated helper. S8 passed with requirements: the token goes over a pipe, never the command line, and the decline and wrong-token paths get tested. Until it ships, elevated sessions open in a separate elevated Marshell window.
   - Exit: fixture suites from the real CLIs pass.
7. **Doctor**: the fix actions, on top of the phase-3 config core: "Fix X issues" and one-click undo; folds in each CLI's own doctor; detects duplicate notifications; reports the tokens injected by project references; checks Codex hook trust; waste checks (unused MCP servers and skills, long descriptions, broad scopes, a token-cost estimate); offers to raise `cleanupPeriodDays`.
   - Added 2026-10-16 (research in `docs/research/2026-10-16-accounts-cloud-extensions.md`):
     - **One-click doctor per CLI.** It runs the CLI's own doctor (`claude doctor` text, `codex doctor --json`) and opens a summary window of suggested fixes. Every fix goes through preview, backup and undo; nothing is applied silently. Antigravity has no doctor command, so it gets none.
     - **Instruction-file check, beside the doctor.** For CLAUDE.md, AGENTS.md, GEMINI.md and the like, it checks per CLI for size, stale paths, contradictions and that CLI's conventions (for example Codex's 32 KiB limit), and shows suggested edits with a preview. Nothing is written until the user accepts; backup and undo apply. It is a check on files that exist; the phase-8 instruction library is where files are authored and wired.
   - Exit: a seeded broken config is fully repaired and fully undone.
8. **Editors + instruction library** (wired per CLI with a content hash and drift check; settings raw mode validated against the SchemaStore schema), smart and native recaps, diff drawer, step timeline, saved prompts, broadcast.
   - Diff drawer comments (added 2026-10-15, after the Codex app's review pane): a **+** on a diff line adds a comment. "Send comments" puts them all in the prompt line as one message, with `file:line` before each. Like phase 2.5, it fills the prompt line and never sends on its own.
   - Instruction library items: frontmatter form, "what loads here" view, token estimate with a 4k warning, templates and one-click project profiles, per-project toggles, agent filter, globs, and "Improve this file" (headless, opt-in).
   - Exit: one doc wired into all three CLIs, verified by each CLI loading it; the 32 KiB Codex limit warns before it truncates.
9. **Release.**
   - Scope: tagged-build workflow, signed and notarized artifacts (NSIS per-user + MSI, dmg, AppImage/deb/rpm), checksums, release notes, the Tauri updater with a signing key and a rollback path, installers that run the unhook on uninstall, a manual release checklist file (including the macOS run), the WebView2 bootstrapper and the arm64 build (moved from phase 2 on 2026-10-15: installer concerns), `cargo-deny` and `pnpm audit`, licence notices for bundled fonts, sounds and OpenConsole, and the formal trademark search.
   - Exit: a signed build installs, updates and uninstalls on all three OSes with zero trace left in any CLI config.

### Accessibility safety net

Two bugs (a prompt line announced twice, a selected rail item that vanished in Windows High Contrast) were found by a late review, not by a test. Four layers now catch that kind of problem early.

1. **Shared building blocks** (`src/components/a11y/`). `Announcer` is the only live region in the app: silent on first render, never the same words twice running, at most one message per 1.5 s with the latest winning, and an assertive message is not held back. `SelectedMark` (`.is-selected-mark`, `selectedProps()`) gives every selected state three independent cues: a fill, a 2 px bar on the end edge and, in forced colours, a Highlight outline. Features use these and do not draw their own.
2. **Automated rules** (`sharedBlocks.test.ts`, `rules.test.tsx`). Fail the build when a file other than the Announcer adds a live region (the few exceptions are listed with reasons, and a debt list may only shrink); when a selected-state selector has no forced-colors rule in its file; when a status lacks a drawn shape or words, or shares them with another; when a toggle changes its name with its state; when a window renders more than one live region; or when a forced-colors rule uses a non-system colour or a border colour.
3. **Matrix** (`pnpm a11y:matrix`, `docs/a11y/a11y-matrix.md`). Windows High Contrast, reduced motion, 150% zoom and four colour-vision filters, with screenshots and in-page checks for selected markers, focus rings, clipped text, looping motion and live regions.
4. **Screen-reader script** (`docs/a11y/screen-reader-script.md`). Ten minutes with Narrator and NVDA at every sub-review sign-off, with the exact phrases expected. A fail is fixed before sign-off.

**What is announced (the setting `announce`).** The default, `needs-you`, announces a request or question arriving (for the focused session by name, and "N sessions need you" when the count rises anywhere), a stop or an end, the session you switch to ("Session 6, billing", plus what it needs), and "No session selected". It never announces progress, "working", a finished turn or anything in a pane you are not in. "Read the focused session's status" (Ctrl+Shift+G, ⌘G) says the focused session's state on request, every time. `everything` (every status change in every visible pane) and `on-request` (nothing until asked) are typed in the setting but not built: until they are, `everything` behaves like `needs-you`.

### Batch 3 design decisions

Onboarding, Departures, the doctor, the settings page, the banners and the tray (phase-0 rows 7 to 11), as they were designed and reviewed.

- **Unknown values.** Rows and the status strip leave an unknown value out (nothing guessed). A table or a form, where a missing cell would break the shape, shows an en dash: Departures, the doctor, the settings page, the consent card ("line –").
- **Pages scroll in their body.** A page (Doctor, Settings) scrolls in `.page__body`; the fix sheet scrolls in `.fix-sheet__body`; the onboarding frame scrolls in `.onboarding__body`. Each has a visible themed scrollbar (like the drawer, since these are not the live window) and its actions are a last row outside the scrolling part, so Cancel, the fix and "Allow these changes" are reachable however long the change is, even at 720 by 480. `dialogBodies.test.ts` guards the exception so it cannot spread.
- **Announcing.** An action the user caused (a fix, an apply) is announced in every announce mode, as the `action-result` kind. The pages add no live region of their own.
- **Config safety copy.** A card or a strip says exactly what changes and where. Undo copy only names things that exist and says what removal does for that CLI, built from its switches (`removalCopy` in `consent.ts`): Claude, "Marshell removes the hook entries it added and puts your own status line back from statusline-chain.json; the rest of the file stays as it is"; Codex, "Marshell removes the hook entries it added; the rest of the file stays as it is"; Antigravity, "Marshell removes the hook entries it added, deletes hooks.json if nothing else is in it, and puts your own status line back from statusline-chain.json; the rest of each file stays as it is". It never says "removes only the entries it added" where a status line command was replaced rather than added. A restore from a snapshot (the fix sheet, the settings change strip) is only promised for the snapshot taken by that same apply, for as long as the file is unchanged since. A chained status line shows where the user's own command is kept (`~/.marshell/agents/claude/statusline-chain.json`) as an added file in the diff. A file that is read-only, does not parse, or is managed is never described as about to change. A conflict offers "Review the difference" and "Reload", never an overwrite.
- **Consent and the diff.** One card per CLI, each with its own diff; no bulk "allow all". The count in the headline is the number of entries in the diff (a test), the bridge path is quoted inside every command (it has a space), and a stand-in made long on purpose says so in its own headline.
- **Fix sheet.** Focus starts on the title (never an action); Tab stays inside, Escape closes and focus returns to the opener (`useDialogFocus`, the pure `trapKey`).
- **Settings.** The change strip is small and sticky; Review opens the exact diff in the page above it, so a long change never covers the form. A managed field shows the value the organisation enforces and says the user's own is not used.
- **Banners.** Allow and Deny appear only when the whole command fits on one line (160 characters) and is shown; an escalation, a long or multi-line command and a risky request offer only Open or Review. Linux shows just Open.
- **Departures.** Rows are held while the pointer is over the board; a row that now needs you says it moves up. Y opens the request with focus on the card, N with focus on Deny (neither answers); the letter shown comes from the user's own layout when the browser can tell. The board has explicit table roles (the grid layout would drop them in some browsers), one row group per group, a roving tab stop, and counts its "+N more" line and an open request in the height it fits.

## Cross-cutting requirements (every phase)

- **Privacy:** no telemetry and no crash reporting, ever. The app makes no network calls except ones you start (updates, a model-backed recap you turned on). Crash information stays local in the diagnostics export. The onboarding says so in one line.
- **Zero token cost by default:** no model call happens unless you enable a model-backed feature; a test asserts it.
- **Secrets at rest:** transcripts and recordings are stored with owner-only permissions (0700 / owner ACL), not encrypted, so search stays fast. Redaction happens on export.
- **Languages:** English only in v1. All user-facing copy lives in one string catalog so translation can be added later. Layout already handles right-to-left names.
- **Accessibility:** besides Storybook axe, each UI phase ends with a keyboard-only walkthrough; the terminal ships with xterm's screen-reader mode available; one screen-reader pass before release.
- **Budgets beyond latency:** memory per session, idle CPU and startup with 1000 sessions are measured in the perf panel and given numbers when phase 2 lands.
- **OS integration:** autostart at login, `marshell://` registration and "Open in Marshell" are settings, all off by default.

## v2 and later (not in v1)

From brief §5.9 and §6: the installer runner (install CLIs; the update button and version display moved to phase 6 on 2026-10-16), review-to-merge (for worktree tasks it moved to slice 2e+ on 2026-10-16), start-from-issue, checkpoints, file tree, phone check-in, and the `marshelld` daemon. v1 keeps the core protocol-first so these are additions, not rewrites. Also deferred: Airport mode (relabel statuses), if wanted at all.

## 10. Testing strategy

- **Rust unit tests**
  - Adapters and normalizers run against **recorded fixtures**. `marshell-hook record` writes the real stdin to `crates/core/tests/fixtures/<agent>/<cli-version>/<event>.json`, with insta snapshots of the `SessionEvent` output.
  - Each new CLI version gets a new fixture directory, which becomes a compatibility matrix.
- **Config service**
  - Property tests (`proptest`): bytes outside the edited region never change; apply followed by undo restores the identical bytes.
  - Conflict tests.
- **PTY**: tests drive `fake-agent`, which emits output, BEL, OSC 7/133/9/777 and calls the bridge, so status flows are deterministic in CI without real CLIs.
- **Frontend**: Vitest + Testing Library for components; Storybook test-runner with axe for a11y on every story.
- **E2E per OS**
  - (a) Playwright drives the real frontend against a headless core binary with `fake-agent`. This works on all three OSes because of the protocol-first design.
  - (b) `tauri-driver` WebDriver smoke test of the packaged app on Windows and Linux. macOS has no WKWebView driver, so it relies on (a) plus a manual checklist per release.
- **Performance**: criterion bench for the pty to WebSocket path in CI; a hidden perf panel measures keypress→paint latency.

## 11. Risks and open questions

- **WebKitGTK** is the biggest platform risk; the S1 gate is the decision point and Electron is the fallback.
- **CLI churn.** Claude's transcript format is officially unstable, and hooks and status fields change monthly. Mitigations: versioned fixtures, defensive parsing, and treating the raw archive copy as the durable artifact.
- **PermissionRequest hold UX** (S9). A poorly chosen hold time could hide Claude's own prompt.
- **Codex hook trust friction.** Installed hooks don't run until the user trusts them, so onboarding has to show this.
- **Antigravity CLI is young.** Its hooks are known from the bundled doc and strings in agy 1.3.3, not yet from a captured payload (see "Corrections to the brief"); the adapter records one fixture per event before it ships, and the shape may move between versions.
- **Windows unpackaged toast limits** in dev builds.
- **macOS notifications need signing**, which needs an Apple Developer account (Account Holder).
- **The 30-day Claude cleanup** deletes transcripts if the app isn't opened for a month. The doctor offers to raise `cleanupPeriodDays`, with consent.
- **Scope.** v1 is large for one developer, so phases ship independently usable increments.
- **Name.** Marshell is clear on a web search only; get a proper trademark search before the first public release.
- **Crowded category** (Maestro, Orca, Agent Desk, Termexo, Schaltwerk, Belfry, herdr). The differentiators stay as in brief §6, "Borrowed from the field".

## First actions after approval

1. `git init` in `C:\dev\Airport`. Ask whether to rename the folder to `marshell`. Move the brief to `docs/BRIEF.md` and update it with the rename and the corrections above.
2. Run `/setup-matt-pocock-skills`. Create the GitHub repo (MIT) and file the tickets via `to-spec` → `to-tickets`.
3. Phase 0 design and the phase 1 spikes run in parallel.

## Verification (of the plan's execution)

Each phase's exit criteria above are the checks. Before claiming any phase done:
- the CI matrix is green on all three OSes;
- the fixture and E2E suites pass;
- a manual run on this Windows machine with real Claude Code shows the behaviour.

## Sources

- Claude Code docs:
  - https://code.claude.com/docs/en/hooks
  - https://code.claude.com/docs/en/statusline
  - https://code.claude.com/docs/en/settings-reference
  - https://code.claude.com/docs/en/settings
  - https://code.claude.com/docs/en/cli-reference
  - https://code.claude.com/docs/en/sessions
  - https://code.claude.com/docs/en/memory
- Codex docs:
  - https://learn.chatgpt.com/docs/hooks
  - https://learn.chatgpt.com/docs/config-file/config-reference
  - https://learn.chatgpt.com/docs/agent-configuration/agents-md
  - https://learn.chatgpt.com/docs/developer-commands?surface=cli
- Antigravity CLI docs (Gemini CLI is dropped, so its docs are no longer sources):
  - https://www.antigravity.google/docs/cli/statusline
  - https://www.antigravity.google/docs/plugins?tab=cli
  - https://www.antigravity.google/docs/getting-started?tab=cli
- Tauri:
  - https://v2.tauri.app/plugin/notification/
  - https://v2.tauri.app/reference/javascript/api/namespacewindow/
  - https://v2.tauri.app/develop/debug/linux-graphics/
  - https://v2.tauri.app/distribute/sign/windows/
  - https://v2.tauri.app/distribute/sign/macos/
- xterm.js:
  - https://github.com/xtermjs/xterm.js/releases (6.0 removed canvas)
  - https://github.com/xtermjs/xterm.js/issues/5894
  - WebKitGTK fix: https://github.com/VoltiusApp/voltius/pull/313
- portable-pty DSR issue: https://github.com/timeloop-vault/skein/issues/484
- rusqlite FTS5: https://github.com/rusqlite/rusqlite/blob/master/libsqlite3-sys/build.rs
- asciicast v2: https://docs.asciinema.org/manual/asciicast/v2/
- Apple trademark list: https://www.apple.com/legal/intellectual-property/trademark/appletmlist.html
- Versions (verified 2026-10-09):

  | Package | Version |
  |---|---|
  | tauri | 2.12.2 |
  | axum | 0.8.9 |
  | tokio | 1.53 |
  | toml_edit | 0.25 |
  | notify | 8.2 |
  | rodio | 0.22 |
  | rusqlite | 0.40 |
  | ts-rs | 12 |
  | @xterm/xterm | 6.0 |
  | vite | 8.3 |
  | react | 19.3 |
  | radix-ui | 1.7 |
