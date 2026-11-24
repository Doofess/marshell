//! GitHub Copilot CLI (`copilot`): its own hooks file and its status line, planned as config changes.

pub mod exe;
pub mod hooks;
pub mod kept;
pub mod notify;
pub mod status;
pub mod transcript;
pub mod writes;

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub use super::common::Gate;

/// `COPILOT_HOME` when it is set and not blank, otherwise `<user home>/.copilot`.
pub fn home(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> PathBuf {
    let value = env("COPILOT_HOME").unwrap_or_default();
    let value = value.trim();
    if value.is_empty() {
        user_home.join(".copilot")
    } else {
        PathBuf::from(value)
    }
}

/// The Copilot CLI versions whose hook events and status line are recorded in `tests/fixtures/copilot/`. One entry
/// per minor line: its oldest recorded patch.
pub const FIXTURE_VERSIONS: &[(u32, u32, u32)] = &[(1, 0, 106)];

pub fn version_gate(version: Option<&str>) -> Gate {
    super::common::version_gate("Copilot CLI", FIXTURE_VERSIONS, version)
}

/// The `x.y.z` after `GitHub Copilot CLI ` on the first line of `copilot --version`, without the full stop.
pub fn parse_version(stdout: &str) -> Option<String> {
    let rest = stdout.lines().next()?.trim().strip_prefix("GitHub Copilot CLI ")?;
    let version = rest.split_whitespace().next()?.trim_end_matches('.');
    let core = version.split_once('-').map_or(version, |(core, _)| core);
    let parts: Vec<&str> = core.split('.').collect();
    let numeric = |p: &&str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    (parts.len() == 3 && parts.iter().all(numeric)).then(|| version.to_string())
}

/// What a hook answers to add `note` for the model: `additionalContext` at the top level.
pub fn context_output(note: &str) -> Value {
    json!({ "additionalContext": note })
}

/// What makes Copilot skip its permission prompts in a launch with no mode flag, when it is set: the setting
/// `defaultPermissionMode` of `allow-all`, or a truthy `COPILOT_ALLOW_ALL`.
pub fn skips_all(settings_text: Option<&str>, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let allow_all = settings_text
        .and_then(|text| crate::config::jsonc::read_value(text).ok())
        .is_some_and(|settings| settings.get("defaultPermissionMode").and_then(Value::as_str) == Some("allow-all"));
    if allow_all {
        return Some("defaultPermissionMode".to_string());
    }
    let truthy = env("COPILOT_ALLOW_ALL").is_some_and(|v| {
        let v = v.trim();
        v == "1" || v.eq_ignore_ascii_case("true")
    });
    truthy.then(|| "COPILOT_ALLOW_ALL".to_string())
}

/// The account Copilot last signed in as, from `lastLoggedInUser` of its `config.json` (JSONC: it starts with comment
/// lines): "<login>" or "<login> on <host>". Only the login and the host are read, and only when they look like a
/// login and a host; anything else of the file is never returned. A host of `github.com` is left out.
pub fn last_account(config_text: &str) -> Option<String> {
    let config = crate::config::jsonc::read_value(config_text).ok()?;
    let user = config.get("lastLoggedInUser")?.as_object()?;
    let login = user.get("login")?.as_str()?;
    let ok = |s: &str, max: usize, extra: &[char]| {
        (1..=max).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric() || extra.contains(&c))
    };
    if !ok(login, 100, &['-', '_']) {
        return None;
    }
    let host = user.get("host").and_then(Value::as_str).and_then(|host| {
        let host = host.trim();
        let host = host
            .strip_prefix("https://")
            .or_else(|| host.strip_prefix("http://"))
            .unwrap_or(host);
        let host = host.strip_suffix('/').unwrap_or(host);
        (ok(host, 253, &['.', '-', ':']) && !host.eq_ignore_ascii_case("github.com")).then(|| host.to_string())
    });
    Some(match host {
        Some(host) => format!("{login} on {host}"),
        None => login.to_string(),
    })
}

