//! The calls that write to the person's GitHub account: create a pull request, merge one (now, or when its checks
//! pass), mark it ready or a draft, and cancel its auto-merge. All through `gh api`, in `gh.rs`.
//!
//! Nothing here asks the webview which repository to act on: the caller hands in the repository of the checkout's own
//! `origin`, and every API path is written from it (`repos/{owner}/{name}/…`, never gh's `{owner}`/`{repo}`
//! placeholders). Every value from a request is checked before gh is run; values go to GraphQL only as variables,
//! never into the text of a mutation.

use super::gh::{self, GhError};
use super::plan::{BODY_MAX, TITLE_MAX};
use super::remote::RemoteRepo;
use super::{plain, valid_branch};
use crate::gitbox::refs::Oid;
use marshell_protocol::api::{CreatePull, CreatedPull, MergeMethod, PullAction, PullActionKind, PullMergeRequest};
use serde_json::{Value, json};
use std::path::Path;

/// Pull requests need the GitHub CLI.
pub const NEED_GH: &str = "Pull requests need the GitHub CLI.";
const NEED_SIGN_IN: &str = "Pull requests need a GitHub sign-in. Run `gh auth login` in a terminal.";
const TOO_LONG: &str = "GitHub took too long to answer.";
const UNREADABLE: &str = "GitHub's answer couldn't be read.";
const RATE_LIMITED: &str = "GitHub's limit for this sign-in is used up.";

/// The most of GitHub's own sentence that is shown.
const MESSAGE_MAX: usize = 300;

const ENABLE_AUTO_MERGE: &str = "mutation($id:ID!,$method:PullRequestMergeMethod!,$oid:GitObjectID!){enablePullRequestAutoMerge(input:{pullRequestId:$id,mergeMethod:$method,expectedHeadOid:$oid}){clientMutationId}}";
const MARK_READY: &str =
    "mutation($id:ID!){markPullRequestReadyForReview(input:{pullRequestId:$id}){clientMutationId}}";
const CONVERT_TO_DRAFT: &str =
    "mutation($id:ID!){convertPullRequestToDraft(input:{pullRequestId:$id}){clientMutationId}}";
const DISABLE_AUTO_MERGE: &str =
    "mutation($id:ID!){disablePullRequestAutoMerge(input:{pullRequestId:$id}){clientMutationId}}";

/// Why a write didn't happen, once it was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// GitHub understood and said no: 409, with its sentence.
    Refused(String),
    /// gh or GitHub couldn't be used now: 503, with a sentence.
    Unavailable(String),
    /// gh isn't there any more (503, and the caller forgets where it was).
    NoGh,
}

impl Rejection {
    /// The sentence shown to the person.
    pub fn reason(&self) -> &str {
        match self {
            Rejection::Refused(s) | Rejection::Unavailable(s) => s,
            Rejection::NoGh => NEED_GH,
        }
    }
}

/// Whether `id` is a GitHub node id as far as Marshell sends one: `^[A-Za-z0-9_=-]{1,100}$`.
pub fn valid_node_id(id: &str) -> bool {
    (1..=100).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'=' | b'-'))
}

/// Whether a pull request's description is only text: no control character but a line break and a tab.
fn plain_body(body: &str) -> bool {
    body.chars().all(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
}

/// The sentence for a create request that is malformed, or `None` when it is fine.
pub fn check_create(req: &CreatePull) -> Option<&'static str> {
    let title_len = req.title.chars().count();
    if title_len == 0 || title_len > TITLE_MAX || plain(&req.title, TITLE_MAX) != req.title {
        return Some("The title must be 1 to 256 characters of plain text.");
    }
    if req.body.chars().count() > BODY_MAX || !plain_body(&req.body) {
        return Some("The description is too long or holds a control character.");
    }
    if !valid_branch(&req.base) || !valid_branch(&req.branch) {
        return Some("That isn't a branch name Marshell can send.");
    }
    if Oid::parse(&req.head).is_none() {
        return Some("That isn't a commit id.");
    }
    None
}

