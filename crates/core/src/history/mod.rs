//! The history of agent conversations: a compressed copy of each transcript, a sidecar describing it,
//! and an index over the sidecars. Transcripts are only ever read, never changed.

use crate::hooks::risk::is_remote_path;
use crate::store;
use marshell_protocol::history::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod archive;
pub mod cast;
pub mod export;
pub mod import;
pub mod index;
pub mod marks;
pub mod recorder;
pub mod recordings;
pub mod storage;

/// What Marshell knows about one agent conversation. Stored as the `<id>.meta.json` sidecar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMeta {
    pub agent: String,
    pub session_id: String,
    pub tab_id: String,
    pub cwd: String,
    pub transcript_path: String,
    pub started_ms: u64,
    pub updated_ms: u64,
    /// The session this tab had before this one (after a /clear), if any.
    pub previous_in_tab: Option<String>,
    /// The recordings (in `recordings/`) of the tabs this conversation ran in, oldest first.
    #[serde(default)]
    pub recordings: Vec<RecordingLink>,
    /// Where the conversation ran. Sidecars from before this field read as Marshell.
    #[serde(default)]
    pub client: Client,
    /// The preset the conversation's tab started with ("deepseek"), so Open in new tab starts it the same way. Only the
    /// name: never a key. Sidecars from before this field read as None, and the index does not keep it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// The WSL distro the conversation ran in. Its `transcript_path` and `cwd` are then Windows paths under
    /// `\\wsl.localhost\<distro>` (or a drive, for a folder under `/mnt`). Sidecars from before this field read as
    /// None, and the index does not keep it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
}

/// One recording a conversation ran in, and when.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingLink {
    /// The file name in `recordings/`.
    pub file: String,
    /// The first hook event of this conversation in that recording, unix ms.
    pub first_ms: u64,
    /// The last hook event of this conversation in that recording, unix ms.
    pub last_ms: u64,
}

impl store::Versioned for SessionMeta {
    const VERSION: u32 = 2;
}

/// A session id is used in file names and as a command-line argument, so it must be 1 to 64 letters, digits,
/// `_` or `-`, and start with a letter or digit (so it can't read as an option).
pub fn valid_session_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.starts_with(|c: char| c.is_ascii_alphanumeric())
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// True when `path` is somewhere a transcript may be read from: absolute and local, or, for a conversation in a WSL
/// `distro`, under that distro's own share (`\\wsl.localhost\<distro>` or `\\wsl$\<distro>`, with no `..`). Every
/// other UNC or device path is refused. Makes no filesystem call.
fn readable_place(path: &str, distro: Option<&str>) -> bool {
    distro.is_some_and(|d| crate::wsl::under_distro(d, Path::new(path)))
        || (!is_remote_path(path) && Path::new(path).is_absolute())
}

/// The last component of `path`, split on both separators (a UNC path is not one `Path` on every host).
fn last_component(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or_default()
}

/// True when the path is absolute, local (not UNC or a device path), and named exactly
/// `<session_id>.jsonl`. With a `distro`, a path under that distro's share counts as local. Makes no filesystem call.
pub fn transcript_allowed(path: &str, session_id: &str, distro: Option<&str>) -> bool {
    valid_session_id(session_id)
        && readable_place(path, distro)
        && last_component(path) == format!("{session_id}.jsonl")
}

/// Like `transcript_allowed` for a sidecar read back from the archive. A Codex conversation's source is a
/// rollout named `rollout-<time>-<id>.jsonl` (or `.jsonl.zst`), so only its place and kind are checked.
pub fn sidecar_allowed(agent: &str, path: &str, session_id: &str) -> bool {
    sidecar_allowed_in(agent, path, session_id, None)
}

/// `sidecar_allowed` for a conversation that ran in the WSL `distro`, if any: its share is a place the source may be.
pub fn sidecar_allowed_in(agent: &str, path: &str, session_id: &str, distro: Option<&str>) -> bool {
    if agent == "antigravity" {
        return antigravity_transcript_allowed(path, session_id, distro);
    }
    if agent != "codex" {
        return transcript_allowed(path, session_id, distro);
    }
    let name = last_component(path);
    valid_session_id(session_id)
        && readable_place(path, distro)
        && (name.ends_with(".jsonl") || name.ends_with(".jsonl.zst"))
}

