# Phase 2d: Themes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Marshell a saved Appearance: app theme, accent (eight presets or a checked custom colour), high contrast, three text sizes, and a terminal theme. The terminal theme follows the app by default and can be set per profile, switches live, and can be imported from Windows Terminal, iTerm2 or VS Code. Running programs are told about it through `COLORFGBG`, OSC 10/11 answers and mode 2031.

**Architecture:** The core owns `~/.marshell/settings.json` through the existing versioned `store::write`/`store::read`, behind `GET`/`PUT /v1/settings`. Only the `appearance` section exists in this slice. The frontend loads it once and writes every resolved choice onto `<html>` as data attributes (`data-theme`, `data-accent`, `data-contrast`, `data-text-size`), which the CSS keys off. The terminal resolves its palette from the setting, the profile override and the imported themes, and switches it live through `term.options.theme`. All theme-file parsing is TypeScript. For Windows Terminal, the core only finds the settings files and returns their text, read-only.

**Tech Stack:** Rust (axum 0.8, serde, ts-rs), React 19 + TypeScript, xterm.js 6, Vitest (node environment, `renderToStaticMarkup` for components).

**Spec:**
- `docs/PLAN.md`:
  - §9 item 2 (slice 2d);
  - "Terminal theme" (lines 145–148);
  - "Visual system" (lines 292–309);
  - line 536 (`settings.json`).
- User decisions from 2026-10-16, recorded in this plan's header and to be added to PLAN by Task 10:
  1. Build the real settings store and API, and wire only the Appearance fields.
  2. Import all three formats from a file, and also list the schemes already in the user's Windows Terminal settings.
  3. High contrast is Follow system (`prefers-contrast: more`) / On / Off.
  4. Text size is Default / Large / Larger. This changes only the UI type scale (13 → 15 → 17 px body); the terminal font size stays its own setting.

## Global Constraints

- **Settings file:**
  - It is `~/.marshell/settings.json`, written only through `crate::store::write`, which backs up, writes atomically and takes the process-wide lock.
  - Version 1.
  - Every field is `#[serde(default)]`, so a missing field gets its default.
- **Defaults:**

  | Setting | Default |
  |---|---|
  | `theme` | `"follow-system"` |
  | `accent` | `"amber"` |
  | `contrast` | `"follow-system"` |
  | `text_size` | `"default"` |
  | `terminal_theme` | `"follow-app"` |
  | `terminal_font` | `{ family: null, size: 13 }` |
  | `contrast_protection` | `true` |
  | `imported_themes` | `[]` |

- **Serde casing:** enum values are kebab-case strings, as above. Field names are snake_case, matching `Profile`.
- **Accents:**
  - Presets are `amber`, `blue`, `indigo`, `violet`, `magenta`, `cyan`, `teal` and `slate`, with amber the default.
  - A custom accent is `#rrggbb`, lower case.
- **Minimum contrast:** `MIN_CONTRAST` 4.5 (already in `palettes.ts`). When contrast protection is off, `minimumContrastRatio` is 1.
- **Never write another program's config.** Windows Terminal, iTerm2 and VS Code files are only read, and only when the user asks (the file picker) or to list WT schemes and the WT font. AGENTS.md: "Never silently edit a user's CLI configuration."
- **No new npm or Cargo dependencies.**
  - File picking uses `<input type="file">`.
  - Parsers are hand-written: a JSONC stripper and a flat plist reader.
  - The only new asset is JetBrains Mono, which PLAN:292 names as the bundled fallback (SIL OFL 1.1; ship its `OFL.txt`).
- **Copy:** sentence case, plain verbs, and errors that say what is wrong and how to fix it (PLAN visual rules). The terminal theme help text must say: "A running program may not notice the change until it redraws or restarts."
- **CSS:**
  - Use tokens only (`tokensOnly.test.ts`).
  - No boxed outlines (`noBoxedOutlines.test.ts`).
  - Forced-colours edges must pass `forcedColorsEdges.test.ts`.
  - No scrollbars in the main window.
- **Keep the Rust simple:** plain structs, `Result<(), String>` validation as in `profiles::store::validate`, and no traits beyond `Versioned`.
- **Generated bindings:** after any protocol change, regenerate them and run `pnpm check:generated`.

## Review Focus

1. **A broken or newer `settings.json`.** The app must still start with defaults and say so once in the log. The broken text must survive: `store::read` already falls back to `.bak`, and the first save backs the broken main file up. Test: `load_broken_file_gives_defaults` (Task 1).
2. **A profile override that names a deleted imported theme.** The terminal falls back to the app's terminal setting and doesn't go blank. Removing the theme that is the current global setting switches the setting to `follow-app` in the same save. Tests: `override_naming_missing_theme_falls_back` (Task 5) and `removing_current_theme_resets_setting` (Task 9).
3. **Imported files that are almost right.**
   - `#rgb` short hex is expanded.
   - `#rrggbbaa` drops the alpha.
   - A scheme missing one of the 16 ANSI colours is rejected, naming the missing key.
   - A VS Code theme without `terminal.*` colours is rejected with "This VS Code theme has no terminal colours."
   - A file over 1 MB is rejected before it's parsed.

   Tests are in Task 6.
