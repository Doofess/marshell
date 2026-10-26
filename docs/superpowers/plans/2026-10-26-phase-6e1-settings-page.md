# Slice 6e1: the Settings page, the terminal font picker and the tools editor

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Marshell gets one Settings page in the main window. Today's six settings dialogs become its sections, the terminal font becomes a picker with three bundled fonts, and Settings → Agents gains the own-agent form and an editor for `~/.marshell/tools.json`.

**Architecture:** A new `src/features/settings/LiveSettings.tsx` renders a full-window page, as Departures does, with the phase-0 section nav (`settings__nav`, `selectedProps(…, "current")`) and one section component per area. Each dialog keeps its presentational `…View` and loses its dialog shell (`role="dialog"`, `useDialogFocus`, the Close button). Its live wrapper becomes a `…Section`. `LiveApp`'s `overlay` gets one `"settings"` value with a section id in place of the six dialog values. On the Rust side, `tools/own.rs` gains a `save` shaped exactly like `agents/own.rs::save`, behind `GET`/`PUT /v1/tools/own`.

**Tech stack:** React + TypeScript, Vitest through `renderToStaticMarkup`, Rust (axum, serde_json, `store::write`), ts-rs. No new dependency. Font files only, approved by slice 6e decision 6.

**Spec:** `docs/PLAN.md` §9 "Slice 6e decisions (2026-10-26)" 1 to 6. Also "Visual system", "Terminal font setting", 6b decision 6 (own-agent form), 6c decisions 8 and 9 (`tools.json`, presets), 5d decision 5 (Diagnostics becomes Advanced) and "Batch 3 design decisions" (pages scroll in `.page__body`, announcing, en dash for unknown values).

**Checked before planning (2026-10-26):**
- The settings page exists only in stories (`SettingsPage.tsx`, fixtures nav). The six live dialogs are `AppearanceDialog`, `NotificationsDialog`, `HistorySettingsDialog` (with `StorageSection`), `DiagnosticsDialog`, `AgentsDialog` (with `OwnAgentsSection` and `ToolsSection`) and `ProfilesDialog`. Each has a pure `…View`, opened by `LiveApp.tsx:215`'s `overlay` and rendered at `:1514`–`:1531`.
- The terminal font is a free-text field in `AppearanceView`, saved as `appearance.terminal_font.family`. An empty family means Windows Terminal's font, else JetBrains Mono (`terminalFont.ts`). No protocol change is needed: the family string carries the choice.
- `tools.json` has no write path (`tools/own.rs`: "never written"). `agents.json` has one: `agents/own.rs::save` with a SHA-256 `version`, 409 when stale, 422 with problems, and broken entries written back unchanged.
- Fonts, from the official releases, unchanged (SHA-256 verified):

  | File | Source | SHA-256 |
  |---|---|---|
  | `CascadiaMono-Regular.woff2` | microsoft/cascadia-code `v2407.24`, `CascadiaCode-2407.24.zip` → `woff2/static/` | `abbf67cceb200508c9590ac9b75b37dd2211f58ea56279f1fa6ec87265f9d713` |
  | `CascadiaMono-Bold.woff2` | same | `d84776a2893f382b96c8bfe085d438dc07f2fdd98210fa9f2d3ac62d82a7aef4` |
  | `AtkinsonHyperlegibleMono-Regular.woff2` | googlefonts/atkinson-hyperlegible-next-mono at `154d50362016cc3e873eb21d242cd0772384c8f9`, `fonts/webfonts/` (no releases or tags upstream) | `3b916de5c5247c5fa9736aad6c2673f1b0b0381ab306dcddfcee4a184a842a51` |
  | `AtkinsonHyperlegibleMono-Bold.woff2` | same | `039936edaa268366426a7f0ca8014da637b8bf8da24859afbd85a5b313e9315e` |
  | `JetBrainsMono-Regular.woff2` (bundled today) | JetBrains/JetBrainsMono `v2.304`, `fonts/webfonts/` | `a9cb1cd82332b23a47e3a1239d25d13c86d16c4220695e34b243effa999f45f2` |
  | `JetBrainsMono-Bold.woff2` (bundled today) | same | `c503cc5ec5f8b2c7666b7ecda1adf44bd45f2e6579b2eba0fc292150416588a2` |

  Internal family names, read from each file's `name` table: `Cascadia Mono`, `Atkinson Hyperlegible Mono`, `JetBrains Mono`. All three fonts are under SIL OFL 1.1. Cascadia's copyright line reserves the name "Cascadia Code" only, and nothing is modified, so the RFN doesn't apply. The downloads and licence texts are in the session scratchpad, `fonts-dl\` (`cc\`, `atk\`, `jb\`).
