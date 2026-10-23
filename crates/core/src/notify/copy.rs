//! The words of a banner, ported from `src/features/notifications/banner.ts`. A command that a person
//! must look at is never in a banner: a risky one reads "Needs a careful look." and one that does not
//! fit on a line is not shown.

use super::{Banner, BannerAction};
use marshell_protocol::api::SessionInfo;

/// The longest permission phrase a banner shows; longer, or with a line break, it says it does not fit.
const PHRASE_MAX: usize = 160;

/// What the words need of a session, taken when the event happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Who {
    pub tab_id: String,
    pub number: u32,
    pub name: String,
    /// The project folder's name; empty when the session has none.
    pub project: String,
}

impl Who {
    pub fn of(info: &SessionInfo) -> Who {
        let project = info
            .project
            .trim_end_matches(['/', '\\'])
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("");
        Who {
            tab_id: info.tab_id.clone(),
            number: info.number,
            name: info.name.clone(),
            project: project.to_string(),
        }
    }

    /// "Session 3 · my-app", or "Session 3" with no project.
    pub fn meta(&self) -> String {
        if self.project.is_empty() {
            format!("Session {}", self.number)
        } else {
            format!("Session {} · {}", self.number, self.project)
        }
    }

    fn banner(&self, title: String, body: String, action: BannerAction) -> Banner {
        Banner {
            title,
            body,
            meta: self.meta(),
            action,
            tab_id: Some(self.tab_id.clone()),
        }
    }
}

/// The kinds of event that can fold into one banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    NeedsYou,
    Done,
    Error,
}

/// A session that needs a permission or an answer. `phrase` is the row's line 2.
pub fn needs_you(who: &Who, phrase: &str, risky: bool) -> Banner {
    let title = format!("{} needs you", who.name);
    if risky {
        return who.banner(title, "Needs a careful look.".into(), BannerAction::Review);
    }
    let body = if phrase.chars().count() > PHRASE_MAX || phrase.contains(['\r', '\n']) {
        "Wants to run a command that does not fit here.".to_string()
    } else if phrase.is_empty() {
        "Needs you".to_string()
    } else {
        phrase.to_string()
    };
    who.banner(title, body, BannerAction::Open)
}

/// A finished turn; `recap` is what the agent last said.
pub fn done(who: &Who, recap: Option<&str>) -> Banner {
    let line = recap
        .and_then(|r| r.lines().map(str::trim).find(|l| !l.is_empty()))
        .unwrap_or("Done");
    who.banner(format!("{} is done", who.name), line.to_string(), BannerAction::Open)
}

/// A session that stopped on an error; `phrase` is why.
pub fn error(who: &Who, phrase: &str) -> Banner {
    let why = phrase.trim().trim_end_matches('.');
    let why = if why.is_empty() { "an error" } else { why };
    who.banner(
        format!("{} stopped", who.name),
        format!("Stopped: {why}."),
        BannerAction::Open,
    )
}

/// The one reminder for a wait that has gone on: the needs-you title, and how long it has been.
pub fn escalation(who: &Who, minutes: u64, permission: bool, risky: bool) -> Banner {
    let what = if permission {
        "to run a command"
    } else {
        "for an answer"
    };
    who.banner(
        format!("{} needs you", who.name),
        format!("{} has waited {minutes} min {what}", who.name),
        if risky {
            BannerAction::Review
        } else {
            BannerAction::Open
        },
    )
}

