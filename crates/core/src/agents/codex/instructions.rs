//! Which instruction files Codex loads: `AGENTS.override.md` or `AGENTS.md` in `CODEX_HOME`, then one file per folder
//! from the project root down to the project, all within the `project_doc_max_bytes` budget.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use toml_edit::{DocumentMut, Item};

use super::extensions::trust_key;
use crate::doctor::instructions::display as shown;
use crate::doctor::instructions::{CliLoad, Cut, LoadedFile, Place, Reader, Role, Text, When};

/// Codex's default `project_doc_max_bytes`.
const DEFAULT_BUDGET: u64 = 32_768;
const OVERRIDE: &str = "AGENTS.override.md";
const PLAIN: &str = "AGENTS.md";
const TOO_LARGE: &str = "over 4 MiB, size estimated from its bytes";
const UNTRUSTED: &str = "Codex doesn't trust this folder";
const ZERO_BUDGET: &str = "project_doc_max_bytes is 0";

struct Settings {
    budget: u64,
    fallbacks: Vec<String>,
    markers: Vec<String>,
}

impl Settings {
    fn from(doc: Option<&DocumentMut>) -> Settings {
        let strings = |key: &str| -> Option<Vec<String>> {
            let array = doc?.get(key)?.as_array()?;
            Some(array.iter().filter_map(|v| v.as_str()).map(str::to_string).collect())
        };
        let budget = doc
            .and_then(|d| d.get("project_doc_max_bytes"))
            .and_then(Item::as_integer)
            .and_then(|n| u64::try_from(n).ok())
            .unwrap_or(DEFAULT_BUDGET);
        let fallbacks = strings("project_doc_fallback_filenames")
            .unwrap_or_default()
            .into_iter()
            .filter(|n| !n.trim().is_empty() && !n.contains('/') && !n.contains('\\'))
            .collect();
        let markers = strings("project_root_markers").unwrap_or_else(|| vec![".git".to_string()]);
        Settings {
            budget,
            fallbacks,
            markers,
        }
    }
}

fn read_config(cx: &mut Reader, file: &Path) -> Option<DocumentMut> {
    match cx.read(file) {
        Text::Text(text) => text.parse().ok(),
        _ => None,
    }
}

/// Whether Codex's config says the folder is untrusted: only the entry that decides for the folder counts, which is the
/// folder's own `[projects.'<key>']`, else its git root's (the nearest folder at or above it that holds `.git`, which
/// the caller finds through the guarded reader). A folder with no entry loads its files.
fn untrusted(user: Option<&DocumentMut>, project: &Path, git_root: Option<&Path>) -> bool {
    let Some(table) = user.and_then(|d| d.get("projects")).and_then(Item::as_table_like) else {
        return false;
    };
    let mut wanted = vec![trust_key(&project.display().to_string())];
    if let Some(root) = git_root {
        wanted.push(trust_key(&root.display().to_string()));
    }
    let level = |want: &String| {
        table.iter().find(|(key, _)| trust_key(key) == *want).map(|(_, item)| {
            item.as_table_like()
                .and_then(|t| t.get("trust_level"))
                .and_then(Item::as_str)
                == Some("untrusted")
        })
    };
    wanted.iter().find_map(level).unwrap_or(false)
}

/// The file that wins in a folder.
struct Winner {
    path: PathBuf,
    bytes: u64,
    /// None: over 4 MiB, never read.
    text: Option<Arc<str>>,
}

struct Found {
    winner: Winner,
    /// Existing files the winner shadows.
    others: Vec<PathBuf>,
}

