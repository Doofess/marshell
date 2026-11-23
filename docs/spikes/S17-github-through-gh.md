# S17: GitHub through `gh` with no git, and a push that can be stopped (Windows)

Slice 6r-4a, run 2026-11-23 after the questions and before the plan, to make "Slice 6r-4a decisions" 2, 5, 8, 16 and 18 concrete. Dev box: Windows 11 Pro (build 26300), `gh` 2.101.0, Git for Windows. Everything ran against a private scratch repository made for it, `Doofess/marshell-scratch-6r4` (the user's OK, 2026-11-23), from a scratch clone and an empty "neutral" folder under `C:\dev\lc-6r4a-spike`. No app code changed.

**Verdict: go.** Every GitHub call the slice needs works through `gh api` from a folder that is no checkout, with no git on the PATH, and a push can be read as it runs and stopped, provided the whole process tree is stopped.

## How it was measured

- The `gh` calls ran from the empty folder with `PATH` holding only gh's folder, `System32` and the shell's own tools, so a call that needed git would have failed. `GH_PROMPT_DISABLED=1`, `GH_NO_UPDATE_NOTIFIER=1` and `NO_COLOR=1` were set.
- Bodies went to gh as JSON on standard input (`gh api <path> --input -`), never on the command line.
- The pushes ran in the scratch clone as `git push --progress --porcelain -u origin feat/one`, with a `pre-push` hook that prints, sleeps and prints.
- The stop was measured by starting the push with a 45 s hook, listing the process tree after 3 s, ending the first `git.exe` alone, and listing again.

## Findings

1. **`gh api` needs no git and no checkout.** The lookup, create, ready, draft, merge and status calls below all answered from the neutral folder with git off the PATH.
2. **One GraphQL query is the whole bar, for 1 point.** `repository { id viewerPermission viewerDefaultMergeMethod mergeCommitAllowed squashMergeAllowed rebaseMergeAllowed autoMergeAllowed deleteBranchOnMerge isFork parent defaultBranchRef ref(qualifiedName:) pullRequests(headRefName:, first: 5, orderBy: CREATED_AT DESC) { … statusCheckRollup { contexts(first: 100) } } }` cost 1 of 5,000 points.
   - `ref(qualifiedName: "refs/heads/<branch>").target.oid` is GitHub's head of the branch, so "commits to push" needs no fetch: it is the count from that id to the local `HEAD`.
   - A branch with no pull request gives an empty `nodes`. A branch that isn't on GitHub gives `ref: null`.
   - `isCrossRepository` tells a pull request whose head is in another repository (decision 13 leaves those out).
   - A check is a `CheckRun` (`name`, `status` QUEUED / IN_PROGRESS / COMPLETED, `conclusion`, `detailsUrl`, `startedAt`, `completedAt`, `checkSuite.workflowRun.databaseId` and `.workflow.name`) or a `StatusContext` (`context`, `state` PENDING / SUCCESS / FAILURE / ERROR, `targetUrl`). The run id for `gh run view <id> --log-failed` is `workflowRun.databaseId`; a `StatusContext` has none.
   - `statusCheckRollup` is `null` on a commit with no checks yet.
3. **`mergeable` and `mergeStateStatus` read UNKNOWN for a while after the base or the head moves.** Seconds after a commit landed on `main`, pull request 1 read `UNKNOWN` / `UNKNOWN`; it had read `MERGEABLE` / `UNSTABLE` before. The bar has to treat UNKNOWN as "ask again soon", not as a state to show.
4. **Create is `POST repos/{owner}/{repo}/pulls`** with `{title, head, base, body, draft}`. It answered with `number`, `html_url` and `head.sha`.
   - A second create for the same branch: HTTP 422, `errors[0].message` "A pull request already exists for Doofess:feat/one.", gh's exit code 1.
   - A head that isn't on GitHub: HTTP 422, `errors[0]` `{field: "head", code: "invalid"}`.
5. **Ready and draft are GraphQL mutations on the pull request's node id:** `markPullRequestReadyForReview` and `convertPullRequestToDraft`. Each answered with the new `isDraft`, in both directions.
6. **Merge is `PUT repos/{owner}/{repo}/pulls/{n}/merge`** with `{merge_method, sha}`, where `sha` is the head the bar showed.
   - The right `sha`: `{"merged": true, "message": "Pull Request successfully merged"}`. The same call again answered the same, exit code 0.
   - A wrong `sha`: HTTP 409, "Head branch was modified. Review and try the merge again." So a push that lands between the bar's last look and the click can't be merged unseen.
   - A conflicting pull request: HTTP 405, "Pull Request has merge conflicts".
   - `deleteBranchOnMerge` was false here and the branch stayed after the merge.
7. **Auto-merge was refused here, and its refusal is readable.** `enablePullRequestAutoMerge` answered `errors[0].message` "Auto merge is not allowed for this repository" with exit code 1; the query's `autoMergeAllowed` was false and `viewerCanEnableAutoMerge` false. The accepted path was not seen: neither this repository nor `Doofess/marshell` allows auto-merge.
8. **The signed-out and broken cases can be told apart.**

   | Case | What gh did |
   | --- | --- |
   | No sign-in (`GH_CONFIG_DIR` at an empty folder) | `gh auth status --json hosts` printed `{"hosts":{}}`, exit code 0; `gh api` printed "To get started with GitHub CLI, please run: gh auth login", exit code 4 |
   | A bad token (`GH_TOKEN=ghp_bad`) | HTTP 401 "Bad credentials", exit code 1 |
   | No network (a dead proxy) | `Get "https://api.github.com/…": proxyconnect tcp: …`, exit code 1, nothing on standard output |
   | No such repository | GraphQL `errors[0].type` "NOT_FOUND", exit code 1 |

   A signed-in host reads `{"state":"success","active":true,"host":"github.com","login":…,"gitProtocol":"https"}` in 0.4 s. `gh auth status` checks the token over the network, so it is too slow and too chatty to run on every poll.
9. **A push's streams.** With `--progress --porcelain`:
   - Standard output carries the hook's own output, then `To <url>`, one line per ref (`*<TAB>refs/heads/feat/one:refs/heads/feat/one<TAB>[new branch]`, or `!<TAB>…<TAB>[rejected] (fetch first)`), `branch 'feat/one' set up to track 'origin/feat/one'.` and `Done`.
   - Standard error carries git's progress, where one line is redrawn with carriage returns ("Counting objects:  14% (1/7)\rCounting objects:  28% (2/7)\r…"), and the `remote:` lines, among them GitHub's "Create a pull request for 'feat/one' on GitHub by visiting:".
   - A push the remote refuses exits with 1 and "hint: Updates were rejected because the remote contains work that you do not have locally." No flag made it force.
10. **Ending `git.exe` alone stops nothing.** The push was a tree: `git.exe` (the one started), a `conhost.exe`, a second `git.exe`, under it a third `git.exe` with `git-remote-https.exe`, and `sh.exe` for the hook. After the first `git.exe` was ended, all five others still ran, the hook among them. Ending every process of the tree stopped it, and GitHub's branch had not moved (`ref.target.oid` was still the earlier commit). So Cancel has to end the tree, which on Windows means a job object the push is started in.
11. **GitHub Actions did not start in the private scratch repository.** Each job ended at once with "The job was not started because recent account payments have failed or your spending limit needs to be increased." They still show as three failed `CheckRun`s with a workflow name and a run id, which is what a failed check looks like to the bar. A running or passing state was made with the commit status API instead (`POST repos/{owner}/{repo}/statuses/<sha>` with `state=pending`), which gives a `StatusContext`. A `CheckRun` that is queued was seen once, on the merge commit. A running `CheckRun` can be seen read-only on a pull request of `Doofess/marshell`, whose CI does run.

## What 6r-4a takes from this

- Marshell talks to GitHub only through `gh api` (and `gh auth status --json hosts` to learn the signed-in hosts), started in a folder of its own, with the repository named in the call. `gh pr …` isn't used: those commands look for a checkout.
- Bodies go in on standard input as JSON.
- "Commits to push" is counted in the sandbox from GitHub's `ref.target.oid` to `HEAD`. When that commit isn't in the local repository, GitHub's branch has work the local one lacks, and the bar says so (decision 8).
- Merge sends the head the bar showed as `sha`, and shows GitHub's sentence when it answers 405 or 409.
- UNKNOWN merge states make the next ask come soon, and the button waits.
- The push runs as `git push --progress --porcelain -u origin <branch>` in a job object. Its two streams are read as they come, a carriage return redraws the last line, and Cancel ends the job.
- A failure is told by gh's exit code 4 (no sign-in), a 401 (a token that no longer works), an empty standard output with a non-zero exit (no network), and otherwise GitHub's own `message`.

## Not verified

- An accepted auto-merge (`enablePullRequestAutoMerge` on a repository that allows it) and "Merge when checks pass" turning into a merge.
- A GitHub Enterprise host (`gh api --hostname`).
- A required check or review holding a merge (`mergeStateStatus` BLOCKED): branch protection on a private repository needs a paid plan.
- A credential prompt during the push. The clone used the credentials Git for Windows already had.
