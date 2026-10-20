# Approve from anywhere (slice 3d) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Claude Code permission request shows as a live approve card in the needs-you lane, with the risk check, and can be answered from the card. The lane is announced. Peek shows a session's last rows. The taskbar shows the needs-you count and flashes with a sound when Marshell isn't focused. A security review gates the slice.

**Architecture:**
- **Core:**
  - The core turns each held `PendingPermission` into a `PermissionView`: what is asked, the risk reason, and the exact Always rule.
  - The view is built by a pure Claude mapper and a pure risk classifier.
  - Holds publish the list on a `watch` channel, and `/v1/events` sends it as a new `permissions` frame beside `sessions`.
- **UI:** LiveApp maps views to the existing `ApprovalRequest` and feeds the existing Lane and ApproveCard through a new controller that posts to the existing decision and release routes.
- **Peek:** reads a new `GET /v1/sessions/{tab}/peek?lines=30`.
- **Badge, flash and sound:** two Tauri commands in the app shell. The sound uses rodio, the badge an overlay icon, and the flash `request_user_attention`.

**Tech Stack:** Rust 2024 (axum, tokio watch, vt100, diffy), Tauri 2, rodio, ts-rs, React/TS, Vitest (`renderToStaticMarkup`).

**Spec:**
- `docs/PLAN.md`:
  - "The approve card" (lines 209–238).
  - Sidebar rows (186–207).
  - Key flows (280–284).
  - Motion and latency budgets (329, 393).
  - §3 route list (519).
  - §5 "Permission scopes" and "Permission releases".
  - The platform table rows for notifications, DND and sound.
  - Keyboard routing (lane order, Peek on Space).
  - §9 "Slice 3d decisions (2026-10-19)" and 3c decision 3.
- `docs/spikes/S9-permission-hold.md` and `docs/adr/0001-threat-model.md`.

## Global Constraints

- **Card keyboard safety (PLAN 222–226):**
  - The card is reached only by deliberate focus: Ctrl+Shift+N, a click, or Tab inside the lane.
  - **Enter never allows** (it jumps to the session).
  - Keys and buttons stay inert for 500 ms after focus or a content change.
  - There is no global approve shortcut.
  - Y, N, A and T act only while the card has focus.
- **Risky requests:**
  - Allow becomes hold Y for 600 ms.
  - Always is hidden.
  - A truncated payload must be expanded before Allow arms.
- **After a decision:**
  - A one-line receipt ("Allowed · npm test") for 1.5 s, then a 140 ms exit.
  - No undo, and the copy never implies one.
  - Focus returns to where the person was typing.
- **Scopes:** Always means `PermissionScope::Project` (`localSettings`), and its button spells out the exact rule: "Always allow `Bash(npm test:*)` in this project".
- **Where Always shows (3d decisions 1–2):**
  - A request with no `addRules` suggestion gets no Always.
  - Sessions keep each CLI's own mode flags, so no mode-based auto-answering.
- **The lane:** ordered oldest-waiting first, and Ctrl+Shift+N follows that order. Ages use `formatAge` ("<1m", then minutes) on screen and in names.
- **A request with no tab, or whose tab isn't open:** answered 204 at once (3d decision 3).
- **Live regions:** only the existing `SessionAnnouncer` / `Announcer`. No new live region (`sharedBlocks.test.ts`, `rules.test.tsx`).
- **Peek:**
  - Plain monospace text, no colours.
  - Shows within 100 ms of the request.
  - 120 ms fade plus a 2 px rise; reduced motion is a fade only.
  - Esc closes it and returns focus to the terminal.
- **Badge, flash and sound:**
  - The taskbar overlay shows the count whenever it is above 0.
  - Flash and sound fire only on a new needs-you while the window isn't focused.
  - Muted sessions never flash or sound.
  - A Settings switch mutes the sound.
- **Every UI change works:**
  - with the keyboard alone;
  - in forced colours;
  - with label-in-name (`src/lib/labelInName.ts`).
