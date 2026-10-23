---
name: marshell-ship
description: Use before opening a pull request in this repo.
---

# Ship a Marshell branch

## 1. Run what CI runs

These are the commands from `.github/workflows/ci.yml`, in its order, plus the Claude Code hook tests. Run them in the branch's worktree. Send each one's output to a file in the scratchpad and keep its tail; the PR quotes the counts.

```
pnpm install --frozen-lockfile
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm check:generated
pnpm test
pnpm test:claude
pnpm build
pnpm tauri build --no-bundle
```

- `cargo fmt --all --check` fails: run `cargo fmt --all`, commit, and start again.
- `check:generated` fails: run `cargo test -p marshell-protocol` and commit `src/generated/`. Never edit it by hand.
- CI also runs the two S6 throughput tests in release (`s6_raw_conpty_50mb`, `s6_throughput_50mb`). Run them locally only when the branch touches the pty or the pty WebSocket.
- Any other failure is a bug in the branch: use systematic-debugging, don't rerun until it passes.

## 2. Open the pull request

Push the branch and open the PR with `gh pr create`. The body has these sections:

```markdown
## What it does

<What the person using Marshell gets, by feature, in a few bullets. Name the slice decisions it implements.>

## Live check

<What ran in the real app and the result, linking docs/research/<note>.md. Or "None: <why the exit criteria don't need one>.">

## Known limits

<What's deliberately not done, deferred minors from the final review, and anything verified only on Windows.>

## Checks

- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: <N> passed, <N> ignored.
- `pnpm check:generated`: ok.
- `pnpm test`: <N>/<N> passed.
- `pnpm test:claude`: <N>/<N> passed.
- `pnpm build`: ok.
- `pnpm tauri build --no-bundle`: ok.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
```

Every count comes from a run in this session. A section with nothing to say says so in one line.

## 3. CI and merge

1. Wait for CI: `gh pr checks <n> --watch`, in the background.
2. A failure in `a_status_line_update_does_not_cancel_the_held_done` is the known hook timing flake. It gets one rerun (`gh run rerun <run id> --failed`), and the PR summary names it. Any other failure, or a second failure of that test, is a real failure: fix it on the branch.
3. When CI is green, merge (`gh pr merge <n> --merge`, the repo's style), then fast-forward the main checkout: `git -C C:/dev/Airport pull --ff-only`.
4. Clean up after the merge. Delete the slice's ledger directory under `.superpowers/sdd/`. Remove the Codex and worker worktrees under `C:/dev/codex-wt/` and their branches. Literal paths only; `git branch -D` and folder deletes are confirmed with the user.