4. **Windows Terminal JSON as people really write it:** `//` and `/* */` comments, trailing commas, a UTF-8 BOM, and `//` inside a string value such as a URL. Test: `jsonc_keeps_slashes_inside_strings` (Task 6).
5. **The OS flipping dark/light while everything follows the system.**
   - The terminal palette switches.
   - A program that turned on mode 2031 gets exactly one `CSI ? 997 ; n n` per actual scheme change. A palette change within the same scheme sends nothing.
   - A program that didn't opt in gets nothing.
   - A full reset (`ESC c`) turns the subscription off.

   Tests are in Task 5.

---

### Task 1: Settings store and `/v1/settings`

**Files:**
- Modify: `crates/protocol/src/api.rs`. Add `Settings`, `Appearance`, `AppTheme`, `ContrastMode`, `TextSize`, `TerminalFont`, `ImportedTheme` and `TerminalColors`.
- Create: `crates/core/src/settings.rs`. It provides `load`, `save` and `validate`, and holds the unit tests.
- Modify:
  - `crates/core/src/lib.rs`: add `mod settings`.
  - `crates/core/src/paths.rs`: add `settings_file(home) -> home/.marshell/settings.json`, following `endpoint_file`.
  - `crates/core/src/server/mod.rs`: add a `settings: Mutex<Settings>` field to the state, loaded in the same place as `profiles` (around line 58), and the route.
- Create: `crates/core/src/server/settings.rs`, with `get` and `put` copied in shape from `server/profiles.rs`.
- Regenerate: the `src/generated/*.ts` bindings.

**Interfaces:**
- Produces (Rust, protocol):
  - `Settings { appearance: Appearance }`
  - `Appearance { theme: AppTheme, accent: String, contrast: ContrastMode, text_size: TextSize, terminal_theme: String, terminal_font: TerminalFont, contrast_protection: bool, imported_themes: Vec<ImportedTheme> }`
  - `AppTheme = FollowSystem | Dark | Light`
  - `ContrastMode = FollowSystem | On | Off`
  - `TextSize = Default | Large | Larger`
  - `TerminalFont { family: Option<String>, size: u16 }`
  - `ImportedTheme { id: String, name: String, source: String, colors: TerminalColors }`, where `source` is one of `"windows-terminal" | "iterm2" | "vscode"`
  - `TerminalColors { foreground: String, background: String, cursor: String, selection: String, ansi: Vec<String> }`
  - Each struct derives `Default` with the Global Constraints defaults, plus `#[serde(default)]`.
- Produces (core):
  - `settings::load(home: &Path) -> Settings`
  - `settings::save(home: &Path, s: &Settings) -> anyhow::Result<()>`
  - `settings::validate(s: &Settings) -> Result<(), String>`
- Produces (HTTP): `GET /v1/settings -> Settings`, and `PUT /v1/settings (Settings) -> Settings`, which returns 400 with the `validate` message.

- [ ] **Step 1: Write the failing tests in `crates/core/src/settings.rs`.**

```rust
#[test] fn missing_file_gives_defaults()        // load(tmp) == Settings::default(); accent "amber", size 13
#[test] fn save_then_load_round_trips()         // custom accent "#3366cc", one imported theme
#[test] fn load_broken_file_gives_defaults()    // write "{not json" to settings.json, no .bak → load == default, file text unchanged
#[test] fn missing_fields_take_defaults()       // file {"version":1,"appearance":{"accent":"teal"}} → accent teal, terminal_theme "follow-app"
#[test] fn validate_rejects_bad_values()        // each case below returns Err containing the quoted word
```

These are the `validate` cases and the words their errors must contain:

| Bad value | Error contains |
|---|---|
| accent `"pink"` | `"accent"` |
| accent `"#12345"` | `"accent"` |
| font size 7 | `"font size"` (allowed range 8–32) |
| font size 33 | `"font size"` |
| an imported theme with 15 ANSI colours | `"16"` |
| colour `"red"` | `"#rrggbb"` |
| `terminal_theme` `"nope"` that is not a built-in and not an imported id | `"terminal theme"` |
| two imported themes with the same id | `"id"` |

- [ ] **Step 2: Run them to verify they fail.**
  Run: `cargo test -p marshell-core --lib settings`.
  Expected: they fail to compile because `settings` is not defined.

- [ ] **Step 3: Implement the protocol types and `settings.rs`.**
  - Wrap the document as `FileDoc { appearance }` with `impl Versioned { const VERSION: u32 = 1 }`, the same way `profiles/store.rs:16` does.
  - In `load`, call `store::read`. On `Err`, log a warning with `tracing::warn!` and return the default. On `Ok(None)`, return the default.
  - `validate` checks:
    - accent is a preset or `^#[0-9a-f]{6}$`;
    - font size is 8–32;
    - each colour is `#rrggbb`;
    - there are exactly 16 ANSI colours;
    - imported ids are unique;
    - `terminal_theme` is in `{"follow-app","dark","light"}` or is an imported id.