- **Dependencies, guard and CSP:**
  - The one new dependency is `rodio` in `src-tauri` (approved, 3d decision 4).
  - New routes go behind the existing `auth_guard` and `host_guard`, and the guard and token code are unchanged.
  - No CSP change.
- **Generated types:** `src/generated/` only from `cargo test -p marshell-protocol`. Run `cargo fmt --all` before every Rust commit.
- **Scratch only:** never write the real `~/.claude`, `~/.codex` or `~/.marshell`, and no real e-mail in fixtures.

## Review Focus

- **A decision for a request that has just left** (the hold expired, typing released it, or the tab closed): the route returns 404 and the card leaves with "This request is no longer waiting. Answer it in the terminal." It is never answered twice and never shows an error dialog.
- **Two held requests from one tab** (parallel tool calls): two lane entries, oldest first, each answered on its own. The badge counts entries, not tabs.
- **Risky Bash written other ways:**
  - `rm -fr`, `rm -r -f`;
  - `Remove-Item -Recurse -Force`, `rd /s /q`;
  - `git push origin +main`, `git push --force-with-lease`;
  - `iwr … | iex`, `curl … | bash`;
  - `sudo`, `Start-Process -Verb RunAs`.

  All are risky. `rm file.txt`, `git push`, `curl http://localhost:3000` are not.
- **An unknown tool or malformed `tool_input`** (`NotebookEdit`, `mcp__github__create_pr` with nested args, a missing `command`): the card falls back to the tool name and its input as `key: value` lines. Never a panic, and never a blank card.
- **The focused card's tab closes,** or the request is answered in the terminal: the card leaves, focus goes back to the terminal (or the sidebar list when no terminal is on screen), and the badge drops.

---

### Task 1: Protocol types

**Files:**
- Modify: `crates/protocol/src/hook.rs`, `crates/protocol/src/api.rs`, `crates/protocol/src/agents.rs`
- Test: the existing ts-rs export test in `crates/protocol`

**Interfaces:**
- **Produces, in `marshell_protocol::hook`** (all `#[ts(export)]`, serde `snake_case`, tagged `kind` for the enum):
  - `enum DiffKind { Context, Add, Del }`
  - `struct DiffLine { kind: DiffKind, text: String }`
  - The detail enum:
    ```
    enum PermissionDetail {
      Bash { command: String, cwd: Option<String> },
      Edit { path: String, add: u32, del: u32, hunk: Vec<DiffLine> },
      Write { path: String, preview: String, is_new: bool },
      Fetch { url: String },
      Mcp { server: String, tool: String, args: Vec<(String, String)> },
      Other { tool: String, args: Vec<(String, String)> },
    }
    ```
  - `struct PermissionView { rid: String, tab: String, agent: String, at_ms: u64, detail: PermissionDetail, risk: Option<String>, always_rule: Option<String> }`
    - `risk` is the plain reason, present only when the request is risky.
    - `always_rule` is the exact rule text, e.g. `Bash(npm test:*)`.
  - `struct PeekRows { rows: Vec<String> }`
- **Changes to existing types:**
  - `HookPlanRequest` gains `approve: bool` (`#[serde(default)]`): "Add the PermissionRequest hook so requests can be answered from Marshell."
  - `Settings` gains `sound_off: bool` (`#[serde(default)]`).
- **u64 fields:** keep the existing `#[ts(type = "number")]` convention, so `pnpm check:generated` stays free of `bigint`.

- [ ] **Step 1:** Add the types.
- [ ] **Step 2:** Run `cargo test -p marshell-protocol`.
  - Expected: PASS, with the new files in `src/generated/`.
- [ ] **Step 3:** Run `pnpm check:generated`.
  - Expected: exit 0.
- [ ] **Step 4:** Fix any TS fixture literals the new required fields break.
  - `sound_off` is serde-default, but TS literals of `Settings` and `HookPlanRequest` need the field.
  - Run `pnpm build`. Expected: built.
- [ ] **Step 5:** Commit: `feat(protocol): add the permission view, peek rows, the approve hook toggle and the sound switch`.

### Task 2: The request view and the risk check (core, pure)

Worker effort: **high** (a parser with a security role).

