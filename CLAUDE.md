@AGENTS.md

# Claude Code in Marshell

The shared rules are above, imported from `AGENTS.md`, and `SCOPE.md` is binding. This part adds only what is specific to Claude Code and to the setup this repo is worked with (global setup: `Doofess/claude-setup`).

## How a slice runs

1. The open design questions go to the user through AskUserQuestion. The answers are recorded in `docs/PLAN.md` §9, in the slice's "Slice <x> decisions (<date>)" list.
2. The plan goes in `docs/superpowers/plans/<date>-phase-<x>-<name>.md` (writing-plans), committed on a branch `phase-<x>` cut from `origin/main`.
3. executing-plans runs it. The ledger is `.superpowers/sdd/<plan basename>/progress.md` (gitignored). After compaction, trust the ledger and `git log` over memory, and delete the ledger once the branch merges.
4. A live check runs where the plan's exit criteria need the real app (below). Its findings go in `docs/research/<topic>.md`.
5. The final whole-branch review runs on Opus, followed by one fix pass. Then the PR, a green CI and the merge.

A PR body has "What it does", "Live check", "Known limits" and "Checks" sections, with the exact commands and counts from this session.

## Subagents

- `worker` (Sonnet) loads this file and `AGENTS.md`. Brief it with the plan task, the files it touches and the exact test command. Use `effort: high` for the pty and ConPTY code, the local API's guard and token handling, parsers, concurrency, or a second attempt.
- `Explore` and `Plan` don't load `CLAUDE.md` or `AGENTS.md`. Put what they need in the prompt: read-only, the folders to search, and which docs are binding.
- Check every worker report yourself: read the diff and run its tests before ticking the task.

## Live check (Windows)

1. In the worktree, run `pnpm tauri dev` in the background with `MARSHELL_HOME` set to a scratch folder and `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333`.
2. Drive the webview over CDP at `http://127.0.0.1:9333`. Prefer evaluations that return small JSON over screenshots, and screenshot only to judge the look.
3. Agent sessions in a live check are ones you start for it. Pressing Enter in one is part of the check, never part of the feature.
4. Stop what you started by PID, then remove the scratch folder.

## Gotchas

- xterm.js 6 on Windows: Ctrl+V reaches the CLI as ^V and fires no paste event, and Ctrl+Shift+V is the terminal's paste.
- Enabling Tauri's native file drop disables in-page HTML5 drag and drop on Windows.
- The webview's CSP allows only the app and loopback. A request to anywhere else is blocked in dev and in the build, and shows only as a CSP error in the webview console.

## Compact instructions

When compacting, keep the branch, the plan and ledger paths, the current task, every ruling made, failing test names, and live-check state (PIDs started, the scratch `MARSHELL_HOME`).
