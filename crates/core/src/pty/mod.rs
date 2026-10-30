pub mod buffer;
pub mod dsr;
mod freeze;
pub mod osc133;
#[cfg(windows)]
mod proctree;
pub mod record;
mod session;

pub use buffer::{FLUSH_BYTES, MAX_UNACKED, SCROLLBACK_BYTES};
pub use freeze::FreezeError;
pub use osc133::Mark;
pub(crate) use session::force_kill_tree;
pub use session::{Classifier, MarkCallback, Pull, Session, SpawnSpec, ToolCallback};

/// `root` and every process it started, for reading their memory and CPU. `None` where the platform has no way to
/// list them or `root` is gone.
pub(crate) fn process_tree(root: u32) -> Option<Vec<u32>> {
    #[cfg(windows)]
    {
        proctree::descendants(root)
    }
    #[cfg(target_os = "linux")]
    {
        linux_process_tree(root)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = root;
        None
    }
}

/// One read of the machine's process list, for asking about several sessions at once. Blocking to make.
pub(crate) struct ProcessList {
    #[cfg(windows)]
    inner: proctree::Processes,
    /// (pid, parent pid, name)
    #[cfg(target_os = "linux")]
    inner: Vec<(u32, u32, String)>,
}

impl ProcessList {
    /// `None` where the platform has no way to list processes or the list cannot be read.
    pub(crate) fn read() -> Option<ProcessList> {
        #[cfg(windows)]
        {
            proctree::Processes::read().map(|inner| ProcessList { inner })
        }
        #[cfg(target_os = "linux")]
        {
            let mut inner = Vec::new();
            for entry in std::fs::read_dir("/proc").ok()?.flatten() {
                let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
                    continue;
                };
                // A process that exits during the scan is skipped.
                let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
                    continue;
                };
                let Some(parent) = parent_pid(&stat) else { continue };
                let name = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
                inner.push((pid, parent, name.trim().to_string()));
            }
            Some(ProcessList { inner })
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            None
        }
    }

    /// `root` and every process it started as `(pid, name)`, the name as `detect::program_name` gives it. `None` when
    /// `root` is gone or cannot be queried.
    pub(crate) fn tree(&self, root: u32) -> Option<Vec<(u32, String)>> {
        #[cfg(windows)]
        {
            let mut tree = self.inner.tree(root)?;
            for (_, name) in &mut tree {
                *name = crate::profiles::detect::program_name(name);
            }
            Some(tree)
        }
        #[cfg(target_os = "linux")]
        {
            if !self.inner.iter().any(|(pid, _, _)| *pid == root) {
                return None;
            }
            let mut tree = vec![root];
            let mut next = 0;
            while next < tree.len() {
                let parent = tree[next];
                for (pid, ppid, _) in &self.inner {
                    if *ppid == parent && !tree.contains(pid) {
                        tree.push(*pid);
                    }
                }
                next += 1;
            }
            Some(
                tree.into_iter()
                    .filter_map(|pid| {
                        let (_, _, name) = self.inner.iter().find(|(p, _, _)| *p == pid)?;
                        Some((pid, crate::profiles::detect::program_name(name)))
                    })
                    .collect(),
            )
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = root;
            None
        }
    }

    /// The program `root` itself runs: its newest child that is not a console host. On Linux, its newest (highest pid)
    /// child. `None` when `root` has no child or cannot be queried.
    pub(crate) fn outermost(&self, root: u32) -> Option<u32> {
        #[cfg(windows)]
        {
            self.inner.outermost(root)
        }
        #[cfg(target_os = "linux")]
        {
            self.inner
                .iter()
                .filter(|(pid, ppid, _)| *ppid == root && *pid != root)
                .map(|(pid, _, _)| *pid)
                .max()
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = root;
            None
        }
    }
}

/// The command line of process `pid` as the OS keeps it, or `None` where it cannot be read (the process is gone, runs as
/// another user, or the platform keeps none). Blocking.
pub(crate) fn command_line(pid: u32) -> Option<String> {
    #[cfg(windows)]
    {
        proctree::command_line(pid)
    }
    #[cfg(target_os = "linux")]
    {
        let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
        let text = String::from_utf8_lossy(&raw);
        let line = text.trim_end_matches('\0').replace('\0', " ");
        (!line.is_empty()).then_some(line)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = pid;
        None
    }
}

/// The words of a command line from `command_line`: the `CommandLineToArgvW` rules on Windows, whitespace elsewhere.
pub(crate) fn split_args(line: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        proctree::split_args(line)
    }
    #[cfg(not(windows))]
    {
        line.split_whitespace().map(str::to_string).collect()
    }
}

/// The parent pid in `/proc/<pid>/stat`: the second field after the last `)` (the state comes first).
#[cfg(target_os = "linux")]
fn parent_pid(stat: &str) -> Option<u32> {
    stat[stat.rfind(')')? + 1..].split_whitespace().nth(1)?.parse().ok()
}

#[cfg(target_os = "linux")]
fn linux_process_tree(root: u32) -> Option<Vec<u32>> {
    std::fs::metadata(format!("/proc/{root}")).ok()?;
    let mut children: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        // A process that exits during the scan is skipped.
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        if let Some(parent) = parent_pid(&stat) {
            children.entry(parent).or_default().push(pid);
        }
    }
    let mut tree = vec![root];
    let mut next = 0;
    while next < tree.len() {
        if let Some(kids) = children.get(&tree[next]) {
            for &kid in kids {
                if !tree.contains(&kid) {
                    tree.push(kid);
                }
            }
        }
        next += 1;
    }
    Some(tree)
}
