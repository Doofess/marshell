//! Startup time, memory and CPU for the core and for each session's process tree.

use crate::server::AppState;
use marshell_protocol::diagnostics::{PerfSnapshot, ProcessUsage, SessionUsage};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The two moments the startup time sits between. Each is set once; later marks do nothing.
pub(crate) struct Marks {
    launch: OnceLock<Instant>,
    ready: OnceLock<Instant>,
}

impl Marks {
    pub(crate) const fn new() -> Marks {
        Marks {
            launch: OnceLock::new(),
            ready: OnceLock::new(),
        }
    }

    pub(crate) fn launch_at(&self, at: Instant) {
        let _ = self.launch.set(at);
    }

    pub(crate) fn ready_at(&self, at: Instant) {
        let _ = self.ready.set(at);
    }

    /// None until both marks exist.
    pub(crate) fn startup_ms(&self) -> Option<u64> {
        let (launch, ready) = (self.launch.get()?, self.ready.get()?);
        Some(u64::try_from(ready.saturating_duration_since(*launch).as_millis()).unwrap_or(u64::MAX))
    }
}

static MARKS: Marks = Marks::new();

/// The app has just started. Call it first in `main`.
pub fn mark_launch() {
    MARKS.launch_at(Instant::now());
}

/// A core is serving. Only the first call counts, so a restarted core does not move it.
pub fn mark_ready() {
    MARKS.ready_at(Instant::now());
}

/// Milliseconds from `mark_launch` to `mark_ready`; None until both have been called.
pub fn startup_ms() -> Option<u64> {
    MARKS.startup_ms()
}

/// Working set in bytes and total (kernel + user) CPU time of a process. None if it is gone, cannot be
/// opened, or the platform has no reader.
#[cfg(windows)]
pub fn read_process(pid: u32) -> Option<(u64, Duration)> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    /// A handle that closes itself.
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: the handle is valid and nothing else closes it.
            unsafe { CloseHandle(self.0) };
        }
    }

    // SAFETY: the result is checked; the handle is closed by `Handle`.
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if raw.is_null() {
        return None;
    }
    let handle = Handle(raw);

    // SAFETY: an all-zero PROCESS_MEMORY_COUNTERS is valid (plain integers).
    let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: `handle` is a valid process handle and `counters` has its size set.
    if unsafe { GetProcessMemoryInfo(handle.0, &mut counters, counters.cb) } == 0 {
        return None;
    }

    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: `handle` is a valid process handle; the four pointers are valid FILETIMEs.
    if unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        return None;
    }
    let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    // A FILETIME counts 100 ns.
    let cpu = Duration::from_nanos(ticks(kernel).saturating_add(ticks(user)).saturating_mul(100));
    Some((counters.WorkingSetSize as u64, cpu))
}

#[cfg(target_os = "linux")]
pub fn read_process(pid: u32) -> Option<(u64, Duration)> {
    // SAFETY: sysconf has no preconditions.
    let (page, tick) = unsafe { (libc::sysconf(libc::_SC_PAGESIZE), libc::sysconf(libc::_SC_CLK_TCK)) };
    if page <= 0 || tick <= 0 {
        return None;
    }
    let statm = std::fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
    let resident: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name is in parentheses and may hold spaces; the numbered fields follow the last ')'.
    let rest = &stat[stat.rfind(')')? + 1..];
    let mut fields = rest.split_whitespace();
    // `rest` starts at field 3 (state); utime and stime are fields 14 and 15.
    let utime: u64 = fields.nth(11)?.parse().ok()?;
    let stime: u64 = fields.next()?.parse().ok()?;
    let ticks = utime.saturating_add(stime);
    let cpu = Duration::from_nanos(ticks.saturating_mul(1_000_000_000 / tick as u64));
    Some((resident.saturating_mul(page as u64), cpu))
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn read_process(_pid: u32) -> Option<(u64, Duration)> {
    None
}

/// Turns processor-time readings into a CPU share. It keeps the previous reading of every pid it last saw.
pub struct Sampler {
    cpus: u32,
    previous: HashMap<u32, (Instant, Duration)>,
}

impl Sampler {
    pub fn new() -> Sampler {
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get() as u32);
        Sampler::with_cpus(cpus)
    }

    /// A sampler that divides by `cpus` logical CPUs (at least 1).
    pub fn with_cpus(cpus: u32) -> Sampler {
        Sampler {
            cpus: cpus.max(1),
            previous: HashMap::new(),
        }
    }

    /// One usage per group, in order, read with `read_process`.
    pub fn sample(&mut self, groups: &[(String, Vec<u32>)], now: Instant) -> Vec<ProcessUsage> {
        self.sample_with(groups, now, read_process)
    }

    /// As `sample`, with the reader given. A pid the reader cannot read is skipped. CPU is the summed share of the
    /// pids that have an earlier reading, so it is None until some pid has been read twice. A pid not in
    /// `groups` is forgotten.
    pub fn sample_with(
        &mut self,
        groups: &[(String, Vec<u32>)],
        now: Instant,
        read: impl Fn(u32) -> Option<(u64, Duration)>,
    ) -> Vec<ProcessUsage> {
        let mut next = HashMap::new();
        let mut out = Vec::with_capacity(groups.len());
        for (_, pids) in groups {
            let mut processes = 0u32;
            let mut memory: Option<u64> = None;
            let mut cpu: Option<f32> = None;
            for &pid in pids {
                let Some((bytes, cpu_time)) = read(pid) else {
                    continue;
                };
                processes += 1;
                memory = Some(memory.unwrap_or(0).saturating_add(bytes));
                if let Some(&(then, before)) = self.previous.get(&pid) {
                    let wall = now.saturating_duration_since(then).as_secs_f64();
                    if wall > 0.0 {
                        let used = cpu_time.saturating_sub(before).as_secs_f64();
                        let share = (used / (wall * f64::from(self.cpus)) * 100.0) as f32;
                        cpu = Some(cpu.unwrap_or(0.0) + share);
                    }
                }
                next.insert(pid, (now, cpu_time));
            }
            out.push(ProcessUsage {
                processes,
                memory_bytes: memory,
                cpu_percent: cpu,
            });
        }
        self.previous = next;
        out
    }
}

