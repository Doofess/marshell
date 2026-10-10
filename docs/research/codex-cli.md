# Codex CLI integration notes

Checked against OpenAI's Codex CLI documentation on 2026-10-10. This is an implementation handoff for Marshell's planned Codex adapter; it does not mean the adapter exists yet. Recheck the docs and record the installed CLI version when implementation starts because Codex hooks and app-server schemas evolve.

## Integration choice

Marshell's product is a terminal session manager, so launch the normal `codex` CLI inside Marshell's PTY. Do not substitute Codex app-server for the interactive CLI: app-server is a separate JSON-RPC integration that can create and drive Codex threads, and may be evaluated separately if Marshell later needs a non-terminal client. The app-server protocol is versioned with the installed CLI; its docs support generating TypeScript or JSON Schema from that exact binary.

## Hooks

Codex command hooks receive one JSON object on stdin. Current lifecycle events documented by OpenAI are:

| Event | Marshell use |
|---|---|
| `SessionStart` | Start/resume/compact lifecycle; use `source` (`startup`, `resume`, `clear`, `compact`) to initialize or refresh state. |
| `UserPromptSubmit` | Prompt submitted; payload includes the prompt and turn id. |
| `PreToolUse` | Tool activity and possible context. This fires before tool use, but is not the permission grant hook. |
| `PermissionRequest` | Approve or deny requests that Codex is about to present for approval. If Marshell returns no decision, Codex keeps its normal prompt. |
| `PostToolUse` | Tool completion and result; useful for activity/recap state. It cannot undo a completed tool action. |
| `SubagentStart`, `SubagentStop` | Track subagents by id; stop payload can include its transcript path and last assistant message. |
| `Stop` | Turn stopped; payload can include `last_assistant_message`. |
| `Interrupt` | User interrupted the active turn. |
| `PreCompact`, `PostCompact` | Manual or automatic context compaction. |
| `SessionEnd` | Main session ended; this is advisory and is not a reliable signal for closing a terminal tab. |

The common payload includes `session_id`, `transcript_path`, `cwd`, `hook_event_name`, and `model`. The documented `permission_mode` field is present on `SessionStart`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, `UserPromptSubmit`, `SubagentStart`, `SubagentStop`, `Stop`, and `Interrupt`; documented values are `default`, `acceptEdits`, `plan`, `dontAsk`, and `bypassPermissions`. Event-specific payload adds fields such as `turn_id`, `tool_name`, `tool_input`, `tool_response`, `prompt`, and `agent_id`. Subagent hooks use the parent `session_id`. Parse defensively and preserve raw events for fixtures. Hook docs do not define an effort field.

For `PermissionRequest`, Codex requires JSON on stdout to decide. Allow:

```json
{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}
```

Deny with `behavior: "deny"` and an optional `message`. If no decision is returned, the ordinary Codex approval UI remains in control. Do not use `updatedInput`, `updatedPermissions`, or `interrupt` for this event; the current docs say those are unsupported and fail closed. Multiple matching hooks may run concurrently; any deny wins. Keep the bridge response bounded by the hook timeout and fall back to Codex's prompt if Marshell is unavailable or no user decision arrives.

### Hook sources, trust, and config safety

OpenAI currently documents user hooks in `hooks.json` or `config.toml` under the Codex home directory (default `~/.codex`; overridden by `CODEX_HOME`), plus `.codex/hooks.json` and `.codex/config.toml` for project hooks. Active config layers and plugins can contribute hooks additively. Avoid installing a duplicate into both hook representations at the same config layer: Codex merges them and warns. Project hooks load only when the project's `.codex/` layer is trusted.

The current TOML form is event-nested, for example:

```toml
[[hooks.SessionStart]]
matcher = "startup|resume"

[[hooks.SessionStart.hooks]]
type = "command"
command = '"C:/Users/Home Office/.marshell/bin/marshell-hook.exe" event --agent codex'
timeout = 10
```

The JSON form uses `hooks.SessionStart` as an array of matcher groups, each containing a `hooks` array of handlers. Audit existing Codex onboarding and doctor fixtures before using them as templates: the current `src/features/onboarding/fixtures.ts` sample shape (`[[hooks]]` with flat `event` and `command`) does not match the event-nested TOML shape in the current OpenAI docs. Keep this as a known fixture gap for the implementation pass; this research-only update does not modify runtime or story fixtures.

