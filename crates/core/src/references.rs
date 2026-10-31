//! A project's references: aliases the person names in a prompt, each pointing to a file or folder inside the
//! project. They live in `<home>/projects/<id>/references.json`, where `<id>` is a hash of the project's root, so
//! nothing is written into the repo and no request names the file's location. `resolve` matches a prompt against
//! them in memory and keeps no prompt text.

use crate::config::hash::hash_bytes;
use crate::projects;
use crate::store::{self, Versioned};
use marshell_protocol::api::{Alias, AliasInfo, ProjectReferences, ReferenceNote, ReferencesUpdate};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const FILE_NAME: &str = "references.json";
/// The most aliases a project has, and the most a note names.
pub const MAX_ALIASES: usize = 100;
pub const MAX_NOTE_ALIASES: usize = 8;
/// The most characters in a note (about 300 tokens).
pub const MAX_NOTE_CHARS: usize = 1200;
pub const MAX_NAME_CHARS: usize = 40;

/// The file's content.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefDoc {
    /// The project's root as `project_id` saw it. A file whose root differs is not this project's.
    pub root: String,
    pub enabled: bool,
    pub aliases: Vec<Alias>,
}

impl Versioned for RefDoc {
    const VERSION: u32 = 1;
}

fn root_text(root: &Path) -> String {
    projects::canonical(root).to_string_lossy().into_owned()
}

/// The first 16 hex characters of the SHA-256 of the canonical root, lowercased on Windows.
pub fn project_id(root: &Path) -> String {
    let mut text = root_text(root);
    if cfg!(windows) {
        text = text.to_lowercase();
    }
    let mut id = hash_bytes(text.as_bytes());
    id.truncate(16);
    id
}

fn file_of(home: &Path, root: &Path) -> PathBuf {
    home.join("projects").join(project_id(root)).join(FILE_NAME)
}

/// The project's references. A missing file, or one kept for another root (a hash collision), is an empty,
/// disabled doc.
pub fn load(home: &Path, root: &Path) -> anyhow::Result<RefDoc> {
    let text = root_text(root);
    let found = store::read::<RefDoc>(&file_of(home, root))?.filter(|doc| projects::same_path(&doc.root, &text));
    Ok(found.unwrap_or_else(|| RefDoc {
        root: text,
        ..RefDoc::default()
    }))
}

/// Whether `path` stays inside the project by its text: relative, with no `..`, drive or share prefix.
fn is_relative_inside(path: &str) -> bool {
    let path = path.replace('\\', "/");
    let bytes = path.as_bytes();
    let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    !(path.starts_with('/') || drive || path.split('/').any(|part| part == ".."))
}

/// Replaces the project's toggle and aliases. Names and paths are trimmed and `\` in a path becomes `/`. The `Err`
/// is the sentence to show the person. A path that doesn't exist is allowed: it shows as broken.
pub fn save(home: &Path, update: &ReferencesUpdate) -> Result<(), String> {
    if update.aliases.len() > MAX_ALIASES {
        return Err(format!("A project can have at most {MAX_ALIASES} aliases."));
    }
    let mut aliases: Vec<Alias> = Vec::with_capacity(update.aliases.len());
    for given in &update.aliases {
        let name = given.name.trim();
        let path = given.path.trim();
        if name.is_empty() {
            return Err("An alias needs a name.".into());
        }
        if path.is_empty() {
            return Err("An alias needs a path.".into());
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(format!("A name can be at most {MAX_NAME_CHARS} characters."));
        }
        let lower = name.to_lowercase();
        if aliases.iter().any(|a| a.name.to_lowercase() == lower) {
            return Err(format!("Two aliases are named \"{name}\"."));
        }
        if !is_relative_inside(path) {
            return Err(format!("\"{path}\" isn't inside the project."));
        }
        aliases.push(Alias {
            name: name.to_string(),
            path: path.replace('\\', "/"),
        });
    }
    let root = Path::new(&update.root);
    let doc = RefDoc {
        root: root_text(root),
        enabled: update.enabled,
        aliases,
    };
    store::write(&file_of(home, root), &doc).map_err(|e| format!("The references can't be saved: {e}"))
}

