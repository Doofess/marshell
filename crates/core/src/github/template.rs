//! The repository's pull request template, read from the user's own checkout for a new pull request's description.
//!
//! It is a file read from a folder a repository controls, and its text goes into a pull request's description, so
//! each step is checked: no link is followed below the checkout (a repository can hold a link to any file on the
//! disk), a folder is listed and a file read only after `links_refused` says the path leads nowhere refused, at
//! most `LIST_MAX` entries of a folder are read, only a regular file is opened, and a file over `TEMPLATE_MAX` is
//! not read at all.

use crate::server::files::links_refused;
use crate::server::ignores::read_capped;
use std::path::Path;

/// The largest template that is read; a bigger one gives none, not a cut one.
pub const TEMPLATE_MAX: u64 = 64 * 1024;
/// The most entries of a folder that are looked at.
const LIST_MAX: usize = 1_000;
/// The names a template has, without regard to ASCII case, in the order they are preferred in one folder.
const NAMES: [&str; 3] = [
    "pull_request_template",
    "pull_request_template.md",
    "pull_request_template.txt",
];

/// The checkout's pull request template: the first of `.github/`, the checkout's root and `docs/` that holds a file
/// named `pull_request_template`, `.md` or `.txt` (in that order of preference). None when there is none, when it is
/// a link or in a folder that is one, isn't a regular file, is over `TEMPLATE_MAX` or isn't UTF-8. A byte order mark
/// is dropped. Blocking.
pub fn pull_template(checkout: &Path) -> Option<String> {
    // The checkout's own folder is the person's choice, link or not; what the repository put in it is not.
    find(checkout, &|path| {
        links_refused(path) || (path != checkout && is_link(path))
    })
}

/// Whether `path` itself is a link (a symlink, or a junction on Windows), or can't be looked at.
fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).map_or(true, |meta| meta.file_type().is_symlink())
}

