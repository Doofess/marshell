//! The port watcher: every two seconds it reads the machine's TCP listeners once and each tab's process tree, and hands
//! the registry the ports a tab's own processes listen on. The registry turns them into `SessionInfo.ports` chips
//! (see `Registry::set_listening` for the rules). A port line is never logged.

use super::AppState;
use crate::agents::manifest::Agents;
use crate::pty::ProcessList;
use crate::tools::listen::{chips_for, listeners};
use std::sync::Arc;
use std::time::Duration;

/// How often the listeners are read.
const POLL: Duration = Duration::from_secs(2);

/// Starts the watcher on the current runtime. Does nothing outside one.
pub(super) fn spawn_watcher(state: &AppState) {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    // A weak handle, so this task does not keep a dropped core alive.
    let inner = Arc::downgrade(&state.0);
    runtime.spawn(async move {
        loop {
            tokio::time::sleep(POLL).await;
            let Some(inner) = inner.upgrade() else {
                return;
            };
            tick(&AppState(inner)).await;
        }
    });
}

/// What one tab is read for: the tab, its generation, its shell's pid, whether its command's ports are transient and
/// whether the row is, or shows, an agent.
struct Candidate {
    tab: String,
    generation: u64,
    pid: u32,
    quiet: bool,
    runs_agent: bool,
}

/// One pass. The registry is locked only while the work list is made and while the results are applied, never across
/// the blocking reads or an `.await`.
async fn tick(st: &AppState) {
    let mut work = Vec::new();
    for (tab, generation) in st.0.registry.ports_candidates() {
        if let Some(session) = st.session(&tab)
            && !session.has_exited()
            && !session.is_frozen()
            && let Some(pid) = session.pid()
        {
            work.push(Candidate {
                runs_agent: st.0.registry.runs_agent(&tab),
                quiet: session.quiet_ports(),
                tab,
                generation,
                pid,
            });
        }
    }
    // Nothing to look at: no table is read.
    if work.is_empty() {
        return;
    }
    let agents = st.agents();
    let found = tokio::task::spawn_blocking(move || read(&agents, work)).await;
    // `None` or a failed task: the lists stay as they were.
    let Ok(Some(found)) = found else {
        return;
    };
    for (tab, generation, ports) in found {
        st.0.registry.set_listening(&tab, generation, &ports);
    }
}

/// The ports each candidate's processes listen on, from one read of the listener table and one of the process list.
/// `None` when either cannot be read. A candidate whose tree cannot be read is left out, so its chips stay. Blocking.
fn read(agents: &Agents, work: Vec<Candidate>) -> Option<Vec<(String, u64, Vec<u16>)>> {
    let table = listeners().or_else(|| {
        tracing::debug!("the TCP listener table could not be read");
        None
    })?;
    let processes = ProcessList::read()?;
    Some(
        work.into_iter()
            .filter_map(|c| {
                let tree = processes.tree(c.pid)?;
                let outermost = if c.runs_agent { processes.outermost(c.pid) } else { None };
                let agent = agent_pids(agents, &tree, outermost, c.runs_agent);
                Some((c.tab, c.generation, chips_for(&table, &tree, &agent, c.quiet)))
            })
            .collect(),
    )
}

/// The processes of a tab's `tree` (pid, program name) whose listeners are an agent's own, not a dev server's: every
/// process a known agent's exe names, and, for a row launched as an agent or showing one in front, the outermost
/// process (`outermost`): for an npm-installed agent that is `node`, which names no agent. What the agent starts, such
/// as the dev server its Bash tool runs, is not in the list.
fn agent_pids(agents: &Agents, tree: &[(u32, String)], outermost: Option<u32>, runs_agent: bool) -> Vec<u32> {
    let mut pids: Vec<u32> = tree
        .iter()
        .filter(|(_, name)| agents.agent_of_exe(name).is_some())
        .map(|(pid, _)| *pid)
        .collect();
    if runs_agent && let Some(pid) = outermost {
        pids.push(pid);
    }
    pids.sort_unstable();
    pids.dedup();
    pids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Registry, Relaunch};
    use crate::tools::listen::chips_for;

    fn tree(items: &[(u32, &str)]) -> Vec<(u32, String)> {
        items.iter().map(|(pid, name)| (*pid, name.to_string())).collect()
    }

    #[test]
    fn listening_ports_skip_agents_quiet_commands_and_one_poll_blips() {
        let agents = Agents::shipped();
        // A known agent's own listener (its OAuth callback) and a pid outside the tab get no chip.
        let t = tree(&[(1, "pwsh"), (10, "node"), (11, "claude"), (12, "node")]);
        let listeners = [(10, 3000), (11, 8085), (12, 5173), (99, 4000)];
        let agent = agent_pids(&agents, &t, None, false);
        assert_eq!(agent, [11]);
        assert_eq!(chips_for(&listeners, &t, &agent, false), [3000, 5173]);
        // A quiet command (vercel login) makes none.
        assert_eq!(chips_for(&listeners, &t, &agent, true), Vec::<u16>::new());
        // A Claude row whose chain is node, node: the outer node is the agent, its child's port is a dev server's.
        let t = tree(&[(1, "pwsh"), (20, "node"), (21, "node")]);
        let listeners = [(20, 43111), (21, 5173)];
        let agent = agent_pids(&agents, &t, Some(20), true);
        assert_eq!(agent, [20]);
        assert_eq!(chips_for(&listeners, &t, &agent, false), [5173]);
        // A plain node program in a shell row is no agent.
        let agent = agent_pids(&agents, &t, Some(20), false);
        assert!(agent.is_empty());
        assert_eq!(chips_for(&listeners, &t, &agent, false), [5173, 43111]);
        // One poll is a blip: the port needs two in a row.
        let reg = Registry::new();
        reg.reserve(
            "a",
            Relaunch {
                profile_id: "pwsh".into(),
                agent: None,
                cwd: "C:/work".into(),
                mode: None,
                name: "A".into(),
                preset: None,
            },
        );
        let (tab, generation) = reg.ports_candidates().remove(0);
        let ports = |reg: &Registry| {
            reg.snapshot().sessions[0]
                .ports
                .iter()
                .map(|p| p.port)
                .collect::<Vec<_>>()
        };
        reg.set_listening(&tab, generation, &[3000]);
        reg.set_listening(&tab, generation, &[]);
        reg.set_listening(&tab, generation, &[3000]);
        assert!(ports(&reg).is_empty());
        reg.set_listening(&tab, generation, &[3000]);
        assert_eq!(ports(&reg), [3000]);
    }
}
