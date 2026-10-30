# ADR 0001: Threat model for the local API, hooks and config writes

- Status: accepted
- Date: 2026-10-18
- Slice: 3b (PLAN §9, "Slice 3b decisions")

## Context

Marshell runs AI coding-agent CLIs in terminals it owns, listens on a loopback HTTP and WebSocket API, receives hook calls from those CLIs, and from slice 3c on writes their configuration files. It can type into a running agent and can change what an agent is allowed to do. Each of those is worth attacking, so this record lists what Marshell protects, from whom, how each entry point is guarded today, and what stays open.

## Assets

1. **The CLIs' configuration**: `~/.claude/settings.json`, project `.claude/settings*.json`, `~/.codex/config.toml` and the like. A hostile change can grant an agent permissions or run a command on every hook.
2. **The API token and `run/endpoint.json`**: whoever has the token can do anything the UI can.
3. **Terminal input**: text sent to a session reaches an agent that can run commands.
4. **Permission decisions**: an answer to a held permission request lets an agent act.
5. **The Marshell home** (`~/.marshell`): settings, layout, recordings, snapshots of config files, `managed.json`.
6. **The instruction library** (phase 7): text injected into an agent's instructions.

## Attackers

In scope:

- **Another local user** on the same machine, reading or writing files under this user's home, or calling the loopback port.
- **A web page** in any browser on the machine, through cross-origin requests, DNS rebinding, or a `marshell://` link.
- **Repository content** an agent reads (README, issues, code comments) that tries to steer the agent into calling Marshell or editing config.
- **A hostile `marshell://` link** from a page, a chat message or a document.

Out of scope:

- Malware already running as the same user. It can read the token, the CLIs' own credentials and every file Marshell can; nothing in a user-level app stops it.
- An administrator of the machine.
- The agents' own model behaviour and their own sandboxes, which belong to each CLI.

## Entry points and controls

### Loopback API (`crates/core/src/server/`)