**Files:**
- Create:
  - `crates/core/src/hooks/risk.rs`: the classifier, agent-neutral, working on `PermissionDetail`.
  - `crates/core/src/agents/claude/request.rs`: the Claude mapper.
- Modify:
  - `crates/core/src/agents/mod.rs`: an agent dispatch `pub fn permission_view(p: &PendingPermission) -> Option<PermissionView>`, which returns `None` for an unknown agent or a `None` tab.
  - `crates/core/src/hooks/mod.rs`
- Test: unit tests in both new files. Mapper tests also use `crates/core/tests/fixtures/claude/2.1.302/PermissionRequest.json`.

**Interfaces:**
- **Produces:**
  - `pub fn claude::request::detail(request: &Value) -> PermissionDetail`
  - `pub fn claude::request::always_rule(request: &Value) -> Option<String>`
  - `pub fn risk::classify(detail: &PermissionDetail, cwd: &Path) -> Option<String>`
  - `pub fn claude::request::view(p: &PendingPermission) -> Option<PermissionView>`
- **Consumes:** `PendingPermission` (`hooks/holds.rs:17`) and Task 1's types.

**Mapping (`tool_name` → detail):**
- **Bash:** `command`, with `cwd` only when it differs from the payload's `cwd`.
- **Edit:** `file_path`, `old_string`, `new_string`. Build the first hunk with `diffy` (already a dependency), and count the added and deleted lines.
- **Write:** `file_path`, `content`. `preview` is the first 4 lines, and `is_new` is whether the file exists.
- **WebFetch:** `url`.
- **`mcp__<server>__<tool>`:** `Mcp`, with the args flattened to `key: value`. A non-string value is written as compact JSON.
- **Anything else, or a missing field:** `Other`, with the tool name and its flattened input.

**`always_rule`:**
- Take the first `permission_suggestions` entry with `type == "addRules"` and `behavior == "allow"`.
- Render each rule as `toolName(ruleContent)`, or just `toolName` when there is no content, and join several with ", ".
- `None` when there is none. This must agree with `permission_output`'s "no rule to add" (`agents/claude/mod.rs:131`).

**Risk check (PLAN 229), first match wins.** The reason copy is exact:

| Rule | Reason |
|---|---|
| Recursive delete: `rm` with `-r`/`-R`/`-rf`/`-fr`/`--recursive`; `Remove-Item … -Recurse`; `rd`/`rmdir` with `/s` | `Deletes files recursively` |
| The same, with a target outside the session `cwd` | `Deletes files recursively outside this project` |
| Force push: `git push` with `-f`, `--force`, `--force-with-lease`, or a `+` refspec | `Force-pushes over the remote history` |
| `git reset --hard` | `Discards uncommitted changes` |
| A download piped into a shell: `curl`/`wget`/`iwr`/`irm`/`Invoke-WebRequest`/`Invoke-RestMethod` piped into `sh`/`bash`/`zsh`/`iex`/`Invoke-Expression`/`pwsh`/`powershell` | `Runs a script downloaded from the internet` |
| Elevation: `sudo`, `runas`, `Start-Process … -Verb RunAs`, `gsudo` | `Runs with administrator rights` |
| An Edit or Write whose path is outside `cwd` | `Writes outside this project` |
| An Edit or Write whose path has a component `.env*`, `.ssh`, or any dot-named file or folder in the home folder | `Writes to <name>` |
| WebFetch, or a Bash `curl`/`wget`/`iwr`/`irm`, to a host that isn't `localhost`, `127.0.0.1` or `::1` | `Reaches <host> on the network` |

- Commands split on `;`, `&&`, `||`, `|` and newlines, then match per segment, case-insensitive for PowerShell cmdlets.
- Paths resolve against `cwd`, and the comparison is case-insensitive on Windows.
- Never a panic: unparseable input is not risky through the parser.
- Bash with an unknown tool falls through to `None`.

