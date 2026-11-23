//! One GraphQL answer from GitHub (`query::STATUS_QUERY`) as the status the pull request bar shows. Pure: no
//! process, no network.

use marshell_protocol::api::{
    CheckKind, CheckState, CiCheck, MergeMethod, PullState, PullStatus, Readiness, ReviewNeed,
};
use serde_json::Value;

use super::gh::GhError;
use super::query::{Answer, Context, Pull, Repository};
use super::{plain, valid_branch};
use crate::agents::common::rfc3339_ms;
use crate::gitbox::refs::Oid;

/// When to ask again while the first open pull request is settled.
pub const NEXT_IDLE_MS: u64 = 30_000;
/// When to ask again while it has a check running or waiting, an auto-merge, or a merge state GitHub is still working out.
pub const NEXT_BUSY_MS: u64 = 10_000;

/// The longest text from GitHub that is kept.
const TEXT_MAX: usize = 200;

/// What GitHub said about a repository and one branch of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub default_branch: Option<String>,
    /// The upstream repository (`owner/name`) when this one is a fork.
    pub fork_of: Option<String>,
    /// The upstream repository's default branch.
    pub parent_default: Option<String>,
    /// The commit GitHub has at the branch.
    pub branch_oid: Option<String>,
    /// The same-repository pull requests from the branch, newest first.
    pub pulls: Vec<Candidate>,
    pub can_write: bool,
    pub merge_methods: Vec<MergeMethod>,
    pub auto_merge_allowed: bool,
    /// When to ask again.
    pub next_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// `head` is GitHub's head commit of the pull request, the one a merge names.
    pub status: PullStatus,
}

fn refused_branch() -> GhError {
    GhError::Refused {
        status: None,
        message: "GitHub named a branch Marshell can't use.".into(),
    }
}

/// A branch name from GitHub: checked as it came, then made plain.
fn branch_of(raw: &str) -> Result<String, GhError> {
    if valid_branch(raw) {
        Ok(plain(raw, TEXT_MAX))
    } else {
        Err(refused_branch())
    }
}

/// A link kept only when it is `https://` and holds no control character and no space.
fn link_of(url: Option<&str>) -> Option<String> {
    let url = url?;
    (url.starts_with("https://") && !url.chars().any(|c| c.is_control() || c == ' ')).then(|| url.to_string())
}

fn merge_method_of(text: &str) -> Option<MergeMethod> {
    match text {
        "MERGE" => Some(MergeMethod::Merge),
        "SQUASH" => Some(MergeMethod::Squash),
        "REBASE" => Some(MergeMethod::Rebase),
        _ => None,
    }
}

fn run_state(status: Option<&str>, conclusion: Option<&str>) -> CheckState {
    match status {
        Some("QUEUED" | "WAITING" | "PENDING" | "REQUESTED") => CheckState::Waiting,
        Some("IN_PROGRESS") => CheckState::Running,
        Some("COMPLETED") => match conclusion {
            Some("SUCCESS") => CheckState::Passed,
            Some("FAILURE" | "TIMED_OUT" | "STARTUP_FAILURE" | "ACTION_REQUIRED") => CheckState::Failed,
            Some("CANCELLED") => CheckState::Cancelled,
            Some("SKIPPED" | "STALE") => CheckState::Skipped,
            _ => CheckState::Neutral,
        },
        _ => CheckState::Neutral,
    }
}

fn context_state(state: Option<&str>) -> CheckState {
    match state {
        Some("PENDING" | "EXPECTED") => CheckState::Running,
        Some("SUCCESS") => CheckState::Passed,
        Some("FAILURE" | "ERROR") => CheckState::Failed,
        _ => CheckState::Neutral,
    }
}

/// A check of the rollup, or `None` for a node of a kind this build doesn't know (or one without a name).
fn check_of(node: &Context) -> Option<CiCheck> {
    match node.typename.as_deref() {
        Some("CheckRun") => {
            let run = node.check_suite.as_ref().and_then(|s| s.workflow_run.as_ref());
            Some(CiCheck {
                kind: CheckKind::Run,
                state: run_state(node.status.as_deref(), node.conclusion.as_deref()),
                name: plain(node.name.as_deref()?, TEXT_MAX),
                workflow: run
                    .and_then(|r| r.workflow.as_ref())
                    .and_then(|w| w.name.as_deref())
                    .map(|n| plain(n, TEXT_MAX)),
                url: link_of(node.details_url.as_deref()),
                run_id: run.and_then(|r| r.database_id),
                started_ms: node.started_at.as_deref().and_then(rfc3339_ms),
                completed_ms: node.completed_at.as_deref().and_then(rfc3339_ms),
            })
        }
        Some("StatusContext") => Some(CiCheck {
            kind: CheckKind::Status,
            state: context_state(node.state.as_deref()),
            name: plain(node.context.as_deref()?, TEXT_MAX),
            workflow: None,
            url: link_of(node.target_url.as_deref()),
            run_id: None,
            started_ms: node.created_at.as_deref().and_then(rfc3339_ms),
            completed_ms: None,
        }),
        _ => None,
    }
}

