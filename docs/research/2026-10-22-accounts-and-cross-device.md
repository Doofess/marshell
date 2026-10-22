# Accounts and cross-device: design proposal (phase 10, v2 track)

- Date: 2026-10-22. Status: decided the same day. The owner answered every question in §8 with its recommended option; the record is `docs/PLAN.md` §9, "Phase 10 decisions (2026-10-22)".
- Brief: sign in with GitHub, connect several PCs to one account, use a terminal that runs on one PC from another, and carry agent conversation context between PCs.
- Naming: the brief says "Airport" and "Gemini CLI". The app is Marshell (PLAN "Rename"), and Google's CLI is Antigravity (`agy`; PLAN "Corrections" 1). This document uses those.
- Everything about the codebase below was read from the source on 2026-10-22 (main at `d3f098c`). External facts carry a source in §9; anything marked *to verify* is settled by a spike in slice 10a.

## 1. What is here, and what has to change

### 1.1 What the codebase gives us

- **The webview never talks to the network.** The CSP allows only the app origin and `127.0.0.1` (`src-tauri/tauri.conf.json:26`), and SCOPE says the CSP is never loosened. So every remote byte must enter and leave through the Rust core. That is the right shape anyway: the core becomes the relay client, does the cryptography, and presents remote sessions to the webview through the same loopback API it already serves. The UI needs new rows and dialogs, not a new transport.
- **The pty stream is already a resumable, acknowledged byte stream.** `GET /v1/pty/{tab}` speaks binary frames with a byte-offset `seq`: `OUTPUT`, `RESET` (the vt100 screen snapshot), `EXIT`; and `INPUT`, `RESIZE`, `ACK`, `RESUME` back (`crates/protocol/src/frames.rs`, `crates/core/src/server/pty_ws.rs`). Each session keeps an 8 MiB ring (`pty/buffer.rs:4`), a 64 KiB flush cap, a 1 MiB unacknowledged window, and a `vt100::Parser` for the screen. A remote viewer can reuse this protocol byte for byte: the viewer's core forwards the same frames over the relay, and the viewer's webview opens `/v1/pty/{remote tab}` exactly as it does for a local one.
- **The session model is one struct the UI mirrors.** `SessionInfo` (`crates/protocol/src/api.rs:131`) carries number, name, agent, project, mode, state, live status, foreground, paused and muted. The UI holds a hand-rolled `SessionsStore` fed by `{"type":"sessions"}` snapshots on `/v1/events` (`src/features/sessions/useSessions.ts:57`, `crates/core/src/server/events.rs:29`), and rows are a pure function `toRow(info)` (`src/features/sessions/toRow.ts:111`). Adding an `origin` field and shipping remote sessions inside the same snapshot gives the sidebar, the rail, the palette and the lane remote rows for little UI work.
- **Secrets and files already have a discipline.** `~/.marshell` is owner-only (3b decision 3), every store file carries a `version` and a `.bak` with migrations (`crates/core/src/store.rs`), the endpoint token is per launch and compared in constant time (`auth.rs`), and the config service does preview, snapshot, atomic write and undo. The cloud layer must use the same store for its public state and the OS keychain for its secrets, and must never add a second write path to a CLI's configuration.
- **History knows which conversation a tab ran.** The hooks give the core `session_id` and `transcript_path`; the archive keeps a zstd copy refreshed on every `Stop` and at `SessionEnd`, with a `.meta.json` sidecar, and the SQLite index links sessions to tabs and folders (`crates/core/src/history/`). A handoff record can be built from what the core already holds, and after a crash from the archived copy.
- **Live status travels with the session.** `LiveStatus` is part of `SessionInfo`, so a mirrored remote session shows the same Working, Needs you, Done and context ring as a local one with no extra protocol.
- **A plan for daemons exists.** PLAN §3 and §4 keep the core protocol-first so that v2's `marshelld` is a packaging change. The cloud module belongs in `crates/core` with no Tauri dependency, so it moves with the core when the daemon lands.
- **No adapter trait yet.** Agent behaviour is `match` arms on `"claude" | "codex" | "antigravity"` in `profiles/detect.rs` (launch, mode, resume and attachment arms) and `agents/mod.rs`. Only Claude has a module. Handoff delivery adds one more arm per agent, in the same places, until phase 6 lifts them into the trait.

### 1.2 What must change for remote sessions

1. **Pty fan-out.** Today each `/v1/pty/{tab}` socket runs its own loop with its own cursor, but `attached` and `acked` are shared per session (`pty/session.rs:434-445`, `wait_for_space` at 343). Two sockets can coexist by accident and the slowest one stalls the reader. The refactor: a `Viewers` set per session, each viewer with its own cursor and window. The **local** viewer keeps today's back-pressure (the pty reader waits on it). **Remote** viewers are lossy by design: a viewer more than its window behind is skipped to the end and sent a `RESET` snapshot, so a slow laptop can never slow the desktop's agent. The DSR answer and `attached` semantics stay tied to the local viewer only.
2. **Session identity and origin.** `SessionInfo` gains `origin: Local | Remote { device_id, device_name }` and `viewers: Vec<Viewer>` (who is watching a local session). A remote session's `tab_id` on the viewer is `"{device_id}:{host tab_id}"`, stable across reconnects; routes take it as a string already. The registry gains a `RemoteMirror` (per online device, its session list) merged into the snapshot. `saved_tabs` must skip remote rows so `layout.json` never records them (today it writes every open row, `registry.rs:224`). Routes that assume a local pty (`resume`, `restart`, `pause`, `continue`, `recording`, `attachments`, `paste-paths`, `files`, `resolve`, `PATCH`, `DELETE`) answer 409 "Runs on desktop" for a remote tab; `pty`, `input`, `peek` and, in 10e, `muted` and permissions are proxied.
3. **Session numbers.** A remote row keeps the host's number and is shown as "desktop · 6" (PLAN "Session numbers": the number names a terminal on *its* machine). Ctrl+Shift+1–9 stay local. The palette's "Go to session" accepts "desktop 6". Accessible names read "Session 6 on desktop, billing, …".
4. **Events socket.** `/v1/events` gains `{"type":"account"}` and `{"type":"devices"}` messages (presence, pending approvals), sent on connect and on change like `sessions` and `permissions` are today. The UI store gains two fields.
5. **Project identity.** `projects.rs` identifies a project by path. Handoffs need a key that is the same on every PC: the normalised git remote URL, else a key the user assigns. `projects.json` goes to version 2 with `key` and `remote_url` per project, through the existing migration path.
6. **Launch arguments per agent.** `launch_argv` gains a handoff argument (a Claude flag, an env variable for the others), one more arm per agent next to `mode_args`.
7. **Files touched, durably.** The hook log keeps the last 200 raw events in memory (`hooks/log.rs`). The handoff record needs each session's edited files for its whole life, so the fold that builds `LiveStatus` also keeps a bounded per-session set of paths from `PostToolUse` (Edit, Write, MultiEdit, NotebookEdit) and drops it when the tab closes.
8. **Config service.** If the instruction-block delivery in §6.4 is chosen for Codex and Antigravity, the `MarkdownBlock` edit op from PLAN §6 (designed, not built; `EditOp` has JsonSet, JsonRemove, CreateFile, Restore) is built here rather than in phase 7.
9. **Notification engine.** No change until 10e, which adds remote rows to the needs-you count. Banners for remote sessions are a switch in 10g.
10. **Dependencies (SCOPE policy applies, each with its concrete need in the slice plan).** Core: the OS keychain through `keyring-core` 1.0 with the three store crates (`windows-native-keyring-store`, `apple-native-keyring-store`, `zbus-secret-service-keyring-store` with its tokio feature), or the thin `keyring` 4.2 wrapper if S12 shows its default feature set behaves under tokio; `snow` 0.10 for Noise, or the RustCrypto set (`x25519-dalek` 3, `ed25519-dalek` 3, `chacha20poly1305` 0.11, `hkdf`) if `snow` is ruled out; `tokio-tungstenite` promoted from dev to normal with `rustls-tls-webpki-roots` (the relay socket); `ureq` promoted from dev with `rustls` (a handful of HTTP calls to the backend and GitHub, run in `spawn_blocking`; `reqwest` is the heavier alternative); `base64`. The tree's `sha2` is 0.10 and the newest `hkdf` pairs with 0.11, so the versions are reconciled in 10a rather than bumped blind. Backend: a TypeScript Worker with `wrangler`, in a new top-level `cloud/` folder with its own `package.json`, outside the app's build. Versions, audits and licences are in §9.3.
11. **Development and tests.** The single-instance plugin focuses the first window, so two instances on one PC need a dev escape: the single-instance id is derived from `MARSHELL_HOME` in debug builds. Core tests run two cores in one process against an in-process fake relay (`axum` WebSocket, already a dev dependency path) with no network. A test asserts that a signed-out core opens no outbound socket, beside the "zero token cost" test.