- [ ] **Step 1: Write the failing tests.**
  - A table test `classify_table` covering every row above and every Review Focus Bash form, plus these not-risky cases:
    - `rm file.txt`, `git push`, `git reset --soft HEAD~1`, `curl http://localhost:3000`;
    - a Write to `src/a.ts` inside `cwd`;
    - `ls -la ~/.ssh` (read-only).
  - `detail_from_fixture`: the 2.1.302 fixture gives `Bash { command: "python -c \"print(4242)\"", cwd: None }` and `always_rule == Some("Bash(python -c \"print(4242)\")")`.
  - `detail_edit_counts_the_hunk`.
  - `detail_mcp_flattens_args`.
  - `detail_unknown_tool_is_other`.
  - `detail_missing_command_is_other`.
  - `always_rule_none_for_add_directories_only`.
- [ ] **Step 2:** Run `cargo test -p marshell-core risk:: request::`.
  - Expected: FAIL (missing).
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same command.
  - Expected: PASS.
- [ ] **Step 5:** `cargo fmt --all`, then commit: `feat(core): describe a held permission request and classify its risk`.

### Task 3: Holds push, the events frame, no-tab answers and the status after a decision

Worker effort: **high** (concurrency, the local API).

**Files:**
- Modify:
  - `crates/core/src/hooks/holds.rs`
  - `crates/core/src/server/hook.rs` (`post`, `hold_permission`)
  - `crates/core/src/server/permissions.rs` (`decision`)
  - `crates/core/src/server/events.rs`
- Tests:
  - Unit tests in `holds.rs`.
  - Integration tests beside the existing permission tests in `crates/core/tests/` (find them with `grep -rl "permissions/" crates/core/tests`).

**Interfaces:**
- **Produces:**
  - `Holds::subscribe(&self) -> watch::Receiver<u64>`: a change counter, bumped by every open, decide, release, release_tab and drop.
  - `AppState::permission_views(&self) -> Vec<PermissionView>`: oldest first, built with `agents::permission_view`.
  - `/v1/events` frames `{"type":"permissions","data":[PermissionView]}`:
    - one on connect, after the first `sessions` frame;
    - one on every change.
- **Consumes:** Task 2's `permission_view`.

**Behaviour:**
- **In `post`:** a `Permission` role with no tab header, or a tab where `registry().is_open(tab)` is false, returns 204 at once. No hold is opened.
- **In `decision`:** after `holds().decide` succeeds, set the tab's live status to `Working` with the phrase `Thinking`. This applies to both allow and deny, and uses the existing `registry().update_live` (Claude continues after either).
- **Release:** a release leaves the live status alone, because the native prompt is still showing.
- **Locking:** never hold the holds lock while sending on the socket (PLAN §4 concurrency rule).

- [ ] **Step 1: Write the failing tests.**
  - `no_tab_permission_is_answered_at_once`: a POST with `X-Marshell-Role: permission` and no tab returns 204 in under 1 s while the hold is set to 120 s.
  - `closed_tab_permission_is_answered_at_once`.
  - `events_socket_sends_permissions_on_open_and_change`: connect, open a hold for an open tab, receive a frame with one view, decide it, then receive an empty list.
  - `decision_sets_working_thinking`.
  - `holds_subscribe_bumps_on_drop`.
- [ ] **Step 2:** Run `cargo test -p marshell-core permission`.
  - Expected: the new tests FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -p marshell-core`.
  - Expected: PASS. The existing S9-era hold tests are still green.
- [ ] **Step 5:** `cargo fmt --all`, then commit: `feat(core): push held permission requests to the UI and answer requests with no open tab at once`.

### Task 4: Peek route

**Files:**
- Modify:
  - `crates/core/src/pty/session.rs`: add `pub fn peek(&self, lines: usize) -> Vec<String>`.
  - `crates/core/src/server/sessions.rs`
  - `crates/core/src/server/mod.rs`: the route `GET /v1/sessions/{tab}/peek`.
- Test: a unit test in `session.rs` (or `pty/buffer.rs`, which already drives `vt100::Parser`), plus a route test.

**Interfaces:**
- **Produces:** `GET /v1/sessions/{tab}/peek?lines=N` returns `PeekRows`.
  - `N` defaults to 30 and is clamped to 1..=100.
  - 404 for an unknown tab.
- **Rows:**
  - The bottom `N` rows of `screen.screen()`, each row's text with trailing spaces trimmed.
  - Trailing blank rows at the bottom are dropped before taking N, so a short screen gives its content, not 30 blanks.
  - Read under the `output` lock, which is dropped before the response is built.

- [ ] **Step 1: Write the failing tests.**
  - `peek_returns_the_last_rows_of_a_tui_redraw`: feed `"a\r\nb\r\n\x1b[1;1Hz"`, then peek(30) gives `["z", "b"]`.
  - `peek_clamps_lines`.
  - `peek_route_404_for_unknown_tab`.
- [ ] **Step 2:** Run `cargo test -p marshell-core peek`.
  - Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -p marshell-core peek`.
  - Expected: PASS.
