# Slice 6c: tool profiles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Developer tools that aren't agents (Vite, Terraform, gh, kubectl and the rest) get data profiles. A profile can start the tool from the picker, mark the tab Needs you when the tool's prompt text shows, and turn the URL it announces into a port chip. Any process in a tab that listens on a port gets a chip too. A command known to print secrets is neither scanned nor recorded, and a command line carrying a secret is scrubbed.

**Architecture:** A tool profile is a protocol type. The core ships a set in an embedded JSON file (`crates/core/src/tools/profiles.json`) and reads the person's own from `~/.marshell/tools.json` (read-only, reloaded on an mtime or length change, like `agents.json`). The pty `Session` gets a watch: which profiles apply to the command running now, and whether that command prints secrets. The watch is set at once from the shell integration's command line (OSC 633;E) through a classifier closure, and set again by the foreground watcher from the process chain and the deepest process's own arguments. While a watch is on, `ingest` feeds the output to a matcher: port lines from an ANSI-stripped line stream, and needs-you patterns from the rendered screen rows at the cursor. Its events go to the registry through a callback, the way marks do today. A ports watcher reads the OS's TCP listener table every 2 s and keeps `SessionInfo.ports` for each running tab. A launch preset is a `tool` field on `CreateSessionRequest`, run as a command inside the default shell profile with shell integration on.

**Tech Stack:** Rust (axum, serde, ts-rs, regex, windows-sys), React and TypeScript, Vitest.

**Spec:** `docs/PLAN.md` §9, "Slice 6c decisions (2026-10-25)", items 1–10. Evidence: `docs/research/2026-10-25-agent-and-tool-cli-manifests.md`, "Tool profiles" and "Secret-printing commands".

## Global Constraints

- A shipped needs-you or port pattern needs a `[src]`, `[docs]` or `[local]` tag in the research note. A string tagged `[unverified]` doesn't ship. (6c decision 7) The secret-command list is the exception: it ships whole, because hiding output is the safe failure.
- Needs-you patterns run only while the foreground command is a profiled tool, including one started through `npx`, `pnpm dlx`, `pnpm exec`, `npm exec` or `bunx`. (decision 6)
- While a command that prints secrets runs, its output is neither matched nor written to the recording. The recording gets a marker when the hiding starts and when it ends. Best effort: output that comes before Marshell sees the command is recorded. (decision 2)
- A matched line, its neighbours and a secret are never logged, stored or put in `SessionInfo`. Only the normalized `scheme://host:port/path` of a port line survives. (research note, rule 3)
- A port chip's URL is always loopback: the host is `localhost`, `127.0.0.1` or `[::1]` after normalization (`0.0.0.0`, `[::]` and other `127.x` addresses become `localhost`). A capture with userinfo (`@`), any other host or a port outside 1–65535 gives no chip. The frontend checks again before it opens a URL. (decision 4)
- Clicking a chip opens the URL in the default browser without a question, through `openUrl` from `@tauri-apps/plugin-opener`. `src-tauri/capabilities/` already allows `http://*` and `https://*` and isn't changed. (decision 4)
- A launch preset is never run again by Marshell: Restart, Reopen and layout restore start the plain shell in the same folder. Fill-in field values are never written to the layout, the launcher memory, history, logs or `Relaunch`. (decision 9; Marshell never presses Enter, SCOPE.md)
- `~/.marshell/tools.json` is only read; this slice writes no file there. Tests use a temp home, never the real `~/.marshell`. (decision 8; SCOPE.md)
- New routes go in the token-guarded router, each with a "refused without the bearer" test. No route takes a file path. New request and response types go in `crates/protocol`, and `pnpm check:generated` passes. (`.claude/rules/local-api.md`)
- Blocking reads (process lists, process arguments, the TCP table) run in `spawn_blocking`. No `MutexGuard` is held across an `.await` or I/O. The output lock is never held while a callback runs.
- Dependencies: no new crate. `crates/core/Cargo.toml` adds three `windows-sys` features: `Win32_NetworkManagement_IpHelper`, `Win32_Networking_WinSock` (the TCP listener table) and `Wdk_System_Threading` (`NtQueryInformationProcess` for a process's command line). Each concrete need is written here, as the dependency policy in SCOPE.md asks.
- Non-Windows: the listener table and process arguments are read from `/proc` on Linux and return nothing elsewhere. CI builds Windows only, so the Linux code is unverified and the PR says so.
- Never name a test binary after a real CLI. Name matching is tested as pure functions on strings. (`.claude/rules/pty.md`)
- Copy follows PLAN §Microcopy: sentence case, subject first, no "!" or "successfully". UI works with the keyboard alone and in forced colors; tokens only, no hex; no boxed outlines. (`.claude/rules/ui.md`)
- Rust: `cargo test -j 4`. Run `cargo fmt --all` before every Rust commit. Commit messages read `type(scope): sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **A chip URL that isn't loopback.** Output such as `Local: http://localhost:3000@evil.example/`, `http://localhost.evil.example:80`, or a `Ready on http://192.168.1.5:8787`. Expected: no chip. Pinned by Task 3's `a_port_capture_that_is_not_loopback_gives_no_chip` and the frontend's `refuses_a_non_loopback_url`.
2. **A spinner or redraw under a matched prompt.** `vercel login` prints `Waiting for authentication...` with a spinner that rewrites the same row, and `gh` reprints its question line. Expected: Needs you stays until a new row of other text shows or the person types; it doesn't flicker on and off. Pinned by Task 3's `a_spinner_rewriting_the_matched_row_keeps_needs_you`.
3. **Secret output reaching the recording in the same chunk as the command line.** PowerShell's OSC 633;E and the start of `gh auth token`'s output can arrive in one read. Expected: the marks are handled before the chunk is recorded, so none of the token's bytes are recorded. Pinned by Task 4's `a_secret_command_line_in_the_same_chunk_hides_that_chunk`.
4. **A layout restore or Restart re-running `terraform apply`.** Expected: the tab comes back as the plain shell in the same folder, and nothing is typed or run. Pinned by Task 7's `a_preset_session_relaunches_as_its_plain_shell`.
5. **An agent CLI's own listener or an auth callback becoming a chip** (Claude Code's OAuth callback, `gcloud auth login` on 8085, `az login`). Expected: no chip for a port owned by a known agent exe, none while the foreground command matches the profile's `quiet_ports`, and none for a port seen on only one poll. Pinned by Task 6's `listening_ports_skip_agents_quiet_commands_and_one_poll_blips`.

