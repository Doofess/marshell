//! Which instruction files Antigravity loads: `GEMINI.md` and `AGENTS.md` in `~/.gemini` and `~/.gemini/config`, the
//! rules in `~/.gemini/config/rules`, then `GEMINI.md` and `AGENTS.md` from the repository root down to the project,
//! and the project's rules folders, each rule by its `trigger`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::doctor::ext::tokens;
use crate::doctor::instructions::display as shown;
use crate::doctor::instructions::{
    CliLoad, Cut, ImportBase, ImportRules, ImportSyntax, LoadedFile, Place, Reader, Role, Text, When, expand,
    frontmatter_value, hash_part,
};

/// Antigravity cuts a rule at this many bytes, includes expanded.
const RULE_BYTES: usize = 24_000;
/// What the always-on and user rules may add up to before the rest are sent as paths.
const RULES_BUDGET_TOKENS: usize = 20_000;
/// A guess: the limit only guards against loops.
const IMPORT_DEPTH: u32 = 5;
const TOO_LARGE: &str = "over 4 MiB, size estimated from its bytes";
const NO_TRIGGER: &str = "no valid trigger";
const OVER_BUDGET: &str = "past Antigravity's 20,000-token budget, sent as a path";
const NESTED: &str = "Antigravity loads it only in a session started in its folder";
const UNKNOWN: &str = "Antigravity documents no limit on includes, so Marshell follows them five deep.";
const DESCRIPTION: &str = "its description loads at start";

/// One file and what it pulls in; the unit the budget counts.
struct Unit {
    rows: Vec<LoadedFile>,
    /// Counted against the rules budget: a user file, or a project rule that is always on.
    counted: bool,
}

struct Run<'a, 'p> {
    place: &'a Place<'p>,
    rules: ImportRules,
    units: Vec<Unit>,
    cuts: Vec<Cut>,
    unreadable: Vec<PathBuf>,
    seen: HashSet<PathBuf>,
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

/// One text of a file's expansion, for finding where the cut falls.
struct Segment {
    row: usize,
    bytes: usize,
    /// None: over 4 MiB, never read.
    text: Option<std::sync::Arc<str>>,
}

impl Run<'_, '_> {
    fn project(&self) -> Option<&Path> {
        self.place.project
    }

    /// Where the cut and budget items say the rules load: the project's folder name, the distro, or every folder.
    fn where_text(&self) -> String {
        match (self.place.distro, self.place.project) {
            (Some(distro), _) => distro.to_string(),
            (None, Some(project)) => project
                .file_name()
                .map_or_else(|| project.display().to_string(), |n| n.to_string_lossy().into_owned()),
            (None, None) => "every folder".to_string(),
        }
    }

    /// One instruction file: its row and its includes, cut at `RULE_BYTES`. `trigger` decides a rule's `when`.
    fn add(&mut self, cx: &mut Reader, path: &Path, role: Role, counted: bool) {
        let text = match cx.read(path) {
            Text::Missing => return,
            Text::Refused | Text::Unreadable => {
                self.unreadable.push(path.to_path_buf());
                return;
            }
            other => other,
        };
        if let Some(canonical) = cx.canonical(path)
            && !self.seen.insert(canonical)
        {
            let rows = vec![row(path, role, When::NotLoaded("already loaded".into()), 0, Vec::new())];
            self.units.push(Unit { rows, counted: false });
            return;
        }
        let is_rule = role == Role::Rule;
        let Text::Text(text) = text else {
            // Over 4 MiB: counted by its bytes, up to the cut. Its trigger can't be read, so a rule is taken as always on.
            let Text::TooLarge { bytes } = text else {
                return;
            };
            let bytes = usize::try_from(bytes).unwrap_or(usize::MAX);
            let mut rows = vec![row(path, role, When::Start, bytes, vec![TOO_LARGE.to_string()])];
            let segments = [Segment {
                row: 0,
                bytes,
                text: None,
            }];
            self.cut(&mut rows, &segments);
            self.units.push(Unit { rows, counted });
            return;
        };
        let trigger = is_rule.then(|| frontmatter_value(&text, "trigger"));
        let when = match trigger.as_ref().map(|t| t.as_deref().map(str::trim)) {
            None | Some(Some("always_on")) => When::Start,
            Some(Some("glob" | "model_decision" | "manual")) => When::WhenUsed,
            Some(_) => {
                let rows = vec![row(path, role, When::NotLoaded(NO_TRIGGER.into()), 0, Vec::new())];
                self.units.push(Unit { rows, counted: false });
                return;
            }
        };
        let mut rows = expand(cx, path, &text, &self.rules, role.clone(), when.clone());
        let segments = self.segments(cx, &rows);
        self.cut(&mut rows, &segments);
        let counted_rule = counted && when == When::Start;
        let kind = trigger.and_then(|t| t).map(|t| t.trim().to_string());
        match kind.as_deref() {
            Some("manual") => rows[0].notes.push("only when you mention it".to_string()),
            Some("model_decision") => {
                rows[0].notes.push(DESCRIPTION.to_string());
                let description = frontmatter_value(&text, "description").unwrap_or_default();
                let description = description.trim();
                if !description.is_empty() {
                    let start = row(
                        path,
                        role,
                        When::Start,
                        description.chars().count(),
                        vec![DESCRIPTION.to_string()],
                    );
                    // The description is what loads at start; the rule's full text is the row after it.
                    self.units.push(Unit {
                        rows: vec![start],
                        // A user rule's description counts against the budget; a project rule's doesn't.
                        counted: counted && self.is_user(path),
                    });
                }
            }
            _ => {}
        }
        self.units.push(Unit {
            rows,
            counted: counted_rule,
        });
    }

