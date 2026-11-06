# Slice 7c2: the usage scan and "Measure MCP servers" — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Doctor reads each CLI's history to report MCP servers and skills unused in 30 days, and user-level ones used in one project only. A "Measure MCP servers" button starts each approved local server once, reads its tool list, reports the servers that don't start and shows each CLI's MCP token cost.

**Architecture:**
- **Readers stay in the adapters.** Each `agents/<cli>/extensions.rs` (7c1) now also says how its CLI starts a server: the launch, whether it is approved, what loads up front, and how history names it. A new `agents/<cli>/usage.rs` reads that CLI's history, incrementally, into vendor-neutral events.
- **Pure engines in `doctor/`:**
  - `probe.rs` speaks MCP over stdio to one process and ends its whole tree.
  - `measure.rs` turns measurements into items.
  - `usage.rs` keeps the usage store and turns it into items.
  - `ext.rs` composes them with 7c1's items.
- **Server wiring:**
  - `server/doctor_measure.rs` runs the click-only measurement.
  - `server/doctor_usage.rs` runs the scan inside every background run and keeps `<home>/doctor/usage.json`.
  - `server/doctor.rs` gets one route and the fix arms.
- **Frontend:** one button and one status line, through a hook shared with "Run CLI doctors".

No new dependencies. `rusqlite`, `serde_json`, `toml_edit`, `regex`, `sha2` and `windows-sys` (Job Objects) are already in `marshell-core`.

