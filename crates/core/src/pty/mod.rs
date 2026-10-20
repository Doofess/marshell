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
pub use session::{MarkCallback, Pull, Session, SpawnSpec};

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