- [ ] **Step 4: Add the route and handlers, and run the tests.**
  - Register `.route("/v1/settings", get(settings::get).put(settings::put))`.
  - In `put`, call `validate`, then `spawn_blocking(save)`, then update the in-memory copy.
  - Add a handler test next to the existing server tests: PUT with accent `"pink"` gets 400, and PUT with a valid body followed by GET returns it.
  - Run: `cargo test -p marshell-core`. Expected: PASS.

- [ ] **Step 5: Regenerate the bindings and commit.**
  - Run: `pnpm check:generated`. Expected: clean after regenerating, the same way 2b/2c did.
  - Commit: `git commit -q -F <msg>` with the message `feat(core): settings.json store and /v1/settings for appearance`.

---

### Task 2: Profile terminal theme and `COLORFGBG` at launch

**Files:**
- Modify:
  - `crates/protocol/src/api.rs`:
    - add `Profile.terminal_theme: Option<String>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`, `#[ts(optional)]`);
    - add `CreateSessionRequest.color_scheme: Option<ColorScheme>`, with the same attributes;
    - add a new enum `ColorScheme = Dark | Light` (kebab-case).
  - `crates/core/src/server/sessions.rs`, or wherever `create` builds the `SpawnSpec` env: insert `COLORFGBG` before the profile env, so a profile's own `COLORFGBG` wins.
  - `crates/core/src/profiles/store.rs`: `validate` lets the field through as is. It can't see settings; the frontend handles a stale id (see Review Focus 2).
- Test: the existing sessions tests module.

**Interfaces:**
- Produces:
  - `fn colorfgbg(scheme: ColorScheme) -> &'static str`, which returns `"15;0"` for dark and `"0;15"` for light.
  - When `color_scheme` is `None`, the core sets no `COLORFGBG`.

- [ ] **Step 1: Write the failing tests.**
  - `colorfgbg_values`: dark gives `"15;0"`, light gives `"0;15"`.
  - `spawn_env_puts_colorfgbg_before_profile_env`: build the env for a request with `Some(Dark)` and a profile env of `[("COLORFGBG","7;0")]`. The last `COLORFGBG` in the list must be `"7;0"`, and a request with no profile env must contain `("COLORFGBG","15;0")`.
  - If the env is built inline, pull it into `fn spawn_env(req_scheme: Option<ColorScheme>, profile_env: &[(String,String)]) -> Vec<(String,String)>` first so the test can call it.
- [ ] **Step 2: Run them to verify they fail.** Run `cargo test -p marshell-core --lib colorfgbg spawn_env`. Expected: they fail to compile.
- [ ] **Step 3: Implement it, then run `cargo test -p marshell-core`.** Expected: PASS. Also run `cargo test -p marshell-protocol` and regenerate the bindings.
- [ ] **Step 4: Commit:** `feat(core): COLORFGBG at launch and a per-profile terminal theme field`.

---

### Task 3: Custom accent check

**Files:**
- Create: `src/features/appearance/customAccent.ts` and `customAccent.test.ts`.

**Interfaces:**
- Consumes: `contrast` and `minOnSurfaces` from `src/design/contrast.ts`, and `parseTokens` over `tokens.css`, using the same import the existing contrast test uses.
- Produces:
  - `normalizeHex(input: string): string | null`. It accepts `#abc`, `abc`, `#AABBCC` and `aabbcc`, returns `#aabbcc`, and returns `null` for anything else.
  - `checkAccent(input: string): { ok: true; hex: string; ink: "#000000" | "#ffffff" } | { ok: false; reason: string }`.

The check:
- `ink` is whichever of black or white has the higher contrast with the hex, and that contrast must be at least 4.5.
- The hex must have at least 3:1 on every surface token in the light theme and in the dark theme. It's used for focus rings and fills, so WCAG 1.4.11 applies.

The reason copy is exact:
- `"Enter a colour as #rrggbb, for example #3a7bd5."`
- `"Too light for the light theme: {n}:1 against the background, needs 3:1."`
- `"Too dark for the dark theme: {n}:1 against the background, needs 3:1."`
- `"Text on this colour would be hard to read: {n}:1, needs 4.5:1."`

`{n}` has one decimal place. When more than one rule fails, report the first in this order: format, light, dark, ink.

- [ ] **Step 1: Write the failing tests.**

| Test | Expectation |
|---|---|
| `normalizeHex` | expands `#ABC`, adds a missing `#`, rejects `"#12345"` and `"blue"` |
| a mid blue | `#3a7bd5` is ok, with ink `#ffffff` or `#000000`. Assert `ok` and that `contrast(hex, ink) >= 4.5`. |
| `#ffff66` | fails with a reason starting `"Too light for the light theme"` |
| `#202020` | fails with a reason starting `"Too dark for the dark theme"` |
| every preset's dark-theme value | passes, so the check isn't stricter than the shipped accents. Read the values from `accents.css` with `parseTokens`. If a preset fails, the check is wrong; don't change the preset. |

