use std::path::PathBuf;

use marshell_protocol::{api::Endpoint, brand};

/// `MARSHELL_HOME` if set and not empty, else `<user home>/.marshell`.
pub fn home(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    match env(&brand::env_var("HOME")).filter(|h| !h.is_empty()) {
        Some(h) => Some(PathBuf::from(h)),
        None => std::env::home_dir().map(|h| h.join(brand::HOME_DIR_NAME)),
    }
}

/// Whether a process with this id is running. Marshell deletes `endpoint.json` when it closes cleanly, but a
/// crash leaves it behind, and its port may then belong to someone else.
#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, GetLastError, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    // SAFETY: plain Win32 calls; the handle is closed before returning.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            // A process we may not open is still running; any other failure means there is no such process.
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code) != 0;
        CloseHandle(handle);
        // If the exit code cannot be read, assume the process is running.
        !ok || code == STILL_ACTIVE as u32
    }
}

/// Linux lists every running process under `/proc`.
#[cfg(target_os = "linux")]
fn process_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

/// Other Unix systems: there is no check without adding `libc`, so the process counts as running and the
/// bridge behaves as it did before the PID was recorded.
#[cfg(all(unix, not(target_os = "linux")))]
fn process_alive(_pid: u32) -> bool {
    true
}

/// The parsed `<home>/run/endpoint.json`: the home folder, the endpoint, and the `pid` when the file has one.
fn read(env: &dyn Fn(&str) -> Option<String>) -> Option<(PathBuf, Endpoint, Option<u32>)> {
    let home = home(env)?;
    let text = std::fs::read_to_string(home.join("run").join("endpoint.json")).ok()?;
    let ep: Endpoint = serde_json::from_str(&text).ok()?;
    let pid = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("pid")?.as_u64())
        .and_then(|p| u32::try_from(p).ok());
    Some((home, ep, pid))
}

/// Reads `<home>/run/endpoint.json`. A missing or unreadable file gives `None`: the app is closed.
/// So does a file whose `pid` names a process that is gone. A file without `pid` (an older core) is used as is.
pub fn discover(env: &dyn Fn(&str) -> Option<String>) -> Option<(PathBuf, u16, String)> {
    let (home, ep, pid) = read(env)?;
    if pid.is_some_and(|pid| !process_alive(pid)) {
        return None;
    }
    Some((home, ep.port, ep.token))
}

/// The id of the running app's process, from `endpoint.json`. `None` when the app is closed, when the file names a
/// process that is gone, or when the file has no `pid` (an older core).
pub fn discover_pid(env: &dyn Fn(&str) -> Option<String>) -> Option<u32> {
    let (_, _, pid) = read(env)?;
    pid.filter(|pid| process_alive(*pid))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discover_with(pid_field: &str) -> Option<(PathBuf, u16, String)> {
        discover_in_home(pid_field, |env| discover(env))
    }

    fn discover_in_home<T>(pid_field: &str, f: impl Fn(&dyn Fn(&str) -> Option<String>) -> T) -> T {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("run")).unwrap();
        std::fs::write(
            home.path().join("run").join("endpoint.json"),
            format!(r#"{{"version":1,"port":4242,"token":"t0k","platform":"windows"{pid_field}}}"#),
        )
        .unwrap();
        let path = home.path().to_string_lossy().into_owned();
        f(&|k| (k == "MARSHELL_HOME").then(|| path.clone()))
    }

    /// The id of a process that has already exited.
    fn dead_pid() -> u32 {
        #[cfg(windows)]
        let mut cmd = std::process::Command::new("cmd");
        #[cfg(windows)]
        cmd.args(["/C", "exit"]);
        #[cfg(not(windows))]
        let mut cmd = std::process::Command::new("true");
        let mut child = cmd.spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[test]
    fn a_file_naming_a_dead_core_is_not_an_endpoint() {
        let pid = dead_pid();
        assert!(discover_with(&format!(r#","pid":{pid}"#)).is_none());
    }

    #[test]
    fn a_file_naming_a_live_core_is_an_endpoint() {
        let pid = std::process::id();
        let (_, port, token) = discover_with(&format!(r#","pid":{pid}"#)).unwrap();
        assert_eq!((port, token.as_str()), (4242, "t0k"));
    }

    #[test]
    fn a_file_without_a_pid_is_still_an_endpoint() {
        assert!(discover_with("").is_some());
    }

    #[test]
    fn discover_pid_names_a_live_core_only() {
        let me = std::process::id();
        assert_eq!(
            discover_in_home(&format!(r#","pid":{me}"#), |env| discover_pid(env)),
            Some(me)
        );
        let dead = dead_pid();
        assert_eq!(
            discover_in_home(&format!(r#","pid":{dead}"#), |env| discover_pid(env)),
            None
        );
        // An older core wrote no pid.
        assert_eq!(discover_in_home("", |env| discover_pid(env)), None);
        // No file: the app is closed.
        let home = tempfile::tempdir().unwrap();
        let path = home.path().to_string_lossy().into_owned();
        assert_eq!(discover_pid(&|k| (k == "MARSHELL_HOME").then(|| path.clone())), None);
    }

    #[test]
    fn our_own_process_is_alive() {
        assert!(process_alive(std::process::id()));
    }
}
