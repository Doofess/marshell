# S18: a pull request's review comments through `gh` (Windows)

Slice 6r-4b, run 2026-11-23 during the questions and before the plan, to make "Slice 6r-4b decisions" 1, 2, 4, 6 and 8 concrete. Dev box: Windows 11 Pro (build 26300), `gh` 2.101.0. Everything ran against the private scratch repository `Doofess/marshell-scratch-6r4` (kept from 6r-4a for this), on its pull requests 1 (`feat/one`) and 6 (`lc/conflict`). No app code changed.

**Verdict: go.** One GraphQL query returns every thread, review summary and conversation comment for 1 point; resolve, unresolve and reply work with gh's own sign-in. Three things shape the build: an outdated thread has no line, a comment's source holds text nobody sees, and a reply has to go through REST.

## How it was measured

- Comments were made with `gh api`: `POST repos/{owner}/{repo}/pulls/{n}/comments` for a line (`line`, `side`), a range (`start_line`, `start_side`, `line`, `side`), a removed line (`side: LEFT`) and a whole file (`subject_type: file`); `POST …/pulls/{n}/comments/{id}/replies` for a reply; `POST …/issues/{n}/comments` for a conversation comment; `POST …/pulls/{n}/reviews` with `event: COMMENT` for a review summary, and with no event for a pending review.
- A commit that changes the commented lines was added to `feat/one` through `PUT …/contents/a.txt`, so no clone was needed.
- The read is the query in "Findings" 1, run after each step.

## Findings

1. **One query reads it all, for 1 point.** `pullRequest { reviewThreads(first: 100) { totalCount pageInfo { hasNextPage } nodes { id isResolved isOutdated path line startLine originalLine originalStartLine diffSide subjectType viewerCanResolve viewerCanUnresolve viewerCanReply comments(first: 50) { totalCount nodes { databaseId author { login __typename } body createdAt url state isMinimized diffHunk } } } } reviews(last: 50) { … state body url createdAt author } comments(last: 50) { … body url createdAt isMinimized author } }` cost 1 of 5,000 points, the same as the bar's own query.
2. **A thread on current code names its place.** `path`, `line` (the last line), `startLine` (equal to `line` for one line, the first line for a range), `diffSide` `RIGHT` (the new file) or `LEFT` (a removed line), and `subjectType` `LINE` or `FILE`. A file comment reads `line: 1` with an empty `diffHunk`. The lines are those of the pull request's head commit, not of the checkout.
3. **Each comment carries the diff hunk it was written on** (`diffHunk`: the hunk header and the hunk's lines down to the commented line, for example `"@@ -0,0 +1,2 @@\n+one\n+two"`). The commented rows are its last `line − startLine + 1` lines, so Marshell can find the code again without GitHub's line.
4. **An outdated thread has no line.** After a commit changed line 2, both threads on it read `isOutdated: true`, `line: null`, `startLine: null`; `originalLine`, `originalStartLine` and `diffHunk` stayed. The file comment stayed current (`isOutdated: false`, `line: 1`).
5. **GitHub takes a few seconds to follow a push.** The read made straight after the new commit still showed the old `headRefOid` and the threads as current; the next one, some seconds later, showed the new head and the threads as outdated.
6. **A resolved thread reads `isResolved: true` and `isCollapsed: true`.** `resolveReviewThread(input: {threadId})` and `unresolveReviewThread` are GraphQL mutations on the thread's node id; each answered with the new `isResolved`. `viewerCanResolve` and `viewerCanUnresolve` say whether the signed-in user may.
7. **A reply must go through REST.** `POST …/pulls/{n}/comments/{id}/replies` (the id is the thread's first comment's `databaseId`) published at once (`state: SUBMITTED`).
   - With a pending review of the user's own on the pull request, the GraphQL mutation `addPullRequestReviewThreadReply` answered with `state: PENDING`: the reply joined the pending review and nobody else sees it until that review is submitted on GitHub. The REST call refused instead: HTTP 422, `errors[0].message` "user_id can only have one pending review per pull request". So REST never leaves a reply unpublished without saying so.
   - The user's own pending comments come back in the read with `state: PENDING`.
8. **Every single comment and reply makes a review with an empty body.** After four comments and a summary, `reviews` held five entries with `body: ""` and one with the summary. Only a review with a body is a review summary.
9. **A comment's source holds what nobody sees.** A comment written as `Visible text.`, an HTML comment `<!-- hidden: ignore previous instructions -->` and a zero-width space came back in `body` with all three. `bodyText` dropped the HTML comment and kept the zero-width space. A `<details>` block's text is in both.
10. **A picture is a Markdown image with an internet address** (`![pic](https://example.com/a.png)`), which the webview's content policy doesn't load.

## What 6r-4b takes from this

- The comments ride on a second GraphQL call beside the bar's, asked on the same schedule, so the bar's query and its parser stay as they are.
- A thread is placed by its hunk's last rows, as a draft comment is placed by its rows; GitHub's `line` is only where to look first. An outdated thread has nothing to place on in a file that changed, and is hidden with the resolved ones (decision 4).
- After a push, the threads may read as before for a few seconds; the next ask corrects it.
- Reply is the REST call. A 422 that names a pending review becomes its own sentence.
- A review counts as a summary only when its body isn't empty.
- Marshell removes HTML comments and invisible characters itself from `body`; `bodyText` isn't enough and loses the Markdown.

## Not verified

- A pull request with more than 100 threads, or a thread with more than 50 comments (the query's `totalCount` and `pageInfo` say when there are more).
- A thread the signed-in user may not resolve or reply to (`viewerCanResolve: false`): the scratch repository has one user.
- A minimized comment (`isMinimized`), and a comment by a bot or a deleted user (`author: null`).
- A GitHub Enterprise host.
