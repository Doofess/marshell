# Slice 6q-3a: the browser tab — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The side pane gets a browser tab: a real web page (a WebView2 child webview) with a URL bar, placed exactly over its slot, isolated from Marshell's own commands and API, with its own signed-in profile. Web addresses from the terminal, port chips and Markdown open in it. `marshell open` and region to prompt on a page are 6q-3b.

**Architecture:** The shell (`src-tauri/src/browser/`) owns the pages: one child webview per loaded browser tab on the main window, all on one profile under `<MARSHELL_HOME>/browser-profile`. Pure rules (`rules.rs`: the navigation allow-list, the key route, the placer) are unit-tested; the COM layer (`com.rs`) only reads events and posts to the main thread. The frontend (`src/features/browser/`) owns which pages exist (at most 8, least recently shown unloaded), sends each page's box and the layout rule, freezes the page as a picture while anything covers it, and keeps "the page has the keyboard" from the page's own focus events. A browser tab is a `PaneTab` of kind `browser` whose one history entry holds its address.

**Tech Stack:** Tauri 2.12 with `unstable`, wry 0.57, webview2-com 0.39.1, `windows` 0.62, Rust; React 19, TypeScript, Vitest (node, `renderToStaticMarkup`).

**Spec:** `docs/PLAN.md` §9: "Slice 6q-3 decisions (2026-11-10)" 1–12; "Slice 6q decisions" first round 6 and 10, second round 3–5; "Slice 6q-0 decisions" 6, 8 and 9; "Slice 6q-2 decisions" 11. Carry-over: `docs/spikes/S12-browser-pane.md`, "What 6q-2 and 6q-3 take from this" and "Go or no-go" (limits 1–6). The spike's code is the branch `phase-6q-0` (`src-tauri/src/spike_browser/`, `src/features/spike/`), read with `git show phase-6q-0:<path>`; it is a reference, not a base: nothing is cherry-picked.

## Global Constraints

- **Security surface.** The user approved exactly three changes (6q-3 decision 2): Tauri's `unstable` feature; the capability scoped by `webviews: ["main"]` with an app manifest; `frame-ancestors` in the CSP only if Task 13 shows a frame at the app's origin reaching IPC. Nothing else in `src-tauri/capabilities/`, `tauri.conf.json`, `tauri.windows.conf.json` or `crates/core/src/server/guard.rs` changes. No new crate or package; `windows` and `windows-sys` gain Cargo features only. If a task seems to need more, stop and report.
- **A page never gets:** a plugin permission, a Marshell command, the local API's token, or a navigation to anything but `http`, `https` and `about:blank`. Typed text that is a path or another scheme is refused, never sent to a search engine.
- **Windows only.** Everything that touches a page is `#[cfg(windows)]` in Rust; the commands exist on every OS and answer `Err("Browser tabs aren't available on this system yet.")` elsewhere. In the frontend `browserTabs = os === "windows"`; where it is false, web addresses keep opening the system browser as today.
- **Names.** Feature folders `src-tauri/src/browser/` and `src/features/browser/`. A page's key is `` `${tab_id}/${paneTab.id}` `` (`pageKey`). A session's id is `tab_id`; a pane's tab is "pane tab" in identifiers.
- **Numbers.** 8 loaded pages in the whole app. 32 tabs per session (unchanged). The placer's hold is 250 ms. A capture waits at most 5 s. A site icon over 64 KB is dropped. A title is saved up to 200 characters, an address up to 4096.
- **Copy** (sentence case, no "!", no "successfully"; exact strings):
  - Commands: `new-browser-tab` "New browser tab"; `open-url` "Open URL…"; `clear-browser-data` "Clear the browser's data…". None has a default key.
  - + menu: "New browser tab" first, then the existing rows. Where `browserTabs` is false the row is disabled and its hint reads "Not available on this system yet".
  - URL bar: toolbar `aria-label="Page"`; icon buttons `aria-label` and `title` "Back", "Forward", "Reload" (while loading: "Stop"), "Open in the system browser"; the box `aria-label="Address"`, placeholder "Search Google or type an address".
  - The page's slot: `aria-label="Web page"`.
  - A tab's title: the page's title, else its host, else "New tab". Its tooltip is the address.
  - Status line under the bar: "That address can't be opened in the side pane." · "This page stopped working. Reload to try again."
  - Announcements: "Opened ${title} in the side pane." (the existing text; `${title}` is the host) · "That address can't be opened in the side pane." · "Cleared the browser's data." · "Couldn't clear the browser's data."
  - Clear dialog: heading "Clear the browser's data?"; body "This signs you out of every site in Marshell's browser and clears its cookies, cache and site permissions. Your own browser isn't touched."; buttons "Clear data" and "Cancel".
  - Terminal tooltip on a web address where `browserTabs`: "Ctrl+click to open in the side pane".
  - A `browser` tab where `browserTabs` is false: body "Browser tabs aren't available on this system yet." with a button "Open in the system browser".