/// An Antigravity conversation id: a UUID, 8-4-4-4-12 hex digits.
fn is_uuid(id: &str) -> bool {
    id.len() == 36
        && id.char_indices().all(|(at, c)| match at {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// True when `path` is absolute, local (or under the `distro`'s share), free of `..`, and ends in agy's transcript for this conversation:
/// `antigravity-cli/brain/<id>/.system_generated/logs/transcript_full.jsonl`, with `/` or `\` between the
/// parts. Makes no filesystem call.
fn antigravity_transcript_allowed(path: &str, session_id: &str, distro: Option<&str>) -> bool {
    if !is_uuid(session_id) || !readable_place(path, distro) {
        return false;
    }
    let parts: Vec<&str> = path.split(['/', '\\']).collect();
    parts.iter().all(|part| *part != "..")
        && parts.ends_with(&[
            "antigravity-cli",
            "brain",
            session_id,
            ".system_generated",
            "logs",
            "transcript_full.jsonl",
        ])
}

/// Reads the conversation in `text`, the decoded transcript of `agent`'s session. None for an agent whose
/// transcripts Marshell doesn't read.
pub fn parse_for(agent: &str, text: &str, session_id: &str) -> Option<marshell_protocol::history::Conversation> {
    match agent {
        "claude" => Some(crate::agents::claude::transcript::parse(text, session_id)),
        "codex" => Some(crate::agents::codex::rollout::parse(text, session_id)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "00000000-0000-4000-8000-000000000001";

    fn dir_prefix() -> &'static str {
        if cfg!(windows) {
            r"C:\Users\user\.claude\projects\p\"
        } else {
            "/home/u/.claude/projects/p/"
        }
    }

    #[test]
    fn rejects_unsafe_ids_and_paths() {
        assert!(valid_session_id(ID));
        for bad in ["", "../x", "a/b", "a b"] {
            assert!(!valid_session_id(bad), "{bad:?}");
        }
        assert!(!valid_session_id(&"a".repeat(65)));
        // A leading `-` or `_` could read as an option.
        for bad in ["-x", "--dangerously-skip-permissions", "_x"] {
            assert!(!valid_session_id(bad), "{bad:?}");
        }
        assert!(valid_session_id("a-_"));
        assert!(valid_session_id("0"));

        assert!(transcript_allowed(&format!("{}{ID}.jsonl", dir_prefix()), ID, None));
        assert!(!transcript_allowed(&format!("p/{ID}.jsonl"), ID, None));
        assert!(!transcript_allowed(&format!(r"\\host\share\{ID}.jsonl"), ID, None));
        assert!(!transcript_allowed(&format!(r"\\?\C:\x\{ID}.jsonl"), ID, None));
        assert!(!transcript_allowed(&format!(r"\\.\pipe\{ID}.jsonl"), ID, None));
        assert!(!transcript_allowed(&format!("{}other.jsonl", dir_prefix()), ID, None));
        assert!(!transcript_allowed(&format!("{}{ID}.json", dir_prefix()), ID, None));
    }

    #[test]
    fn a_wsl_transcript_under_the_tabs_distro_is_allowed() {
        let file = format!(r"\\wsl.localhost\Ubuntu\home\u\.claude\projects\p\{ID}.jsonl");
        assert!(transcript_allowed(&file, ID, Some("Ubuntu")));
        // The legacy host, the other separator and another case of the distro name.
        assert!(transcript_allowed(
            &file.replace("wsl.localhost", "wsl$"),
            ID,
            Some("Ubuntu")
        ));
        assert!(transcript_allowed(&file.replace('\\', "/"), ID, Some("Ubuntu")));
        assert!(transcript_allowed(&file, ID, Some("ubuntu")));
        // It is still Claude's file for this conversation, and not the share root.
        assert!(!transcript_allowed(&file.replace(ID, "other"), ID, Some("Ubuntu")));
        assert!(!transcript_allowed(
            &file.replace(".jsonl", ".json"),
            ID,
            Some("Ubuntu")
        ));
        assert!(!transcript_allowed(&format!(r"{file}\"), ID, Some("Ubuntu")));
        assert!(!transcript_allowed(r"\\wsl.localhost\Ubuntu", ID, Some("Ubuntu")));
        assert!(!transcript_allowed(&file, "../x", Some("Ubuntu")));
        // The other agents' sources too.
        let rollout = r"\\wsl.localhost\Ubuntu\home\u\.codex\sessions\2026\10\31\rollout-2026-10-31T10-00-00-abc.jsonl";
        assert!(sidecar_allowed_in("codex", rollout, ID, Some("Ubuntu")));
        let agy = agy_path('\\', ID).replacen("C:\\Users", r"\\wsl.localhost\Ubuntu\home", 1);
        assert!(sidecar_allowed_in("antigravity", &agy, ID, Some("Ubuntu")));
    }

    #[test]
    fn another_distros_and_network_unc_stay_refused() {
        let tail = format!(r"home\u\.claude\projects\p\{ID}.jsonl");
        let bad = [
            format!(r"\\wsl$\Debian\{tail}"),
            format!(r"\\wsl.localhost\Debian\{tail}"),
            format!(r"\\wsl.localhost\Ubuntu-24.04\{tail}"),
            format!(r"\\server\share\{tail}"),
            format!(r"\\server\Ubuntu\{tail}"),
            format!(r"\\?\UNC\wsl.localhost\Ubuntu\{tail}"),
            format!(r"\\?\C:\{tail}"),
            format!(r"\\.\pipe\{tail}"),
            format!(r"\\wsl.localhost\Ubuntu\..\x\{ID}.jsonl"),
            format!(r"\\wsl.localhost\Ubuntu\home\..\..\x\{ID}.jsonl"),
            format!(r"\\wsl.localhost\Ubuntu\home\..\{ID}.jsonl"),
            format!("//server/share/{tail}"),
        ];
        for path in &bad {
            assert!(!transcript_allowed(path, ID, Some("Ubuntu")), "{path}");
            assert!(!sidecar_allowed_in("claude", path, ID, Some("Ubuntu")), "{path}");
            // Nothing opens up for a conversation that did not run in WSL.
            assert!(!transcript_allowed(path, ID, None), "{path}");
        }
        // An agy or Codex source follows the same rule.
        let agy = agy_path('\\', ID).replacen("C:\\Users", r"\\wsl.localhost\Debian\home", 1);
        assert!(!sidecar_allowed_in("antigravity", &agy, ID, Some("Ubuntu")));
        let rollout = r"\\server\share\rollout-2026-10-31T10-00-00-abc.jsonl";
        assert!(!sidecar_allowed_in("codex", rollout, ID, Some("Ubuntu")));
        // A distro name that is not one path component opens nothing.
        for distro in ["", r"Ubuntu\x", "a/b"] {
            assert!(!transcript_allowed(
                &format!(r"\\wsl.localhost\Ubuntu\{tail}"),
                ID,
                Some(distro)
            ));
        }
    }

    fn agy_path(sep: char, id: &str) -> String {
        let parts = [
            "C:",
            "Users",
            "u",
            ".gemini",
            "antigravity-cli",
            "brain",
            id,
            ".system_generated",
            "logs",
            "transcript_full.jsonl",
        ];
        parts.join(&sep.to_string())
    }

    #[test]
    fn an_antigravity_path_under_brain_is_allowed_with_either_separator() {
        for sep in ['/', '\\'] {
            assert!(sidecar_allowed("antigravity", &agy_path(sep, ID), ID), "{sep}");
        }
    }

    #[test]
    fn an_antigravity_path_outside_brain_is_refused() {
        let good = agy_path('/', ID);
        let bad = [
            // Not the transcript file, or not in the logs folder.
            good.replace("transcript_full.jsonl", "transcript.jsonl"),
            good.replace("/logs/", "/other/"),
            good.replace(".system_generated", "system"),
            good.replace("antigravity-cli/brain", "antigravity-cli/other"),
            // The id in the path is another conversation's.
            agy_path('/', "00000000-0000-4000-8000-000000000002"),
            // Traversal, a remote path, a relative path, and a path with more in front of the tail.
            good.replace("/brain/", "/brain/../brain/"),
            format!("C:/x/..{}", &good[good.find("/antigravity-cli").unwrap()..]),
            format!("//host/share{}", &good[good.find("/antigravity-cli").unwrap()..]),
            format!(r"\\?\{}", good.replace('/', r"\")),
            good[good.find("antigravity-cli").unwrap()..].to_string(),
            format!("{good}/more"),
            String::new(),
        ];
        for path in &bad {
            assert!(!sidecar_allowed("antigravity", path, ID), "{path}");
        }
    }

    #[test]
    fn an_antigravity_id_that_is_not_a_uuid_is_refused() {
        for id in [
            "",
            "abc-1",
            "../x",
            "x; calc",
            "00000000-0000-4000-8000-00000000000",
            "00000000-0000-4000-8000-0000000000012",
            "0000000000004000800000000000000001",
            "0000000g-0000-4000-8000-000000000001",
            "-0000000-0000-4000-8000-000000000001",
        ] {
            assert!(!sidecar_allowed("antigravity", &agy_path('/', id), id), "{id:?}");
        }
        // The hex digits may be in either case.
        let upper = ID.replace('0', "A");
        assert!(sidecar_allowed("antigravity", &agy_path('/', &upper), &upper));
    }
}