- [ ] **Step 5:** `cargo fmt --all`, then commit: `feat(core): add the peek route with a session's last rendered rows`.

### Task 5: The PermissionRequest hook in the install plan and the consent card

**Files:**
- Modify:
  - `crates/core/src/agents/claude/hooks.rs`: `install_plan`, the remove plan, and the setup state.
  - `src/features/onboarding/consent.ts`, `ConsentCard.tsx`
  - `src/features/agents/AgentsDialog.tsx` and its state helper.
- Tests: the existing tests in `hooks.rs`, `consent.test.ts`, `agentsState.test.ts` and `AgentsDialog` tests.

**Interfaces:**
- **Consumes:** `HookPlanRequest.approve` (Task 1).

**Install and remove:**
- With `approve`, `install_plan` adds a `PermissionRequest` entry whose command is the bridge in the `permission` role (`… marshell-hook" permission --agent claude --tag mshl1`, quoted as the other entries are), with `"timeout": 600`.
- Without it, no such entry is added, and an existing Marshell-tagged `PermissionRequest` entry is removed. That makes switching it off a previewed diff like any other.
- The remove plan and `marshell uninstall` remove it too.
- The setup state reports whether it is installed. Add a field next to the existing installed state, `approve_installed: bool`.

**Consent and Settings:**
- The consent card offers the "Approve from anywhere" capability, on by default (3d decision 5), so the diff shows the new entry.
- AgentsDialog: when hooks are installed without it, it shows "Approve from anywhere: off" and a "Set up" button that opens the same ConsentCard with only that capability's change.
- AgentsDialog also gets a "Needs-you sound" switch that writes `Settings.sound_off` through the existing settings route.

- [ ] **Step 1: Write the failing tests.**
  - `install_plan_adds_permission_request_when_approve`.
  - `install_plan_without_approve_removes_our_permission_request`.
  - `remove_plan_drops_permission_request`.
  - The consent test: the approve capability is offered and on by default.
  - The AgentsDialog test: "Approve from anywhere: off" and Set up show when `approve_installed` is false.
  - The sound switch renders and calls the settings writer.
- [ ] **Step 2:** Run `cargo test -p marshell-core hooks` and `pnpm test src/features/onboarding src/features/agents`.
  - Expected: the new tests FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same commands.
  - Expected: PASS.
- [ ] **Step 5:** `cargo fmt --all`, then commit: `feat(claude): offer approve from anywhere in the consent card and install its permission hook`.

### Task 6: Live requests in the lane and the card controller

**Files:**
- Create:
  - `src/features/approval/fromCore.ts`
  - `src/lib/api/permissions.ts`
  - `src/features/approval/LiveCard.tsx`
- Modify:
  - `src/lib/api/events.ts`: parse the `permissions` frame and call a new `onPermissions` callback.
  - `src/features/lane/Lane.tsx` and `LaneRow.tsx`: actions, `formatAge`, and `terminalOnly` entries.
  - `src/features/window/Sidebar.tsx` and `MainWindow.tsx`: pass handlers through, and let the drawer take a real request.
  - `src/features/approval/ApproveCard.tsx`: use `formatAge` for the wait.
  - `src/features/approval/types.ts`
  - `src/features/sessions/LiveApp.tsx`