- [ ] **Step 2: Run them to verify they fail.** Run `pnpm test -- customAccent`. Expected: FAIL (module missing).
- [ ] **Step 3: Implement it.** Expected: `pnpm test` passes.
- [ ] **Step 4: Commit:** `feat(appearance): check a custom accent against both themes`.

---

### Task 4: Frontend settings client, root attributes, high contrast and text size

**Files:**
- Create:
  - `src/lib/api/settings.ts`, in the shape of `src/lib/api/profiles.ts`.
  - `src/features/appearance/rootState.ts`, which is pure.
  - `src/features/appearance/AppearanceProvider.tsx`, which provides the context and the effects.
  - `src/styles/appearance.css`, holding the `[data-contrast="more"]` and `[data-text-size]` blocks, and imported wherever `tokens.css` is imported.
- Modify: `src/App.tsx`, to wrap `<LiveApp />` in `<AppearanceProvider>`. The provider needs the endpoint, so it can go inside LiveApp at the top if the endpoint is only known there; the implementer picks one and notes it.
- Test:
  - `src/features/appearance/rootState.test.ts`;
  - extend `src/design/contrast.test.ts` for the `[data-contrast="more"]` block;
  - `src/styles/textSize.test.ts`.

**Interfaces:**
- Consumes: the `Settings` and `Appearance` TS types from `src/generated/`.
- Produces:
  - `settingsApi.get(ep): Promise<Settings>` and `settingsApi.put(ep, s): Promise<Settings>`.
  - `rootState(a: Appearance, sys: { prefersContrastMore: boolean }): { attrs: Record<"data-theme"|"data-accent"|"data-contrast"|"data-text-size", string | null>; style: Record<"--accent"|"--accent-ink", string | null> }`. Here `null` means "remove the attribute or property".
  - `useAppearance(): { appearance: Appearance; save(next: Appearance): Promise<void>; error: string | null }`. `save` applies the new values at once, rolls back and sets `error` if the PUT fails, and `error` is shown by the caller.

The `rootState` rules:

| Attribute | Value |
|---|---|
| `data-theme` | `null` for follow-system, otherwise `"dark"` / `"light"` |
| `data-accent` | the preset name, or `"custom"` with `style["--accent"]` set to the hex and `style["--accent-ink"]` set from `checkAccent(...).ink` (Task 3). |
| `data-contrast` | `"more"` when contrast is On, or Follow system with `prefersContrastMore`; otherwise `null` |
| `data-text-size` | `null` for default, otherwise `"large"` / `"larger"` |

- [ ] **Step 1: Write the failing tests.**
  - `rootState.test.ts`, one test per row of the table above, plus `follow-system contrast is off when the OS does not ask`.
  - `textSize.test.ts`: parse `appearance.css` with `parseTokens` (`src/design/cssTokens.ts`). Under `[data-text-size="large"]` and `[data-text-size="larger"]` the tokens must equal this table:

    | Token | Large | Larger |
    |---|---|---|
    | `--text-11` | `0.75rem` | `0.8125rem` |
    | `--text-12` | `0.8125rem` | `0.9375rem` |
    | `--text-13` | `0.9375rem` | `1.0625rem` |
    | `--text-15` | `1.0625rem` | `1.25rem` |
    | `--text-20` | `1.375rem` | `1.5rem` |
    | `--text-28` | `1.875rem` | `2rem` |

    In px, Large is 12/13/15/17/22/30 and Larger is 13/15/17/20/24/32. Body (`--text-13`) is 15 px on Large and 17 px on Larger.
  - `contrast.test.ts`, new case `high contrast text reaches 7:1`. With the `[data-contrast="more"]` overrides applied over `:root`, every `--text-*` colour token has at least 7:1 on every surface token in both themes (use `minOnSurfaces`). Every divider or border token has at least 3:1 on its surfaces. No overlay or tint token in the block has alpha below 0.2. Read tokens.css to find which tokens are dividers and overlays, and list them in the test.
- [ ] **Step 2: Run them to verify they fail.** Run `pnpm test -- appearance textSize contrast`. Expected: FAIL (module missing, block missing).
- [ ] **Step 3: Implement `settings.ts`, `rootState.ts`, `appearance.css` and the provider.**
  - The provider:
    - fetches once when the endpoint is known;
    - writes `rootState` onto `document.documentElement`;
    - listens to `matchMedia("(prefers-contrast: more)")` changes.
  - If the GET fails, keep the defaults, so the app never blocks on settings.
- [ ] **Step 4: Run `pnpm test`.** Expected: all pass, including the existing token and CSS rule tests.
- [ ] **Step 5: Commit:** `feat(appearance): settings client, root attributes, high contrast and text size`.

---

### Task 5: Terminal theme resolution, live switch and mode 2031

**Files:**
- Modify:
  - `src/features/terminal/terminalTheme.ts`;
  - `src/features/terminal/TerminalView.tsx`: the theme section at lines 145–153, and a new prop;
  - `src/features/sessions/LiveApp.tsx`: pass each tab its resolved setting.