- The Settings shortcut was dropped (decision 4): the keymap allows no punctuation and has no free letter.
- The worktree folder is `.claude/worktrees/phase-6d`, but the branch is `phase-6e1`, cut from `origin/main` at 926cc6d.
- Baseline on 926cc6d: `pnpm test` ran 338 files and 5061 tests, all passing. `cargo test --workspace -j 4` passed 1977, failed none and ignored 5.

## Global constraints

- No new crate or package. Font files only, copied byte-identical from the sources above, with their notices in `THIRD_PARTY_NOTICES.md` (decision 6).
- **Fonts.** The `@font-face` family names are exactly `JetBrains Mono`, `Cascadia Mono` and `Atkinson Hyperlegible Mono`, weights 400 and 700, `font-display: block`, loaded from `src/assets/fonts/`. No web-hosted font (CSP).
- **The picker's labels**, exactly and in this order: "Match my terminal", "JetBrains Mono", "Cascadia Mono", "Atkinson Hyperlegible Mono", "Another font…". Each label is drawn in its own font, with the sample line `0O 1lI {} => ~/src $ git status`. "Match my terminal" is drawn in the font it resolves to.
- **The stored value stays `terminal_font.family`.** `""` is Match my terminal, and a bundled name is that font. Anything else is "Another font…" with that text. Case and surrounding spaces don't matter when matching a bundled name, and the canonical name is what gets saved.
- **The section ids and labels**, in this order: `appearance` "Appearance", `terminal` "Terminal", `notifications` "Notifications", `profiles` "Shell profiles", `history` "History", `agents` "Agents", `advanced` "Advanced". The page title is "Settings".
- **Palette.** One new command, `{ id: "settings", label: "Settings…", group: "Tools" }`, with no binding. `appearance`, `profiles`, `agents`, `diagnostics`, `notifications` and `history-settings` stay, keep their labels and open Settings at `appearance`, `profiles`, `agents`, `advanced`, `notifications` and `history`.
- **Tray.** The `notifications` item becomes `{ id: "settings", label: "Settings…" }` and opens Settings at the section shown last in this run, else `appearance`.
- **The page.** It covers the window, as Departures does. Sessions keep running and the session area stays mounted. Its body scrolls in `.page__body`, and its nav stays in place. On open, focus moves to the shown section's heading (`h2`, `tabIndex={-1}`). Esc closes the page unless something inside it already handled that Esc (`event.defaultPrevented`). A "Back to sessions" button in the page actions does the same. On close, focus returns to what had it before, else the focused session's terminal.
- **Sections.** No section draws its own dialog shell, live region or Close button. Announcements go through the existing `announce` prop.
- **Agents.** The `startWith` flow (the row's "Status limited · set up" link) opens Settings at `agents` with that CLI's card, and "Notifications" inside it moves to the `notifications` section.
- **`tools.json`.**
  - Saves are atomic, through `store::write`.
  - 409 with "tools.json changed since you opened it." when the version is stale. 422 with the problems list.
  - A broken entry sent back as it was shown is written back as it was in the file.
  - An own id that equals a shipped profile's id is refused, with the reason "<id> is a shipped tool's id".
- Every UI change works with the keyboard alone and in forced colours. The guard tests (`noScrollInMainWindow`, `dialogBodies`, `noBoxedOutlines`, `forcedColorsEdges`) are updated to name the new files, never loosened.
- Moved tests are re-homed, not deleted. A dialog's shell tests (focus trap, its Close button) are replaced by the page's equivalents.
- Run `cargo fmt --all` before every Rust commit. `pnpm check:generated` passes after the protocol change.

## Review focus

1. **Esc inside a section's own popup** (the Agents consent card and its diff, the own-agent or own-tool remove confirmation, a theme import's file list, the font field) closes only that popup and keeps the page and any unsaved form. Tests in Task 5 (the page's rule) and Tasks 6 to 8 (each popup prevents its Esc).
2. **A tray "Go to session", a banner click, Departures or a jump from History while Settings is open** closes Settings and shows the session, as it does for the board today. Tests in Task 9 (`trayClosesBoard` and the overlay transitions).
3. **`tools.json` edited by hand while the editor is open.** Save gets 409, the form keeps its draft and says "tools.json changed since you opened it. Reload to see it." A file that can't be read (bad JSON) gets 422 and is never overwritten. Tests in Task 3 (route) and Task 4 (form).
4. **A saved family written another way** (`"cascadia mono "`, `"JETBRAINS MONO"`) selects the bundled choice. A family that isn't installed (a typo under "Another font…") still renders through the fallback stack. Tests in Task 1.
5. **A window at 720 by 480 and text size Larger:** the nav and the "Back to sessions" button stay reachable, and the body scrolls in `.page__body`. Test in Task 5 (CSS guard), checked live in Task 10.

---

### Task 1: The bundled fonts, their notices and the font model

**Files:**
- Create: `src/assets/fonts/CascadiaMono-Regular.woff2`, `CascadiaMono-Bold.woff2`, `AtkinsonHyperlegibleMono-Regular.woff2`, `AtkinsonHyperlegibleMono-Bold.woff2` (copied from the scratchpad `fonts-dl\cc\woff2\static\` and `fonts-dl\atk\`)
- Modify: `src/styles/fonts.css`, `THIRD_PARTY_NOTICES.md` (approved owner-file edit, decision 6), `src/features/terminal/terminalFont.ts`
- Test: `src/features/terminal/terminalFont.test.ts` (extend or create), `src/styles/bundledFonts.test.ts` (new)

**Interfaces:**
- Produces:
  - `BUNDLED_FONTS: readonly ["JetBrains Mono", "Cascadia Mono", "Atkinson Hyperlegible Mono"]`
  - `type FontChoice = { kind: "match" } | { kind: "bundled"; family: BundledFont } | { kind: "other"; family: string }`
  - `fontChoiceOf(family: string | null | undefined): FontChoice`
  - `familyOf(choice: FontChoice): string`: what is saved.
  - `fontStack(family: string, wtFamily?: string): string`
  - `BUNDLED_FONT` stays `"JetBrains Mono"`, and `resolveTerminalFont` keeps its signature.

- [ ] **Step 1: Write the failing tests.**
  - `bundledFonts.test.ts`:
    - every `.woff2` in `src/assets/fonts` has an `@font-face` in `fonts.css` at weight 400 or 700;
    - each of the three families has both weights;
    - each family appears in `THIRD_PARTY_NOTICES.md` beside "SIL Open Font License";
    - each file's SHA-256 (node `crypto`) equals the table above.
  - `terminalFont.test.ts`:
    - `fontChoiceOf("")` and `fontChoiceOf("  ")` are `match`;
    - `fontChoiceOf("cascadia mono ")` is `{ kind: "bundled", family: "Cascadia Mono" }`;
    - `fontChoiceOf("JETBRAINS MONO")` is bundled JetBrains Mono;
    - `fontChoiceOf("FiraCode Nerd Font")` is `other`;
    - `familyOf` round-trips every choice;
    - `fontStack("Cascadia Mono", "CaskaydiaCove NF")` is `"Cascadia Mono", "JetBrains Mono", monospace, "CaskaydiaCove NF"`;
    - `fontStack("JetBrains Mono")` is `"JetBrains Mono", monospace`;
    - quotes and backslashes in either name are stripped;
    - a Windows Terminal family equal to the chosen one isn't repeated.
- [ ] **Step 2: Run `pnpm test src/features/terminal src/styles/bundledFonts.test.ts`.** Expect the new tests to fail.
- [ ] **Step 3: Add the fonts and their notices, and update the font model.**
  - Copy the four files.
  - Add four `@font-face` rules shaped like the JetBrains ones.
  - Add a "Fonts" entry per font to `THIRD_PARTY_NOTICES.md`: name, version or commit, source URL, the copyright line quoted in "Checked before planning", and "SIL Open Font License 1.1", with the full OFL text once.
  - The stack follows PLAN "Terminal font setting": the chosen font, the bundled JetBrains Mono, system monospace, then the Windows Terminal (Nerd) font, so powerline glyphs still resolve.
  - Callers of `fontStack` (`AppearanceDialog`, `RecordingPlayer`, `TerminalView` through `LiveApp`) pass `wtFamily` where they have it.
- [ ] **Step 4: Run the same tests.** Expect them to pass. Then run `pnpm test`: expect every test to pass.
- [ ] **Step 5: Commit** "feat(terminal): bundle Cascadia Mono and Atkinson Hyperlegible Mono, with every font's notice".

### Task 2: The terminal font picker

**Files:**
- Create: `src/features/terminal/FontPicker.tsx`, `FontPicker.css`, `FontPicker.test.tsx`
- Modify: `src/features/appearance/AppearanceDialog.tsx` (the font field is replaced by the picker; Task 6 moves it into Terminal)

**Interfaces:**
- Consumes Task 1's `fontChoiceOf`, `familyOf`, `BUNDLED_FONTS` and `fontStack`.
- Produces `FontPicker({ family, wtFamily, onPick }: { family: string; wtFamily?: string; onPick(family: string): void })`. `onPick` gets the string to save. The parent saves it and announces "Terminal font: <label>".

- [ ] **Step 1: Write the failing tests** (`renderToStaticMarkup` plus the pure helpers).
  - A `fieldset` with legend "Terminal font" holds five radio inputs in the order and with the labels in Global constraints.
  - The checked one follows `fontChoiceOf(family)`.
  - Each label's `style.fontFamily` is `fontStack` of its font. Match my terminal uses `wtFamily ?? "JetBrains Mono"`.
  - Each label carries the sample line.
  - "Another font…" checked renders the text field (label "Font family", the saved text as its value). With another choice checked, there is no field.
  - In forced colours (CSS test on `FontPicker.css`), no `color` or `background` is set inside `@media (forced-colors: active)` except system colours.
  - Pressing Esc in the field reverts the text and calls `preventDefault`. A pure helper `fieldKey(key, draft, saved)` returns `{ next, prevent }`.
- [ ] **Step 2: Run `pnpm test src/features/terminal/FontPicker.test.tsx`.** Expect it to fail.
- [ ] **Step 3: Implement `FontPicker`.**
  - It uses native radios, so the arrow keys move the choice and Tab leaves the group.
  - The field commits on blur and Enter, as the old field did.
  - Load `frontend-design` and `good-css` before writing the CSS.
  - No boxed outlines: the selected option uses `SelectedMark`.
- [ ] **Step 4: Run the test.** Expect it to pass. Run `pnpm test src/features/appearance`: expect it to pass, with the old font-field tests rewritten for the picker.
- [ ] **Step 5: Commit** "feat(terminal): pick the terminal font from a list that shows each one".

### Task 3: `GET` and `PUT /v1/tools/own`

**Files:**
- Modify: `crates/protocol/src/tools.rs`, `crates/core/src/tools/own.rs`, `crates/core/src/server/mod.rs` (routes beside `/v1/tools`), `crates/core/src/server/tools.rs` (handlers; create it if `tools::catalog` lives elsewhere), the `AppState` field next to `own_tools` (a `own_tools_write: Mutex<()>`), and `src/lib/api/catalog.ts`
- Test: unit tests in `tools/own.rs`, route tests where `/v1/agents/own`'s live

**Interfaces:**
- Produces, in Rust:
  - `OwnTools { tools: Vec<ToolProfile>, problems: Vec<OwnToolProblem>, version: String }`. These are the entries as stored, so a problem's `index` points into `tools`. `version` is the SHA-256 hex of the file, or `""` when there is no file.
  - `OwnToolsSave { tools: Vec<ToolProfile>, version: String }`. Both are ts-rs exported.
  - `tools::own::save(home: &Path, req: OwnToolsSave) -> Result<OwnTools, SaveError>`, with `SaveError` shaped as in `agents/own.rs` (`Stale`, `Unreadable`, `Invalid`, `Io`).
  - `ToolsCache::forget()`.
- Produces, in TypeScript: `ownTools.get(endpoint): Promise<OwnTools>` and `ownTools.save(endpoint, req): Promise<OwnTools>`. A 409 throws `StaleError`, and a 422 throws with `problems`, the same shapes `ownAgents` uses.

- [ ] **Step 1: Write the failing tests** (temp `MARSHELL_HOME`).
  - `save_writes_valid_tools_and_returns_the_new_version`.
  - `save_refuses_a_stale_version`: the file changed after `load`, so the result is `Stale` and the bytes are unchanged.
  - `save_refuses_an_unreadable_file_and_leaves_it`: bad JSON gives `Unreadable` and the bytes are unchanged.
  - `save_refuses_an_invalid_regex_with_its_index`.
  - `save_refuses_a_shipped_id`: reason "<id> is a shipped tool's id".
  - `a_broken_entry_sent_back_unchanged_is_kept_byte_equal_in_value`: a hand-written entry with an unknown field stays as the file had it.
  - `get_own_lists_broken_entries_with_their_problems`.
  - Route tests: PUT returns 200, 409 with the exact message, or 422 with a JSON list. A GET after the PUT shows the new entry with no restart, and `/v1/tools` lists it.
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 tools::own server::`.** Expect it to fail.
- [ ] **Step 3: Implement the save.**
  - Mirror `agents/own.rs::save`.
  - `load_full` keeps the raw `Value` of refused entries, so `save` can write them back.
  - The file is `{"version":1,"tools":[…]}`.
  - The handler takes the write mutex, saves in `spawn_blocking` and calls `forget()` after every attempt.
  - Then run `cargo test -p marshell-protocol` and `pnpm check:generated`.
- [ ] **Step 4: Run the tests again,** then `cargo clippy --workspace --all-targets -- -D warnings`. Expect both to pass.
- [ ] **Step 5: Run `cargo fmt --all`, then commit** "feat(tools): save own tool profiles to tools.json through the core".

### Task 4: The own-tools editor

**Files:**
- Create: `src/features/agents/OwnToolsSection.tsx`, `OwnToolsSection.css`, `OwnToolsSection.test.tsx`, `ownTools.ts` (pure draft ⇄ profile helpers) and `ownTools.test.ts`
- Modify: `src/features/agents/ToolsSection.tsx`. The read-only list stays, and its own entries get Edit and Remove. "Add your own tool" opens the form.

**Interfaces:**
- Consumes Task 3's `ownTools.get` and `ownTools.save`.
- Produces `OwnToolsSection({ endpoint, announce })`, which Task 8 places under Agents.
- The form covers:
  - Name. The id is derived as a lowercase slug and shown read-only.
  - Commands: comma-separated program names.
  - Launch presets: each with name, program, and args one per line. Fields come from the `{id}` placeholders in the args, each with a label input.
  - Needs-you patterns: pattern, phrase, and kind "Text" or "Browser".
  - Port lines: pattern.
- Fields the form doesn't show (`scripts`, `launcher`, `quiet_ports`) are kept from the entry being edited.

- [ ] **Step 1: Write the failing tests.**
  - `ownTools.test.ts`:
    - `draftOf(profile)` then `profileOf(draft)` round-trips a profile with every field, including the hidden ones;
    - the placeholders `{target}` and `{ns}` in args give the fields `target` and `ns`, and a label edit survives an args edit;
    - `slug("My Tool!")` is `"my-tool"`.
  - `OwnToolsSection.test.tsx`:
    - a problem from `OwnTools.problems` shows beside its entry with its reason;
    - after a save that gave 409, the form shows "tools.json changed since you opened it. Reload to see it." and keeps the draft;
    - after a 422, each problem shows on its entry;
    - Remove asks "Remove <name>?" with Remove and Cancel, and Esc on the question cancels with `preventDefault`;
    - a saved change announces "Saved <name>".
- [ ] **Step 2: Run `pnpm test src/features/agents/ownTools.test.ts src/features/agents/OwnToolsSection.test.tsx`.** Expect them to fail.
- [ ] **Step 3: Implement the editor.** Follow `OwnAgentsSection`'s structure, state machine and copy style, and use the same CSS primitives as its form.
- [ ] **Step 4: Run the tests, then `pnpm test src/features/agents`.** Expect both to pass.
- [ ] **Step 5: Commit** "feat(tools): add, edit and remove your own tool profiles".

### Task 5: The Settings page shell

**Files:**
- Create: `src/features/settings/SettingsShell.tsx` (presentational), `src/features/settings/settingsSections.ts` (ids, labels, order, `SettingsSection` type), `src/features/settings/SettingsShell.css` and `SettingsShell.test.tsx`
- Modify: `src/styles/noScrollInMainWindow.test.ts` and `dialogBodies.test.ts` only to name the new file, if their rules need it

**Interfaces:**
- Produces:
  - `type SettingsSection = "appearance" | "terminal" | "notifications" | "profiles" | "history" | "agents" | "advanced"`
  - `SETTINGS_SECTIONS: Array<{ id: SettingsSection; label: string }>`
  - `SettingsShell({ section, onSection, onClose, children }: { section: SettingsSection; onSection(id: SettingsSection): void; onClose(): void; children: ReactNode })`. It renders `Page` with title "Settings" and the "Back to sessions" action, the nav, and `children` under an `h2` of the section's label.
  - `settingsKey(e: { key: string; defaultPrevented: boolean }): "close" | null`.
- The phase-0 `SettingsPage` and its fixtures stay for 6e2's company tabs. Its nav markup moves into a shared `SettingsNav` used by both.

- [ ] **Step 1: Write the failing tests.**
  - The nav is `nav[aria-label="Settings sections"]` with the seven labels in order, and only the current one carries `aria-current="page"` and the selected mark.
  - The `h2` holds the label and has `tabIndex=-1`.
  - "Back to sessions" is a button in `.page__actions`.
  - `settingsKey({key:"Escape", defaultPrevented:false})` is `"close"`, and with `defaultPrevented:true` it is `null`.
  - CSS guards: the body scrolls only in `.page__body`; the layout has no fixed width over 720px; at 480px of height the nav and actions sit outside the scroller.
  - `SettingsPage.test.tsx` still passes with the shared `SettingsNav`.
- [ ] **Step 2: Run `pnpm test src/features/settings`.** Expect the new tests to fail.
- [ ] **Step 3: Implement the shell.** Load `frontend-design` and `good-css` first. At narrow widths, the nav wraps into a row above the body through a container query, not a breakpoint ladder.
- [ ] **Step 4: Run `pnpm test src/features/settings src/styles`.** Expect it to pass.
- [ ] **Step 5: Commit** "feat(settings): the Settings page shell and its section nav".

### Task 6: Appearance and Terminal sections

**Files:**
- Modify: `src/features/appearance/AppearanceDialog.tsx`, which becomes `AppearanceSections.tsx`. It exports `AppearanceSection` and `TerminalSection`, each a live wrapper over the shared `useAppearance()` state, with the views split out of `AppearanceView`. Also `AppearanceDialog.css` (renamed with the file), `Appearance.stories.tsx` and the tests.
- Test: `AppearanceDialog.test.tsx`, renamed `AppearanceSections.test.tsx`

**Interfaces:**
- Consumes Task 2's `FontPicker`.
- Produces `AppearanceSection({ announce })` and `TerminalSection({ announce })`.
  - Appearance holds the app theme, accent, contrast and text size.
  - Terminal holds the terminal theme and its preview, the font picker, the font size and the imported themes and theme import.

- [ ] **Step 1: Re-home the tests.** Split them between the two views, drop the dialog-shell assertions (focus trap, the Close button), and add one test per section that it renders no `role="dialog"`. The theme import's list, when open, prevents its Esc.
- [ ] **Step 2: Run `pnpm test src/features/appearance`.** Expect it to fail.
- [ ] **Step 3: Split the view and drop the shell.** Delete `AppearanceDialog`.
- [ ] **Step 4: Run `pnpm test src/features/appearance src/styles`.** Expect it to pass.
- [ ] **Step 5: Commit** "refactor(settings): Appearance and Terminal become Settings sections".

### Task 7: Notifications, Shell profiles, History and Advanced sections

**Files:**
- Modify: `src/features/notifications/NotificationsDialog.tsx` (to `NotificationsSection`), `src/features/settings/ProfilesPage.tsx` (`ProfilesDialog` to `ProfilesSection`), `src/features/history/HistorySettingsDialog.tsx` (to `HistorySection`, with `StorageSection` inside), `src/features/diagnostics/DiagnosticsDialog.tsx` (to `AdvancedSection`), their CSS and tests, and the guard lists in `src/styles/*.test.ts` that name these files
- Test: each one's existing test file, renamed with it

**Interfaces:**
- Produces:
  - `NotificationsSection({ sync })`
  - `ProfilesSection({ endpoint, announce, onSaved })`
  - `HistorySection({ sync, announce })`
  - `AdvancedSection({ endpoint, announce })`

  The props are the dialog's own, minus `onClose`.

- [ ] **Step 1: Re-home each dialog's tests the same way as Task 6.** Shell assertions go, each section renders no `role="dialog"`, and every nested confirmation prevents its Esc.
- [ ] **Step 2: Run `pnpm test src/features/notifications src/features/settings src/features/history src/features/diagnostics`.** Expect it to fail.
- [ ] **Step 3: Drop the shells and rename the files.**
- [ ] **Step 4: Run the same tests plus `src/styles`.** Expect them to pass.
- [ ] **Step 5: Commit** "refactor(settings): Notifications, Shell profiles, History and Advanced become Settings sections".

### Task 8: The Agents section

**Files:**
- Modify: `src/features/agents/AgentsDialog.tsx` (to `AgentsSection.tsx`), `AgentsDialog.css`, `AgentsDialog.test.tsx` and `OwnAgentsSection.tsx` (placement only), and the guard lists naming `AgentsDialog.css`

**Interfaces:**
- Consumes Task 4's `OwnToolsSection`.
- Produces `AgentsSection({ endpoint, sync, startWith, announce, onChanged, onNotifications })`.
  - Its order: setup per CLI, "Your own agents" (`OwnAgentsSection`), then "Tools" (`ToolsSection` with `OwnToolsSection`).
  - `startWith` scrolls to that CLI's card and focuses it, once.

- [ ] **Step 1: Re-home the tests as in Task 6.**
  - The consent card's diff and the Hooks off preview each prevent their Esc.
  - `startWith: "codex"` focuses the Codex card.
  - The own-agent and tools headings appear in that order.
- [ ] **Step 2: Run `pnpm test src/features/agents`.** Expect it to fail.
- [ ] **Step 3: Drop the shell and compose the section.**
- [ ] **Step 4: Run `pnpm test src/features/agents src/styles`.** Expect it to pass.
- [ ] **Step 5: Commit** "refactor(settings): Agents, own agents and tools become one Settings section".

### Task 9: Wiring Settings into the app, the palette and the tray

**Files:**
- Create: `src/features/settings/LiveSettings.tsx`, which composes `SettingsShell` and the seven sections, owns the focus on open and close, and handles Esc
- Modify:
  - `src/features/sessions/LiveApp.tsx`: `overlay` drops the six dialog values and gains `"settings"`; add `settingsSection` state and `lastSettingsSection` for the run; `runFromPalette`, History's `onSettings` and the row's set-up link open their sections.
  - `src/lib/keymap.ts`: the `settings` command.
  - `src/features/tray/trayItems.ts` and `useTray.ts`: the `settings` item and `TrayAction` `{ kind: "overlay"; overlay: "settings" | "departures" }`.
  - `src-tauri/src/tray.rs`, only if it names `notifications`.
  - The palette's command list tests.
- Test: `src/features/settings/LiveSettings.test.tsx`, `trayItems.test.ts`, `useTray.test.ts`, the keymap tests and LiveApp's overlay helpers

**Interfaces:**
- Consumes Tasks 5 to 8.
- Produces `paletteSection(id: string): SettingsSection | null`, with the mapping in Global constraints. `trayClosesBoard` is renamed `trayClosesOverlay`, and it returns true for a `goto` or `nextWaiting` action while `"settings"` or `"departures"` is open.

- [ ] **Step 1: Write the failing tests.**
  - `paletteSection` maps all six old ids and `settings`, and maps everything else to `null`.
  - The tray lists `settings` "Settings…" where `notifications` was, and the item order otherwise matches today's test.
  - `actionFor("settings")` is `{ kind: "overlay", overlay: "settings" }`.
  - `trayClosesOverlay` is true for `goto` and `nextWaiting` over `settings`, and false for `togglePause`.
  - The palette lists "Settings…" in Tools with no shortcut, and `COMMANDS` has no duplicate ids.
  - `LiveSettings` (pure parts) shows the section it was opened at; a tray "Settings…" reopens the last section shown in this run; a section change moves focus to the new `h2`.
- [ ] **Step 2: Run `pnpm test src/features/settings src/features/tray src/lib src/features/sessions`.** Expect it to fail.
- [ ] **Step 3: Wire it in.** Any jump to a session closes Settings: a banner click, a Departures jump, a History "open" or a palette session jump. Remove the old dialog imports.
- [ ] **Step 4: Run `pnpm test`, `pnpm build` and `cargo test -p marshell-app -j 4`** (if `tray.rs` changed). Expect all to pass.
- [ ] **Step 5: Commit** "feat(settings): open Settings from the palette and the tray, at the right section".

### Task 10: Live check and the slice's exit (main thread)

- [ ] **Step 1: Start the app** with the `marshell-live-check` skill: a scratch `MARSHELL_HOME`, a `tools.json` holding one valid and one broken entry, and a Windows Terminal font present. No agent session is needed.
- [ ] **Step 2: Check over CDP** (small JSON evaluations):
  - `document.fonts.check('13px "Cascadia Mono"')` and the same for the other two families are true after the picker shows them;
  - each choice changes xterm's `fontFamily` live, and Match my terminal gives Windows Terminal's family;
  - the palette's six old commands and "Settings…" open the right sections;
  - the tray's "Settings…" opens the last section shown;
  - Esc closes the page, but Esc inside the own-tool remove question doesn't;
  - focus returns to the terminal;
  - a `goto` from the tray closes Settings;
  - saving an own tool writes `tools.json`, and the new preset appears in the picker's Tools group;
  - a hand edit while the form is open gives the 409 copy.
- [ ] **Step 3: Check the keyboard and forced colours.** Run a keyboard-only pass through every section. Emulate forced colours (`Emulation.setEmulatedMedia` `forced-colors: active`) and take one screenshot each of the Terminal section and the Agents section. Take one screenshot of the page at 720 by 480 with text size Larger.
- [ ] **Step 4: Record the findings** in `docs/research/2026-10-2x-settings-page-live-check.md`. Fix what it finds, each fix with its test.
- [ ] **Step 5: Run what CI runs:** `cargo fmt --all --check`, clippy, `cargo test --workspace -j 4`, `pnpm test`, `pnpm check:generated` and `pnpm build`. Then hand over to the final review (`marshell-reviewer`, Opus) with this plan's Review focus.
