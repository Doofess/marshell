# Slice 6aa-1: terminal ids instead of session numbers

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Session numbers go from the app, the core and the CLI. Every terminal gets a short id such as `t7`, which counts up, is never reused and is kept across restarts. Scripts and agents use that id. People name a terminal by "project · name", and the screen says "terminal" wherever it meant a tab.

**Architecture:**
- **Core.** The registry hands out `t<n>` from a counter that `layout.json` keeps, beside the internal tab id, which stays the key everywhere. `SessionInfo.number` becomes `terminal_id`, `MARSHELL_SESSION_NUMBER` becomes `MARSHELL_TERMINAL_ID`, and the name no longer gets a number appended.
- **Webview.** It never shows the id. Its navigation keys on the tab id, and a terminal it names away from its row reads "project · name".
- **CLI and skill.** The CLI resolves an id or a tab id and refuses a bare number with a pointer. The skill teaches agents to find a terminal by project and name.

**Tech Stack:** Rust (serde, axum, ts-rs), React 19 + TypeScript (Vitest in node, `renderToStaticMarkup`), the `marshell` CLI crate.

**Spec:** `docs/PLAN.md` §9:
- "Slice 6aa decisions (2026-11-28): Projects and Terminals", decisions 9, 10 and 17 (6aa-1's line).
- "Slice 6aa-1 decisions (2026-11-28)", with its rulings. This plan's own commit adds that list.

The exact copy for the "terminal" wording is in `docs/superpowers/plans/2026-11-28-phase-6aa-1-copy.md`.

## Global Constraints

- **Ids.**
  - An id is `t` followed by a decimal counter from 1, such as `t7`.
  - Ids count up per Marshell home and are never reused.
  - A restored terminal keeps its id, and a closed terminal's id is never given out again, even after a restart.
  - The tab id (`t` + 12 hex) stays the internal key, the route segment and `MARSHELL_TAB_ID`.
- **Variables.**
  - A terminal's process gets `MARSHELL_TERMINAL_ID` (its id), `MARSHELL_TAB_ID` and `MARSHELL_AGENT`.
  - `MARSHELL_SESSION_NUMBER` is no longer set anywhere.
  - Where Marshell strips its own variables (outside programs, the app started from a terminal), the old name is still stripped, so a terminal opened by an older build doesn't leak it.
- **No id or number on screen.** The webview draws, reads aloud and puts in a tooltip no terminal id and no terminal number, anywhere.
- **Names.**
  - Two terminals may have the same name, and nothing is appended to a taken one.
  - Away from its row, a terminal is named `{project} · {name}` (a middle dot U+00B7 with a space each side), where `{project}` is the last folder of `SessionInfo.project`. When that is empty, the name stands alone.
  - The sidebar row and the rail keep what they draw, minus the number.
- **Keys.** No key or command goes to a terminal by number. Ctrl+Shift+1–9 are left unbound until slice 6aa-3 gives them to projects.
- **Copy.** Use the exact strings in this plan and in the copy table, in sentence case, with no "!".
- **Generated types.** `src/generated/` is never edited by hand: `cargo test -p marshell-protocol -j 4` writes it, and the result is committed.
- **Build rules.**
  - No new dependencies.
  - Cargo always runs with `-j 4`, and `cargo fmt --all` runs before a Rust commit.
  - Every task leaves `pnpm build`, `pnpm test` and the touched crates' tests green.
- **Live check.** Don't start `pnpm tauri dev` before Task 9.
- **Worker rules.**
  - Workers don't run `git checkout`, `git restore` or `git stash` on files they don't own.
  - They delete nothing outside the files their task names.
- **Commits** read `type(scope): what changed, as a sentence.` and end with the session's attribution lines.

## What was checked before planning

- **Numbers in the core.**
  - Numbers are allocated lowest-free in `crates/core/src/registry.rs` (`lowest_free_number`). They are set by `adopt_script`, which also appends the number to a taken name (`"{name} {number}"`), and by `seed_ended`.
  - No lookup in the core is keyed by number: routes take `{tab}` (`crates/core/src/server/mod.rs`), and the CLI turns a number into a tab id (`crates/cli/src/client.rs` `resolve`).
- **The tab id.** `auth::new_id("t")` is `t` + 12 hex characters. It is used for tabs (`server/sessions.rs:598`) and for the shell panel (`server/shell_panel.rs:157`).
- **The shell panel** starts outside the registry with number 0, so its `MARSHELL_SESSION_NUMBER` is `0` today. It gets a number only when it is adopted (`registry.adopt`, `shell_panel.rs:256`).
- **Persistence.**
  - `SavedTab` (`crates/protocol/src/api.rs`) has `number` as a required field and no `deny_unknown_fields`, and neither has `LayoutDoc`.
  - `save_tabs` (`crates/core/src/server/layout.rs`) clones the in-memory doc, sets its `tabs` and saves only when the doc changed. The launcher and the view also write through that in-memory doc.
  - Nothing stores a counter today.
- **The webview and the layout.** The webview only GETs `LayoutDoc` and PUTs `ViewLayout` (`src/lib/api/layout.ts`), so `LayoutDoc.next_terminal` can be `#[ts(skip)]`.
- **Variables.** `MARSHELL_SESSION_NUMBER` is set at `server/sessions.rs:722` and stripped in `crates/core/src/outside.rs` (`marshell_markers`) and `crates/cli/src/start.rs` (`app_command`, a test asserts three removals).
- **Saved key bindings.** The webview stores no custom key bindings. Unknown command ids reach `commandById`, which throws. The palette's Recent list goes through a `byId` map and skips unknown ids (`src/features/palette/paletteList.ts` `emptyBox`).
- **Keymap groups.** The keymap has both a `Session` and a `Terminal` group (`src/lib/keymap.ts`); `paletteList.ts` and `ShortcutSheet.tsx` order them. The only `Digit1`–`Digit9` bindings are the nine `focus-session-N` commands.
- **The skill.**
  - Its text is `crates/core/src/agents/skill/SKILL.md`. A changed text needs the old hash in `PREVIOUS` (`crates/core/src/agents/skill.rs`), or an installed copy reads as Edited.
  - Alpha.2 shipped a changed text (commit `42b2c86d`, 2026-11-18) and its hash was never added. `git show v0.1.0-alpha.2:crates/core/src/agents/skill/SKILL.md | sha256sum` gives `1f850159a12b53c14a79b9c819e09d46c70abadda5b0465676409acae5b5d40e`, the same as today's file.
- **Notifications.** The core's notification copy is a port of `src/features/notifications/banner.ts`, so the two must say the same.

## Review Focus

1. **A `layout.json` from before ids** (tabs with `number`, no `terminal_id`, no `next_terminal`): it reads, its tabs get t1, t2… in saved order, they keep those ids at the next restart, and the next new terminal continues the count. Pinned by `an_old_layout_gets_ids_in_saved_order` (Task 5), `a_layout_from_before_ids_still_reads` (Task 6) and the live check (Task 9).
2. **The counter outlives the terminal that used it:**
   - Close the newest terminal, then restart: its id is not handed out again.
   - Writes to the layout that aren't the tab saver (the view, the launcher's remember) keep `next_terminal`.
   - Pinned by `a_closed_newest_id_is_not_given_again_after_a_restart` and `layout_writes_keep_the_counter` (Task 5).
