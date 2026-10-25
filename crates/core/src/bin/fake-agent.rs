//! A deterministic child process for tests. Not shipped to users.
//!   fake-agent print <text>        print text + newline, exit 0
//!   fake-agent echo                for each stdin line print "got:<line>"; "exit" quits
//!   fake-agent flood <bytes>       write <bytes> of 80-char lines, exit 0
//!   fake-agent cwd                 print the current directory, exit 0
//!   fake-agent spawn-child         start a sleeping grandchild, print "child:<pid>", sleep
//!   fake-agent sleep               sleep 60 s
//!   fake-agent sleep-then-exit <s> sleep <s> seconds, exit 0
//!   fake-agent exit <code>         exit with <code>
//!   fake-agent osc133              print the OSC 133 marks A, C and D;7, exit 0
//!   fake-agent folder <path>       report <path> as the shell's folder (OSC 9;9), sleep 60 s
//!   fake-agent cmdline <text>      report <text> as the command line (633;E), mark C, sleep 2 s, mark D;0
//!   fake-agent secret-run <line> <out>  report <line> (633;E), mark C, print <out>, mark D and A, print "visible after"
//!   fake-agent env-present <names> print "NAME=present" or "NAME=absent" for each name (never a value), exit 0;
//!                                  the names end at a "-c" word, where a launch line starts
//!   fake-agent capture-input <f>   read stdin raw, turn bracketed paste on (2004h), append every byte read to <f>
use std::io::{BufRead, Read, Write};

/// Raw, unechoed input, so what the core types arrives byte for byte.
#[cfg(windows)]
fn raw_stdin() {
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> isize;
        fn SetConsoleMode(handle: isize, mode: u32) -> i32;
    }
    const STD_INPUT_HANDLE: u32 = -10i32 as u32;
    const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x200;
    // SAFETY: both calls take plain values; a failure only leaves the console in line mode.
    unsafe { SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), ENABLE_VIRTUAL_TERMINAL_INPUT) };
}

#[cfg(not(windows))]
fn raw_stdin() {
    let _ = std::process::Command::new("stty").args(["raw", "-echo"]).status();
}

/// `text` as the shell integration escapes a command line in OSC 633;E.
fn escape_633(text: &str) -> String {
    let mut esc = String::new();
    for c in text.chars() {
        match c {
            '\\' => esc.push_str("\\\\"),
            ';' => esc.push_str("\\x3b"),
            c if (c as u32) < 0x20 => esc.push_str(&format!("\\x{:02x}", c as u32)),
            c => esc.push(c),
        }
    }
    esc
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = std::io::stdout().lock();
    match args.first().map(String::as_str) {
        Some("print") => writeln!(out, "{}", args[1..].join(" ")).unwrap(),
        Some("echo") => {
            for line in std::io::stdin().lock().lines() {
                let line = line.unwrap();
                let line = line.trim();
                if line == "exit" {
                    break;
                }
                writeln!(out, "got:{line}").unwrap();
                out.flush().unwrap();
            }
        }
        Some("flood") => {
            let total: usize = args[1].parse().unwrap();
            // Write whole 64 KiB chunks, not one line at a time: stdout is line-buffered, and
            // ~650k 80-byte console writes into ConPTY would measure this agent, not the pipeline.
            let mut line = [b'x'; 80];
            line[79] = b'\n';
            let lines_per_chunk = 64 * 1024 / line.len();
            let chunk = line.repeat(lines_per_chunk);
            // Same bytes as before: `total` rounded up to whole lines.
            let mut lines_left = total.div_ceil(line.len());
            while lines_left > 0 {
                let n = lines_left.min(lines_per_chunk);
                out.write_all(&chunk[..n * line.len()]).unwrap();
                lines_left -= n;
            }
        }
        Some("env-present") => {
            for name in args[1..].iter().take_while(|a| a.as_str() != "-c") {
                let state = if std::env::var_os(name).is_some() {
                    "present"
                } else {
                    "absent"
                };
                writeln!(out, "{name}={state}").unwrap();
            }
        }
        Some("cwd") => writeln!(out, "{}", std::env::current_dir().unwrap().display()).unwrap(),
        Some("spawn-child") => {
            let me = std::env::current_exe().unwrap();
            #[allow(clippy::zombie_processes)] // the grandchild is meant to outlive us
            let child = std::process::Command::new(me).arg("sleep").spawn().unwrap();
            writeln!(out, "child:{}", child.id()).unwrap();
            out.flush().unwrap();
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
        Some("sleep") => std::thread::sleep(std::time::Duration::from_secs(60)),
        Some("sleep-then-exit") => std::thread::sleep(std::time::Duration::from_secs(args[1].parse().unwrap())),
        Some("exit") => std::process::exit(args[1].parse().unwrap()),
        Some("osc133") => write!(out, "\x1b]133;A\x07$ \x1b]133;C\x07out\r\n\x1b]133;D;7\x1b\\").unwrap(),
        Some("folder") => {
            write!(out, "\x1b]9;9;{}\x1b\\", args[1]).unwrap();
            out.flush().unwrap();
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
        Some("cmdline") => {
            // Like the PowerShell integration: report the typed line, mark C, run 2 s, mark D.
            // A literal backslash-n in the text stands for a newline.
            let text = args[1..].join(" ").replace("\\n", "\n");
            let esc = escape_633(&text);
            write!(out, "\x1b]633;E;{esc}\x1b\\\x1b]133;C\x07").unwrap();
            out.flush().unwrap();
            std::thread::sleep(std::time::Duration::from_secs(2));
            write!(out, "\x1b]133;D;0\x07").unwrap();
        }
        Some("secret-run") => {
            // A shell running a secret command: report the line, mark C, print the secret, mark D and A, then
            // plain output. The pauses make the pieces separate reads, though the core must not depend on that.
            let pause = || std::thread::sleep(std::time::Duration::from_millis(400));
            write!(out, "$ \x1b]633;E;{}\x1b\\\x1b]133;C\x07", escape_633(&args[1])).unwrap();
            out.flush().unwrap();
            pause();
            write!(out, "{}\r\n", args[2]).unwrap();
            out.flush().unwrap();
            pause();
            write!(out, "\x1b]133;D;0\x07\x1b]133;A\x07").unwrap();
            out.flush().unwrap();
            pause();
            write!(out, "visible after\r\n").unwrap();
            out.flush().unwrap();
            pause();
        }
        Some("capture-input") => {
            raw_stdin();
            write!(out, "[?2004h").unwrap();
            out.flush().unwrap();
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&args[1])
                .unwrap();
            let mut buf = [0u8; 256];
            let mut stdin = std::io::stdin().lock();
            while let Ok(n) = stdin.read(&mut buf) {
                if n == 0 {
                    break;
                }
                file.write_all(&buf[..n]).unwrap();
                file.flush().unwrap();
            }
        }
        _ => eprintln!("unknown mode"),
    }
    out.flush().unwrap();
}