- Create:
  - `src/features/terminal/colorSchemeMode.ts` and its `colorSchemeMode.test.ts`;
  - extend `src/features/terminal/terminalTheme.test.ts`, or create it if it doesn't exist.

**Interfaces:**
- Consumes:
  - `Appearance` and `ImportedTheme` (Task 1);
  - `Profile.terminal_theme` (Task 2);
  - `useAppearance` (Task 4).
- Produces:
  - `type TerminalThemeSetting = "follow-app" | "dark" | "light" | string`, where any other string is an imported id.
  - `themeFromColors(c: TerminalColors): ITheme`. It maps `ansi[0..15]` to `black…brightWhite` in xterm order, sets `selectionBackground` from `c.selection`, and keeps the transparent scrollbar fields from `PALETTES`.
  - `schemeOfBackground(hex: string): Scheme`. It returns `"dark"` when the background's relative luminance is below 0.5.
  - `resolveTerminalTheme(o: { setting: string; override?: string; imported: ImportedTheme[]; app: Scheme; protection: boolean }): { theme: ITheme; scheme: Scheme; minimumContrastRatio: number }`:
    - The override wins if it's set and resolves.
    - An unknown id falls back: override → setting → `follow-app`.
    - `minimumContrastRatio` is `protection ? MIN_CONTRAST : 1`.
  - `trackColorSchemeMode(term: SchemeTerm, send: (data: string) => void, current: () => Scheme): { notify(scheme: Scheme): void; dispose(): void }`:
    - `SchemeTerm` is the minimal `{ parser: { registerCsiHandler, registerEscHandler } }` slice, on the pattern of `PromptTerm` in `promptMarks.ts`.
    - It handles `CSI ? 2031 h` (on) and `CSI ? 2031 l` (off) and returns `false` so xterm's own handling continues.
    - `CSI ? 996 n` replies at once with `\x1b[?997;1n` for dark or `\x1b[?997;2n` for light, and returns `true`.
    - `ESC c` turns it off and returns `false`.
    - `notify` sends only when the mode is on and the scheme differs from the last one sent or reported.
- TerminalView prop: `theme: { setting: string; override?: string }`. The view resolves it with `appScheme()` and the appearance context. Keep the existing `prefers-color-scheme` listener and the `data-theme` MutationObserver as the triggers for `follow-app`. On every resolve, set `term.options.theme` and `minimumContrastRatio` and call `notify(scheme)`. The terminal isn't recreated, so the scrollback is kept.
- OSC 10/11/12: xterm.js 6 already answers `OSC 10/11/12 ; ?` from the current `options.theme`, through its colour-report event. Add no handler. Step 4 checks that this is true in the installed version.
- While no renderer is attached, the core doesn't answer OSC 11. Programs that ask time out and use their default. This is a ruling and goes in the PR's open items.
- The `LiveApp` override comes from the session's `profile_id` in the profiles doc. The launcher already has a way to get that doc; if LiveApp doesn't hold it, fetch it once with `profilesApi.get`.
- New sessions: the launch path sets `CreateSessionRequest.color_scheme` to the scheme `resolveTerminalTheme` gives for that profile.

- [ ] **Step 1: Write the failing tests.**
  - `terminalTheme.test.ts`:
    - `follow-app takes the app scheme`;
    - `imported theme resolves and reports its scheme`, with background `#fdf6e3` giving light;
    - `override wins over setting`;
    - `override_naming_missing_theme_falls_back` to the setting;
    - `setting naming missing theme falls back to follow-app`;
    - `protection off gives ratio 1`;
    - `themeFromColors maps ansi in order`: `ansi[1]` is `red` and `ansi[9]` is `brightRed`.
  - `colorSchemeMode.test.ts`, with a fake term that records the handlers and lets the test call them:
    - `reports nothing before 2031 is set`;
    - `notify after 2031 h sends 997;1 for dark`;
    - `no second report for the same scheme`;
    - `996 query answers at once even without 2031`;
    - `2031 l stops reports`;
    - `ESC c stops reports`;
    - `handlers let xterm continue for h/l` (they return `false`).
- [ ] **Step 2: Run them to verify they fail.** Run `pnpm test -- terminalTheme colorSchemeMode`. Expected: FAIL.
- [ ] **Step 3: Implement it and wire up TerminalView, LiveApp and the launch path.**
- [ ] **Step 4: Check the OSC 10/11 claim.** Run `rg -n "\]1[01];|ColorRequestType|_handleColorEvent" node_modules/@xterm/xterm/lib/xterm.mjs` and expect a match. If there is no match, stop and rule: add OSC 10/11 handlers in `colorSchemeMode.ts` that reply `\x1b]1x;rgb:rrrr/gggg/bbbb\x1b\\` from the resolved theme, with tests.
- [ ] **Step 5: Run `pnpm test` and `pnpm build`.** Expected: all pass, and `tsc` is clean.
- [ ] **Step 6: Commit:** `feat(terminal): theme setting, per-profile override, live switch and mode 2031`.

