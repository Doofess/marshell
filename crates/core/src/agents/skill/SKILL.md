---
name: marshell
description: "Use inside a Marshell session (MARSHELL_TAB_ID is set) when the person asks to list, start, message or bring forward Marshell sessions, or to hand work to another agent CLI in a new Marshell session."
---

# Marshell

Marshell is the desktop app this terminal runs in. Its `marshell` command lists, starts, fills and focuses the app's sessions, and it is on this session's PATH.

Use it only when `MARSHELL_TAB_ID` is set. Without it you are not inside Marshell: say so and stop. Your own session's number is `MARSHELL_SESSION_NUMBER`.

## Commands

- `marshell list --json`: the open sessions as JSON, each with `number`, `tab_id`, `name`, `agent`, `state` (`running`, `ended` or `exited`), `cwd`, `project` and `mode`. Read this, not the plain table.
- `marshell send <session> "<text>"`: puts the text on that session's prompt line. Marshell never presses Enter: the person reads it and sends it. `<session>` is a number or a tab id. The text is one line, at most 4000 characters.
- `marshell new --agent <agent> [--cwd <folder>] [--prompt "<text>"] [--mode <mode>] [--name <name>]`: starts a session. `<agent>` is an agent id such as `claude`, `codex` or `antigravity`. `--prompt` fills the new session's prompt line once the agent is ready, without pressing Enter. `<mode>` is `manual`, `plan`, `auto-edit`, `full-auto`, `bypass` or `dont-ask`.
- `marshell focus <session>`: brings Marshell's window to that session.

## Rules

- Send only to a session the person named, or one you started for them. Never send to your own session.
- Don't start a session in `bypass`, `dont-ask` or `full-auto` mode unless the person asked for that mode.
- Don't pass `--start`. If the command says "Marshell isn't running.", tell the person.
- When `send` or `new` refuses, tell the person what it said and don't retry:
  - "is waiting for an answer": that session shows a question or a permission request.
  - "has ended": that session can't take text.
  - "wasn't ready within 30 s": the prompt line wasn't filled.