    fn is_user(&self, path: &Path) -> bool {
        path.starts_with(&self.place.roots.gemini_dir)
    }

    /// The loaded rows of an expansion with the bytes of each text.
    fn segments(&self, cx: &mut Reader, rows: &[LoadedFile]) -> Vec<Segment> {
        let mut out = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            if matches!(r.when, When::NotLoaded(_)) {
                continue;
            }
            match cx.read(&r.path) {
                Text::Text(text) => out.push(Segment {
                    row: i,
                    bytes: text.len(),
                    text: Some(text),
                }),
                Text::TooLarge { bytes } => out.push(Segment {
                    row: i,
                    bytes: usize::try_from(bytes).unwrap_or(usize::MAX),
                    text: None,
                }),
                _ => {}
            }
        }
        out
    }

    /// Keeps the first `RULE_BYTES` of the file's text with its includes, up to the last line break, and records a
    /// Cut for the rest.
    fn cut(&mut self, rows: &mut [LoadedFile], segments: &[Segment]) {
        let total: usize = segments.iter().map(|s| s.bytes).sum();
        if total <= RULE_BYTES {
            return;
        }
        let mut head = String::new();
        let mut kept = RULE_BYTES;
        for s in segments {
            let room = RULE_BYTES - head.len();
            match &s.text {
                Some(text) if text.len() > room => {
                    head.push_str(&text[..text.floor_char_boundary(room)]);
                    kept = head.rfind('\n').map_or(head.len(), |n| n + 1);
                    break;
                }
                Some(text) => head.push_str(text),
                None => break,
            }
        }
        let mut done = 0;
        for s in segments {
            let left = kept.saturating_sub(done);
            let chars = match &s.text {
                _ if left >= s.bytes => rows[s.row].chars,
                Some(text) => text[..text.floor_char_boundary(left)].chars().count(),
                None => left,
            };
            rows[s.row].chars = chars;
            done += s.bytes;
        }
        rows[0]
            .notes
            .push(format!("cut: Antigravity loads {kept} of {total} bytes"));
        let path = rows[0].path.clone();
        self.cuts.push(Cut {
            id_part: hash_part(&path),
            title: format!(
                "Antigravity cuts {} in {}",
                shown(&path, self.project(), &self.place.roots.home),
                self.where_text()
            ),
            path: Some(path),
            detail: format!(
                "It loads the first 24,000 bytes of each rule, so {} bytes of it never reach the agent.",
                total - kept
            ),
            cannot_fix: "Shorten it, or split it into rules.".to_string(),
        });
    }

    fn rules_in(&mut self, cx: &mut Reader, dir: &Path, counted: bool) {
        for file in cx.list(dir, ".md", false) {
            self.add(cx, &file, Role::Rule, counted);
        }
    }
}