- Tests:
  - `fromCore.test.ts`
  - `Lane.test.tsx`
  - `LiveCard.test.tsx` (the pure reducer; see below)
  - The existing `ApproveCard.test.tsx` and `labelInName.test.tsx`

**Interfaces:**
- **Produces:**
  - `toRequest(v: PermissionView, row: RowModel | undefined, now: number): ApprovalRequest`
    - `risk = { level: v.risk ? "risky" : "normal", reason: v.risk }`
    - `waitingMs = now - v.at_ms`
    - The session's name, agent and number come from the row.
  - `laneEntries(views: PermissionView[], rows: RowModel[], now: number): ApprovalRequest[]`
    - Every view becomes an entry.
    - A row in `needs-permission` or `needs-question` with no view becomes an entry with `terminalOnly: true` and detail `{ kind: "question", question: row.phrase, options: [] }`.
    - Entries are sorted oldest first. This list is the needs-you count everywhere: lane heading, badge, announcer.
  - `ApprovalRequest` gains `terminalOnly?: boolean` and `tab: string`.
  - In `src/lib/api/permissions.ts`:
    - `decide(rid: string, d: PermissionDecision): Promise<"ok" | "gone" | "conflict">`: 404 maps to gone and 409 to conflict.
    - `release(rid: string): Promise<"ok" | "gone">`
  - `cardReducer(state, event)` in `LiveCard.tsx`, pure and exported.
    - States: `pending | sending | receipt(verdict) | exiting | gone(message)`.
    - Events: `act`, `ok`, `gone`, `conflict`, `timer`, `vanished`.
    - `vanished` means the view left the list without our decision.
    - Receipt 1500 ms, exit 140 ms.
- **The card's actions:**
  - `allow` and `hold-allow` decide `{behavior:"allow", scope:"once"}`.
  - `deny` decides `{behavior:"deny"}`.
  - `always` decides `{behavior:"allow", scope:"project"}`.
  - `terminal` releases.
  - `expand` opens the drawer with this request's full detail.
  - `jump` selects the session's tab and focuses its terminal.
- **Receipts:**
  - Allowed: "Allowed · <subject>". Denied: "Denied · <subject>".
  - gone: "This request is no longer waiting. Answer it in the terminal."
  - conflict: the core's text.
- **Focus:**
  - Ctrl+Shift+N, added to the app keymap (`src/features/shortcuts`), focuses the oldest card. The next card becomes the target but does not take focus.
  - After a decision, focus returns to the last focused terminal pane.
- **Not done here:**
  - A `terminalOnly` entry is a LaneRow whose click and Enter jump to the session. It is never the full card.
  - LiveApp keeps the 500 ms arming and 600 ms hold from `useArming` and `useHold` unchanged.

- [ ] **Step 1: Write the failing tests.**
  - `toRequest` maps risk, rule, wait and session.
  - `laneEntries`:
    - adds terminal-only rows;
    - doesn't double-count a row that has a view;
    - sorts oldest first;
    - two views on one tab give two entries.
  - `cardReducer`:
    - act then ok gives receipt, then exiting after 1500, then removed after 140;
    - `gone` gives the gone message;
    - `vanished` while pending gives exiting.
  - `Lane` renders the first non-terminal-only entry as the card, and a terminal-only entry as a row with "Has a question".
  - The LaneRow and card names use `<1m`, and `nameContainsLabel` holds for the lane rows and the card.
  - `parseEvent` accepts a `permissions` frame.
  - `decide` maps 404 to "gone" (with a fetch stub, as the other api tests do).
- [ ] **Step 2:** Run `pnpm test src/features/approval src/features/lane src/lib/api`.
  - Expected: the new tests FAIL.
- [ ] **Step 3:** Implement and wire LiveApp:
  - `requests={laneEntries(...)}`;
  - the drawer request;
  - the `needsYou` count to `SessionAnnouncer`.
- [ ] **Step 4:** Run `pnpm test`, then `pnpm build`.
  - Expected: all pass, built.
- [ ] **Step 5:** Commit: `feat(ui): show live permission requests in the needs-you lane and answer them from the card`.

### Task 7: Peek