/// The first of the variables Copilot reads a token from that is set and not blank. The value is never returned.
pub fn token_env(env: &dyn Fn(&str) -> Option<String>) -> Option<&'static str> {
    ["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"]
        .into_iter()
        .find(|name| env(name).is_some_and(|v| !v.trim().is_empty()))
}

/// A new session id for `--session-id`: a lowercase v4 UUID.
pub fn new_session_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the OS random number generator failed");
    // Version 4, variant 10.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// The body that links a tab to the Copilot session `body` names: `{hook_event_name, session_id, transcript_path, cwd}`,
/// the shape the history recorder reads. A hook body names the session `sessionId`, a status line `session_id`.
/// `transcript_path` is the body's `transcriptPath` when it has one, otherwise
/// `<copilot_home>/session-state/<id>/events.jsonl`; a status line's own `transcript_path` is the session's folder, not
/// the file, so it is never used. None without a valid id.
pub fn link_body(body: &Value, copilot_home: &Path) -> Option<Value> {
    linked(body, Some(copilot_home), None)
}

/// `link_body`, for a session the tab already has a record of: `known` is the transcript path that record holds.
/// The path is the body's own `transcriptPath`, else `known`, else what `link_body` (with a home) or
/// `link_body_in_distro` (without) would build.
pub fn link_body_known(body: &Value, copilot_home: Option<&Path>, known: Option<&str>) -> Option<Value> {
    linked(body, copilot_home, known)
}

/// `link_body`, where `copilot_home` is None when the transcript must come from the body itself: its `transcriptPath`,
/// the `known` path of the tab's own record, or the `events.jsonl` in the session folder a status line names.
fn linked(body: &Value, copilot_home: Option<&Path>, known: Option<&str>) -> Option<Value> {
    let text = |key: &str| body.get(key).and_then(Value::as_str);
    let id = session_id_of(body)?;
    // The id goes into a path and a command line.
    if !crate::history::valid_session_id(id) {
        return None;
    }
    let transcript = match (text("transcriptPath"), known, copilot_home) {
        (Some(path), _, _) => path.to_string(),
        (None, Some(path), _) => path.to_string(),
        (None, None, Some(home)) => home
            .join("session-state")
            .join(id)
            .join("events.jsonl")
            .to_string_lossy()
            .into_owned(),
        // A status line names the session's folder: the log is the file in it.
        (None, None, None) => {
            let folder = text("transcript_path")?.trim_end_matches('/');
            if folder.rsplit('/').next() != Some(id) {
                return None;
            }
            format!("{folder}/events.jsonl")
        }
    };
    Some(json!({
        // A status line has no event name; any name links the session.
        "hook_event_name": text("hook_event_name").unwrap_or("statusLine"),
        "session_id": id,
        "transcript_path": transcript,
        "cwd": body.get("cwd").cloned().unwrap_or(Value::Null),
    }))
}

/// The session a hook or status line body names: `sessionId` in a hook's, `session_id` in a status line's.
pub fn session_id_of(body: &Value) -> Option<&str> {
    let text = |key: &str| body.get(key).and_then(Value::as_str);
    text("sessionId").or_else(|| text("session_id"))
}

/// The hook is the end of a session (`sessionEnd`).
pub fn ends_session(body: &Value) -> bool {
    body.get("hook_event_name").and_then(Value::as_str) == Some("sessionEnd")
}

/// The hook is an end the person asked for (`sessionEnd` with reason `user_exit`): `/exit`, and `/clear` for the old
/// session, which goes on under a new id in the same process.
pub fn is_requested_end(body: &Value) -> bool {
    ends_session(body) && body.get("reason").and_then(Value::as_str) == Some("user_exit")
}

