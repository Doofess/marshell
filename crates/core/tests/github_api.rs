//! The one place Marshell runs the GitHub CLI, run against a fake: no test reaches the network or runs the real `gh`.

mod common;

use marshell_core::github::gh;
use serde_json::{Value, json};
use std::path::Path;

#[test]
fn gh_runs_in_marshells_folder_with_the_body_on_stdin() {
    common::tie_children_to_test();
    let home = tempfile::tempdir().unwrap();
    let gh_dir = home.path().join("gh");
    std::fs::create_dir_all(&gh_dir).unwrap();
    let script = json!([
        {"match": ["api", "--hostname", "github.com", "repos/octo-org/widgets/pulls"], "exit": 0,
         "stdout": "{\"number\":7}", "stderr": ""},
    ]);
    std::fs::write(gh_dir.join("script.json"), script.to_string()).unwrap();

    let fake = Path::new(env!("CARGO_BIN_EXE_gh-marshell-test"));
    let body = json!({"title": "Add the widget", "body": "a-body-that-must-not-be-an-argument"});
    let got = gh::api(
        fake,
        home.path(),
        "github.com",
        &["-X", "POST", "repos/octo-org/widgets/pulls"],
        Some(&body),
    )
    .unwrap();
    assert_eq!(got, json!({"number": 7}));

    let calls = std::fs::read_to_string(gh_dir.join("calls.jsonl")).expect("the fake ran in <home>/gh");
    let lines: Vec<&str> = calls.lines().collect();
    assert_eq!(lines.len(), 1, "{calls}");
    let call: Value = serde_json::from_str(lines[0]).unwrap();
    let args: Vec<&str> = call["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert_eq!(
        args,
        [
            "api",
            "--hostname",
            "github.com",
            "-X",
            "POST",
            "repos/octo-org/widgets/pulls",
            "--input",
            "-"
        ]
    );
    let stdin: Value = serde_json::from_str(call["stdin"].as_str().unwrap()).unwrap();
    assert_eq!(stdin, body);
    // The environment names it got: the prompts are off, and nothing of Marshell's own is passed along.
    let names: Vec<&str> = call["env_names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert!(names.contains(&"GH_PROMPT_DISABLED"), "{names:?}");
    assert!(
        names
            .iter()
            .all(|n| !n.starts_with("GIT_") && !n.starts_with("MARSHELL")),
        "{names:?}"
    );
}
