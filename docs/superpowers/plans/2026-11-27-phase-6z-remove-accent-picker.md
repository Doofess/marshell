# Slice 6z: the accent picker goes

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Appearance no longer offers an accent. The app accent is sand everywhere no session is in play, a session keeps its CLI's colour, and an `accent` that an older build saved is ignored, then left out of the next save.

**Architecture:** The app accent is already the `:root` block of `src/styles/accents.css`. The eight other preset blocks and every way of choosing one go: the `[data-accent]` attribute and the inline custom colour that `rootState.ts` writes on the page root, the custom-hex check, the Accent part of the Appearance section, the core's preset list, its check and its load repair, and the `accent` field of `Appearance` in the protocol. Serde already skips unknown keys in `Appearance`, so an old file still reads. The vendor accent (`--accent: var(--agent-signal)` inside a session's element, `agent-colors.css`) is not touched.

**Tech Stack:** React 19 + TypeScript (Vitest in node, `renderToStaticMarkup`), CSS custom properties with `light-dark()`, Rust (serde, axum), ts-rs.

**Spec:** `docs/PLAN.md` §9, "Slice 6z decisions (2026-11-27)" (decisions 1 to 3 and "For its plan to settle").

## Global Constraints

- Only the chosen accent goes. `--agent-signal`, `--agent-ink`, `agent-colors.css`, the status colours, marks, dots and stripes don't change. No stylesheet outside `accents.css` changes its colours.
- The tokens `--accent`, `--accent-ink`, `--accent-tint` and `--accent-flash` stay, with sand's values unchanged: `--accent: light-dark(oklch(42% 0.028 70), oklch(80% 0.045 75));` and `--accent-ink: light-dark(oklch(100% 0 none), oklch(0% 0 none));`.
- A saved `accent` is dropped quietly (decision 3). Nothing migrates, nothing rewrites `settings.json` at start, and nothing tells the user. No new copy is shown anywhere.
- `src/generated/` is never edited by hand: `cargo test -p marshell-protocol -j 4` writes it, and the result is committed.
- No new dependencies. Cargo always runs with `-j 4`.
- Don't start `pnpm tauri dev` before Task 5.
- Workers don't run `git checkout`, `git restore` or `git stash` on files they don't own, and delete nothing outside the files their task names.
- Commits read `type(scope): what changed, as a sentence.` and end with the session's attribution lines. Run `cargo fmt --all` before a Rust commit.

## What was checked before planning

- `Appearance` in `crates/protocol/src/api.rs` is `#[serde(default)]` with no `deny_unknown_fields`, and so is `FileDoc` in `crates/core/src/settings.rs`. The only `deny_unknown_fields` in the crates is in `crates/protocol/src/manifest.rs`.
- `settings::save` builds the file from `Settings` alone (`FileDoc { appearance: s.appearance.clone(), .. }`) and keeps no unknown keys, so a dropped field is gone at the next save. `settings::load` never writes.
- The PUT `/v1/settings` body is `Settings`, so a body that still carries `appearance.accent` deserializes and the key is ignored.
- `parseTokens(css, ":root")` returns the first rule whose selector list holds `:root`. In `accents.css` that is today's `:root, [data-accent="sand"]` block. `contrast.test.ts` reads the presets through `[data-accent="<name>"]`.
- `data-accent` is set in four places: `rootState.ts` (the page root), the swatches in `AppearanceSections.tsx`, and two developer tools: the Storybook toolbar (`.storybook/preview.tsx`, default amber) and the review page's Accent select (`scripts/build-review.mjs`). Only `accents.css` selects on it.
- The tray's waiting dot is a fixed colour in `src-tauri/src/tray.rs` (`DOT`), and the webview's `TrayIcon.tsx` uses `var(--accent)`. Neither reads the chosen accent.
- No palette command, Settings search entry, onboarding step, CLI command or a11y document names the accent picker. `settingsSections.ts`, `appMenu.ts` and `keymap.ts` name only "Appearance".
- `pnpm check:generated` fails whenever `git status --porcelain -- src/generated` prints anything (`scripts/check-generated.mjs`), so it passes only once the regenerated file is committed.
- `store::write` copies the previous file to `settings.json.bak` before it writes (`crates/core/src/store.rs`), so an old `accent` stays in the backup until the save after the one that drops it. The spec doesn't cover backups; that is accepted and goes in the PR's Known limits.
- `getComputedStyle(el).getPropertyValue("--accent")` returns the token's text, `light-dark(…)`, in both themes, because `--accent` is an unregistered custom property. The colour a theme draws is read from an element styled `color: var(--accent)`.

