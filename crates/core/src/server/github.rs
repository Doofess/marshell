//! POST /v1/sessions/{tab}/github: what GitHub says about the branch the session's checkout is on, for the Changes
//! panel's pull request bar.
//!
//! The repository always comes from the checkout's `origin`, read in the core. gh runs only through
//! `github::gh`, off the async workers, and the asks for one (checkout, branch) share one call
//! (`GithubState::take`). The route is blocking work, then an `.await` for GitHub's answer, then blocking work: no lock
//! is held across a call to gh, a git command or an `.await`.

use super::AppState;
use super::attachments::effective_agent;
use super::changes::{CancelOnDrop, agent_ready, changes_target, type_in, with_bases};
use super::files::row_of;
use super::sessions::cannot_resume;
use crate::changes::{CHECK_BUDGET, ChangesError, Check, READ_BUDGET, Target, uncommitted_files};
use crate::gitbox::refs;
use crate::github::actions::{self, NOT_THIS_THREAD, Rejection};
use crate::github::ask::{AskContext, ask_text, typed_text};
use crate::github::gh::{self, GhError};
use crate::github::plan::{body_of, outgoing, refusal, refusal_for, title_of};
use crate::github::push::{AlreadyRunning, PushLaunch};
use crate::github::query::status_args;
use crate::github::remote::{RemoteRepo, fetch_repo, push_repo};
use crate::github::status::{Candidate, NEXT_IDLE_MS, Snapshot, snapshot_of};
use crate::github::template::pull_template;
use crate::github::threads;
use crate::github::{
    Failure, Flight, Local, Outcome, Take, compare_url, failure_of, has_commit, in_history, local_counts, valid_branch,
};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{
    AskForm, AskReply, AskRequest, CreatePull, GithubKind, GithubReply, GithubRequest, OutgoingCommit, PullAction,
    PullMergeRequest, PullState, PullStatus, PushPlan, PushPlanRequest, PushRequest, PushStarted, ThreadAction,
    ThreadActionKind, ThreadsRequest,
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
    /// HEAD's commit id, when it has one.
    head: String,
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
        head: check.head_oid.clone().unwrap_or_default(),
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

const SWITCHED_OFF: &str = "Pull requests from GitHub are switched off in Settings.";
const MOVED_FOLDER: &str = "The session moved to another folder. Look again before you go on.";
const MOVED_BRANCH: &str = "The branch moved since you looked. Check the commits again.";
const PUSH_RUNNING: &str = "A push of this checkout is already running.";
const NO_GIT: &str = "Git isn't installed, or isn't on the PATH a session gets.";
const NO_NAME: &str = "That isn't a branch name Marshell can push.";

/// A 409 whose body is `{reason}`.
fn refused(reason: &str) -> Response {
    (StatusCode::CONFLICT, Json(serde_json::json!({ "reason": reason }))).into_response()
}

/// What a click on Push is checked against: the reply a fresh check of the checkout gives, whether that checkout is on
/// no branch, its `HEAD`'s commit id, and the repository the bar shows (when there is one).
struct Fresh {
    reply: GithubReply,
    detached: bool,
    head: String,
    repo: Option<RemoteRepo>,
}

/// The refusals of a push, in order, then the push. Blocking: it reads the PATH and the disk.
fn start_push(st: &AppState, req: &PushRequest, fresh: Fresh) -> Result<PushStarted, String> {
    let Fresh {
        reply,
        detached,
        head,
        repo,
    } = fresh;
    if reply.kind == GithubKind::Off {
        return Err(SWITCHED_OFF.to_string());
    }
    // A reply that names no checkout (the origin isn't GitHub's, or the folder is no checkout now) has nothing to
    // compare: `refusal` says why there is no push.
    if !reply.checkout.is_empty() && !crate::projects::same_path(&req.checkout, &reply.checkout) {
        return Err(MOVED_FOLDER.to_string());
    }
    if let Some(sentence) = refusal(&reply, detached) {
        return Err(sentence);
    }
    if reply.checkout.is_empty() {
        return Err(MOVED_FOLDER.to_string());
    }
    // Nothing refused the branch, so the reply is a Ready one about a branch Marshell can name.
    let branch = reply.branch.clone().unwrap_or_default();
    let Some(repo) = repo.filter(|_| valid_branch(&branch)) else {
        return Err(NO_NAME.to_string());
    };
    if req.branch != branch || head.is_empty() || req.head != head {
        return Err(MOVED_BRANCH.to_string());
    }
    let git = (st.0.tab_path)()
        .and_then(|path| gh::find_exe(&path, "git"))
        .ok_or_else(|| NO_GIT.to_string())?;
    let launch = PushLaunch {
        checkout: PathBuf::from(&reply.checkout),
        branch,
        repo,
        git,
        extra: st.push_config().to_vec(),
    };
    match st.github_arc().start_push(launch) {
        Ok(id) => Ok(PushStarted { id }),
        Err(AlreadyRunning) => Err(PUSH_RUNNING.to_string()),
    }
}

/// POST /v1/sessions/{tab}/push: starts the confirmed push of the session's checkout, after a fresh check of it
/// (GitHub is asked as the plan route asks: from what is kept, else once). It types nothing into the session. 404
/// for a tab that doesn't exist; a refusal is a 409 with `{reason}`.
pub async fn push(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<PushRequest>) -> Response {
    let reported = st.session(&tab).and_then(|s| s.current_cwd());
    let Some(row) = row_of(&st, &tab) else {
        return (StatusCode::NOT_FOUND, "There is no such session.").into_response();
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let _stop = CancelOnDrop(cancel.clone());
    let early = {
        let (state, cancel) = (st.clone(), cancel.clone());
        tokio::task::spawn_blocking(move || prepare(&state, &row, reported, &cancel)).await
    };
    let fresh = match early {
        Ok(Early::Reply(reply)) => Fresh {
            reply: *reply,
            detached: false,
            head: String::new(),
            repo: None,
        },
        Ok(Early::Headless(headless)) => {
            let h = *headless;
            Fresh {
                reply: GithubReply {
                    branch: h.name,
                    ..h.reply
                },
                detached: h.detached,
                head: h.head,
                repo: None,
            }
        }
        Ok(Early::Go(ctx)) => {
            let ctx = Arc::new(*ctx);
            let outcome = snapshot(&st, &ctx, false).await;
            let state = st.clone();
            let done = tokio::task::spawn_blocking(move || {
                let (reply, gathered) = finish_with(&state, &ctx, outcome, &cancel, true);
                Fresh {
                    reply,
                    detached: false,
                    head: gathered.head,
                    repo: Some(ctx.repo.clone()),
                }
            })
            .await;
            match done {
                Ok(fresh) => fresh,
                Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let state = st.clone();
    match tokio::task::spawn_blocking(move || start_push(&state, &req, fresh)).await {
        Ok(Ok(started)) => Json(started).into_response(),
        Ok(Err(reason)) => refused(&reason),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(serde::Deserialize)]
pub struct ProgressQuery {
    from: Option<u64>,
}

/// GET /v1/pushes/{id}?from=<n>: the push's state and its lines from `n - 1` on (`from` is how many lines the caller
/// has seen; the last of them may have been redrawn). 404 for an id that isn't a push's.
pub async fn push_progress(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<ProgressQuery>,
) -> Response {
    match st.github().push_progress(&id, q.from.unwrap_or(0)) {
        Some(progress) => Json(progress).into_response(),
        None => (StatusCode::NOT_FOUND, "There is no such push.").into_response(),
    }
}

/// POST /v1/pushes/{id}/cancel: ends the push whole. 204 also for one that has ended; 404 for an unknown id.
pub async fn cancel_push(State(st): State<AppState>, Path(id): Path<String>) -> StatusCode {
    if st.github().cancel_push(&id) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

/// POST /v1/pushes/stop: ends every push, and answers once they have (2 s at most). The quit question's "Quit".
pub async fn stop_pushes(State(st): State<AppState>) -> StatusCode {
    let github = st.github_arc();
    let _ = tokio::task::spawn_blocking(move || github.stop_pushes(std::time::Duration::from_secs(2))).await;
    StatusCode::NO_CONTENT
}

const NOT_THIS_PULL: &str = "That isn't this branch's pull request any more. Look again.";
const NOT_GITHUB: &str = "This checkout's origin isn't a GitHub repository.";

/// What an action route found on its fresh check of the checkout.
enum Acting {
    /// On a branch Marshell can name, in a GitHub repository, with gh found.
    Go(Box<Ctx>),
    /// The checkout is on no branch Marshell can name.
    NoBranch,
}

/// A 400 whose body is the sentence.
fn malformed(sentence: &str) -> Response {
    (StatusCode::BAD_REQUEST, sentence.to_string()).into_response()
}

/// A 503 whose body is `{reason}`.
fn unavailable(reason: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({ "reason": reason })),
    )
        .into_response()
}

/// The answer to a write GitHub did not do: 409 for its refusal, 503 for the rest.
fn rejected(st: &AppState, rejection: Rejection) -> Response {
    match rejection {
        Rejection::Refused(reason) => refused(&reason),
        Rejection::Unavailable(reason) => unavailable(&reason),
        Rejection::NoGh => {
            st.github().forget_gh_path();
            unavailable(actions::NEED_GH)
        }
    }
}

/// The fresh check an action route makes before it acts, in order: the switch is on, `checkout` is the session's
/// checkout now, the origin is a GitHub repository, gh is there. `Err` is the answer to send. Blocking.
fn acting(
    st: &AppState,
    row: &marshell_protocol::api::SessionInfo,
    reported: Option<PathBuf>,
    cancel: &AtomicBool,
    checkout: &str,
) -> Result<Acting, Box<Response>> {
    let moved = |now: &str| !now.is_empty() && !crate::projects::same_path(checkout, now);
    match prepare(st, row, reported, cancel) {
        Early::Reply(reply) => {
            if reply.kind == GithubKind::Off {
                return Err(Box::new(refused(SWITCHED_OFF)));
            }
            if moved(&reply.checkout) {
                return Err(Box::new(refused(MOVED_FOLDER)));
            }
            match reply.kind {
                GithubKind::NoGh => Err(Box::new(unavailable(actions::NEED_GH))),
                _ => Err(Box::new(refused(reply.reason.as_deref().unwrap_or(NOT_GITHUB)))),
            }
        }
        Early::Headless(headless) => {
            if moved(&headless.reply.checkout) {
                return Err(Box::new(refused(MOVED_FOLDER)));
            }
            Ok(Acting::NoBranch)
        }
        Early::Go(ctx) => {
            if moved(&ctx.target.checkout.to_string_lossy()) || checkout.is_empty() {
                return Err(Box::new(refused(MOVED_FOLDER)));
            }
            Ok(Acting::Go(ctx))
        }
    }
}

/// Runs `act` on the checkout's context after the fresh check, off the async workers, for a request that named
/// `checkout`. `no_branch` is the refusal for a checkout on no branch.
async fn act_on(
    st: AppState,
    tab: String,
    checkout: String,
    no_branch: &'static str,
    act: impl FnOnce(&AppState, Ctx) -> Response + Send + 'static,
) -> Response {
    let reported = st.session(&tab).and_then(|s| s.current_cwd());
    let Some(row) = row_of(&st, &tab) else {
        return (StatusCode::NOT_FOUND, "There is no such session.").into_response();
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let _stop = CancelOnDrop(cancel.clone());
    let state = st.clone();
    let done = tokio::task::spawn_blocking(move || match acting(&state, &row, reported, &cancel, &checkout) {
        Ok(Acting::Go(ctx)) => act(&state, *ctx),
        Ok(Acting::NoBranch) => refused(no_branch),
        Err(response) => *response,
    })
    .await;
    done.unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// POST /v1/sessions/{tab}/github/create: opens a pull request for the session's checkout and its branch on GitHub.
/// 404 for a tab that doesn't exist, 400 for a request that is malformed, 409 for a refusal, 503 when gh or GitHub
/// can't be used. The repository is the checkout's own, never one the request names.
pub async fn create(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<CreatePull>) -> Response {
    // A tab that doesn't exist is a 404 before anything else (`act_on` answers it).
    if row_of(&st, &tab).is_some()
        && let Some(sentence) = actions::check_create(&req)
    {
        return malformed(sentence);
    }
    let checkout = req.checkout.clone();
    act_on(st, tab, checkout, MOVED_BRANCH, move |state, ctx| {
        if req.branch != ctx.branch || ctx.head.is_empty() || req.head != ctx.head {
            return refused(MOVED_BRANCH);
        }
        match actions::create(&ctx.gh, state.home(), &ctx.host, &ctx.repo, &req) {
            Ok(created) => {
                state.github().mark_stale(&ctx.target.checkout);
                Json(created).into_response()
            }
            Err(rejection) => rejected(state, rejection),
        }
    })
    .await
}

/// The pull request `id` and `number` name, when it is the one the core last read for the checkout and its branch.
fn is_seen(st: &AppState, ctx: &Ctx, id: &str, number: u32) -> bool {
    st.github().pull_seen(&ctx.target.checkout, &ctx.branch) == Some((id.to_string(), number))
}

/// POST /v1/sessions/{tab}/github/merge: merges the branch's pull request now or turns on its auto-merge. 404, 400,
/// 409 and 503 as for create; the pull request must be the one the core last read for this checkout and branch.
pub async fn merge(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<PullMergeRequest>) -> Response {
    if row_of(&st, &tab).is_some()
        && let Some(sentence) = actions::check_merge(&req)
    {
        return malformed(sentence);
    }
    let checkout = req.checkout.clone();
    act_on(st, tab, checkout, NOT_THIS_PULL, move |state, ctx| {
        if !is_seen(state, &ctx, &req.id, req.number) {
            return refused(NOT_THIS_PULL);
        }
        match actions::merge(&ctx.gh, state.home(), &ctx.host, &ctx.repo, &req) {
            Ok(()) => {
                state.github().mark_stale(&ctx.target.checkout);
                StatusCode::NO_CONTENT.into_response()
            }
            Err(rejection) => rejected(state, rejection),
        }
    })
    .await
}

/// POST /v1/sessions/{tab}/github/pull: marks the branch's pull request ready, makes it a draft, or cancels its
/// auto-merge. Answers as merge does.
pub async fn pull_action(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<PullAction>) -> Response {
    if row_of(&st, &tab).is_some()
        && let Some(sentence) = actions::check_pull(&req)
    {
        return malformed(sentence);
    }
    let checkout = req.checkout.clone();
    act_on(st, tab, checkout, NOT_THIS_PULL, move |state, ctx| {
        if !is_seen(state, &ctx, &req.id, req.number) {
            return refused(NOT_THIS_PULL);
        }
        match actions::pull_action(&ctx.gh, state.home(), &ctx.host, &ctx.repo, &req) {
            Ok(()) => {
                state.github().mark_stale(&ctx.target.checkout);
                StatusCode::NO_CONTENT.into_response()
            }
            Err(rejection) => rejected(state, rejection),
        }
    })
    .await
}

const CANT_RESOLVE: &str = "GitHub doesn't let this sign-in resolve this thread.";
const CANT_UNRESOLVE: &str = "GitHub doesn't let this sign-in unresolve this thread.";
const CANT_REPLY: &str = "GitHub doesn't let this sign-in reply to this thread.";

/// POST /v1/sessions/{tab}/github/threads: the review threads, reviews and comments of the branch's pull request. 404
/// for a tab that doesn't exist, 400 for a request that is malformed, 409 for a refusal, 503 when gh or GitHub can't
/// be used. The pull request must be the one the core last reported for this checkout and branch; what was read
/// within `FRESH_GAP` answers again without a call, and what is read is kept for the actions on its threads.
pub async fn threads(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<ThreadsRequest>) -> Response {
    if row_of(&st, &tab).is_some()
        && let Some(sentence) = actions::check_threads(&req)
    {
        return malformed(sentence);
    }
    let checkout = req.checkout.clone();
    act_on(st, tab, checkout, NOT_THIS_PULL, move |state, ctx| {
        if !is_seen(state, &ctx, &req.id, req.number) {
            return refused(NOT_THIS_PULL);
        }
        let github = state.github();
        let (checkout, branch) = (&ctx.target.checkout, &ctx.branch);
        if let Some(kept) = github.threads_kept(checkout, branch, &req.id, req.number) {
            return Json(kept).into_response();
        }
        let generation = github.generation(checkout, branch);
        match threads::read(&ctx.gh, state.home(), &ctx.host, &ctx.repo, req.number) {
            Ok(read) if read.pull_id != req.id => refused(NOT_THIS_PULL),
            Ok(read) => {
                let answer = read.threads.clone();
                github.keep_threads(checkout, branch, generation, read);
                Json(answer).into_response()
            }
            Err(e) => rejected(state, actions::rejection(e, &ctx.repo)),
        }
    })
    .await
}

/// POST /v1/sessions/{tab}/github/thread: resolves, unresolves or replies to a thread of the branch's pull request.
/// Answers as threads does, and 204 when done. The pull request must be the one the core last reported, the thread one
/// the core read for it, and GitHub's own flag for the action true in that read; the reply goes under the thread's
/// first comment as the core read it.
pub async fn thread_action(
    State(st): State<AppState>,
    Path(tab): Path<String>,
    Json(req): Json<ThreadAction>,
) -> Response {
    if row_of(&st, &tab).is_some()
        && let Some(sentence) = actions::check_thread(&req)
    {
        return malformed(sentence);
    }
    let checkout = req.checkout.clone();
    act_on(st, tab, checkout, NOT_THIS_PULL, move |state, ctx| {
        if !is_seen(state, &ctx, &req.id, req.number) {
            return refused(NOT_THIS_PULL);
        }
        let github = state.github();
        let Some(seen) = github.thread_seen(&ctx.target.checkout, &ctx.branch, &req.id, req.number, &req.thread) else {
            return refused(NOT_THIS_THREAD);
        };
        let (allowed, sentence) = match req.action {
            ThreadActionKind::Resolve => (seen.can_resolve, CANT_RESOLVE),
            ThreadActionKind::Unresolve => (seen.can_unresolve, CANT_UNRESOLVE),
            ThreadActionKind::Reply => (seen.can_reply, CANT_REPLY),
        };
        if !allowed {
            return refused(sentence);
        }
        match actions::thread_action(&ctx.gh, state.home(), &ctx.host, &ctx.repo, &req, seen.root) {
            Ok(()) => {
                github.mark_stale(&ctx.target.checkout);
                StatusCode::NO_CONTENT.into_response()
            }
            Err(rejection) => {
                // GitHub may have taken the write when its answer didn't come or couldn't be read: the next look asks.
                if matches!(rejection, Rejection::Unavailable(_)) {
                    github.mark_stale(&ctx.target.checkout);
                }
                rejected(state, rejection)
            }
        }
    })
    .await
}

const HEARD_NOTHING: &str = "Marshell hasn't heard from GitHub about this branch yet.";

/// What the text of an ask is built from, or the answer to send. It is the core's own reading of the checkout's
/// branch and repository and of what GitHub last said; the request names only a pull request number, which must be
/// the one the core last saw for this branch. Nothing here asks GitHub. Blocking.
fn ask_context(
    st: &AppState,
    row: &marshell_protocol::api::SessionInfo,
    reported: Option<PathBuf>,
    cancel: &AtomicBool,
    req: &AskRequest,
) -> Result<AskContext, Box<Response>> {
    let ctx = match prepare(st, row, reported, cancel) {
        Early::Go(ctx) => *ctx,
        Early::Reply(reply) => {
            return Err(Box::new(match reply.kind {
                GithubKind::Off => refused(SWITCHED_OFF),
                GithubKind::NoGh => unavailable(actions::NEED_GH),
                _ => refused(reply.reason.as_deref().unwrap_or(NOT_GITHUB)),
            }));
        }
        // On no branch Marshell can name, GitHub has said nothing about one.
        Early::Headless(_) => return Err(Box::new(refused(HEARD_NOTHING))),
    };
    let checkout = &ctx.target.checkout;
    let Some(stored) = st.github().stored_for_ask(checkout, &ctx.branch) else {
        return Err(Box::new(refused(HEARD_NOTHING)));
    };
    let number = match req {
        AskRequest::FixCheck { number, .. } | AskRequest::SwitchAndPull { number, .. } => Some(*number),
        AskRequest::OpenPull { .. } => None,
    };
    if let Some(number) = number
        && st.github().pull_seen(checkout, &ctx.branch).map(|(_, seen)| seen) != Some(number)
    {
        return Err(Box::new(refused(NOT_THIS_PULL)));
    }
    let repo = if ctx.host == "github.com" {
        format!("{}/{}", ctx.repo.owner, ctx.repo.name)
    } else {
        format!("{}/{}/{}", ctx.host, ctx.repo.owner, ctx.repo.name)
    };
    Ok(AskContext {
        repo,
        base: stored.open_base.unwrap_or_else(|| stored.default_branch.clone()),
        default_branch: stored.default_branch,
        branch: ctx.branch,
        plain_shell: effective_agent(row).is_none(),
    })
}

/// POST /v1/sessions/{tab}/github/ask: types a request about the pull request into the session's agent, in words and
/// with no Enter, or with `copy` answers with the text and types nothing. 404 for a tab that doesn't exist; 409 for a
/// refusal (the session takes no text now, no answer from GitHub yet, another pull request than this branch's, a name
/// that can't be typed). The text is built by `ask_text` from what the core read, never from a name in the request.
pub async fn ask_agent(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<AskRequest>) -> Response {
    let reported = st.session(&tab).and_then(|s| s.current_cwd());
    let Some(row) = row_of(&st, &tab) else {
        return (StatusCode::NOT_FOUND, "There is no such session.").into_response();
    };
    let copy = match &req {
        AskRequest::FixCheck { copy, .. } | AskRequest::OpenPull { copy } | AskRequest::SwitchAndPull { copy, .. } => {
            *copy
        }
    };
    let session = st.session(&tab);
    if !copy && session.is_none() {
        return (StatusCode::NOT_FOUND, "There is no such session.").into_response();
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let _stop = CancelOnDrop(cancel.clone());
    let context = {
        let (state, row, req) = (st.clone(), row.clone(), req.clone());
        match tokio::task::spawn_blocking(move || ask_context(&state, &row, reported, &cancel, &req)).await {
            Ok(Ok(context)) => context,
            Ok(Err(response)) => return *response,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    };
    if copy {
        return match ask_text(&req, &context) {
            Ok(text) => Json(AskReply {
                form: AskForm::Copy,
                text,
            })
            .into_response(),
            Err(sentence) => refused(sentence),
        };
    }
    let Some(session) = session else {
        return (StatusCode::NOT_FOUND, "There is no such session.").into_response();
    };
    // The same 409 reasons as /review. The text is built at once after, so the session has no time to change.
    if let Err(reason) = agent_ready(&st, &tab, &row, &session).await {
        return cannot_resume(reason);
    }
    match typed_text(&req, &context) {
        Ok(text) => {
            type_in(&st, &tab, &session, text.clone().into_bytes());
            Json(AskReply {
                form: AskForm::Typed,
                text,
            })
            .into_response()
        }
        Err(sentence) => refused(sentence),
    }
}