**Files:**
- Create:
  - `src/features/peek/Peek.tsx`, `Peek.css`
  - `src/features/peek/usePeek.ts`
  - `src/lib/api/peek.ts`
- Modify:
  - `src/features/sidebar/SessionRow.tsx` or its list: Space on a focused row, and a 350 ms hover.
  - `src/features/sessions/LiveApp.tsx`
- Tests: `Peek.test.tsx`, `Peek.css.test.ts`, `usePeek.test.ts` (pure timing helpers)

**Interfaces:**
- **Produces:**
  - `peek(tab: string, lines = 30): Promise<string[]>`
  - `<Peek row={RowModel} rows={string[] | null} onClose={() => void} />`

**Behaviour:**
- **Structure:**
  - A non-modal popover anchored beside the row, with `role="dialog"`, `aria-label="Peek: Session 6, api-server"`.
  - The rows go in a focusable `<pre>` (`tabIndex=0`) in the terminal font.
  - While loading it shows nothing. It never shows a spinner under 100 ms.
- **Opening:**
  - Space on a focused row opens it and moves focus to the `<pre>`.
  - Hover opens it after 350 ms without moving focus, and leaving both the row and Peek closes it.
- **Closing:** Esc closes it and focuses the last terminal pane, as PLAN 284 says.
- **Motion:** 120 ms fade plus a 2 px rise. Under `prefers-reduced-motion`, a fade only.
- **Forced colours:** a 1 px `CanvasText` border.
- **Empty and failed reads:** an empty screen shows "Nothing on screen yet". A failed read shows "Can't read this session's screen".

- [ ] **Step 1: Write the failing tests.**
  - Render with rows, giving a `<pre>` with the lines joined by `\n` and the dialog name above.
  - The empty and failure copy.
  - The CSS test: a reduced-motion rule and a forced-colors border.
  - `hoverDelay` is 350.
  - `nameContainsLabel` holds.
- [ ] **Step 2:** Run `pnpm test src/features/peek`.
  - Expected: FAIL.
- [ ] **Step 3:** Implement and wire.
- [ ] **Step 4:** Run `pnpm test`, then `pnpm build`.
  - Expected: pass, built.
- [ ] **Step 5:** Commit: `feat(ui): add Peek with a session's last 30 rows on Space or hover`.

### Task 8: Taskbar badge, flash and sound

**Files:**
- Create:
  - `src-tauri/src/attention.rs`: commands, the badge bitmap and the chime.
  - `src/features/attention/attention.ts`, `useAttention.ts`
- Modify:
  - `src-tauri/Cargo.toml`: add `rodio`, using the version PLAN names (0.22) with default features off and only the WAV/playback features needed.
  - `src-tauri/src/main.rs`: register the commands.
  - `src/features/sessions/LiveApp.tsx`
- Tests:
  - Unit tests in `attention.rs` (the bitmap only).
  - `attention.test.ts`

**Interfaces:**
- **Produces, Tauri commands:**
  - `set_badge(count: u32)`: the overlay icon on Windows (`WebviewWindow::set_overlay_icon`), cleared at 0.
  - `alert_needs_you(sound: bool)`: `request_user_attention(Some(UserAttentionType::Informational))`, then the chime on a background thread when `sound`.
- **Produces, Rust:** `pub fn badge_rgba(count: u32) -> (Vec<u8>, u32, u32)`.
  - A 16×16 filled disc in the attention colour, with the count drawn in a built-in 3×5 pixel digit font.
  - "9+" above 9.
  - No asset files.
- **The chime:**
  - Synthesised with rodio sources: two sine tones, 880 Hz then 1320 Hz, 90 ms each with a 10 ms fade, at 0.25 volume. No bundled audio file, so no licence notice.
  - Skipped when `SHQueryUserNotificationState` reports busy, presentation or quiet time, if the `windows` crate with that feature is already in the tree. Otherwise ledger a ruling and leave DND to phase 4.
  - A failure to open audio is ignored (logged).
