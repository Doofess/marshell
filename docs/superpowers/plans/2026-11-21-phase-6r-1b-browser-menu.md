# Slice 6r-1b: the Browser menu's five new things, and issue #161: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task (the user's default). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Browser panel's ⋯ menu gains Save screenshot, Save full-page screenshot, the viewport sizes, the cookie rule, the Ctrl+click switch and the auto-open of HTML files an agent writes, with the same settings on the Settings page; issue #161's four changes and six faults are fixed first.

**Architecture:** Three settings live in `settings.json` under `browser` (core, protocol, generated types, `SettingsSync`); a tab's viewport lives on the tab in `layout.json`. The shell places a page by a pure `fit` of its slot and the tab's viewport (centred when narrower, emulated and scaled when wider), takes a picture through the page's in-process DevTools protocol, and makes private pages under the "Never" cookie rule. The core reads each CLI's PostToolUse hook for the file it wrote and queues an open exactly as `marshell open` does; the app's existing `takeOpens` path shows it.

**Tech Stack:** Rust (`marshell-protocol`, `marshell-core`, `marshell-app` with `webview2-com`), React 19 and TypeScript, Vitest (node, `renderToStaticMarkup`), ts-rs.

**Spec:** `docs/PLAN.md` §9, "Slice 6r decisions" 11 and "Slice 6r-1b decisions" 1 to 29. Facts: `docs/spikes/S14-local-html-page.md`. Issue #161 (`gh issue view 161`).

