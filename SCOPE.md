# Scope for coding agents

This file says what a coding agent working in this repo may change on its own, what needs the user's word first and what it never touches. It applies to every agent (Claude Code, Codex CLI and others) and to anyone working with one. `AGENTS.md` and `CLAUDE.md` point here. When this file and a task disagree, stop and ask.

## Change freely, within the task

- Source and tests in `src/`, `crates/`, `src-tauri/src/` and `scripts/`.
- `src/generated/`, but only by running the generator (`cargo test -p marshell-protocol`).
- The slice's plan in `docs/superpowers/plans/` and the research notes in `docs/research/` for the work at hand.
- In `docs/PLAN.md` §9, the decisions the user made, recorded in the slice's decisions list.

## Ask the user first

- **Product and UX decisions.** Ask about anything `docs/PLAN.md` doesn't settle, and about any change to what it does settle. Don't guess.
- **Dependencies.** A new crate or package, or a version bump the task doesn't need (`Cargo.toml`, `package.json`, the lockfiles). See the policy below.
- **Security surface.**
  - The CSP and window settings in `src-tauri/tauri.conf.json` and `tauri.windows.conf.json`.
  - The capabilities in `src-tauri/capabilities/`.
  - The local API's guard (`crates/core/src/server/guard.rs`), token handling, and file permissions under the Marshell home folder.
  - The CSP is never loosened.
- **Build and release.** CI (`.github/workflows/`), `rust-toolchain.toml`, release and signing setup.
- **Owner files.**
  - `docs/BRIEF.md`, the owner's brief. Amend it only when the user says so.
  - `LICENSE` and `THIRD_PARTY_NOTICES.md`.
  - The approved designs and icons in `assets/`.
  - `SECURITY.md` and this file.
- **Git history that can't be undone.** A force-push to a shared branch, `git reset --hard`, `git clean -f`, deleting an unmerged branch. Pushes, PRs and merges of finished work follow the user's instructions for the session.

## Never

- **Real configuration.** Don't edit a real CLI configuration or Marshell's real data while developing: `~/.claude/`, `~/.codex/`, other agents' config folders, and `~/.marshell/`.
  - Tests and live checks use `MARSHELL_HOME` and temporary folders.
  - Marshell's own writes to a CLI configuration go through the config service: preview, backup, atomic write, undo (`docs/PLAN.md` §6).
- **Credentials.** Don't read or copy credential files (`~/.claude/.credentials.json`, `~/.codex/auth.json`, keychains), `.env` files, or the token in a running Marshell's `run/endpoint.json`.
- **Secrets in the repo.** Don't commit a secret, token or key. Logs, fixtures and screenshots carry no real token, email address or user name.
- **Other people's sessions.** Don't send input to an agent session the user is running, or approve anything on their behalf. Marshell itself never presses Enter in a terminal.
- **Killing by name.** Don't kill processes by name (`taskkill /IM claude.exe`, `Stop-Process -Name node`). Real agent CLIs run on this machine, so stop only the processes you started, by PID.
- **Test binaries named like real CLIs.** Don't name a test binary after a real CLI (`claude.exe`, `codex.exe`). Use a name like `codex-marshell-test.exe` and run it by its full path, since the shell here doesn't search the current folder.
- **Weakened checks.** Don't skip or weaken a test, lint or CI step to get a green run.
- **Bare `git stash`.** The stash is shared by every worktree and session. Set work aside with a WIP commit instead.
- **Instruction files.** Don't rename, move or delete `AGENTS.md`, `CLAUDE.md`, this file, or another CLI's project files.

## Dependency policy

- Prefer what is already in the tree.
- A new dependency needs all of the following:
  - a concrete need written in the slice's plan;
  - an active upstream;
  - a licence that fits the MIT-licensed app, with no copyleft in what ships;
  - no network access or telemetry at runtime unless that is its purpose.
- Versions are pinned through the lockfiles. A lockfile change is committed with the change that needs it.
- Anything bundled into the app gets its notice in `THIRD_PARTY_NOTICES.md`.
