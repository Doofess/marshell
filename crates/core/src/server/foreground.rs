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
    let mut work: Vec<(Target, Arc<Session>, u64)> = Vec::new();
    for (tab, generation, shows) in st.0.registry.foreground_candidates() {
        if let Some(session) = st.session(&tab)
            && polled(&session, shows)
        {
            // The epoch comes before the reading, so a mark that lands during it is noticed.
            let epoch = session.command_epoch();
            work.push((Target::Row { tab, generation }, session, epoch));
        }
    }
    if let Some(tab) = st.panel_tab()
        && let Some(session) = st.session(&tab)
        && polled(&session, false)
    {
        let epoch = session.command_epoch();
        work.push((Target::Panel { tab }, session, epoch));
    }
    if work.is_empty() {
        return;
    }
    // One blocking task for the whole pass: reading a process list is not async.
    let readings = tokio::task::spawn_blocking(move || {
        work.into_iter()
            .map(|(target, session, epoch)| {
                let prompt = at_prompt(session.ever_executed(), session.command_running());
                let reading = reading_for(prompt, || read(&session));
                // The session comes back for its epoch at apply time.
                (target, session, epoch, reading)
            })
            .collect::<Vec<_>>()
    })
    .await;
    let Ok(readings) = readings else {
        return;
    };
    for (target, session, epoch, reading) in readings {
        // `None`: the program could not be named, or a C, D or A mark came while it was being
        // read. Either way leave the row as it is: the marks, and the next pass, have the say.
        let Some(foreground) = fresh(epoch, session.command_epoch(), reading) else {
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

/// What a pass learned about one session: `Some(None)` nothing runs, `Some(Some(_))` a program,
/// `None` something runs that cannot be named.
type Reading = Option<Option<Foreground>>;

/// A shell with integration (it has sent a C mark) that is not running a command is at its prompt.
/// By definition it runs nothing in the foreground: a GUI program it started, like notepad, is
/// not its foreground, but is still a live child that a chain read would find.
fn at_prompt(ever_executed: bool, command_running: bool) -> bool {
    ever_executed && !command_running
}

/// The reading for a session: nothing at its prompt, without looking at the chain, else `read()`.
fn reading_for(at_prompt: bool, read: impl FnOnce() -> Reading) -> Reading {
    if at_prompt { Some(None) } else { read() }
}

/// The reading to apply, or `None` to apply nothing: a reading taken at one command epoch is stale
/// at another, because a C, D or A mark came while it was being made.
fn fresh(taken_at: u64, now: u64, reading: Reading) -> Reading {
    if taken_at == now { reading } else { None }
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

    fn shown(program: &str) -> Foreground {
        Foreground {
            program: program.into(),
            command_line: None,
            agent: None,
        }
    }

    #[test]
    fn a_shell_at_its_prompt_has_no_foreground_and_is_not_read() {
        assert!(at_prompt(true, false));
        assert!(!at_prompt(true, true));
        // No integration: no C mark yet, so the chain is the only source.
        assert!(!at_prompt(false, false));
        let reading = reading_for(true, || panic!("the chain was read at the prompt"));
        assert_eq!(reading, Some(None));
        let reading = reading_for(false, || Some(Some(shown("notepad"))));
        assert_eq!(reading, Some(Some(shown("notepad"))));
    }

    #[test]
    fn a_reading_that_lost_a_race_with_a_mark_is_dropped() {
        let late = Some(Some(shown("git")));
        assert_eq!(fresh(4, 5, late.clone()), None);
        assert_eq!(fresh(4, 5, Some(None)), None);
        assert_eq!(fresh(4, 4, late.clone()), late);
    }

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