- Binds `127.0.0.1` on a random port only (`server::start`).
- `host_guard` (`server/guard.rs`) runs before CORS: `Host` must appear once and equal `127.0.0.1:<port>`, which blocks DNS rebinding.
- `auth_guard` (`server/guard.rs`): `Origin`, when present, must appear once and be on the allowlist (the app's own origin), which blocks other web pages. Then the per-launch token must match, from `Authorization: Bearer` or a `token.<t>` WebSocket subprotocol, compared in constant time (`auth::tokens_match`).
- The token is 32 random bytes from the OS, new every launch (`auth::new_token`). It is published only in `run/endpoint.json` inside the Marshell home, written atomically (`auth::write_endpoint_file`): mode 0600 from creation on Unix, and on Windows covered by the home's owner-only ACL (below), which is set before the file is written.
- Bodies are capped per route (`DefaultBodyLimit`, 1 MiB for `/v1/hook`, the image cap for attachments).
- The webview's CSP (`src-tauri/tauri.conf.json`) lets it connect only to the app itself, its IPC and `127.0.0.1`.

### `/v1/sessions/{tab}/input`, paste and paste-paths

- Token only; anyone with the token can type into any session. That is the point of the route (the UI and the hook bridge use it), so the token is the boundary.
- Marshell never presses Enter on its own. Prompts, pastes and dropped paths are placed in the input, and the person sends them.
- Input sent through the API counts as typing: it releases a held permission request, so a hook can't hold an answer open while text is pushed in behind it.

### Hook endpoint (`/v1/hook`) and the bridge (`crates/hook`)

- The bridge finds the core through `endpoint.json` and talks only to `127.0.0.1`, with no proxy and no redirects (`crates/hook/src/post.rs`).
- Token required; the body is capped at 1 MiB.
- The bridge always exits 0. It prints nothing to the agent except the answer to a held `PermissionRequest`, and only in the shapes the core gives (slice 3d, below). A Marshell that isn't running is ignored, and other failures go to the bridge's log file. A missing or broken Marshell never blocks or changes the agent's own behaviour.

### Permission holds (`hooks/holds.rs`, `server/permissions.rs`)

- A `PermissionRequest` waits for an answer from the UI, for typing, or for its time limit; every way it ends removes it under one lock (`hooks/holds.rs`), so only one answer wins.
- The agent's own dialog stays visible the whole time, and typing in the terminal releases the hold. Terminal reports (focus, mouse motion, replies to queries) don't: `server/reports.rs` recognises them and `server/pty_ws.rs` passes them through without releasing.

### Config writes (`crates/core/src/config/`, slice 3b)

- One service writes every CLI config file. Each write is a ChangeSet that is first previewed as a diff. Apply needs the file hashes the preview showed, and refuses with a conflict if any file changed since, or if a previewed file changed outside Marshell (the watcher marks the preview stale).
- The write guard (`config/guard.rs`) allows only the user's home, project folders the person opened in Marshell, and `<marshell home>/agents/`. It always refuses the managed and organization paths (Program Files, ProgramData, the Windows folder, `/etc`, `/Library`, and the files `managed-settings.json`, `managed-mcp.json`, `managed_config.toml` anywhere), even inside an allowed folder. A link is followed and its target is checked too. A path with a `..` part is refused outright, because Windows resolves `..` by the text before it opens a file while a link resolves it on disk, and the two can name different files.
- JSON is edited through a CST, so comments and the rest of the file are kept; a file that doesn't parse is refused rather than rewritten.
- Every apply takes a snapshot first, and a restore is itself a preview the person applies.
- Marshell never asks for elevation.
- The token is still the boundary: a caller with the token can preview and apply. The guard limits where, and the 3c consent card limits what the person is asked to approve.

### History (`crates/core/src/history/`, slice 3e1)

- The archive copies a Claude transcript named by a hook body: `transcript_path` and `session_id` come from a token-holding caller (the bridge), and are checked before any filesystem call. A `session_id` must be 1–64 characters of `[A-Za-z0-9_-]` and start with a letter or digit, so it can't read as an option (`history::valid_session_id`). A `transcript_path` must be absolute, not a UNC or device path (`hooks::risk::is_remote_path`), and named exactly `<session_id>.jsonl` (`history::transcript_allowed`). A body that fails either check is left out of the history; the hook itself is answered as before.
- So a copy can only read a local file named `<id>.jsonl`. It can't be pointed at an arbitrary file, a share or a device. The path may still lead anywhere on local disks, and a link in it is followed, but a caller with the token already acts as the user.
- Archives (`history/archive/<agent>/<id>.jsonl.zst`), their `.meta.json` sidecars and `history/index.sqlite` live in the Marshell home, so they are owner-only like the rest of it (below). They are compressed, not encrypted: a transcript holds whatever the person and the agent wrote, secrets included, as Claude's own copy does. Encrypting data at rest is out of scope while same-user malware is out of scope.
- The check runs when the hook body arrives and again before Resume or Restart uses an indexed row, because index rows and sidecars live in the Marshell home and are not trusted just for being there: a row that fails it is treated as unknown, before any filesystem call, and a rebuild skips sidecars that fail it. Resume passes only an id that passed the check, as its own argument (`--resume <id>`), and refuses when Marshell holds an archive of the indexed transcript but Claude's file is gone, rather than fall back to another conversation. A session that never wrote a transcript starts fresh, with no resume flag.
- The index is rebuilt from the sidecars when it fails its integrity check, and the bad file is kept as `index.sqlite.broken-<ms>`. No transcript is parsed.

### Diagnostics (`crates/core/src/diagnostics/`, slice 3e2)

- **The routes are behind the token.** `GET`/`PUT /v1/diagnostics/logging`, `GET /v1/perf`, `POST /v1/diagnostics/export` and its two `GET` routes (`/file?name=`, `/zip`) sit behind the same host and auth guards as the rest of the API. The live check saw 401 with no token and with a wrong one on every route, and 403 for a foreign `Origin` with a valid token. An export is one slot with a random 128-bit id, replaced by the next one and gone after 15 minutes; an unknown, expired or replaced id and a name that is not in the export all answer the same 404. The file route finds the name in the list held in memory, so it never opens a path.
- **Redaction is best-effort, and the preview is the control.** Keys, tokens, `Bearer` and `Basic` authorization values, quoted secret values and secret flags, URL credentials, the profile folder in its long and available short (8.3) spellings (as `~`), the Marshell home and the user name are replaced by pattern, in the log as each line is written and in every other file as the export is built; profile env values are dropped whatever they look like. A secret in a shape the patterns don't know passes through. So the Diagnostics dialog lists every file with its size, shows any of them in full before anything is saved, and the zip is the previewed bytes.
- **The save path comes only from the native dialog, opened from Rust.** The webview fetches the zip from the core and hands the bytes to the `save_diagnostics` command, which accepts only a `marshell-diagnostics-YYYY-MM-DD.zip` name and bytes that start like a zip, opens the Save dialog itself, and writes `<path>.part` then renames it. The webview has no file or dialog permission (`src-tauri/capabilities/` grants neither), and no core route takes a path to write: a stray `path` field on the export request is ignored and writes nothing.
- **Logs live in the owner-only home and are redacted per line.** `logs/marshell.log` (2 MiB, five files) is inside the Marshell home, so it has the home's permissions, and each line is redacted before it reaches the file; the crash and bridge logs are written as they are and are redacted when they are put in an export, like every other file (the main log a second time). Marshell's crates log at info, and at debug only while Detailed logs is on, which a restart turns off. Nothing the core logs today is a transcript, a prompt or a hook body, and a debug line added later must not carry one.
- **The export never includes transcripts, recordings or hook bodies.** It is a fixed list: a README, `about.json` (versions), `settings.json`, `profiles.json`, `agents/claude.json` (setup state), `perf.json` and the three logs. History archives, recordings and hook events are not read by the export code.

### The `marshell://` link (phase 4)

- It carries no authority: it can only focus Marshell and select a tab. It never sends input, answers a permission request or applies a change. Registering it is a setting that is off by default.
- Built in slice 4e (`src-tauri/src/link.rs`):
  - **Two shapes.** Only two shapes parse: `marshell://show` and `marshell://focus/<tab>`, where the tab id matches `^[A-Za-z0-9_-]{1,64}$`. The scheme and the word are read in any case, and one trailing `/` is allowed. Anything longer than 128 bytes or not ASCII is refused, and so is anything else, rather than trimmed.
  - **Where it arrives.** A link arrives in two ways. One is a toast's in-process activation. The other is a second launch, where only a lone `argv[1]` is read and the rest of the command line is never looked at. A cold start launched with a link starts Marshell normally and ignores the link. One click reported both ways selects once: the same link is ignored for 2 s.
  - **Registration.** The "Open sessions from the notification centre" switch registers the link under `HKCU\Software\Classes\marshell`, pointing at the running exe. Its state is read from the registry and never stored. Turning it off removes the key only when the key points at this exe. The uninstaller removes it on the same condition, and not during an update.
  - **What a hostile link can do.** A page or document that gets the person to open a link can at most bring Marshell forward and select a session whose random tab id it already knows. Browsers ask before they open an external protocol, but a site the person once told to "always allow" can fire one while they type.
  - **Where the keyboard lands.** Every opened link, and every banner click, first tells the webview, which moves focus to a session's row (the linked one, else the selected one, else the main area), and only then does the window come forward. No terminal takes focus on its own until the person picks a session or a pane, so typing meant for another window cannot answer an agent's prompt. The order between that event and the window's activation is not guaranteed by the OS: in the worst case a keystroke typed in the same few milliseconds reaches the terminal that last had focus.

### Instruction library (phase 7)

- Library text injected into an agent's instructions is a prompt-injection channel. The risk is named here and is designed in phase 7: content is shown before it is added, and written only through the config service.

## Owner-only Marshell home (`crates/core/src/private.rs`)

On every start, before `endpoint.json` is written, the core makes the Marshell home owner-only:

- **Windows:** a protected DACL that grants full control to the current user and SYSTEM only, inherited by every file and folder inside. When the home's DACL is anything else, it is reset and every child goes back to inheriting it.
- **Unix:** folders 0700, files 0600, and files with the owner's execute bit 0700 (so `bin/marshell-hook` still runs). Links inside the home are skipped, never followed.

Only the home's own DACL or mode is checked on each launch, so a launch never walks the recordings and attachments; the children are fixed when the home itself is found wrong.

This keeps other local users away from the token, the recordings and the config snapshots, including when `MARSHELL_HOME` points outside the user profile. A failure is logged and the launch goes on, since the default home under the user profile is already private to the user.

## Reviewed for 3d: approve from anywhere (2026-10-20)

Slice 3d lets the needs-you card answer a held `PermissionRequest`. Before it shipped, an Opus security review of `phase-3d` against this record covered:

- the decision and release routes;
- the `permissions` frame and the peek route behind the guards;
- what a same-user process or a web page can do with them;
- keyboard safety on the card;
- the risk check's bypasses;
- the no-tab rule;
- the quoting of the `PermissionRequest` hook command;
- whether Peek shows a screen to anyone without the token.

### What 3d adds

- `GET /v1/permissions`, `POST /v1/permissions/{rid}/decision` and `/release`, the `permissions` frame on the events socket, and `GET /v1/sessions/{tab}/peek`. All of them sit behind `host_guard` and `auth_guard` like every other route, so a web page can't reach them and a holder of the token can do nothing it couldn't already do by typing into the session.
- The card answers only while it has focus. Keys typed in a terminal never reach a decision, the card's buttons do nothing for 500 ms after it takes focus or its content changes, Enter never allows, and typing in the pane releases the hold instead of answering it. A command too long to show in full must be expanded before Allow acts.
- The risk check (`hooks/risk.rs`, `agents/claude/request.rs`) marks a risky request. A risky card needs a hold to allow and offers no Always. The check is a warning, not a gate: Claude's own dialog stays up in the terminal the whole time.
- A request with no Marshell tab (a Claude run outside Marshell) gets 204 at once, and the bridge doesn't post such a request at all, so an outside session is never held or answered by Marshell.

### Findings fixed

- **A stale port could be answered by an impostor.** After Marshell exits, a process that outlives its session (a child that broke away from the job, or a detached program that inherited `MARSHELL_TAB_ID`) could still run the bridge against the port named in the old `endpoint.json`, and another local user who bound that port could answer the request. The bridge now posts only from a Marshell tab and prints only the answer shapes the core gives: allow, deny, or allow with rules the request itself suggested. So an impostor can't rewrite a command or add its own rules (6291370). The core also writes its PID into `endpoint.json` and deletes the file on a clean shutdown. The bridge skips a file whose PID names no running process (1532fc0).
- **A command the parser couldn't read counted as safe.** It now reads as risky, so a quoting trick that defeats the parser gets the guarded card (2538a6c).
- **Writes to tool config inside the project counted as ordinary edits.** `.claude/`, `.git/`, `.vscode/`, `.codex/` and `.mcp.json` run code on their own, so a write there is risky even inside the project (2538a6c).
- **Checking whether a Write target existed could leak credentials.** Checking a UNC or device path (`\\host\share`, `\\?\`, `\\.\`) can open a network connection and send the user's NTLM credentials to a host the repository chose. The request view now makes no filesystem call on such a path and reads it as "Writes outside this project" (2538a6c).

### Deferred, with their residual risk

- **Other ways to slip past the risk check.** Aliases, `eval`, and interpreters given inline code (`python -c`, `node -e`) aren't classified. An obfuscated command shows as a plain card, and Claude's own dialog still asks.
- **The server doesn't refuse a project-scope Always** on a risky request or on one with no rule. The card never offers Always in either case, so only a holder of the token can send it.
- **Tab attribution trusts the `MARSHELL_TAB_ID` a process inherits.** A request is matched to a tab by that variable, not by Claude's `session_id`, so a Claude started from inside another tab's process tree shows its requests on that tab.
- **Space on a focused Allow button of a plain card** activates it as any button does. Risky cards need a hold, so Space can't allow those.
- **The PID check can be fooled by PID reuse.** If the core's PID now belongs to another process, the bridge treats the file as live. That needs a crash, which leaves the file behind, and then a stale-port impostor, so it is accepted.

## Reviewed for 4e: the `marshell://` link and Marshell's own toasts (2026-10-23)

Slice 4e builds Windows toasts in the shell, adds the `marshell://` link and its switch, and turns `StopFailure` into an Error row. Before it shipped, an Opus review of `phase-4e` against this record covered:

- the link's parsing (Review Focus 1 of the plan: traversal, queries, extra arguments, a 10 KB argument, case, a trailing `/`, a link that is not the first argument);
- what a hostile link can do through a focus change;
- the toast XML (built through the DOM, never from a string), its activation handler and the AUMID;
- the registry code, the switch and the uninstaller hook;
- the `cfg` split that keeps macOS and Linux on the notification plugin.

### Findings fixed

- **A link could put the keyboard in an agent's terminal.** Bringing the window forward let the selected session's terminal take focus, so a page that fired a link while the person typed (a site once told to "always allow" the protocol) could land the rest of their typing, Enter included, in an agent's prompt. Every link and every banner click now tells the webview first, focus goes to a session's row, and no terminal takes focus on its own until the person picks a session or a pane (f99150c). The phase 4 exit check confirmed it on an installed build (`docs/research/2026-10-23-phase-4-exit-check.md`).

### Deferred, with their residual risk

- **The event and the activation race.** Windows does not order the webview's event against the window's activation, so a keystroke typed in the same few milliseconds can still reach the terminal that last had focus.
- **`AllowSetForegroundWindow(ASFW_ANY)`.** A second launch that carries a link lets any process take the foreground for that moment, not only the running Marshell. A local program has easier routes, so it stays as hardening for later.
- **A cold start ignores the link but focuses as any start does.** A hostile link with Marshell closed starts it, and its first window behaves as when the person starts it.
- **Registration writes over the key.** A leftover `DelegateExecute` or other verb under `HKCU\Software\Classes\marshell` would win while the switch reads on.
- **The uninstaller compares the exact path.** A Marshell started through an 8.3 short path would register a command the uninstaller does not recognise, and the key stays.
- **A toast keeps the link it was made with.** One shown while the link pointed here stays a protocol toast if another Marshell takes the key, and its click starts that one.

## `marshell` CLI routes (6h1)

The `marshell` CLI and the two escape hatches add four session routes and one history pair. All sit behind `host_guard` and `auth_guard`; none skips the token, and `guard.rs` and the token handling are unchanged. The CLI reads the token from `run/endpoint.json` as the hook bridge does.

- **`POST /v1/sessions/{tab}/fill`** puts text on a prompt line and never presses Enter. It refuses every C0 control (Enter, Tab, Ctrl+C, and the Escape that starts a CSI-u Enter or the end of a bracketed paste), DEL and every C1 control (U+009B is a one-character CSI), and text over 4000 characters. It refuses a paused session and an Ended or Exited one, so nothing is written to a dead pty. With `wait` it holds the text until the agent is ready (output has started and gone quiet for 1.5 s; for a Claude Code row with hooks also after its `SessionStart`) and gives up after 30 s with `not_ready`, filling nothing.
- **`POST /v1/sessions/{tab}/focus`** asks the shell to bring its window forward on the tab, by the same path as the `marshell://` link. It answers 404 for a tab that isn't open and 503 when no window is attached. It carries no text and answers nothing for an agent.
- **`GET .../resume-command`** (a session, or a conversation in History) and **`POST .../open-outside`** build the command line only from what the core holds: the tab's row, its history row and its profile. No request body carries a command, a folder or an id. An id reaches a command line only after `sidecar_allowed` (a tab's through `resume_session`, a conversation's through `look_up`), and is then one word of letters, digits, `_` and `-`. The line is the per-family builder's own (`resume_line`, the one `launch_argv` uses), quoted for the session's shell, and always one line: a folder name holding a control character is refused, so pasting runs nothing until the person presses Enter.
- **Refused:** a session that has a preset (DeepSeek), because its environment, which holds a key, never leaves Marshell and is not built into any line or console; a plain terminal; a manifest agent with no `continue` arguments; a conversation whose transcript is gone; and, for `open-outside`, a running tab (and a conversation running in a tab), so two CLIs never write one conversation, a folder that no longer exists, and every system but Windows ("works on Windows only for now").
- **What the outside console gets.** It runs `resolve()`'s argv (the profile's shell running the agent, which stays open after it) in the session's folder, started by the core with a new console and out of Marshell's job, and not waited on. Its environment is the profile's own variables, without `MARSHELL_TAB_ID`, `MARSHELL_SESSION_NUMBER` and `MARSHELL_AGENT`, which it also strips from what it inherits from the app, so no process there can act as that tab through the hook bridge. It does not get Marshell's `bin` folder on `PATH`: that is a session's environment only.
- **Known limits.** A `%` in a Command Prompt folder is expanded by cmd and isn't escaped (a `"` can't be in a Windows path). A WSL profile's folder is quoted as a POSIX path, whatever it holds. The console and its CLI are outside Marshell's reach once started: they have no hooks, no status and no recording.

## Residual risks

- **Same-user processes read the token.** Any program running as the user can read `endpoint.json` and drive the API. This is accepted: such a program can already edit the CLIs' config directly.
- **A managed path not on the list.** A new organization-managed location that the guard doesn't know is only protected by the allowed-folder rule. The list lives in `guard::denied_for_os` and is updated with the adapters.
- **Approved but hostile content.** A person who approves a diff they didn't read gets that change. Snapshots and restore make it undoable, and the consent card in 3c keeps diffs short and plain.
- **Archived transcripts are plain text once decompressed.** The history keeps a second copy of every Claude conversation in the home, protected by the home's permissions only.
- **Children of the home are fixed only on reset.** A file inside the home given its own explicit ACL or a looser mode by another tool keeps it until the home's own DACL or mode is found wrong.

## Consequences

- New routes go behind `host_guard` and `auth_guard`; a route that skips the token needs its own ADR.
- Every config write goes through the config service; no feature writes a CLI's files directly.
- Approve-from-anywhere (notifications with Allow/Deny) needs a security review before it ships. The in-app card was reviewed for 3d (above); answering from an OS notification needs its own review.