---

## File structure

- Create `crates/protocol/src/tools.rs`: `ToolProfile`, `NeedsYouPattern`, `PromptKind`, `PortPattern`, `LaunchPreset`, `PresetField`, `ToolCatalog`, `CatalogTool`, `OwnToolProblem`, `ToolLaunch`, `PortChip`. Register in `crates/protocol/src/lib.rs`.
- Modify `crates/protocol/src/api.rs`: `SessionInfo.ports`, `Foreground.tool`, `CreateSessionRequest.tool`.
- Create `crates/core/src/tools/` with `mod.rs`, `profiles.json` (shipped), `profile.rs` (parse, validate, `Tools` lookups), `own.rs` (`tools.json` load and cache), `secrets.rs` (secret commands and scrubbing), `matcher.rs` (line splitter, port normalization, needs-you on screen rows), `listen.rs` (TCP listener table). Add `pub mod tools;` to `crates/core/src/lib.rs`.
- Modify `crates/core/src/pty/session.rs` (watch, classifier, tool callback, recording suppression, scrubbed command line), `crates/core/src/pty/proctree.rs` (chain with pids, a process's command line), `crates/core/src/registry.rs` (tool attention, ports), `crates/core/src/server/foreground.rs` (watch from the chain), `crates/core/src/server/sessions.rs` (preset resolve, callbacks), `crates/core/src/shell.rs` (integration plus a trailing command), `crates/core/src/profiles/detect.rs` (a preset's command line per shell family), `crates/core/src/paths.rs` (`tools_file`).
- Create `crates/core/src/server/tools.rs` (`GET /v1/tools`) and `crates/core/src/server/ports.rs` (the ports watcher).
- Frontend: create `src/lib/api/tools.ts`, `src/features/ports/ports.ts`, `src/features/ports/PortChips.tsx`, `src/features/agents/ToolsSection.tsx`. Modify `src/features/sessions/toRow.ts` (`RowModel.ports`), `src/features/window/SessionHeader.tsx`, `src/features/window/Pane.tsx`, `src/features/window/MainWindow.tsx`, `src/features/composer/stripItems.ts`, `src/features/sessions/launchPrograms.ts`, `src/features/launcher/launchChoice.ts`, `src/features/sessions/LiveLauncher.tsx`, `src/features/agents/AgentsDialog.tsx`, `src/features/sessions/LiveApp.tsx` (drop `ports={[]}`).
- Docs: `docs/research/2026-10-25-tool-profiles-live-check.md` (Task 10), and research-note tags flipped for strings the live check confirms.

---

### Task 1: The profile type, the shipped profiles and the lookups

**Files:**
- Create: `crates/protocol/src/tools.rs`; `crates/core/src/tools/{mod.rs,profiles.json,profile.rs}`
- Test: unit tests in `profile.rs`; regenerate `src/generated/` with `cargo test -j 4 -p marshell-protocol`

**Interfaces:**
- Produces (protocol, all `#[derive(TS)] #[ts(export)]`, `deny_unknown_fields` on the file types):
  ```rust
  #[serde(rename_all = "kebab-case")] pub enum PromptKind { Text, Browser }
  pub struct NeedsYouPattern { pub pattern: String, pub phrase: String, pub kind: PromptKind }
  /// Capture group 1 is a URL, `host:port`, `[v6]:port` or a bare port.
  pub struct PortPattern { pub pattern: String }
  pub struct PresetField { pub id: String, pub label: String }
  /// `args` may hold `{field-id}` placeholders, each a whole argument.
  pub struct LaunchPreset { pub id: String, pub name: String, pub program: String,
      #[serde(default)] pub args: Vec<String>, #[serde(default)] pub fields: Vec<PresetField> }
  pub struct ToolProfile { pub id: String, pub name: String,
      #[serde(default)] pub commands: Vec<String>,   // program names, lowercase, no extension
      #[serde(default)] pub scripts: Vec<String>,    // path fragments in a node or python child's first argument
      #[serde(default)] pub launcher: bool,          // npx-style: its patterns join the launched tool's
      #[serde(default)] pub needs_you: Vec<NeedsYouPattern>,
      #[serde(default)] pub ports: Vec<PortPattern>,
      #[serde(default)] pub quiet_ports: Vec<String>, // command-line prefixes after the program, e.g. "auth login"
      #[serde(default)] pub presets: Vec<LaunchPreset> }
  ```