Prefer one user-level hook source for Marshell's integration, leaving project and managed sources alone. Hook definitions created by Marshell are non-managed. Codex requires users to review and trust each exact current definition in `/hooks`; trust is hash-bound, so edits invalidate it. Marshell must not automate or bypass this trust step. The onboarding card should explain that hooks will not run until the user reviews and trusts them in Codex. The doctor should report trust as user-action-required and provide clear `/hooks` guidance; do not claim a hook is active based only on its presence in a file.

OpenAI documents hook timeouts in seconds: most default to 600 seconds; `SessionEnd` and `Interrupt` default to one second and are capped at three seconds. Set explicit short timeouts for routine telemetry and an explicit bounded timeout for permission decisions. Hook `commandWindows` is available for Windows-specific commands; test quoting and paths with spaces on Windows. Keep Marshell's edit diff, backup, managed-path guard, and undo guarantees for Codex files too.

## `AGENTS.md` and instruction library

Codex builds an instruction chain at startup. In the Codex home directory it uses `AGENTS.override.md` if present, otherwise `AGENTS.md` (only the first non-empty one). From the project root to the current working directory, it includes at most one instruction file per directory, preferring `AGENTS.override.md`, then `AGENTS.md`, then configured fallback filenames. It concatenates these from root to leaf. Empty files are skipped, and Codex stops adding files at the default 32 KiB combined `project_doc_max_bytes` limit. The user may configure another limit, so surface the effective value when Marshell can read it; never silently raise it.

The official `AGENTS.md` docs describe discovery and chaining of files, but no import syntax for a reusable instruction library. Do not depend on imports. If Marshell wires an instruction into Codex, preview and inline the selected content in a tagged, managed block, preserve the user's surrounding text, estimate the size of the full discovered chain, warn before the configured limit, and make removal/undo remove only Marshell's tagged content. Do not replace a user's root or nested `AGENTS.md`. Keep Claude's `CLAUDE.md` and other CLI instructions separate; never convert or rename those files to Codex format.

Codex app-server is documented as a deep-integration interface, and also supports a remote terminal UI pairing (`codex app-server --listen ...` with `codex --remote ...`). For this product's PTY-first architecture, prefer the ordinary interactive CLI unless Marshell intentionally adopts that server/client pairing. The docs do not describe attaching app-server to an already-running ordinary `codex` process; do not assume it can, or that its thread id is interchangeable with a PTY session id. App-server docs expose `model/list` with supported reasoning-effort options, `thread/tokenUsage/updated`, and `account/rateLimits/read` / `account/rateLimits/updated`; account rate-limit data requires Codex-service-backed authentication and is not available under API-key-only or Bedrock authentication. The current public event docs do not define the full token-usage notification payload, so generate schemas or inspect the installed CLI before parsing it. Do not assume rate-limit buckets are fixed 5-hour/weekly windows.

## Confirmed versus still to investigate

Confirmed from current official docs: hook input/output and `PermissionRequest` contract; event names and listed event fields; hook locations and additive behavior; trust-by-current-hash in `/hooks`; project trust gating; the default 32 KiB combined instruction limit; app-server's JSON-RPC thread lifecycle.

Still requires a real installed Codex CLI and versioned fixtures before adapter implementation: exact Windows hook installation syntax/quoting and hook trust UX; transcript/rollout format and compression/retention behavior; effort selection from `model/list` and its relevance to a PTY-launched session; the full `thread/tokenUsage/updated` payload and context-window denominator; availability and semantics of account rate-limit buckets for supported authentication modes; whether app-server can usefully observe the particular terminal session; session resume/fork behavior; and schema differences across supported versions. The active model and documented permission mode are available in hook inputs; do not label those fields unverified.

## Official references

- [Codex CLI hooks](https://learn.chatgpt.com/docs/hooks)
- [Custom instructions with `AGENTS.md`](https://learn.chatgpt.com/docs/agent-configuration/agents-md)
- [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference)
- [Codex app-server](https://learn.chatgpt.com/docs/app-server)