/// The first non-empty file of `names` in `dir`, and the files after it that exist. A file Marshell can't read goes
/// to `unreadable` and takes no part.
fn pick(cx: &mut Reader, dir: &Path, names: &[String], unreadable: &mut Vec<PathBuf>) -> Option<Found> {
    let mut winner: Option<Winner> = None;
    let mut others = Vec::new();
    for name in names {
        let path = dir.join(name);
        let (bytes, text) = match cx.read(&path) {
            Text::Missing => continue,
            Text::Refused | Text::Unreadable => {
                unreadable.push(path);
                continue;
            }
            Text::TooLarge { bytes } => (bytes, None),
            Text::Text(text) => {
                // The size comes from the text the reader holds, so a file is opened once per run.
                (text.len() as u64, Some(text))
            }
        };
        if winner.is_some() {
            others.push(path);
        } else if bytes > 0 {
            winner = Some(Winner { path, bytes, text });
        }
    }
    winner.map(|winner| Found { winner, others })
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn shadowed(found: &Found) -> impl Iterator<Item = LoadedFile> + '_ {
    let winner = name_of(&found.winner.path);
    found.others.iter().map(move |path| LoadedFile {
        path: path.clone(),
        role: Role::File,
        when: When::NotLoaded(format!("{winner} is here")),
        chars: 0,
        notes: Vec::new(),
        depth: 0,
        raw: None,
    })
}

fn row(path: &Path, when: When, chars: usize, notes: Vec<String>) -> LoadedFile {
    LoadedFile {
        path: path.to_path_buf(),
        role: Role::File,
        when,
        chars,
        notes,
        depth: 0,
        raw: None,
    }
}

/// A file counted whole: its characters, or its bytes when it is over 4 MiB.
fn whole(winner: &Winner, when: When) -> LoadedFile {
    match &winner.text {
        Some(text) => row(&winner.path, when, text.chars().count(), Vec::new()),
        None => row(
            &winner.path,
            when,
            usize::try_from(winner.bytes).unwrap_or(usize::MAX),
            vec![TOO_LARGE.to_string()],
        ),
    }
}

/// The characters in the first `n` bytes of a text. A character the cut would split isn't counted.
fn chars_in_first_bytes(text: &str, n: u64) -> usize {
    let n = usize::try_from(n).unwrap_or(usize::MAX);
    text.char_indices().take_while(|(at, c)| at + c.len_utf8() <= n).count()
}

enum Over {
    Cut(u64),
    Skipped,
}

pub fn load(place: &Place, cx: &mut Reader) -> CliLoad {
    let roots = place.roots;
    let doc = read_config(cx, &roots.codex_home.join("config.toml"));
    let settings = Settings::from(doc.as_ref());
    let mut out = CliLoad {
        agent: "codex",
        distro: place.distro.map(str::to_string),
        project: place.project.map(Path::to_path_buf),
        home: roots.home.clone(),
        files: Vec::new(),
        reason: None,
        notes: Vec::new(),
        cuts: Vec::new(),
        imports: None,
    };
    let mut unreadable: Vec<PathBuf> = Vec::new();
    let ignore_case = place.distro.is_none();

    let mut names = vec![OVERRIDE.to_string(), PLAIN.to_string()];
    for fallback in &settings.fallbacks {
        let same = |n: &String| {
            if ignore_case {
                n.eq_ignore_ascii_case(fallback)
            } else {
                n == fallback
            }
        };
        if !names.iter().any(same) {
            names.push(fallback.clone());
        }
    }

    // The user's file, outside the budget.
    let user_names = &names[..2];
    if let Some(found) = pick(cx, &roots.codex_home, user_names, &mut unreadable) {
        out.files.push(whole(&found.winner, When::Start));
        out.files.extend(shadowed(&found));
    }

    if place.project.is_some() {
        project_files(place, cx, &settings, &names, doc.as_ref(), &mut out, &mut unreadable);
    }

    let mut seen = Vec::new();
    for path in unreadable {
        if !seen.contains(&path) {
            out.notes.push(format!(
                "Marshell can't read {}.",
                shown(&path, place.project, &roots.home)
            ));
            seen.push(path);
        }
    }
    out
}