/// The checks of the pull request's last commit, failures first (then cancelled, running, waiting, passed,
/// neutral, skipped; each group in GitHub's order), and how many more GitHub holds than it sent.
fn checks_of(pull: &Pull) -> (Vec<CiCheck>, u32) {
    let contexts = pull
        .commits
        .as_ref()
        .and_then(|c| c.nodes.iter().flatten().next())
        .and_then(|n| n.commit.as_ref())
        .and_then(|c| c.status_check_rollup.as_ref())
        .and_then(|r| r.contexts.as_ref());
    let Some(contexts) = contexts else {
        return (Vec::new(), 0);
    };
    let mut checks: Vec<CiCheck> = contexts.nodes.iter().flatten().filter_map(check_of).collect();
    // `sort_by_key` is stable, so each group stays in GitHub's order.
    checks.sort_by_key(|c| match c.state {
        CheckState::Failed => 0,
        CheckState::Cancelled => 1,
        CheckState::Running => 2,
        CheckState::Waiting => 3,
        CheckState::Passed => 4,
        CheckState::Neutral => 5,
        CheckState::Skipped => 6,
    });
    let more = contexts.total_count.saturating_sub(contexts.nodes.len() as u64);
    (checks, u32::try_from(more).unwrap_or(u32::MAX))
}

/// The first match of: conflicts, still working it out, blocked, behind, else ready (a draft's `DRAFT` too:
/// that a pull request is a draft is read from its `state`).
fn readiness_of(mergeable: Option<&str>, merge_state: Option<&str>) -> Readiness {
    let either = |word: &str| mergeable == Some(word) || merge_state == Some(word);
    if mergeable == Some("CONFLICTING") || merge_state == Some("DIRTY") {
        Readiness::Conflicts
    } else if either("UNKNOWN") {
        Readiness::Checking
    } else if merge_state == Some("BLOCKED") {
        Readiness::Blocked
    } else if merge_state == Some("BEHIND") {
        Readiness::Behind
    } else {
        Readiness::Ready
    }
}

/// The pull request as the bar shows it; `None` when it is left out (another repository's, one of a state this
/// build doesn't know, one whose link isn't a plain `https://` one, or one whose head isn't a commit id).
fn pull_of(node: &Pull) -> Option<Result<PullStatus, GhError>> {
    if node.is_cross_repository {
        return None;
    }
    let state = match (node.state.as_deref()?, node.is_draft) {
        ("OPEN", true) => PullState::Draft,
        ("OPEN", false) => PullState::Open,
        ("MERGED", _) => PullState::Merged,
        ("CLOSED", _) => PullState::Closed,
        _ => return None,
    };
    let url = link_of(node.url.as_deref())?;
    let base = match branch_of(node.base_ref_name.as_deref()?) {
        Ok(base) => base,
        Err(e) => return Some(Err(e)),
    };
    let head = Oid::parse(node.head_ref_oid.as_deref()?)?.as_str().to_string();
    let (checks, more_checks) = checks_of(node);
    Some(Ok(PullStatus {
        id: node.id.clone()?,
        number: node.number?,
        title: plain(node.title.as_deref().unwrap_or_default(), TEXT_MAX),
        url,
        state,
        base,
        head,
        readiness: readiness_of(node.mergeable.as_deref(), node.merge_state_status.as_deref()),
        review: match node.review_decision.as_deref() {
            Some("REVIEW_REQUIRED") => ReviewNeed::Required,
            Some("CHANGES_REQUESTED") => ReviewNeed::ChangesRequested,
            Some("APPROVED") => ReviewNeed::Approved,
            _ => ReviewNeed::None,
        },
        checks,
        more_checks,
        auto_merge: node.auto_merge_request.as_ref().map(|a| {
            a.merge_method
                .as_deref()
                .and_then(merge_method_of)
                .unwrap_or(MergeMethod::Merge)
        }),
        can_auto_merge: node.viewer_can_enable_auto_merge,
    }))
}

fn merge_methods_of(repo: &Repository) -> Vec<MergeMethod> {
    let mut methods: Vec<MergeMethod> = [
        (MergeMethod::Merge, repo.merge_commit_allowed),
        (MergeMethod::Squash, repo.squash_merge_allowed),
        (MergeMethod::Rebase, repo.rebase_merge_allowed),
    ]
    .into_iter()
    .filter_map(|(method, allowed)| allowed.then_some(method))
    .collect();
    let default = repo.viewer_default_merge_method.as_deref().and_then(merge_method_of);
    if let Some(at) = default.and_then(|d| methods.iter().position(|m| *m == d)) {
        let first = methods.remove(at);
        methods.insert(0, first);
    }
    methods
}

/// When to ask again: soon while the first open (or draft) pull request has something running or waiting, an
/// auto-merge, or a merge state GitHub is still working out.
fn next_ms_of(pulls: &[Candidate]) -> u64 {
    let first_open = pulls
        .iter()
        .map(|c| &c.status)
        .find(|p| matches!(p.state, PullState::Open | PullState::Draft));
    let busy = first_open.is_some_and(|p| {
        p.auto_merge.is_some()
            || p.readiness == Readiness::Checking
            || p.checks
                .iter()
                .any(|c| matches!(c.state, CheckState::Running | CheckState::Waiting))
    });
    if busy { NEXT_BUSY_MS } else { NEXT_IDLE_MS }
}

