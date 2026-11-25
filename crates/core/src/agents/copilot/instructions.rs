//! Which instruction files Copilot CLI loads: `copilot-instructions.md` and `instructions/**` in `~/.copilot`, then
//! from the repository root down to the project `AGENTS.md`, `CLAUDE.md`, `.claude/CLAUDE.md` and `GEMINI.md`, and at the
//! root `.github/copilot-instructions.md`, `.github/instructions/**` and `.claude/rules/**`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::doctor::instructions::display as shown;
use crate::doctor::instructions::{
    CliLoad, ImportBase, ImportRules, ImportStyle, ImportSyntax, LoadedFile, Place, Reader, Role, Text, When, expand,
    frontmatter_value,
};

const IMPORT_DEPTH: u32 = 5;
const TOO_LARGE: &str = "over 4 MiB, size estimated from its bytes";

/// What decides whether a file loads at start.
#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Start,
    /// Loads when used with this frontmatter key (`applyTo`, `paths`), at start without it.
    Key(&'static str),
}

fn row(path: &Path, role: Role, when: When, chars: usize, notes: Vec<String>) -> LoadedFile {
    LoadedFile {
        path: path.to_path_buf(),
        role,
        when,
        chars,
        notes,
        depth: 0,
        raw: None,
    }
}

struct Run {
    project: Option<PathBuf>,
    home: PathBuf,
    files: Vec<LoadedFile>,
    unreadable: Vec<PathBuf>,
    /// The files loaded at start by name, with their text, so an identical copy isn't counted again.
    started: Vec<(Arc<str>, PathBuf)>,
}

impl Run {
    /// One file. `imports` is None for a kind of file where an `@` is plain text.
    fn add(&mut self, cx: &mut Reader, path: &Path, role: Role, scope: Scope, imports: Option<&ImportRules>) {
        let text = match cx.read(path) {
            Text::Missing => return,
            Text::Refused | Text::Unreadable => {
                self.unreadable.push(path.to_path_buf());
                return;
            }
            Text::TooLarge { bytes } => {
                let chars = usize::try_from(bytes).unwrap_or(usize::MAX);
                self.files
                    .push(row(path, role, When::Start, chars, vec![TOO_LARGE.to_string()]));
                return;
            }
            Text::Text(text) => text,
        };
        let when = match scope {
            Scope::Key(key) if frontmatter_value(&text, key).is_some() => When::WhenUsed,
            _ => When::Start,
        };
        if when == When::Start && !text.is_empty() {
            if let Some((_, first)) = self.started.iter().find(|(seen, _)| **seen == *text) {
                let reason = format!("same text as {}", shown(first, self.project.as_deref(), &self.home));
                self.files.push(row(path, role, When::NotLoaded(reason), 0, Vec::new()));
                return;
            }
            self.started.push((text.clone(), path.to_path_buf()));
        }
        match imports {
            Some(rules) => self.files.extend(expand(cx, path, &text, rules, role, when)),
            None => self.files.push(row(path, role, when, text.chars().count(), Vec::new())),
        }
    }

    fn rules_in(&mut self, cx: &mut Reader, dir: &Path, suffix: &str, key: &'static str) {
        for file in cx.list(dir, suffix, true) {
            self.add(cx, &file, Role::Rule, Scope::Key(key), None);
        }
    }
}