/// The absolute path `path` means inside `checkout`, when it exists and stays inside (links followed).
fn locate(checkout: &Path, path: &str) -> Option<PathBuf> {
    if !is_relative_inside(path) {
        return None;
    }
    let joined = checkout.join(path.replace('/', std::path::MAIN_SEPARATOR_STR));
    let real = std::fs::canonicalize(joined).ok()?;
    let base = std::fs::canonicalize(checkout).ok()?;
    // `canonical` drops Windows' `\\?\` prefix from the path that is shown.
    real.starts_with(&base).then(|| projects::canonical(&real))
}

/// The project's references with each alias's path checked against the project.
pub fn info(home: &Path, root: &Path) -> anyhow::Result<ProjectReferences> {
    let doc = load(home, root)?;
    let aliases = doc
        .aliases
        .iter()
        .map(|a| AliasInfo {
            name: a.name.clone(),
            path: a.path.clone(),
            exists: locate(root, &a.path).is_some(),
        })
        .collect();
    Ok(ProjectReferences {
        root: doc.root,
        enabled: doc.enabled,
        aliases,
    })
}

/// Where `needle` first stands in `text` as a whole word (the characters just outside it are not alphanumeric) and
/// clear of the spans already taken.
fn find_word(text: &[char], needle: &[char], taken: &[(usize, usize)]) -> Option<usize> {
    if needle.is_empty() || needle.len() > text.len() {
        return None;
    }
    (0..=text.len() - needle.len()).find(|&at| {
        let end = at + needle.len();
        text[at..end] == *needle
            && (at == 0 || !text[at - 1].is_alphanumeric())
            && (end == text.len() || !text[end].is_alphanumeric())
            && taken.iter().all(|&(s, e)| end <= s || at >= e)
    })
}

