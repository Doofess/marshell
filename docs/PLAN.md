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
| Installed CLIs | Claude Code 2.1.295 only. Codex and Antigravity (`agy`) get installed before phase 6, so fixtures are recorded from the real CLIs. |
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
   - Plugins (skills, agents, rules, MCP, hooks) live in `~/.gemini/antigravity-cli/plugins/<name>/`; `agy plugin list`. A plugin's `hooks.json` runs commands "before or after tool calls", and that is all the docs say: the event names, a settings-level hook config and any permission-grant hook are **unverified**, to be verified when the adapter is built from a real install. Until then the consent card offers only the status line (no hook entry count) and the approve card focuses the tab.
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
| Top bar, sidebar column | macOS: traffic lights, inset 78 px. Windows: the app icon and the lowercase "marshell" wordmark (vector, Space Grotesk SemiBold) as one button that opens the app menu, then a palette button. In the 52 px rail only the icon fits, so the palette button moves to the foot of the rail. |
| Top bar, main column | Session header: name · project/branch · mode chip · model/effort · subagents · context ring · port chips. |
| Window controls | Windows: 46×40 controls at the right. Linux: native decorations plus a 36 px header. |
| Sidebar | Default 288 px, minimum 200, maximum 400, rail 52, focus mode 0. Drag the edge to resize; double-click resets. |
| Sidebar footer | 40 px: plan ring, 5h %, and doctor glyph with issue count. |
| Needs-you lane | The header is **always present** at 28 px ("Needs you · 2" or "All clear"), so it never pops in or out. |
| Lane cards | Accordion: only the targeted card (oldest waiting) is expanded, at about 250 px (measured: 246 for a question, 255 for a command with its Always and terminal links). The others are 36 px one-liners. |
| Lane height | At most 40% of sidebar height, in whole rows, and it **never scrolls**: it shows the targeted card and as many 36 px one-liners as fit, and a "+N more" button in its header opens a menu with every waiting request. If even one 256 px card does not fit in 40% (a sidebar under about 640 px tall), it shows one-liners only. |
| Sidebar list and rail | The **session list scrolls** (the sidebar list and the rail list), with **no visible scrollbar**: wheel, touch and keys, and a soft fade at an edge only while more rows continue that way. Everything else in the main window never scrolls; the lane keeps its "+N more" button, and the all-requests menu may scroll with the themed scrollbar. |
| Shell panel | A plain shell (PowerShell on Windows, zsh on macOS, bash on Linux) in the expanded sidebar between the list and the footer, for running manual commands. It is not tied to any agent: no vendor colour, no prompt line, typing there never reaches a session. Header 32 px with the shell name, the focused session's folder, "Open in a pane" and hide/show; body 30% of the sidebar (120 to 280 px), a real terminal about 34 columns wide. It folds to its header by itself when the list would be left with under two rows, and the user's choice to hide it always wins. Ctrl+Shift+S (Cmd+S) shows or focuses it. Not in the rail, focus mode or below 960 px. |
| Right drawer | Default 360 px, minimum 300, maximum 560. It pushes the terminal, and overlays instead when the terminal would drop below 720 px. |
| Split view | **Up to four terminals on screen at once** (any number can run; this is only how many are visible). The layout button in the top bar opens a menu: 1 to 4 panes, an arrangement per count (2: side by side or stacked; 3: one main beside two, one main on top, side by side or stacked; 4: grid, side by side or stacked) and an Automatic switch. Automatic picks the arrangement that keeps the smallest pane closest to 80 by 24, judged on even panes so dragging a divider never flips it. **Every pane for an AI agent carries its own prompt line** (see Prompt line; a pane for a manual CLI has none and gives those rows to its terminal), so each such pane's terminal is its height minus the 28 px header, its prompt line and the padding. Dividers are a 1 px hairline with an 8 px hit area that overhangs the panes, so a divider costs one pixel of terminal. Each pane header (28 px) has the brand stripe, the status glyph (the vendor mark moving while working), name, project, and zoom and remove buttons; a pane that needs you keeps a vendor tint at full strength while other inactive headers sit at 60%. **Four only when the window is big enough to use them:** a count is offered only if some arrangement gives every pane at least 60 columns by 14 rows **of terminal** (each pane's header and own prompt line come on top, and the status bar's height counts). Below that the layout shows fewer (the smallest window shows one), the layout menu greys out the counts that do not fit and says "Showing 2 of 3", and the sessions not shown keep running (the focused pane's header says "N more running"). A pane under 80 columns says its size ("39×11"). Zoom (Ctrl+Shift+P, Cmd+P on macOS) fills the view and the others keep running. A new pane starts empty and offers the sessions not on screen. The active pane has a 2 px bar in the vendor colour. Terminal text is **never** dimmed. **A pane header is that pane's title bar:** the number and the name are the last things to give way (the project shrinks first). The app window's own title in the OS title bar and taskbar follows the focused session ("billing — Marshell", or "Marshell" when none is focused). The window has **one** screen-reader live region, for the focused session (empty on first render, announces only a change, never a repeat); the prompt lines in the panes are plain text. |
| Focus mode | Sidebar and header go to 0. The header reveals on an 8 px top-edge hover. A floating pill at top right reads "2 need you ⌃⇧N" and stays silent apart from its glyph bounce. |
| Prompt line | Every session on screen has its **own** prompt line, so a line of text always goes to the session of the pane it was typed in. A single terminal has one **full** line under it (**66 px**); in split view every pane has a **compact** line docked at its foot (**42 px**), including a zoomed pane. An empty pane has none, and **neither does a manual CLI** (a shell or dev tool is a plain terminal: you type in the terminal, see Session numbers and manual CLIs). The look is a **flat command-line**, not a floating card or a rounded composer: a docked bar with a prompt marker and a field, separated from the terminal by a hairline and a vendor-coloured top edge on the focused one, so it reads as part of the terminal rather than a chat box. The height is fixed in a slot, so the terminals never resize when a session starts or stops working; a long prompt grows the line upward over its own terminal while it is written, then scrolls with its scrollbar hidden. It takes its session's vendor colour. It is blocked while that session waits on the user or has ended, so typed text can never answer a request by accident; the reason sits in the line ("billing is waiting for your approval"). Only that pane's line is off: the others stay live. Enter sends (Queue while the session is working), Shift+Enter adds a line, Ctrl+Shift+L (Cmd+L on macOS) puts the cursor in the focused pane's prompt line without changing which pane is focused. Hidden in focus mode. The terminal itself stays typeable. |
| Status bar | One **24 px** bar at the foot of the centre column, the same in a single window and in split view. It carries the status strip (inspired by the user's statusline.py) for the **focused** pane's session: model and effort, context % with a tiny bar and used/size, the 5-hour and 7-day limits with "resets in", cost, session time, uncommitted changes, MCP health, version, skills, venv and ports. Unknown values are left out of the strip, never shown as a dash; items drop by priority as the window narrows (the model, context and 5-hour limit last) and never wrap; an MCP problem replaces the healthy count and outranks everything. Hidden in focus mode. Which items every CLI can supply is in `docs/research/2026-10-10-cli-data-sources.md`: only project, branch, elapsed time, ports, venv and version are universal. **It starts with the focused session's number and name** (never dropped, ellipsized only at its own 24-character cap), so the facts always say whose they are; the strip fits in what is left. |
| Rail (52 px) | Brand stripe, the agent mark and a 2-letter monogram per session, with a small status glyph. While a session works, its vendor mark carries the working loop in place of any pulsing dot. Needs-you shows as a badge stack with a count at the top. It shows the items that fit and a "+N" button for the rest; the command palette button sits at its foot, since the bar's 52 px column holds only the logo. |

**Terminal theme.** The terminal has its own palette, separate from the app theme.
- Setting: **Follow app** (default), **Always dark**, **Always light**, or an imported theme; a profile can override it. Switching applies to every open terminal at once through xterm's live `options.theme`, keeping scrollback.
- Switching changes the default foreground and background, cursor, selection and the 16 ANSI colours. Colours a program painted in 24-bit RGB stay as drawn, so `minimumContrastRatio` (default 4.5) is on to keep them readable.
- Programs that read the background once at startup keep their old look until restarted. Marshell helps in three ways: it answers OSC 10/11 colour queries with the current theme, sets `COLORFGBG` at launch, and sends the colour-scheme-change notice (mode 2031) to programs that opt in. The setting's help text says what a running program may not pick up.

**Hover-freeze rule.** While the pointer is inside the sidebar, lane height changes are deferred until the pointer leaves, with a 3 s cap. Meanwhile a "1 new" chip appears in the lane header.

### Session numbers and manual CLIs

**Session numbers.** Every open terminal has a number, so "go talk to session 6" names one terminal.
- **Stable, lowest free.** A session keeps its number until it closes. A new session takes the lowest number nobody holds (`nextSessionNumber`), so with 1, 2, 3, 5 and 6 open the next one is 4, and a number never moves while its session lives. Numbers are never renumbered when the list is reordered.
- **Shown on every surface that names a session:** the sidebar row (a quiet tabular numeral between the mark and the name), the rail item (a small numeral on the mark), the session header, every pane header, the lane card ("1 · billing") and so the drawer, the all-sessions menu, and the palette and launcher results. It is a numeral on a quiet fill, never an outline, and it never decides truncation: the name keeps its room. Accessible names start with it: "Session 6, billing, ...". Its tooltip teaches the phrase: "Say 'session 6' to an agent and Marshell knows which terminal you mean."
- **Keys.** Ctrl+Shift+1 to 9 (Cmd+1 to 9 on macOS) go to sessions 1 to 9, as written in section 7. A higher number is "Go to session..." in the palette (type 12).
- **Data contract (design level).** The core owns the number (`crates/protocol`, later): it is part of the session record, assigned when the session starts and released when it closes. Each session's process gets `MARSHELL_SESSION_NUMBER` (and `MARSHELL_TAB_ID`) in its environment. The future Marshell skill, plugin and `marshell` CLI resolve "session 6" through `marshell list` and `marshell send 6 ...`, so an agent in one terminal can message another.

**Manual CLIs.** A terminal is not always an AI agent. A manual CLI is **any program that is not an LLM agent**: a shell, a dev tool, a remote tool, or anything else you run by hand. The named marks below are only the ones with a known mark; every other program is still a manual CLI and gets the neutral mark with its own name.
- **Mark.** Marshell picks the mark from the session's foreground process (its executable name) and draws it the way an AI vendor's is drawn: shells (PowerShell, Bash, Zsh, Command Prompt, Git Bash, WSL), dev tools (Git, Docker, kubectl, Node.js, Python, Cargo, GitHub CLI, Terraform), remote and cloud tools (SSH, AWS, Google Cloud, Azure, Supabase, Vercel), and a neutral "terminal program" mark for anything else, which shows the executable's name. The mark changes when the foreground command changes (PowerShell, then ssh). The Phase 0 marks are simple glyphs drawn here; the programs' own logos are sourced and checked before release.
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
| Auto / dontAsk / bypass | ⚠, **always visible** | — | "Full auto" | Header tinted red with a red bar on its start edge (bypass, auto) |
| Ended | Hollow square | — | "Ended · exit 0" | Text 60% |
| Limited (no hooks) | Dotted circle | — | "Status limited · set up" | Links to setup |
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

### Key flows

**First run (4 screens, under 60 s):**
1. **Detected:** CLIs found, with versions, and "Not installed" rows.
2. **Consent, one card per CLI**, in plain words. Example: "Add 9 hook entries and chain your status line in `C:\Users\Home Office\.claude\settings.json`."
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
| Mono font | **Import the user's existing terminal font** from Windows Terminal or iTerm settings, so Nerd Font prompts keep working. Otherwise use bundled JetBrains Mono, 13 px. |

**Colour tokens:** the source of truth is `src/styles/tokens.css` (oklch, `light-dark()`), `accents.css` and the Storybook Colours story; this plan holds no hex values. Rules that stay binding:
- Surfaces: true-black dark, warm-white light. Hover and selected are low-alpha overlays of the text colour. Text has three levels, and `--text-3` must pass AA on every surface.
- **The accent follows the vendor.** Everything in a session's row, card, pane or window takes that CLI's signal colour: focus rings, the active-pane bar, primary buttons, the needs-you tint. Where no session is in play (onboarding, settings, doctor) the user's chosen accent applies; the default is signal amber (8 accents: amber, blue, indigo, violet, magenta, cyan, teal, slate). Vendor colours are checked for 4.5:1 text on their button fill and 3:1 for rings and bars on every surface (`accentFollowsVendor.test.ts`).
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
    bridge/                   # two tiny console bins sharing a client module:
                              #   marshell-hook (hook + status line bridge), marshell (CLI: new/list/focus/send/uninstall)
    host/                     # marshell-host: Windows elevated pty helper (spike S8 in phase 1, ships in phase 6)
    fake-agent/               # test-only binary that emulates an agent: prints output, calls the bridge
  src-tauri/                  # thin window shell: starts core thread, window, tray, notifier, badge
  src/                        # React + TS frontend
    generated/                # ts-rs output, committed, CI checks it is fresh
    lib/api/                  # http client, pty socket, status socket
    tokens/                   # tokens.css (design tokens), themes
    components/               # primitives over radix-ui
    features/                 # sidebar, terminal, launcher, palette, departures, history, doctor, settings, editors
  stories/                    # Storybook mockups (approval gate before UI code)
  assets/{icon,sounds}/
  docs/                       # BRIEF.md (updated living spec), adr/, fixtures notes
  .github/workflows/ci.yml    # Win/macOS/Linux matrix
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

`PermissionMode` is the shared set {Manual, Plan, AutoEdit, FullAuto}; the CLI's own label is kept in `raw`.

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
- **Permission releases.** If the hold expires, the user clicks "Answer in terminal", or the user starts typing in that tab, the core releases the request. The bridge then exits 0 with no output and the native prompt takes over.
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
| Codex paths | `%USERPROFILE%\.codex` (or `CODEX_HOME`): `config.toml`, `hooks.json`, `AGENTS.md`, `sessions/YYYY/MM/DD/rollout-*.jsonl[.zst]`; admin `requirements.toml` is read-only to us | `~/.codex`, same layout | `~/.codex`, same layout |
| Antigravity paths | `%USERPROFILE%\.gemini\antigravity-cli\settings.json`, `plugins\<name>\`; binary `%LOCALAPPDATA%\agy\bin\agy.exe`; the transcript location comes from the status line payload (`transcript_path`) | `~/.gemini/antigravity-cli`, same layout; `~/.local/bin/agy` | same as macOS |
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
     - app-owned init scripts (OSC 7/133, PSReadLine and prompt options) and "make permanent"
     - themes, accent, high-contrast and large-text modes, theme import (Windows Terminal, iTerm2, VS Code)
     - **terminal theme** (below): follow-app default, per-profile override, live switch, contrast protection, background-colour answers
     - split view, clickable paths, layout restore, F2 rename, reopen a closed session, drag reorder and project grouping, focus mode and rail
     - asciicast v2 recording, palette basics, the keyboard routing above, the cheat sheet, brand colors
     - the structural UX rules, which are hard to retrofit: focus restoration, the hover-freeze rule, the permanent lane header, rows that never resize
     - **Windows process hygiene (from S2):** Job Object with kill-on-close so a crash leaves no orphans; sideloaded `conpty.dll` + `OpenConsole.exe` with the MIT notice; WebView2 bootstrapper; arm64 build.
     - **Resilience:** a core panic hook and supervisor with a "core restarted" UI state; every file in `~/.marshell` carries a `version` field, is written atomically and keeps a `.bak`; migrations run on load.
     - **Restart behaviour:** on relaunch tabs come back as "Ended" with a one-click Resume (the agent's own `--resume`). Nothing relaunches itself or spends tokens unasked.
   - Exit: 5 tabs across 3 shells; restart restores the layout; kill the app mid-task, relaunch, and every tab is back with Resume; no orphan processes after a crash; a recording replays in asciinema-player; OSC 133 prompt jumps work.
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
   - Exit: a real Claude session shows correct model, effort, mode, subagents and context; approval from a card works; uninstall leaves zero byte diff; corrupting `index.sqlite` is repaired on start.
4. **Notifications + choreography + Departures board + stuck detection** (S3).
   - Exit: every rule in brief §5.5 is verified on all three OSes. Every sound has a visual equivalent.
5. **History view.**
   - Scope: search, conversation view, files-changed tab, recording replay, bookmarks, tags, archive and delete-copy, export with redaction, import of sessions run outside the app, retention limits (size and age), the optional backup-folder mirror, a disk-usage view, recording compression and cleanup.
   - Exit: a 1000-session index searches in under 100 ms; recordings stay within a stated MB-per-hour budget.
6. **Agents, integration and the elevated helper.**
   - Scope: settings pane (read-only), Codex + Antigravity adapters (install both CLIs first; the Antigravity adapter verifies its hook events and any permission hook against a real install), custom agents, repo scripts (`marshell.json`), port chips, the `marshell` CLI and its built-in skill (when the app is closed, `marshell new` says so and offers to start it), project references (`@` picker, alias learning), WSL (S11), "open in system terminal" and "copy resume command", and the `marshell-host` elevated helper. S8 passed with requirements: the token goes over a pipe, never the command line, and the decline and wrong-token paths get tested. Until it ships, elevated sessions open in a separate elevated Marshell window.
   - Exit: fixture suites from the real CLIs pass.
7. **Doctor**: the fix actions, on top of the phase-3 config core: "Fix X issues" and one-click undo; folds in each CLI's own doctor; detects duplicate notifications; reports the tokens injected by project references; checks Codex hook trust; waste checks (unused MCP servers and skills, long descriptions, broad scopes, a token-cost estimate); offers to raise `cleanupPeriodDays`.
   - Exit: a seeded broken config is fully repaired and fully undone.
8. **Editors + instruction library** (wired per CLI with a content hash and drift check; settings raw mode validated against the SchemaStore schema), smart and native recaps, diff drawer, step timeline, saved prompts, broadcast.
   - Instruction library items: frontmatter form, "what loads here" view, token estimate with a 4k warning, templates and one-click project profiles, per-project toggles, agent filter, globs, and "Improve this file" (headless, opt-in).
   - Exit: one doc wired into all three CLIs, verified by each CLI loading it; the 32 KiB Codex limit warns before it truncates.
9. **Release.**
   - Scope: tagged-build workflow, signed and notarized artifacts (NSIS per-user + MSI, dmg, AppImage/deb/rpm), checksums, release notes, the Tauri updater with a signing key and a rollback path, installers that run the unhook on uninstall, a manual release checklist file (including the macOS run), `cargo-deny` and `pnpm audit`, licence notices for bundled fonts, sounds and OpenConsole, and the formal trademark search.
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
- **Config safety copy.** A card or a strip says exactly what changes and where. Undo copy only names things that exist and says what removal does for that CLI, built from its switches (`removalCopy` in `consent.ts`): Claude, "Marshell removes the hook entries it added and puts your own status line back from statusline-chain.json; the rest of the file stays as it is"; Codex, "Marshell removes the hook entries it added; the rest of the file stays as it is"; Antigravity, "Marshell puts your own status line back from statusline-chain.json; the rest of the file stays as it is". It never says "removes only the entries it added" where a status line command was replaced rather than added. A restore from a snapshot (the fix sheet, the settings change strip) is only promised for the snapshot taken by that same apply, for as long as the file is unchanged since. A chained status line shows where the user's own command is kept (`~/.marshell/agents/claude/statusline-chain.json`) as an added file in the diff. A file that is read-only, does not parse, or is managed is never described as about to change. A conflict offers "Review the difference" and "Reload", never an overwrite.
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

From brief §5.9 and §6: the installer runner (install and update CLIs), review-to-merge, start-from-issue, checkpoints, file tree, phone check-in, more than two panes, and the `marshelld` daemon. v1 keeps the core protocol-first so these are additions, not rewrites. Also deferred: Airport mode (relabel statuses), if wanted at all.

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
- **Antigravity CLI is young.** Its hook events and any permission hook are unverified (see "Corrections to the brief"); until the adapter confirms them against a real install, status comes through the chained status line and the approve card focuses the tab.
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
