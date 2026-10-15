//! Test helper for `tests/job_object.rs`: joins a job like Marshell does, starts a child, then waits
//! to be killed. Prints `ok` (or `err <message>`) and then the child's pid, one per line.

use std::io::Write;
use std::process::{Command, Stdio};

fn main() {
    match marshell_core::job::tie_children_to_app() {
        Ok(()) => println!("ok"),
        Err(e) => println!("err {e}"),
    }
    let mut child = Command::new("ping")
        .args(["-n", "60", "127.0.0.1"])
        .stdout(Stdio::null())
        .spawn()
        .expect("could not start ping");
    println!("{}", child.id());
    std::io::stdout().flush().unwrap();
    // Wait to be killed; ping itself runs about 60 s.
    let _ = child.wait();
}
