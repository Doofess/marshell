//! A stand-in for the GitHub CLI in tests. It is never named `gh.exe`, and tests find it by its full path.
//!
//! It reads `script.json` in its working folder: a list of `{"match": [argument substrings], "exit": n,
//! "stdout": "...", "stderr": "..."}`. The first entry whose substrings all appear in the arguments (joined by a
//! space) answers: it prints its `stdout` and `stderr` and exits with `exit`. With no match it exits with 2.
//! Every call, matched or not, is first appended to `calls.jsonl` there as `{"args": [...], "stdin": "...",
//! "env_names": [...]}`: the arguments, everything read from standard input, and the names (never the values) of its
//! environment, sorted.
use serde_json::{Value, json};
use std::io::{Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdin = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut stdin);
    let mut env_names: Vec<String> = std::env::vars_os()
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect();
    env_names.sort();
    let call = json!({
        "args": args,
        "stdin": String::from_utf8_lossy(&stdin),
        "env_names": env_names,
    });
    if let Ok(mut calls) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("calls.jsonl")
    {
        let _ = writeln!(calls, "{call}");
    }
    let script: Vec<Value> = std::fs::read_to_string("script.json")
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let joined = args.join(" ");
    let found = script.iter().find(|entry| {
        entry["match"]
            .as_array()
            .is_some_and(|parts| parts.iter().all(|p| p.as_str().is_some_and(|p| joined.contains(p))))
    });
    let Some(entry) = found else {
        eprintln!("gh-marshell-test: no script entry matches: {joined}");
        std::process::exit(2);
    };
    print!("{}", entry["stdout"].as_str().unwrap_or(""));
    eprint!("{}", entry["stderr"].as_str().unwrap_or(""));
    let _ = std::io::stdout().flush();
    std::process::exit(entry["exit"].as_i64().unwrap_or(0) as i32);
}