## Review Focus

1. **A `settings.json` from an older build with `"accent":"teal"` or `"#3366cc"`:** the app starts, draws sand, leaves the file alone at start, and the key is gone after the next save of anything. Pinned by `an_old_accent_is_read_past_and_left_out_of_the_next_save` (Task 3) and by the live check (Task 5).
2. **An old accent that was invalid (`"pink"`):** it's no longer a repair. No warning names it, and the other repairs still happen. Pinned by `repair_does_not_name_an_old_accent` (Task 3).
3. **A client that still sends `appearance.accent` in a PUT** (a stale dev webview, a script): it gets 200, the key is ignored and isn't saved, and it is never a 400. Pinned by `settings_put_with_an_old_accent_is_accepted_and_not_saved` (Task 3).
4. **Something left that still sets or selects `data-accent`** (a story, the review page, a swatch): it would do nothing, silently. Pinned by `src/design/accentChoiceGone.test.ts` (Task 2) and the `accents.css` check in `contrast.test.ts` (Task 1).
5. **Sand checked against nothing:** once the presets are gone, the contrast tests could pass with no accent read. Pinned by "the app accent is sand, on :root alone" (Task 1), which asserts sand's exact values before any ratio test runs.

---

### Task 1: sand alone in `accents.css`, and the colour tests read `:root`

**Files:**
- Modify: `src/styles/accents.css`
- Modify: `src/design/contrast.test.ts`
- Modify: `src/design/Colours.stories.tsx`
- Modify: `src/design/cssTokens.test.ts`
- Modify: `src/styles/tokens.css` (two comments, lines 24 and 32)

**Interfaces:**
- Produces: `accents.css` holds only `:root` rules. The first is `--accent` and `--accent-ink` (sand); the second is the derived `--accent-tint` and `--accent-flash`, unchanged.

- [ ] **Step 1: Write the failing test.** In `contrast.test.ts`, delete `ACCENTS` and the `accent(name)` helper. Add `const appAccent = parseTokens(accentsCss, ":root");`, and replace `describe("accents", …)` with:

```ts
describe("the app accent", () => {
  it("is sand, on :root alone: no preset blocks and no data-accent", () => {
    expect(appAccent["--accent"]).toEqual({ light: "oklch(42% 0.028 70)", dark: "oklch(80% 0.045 75)" });
    expect(appAccent["--accent-ink"]).toEqual({ light: "oklch(100% 0 none)", dark: "oklch(0% 0 none)" });
    expect(accentsCss).not.toContain("data-accent");
  });
  for (const theme of THEMES) {
    it(`is at least 3:1 on every surface (${theme})`, () => {
      expect(minOnSurfaces(appAccent["--accent"]![theme], tokens, theme)).toBeGreaterThanOrEqual(3);
    });
    it(`its ink is at least 4.5:1 on it (${theme})`, () => {
      expect(contrast(appAccent["--accent-ink"]![theme], appAccent["--accent"]![theme])).toBeGreaterThanOrEqual(4.5);
    });
  }
});
```

Then rewrite the other blocks that loop over `ACCENTS` so each reads `appAccent` once:
- In `distinctness`, keep ΔE ≥ 15 from `--error`, `--ok` and `--caution` (`the app accent is ΔE ≥ 15 from ${s} (${theme})`) and sand's ΔE ≥ 12 from `--brand-codex`, `--brand-openai` and `--brand-claude` (`the app accent is ΔE ≥ 12 from ${b} (${theme})`, keeping the comment above it). Delete "default amber is ΔE ≥ 18 from Claude orange" and "the :root default is sand", which the first test now covers.
- In "text on the blocked-row tints", name the case `${t} is at least 4.5:1 on the app accent's needs-you tint (${theme}, ${surface})`.
- In "high contrast", rename "every accent is at least 3:1 on every high contrast surface" to "the app accent is at least 3:1 on every high contrast surface", over `appAccent` alone.

