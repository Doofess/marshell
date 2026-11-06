//! Starts one MCP server, lists its tools over stdio and ends its whole process tree, whatever happened.
//! The server's output is only parsed, never kept: its stderr is dropped and a failure names its kind, not its text.

use crate::job::ProcessTree;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// More than this read from a server in all is not an MCP server.
const MAX_BYTES: usize = 16 * 1024 * 1024;
/// A server that keeps sending `nextCursor` is listed no further than this.
const MAX_PAGES: usize = 50;
const PROTOCOL_VERSION: &str = "2025-06-18";

/// A server ready to start: the program already resolved, the environment complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// The whole environment of the server: nothing is inherited.
    pub env: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
    /// One deadline for the whole exchange, from the start.
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// The characters of the tool's `inputSchema` as compact JSON.
    pub schema_chars: usize,
    /// The `_meta` keys whose value is `true`.
    pub meta_true: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub instructions: String,
    pub tools: Vec<Tool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The program did not start.
    CantStart,
    /// The server ended before it answered, with this exit code.
    Exited(Option<i32>),
    /// No answer within this many seconds.
    NoAnswer(u64),
    /// What the server wrote is not MCP, or is too much.
    NotMcp,
    /// The server answered `initialize` or `tools/list` with an error.
    Refused,
}

/// What the stdout reader thread passes on.
enum Out {
    Line(String),
    /// The pipe closed.
    Eof,
    /// More than `MAX_BYTES` came.
    TooMuch,
}

/// Starts the server, lists its tools and ends its whole process tree, whatever happened.
pub fn probe(ready: &Ready) -> Result<Listed, Failure> {
    let started = Instant::now();
    let mut cmd = Command::new(&ready.program);
    cmd.args(&ready.args)
        .env_clear()
        .envs(ready.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = &ready.cwd {
        cmd.current_dir(cwd);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console window flashes.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|_| Failure::CantStart)?;
    let tree = ProcessTree::adopt(&child);
    let result = match (child.stdin.take(), child.stdout.take(), child.stderr.take()) {
        (Some(stdin), Some(stdout), Some(stderr)) => {
            std::thread::spawn(move || {
                let _ = std::io::copy(&mut { stderr }, &mut std::io::sink());
            });
            exchange(&mut child, stdin, stdout, started + ready.timeout, ready.timeout)
        }
        _ => Err(Failure::CantStart),
    };
    // Whatever happened, the whole tree ends. The reader threads end once the processes have closed their pipes.
    tree.kill(&mut child);
    let _ = child.wait();
    result
}

/// The seconds of `timeout`, rounded up.
fn whole_secs(timeout: Duration) -> u64 {
    timeout.as_secs() + u64::from(timeout.subsec_nanos() > 0)
}

fn exchange(
    child: &mut Child,
    stdin: std::process::ChildStdin,
    stdout: std::process::ChildStdout,
    deadline: Instant,
    timeout: Duration,
) -> Result<Listed, Failure> {
    let no_answer = Failure::NoAnswer(whole_secs(timeout));
    // Writes go through a thread, so a server that doesn't read its stdin can't hold the probe past the deadline.
    let (to_server, writes) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut stdin = stdin;
        for line in writes {
            if writeln!(stdin, "{line}").and_then(|()| stdin.flush()).is_err() {
                break;
            }
        }
    });
    let send = |value: Value| {
        let _ = to_server.send(value.to_string());
    };
    let lines = read_lines(stdout);

    send(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": "marshell-doctor", "version": env!("CARGO_PKG_VERSION")},
        },
    }));

    let mut saw_garbage = false;
    let mut instructions: Option<String> = None;
    let mut next_id = 1u64;
    let mut pages = 0usize;
    let mut tools: Vec<Tool> = Vec::new();
    loop {
        let line = match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Out::Line(line)) => line,
            Ok(Out::TooMuch) => return Err(Failure::NotMcp),
            Ok(Out::Eof) | Err(RecvTimeoutError::Disconnected) => {
                if saw_garbage {
                    return Err(Failure::NotMcp);
                }
                // The pipe closed; the exit code follows shortly, if the process is ending at all.
                while Instant::now() < deadline {
                    if let Ok(Some(status)) = child.try_wait() {
                        return Err(Failure::Exited(status.code()));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                return Err(no_answer);
            }
            Err(RecvTimeoutError::Timeout) => return Err(no_answer),
        };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            saw_garbage = true;
            continue;
        };
        let Some(object) = msg.as_object() else {
            saw_garbage = true;
            continue;
        };
        if object.contains_key("method") {
            // A request from the server gets "Method not found"; a notification gets nothing.
            if let Some(id) = object.get("id") {
                send(json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "Method not found"}}));
            }
            continue;
        }
        if object.get("id") != Some(&json!(next_id)) {
            continue;
        }
        if object.contains_key("error") {
            return Err(Failure::Refused);
        }
        let Some(result) = object.get("result").and_then(Value::as_object) else {
            continue;
        };
        if instructions.is_none() {
            instructions = Some(
                result
                    .get("instructions")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            );
            send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
            if result
                .get("capabilities")
                .and_then(|c| c.get("tools"))
                .is_none_or(Value::is_null)
            {
                break;
            }
        } else {
            tools.extend(
                result
                    .get("tools")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(tool_of),
            );
            pages += 1;
            match result.get("nextCursor").and_then(Value::as_str) {
                Some(cursor) if pages < MAX_PAGES => {
                    next_id += 1;
                    send(
                        json!({"jsonrpc": "2.0", "id": next_id, "method": "tools/list", "params": {"cursor": cursor}}),
                    );
                    continue;
                }
                _ => break,
            }
        }
        next_id += 1;
        send(json!({"jsonrpc": "2.0", "id": next_id, "method": "tools/list", "params": {}}));
    }
    Ok(Listed {
        instructions: instructions.unwrap_or_default(),
        tools,
    })
}

fn tool_of(tool: &Value) -> Option<Tool> {
    let name = tool.get("name")?.as_str()?.to_string();
    Some(Tool {
        name,
        description: tool
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        schema_chars: tool
            .get("inputSchema")
            .and_then(|schema| serde_json::to_string(schema).ok())
            .map_or(0, |s| s.chars().count()),
        meta_true: tool
            .get("_meta")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter(|(_, v)| v.as_bool() == Some(true))
            .map(|(k, _)| k.clone())
            .collect(),
    })
}

/// Reads the server's stdout on a thread, a line at a time. The receiver is dropped when the probe is done and the
/// thread is not waited for: it ends once the killed processes have closed the pipe. At most `MAX_BYTES` are read,
/// a single line without a newline included.
fn read_lines(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<Out> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut total = 0usize;
        let mut line: Vec<u8> = Vec::new();
        let mut chunk = vec![0u8; 64 * 1024];
        loop {
            let n = match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => {
                    if !line.is_empty() {
                        let _ = tx.send(Out::Line(String::from_utf8_lossy(&line).into_owned()));
                    }
                    let _ = tx.send(Out::Eof);
                    return;
                }
                Ok(n) => n,
            };
            total += n;
            if total > MAX_BYTES {
                let _ = tx.send(Out::TooMuch);
                return;
            }
            for piece in chunk[..n].split_inclusive(|b| *b == b'\n') {
                line.extend_from_slice(piece);
                if line.last() == Some(&b'\n') {
                    let text = String::from_utf8_lossy(&line);
                    if tx
                        .send(Out::Line(text.trim_end_matches(['\r', '\n']).to_string()))
                        .is_err()
                    {
                        return;
                    }
                    line.clear();
                }
            }
        }
    });
    rx
}
