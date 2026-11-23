//! What a push or a create would send, and the first title and description of a new pull request.

use super::{COMMITS_MAX, Oid, count, plain, valid_branch};
use crate::changes::{Halt, READ_BUDGET, collect};
use crate::gitbox::GitBox;
use marshell_protocol::api::{GithubKind, GithubReply, OutgoingCommit, PullState};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

/// The longest subject shown, in characters.
pub const SUBJECT_MAX: usize = 200;
/// The longest title proposed, in characters.
pub const TITLE_MAX: usize = 256;
/// The longest description proposed, in characters.
pub const BODY_MAX: usize = 65_536;
/// The refusal for a status that isn't Ready and has no sentence of its own.
const UNCHECKED: &str = "Marshell couldn't check this branch with GitHub, so push from the terminal.";
/// The most of `git log`'s answer that is read: a hundred and one records of ordinary length, and then some.
const LOG_MAX: usize = 1024 * 1024;

/// The commits in `from_oid..head_oid`, newest first, at most `COMMITS_MAX`, and how many more there are. Both
/// arguments are commit ids; anything else gives no commits. Blocking.
pub(crate) fn outgoing(git: &GitBox, from_oid: &str, head_oid: &str) -> Result<(Vec<OutgoingCommit>, u32), Halt> {
    let (Some(from), Some(head)) = (Oid::parse(from_oid), Oid::parse(head_oid)) else {
        return Ok((Vec::new(), 0));
    };
    let range = format!("{}..{}", from.as_str(), head.as_str());
    let max = format!("--max-count={}", COMMITS_MAX + 1);
    let cancel = AtomicBool::new(false);
    let (bytes, cut) = collect(
        git,
        &[
            "log",
            "--no-decorate",
            "--no-mailmap",
            "-z",
            "--format=%h%x1f%s",
            &max,
            &range,
        ],
        Instant::now() + READ_BUDGET,
        &cancel,
        LOG_MAX,
    )?;
    let mut records: Vec<&[u8]> = bytes.split(|b| *b == 0).filter(|r| !r.is_empty()).collect();
    // A record cut by the cap is not one.
    if cut {
        records.pop();
    }
    let many = cut || records.len() > COMMITS_MAX;
    let commits: Vec<OutgoingCommit> = records
        .iter()
        .take(COMMITS_MAX)
        .filter_map(|record| {
            let text = String::from_utf8_lossy(record);
            let (id, subject) = text.split_once('\u{1f}')?;
            let hex = (4..=40).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit());
            hex.then(|| OutgoingCommit {
                id: id.to_string(),
                subject: plain(subject, SUBJECT_MAX),
            })
        })
        .collect();
    let more = if many {
        let total = count(git, &from, &head)?;
        total.saturating_sub(u32::try_from(commits.len()).unwrap_or(u32::MAX))
    } else {
        0
    };
    Ok((commits, more))
}

/// `text` cut at `max` characters.
fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// The title of a new pull request: the only commit's subject, else the branch name as it is.
pub fn title_of(commits: &[OutgoingCommit], branch: &str) -> String {
    match commits {
        [only] => cut(&only.subject, TITLE_MAX),
        _ => cut(branch, TITLE_MAX),
    }
}

