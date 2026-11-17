---
name: marshell
description: "Use inside a Marshell session (MARSHELL_TAB_ID is set) when the person asks to list, start, message or bring forward Marshell sessions, to hand work to another agent CLI in a new Marshell session, or to show a web page or a file in the side pane."
---

# Marshell

Marshell is the desktop app this terminal runs in. Its `marshell` command lists, starts, fills and focuses the app's sessions, and opens tabs in the side pane. It is on this session's PATH.

Use it only when `MARSHELL_TAB_ID` is set. Without it you are not inside Marshell: say so and stop. Your own session's number is `MARSHELL_SESSION_NUMBER`.

## Commands

- `marshell list --json`: the open sessions as JSON, each with `number`, `tab_id`, `name`, `agent`, `state` (`running`, `ended` or `exited`), `cwd`, `project`, `mode` and `elevated` (true when the session runs as administrator). Read this, not the plain table.
- `marshell send <session> "<text>"`: puts the text on that session's prompt line. Marshell never presses Enter: the person reads it and sends it. `<session>` is a number or a tab id. The text is one line, at most 4000 characters.
- `marshell new --agent <agent> [--cwd <folder>] [--prompt "<text>"] [--mode <mode>] [--name <name>]`: starts a session. `<agent>` is an agent id such as `claude`, `codex` or `antigravity`. `--prompt` fills the new session's prompt line once the agent is ready, without pressing Enter. `<mode>` is `manual`, `plan`, `auto-edit`, `full-auto`, `bypass` or `dont-ask`.
- `marshell focus <session>`: brings Marshell's window to that session.
- `marshell open <address or file>...`: opens each as a tab in the side pane of your own session, in the background. The person sees a mark on the tab and selects it when they want to. An address starts with `http://` or `https://`; anything else is a file path. `--session <session>` names another session.

## Rules

- Send only to a session the person named, or one you started for them. Never send to your own session.
- Don't start a session in `bypass`, `dont-ask` or `full-auto` mode unless the person asked for that mode.
- Open only what the person asked to see, or a page or file you made for them. Don't open the same thing again and again.
- Don't pass `--start`. If the command says "Marshell isn't running.", tell the person.
- When `send`, `new` or `open` refuses, tell the person what it said and don't retry:
  - "is waiting for an answer": that session shows a question or a permission request.
  - "has ended": that session can't take text.
  - "wasn't ready within 30 s": the prompt line wasn't filled.
  - "already has 32 tabs": tell the person, and don't close tabs for them.