/// GitHub's answer to `STATUS_QUERY` as a `Snapshot`.
///
/// `Err(NotFound)` when the repository is null, `Err(Unreadable)` when the answer holds no data, and
/// `Err(Refused)` when GitHub names a base or default branch Marshell won't hand to git.
pub fn snapshot_of(answer: &Value) -> Result<Snapshot, GhError> {
    let answer: Answer = serde_json::from_value(answer.clone()).map_err(|_| GhError::Unreadable)?;
    let data = answer.data.ok_or(GhError::Unreadable)?;
    let repo = data.repository.ok_or(GhError::NotFound)?;

    let default_branch = match repo.default_branch_ref.as_ref().and_then(|b| b.name.as_deref()) {
        Some(name) => Some(branch_of(name)?),
        None => None,
    };
    let parent = repo.parent.as_ref().filter(|_| repo.is_fork);
    let fork_of = parent
        .and_then(|p| p.name_with_owner.as_deref())
        .map(|n| plain(n, TEXT_MAX));
    let parent_default = parent
        .and_then(|p| p.default_branch_ref.as_ref())
        .and_then(|b| b.name.as_deref())
        .and_then(|name| branch_of(name).ok());
    let branch_oid = repo
        .r#ref
        .as_ref()
        .and_then(|r| r.target.as_ref())
        .and_then(|t| Oid::parse(t.oid.as_deref()?))
        .map(|oid| oid.as_str().to_string());

    let mut pulls = Vec::new();
    for node in repo.pull_requests.iter().flat_map(|p| p.nodes.iter().flatten()) {
        if let Some(status) = pull_of(node) {
            pulls.push(Candidate { status: status? });
        }
    }

    Ok(Snapshot {
        default_branch,
        fork_of,
        parent_default,
        branch_oid,
        can_write: matches!(repo.viewer_permission.as_deref(), Some("ADMIN" | "MAINTAIN" | "WRITE")),
        merge_methods: merge_methods_of(&repo),
        auto_merge_allowed: repo.auto_merge_allowed,
        next_ms: next_ms_of(&pulls),
        pulls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::{CheckKind, CheckState, MergeMethod, PullState, Readiness, ReviewNeed};
    use serde_json::{Value, json};

    const OPEN_FAILING: &str = include_str!("../../tests/fixtures/github/open-failing.json");
    const DRAFT: &str = include_str!("../../tests/fixtures/github/draft.json");
    const MERGED: &str = include_str!("../../tests/fixtures/github/merged.json");
    const NO_PULL: &str = include_str!("../../tests/fixtures/github/no-pull.json");
    const NO_BRANCH: &str = include_str!("../../tests/fixtures/github/no-branch.json");
    const UNKNOWN: &str = include_str!("../../tests/fixtures/github/unknown.json");
    const CROSS_REPO: &str = include_str!("../../tests/fixtures/github/cross-repo.json");
    const NO_ROLLUP: &str = include_str!("../../tests/fixtures/github/no-rollup.json");
    const NOT_FOUND: &str = include_str!("../../tests/fixtures/github/not-found.json");

    fn load(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    fn snap(text: &str) -> Snapshot {
        snapshot_of(&load(text)).unwrap()
    }

    fn repo(v: &mut Value) -> &mut Value {
        &mut v["data"]["repository"]
    }

    fn first_pull(v: &mut Value) -> &mut Value {
        &mut v["data"]["repository"]["pullRequests"]["nodes"][0]
    }

    /// The first pull request of `v` with these check nodes as its rollup.
    fn with_checks(mut v: Value, nodes: Vec<Value>) -> Value {
        let total = nodes.len();
        first_pull(&mut v)["commits"]["nodes"][0]["commit"]["statusCheckRollup"] =
            json!({"contexts": {"totalCount": total, "nodes": nodes}});
        v
    }

    fn run_node(name: &str, status: &str, conclusion: Value) -> Value {
        json!({"__typename": "CheckRun", "name": name, "status": status, "conclusion": conclusion,
            "detailsUrl": "https://github.com/octo-org/widgets/actions/runs/1/job/2",
            "startedAt": "2026-11-23T14:20:03Z", "completedAt": null, "checkSuite": null})
    }

    fn context_node(state: &str) -> Value {
        json!({"__typename": "StatusContext", "context": "ci/status", "state": state,
            "targetUrl": "https://example.com/build/1", "createdAt": "2026-11-23T14:00:00Z"})
    }

    #[test]
    fn picks_same_repository_pull_requests_newest_first() {
        let s = snap(OPEN_FAILING);
        let numbers: Vec<u32> = s.pulls.iter().map(|c| c.status.number).collect();
        assert_eq!(numbers, [7, 3]);
        let p = &s.pulls[0].status;
        assert_eq!(p.id, "PR_kwDOAAAAAA");
        assert_eq!(p.title, "Add the login form");
        assert_eq!(p.url, "https://github.com/octo-org/widgets/pull/7");
        assert_eq!(p.state, PullState::Open);
        assert_eq!(p.base, "main");
        assert_eq!(p.readiness, Readiness::Ready);
        assert_eq!(p.review, ReviewNeed::Required);
        assert!(p.can_auto_merge);
        assert_eq!(p.auto_merge, None);
        assert_eq!(s.pulls[1].status.state, PullState::Closed);
        assert_eq!(s.pulls[1].status.readiness, Readiness::Conflicts);
        assert_eq!(s.pulls[1].status.review, ReviewNeed::None);
        assert_eq!(s.default_branch.as_deref(), Some("main"));
        assert_eq!(
            s.branch_oid.as_deref(),
            Some("1111111111111111111111111111111111111111")
        );
        assert_eq!(s.fork_of, None);
        assert_eq!(s.parent_default, None);
        assert!(s.can_write);
        assert!(!s.auto_merge_allowed);

        let merged = snap(MERGED);
        assert_eq!(merged.pulls[0].status.state, PullState::Merged);
        let draft = snap(DRAFT);
        assert_eq!(draft.pulls[0].status.state, PullState::Draft);
        assert!(snap(NO_PULL).pulls.is_empty());
        assert_eq!(
            snap(NO_PULL).branch_oid.as_deref(),
            Some("1111111111111111111111111111111111111111")
        );
        let none = snap(NO_BRANCH);
        assert!(none.pulls.is_empty());
        assert_eq!(none.branch_oid, None);
    }

    /// Merge sends `head` back as the commit the bar showed, so it is GitHub's head commit and nothing else.
    #[test]
    fn head_is_githubs_head_commit_and_a_node_without_one_is_left_out() {
        let s = snap(OPEN_FAILING);
        assert_eq!(s.pulls[0].status.head, "1111111111111111111111111111111111111111");

        let mut v = load(OPEN_FAILING);
        first_pull(&mut v)["headRefOid"] = json!("main; rm -rf x");
        let without = snapshot_of(&v).unwrap();
        assert_eq!(without.pulls.len(), s.pulls.len() - 1);

        // The branch's own commit is held to the same form.
        let mut v = load(OPEN_FAILING);
        v["data"]["repository"]["ref"]["target"]["oid"] = json!("not-a-commit");
        assert_eq!(snapshot_of(&v).unwrap().branch_oid, None);
    }

    #[test]
    fn a_cross_repository_pull_request_is_left_out() {
        let s = snap(CROSS_REPO);
        let numbers: Vec<u32> = s.pulls.iter().map(|c| c.status.number).collect();
        assert_eq!(numbers, [4]);
        assert_eq!(s.fork_of.as_deref(), Some("up-org/widgets"));
        assert_eq!(s.parent_default.as_deref(), Some("trunk"));
        assert_eq!(s.pulls[0].status.auto_merge, Some(MergeMethod::Squash));
        assert_eq!(s.pulls[0].status.review, ReviewNeed::Approved);

        // Left out before anything else looks at it: a bad base branch on that node doesn't refuse the snapshot.
        let mut v = load(CROSS_REPO);
        first_pull(&mut v)["baseRefName"] = json!("-x");
        assert_eq!(snapshot_of(&v).unwrap().pulls.len(), 1);
    }

    #[test]
    fn checks_put_failures_first() {
        let s = snap(OPEN_FAILING);
        let p = &s.pulls[0].status;
        let got: Vec<(&str, CheckState)> = p.checks.iter().map(|c| (c.name.as_str(), c.state)).collect();
        assert_eq!(
            got,
            [
                ("pass", CheckState::Failed),
                ("maybe-fail", CheckState::Failed),
                ("slow", CheckState::Failed),
                ("e2e", CheckState::Running),
                ("deploy", CheckState::Waiting),
                ("lint", CheckState::Passed),
            ]
        );
        assert_eq!(p.more_checks, 2);
        let c = &p.checks[0];
        assert_eq!(c.kind, CheckKind::Run);
        assert_eq!(c.workflow.as_deref(), Some("ci"));
        assert_eq!(c.run_id, Some(4519306504));
        assert_eq!(
            c.url.as_deref(),
            Some("https://github.com/octo-org/widgets/actions/runs/4519306504/job/137756214968")
        );
        assert_eq!(c.started_ms, rfc3339_ms("2026-11-23T14:20:03Z"));
        assert!(c.started_ms.is_some());
        assert_eq!(c.completed_ms, rfc3339_ms("2026-11-23T14:20:05Z"));
        let queued = &p.checks[4];
        assert_eq!((queued.started_ms, queued.completed_ms), (None, None));

        // The merged fixture holds three failures in GitHub's order.
        let merged = snap(MERGED);
        let names: Vec<&str> = merged.pulls[0].status.checks.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["pass", "maybe-fail", "slow"]);
        assert_eq!(merged.pulls[0].status.more_checks, 0);
    }

    #[test]
    fn the_groups_after_failures_keep_their_order() {
        let v = with_checks(
            load(NO_ROLLUP),
            vec![
                run_node("a", "COMPLETED", json!("NEUTRAL")),
                run_node("b", "COMPLETED", json!("SUCCESS")),
                run_node("c", "COMPLETED", json!("SKIPPED")),
                run_node("d", "COMPLETED", json!("CANCELLED")),
                run_node("e", "COMPLETED", json!("SUCCESS")),
                run_node("f", "COMPLETED", json!("TIMED_OUT")),
            ],
        );
        let s = snapshot_of(&v).unwrap();
        let got: Vec<(&str, CheckState)> = s.pulls[0]
            .status
            .checks
            .iter()
            .map(|c| (c.name.as_str(), c.state))
            .collect();
        assert_eq!(
            got,
            [
                ("f", CheckState::Failed),
                ("d", CheckState::Cancelled),
                ("b", CheckState::Passed),
                ("e", CheckState::Passed),
                ("a", CheckState::Neutral),
                ("c", CheckState::Skipped),
            ]
        );
    }

    #[test]
    fn status_context_and_check_run_share_states() {
        let conclusions = [
            "SUCCESS",
            "FAILURE",
            "TIMED_OUT",
            "STARTUP_FAILURE",
            "ACTION_REQUIRED",
            "CANCELLED",
            "SKIPPED",
            "STALE",
            "NEUTRAL",
        ];
        let mut nodes: Vec<Value> = conclusions
            .iter()
            .map(|c| run_node("r", "COMPLETED", json!(c)))
            .collect();
        for status in ["QUEUED", "WAITING", "PENDING", "REQUESTED", "IN_PROGRESS"] {
            nodes.push(run_node("r", status, Value::Null));
        }
        for state in ["PENDING", "EXPECTED", "SUCCESS", "FAILURE", "ERROR"] {
            nodes.push(context_node(state));
        }
        let s = snapshot_of(&with_checks(load(NO_ROLLUP), nodes)).unwrap();
        let checks = &s.pulls[0].status.checks;
        // Failed, Cancelled, Running, Waiting, Passed, Neutral, Skipped, each in GitHub's order.
        let runs: Vec<CheckState> = checks
            .iter()
            .filter(|c| c.kind == CheckKind::Run)
            .map(|c| c.state)
            .collect();
        assert_eq!(
            runs,
            [
                CheckState::Failed,
                CheckState::Failed,
                CheckState::Failed,
                CheckState::Failed,
                CheckState::Cancelled,
                CheckState::Running,
                CheckState::Waiting,
                CheckState::Waiting,
                CheckState::Waiting,
                CheckState::Waiting,
                CheckState::Passed,
                CheckState::Neutral,
                CheckState::Skipped,
                CheckState::Skipped,
            ]
        );
        let contexts: Vec<CheckState> = checks
            .iter()
            .filter(|c| c.kind == CheckKind::Status)
            .map(|c| c.state)
            .collect();
        assert_eq!(
            contexts,
            [
                CheckState::Failed,
                CheckState::Failed,
                CheckState::Running,
                CheckState::Running,
                CheckState::Passed
            ]
        );
        let c = checks.iter().find(|c| c.kind == CheckKind::Status).unwrap();
        assert_eq!(c.name, "ci/status");
        assert_eq!(c.url.as_deref(), Some("https://example.com/build/1"));
        assert_eq!((c.workflow.clone(), c.run_id, c.completed_ms), (None, None, None));
        assert_eq!(c.started_ms, rfc3339_ms("2026-11-23T14:00:00Z"));
    }

    #[test]
    fn an_unknown_conclusion_is_neutral() {
        let v = with_checks(
            load(NO_ROLLUP),
            vec![
                run_node("a", "COMPLETED", json!("SOMETHING_NEW")),
                run_node("b", "COMPLETED", Value::Null),
                run_node("c", "SOMETHING_NEW", Value::Null),
                context_node("SOMETHING_NEW"),
            ],
        );
        let s = snapshot_of(&v).unwrap();
        let checks = &s.pulls[0].status.checks;
        assert_eq!(checks.len(), 4);
        assert!(checks.iter().all(|c| c.state == CheckState::Neutral));
    }

    #[test]
    fn unknown_values_never_fail_the_snapshot() {
        let mut v = load(OPEN_FAILING);
        first_pull(&mut v)["mergeStateStatus"] = json!("SOMETHING_NEW");
        first_pull(&mut v)["mergeable"] = json!("SOMETHING_NEW");
        first_pull(&mut v)["reviewDecision"] = json!("SOMETHING_NEW");
        repo(&mut v)["viewerPermission"] = json!("SOMETHING_NEW");
        v["data"]["repository"]["pullRequests"]["nodes"][1]["state"] = json!("SOMETHING_NEW");
        let s = snapshot_of(&v).unwrap();
        assert_eq!(s.pulls.len(), 1, "a pull request of an unknown state is left out");
        assert_eq!(s.pulls[0].status.readiness, Readiness::Ready);
        assert_eq!(s.pulls[0].status.review, ReviewNeed::None);
        assert!(!s.can_write);
    }

    #[test]
    fn unknown_merge_state_is_checking_and_asks_in_10_s() {
        let s = snap(UNKNOWN);
        assert_eq!(s.pulls[0].status.readiness, Readiness::Checking);
        assert_eq!(s.next_ms, NEXT_BUSY_MS);
        assert_eq!(NEXT_BUSY_MS, 10_000);
        // Either one alone is enough.
        let mut v = load(UNKNOWN);
        first_pull(&mut v)["mergeable"] = json!("MERGEABLE");
        first_pull(&mut v)["commits"]["nodes"][0]["commit"]["statusCheckRollup"] = Value::Null;
        assert_eq!(snapshot_of(&v).unwrap().pulls[0].status.readiness, Readiness::Checking);
        let mut v = load(UNKNOWN);
        first_pull(&mut v)["mergeStateStatus"] = json!("CLEAN");
        assert_eq!(snapshot_of(&v).unwrap().pulls[0].status.readiness, Readiness::Checking);
    }

    #[test]
    fn a_running_check_asks_in_10_s_and_a_settled_one_in_30() {
        assert_eq!(snap(OPEN_FAILING).next_ms, NEXT_BUSY_MS);
        let settled = snap(NO_ROLLUP);
        assert_eq!(settled.next_ms, NEXT_IDLE_MS);
        assert_eq!(NEXT_IDLE_MS, 30_000);
        let v = with_checks(load(NO_ROLLUP), vec![run_node("a", "COMPLETED", json!("SUCCESS"))]);
        assert_eq!(snapshot_of(&v).unwrap().next_ms, NEXT_IDLE_MS);
        let v = with_checks(load(NO_ROLLUP), vec![run_node("a", "QUEUED", Value::Null)]);
        assert_eq!(snapshot_of(&v).unwrap().next_ms, NEXT_BUSY_MS);
        // An auto-merge asks quickly too.
        let mut v = load(NO_ROLLUP);
        first_pull(&mut v)["autoMergeRequest"] = json!({"mergeMethod": "REBASE"});
        let s = snapshot_of(&v).unwrap();
        assert_eq!(s.pulls[0].status.auto_merge, Some(MergeMethod::Rebase));
        assert_eq!(s.next_ms, NEXT_BUSY_MS);
        // Only the first open pull request counts: a merged one with running checks is ignored.
        let mut v = with_checks(load(MERGED), vec![run_node("a", "IN_PROGRESS", Value::Null)]);
        assert_eq!(snapshot_of(&v).unwrap().next_ms, NEXT_IDLE_MS);
        // And the first open one only, not a later open one.
        let busy = load(OPEN_FAILING)["data"]["repository"]["pullRequests"]["nodes"][0].clone();
        v["data"]["repository"]["pullRequests"]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(busy.clone());
        assert_eq!(snapshot_of(&v).unwrap().next_ms, NEXT_BUSY_MS);
        let mut settled_first = load(NO_ROLLUP);
        settled_first["data"]["repository"]["pullRequests"]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(busy);
        assert_eq!(snapshot_of(&settled_first).unwrap().next_ms, NEXT_IDLE_MS);
        // No pull request at all: idle.
        assert_eq!(snap(NO_PULL).next_ms, NEXT_IDLE_MS);
    }

    #[test]
    fn merge_methods_put_githubs_default_first() {
        assert_eq!(
            snap(NO_ROLLUP).merge_methods,
            [MergeMethod::Merge, MergeMethod::Squash, MergeMethod::Rebase]
        );
        let mut v = load(NO_ROLLUP);
        repo(&mut v)["viewerDefaultMergeMethod"] = json!("SQUASH");
        assert_eq!(
            snapshot_of(&v).unwrap().merge_methods,
            [MergeMethod::Squash, MergeMethod::Merge, MergeMethod::Rebase]
        );
        repo(&mut v)["viewerDefaultMergeMethod"] = json!("REBASE");
        repo(&mut v)["mergeCommitAllowed"] = json!(false);
        assert_eq!(
            snapshot_of(&v).unwrap().merge_methods,
            [MergeMethod::Rebase, MergeMethod::Squash]
        );
        repo(&mut v)["squashMergeAllowed"] = json!(false);
        repo(&mut v)["rebaseMergeAllowed"] = json!(false);
        assert!(snapshot_of(&v).unwrap().merge_methods.is_empty());
        let mut v = load(NO_ROLLUP);
        repo(&mut v)["autoMergeAllowed"] = json!(true);
        assert!(snapshot_of(&v).unwrap().auto_merge_allowed);
    }

    #[test]
    fn can_write_follows_the_viewers_permission() {
        for (permission, expect) in [
            ("ADMIN", true),
            ("MAINTAIN", true),
            ("WRITE", true),
            ("TRIAGE", false),
            ("READ", false),
        ] {
            let mut v = load(NO_ROLLUP);
            repo(&mut v)["viewerPermission"] = json!(permission);
            assert_eq!(snapshot_of(&v).unwrap().can_write, expect, "{permission}");
        }
        let mut v = load(NO_ROLLUP);
        repo(&mut v)["viewerPermission"] = Value::Null;
        assert!(!snapshot_of(&v).unwrap().can_write);
    }

    #[test]
    fn a_null_rollup_is_no_checks() {
        let s = snap(NO_ROLLUP);
        assert!(s.pulls[0].status.checks.is_empty());
        assert_eq!(s.pulls[0].status.more_checks, 0);
        // So is a pull request without a commit, and a rollup without nodes.
        let mut v = load(NO_ROLLUP);
        first_pull(&mut v)["commits"]["nodes"] = json!([]);
        assert!(snapshot_of(&v).unwrap().pulls[0].status.checks.is_empty());
        let mut v = load(NO_ROLLUP);
        first_pull(&mut v)["commits"]["nodes"][0]["commit"]["statusCheckRollup"] = json!({"contexts": null});
        assert!(snapshot_of(&v).unwrap().pulls[0].status.checks.is_empty());
    }

    #[test]
    fn more_checks_is_the_total_less_the_nodes_read_and_never_below_zero() {
        let mut v = with_checks(load(NO_ROLLUP), vec![run_node("a", "COMPLETED", json!("SUCCESS"))]);
        first_pull(&mut v)["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["totalCount"] = json!(101);
        assert_eq!(snapshot_of(&v).unwrap().pulls[0].status.more_checks, 100);
        first_pull(&mut v)["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["totalCount"] = json!(0);
        assert_eq!(snapshot_of(&v).unwrap().pulls[0].status.more_checks, 0);
    }

    #[test]
    fn a_non_https_link_is_dropped() {
        let mut a = run_node("a", "COMPLETED", json!("SUCCESS"));
        a["detailsUrl"] = json!("http://example.com/x");
        let mut b = run_node("b", "COMPLETED", json!("SUCCESS"));
        b["detailsUrl"] = json!("https://example.com/a b");
        let mut c = run_node("c", "COMPLETED", json!("SUCCESS"));
        c["detailsUrl"] = json!("https://example.com/a\u{1b}b");
        let mut d = run_node("d", "COMPLETED", json!("SUCCESS"));
        d["detailsUrl"] = json!("javascript:alert(1)");
        let mut e = context_node("SUCCESS");
        e["targetUrl"] = json!("file:///c:/x");
        let mut f = run_node("f", "COMPLETED", json!("SUCCESS"));
        f["detailsUrl"] = Value::Null;
        let g = run_node("g", "COMPLETED", json!("SUCCESS"));
        let s = snapshot_of(&with_checks(load(NO_ROLLUP), vec![a, b, c, d, e, f, g])).unwrap();
        let checks = &s.pulls[0].status.checks;
        assert_eq!(checks.len(), 7, "the checks stay");
        for c in checks.iter().filter(|c| c.name != "g") {
            assert_eq!(c.url, None, "{}", c.name);
        }
        let g = checks.iter().find(|c| c.name == "g").unwrap();
        assert_eq!(
            g.url.as_deref(),
            Some("https://github.com/octo-org/widgets/actions/runs/1/job/2")
        );

        // A pull request's own link is held to the same rule; one that fails leaves the pull request out.
        let mut v = load(NO_ROLLUP);
        first_pull(&mut v)["url"] = json!("http://github.com/octo-org/widgets/pull/1");
        assert!(snapshot_of(&v).unwrap().pulls.is_empty());
    }

    #[test]
    fn a_repository_that_is_null_is_not_found() {
        assert_eq!(snapshot_of(&load(NOT_FOUND)).err(), Some(GhError::NotFound));
        assert_eq!(
            snapshot_of(&json!({"data": {"repository": null}})).err(),
            Some(GhError::NotFound)
        );
    }

    #[test]
    fn an_answer_without_data_is_unreadable() {
        assert_eq!(snapshot_of(&json!({})).err(), Some(GhError::Unreadable));
        assert_eq!(snapshot_of(&json!({"data": null})).err(), Some(GhError::Unreadable));
        assert_eq!(snapshot_of(&json!([1, 2])).err(), Some(GhError::Unreadable));
    }

    #[test]
    fn a_draft_merge_state_is_not_checking() {
        let s = snap(DRAFT);
        let p = &s.pulls[0].status;
        assert_eq!(p.state, PullState::Draft);
        assert_eq!(p.readiness, Readiness::Ready);
        assert_eq!(s.next_ms, NEXT_IDLE_MS);
    }

    #[test]
    fn conflicts_come_before_unknown() {
        let mut v = load(NO_ROLLUP);
        first_pull(&mut v)["mergeable"] = json!("CONFLICTING");
        first_pull(&mut v)["mergeStateStatus"] = json!("UNKNOWN");
        assert_eq!(snapshot_of(&v).unwrap().pulls[0].status.readiness, Readiness::Conflicts);
        first_pull(&mut v)["mergeable"] = json!("UNKNOWN");
        first_pull(&mut v)["mergeStateStatus"] = json!("DIRTY");
        assert_eq!(snapshot_of(&v).unwrap().pulls[0].status.readiness, Readiness::Conflicts);
        // And Checking before Blocked and Behind.
        first_pull(&mut v)["mergeStateStatus"] = json!("BLOCKED");
        assert_eq!(snapshot_of(&v).unwrap().pulls[0].status.readiness, Readiness::Checking);
        for (state, expect) in [
            ("BLOCKED", Readiness::Blocked),
            ("BEHIND", Readiness::Behind),
            ("CLEAN", Readiness::Ready),
            ("HAS_HOOKS", Readiness::Ready),
            ("UNSTABLE", Readiness::Ready),
            ("DRAFT", Readiness::Ready),
        ] {
            first_pull(&mut v)["mergeable"] = json!("MERGEABLE");
            first_pull(&mut v)["mergeStateStatus"] = json!(state);
            assert_eq!(snapshot_of(&v).unwrap().pulls[0].status.readiness, expect, "{state}");
        }
    }

    #[test]
    fn plain_replaces_control_and_bidi_characters() {
        assert_eq!(plain("a\u{202E}b\u{2028}c\nd", 200), "a b c d");
        for c in [
            '\u{ad}',
            '\u{61c}',
            '\u{200b}',
            '\u{200f}',
            '\u{2028}',
            '\u{202e}',
            '\u{2060}',
            '\u{206f}',
            '\u{feff}',
            '\u{fff9}',
            '\u{fffb}',
            '\u{e0001}',
            '\u{e0020}',
            '\u{e007f}',
            '\u{7}',
            '\u{1b}',
            '\u{7f}',
            '\u{85}',
        ] {
            assert_eq!(plain(&format!("a{c}b"), 200), "a b", "{:x}", c as u32);
        }
        for c in [
            '\u{200a}',
            '\u{2010}',
            '\u{202f}',
            '\u{205f}',
            '\u{2070}',
            '\u{fffc}',
            '\u{e0002}',
            'é',
            '日',
            ' ',
        ] {
            assert_eq!(plain(&format!("a{c}b"), 200), format!("a{c}b"), "{:x}", c as u32);
        }
        assert_eq!(plain("日本語です", 3), "日本語");
        assert_eq!(plain("abc", 3), "abc");
        assert_eq!(plain("", 3), "");
    }

    #[test]
    fn texts_from_github_go_through_plain() {
        let mut v = load(OPEN_FAILING);
        first_pull(&mut v)["title"] = json!(format!("a\u{202E}b{}", "x".repeat(300)));
        let mut node = run_node("n\u{1b}ame", "COMPLETED", json!("SUCCESS"));
        node["checkSuite"] = json!({"workflowRun": {"databaseId": 5, "workflow": {"name": "w\u{200b}f"}}});
        let v = with_checks(v, vec![node]);
        let p = snapshot_of(&v).unwrap().pulls.remove(0).status;
        assert_eq!(p.title.chars().count(), 200);
        assert!(p.title.starts_with("a b"));
        assert_eq!(p.checks[0].name, "n ame");
        assert_eq!(p.checks[0].workflow.as_deref(), Some("w f"));
        assert_eq!(p.checks[0].run_id, Some(5));
    }

    #[test]
    fn a_base_branch_that_starts_with_a_dash_is_refused() {
        let refused = Err(GhError::Refused {
            status: None,
            message: "GitHub named a branch Marshell can't use.".into(),
        });
        let mut v = load(OPEN_FAILING);
        first_pull(&mut v)["baseRefName"] = json!("-x");
        assert_eq!(snapshot_of(&v).map(|_| ()), refused);
        let mut v = load(OPEN_FAILING);
        repo(&mut v)["defaultBranchRef"]["name"] = json!("-x");
        assert_eq!(snapshot_of(&v).map(|_| ()), refused);
        let mut v = load(OPEN_FAILING);
        first_pull(&mut v)["baseRefName"] = json!("a b");
        assert_eq!(snapshot_of(&v).map(|_| ()), refused);
        // The parent's default branch is only dropped.
        let mut v = load(CROSS_REPO);
        repo(&mut v)["parent"]["defaultBranchRef"]["name"] = json!("-x");
        let s = snapshot_of(&v).unwrap();
        assert_eq!(s.parent_default, None);
        assert_eq!(s.fork_of.as_deref(), Some("up-org/widgets"));
    }

    #[test]
    fn a_missing_optional_field_does_not_fail_the_snapshot() {
        let mut v = load(OPEN_FAILING);
        let r = repo(&mut v).as_object_mut().unwrap();
        for k in [
            "rateLimit",
            "parent",
            "ref",
            "defaultBranchRef",
            "viewerDefaultMergeMethod",
            "isFork",
        ] {
            r.remove(k);
        }
        let node = first_pull(&mut v).as_object_mut().unwrap();
        for k in [
            "reviewDecision",
            "autoMergeRequest",
            "viewerCanEnableAutoMerge",
            "isDraft",
            "commits",
        ] {
            node.remove(k);
        }
        let s = snapshot_of(&v).unwrap();
        assert_eq!(s.default_branch, None);
        assert_eq!(s.branch_oid, None);
        assert_eq!(s.pulls.len(), 2);
        assert!(s.pulls[0].status.checks.is_empty());
        assert_eq!(s.merge_methods.first(), Some(&MergeMethod::Merge));
    }
}