---

### Task 6: Theme file parsers

**Files:**
- Create in `src/features/appearance/import/`:
  - `jsonc.ts`;
  - `windowsTerminal.ts`;
  - `iterm.ts`;
  - `vscode.ts`;
  - `importTheme.ts`, the entry point that picks a parser by extension and content;
  - and a test file for each. Fixtures are inline strings in the tests.

**Interfaces:**
- Produces:
  - `parseJsonc(text: string): unknown`. It strips a leading BOM, `//` and `/* */` comments outside strings, and trailing commas before `}` or `]`, then calls `JSON.parse`. It throws `Error("Not valid JSON: <JSON.parse message>")`.
  - `parseWtSchemes(text: string): ImportedTheme[]`. It accepts a WT `settings.json` (reading `schemes[]`) or a single scheme object.
  - `wtDefaultFont(text: string): { family?: string; size?: number }`. It reads `profiles.defaults.font.face` and `.size`, falling back to the legacy `profiles.defaults.fontFace` and `fontSize`.
  - `parseItermColors(text: string): ImportedTheme`, which reads a flat plist.
  - `parseVsCodeTheme(text: string): ImportedTheme`.
  - `importThemeFile(name: string, text: string): { ok: true; themes: ImportedTheme[] } | { ok: false; reason: string }`:
    - `.itermcolors` goes to iTerm.
    - `.json` goes to WT if it has `schemes` or a `brightPurple` key, and to VS Code if it has `colors` or `tokenColors`.
    - Anything else gives `"Marshell can read Windows Terminal .json, iTerm2 .itermcolors and VS Code theme .json files."`
    - Text over 1 MB (`text.length > 1_000_000`) gives `"This file is too big to be a theme (over 1 MB)."`
  - `normalizeColor(v: string): string | null`, which turns `#rgb`, `#rrggbb` and `#rrggbbaa` into `#rrggbb`. It goes in `jsonc.ts` or a shared `colors.ts`.

Ids are `"<source>:<slug of name>"`, for example `wt:one-half-dark`. A duplicate id gets `-2`.

The key maps:

| Theme part | WT | VS Code | iTerm (plist dict) |
|---|---|---|---|
| ANSI colours | `black, red, green, yellow, blue, purple, cyan, white, brightBlack, brightRed, brightGreen, brightYellow, brightBlue, brightPurple, brightCyan, brightWhite` | `terminal.ansiBlack … terminal.ansiBrightWhite` (Magenta, not Purple) | `"Ansi 0 Color"` … `"Ansi 15 Color"`, each a dict of `"Red Component"`, `"Green Component"` and `"Blue Component"` reals from 0 to 1 |
| Foreground | `foreground` | `terminal.foreground`, else `editor.foreground` | `"Foreground Color"` |
| Background | `background` | `terminal.background`, else `editor.background` | `"Background Color"` |
| Cursor | `cursorColor`, default foreground | `terminalCursor.foreground`, default foreground | `"Cursor Color"`, default foreground |
| Selection | `selectionBackground`, default `#808080` | `terminal.selectionBackground`, default `#808080` | `"Selection Color"`, default `#808080` |
| Name | `name` | `name`, else the file name | the file name without its extension |

- [ ] **Step 1: Write the failing tests.**

| File | Tests |
|---|---|
| `jsonc.test.ts` | `strips comments and trailing commas`; `strips BOM`; `jsonc_keeps_slashes_inside_strings` (`{"u":"https://x//y"}` keeps the value); `error says Not valid JSON` |
| `windowsTerminal.test.ts` | `reads schemes from settings.json` (two schemes, comments); `reads a single scheme object`; `missing brightPurple is rejected naming brightPurple`; `wtDefaultFont reads new and legacy keys` |
| `iterm.test.ts` | `converts 0–1 components to hex` (Red 1, Green 0.5, Blue 0 gives `#ff8000`); `missing Ansi 7 Color is rejected naming it` |
| `vscode.test.ts` | `reads terminal colours`; `falls back to editor colours for fg/bg`; `no terminal colours gives the exact reason`, which is `"This VS Code theme has no terminal colours."` |
| `importTheme.test.ts` | `rejects files over 1 MB before parsing`; `rejects unknown extensions with the exact copy`; `normalizes #abc and #aabbccdd` |

- [ ] **Step 2: Run them to verify they fail.** Run `pnpm test -- import`. Expected: FAIL.
- [ ] **Step 3: Implement the parsers.**
  - The plist reader only needs `<dict>`, `<key>`, `<real>`, `<integer>` and `<string>`, nested one level. Use a small tokenizer over tags; the test env has no `DOMParser`.
  - The rounding rule: a hex byte is `Math.round(c * 255)`.
- [ ] **Step 4: Run `pnpm test`.** Expected: PASS.
- [ ] **Step 5: Commit:** `feat(appearance): read Windows Terminal, iTerm2 and VS Code themes`.

---

### Task 7: Find the Windows Terminal settings files (core, read-only)