- **Produces, TS:** `attentionStep(prev: string[], next: ApprovalRequest[], focused: boolean, muted: Set<string>): { count: number; alert: boolean }`.
  - `alert` is true only when an entry id that wasn't in `prev` arrives, the window isn't focused, and the entry's tab isn't muted.
  - `useAttention` calls `set_badge` on every count change and `alert_needs_you(!settings.sound_off)` on alert.
  - It uses `document.hasFocus()` and the window's focus and blur events.

- [ ] **Step 1: Write the failing tests.**
  - `badge_rgba_sizes_and_nine_plus`: 16×16×4 bytes, and the "9+" and "3" bitmaps differ.
  - `attentionStep`:
    - a new entry while unfocused alerts;
    - focused doesn't;
    - muted doesn't;
    - the same entries again don't;
    - the count follows the entries.
- [ ] **Step 2:** Run `cargo test -p marshell-app attention` and `pnpm test src/features/attention`.
  - Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same commands, then `cargo clippy --workspace --all-targets -- -D warnings`.
  - Expected: pass, 0 warnings.
- [ ] **Step 5:** `cargo fmt --all`, then commit: `feat(app): show the needs-you count on the taskbar and flash with a chime when Marshell isn't focused`.

### Task 9: Live check

**Files:**
- Create: `docs/research/2026-10-19-approve-from-anywhere-live-check.md`

**Setup:** as CLAUDE.md "Live check":
- a scratch `MARSHELL_HOME`;
- `MARSHELL_CLAUDE_SETTINGS` pointing at a scratch project's `.claude/settings.local.json`;
- CDP on 9333.

Apply the consent card with Approve from anywhere on in a Claude session started for the check.

**Checks:**
1. A plain Bash request:
   - the card shows within a second, with the exact command;
   - Y within 500 ms of focus does nothing;
   - Y after that allows, and the receipt shows then leaves;
   - focus returns to the terminal.
2. Deny with N: Claude says it was denied.
3. Always with A: the scratch `settings.local.json` gains the rule, and the same command then runs without asking.
4. A risky `rm -rf` of a scratch subfolder: the card reads the risk reason, Always is hidden, and a short Y does nothing. Deny it.
5. Typing in the pane releases the card, which leaves with no error.
6. A request with no tab (the bridge run from a plain terminal, no `MARSHELL_TAB_ID`) returns at once.
7. Peek opens on Space and closes with Esc, back to the terminal.
8. Badge and flash, with the window unfocused through CDP or a second window:
   - `set_badge` and `alert_needs_you` are called (check the app log or a test hook);
   - a screenshot of the taskbar judges the overlay.
9. Forced colours (CDP emulation) and keyboard-only for the card and Peek.

Each finding is fixed with a test that fails first, and listed with its commit.

- [ ] Run the checks, write the doc, and commit: `docs(research): record the approve-from-anywhere live check`.

### Task 10: Security review gate

**Files:**
- Modify: `docs/adr/0001-threat-model.md`, which gets a "Reviewed for 3d" section.

- [ ] **Step 1:** Dispatch an Opus security review of `origin/main..HEAD` against ADR 0001. It must cover:
  - the decision and release routes;
  - the new `permissions` frame and peek route behind the guards;
  - what a same-user process or a web page can do with them;
  - card keyboard safety (no path from terminal keystrokes to a decision);
  - the risk check's bypasses;
  - the no-tab rule;
  - the PermissionRequest entry's command quoting;
  - Peek leaking a screen to anyone without the token.
- [ ] **Step 2:** Fix each Critical and Important finding with a test that fails first. Ledger the rest.
- [ ] **Step 3:** Write the ADR section: what was reviewed, findings and fixes, and the residual risks (such as risk-check evasion by obfuscated commands, which still gets Claude's own dialog). Commit: `docs(adr): record the security review of approve from anywhere`.

### Task 11: Whole-branch review and PR

- [ ] The final whole-branch review on Opus, then one fix pass, each fix RED→GREEN.
- [ ] The CI-equivalent checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `pnpm test`
  - `pnpm check:generated`
  - `pnpm build`
  - `pnpm tauri build --no-bundle`
- [ ] The PR (What it does / Live check / Known limits / Checks), green CI, merge, and delete the ledger.