/// The note for the aliases `prompt` names, with paths in `checkout`. None when references are off, nothing
/// matches, or no matching alias has a path that exists inside `checkout`. An alias is matched whole-word and
/// case-insensitively, the longest aliases first, so a shorter alias inside a longer match is not added. The note
/// names at most 8 aliases in prompt order, cut at a whole alias to fit 1,200 characters.
pub fn resolve(doc: &RefDoc, checkout: &Path, prompt: &str) -> Option<ReferenceNote> {
    if !doc.enabled || doc.aliases.is_empty() {
        return None;
    }
    let text: Vec<char> = prompt.to_lowercase().chars().collect();
    let mut by_length: Vec<(Vec<char>, &Alias)> = doc
        .aliases
        .iter()
        .map(|a| (a.name.trim().to_lowercase().chars().collect(), a))
        .collect();
    by_length.sort_by_key(|(chars, _)| std::cmp::Reverse(chars.len()));

    let mut taken: Vec<(usize, usize)> = Vec::new();
    let mut hits: Vec<(usize, &Alias)> = Vec::new();
    for (chars, alias) in &by_length {
        if let Some(at) = find_word(&text, chars, &taken) {
            taken.push((at, at + chars.len()));
            hits.push((at, alias));
        }
    }
    hits.sort_by_key(|&(at, _)| at);

    // A matched alias whose path is gone, or leaves the checkout, is dropped after it took its span.
    let found: Vec<(&Alias, PathBuf)> = hits
        .into_iter()
        .filter_map(|(_, alias)| locate(checkout, &alias.path).map(|path| (alias, path)))
        .take(MAX_NOTE_ALIASES)
        .collect();

    let parts: Vec<String> = found
        .iter()
        .map(|(alias, path)| format!("'{}' = {}", alias.name, path.display()))
        .collect();
    let mut count = parts.len();
    let build = |n: usize| format!("References: {}.", parts[..n].join("; "));
    while count > 0 && build(count).chars().count() > MAX_NOTE_CHARS {
        count -= 1;
    }
    if count == 0 {
        return None;
    }
    Some(ReferenceNote {
        names: found[..count].iter().map(|(alias, _)| alias.name.clone()).collect(),
        note: build(count),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::{Alias, ReferencesUpdate};
    use std::path::{Path, PathBuf};

    fn alias(name: &str, path: &str) -> Alias {
        Alias {
            name: name.into(),
            path: path.into(),
        }
    }

    fn doc(enabled: bool, aliases: &[(&str, &str)]) -> RefDoc {
        RefDoc {
            root: String::new(),
            enabled,
            aliases: aliases.iter().map(|(n, p)| alias(n, p)).collect(),
        }
    }

    /// A checkout with `src/LoginButton.tsx` and a few more files. Returns the folder and its canonical path.
    fn checkout() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = projects::canonical(dir.path());
        std::fs::create_dir_all(root.join("src")).unwrap();
        for file in [
            "LoginButton.tsx",
            "Login.tsx",
            "a.rs",
            "b.rs",
            "c.rs",
            "d.rs",
            "e.rs",
            "f.rs",
            "g.rs",
            "h.rs",
            "i.rs",
            "j.rs",
        ] {
            std::fs::write(root.join("src").join(file), "x").unwrap();
        }
        (dir, root)
    }

    fn display(root: &Path, rel: &str) -> String {
        projects::canonical(&root.join(rel)).to_string_lossy().into_owned()
    }

    fn update(root: &Path, enabled: bool, aliases: Vec<Alias>) -> ReferencesUpdate {
        ReferencesUpdate {
            root: root.to_string_lossy().into_owned(),
            enabled,
            aliases,
        }
    }

    #[test]
    fn matches_whole_words_in_any_case() {
        let (_dir, root) = checkout();
        let d = doc(true, &[("login button", "src/LoginButton.tsx")]);
        let got = resolve(&d, &root, "Fix the Login Button now").unwrap();
        assert_eq!(got.names, vec!["login button".to_string()]);
        assert_eq!(
            got.note,
            format!(
                "References: 'login button' = {}.",
                display(&root, "src/LoginButton.tsx")
            )
        );
        // Punctuation next to the alias is a word edge.
        assert!(resolve(&d, &root, "(login button), please").is_some());
        assert!(resolve(&d, &root, "login button").is_some());
    }

    #[test]
    fn ignores_part_of_a_word_and_plurals() {
        let (_dir, root) = checkout();
        let d = doc(true, &[("login button", "src/LoginButton.tsx")]);
        for prompt in [
            "loginbuttons",
            "relogin button",
            "login buttons",
            "xlogin button",
            "login buttonx",
        ] {
            assert_eq!(resolve(&d, &root, prompt), None, "{prompt}");
        }
    }

    #[test]
    fn a_shorter_alias_inside_a_longer_match_is_skipped() {
        let (_dir, root) = checkout();
        let d = doc(
            true,
            &[("login", "src/Login.tsx"), ("login button", "src/LoginButton.tsx")],
        );
        let got = resolve(&d, &root, "the login button").unwrap();
        assert_eq!(got.names, vec!["login button".to_string()]);
        // Used on its own, the shorter alias still matches.
        let got = resolve(&d, &root, "the login page and the login button").unwrap();
        assert_eq!(got.names, vec!["login".to_string(), "login button".to_string()]);
    }

    #[test]
    fn aliases_are_listed_in_prompt_order_and_capped() {
        let (_dir, root) = checkout();
        let names = ["ka", "kb", "kc", "kd", "ke", "kf", "kg", "kh", "ki", "kj"];
        let files = [
            "a.rs", "b.rs", "c.rs", "d.rs", "e.rs", "f.rs", "g.rs", "h.rs", "i.rs", "j.rs",
        ];
        let pairs: Vec<(&str, String)> = names.iter().zip(files).map(|(n, f)| (*n, format!("src/{f}"))).collect();
        let pairs: Vec<(&str, &str)> = pairs.iter().map(|(n, p)| (*n, p.as_str())).collect();
        let d = doc(true, &pairs);
        // The prompt names them in reverse order.
        let prompt = "kj ki kh kg kf ke kd kc kb ka";
        let got = resolve(&d, &root, prompt).unwrap();
        assert_eq!(got.names, ["kj", "ki", "kh", "kg", "kf", "ke", "kd", "kc"]);
        assert!(got.note.chars().count() <= 1200);
        assert!(got.note.starts_with("References: 'kj' = "));
        assert!(got.note.ends_with('.'));
    }

    #[test]
    fn the_note_is_cut_at_a_whole_alias() {
        let (_dir, root) = checkout();
        let long = "n".repeat(150);
        let mut pairs = Vec::new();
        for i in 0..8 {
            let rel = format!("src/{long}{i}.txt");
            std::fs::write(root.join(&rel), "x").unwrap();
            pairs.push((format!("w{i}"), rel));
        }
        let d = RefDoc {
            root: String::new(),
            enabled: true,
            aliases: pairs.iter().map(|(n, p)| alias(n, p)).collect(),
        };
        let got = resolve(&d, &root, "w0 w1 w2 w3 w4 w5 w6 w7").unwrap();
        assert!(got.note.chars().count() <= 1200, "{}", got.note.chars().count());
        assert!(!got.names.is_empty() && got.names.len() < 8, "{:?}", got.names);
        // Every kept alias is whole, and the first ones are kept.
        let expected: Vec<String> = got
            .names
            .iter()
            .map(|n| {
                let i: usize = n[1..].parse().unwrap();
                format!("'{n}' = {}", display(&root, &pairs[i].1))
            })
            .collect();
        assert_eq!(got.note, format!("References: {}.", expected.join("; ")));
        assert_eq!(got.names[0], "w0");
    }

    #[test]
    fn a_missing_or_escaping_path_is_never_sent() {
        let (dir, root) = checkout();
        let missing = doc(true, &[("gone", "src/gone.rs")]);
        assert_eq!(resolve(&missing, &root, "see gone"), None);
        let up = doc(true, &[("up", "../x")]);
        assert_eq!(resolve(&up, &root, "see up"), None);
        // An absolute path is refused even when it points inside the checkout.
        let inside = root.join("src").join("a.rs").to_string_lossy().into_owned();
        let abs = doc(true, &[("abs", &inside)]);
        assert_eq!(resolve(&abs, &root, "see abs"), None);
        // The good alias still goes when a broken one is named beside it.
        let mixed = doc(true, &[("gone", "src/gone.rs"), ("ok", "src/a.rs")]);
        let got = resolve(&mixed, &root, "gone and ok").unwrap();
        assert_eq!(got.names, vec!["ok".to_string()]);
        drop(dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_checkout_is_never_sent() {
        let (_dir, root) = checkout();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "x").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("src").join("out")).unwrap();
        let d = doc(true, &[("secret", "src/out/secret.txt")]);
        assert_eq!(resolve(&d, &root, "read secret"), None);
    }

    #[test]
    fn disabled_sends_nothing() {
        let (_dir, root) = checkout();
        let d = doc(false, &[("login button", "src/LoginButton.tsx")]);
        assert_eq!(resolve(&d, &root, "Fix the Login Button now"), None);
    }

    #[test]
    fn save_refuses_bad_updates() {
        let (_dir, root) = checkout();
        let home = tempfile::tempdir().unwrap();
        let try_save = |aliases: Vec<Alias>| save(home.path(), &update(&root, true, aliases));

        assert_eq!(
            try_save(vec![alias("  ", "src/a.rs")]),
            Err("An alias needs a name.".into())
        );
        assert_eq!(try_save(vec![alias("a", "  ")]), Err("An alias needs a path.".into()));
        let long = "x".repeat(41);
        assert_eq!(
            try_save(vec![alias(&long, "src/a.rs")]),
            Err("A name can be at most 40 characters.".into())
        );
        // Forty characters is fine, and characters are counted, not bytes.
        assert_eq!(try_save(vec![alias(&"é".repeat(40), "src/a.rs")]), Ok(()));
        assert_eq!(
            try_save(vec![alias("Login", "src/a.rs"), alias(" login ", "src/b.rs")]),
            Err("Two aliases are named \"login\".".into())
        );
        assert_eq!(
            try_save(vec![alias("up", "../x")]),
            Err("\"../x\" isn't inside the project.".into())
        );
        assert_eq!(
            try_save(vec![alias("up", "src/../../x")]),
            Err("\"src/../../x\" isn't inside the project.".into())
        );
        assert_eq!(
            try_save(vec![alias("abs", "/etc/passwd")]),
            Err("\"/etc/passwd\" isn't inside the project.".into())
        );
        assert_eq!(
            try_save(vec![alias("drive", "C:/Windows")]),
            Err("\"C:/Windows\" isn't inside the project.".into())
        );
        assert_eq!(
            try_save(vec![alias("unc", "\\\\server\\share")]),
            Err("\"\\\\server\\share\" isn't inside the project.".into())
        );
        // More than 100 is refused first, before the per-alias checks.
        let many: Vec<Alias> = (0..101).map(|_| alias("", "")).collect();
        assert_eq!(try_save(many), Err("A project can have at most 100 aliases.".into()));
        // Exactly 100 is fine.
        let hundred: Vec<Alias> = (0..100).map(|i| alias(&format!("n{i}"), "src/a.rs")).collect();
        assert_eq!(try_save(hundred), Ok(()));
    }

    #[test]
    fn save_trims_and_stores_forward_slashes() {
        let (_dir, root) = checkout();
        let home = tempfile::tempdir().unwrap();
        save(
            home.path(),
            &update(&root, true, vec![alias("  Login Button ", "  src\\LoginButton.tsx ")]),
        )
        .unwrap();
        let loaded = load(home.path(), &root).unwrap();
        assert_eq!(loaded.aliases, vec![alias("Login Button", "src/LoginButton.tsx")]);
    }

    #[test]
    fn save_then_load_round_trips() {
        let (_dir, root) = checkout();
        let home = tempfile::tempdir().unwrap();
        let empty = load(home.path(), &root).unwrap();
        assert!(!empty.enabled && empty.aliases.is_empty());

        let aliases = vec![alias("login button", "src/LoginButton.tsx"), alias("src", "src")];
        save(home.path(), &update(&root, true, aliases.clone())).unwrap();
        let loaded = load(home.path(), &root).unwrap();
        assert!(loaded.enabled);
        assert_eq!(loaded.aliases, aliases);
        assert_eq!(loaded.root, projects::canonical(&root).to_string_lossy());
    }

    #[test]
    fn info_marks_broken_paths() {
        let (_dir, root) = checkout();
        let home = tempfile::tempdir().unwrap();
        // A path that isn't there is allowed in a save.
        save(
            home.path(),
            &update(
                &root,
                true,
                vec![
                    alias("ok", "src/a.rs"),
                    alias("folder", "src"),
                    alias("gone", "src/gone.rs"),
                ],
            ),
        )
        .unwrap();
        let got = info(home.path(), &root).unwrap();
        assert!(got.enabled);
        let seen: Vec<(&str, bool)> = got.aliases.iter().map(|a| (a.name.as_str(), a.exists)).collect();
        assert_eq!(seen, [("ok", true), ("folder", true), ("gone", false)]);
        assert_eq!(got.root, projects::canonical(&root).to_string_lossy());
    }

    #[test]
    fn the_file_lives_under_the_root_hash() {
        let (_dir, root) = checkout();
        let home = tempfile::tempdir().unwrap();
        let id = project_id(&root);
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        save(home.path(), &update(&root, true, vec![alias("a", "src/a.rs")])).unwrap();
        assert!(home.path().join("projects").join(&id).join("references.json").is_file());
        #[cfg(windows)]
        {
            let shouted = PathBuf::from(root.to_string_lossy().to_uppercase());
            assert_eq!(project_id(&shouted), id);
        }
        let (_other_dir, other) = checkout();
        assert_ne!(project_id(&other), id);
    }

    #[test]
    fn a_doc_for_another_root_is_treated_as_missing() {
        let (_dir, root) = checkout();
        let home = tempfile::tempdir().unwrap();
        let path = home
            .path()
            .join("projects")
            .join(project_id(&root))
            .join("references.json");
        let stranger = RefDoc {
            root: "/somewhere/else".into(),
            enabled: true,
            aliases: vec![alias("a", "src/a.rs")],
        };
        crate::store::write(&path, &stranger).unwrap();
        let got = load(home.path(), &root).unwrap();
        assert!(!got.enabled && got.aliases.is_empty());
    }
}