**Files:**
- Create: `crates/core/src/wt.rs`.
- Modify:
  - `crates/protocol/src/api.rs`: add `FoundFile { path: String, text: String }`.
  - `crates/core/src/server/mod.rs`: add the route `GET /v1/import/windows-terminal`.
  - A small handler, which can live in `server/settings.rs`.

**Interfaces:**
- Produces:
  - `wt::candidates(local_app_data: &Path) -> Vec<PathBuf>`, in this order:
    1. `Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json`
    2. `Packages\Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe\LocalState\settings.json`
    3. `Microsoft\Windows Terminal\settings.json`
  - `wt::find(local_app_data: &Path) -> Vec<FoundFile>`. It reads the candidates that exist, skips files over 1 MB and files that fail to read, and never writes.
  - HTTP: `GET /v1/import/windows-terminal -> Vec<FoundFile>`. It uses `dirs::data_local_dir()` on Windows. On other platforms, and when there is no directory, it returns an empty list.

- [ ] **Step 1: Write the failing tests.**
  - `candidates_in_order`.
  - `find_reads_existing_and_skips_missing`: a temp dir with only the Preview file gives one `FoundFile` with its text.
  - `find_skips_files_over_1mb`.
- [ ] **Step 2: Run them to verify they fail.** Run `cargo test -p marshell-core --lib wt`. Expected: they fail to compile.
- [ ] **Step 3: Implement it.** Then run `cargo test -p marshell-core`, `cargo clippy --workspace --all-targets -- -D warnings`, and regenerate the bindings. Expected: all clean.
- [ ] **Step 4: Commit:** `feat(core): list Windows Terminal settings files for theme import`.

---

### Task 8: Terminal font: imported from Windows Terminal, else bundled JetBrains Mono

**Files:**
- Create:
  - `src/assets/fonts/JetBrainsMono-Regular.woff2`, `JetBrainsMono-Bold.woff2` and `OFL.txt`, from the official JetBrains Mono 2.304 release (`fonts/webfonts/`);
  - `src/styles/fonts.css`, with `@font-face` for `"JetBrains Mono"` at 400 and 700 and `font-display: block`;
  - `src/features/terminal/terminalFont.ts` and its `terminalFont.test.ts`.
- Modify:
  - `TerminalView.tsx`: set `fontFamily` and `fontSize` from the resolved font live, then re-fit. Before the first `open`, await `document.fonts.load` for the family so xterm measures the right glyphs. If that fails, go on anyway.
  - `src/styles/tokens.css:49`: `--font-mono` puts `"JetBrains Mono"` first among the fallbacks after any user family. Keep the rest of the stack.

**Interfaces:**
- Consumes: `wtDefaultFont` (Task 6), the `/v1/import/windows-terminal` endpoint (Task 7), and `Appearance.terminal_font` (Task 1).
- Produces: `resolveTerminalFont(setting: TerminalFont, wt: { family?: string; size?: number } | null): { family: string; size: number; source: "yours" | "windows-terminal" | "bundled" }`:
  - If `setting.family` is set, use it as `"yours"` with `setting.size`.
  - Otherwise, if WT has a family, use it as `"windows-terminal"` with `setting.size`. The WT size is shown in the dialog as a hint only; it doesn't override the user's size.
  - Otherwise use `"JetBrains Mono"` as `"bundled"`.
  - `family` is passed to xterm as the CSS stack `"<family>", "JetBrains Mono", monospace`.

- [ ] **Step 1: Write the failing tests.** Add `yours wins`, `windows terminal next` and `bundled last`. Add `family stack always ends in JetBrains Mono, monospace`.
- [ ] **Step 2: Run them to verify they fail.** Run `pnpm test -- terminalFont`. Expected: FAIL.
- [ ] **Step 3: Add the font files, the CSS and the resolver, and wire up TerminalView.** Download with `curl -L` from the GitHub release zip into the scratchpad, then copy only the two woff2 files and `OFL.txt`.
- [ ] **Step 4: Run `pnpm test` and `pnpm build`.** Expected: PASS, and the build output includes the two woff2 files.
- [ ] **Step 5: Commit:** `feat(terminal): font from Windows Terminal or bundled JetBrains Mono`.

---

### Task 9: Appearance dialog and profile override

**Files:**
- Create:
  - `src/features/appearance/AppearanceDialog.tsx`, with a stateless `AppearanceView` for tests, like `ProfilesPageView`;
  - `AppearanceDialog.css`;
  - `AppearanceDialog.test.tsx`;
  - `Appearance.stories.tsx`.
- Modify:
  - `src/lib/keymap.ts`: add a command `{ id: "appearance", label: "Appearance…", group: "Tools" }`.
  - `src/features/sessions/LivePalette.tsx`: let `appearance` through the `LIVE_COMMANDS` filter, as `profiles` is.
  - `LiveApp.tsx`: add the `"appearance"` overlay.
  - `ProfilesPage.tsx`: add a "Terminal theme" select per profile. It offers "Same as the app setting" (no override), Dark, Light, and each imported theme.