- [ ] **Step 2: Run it and see it fail.** Run `pnpm test src/design/contrast.test.ts`. Expected: FAIL on "is sand, on :root alone" (`accentsCss` still contains `data-accent`). The other cases pass, because they read today's `:root` block.

- [ ] **Step 3: Cut `accents.css` to sand.** Delete the `amber` to `slate` blocks, and change the first selector list from `:root,\n[data-accent="sand"]` to `:root`. Replace the two header comments with one:

```css
/*
 * The app accent, sand: the window's own colour where no session is in play. Inside a session's element the accent is that
 * CLI's colour instead (src/components/AgentMark/agent-colors.css). It marks attention only: fills, badges, rings, focus.
 * Sand is a warm dark grey (light) and a beige (dark), warm enough not to read as Codex's neutral grey and low enough in
 * chroma not to read as OpenAI's pale yellow. --accent-ink is the text colour on an accent fill (at least 4.5:1;
 * src/design/contrast.test.ts).
 */
```

The comment above the derived block becomes `/* Derived from the accent. */`.

- [ ] **Step 4: The token comments and the parser test.** In `tokens.css`, line 24 becomes `/* Status. Caution is yellow (dark) and olive (light), and ΔE ≥ 15 from the app accent. */`, and line 32 becomes `/* Claude is a muted terracotta so it stays ΔE ≥ 15 from --error and ≥ 12 from the app accent. */`. In `cssTokens.test.ts`, the fixture's `[data-accent="amber"]` becomes `[data-agent="codex"]` and `[data-accent="blue"]` becomes `[data-agent="claude"]`, with their declarations unchanged, and the three selector strings in the cases change to match (the missing one is `[data-agent="nope"]`).

- [ ] **Step 5: The Colours story.** In `Colours.stories.tsx`, delete `ACCENTS`. `AccentMatrix` reads `parseTokens(accentsCss, ":root")` and draws one row per theme. Its columns are Theme, Chip, "On surfaces (≥ 3)", "Ink on accent (≥ 4.5)", "ΔE error / ok / caution (≥ 15)" and "ΔE Codex / OpenAI / Claude (≥ 12)" (three `Ratio`s with floor 12), with no Accent column. In `Palette`, the group title "Accent (toolbar)" becomes "Accent".

- [ ] **Step 6: Run the tests and the build.** Run `pnpm test src/design`, then `pnpm build`. Expected: both pass. `renderReview.test.tsx` mocks the Colours module, so only the build type-checks `Colours.stories.tsx`.

- [ ] **Step 7: Commit.** `git add src/styles/accents.css src/styles/tokens.css src/design/contrast.test.ts src/design/cssTokens.test.ts src/design/Colours.stories.tsx` then `git commit` with `style(accent): accents.css keeps only sand, and the colour tests check it on :root.`

### Task 2: the page stops choosing an accent

