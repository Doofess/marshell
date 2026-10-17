<p align="center"><img src="assets/icon/app-night-shift.svg" alt="" width="96" height="96"></p>

# Marshell

Marshell is a desktop terminal for running AI coding agents side by side. Claude Code, Codex CLI, other agent CLIs and ordinary shells each get their own session. Marshell keeps the sessions organised and works the same way whichever agent a session runs.

> **Status: early development.** No release yet. Windows 11 is the only platform tested. Linux and macOS are planned but not yet verified. Expect things to change.

## What works today

- **Sessions.** Tabs and a sidebar of sessions, with shell detection, profiles and a launcher. Split view takes up to four panes, with focus mode, project grouping and task worktrees.
- **Shell integration.** Prompt and command marks, clickable paths that resolve against the shell's current folder.
- **Themes.** App and terminal themes, high-contrast and large-text modes, and theme import from Windows Terminal, iTerm2 and VS Code.
- **Restart and recording.** After a restart, sessions come back as "Ended" with a one-click Resume. Nothing relaunches or spends tokens on its own. Sessions can be recorded as asciicast v2.
- **Preview.** Code, Markdown, images, PDF and HTML open in a read-only preview pane. You can mark a region of a PDF page or an image and send it to the agent with a comment.
- **Keyboard and accessibility.** The app is keyboard-first, with a command palette and a cheat sheet. Accessibility is part of every change.

Agent status, notifications, history, the settings editors and the doctor come in later phases. See the roadmap in [`docs/PLAN.md`](docs/PLAN.md) §9.

## Principles

- **Marshell never presses Enter for you.** Anything it puts in an agent's prompt waits for you to send it.
- **Marshell never touches your CLI configuration silently.** Every change it makes to an agent CLI's configuration is shown first, backed up and can be undone.
- **Marshell never handles your credentials.** It never reads, stores or brokers your CLI credentials. Sign-in runs the CLI's own login command.
- **Nothing is guessed.** Marshell shows what a CLI reports, or nothing.

## Build from source

You need:

- Windows 11
- Rust (the version in `rust-toolchain.toml`; rustup picks it up)
- Node.js
- pnpm (the version in `package.json`)
- WebView2, which comes with Windows 11

```sh
pnpm install
pnpm fetch:conpty   # once, on Windows: the bundled ConPTY
pnpm tauri dev      # run the app
```

Tests and checks:

```sh
cargo test --workspace
pnpm test
pnpm build
pnpm check:generated
```

## Repository layout

| Path | What it holds |
|---|---|
| `src/` | React and TypeScript frontend, with xterm.js |
| `crates/core/` | Rust core: the local API, sessions and terminals |
| `crates/protocol/` | Shared protocol types; the TypeScript types in `src/generated/` are generated from them |
| `src-tauri/` | The Tauri 2 application shell |
| `docs/` | The brief, the plan, research notes, spikes and accessibility checks |

## Documentation

- [`docs/BRIEF.md`](docs/BRIEF.md): the original brief and its amendments.
- [`docs/PLAN.md`](docs/PLAN.md): the product, UX, architecture and the phase plan.
- [`AGENTS.md`](AGENTS.md), [`CLAUDE.md`](CLAUDE.md) and [`SCOPE.md`](SCOPE.md): instructions for coding agents working on this repository.
- [`SECURITY.md`](SECURITY.md): how to report a vulnerability, and the security model.

## License

[MIT](LICENSE). Third-party components bundled with the app are listed in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
