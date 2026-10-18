# Phase 3b Config Safety Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every write Marshell makes to a CLI's configuration goes through one core service: a ChangeSet is previewed as a unified diff, applied only with the hashes the user saw, snapshotted, written atomically, recorded in `managed.json`, and can be restored. `~/.marshell` becomes owner-only.

**Architecture:** A new module `crates/core/src/config/` holds pure parts (hashing, JSONC CST edits, the write guard, atomic replace, snapshots, `managed.json`, the 3-way merge) and one stateful `ConfigService` (stored previews, per-path locks, our own write hashes, the watcher). `AppState` owns the service, and `server/config.rs` exposes PLAN §3's config and snapshot routes. The owner-only folder rule lives in `crates/core/src/private.rs` and runs on every core start. No UI.

**Tech Stack:** Rust 2024, axum 0.8, `jsonc-parser` (cst), `notify` 8, `diffy`, `sha2`, `windows-sys` 0.61, ts-rs.

**Spec:** `docs/PLAN.md` §6 (config safety layer), §3 (Config routes), §4 (data layout), §9 "Slice 3b decisions (2026-10-18)", and "Config safety copy" under "Batch 3 design decisions".

## Global Constraints

- New dependencies are exactly the approved ones: `jsonc-parser` 0.34 with features `cst` and `serde`, `notify` 8 (the latest stable 8.x, not the 9.0 release candidates), `diffy` 0.5, and `sha2` 0.10 (the 0.10.9 already in `Cargo.lock`). Nothing else. Adding `windows-sys` features is allowed. `THIRD_PARTY_NOTICES.md` lists no Rust crates today and isn't changed.
- JSON edits are CST edits only: "There is no whole-file re-serialize fallback. If the CST can't parse a file, fail closed." `serde_json` is for reading only.
- A file hash is the SHA-256 of its raw bytes as 64 lowercase hex characters. A missing file has no hash (`None`).
- Snapshots keep the last 20 per file (`KEEP_PER_FILE = 20`).
- The write guard allows only paths under the user's home folder, a project folder in `projects.json`, or `<marshell home>/agents/`. It always refuses the managed and organization paths, even inside an allowed folder. Elevation is never run.
- "A file that is read-only, does not parse, or is managed is never described as about to change": a refused file's preview has an empty diff.
- A conflict never overwrites. Apply refuses with 409 and leaves every file as it was.
- Edit kinds in 3b are JsonSet, JsonRemove, CreateFile and Restore. TOML and Markdown blocks aren't built.
- No UI and no frontend source changes. The only frontend change is the regenerated `src/generated/` from new protocol types.
- `GET /config/sources?agent&cwd` needs the adapter's list of config files and moves to 3c with the Claude adapter.
- Tests use temp folders only. Never touch the real `~/.claude`, `~/.codex` or `~/.marshell`.
- Concurrency rule (PLAN §4): never hold a `MutexGuard` across `.await` or I/O that waits on another lock. Handlers run file work in `spawn_blocking`.
- Windows 11 is the tested platform and CI is Windows-only. Unix code paths are written under `#[cfg(unix)]` and must compile, but are not run.
- Commits: `type(scope): sentence`, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Run `cargo fmt --all` before every Rust commit, and `cargo clippy --workspace --all-targets -- -D warnings` must pass.

## Review Focus

1. **A symlinked settings file** (dotfile managers link `~/.claude/settings.json`). The write goes to the link's target, the link stays a link, and the guard checks the target. Test: Task 4 `writes_through_a_link_to_its_target`, Task 3 `a_link_out_of_the_allowed_folder_is_refused`.
2. **CRLF line endings and a UTF-8 BOM** (Windows editors). After an edit, every line still ends in CRLF and the BOM is kept. Test: Task 2 `crlf_stays_crlf`, `bom_is_kept`.
3. **A multi-file apply where a later write fails.** The files already written are put back, no snapshot is left behind, and `managed.json` is unchanged. Test: Task 8 `a_failed_second_write_rolls_back_the_first`.
4. **Two applies racing on one file.** Exactly one wins and the other gets a conflict; neither change is lost silently. Test: Task 8 `concurrent_applies_on_one_file_one_conflicts`.
5. **A file edited outside Marshell between preview and apply.** Apply is a 409 and the file keeps the outside edit. Test: Task 8 `a_file_changed_after_preview_is_a_conflict_and_untouched`, Task 10 `apply_after_an_outside_edit_is_409_and_the_file_is_untouched`.

---

## File map

| File | Responsibility |
|---|---|
| `crates/core/src/config/mod.rs` | Module docs, `pub mod` list, re-exports |
| `crates/core/src/config/hash.rs` | SHA-256 of bytes and files |
| `crates/core/src/config/jsonc.rs` | JSON pointer set and remove on JSONC text through the CST |
| `crates/core/src/config/guard.rs` | Which paths the service may write |
| `crates/core/src/config/write.rs` | Atomic replace and remove with retries, link resolution |
| `crates/core/src/config/snapshots.rs` | `snapshots/index.json` and blobs, keeping 20 per file |
| `crates/core/src/config/managed.rs` | `managed.json`, the entries Marshell wrote |
| `crates/core/src/config/preview.rs` | ChangeSet to planned files and `ConfigPreview` |
| `crates/core/src/config/service.rs` | `ConfigService`: stored previews, apply, restore, locks, own-write hashes |
| `crates/core/src/config/watch.rs` | `notify` watcher with a 200 ms debounce |
| `crates/core/src/config/merge.rs` | 3-way merge through `diffy` |
| `crates/core/src/private.rs` | Owner-only permissions on the Marshell home |
| `crates/core/src/server/config.rs` | HTTP handlers |
| `crates/protocol/src/config.rs` | Wire types (ts-rs) |
| `crates/core/tests/config_api.rs` | Route tests |
| `docs/adr/0001-threat-model.md` | The threat model ADR |

---

### Task 1: Dependencies and hashing

**Files:**
- Modify: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `Cargo.lock`
- Create: `crates/core/src/config/mod.rs`, `crates/core/src/config/hash.rs`

