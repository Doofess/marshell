# Prompt-time context injection, per CLI (slice 6i2)

- Date: 2026-10-31. Read from official docs and source; no CLI was run. The docs pages were read through a fetch tool that summarises, so quotes are from its extract.
- Question: how each CLI lets a hook add a short factual note to the model's context when the person submits a prompt, for project references (brief §5.11).

## Claude Code

Source: [code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks). High confidence.

- `UserPromptSubmit`, exit 0, with plain stdout or `{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"…"}}`. The text sits beside the prompt and never replaces it. `SessionStart` takes the same shape.
- Cap: 10,000 characters per field. Above that, the text goes to a file and Claude sees a path plus a 2,000-character preview.
- Not shown to the person: it reaches the model as a system reminder named after the hook, and is saved in the session transcript.
- The prompt waits for the hook. The default timeout on `UserPromptSubmit` is 30 s. On a timeout the context is dropped and the prompt goes on. Exit 2 blocks the prompt, and any other non-zero exit shows a "hook error" notice.

## Codex CLI

Sources: [learn.chatgpt.com/docs/hooks](https://learn.chatgpt.com/docs/hooks); `codex-rs/hooks/src/events/user_prompt_submit.rs`, `events/common.rs` and `tui/src/history_cell/hook_cell.rs` in openai/codex. High confidence on the contract; medium on the size limit.

- `UserPromptSubmit` (and `SessionStart`) take plain stdout as "extra developer context", or the same `hookSpecificOutput.additionalContext` JSON as Claude Code.
- Plain stdout that looks like JSON but doesn't parse fails the hook ("invalid user prompt submit JSON output"), so Marshell answers with the JSON form.
- Cap: about 2,500 tokens per hook output. Above that, the output spills to disk as a head-and-tail preview. The handler's `additionalContextLimit` changes the cap, and `0` removes it.
- Not shown in the TUI ("Model-facing hook context never appears in the TUI"). A `systemMessage` does show, as a "↳ Hook ·" line.
- Synchronous hooks hold the prompt; `async: true` hooks can't. The default timeout is 600 s.

## Antigravity CLI (agy 1.4.x)

Source: [antigravity.google/docs/hooks?tab=cli](https://antigravity.google/docs/hooks?tab=cli). High confidence on field names; runtime behaviour is unknown.

- There is no `additionalContext`. `PreInvocation` (before each model call), `SessionStart` and `PostInvocation` read `injectSteps`: an array of `systemMessage` (object), `userMessage` (string), `ephemeralMessage` ("a transient system message") or `toolCall` items. For example: `{"injectSteps":[{"ephemeralMessage":"…"}]}`.
- The `PreInvocation` payload carries `lastUserInput` and `invocationNum` (spike A5 in `2026-10-26-antigravity-hooks.md`), so matching is possible. Because the hook fires per model call, Marshell must send the note only once per prompt.
- The default timeout is 30 s. A failing `PreInvocation` hook is logged and the turn goes on (spike A1). Empty stdout with exit 0 is the neutral answer.
- Unknown, for the 6i2 spike:
  - whether the model sees an `ephemeralMessage`;
  - whether it is saved in `transcript.jsonl` or shown;
  - how often `PreInvocation` fires in one turn with tool steps, and what `invocationNum` counts;
  - whether a slow hook stalls the prompt.

## Manual `@` references (for the later `@` picker)

| CLI | What `@` does |
| --- | --- |
| Claude Code | `@path` includes the file (up to 25,000 tokens; not over 256 KB), and a folder gives a listing. A typed `@path` is likely expanded on submit (inferred, not tested). See [common-workflows](https://code.claude.com/docs/en/common-workflows). |
| Codex | `@` opens a fuzzy file search in the composer, and `/mention <path>` also exists ([slash-commands](https://learn.chatgpt.com/docs/cli/slash-commands)). A typed path is likely plain text (low confidence). |
| agy | `@` plus part of a path. A picked item sends its full path. A typed or edited path "is sent as plain text" ([at-mentions](https://antigravity.google/docs/at-mentions)). |

## Corrections

- An earlier note gave Claude Code's hook timeout as 600 s. That is the general default; `UserPromptSubmit` defaults to 30 s. Marshell sets its own explicit timeout on each entry.