/// The sentence for a merge request that is malformed, or `None` when it is fine.
pub fn check_merge(req: &PullMergeRequest) -> Option<&'static str> {
    if !valid_node_id(&req.id) {
        return Some("That isn't a pull request id.");
    }
    if Oid::parse(&req.head).is_none() {
        return Some("That isn't a commit id.");
    }
    if req.number < 1 {
        return Some("That isn't a pull request number.");
    }
    None
}

/// The sentence for a pull request action that is malformed, or `None` when it is fine.
pub fn check_pull(req: &PullAction) -> Option<&'static str> {
    if !valid_node_id(&req.id) {
        return Some("That isn't a pull request id.");
    }
    if req.number < 1 {
        return Some("That isn't a pull request number.");
    }
    None
}

/// What a failed gh call is to the caller: GitHub's own sentence for a refusal, or one of Marshell's.
fn rejection(error: GhError, repo: &RemoteRepo) -> Rejection {
    match error {
        GhError::Missing => Rejection::NoGh,
        GhError::SignedOut => Rejection::Unavailable(NEED_SIGN_IN.to_string()),
        GhError::Unreachable(sentence) => {
            Rejection::Unavailable(format!("Can't reach GitHub: {}", plain(&sentence, MESSAGE_MAX)))
        }
        GhError::NotFound => Rejection::Refused(format!(
            "GitHub has no repository {}/{} that this sign-in can see.",
            repo.owner, repo.name
        )),
        GhError::RateLimited { .. } => Rejection::Unavailable(RATE_LIMITED.to_string()),
        GhError::Refused { message, .. } => Rejection::Refused(plain(&message, MESSAGE_MAX)),
        GhError::TooLong => Rejection::Unavailable(TOO_LONG.to_string()),
        GhError::Unreadable => Rejection::Unavailable(UNREADABLE.to_string()),
    }
}

/// Where the repository's pull requests are, written from its owner and name.
fn pulls_path(repo: &RemoteRepo) -> String {
    format!("repos/{}/{}/pulls", repo.owner, repo.name)
}

/// The merge method as REST writes it.
fn method_rest(method: MergeMethod) -> &'static str {
    match method {
        MergeMethod::Merge => "merge",
        MergeMethod::Squash => "squash",
        MergeMethod::Rebase => "rebase",
    }
}

/// The merge method as GraphQL writes it.
fn method_graphql(method: MergeMethod) -> &'static str {
    match method {
        MergeMethod::Merge => "MERGE",
        MergeMethod::Squash => "SQUASH",
        MergeMethod::Rebase => "REBASE",
    }
}

/// One GraphQL mutation with its values as variables. A 200 answer with `errors` is a failure (`gh::api` maps it).
fn mutate(
    gh: &Path,
    home: &Path,
    host: &str,
    repo: &RemoteRepo,
    query: &str,
    variables: Value,
) -> Result<(), Rejection> {
    let body = json!({ "query": query, "variables": variables });
    gh::api(gh, home, host, &["graphql"], Some(&body))
        .map(|_| ())
        .map_err(|e| rejection(e, repo))
}

/// Opens a pull request from `req.branch` (the checkout's own branch, in this repository) into `req.base`. The
/// request is already checked (`check_create`) and compared with the checkout.
pub fn create(
    gh: &Path,
    home: &Path,
    host: &str,
    repo: &RemoteRepo,
    req: &CreatePull,
) -> Result<CreatedPull, Rejection> {
    let path = pulls_path(repo);
    let body = json!({
        "title": req.title,
        "head": req.branch,
        "base": req.base,
        "body": req.body,
        "draft": req.draft,
    });
    let answer = gh::api(gh, home, host, &[&path], Some(&body)).map_err(|e| rejection(e, repo))?;
    let number = answer
        .get("number")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n >= 1)
        .ok_or_else(|| Rejection::Unavailable(UNREADABLE.to_string()))?;
    // The address is shown and opened: only one that is plainly a web address is kept.
    let url = answer
        .get("html_url")
        .and_then(Value::as_str)
        .filter(|u| u.starts_with("https://") && !u.chars().any(|c| c.is_control() || c.is_whitespace()))
        .unwrap_or("")
        .to_string();
    Ok(CreatedPull { number, url })
}