**Interfaces:**
- Produces: `config::hash::hash_bytes(bytes: &[u8]) -> String`, `config::hash::hash_file(path: &Path) -> std::io::Result<Option<String>>` (`Ok(None)` when the file doesn't exist).

- [ ] **Step 1: Add the four dependencies** to `crates/core/Cargo.toml`: `jsonc-parser = { version = "0.34", features = ["cst", "serde"] }`, `notify = "8"`, `diffy = "0.5"`, `sha2 = "0.10"`. Run `cargo tree -p marshell-core -e normal --depth 1` and check that `sha2` resolves to 0.10.9. Read each crate's `license` field in `~/.cargo/registry/src/*/<crate>-<ver>/Cargo.toml` and note it in the commit body. The expected licences are MIT, MIT OR Apache-2.0, or CC0-1.0, with no copyleft.
- [ ] **Step 2: Write the failing tests** in `hash.rs`:

```rust
#[test]
fn hash_is_sha256_hex() {
    assert_eq!(hash_bytes(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}
#[test]
fn missing_file_hashes_to_none() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(hash_file(&dir.path().join("nope.json")).unwrap(), None);
    std::fs::write(dir.path().join("a"), b"abc").unwrap();
    assert_eq!(hash_file(&dir.path().join("a")).unwrap().as_deref(), Some(hash_bytes(b"abc").as_str()));
}
```

- [ ] **Step 3: Run** `cargo test -p marshell-core config::hash`. Expected: compile FAIL, because the functions don't exist yet.
- [ ] **Step 4: Implement** both functions. Then add `pub mod config;` to `lib.rs` and `pub mod hash;` to `config/mod.rs`. The module docs in `mod.rs` summarise PLAN §6 in two sentences.
- [ ] **Step 5: Run** `cargo test -p marshell-core config::hash`. Expected: 2 passed.
- [ ] **Step 6: Commit** `feat(config): add the config module with SHA-256 file hashing and the approved crates`.

### Task 2: JSONC CST edits

**Files:**
- Create: `crates/core/src/config/jsonc.rs`

**Interfaces:**
- Produces:

```rust
pub enum JsonOp<'a> { Set { pointer: &'a str, value: &'a serde_json::Value }, Remove { pointer: &'a str } }
#[derive(Debug, PartialEq)]
pub enum EditError { DoesNotParse(String), BadPointer { pointer: String, detail: String }, Internal(String) }
/// Applies the ops in order and returns the new text. `text` may start with a BOM and may be empty or whitespace only (treated as `{}`).
pub fn apply_ops(text: &str, ops: &[JsonOp]) -> Result<String, EditError>
/// Reads JSONC (comments, trailing commas) into a serde value, for reading only.
pub fn read_value(text: &str) -> Result<serde_json::Value, EditError>
```

**Rules the tests pin down:**
- Pointers follow RFC 6901, including `~0` and `~1`. The root must be an object, and the pointer `""` is a `BadPointer`.
- `Set` creates missing parent objects. In an array, a numeric index must already exist (replace) or be `-` (append). Going through a scalar is a `BadPointer`.
- `Remove` of a pointer that isn't there returns the input unchanged, byte for byte.
- A BOM is stripped before parsing and put back on the output.
- If the input has CRLF and no lone LF, every lone LF in the output becomes CRLF.
- **The self-check is fail closed.** After editing, `read_value(output)` must equal the same ops applied to `read_value(input)` with `serde_json::Value::pointer_mut` logic. If they differ, return `EditError::Internal`, so a CST bug can never reach the disk.
- Use `jsonc_parser::cst::CstRootNode::parse` and the CST's object/array/property methods (`object_value_or_set`, `get`, `append`, `set_value`, `remove`; check the 0.34 source in the cargo registry for the exact names). Convert a `serde_json::Value` to the CST's input value with a small recursive helper if the crate has no `From`.

- [ ] **Step 1: Write the failing tests**:

```rust
const COMMENTED: &str = "{\n  // my theme\n  \"theme\": \"dark\",\n  \"zeta\": 1, /* trailing */\n  \"alpha\": 2,\n}\n";

#[test]
fn set_keeps_comments_and_key_order() {
    let v = serde_json::json!([{"type": "command"}]);
    let out = apply_ops(COMMENTED, &[JsonOp::Set { pointer: "/hooks/Stop", value: &v }]).unwrap();
    assert!(out.contains("// my theme") && out.contains("/* trailing */"));
    let pos = |k: &str| out.find(k).unwrap();
    assert!(pos("\"theme\"") < pos("\"zeta\"") && pos("\"zeta\"") < pos("\"alpha\"") && pos("\"alpha\"") < pos("\"hooks\""));
    assert_eq!(read_value(&out).unwrap()["hooks"]["Stop"], v);
}
#[test]
fn set_replaces_an_existing_value_in_place() {
    let out = apply_ops("{\"a\": 1, \"b\": 2}", &[JsonOp::Set { pointer: "/a", value: &5.into() }]).unwrap();
    assert_eq!(out, "{\"a\": 5, \"b\": 2}");
}
#[test]
fn set_creates_missing_parents() { /* "{}" set /a/b/c = true → read_value == {"a":{"b":{"c":true}}} */ }
#[test]
fn append_to_array_with_dash() { /* {"hooks":{"Stop":[{"x":1}]}} set /hooks/Stop/- {"y":2} → Stop[1] == {"y":2}, len 2 */ }
#[test]
fn remove_drops_the_entry_and_its_comma() {
    let out = apply_ops("{\"a\": 1, \"b\": 2, \"c\": 3}", &[JsonOp::Remove { pointer: "/b" }]).unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&out).unwrap(), serde_json::json!({"a": 1, "c": 3}));
}
#[test]
fn remove_missing_pointer_is_no_change() { /* COMMENTED, remove /nope/x → out == COMMENTED */ }
#[test]
fn pointer_escapes() { /* set /a~1b = 1 and /m~0n = 2 on "{}" → keys "a/b" and "m~n" */ }
#[test]
fn unparseable_file_fails_closed() {
    assert!(matches!(apply_ops("{ \"a\": ", &[JsonOp::Remove { pointer: "/a" }]), Err(EditError::DoesNotParse(_))));
}
#[test]
fn set_through_a_scalar_is_bad_pointer() { /* {"a":1} set /a/b → BadPointer */ }
#[test]
fn root_must_be_an_object() { /* "[1]" set /a → BadPointer; "{}" set "" → BadPointer */ }
#[test]
fn crlf_stays_crlf() {
    let out = apply_ops("{\r\n  \"a\": 1\r\n}\r\n", &[JsonOp::Set { pointer: "/b", value: &serde_json::json!({"c": 2}) }]).unwrap();
    assert!(out.match_indices('\n').all(|(i, _)| i > 0 && out.as_bytes()[i - 1] == b'\r'), "{out:?}");
}
#[test]
fn bom_is_kept() { /* "\u{feff}{\"a\": 1}" set /b 2 → starts_with('\u{feff}') and read_value ok */ }
#[test]
fn whitespace_only_file_starts_as_an_object() { /* "  \n" set /a 1 → read_value == {"a":1} */ }
```

- [ ] **Step 2: Run** `cargo test -p marshell-core config::jsonc`. Expected: compile FAIL.
- [ ] **Step 3: Implement** `apply_ops` and `read_value`, following the rules above.
- [ ] **Step 4: Run** `cargo test -p marshell-core config::jsonc`. Expected: 13 passed.
- [ ] **Step 5: Commit** `feat(config): JSON pointer edits on JSONC through the CST, failing closed on a mismatch`.

### Task 3: The write guard

**Files:**
- Create: `crates/core/src/config/guard.rs`
- Modify: `crates/core/src/projects.rs` (make `canonical` reachable as `pub(crate)` if it isn't already)

**Interfaces:**
- Consumes: `projects::canonical`, `projects::is_inside`, `projects::list`.
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal { NotAbsolute, Managed, OutsideAllowed, MarshellOwn }
#[derive(Clone)]
pub struct Guard { /* allowed roots, denied folders, marshell home */ }
impl Guard {
    pub fn new(allowed: Vec<PathBuf>, denied: Vec<PathBuf>, marshell_home: PathBuf) -> Self;
    /// Allowed: the user's home (`dirs::home_dir`), every folder in `projects.json`, and `<home>/agents`. Denied: `denied_for_os()`.
    pub fn for_home(home: &Path) -> Self;
    /// The resolved path (links and `..` resolved) when the service may write it.
    pub fn check(&self, path: &Path) -> Result<PathBuf, Refusal>;
}
pub fn denied_for_os() -> Vec<PathBuf>;
pub const MANAGED_NAMES: [&str; 3] = ["managed-settings.json", "managed-mcp.json", "managed_config.toml"];
```

**How `check` decides, in this order:**
1. A relative path is `NotAbsolute`.
2. The path is resolved: the deepest existing ancestor goes through `canonical`, the rest is appended, and `..` is tidied.
3. If it's inside the Marshell home, only `<home>/agents/**` passes; anything else there is `MarshellOwn`.
4. It is `Managed` if it's inside a denied folder, or if its file name is in `MANAGED_NAMES` (case-insensitive on Windows).
5. It passes only if it's inside an allowed root; otherwise it is `OutsideAllowed`.

**`denied_for_os`:**
- Windows: `%ProgramFiles%`, `%ProgramFiles(x86)%`, `%ProgramData%` and `%SystemRoot%`. When a variable is unset, use `C:\Program Files`, `C:\Program Files (x86)`, `C:\ProgramData` and `C:\Windows`.
- macOS: `/Library`, `/System`, `/etc` and `/private/etc`.
- Linux: `/etc` and `/usr`.

- [ ] **Step 1: Write the failing tests**. Use a tempdir `root` with `Guard::new(vec![root], vec![root/org], root/home)`:
  - `a_file_in_an_allowed_folder_passes`: `root/proj/.claude/settings.json` gives `Ok` with a resolved path that ends in `settings.json`.
  - `a_managed_folder_is_refused_even_inside_an_allowed_one`: `root/org/x.json` gives `Err(Managed)`.
  - `managed_file_names_are_refused_anywhere`: `root/a/managed-settings.json` gives `Managed`. On Windows, `root/a/MANAGED-SETTINGS.JSON` also gives `Managed`.
  - `outside_every_allowed_folder_is_refused`: a second tempdir gives `OutsideAllowed`.
  - `dot_dot_cannot_climb_out`: `root/a/../../x.json` gives `OutsideAllowed`.
  - `marshell_home_is_closed_except_agents`: `root/home/managed.json` gives `MarshellOwn`, and `root/home/agents/claude/statusline-chain.json` gives `Ok`.
  - `relative_path_is_refused`: `.claude/settings.json` gives `NotAbsolute`.
  - `a_link_out_of_the_allowed_folder_is_refused`: a directory symlink `root/link` pointing at a second tempdir, then `root/link/x.json`, gives `OutsideAllowed`. Create the link with `std::os::unix::fs::symlink` or `std::os::windows::fs::symlink_dir`; if creating it fails (Windows without developer mode), the test returns early after printing why.
  - `#[cfg(windows)] windows_denies_program_files`: `Guard::new(vec!["C:\\".into()], denied_for_os(), root/home).check("C:\\Program Files\\ClaudeCode\\settings.json")` gives `Managed`, and the same for `C:\ProgramData\OpenAI\Codex\requirements.toml`.
- [ ] **Step 2: Run** `cargo test -p marshell-core config::guard`. Expected: compile FAIL.
- [ ] **Step 3: Implement** the guard, following the rules above.
- [ ] **Step 4: Run** `cargo test -p marshell-core config::guard`. Expected: all pass. On a machine where the symlink can't be made, that test prints and returns.
- [ ] **Step 5: Commit** `feat(config): a write guard that allows home and project folders and refuses managed paths`.

### Task 4: Atomic replace and remove

**Files:**
- Create: `crates/core/src/config/write.rs`

**Interfaces:**
- Produces:

```rust
/// The file a write lands in: the link's target when `path` is a symlink, else `path` itself.
pub fn resolve_link(path: &Path) -> std::io::Result<PathBuf>;
/// Writes `bytes` to `.<name>.marshell-<pid>-<n>.tmp` in the same folder, fsyncs it, keeps the old file's
/// unix mode, and renames it over `path` (5 tries, 50 ms apart). Missing parent folders are created.
pub fn replace_file(path: &Path, bytes: &[u8]) -> std::io::Result<()>;
/// Removes `path` with the same retries; a missing file is Ok.
pub fn remove_file(path: &Path) -> std::io::Result<()>;
```

These are user config files, so they are not made private: the folder keeps its permissions, and a new file gets the default mode. The retry loop follows `bridge::swap_in`. Inject the rename through a private `replace_with(path, bytes, rename: impl Fn(&Path, &Path) -> io::Result<()>)` so the tests can make it fail.

- [ ] **Step 1: Write the failing tests**:
  - `replaces_contents_and_leaves_no_temp`: after two writes, the folder holds only the file, and its content is the second write.
  - `creates_missing_parent_folders`: `dir/.claude/settings.local.json`.
  - `a_locked_rename_is_retried`: a rename that fails twice with `PermissionDenied` and then succeeds gives `Ok`, and the new content is in place.
  - `a_rename_that_keeps_failing_leaves_the_old_file`: a rename that always fails gives `Err`, the old content is untouched, and no `.tmp` is left.
  - `#[cfg(unix)] keeps_unix_mode`: a 0640 file is still 0640 after the replace.
  - `writes_through_a_link_to_its_target`: `link.json` points at `real.json`. After `replace_file(&resolve_link(link)?, ..)`, `real.json` has the new bytes and `link.json` is still a symlink (`symlink_metadata().file_type().is_symlink()`). If creating the link fails, the test returns early after printing why.
  - `remove_missing_is_ok`.
- [ ] **Step 2: Run** `cargo test -p marshell-core config::write`. Expected: compile FAIL.
- [ ] **Step 3: Implement** the three functions as described above.
- [ ] **Step 4: Run** `cargo test -p marshell-core config::write`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(config): atomic replace with retries that writes through links`.

### Task 5: Snapshots

**Files:**
- Create: `crates/core/src/config/snapshots.rs`
- Modify: `crates/core/src/paths.rs` (add `snapshots_dir(home) -> <home>/snapshots`)
- Modify: `crates/protocol/src/lib.rs` and create `crates/protocol/src/config.rs`, with only the types this task needs: `ConfigScope`, `Applies`, `SnapshotInfo` and `SnapshotFileInfo` (Task 7 adds the rest)

**Interfaces:**
- Produces, in protocol (each derive and `#[ts(export)]` follows `api.rs`, and optional fields use `api.rs`'s `#[ts(optional)]` pattern):

```rust
#[serde(rename_all = "lowercase")] pub enum ConfigScope { User, Project, Local, Marshell }
#[serde(rename_all = "camelCase")] pub enum Applies { Live, NewSessions, RestartSession }
pub struct SnapshotInfo { pub id: String, pub created_ms: u64, pub changeset_id: String, pub title: String, pub files: Vec<SnapshotFileInfo> }
pub struct SnapshotFileInfo { pub path: String, pub before_hash: Option<String>, pub after_hash: Option<String>, pub scope: ConfigScope, pub applies: Applies }
```

- Produces, in core:

```rust
pub const KEEP_PER_FILE: usize = 20;
pub struct SnapshotInput { pub path: PathBuf, pub before: Option<Vec<u8>>, pub after_hash: Option<String>, pub scope: ConfigScope, pub applies: Applies }
pub struct Snapshots { /* <home>/snapshots */ }
impl Snapshots {
    pub fn new(home: &Path) -> Self;
    pub fn take(&self, changeset_id: &str, title: &str, files: &[SnapshotInput], now_ms: u64) -> anyhow::Result<String>;
    pub fn list(&self, path: Option<&Path>) -> anyhow::Result<Vec<SnapshotInfo>>; // newest first
    pub fn get(&self, id: &str) -> anyhow::Result<Option<SnapshotInfo>>;
    /// None: no such snapshot or file in it. Some(None): the file did not exist before.
    pub fn blob(&self, id: &str, path: &Path) -> anyhow::Result<Option<Option<Vec<u8>>>>;
    pub fn discard(&self, id: &str) -> anyhow::Result<()>;
}
```

**Storage:**
- The index is `snapshots/index.json`, a `store::Versioned` v1 list of entries, with one static lock around every read-modify-write.
- Each blob is `snapshots/<id>/<n>.bin`, written with `store::write_bytes_atomic`.
- Ids come from `auth::new_id("s")`. Any id that isn't `s` followed by 12 lowercase hex characters is unknown, so `../x` can never reach the disk.
- Paths are compared with `projects::same_path`.

**Retention:** after `take`, for each path in the new snapshot, keep that path's newest 20 file entries. The older ones lose that file entry and its blob, and an entry left with no files is removed along with its folder.

- [ ] **Step 1: Write the failing tests**:
  - `take_and_read_back_a_blob`
  - `a_file_that_did_not_exist_reads_back_as_absent`: `blob` gives `Some(None)`.
  - `keeps_the_last_20_per_file`:
    - Take 21 snapshots of path A, with `now_ms` running from 1 to 21, and one snapshot of B after the 10th.
    - Then `list(Some(A)).len() == 20`, and the one with `created_ms == 1` is gone.
    - Its blob folder no longer exists.
    - `list(Some(B)).len() == 1`.
  - `list_is_newest_first`
  - `unknown_and_malformed_ids_are_none`: `get("s000000000000")`, `get("../index")` and `blob("..\\x", A)` all give `None`.
  - `discard_removes_the_entry_and_its_blobs`
- [ ] **Step 2: Run** `cargo test -p marshell-core config::snapshots`. Expected: compile FAIL.
- [ ] **Step 3: Implement** `Snapshots` and the protocol types.
- [ ] **Step 4: Run** `cargo test -p marshell-core config::snapshots` and `cargo test -p marshell-protocol`. Expected: pass, and `src/generated/` gains the four new types.
- [ ] **Step 5: Commit** `feat(config): snapshots with an index, keeping the last 20 per file`, including `src/generated/`.

### Task 6: managed.json

**Files:**
- Create: `crates/core/src/config/managed.rs`
- Modify: `crates/core/src/paths.rs` (add `managed_file(home) -> <home>/managed.json`)

**Interfaces:**
- Produces:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ManagedEntry { pub path: String, pub pointer: Option<String>, pub changeset_id: String, pub applied_ms: u64 }
pub enum Change<'a> { Set { path: &'a Path, pointer: &'a str }, Remove { path: &'a Path, pointer: &'a str }, Created { path: &'a Path }, Restored { path: &'a Path, changeset_id: &'a str } }
pub fn list(home: &Path) -> anyhow::Result<Vec<ManagedEntry>>;
/// Computes the new list without writing it, so apply can write files first and roll back if this fails.
pub fn updated(home: &Path, changeset_id: &str, changes: &[Change], now_ms: u64) -> anyhow::Result<Vec<ManagedEntry>>;
pub fn save(home: &Path, entries: &[ManagedEntry]) -> anyhow::Result<()>;
```

**Rules:**
- `Set` upserts on `(same_path, pointer)`, refreshing `changeset_id` and `applied_ms`.
- `Remove` drops the entries for that path whose pointer equals the pointer or starts with `pointer + "/"`.
- `Created` upserts `(path, None)`.
- `Restored` drops that path's entries that carry the given `changeset_id`.
- The file is a `store::Versioned` v1 `{ "entries": [...] }`, so a file from a newer Marshell is refused and left alone (`store` already does that).

- [ ] **Step 1: Write the failing tests**:
  - `set_then_remove_leaves_nothing`
  - `set_twice_keeps_one_entry`
  - `removing_a_parent_drops_its_children`
  - `restore_drops_only_that_changesets_entries`
  - `a_newer_managed_file_is_left_alone`: write `{"version":9,"entries":[]}`; `updated` fails and the file's bytes are unchanged.
- [ ] **Step 2: Run** `cargo test -p marshell-core config::managed`. Expected: compile FAIL.
- [ ] **Step 3: Implement** the module.
- [ ] **Step 4: Run** `cargo test -p marshell-core config::managed`. Expected: 5 passed.
- [ ] **Step 5: Commit** `feat(config): managed.json records every entry Marshell wrote`.

### Task 7: Protocol types and preview

**Files:**
- Modify: `crates/protocol/src/config.rs`
- Create: `crates/core/src/config/preview.rs`

**Interfaces:**
- Consumes: Tasks 2–5.
- Produces, in protocol:

```rust
pub struct ChangeSet { pub title: String, pub reason: String, pub edits: Vec<FileEdit> }
pub struct FileEdit { pub path: String, pub scope: ConfigScope, pub op: EditOp, pub expected_hash: Option<String>, pub applies: Applies }
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EditOp { JsonSet { pointer: String, value: String /* JSON text */ }, JsonRemove { pointer: String }, CreateFile { content: String }, Restore { snapshot_id: String } }
pub struct ConfigPreview { pub changeset_id: String, pub title: String, pub reason: String, pub files: Vec<FilePreview>, pub stale: bool, pub can_apply: bool }
pub struct FilePreview { pub path: String, pub scope: ConfigScope, pub applies: Applies, pub before_hash: Option<String>, pub after_hash: Option<String>, pub diff: String, pub status: FileStatus }
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FileStatus { Changes, NoChange, Refused { reason: RefuseReason, detail: String } }
#[serde(rename_all = "camelCase")]
pub enum RefuseReason { NotAbsolute, Managed, OutsideAllowed, MarshellOwn, Unreadable, TooLarge, ReadOnly, NotUtf8, DoesNotParse, BadPointer, BadValue, Conflict, Exists, SnapshotMissing }
pub struct ApplyRequest { pub changeset_id: String, pub expected_hashes: Vec<ExpectedHash> }
pub struct ExpectedHash { pub path: String, pub hash: Option<String> }
pub struct ApplyResult { pub snapshot_id: String, pub files: Vec<AppliedFile> }
pub struct AppliedFile { pub path: String, pub after_hash: Option<String> }
pub struct ApplyConflict { pub reason: String, pub paths: Vec<String> }
pub struct ConfigFile { pub path: String, pub exists: bool, pub hash: Option<String>, pub content: Option<String>, pub refused: Option<RefuseReason> }
```

`value` is JSON text rather than a JSON value, so the protocol crate needs no `serde_json` dependency.

- Produces, in core:

```rust
pub const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;
pub struct PlannedFile { pub path: PathBuf, pub target: PathBuf, pub scope: ConfigScope, pub applies: Applies,
                         pub before: Option<Vec<u8>>, pub after: Option<Vec<u8>>, pub status: FileStatus,
                         pub ops: Vec<EditOp> /* this file's ops, in order, for managed.json */ }
/// Groups the edits by path (first-seen order) and works out each file's new bytes. Never writes.
pub fn plan(cs: &ChangeSet, guard: &Guard, snapshots: &Snapshots) -> Vec<PlannedFile>;
pub fn to_preview(changeset_id: &str, cs: &ChangeSet, files: &[PlannedFile]) -> ConfigPreview;
```

**Per file, in order (the first refusal wins, with `before`/`after` left `None`):**
1. `guard.check` maps each `Refusal` to the `RefuseReason` of the same name. `target` is `write::resolve_link(resolved)`.
2. Reading `target`:
   - `NotFound` gives `before = None`.
   - Any other error gives `Unreadable`.
   - A file over `MAX_CONFIG_BYTES` gives `TooLarge`.
   - `metadata.permissions().readonly()` gives `ReadOnly`.
3. Any of the file's edits with `expected_hash = Some(h)` where `h != hash(before)` gives `Conflict`.
4. The ops run on a working copy:
   - JSON ops on non-UTF-8 bytes give `NotUtf8`, and an unparseable `value` gives `BadValue`.
   - A `JsonSet` on a missing file starts from `"{}\n"`; a `JsonRemove` on a missing file changes nothing.
   - The `jsonc` errors map to `DoesNotParse` and `BadPointer`, and `Internal` maps to `DoesNotParse` with its detail.
   - `CreateFile` on an existing file gives `Exists`.
   - `Restore` takes `snapshots.blob(id, path)`: `None` gives `SnapshotMissing`, and `Some(None)` means the file is deleted.
5. The status is `Changes` if `after != before`, otherwise `NoChange`.

**Preview:**
- The diff is `diffy::create_patch(before_text, after_text).to_string()`, using lossy UTF-8 and `""` for a missing side. Refused and NoChange files have `""`.
- `can_apply` is true when no file is refused and at least one has `Changes`.
- `stale` is false here.

- [ ] **Step 1: Write the failing tests** in `preview.rs`. Use a tempdir `root` and `Guard::new(vec![root], vec![], root/home)`, and an empty `Snapshots`.
  - `json_set_on_a_missing_file_creates_it`: Changes, `before_hash` None, and `after` reads as `{"a":1}`.
  - `a_managed_path_is_refused_and_shows_no_diff`: `root/managed-settings.json` gives Refused Managed, `diff == ""` and `!can_apply`.
  - `expected_hash_mismatch_is_a_conflict`
  - `two_edits_to_one_file_make_one_preview`: `files.len() == 1`, and both keys are in `after`.
  - `create_file_over_an_existing_file_is_refused`: Exists.
  - `unparseable_file_is_refused_not_described_as_changing`: DoesNotParse and `diff == ""`.
  - `read_only_file_is_refused`: set read-only with `set_permissions`, and clear it before the tempdir drops.
  - `remove_of_missing_pointer_is_no_change`: NoChange and `!can_apply`.
  - `diff_is_unified`: setting `/a` on `{}` gives a diff containing `"@@"` and a line starting with `+` that contains `"a"`.
  - `bad_value_text_is_refused`: `value: "{nope"` gives BadValue.
- [ ] **Step 2: Run** `cargo test -p marshell-core config::preview`. Expected: compile FAIL.
- [ ] **Step 3: Implement** the protocol types, `plan` and `to_preview`.
- [ ] **Step 4: Run** `cargo test -p marshell-core config::preview` and `cargo test -p marshell-protocol`. Expected: pass.
- [ ] **Step 5: Commit** `feat(config): preview a ChangeSet as per-file diffs, refusing what it may not write`, including `src/generated/`.

### Task 8: ConfigService: previews, apply, restore

**Files:**
- Create: `crates/core/src/config/service.rs`

**Interfaces:**
- Consumes: Tasks 1–7.
- Produces:

```rust
pub const MAX_PREVIEWS: usize = 32;
pub const PREVIEW_TTL: Duration = Duration::from_secs(600);
#[derive(Debug)]
pub enum ApplyError { UnknownChangeSet, Conflict { reason: String, paths: Vec<String> }, Failed(anyhow::Error) }
pub struct ConfigService { /* Arc<Shared>: home, guard source, previews, per-path locks, own-write hashes, snapshots, watcher (Task 9) */ }
impl ConfigService {
    pub fn new(home: PathBuf) -> Self;                       // guard = Guard::for_home(home) per call
    pub fn with_guard(home: PathBuf, guard: Guard) -> Self;   // tests
    pub fn preview(&self, cs: ChangeSet) -> ConfigPreview;
    pub fn get_preview(&self, id: &str) -> Option<ConfigPreview>;
    pub fn apply(&self, req: &ApplyRequest, now_ms: u64) -> Result<ApplyResult, ApplyError>;
    pub fn restore_preview(&self, snapshot_id: &str) -> Option<ConfigPreview>;
    pub fn snapshots(&self) -> &Snapshots;
}
```

**Previews** are stored under `auth::new_id("c")` with the ChangeSet, the planned targets and the time they were made. A preview older than `PREVIEW_TTL` is dropped on the next call. When there are more than `MAX_PREVIEWS`, the oldest go first.

**`apply`** runs `apply_with(req, now_ms, write: &dyn Fn(&Path, Option<&[u8]>) -> io::Result<()>)`, and the real writer is `write::replace_file` / `write::remove_file`.
1. Look up the preview; `UnknownChangeSet` if it's missing. If it's stale, `Conflict { reason: "changed outside Marshell" }`. If `!can_apply`, `Conflict { reason: "refused" }` with the refused paths.
2. Each `expected_hashes` entry must match the preview's `before_hash` for that path, and every previewed file must be listed. Otherwise `Conflict { reason: "not what was shown" }`.
3. Lock every target's per-path lock in sorted order (`HashMap<PathBuf, Arc<Mutex<()>>>`), and hold them to the end.
4. Re-plan from disk. Any file whose `before_hash` changed, or that is now refused, gives `Conflict { reason: "changed" }`, and nothing is written.
5. Take a snapshot of the files with `Changes`.
6. Compute `managed::updated(..)` from the ops:
   - JsonSet becomes `Set`.
   - JsonRemove becomes `Remove`.
   - CreateFile becomes `Created`.
   - Restore becomes `Restored`, with the restored snapshot's `changeset_id`.
   - If this fails, `discard` the snapshot and return `Failed`.
7. For each file in order, record `own_writes[target] = after_hash`, then `write(target, after)`. If file *k* fails, write back the `before` of files 0..k (`None` means remove), `discard` the snapshot, and return `Failed`.
8. Run `managed::save`. If it fails, roll back as in step 7 and return `Failed`.
9. Remove this preview. Mark every other stored preview that touches a written target stale. Return `ApplyResult`.

**`restore_preview(id)`** builds a ChangeSet:
- The title is `Restore: <snapshot title>`.
- The reason is "Puts the files back as they were before this change."
- It has one `Restore` edit per snapshot file, carrying that file's `scope` and `applies`. Its `expected_hash` is `None`, because a file changed since still shows its diff and the user decides.
- It is previewed through `preview`.
- An unknown id gives `None`.

- [ ] **Step 1: Write the failing tests** in `service.rs`, using `with_guard` on a tempdir root. The helper `apply_shown(svc, &preview)` sends the preview's own hashes.
  - `apply_writes_snapshots_and_records_managed`: after a set on `/hooks/Stop`, the file has it, `snapshots().list(Some(path)).len() == 1`, and `managed::list` has `(path, Some("/hooks/Stop"))`.
  - `apply_needs_the_hashes_that_were_shown`: a wrong hash gives `Conflict`, and the file is untouched.
  - `a_file_changed_after_preview_is_a_conflict_and_untouched`: preview, then `fs::write(path, "{\"mine\":1}")`, then apply gives `Conflict`, and the file still reads `{"mine":1}`.
  - `a_failed_second_write_rolls_back_the_first`:
    - Two files, A and B. The injected writer fails on B.
    - The result is `Failed`.
    - A's bytes equal the original.
    - `snapshots().list(None)` is empty.
    - `managed.json` doesn't exist.
  - `concurrent_applies_on_one_file_one_conflicts`:
    - Two previews of the same file from the same `before` (keys `x` and `y`), applied from two threads behind a `Barrier`.
    - Exactly one `Ok` and one `Err(Conflict)`.
    - The file has exactly one of `x` and `y`.
  - `a_preview_applies_once`: the second apply gives `UnknownChangeSet`.
  - `applying_one_preview_makes_others_on_that_file_stale`
  - `restore_puts_the_file_back_and_is_itself_undoable`: apply, then `restore_preview(snapshot_id)` and apply it. The bytes equal the original, and `snapshots().list(Some(path)).len() == 2`.
  - `restore_of_a_created_file_deletes_it`
  - `refused_preview_cannot_apply`
  - `old_previews_are_dropped`: after 33 previews, the first id gives `UnknownChangeSet`.
- [ ] **Step 2: Run** `cargo test -p marshell-core config::service`. Expected: compile FAIL.
- [ ] **Step 3: Implement** `ConfigService`, following the apply steps above.
- [ ] **Step 4: Run** `cargo test -p marshell-core config::service`. Expected: 11 passed. Run it 5 times for the race test: `for ($i=0;$i -lt 5;$i++){ cargo test -p marshell-core config::service::tests::concurrent -q }`.
- [ ] **Step 5: Commit** `feat(config): apply only what was shown, with locks, a snapshot, rollback and restore`.

### Task 9: Watching and the 3-way merge

**Files:**
- Create: `crates/core/src/config/watch.rs`, `crates/core/src/config/merge.rs`
- Modify: `crates/core/src/config/service.rs`

**Interfaces:**
- Produces:

```rust
// merge.rs
/// Ok(merged) when the edits don't overlap; Err(text with <<<<<<< ======= >>>>>>> markers) when they do.
pub fn merge3(base: &str, ours: &str, theirs: &str) -> Result<String, String>;   // diffy::merge
// watch.rs
pub const DEBOUNCE: Duration = Duration::from_millis(200);
pub struct Watcher { /* notify::RecommendedWatcher + the folders watched */ }
impl Watcher {
    /// `on_change` gets each changed path once, after 200 ms with no further event for it.
    pub fn start(on_change: impl Fn(PathBuf) + Send + 'static) -> notify::Result<Self>;
    /// Watches the file's folder (non-recursive) once; later calls for the same folder do nothing.
    pub fn watch_file(&mut self, file: &Path) -> notify::Result<()>;
}
```

**Service wiring:**
- `preview` starts the watcher the first time, and watches each target that is allowed and resolved. A watcher error is logged with `tracing::warn!` and doesn't fail the preview.
- `on_change(path)` holds a `Weak<Shared>`. It hashes the file, and if `own_writes[path]` equals that hash it ignores the change. Otherwise it marks every stored preview with that target stale.
- The debounce thread collects paths from notify's channel and fires each one once it has been quiet for `DEBOUNCE`.

- [ ] **Step 1: Write the failing tests**:
  - `merge.rs`:
    - `non_overlapping_edits_merge`: base `"a\nb\nc\n"`, ours `"A\nb\nc\n"`, theirs `"a\nb\nC\n"` gives `Ok("A\nb\nC\n")`.
    - `overlapping_edits_conflict_with_markers`: the `Err` contains `"<<<<<<<"`.
  - `service.rs`, polling `get_preview(id).unwrap().stale` every 50 ms for up to 5 s:
    - `an_outside_edit_marks_the_preview_stale`
    - `our_own_write_does_not_mark_a_new_preview_stale`: apply P1, preview P3 on the same file straight away, wait 1 s; P3 isn't stale.
- [ ] **Step 2: Run** `cargo test -p marshell-core config::merge config::service`. Expected: the new tests FAIL.
- [ ] **Step 3: Implement** the merge, the watcher and the service wiring.
- [ ] **Step 4: Run** `cargo test -p marshell-core config`. Expected: all pass. Run the two watch tests 3 times.
- [ ] **Step 5: Commit** `feat(config): watch managed files, ignore our own writes, and add the 3-way merge`.

### Task 10: The HTTP routes

**Files:**
- Create: `crates/core/src/server/config.rs`, `crates/core/tests/config_api.rs`
- Modify: `crates/core/src/server/mod.rs` (`Inner.config: crate::config::service::ConfigService`, `AppState::config()`, routes)

**Interfaces:**
- Consumes: `ConfigService` (Task 8).
- Produces, with each handler running its work in `spawn_blocking`:

| Route | OK | Errors |
|---|---|---|
| `GET /v1/config/file?path=` | `ConfigFile` | `refused` set and no content for NotAbsolute, OutsideAllowed and MarshellOwn. A Managed file **is** readable. Content only for UTF-8 files up to `MAX_CONFIG_BYTES`. |
| `POST /v1/config/preview` (body `ChangeSet`) | `ConfigPreview` | |
| `GET /v1/config/preview/{id}` | `ConfigPreview` | 404 |
| `POST /v1/config/apply` (body `ApplyRequest`) | `ApplyResult` | 404 UnknownChangeSet, 409 `ApplyConflict`, 500 text for Failed |
| `GET /v1/snapshots?path=` | `Vec<SnapshotInfo>` | |
| `POST /v1/snapshots/{id}/restore` | `ConfigPreview` (apply it with `/v1/config/apply`) | 404 |

PLAN §3 lists `POST /snapshots/:id/restore`. Ruling: it returns a preview instead of writing, so a restore also goes "preview, then the user sees the diff, then apply".

- [ ] **Step 1: Write the failing tests** in `crates/core/tests/config_api.rs`:
  - The setup follows `permissions.rs` (`AppState::new` with a temp home), and a project tempdir is recorded with `marshell_core::projects::record(home, project, 1)`.
  - `preview_then_apply_writes_the_file`: POST a preview that sets `/hooks/Stop`, POST apply with the hashes from the preview, then the file has the entry and `GET /v1/snapshots?path=` lists 1.
  - `apply_after_an_outside_edit_is_409_and_the_file_is_untouched`
  - `restore_route_returns_a_preview_that_puts_the_file_back`
  - `a_managed_path_previews_as_refused`: `<project>/managed-settings.json` gives `can_apply == false` and a status of `{"kind":"refused","reason":"managed",..}`; apply gives 409.
  - `unknown_preview_is_404`
  - `config_routes_need_the_token`: with no `Authorization`, `/v1/config/preview` gives 401.
  - `file_route_reads_an_allowed_file_and_refuses_marshell_files`: `<home>/managed.json` gives `refused: "marshellOwn"`.
- [ ] **Step 2: Run** `cargo test -p marshell-core --test config_api`. Expected: FAIL (404s).
- [ ] **Step 3: Implement** the handlers and routes.
- [ ] **Step 4: Run** `cargo test -p marshell-core --test config_api`, `cargo test -p marshell-protocol` and `pnpm check:generated`. Expected: all pass, and check:generated exits 0.
- [ ] **Step 5: Commit** `feat(server): config preview, apply, file, snapshot and restore routes`.

### Task 11: Owner-only permissions on the Marshell home

**Files:**
- Create: `crates/core/src/private.rs`, `crates/core/tests/private_home.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/Cargo.toml` (`windows-sys` features: `Win32_Security_Authorization` and whatever else the calls need), `crates/core/src/server/mod.rs` (`start`), `crates/core/src/auth.rs:39-42` (doc comment)

**Interfaces:**
- Produces:

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum HomeAcl { AlreadyPrivate, Fixed }
/// Creates the home if needed and makes it owner-only.
/// Windows: a protected DACL with two allow ACEs, the current user and SYSTEM (S-1-5-18), full control, inherited by
/// files and folders (CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE). When the home's DACL is not exactly that, it is set with
/// TreeResetNamedSecurityInfoW (KeepExplicit = FALSE), so every child goes back to inheriting it.
/// Unix: folders 0700, files 0600, a file with the owner's x bit 0700. Links are not followed.
pub fn secure_home(home: &Path) -> anyhow::Result<HomeAcl>;
#[cfg(windows)] pub struct Dacl { pub protected: bool, pub allow_sids: Vec<String>, pub all_inherited: bool }
#[cfg(windows)] pub fn dacl_of(path: &Path) -> anyhow::Result<Dacl>;
#[cfg(windows)] pub fn current_user_sid() -> anyhow::Result<String>;
```

Only the home's own DACL is checked on each launch. Children are fixed only when the home is reset. Reading every file's ACL on every launch would cost a walk of the recordings and attachments.

`server::start` calls `secure_home(&cfg.home)` before `auth::write_endpoint_file`. A failure is `tracing::warn!` and the launch goes on, because the default home under the user profile is already private to the user. The `auth.rs` comment then says the ACL is set by `private::secure_home`.

- [ ] **Step 1: Write the failing tests** in `crates/core/tests/private_home.rs`:
  - `#[cfg(windows)] home_gets_a_protected_dacl_for_the_user_and_system`:
    - Make a temp home with `sub/f.txt` before the call.
    - `secure_home` gives `Fixed`.
    - `dacl_of(home)` is `protected`, and its sorted `allow_sids` equal the sorted `[current_user_sid(), "S-1-5-18"]`.
    - `dacl_of(home/sub/f.txt)` has the same SIDs and `all_inherited`.
  - `#[cfg(windows)] a_file_created_later_inherits_it`
  - `second_call_is_already_private`: the second call gives `AlreadyPrivate`. Runs on both OSes.
  - `#[cfg(unix)] modes_are_0700_and_0600_and_executables_keep_x`
- [ ] **Step 2: Run** `cargo test -p marshell-core --test private_home`. Expected: compile FAIL.
- [ ] **Step 3: Implement** `private.rs` and the call in `start`.
- [ ] **Step 4: Run** `cargo test -p marshell-core --test private_home` and `cargo test -p marshell-core --test server`. Expected: pass.
- [ ] **Step 5: Commit** `feat(core): make the Marshell home owner-only on every start`.

### Task 12: The threat model ADR

**Files:**
- Create: `docs/adr/0001-threat-model.md`

The ADR records:
- **Context:** a local app that writes other programs' configuration and runs their hooks.
- **Assets:** CLI configs, the API token and `endpoint.json`, terminal input, the instruction library.
- **Attackers in scope:**
  - other local users
  - web pages, through DNS rebinding and cross-origin requests
  - repository content an agent reads, for instruction injection
  - a hostile `marshell://` link
- **Out of scope:** malware running as the same user, and administrators.
- **For each entry point,** the controls that exist today, with the file that holds them:
  - the loopback server: Host check, Origin and CORS, the token, the WS subprotocol (`server/guard.rs`)
  - `/sessions/:id/input`: token only; Marshell never presses Enter on its own
  - `/hook`: token and body cap; the bridge fails silently
  - permission holds: only one answer wins; typing releases
  - the config service: guard, preview, hashes, snapshots, no elevation
  - the `marshell://` link: focus only, with no authority (phase 4 builds it)
  - the instruction library: phase 7, with the risk named
- **Owner-only home:** what the `private.rs` ACL protects.
- **Residual risks:** the token is readable by any process of the same user; a managed path not on the list.

- [ ] **Step 1: Write the ADR**, checking each named control against the code it names.
- [ ] **Step 2: Commit** `docs(adr): the threat model for the local API, hooks and config writes`.

### Task 13: Live check

Run the real app as `CLAUDE.md` "Live check (Windows)" describes, with a scratch `MARSHELL_HOME` under a path containing a space, and a scratch project folder.

1. After launch, run `icacls "<scratch home>"`. Expected: only the user and `NT AUTHORITY\SYSTEM`, `(OI)(CI)(F)`, no inherited entries.
2. Over CDP, get the endpoint with `core_endpoint`, then `fetch` against the API:
   - Preview a JsonSet into `<project>/.claude/settings.local.json` (a new file), then apply it.
   - Preview an edit to a JSONC file with comments, and check that the comments survive in the file on disk.
   - Edit the file on disk between preview and apply. `GET /v1/config/preview/{id}` shows `stale: true` within 1 s, and apply is a 409.
   - Restore the first snapshot through `/v1/snapshots/{id}/restore` and apply it; the file is gone again.
   - Preview `<project>/managed-settings.json`; it's refused.
3. Write the results to `docs/research/2026-10-18-config-safety-live-check.md` and commit `docs(research): config safety live check`.
4. Stop the app by PID and remove the scratch folders.

---

## Exit criteria

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `pnpm test`, `pnpm check:generated` and `pnpm build` all pass.
- The live check passes and is written up.
- The Opus whole-branch review has run, followed by its fix pass.
