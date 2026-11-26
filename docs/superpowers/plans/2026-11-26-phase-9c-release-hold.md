# Slice 9c: a release can hold its winget submission (issue #216)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** a tag's release run submits to winget only when the owner wants it to, a cancel stops the winget jobs, and a fork that can't be synced stops the submission with what to do.

**Architecture:** `winget-submit`'s first step asks a small script (`scripts/release/winget-hold.mjs`) whether this run submits: not without the token, not for a tag while the repository variable `WINGET_HOLD` is set, always for a manual `submit`. Every later step of the job reads that answer. A new step before `wingetcreate` asks GitHub to sync the token owner's fork. The two winget jobs' conditions use `!cancelled()` in place of `always()`.

**Tech Stack:** GitHub Actions (`.github/workflows/release.yml`), PowerShell 7 in the steps, Node 24 for the release scripts, Vitest.

**Spec:** `docs/PLAN.md` §9, "Slice 9c decisions (2026-11-26)"; issue #216.

## Global Constraints

- This changes CI (`SCOPE.md`). The user approved exactly three changes: the `WINGET_HOLD` variable, `!cancelled()` in the two winget jobs, and the fork-sync step. No other job, trigger or permission changes.
- Never push a tag, run `action=submit`, or sync the fork in this slice. The only runs started by hand are `action=check` runs of the branch (Task 4).
- `secrets.WINGET_TOKEN` is read only in `winget-submit`: the token check, the fork-sync step and the submit step. It never appears on a command line and is never printed.
- A YAML value that starts with `!` is a tag, so a condition with `!cancelled()` is written `if: ${{ !cancelled() && (…) }}`.
- With the hold off, a tag's run does what it does today, plus the fork-sync step before the submission.
- The hold is on unless `WINGET_HOLD` is empty (unset) or exactly `false` (the spec: "set to anything but `false`"). Anything else, `FALSE` and blanks included, holds.

## What was verified before planning

- `release.yml` on `main` (`d928a0f4`): `winget-check` and `winget-submit` have `if: always() && (…)`. Run 45872192223 (`v0.1.0-alpha.2`) shows that cancelling doesn't stop them (issue #216).
- `winget-submit`'s step `release` prints the no-token notice and writes `token=<bool>`, and every later step has `if: steps.release.outputs.token == 'true'`.
- The fork `Doofess/winget-pkgs` is 0 commits ahead of `microsoft/winget-pkgs` and 6,677 behind, so a sync would be a fast-forward. Its `master` was last changed 2026-11-17. Upstream commit `805e6d11` (2026-11-24) changed `.github/workflows/spellCheck.yaml`, and GitHub won't let a token without the `workflow` scope bring that into a fork. RELEASING.md has the token as a classic token with `public_repo` only. So the likely cause of "The forked repository could not be synced" is the token's scope. That isn't proven: GitHub's own answer was never seen.
- `vite.config.ts` runs every `*.test.ts` outside `.claude/**` in a node environment. `scripts/release/*.test.ts` already run that way (`hooks.test.ts`, `notes.test.ts`). `ci.yml` runs them on windows-latest, where the runner sets `GITHUB_OUTPUT` for every step.
- `notes.mjs` and `winget-manifest.mjs` export their logic and run a CLI only when invoked as the script (`fileURLToPath(import.meta.url) === process.argv[1]`). The guard works from a relative path on windows-latest: alpha.2's notes and manifest came from it.
- The plan review (Opus, 2026-11-26) checked these. `!cancelled()` still runs a job whose `needs` job was skipped, and a cancel stops a job that is running. `vars.<name>` is an empty string when unset, on any event. GitHub's `pwsh` shell (7.4) doesn't throw on a native command's exit code or on redirected stderr. `merge-upstream` answers 200 with `merge_type` `none`, `fast-forward` or `merge`, and 409 or 422 when it refuses. The runner reads `::` commands at the start of a line and legacy `##[` commands anywhere in a line, and `::stop-commands::<token>` turns both off until `::<token>::`. Jobs that hadn't started when a run was cancelled end `cancelled` with no steps (run 45860766849).

## Review Focus

