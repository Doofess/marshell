# Codex instructions for Marshell

Marshell is a Windows-first desktop terminal session manager for AI coding-agent CLIs, built with Tauri 2, Rust, React, TypeScript, and xterm.js. The product is deliberately multi-CLI: Claude Code, Codex CLI, other agents, and ordinary terminal programs are all in scope.

## Read before changing code

- `docs/PLAN.md` is the detailed product, UX, architecture, and implementation plan. Its UX and accessibility decisions are binding.
- `docs/BRIEF.md` records the original brief and amendments. Prefer the amendments and the detailed plan where older text conflicts.
- `docs/research/` and `docs/spikes/` contain evidence and platform constraints; check relevant material before changing adapter behavior or infrastructure.
- For Codex CLI adapter, hook, or instruction-file work, read `docs/research/codex-cli.md` first; it records verified behavior, source links, known fixture mismatches, and open verification items.
- `docs/IDEAS.md` is a scratchpad, not a commitment.

The codebase is still being built. Treat the source and current phase plans as authoritative for what exists today; do not assume every planned feature is implemented.

## Project map and commands

- `src/`: React and TypeScript frontend; shared UI primitives live in `src/components/`, product areas in `src/features/`, and design tokens in `src/styles/`.
- `crates/protocol/`: shared Rust protocol types and brand constants.
- `crates/core/`: Rust core, local API, sessions, and terminal handling.
- `src-tauri/`: thin Tauri application shell.
- `docs/`: product plans, research, accessibility checks, and spikes.

Use pnpm for the frontend and Cargo for Rust. Common commands:

- `pnpm dev` — frontend development server.
- `pnpm build` — TypeScript check and Vite production build.
- `pnpm test` — frontend tests.
- `pnpm tauri dev` — run the desktop app.
- `cargo test --workspace` — Rust workspace tests.
- `pnpm check:generated` — check generated protocol bindings.

Do not run tests or builds unless the user asks for verification. Keep generated files consistent with their source when intentionally changing protocol types.

## Working rules

- Preserve the multi-CLI architecture. Keep vendor-specific behavior inside adapters or clearly scoped vendor modules; generic terminal sessions must remain useful without an agent.
- Keep UI decisions consistent with `docs/PLAN.md`: keyboard-first, accessible, no guessed status values, and respect the documented layout and visual rules.
- Treat configuration and instruction-file writes as safety-critical. Follow the planned config service, preview, backup, atomic-write, managed-file, and undo rules. Never silently edit a user's CLI configuration.
- Keep Codex-specific behavior scoped to Codex (for example hook trust and `AGENTS.md` generation). Do not make Claude Code or another CLI depend on Codex conventions.
- Preserve existing Claude project files, including any `CLAUDE*.md` files. Do not rename, move, delete, or rewrite them as part of Codex support. Keep Codex instructions in this `AGENTS.md`.
- Avoid unrelated cleanup. Inspect `git status` before editing and do not overwrite or revert pre-existing user changes.
- Follow nearby code style and make focused changes. Do not add dependencies or alter platform support assumptions without a concrete need documented in the plan.

## Platform context

Windows 11 is the platform currently tested by the project. Linux and macOS are planned targets but are not yet verified; preserve cross-platform design where practical and clearly identify platform-specific changes.