/// Merges the pull request now, or turns on its auto-merge. GitHub refuses when the branch is no longer at `req.head`.
pub fn merge(gh: &Path, home: &Path, host: &str, repo: &RemoteRepo, req: &PullMergeRequest) -> Result<(), Rejection> {
    if req.auto {
        return mutate(
            gh,
            home,
            host,
            repo,
            ENABLE_AUTO_MERGE,
            json!({ "id": req.id, "method": method_graphql(req.method), "oid": req.head }),
        );
    }
    let path = format!("{}/{}/merge", pulls_path(repo), req.number);
    let body = json!({ "merge_method": method_rest(req.method), "sha": req.head });
    gh::api(gh, home, host, &["-X", "PUT", &path], Some(&body))
        .map(|_| ())
        .map_err(|e| rejection(e, repo))
}

/// Marks the pull request ready for review, converts it to a draft, or cancels its auto-merge.
pub fn pull_action(gh: &Path, home: &Path, host: &str, repo: &RemoteRepo, req: &PullAction) -> Result<(), Rejection> {
    let query = match req.action {
        PullActionKind::Ready => MARK_READY,
        PullActionKind::Draft => CONVERT_TO_DRAFT,
        PullActionKind::CancelAutoMerge => DISABLE_AUTO_MERGE,
    };
    mutate(gh, home, host, repo, query, json!({ "id": req.id }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widgets() -> RemoteRepo {
        RemoteRepo {
            host: "github.com".to_string(),
            owner: "octo-org".to_string(),
            name: "widgets".to_string(),
        }
    }

    #[test]
    fn a_node_id_is_letters_digits_and_three_marks() {
        for good in ["PR_kwDOAAAAAA", "MDExOlB1bGxSZXF1ZXN0MQ==", "a", "a-b_c"] {
            assert!(valid_node_id(good), "{good}");
        }
        let long = "A".repeat(101);
        for bad in [
            "",
            "PR x",
            "PR\"x",
            "PR'x",
            "a}b",
            "a\n",
            "\u{e9}",
            "a.b",
            long.as_str(),
        ] {
            assert!(!valid_node_id(bad), "{bad:?}");
        }
        assert!(valid_node_id(&"A".repeat(100)));
    }

    #[test]
    fn a_description_holds_a_line_break_and_a_tab_and_no_other_control() {
        assert!(plain_body("a\r\nb\tc\n"));
        assert!(!plain_body("a\u{0}b"));
        assert!(!plain_body("a\u{1b}[0m"));
        assert!(!plain_body("a\u{7f}"));
    }

    #[test]
    fn failures_read_as_the_brief_says() {
        let repo = widgets();
        let cases = [
            (GhError::Missing, Rejection::NoGh),
            (GhError::SignedOut, Rejection::Unavailable(NEED_SIGN_IN.to_string())),
            (
                GhError::Unreachable("no route\u{7}".to_string()),
                Rejection::Unavailable("Can't reach GitHub: no route ".to_string()),
            ),
            (
                GhError::NotFound,
                Rejection::Refused("GitHub has no repository octo-org/widgets that this sign-in can see.".to_string()),
            ),
            (
                GhError::Refused {
                    status: Some(405),
                    message: "Pull Request has merge conflicts\n".to_string(),
                },
                Rejection::Refused("Pull Request has merge conflicts ".to_string()),
            ),
            (GhError::TooLong, Rejection::Unavailable(TOO_LONG.to_string())),
            (GhError::Unreadable, Rejection::Unavailable(UNREADABLE.to_string())),
            (
                GhError::RateLimited { reset_ms: None },
                Rejection::Unavailable(RATE_LIMITED.to_string()),
            ),
        ];
        for (error, want) in cases {
            assert_eq!(rejection(error, &repo), want);
        }
    }

    #[test]
    fn the_mutations_take_their_values_as_variables() {
        for text in [ENABLE_AUTO_MERGE, MARK_READY, CONVERT_TO_DRAFT, DISABLE_AUTO_MERGE] {
            assert!(text.starts_with("mutation($id:ID!"), "{text}");
            assert!(!text.contains('"'), "{text}");
        }
        assert_eq!(pulls_path(&widgets()), "repos/octo-org/widgets/pulls");
    }
}
