# Phase 1: Scaffold, One Live Tab, Webview Gate — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One real terminal tab works end to end. A Tauri 2 window renders xterm.js and talks only to a Rust core over localhost HTTP and WebSocket. The core owns the pty, scrollback, coalescing and backpressure. Spikes S1, S2, S6, S7 and S8 then decide whether the stack holds.

**Architecture:**
- `crates/protocol` holds the shared types and constants and exports them to TypeScript with ts-rs.
- `crates/core` is the engine:
  - an axum server on 127.0.0.1 with a per-launch token;
  - a pty `Session` built on portable-pty, whose blocking I/O runs on plain threads;
  - a byte-offset ring buffer plus a `vt100` screen, used for reconnect.
- `src-tauri` is a thin shell. It starts the core and hands the frontend `{port, token, platform}`.
- `src/` is a bare React window holding one `TerminalView`. There is no app chrome until phase 0 design approval.

**Tech Stack:**
- Rust (edition 2024, toolchain 1.98.1): tokio 1.53, axum 0.8 (ws), tower-http 0.7 (cors), portable-pty 0.9, vt100 0.16, ts-rs 12, getrandom 0.4.
- Tauri 2.12 with the single-instance and window-state plugins.
- React 19, Vite 8, TypeScript 7.
- xterm 6 with the fit and webgl addons.
- Vitest 5, pnpm 12.

**Spec:** `docs/PLAN.md` (approved). The "UX and UI design" section binds the tokens; §3 binds the IPC; §4 binds the concurrency rule; §7 holds the per-OS notes; §8 has the spikes. Also `docs/BRIEF.md` (its Amendments list wins over its body).

## Global Constraints

- **Names.** Every user-visible name, binary name, directory and env var comes from `crates/protocol/src/brand.rs`. The frontend gets them from `src/generated/constants.ts`. Never write the literal "marshell" or "Marshell" anywhere else, except in tests (which pin the wire format) and in config files that can't import (`tauri.conf.json`, `package.json`, `Cargo.toml`, `index.html`).
- **Server security.** The server binds `127.0.0.1` only. Every request needs the token (`Authorization: Bearer` or the `token.<t>` WebSocket subprotocol). `Host` must equal `127.0.0.1:<port>`. A present `Origin` must be on the allowlist.
- **Rust style** (the user is new to Rust):
  - structs, enums, `Result` + `?`, `Arc<Mutex<…>>`;
  - no lifetimes in public APIs and no custom traits in phase 1;
  - **lock, change, drop, then `.await` or do I/O**: never hold a `std::sync::MutexGuard` across `.await` or blocking I/O;
  - blocking pty reads, writes and waits run on `std::thread`s only, never on tokio worker threads.
- **Output pipeline numbers:**
  - coalesce every 8 ms or at 64 KiB (`FLUSH_BYTES`);
  - the reader stops when an attached client is more than 1 MiB behind (`MAX_UNACKED`);
  - the per-session byte ring is 8 MiB (`SCROLLBACK_BYTES`);
  - `seq` is a **byte offset**.
- **Frames** (`/v1/pty/{tab}`, binary, big-endian):

  | Direction | Opcode | Layout |
  |---|---|---|
  | Server → client | `0x01` OUTPUT | `[seq u64][bytes]` |
  | Server → client | `0x02` EXIT | `[code i32]` |
  | Server → client | `0x03` RESET | `[seq u64][screen bytes]` |
  | Client → server | `0x10` INPUT | `[bytes]` |
  | Client → server | `0x11` RESIZE | `[json {cols, rows}]` |
  | Client → server | `0x12` ACK | `[processed_up_to u64]` |
  | Client → server | `0x13` RESUME | `[from u64]` |

- **ts-rs.** Every `u64`/`i64` field gets `#[ts(type = "number")]`. `src/generated` never contains `bigint`.
- **Frontend.** It never writes files and talks only to the core API. Window state is Tauri's job.
- **xterm 6.** WebGL renderer, DOM renderer fallback (there is no canvas renderer in xterm 6). On Linux, `preserveDrawingBuffer` is true.
- **CSS.** Follow the good-css rules:
  - `oklch()` with `none` hue for grays, `light-dark()` tokens, logical properties;
  - `:focus-visible` outlines, `:hover` only inside `(hover: hover) and (pointer: fine)`;
  - no `ease-in`, no `transition: all`.
- **No UI chrome in phase 1.** The window shows the terminal and a hidden perf overlay only.
- **Package manager and license:** pnpm (`packageManager` field) and the MIT license.
- **Commits.** Use conventional commit messages, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. The repo has no git identity yet; before the first commit, ask the user for their name and email and set them with `git config user.name/user.email` (repo-local).

## Review Focus

| # | Condition | Expected behaviour | Test |
|---|---|---|---|
| 1 | Paths with spaces: the user's home is `C:\Users\Home Office` | A session started with a cwd containing a space starts there | Task 6, `cwd_with_space` |
| 2 | Multibyte UTF-8 split across pty reads | Bytes pass through untouched; the core never converts output to `String` | Task 6, `utf8_passthrough` |
| 3 | A bad shell path | `POST /v1/sessions` returns 4xx/5xx with a message instead of hanging, and the UI shows "Couldn't start a shell" | Task 6, `bad_shell_is_an_error` |
| 4 | Orphaned processes | Killing a session (or shutting the core down) kills the whole process tree, grandchildren included | Task 5, `kill_takes_the_tree` |
| 5 | Zero-size resize, e.g. a minimized window gives 0×0 | Ignored; the pty is never sized to 0 | Task 5, `zero_resize_is_ignored` |

---

## File structure

```
Cargo.toml                         workspace
rust-toolchain.toml                1.98.1
.cargo/config.toml                 TS_RS_EXPORT_DIR
.gitignore  LICENSE  README.md
package.json  pnpm-lock.yaml  tsconfig.json  vite.config.ts  index.html
scripts/check-generated.mjs        fails on bigint or stale generated files
crates/protocol/                   brand.rs (names), api.rs (HTTP types), frames.rs (opcodes), lib.rs (constants_ts + tests)
crates/core/
  src/lib.rs                       pub mods + re-exports
  src/paths.rs                     ~/.marshell resolution
  src/auth.rs                      token, constant-time compare, endpoint.json
  src/shell.rs                     default shell argv per OS
  src/pty/mod.rs                   pub use
  src/pty/buffer.rs                OutputBuffer (byte-offset ring)
  src/pty/dsr.rs                   DsrFilter (answers ConPTY's startup ESC[6n)
  src/pty/session.rs               Session: spawn, read/write/wait threads, backpressure, pull, kill
  src/server/mod.rs                CoreConfig, RunningCore, start(), router()
  src/server/guard.rs              Host / Origin / token middleware
  src/server/frames.rs             encode/decode pty frames
  src/server/sessions.rs           POST /v1/sessions
  src/server/pty_ws.rs             GET /v1/pty/{tab}
  src/bin/fake-agent.rs            deterministic child for tests (print, echo, flood, cwd, spawn-child)
  tests/server.rs  tests/session.rs  tests/pty_ws.rs  tests/fts5.rs
src-tauri/                         Cargo.toml, build.rs, tauri.conf.json, capabilities/default.json, src/main.rs, icons/
src/
  main.tsx  App.tsx
  generated/                       ts-rs output + constants.ts (committed)
  styles/tokens.css  styles/base.css  styles/app.css
  lib/api/frames.ts  lib/api/frames.test.ts  lib/api/ptySocket.ts  lib/api/session.ts
  features/terminal/TerminalView.tsx  features/terminal/renderer.ts  features/terminal/theme.ts
  features/perf/latency.ts  features/perf/latency.test.ts  features/perf/PerfOverlay.tsx
.github/workflows/ci.yml
docs/spikes/S1-webview-gate.md  S2-conpty.md  S6-throughput.md  S7-fts5.md  S8-elevated-host.md
spikes/elevated-host/              throwaway S8 spike crate (Windows)
```

---

### Task 1: Workspace, protocol crate, generated TypeScript

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.cargo/config.toml`, `.gitignore`, `LICENSE`, `README.md`
- Create: `crates/protocol/Cargo.toml`, `crates/protocol/src/{lib.rs,brand.rs,api.rs,frames.rs}`
- Create: `scripts/check-generated.mjs`
- Test: `crates/protocol/src/lib.rs` (`#[cfg(test)]`)

**Interfaces:**
- Produces:
  - `marshell_protocol::brand::{APP_NAME, CLI_BIN, HOOK_BIN, HOME_DIR_NAME, ENV_PREFIX, WS_SUBPROTOCOL, TOKEN_SUBPROTOCOL_PREFIX, env_var(&str) -> String}`
  - `marshell_protocol::api::{Platform, Endpoint, CreateSessionRequest, CreateSessionResponse, Resize, Health}`
  - `marshell_protocol::frames::{OUTPUT, EXIT, RESET, INPUT, RESIZE, ACK, RESUME}: u8`
  - `marshell_protocol::constants_ts() -> String`
  - Generated files `src/generated/{Platform,Endpoint,CreateSessionRequest,CreateSessionResponse,Resize,Health,constants}.ts`

- [ ] **Step 1: Create workspace files**

`Cargo.toml`:
```toml
[workspace]
resolver = "3"
members = ["crates/protocol", "crates/core", "src-tauri"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
rust-version = "1.98"

[workspace.dependencies]
marshell-protocol = { path = "crates/protocol" }
marshell-core = { path = "crates/core" }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }
ts-rs = "12"
anyhow = "1"
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "1.98.1"
components = ["rustfmt", "clippy"]
```

`.cargo/config.toml`:
```toml
[env]
# ts-rs writes generated TypeScript here (default would be ./bindings).
TS_RS_EXPORT_DIR = { value = "src/generated", relative = true }
```

`.gitignore`:
```
/target
/node_modules
/dist
/src-tauri/gen
*.log
.remember/
```

`LICENSE`: the standard MIT text with `Copyright (c) 2026 Marshell contributors`.

`README.md`:
```markdown
# Marshell

A desktop terminal session manager for AI coding-agent CLIs. See `docs/BRIEF.md` and `docs/PLAN.md`.

Dev: `pnpm install`, then `pnpm tauri dev`. Tests: `cargo test --workspace` and `pnpm test`.
```

Temporarily set `members = ["crates/protocol"]` until Tasks 2 and 9 create the other crates. Each of those tasks adds its member back.

- [ ] **Step 2: Create the protocol crate**

`crates/protocol/Cargo.toml`:
```toml
[package]
name = "marshell-protocol"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
ts-rs.workspace = true
```

`crates/protocol/src/brand.rs`:
```rust
//! Every user-visible name lives here, so a rename is one edit.
//! The frontend gets these through `src/generated/constants.ts`.

pub const APP_NAME: &str = "Marshell";
pub const CLI_BIN: &str = "marshell";
pub const HOOK_BIN: &str = "marshell-hook";
pub const HOME_DIR_NAME: &str = ".marshell";
pub const ENV_PREFIX: &str = "MARSHELL_";
pub const WS_SUBPROTOCOL: &str = "marshell.v1";
pub const TOKEN_SUBPROTOCOL_PREFIX: &str = "token.";

/// `env_var("TAB_ID")` -> `"MARSHELL_TAB_ID"`.
pub fn env_var(name: &str) -> String {
    format!("{ENV_PREFIX}{name}")
}
```

`crates/protocol/src/api.rs`:
```rust
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Linux
        }
    }
}

/// What the window needs to reach the core. Handed over by the Tauri command `core_endpoint`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Endpoint {
    pub port: u16,
    pub token: String,
    pub platform: Platform,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateSessionRequest {
    /// Full argv. Absent means the platform default shell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub shell: Option<Vec<String>>,
    /// Absent means the user's home directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateSessionResponse {
    pub tab_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Resize {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Health {
    pub app: String,
    pub version: String,
}
```

`crates/protocol/src/frames.rs`:
```rust
//! Opcodes for the binary frames on `/v1/pty/{tab}`. All integers are big-endian.

/// Server -> client: `[OUTPUT][seq u64][bytes]`. `seq` is the byte offset of the first byte.
pub const OUTPUT: u8 = 0x01;
/// Server -> client: `[EXIT][code i32]`. Sent after all output has been sent.
pub const EXIT: u8 = 0x02;
/// Server -> client: `[RESET][seq u64][screen bytes]`. The client clears its terminal, writes the
/// screen, and continues the live stream from `seq`.
pub const RESET: u8 = 0x03;
/// Client -> server: `[INPUT][bytes]`.
pub const INPUT: u8 = 0x10;
/// Client -> server: `[RESIZE][json Resize]`.
pub const RESIZE: u8 = 0x11;
/// Client -> server: `[ACK][processed_up_to u64]`. The renderer has fully processed output before this offset.
pub const ACK: u8 = 0x12;
/// Client -> server: `[RESUME][from u64]`. First frame after connecting; 0 on a fresh terminal.
pub const RESUME: u8 = 0x13;
```

`crates/protocol/src/lib.rs`:
```rust
pub mod api;
pub mod brand;
pub mod frames;

/// TypeScript source for the constants ts-rs cannot export (names and frame opcodes).
pub fn constants_ts() -> String {
    format!(
        "// Generated by crates/protocol (constants_ts). Do not edit.\n\
         export const APP_NAME = {app:?};\n\
         export const WS_SUBPROTOCOL = {ws:?};\n\
         export const TOKEN_SUBPROTOCOL_PREFIX = {tok:?};\n\
         export const FRAME = {{ OUTPUT: {o}, EXIT: {e}, RESET: {r}, INPUT: {i}, RESIZE: {rs}, ACK: {a}, RESUME: {re} }} as const;\n",
        app = brand::APP_NAME,
        ws = brand::WS_SUBPROTOCOL,
        tok = brand::TOKEN_SUBPROTOCOL_PREFIX,
        o = frames::OUTPUT,
        e = frames::EXIT,
        r = frames::RESET,
        i = frames::INPUT,
        rs = frames::RESIZE,
        a = frames::ACK,
        re = frames::RESUME,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_var_uses_prefix() {
        assert_eq!(brand::env_var("TAB_ID"), "MARSHELL_TAB_ID");
    }

    #[test]
    fn constants_ts_lists_every_opcode() {
        let ts = constants_ts();
        assert!(ts.contains("OUTPUT: 1,"));
        assert!(ts.contains("RESUME: 19 }"));
        assert!(ts.contains("export const APP_NAME = \"Marshell\";"));
    }

    /// Writes src/generated/constants.ts next to the ts-rs output.
    #[test]
    fn export_constants_ts() {
        let dir = std::env::var("TS_RS_EXPORT_DIR").expect("TS_RS_EXPORT_DIR is set in .cargo/config.toml");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join("constants.ts"), constants_ts()).unwrap();
    }
}
```

- [ ] **Step 3: Run the tests, which also generate the TypeScript**

Run: `cargo test -p marshell-protocol`