1. The hold off (`WINGET_HOLD` unset or exactly `false`): a tag submits as before. Pinned by `winget-hold.test.ts`, "a tag submits while the hold is off", and by `workflow.test.ts`, "every later step reads the decision".
2. A value other than exactly `false` (`FALSE`, `True`, `yes`, `"  "`): it holds. Pinned by `winget-hold.test.ts`, "isHeld".
3. A manual `submit` with the hold on submits. Pinned by "a manual submit ignores the hold".
4. A fork whose `master` has commits of its own: syncing would merge them, and the pull request to winget-pkgs could carry them. The sync step stops with an error before it syncs. The 404 path (no fork yet) must not fail the job, because `wingetcreate` makes the fork. Both are read in review; neither can be tried without a submission.
5. The variable changed while a tag's run is going: which value a run reads isn't pinned. RELEASING.md says to set or delete it before pushing the tag. A held tag run that is re-run doesn't submit either, because `publish` fails on the existing release; the notice's `action=submit` is the way out.

---

### Task 1: the decision script

**Files:**
- Create: `scripts/release/winget-hold.mjs`
- Test: `scripts/release/winget-hold.test.ts`

**Interfaces:**
- Produces:
  - `isHeld(value: string | undefined): boolean`. False only for `undefined`, `""` and `"false"`.
  - `submitDecision({ event, tag, hasToken, hold }: { event: string; tag: string; hasToken: string; hold?: string }): { submit: boolean; notice?: string }`.
  - The CLI `node scripts/release/winget-hold.mjs`. It reads `EVENT`, `TAG`, `HAS_TOKEN` and `WINGET_HOLD` from the environment and appends `submit=true` or `submit=false` to the file in `GITHUB_OUTPUT`. It prints `::notice::<notice>` when there is a notice. It exits 1 without `GITHUB_OUTPUT` or `TAG`.

- [ ] **Step 1: Write the failing tests**

`describe("isHeld")`, with `it.each`:
- `undefined`, `""` and `"false"` give `false`.
- `"true"`, `"True"`, `"FALSE"`, `" false "`, `"  "`, `"yes"` and `"1"` give `true`.

`describe("submitDecision")`, tag `v0.1.0-alpha.3`:
- "a tag submits while the hold is off": `{ event: "push", hasToken: "true", hold: "" }` gives `{ submit: true }`, and so do `hold: "false"` and no `hold`.
- "a tag is held while WINGET_HOLD is set": `{ event: "push", hasToken: "true", hold: "true" }` gives `submit: false` and the notice `The repository variable WINGET_HOLD is set, so v0.1.0-alpha.3 wasn't submitted to winget-pkgs. To submit it, run this workflow with action 'submit' and tag 'v0.1.0-alpha.3'. To let tags submit again, delete the variable (docs/RELEASING.md).`
- "a manual submit ignores the hold": `{ event: "workflow_dispatch", hasToken: "true", hold: "true" }` gives `{ submit: true }`.
- "without the token the notice is the token's": `{ event: "push", hasToken: "false", hold: "true" }` gives `submit: false` and the notice `No WINGET_TOKEN secret, so v0.1.0-alpha.3 wasn't submitted to winget-pkgs. Add the secret (docs/RELEASING.md), then run this workflow with action 'submit' and tag 'v0.1.0-alpha.3'.` `hasToken: ""` gives the same.

`describe("the CLI")`. Each run uses `execFileSync(process.execPath, [script])` or `spawnSync(process.execPath, [script])`, where the script path is `fileURLToPath(new URL("./winget-hold.mjs", import.meta.url))`. Each run's env is a copy of `process.env` with `GITHUB_OUTPUT`, `WINGET_HOLD`, `EVENT`, `TAG` and `HAS_TOKEN` deleted, and then the case's own values added. Without that, CI's own `GITHUB_OUTPUT` leaks in. `GITHUB_OUTPUT` points to a file in a fresh `mkdtempSync(join(tmpdir(), "winget-hold-"))` folder, which the test removes.
- `EVENT=push`, `TAG=v0.1.0-alpha.3`, `HAS_TOKEN=true` and `WINGET_HOLD=true`: the file holds the line `submit=false`, and stdout has a line that starts with `::notice::The repository variable WINGET_HOLD is set`.
- The same without `WINGET_HOLD`: the file holds `submit=true`, and stdout has no `::notice::`.
- Without `GITHUB_OUTPUT`: exit status 1.

