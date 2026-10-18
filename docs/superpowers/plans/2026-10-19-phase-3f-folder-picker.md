# Phase 3f Folder Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The launcher's folder field accepts a typed full path and opens the OS folder picker, so a session can start in a folder Marshell has never seen.

**Architecture:**
- The core gains `GET /v1/folders/check`. It expands `~`, decides whether the text is a full path for the running OS, and answers with the folder's canonical path, name and branch, or with a refusal.
- The app shell gains a `pick_folder` Tauri command that opens the native dialog through `tauri-plugin-dialog` from Rust. The webview gets no dialog permission.
- The launcher list gains a "Use <path>" row first and a "Browse…" row last, and Ctrl+O works in the field. `LiveLauncher` keeps the chosen folder next to the recent list until it is recorded on launch.
- `projects::record` never records a drive root, and the write guard ignores one already in `projects.json`.

**Tech Stack:** Rust (axum 0.8, Tauri 2, `tauri-plugin-dialog` 2), React + TypeScript, Vitest (node, `renderToStaticMarkup`).

**Spec:** `docs/PLAN.md`:
- "Launch a session" (line 277)
- §9 "Slice 3f decisions" (items 1–7)

## Global Constraints

- **Path syntax:** a full path is absolute in the running OS's syntax (`C:\x`, `\\server\share`, `/x` on Unix) or starts with `~` for the home folder. Quotes pasted around it are stripped, and anything else only filters the known folders.
- **Path row:** an existing folder gets "Use <path>". A missing one (or a file) gets a disabled "No folder at <path>" row, where Enter does nothing.
- **Browse:** a "Browse…" row sits at the end of the list, and Ctrl+O in the field opens the native picker. Mouse and keyboard reach it alike.
- **Dialog wiring:** Marshell's own `pick_folder` command calls the plugin from Rust. `src-tauri/capabilities/default.json` gets **no** `dialog:` permission, and no npm package is added.
- **Remembering:** a drive root (`C:\`, `/`, a UNC share root) is never recorded in `projects.json`, as the home folder isn't. The config write guard never allows one.
- **Approved dependencies:** only `tauri-plugin-dialog` (Rust crate) is new.
- **UI:** keyboard-first; works in forced colours. Copy uses "folder", never "directory".
- **Protocol:** structs are snake_case, the generated TS in `src/generated/` is committed, and u64 fields use `#[ts(type = "number")]`.
- **Commits:** `type(scope): sentence`. Run `cargo fmt --all` before a Rust commit.

## Review Focus

1. **A stale check answer.** The person types `C:\a`, then `C:\ab`, and the answer for `C:\a` arrives last. The row must show `C:\ab`'s state, never `C:\a`'s. Task 5 pins it with `latestOnly` sequencing.
2. **A typed or picked folder that is already a recent project, spelt differently** (case, trailing `\`, `~`). It selects the existing project and doesn't add a duplicate. Task 5 pins it with `chooseFolder` cases.
3. **Enter on the path row while it is still checking or missing.** Nothing launches. Tasks 3 and 4 pin it.
4. **Cancelling the native dialog.** The field stays open, its text is kept and focus is back in it. A second Ctrl+O while the dialog is open opens no second dialog. Task 5 covers it, plus the live check.
5. **`/x` typed on Windows.** It isn't a full path there, so it just filters and there's no path row. The core's 400 decides it, not the frontend. Tasks 1 and 5 pin it.

---

### Task 1: Core folder check, roots never recorded

**Files:**
- Create: `crates/core/src/folders.rs`
- Create: `crates/core/src/server/folders.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod folders;`)
- Modify: `crates/core/src/server/mod.rs` (route next to `/v1/projects`, before `.layer(auth_guard)`)
- Modify: `crates/protocol/src/api.rs` (`FolderInfo`)
- Modify: `crates/core/src/projects.rs` (`record` skips a root)
- Modify: `crates/core/src/config/guard.rs` (`for_home` skips a root project)
- Test: unit tests in `folders.rs`, `projects.rs` and `guard.rs`, and `crates/core/tests/folders_api.rs` (copy the setup of an existing `tests/*_api.rs`)

**Interfaces:**
- Produces:
  - `marshell_protocol::api::FolderInfo { path: String, name: String, branch: Option<String> }`, with derives as for `ProjectInfo` and `#[ts(export)]`.
  - `folders::check(raw: &str) -> Result<FolderInfo, FolderRefusal>`.
  - `enum FolderRefusal { NotFullPath, Missing }`.
  - `projects::is_root(path: &Path) -> bool`.
  - `GET /v1/folders/check?path=<text>` (token required):
    - 200 with `FolderInfo`
    - 400 when the text isn't a full path
    - 404 when nothing is there or it isn't a folder

- [ ] **Step 1: Write the failing tests.** In `folders.rs` `#[cfg(test)]`:
  - `an_existing_folder_is_found`: a tempdir gives `Ok`. `path == projects::canonical(dir)` as a string, `name` is the dir's file name, and `branch` is `None`.
  - `a_folder_in_git_has_its_branch`: write `<dir>/.git/HEAD` = `ref: refs/heads/main\n`, and `branch == Some("main")`.
  - `a_missing_folder_is_missing`: `<tempdir>/nope` gives `Err(Missing)`.
  - `a_file_is_missing`: a file path gives `Err(Missing)`.
  - `tilde_is_the_home_folder`: `check("~")?.path == canonical(dirs::home_dir())`. Also `check("~/")` and, on Windows, `check("~\\")`.
  - `a_relative_path_is_not_a_full_path`: `"src"`, `"./x"`, `""` and `"   "` each give `Err(NotFullPath)`.
  - `#[cfg(windows)] a_rooted_path_without_a_drive_is_not_full_on_windows`: `"/x"` and `"\\x"` give `Err(NotFullPath)`.
  - `surrounding_whitespace_is_ignored`: `format!("  {dir}  ")` gives `Ok`.

  In `projects.rs`: `a_root_is_not_recorded` records `Path::new(if cfg!(windows) { "C:\\" } else { "/" })`, and the file isn't created. `is_root` holds for `C:\`, `C:/`, `/` and `\\server\share\`, and not for `C:\x`.

  In `guard.rs`: `a_root_in_projects_json_allows_nothing`. Write `projects.json` with a root entry using the `store` format, then check that `Guard::for_home(home).check(<a file directly under a temp dir outside home>)` is still `Err(OutsideAllowed)`. If a root can't be written portably, build the guard through the same filter `for_home` applies.

  In `tests/folders_api.rs`: `check_answers_200_404_400` covers each status, plus 401 without the token.
- [ ] **Step 2: Run them and check they fail.** Run `cargo test -p marshell-core folders` and `cargo test -p marshell-core root`. Expected: they fail to compile (`folders` is not defined).
- [ ] **Step 3: Implement.**
  - `check` trims whitespace, then expands a leading `~` (alone, or followed by `/` or `\`) with `dirs::home_dir()`.
  - A path that isn't absolute gets `NotFullPath`. On Windows `Path::is_absolute` already rejects `/x`.
  - A path that isn't a directory gets `Missing`. Otherwise it's `canonical(path)`, with `name` its file name or the whole path for a root, and `branch` from `projects::git_branch`.
  - `is_root`: `trim_separators`, then `Path::new(..).parent().is_none()`.
  - `record` returns `Ok(())` early for a root, next to the home check.
  - `for_home` filters roots out of the project list.
  - The route uses `spawn_blocking` and serialises `FolderInfo`. Its 400 and 404 bodies are short text, like the other routes.
- [ ] **Step 4: Run the tests and check they pass.** Run `cargo test -p marshell-core` and `cargo test -p marshell-protocol`; the second regenerates `src/generated/FolderInfo.ts`. Expected: all pass, and `pnpm check:generated` exits 0.
- [ ] **Step 5: Commit.** Run `cargo fmt --all`, then commit with `feat(core): check a typed folder path, and never record a drive root`.

### Task 2: `pick_folder` command

**Files:**
- Modify: `src-tauri/Cargo.toml` (`tauri-plugin-dialog = "2"`)
- Create: `src-tauri/src/folder_picker.rs`
- Modify: `src-tauri/src/main.rs` (`.plugin(tauri_plugin_dialog::init())` with a one-line comment that no capability grants it to the webview, then `mod folder_picker;` and `folder_picker::pick_folder` in `generate_handler!`)
- Create: `src/lib/folderPicker.ts`
- Test: unit tests in `folder_picker.rs`, and `src/lib/folderPicker.test.ts`

**Interfaces:**
- Produces:
  - Tauri command `pick_folder(window: tauri::Window, start: Option<String>) -> Result<Option<String>, String>`, which is `None` when cancelled.
  - TS `pickFolder(start: string | null): Promise<string | null>`. It calls `invoke("pick_folder", { start })` the same way `core_endpoint` is invoked; find that call with grep and reuse its import.

- [ ] **Step 1: Write the failing tests.**
  - `start_folder(start: Option<&str>) -> Option<PathBuf>` returns the path only when it is an existing directory. Tests: an existing tempdir gives `Some`, a missing path gives `None`, `None` gives `None`, and a file gives `None`.
  - TS: `pickFolder` passes `{ start }` and maps `null` through. Mock the invoke function the way other `src/lib` tests do; grep for `vi.mock("@tauri-apps/api/core"`.
- [ ] **Step 2: Run them and check they fail.** Run `cargo test -p marshell-app folder_picker` and `pnpm test src/lib/folderPicker.test.ts`. Expected: both fail because the items are not defined.
- [ ] **Step 3: Implement.**
  - The command calls `window.dialog().file()`, with `.set_directory(dir)` when `start_folder` gives one and `.set_parent(&window)` so the dialog is modal to Marshell.
  - Then `.pick_folder(move |p| { let _ = tx.send(p); })` on a `tokio::sync::oneshot`, awaited in an `async` command, so the UI thread isn't blocked.
  - Map `FilePath::into_path()` to a string, and turn an error into its text.
  - `default.json` stays untouched.
- [ ] **Step 4: Run the tests and check they pass.** Same commands, plus `cargo clippy -p marshell-app --all-targets -- -D warnings`. Expected: green and clean.
- [ ] **Step 5: Commit.** `feat(app): open the native folder picker from a Rust command`, with `Cargo.lock` included.

### Task 3: Folder rows and typed paths (pure)

**Files:**
- Create: `src/features/launcher/folderRows.ts`
- Test: `src/features/launcher/folderRows.test.ts`

**Interfaces:**
- Consumes: the `Hit<Project>` type returned by `filterProjects` in `filter.ts`.
- Produces:
  - `typedPath(query: string): string | null` returns the query with surrounding whitespace and one pair of matching `"` or `'` stripped. It returns the text when that looks like a path candidate, otherwise `null`. Candidates are text starting with `~`, `/`, `\` or `<letter>:`. The core decides whether the candidate is full (Task 1).
  - `type PathCheck = { path: string; state: "checking" } | { path: string; state: "found"; folder: FolderInfo } | { path: string; state: "missing" }`. `path` is the typed text being checked.
  - `type FolderRow = { kind: "path"; check: PathCheck } | { kind: "project"; hit: Hit<Project> } | { kind: "browse" }`.
  - `folderRows(check: PathCheck | null, hits: Hit<Project>[]): FolderRow[]` puts the path row first when `check` is non-null, then the hits, then `browse`, which is always last.
  - `rowEnabled(row: FolderRow): boolean` is false only for a path row that is checking or missing.

- [ ] **Step 1: Write the failing tests.**
  - `typedPath`:
    - `"C:\\dev"`, `"c:/dev"`, `"\\\\srv\\share"`, `"/usr"`, `"~"`, `"~/x"` and `"\"C:\\a b\""` give the path, the last one without its quotes.
    - `"  C:\\x  "` gives `"C:\\x"`.
    - `"dev"`, `"my-app"`, `""` and `"\"dev\""` give `null`.
    - A lone `"\""` gives `null`.
  - `folderRows`:
    - With no check and two hits it's `[project, project, browse]`.
    - With a check it's `[path, ...hits, browse]`.
    - With no hits and no check it's `[browse]`.
  - `rowEnabled` holds for found, project and browse rows, and not for checking or missing rows.
- [ ] **Step 2: Run them and check they fail.** Run `pnpm test src/features/launcher/folderRows.test.ts`. Expected: it fails because the module is missing.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the tests and check they pass.** Same command. Expected: they pass.
- [ ] **Step 5: Commit.** `feat(launcher): list a typed path and Browse among the folder rows`.

### Task 4: Launcher renders the rows; Enter, click and Ctrl+O

**Files:**
- Modify: `src/features/launcher/Launcher.tsx`
- Modify: the launcher stylesheet that styles `.launcher__opt` (find it with grep). Add only what the new rows need: a disabled look and the `Ctrl+O` hint, using existing tokens. Read the `frontend-design` and `good-css` notes in the global setup before you write declarations.
- Modify: `src/features/launcher/Launcher.stories.test.tsx` and its stories, if stories enumerate the launcher's states. Add one story with a found path row and one with a missing row.
- Test: `src/features/launcher/Launcher.test.tsx`

**Interfaces:**
- Consumes: `folderRows`, `rowEnabled`, `PathCheck` (Task 3).
- Produces:
  - New optional `view.pathCheck?: PathCheck | null`.
  - New props `onUsePath?: (folder: FolderInfo) => void` and `onBrowse?: () => void`.
  - The listbox is built from `folderRows(view.pathCheck ?? null, options)`, so `listSize` counts all rows.

- [ ] **Step 1: Write the failing tests** (`renderToStaticMarkup`, as in the existing folder cases):
  - **Browse row:** the open field with no query renders a last option with the text `Browse…`.
  - **Found path:** `pathCheck` found renders an option `Use C:\dev` first, with `aria-disabled` absent.
  - **Missing path:** renders `No folder at C:\nope` with `aria-disabled="true"`.
  - **Checking:** renders `Checking C:\de…` with `aria-disabled="true"`.
  - **No match:** with no hit and no check, the "No folder matches" status still renders, and the listbox holds only `Browse…`.

  Key handling is easiest to pin as a pure function. Extract `folderEnter(row: FolderRow | undefined): { kind: "pick"; id: string } | { kind: "use"; folder: FolderInfo } | { kind: "browse" } | null` into `folderRows.ts` and test it there:
  - A found row gives `use`.
  - A missing or checking row gives `null`.
  - A project row gives `pick`.
  - The browse row gives `browse`.
- [ ] **Step 2: Run them and check they fail.** Run `pnpm test src/features/launcher`. Expected: the new cases fail.
- [ ] **Step 3: Implement.**
  - `onSheetKey`: with the folder field open, Enter runs `folderEnter(rows[activeIndex])`. `Ctrl+O` (no Shift or Alt) with the folder field open calls `preventDefault`, then `onBrowse`.
  - Each row's `onClick` does the same as Enter. A disabled row does nothing.
  - The empty state keeps its status text, and the listbox renders whenever there is at least one row.
  - `aria-expanded` and `aria-controls` on the input follow `rows.length > 0`.
  - The path row shows the path in the `launcher__opt-path` span and the verb in `launcher__opt-name`, so long paths wrap as now.
  - The Browse row shows `Browse…` and a `<kbd>Ctrl+O</kbd>` hint.
- [ ] **Step 4: Run the tests and check they pass.** Run `pnpm test src/features/launcher`. Expected: they pass.
- [ ] **Step 5: Commit.** `feat(launcher): Use <path>, No folder at <path> and Browse… rows, with Ctrl+O`.

### Task 5: LiveLauncher wiring

**Files:**
- Modify: `src/features/sessions/LiveLauncher.tsx`
- Modify: `src/features/sessions/launchPrograms.ts`
- Create: `src/lib/api/folders.ts` (shaped like `src/lib/api/projects.ts`)
- Test: `src/features/sessions/launchPrograms.test.ts`, `src/lib/api/folders.test.ts` (if API wrappers have tests; follow `projects`)

**Interfaces:**
- Consumes:
  - `GET /v1/folders/check` (Task 1)
  - `pickFolder` (Task 2)
  - `typedPath`, `PathCheck` (Task 3)
  - The `Launcher` props from Task 4
- Produces:
  - `folders.check(endpoint, path): Promise<FolderInfo>`. It throws `ApiError` with `.status` 400 or 404.
  - `checkState(path: string, result: { ok: FolderInfo } | { status: number }): PathCheck | null`: found for ok, `null` for 400 (not a full path, so there's no row), and missing for anything else.
  - `chooseFolder(projects: Project[], folder: FolderInfo): { projectId: string; extra: Project | null }`. It returns the id of the recent project with the same path when there is one, otherwise a new `Project` built from `folder` with `lastUsedMs: 0`.
  - `samePath(a, b)`: the frontend twin of the core's `same_path`, which trims trailing separators and ignores case for a Windows-shaped path. Reuse `taskOf.ts`'s Windows test if it can be exported cleanly.
  - `latestOnly()`: returns `{ next(): number; isLatest(n: number): boolean }`, used to drop stale check answers.

- [ ] **Step 1: Write the failing tests** in `launchPrograms.test.ts`:
  - `chooseFolder` matches `C:\Dev\App\` to the project `c:\dev\app`, with `extra` null.
  - `chooseFolder` on an unknown folder gives `extra` with `id === folder.path`, `name` and `branch` copied when non-null.
  - `checkState` maps 200 to found, 400 to `null`, and 404 and 500 to missing.
  - `latestOnly`: after `a = next(); b = next();`, `isLatest(a)` is false and `isLatest(b)` is true.
- [ ] **Step 2: Run them and check they fail.** Run `pnpm test src/features/sessions/launchPrograms.test.ts`. Expected: the new cases fail.
- [ ] **Step 3: Implement the helpers, then wire `LiveLauncher`.**
  - **State:** `extra: Project | null`, and the projects passed down are `extra ? [extra, ...recent] : recent`. `pathCheck: PathCheck | null`, and `browsing` as a ref.
  - **Checking:** on every query change, if `typedPath(q)` is non-null, set `{ path, state: "checking" }` and call `folders.check` after 150 ms of quiet through `latestOnly`, then apply `checkState`. Otherwise clear `pathCheck`. Closing the field clears it and cancels the timer.
  - **`onUsePath(folder)`:** `chooseFolder`, set `extra`, then patch `projectId` and close the field, as `onPick` does.
  - **`onBrowse`:** when `browsing` is set, return. Otherwise start the dialog in the found path, else the chosen folder's path, else `null`. Then `pickFolder(start)`:
    - `null` (cancelled): leave the field open with its text and focus the input again.
    - A path: `folders.check(path)`, then do what `onUsePath` does.
    - A failed check after a pick: show `Couldn't use <path>: <reason>` through `setError`.
  - **Activity:** `setActiveIndex(0)` on every query change, as now.
- [ ] **Step 4: Run the tests and check they pass.** Run `pnpm test src/features` and `pnpm build`. Expected: both green.
- [ ] **Step 5: Commit.** `feat(launcher): start a session in a typed or browsed folder`.

### Task 6: Live check

**Files:**
- Create: `docs/research/2026-10-19-folder-picker-live-check.md`

Run the check per `CLAUDE.md` "Live check (Windows)": a scratch `MARSHELL_HOME`, CDP on 9333, and stop by PID. Only shell sessions are started; no agent is needed.

- [ ] **Step 1: Prepare.** Make a scratch project folder with spaces in its name, run `git init` in it, and note a missing path next to it.
- [ ] **Step 2: Check the typed paths.** Open the launcher and type the scratch path. The "Use <path>" row appears, and Enter selects it. The chip shows its name, and the worktree chip is offered because it's in git. Type the missing path: there's a disabled row, and Enter does nothing. Type `/x`: there's no path row. Type `"<path>"` with quotes: the row is found.
- [ ] **Step 3: Check the dialog.** Press Ctrl+O. The native dialog opens, and since it is native, read it with a screenshot. Cancel it: the field and its text remain. Use Browse… again and pick the scratch folder (keyboard in the dialog via UI Automation or a screenshot check, as the earlier capture check did). The chip shows the folder.
- [ ] **Step 4: Start a shell.** Start a shell session in the typed folder. Its tab starts in that folder, and `projects.json` in the scratch home now lists it. Type `C:\`, use it, and start a shell. `projects.json` doesn't gain `C:\`.
- [ ] **Step 5: Check the theme.** The new rows are readable in forced colours (Windows contrast theme), judged from a screenshot.
- [ ] **Step 6: Write it up.** Write the results table in the research doc, then commit with `docs(research): live check of the folder picker`.
