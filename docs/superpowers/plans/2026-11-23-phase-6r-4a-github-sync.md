# Slice 6r-4a: GitHub in the Changes panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show the session branch's pull request and CI checks in a bar in the Changes panel, and let the user create the pull request (with its push), push later commits, merge, and hand a failed check to the agent.

**Architecture:** A new core module `crates/core/src/github/` owns everything GitHub: it reads the checkout's `origin` address through the git sandbox's own config read, runs `gh api` from a folder of Marshell's own with the repository named in the call, and turns one GraphQL answer into a small status the webview polls on its own route (`POST /v1/sessions/{tab}/github`). The push is the one plain `git` call: `git push --progress --porcelain -u origin <branch>` in the real checkout, started inside a Windows job object so Cancel ends the whole tree, with its output polled by the sheet. The frontend adds a bar, a checks list and three sheets to `src/features/changes/`.

**Tech Stack:** Rust (`marshell-core`, `marshell-protocol`), React and TypeScript, the user's own `gh` and `git`. No new dependency.

**Spec:** `docs/PLAN.md` §9, "Slice 6r-4a decisions (2026-11-23)", decisions 1 to 23 (20 to 22 were asked after the plan review; 23 is this plan's "Decided, not asked"). Facts: `docs/spikes/S17-github-through-gh.md`. The panel as built: "Slice 6r-3 decisions" and `docs/superpowers/plans/2026-11-23-phase-6r-3-changes.md`.

## Global Constraints

- Rust tests: `cargo test -p marshell-core -j 4` (never more jobs). `cargo fmt --all` before every commit; `cargo clippy --workspace --all-targets -j 4 -- -D warnings` stays clean. After a protocol change: `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated`. Frontend: `pnpm test <files>`, and `pnpm build` at the end of each frontend task.
- No new dependency. No change to `guard.rs`, token handling, or file permissions under the home. Every new route sits behind the same bearer-token guard as `/changes`.
- **gh runs only through `github/gh.rs`:** `gh api …` and `gh auth status --json hosts`, never `gh pr`, `gh repo` or `gh run`. Its working folder is `<home>/gh`, never a checkout. A request body goes in on standard input as JSON, never on the command line.
- **The repository (host, owner, name) always comes from the checkout's `origin`, read in the core.** No route takes it from the webview.
- **Plain git runs only in `github/push.rs`, and only as `git -c protocol.allow=never -c protocol.https.allow=always -c protocol.ssh.allow=always push --progress --porcelain -u origin refs/heads/<b>:refs/heads/<b>`.** The refspec is always the full one, so a branch named `+x` can't force and one named `-x` can't be an option. No `--force`, no other subcommand. Every read stays in the sandbox (`gitbox`).
- **Marshell pushes only to the repository the bar shows** (asked 2026-11-23, after the plan review): `origin` must have exactly one effective push address, and it must parse to the same host, owner and name as the bar's repository. Otherwise Push and Create are off with "origin pushes somewhere other than {repo}, so push from the terminal."
- **A branch name is checked with `github::valid_branch(b)`:** `refs::valid_name("refs/heads/{b}")` (which takes a full ref, never a bare branch) and `b` doesn't start with `-` or `+`. A commit id is checked with `refs::Oid::parse`, a node id with `^[A-Za-z0-9_=-]{1,100}$`.
- **A name written into a command a person or an agent may run** (`git switch {default}`, `gh run view … --repo {repo}`) must also match `^[A-Za-z0-9._/][A-Za-z0-9._/-]*$` (`github::command_safe`). A name that doesn't is named in prose only, and the command is left out.
- **Every revision given to the sandbox's git is a commit id,** never a name: the sandbox has no refs. Ids come from `Check::head_oid`, `refs::resolve`, or GitHub's answer after `Oid::parse`.
- **Every action names the checkout it was confirmed for.** `PushRequest`, `CreatePull`, `PullMergeRequest` and `PullAction` carry `checkout`; the core answers 409 "The session moved to another folder. Look again before you go on." when it isn't the session's checkout now, and for merge and pull actions when `id` and `number` aren't the pull request the core last saw for that checkout and branch.
- Nothing asks GitHub while the Changes panel is hidden, the window is unfocused, or "Show pull requests from GitHub" is off.
- Tests use a temp folder as the Marshell home and a fake gh (`gh-marshell-test`, Task 4), found by its full path. No test reaches the network or runs the real `gh`. Fixtures use `octo-org/widgets`, never a real name.
- Multi-CLI: the three typed requests are the same bytes for Claude Code, Codex, Copilot CLI, Qwen Code and Antigravity. Nothing vendor-specific is added.
- Every new control works with the keyboard alone and in forced colours; a check's state is a glyph and a word, never colour alone. Every sheet and menu calls `useCoverPage`.
- Constants, exact: `GH_DEADLINE` 15 s, `GH_MAX_BYTES` 4 MiB, `GH_PATH_AGE` 60 s, `AUTH_AGE` 10 min, `FRESH_GAP` 3 s, `NEXT_IDLE_MS` 30,000, `NEXT_BUSY_MS` 10,000, `RATE_MIN_MS` 60,000, `PUSH_LINES` 2,000, `PUSH_LINE_CHARS` 1,000, `PUSH_KEPT` 10 min, `PUSH_POLL_MS` 250, `TEMPLATE_MAX` 64 KiB, `TITLE_MAX` 256, `BODY_MAX` 65,536, `COMMITS_MAX` 100, `SUBJECT_MAX` 200, `CHECKS_MAX` 100.
- Copy that the decisions fix is used word for word (the tables in Tasks 11 to 15). A count agrees in number: "1 commit to push", "3 commits to push".
- Commit messages read `type(scope): what changed, as a sentence.` and end with the session's attribution lines. Workers never run `git add`, `commit`, `checkout`, `restore`, `reset`, `stash` or `clean`; the main session commits. Files keep LF line endings.
- A hook runs rustfmt on every edited Rust file: read the file again before the next edit.

## Review Focus

1. A push lands on GitHub between the bar's last look and the Merge click: the merge must not take commits the user didn't see. Merge sends the head the bar showed, and GitHub's 409 sentence is shown. Test in Task 10 (`merge_409_says_the_branch_moved`).
2. A branch, check or workflow name with a leading `-` or `+`, a quote, a backtick, `$`, `;`, `|` or a control character (git allows most of these, and GitHub's default branch name is the repository owner's to choose): it never becomes an option or a forced refspec to git, never types a line break into a session, and never rides inside a command Marshell copies or asks an agent to run. Tests in Task 9 (`push_refuses_a_branch_that_starts_with_a_dash_or_a_plus`, `the_command_pushes_a_full_refspec`) and Task 11 (`control_and_format_characters_in_a_name_become_spaces`, `a_backtick_in_a_name_cannot_close_the_command`, `a_shell_character_in_the_default_branch_drops_the_command`).
3. GitHub can't be reached, or gh's sign-in ends, while a pull request shows: the bar says so, offers no action on a stale state, and the diff keeps working. Tests in Task 6 (`an_unreachable_ask_drops_the_snapshot`) and Task 12 (`an unreachable reply shows the line and no buttons`).
4. Cancel while the pre-push hook runs, and a webview reload while a push runs: the whole process tree ends on Cancel and GitHub's branch doesn't move; after a reload the bar finds the running push again. Tests in Task 9 (`push_cancel_ends_the_hook_and_moves_nothing`, `a_running_push_is_named_in_the_status`).
5. Two sessions on one checkout, or a shell that moves between the confirm and the click: a second push is refused while one runs, a branch that moved since the confirm was drawn is refused, and so is a session that now stands in another folder. Tests in Task 9 (`push_refuses_a_second_push`, `push_refuses_a_moved_head`, `push_refuses_another_checkout_than_the_confirmed_one`).
6. `origin` set up to push somewhere else (`pushurl`, `pushInsteadOf`, two addresses, a local path): the confirm must not name one repository while git pushes to another. Tests in Task 2 (`a_pushurl_to_another_repository_does_not_match`) and Task 9 (`push_refuses_an_origin_that_pushes_elsewhere`).

---

### Task 0: The decisions, the spike and this plan (done while planning)

**Files:** `docs/PLAN.md` (§9, "Slice 6r-4a decisions"), `docs/spikes/S17-github-through-gh.md`, this file.

- [ ] **Step 1:** Add decisions 20 to 22 (asked after the plan review) and 23 ("Decided, not asked", the list at the end of this plan) to `docs/PLAN.md`, and commit it with this plan: `docs(changes): slice 6r-4a's plan and what it settles without asking.`

### Task 1: Wire types and the settings switch

**Files:**
- Modify: `crates/protocol/src/api.rs` (after `CommentsReply`; `ChangesReply`; `ChangesSettings`)
- Modify: `crates/core/src/settings.rs` (the `changes` section's defaults and lenient read), `crates/core/src/server/changes.rs` (`empty_reply`, `build_reply`: the two new `ChangesReply` fields, filled with `None` for now)
- Modify: `src/features/changes/changesSettings.tsx` (`CHANGES_DEFAULTS`), `src/features/changes/changesMenu.ts` (`SETTING_LABELS` is a `Record` over every `ChangesSettings` key, so it needs "Show pull requests from GitHub" now, or `pnpm build` fails; `changesChoice`), `src/features/changes/ChangesSection.tsx` (`type Flag`), TS test fixtures that build a `ChangesReply` or `ChangesSettings`
- Regenerated: `src/generated/*.ts`

`ChangesSettings` derives `Default` today, which would make a missing `pull_requests` read `false`: write `impl Default` by hand (`hide_whitespace: false, wrap: false, pull_requests: true`), and add the new name to `lenient_changes` in `settings.rs`, where a value that isn't a boolean reads as the default. The switch is not shown in the ⋯ menu's checked rows (it lives in Settings and behind the bar's Hide).

**Interfaces (Produces):** all derive what `ChangesReply` derives (serde, `TS`, `Clone`, `Debug`, `PartialEq`); enums serialise in `snake_case`.

```rust
pub struct GithubRequest { pub fresh: bool }
pub enum GithubKind { Off, None, NoGh, SignedOut, Unreachable, NotFound, RateLimited, DefaultBranch, Ready }
pub struct GithubReply {
    pub kind: GithubKind, pub checkout: String, pub host: String, pub repo: String, // "owner/name"
    pub branch: Option<String>, pub default_branch: Option<String>,
    pub fork_of: Option<String>, pub compare_url: Option<String>,
    pub pull: Option<PullStatus>, pub closed: Option<u32>,   // the newest pull request, when it was closed unmerged
    pub on_github: bool, pub to_push: u32, pub github_ahead: bool, pub can_write: bool,
    pub merge_methods: Vec<MergeMethod>, pub auto_merge_allowed: bool,
    pub push: Option<String>,                                // the id of a push of this checkout that is running
    pub push_blocked: Option<String>,                        // why Marshell won't push here (origin pushes elsewhere)
    pub reason: Option<String>,
    #[ts(type = "number")] pub next_ms: u64,
}
pub enum MergeMethod { Merge, Squash, Rebase }
pub enum PullState { Open, Draft, Merged, Closed }
pub enum Readiness { Ready, Checking, Conflicts, Blocked, Behind }
pub enum ReviewNeed { None, Required, ChangesRequested, Approved }
pub struct PullStatus {
    pub id: String, pub number: u32, pub title: String, pub url: String, pub state: PullState,
    pub base: String, pub head: String, pub readiness: Readiness, pub review: ReviewNeed,
    pub checks: Vec<CiCheck>, pub more_checks: u32,
    pub auto_merge: Option<MergeMethod>, pub can_auto_merge: bool,
}
pub enum CheckKind { Run, Status }
pub enum CheckState { Failed, Cancelled, Running, Waiting, Passed, Neutral, Skipped }
pub struct CiCheck {
    pub kind: CheckKind, pub state: CheckState, pub name: String, pub workflow: Option<String>, pub url: Option<String>,
    #[ts(type = "number | null")] pub run_id: Option<u64>,
    #[ts(type = "number | null")] pub started_ms: Option<u64>,
    #[ts(type = "number | null")] pub completed_ms: Option<u64>,
}
pub struct PushPlanRequest { pub create: bool }
pub struct OutgoingCommit { pub id: String, pub subject: String }
pub struct PushPlan {
    pub checkout: String, pub branch: String, pub head: String, pub moved: bool, pub on_github: bool,
    pub to_push: u32, pub commits: Vec<OutgoingCommit>, pub more: u32, pub uncommitted: u32,
    pub base: String, pub title: String, pub body: String, pub refusal: Option<String>,
}
pub struct PushRequest { pub checkout: String, pub branch: String, pub head: String }
pub struct PushStarted { pub id: String }
pub enum PushState { Running, Pushed, Failed, Cancelled }
pub struct PushProgress {
    pub state: PushState, #[ts(type = "number")] pub start: u64, pub lines: Vec<String>,
    #[ts(type = "number")] pub total: u64, pub reason: Option<String>,
}
pub struct CreatePull { pub checkout: String, pub title: String, pub body: String, pub base: String, pub draft: bool, pub branch: String, pub head: String }
pub struct CreatedPull { pub number: u32, pub url: String }
pub struct PullMergeRequest { pub checkout: String, pub number: u32, pub method: MergeMethod, pub head: String, pub id: String, pub auto: bool }
pub enum PullActionKind { Ready, Draft, CancelAutoMerge }
pub struct PullAction { pub checkout: String, pub id: String, pub number: u32, pub action: PullActionKind }#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AskRequest {
    FixCheck { copy: bool, number: u32, name: String, workflow: Option<String>, url: Option<String>, #[ts(type = "number | null")] run_id: Option<u64> },
    OpenPull { copy: bool },
    SwitchAndPull { copy: bool, number: u32 },
}
pub enum AskForm { Typed, Copy }
pub struct AskReply { pub form: AskForm, pub text: String }
```

`ChangesReply` gains `pub head: Option<String>` (the full commit id of `HEAD`) and `pub pr_base_unfetched: Option<String>`. `ChangesSettings` gains `pub pull_requests: bool`, default `true`; a value that isn't a boolean reads as `true`.

- [ ] **Step 1:** Add the types and a round-trip test per tagged enum in `api.rs`'s tests: `ask_request_reads_its_kind_tag` (`{"kind":"fix_check","copy":false,"number":182,"name":"build","workflow":null,"url":null,"run_id":null}` parses to `FixCheck`), `github_kind_is_snake_case` (`NoGh` writes `"no_gh"`).
- [ ] **Step 2:** In `settings.rs`, update the existing Changes settings test so `pull_requests` defaults to `true` and `"no"` reads as `true`, and add `a_settings_file_without_pull_requests_reads_it_as_on` (a `changes` section holding only the two older keys). If ts-rs refuses `#[ts(type = …)]` on a field of the tagged enum's variant, make `FixCheck`'s fields a struct of their own (`FixCheck(FixCheckAsk)`); the JSON stays the same.
- [ ] **Step 3:** Run `cargo test -p marshell-protocol -j 4`, `cargo test -p marshell-core -j 4 --lib settings::`, `pnpm check:generated`, `pnpm test src/features/changes`, `pnpm build`. Expected: all pass, no `bigint` in `src/generated/`.
- [ ] **Step 4:** Commit: `feat(protocol): the pull request bar's wire types and its settings switch.`

### Task 2: The checkout's GitHub identity (effort: high)

**Files:**
- Modify: `crates/core/src/gitbox/config.rs` (`key_regex`, a new `REMOTE_KEYS`), `crates/core/src/gitbox/mod.rs` (`GitBox` struct, `read_config`, a new accessor), `crates/core/src/gitbox/refs.rs` (`valid_name` becomes `pub(crate)`), `crates/core/src/changes/mod.rs` (`Check`, `compute`), `crates/core/src/lib.rs` (`pub mod github;`)
- Create: `crates/core/src/github/mod.rs` (`valid_branch`, `command_safe`), `crates/core/src/github/remote.rs`

**Interfaces (Produces):**

```rust
// gitbox/config.rs
pub const REMOTE_KEYS: &[&str] = &["remote.origin.url", "remote.origin.pushurl"];   // read, never passed on: `allowed_pair` still refuses them
// gitbox/mod.rs
pub struct Origin { pub urls: Vec<String>, pub push_urls: Vec<String>, pub instead_of: Vec<(String, String)>, pub push_instead_of: Vec<(String, String)> } // raw values, in read order
impl GitBox { pub fn origin(&self) -> &Origin }
// changes/mod.rs: Check gains `pub origin: Origin` (not part of the stamp) and `pub head_oid: Option<String>`
// github/remote.rs
#[derive(Clone, PartialEq, Eq)] pub struct RemoteRepo { pub host: String, pub owner: String, pub name: String }
pub fn parse_remote(url: &str) -> Option<RemoteRepo>;
pub fn apply_instead_of(url: &str, rules: &[(String, String)]) -> Option<String>;  // (base, insteadOf); the longest matching prefix wins; None when none matches
pub fn fetch_repo(origin: &Origin) -> Option<RemoteRepo>;      // the first url, with insteadOf applied
pub fn push_repo(origin: &Origin) -> Result<RemoteRepo, ()>;   // where `git push origin` goes, when that is one repository Marshell can name
pub fn github_host(host: &str, signed_in: &[String]) -> Option<String>;     // github.com always; another host only when signed in
pub fn valid_branch(b: &str) -> bool;    // in github/mod.rs: refs::valid_name("refs/heads/{b}") and no leading '-' or '+'
pub fn command_safe(name: &str) -> bool; // in github/mod.rs: ^[A-Za-z0-9._/][A-Za-z0-9._/-]*$
```

The config read adds `remote.origin.url`, `remote.origin.pushurl` and every `url.<base>.insteadof` and `url.<base>.pushinsteadof` to the keys its `--get-regexp` asks for, from the system, user and repository files it already reads without includes (`config_entries`, which runs git with the sandbox's own settings and reads values only). `refs::valid_name` becomes `pub(crate)`.

`push_repo` follows git's own rule (`remote.c`, `alias_all_urls`), and refuses what it can't name:
- More than one `url`, or more than one `pushurl` → `Err` (git would push to several places).
- One `pushurl` → that value with `insteadOf` applied (`pushInsteadOf` doesn't apply to a `pushurl`).
- No `pushurl` → the `url` with `pushInsteadOf` applied when a rule matches, else with `insteadOf` applied.
- The result must pass `parse_remote`, else `Err` (a local path, `ext::`, another form).

The bar's repository is `fetch_repo`; Push and Create need `push_repo(origin) == Ok(fetch_repo)`. A setting that only an `[include]`d file holds isn't seen (decision 23, item 3).

`parse_remote` rules:
- Accepted: `https://` and `http://` (a `user[:password]@` part and a port are dropped), `ssh://`, `git+ssh://`, `ssh+git://` and `git://` (a user and a port are dropped), and the scp form `[user@]host:owner/name` where the host is longer than one character and holds no `/` (so `C:\repo` is never a host).
- The path is exactly `owner/name`, with one optional `.git` suffix and one optional trailing `/`.
- The host is lowercased; `ssh.github.com` and `www.github.com` become `github.com`.
- Refused (`None`): a local path, `file://`, more or fewer than two path parts, an owner outside `[A-Za-z0-9-]{1,39}`, a name outside `[A-Za-z0-9._-]{1,100}`, the names `.` and `..`.

- [ ] **Step 1: Write the tests** in `github/remote.rs`:

```rust
#[test] fn parses_https_with_and_without_dot_git()        // "https://github.com/octo-org/widgets.git" and without -> ("github.com","octo-org","widgets")
#[test] fn parses_scp_like_ssh_and_skips_a_drive_letter() // "git@github.com:octo-org/widgets.git" parses; "C:/work/widgets" and "c:widgets/x" are None
#[test] fn parses_ssh_url_with_port_and_ssh_github_com()  // "ssh://git@ssh.github.com:443/octo-org/widgets" -> host "github.com"
#[test] fn drops_user_and_password_from_https()           // "https://u:secret@GitHub.com/octo-org/widgets/" -> host "github.com"; Debug of the result holds no "secret"
#[test] fn refuses_local_paths_and_file_urls()            // "../widgets", "/srv/git/widgets.git", "file:///c:/x/y", "//server/share/o/n"
#[test] fn refuses_extra_segments_and_bad_names()         // "https://github.com/a/b/c", ".../octo-org/..", an owner with "_", a name with a space
#[test] fn instead_of_takes_the_longest_prefix()          // rules ("https://github.com/","gh:") and ("https://github.com/octo-org/","gh:oo/"): "gh:oo/widgets" -> ".../octo-org/widgets"
#[test] fn github_host_matches_the_github_com_family_and_signed_in_hosts() // "github.com" with [] is Some; "ghe.example.com" only when listed; "gitlab.com" None
#[test] fn push_goes_where_fetch_does_with_one_url()      // push_repo == Ok(fetch_repo)
#[test] fn a_pushurl_to_another_repository_does_not_match() // pushurl ".../octo-org/other.git": push_repo is Ok but != fetch_repo
#[test] fn a_pushurl_to_a_local_path_is_refused()         // "C:/bare.git", "../bare", "ext::sh -c x": Err
#[test] fn two_urls_or_two_pushurls_are_refused()
#[test] fn push_instead_of_applies_only_without_a_pushurl() // ("git@github.com:","https://github.com/") rewrites the url for push; with a pushurl set it is ignored
#[test] fn an_ssh_pushurl_of_the_same_repository_matches() // url https://github.com/octo-org/widgets, pushurl git@github.com:octo-org/widgets.git
#[test] fn valid_branch_refuses_a_leading_dash_or_plus_and_bad_ref_names() // "-x", "+main", "a..b", "a b", "" refused; "feat/login", "v1.2" pass
#[test] fn command_safe_refuses_shell_characters()        // "main;x", "x$(y)", "a|b", "a&b", "a'b", "a>b", "-x" refused; "release/1.2", "main" pass
```

and in `gitbox/mod.rs`'s tests (each builds a repository with the module's existing helpers):

```rust
#[test] fn the_origin_url_is_read_without_includes()                 // an [include] file that sets remote.origin.url is not seen
#[test] fn a_linked_worktree_reads_origin_from_the_common_config()
#[test] fn the_url_never_becomes_a_dash_c_pair()                     // the command's args hold no "remote.origin.url="
#[test] fn key_regex_names_the_remote_keys()                         // in config.rs
#[test] fn pushurl_and_push_instead_of_are_read()                    // Origin holds every value, in read order
```

- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 --lib github:: gitbox::`. Expected: the new tests fail to compile or fail.
- [ ] **Step 3:** Implement to the interfaces above. `compute` fills `Check::origin` from `GitBox::origin()` and `Check::head_oid` from the head it already resolves.
- [ ] **Step 4:** Run the same command, then `cargo test -p marshell-core -j 4 --test changes_api`. Expected: `0 failed`.
- [ ] **Step 5:** Commit: `feat(changes): the sandbox reads origin's address, and a GitHub repository is told from it.`

### Task 3: A child started inside its job, and a watched run (effort: high)

`git::stream_command` reads exit code 1 as done (kept for `git grep`) and keeps only the end of standard error, and `job::ProcessTree` adopts a child after it has started, so neither can run the push (S17 findings 9 and 10).

**Files:**
- Modify: `crates/core/src/job.rs`, `crates/core/src/git.rs`
- Test: both files' `tests` modules, `crates/core/tests/job_object.rs` (uses the existing `job-probe` binary)

**Interfaces (Produces):**

```rust
// job.rs
impl ProcessTree {
    /// Starts `cmd` suspended, puts it in a job of its own, then lets it run, so nothing it starts is missed.
    /// The job has no limits, as `adopt`'s: a child that ends on its own leaves what it started running, and only
    /// `kill` ends the tree. On other OSes it only spawns. If the job can't be made or joined, the child is killed
    /// and the error returned.
    pub fn spawn(cmd: &mut std::process::Command) -> std::io::Result<(std::process::Child, ProcessTree)>;
}
// git.rs
pub enum Pipe { Out, Err }
pub enum Ended { Exited(Option<i32>), Cancelled }
/// Runs with no deadline. `each` gets every chunk as it arrives, from two reader threads through one channel.
pub(crate) fn run_watched(cmd: std::process::Command, cancel: &std::sync::atomic::AtomicBool, each: impl FnMut(Pipe, &[u8])) -> anyhow::Result<Ended>;
```

- Port the suspended start from `crates/hook/src/job.rs` (`assign`, `resume`) and `crates/hook/src/statusline.rs` (the `CREATE_SUSPENDED` spawn); the core already has the `windows-sys` features. The job has **no** `KILL_ON_JOB_CLOSE` (the hook crate's has): a push that ends on its own must leave alone what it started, a sign-in window from Git Credential Manager for one.
- The child is already in Marshell's own job (`tie_children_to_app`); a second, nested job is what `adopt` does today, and Marshell's own still ends everything if Marshell crashes.
- `run_watched` sets no window and closes standard input. It polls `cancel` every 50 ms; on cancel it calls `ProcessTree::kill` and returns `Cancelled`.
- After the child has ended it waits at most 200 ms more for the pipes, as `stream_command` does, then returns: a hook's background child that keeps a pipe open must not hold the run. The readers are left to end on their own, and nothing is killed.

- [ ] **Step 1: Write the tests:** `watched_reads_both_pipes`, `watched_reports_exit_code_one` (`Ended::Exited(Some(1))`, not an error), `watched_says_when_the_program_cannot_start`, `watched_returns_when_a_grandchild_keeps_a_pipe_open` (it returns within 2 s and the grandchild still runs; the test then ends it by PID), and on Windows `watched_cancel_ends_a_grandchild_started_at_once` and `a_tree_started_in_its_job_ends_whole` (`job-probe` starts a grandchild that writes its PID, the test cancels and waits for that PID to be gone).
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 --lib git:: job::` and `--test job_object`. Expected: fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same commands. Expected: `0 failed`.
- [ ] **Step 5:** Commit: `feat(core): a child can start inside its own job, and a run can be watched and stopped whole.`

### Task 4: Running gh (effort: high)

**Files:**
- Create: `crates/core/src/github/gh.rs`, `crates/core/src/bin/gh-marshell-test.rs` (beside `fake-agent.rs`, built the same way)
- Modify: `crates/core/src/paths.rs` (`pub fn gh_dir(home: &Path) -> PathBuf`, `<home>/gh`, beside `git_sandbox_dir`), `crates/core/src/agents/mod.rs` (a new `run_bounded_with`, below; `run_bounded` keeps its signature for its five callers and calls it)
- Test: `github/gh.rs` tests, `crates/core/tests/github_api.rs` (new; shares `tests/common`)

**Interfaces (Produces):**

```rust
// agents/mod.rs
pub(crate) enum Bounded { NotStarted, Deadline, TooMuch, Done(i32, Vec<u8>, Vec<u8>) }
pub(crate) fn run_bounded_with(cmd: Command, timeout: Duration, input: Option<Vec<u8>>, max_bytes: usize) -> Bounded; // `input` is written on its own thread, then standard input is closed
// github/gh.rs
pub enum GhError { Missing, SignedOut, Unreachable(String), NotFound, RateLimited { reset_ms: Option<u64> }, Refused { status: Option<u16>, message: String }, TooLong, Unreadable }
pub fn find_gh(path: &std::ffi::OsStr) -> Option<PathBuf>;
pub fn gh_env(vars: impl Iterator<Item = (OsString, OsString)>, gh_dir: &Path) -> Vec<(OsString, OsString)>;
pub fn api(gh: &Path, home: &Path, host: &str, args: &[&str], body: Option<&serde_json::Value>) -> Result<serde_json::Value, GhError>;
pub fn auth_hosts(gh: &Path, home: &Path) -> Result<Vec<String>, GhError>;
pub fn map_failure(code: i32, stdout: &[u8], stderr: &[u8]) -> GhError;   // pure
```

- `find_gh` has its own loop over `std::env::split_paths(path)` (`command_path_in` returns the first PATHEXT match, a `.cmd` or a bare file included, so it can't be used): the first folder that is absolute, that `refused` and `links_refused` don't reject, and that holds a file `gh.exe` on Windows (`gh` elsewhere). The caller passes `detect::tab_path()`, the PATH a session gets, so a gh installed while Marshell runs is found within `GH_PATH_AGE`.
- `api` runs `gh api --hostname <host> <args…>` (and `--input -` when there is a body) with the working folder `paths::gh_dir(home)`, created like `git_sandbox_dir` with no permission change. `Bounded::NotStarted` → `Missing` (and the cached gh path is dropped); `Deadline` and `TooMuch` → `TooLong`; standard output that isn't JSON on exit code 0 → `Unreadable`.
- Callers map every `GhError`: the status route turns `TooLong` and `Unreadable` into `Unreachable` with the reasons "GitHub took too long to answer." and "GitHub's answer couldn't be read.", and `Refused` into `Unreachable` with GitHub's message; the action routes (Task 10) answer 503 for `Missing`, `SignedOut`, `Unreachable`, `TooLong`, `Unreadable` and `RateLimited`, and 409 for `Refused` and `NotFound`.
- `gh_env` matches names without case on Windows (`Path`, `SYSTEMROOT`), starts from nothing and keeps only: `SystemRoot`, `SystemDrive`, `windir`, `TEMP`, `TMP`, `USERPROFILE`, `HOMEDRIVE`, `HOMEPATH`, `APPDATA`, `LOCALAPPDATA`, `ProgramData`, `HOME`, `XDG_CONFIG_HOME`, `GH_CONFIG_DIR`, `GH_TOKEN`, `GITHUB_TOKEN`, `GH_ENTERPRISE_TOKEN`, `GITHUB_ENTERPRISE_TOKEN`, `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY` (either case), `SSL_CERT_FILE`, `SSL_CERT_DIR`. It sets `PATH` to gh's own folder plus `System32`, and `GH_PROMPT_DISABLED=1`, `GH_NO_UPDATE_NOTIFIER=1`, `GH_NO_EXTENSION_UPDATE_NOTIFIER=1`, `GH_SPINNER_DISABLED=1`, `NO_COLOR=1`, `CLICOLOR=0`. So `GH_HOST`, `GH_REPO`, `GH_DEBUG`, `GH_PAGER`, `PAGER` and every `GIT_*` are gone.
- `auth_hosts` runs `gh auth status --json hosts` and returns the hosts that hold an entry with `"state": "success"` (the shape is `{"hosts": {"github.com": [{"state": "success", "active": true, "host": …}]}}`).
- `map_failure`, in this order: exit code 4 → `SignedOut`; `(HTTP 401)` in standard error → `SignedOut` (never looked for in standard output, which holds GitHub's data, a pull request's title for one); a non-zero exit with empty standard output → `Unreachable(last line of standard error)`; GraphQL `errors[0].type == "NOT_FOUND"` or `HTTP 404` → `NotFound`; `HTTP 403` with "rate limit", or GraphQL `RATE_LIMITED` → `RateLimited`; otherwise `Refused { status from "(HTTP nnn)", message: errors[0].message, else message }`. A GraphQL answer with exit code 0 and a non-empty `errors` is mapped the same way.
- `gh-marshell-test` reads `script.json` in its working folder (a list of `{match: [arg substrings], exit, stdout, stderr}`; the first whose substrings all appear in the arguments wins), appends `{args, stdin, env_names}` to `calls.jsonl` there, and exits 2 on no match.

- [ ] **Step 1: Write the tests:** `finds_only_an_exe_on_an_absolute_path_dir` (a `gh.cmd` in an earlier folder and a `gh.exe` in a relative one are passed over), `env_keeps_the_allowlist_and_sets_no_prompt` (a `GIT_DIR`, `GH_REPO` and `GH_DEBUG` in, none out; `GH_TOKEN` and a `Path`-cased `systemroot` kept), `exit_4_is_signed_out`, `http_401_is_signed_out`, `a_401_in_githubs_data_is_not_signed_out`, `a_program_that_cannot_start_is_missing`, `an_answer_over_the_cap_is_too_long`, `bounded_writes_the_input_and_closes_it`, `empty_stdout_and_a_failure_is_unreachable`, `graphql_not_found_is_not_found`, `graphql_errors_with_exit_0_are_refused`, `rate_limit_is_told_apart`, `auth_hosts_skip_failed_accounts` (use S17 finding 8's strings as the inputs), and in `tests/github_api.rs` `gh_runs_in_marshells_folder_with_the_body_on_stdin` (the fake's `calls.jsonl` is under `<home>/gh`, its `args` hold no body text, its `stdin` holds the JSON).
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 --lib github::gh` and `--test github_api`. Expected: fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same commands, and `cargo test -p marshell-core -j 4 --lib agents::` (for `run_bounded`'s callers). Expected: `0 failed`.
- [ ] **Step 5:** Commit: `feat(changes): gh runs from Marshell's own folder with a short environment, and its failures are told apart.`

### Task 5: The query and the status it gives (effort: high)

**Files:**
- Create: `crates/core/src/github/query.rs` (the query text and the serde shapes of its answer), `crates/core/src/github/status.rs` (the pure mapping), `crates/core/tests/fixtures/github/*.json`
- Test: `github/status.rs` tests

**Interfaces (Produces):**

```rust
// query.rs
pub const STATUS_QUERY: &str;                      // below
pub fn status_args(repo: &RemoteRepo, branch: &str) -> Vec<String>;   // "graphql", "-f", "query=…", "-f", "o=…", "-f", "n=…", "-f", "b=refs/heads/<branch>", "-f", "h=<branch>"
// status.rs
pub fn plain(text: &str, max: usize) -> String;    // in github/mod.rs, used for every text that comes from GitHub or from git: below
pub struct Snapshot { pub default_branch: Option<String>, pub fork_of: Option<String>, pub parent_default: Option<String>, pub branch_oid: Option<String>,
    pub pulls: Vec<Candidate>,                      // same-repository pull requests, newest first
    pub can_write: bool, pub merge_methods: Vec<MergeMethod>, pub auto_merge_allowed: bool, pub next_ms: u64 }
pub struct Candidate { pub status: PullStatus }
pub fn snapshot_of(answer: &serde_json::Value) -> Result<Snapshot, GhError>;
```

`STATUS_QUERY` is S17's (`C:\dev\lc-6r4a-spike\scripts\c-status-ready.sh`), with the checks read from the pull request's last commit, 5 pull requests newest first, and `isRequired` left out:

```graphql
query($o:String!,$n:String!,$b:String!,$h:String!){rateLimit{remaining resetAt} repository(owner:$o,name:$n){viewerPermission viewerDefaultMergeMethod mergeCommitAllowed squashMergeAllowed rebaseMergeAllowed autoMergeAllowed isFork parent{nameWithOwner defaultBranchRef{name}} defaultBranchRef{name} ref(qualifiedName:$b){target{oid}} pullRequests(headRefName:$h,first:5,orderBy:{field:CREATED_AT,direction:DESC}){nodes{id number title url state isDraft isCrossRepository headRefOid baseRefName mergeable mergeStateStatus reviewDecision viewerCanEnableAutoMerge autoMergeRequest{mergeMethod} commits(last:1){nodes{commit{statusCheckRollup{contexts(first:100){totalCount nodes{__typename ... on CheckRun{name status conclusion detailsUrl startedAt completedAt checkSuite{workflowRun{databaseId workflow{name}}}} ... on StatusContext{context state targetUrl createdAt}}}}}}}}}}}
```

Mapping:
- A node with `isCrossRepository == true` is left out.
- `PullState`: `OPEN` with `isDraft` → Draft; `OPEN` → Open; `MERGED` → Merged; `CLOSED` → Closed.
- `CheckRun`: status QUEUED, WAITING, PENDING, REQUESTED → Waiting; IN_PROGRESS → Running; COMPLETED by conclusion: SUCCESS → Passed; FAILURE, TIMED_OUT, STARTUP_FAILURE, ACTION_REQUIRED → Failed; CANCELLED → Cancelled; SKIPPED, STALE → Skipped; NEUTRAL → Neutral. `run_id` is `workflowRun.databaseId`, `workflow` its workflow's name, `url` is `detailsUrl`.
- `StatusContext`: PENDING, EXPECTED → Running; SUCCESS → Passed; FAILURE, ERROR → Failed. `name` is `context`, `url` is `targetUrl`, no `run_id`.
- A `url` is kept only when it starts with `https://` and holds no control character or space.
- `plain(text, max)`: every control character, and every character that hides or reorders text, becomes one space, and the result is cut at `max` characters. The second group, exactly: U+00AD, U+061C, U+200B to U+200F, U+2028 to U+202E, U+2060 to U+206F, U+FEFF, U+FFF9 to U+FFFB, U+E0001, U+E0020 to U+E007F. A check's name and workflow, a pull request's title and a branch name from GitHub go through `plain(_, 200)`.
- A `baseRefName` or a default branch name that `valid_branch` refuses makes the snapshot `Refused` with "GitHub named a branch Marshell can't use."
- Order: Failed, Cancelled, Running, Waiting, then Passed, Neutral, Skipped, each group in GitHub's order. `more_checks` is `totalCount` less the nodes read.
- `Readiness`, first match: `mergeable` CONFLICTING or `mergeStateStatus` DIRTY → Conflicts; either one UNKNOWN → Checking; BLOCKED → Blocked; BEHIND → Behind; anything else (CLEAN, HAS_HOOKS, UNSTABLE, DRAFT) → Ready. That a pull request is a draft is read from `state`, never from here.
- `ReviewNeed`: `REVIEW_REQUIRED` → Required; `CHANGES_REQUESTED` → ChangesRequested; `APPROVED` → Approved; null → None.
- `can_write`: `viewerPermission` is ADMIN, MAINTAIN or WRITE. `merge_methods`: the allowed ones, `viewerDefaultMergeMethod` first.
- `next_ms`: `NEXT_BUSY_MS` while the first open pull request has a Running or Waiting check, an `auto_merge`, or `Checking`; else `NEXT_IDLE_MS`.
- Times: `agents::common::rfc3339_ms`.

- [ ] **Step 1: Write the fixtures** from S17's answers with the names replaced (`octo-org/widgets`): `open-failing.json`, `draft.json`, `merged.json`, `no-pull.json` (empty `nodes`), `no-branch.json` (`ref: null`), `unknown.json`, `cross-repo.json`, `no-rollup.json` (`statusCheckRollup: null`), `not-found.json`.
- [ ] **Step 2: Write the tests:** `picks_same_repository_pull_requests_newest_first`, `a_cross_repository_pull_request_is_left_out`, `checks_put_failures_first`, `status_context_and_check_run_share_states`, `unknown_merge_state_is_checking_and_asks_in_10_s`, `a_running_check_asks_in_10_s_and_a_settled_one_in_30`, `merge_methods_put_githubs_default_first`, `a_null_rollup_is_no_checks`, `a_non_https_link_is_dropped`, `a_repository_that_is_null_is_not_found`, `a_draft_merge_state_is_not_checking` (DRAFT with MERGEABLE is Ready and asks in 30 s), `conflicts_come_before_unknown`, `plain_replaces_control_and_bidi_characters` ("a\u{202E}b\u{2028}c\nd" becomes "a b c d"), `a_base_branch_that_starts_with_a_dash_is_refused`.
- [ ] **Step 3:** Run `cargo test -p marshell-core -j 4 --lib github::status github::query`. Expected: fail; then implement; then `0 failed`.
- [ ] **Step 4:** Commit: `feat(changes): one GraphQL answer becomes the pull request's status.`

### Task 6: The status route (effort: high)

**Files:**
- Modify: `crates/core/src/github/mod.rs` (`GithubState`), `crates/core/src/changes/cache.rs` (`with_box`), `crates/core/src/server/mod.rs` (`AppState` holds `github: GithubState`, `with_gh` for tests as `with_status_runner` is, the route), `crates/core/src/server/changes.rs` (fill `ChangesReply.head`; `with_task_base(home, target)` becomes `pub(super) fn with_bases(st, target)` at its two callers, `answer` and `tree_marks`, doing only what it does today)
- Create: `crates/core/src/server/github.rs`
- Test: `crates/core/tests/github_api.rs`

**Interfaces (Produces):**

```rust
// changes/cache.rs
impl ChangesCache { pub fn with_box<T>(&self, home: &Path, target: &Target, deadline: Instant, cancel: &AtomicBool,
    f: impl FnOnce(&GitBox, &Check) -> Result<T, Halt>) -> Result<T, ChangesError>; }
// github/mod.rs
pub struct GithubState { /* snapshots by (checkout, branch), each with a generation; one ask in flight per key; the gh path (GH_PATH_AGE); auth hosts (AUTH_AGE); pr_bases by checkout; pushes */ }
impl GithubState {
    pub fn pr_base(&self, checkout: &Path) -> Option<(String, String)>;       // (branch, base) of the open pull request last seen here, for Task 7
    pub fn mark_stale(&self, checkout: &Path);                                // after a push, create, merge or pull action
    pub fn pull_seen(&self, checkout: &Path, branch: &str) -> Option<(String, u32)>; // (node id, number) of the pull request last reported, for Task 10
}
pub struct Local { pub on_github: bool, pub to_push: u32, pub github_ahead: bool }
pub fn local_counts(git: &GitBox, check: &Check, branch_oid: Option<&str>, base_oid: Option<&str>) -> Result<Local, Halt>;
// server/changes.rs
pub(super) fn with_bases(st: &AppState, target: Target) -> Target;   // today's with_task_base, renamed; Task 7 adds the pull request's base to it
// server/github.rs
pub async fn github(/* State, Path(tab), Json(GithubRequest) */) -> Response;   // POST /v1/sessions/{tab}/github -> GithubReply
```

The route, in order:
1. `settings.changes.pull_requests == false` → `Off` (gh is never looked for).
2. The checkout, as `/changes` finds it: `changes_target`, then `with_bases`, then the cache's check. The target is built exactly as `/changes` builds it, so the two routes share the checkout's one cache slot and never swap checks with different bases. A Changes error or no checkout → `None`.
3. `fetch_repo(&check.origin)` → `github_host` (asking `auth_hosts` only for a host other than `github.com`); no match → `None`.
4. gh not found → `NoGh`. A detached `HEAD`, or a branch `valid_branch` refuses → `None`.
5. The snapshot:
   - `fresh: false` answers from the stored snapshot and runs no gh; with none stored yet it is treated as `fresh: true`.
   - `fresh: true` answers from the stored snapshot when it is under `FRESH_GAP` old; else one `gh::api` call, shared by every ask for that key that arrives while it runs.
   - Each key has a generation that `mark_stale` raises. A call notes the generation it started under; when it ends under a higher one, its answer is neither stored nor given to those waiting, and the ask runs again once. So an ask that began before a merge can't bring the old state back.
   - `SignedOut`, `Unreachable`, `NotFound`, `RateLimited` become the kind of the same name with `reason`, and the snapshot for that key is dropped, so no stale pull request is served. `next_ms` is `NEXT_IDLE_MS`, or for `RateLimited` the time to the reset and at least `RATE_MIN_MS`.
6. The branch is the default branch → `DefaultBranch`.
7. `pull`: the first Open or Draft candidate; else the newest Merged or Closed one whose `head` is in the local branch's history (its commit is here and `rev-list --count <head_oid>..<that>` is 0), so a reused branch name doesn't read "Merged"; a Closed one is reported as `closed: Some(number)` with `pull: None`.
8. `local_counts`, always fresh, in the sandbox, with commit ids only (`check.head_oid`; the sandbox has no refs):
   - is GitHub's commit here: `rev-parse --verify --quiet <branch_oid>^{commit}`, empty output = missing = `github_ahead`;
   - `to_push`: `rev-list --count <branch_oid>..<head_oid>`; `github_ahead` also when `rev-list --count <head_oid>..<branch_oid>` > 0;
   - a branch that isn't on GitHub: `on_github = false`, and `to_push` = `rev-list --count <base_oid>..<head_oid>`, where `base_oid` is `refs::resolve(git_dir, common, "refs/remotes/origin/<default>")`, else the check's base commit, else 0.
9. `push_blocked`: when `push_repo(&check.origin)` isn't `Ok` of the bar's repository, "origin pushes somewhere other than {repo}, so push from the terminal."
10. An Open or Draft pull request's (branch, base) is stored in `pr_bases` for the checkout, and removed when the branch has no open pull request; the Changes cache's slot for the checkout is dropped when that value changes, so the next `/changes` compares with the new base. `pull_seen` is stored.
11. A fork: `fork_of` is the parent's `nameWithOwner`, and `compare_url` is `https://{host}/{parent}/compare/{parent's default}...{owner}:{branch}?expand=1` with the branch percent-encoded (everything but `A-Z a-z 0-9 - . _ ~ /`).
12. `push` from the running pushes (Task 9, `None` until then).

- [ ] **Step 1: Write the tests** (each starts the test server with a fake gh and a temp repository whose `origin` is `https://github.com/octo-org/widgets.git`): `github_off_runs_no_gh` (the fake's `calls.jsonl` doesn't exist), `github_without_gh_says_so`, `github_signed_out_says_so`, `github_for_a_gitlab_origin_is_none`, `github_on_the_default_branch_says_so`, `github_counts_commits_to_push_from_githubs_branch`, `github_is_ahead_when_its_commit_is_missing`, `a_branch_that_is_not_on_github_counts_from_the_base`, `a_merged_pull_request_of_another_history_is_not_shown`, `a_second_fresh_ask_within_3_s_uses_the_snapshot` (one line in `calls.jsonl`), `an_unreachable_ask_drops_the_snapshot`, `a_not_fresh_ask_recounts_and_runs_no_gh`, `a_not_fresh_ask_with_no_snapshot_asks_github`, `an_answer_from_before_mark_stale_is_not_kept` (the fake gh sleeps; `mark_stale` lands meanwhile; `calls.jsonl` ends with two lines and the reply is the second answer), `a_pushurl_elsewhere_blocks_the_push_and_keeps_the_bar`, `a_fork_links_to_the_parents_default_branch_with_the_branch_encoded` (branch `fix/#12`), `github_route_needs_the_token`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 --test github_api`. Expected: fail; then implement; then `0 failed`, and `--test changes_api` still passes.
- [ ] **Step 3:** Commit: `feat(changes): a route answers the pull request's status for the session's checkout.`

### Task 7: The diff follows the pull request's base (decision 15)

**Files:**
- Modify: `crates/core/src/gitbox/refs.rs` (`base`), `crates/core/src/changes/mod.rs` (`Target`, `compute`, `Check`, the stamp), `crates/core/src/server/changes.rs` (`with_bases` also sets `target.pr_base`; `build_reply`, `empty_reply`), `src/features/changes/copy.ts`, `src/features/changes/ChangesPanel.tsx` (`noticesOf`), `src/features/changes/drafts.ts` (`sendMessageOf`)
- Test: `refs.rs` and `changes/mod.rs` tests, `crates/core/tests/changes_api.rs`, `src/features/changes/drafts.test.ts`, `src/features/changes/copy.test.ts`

**Interfaces:**
- Consumes: `GithubState::pr_base(checkout) -> Option<(String, String)>`, `with_bases` (Task 6), `valid_branch` (Task 2).
- Produces: `pub fn base(git_dir: &Path, common: &Path, task_base: Option<&str>, pr_base: Option<&str>) -> Result<(Option<Base>, Option<String>), RefError>`. A `pr_base` is tried first and resolves only `refs/remotes/origin/<b>` (label `origin/<b>`); when that ref is missing the order is today's and the second value is `Some(b)`. A `pr_base` that `valid_branch` refuses is ignored.
- `Target` gains `pr_base: Option<(String, String)>` (branch, base), set by `with_bases` from `GithubState::pr_base`. `compute` passes the base on to `refs::base` only when the head it resolved is on that branch: the target is built before the head is known.
- `Check` gains `pub pr_base_unfetched: Option<String>`, and it is part of the stamp's hash with the base's label and commit: otherwise a reply that differs only in this note has the old stamp, `/changes` answers "same", and the note never shows or clears. `ChangesReply.pr_base_unfetched` carries it.
- `copy.prBaseUnfetched(b)`: "This pull request targets develop, which isn't fetched here." (with `b` for develop).
- `sendMessageOf` names `drafts.base` whenever the drafts hold comments, so comments keep the base they were written against.

- [ ] **Step 1: Write the tests:** `base_follows_an_open_pull_requests_fetched_base`, `an_unfetched_pull_request_base_keeps_the_default_and_says_so`, `a_pull_request_base_of_another_branch_is_not_used`, `a_pull_request_base_comes_before_a_tasks_base`, `the_stamp_changes_when_the_unfetched_note_does`, in `changes_api.rs` `the_tree_and_the_panel_share_the_pull_requests_base` (two polls, one stamp) and `the_diff_moves_to_the_pull_requests_base_after_github_answers` (a `/changes` poll, a `/github` ask that reports base `develop`, then a `/changes` poll with a new stamp and the label `origin/develop`), and in `drafts.test.ts` `it("sends against the base the comments were written on")`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 --lib gitbox::refs changes::`, `--test changes_api`, `pnpm test src/features/changes`. Expected: fail; implement; `0 failed`; `pnpm build` passes.
- [ ] **Step 3:** Commit: `feat(changes): with an open pull request, the diff compares with that pull request's base.`

### Task 8: The plan for a push or a create, and the template

**Files:**
- Create: `crates/core/src/github/template.rs`, `crates/core/src/github/plan.rs`
- Modify: `crates/core/src/server/github.rs` (route), `crates/core/src/server/mod.rs`
- Test: both modules' tests, `tests/github_api.rs`

**Interfaces (Produces):**

```rust
pub fn pull_template(checkout: &Path) -> Option<String>;
pub fn outgoing(git: &GitBox, from_oid: &str, head_oid: &str) -> Result<(Vec<OutgoingCommit>, u32), Halt>;  // newest first, COMMITS_MAX, then the count of the rest
pub fn title_of(commits: &[OutgoingCommit], branch: &str) -> String;       // the only commit's subject, else the branch name as it is
pub fn body_of(template: Option<String>, commits: &[OutgoingCommit]) -> String; // the template, else "- <subject>" lines oldest first
pub fn refusal(reply: &GithubReply, detached: bool) -> Option<String>;     // the sentences below
// POST /v1/sessions/{tab}/github/plan, PushPlanRequest -> PushPlan
```

- `outgoing` runs `log --no-decorate --no-mailmap -z --format=%h%x1f%s --max-count=101 <from_oid>..<head_oid>` in the sandbox, with commit ids only. `from_oid` is GitHub's branch commit, or `local_counts`'s `base_oid` when the branch isn't on GitHub; these are the same commits `to_push` counts. A subject goes through `plain(_, SUBJECT_MAX)`.
- `uncommitted`: the count `build_reply` already has for the panel (the changed files against the head's commit id, plus the new files).
- `pull_template`: in `.github/`, then the checkout's root, then `docs/`, the first file named `pull_request_template` without ASCII case, with `.md`, `.txt` or no extension (at most 1,000 entries read per folder). A folder is listed only after `links_refused(folder)` is false, and the file is read only after `links_refused(file)` is false, with `read_capped(path, TEMPLATE_MAX)`. A BOM is dropped. A file that isn't UTF-8, and the folder `PULL_REQUEST_TEMPLATE/`, give `None`.
- `base` is the pull request's base when one is open, else GitHub's default branch. `moved` is `moved_from_launch` (what `ChangesReply.moved` is).
- `refusal`, first match:
  - detached → "This checkout is on no branch, so there is nothing to push.";
  - a branch `valid_branch` refuses → "That isn't a branch name Marshell can push.";
  - the default branch → "{branch} is the default branch on GitHub. Marshell doesn't push it.";
  - `push_blocked` → that sentence;
  - `github_ahead` → "GitHub's {branch} has commits this checkout doesn't. Pull them in the terminal." (the bar's own note, word for word);
  - kind isn't `Ready` → the reply's `reason`;
  - with `create: true`, an open pull request → "This branch already has pull request #{n}."

- [ ] **Step 1: Write the tests:** `finds_the_template_in_dot_github_then_root_then_docs`, `matches_any_case_and_txt`, `refuses_a_linked_template_and_a_file_over_64_kib`, `plan_lists_commits_newest_first_and_counts_the_rest`, `plan_title_is_the_only_subject_else_the_branch`, `plan_body_is_the_template_else_the_subjects`, `plan_counts_uncommitted_and_new_files`, `plan_refuses_a_detached_head_and_the_default_branch`, `plan_says_when_github_is_ahead`, `plan_refuses_when_origin_pushes_elsewhere`, `a_template_folder_that_is_a_link_is_not_listed`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 --lib github::template github::plan` and `--test github_api`. Expected: fail; implement; `0 failed`.
- [ ] **Step 3:** Commit: `feat(changes): what a push would send, and the pull request's first title and description.`

### Task 9: The push (effort: high)

**Files:**
- Create: `crates/core/src/github/push.rs`
- Modify: `crates/core/src/github/mod.rs` (`pushes` in `GithubState`), `crates/core/src/server/github.rs`, `crates/core/src/server/mod.rs`
- Test: `github/push.rs` tests, `tests/github_api.rs`

**Interfaces:**
- Consumes: `ProcessTree::spawn`, `git::run_watched`, `plan::refusal`, `valid_branch`, `push_repo`.
- Produces:

```rust
pub struct Transcript { /* at most PUSH_LINES lines of PUSH_LINE_CHARS characters; the oldest dropped */ }
impl Transcript { pub fn feed(&mut self, pipe: Pipe, bytes: &[u8]); pub fn since(&self, from: u64) -> (u64, Vec<String>, u64); } // (start, lines, total)
pub fn push_command(git: &Path, checkout: &Path, branch: &str, env: impl Iterator<Item = (OsString, OsString)>, extra: &[String]) -> Command;
pub fn pushed_to(stdout_line: &str) -> Option<&str>;   // the address in git's "To <address>" line
impl GithubState {
    pub fn pushes_running(&self) -> u32;
    pub fn stop_pushes(&self, wait: Duration);   // cancels every running push and waits for them to end, at most `wait`
}
// POST /v1/sessions/{tab}/push           PushRequest -> PushStarted
// GET  /v1/pushes/{id}?from=<n>          -> PushProgress (lines from n-1 on: only the last line can be redrawn)
// POST /v1/pushes/{id}/cancel            -> 204
// POST /v1/pushes/stop                   -> 204, after stop_pushes(2 s): the quit question's "Quit" (Task 16)
```

- `push_command`, exactly: `git -C <checkout> -c protocol.allow=never -c protocol.https.allow=always -c protocol.ssh.allow=always [-c <extra>]… push --progress --porcelain -u origin refs/heads/<branch>:refs/heads/<branch>`.
  - Settings on the command line win over the repository's, so a `file`, `ext` or other helper transport can't run even when a setting Marshell didn't see names one.
  - `extra` is empty outside tests. The route tests pass `protocol.file.allow=always` and `url.<a bare repository's path>.insteadOf=https://github.com/octo-org/widgets.git` through `AppState::with_push_config(Vec<String>)` (test-only, as `with_gh` is), so the push lands in a local bare repository while the sandbox still reads a GitHub address.
  - git is the one on the session PATH (`detect::tab_path()`), found as `find_gh` finds gh. The environment is Marshell's own, less `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_COMMON_DIR`, `GIT_NAMESPACE` and every `GIT_CONFIG*`, plus `GIT_TERMINAL_PROMPT=0`.
- It runs on its own named thread through `run_watched`, with no deadline. `Exited(Some(0))` → Pushed; any other exit → Failed with `reason` "git ended with code {n}."; a start failure → Failed with its sentence; `Cancelled` → Cancelled.
- When git's `To <address>` line doesn't parse to the bar's repository (a setting from an included file sent it elsewhere), the transcript gains the line "git pushed to {address}, which isn't {repo}." and a Pushed push carries it as `reason`.
- `Transcript::feed`: CSI and OSC escape sequences are dropped; a carriage return redraws the last line only when that line came from the same pipe; a UTF-8 character split over two chunks is kept whole; other control characters but a tab become U+FFFD.
- One push per checkout at a time. A finished push is kept `PUSH_KEPT`. A Pushed one calls `GithubState::mark_stale`. `GithubReply.push` names a running push of the checkout.
- The core's `shutdown` calls `stop_pushes(2 s)` before the sessions end, so no exit leaves a push running unseen.
- Refusals, 409 with `{reason}` (404 for an unknown tab or push id), checked in this order at the click, all on a fresh check of the checkout:
  1. the switch is off: "Pull requests from GitHub are switched off in Settings.";
  2. `checkout` isn't the session's checkout now: "The session moved to another folder. Look again before you go on.";
  3. `plan::refusal`'s sentences (no branch, a branch name Marshell can't push, the default branch, origin pushes elsewhere, GitHub ahead);
  4. `branch` or `head` isn't the checkout's `HEAD` now: "The branch moved since you looked. Check the commits again.";
  5. a push of this checkout runs: "A push of this checkout is already running."
- No session state refuses a push: it types nothing into the session.

- [ ] **Step 1: Write the tests.** `github/push.rs`: `a_carriage_return_redraws_the_last_line` (S17 finding 9's "Counting objects" bytes end as one line), `a_carriage_return_from_the_other_pipe_starts_a_line`, `escape_sequences_are_dropped`, `a_split_utf8_character_is_kept_whole`, `the_transcript_keeps_the_last_2000_lines`, `the_command_pushes_a_full_refspec` (the exact argument list for `feat/login`, ending `origin refs/heads/feat/login:refs/heads/feat/login`, with the three `protocol` pairs before `push` and no `GIT_DIR` in its environment), `pushed_to_reads_gits_to_line`. `tests/github_api.rs`, with a local bare repository reached through `with_push_config` and a `pre-push` hook: `push_runs_the_hook_and_streams_its_output`, `push_cancel_ends_the_hook_and_moves_nothing` (Windows; the hook's PID file names a process that is gone, the bare repository's branch is unchanged), `a_rejected_push_ends_failed_with_gits_output`, `push_refuses_a_moved_head`, `push_refuses_a_second_push`, `push_refuses_a_branch_that_starts_with_a_dash_or_a_plus` (a checkout whose `HEAD` is `refs/heads/+main`: 409, and the bare repository's `main` is unchanged), `push_refuses_the_default_branch`, `push_refuses_an_origin_that_pushes_elsewhere` (a `pushurl` to another repository, and one to a local path), `push_refuses_another_checkout_than_the_confirmed_one`, `a_file_address_is_refused_by_git_without_the_test_setting` (no `with_push_config`, `url.<bare>.insteadOf` set in an included file: git ends Failed and the bare repository is unchanged), `a_running_push_is_named_in_the_status`, `stop_ends_every_running_push`, `push_of_an_unknown_id_is_404`, `push_routes_need_the_token`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 --lib github::push` and `--test github_api`. Expected: fail; implement; `0 failed`.
- [ ] **Step 3:** Commit: `feat(changes): a confirmed push runs the user's own git, shows its output and can be stopped whole.`

### Task 10: Create, merge, auto-merge, ready and draft

**Files:**
- Create: `crates/core/src/github/actions.rs`
- Modify: `crates/core/src/server/github.rs`, `crates/core/src/server/mod.rs`
- Test: `tests/github_api.rs`

**Interfaces (Produces):**

```rust
// POST /v1/sessions/{tab}/github/create   CreatePull  -> CreatedPull
// POST /v1/sessions/{tab}/github/merge    PullMergeRequest -> 204
// POST /v1/sessions/{tab}/github/pull     PullAction  -> 204
```

- The path of every call is written by the core from the checkout's own repository (`format!("repos/{owner}/{name}/pulls")`): gh's `{owner}`/`{repo}` placeholders are never used, since they would be filled from a checkout.
- Every route first checks, on a fresh check of the checkout: the switch is on; `checkout` is the session's checkout now (409 "The session moved to another folder. Look again before you go on."); the origin is a GitHub repository.
- Create: `gh api --hostname <host> repos/<owner>/<name>/pulls --input -` with `{title, head: branch, base, body, draft}`. Checked first: title 1 to `TITLE_MAX` characters and equal to its own `plain`; body at most `BODY_MAX` with no control character but a line break and a tab; `base` and `branch` pass `valid_branch`; `branch` and `head` are the checkout's `HEAD` now.
- Merge and pull actions: `id` and `number` must be what `GithubState::pull_seen(checkout, branch)` holds, else 409 "That isn't this branch's pull request any more. Look again." A node id names a pull request anywhere on GitHub, so one the core didn't itself read for this checkout is never acted on.
- Merge, `auto: false`: `gh api -X PUT repos/<owner>/<name>/pulls/<number>/merge --input -` with `{merge_method: "merge" | "squash" | "rebase", sha: head}`.
- Merge, `auto: true`: the mutation `enablePullRequestAutoMerge(input: {pullRequestId, mergeMethod, expectedHeadOid})`, its values sent as GraphQL variables in the JSON body on standard input.
- Pull action: `markPullRequestReadyForReview`, `convertPullRequestToDraft`, `disablePullRequestAutoMerge`, on the node id, the same way.
- `id` must match `^[A-Za-z0-9_=-]{1,100}$`, `head` `refs::Oid::parse`, `number` ≥ 1; else 400.
- A `Refused` answer → 409 `{reason: plain(GitHub's message, 300)}` (so a 405 reads "Pull Request has merge conflicts" and a 409 "Head branch was modified. Review and try the merge again."); `NotFound` → 409 "GitHub has no repository {repo} that this sign-in can see."; the rest → 503 `{reason}`. Every success calls `mark_stale`.
- Reasons for 503, the bar's own sentences: "Pull requests need the GitHub CLI.", "Pull requests need a GitHub sign-in. Run `gh auth login` in a terminal.", "Can't reach GitHub: {gh's sentence}", "GitHub took too long to answer.", "GitHub's answer couldn't be read.", "GitHub's limit for this sign-in is used up."

- [ ] **Step 1: Write the tests:** `create_sends_title_head_base_body_draft_on_stdin`, `create_shows_githubs_422_message` ("A pull request already exists for octo-org:feat/login."), `create_refuses_a_control_character_in_the_title`, `merge_sends_the_head_the_bar_showed`, `merge_409_says_the_branch_moved`, `merge_405_says_conflicts`, `auto_merge_sends_the_expected_head`, `ready_and_draft_use_the_node_id`, `a_node_id_with_a_quote_is_400`, `a_node_id_the_core_did_not_report_is_refused` (a well-formed id of another pull request: 409, and no line in `calls.jsonl`), `actions_refuse_another_checkout_than_the_confirmed_one`, `the_repository_comes_from_the_checkout` (a body naming another repository changes nothing in the fake's `args`), `action_routes_need_the_token`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 --test github_api`. Expected: fail; implement; `0 failed`.
- [ ] **Step 3:** Commit: `feat(changes): create, merge, auto-merge, ready and draft go to GitHub through gh.`

### Task 11: The three requests typed to the agent (effort: high)

**Files:**
- Create: `crates/core/src/github/ask.rs`
- Modify: `crates/core/src/server/github.rs` (`POST /v1/sessions/{tab}/github/ask`), `crates/core/src/server/changes.rs` (`agent_ready` and `type_in` become `pub(super)`), `crates/core/src/server/mod.rs`
- Test: `github/ask.rs` tests, `tests/github_api.rs`

**Interfaces (Produces):** `pub fn ask_text(ask: &AskRequest, ctx: &AskContext) -> Result<String, &'static str>` (the `Err` is a refusal's sentence) with `pub struct AskContext { pub repo: String /* owner/name, or host/owner/name off github.com */, pub branch: String, pub base: String, pub default_branch: String, pub plain_shell: bool }`.

The text is one line, the same for all five CLIs, typed without Enter through `type_in` after `agent_ready` (so the 409 reasons are `/review`'s: `ended`, `elevated`, `paused`, `waiting`, `not_ready`, `no_agent`). `copy: true` types nothing and returns the text with `form: Copy`.

| Request | Text |
| --- | --- |
| Fix a check that has a run id | ``The check "{name}" ({workflow}) failed on pull request #{n}: {url}. Read its log with `gh run view {run_id} --repo {repo} --log-failed`, find the cause and fix it.`` (with no workflow, ` ({workflow})` is left out) |
| Fix a check with no run id | `The check "{name}" failed on pull request #{n}. Its details are at {url}. Find the cause and fix it.` (with no link, the middle sentence is left out) |
| Open the pull request | `Open a pull request on GitHub for the branch {branch} into {base}: push the branch if it isn't pushed yet, then create the pull request with a title and a description that sum up its commits.` |
| Switch and pull | ``Pull request #{n} is merged. Switch this checkout to {default} and pull it: run `git switch {default}`, then `git pull`. If there are uncommitted changes, ask me first.`` |
| Switch and pull, when `{default}` isn't `command_safe` | `Pull request #{n} is merged. Switch this checkout to its default branch and pull it. If there are uncommitted changes, ask me first.` |
| Switch and pull, copied for a plain shell | `git switch {default}` and `git pull`, on two lines; refused with 409 "The default branch's name can't be put in a command safely." when `{default}` isn't `command_safe` |

What goes into a text:
- `{branch}`, `{base}`, `{default}` and `{repo}` are never taken from the request: the core reads them from the checkout and from its own last answer from GitHub (no answer yet → 409 "Marshell hasn't heard from GitHub about this branch yet."). The request carries only the check's name, workflow, link and run id, and the pull request's number, which must be `pull_seen`'s.
- A name in prose (`{name}`, `{workflow}`, `{branch}`, `{base}`) goes through `plain(_, 200)`, and its backticks and double quotes become spaces. So no text holds a line break, a hidden or reordering character, or a quote that closes early.
- A name inside a command (`{default}` in `git switch`, `{repo}` in `--repo`) must be `command_safe`; `{repo}`'s parts already are, by `parse_remote`. The default branch's name is the repository owner's to choose and git allows `;`, `$`, `|` and `&` in it, so one that isn't `command_safe` never reaches a command.
- A link is used only when it starts with `https://{host}/` and holds no space, quote, backtick or control character; else it is left out.
- `{run_id}` and `{n}` are numbers.

- [ ] **Step 1: Write the tests:** `fix_text_names_the_check_link_and_log_command`, `a_check_with_no_run_id_has_no_log_command`, `control_and_format_characters_in_a_name_become_spaces` (a name with `\n`, U+202E and U+2028), `a_backtick_in_a_name_cannot_close_the_command`, `a_shell_character_in_the_default_branch_drops_the_command` (default `main;calc`: the agent's text holds no `git switch` and no `;calc`, and the plain-shell copy is refused), `the_branch_names_come_from_the_core` (a request can't supply them: the type has no such field), `a_link_to_another_host_is_left_out`, `an_enterprise_repo_is_named_with_its_host`, and in `tests/github_api.rs` `ask_types_the_same_bytes_for_all_five_agents_with_no_enter` (no `\r` or `\n` in what each session received), `ask_copy_types_nothing`, `ask_waits_while_the_agent_asks` (409 `waiting`), `ask_in_a_session_with_no_agent_is_refused_unless_copy`, `ask_needs_the_token`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 --lib github::ask` and `--test github_api`. Expected: fail; implement; `0 failed`.
- [ ] **Step 3:** Commit: `feat(changes): a failed check, the pull request and the switch back to main can be asked of the agent.`

### Task 12: The bar's model, copy and polling (frontend, pure)

**Files:**
- Create: `src/features/changes/githubApi.ts`, `useGithub.ts`, `pullModel.ts`, `githubCopy.ts`, and a `.test.ts` for each of the last three

**Interfaces (Produces):**

```ts
// githubApi.ts: readGithub(tab, fresh), readPlan(tab, create), startPush(tab, req), readPush(id, from), cancelPush(id),
//               stopPushes(), createPull(tab, req), mergePull(tab, req), pullAction(tab, req), ask(tab, req); a failure is { reason } as changesApi's.
//               Every action's request carries `checkout` from the reply or the plan it was drawn from.
// useGithub.ts
export function githubSchedule(s: { inFlight: boolean; shown: boolean; focused: boolean; on: boolean; lastAsk: number; nextMs: number; now: number; headMoved: boolean; asked: boolean }): "none" | "ask" | "local";
export function useGithub(tab: string | null, shown: boolean, head: string | null, on: boolean): { reply: GithubReply | null; refresh: () => void };
// pullModel.ts
export type BarView = { line: string; second: string | null; notes: string[]; main: BarButton | null; menu: BarMenuItem[]; checks: ChecksView | null; link: { label: string; url: string } | null; hide: boolean };
export function barOf(reply: GithubReply | null, agent: string | null): BarView | null;   // null: no bar
export function checksOf(pull: PullStatus): ChecksView;      // { summary, listed: CiCheck[], folded: CiCheck[], foldedLabel, more }
export function announcementsOf(before: GithubReply | null, after: GithubReply): string[];
```

- `githubSchedule`: "none" while hidden, unfocused, off or in flight; "ask" when nothing was asked yet, on `refresh`, or `now - lastAsk >= nextMs`; "local" (a `fresh: false` ask) when `head` moved since the last answer; else "none". `refresh` and every finished action ask at once.
- The summary reads passed, failed, cancelled, running, waiting, skipped, leaving out a zero: "Checks: 5 passed, 1 failed". A neutral check counts as passed. No checks: "No checks".
- Order of the main button, first match: a push runs → "Show output"; commits to push → "Push N commits"; a draft → "Mark as ready for review"; auto-merge on → none; `can_auto_merge` while a check runs or waits, or while the pull request is Blocked → "Merge when checks pass"; else "Merge" (off with its reason when it can't).

The bar, by case (exact copy; `{agent}` is the session's agent name as "Ask {agent} to review" has it):

| Case | Line, second line and notes | Main button | ⋯ menu and other controls |
| --- | --- | --- | --- |
| `off`, `none` | no bar | | |
| `no_gh` | "Pull requests need the GitHub CLI." | "Get it" (opens `https://cli.github.com`) | "Hide" |
| `signed_out` | "Pull requests need a GitHub sign-in. Run `gh auth login` in a terminal." | "Copy" (copies `gh auth login`, says "Copied the command.") | "Hide" |
| `unreachable` | "Can't reach GitHub: {reason}" | "Try again" | |
| `not_found` | "GitHub has no repository {repo} that this sign-in can see." | | |
| `rate_limited` | "GitHub's limit for this sign-in is used up. Marshell asks again at {time}." | | |
| `default_branch` | "{branch} is the default branch on GitHub, so it has no pull request." | | |
| ready, no pull request | "No pull request · 3 commits to push" ("No pull request · Nothing to push" at 0; "No pull request · #182 was closed · 3 commits to push" with `closed`) | "Create pull request…" (off at 0 commits ahead of the base, reason "Nothing differs from {base}.") | "Ask {agent} to open the pull request" |
| GitHub ahead | adds the note "GitHub's {branch} has commits this checkout doesn't. Pull them in the terminal." | Push and Create off with that reason | |
| `push_blocked` | adds the note "origin pushes somewhere other than {repo}, so push from the terminal." | Push and Create off with that reason; Merge and the rest work | |
| draft | "#182 Draft · Checks: 2 passed" | "Mark as ready for review" | |
| open | "#182 Open · Checks: 5 passed, 1 failed"; second line "Review required", "Changes requested" or "Approved" | "Merge" with the default method's label and a ▾ of the others | "Convert to draft" |
| open or draft, commits to push | "#182 Open · 2 commits to push · Checks: …" | "Push 2 commits" | Merge is off: "2 commits aren't on GitHub yet. Push them first." |
| checks run or the merge is blocked, `can_auto_merge` | | "Merge when checks pass" | |
| auto-merge on | "#182 Open · Merges when checks pass" | none | "Cancel auto-merge" |
| not ready | Merge off with: Checking "GitHub is still checking whether #182 can merge."; Conflicts "#182 has conflicts with {base}. Resolve them in the terminal."; Blocked "GitHub won't merge #182 yet: a required check or review is missing."; Behind "{branch} must be brought up to date with {base} first."; no write access "Your GitHub account can't merge in {repo}." | | |
| merged | "#182 Merged" with its link | none | "Ask {agent} to switch to {default} and pull"; in a session with no agent "Copy the commands to switch to {default} and pull" |
| a fork | adds the note "A fork of {parent}" | | link "Open a pull request to {parent} on GitHub" |
| a push runs (`push`) | "Pushing {branch}…" | "Show output" | |

Every bar with a pull request has "Open on GitHub" and "Copy link" in the ⋯ menu. Method labels: "Create a merge commit", "Squash and merge", "Rebase and merge"; the Merge button reads "Merge", "Squash and merge" or "Rebase and merge".

Announcements, once per change, through `announce`: "Checks passed on #182.", "A check failed on #182: {name}.", "#182 was merged.", "Pull request #182 was created."

- [ ] **Step 1: Write the tests:** `it("reads '#182 Open · Checks: 5 passed, 1 failed'")`, `it("reads 'No pull request · 3 commits to push' and '1 commit to push'")`, `it("puts Push before Merge while commits wait")`, `it("turns Merge off with the reason for each readiness")`, `it("offers Merge when checks pass while checks run, and when the merge is blocked")`, `it("turns Push and Create off when origin pushes elsewhere, and keeps Merge")`, `it("counts cancelled checks, and neutral ones as passed")`, `it("an unreachable reply shows the line and no buttons but Try again")`, `it("shows no bar when off or not GitHub")`, `it("folds passed and skipped checks behind '5 passed, 1 skipped'")`, `it("asks every 30 s, and every 10 s while checks run")`, `it("asks nothing while hidden, unfocused or off")`, `it("recounts locally when the head moved")`, `it("announces a change of checks once")`.
- [ ] **Step 2:** Run `pnpm test src/features/changes/pullModel.test.ts src/features/changes/useGithub.test.ts src/features/changes/githubCopy.test.ts`. Expected: fail; implement; pass; `pnpm build` passes.
- [ ] **Step 3:** Commit: `feat(changes): the pull request bar's model, its copy and when it asks GitHub.`

### Task 13: The bar and the checks list in the panel

**Files:**
- Create: `src/features/changes/PullBar.tsx`, `ChecksList.tsx`, `PullBar.css`, `PullBar.test.tsx`, `PullBar.css.test.ts`
- Modify: `src/features/changes/ChangesPanel.tsx` (the bar between `changes__strip` and `DiffList`, and in the empty-diff branch too; `useGithub`; the notices), `ChangesPanel.css`

**Interfaces:**
- Consumes: `barOf`, `checksOf`, `useGithub`, `ask`, `openUrl` from `@tauri-apps/plugin-opener` (as `LiveApp.tsx` uses it; only an `https:` link is opened).
- Produces: `<PullBar view={…} onMain={…} onMenu={…} onEscape={…} />`; `ChangesHandle` gains `createPull()`, `push()`, `merge()`, `openPull()` (wired in Task 15; they open the sheets of Task 14).

Layout and keyboard:
- The bar is a `<section aria-label="Pull request">` under the branch line. It is also drawn when the diff is empty (the panel's early return for no sections), since a merged pull request leaves no diff.
- Its controls are ordinary Tab stops, before the diff's one Tab stop: the "Checks: …" disclosure (`aria-expanded`), the main button, the method ▾ (a `ContextMenu`), the fork link.
- `ChecksList` is one Tab stop with a roving focus (Up, Down, Home, End). Enter opens the check on GitHub. Shift+F10, the Menu key or a right-click opens a `ContextMenu`: "Open on GitHub", "Ask {agent} to fix" (a failed or cancelled check only; "Copy a request to fix" in a session with no agent), "Copy link". A failed row also shows "Ask {agent} to fix" as a mouse button (`tabIndex={-1}`).
- A row reads, for a screen reader, "Failed: clippy, ci, 48 s" / "Running: test, ci, 2 min" / "Passed: build, ci, 1 min 12 s"; its glyph is `aria-hidden`.
- The folded checks sit behind a disclosure row "5 passed, 1 skipped"; with `more_checks` the list ends "And {n} more on GitHub."
- Esc in the bar or the list calls `onEscape`, as the diff does.
- In forced colours the bar draws a `CanvasText` edge and the glyphs use the text colour.
- "Hide" turns `pull_requests` off through the Changes settings and says "Pull requests are hidden. Settings → Changes shows them again."

- [ ] **Step 1: Write the tests** (`renderToStaticMarkup`): `it("draws the bar between the branch line and the diff")`, `it("keeps the bar over an empty diff")`, `it("shows the gh line with Get it and Hide")`, `it("lists failed checks first and folds passed ones")`, `it("gives a failed check 'Ask Claude to fix' and a passed one none")`, `it("names each check's state in words")`, `it("draws an edge in forced colours")` (the CSS test reads `PullBar.css`).
- [ ] **Step 2:** Run `pnpm test src/features/changes src/features/browser/coverScan.test.ts`. Expected: fail; implement; pass; `pnpm build` passes.
- [ ] **Step 3:** Commit: `feat(changes): the pull request bar and its checks list show in the Changes panel.`

### Task 14: The sheets: create, push and merge

**Files:**
- Create: `src/features/changes/CreatePullSheet.tsx`, `PushSheet.tsx`, `MergeDialog.tsx`, `pushRun.ts`, `PullSheets.css`, and a test file for each
- Modify: `src/styles/dialogBodies.test.ts` (lists the new sheets), `src/styles/noScrollInMainWindow.test.ts` (lists every scrolling selector: add the sheets' body with its reason), `src/styles/overlayStacking.test.ts` and `src/features/window/dialogStacking.css.test.ts` if they list dialogs by name, `src/features/changes/ChangesPanel.tsx` (opens them)

**Interfaces:**
- Consumes: `readPlan`, `startPush`, `readPush`, `cancelPush`, `createPull`, `mergePull`.
- Produces: `pushRun.ts`: `export function pushStep(state: RunState, event: RunEvent): RunState` (the pure machine: `confirm → running → pushed | failed | cancelled`, `failed → running` on Try again, and for Create `pushed → creating → created | create_failed`), and `linesOf(prev: string[], start: number, progress: PushProgress): string[]`.

The sheets follow `ClearDataDialog.tsx` (the `close-dialog` classes, `useDialogFocus`, `useCoverPage()`). The body is the sheet's one scroller (`dialogBodies.test.ts` allows one per stylesheet): the commit list and git's output wrap and grow inside it, and neither scrolls on its own.

- **Create pull request** (title "Create pull request"):
  - "{folder}" (the checkout's full path, as the Push sheet shows it: decisions 2 and 12 have every confirm name the folder), then the head line "{branch} → {base} · {repo}".
  - "3 commits will be pushed to origin." (left out at 0), the commit list (short id and subject; "And {n} more." past 100), "2 files aren't committed and stay out." when `uncommitted > 0`, and when `moved`: "This isn't the folder the session started in. A push runs this repository's hooks and settings."
  - Fields "Title", "Description" (a textarea), "Base branch"; the checkbox "Create as draft".
  - Buttons "Cancel" and "Push and create" ("Create pull request" when nothing is to push). The button is off with an empty title. A `refusal` replaces the form with its sentence and "Close".
- **Push** (title "Push to GitHub"): "{folder}", "{branch} → origin", the commit list, the `moved` warning; buttons "Cancel" and "Push 2 commits".
- **The running view**, in either sheet: "Pushing {branch} to origin…", git's output in a `<pre>` (polled every `PUSH_POLL_MS`), and "Cancel".
  - The output is not a live region: git redraws its progress line many times a second. While it runs the body has `tabIndex={0}` and `aria-label="git's output"`, so the keyboard can scroll it; it follows the last line unless the user scrolled up.
  - Each end is said once through `announce` and focus moves to the sentence or the next button: Cancel → "The push was stopped."; failure → the reason, with the output kept, "Close" and "Try again" (focus on the reason, `tabIndex={-1}`); success in the Push sheet → it closes and says "Pushed 2 commits to origin." (with the push's `reason` after it, when git pushed somewhere unexpected).
  - The Create sheet goes on to create; a create failure keeps the form (the branch is pushed) with GitHub's sentence, announced, and the button "Create pull request".
- Esc and the sheet's close button are off while a push runs; Cancel is the way out.
- A sheet opened by "Show output" for a push that was already running (after a reload) shows the running view only.
- **Merge** (title "Merge #182?"): "{Method button} "{title}" (#182) into {base}." (the button's words: "Merge", "Squash and merge", "Rebase and merge"; the menu label "Create a merge commit" is no sentence) and, for auto-merge, "GitHub will merge it once its checks pass."; buttons "Cancel" and the method's label ("Merge when checks pass" for auto-merge). A failure shows GitHub's sentence in the dialog.

- [ ] **Step 1: Write the tests:** `pushRun.test.ts`: `it("goes from confirm to running to pushed")`, `it("a create failure after a push keeps the form and skips the push")`, `it("Try again starts a new push")`, `it("replaces the redrawn last line and appends the rest")`; `CreatePullSheet.test.tsx`: `it("names the folder, the branch, the base and the repository")`, `it("warns about uncommitted files and a moved folder")`, `it("reads 'Create pull request' when nothing is to push")`, `it("shows a refusal with Close only")`; `PushSheet.test.tsx`: `it("names the folder, the branch and the commits")`, `it("shows git's output and Cancel while running")`, `it("keeps the output with Try again after a failure")`, `it("lets the keyboard scroll the output while a push runs")`; `MergeDialog.test.tsx`: `it("names the pull request, the method and the base")`, `it("says GitHub merges once checks pass for auto-merge")`.
- [ ] **Step 2:** Run `pnpm test src/features/changes src/styles/dialogBodies.test.ts src/features/browser/coverScan.test.ts`. Expected: fail; implement; pass; `pnpm build` passes.
- [ ] **Step 3:** Commit: `feat(changes): sheets to create a pull request, push and merge.`

### Task 15: Menu, palette, Settings and the asks

**Files:**
- Modify: `src/features/changes/changesMenu.ts` (+ test), `src/lib/keymap.ts`, `src/lib/appShortcuts.ts` (+ tests), `src/features/changes/ChangesSection.tsx` (+ test), `src/features/sessions/LiveApp.tsx` (dispatch to `ChangesHandle`), `src/features/changes/send.ts` (a shared refusal notice for asks), `src/features/changes/ChangesPanel.tsx`

**Interfaces:** `changesMenuItems` gains `github: BarMenuItem[]` (from `barOf`), listed after "Ask {agent} to review" under a separator; `changesChoice` parses their ids.

- Palette commands (group as "Refresh changes"; no default key): "Create pull request…" (`changes-create-pull`), "Push commits" (`changes-push`), "Merge pull request…" (`changes-merge`), "Open the pull request on GitHub" (`changes-open-pull`). A command that can't run now says why through `announce` (the bar's own reason, or "This branch has no pull request.").
- "Refresh" and "Refresh changes" also ask GitHub (`refresh()` of `useGithub`).
- Settings → Changes: "Show pull requests from GitHub", help line "Asks GitHub through the GitHub CLI while Changes shows a checkout whose origin is on GitHub. Off, Marshell never runs gh."
- The asks: a session with an agent → `ask(tab, {…, copy: false})`, then "Typed a request into {session}." and the keyboard moves to that terminal, as the review action does; a refusal uses `/review`'s notices ("{name} runs elevated, so nothing was typed." and the rest), and a 409 with a sentence of its own shows that sentence.
- A session with no agent → `copy: true`, and the text goes to the clipboard: "Copied the request. {session} has no prompt line." for a check, "Copied the two commands." for switch and pull. The rows read "Copy a request to fix" and "Copy the commands to switch to {default} and pull" there, and "Ask {agent} to open the pull request" is left out.

- [ ] **Step 1: Write the tests:** `it("adds the bar's rows under the review row")`, `it("leaves out the open-pull-request row with no agent")`, `it("maps the four palette commands")`, `it("shows the GitHub switch with its help line")`.
- [ ] **Step 2:** Run `pnpm test src/features/changes src/lib src/features/appearance`, then `pnpm build`. Expected: pass.
- [ ] **Step 3:** Commit: `feat(changes): the pull request's commands reach the menu, the palette and Settings.`

### Task 16: Quitting while a push runs (asked 2026-11-23: "Ask, then stop it")

**Files:**
- Modify: `crates/core/src/supervisor.rs` (the handle the shell holds gains `pub fn pushes_running(&self) -> u32`, 0 while no core runs), `src-tauri/src/main.rs` (`quit`), `src/features/sessions/LiveApp.tsx` (listens and shows the dialog)
- Create: `src/features/window/QuitDialog.tsx`, `QuitDialog.test.tsx`
- Modify: the tests that list dialogs (`src/styles/overlayStacking.test.ts`, `src/features/window/dialogStacking.css.test.ts`, `src/features/browser/coverScan.test.ts` if it lists by name)
- Not touched: `src-tauri/capabilities/`, `tauri.conf.json`, `build.rs`. No new Tauri command: the dialog's Quit uses the core's route and the existing `close_once` command.

**Interfaces:**
- Consumes: `GithubState::pushes_running`, `POST /v1/pushes/stop` (`stopPushes()`), the `close_once` command.
- Produces: in `main.rs`, `fn quit_step(pushes_running: u32) -> QuitStep { Ask(u32), Go }` (pure, tested), and the event `quit-ask` with the count as its payload, sent as `close-choice` is.

How it runs:
- `quit(app)` (the tray's Quit, the close button when it quits, `close_once`) first reads `pushes_running()`. At 0 it quits as today. Otherwise it shows the main window (`tray::show_main`, as the close question does) and emits `quit-ask`; if the emit fails it quits.
- The webview shows `QuitDialog`: "A push is still running. Quit and stop it?" ("2 pushes are still running. Quit and stop them?"), buttons "Cancel" and "Quit". It is a `close-dialog` sheet with `useDialogFocus` and `useCoverPage()`; focus starts on "Cancel"; Esc cancels.
- "Quit" calls `stopPushes()` (the core ends every push's process tree and answers within 2 s), then `invoke("close_once", { keepInTray: false })`. No push runs any more, so `quit` goes through. A failure reads "Couldn't stop the push. Try again." in the dialog.
- An exit that doesn't come through `quit` (the system shutting down) reaches the core's `shutdown`, which stops the pushes (Task 9).

- [ ] **Step 1: Write the tests:** in `main.rs` `quit_asks_while_a_push_runs` and `quit_goes_with_no_push`; `QuitDialog.test.tsx`: `it("asks about one push and about several")`, `it("offers Cancel first")`; the dialog lists.
- [ ] **Step 2:** Run `cargo test -p marshell-app -j 4`, `cargo test -p marshell-core -j 4 --lib supervisor::`, `pnpm test src/features/window src/styles src/features/browser/coverScan.test.ts`, `pnpm build`. Expected: fail first, then pass.
- [ ] **Step 3:** Commit: `feat(changes): quitting while a push runs asks first, then stops it.`

### Task 17: The live check (main session)

Run with the `marshell-live-check` skill against a scratch home, with the real `gh` sign-in and the scratch repository `Doofess/marshell-scratch-6r4` cloned to a fresh scratch folder. GitHub Actions don't start there (S17 finding 11), so running and passing checks are made with the commit status API, and a running `CheckRun` is read from this slice's own pull request. Findings go in `docs/research/2026-11-2x-github-live-check.md`.

Rows:
1. A branch with commits and no pull request: the bar reads "No pull request · N commits to push"; the Create sheet shows the commits, the template or subject list, and the uncommitted warning.
2. "Push and create" with a `pre-push` hook that prints and sleeps: the output arrives while it runs; the pull request exists on GitHub; the bar reads "#N Open".
3. Cancel during the hook: the hook's process is gone and GitHub's branch didn't move.
4. A pending status, then success, then failure: the bar and the list follow within 10 s, each change is announced once, the failed row offers "Ask {agent} to fix", and the request is typed without Enter into a session started for the check (one per CLI that is installed; the others by the route test).
5. A later commit: "1 commit to push", the Push confirm, the push, and Merge is off until it is done.
6. Draft → "Mark as ready for review" → "Convert to draft" → ready.
7. Merge with each method on three pull requests; a conflicting one shows GitHub's sentence; after a merge the bar reads "#N Merged" and "Ask {agent} to switch to main and pull" types its request.
8. A commit pushed to the branch from outside: "GitHub's {branch} has commits this checkout doesn't…", no Push.
9. A pull request against a second base branch (`develop`): the diff's base and the branch line follow; unfetched, the note shows.
10. gh off the PATH, and `GH_CONFIG_DIR` at an empty folder: the two lines, "Get it", "Copy", "Hide" and the Settings switch.
11. The switch off: no gh process starts (watched for a minute).
12. Keyboard alone through the bar, the list, the three sheets; forced colours; the bar over an empty diff; a page in the Browser panel is covered by each sheet.
13. A session whose shell moved into the scratch clone: the `moved` warning in both confirms.
14. This slice's own pull request, read-only: its running checks show as Running and fold when they pass.
15. `origin` with a `pushurl` to a second scratch folder, then two `url` values: the bar keeps the pull request, Push and Create are off with "origin pushes somewhere other than …", and nothing is pushed.
16. A branch named `+main` and one named `-x` in the scratch clone: no push is offered, and GitHub's `main` didn't move.
17. Quit from the tray while the hook sleeps: the question shows; Cancel keeps the push running; Quit ends the hook's process and Marshell, and GitHub's branch didn't move.
18. The session's shell moves to another clone while the Create sheet is open: "Push and create" is refused with "The session moved to another folder…".
19. `origin` over SSH (`git@github.com:…`), if the dev PC has a key for it: the push runs with no terminal; what a passphrase or host-key question does is written down (the sheet's Cancel must end it).
20. A sign-in window from Git Credential Manager, if one can be made to appear (a scratch credential store): it survives the push's end.

- [ ] **Step 1:** Run the rows, fix what they find (each fix with its test), and write the note.
- [ ] **Step 2:** Commit: `docs(changes): slice 6r-4a's live check.`

---

## Asked after the plan review (PLAN §9, decisions 20 to 22)

- **20. Marshell pushes only to the repository the bar shows.** It reads origin's push address too; Push and Create work only when origin has one address and it is the bar's repository. Otherwise the bar still shows the pull request and its checks, and says "origin pushes somewhere other than owner/repo, so push from the terminal." SSH host aliases get their own issue.
- **21. Quitting while a push runs asks first:** "A push is still running. Quit and stop it?" Quit ends the push's whole process tree. A push that ends on its own leaves alone what it started.
- **22. The pull request's base is kept in memory only.** After a restart the diff opens against the default branch and moves to the pull request's base when GitHub first answers; offline it stays on the default branch.

## Decided, not asked (PLAN §9, decision 23)

The plan settles these on what the code read and S17 found; the user can change any of them.

1. The status has its own route and its own poll; the diff's 2-second poll is untouched, and a slow or failed GitHub ask never holds the diff.
2. Marshell uses only `gh api` and `gh auth status`. gh must be a `gh.exe` in an absolute folder of the PATH a session gets; it runs in `<home>/gh` with a short list of environment variables, 15 s and 4 MiB at most.
3. `origin` is the only remote looked at, and the bar's repository is its fetch address with `insteadOf` applied. An SSH host alias (`github-work:owner/repo`) isn't resolved, so such a checkout shows no bar; that gets its own issue. A setting that only an included config file holds isn't seen: the push runs with every transport but https and ssh switched off, and says so afterwards if git pushed to another address than the bar's.
4. A host other than github.com counts only while gh is signed in to it; `gh auth status` runs only for such hosts, at most every 10 minutes.
5. A pull request counts only when its branch is in origin's own repository. An open or draft one comes first; else the newest merged or closed one, and only if its last commit is in the local branch, so a reused branch name doesn't read "Merged". A closed one reads "No pull request · #182 was closed".
6. On GitHub's default branch the bar is one quiet line, and Marshell doesn't push that branch. A detached HEAD shows no bar.
7. With commits still to push, Push is the main button and Merge is off until they are pushed.
8. The checks summary counts passed, failed, running, waiting and skipped; the list puts failed, cancelled, running and waiting first; past 100 checks it says "And N more on GitHub."
9. The bar's controls are Tab stops before the diff; the checks list is one Tab stop with arrow keys and a row menu.
10. "Cancel auto-merge", "Convert to draft", "Open on GitHub" and "Copy link" are in the ⋯ menu. Four palette commands, none with a default key.
11. An ask while GitHub is unreachable or signed out shows one line and no actions; a pull request seen earlier is not shown as current.
12. When GitHub's limit is used up, Marshell waits until the reset, and at least a minute.
13. A second ask within 3 seconds answers from the last one. A moved `HEAD` recounts the commits to push without asking GitHub.
14. No session state stops a push: it types nothing into the session. One push per checkout at a time. Its output keeps the last 2,000 lines. A push that outlives its sheet (the window reloaded) keeps running, and the bar finds it again.
15. The push uses the git a session would find, with Marshell's own environment (not the session shell's: a proxy or an SSH agent set only in a shell profile isn't there) and `GIT_TERMINAL_PROMPT=0`. A Git Credential Manager window can still appear, and what the push started is left running when it ends on its own. An SSH question (a passphrase, a new host key) has no terminal to ask in; Cancel ends it.
16. A branch whose name starts with `-` or `+` is never pushed, and every push names the full ref on both sides. A branch name from GitHub that Marshell can't use safely makes the bar say so, and a default branch name with shell characters is never written into a command.
17. Every action names the folder it was confirmed for and is refused if the session has moved; merge, ready, draft and auto-merge act only on the pull request Marshell itself last read for that branch.
18. A commit made in the instant between the click and git reading the branch is pushed with the rest; the bar shows the true state straight after.
19. With several commits the title starts as the branch name, unchanged. The template is read from the work tree (`.github/`, the root, `docs/`), not from GitHub; a folder of several templates isn't used. "Base branch" is a text field.
20. With nothing to push, Create's button reads "Create pull request" and no push runs. If the push works and the create fails, the form stays with GitHub's sentence.
21. A pull request's base comes before a task's base branch for the diff.
22. The three requests are written in the core, one line each, the same for every CLI, and the branch names in them come from the core, never from the webview. The fix request names the repository (`--repo`) so the agent's gh finds it. A session with no agent gets the text copied; "Ask … to open the pull request" is left out there. For a plain shell, switch-and-pull is copied as two commands.
23. A fork's link opens GitHub's compare page for the parent, against the parent's default branch.
24. Quitting asks only when it goes through Marshell's own quit; when the system shuts down, a running push is stopped without a question.
25. Not verified live (S17): an accepted auto-merge, a merge held by a required check or review, a GitHub Enterprise host, a credential prompt during the push.
