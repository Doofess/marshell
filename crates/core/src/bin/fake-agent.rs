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