fn project_files(
    place: &Place,
    cx: &mut Reader,
    settings: &Settings,
    names: &[String],
    doc: Option<&DocumentMut>,
    out: &mut CliLoad,
    unreadable: &mut Vec<PathBuf>,
) {
    let Some(project) = place.project else {
        return;
    };
    let git_root = place.marked_root(cx, &[".git"]);
    let blocker = if untrusted(doc, project, git_root.as_deref()) {
        Some(UNTRUSTED)
    } else if settings.budget == 0 {
        Some(ZERO_BUDGET)
    } else {
        None
    };
    out.reason = blocker.map(str::to_string);

    let markers: Vec<&str> = settings.markers.iter().map(String::as_str).collect();
    let chain = place.ancestors();
    let root = if markers.is_empty() {
        None
    } else {
        place.marked_root(cx, &markers)
    };
    let start = root
        .and_then(|root| chain.iter().position(|d| *d == root))
        .unwrap_or_else(|| chain.len().saturating_sub(1));

    let mut left = settings.budget;
    let mut over: Vec<(PathBuf, Over)> = Vec::new();
    for dir in &chain[start..] {
        let Some(found) = pick(cx, dir, names, unreadable) else {
            continue;
        };
        let winner = &found.winner;
        if let Some(reason) = blocker {
            out.files
                .push(row(&winner.path, When::NotLoaded(reason.into()), 0, Vec::new()));
        } else if winner.bytes <= left {
            left -= winner.bytes;
            out.files.push(whole(winner, When::Start));
        } else if left > 0 {
            let kept = left;
            left = 0;
            let chars = winner
                .text
                .as_deref()
                .map_or(usize::try_from(kept).unwrap_or(usize::MAX), |text| {
                    chars_in_first_bytes(text, kept)
                });
            let mut notes = Vec::new();
            if winner.text.is_none() {
                notes.push(TOO_LARGE.to_string());
            }
            notes.push(format!("cut: Codex loads {kept} of {} bytes", winner.bytes));
            out.files.push(row(&winner.path, When::Start, chars, notes));
            over.push((winner.path.clone(), Over::Cut(winner.bytes - kept)));
        } else {
            let when = When::NotLoaded("past project_doc_max_bytes".into());
            out.files.push(row(&winner.path, when, 0, Vec::new()));
            over.push((winner.path.clone(), Over::Skipped));
        }
        out.files.extend(shadowed(&found));
    }

    if !over.is_empty() {
        let list: Vec<String> = over
            .iter()
            .map(|(path, what)| {
                let path = shown(path, Some(project), &place.roots.home);
                match what {
                    Over::Cut(lost) => format!("{path} ({lost} bytes cut)"),
                    Over::Skipped => format!("{path} (not loaded)"),
                }
            })
            .collect();
        let folder = project
            .file_name()
            .map_or_else(|| project.display().to_string(), |n| n.to_string_lossy().into_owned());
        out.cuts.push(Cut {
            id_part: "budget".into(),
            path: None,
            title: format!("Codex cuts its project instructions in {folder}"),
            detail: format!(
                "It loads {} bytes of project instructions, and these are over: {}.",
                settings.budget,
                list.join(", ")
            ),
            cannot_fix: format!(
                "Set project_doc_max_bytes in {} to raise the limit, or shorten the files.",
                shown(&place.roots.codex_home.join("config.toml"), None, &place.roots.home)
            ),
        });
    }

    // Nested files load when Codex works in their folder.
    let nested_names: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|n| n.to_ascii_lowercase().ends_with(".md"))
        .collect();
    let nested = cx.nested(project).clone();
    if nested.stopped {
        out.notes
            .push("Marshell stopped looking for files in subfolders after 10,000 entries.".to_string());
    }
    for path in nested.named(&nested_names, place.distro.is_none()) {
        if cx.out_of_time() {
            break;
        }
        let (chars, notes) = match cx.read(&path) {
            Text::Missing => continue,
            Text::Refused | Text::Unreadable => {
                unreadable.push(path);
                continue;
            }
            Text::TooLarge { bytes } => (
                usize::try_from(bytes).unwrap_or(usize::MAX),
                vec![TOO_LARGE.to_string()],
            ),
            Text::Text(text) => (text.chars().count(), Vec::new()),
        };
        out.files.push(match blocker {
            Some(reason) => row(&path, When::NotLoaded(reason.into()), 0, Vec::new()),
            None => row(&path, When::WhenUsed, chars, notes),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::ext::ExtRoots;
    use std::fs;

    struct Setup {
        tmp: tempfile::TempDir,
        roots: ExtRoots,
    }

    impl Setup {
        fn new() -> Setup {
            let tmp = tempfile::tempdir().unwrap();
            let home = tmp.path().join("home");
            fs::create_dir_all(home.join(".codex")).unwrap();
            let roots = ExtRoots::under(&home);
            Setup { tmp, roots }
        }

        fn at(&self, rel: &str) -> PathBuf {
            rel.split('/')
                .fold(self.tmp.path().to_path_buf(), |path, part| path.join(part))
        }

        fn write(&self, rel: &str, text: &str) -> PathBuf {
            let path = self.at(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, text).unwrap();
            path
        }

        fn config(&self, text: &str) {
            self.write("home/.codex/config.toml", text);
        }

        fn load(&self, project: Option<&Path>) -> CliLoad {
            let place = Place {
                roots: &self.roots,
                project,
                ceiling: Some(self.tmp.path()),
                claude_managed: None,
                qwen_system_settings: None,
                distro: None,
            };
            load(&place, &mut Reader::windows())
        }
    }

    fn rows(load: &CliLoad) -> Vec<(String, When, usize)> {
        load.files
            .iter()
            .map(|f| (p(&f.path), f.when.clone(), f.chars))
            .collect()
    }

    fn p(path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }

    fn not_loaded(reason: &str) -> When {
        When::NotLoaded(reason.into())
    }

    #[test]
    fn the_load_says_how_this_cli_writes_imports() {
        let s = Setup::new();
        assert_eq!(s.load(None).imports, None);
    }

    #[test]
    fn user_override_beats_agents_md() {
        let s = Setup::new();
        let over = s.write("home/.codex/AGENTS.override.md", "over");
        let plain = s.write("home/.codex/AGENTS.md", "plain text");
        let got = s.load(None);
        assert_eq!(
            rows(&got),
            vec![
                (p(&over), When::Start, 4),
                (p(&plain), not_loaded("AGENTS.override.md is here"), 0)
            ]
        );
        // An empty override gives way to AGENTS.md.
        s.write("home/.codex/AGENTS.override.md", "");
        let got = s.load(None);
        assert_eq!(rows(&got), vec![(p(&plain), When::Start, 10)]);
        assert_eq!(got.agent, "codex");
        assert_eq!(got.home, s.roots.home);
    }

    #[test]
    fn one_file_per_folder_root_to_project() {
        let s = Setup::new();
        fs::create_dir_all(s.at("work/.git")).unwrap();
        let root = s.write("work/AGENTS.md", "root");
        let over = s.write("work/pkg/AGENTS.override.md", "override");
        let plain = s.write("work/pkg/AGENTS.md", "plain");
        let project = s.at("work/pkg/app");
        fs::create_dir_all(&project).unwrap();
        let got = s.load(Some(&project));
        assert_eq!(
            rows(&got),
            vec![
                (p(&root), When::Start, 4),
                (p(&over), When::Start, 8),
                (p(&plain), not_loaded("AGENTS.override.md is here"), 0)
            ]
        );
        assert_eq!(got.reason, None);
        assert!(got.cuts.is_empty());
    }

    #[test]
    fn fallback_names_are_followed() {
        let s = Setup::new();
        s.config("project_doc_fallback_filenames = [\"TEAM.md\", \"NOTES\"]\n");
        fs::create_dir_all(s.at("work/.git")).unwrap();
        let team = s.write("work/TEAM.md", "team");
        let notes = s.write("work/NOTES", "notes!");
        // AGENTS.md wins over a fallback in the same folder.
        let sub_plain = s.write("work/sub/AGENTS.md", "plain");
        let sub_team = s.write("work/sub/TEAM.md", "team");
        let below = s.write("work/sub/deep/TEAM.md", "below");
        let below_notes = s.write("work/sub/deep/NOTES", "no md");
        let got = s.load(Some(&s.at("work/sub")));
        assert_eq!(
            rows(&got),
            vec![
                (p(&team), When::Start, 4),
                (p(&notes), not_loaded("TEAM.md is here"), 0),
                (p(&sub_plain), When::Start, 5),
                (p(&sub_team), not_loaded("AGENTS.md is here"), 0),
                (p(&below), When::WhenUsed, 5),
            ]
        );
        // A name without .md is found in the chain but not below it.
        assert!(!got.files.iter().any(|f| f.path == below_notes));
        let at_root = s.load(Some(&s.at("work")));
        assert!(at_root.files.iter().any(|f| f.path == team && f.when == When::Start));
    }

    #[test]
    fn a_fallback_name_without_md_loads_in_the_chain() {
        let s = Setup::new();
        s.config("project_doc_fallback_filenames = [\"NOTES\"]\n");
        let notes = s.write("work/NOTES", "notes!");
        let got = s.load(Some(&s.at("work")));
        assert_eq!(rows(&got), vec![(p(&notes), When::Start, 6)]);
    }

    #[test]
    fn a_fallback_name_ending_in_a_wide_character_is_read() {
        let s = Setup::new();
        s.config("project_doc_fallback_filenames = [\"notes😀\", \"é\"]\n");
        let notes = s.write("work/notes😀", "notes!");
        let got = s.load(Some(&s.at("work")));
        assert_eq!(rows(&got), vec![(p(&notes), When::Start, 6)]);
    }

    #[test]
    fn a_fallback_with_a_path_is_ignored() {
        let s = Setup::new();
        s.config("project_doc_fallback_filenames = [\"docs/X.md\", \"..\\\\Y.md\", \"\"]\n");
        s.write("work/docs/X.md", "x");
        s.write("Y.md", "y");
        let got = s.load(Some(&s.at("work")));
        assert!(got.files.is_empty());
    }

    #[test]
    fn an_untrusted_folder_loads_no_project_file_and_still_its_user_file() {
        let s = Setup::new();
        let user = s.write("home/.codex/AGENTS.md", "user");
        let project = s.at("work/proj");
        let file = s.write("work/proj/AGENTS.md", "project");
        s.config(&format!(
            "[projects.'{}']\ntrust_level = \"untrusted\"\n",
            project.display()
        ));
        let got = s.load(Some(&project));
        assert_eq!(
            rows(&got),
            vec![
                (p(&user), When::Start, 4),
                (p(&file), not_loaded("Codex doesn't trust this folder"), 0)
            ]
        );
        assert_eq!(got.reason.as_deref(), Some("Codex doesn't trust this folder"));
        // The git root's entry decides for a folder with none of its own.
        let s = Setup::new();
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        let file = s.write("repo/pkg/AGENTS.md", "project");
        s.config(&format!(
            "[projects.'{}']\ntrust_level = \"untrusted\"\n",
            s.at("repo").display()
        ));
        let got = s.load(Some(&s.at("repo/pkg")));
        assert_eq!(
            rows(&got),
            vec![(p(&file), not_loaded("Codex doesn't trust this folder"), 0)]
        );
    }

    #[test]
    fn a_folder_with_no_trust_entry_loads_its_files() {
        let s = Setup::new();
        let project = s.at("work/proj");
        let file = s.write("work/proj/AGENTS.md", "project");
        s.config("[projects.'C:/somewhere/else']\ntrust_level = \"untrusted\"\n");
        let got = s.load(Some(&project));
        assert_eq!(rows(&got), vec![(p(&file), When::Start, 7)]);
        assert_eq!(got.reason, None);
        s.config(&format!(
            "[projects.'{}']\ntrust_level = \"trusted\"\n",
            project.display()
        ));
        assert_eq!(rows(&s.load(Some(&project))), vec![(p(&file), When::Start, 7)]);
    }

    #[test]
    fn zero_budget_loads_none() {
        let s = Setup::new();
        s.config("project_doc_max_bytes = 0\n");
        let user = s.write("home/.codex/AGENTS.md", "user");
        let file = s.write("work/AGENTS.md", "project");
        let got = s.load(Some(&s.at("work")));
        assert_eq!(
            rows(&got),
            vec![
                (p(&user), When::Start, 4),
                (p(&file), not_loaded("project_doc_max_bytes is 0"), 0)
            ]
        );
        assert_eq!(got.reason.as_deref(), Some("project_doc_max_bytes is 0"));
        assert!(got.cuts.is_empty());
    }

    #[test]
    fn the_root_search_stops_at_the_ceiling() {
        let s = Setup::new();
        // A marker above the ceiling is never seen, so the project's own folder is the root.
        fs::create_dir_all(s.at("outer/.git")).unwrap();
        s.write("outer/AGENTS.md", "above");
        let ceiling = s.at("outer/mid");
        let inside = s.write("outer/mid/proj/AGENTS.md", "inside");
        let project = s.at("outer/mid/proj");
        let place = Place {
            roots: &s.roots,
            project: Some(&project),
            ceiling: Some(&ceiling),
            claude_managed: None,
            qwen_system_settings: None,
            distro: None,
        };
        let got = load(&place, &mut Reader::windows());
        assert_eq!(rows(&got), vec![(p(&inside), When::Start, 6)]);
    }

    #[test]
    fn a_codex_home_outside_the_home_folder_names_its_config_file_in_one_form() {
        let mut s = Setup::new();
        s.roots.codex_home = s.at("elsewhere/cx");
        fs::create_dir_all(&s.roots.codex_home).unwrap();
        fs::create_dir_all(s.at("work/.git")).unwrap();
        s.write("work/AGENTS.md", &"a".repeat(40_000));
        let got = s.load(Some(&s.at("work")));
        let config = s.roots.codex_home.join("config.toml");
        assert_eq!(
            got.cuts[0].cannot_fix,
            format!(
                "Set project_doc_max_bytes in {} to raise the limit, or shorten the files.",
                config.display()
            )
        );
    }

    #[test]
    fn the_crossing_file_is_cut_and_later_ones_skipped() {
        let s = Setup::new();
        fs::create_dir_all(s.at("work/.git")).unwrap();
        let root = s.write("work/AGENTS.md", &"a".repeat(30_000));
        let sub = s.write("work/sub/AGENTS.md", &"b".repeat(5_000));
        let got = s.load(Some(&s.at("work/sub")));
        assert_eq!(
            rows(&got),
            vec![(p(&root), When::Start, 30_000), (p(&sub), When::Start, 2_768)]
        );
        assert_eq!(
            got.files[1].notes,
            vec!["cut: Codex loads 2768 of 5000 bytes".to_string()]
        );
        assert_eq!(got.cuts.len(), 1);
        let cut = &got.cuts[0];
        assert_eq!(cut.id_part, "budget");
        assert_eq!(cut.path, None);
        assert_eq!(cut.title, "Codex cuts its project instructions in sub");
        assert_eq!(
            cut.detail,
            "It loads 32768 bytes of project instructions, and these are over: AGENTS.md (2232 bytes cut)."
        );
        assert_eq!(
            cut.cannot_fix,
            "Set project_doc_max_bytes in ~/.codex/config.toml to raise the limit, or shorten the files."
        );
        // A file after the crossing one isn't loaded.
        let leaf = s.write("work/sub/leaf/AGENTS.md", "later");
        let got = s.load(Some(&s.at("work/sub/leaf")));
        assert_eq!(got.files[2].path, leaf);
        assert_eq!(got.files[2].when, not_loaded("past project_doc_max_bytes"));
        assert_eq!(got.files[2].chars, 0);
        assert_eq!(
            got.cuts[0].detail,
            format!(
                "It loads 32768 bytes of project instructions, and these are over: {} (2232 bytes cut), AGENTS.md (not loaded).",
                s.at("work").join("sub").join("AGENTS.md").display()
            )
        );
    }

    #[test]
    fn a_configured_budget_is_the_limit() {
        let s = Setup::new();
        s.config("project_doc_max_bytes = 10\n");
        let file = s.write("work/AGENTS.md", "0123456789abcdef");
        let got = s.load(Some(&s.at("work")));
        assert_eq!(rows(&got), vec![(p(&file), When::Start, 10)]);
        assert!(
            got.cuts[0]
                .detail
                .starts_with("It loads 10 bytes of project instructions")
        );
    }

    #[test]
    fn no_marker_means_only_the_project_folder() {
        let s = Setup::new();
        s.write("work/AGENTS.md", "above");
        let sub = s.write("work/sub/AGENTS.md", "sub");
        let got = s.load(Some(&s.at("work/sub")));
        assert_eq!(rows(&got), vec![(p(&sub), When::Start, 3)]);
        // An empty marker list does the same, even with a .git above.
        fs::create_dir_all(s.at("work/.git")).unwrap();
        s.config("project_root_markers = []\n");
        let got = s.load(Some(&s.at("work/sub")));
        assert_eq!(rows(&got), vec![(p(&sub), When::Start, 3)]);
        // Other markers are followed.
        s.config("project_root_markers = [\".hg\"]\n");
        fs::create_dir_all(s.at("work/.hg")).unwrap();
        let got = s.load(Some(&s.at("work/sub")));
        assert_eq!(got.files.len(), 2);
    }

    #[test]
    fn nested_files_load_when_used() {
        let s = Setup::new();
        s.write("work/AGENTS.md", "root");
        let nested = s.write("work/pkg/AGENTS.md", "nested!");
        let over = s.write("work/pkg/deep/AGENTS.override.md", "o");
        s.write("work/pkg/README.md", "not an instruction file");
        let got = s.load(Some(&s.at("work")));
        assert_eq!(got.files.len(), 3);
        assert_eq!(got.files[1].path, nested);
        assert_eq!(got.files[1].when, When::WhenUsed);
        assert_eq!(got.files[1].chars, 7);
        assert_eq!(got.files[2].path, over);
        assert_eq!(got.files[2].role, Role::File);
        assert!(got.notes.is_empty());
    }

    #[test]
    fn a_file_over_four_mib_counts_by_its_bytes() {
        let s = Setup::new();
        let big = s.write("home/.codex/AGENTS.md", &"x".repeat(4 * 1024 * 1024 + 1));
        let got = s.load(None);
        assert_eq!(rows(&got), vec![(p(&big), When::Start, 4_194_305)]);
        assert_eq!(
            got.files[0].notes,
            vec!["over 4 MiB, size estimated from its bytes".to_string()]
        );
    }

    #[test]
    fn untrusted_reads_only_the_deciding_entry() {
        let doc = |text: &str| text.parse::<DocumentMut>().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("p");
        fs::create_dir_all(project.join(".git")).unwrap();
        let key = project.display();
        let root = Some(project.as_path());
        let no = doc(&format!("[projects.'{key}']\ntrust_level = \"untrusted\"\n"));
        let yes = doc(&format!("[projects.'{key}']\ntrust_level = \"trusted\"\n"));
        assert!(untrusted(Some(&no), &project, root));
        assert!(!untrusted(Some(&yes), &project, root));
        assert!(!untrusted(Some(&doc("")), &project, root));
        assert!(!untrusted(None, &project, root));
        // A subfolder with no entry of its own follows its git root's; its own entry decides before the root's.
        let sub = project.join("sub");
        assert!(untrusted(Some(&no), &sub, root));
        assert!(!untrusted(Some(&no), &sub, None));
        let both = doc(&format!(
            "[projects.'{key}']\ntrust_level = \"untrusted\"\n[projects.'{}']\ntrust_level = \"trusted\"\n",
            sub.display()
        ));
        assert!(!untrusted(Some(&both), &sub, root));
    }
}
