# Marshell: instructions for coding agents

These instructions are shared by every coding agent working in this repo. Codex CLI and other agents read this file directly; Claude Code reads it through `CLAUDE.md`, which adds only Claude Code specifics. What an agent may and may not touch is in `SCOPE.md`, which is binding: read it before your first change.

Marshell is a Windows-first desktop terminal session manager for AI coding-agent CLIs, built with Tauri 2, Rust, React, TypeScript, and xterm.js. The product is deliberately multi-CLI: Claude Code, Codex CLI, other agents, and ordinary terminal programs are all in scope.

## Read before changing code

- `docs/PLAN.md` is the detailed product, UX, architecture, and implementation plan. Its UX and accessibility decisions are binding. §9 lists the phases and, per slice, the decisions the user made.
- `docs/BRIEF.md` records the original brief and amendments. Prefer the amendments and the detailed plan where older text conflicts.
- `docs/superpowers/plans/` holds one implementation plan per slice; the newest one for the area you touch says what was built and why.
- `docs/research/` and `docs/spikes/` contain evidence and platform constraints; check relevant material before changing adapter behavior or infrastructure.
- For Codex CLI adapter, hook, or instruction-file work, read `docs/research/codex-cli.md` first; it records verified behavior, source links, known fixture mismatches, and open verification items.
- `docs/IDEAS.md` is a scratchpad, not a commitment.

The codebase is still being built. Treat the source and current phase plans as authoritative for what exists today; do not assume every planned feature is implemented.

## Project map

- `src/`: React and TypeScript frontend; shared UI primitives live in `src/components/`, product areas in `src/features/`, and design tokens in `src/styles/`.
- `src/generated/`: TypeScript types generated from `crates/protocol` by ts-rs. Never edit them by hand; `cargo test -p marshell-protocol` regenerates them, and the result is committed.
- `crates/protocol/` (`marshell-protocol`): shared Rust protocol types and brand constants.
- `crates/core/` (`marshell-core`): Rust core, local API, sessions, and terminal handling.
- `src-tauri/` (`marshell-app`): thin Tauri application shell.
- `scripts/`: check, fetch, and review scripts run through `package.json`.
- `spikes/`: spike code in its own Cargo workspace, not part of the build.
- `docs/`: product plans, research, accessibility checks, and spikes.

## Commands

Use pnpm for the frontend and Cargo for Rust; both versions are pinned (`packageManager` in `package.json`, `rust-toolchain.toml`).

- `pnpm install`, then on Windows `pnpm fetch:conpty` once for the bundled ConPTY.
- `pnpm tauri dev`: run the desktop app.
- `pnpm test`: frontend tests (Vitest in a node environment with no DOM; components are tested through `renderToStaticMarkup`). One file: `pnpm test <path>`.
- `pnpm build`: TypeScript check and Vite production build.
- `cargo test --workspace`: Rust tests. One crate: `cargo test -p marshell-core`.
- `pnpm check:generated`: fails when `src/generated/` is stale or uses `bigint`.
- `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`.

CI (`.github/workflows/ci.yml`, Windows only for now) runs the format check, clippy with warnings as errors, both test suites, `check:generated`, `pnpm build`, and `pnpm tauri build --no-bundle`.

## Verifying a change

- Before calling a change done, run the checks that cover it and report the command and what it printed: the crate's `cargo test` for Rust, `pnpm test <file>` for the frontend, `pnpm check:generated` after a protocol change.
- Before a pull request, run what CI runs. Run `cargo fmt --all` before every Rust commit.
- Don't start `pnpm tauri dev`, a release build, or the accessibility matrix unless the task calls for it (a live check or a release).
- A live check runs with `MARSHELL_HOME` set to a scratch folder, so it never touches the real `~/.marshell`.

## Working rules

- Preserve the multi-CLI architecture. Keep vendor-specific behavior inside adapters or clearly scoped vendor modules; generic terminal sessions must remain useful without an agent.
- Keep one CLI's conventions out of the others: Codex hook trust and `AGENTS.md` generation stay in Codex code, Claude Code's settings hooks stay in Claude Code code.
- Keep UI decisions consistent with `docs/PLAN.md`: keyboard-first, accessible, no guessed status values, and respect the documented layout and visual rules. Every UI change works with the keyboard alone and in forced colors; screen-reader hand checks happen at release sign-off (`docs/a11y/`).
- Treat configuration and instruction-file writes as safety-critical. Follow the planned config service, preview, backup, atomic-write, managed-file, and undo rules. Never silently edit a user's CLI configuration.
- Instruction files: a rule every agent needs goes in this file; `CLAUDE.md` holds only what is specific to Claude Code. Do not rename, move, or delete either one.
- Avoid unrelated cleanup. Inspect `git status` before editing and do not overwrite or revert pre-existing user changes.
- Follow nearby code style and make focused changes. New dependencies follow the policy in `SCOPE.md`.
- Commit messages read `type(scope): what changed, as a sentence`, as in `git log`.

## Platform context

Windows 11 is the platform currently tested by the project. Linux and macOS are planned targets but are not yet verified; preserve cross-platform design where practical and clearly identify platform-specific changes.