3. **Old number habits:**
   - `marshell send 3`, `#3`, `focus 3`, `open --session t2`, and an agent on the old skill that reads an unset `MARSHELL_SESSION_NUMBER`.
   - Each one gets a pointer or a usage error, never another terminal.
   - Pinned by `a_bare_number_is_pointed_at_ids` and `the_session_flag_points_at_terminal` (Task 6), and `the_skill_names_ids_not_numbers` (Task 7).
4. **What older builds left in the webview:**
   - A palette Recent list that names `focus-session-3` or `go-to-session` opens without them and without a throw.
   - Ctrl+Shift+1 in a focused terminal reaches the program in it.
   - Pinned by `a_recent_list_from_an_older_build_skips_removed_commands` (Task 1) and the live check (Task 9).
5. **Two terminals with the same name in one project:**
   - `marshell send <id>` reaches the one named by id.
   - The navigation, now keyed by tab id, selects the right row.
   - The notifications and the tray read the same label for both, which is accepted.
   - Pinned by `send_by_id_reaches_one_of_two_same_named` (Task 6) and `two_rows_with_one_name_are_two_targets` (Task 2).

---

### Task 1: no key, command or search goes to a terminal by number

**Files:**
- Modify: `src/lib/keymap.ts`, `src/lib/appShortcuts.ts`, `src/features/shortcuts/ShortcutSheet.tsx`, `src/features/settings/settingsRouting.ts`, `src/features/sessions/LiveShortcuts.tsx`
- Modify: `src/features/palette/Palette.tsx`, `src/features/palette/runnable.ts`, `src/features/launcher/filter.ts`, `src/features/launcher/Launcher.tsx`, `src/features/sessions/LiveLauncher.tsx`, `src/features/sessions/LiveApp.tsx`, `src/features/onboarding/FirstSession.tsx`, `src/features/onboarding/emptyStates.ts`
- Test: `src/lib/keymap.test.ts`, `src/lib/appShortcuts.test.ts`, `src/lib/bindings.test.ts`, `src/features/shortcuts/ShortcutSheet.test.tsx`, `src/features/settings/LiveSettings.test.tsx`, `src/features/palette/Palette.test.tsx`, `src/features/palette/Palette.stories.test.tsx`, `src/features/palette/paletteList.test.ts`, `src/features/palette/runnable.test.ts`, `src/features/launcher/filter.test.ts`, `src/features/launcher/Launcher.test.tsx`, `src/features/sessions/LiveShortcuts.test.tsx`, `src/features/sessions/LivePalette.test.tsx`, plus the stories that pass `nextNumber`.

**Interfaces:**
- Produces:
  - The ids `focus-session-1` … `focus-session-9` and `go-to-session` no longer exist.
  - `FOCUS_SESSION` and the mapping that returns `goto` from it (`appShortcuts.ts:242-243`) are gone. The `SessionShortcut` type keeps its `goto` member, which Task 2 changes to `{ kind: "goto"; tab: string }`.
  - `parseSessionNumber`, `NUMBER_QUERY` and the `nextNumber` prop of `LauncherView` are gone.
  - The tray's `goto` action and `sessionActions`'s `goto` stay for Task 2.

- [ ] **Step 1: Write the failing tests**
  - `keymap.test.ts`, test `no command goes to a terminal by number`: no `COMMANDS` id matches `/^focus-session-|^go-to-session$/`, and no binding has a `code` of `Digit1` to `Digit9`. Replace the old `focus-session-N` and `go-to-session` label tests with it.
  - `appShortcuts.test.ts`: Ctrl+Shift+1, a keydown with `code: "Digit1"`, `ctrlKey` and `shiftKey` on Windows, resolves to no app shortcut.
  - `ShortcutSheet.test.tsx`: the markup does not contain `Go to session`.
  - `LiveSettings.test.tsx`: `closesSettings("focus-session-3")` is `false`.
  - `paletteList.test.ts`, test `a_recent_list_from_an_older_build_skips_removed_commands`: `emptyBox(COMMANDS, ["focus-session-3", "go-to-session", "close"], []).recent.map((c) => c.id)` equals `["close"]`.
  - `Palette.test.tsx`:
    - The search box's placeholder is `Type a command or a terminal's name`.
    - With the query `6`, no option's label starts with `Go to session 6`.
    - No terminal option carries a `Ctrl+Shift+` key hint.
    - The text `No session 12. Open sessions:` never renders.
  - `filter.test.ts`: the launcher's filter treats `6`, `#6` and `session 6` as text. It matches folders whose name holds it and jumps to no terminal.
  - `Launcher.test.tsx`: the screen-reader paragraph does not contain `It becomes`.

- [ ] **Step 2: Run them and see them fail**

Run: `pnpm test src/lib src/features/shortcuts src/features/settings/LiveSettings.test.tsx src/features/palette src/features/launcher`
Expected: FAIL on the new assertions.

- [ ] **Step 3: Remove the number paths**
  - **Keymap:** delete `SESSION_SHORTCUTS` and the `go-to-session` command, along with the comment above `SESSION_SHORTCUTS` about Ctrl+Shift+1–9.
  - **App shortcuts and Settings:** delete `FOCUS_SESSION` and the mapping that returns `goto` from it (`appShortcuts.ts:242-243`), and the `focus-session` alternative in `closesSettings`. Keep the `goto` member of `SessionShortcut` and its `case` in `sessionActions.ts` (~227): the tray still sends it until Task 2.
  - **Cheat sheet:** delete the "Go to session 1 to 9" row and its special case.
  - **Palette:**
    - Delete the numeric-query path (`parseSessionNumber` use, `wanted`, and the `No session ${wanted}. Open sessions: …` copy), the Ctrl+Shift+digit hint on terminal rows, and the `palette__num` column with its CSS rule and `Palette.css.test.ts` assertion.
    - The placeholder becomes `Type a command or a terminal's name`. Terminal rows are still listed and filtered by their text.
  - **runnable.ts:** delete the `go-to-session` entry and the "any session" case that needs it.
  - **Launcher:**
    - Delete `NUMBER_QUERY`, `parseSessionNumber` and the screen-reader `. It becomes session ${nextNumber}` with the `nextNumber` prop.
    - Its feeds in `LiveLauncher.tsx`, `LiveApp.tsx` (`nextSessionNumber(...)` at the launcher) and `FirstSession.tsx` go. `nextSessionNumber` itself goes in Task 2.
  - **Empty state:** in `emptyStates.ts`, `Try fewer letters, or a session number.` becomes `Try fewer letters.`

- [ ] **Step 4: Run the frontend checks**