**Files:**
- Delete: `src/features/appearance/customAccent.ts`, `src/features/appearance/customAccent.test.ts`
- Modify: `src/features/appearance/rootState.ts`, `rootState.test.ts`
- Modify: `src/features/appearance/drafts.ts`, `drafts.test.ts`
- Modify: `src/features/appearance/AppearanceSections.tsx`, `AppearanceSections.test.tsx`, `AppearanceSections.css`, `AppearanceSections.css.test.ts`
- Modify: `src/features/appearance/Appearance.stories.tsx`, `Appearance.stories.test.tsx`
- Modify: `src/features/appearance/AppearanceProvider.tsx` (docstring), `settingsSync.test.ts` (fixture)
- Modify: `.storybook/preview.tsx`, `scripts/build-review.mjs`, `src/design/Welcome.stories.tsx`, `src/features/window/ShellPanel.test.tsx` (a test name), `src/styles/tokensOnly.test.ts` (one reason's wording)
- Create: `src/design/accentChoiceGone.test.ts`

**Interfaces:**
- Consumes: Task 1's `accents.css` (sand on `:root`, no `[data-accent]`).
- Produces:
  - `RootState = { attrs: Record<"data-theme" | "data-contrast" | "data-text-size", string | null> }`, with no `style` member. `rootState(a, sys): RootState` and `applyRootState(el, s): void` keep their names, and `applyRootState` writes only the attributes.
  - `Field = "font" | "size"`, `Drafts = { font: string; size: Draft }`, and `draftsOf(a): Drafts` and `refill(prev, a, focused): Drafts`. `commitAccent`, `blurAccent` and `pickPreset` are gone.
  - `ErrorPlace = "theme" | "terminal" | "imported"`. `AppearanceViewState` loses `accent`. `AppearanceHandlers` loses `onAccentText`, `commitAccent`, `blurAccent` and `pickAccent`.
  - `DEFAULT_APPEARANCE` keeps `accent: "sand"` until Task 3, because the generated type still has the field. Nothing reads it.

- [ ] **Step 1: Write the failing tests.**
  - `rootState.test.ts`: delete the three accent cases and add:

    ```ts
    it("sets no accent: the root carries only the theme, contrast and text size", () => {
      const r = rootState(DEFAULT_APPEARANCE, off);
      expect(Object.keys(r.attrs).sort()).toEqual(["data-contrast", "data-text-size", "data-theme"]);
      expect(r).not.toHaveProperty("style");
    });
    ```

  - `drafts.test.ts`: delete the custom-accent describes and every `accent` in the fixtures (`loaded` keeps only `terminal_font`). Delete the `after.accent.text` line in "fields opened before the settings loaded pick up the loaded values", and add `it("holds only the font and the size", () => expect(Object.keys(draftsOf(DEFAULT_APPEARANCE)).sort()).toEqual(["font", "size"]));`.
  - `AppearanceSections.test.tsx`: the `handlers` and `STATE` fixtures keep their accent members for now, since `AppearanceView` reads `state.accent` until Step 3. "holds the app theme, contrast, text size and accent, and nothing of the terminal" becomes "holds the app theme, contrast and text size, no accent, and nothing of the terminal": it checks Theme, High contrast and Text size as now, and also `expect(m).not.toMatch(/Accent|name="accent"|appearance__swatch|Custom colour|data-accent/)`. Delete "marks the current accent" and "custom accent shows the reason and keeps the old value".
  - `AppearanceSections.css.test.ts`: add `it("has no accent swatch or custom colour field left", () => expect(css).not.toMatch(/appearance__(swatch|hex|inline)/));`.
  - Create `src/design/accentChoiceGone.test.ts`. It reads every `.ts`, `.tsx` and `.css` file under `src/` except test files (`*.test.ts`, `*.test.tsx`; `readdirSync(…, { recursive: true })`) and expects none to contain `data-accent` or `dataset.accent`. Test files are skipped because some assert that the attribute is absent. It also reads `.storybook/preview.tsx` and `scripts/build-review.mjs` and expects neither to match `/data-accent|dataset\.accent|globals\.accent|id="accent"/`. The case is named "nothing sets or selects a chosen accent any more: sand is the only app accent".

- [ ] **Step 2: Run them and see them fail.** Run `pnpm test src/features/appearance src/design/accentChoiceGone.test.ts`. Expected: FAIL. The new rootState case finds `data-accent` and `style`, the drafts case finds `accent` and `accentBase`, the section case finds "Accent colour" while the other section cases pass, the CSS case finds `.appearance__swatch`, and the guard names `rootState.ts`, `AppearanceSections.tsx`, `.storybook/preview.tsx` and `scripts/build-review.mjs`.

- [ ] **Step 3: Remove the choice.**
  - Delete `customAccent.ts` and `customAccent.test.ts`.
  - `rootState.ts`: drop the `checkAccent` import, the `data-accent` attribute, the `style` member and its loop in `applyRootState`. The `rootState` docstring reads "The attributes the root element carries for these settings. `null` removes one."
  - `AppearanceSections.test.tsx`: now drop the accent members from `handlers` and `STATE`.
  - `src/styles/tokensOnly.test.ts`: Logo.css's reason ends "artwork, not a themed surface, and the same in every theme." instead of "in every accent".
  - `drafts.ts`: drop the `checkAccent` import, `accent`, `accentBase`, `commitAccent`, `blurAccent` and `pickPreset`. Change the docstrings to match: "The field text for settings that are saved: the font and the size."
  - `AppearanceSections.tsx`: delete `ACCENTS` and the Accent `<Section>`, along with the `ids` and `custom` locals it used. Then remove the members named in Interfaces, `setAccent`, `applyAccent`, `appearance.accent` in the refill effect's dependencies, and the unused `Drafts` import. The two docstrings that list the section's fields end "app theme, contrast and text size".
  - `AppearanceSections.css`: delete `.appearance__swatch`, `.appearance__swatch--custom`, `.appearance__inline` and `.appearance__hex`, and take `.appearance__swatch` out of the forced-colors `forced-color-adjust: none` list.
  - `AppearanceProvider.tsx`: the docstring reads "keeps the root element's theme, contrast and text size in step with them."
  - `settingsSync.test.ts`: `const teal = { ...DEFAULT_APPEARANCE, accent: "teal" }` becomes `const large = { ...DEFAULT_APPEARANCE, text_size: "large" as const }`, and every `teal` in the file becomes `large`.
  - `Appearance.stories.tsx`: drop the accent members from `handlers` and `state`, and delete `CustomAccentRefused`. Default's label is "The defaults: follow the system, the bundled font, no Windows Terminal found". In `Appearance.stories.test.tsx`, the story count is 2, and the second case is "show Windows Terminal's font and schemes", without the refused-accent line.
  - `.storybook/preview.tsx`: delete `ACCENTS`, the `accent` global type and `initialGlobals.accent`. The decorator becomes `withMotion` and only sets or clears `data-motion`.
  - `scripts/build-review.mjs`: delete `ACCENTS`, `options`, the Accent `<label>` in the review bar and the two lines of script that wire it. `src/styles/accents.css` stays in its CSS list.
  - `Welcome.stories.tsx`: "Use the toolbar to switch theme, accent and motion." becomes "Use the toolbar to switch theme and motion."
  - `ShellPanel.test.tsx`: the case named "is not an agent: no vendor, so the user's own accent applies, …" becomes "is not an agent: no vendor, so the app accent applies, …", with its body unchanged.

- [ ] **Step 4: Run the tests and the build.** Run `pnpm test src/features/appearance src/design src/features/window`, then `pnpm build`. Expected: both pass. Then `git grep -nE "data-accent|dataset\.accent|checkAccent|customAccent|pickAccent|commitAccent|blurAccent|pickPreset" -- src .storybook scripts ':!*.test.ts' ':!*.test.tsx'` prints nothing.

- [ ] **Step 5: Run the whole frontend suite.** Run `pnpm test`. Expected: every file passes.

- [ ] **Step 6: Commit.** Stage the files above (`git add -A src/features/appearance` covers the deletions) and commit with `feat(appearance): Appearance no longer offers an accent, and the page root carries none.`

### Task 3: the core and the protocol forget the accent

**Files:**
- Modify: `crates/protocol/src/api.rs` (`Appearance`, `impl Default for Appearance`)
- Regenerate: `src/generated/Appearance.ts`
- Modify: `crates/core/src/settings.rs` (`ACCENT_PRESETS`, `check_accent`, `validate`, `repair`, tests)
- Modify: `crates/core/src/server/settings.rs` (three tests)
- Modify: `crates/core/tests/server.rs` (two tests, one new)
- Modify: `crates/core/src/paths.rs:44` (doc comment)
- Modify: `src/features/appearance/rootState.ts` (`DEFAULT_APPEARANCE`)

**Interfaces:**
- Consumes: Task 2, so nothing in `src/` reads `Appearance.accent` any more.
- Produces: `Appearance { theme, contrast, text_size, terminal_theme, terminal_font, contrast_protection, imported_themes }`. `validate` and `repair` keep their signatures, and `repair` can name only "terminal font size", "imported themes" and "terminal theme".

- [ ] **Step 1: Write the failing tests.** In `crates/core/src/settings.rs`'s tests:

```rust
#[test]
fn an_old_accent_is_read_past_and_left_out_of_the_next_save() {
    let home = tempfile::tempdir().unwrap();
    let path = crate::paths::settings_file(home.path());
    let old = r#"{"version":1,"appearance":{"theme":"dark","accent":"teal"}}"#;
    std::fs::write(&path, old).unwrap();
    let s = load(home.path());
    assert_eq!(s.appearance.theme, marshell_protocol::api::AppTheme::Dark);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), old, "load must not rewrite the file");
    save(home.path(), &s).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["appearance"]["theme"], "dark");
    assert!(saved["appearance"].get("accent").is_none(), "{saved}");
}

#[test]
fn repair_does_not_name_an_old_accent() {
    let mut a: Appearance = serde_json::from_str(r#"{"accent":"pink","terminal_font":{"size":40}}"#).unwrap();
    assert_eq!(repair(&mut a), vec!["terminal font size"]);
    assert_eq!(a.terminal_font.size, 13);
}
```

In `crates/core/tests/server.rs`:

```rust
#[tokio::test]
async fn settings_put_with_an_old_accent_is_accepted_and_not_saved() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path());
    let body = serde_json::json!({ "appearance": { "theme": "dark", "accent": "pink" } });
    let res = app.clone().oneshot(settings_request("PUT", Some(body))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let after = json_body(app.oneshot(settings_request("GET", None)).await.unwrap()).await;
    assert_eq!(after["appearance"]["theme"], "dark");
    assert!(after["appearance"].get("accent").is_none());
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.path().join("settings.json")).unwrap()).unwrap();
    assert!(saved["appearance"].get("accent").is_none());
}
```

- [ ] **Step 2: Run them and see them fail.** Run `cargo test -p marshell-core -j 4 old_accent`. Expected: all three FAIL. The save still writes `"accent":"teal"`, `repair` returns `["accent", "terminal font size"]`, and the PUT answers 400.

- [ ] **Step 3: Remove the field and its checks.**
  - `api.rs`: delete `accent` and its doc comment from `Appearance`, and `accent: "sand".into()` from its `Default`.
  - `settings.rs`: delete `ACCENT_PRESETS`, `check_accent`, its call in `validate` and its entry in `repair`. `is_hex_colour` stays, because the imported themes use it.
  - `paths.rs:44`: the comment reads "how the app looks (theme, terminal colours and font)."
  - `rootState.ts`: delete `accent: "sand",` from `DEFAULT_APPEARANCE`.
  - Change the existing tests that set or read the accent:
    - `settings.rs`: `missing_file_gives_defaults` loses its accent assert. `save_then_load_round_trips` sets `s.appearance.theme = marshell_protocol::api::AppTheme::Dark` instead of the hex. `missing_fields_take_defaults` writes `{"appearance":{"theme":"dark"}}` and asserts the theme. `load_replaces_invalid_values_with_defaults_and_keeps_the_rest` loses `"accent":"pink"` from its JSON and its accent assert. `a_notifications_block_of_the_wrong_type_falls_back_alone` writes `"theme":"dark"` and asserts it. `validate_accepts_the_defaults_and_a_custom_accent` is renamed `validate_accepts_the_defaults_and_an_imported_terminal_theme` and loses the hex line. `validate_rejects_bad_values` loses its two accent lines.
    - `server/settings.rs`: the two tests that set `accent = "teal"` (`put_writes_the_file_only_while_it_holds_the_settings_lock` and `put_does_not_look_at_an_unchanged_default_folder`) set `theme = AppTheme::Dark` and assert `theme` (add `AppTheme` to the test module's `use`).
    - `tests/server.rs`: `settings_put_invalid_is_400` sends `terminal_font.size = 40` and expects the body to contain "font size", with its first assert reading `terminal_font.size == 13`. `settings_put_then_get_returns_it` drops `accent` from the body and both asserts.

- [ ] **Step 4: Regenerate and run.** Run, one at a time: `cargo test -p marshell-protocol -j 4`, then `cargo test -p marshell-core -j 4`, then `pnpm build`, then `pnpm test`. Expected: all pass, and `git status --short src/generated` lists only `Appearance.ts`, which no longer has `accent`.

- [ ] **Step 5: Lint.** Run `cargo fmt --all`, then `cargo clippy --workspace --all-targets -j 4 -- -D warnings`. Expected: no change from fmt, no warnings. `git grep -nE "ACCENT_PRESETS|check_accent|appearance\.accent" -- crates src` prints nothing.

- [ ] **Step 6: Commit.** `git add crates src/generated/Appearance.ts src/features/appearance/rootState.ts`, then commit with `feat(settings): the core and the protocol drop the accent, and an old saved one is read past and left out of the next save.`

- [ ] **Step 7: Check the generated types.** Run `pnpm check:generated`. It passes only with `src/generated` committed. Expected: it exits 0 and prints nothing.

### Task 4: the docs describe an app with no accent choice

**Files:**
- Modify: `docs/PLAN.md` (lines 87, 316, 319, 321, 430, 562, 782, 797, 1217, and §9's 6z entry)
- Modify: `docs/BRIEF.md` (one amendment line, and a note on the Phase 0 line)

This task is done in the main thread. The copy is below; match each line by its text, because line numbers move.

- [ ] **Step 1: Rewrite what describes today's app.**
  - Line 316, the accent rule. Replace the sentence "Where no session is in play (onboarding, settings, doctor) the user's chosen accent applies; … (9 accents: sand, amber, blue, indigo, violet, magenta, cyan, teal, slate)." with: "Where no session is in play (onboarding, settings, the Doctor, History, Instructions) the app accent applies. It is sand, a warm dark grey in light mode and a beige in dark mode, kept apart from Codex's neutral grey and OpenAI's pale yellow, and it can't be changed: slice 6z removed the accent picker (decided 2026-11-27)." Delete the last sentence, "This rule stands as written until slice 6z merges … and rewrites this rule then." Everything else in the bullet stays: the bold lead, the sentence about the session's elements, and the sentence on vendor colours and `accentFollowsVendor.test.ts`.
  - Line 319: "Caution is yellow (dark) or olive (light), and `contrast.test.ts` checks AA and CIEDE2000 distance for every pair, including the app accent against each status colour."
  - Line 321, the "Custom accent hex" bullet, becomes: "- No accent choice: slice 6z removed the nine presets and the custom hex (decided 2026-11-27). An `accent` that an older build saved is ignored, then left out of the next save."
  - Line 430: `# Appearance dialog, provider, theme import parsers`.
  - Line 562: `# app settings (theme, notif rules, …)`, with the rest of the list unchanged.
- [ ] **Step 2: Note the history passages.** Their text stays, and each gets a note:
  - Line 87, the tokens row: after "AA matrix for the 8 accents in both themes.", add "Slice 6z (2026-11-27) removed the accent choice; only sand is left."
  - Line 782, phase 2's scope ("themes, accent, high-contrast and large-text modes, …"): at the end, add "(slice 6z removed the accent choice)".
  - Line 797, phase 2's "What shipped": at the end, add "Slice 6z (2026-11-27) removed the presets and the custom accent; the app accent is sand."
  - Line 1217, 6e1's Appearance line: at the end, add "(slice 6z removed the accent)".
- [ ] **Step 3: §9 and the brief.** After decision 3 in "Slice 6z decisions (2026-11-27)", add "The slice's plan is `docs/superpowers/plans/2026-11-27-phase-6z-remove-accent-picker.md`." Then add a "Settled while it was built (not asked):" list with the rulings below, plus whatever the live check and the review add. In `docs/BRIEF.md`, after the "Departures board (slice 4c, 2026-10-23)" amendment, add: "- **Accent (slice 6z, 2026-11-27).** The user-chosen accent colour (§6) is removed. The app accent is sand, and a session's elements keep their CLI's colour. `docs/PLAN.md` §9 has the decisions." At the end of the Phase 0 line "The default accent ("needs you" attention colour) is a signal amber, …", add "(Later sand; since slice 6z, sand is the only app accent.)"
- [ ] **Step 4: Commit.** Commit with `docs(plan): describe the app with sand as its only accent, and note slice 6z where earlier phases shipped the picker.`

Rulings made while planning (they go in Step 3's list):
- The Storybook toolbar's Accent switch and the review page's Accent select go. They only switched presets that no longer exist, and Storybook's default was amber, so stories now show sand like the app.
- The Colours story's accent matrix stays, for sand alone. It shows the same checks the tests make.
- The `tokens.css` comments that measured against amber now name the app accent. Amber's ΔE ≥ 18 from Claude goes with amber, and sand's ΔE ≥ 12 from Claude, Codex and OpenAI stays.
- The parser test's fixture takes `[data-agent=…]` selectors, so no file under `src/` other than a test mentions `data-accent`. A guard test keeps it that way.
- `agent-colors.css` keeps its comment that calls the vendor colour the "default accent", because the Global Constraints leave that file alone.
- An old `accent` stays in `settings.json.bak` until the save after the one that drops it, since `store::write` backs up the previous file. This is accepted and goes in the PR's Known limits.
- One BRIEF amendment records the removal, since §6 of the brief asks for the picker.

### Task 5: live check

Run the `marshell-live-check` skill. No agent session is needed. Don't bring the window to the front. Drive the webview over CDP with `browser_evaluate`, which returns small JSON values.

Setup: `live-check.ps1 start` refuses a folder that isn't empty, so the seed goes in between two runs. Run `start -Scratch <new empty literal path>` (a new folder in the session's scratchpad, as the skill says), wait for the window, then `stop -Scratch <same>`. Write `{"version":1,"onboarded":true,"appearance":{"theme":"light","accent":"teal"}}` to `<scratch>\mh\settings.json`, replacing what the first run wrote. `onboarded` keeps onboarding from opening, because finishing it would save the settings and drop the key before Step 3. Then run `restart -Scratch <same>`.

To read the drawn accent, add a probe once with `browser_evaluate`: a `span` styled `color: var(--accent)` appended to `document.body`. Read its computed `color`, then remove it. Sand is `oklch(0.42 0.028 70)` in light and `oklch(0.8 0.045 75)` in dark; WebView2 may report either one in sRGB, so the first reading fixes the form. Steps 1, 3 and 4 compare against these. `getPropertyValue("--accent")` gives only the token's text, `light-dark(oklch(42% 0.028 70), oklch(80% 0.045 75))`.

- [ ] **Step 1: The old accent is read past.** `document.documentElement` has no `data-accent` attribute, and `style.getPropertyValue("--accent")` on it is empty. `getComputedStyle(document.documentElement).getPropertyValue("--accent").trim()` is exactly sand's `light-dark(…)` text. The probe draws sand's light colour. `settings.json` still holds the seeded text, byte for byte.
- [ ] **Step 2: Appearance.** Open Settings → Appearance from the palette's "Settings: Appearance…". The section's legends are exactly Theme, High contrast and Text size, with no "Accent" text. Tab moves through it to Text size and then out, and the arrows move within each radio group.
- [ ] **Step 3: The next save drops the key.** Pick Dark with the keyboard. `settings.json` now has `"theme":"dark"` and no `accent` key. `settings.json.bak` still holds the seed, `accent` included, which is the accepted limit. The probe's colour is sand in dark.
- [ ] **Step 4: Sessions keep their colour.** Open the shell panel. A probe appended inside it draws sand, and the window element has no `data-agent`. A session's row still takes its vendor's colour, which `accentFollowsVendor.test.ts` already covers; check it live only if a plain shell session is open anyway.
- [ ] **Step 5: Forced colors.** Emulate `forced-colors: active` and take one screenshot of the Appearance section to judge: the chips and their selected mark are readable, and nothing is left where the swatches were.
- [ ] **Step 6: Write it down and stop.** Write the findings in `docs/research/2026-11-27-remove-accent-6z-live-check.md` and commit it with `docs(research): slice 6z's live check.`. Stop only the PIDs the check started, after listing the process tree. Ask before deleting the scratch folder.

## Exit criteria

- Appearance shows Theme, High contrast and Text size, and no accent. Tasks 2 and 5 prove it.
- Every page with no session in play draws sand, and sessions keep their CLI's colour. Tasks 1, 2 and 5 prove it.
- An old `accent` is ignored when the file is read, is left out of the next save, and is never a repair or a 400. Tasks 3 and 5 prove it.
- Sand is checked against every surface, in both themes and in high contrast. Task 1 proves it.
- What CI runs passes: `cargo fmt --all --check`, clippy with `-D warnings`, `cargo test --workspace -j 4`, `pnpm test`, `pnpm check:generated`, `pnpm build` and `pnpm tauri build --no-bundle`. The `marshell-ship` skill runs them.