- [ ] **Step 2: Run them and see them fail**

Run: `pnpm test scripts/release/winget-hold.test.ts`
Expected: FAIL, because `./winget-hold.mjs` can't be found.

- [ ] **Step 3: Write `scripts/release/winget-hold.mjs`**

Use `notes.mjs`'s layout:
- a header comment that names PLAN §9, slice 9c decisions 1 and 4;
- the two exported functions;
- then the CLI, guarded by `fileURLToPath(import.meta.url) === process.argv[1]`, using `appendFileSync(process.env.GITHUB_OUTPUT, ...)`.

Only `hasToken === "true"` counts as a token.

- [ ] **Step 4: Run them and see them pass**

Run: `pnpm test scripts/release/winget-hold.test.ts`
Expected: PASS, every test.

- [ ] **Step 5: Commit**

```bash
git add scripts/release/winget-hold.mjs scripts/release/winget-hold.test.ts
git commit -m "feat(release): a script decides whether a run submits to winget, and a tag is held while WINGET_HOLD is set."
```

### Task 2: the workflow

**Files:**
- Modify: `.github/workflows/release.yml` (the header comment, lines 3-11; `winget-check`'s `if`, line 157; `winget-submit`, lines 218-304)
- Test: `scripts/release/workflow.test.ts`

**Interfaces:**
- Consumes: Task 1's CLI and its output `submit`.
- Produces: the step with id `release` keeps its outputs `tag` and `version`, and `token` becomes `submit`.

- [ ] **Step 1: Write the failing tests**

The test reads `.github/workflows/release.yml` as text. No YAML library is added.
- A job's block runs from its line `^  <name>:$` to the next line that starts with two spaces and a non-space.
- A step's block runs from its `^      - ` line to the next one. A comment between two steps falls into the block of the step before it, which no assertion here minds.
- A job's own condition is the line matching `^    if: ` inside its block (step-level `if:` lines are indented further).
- Every block a test uses is asserted non-empty first, so no test passes on nothing.

The tests:
- "a cancel stops the winget jobs": the two jobs' condition lines are exactly these.
  - `    if: ${{ !cancelled() && (needs.publish.result == 'success' || (github.event_name == 'workflow_dispatch' && inputs.action == 'check')) }}`
  - `    if: ${{ !cancelled() && ((needs.publish.result == 'success' && needs.winget-check.result == 'success') || (github.event_name == 'workflow_dispatch' && inputs.action == 'submit')) }}`
  - The file holds no `always()` in any line matching `^    if: `.
- "the hold reaches the decision": `winget-submit`'s step with `id: release` has the env lines `WINGET_HOLD: ${{ vars.WINGET_HOLD }}`, `EVENT: ${{ github.event_name }}` and `HAS_TOKEN: ${{ secrets.WINGET_TOKEN != '' }}` (each line exactly, with the comment on its own line above), and runs `node scripts/release/winget-hold.mjs`.
- "every later step reads the decision": the names of `winget-submit`'s steps after `release` are exactly `["Download the release's manifest folder", "Download wingetcreate", "The pull request's title", "Bring the fork up to date", "Submit to microsoft/winget-pkgs"]`. Each has the line `if: steps.release.outputs.submit == 'true'`. The file has no `outputs.token`.
- "the fork is synced before the submission": the step `Bring the fork up to date` has the env line `GH_TOKEN: ${{ secrets.WINGET_TOKEN }}` and holds `merge-upstream`, `ahead_by` and `::stop-commands::`. The step order is already pinned by the test before.
- "only winget-submit reads the token": the jobs whose blocks hold `secrets.WINGET_TOKEN` are `toEqual(["winget-submit"])`. Inside it, the steps that hold it, by id or name, are `toEqual(["release", "Bring the fork up to date", "Submit to microsoft/winget-pkgs"])`.

- [ ] **Step 2: Run them and see them fail**

Run: `pnpm test scripts/release/workflow.test.ts`
Expected: FAIL on all five. The last one fails because the sync step doesn't exist yet.

- [ ] **Step 3: Change `release.yml`**

- The header comment:
  - the tag line becomes `build -> publish -> winget-check -> winget-submit (held while the variable WINGET_HOLD is set)`;
  - the sentence about `WINGET_TOKEN` gains that `winget-submit` also reads the variable `WINGET_HOLD`.
- `winget-check` and `winget-submit`: their conditions become exactly the two lines in Step 1.
- `winget-submit`, step `release`, renamed `The release's tag and version, and whether it is submitted`:
  - Add the env line `WINGET_HOLD: ${{ vars.WINGET_HOLD }}`, under the comment line `# docs/RELEASING.md, "Holding the submission". A manual submit ignores it.`, and the env line `EVENT: ${{ github.event_name }}`.
  - Keep the tag and version lines.
  - Replace the `token=` line and the no-token notice with `node scripts/release/winget-hold.mjs`, then `if ($LASTEXITCODE -ne 0) { exit 1 }`.
- Every later step: `steps.release.outputs.token == 'true'` becomes `steps.release.outputs.submit == 'true'`.
- A new step goes between `The pull request's title` and `Submit to microsoft/winget-pkgs`. Its order:
  1. `$owner = gh api user --jq .login`. When that fails or is empty, the step prints `::error::Couldn't read whose WINGET_TOKEN this is, so <tag> wasn't submitted. Run this workflow with action 'submit' and tag '<tag>' again.` and exits 1.
  2. `$fork = "${owner}/winget-pkgs"`. Then `gh api -i "repos/$fork" 2>$null | Select-Object -First 1`, matched the way `The pull request's title` matches it:
     - on a 404, it prints `There's no fork $fork yet. wingetcreate makes it.` and exits 0;
     - on anything but a 200, it prints `::error::Couldn't read the fork $fork, so <tag> wasn't submitted. The answer began: '<first>'. Run this workflow with action 'submit' and tag '<tag>' again.` and exits 1.
  3. `gh api "repos/microsoft/winget-pkgs/compare/master...${owner}:winget-pkgs:master" --jq .ahead_by`. Write `${owner}`, because `$owner:` would read as a scoped variable. When it fails, or is anything but `0`, the step prints `::error::The fork $fork has commits of its own on master, so syncing it would merge them and <tag> wasn't submitted. Put its master back to microsoft/winget-pkgs's, then run this workflow with action 'submit' and tag '<tag>' (docs/RELEASING.md).` and exits 1. When the call itself failed, the message is the read error's instead.
  4. `$answer = gh api -X POST "repos/$fork/merge-upstream" -f branch=master 2>&1`.
     - On a non-zero exit, it prints a fresh `[guid]::NewGuid()` token as `::stop-commands::<token>`, then `GitHub's answer: $answer`, then `::<token>::`. After that it prints `::error::GitHub wouldn't bring the fork $fork up to date with microsoft/winget-pkgs, so <tag> wasn't submitted. Press Sync fork on https://github.com/$fork, then run this workflow with action 'submit' and tag '<tag>' (docs/RELEASING.md).` and exits 1.
     - Otherwise, it prints `The fork $fork is up to date.`.

  The step's env holds only `GH_TOKEN: ${{ secrets.WINGET_TOKEN }}` and `TAG: ${{ steps.release.outputs.tag }}`. Above it, a comment says why it is there: wingetcreate stops when it can't sync the fork itself, as for v0.1.0-alpha.2 (issue #216). Asking GitHub first says what to do, and a fork that doesn't exist yet is made by wingetcreate.

- [ ] **Step 4: Run the tests and see them pass**

Run: `pnpm test scripts/release/`
Expected: PASS, every file in the folder.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml scripts/release/workflow.test.ts
git commit -m "feat(release): a tag's winget submission waits while WINGET_HOLD is set, a cancel stops the winget jobs, and the fork is synced before a submission."
```

### Task 3: the docs

**Files:**
- Modify: `docs/RELEASING.md`
- Create: `docs/research/2026-11-26-winget-hold-and-fork-sync.md`

- [ ] **Step 1: `docs/RELEASING.md`**

- **"Cut a release":** a new step before the tag, which decides whether the tag will submit.
  - While `WINGET_HOLD` is set, it doesn't (link to the new section). Set or delete the variable before pushing the tag.
  - If it will submit, press **Sync fork** on the fork's page first. The workflow tries the sync itself, but GitHub refuses it whenever winget-pkgs has changed one of its own workflow files since the fork's last sync. That happened within a week after alpha.1.
- **The table's `winget-submit` row:**
  - While `WINGET_HOLD` is set, a tag's run ends green with a notice and submits nothing.
  - Before `wingetcreate`, the job syncs the fork. It stops with what to do when GitHub refuses, or when the fork has commits of its own.
- **A new section, "Holding the submission", after "The winget token":**
  - `gh variable set WINGET_HOLD --body true` holds the submission, and `gh variable delete WINGET_HOLD` lets tags submit again. Anything but an empty value or exactly `false` holds.
  - A manual `submit` ignores the hold, and `winget-check` still runs.
  - Cancelling a tagged run stops the winget jobs, but a cancel holds nothing once `wingetcreate` has opened the pull request, so use the variable.
  - A held tag's run that is re-run doesn't submit either, because `publish` fails on the existing release. Use `action=submit`.
  - The variable is set as of 2026-11-26, until microsoft/winget-pkgs#482096 is merged.
- **"A release that went out before the token, or whose submission failed":** a submission that stopped at "Bring the fork up to date" is run again after Sync fork.

- [ ] **Step 2: the research note**

`docs/research/2026-11-26-winget-hold-and-fork-sync.md` holds:
- what happened in run 45872192223 (from issue #216);
- the fork facts and upstream commit `805e6d11` from "What was verified before planning", with the cause marked unproven;
- the plan review's GitHub Actions facts, with their sources (the workflow cancellation and contexts docs, the sync-fork REST docs, `actions/runner`'s `ActionCommand.cs`);
- Task 4's results: run ids, job conclusions, the step `winget-check` was in when cancelled, and how long the cancel took to end it.

- [ ] **Step 3: Commit**

```bash
git add docs/RELEASING.md docs/research/2026-11-26-winget-hold-and-fork-sync.md
git commit -m "docs(release): how to hold a submission, why a cancel isn't one, and the fork's sync."
```

### Task 4: the check on GitHub

- [ ] **Step 1: push the branch and open a draft pull request**

`git push -u origin phase-9c`, then `gh pr create --draft`, with the body's sections as CLAUDE.md lists them, to be filled in at the end. The pull request starts the release dry run by itself, because it touches `release.yml` and `scripts/release/**`.

- [ ] **Step 2: the pull request's dry run**

`gh run list --branch phase-9c --workflow release.yml --event pull_request -L 1`, then `gh run view <id> --json jobs --jq '.jobs[] | "\(.name) \(.conclusion)"'`.
Expected: `build success`, and `publish`, `winget-check` and `winget-submit` skipped.

- [ ] **Step 3: a manual check, cancelled**

1. Run `gh workflow run release.yml --ref phase-9c -f action=check -f tag=v0.1.0-alpha.2`.
2. Poll the run's jobs every few seconds. The moment `winget-check` is `in_progress`, in any step, run `gh run cancel <id>` once.
3. Note which step it was in, and the time.

Expected:
- `winget-check` started with `publish` skipped, so the condition still lets a manual check through.
- It ended `cancelled` within a couple of minutes of the one cancel.
- `winget-submit` never started (`skipped` or `cancelled`, no steps).

If `winget-check` ends `success` (the cancel came too late), start one more check run and cancel it earlier. Record each run in the research note.

- [ ] **Step 4: Commit the note's results**

```bash
git add docs/research/2026-11-26-winget-hold-and-fork-sync.md
git commit -m "docs(release): the check run on GitHub: a manual check starts and one cancel stops it."
```

## After the tasks

1. The checks CI runs (`marshell-ship`), the final review on Opus (`marshell-reviewer`) and one fix pass, then the pull request ready for review, a green CI, `pre-merge-review` and the merge.
2. After the merge:
   - Run `gh variable set WINGET_HOLD --body true` and `gh variable list`.
   - Close #216 with a comment that points to the decisions.
   - RELEASING.md says how alpha.2 goes out once #482096 is merged.

## Known limits (for the pull request)

- The hold on a real tag, the fork-sync step, and what a cancel does to `winget-submit` on a tag's run can't be tried without a tag or a submission. All three run first on the next release, and the tests pin their logic.
- That the token's scope causes the failed sync is likely, not proven.