- **Styles.** Read the good-css skill before writing declarations. Tokens only, no hex. `:focus-visible` with `outline`; `:hover` only inside `@media (hover: hover) and (pointer: fine)`; forced-colors rules use system colours only; no boxed outlines; `SelectedMark` is the only selected-state marker and `Announcer` the only live region.
- **Keyboard and forced colours.** Everything new works with the keyboard alone and in forced colours. Tooltips on the URL bar are native `title`s: an HTML tooltip would be drawn behind the page.
- **Who writes what** (6q-3 decision 11). Tasks 3–6 (the page's Rust) are written in the main thread. Tasks 1, 2 and 7–12 go to Sonnet workers, `effort: high` where the heading says so; the main thread reads each diff and runs its tests.
- **Checks.** Rust: `cargo test -p <crate> -j 4`; `cargo fmt --all` before each Rust commit; `cargo clippy --workspace --all-targets -- -D warnings` at the end of each Rust task. After a protocol change: `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated`. Frontend: `pnpm test <file>`, and `pnpm build` at the end of every frontend task.
- **Commits** read `type(scope): what changed, as a sentence`.
- **Not in this slice:** `marshell open` and its route, the agent's mark on a tab, region to prompt on a page, saving passwords, a choice of search engine, a start page, the text editor, PDF features.

## Review Focus

1. **Text in the URL bar that is not a plain address:** `localhost:3000`, `192.168.1.5:8080/x`, `[::1]:5173`, `how to center a div`, `what is a::b`, `c:\work\readme.md`, `\\server\share`, `file:///C:/x`, `javascript:alert(1)`, `mailto:a@b.c`, `http://user:pw@example.com/`, `   `. Expected: an address opens (http for localhost and IP addresses, https otherwise); words search Google; a path or any other scheme is refused with the status line and never reaches Google; userinfo is refused; blank does nothing. Tests: Task 2 `addressFor` table.
2. **A page that sends itself somewhere it may not go:** a link or redirect to `file:`, `tauri://`, `http://tauri.localhost/`, `http://ipc.localhost/`, the dev server's origin, the local API's port, or `mailto:`/`vscode:`. Expected: refused, the page stays where it was, the status line says so, and no outside program starts. Tests: Task 4 `allowed_navigation` table; Task 13 check 7.
3. **A page that is gone under the UI:** WebView2's process dies, a page is closed by eviction while its tab is selected by another path, a command names a key with no page. Expected: the tab stays in the strip; shown again it loads its address; a command for a missing page answers an error the UI ignores. Tests: Task 4 `registry_*`; Task 7 `nextLive_*`; Task 8 `a_gone_page_loads_again_when_shown`.
4. **Fast switches:** a tab closed before its page finished opening; two opens of one key; a session switch while a page loads; 9 tabs shown in turn. Expected: at most one page per key, never a page whose tab is gone, never more than 8. Tests: Task 4 `registry_open_twice_makes_one_page`, `registry_close_before_created_leaves_none`; Task 7 `nextLive_evicts_the_page_shown_longest_ago`.
5. **Covers that overlap in time:** the palette opened and closed inside the capture's 40 ms; a menu opened over a frozen page; a cover with the page unloaded or hidden; a file dragged over the page while a dialog is open. Expected: the page is on screen exactly when nothing covers it and its tab is shown; a picture never stays after the last cover goes. Tests: Task 9 `freezeStep_*`, `cover_counts_*`.

---

### Task 1: `layout.json` holds a browser tab (worker)

**Files:**
- Modify: `crates/protocol/src/api.rs` (`PaneTab`)
- Modify: `crates/core/src/layout.rs` (`repair_pane`, `validate_pane`, tests), `crates/core/tests/layout_api.rs`
- Regenerated: `src/generated/PaneTab.ts`

**Interfaces:**
- Produces: `PaneTab` gains `#[serde(default)] pub title: Option<String>` (doc comment: "A browser tab's last title, for the strip before its page loads."). `kind` is `"file"` or `"browser"`.
- A `browser` tab is valid when: `history` has exactly one entry, `index` is 0, the entry's `path` is `about:blank` or starts with `http://` or `https://` and is at most 4096 chars, its `view` is `"rendered"`, `line` and `col` are `None`, and `title` is at most 200 chars. A `file` tab's `title` must be `None`.

- [ ] **Step 1: Write the failing tests** in `layout.rs`'s test module (helper `fn browser_tab(id: &str, url: &str) -> PaneTab`):
  - `repair_keeps_a_browser_tab`: tabs `t1` file, `t2` browser `https://example.com/` → both stay. Replace `repair_drops_a_tab_of_an_unknown_kind`'s `"browser"` with `"editor"`.
  - `repair_drops_a_browser_tab_with_a_bad_address`: addresses `file:///C:/x`, `javascript:alert(1)`, `` (empty), `ftp://x/` → each tab dropped; `about:blank` and `http://localhost:3000/` stay.
  - `repair_folds_a_browser_tab_to_one_entry`: a browser tab with 3 entries and index 2 → one entry (the one at `index`), index 0.
  - `repair_trims_a_long_title`: 300 chars → the first 200 (by `char`, not byte); a `file` tab's title → `None`.
  - `validate_view_names_the_browser_field`: two entries "history"; index 1 "index"; `file:///x` "path"; view `"source"` "view"; `line: Some(3)` "line"; a 201-char title "title"; a file tab with a title "title". Update the existing kind case (`"browser"` → `"editor"`).
  - In `layout_api.rs`, `put_view_round_trips_a_browser_tab`: PUT one session with a file tab and a browser tab titled "Example Domain"; GET returns the same.
  - Update the fixture at `layout.rs:942` (a `browser` tab with a `url` field and no history): it is still dropped, now for its empty history.

- [ ] **Step 2: Run them to see them fail** — `cargo test -p marshell-core -j 4 layout` → compile error for `title`, then FAIL.

- [ ] **Step 3: Implement** the field, `fn is_page_address(s: &str) -> bool`, and the two kinds in `repair_pane` and `validate_pane`. Error text names the field as the others do, e.g. `"{at}: a browser tab's path must be a web address"`.

- [ ] **Step 4: Regenerate** — `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated` → pass; `PaneTab.ts` shows `title: string | null`. Fix TS fixtures that build a `PaneTab` (`rg "kind: \"file\"" src`) by adding `title: null`.

- [ ] **Step 5: Run** `cargo test -p marshell-core -j 4 layout` · `cargo test -p marshell-core -j 4 --test layout_api --test layout_restore` · `pnpm test src/features/pane` · `pnpm build` → all pass.

- [ ] **Step 6: Commit** — `cargo fmt --all`; `feat(layout): layout.json holds a browser tab's address and title.`

---

### Task 2: Addresses, and browser tabs in the pane's state (worker `effort: high`: a parser)

**Files:**
- Create: `src/features/browser/address.ts`, `address.test.ts`
- Modify: `src/features/pane/paneState.ts`, `paneState.test.ts`, `src/features/pane/paneCommands.ts`

**Interfaces:**
- Produces (`address.ts`):

```ts
export const SEARCH_URL = "https://www.google.com/search?q=";
export const BLANK = "about:blank";
/** What typed text becomes. "open": a web address to load. "search": a Google search for the words. "refused": a path, another scheme, or userinfo. "empty": nothing typed. */
export type Typed = { kind: "open" | "search"; url: string } | { kind: "refused" } | { kind: "empty" };
export function addressFor(text: string): Typed;
/** True for an http or https URL with no userinfo, or about:blank: what a browser tab may hold and what the terminal, a chip or a link may ask for. */
export function isPageUrl(url: string): boolean;
/** "example.com", "localhost:3000"; "" for about:blank or a string that is no URL. */
export function hostOf(url: string): string;
/** The page's title, else its host, else "New tab". */
export function pageTitle(title: string | null | undefined, url: string): string;
/** Two addresses name one page: equal once parsed (so a missing trailing slash on a bare host and the host's case don't matter); the fragment counts. */
export function sameUrl(a: string, b: string): boolean;
```

- Produces (`paneState.ts`): actions `{ type: "openPage"; session: string; url: string }` (a new `browser` tab selected, or the tab whose address is `sameUrl`; `about:blank` always makes a new tab; shows the pane; refused at 32 tabs; never touches `recent`) and `{ type: "page"; session: string; paneTab: string; url: string; title: string | null }` (a browser tab's entry follows its page; the same state when nothing changed; ignored for a file tab or a `url` that fails `isPageUrl`; `title` cut to 200 chars). `openRefused(s, session, path)` treats a page URL with `sameUrl`. `tabTitles` gives a browser tab `pageTitle(t.title, url)` with no folder suffix. `recentFor` never lists a URL. `navigate`, `back`, `forward` and `setView` leave a browser tab alone. Export `isBrowserTab(t: PaneTab): boolean`.

- [ ] **Step 1: Write the failing tests.**
  - `address.test.ts`, `addressFor` as a table (input → result): `example.com` → open `https://example.com/`; `example.com/a b?q=1` → open `https://example.com/a%20b?q=1`; `localhost:3000` → open `http://localhost:3000/`; `localhost` → open `http://localhost/`; `127.0.0.1:8080/x` → open `http://127.0.0.1:8080/x`; `192.168.1.5` → open `http://192.168.1.5/`; `[::1]:5173` → open `http://[::1]:5173/`; `app.localhost:3000` → open `http://app.localhost:3000/`; `https://a.dev/x#y` → open unchanged; `HTTP://A.dev` → open `http://a.dev/`; `about:blank` → open `about:blank`; `how to center a div` → search `SEARCH_URL + "how%20to%20center%20a%20div"`; `rust` → search; `what is a::b` → search; `a.b c` → search; `c:\work\readme.md`, `C:/work/x`, `\\server\share`, `/usr/bin`, `~/notes.md`, `./x`, `file:///C:/x`, `javascript:alert(1)`, `mailto:a@b.c`, `vscode://x`, `about:config`, `data:text/html,x`, `http://user:pw@example.com/`, `user@example.com/x` → refused; `""` and `"   "` → empty. A single word with a dot and a known shape (`readme.md`) → open `https://readme.md/` (a browser does the same).
  - `isPageUrl`: true for `http://x/`, `https://x/`, `about:blank`; false for `file:///x`, `http://u@x/`, `about:config`, `x`, `""`.
  - `sameUrl("https://Example.com", "https://example.com/")` true; with different fragments false.
  - `pageTitle("", "https://a.dev/x")` → `a.dev`; `pageTitle(null, "about:blank")` → `New tab`.
  - `paneState.test.ts`: `openPage makes a browser tab and shows the pane`; `openPage of an address already open selects its tab`; `openPage of about:blank always makes a new tab`; `openPage is refused at 32 tabs`; `openPage leaves the recent files alone`; `a page event moves the tab's address and title`; `a page event with the same address and title returns the same state`; `a page event for a file tab is ignored`; `a browser tab's title is its page's, with no folder`; `back and forward leave a browser tab alone`.

- [ ] **Step 2: Run to see them fail** — `pnpm test src/features/browser/address.test.ts src/features/pane/paneState.test.ts`.

- [ ] **Step 3: Implement.** `addressFor` decides in this order: trim; empty; a Windows or POSIX path shape or a leading `~` → refused; text with whitespace → search; a scheme (`^[a-z][a-z0-9+.-]*:` not followed by a port-like `\d+(/|$)`) → `http`/`https` without userinfo and `about:blank` open, anything else refused; an `@` before the first `/` → refused; `localhost`, an IPv4 or bracketed IPv6 host (with optional port and path), or a host ending `.localhost` → `http://`; a host with a dot or a port → `https://`; otherwise search. Build URLs with `new URL`, and fall back to search when it throws.

- [ ] **Step 4: Run** the two test files and `pnpm build` → pass.

- [ ] **Step 5: Commit** — `feat(browser): typed text becomes an address or a search, and the pane's state holds browser tabs.`

---

### Task 3: Isolation: the capability, the app manifest and the window lookups (main thread)

**Files:**
- Modify: `src-tauri/Cargo.toml` (`tauri` features `["tray-icon", "unstable"]`; `windows` adds `Win32_Foundation`, `Win32_System_Com`, `Win32_System_Com_StructuredStorage`; `windows-sys` adds `Win32_UI_Input_KeyboardAndMouse`; `serde_json` moves to `[dependencies]`)
- Modify: `src-tauri/build.rs`, `src-tauri/capabilities/default.json`
- Modify: `src-tauri/src/main.rs` (opener, `close_once`, tests), `src-tauri/src/attention.rs:73,91`, `src-tauri/src/tray.rs:215`

**Interfaces:**
- Produces: `src-tauri/build.rs` holds `const COMMANDS: &[&str]`, every name in `generate_handler!`, passed to `tauri_build::try_build(Attributes::new().app_manifest(AppManifest::new().commands(COMMANDS)))`. `default.json` has `"webviews": ["main"]`, no `"windows"` key, and one `allow-<command-in-kebab-case>` permission per command. Later tasks add each new `browser_*` command to all three places.
- Produces: `pub fn main_window(app: &AppHandle) -> Option<tauri::Window>` in `main.rs` (`app.get_window("main")`), used by the badge, the flash, close to tray and the tray's show. The opener is `tauri_plugin_opener::Builder::new().open_js_links_on_click(false).build()`.

- [ ] **Step 1: Check the API before writing.** Read `tauri-build`'s `AppManifest` and the generated permission names in the docs (context7: `tauri` "app manifest commands capabilities") and in `~/.cargo/registry/src/*/tauri-build-*/src/`. S12 finding 14 says this route was never tried; if `AppManifest` does not put app commands under the capability, stop and report before changing anything else.

- [ ] **Step 2: Write the failing tests** in `main.rs`'s test module:
  - `every_command_is_granted_to_the_main_webview_only`: parse the names between `generate_handler![` and `]` in `include_str!("main.rs")` (the last path segment of each), the `COMMANDS` list in `include_str!("../build.rs")`, and the `allow-*` permissions without a `:` in `default.json`. The three sets are equal (kebab-cased for the third); `default.json` has `"webviews": ["main"]` and no `"windows"` key.
  - `no_capability_names_a_page`: no file in `src-tauri/capabilities/` contains `"remote"` or a webview label other than `main`.
  - `the_opener_leaves_page_links_alone`: `include_str!("main.rs")` contains `open_js_links_on_click(false)`.

- [ ] **Step 3: Run to see them fail** — `cargo test -p marshell-app -j 4`.

- [ ] **Step 4: Implement** the manifest, the capability, the opener and `main_window`; move the four lookups. `disable_browser_shortcuts` keeps `get_webview_window` (it runs at setup, before any page; say so in its comment).

- [ ] **Step 5: Run** `cargo test -p marshell-app -j 4` · `cargo clippy --workspace --all-targets -- -D warnings` · `pnpm tauri build --no-bundle` (the ACL is resolved at build time: a command missing from the capability fails here or at its first call). Expected: all pass.

- [ ] **Step 6: Commit** — `cargo fmt --all`; `feat(shell): the capability covers only the app's own webview, and Marshell's commands come under it.`

---

### Task 4: The page's rules: where it may go, whose key it is, where it sits (main thread)

**Files:**
- Create: `src-tauri/src/browser/mod.rs` (declares `pub mod rules;`), `src-tauri/src/browser/rules.rs`, `src-tauri/src/browser/codes.json` (a JSON array of every code `vk_to_code` can return, sorted; Task 10's frontend test reads the same file)
- Modify: `src-tauri/src/main.rs` (`mod browser;`)

**Interfaces:**
- Produces (`rules.rs`, all pure, no Tauri types except `tauri::Url`):

```rust
/// An origin a page may never open: the dev server's, the local API's.
pub type Own = (String, u16);
/// http and https only, plus about:blank. Refused: userinfo; a host ending ".localhost" on the scheme's default port
/// (Tauri's own tauri.localhost, ipc.localhost); any (host, port) in `own`, with "localhost", "127.0.0.1" and "[::1]" as one host.
pub fn allowed_navigation(url: &Url, own: &[Own]) -> bool;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)] pub struct Chord { pub code: String, pub ctrl: bool, pub shift: bool, pub alt: bool }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum KeyRoute { App, DevTools, Page }
/// F12 alone opens DevTools. Esc alone and Ctrl+L go to the app. A chord in `claimed` goes to the app. Everything else is the page's.
pub fn route_key(vk: u32, ctrl: bool, shift: bool, alt: bool, claimed: &HashSet<Chord>) -> KeyRoute;
pub fn vk_to_code(vk: u32) -> Option<&'static str>;   // letters, digits, F1–F24, arrows, Tab, Enter, Space, Escape, Backspace, Delete, Home, End, PageUp, PageDown, Insert, and the OEM punctuation keys
pub fn focus_out_direction(reason: i32) -> Option<&'static str>;  // "next" | "previous"

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)] pub struct Rect { pub x: f64, pub y: f64, pub width: f64, pub height: f64 }
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)] pub struct Size { pub width: f64, pub height: f64 }
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)] #[serde(rename_all = "camelCase")] pub struct Piece { pub from: f64, pub x: f64, pub x_per_px: f64, pub width: f64, pub width_per_px: f64 }
#[derive(Clone, Debug, Default, PartialEq, Deserialize)] pub struct Rule { pub pieces: Vec<Piece>, pub top: f64, pub bottom: f64 }
#[derive(Clone, Debug, Deserialize)] pub struct Report { pub rect: Rect, pub win: Size, pub rule: Option<Rule>, pub moving: bool }

pub const HOLD_MS: u64 = 250;
#[derive(Default)] pub struct Placer { /* rule, last box, last piece, hold-until */ }
impl Placer {
    /// The window resized to `win` (logical px). None while a motion is held, or when the width crossed into another piece
    /// (which starts a hold): the app's own frames place the page until the motion ends. Otherwise the rule's box.
    pub fn on_resize(&mut self, win: Size, now_ms: u64) -> Option<Rect>;
    /// A report from the app. `moving` holds (again, for HOLD_MS) and takes the report's box as measured. Otherwise the hold
    /// ends, and the box is the report's when it was measured at the window's size now (`win_now`, compared in physical
    /// pixels at `scale`, within 1), else the rule's box for `win_now`, else the report's.
    pub fn on_report(&mut self, report: Report, win_now: Size, scale: f64, now_ms: u64) -> Rect;
    pub fn last(&self) -> Option<Rect>;
}

/// Which pages exist, by key: the label of each page's webview, and which one is shown.
#[derive(Default)] pub struct Registry { /* key → label, next number, shown */ }
impl Registry {
    /// The label for a new page ("page-1", "page-2", …), or None when the key already has one.
    pub fn open(&mut self, key: &str) -> Option<String>;
    pub fn label(&self, key: &str) -> Option<&str>;
    pub fn key_of(&self, label: &str) -> Option<&str>;
    /// Forgets the key; its label, if it had one.
    pub fn close(&mut self, key: &str) -> Option<String>;
    /// Sets the shown page; the labels to hide and the one to show. A key with no page shows nothing.
    pub fn show(&mut self, key: Option<&str>) -> (Vec<String>, Option<String>);
    pub fn shown(&self) -> Option<&str>;
    pub fn labels(&self) -> Vec<String>;
}
pub fn base64(bytes: &[u8]) -> String;
```

- [ ] **Step 1: Write the failing tests** in `rules.rs`:
  - `allowed_navigation`, with `own = [("localhost", 1420), ("127.0.0.1", 49152)]`: allowed `https://example.com/`, `http://localhost:3000/`, `http://app.localhost:3000/`, `http://127.0.0.1:8080/`, `about:blank`; refused `file:///C:/Windows/win.ini`, `tauri://localhost/`, `http://tauri.localhost/`, `https://tauri.localhost/`, `http://ipc.localhost/`, `http://TAURI.LOCALHOST/`, `http://localhost:1420/`, `http://127.0.0.1:1420/x`, `http://[::1]:1420/`, `http://localhost:49152/v1/health`, `http://user:pw@example.com/`, `about:config`, `javascript:alert(1)`, `mailto:a@b.c`, `vscode://x`, `data:text/html,x`, `blob:https://a/1`, `ws://a/`.
  - `route_key`, with `claimed = {KeyK+ctrl+shift, Tab+ctrl, KeyT+ctrl+shift+alt}`: Ctrl+Shift+K App; Ctrl+Tab App; Ctrl+Shift+Alt+T App; Ctrl+Shift+V Page; Ctrl+Shift+Z Page; Ctrl+Alt+Q Page (AltGr); F5, Ctrl+R, Ctrl+F, Ctrl+P Page; Esc App; Shift+Esc Page; Ctrl+L App; Ctrl+Shift+L Page; F12 DevTools; Ctrl+F12 Page; a vk with no code Page.
  - `vk_to_code`: `0x41` KeyA, `0x30` Digit0, `0x74` F5, `0x09` Tab, `0x25` ArrowLeft, `0xBF` Slash, `0x07` None. `the_codes_file_lists_every_code`: the set of `vk_to_code(vk)` over `0..=0xFF` equals `codes.json` (`include_str!`).
  - Placer, with a two-piece rule (under 960: `x = 48 + (w−48)/2`; from 960: `x = 280 + (w−280)/2`, each width the other half; top 80, bottom 24): `a_resize_inside_a_piece_follows_the_rule` (900 → 880: `x` 464, width 416, y 80, height `h − 104`); `a_resize_across_a_piece_holds` (960 → 950 gives None, and so does the next resize 100 ms later); `the_hold_ends_after_250_ms` (a resize at +251 ms places); `a_moving_report_is_taken_as_measured_and_holds`; `a_still_report_ends_the_hold`; `a_report_at_the_windows_size_is_taken_as_measured` (logical sizes 0.8 apart at scale 1.25 still count as the same); `a_report_at_another_size_gets_the_rule`; `without_a_rule_the_report_is_followed`; `a_tiny_window_never_gives_a_negative_size`; `the_first_resize_after_a_rule_never_holds` (no last piece yet).
  - Registry: `registry_open_twice_makes_one_page`; `registry_close_before_created_leaves_none` (open, close, `labels()` empty, `show(key)` shows nothing); `registry_show_hides_the_others`; `registry_closing_the_shown_page_shows_nothing`; `registry_labels_are_never_reused`.
  - `base64` against three known vectors (`""`, `"f"` → `Zg==`, `"foobar"` → `Zm9vYmFy`).

- [ ] **Step 2: Run to see them fail** — `cargo test -p marshell-app -j 4 browser`.
- [ ] **Step 3: Implement** `rules.rs`.
- [ ] **Step 4: Run** `cargo test -p marshell-app -j 4 browser` · clippy → pass.
- [ ] **Step 5: Commit** — `cargo fmt --all`; `feat(browser): the rules for a page's addresses, keys and place.`

---

### Task 5: Pages: open, place, navigate, close (main thread)

**Files:**
- Modify: `src-tauri/src/browser/mod.rs`; create `src-tauri/src/browser/com.rs` (`#[cfg(windows)]`)
- Modify: `src-tauri/src/main.rs` (state, `on_window_event`, the exit path, `generate_handler!`), `src-tauri/build.rs`, `src-tauri/capabilities/default.json`

**Interfaces:**
- Consumes: Task 4's `rules`; Task 3's `main_window`.
- Produces: `browser::BrowserState` (managed: the profile's folder `<home>/browser-profile`, `Mutex<Registry>`, `Mutex<Placer>`, `Mutex<HashSet<Chord>>`, the `own` origins: the dev URL's host and port from `app.config().build.dev_url`, and the core's API port once the supervisor has started).
- Produces: commands, each `#[tauri::command] pub async fn …(app: AppHandle, …) -> Result<…, String>`, each added to `generate_handler!`, `COMMANDS` and the capability:
  - `browser_open(key: String, url: String)`: refuses with `Err("refused")` (and a `browser://refused` event) an address `allowed_navigation` refuses. Navigates the key's page when it exists. Otherwise creates the page with `Window::add_child` at the placer's last box (0×0 at the origin when there is none), hidden, with `.focused(false)`, `.devtools(false)`, `.data_directory(profile)`, `.additional_browser_args("--disable-features=msWebOOUI,msPdfOOUI")`, `on_navigation` (the allow-list; a refusal emits `browser://refused`) and `on_new_window` (emits `browser://popup` and answers `Deny`).
  - `browser_show(key: Option<String>)`: hides every other page, shows this one at the placer's last box.
  - `browser_place(report: Report)`: on the main thread, `Placer::on_report` with the window's inner size and scale now, then `set_bounds` on the shown page.
  - `browser_nav(key: String, action: String, url: Option<String>)`: `"go"` (through the allow-list), `"back"`, `"forward"`, `"reload"`, `"stop"`.
  - `browser_close(key: String)`: closes the page's webview and forgets the key. Not an error when the key has no page.
- Produces: events to the app's webview only (`app.emit_to("main", …)`), payloads camelCase with the page's `key`: `browser://state` `{ key, url, title, canBack, canForward, loading }` (on `HistoryChanged`, `SourceChanged`, `DocumentTitleChanged`, `NavigationStarting`, `NavigationCompleted`, and once when the handlers are installed); `browser://popup` `{ key, url }`; `browser://refused` `{ key, url }`; `browser://gone` `{}` (on `ProcessFailed` with the browser process exited: every page is closed and the registry emptied first).
- Produces: `browser::on_resized(window: &tauri::Window, size: PhysicalSize<u32>)`, called from `on_window_event` for `WindowEvent::Resized` on `main`: `Placer::on_resize`, then `set_bounds` on the shown page. `browser::close_all(app: &AppHandle)`, called before every `app.exit(0)` and in `RunEvent::Exit`: closes every page's webview, then waits up to 2 s for the profile's browser process to exit (S12 finding 12; a no-op with no page).
- `WindowEvent::Focused(false)` on `main` no longer calls `window_blurred()` when the foreground window is still Marshell's (`GetForegroundWindow` equals the window's HWND): a page taking the keyboard inside the window is not the window losing focus.

- [ ] **Step 1: Write the failing tests** (pure parts only; the rest is Task 13's):
  - `mod.rs`: `the_own_origins_name_the_dev_server_and_the_api` (`own_origins(Some("http://localhost:1420/"), Some(49152))` → `[("localhost", 1420), ("127.0.0.1", 49152)]`; `own_origins(None, None)` → empty).
  - `main.rs`: extend `every_command_is_granted_to_the_main_webview_only` by running it (it fails until the five commands are in all three lists).

- [ ] **Step 2: Implement.** Read `phase-6q-0`'s `spike_browser/mod.rs` and `com.rs` first; carry the patterns, drop the modes, the JSONL log, oversize and clip. `com.rs` handlers only read their event and post with `run_on_main_thread`; every `unsafe` block has its `// SAFETY:` line. `set_bounds` takes logical positions and sizes and never a negative size. A page's commands look its label up in the registry under the lock and release the lock before any `with_webview` call.

- [ ] **Step 3: Run** `cargo test -p marshell-app -j 4` · clippy · `pnpm tauri build --no-bundle` → pass.

- [ ] **Step 4: Commit** — `cargo fmt --all`; `feat(browser): the shell opens, places, navigates and closes pages.`

---

### Task 6: Pages: keys, focus, the picture, the theme, the profile's data (main thread)

**Files:**
- Modify: `src-tauri/src/browser/mod.rs`, `com.rs`, `src-tauri/src/main.rs`, `src-tauri/build.rs`, `src-tauri/capabilities/default.json`

**Interfaces:**
- Produces: commands (registered in all three lists):
  - `browser_keys(chords: Vec<Chord>)`: replaces the claimed set.
  - `browser_focus(key: String, reason: String)`: `MoveFocus` with `"next"`, `"previous"` or `"programmatic"`.
  - `browser_capture(key: String) -> String`: the page as WebView2 last drew it, a `data:image/png;base64,` URL. `Err("hidden")` at once for a page that is not shown; `Err` after 5 s.
  - `browser_theme(dark: bool)`: the profile's `PreferredColorScheme`, kept in the state and applied to each page as it is created.
  - `browser_clear_data()`: `ICoreWebView2Profile2::ClearBrowsingDataAll` on the profile, then `Reload` on every page. With no page open it creates one hidden `about:blank` page, clears, and closes it.
- Produces: events: `browser://focus` `{ key, focused }` (`GotFocus`, `LostFocus`); `browser://focus-out` `{ key, direction }` (`MoveFocusRequested` with `Handled(true)`, after moving focus to the app's webview); `browser://key` `{ key, code, ctrl, shift, alt, repeat }` (a key `route_key` gives the app, after moving focus to the app's webview); `browser://icon` `{ key, src }` (`FaviconChanged`, then `GetFavicon` as PNG; `src` is a `data:` URL, or null when there is none or it is over 64 KB); `browser://drag` `{ key, kind, paths }` (`kind` `"enter"`, `"drop"` or `"leave"`; `paths` only on a drop), from the builder's drag-drop handler for the page.
- F12 calls `OpenDevToolsWindow` on the page, in release builds too (`SetAreDevToolsEnabled(true)` on the page only).
- The fixes S12 lists: the `taken` set of key-ups is cleared on `GotFocus`; the lock is released before focus moves, and the move is posted, not run inside the key callback.
- Permissions: every `PermissionRequested` ends with WebView2's own prompt (6q-3 decision 6). Read wry 0.57's handler in `~/.cargo/registry/src/*/wry-0.57.0/src/webview2/mod.rs`; if it allows clipboard reading without asking, add a handler after it that sets the state back to `COREWEBVIEW2_PERMISSION_STATE_DEFAULT`.
- Downloads keep WebView2's defaults (6q-3 decision 5): no handler.

- [ ] **Step 1: Write the failing tests:** in `rules.rs`, `key_event(vk, ctrl, shift, alt, was_down) -> Option<KeyEvent>` (`a_held_key_is_a_repeat`, `a_key_with_no_code_is_not_forwarded`); `icon_src(png: &[u8]) -> Option<String>` (`an_icon_over_64_kb_is_dropped`, `an_empty_icon_is_none`). Run `every_command_is_granted_…` again.

- [ ] **Step 2: Implement.** Reach `ICoreWebView2_13::Profile` by `cast`; where a cast fails (an old runtime), log once and go on without that feature.

- [ ] **Step 3: Run** `cargo test -p marshell-app -j 4` · clippy · `pnpm tauri build --no-bundle` → pass.

- [ ] **Step 4: Commit** — `cargo fmt --all`; `feat(browser): a page's keys, focus, picture, theme and data.`

---

### Task 7: The frontend's side of a page: the calls, the page's state, the loaded set, the rule (worker)

**Files:**
- Create in `src/features/browser/`: `browserApi.ts`, `pageStore.ts`, `pages.ts`, `pageRule.ts`, `placing.ts`, and a `.test.ts` for `pageStore`, `pages`, `pageRule`, `placing`
- Create: `src/features/window/paneColumn.ts`, `paneColumn.test.ts`; modify `src/features/window/MainWindow.tsx` to take its room from it

**Interfaces:**
- Consumes: Task 5 and 6's commands and events (exact names and payloads above); `paneGeometry`, `sidebarMode`, `sidebarWidth`, `drawerPlacement`, `DRAWER`.
- Produces (`paneColumn.ts`):

```ts
export type ColumnInputs = { pref: SidebarPref; sidebarWidth?: number; drawer: boolean; shown: boolean; saved: number | null; terminalMin: number; rtl: boolean };
/** The main area in a window `width` wide: what MainWindow computes today as mode, sideW, pushed and spaceW. */
export function mainRoom(width: number, i: Pick<ColumnInputs, "pref" | "sidebarWidth" | "drawer">): { mode: SidebarMode; side: number; pushed: number; room: number };
/** The pane's column in window coordinates: the pane sits at the main area's end edge (its left in a right-to-left window). Width 0 when hidden. */
export function paneColumn(width: number, i: ColumnInputs): { x: number; width: number };
```

- Produces (`pageRule.ts`): `export type Piece = { from: number; x: number; xPerPx: number; width: number; widthPerPx: number }` and `export function pageRule(i: ColumnInputs, inset: { start: number; end: number }, maxWidth = 8192): Piece[]` (the pane's column, less the slot's distance from the column's two edges, as straight pieces over whole window widths: a new piece wherever the line so far stops matching).
- Produces (`browserApi.ts`): the types `Chord`, `Rect`, `Size`, `Rule` and `Report`, field for field as Task 4's Rust structs deserialize them; `pageKey(session: string, paneTab: string): string`; `browser.open(key, url)`, `.show(key | null)`, `.place(report)`, `.nav(key, action, url?)`, `.close(key)`, `.keys(chords)`, `.focus(key, reason)`, `.capture(key)`, `.theme(dark)`, `.clearData()`: each a promise from `invoke`, all through one queue so they run in the order asked (StrictMode), and each a resolved no-op outside Tauri (`inTauri()`). `onBrowser(event, handler): () => void` wraps `listen`.
- Produces (`pageStore.ts`): `type PageState = { url: string; title: string; canBack: boolean; canForward: boolean; loading: boolean; icon: string | null; focused: boolean; status: "refused" | "failed" | null }`; `pageReducer(pages: ReadonlyMap<string, PageState>, e: PageEvent): ReadonlyMap<string, PageState>` (events `state`, `icon`, `focus`, `refused`, `closed`, `gone`); `usePage(key: string | null): PageState | undefined`; `focusedPage(): string | null` (the key of the page that has the keyboard); `startPageStore(): () => void` (subscribes to the `browser://` events once).
- Produces (`pages.ts`): `export const LIVE_PAGES = 8`; `nextLive(prev: readonly string[], shown: string | null, open: ReadonlySet<string>): { live: string[]; close: string[] }` (`live` is most recently shown first: `shown` first when it is in `open`, then `prev` without closed keys, at most 8; `close` is every key in `prev` not in `live`).
- Produces (`placing.ts`): `createPlacer(send: (r: Report) => Promise<void>, nextPaint: () => Promise<void>): { report(measure: () => Report): void }`. At most one report waits behind the one in flight, and it measures when its turn comes (S12 finding 20). A report at the same window size as the last one is sent after the app paints that frame (finding 21); a report at a new size goes at once.

- [ ] **Step 1: Write the failing tests.**
  - `paneColumn.test.ts`: `mainRoom matches the window's arithmetic` (widths 800, 959, 960, 1400 with `pref` expanded, rail and focus, drawer on and off: the same `mode`, `side`, `pushed`, `room` as `sidebarMode`, `widthFor`, `drawerPlacement` give); `the pane sits at the end edge` (1400 wide, expanded, saved 480 → `x = 1400 − 480`, width 480); `in a right-to-left window it sits at the start of the screen` (`x` = the pushed drawer's width, 0 without one); `under 720 px of room the pane is the room`; `a hidden pane has no width`.
  - `pageRule.test.ts`: `the rule has a piece per stretch` (expanded, saved null: pieces start where the room crosses 720 and where the window crosses 960); `every whole width from 400 to 2000 lands within 1e-6 of paneColumn less the insets`; `a saved width held by the 80-column stop makes its own piece`.
  - `pages.test.ts`: `nextLive puts the shown page first`; `nextLive_evicts_the_page_shown_longest_ago` (9 keys shown in turn → the first is in `close`); `nextLive drops a page whose tab is gone`; `nextLive with nothing shown keeps the order`; `showing a page already first changes nothing` (same arrays by value, `close` empty).
  - `pageStore.test.ts`: `a state event makes the page`; `an icon event before any state is kept`; `refused sets the status and the next state event clears it`; `gone empties the store`; `closed removes one page`; `only one page is focused at a time`.
  - `placing.test.ts` (fake `send` and `nextPaint`): `a second report while one is in flight waits and measures late`; `three reports in flight send two`; `a report at the same size waits for the paint`; `a report at a new size goes at once and drops the one waiting for a paint`.

- [ ] **Step 2: Run to see them fail** — `pnpm test src/features/browser src/features/window/paneColumn.test.ts`.
- [ ] **Step 3: Implement.** `MainWindow` takes `mode`, `sideW`, `pushed` and `spaceW` from `mainRoom` (no behaviour change; its tests still pass).
- [ ] **Step 4: Run** the tests above, `pnpm test src/features/window`, `pnpm build` → pass.
- [ ] **Step 5: Commit** — `feat(browser): the app's calls to a page, its state, the eight loaded pages and the layout rule.`

---

### Task 8: The browser tab: the URL bar and the page's slot (worker `effort: high`)

**Files:**
- Create in `src/features/browser/`: `BrowserTab.tsx`, `UrlBar.tsx`, `BrowserTab.css`, `usePageSlot.ts`, `BrowserTab.test.tsx`, `UrlBar.test.tsx`
- Modify: `src/features/pane/SidePane.tsx` (`StripTab` gains `icon?: string | "globe"`; the strip draws it before the title), `SidePane.css`, their tests; `src/features/window/MainWindow.tsx` (publishes `ColumnInputs` and `layoutMoving` through a context `LayoutContext` from `paneColumn.ts`)

**Interfaces:**
- Consumes: Task 7's `browser`, `usePage`, `pageRule`, `createPlacer`, `ColumnInputs`; Task 2's `addressFor`, `pageTitle`; Task 9's `usePageCovered` and `freezeStep` (Task 9 may land first; if not, stub `usePageCovered` as `() => false` and Task 9 replaces it).
- Produces: `<BrowserTab pageKey url active onPage onOpenOutside onEscape />`, where `active` is "this tab is the one on screen", `onPage(url, title)` reports the page's address and title for the layout, and `onOpenOutside(url)` opens the system browser.
- Produces: `<UrlBar page onGo onNav onOpenOutside ref />` with an imperative `focusAddress()` (focus and select). Back, Forward, Reload/Stop and "Open in the system browser" are icon buttons (6q-2 decision 11), each with `aria-label` and `title`; Back and Forward are `disabled` from `canBack` and `canForward`; the open-outside button is disabled on `about:blank`. Enter in the box runs `addressFor`: `open` and `search` call `onGo(url)`; `refused` shows the status line and announces it; `empty` does nothing. Esc in the box restores the page's address and calls `onEscape`. The box shows the page's address unless it has focus and was edited.
- Produces (`usePageSlot.ts`): `usePageSlot(ref, pageKey, active, covered)`: opens the page when the tab is first active (`browser.open(key, url)`, where `about:blank` is opened too), shows it while `active && !covered`, and reports its box. A report is `{ rect: the slot's box, win: the size of `.window`, rule: { pieces: pageRule(inputs, insets), top, bottom }, moving: layoutMoving }`, sent on the slot's `ResizeObserver`, on any change of `ColumnInputs` or `layoutMoving`, on a `devicePixelRatio` change (`matchMedia`), and once per frame while `layoutMoving`. Never from the window's `resize` event (S12 finding 17). On unmount or `!active` it calls `browser.show(null)` only if this page is still the shown one; it never closes the page (Task 11 owns that).
- The slot: `<div class="browser-tab__slot" tabIndex={0} aria-label="Web page">`. While frozen it holds the picture (`<img alt="">`, `object-fit: fill`) or, when the capture failed, a plain fill in the pane's background.
- `a_gone_page_loads_again_when_shown`: when `usePage(key)` is undefined for an active tab (never opened, evicted, or after `browser://gone`), the hook opens it at the tab's saved address.

- [ ] **Step 1: Write the failing tests** (`renderToStaticMarkup`):
  - `UrlBar.test.tsx`: `the bar has four icon buttons with labels and titles`; `Back and Forward follow the page's history`; `Reload reads Stop while the page loads`; `open outside is disabled on a blank page`; `the box shows the page's address`; `the placeholder reads "Search Google or type an address"`; `no button has visible text` (each button's text content is empty; an `svg[aria-hidden]` inside).
  - `BrowserTab.test.tsx`: `the slot is focusable and labelled "Web page"`; `a refused address shows the status line`; `a failed page shows "This page stopped working. Reload to try again."`; `where browser tabs aren't available the body says so and offers the system browser`.
  - A pure helper `slotStep(page: PageState | undefined, active: boolean, covered: boolean): "open" | "show" | "hide" | "none"` in `usePageSlot.ts`: `a_gone_page_loads_again_when_shown` (no page and active → `open`); `a page that exists and is active and uncovered is shown`; `a covered or inactive page is hidden`; `a tab that was never active opens nothing`.
  - A pure helper `slotReport(slot: DOMRectLike, win: DOMRectLike, inputs, moving)` in `usePageSlot.ts`: `the report's rule ends where the slot ends` (top = slot.y, bottom = win.height − slot.bottom, insets from the column); `a slot with no size reports a zero box, not NaN`.
  - `SidePane.test.tsx`: `a browser tab shows its site's icon, or a globe`; `a file tab shows no icon`.

- [ ] **Step 2: Run to see them fail** — `pnpm test src/features/browser src/features/pane`.
- [ ] **Step 3: Implement.** Read the good-css skill first. The bar is one row: the three buttons, the box taking the rest, the open-outside button. The icon in the strip is 14 px, `<img alt="">` for a site's icon and an inline `svg` globe otherwise.
- [ ] **Step 4: Run** the tests, `pnpm test src/styles src/components/a11y` (the no-boxed-outlines and shared-block scans), `pnpm build` → pass.
- [ ] **Step 5: Commit** — `feat(browser): the browser tab's URL bar and the slot its page sits over.`

---

### Task 9: The freeze: a picture stands in while anything covers the page (worker `effort: high`: concurrency)

**Files:**
- Create: `src/features/browser/pageCover.ts`, `pageCover.test.ts`, `freeze.ts`, `freeze.test.ts`, `coverScan.test.ts`
- Modify: every overlay that can draw over the pane: `src/components/ContextMenu/ContextMenu.tsx`, `src/features/appMenu/AppMenuPopup.tsx`, `src/features/tray/TrayMenu.tsx`, `src/features/palette/Palette.tsx`, `src/features/launcher/Launcher.tsx`, `src/features/settings/LiveSettings.tsx`, `src/features/sessions/LiveShortcuts.tsx`, `src/features/departures/LiveDepartures.tsx`, `src/features/doctor/LiveDoctor.tsx`, `src/features/doctor/FixSheet.tsx`, `src/features/history/ExportDialog.tsx`, `src/features/scripts/ScriptsTrustDialog.tsx`, `src/features/tasks/ReviewSheet.tsx`, `src/features/window/CloseDialog.tsx`, `src/features/window/NavCursor.tsx`; `src/features/drop/useFileDrop.ts` (a file drag over the window covers); `src/features/browser/usePageSlot.ts`

**Interfaces:**
- Produces (`pageCover.ts`): `coverPage(): () => void` (adds one cover; the returned function removes it, once); `pageCovered(): boolean`; `usePageCovered(): boolean` (`useSyncExternalStore`); `useCoverPage(on = true): void` (covers while mounted and `on`).
- Produces (`freeze.ts`), the sequencing as a pure step so the races are testable:

```ts
export type Freeze = { phase: "live" | "capturing" | "frozen" | "thawing"; seq: number; picture: string | null };
export type FreezeEvent = { type: "cover" } | { type: "uncover" } | { type: "captured"; seq: number; picture: string | null } | { type: "painted"; seq: number } | { type: "shown"; seq: number };
export type FreezeEffect = "capture" | "hide" | "show" | "dropPicture";
/** live → (cover) capturing → (captured) picture on screen → (painted) hide → frozen → (uncover) show → (shown) dropPicture → live. An event with an old `seq` does nothing. */
export function freezeStep(s: Freeze, e: FreezeEvent): { next: Freeze; effects: FreezeEffect[] };
```

  The order is fixed by S12: capture first, hide only after the picture has painted; on thaw, show first and drop the picture after the next paint. A failed capture (`picture: null`) still hides the page, over a plain fill. A page that is not shown (its tab hidden, or unloaded) skips the capture.
- `usePageSlot` runs `freezeStep` with `usePageCovered()`; while frozen the slot shows the picture.

- [ ] **Step 1: Write the failing tests.**
  - `pageCover.test.ts`: `cover_counts_two_covers_and_one_release_still_covers`; `a release called twice removes one cover`; `subscribers hear only a change of covered`.
  - `freeze.test.ts`: `freezeStep_a_cover_captures_then_hides_after_the_paint`; `freezeStep_an_uncover_during_the_capture_goes_back_to_live_and_the_late_capture_does_nothing`; `freezeStep_a_second_cover_while_frozen_changes_nothing`; `freezeStep_cover_uncover_cover_ends_frozen_with_the_newest_picture`; `freezeStep_a_failed_capture_still_hides`; `freezeStep_thaw_shows_before_dropping_the_picture`; `freezeStep_a_stale_shown_keeps_the_picture` (a cover arrived between `show` and `shown`).
  - `coverScan.test.ts`: every `.tsx` under `src/` (stories and tests aside) that contains `aria-modal="true"`, `role="menu"` or `role="dialog"` also contains `useCoverPage(`; the test lists the files that don't.

- [ ] **Step 2: Run to see them fail** — `pnpm test src/features/browser`.
- [ ] **Step 3: Implement.** `useFileDrop` covers from a drag's `enter` to its `leave` or `drop`, and also on a `browser://drag` `enter` from a page (the drag entered over the page; once the page is hidden the main webview hears the rest). `NavCursor` covers while it is drawn.
- [ ] **Step 4: Run** `pnpm test src/features/browser src/features/drop src/components` · `pnpm build` → pass.
- [ ] **Step 5: Commit** — `feat(browser): a page freezes as a picture while a menu, a dialog or a drag covers it.`

---

### Task 10: Keys and focus between the app and a page (worker `effort: high`)

**Files:**
- Create: `src/features/browser/claimedChords.ts`, `claimedChords.test.ts`, `useBrowserKeys.ts`, `pageFocus.ts`, `pageFocus.test.ts`
- Modify: `src/features/browser/BrowserTab.tsx`, `src/features/pane/focusPane.ts`, `src/features/sessions/LiveApp.tsx` (mounts `useBrowserKeys`), `src/features/sessions/useNavigation.ts` (`focusKindOf`), and every reader of "does the window have focus" (`rg "hasFocus\(\)|addEventListener\(\"(blur|focus)\"" src`), `useViewReport` included

**Interfaces:**
- Consumes: `browser.keys`, `browser.focus`, the `browser://key`, `browser://focus`, `browser://focus-out` events; `focusedPage()`; `claimsKey`, `sessionShortcut`, the perf toggle's test (the three checks `TerminalView.tsx:214-225` makes).
- Produces (`claimedChords.ts`): `claimedChords(claims: (e: KeyEventLike) => boolean): Chord[]`, which tries every code Rust's `vk_to_code` knows with each of the 8 Ctrl/Shift/Alt combinations and returns those `claims` takes; and `appClaims(os, focus)`: the same predicate the terminal's key handler uses. `useBrowserKeys` sends the list with `browser.keys` on mount and whenever the navigation state or the keymap changes (6q-0 decision 9: the navigation chords count only once navigation has started).
- Produces (`pageFocus.ts`), pure:
  - `tabNeighbour(all: T[], slot: T, direction: "next" | "previous"): T | undefined` (the app's element on that side of the slot, wrapping).
  - `enterReason(slotIsBefore: boolean): "next" | "previous"` (focus arriving at the slot from an element after it enters the page at its end).
  - `handBack(s: { pageHadKeys: boolean; pointerSince: boolean; covered: boolean; docHasFocus: boolean }): boolean` (after the window comes back to the front: true when the page had the keyboard, nothing in the app was pressed since, and nothing covers the page).
- Behaviour:
  - **Tab in:** the slot's `focus` event, when focus came from an app element, calls `browser.focus(key, enterReason(…))`. **Tab out:** `browser://focus-out` focuses `tabNeighbour(…)`.
  - **A forwarded key** (`browser://key`): Ctrl+L focuses the URL bar's box and selects it. Esc focuses the terminal (as Esc in a file tab does). Anything else: focus the slot without entering the page, dispatch a synthetic `keydown` (`code`, `key`, the modifiers, `repeat`, `bubbles`, `cancelable`) on it so the existing keymap acts, and on the next frame, if focus is still on the slot and nothing covers the page, give the keyboard back with `browser.focus(key, "programmatic")`.
  - **After a cover goes:** if the page had the keyboard when the cover came and focus is on the slot or on `body`, the page gets it back.
  - **The window comes back** (Alt+Tab, a click on the title bar's drag area): 100 ms after the window's `focus` event, `handBack(…)` decides (S12 3b, finding 4). A `pointerdown` in the app outside a drag region clears `pageHadKeys`.
  - **"Where is the keyboard":** `focusKindOf` and every focus reader treat `focusedPage() !== null` as "in the side pane, not in a terminal", and the window as focused (S12 finding 8: `document.hasFocus()` is false while a page has the keys). `focusPane()` on a browser tab focuses the slot, which enters the page.

- [ ] **Step 1: Write the failing tests.**
  - `claimedChords.test.ts`: `the list holds exactly what the predicate claims` (a predicate claiming Ctrl+Shift+K and Ctrl+Tab → those two); `every code Rust maps is tried` (the code table is Task 4's `src-tauri/src/browser/codes.json`, imported by the test and by `claimedChords.ts`; Task 4's Rust test holds the same file, so the two can't drift); `with the app's own predicate, Ctrl+Shift+K is claimed and Ctrl+Shift+V, Ctrl+Shift+Z, Ctrl+L and Escape are not` (Ctrl+L and Esc are Rust's own rule); `the navigation chords are claimed only once navigation has started`.
  - `pageFocus.test.ts`: `tabNeighbour wraps at both ends`; `focus from an element after the slot enters at the page's end`; `handBack` truth table (true only for `{true, false, false, true}`).

- [ ] **Step 2: Run to see them fail** — `pnpm test src/features/browser`.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `pnpm test src/features/browser src/features/sessions src/features/pane` · `cargo test -p marshell-app -j 4 browser` (the shared fixture) · `pnpm build` → pass.
- [ ] **Step 5: Commit** — `feat(browser): the app's shortcuts work from a page, and the keyboard moves between the two.`

---

### Task 11: The app opens web addresses in browser tabs (worker `effort: high`: it rewires `LiveApp`)

**Files:**
- Create: `src/features/browser/useBrowserPages.ts`, `ClearDataDialog.tsx`, `ClearDataDialog.test.tsx`
- Modify: `src/features/sessions/LiveApp.tsx`, `src/features/pane/plusMenu.ts` and its test, `src/features/pane/paneCommands.ts`, `src/lib/keymap.ts`, `src/lib/appShortcuts.ts`, `src/features/terminal/urlLinks.ts` and `TerminalView.tsx`, `src/features/ports/ports.ts` and `PortChips.tsx`, `src/features/preview/Preview.tsx` and `WebLinkBar.tsx`, `src/features/drop/dropTarget.ts`, `src/features/pane/KeptTabs.tsx`

**Interfaces:**
- Consumes: Tasks 2, 7, 8, 9, 10.
- Produces: `openPageInPane(url: string, opts?: { focus: "page" | "address" }): boolean` in `LiveApp`, beside `openInPane`: refuses a URL that fails `isPageUrl` ("That address can't be opened in the side pane."), a missing session and the 33rd tab (the existing texts); dispatches `openPage`; announces "Opened ${hostOf(url)} in the side pane."; moves the keyboard to the page, or to the address box for a new blank tab. `PaneContext` carries it beside the file opener.
- Behaviour:
  - Where `browserTabs`: Ctrl+click on a web address in the terminal, a port chip's click, and a web link in a Markdown tab call `openPageInPane`. `WebLinkBar`'s question goes; the link opens at once. The port chip's menu keeps its rows and gains "Open in the system browser". Where `browserTabs` is false, all three keep `openUrl`.
  - History's web links keep `openUrl` (the pane is hidden there).
  - The + menu's first row is "New browser tab"; the palette gets "New browser tab", "Open URL…" (both `openPageInPane("about:blank", { focus: "address" })`) and "Clear the browser's data…" (opens `ClearDataDialog`; "Clear data" calls `browser.clearData()` and announces the result).
  - `KeptTabs`' `render` draws `<BrowserTab>` for a browser tab and `<Preview>` for a file tab. A browser tab's panel stays mounted like any kept tab; its page's life is `useBrowserPages`' business, not the panel's.
  - `useBrowserPages(pane: PaneView, shownKey: string | null)`: keeps the loaded list with `nextLive` (the open set is every browser tab of every session in `pane`), calls `browser.close(key)` for each key `nextLive` closes, and on unmount nothing (the shell closes pages at quit). A `browser://state` event for a known tab dispatches `{ type: "page", … }`; a `browser://popup` opens a new browser tab in the popup's own session, selected only if that session is on screen (6q second round 5).
  - `browser.theme(dark)` follows `appScheme()`, sent on mount and on every theme change.
  - A `browser://drag` `drop` is a drop on the pane: its files open as tabs through the same path `dropTarget`'s pane delivery uses.
  - `StripTab.icon`: `usePage(key)?.icon ?? "globe"` for a browser tab.

- [ ] **Step 1: Write the failing tests:** `plusMenu.test.ts` (`the first row is "New browser tab"`, `where browser tabs aren't available the row is disabled with its hint`); `keymap` test (`the three commands exist and have no default key`); `urlLinks` test (`the hint reads "Ctrl+click to open in the side pane" where browser tabs are available`, `and "…in your browser" elsewhere`); `ClearDataDialog.test.tsx` (exact heading, body and button texts; `aria-modal="true"`; it calls `useCoverPage`); `Preview` test (`a web link opens a browser tab with no question`); a `useBrowserPages` pure helper `openKeys(pane): Set<string>` (`every browser tab of every session is counted, file tabs are not`).

- [ ] **Step 2: Run to see them fail** — `pnpm test src/features/pane src/features/terminal src/features/preview src/features/browser src/lib`.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `pnpm test` (the whole suite: this task touches shared files) · `pnpm build` → pass.
- [ ] **Step 5: Commit** — `feat(browser): web addresses, port chips and links open browser tabs in the side pane.`

---

### Task 12: The binding text follows (worker)

**Files:**
- Modify: `docs/PLAN.md` (the sections that describe the Preview's web links, port chips and the + menu; §3's list of shell events gains the page's events; no change to §9's decision lists), `.claude/rules/tauri-shell.md` (the events rule names the browser page's events; the window lookups use `main_window`), `CLAUDE.md` "Gotchas" (a child webview draws above the app's webview, so anything drawn over the pane must call `useCoverPage`; once a page exists `get_webview_window("main")` returns None), `docs/research/codex-cli.md` untouched.

- [ ] **Step 1:** `rg -n "system browser|WebLinkBar|open in your browser|port chip" docs/PLAN.md` and bring each place in line with 6q-3 decisions 3 and 9. Plain statements, no history of the change.
- [ ] **Step 2:** `pnpm test scripts` if the docs have a check; otherwise none.
- [ ] **Step 3: Commit** — `docs(plan): the plan's text follows the browser tab.`

---

### Task 13: Live check (main thread, `marshell-live-check` skill)

Scratch `MARSHELL_HOME` and CLI homes; the loopback test site from `phase-6q-0` (`git show phase-6q-0:spikes/browser-pane/server.mjs`, run from a scratch folder; it is not committed). Findings go in `docs/research/2026-11-1x-browser-tab-live-check.md`. A page has no CDP (S12 finding 2): drive the app's webview over CDP, read a page through the site's log and UI Automation, and send real keys with a foreground-PID guard (`keys.ps1` on `phase-6q-0`).

- [ ] 1. **Open.** Ctrl+click on a URL in terminal output, a port chip, a Markdown link, "New browser tab", "Open URL…": each opens a browser tab; the first three put the keyboard in the page, the last two in the address box. A second Ctrl+click on the same URL selects its tab.
- [ ] 2. **The bar.** Back, Forward, Reload, Stop on a slow page, open outside. `localhost:47620`, `example.com`, words (a Google search), `c:\x` (refused, and nothing reaches the site or Google).
- [ ] 3. **Place.** The page matches its slot within 1 px: at rest, after a divider drag, a sidebar fold, a window-edge drag across 960 and 720 px, maximize and restore, and at an emulated 125%. The user judges the motion and the edge drags by eye.
- [ ] 4. **Keys.** From the page, 20 of 20 each: Ctrl+Shift+K (palette), Ctrl+Shift+Q, Ctrl+Tab, Ctrl+L, Esc. Kept by the page: F5, Ctrl+R, Ctrl+F, Ctrl+P, Ctrl+Shift+V, Ctrl+Shift+Z. F12 opens DevTools for the page. After the palette closes, typing lands in the page.
- [ ] 5. **Focus.** Tab in and out both ways, 10 of 10. Alt+Tab away and back with the page focused, then with the terminal focused: typing lands where it was, 10 of 10. An edge drag with the page focused (S12 limit 5). No banner fires while the page has the keyboard (the window is not "blurred").
- [ ] 6. **Freeze.** The palette, the + menu, a context menu, Settings, the navigation cursor and a file dragged from Explorer over the page: each draws over the page's picture, and the page is back after. Open and close the palette as fast as the keys go, 20 times: the page ends live.
- [ ] 7. **Isolation.** From a page at the test site and, in dev, from a typed `http://localhost:1420/…`: the dev origin is refused; `plugin:app|version` and `core_endpoint` are refused from the site; a redirect to `http://tauri.localhost/` and to `file:` is refused; a `mailto:` link starts nothing. **A frame at the app's own origin inside a page** (`<iframe src="http://tauri.localhost/">` in a release build, `localhost:1420` in dev): does it load, and can it call `core_endpoint`? If it can, add `frame-ancestors 'none'` to the CSP (approved, 6q-3 decision 2), rerun, and record both runs.
- [ ] 8. **Profile.** A cookie set 1 s before a normal quit (the title bar's close with "quit", and the tray's Quit) survives the restart. No `msedgewebview2.exe` of the profile is left 3 s after the quit. Sign-in state survives a restart.
- [ ] 9. **Tabs.** A restart restores the browser tabs with their titles; each loads when first shown. Nine pages shown in turn: the first reloads when shown again, and the other eight keep their scroll position across a session switch. Closing a tab closes its page (the process count drops).
- [ ] 10. **The tray.** With a page open: close to the tray and bring the window back from the tray; the badge and the flash still work (S12 finding 16).
- [ ] 11. **Decisions 5, 6, 8, 10.** A download lands in Downloads with WebView2's flyout. A page asking for the clipboard, location and notifications gets WebView2's prompt each time. A site with two designs follows Marshell's theme, and changes with it. A site's icon shows on its tab. "Clear the browser's data…" signs the test site out.
- [ ] 12. **Drop.** A file dropped on the page, on the URL bar and on the terminal: the first two open tabs, the third types the path.
- [ ] 13. **Forced colours and the keyboard alone:** the bar and the strip's icon under an emulated forced-colors theme; every control in the bar by Tab.
- [ ] 14. **Left for the user's hands:** the feel of the freeze's flash with real menus, the motion with a page open, a real 125% and 150% display.

Stop only what the check started, by PID, after listing its tree.

## After the tasks

1. The final whole-branch review: the `marshell-reviewer` agent on Opus, given the review package, this plan and its Review Focus; then one fix pass.
2. `marshell-ship`: what CI runs, the PR (sections "What it does", "Live check", "Known limits", "Checks"), a green CI, the merge. CI flakes #112 and #115 each get one rerun, named in the PR.
3. Delete the ledger; update the memory notes; 6q-3b is next.