Expected: all tests PASS, including the ts-rs `export_bindings_*` tests. `src/generated/` contains `Platform.ts`, `Endpoint.ts`, `CreateSessionRequest.ts`, `CreateSessionResponse.ts`, `Resize.ts`, `Health.ts` and `constants.ts`. Open `Endpoint.ts`: `port` must be typed `number`.

- [ ] **Step 4: Write the generated-files check**

`scripts/check-generated.mjs`:
```js
// Fails CI when generated TypeScript uses bigint or is out of date with the Rust types.
import { execSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";

const dir = "src/generated";
const withBigint = readdirSync(dir).filter((f) => readFileSync(`${dir}/${f}`, "utf8").includes("bigint"));
if (withBigint.length > 0) {
  console.error(`bigint in generated types; add #[ts(type = "number")] to: ${withBigint.join(", ")}`);
  process.exit(1);
}
const stale = execSync(`git status --porcelain -- ${dir}`, { encoding: "utf8" }).trim();
if (stale) {
  console.error(`src/generated is stale. Run \`cargo test -p marshell-protocol\` and commit:\n${stale}`);
  process.exit(1);
}
```

Run: `node scripts/check-generated.mjs`

Expected: it exits 1 with "src/generated is stale". The files are untracked and not yet committed, so this proves the check works.

- [ ] **Step 5: Commit**

Ask for the git identity first (see Global Constraints).
```bash
git add Cargo.toml rust-toolchain.toml .cargo .gitignore LICENSE README.md crates/protocol scripts src/generated docs
git commit -m "chore: workspace, protocol crate and generated TS types"
node scripts/check-generated.mjs   # now exits 0
```

---

### Task 2: Core paths, token and endpoint file

**Files:**
- Create: `crates/core/Cargo.toml`, `crates/core/src/{lib.rs,paths.rs,auth.rs}`
- Modify: `Cargo.toml` (add `crates/core` to `members`)
- Test: unit tests in `auth.rs` and `paths.rs`

**Interfaces:**
- Consumes: `brand`, `api::Endpoint` (Task 1)
- Produces:
  - `paths::home_dir() -> anyhow::Result<PathBuf>`
  - `paths::endpoint_file(home: &Path) -> PathBuf`
  - `auth::new_token() -> String` (64 hex chars)
  - `auth::new_id(prefix: &str) -> String`
  - `auth::tokens_match(a: &str, b: &str) -> bool`
  - `auth::write_endpoint_file(path: &Path, ep: &Endpoint) -> anyhow::Result<()>`

- [ ] **Step 1: Create the crate and add dependencies**

`crates/core/Cargo.toml`:
```toml
[package]
name = "marshell-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
marshell-protocol.workspace = true
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
```

Then run:
```bash
cargo add -p marshell-core tokio --features full
cargo add -p marshell-core axum --features ws
cargo add -p marshell-core tower-http --features cors
cargo add -p marshell-core portable-pty@0.9 vt100@0.16 getrandom@0.4 hex dirs futures-util tracing
cargo add -p marshell-core --target 'cfg(unix)' libc
cargo add -p marshell-core --dev tempfile tower --features tower/util
cargo add -p marshell-core --dev tokio-tungstenite ureq --features ureq/json
cargo add -p marshell-core --dev rusqlite --features bundled
```

Add `"crates/core"` back to the workspace `members`.

- [ ] **Step 2: Write the failing tests**

`crates/core/src/lib.rs`:
```rust
pub mod auth;
pub mod paths;
```

`crates/core/src/paths.rs`:
```rust
use marshell_protocol::brand;
use std::path::{Path, PathBuf};

