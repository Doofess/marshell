# Holding a winget submission, and the fork that couldn't be synced (issue #216, slice 9c)

- Date: 2026-11-26.
- Where: the release workflow's runs on GitHub, the GitHub REST API (read-only calls), GitHub's and the runner's documentation and source. Nothing was tagged, submitted or synced.
- Outcome: a cancel didn't stop the winget jobs because their conditions used `always()`. The fork sync most likely failed on the token's scope, which is not proven. Slice 9c adds a repository variable that holds a tag's submission, uses `!cancelled()` in the two jobs, and syncs the fork in a step of its own before `wingetcreate` runs.

## What happened

`v0.1.0-alpha.2` was tagged with its winget submission meant to be held, by cancelling run 45872192223 while `winget-check` ran.

| Job | Result | Started | Ended |
| --- | --- | --- | --- |
| `build` | success | 17:58:50 | 18:10:18 |
| `publish` | success | 18:10:20 | 18:10:31 |
| `winget-check` | success, after the cancel | 18:10:33 | 18:11:37 |
| `winget-submit` | failure | 18:11:39 | 18:12:26 |

Cancel requests went out every few seconds from the moment `winget-check` started. Both jobs ran to their end, because a job whose `if` uses `always()` isn't stopped by a cancel. `winget-submit` ran `wingetcreate submit`, which failed:

```
Manifest validation succeeded: True
Submitting pull request for manifest...
The forked repository could not be synced with the upstream commits. Sync your fork manually and try again.
```

No pull request for alpha.2 reached `microsoft/winget-pkgs`. alpha.1's run (43942007171) had looked as if a cancel held it, but its check had run into its 30-minute limit instead.

## Why the fork couldn't be synced

These facts were read on 2026-11-26 through the REST API:

- `Doofess/winget-pkgs` is a fork of `microsoft/winget-pkgs` with default branch `master`. It is 0 commits ahead and 6,677 behind, so the sync would have been a fast-forward.
- The fork's `master` was last changed 2026-11-17, when alpha.1's pull request was opened.
- Since then, upstream changed `.github/` three times. One of those is a workflow file: `805e6d11` (2026-11-24, "Harden spell-check reporting jobs") changed `.github/workflows/spellCheck.yaml`. The other two, `f3523ba7` and `9929662f`, changed `.github/policies/`.
- `WINGET_TOKEN` is a classic token with `public_repo` only (`docs/RELEASING.md`).
- GitHub doesn't let a token without the `workflow` scope create or update a workflow file, and that covers bringing one into a fork (GitHub community discussion 164432).

So the token's scope is the likely cause. It is not proven: GitHub's own answer was never seen, because `wingetcreate` prints only its own message. The new step prints GitHub's answer, so the next refusal will show it.

Since winget-pkgs changes its workflows within days, a sync by the workflow will often be refused. RELEASING.md therefore says to press Sync fork, as the owner in the browser, before a release that submits. Giving the token the `workflow` scope would also let it change this repository's own workflows, and was turned down (PLAN §9, slice 9c decision 3).

## What GitHub Actions does here

The plan review (2026-11-26) checked these against the docs and the runner's source:

- `!cancelled()` is a status function, so a job with it still runs when a job it needs was skipped. That is how a manual `check` runs with `publish` skipped. When the run is cancelled, the conditions of running jobs are evaluated again, and a job whose condition is now false is cancelled ([workflow cancellation](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-cancellation)). A job that hadn't started when a run was cancelled ends `cancelled` with no steps (run 45860766849).
- A YAML value that starts with `!` is a tag, so the condition is written `${{ !cancelled() && (…) }}`.
- `vars.<name>` can be used in a step's `env` on every event, and it is an empty string when the variable doesn't exist ([contexts](https://docs.github.com/en/actions/reference/workflows-and-actions/contexts#vars-context)).
- `POST repos/{owner}/{repo}/merge-upstream` answers 200 with `merge_type` `none`, `fast-forward` or `merge`, and 409 or 422 when it can't sync ([sync a fork branch](https://docs.github.com/en/rest/branches/branches#sync-a-fork-branch-with-the-upstream-repository)). Which code a token without the `workflow` scope gets isn't documented.
- The runner reads a `::` command at the start of a line. It also reads a legacy `##[` command anywhere in a line (`actions/runner`, `src/Runner.Common/ActionCommand.cs`). `::stop-commands::<token>` turns both off until `::<token>::`, which is how the new step prints GitHub's answer.
- GitHub's `pwsh` shell is PowerShell 7.4 on `windows-latest`. A native command's exit code doesn't throw there (`$PSNativeCommandUseErrorActionPreference` is `$false`), and neither does redirected stderr.

The new step's read-only calls ran on the development PC with the owner's own `gh` login, not with `WINGET_TOKEN`: `gh api user` gave `Doofess`, `gh api -i repos/Doofess/winget-pkgs` began `HTTP/2.0 200 OK`, the compare gave `ahead_by` 0, and a repository that doesn't exist began `HTTP/2.0 404 Not Found`. The step's script parses in PowerShell's parser with no errors.

## The check on GitHub

(Filled in by slice 9c's Task 4.)
