---
name: marshell-codex-task
description: Use when handing a plan task to Codex CLI.
---

# Hand a plan task to Codex CLI

The user wants plan and fix tasks to run on Codex CLI, so they spend Codex usage instead of Claude usage. Judgment stays in this session: design questions, review triage and security calls. Codex implements one task; Claude reviews it, runs the tests, commits and cherry-picks.

## 1. A worktree per task

Cut it from the slice branch, outside the repo:

```
git worktree add -b codex-<slice>-t<N> C:/dev/codex-wt/<slice>-t<N> phase-<slice>
```

Run `pnpm install --frozen-lockfile` there if the task touches the frontend. Then run `pnpm fetch:conpty` if it runs the core's pty tests.

## 2. The prompt

Write it to `<scratchpad>/codex-<slice>-t<N>-prompt.md`:

```markdown
You are implementing one task of a written plan in the Marshell repo (this worktree). Read AGENTS.md and SCOPE.md first; they are binding.

Plan: docs/superpowers/plans/<plan file>
Read its header, "Global Constraints" and "Review Focus", then implement **Task <N> only** ("<task title>"). Do not start Task <N+1> or later.

How to work:
- TDD: write the tests the task names (exact names and assertions), run them and see them fail, then implement until they pass.
- Follow nearby code style (<the files to imitate>). No new dependencies.
- <Task-specific notes: interfaces from earlier tasks, rulings from the ledger, protocol regeneration.>
- Run: `cargo fmt --all`, <the task's test commands>, `cargo clippy --workspace --all-targets -- -D warnings`<, `pnpm check:generated`, `pnpm build`, `pnpm test <file>`>. If a command can't run in your sandbox, say which and why.
- Do not commit. Leave the changes in the worktree, and write the commit message to `.superpowers/codex-commit-message.txt` as UTF-8 without a BOM: the subject from the task's commit step, a blank line, then `Co-Authored-By: Codex <noreply@openai.com>`. Don't write it with Windows PowerShell 5's `Set-Content -Encoding utf8`, which adds a BOM.

Final message: files changed, each test command and its pass/fail counts, and anything you decided that the plan didn't specify.
```

## 3. Run it

In the background, from this session:

```
codex exec -C C:/dev/codex-wt/<slice>-t<N> -s workspace-write -o <scratchpad>/codex-<slice>-t<N>.md - < <scratchpad>/codex-<slice>-t<N>-prompt.md > <scratchpad>/codex-<slice>-t<N>.log 2>&1
```

- The `workspace-write` sandbox can't write the worktree's index, which lives in the main checkout's `.git`. That's why Claude commits. Don't widen the sandbox with `--add-dir` to the main `.git`: that opens every branch ref and the stash shared by all sessions.
- **Quota:** if the log shows `usage limit`, look at the worktree before anything else. Codex may have finished the work before the limit hit (it did in 5b Task 2). If it didn't, hand the rest to a `worker` agent with the same brief (`effort: high` for pty, ConPTY, guard and token, parser or concurrency work, or a second attempt), and tell the user Codex ran out.

## 4. Review and commit

1. Read the whole diff (`git -C <wt> diff` plus new files). Check it against the task's brief and the ledger's rulings. Look for scope creep, guessed values, and tests that never failed first.
2. Run the task's tests yourself in the worktree. Codex's sandbox can't run some core tests (pty, process tree, files outside the worktree), so its "pass" is not evidence. Codex output on Windows can be CRLF; check `git diff --stat` for whole-file churn.
3. Fix what's wrong yourself, or send it back to Codex with a short follow-up prompt.
4. Commit in the worktree. Strip a leading BOM from the message first:
   ```
   sed -i '1s/^\xEF\xBB\xBF//' .superpowers/codex-commit-message.txt
   git -C <wt> add -A && git -C <wt> commit -F <wt>/.superpowers/codex-commit-message.txt
   ```
   Keep the message file out of the commit: `.superpowers/` is gitignored.
5. Cherry-pick onto the slice branch from its worktree (`git cherry-pick <sha>...`). Then run `task-done` with the task's test command, and ledger any ruling you made in review.
6. When the slice merges, remove the task's worktree (`git worktree remove`) and its `codex-<slice>-t<N>` branch. A cherry-picked branch isn't merged by ancestry, so removing it takes `git branch -D`, which the user confirms.