Run: `pnpm test` then `pnpm build`
Expected: both pass.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat(keys): no key, palette search or launcher query goes to a terminal by number."
```

### Task 2: numbers leave the screen, and the navigation keys on the tab id

**Files:**
- Delete: `src/components/SessionNumber/SessionNumber.tsx`, `src/components/SessionNumber/SessionNumber.css`, `src/features/sidebar/sessionNumber.ts`, `src/features/sidebar/sessionNumber.test.ts`
- Modify:
  - Rows and labels: `src/features/sidebar/types.ts`, `labels.ts`, `SessionRow.tsx`; `src/features/sessions/toRow.ts`.
  - Window: `src/features/window/Rail.tsx`, `Rail.css` (`.rail__number`, 126 and 132), `SessionHeader.tsx`, `AllSessionsMenu.tsx`, `AllSessionsMenu.css` (57), `Sidebar.tsx` (290, and the refocus at 299–300), `announce.ts`, `NavCursor.tsx`; `src/features/sidebar/SessionRow.css` (109).
  - Navigation: `src/lib/navigation/types.ts`, `machine.ts`, `fixtures.ts`; `src/features/sessions/navLayout.ts`, `useNavigation.ts`, `LiveApp.tsx`, `sessionActions.ts` (the goto, and the restart, reopen and pause bars at 52, 59 and 76); `src/lib/appShortcuts.ts` (the `goto` member); `src/features/notifications/notificationLink.ts`; `src/features/pane/paneCommands.ts` (the example in the comment at 181).
  - Everything else that showed a number: `src/features/composer/StatusBar.tsx`, `src/features/approval/ApproveCard.tsx`, `src/features/approval/types.ts`, `src/features/departures/DeparturesBoard.tsx`, `src/features/departures/board.ts`, `src/features/palette/Palette.tsx`, `src/features/palette/sessions.ts`, `src/features/approval/fromCore.ts` (36, 64), `src/features/tray/trayItems.ts`, `useTray.ts` (29, 44, and the id match at 60), `TrayMenu.tsx`, `src/features/notifications/banner.ts`, `src/features/peek/usePeek.ts`, `src/features/doctor/doctorRestart.ts`, `LiveDoctor.tsx` (249), `report.ts` (`listNumbers`), `src/design/NavDemo.tsx`.
- Modify stories and fixtures that pass `number`: `src/features/sidebar/fixtures.ts`, `src/features/palette/fixtures.ts`, `src/features/approval/fixtures.ts`, `src/features/notifications/Notifications.stories.tsx`, `src/features/tray/Tray.stories.tsx`, `src/features/palette/Palette.stories.tsx`, `src/features/window/MainWindow.stories.tsx`, `src/components/AgentMark/ManualCli.stories.tsx` (its title too), and every other literal that sets `RowModel.number` (Step 3's sweep).
- Modify: `src/features/window/manualCli.test.tsx` (it imports `nextSessionNumber`), `scripts/build-review.mjs` (33 lists `SessionNumber.css`), `scripts/a11y-matrix.mjs` (23, the section `manual-clis-and-session-numbers`).
- Create: `src/features/sessions/numbersGone.test.ts`
- Test: the tests of every file above.

**Interfaces:**
- Consumes: Task 1 (no number shortcuts left).
- Produces:
  - `terminalLabel(r: { project: string; name: string }): string` in `src/features/sidebar/labels.ts`.
  - `RowModel` has no `number`.
  - The navigation target is `{ kind: "row"; id: string }`, with the tab id. `data-nav-row` holds the tab id, and `data-number` is gone.
  - `TraySession` is `{ id: string; project: string; name: string; status; phrase; agent }`.
  - `TrayAction` and `SessionShortcut` both have `{ kind: "goto"; tab: string }`, and `tabIdOfNumber` is gone.
  - `NavContext.names` is `Record<string, string>`, keyed by tab id, holding `terminalLabel(row)`.
  - `listNames(names: readonly string[]): string` in `src/features/doctor/report.ts` replaces `listNumbers`. It keeps the input order: `a`, `a and b`, `a, b and c`.
  - The approval `session` is `{ name: string; agent: AgentId; project?: string }`.

- [ ] **Step 1: Write the failing tests**
  - `labels.test.ts`:
    - `terminalLabel({ project: "billing", name: "claude" })` is `billing · claude`.
    - `terminalLabel({ project: "", name: "claude" })` is `claude`.
    - `rowLabel(row)` starts with `${row.name}, `, and `rowTitle(row)`'s first line is the name.
  - `numbersGone.test.ts`:
    - Read every `.ts`/`.tsx`/`.css` file under `src/` except `src/generated/`, `*.test.*` and this file.
    - Assert that none contains `SessionNumber`, `sessionWord`, `SESSION_NUMBER_HINT`, `nextSessionNumber`, `session-number`, `data-number`, `rail__number` or `palette__num`, and that none matches `/session \$\{/i`.
    - Model it on `src/design/accentChoiceGone.test.ts`.
  - `machine.test.ts`, test `two_rows_with_one_name_are_two_targets`: two row targets `{ kind: "row", id: "tab-a" }` and `{ kind: "row", id: "tab-b" }`, with one name. Moving down from the first selects the second.
  - `navLayout` test: `data-nav-row="t3f9a0c41b2de"` becomes `{ kind: "row", id: "t3f9a0c41b2de" }`, and an empty `data-nav-row` becomes no target.
  - `banner.test.ts`, to match the core in Task 4:
    - meta `my-app · api-server`, and `api-server` with no project;
    - for a batch of two: title `2 terminals need you`, body `my-app · api-server and web · web`, meta `2 terminals`;
    - the done and stopped batches read `{k} terminals are done` and `{k} terminals stopped`.
  - `trayItems.test.ts`: a waiting terminal is `{ id: "session-tab-a", label: "my-app · api: Wants to run", kind: "session" }`, with no `number` key.
  - `useTray.test.ts`: the tray item id `session-tab-a` maps to `{ kind: "goto", tab: "tab-a" }`.
  - `board.test.ts`: rows that aren't gate calls keep the order they came in (the sidebar's), and gate calls tie-break by that order.
  - `usePeek.test.ts`: `peekName({ project: "billing", name: "api" })` is `Peek: billing · api`.
  - `doctorRestart.test.ts`:
    - one left: `billing · claude wasn't restarted. Restart it from its tab.`;
    - two left: `billing · claude and web · codex weren't restarted. Restart them from their tabs.`;
    - three left: `a, b and c weren't restarted. Restart them from their tabs.`
  - Component tests (`SessionRow`, `Rail`, `SessionHeader`, `StatusBar`, `AllSessionsMenu`, `ApproveCard`, `DeparturesBoard`, `TrayMenu`, `Palette`):
    - the markup has no `session-number` class;
    - the header's group `aria-label`, the status bar's `aria-label`, the all-terminals menu item's `aria-label` and the approve card's `aria-label` lead with `terminalLabel(...)`. The card's reads `${terminalLabel(session)}: ${headline}`, or the name alone when the session has no project.
    - a palette terminal option's `aria-label` starts with `Go to ${terminalLabel(s)}, `.
  - `announce.test.ts`:
    - `switchAnnouncement` reads `billing · api` for a row that needs nothing, `billing · api is waiting for your approval` for a permission, and `api is waiting for your approval` when the project is empty.
    - `statusSentence` reads `billing · api: Working. <phrase>`.
  - `machine.test.ts` (the "Selected …" text is made in `machine.ts`, not `announce.ts`):
    - selecting a row announces `Selected billing · api`, and the cursor reads `Cursor on billing · api`;
    - with no name for the id, they read `Selected terminal` and `Cursor on terminal`.
  - `useNavigation` test: `labelOf` for a row is `billing · api`, or `Terminal` with no name.
  - `sessionActions.test.ts` (24, 36, 112 and the other bar texts):
    - restart: `Restart billing · claude in Plan? It ends and runs …`;
    - reopen: `Reopen billing · claude as administrator? …`;
    - pause: `Pause billing · claude? It freezes the program and everything it started. Network connections can time out while it's paused.`
  - `fromCore.test.ts`: both the approval request's and the lane entry's `session` carry `project: row.project`, and have no `number`.
  - `report.test.ts`: `listNames(["b", "a", "c"])` is `b, a and c`.

- [ ] **Step 2: Run them and see them fail**

Run: `pnpm test src/features src/lib src/components src/design`
Expected: FAIL on the new assertions.

- [ ] **Step 3: Implement**
  - **Navigation:**
    - Key every number-keyed path on the tab id: `Target`, `panes`, the `bring-into-view` effect, `navNames`, `tabIdOfNumber` (delete it), `linkFocusSelector`, the tray's and the shortcut's `goto`, and the sidebar's refocus after its row menu closes (`[data-nav-row="${menu.id}"]`).
    - `navLayout` accepts any non-empty `data-nav-row` as the id.
    - `machine.ts`'s `sessionText` and `useNavigation.ts`'s `labelOf` read the label from `names`, with the fallbacks in Step 1.
  - **Rows and names:**
    - Drop every badge, `data-number` and `SESSION_NUMBER_HINT` title.
    - The rail tile keeps its stripe, mark and glyph.
    - The Departures board's name cell drops `Session ` and the badge. `boardRowLabel` drops its `Session ${r.number}, ` prefix, and `board.ts` sorts gate calls by wait, then input order, and the rest by input order.
  - **Tray:**
    - `useTray` builds `TraySession` from the row (`id`, `project`) or, without a row, from the entry (`e.tab`, the last folder of `e.session.project`, or `""`). Its item-id match becomes `/^session-(.+)$/`.
    - `trayItems` labels a waiting terminal `${terminalLabel(w)}: ${w.phrase}`, with id `session-${w.id}`.
    - `TrayMenu` drops the badge.
  - **Banner:** `banner.ts` reads as in Step 1, and its doc comment is updated to drop "names each session number first".
  - **Other names:**
    - `announce.ts`: `switchAnnouncement` and `statusSentence` as in Step 1. A need is prefixed with `${row.project} · ` when the project isn't empty.
    - The bars in `sessionActions.ts` name the terminal with `terminalLabel({ project: lastSegment(info.project), name: info.name })`, using `lastSegment` from `toRow.ts`.
    - `fromCore.ts` sets `project: row.project` where it set `number`, and leaves it out when there is no row.
    - `LiveDoctor.tsx` passes the labels of the terminals it didn't restart to `doctorRestart`, which joins them with `listNames`.
    - `NavDemo.tsx`: drop the `Session ${n}, ` prefixes.
    - Update the example strings in comments: `NavCursor.tsx:34`, `announce.ts:63`, `paneCommands.ts:181`, `useNavigation.ts:65`.
  - **Stories and scripts:**
    - `ManualCli.stories.tsx`'s title becomes `Manual CLIs`, so its a11y-matrix section becomes `manual-clis`. `scripts/a11y-matrix.mjs` follows, and so does any other hit of `rg -n "manual-clis-and-session-numbers"`.
    - `scripts/build-review.mjs` drops `SessionNumber.css`.
    - `manualCli.test.tsx` drops its `nextSessionNumber` tests.
    - The `.session-number` rules in `SessionRow.css`, `AllSessionsMenu.css` and `Rail.css` go.
  - **Sweep:** after `RowModel.number` goes, `pnpm build` lists each literal that still sets it, such as `rowFlash.test.ts:11`, `src/features/scripts/fixtures.ts:20`, `toRow.test.ts` and `src/lib/navigation/keys.test.ts`. Drop the field from each. `SessionInfo` literals keep `number` until Task 6.
  - **The sidebar's row menu:** its label `` `Session ${…name}` `` (`Sidebar.tsx`, ~290) becomes `` `Terminal ${…name}` ``.
  - **toRow:** stop copying `info.number`. `SessionInfo.number` stays in the generated type until Task 6, and nothing reads it.

- [ ] **Step 4: Run the frontend checks**

Run: `pnpm test` then `pnpm build`
Expected: both pass.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat(sessions): no number is drawn or read; a terminal away from its row is named by project and name, and the navigation keys on the tab id."
```

### Task 3: the screen says "terminal"

**Files:**
- Modify: every file in part A of `docs/superpowers/plans/2026-11-28-phase-6aa-1-copy.md`.
- Create: `src/lib/terminalWording.test.ts`
- Test: the tests that assert a changed string; find them with `rg -l "<old string>" src`.

**Interfaces:**
- Consumes: Task 2.
- Produces: `Command["group"]` is `"Terminal" | "Window" | "App"`.

- [ ] **Step 1: Write the failing test** `src/lib/terminalWording.test.ts`
  - No `COMMANDS` label matches `/\bsessions?\b/i` except the `resume` command's (`Resume this session`).
  - No command has the group `"Session"`.
  - `ShortcutSheet`'s markup has exactly one heading `Terminal`.

- [ ] **Step 2: Run it and see it fail**

Run: `pnpm test src/lib/terminalWording.test.ts`
Expected: FAIL.

- [ ] **Step 3: Apply part A of the copy table**, and update each test that asserted an old string.

- [ ] **Step 4: Look for what the table missed.**

Run: `rg -n -i "sessions?\b" src --glob "!*.test.*" --glob "!*.stories.*" --glob "!src/generated/**" --glob "!**/fixtures.ts"`

Each hit is one of these:
- a code identifier, a comment, a CSS class or an event name;
- a "stays" string in the copy table;
- a new find. Apply the rule to it and list it in the task report.

- [ ] **Step 5: Run the frontend checks**

Run: `pnpm test` then `pnpm build`
Expected: both pass.

- [ ] **Step 6: Commit**

```bash
git add -A src
git commit -m "feat(copy): what runs in a tab is a terminal on screen; session stays for an agent's conversation."
```

### Task 4: the core's and the tray's words name terminals without numbers

**Files:**
- Modify: `crates/core/src/notify/copy.rs`, `crates/core/src/notify/engine.rs` (tests), `crates/core/src/notify/mod.rs` (tests), `crates/core/src/server/doctor.rs`, `src-tauri/src/tray.rs`, `src-tauri/src/notify.rs` (tests), `src-tauri/src/toast.rs` (tests), `crates/cli/src/open.rs` (the matched core text only)
- Modify: every file in part B of the copy table, and the tests that assert its strings, including `crates/core/tests/*.rs` and `crates/host/tests/elevated_session.rs`.

**Interfaces:**
- Produces:
  - `Who { tab_id: String, name: String, project: String }`, with no `number`.
  - `Who::label(&self) -> String` is `"{project} · {name}"`, or `name` when the project is empty, and `Who::meta` returns it.
  - `TrayItemDto` has no `number`.

- [ ] **Step 1: Write the failing tests**
  - `copy.rs`:
    - `a_permission_request_shows_the_row_phrase`: meta `my-app · api-server`.
    - New `a_terminal_with_no_project_is_named_alone`: meta `api-server`.
    - The batch test: titles `2 terminals need you`, `2 terminals are done` and `2 terminals stopped`, body `my-app · api-server and web · web`, meta `2 terminals`.
    - Three items read `a · x, b · y and c · z`.
  - `engine.rs`: its `"Session 1 · app"` assertions read `app · <name>`.
  - `tray.rs`:
    - `a_session_item_reads_name_and_phrase` (renamed from `a_session_item_reads_number_name_and_phrase`): `menu_label` returns the item's label unchanged.
    - New `an_item_from_an_older_webview_with_a_number_still_parses`: the JSON `{ "id": "session-3", "label": "api: Wants to run", "kind": "session", "number": 3 }` parses, and its label has no `3 · `.
  - `doctor.rs`: the restart list's test asserts the tabs in the snapshot's order, not by number. The function is at ~1590; find its test.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p marshell-core -j 4 notify` and `cargo test -p marshell-app -j 4 tray`
Expected: FAIL.

- [ ] **Step 3: Implement**
  - `Who` and `batch` read as above, and `Who::of` drops `number`.
  - `menu_label` no longer prefixes a number, and `TrayItemDto.number` goes. There is no `deny_unknown_fields`, so an old webview's key is ignored.
  - `doctor.rs` keeps the snapshot's order (drop the sort by number).
  - Apply part B of the copy table, and change `open.rs`'s matched text with its `Fake` reply.

- [ ] **Step 4: Run the Rust checks**

Run: `cargo test -p marshell-core -j 4`, `cargo test -p marshell-host -j 4`, `cargo test -p marshell-app -j 4`, `cargo test -p marshell-cli -j 4`
Expected: all pass.

- [ ] **Step 5: Commit** (after `cargo fmt --all`)

```bash
git add -A crates src-tauri
git commit -m "feat(core): notifications and the tray name a terminal by project and name, and the core's messages say terminal."
```

### Task 5: terminal ids in the core, beside the numbers

**Files:**
- Modify: `crates/protocol/src/api.rs` (`SessionInfo`, `CreateSessionResponse`, `SavedTab`, `LayoutDoc`)
- Modify: `crates/core/src/registry.rs`, `crates/core/src/server/sessions.rs`, `crates/core/src/server/shell_panel.rs`, `crates/core/src/server/layout.rs`, `crates/core/src/server/mod.rs` (the `seed_ended` call), `crates/core/src/server/opens.rs`, `crates/core/src/outside.rs`
- Modify: `crates/core/tests/history_api.rs` (~2215) and `crates/core/tests/tasks.rs` (~1386), which call `seed_ended`.
- Modify: every Rust and TS test or fixture that builds a `SessionInfo`, `SavedTab` or `CreateSessionResponse`: it gains `terminal_id` (`"tN"` beside `number: N`).
- Regenerate: `src/generated/SessionInfo.ts`, `CreateSessionResponse.ts`, `SavedTab.ts`
- Test: `crates/core/src/registry.rs`, `crates/core/tests/layout_restore.rs`, `crates/core/tests/pty_ws.rs`, `crates/core/tests/outside.rs`, `crates/core/src/outside.rs`, `crates/core/src/server/opens.rs`

**Interfaces:**
- Consumes: Task 4 (the core's words no longer use the number).
- Produces:
  - **Protocol:**
    - `SessionInfo.terminal_id: String`, doc "The terminal's id, such as t7: counts up per Marshell home, never reused, kept across restarts."
    - `CreateSessionResponse.terminal_id: String`.
    - `SavedTab.terminal_id: String` with `#[serde(default)]`.
    - `LayoutDoc.next_terminal: u32` with `#[serde(default)]` and `#[ts(skip)]`.
    - `number` stays on all three until Task 6.
  - **Registry:**
    - `Registry::take_terminal_id(&self) -> String`.
    - `Registry::reserve(&self, tab_id: &str, relaunch: Relaunch) -> (String, u64)` and `reserve_script(...) -> (String, u64)`, which return the terminal id.
    - `Registry::adopt(&self, tab_id: &str, terminal_id: String, relaunch: Relaunch, generation: u64)`.
    - `Registry::seed_ended(&self, tabs: &[SavedTab], next_terminal: u32, relaunch_of: impl Fn(&SavedTab) -> Relaunch)`.
    - `Registry::next_terminal(&self) -> u32`.
  - **Spawning:** `spawn_into_map(..., terminal_id: &str, ...)` replaces `number: u32`.

- [ ] **Step 1: Write the failing tests**
  - In `registry.rs`:
    - `ids_count_up_and_are_never_reused`: reserve A, B and C (`t1`, `t2`, `t3`). Close C, then reserve D: it is `t4`.
    - `reorder_never_changes_ids`: like `reorder_never_renumbers`, for ids.
    - `duplicate_names_stay_as_given` (from `duplicate_name_gets_number_suffix`): two reserves named `pwsh` both show `pwsh`.
    - `a_restart_keeps_ids_and_the_counter`: `seed_ended` with tabs `t1` and `t4` and `next_terminal` 6 keeps `t1` and `t4`, and the next reserve is `t6`.
    - `the_counter_runs_past_the_highest_saved_id`: tabs `t9`, `next_terminal` 3; the next reserve is `t10`.
    - `an_old_layout_gets_ids_in_saved_order`: three tabs with `terminal_id` `""` and `next_terminal` 0 become `t1`, `t2`, `t3` in saved order, and the next reserve is `t4`.
    - `a_saved_id_that_is_taken_or_malformed_gets_a_fresh_one`: tabs `t2`, `t2`, `x`, `t0`, `t02`, `t4294967295`. The first keeps `t2`. The others get `t3`, `t4`, `t5`, `t6`, `t7` in saved order. A valid id is `t` and a decimal from 1 with no leading zero, below `u32::MAX`, so the counter can always move past it.
    - `take_terminal_id_moves_the_counter_and_wakes_the_saver`: after `take_terminal_id()` returns `t1`, a reserve gets `t2`, and a subscriber sees a change.
    - `saved_tabs_carry_their_ids`, and `next_terminal()` returns the next value.
    - `clear_keeps_the_counter`: after two reserves and `clear()`, the next reserve is `t3`.
  - In `crates/core/tests/layout_restore.rs`, next to `restart_restores_tabs_as_ended`. `save_tabs` is private and its saver waits 250 ms, so these go through the API and poll `layout.json`, as `wait_for_saved` does:
    - `layout_writes_keep_the_counter`: after two creates, a PUT of the view and the launcher's remember both leave `next_terminal` at 3 in the file.
    - `a_closed_newest_id_is_not_given_again_after_a_restart`: create two terminals (`t1`, `t2`), close `t2`, and poll until the file's `next_terminal` is 3. Then start a fresh state on the same home and create one: its `terminal_id` is `t3`.
  - In `crates/core/tests/pty_ws.rs`, `session_env_has_terminal_id` (from `session_env_has_number`): the child's environment has `MARSHELL_TERMINAL_ID` equal to the row's `terminal_id`, and no `MARSHELL_SESSION_NUMBER`.
  - For the shell panel, in its tests in `server/shell_panel.rs` or `crates/core/tests/`: `the_panel_shell_keeps_its_id_when_adopted`. The panel's `MARSHELL_TERMINAL_ID` equals the adopted row's `terminal_id`.
  - In `outside.rs`, the unit tests and `tests/outside.rs`: the stripped markers are `MARSHELL_TAB_ID`, `MARSHELL_TERMINAL_ID`, `MARSHELL_AGENT` and `MARSHELL_SESSION_NUMBER`.
  - In `opens.rs`: the refusals read `Terminal t1 has closed.` and `Terminal t1 already has 32 tabs in its side pane.`

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p marshell-core -j 4 registry` and `cargo test -p marshell-core -j 4 layout`
Expected: FAIL to compile, or FAIL on the new tests.

- [ ] **Step 3: Implement**
  - **The counter.** `State` holds `next_terminal: u32`, starting at 1. Every new row takes `format!("t{n}")` and moves the counter on. `take_terminal_id` does the same and sends a snapshot, so the layout saver writes the counter.
  - **`seed_ended`:**
    - First, the counter becomes the largest of its current value, `next_terminal`, and one more than the highest valid saved id.
    - Then each tab in saved order keeps a valid id that no earlier tab took, and otherwise gets the next fresh id. Its number is chosen as today.
  - **Names.** `adopt_script` stops appending anything to a taken name.
  - **`clear`** empties the lists and leaves the counter as it is.
  - **The shell panel.** It calls `take_terminal_id()` before it spawns its shell. It keeps the id in `PanelShell`, puts it in the panel's row beside `number: 0`, and passes it to `adopt`.
  - **Saving.** `save_tabs` sets `next.next_terminal = registry.next_terminal()`, and startup passes `layout.next_terminal` to `seed_ended`.
  - **The environment.** It sets `(brand::env_var("TERMINAL_ID"), terminal_id)` in place of `SESSION_NUMBER`.
  - **Stripping.** `marshell_markers` lists `TAB_ID`, `TERMINAL_ID`, `AGENT` and `SESSION_NUMBER`, with a comment that the last is what builds before slice 6aa-1 set.
  - **`opens.rs`.** Its refusals name `terminal_id`.
  - **Gaps that are expected**, to write in the doc comment on the counter:
    - Each time the shell panel opens, it takes an id, even if its shell never moves to the list, so ids can skip.
    - A reopened terminal is a new tab, so it gets a new id.
    - The panel's shell isn't in `marshell list` until it moves there.
  - **Creating a terminal.** The create handler returns the new id in `CreateSessionResponse.terminal_id`. Until Task 6 removes it, `number` is read from the new row.

- [ ] **Step 4: Regenerate and run every check**

Run, in order:
1. `cargo test -p marshell-protocol -j 4`
2. `cargo test --workspace -j 4`
3. `pnpm check:generated`, after staging `src/generated`
4. `pnpm test`
5. `pnpm build`

Expected: all pass, and the generated `LayoutDoc` has no `next_terminal`.

- [ ] **Step 5: Commit** (after `cargo fmt --all`)

```bash
git add -A crates src src-tauri
git commit -m "feat(core): every terminal gets an id such as t7 that counts up, is never reused and survives a restart."
```

### Task 6: the CLI takes ids, and the numbers go

**Files:**
- Modify: `crates/cli/src/args.rs`, `client.rs`, `list.rs`, `send.rs`, `focus.rs`, `new.rs`, `open.rs`, `start.rs`, `main.rs` (it destructures the `session` fields)
- Modify: `crates/core/src/layout.rs` (`FileDoc::serialize`), `crates/core/tests/layout_restore.rs` (~133 asserts `number`)
- Modify: `crates/protocol/src/api.rs` (the `number` fields of `SessionInfo`, `CreateSessionResponse` and `SavedTab` go), `crates/core/src/registry.rs` (`lowest_free_number` and every number path go), `crates/core/src/server/shell_panel.rs` (`number: 0` goes), `crates/core/src/history/recorder.rs` (its test's `reserve` call)
- Modify: every Rust and TS test or fixture that still sets `number` on those three types.
- Regenerate: `src/generated/SessionInfo.ts`, `CreateSessionResponse.ts`, `SavedTab.ts`
- Test: `crates/cli/tests/cli.rs`, the unit tests in those files, `crates/core/src/registry.rs`, `crates/core/src/layout.rs`

**Interfaces:**
- Consumes: Task 5 (`SessionInfo.terminal_id`, `CreateSessionResponse.terminal_id`).
- Produces:
  - In `client.rs`:
    - `pub fn number_form(arg: &str) -> bool` is true for digits, or `#` and digits.
    - `pub fn resolve<'a>(snapshot: &'a SessionsSnapshot, arg: &str) -> Option<&'a SessionInfo>` returns the first open row whose `terminal_id` or `tab_id` equals the argument, ASCII case-insensitively.
  - `send`, `focus` and `open` check `number_form` before they connect. So the pointer comes even when the app is closed, and `focus 3 --start` starts nothing.
  - `pub const NUMBERS_GONE: &str` (below).
  - `Refusal::message(&self, terminal_id: &str) -> String`.
  - The args fields are `Send { terminal, text }`, `Focus { terminal, start }` and `Open { targets, terminal: Option<String> }`.
  - No protocol type has a session `number`.

**The CLI's copy (exact):**
- **Pointers and lookups.**
  - `NUMBERS_GONE`: ``Terminals have ids now, such as t7. `marshell list` prints them.``
  - Not found: `No terminal {arg}.`
- **Refusals.**
  - `Terminal {id} has ended, so it can't take text.`
  - `Terminal {id} wasn't ready within 30 s, so its prompt line wasn't filled.`
  - `Terminal {id} is waiting for an answer, so it can't take text.`
- **Success.**
  - `Filled terminal {id}'s prompt line.`
  - `Started terminal {id}.`
- **`open` and its flag.**
  - Outside a terminal: `Outside a Marshell terminal, open needs --terminal <terminal>.`
  - The old flag: `--session is now --terminal.` (exit 2)
- **Usage errors.**
  - `send takes a terminal and the text to put on its prompt line.`
  - `focus takes a terminal id or a tab id.`
  - `focus takes one terminal.`
  - `--terminal can be given once.`
  - `--terminal takes a value.`
- **`list`.**
  - With nothing open: `No terminals.`
  - The table header is `ID  PROJECT  NAME  AGENT  STATE  MODE`, then `ADMIN` when any terminal is elevated, then `CWD`.
  - PROJECT is the last folder of `project`, or `-` when that is empty, like the table's other empty cells.
  - The JSON rows are `{ id, tab_id, name, agent, state, cwd, project, mode, elevated }`, in that order. `id` takes the place of `number`.
- **USAGE.**
  - `list` reads `List the open terminals of the running app: id, project, name, agent, state, mode and folder.` / `--json prints them as JSON, with each terminal's id and tab id.`
  - `send <terminal> <text>` reads `Put <text> on the terminal's prompt line. Marshell never presses Enter. <terminal> is` / `a terminal id such as t7, or a tab id.`
  - `new` reads `Start a terminal.` (the rest unchanged).
  - `focus <terminal> [--start]` reads `Bring Marshell's window to the terminal.` (the rest unchanged).
  - `open [--terminal <terminal>] <address or file>...` reads `Open a web address or a file as a tab in a terminal's side pane, in the background.` (the next sentence unchanged), then `--terminal defaults to the terminal the command runs in.`
  - Keep the 22-column indent and wrap lines under 110 characters.

- [ ] **Step 1: Write the failing tests**
  - `client.rs`:
    - `resolve_matches_open_ids_and_tab_ids`: `t1` and `T1` are the row `t1`, and `tab-a` is its row by tab id. A closed row's id, `+1` and `""` are `None`.
    - `number_forms`: `6`, `#6` and `12` are number forms; `t6`, `+1`, `#`, `""` and `6a` aren't.
  - `cli.rs`:
    - **`a_bare_number_is_pointed_at_ids`:** `send 1 hi`, `focus 1` and `open --terminal 1 x.md` each print `NUMBERS_GONE` on stderr and exit 1. So do `send 3 hi` and `focus 3 --start` with no app running, which start nothing.
    - **`the_session_flag_points_at_terminal`:** `open --session t1 x.md` prints `--session is now --terminal.` and exits 2.
    - **`send_by_id_reaches_one_of_two_same_named`:** two shells named `pwsh`; `send <second id> hi` fills only the second.
    - **`list_and_send_fill_without_enter`:**
      - the table header, split on whitespace, starts `ID`, `PROJECT`, `NAME`;
      - the JSON's first key is `"id": "t1"`, and the JSON has no `number`;
      - `send t1 hi` and `send <tab id> hi` both print `Filled terminal t1's prompt line.`
    - **The new, refusal and focus tests** read the copy above: `new_creates_then_fills`, `send_refuses_controls_and_ended`, and `focus_posts_the_tab` with `focus t9` printing `No terminal t9.`
  - `start.rs`, `the_app_is_started_without_the_terminals_session`: it removes the four names `MARSHELL_TAB_ID`, `MARSHELL_TERMINAL_ID`, `MARSHELL_AGENT` and `MARSHELL_SESSION_NUMBER` (`removed.len() == 4`).
  - `crates/core/src/layout.rs`, `a_layout_from_before_ids_still_reads`: JSON whose tabs have `"number": 3` and no `terminal_id`, with no `next_terminal`, parses. The tabs have `terminal_id` `""` and `next_terminal` is 0.
  - `crates/core/src/layout.rs`, `the_file_still_gives_older_builds_a_number`: a save writes each tab's `number` as its position from 1. A build from before ids requires that field, and without it reads the whole file as broken and loses every tab.
  - `registry.rs`: the number tests go with the field (`numbers_are_lowest_free`, `registry_marks_exit_and_keeps_number` keeps its exit half as `registry_marks_exit_and_keeps_its_id`, `seed_ended_renumbers_a_duplicate_number`, the number half of `seed_ended_keeps_order_ids_and_numbers`).

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p marshell-cli -j 4` and `cargo test -p marshell-core -j 4 layout`
Expected: FAIL.

- [ ] **Step 3: Implement**
  - Move the CLI to `terminal_id` with the copy above. `open.rs` still reads its own tab from `MARSHELL_TAB_ID`.
  - Then remove `number` from the three protocol types, from the registry (`lowest_free_number`, its uses, and its doc comments) and from the panel's row.
  - `FileDoc::serialize` writes `number` (position + 1) on each saved tab, as it already writes the split view fields. Its doc comment says why. This build ignores that field when it reads.

- [ ] **Step 4: Regenerate and run every check**

Run, in order:
1. `cargo test -p marshell-protocol -j 4`
2. `cargo test --workspace -j 4`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `pnpm check:generated`, after staging `src/generated`
5. `pnpm test`
6. `pnpm build`

Expected: all pass, and the generated `SessionInfo` has `terminal_id: string` and no `number`.

- [ ] **Step 5: Commit** (after `cargo fmt --all`)

```bash
git add -A crates src
git commit -m "feat(cli): marshell takes terminal ids and points a bare number at them, and session numbers are gone from the core."
```

### Task 7: the skill says how to find a terminal

**Files:**
- Modify: `crates/core/src/agents/skill/SKILL.md`, `crates/core/src/agents/skill.rs`

**The new `SKILL.md`:**
- The front matter's `description` becomes: `Use inside a Marshell terminal (MARSHELL_TAB_ID is set) when the person asks to list, start, message or bring forward Marshell terminals, to hand work to another agent CLI in a new Marshell terminal, or to show a web page or a file in the side pane.`
- Then, in the body:
  - Every "session" that names a terminal becomes "terminal".
  - `Your own session's number is `MARSHELL_SESSION_NUMBER`.` becomes ``Your own terminal's id is `MARSHELL_TERMINAL_ID`.``
  - The `list --json` line lists `id`, `tab_id`, `name`, … and says ```project` (the project's folder)``.
  - `<session> is a number or a tab id.` becomes `` `<terminal>` is an id such as `t7`, or a tab id.``
  - `--session <session>` becomes `--terminal <terminal>`.
  - `When send, new or open refuses, tell the person what it said and don't retry:` becomes `When send, new or open refuses, tell the person what it said, naming the terminal by its project and name, and don't retry:`.
  - The four quoted refusal phrases stay word for word.
  - This section goes after the opening paragraphs:

```markdown
## Finding a terminal

The person names a terminal by its project and its name, as Marshell shows it: "billing · claude" is the terminal named `claude` whose `project` folder is named `billing`. Find it in `marshell list --json` and use its `id`, such as `t7`, in the commands below. Two terminals can have the same name: when more than one matches, or none does, ask the person which one they mean. Tell the person about a terminal by its project and name, not by its id, which Marshell doesn't show.
```

- [ ] **Step 1: Pin the shipped alpha.2 text first.** Add `const ALPHA_2: &str = "1f850159a12b53c14a79b9c819e09d46c70abadda5b0465676409acae5b5d40e";` beside `ALPHA_1`, and the test `the_shipped_alpha_2_text_is_a_previous_one`, which asserts `PREVIOUS.contains(&ALPHA_2)`.
  - Add `ALPHA_2` to `PREVIOUS` under a `// v0.1.0-alpha.2` comment.
  - Run `cargo test -p marshell-core -j 4 skill`. Expected: the new test passes, and `the_shipped_alpha_1_text_is_a_previous_one` fails on its last assertion, because today's text is alpha.2's and is now in `PREVIOUS`. That failure is what the new text fixes.
- [ ] **Step 2: Write the failing test** `the_skill_names_ids_not_numbers`.
  - `SKILL_TEXT` contains `MARSHELL_TERMINAL_ID`, `## Finding a terminal` and `--terminal`.
  - It contains neither `MARSHELL_SESSION_NUMBER` nor `--session`.
  - Extend `the_shipped_alpha_2_text_is_a_previous_one` to assert that the current text's hash is neither `ALPHA_1` nor `ALPHA_2`.
- [ ] **Step 3: Write the new text** as above.
- [ ] **Step 4: Run the checks**

Run: `cargo test -p marshell-core -j 4 skill`
Expected: all pass, `skill_text_fits_every_cli` included.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/agents
git commit -m "feat(skill): the marshell skill tells agents to find a terminal by project and name and use its id, and alpha.2's text counts as an older copy."
```

### Task 8: the plan's current-state text

**Files:**
- Modify: `docs/PLAN.md`, `docs/a11y/screen-reader-script.md`, `docs/IDEAS.md` (the section name it links only)

- [ ] **Step 1: Rewrite the current-state passages**
  - **The section.** In `docs/PLAN.md`, the section `### Session numbers and manual CLIs` becomes `### Terminal ids and manual CLIs`. Its first paragraph and its four bullets become the text below. Line 142's "see Session numbers and manual CLIs" and `docs/IDEAS.md:26`'s link follow the new name.
  - **The keys.** The keys passages (~682, and the table row ~733 "Sessions 1 to 9") say: Ctrl+Shift+1 to 9 pick the Nth project from slice 6aa-3, and no key goes to a terminal by number. They keep the reason given for Ctrl+Shift.
  - **Other passages.** Run `rg -n -i "session number|MARSHELL_SESSION_NUMBER|Go to session|Ctrl\+Shift\+1" docs/PLAN.md`. Every hit outside §9's dated decision lists, and outside the phase-10 text slice 6aa already rewrote, is rewritten to match the text below. §9's decision lists are history and stay.

```markdown
**Terminal ids.** Every terminal has a short id such as `t7`, for scripts and agents. People name a terminal by its project and name, "billing · claude", and no id or number is drawn anywhere in the app (slice 6aa decisions 9 and 10, slice 6aa-1 decisions).
- **Counts up, never reused.** Ids go t1, t2, t3… per Marshell home. A closed terminal's id is never given out again, and a restored terminal keeps its id after a restart: `layout.json` keeps each tab's id and the next one (`next_terminal`). A layout from before ids gives its tabs t1, t2… in saved order. The shell panel's shell takes its id when it starts, so its variable stays right when it moves to the list.
- **Named, not numbered.** Away from its row, a terminal is named "project · name": in the status bar, the header, the approve card, the palette, the all-terminals menu, announcements, notifications and the tray. The row and the rail draw what they did, without a number. Two terminals may share a name.
- **Keys.** No key goes to a terminal by number. Ctrl+Shift+1 to 9 pick the Nth project from slice 6aa-3 (6aa decision 12), and terminals are reached with the arrow keys in the list.
- **Data contract.** The core owns the id (`SessionInfo.terminal_id`). Each terminal's process gets `MARSHELL_TERMINAL_ID` and `MARSHELL_TAB_ID`. `marshell list` prints the ids, and `marshell send`, `focus` and `open --terminal` take an id or a tab id; a bare number is refused with "Terminals have ids now, such as t7. `marshell list` prints them." The marshell skill tells agents to find a terminal by its project and name in `marshell list --json`, and to ask the person when it isn't clear which one is meant.
```

- [ ] **Step 2: Rewrite the screen-reader script's number rows** in `docs/a11y/screen-reader-script.md`.
  - **B7:**
    - Action: Focus the search field, and type `scr`.
    - Expect: The option for the scratch terminal is read first: "Go to <project> · scratch …" with its state.
  - **B8:**
    - Action: Type `6`.
    - Expect: No terminal is picked by number. Only commands and terminals whose text holds 6 are listed.
  - **F1:** the expectation reads "api-server, needs you, ..." with no session number, and the clause "with the session number first" goes.

- [ ] **Step 3: Check**

Run: `rg -n -i "session number|MARSHELL_SESSION_NUMBER" docs/PLAN.md docs/a11y`
Expected: hits only inside §9's dated decision lists.

- [ ] **Step 4: Commit**

```bash
git add docs/PLAN.md docs/a11y/screen-reader-script.md docs/IDEAS.md
git commit -m "docs(plan): terminal ids replace session numbers in the plan's current-state text."
```

### Task 9: live check

Run it with the `marshell-live-check` skill, against scratch homes. Every variable in AGENTS.md's live-check list points at a fresh scratch folder. Stop only what the check started, by PID. Findings go in `docs/research/2026-11-28-terminal-ids-live-check.md`.

- [ ] **Step 1: An old layout.**
  - Put a `layout.json` from main (`06ba7e02`) in the scratch home, with three tabs that have `number` 1, 2 and 5.
  - Start the app. Expect three Ended rows with no numbers drawn, and `marshell list` IDs `t1`, `t2`, `t3` in the saved order.
- [ ] **Step 2: The ids at work.**
  - Open two PowerShell terminals. Both are named `pwsh` (or the profile's name) with nothing appended.
  - In each, `$env:MARSHELL_TERMINAL_ID` prints its id (`t4`, `t5`), and `$env:MARSHELL_SESSION_NUMBER` prints nothing.
- [ ] **Step 3: The CLI.** From a third terminal:
  - `marshell send t5 "echo five"` fills t5's prompt line, and not t4's. Read it from the webview; don't press Enter in t5.
  - `marshell send 5 hi` prints the pointer and exits 1.
  - `marshell open --session t4 README.md` prints `--session is now --terminal.`
- [ ] **Step 4: A restart.**
  - Close the newest terminal and stop the app.
  - Start the app again. Expect the remaining rows to keep their ids, and a new terminal to take the next id, not the closed one's.
- [ ] **Step 5: The shell panel.**
  - Open the shell panel. Note its `$env:MARSHELL_TERMINAL_ID`, then move it to the list.
  - Expect `marshell list` to show it with that id.
- [ ] **Step 6: The screen, over CDP.**
  - Expect no `.session-number` or `[data-number]` in the DOM.
  - The palette with the query `2` offers no "Go to session 2", and the placeholder reads `Type a command or a terminal's name`.
  - The sidebar heading reads `Terminals`, and the shortcut sheet has one `Terminal` group with no "Go to session 1 to 9".
  - Ctrl+Shift+1, sent to the focused terminal, changes no selection.
  - Rows, the rail and the header look right with the keyboard alone and with forced colors (`browser_emulate_media`).
- [ ] **Step 7: A notification.**
  - From a terminal that isn't on screen, print `Write-Host "$([char]27)]9;Build done$([char]7)"`.
  - Expect the banner's meta (read from the webview or the toast record) to read `<project> · <name>`.
- [ ] **Step 8: Write the findings, then commit.**

```bash
git add docs/research/2026-11-28-terminal-ids-live-check.md
git commit -m "docs(research): the terminal-ids live check."
```