Nothing above touches `~/.claude`, `~/.codex` or the guard, the CSP or the capabilities. 10f's instruction block, if chosen, goes through the config service with the consent card, like hooks.

## 2. Architecture

### 2.1 Diagram

```
 ┌──────────────── PC "desktop" (host) ─────────────────┐     ┌──────────────── PC "laptop" (viewer) ────────────────┐
 │  webview ── loopback HTTP + WS, per-launch token ── core │     │  webview ── loopback HTTP + WS, per-launch token ── core │
 │   xterm  ─ /v1/pty/t_ab12 ─┐  pty (ConPTY)  ring 8 MiB  │     │   xterm  ─ /v1/pty/dev_desktop:t_ab12 ─┐ (proxied)    │
 │                            └─ viewers: local, laptop    │     │                                        └─ remote view  │
 │  core/cloud: keychain · device keys · Noise · relay WS  │     │  core/cloud: keychain · device keys · Noise · relay WS  │
 └───────────────────────────┬───────────────────────────┘     └───────────────────────────┬──────────────────────────┘
                             │ wss, device access token                                     │ wss
                             │              ◄════════ Noise transport, end to end ════════► │
                             └──────────────────────────┐             ┌─────────────────────┘
                                                        ▼             ▼
                           ┌──────────────────── Cloudflare Worker "marshell-cloud" ────────────────────┐
                           │ HTTP: /auth/github, /auth/refresh, /devices, /devices/{id}/approve|revoke, │
                           │       /keys, /handoffs                     (device JWT; signed mutations)  │
                           │ AccountHub Durable Object, one per account:                                 │
                           │   the online devices' WebSockets (hibernation) · presence · opaque relay   │
                           │ D1: accounts, devices, device_keys, refresh_tokens, handoffs (ciphertext)  │
                           └──────────────────────────────┬────────────────────────────────────────────┘
                                                          │ at sign-in only: check the user token with the client secret
                                                          ▼
                                                 GitHub · GitHub App "Marshell" · device flow
```

- The relay forwards ciphertext between a user's own devices and keeps presence. It never holds a key that opens terminal traffic or a handoff.
- GitHub is an identity provider at sign-in time only. Nothing of the user's GitHub data is read beyond the numeric user id, login and avatar.
- Both cores keep serving the loopback API exactly as today. Signed out, the cloud module is not constructed and nothing changes.

### 2.2 Transport: relay, not WebRTC (recommendation)

| | WebSocket relay with end-to-end encryption | WebRTC data channels |
|---|---|---|
| NAT, firewalls, corporate networks | Outbound 443 only; works wherever the browser works | Needs STUN and, in practice, TURN for a share of connections; TURN is a relay anyway, billed per GB |
| Rust side | `tokio-tungstenite` and `axum` are in the tree (dev) | `webrtc` crate or `str0m`, both large; or drive `RTCPeerConnection` in the webview, which the CSP forbids and which would move crypto and I/O out of the core |
| Latency | One hop through the nearest edge: tens of ms on the same continent | Direct path can be lower on a LAN, same or worse through TURN |
| Behaviour | Deterministic: one ordered stream per device pair | Candidate gathering, renegotiation, ICE restarts on network change |
| Privacy from the server | Noise transport: the relay sees sizes, timing and routing | DTLS between peers; the TURN server sees the same metadata |
| Cost at small scale | Pennies: messages and duration on a Durable Object | TURN egress per GB plus the signalling server you still need |

Recommendation: the relay. Terminal traffic is small (a busy agent writes well under 1 MB a minute) and the hard parts of WebRTC buy nothing a terminal needs. A direct path (`iroh`, or WebRTC) can be added later behind the same device-to-device frame protocol, because the frames do not know what carries them.

### 2.3 Backend: Cloudflare Workers + Durable Objects + D1 (recommendation)

What the backend must do: exchange a GitHub sign-in for an account; register, approve, rename and revoke devices; hold one wrapped copy of the account key per device; know which devices are online; forward ciphertext between them; store small ciphertext blobs (handoffs) with a blind index per project.

| Candidate | Fit | Notes |
|---|---|---|
| **Cloudflare**: Worker (HTTP), one Durable Object per account (sockets, presence, relay), D1 (tables), R2 later if blobs grow | Best fit | A DO is exactly "one tiny stateful server per account": it holds the account's WebSockets, hibernates while idle, and wakes on a message. Costs scale with messages, not with open sockets. One codebase in TypeScript, deployed with `wrangler`, open-sourced in `cloud/`. |
| Supabase (Postgres, Auth, Realtime) | Partial | Auth has "Sign in with GitHub" out of the box, but as an OAuth App web flow, not a GitHub App device flow. Realtime Broadcast is a pub/sub for small JSON messages with per-tier message and connection quotas; it is not built to be an opaque binary relay at terminal throughput. It would still need a Worker or an edge function for the device-flow exchange. |
| A small Rust server (`axum`, `tokio-tungstenite`, SQLite) on Fly.io or Hetzner | Possible | The same crates the core uses, one binary, full control. Costs a fixed few dollars a month for a VM plus the on-call: TLS, backups, upgrades, a second region when one is down. Right if the owner wants to self-host; wrong as the default while the service is small. |

Rough monthly running costs, from the vendors' pages on 2026-10-22 and the research pass's estimate of 50 MB of relayed output per user per day (§9.2 has the inputs and sources):

| Monthly, estimated | 100 users | 1,000 users |
|---|---|---|
| Cloudflare, Durable Objects hibernating | about $5 (the Workers Paid base fee) | about $8 |
| Cloudflare, Durable Objects kept awake | about $135 | about $1,350 |
| Supabase Pro, Realtime Broadcast at 4 KB messages | about $205 | about $1,950 |
| Fly.io, one small VM plus egress | about $10 | about $43 |
| Hetzner, one CX23 VM | about €6 | about €6 to €9 |

The VM rows leave out backups, a second region and the owner's time. The two Cloudflare rows differ by one design rule: the relay must hold its sockets with the Durable Object WebSocket Hibernation API, so an idle account costs nothing while its sockets stay open. Supabase's Realtime bills per message in both directions, which is the wrong shape for a terminal stream.

The relay URL is a setting (Advanced), so a self-hosted Worker or Rust relay is one field away, and the backend is MIT in the same repository under `cloud/`.

## 3. Data model

### 3.1 Backend (D1)

| Table | Columns | Notes |
|---|---|---|
| `accounts` | `id` (ULID), `github_user_id` (int, unique), `github_login`, `avatar_url`, `created_at`, `ak_version` (int) | The numeric GitHub id keys the account; `login` can change and is display only. |
| `devices` | `id` (ULID, minted by the device), `account_id`, `name`, `hostname`, `os`, `arch`, `app_version`, `pk_sign` (Ed25519), `pk_dh` (X25519), `status` (`pending`, `approved`, `revoked`), `created_at`, `approved_at`, `approved_by` (device id), `revoked_at`, `revoked_by`, `last_seen_at` | Public keys are set once at registration and never updated; a device with new keys is a new device. |
| `device_keys` | `device_id`, `ak_version`, `wrapped_ak` (sealed to `pk_dh`), `signed_by`, `signature`, `created_at` | One row per device per account-key version. The server stores, never opens. |
| `refresh_tokens` | `hash` (SHA-256), `device_id`, `created_at`, `expires_at`, `replaced_by` | Rotating. A reuse of a replaced token revokes the device's whole chain. |
| `approvals` | `id`, `device_id` (pending), `nonce`, `created_at`, `expires_at` | One open approval per pending device; 15 minutes. |
| `handoffs` | `id` (ULID), `account_id`, `project_index` (HMAC-SHA256 of the project key under the account key, hex), `device_id`, `agent`, `kind` (`record`, `smart`), `ak_version`, `nonce`, `ciphertext` (BLOB, ≤ 256 KiB), `size`, `created_at`, `ended` (`closed`, `crashed`, `resumed-elsewhere`) | The server can list by project without knowing the project. Retention: the newest 20 per project index, 90 days, both server-side. |
| `limits` | `account_id`, `devices_max` (10), `bytes_month`, `handoffs_max` | Abuse limits; see §8. |

