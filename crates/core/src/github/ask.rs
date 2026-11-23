//! The three requests the Changes panel types into the agent's prompt (or hands over to copy): fix a failed check,
//! open the pull request, switch back to the default branch and pull.
//!
//! The text holds names that come from GitHub and from git, lands in an agent's prompt and may hold a command, so it
//! is built only here, from the request's few fields and from what the core read itself (`AskContext`).

use super::{command_safe, plain};
use marshell_protocol::api::AskRequest;

/// What the text needs that a request can't supply: the core reads it from the checkout and from GitHub's last answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskContext {
    /// `owner/name` on github.com, `host/owner/name` on another host.
    pub repo: String,
    pub branch: String,
    /// The base of the branch's open pull request, else the default branch.
    pub base: String,
    pub default_branch: String,
    /// The session runs no agent: the text is copied to a shell.
    pub plain_shell: bool,
}

/// The refusal for a check whose name is empty once cleaned.
pub const NO_CHECK_NAME: &str = "That check has no name Marshell can type.";
/// The refusal for a plain-shell copy of a default branch that can't go in a command.
pub const NO_COMMAND_NAME: &str = "The default branch's name can't be put in a command safely.";
/// The refusal for a branch or base with no name left once cleaned.
pub const NO_BRANCH_NAME: &str = "That branch has no name Marshell can type.";

/// The longest name put in prose.
const NAME_MAX: usize = 200;

/// A name from GitHub or git as prose in a one-line request: `plain` (no control or hidden character, at most
/// `NAME_MAX` characters), then every backtick and double quote a space (so it can neither close a command nor the
/// quotes around it), then trimmed. The one helper for every name put in prose.
pub fn prose(name: &str) -> String {
    plain(name, NAME_MAX)
        .chars()
        .map(|c| if matches!(c, '`' | '"') { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

/// The host a repository is named on: `github.com` for `owner/name`, the first part of `host/owner/name`.
fn host_of(repo: &str) -> &str {
    let mut parts = repo.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(host), Some(_), Some(_)) => host,
        _ => "github.com",
    }
}

/// A check's link, when it is on the repository's host and holds nothing that could end a sentence early, a quote,
/// or a line: it starts with `https://{host}/` (the slash is part of the prefix, so `github.com.evil.example` isn't
/// `github.com`), and has no space, quote, backtick, control or hidden character.
fn usable_link<'a>(url: Option<&'a str>, host: &str) -> Option<&'a str> {
    let url = url?;
    let on_host = url
        .strip_prefix("https://")
        .and_then(|rest| rest.strip_prefix(host))
        .is_some_and(|rest| rest.starts_with('/'));
    let clean = !url
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '`'))
        && plain(url, usize::MAX) == url;
    (on_host && clean).then_some(url)
}

/// The text for `ask`, or the sentence that says why there is none.
pub fn ask_text(ask: &AskRequest, ctx: &AskContext) -> Result<String, &'static str> {
    match ask {
        AskRequest::FixCheck {
            number,
            name,
            workflow,
            url,
            run_id,
            ..
        } => {
            let name = prose(name);
            if name.is_empty() {
                return Err(NO_CHECK_NAME);
            }
            let link = usable_link(url.as_deref(), host_of(&ctx.repo));
            // A run id is a number; 0 is none. The command also needs the repository to be safe in one.
            match run_id.filter(|id| *id > 0).filter(|_| command_safe(&ctx.repo)) {
                Some(id) => {
                    let workflow = Some(prose(workflow.as_deref().unwrap_or_default()))
                        .filter(|w| !w.is_empty())
                        .map(|w| format!(" ({w})"))
                        .unwrap_or_default();
                    let link = link.map(|url| format!(": {url}")).unwrap_or_default();
                    Ok(format!(
                        "The check \"{name}\"{workflow} failed on pull request #{number}{link}. Read its log with `gh run view {id} --repo {} --log-failed`, find the cause and fix it.",
                        ctx.repo
                    ))
                }
                None => {
                    let details = link
                        .map(|url| format!(" Its details are at {url}."))
                        .unwrap_or_default();
                    Ok(format!(
                        "The check \"{name}\" failed on pull request #{number}.{details} Find the cause and fix it."
                    ))
                }
            }
        }
        AskRequest::OpenPull { .. } => {
            let (branch, base) = (prose(&ctx.branch), prose(&ctx.base));
            if branch.is_empty() || base.is_empty() {
                return Err(NO_BRANCH_NAME);
            }
            Ok(format!(
                "Open a pull request on GitHub for the branch {branch} into {base}: push the branch if it isn't pushed yet, then create the pull request with a title and a description that sum up its commits."
            ))
        }
        AskRequest::SwitchAndPull { number, .. } => {
            let default = &ctx.default_branch;
            match (command_safe(default), ctx.plain_shell) {
                (true, true) => Ok(format!("git switch {default}\ngit pull")),
                (true, false) => Ok(format!(
                    "Pull request #{number} is merged. Switch this checkout to {default} and pull it: run `git switch {default}`, then `git pull`. If there are uncommitted changes, ask me first."
                )),
                (false, true) => Err(NO_COMMAND_NAME),
                (false, false) => Ok(format!(
                    "Pull request #{number} is merged. Switch this checkout to its default branch and pull it. If there are uncommitted changes, ask me first."
                )),
            }
        }
    }
}

