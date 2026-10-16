use marshell_core::git::{GitOutput, GitStep, display, last_line, run, run_steps, said};
use std::path::{Path, PathBuf};

fn step(dir: &Path, args: &[&str]) -> GitStep {
    GitStep {
        dir: dir.to_path_buf(),
        args: args.iter().map(|a| a.to_string()).collect(),
    }
}

#[test]
fn display_quotes_paths_with_spaces() {
    let s = GitStep {
        dir: PathBuf::from("C:\\Users\\Home Office\\app"),
        args: ["worktree", "add", "-b", "marshell/x", "C:\\a b\\x", "main"]
            .iter()
            .map(|a| a.to_string())
            .collect(),
    };
    assert_eq!(
        display(&s),
        r#"git -C "C:\Users\Home Office\app" worktree add -b marshell/x "C:\a b\x" main"#
    );
}

#[test]
fn display_quotes_empty_and_embedded_quotes() {
    let s = step(Path::new("repo"), &["commit", "-m", "", "say \"hi\"", "tab\there"]);
    assert_eq!(
        display(&s),
        "git -C repo commit -m \"\" \"say \\\"hi\\\"\" \"tab\there\""
    );
}

#[test]
fn run_reports_status_and_output() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("with space");
    std::fs::create_dir(&dir).unwrap();

    let init = run(&dir, &["init", "-q", "-b", "main"]).unwrap();
    assert!(init.ok);

    let head = run(&dir, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap();
    assert!(head.stdout == "HEAD\n" || head.stdout == "main\n", "{:?}", head.stdout);

    let bad = run(&dir, &["checkout", "nope"]).unwrap();
    assert!(!bad.ok);
    assert!(!last_line(&bad.stderr).is_empty());
}

#[test]
fn run_forbids_optional_index_locks() {
    let tmp = tempfile::tempdir().unwrap();
    // A shell alias echoes the environment git passes to its children.
    let out = run(
        tmp.path(),
        &["-c", "alias.showenv=!echo lock=$GIT_OPTIONAL_LOCKS", "showenv"],
    )
    .unwrap();
    assert!(out.ok, "{}", out.stderr);
    assert_eq!(out.stdout.trim(), "lock=0");
}

#[test]
fn last_line_is_the_last_non_empty_trimmed_line() {
    assert_eq!(last_line("first\n  second  \n\n"), "second");
    assert_eq!(last_line(""), "");
}

#[test]
fn run_steps_stops_at_first_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let steps = vec![
        step(dir, &["init", "-q", "-b", "main"]),
        step(dir, &["checkout", "nope"]),
        step(dir, &["init", "-q", "marker"]),
    ];
    let failure = run_steps(&steps).unwrap_err();
    assert_eq!(failure.index, 1);
    assert!(!failure.output.ok);
    assert!(!dir.join("marker").exists());
}

#[test]
fn run_steps_succeeds_when_every_step_is_ok() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let steps = vec![
        step(dir, &["init", "-q", "-b", "main"]),
        step(dir, &["init", "-q", "marker"]),
    ];
    run_steps(&steps).unwrap();
    assert!(dir.join("marker").exists());
}

#[test]
fn display_quotes_line_breaks() {
    let s = step(Path::new("repo"), &["commit", "-m", "one\ntwo", "a\rb"]);
    assert_eq!(display(&s), "git -C repo commit -m \"one\ntwo\" \"a\rb\"");
}

fn output(stdout: &str, stderr: &str) -> GitOutput {
    GitOutput {
        ok: false,
        stdout: stdout.into(),
        stderr: stderr.into(),
    }
}

#[test]
fn said_prefers_the_last_error_or_fatal_line() {
    let out = output(
        "",
        "error: The following untracked working tree files would be overwritten by merge:\n\tn.txt\nPlease move or remove them.\nAborting\n",
    );
    assert_eq!(
        said(&out),
        "The following untracked working tree files would be overwritten by merge:"
    );
    let out = output("", "error: first\nfatal:  second \nhint: try again\n");
    assert_eq!(said(&out), "second");
}

#[test]
fn said_falls_back_to_the_last_line_then_stdout_then_a_sentence() {
    assert_eq!(said(&output("", "hint: x\n  plain  \n\n")), "plain");
    assert_eq!(said(&output("nothing added\n\n", "")), "nothing added");
    assert_eq!(said(&output("", "  \n")), "it failed without saying why");
}