- Produces (core): `pub struct Tools` with `Tools::shipped() -> Tools`, `Tools::with_own(own: Vec<ToolProfile>) -> Tools`, `fn profile(&self, id: &str) -> Option<&Arc<ToolProfile>>`, `fn classify_line(&self, line: &str) -> Classified`, `fn classify_argv(&self, argv: &[String]) -> Classified`, and `pub struct Classified { pub profiles: Vec<Arc<ToolProfile>>, pub tool: Option<String>, pub rest: String }`. `tool` is the non-launcher profile's id; `rest` is the command line from the tool's program word on, which `quiet_ports` prefixes are compared with. Compiled regexes live beside each profile (`Arc<Compiled>`), built once at load. `pub fn validate_own(list: &[ToolProfile], shipped: &Tools) -> (Vec<ToolProfile>, Vec<OwnToolProblem>)`.
- `OwnToolProblem { index: u32, id: Option<String>, reason: String }`.

- [ ] **Step 1: Write the failing tests** in `profile.rs`:
  - `shipped_profiles_parse_with_unique_ids_and_valid_regexes`: ids exactly `["npx","vite","next","vercel","wrangler","netlify","firebase","supabase","docker","gh","kubectl","gcloud","aws","az","terraform","fly","stripe","ngrok","op"]`; every pattern compiles; every port pattern has capture group 1.
  - `classify_line_skips_launchers`: `npx vite --port 3000` gives profiles `[npx, vite]` and tool `vite`; `pnpm dlx wrangler dev` gives `[npx, wrangler]` (the `npx` profile is the launcher one for all five launchers); `bunx --bun vercel dev`, `npm exec -- netlify dev` likewise; `& 'C:\Program Files\Terraform\terraform.exe' apply` gives `terraform`; `docker compose up` gives `docker` with `rest == "docker compose up"`; `git status` gives none.
  - `classify_argv_names_a_node_child_by_its_script`: `["node.exe", "C:\\Users\\a b\\AppData\\Roaming\\npm\\node_modules\\vercel\\dist\\index.js", "dev"]` gives `vercel`; `["python.exe", "...\\google-cloud-sdk\\lib\\gcloud.py", "auth", "login"]` gives `gcloud` with `rest == "gcloud auth login"`.
  - `own_tool_validation_reasons`, each reason exact:
    - id `vite`: "The id \"vite\" is taken by a shipped tool."
    - id `Bad Id`: "An id uses lowercase letters, digits and dashes, up to 32."
    - empty name: "A tool needs a name."
    - no `commands` and no `scripts`: "A tool needs a command or a script to recognise it by."
    - a pattern that doesn't compile: "The pattern \"…\" isn't a valid regular expression."
    - a port pattern with no group: "The port pattern \"…\" needs a group around the URL or port."
    - a preset arg `{x}` with no field `x`: "The preset \"…\" uses {x}, which it doesn't ask for."
    - a preset arg that `secrets::scrub` would change: "The preset \"…\" holds a secret. Ask for it as a field instead." (uses Task 4's function; until Task 4 lands, leave this one case `#[ignore]` with a note)
    - two own entries with one id: "Two tools share the id \"x\"."
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-core tools::profile`. Expected: compile failure (no module).
- [ ] **Step 3: Write `profiles.json`** from the research note's "Tool profiles" sections, following its rules 1–9:
  - Port patterns: the table rows for Vite, Next.js, Vercel, Wrangler, Netlify (the current banner only), Firebase (both shapes), kubectl `port-forward` and `proxy`, gh `codespace ports forward`, gcloud `start-iap-tunnel`, AWS `ssm start-session`. None for Supabase (its `start` and `status` print secrets) and none for `fly redis proxy`.
  - Needs-you patterns: every `[src]`, `[docs]` or `[local]` string in "Needs-you prompt texts", with a phrase in Marshell's voice: "Asks for a value", "Asks to continue", "Waiting for sign-in in the browser", "Asks for a sign-in code". Anchor end-of-output prompts with `\s*$`. Use az's loose match (`microsoft\.com/devicelogin|aka\.ms/devicelogin`). The `npx` profile carries `Ok to proceed\?\s*$` and sets `launcher: true`.
  - `quiet_ports`: `auth login` (gh, gcloud), `login` (az, aws, terraform, wrangler, firebase, netlify, vercel, docker, fly, stripe, supabase).
  - `scripts`: `node_modules/<pkg>/` for each npm tool (vite, next, vercel, wrangler, netlify-cli, firebase-tools, supabase), and `google-cloud-sdk/lib/gcloud.py` and `azure-cli` for gcloud and az.
  - Presets: vite "Vite dev server" `npx vite`; next "Next.js dev server" `npx next dev`; wrangler "Wrangler dev" `npx wrangler dev`; netlify "Netlify dev" `npx netlify dev`; vercel "Vercel dev" `npx vercel dev`; firebase "Firebase emulators" `npx firebase emulators:start`; docker "Compose up" `docker compose up`; kubectl "Port forward" `kubectl port-forward {target} {ports}` with fields target "Resource (pod/name or svc/name)" and ports "Ports (local:remote)"; terraform "Plan" `terraform plan` and "Apply" `terraform apply`; gh "Sign in" `gh auth login`.
- [ ] **Step 4: Implement** the protocol types and `profile.rs`. Matching names: lowercase, strip one of `.exe .cmd .bat .ps1`, basename after the last `\` or `/`, quotes and a leading `&` removed. Launchers: `npx`, `bunx`, `pnpm dlx`, `pnpm exec`, `npm exec`, `yarn dlx`, then skip words that start with `-` and a lone `--`. Script fragments compare case-insensitively with `\` normalized to `/`.
- [ ] **Step 5: Run** `cargo test -j 4 -p marshell-core tools::profile`, then `cargo test -j 4 -p marshell-protocol` and `pnpm check:generated`. Expected: PASS and ok.
- [ ] **Step 6: Commit** `feat(tools): add tool profiles and the shipped set`.

### Task 2: Own tools and the tools route

**Files:**
- Create: `crates/core/src/tools/own.rs`, `crates/core/src/server/tools.rs`
- Modify: `crates/core/src/paths.rs` (`pub fn tools_file(home: &Path) -> PathBuf` → `<home>/tools.json`), `crates/core/src/server/mod.rs` (route and an `AppState::tools() -> Arc<Tools>` that reads through the cache)
- Test: unit tests in `own.rs`; route tests next to the catalog route's

**Interfaces:**
- Consumes: Task 1's `Tools`, `validate_own`.
- Produces: `pub fn load_full(home: &Path) -> LoadedTools { pub tools: Arc<Tools>, pub own: Vec<ToolProfile>, pub problems: Vec<OwnToolProblem> }`, `pub struct ToolsCache` with `get(&self, home) -> Arc<LoadedTools>` (same stamp rule as `agents::own::OwnCache`). File shape `{"version":1,"tools":[...]}`; a newer version, bad JSON or a wrong shape gives no own tools and one problem at index 0, "tools.json can't be read: <why>.", and an unparsable entry is a problem at its own index while the others load (the `agents.json` rules).
- Produces (protocol): `ToolCatalog { tools: Vec<CatalogTool>, problems: Vec<OwnToolProblem> }`, `CatalogTool { id, name, own: bool, presets: Vec<CatalogPreset> }`, `CatalogPreset { id, name, fields: Vec<PresetField>, found: bool }`. `found` is whether the preset's `program` resolves on the core's PATH (`detect::command_on_path`), read in `spawn_blocking`.
- Route: `GET /v1/tools` → `ToolCatalog`.

- [ ] **Step 1: Write the failing tests**: `a_missing_tools_file_gives_the_shipped_set`, `an_edited_file_is_read_again` (write, read, rewrite with another length, read: the new entry is there), `a_broken_entry_is_listed_at_its_place_and_the_others_load`, `a_newer_file_gives_one_problem_and_no_own_tools`, and the route's `tools_route_is_refused_without_the_bearer` and `tools_route_lists_shipped_and_own_with_problems`.
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-core tools::own server::tools`. Expected: FAIL.
- [ ] **Step 3: Implement** as above.
- [ ] **Step 4: Run** the same tests, then `pnpm check:generated`. Expected: PASS, ok.
- [ ] **Step 5: Commit** `feat(tools): read own tool profiles from tools.json and serve the tools catalog`.

### Task 3: The output matcher

**Files:**
- Create: `crates/core/src/tools/matcher.rs`
- Test: unit tests in the same file

**Interfaces:**
- Consumes: Task 1's compiled profiles.
- Produces:
  ```rust
  pub enum ToolEvent { NeedsYou(String), Cleared, Port(PortChip) }
  pub struct Matcher { /* profiles, line splitter, raised phrase, seen ports */ }
  impl Matcher {
      pub fn new(profiles: Vec<Arc<ToolProfile>>) -> Matcher;
      /// `pass` is what the terminal got; `tail` is the screen's last non-empty rows up to and including the
      /// cursor row, right-trimmed and joined with '\n', at most 3 rows.
      pub fn feed(&mut self, pass: &[u8], tail: &str) -> Vec<ToolEvent>;
  }
  pub fn normalize_port(capture: &str) -> Option<PortChip>;
  ```
  `PortChip { port: u16, url: String, announced: bool }` (protocol, `port` as a number in TS).
- Rules: the line splitter strips CSI, OSC (including OSC 8, ended by BEL or ST) and two-byte ESC sequences, treats `\r` as a return to the line's start (later text overwrites), and yields lines at `\n`; a partial line carries over, capped at 4 KiB. Box characters `│┃|╭╮╰╯─` are replaced with spaces before port patterns run. Each new port is reported once per matcher. Needs-you: when any pattern matches `tail` and nothing is raised, raise its phrase; when nothing matches and a phrase is raised, send `Cleared` only if `pass` held a `\n` (a spinner that rewrites its row with `\r` or cursor moves keeps it raised).
- `normalize_port`: accepts `scheme://host:port[/path]`, `host:port`, `[v6]:port` and a bare port. The host must be `localhost`, `127.0.0.0/8`, `0.0.0.0`, `[::1]` or `[::]`, and the result's host is `localhost` unless it was `127.0.0.1` or `[::1]`. The scheme is `http` unless `https` was given. The path keeps only `[A-Za-z0-9/._~-]`, so the query and fragment are dropped. Userinfo means no chip.

- [ ] **Step 1: Write the failing tests**:
  - `vite_bold_port_is_read_after_stripping`: `b"  \x1b[32m\xe2\x9e\x9c\x1b[39m  \x1b[1mLocal\x1b[22m:   \x1b[36mhttp://localhost:\x1b[1m5173\x1b[22m/\x1b[39m\n"` with the vite profile gives `Port(PortChip { port: 5173, url: "http://localhost:5173/", announced: true })`.
  - `a_line_split_across_two_chunks_is_read_once`.
  - `osc8_links_and_box_characters_are_stripped`: a Firebase row `│ Authentication │ 127.0.0.1:9099 │ http://127.0.0.1:4000/auth │` gives port 9099 with `http://127.0.0.1:9099`.
  - `kubectl_forwarding_lines_give_one_chip`: the `127.0.0.1:8080 -> 80` and `[::1]:8080 -> 80` lines give one chip for 8080.
  - `a_port_capture_that_is_not_loopback_gives_no_chip`: `http://localhost:3000@evil.example/`, `http://localhost.evil.example:80`, `http://192.168.1.5:8787`, `localhost:0`, `localhost:70000` all give `None`.
  - `terraform_value_prompt_raises_and_new_output_clears`: tail `"Only 'yes' will be accepted to approve.\n\n  Enter a value:"` raises "Asks for a value"; feeding `b"\r\nApply complete!\r\n"` with a tail that doesn't match gives `Cleared`.
  - `a_spinner_rewriting_the_matched_row_keeps_needs_you`: after a raise on `Waiting for authentication...`, ten feeds of `b"\r\x1b[2K⠋ Waiting for authentication..."` give no events.
  - `npx_ok_to_proceed_is_matched_through_the_launcher_profile`.
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-core tools::matcher`. Expected: FAIL.
- [ ] **Step 3: Implement** as above.
- [ ] **Step 4: Run** the same. Expected: PASS.
- [ ] **Step 5: Commit** `feat(tools): match port lines and prompts in a tool's output`.

### Task 4: Secret commands, scrubbing and the recording

**Files:**
- Create: `crates/core/src/tools/secrets.rs`
- Modify: `crates/core/src/pty/session.rs` (`shown_command_line` scrubs; the watch; recording suppression), `crates/core/src/pty/record.rs` only if a marker needs a new helper
- Test: unit tests in `secrets.rs`; session tests in `crates/core/tests/session.rs` next to the recording tests

**Interfaces:**
- Produces: `pub fn prints_secrets(line: &str) -> bool`; `pub fn scrub(line: &str) -> Cow<'_, str>`; `pub fn shown(line: &str) -> String` (the program word and up to two subcommand words, then ` …`, used for markers).
- Produces (session): `pub struct Watch { pub profiles: Vec<Arc<ToolProfile>>, pub tool: Option<String>, pub secret: bool, pub label: String }` (`label` is `secrets::shown(line)`); `pub type Classifier = Arc<dyn Fn(&str) -> Watch + Send + Sync>`; `SpawnSpec.classify: Option<Classifier>`; `SpawnSpec.on_tool: Option<ToolCallback>` with `pub type ToolCallback = Box<dyn Fn(ToolEvent) + Send + Sync>`; `SpawnSpec.initial_command: Option<String>`; `Session::set_watch(&self, epoch: u64, watch: Watch) -> bool` (applied only if `command_epoch() == epoch` and no watch came from a command line at this epoch; true when it changed something); `Session::watch_tool(&self) -> Option<String>`.
- `prints_secrets` covers every command in the research note's "Secret-printing commands" list, matched on the classified program and its leading words (`op read`, `op item get` with `--reveal`, `--fields`, `--otp`, `--share-link` or `--format json`, `supabase start`, `supabase status`, `gh auth token`, `gh auth status --show-token`, `kubectl get secret` with `-o`, `kubectl config view --raw`, `docker compose config` and the rest). `scrub` changes a line holding a secret flag or argument from the note's argv list: any `--token`, `--access-token`, `--api-key`, `--auth`, `--password`, `--secret`, `--from-literal`, `--basic-auth` (with a space or `=`), and the program-specific ones: `docker login -p`, `az login -p`, `fly -t`, `terraform -var`, `gh secret set -b`, `ngrok config add-authtoken`, `fly secrets set`, `aws configure set`, `op item create` and `op item edit` with `key=value` arguments. The result is `shown(line)`.
- Session rules: `shown_command_line` returns `scrub`'s result, so `Foreground.command_line`, the row phrase and any log of it are scrubbed at the source. At `Mark::CommandLine(text)`, `track_command` sets the watch from `classify(text)` (unscrubbed: classification sees the real words; nothing stores them). `Finished` and `PromptStart` drop the watch and the matcher. `initial_command` is classified at spawn and dropped at the first `PromptStart`. In `ingest`, the marks of a chunk are folded before that chunk is recorded or matched. While `watch.secret` is on, `pass` isn't recorded and no matcher runs; the recording gets `marker("Output hidden: <label> prints secrets")` when hiding starts and `marker("Output recorded again")` when it stops.

- [ ] **Step 1: Write the failing tests**: `secret_commands_are_recognised` (`op read op://a/b/c`, `npx supabase status -o env`, `gh auth token`, `kubectl get secret db -o yaml` true; `op whoami`, `kubectl get pods`, `gh auth status` false); `secret_flags_are_scrubbed` (`docker login -u me -p hunter2` → `docker login …`; `vercel deploy --token=abc` → `vercel deploy …`; `kubectl create secret generic s --from-literal=k=v` → `kubectl create secret …`; `git status` unchanged); session tests `a_secret_command_hides_its_output_from_the_recording` and `a_secret_command_line_in_the_same_chunk_hides_that_chunk` (feed one chunk holding `ESC]633;E;gh auth token ESC\ ESC]133;C ESC\ gho_SECRET\r\n`; the cast file holds the marker and not `gho_SECRET`), and `the_foreground_phrase_is_scrubbed`.
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-core tools::secrets` and `cargo test -j 4 -p marshell-core --test session secret`. Expected: FAIL.
- [ ] **Step 3: Implement** as above. Turn on the `#[ignore]`d Task 1 case.
- [ ] **Step 4: Run** the same, plus `cargo test -j 4 -p marshell-core --test session` for the recording tests that were there. Expected: PASS.
- [ ] **Step 5: Commit** `feat(tools): keep secret output out of the recording and scrub secret command lines`.

### Task 5: Watches from the session and the foreground watcher, into the registry

**Files:**
- Modify: `crates/core/src/pty/session.rs` (matcher in `ingest`), `crates/core/src/pty/proctree.rs`, `crates/core/src/server/foreground.rs`, `crates/core/src/server/sessions.rs` (`spawn_into_map` builds `classify` and `on_tool`), `crates/core/src/registry.rs`, `crates/core/Cargo.toml` (`Wdk_System_Threading`)
- Test: unit tests in `registry.rs`, `foreground.rs`, `proctree.rs`; one session test

**Interfaces:**
- Consumes: Tasks 1, 3 and 4.
- Produces: `proctree::foreground_chain_pids(root) -> Option<Vec<(u32, String)>>` (the existing `foreground_chain` maps it to names), `proctree::command_line(pid: u32) -> Option<String>` (Windows: `NtQueryInformationProcess` with `ProcessCommandLineInformation` (60) on a `PROCESS_QUERY_LIMITED_INFORMATION` handle; Linux: `/proc/<pid>/cmdline` with NULs as spaces; else `None`); `Session::foreground_argv(&self) -> Option<Vec<String>>` (the deepest chain process's command line, split by a new pure `proctree::split_args(line: &str) -> Vec<String>` that follows the `CommandLineToArgvW` quoting rules on Windows and splits on spaces elsewhere; tested with a quoted path holding a space and an escaped quote).
- Registry: `Entry.tool_attention: bool` (not serialized); `pub fn tool_event(&self, tab_id: &str, event: ToolEvent, view: &UiView)`. `NeedsYou(phrase)` sets attention through `note_attention(Some(phrase))` under the existing rule (not while the tab is on screen in a focused window; `attention_allowed` with `bell == false`), and sets `tool_attention`. `Cleared` takes attention only if `tool_attention` is set. `Port(chip)` adds an announced chip (Task 6 owns the list's lifetime; until then, append if the port is new, at most 8). `note_input` keeps clearing attention as it does.
- `Foreground.tool: Option<String>` is filled from the watch (`#[serde(default)]`, TS optional).
- Foreground watcher: after a reading, `classify_argv(foreground_argv)` (falling back to the chain's names through `classify_line`) and `session.set_watch(epoch, watch)`. Process reading stays in the existing `spawn_blocking`.

- [ ] **Step 1: Write the failing tests**: `a_tool_prompt_raises_attention_and_typing_clears_it`; `a_cleared_event_does_not_take_a_bells_attention`; `a_tool_prompt_on_screen_is_already_seen`; foreground `a_node_child_running_vercel_sets_the_vercel_watch` (pure: `classify_argv` plus the epoch rule); `a_command_line_watch_beats_a_late_watcher_reading`; proctree `command_line_of_this_process_names_the_test_binary` (reads `std::process::id()`; Windows and Linux only).
- [ ] **Step 2: Run** the named tests with `cargo test -j 4 -p marshell-core <name>`. Expected: FAIL.
- [ ] **Step 3: Implement.** In `ingest`, with the output lock held: if a watch with profiles is on and not secret, compute `tail` from the screen (the rows up to the cursor row, last 3 non-empty), call `matcher.feed`, and keep the events; after the lock drops, call `on_tool` for each. `spawn_into_map` builds `classify` from `st.tools()` and `on_tool` from the registry and `view_reader()`, the way `on_mark` is built.
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(tools): raise Needs you from a tool's prompt text`.

### Task 6: Listening ports and the chip list

**Files:**
- Create: `crates/core/src/tools/listen.rs`, `crates/core/src/server/ports.rs`
- Modify: `crates/core/src/registry.rs`, `crates/core/src/server/mod.rs` (spawn the watcher), `crates/core/Cargo.toml` (`Win32_NetworkManagement_IpHelper`, `Win32_Networking_WinSock`), `crates/protocol/src/api.rs` (`SessionInfo.ports: Vec<PortChip>`, `#[serde(default)]`)
- Test: unit tests in `listen.rs`, `ports.rs`, `registry.rs`

**Interfaces:**
- Produces: `listen::listeners() -> Option<Vec<(u32, u16)>>` (pid and local port of every TCP listener, IPv4 and IPv6, from `GetExtendedTcpTable` with `TCP_TABLE_OWNER_PID_LISTENER`; Linux: `/proc/net/tcp{,6}` state `0A` joined to `/proc/<pid>/fd` socket inodes; else `None`); `pub fn chips_for(listeners: &[(u32, u16)], tree: &[(u32, String)], agents: &Agents, quiet: bool) -> Vec<u16>` (ports owned by a pid in `tree` whose exe isn't a known agent exe; empty when `quiet`).
- Registry: `pub fn ports_candidates(&self) -> Vec<(String, u64)>` (Running, not paused); `pub fn set_listening(&self, tab_id: &str, generation: u64, ports: &[u16])`. Chip list rules, in `SessionInfo.ports`, ordered by first sight, at most 8:
  - A listening port becomes a chip after two polls in a row (`http://localhost:<port>/`, `announced: false`), and goes on the first poll that misses it.
  - An announced chip keeps its URL. It goes when its command ends (`Finished` or `PromptStart`, or the foreground clears on a shell without marks), or on a poll that misses it after one that saw it.
  - A listening port that matches an announced chip doesn't add a second chip.
  - `clear_live` and an exit clear the list.
- `ports.rs` watcher: every 2 s, one `listeners()` read and one process snapshot in `spawn_blocking`; per candidate, the tree from `proctree::descendants` with names; `quiet` when the session's watch has a profile whose `quiet_ports` prefixes `Classified.rest` after the program word.

- [ ] **Step 1: Write the failing tests**: `listening_ports_skip_agents_quiet_commands_and_one_poll_blips` (pure `chips_for` plus the two-poll rule in the registry), `an_announced_chip_outlives_a_slow_listener_and_goes_with_its_command`, `an_announced_and_listening_port_is_one_chip`, `the_chip_list_stops_at_eight`, and `listeners_finds_a_socket_this_test_opened` (bind `127.0.0.1:0`, call `listen::listeners()`, find `(std::process::id(), port)`; Windows and Linux only).
- [ ] **Step 2: Run** them with `cargo test -j 4 -p marshell-core <name>`. Expected: FAIL.
- [ ] **Step 3: Implement** as above. Regenerate with `cargo test -j 4 -p marshell-protocol`.
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core` and `pnpm check:generated`. Expected: PASS, ok.
- [ ] **Step 5: Commit** `feat(ports): keep a port chip for every port a tab listens on`.

### Task 7: Launch presets in the core

**Files:**
- Modify: `crates/protocol/src/api.rs` (`CreateSessionRequest.tool: Option<ToolLaunch>`), `crates/protocol/src/tools.rs` (`ToolLaunch { tool: String, preset: String, fields: BTreeMap<String, String> }`), `crates/core/src/server/sessions.rs`, `crates/core/src/shell.rs`, `crates/core/src/profiles/detect.rs`
- Test: unit tests in `detect.rs` and `shell.rs`; server tests beside the create-session tests

**Interfaces:**
- Produces: `detect::preset_line(family: ShellFamily, program: &str, args: &[String]) -> Result<String, String>`, quoting with the existing `word` rules (PowerShell: `& prog args`; cmd: plain words only, else "Command Prompt can't pass an argument that needs quoting. Use PowerShell."; POSIX and WSL: `posix_quote`); `shell::integrate_with_command(profile, dir, outer_env, line) -> anyhow::Result<Applied>`: PowerShell appends `; <line>` to the integration's `-Command` text; cmd keeps the `PROMPT` env and adds `/k <line>`; bash, zsh, fish and WSL run the line the way an agent runs (`launch_argv` style, `exec` the shell after) with no integration.
- Rules for a request with `tool`: `agent`, `prompt`, `mode` and `preset` must be absent (400 "A tool preset starts without an agent."). The tool and preset must exist (400 "Unknown tool preset: <tool>/<preset>."). Every field must be given and non-empty, and none may hold a newline or NUL (400 "Fill in <label>."). Placeholders are replaced whole-argument. The program must be on PATH (400 "<program> isn't installed, or isn't on PATH."). The session's name is the preset's name. `initial_command` is the built line. `Relaunch` gets no tool: a restart, reopen or layout restore starts the plain profile.

- [ ] **Step 1: Write the failing tests**: `a_preset_runs_inside_the_integrated_powershell` (argv ends with `-Command`, `. ([scriptblock]…); & npx vite`), `cmd_runs_a_preset_after_k_with_its_prompt_env`, `a_field_value_with_a_space_is_quoted_per_shell`, `a_missing_field_is_refused_with_its_label`, `a_preset_with_an_agent_is_refused`, `a_preset_session_relaunches_as_its_plain_shell` (the registry's `Relaunch` has `agent: None` and the profile id, and nothing of the line or the fields).
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement**, then regenerate the TS types.
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core` and `pnpm check:generated`. Expected: PASS, ok.
- [ ] **Step 5: Commit** `feat(tools): start a tool preset inside the default shell`.

### Task 8: Port chips in the UI

**Files:**
- Create: `src/features/ports/ports.ts`, `src/features/ports/PortChips.tsx` (+ `.css` beside it if the header's chip styles don't cover it), `src/features/ports/ports.test.ts`, `src/features/ports/PortChips.test.tsx`
- Modify: `src/features/sessions/toRow.ts` (`RowModel.ports: PortChip[]`, default `[]`), `src/features/window/SessionHeader.tsx`, `src/features/window/Pane.tsx`, `src/features/window/MainWindow.tsx` (the `ports` prop stays for stories and is used only when given; live rows use `row.ports`), `src/features/composer/stripItems.ts`, `src/features/sessions/LiveApp.tsx` (drop `ports={[]}`)

**Interfaces:**
- Produces: `chipLabel(chip: PortChip): string` (`localhost:5173`, or `127.0.0.1:9099`); `isLoopbackUrl(url: string): boolean` (parses with `URL`; protocol `http:` or `https:`; no username or password; hostname `localhost`, `127.0.0.1` or `[::1]`); `openChip(url: string): Promise<void>` (calls `openUrl` only when `isLoopbackUrl`); `PortChips({ ports, max?, onMenu })`.
- UI: each chip is a `<button>` showing `chipLabel`, with `aria-label="Open <url> in the browser"`. A click opens the URL. Right-click, Shift+F10 and the Menu key open the shared `ContextMenu` (`opensRowMenu`, `keyboardAnchor` from `src/features/sidebar/rowMenu.ts`) with "Open in browser" and "Copy URL" (`navigator.clipboard.writeText`). The session header shows all chips; a pane header shows at most 2, then a `+N` chip that opens a menu listing the rest. The status strip keeps its `:5173` text from `row.ports`. Look from `frontend-design` and `good-css` (read both skills before writing styles).

- [ ] **Step 1: Write the failing tests**: `refuses_a_non_loopback_url` (`http://localhost:3000@evil.example/`, `http://evil.example/`, `file:///c:/x`, `javascript:alert(1)` false; `http://localhost:5173/`, `https://127.0.0.1:8443/x`, `http://[::1]:3000/` true); `chipLabel` cases; `PortChips` markup: buttons with the aria-label, a `+1` chip at `max: 2` with three ports, nothing rendered for no ports.
- [ ] **Step 2: Run** `pnpm test src/features/ports`. Expected: FAIL.
- [ ] **Step 3: Implement** as above.
- [ ] **Step 4: Run** `pnpm test` and `pnpm build`. Expected: PASS, build ok.
- [ ] **Step 5: Commit** `feat(ports): show port chips that open in the browser`.

### Task 9: Tools in the picker and the Agents dialog

**Files:**
- Create: `src/lib/api/tools.ts`, `src/features/agents/ToolsSection.tsx`, `src/features/agents/ToolsSection.test.tsx`
- Modify: `src/features/launcher/launchChoice.ts` (`LaunchProgram.fields?: PresetField[]`, `LaunchProgram.toolLaunch?: { tool: string; preset: string }`), `src/features/sessions/launchPrograms.ts` (`livePrograms(catalog, profiles, tools?)` adds one "Tools" entry per catalog preset, id `tool:<tool>:<preset>`, label `<preset name>`, `cli` the tool's name, `noFirst: true`, `problem` "Not installed, or not on PATH." when `!found`; `launchRequest` sends `tool: { tool, preset, fields }` with `profile_id: profiles.default_id`), `src/features/sessions/LiveLauncher.tsx` (load `toolsApi.get` with the others; a labelled text box per field, Enter in the last one starts; Start stays disabled until each is filled; field values live in component state only), `src/features/agents/AgentsDialog.tsx` (mount `ToolsSection` after the own-agents section)
- Test: `src/features/sessions/launchPrograms.test.ts`, `ToolsSection.test.tsx`

**Interfaces:**
- Consumes: Task 2's `GET /v1/tools`, Task 7's `ToolLaunch`.
- `toolsApi.get(endpoint): Promise<ToolCatalog>` beside `catalogApi`.
- `ToolsSection({ endpoint })` / `ToolsView({ catalog })`: a list with each tool's name, "Shipped" or "Own", and its presets; file problems first, then each own entry's problems by index; the line "Add your own in tools.json in the Marshell folder. Settings gets an editor later." A failed load says "Couldn't read the tools list." and offers Try again.

- [ ] **Step 1: Write the failing tests**: `livePrograms` puts tool presets in the Tools group after Shells, and a missing program carries the problem; `launchRequest` for a tool preset carries `tool` with the fields and no `agent`, `prompt` or `mode`; the launcher memory saver ignores field values (assert on the helper that writes memory, if it's a pure function; otherwise on `launchRequest`'s inputs); `ToolsView` lists a problem under its entry and the file problem at the top.
- [ ] **Step 2: Run** `pnpm test src/features/sessions src/features/agents`. Expected: FAIL.
- [ ] **Step 3: Implement** as above.
- [ ] **Step 4: Run** `pnpm test` and `pnpm build`. Expected: PASS, build ok.
- [ ] **Step 5: Commit** `feat(tools): start tool presets from the picker and list tools in the Agents dialog`.

### Task 10: Live check, research tags and docs

**Files:**
- Create: `docs/research/2026-10-25-tool-profiles-live-check.md`
- Modify: `docs/research/2026-10-25-agent-and-tool-cli-manifests.md` (flip `[unverified]` to `[local]` only for strings seen live), `crates/core/src/tools/profiles.json` (add a confirmed string only with that flip)

- [ ] **Step 1: Run the live check** with the `marshell-live-check` skill in a scratch home and a scratch project folder:
  - `npx vite` from the Vite preset in a folder with a minimal `index.html` (`npx` asks Ok to proceed: Needs you shows, then clears on `y`). The chip `localhost:5173` appears and opens the browser. Kill the server: the chip goes.
  - `npx next dev` in a minimal Next app (or record that the scratch install was too heavy).
  - `python -m http.server 8123` in a plain shell: a listening chip with no profile.
  - Installed tools, signed in as they already are, without changing anything: `gh auth status`, `gh auth token` (the recording holds the marker and not the token; the row phrase shows `gh auth token`), `docker compose config` in a scratch compose file (hidden), `kubectl version --client`, `gcloud version`, `op --version`. Something with a y/N prompt that changes nothing: `docker system prune` answered `N` (its `[y/N]` is `[src]`).
  - A scrubbed line: type `docker login -u x -p notreal localhost:1` (refused by the registry, harmless): the row reads `docker login …`.
  - Keyboard: Tab to a chip, Enter opens, Shift+F10 opens the menu, Copy URL copies. Forced colors: chips still read as buttons.
- [ ] **Step 2: Write the findings** to the live-check note (each step, what showed, the versions). Flip research tags only for strings seen.
- [ ] **Step 3: Commit** `docs(research): record the tool profiles live check`.

## Known limits to carry into the PR

- Docker-published ports and ngrok tunnels get no chip (their processes aren't in the tab's tree); the polling fallback waits for a later slice.
- Supabase's `start` and `status` print secrets, so they get no announced chips, and Supabase runs in Docker, so it gets no listening chips either.
- Command-derived ports (`fly proxy`, `stripe listen --forward-to`, gcloud `--host-port`) aren't read.
- A tool's text prompt on the tab that's on screen in a focused window counts as seen, as a terminal notification does (slice 4d). The banner, sound and count are as for a hook's Needs you.
- Presets on bash, zsh, fish and WSL run without shell integration for the shell that follows.
- The Linux listener and argument reads are compiled nowhere in CI.
