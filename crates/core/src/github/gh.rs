//! The one place Marshell runs the GitHub CLI: `gh api …` and `gh auth status --json hosts`, nothing else.
//!
//! gh is found by its full path, runs in `<home>/gh` (never a checkout) with a short environment of its own, and gets
//! a request body on standard input, never on the command line. It is given 15 seconds and 4 MiB of answer.

use crate::agents::{Bounded, run_bounded_with};
use crate::server::{links_refused, refused};
use serde_json::Value;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// How long one gh call may take, whole: its start, its answer and the end of its pipes.
pub const GH_DEADLINE: Duration = Duration::from_secs(15);
/// The most gh may print, standard output and standard error together.
pub const GH_MAX_BYTES: usize = 4 * 1024 * 1024;

/// Why a gh call gave no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GhError {
    /// gh isn't installed, or couldn't start.
    Missing,
    /// gh holds no sign-in, or GitHub refused its token.
    SignedOut,
    /// GitHub couldn't be reached; the reason is what gh printed last.
    Unreachable(String),
    /// The repository or the thing asked for doesn't exist, or isn't visible to this account.
    NotFound,
    /// GitHub's rate limit. `reset_ms` is when it lifts, when gh said (it doesn't print the header, so never today).
    RateLimited { reset_ms: Option<u64> },
    /// GitHub understood and said no, with its own message.
    Refused { status: Option<u16>, message: String },
    /// gh didn't finish in `GH_DEADLINE`, or printed more than `GH_MAX_BYTES`.
    TooLong,
    /// gh succeeded, but what it printed isn't JSON.
    Unreadable,
}

/// The file `name` (no ending) in the first folder of `path` that is absolute, passes `refused` and `links_refused`,
/// and holds it: `<name>.exe` on Windows, `<name>` elsewhere. Unlike `command_path_in`, a `.cmd`, a `.bat` or a bare
/// file on Windows is never taken, and a relative folder (which would mean the working folder) is passed over.
pub fn find_exe(path: &OsStr, name: &str) -> Option<PathBuf> {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    std::env::split_paths(path).find_map(|dir| {
        if !dir.is_absolute() || refused(dir.to_str()?) || links_refused(&dir) {
            return None;
        }
        let candidate = dir.join(&file);
        candidate.is_file().then_some(candidate)
    })
}

/// gh on `path`, which the caller takes from `detect::tab_path()`: the PATH a session gets.
pub fn find_gh(path: &OsStr) -> Option<PathBuf> {
    find_exe(path, "gh")
}