/// Several sessions in one banner. A click opens the window only; any risky request makes it Review.
pub fn batch(kind: Kind, items: &[(Who, bool)]) -> Banner {
    let k = items.len();
    let title = match kind {
        Kind::NeedsYou => format!("{k} sessions need you"),
        Kind::Done => format!("{k} sessions are done"),
        Kind::Error => format!("{k} sessions stopped"),
    };
    let named: Vec<String> = items.iter().map(|(w, _)| format!("{} {}", w.number, w.name)).collect();
    let list = match named.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => named.join(""),
    };
    Banner {
        title,
        body: format!("Sessions {list}"),
        meta: format!("{k} sessions"),
        action: if items.iter().any(|(_, risky)| *risky) {
            BannerAction::Review
        } else {
            BannerAction::Open
        },
        tab_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notify::BannerAction;

    fn who(tab: &str, number: u32, name: &str, project: &str) -> Who {
        Who {
            tab_id: tab.into(),
            number,
            name: name.into(),
            project: project.into(),
        }
    }

    fn api() -> Who {
        who("t3", 3, "api-server", "my-app")
    }

    #[test]
    fn a_permission_request_shows_the_row_phrase() {
        let b = needs_you(&api(), "Wants to run `npm test`", false);
        assert_eq!(b.title, "api-server needs you");
        assert_eq!(b.body, "Wants to run `npm test`");
        assert_eq!(b.meta, "Session 3 · my-app");
        assert_eq!(b.action, BannerAction::Open);
        assert_eq!(b.tab_id.as_deref(), Some("t3"));
    }

    #[test]
    fn a_question_says_so() {
        assert_eq!(needs_you(&api(), "Has a question", false).body, "Has a question");
    }

    #[test]
    fn a_risky_request_never_shows_its_command() {
        let b = needs_you(&api(), "Wants to run `rm -rf node_modules`", true);
        assert_eq!(b.body, "Needs a careful look.");
        assert_eq!(b.action, BannerAction::Review);
        assert!(!format!("{} {} {}", b.title, b.body, b.meta).contains("rm -rf"));
    }

    #[test]
    fn a_permission_phrase_that_does_not_fit_is_not_shown() {
        let fits = format!("Wants to run `{}`", "x".repeat(160 - "Wants to run ``".len()));
        assert_eq!(fits.chars().count(), 160);
        assert_eq!(needs_you(&api(), &fits, false).body, fits);
        let long = format!("{fits}x");
        assert_eq!(
            needs_you(&api(), &long, false).body,
            "Wants to run a command that does not fit here."
        );
        assert_eq!(
            needs_you(&api(), "Wants to run `cd app\nrm file`", false).body,
            "Wants to run a command that does not fit here."
        );
    }

    #[test]
    fn done_shows_the_first_line_of_the_recap() {
        let b = done(&api(), Some("All tests pass.\nI also fixed the lint."));
        assert_eq!(b.title, "api-server is done");
        assert_eq!(b.body, "All tests pass.");
        assert_eq!(b.action, BannerAction::Open);
        assert_eq!(b.tab_id.as_deref(), Some("t3"));
    }

    #[test]
    fn done_without_a_recap_says_done() {
        assert_eq!(done(&api(), None).body, "Done");
        assert_eq!(done(&api(), Some("  \n ")).body, "Done");
    }

    #[test]
    fn an_error_says_what_stopped_it() {
        let b = error(&api(), "rate limit reached");
        assert_eq!(b.title, "api-server stopped");
        assert_eq!(b.body, "Stopped: rate limit reached.");
        assert_eq!(error(&api(), "").body, "Stopped: an error.");
        assert_eq!(error(&api(), "out of credit.").body, "Stopped: out of credit.");
    }

    #[test]
    fn the_meta_line_names_the_project_when_there_is_one() {
        assert_eq!(api().meta(), "Session 3 · my-app");
        assert_eq!(who("t", 4, "x", "").meta(), "Session 4");
    }

    #[test]
    fn the_project_is_the_folders_name() {
        let info = |project: &str| marshell_protocol::api::SessionInfo {
            tab_id: "t".into(),
            number: 1,
            name: "n".into(),
            agent: None,
            profile_id: "p".into(),
            cwd: String::new(),
            project: project.into(),
            mode: None,
            state: marshell_protocol::api::SessionState::Running,
            command: marshell_protocol::api::CommandState::Unknown,
            paused: false,
            foreground: None,
            live: None,
            activity: None,
            status_note: None,
            muted: false,
            attention: None,
            stuck: None,
        };
        assert_eq!(Who::of(&info("C:\\work\\my-app")).project, "my-app");
        assert_eq!(Who::of(&info("/home/me/my-app/")).project, "my-app");
        assert_eq!(Who::of(&info("")).project, "");
    }

    #[test]
    fn a_batch_lists_every_session() {
        let items = [
            (who("a", 3, "api-server", "x"), false),
            (who("b", 5, "web", "x"), false),
            (who("c", 7, "docs", "x"), false),
        ];
        let b = batch(Kind::NeedsYou, &items);
        assert_eq!(b.title, "3 sessions need you");
        assert_eq!(b.body, "Sessions 3 api-server, 5 web and 7 docs");
        assert_eq!(b.meta, "3 sessions");
        assert_eq!(b.action, BannerAction::Open);
        assert_eq!(b.tab_id, None);
        assert_eq!(batch(Kind::Done, &items).title, "3 sessions are done");
        assert_eq!(batch(Kind::Error, &items).title, "3 sessions stopped");
        assert_eq!(
            batch(Kind::NeedsYou, &items[..2]).body,
            "Sessions 3 api-server and 5 web"
        );
    }

    #[test]
    fn a_batch_with_a_risky_request_says_review() {
        let items = [(who("a", 3, "api", "x"), false), (who("b", 5, "web", "x"), true)];
        assert_eq!(batch(Kind::NeedsYou, &items).action, BannerAction::Review);
    }

    #[test]
    fn an_escalation_says_how_long_it_has_waited() {
        let b = escalation(&api(), 1, true, false);
        assert_eq!(b.title, "api-server needs you");
        assert_eq!(b.body, "api-server has waited 1 min to run a command");
        assert_eq!(b.action, BannerAction::Open);
        assert_eq!(
            escalation(&api(), 3, false, false).body,
            "api-server has waited 3 min for an answer"
        );
        assert_eq!(escalation(&api(), 1, true, true).action, BannerAction::Review);
    }

    #[test]
    fn no_line_cheers() {
        let all = [
            needs_you(&api(), "Has a question", false),
            done(&api(), None),
            error(&api(), "rate limit reached"),
            escalation(&api(), 1, true, false),
        ];
        for b in all {
            let text = format!("{} {}", b.title, b.body).to_lowercase();
            assert!(!text.contains('!') && !text.contains("successfully") && !text.contains("oops"));
        }
    }
}
