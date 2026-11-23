//! A stand-in for the GitHub CLI in tests. It is never named `gh.exe`, and tests find it by its full path.
//!
//! It reads `script.json` in its working folder: a list of `{"match": [argument substrings], "exit": n,
//! "stdout": "...", "stderr": "..."}`. The first entry whose substrings all appear in the arguments (joined by a
//! space) answers: it prints its `stdout` and `stderr` and exits with `exit`. With no match it exits with 2.
//! An entry may also hold `"json": <value>`, printed as its `stdout` when it has none; `"sleep_ms": n`, which
//! waits that long before it answers; and `"call": n`, which makes it answer only the n-th call (counted from 1)
//! that its `match` fits, so a later call can get another answer than the first.
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
    let fits = |entry: &Value, joined: &str| {
        entry["match"]
            .as_array()
            .is_some_and(|parts| parts.iter().all(|p| p.as_str().is_some_and(|p| joined.contains(p))))
    };
    // The calls so far, this one included (it was appended above), as the lines of `calls.jsonl`.
    let calls: Vec<String> = std::fs::read_to_string("calls.jsonl")
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .map(|call| {
                    call["args"]
                        .as_array()
                        .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "))
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();
    let found = script.iter().find(|entry| {
        fits(entry, &joined)
            && entry["call"]
                .as_u64()
                .is_none_or(|n| calls.iter().filter(|c| fits(entry, c)).count() as u64 == n)
    });
    let Some(entry) = found else {
        eprintln!("gh-marshell-test: no script entry matches: {joined}");
        std::process::exit(2);
    };
    if let Some(ms) = entry["sleep_ms"].as_u64() {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    match (entry["stdout"].as_str(), entry.get("json")) {
        (Some(text), _) => print!("{text}"),
        (None, Some(json)) => print!("{json}"),
        (None, None) => {}
    }
    eprint!("{}", entry["stderr"].as_str().unwrap_or(""));
    let _ = std::io::stdout().flush();
    std::process::exit(entry["exit"].as_i64().unwrap_or(0) as i32);
}
