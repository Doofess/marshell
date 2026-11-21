//! A picture of a page, saved to the person's Downloads folder. The client sends a PNG, the page's host
//! and the time; the core makes the file name and writes only into Downloads, never over a file.

use crate::paths;
use axum::Json;
use axum::body::Bytes;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::ScreenshotSaved;
use serde::Deserialize;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;

/// The most a picture may weigh: the body limit of the route is one byte more, so that this check answers.
pub const SHOT_CAP: u64 = 64 * 1024 * 1024;

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
/// How much of the host goes into a name.
const HOST_CHARS: usize = 80;
/// `name.png`, then `name (2).png` to `name (99).png`.
const LAST_NUMBER: u32 = 99;

/// Windows opens these as devices whatever follows the first dot, so a host that starts with one is `page`.
fn is_device_name(part: &str) -> bool {
    let lower = part.to_ascii_lowercase();
    if matches!(lower.as_str(), "con" | "prn" | "aux" | "nul") {
        return true;
    }
    let numbered = |prefix: &str| {
        lower
            .strip_prefix(prefix)
            .is_some_and(|n| matches!(n.as_bytes(), [b'1'..=b'9']))
    };
    numbered("com") || numbered("lpt")
}

/// `YYYY-MM-DD HHMMSS`, ASCII digits only.
fn is_time(at: &str) -> bool {
    at.len() == 17
        && at.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b'-',
            10 => b == b' ',
            _ => b.is_ascii_digit(),
        })
}

/// "example.com 2026-11-21 134501.png" from a host and a time. None when `at` isn't `YYYY-MM-DD HHMMSS` in digits.
///
/// `host` is untrusted: every character outside `A-Z a-z 0-9 . -` becomes `-` (so no separator, drive colon,
/// control character or space gets through), a run of dots is one dot, leading and trailing `-` and `.` are
/// dropped, and at most 80 characters stay. What is left is `page` when it is empty or starts with a Windows
/// device name.
pub(crate) fn shot_name(host: &str, at: &str) -> Option<String> {
    if !is_time(at) {
        return None;
    }
    let mut part = String::new();
    for c in host.chars() {
        let c = if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            c
        } else {
            '-'
        };
        if c == '.' && part.ends_with('.') {
            continue;
        }
        part.push(c);
    }
    let edge = |c: char| c == '-' || c == '.';
    // Only ASCII is left, so cutting at a byte is cutting at a character.
    let mut part = part.trim_matches(edge);
    part = part.get(..HOST_CHARS).unwrap_or(part).trim_matches(edge);
    let first = part.split('.').next().unwrap_or("");
    if part.is_empty() || is_device_name(first) {
        part = "page";
    }
    Some(format!("{part} {at}.png"))
}

/// Writes `png` into `dir` as `name`, or as `stem (2).png` to `stem (99).png` when that name is taken. The file
/// is created new, so a file that is there is never opened for writing. Makes no folder. Returns the name used.
pub(crate) fn save_shot(dir: &Path, name: &str, png: &[u8]) -> io::Result<String> {
    let stem = name.strip_suffix(".png").unwrap_or(name);
    for n in 1..=LAST_NUMBER {
        let candidate = if n == 1 {
            name.to_string()
        } else {
            format!("{stem} ({n}).png")
        };
        let path = dir.join(&candidate);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(e) = file.write_all(png).and_then(|()| file.flush()) {
                    drop(file);
                    // The file is the one just made, and holds a part of the picture.
                    let _ = std::fs::remove_file(&path);
                    return Err(e);
                }
                return Ok(candidate);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "ninety-nine pictures have that name",
    ))
}

#[derive(Deserialize)]
pub struct ShotQuery {
    #[serde(default)]
    host: String,
    #[serde(default)]
    at: String,
}