**Who does what:** the main thread writes Tasks 8, 10 and 19 (the page's COM code, the capability file, the live check). Every other task goes to a `worker` with the task's text, this header and the exact test command; `effort: high` is marked on the task. Each worker is told: touch only the task's files, never `git checkout`, `git restore` or `git stash`, and report the test command's last lines.

## Global Constraints

- Copy is exact, in sentence case, with no "!" and no "successfully". Quoted strings in this plan are the strings in the product and in the tests.
- Every new control works with the keyboard alone and in forced colors (system colors only in a `forced-colors` rule). No boxed outlines; colors come from the tokens in `src/styles/`. Read the `good-css` skill before writing declarations.
- `Announcer` is the only live region. Nothing new announces through anything else.
- Vendor-specific code stays in its adapter: each CLI's "which file did this event write" lives in `crates/core/src/agents/<cli>/writes.rs`.
- `src/generated/` is never edited by hand: `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated`.
- The shell (`src-tauri/`) stays thin: no command takes a DevTools method name, a JSON parameter string or a capture size from the app. The only capability change is two lines, `allow-browser-screenshot` and `allow-browser-cookies` (decision 21). The CSP and the window settings don't change.
- No new crate or package. `serde_json` (in the workspace already) becomes a real dependency of `marshell-app` (decision 21).
- Viewport widths, everywhere: phone 390, tablet 768, laptop 1024, desktop 1440. Ids: `phone`, `tablet`, `laptop`, `desktop`. A value that is none of these means Responsive.
- Cookie rules, everywhere: `always` (the default), `until-quit`, `never`.
- A picture is at most 16,384 device pixels on a side (S14).
- Rust: `cargo fmt --all` before each Rust commit; `cargo test ... -j 4`; clippy with `-D warnings` must stay clean. A hook reformats edited Rust files: read the file again after it says so.
- Commit messages: `type(scope): what changed, as a sentence.` One commit per task unless a task says otherwise.
- Pages exist on Windows only today. Where they don't, the screenshot rows and the cookie choice are off with "Not available on this system yet"; the viewport still works for an HTML file.

## Review Focus

1. **A fixed viewport across a resize and a tab switch.** A panel dragged from wider than the viewport to narrower switches the page from centred to scaled, and a switch from a fixed tab to a Responsive one must not put the first tab's size on the second. Expected: each page keeps its own size, with no wrong-size frame left behind. Pinned by Task 8's `fit` boundary tests and `viewport_is_kept_per_page`.
2. **A screenshot asked while a menu or the palette still covers the page.** A covered page is hidden, and a hidden page never answers a capture (S14). Expected: the capture waits until the page is back, and a page that isn't on screen gets a line, not a 20 s wait. Pinned by Task 12's `waits_for_the_cover_to_lift` and Task 10's `a_page_that_is_not_shown_is_refused`.
3. **The cookie rule is unknown when the first tab loads** (the settings arrive late, or can't be read). Expected: no page is made before the rule is told, and unreadable settings mean private pages for that run, never a kept cookie under "Never". Pinned by Task 13's gate tests and Task 10's `open_waits_for_the_rule`.
4. **Paths an agent's hook reports in odd shapes:** a relative path from Codex, a WSL path, `REPORT.HTML`, a file deleted by the same patch, a path outside the project, eighty files in one turn. Expected: each real `.html` or `.htm` file opens once, nothing else does, and the queue's limit drops the rest without an error. Pinned by Task 15's `html_targets` tests.
5. **Hand-edited files:** `"cookies": "nevr"` in `settings.json`, `"viewport": 390` on a tab in `layout.json`. Expected: the other settings and the tab survive; the bad value reads as the default. Pinned by Task 4's tests.
6. **A page taller or wider than one picture can hold, on a 150% display.** Expected: the clip is cut to the page's measured size and to 16,384 device pixels, and the app says the picture was cut. Pinned by Task 10's `shot_clip` tests.

---

## File structure

| File | Responsibility |
| --- | --- |
| `src/lib/navigation/names.ts` (new) | The spoken name of a title-bar target. |
| `src/features/browser/viewport.ts` (new) | The sizes, a tab's size, the fit of a size in a slot, labels. |
| `src/features/browser/ViewportChip.tsx`, `.css` (new) | The chip at the end of the address bar and of an HTML file's toolbar. |
| `src/features/browser/screenshot.ts` (new) | What a picture is named from, the save flow, its announcements. |
| `src/features/browser/autoOpen.ts` (new) | Why a session can't open an agent's HTML files; the Settings list. |
| `src/features/browser/browserSettings.tsx` (new) | The `browser` settings as a store for React, and their setter. |
| `src/features/browser/BrowserSection.tsx` (new) | The Settings page's Browser section. |
| `src/lib/api/screenshots.ts` (new) | `POST /v1/screenshots`. |
| `crates/core/src/server/screenshots.rs` (new) | Writes a PNG into Downloads. |
| `crates/core/src/server/auto_open.rs` (new) | From a hook event to queued opens. |
| `crates/core/src/agents/<cli>/writes.rs` (new, five) | Which files one CLI's event wrote. |
| `src-tauri/src/browser/rules.rs`, `com.rs`, `mod.rs` | `fit`, `shot_clip`, the cookie rule; the COM calls; the commands. |

---

### Task 1: Issue #161's four changes (decisions 1 to 4)

**Files:**
- Create: `src/lib/navigation/names.ts`, `src/lib/navigation/names.test.ts`
- Modify: `src/lib/navigation/machine.ts:64-85` (`commit`, `describeTarget`), `src/lib/navigation/fixtures.ts:8-10` (the top bar gains `panel-browser` and `panel-files`), `src/features/sessions/useNavigation.ts:54-57`, `src/features/pane/SidePane.tsx:298-313`, `src/features/browser/useBrowserPages.ts:38-51,124-132`
- Test: `src/lib/navigation/machine.test.ts`, `src/features/pane/SidePane.test.tsx` (lines 71, 95-103 and 138 match the old Fill label and change with it), `src/features/browser/useBrowserPages.test.ts`

**Interfaces:**
- Produces: `topName(id: string): string` in `names.ts`. It and `TOP_NAMES` exist today in `useNavigation.ts:54-57` and move, unchanged (palette "Command palette", sidebar "Sidebar", launcher "New session", `panel-browser` "Browser", `panel-files` "Files"; an unknown id is returned as it is); `useNavigation.ts` exports it on from there, so its importers don't change. `popupOutcome(pane: PaneView, session: string, url: string, onScreen: boolean, own: readonly (readonly [string, number])[]): { actions: PaneAction[]; say: string | null }` in `useBrowserPages.ts`.

- [ ] **Step 1: Write the failing tests**

```ts
// names.test.ts
it("names every title-bar target", () => {
  expect(["palette", "sidebar", "launcher", "panel-browser", "panel-files"].map(topName)).toEqual(["Command palette", "Sidebar", "New session", "Browser", "Files"]);
  expect(topName("brand")).toBe("brand");
});
// machine.test.ts. `choose` commits a target and says "Activated …"; only a cursor move says "Cursor on …".
it("the cursor speaks a title-bar target's name, not its id", () => {
  // For each of palette, sidebar, panel-browser and panel-files: move the cursor onto the top-bar target with the
  // arrow events the tests at lines 222-240 use, and expect announced(...) to be [`Cursor on ${name}`]:
  // "Cursor on Command palette", "Cursor on Sidebar", "Cursor on Browser", "Cursor on Files".
});
it("activating a title-bar target speaks its name", () => {
  for (const [id, name] of [["palette", "Command palette"], ["sidebar", "Sidebar"], ["panel-browser", "Browser"], ["panel-files", "Files"]] as const)
    expect(announced(run(at(0), [{ type: "choose", target: { kind: "top", id } }], GRID_CONTEXT).effects)).toEqual([`Activated ${name}`]);
});
// SidePane.test.tsx. Every tab's close button draws the × path too (SidePane.tsx:258), so the Hide assert reads the Hide button alone.
it("names each Fill button after its panel and draws Hide as a bar", () => {
  expect(html({ panel: "browser" })).toContain('aria-label="Fill the side pane with Browser"');
  expect(html({ panel: "files" })).toContain('title="Fill the side pane with Files"');
  expect(btn(html({ panel: "browser", fill: { on: true } }), "Fill the side pane with Browser")).toContain('title="Show the other panels"');
  const hide = btn(html({ panel: "browser" }), "Hide Browser");
  expect(hide).toContain('d="M3 6h6"');
  expect(hide).not.toContain("M3 3l6 6M9 3 3 9");
});
// useBrowserPages.test.ts
it("says the limit line for a popup in a session with 32 tabs", () => {
  expect(popupOutcome(fullPane, "s1", "https://example.com/", true, [])).toEqual({ actions: [], say: TAB_LIMIT });
});
it("says the refusal for an address the shell would not open", () => {
  expect(popupOutcome(pane, "s1", "file:///c:/x.html", true, []).say).toBe(PAGE_REFUSED);
});
```

`pane` and `fullPane` stand for the file's own fixtures: read `useBrowserPages.test.ts` first, use its real builders, and build the full one with `PANE_LIMITS.tabs` tabs as the `popupNews` test near line 114 does. Match the `html(...)` and `btn(...)` helpers' real props in `SidePane.test.tsx`, and write the cursor test's moves out in full.

- [ ] **Step 2: Run them and see them fail**

Run: `pnpm test src/lib/navigation src/features/pane/SidePane.test.tsx src/features/browser/useBrowserPages.test.ts`
Expected: FAIL (`topName` is not exported; "Cursor on palette"; `aria-label="Fill the side pane"`; `popupOutcome` is not exported).

- [ ] **Step 3: Implement**

`describeTarget`'s `top` branch says `Cursor on ${topName(t.id)}` and `commit`'s says `Activated ${topName(t.id)}`; `useNavigation.ts` imports `topName`, exports it on, and drops its own table. The existing tests that match the old Fill label take the new one. The Fill button's `aria-label` is `Fill the side pane with ${PANEL_LABEL[panel]}` and its `title` is the disabled reason, else "Show the other panels" while filling, else the same text as the label. The Hide button draws `icon("M3 6h6")`; its name and tooltip don't change. `popupOutcome` wraps `popupActions`: a session at `PANE_LIMITS.tabs` gives `{ actions: [], say: TAB_LIMIT }`, no actions for another reason gives `PAGE_REFUSED`, else `say: null`; the handler says `say` when it is not null.

- [ ] **Step 4: Run the tests and see them pass**

Run: the Step 2 command. Expected: PASS.

- [ ] **Step 5: Commit**

`fix(pane): the cursor speaks title-bar names, Hide is a bar, each Fill button names its panel, and a popup at 32 tabs says the limit.`

### Task 2: Issue #161's faults in the app (1, 3, 4, 5, 6)

**Files:**
- Modify: `src/features/pane/paneState.ts:434-447`, `src/features/pane/stackLayout.ts:93-100`, `src/features/preview/frameKeys.ts`, `src/features/preview/HtmlView.tsx:63-72`, `src/features/browser/useBrowserPages.ts:124-132`
- Test: `src/features/pane/paneState.test.ts`, `src/features/pane/stackLayout.test.ts`, `src/features/preview/frameKeys.test.ts`, `src/features/browser/useBrowserPages.test.ts`

**Interfaces:**
- Produces: `trackHeld(e: FrameKey, held: Set<string>): boolean` in `frameKeys.ts` (true when the key goes on to the app; it adds a forwarded keydown's `code` to `held` and removes a keyup's). `FrameKey` gains `getModifierState?: (key: string) => boolean`. `popupFold(pane: PaneView, actions: readonly PaneAction[]): PaneView` in `useBrowserPages.ts`. `MIN_PAIR = 100` in `stackLayout.ts`.

- [ ] **Step 1: Write the failing tests**

```ts
// paneState.test.ts
it("a divider move with no stack saved writes the stack it merges into", () => {
  const next = paneReducer({ ...empty, panels: undefined }, { type: "weights", weights: { browser: 3, files: 1 } });
  expect(next.panels?.weights).toMatchObject({ browser: 3, files: 1 });
  expect(next.panels?.shown).toEqual(stackOf({ ...empty, panels: undefined }, null).shown);
});
// stackLayout.test.ts
it("a tiny weight sum still takes a divider move", () => {
  const w = pairWeights({ browser: 1, files: 1 }, "browser", "files", 300, 100);
  expect(w.browser! / w.files!).toBeCloseTo(3, 1);
});
it("scales the weights it does not move by the same factor", () => {
  // `Weights` knows only browser and files today: cast the literal as the test at paneState.test.ts:883-893 does.
  const w = pairWeights({ browser: 1, files: 1, changes: 2 } as Weights, "browser", "files", 100, 100) as Record<string, number>;
  expect(w.changes! / (w.browser! + w.files!)).toBeCloseTo(1, 5);
});
// frameKeys.test.ts
it("does not forward a chord typed with AltGr", () => {
  expect(forwardsToApp(key({ ctrlKey: true, altKey: true, key: "@", getModifierState: (k) => k === "AltGraph" }), new Set())).toBe(false);
});
it("forgets a held key when told the keyboard left", () => {
  const held = new Set<string>();
  expect(trackHeld(key({ type: "keydown", ctrlKey: true, code: "KeyK" }), held)).toBe(true);
  held.clear(); // what HtmlView does on the frame's blur
  expect(trackHeld(key({ type: "keyup", code: "KeyK", key: "k" }), held)).toBe(false);
});
// useBrowserPages.test.ts
it("a second popup in the same tick sees the first one's tab", () => {
  const first = popupOutcome(pane, "s1", "https://a.example/", false, []).actions;
  const folded = popupFold(pane, first);
  expect(popupNews(popupOutcome(folded, "s1", "https://b.example/", false, []).actions, folded)).toEqual(["s1"]);
  expect(tabsOf(folded, "s1").tabs.length).toBe(tabsOf(pane, "s1").tabs.length + 1);
});
```

- [ ] **Step 2: Run them and see them fail**

Run: `pnpm test src/features/pane/paneState.test.ts src/features/pane/stackLayout.test.ts src/features/preview/frameKeys.test.ts src/features/browser/useBrowserPages.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement**

- The test code above names fixtures loosely (`empty`, `pane`, `key(...)`, `tabsOf`): read each test file first and use its real builders and constants (`paneState.test.ts` has `EMPTY`-style constants of its own).
- `mergeWeights`: with no `panels`, merge into `stackOf(s, null)` and save it, where it returned `s`. Its "nothing changed" guard stays: `paneState.test.ts:888` expects the same object back for an unchanged merge.
- `pairWeights`: when the pair's sum is under `MIN_PAIR`, multiply every weight in `weights` by `Math.ceil(MIN_PAIR / sum)` first (capped so none passes `MAX_WEIGHT`), then go on as now.
- `forwardsToApp`: a keydown with `e.getModifierState?.("AltGraph") === true` is not forwarded.
- `trackHeld` holds `pass`'s bookkeeping; `HtmlView` calls it and adds `win.addEventListener("blur", () => held.clear())`.
- The popup handler reads `paneRef.current`, and after sending its actions writes `paneRef.current = popupFold(pane, actions)`, so the next popup before a render folds onto it. `popupFold` reduces the actions through `paneReducer`.

- [ ] **Step 4: Run the tests and see them pass**

Run: the Step 2 command. Expected: PASS.

- [ ] **Step 5: Commit**

`fix(pane): a divider move with no stack saved is kept, a tiny weight sum moves, AltGr chords and stale held keys stay in an HTML file, and two popups in a tick fold in order.`

### Task 3: One unreadable value in `panels` drops only itself (#161 fault 2)

**Files:**
- Modify: `crates/core/src/layout.rs:45-88` (`lenient_pane`)
- Test: the `mod tests` of the same file, beside `a_stack_of_the_wrong_type_loses_only_itself` (line 1420)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_bad_weight_or_fill_loses_only_itself() {
    let doc = read_layout(json!({"pane": {"panels": {"shown": ["browser", "files"], "weights": {"files": -5, "browser": 3}, "filled": 7}}}));
    let panels = doc.view.pane.panels.expect("the stack is kept");
    assert_eq!(panels.shown, ["browser", "files"]);
    assert_eq!(panels.weights, BTreeMap::from([("browser".to_string(), 3)]));
    assert_eq!(panels.filled, None);
}
#[test]
fn a_stack_whose_shown_cannot_be_read_is_dropped() {
    let doc = read_layout(json!({"pane": {"panels": {"shown": "browser", "weights": {"browser": 3}}}}));
    assert_eq!(doc.view.pane.panels, None);
}
```

`read_layout` stands for whatever the neighbouring tests use to load a document from JSON; use that helper and its real field path to `panels`.

- [ ] **Step 2: Run and see it fail**

Run: `cargo test -p marshell-core -j 4 a_bad_weight_or_fill`
Expected: FAIL (`panels` is `None`).

- [ ] **Step 3: Implement**

In `lenient_pane`, when `panels` doesn't parse as `Panels`: if `shown` is not an array of strings, remove `panels`; otherwise remove from `weights` every entry that isn't a `u32` (and `weights` itself when it isn't an object), remove `filled` when it isn't a string, and keep the rest.

The existing test `a_stack_of_the_wrong_type_loses_only_itself` (line 1420) holds two cases this changes, `{"shown":["files"],"weights":{"files":1.5}}` and `{"shown":["files"],"filled":3}`, and expects no stack for both. Rewrite those two cases to the new rule (the stack is kept with `shown == ["files"]`, without the bad weight, without the fill); its other cases stay.

- [ ] **Step 4: Run and see it pass**

Run: `cargo test -p marshell-core -j 4 layout::`
Expected: PASS, with every other `layout` test unchanged.

- [ ] **Step 5: Commit**

`fix(layout): an unreadable weight or fill in the saved stack is dropped alone, and the stack is kept.`

### Task 4: The settings, a tab's viewport and an open's `auto`, in the protocol and the core

**Files:**
- Modify: `crates/protocol/src/api.rs` (`Settings` at 1230, `PaneTab` at 439, `PaneOpen` at 474), `crates/core/src/settings.rs` (`FileDoc`, `load`, `save`), `crates/core/src/layout.rs` (`lenient_session` at 91, `repair_pane` at 247, `validate_pane` at 407), `crates/core/src/server/opens.rs` (`push` at 69 and its callers), `src/features/pane/paneRules.ts` (`paneProblem`, the app's copy of `validate_pane`, takes the same viewport rule, with a test beside its others), `src/features/appearance/settingsSync.ts` (`save` at 129 and `writeFlags`: both full `Settings` payloads carry `browser` from the last read, so the build passes; Task 5 adds the getter and the setter), `src/features/appearance/settingsSync.test.ts` (the exact payload asserts at 31, 41 and 44 gain `browser`), `src/features/pane/opens.test.ts:14` (`req` gains `auto: false`)
- Generated: `src/generated/BrowserSettings.ts`, `CookieRule.ts`, `Settings.ts`, `PaneTab.ts`, `PaneOpen.ts`
- Test: the `mod tests` of `settings.rs`, `layout.rs` and `opens.rs`

**Interfaces:**
- Produces (Rust):

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum CookieRule { #[default] Always, UntilQuit, Never }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct BrowserSettings { pub terminal_links: bool, pub open_agent_html: bool, pub cookies: CookieRule }
// Default: terminal_links true, open_agent_html true, cookies Always

// Settings gains:   pub browser: BrowserSettings,
// PaneTab gains:    #[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)] pub viewport: Option<String>,
// PaneOpen gains:   #[serde(default)] pub auto: bool,   // true for an open the core queued from a hook
pub fn is_viewport(id: &str) -> bool   // layout.rs, pub(crate): phone, tablet, laptop, desktop
// Opens::push(&self, tab_id: &str, kind: &str, target: String, by: String, auto: bool)
```

- Produces (TS, generated): `BrowserSettings = { terminal_links: boolean, open_agent_html: boolean, cookies: CookieRule }`, `CookieRule = "always" | "until-quit" | "never"`, `PaneTab.viewport?: string`, `PaneOpen.auto: boolean`.

- [ ] **Step 1: Write the failing tests**

```rust
// settings.rs
#[test]
fn browser_settings_default_to_links_here_auto_open_and_cookies_kept() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(load(home.path()).browser, BrowserSettings { terminal_links: true, open_agent_html: true, cookies: CookieRule::Always });
}
#[test]
fn browser_settings_round_trip() {
    let home = tempfile::tempdir().unwrap();
    let s = Settings { browser: BrowserSettings { terminal_links: false, open_agent_html: false, cookies: CookieRule::UntilQuit }, ..Settings::default() };
    save(home.path(), &s).unwrap();
    assert_eq!(load(home.path()).browser, s.browser);
}
#[test]
fn one_unreadable_browser_value_reads_as_its_default_and_the_rest_is_kept() {
    let home = tempfile::tempdir().unwrap();
    write_settings(home.path(), r#"{"browser": {"terminal_links": false, "open_agent_html": "yes", "cookies": "nevr"}}"#);
    assert_eq!(load(home.path()).browser, BrowserSettings { terminal_links: false, open_agent_html: true, cookies: CookieRule::Always });
}
// layout.rs
#[test]
fn a_tab_with_an_unreadable_viewport_is_kept_without_it() {
    for bad in [json!(390), json!("wide"), json!(null)] {
        let pane = read_pane(json!({"sessions": {"t1": {"tabs": [{"id": "a", "kind": "browser", "history": [{"path": "https://example.com/", "view": "rendered"}], "index": 0, "viewport": bad}], "active": "a"}}}));
        assert_eq!(pane.sessions["t1"].tabs.len(), 1);
        assert_eq!(pane.sessions["t1"].tabs[0].viewport, None);
    }
}
#[test]
fn a_tab_keeps_a_viewport_it_can_have() { /* "phone" round-trips through save and load, and validate_pane accepts it */ }
#[test]
fn validate_pane_refuses_a_viewport_that_is_not_a_size() { /* Err containing "viewport must be phone, tablet, laptop or desktop" */ }
// opens.rs
#[test]
fn a_pushed_open_carries_whether_a_hook_asked() { /* push(.., true) shows a PaneOpen with auto == true; the open route's push shows false */ }
```

`write_settings` and `read_pane` stand for the neighbouring tests' helpers.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p marshell-core -j 4 browser_settings` (does not compile: `BrowserSettings` is unknown).

- [ ] **Step 3: Implement**

`BrowserSettings` has a hand-written `Default` (two of its fields default to on), as `Settings` has in the same file. `FileDoc` gets `#[serde(default, deserialize_with = "lenient_browser")] browser: BrowserSettings`. `lenient_browser` reads the block as a JSON value and takes each field alone: a boolean that isn't one and a rule that isn't one of the three read as the default, each with a `tracing::warn!` that names the field and never the value. `load` and `save` carry `browser` through; `validate` needs nothing (the types hold every value). `lenient_session` removes a tab's `viewport` that is not a string for which `is_viewport` holds, before the tab is parsed; `repair_pane` clears one too; `validate_pane` refuses one with `"{at}: viewport must be phone, tablet, laptop or desktop"`. Every `PaneTab { .. }` literal in the crates gets the field. `Opens::push` takes `auto` and stores it on the `PaneOpen`; the `open` route passes `false`.

- [ ] **Step 4: Regenerate and run**

Run: `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated`, then `cargo test -p marshell-core -j 4 settings:: layout:: opens::` (three filters, three runs), then `pnpm build`.
Then `pnpm test src/features/appearance src/features/pane`.
Expected: all PASS. `pnpm build` type-checks the tests too, which is why the two TS files and the two test files above change in this task: until a read works, `SettingsSync` sends `{ terminal_links: true, open_agent_html: true, cookies: "always" }`, and it can't send before a read (`hasRead`).

- [ ] **Step 5: Commit**

`feat(protocol): settings.json holds the browser settings, a tab holds its viewport, and a queued open says when a hook asked.`

### Task 5: The browser settings reach React

**Files:**
- Modify: `src/features/appearance/settingsSync.ts` (the class at 34, `save` at 121, `writeFlags` at 196-227)
- Create: `src/features/browser/browserSettings.tsx`, `src/features/browser/browserSettings.test.ts`
- Test: `src/features/appearance/settingsSync.test.ts`

**Interfaces:**
- Consumes: `BrowserSettings`, `CookieRule` (Task 4).
- Produces:

```ts
// SettingsSync
get browser(): BrowserSettings | null            // null until the first read; the same object until it changes
get readFailed(): boolean                        // true after a read that failed, until one succeeds
setBrowser(change: Partial<BrowserSettings>): Promise<void>
subscribe(listener: () => void): () => void      // called after every read and write, also a failed read

// browserSettings.tsx
export const BROWSER_DEFAULTS: BrowserSettings   // { terminal_links: true, open_agent_html: true, cookies: "always" }
export function useBrowserSettings(): { settings: BrowserSettings | null; failed: boolean; set(change: Partial<BrowserSettings>): Promise<void> }
```

`useBrowserSettings` reads `useAppearance().sync` through `useSyncExternalStore`.

- [ ] **Step 1: Write the failing tests** in `settingsSync.test.ts`, in the style of `describe("SettingsSync: notifications")` (line 215):

```ts
describe("SettingsSync: browser", () => {
  it("is null before the first read, then what the core holds", async () => { /* get → { ...defaults, browser: { terminal_links: false, open_agent_html: true, cookies: "never" } } */ });
  it("writes one changed field and keeps the others", async () => { /* setBrowser({ cookies: "until-quit" }) puts browser: { terminal_links: false, open_agent_html: true, cookies: "until-quit" } and every other stored field unchanged */ });
  it("keeps the same object between changes and tells subscribers after a write", async () => { /* sync.browser === sync.browser; a subscribe listener runs once per setBrowser */ });
  it("says a failed read", async () => { /* get rejects → readFailed true, browser null, the listener ran */ });
});
```

- [ ] **Step 2: Run and see them fail**

Run: `pnpm test src/features/appearance/settingsSync.test.ts`. Expected: FAIL.

- [ ] **Step 3: Implement.** Task 4 left the private field and both payloads in place. This task adds the getter (null until `hasRead`), `readFailed` (set by a `load` that rejects, cleared by one that works), `setBrowser` through `writeFlags` (`browser` joins its `Pick`), and the listener set. Nothing here retries a failed read: `load` runs again when the app's existing callers run it.

- [ ] **Step 4: Run and see them pass**

Run: `pnpm test src/features/appearance src/features/browser/browserSettings.test.ts`. Expected: PASS.

- [ ] **Step 5: Commit**

`feat(settings): the app reads and writes the browser settings and tells React when they change.`

### Task 6: A context menu's checked rows, radio rows, notes and submenus (worker `effort: high`)

**Files:**
- Modify: `src/components/ContextMenu/menuKeys.ts`, `src/components/ContextMenu/ContextMenu.tsx`, `src/components/ContextMenu/ContextMenu.css`
- Create: `src/components/ContextMenu/ContextMenu.test.tsx`
- Test: `src/components/ContextMenu/menuKeys.test.ts`, `src/components/ContextMenu/ContextMenu.css.test.ts`
- Read first: `src/features/appMenu/AppMenuPopup.tsx` and `appMenu.ts` (`submenuPlace` at 46, `appMenuKey`): the app menu's Settings submenu is the model for keys, focus and placing.
- Move: `submenuPlace` (and its tests) from `src/features/appMenu/appMenu.ts` to `src/components/ContextMenu/submenuPlace.ts`; the app menu imports it from there. A component never imports from a feature.

**Interfaces:**
- Produces:

```ts
export type MenuRow = {
  id: string; label: string; shortcut?: string; disabled?: string;
  /** Present on a row that is on or off: `menuitemcheckbox`, or `menuitemradio` with `radio`. */
  checked?: boolean; radio?: true;
  /** A line under the label that is always shown; the row still runs. */
  note?: string;
  /** With `rows`, the row opens a submenu of them; `submenu: true` alone is the app menu's own marker and stays. */
  submenu?: true; rows?: MenuItem[];
};
export type MenuItem = MenuRow | { separator: true };
```

`ContextMenu`'s props don't change: `onRun(id)` gets a submenu row's own id, and the menu closes as it does today.

- [ ] **Step 1: Write the failing tests**

```tsx
// ContextMenu.test.tsx (renderToStaticMarkup; mock nothing the component doesn't need)
const items: MenuItem[] = [
  { id: "links", label: "Open terminal links here", checked: true },
  { id: "auto", label: "Open HTML files agents write", checked: false, note: "Not in this session: Codex's hooks aren't set up." },
  { id: "viewport", label: "Viewport", submenu: true, rows: [{ id: "viewport:responsive", label: "Responsive", checked: true, radio: true }, { id: "viewport:phone", label: "Phone (390 px)", checked: false, radio: true }] },
];
// `row(html, label)` is a test helper: the opening tag of the row whose text holds `label`. Asserts read one attribute at a time, never their order.
it("draws a checked row as a checkbox item with its state", () => {
  expect(row(html, "Open terminal links here")).toContain('role="menuitemcheckbox"');
  expect(row(html, "Open terminal links here")).toContain('aria-checked="true"');
  expect(row(html, "Open HTML files agents write")).toContain('aria-checked="false"');
});
it("shows a note under a row that still runs", () => {
  expect(html).toContain("Not in this session: Codex&#x27;s hooks aren&#x27;t set up.");
  expect(row(html, "Open HTML files agents write")).not.toContain("aria-disabled");
  expect(row(html, "Open HTML files agents write")).toContain("aria-describedby=");
});
it("marks a row that opens a submenu, closed at first", () => {
  expect(row(html, "Viewport")).toContain('aria-haspopup="menu"');
  expect(row(html, "Viewport")).toContain('aria-expanded="false"');
  expect(html).not.toContain("Phone (390 px)");
});
it("draws a radio row inside an open submenu", () => { /* render the submenu's rows directly (export the list component the menu and its submenu share) and expect role="menuitemradio" with aria-checked on both rows */ });
// menuKeys.test.ts
it("a row with rows opens on Enter, Space and the right arrow, like a submenu row", () => { /* { kind: "open", id: "viewport" } for all three */ });
it("type-ahead finds a checked row by its label", () => { /* "o" from index 0 moves to index 1 */ });
```

Add to `ContextMenu.css.test.ts`: the check mark and the radio dot use `currentColor`, and a `forced-colors` rule for them names system colors only.

- [ ] **Step 2: Run and see them fail**

Run: `pnpm test src/components/ContextMenu`. Expected: FAIL.

- [ ] **Step 3: Implement**

- Roles: `menuitemcheckbox` or `menuitemradio` with `aria-checked` when `checked` is present, else `menuitem`. The roving-focus query becomes `[role^="menuitem"]`.
- The mark: an `aria-hidden` SVG before the label (a check for a checkbox, a dot for a radio), drawn in `currentColor`, with its space kept when the row is off so labels line up. Rows without `checked` in a menu that has none keep today's look.
- `note` renders as `<span className="context-menu__note">` under the label; `aria-describedby` points the row at it.
- A row with `rows` gets `aria-haspopup="menu"` and `aria-expanded`. It opens on Enter, Space, the right arrow, a click, and a pointer resting on it; the submenu is a second `<ul role="menu" aria-label={row.label}>` placed with `submenuPlace`, its first row (or its checked row) focused. In it: the arrows, Home, End and type-ahead move; Enter and Space run; the left arrow and Escape close the submenu and put focus back on its row; Tab closes the whole menu. `menuKey`'s `open` and `back` results drive this (they return early today). The submenu is inside the menu's element for `leavesMenu` and the pointer-away test.

- [ ] **Step 4: Run and see them pass**

Run: `pnpm test src/components/ContextMenu src/features/browser/browserMenu.test.ts src/styles`. Expected: PASS (the style-rule tests under `src/styles` hold the no-outline and forced-colors rules).

- [ ] **Step 5: Commit**

`feat(menu): a context menu row can be checked, be one of a radio group, carry a note, and open a submenu.`

### Task 7: The viewport's sizes, a tab's size, and the fit

**Files:**
- Create: `src/features/browser/viewport.ts`, `src/features/browser/viewport.test.ts`
- Modify: `src/features/pane/paneState.ts` (`PaneAction` at 23-64, the reducer)
- Test: `src/features/pane/paneState.test.ts`

**Interfaces:**
- Consumes: `PaneTab.viewport?: string` (Task 4).
- Produces:

```ts
export type ViewportId = "phone" | "tablet" | "laptop" | "desktop";
export const VIEWPORTS: readonly { id: ViewportId; name: string; width: number }[];   // Phone 390, Tablet 768, Laptop 1024, Desktop 1440
export const RESPONSIVE = "Responsive";
export function viewportOf(tab: { viewport?: string } | undefined): ViewportId | null;  // an unknown value is null
export function viewportWidth(id: ViewportId | null): number | null;
/** How a `width` px page sits in a `slot` px wide place: null when Responsive or the slot has no width. `scale` is 1 when it fits. */
export function viewportFit(slot: number, width: number | null): { width: number; scale: number } | null;
/** Where a fixed-width page sits in its `slot` px wide place, exactly as the shell's `fit` places it (Task 8): undefined when Responsive and when the page doesn't fit, where the page is the whole place. */
export function pageBox(slot: number, width: number | null): { inlineSize: number; marginInlineStart: number } | undefined;
export function viewportRows(now: ViewportId | null): MenuItem[];   // ids "viewport:responsive", "viewport:phone", … as radio rows; labels "Responsive", "Phone (390 px)", "Tablet (768 px)", "Laptop (1024 px)", "Desktop (1440 px)"
export function viewportChoice(id: string): ViewportId | null | undefined;   // undefined for an id that is not a viewport row
export function chipText(id: ViewportId, slot: number): string;       // "Phone 390", "Desktop 1440 · 25%"
export function chipLabel(id: ViewportId, slot: number): string;      // "Viewport: Phone, 390 pixels wide", "Viewport: Desktop, 1440 pixels wide, shown at 25%"
export const viewportSaid = (id: ViewportId | null): string => …;    // "Viewport: Phone", "Viewport: Responsive"
/** The style of an HTML file's frame in a box: undefined when Responsive. */
export function frameBox(box: { width: number; height: number }, width: number | null): { inlineSize: number; blockSize?: number; marginInline?: "auto"; transform?: string; transformOrigin?: "0 0" } | undefined;
// paneState.ts
| { type: "viewport"; session: string; paneTab: string; viewport: ViewportId | null }
```

- [ ] **Step 1: Write the failing tests**

```ts
it("fits a narrower size at full scale and a wider one scaled", () => {
  expect(viewportFit(800, 390)).toEqual({ width: 390, scale: 1 });
  expect(viewportFit(390, 390)).toEqual({ width: 390, scale: 1 });
  expect(viewportFit(360, 1440)).toEqual({ width: 1440, scale: 0.25 });
  expect(viewportFit(0, 390)).toBeNull();
  expect(viewportFit(800, null)).toBeNull();
});
it("puts a page that fits where the shell puts it: centred, rounded down", () => {
  expect(pageBox(801, 390)).toEqual({ inlineSize: 390, marginInlineStart: 205 });   // Task 8's fit test: x 100 + 205 = 305
  expect(pageBox(390, 390)).toBeUndefined();
  expect(pageBox(360, 1440)).toBeUndefined();
  expect(pageBox(801, null)).toBeUndefined();
});
it("words the chip", () => {
  expect(chipText("phone", 800)).toBe("Phone 390");
  expect(chipText("desktop", 360)).toBe("Desktop 1440 · 25%");
  expect(chipLabel("desktop", 360)).toBe("Viewport: Desktop, 1440 pixels wide, shown at 25%");
  expect(chipLabel("phone", 800)).toBe("Viewport: Phone, 390 pixels wide");
});
it("lists five radio rows with the tab's one checked", () => {
  expect(viewportRows("tablet").map((r) => "label" in r && [r.id, r.label, r.checked])).toEqual([["viewport:responsive", "Responsive", false], ["viewport:phone", "Phone (390 px)", false], ["viewport:tablet", "Tablet (768 px)", true], ["viewport:laptop", "Laptop (1024 px)", false], ["viewport:desktop", "Desktop (1440 px)", false]]);
});
it("reads a tab's size and nothing else", () => {
  expect(viewportOf({ viewport: "laptop" })).toBe("laptop");
  expect(viewportOf({ viewport: "wide" })).toBeNull();
  expect(viewportOf(undefined)).toBeNull();
  expect(viewportChoice("viewport:responsive")).toBeNull();
  expect(viewportChoice("copy")).toBeUndefined();
});
it("centres an HTML file's frame that fits and scales one that doesn't", () => {
  expect(frameBox({ width: 800, height: 600 }, 390)).toEqual({ inlineSize: 390, marginInline: "auto" });
  expect(frameBox({ width: 360, height: 600 }, 1440)).toEqual({ inlineSize: 1440, blockSize: 2400, transform: "scale(0.25)", transformOrigin: "0 0" });
  expect(frameBox({ width: 800, height: 600 }, null)).toBeUndefined();
});
// paneState.test.ts
it("sets and clears a Browser tab's viewport, and leaves a Files tab alone", () => {
  const set = paneReducer(withPage, { type: "viewport", session: "s1", paneTab: "p1", viewport: "phone" });
  expect(tabsOf(set, "s1").tabs[0]!.viewport).toBe("phone");
  expect("viewport" in tabsOf(paneReducer(set, { type: "viewport", session: "s1", paneTab: "p1", viewport: null }), "s1").tabs[0]!).toBe(false);
  expect(paneReducer(withTextFile, { type: "viewport", session: "s1", paneTab: "f1", viewport: "phone" })).toBe(withTextFile);
});
```

- [ ] **Step 2: Run and see them fail**

Run: `pnpm test src/features/browser/viewport.test.ts src/features/pane/paneState.test.ts`. Expected: FAIL.

- [ ] **Step 3: Implement.** `scale` is `Math.min(1, slot / width)`; the percentage is `Math.round(scale * 100)`. The reducer edits through `editTab` only where `panelOf(tab) === "browser"` (a page, or an HTML file).

- [ ] **Step 4: Run and see them pass.** Run: the Step 2 command. Expected: PASS.

- [ ] **Step 5: Commit**

`feat(browser): a Browser tab holds a viewport size, with its fit, its rows and its chip's words.`

### Task 8: The shell places a page by its viewport (main thread)

**Files:**
- Modify: `src-tauri/src/browser/rules.rs` (`Report` at 530; new `fit`), `src-tauri/src/browser/com.rs` (new `cdp`, `emulate`), `src-tauri/src/browser/mod.rs` (`BrowserState`, `place` at 326, `on_resized` at 308, `show` at 347, the close paths)
- Test: the `mod tests` of `rules.rs`

**Interfaces:**
- Produces:

```rust
// rules.rs
pub const VIEWPORTS: [u32; 4] = [390, 768, 1024, 1440];
#[derive(Clone, Copy, Debug, PartialEq)] pub struct Emulate { pub width: u32, pub scale: f64 }   // scale rounded to 4 places
#[derive(Clone, Copy, Debug, PartialEq)] pub struct Fit { pub rect: Rect, pub emulate: Option<Emulate> }
/** Where a page with `viewport` sits in `slot`: centred at its own width when it fits, the whole slot with an emulation when it doesn't. A width not in `VIEWPORTS` is no viewport. */
pub fn fit(slot: Rect, viewport: Option<u32>) -> Fit
// Report gains:  #[serde(default)] pub page: Option<String>,  #[serde(default)] pub viewport: Option<u32>
#[derive(Default)] pub struct Viewports { .. }   // per page key
impl Viewports { pub fn set(&mut self, key: &str, viewport: Option<u32>); pub fn of(&self, key: &str) -> Option<u32>; pub fn forget(&mut self, key: &str); pub fn clear(&mut self) }
// com.rs
async fn cdp(page: &Webview, method: &'static str, params: String, limit: Duration, late: &'static str) -> Result<String, String>
pub fn emulate(page: &Webview, to: Option<Emulate>, failed: impl FnOnce() + Send + 'static)   // set or clear, without waiting; `failed` runs when the page refuses or doesn't answer
```

- Consumed by Task 9: the app sends `page` (the page's key) and `viewport` (a width, or null) in every `browser_place` report.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_viewport_that_fits_is_centred_at_its_own_width() {
    let f = fit(Rect { x: 100.0, y: 40.0, width: 801.0, height: 600.0 }, Some(390));
    assert_eq!(f, Fit { rect: Rect { x: 305.0, y: 40.0, width: 390.0, height: 600.0 }, emulate: None });
}
#[test]
fn a_viewport_as_wide_as_the_slot_is_the_slot() {
    let slot = Rect { x: 0.0, y: 0.0, width: 390.0, height: 600.0 };
    assert_eq!(fit(slot, Some(390)), Fit { rect: slot, emulate: None });
}
#[test]
fn a_viewport_wider_than_the_slot_is_emulated_and_scaled() {
    let slot = Rect { x: 10.0, y: 0.0, width: 317.0, height: 672.0 };
    assert_eq!(fit(slot, Some(1440)), Fit { rect: slot, emulate: Some(Emulate { width: 1440, scale: 0.2201 }) });
}
#[test]
fn no_viewport_an_unknown_width_and_an_empty_slot_are_the_slot() {
    let slot = Rect { x: 0.0, y: 0.0, width: 500.0, height: 500.0 };
    for v in [None, Some(0), Some(1280), Some(100_000)] { assert_eq!(fit(slot, v), Fit { rect: slot, emulate: None }); }
    let none = Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };
    assert_eq!(fit(none, Some(1440)), Fit { rect: none, emulate: None });
}
#[test]
fn viewport_is_kept_per_page() {
    let mut v = Viewports::default();
    v.set("s/a", Some(390)); v.set("s/b", None);
    assert_eq!((v.of("s/a"), v.of("s/b"), v.of("s/c")), (Some(390), None, None));
    v.forget("s/a");
    assert_eq!(v.of("s/a"), None);
}
#[test]
fn a_report_without_the_new_fields_still_reads() { /* the JSON of today's report deserializes with page None and viewport None */ }
```

- [ ] **Step 2: Run and see them fail.** Run: `cargo test -p marshell-app -j 4 viewport`. Expected: does not compile.

- [ ] **Step 3: Implement**

- `fit`: the centred `x` is `slot.x + ((slot.width - w) / 2.0).floor()`; the scale is `(slot.width / w * 10000.0).round() / 10000.0`.
- `cdp`: the pattern of `capture` (`with_webview`, a handler that sends on an `mpsc` channel, `answer`): `CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |result, json| ..))` and `ICoreWebView2::CallDevToolsProtocolMethod(&HSTRING::from(method), &HSTRING::from(params), &handler)`, as the spike's `spike` function did (`git show phase-6r-1b-spike:src-tauri/src/browser/com.rs`). `method` is a `&'static str`: only this module names one.
- `emulate(Some(e))` calls `Emulation.setDeviceMetricsOverride` with `{"width":W,"height":0,"deviceScaleFactor":0,"mobile":false,"scale":S}`, built with `format!` from the two numbers; `emulate(None)` calls `Emulation.clearDeviceMetricsOverride` with `{}`. Both are fired from the main thread without waiting.
- `BrowserState` gains `viewports: Mutex<Viewports>` and `emulated: Mutex<HashMap<String, Emulate>>` (by page key). One function settles a page: `fn settle(app: &AppHandle, key: &str, page: &Webview, slot: Rect)` computes `fit(slot, viewports.of(key))`, sets the bounds to `fit.rect`, and calls `com::emulate` only when `emulated` holds something else for the key. It records the new value at once (so a drag's many settles send one call per change), and `failed` takes the record out again if it still holds that value, so the next settle tries again.
- `place` stores the report's viewport under the report's `page` (when it has one) before it settles the shown page; `on_resized` and `show` settle the shown page with the placer's slot, so a page shown again gets its own size at once; `create` settles a page it makes for the key that is wanted on screen, once the page exists.
- A tab's viewport outlives its page: `browser_close` and `browser_gone` forget the key in `emulated` only (a page made again starts with no emulation and must be given it), and `viewports` keeps the key, because the app sends no new report for a page that is loaded again in the same place. `close_all` clears both.
- The slot is the page's whole place: the app reports the place without the viewport in it (Task 9), and `fit` alone centres and scales. All rects here are logical pixels (`set_bounds` takes logical ones), so a viewport's width is compared with the slot's as it is.
- `Placer` is not changed: it keeps deciding the slot.

- [ ] **Step 4: Run.** `cargo test -p marshell-app -j 4`, `cargo clippy -p marshell-app --all-targets -- -D warnings`. Expected: PASS, no warning.

- [ ] **Step 5: Commit**

`feat(shell): a page is placed by its tab's viewport: centred at its width when it fits, emulated and scaled when it doesn't.`

### Task 9: The viewport in the app: the report, the slot, the HTML file's frame and the chip

**Files:**
- Create: `src/features/browser/ViewportChip.tsx`, `src/features/browser/ViewportChip.css`, `src/features/browser/ViewportChip.test.tsx`
- Modify: `src/features/browser/browserApi.ts` (`Report`), `src/features/browser/usePageSlot.ts` (`measure`, `slotReport`, the hook's arguments), `src/features/browser/BrowserTab.tsx` and `BrowserTab.css` (the slot), `src/features/browser/UrlBar.tsx`, `src/features/preview/HtmlView.tsx`, `src/features/preview/Preview.tsx` (the Rendered / Source toolbar near 556), their callers in `src/features/pane/` for the new props
- Test: `src/features/browser/usePageSlot.test.ts`, `src/features/browser/UrlBar.test.tsx`, `src/features/browser/BrowserTab.css.test.ts`, `src/features/browser/BrowserTab.test.tsx` (the file that renders `BrowserTabView` today; create it if there is none)

**Interfaces:**
- Consumes: Task 7's `viewport.ts`; Task 8's report fields.
- Produces: `Report` gains `page: string | null` and `viewport: number | null`. `slotReport(slot, win, inputs, moving, bottomPerPx, page, viewport)`. `usePageSlot(ref, pageKey, active, covered, url, viewport: number | null)` (it still returns `Frozen | null`). `useSlotWidth(ref): number` in `usePageSlot.ts`: the slot's width less `2 * PAGE_GUTTER`, kept current by a `ResizeObserver`, 0 before the first measure. `nextPaint` becomes an export of `usePageSlot.ts` (Task 12 uses it). `<ViewportChip id={ViewportId} slot={number} onOpen={(anchor: { x: number; y: number }) => void} />`. `UrlBar` gains `viewport?: { id: ViewportId; slot: number } | null` and `onViewport?: (anchor: { x: number; y: number }) => void`. `HtmlView` gains `viewport: number | null` and `onBox?: (width: number) => void` (its frame's place, each time it changes). `BrowserTab` and `Preview` gain `onViewport?` and read the tab's own `viewport`.
- Who owns the chip's `slot`: `BrowserTab` for a page (from `useSlotWidth`), `Preview` for an HTML file (from `HtmlView`'s `onBox`). Nothing above them measures.

- [ ] **Step 1: Write the failing tests**

```tsx
// usePageSlot.test.ts
it("reports the page's key and its tab's viewport", () => {
  expect(slotReport(slot, win, null, false, 0, "s1/p1", 390)).toMatchObject({ page: "s1/p1", viewport: 390 });
  expect(slotReport(slot, win, null, false, 0, "s1/p1", null)).toMatchObject({ viewport: null });
});
it("reports the whole place, whatever the viewport", () => {
  // The shell's rule works out the place for a window size from constant insets; a centred box would break it.
  expect(slotReport(slot, win, null, false, 0, "s1/p1", 390).rect).toEqual(slotReport(slot, win, null, false, 0, "s1/p1", null).rect);
});
// BrowserTab.test.tsx (BrowserTabView through renderToStaticMarkup)
it("draws the frozen picture in the page's box, not across the place", () => {
  const html = view({ frozen: { picture: "data:image/png;base64,AA==", phase: "frozen" }, box: { inlineSize: 390, marginInlineStart: 205 } });
  expect(html).toMatch(/class="browser-tab__page" style="inline-size:390px;margin-inline-start:205px"[^>]*><img/);
  expect(view({ frozen: null, box: undefined })).not.toContain("inline-size");
});
// ViewportChip.test.tsx
it("is a button that names the size and opens a menu", () => {
  const html = renderToStaticMarkup(<ViewportChip id="desktop" slot={360} onOpen={() => {}} />);
  expect(html).toContain('aria-haspopup="menu"');
  expect(html).toContain('aria-label="Viewport: Desktop, 1440 pixels wide, shown at 25%"');
  expect(html).toContain("Desktop 1440 · 25%");
});
// UrlBar.test.tsx
it("shows the chip at the end of the bar only for a fixed viewport", () => {
  expect(bar({ viewport: { id: "phone", slot: 800 }, onViewport: () => {} })).toMatch(/Select region[\s\S]*Phone 390/);
  expect(bar({})).not.toContain("viewport-chip");
});
```

Add to `BrowserTab.css.test.ts` (or a new `ViewportChip.css.test.ts`): `.viewport-chip` has no `border` or `outline` declaration outside `:focus-visible`, and its `forced-colors` rule names system colors only.

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/browser`. Expected: FAIL.

- [ ] **Step 3: Implement**

- The slot element (`.browser-tab__slot`, which carries `data-page-key`) keeps its size and its report: the rect the app sends is the whole place, as today, and the shell's `fit` alone centres or scales the page in it. (A slot narrowed by CSS would break the shell's placing rule on a window resize, with nothing to correct it afterwards.)
- What the slot holds today (the frozen picture or fill at line 138, the Select region layer at 139-142) moves into one child, `<div className="browser-tab__page" style={box}>`, where `box` is `pageBox(useSlotWidth(slot), viewportWidth(viewportOf(tab)))`: the app's own overlays then cover the page's box and nothing beside it, at the same pixels the shell uses. With no box the child fills the slot's content box as the picture does today. The margins beside a centred page show the panel's own surface, and the page's focus ring is drawn around the page's box, also when it is centred. `BrowserTabView` gains the prop `box`.
- `BrowserTab` passes the tab's width to `usePageSlot`, which sends it with the key in every report; a change of viewport sends a report at once (add `viewport` to the measuring effect's dependencies).
- The chip's `slot` is `useSlotWidth(slot)` for a page. For an HTML file it is the frame's box (below).
- `HtmlView` wraps the frame in `<div className="html-view__fit">`, measures that box with a `ResizeObserver`, calls `onBox` with its width, and gives the frame `frameBox(box, viewport)` as its style. The wrapper hides what a scaled frame's layout box leaves outside it (`overflow: clip`). With `viewport` null the markup's style is absent and the look is today's.
- `ViewportChip` is a `<button type="button" className="viewport-chip">` with `chipText` as its text, `chipLabel` as its name, and `title` "Change the viewport"; a click, Enter or Space calls `onOpen` with the button's bottom-left corner. It is told apart by fill and tint, like the other chips.
- `UrlBar` renders the chip after "Select region"; `Preview` renders it after the Rendered / Source buttons of an HTML file in the Browser panel. Both only when `viewport` is not null.

- [ ] **Step 4: Run.** `pnpm test src/features/browser src/features/preview src/features/pane src/styles`, then `pnpm build`. Expected: PASS.

- [ ] **Step 5: Commit**

`feat(browser): a fixed viewport centres or scales a page and an HTML file, and shows as a chip at the end of the bar.`

### Task 10: The shell's screenshot, the cookie rule and private pages (main thread)

**Files:**
- Modify: `src-tauri/Cargo.toml` (`serde_json.workspace = true` moves from `[dev-dependencies]` to `[dependencies]`), `src-tauri/build.rs` (`COMMANDS`), `src-tauri/capabilities/default.json` (two lines after `allow-browser-clear-data`), `src-tauri/src/main.rs` (`generate_handler!` at 446-455, `quit` at 251), `src-tauri/src/browser/rules.rs`, `com.rs`, `mod.rs` (`create` at 438, `browser_open`, `blank_page` at 698)
- Test: the `mod tests` of `rules.rs`; the list test at `main.rs:497-533`

**Interfaces:**
- Produces (commands):

```rust
#[derive(Serialize)] pub struct Shot { pub png: String /* base64 */, pub cut: bool }
#[tauri::command] pub async fn browser_screenshot(app: AppHandle, key: String, full: bool) -> Result<Shot, String>
#[tauri::command] pub async fn browser_cookies(app: AppHandle, rule: String) -> Result<(), String>
```

Errors the app matches on, exactly: `"hidden"` (the key's page is not the one shown), the existing `NO_PAGE` text ("The page isn't open."), `"the page's picture was not ready in 10 s"`, `"no such cookie rule"`, `"the cookie rule isn't known yet"` (from `browser_open`).

- Produces (pure, in `rules.rs`):

```rust
pub const SHOT_MAX_PX: f64 = 16384.0;
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum CookieRule { Always, UntilQuit, Never }
impl CookieRule { pub fn parse(s: &str) -> Option<Self> }              // "always", "until-quit", "never"
/** A page's size from `Page.getLayoutMetrics`' answer (`cssContentSize`); None when it isn't there or isn't a size. */
pub fn content_size(metrics: &str) -> Option<Size>
/** The clip for a whole-page picture at display `scale`, and whether the page was cut: each side at most `SHOT_MAX_PX` device pixels and never more than the page. None for a page with no size. */
pub fn shot_clip(content: Size, scale: f64) -> Option<(Rect, bool)>
/** The PNG, base64, from `Page.captureScreenshot`'s answer. */
pub fn shot_png(answer: &str) -> Option<String>
/** What the commands share about cookies; `mod.rs` holds one behind a mutex. */
#[derive(Default)] pub struct Cookies { .. }
impl Cookies {
    pub fn tell(&mut self, rule: CookieRule);
    pub fn rule(&self) -> Option<CookieRule>;
    /** Whether the next page is private: an error until a rule was told. */
    pub fn private(&self) -> Result<bool, &'static str>;          // Err("the cookie rule isn't known yet")
    /** Called as a page that keeps data is about to be made: true the first time in a run, when the rule is UntilQuit then. What the last run left goes before that page loads. */
    pub fn take_start_clear(&mut self) -> bool;
    pub fn made(&mut self, key: &str, private: bool);
    pub fn gone(&mut self, key: &str);
    /** A page that keeps data, to clear the profile through: none when every open page is private. */
    pub fn kept_page(&self) -> Option<&str>;
    /** UntilQuit now, and a page that keeps data was made this run. */
    pub fn clears_at_quit(&self) -> bool;
}
```

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn reads_the_three_rules_and_nothing_else() {
    assert_eq!(["always", "until-quit", "never", "Never", ""].map(CookieRule::parse), [Some(CookieRule::Always), Some(CookieRule::UntilQuit), Some(CookieRule::Never), None, None]);
}
#[test]
fn reads_a_pages_size_from_the_metrics() {
    let m = r#"{"cssContentSize":{"height":5164,"width":1425,"x":0,"y":0},"cssVisualViewport":{"clientHeight":3053,"clientWidth":1425,"pageX":0,"pageY":1000}}"#;
    assert_eq!(content_size(m), Some(Size { width: 1425.0, height: 5164.0 }));
    assert_eq!(content_size("{}"), None);
    assert_eq!(content_size(r#"{"cssContentSize":{"height":"tall","width":1}}"#), None);
}
#[test]
fn a_page_that_fits_is_clipped_to_its_own_size() {
    assert_eq!(shot_clip(Size { width: 1425.0, height: 5164.0 }, 1.0), Some((Rect { x: 0.0, y: 0.0, width: 1425.0, height: 5164.0 }, false)));
}
#[test]
fn a_page_taller_than_one_picture_is_cut_at_the_limit() {
    assert_eq!(shot_clip(Size { width: 302.0, height: 40021.0 }, 1.0), Some((Rect { x: 0.0, y: 0.0, width: 302.0, height: 16384.0 }, true)));
}
#[test]
fn the_limit_is_in_device_pixels() {
    assert_eq!(shot_clip(Size { width: 1440.0, height: 20000.0 }, 1.5), Some((Rect { x: 0.0, y: 0.0, width: 1440.0, height: 10922.0 }, true)));
}
#[test]
fn a_page_wider_than_one_picture_is_cut_too_and_a_page_with_no_size_has_no_clip() {
    assert_eq!(shot_clip(Size { width: 100000.0, height: 100.0 }, 1.0).unwrap().0.width, 16384.0);
    for bad in [Size { width: 0.0, height: 10.0 }, Size { width: f64::NAN, height: 10.0 }, Size { width: 10.0, height: f64::INFINITY }] { assert_eq!(shot_clip(bad, 1.0), None); }
    assert_eq!(shot_clip(Size { width: 10.0, height: 10.0 }, 0.0), None);
}
#[test]
fn reads_the_picture_from_the_answer() {
    assert_eq!(shot_png(r#"{"data":"iVBORw0KGgo="}"#).as_deref(), Some("iVBORw0KGgo="));
    assert_eq!(shot_png(r#"{"data":""}"#), None);
    assert_eq!(shot_png("not json"), None);
}
#[test]
fn open_waits_for_the_rule() {
    let mut c = Cookies::default();
    assert_eq!(c.private(), Err("the cookie rule isn't known yet"));
    c.tell(CookieRule::Never);
    assert_eq!(c.private(), Ok(true));
    c.tell(CookieRule::Always);
    assert_eq!(c.private(), Ok(false));
}
#[test]
fn the_first_kept_page_under_until_quit_clears_what_the_last_run_left_once() {
    let mut c = Cookies::default();
    c.tell(CookieRule::UntilQuit);
    assert!(c.take_start_clear());
    c.made("s/a", false);
    assert!(!c.take_start_clear());
}
#[test]
fn a_run_that_was_told_never_first_still_clears_at_its_first_kept_page() {
    // The app tells "never" when it can't read the settings, and the real rule when it can.
    let mut c = Cookies::default();
    c.tell(CookieRule::Never);
    c.made("s/a", true);
    c.tell(CookieRule::UntilQuit);
    assert!(c.take_start_clear());
}
#[test]
fn a_run_that_made_a_kept_page_under_always_does_not_clear_at_the_next_page() {
    let mut c = Cookies::default();
    c.tell(CookieRule::Always);
    assert!(!c.take_start_clear());
    c.made("s/a", false);
    c.tell(CookieRule::UntilQuit);
    assert!(!c.take_start_clear());
    assert!(c.clears_at_quit());
}
#[test]
fn quit_clears_only_under_until_quit_after_a_kept_page() {
    let mut c = Cookies::default();
    c.tell(CookieRule::UntilQuit);
    assert!(!c.clears_at_quit());
    c.made("s/a", false);
    c.gone("s/a");
    assert!(c.clears_at_quit());
    c.tell(CookieRule::Never);
    assert!(!c.clears_at_quit());
}
#[test]
fn the_profile_is_cleared_through_a_page_that_keeps_data() {
    let mut c = Cookies::default();
    c.tell(CookieRule::Never);
    c.made("s/a", true);
    assert_eq!(c.kept_page(), None);
    c.made("s/b", false);
    assert_eq!(c.kept_page(), Some("s/b"));
    c.gone("s/b");
    assert_eq!(c.kept_page(), None);
}
```

And `a_page_that_is_not_shown_is_refused`: `Registry` with page `a` shown answers `shot_target("b")` with `Err("hidden")` and `shot_target("a")` with its label (a pure helper on `Registry`).

- [ ] **Step 2: Run and see them fail.** Run: `cargo test -p marshell-app -j 4 rules::`. Expected: does not compile.

- [ ] **Step 3: Implement**

- `browser_screenshot`: `supported()?`; the key's page must be the shown one (`shot_target`), else `"hidden"`. Not `full`: `cdp(page, "Page.captureScreenshot", r#"{"format":"png"}"#, 10 s, ..)`. `full`: `cdp(page, "Page.getLayoutMetrics", "{}", 5 s, ..)`, `content_size`, `shot_clip` with the window's `scale_factor()`, then `Page.captureScreenshot` with `{"format":"png","captureBeyondViewport":true,"clip":{"x":0,"y":0,"width":W,"height":H,"scale":1}}` built by `format!` from the clip's two numbers. Answer with `shot_png` and `cut`. No size, method or parameter comes from the app.
- `browser_cookies`: `CookieRule::parse` or `"no such cookie rule"`; `tell` it to the state.
- `create` asks the state `private()` (the error goes back to `browser_open`) and builds with `.incognito(private)`; it calls `made(key, private)` when the page exists, and the close paths call `gone(key)`. `blank_page` is never private and is not recorded.
- The start clear: when the page is not private and `take_start_clear()` is true, `create` builds the page at `about:blank`, and a task clears the profile's site data through that page (`ICoreWebView2Profile2::ClearBrowsingData` with `COREWEBVIEW2_BROWSING_DATA_KINDS_ALL_SITE`, the handler pattern of `clear_data`), then navigates the page to its address on the main thread (`post`), whether the clear answered or timed out after 5 s.
- `browser_clear_data` ("Clear browsing data…") clears through `kept_page()`, and through a temporary `blank_page` when there is none: a private page's profile is the one in memory, and clearing through it would leave the disk as it was. The private pages' own data goes with them: after the clear, when the rule is Never, the command answers as today and the app loads its pages again (Task 13 reuses the reload for it). The constant is in `webview2-com-sys` 0.39.1 (the locked version): cookies and all DOM storage. `.incognito(true)` is `SetIsInPrivateModeEnabled` on the controller's options in wry 0.57, the same switch S14 measured.
- The quit clear: `pub fn before_quit(app: &AppHandle, then: impl FnOnce(&AppHandle) + Send + 'static)`. When `clears_at_quit()` holds, it starts the same site-data clear through `kept_page()`, or through a `blank_page` made for it when no page that keeps data is open, and runs `then` on the main thread when the clear answers or after 3 s, once; otherwise it runs `then` at once. `main.rs`'s `quit` becomes `browser::before_quit(app, |app| { browser::close_all(app); app.exit(0); })`. The `RunEvent::Exit` path keeps calling `close_all` alone: what it leaves, the next start's clear takes.
- Register both commands in `build.rs`' `COMMANDS`, `generate_handler!` and `capabilities/default.json`. The build writes one permission file per command under `src-tauri/permissions/autogenerated/`, which is ignored by git: nothing there is committed.

- [ ] **Step 4: Run.** `cargo test -p marshell-app -j 4`, `cargo clippy -p marshell-app --all-targets -- -D warnings`. Expected: PASS (the list test at `main.rs:497` sees twelve browser commands in all three places).

- [ ] **Step 5: Commit**

`feat(shell): a page's picture is taken through its DevTools protocol, and the cookie rule makes private pages or clears site data at quit and at the next start.`

### Task 11: The core saves a picture to Downloads

**Files:**
- Create: `crates/core/src/server/screenshots.rs`, `src/lib/api/screenshots.ts`
- Modify: `crates/core/src/paths.rs`, `crates/core/src/server/mod.rs` (the router; follow how `attachments::save` is routed and guarded), `crates/protocol/src/api.rs`
- Generated: `src/generated/ScreenshotSaved.ts`
- Test: the `mod tests` of `screenshots.rs` and `paths.rs`; an integration test beside the attachments one in `crates/core/tests/`

**Interfaces:**
- Produces: `POST /v1/screenshots?host=<the page's host and port>&at=<YYYY-MM-DD HHMMSS>` with a PNG body, in the token-guarded router with `DefaultBodyLimit::max(SHOT_CAP as usize + 1)` (axum's own limit is 2 MB), as the attachments route is set up at `server/mod.rs:1342`. `200 { "name": "<the name written>" }`. The route takes no file name and no path (`.claude/rules/local-api.md`): the core makes the name from the two parts and writes only into Downloads.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)] #[ts(export)] pub struct ScreenshotSaved { pub name: String }
pub fn downloads_dir() -> Option<PathBuf>            // paths.rs: MARSHELL_DOWNLOADS when set and not empty, else dirs::download_dir()
pub const SHOT_CAP: u64 = 64 * 1024 * 1024;          // screenshots.rs
/// "example.com 2026-11-21 134501.png" from a host and a time. None when `at` isn't `YYYY-MM-DD HHMMSS` in digits.
pub(crate) fn shot_name(host: &str, at: &str) -> Option<String>
pub(crate) fn save_shot(dir: &Path, name: &str, png: &[u8]) -> io::Result<String>
```

```ts
export async function saveScreenshot(ep: Endpoint, host: string, at: string, png: Blob): Promise<ScreenshotSaved>   // rejects with ApiError carrying the core's words, as saveAttachment does
```

- Refusals, exactly: 400 "That time can't be used in a file name." · 400 "That isn't a PNG image." · 413 "That picture is over 64 MB." · 500 "The Downloads folder wasn't found." · 500 "The picture couldn't be written to Downloads."
- `MARSHELL_DOWNLOADS` is read in every build, as `MARSHELL_HOME` is: a live check and the tests point it at a scratch folder, because `dirs::download_dir()` asks Windows for the known folder and does not follow `USERPROFILE`.

- [ ] **Step 1: Write the failing tests**

```rust
const AT: &str = "2026-11-21 134501";
#[test]
fn a_name_is_the_host_and_the_time() {
    assert_eq!(shot_name("example.com", AT).as_deref(), Some("example.com 2026-11-21 134501.png"));
    assert_eq!(shot_name("localhost:5173", AT).as_deref(), Some("localhost-5173 2026-11-21 134501.png"));
    assert_eq!(shot_name("[::1]:8080", AT).as_deref(), Some("1--8080 2026-11-21 134501.png"));
}
#[test]
fn a_host_that_cannot_name_a_file_is_called_page() {
    // Nothing left, a path, and a Windows device name before the first dot ("con.example" is the device "con").
    for host in ["", "...", "..\\..\\x", "a/b", "con.example", "NUL", "com1.dev", "\u{202e}"] {
        let name = shot_name(host, AT).unwrap();
        assert!(!name.contains(['/', '\\', ':']), "{name}");
        assert!(name.ends_with(" 2026-11-21 134501.png"), "{name}");
    }
    assert_eq!(shot_name("", AT).as_deref(), Some("page 2026-11-21 134501.png"));
    assert_eq!(shot_name("con.example", AT).as_deref(), Some("page 2026-11-21 134501.png"));
    assert_eq!(shot_name(&"a".repeat(300), AT).unwrap().len(), 80 + " 2026-11-21 134501.png".len());
}
#[test]
fn a_time_that_is_not_digits_in_that_shape_makes_no_name() {
    for at in ["", "2026-11-21", "2026-11-21 13:45:01", "2026-11-21 134501 ", "..\\x", "２０２６-11-21 134501"] { assert_eq!(shot_name("example.com", at), None, "{at}"); }
}
#[test]
fn a_taken_name_gets_a_number_and_nothing_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(save_shot(dir.path(), "a 1.png", PNG).unwrap(), "a 1.png");
    assert_eq!(save_shot(dir.path(), "a 1.png", PNG).unwrap(), "a 1 (2).png");
    assert_eq!(save_shot(dir.path(), "a 1.png", PNG).unwrap(), "a 1 (3).png");
    assert_eq!(std::fs::read(dir.path().join("a 1.png")).unwrap(), PNG);
}
#[test]
fn downloads_follows_the_override() { /* with MARSHELL_DOWNLOADS set to a temp folder, downloads_dir() is it; serialise env use as the crate's other env tests do */ }
```

Integration: a POST with a PNG, `host=localhost:5173` and a good `at` answers 200 with "localhost-5173 2026-11-21 134501.png" and the file is in the overridden Downloads; a body without the PNG signature answers 400 with "That isn't a PNG image."; a body over the cap answers 413 with "That picture is over 64 MB."; a request without the bearer token is refused as every other route is.

- [ ] **Step 2: Run and see them fail.** Run: `cargo test -p marshell-core -j 4 screenshots`. Expected: does not compile.

- [ ] **Step 3: Implement.** `shot_name`: every character of `host` outside `A-Z a-z 0-9 . -` becomes `-`; leading and trailing `-` and `.` are dropped; the result is cut to 80 characters; when it is empty, or the part before its first dot is a Windows device name (`con`, `prn`, `aux`, `nul`, `com1`-`com9`, `lpt1`-`lpt9`, any case), it is `page`. `at` must be exactly ASCII digits in the shape `YYYY-MM-DD HHMMSS`. `save_shot` creates the file with `create_new` (an ordinary file: Downloads is the person's own folder), trying `name`, then `stem (2).png` up to `(99)`. The handler checks the size and the PNG signature as `attachments::save` does and writes in `spawn_blocking`. It creates no folder: a missing Downloads is the 500.

- [ ] **Step 4: Run.** `cargo test -p marshell-protocol -j 4`, `pnpm check:generated`, `cargo test -p marshell-core -j 4 screenshots`, `cargo test -p marshell-core -j 4 paths::`. Expected: PASS.

- [ ] **Step 5: Commit**

`feat(core): a picture of a page is written to Downloads under a checked name, never over a file that is there.`

### Task 12: "Save screenshot" in the app

**Files:**
- Create: `src/features/browser/screenshot.ts`, `src/features/browser/screenshot.test.ts`
- Modify: `src/features/browser/browserApi.ts`, `src/features/browser/pageCover.ts`
- Test: `src/features/browser/pageCover.test.ts`, `src/features/browser/browserApi.test.ts` (create either if there is none)

**Interfaces:**
- Consumes: `browser_screenshot` (Task 10), `saveScreenshot` (Task 11).
- Produces:

```ts
// browserApi.ts
/** Waits for the calls asked so far (so the page's show has run), then asks outside the queue: a capture can take seconds, and a show, a place or a close must not wait behind it. */
screenshot: (key: string, full: boolean) => Promise<{ png: string; cut: boolean }>,   // "browser_screenshot"; { png: "", cut: false } outside Tauri
// pageCover.ts
export function uncovered(limitMs: number): Promise<boolean>     // true at once when nothing covers the page, true when the last cover goes, false when `limitMs` passes first
// screenshot.ts
/** What the core names the file from: the page's host with its port ("localhost:5173"; "" for an address without one) and the local time as "2026-11-21 134501". */
export function shotAsk(url: string, at: Date): { host: string; at: string }
export type ShotTarget = { kind: "page"; key: string; url: string; loaded: boolean } | { kind: "file" } | { kind: "none" };
/** Why a picture can't be taken now; null when it can. */
export function shotReason(t: ShotTarget, pages: boolean, panelShown: boolean): string | null
export const savedText = (name: string, cut: boolean): string
export const SHOT_FAILED = "The screenshot wasn't saved.";
export const SHOT_COVERED = "The page is covered. Close what covers it and try again.";
export const COVER_WAIT_MS = 5000;
export type ShotDeps = { uncovered(limitMs: number): Promise<boolean>; paint(): Promise<void>; shoot(key: string, full: boolean): Promise<{ png: string; cut: boolean }>; save(host: string, at: string, png: Blob): Promise<{ name: string }>; now(): Date };
/** Takes and saves one picture; resolves to the line to announce. Never rejects. */
export function saveShot(deps: ShotDeps, t: { key: string; url: string }, full: boolean): Promise<string>
```

- [ ] **Step 1: Write the failing tests**

```ts
it("sends the page's host and the local time for the name", () => {
  const at = new Date(2026, 10, 21, 13, 45, 1);
  expect(shotAsk("https://example.com/a/b?c", at)).toEqual({ host: "example.com", at: "2026-11-21 134501" });
  expect(shotAsk("http://localhost:5173/", at)).toEqual({ host: "localhost:5173", at: "2026-11-21 134501" });
  expect(shotAsk("about:blank", at).host).toBe("");
  expect(shotAsk("not an address", at).host).toBe("");
  expect(shotAsk("https://example.com/", new Date(2026, 0, 2, 3, 4, 5)).at).toBe("2026-01-02 030405");
});
it("says why a picture can't be taken", () => {
  const page = { kind: "page", key: "s/p", url: "https://example.com/", loaded: true } as const;
  expect(shotReason(page, true, true)).toBeNull();
  expect(shotReason(page, false, true)).toBe("Not available on this system yet");
  expect(shotReason({ kind: "file" }, true, true)).toBe("Not for an HTML file");
  expect(shotReason({ kind: "none" }, true, true)).toBe("No page is open");
  expect(shotReason({ ...page, loaded: false }, true, true)).toBe("The page isn't loaded");
  expect(shotReason(page, true, false)).toBe("Show the Browser panel to save a screenshot.");
});
it("words what was saved", () => {
  expect(savedText("example.com 2026-11-21 134501.png", false)).toBe("Saved example.com 2026-11-21 134501.png to Downloads.");
  expect(savedText("example.com 2026-11-21 134501.png", true)).toBe("Saved the first 16,384 pixels of the page as example.com 2026-11-21 134501.png to Downloads.");
});
it("waits_for_the_cover_to_lift, then two paints, before it asks the shell", async () => {
  const order: string[] = [];
  let lift!: () => void;
  const deps = fake({ uncovered: () => new Promise<boolean>((r) => (lift = () => (order.push("uncovered"), r(true)))), paint: async () => void order.push("paint"), shoot: async () => (order.push("shoot"), { png: PNG_B64, cut: false }) });
  const said = saveShot(deps, { key: "s/p", url: "https://example.com/" }, false);
  await Promise.resolve();
  expect(order).toEqual([]);
  lift();
  expect(await said).toBe("Saved example.com 2026-11-21 134501.png to Downloads.");   // the fake's save answers with that name
  expect(order).toEqual(["uncovered", "paint", "paint", "shoot"]);
});
it("gives up when the page stays covered, and takes no picture", async () => { /* uncovered resolves false → SHOT_COVERED; shoot was not called */ });
it("uses the name the core wrote", async () => { /* save resolves { name: "example.com 2026-11-21 134501 (2).png" } → that name is in the line */ });
it("says a page that went off screen, the core's words for a refusal, and a plain line for anything else", async () => {
  /* shoot rejects "hidden" → "The page isn't on screen. Show it and try again."
     save rejects new ApiError(413, "That picture is over 64 MB.") → "The screenshot wasn't saved. That picture is over 64 MB."
     shoot rejects "the page's picture was not ready in 10 s" → "The screenshot wasn't saved." */
});
// pageCover.test.ts
it("uncovered is true at once with no cover and after the last cover goes", async () => { /* two covers; resolved true only after both release */ });
it("uncovered is false when the limit passes first", async () => { /* vi.useFakeTimers; a cover that stays; advance 5000 → false */ });
// browserApi.test.ts
it("a screenshot waits for the calls before it and holds up none after it", async () => {
  /* invoke mocked with a browser_show that resolves on demand and a browser_screenshot that never resolves:
     show("s/p") then screenshot("s/p", false) then place(report) → browser_screenshot is invoked only after browser_show resolved, and browser_place is invoked while the screenshot is still pending */
});
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/browser/screenshot.test.ts src/features/browser/pageCover.test.ts`. Expected: FAIL.

- [ ] **Step 3: Implement.** `saveShot`: `await uncovered(COVER_WAIT_MS)` (false ends it with `SHOT_COVERED`), two `paint()`s (the uncover effect has then queued the page's show in `browserApi`'s queue), `shoot` (which waits for that queue, then runs beside it), base64 to a `Blob` of type `image/png`, `save(host, at, blob)` with `shotAsk(url, now())`, the line with the name the core answered. The time is local. `browser.screenshot` is the one call that doesn't chain onto `tail`: it awaits the tail as it is when asked, then invokes. The `browserApi.test.ts` file exists or is created here with the module's `invoke` mocked.

- [ ] **Step 4: Run and see them pass.** Run: `pnpm test src/features/browser`. Expected: PASS.

- [ ] **Step 5: Commit**

`feat(browser): a page's picture is taken once nothing covers it and saved to Downloads under its host and the time.`

### Task 13: The cookie rule in the app

**Files:**
- Modify: `src/features/browser/browserApi.ts`, `src/features/browser/useBrowserPages.ts` (the theme effect at 137-149 is the pattern), `src/features/sessions/LiveApp.tsx` (the clear-data handler only)
- Create: `src/features/browser/cookies.ts`, `src/features/browser/cookies.test.ts`
- Test: `src/features/browser/browserApi.test.ts`, `src/features/browser/useBrowserPages.test.ts`

**Interfaces:**
- Consumes: `useBrowserSettings` (Task 5), `browser_cookies` (Task 10).
- Produces:

```ts
// browserApi.ts
/** Tells the shell the rule. The queue every call goes through starts closed: the first `cookies` call goes straight to the shell, and its end, good or bad, opens the queue. Later calls take their turn in it. */
cookies: (rule: CookieRule) => Promise<void>     // "browser_cookies"
// The queue's first `tail` is that gate, so every call asked before the rule is told keeps its order behind it:
// an open asked first is still sent before the close asked after it. Outside Tauri nothing waits.
// cookies.ts
export const COOKIE_ROWS: readonly { rule: CookieRule; label: string; said: string }[];   // "Keep always" / "Always", "Keep until quit" / "Until quit", "Never keep" / "Never"
export const SETTINGS_UNREAD = "Your settings couldn't be read";
export function cookieRows(now: CookieRule | null, pages: boolean): MenuItem[];   // ids "cookies:always", "cookies:until-quit", "cookies:never"; each off with "Not available on this system yet" when !pages; with `now` null (the settings aren't read) none is checked and each is off with SETTINGS_UNREAD
export function cookieChoice(id: string): CookieRule | undefined;
/** The rule to tell the shell: the saved one, or "never" when the settings can't be read; null while they are still loading. */
export function ruleToTell(settings: BrowserSettings | null, failed: boolean): CookieRule | null;
/** Whether a change of rule needs the loaded pages made again. */
export function needsReload(from: CookieRule | null, to: CookieRule): boolean;
export function cookiesSaid(rule: CookieRule, reloaded: number): string;
```

- [ ] **Step 1: Write the failing tests**

```ts
it("tells the saved rule, private pages when the settings can't be read, and nothing while they load", () => {
  expect(ruleToTell({ ...BROWSER_DEFAULTS, cookies: "until-quit" }, false)).toBe("until-quit");
  expect(ruleToTell(null, true)).toBe("never");
  expect(ruleToTell(null, false)).toBeNull();
});
it("loads pages again only when a page's privacy changes", () => {
  expect(needsReload(null, "never")).toBe(false);
  expect(needsReload("always", "until-quit")).toBe(false);
  expect(needsReload("always", "never")).toBe(true);
  expect(needsReload("never", "until-quit")).toBe(true);
  expect(needsReload("never", "never")).toBe(false);
});
it("words a change", () => {
  expect(cookiesSaid("until-quit", 0)).toBe("Cookies: Until quit.");
  expect(cookiesSaid("never", 1)).toBe("Cookies: Never. 1 tab was loaded again.");
  expect(cookiesSaid("never", 3)).toBe("Cookies: Never. 3 tabs were loaded again.");
  expect(cookiesSaid("always", 2)).toBe("Cookies: Always. 2 tabs were loaded again.");
});
it("lists three radio rows", () => {
  expect(cookieRows("never", true).map((r) => "label" in r && [r.id, r.label, r.checked, r.disabled])).toEqual([["cookies:always", "Keep always", false, undefined], ["cookies:until-quit", "Keep until quit", false, undefined], ["cookies:never", "Never keep", true, undefined]]);
  expect(cookieRows("always", false).every((r) => "label" in r && r.disabled === "Not available on this system yet")).toBe(true);
  expect(cookieRows(null, true).every((r) => "label" in r && r.checked === false && r.disabled === "Your settings couldn't be read")).toBe(true);
});
// browserApi.test.ts: the gate (the module is loaded fresh for each test, with invoke mocked)
it("nothing reaches the shell before the first rule is told, and then in the order asked", async () => {
  /* open("s/a", url), close("s/a"), then cookies("always") → the shell saw browser_cookies, browser_open, browser_close, in that order */
});
it("a rule the shell refused still opens the queue", async () => { /* invoke rejects for browser_cookies → the open goes on */ });
it("a second rule takes its turn in the queue", async () => { /* cookies("always"), open, cookies("never"), open → cookies, open, cookies, open */ });
// useBrowserPages.test.ts
it("loads every loaded page again, each at its tab's address, and counts them", () => {
  /* reloadPlan(pane, loadedKeys) → [{ key: "s1/p1", url: "https://a.example/" }, { key: "s2/p9", url: "https://b.example/x" }]; a key whose tab is gone is listed with url null (closed, not opened again) and is not counted */
});
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/browser/cookies.test.ts src/features/browser/browserApi.test.ts src/features/browser/useBrowserPages.test.ts`. Expected: FAIL.

- [ ] **Step 3: Implement.**
  - `reloadPlan(pane: PaneView, keys: readonly string[]): { key: string; url: string | null }[]` and `reloadPages(plan): number` in `useBrowserPages.ts`: each page is closed (`browser.close`, `dispatchPage({ type: "closed" })`), and each with an address is opened again at once with `openPage(key, url)`, shown or not (a page behind another tab loads there, as it was). It returns how many were opened. Decision 22's line is then true as written.
  - In `useBrowserPages`, an effect on `ruleToTell(settings, failed)`: when it is not null and differs from the rule last told, call `browser.cookies(rule)`; when `needsReload(last, rule)`, run the reload over every live page and announce `cookiesSaid(rule, count)` through the hook's `say`. A change that needs no reload is announced by whoever made it (Tasks 17 and 18), not here.
  - A read that fails tells "never", so a run that can't know the rule keeps nothing; when a later read works and says another rule, the pages load again and the line is said then, which is true. Until a read works, the menu and Settings show no rule as chosen (`cookieRows(null, ..)`), never the default.
  - "Clear browsing data…" under "Never keep": after `browser.clearData()` answers, the same reload runs with no line of its own (the private pages' cookies live in memory and go only when the last private page closes); the line the clear says today stays the only one. The clear's handler is in `LiveApp.tsx`, where `clearBrowserData` is handled.
  - Existing tests that call `browser.*` tell a rule first.

- [ ] **Step 4: Run and see them pass.** Run: `pnpm test src/features/browser`. Expected: PASS.

- [ ] **Step 5: Commit**

`feat(browser): the app tells the shell the cookie rule before any page loads, and loads the pages again when their privacy changes.`

### Task 14: "Open terminal links here"

**Files:**
- Modify: `src/features/terminal/urlLinks.ts:49-135`, `src/features/terminal/TerminalView.tsx:175-225`
- Test: `src/features/terminal/urlLinks.test.ts`

**Interfaces:**
- Consumes: `useBrowserSettings` (Task 5).
- Produces: `urlLinkProvider(term, platform, open, inPane: () => boolean)` and `webLinkHandler(platform, element, open, inPane: () => boolean)`: the hint is read when it is shown, not when the terminal was made. `linkTarget(page: OpenPageInPane | null, here: boolean): "pane" | "browser"` in `urlLinks.ts`.

- [ ] **Step 1: Write the failing tests**

```ts
it("a link goes to the side pane only when there are pages and the switch is on", () => {
  const page = () => {};
  expect(linkTarget(page, true)).toBe("pane");
  expect(linkTarget(page, false)).toBe("browser");
  expect(linkTarget(null, true)).toBe("browser");
});
it("the hint follows the switch while the terminal lives", () => {
  let here = true;
  const provider = urlLinkProvider(fakeTerm("see https://example.com/ now"), "windows", async () => {}, () => here);
  expect(hintOf(provider)).toBe("Ctrl+click to open in the side pane");
  here = false;
  expect(hintOf(provider)).toBe("Ctrl+click to open in your browser");
});
```

`fakeTerm` and `hintOf` stand for what the test at `urlLinks.test.ts:167-173` does already: a stub buffer with one line, the link's `hover`, and the `title` it sets. That test and its neighbour pass `true` for `inPane` today and change to `() => true`.

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/terminal/urlLinks.test.ts`. Expected: FAIL.

- [ ] **Step 3: Implement.** `TerminalView` keeps `useBrowserSettings().settings?.terminal_links ?? true` in a ref beside `pageRef`; `openWeb` opens in the pane when `linkTarget(pageRef.current, hereRef.current) === "pane"`, else `openUrl(url)`; both providers get `() => linkTarget(pageRef.current, hereRef.current) === "pane"`. Nothing else that opens a page changes (decision 27).

- [ ] **Step 4: Run and see them pass.** Run: `pnpm test src/features/terminal`. Expected: PASS.

- [ ] **Step 5: Commit**

`feat(terminal): Ctrl+click on a link in terminal output follows "Open terminal links here", and its hint says where it goes.`

### Task 15: The core opens an HTML file an agent wrote (worker `effort: high`)

**Files:**
- Create: `crates/core/src/agents/claude/writes.rs`, `codex/writes.rs`, `antigravity/writes.rs`, `qwen/writes.rs`, `copilot/writes.rs`, `crates/core/src/server/auto_open.rs`
- Modify: each agent's `mod.rs` (the module line), `crates/core/src/agents/mod.rs`, `crates/core/src/server/mod.rs`, `crates/core/src/server/hook.rs` (`post`, right after the "tab runs another agent" return at 174-178), `crates/core/src/server/opens.rs` (`classify` and `file_refusal` become `pub(crate)`; `push` gets the cap below)
- Test: each `writes.rs`' `mod tests`, `auto_open.rs`' `mod tests`, `opens.rs`' `mod tests`, `crates/core/tests/hook.rs`
- Read first: `docs/research/codex-cli.md` (AGENTS.md requires it before Codex hook work) and `docs/research/2026-10-24-codex-hooks.md` (the `apply_patch` body); each CLI's `status.rs` (how it reads the event's name, the tool and its arguments); the fixtures under `crates/core/tests/fixtures/<cli>/hooks/`; and the tool tables in `docs/research/2026-10-26-antigravity-hooks.md`, `2026-11-03-qwen-code-surfaces.md`, `2026-11-06-copilot-cli.md`.
- Not touched: `files::refused`, `files::links_refused`, `guard.rs`. A path the file guard refuses doesn't open (decision 29).

**Interfaces:**
- Consumes: `Settings.browser.open_agent_html`, `Opens::push(.., auto)` (Task 4).
- Produces:

```rust
// each agents/<cli>/writes.rs
/// The files this event says the CLI's file tool wrote, as the CLI gave them. Empty for any other event, and for a call the event says failed.
pub fn written(body: &serde_json::Value) -> Vec<String>
// agents/mod.rs
pub fn written(agent: &str, body: &serde_json::Value) -> Vec<String>   // "claude" | "codex" | "antigravity" | "qwen" | "copilot"; any other agent is empty
// server/auto_open.rs
/// The HTML files among `written`, as absolute Windows paths, each once, in order. A relative path is joined to `cwd` first (the Linux way with a `distro`), and one with no `cwd` is dropped.
pub(crate) fn html_targets(written: &[String], cwd: Option<&str>, distro: Option<&str>) -> Vec<PathBuf>
/// Queues the opens for one hook event. Spawned, never awaited by the hook's answer.
pub(crate) async fn queue(st: AppState, agent: String, tab: String, body: serde_json::Value)
// opens.rs
/// How many opens a hook asked for may wait for the app: half the queue, so an agent that writes many files can't refuse a `marshell open`.
pub const MAX_AUTO_WAITING: usize = 32;
```

- What each adapter reads (the end-of-tool event, as that CLI's `status.rs` names it):

| CLI | Event | Tools | Path |
| --- | --- | --- | --- |
| Claude Code | `PostToolUse` | `Write`, `Edit`, `MultiEdit` | `/tool_input/file_path` |
| Codex | `PostToolUse` | `apply_patch` | every `*** Add File:`, `*** Update File:` and `*** Move to:` line of `/tool_input/command`, each file a patch names; a `*** Delete File:` line names nothing |
| Antigravity | `PostToolUse` | `write_file`, `edit_file` | `/toolCall/args/Path`; nothing when `/error` is a non-empty string |
| Qwen Code | `PostToolUse` | `write_file`, `edit`, `replace` | `/tool_input/file_path` |
| Copilot CLI | `postToolUse` | `create`, `edit` | `path` in `toolArgs`, read as `copilot/status.rs` reads its arguments; only when `toolResult.resultType` is `"success"`, and nothing for `postToolUseFailure` |

Antigravity has no recorded PostToolUse body for a write: its test builds one from `PreToolUse-write_file.json` with the event's name changed and `"error": ""` added, and says so in a comment.

- [ ] **Step 1: Write the failing tests**

One per adapter, with the recorded fixture where one exists (`include_str!` of `codex/hooks/0.163.0/PostToolUse-apply_patch.json`, `qwen/hooks/0.26.5/PostToolUse-write_file.json`) and a `json!` body of the documented shape where none does:

```rust
// codex
#[test] fn a_patch_reports_the_files_it_adds_updates_and_moves_to() {
    let body = json!({"hook_event_name": "PostToolUse", "tool_name": "apply_patch", "tool_input": {"command": "*** Begin Patch\n*** Add File: out/report.html\n+<p>hi</p>\n*** Update File: index.html\n*** Move to: site/index.html\n*** Delete File: old.html\n*** End Patch"}});
    assert_eq!(written(&body), ["out/report.html", "index.html", "site/index.html"]);
}
#[test] fn the_recorded_patch_reports_its_file() { assert_eq!(written(&fixture("PostToolUse-apply_patch.json")), ["notes.md"]); }
#[test] fn a_shell_call_and_a_pre_tool_event_report_nothing() { /* PostToolUse.json (a shell call); PreToolUse-apply_patch.json */ }
// claude
#[test] fn write_edit_and_multiedit_report_their_file() { /* each tool with tool_input.file_path "C:\\proj\\a.html" → ["C:\\proj\\a.html"]; Bash, Read and a PreToolUse Write → [] */ }
// antigravity
#[test] fn a_failed_write_reports_nothing() { /* error: "permission denied" → []; error: "" → [Path] */ }
// qwen
#[test] fn the_recorded_write_reports_its_file() { assert_eq!(written(&fixture("PostToolUse-write_file.json")), ["C:/Users/user/AppData/Local/Temp/lc-6l1-2/proj6/b.txt"]); }
// copilot
#[test] fn create_and_edit_report_their_path_and_a_failure_does_not() { /* toolArgs as an object and as the string form status.rs accepts */ }
```

```rust
// auto_open.rs
#[test] fn keeps_html_files_only_whatever_the_case() {
    let t = html_targets(&s(&["C:\\p\\a.html", "C:\\p\\B.HTM", "C:\\p\\c.htmlx", "C:\\p\\d.md", "C:\\p\\html"]), None, None);
    assert_eq!(t, [PathBuf::from("C:\\p\\a.html"), PathBuf::from("C:\\p\\B.HTM")]);
}
#[test] fn a_relative_path_is_read_from_the_hooks_folder_and_dropped_without_one() {
    assert_eq!(html_targets(&s(&["out/report.html"]), Some("C:\\proj"), None), [PathBuf::from("C:\\proj\\out\\report.html")]);
    assert_eq!(html_targets(&s(&["out/report.html"]), None, None), Vec::<PathBuf>::new());
}
#[test] fn dot_segments_are_folded_and_a_file_is_listed_once() {
    assert_eq!(html_targets(&s(&["C:\\proj\\a\\..\\x.html", "C:/proj/x.html"]), None, None), [PathBuf::from("C:\\proj\\x.html")]);
}
#[test] fn a_wsl_sessions_path_on_a_windows_drive_becomes_that_drives_path() {
    assert_eq!(html_targets(&s(&["/mnt/c/proj/site/index.html"]), None, Some("Ubuntu")), [PathBuf::from("C:\\proj\\site\\index.html")]);
    assert_eq!(html_targets(&s(&["site/index.html"]), Some("/mnt/d/proj"), Some("Ubuntu")), [PathBuf::from("D:\\proj\\site\\index.html")]);
}
#[test] fn a_wsl_sessions_path_inside_the_distro_is_its_share_path_which_the_file_guard_refuses() {
    // Decision 29: the guard is not changed here, so this target is made and then refused by `classify`.
    let t = html_targets(&s(&["/home/u/site/index.html"]), None, Some("Ubuntu"));
    assert_eq!(t, [PathBuf::from(r"\\wsl.localhost\Ubuntu\home\u\site\index.html")]);
    assert!(crate::server::files::refused(&t[0].to_string_lossy()));
}
#[test] fn eighty_files_are_eighty_targets() { /* the queue's own limit, not this function, drops what is too many */ }
// opens.rs
#[test] fn opens_a_hook_asked_for_fill_half_the_queue_and_leave_room_for_a_command() {
    /* 32 pushes with auto true are held; the 33rd is Err(Full); a push with auto false is still held; and with 64 waiting in all, that one is Full too */
}
```

```rust
// crates/core/tests/hook.rs, in the style of hook_event_is_kept_under_its_live_tab
#[tokio::test] async fn a_written_html_file_is_queued_for_its_session() {
    /* a session tab; a real file <tmp>/report.html; POST /v1/hook as agent "claude" with a PostToolUse Write body for it
       → the opens the app would see hold one { tab_id, kind: "file", target: <that path>, by: "claude", auto: true } */
}
#[tokio::test] async fn nothing_is_queued_with_the_setting_off_for_a_file_that_is_not_there_or_for_a_text_file() { /* three posts, no open */ }
#[tokio::test] async fn a_full_queue_drops_the_rest_and_the_hook_still_answers() { /* 40 files in 40 posts → 32 waiting, every POST answers as before */ }
#[tokio::test] async fn a_relative_codex_path_opens_from_the_hooks_cwd() { /* an apply_patch body with "*** Add File: out/report.html" and "cwd": <tmp>; the file exists at <tmp>/out/report.html → one open for that absolute path */ }
#[tokio::test] async fn an_event_of_another_agent_than_the_tabs_opens_nothing() { /* the tab runs codex; a claude PostToolUse Write for a real .html → no open */ }
```

The queue is filled by a spawned task: each integration test waits for the open to appear (poll the opens for up to 2 s), and for "nothing is queued" waits 300 ms before it looks.

- [ ] **Step 2: Run and see them fail.** Run: `cargo test -p marshell-core -j 4 writes::` (does not compile).

- [ ] **Step 3: Implement**

- `queue`: returns at once when `st.0.settings.lock().unwrap().browser.open_agent_html` is false (the guard is dropped before any await) or `agents::written(agent, body)` is empty. The folder for relative paths is the body's `/cwd` when it is a string, else the tab's (`st.registry().cwd_of_tab`); the distro is the tab's WSL distro, as `in_wsl` finds it. In `spawn_blocking`: `html_targets`, then for each target `classify` must say `"file"` and `file_refusal` must be `None` (the checks `marshell open` makes; this is also where a path the file guard refuses, such as a distro's `\\wsl.localhost` share, ends), then `st.opens().push(tab, "file", path, agent, true)`; the receiver is dropped and `Err(Full)` is ignored.
- `Opens::push` refuses an `auto` open with `Full` when `MAX_AUTO_WAITING` of them wait already; the limit of `MAX_WAITING` in all stays.
- `hook::post` starts it right after the return at 174-178 (so an event of another agent than the tab runs opens nothing), only for `HookRole::Event`, one of the five agents, a named tab and `st.registry().is_open(tab)`: `tokio::spawn(auto_open::queue(st.clone(), agent.clone(), tab.to_string(), json.clone()))`. It is not awaited: the hook's answer and its time budget are as before. It sits outside the status fold, so the live-status switch doesn't reach it (decision 28). It never logs a path.
- The file is on disk when a PostToolUse arrives (the tool has returned); a file that isn't there, or isn't a file, is the tool's failure and opens nothing.
- Nothing in an adapter knows about HTML or the pane; nothing in `auto_open.rs` knows a tool's name.

- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4 writes::`, `cargo test -p marshell-core -j 4 auto_open::`, `cargo test -p marshell-core -j 4 --test hook`, `cargo clippy -p marshell-core --all-targets -- -D warnings`. Expected: PASS.

