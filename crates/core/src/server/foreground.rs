//! The foreground watcher: once a second it reads what each plain shell session is running and
//! puts it in the registry as `SessionInfo.foreground`. It also hands the shell panel's shell to
//! the session list when that shell runs a known agent.
//!
//! A session is looked at only if it was not launched as an agent, is running and not paused, and
//! either has a command running, has never sent a C mark (cmd, and shells without integration), or
//! still shows a program (so a result that lost a race with the D mark is cleared next tick).

use super::AppState;
use super::shell_panel::{PANEL_GENERATION, take_and_adopt};
use crate::profiles::detect::{agent_in, program_of};
use crate::pty::Session;
use marshell_protocol::api::Foreground;
use std::sync::Arc;
use std::time::Duration;

/// How often a running command is looked at, at most. A command that starts wakes the watcher early.
const POLL: Duration = Duration::from_secs(1);

/// Which session a reading is for.
enum Target {
    /// A registry row, and the generation of the process the reading was taken from.
    Row { tab: String, generation: u64 },
    /// The shell panel's shell, which has no row.
    Panel { tab: String },
}

/// Starts the watcher on the current runtime. Does nothing outside one.
pub(super) fn spawn_watcher(state: &AppState) {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let wake = state.0.foreground_wake.clone();
    // A weak handle, so this task does not keep a dropped core alive.
    let inner = Arc::downgrade(&state.0);
    runtime.spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(POLL) => {}
                _ = wake.notified() => {}
            }
            let Some(inner) = inner.upgrade() else {
                return;
            };
            tick(&AppState(inner)).await;
        }
    });
}

/// Whether this session is worth looking at now.
fn polled(session: &Session, shows_foreground: bool) -> bool {
    !session.has_exited()
        && !session.is_frozen()
        && (session.command_running() || !session.ever_executed() || shows_foreground)
}

/// One pass. The registry and the sessions map are locked only while the work list is made and
/// while the results are applied, never across the blocking process reads or an `.await`.
async fn tick(st: &AppState) {
    let mut work: Vec<(Target, Arc<Session>)> = Vec::new();
    for (tab, generation, shows) in st.0.registry.foreground_candidates() {
        if let Some(session) = st.session(&tab)
            && polled(&session, shows)
        {
            work.push((Target::Row { tab, generation }, session));
        }
    }
    if let Some(tab) = st.panel_tab()
        && let Some(session) = st.session(&tab)
        && polled(&session, false)
    {
        work.push((Target::Panel { tab }, session));
    }
    if work.is_empty() {
        return;
    }
    // One blocking task for the whole pass: reading a process list is not async.
    let readings = tokio::task::spawn_blocking(move || {
        work.into_iter()
            .map(|(target, session)| (target, read(&session)))
            .collect::<Vec<_>>()
    })
    .await;
    let Ok(readings) = readings else {
        return;
    };
    for (target, reading) in readings {
        // `None`: the program could not be named, so leave the row as it is.
        let Some(foreground) = reading else {
            continue;
        };
        match target {
            Target::Row { tab, generation } => st.0.registry.set_foreground(&tab, generation, foreground),
            Target::Panel { tab } => {
                // An agent in the panel moves to the list. Only if the panel still holds this shell
                // and nobody (the "Open in a pane" button) adopted it first.
                if let Some(foreground) = foreground
                    && foreground.agent.is_some()
                    && let Some(adopted) = take_and_adopt(st, Some(&tab))
                {
                    st.0.registry
                        .set_foreground(&adopted, PANEL_GENERATION, Some(foreground));
                }
            }
        }
    }
}

/// Reads the session's process chain and command line. Blocking.
fn read(session: &Session) -> Option<Option<Foreground>> {
    foreground_of(session.foreground_chain().as_deref(), session.command_line())
}

/// What the chain (outermost first) and the reported command line say the shell is running.
/// `Some(None)`: nothing. `None`: something runs but it cannot be named.
fn foreground_of(chain: Option<&[String]>, command_line: Option<String>) -> Option<Option<Foreground>> {
    let chain = chain.unwrap_or(&[]);
    if chain.is_empty() && command_line.is_none() {
        return Some(None);
    }
    let program = match chain.last() {
        Some(last) => last.clone(),
        None => program_of(command_line.as_deref()?)?,
    };
    let agent = agent_in(chain, command_line.as_deref()).map(String::from);
    Some(Some(Foreground {
        program,
        command_line,
        agent,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn nothing_running_is_no_foreground() {
        assert_eq!(foreground_of(None, None), Some(None));
        assert_eq!(foreground_of(Some(&[][..]), None), Some(None));
    }

    #[test]
    fn the_deepest_process_is_the_program() {
        let chain = names(&["pnpm", "node"]);
        let fg = foreground_of(Some(chain.as_slice()), Some("pnpm dev".into()))
            .unwrap()
            .unwrap();
        assert_eq!(fg.program, "node");
        assert_eq!(fg.command_line.as_deref(), Some("pnpm dev"));
        assert_eq!(fg.agent, None);
    }

    #[test]
    fn the_command_line_names_the_program_when_the_chain_is_empty() {
        let fg = foreground_of(None, Some("npx @openai/codex".into())).unwrap().unwrap();
        assert_eq!(fg.program, "@openai/codex");
        assert_eq!(fg.agent.as_deref(), Some("codex"));
    }

    #[test]
    fn an_agent_anywhere_in_the_chain_names_it() {
        let chain = names(&["node", "codex-win32-x64"]);
        let fg = foreground_of(Some(chain.as_slice()), None).unwrap().unwrap();
        assert_eq!(fg.agent.as_deref(), Some("codex"));
        assert_eq!(fg.command_line, None);
    }

    #[test]
    fn a_command_line_that_names_nothing_is_skipped() {
        assert_eq!(foreground_of(None, Some("&".into())), None);
    }
}
