//! `marshell-host --pipe <base> --core-pid <pid>`: the helper that runs one elevated session.
//! It connects to the two pipes the core created, takes one Spawn frame, runs the program in a
//! pty and pumps bytes. It accepts nothing but its frames (see the slice 6j2 plan).
//!
//! Exit codes: 2 bad arguments, 3 not elevated, 4 could not connect or the pipe's server is not
//! the core, 5 the working folder is not reachable.

mod pump;
#[cfg(windows)]
mod win;

/// The copy for a working folder that the elevated token cannot reach, or `None` when it exists.
#[cfg_attr(not(windows), allow(dead_code))]
fn missing_cwd_message(cwd: &str) -> Option<String> {
    if std::path::Path::new(cwd).is_dir() {
        return None;
    }
    Some(format!(
        "The folder {cwd} can't be reached as administrator. A mapped or substituted drive is visible only to your own sessions. Nothing was started."
    ))
}

#[cfg_attr(not(windows), allow(dead_code))]
struct Args {
    pipe: String,
    core_pid: u32,
}

#[cfg_attr(not(windows), allow(dead_code))]
fn parse_args(args: impl Iterator<Item = String>) -> Option<Args> {
    let mut args = args;
    let (mut pipe, mut core_pid) = (None, None);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--pipe" => pipe = Some(args.next()?),
            "--core-pid" => core_pid = Some(args.next()?.parse().ok()?),
            _ => return None,
        }
    }
    let pipe = pipe.filter(|p| !p.is_empty() && !p.contains(['\\', '/']))?;
    Some(Args {
        pipe,
        core_pid: core_pid?,
    })
}

#[cfg(not(windows))]
fn main() {
    println!("marshell-host runs on Windows only.");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    let Some(args) = parse_args(std::env::args().skip(1)) else {
        eprintln!("usage: marshell-host --pipe <name> --core-pid <pid>");
        std::process::exit(2);
    };
    if !elevation_ok() {
        eprintln!("marshell-host must run as administrator.");
        std::process::exit(3);
    }
    // Held until the process ends: closing it kills the whole tree.
    let _job = match win::Job::tie_this_process() {
        Ok(job) => job,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let pipes = match win::connect(&args.pipe, args.core_pid) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(4);
        }
    };
    let code = session::run(pipes);
    std::process::exit(code);
}

#[cfg(windows)]
fn elevation_ok() -> bool {
    #[cfg(debug_assertions)]
    if std::env::var("MARSHELL_HOST_ALLOW_UNELEVATED").as_deref() == Ok("1") {
        return true;
    }
    win::is_elevated()
}

#[cfg(windows)]
mod session {
    use super::{missing_cwd_message, pump, win};
    use marshell_protocol::host::{FromHost, ToHost, read_to_host, write_from_host};
    use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
    use std::ffi::OsString;
    use std::fs::File;
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::Duration;

    /// One writer for the up pipe, shared so frames from different threads never interleave.
    type Up = Arc<Mutex<File>>;

    fn send(up: &Up, msg: &FromHost) -> bool {
        match up.lock() {
            Ok(mut w) => write_from_host(&mut *w, msg).is_ok(),
            Err(_) => false,
        }
    }

    struct PtyInput {
        master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
        writer: Box<dyn Write + Send>,
    }

    impl pump::PtySink for PtyInput {
        fn input(&mut self, b: &[u8]) {
            let _ = self.writer.write_all(b).and_then(|()| self.writer.flush());
        }
        fn resize(&mut self, cols: u16, rows: u16) {
            if let Ok(guard) = self.master.lock()
                && let Some(master) = guard.as_ref()
            {
                let _ = master.resize(size(cols, rows));
            }
        }
    }

    fn size(cols: u16, rows: u16) -> PtySize {
        PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    /// Runs the whole session and returns the process exit code. Returning ends the process,
    /// which closes the job and kills anything still running.
    pub fn run(pipes: win::Pipes) -> i32 {
        let win::Pipes { mut down, up } = pipes;
        let up: Up = Arc::new(Mutex::new(up));

        let req = match read_to_host(&mut down) {
            Ok(Some(ToHost::Spawn(req))) => req,
            _ => return 1,
        };
        if req.argv.is_empty() {
            send(&up, &FromHost::Failed("Nothing to start: the command is empty.".into()));
            return 1;
        }
        if let Some(cwd) = &req.cwd
            && let Some(text) = missing_cwd_message(cwd)
        {
            send(&up, &FromHost::Failed(text));
            return 5;
        }

        let mut cmd = CommandBuilder::from_argv(req.argv.iter().map(OsString::from).collect());
        cmd.env_clear();
        for (k, v) in &req.env {
            cmd.env(k, v);
        }
        if let Some(cwd) = &req.cwd {
            cmd.cwd(cwd);
        }

        let started = (|| -> anyhow::Result<_> {
            let pty = native_pty_system().openpty(size(req.cols, req.rows))?;
            let child = pty.slave.spawn_command(cmd)?;
            // First thing after the start, so the shell's children join it too. Held until this
            // process ends, however it ends. If Windows refuses, the helper's own job still covers
            // every program that stays in it.
            let child_job = child
                .as_raw_handle()
                .map(win::Job::tie_child)
                .transpose()
                .unwrap_or_else(|e| {
                    eprintln!("{e}");
                    None
                });
            let reader = pty.master.try_clone_reader()?;
            let writer = pty.master.take_writer()?;
            Ok((pty.master, child, reader, writer, child_job))
        })();
        let (master, mut child, mut reader, writer, _child_job) = match started {
            Ok(parts) => parts,
            Err(e) => {
                send(&up, &FromHost::Failed(format!("{e:#}")));
                return 1;
            }
        };
        if !send(
            &up,
            &FromHost::Started {
                pid: child.process_id().unwrap_or(0),
            },
        ) {
            return 1;
        }

        let master = Arc::new(Mutex::new(Some(master)));

        // pty output to the core. The helper does not answer ESC[6n; the core does.
        let (out_done_tx, out_done_rx) = mpsc::channel::<()>();
        let out_up = Arc::clone(&up);
        std::thread::spawn(move || {
            let mut buf = [0u8; 16 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if !send(&out_up, &FromHost::Output(buf[..n].to_vec())) {
                            break;
                        }
                    }
                }
            }
            let _ = out_done_tx.send(());
        });

        // The child ends: close the pty so the last output drains, then report and exit.
        let wait_master = Arc::clone(&master);
        let wait_up = Arc::clone(&up);
        std::thread::spawn(move || {
            let code = child.wait().map(|s| s.exit_code() as i32).unwrap_or(-1);
            if let Ok(mut guard) = wait_master.lock() {
                guard.take();
            }
            let _ = out_done_rx.recv_timeout(Duration::from_secs(2));
            send(&wait_up, &FromHost::Exit { code });
            std::process::exit(0);
        });

        // The core's frames to the pty, until EOF or a protocol error.
        let mut sink = PtyInput { master, writer };
        let _ = pump::serve_frames(&mut down, &mut sink);
        0
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn a_missing_cwd_fails_with_the_copy() {
        assert_eq!(
            missing_cwd_message(r"Z:\no-such-marshell-folder").as_deref(),
            Some(
                r"The folder Z:\no-such-marshell-folder can't be reached as administrator. A mapped or substituted drive is visible only to your own sessions. Nothing was started."
            )
        );
        assert_eq!(missing_cwd_message(r"C:\Windows"), None);
    }
}