**Interfaces:**
- Consumes: `useAppearance` (Task 4), `checkAccent` (Task 3), `importThemeFile` and `parseWtSchemes` (Task 6), `GET /v1/import/windows-terminal` (Task 7), and `resolveTerminalFont` (Task 8).
- Produces: `removeImported(a: Appearance, id: string): Appearance`. It drops the theme, and if `a.terminal_theme === id` it sets `"follow-app"` (Review Focus 2). It goes in `rootState.ts` or a small `appearanceEdits.ts`.

The dialog's sections, in this order. Each control saves at once through `save()`, because these settings apply live and there is no change bar. A failed save shows `error` inline next to the control.

1. **Theme:** a radio group with Follow system, Dark and Light.
2. **Accent:**
   - A radio group of eight swatches. Each has an accessible name such as "Amber", and the selection is shown by `SelectedMark`, not by colour alone.
   - Then a "Custom" swatch with a hex text field. It saves on Enter or blur, but only when `checkAccent` is ok. Otherwise the reason is shown in the field's description (`aria-describedby`) and nothing is saved.
   - Help text: "Sessions use their agent's colour. Your accent shows everywhere else."
3. **High contrast:** Follow system, On, Off. Help: "Follow system turns this on when Windows asks for more contrast."
4. **Text size:** Default, Large, Larger. Help: "Changes the size of Marshell's text. The terminal font size is set below."
5. **Terminal:**
   - **Theme:** a select with Follow the app, Dark, Light and each imported theme, plus a 16-colour preview strip. The help text is the Global Constraints copy.
   - **Font:** shows "From Windows Terminal: Cascadia Mono", "Bundled: JetBrains Mono" or your own, with a text field to set your own (empty means automatic).
   - **Size:** a number from 8 to 32.
   - **Contrast protection:** a switch. Help: "Lifts dim text so it stays readable on the terminal's background."
6. **Imported themes:**
   - "Import a theme file…" opens `<input type="file" accept=".json,.itermcolors">`. Its result is added to `imported_themes`. A failure shows the parser's reason.
   - "In your Windows Terminal settings" lists `parseWtSchemes` over every `FoundFile`, each with "Add". When nothing is found: "No Windows Terminal settings found."
   - Each imported theme has "Remove".

- [ ] **Step 1: Write the failing tests** in `AppearanceDialog.test.tsx`, rendering `AppearanceView` with `renderToStaticMarkup`:
  - `marks the current accent` with `aria-checked`;
  - `custom accent shows the reason and keeps the old value`, with an invalid hex as state;
  - `lists Windows Terminal schemes with Add`;
  - `empty Windows Terminal list says so`;
  - `terminal help says a running program may not notice`;
  - `removing_current_theme_resets_setting`, a unit test on `removeImported`.

  In `ProfilesPage.test.tsx`, add `profile terminal theme select offers same-as-app and imported themes`.
- [ ] **Step 2: Run them to verify they fail.** Run `pnpm test -- Appearance ProfilesPage`. Expected: FAIL.
- [ ] **Step 3: Implement the dialog and its CSS.**
  - Follow `frontend-design` and the PLAN visual rules: no boxed outlines and tokens only.
  - Copy the dialog shell (focus trap via `useDialogFocus`, Escape closes) from `ProfilesDialog`.
  - Story: `Appearance.stories.tsx` with default, custom-accent-error and with-WT-schemes states.
- [ ] **Step 4: Run `pnpm test` and `pnpm build`.** Expected: PASS. The a11y rules test (`src/components/a11y/rules.test.tsx`) must also pass if it picks up the new stories.
- [ ] **Step 5: Commit:** `feat(appearance): appearance dialog and per-profile terminal theme`.

---

### Task 10: Docs and the manual checklist

**Files:**
- Modify `docs/PLAN.md`:
  - §9 item 2: mark 2d built, and note the four user decisions in one line each.
  - "Terminal theme": the OSC 10/11 answers come from xterm while a renderer is attached, with no answer before that.
  - The file map: add `settings.rs`, `wt.rs`, `src/features/appearance/` and `src/styles/appearance.css`/`fonts.css`.
- Modify `docs/BRIEF.md` amendments: one line for the 2026-10-16 appearance decisions, if the amendment list records slice decisions (check how 2b/2c did it and match).
- PR body (scratchpad): a manual `pnpm tauri dev` checklist covering:
  - switching the theme with a running `vim`/`nvim` (scrollback kept);
  - Windows "Contrast themes" off and on, with `prefers-contrast` to check;
  - text sizes;
  - importing one file of each format;
  - the WT schemes list;
  - `echo $env:COLORFGBG` in a new PowerShell tab;
  - `printf '\e]11;?\a'` in Git Bash, which shows an `rgb:` reply.

- [ ] **Step 1: Make the edits.**
- [ ] **Step 2: Run the full CI set locally** (fmt, clippy, cargo test, `pnpm check:generated`, `pnpm test`, `pnpm build`). Expected: all green.
- [ ] **Step 3: Commit:** `docs: slice 2d themes in PLAN`.
