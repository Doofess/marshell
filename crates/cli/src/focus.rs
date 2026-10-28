//! `marshell focus <session>`: brings Marshell's window to a session.
//!
//! Exit codes: 0 asked the window to show the session, 1 refused, failed, or the app is closed and wasn't started.

use crate::client::{NOT_RUNNING, resolve};
use crate::start::ensure_running;
use std::io::{self, BufRead, Write};

pub fn run(
    env: &dyn Fn(&str) -> Option<String>,
    session: &str,
    start: bool,
    stdin_is_tty: bool,
    ask: &mut dyn BufRead,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let client = match ensure_running(env, stdin_is_tty, ask, out, start) {
        Ok(client) => client,
        Err(line) => {
            let _ = writeln!(err, "{line}");
            return 1;
        }
    };
    let snapshot = match client.sessions() {
        Ok(snapshot) => snapshot,
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
            let _ = writeln!(err, "{NOT_RUNNING}");
            return 1;
        }
        Err(e) => {
            let _ = writeln!(err, "{e}");
            return 1;
        }
    };
    let Some(row) = resolve(&snapshot, session) else {
        let _ = writeln!(err, "No session {session}.");
        return 1;
    };
    // Windows lets a process bring its window forward only when the one that is allowed to has said so. The app
    // is not in the foreground when this runs, as the person is in a terminal.
    allow_foreground(env);
    match client.focus(&row.tab_id) {
        Ok(()) => 0,
        Err(line) => {
            let _ = writeln!(err, "{line}");
            1
        }
    }
}

/// Lets the app's process take the foreground. A failure changes nothing: the window then flashes in the taskbar.
#[cfg(windows)]
fn allow_foreground(env: &dyn Fn(&str) -> Option<String>) {
    use windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;
    if let Some(pid) = marshell_hook::endpoint::discover_pid(env) {
        // SAFETY: a plain Win32 call with a process id.
        unsafe {
            AllowSetForegroundWindow(pid);
        }
    }
}

#[cfg(not(windows))]
fn allow_foreground(_: &dyn Fn(&str) -> Option<String>) {}