/// POST /v1/screenshots?host=&at=: the body is a PNG. 400 for a time that can't be used in a name and for
/// anything else than a PNG, 413 over the cap, 500 when Downloads isn't there or can't be written.
pub async fn save(Query(q): Query<ShotQuery>, body: Bytes) -> Response {
    let Some(name) = shot_name(&q.host, &q.at) else {
        return (StatusCode::BAD_REQUEST, "That time can't be used in a file name.").into_response();
    };
    if body.len() as u64 > SHOT_CAP {
        return (StatusCode::PAYLOAD_TOO_LARGE, "That picture is over 64 MB.").into_response();
    }
    if !body.starts_with(&PNG_SIGNATURE) {
        return (StatusCode::BAD_REQUEST, "That isn't a PNG image.").into_response();
    }
    let Some(dir) = paths::downloads_dir() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "The Downloads folder wasn't found.").into_response();
    };
    let saved = tokio::task::spawn_blocking(move || {
        if !dir.is_dir() {
            return None;
        }
        Some(save_shot(&dir, &name, &body))
    })
    .await;
    match saved {
        Ok(None) => (StatusCode::INTERNAL_SERVER_ERROR, "The Downloads folder wasn't found.").into_response(),
        Ok(Some(Ok(name))) => Json(ScreenshotSaved { name }).into_response(),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "The picture couldn't be written to Downloads.",
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: &str = "2026-11-21 134501";
    const PNG: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];

    #[test]
    fn a_name_is_the_host_and_the_time() {
        assert_eq!(
            shot_name("example.com", AT).as_deref(),
            Some("example.com 2026-11-21 134501.png")
        );
        assert_eq!(
            shot_name("localhost:5173", AT).as_deref(),
            Some("localhost-5173 2026-11-21 134501.png")
        );
        assert_eq!(
            shot_name("[::1]:8080", AT).as_deref(),
            Some("1--8080 2026-11-21 134501.png")
        );
    }

    #[test]
    fn a_host_that_cannot_name_a_file_is_called_page() {
        // Nothing left, a path, and a Windows device name before the first dot ("con.example" is the device "con").
        for host in [
            "",
            "...",
            "..\\..\\x",
            "a/b",
            "con.example",
            "NUL",
            "com1.dev",
            "\u{202e}",
        ] {
            let name = shot_name(host, AT).unwrap();
            assert!(!name.contains(['/', '\\', ':']), "{name}");
            assert!(name.ends_with(" 2026-11-21 134501.png"), "{name}");
        }
        assert_eq!(shot_name("", AT).as_deref(), Some("page 2026-11-21 134501.png"));
        assert_eq!(
            shot_name("con.example", AT).as_deref(),
            Some("page 2026-11-21 134501.png")
        );
        assert_eq!(
            shot_name(&"a".repeat(300), AT).unwrap().len(),
            80 + " 2026-11-21 134501.png".len()
        );
    }

    #[test]
    fn every_reserved_device_name_is_called_page_in_any_case() {
        let mut names: Vec<String> = ["con", "prn", "aux", "nul"].iter().map(|s| s.to_string()).collect();
        for n in 1..=9 {
            names.push(format!("com{n}"));
            names.push(format!("lpt{n}"));
        }
        for n in names {
            for host in [n.clone(), n.to_uppercase(), format!("{n}.example.com"), format!("{n}.")] {
                assert_eq!(
                    shot_name(&host, AT).as_deref(),
                    Some("page 2026-11-21 134501.png"),
                    "{host}"
                );
            }
        }
        // Only the whole part before the first dot is a device name.
        assert_eq!(
            shot_name("console.dev", AT).as_deref(),
            Some("console.dev 2026-11-21 134501.png")
        );
        assert_eq!(
            shot_name("com10.dev", AT).as_deref(),
            Some("com10.dev 2026-11-21 134501.png")
        );
    }

    #[test]
    fn nothing_that_names_a_place_other_than_a_file_gets_through() {
        let hosts = [
            "../../etc/passwd",
            "..\\..\\Windows\\win.ini",
            "C:\\Windows\\x",
            "c:x",
            "\\\\server\\share",
            "a\0b",
            "a\r\nb",
            "a\u{1b}[31mb",
            "evil. ",
            "evil.   ",
            "evil ",
            " evil",
            "a b",
            "ünï.com",
            "nul:stream",
            "x\u{200b}y",
            "%2e%2e%2f",
        ];
        for host in hosts {
            let name = shot_name(host, AT).unwrap();
            let stem = name.strip_suffix(" 2026-11-21 134501.png").expect(&name);
            assert!(!stem.is_empty() && stem.chars().count() <= 80, "{name}");
            assert!(
                stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'),
                "{name}"
            );
            assert!(!stem.starts_with(['.', '-']) && !stem.ends_with(['.', '-']), "{name}");
            assert!(!name.contains(".."), "{name}");
            assert!(!name.ends_with(['.', ' ']), "{name}");
            assert!(!name.chars().any(char::is_control), "{name}");
        }
        assert_eq!(
            shot_name("../../etc/passwd", AT).as_deref(),
            Some("etc-passwd 2026-11-21 134501.png")
        );
    }

    #[test]
    fn a_dot_run_inside_a_host_cannot_make_a_parent_reference() {
        let name = shot_name("a..b", AT).unwrap();
        assert!(!name.contains(".."), "{name}");
    }

    #[test]
    fn a_time_that_is_not_digits_in_that_shape_makes_no_name() {
        for at in [
            "",
            "2026-11-21",
            "2026-11-21 13:45:01",
            "2026-11-21 134501 ",
            "..\\x",
            "２０２６-11-21 134501",
            "2026-11-21_134501",
            "2026-11-21 13450a",
            "2026-11-21 1345012",
        ] {
            assert_eq!(shot_name("example.com", at), None, "{at}");
        }
    }

    #[test]
    fn a_taken_name_gets_a_number_and_nothing_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(save_shot(dir.path(), "a 1.png", PNG).unwrap(), "a 1.png");
        assert_eq!(save_shot(dir.path(), "a 1.png", PNG).unwrap(), "a 1 (2).png");
        assert_eq!(save_shot(dir.path(), "a 1.png", PNG).unwrap(), "a 1 (3).png");
        assert_eq!(std::fs::read(dir.path().join("a 1.png")).unwrap(), PNG);
    }

    #[test]
    fn an_existing_file_keeps_its_bytes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b 1.png"), b"mine").unwrap();
        assert_eq!(save_shot(dir.path(), "b 1.png", PNG).unwrap(), "b 1 (2).png");
        assert_eq!(std::fs::read(dir.path().join("b 1.png")).unwrap(), b"mine");
    }

    #[test]
    fn the_numbers_stop_at_99() {
        let dir = tempfile::tempdir().unwrap();
        for _ in 0..99 {
            save_shot(dir.path(), "c 1.png", PNG).unwrap();
        }
        assert!(dir.path().join("c 1 (99).png").is_file());
        let err = save_shot(dir.path(), "c 1.png", PNG).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 99);
    }

    #[test]
    fn a_missing_folder_is_an_error_and_is_not_made() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("nope");
        assert!(save_shot(&gone, "a 1.png", PNG).is_err());
        assert!(!gone.exists());
    }
}