impl Default for Sampler {
    fn default() -> Self {
        Sampler::new()
    }
}

/// The core's own usage and each open session's process tree. Blocking: it reads the process list.
pub fn snapshot(st: &AppState) -> PerfSnapshot {
    let mut sessions = st.session_pids();
    sessions.sort();
    let mut groups = vec![("core".to_string(), vec![std::process::id()])];
    for (tab, pid) in &sessions {
        // When the tree cannot be read whole, the session's own process still counts.
        let tree = crate::pty::process_tree(*pid).unwrap_or_else(|| vec![*pid]);
        groups.push((tab.clone(), tree));
    }
    let now = Instant::now();
    let mut usages = st.sampler().sample(&groups, now).into_iter();
    let core = usages.next().unwrap_or(ProcessUsage {
        processes: 0,
        memory_bytes: None,
        cpu_percent: None,
    });
    let sessions = sessions
        .into_iter()
        .zip(usages)
        .map(|((tab_id, _), usage)| SessionUsage { tab_id, usage })
        .collect();
    PerfSnapshot {
        startup_ms: startup_ms(),
        core,
        sessions,
        sampled_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    fn groups(pids: &[u32]) -> Vec<(String, Vec<u32>)> {
        vec![("a".to_string(), pids.to_vec())]
    }

    #[test]
    fn cpu_is_none_first_then_a_share() {
        let mut sampler = Sampler::with_cpus(4);
        let t0 = Instant::now();
        let first = sampler.sample_with(&groups(&[7]), t0, |_| Some((1000, Duration::from_millis(100))));
        assert_eq!(first[0].cpu_percent, None);
        assert_eq!(first[0].memory_bytes, Some(1000));
        let second = sampler.sample_with(&groups(&[7]), t0 + Duration::from_millis(100), |_| {
            Some((1000, Duration::from_millis(150)))
        });
        assert_eq!(second[0].cpu_percent, Some(12.5));
    }

    #[test]
    fn snapshot_skips_a_process_that_is_gone() {
        let mut sampler = Sampler::with_cpus(4);
        let mem: HashMap<u32, u64> = HashMap::from([(1, 100), (3, 300)]);
        let out = sampler.sample_with(&groups(&[1, 2, 3]), Instant::now(), |pid| {
            mem.get(&pid).map(|m| (*m, Duration::ZERO))
        });
        assert_eq!(out[0].processes, 2);
        assert_eq!(out[0].memory_bytes, Some(400));
        let none = sampler.sample_with(&groups(&[2]), Instant::now(), |_| None);
        assert_eq!(none[0].processes, 0);
        assert_eq!(none[0].memory_bytes, None);
    }

    #[test]
    fn startup_is_ready_minus_launch() {
        let marks = Marks::new();
        assert_eq!(marks.startup_ms(), None);
        let t0 = Instant::now();
        marks.launch_at(t0);
        assert_eq!(marks.startup_ms(), None);
        marks.ready_at(t0 + Duration::from_millis(250));
        assert_eq!(marks.startup_ms(), Some(250));
        // Later marks do nothing.
        marks.launch_at(t0 + Duration::from_secs(5));
        marks.ready_at(t0 + Duration::from_secs(9));
        assert_eq!(marks.startup_ms(), Some(250));
    }

    #[test]
    fn a_pid_that_is_not_listed_is_forgotten() {
        let mut sampler = Sampler::with_cpus(1);
        let t0 = Instant::now();
        let read = |_| Some((1, Duration::from_millis(10)));
        sampler.sample_with(&groups(&[7]), t0, read);
        sampler.sample_with(&groups(&[8]), t0 + Duration::from_millis(50), read);
        // 7 comes back: it is a first sight again.
        let out = sampler.sample_with(&groups(&[7]), t0 + Duration::from_millis(100), read);
        assert_eq!(out[0].cpu_percent, None);
    }
}
