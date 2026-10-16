use marshell_core::git::{GitStep, display, last_line, run, run_steps};
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