/// The hook is the start of a session (`sessionStart`), a resumed one included.
pub fn starts_session(body: &Value) -> bool {
    body.get("hook_event_name").and_then(Value::as_str) == Some("sessionStart")
}

/// `link_body` for a tab in a WSL distro: the body's own `transcriptPath` (a Linux path, which the recorder reads under the
/// distro's share), or the log in the session folder its status line names, and nothing built from the Windows Copilot
/// home. Only `agentStop` carries a `transcriptPath`, so the status line is what links a session before its first reply
/// ends. None when the body names neither.
pub fn link_body_in_distro(body: &Value) -> Option<Value> {
    linked(body, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "00000000-0000-4000-8000-000000000001";

    fn transcript(home: &Path) -> String {
        home.join("session-state")
            .join(ID)
            .join("events.jsonl")
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn new_session_ids_are_v4_uuids() {
        let shape = regex::Regex::new("^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$").unwrap();
        let (a, b) = (new_session_id(), new_session_id());
        assert!(shape.is_match(&a), "{a}");
        assert!(shape.is_match(&b), "{b}");
        assert_ne!(a, b);
    }

    #[test]
    fn link_body_derives_the_transcript_path() {
        let home = Path::new("C:/Users/u/.copilot");
        let body = json!({"sessionId": ID, "cwd": "C:\\proj", "hook_event_name": "sessionStart", "initialPrompt": "x"});
        let linked = link_body(&body, home).unwrap();
        assert_eq!(
            linked,
            json!({
                "hook_event_name": "sessionStart",
                "session_id": ID,
                "transcript_path": transcript(home),
                "cwd": "C:\\proj",
            })
        );
        // The hook's own transcript path wins when it has one.
        let mut with_path = body.clone();
        with_path["transcriptPath"] = json!("D:\\c\\session-state\\x\\events.jsonl");
        assert_eq!(
            link_body(&with_path, home).unwrap()["transcript_path"],
            "D:\\c\\session-state\\x\\events.jsonl"
        );
    }

    #[test]
    fn a_body_with_its_own_transcript_path_ignores_the_known_one() {
        let home = Path::new("C:/Users/u/.copilot");
        let body = json!({"sessionId": ID, "hook_event_name": "agentStop", "transcriptPath": r"D:\c\session-state\x\events.jsonl"});
        let known = Some(r"D:\other\session-state\x\events.jsonl");
        for home in [Some(home), None] {
            assert_eq!(
                link_body_known(&body, home, known).unwrap()["transcript_path"],
                r"D:\c\session-state\x\events.jsonl"
            );
        }
    }

    #[test]
    fn an_end_with_no_transcript_path_uses_the_known_one() {
        let home = Path::new("C:/Users/u/.copilot");
        let body = json!({"sessionId": ID, "cwd": "C:\\proj", "hook_event_name": "sessionEnd"});
        let known = format!(r"D:\other\session-state\{ID}\events.jsonl");
        let linked = link_body_known(&body, Some(home), Some(&known)).unwrap();
        assert_eq!(linked["transcript_path"], known.as_str());
        assert_ne!(linked["transcript_path"], transcript(home).as_str());
        assert_eq!(linked["hook_event_name"], "sessionEnd");
        assert_eq!(linked["session_id"], ID);
    }

    #[test]
    fn an_end_in_a_distro_with_no_transcript_path_uses_the_known_one() {
        let body = json!({"sessionId": ID, "cwd": "/home/u/p", "hook_event_name": "sessionEnd"});
        let known = format!(r"\\wsl.localhost\Ubuntu\tmp\c\session-state\{ID}\events.jsonl");
        assert_eq!(
            link_body_known(&body, None, Some(&known)).unwrap()["transcript_path"],
            known.as_str()
        );
        // Nothing known and no home: nothing is built, as `link_body_in_distro` answers.
        assert_eq!(link_body_known(&body, None, None), None);
        assert_eq!(link_body_in_distro(&body), None);
        // The id must still be a plain one.
        let bad = json!({"sessionId": "../x", "hook_event_name": "sessionEnd"});
        assert_eq!(link_body_known(&bad, None, Some(&known)), None);
    }

    #[test]
    fn a_first_event_with_nothing_known_builds_the_path_from_the_home() {
        let home = Path::new("C:/Users/u/.copilot");
        let body = json!({"sessionId": ID, "cwd": "C:\\proj", "hook_event_name": "sessionStart"});
        let linked = link_body_known(&body, Some(home), None).unwrap();
        assert_eq!(linked["transcript_path"], transcript(home).as_str());
        assert_eq!(linked, link_body(&body, home).unwrap());
    }

    #[test]
    fn a_hooks_session_id_and_event_come_from_its_own_keys() {
        let start = json!({"sessionId": ID, "hook_event_name": "sessionStart"});
        let end = json!({"sessionId": ID, "hook_event_name": "sessionEnd"});
        assert_eq!(session_id_of(&start), Some(ID));
        // A status line names it in snake case.
        assert_eq!(session_id_of(&json!({"session_id": ID})), Some(ID));
        assert_eq!(session_id_of(&json!({"cwd": "/x"})), None);
        assert_eq!(session_id_of(&json!({"sessionId": 7})), None);
        assert!(ends_session(&end) && !ends_session(&start));
        assert!(starts_session(&start) && !starts_session(&end));
        assert!(!ends_session(&json!({"sessionId": ID})));
        assert!(!ends_session(&json!({"hook_event_name": "SessionEnd"})));
    }

    #[test]
    fn a_distro_body_links_by_its_own_transcript_path_only() {
        let path = format!("/home/u/.copilot/session-state/{ID}/events.jsonl");
        let body = json!({"sessionId": ID, "cwd": "/home/u/p", "hook_event_name": "agentStop", "transcriptPath": path});
        assert_eq!(
            link_body_in_distro(&body).unwrap(),
            json!({"hook_event_name": "agentStop", "session_id": ID, "transcript_path": path, "cwd": "/home/u/p"})
        );
        // No transcript of its own: nothing is built from a Windows home.
        assert_eq!(
            link_body_in_distro(&json!({"sessionId": ID, "cwd": "/home/u/p", "hook_event_name": "sessionStart"})),
            None
        );
        // A status line names the session's folder, and the log is the file in it.
        for folder in [
            format!("/home/u/.copilot/session-state/{ID}"),
            format!("/home/u/.copilot/session-state/{ID}/"),
        ] {
            let line = json!({"session_id": ID, "cwd": "/home/u/p", "transcript_path": folder});
            assert_eq!(
                link_body_in_distro(&line).unwrap(),
                json!({"hook_event_name": "statusLine", "session_id": ID, "transcript_path": path, "cwd": "/home/u/p"})
            );
        }
        // A folder that is not this session's is not used.
        let other =
            json!({"session_id": ID, "cwd": "/home/u/p", "transcript_path": "/home/u/.copilot/session-state/other"});
        assert_eq!(link_body_in_distro(&other), None);
        // The id must be a plain one.
        assert_eq!(
            link_body_in_distro(&json!({"sessionId": "../x", "cwd": "/p", "transcriptPath": path})),
            None
        );
    }

    #[test]
    fn link_body_needs_a_session_id() {
        let home = Path::new("C:/h");
        for body in [
            json!({"cwd": "C:\\p", "hook_event_name": "sessionStart"}),
            json!({"sessionId": "../x", "cwd": "C:\\p", "hook_event_name": "sessionStart"}),
            json!({"sessionId": "", "cwd": "C:\\p", "hook_event_name": "sessionStart"}),
            json!({"sessionId": 7, "cwd": "C:\\p", "hook_event_name": "sessionStart"}),
            json!({"sessionId": "a b", "cwd": "C:\\p", "hook_event_name": "sessionStart"}),
        ] {
            assert_eq!(link_body(&body, home), None, "{body}");
        }
    }

    #[test]
    fn link_body_reads_a_status_lines_session_id() {
        let home = Path::new("C:/h");
        let line = json!({
            "session_id": ID,
            "cwd": "C:\\proj",
            "transcript_path": format!("C:\\h\\session-state\\{ID}"),
        });
        let linked = link_body(&line, home).unwrap();
        assert_eq!(linked["session_id"], ID);
        assert_eq!(linked["cwd"], "C:\\proj");
        assert_eq!(linked["transcript_path"], transcript(home));
        assert!(linked["hook_event_name"].as_str().is_some_and(|e| !e.is_empty()));
    }

    #[test]
    fn last_account_reads_login_and_host() {
        let text = |host: &str| format!(r#"{{"lastLoggedInUser":{{"host":"{host}","login":"octo"}}}}"#);
        assert_eq!(last_account(&text("https://github.com")).as_deref(), Some("octo"));
        assert_eq!(
            last_account(&text("https://ghe.example.com/")).as_deref(),
            Some("octo on ghe.example.com")
        );
        assert_eq!(
            last_account(r#"{"lastLoggedInUser":{"login":"octo"}}"#).as_deref(),
            Some("octo")
        );
    }

    #[test]
    fn last_account_reads_through_copilots_comment_header() {
        let text = "// User settings belong in settings.json.\n// This file is managed automatically.\n{\n  \"lastLoggedInUser\": {\"host\": \"https://github.com\", \"login\": \"octo\"}\n}\n";
        assert_eq!(last_account(text).as_deref(), Some("octo"));
    }

    #[test]
    fn last_account_is_none_for_other_shapes() {
        let long = format!(r#"{{"lastLoggedInUser":{{"login":"{}"}}}}"#, "a".repeat(500));
        for text in [
            r#"{"lastLoggedInUser":"octo"}"#,
            r#"{"lastLoggedInUser":null}"#,
            r#"{"lastLoggedInUser":{"host":"https://github.com"}}"#,
            long.as_str(),
            r#"{"lastLoggedInUser":{"login":"oc\u001bto"}}"#,
            "{not json",
            "",
        ] {
            assert_eq!(last_account(text), None, "{text:?}");
        }
    }

    #[test]
    fn nothing_else_of_the_file_is_returned() {
        let text = r#"{"copilotTokens":{"h:octo":"ghu_secret"},"authTokens":["x"],"lastLoggedInUser":{"host":"https://github.com","login":"octo","token":"ghu_other"}}"#;
        assert_eq!(last_account(text).as_deref(), Some("octo"));
    }

    #[test]
    fn token_env_names_the_first_set_variable_and_skips_blank_ones() {
        let with = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
        };
        assert_eq!(token_env(&|_| None), None);
        assert_eq!(token_env(&with(&[("GITHUB_TOKEN", "x")])), Some("GITHUB_TOKEN"));
        assert_eq!(
            token_env(&with(&[("GITHUB_TOKEN", "x"), ("GH_TOKEN", "y")])),
            Some("GH_TOKEN")
        );
        assert_eq!(
            token_env(&with(&[
                ("COPILOT_GITHUB_TOKEN", "  "),
                ("GH_TOKEN", ""),
                ("GITHUB_TOKEN", "z")
            ])),
            Some("GITHUB_TOKEN")
        );
        assert_eq!(
            token_env(&with(&[("COPILOT_GITHUB_TOKEN", "a"), ("GH_TOKEN", "y")])),
            Some("COPILOT_GITHUB_TOKEN")
        );
    }

    #[test]
    fn the_home_follows_copilot_home() {
        let user = Path::new("C:/Users/test user");
        assert_eq!(home(&|_| None, user), user.join(".copilot"));
        let env = |k: &str| (k == "COPILOT_HOME").then(|| " D:/c/home ".to_string());
        assert_eq!(home(&env, user), PathBuf::from("D:/c/home"));
    }

    #[test]
    fn a_blank_copilot_home_is_ignored() {
        let user = Path::new("C:/Users/test user");
        for value in ["", " \t "] {
            assert_eq!(home(&|_| Some(value.into()), user), user.join(".copilot"));
        }
    }

    #[test]
    fn copilot_1_0_106_is_known() {
        for v in ["1.0.106", "1.0.120"] {
            assert_eq!(version_gate(Some(v)), Gate::Known, "{v}");
        }
        assert_eq!(
            version_gate(None),
            Gate::Unknown("Copilot CLI's version could not be read".to_string())
        );
    }

    #[test]
    fn a_later_minor_is_not_recorded() {
        assert_eq!(
            version_gate(Some("1.1.0")),
            Gate::Unknown("Copilot CLI 1.1.0 is not one Marshell has recorded yet".to_string())
        );
    }

    #[test]
    fn an_earlier_patch_is_older() {
        assert_eq!(
            version_gate(Some("1.0.105")),
            Gate::Unknown("Copilot CLI 1.0.105 is older than the versions Marshell knows".to_string())
        );
    }

    #[test]
    fn parse_version_drops_the_full_stop() {
        let out = "GitHub Copilot CLI 1.0.106.\nRun 'copilot update' to check for updates.\n";
        assert_eq!(parse_version(out), Some("1.0.106".to_string()));
        assert_eq!(
            parse_version("GitHub Copilot CLI 1.0.107-0.\r\nmore"),
            Some("1.0.107-0".to_string())
        );
        for bad in [
            "",
            "1.0.106",
            "GitHub Copilot CLI .",
            "GitHub Copilot CLI 1.0.\n",
            "other\nGitHub Copilot CLI 1.0.106.",
        ] {
            assert_eq!(parse_version(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_context_output_is_top_level_additional_context() {
        assert_eq!(
            context_output("References: 'a' = /r/a."),
            json!({"additionalContext": "References: 'a' = /r/a."})
        );
    }

    #[test]
    fn skips_all_reads_the_setting_and_the_env() {
        let no_env = |_: &str| None;
        let env = |v: &'static str| move |k: &str| (k == "COPILOT_ALLOW_ALL").then(|| v.to_string());
        assert_eq!(
            skips_all(Some(r#"{"defaultPermissionMode": "allow-all"}"#), &no_env).as_deref(),
            Some("defaultPermissionMode")
        );
        // JSONC: a comment, and a BOM.
        assert_eq!(
            skips_all(
                Some("\u{feff}// mine\n{\n  \"defaultPermissionMode\": \"allow-all\" // yes\n}\n"),
                &no_env
            )
            .as_deref(),
            Some("defaultPermissionMode")
        );
        for text in [
            r#"{"defaultPermissionMode": "manual"}"#,
            r#"{"defaultPermissionMode": "assisted"}"#,
            r#"{"defaultPermissionMode": true}"#,
            r#"{"ui": {"defaultPermissionMode": "allow-all"}}"#,
            "{}",
            "",
            "  \n",
            "not json {",
        ] {
            assert_eq!(skips_all(Some(text), &no_env), None, "{text:?}");
        }
        assert_eq!(skips_all(None, &no_env), None);
        for v in ["true", "TRUE", "True", "1", " 1 "] {
            assert_eq!(skips_all(None, &env(v)).as_deref(), Some("COPILOT_ALLOW_ALL"), "{v}");
        }
        for v in ["", "0", "false", "yes", "2"] {
            assert_eq!(skips_all(None, &env(v)), None, "{v}");
        }
        // The setting is named first when both are there.
        assert_eq!(
            skips_all(Some(r#"{"defaultPermissionMode":"allow-all"}"#), &env("true")).as_deref(),
            Some("defaultPermissionMode")
        );
    }
}