/// The description of a new pull request: the template as it is, else one `- <subject>` line per commit, oldest
/// first. Nothing without either.
pub fn body_of(template: Option<String>, commits: &[OutgoingCommit]) -> String {
    let body = match template {
        Some(template) => template,
        None => commits
            .iter()
            .rev()
            .map(|c| format!("- {}", c.subject))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    cut(&body, BODY_MAX)
}

/// Why Marshell won't push (or create from) this checkout, as a sentence; the first that holds. `detached` is a
/// checkout on no branch.
pub fn refusal(reply: &GithubReply, detached: bool) -> Option<String> {
    if detached {
        return Some("This checkout is on no branch, so there is nothing to push.".to_string());
    }
    let branch = reply.branch.as_deref();
    if branch.is_some_and(|b| !valid_branch(b)) {
        return Some("That isn't a branch name Marshell can push.".to_string());
    }
    let name = branch.unwrap_or_default();
    if reply.kind == GithubKind::DefaultBranch || (branch.is_some() && reply.default_branch.as_deref() == branch) {
        return Some(format!(
            "{name} is the default branch on GitHub. Marshell doesn't push it."
        ));
    }
    if let Some(blocked) = &reply.push_blocked {
        return Some(blocked.clone());
    }
    if reply.github_ahead {
        return Some(format!(
            "GitHub's {name} has commits this checkout doesn't. Pull them in the terminal."
        ));
    }
    if reply.kind != GithubKind::Ready {
        // Without GitHub's answer the default branch isn't known, so there is always a sentence.
        return Some(reply.reason.clone().unwrap_or_else(|| UNCHECKED.to_string()));
    }
    None
}

/// `refusal`, and with `create` an open pull request too.
pub fn refusal_for(reply: &GithubReply, detached: bool, create: bool) -> Option<String> {
    refusal(reply, detached).or_else(|| {
        let open = reply
            .pull
            .as_ref()
            .filter(|p| matches!(p.state, PullState::Open | PullState::Draft))?;
        create.then(|| format!("This branch already has pull request #{}.", open.number))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(id: &str, subject: &str) -> OutgoingCommit {
        OutgoingCommit {
            id: id.to_string(),
            subject: subject.to_string(),
        }
    }

    #[test]
    fn the_title_is_the_only_subject_else_the_branch() {
        assert_eq!(title_of(&[commit("a1", "Add login")], "feat/login"), "Add login");
        assert_eq!(title_of(&[], "feat/login"), "feat/login");
        let two = [commit("a1", "Two"), commit("b2", "One")];
        assert_eq!(title_of(&two, "feat/login"), "feat/login");
        assert_eq!(title_of(&[], &"x".repeat(300)).chars().count(), TITLE_MAX);
    }

    #[test]
    fn the_body_is_the_template_as_it_is_else_the_subjects_oldest_first() {
        let two = [commit("a1", "Second"), commit("b2", "First")];
        assert_eq!(body_of(None, &two), "- First\n- Second");
        assert_eq!(body_of(None, &[]), "");
        assert_eq!(body_of(Some("## T\r\n".to_string()), &two), "## T\r\n");
        assert_eq!(body_of(Some("y".repeat(BODY_MAX + 5)), &[]).chars().count(), BODY_MAX);
    }

    fn reply(kind: GithubKind, reason: Option<&str>) -> GithubReply {
        GithubReply {
            kind,
            checkout: String::new(),
            host: "github.com".to_string(),
            repo: "octo-org/widgets".to_string(),
            branch: Some("feat/login".to_string()),
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
            reason: reason.map(str::to_string),
            next_ms: 30_000,
        }
    }

    /// Without GitHub's answer the default branch isn't known, so nothing is pushed, whether or not the reply
    /// carries a sentence of its own.
    #[test]
    fn a_status_that_is_not_ready_always_refuses() {
        assert_eq!(refusal(&reply(GithubKind::Ready, None), false), None);
        assert_eq!(
            refusal(&reply(GithubKind::Unreachable, Some("GitHub didn't answer.")), false).as_deref(),
            Some("GitHub didn't answer.")
        );
        for kind in [
            GithubKind::Off,
            GithubKind::None,
            GithubKind::NoGh,
            GithubKind::SignedOut,
            GithubKind::NotFound,
            GithubKind::RateLimited,
            GithubKind::Unreachable,
        ] {
            assert_eq!(
                refusal(&reply(kind, None), false).as_deref(),
                Some("Marshell couldn't check this branch with GitHub, so push from the terminal."),
                "{kind:?}"
            );
            assert!(refusal_for(&reply(kind, None), false, true).is_some(), "{kind:?}");
        }
    }

    #[test]
    fn outgoing_takes_commit_ids_only() {
        // A name, or text git would read as an option, never reaches git: no repository is needed to say no.
        assert!(Oid::parse("main").is_none());
        assert!(Oid::parse("--all").is_none());
    }
}
