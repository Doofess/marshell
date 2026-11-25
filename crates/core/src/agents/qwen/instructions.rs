//! Which instruction files Qwen Code loads: its context files (`QWEN.md` and `AGENTS.md`, or what `context.fileName`
//! names) in `~/.qwen` and from the project root down to the project, its rules, folder trust, and its auto memory.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::extensions::{folder_trust_on, folder_trusted};
use crate::config::jsonc::read_value;
use crate::doctor::instructions::display as shown;
use crate::doctor::instructions::{
    CliLoad, ImportBase, ImportRules, ImportSyntax, LoadedFile, Place, Reader, Role, Text, When, expand,
    frontmatter_value, strip_html_comments,
};

const IMPORT_DEPTH: u32 = 5;
/// Qwen Code sends this many lines of each `MEMORY.md`.
const MEMORY_LINES: usize = 200;
const TOO_LARGE: &str = "over 4 MiB, size estimated from its bytes";
const UNTRUSTED: &str = "Qwen Code doesn't trust this folder";
const NOT_NAMED: &str = "context.fileName doesn't name it";

/// The folder name Qwen Code keeps a project's auto memory under: the project's own path, lowercased, with every
/// character that isn't an ASCII letter or digit as `-`. A git worktree has its own.
pub fn memory_dir_name(project: &Path) -> String {
    let text = project.to_string_lossy();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn json(cx: &mut Reader, file: &Path) -> Option<Value> {
    match cx.read(file) {
        Text::Text(text) => read_value(&text).ok(),
        _ => None,
    }
}

/// `context.fileName` of one settings file: a string or an array of strings.
fn file_names(settings: Option<&Value>) -> Option<Vec<String>> {
    let value = settings?.get("context")?.get("fileName")?;
    let names: Vec<String> = match value {
        Value::String(name) => vec![name.clone()],
        Value::Array(items) => items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        _ => return None,
    };
    // A name is a file's name in each folder Qwen Code looks in, so one that holds a folder is left out.
    let names: Vec<String> = names
        .into_iter()
        .filter(|n| !n.trim().is_empty() && !n.contains(['/', '\\']))
        .collect();
    (!names.is_empty()).then_some(names)
}

fn norm(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let text = text.trim_end_matches('/').to_string();
    if cfg!(windows) { text.to_ascii_lowercase() } else { text }
}

/// `path` is inside `base`, and isn't `base`.
fn strictly_under(path: &Path, base: &Path) -> bool {
    norm(path)
        .strip_prefix(&norm(base))
        .is_some_and(|rest| rest.starts_with('/'))
}

fn row(path: &Path, role: Role, when: When, chars: usize, notes: Vec<String>) -> LoadedFile {
    LoadedFile {
        path: path.to_path_buf(),
        role,
        when,
        chars,
        notes,
        depth: 0,
    }
}

struct Run {
    files: Vec<LoadedFile>,
    unreadable: Vec<PathBuf>,
}

impl Run {
    /// One context file or rule, with its imports. `blocked` is the reason a project file doesn't load.
    fn add(&mut self, cx: &mut Reader, path: &Path, role: Role, rules: &ImportRules, blocked: Option<&str>) {
        match cx.read(path) {
            Text::Missing => {}
            Text::Refused | Text::Unreadable => self.unreadable.push(path.to_path_buf()),
            Text::TooLarge { bytes } => {
                let when = blocked.map_or(When::Start, |r| When::NotLoaded(r.into()));
                let chars = if blocked.is_some() {
                    0
                } else {
                    usize::try_from(bytes).unwrap_or(usize::MAX)
                };
                let notes = if blocked.is_some() {
                    Vec::new()
                } else {
                    vec![TOO_LARGE.to_string()]
                };
                self.files.push(row(path, role, when, chars, notes));
            }
            Text::Text(text) => match blocked {
                Some(reason) => self
                    .files
                    .push(row(path, role, When::NotLoaded(reason.into()), 0, Vec::new())),
                None => {
                    // A rule with `paths` loads when a matching file is used; one without loads at start.
                    let when = if role == Role::Rule && frontmatter_value(&text, "paths").is_some() {
                        When::WhenUsed
                    } else {
                        When::Start
                    };
                    self.files.extend(expand(cx, path, &text, rules, role, when));
                }
            },
        }
    }

    /// An existing `AGENTS.md` that `context.fileName` leaves out.
    fn left_out(&mut self, cx: &mut Reader, dir: &Path) {
        let path = dir.join("AGENTS.md");
        if matches!(cx.read(&path), Text::Missing | Text::Refused | Text::Unreadable) {
            return;
        }
        self.files
            .push(row(&path, Role::File, When::NotLoaded(NOT_NAMED.into()), 0, Vec::new()));
    }

    /// An auto-memory index: its first `MEMORY_LINES` lines, when it has any text.
    fn memory(&mut self, cx: &mut Reader, path: &Path) {
        let notes = vec![format!("first {MEMORY_LINES} lines")];
        match cx.read(path) {
            Text::Text(text) if !text.trim().is_empty() => {
                let chars = text
                    .split_inclusive('\n')
                    .take(MEMORY_LINES)
                    .map(|line| line.chars().count())
                    .sum();
                self.files.push(row(path, Role::Memory, When::Start, chars, notes));
            }
            Text::TooLarge { bytes } => {
                let chars = usize::try_from(bytes).unwrap_or(usize::MAX);
                let notes = [notes, vec![TOO_LARGE.to_string()]].concat();
                self.files.push(row(path, Role::Memory, When::Start, chars, notes));
            }
            Text::Refused | Text::Unreadable => self.unreadable.push(path.to_path_buf()),
            _ => {}
        }
    }
}

pub fn load(place: &Place, cx: &mut Reader) -> CliLoad {
    let roots = place.roots;
    let mut out = CliLoad {
        agent: "qwen",
        distro: place.distro.map(str::to_string),
        project: place.project.map(Path::to_path_buf),
        home: roots.home.clone(),
        files: Vec::new(),
        reason: None,
        notes: vec!["Qwen Code documents no limit.".to_string()],
        cuts: Vec::new(),
    };
    let mut run = Run {
        files: Vec::new(),
        unreadable: Vec::new(),
    };

    // Settings: the user's, the project's when its folder is trusted, and the system file over both.
    let user = json(cx, &roots.qwen_dir.join("settings.json"));
    let trust_on = folder_trust_on(user.as_ref());
    let blocked = match place.project {
        Some(project) if trust_on => {
            let rules = json(cx, &roots.qwen_dir.join("trustedFolders.json"));
            (!folder_trusted(&rules, project)).then_some(UNTRUSTED)
        }
        _ => None,
    };
    out.reason = blocked.map(str::to_string);
    let own = match place.project {
        Some(project) if blocked.is_none() => json(cx, &project.join(".qwen").join("settings.json")),
        _ => None,
    };
    let system = place.qwen_system_settings.and_then(|file| json(cx, file));
    let names = [&user, &own, &system]
        .into_iter()
        .rev()
        .find_map(|s| file_names(s.as_ref()));
    let names_left_out = names
        .as_ref()
        .is_some_and(|n| !n.iter().any(|name| name.eq_ignore_ascii_case("AGENTS.md")));
    let names = names.unwrap_or_else(|| vec!["QWEN.md".to_string(), "AGENTS.md".to_string()]);

    let user_rules = ImportRules {
        syntax: ImportSyntax::At,
        depth: IMPORT_DEPTH,
        base: ImportBase::File,
        inside: None,
        outside_reason: "outside the project",
        home: None,
        linux_root: place
            .distro
            .and_then(|_| roots.home.ancestors().last().map(Path::to_path_buf)),
        clean: strip_html_comments,
    };

    // User level.
    for name in &names {
        run.add(cx, &roots.qwen_dir.join(name), Role::File, &user_rules, None);
    }
    if names_left_out {
        run.left_out(cx, &roots.qwen_dir);
    }
    for file in cx.list(&roots.qwen_dir.join("rules"), ".md", true) {
        run.add(cx, &file, Role::Rule, &user_rules, None);
    }

    if let Some(project) = place.project
        && norm(project) != norm(&roots.home)
    {
        let chain = place.ancestors();
        let git_root = place.marked_root(cx, &[".git"]);
        let start = match &git_root {
            Some(root) => chain.iter().position(|d| d == root).unwrap_or(0),
            None if strictly_under(project, &roots.home) => chain
                .iter()
                .position(|d| strictly_under(d, &roots.home))
                .unwrap_or(chain.len()),
            None => 0,
        };
        let walk = &chain[start.min(chain.len())..];
        let root = walk.first().map_or(project, PathBuf::as_path);
        let project_rules = ImportRules {
            inside: Some(root.to_path_buf()),
            ..user_rules.clone()
        };
        for dir in walk {
            for name in &names {
                run.add(cx, &dir.join(name), Role::File, &project_rules, blocked);
            }
            if names_left_out {
                run.left_out(cx, dir);
            }
        }
        if let Some(git) = &git_root {
            run.add(
                cx,
                &git.join(".qwen").join("QWEN.local.md"),
                Role::File,
                &project_rules,
                blocked,
            );
        }
        for file in cx.list(&project.join(".qwen").join("rules"), ".md", true) {
            run.add(cx, &file, Role::Rule, &project_rules, blocked);
        }
    }

    // Written by Qwen Code itself, and sent after the project's files.
    run.add(
        cx,
        &roots.qwen_dir.join("output-language.md"),
        Role::File,
        &user_rules,
        None,
    );
    run.memory(cx, &roots.qwen_dir.join("memories").join("MEMORY.md"));
    if let Some(project) = place.project {
        let index = roots
            .qwen_dir
            .join("projects")
            .join(memory_dir_name(project))
            .join("memory")
            .join("MEMORY.md");
        run.memory(cx, &index);
    }

    out.files = run.files;
    let mut seen: Vec<PathBuf> = Vec::new();
    for path in run.unreadable {
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
            let roots = ExtRoots::under(&tmp.path().join("home"));
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

        fn load(&self, project: Option<&Path>) -> CliLoad {
            let system = self.at("system/settings.json");
            let place = Place {
                roots: &self.roots,
                project,
                ceiling: Some(self.tmp.path()),
                claude_managed: None,
                qwen_system_settings: Some(&system),
                distro: None,
            };
            load(&place, &mut Reader::windows())
        }
    }

    fn row<'a>(load: &'a CliLoad, path: &Path) -> &'a LoadedFile {
        load.files
            .iter()
            .find(|f| f.path == path)
            .unwrap_or_else(|| panic!("no row for {}", path.display()))
    }

    fn paths(load: &CliLoad) -> Vec<&Path> {
        load.files.iter().map(|f| f.path.as_path()).collect()
    }

    fn not_loaded(reason: &str) -> When {
        When::NotLoaded(reason.into())
    }

    const TRUST_ON: &str = r#"{"security":{"folderTrust":{"enabled":true}}}"#;

    #[test]
    fn default_names_load_root_to_project() {
        let s = Setup::new();
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        let user = s.write("home/.qwen/QWEN.md", "user");
        let root_qwen = s.write("repo/QWEN.md", "root");
        let root_agents = s.write("repo/AGENTS.md", "agents");
        let pkg = s.write("repo/pkg/QWEN.md", "pkg");
        s.write("repo/pkg/CLAUDE.md", "not read");
        s.write("repo/pkg/deeper/QWEN.md", "below, never loaded");
        let got = s.load(Some(&s.at("repo/pkg")));
        assert_eq!(
            paths(&got),
            vec![
                user.as_path(),
                root_qwen.as_path(),
                root_agents.as_path(),
                pkg.as_path()
            ]
        );
        assert!(got.files.iter().all(|f| f.when == When::Start));
        assert_eq!(got.files.iter().map(|f| f.chars).collect::<Vec<_>>(), vec![4, 4, 6, 3]);
        assert_eq!(got.agent, "qwen");
        assert_eq!(got.reason, None);
        assert!(got.cuts.is_empty());
    }

    #[test]
    fn a_file_name_with_a_folder_in_it_is_left_out() {
        let s = Setup::new();
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        s.write(
            "repo/.qwen/settings.json",
            r#"{"context":{"fileName":["QWEN.md","../outside/notes.md","sub\\deep.md"]}}"#,
        );
        let qwen = s.write("repo/QWEN.md", "qwen");
        s.write("outside/notes.md", "not an instruction file");
        s.write("repo/sub/deep.md", "nor this");
        let got = s.load(Some(&s.at("repo")));
        assert_eq!(paths(&got), vec![qwen.as_path()]);
    }

    #[test]
    fn file_name_setting_replaces_the_list() {
        let s = Setup::new();
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        s.write("home/.qwen/settings.json", r#"{"context":{"fileName":["CLAUDE.md"]}}"#);
        let claude = s.write("repo/CLAUDE.md", "claude");
        s.write("repo/QWEN.md", "not named");
        let agents = s.write("repo/AGENTS.md", "agents");
        let got = s.load(Some(&s.at("repo")));
        assert_eq!(paths(&got), vec![claude.as_path(), agents.as_path()]);
        assert_eq!(row(&got, &claude).when, When::Start);
        assert_eq!(row(&got, &agents).when, not_loaded("context.fileName doesn't name it"));
        // A project's own value is followed (finding 11), and a string works as well as an array.
        s.write("home/.qwen/settings.json", "{}");
        s.write("repo/.qwen/settings.json", r#"{"context":{"fileName":"CLAUDE.md"}}"#);
        let got = s.load(Some(&s.at("repo")));
        assert_eq!(paths(&got), vec![claude.as_path(), agents.as_path()]);
        // When the setting names AGENTS.md as well, it loads.
        s.write(
            "repo/.qwen/settings.json",
            r#"{"context":{"fileName":["AGENTS.md","CLAUDE.md"]}}"#,
        );
        let got = s.load(Some(&s.at("repo")));
        assert!(got.files.iter().all(|f| f.when == When::Start) && got.files.len() == 2);
    }

    #[test]
    fn an_untrusted_folder_loads_no_project_file_and_still_its_user_file() {
        let s = Setup::new();
        s.write("home/.qwen/settings.json", TRUST_ON);
        let user = s.write("home/.qwen/QWEN.md", "user");
        let project = s.at("work/proj");
        let file = s.write("work/proj/QWEN.md", "project");
        let rule = s.write("work/proj/.qwen/rules/r.md", "rule");
        let key = project.display().to_string().replace('\\', "\\\\");
        s.write(
            "home/.qwen/trustedFolders.json",
            &format!(r#"{{"{key}":"DO_NOT_TRUST"}}"#),
        );
        let got = s.load(Some(&project));
        assert_eq!(row(&got, &user).when, When::Start);
        assert_eq!(row(&got, &file).when, not_loaded("Qwen Code doesn't trust this folder"));
        assert_eq!(row(&got, &file).chars, 0);
        assert_eq!(row(&got, &rule).when, not_loaded("Qwen Code doesn't trust this folder"));
        assert_eq!(got.reason.as_deref(), Some("Qwen Code doesn't trust this folder"));
        // TRUST_FOLDER loads it.
        s.write(
            "home/.qwen/trustedFolders.json",
            &format!(r#"{{"{key}":"TRUST_FOLDER"}}"#),
        );
        let got = s.load(Some(&project));
        assert_eq!(row(&got, &file).when, When::Start);
        assert_eq!(got.reason, None);
    }

    #[test]
    fn a_folder_with_no_rule_loads_no_project_file() {
        let s = Setup::new();
        s.write("home/.qwen/settings.json", TRUST_ON);
        let file = s.write("work/proj/QWEN.md", "project");
        let got = s.load(Some(&s.at("work/proj")));
        assert_eq!(row(&got, &file).when, not_loaded("Qwen Code doesn't trust this folder"));
        assert_eq!(got.reason.as_deref(), Some("Qwen Code doesn't trust this folder"));
        // With folder trust off, no rule is needed.
        s.write("home/.qwen/settings.json", "{}");
        assert_eq!(row(&s.load(Some(&s.at("work/proj"))), &file).when, When::Start);
    }

    #[test]
    fn local_file_needs_a_git_root() {
        let s = Setup::new();
        let local = s.write("repo/.qwen/QWEN.local.md", "mine");
        let plain = s.write("repo/QWEN.md", "plain");
        let got = s.load(Some(&s.at("repo")));
        assert_eq!(paths(&got), vec![plain.as_path()]);
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        let got = s.load(Some(&s.at("repo")));
        assert_eq!(row(&got, &local).when, When::Start);
        assert_eq!(row(&got, &local).chars, 4);
    }

    #[test]
    fn without_git_the_walk_starts_below_the_home() {
        let s = Setup::new();
        s.write("home/QWEN.md", "the home itself, never walked");
        let dev = s.write("home/dev/QWEN.md", "dev");
        let app = s.write("home/dev/app/QWEN.md", "app");
        let got = s.load(Some(&s.at("home/dev/app")));
        assert_eq!(paths(&got), vec![dev.as_path(), app.as_path()]);
        // Outside the home, the walk starts at the ceiling.
        let top = s.write("QWEN.md", "top");
        let mid = s.write("other/QWEN.md", "mid");
        let leaf = s.write("other/leaf/QWEN.md", "leaf");
        let got = s.load(Some(&s.at("other/leaf")));
        assert_eq!(paths(&got), vec![top.as_path(), mid.as_path(), leaf.as_path()]);
        // The home itself as the project: only the user level.
        let user = s.write("home/.qwen/QWEN.md", "user");
        let got = s.load(Some(&s.at("home")));
        assert_eq!(paths(&got), vec![user.as_path()]);
    }

    #[test]
    fn an_import_outside_the_root_is_refused() {
        let s = Setup::new();
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        s.write("outside.md", "outside the repository");
        let inside = s.write("repo/docs/in.md", "inside");
        let outside = s.at("outside.md");
        let root = s.write("repo/QWEN.md", "see @docs/in.md and @../outside.md and @gone.md\n");
        let got = s.load(Some(&s.at("repo")));
        assert_eq!(got.files.len(), 4);
        assert_eq!(got.files[0].path, root);
        assert_eq!(got.files[1].path, inside);
        assert_eq!(got.files[1].when, When::Start);
        assert_eq!(got.files[1].role, Role::Import { by: root.clone() });
        assert_eq!(got.files[2].path, outside);
        assert_eq!(got.files[2].when, not_loaded("outside the project"));
        assert_eq!(got.files[3].when, not_loaded("not found"));
        // The missing import's text stays in the file's own count.
        assert_eq!(
            got.files[0].chars,
            "see @docs/in.md and @../outside.md and @gone.md\n".chars().count()
        );
    }

    #[test]
    fn rules_strip_html_comments() {
        let s = Setup::new();
        let always = s.write("home/.qwen/rules/sub/a.md", "<!-- hidden -->visible");
        let scoped = s.write("home/.qwen/rules/b.md", "---\npaths:\n  - \"src/**\"\n---\nbody");
        let project_rule = s.write("work/.qwen/rules/c.md", "abc<!-- x -->");
        let got = s.load(Some(&s.at("work")));
        assert_eq!(row(&got, &always).when, When::Start);
        assert_eq!(row(&got, &always).chars, 7);
        assert_eq!(row(&got, &always).role, Role::Rule);
        assert_eq!(row(&got, &scoped).when, When::WhenUsed);
        assert_eq!(row(&got, &project_rule).when, When::Start);
        assert_eq!(row(&got, &project_rule).chars, 3);
    }

    #[test]
    fn the_no_limit_note_is_there() {
        let s = Setup::new();
        let got = s.load(None);
        assert_eq!(got.notes, vec!["Qwen Code documents no limit.".to_string()]);
    }

    #[test]
    fn the_output_language_file_loads_at_start() {
        let s = Setup::new();
        let file = s.write("home/.qwen/output-language.md", "Reply in French.");
        let got = s.load(None);
        assert_eq!(paths(&got), vec![file.as_path()]);
        assert_eq!(got.files[0].when, When::Start);
        assert_eq!(got.files[0].chars, 16);
    }

    #[test]
    fn memory_counts_both_indexes_up_to_200_lines() {
        let s = Setup::new();
        let project = s.at("work/proj");
        fs::create_dir_all(&project).unwrap();
        let lines = "line\n".repeat(250);
        let user = s.write("home/.qwen/memories/MEMORY.md", &lines);
        let own = s.write(
            &format!("home/.qwen/projects/{}/memory/MEMORY.md", memory_dir_name(&project)),
            &lines,
        );
        let got = s.load(Some(&project));
        for file in [&user, &own] {
            let r = row(&got, file);
            assert_eq!(r.role, Role::Memory);
            assert_eq!(r.when, When::Start);
            assert_eq!(r.chars, 1_000);
            assert_eq!(r.notes, vec!["first 200 lines".to_string()]);
        }
        // Without a project, only the user's index.
        let got = s.load(None);
        assert_eq!(paths(&got), vec![user.as_path()]);
    }

    #[test]
    fn memory_dir_name_lowercases_and_dashes() {
        assert_eq!(memory_dir_name(Path::new(r"C:\dev\Airport")), "c--dev-airport");
        assert_eq!(memory_dir_name(Path::new("/home/Ann/My App.2")), "-home-ann-my-app-2");
    }

    #[test]
    fn an_empty_memory_index_gives_no_row() {
        let s = Setup::new();
        s.write("home/.qwen/memories/MEMORY.md", "");
        s.write("home/.qwen/projects/whatever/memory/MEMORY.md", "  \n");
        let got = s.load(Some(&s.at("work")));
        assert!(got.files.is_empty());
    }
}