**Tech Stack:** Rust (axum core, `rusqlite` read-only, std processes with 7b's `job::ProcessTree`), React and TypeScript.

**Spec:**
- `docs/PLAN.md` §9, "Slice 7c decisions (2026-11-06)". 7c2 owns decisions 2, 4 and 5, with 13–21 asked at its start. Decisions 3, 6, 7 and 8 (7c1) still apply.
- The facts are in `docs/research/2026-11-06-usage-and-measure.md`, plus 7c1's `docs/research/2026-11-06-mcp-and-skills.md`.
- The code was read on `d7342ef`; line numbers may drift a little.

Existing code this plan builds on:
- **7c1's model** (`crates/core/src/doctor/ext.rs`):
  - `Inventory`, `Server`, `Skill`, `Origin`, `Switch`, `NoSwitch` and `ExtRoots`;
  - `item_key(file, origin, name)` (private today), `fixability(...)`, `FIX_NOTE` and `tokens(chars) = chars.div_ceil(4)`;
  - `build()`, which feeds `items()` and `switch_of()`.
- **7c1's wiring** (`server/doctor_ext.rs`): `windows_roots(st)`, `inventories(st, facts, distro_homes)`, and `judge()`, which gives `CommandState`.
- **The run and fix flow** (`server/doctor.rs`):
  - `run_once` builds a report from `facts_of(st) -> Gathered`.
  - `fix_of` rebuilds the items "now" and needs each picked id fixable in both lists, then dispatches on the id's first segment.
  - `request_cli_run` and `CliRun` (7b) are the pattern for a click-only run whose results ride along in every report.
- **Helpers:**
  - `crate::job::ProcessTree::{adopt, kill}`;
  - `crate::agents::run_bounded`, the pattern for reading pipes on threads under a deadline;
  - `crate::agents::common::rfc3339_ms(&str) -> Option<u64>`;
  - `crate::profiles::detect::command_path_in(path: &OsStr, name) -> Option<PathBuf>` (PATHEXT-aware);
  - `AppState::agent_finder(&[]).path_in_use()`, the PATH Marshell's tabs get;
  - `crate::store::{Versioned, read, write}`, atomic and versioned;
  - `crate::config::hash::hash_bytes`;
  - `crate::projects::list(home)`.
- **Test helpers:**
  - `crates/core/src/bin/fake-agent.rs`, a test-only program that tests reach as `env!("CARGO_BIN_EXE_fake-agent")`;
  - `AppState::with_ext_home`, and a fake home under `target/`, as in 7c1's tests.

## Global Constraints

- **Commits and staging:**
  - Commit messages read `type(scope): what changed, as a sentence`.
  - Run `cargo fmt --all` before every Rust commit.
  - Every cargo command uses `-j 3`, for example `cargo test -j 3 -p marshell-core doctor::usage`.
  - Stage only your own files, by path. Never `git add -A` or `git commit -a`.
  - Run only your own tests; the lead runs the full suites.
- **Worktree:** `C:\dev\Airport\.claude\worktrees\phase-7c2`, branch `phase-7c2`, with plain commands from its root.
- **Vendor code stays in adapters.** File locations, history formats, approval rules, launch rules, deferral rules and move hints live in `agents/<cli>/`. `doctor/` knows no CLI's file names or record shapes.
- **Never in a test or worker:**
  - Start a real MCP server or a real CLI. A test probes only `fake-agent`.
  - Read the real home, `~/.codex/*.sqlite` included. Readers take their roots as arguments, and tests build fake homes under `target/`.
  - Run `wsl.exe` or `pnpm tauri dev`.
- **Secrets:**
  - A server's `env` values, `headers` and its stderr are never shown in an item, logged or stored.
  - Only a hash of a launch is kept, as its fingerprint. A failure item names the kind of failure, never output text.
- **Async:**
  - Never hold a `MutexGuard` across `.await` or I/O: lock, clone, drop, work, lock, merge, drop.
  - File and process work runs in `spawn_blocking`.
  - The one new route sits in the token-guarded router and gets a "refused without the bearer" test.
- **Writes:** the only new fix is 7c1's turn-off switch, reused through `ext::switch_of`. Nothing new writes a CLI's files. `<home>/doctor/usage.json` is Marshell's own file, written with `store::write`.
- **Time:**
  - "30 days" is `usage::WINDOW_MS = 30 * 24 * 60 * 60 * 1000`.
  - Every engine function takes `now_ms`, and only the server reads the clock.
- **Copy:**
  - Sentence case, subject first, no "!", no "successfully". This plan's copy is exact.
  - `<CLI>` is `checks::display_name(agent)`. `<place>` is empty on Windows and ` in <distro>` in a distro, as in 7c1.
  - A path shows with `display()`.

## Review Focus

1. **A use the scan can't attribute must never make an item "unused".** Examples:
   - a plugin server's mangled name (`plugin:a:b` → `plugin_a_b`);
   - a worktree-scoped skill name (`.claude/worktrees/x:marshell-ship`);
   - a skill name in another case;
   - a Codex `SKILL.md` read with forward slashes, or a quoted path holding a space (`C:\Users\Home Office\…`);
   - calls made in subagent files;
   - records out of time order.

   When a use could belong to several items, it counts for each. Tasks 6, 7 and 8 test this.
2. **Coverage after a gap.** Examples:
   - Marshell doesn't run for 40 days while the CLI deletes old files;
   - a file is replaced by a shorter one;
   - a file is deleted between listing and reading;
   - a record is dated in the future;
   - `usage.json` is corrupt or from a newer version.

   Coverage then restarts from what is on disk, never from what was lost, and nothing panics. Task 6 tests the rule, and Task 9 tests the file.
3. **A server that misbehaves when measured.** Examples: one that never answers, floods stdout, prints a banner before its JSON, asks the client a request, exits at once, or starts a grandchild. Each ends within its timeout plus 2 s with its whole tree gone, and the Doctor's background runs aren't held up meanwhile. Tasks 3 and 5 test this.
4. **Starting what the CLI wouldn't.** None of these may be started:
   - a `.mcp.json` server in a folder whose trust dialog wasn't accepted, approved only in the committed settings;
   - a Codex server in an untrusted project;
   - a Qwen Code server whose approval hash doesn't match;
   - a server that is off;
   - a remote server;
   - a server in a distro.

   Tasks 1, 2 and 5 test this.
5. **One server, one verdict.** A server with a 7c1 Broken item ("can't start") or a failed measurement gets no "unused" item too. Picking an unused item and its Broken item in one fix gives one edit, not two. Tasks 4, 6 and 9 test this.

---

### Task 1: Inventory additions, and Claude Code's and Qwen Code's launch rules

**Files:**
- Modify: `crates/core/src/doctor/ext.rs` (types only, and `item_key` made `pub(crate)`)
- Modify: `crates/core/src/agents/claude/extensions.rs`, `crates/core/src/agents/qwen/extensions.rs`
- Modify: `crates/core/src/server/doctor_ext.rs` (`ExtRoots` fields, filled from the environment in `windows_roots`)
- Test: the `#[cfg(test)]` modules of the two readers

**Interfaces:**
- Produces, in `doctor/ext.rs` (Tasks 2–9 use these names):

```rust
pub struct Launch {
    pub command: String,
    pub args: Vec<String>,
    /// The entry's own variables, as written. Never shown or logged.
    pub env: Vec<(String, String)>,
    pub inherit: Inherit,
    /// Variables the CLI sets for the server itself (CLAUDE_PROJECT_DIR, CLAUDE_PLUGIN_ROOT).
    pub set: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
    pub expand: Expand,
    pub timeout_secs: u64,
}
pub enum Inherit { All, Only(Vec<String>) }
/// How the CLI expands variables in command, args, env values and cwd.
pub enum Expand { None, Braces /* ${VAR}, ${VAR:-default} */, Dollar /* $VAR, ${VAR} */ }
/// Why a server isn't started by "Measure MCP servers".
pub enum NotMeasured { Remote, NotApproved }
pub struct CostRule {
    pub listing: Listing,
    /// A tool whose `_meta` has this key set to `true` loads in full whatever `listing` says.
    pub always_load_meta: Option<&'static str>,
    /// When set, only these tools count.
    pub only: Option<Vec<String>>,
    /// These tools never count.
    pub except: Vec<String>,
}
pub enum Listing {
    /// Each tool's full definition: `prefix` + name, its description cut at `desc_cap`, its input schema.
    Full { prefix: String, desc_cap: Option<usize> },
    /// Each tool's name (`prefix` + name), and the server's instructions cut at `instructions_cap` (Claude Code).
    Names { prefix: String, instructions_cap: Option<usize> },
    /// One line: `namespace`, and the first line of the instructions cut at `cap` (Codex).
    Namespace { namespace: String, cap: usize },
    /// `heading`, then each tool's name and the first line of its description, cut at `cap` (Qwen Code).
    FirstLines { heading: String, cap: usize },
    /// Each tool's name only (Antigravity).
    NamesOnly,
    /// Claude Code's `auto:N`: Full while every such server's full cost totals under `percent`% of 200,000 tokens,
    /// else Names.
    FullUnder { percent: u32, prefix: String, desc_cap: Option<usize>, instructions_cap: Option<usize> },
}
/// How the CLI's history names a server's calls.
pub enum UsedAs { ToolPrefix(String), Server(String), Unknown }
/// How the CLI's history names a skill's use.
pub enum SkillUse { Name(String), File(PathBuf) }
/// The move hint of a broad-scope item: (name, project) -> the sentence.
pub struct Hints { pub move_server: fn(&str, &Path) -> String, pub move_skill: fn(&str, &Path) -> String }
```

- New fields:
  - **`Server`:** `launch: Result<Launch, NotMeasured>`, `cost: CostRule` and `used_as: UsedAs`.
  - **`Skill`:** `used_as: Vec<SkillUse>` and `created_ms: Option<u64>`, the skill folder's creation time from `metadata().created()`.
  - **`Inventory`:** `hints: Hints` and `cost_note: Option<&'static str>`.
  - **`ExtRoots`:**
    - `claude_tool_search: Option<String>`, Marshell's own `ENABLE_TOOL_SEARCH`;
    - `qwen_runtime: PathBuf`, `QWEN_RUNTIME_DIR`, else `qwen_dir`;
    - `gemini_dir: PathBuf`, `<home>/.gemini`.

  Every derive these types need is the one their containing type already has (`Debug, Clone, PartialEq, Eq`).
- Produces: `pub fn sanitize_tool_part(name: &str) -> String` in `ext.rs`. It turns every character outside `[A-Za-z0-9_-]` into `_`; Claude Code and Qwen Code both use it.

**Claude Code's rules** (research note, Claude Code):
- **Approval:**
  - User, local-scope and plugin servers: `launch` is Ok.
  - A `.mcp.json` server is approved when `enableAllProjectMcpServers: true`, or its name in `enabledMcpjsonServers`, is in one of:
    - the user settings;
    - the project's `.claude/settings.local.json`;
    - the project's entry in `~/.claude.json`;
    - the project's `.claude/settings.json`, but only when that `~/.claude.json` entry has `hasTrustDialogAccepted: true`.
  - Otherwise `Err(NotApproved)`.
  - A remote entry is `Err(Remote)`.
- **Launch:**
  - `inherit: All`, `expand: Braces`.
  - `set`: `CLAUDE_PROJECT_DIR=<project>` for project and local servers, and `CLAUDE_PLUGIN_ROOT=<installPath>` for plugin servers.
  - `cwd`: the project for project and local servers, the home otherwise.
  - `timeout_secs`: the user settings' `env.MCP_TIMEOUT` (ms, rounded up to seconds), else 30, at most 60.
- **Cost:**
  - Tool search comes from the user settings' `env.ENABLE_TOOL_SEARCH`, else `roots.claude_tool_search`.
    - Unset, empty or `true`: `Names { prefix: "mcp__<s>__", instructions_cap: Some(4096) }`.
    - `false`: `Full { prefix: "mcp__<s>__", desc_cap: Some(4096) }`.
    - `auto`: `FullUnder` with percent 10; `auto:N` with percent N.
  - A server with `alwaysLoad: true` is `Full`.
  - `always_load_meta` is `Some("anthropic/alwaysLoad")`.
  - `<s>` is `sanitize_tool_part` of the display name: the name itself, or `plugin:<plugin>:<server>` for a plugin server, where `<plugin>` is the part of the key before `@`.
- **Usage:**
  - Servers: `used_as: ToolPrefix("mcp__<s>__")`.
  - Skills: `used_as: [Name(<listed name>)]`.
- **Hints:**
  - Server: `To load it only there, add it in that project with claude mcp add -s local, and remove the user-level entry with claude mcp remove <name> -s user.`
  - Skill: `To load it only there, move its folder to <project>\.claude\skills.`

**Qwen Code's rules** (research note, Qwen Code):
- **Approval:**
  - User servers: Ok.
  - A server from `<project>/.mcp.json` or `<project>/.qwen/settings.json` needs `<qwen_dir>/mcpApprovals.json`, under `[<project, lowercased>][<name>]`, with `status == "approved"` and a `hash` equal to the SHA-256 hex of the entry's canonical JSON. Otherwise `Err(NotApproved)`.
  - Before writing the canonical form, read its function in `…\AppData\Local\qwen-code\qwen-code\lib\chunks\chunk-CPBGKA77.js` (read-only). Copy exactly which keys it drops (`scope`, `extensionName`, `description`) and at which depth it sorts keys. Put the chunk's function name in a comment.
  - When the user settings' `security.folderTrust.enabled` is `true`, a project not trusted in `<qwen_dir>/trustedFolders.json` is `NotApproved` for every project server.
- **Launch:**
  - `inherit: All`.
  - `expand`: `Dollar` for `settings.json` entries, `None` for `.mcp.json`.
  - `cwd`: the entry's `cwd`.
  - `timeout_secs`: 30.
- **Cost:**
  - When the user settings' `tools.toolSearch.enabled` isn't `false`: `FirstLines { heading: <name>, cap: 160 }`. Otherwise, or with the server's `alwaysLoadTools: true`: `Full { prefix: "mcp__<s>__", desc_cap: None }`.
  - `only` is `includeTools`, and `except` is `excludeTools`.
- **Usage:**
  - Servers: `used_as: ToolPrefix("mcp__<s>__")`, with `<s> = sanitize_tool_part(name)`.
  - Skills: `[Name(name)]`.
- **Hints:**
  - Server: `To load it only there, move its entry to mcpServers in <project>\.qwen\settings.json.`
  - Skill: `To load it only there, move its folder to <project>\.qwen\skills.`

- [ ] **Step 1: Write the failing tests** in each reader's test module, against fake homes:
  - `claude_mcpjson_approval`, with four `.mcp.json` servers, each a different case:
    - `a` is in `settings.local.json`'s `enabledMcpjsonServers`, so `launch.is_ok()`;
    - `b` is approved only in the committed `.claude/settings.json`, with `hasTrustDialogAccepted: false`, so `Err(NotApproved)`;
    - `c` is the same as `b`, but with `hasTrustDialogAccepted: true`, so it is Ok;
    - `d` is approved by `enableAllProjectMcpServers: true` in the user settings, so it is Ok.
  - `claude_launch_and_cost`:
    - A plugin server gets `set` with `CLAUDE_PLUGIN_ROOT`, `used_as == ToolPrefix("mcp__plugin_plug_srv__")` and the cost `Names`.
    - `alwaysLoad: true` gives `Full`.
    - User settings `env.ENABLE_TOOL_SEARCH = "auto:5"` give `FullUnder { percent: 5, .. }`.
    - `env.MCP_TIMEOUT = "90000"` gives 60.
    - A `url` entry gives `Err(Remote)`.
    - A skill folder's `created_ms` is `Some`.
  - `qwen_approval_hash`:
    - A project server whose `mcpApprovals.json` entry has the right hash is Ok.
    - The same entry with one argument changed is `NotApproved`.
    - `status: "rejected"` is `NotApproved`.
    - `folderTrust.enabled` with no `trustedFolders.json` gives `NotApproved`.
    - A user-level server is Ok in every case.
  - `qwen_launch_and_cost`:
    - `settings.json` gives `Expand::Dollar`, and `.mcp.json` gives `Expand::None`.
    - `includeTools` fills `only`.
    - `tools.toolSearch.enabled: false` gives `Full`.

  Compute the hash vector once with `python -I -c "import hashlib,json;…"` on the canonical form you copied, and write it into the test as a literal.
- [ ] **Step 2:** `cargo test -j 3 -p marshell-core agents::claude::extensions agents::qwen::extensions`. Expected: FAIL (new fields missing).
- [ ] **Step 3: Implement.** Add the types and fields. Update every place `Server`, `Skill`, `Inventory` and `ExtRoots` are built, test helpers in `doctor/ext.rs` and `server/doctor_ext.rs` included: the Codex and Antigravity readers get `launch: Err(NotMeasured::NotApproved)`, `cost: CostRule { listing: Listing::NamesOnly, .. }`, `used_as: UsedAs::Unknown` and `Hints::default()` until Task 2. `Hints::default()` returns the sentence `Move it to that project.` for both.
- [ ] **Step 4:** run the same command, plus `cargo test -j 3 -p marshell-core doctor` and `cargo clippy -j 3 -p marshell-core --all-targets -- -D warnings`. Expected: PASS, with no warnings.
- [ ] **Step 5:** `cargo fmt --all`, then commit `feat(doctor): Claude Code's and Qwen Code's servers say how their CLI starts them, whether it approved them and what they load`.

### Task 2: Codex's and Antigravity's launch rules

**Files:**
- Modify: `crates/core/src/agents/codex/extensions.rs`, `crates/core/src/agents/antigravity/extensions.rs`
- Test: their `#[cfg(test)]` modules

**Interfaces:**
- Consumes: Task 1's types.
- Produces: the same fields, filled in for Codex and Antigravity.

**Codex** (research note, Codex):
- **Approval:**
  - Servers in `~/.codex/config.toml`, and plugin servers (as 7c1 reads them): Ok.
  - A server in `<project>/.codex/config.toml` is Ok only when the user config has `[projects.'<key>'] trust_level = "trusted"` for the project folder or for its git root, the nearest ancestor holding `.git`.
    - Keys match case-insensitively, with `\\?\` stripped and `/` and `\` alike.
    - Otherwise `Err(NotApproved)`.
- **Launch:**
  - `inherit: Only(...)`: the 23 names of `WINDOWS_CORE_ENV_VARS` in the research note, plus the entry's `env_vars`.
  - `env`: the entry's `env` table. `expand`: `None`.
  - `cwd`: the entry's `cwd`, else the project for a project server, else the home.
  - `timeout_secs`: `startup_timeout_sec`, else `startup_timeout_ms` rounded up, else 30, at most 60. A plugin's `.mcp.json` may set `startup_timeout_sec` too.
- **Cost:**
  - A plugin server whose `.mcp.json` entry has `omit_tools_from` containing `"deferred"`: `Full { prefix: "mcp__<name>__", desc_cap: None }`. Every other server: `Namespace { namespace: "mcp__<name>", cap: 250 }`.
  - `only` is `enabled_tools`, and `except` is `disabled_tools`.
- **Usage:**
  - Servers: `used_as: Server(<name>)`.
  - Skills: `used_as: [File(<SKILL.md>)]`.
- **Hints:**
  - Server: `To load it only there, move its [mcp_servers.<name>] table to <project>\.codex\config.toml. Codex reads that file only in a trusted project.`
  - Skill: `To load it only there, move its folder to <project>\.agents\skills.`

**Antigravity** (research note, Antigravity):
- **Approval:**
  - User and plugin servers: Ok.
  - A server in `<workspace>/.agents/mcp_config.json` is Ok only when its workspace is listed in `trustedWorkspaces`. Find which settings file holds that key under `~/.gemini` by reading key names only, and name it in a comment.
  - Otherwise `Err(NotApproved)`.
- **Launch:**
  - `inherit: All`, `expand: Braces`.
  - `cwd`: the entry's `cwd`, else the home.
  - `timeout_secs`: 60.
- **Cost:**
  - `NamesOnly`, with `always_load_meta: Some("antigravity/alwaysLoad")`.
  - `Inventory.cost_note`: `Some("Counted from tool names: Antigravity doesn't document what else it loads.")`.
- **Usage:**
  - Servers: `used_as: Unknown` (decision 16).
  - Skills: `[File(<SKILL.md>)]`.
- **Hints:**
  - Server: `To load it only there, move its entry to <project>\.agents\mcp_config.json.`
  - Skill: `To load it only there, move its folder to <project>\.agents\skills.`

- [ ] **Step 1: Write the failing tests:**
  - `codex_project_trust`:
    - A project server under `[projects.'c:/dev/x'] trust_level = "trusted"`, with the project at `C:\dev\x`, is Ok.
    - The same under `[projects.'\\?\C:\dev\x']` is Ok.
    - A subfolder project trusted through its git root is Ok.
    - An untrusted project is `NotApproved`.
  - `codex_launch_and_cost`:
    - `env_vars = ["FOO"]` makes `Inherit::Only` hold `PATH` and `FOO`.
    - `startup_timeout_ms = 1500` gives 2.
    - A plugin entry with `omit_tools_from = ["deferred"]` and `enabled_tools` gives `Full` and fills `only`.
    - A config server gives `Namespace`.
  - `antigravity_launch_and_cost`:
    - A workspace server outside `trustedWorkspaces` is `NotApproved`, and one inside is Ok.
    - `used_as` is `Unknown`, and `cost_note` is set.
    - A skill's `used_as` is `File`.
- [ ] **Step 2:** `cargo test -j 3 -p marshell-core agents::codex::extensions agents::antigravity::extensions`. Expected: FAIL.
- [ ] **Step 3: Implement** the rules above in the two readers.
- [ ] **Step 4:** run the same command, plus clippy as in Task 1. Expected: PASS, with no warnings.
- [ ] **Step 5:** commit `feat(doctor): Codex's and Antigravity's servers say how their CLI starts them, whether it trusts them and what they load`.

### Task 3: The MCP probe

**Files:**
- Create: `crates/core/src/doctor/probe.rs`, with `pub mod probe;` in `doctor/mod.rs`
- Modify: `crates/core/src/bin/fake-agent.rs` (an `mcp <mode>` command; list it in the file's header comment)
- Test: `crates/core/tests/mcp_probe.rs`

**Interfaces:**
- Produces:

```rust
/// A server ready to start: the program already resolved, the environment complete.
pub struct Ready { pub program: PathBuf, pub args: Vec<String>, pub env: Vec<(String, String)>, pub cwd: Option<PathBuf>, pub timeout: Duration }
pub struct Tool { pub name: String, pub description: String, pub schema_chars: usize, pub meta_true: Vec<String> }
pub struct Listed { pub instructions: String, pub tools: Vec<Tool> }
pub enum Failure { CantStart, Exited(Option<i32>), NoAnswer(u64), NotMcp, Refused }
/// Starts the server, lists its tools and ends its whole process tree, whatever happened.
pub fn probe(ready: &Ready) -> Result<Listed, Failure>
```

All types derive `Debug, Clone, PartialEq, Eq`.

**The protocol** (MCP over stdio, one JSON object per line):
1. Send `initialize` with `id: 1`, `protocolVersion: "2025-06-18"`, `capabilities: {}` and `clientInfo: {"name": "marshell-doctor", "version": env!("CARGO_PKG_VERSION")}`.
2. On its result, keep `instructions` (empty when absent), then send `notifications/initialized`.
3. If the result's `capabilities` has `tools`, send `tools/list` with `id: 2, 3, …`, following `nextCursor` for at most 50 pages. Otherwise the server has no tools.
4. For each tool keep:
   - `name` and `description`;
   - `schema_chars`, the characters of `serde_json::to_string(inputSchema)`;
   - `meta_true`, the `_meta` keys whose value is `true`.

**What the probe tolerates:**
- It skips lines that aren't JSON, and notifications.
- It answers a request from the server (a line with both `method` and `id`) with error `-32601` `"Method not found"`.
- A JSON-RPC `error` to `initialize` or `tools/list` is `Refused`.
- More than 16 MiB read in all is `NotMcp`.
- stdout closing before the answer is `Exited(code)`, or `NotMcp` if a non-JSON line came.
- No answer by the deadline is `NoAnswer(timeout secs)`.
- A spawn error is `CantStart`.

**The process:**
- `env_clear()`, then `ready.env`, with `CREATE_NO_WINDOW`.
- stdin, stdout and stderr are piped, and stderr is drained on a thread and dropped.
- `ProcessTree::adopt` right after spawn.
- stdout is read on a thread that sends lines over a channel, so the deadline holds while a read blocks.
- When done, success or not: `tree.kill(&mut child)`, `child.wait()`.

**`fake-agent mcp <mode>`** answers in a mode chosen per test. Each mode that answers prints an `instructions` of `"Use it."`:
- `ok`: one stdout banner line, `fake mcp ready`, then a request `{"jsonrpc":"2.0","id":"s1","method":"roots/list"}` before answering `initialize`. Two pages of tools: `alpha` (description `"First tool.\nMore."`, `_meta: {"anthropic/alwaysLoad": true}`) and `beta`.
- `notools`: no `tools` capability.
- `silent`: reads stdin and never writes.
- `exit3`: exits with code 3 at once.
- `garbage`: prints `hello` and exits 0.
- `refuse`: answers `initialize` with an error.
- `flood`: prints 20 MiB of non-JSON lines.
- `grandchild`: starts a sleeping `fake-agent sleep`, prints `grandchild:<pid>` on stderr into a file named by the `FAKE_MCP_PIDFILE` variable, then never answers.

- [ ] **Step 1: Write the failing tests** in `tests/mcp_probe.rs`, each with a `Ready` running `fake-agent mcp <mode>`:
  - `ok_lists_tools_over_pages`: `instructions == "Use it."`, the tool names are `["alpha", "beta"]`, alpha's `meta_true == ["anthropic/alwaysLoad"]`, and `schema_chars > 0`.
  - `no_tools_capability_lists_none`.
  - `silent_times_out`: a 2 s timeout gives `NoAnswer(2)` within 4 s.
  - `exit_code_is_reported`: `Exited(Some(3))`.
  - `garbage_is_not_mcp`: `NotMcp`.
  - `refused_initialize`: `Refused`.
  - `flood_is_not_mcp_and_bounded`: `NotMcp` within the timeout.
  - `missing_program_cant_start`: `CantStart`.
  - `timeout_ends_the_grandchild` (Windows only): after `NoAnswer`, the pid read from the pidfile is gone within 3 s. Copy the `gone(pid)` helper of `crates/host/tests/core_killed.rs`.
  - `env_is_exactly_ready_env`: run with `env` holding only `PATH` and `SYSTEMROOT` plus `FAKE_MCP_ECHO=1`. In that mode `ok` returns the names of its environment in `instructions`, and they must hold exactly the given names, case-insensitively, plus any that Windows adds itself. List those in the assertion.
- [ ] **Step 2:** `cargo test -j 3 -p marshell-core --test mcp_probe`. Expected: FAIL (no module).
- [ ] **Step 3: Implement** `probe.rs` and the fake modes.
- [ ] **Step 4:** run the same command three times in a row. Expected: PASS each time.
- [ ] **Step 5:** commit `feat(doctor): a probe that starts one MCP server, lists its tools and ends its whole process tree`.

### Task 4: Measurement items

**Files:**
- Create: `crates/core/src/doctor/measure.rs`, with `pub mod measure;`
- Modify: `crates/core/src/doctor/ext.rs`. It gets `Signals`, and `build` composes the 7c1, measure and usage items. The usage call is a stub that returns nothing until Task 6.
- Test: `measure.rs` and `ext.rs` test modules

**Interfaces:**
- Consumes: Task 1's `Launch`, `CostRule` and `Listing`; Task 3's `Listed`, `Tool` and `Failure`.
- Produces:

```rust
// measure.rs
pub struct Measured { pub agent: &'static str, pub key: String, pub fingerprint: String, pub outcome: Result<Listed, Failure> }
pub struct Measurements { pub at_ms: u64, pub servers: Vec<Measured> }
/// The first 16 hex digits of `hash_bytes` over the launch's `Debug` text. A launch that changed makes the old
/// measurement stale.
pub fn fingerprint(launch: &Launch) -> String
/// The characters this rule loads up front for these tools. `FullUnder` is decided by the caller, which passes `full`.
pub fn server_chars(rule: &CostRule, listed: &Listed, full: bool) -> usize
pub(crate) fn items(inv: &Inventory, measured: Option<&Measurements>, out: &mut Vec<(DoctorItem, Option<Switch>)>)
// ext.rs
pub struct Signals<'a> { pub usage: Option<&'a crate::doctor::usage::UsageStore>, pub measured: Option<&'a Measurements>, pub projects: &'a [PathBuf], pub now_ms: u64 }
impl Default for Signals<'_> { /* None, None, &[], 0 */ }
pub fn items(inv: &[Inventory], sig: &Signals) -> Vec<DoctorItem>
pub fn switch_of(inv: &[Inventory], sig: &Signals, id: &str) -> Option<Switch>
```

Task 6 creates `doctor::usage::UsageStore`. Until then, declare `pub struct UsageStore;` in a new `doctor/usage.rs` so this task compiles; Task 6 replaces it.

**Counting** (`server_chars`):
- A tool counts when it is in `only` (if set) and not in `except`.
- A counted tool whose `meta_true` holds `always_load_meta` counts in full, with the listing's prefix, if it has one.
- Characters are `chars().count()`. A cut takes the first `cap` characters, and a "first line" is the text up to the first `\n`.

**Items** (`items(inv, measured, out)`), only when `measured` is `Some`:
- **A distro inventory** with at least one server that is on and not bundled gets one Info item:
  - id `mcpcost.<agent>@<distro>`;
  - title `<CLI>'s MCP servers in <distro> weren't measured`;
  - detail `Not measured: in <distro>. Measuring runs on Windows only.`
- **A Windows inventory:** take each server that is on and not bundled. Skip those whose transport is `Nothing`, or `Stdio` with `Missing` or `NotOnPath`, since 7c1 already reports them. For the rest:
  - **`Remote`:** count it as "remote".
  - **`NotApproved`:** count it as "not approved".
  - **A `Measured` with the same `key`** (7c1's `item_key`) and the same `fingerprint`:
    - `Ok`: add its cost.
    - `Err(failure)`: one Broken item, with:
      - id `mcpfail.<agent>.<key>`;
      - title `<CLI>'s MCP server <name> didn't start`;
      - detail `When Marshell started it to measure it, <why>. A CLI started from your shell can have an environment Marshell's doesn't.`;
      - `unchecked: true`;
      - `fixability()` for the fix note or `cannot_fix`.

      `<why>` is:
      - `its command couldn't start`;
      - `it exited with code <n> before answering`, or `it ended before answering`;
      - `it didn't answer within <n> s`;
      - `what it printed wasn't MCP`;
      - `it refused to start a session`.
  - **No match:** count it as "added since the measurement".
- **`FullUnder`:** sum every `FullUnder` server's full characters. If `tokens(sum) < percent * 2000`, count each in full; otherwise by `Names`.
- **The cost line**, id `mcpcost.<agent>`, Info, once per inventory with at least one counted or skipped server:
  - **At least one measured:**
    - title `<CLI>'s MCP servers add about <t> tokens to each session`;
    - detail `<k> measured. The largest: <a> (<ta>), <b> (<tb>), <c> (<tc>).`, the top three by tokens, ties by name.
  - **None measured:**
    - title `<CLI>'s MCP servers weren't measured`;
    - no first sentence.
  - The detail then adds ` Not measured: <parts>.` when anything was skipped. The parts are joined with `, `, in this order:
    - `<n> remote server(s)`;
    - `<n> that <CLI> hasn't approved`;
    - `<n> added since the measurement`.
  - Then ` <cost_note>` when set.
- **`ext.rs` `build`:**
  - Order per inventory: 7c1's items, then `measure::items`, then usage items (Task 6).
  - `switch_of(inv, sig, id)` searches the composed list.
  - Every 7c1 caller of `items`, `switch_of` and the tests passes `&Signals::default()` until Tasks 5 and 9.

- [ ] **Step 1: Write the failing tests:**
  - `server_chars_by_listing`, one listing at a time over a `Listed` with `instructions: "Line one\nLine two"` and tools `alpha` (description `"First.\nSecond."`, `schema_chars` 10) and `beta` (empty description, `schema_chars` 2). It asserts the exact sums:
    - `Names { prefix: "mcp__s__", instructions_cap: None }` gives `"mcp__s__alpha".len() + "mcp__s__beta".len() + 17`;
    - `Namespace { namespace: "mcp__s", cap: 4 }` gives `6 + 4`;
    - `FirstLines { heading: "s", cap: 3 }` gives `1 + (5 + 3) + (4 + 0)`;
    - `Full { prefix: "", desc_cap: Some(5) }` gives `(5 + 5 + 10) + (4 + 0 + 2)`;
    - `NamesOnly` gives `9`.
  - `only_except_and_always_load`: `only: Some(["alpha"])` drops beta. A `meta_true` match counts alpha in full under `Names`.
  - `cost_line_copy`: the exact title and detail for:
    - three measured servers, one remote and one not approved;
    - a Windows inventory with only remote servers;
    - the singular forms (`1 remote server`).
  - `stale_measurement_counts_as_added_since`: a changed launch fingerprint gives `1 added since the measurement` and no `mcpfail`.
  - `failure_items`: one per `Failure` kind, with the exact detail, `unchecked == true`, and `fixable` following the switch.
  - `no_measurement_no_items`: `measured: None` gives no `mcpcost` and no `mcpfail` item.
  - `distro_line`: the exact copy.
  - `full_under_threshold`: two `FullUnder { percent: 1 }` servers totalling under 2,000 tokens count in full, and over it by names.
  - In `ext.rs`, `composed_switch_of_finds_mcpfail`: `switch_of` returns the server's switch for an `mcpfail.` id.
- [ ] **Step 2:** `cargo test -j 3 -p marshell-core doctor::measure doctor::ext`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** run the same command, plus clippy. Expected: PASS.
- [ ] **Step 5:** commit `feat(doctor): measurements become each CLI's MCP cost line and a Broken item for each server that didn't start`.

### Task 5: "Measure MCP servers", end to end in the core (use `effort: high`)

**Files:**
- Modify: `crates/protocol/src/doctor.rs`. `DoctorSummary` gets `#[serde(default)] pub measuring: bool`. `DoctorReport` gets `measured_at_ms: Option<u64>`, with the same attributes as `cli_checked_at_ms`. Then run `cargo test -j 3 -p marshell-protocol` to regenerate `src/generated/`.
- Modify: `crates/core/src/doctor/state.rs`, which gets `pub measure: Mutex<MeasureRun>`, with `pub struct MeasureRun { pub running: bool, pub results: Option<Measurements> }`.
- Create: `crates/core/src/server/doctor_measure.rs`
- Modify:
  - `crates/core/src/server/doctor.rs`: the route handler, and `Signals` in `run_once` and `fix_of`;
  - `crates/core/src/server/mod.rs`: `.route("/v1/doctor/measure", post(doctor::measure_run))`, and an `AppState` probe swap `with_probe(fn(&Ready) -> Result<Listed, Failure>)` whose default is `probe::probe`.
- Test: `server/doctor_measure.rs` tests, and the bearer test where 7b's `/v1/doctor/cli` one is

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces:
  - `pub async fn measure_run(State(st)) -> (StatusCode, Json<DoctorSummary>)`, which answers 202, as `cli_run` does.
  - `pub(crate) fn request_measure(st: &AppState)`.
  - `pub(crate) fn ready_of(launch: &Launch, base: &[(String, String)]) -> Result<Ready, Failure>`.
  - `fn base_env(st: &AppState) -> Vec<(String, String)>`.

**Behaviour:**
- **`request_measure`** follows `request_cli_run` exactly:
  - The flag and `summary.measuring` are set under one lock.
  - The run is `spawn_blocking(measure_all)`.
  - It stores the results, calls `request_run`, and only then clears `measuring`.
  - A second request during a run joins it.
- **`measure_all(st)`:**
  - Take `facts_of(st).ext`, Windows inventories only.
  - Pick each server that is on, not bundled, `Transport::Stdio` with state `Found` or `NotChecked`, and `launch: Ok`.
  - Probe at most 4 at once, on scoped threads.
  - Each gives `Measured { agent, key: item_key(..), fingerprint, outcome }`, with `outcome` from `ready_of` and then the probe swap.
- **`base_env(st)`:** Marshell's own environment, with `PATH` replaced by `agent_finder(&[]).path_in_use()` when that is `Some`.
- **`ready_of`:**
  1. Start from `base` filtered by `inherit` (names compared case-insensitively).
  2. Add `set`, then `env`.
  3. Expand per `expand` over command, args, env values and cwd, using the environment built so far. `Braces` takes `${VAR}` and `${VAR:-default}`, and `Dollar` takes `$VAR` and `${VAR}`. An unknown name stays as written.
  4. Resolve a command with no `\` or `/` through `command_path_in` on that environment's `PATH`; not found gives `CantStart`. A relative path is joined to `cwd`.
  5. `timeout` is `timeout_secs`.
- **`run_once`:** clone `measure.results` under the lock, drop the lock, and pass it in `Signals.measured`. The report's `measured_at_ms` is its `at_ms`.
- **`fix_of`:**
  - It builds `now` with the same `Signals` as `run_once`.
  - Its dispatch arm becomes `Some("mcp" | "skill" | "mcpfail" | "mcpunused" | "skillunused")`, through `ext::switch_of(&gathered.ext, &signals, &item.id)`.
  - It drops a switch equal to one already taken, so two items for one server give one edit.

- [ ] **Step 1: Write the failing tests:**
  - `ready_of_builds_the_env_and_expands`:
    - With `Inherit::Only(["PATH"])` and a base holding `PATH` and `SECRET`, `env` has no `SECRET`.
    - With `Braces`, `${X:-d}` gives `d`, and `${MISSING}` stays as written.
    - With `Dollar`, `$HOME_X` expands.
    - A bare command missing from `PATH` gives `CantStart`.
  - `measure_picks_only_approved_local_windows_servers`: a fake home with Claude Code servers that are on, off, remote, unapproved and command-missing, plus a running-distro inventory. The probe swap records the commands it gets; only the approved one that is on is probed.
  - `measure_results_ride_along_and_fail_unchecked`:
    - A swap returning `Exited(Some(1))` for one server and `Ok` for another.
    - After `request_measure` and the run that follows, the report has `mcpfail.claude.<key>` with `unchecked: true`, the `mcpcost.claude` line and `measured_at_ms`.
    - `summary.measuring` is `false` only after that report is stored.
  - `measure_joins_a_running_measure`: two requests while a swap blocks on a channel call the swap once per server.
  - `fix_picks_unused_and_failed_once`: a server with both an `mcpfail` and an `mcpunused` id. Build the usage half with a `Signals` holding a seeded store once Task 6 exists, and mark it `#[ignore]` until Task 9 removes that. The plan's edits hold one entry for the server.
  - `measure_route_refuses_without_bearer`.
- [ ] **Step 2:** `cargo test -j 3 -p marshell-core server::doctor_measure server::doctor`. Expected: FAIL.
- [ ] **Step 3: Implement.** Then run `cargo test -j 3 -p marshell-protocol` and `pnpm check:generated`.
- [ ] **Step 4:** run the same tests, `pnpm check:generated`, and clippy for the workspace. Expected: PASS, with `check:generated` clean.
- [ ] **Step 5:** commit the code and `src/generated/` together: `feat(doctor): Measure MCP servers starts each approved local server once and reports the result in every check`.

### Task 6: The usage engine

**Files:**
- Modify: `crates/core/src/doctor/usage.rs`, replacing Task 4's placeholder
- Modify: `crates/core/src/doctor/ext.rs` (`build` calls `usage::items`)
- Test: `usage.rs` test module

**Interfaces:**
- Produces (Tasks 7–9 use these):

```rust
pub const WINDOW_MS: u64 = 30 * 24 * 60 * 60 * 1000;
pub enum UseKey { Tool(String), Server(String), SkillName(String), SkillFile(String) }
pub enum EventKind { Used, Available }
pub struct Event { pub kind: EventKind, pub key: UseKey, pub at_ms: u64, pub cwd: Option<String> }
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Cursor { pub len: u64, pub read_to: u64, pub first_ms: Option<u64>, pub cwd: Option<String> }
/// What one scan of one CLI found: the events in new records, every history file present with its cursor, and
/// whether the history could be read at all.
pub struct Scan { pub events: Vec<Event>, pub cursors: BTreeMap<String, Cursor>, pub unreadable: Option<String> }
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Use { pub last_ms: u64, pub places: BTreeMap<String, u64> }
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct AgentStore { pub oldest_ms: Option<u64>, pub scanned_ms: Option<u64>, pub cursors: BTreeMap<String, Cursor>, pub used: BTreeMap<String, Use>, pub available: BTreeMap<String, u64>, pub unreadable: Option<String> }
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct UsageStore { pub agents: BTreeMap<String, AgentStore>, pub first_sight: BTreeMap<String, u64> }
impl crate::store::Versioned for UsageStore { const VERSION: u32 = 1; }

/// Lowercase, `\` to `/`, `\\?\` and a trailing `/` removed.
pub fn norm_path(p: &str) -> String
/// The absolute `SKILL.md` paths in a command or argument text, normalised.
pub fn skill_md_paths(text: &str) -> Vec<String>
/// The new complete lines of `file` after `cursor`. A file shorter than `cursor.read_to` is read from the start.
/// Returns the lines and the new cursor (`len`, `read_to` at the end of the last whole line; `first_ms` and `cwd` kept).
pub fn new_lines(file: &Path, cursor: Option<&Cursor>) -> std::io::Result<(Vec<String>, Cursor)>
pub fn merge(store: &mut AgentStore, scan: Scan, now_ms: u64)
/// Joins two stores written from the same snapshot by runs that overlapped.
pub fn absorb(into: &mut UsageStore, other: &UsageStore)
pub(crate) fn items(inv: &Inventory, store: Option<&UsageStore>, broken: &dyn Fn(&str) -> bool, projects: &[PathBuf], now_ms: u64, out: &mut Vec<(DoctorItem, Option<Switch>)>)
```

- A key's text form in `used` and `available` is `tool:<name>`, `server:<name>`, `skill:<name lowercased>` or `file:<norm_path>`.
- A place is `norm_path(cwd)`, or `""` when unknown.
- `first_sight` keys are `<agent>:<item key>`.

**`merge`:**
- Events:
  - A `Used` event raises the key's `last_ms`, and the place's own entry, to the max.
  - An `Available` event lowers `available[key]` to the min.
  - An event dated after `now_ms + 1 day` is dropped.
- **Coverage:**
  - `disk = min(first_ms)` over `scan.cursors`.
  - If `store.scanned_ms` is `Some(s)` and `disk > s`, history may have been lost unseen, so `oldest_ms = disk`.
  - Otherwise `oldest_ms = min(oldest_ms, disk)`.
- Then:
  - `scanned_ms = now_ms`;
  - `cursors = scan.cursors`;
  - `unreadable = scan.unreadable`;
  - every place older than `now_ms - WINDOW_MS` is pruned.

**`absorb`:**
- Per agent, the store with the later `scanned_ms` gives `oldest_ms`, `cursors` and `unreadable`.
- `used` takes the max per key and place, and `available` the min.
- `first_sight` takes the min.

**Matching** an inventory item to keys:
- **A server:**
  - `ToolPrefix(p)` matches every `tool:` key that starts with `p`.
  - `Server(n)` matches `server:n`.
  - `Unknown` is never judged.
- **A skill:**
  - `Name(n)` matches `skill:` keys equal to `n` lowercased.
  - It also matches a `skill:` key of the form `<x>:<b>` whose `<x>` holds `/` or `\`, when `<b>` equals the bare name: the part of `n` after its last `:`, lowercased.
  - `File(f)` matches `file:<norm_path(f)>`.
  - Each match adds its uses, and its availability.

**`items`, for a Windows inventory that was found:**
- **Unreadable:** when `store.agents[agent].unreadable` is `Some(why)`, add one Info item, id `usage.<agent>`:
  - title `<CLI>'s history can't be read`;
  - detail `<why> So its servers and skills aren't called unused.`
  - Nothing else follows.
- **Judged items** are:
  - servers that are on, not bundled, not `Unknown`, without `Err(NotApproved)`, and with no Broken item (`broken(key)` is false);
  - skills that are on, not bundled, not hidden, in `SkillState::Ok`, and with no Broken item.
- **Not covered yet:** `oldest_ms` is `None`, or after `now_ms - WINDOW_MS`. If there is a judged item, add one Info item, id `usage.<agent>`:
  - with `d = (now - oldest) / day`, at least 1: title `<CLI>'s history covers <d> day(s)`, and detail `Servers and skills are called unused once it covers 30. Marshell keeps what it reads, so this grows each day it runs.`
  - with no history: title `<CLI> has no history yet`, and detail `Servers and skills are called unused once Marshell has read 30 days of it.`
- **Antigravity**, with at least one server that is on and not bundled, gets an Info item, id `usage.antigravity.mcp`:
  - title `Marshell can't yet tell when Antigravity uses an MCP server`;
  - detail `No Antigravity MCP call has been seen in a transcript, so its servers are never called unused.`
- **When covered**, for each judged item:
  - **`since`** is the min of: the skill's `created_ms`, the item's `available` keys, and `first_sight[<agent>:<key>]`.
  - **`last`** is the max `last_ms` of its `used` keys.
  - **Unused:** no `last` at or after `now - WINDOW_MS`, and `since` at or before `now - WINDOW_MS`. Add a Waste item:
    - id `mcpunused.<agent>.<key>` or `skillunused.<agent>.<key>`;
    - title `<CLI>'s MCP server <name> wasn't used in 30 days` / `<CLI>'s skill <name> wasn't used in 30 days`;
    - detail `It still starts with every <CLI> session.` / `Its name and description still load into every session.`;
    - then `fixability()`, as 7c1 does.
  - **Broad scope:** the item's `Origin` is `User`, and it was used within the window. Map each place used within the window to the longest Marshell project that holds it (`projects`, compared by `norm_path` prefix at a `/` boundary), or else to itself. When all of them map to one project and none is `""`, add an Info item:
    - id `mcpbroad.<agent>.<key>` or `skillbroad.<agent>.<key>`;
    - title `<CLI>'s MCP server <name> is used in one project only` / `<CLI>'s skill <name> is used in one project only`;
    - detail `In the last 30 days it was used only in <project>. <hint>`, with the inventory's `hints`;
    - `cannot_fix: "The Doctor doesn't move it, because a project's files are shared with its team."`

- [ ] **Step 1: Write the failing tests:**
  - `norm_and_skill_paths`. `skill_md_paths` finds `c:/users/home office/.agents/skills/x/skill.md` in each of:
    - `"pwsh.exe" -Command "Get-Content 'C:\Users\Home Office\.agents\skills\x\SKILL.md'"`;
    - `Get-Content -LiteralPath 'C:/Users/Home Office/.agents/skills/x/SKILL.md'`;
    - the argv list text `["cat","C:\\Users\\Home Office\\.agents\\skills\\x\\SKILL.md"]`, after JSON unescaping by the caller.

    It finds nothing in `see SKILL.md for details`.
  - `new_lines_is_incremental`:
    - Write two lines and a partial third. The first call returns two lines.
    - Append the rest. The second call returns the third.
    - Truncate the file to one shorter line. The call reads from the start.
  - `coverage_grows_and_resets_after_a_gap`:
    - A first merge, with files from day −28, gives `oldest` at −28.
    - Two days later, with the same files and day −28 now deleted (disk oldest −27, but `scanned_ms` at −2): `oldest` stays at −28 and is covered.
    - A merge where disk oldest is after `scanned_ms` resets `oldest` to disk oldest.
  - `future_events_are_dropped`.
  - `unused_needs_coverage_and_age`:
    - covered, no use, `since` at −40: unused;
    - `since` at −10: not unused;
    - not covered: a `usage.claude` Info item with `covers 28 days`, and no unused.
  - `uses_count_for_every_match`:
    - A use `skill:.claude/worktrees/x:marshell-ship` keeps a skill named `marshell-ship` used.
    - `skill:SUPERPOWERS:writing-plans` keeps `superpowers:writing-plans` used.
    - `tool:mcp__plugin_plug_srv__x` keeps the plugin server used.
    - Two servers whose prefixes are equal are both used.
  - `broken_items_get_no_unused`.
  - `broad_scope`:
    - A user skill used at `c:/dev/airport/.claude/worktrees/a` and `c:/dev/airport`, with project `C:\dev\Airport`, gives `skillbroad` naming `C:\dev\Airport` and the hint.
    - With one use at place `""`, there is no item.
    - A project-origin skill gets no item.
  - `unreadable_item`: the exact copy, and no unused item.
  - `antigravity_mcp_line`.
  - `absorb_is_commutative` for `used`, `available` and `first_sight`.
- [ ] **Step 2:** `cargo test -j 3 -p marshell-core doctor::usage`. Expected: FAIL.
- [ ] **Step 3: Implement**, and add the call in `ext::build`. `broken` is true for a key that has an `mcp.`, `skill.` or `mcpfail.` item in this inventory's list so far.
- [ ] **Step 4:** run the same command, plus `doctor::ext`, plus clippy. Expected: PASS.
- [ ] **Step 5:** commit `feat(doctor): a usage store that remembers what each CLI used, and the unused and one-project items it gives`.

### Task 7: Claude Code's and Qwen Code's history readers

**Files:**
- Create: `crates/core/src/agents/claude/usage.rs` and `crates/core/src/agents/qwen/usage.rs`, with `pub mod usage;` in each adapter's `mod.rs`
- Test: their test modules, with fixture lines written into a fake home

**Interfaces:**
- Consumes: Task 6's `Scan`, `Event`, `UseKey`, `Cursor`, `new_lines`, `norm_path` and `AgentStore`.
- Produces: `pub fn scan(roots: &ExtRoots, store: &AgentStore) -> Scan` in each module. The cursor map is keyed by the file's `display()` text, and `first_ms` is set from the first record that has a time.

**Claude Code** (research note, Claude Code):
- **Files:** every name ending in `.jsonl` in `<claude_dir>/projects/*/` and in `<claude_dir>/projects/*/*/subagents/`.
- **Parsing:** a line is parsed only when it contains one of `"tool_use"`, `<command-`, `"deferred_tools_delta"` or `"skill_listing"`, or while the file's `first_ms` is still unknown. Time is `rfc3339_ms(timestamp)`, and place is `cwd`.
- **Events:**
  - **Assistant records:** each `message.content[]` item with `type == "tool_use"`:
    - a `name` starting with `mcp__` is `Used Tool(name)`;
    - `name == "Skill"` is `Used SkillName(input.skill)`.
  - **User records** whose `message.content` is a string starting with `<command-`: the text inside `<command-name>/…</command-name>` is `Used SkillName`.
  - **Attachment records:**
    - `deferred_tools_delta.addedNames` starting with `mcp__` are `Available Tool`;
    - `skill_listing.names` are `Available SkillName`.
- **`~/.claude.json`:** each `skillUsage.<name>.lastUsedAt` is `Used SkillName(name)` at that time, with no place. It is read whole on each scan.

**Qwen Code** (research note, Qwen Code):
- **Files:** `<qwen_runtime>/projects/*/chats/*.jsonl`, skipping `*.ledger.jsonl`.
- **Events:** time from `timestamp`, place from `cwd`.
  - **`message.parts[].functionCall`:**
    - a `name` starting with `mcp__` is `Used Tool(name)`;
    - `name == "skill"` is `Used SkillName(args.skill)`;
    - any call whose `args.name` is a string starting with `mcp__` is `Used Tool(args.name)`.
  - **A `user` record** whose first text part starts with `/`: its first word, without the `/`, is `Used SkillName`.

- [ ] **Step 1: Write the failing tests:**
  - `claude_events`: a main file and a subagent file whose lines cover each case above. The scan gives the exact events, place `c:/dev/x`, `first_ms` from the first timed record, and no event from:
    - a `<command-name>` inside a tool result array;
    - an `mcp__` name inside `api-request-blob`;
    - a `"tool_use"` mention inside a text block.
  - `claude_incremental`: a second scan with the returned cursors and one appended line gives only that line's event.
  - `claude_skill_usage_json`: `skillUsage` gives a used event with place `None`.
  - `qwen_events`: each case above. A `.ledger.jsonl` file is ignored.
- [ ] **Step 2:** `cargo test -j 3 -p marshell-core agents::claude::usage agents::qwen::usage`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** run the same command, plus clippy. Expected: PASS.
- [ ] **Step 5:** commit `feat(doctor): Claude Code's and Qwen Code's history readers find MCP calls and skill uses incrementally`.

### Task 8: Codex's and Antigravity's history readers (use `effort: high`)

**Files:**
- Create: `crates/core/src/agents/codex/usage.rs` and `crates/core/src/agents/antigravity/usage.rs`
- Test: their test modules. The Codex tests build the two SQLite files with `rusqlite` in the fake home.

**Interfaces:** the same `scan(roots, store) -> Scan` as Task 7.

**Codex** (research note, Codex):
- **The database:**
  - Open `<codex_home>/thread_history_1.sqlite` and `<codex_home>/state_5.sqlite` with `OpenFlags::SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`, and set `busy_timeout` to 2 s.
  - Check the schema first:
    - `thread_items` must have `thread_id`, `created_at_ms`, `item_type` and `item_json`;
    - `threads` must have `id` and `cwd`.

    Otherwise, or on any SQLite error while `thread_history_1.sqlite` exists, the scan's `unreadable` is `Some("Its history database isn't in a form Marshell reads.")`, and its events are dropped.
  - Read `threads` into an id → cwd map, with `\\?\` stripped.
  - Query `thread_items WHERE created_at_ms >= ?cursor AND item_type IN ('mcpToolCall','commandExecution','userMessage')`.
  - The cursor key is `sqlite:<path>`. Its `read_to` is the max `created_at_ms` read, and its `first_ms` is `MIN(created_at_ms)`.
- **Database events:**
  - `mcpToolCall` is `Used Server(item_json.server)`.
  - `commandExecution` gives `Used SkillFile` for each path in `skill_md_paths(item_json.command)`. The output is never read.
  - `userMessage` whose JSON holds `<skill>` gives `Used SkillFile` for each `<path>…</path>`.
- **Rollouts:** every `rollout-*.jsonl` under `<codex_home>/sessions/`; `.zst` files are skipped.
  - `session_meta` and `turn_context` set the cursor's `cwd`.
  - An `event_msg` whose payload item has `type == "McpToolCall"` is `Used Server(server)`.
  - Its `CommandExecution` items, joining the `command` argv list, give `Used SkillFile`.
  - So do `response_item` items of type `custom_tool_call` or `function_call`, from the text of their `input` or `arguments`.
  - `response_item` messages are never searched, because `<skills_instructions>` lists every skill.
- **When there is no database**, rollouts alone are read.

**Antigravity** (research note, Antigravity):
- **Files:** `<gemini_dir>/antigravity-cli/brain/*/.system_generated/logs/transcript_full.jsonl`, else `transcript.jsonl` in the same folder.
- **Place:** a map from `conversationId` to `workspace`, read from `<gemini_dir>/antigravity-cli/history.jsonl`. The conversation id is the brain folder's name, and an unknown one gives no place.
- **Events:** time from `created_at`. A `tool_calls[]` entry with `name == "view_file"` and an `args.AbsolutePath` ending in `SKILL.md` (any case) is `Used SkillFile(norm_path)`.

- [ ] **Step 1: Write the failing tests:**
  - `codex_db_events`:
    - Rows of each kind, with a commandExecution whose `aggregatedOutput` alone holds `SKILL.md`, which gives no event.
    - A `\\?\C:\dev\x` cwd gives place `c:/dev/x`.
    - A second scan with the returned cursor gives no repeated events beyond rows at the cursor's own millisecond.
  - `codex_db_wrong_schema_is_unreadable`: a `thread_items` table without `item_json` gives `unreadable` set and no events.
  - `codex_rollout_events`: a rollout whose `<skills_instructions>` message lists a `SKILL.md` gives no event from it. Its `CommandExecution` and `McpToolCall` lines do give events.
  - `antigravity_events`: a `view_file` of a `SKILL.md` gives a `SkillFile` event placed through `history.jsonl`. A `view_file` of another file gives nothing.
- [ ] **Step 2:** `cargo test -j 3 -p marshell-core agents::codex::usage agents::antigravity::usage`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** run the same command, plus clippy. Expected: PASS.
- [ ] **Step 5:** commit `feat(doctor): Codex's and Antigravity's history readers find MCP calls and skill reads, Codex's from its thread database read-only`.

### Task 9: The usage scan in every check

**Files:**
- Create: `crates/core/src/server/doctor_usage.rs`
- Modify:
  - `crates/core/src/paths.rs`: `pub fn doctor_usage(home) -> PathBuf`, which is `<home>/doctor/usage.json`;
  - `crates/core/src/doctor/state.rs`: `pub usage: Mutex<Option<UsageStore>>`;
  - `crates/core/src/server/doctor.rs`: `Gathered` gets `usage: UsageStore` and `projects: Vec<PathBuf>`, filled in `facts_of`, and `run_once` and `fix_of` pass them in `Signals`;
  - `crates/core/src/server/doctor_ext.rs`: `ExtRoots`' new fields are filled in `windows_roots`, as Task 1 started.
- Test: `server/doctor_usage.rs` tests

**Interfaces:**
- Consumes: Tasks 6–8.
- Produces: `pub(crate) fn scan_all(st: &AppState, inv: &[Inventory], roots: Option<&ExtRoots>, now_ms: u64) -> UsageStore`.

**`scan_all`:**
1. Lock `doctor.usage`. If it is `None`, load it with `store::read(&paths::doctor_usage(home))`; an error or a missing file gives the default, with a `tracing::warn!` for an error. Clone it and drop the lock.
2. For each Windows inventory that was found, run its CLI's `scan(roots, &snapshot.agents[agent])` and `merge` the result into the clone.
3. For each item of each Windows inventory (servers and skills), insert `first_sight["<agent>:<key>"]` at `now_ms` if absent.
4. Lock again, `absorb` the clone into the stored value, clone the result and drop the lock.
5. Write it with `store::write`, and log a failure with `tracing::warn!`.
6. Return the result.

With no `roots` (unit tests that name no home), nothing is scanned or written.

- [ ] **Step 1: Write the failing tests**, with `AppState::with_ext_home` and a fake home holding a Claude Code transcript and a user skill:
  - `first_run_writes_usage_json_and_second_reads_only_new_lines`: the file exists after the first run. The second run's `Scan` for that file reads zero lines; check this through the cursor's `read_to`, which equals the file's length.
  - `corrupt_usage_json_starts_fresh`: garbage in the file gives a default store, and the run still ends with a report.
  - `newer_usage_json_is_left_alone`: a file with `"version": 99` gives a default store in memory and a report. The file's bytes are unchanged after the run, because `store::write` refuses to replace a newer version.
  - `unused_item_reaches_the_report_and_fix`:
    - A Claude Code `.mcp.json` server approved in `settings.local.json`.
    - A transcript whose first record is 40 days old, and no call to the server.
    - `first_sight` seeded at −40 days through a pre-written `usage.json`.
    - The report then has `mcpunused.claude.<key>` in Waste, and `fix_of` for it gives the `disabledMcpjsonServers` edit.
    - Remove the `#[ignore]` from Task 5's `fix_picks_unused_and_failed_once`.
  - `two_runs_at_once_lose_nothing`: two `scan_all` calls on threads, one fake home, different new lines in two files. The stored value holds both lines' events.
- [ ] **Step 2:** `cargo test -j 3 -p marshell-core server::doctor_usage server::doctor`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** run the same command, plus `cargo test -j 3 -p marshell-core doctor` and clippy. Expected: PASS.
- [ ] **Step 5:** commit `feat(doctor): every check scans each CLI's new history and keeps what it found in usage.json`.

### Task 10: The Measure button and line

**Files:**
- Modify: `src/lib/api/doctor.ts`: `export const measureMcpServers = (ep: Endpoint) => api<DoctorSummary>(ep, "POST", "/v1/doctor/measure");`
- Create: `src/features/doctor/measure.ts` (`measureLine`, `measureDoneAnnouncement`) and `src/features/doctor/useClickRun.ts`
- Modify:
  - `src/features/doctor/DoctorPage.tsx`: props `measuring: boolean` and `onMeasure: () => void`;
  - `src/features/doctor/LiveDoctor.tsx`;
  - `src/features/doctor/Doctor.stories.tsx` and `src/features/doctor/fixtures.ts`.
- Test: `measure.test.ts`, `DoctorPage.test.tsx`, `cliDoctors.test.ts` (unchanged and still green)

**Interfaces:**
- Consumes: `DoctorSummary.measuring` and `DoctorReport.measured_at_ms` (Task 5, generated).
- Produces:
  - `measureLine(measuredAtMs: number | undefined, nowMs: number): string`;
  - `measureDoneAnnouncement(report: DoctorReport): string`;
  - `useClickRun(opts: { endpoint, report, running: boolean, at: (r: DoctorReport) => number | undefined, start: (ep) => Promise<DoctorSummary>, isRunning: (s: DoctorSummary) => boolean, load: () => Promise<void>, announce, done: (r: DoctorReport) => string, failed: (message: string) => string }): { pending: boolean; run: () => Promise<void> }`.

  `useClickRun` is LiveDoctor's CLI-doctors logic (lines 95–140 on `d7342ef`) moved as is. The CLI doctors and Measure both use it.

**Copy:**
- **Button:** `Measure MCP servers`. While running, it reads `Measuring MCP servers…`, with `aria-disabled="true"` and no handler, as "Run CLI doctors" does. It sits after "Run CLI doctors" and before "Run again", and is hidden while fixing.
- **`measureLine`:**
  - `MCP servers haven't been measured. Measuring starts each local server for a moment.`;
  - `MCP servers were measured <n> s ago.`, or `<n> min ago` from 60 s, using `cliLine`'s rounding.

  It renders in a second `<p className="doctor__cli">` after the CLI doctors' line.
- **`measureDoneAnnouncement`:** `n` counts the items whose id starts with `mcpfail.`. It says `No MCP server failed to start.`, `1 MCP server didn't start.` or `<n> MCP servers didn't start.`
- **A failed request** announces `Couldn't measure the MCP servers: <message>`.

- [ ] **Step 1: Write the failing tests:**
  - `measure.test.ts`: the three line forms, and the three announcement forms.
  - `DoctorPage.test.tsx`:
    - the order `>Run CLI doctors<` … `>Measure MCP servers<` … `>Run again<`;
    - `measuring: true` shows `Measuring MCP servers…` with `aria-disabled="true"`;
    - while fixing, neither button shows;
    - the measure line shows below the CLI line.
- [ ] **Step 2:** `pnpm test src/features/doctor`. Expected: FAIL.
- [ ] **Step 3: Implement.** Add stories `MeasureRan` (with a fixture of an `mcpcost.claude` Info item and one `mcpfail.claude.*` Broken item, unchecked) and `Measuring`, using the same `ThemePair` labels as the CLI stories. No new CSS: the button uses `.doctor__btn`.
- [ ] **Step 4:** `pnpm test src/features/doctor` and `pnpm build`. Expected: PASS. `cliDoctors.test.ts` and `undo.test.tsx` stay green.
- [ ] **Step 5:** commit `feat(doctor): a Measure MCP servers button with its own status line`.

### Task 11: Live check, research note and checks (the lead)

- [ ] **Step 1: Live check** (`marshell-live-check`). Set up:
  - Scratch `MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `QWEN_HOME`, with literal new folders.
  - Seed the scratch Claude config:
    - a user skill whose folder `CreationTime` is set 40 days back;
    - an approved `.mcp.json` server in a scratch project listed in `projects.json`, which runs `fake-agent mcp ok`;
    - a user server running `fake-agent mcp exit3`, and one running `fake-agent mcp silent`;
    - transcripts with a first record 40 days old, a call to the skill only in the scratch project 3 days ago, and no call to the `.mcp.json` server;
    - `usage.json` with `first_sight` 40 days back for the `.mcp.json` server.
  - Seed the scratch `CODEX_HOME` with a `config.toml` holding a `fake-agent mcp ok` server, and a `thread_history_1.sqlite` and `state_5.sqlite` with one `commandExecution` row 40 days old.

  Check, with small JSON evaluations:
  1. The first report has `mcpunused.claude.*` (Waste, unchecked) for the `.mcp.json` server, and `skillbroad.claude.*` naming the scratch project. Record the first and second runs' `duration_ms`.
  2. "Measure MCP servers" gives `mcpcost.claude`, with `1 measured`, and `mcpfail.claude.*` for the exit3 server (unchecked) and for the silent one after 30 s. No `fake-agent` process is left 5 s later; check with `Get-Process fake-agent` only, and never touch another program.
  3. Fixing the unused item, then Undo, gives back the exact bytes of `settings.local.json` (hash before and after).
  4. Keyboard alone: Tab reaches "Measure MCP servers", and Enter runs it. Under `browser_emulate_media` `forcedColors: active`, the button stays visible.

  Write the findings to `docs/research/2026-11-06-doctor-7c2-live-check.md`.
- [ ] **Step 2:** run what CI runs: `cargo fmt --all -- --check`, `cargo clippy -j 3 --workspace --all-targets -- -D warnings`, `cargo test -j 3 --workspace`, `pnpm test`, `pnpm check:generated` and `pnpm build`. Quote the counts.
- [ ] **Step 3:** commit the note, and tick the ledger.

## Known limits (for the PR)

- **Antigravity:**
  - Its servers are never called unused, because no MCP call has been seen in its transcripts (decision 16).
  - Its up-front MCP cost counts tool names only.
- **Qwen Code:**
  - Its history format, approval hash and deferred-tool bridge come from its source, with no local sample.
  - The `tool_call` bridge record is inferred.
- **Codex:**
  - Its full history is in an internal database whose schema can change; a schema Marshell doesn't know makes the Codex usage items report-only.
  - A deferred Codex server has no availability record, so its "added" date is Marshell's first sight.
  - The source of its namespace line's description wasn't traced.
  - A skill read by a relative path isn't seen.
- **Claude Code:** `auto` tool search assumes a 200,000-token window.
- **Not covered:**
  - The usage scan and Measure cover Windows only. A distro's servers and skills are never called unused.
  - Measurements are kept until the app restarts.
- **Start-up:** a server that `npx` must first download may need longer than its CLI's timeout and show as "didn't start". The item is unchecked.
