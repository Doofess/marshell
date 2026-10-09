//! A deterministic child process for tests. Not shipped to users.
//!   fake-agent print <text>        print text + newline, exit 0
//!   fake-agent echo                for each stdin line print "got:<line>"; "exit" quits
//!   fake-agent flood <bytes>       write <bytes> of 80-char lines, exit 0
//!   fake-agent cwd                 print the current directory, exit 0
//!   fake-agent spawn-child         start a sleeping grandchild, print "child:<pid>", sleep
//!   fake-agent sleep               sleep 60 s
//!   fake-agent exit <code>         exit with <code>
use std::io::{BufRead, Write};

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
            let line = [b'x'; 79];
            let mut written = 0;
            while written < total {
                out.write_all(&line).unwrap();
                out.write_all(b"\n").unwrap();
                written += 80;
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
        Some("exit") => std::process::exit(args[1].parse().unwrap()),
        _ => eprintln!("unknown mode"),
    }
    out.flush().unwrap();
}