/// The text to type into a session: `ask_text` for a session with an agent, whatever `ctx` says of it, so what is
/// typed is one line. The two-line shell form is only ever copied.
pub fn typed_text(ask: &AskRequest, ctx: &AskContext) -> Result<String, &'static str> {
    let ctx = AskContext {
        plain_shell: false,
        ..ctx.clone()
    };
    ask_text(ask, &ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> AskContext {
        AskContext {
            repo: "octo-org/widgets".into(),
            branch: "feat/login".into(),
            base: "main".into(),
            default_branch: "main".into(),
            plain_shell: false,
        }
    }

    fn fix(name: &str, workflow: Option<&str>, url: Option<&str>, run_id: Option<u64>) -> AskRequest {
        AskRequest::FixCheck {
            copy: false,
            number: 7,
            name: name.into(),
            workflow: workflow.map(Into::into),
            url: url.map(Into::into),
            run_id,
        }
    }

    const RUN_URL: &str = "https://github.com/octo-org/widgets/actions/runs/4519306504/job/137756214968";

    /// What is typed lands in a prompt, where a line break would send it: the two-line shell form is only ever
    /// copied, whatever the context says about the session.
    #[test]
    fn typed_text_is_one_line_even_for_a_plain_shell() {
        let shell = AskContext {
            plain_shell: true,
            ..ctx()
        };
        let ask = AskRequest::SwitchAndPull { copy: false, number: 7 };
        assert_eq!(
            ask_text(&ask, &shell).unwrap(),
            "git switch main
git pull"
        );
        let typed = typed_text(&ask, &shell).unwrap();
        assert!(!typed.chars().any(char::is_control), "{typed:?}");
        assert_eq!(typed, ask_text(&ask, &ctx()).unwrap());
    }

    #[test]
    fn fix_text_names_the_check_link_and_log_command() {
        let text = ask_text(&fix("pass", Some("ci"), Some(RUN_URL), Some(4519306504)), &ctx()).unwrap();
        assert_eq!(
            text,
            format!(
                "The check \"pass\" (ci) failed on pull request #7: {RUN_URL}. Read its log with `gh run view 4519306504 --repo octo-org/widgets --log-failed`, find the cause and fix it."
            )
        );
        let text = ask_text(&fix("pass", None, Some(RUN_URL), Some(4519306504)), &ctx()).unwrap();
        assert_eq!(
            text,
            format!(
                "The check \"pass\" failed on pull request #7: {RUN_URL}. Read its log with `gh run view 4519306504 --repo octo-org/widgets --log-failed`, find the cause and fix it."
            )
        );
    }

    #[test]
    fn a_workflow_that_is_empty_once_cleaned_is_left_out() {
        let text = ask_text(&fix("pass", Some(" \u{202e}`\" "), None, Some(5)), &ctx()).unwrap();
        assert_eq!(
            text,
            "The check \"pass\" failed on pull request #7. Read its log with `gh run view 5 --repo octo-org/widgets --log-failed`, find the cause and fix it."
        );
    }

    #[test]
    fn a_check_with_no_run_id_has_no_log_command() {
        let text = ask_text(&fix("pass", Some("ci"), Some(RUN_URL), None), &ctx()).unwrap();
        assert_eq!(
            text,
            format!(
                "The check \"pass\" failed on pull request #7. Its details are at {RUN_URL}. Find the cause and fix it."
            )
        );
        assert!(!text.contains("gh run"));
        // No link either: the middle sentence is left out.
        let text = ask_text(&fix("pass", None, None, None), &ctx()).unwrap();
        assert_eq!(
            text,
            "The check \"pass\" failed on pull request #7. Find the cause and fix it."
        );
        // A run id of 0 is no run id.
        let text = ask_text(&fix("pass", None, None, Some(0)), &ctx()).unwrap();
        assert!(!text.contains("gh run"), "{text}");
    }

    #[test]
    fn a_repository_that_cannot_go_in_a_command_gets_the_text_without_one() {
        for repo in [
            "octo-org/wid gets",
            "octo-org/widgets;calc",
            "octo-org/$(x)",
            "-x/y",
            "",
        ] {
            let c = AskContext {
                repo: repo.into(),
                ..ctx()
            };
            let text = ask_text(&fix("pass", Some("ci"), Some(RUN_URL), Some(9)), &c).unwrap();
            assert!(!text.contains("gh run") && !text.contains('`'), "{repo}: {text}");
            assert!(text.contains(RUN_URL), "{text}");
        }
    }

    #[test]
    fn prose_turns_control_hidden_and_quote_characters_into_spaces() {
        assert_eq!(prose("a\nb"), "a b");
        assert_eq!(prose("a`b\"c"), "a b c");
        assert_eq!(prose("  a  "), "a");
        assert_eq!(prose("`\"\n"), "");
        assert_eq!(prose(&"x".repeat(500)).chars().count(), 200);
        // A cut name has no trailing space from a name that was cut mid-run.
        assert_eq!(prose(&format!("{}`rest", "y".repeat(199))), "y".repeat(199));
    }

    #[test]
    fn control_and_format_characters_in_a_name_become_spaces() {
        let text = ask_text(&fix("a\nb\u{202e}c\u{2028}d\re\u{1b}f", None, None, None), &ctx()).unwrap();
        assert_eq!(
            text,
            "The check \"a b c d e f\" failed on pull request #7. Find the cause and fix it."
        );
        assert!(!text.chars().any(char::is_control));
    }

    #[test]
    fn a_backtick_in_a_name_cannot_close_the_command() {
        let text = ask_text(&fix("x` && calc `\"y", Some("w`f"), None, Some(1)), &ctx()).unwrap();
        assert_eq!(text.matches('`').count(), 2, "{text}");
        assert!(text.contains("\"x  && calc   y\" (w f)"), "{text}");
        let open = AskRequest::OpenPull { copy: false };
        let c = AskContext {
            branch: "a`b".into(),
            base: "c\"d".into(),
            ..ctx()
        };
        let text = ask_text(&open, &c).unwrap();
        assert!(!text.contains('`') && !text.contains('"'), "{text}");
    }

    #[test]
    fn an_empty_name_is_refused() {
        for name in ["", "   ", "\n\r\t", "`\"`", "\u{202e}\u{200b}"] {
            assert_eq!(
                ask_text(&fix(name, None, None, None), &ctx()),
                Err(NO_CHECK_NAME),
                "{name:?}"
            );
        }
    }

    #[test]
    fn open_pull_names_the_branch_and_base() {
        let text = ask_text(&AskRequest::OpenPull { copy: false }, &ctx()).unwrap();
        assert_eq!(
            text,
            "Open a pull request on GitHub for the branch feat/login into main: push the branch if it isn't pushed yet, then create the pull request with a title and a description that sum up its commits."
        );
        let c = AskContext {
            branch: String::new(),
            ..ctx()
        };
        assert_eq!(ask_text(&AskRequest::OpenPull { copy: false }, &c), Err(NO_BRANCH_NAME));
    }

    #[test]
    fn switch_and_pull_names_the_default_branch_in_its_commands() {
        let ask = AskRequest::SwitchAndPull { copy: false, number: 7 };
        assert_eq!(
            ask_text(&ask, &ctx()).unwrap(),
            "Pull request #7 is merged. Switch this checkout to main and pull it: run `git switch main`, then `git pull`. If there are uncommitted changes, ask me first."
        );
        let c = AskContext {
            default_branch: "release/v1.2-x".into(),
            ..ctx()
        };
        assert!(ask_text(&ask, &c).unwrap().contains("`git switch release/v1.2-x`"));
    }

    #[test]
    fn a_shell_character_in_the_default_branch_drops_the_command() {
        let ask = AskRequest::SwitchAndPull { copy: false, number: 7 };
        for default in ["main;calc", "a$b", "a|b", "a&b", "-x", "a b", "a`b`", "", "a\nb"] {
            let c = AskContext {
                default_branch: default.into(),
                ..ctx()
            };
            let text = ask_text(&ask, &c).unwrap();
            assert_eq!(
                text,
                "Pull request #7 is merged. Switch this checkout to its default branch and pull it. If there are uncommitted changes, ask me first.",
                "{default:?}"
            );
            assert!(!text.contains("git switch") && !text.contains(';'), "{text}");
            let shell = AskContext { plain_shell: true, ..c };
            assert_eq!(ask_text(&ask, &shell), Err(NO_COMMAND_NAME), "{default:?}");
        }
    }

    #[test]
    fn a_plain_shell_copy_is_the_two_commands_on_two_lines() {
        let ask = AskRequest::SwitchAndPull { copy: true, number: 7 };
        let c = AskContext {
            plain_shell: true,
            ..ctx()
        };
        assert_eq!(ask_text(&ask, &c).unwrap(), "git switch main\ngit pull");
    }

    #[test]
    fn the_branch_names_come_from_the_core() {
        // A request has no field for them: whatever a caller adds to the body is dropped.
        let body =
            r#"{"kind":"open_pull","copy":false,"branch":"evil","base":"evil","default_branch":"evil","repo":"x/y"}"#;
        let ask: AskRequest = serde_json::from_str(body).unwrap();
        let text = ask_text(&ask, &ctx()).unwrap();
        assert!(
            text.contains("feat/login into main") && !text.contains("evil"),
            "{text}"
        );
        let body = r#"{"kind":"switch_and_pull","copy":false,"number":7,"default_branch":"evil"}"#;
        let ask: AskRequest = serde_json::from_str(body).unwrap();
        let text = ask_text(&ask, &ctx()).unwrap();
        assert!(text.contains("`git switch main`") && !text.contains("evil"), "{text}");
    }

    #[test]
    fn a_link_to_another_host_is_left_out() {
        for url in [
            "https://github.com.evil.example/octo-org/widgets/actions/runs/1",
            "https://github.com@evil.example/x",
            "http://github.com/octo-org/widgets/actions/runs/1",
            "https://github.com",
            "https://evil.example/https://github.com/",
            "//github.com/x",
            "https://GitHub.com/x",
            "",
        ] {
            let text = ask_text(&fix("pass", None, Some(url), None), &ctx()).unwrap();
            assert_eq!(
                text, "The check \"pass\" failed on pull request #7. Find the cause and fix it.",
                "{url}"
            );
            let text = ask_text(&fix("pass", None, Some(url), Some(3)), &ctx()).unwrap();
            assert!(!text.contains("://") || url.is_empty(), "{url}: {text}");
        }
    }

    #[test]
    fn a_link_with_a_space_quote_backtick_or_control_character_is_left_out() {
        for tail in [
            " x",
            "\"x",
            "'x",
            "`x",
            "\nx",
            "\tx",
            "\u{1b}x",
            "\u{202e}x",
            "\u{2028}x",
            "\u{a0}x",
        ] {
            let url = format!("https://github.com/octo-org/widgets{tail}");
            let text = ask_text(&fix("pass", None, Some(&url), None), &ctx()).unwrap();
            assert!(!text.contains("github.com"), "{tail:?}: {text}");
        }
    }

    #[test]
    fn a_run_id_with_an_unusable_link_keeps_the_log_command_and_drops_the_link() {
        let text = ask_text(
            &fix("pass", Some("ci"), Some("https://evil.example/x"), Some(4519306504)),
            &ctx(),
        )
        .unwrap();
        assert_eq!(
            text,
            "The check \"pass\" (ci) failed on pull request #7. Read its log with `gh run view 4519306504 --repo octo-org/widgets --log-failed`, find the cause and fix it."
        );
    }

    #[test]
    fn an_enterprise_repo_is_named_with_its_host() {
        let c = AskContext {
            repo: "ghe.example.com/octo-org/widgets".into(),
            ..ctx()
        };
        let url = "https://ghe.example.com/octo-org/widgets/actions/runs/9";
        let text = ask_text(&fix("pass", None, Some(url), Some(9)), &c).unwrap();
        assert_eq!(
            text,
            format!(
                "The check \"pass\" failed on pull request #7: {url}. Read its log with `gh run view 9 --repo ghe.example.com/octo-org/widgets --log-failed`, find the cause and fix it."
            )
        );
        // The link must be on the bar's host: github.com's is not this repository's.
        let text = ask_text(&fix("pass", None, Some(RUN_URL), Some(9)), &c).unwrap();
        assert!(!text.contains("https://"), "{text}");
        // And on github.com a link to the enterprise host is not.
        let text = ask_text(&fix("pass", None, Some(url), Some(9)), &ctx()).unwrap();
        assert!(!text.contains("https://"), "{text}");
    }

    /// Every field of every request kind holds something hostile: no text has a control or hidden character, and
    /// no backtick but the ones of the template's own commands.
    #[test]
    fn hostile_values_in_any_field_never_make_a_multi_line_or_a_stray_backtick() {
        let hostile = [
            "a\nb",
            "a\r\nb",
            "a`b",
            "``",
            "a\"b",
            "a\u{202e}b",
            "a\u{2028}b\u{2029}",
            "a\u{200b}b\u{feff}",
            "a\u{1b}[31mb",
            "a\0b",
            "a\u{85}b",
            "$(calc)",
            "x; calc",
            "a\u{e0041}b",
            "\u{7f}",
        ];
        let without_template_spans = |text: &str, c: &AskContext| -> String {
            let mut rest = text.to_string();
            for span in [
                format!("`gh run view 42 --repo {} --log-failed`", c.repo),
                format!("`git switch {}`", c.default_branch),
                "`git pull`".to_string(),
            ] {
                rest = rest.replace(&span, "");
            }
            rest
        };
        let check = |text: String, c: &AskContext, two_lines: bool| {
            let line = if two_lines {
                text.replace('\n', " ")
            } else {
                text.clone()
            };
            assert!(!line.chars().any(char::is_control), "{text:?}");
            assert_eq!(plain(&line, usize::MAX), line, "{text:?}");
            assert!(!without_template_spans(&text, c).contains('`'), "{text:?}");
        };
        for h in hostile {
            let urls = [
                format!("https://github.com/octo-org/{h}"),
                format!("{h}https://github.com/x"),
                h.to_string(),
            ];
            let contexts = [
                ctx(),
                AskContext {
                    repo: format!("octo-org/{h}"),
                    ..ctx()
                },
                AskContext {
                    branch: h.into(),
                    base: h.into(),
                    default_branch: h.into(),
                    ..ctx()
                },
                AskContext {
                    default_branch: h.into(),
                    plain_shell: true,
                    ..ctx()
                },
            ];
            for c in &contexts {
                for url in &urls {
                    for (workflow, run_id) in [(Some(h), Some(42)), (Some(h), None), (None, Some(42))] {
                        let ask = fix(&format!("n{h}"), workflow, Some(url), run_id);
                        if let Ok(text) = ask_text(&ask, c) {
                            check(text, c, false);
                        }
                    }
                }
                for ask in [
                    AskRequest::OpenPull { copy: false },
                    AskRequest::SwitchAndPull { copy: false, number: 3 },
                    AskRequest::SwitchAndPull { copy: true, number: 3 },
                ] {
                    if let Ok(text) = ask_text(&ask, c) {
                        // Only the plain-shell copy of a switch is two lines, and it is never typed.
                        let two_lines = c.plain_shell && matches!(ask, AskRequest::SwitchAndPull { .. });
                        check(text, c, two_lines);
                    }
                }
            }
        }
    }
}