Presence is not a table: it is the set of sockets the account's Durable Object holds, with `last_seen_at` written back to `devices` on connect, disconnect and every 5 minutes.

### 3.2 Device (under `~/.marshell`)

| Where | Holds |
|---|---|
| OS keychain, service `marshell`, user `device-key` | The Ed25519 and X25519 secret keys (64 bytes, base64). |
| OS keychain, `refresh-token` | The backend refresh token. |
| OS keychain, `account-key.<version>` | One unwrapped account key per version (32 bytes, base64). One entry per version because Windows Credential Manager caps a secret at 2,560 bytes. |
| `cloud/device.json` (version 1) | `device_id`, `name`, the two public keys, `account: {id, github_login, avatar_url}`, `relay_url`, `signed_in_at`. Public, so it lives in the store with `.bak`. |
| `cloud/peers.json` (version 1) | For every approved device of the account: id, name, os, public keys as pinned at approval, `allow_input` (host-side policy, default true), `pinned_at`. The registry can rename a peer, never change its keys. |
| `handoffs/<project index>/<id>.md` and `.json` | The local plaintext copy of each handoff, owner-only like transcripts and recordings (cross-cutting requirement "secrets at rest"). |
| `settings.json` | `cloud: { remote_input: bool (true), remote_banners: bool (false), handoff: { record: bool (true), smart: bool (false), redact: bool (true) } }` |

If the keychain is unavailable (no Secret Service on Linux, a locked keychain, a denied prompt), the account layer is off with the reason shown in the Account dialog. Secrets are never written to a file instead. Three platform rules from the research (§9.3): Windows entries are written with local persistence, because the default enterprise persistence can roam with a Microsoft account and a roamed device key would make two PCs one device; the keychain crates have no async API and are not safe for one credential across threads, so every call runs in `spawn_blocking` behind one mutex; on macOS a debug build and a release build are different signing identities, so dev builds prompt and the live check uses a signed build. A headless Linux host has no Secret Service; that is the v2 daemon's problem, not this phase's.

### 3.3 Session, project, summary (protocol types)

```rust
// crates/protocol/src/api.rs additions (generated to src/generated/)
pub enum SessionOrigin { Local, Remote { device_id: String, device_name: String } }
pub struct Viewer { pub device_id: String, pub device_name: String, pub since_ms: u64, pub typing: bool }
// SessionInfo gains:  origin: SessionOrigin,  viewers: Vec<Viewer>,  project_key: Option<String>

pub struct Device { id, name, hostname, os, arch, app_version, status: DeviceStatus, online: bool,
                    last_seen_ms: Option<u64>, is_this: bool, approved_by: Option<String>, allow_input: bool }
pub enum DeviceStatus { Pending { code: String }, Approved, Revoked }
pub struct Account { signed_in: bool, github_login: Option<String>, avatar_url: Option<String>,
                     relay: RelayState /* Off | Connecting | Online | Error(String) */, keychain: KeychainState }

pub struct Handoff { id, project_key, project_name, device_id, device_name, agent, session_id: Option<String>,
                     kind: HandoffKind, created_ms: u64, ended: HandoffEnd, branch: Option<String>,
                     text: String /* Markdown, decrypted, ≤ 64 KiB */ }
```

