//! POST /v1/sessions/{tab}/github: what GitHub says about the branch the session's checkout is on, for the Changes
//! panel's pull request bar.
//!
//! The repository always comes from the checkout's `origin`, read in the core. gh runs only through
//! `github::gh`, off the async workers, and the asks for one (checkout, branch) share one call
//! (`GithubState::take`). The route is blocking work, then an `.await` for GitHub's answer, then blocking work: no lock
//! is held across a call to gh, a git command or an `.await`.

use super::AppState;
use super::changes::{CancelOnDrop, changes_target, with_bases};
use super::files::row_of;
use crate::changes::{CHECK_BUDGET, ChangesError, Check, READ_BUDGET, Target, uncommitted_files};
use crate::gitbox::refs;
use crate::github::gh::{self, GhError};
use crate::github::plan::{body_of, outgoing, refusal, refusal_for, title_of};
use crate::github::query::status_args;
use crate::github::remote::{RemoteRepo, fetch_repo, push_repo};
use crate::github::status::{Candidate, NEXT_IDLE_MS, Snapshot, snapshot_of};
use crate::github::template::pull_template;
use crate::github::{
    Failure, Flight, Local, Outcome, Take, compare_url, failure_of, has_commit, in_history, local_counts, valid_branch,
};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{
    GithubKind, GithubReply, GithubRequest, OutgoingCommit, PullState, PullStatus, PushPlan, PushPlanRequest,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

/// What the bar needs to ask GitHub and to count commits, once the checkout is known to be a GitHub one.
struct Ctx {
    target: Target,
    repo: RemoteRepo,
    host: String,
    gh: PathBuf,
    branch: String,
}

/// A checkout on a repository GitHub has, that is on no branch Marshell can name.
struct Headless {
    reply: GithubReply,
    /// HEAD's commit id, when it has one.
    head: String,
    /// HEAD is on no branch (detached), as opposed to a branch whose name `valid_branch` refuses.
    detached: bool,
    /// The refused name, as git has it.
    name: Option<String>,
    moved: bool,
}

/// Either the answer already, or what is needed to ask for it.
enum Early {
    Reply(Box<GithubReply>),
    /// The answer, and what the push plan says about why there is no branch.
    Headless(Box<Headless>),
    Go(Box<Ctx>),
}

/// A reply of `kind` that says nothing else. The next look is in `NEXT_IDLE_MS`.
fn bare(kind: GithubKind) -> GithubReply {
    GithubReply {
        kind,
        checkout: String::new(),
        host: String::new(),
        repo: String::new(),
        branch: None,
        default_branch: None,
        fork_of: None,
        compare_url: None,
        pull: None,
        closed: None,
        on_github: false,
        to_push: 0,
        github_ahead: false,
        can_write: false,
        merge_methods: Vec::new(),
        auto_merge_allowed: false,
        push: None,
        push_blocked: None,
        reason: None,
        next_ms: NEXT_IDLE_MS,
    }
}

/// A reply of `kind` for the repository `ctx` names.
fn about(kind: GithubKind, ctx: &Ctx) -> GithubReply {
    GithubReply {
        checkout: ctx.target.checkout.to_string_lossy().into_owned(),
        host: ctx.host.clone(),
        repo: format!("{}/{}", ctx.repo.owner, ctx.repo.name),
        branch: Some(ctx.branch.clone()),
        ..bare(kind)
    }
}

/// `None`, with the sentence of a Changes failure or refusal.
fn none_for(error: ChangesError) -> GithubReply {
    GithubReply {
        reason: match error {
            ChangesError::NoRepo => None,
            ChangesError::Refused(sentence) | ChangesError::Failed(sentence) => Some(sentence),
        },
        ..bare(GithubKind::None)
    }
}

/// Where gh is: the test's, else the PATH a session gets (kept for `GH_PATH_AGE`). Blocking.
fn find_gh(st: &AppState) -> Option<PathBuf> {
    st.github()
        .gh_path(|| (st.0.tab_path)().and_then(|path| gh::find_gh(&path)))
}

/// Steps 1 to 4: whether the bar is on, the checkout, its GitHub repository, gh, and a branch to ask about. Blocking.
fn prepare(
    st: &AppState,
    row: &marshell_protocol::api::SessionInfo,
    reported: Option<PathBuf>,
    cancel: &AtomicBool,
) -> Early {
    let reply = |r: GithubReply| Early::Reply(Box::new(r));
    if !st.settings_snapshot().changes.pull_requests {
        return reply(bare(GithubKind::Off));
    }
    let target = match changes_target(row, reported) {
        Ok(target) => with_bases(st, target),
        Err(_) => return reply(bare(GithubKind::None)),
    };
    let check = match st
        .changes()
        .check(st.home(), &target, Instant::now() + CHECK_BUDGET, cancel)
    {
        Ok(check) => check,
        Err(e) => return reply(none_for(e)),
    };
    let Some(repo) = fetch_repo(&check.origin) else {
        return reply(bare(GithubKind::None));
    };
    let host = match host_of(st, &repo) {
        Some(host) => host,
        None => return reply(bare(GithubKind::None)),
    };
    let named = check.head.branch.clone();
    let branch = named.clone().filter(|b| valid_branch(b));
    let ctx = |gh: PathBuf, branch: String| Ctx {
        target: target.clone(),
        repo: repo.clone(),
        host: host.clone(),
        gh,
        branch,
    };
    let Some(gh) = find_gh(st) else {
        let ctx = ctx(PathBuf::new(), branch.unwrap_or_default());
        return reply(GithubReply {
            branch: None,
            ..about(GithubKind::NoGh, &ctx)
        });
    };
    match branch {
        Some(branch) => Early::Go(Box::new(ctx(gh, branch))),
        None => {
            let ctx = ctx(gh, String::new());
            Early::Headless(Box::new(Headless {
                reply: GithubReply {
                    branch: None,
                    ..about(GithubKind::None, &ctx)
                },
                head: check.head_oid.clone().unwrap_or_default(),
                detached: named.is_none(),
                name: named,
                moved: ctx.target.moved,
            }))
        }
    }
}

/// The GitHub host of `repo`: `github.com` for it, another host only when gh is signed in to it. A host that can't
/// be told (gh isn't there, or says nothing) is not GitHub's. Blocking.
fn host_of(st: &AppState, repo: &RemoteRepo) -> Option<String> {
    if let Some(host) = crate::github::remote::github_host(&repo.host, &[]) {
        return Some(host);
    }
    let gh = find_gh(st)?;
    let signed_in = st.github().auth_hosts(|| gh::auth_hosts(&gh, st.home()));
    match signed_in {
        Ok(hosts) => crate::github::remote::github_host(&repo.host, &hosts),
        Err(GhError::Missing) => {
            st.github().forget_gh_path();
            None
        }
        Err(_) => None,
    }
}

/// One call to GitHub for the branch's status. Blocking: up to `GH_DEADLINE`.
fn ask(ctx: &Ctx, home: &std::path::Path) -> Outcome {
    let args = status_args(&ctx.repo, &ctx.branch);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let answer = gh::api(&ctx.gh, home, &ctx.host, &args, None).map_err(failure_of)?;
    snapshot_of(&answer).map_err(failure_of)
}

/// The answer of an ask that never came.
fn lost() -> Failure {
    Failure {
        kind: GithubKind::Unreachable,
        reason: Some("GitHub's answer didn't arrive.".to_string()),
        next_ms: NEXT_IDLE_MS,
    }
}

/// What the ask in flight brings.
async fn wait(mut flight: Flight) -> Outcome {
    let got = flight
        .wait_for(|answer| answer.is_some())
        .await
        .map(|answer| answer.clone());
    match got {
        Ok(Some(outcome)) => outcome,
        _ => Err(lost()),
    }
}

/// Step 5: the snapshot GitHub gave, from the one kept, from the ask already in flight, or from a call of our own.
async fn snapshot(st: &AppState, ctx: &Arc<Ctx>, fresh: bool) -> Outcome {
    match st.github_arc().take(&ctx.target.checkout, &ctx.branch, fresh) {
        Take::Stored(snapshot) => Ok(snapshot),
        Take::Wait(flight) => wait(flight).await,
        Take::Lead(lead) => {
            let flight = lead.flight();
            let (state, ctx) = (st.clone(), ctx.clone());
            // The call outlives this request: others wait for it. If it never runs or panics, `lead` is dropped and
            // everyone waiting is told.
            drop(tokio::task::spawn_blocking(move || {
                let changes = state.changes_arc();
                lead.run(|| ask(&ctx, state.home()), |checkout| changes.forget(checkout));
            }));
            wait(flight).await
        }
    }
}

/// What the checkout holds against the snapshot.
struct Counted {
    pull: Option<PullStatus>,
    closed: Option<u32>,
    local: Local,
    plan: Gathered,
}

/// What a push plan needs of the checkout besides the reply, read in the same look at it. Empty when nothing is asked
/// for, and where it can't be counted.
#[derive(Default)]
struct Gathered {
    /// HEAD's commit id, when it has one.
    head: String,
    commits: Vec<OutgoingCommit>,
    more: u32,
    uncommitted: u32,
}

/// Step 7: the first open or draft pull request; else the newest merged or closed one whose head is in the branch's
/// history, so a reused branch name doesn't read as merged. A closed one is only its number.
fn choose(
    git: &crate::gitbox::GitBox,
    check: &Check,
    pulls: &[Candidate],
) -> Result<(Option<PullStatus>, Option<u32>), crate::changes::Halt> {
    if let Some(open) = pulls
        .iter()
        .find(|c| matches!(c.status.state, PullState::Open | PullState::Draft))
    {
        return Ok((Some(open.status.clone()), None));
    }
    for candidate in pulls {
        if in_history(git, check, &candidate.status.head)? {
            return Ok(match candidate.status.state {
                PullState::Closed => (None, Some(candidate.status.number)),
                _ => (Some(candidate.status.clone()), None),
            });
        }
    }
    Ok((None, None))
}

/// Steps 6 to 12: the reply for a snapshot. Blocking.
fn finish(st: &AppState, ctx: &Ctx, outcome: Outcome, cancel: &AtomicBool) -> GithubReply {
    finish_with(st, ctx, outcome, cancel, false).0
}

/// `finish`, and with `plan` what a push plan needs, read in the same look at the checkout as the reply's counts.
fn finish_with(st: &AppState, ctx: &Ctx, outcome: Outcome, cancel: &AtomicBool, plan: bool) -> (GithubReply, Gathered) {
    let checkout = &ctx.target.checkout;
    let snapshot: Snapshot = match outcome {
        Ok(snapshot) => snapshot,
        Err(failure) => {
            let reply = GithubReply {
                reason: failure.reason,
                next_ms: failure.next_ms,
                ..about(failure.kind, ctx)
            };
            return (reply, Gathered::default());
        }
    };
    if snapshot.default_branch.as_deref() == Some(ctx.branch.as_str()) {
        if st.github().note_pull(checkout, &ctx.branch, None, None) {
            st.changes().forget(checkout);
        }
        let reply = GithubReply {
            default_branch: snapshot.default_branch.clone(),
            next_ms: snapshot.next_ms,
            ..about(GithubKind::DefaultBranch, ctx)
        };
        return (reply, Gathered::default());
    }

    let counted = st.changes().with_box(
        st.home(),
        &ctx.target,
        Instant::now() + READ_BUDGET,
        cancel,
        |git, check| {
            // The checkout moved to another branch since the ask began: this snapshot is not about it.
            if check.head.branch.as_deref() != Some(ctx.branch.as_str()) {
                return Ok(None);
            }
            let (pull, closed) = choose(git, check, &snapshot.pulls)?;
            // A branch that isn't on GitHub is counted from the remote's default branch, else the check's base.
            let base_oid = match (&snapshot.branch_oid, &snapshot.default_branch) {
                (None, default) => {
                    let remote = match default {
                        Some(default) => {
                            refs::resolve(git.git_dir(), git.common(), &format!("refs/remotes/origin/{default}"))?
                        }
                        None => None,
                    };
                    remote.or_else(|| check.base.as_ref().map(|b| b.oid.clone()))
                }
                (Some(_), _) => None,
            };
            let local = local_counts(
                git,
                check,
                snapshot.branch_oid.as_deref(),
                base_oid.as_ref().map(refs::Oid::as_str),
            )?;
            let mut gathered = Gathered::default();
            if plan && let Some(head) = &check.head_oid {
                gathered.head = head.clone();
                // The commits `to_push` counted: from GitHub's commit for the branch when it is here, else from the
                // base the branch was counted from. With neither, none are listed.
                let from = match snapshot.branch_oid.as_deref().and_then(refs::Oid::parse) {
                    Some(branch) => has_commit(git, &branch)?.then_some(branch),
                    None => base_oid,
                };
                if let Some(from) = from {
                    (gathered.commits, gathered.more) = outgoing(git, from.as_str(), head)?;
                }
                gathered.uncommitted = uncommitted_files(git, check, Instant::now() + READ_BUDGET, cancel)?;
            }
            Ok(Some((
                Counted {
                    pull,
                    closed,
                    local,
                    plan: gathered,
                },
                check.origin.clone(),
            )))
        },
    );
    let (counted, origin) = match counted {
        Ok(Some(found)) => found,
        Ok(None) => return (about(GithubKind::None, ctx), Gathered::default()),
        Err(e) => return (none_for(e), Gathered::default()),
    };

    let open_base = counted
        .pull
        .as_ref()
        .filter(|p| matches!(p.state, PullState::Open | PullState::Draft))
        .map(|p| p.base.as_str());
    let seen = counted.pull.as_ref().map(|p| (p.id.clone(), p.number));
    if st.github().note_pull(checkout, &ctx.branch, seen, open_base) {
        st.changes().forget(checkout);
    }
    let blocked = !matches!(push_repo(&origin), Ok(ref pushes) if *pushes == ctx.repo);
    let reply = GithubReply {
        default_branch: snapshot.default_branch.clone(),
        compare_url: compare_url(
            &ctx.host,
            &ctx.repo.owner,
            &ctx.branch,
            snapshot.fork_of.as_deref(),
            snapshot.parent_default.as_deref(),
        ),
        fork_of: snapshot.fork_of.clone(),
        pull: counted.pull,
        closed: counted.closed,
        on_github: counted.local.on_github,
        to_push: counted.local.to_push,
        github_ahead: counted.local.github_ahead,
        can_write: snapshot.can_write,
        merge_methods: snapshot.merge_methods.clone(),
        auto_merge_allowed: snapshot.auto_merge_allowed,
        push: st.github().running_push(checkout),
        push_blocked: blocked.then(|| {
            format!(
                "origin pushes somewhere other than {}/{}, so push from the terminal.",
                ctx.repo.owner, ctx.repo.name
            )
        }),
        next_ms: snapshot.next_ms,
        ..about(GithubKind::Ready, ctx)
    };
    (reply, counted.plan)
}

/// POST /v1/sessions/{tab}/github. 404 for a tab that doesn't exist; every other case is an answer.
pub async fn github(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<GithubRequest>) -> Response {
    let reported = st.session(&tab).and_then(|s| s.current_cwd());
    let Some(row) = row_of(&st, &tab) else {
        return (StatusCode::NOT_FOUND, "There is no such session.").into_response();
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let _stop = CancelOnDrop(cancel.clone());
    let ctx = {
        let (state, cancel) = (st.clone(), cancel.clone());
        match tokio::task::spawn_blocking(move || prepare(&state, &row, reported, &cancel)).await {
            Ok(Early::Reply(reply)) => return Json(*reply).into_response(),
            Ok(Early::Headless(headless)) => return Json(headless.reply).into_response(),
            Ok(Early::Go(ctx)) => Arc::new(*ctx),
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    };
    let outcome = snapshot(&st, &ctx, req.fresh).await;
    let state = st.clone();
    match tokio::task::spawn_blocking(move || finish(&state, &ctx, outcome, &cancel)).await {
        Ok(reply) => Json(reply).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// The plan for a checkout that is on no branch Marshell can name.
fn headless_plan(h: Headless) -> PushPlan {
    let named = GithubReply {
        branch: h.name,
        ..h.reply.clone()
    };
    PushPlan {
        checkout: h.reply.checkout,
        branch: String::new(),
        head: h.head,
        moved: h.moved,
        on_github: false,
        to_push: 0,
        commits: Vec::new(),
        more: 0,
        uncommitted: 0,
        base: String::new(),
        title: String::new(),
        body: String::new(),
        refusal: refusal(&named, h.detached),
    }
}

/// The plan for a reply from a checkout on a branch. Blocking: it reads the pull request template.
fn plan_of(reply: GithubReply, gathered: Gathered, moved: bool, create: bool) -> PushPlan {
    let branch = reply.branch.clone().unwrap_or_default();
    let open_base = reply
        .pull
        .as_ref()
        .filter(|p| matches!(p.state, PullState::Open | PullState::Draft))
        .map(|p| p.base.clone());
    let template = (reply.kind == GithubKind::Ready)
        .then(|| pull_template(std::path::Path::new(&reply.checkout)))
        .flatten();
    PushPlan {
        refusal: refusal_for(&reply, false, create),
        title: title_of(&gathered.commits, &branch),
        body: body_of(template, &gathered.commits),
        base: open_base.or(reply.default_branch.clone()).unwrap_or_default(),
        checkout: reply.checkout,
        branch,
        head: gathered.head,
        moved,
        on_github: reply.on_github,
        to_push: reply.to_push,
        commits: gathered.commits,
        more: gathered.more,
        uncommitted: gathered.uncommitted,
    }
}

/// POST /v1/sessions/{tab}/github/plan: what a push (or, with `create`, a push and a new pull request) would send
/// and say, and why Marshell will not, from the reply the bar has and the checkout. It uses what GitHub last said and
/// asks only when nothing is kept. 404 for a tab that does not exist; every other case is an answer.
pub async fn plan(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<PushPlanRequest>) -> Response {
    let reported = st.session(&tab).and_then(|s| s.current_cwd());
    let Some(row) = row_of(&st, &tab) else {
        return (StatusCode::NOT_FOUND, "There is no such session.").into_response();
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let _stop = CancelOnDrop(cancel.clone());
    let ctx = {
        let (state, cancel) = (st.clone(), cancel.clone());
        match tokio::task::spawn_blocking(move || prepare(&state, &row, reported, &cancel)).await {
            Ok(Early::Reply(reply)) => {
                let (reply, create) = (*reply, req.create);
                let done =
                    tokio::task::spawn_blocking(move || plan_of(reply, Gathered::default(), false, create)).await;
                return match done {
                    Ok(plan) => Json(plan).into_response(),
                    Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                };
            }
            Ok(Early::Headless(headless)) => return Json(headless_plan(*headless)).into_response(),
            Ok(Early::Go(ctx)) => Arc::new(*ctx),
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    };
    let outcome = snapshot(&st, &ctx, false).await;
    let state = st.clone();
    let created = tokio::task::spawn_blocking(move || {
        let (reply, gathered) = finish_with(&state, &ctx, outcome, &cancel, true);
        plan_of(reply, gathered, ctx.target.moved, req.create)
    })
    .await;
    match created {
        Ok(plan) => Json(plan).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