- [ ] **Step 5: Commit**

`feat(core): an HTML file an agent's file tool wrote is queued to open in its session, for each of the five CLIs.`

### Task 16: The app shows an agent's HTML file, says the limit once, and knows which sessions can't

**Files:**
- Create: `src/features/browser/autoOpen.ts`, `src/features/browser/autoOpen.test.ts`
- Modify: `src/features/pane/opens.ts:22-89`, its caller in `src/features/sessions/LiveApp.tsx` (where `takeOpens`' `said` and `news` are used)
- Test: `src/features/pane/opens.test.ts`

**Interfaces:**
- Consumes: `PaneOpen.auto` (Task 4); `HookInstall`, `HookAgent`, `agentLabel`.
- Produces:

```ts
// opens.ts: TakenOpens gains
/** The sessions where an open a hook asked for met the 32-tab limit, each once. */
full: string[];
/** Which of `full` to say the limit line for, and the sessions told so far: a session is told once, and again only after it has had room. */
export function limitNews(told: ReadonlySet<string>, full: readonly string[], pane: PaneView): { say: string[]; told: Set<string> }
// autoOpen.ts
/** Why the session's agent can't have its HTML files opened; null when it can, and for a session with no agent. */
export function autoOpenNote(agent: string | null, hooks: Partial<Record<HookAgent, HookInstall>>): string | null
export function autoOpenList(found: readonly string[], hooks: Partial<Record<HookAgent, HookInstall>>): { agent: string; label: string; text: string }[]
export const AUTO_OPEN_OK = "Opens its HTML files";
```

- [ ] **Step 1: Write the failing tests**

```ts
// opens.test.ts
it("an agent's own write reloads an open file in place and marks it, unless it is in front", () => { /* auto open of a path already open: no tab added; unseen true when not on screen, no mark when it is the front tab on screen (decision 8) */ });
it("lists a session whose auto open met the limit, once, and answers every request", () => {
  const taken = takeOpens(fullView, [auto(1, "a.html"), auto(2, "b.html"), byHand(3, "c.html")], new Set(), ctx);
  expect(taken.full).toEqual(["s1"]);
  expect(taken.done).toEqual([{ id: 1, refused: "limit" }, { id: 2, refused: "limit" }, { id: 3, refused: "limit" }]);
  expect(taken.said).toEqual([]);
});
it("says the limit once per session until it has had room", () => {
  const first = limitNews(new Set(), ["s1"], fullView);
  expect(first.say).toEqual(["s1"]);
  expect(limitNews(first.told, ["s1"], fullView).say).toEqual([]);
  const roomy = limitNews(first.told, [], viewWith31Tabs);
  expect(roomy.told.has("s1")).toBe(false);
  expect(limitNews(roomy.told, ["s1"], fullView).say).toEqual(["s1"]);
});
// autoOpen.test.ts
it("says nothing for a plain shell or a CLI whose hooks are in", () => {
  expect(autoOpenNote(null, {})).toBeNull();
  expect(autoOpenNote("claude", { claude: "installed" })).toBeNull();
});
it("says whose hooks aren't set up", () => {
  for (const state of ["none", "partial", "orphaned", undefined] as const) expect(autoOpenNote("codex", { codex: state })).toBe("Not in this session: Codex's hooks aren't set up.");
});
it("says an agent that reports no file writes", () => {
  expect(autoOpenNote("my-agent", {})).toBe("Not in this session: my-agent reports no file writes.");
});
it("lists each CLI found with what it does", () => {
  expect(autoOpenList(["claude", "codex"], { claude: "installed" })).toEqual([{ agent: "claude", label: "Claude Code", text: "Opens its HTML files" }, { agent: "codex", label: "Codex", text: "Its hooks aren't set up" }]);
});
```

Use `agentLabel` for the names, and the test file's existing builders for the views and requests.

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/pane/opens.test.ts src/features/browser/autoOpen.test.ts`. Expected: FAIL.

- [ ] **Step 3: Implement.** `takeOpens` adds a session to `full` when an open with `auto` is refused for the limit. `LiveApp` keeps the told set in a ref, runs `limitNews` on every frame of opens (with an empty `full` when none was refused, so room is noticed), and announces `TAB_LIMIT` for each session in `say` that is the one on screen; a session not on screen is told when nothing else: no line. An opened auto file is announced as `marshell open`'s is ("Claude Code opened report.html in Browser."), and its panel's dot and "A new tab is waiting" come from the existing `news` path unchanged.

- [ ] **Step 4: Run and see them pass.** Run: `pnpm test src/features/pane src/features/browser/autoOpen.test.ts`. Expected: PASS.

- [ ] **Step 5: Commit**

`feat(pane): an agent's HTML file opens as marshell open's does, the 32-tab limit is said once, and a session that can't says why.`

### Task 17: The Browser menu's rows and the palette's commands (worker `effort: high`)

**Files:**
- Modify: `src/features/browser/browserMenu.ts`, `src/features/sessions/LiveApp.tsx` (the menu at 2366-2394; the palette's hidden list at 777; the shortcut handling where `clearBrowserData` is), `src/lib/keymap.ts` (`COMMANDS` near 98-100), `src/lib/appShortcuts.ts:125-160`
- Test: `src/features/browser/browserMenu.test.ts`, `src/lib/keymap.test.ts`, `src/lib/appShortcuts.test.ts`

**Interfaces:**
- Consumes: Tasks 5, 6, 7, 9, 12, 13, 16.
- Produces:

```ts
export type BrowserMenuState = {
  pages: boolean;
  settings: BrowserSettings | null;     // null until a read works: nothing is shown as chosen then
  viewport: ViewportId | null;          // the tab's
  shot: string | null;                  // shotReason(...)
  autoNote: string | null;              // autoOpenNote(...) for the session on screen
};
export function browserMenuItems(t: MenuTarget, s: BrowserMenuState): MenuItem[]
export type BrowserMenuChoice =
  | { kind: "copy" | "outside" | "html" | "clear" | "links" | "auto" } | { kind: "shot"; full: boolean }
  | { kind: "viewport"; viewport: ViewportId | null } | { kind: "cookies"; rule: CookieRule } | null;
export function browserMenuChoice(id: string): BrowserMenuChoice
export const toggledText = (label: string, on: boolean): string => `${label}: ${on ? "on" : "off"}`;
```

- The rows, in order: "Copy link" · "Save screenshot" · "Save full-page screenshot" · "Open in the system browser" · — · "Open HTML file…" · — · "Open terminal links here" (checked) · "Open HTML files agents write" (checked, with `autoNote`) · "Viewport" (submenu of `viewportRows`) · "Cookies" (submenu of `cookieRows`) · — · "Clear browsing data…".
- Ids: `copy`, `shot`, `shot-full`, `outside`, `html`, `links`, `auto`, `viewport`, `cookies`, `clear`.
- Off, with the reason: the two screenshot rows with `shot`; "Viewport" with "No tab is open" when the target is `none`; "Open terminal links here", "Cookies" and "Clear browsing data…" with "Not available on this system yet" when `!pages` (a link can only open in the system browser there); and with `settings` null the two checked rows and the cookie rows are unchecked and off with `SETTINGS_UNREAD` ("Your settings couldn't be read").
- Palette commands, group "Window", no binding: `save-screenshot` "Save screenshot", `save-full-screenshot` "Save full-page screenshot", `viewport-responsive` "Viewport: Responsive", `viewport-phone` "Viewport: Phone", `viewport-tablet` "Viewport: Tablet", `viewport-laptop` "Viewport: Laptop", `viewport-desktop` "Viewport: Desktop". `commandShortcut` maps them to `{ kind: "screenshot"; full: boolean }` and `{ kind: "viewport"; viewport: ViewportId | null }`.

- [ ] **Step 1: Write the failing tests**

```ts
// browserMenu.test.ts
const state: BrowserMenuState = { pages: true, settings: BROWSER_DEFAULTS, viewport: null, shot: null, autoNote: null };
const page: MenuTarget = { kind: "page", url: "https://example.com/" };
it("lists the rows in the decided order", () => {
  expect(browserMenuItems(page, state).map((r) => ("separator" in r ? "—" : r.label))).toEqual(["Copy link", "Save screenshot", "Save full-page screenshot", "Open in the system browser", "—", "Open HTML file…", "—", "Open terminal links here", "Open HTML files agents write", "Viewport", "Cookies", "—", "Clear browsing data…"]);
});
it("shows the two switches' state and the session's note", () => {
  const rows = byId(browserMenuItems(page, { ...state, settings: { ...BROWSER_DEFAULTS, terminal_links: false }, autoNote: "Not in this session: Codex's hooks aren't set up." }));
  expect(rows.links).toMatchObject({ checked: false });
  expect(rows.auto).toMatchObject({ checked: true, note: "Not in this session: Codex's hooks aren't set up." });
  expect(rows.auto.disabled).toBeUndefined();
});
it("holds the viewport and cookie rows as radio submenus with the current one checked", () => {
  const rows = byId(browserMenuItems(page, { ...state, viewport: "laptop", settings: { ...BROWSER_DEFAULTS, cookies: "until-quit" } }));
  expect(rows.viewport.rows!.filter((r) => "checked" in r && r.checked).map((r) => "id" in r && r.id)).toEqual(["viewport:laptop"]);
  expect(rows.cookies.rows!.filter((r) => "checked" in r && r.checked).map((r) => "id" in r && r.id)).toEqual(["cookies:until-quit"]);
});
it("turns rows off with their reasons", () => {
  expect(byId(browserMenuItems({ kind: "file" }, { ...state, shot: "Not for an HTML file" })).shot.disabled).toBe("Not for an HTML file");
  expect(byId(browserMenuItems({ kind: "file" }, state)).viewport.disabled).toBeUndefined();
  expect(byId(browserMenuItems({ kind: "none" }, state)).viewport.disabled).toBe("No tab is open");
  const off = byId(browserMenuItems(page, { ...state, pages: false, shot: "Not available on this system yet" }));
  expect([off.links.disabled, off.cookies.disabled, off.clear.disabled, off["shot-full"].disabled]).toEqual(Array(4).fill("Not available on this system yet"));
  expect(off.auto.disabled).toBeUndefined();
});
it("shows nothing as chosen while the settings aren't read", () => {
  const rows = byId(browserMenuItems(page, { ...state, settings: null }));
  expect([rows.links.checked, rows.links.disabled, rows.auto.checked, rows.auto.disabled]).toEqual([false, "Your settings couldn't be read", false, "Your settings couldn't be read"]);
  expect(rows.cookies.rows!.every((r) => "checked" in r && r.checked === false && r.disabled === "Your settings couldn't be read")).toBe(true);
});
it("reads a chosen row", () => {
  expect(browserMenuChoice("shot-full")).toEqual({ kind: "shot", full: true });
  expect(browserMenuChoice("viewport:responsive")).toEqual({ kind: "viewport", viewport: null });
  expect(browserMenuChoice("cookies:never")).toEqual({ kind: "cookies", rule: "never" });
  expect(browserMenuChoice("links")).toEqual({ kind: "links" });
  expect(browserMenuChoice("viewport")).toBeNull();
  expect(browserMenuChoice("nope")).toBeNull();
});
// keymap.test.ts, beside the browser commands' test at 269
it("has the screenshot and viewport commands, with no default key", () => { /* the seven ids exist, group "Window", binding undefined, labels as above */ });
// appShortcuts.test.ts
it("maps the new commands", () => { /* commandShortcut("save-full-screenshot") → { kind: "screenshot", full: true }; "viewport-responsive" → { kind: "viewport", viewport: null }; "viewport-desktop" → { kind: "viewport", viewport: "desktop" } */ });
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/browser/browserMenu.test.ts src/lib`. Expected: FAIL.

- [ ] **Step 3: Implement**

In `LiveApp`, one function per act, used by the menu's `onRun`, the chip's menu and the palette alike:

- **Screenshot** (`full`): the target is the Browser panel's front tab of the session on screen. When `shotReason` gives a reason, announce it and stop. Else `saveShot(...)` with `uncovered` from `pageCover`, the hook's `nextPaint`, `browser.screenshot`, `saveScreenshot(endpoint, ..)`; announce its line.
- **Viewport** (`viewport`): the same tab. No tab in Browser: announce "No tab is open in Browser." Else dispatch `{ type: "viewport", .. }` and announce `viewportSaid(viewport)`.
- **Links / auto**: `set({ terminal_links: !now })` or `set({ open_agent_html: !now })`, then announce `toggledText("Open terminal links here", on)` or `toggledText("Open HTML files agents write", on)`. A write that fails announces "The setting wasn't saved."
- **Cookies** (`rule`): `set({ cookies: rule })`; when `needsReload(current, rule)` is false, announce `cookiesSaid(rule, 0)` here (Task 13 announces the other case).
- The chip (`onViewport` from Task 9) opens a second `ContextMenu`, labelled "Viewport", with `viewportRows(viewportOf(tab))` at the chip's anchor; a chosen row runs the viewport act; closing it puts focus back on the chip.
- `hiddenCommands` (line 777) gains `save-screenshot` and `save-full-screenshot` where pages aren't available. The five viewport commands are never hidden: an HTML file has a viewport on every system.
- `BrowserTab` and `Preview` get `onViewport` from here and nothing else: each reads its tab's viewport and measures its own `slot` (Task 9).

- [ ] **Step 4: Run.** `pnpm test`, then `pnpm build`. Expected: PASS (the overlay list test still names every overlay that covers the page; the new Viewport menu is a `ContextMenu`, which covers).

- [ ] **Step 5: Commit**

`feat(browser): the Browser menu saves screenshots, sets the viewport and the cookie rule, and switches terminal links and auto-open; the palette gains the screenshots and the viewport.`

### Task 18: The Settings page's Browser section

**Files:**
- Create: `src/features/browser/BrowserSection.tsx`, `src/features/browser/BrowserSection.test.tsx`
- Modify: `src/features/settings/settingsSections.ts:2-19`, `src/features/settings/LiveSettings.tsx` (`render` at 130), `src/features/appearance/AppearanceSections.tsx` (export `Choice`, lines 78-96)
- Test: `src/features/settings/settingsSections.test.ts` if there is one; else the section's own test

**Interfaces:**
- Consumes: `useBrowserSettings` (Task 5), `COOKIE_ROWS`, `cookiesSaid`, `needsReload` (Task 13), `autoOpenList` (Task 16), `toggledText` (Task 17), `Switch`, `Choice`.
- Produces: `SettingsSection` gains `"browser"`; `SETTINGS_SECTIONS` gets `{ id: "browser", label: "Browser" }` after `terminal`. `<BrowserSection announce={(text: string) => void} pages={boolean} platform={Platform} found={string[]} hooks={Partial<Record<HookAgent, HookInstall>>} />`.

- Copy, exactly:
  - Heading: "Browser".
  - Switch "Open terminal links here". Line: "Ctrl+click on a web link in terminal output opens it in the Browser panel. When this is off, it opens your system browser." ("⌘+click" on macOS.) Where pages aren't available: the switch is off and disabled with the reason "Not available on this system yet".
  - Switch "Open HTML files agents write". Line: "An HTML file an agent creates or edits opens as a background tab in its session's Browser panel. A file written by a shell command isn't seen. In a WSL session, only files on a Windows drive (under /mnt) open." (the last sentence on Windows only; decision 29). Under it, a list with one row per CLI found: "Claude Code: Opens its HTML files", "Codex: Its hooks aren't set up". With no CLI found: "No agent CLI was found."
  - Choice "Cookies", with the three options and a line each: "Keep always" — "Sign-ins last until you clear browsing data."; "Keep until quit" — "Cookies and site storage are cleared when Marshell quits and when it starts."; "Never keep" — "Every Browser tab is a private page. Nothing is written to disk, and it is gone when the last Browser tab closes." Where pages aren't available: disabled, with "Not available on this system yet".
  - Until the settings are read, the two switches are off and no cookie rule is chosen, all disabled; when the read failed, the reason on each is "Your settings couldn't be read" (`SETTINGS_UNREAD`). Nothing shows a default as if it were the saved value.

- [ ] **Step 1: Write the failing tests** (`renderToStaticMarkup`, with the settings hook mocked to given values):

```tsx
it("shows the two switches with their state and lines", () => { /* both labels, both lines above; role="switch" aria-checked follows the settings */ });
it("lists each CLI found with what it does", () => { /* "Claude Code: Opens its HTML files", "Codex: Its hooks aren't set up"; and "No agent CLI was found." with none */ });
it("offers the three cookie rules with the saved one chosen", () => { /* three radios named "Keep always", "Keep until quit", "Never keep"; the checked one follows settings.cookies; the three lines */ });
it("turns the page-only settings off where there are no pages", () => { /* pages=false: the links switch and the cookie radios are disabled with "Not available on this system yet"; the auto-open switch is not */ });
it("is the section after Terminal", () => { expect(SETTINGS_SECTIONS.map((s) => s.id).slice(0, 3)).toEqual(["appearance", "terminal", "browser"]); });
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/browser/BrowserSection.test.tsx src/features/settings`. Expected: FAIL.

- [ ] **Step 3: Implement.** A switch calls `set` and announces `toggledText(label, on)`; the cookie choice calls `set({ cookies })` and announces `cookiesSaid(rule, 0)` when `needsReload` is false (Task 13 says the other case). A failed write announces "The setting wasn't saved." `LiveSettings` passes `pages` (`browserTabs(os)`), the platform, the CLIs found and the hooks' state it already has for the other sections.

- [ ] **Step 4: Run.** `pnpm test`, `pnpm build`. Expected: PASS.

- [ ] **Step 5: Commit**

`feat(settings): the Settings page has a Browser section with terminal links, auto-open and the cookie rule.`

### Task 19: The plan's text follows, then the live check (main thread)

**Files:**
- Modify: `docs/PLAN.md` (the Browser panel's menu wherever §4 to §7 describe it, the settings list, the palette's commands, the announcer examples), `.claude/skills/marshell-live-check/live-check.ps1` and `SKILL.md` (`start` and `restart` set `MARSHELL_DOWNLOADS` to `<scratch>\home\Downloads`, the folder `start` already makes, and print it)
- Create: `docs/research/2026-11-21-browser-menu-live-check.md`

- [ ] **Step 1: Bring `docs/PLAN.md`'s running text in line** with decisions 1 to 29 where it still describes the old menu, "Fill the side pane", or a cursor id. Commit: `docs(plan): the Browser menu, the viewport, the cookie rule and auto-open are described where the plan describes the pane.`

- [ ] **Step 2: Run what CI runs**, one command per call, in the foreground: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace -j 4`, `pnpm test`, `pnpm check:generated`, `pnpm build`. Expected: all pass; the counts go in the PR body.

- [ ] **Step 3: Live check** with the `marshell-live-check` skill: a new scratch folder, `-QwenFake` and `-CopilotFake`, CDP on 9333, the spike's loopback site from `git show phase-6r-1b-spike:spikes/local-html-page/server.mjs` copied to the scratchpad. No desktop input. Exit criteria, each a row of the note:

| # | Check |
| --- | --- |
| 1 | The cursor says "Cursor on Browser", "Cursor on Files", "Cursor on Command palette", "Cursor on Sidebar". Hide draws a bar; Fill is named "Fill the side pane with Browser". |
| 2 | The ⋯ menu lists the thirteen rows in order; the keyboard alone opens "Viewport", moves in it, picks "Phone (390 px)", and comes back to the ⋯ button. In emulated forced colors the check marks and the chip are visible. |
| 3 | Phone in a wide panel: the page's `innerWidth` is 390 and it sits centred. Desktop in a narrow panel: `innerWidth` is 1440, the chip reads "Desktop 1440 · N%", and N follows a divider move. A switch to a Responsive tab and back keeps each tab's size. The viewport survives a restart of the app (`restart`). |
| 4 | An HTML file tab at Phone and at Desktop: the frame is 390 px wide and centred, then 1440 px wide and scaled, with the chip in its toolbar. |
| 5 | "Save screenshot" at Desktop writes a 1440 px wide PNG to the scratch Downloads and says "Saved … to Downloads."; a second in the same second gets " (2)". "Save full-page screenshot" on `/tall` writes the page's whole height; on a 40,000 px page it writes 16,384 px and says "Saved the first 16,384 pixels of the page as …". From the palette, with the Browser panel hidden: "Show the Browser panel to save a screenshot." While a full-page capture runs, a tab switch is not held up. The real Downloads folder got nothing. The 16,384 limit at a display scale over 100% can't be measured on this PC's one 100% monitor: the note says so. |
| 6 | Cookies: under "Keep always" a cookie from `/set-cookie` is sent after a restart. Under "Keep until quit" it is gone after a restart, also when every Browser tab was closed before the quit. A switch to "Never keep" says "Cookies: Never. N tabs were loaded again.", each of the N has loaded again (the ones behind other tabs too), the page has no cookie, and nothing of it is in the profile after a restart. A switch between "Keep always" and "Keep until quit" loads nothing again. "Clear browsing data…" under "Never keep" removes a cookie kept earlier under "Keep always". |
| 7 | "Open terminal links here" off: Ctrl+click on a link in terminal output (sent as CDP mouse events into the app's webview) opens no Browser tab, and the hint reads "Ctrl+click to open in your browser". On again: it opens a tab. |
| 8 | Auto-open with real CLIs: a Qwen Code session (`WRITE <abs>\report.html`) and a Copilot CLI session each get a background Browser tab, the Browser dot and "Qwen Code opened report.html in Browser."; an `EDIT` of the open file reloads it in place. With the switch off nothing opens. For Claude Code, Codex and Antigravity the recorded PostToolUse body is posted through the real `marshell-hook` in a session of that agent's kind, and the tab opens. |
| 9 | A session whose CLI has no hooks shows "Not in this session: …" under the menu row; Settings' Browser section lists the CLIs found. |
| 10 | Thirty-two tabs, then two more written HTML files: the limit line is said once. |
| 11 | `~/.claude/settings.json`'s modified time is unchanged; only what the check started was stopped. |

- [ ] **Step 4: Write the note** in the skill's format, with every finding and its fix commit, and commit it: `docs(research): the Browser menu's live check.`

## After the tasks

1. The whole-branch review by `marshell-reviewer` (Opus) with this plan's Review Focus, then one fix pass.
2. `marshell-ship`: the checks, the PR ("What it does", "Live check", "Known limits", "Checks"), a green CI.
3. `pre-merge-review`, then the merge. Delete the ledger; update issue #161 (close it when its ten items are done).

Also after the merge: open an issue for decision 29's follow-up (letting the viewer and `marshell open` read a session's own distro files, which changes the file guard and gets a plan of its own).

Known limits to state in the PR: the viewport is a width (no touch, no phone user agent, the page's own zoom is not held); "Select region" on a scaled page captures the scaled drawing; a full-page picture stops at 16,384 device pixels, measured at 100% only; only the page on screen can be captured; Codex reports writes made by `apply_patch` only, and no CLI reports a file a shell command wrote; from a WSL session only files on a Windows drive open (decision 29); Claude Code, Codex and Antigravity were checked with recorded hook bodies, not a signed-in CLI; a local HTML file still runs no script (slice 6r-1c).