/// What gh keeps of the caller's environment.
const KEPT: &[&str] = &[
    "SystemRoot",
    "SystemDrive",
    "windir",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "ProgramData",
    "HOME",
    "XDG_CONFIG_HOME",
    "GH_CONFIG_DIR",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

/// Whether the variable name `name` is `want`: without regard to case on Windows, where names have none.
fn same_name(name: &OsStr, want: &str) -> bool {
    if cfg!(windows) {
        name.eq_ignore_ascii_case(want)
    } else {
        name == OsStr::new(want)
    }
}

/// The environment gh runs in: nothing of `vars` but `KEPT` (so no `GH_HOST`, `GH_REPO`, `GH_DEBUG`, `GH_PAGER`,
/// `PAGER` and no `GIT_*`), each with the name it came with; `PATH` set to `gh_dir` (the folder gh itself is in) and,
/// on Windows, `System32` under the kept `SystemRoot`; and gh's prompts, spinner, colours and update notices off.
pub fn gh_env(vars: impl Iterator<Item = (OsString, OsString)>, gh_dir: &Path) -> Vec<(OsString, OsString)> {
    let mut env: Vec<(OsString, OsString)> = vars
        .filter(|(name, _)| KEPT.iter().any(|kept| same_name(name, kept)))
        .collect();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if !gh_dir.as_os_str().is_empty() {
        dirs.push(gh_dir.to_path_buf());
    }
    if cfg!(windows)
        && let Some((_, root)) = env.iter().find(|(name, _)| same_name(name, "SystemRoot"))
    {
        dirs.push(Path::new(root).join("System32"));
    }
    env.push((OsString::from("PATH"), std::env::join_paths(dirs).unwrap_or_default()));
    for (name, value) in [
        ("GH_PROMPT_DISABLED", "1"),
        ("GH_NO_UPDATE_NOTIFIER", "1"),
        ("GH_NO_EXTENSION_UPDATE_NOTIFIER", "1"),
        ("GH_SPINNER_DISABLED", "1"),
        ("NO_COLOR", "1"),
        ("CLICOLOR", "0"),
    ] {
        env.push((OsString::from(name), OsString::from(value)));
    }
    env
}

/// Runs gh with `args` in `<home>/gh`, with `input` on standard input, and returns its exit code and both streams.
fn run_gh(gh: &Path, home: &Path, args: &[&str], input: Option<Vec<u8>>) -> Result<(i32, Vec<u8>, Vec<u8>), GhError> {
    let dir = crate::paths::gh_dir(home);
    let dir = std::path::absolute(&dir).unwrap_or(dir);
    std::fs::create_dir_all(&dir)
        .map_err(|e| GhError::Unreachable(format!("Marshell's folder for GitHub couldn't be made: {e}")))?;
    let mut cmd = Command::new(gh);
    cmd.args(args)
        .current_dir(&dir)
        .env_clear()
        .envs(gh_env(std::env::vars_os(), gh.parent().unwrap_or(Path::new(""))));
    match run_bounded_with(cmd, GH_DEADLINE, input, GH_MAX_BYTES) {
        Bounded::NotStarted => Err(GhError::Missing),
        Bounded::Deadline | Bounded::TooMuch => Err(GhError::TooLong),
        Bounded::Done(code, stdout, stderr) => Ok((code, stdout, stderr)),
    }
}

/// `gh api --hostname <host> <args…>`, with `body` as JSON on standard input (`--input -`) when there is one. The
/// answer is the JSON gh printed. Nothing is cached: finding gh is the caller's.
pub fn api(gh: &Path, home: &Path, host: &str, args: &[&str], body: Option<&Value>) -> Result<Value, GhError> {
    let mut all = vec!["api", "--hostname", host];
    all.extend_from_slice(args);
    let input = match body {
        Some(body) => {
            all.extend(["--input", "-"]);
            Some(serde_json::to_vec(body).map_err(|_| GhError::Unreadable)?)
        }
        None => None,
    };
    let (code, stdout, stderr) = run_gh(gh, home, &all, input)?;
    if code != 0 {
        return Err(map_failure(code, &stdout, &stderr));
    }
    let value: Value = serde_json::from_slice(&stdout).map_err(|_| GhError::Unreadable)?;
    // A GraphQL answer can carry errors with exit code 0.
    if value
        .get("errors")
        .and_then(Value::as_array)
        .is_some_and(|errors| !errors.is_empty())
    {
        return Err(map_failure(code, &stdout, &stderr));
    }
    Ok(value)
}

/// The hosts that hold an entry with `"state": "success"` in the output of `gh auth status --json hosts`, sorted.
/// `None` when it isn't that shape.
pub(crate) fn hosts_from(stdout: &[u8]) -> Option<Vec<String>> {
    let json: Value = serde_json::from_slice(stdout).ok()?;
    let hosts = json.get("hosts")?.as_object()?;
    let mut signed_in: Vec<String> = hosts
        .iter()
        .filter(|(_, entries)| {
            entries.as_array().is_some_and(|e| {
                e.iter()
                    .any(|e| e.get("state").and_then(Value::as_str) == Some("success"))
            })
        })
        .map(|(host, _)| host.clone())
        .collect();
    signed_in.sort();
    Some(signed_in)
}

/// The hosts gh holds a working sign-in for: `gh auth status --json hosts`.
pub fn auth_hosts(gh: &Path, home: &Path) -> Result<Vec<String>, GhError> {
    let (code, stdout, stderr) = run_gh(gh, home, &["auth", "status", "--json", "hosts"], None)?;
    if code != 0 {
        return Err(map_failure(code, &stdout, &stderr));
    }
    hosts_from(&stdout).ok_or(GhError::Unreadable)
}

/// The status of gh's `gh: <message> (HTTP <code>)` line, from the last one in `stderr`.
fn http_status(stderr: &str) -> Option<u16> {
    let at = stderr.rfind("(HTTP ")?;
    let rest = &stderr[at + "(HTTP ".len()..];
    let (digits, _) = rest.split_once(')')?;
    digits.trim().parse().ok()
}

/// The last non-empty line of `text`, trimmed.
fn last_line(text: &str) -> &str {
    text.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or("")
}

/// Why a gh call failed, from its exit code and what it printed. In this order: exit code 4 and an `(HTTP 401)` on
/// standard error are a missing sign-in (standard output is never looked at for it: it holds GitHub's data, a pull
/// request's title for one); a failure with nothing on standard output is GitHub out of reach; a GraphQL
/// `NOT_FOUND` or `(HTTP 404)` is not found; `(HTTP 403)` saying "rate limit", or a GraphQL `RATE_LIMITED`, is the
/// rate limit; anything else is a refusal with GitHub's message. A GraphQL answer with exit code 0 and `errors` is
/// mapped the same way.
pub fn map_failure(code: i32, stdout: &[u8], stderr: &[u8]) -> GhError {
    let stderr = String::from_utf8_lossy(stderr);
    if code == 4 || stderr.contains("(HTTP 401)") {
        return GhError::SignedOut;
    }
    if code != 0 && stdout.iter().all(u8::is_ascii_whitespace) {
        return GhError::Unreachable(match last_line(&stderr) {
            "" => format!("gh stopped with exit code {code}."),
            line => line.to_string(),
        });
    }
    let json: Option<Value> = serde_json::from_slice(stdout).ok();
    let first = json.as_ref().and_then(|j| j.get("errors")).and_then(|e| e.get(0));
    let kind = first.and_then(|e| e.get("type")).and_then(Value::as_str);
    let status = http_status(&stderr);
    if kind == Some("NOT_FOUND") || status == Some(404) {
        return GhError::NotFound;
    }
    let message = first
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .or_else(|| json.as_ref().and_then(|j| j.get("message")).and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| {
            // gh's own line, without its `gh: ` and ` (HTTP nnn)`.
            let line = last_line(&stderr);
            let line = line.strip_prefix("gh: ").unwrap_or(line);
            let line = line.rfind(" (HTTP ").map_or(line, |at| &line[..at]);
            if line.is_empty() {
                "GitHub refused the request.".to_string()
            } else {
                line.to_string()
            }
        });
    if kind == Some("RATE_LIMITED")
        || (status == Some(403)
            && (stderr.to_lowercase().contains("rate limit") || message.to_lowercase().contains("rate limit")))
    {
        return GhError::RateLimited { reset_ms: None };
    }
    GhError::Refused { status, message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{Bounded, run_bounded_with};
    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Duration;

    const EXE: &str = if cfg!(windows) { "gh.exe" } else { "gh" };

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, b"x").unwrap();
        p
    }

    fn join(dirs: &[PathBuf]) -> OsString {
        std::env::join_paths(dirs).unwrap()
    }

    #[test]
    fn finds_only_an_exe_on_an_absolute_path_dir() {
        let early = tempfile::tempdir().unwrap();
        let good = tempfile::tempdir().unwrap();
        // A folder under the working folder, reached by a relative name.
        let rel = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        touch(early.path(), "gh.cmd");
        touch(rel.path(), EXE);
        let want = touch(good.path(), EXE);
        let rel_name = PathBuf::from(rel.path().file_name().unwrap());
        let path = join(&[early.path().to_path_buf(), rel_name.clone(), good.path().to_path_buf()]);
        assert_eq!(find_gh(&path), Some(want));
        // Only the relative folder holds one: not found.
        assert_eq!(find_gh(&join(&[rel_name])), None);
        assert_eq!(find_gh(OsStr::new("")), None);
        // The same walk finds another program by name.
        let git = touch(good.path(), if cfg!(windows) { "git.exe" } else { "git" });
        assert_eq!(find_exe(&join(&[good.path().to_path_buf()]), "git"), Some(git));
    }

    fn os(s: &str) -> OsString {
        OsString::from(s)
    }

    #[test]
    fn env_keeps_the_allowlist_and_sets_no_prompt() {
        let vars = [
            ("GIT_DIR", "x"),
            ("GIT_WORK_TREE", "x"),
            ("GH_REPO", "octo-org/widgets"),
            ("GH_DEBUG", "api"),
            ("GH_HOST", "example.com"),
            ("GH_PAGER", "less"),
            ("PAGER", "less"),
            ("GH_TOKEN", "t0ken"),
            ("HOME", "/home/u"),
            ("HTTPS_PROXY", "http://proxy:3128"),
            ("no_proxy", "localhost"),
            ("Path", "C:\\other"),
            ("systemroot", "C:\\Windows"),
            ("SECRET_THING", "no"),
        ];
        let kept = gh_env(vars.iter().map(|(k, v)| (os(k), os(v))), Path::new("/tools/gh"));
        let get = |name: &str| kept.iter().find(|(k, _)| k == OsStr::new(name)).map(|(_, v)| v.clone());
        for gone in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GH_REPO",
            "GH_DEBUG",
            "GH_HOST",
            "GH_PAGER",
            "PAGER",
            "SECRET_THING",
        ] {
            assert_eq!(get(gone), None, "{gone}");
        }
        assert_eq!(get("GH_TOKEN"), Some(os("t0ken")));
        assert_eq!(get("HOME"), Some(os("/home/u")));
        assert_eq!(get("HTTPS_PROXY"), Some(os("http://proxy:3128")));
        assert_eq!(get("no_proxy"), Some(os("localhost")));
        for (name, value) in [
            ("GH_PROMPT_DISABLED", "1"),
            ("GH_NO_UPDATE_NOTIFIER", "1"),
            ("GH_NO_EXTENSION_UPDATE_NOTIFIER", "1"),
            ("GH_SPINNER_DISABLED", "1"),
            ("NO_COLOR", "1"),
            ("CLICOLOR", "0"),
        ] {
            assert_eq!(get(name), Some(os(value)), "{name}");
        }
        // PATH is gh's own folder, and on Windows System32 of the kept SystemRoot; the caller's is gone.
        let path = kept
            .iter()
            .find(|(k, _)| k == OsStr::new("PATH"))
            .map(|(_, v)| v.clone())
            .expect("PATH is set");
        let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(dirs[0], PathBuf::from("/tools/gh"));
        assert!(!path.to_string_lossy().contains("other"), "{path:?}");
        assert_eq!(kept.iter().filter(|(k, _)| k.eq_ignore_ascii_case("path")).count(), 1);
        if cfg!(windows) {
            assert_eq!(dirs[1], PathBuf::from("C:\\Windows\\System32"));
            // Names match without case, and the kept one keeps the name it came with.
            assert_eq!(get("systemroot"), Some(os("C:\\Windows")));
        } else {
            assert_eq!(get("systemroot"), None);
        }
    }

    #[test]
    fn exit_4_is_signed_out() {
        let err = b"To get started with GitHub CLI, please run:  gh auth login\nAlternatively, populate the GH_TOKEN environment variable.\n";
        assert_eq!(map_failure(4, b"", err), GhError::SignedOut);
    }

    #[test]
    fn http_401_is_signed_out() {
        let out = br#"{"message":"Bad credentials","documentation_url":"https://docs.github.com/rest","status":"401"}"#;
        assert_eq!(
            map_failure(1, out, b"gh: Bad credentials (HTTP 401)\n"),
            GhError::SignedOut
        );
    }

    #[test]
    fn a_401_in_githubs_data_is_not_signed_out() {
        let out =
            br#"{"message":"Validation Failed (HTTP 401)","errors":[{"resource":"PullRequest","code":"custom"}]}"#;
        let got = map_failure(1, out, b"gh: Validation Failed (HTTP 422)\n");
        assert_eq!(
            got,
            GhError::Refused {
                status: Some(422),
                message: "Validation Failed (HTTP 401)".into()
            }
        );
    }

    #[test]
    fn empty_stdout_and_a_failure_is_unreachable() {
        let err = b"Get \"https://api.github.com/repos/octo-org/widgets\": proxyconnect tcp: dial tcp 127.0.0.1:9: connectex: refused\n";
        let want = "Get \"https://api.github.com/repos/octo-org/widgets\": proxyconnect tcp: dial tcp 127.0.0.1:9: connectex: refused";
        assert_eq!(map_failure(1, b"", err), GhError::Unreachable(want.into()));
        // The last line of standard error is the reason.
        assert_eq!(
            map_failure(1, b"", b"first\nsecond\n\n"),
            GhError::Unreachable("second".into())
        );
    }

    #[test]
    fn graphql_not_found_is_not_found() {
        let out = br#"{"data":{"repository":null},"errors":[{"type":"NOT_FOUND","path":["repository"],"message":"Could not resolve to a Repository with the name 'octo-org/nope'."}]}"#;
        let err = b"gh: Could not resolve to a Repository with the name 'octo-org/nope'.\n";
        assert_eq!(map_failure(1, out, err), GhError::NotFound);
        let rest = br#"{"message":"Not Found","status":"404"}"#;
        assert_eq!(map_failure(1, rest, b"gh: Not Found (HTTP 404)\n"), GhError::NotFound);
    }

    #[test]
    fn graphql_errors_with_exit_0_are_refused() {
        let out =
            br#"{"data":null,"errors":[{"type":"FORBIDDEN","message":"Resource not accessible by integration"}]}"#;
        assert_eq!(
            map_failure(0, out, b""),
            GhError::Refused {
                status: None,
                message: "Resource not accessible by integration".into()
            }
        );
    }

    #[test]
    fn rate_limit_is_told_apart() {
        let rest = br#"{"message":"API rate limit exceeded for user ID 1.","documentation_url":"https://docs.github.com/rest"}"#;
        let err = b"gh: API rate limit exceeded for user ID 1. (HTTP 403)\n";
        assert_eq!(map_failure(1, rest, err), GhError::RateLimited { reset_ms: None });
        let graphql = br#"{"data":null,"errors":[{"type":"RATE_LIMITED","message":"API rate limit already exceeded for user ID 1."}]}"#;
        assert_eq!(
            map_failure(1, graphql, b"gh: API rate limit already exceeded for user ID 1.\n"),
            GhError::RateLimited { reset_ms: None }
        );
        // A 403 that is not about the rate limit is a refusal.
        let other = br#"{"message":"Resource not accessible by integration"}"#;
        assert_eq!(
            map_failure(1, other, b"gh: Resource not accessible by integration (HTTP 403)\n"),
            GhError::Refused {
                status: Some(403),
                message: "Resource not accessible by integration".into()
            }
        );
    }

    #[test]
    fn auth_hosts_skip_failed_accounts() {
        // S17 finding 8: no sign-in prints no hosts; a signed-in host holds a "success" entry.
        assert_eq!(hosts_from(br#"{"hosts":{}}"#), Some(vec![]));
        let out = br#"{"hosts":{
            "github.com":[{"state":"success","active":true,"host":"github.com","login":"octocat","gitProtocol":"https"}],
            "ghe.example.com":[{"state":"error","active":true,"host":"ghe.example.com","login":"octocat"}],
            "both.example.com":[{"state":"error","active":false,"host":"both.example.com"},{"state":"success","active":true,"host":"both.example.com"}]
        }}"#;
        assert_eq!(
            hosts_from(out),
            Some(vec!["both.example.com".to_string(), "github.com".to_string()])
        );
        assert_eq!(hosts_from(b"not json"), None);
        assert_eq!(hosts_from(br#"{"nothing":1}"#), None);
    }

    #[test]
    fn a_program_that_cannot_start_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let nope = dir.path().join(EXE);
        assert!(matches!(
            run_bounded_with(Command::new(&nope), Duration::from_secs(5), None, 1000),
            Bounded::NotStarted
        ));
        let home = tempfile::tempdir().unwrap();
        assert_eq!(
            api(&nope, home.path(), "github.com", &["user"], None),
            Err(GhError::Missing)
        );
        assert_eq!(auth_hosts(&nope, home.path()), Err(GhError::Missing));
    }

    /// A command that prints the file `path`: `cmd /c type` on Windows, `cat` elsewhere.
    fn print_file(path: &Path) -> Command {
        if cfg!(windows) {
            let mut cmd = Command::new("cmd.exe");
            cmd.arg("/c").arg("type").arg(path);
            cmd
        } else {
            let mut cmd = Command::new("cat");
            cmd.arg(path);
            cmd
        }
    }

    #[test]
    fn an_answer_over_the_cap_is_too_long() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.txt");
        std::fs::write(&big, "x".repeat(10_000)).unwrap();
        let got = run_bounded_with(print_file(&big), Duration::from_secs(10), None, 1000);
        assert!(matches!(got, Bounded::TooMuch), "{got:?}");
        // Under the cap it is whole.
        let got = run_bounded_with(print_file(&big), Duration::from_secs(10), None, 20_000);
        let Bounded::Done(0, out, _) = got else {
            panic!("{got:?}")
        };
        assert_eq!(out.len(), 10_000);
    }

    #[test]
    fn bounded_writes_the_input_and_closes_it() {
        // `findstr ^` and `cat` copy their input until it ends, so they return only if the input was closed.
        let mut cmd = if cfg!(windows) {
            Command::new("findstr")
        } else {
            Command::new("cat")
        };
        if cfg!(windows) {
            cmd.arg("^");
        }
        let input = b"line one\nline two\n".to_vec();
        let got = run_bounded_with(cmd, Duration::from_secs(10), Some(input), 1000);
        let Bounded::Done(0, out, _) = got else {
            panic!("{got:?}")
        };
        let text = String::from_utf8_lossy(&out).into_owned();
        assert_eq!(text.lines().collect::<Vec<_>>(), ["line one", "line two"]);
    }
}