The project key: `ProjectKey::Remote("github.com/user/repo")` from the normalised `origin` URL (scheme, user, port and `.git` dropped, host lower-cased, `git@host:path` and `ssh://` forms folded), read from `.git/config` of the main checkout (a task worktree follows `commondir`, as `repo_root` does today). Without a remote, `ProjectKey::Assigned("<ulid>")`, created when the user names the project in the launcher ("Same project as… on another device" picks an existing assigned key from the account's list, which syncs as a tiny encrypted blob of its own). A path never leaves the device.

## 4. Protocol

Two layers. The relay understands only the first.

### 4.1 Device ↔ relay (WebSocket to the account's Durable Object)

Text frames are JSON; data frames are binary.

| Direction | Message | Fields | Notes |
|---|---|---|---|
| → | `hello` | `proto: 1`, `access_token`, `device_id`, `app_version` | First frame; the token is never in the URL. A bad or revoked token closes with code 4401. |
| ← | `welcome` | `device_id`, `server_time_ms`, `limits` | |
| ← | `presence` | `devices: [{id, online, last_seen_ms}]` | On connect and on every change. |
| ← | `device_event` | `kind: pending \| approved \| revoked \| renamed`, `device: {…}`, `approval?: {id, nonce}` | Fan-out of registry changes so the Devices dialog is live without polling. |
| ← | `handoff_event` | `kind: new`, `id`, `project_index`, `device_id` | Metadata only; the blob is fetched over HTTP. |
| → | data `0x20` | `to` (16 bytes) · ciphertext | Relay rewrites to `0x21 · from · ciphertext` and forwards. ≤ 256 KiB. Unknown or offline `to` returns `error`. |
| ← | data `0x21` | `from` (16 bytes) · ciphertext | |
| ↔ | `ping` / `pong` | | The Durable Object holds sockets with the WebSocket Hibernation API and answers pings with its auto-response, so an idle device wakes nothing and costs nothing. Per-socket state (device id, account id) stays under the 16 KiB attachment cap. |
| ← | `error` | `code`, `text`, `ref?` | Rate limited, too large, unknown device, revoked. |
| ← | `bye` | `reason` | Revoked, or the account's limits; the client signs out on `revoked`. |

The relay enforces per-device rate limits (frames and bytes per minute), the frame size, and that `to` is an approved device of the same account.

### 4.2 Device ↔ device (inside the Noise transport)

- Handshake: `Noise_KK_25519_ChaChaPoly_BLAKE2s`, both static keys known in advance (pinned in `peers.json` at approval). Prologue binds `proto`, both device ids and the account id, so a frame cannot be replayed into another pair. One Noise session per device pair per relay connection; rekey on reconnect. If `snow` fails the dependency policy (§9.3), the fallback is the RustCrypto set with an X25519 ECDH per connection, HKDF, and XChaCha20-Poly1305 with a per-direction counter, which is Noise KK by hand without the audits.
- Frames follow the style of `frames.rs`: `[kind u8][tab_len u8][tab bytes][payload]`. JSON payloads are small; output and input payloads are raw bytes.

| Kind | Name | From | Payload | Notes |
|---|---|---|---|---|
| `0x01` | `SESSIONS` | host → every peer | JSON `[SessionInfoLite]` | Sent on connect and on each registry change (same cadence as `/v1/events`). Lite drops `profile_id`, `cwd`, `foreground`; keeps number, name, agent, project key and name, branch, mode, state, live, paused, muted, viewers. |
| `0x10` | `ATTACH` | viewer → host | JSON `{cols, rows, from_seq}` | Opens a viewer; `from_seq` resumes. |
| `0x11` | `DETACH` | viewer → host | | |
| `0x20` | `OUTPUT` | host → viewer | `seq u64 BE` + bytes | Same `seq` as the host's pty stream. |
| `0x21` | `RESET` | host → viewer | `seq u64 BE` + screen | The vt100 snapshot; also the catch-up for a slow viewer. |
| `0x22` | `EXIT` | host → viewer | `code i32 BE` | |
| `0x23` | `ACK` | viewer → host | `seq u64 BE` | Advances the viewer's window (256 KiB). |
| `0x30` | `INPUT` | viewer → host | bytes | 10e. Refused at the host when `allow_input` is off or the session is paused; the host answers `0x7E REFUSED`. Never contains an Enter the viewer did not type. |
| `0x31` | `RESIZE` | viewer → host | JSON `{cols, rows}` | 10e. Applied only under the size rule in §6.3. |
| `0x40` | `KICK` | host → viewer | JSON `{reason}` | The host disconnected this viewer. |
| `0x50` | `PEEK_REQ` / `0x51 PEEK` | viewer ↔ host | JSON `{rows}` / text | Peek on a remote row. |
| `0x60` | `KEY_GRANT` | approver → new device | JSON `{ak_version, wrapped_ak, signature}` | Also written to `device_keys` so an offline new device gets it later. |
| `0x61` | `PROJECT_ALIAS` | any → all | JSON `{key, name}` | An assigned project key, so the other devices offer it by name. |
| `0x70` | `DECISION` | viewer → host | JSON `{rid, behavior, scope}` | 10g: answer a permission request remotely. |
| `0x7E` | `REFUSED` | host → viewer | JSON `{kind, reason}` | |
| `0x7F` | `ERROR` | | JSON | |

### 4.3 Backend HTTP (device access token unless noted)

| Route | Does |
|---|---|
| `POST /auth/github` (no token) | Body: a GitHub user access token from the device flow, plus the device's public record. The Worker checks the token with the GitHub App's client secret, loads or creates the account, registers the device (`pending`, or `approved` if it is the account's first), revokes the GitHub token, and returns an access token (JWT, 1 h) and a refresh token. |
| `POST /auth/refresh` (refresh token) | Rotates. |
| `POST /auth/sign-out` | Revokes this device's refresh chain; the device stays `approved` (it can sign in again). |
| `GET /devices` | The account's devices with presence. Pending devices get only their own row. |
| `PATCH /devices/{id}` | Rename. Body signed by an approved device (Ed25519 over the canonical body and a timestamp). |
| `POST /devices/{id}/approve` | Body: the approval id, the wrapped key, the signature by the approving device over the new device's keys. |
| `POST /devices/{id}/revoke` | Signed. Closes the device's socket, revokes its tokens, and expects a `device_keys` upload for the new `ak_version` in the same request. |
| `GET /keys` | This device's wrapped account keys, every version. |
| `GET /handoffs?project_index=…` · `GET /handoffs/{id}` · `POST /handoffs` · `DELETE /handoffs/{id}` | Ciphertext in, ciphertext out. |
| `GET /relay` | WebSocket upgrade to the account's Durable Object. |

## 5. Security model

This extends ADR 0001 and becomes ADR 0002 (`docs/adr/0002-cross-device-trust.md`) in slice 10b.

**Assets.** Terminal input and output of every session on every device (remote command execution); the handoff texts (they can hold secrets); the account key; each device's secret keys; the backend tokens; the device registry (who may approve and revoke).

**Trust boundaries.**
- The webview trusts only the loopback core, as today. It never sees a key, a token or a relay frame.
- The core trusts its keychain and the pinned public keys in `peers.json`.
- The relay and the backend are trusted for availability and for routing between a user's own devices. They are not trusted with content: everything a device sends another device is inside the Noise transport, and every blob at rest is sealed under the account key.
- GitHub is trusted to say who the user is at sign-in. Its token is used once and revoked.
- The user's own approved devices trust each other fully, because the user said so with the code in §6.2.

**Attackers in scope.** Someone who controls the backend (an insider, a compromise); someone with the user's GitHub account but none of their devices; someone who phishes the user into entering a device-flow code the attacker started (GitHub's own warning about the device flow, §9.1); someone on the network; another device of the same account that has been revoked; a web page (unchanged from ADR 0001).

**Rules.**
1. Every remote byte passes through the core's cloud module, behind Noise, over TLS, through a loopback API that keeps today's token, Host, Origin and CORS guards. No new listening socket. The CSP and capabilities are unchanged.
2. A device is only ever `approved` by another approved device, which signs the new device's public keys and seals the account key to them. The backend cannot approve, cannot forge an approval (no signing key) and cannot swap keys unnoticed: the code the user compares on both screens is derived from both devices' public keys, so a substituted key shows a different code.
3. A compromised or phished GitHub sign-in yields a new device and nothing else: a pending device gets no device list, no keys and no relay routing until approved, and every approved device shows it with a "Not mine" action that revokes it. The GitHub App asks for no permissions, so the phished token itself opens nothing on GitHub.
4. Mutations that change trust (approve, revoke, rename) carry an Ed25519 signature by an approved device and a timestamp; the backend verifies them against the registry. The access token alone cannot revoke a device.
5. Revocation rotates the account key: the revoking device mints version n+1, wraps it for every remaining approved device, and uploads the set with the revoke. The revoked device's tokens die, its socket is closed, and new blobs use n+1. Blobs under old versions stay readable by the remaining devices, which keep every version.
6. Remote input is authorised by the **host**, not the viewer: the host refuses `INPUT` and `RESIZE` from a peer whose `allow_input` is off, from any session that is paused, and from any peer not in `peers.json`. The host shows, for as long as a viewer is attached, who is watching and whether they typed in the last ten seconds, and can disconnect any viewer with one action. An approve-from-anywhere decision from a viewer (10g) goes through the same `permission_output` path and risk check as a local one, and gets its own security review as 3d did.
7. The relay never learns a key. It sees device ids, frame sizes and timing. The handoff blind index is an HMAC under the account key, so the backend cannot list a user's repositories.
8. Secrets live in the OS keychain. Nothing secret is ever written to `~/.marshell` or to a log. The redaction filter (`diagnostics/redact.rs`) runs on every handoff before it is sealed, and a handoff never includes a transcript, a recording or a hook body: only the fields in §6.4.
9. Marshell still never presses Enter: a remote viewer's `INPUT` carries the bytes they typed, and a handoff only ever fills a launcher field or a prompt line that the person sends.
10. Signed out, or with the keychain unavailable, the cloud module does not exist: no socket, no timer, no file. A test asserts it.

**Residual risks, stated.** A device that is physically taken while unlocked is that user; the keychain and OS login are the boundary, as with any desktop app. A user who loses every approved device loses the account key and so every handoff: §8 offers a recovery phrase. The relay can observe when the user works and from where; the onboarding line says so in one sentence. A phished sign-in by someone who has never used Marshell creates an empty account under their GitHub id with the attacker's device as its first; their own first sign-in then shows that device and offers "Not mine: start over", which wipes an account that has one device and no handoffs at once, and otherwise starts a seven-day reset that every approved device is told about and can cancel (§8, question 16).

## 6. Feature design

### 6.1 Sign in with GitHub

- **Why the device flow, not a loopback callback.** The device flow needs no listening port and no client secret anywhere near the device: a GitHub App's device flow takes only the client id, while the loopback alternative needs the client secret at the code exchange even with PKCE, so the exchange would run on the Worker and the app would still need an unauthenticated loopback listener next to the token-guarded API, or a registered `marshell://` scheme (designed, not registered). GitHub's own pages disagree: one says desktop apps should use the device flow, the other prefers the web flow with PKCE because a device-flow code can be phished (§9.1). The compensating control here is the approval model: a phished sign-in gets a pending device and nothing else (§5). The `gh` CLI tries the device flow first and falls back to a loopback web flow. Recommendation: the device flow, with the loopback web flow as a documented fallback that S13 tests, because an unregistered loopback port is documented for OAuth Apps only, not for GitHub Apps.
- **GitHub App setup (owner):** a GitHub App "Marshell", Device Flow enabled, "Expire user authorization tokens" on, no repository permissions and no account permissions (`GET /user` returns the numeric id, login and avatar for a user token; S13 confirms it needs no permission), not installable on repositories. The client id ships in the app; the client secret lives only in the Worker, where the token check and revoke endpoints need it.
- **Flow.** Account dialog → "Sign in with GitHub" → the core asks GitHub for a device code and shows "Enter **ABCD-1234** at github.com/login/device", with the code focused and copied on Enter and an "Open GitHub" button through the opener plugin (already allowed for `https://*`). The core polls GitHub at the given interval, honours `slow_down`, and times out with GitHub's expiry. On success it sends the user token and its device record to `POST /auth/github`, gets the backend tokens, stores them in the keychain, and the dialog reads "Signed in as @login · this is your first device: desktop" or "waiting for approval on one of your devices".
- **Tokens.** GitHub's user token (8 h) is used once: the Worker checks it against GitHub's token-check endpoint with the client secret, reads the user, and revokes it; the device never stores it. A device-flow token can be refreshed without the client secret (§9.1), so a later GitHub mirror could keep one on the device; this phase has no reason to. What the device refreshes is the backend's access token (1 h) with the rotating refresh token in the keychain, serialised so two refreshes never race, the new pair written before the old is dropped. A failed refresh (revoked, expired after 90 days unused) signs the device out with a one-line reason and leaves everything local as it was. The account is keyed by GitHub's numeric user id, never the login (a renamed login can be claimed by someone else) and never the node id (its format is being retired).
- **Where it lives.** "Account…" in the palette's Tools group beside Notifications… and Agents…, as the Diagnostics decision did; when the Settings page is live it becomes Settings → Account, with Devices as its second section. The onboarding gains no step: sign-in is never asked for.
- **Offline and signed out.** The cloud module is `Option<Cloud>`; `None` means today's app. Every cloud route answers 409 "Signed out" and the UI hides what does not apply (the "hidden, never disabled" rule of the session menu).

### 6.2 Devices

- **Registry.** At sign-in a device registers `device_id` (minted locally, ULID), name (hostname by default), OS, arch and app version with its two public keys. The dialog's Devices section lists every device: a device glyph, name, "this device", OS, online dot with words ("Online", "Last seen 2 h ago"), app version, and a menu: Rename, Remote control on/off (host-side), Remove. Keyboard: a list with roving tab stop; Shift+F10 opens the menu, as session rows do.
- **Approval (recommended trust model).** The first device of an account is approved on creation and mints the account key. Every later device is **pending** until one approved device approves it:
  1. The new device shows "Approve this device on desktop or laptop. Code **48 21 07**." The code is six digits from HKDF over both devices' Ed25519 public keys and the approval nonce.
  2. Every online approved device gets a `device_event: pending` and shows a card (in the lane's style, not a banner): "A new device wants to join: laptop · Windows 11 · Marshell 0.9. Its code should read **48 21 07**. Approve · Not mine." Approve seals the account key to the new device's X25519 key, signs its keys, and posts the approval; "Not mine" revokes it.
  3. The new device gets `approved` and `KEY_GRANT`, unwraps the account key into its keychain, and pins every peer's keys.
  If no approved device is online, the new device waits (15 minutes per approval, retried on demand) and says so with the last-seen time of each device. This is the shape of Bitwarden's log-in-with-device (a fingerprint phrase compared on both screens, requests that expire in 15 minutes), Signal's linked devices (the primary scans the new device's provisioning key) and Tailscale's tailnet lock (an existing trusted node signs the new node's key): the server introduces, the user confirms with a code that depends on the keys, and nothing is decryptable before that.
- **Revoke.** From any approved device. The revoked device is signed out at once if online (`bye: revoked`), its local keys are deleted on its next launch, and the account key rotates (§5 rule 5). "Remove" on *this* device is sign-out plus revoke.
- **Presence.** The Durable Object's socket set; a device is online while its socket is open. `last_seen` is written on connect and disconnect and every five minutes.

### 6.3 Remote sessions

- **Rows.** A remote session appears in the sidebar and rail like a local one, with a small device glyph in the right cluster (beside the mute glyph) and the device name in the tooltip and accessible name; line 2 reads "On desktop · Working · Editing src/auth.ts". With project grouping on, a remote session of the same project key sits in that project's group; otherwise remote sessions form a last group "On desktop". Rows keep the host's number. In forced colours the device glyph is drawn, not tinted, as the status glyphs are.
- **Open.** Clicking a remote row fills a pane as a local one does; the pane header adds "on desktop". The webview opens `/v1/pty/{device}:{tab}` and the viewer's core sends `ATTACH`; `OUTPUT`, `RESET` and `EXIT` come back as the same frames the local terminal speaks, so `TerminalView` and `PtySocket` are unchanged. A remote pane is read-only in 10d (its xterm keeps `disableStdin` and a quiet "View only · desktop" note in the pane footer where the prompt line would be); 10e makes it typeable.
- **Size.** The pty has one size. Rule: a session shown on screen on its **host** keeps the host's size, and a remote pane renders at that size (xterm at the host's cols and rows inside the pane; a smaller pane scrolls and says "Desktop's terminal is 120×40"). A session that is not on screen on the host takes the size of the viewer that shows it. The host wins ties. This is tmux's "the attached client sizes the window" with the owner first.
- **Reconnects.** The viewer's `PtySocket` already resumes from `processedUpTo`; the viewer core maps that to `ATTACH {from_seq}`; the host replays from its ring or sends `RESET`. A relay drop re-runs the Noise handshake and re-attaches every open viewer. A pane whose host went offline shows "desktop is offline" over the last screen and keeps it until the row is closed; the row stays, greyed like Ended, until the host's next `SESSIONS` frame drops it.
- **Several viewers.** Each viewer has its own cursor and 256 KiB window at the host. A viewer that falls behind gets a `RESET` catch-up and a one-line "Output skipped while catching up" in the pane; the host's reader never waits on a remote viewer.
- **Host indicator.** On the host, a watched session shows an eye glyph in the row cluster and in the pane header, "Viewed from laptop", becoming "Controlled from laptop" (a keyboard glyph) for ten seconds after each remote input. The session header gets a chip with Disconnect; the row menu gets "Disconnect viewers"; the palette and tray get "Disconnect all remote viewers". The tray tooltip adds "1 remote viewer". Nothing about a viewer is colour alone.
- **Peek** on a remote row asks the host for its last 30 rows (`PEEK_REQ`), rendered as today's Peek.
- **What does not cross.** Pause, Resume, Restart, Mode ▸, recordings, attachments, drop and paste of files, the shell panel, worktree tasks: all answer "Runs on desktop" and the row menu hides them. The viewer's own launcher can still start a *local* session in the same project (10f makes that a handoff).

### 6.4 Conversation handoff

- **While a session is open**, other devices use it through §6.3. Nothing is summarised while it runs.
- **Project identity.** The project key from §3.3. The launcher shows the key's display name under the folder chip ("github.com/user/repo"), and for a folder with no remote a "Same project as…" field that lists the account's assigned keys or creates one.
- **Who generates the summary (recommendation: a record always, a model only if asked).**
  - **The handoff record**, built by the host core with no model call, at every session close and at every `Stop` that the archive already refreshes, so a crash loses at most the turn in progress. Contents, in Markdown: project, device, agent, branch and last commit subject, started and ended, how it ended; the last five user prompts (first 300 characters each) and the final assistant message (first 1500 characters), both from the transcript the archive already copies, version-gated like everything that parses a transcript; files touched (from `PostToolUse`, §1.2 item 7); `git status --porcelain` counts; the resume command valid on the host (`claude --resume <id>` on desktop). Unknown fields are left out, never guessed. The redaction filter runs before sealing. Cost: zero tokens, which the default has to be (cross-cutting requirement).
  - **A smart summary**, opt-in per agent in the Account dialog ("Smart handoffs use your Claude plan"), generated on the host at close by the CLI's own headless mode with the session's cwd and a 120 s timeout: for Claude Code, `claude -p --resume <session id> "<handoff prompt>"` *(to verify in S16: whether `-p --resume` continues the conversation without forking, and what it costs with caching)*. Codex (`codex exec`) and Antigravity follow when their adapters land in phase 6. A failed or slow smart summary falls back to the record; the record always exists.
  - **Not** "the same agent before exit": Marshell never presses Enter, so it cannot make an agent summarise on close. What it can do is offer a "Hand off" action in the row menu that fills the prompt line with the handoff prompt for the person to send; the resulting answer becomes the smart summary when the `Stop` arrives. This is the cheapest high-quality path and stays within the rule.
- **Crash, sleep, kill.** The record is refreshed at every `Stop`, so the last complete turn is always on the backend. On the host's next launch, any session whose index row has no close record gets one from its archived transcript, marked `ended: crashed`. A sleeping PC simply resumes its relay socket; its sessions keep running.
- **Where each CLI keeps its transcripts** (`docs/research/2026-10-16-accounts-cloud-extensions.md` line 73, PLAN §7): Claude Code `~/.claude/projects/<encoded cwd>/<session id>.jsonl` (the hooks send the path; the archive copies it); Codex `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` plus SQLite; Antigravity `~/.gemini/antigravity-cli/brain/<conversation>/.system_generated/logs/transcript.jsonl` and `conversations/<uuid>.db`, read from the hook payload's `transcriptPath` when its adapter lands. Marshell reads them and never writes them.
- **Delivery to the next session (vendor-neutral, no repo pollution).** When the launcher opens on a project that has a newer handoff from another device than this device's last session there, it shows "Continue from desktop · 2 h ago · Preview", on by default. Launching with it on:
  - **Claude Code:** the handoff goes in through `--append-system-prompt` for that launch only *(flag name to verify in S16; the `context` hook role, which the bridge already has and the core answers with 204 today, is the alternative: `additionalContext` on the first `UserPromptSubmit`, capped at 10,000 characters)*. Nothing is written to any file.
  - **Codex CLI and Antigravity:** the core sets `MARSHELL_HANDOFF=<path to the local .md>` in the session's environment, and a two-line tagged block in the CLI's **user-level** instruction file (`~/.codex/AGENTS.md`; Antigravity's user-level file when its adapter is verified) says to read that file first when the variable is set. The block is installed through the config service's `MarkdownBlock` op with the consent card (preview, snapshot, undo), the same way hooks are, and removed by uninstall. The repository's own `AGENTS.md`, `CLAUDE.md` and `GEMINI.md` are never touched.
  - **Every CLI, visibly:** the launcher's first-prompt field is prefilled with "Read the handoff from desktop at <path> before starting" when the native route is unavailable (a manual CLI, a gated version), and the row menu has "Show handoff" which opens it in the preview pane. The phase-6 `marshell` CLI gets `marshell handoff show` so an agent can fetch it itself.
- **Storage (recommendation: our backend, not GitHub).** A secret gist is not private (anyone with the URL) and GitHub Apps have no Gists permission *(to verify)*; a private repository needs `contents: write` on a repository the user must install the app on, turns every handoff into a commit the user sees, and still needs the backend for the devices and the relay. The backend stores ciphertext under the account key with the blind project index; retention is the newest 20 per project and 90 days. The user-owned alternative is the brief's backup folder (BRIEF §5.10): "Export handoffs to a folder" writes the plaintext `.md` files to any synced folder, off by default. A GitHub mirror can come later if asked for.

## 7. Build order

Each slice is its own plan, branch and PR with a live check, as the phase-3 and phase-4 slices were. The live checks need two PCs for 10c onward; a second instance on one PC works for development through the `MARSHELL_HOME`-scoped single instance (§1.2 item 11). Phase 10 can start when 4d merges and run beside phase 5; 10f's Codex and Antigravity delivery waits for their adapters in phase 6.

| Slice | Ships | Exit |
|---|---|---|
| **10a Spikes and the GitHub App** | S12: `keyring-core` with the three stores on Windows (local persistence, the 2,560-byte cap), macOS (a debug build's prompts against a signed build) and Linux (with and without a Secret Service daemon), under tokio. S13: the device flow against a throwaway GitHub App: whether `GET /user` works with no permissions, whether revoking the access token also kills its refresh token, the token-check endpoint from a Worker, and whether a loopback redirect with an unregistered port works for a GitHub App (the fallback). S14: a toy Durable Object relay on the Hibernation API: auto-pong, 64 KiB frames at 5 MB/s, latency from this house, and the bill after a day. S15: the viewer fan-out under the S6 throughput test (`yes`, 50 MB `cat`) with one local and two lossy viewers. S16: `claude -p --resume` and `--append-system-prompt` behaviour and cost. S17: `snow` KK over a fake relay in a core test, and the dependency tree it brings. Outcome: go/no-go per dependency, written to `docs/spikes/`. | Every spike has a pass/fail line and its evidence. |
| **10b Sign in** | The Worker (`cloud/`): `/auth/*`, `accounts`, `devices` (register only), D1 migrations, `wrangler` deploy, MIT. The core's `cloud` module: keychain, device keys, the device flow, token refresh, `GET /v1/account`, `POST /v1/account/sign-in`, `/sign-out`, the `account` event. The Account dialog with the code screen. ADR 0002. | Sign in on one PC, restart, still signed in; sign out leaves the keychain empty; no outbound socket when signed out (test); the local app unchanged by the keyboard walkthrough. |
| **10c Devices** | The AccountHub Durable Object: hello, presence, device events, opaque relay. Noise KK over the relay; `peers.json`; approval with the code card; `KEY_GRANT`; revoke with key rotation; the Devices section: list, online, rename, Remote control switch, Remove. | Two PCs: approve the laptop from the desktop with matching codes; both online within 2 s of launch; rename shows on the other within 2 s; revoke signs the laptop out at once and rotates the key; a device approved while offline gets its key on its next connect. |
| **10d Remote view** | The viewer fan-out refactor (S15); `SESSIONS` frames; the remote mirror in the registry and the snapshot; `origin` and `viewers` on `SessionInfo`; remote rows and the "On desktop" group; `ATTACH`/`OUTPUT`/`RESET`/`EXIT`/`ACK`; the proxied `/v1/pty/{device}:{tab}`; read-only panes; Peek; the host's "Viewed from" indicator and Disconnect; "desktop is offline". | Live output from the desktop appears on the laptop within 200 ms on the LAN; pull the laptop's network for 10 s and the pane resumes with no gap or with one "Output skipped" line; a viewer stalled by a debugger never slows the host's `yes` test; the host sees and disconnects the viewer; nothing remote in `layout.json`. |
| **10e Remote input** | `INPUT` and `RESIZE` with the size rule; the host policy switch; "Controlled from"; remote rows in the needs-you count and lane (status only; decisions stay on the host); the security review against ADR 0002 before merge. | Typing round trip under 150 ms on the LAN; Remote control off on the host refuses input at the host, not the viewer; a paused session refuses; the indicator reads "Controlled" within one frame of the keystroke. |
| **10f Handoffs** | Project keys and `projects.json` v2; files touched; the record at close, at `Stop` and after a crash; redaction; sealing and the blind index; `/handoffs` routes; `handoff_event`; the launcher's "Continue from"; Claude delivery; `MARSHELL_HANDOFF` plus the user-level block for the others, through the consent card; "Show handoff"; export to a folder; smart handoffs for Claude, off by default. | Close on the desktop, launch on the laptop within a minute with "Continue from desktop", and the agent's first answer cites the handoff; kill the desktop app mid-turn, relaunch, and a `crashed` record arrives; a fake API key in the conversation is redacted in the record; `git status` of the repository is clean after every step. |
| **10g Later** | Remote approve cards (`DECISION`), banners for remote sessions, the GitHub mirror, a direct path (`iroh` or WebRTC) behind the same frames, the daemon. | Each its own slice. |

## 8. Open questions, with a recommendation each

All sixteen were answered on 2026-10-22 with the recommended option (PLAN.md §9, phase 10). The list stays as the record of the alternatives considered.

1. **Sign-in flow: device flow or a loopback callback?** Recommend the device flow (§6.1): no port, no secret on the device, no scheme registration, and the approval model absorbs the phishing risk GitHub's best-practice page raises. The loopback web flow stays as the fallback S13 tests.
2. **Who holds the GitHub token?** Recommend nobody: the Worker checks it once and revokes it, and the device keeps only Marshell's own refresh token in the keychain. A device-flow token can be refreshed without the client secret, so if a GitHub mirror of handoffs is ever wanted the device can hold one then.
3. **Backend?** Recommend Cloudflare (Worker, Durable Object per account, D1), open-sourced under `cloud/`, with the relay URL as an Advanced setting so the Rust-server option stays open for self-hosters.
4. **Transport?** Recommend the relay with Noise end to end; no WebRTC in v1 of this phase.
5. **New-device trust?** Recommend approval from an existing device with a key-derived code (§6.2), no server-side escrow, and an optional recovery phrase shown once at account creation (a BIP39-style phrase that unwraps the account key) so a user who loses every device keeps their handoffs. Without the phrase, losing every device loses the handoffs and nothing else.
6. **Remote control default?** Recommend on for approved devices (the user just typed a code to approve it), with the host-side switch per device, the always-on indicator and one-action disconnect. The alternative, off until enabled per device, is one more step every user will take anyway.
7. **Remote session numbers and keys?** Recommend keeping the host's number with the device name ("desktop · 6"), Ctrl+Shift+1–9 local only, and "Go to session" in the palette accepting "desktop 6".
8. **Should remote sessions ring here?** Recommend: they count in the needs-you count and lane from 10e, and banners for them are a switch, off by default, in 10g; both devices ringing for one request is worse than one.
9. **Who summarises?** Recommend the no-model record always, the "Hand off" prompt-line action for a human-sent smart summary, and headless smart summaries opt-in per agent (Claude first).
10. **Delivery for Codex and Antigravity?** Recommend `MARSHELL_HANDOFF` plus a two-line block in the CLI's **user-level** instruction file through the consent card. The alternatives are a prefilled first prompt (visible, but it spends a turn) or waiting for phase 6's `marshell` skill. Never the repository's files.
11. **Where do handoffs live?** Recommend the backend's ciphertext store with the blind index; export to a synced folder as the user-owned option; GitHub later if asked.
12. **Retention and limits?** Recommend the newest 20 handoffs per project and 90 days; 10 devices per account; a relayed-bytes cap per month with a clear message when hit (the number from S14's measurements).
13. **Who runs and pays for the backend?** Recommend the owner's Cloudflare account for the official relay, the costs in §9.2 being small; the open-source Worker plus the relay-URL setting is the answer for anyone who wants their own.
14. **Where in the roadmap?** Recommend starting 10a after 4d, running 10b–10e beside phase 5, and 10f after phase 6's adapters so Codex and Antigravity delivery is verified against real installs rather than designed blind.
15. **Local plaintext handoffs on disk?** Recommend yes, owner-only like transcripts and recordings (the cross-cutting requirement), because the agent has to read a file and the CLI's own transcript is plaintext beside it. The backend copy is always sealed.
16. **What does "Not mine" do at a first sign-in that finds an account?** Recommend: wipe at once when the account has one device and no handoffs (the squatting case has nothing to lose), otherwise a seven-day reset announced to every approved device and cancellable from any of them. The stalemate where an attacker's device keeps cancelling is handled by the owner by hand and recorded as a residual risk in ADR 0002.

## 9. External facts and sources

Gathered on 2026-10-22 from the vendors' pages and repositories by two research passes; "est." marks the researcher's own estimate. Anything not listed here is an assumption to settle in 10a.

### 9.1 GitHub App sign-in and tokens

- **Device flow.** A GitHub App needs "Enable Device Flow" ticked in its registration, else polling answers `device_flow_disabled`. `POST https://github.com/login/device/code` takes `client_id` (and optionally `scope=offline_access`, the only scope GitHub Apps accept); the app polls `POST https://github.com/login/oauth/access_token` with `client_id`, `device_code` and `grant_type=urn:ietf:params:oauth:grant-type:device_code`. Defaults: an 8-character user code, 900 s to enter it, a 5 s interval, `https://github.com/login/device`. Errors: `authorization_pending`, `slow_down` (add 5 s), `expired_token`, `access_denied`, `incorrect_device_code`. No client secret is sent. [Generating a user access token](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app), [Modifying a GitHub App registration](https://docs.github.com/en/apps/maintaining-github-apps/modifying-a-github-app-registration).
- **Token lifetimes.** "Expire user authorization tokens" is on by default: the user access token (`ghu_`) lasts 8 h and the refresh token (`ghr_`) 6 months. A refresh (`grant_type=refresh_token`) returns a new pair and kills the old one. `client_secret` is required on refresh "unless the user access token was generated using the device flow". An expired refresh token answers `bad_refresh_token`. [Refreshing user access tokens](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens).
- **Identity.** `GET /user` is on the list of endpoints a GitHub App user token may call, and GitHub's own CLI tutorial calls it after the device flow; that it needs no app permission is not stated anywhere (S13). `/user/emails` needs the Email addresses permission. The numeric `id` (int64) is the durable key: a renamed `login` "becomes available for anyone else to claim", and the legacy node id format is being retired. [Endpoints available to user access tokens](https://docs.github.com/en/rest/authentication/endpoints-available-for-github-app-user-access-tokens), [Building a CLI with a GitHub App](https://docs.github.com/en/apps/creating-github-apps/writing-code-for-a-github-app/building-a-cli-with-a-github-app), [Changing your username](https://docs.github.com/en/account-and-profile/setting-up-and-managing-your-personal-account-on-github/managing-user-account-settings/changing-your-github-username).
- **Server-side token endpoints.** `POST /applications/{client_id}/token` checks a token, `DELETE` on it revokes the token, `DELETE …/grant` revokes the whole authorization; all take Basic auth with the client id and secret, so they belong on the Worker; an invalid token answers 404. A user's own revocation reaches the app through the `github_app_authorization` webhook, or as 401 on next use. [OAuth and GitHub App token endpoints](https://docs.github.com/en/rest/apps/oauth-applications), [Token expiration and revocation](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/token-expiration-and-revocation).
- **Web flow and PKCE.** PKCE (`S256` only) is supported and "strongly recommended", but the code exchange still requires `client_secret`. GitHub's best-practices page says a native app "cannot secure your client secret", prefers "the authorization code with PKCE over the device flow" because the device flow lets an attacker impersonate the app in phishing, and reserves the device flow for "CLIs, IoT devices, or headless systems"; the token-generation page says "desktop applications should use the device flow". A loopback `redirect_uri` with an unregistered port is documented for OAuth Apps only; a GitHub App's callback must match one of up to ten registered URLs with host and port exact. [Best practices for creating a GitHub App](https://docs.github.com/en/apps/creating-github-apps/about-creating-github-apps/best-practices-for-creating-a-github-app), [About the user authorization callback URL](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/about-the-user-authorization-callback-url), [Authorizing OAuth apps](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps).
- **`gh`.** An OAuth App with an embedded client id and secret; `DetectFlow` tries the device flow first and falls back to a web flow on `http://127.0.0.1:<port>`; no PKCE in `cli/oauth`. [cli/cli flow.go](https://github.com/cli/cli/blob/trunk/internal/authflow/flow.go), [cli/oauth](https://github.com/cli/oauth).
- **Tauri.** `tauri-plugin-opener` 2.7.2 is in the tree and opens `https://` URLs; `tauri-plugin-deep-link` 2.6.2 exists if a scheme is ever needed (on Windows and Linux the OS starts a second process with the URL as an argument, so it pairs with the single-instance plugin's `deep-link` feature; on macOS the scheme cannot be registered at runtime). [Opener](https://v2.tauri.app/plugin/opener/), [Deep linking](https://v2.tauri.app/plugin/deep-linking/).

### 9.2 Backend costs at small scale

- **Cloudflare Workers.** Paid plan $5 a month minimum with 10M requests, then $0.30 per million; WebSocket messages through a Worker are not requests, only the upgrade is. [Workers pricing](https://developers.cloudflare.com/workers/platform/pricing/).
- **Durable Objects.** Paid: 1M requests included, then $0.15 per million; 400k GB-s of duration included, then $12.50 per million GB-s, billed at a fixed 128 MB. Incoming WebSocket messages count 20:1 (100 messages are 5 requests); outgoing messages and protocol pings are free. An object on the Hibernation API accrues no duration while idle; a plain `accept()` bills the whole connection; pending I/O can keep an object awake for up to 15 minutes. Cloudflare's own worked example, 100 objects with 100 hibernating sockets each at one message a minute, comes to $20.65 a month; the same without hibernation at 50 sockets, $142.95. A free tier exists for SQLite-backed objects (100k requests a day, hard failure beyond). Limits: a soft 1,000 requests a second per object, 32 MiB per WebSocket message (raised 2025-10-31), 16 KiB per hibernation attachment, 10 GB of SQLite per object. [Durable Objects pricing](https://developers.cloudflare.com/durable-objects/platform/pricing/), [limits](https://developers.cloudflare.com/durable-objects/platform/limits/), [WebSocket best practices](https://developers.cloudflare.com/durable-objects/best-practices/websockets/), [changelog 2025-10-31](https://developers.cloudflare.com/changelog/post/2025-10-31-increased-websocket-message-size-limit/).
- **D1 and R2.** D1 free: 5M rows read and 100k written a day, 5 GB; paid: 25B reads and 50M writes included, then $0.001 and $1 per million, $0.75 per GB-month past 5 GB, no egress charge. R2: $0.015 per GB-month, free egress. [D1 pricing](https://developers.cloudflare.com/d1/platform/pricing/), [R2 pricing](https://developers.cloudflare.com/r2/pricing/).
- **Estimate inputs (est.).** 4 KB average message, 50 MB of relayed output per user per day, 5 ms of handler time per message, one object per account. At 100 users: 38.4M incoming messages a month, 1.92M billable requests, about $0.14 over the allowance, about 24k GB-s of duration, about $5.2 in all; with objects kept awake eight hours a day, about $135. At 1,000 users: about $8, or about $1,350 awake.
- **Supabase.** Free $0 (500 MB database, 200 concurrent Realtime connections, 2M messages a month, 100 a second); Pro from $25 (500 connections, 5M messages then $2.50 per million, 500 a second; without the spend cap 10,000 connections and 2,500 a second); Team from $599. A broadcast counts once when sent and once per receiving subscriber. Binary broadcast needs supabase-js 2.91 or supabase-swift 2.44; no Rust client was found. GitHub sign-in is an OAuth App flow; a GitHub App as provider is undocumented; provider tokens are "intentionally not stored" and never refreshed. At terminal message rates the free tier covers about three users and Pro lands near $205 a month for 100 (est.). [Pricing](https://supabase.com/pricing), [Realtime limits](https://supabase.com/docs/guides/realtime/limits), [Realtime pricing](https://supabase.com/docs/guides/realtime/pricing), [Broadcast](https://supabase.com/docs/guides/realtime/broadcast), [Social login](https://supabase.com/docs/guides/auth/social-login).
- **A VM.** Fly.io: shared-cpu-1x at $2.19 (256 MB), $3.69 (512 MB) or $6.70 (1 GB) a month, shared-cpu-2x with 2 GB at $13.39, a dedicated IPv4 $2, egress $0.02 per GB in North America and Europe, no free allowance for new organisations. Hetzner: CX23 (2 vCPU, 4 GB, 20 TB traffic) €5.49 to €5.99 after the 2026-06-15 price rise, IPv4 €0.50; the cost-optimised page showed every plan unavailable on the day. [Fly pricing](https://docs.fly.io/about/pricing), [Hetzner price adjustment](https://docs.hetzner.com/general/infrastructure-and-availability/price-adjustment/), [Hetzner cloud](https://www.hetzner.com/cloud/cost-optimized/).
- **TURN and P2P.** Cloudflare TURN $0.05 per GB of egress, STUN free (a 1,000 GB free allowance appears on the FAQ only); Twilio TURN $0.40 per GB. iroh 1.4.1 (1.0 on 2026-06-15): QUIC with Ed25519 node keys, relays see metadata only, public relays are for development, Pro $19 a month for 100 GB at 5 MB/s, a dedicated relay $199 a month per region, no audit mentioned. [Cloudflare TURN FAQ](https://developers.cloudflare.com/realtime/turn/faq/), [Twilio STUN/TURN](https://www.twilio.com/en-us/stun-turn/pricing), [iroh security](https://docs.iroh.computer/concepts/security-privacy), [iroh pricing](https://www.iroh.computer/pricing).

### 9.3 Crates and libraries

- **Keychain.** `keyring` 4.2.0 (2026-08-29) is a thin wrapper; apps wanting control depend on `keyring-core` 1.0.1 plus store crates. Windows (`windows-native-keyring-store`): Credential Manager, a 2,560-byte secret limit, enterprise persistence by default (can roam), a `persistence` modifier for local, calls serialised by a named mutex. macOS (`apple-native-keyring-store`): the item's ACL is the creating app's designated requirement, so unstable signing identities and debug versus release builds prompt; the `protected` feature is the data-protection keychain and needs a provisioning profile. Linux (`zbus-secret-service-keyring-store`): needs a running Secret Service daemon; an app with a tokio runtime should enable `rt-tokio-crypto-rust`. No async API; `keyring-core` is thread-safe but stores may not handle one credential across threads. The keyutils store is memory only and cleared on reboot. `tauri-plugin-stronghold` 2.4.0 is an encrypted vault that still needs a password kept somewhere. [keyring-rs](https://github.com/open-source-cooperative/keyring-rs), [windows store](https://github.com/open-source-cooperative/windows-native-keyring-store), [apple store](https://github.com/open-source-cooperative/apple-native-keyring-store), [zbus store](https://github.com/open-source-cooperative/zbus-secret-service-keyring-store), [CREDENTIALW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentialw).
- **Noise.** `snow` 0.10.0 (2025-07-19), Noise revision 34, KK among the supported patterns, the `fallback` modifier not implemented, no formal audit; `libp2p-noise` 0.47 builds on it with the ring resolver. `magic-wormhole` 0.8.1 uses `noise-protocol` for transit instead. [snow](https://github.com/mcginty/snow), [patterns](https://docs.rs/snow/latest/snow/params/enum.HandshakePattern.html).
- **RustCrypto and dalek.** `x25519-dalek` 3.0.0 and `ed25519-dalek` 3.0.0 (2026-07-06); the 2019 Quarkslab audit covered `curve25519-dalek` with a brief look at the two; CVE-2024-58262 was a timing issue in `curve25519-dalek` below 4.1.3, so the lockfile pins above it. `chacha20poly1305` 0.11.0 (2026-06-28), audited by NCC Group with no significant findings. `hkdf` 0.13.0 (2026-03-30) and `sha2` 0.11.1 (2026-10-12), audit status unverified; the tree has `sha2` 0.10. [Quarkslab audit](https://blog.quarkslab.com/security-audit-of-dalek-libraries.html), [chacha20poly1305](https://docs.rs/crate/chacha20poly1305/latest).
- **age.** 0.12.1 (2026-07-14), tagged beta, no audit mentioned, multiple recipients; a candidate format for the wrapped account key if hand-rolled sealing is not wanted. [rage](https://github.com/str4d/rage).
- **WebRTC.** `webrtc` 0.21.1 (2026-10-11), pre-1.0, now a thin async layer over the sans-IO `rtc` crate; `str0m` is sans-IO with data channels and no TURN client. WebView2 has had `RTCPeerConnection` since Edge 79; WebKitGTK's story is uncertain (bug 274711 closed after the move to libwebrtc in July 2026; the 2.52 notes describe GstWebRTC). [webrtc-rs](https://github.com/webrtc-rs/webrtc), [str0m](https://github.com/algesten/str0m), [caniuse](https://caniuse.com/rtcpeerconnection), [WebKitGTK 2.52](https://webkitgtk.org/2026/03/18/webkitgtk-2.52-highlights.html).
- **Already in the tree.** `axum` 0.8.9 (2026-04-14), `tokio-tungstenite` 0.30.0 (2026-07-11).

### 9.4 Precedents

- **tmate.** Its public servers shut down in July 2025; a Rust server rewrite is planned; self-hosting needs `tmate-ssh-server`. [issue #336](https://github.com/tmate-io/tmate/issues/336).
- **Upterm.** The host's SSH server reverse-tunnels to `uptermd`; guests join by `ssh` or WebSocket; end-to-end encryption is not stated; the host sees join and leave notices and `upterm session info`, can hide client IPs, and can restrict guests by authorised keys or GitHub users. [upterm](https://github.com/owenthereal/upterm).
- **Warp session sharing.** Output goes through Warp's servers; end-to-end encryption is not stated; viewers sign in; edit access needs the host's approval; participants see each other's avatars and cursors; secret redaction does not apply to shared sessions; data expires after about a week. [Warp docs](https://docs.warp.dev/agents/local-agents/session-sharing/).
- **Tailscale.** WireGuard end to end between node keys; DERP relays forward ciphertext only; the host knows the peer's identity; device approval by an admin (a new device passes no traffic until approved); tailnet lock has an existing trusted node sign the new node's key. [DERP](https://tailscale.com/kb/1232/derp-servers), [Tailscale SSH](https://tailscale.com/kb/1193/tailscale-ssh), [device approval](https://tailscale.com/kb/1099/device-approval), [tailnet lock](https://tailscale.com/kb/1226/tailnet-lock).
- **Bitwarden and Signal.** Bitwarden's log-in-with-device sends a request to the logged-in apps, the user checks a fingerprint phrase on both and confirms on the trusted device, requests expire after 15 minutes. Signal's new device shows a QR code carrying a provisioning address and public key that the primary phone scans; five linked devices. [Bitwarden](https://bitwarden.com/help/log-in-with-device/), [Signal](https://signal.org/blog/a-synchronized-start-for-linked-devices/).
- **Flow control.** xterm.js parses about 5 to 35 MB/s with a hard 50 MB write buffer, and its guide has the client acknowledge output back to the pty side for remote terminals. VS Code pauses the pty at 100,000 unacknowledged characters, resumes at 5,000 and acknowledges every 5,000. Mosh sends frames every 20 to 250 ms with a 100 ms delayed ACK. Marshell's local path (8 ms flush, 64 KiB, 1 MiB window) and the 256 KiB remote window in §4.2 sit inside that range. [xterm.js flow control](https://xtermjs.org/docs/guides/flowcontrol/), [VS Code terminal.ts](https://github.com/microsoft/vscode/blob/main/src/vs/platform/terminal/common/terminal.ts), [Mosh transportsender.h](https://github.com/mobile-shell/mosh/blob/master/src/network/transportsender.h).
