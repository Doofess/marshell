//! A fake core for the bridge tests: one connection, the raw request captured, a canned answer.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

pub struct Captured {
    pub request_line: String,
    /// Header names are lower-cased.
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Clone)]
pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
    /// Waited after the request is read, before the answer is written.
    pub delay: Duration,
}

impl Reply {
    pub fn new(status: u16, body: &str) -> Self {
        Self {
            status,
            body: body.as_bytes().to_vec(),
            delay: Duration::ZERO,
        }
    }

    pub fn delayed(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

pub struct Fake {
    pub port: u16,
    rx: Receiver<Captured>,
}

impl Fake {
    /// The request the fake saw, waiting up to `wait` for it.
    pub fn captured(&self, wait: Duration) -> Option<Captured> {
        self.rx.recv_timeout(wait).ok()
    }
}

/// Listens on `127.0.0.1:0` and serves exactly one connection in a thread.
pub fn serve(reply: Reply) -> Fake {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let Ok((mut conn, _)) = listener.accept() else {
            return;
        };
        let mut raw = Vec::new();
        let mut chunk = [0u8; 4096];
        let split = loop {
            if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
            match conn.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(n) => raw.extend_from_slice(&chunk[..n]),
            }
        };
        let head = String::from_utf8_lossy(&raw[..split]).into_owned();
        let mut lines = head.split("\r\n");
        let request_line = lines.next().unwrap_or("").to_string();
        let headers: HashMap<String, String> = lines
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
            .collect();
        let len: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
        let mut body = raw[split..].to_vec();
        while body.len() < len {
            match conn.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => body.extend_from_slice(&chunk[..n]),
            }
        }
        let _ = tx.send(Captured {
            request_line,
            headers,
            body,
        });
        std::thread::sleep(reply.delay);
        let head = format!(
            "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            reply.status,
            reply.body.len()
        );
        let _ = conn.write_all(head.as_bytes());
        let _ = conn.write_all(&reply.body);
    });
    Fake { port, rx }
}

/// A port nothing listens on.
pub fn refused_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

pub fn write_endpoint(home: &Path, port: u16, token: &str) {
    std::fs::create_dir_all(home.join("run")).unwrap();
    std::fs::write(
        home.join("run").join("endpoint.json"),
        format!(r#"{{"version":1,"port":{port},"token":"{token}","platform":"windows"}}"#),
    )
    .unwrap();
}

/// An environment made of `home` as `MARSHELL_HOME` and the extra pairs. Never touches the
/// process environment.
pub fn env_of(home: &Path, extra: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let mut map: HashMap<String, String> = extra.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    map.insert("MARSHELL_HOME".into(), home.to_string_lossy().into_owned());
    move |k| map.get(k).cloned()
}

pub fn argv(role: &str) -> Vec<String> {
    ["--agent", "claude", "--tag", "mshl1"]
        .iter()
        .fold(vec![role.to_string()], |mut v, s| {
            v.push(s.to_string());
            v
        })
}