/// `pull_template` with the link check given, so a test can refuse a path it can't make a link to.
fn find(checkout: &Path, refused: &dyn Fn(&Path) -> bool) -> Option<String> {
    for folder in [checkout.join(".github"), checkout.to_path_buf(), checkout.join("docs")] {
        if refused(&folder) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        // Files and links, never a folder: the folder `PULL_REQUEST_TEMPLATE/` holds several templates and gives
        // none. What kind of file a link leads to is `read_capped`'s to say.
        let names: Vec<std::ffi::OsString> = entries
            .take(LIST_MAX)
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| !kind.is_dir()))
            .map(|entry| entry.file_name())
            .collect();
        for wanted in NAMES {
            let Some(name) = names
                .iter()
                .find(|name| name.to_str().is_some_and(|n| n.eq_ignore_ascii_case(wanted)))
            else {
                continue;
            };
            // The first file found is the template; one that can't be used is not replaced by a lesser one.
            let path = folder.join(name);
            if refused(&path) {
                return None;
            }
            let bytes = read_capped(&path, TEMPLATE_MAX)?;
            let text = String::from_utf8(bytes).ok()?;
            return Some(text.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(text));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn finds_the_template_in_dot_github_then_root_then_docs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        assert_eq!(pull_template(root), None);
        put(root, "docs/pull_request_template.md", "from docs");
        assert_eq!(pull_template(root).as_deref(), Some("from docs"));
        put(root, "pull_request_template.md", "from root");
        assert_eq!(pull_template(root).as_deref(), Some("from root"));
        put(root, ".github/pull_request_template.md", "from dot github");
        assert_eq!(pull_template(root).as_deref(), Some("from dot github"));
    }

    #[test]
    fn matches_any_case_and_txt() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        put(root, ".github/PULL_REQUEST_TEMPLATE.TXT", "upper txt");
        assert_eq!(pull_template(root).as_deref(), Some("upper txt"));
        // In one folder the bare name beats .md, and .md beats .txt, whatever order they are listed in.
        put(root, ".github/Pull_Request_Template.md", "md");
        assert_eq!(pull_template(root).as_deref(), Some("md"));
        put(root, ".github/pull_request_template", "bare");
        assert_eq!(pull_template(root).as_deref(), Some("bare"));
        // Another name is no template.
        let other = tempfile::tempdir().unwrap();
        put(other.path(), ".github/pull_request_template.rst", "x");
        put(other.path(), ".github/my_pull_request_template.md", "x");
        assert_eq!(pull_template(other.path()), None);
    }

    #[test]
    fn drops_a_byte_order_mark_and_keeps_the_text_as_it_is() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join(".github")).unwrap();
        let bytes = b"\xEF\xBB\xBF## Summary\r\n\r\n<!-- fill -->\n";
        std::fs::write(tmp.path().join(".github/pull_request_template.md"), bytes).unwrap();
        assert_eq!(
            pull_template(tmp.path()).as_deref(),
            Some("## Summary\r\n\r\n<!-- fill -->\n")
        );
    }

    #[test]
    fn a_file_that_is_not_utf8_or_over_64_kib_gives_none_and_so_does_a_template_folder() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join(".github")).unwrap();
        std::fs::write(tmp.path().join(".github/pull_request_template.md"), b"a\xFFb").unwrap();
        assert_eq!(pull_template(tmp.path()), None, "not UTF-8");

        // A bad file in `.github/` is not replaced by the root's.
        put(tmp.path(), "pull_request_template.md", "root");
        assert_eq!(pull_template(tmp.path()), None);

        let big = tempfile::tempdir().unwrap();
        put(
            big.path(),
            "pull_request_template.md",
            &"x".repeat(TEMPLATE_MAX as usize),
        );
        assert_eq!(pull_template(big.path()).map(|t| t.len()), Some(TEMPLATE_MAX as usize));
        put(
            big.path(),
            "pull_request_template.md",
            &"x".repeat(TEMPLATE_MAX as usize + 1),
        );
        assert_eq!(
            pull_template(big.path()),
            None,
            "over the cap is none, not a cut template"
        );

        // Several templates in a folder: that location gives none, and the next is looked at.
        let many = tempfile::tempdir().unwrap();
        put(many.path(), ".github/PULL_REQUEST_TEMPLATE/a.md", "a");
        assert_eq!(pull_template(many.path()), None);
        put(many.path(), "pull_request_template.md", "root");
        assert_eq!(pull_template(many.path()).as_deref(), Some("root"));
    }

    #[test]
    fn refuses_a_linked_template_and_a_file_over_64_kib() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        put(root, ".github/pull_request_template.md", "text");
        put(root, "pull_request_template.md", "root");
        // The file's own path is refused: neither it nor the root's is read.
        let file = root.join(".github").join("pull_request_template.md");
        assert_eq!(find(root, &|p| p == file), None);
        // The over-cap file refuses too.
        put(
            root,
            ".github/pull_request_template.md",
            &"y".repeat(TEMPLATE_MAX as usize + 1),
        );
        assert_eq!(pull_template(root), None);
    }

    /// A repository can hold a link where the template or its folder should be. Its target is any file on the
    /// disk, and the text would go into a pull request's description, so a link is never followed.
    #[test]
    fn a_link_to_a_local_folder_or_file_is_never_followed() {
        let outside = tempfile::tempdir().unwrap();
        put(
            outside.path(),
            "secret/pull_request_template.md",
            "outside the checkout",
        );

        // The folder is a junction (no privilege needed on Windows) or a symlink.
        let tmp = tempfile::tempdir().unwrap();
        let link = tmp.path().join(".github");
        let target = outside.path().join("secret");
        #[cfg(windows)]
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .output()
            .is_ok_and(|o| o.status.success());
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&target, &link).is_ok();
        assert!(made, "the test couldn't make a link to a folder");
        assert_eq!(
            std::fs::read_to_string(link.join("pull_request_template.md")).unwrap(),
            "outside the checkout"
        );
        assert_eq!(pull_template(tmp.path()), None);
        put(tmp.path(), "docs/pull_request_template.md", "docs");
        assert_eq!(pull_template(tmp.path()).as_deref(), Some("docs"));

        // The file is a symlink, where the OS lets a test make one. It is not replaced by a lesser place's.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join(".github")).unwrap();
        let link = tmp.path().join(".github").join("pull_request_template.md");
        let target = outside.path().join("secret").join("pull_request_template.md");
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_file(&target, &link).is_ok();
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&target, &link).is_ok();
        if made {
            put(tmp.path(), "docs/pull_request_template.md", "docs");
            assert_eq!(pull_template(tmp.path()), None);
        }
    }

    #[test]
    fn a_template_folder_that_is_a_link_is_not_listed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        put(root, ".github/pull_request_template.md", "text");
        put(root, "docs/pull_request_template.md", "docs");
        let folder = root.join(".github");
        // `.github/` is refused: the next place is looked at.
        assert_eq!(find(root, &|p| p == folder).as_deref(), Some("docs"));
        assert_eq!(find(root, &|p| p.starts_with(root)), None);

        // A real link whose target is refused (a network path), where the OS lets a test make one.
        let linked = tempfile::tempdir().unwrap();
        let link = linked.path().join(".github");
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_dir(r"\\marshell-test-host\share\gh", &link).is_ok();
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink("//marshell-test-host/share/gh", &link).is_ok();
        if made {
            put(linked.path(), "docs/pull_request_template.md", "docs");
            assert_eq!(pull_template(linked.path()).as_deref(), Some("docs"));
        }
    }
}