pub fn load(place: &Place, cx: &mut Reader) -> CliLoad {
    let roots = place.roots;
    let mut out = CliLoad {
        agent: "copilot",
        distro: place.distro.map(str::to_string),
        project: place.project.map(Path::to_path_buf),
        home: roots.home.clone(),
        files: Vec::new(),
        reason: None,
        notes: vec!["Copilot CLI documents no limit; none was seen up to 256 KB.".to_string()],
        cuts: Vec::new(),
        imports: Some(ImportStyle {
            syntax: ImportSyntax::At,
            in_comments: true,
        }),
    };
    let mut run = Run {
        project: place.project.map(Path::to_path_buf),
        home: roots.home.clone(),
        files: Vec::new(),
        unreadable: Vec::new(),
        started: Vec::new(),
    };
    // A `~/` or absolute import isn't loaded; a relative one must stay inside the repository.
    let rules = |inside: &Path| ImportRules {
        syntax: ImportSyntax::At,
        depth: IMPORT_DEPTH,
        base: ImportBase::File,
        inside: Some(inside.to_path_buf()),
        outside_reason: "outside the repository",
        home: None,
        linux_root: place
            .distro
            .and_then(|_| roots.home.ancestors().last().map(Path::to_path_buf)),
        clean: |text| text.to_string(),
    };

    // User level.
    let user_rules = rules(&roots.copilot_dir);
    run.add(
        cx,
        &roots.copilot_dir.join("copilot-instructions.md"),
        Role::File,
        Scope::Start,
        Some(&user_rules),
    );
    run.rules_in(
        cx,
        &roots.copilot_dir.join("instructions"),
        ".instructions.md",
        "applyTo",
    );

    if let Some(project) = place.project {
        let chain = place.ancestors();
        let root = place.marked_root(cx, &[".git"]);
        let start = root
            .and_then(|root| chain.iter().position(|d| *d == root))
            .unwrap_or_else(|| chain.len().saturating_sub(1));
        let walk = &chain[start..];
        let Some(root) = walk.first() else {
            return finish(out, run, place);
        };
        let repo_rules = rules(root);
        for dir in walk {
            run.add(cx, &dir.join("AGENTS.md"), Role::File, Scope::Start, Some(&repo_rules));
            run.add(cx, &dir.join("CLAUDE.md"), Role::File, Scope::Start, Some(&repo_rules));
            run.add(
                cx,
                &dir.join(".claude").join("CLAUDE.md"),
                Role::File,
                Scope::Start,
                Some(&repo_rules),
            );
            run.add(cx, &dir.join("GEMINI.md"), Role::File, Scope::Start, None);
            if dir == root {
                let github = dir.join(".github");
                run.add(
                    cx,
                    &github.join("copilot-instructions.md"),
                    Role::File,
                    Scope::Start,
                    Some(&repo_rules),
                );
                run.rules_in(cx, &github.join("instructions"), ".instructions.md", "applyTo");
                run.rules_in(cx, &dir.join(".claude").join("rules"), ".md", "paths");
            }
        }

        // Files below the project load once the agent works in their folder.
        let nested = cx.nested(project).clone();
        if nested.stopped {
            out.notes
                .push("Marshell stopped looking for files in subfolders after 10,000 entries.".to_string());
        }
        for path in nested.named(&["AGENTS.md", "CLAUDE.md"], place.distro.is_none()) {
            if cx.out_of_time() {
                break;
            }
            match cx.read(&path) {
                Text::Missing => {}
                Text::Refused | Text::Unreadable => run.unreadable.push(path),
                Text::TooLarge { bytes } => {
                    let chars = usize::try_from(bytes).unwrap_or(usize::MAX);
                    run.files.push(row(
                        &path,
                        Role::File,
                        When::WhenUsed,
                        chars,
                        vec![TOO_LARGE.to_string()],
                    ));
                }
                Text::Text(text) => {
                    run.files
                        .push(row(&path, Role::File, When::WhenUsed, text.chars().count(), Vec::new()))
                }
            }
        }
    }
    finish(out, run, place)
}