/// Root of everything Marshell stores: `$MARSHELL_HOME` if set (tests), else `~/.marshell`.
pub fn home_dir() -> anyhow::Result<PathBuf> {
    if let Some(dir) = std::env::var_os(brand::env_var("HOME")) {
        return Ok(PathBuf::from(dir));
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not find the user's home directory"))?;
    Ok(home.join(brand::HOME_DIR_NAME))
}

/// `<home>/run/endpoint.json`: port + token for the bridge and CLI.
pub fn endpoint_file(home: &Path) -> PathBuf {
    home.join("run").join("endpoint.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_file_is_under_run() {
        let p = endpoint_file(Path::new("/x/.marshell"));
        assert!(p.ends_with("run/endpoint.json") || p.ends_with("run\\endpoint.json"));
    }
}
```

`crates/core/src/auth.rs` (tests first; the functions are stubs):
```rust
use marshell_protocol::api::Endpoint;
use std::path::Path;

pub fn new_token() -> String {
    todo!()
}

pub fn new_id(prefix: &str) -> String {
    todo!()
}

pub fn tokens_match(a: &str, b: &str) -> bool {
    todo!()
}

pub fn write_endpoint_file(path: &Path, ep: &Endpoint) -> anyhow::Result<()> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::Platform;

    #[test]
    fn token_is_64_hex_and_unique() {
        let a = new_token();
        let b = new_token();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn ids_have_prefix() {
        let id = new_id("t");
        assert!(id.starts_with('t'));
        assert_eq!(id.len(), 1 + 12);
    }

    #[test]
    fn tokens_match_only_exactly() {
        assert!(tokens_match("abc", "abc"));
        assert!(!tokens_match("abc", "abd"));
        assert!(!tokens_match("abc", "abcd"));
        assert!(!tokens_match("", "a"));
    }

    #[test]
    fn endpoint_file_round_trips_and_is_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run").join("endpoint.json");
        let ep = Endpoint { port: 4242, token: "t0k".into(), platform: Platform::current() };
        write_endpoint_file(&path, &ep).unwrap();
        let back: Endpoint = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(back.port, 4242);
        assert_eq!(back.token, "t0k");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p marshell-core auth`

Expected: FAIL; the tests panic at `not yet implemented`.

- [ ] **Step 4: Implement**

Replace the four stubs in `auth.rs`:
```rust
fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("the OS random number generator failed");
    hex::encode(buf)
}

/// 32 random bytes as hex. A new token every launch.
pub fn new_token() -> String {
    random_hex(32)
}

/// Short random id such as `t3f9a0c41b2de` (prefix + 12 hex chars).
pub fn new_id(prefix: &str) -> String {
    format!("{prefix}{}", random_hex(6))
}

/// Compares in constant time, so response timing does not leak how much of a guess was right.
pub fn tokens_match(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Writes the endpoint file atomically (temp file + rename), readable by the user only.
/// On Windows the file inherits the user-only ACL of the profile directory.
pub fn write_endpoint_file(path: &Path, ep: &Endpoint) -> anyhow::Result<()> {
    let dir = path.parent().ok_or_else(|| anyhow::anyhow!("endpoint path has no parent"))?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(".endpoint.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(ep)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}
```

If `getrandom::fill` doesn't exist in 0.4, check `cargo doc -p getrandom --open` for the fill-a-buffer function, and keep the same `random_hex` signature.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p marshell-core`

Expected: PASS (5 tests).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/core
git commit -m "feat(core): home paths, per-launch token and endpoint file"
```

---

### Task 3: Core HTTP server with Host, Origin and token guard

**Files:**
- Create: `crates/core/src/server/{mod.rs,guard.rs}`
- Modify: `crates/core/src/lib.rs`
- Test: `crates/core/tests/server.rs`

**Interfaces:**
- Consumes: `auth::*`, `paths::endpoint_file` (Task 2)
- Produces:
  - `server::CoreConfig { home: PathBuf, allowed_origins: Vec<String> }`
  - `server::RunningCore { pub endpoint: Endpoint, pub state: AppState }`, with `RunningCore::shutdown(self)`
  - `server::start(cfg) -> anyhow::Result<RunningCore>` (async)
  - `server::router(state: AppState) -> axum::Router`
  - `server::AppState`, a clonable handle holding `token`, `host`, `allowed_origins` and `sessions: Mutex<HashMap<String, Arc<Session>>>`. The `sessions` field is used from Task 6 on; in this task it is created empty.
  - Route `GET /v1/health -> Health`

- [ ] **Step 1: Write the failing integration test**

`crates/core/tests/server.rs`:
```rust
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use marshell_core::server::{router, AppState};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";

fn app() -> axum::Router {
    router(AppState::new(TOKEN.into(), PORT, vec![ORIGIN.into()]))
}

fn get(path: &str) -> axum::http::request::Builder {
    Request::builder().uri(path).header(header::HOST, format!("127.0.0.1:{PORT}"))
}

#[tokio::test]
async fn health_requires_token() {
    let res = app().oneshot(get("/v1/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn health_with_token_is_ok() {
    let req = get("/v1/health").header(header::AUTHORIZATION, format!("Bearer {TOKEN}")).body(Body::empty()).unwrap();
    let res = app().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn wrong_host_is_forbidden_even_with_token() {
    let req = Request::builder()
        .uri("/v1/health")
        .header(header::HOST, "evil.example:5555")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn foreign_origin_is_forbidden() {
    let req = get("/v1/health")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn token_in_websocket_subprotocol_is_accepted() {
    let req = get("/v1/health")
        .header(header::SEC_WEBSOCKET_PROTOCOL, format!("marshell.v1, token.{TOKEN}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn cors_preflight_from_allowed_origin() {
    let req = get("/v1/health")
        .method("OPTIONS")
        .header(header::ORIGIN, ORIGIN)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "authorization,content-type")
        .body(Body::empty())
        .unwrap();
    let res = app().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], ORIGIN);
}

#[tokio::test]
async fn start_writes_endpoint_file_and_serves() {
    let home = tempfile::tempdir().unwrap();
    let core = marshell_core::server::start(marshell_core::server::CoreConfig {
        home: home.path().to_path_buf(),
        allowed_origins: vec![],
    })
    .await
    .unwrap();
    let ep: marshell_protocol::api::Endpoint =
        serde_json::from_slice(&std::fs::read(home.path().join("run/endpoint.json")).unwrap()).unwrap();
    assert_eq!(ep.port, core.endpoint.port);
    let url = format!("http://127.0.0.1:{}/v1/health", ep.port);
    let token = ep.token.clone();
    let status = tokio::task::spawn_blocking(move || {
        ureq::get(&url).header("Authorization", &format!("Bearer {token}")).call().unwrap().status()
    })
    .await
    .unwrap();
    assert_eq!(status, 200);
    core.shutdown();
}
```

Run: `cargo test -p marshell-core --test server`

Expected: FAIL to compile (`server` module missing).

- [ ] **Step 2: Implement the guard**

`crates/core/src/server/guard.rs`:
```rust
use super::AppState;
use crate::auth;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use marshell_protocol::brand;

/// Every request passes three checks, in order:
/// 1. `Host` is exactly `127.0.0.1:<port>` (blocks DNS rebinding).
/// 2. `Origin`, when present, is on the allowlist (blocks other web pages).
/// 3. The per-launch token is present (Bearer header, or `token.<t>` WebSocket subprotocol).
pub async fn guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    let host_ok = headers.get(header::HOST).and_then(|h| h.to_str().ok()) == Some(st.host());
    if !host_ok {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Some(origin) = headers.get(header::ORIGIN) {
        let allowed = origin.to_str().map(|o| st.origin_allowed(o)).unwrap_or(false);
        if !allowed {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    let ok = request_token(headers).is_some_and(|t| auth::tokens_match(&t, st.token()));
    if !ok {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(req).await
}

/// The token from `Authorization: Bearer <t>` or from a `token.<t>` entry in `Sec-WebSocket-Protocol`.
pub fn request_token(headers: &HeaderMap) -> Option<String> {
    if let Some(v) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(t) = v.strip_prefix("Bearer ") {
            return Some(t.trim().to_string());
        }
    }
    let protocols = headers.get(header::SEC_WEBSOCKET_PROTOCOL)?.to_str().ok()?;
    protocols
        .split(',')
        .map(str::trim)
        .find_map(|p| p.strip_prefix(brand::TOKEN_SUBPROTOCOL_PREFIX))
        .map(str::to_string)
}
```

- [ ] **Step 3: Implement the server**

`crates/core/src/server/mod.rs`:
```rust
mod guard;

use crate::{auth, paths};
use axum::http::{header, HeaderValue, Method};
use axum::routing::get;
use axum::{Json, Router};
use marshell_protocol::api::{Endpoint, Health, Platform};
use marshell_protocol::brand;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tower_http::cors::{AllowOrigin, CorsLayer};

pub struct CoreConfig {
    /// Usually `paths::home_dir()`; a temp dir in tests.
    pub home: PathBuf,
    /// Webview origins allowed to call the API, e.g. `http://tauri.localhost`.
    pub allowed_origins: Vec<String>,
}

/// Shared server state. Cloning is cheap (one `Arc`).
#[derive(Clone)]
pub struct AppState(Arc<Inner>);

struct Inner {
    token: String,
    host: String,
    allowed_origins: Vec<String>,
    // Filled in Task 6. Kept here so every handler reaches it through one State.
    sessions: Mutex<HashMap<String, Arc<crate::pty::Session>>>,
}

impl AppState {
    pub fn new(token: String, port: u16, allowed_origins: Vec<String>) -> Self {
        Self(Arc::new(Inner {
            token,
            host: format!("127.0.0.1:{port}"),
            allowed_origins,
            sessions: Mutex::new(HashMap::new()),
        }))
    }
    pub fn token(&self) -> &str {
        &self.0.token
    }
    pub fn host(&self) -> &str {
        &self.0.host
    }
    pub fn origin_allowed(&self, origin: &str) -> bool {
        self.0.allowed_origins.iter().any(|o| o == origin)
    }
}

pub fn router(state: AppState) -> Router {
    let origins: Vec<HeaderValue> = state.0.allowed_origins.iter().filter_map(|o| o.parse().ok()).collect();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
    Router::new()
        .route("/v1/health", get(health))
        .layer(axum::middleware::from_fn_with_state(state.clone(), guard::guard))
        // Added last = outermost: CORS answers preflights before the guard runs.
        .layer(cors)
        .with_state(state)
}

async fn health() -> Json<Health> {
    Json(Health { app: brand::APP_NAME.into(), version: env!("CARGO_PKG_VERSION").into() })
}

pub struct RunningCore {
    pub endpoint: Endpoint,
    pub state: AppState,
    shutdown: tokio::sync::oneshot::Sender<()>,
}

impl RunningCore {
    /// Stops the server. Task 5 adds: kill every session's process tree.
    pub fn shutdown(self) {
        let _ = self.shutdown.send(());
    }
}

/// Binds 127.0.0.1 on a random port, writes `run/endpoint.json`, serves in the background.
pub async fn start(cfg: CoreConfig) -> anyhow::Result<RunningCore> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let endpoint = Endpoint { port, token: auth::new_token(), platform: Platform::current() };
    auth::write_endpoint_file(&paths::endpoint_file(&cfg.home), &endpoint)?;
    let state = AppState::new(endpoint.token.clone(), port, cfg.allowed_origins);
    let app = router(state.clone());
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let serve = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = rx.await;
        });
        if let Err(e) = serve.await {
            tracing::error!("core server stopped: {e}");
        }
    });
    Ok(RunningCore { endpoint, state, shutdown: tx })
}
```

Until Task 5 exists, `crate::pty::Session` is missing. Add a placeholder `pub mod pty { pub struct Session; }` in `lib.rs` for this task; Task 5 replaces it with the real module.

`crates/core/src/lib.rs`:
```rust
pub mod auth;
pub mod paths;
pub mod pty {
    pub struct Session;
}
pub mod server;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p marshell-core --test server`

Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "feat(core): loopback server with host, origin and token guard"
```

---

### Task 4: Output buffer, DSR filter and screen snapshot (pure logic)

**Files:**
- Create: `crates/core/src/pty/{mod.rs,buffer.rs,dsr.rs}`
- Modify: `crates/core/src/lib.rs` (replace the placeholder `pty` module with `pub mod pty;`, and add a temporary `pub struct Session;` inside `pty/mod.rs` so `server` still compiles)
- Test: unit tests in both files

**Interfaces:**
- Produces:
  - `pty::buffer::{OutputBuffer, SCROLLBACK_BYTES, FLUSH_BYTES, MAX_UNACKED}`
  - `OutputBuffer::new(cap: usize)`, `.push(&[u8])`, `.start() -> u64`, `.end() -> u64`, `.read_from(from: u64, max: usize) -> Option<Vec<u8>>` (`None` means `from` has been evicted)
  - `pty::dsr::{DsrFilter, DSR_REPLY}`
  - `DsrFilter::filter(&mut self, input: &[u8]) -> (Vec<u8>, usize)`
  - `DsrFilter::take_carry(&mut self) -> Vec<u8>`

- [ ] **Step 1: Write the failing tests**

`crates/core/src/pty/buffer.rs`:
```rust
use std::collections::VecDeque;

/// Bytes of raw output kept per session for replay (roughly 10k+ lines).
pub const SCROLLBACK_BYTES: usize = 8 * 1024 * 1024;
/// Send output in frames of at most this size; also the "flush now" threshold.
pub const FLUSH_BYTES: usize = 64 * 1024;
/// Stop reading the pty when an attached client is this far behind.
pub const MAX_UNACKED: u64 = 1024 * 1024;

/// A ring of output bytes addressed by absolute byte offset ("seq").
/// Old bytes fall off the front once `cap` is exceeded.
pub struct OutputBuffer {
    data: VecDeque<u8>,
    start: u64,
    cap: usize,
}

impl OutputBuffer {
    pub fn new(cap: usize) -> Self {
        todo!()
    }
    pub fn start(&self) -> u64 {
        todo!()
    }
    pub fn end(&self) -> u64 {
        todo!()
    }
    pub fn push(&mut self, bytes: &[u8]) {
        todo!()
    }
    pub fn read_from(&self, from: u64, max: usize) -> Option<Vec<u8>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_read() {
        let mut b = OutputBuffer::new(100);
        b.push(b"hello ");
        b.push(b"world");
        assert_eq!(b.end(), 11);
        assert_eq!(b.read_from(0, 100).unwrap(), b"hello world");
        assert_eq!(b.read_from(6, 3).unwrap(), b"wor");
    }

    #[test]
    fn read_at_end_is_empty_not_none() {
        let mut b = OutputBuffer::new(10);
        b.push(b"abc");
        assert_eq!(b.read_from(3, 10).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn eviction_moves_start_and_old_reads_return_none() {
        let mut b = OutputBuffer::new(4);
        b.push(b"abcdef");
        assert_eq!(b.start(), 2);
        assert_eq!(b.end(), 6);
        assert_eq!(b.read_from(2, 10).unwrap(), b"cdef");
        assert!(b.read_from(1, 10).is_none());
    }

    #[test]
    fn read_past_end_is_clamped_to_empty() {
        let mut b = OutputBuffer::new(10);
        b.push(b"ab");
        assert_eq!(b.read_from(9, 10).unwrap(), Vec::<u8>::new());
    }
}
```

`crates/core/src/pty/dsr.rs`:
```rust
//! ConPTY (portable-pty 0.9 sets PSEUDOCONSOLE_INHERIT_CURSOR) asks for the cursor position with
//! ESC[6n at startup and holds back all output until it gets an answer. While no renderer is
//! attached, the core answers instead and removes the query from the stream. That way a renderer
//! that attaches later never answers it a second time.

const DSR: &[u8] = b"\x1b[6n";
/// "Cursor is at row 1, column 1."
pub const DSR_REPLY: &[u8] = b"\x1b[1;1R";

#[derive(Default)]
pub struct DsrFilter {
    carry: Vec<u8>,
}

impl DsrFilter {
    /// Returns the bytes to pass on, and how many queries need an answer.
    pub fn filter(&mut self, input: &[u8]) -> (Vec<u8>, usize) {
        todo!()
    }

    /// Bytes held back because they might start a query. Call this when a renderer attaches.
    pub fn take_carry(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.carry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_and_counts_a_whole_query() {
        let mut f = DsrFilter::default();
        assert_eq!(f.filter(b"a\x1b[6nb"), (b"ab".to_vec(), 1));
    }

    #[test]
    fn handles_a_query_split_across_reads() {
        let mut f = DsrFilter::default();
        assert_eq!(f.filter(b"x\x1b["), (b"x".to_vec(), 0));
        assert_eq!(f.filter(b"6ny"), (b"y".to_vec(), 1));
    }

    #[test]
    fn passes_look_alike_sequences() {
        let mut f = DsrFilter::default();
        assert_eq!(f.filter(b"\x1b[6m\x1b[2J"), (b"\x1b[6m\x1b[2J".to_vec(), 0));
    }

    #[test]
    fn counts_several_queries() {
        let mut f = DsrFilter::default();
        assert_eq!(f.filter(b"\x1b[6n\x1b[6n"), (Vec::new(), 2));
    }

    #[test]
    fn carry_is_released_on_attach() {
        let mut f = DsrFilter::default();
        let _ = f.filter(b"z\x1b[6");
        assert_eq!(f.take_carry(), b"\x1b[6".to_vec());
    }
}
```

`crates/core/src/pty/mod.rs`:
```rust
pub mod buffer;
pub mod dsr;

/// Placeholder until Task 5.
pub struct Session;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p marshell-core pty::`

Expected: FAIL (`not yet implemented`).

- [ ] **Step 3: Implement**

The `OutputBuffer` bodies:
```rust
    pub fn new(cap: usize) -> Self {
        Self { data: VecDeque::new(), start: 0, cap }
    }
    pub fn start(&self) -> u64 {
        self.start
    }
    pub fn end(&self) -> u64 {
        self.start + self.data.len() as u64
    }
    pub fn push(&mut self, bytes: &[u8]) {
        self.data.extend(bytes);
        let excess = self.data.len().saturating_sub(self.cap);
        if excess > 0 {
            self.data.drain(..excess);
            self.start += excess as u64;
        }
    }
    pub fn read_from(&self, from: u64, max: usize) -> Option<Vec<u8>> {
        if from < self.start {
            return None;
        }
        let offset = ((from - self.start) as usize).min(self.data.len());
        let len = max.min(self.data.len() - offset);
        Some(self.data.range(offset..offset + len).copied().collect())
    }
```

`DsrFilter::filter`:
```rust
    pub fn filter(&mut self, input: &[u8]) -> (Vec<u8>, usize) {
        let mut buf = std::mem::take(&mut self.carry);
        buf.extend_from_slice(input);
        let mut out = Vec::with_capacity(buf.len());
        let mut answered = 0;
        let mut i = 0;
        while i < buf.len() {
            let rest = &buf[i..];
            if rest.starts_with(DSR) {
                answered += 1;
                i += DSR.len();
            } else if rest.len() < DSR.len() && DSR.starts_with(rest) {
                // Might be the start of a query; wait for the next read.
                self.carry = rest.to_vec();
                break;
            } else {
                out.push(buf[i]);
                i += 1;
            }
        }
        (out, answered)
    }
```

- [ ] **Step 4: Add the screen snapshot test (pins the vt100 API)**

Append to `buffer.rs` tests:
```rust
    #[test]
    fn vt100_snapshot_reproduces_the_screen() {
        let mut p = vt100::Parser::new(5, 20, 0);
        p.process(b"hello\r\n\x1b[1mbold\x1b[0m world");
        let snapshot = p.screen().state_formatted();
        let mut q = vt100::Parser::new(5, 20, 0);
        q.process(&snapshot);
        assert_eq!(q.screen().contents(), p.screen().contents());
        assert_eq!(q.screen().cursor_position(), p.screen().cursor_position());
    }
```

Run: `cargo test -p marshell-core pty::`

Expected: PASS (10 tests). If `state_formatted` isn't found, use `contents_formatted()` followed by `cursor_state_formatted()`, concatenated, and record which API worked in a code comment.

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "feat(core): output ring buffer, ConPTY DSR filter, vt100 snapshot"
```

---

### Task 5: fake-agent and the pty Session

**Files:**
- Create: `crates/core/src/bin/fake-agent.rs`, `crates/core/src/shell.rs`, `crates/core/src/pty/session.rs`
- Modify: `crates/core/src/pty/mod.rs` (drop the placeholder, add `mod session; pub use session::*;`), `crates/core/src/lib.rs` (`pub mod shell;`), `crates/core/src/server/mod.rs` (`RunningCore::shutdown` kills sessions)
- Test: `crates/core/tests/session.rs`

**Interfaces:**
- Consumes: `OutputBuffer`, `DsrFilter`, `DSR_REPLY` and the constants (Task 4)
- Produces:
  - `pty::SpawnSpec { argv: Vec<String>, cwd: Option<PathBuf>, env: Vec<(String, String)>, cols: u16, rows: u16 }`
  - `pty::Session::spawn(id: String, spec: SpawnSpec) -> anyhow::Result<Arc<Session>>`
  - `Session::{write_input(&self, Vec<u8>), resize(&self, cols: u16, rows: u16) -> anyhow::Result<()>, attach(&self) -> u64, detach(&self, generation: u64), resume(&self, from: u64), ack(&self, upto: u64), pull(&self, cursor: u64, max: usize) -> Pull, available(&self, cursor: u64) -> u64, notified(&self) -> Notified<'_>, kill(&self), counters(&self) -> (u64, u64), id(&self) -> &str}`
  - `pty::Pull { Data { seq: u64, bytes: Vec<u8> }, Reset { seq: u64, screen: Vec<u8> }, Exit(i32), Idle }`
  - `shell::default_shell() -> Vec<String>`

- [ ] **Step 1: Write fake-agent**

`crates/core/src/bin/fake-agent.rs`:
```rust
//! A deterministic child process for tests. Not shipped to users.
//!   fake-agent print <text>        print text + newline, exit 0
//!   fake-agent echo                for each stdin line print "got:<line>"; "exit" quits
//!   fake-agent flood <bytes>       write <bytes> of 80-char lines, exit 0
//!   fake-agent cwd                 print the current directory, exit 0
//!   fake-agent spawn-child         start a sleeping grandchild, print "child:<pid>", sleep
//!   fake-agent sleep               sleep 60 s
//!   fake-agent exit <code>         exit with <code>
use std::io::{BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = std::io::stdout().lock();
    match args.first().map(String::as_str) {
        Some("print") => writeln!(out, "{}", args[1..].join(" ")).unwrap(),
        Some("echo") => {
            for line in std::io::stdin().lock().lines() {
                let line = line.unwrap();
                let line = line.trim();
                if line == "exit" {
                    break;
                }
                writeln!(out, "got:{line}").unwrap();
                out.flush().unwrap();
            }
        }
        Some("flood") => {
            let total: usize = args[1].parse().unwrap();
            let line = [b'x'; 79];
            let mut written = 0;
            while written < total {
                out.write_all(&line).unwrap();
                out.write_all(b"\n").unwrap();
                written += 80;
            }
        }
        Some("cwd") => writeln!(out, "{}", std::env::current_dir().unwrap().display()).unwrap(),
        Some("spawn-child") => {
            let me = std::env::current_exe().unwrap();
            let child = std::process::Command::new(me).arg("sleep").spawn().unwrap();
            writeln!(out, "child:{}", child.id()).unwrap();
            out.flush().unwrap();
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
        Some("sleep") => std::thread::sleep(std::time::Duration::from_secs(60)),
        Some("exit") => std::process::exit(args[1].parse().unwrap()),
        _ => eprintln!("unknown mode"),
    }
    out.flush().unwrap();
}
```

- [ ] **Step 2: Write the failing session tests**

`crates/core/tests/session.rs`:
```rust
use marshell_core::pty::{Pull, Session, SpawnSpec, MAX_UNACKED};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn fake(args: &[&str]) -> SpawnSpec {
    let mut argv = vec![env!("CARGO_BIN_EXE_fake-agent").to_string()];
    argv.extend(args.iter().map(|s| s.to_string()));
    SpawnSpec { argv, cwd: None, env: vec![], cols: 80, rows: 24 }
}

/// Reads everything until Exit (or the timeout), acking as it goes. Returns (bytes, exit code).
fn drain(s: &Arc<Session>, timeout: Duration) -> (Vec<u8>, Option<i32>) {
    let generation = s.attach();
    s.resume(0);
    let deadline = Instant::now() + timeout;
    let (mut cursor, mut all) = (0u64, Vec::new());
    while Instant::now() < deadline {
        match s.pull(cursor, 64 * 1024) {
            Pull::Data { seq, bytes } => {
                cursor = seq + bytes.len() as u64;
                all.extend(bytes);
                s.ack(cursor);
            }
            Pull::Reset { seq, screen } => {
                cursor = seq;
                all.extend(screen);
                s.ack(cursor);
            }
            Pull::Exit(code) => {
                s.detach(generation);
                return (all, Some(code));
            }
            Pull::Idle => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    s.detach(generation);
    (all, None)
}

#[test]
fn prints_and_exits_zero() {
    let s = Session::spawn("t1".into(), fake(&["print", "hello marshell"])).unwrap();
    let (out, code) = drain(&s, Duration::from_secs(10));
    assert!(String::from_utf8_lossy(&out).contains("hello marshell"));
    assert_eq!(code, Some(0));
}

#[test]
fn reports_nonzero_exit() {
    let s = Session::spawn("t2".into(), fake(&["exit", "3"])).unwrap();
    assert_eq!(drain(&s, Duration::from_secs(10)).1, Some(3));
}

#[test]
fn input_reaches_the_child() {
    let s = Session::spawn("t3".into(), fake(&["echo"])).unwrap();
    s.write_input(b"ping\r".to_vec());
    s.write_input(b"exit\r".to_vec());
    let (out, code) = drain(&s, Duration::from_secs(10));
    assert!(String::from_utf8_lossy(&out).contains("got:ping"));
    assert_eq!(code, Some(0));
}

#[test]
fn backpressure_bounds_unacked_output() {
    let s = Session::spawn("t4".into(), fake(&["flood", "8000000"])).unwrap();
    let generation = s.attach();
    s.resume(0);
    std::thread::sleep(Duration::from_millis(1500)); // never ack
    let (end, acked) = s.counters();
    assert!(end - acked <= MAX_UNACKED + 64 * 1024, "unacked {} exceeds the limit", end - acked);
    s.detach(generation);
    s.kill();
}

#[test]
fn zero_resize_is_ignored() {
    let s = Session::spawn("t5".into(), fake(&["sleep"])).unwrap();
    s.resize(0, 0).unwrap();
    s.resize(100, 30).unwrap();
    s.kill();
}

#[test]
fn kill_takes_the_tree() {
    let s = Session::spawn("t6".into(), fake(&["spawn-child"])).unwrap();
    let generation = s.attach();
    s.resume(0);
    let deadline = Instant::now() + Duration::from_secs(10);
    let (mut cursor, mut text) = (0u64, String::new());
    let child_pid: u32 = loop {
        assert!(Instant::now() < deadline, "no child pid printed");
        if let Pull::Data { seq, bytes } = s.pull(cursor, 65536) {
            cursor = seq + bytes.len() as u64;
            text.push_str(&String::from_utf8_lossy(&bytes));
            s.ack(cursor);
            if let Some(rest) = text.split("child:").nth(1) {
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !digits.is_empty() && rest.len() > digits.len() {
                    break digits.parse().unwrap();
                }
            }
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    s.detach(generation);
    s.kill();
    std::thread::sleep(Duration::from_millis(2500));
    assert!(!process_alive(child_pid), "grandchild {child_pid} survived kill");
}

#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    let out = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}
```

Add `libc` as a unix dev-dependency so the test compiles: `cargo add -p marshell-core --dev --target 'cfg(unix)' libc`.

Run: `cargo test -p marshell-core --test session`

Expected: FAIL to compile (`Session`, `SpawnSpec` and `Pull` don't exist).

- [ ] **Step 3: Implement the default shell**

`crates/core/src/shell.rs`:
```rust
/// The shell a new tab runs when the request names none.
/// Windows: PowerShell 7 if installed, else Windows PowerShell. Unix: `$SHELL -l`.
/// (Full shell detection and profiles come in phase 2.)
pub fn default_shell() -> Vec<String> {
    #[cfg(windows)]
    {
        let exe = if on_path("pwsh.exe") { "pwsh.exe" } else { "powershell.exe" };
        vec![exe.to_string(), "-NoLogo".to_string()]
    }
    #[cfg(unix)]
    {
        let sh = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        vec![sh, "-l".to_string()]
    }
}

#[cfg(windows)]
fn on_path(exe: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|dir| dir.join(exe).is_file()))
        .unwrap_or(false)
}
```

- [ ] **Step 4: Implement Session**

`crates/core/src/pty/session.rs`:
```rust
use super::buffer::{OutputBuffer, MAX_UNACKED, SCROLLBACK_BYTES};
use super::dsr::{DsrFilter, DSR_REPLY};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub struct SpawnSpec {
    pub argv: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub cols: u16,
    pub rows: u16,
}

/// What the WebSocket task should send next.
pub enum Pull {
    Data { seq: u64, bytes: Vec<u8> },
    /// The cursor fell out of the ring: send the screen, then continue from `seq`.
    Reset { seq: u64, screen: Vec<u8> },
    Exit(i32),
    Idle,
}

/// Everything about the output stream, behind one std Mutex.
/// Rule: lock, change, drop. Never hold it across `.await` or I/O.
struct Output {
    buf: OutputBuffer,
    screen: vt100::Parser,
    dsr: DsrFilter,
    attached: bool,
    generation: u64,
    acked: u64,
    exit: Option<(i32, Instant)>,
    reader_done: bool,
}

pub struct Session {
    id: String,
    output: Mutex<Output>,
    /// The reader thread waits here while the client is more than MAX_UNACKED behind.
    space: Condvar,
    /// Wakes the async WebSocket task when there is something new to pull.
    notify: tokio::sync::Notify,
    /// Input goes through a channel to a writer thread, so async code never blocks on the pty.
    input: mpsc::Sender<Vec<u8>>,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    pid: Option<u32>,
}

impl Session {
    pub fn spawn(id: String, spec: SpawnSpec) -> anyhow::Result<Arc<Session>> {
        let size = PtySize { rows: spec.rows.max(1), cols: spec.cols.max(1), pixel_width: 0, pixel_height: 0 };
        let pair = native_pty_system().openpty(size)?;
        let mut cmd = CommandBuilder::from_argv(spec.argv.iter().map(OsString::from).collect());
        if let Some(cwd) = &spec.cwd {
            cmd.cwd(cwd);
        }
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        let mut child = pair.slave.spawn_command(cmd)?;
        // Our copy of the slave must close, or the reader never sees EOF on Unix.
        drop(pair.slave);
        let pid = child.process_id();
        let reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;
        let (input_tx, input_rx) = mpsc::channel::<Vec<u8>>();

        let session = Arc::new(Session {
            id: id.clone(),
            output: Mutex::new(Output {
                buf: OutputBuffer::new(SCROLLBACK_BYTES),
                screen: vt100::Parser::new(size.rows, size.cols, 0),
                dsr: DsrFilter::default(),
                attached: false,
                generation: 0,
                acked: 0,
                exit: None,
                reader_done: false,
            }),
            space: Condvar::new(),
            notify: tokio::sync::Notify::new(),
            input: input_tx,
            master: Mutex::new(Some(pair.master)),
            pid,
        });

        std::thread::Builder::new().name(format!("pty-write-{id}")).spawn(move || {
            for bytes in input_rx {
                if writer.write_all(&bytes).and_then(|_| writer.flush()).is_err() {
                    break;
                }
            }
        })?;

        let s = session.clone();
        std::thread::Builder::new().name(format!("pty-read-{id}")).spawn(move || s.read_loop(reader))?;

        let s = session.clone();
        std::thread::Builder::new().name(format!("pty-wait-{id}")).spawn(move || {
            let code = child.wait().map(|st| st.exit_code() as i32).unwrap_or(-1);
            s.on_exit(code);
        })?;

        Ok(session)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    fn read_loop(&self, mut reader: Box<dyn Read + Send>) {
        let mut chunk = vec![0u8; 16 * 1024];
        loop {
            let n = match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let answered = self.ingest(&chunk[..n]);
            for _ in 0..answered {
                self.write_input(DSR_REPLY.to_vec());
            }
            self.wait_for_space();
        }
        self.output.lock().unwrap().reader_done = true;
        self.notify.notify_one();
    }

    /// Appends output to the ring and the screen. Returns how many ESC[6n queries we must answer.
    fn ingest(&self, bytes: &[u8]) -> usize {
        let mut out = self.output.lock().unwrap();
        let (pass, answered) = if out.attached { (bytes.to_vec(), 0) } else { out.dsr.filter(bytes) };
        out.screen.process(&pass);
        out.buf.push(&pass);
        drop(out);
        self.notify.notify_one();
        answered
    }

    fn wait_for_space(&self) {
        let mut out = self.output.lock().unwrap();
        while out.attached && out.exit.is_none() && out.buf.end().saturating_sub(out.acked) > MAX_UNACKED {
            out = self.space.wait(out).unwrap();
        }
    }

    fn on_exit(&self, code: i32) {
        self.output.lock().unwrap().exit = Some((code, Instant::now()));
        self.space.notify_all();
        // Windows: the reader only gets EOF once the pseudoconsole is closed.
        self.master.lock().unwrap().take();
        self.notify.notify_one();
    }

    pub fn write_input(&self, bytes: Vec<u8>) {
        let _ = self.input.send(bytes);
    }

    /// Ignores 0×0 (a minimized window reports it).
    pub fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        if cols == 0 || rows == 0 {
            return Ok(());
        }
        if let Some(m) = self.master.lock().unwrap().as_ref() {
            m.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
        }
        // vt100 0.16; on 0.15 this is `parser.set_size(rows, cols)`.
        self.output.lock().unwrap().screen.screen_mut().set_size(rows, cols);
        Ok(())
    }

    /// A renderer connected. Returns a generation number for `detach`.
    /// From now on the renderer (xterm) answers cursor queries itself.
    pub fn attach(&self) -> u64 {
        let mut out = self.output.lock().unwrap();
        let carry = out.dsr.take_carry();
        out.screen.process(&carry);
        out.buf.push(&carry);
        out.attached = true;
        out.generation += 1;
        out.generation
    }

    /// Only the latest connection may detach (a reconnect replaces the old socket).
    pub fn detach(&self, generation: u64) {
        let mut out = self.output.lock().unwrap();
        if out.generation == generation {
            out.attached = false;
        }
        drop(out);
        self.space.notify_all();
    }

    /// The client has everything before `from`.
    pub fn resume(&self, from: u64) {
        let mut out = self.output.lock().unwrap();
        out.acked = from.min(out.buf.end());
        drop(out);
        self.space.notify_all();
        self.notify.notify_one();
    }

    pub fn ack(&self, upto: u64) {
        let mut out = self.output.lock().unwrap();
        let upto = upto.min(out.buf.end());
        if upto > out.acked {
            out.acked = upto;
        }
        drop(out);
        self.space.notify_all();
    }

    pub fn pull(&self, cursor: u64, max: usize) -> Pull {
        let out = self.output.lock().unwrap();
        match out.buf.read_from(cursor, max) {
            None => Pull::Reset { seq: out.buf.end(), screen: out.screen.screen().state_formatted() },
            Some(bytes) if !bytes.is_empty() => Pull::Data { seq: cursor, bytes },
            Some(_) => match out.exit {
                // All output is out once the reader is done; on Windows EOF can be late, so give up after 500 ms.
                Some((code, at)) if out.reader_done || at.elapsed() > Duration::from_millis(500) => Pull::Exit(code),
                _ => Pull::Idle,
            },
        }
    }

    /// Bytes waiting after `cursor` (used for the 64 KiB flush rule).
    pub fn available(&self, cursor: u64) -> u64 {
        self.output.lock().unwrap().buf.end().saturating_sub(cursor)
    }

    pub fn notified(&self) -> tokio::sync::futures::Notified<'_> {
        self.notify.notified()
    }

    /// (end of stream, acked). For tests and the perf panel.
    pub fn counters(&self) -> (u64, u64) {
        let out = self.output.lock().unwrap();
        (out.buf.end(), out.acked)
    }

    /// Kills the whole process tree, not just the direct child.
    pub fn kill(&self) {
        if let Some(pid) = self.pid {
            kill_tree(pid);
        }
    }
}

#[cfg(windows)]
fn kill_tree(pid: u32) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

#[cfg(unix)]
fn kill_tree(pid: u32) {
    // portable-pty starts the child in its own session, so its pid is the process group id.
    unsafe {
        libc::killpg(pid as i32, libc::SIGHUP);
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(2));
        unsafe {
            libc::killpg(pid as i32, libc::SIGKILL);
        }
    });
}
```

`crates/core/src/pty/mod.rs`:
```rust
pub mod buffer;
pub mod dsr;
mod session;

pub use buffer::{FLUSH_BYTES, MAX_UNACKED, SCROLLBACK_BYTES};
pub use session::{Pull, Session, SpawnSpec};
```

`fake-agent spawn-child` starts its grandchild with plain `Command`, so on Unix the grandchild stays in the same process group and `killpg` reaches it. On Windows, `taskkill /T` walks the tree.

In `server/mod.rs`, make `RunningCore::shutdown` kill every session first:
```rust
    /// Kills every session's process tree, then stops the server.
    pub fn shutdown(self) {
        let sessions: Vec<_> = self.state.0.sessions.lock().unwrap().values().cloned().collect();
        for s in sessions {
            s.kill();
        }
        let _ = self.shutdown.send(());
    }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p marshell-core --test session -- --test-threads=1`

Expected: PASS (6 tests) on Windows. CI covers macOS and Linux in Task 10.

If `prints_and_exits_zero` hangs on Windows with no output, the DSR answer isn't reaching ConPTY. Print the first 32 raw bytes in `read_loop` to confirm that `ESC[6n` arrives.

- [ ] **Step 6: Commit**

```bash
git add crates/core
git commit -m "feat(core): pty Session with backpressure, DSR answer, tree kill; fake-agent"
```

---

### Task 6: Sessions API and the pty WebSocket

**Files:**
- Create: `crates/core/src/server/{frames.rs,sessions.rs,pty_ws.rs}`
- Modify: `crates/core/src/server/mod.rs` (routes, `AppState::session`, `AppState::insert_session`)
- Test: unit tests in `frames.rs`; integration tests in `crates/core/tests/pty_ws.rs`

**Interfaces:**
- Consumes: `Session`, `Pull` and `FLUSH_BYTES` (Task 5); `CreateSessionRequest/Response`, `Resize` and the frame opcodes (Task 1)
- Produces:
  - `POST /v1/sessions` (`CreateSessionRequest` → `CreateSessionResponse`; 500 with a text body on spawn failure)
  - `GET /v1/pty/{tab}` (WebSocket, subprotocol `marshell.v1`)
  - `server::frames::{encode_output, encode_reset, encode_exit, decode_client, ClientFrame}`

- [ ] **Step 1: Write the frame codec with its tests**

`crates/core/src/server/frames.rs`:
```rust
use marshell_protocol::api::Resize;
use marshell_protocol::frames as op;

pub fn encode_output(seq: u64, bytes: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(9 + bytes.len());
    f.push(op::OUTPUT);
    f.extend_from_slice(&seq.to_be_bytes());
    f.extend_from_slice(bytes);
    f
}

pub fn encode_reset(seq: u64, screen: &[u8]) -> Vec<u8> {
    let mut f = encode_output(seq, screen);
    f[0] = op::RESET;
    f
}

pub fn encode_exit(code: i32) -> Vec<u8> {
    let mut f = vec![op::EXIT];
    f.extend_from_slice(&code.to_be_bytes());
    f
}

#[derive(Debug, PartialEq)]
pub enum ClientFrame {
    Input(Vec<u8>),
    Resize(Resize),
    Ack(u64),
    Resume(u64),
}

/// Unknown or malformed frames return None and are ignored.
pub fn decode_client(frame: &[u8]) -> Option<ClientFrame> {
    let (&kind, rest) = frame.split_first()?;
    let u64_arg = |r: &[u8]| -> Option<u64> { Some(u64::from_be_bytes(r.get(..8)?.try_into().ok()?)) };
    match kind {
        op::INPUT => Some(ClientFrame::Input(rest.to_vec())),
        op::RESIZE => serde_json::from_slice(rest).ok().map(ClientFrame::Resize),
        op::ACK => u64_arg(rest).map(ClientFrame::Ack),
        op::RESUME => u64_arg(rest).map(ClientFrame::Resume),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_layout() {
        assert_eq!(encode_output(258, b"hi"), vec![1, 0, 0, 0, 0, 0, 0, 1, 2, b'h', b'i']);
    }

    #[test]
    fn exit_is_signed() {
        assert_eq!(encode_exit(-1), vec![2, 0xff, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn decodes_client_frames() {
        assert_eq!(decode_client(&[0x10, b'a']), Some(ClientFrame::Input(b"a".to_vec())));
        assert_eq!(decode_client(&[0x12, 0, 0, 0, 0, 0, 0, 0, 7]), Some(ClientFrame::Ack(7)));
        assert_eq!(decode_client(&[0x13, 0, 0, 0, 0, 0, 0, 1, 0]), Some(ClientFrame::Resume(256)));
        let mut resize = vec![0x11];
        resize.extend_from_slice(br#"{"cols":120,"rows":40}"#);
        assert_eq!(decode_client(&resize), Some(ClientFrame::Resize(Resize { cols: 120, rows: 40 })));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(decode_client(&[]), None);
        assert_eq!(decode_client(&[0x12, 1, 2]), None);
        assert_eq!(decode_client(&[0x11, b'{']), None);
        assert_eq!(decode_client(&[0x7f]), None);
    }
}
```

Add `pub mod frames;` to `server/mod.rs`, then run `cargo test -p marshell-core server::frames`.

Expected: PASS (4 tests). The codec is written whole here because its tests are the specification.

- [ ] **Step 2: Write the failing WebSocket integration tests**

`crates/core/tests/pty_ws.rs`:
```rust
use futures_util::{SinkExt, StreamExt};
use marshell_core::server::{start, CoreConfig, RunningCore};
use marshell_protocol::api::{CreateSessionRequest, CreateSessionResponse};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

async fn core() -> (RunningCore, tempfile::TempDir) {
    let home = tempfile::tempdir().unwrap();
    let core = start(CoreConfig { home: home.path().to_path_buf(), allowed_origins: vec![] }).await.unwrap();
    (core, home)
}

fn fake(args: &[&str]) -> Vec<String> {
    let mut v = vec![env!("CARGO_BIN_EXE_fake-agent").to_string()];
    v.extend(args.iter().map(|s| s.to_string()));
    v
}

async fn create(core: &RunningCore, shell: Vec<String>, cwd: Option<String>) -> Result<String, (u16, String)> {
    let url = format!("http://127.0.0.1:{}/v1/sessions", core.endpoint.port);
    let token = core.endpoint.token.clone();
    let body = CreateSessionRequest { shell: Some(shell), cwd, cols: 80, rows: 24 };
    tokio::task::spawn_blocking(move || {
        let res = ureq::post(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .config()
            .http_status_as_error(false)
            .build()
            .send_json(&body)
            .unwrap();
        let status = res.status().as_u16();
        let mut res = res;
        if status == 200 {
            Ok(res.body_mut().read_json::<CreateSessionResponse>().unwrap().tab_id)
        } else {
            Err((status, res.body_mut().read_to_string().unwrap_or_default()))
        }
    })
    .await
    .unwrap()
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(core: &RunningCore, tab: &str, resume_from: u64) -> Ws {
    let mut req = format!("ws://127.0.0.1:{}/v1/pty/{tab}", core.endpoint.port).into_client_request().unwrap();
    req.headers_mut()
        .insert("Sec-WebSocket-Protocol", format!("marshell.v1, token.{}", core.endpoint.token).parse().unwrap());
    let (mut ws, res) = tokio_tungstenite::connect_async(req).await.unwrap();
    assert_eq!(res.headers()["sec-websocket-protocol"], "marshell.v1");
    let mut resume = vec![0x13];
    resume.extend_from_slice(&resume_from.to_be_bytes());
    ws.send(Message::Binary(resume.into())).await.unwrap();
    ws
}

struct Received {
    bytes: Vec<u8>,
    exit: Option<i32>,
    end: u64,
    resets: usize,
}

/// Reads frames, acking each, until EXIT, `stop` matches, or the timeout passes.
async fn read_until(ws: &mut Ws, timeout: Duration, stop: impl Fn(&[u8]) -> bool) -> Received {
    let mut r = Received { bytes: vec![], exit: None, end: 0, resets: 0 };
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline && !stop(&r.bytes) {
        let Ok(Some(Ok(Message::Binary(f)))) = tokio::time::timeout(Duration::from_millis(200), ws.next()).await else {
            continue;
        };
        let seq = || u64::from_be_bytes(f[1..9].try_into().unwrap());
        match f[0] {
            0x01 => {
                r.bytes.extend_from_slice(&f[9..]);
                r.end = seq() + (f.len() - 9) as u64;
            }
            0x03 => {
                r.resets += 1;
                r.bytes.extend_from_slice(&f[9..]);
                r.end = seq();
            }
            0x02 => {
                r.exit = Some(i32::from_be_bytes(f[1..5].try_into().unwrap()));
                break;
            }
            _ => {}
        }
        let mut ack = vec![0x12];
        ack.extend_from_slice(&r.end.to_be_bytes());
        ws.send(Message::Binary(ack.into())).await.unwrap();
    }
    r
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[tokio::test]
async fn echo_round_trip_and_exit() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["echo"]), None).await.unwrap();
    let mut ws = connect(&core, &tab, 0).await;
    let mut input = vec![0x10];
    input.extend_from_slice(b"ping\r");
    ws.send(Message::Binary(input.into())).await.unwrap();
    let got = read_until(&mut ws, Duration::from_secs(10), |b| text(b).contains("got:ping")).await;
    assert!(text(&got.bytes).contains("got:ping"));
    let mut input = vec![0x10];
    input.extend_from_slice(b"exit\r");
    ws.send(Message::Binary(input.into())).await.unwrap();
    let rest = read_until(&mut ws, Duration::from_secs(10), |_| false).await;
    assert_eq!(rest.exit, Some(0));
}

#[tokio::test]
async fn cwd_with_space() {
    let (core, _home) = core().await;
    let dir = tempfile::Builder::new().prefix("has space ").tempdir().unwrap();
    let tab = create(&core, fake(&["cwd"]), Some(dir.path().display().to_string())).await.unwrap();
    let mut ws = connect(&core, &tab, 0).await;
    let got = read_until(&mut ws, Duration::from_secs(10), |_| false).await;
    let name = dir.path().file_name().unwrap().to_string_lossy().into_owned();
    assert!(text(&got.bytes).contains(&name), "cwd output: {}", text(&got.bytes));
}

#[tokio::test]
async fn utf8_passthrough() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["print", "héllo ✓ 日本"]), None).await.unwrap();
    let mut ws = connect(&core, &tab, 0).await;
    let got = read_until(&mut ws, Duration::from_secs(10), |_| false).await;
    assert!(text(&got.bytes).contains("héllo ✓ 日本"), "got: {:?}", text(&got.bytes));
}

#[tokio::test]
async fn bad_shell_is_an_error() {
    let (core, _home) = core().await;
    let err = create(&core, vec!["definitely-not-a-shell-xyz".into()], None).await.unwrap_err();
    assert!(err.0 >= 400, "status {}", err.0);
    assert!(!err.1.is_empty());
}

#[tokio::test]
async fn reconnect_resumes_without_duplicates() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["echo"]), None).await.unwrap();
    let mut ws = connect(&core, &tab, 0).await;
    let mut input = vec![0x10];
    input.extend_from_slice(b"one\r");
    ws.send(Message::Binary(input.into())).await.unwrap();
    let first = read_until(&mut ws, Duration::from_secs(10), |b| text(b).contains("got:one")).await;
    drop(ws);
    let mut ws = connect(&core, &tab, first.end).await;
    let mut input = vec![0x10];
    input.extend_from_slice(b"two\r");
    ws.send(Message::Binary(input.into())).await.unwrap();
    let second = read_until(&mut ws, Duration::from_secs(10), |b| text(b).contains("got:two")).await;
    assert!(!text(&second.bytes).contains("got:one"), "replayed output that was already acked");
    assert!(text(&second.bytes).contains("got:two"));
}

#[tokio::test]
async fn evicted_cursor_gets_a_reset() {
    let (core, _home) = core().await;
    // 9 MiB with nobody attached overflows the 8 MiB ring.
    let tab = create(&core, fake(&["flood", "9437184"]), None).await.unwrap();
    tokio::time::sleep(Duration::from_secs(4)).await;
    let mut ws = connect(&core, &tab, 0).await;
    let got = read_until(&mut ws, Duration::from_secs(20), |_| false).await;
    assert!(got.resets >= 1);
    assert_eq!(got.exit, Some(0));
}

/// S6 throughput. Run with: cargo test -p marshell-core --release --test pty_ws -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn s6_throughput_50mb() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["flood", "52428800"]), None).await.unwrap();
    let started = Instant::now();
    let mut ws = connect(&core, &tab, 0).await;
    let got = read_until(&mut ws, Duration::from_secs(120), |_| false).await;
    let secs = started.elapsed().as_secs_f64();
    println!(
        "S6: {} bytes in {secs:.2}s = {:.1} MB/s, exit {:?}",
        got.bytes.len(),
        got.bytes.len() as f64 / secs / 1e6,
        got.exit
    );
    assert_eq!(got.exit, Some(0));
    assert!(got.bytes.len() >= 50 * 1024 * 1024 - 1024);
}
```

The ureq 3 calls above are `.config().http_status_as_error(false).build()`, `.body_mut().read_json()` and `.read_to_string()`. If any name differs, check `cargo doc -p ureq` and fix it in `create` only.

Run: `cargo test -p marshell-core --test pty_ws`

Expected: FAIL; the routes return 404.

- [ ] **Step 3: Implement session creation**

In `server/mod.rs`, add the state helpers and routes:
```rust
mod pty_ws;
mod sessions;

impl AppState {
    pub fn session(&self, tab: &str) -> Option<Arc<crate::pty::Session>> {
        self.0.sessions.lock().unwrap().get(tab).cloned()
    }
    pub fn insert_session(&self, session: Arc<crate::pty::Session>) {
        self.0.sessions.lock().unwrap().insert(session.id().to_string(), session);
    }
}

// in router(), before the .layer(...) calls:
        .route("/v1/sessions", axum::routing::post(sessions::create))
        .route("/v1/pty/{tab}", get(pty_ws::handler))
```

`crates/core/src/server/sessions.rs`:
```rust
use super::AppState;
use crate::pty::{Session, SpawnSpec};
use crate::{auth, shell};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use marshell_protocol::api::{CreateSessionRequest, CreateSessionResponse};
use marshell_protocol::brand;
use std::path::PathBuf;

pub async fn create(
    State(st): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>, (StatusCode, String)> {
    let tab_id = auth::new_id("t");
    let spec = SpawnSpec {
        argv: req.shell.unwrap_or_else(shell::default_shell),
        cwd: req.cwd.map(PathBuf::from).or_else(dirs::home_dir),
        env: vec![
            (brand::env_var("TAB_ID"), tab_id.clone()),
            (brand::env_var("AGENT"), "shell".into()),
            ("TERM".into(), "xterm-256color".into()),
            ("COLORTERM".into(), "truecolor".into()),
        ],
        cols: req.cols,
        rows: req.rows,
    };
    let id = tab_id.clone();
    // Spawning touches the OS (process creation): keep it off the async worker threads.
    let session = tokio::task::spawn_blocking(move || Session::spawn(id, spec))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("could not start the shell: {e:#}")))?;
    st.insert_session(session);
    Ok(Json(CreateSessionResponse { tab_id }))
}
```

- [ ] **Step 4: Implement the WebSocket**

`crates/core/src/server/pty_ws.rs`:
```rust
use super::frames::{decode_client, encode_exit, encode_output, encode_reset, ClientFrame};
use super::AppState;
use crate::pty::{Pull, Session, FLUSH_BYTES};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use marshell_protocol::brand;
use std::sync::Arc;
use std::time::{Duration, Instant};

const FLUSH_INTERVAL: Duration = Duration::from_millis(8);

pub async fn handler(State(st): State<AppState>, Path(tab): Path<String>, ws: WebSocketUpgrade) -> Response {
    let Some(session) = st.session(&tab) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // Echo our subprotocol, or browsers drop the connection.
    ws.protocols([brand::WS_SUBPROTOCOL]).on_upgrade(move |socket| run(socket, session))
}

async fn run(socket: WebSocket, session: Arc<Session>) {
    let generation = session.attach();
    let (mut tx, mut rx) = socket.split();
    // None until the client sends RESUME.
    let mut cursor: Option<u64> = None;
    let mut last_flush = Instant::now();

    loop {
        tokio::select! {
            msg = rx.next() => match msg {
                Some(Ok(Message::Binary(frame))) => match decode_client(&frame) {
                    Some(ClientFrame::Input(bytes)) => session.write_input(bytes),
                    Some(ClientFrame::Resize(r)) => { let _ = session.resize(r.cols, r.rows); }
                    Some(ClientFrame::Ack(n)) => session.ack(n),
                    Some(ClientFrame::Resume(from)) => { session.resume(from); cursor = Some(from); }
                    None => {}
                },
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            },
            _ = session.notified(), if cursor.is_some() => {}
            _ = tokio::time::sleep(FLUSH_INTERVAL), if cursor.is_some() => {}
        }

        let Some(mut c) = cursor else { continue };
        // Coalesce: flush when 64 KiB is waiting or 8 ms have passed since the last flush.
        if session.available(c) < FLUSH_BYTES as u64 && last_flush.elapsed() < FLUSH_INTERVAL {
            continue;
        }
        let mut exited = false;
        loop {
            let frame = match session.pull(c, FLUSH_BYTES) {
                Pull::Data { seq, bytes } => {
                    c = seq + bytes.len() as u64;
                    encode_output(seq, &bytes)
                }
                Pull::Reset { seq, screen } => {
                    c = seq;
                    encode_reset(seq, &screen)
                }
                Pull::Exit(code) => {
                    exited = true;
                    encode_exit(code)
                }
                Pull::Idle => break,
            };
            if tx.send(Message::Binary(frame.into())).await.is_err() {
                exited = true;
                break;
            }
            if exited {
                break;
            }
        }
        cursor = Some(c);
        last_flush = Instant::now();
        if exited {
            break;
        }
    }
    session.detach(generation);
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p marshell-core --test pty_ws`

Expected: PASS (6 tests; S6 is ignored).

Run: `cargo test -p marshell-core --release --test pty_ws -- --ignored --nocapture`

Expected: PASS. It prints the `S6:` throughput line; keep it for Task 11.

- [ ] **Step 6: Commit**

```bash
git add crates/core
git commit -m "feat(core): sessions API and pty WebSocket with coalescing, ack, resume, reset"
```

---

### Task 7: S7, FTS5 through rusqlite's bundled build

**Files:**
- Test: `crates/core/tests/fts5.rs`
- Create: `docs/spikes/S7-fts5.md`

- [ ] **Step 1: Write the test**

`crates/core/tests/fts5.rs`:
```rust
#[test]
fn bundled_sqlite_has_fts5() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE VIRTUAL TABLE t USING fts5(body);
         INSERT INTO t(body) VALUES ('fixed the login button'), ('ran npm test');",
    )
    .unwrap();
    let hit: String = db
        .query_row("SELECT body FROM t WHERE t MATCH 'login'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(hit, "fixed the login button");
    let snippet: String = db
        .query_row("SELECT snippet(t, 0, '[', ']', '…', 4) FROM t WHERE t MATCH 'npm'", [], |r| r.get(0))
        .unwrap();
    assert!(snippet.contains("[npm]"));
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p marshell-core --test fts5`

Expected: PASS on Windows. CI confirms macOS and Linux (Task 10).

- [ ] **Step 3: Record the result**

`docs/spikes/S7-fts5.md`:
```markdown
# S7: FTS5 via rusqlite bundled

Pass criterion: an FTS5 table is created and matched (with snippet()) on Windows, macOS and Linux.

| OS | Result | Evidence |
|---|---|---|
| Windows 11 (dev box) | PASS | `cargo test -p marshell-core --test fts5`, <date> |
| macOS (CI) | | CI run link |
| Linux (CI) | | CI run link |
```
Fill in the date now; fill in the CI rows after Task 10.

- [ ] **Step 4: Commit**

```bash
git add crates/core/tests/fts5.rs docs/spikes/S7-fts5.md
git commit -m "test(spike): S7 FTS5 available through rusqlite bundled"
```

---

### Task 8: Frontend toolchain, tokens, frame codec, pty socket, latency meter

**Files:**
- Create: `package.json`, `tsconfig.json`, `vite.config.ts`, `index.html`
- Create: `src/styles/{tokens.css,base.css,app.css}`
- Create: `src/lib/api/{frames.ts,frames.test.ts,ptySocket.ts,session.ts}`
- Create: `src/features/perf/{latency.ts,latency.test.ts}`

**Interfaces:**
- Consumes: `src/generated/*` (Task 1)
- Produces:
  - `decodeServerFrame(buf: ArrayBuffer): ServerFrame | null`
  - `encodeInput(data: string | Uint8Array): Uint8Array`
  - `encodeBinaryString(data: string): Uint8Array`
  - `encodeResize(size: Resize): Uint8Array`
  - `encodeAck(n: number): Uint8Array`, `encodeResume(n: number): Uint8Array`
  - `class PtySocket(endpoint, tabId, handlers)` with `sendInput`, `sendBytes`, `resize`, `processed(upTo)`, `close()`
  - `startSession(): Promise<StartResult>`
  - `class LatencyMeter` with `markInput()`, `markOutput()`, `record(ms)`, `stats()`

- [ ] **Step 1: Create the package and install dependencies**

`package.json`:
```json
{
  "name": "marshell",
  "private": true,
  "version": "0.1.0",
  "license": "MIT",
  "type": "module",
  "packageManager": "pnpm@12.10.1",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "test": "vitest run",
    "tauri": "tauri",
    "check:generated": "node scripts/check-generated.mjs"
  }
}
```

Run:
```bash
pnpm add react react-dom @tauri-apps/api @xterm/xterm @xterm/addon-fit @xterm/addon-webgl
pnpm add -D @tauri-apps/cli typescript vite @vitejs/plugin-react vitest @types/react @types/react-dom
```

Expected majors: react 19, @tauri-apps/api 2.12, @xterm/xterm 6, addon-fit 0.11, addon-webgl 0.19, typescript 7, vite 8, vitest 5. If `tsc` from TypeScript 7 rejects the tsconfig below, use `pnpm add -D typescript@6` and note it in the commit message.

`tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "isolatedModules": true,
    "skipLibCheck": true,
    "types": ["vite/client"]
  },
  "include": ["src"]
}
```

`vite.config.ts`:
```ts
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri expects a fixed dev port (tauri.conf.json devUrl).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022" },
  test: { environment: "node" },
});
```

`index.html`:
```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta name="color-scheme" content="light dark" />
    <title>Marshell</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 2: Write the tokens and base CSS**

Phase 1 needs only a subset; phase 0 refines them. Values come from the "UX and UI design" section of `docs/PLAN.md`, converted to OKLCH.

`src/styles/tokens.css`:
```css
:root {
  color-scheme: light dark;

  /* Surfaces: true black in dark, white and warm grays in light. */
  --bg-base: light-dark(oklch(100% 0 none), oklch(0% 0 none));
  --bg-raised: light-dark(oklch(98.2% 0.003 85), oklch(14.5% 0 none));
  --bg-overlay: light-dark(oklch(95.6% 0.004 75), oklch(19.1% 0 none));

  --text-1: light-dark(oklch(21.6% 0.006 56), oklch(92.2% 0 none));
  --text-2: light-dark(oklch(44.4% 0.011 74), oklch(71.9% 0 none));
  --text-3: light-dark(oklch(55.3% 0.013 58), oklch(63.3% 0 none));

  --hairline: color-mix(in oklch, var(--text-1) 10%, transparent);
  --hover: color-mix(in oklch, var(--text-1) 4%, transparent);
  --selected: color-mix(in oklch, var(--text-1) 7%, transparent);

  /* User-chosen accent (phase 2 makes it configurable). */
  --accent: oklch(62% 0.19 255);

  --error: light-dark(oklch(56% 0.22 27), oklch(67% 0.21 29));
  --ok: light-dark(oklch(55% 0.14 147), oklch(76.5% 0.19 147));
  --caution: light-dark(oklch(56% 0.14 52), oklch(88.5% 0.18 95));

  --font-ui: "Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, "Inter Variable", sans-serif;
  --font-mono: "Cascadia Mono", "JetBrains Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace;

  /* Desktop chrome uses fixed sizes; 13 px is the base. */
  --text-11: 0.6875rem;
  --text-12: 0.75rem;
  --text-13: 0.8125rem;
  --text-15: 0.9375rem;
  --text-20: 1.25rem;
  --text-28: 1.75rem;
  --leading: 1.5;

  /* 8 px grid. */
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-6: 24px;
  --space-8: 32px;

  --radius: 6px;
  --ease-out: cubic-bezier(0.2, 0.8, 0.2, 1);
  --dur-fast: 120ms;
  --dur: 180ms;
  --dur-slow: 200ms;
}

[data-theme="light"] {
  color-scheme: light;
}
[data-theme="dark"] {
  color-scheme: dark;
}
```

`src/styles/base.css`:
```css
*,
*::before,
*::after {
  box-sizing: border-box;
  min-width: 0;
}

:root {
  font-synthesis: none;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
  -webkit-tap-highlight-color: transparent;
}

html {
  overscroll-behavior-y: none;
}

body {
  margin: 0;
  block-size: 100dvh;
  background: var(--bg-base);
  color: var(--text-1);
  font-family: var(--font-ui);
  font-size: var(--text-13);
  line-height: var(--leading);
}

:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
```

`src/styles/app.css`:
```css
#root {
  block-size: 100dvh;
}

.terminal-host {
  block-size: 100%;
  inline-size: 100%;
  padding: var(--space-2);
  background: var(--bg-base);
  overflow: clip;
}

.startup-error {
  margin: 0;
  padding: var(--space-6);
  color: var(--error);
}

.perf-overlay {
  position: fixed;
  inset-block-start: var(--space-2);
  inset-inline-end: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
  background: var(--bg-overlay);
  color: var(--text-2);
  font-family: var(--font-mono);
  font-size: var(--text-11);
  font-variant-numeric: tabular-nums;
}
```

- [ ] **Step 3: Write the failing codec and latency tests**

`src/lib/api/frames.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { decodeServerFrame, encodeAck, encodeBinaryString, encodeInput, encodeResize, encodeResume } from "./frames";

function buf(bytes: number[]): ArrayBuffer {
  return new Uint8Array(bytes).buffer;
}

describe("decodeServerFrame", () => {
  it("decodes output with a u64 big-endian seq", () => {
    const f = decodeServerFrame(buf([1, 0, 0, 0, 0, 0, 0, 1, 2, 104, 105]));
    expect(f).toEqual({ kind: "output", seq: 258, bytes: new Uint8Array([104, 105]) });
  });
  it("decodes reset", () => {
    expect(decodeServerFrame(buf([3, 0, 0, 0, 0, 0, 0, 0, 9, 65]))).toEqual({
      kind: "reset",
      seq: 9,
      screen: new Uint8Array([65]),
    });
  });
  it("decodes a negative exit code", () => {
    expect(decodeServerFrame(buf([2, 255, 255, 255, 255]))).toEqual({ kind: "exit", code: -1 });
  });
  it("rejects short and unknown frames", () => {
    expect(decodeServerFrame(buf([]))).toBeNull();
    expect(decodeServerFrame(buf([1, 0, 0]))).toBeNull();
    expect(decodeServerFrame(buf([2, 0]))).toBeNull();
    expect(decodeServerFrame(buf([99]))).toBeNull();
  });
});

describe("client frames", () => {
  it("encodes input as UTF-8", () => {
    expect(Array.from(encodeInput("é"))).toEqual([0x10, 0xc3, 0xa9]);
  });
  it("encodes xterm binary strings byte for byte", () => {
    expect(Array.from(encodeBinaryString("\x00\xff"))).toEqual([0x10, 0x00, 0xff]);
  });
  it("encodes resize as JSON", () => {
    const f = encodeResize({ cols: 120, rows: 40 });
    expect(f[0]).toBe(0x11);
    expect(new TextDecoder().decode(f.slice(1))).toBe('{"cols":120,"rows":40}');
  });
  it("encodes ack and resume as u64 big-endian", () => {
    expect(Array.from(encodeAck(258))).toEqual([0x12, 0, 0, 0, 0, 0, 0, 1, 2]);
    expect(Array.from(encodeResume(0))).toEqual([0x13, 0, 0, 0, 0, 0, 0, 0, 0]);
  });
});
```

`src/features/perf/latency.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { LatencyMeter } from "./latency";

describe("LatencyMeter", () => {
  it("reports percentiles of recorded samples", () => {
    const m = new LatencyMeter();
    for (let i = 1; i <= 100; i++) m.record(i);
    expect(m.stats()).toEqual({ p50: 50, p95: 95, n: 100 });
  });
  it("keeps only the last 200 samples", () => {
    const m = new LatencyMeter();
    for (let i = 0; i < 300; i++) m.record(1);
    expect(m.stats().n).toBe(200);
  });
  it("is empty before any input", () => {
    expect(new LatencyMeter().stats()).toEqual({ p50: 0, p95: 0, n: 0 });
  });
});
```

Run: `pnpm test`

Expected: FAIL (the modules don't exist).

- [ ] **Step 4: Implement the codec, the socket, session start and the latency meter**

`src/lib/api/frames.ts`:
```ts
import { FRAME } from "../../generated/constants";
import type { Resize } from "../../generated/Resize";

export type ServerFrame =
  | { kind: "output"; seq: number; bytes: Uint8Array }
  | { kind: "reset"; seq: number; screen: Uint8Array }
  | { kind: "exit"; code: number };

export function decodeServerFrame(buf: ArrayBuffer): ServerFrame | null {
  if (buf.byteLength < 1) return null;
  const view = new DataView(buf);
  const op = view.getUint8(0);
  if (op === FRAME.OUTPUT || op === FRAME.RESET) {
    if (buf.byteLength < 9) return null;
    const seq = Number(view.getBigUint64(1));
    const bytes = new Uint8Array(buf, 9);
    return op === FRAME.OUTPUT ? { kind: "output", seq, bytes } : { kind: "reset", seq, screen: bytes };
  }
  if (op === FRAME.EXIT) {
    if (buf.byteLength < 5) return null;
    return { kind: "exit", code: view.getInt32(1) };
  }
  return null;
}

function withOp(op: number, payload: Uint8Array): Uint8Array {
  const out = new Uint8Array(1 + payload.length);
  out[0] = op;
  out.set(payload, 1);
  return out;
}

export function encodeInput(data: string | Uint8Array): Uint8Array {
  return withOp(FRAME.INPUT, typeof data === "string" ? new TextEncoder().encode(data) : data);
}

/** xterm's onBinary gives a string where each char is one byte (e.g. mouse reports). */
export function encodeBinaryString(data: string): Uint8Array {
  return withOp(FRAME.INPUT, Uint8Array.from(data, (c) => c.charCodeAt(0) & 0xff));
}

export function encodeResize(size: Resize): Uint8Array {
  return withOp(FRAME.RESIZE, new TextEncoder().encode(JSON.stringify(size)));
}

function encodeU64(op: number, n: number): Uint8Array {
  const out = new Uint8Array(9);
  const view = new DataView(out.buffer);
  view.setUint8(0, op);
  view.setBigUint64(1, BigInt(n));
  return out;
}

export const encodeAck = (n: number) => encodeU64(FRAME.ACK, n);
export const encodeResume = (n: number) => encodeU64(FRAME.RESUME, n);
```

`src/lib/api/ptySocket.ts`:
```ts
import { TOKEN_SUBPROTOCOL_PREFIX, WS_SUBPROTOCOL } from "../../generated/constants";
import type { Endpoint } from "../../generated/Endpoint";
import { decodeServerFrame, encodeAck, encodeBinaryString, encodeInput, encodeResize, encodeResume } from "./frames";

export interface PtyHandlers {
  onOutput(seq: number, bytes: Uint8Array): void;
  onReset(seq: number, screen: Uint8Array): void;
  onExit(code: number): void;
}

const ACK_EVERY_BYTES = 64 * 1024;
const ACK_DELAY_MS = 16;

/** One terminal's connection to the core. Reconnects with backoff and resumes where the renderer left off. */
export class PtySocket {
  private ws: WebSocket | null = null;
  private processedUpTo = 0;
  private lastAckSent = 0;
  private ackTimer: ReturnType<typeof setTimeout> | null = null;
  private retries = 0;
  private done = false;

  constructor(
    private readonly endpoint: Endpoint,
    private readonly tabId: string,
    private readonly handlers: PtyHandlers,
  ) {
    this.connect();
  }

  private connect(): void {
    const url = `ws://127.0.0.1:${this.endpoint.port}/v1/pty/${encodeURIComponent(this.tabId)}`;
    const ws = new WebSocket(url, [WS_SUBPROTOCOL, TOKEN_SUBPROTOCOL_PREFIX + this.endpoint.token]);
    ws.binaryType = "arraybuffer";
    ws.onopen = () => {
      this.retries = 0;
      ws.send(encodeResume(this.processedUpTo));
    };
    ws.onmessage = (ev) => {
      if (!(ev.data instanceof ArrayBuffer)) return;
      const frame = decodeServerFrame(ev.data);
      if (!frame) return;
      if (frame.kind === "output") this.handlers.onOutput(frame.seq, frame.bytes);
      else if (frame.kind === "reset") this.handlers.onReset(frame.seq, frame.screen);
      else {
        this.done = true;
        this.handlers.onExit(frame.code);
      }
    };
    ws.onclose = () => {
      this.ws = null;
      if (this.done) return;
      const delay = Math.min(2000, 100 * 2 ** this.retries++);
      setTimeout(() => this.connect(), delay);
    };
    this.ws = ws;
  }

  private send(frame: Uint8Array): void {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(frame);
  }

  sendInput(data: string): void {
    this.send(encodeInput(data));
  }

  sendBytes(binary: string): void {
    this.send(encodeBinaryString(binary));
  }

  resize(cols: number, rows: number): void {
    if (cols > 0 && rows > 0) this.send(encodeResize({ cols, rows }));
  }

  /** The renderer has fully processed output before this byte offset. Acks are batched. */
  processed(upTo: number): void {
    this.processedUpTo = upTo;
    if (upTo - this.lastAckSent >= ACK_EVERY_BYTES) {
      this.flushAck();
    } else if (this.ackTimer === null) {
      this.ackTimer = setTimeout(() => this.flushAck(), ACK_DELAY_MS);
    }
  }

  private flushAck(): void {
    if (this.ackTimer !== null) clearTimeout(this.ackTimer);
    this.ackTimer = null;
    this.lastAckSent = this.processedUpTo;
    this.send(encodeAck(this.processedUpTo));
  }

  close(): void {
    this.done = true;
    if (this.ackTimer !== null) clearTimeout(this.ackTimer);
    this.ws?.close();
  }
}
```

`src/lib/api/session.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";
import type { CreateSessionRequest } from "../../generated/CreateSessionRequest";
import type { CreateSessionResponse } from "../../generated/CreateSessionResponse";
import type { Endpoint } from "../../generated/Endpoint";

export type StartResult =
  | { kind: "ready"; endpoint: Endpoint; tabId: string }
  | { kind: "error"; message: string };

let pending: Promise<StartResult> | null = null;

/** Creates the phase 1 tab once, even if React runs effects twice in development. */
export function startSession(): Promise<StartResult> {
  pending ??= create();
  return pending;
}

async function create(): Promise<StartResult> {
  let endpoint: Endpoint;
  try {
    endpoint = await invoke<Endpoint>("core_endpoint");
  } catch (e) {
    return { kind: "error", message: `Couldn't reach the core: ${String(e)}` };
  }
  const body: CreateSessionRequest = { cols: 80, rows: 24 };
  try {
    const res = await fetch(`http://127.0.0.1:${endpoint.port}/v1/sessions`, {
      method: "POST",
      headers: { Authorization: `Bearer ${endpoint.token}`, "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) return { kind: "error", message: `Couldn't start a shell. ${await res.text()}` };
    const { tab_id } = (await res.json()) as CreateSessionResponse;
    return { kind: "ready", endpoint, tabId: tab_id };
  } catch (e) {
    return { kind: "error", message: `Couldn't start a shell. ${String(e)}` };
  }
}
```

`src/features/perf/latency.ts`:
```ts
const MAX_SAMPLES = 200;

/** Keypress → output parsed → next frame, in ms. Measured for the phase 1 typing gate (< 30 ms). */
export class LatencyMeter {
  private pendingSince: number | null = null;
  private samples: number[] = [];

  markInput(): void {
    this.pendingSince ??= performance.now();
  }

  /** Call when output was parsed; the sample closes on the next animation frame (the paint). */
  markOutput(): void {
    const since = this.pendingSince;
    if (since === null) return;
    this.pendingSince = null;
    requestAnimationFrame(() => this.record(performance.now() - since));
  }

  record(ms: number): void {
    this.samples.push(ms);
    if (this.samples.length > MAX_SAMPLES) this.samples.shift();
  }

  stats(): { p50: number; p95: number; n: number } {
    const n = this.samples.length;
    if (n === 0) return { p50: 0, p95: 0, n: 0 };
    const sorted = [...this.samples].sort((a, b) => a - b);
    const at = (p: number) => sorted[Math.min(n - 1, Math.ceil((p / 100) * n) - 1)] ?? 0;
    return { p50: at(50), p95: at(95), n };
  }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `pnpm test`

Expected: PASS (11 tests).

- [ ] **Step 6: Commit**

```bash
git add package.json pnpm-lock.yaml tsconfig.json vite.config.ts index.html src/styles src/lib src/features/perf
git commit -m "feat(ui): tokens, frame codec, pty socket, latency meter"
```

---

### Task 9: Tauri shell and the live terminal window

**Files:**
- Create: `src-tauri/{Cargo.toml,build.rs,tauri.conf.json,capabilities/default.json,src/main.rs}`, `src-tauri/icons/*` (generated), `assets/icon/placeholder.svg`
- Create: `src/main.tsx`, `src/App.tsx`, `src/features/terminal/{TerminalView.tsx,renderer.ts,theme.ts}`, `src/features/perf/PerfOverlay.tsx`
- Modify: `Cargo.toml` (add `src-tauri` to `members`)

**Interfaces:**
- Consumes:
  - `marshell_core::server::{start, CoreConfig, RunningCore}` and `marshell_core::paths::home_dir` (Tasks 2–6)
  - `PtySocket`, `startSession` and `LatencyMeter` (Task 8)
- Produces:
  - Tauri command `core_endpoint() -> Endpoint`
  - Env var `MARSHELL_LINUX_GFX` = `no-dmabuf` | `no-compositing` | `both`, which spike S1 uses

- [ ] **Step 1: Create the shell crate**

`src-tauri/Cargo.toml`:
```toml
[package]
name = "marshell-app"
version.workspace = true
edition.workspace = true
license.workspace = true

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
marshell-core.workspace = true
marshell-protocol.workspace = true
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true
tauri = { version = "2", features = [] }
tauri-plugin-single-instance = "2"
tauri-plugin-window-state = "2"
```

`src-tauri/build.rs`:
```rust
fn main() {
    tauri_build::build()
}
```

`src-tauri/tauri.conf.json`:
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Marshell",
  "version": "0.1.0",
  "identifier": "dev.marshell.app",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "Marshell",
        "width": 1200,
        "height": 780,
        "minWidth": 720,
        "minHeight": 480,
        "transparent": false
      }
    ],
    "security": {
      "csp": "default-src 'self'; connect-src 'self' ipc: http://ipc.localhost http://127.0.0.1:* ws://127.0.0.1:*; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]
  }
}
```

`src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "windows": ["main"],
  "permissions": ["core:default", "window-state:default"]
}
```

`assets/icon/placeholder.svg` is a stand-in until phase 0 delivers the real plane icon:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <rect width="1024" height="1024" rx="224" fill="#111"/>
  <path d="M512 200 L560 440 L800 520 L800 580 L560 540 L540 760 L620 820 L620 860 L512 830 L404 860 L404 820 L484 760 L464 540 L224 580 L224 520 L464 440 Z" fill="#f2f2f2"/>
</svg>
```

Run: `pnpm tauri icon assets/icon/placeholder.svg`

Expected: `src-tauri/icons/` is filled with every platform size.

- [ ] **Step 2: Write main.rs**

`src-tauri/src/main.rs`:
```rust
// No console window behind the app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use marshell_core::server::{CoreConfig, RunningCore};
use marshell_protocol::api::Endpoint;
use marshell_protocol::brand;
use std::sync::Mutex;
use tauri::Manager;

/// Keeps the core alive for the app's lifetime; taken on exit to shut it down.
struct CoreHandle(Mutex<Option<RunningCore>>);

#[tauri::command]
fn core_endpoint(core: tauri::State<'_, CoreHandle>) -> Result<Endpoint, String> {
    core.0
        .lock()
        .unwrap()
        .as_ref()
        .map(|c| c.endpoint.clone())
        .ok_or_else(|| "the core is not running".to_string())
}

fn allowed_origins() -> Vec<String> {
    let mut origins = vec![
        "tauri://localhost".to_string(),
        "http://tauri.localhost".to_string(),
        "https://tauri.localhost".to_string(),
    ];
    if cfg!(debug_assertions) {
        origins.push("http://localhost:1420".to_string());
    }
    origins
}

/// Spike S1 tries WebKitGTK workarounds one at a time through MARSHELL_LINUX_GFX.
/// Never overrides a value the user already set.
fn linux_webview_workarounds() {
    #[cfg(target_os = "linux")]
    {
        let mode = std::env::var(brand::env_var("LINUX_GFX")).unwrap_or_default();
        let vars: &[&str] = match mode.as_str() {
            "no-dmabuf" => &["WEBKIT_DISABLE_DMABUF_RENDERER"],
            "no-compositing" => &["WEBKIT_DISABLE_COMPOSITING_MODE"],
            "both" => &["WEBKIT_DISABLE_DMABUF_RENDERER", "WEBKIT_DISABLE_COMPOSITING_MODE"],
            _ => &[],
        };
        for v in vars {
            if std::env::var_os(v).is_none() {
                // SAFETY: runs first thing in main, before any other thread exists.
                unsafe { std::env::set_var(v, "1") };
            }
        }
    }
}

fn main() {
    linux_webview_workarounds();
    let app = tauri::Builder::default()
        // Must be the first plugin: a second launch focuses the existing window instead.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            let home = marshell_core::paths::home_dir()?;
            let cfg = CoreConfig { home, allowed_origins: allowed_origins() };
            let core = tauri::async_runtime::block_on(marshell_core::server::start(cfg))?;
            app.manage(CoreHandle(Mutex::new(Some(core))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![core_endpoint])
        .build(tauri::generate_context!())
        .expect("failed to build the app");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            if let Some(core) = handle.state::<CoreHandle>().0.lock().unwrap().take() {
                core.shutdown();
            }
        }
    });
}
```

Add `"src-tauri"` back to the workspace `members`.

On non-Linux builds, `brand` is unused in `main.rs` because it is only used inside the `#[cfg(target_os = "linux")]` block. Add `#[cfg_attr(not(target_os = "linux"), allow(unused_imports))]` above that `use`.

Run: `cargo build -p marshell-app`

Expected: builds.

- [ ] **Step 3: Write the terminal view**

`src/features/terminal/theme.ts`:
```ts
import type { ITheme } from "@xterm/xterm";

// Phase 1: the terminal follows the app's light/dark theme. Phase 2 adds per-profile themes.
const dark: ITheme = { background: "#000000", foreground: "#e5e5e5", cursor: "#e5e5e5", selectionBackground: "#3a3a3a" };
const light: ITheme = { background: "#ffffff", foreground: "#1c1917", cursor: "#1c1917", selectionBackground: "#d6d3d1" };

export function currentTheme(): ITheme {
  return matchMedia("(prefers-color-scheme: dark)").matches ? dark : light;
}
```

`src/features/terminal/renderer.ts`:
```ts
import { WebglAddon } from "@xterm/addon-webgl";
import type { Terminal } from "@xterm/xterm";
import type { Platform } from "../../generated/Platform";

export type RendererKind = "webgl" | "dom";

/** WebGL when available; xterm's DOM renderer otherwise (xterm 6 has no canvas renderer). */
export function loadRenderer(term: Terminal, platform: Platform, onChange: (kind: RendererKind) => void): void {
  try {
    // WebKitGTK shows a WebGL canvas one frame behind unless the drawing buffer is preserved (WebKit bug 324549).
    const webgl = new WebglAddon(platform === "linux");
    webgl.onContextLoss(() => {
      webgl.dispose();
      onChange("dom");
    });
    term.loadAddon(webgl);
    onChange("webgl");
  } catch {
    onChange("dom");
  }
}
```

Check the constructor before relying on it: open `node_modules/@xterm/addon-webgl/typings/addon-webgl.d.ts` and confirm that it takes `preserveDrawingBuffer?: boolean`. If 0.19 takes an options object instead, pass `{ preserveDrawingBuffer: platform === "linux" }`.

`src/features/perf/PerfOverlay.tsx`:
```tsx
import { useEffect, useState } from "react";
import type { RendererKind } from "../terminal/renderer";
import type { LatencyMeter } from "./latency";

/** Hidden panel for the performance gate. Toggle: Ctrl+Shift+Alt+P (Cmd+Shift+Alt+P on macOS). */
export function PerfOverlay({ meter, renderer }: { meter: LatencyMeter; renderer: RendererKind }) {
  const [stats, setStats] = useState(meter.stats());
  useEffect(() => {
    const id = setInterval(() => setStats(meter.stats()), 500);
    return () => clearInterval(id);
  }, [meter]);
  return (
    <output className="perf-overlay" aria-live="off">
      {renderer} · typing p50 {stats.p50.toFixed(1)} ms · p95 {stats.p95.toFixed(1)} ms · n {stats.n}
    </output>
  );
}
```

`src/features/terminal/TerminalView.tsx`:
```tsx
import "@xterm/xterm/css/xterm.css";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import { PtySocket } from "../../lib/api/ptySocket";
import { LatencyMeter } from "../perf/latency";
import { PerfOverlay } from "../perf/PerfOverlay";
import { loadRenderer, type RendererKind } from "./renderer";
import { currentTheme } from "./theme";

function isPerfToggle(e: KeyboardEvent): boolean {
  return e.type === "keydown" && e.shiftKey && e.altKey && (e.ctrlKey || e.metaKey) && e.code === "KeyP";
}

export function TerminalView({ endpoint, tabId }: { endpoint: Endpoint; tabId: string }) {
  const host = useRef<HTMLDivElement>(null);
  const meter = useMemo(() => new LatencyMeter(), []);
  const [renderer, setRenderer] = useState<RendererKind>("dom");
  const [perfOpen, setPerfOpen] = useState(false);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const fontFamily = getComputedStyle(document.documentElement).getPropertyValue("--font-mono").trim();
    const term = new Terminal({ fontFamily, fontSize: 13, cursorBlink: true, scrollback: 10_000, theme: currentTheme() });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);
    loadRenderer(term, endpoint.platform, setRenderer);
    term.attachCustomKeyEventHandler((e) => {
      if (isPerfToggle(e)) {
        setPerfOpen((open) => !open);
        return false;
      }
      return true;
    });

    const socket = new PtySocket(endpoint, tabId, {
      onOutput: (seq, bytes) => term.write(bytes, () => socket.processed(seq + bytes.length)),
      onReset: (seq, screen) => {
        term.reset();
        term.write(screen, () => socket.processed(seq));
      },
      onExit: (code) => term.write(`\r\n\x1b[2mProcess exited with code ${code}.\x1b[0m\r\n`),
    });

    const subs = [
      term.onData((d) => {
        meter.markInput();
        socket.sendInput(d);
      }),
      term.onBinary((d) => socket.sendBytes(d)),
      term.onResize(({ cols, rows }) => socket.resize(cols, rows)),
      term.onWriteParsed(() => meter.markOutput()),
    ];

    let frame = 0;
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => fit.fit());
    });
    observer.observe(el);
    fit.fit();
    socket.resize(term.cols, term.rows);
    term.focus();

    const scheme = matchMedia("(prefers-color-scheme: dark)");
    const onScheme = () => {
      term.options.theme = currentTheme();
    };
    scheme.addEventListener("change", onScheme);

    return () => {
      scheme.removeEventListener("change", onScheme);
      observer.disconnect();
      cancelAnimationFrame(frame);
      subs.forEach((s) => s.dispose());
      socket.close();
      term.dispose();
    };
  }, [endpoint, tabId, meter]);

  return (
    <>
      <div ref={host} className="terminal-host" />
      {perfOpen && <PerfOverlay meter={meter} renderer={renderer} />}
    </>
  );
}
```

`src/App.tsx`:
```tsx
import { useEffect, useState } from "react";
import { type StartResult, startSession } from "./lib/api/session";
import { TerminalView } from "./features/terminal/TerminalView";

export function App() {
  const [state, setState] = useState<StartResult | null>(null);
  useEffect(() => {
    let live = true;
    void startSession().then((s) => live && setState(s));
    return () => {
      live = false;
    };
  }, []);

  // Startup is well under 150 ms, so no spinner (UX gate: loading states only past 150 ms).
  if (state === null) return null;
  if (state.kind === "error") {
    return (
      <p className="startup-error" role="alert">
        {state.message}
      </p>
    );
  }
  return <TerminalView endpoint={state.endpoint} tabId={state.tabId} />;
}
```

`src/main.tsx`:
```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/app.css";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
```

- [ ] **Step 4: Type-check and run the frontend tests**

Run: `pnpm build && pnpm test`

Expected: `tsc --noEmit` passes, Vite builds `dist/`, and all 11 tests pass.

- [ ] **Step 5: Run the app (manual check on Windows)**

Run: `pnpm tauri dev`

Expected:
- [ ] The window opens on a PowerShell 7 prompt in the home directory, with no visible chrome.
- [ ] Typing echoes instantly. `Get-ChildItem` lists files and the colours render.
- [ ] Resizing the window reflows: `$Host.UI.RawUI.WindowSize` matches the window.
- [ ] Ctrl+Shift+Alt+P shows `webgl · typing p50 … ms`. After typing about 50 characters, **p95 is under 30 ms**.
- [ ] `exit` prints "Process exited with code 0." in dim text.
- [ ] Closing the window leaves no `pwsh.exe` started by Marshell running (check Task Manager).
- [ ] Launching a second instance focuses the first window.
- [ ] `~/.marshell/run/endpoint.json` exists, and its port matches the window's connection (see devtools → Network → WS).

If anything fails, stop and use the `mattpocock-skills:diagnosing-bugs` skill before changing code.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src-tauri assets src
git commit -m "feat(app): Tauri shell with one live terminal tab over the core API"
```

---

### Task 10: CI on Windows, macOS and Linux

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Write the workflow**

`.github/workflows/ci.yml`:
```yaml
name: ci
on:
  push:
    branches: [main]
  pull_request:

jobs:
  build-test:
    strategy:
      fail-fast: false
      matrix:
        os: [windows-latest, macos-latest, ubuntu-24.04]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - name: Linux system packages (Tauri + audio)
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev \
            libssl-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev
      # rust-toolchain.toml pins the toolchain; rustup installs it on first use.
      - uses: Swatinem/rust-cache@v2
      - uses: pnpm/action-setup@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 24
          cache: pnpm
      - run: pnpm install --frozen-lockfile
      - run: cargo test --workspace
      - run: pnpm check:generated
      - run: pnpm test
      - run: pnpm build
      - run: pnpm tauri build --no-bundle
      - name: S6 throughput (release)
        run: cargo test -p marshell-core --release --test pty_ws -- --ignored --nocapture
```

- [ ] **Step 2: Validate locally**

Run: `cargo test --workspace && pnpm check:generated && pnpm test && pnpm build`

Expected: all PASS. `check:generated` passes only once `src/generated` is committed.

- [ ] **Step 3: Commit (push only after the user approves creating the GitHub repo)**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: build and test on Windows, macOS and Linux"
```

Creating the remote is outward-facing, so ask the user first. Then run `gh repo create <owner>/marshell --public --source . --push` (MIT, public, as decided). Check that all three jobs are green, and record the macOS and Linux S7 rows and the S6 numbers from the logs.

---

### Task 11: Gate spikes S1, S2 and S6 (run and record)

**Files:**
- Create: `docs/spikes/S1-webview-gate.md`, `docs/spikes/S2-conpty.md`, `docs/spikes/S6-throughput.md`

These are manual runs with written results. **S1 needs a macOS machine and a Linux desktop** (GNOME on Wayland and X11, ideally one NVIDIA box). WSLg on this Windows machine is a partial stand-in for Linux only. Ask the user which machines are available before starting.

- [ ] **Step 1: Write the S1 procedure and result sheet**

`docs/spikes/S1-webview-gate.md`:
````markdown
# S1: Webview gate (Linux WebKitGTK, macOS WKWebView)

Build: `pnpm tauri build --debug --no-bundle`, then run the binary.

| # | Check | How | Pass |
|---|---|---|---|
| a | Idle repaint | Run `while true; do date; sleep 1; done`; take hands off the keyboard and mouse for 30 s | Clock ticks every second with no input |
| b1 | Dead keys | US-International: `'` then `e` -> é; `~` then `/` -> `~/` | Exact characters, no doubles |
| b2 | IME | Japanese or Chinese IME: compose and commit a word | Committed once, no duplicates |
| b3 | Accented keys on WebKitGTK | German layout: type `äöü` | Once each |
| c1 | Flood | `yes \| head -n 2000000` | UI never freezes over 1 s; perf p95 stays < 30 ms while typing in the same tab afterwards |
| c2 | Big file | `cat` of a 50 MB text file | Completes; window stays responsive (move or resize during it) |
| d | Renderer | Perf overlay shows `webgl`, or `dom` with acceptable c1/c2 | As stated |

Linux variants to try in order if a check fails: `MARSHELL_LINUX_GFX=no-dmabuf`, then `no-compositing`, then `both`.

Known fixes to apply if needed: WebGL `preserveDrawingBuffer` (already on for Linux); the IME commit guard (orbit-desktop PR #23); the dead-key shim (xterm.js #5894).

## Results
| Platform | Machine / GPU / session | a | b1 | b2 | b3 | c1 | c2 | d | Variant | Notes |
|---|---|---|---|---|---|---|---|---|---|---|

## Decision
- [ ] PASS: keep Tauri
- [ ] FAIL without a clean fix: switch the window shell to Electron (core and frontend unchanged)
````

- [ ] **Step 2: Write and run S2 on this Windows machine**

`docs/spikes/S2-conpty.md`:
````markdown
# S2: ConPTY + shells on Windows

Run each shell by changing `CreateSessionRequest.shell` (temporarily, in `startSession`).

| Shell argv | Renders prompt | Colours | Resize reflows | ESC / Alt+key | PSReadLine prediction | `exit` -> code | Orphans after close |
|---|---|---|---|---|---|---|---|
| `pwsh.exe -NoLogo` | | | | | | | |
| `powershell.exe -NoLogo` | | | | | n/a | | |
| `cmd.exe` | | | | | n/a | | |
| `C:\Program Files\Git\bin\bash.exe --login -i` | | | | | n/a | | |
| `wsl.exe` | | | | | n/a | | |

Also check:
- the first output arrives without a hang (the DSR answer from Task 4);
- `claude` runs inside pwsh and its TUI renders and resizes correctly;
- Ctrl+C interrupts `ping -t localhost`.

## Decision
- [ ] Stock ConPTY is fine
- [ ] Needs a sideloaded conpty.dll + OpenConsole (record which bug)
````

Fill in every cell from a real run.

- [ ] **Step 3: Record S6**

`docs/spikes/S6-throughput.md`:
```markdown
# S6: WebSocket throughput with coalescing and backpressure

Pass: 50 MB through the full pipeline in under 2 s end to end (pty -> core -> WS). Memory stays bounded
(unacked ≤ 1 MiB + 64 KiB, proven by `backpressure_bounds_unacked_output`). Typing in another tab stays under 30 ms.

| OS | `s6_throughput_50mb` output | Pass |
|---|---|---|
| Windows (dev box) | | |
| Windows (CI) | | |
| macOS (CI) | | |
| Linux (CI) | | |

If Windows misses 2 s, the likely ceiling is ConPTY itself. Measure a raw ConPTY read of the same flood (a
`Session::pull` loop with no WebSocket) and record both numbers. Our pipeline passes if it adds under 20%.
```

The "typing in another tab" part of S6 waits for phase 2 (multiple tabs); note that in the file.

- [ ] **Step 4: Commit**

```bash
git add docs/spikes
git commit -m "docs(spikes): S1, S2, S6 procedures and results"
```

---

### Task 12: S8, the elevated helper on Windows (spike only, time-boxed to one day)

**Files:**
- Create: `spikes/elevated-host/{Cargo.toml,src/main.rs}` (a standalone crate, **not** a workspace member)
- Create: `docs/spikes/S8-elevated-host.md`

**Goal:** prove or disprove four things:
1. A non-elevated parent can start a helper through UAC.
2. The helper runs elevated.
3. The helper connects back over loopback with a one-time token.
4. Text typed in the non-elevated parent drives an elevated pwsh in the helper.

Production design is phase 6.

- [ ] **Step 1: Write the spike crate**

`spikes/elevated-host/Cargo.toml`:
```toml
[package]
name = "elevated-host-spike"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
anyhow = "1"
portable-pty = "0.9"
getrandom = "0.4"
hex = "0.4"

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61", features = [
  "Win32_Foundation", "Win32_Security", "Win32_System_Threading",
  "Win32_UI_Shell", "Win32_UI_WindowsAndMessaging",
] }
```

`spikes/elevated-host/src/main.rs`:
```rust
//! S8 spike. `launch` (run non-elevated) listens on loopback, starts `host` elevated through UAC,
//! and forwards stdin lines to the elevated pwsh. Raw bytes only; no Marshell protocol.
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("launch") => launch(),
        Some("host") => host(args[2].parse()?, &args[3]),
        _ => anyhow::bail!("usage: elevated-host-spike launch"),
    }
}

fn launch() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let mut raw = [0u8; 32];
    getrandom::fill(&mut raw).expect("rng");
    let token = hex::encode(raw);
    let exe = std::env::current_exe()?;
    println!("elevated before UAC? {}", is_elevated());
    shell_execute_runas(&exe, &format!("host {port} {token}"))?;
    let (mut stream, peer) = listener.accept()?;
    let mut line = String::new();
    std::io::BufReader::new(stream.try_clone()?).read_line(&mut line)?;
    anyhow::ensure!(line.trim() == token, "helper sent a wrong token");
    println!("helper connected from {peer}; type commands (e.g. `whoami /groups | findstr Mandatory`)");
    let mut out = stream.try_clone()?;
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = out.read(&mut buf) {
            if n == 0 {
                break;
            }
            std::io::stdout().write_all(&buf[..n]).ok();
            std::io::stdout().flush().ok();
        }
    });
    for l in std::io::stdin().lock().lines() {
        stream.write_all(format!("{}\r", l?).as_bytes())?;
    }
    Ok(())
}

fn host(port: u16, token: &str) -> anyhow::Result<()> {
    anyhow::ensure!(is_elevated(), "host must run elevated");
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.write_all(format!("{token}\n").as_bytes())?;
    let pty = portable_pty::native_pty_system().openpty(portable_pty::PtySize {
        rows: 30,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    let _child = pty.slave.spawn_command(portable_pty::CommandBuilder::new("pwsh.exe"))?;
    let mut reader = pty.master.try_clone_reader()?;
    let mut writer = pty.master.take_writer()?;
    let mut out = stream.try_clone()?;
    std::thread::spawn(move || std::io::copy(&mut reader, &mut out));
    // The helper accepts input bytes only. In production it also accepts resize, and nothing else.
    std::io::copy(&mut stream, &mut writer)?;
    Ok(())
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn shell_execute_runas(exe: &std::path::Path, params: &str) -> anyhow::Result<()> {
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;
    let (verb, file, params) = (wide("runas"), wide(&exe.display().to_string()), wide(params));
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = SW_HIDE as i32;
    let ok = unsafe { ShellExecuteExW(&mut info) };
    anyhow::ensure!(ok != 0, "UAC declined or launch failed: {}", std::io::Error::last_os_error());
    Ok(())
}

#[cfg(windows)]
fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
fn shell_execute_runas(_: &std::path::Path, _: &str) -> anyhow::Result<()> {
    anyhow::bail!("Windows only")
}

#[cfg(not(windows))]
fn is_elevated() -> bool {
    false
}
```

If windows-sys 0.61 rejects a type (for example, `HANDLE` is now a pointer type, or `SW_HIDE` has a different integer type), fix the cast at the call site. This is spike code.

- [ ] **Step 2: Run the spike (needs the user at the keyboard for UAC)**

Run: `cargo run --manifest-path spikes/elevated-host/Cargo.toml -- launch`

Expected:
1. It prints `elevated before UAC? false`.
2. The UAC prompt appears; approve it.
3. It prints `helper connected …`.
4. Typing `whoami /groups | findstr Mandatory` shows **High Mandatory Level**.

Then repeat, decline UAC, and confirm that it reports the error cleanly.

- [ ] **Step 3: Record the result and the threat notes**

`docs/spikes/S8-elevated-host.md`:
```markdown
# S8: Elevated session helper (Windows)

| Check | Pass criterion | Result |
|---|---|---|
| UAC prompt from non-elevated parent | Prompt appears; decline is reported as an error | |
| Helper is elevated | `is_elevated()` true; `whoami /groups` shows High Mandatory Level | |
| Loopback + one-time token | Helper connects; wrong token is rejected | |
| Input reaches elevated pwsh | Commands typed in the parent run in the elevated shell | |

Threat notes for phase 6:
- Any non-elevated process holding the token can drive an admin shell. So: a one-time token kept only in
  core memory, never written to endpoint.json; the helper accepts one connection only; the helper checks that the
  peer's owning pid is the Marshell core (GetExtendedTcpTable); the helper accepts only input and resize.
- If these can't be made safe: open elevated sessions in a separate elevated Marshell window (Windows Terminal's approach).

Decision: [ ] helper approach viable for phase 6  [ ] use separate elevated window
```

- [ ] **Step 4: Commit**

```bash
git add spikes/elevated-host docs/spikes/S8-elevated-host.md
git commit -m "spike(S8): elevated pty helper through UAC and loopback token"
```

---

## Phase 1 exit criteria (from docs/PLAN.md §9)

- [ ] Typing feels instant: perf overlay p95 under 30 ms on Windows, and under 30 ms on macOS and Linux in S1.
- [ ] Resize reflows on all three OSes.
- [ ] S1 gate decided (Tauri kept, or the Electron switch planned).
- [ ] CI is green on Windows, macOS and Linux (cargo tests, generated-types check, vitest, `tauri build --no-bundle`, S6).
- [ ] S2, S6, S7 and S8 results recorded in `docs/spikes/`.
