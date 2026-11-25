//! The Doctor's instruction files, on the server side: where each CLI's files are looked for, for each of Marshell's
//! projects and in each running managed distro. What each CLI loads is read by `agents/<cli>/instructions.rs`, and the
//! items come from `doctor::instructions`. Only a run calls `loads`; a fix preview, Keep and Measure never do. Nothing
//! here writes a file.

use super::AppState;
use super::doctor_ext::windows_roots;
use crate::agents::{antigravity, claude, codex, copilot, qwen};
use crate::doctor::AgentFacts;
use crate::doctor::ext::ExtRoots;
use crate::doctor::instructions::{CliLoad, Place, Reader, display};
use std::path::{Component, Path, PathBuf, Prefix};

/// The reason a project's own files are out when the folder can't be read.
const UNREADABLE: &str = "Marshell can't read this folder";
/// The reason for a project the 10-second budget didn't reach.
const OUT_OF_TIME: &str = "not checked: the check ran out of time";

/// The note on a project the budget ran out in, after some of its files were read.
const PARTIAL: &str = "Marshell ran out of time while reading this folder, so files may be missing.";

/// One CLI's `CliLoad` in one place, or none for an agent this doesn't know.
fn load_of(agent: &str, place: &Place, cx: &mut Reader) -> Option<CliLoad> {
    Some(match agent {
        "claude" => claude::instructions::load(place, cx),
        "codex" => codex::instructions::load(place, cx),
        "antigravity" => antigravity::instructions::load(place, cx),
        "qwen" => qwen::instructions::load(place, cx),
        "copilot" => copilot::instructions::load(place, cx),
        _ => return None,
    })
}

/// Every installed CLI's instruction files, on Windows once per project (once with no project when there are none) and
/// in each running managed distro at user level. A CLI whose facts have an error, or that isn't found, is left out.
/// Reads files: callers are already off the async threads.
pub(crate) fn loads(
    st: &AppState,
    facts: &[AgentFacts],
    distro_homes: &[(String, PathBuf)],
    projects: &[PathBuf],
) -> Vec<CliLoad> {
    #[cfg(test)]
    st.0.instr_reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let mut out = Vec::new();
    let installed = |f: &&AgentFacts| f.found && f.error.is_none();
    if let Some(roots) = windows_roots(st) {
        let agents: Vec<&'static str> = facts
            .iter()
            .filter(|f| f.distro.is_none())
            .filter(installed)
            .map(|f| f.agent)
            .collect();
        if !agents.is_empty() {
            // One reader for every project and CLI, so a file is read once and the time budget covers the whole run.
            let mut cx = Reader::windows();
            windows_loads(st, &roots, &agents, projects, &mut cx, &mut out);
        }
    }
    for (distro, home) in distro_homes {
        let Some(unc_root) = unc_root(home) else {
            continue;
        };
        let agents: Vec<&'static str> = facts
            .iter()
            .filter(|f| f.distro.as_deref().is_some_and(|d| d.eq_ignore_ascii_case(distro)))
            .filter(installed)
            .map(|f| f.agent)
            .collect();
        if agents.is_empty() {
            continue;
        }
        let roots = ExtRoots::under(home);
        let place = Place {
            roots: &roots,
            project: None,
            ceiling: None,
            claude_managed: None,
            qwen_system_settings: None,
            distro: Some(distro),
        };
        let mut cx = Reader::distro(unc_root);
        for agent in &agents {
            out.extend(load_of(agent, &place, &mut cx));
        }
    }
    out
}

pub(super) fn windows_loads(
    st: &AppState,
    roots: &ExtRoots,
    agents: &[&'static str],
    projects: &[PathBuf],
    cx: &mut Reader,
    out: &mut Vec<CliLoad>,
) {
    // A test that pins the home reads nothing above a project, and only the managed folder it names. Otherwise the
    // walk goes up to the filesystem root, as the CLIs' do, and the organisation's folders are read.
    let pinned = st.0.ext_home.is_some();
    let claude_managed: Option<PathBuf> = if pinned {
        st.0.claude_compact_paths
            .as_ref()
            .and_then(|(_, managed)| managed.first())
            .and_then(|file| file.parent().map(Path::to_path_buf))
    } else {
        claude::compact::managed_settings_paths()
            .first()
            .and_then(|file| file.parent().map(Path::to_path_buf))
    };
    let qwen_system: Option<PathBuf> = if pinned {
        st.0.qwen_system_settings.clone()
    } else {
        Some(
            st.0.qwen_system_settings
                .clone()
                .unwrap_or_else(|| PathBuf::from(qwen::system_settings_for(std::env::consts::OS))),
        )
    };
    let read = |project: Option<&Path>, cx: &mut Reader| -> Vec<CliLoad> {
        let place = Place {
            roots,
            project,
            ceiling: if pinned { project } else { None },
            claude_managed: claude_managed.as_deref(),
            qwen_system_settings: qwen_system.as_deref(),
            distro: None,
        };
        agents.iter().filter_map(|agent| load_of(agent, &place, cx)).collect()
    };

    if projects.is_empty() {
        out.extend(read(None, cx));
        return;
    }
    let mut seen: Vec<String> = Vec::new();
    for project in projects {
        // A folder listed twice is one line.
        let key = key_of(project);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        if cx.out_of_time() {
            for agent in agents {
                out.push(CliLoad {
                    agent,
                    distro: None,
                    project: Some(project.clone()),
                    home: roots.home.clone(),
                    files: Vec::new(),
                    reason: Some(OUT_OF_TIME.to_string()),
                    notes: Vec::new(),
                    cuts: Vec::new(),
                    imports: None,
                });
            }
        } else if cx.is_dir(project) {
            let mut read = read(Some(project), cx);
            // The budget ran out inside this project: its walk and its lists stopped early, so the line says so.
            if cx.out_of_time() {
                for load in &mut read {
                    load.notes.push(PARTIAL.to_string());
                }
            }
            out.extend(read);
        } else {
            // The user-level files still count; the project's own can't be read.
            let note = format!("Marshell can't read {}.", display(project, None, &roots.home));
            for mut load in read(None, cx) {
                load.project = Some(project.clone());
                load.reason = Some(UNREADABLE.to_string());
                load.notes.push(note.clone());
                out.push(load);
            }
        }
    }
}

/// A path as one comparable text, to tell a folder listed twice.
fn key_of(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let text = text.trim_end_matches('/');
    if cfg!(windows) {
        text.to_ascii_lowercase()
    } else {
        text.to_string()
    }
}

/// The UNC server and share a distro's home is on (`\\wsl.localhost\Ubuntu\`), or None for a home that isn't a UNC path.
fn unc_root(home: &Path) -> Option<PathBuf> {
    let mut parts = home.components();
    let Some(Component::Prefix(prefix)) = parts.next() else {
        return None;
    };
    if !matches!(prefix.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..)) {
        return None;
    }
    let mut root = PathBuf::from(prefix.as_os_str());
    if let Some(Component::RootDir) = parts.next() {
        root.push(Component::RootDir.as_os_str());
    }
    Some(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn a_distros_root_is_its_server_and_share() {
        let home = Path::new(r"\\wsl.localhost\Ubuntu\home\me");
        let root = unc_root(home).expect("a UNC root");
        assert!(home.starts_with(&root));
        assert_eq!(root, Path::new(r"\\wsl.localhost\Ubuntu\"));
        assert!(unc_root(Path::new(r"C:\Users\me")).is_none());
    }
}