pub fn load(place: &Place, cx: &mut Reader) -> CliLoad {
    let roots = place.roots;
    let mut out = CliLoad {
        agent: "antigravity",
        distro: place.distro.map(str::to_string),
        project: place.project.map(Path::to_path_buf),
        home: roots.home.clone(),
        files: Vec::new(),
        reason: None,
        notes: vec![UNKNOWN.to_string()],
        cuts: Vec::new(),
    };
    let rules = ImportRules {
        syntax: ImportSyntax::AgyLink,
        depth: IMPORT_DEPTH,
        base: ImportBase::File,
        inside: None,
        outside_reason: "outside the project",
        home: Some(roots.home.clone()),
        linux_root: place
            .distro
            .and_then(|_| roots.home.ancestors().last().map(Path::to_path_buf)),
        clean: |text| text.to_string(),
    };
    let mut run = Run {
        place,
        rules,
        units: Vec::new(),
        cuts: Vec::new(),
        unreadable: Vec::new(),
        seen: HashSet::new(),
    };

    // The user's files, in the order Antigravity sends them.
    let config = roots.gemini_dir.join("config");
    for dir in [&roots.gemini_dir, &config] {
        for name in ["GEMINI.md", "AGENTS.md"] {
            run.add(cx, &dir.join(name), Role::File, true);
        }
    }
    run.rules_in(cx, &config.join("rules"), true);

    if let Some(project) = place.project {
        let chain = place.ancestors();
        let root = place.marked_root(cx, &[".git"]);
        let start = root
            .and_then(|root| chain.iter().position(|d| *d == root))
            .unwrap_or_else(|| chain.len().saturating_sub(1));
        for dir in &chain[start..] {
            for name in ["GEMINI.md", "AGENTS.md"] {
                run.add(cx, &dir.join(name), Role::File, false);
            }
        }
        for folder in [".agents", ".agent", "_agents", "_agent"] {
            run.rules_in(cx, &project.join(folder).join("rules"), true);
        }
    }

    // The rules budget: user files and always-on rules, in load order.
    let mut chars = 0;
    let mut over = false;
    let mut paths = Vec::new();
    for unit in &mut run.units {
        let start: usize = unit
            .rows
            .iter()
            .filter(|r| r.when == When::Start)
            .map(|r| r.chars)
            .sum();
        if !unit.counted || start == 0 {
            continue;
        }
        if over || tokens(chars + start) > RULES_BUDGET_TOKENS {
            over = true;
            for r in &mut unit.rows {
                if r.when == When::Start {
                    r.when = When::NotLoaded(OVER_BUDGET.into());
                    r.chars = 0;
                }
            }
            paths.push(shown(&unit.rows[0].path, place.project, &roots.home));
        } else {
            chars += start;
        }
    }
    if over {
        let where_text = run.where_text();
        run.cuts.push(Cut {
            id_part: "budget".into(),
            path: None,
            title: format!("Antigravity sends some rules as paths only in {where_text}"),
            detail: format!(
                "Its always-on and user rules are over 20,000 tokens, so these reach the agent as paths: {}.",
                paths.join(", ")
            ),
            cannot_fix: "Give the rules it needs only sometimes trigger: model_decision or glob.".to_string(),
        });
    }

    // A GEMINI.md or AGENTS.md below the project loads only in a session started in its folder.
    if let Some(project) = place.project {
        let nested = cx.nested(project).clone();
        if nested.stopped {
            out.notes
                .push("Marshell stopped looking for files in subfolders after 10,000 entries.".to_string());
        }
        for path in nested.named(&["GEMINI.md", "AGENTS.md"], place.distro.is_none()) {
            run.units.push(Unit {
                rows: vec![row(&path, Role::File, When::NotLoaded(NESTED.into()), 0, Vec::new())],
                counted: false,
            });
        }
    }

    out.files = run.units.into_iter().flat_map(|u| u.rows).collect();
    out.cuts = run.cuts;
    let mut seen = Vec::new();
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

    fn rule(trigger: &str, body: &str) -> String {
        format!("---\ntrigger: {trigger}\n---\n{body}")
    }

    fn row<'a>(load: &'a CliLoad, path: &Path) -> &'a LoadedFile {
        load.files
            .iter()
            .find(|f| f.path == path)
            .unwrap_or_else(|| panic!("no row for {}", path.display()))
    }

    fn start_chars(load: &CliLoad) -> usize {
        load.files
            .iter()
            .filter(|f| f.when == When::Start)
            .map(|f| f.chars)
            .sum()
    }

    fn not_loaded(reason: &str) -> When {
        When::NotLoaded(reason.into())
    }

    #[test]
    fn user_and_config_files_load() {
        let s = Setup::new();
        let gemini = s.write("home/.gemini/GEMINI.md", "one");
        let agents = s.write("home/.gemini/AGENTS.md", "two!");
        let cfg_gemini = s.write("home/.gemini/config/GEMINI.md", "three");
        let cfg_agents = s.write("home/.gemini/config/AGENTS.md", "four");
        let user_rule = s.write("home/.gemini/config/rules/r.md", &rule("always_on", "rule text"));
        let got = s.load(None);
        let paths: Vec<&Path> = got.files.iter().map(|f| f.path.as_path()).collect();
        assert_eq!(
            paths,
            vec![
                gemini.as_path(),
                agents.as_path(),
                cfg_gemini.as_path(),
                cfg_agents.as_path(),
                user_rule.as_path()
            ]
        );
        assert!(got.files.iter().all(|f| f.when == When::Start));
        assert_eq!(
            got.files.iter().map(|f| f.chars).collect::<Vec<_>>(),
            vec![3, 4, 5, 4, rule("always_on", "rule text").chars().count()]
        );
        assert_eq!(got.files[4].role, Role::Rule);
        assert_eq!(got.files[0].role, Role::File);
        assert_eq!(got.agent, "antigravity");
        assert_eq!(got.notes.len(), 1);
    }

    #[test]
    fn the_chain_runs_from_the_repository_root_to_the_project() {
        let s = Setup::new();
        fs::create_dir_all(s.at("repo/.git")).unwrap();
        let root_gemini = s.write("repo/GEMINI.md", "root");
        let root_agents = s.write("repo/AGENTS.md", "root agents");
        let sub = s.write("repo/pkg/AGENTS.md", "pkg");
        s.write("above.md", "x");
        let below = s.write("repo/pkg/app/deeper/GEMINI.md", "never");
        let below_agents = s.write("repo/pkg/app/AGENTS.md", "app");
        let got = s.load(Some(&s.at("repo/pkg/app")));
        let paths: Vec<&Path> = got
            .files
            .iter()
            .filter(|f| f.when == When::Start)
            .map(|f| f.path.as_path())
            .collect();
        assert_eq!(
            paths,
            vec![
                root_gemini.as_path(),
                root_agents.as_path(),
                sub.as_path(),
                below_agents.as_path()
            ]
        );
        // Finding 8: below the project, only a session started there loads it.
        let nested = row(&got, &below);
        assert_eq!(
            nested.when,
            not_loaded("Antigravity loads it only in a session started in its folder")
        );
        assert_eq!(nested.chars, 0);
    }

    #[test]
    fn rules_follow_their_trigger() {
        let s = Setup::new();
        let dir = "work/.agents/rules";
        let always = s.write(&format!("{dir}/a.md"), &rule("always_on", "always"));
        let glob = s.write(&format!("{dir}/g.md"), &rule("glob\nglobs: \"*.rs\"", "globbed"));
        let manual = s.write(&format!("{dir}/m.md"), &rule("manual", "manual"));
        let none = s.write(&format!("{dir}/n.md"), "no frontmatter");
        let bad = s.write(&format!("{dir}/b.md"), &rule("sometimes", "bad"));
        let underscore = s.write("work/_agent/rules/u.md", &rule("always_on", "u"));
        let got = s.load(Some(&s.at("work")));
        assert_eq!(row(&got, &always).when, When::Start);
        assert_eq!(row(&got, &always).role, Role::Rule);
        assert_eq!(row(&got, &glob).when, When::WhenUsed);
        assert_eq!(row(&got, &manual).when, When::WhenUsed);
        assert_eq!(row(&got, &manual).notes, vec!["only when you mention it".to_string()]);
        assert_eq!(row(&got, &none).when, not_loaded("no valid trigger"));
        assert_eq!(row(&got, &bad).when, not_loaded("no valid trigger"));
        assert_eq!(row(&got, &none).chars, 0);
        assert_eq!(row(&got, &underscore).when, When::Start);
    }

    #[test]
    fn model_decision_counts_its_description() {
        let s = Setup::new();
        let text = "---\ntrigger: model_decision\ndescription: \"Use for database work\"\n---\nbody of the rule\n";
        let file = s.write("work/.agents/rules/db.md", text);
        let got = s.load(Some(&s.at("work")));
        let rows: Vec<&LoadedFile> = got.files.iter().filter(|f| f.path == file).collect();
        assert_eq!(rows.len(), 2);
        let start = rows.iter().find(|f| f.when == When::Start).unwrap();
        assert_eq!(start.chars, "Use for database work".chars().count());
        assert!(start.notes.contains(&"its description loads at start".to_string()));
        let used = rows.iter().find(|f| f.when == When::WhenUsed).unwrap();
        assert_eq!(used.chars, text.chars().count());
        assert!(used.notes.contains(&"its description loads at start".to_string()));
        assert_eq!(start_chars(&got), 21);
    }

    #[test]
    fn an_include_is_read_from_its_base() {
        let s = Setup::new();
        let rule_file = s.write(
            "work/.agents/rules/a.md",
            &rule("always_on", "see @[x](sub/inc.md) and @[y](nope/missing.md)\n"),
        );
        let inc = s.write("work/.agents/rules/sub/inc.md", "included");
        s.write("work/sub/inc.md", "the project's own, which isn't read");
        let got = s.load(Some(&s.at("work")));
        let rows: Vec<&LoadedFile> = got.files.iter().filter(|f| f.path != rule_file).collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].path, inc);
        assert_eq!(rows[0].role, Role::Import { by: rule_file.clone() });
        assert_eq!(rows[0].chars, 8);
        assert_eq!(rows[0].when, When::Start);
        assert_eq!(rows[1].when, not_loaded("not found"));
    }

    #[test]
    fn a_rule_over_24000_bytes_is_cut_on_a_line_break() {
        let s = Setup::new();
        let head = rule("always_on", "");
        let line = format!("{}\n", "x".repeat(99));
        let body = line.repeat(300);
        let file = s.write("work/.agents/rules/big.md", &format!("{head}{body}"));
        let total = head.len() + body.len();
        let got = s.load(Some(&s.at("work")));
        // 24,000 bytes end the head and 99 of the next line: the cut is the line break before.
        let kept = head.len() + ((24_000 - head.len()) / 100) * 100;
        let big = row(&got, &file);
        assert_eq!(big.chars, kept);
        assert_eq!(
            big.notes,
            vec![format!("cut: Antigravity loads {kept} of {total} bytes")]
        );
        assert_eq!(got.cuts.len(), 1);
        let cut = &got.cuts[0];
        assert_eq!(cut.id_part, hash_part(&file));
        assert_eq!(cut.path.as_deref(), Some(file.as_path()));
        assert_eq!(cut.title, "Antigravity cuts .agents/rules/big.md in work");
        assert_eq!(
            cut.detail,
            format!(
                "It loads the first 24,000 bytes of each rule, so {} bytes of it never reach the agent.",
                total - kept
            )
        );
        assert_eq!(cut.cannot_fix, "Shorten it, or split it into rules.");
    }

    #[test]
    fn past_the_budget_rules_become_paths() {
        let s = Setup::new();
        let body = format!("{}\n", "y".repeat(99)).repeat(230);
        let mut files = Vec::new();
        for n in 1..=5 {
            files.push(s.write(&format!("work/.agents/rules/r{n}.md"), &rule("always_on", &body)));
        }
        let got = s.load(Some(&s.at("work")));
        for file in &files[..3] {
            assert_eq!(row(&got, file).when, When::Start);
        }
        for file in &files[3..] {
            let r = row(&got, file);
            assert_eq!(
                r.when,
                not_loaded("past Antigravity's 20,000-token budget, sent as a path")
            );
            assert_eq!(r.chars, 0);
        }
        assert_eq!(got.cuts.len(), 1);
        let cut = &got.cuts[0];
        assert_eq!(cut.id_part, "budget");
        assert_eq!(cut.path, None);
        assert_eq!(cut.title, "Antigravity sends some rules as paths only in work");
        assert_eq!(
            cut.detail,
            "Its always-on and user rules are over 20,000 tokens, so these reach the agent as paths: \
             .agents/rules/r4.md, .agents/rules/r5.md."
        );
        assert_eq!(
            cut.cannot_fix,
            "Give the rules it needs only sometimes trigger: model_decision or glob."
        );
    }
}
