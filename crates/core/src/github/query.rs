//! The one GraphQL query the pull request bar asks, and the shape of GitHub's answer to it.
//!
//! Every field of the answer is optional and defaulted, so an answer that lacks one (or holds an enum
//! string this build doesn't know, which is read as text) never fails the whole snapshot.

use serde::Deserialize;

use super::remote::RemoteRepo;

/// Asked with `gh api graphql`: the repository's permissions and merge settings, the branch's head commit, and
/// the 5 newest pull requests from the branch with the checks of their last commit.
pub const STATUS_QUERY: &str = "query($o:String!,$n:String!,$b:String!,$h:String!){rateLimit{remaining resetAt} repository(owner:$o,name:$n){viewerPermission viewerDefaultMergeMethod mergeCommitAllowed squashMergeAllowed rebaseMergeAllowed autoMergeAllowed isFork parent{nameWithOwner defaultBranchRef{name}} defaultBranchRef{name} ref(qualifiedName:$b){target{oid}} pullRequests(headRefName:$h,first:5,orderBy:{field:CREATED_AT,direction:DESC}){nodes{id number title url state isDraft isCrossRepository headRefOid baseRefName mergeable mergeStateStatus reviewDecision viewerCanEnableAutoMerge autoMergeRequest{mergeMethod} commits(last:1){nodes{commit{statusCheckRollup{contexts(first:100){totalCount nodes{__typename ... on CheckRun{name status conclusion detailsUrl startedAt completedAt checkSuite{workflowRun{databaseId workflow{name}}}} ... on StatusContext{context state targetUrl createdAt}}}}}}}}}}}";

/// The arguments of `gh api` for `STATUS_QUERY`. The branch travels as a value of its own (`-f`), never
/// inside the query text.
pub fn status_args(repo: &RemoteRepo, branch: &str) -> Vec<String> {
    vec![
        "graphql".to_string(),
        "-f".to_string(),
        format!("query={STATUS_QUERY}"),
        "-f".to_string(),
        format!("o={}", repo.owner),
        "-f".to_string(),
        format!("n={}", repo.name),
        "-f".to_string(),
        format!("b=refs/heads/{branch}"),
        "-f".to_string(),
        format!("h={branch}"),
    ]
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Answer {
    pub data: Option<Data>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Data {
    pub repository: Option<Repository>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Repository {
    pub viewer_permission: Option<String>,
    pub viewer_default_merge_method: Option<String>,
    pub merge_commit_allowed: bool,
    pub squash_merge_allowed: bool,
    pub rebase_merge_allowed: bool,
    pub auto_merge_allowed: bool,
    pub is_fork: bool,
    pub parent: Option<Parent>,
    pub default_branch_ref: Option<BranchRef>,
    pub r#ref: Option<GitRef>,
    pub pull_requests: Option<Pulls>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Parent {
    pub name_with_owner: Option<String>,
    pub default_branch_ref: Option<BranchRef>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct BranchRef {
    pub name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct GitRef {
    pub target: Option<Target>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Target {
    pub oid: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Pulls {
    pub nodes: Vec<Option<Pull>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Pull {
    pub id: Option<String>,
    pub number: Option<u32>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub state: Option<String>,
    pub is_draft: bool,
    pub is_cross_repository: bool,
    pub head_ref_oid: Option<String>,
    pub base_ref_name: Option<String>,
    pub mergeable: Option<String>,
    pub merge_state_status: Option<String>,
    pub review_decision: Option<String>,
    pub viewer_can_enable_auto_merge: bool,
    pub auto_merge_request: Option<AutoMerge>,
    pub commits: Option<Commits>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AutoMerge {
    pub merge_method: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Commits {
    pub nodes: Vec<Option<CommitNode>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CommitNode {
    pub commit: Option<Commit>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Commit {
    pub status_check_rollup: Option<Rollup>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Rollup {
    pub contexts: Option<Contexts>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Contexts {
    pub total_count: u64,
    pub nodes: Vec<Option<Context>>,
}

/// A `CheckRun` or a `StatusContext`, told apart by `__typename`; the fields of the other kind stay `None`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Context {
    #[serde(rename = "__typename")]
    pub typename: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub conclusion: Option<String>,
    pub details_url: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub check_suite: Option<CheckSuite>,
    pub context: Option<String>,
    pub state: Option<String>,
    pub target_url: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CheckSuite {
    pub workflow_run: Option<WorkflowRun>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkflowRun {
    pub database_id: Option<u64>,
    pub workflow: Option<Workflow>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Workflow {
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_args_is_exact() {
        let repo = RemoteRepo {
            host: "github.com".into(),
            owner: "octo-org".into(),
            name: "widgets".into(),
        };
        let args = status_args(&repo, "feat/login");
        assert_eq!(
            args,
            [
                "graphql".to_string(),
                "-f".into(),
                format!("query={STATUS_QUERY}"),
                "-f".into(),
                "o=octo-org".into(),
                "-f".into(),
                "n=widgets".into(),
                "-f".into(),
                "b=refs/heads/feat/login".into(),
                "-f".into(),
                "h=feat/login".into(),
            ]
        );
        assert!(!STATUS_QUERY.contains('\n'));
        assert!(STATUS_QUERY.starts_with("query($o:String!,$n:String!,$b:String!,$h:String!){rateLimit{"));
    }
}