fn finish(mut out: CliLoad, run: Run, place: &Place) -> CliLoad {
    out.files = run.files;
    let mut seen: Vec<PathBuf> = Vec::new();
    for path in run.unreadable {
        if !seen.contains(&path) {
            out.notes.push(format!(
                "Marshell can't read {}.",
                shown(&path, place.project, &place.roots.home)
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

    #[test]
    fn the_load_says_how_this_cli_writes_imports() {
        let s = Setup::new();
        let got = s.load(None);
        assert_eq!(
            got.imports,
            Some(ImportStyle {
                syntax: ImportSyntax::At,
                in_comments: true
            })
        );
    }

    #[test]
    fn repo_files_load_from_the_git_root_to_the_project() {
        let s = Setup::new();
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        let agents = s.write("repo/AGENTS.md", "agents");
        let claude = s.write("repo/CLAUDE.md", "claude");
        let dot_claude = s.write("repo/.claude/CLAUDE.md", "dot claude");
        let gemini = s.write("repo/GEMINI.md", "gemini");
        let github = s.write("repo/.github/copilot-instructions.md", "github");
        let pkg = s.write("repo/pkg/AGENTS.md", "pkg agents");
        s.write("repo/QWEN.md", "not read");
        s.write("repo/pkg/.github/copilot-instructions.md", "only at the root");
        s.write("above.md", "x");
        let got = s.load(Some(&s.at("repo/pkg")));
        assert_eq!(
            paths(&got),
            vec![
                agents.as_path(),
                claude.as_path(),
                dot_claude.as_path(),
                gemini.as_path(),
                github.as_path(),
                pkg.as_path()
            ]
        );
        assert!(got.files.iter().all(|f| f.when == When::Start));
        assert_eq!(got.agent, "copilot");
        assert_eq!(got.reason, None);
        assert!(got.cuts.is_empty());
        // Without a .git, the project itself is the root.
        let s = Setup::new();
        s.write("work/AGENTS.md", "above the project");
        let own = s.write("work/proj/AGENTS.md", "own");
        let got = s.load(Some(&s.at("work/proj")));
        assert_eq!(paths(&got), vec![own.as_path()]);
    }

    #[test]
    fn claude_rules_with_paths_load_when_used() {
        let s = Setup::new();
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        let scoped = s.write("repo/.claude/rules/sub/a.md", "---\npaths:\n  - \"src/**\"\n---\nbody");
        let plain = s.write("repo/.claude/rules/b.md", "plain rule");
        let scoped_instr = s.write(
            "repo/.github/instructions/c.instructions.md",
            "---\napplyTo: \"**/*.ts\"\n---\nts only",
        );
        let plain_instr = s.write("repo/.github/instructions/d.instructions.md", "always");
        s.write("repo/.github/instructions/readme.md", "not an instructions file");
        let got = s.load(Some(&s.at("repo")));
        assert_eq!(row(&got, &scoped).when, When::WhenUsed);
        assert_eq!(row(&got, &plain).when, When::Start);
        assert_eq!(row(&got, &plain).role, Role::Rule);
        assert_eq!(row(&got, &scoped_instr).when, When::WhenUsed);
        assert_eq!(row(&got, &plain_instr).when, When::Start);
        assert_eq!(got.files.len(), 4);
    }

    #[test]
    fn imports_only_in_three_file_kinds() {
        let s = Setup::new();
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        let imported = s.write("repo/docs/a.md", "imported");
        let text = "see @docs/a.md\n";
        let agents = s.write("repo/AGENTS.md", text);
        // Imports resolve from the importing file's folder.
        let copilot = s.write("repo/.github/copilot-instructions.md", "see @../docs/a.md\nx");
        let gemini = s.write("repo/GEMINI.md", &format!("{text}y"));
        let instr = s.write("repo/.github/instructions/z.instructions.md", &format!("{text}z"));
        let claude = s.write("repo/CLAUDE.md", &format!("{text}w"));
        let got = s.load(Some(&s.at("repo")));
        let imports: Vec<&LoadedFile> = got
            .files
            .iter()
            .filter(|f| matches!(&f.role, Role::Import { .. }))
            .collect();
        // AGENTS.md, CLAUDE.md and copilot-instructions.md each pull docs/a.md in; the other two kinds don't.
        let by: Vec<&Path> = imports
            .iter()
            .map(|f| match &f.role {
                Role::Import { by } => by.as_path(),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(by, vec![agents.as_path(), claude.as_path(), copilot.as_path()]);
        assert!(
            imports
                .iter()
                .all(|f| f.path == imported && f.chars == 8 && f.when == When::Start)
        );
        assert_eq!(row(&got, &gemini).chars, text.len() + 1);
        assert_eq!(row(&got, &instr).chars, text.len() + 1);
    }

    #[test]
    fn a_home_import_is_not_loaded() {
        let s = Setup::new();
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        let outside = s.write("home/secret.md", "secret");
        let elsewhere = s.write("elsewhere.md", "elsewhere");
        s.write(
            "repo/AGENTS.md",
            &format!(
                "@~/secret.md and @{}\n",
                elsewhere.display().to_string().replace('\\', "/")
            ),
        );
        let got = s.load(Some(&s.at("repo")));
        assert_eq!(got.files.len(), 3);
        assert_eq!(got.files[1].path, PathBuf::from("~/secret.md"));
        assert_eq!(got.files[1].when, not_loaded("outside the repository"));
        assert_eq!(got.files[2].when, not_loaded("outside the repository"));
        assert_eq!(got.files[1].chars + got.files[2].chars, 0);
        assert!(got.files.iter().all(|f| f.path != outside));
    }

    #[test]
    fn identical_files_count_once() {
        let s = Setup::new();
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        let agents = s.write("repo/AGENTS.md", "same words");
        let claude = s.write("repo/CLAUDE.md", "same words");
        let got = s.load(Some(&s.at("repo")));
        assert_eq!(row(&got, &agents).when, When::Start);
        assert_eq!(row(&got, &claude).when, not_loaded("same text as AGENTS.md"));
        assert_eq!(row(&got, &claude).chars, 0);
        // A CLAUDE.md that imports AGENTS.md is a different text: both load, and the import counts a second time.
        s.write("repo/CLAUDE.md", "@AGENTS.md");
        let got = s.load(Some(&s.at("repo")));
        let counted: Vec<usize> = got
            .files
            .iter()
            .filter(|f| f.when == When::Start)
            .map(|f| f.chars)
            .collect();
        assert_eq!(counted, vec![10, 10, 10]);
        assert_eq!(got.files[2].role, Role::Import { by: claude });
    }

    #[test]
    fn user_instructions_follow_apply_to() {
        let s = Setup::new();
        let top = s.write("home/.copilot/copilot-instructions.md", "user wide");
        let plain = s.write("home/.copilot/instructions/a.instructions.md", "always");
        let scoped = s.write(
            "home/.copilot/instructions/sub/b.instructions.md",
            "---\napplyTo: \"**/*.py\"\n---\npython",
        );
        s.write("home/.copilot/instructions/c.md", "not an instructions file");
        let got = s.load(None);
        assert_eq!(paths(&got), vec![top.as_path(), plain.as_path(), scoped.as_path()]);
        assert_eq!(row(&got, &top).when, When::Start);
        assert_eq!(row(&got, &plain).when, When::Start);
        assert_eq!(row(&got, &scoped).when, When::WhenUsed);
        assert_eq!(row(&got, &scoped).role, Role::Rule);
    }

    #[test]
    fn the_no_limit_note_is_there() {
        let s = Setup::new();
        let got = s.load(None);
        assert_eq!(
            got.notes,
            vec!["Copilot CLI documents no limit; none was seen up to 256 KB.".to_string()]
        );
    }

    #[test]
    fn nested_agents_and_claude_files_load_when_used() {
        let s = Setup::new();
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        let own = s.write("repo/AGENTS.md", "own");
        let agents = s.write("repo/pkg/AGENTS.md", "nested agents");
        let claude = s.write("repo/pkg/deeper/CLAUDE.md", "nested claude");
        s.write("repo/pkg/GEMINI.md", "not read when nested");
        let got = s.load(Some(&s.at("repo")));
        assert_eq!(paths(&got), vec![own.as_path(), agents.as_path(), claude.as_path()]);
        assert_eq!(row(&got, &agents).when, When::WhenUsed);
        assert_eq!(row(&got, &agents).chars, 13);
        assert_eq!(row(&got, &claude).when, When::WhenUsed);
    }
}
