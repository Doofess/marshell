//! Which instruction files Claude Code loads: the managed and user `CLAUDE.md`, rules, the `CLAUDE.md` chain from the
//! filesystem root down to the project (and `AGENTS.md` by the plugin's mode), the files below the project that load
//! when used, imports, `claudeMdExcludes`, and auto memory. Every read goes through the shared `Reader`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value;

use crate::config::jsonc::read_value;
use crate::doctor::instructions::display as shown;
use crate::doctor::instructions::{
    CliLoad, Cut, ImportBase, ImportRules, ImportSyntax, LoadedFile, Place, Reader, Role, Text, When, expand,
    frontmatter_value, hash_part, strip_html_comments,
};
use crate::projects;

/// Hops below the root file that an `@import` is followed.
const IMPORT_DEPTH: u32 = 4;
/// Claude Code sends the first 200 lines or 25,000 bytes of `MEMORY.md`, whichever is shorter.
const MEMORY_LINES: usize = 200;
const MEMORY_BYTES: usize = 25_000;
/// A file that loads later is cut here: Claude Code loads its start and a note to read the rest.
const LATER_CHARS: usize = 40_000;
/// Claude Code warns at start about a file over this many lines.
const WARN_LINES: usize = 200;

const ONLY_MANAGED: &str = "Claude Code is set to load only managed instructions";
const CLAUDE_ONLY: &str = "Claude Code is set to read CLAUDE.md only";
const CLAUDE_IS_HERE: &str = "CLAUDE.md is here";
const EXCLUDED: &str = "claudeMdExcludes leaves it out";
const ALREADY: &str = "already loaded";
const SKIPPED: &str = "over 4 MiB";
const TOO_LARGE_ESTIMATE: &str = "over 4 MiB, size estimated from its bytes";
const ASKS_ONCE: &str = "Claude Code asks once before loading it";
const MEMORY_NOTE: &str = "first 200 lines or 25 KB";
/// Said until a signed-in Claude Code has shown the three values (7d decision 10).
const UNCONFIRMED: &str = "Not yet confirmed: how deep Claude Code follows imports, where it cuts auto memory, and when \
                           it warns about a long file.";
const WALK_STOPPED: &str = "Marshell stopped looking for files in subfolders after 10,000 entries.";

/// The folder name Claude Code keeps a repository's auto memory under: its root's path with every character that
/// isn't an ASCII letter or digit as `-` (`C:\dev\Airport` is `C--dev-Airport`).
pub fn memory_dir_name(repo_root: &Path) -> String {
    repo_root
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// What the plugin's "Project instructions" setting says.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    ManagedOnly,
    ClaudeMd,
    ClaudeOrAgents,
    ClaudeAndAgents,
}

fn mode_in(settings: Option<&Value>) -> Option<Mode> {
    let value = settings?
        .get("pluginConfigs")?
        .get("cc-plugin-agents-md@builtin")?
        .get("options")?
        .get("instructionFiles")?
        .as_str()?;
    match value {
        "managed-only" => Some(Mode::ManagedOnly),
        "claude-md" => Some(Mode::ClaudeMd),
        "claude-md-or-agents-md" => Some(Mode::ClaudeOrAgents),
        "claude-md-and-agents-md" => Some(Mode::ClaudeAndAgents),
        _ => None,
    }
}

fn json(cx: &mut Reader, file: &Path) -> Option<Value> {
    match cx.read(file) {
        Text::Text(text) => read_value(&text).ok(),
        _ => None,
    }
}

/// The `claudeMdExcludes` arrays of the given settings, merged into one matcher over absolute paths.
fn excludes(settings: &[&Option<Value>], ignore_case: bool) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let mut any = false;
    for value in settings.iter().filter_map(|s| s.as_ref()) {
        let Some(items) = value.get("claudeMdExcludes").and_then(Value::as_array) else {
            continue;
        };
        for pattern in items.iter().filter_map(Value::as_str) {
            let glob = GlobBuilder::new(pattern)
                .literal_separator(true)
                .case_insensitive(ignore_case)
                .build();
            if let Ok(glob) = glob {
                builder.add(glob);
                any = true;
            }
        }
    }
    if any { builder.build().ok() } else { None }
}

/// A path as one comparable text: forward slashes, no trailing slash, ASCII-lowercased on Windows.
fn key(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let text = text.trim_end_matches('/');
    if cfg!(windows) {
        text.to_ascii_lowercase()
    } else {
        text.to_string()
    }
}

fn is_within(path: &Path, base: &Path) -> bool {
    let (path, base) = (key(path), key(base));
    path == base || path.strip_prefix(&base).is_some_and(|rest| rest.starts_with('/'))
}

/// The first `MEMORY_LINES` lines or `MEMORY_BYTES` bytes of a `MEMORY.md`, whichever is shorter, in characters.
fn memory_chars(text: &str) -> usize {
    let lines: usize = text.split_inclusive('\n').take(MEMORY_LINES).map(str::len).sum();
    let mut end = lines.min(MEMORY_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].chars().count()
}

fn row(path: &Path, role: Role, when: When) -> LoadedFile {
    LoadedFile {
        path: path.to_path_buf(),
        role,
        when,
        chars: 0,
        notes: Vec::new(),
        depth: 0,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Managed,
    User,
    Project,
}

struct Run<'a> {
    files: Vec<LoadedFile>,
    unreadable: Vec<PathBuf>,
    cuts: Vec<Cut>,
    /// The canonical paths already counted, so a file reached twice (a link, an import) counts once.
    loaded: HashSet<PathBuf>,
    excludes: Option<GlobSet>,
    project: Option<&'a Path>,
    home: &'a Path,
    rules: ImportRules,
}

impl Run<'_> {
    fn excluded(&self, path: &Path) -> bool {
        let text = path.to_string_lossy().replace('\\', "/");
        self.excludes.as_ref().is_some_and(|set| set.is_match(&text))
    }

    /// The name a Cut's title gives for where the file is.
    fn place_name(&self) -> String {
        match self.project {
            Some(project) => project
                .file_name()
                .map_or_else(|| project.display().to_string(), |n| n.to_string_lossy().into_owned()),
            None => "every folder".to_string(),
        }
    }

    fn cut(&mut self, path: &Path) {
        let id_part = hash_part(path);
        if self.cuts.iter().any(|c| c.id_part == id_part) {
            return;
        }
        self.cuts.push(Cut {
            id_part,
            path: Some(path.to_path_buf()),
            title: format!(
                "Claude Code skips {} in {}",
                shown(path, self.project, self.home),
                self.place_name()
            ),
            detail: "It is over 4 MiB, so Claude Code loads none of it.".to_string(),
            cannot_fix: "Shorten it, or split it into files that load when used.".to_string(),
        });
    }

    /// One file with its imports. `blocked` is the reason it doesn't load; `later` says it loads when used.
    fn add(&mut self, cx: &mut Reader, path: &Path, role: Role, scope: Scope, blocked: Option<&str>, later: bool) {
        let text = match cx.read(path) {
            Text::Missing => return,
            Text::Refused | Text::Unreadable => {
                self.unreadable.push(path.to_path_buf());
                return;
            }
            other => other,
        };
        let blocked = blocked
            .filter(|_| scope != Scope::Managed)
            .or_else(|| (scope != Scope::Managed && self.excluded(path)).then_some(EXCLUDED));
        if let Some(reason) = blocked {
            self.files.push(row(path, role, When::NotLoaded(reason.into())));
            return;
        }
        let canonical = cx.canonical(path);
        if canonical.as_ref().is_some_and(|c| self.loaded.contains(c)) {
            self.files.push(row(path, role, When::NotLoaded(ALREADY.into())));
            return;
        }
        let text = match text {
            Text::Text(text) => text,
            _ => {
                // Over 4 MiB: Claude Code loads none of it.
                self.files.push(row(path, role, When::NotLoaded(SKIPPED.into())));
                self.cut(path);
                return;
            }
        };
        let when = if later || (role == Role::Rule && frontmatter_value(&text, "paths").is_some()) {
            When::WhenUsed
        } else {
            When::Start
        };
        let rows = expand(cx, path, &text, &self.rules, role, when);
        for mut file in rows {
            let canonical = cx.canonical(&file.path);
            let loads = !matches!(file.when, When::NotLoaded(_));
            // Claude Code keeps one list of the files it has taken in, so a file a second file imports isn't sent again.
            if loads
                && matches!(file.role, Role::Import { .. })
                && canonical.as_ref().is_some_and(|c| self.loaded.contains(c))
            {
                file.when = When::NotLoaded(ALREADY.into());
                file.chars = 0;
                file.notes.clear();
                self.files.push(file);
                continue;
            }
            self.finish(cx, &mut file, scope);
            if !matches!(file.when, When::NotLoaded(_))
                && let Some(c) = canonical
            {
                self.loaded.insert(c);
            }
            self.files.push(file);
        }
    }

    /// Notes and corrections to one row of an expansion.
    fn finish(&mut self, cx: &mut Reader, file: &mut LoadedFile, scope: Scope) {
        if matches!(file.role, Role::Import { .. }) && file.notes.iter().any(|n| n == TOO_LARGE_ESTIMATE) {
            // An import over 4 MiB is skipped too.
            file.when = When::NotLoaded(SKIPPED.into());
            file.chars = 0;
            file.notes.clear();
            self.cut(&file.path);
            return;
        }
        if matches!(file.when, When::NotLoaded(_)) {
            return;
        }
        if scope == Scope::Project
            && matches!(file.role, Role::Import { .. })
            && let Some(project) = self.project
            && !is_within(&file.path, project)
        {
            file.notes.push(ASKS_ONCE.to_string());
        }
        if file.when == When::Start
            && let Text::Text(text) = cx.read(&file.path)
            && strip_html_comments(&text).lines().count() > WARN_LINES
        {
            file.notes
                .push(format!("over {WARN_LINES} lines: Claude Code warns at start"));
        }
        if file.when == When::WhenUsed && file.chars > LATER_CHARS {
            file.notes
                .push("over 40,000 characters: Claude Code loads its start and a note to read the rest".to_string());
        }
    }

    /// An auto-memory index: its first lines and bytes, when it has any text.
    fn memory(&mut self, cx: &mut Reader, path: &Path) {
        let notes = vec![MEMORY_NOTE.to_string()];
        let (chars, notes) = match cx.read(path) {
            Text::Text(text) if !text.trim().is_empty() => (memory_chars(&text), notes),
            Text::TooLarge { .. } => (MEMORY_BYTES, [notes, vec![TOO_LARGE_ESTIMATE.to_string()]].concat()),
            Text::Refused | Text::Unreadable => {
                self.unreadable.push(path.to_path_buf());
                return;
            }
            _ => return,
        };
        self.files.push(LoadedFile {
            path: path.to_path_buf(),
            role: Role::Memory,
            when: When::Start,
            chars,
            notes,
            depth: 0,
        });
    }
}

/// The reason an `AGENTS.md` doesn't load in this mode, if it doesn't. `claude_here`: a CLAUDE.md-type file is there.
fn agents_block(mode: Mode, claude_here: bool) -> Option<&'static str> {
    match mode {
        Mode::ManagedOnly => Some(ONLY_MANAGED),
        Mode::ClaudeMd => Some(CLAUDE_ONLY),
        Mode::ClaudeOrAgents => claude_here.then_some(CLAUDE_IS_HERE),
        Mode::ClaudeAndAgents => None,
    }
}

const CLAUDE_NAMES: [&str; 3] = ["CLAUDE.md", ".claude/CLAUDE.md", "CLAUDE.local.md"];

pub fn load(place: &Place, cx: &mut Reader) -> CliLoad {
    let roots = place.roots;
    let mut out = CliLoad {
        agent: "claude",
        distro: place.distro.map(str::to_string),
        project: place.project.map(Path::to_path_buf),
        home: roots.home.clone(),
        files: Vec::new(),
        reason: None,
        notes: Vec::new(),
        cuts: Vec::new(),
    };

    // Settings: managed, then the project's local and shared files, then the user's.
    let managed_settings = place
        .claude_managed
        .and_then(|dir| json(cx, &dir.join("managed-settings.json")));
    let (local, shared) = match place.project {
        Some(project) => (
            json(cx, &project.join(".claude").join("settings.local.json")),
            json(cx, &project.join(".claude").join("settings.json")),
        ),
        None => (None, None),
    };
    let user = json(cx, &roots.claude_settings);

    // Project and local settings can't set the mode.
    let mode = [&managed_settings, &user]
        .into_iter()
        .find_map(|s| mode_in(s.as_ref()))
        .unwrap_or(Mode::ClaudeOrAgents);
    let managed_only = mode == Mode::ManagedOnly;
    let ignore_case = cfg!(windows) && place.distro.is_none();

    let import_rules = ImportRules {
        syntax: ImportSyntax::At,
        depth: IMPORT_DEPTH,
        base: ImportBase::File,
        inside: None,
        outside_reason: "outside the project",
        home: Some(roots.home.clone()),
        linux_root: place
            .distro
            .and_then(|_| roots.home.ancestors().last().map(Path::to_path_buf)),
        clean: strip_html_comments,
    };
    let mut run = Run {
        files: Vec::new(),
        unreadable: Vec::new(),
        cuts: Vec::new(),
        loaded: HashSet::new(),
        excludes: excludes(&[&user, &shared, &local], ignore_case),
        project: place.project,
        home: &roots.home,
        rules: import_rules,
    };
    let user_block = managed_only.then_some(ONLY_MANAGED);

    // Managed, then the user's own file and rules.
    if let Some(dir) = place.claude_managed {
        run.add(cx, &dir.join("CLAUDE.md"), Role::Managed, Scope::Managed, None, false);
    }
    run.add(
        cx,
        &roots.claude_dir.join("CLAUDE.md"),
        Role::File,
        Scope::User,
        user_block,
        false,
    );
    for file in cx.list(&roots.claude_dir.join("rules"), ".md", true) {
        run.add(cx, &file, Role::Rule, Scope::User, user_block, false);
    }

    if let Some(project) = place.project {
        let chain = place.ancestors();
        // The walk up passes the home folder for a project under it. The user's own file there was read above: it
        // isn't a project file, and it doesn't keep AGENTS.md out.
        let users_own = roots.claude_dir.join("CLAUDE.md");
        // A CLAUDE.md-type file in any of these folders keeps the default mode from reading AGENTS.md.
        let claude_here = chain.iter().any(|dir| {
            CLAUDE_NAMES.iter().any(|name| {
                let file = dir.join(name);
                cx.exists(&file) && !cx.same_file(&file, &users_own)
            })
        });
        let chain_agents = agents_block(mode, claude_here);
        for dir in &chain {
            for name in ["CLAUDE.md", ".claude/CLAUDE.md", "CLAUDE.local.md"] {
                let path = name.split('/').fold(dir.clone(), |p, part| p.join(part));
                if cx.same_file(&path, &users_own) {
                    continue;
                }
                run.add(cx, &path, Role::File, Scope::Project, user_block, false);
            }
            for name in ["AGENTS.md", ".claude/AGENTS.md"] {
                let path = name.split('/').fold(dir.clone(), |p, part| p.join(part));
                run.add(cx, &path, Role::File, Scope::Project, chain_agents, false);
            }
        }
        for file in cx.list(&project.join(".claude").join("rules"), ".md", true) {
            run.add(cx, &file, Role::Rule, Scope::Project, user_block, false);
        }

        // Files below the project load when Claude Code reads something there.
        let found = cx.nested(project);
        let stopped = found.stopped;
        let nested = found.named(&["CLAUDE.md", "CLAUDE.local.md", "AGENTS.md"], ignore_case);
        let dot_claude = project.join(".claude");
        for file in nested {
            let Some(parent) = file.parent() else {
                continue;
            };
            // The project's own `.claude` folder was handled above.
            if key(parent) == key(&dot_claude) {
                continue;
            }
            let is_agents = file
                .file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("AGENTS.md"));
            let blocked = if is_agents {
                let own_claude = CLAUDE_NAMES.iter().any(|name| cx.exists(&parent.join(name)));
                agents_block(mode, own_claude)
            } else {
                user_block
            };
            run.add(cx, &file, Role::File, Scope::Project, blocked, true);
        }
        if stopped {
            out.notes.push(WALK_STOPPED.to_string());
        }
    }

    // Auto memory, keyed by the repository's main checkout.
    let enabled = [&managed_settings, &local, &shared, &user]
        .into_iter()
        .find_map(|s| s.as_ref()?.get("autoMemoryEnabled")?.as_bool())
        .unwrap_or(true);
    if enabled && let Some(project) = place.project {
        let own = user
            .as_ref()
            .and_then(|s| s.get("autoMemoryDirectory")?.as_str())
            .map(
                |text| match text.strip_prefix("~/").or_else(|| text.strip_prefix("~\\")) {
                    Some(rest) => roots.home.join(rest),
                    None => PathBuf::from(text),
                },
            )
            .filter(|dir| dir.is_absolute());
        let folder = own.unwrap_or_else(|| {
            let repo = projects::project_of(&project.to_string_lossy());
            roots
                .claude_dir
                .join("projects")
                .join(memory_dir_name(Path::new(&repo)))
                .join("memory")
        });
        run.memory(cx, &folder.join("MEMORY.md"));
    }

    out.files = run.files;
    out.cuts = run.cuts;
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut notes = Vec::new();
    for path in run.unreadable {
        if !seen.contains(&path) {
            notes.push(format!(
                "Marshell can't read {}.",
                shown(&path, place.project, &roots.home)
            ));
            seen.push(path);
        }
    }
    notes.append(&mut out.notes);
    notes.push(UNCONFIRMED.to_string());
    out.notes = notes;
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

        fn load_with(&self, project: Option<&Path>, managed: Option<&Path>) -> CliLoad {
            let place = Place {
                roots: &self.roots,
                project,
                ceiling: Some(self.tmp.path()),
                claude_managed: managed,
                qwen_system_settings: None,
                distro: None,
            };
            load(&place, &mut Reader::windows())
        }

        fn load(&self, project: Option<&Path>) -> CliLoad {
            let managed = self.at("managed");
            self.load_with(project, Some(&managed))
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

    fn mode_settings(mode: &str) -> String {
        format!(
            r#"{{"pluginConfigs":{{"cc-plugin-agents-md@builtin":{{"options":{{"instructionFiles":"{mode}"}}}}}}}}"#
        )
    }

    #[test]
    fn claude_md_and_local_load_ceiling_to_project() {
        let s = Setup::new();
        let top = s.write("CLAUDE.md", "top");
        let root = s.write("proj/CLAUDE.md", "root");
        let local = s.write("proj/CLAUDE.local.md", "mine");
        let dot = s.write("proj/.claude/CLAUDE.md", "dot");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(
            paths(&got),
            vec![top.as_path(), root.as_path(), dot.as_path(), local.as_path()]
        );
        assert!(got.files.iter().all(|f| f.when == When::Start && f.role == Role::File));
        assert_eq!(got.files.iter().map(|f| f.chars).collect::<Vec<_>>(), vec![3, 4, 3, 4]);
        assert_eq!(got.agent, "claude");
        assert_eq!(got.reason, None);
        assert!(got.cuts.is_empty());
        // Nothing above the ceiling: the parent of the tempdir isn't looked at.
        assert!(got.files.iter().all(|f| f.path.starts_with(s.tmp.path())));
    }

    #[test]
    fn user_level_claude_md_and_rules_load_first() {
        let s = Setup::new();
        let user = s.write("home/.claude/CLAUDE.md", "user");
        let always = s.write("home/.claude/rules/sub/a.md", "<!-- hidden -->visible");
        let scoped = s.write("home/.claude/rules/b.md", "---\npaths:\n  - \"src/**\"\n---\nbody");
        let project = s.write("proj/CLAUDE.md", "p");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(
            paths(&got),
            vec![user.as_path(), scoped.as_path(), always.as_path(), project.as_path()]
        );
        assert_eq!(row(&got, &always).role, Role::Rule);
        assert_eq!(row(&got, &always).when, When::Start);
        assert_eq!(row(&got, &always).chars, 7);
        assert_eq!(row(&got, &scoped).when, When::WhenUsed);
    }

    #[test]
    fn settings_come_from_the_roots() {
        let mut s = Setup::new();
        s.roots.claude_settings = s.at("elsewhere/settings.json");
        s.write("elsewhere/settings.json", &mode_settings("claude-md"));
        // The default place for the user settings holds another mode, which must not be read.
        s.write("home/.claude/settings.json", &mode_settings("claude-md-and-agents-md"));
        let agents = s.write("proj/AGENTS.md", "agents");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(
            row(&got, &agents).when,
            not_loaded("Claude Code is set to read CLAUDE.md only")
        );
        assert_eq!(row(&got, &agents).chars, 0);
    }

    #[test]
    fn project_settings_cannot_set_the_mode() {
        let s = Setup::new();
        s.write("proj/.claude/settings.json", &mode_settings("managed-only"));
        s.write("proj/.claude/settings.local.json", &mode_settings("claude-md"));
        let agents = s.write("proj/AGENTS.md", "agents");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(row(&got, &agents).when, When::Start);
    }

    #[test]
    fn the_managed_file_comes_only_from_the_place() {
        let s = Setup::new();
        let managed = s.write("managed/CLAUDE.md", "org rules");
        let got = s.load_with(None, None);
        assert!(got.files.is_empty());
        let got = s.load_with(None, Some(&s.at("managed")));
        assert_eq!(paths(&got), vec![managed.as_path()]);
        assert_eq!(got.files[0].role, Role::Managed);
        assert_eq!(got.files[0].when, When::Start);
        assert_eq!(got.files[0].chars, 9);
    }

    #[test]
    fn the_mode_can_come_from_the_managed_settings() {
        let s = Setup::new();
        s.write("managed/managed-settings.json", &mode_settings("claude-md"));
        s.write("home/.claude/settings.json", &mode_settings("claude-md-and-agents-md"));
        let agents = s.write("proj/AGENTS.md", "agents");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(
            row(&got, &agents).when,
            not_loaded("Claude Code is set to read CLAUDE.md only")
        );
    }

    #[test]
    fn agents_md_is_skipped_when_claude_md_is_here() {
        let s = Setup::new();
        let claude = s.write("proj/CLAUDE.md", "claude");
        let agents = s.write("proj/AGENTS.md", "agents");
        let dot_agents = s.write("proj/.claude/AGENTS.md", "dot agents");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(row(&got, &claude).when, When::Start);
        assert_eq!(row(&got, &agents).when, not_loaded("CLAUDE.md is here"));
        assert_eq!(row(&got, &dot_agents).when, not_loaded("CLAUDE.md is here"));
        assert_eq!(row(&got, &agents).chars, 0);
        // A CLAUDE.md in a folder above counts, a CLAUDE.local.md too, the user's own doesn't.
        fs::remove_file(&claude).unwrap();
        s.write("home/.claude/CLAUDE.md", "user");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(row(&got, &agents).when, When::Start);
        s.write("CLAUDE.local.md", "above");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(row(&got, &agents).when, not_loaded("CLAUDE.md is here"));
    }

    #[test]
    fn agents_md_loads_when_no_claude_md() {
        let s = Setup::new();
        let above = s.write("AGENTS.md", "above");
        let agents = s.write("proj/AGENTS.md", "agents");
        let dot_agents = s.write("proj/.claude/AGENTS.md", "dot agents");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(
            paths(&got),
            vec![above.as_path(), agents.as_path(), dot_agents.as_path()]
        );
        assert!(got.files.iter().all(|f| f.when == When::Start));
        assert_eq!(got.files.iter().map(|f| f.chars).collect::<Vec<_>>(), vec![5, 6, 10]);
    }

    #[test]
    fn and_mode_loads_both_once() {
        let s = Setup::new();
        s.write("home/.claude/settings.json", &mode_settings("claude-md-and-agents-md"));
        let claude = s.write("proj/CLAUDE.md", "claude");
        let agents = s.write("proj/AGENTS.md", "agents");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(paths(&got), vec![claude.as_path(), agents.as_path()]);
        assert!(got.files.iter().all(|f| f.when == When::Start));
        // An AGENTS.md that CLAUDE.md already imports isn't read twice.
        s.write("proj/CLAUDE.md", "see @AGENTS.md\n");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(got.files.len(), 3);
        assert_eq!(got.files[1].path, agents);
        assert_eq!(got.files[1].role, Role::Import { by: claude.clone() });
        assert_eq!(got.files[1].when, When::Start);
        assert_eq!(got.files[2].path, agents);
        assert_eq!(got.files[2].role, Role::File);
        assert_eq!(got.files[2].when, not_loaded("already loaded"));
        assert_eq!(got.files[2].chars, 0);
    }

    #[test]
    fn managed_only_leaves_everything_else_out() {
        let s = Setup::new();
        s.write("managed/managed-settings.json", &mode_settings("managed-only"));
        let managed = s.write("managed/CLAUDE.md", "org");
        let user = s.write("home/.claude/CLAUDE.md", "user");
        let user_rule = s.write("home/.claude/rules/r.md", "rule");
        let claude = s.write("proj/CLAUDE.md", "claude");
        let local = s.write("proj/CLAUDE.local.md", "local");
        let agents = s.write("proj/AGENTS.md", "agents");
        let rule = s.write("proj/.claude/rules/r.md", "rule");
        let nested = s.write("proj/sub/CLAUDE.md", "nested");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(row(&got, &managed).when, When::Start);
        assert_eq!(row(&got, &managed).chars, 3);
        let reason = not_loaded("Claude Code is set to load only managed instructions");
        for file in [&user, &user_rule, &claude, &local, &agents, &rule, &nested] {
            assert_eq!(row(&got, file).when, reason, "{}", file.display());
            assert_eq!(row(&got, file).chars, 0);
        }
    }

    #[test]
    fn a_linked_claude_md_counts_once() {
        let s = Setup::new();
        let agents = s.write("proj/AGENTS.md", "shared text");
        let claude = s.at("proj/CLAUDE.md");
        // A symlink needs Developer Mode (or a privilege) on Windows: without it there is nothing to test.
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_file(&agents, &claude);
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&agents, &claude);
        if made.is_err() {
            return;
        }
        let got = s.load(Some(&s.at("proj")));
        let counted: usize = got
            .files
            .iter()
            .filter(|f| f.when == When::Start)
            .map(|f| f.chars)
            .sum();
        assert_eq!(counted, 11);
        assert_eq!(row(&got, &claude).when, When::Start);
        // In "and" mode the second name is "already loaded".
        s.write("home/.claude/settings.json", &mode_settings("claude-md-and-agents-md"));
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(row(&got, &claude).when, When::Start);
        assert_eq!(row(&got, &agents).when, not_loaded("already loaded"));
        let counted: usize = got
            .files
            .iter()
            .filter(|f| f.when == When::Start)
            .map(|f| f.chars)
            .sum();
        assert_eq!(counted, 11);
    }

    #[test]
    fn path_rules_load_when_used() {
        let s = Setup::new();
        let always = s.write("proj/.claude/rules/always.md", "always");
        let scoped = s.write(
            "proj/.claude/rules/web/api.md",
            "---\npaths: \"src/**/*.ts\"\n---\nbody",
        );
        let plain = s.write("proj/.claude/rules/web/plain.md", "---\ndescription: x\n---\nbody");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(row(&got, &always).when, When::Start);
        assert_eq!(row(&got, &always).role, Role::Rule);
        assert_eq!(row(&got, &scoped).when, When::WhenUsed);
        assert_eq!(row(&got, &scoped).role, Role::Rule);
        assert_eq!(row(&got, &plain).when, When::Start);
    }

    #[test]
    fn excludes_leave_a_file_out() {
        let s = Setup::new();
        s.write(
            "home/.claude/settings.json",
            r#"{"claudeMdExcludes":["**/docs/CLAUDE.md"]}"#,
        );
        s.write(
            "proj/.claude/settings.local.json",
            r#"{"claudeMdExcludes":["**/AGENTS.md"]}"#,
        );
        s.write("managed/CLAUDE.md", "org");
        let docs = s.write("proj/docs/CLAUDE.md", "docs");
        let kept = s.write("proj/CLAUDE.md", "kept");
        let agents = s.write("proj/AGENTS.md", "agents");
        let other = s.write("proj/pkg/CLAUDE.md", "pkg");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(row(&got, &docs).when, not_loaded("claudeMdExcludes leaves it out"));
        assert_eq!(row(&got, &docs).chars, 0);
        assert_eq!(row(&got, &other).when, When::WhenUsed);
        assert_eq!(row(&got, &kept).when, When::Start);
        // The AGENTS.md is left out by the local file's pattern (the mode never gets to it: CLAUDE.md is here).
        assert!(matches!(row(&got, &agents).when, When::NotLoaded(_)));
        // The managed file is never excluded.
        s.write("home/.claude/settings.json", r#"{"claudeMdExcludes":["**/CLAUDE.md"]}"#);
        let got = s.load(Some(&s.at("proj")));
        let managed = s.at("managed/CLAUDE.md");
        assert_eq!(row(&got, &managed).when, When::Start);
        assert_eq!(row(&got, &kept).when, not_loaded("claudeMdExcludes leaves it out"));
    }

    #[test]
    fn imports_follow_the_file_and_count() {
        let s = Setup::new();
        let inside = s.write("proj/docs/in.md", "inside");
        let outside = s.write("shared/out.md", "outside text");
        let user_only = s.write("home/notes.md", "from home");
        let root = s.write(
            "proj/CLAUDE.md",
            "a @docs/in.md b @../shared/out.md c @~/notes.md d @gone.md\n",
        );
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(got.files.len(), 5);
        assert_eq!(got.files[0].path, root);
        assert_eq!(got.files[1].path, inside);
        assert_eq!(got.files[1].role, Role::Import { by: root.clone() });
        assert!(got.files[1].notes.is_empty());
        assert_eq!(got.files[2].path, s.at("shared/out.md"));
        assert_eq!(got.files[2].when, When::Start);
        assert_eq!(got.files[2].chars, 12);
        assert_eq!(
            got.files[2].notes,
            vec!["Claude Code asks once before loading it".to_string()]
        );
        assert_eq!(got.files[3].path, user_only);
        assert_eq!(got.files[3].when, When::Start);
        assert_eq!(got.files[4].when, not_loaded("not found"));
        let _ = outside;
    }

    #[test]
    fn the_users_own_file_is_not_a_project_file_above_the_project() {
        let s = Setup::new();
        // The project is under the home folder, so the walk up passes `<home>/.claude/CLAUDE.md`.
        let user = s.write("home/.claude/CLAUDE.md", "mine");
        let agents = s.write("home/work/proj/AGENTS.md", "agents");
        let got = s.load(Some(&s.at("home/work/proj")));
        let paths: Vec<&Path> = got.files.iter().map(|f| f.path.as_path()).collect();
        assert_eq!(paths, vec![user.as_path(), agents.as_path()]);
        assert_eq!(row(&got, &agents).when, When::Start);
    }

    #[test]
    fn the_limits_not_yet_confirmed_are_said() {
        let s = Setup::new();
        s.write("proj/CLAUDE.md", "rules");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(
            got.notes,
            vec![
                "Not yet confirmed: how deep Claude Code follows imports, where it cuts auto memory, and when it warns \
                 about a long file."
                    .to_string()
            ]
        );
    }

    #[test]
    fn a_file_two_files_import_counts_once() {
        let s = Setup::new();
        let shared = s.write("proj/docs/shared.md", "shared words");
        let first = s.write("proj/CLAUDE.md", "one @docs/shared.md\n");
        let second = s.write("proj/CLAUDE.local.md", "two @docs/shared.md\n");
        let got = s.load(Some(&s.at("proj")));
        let rows: Vec<(&Path, &When, usize)> = got
            .files
            .iter()
            .filter(|f| f.path == shared)
            .map(|f| (f.path.as_path(), &f.when, f.chars))
            .collect();
        assert_eq!(
            rows,
            vec![
                (shared.as_path(), &When::Start, 12),
                (shared.as_path(), &not_loaded("already loaded"), 0)
            ]
        );
        assert_eq!(got.files[1].role, Role::Import { by: first });
        assert_eq!(got.files[3].role, Role::Import { by: second });
    }

    #[test]
    fn imports_stop_at_four_hops() {
        let s = Setup::new();
        s.write("proj/CLAUDE.md", "@a.md\n");
        s.write("proj/a.md", "@b.md\n");
        s.write("proj/b.md", "@c.md\n");
        s.write("proj/c.md", "@d.md\n");
        s.write("proj/d.md", "@e.md\n");
        s.write("proj/e.md", "end");
        let got = s.load(Some(&s.at("proj")));
        let depths: Vec<u32> = got.files.iter().map(|f| f.depth).collect();
        assert_eq!(depths, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(got.files[4].when, When::Start);
        assert_eq!(got.files[5].when, not_loaded("past the import limit"));
    }

    #[test]
    fn memory_counts_its_first_200_lines() {
        let s = Setup::new();
        let project = s.at("work/proj");
        fs::create_dir_all(&project).unwrap();
        let file = s.write(
            &format!("home/.claude/projects/{}/memory/MEMORY.md", memory_dir_name(&project)),
            &"line\n".repeat(250),
        );
        let got = s.load(Some(&project));
        let r = row(&got, &file);
        assert_eq!(r.role, Role::Memory);
        assert_eq!(r.when, When::Start);
        assert_eq!(r.chars, 1_000);
        assert_eq!(r.notes, vec!["first 200 lines or 25 KB".to_string()]);
        // With no project, no memory row.
        let got = s.load(None);
        assert!(got.files.is_empty());
    }

    #[test]
    fn memory_stops_at_25000_bytes() {
        let s = Setup::new();
        let project = s.at("work/proj");
        fs::create_dir_all(&project).unwrap();
        let line = format!("{}\n", "x".repeat(149));
        let file = s.write(
            &format!("home/.claude/projects/{}/memory/MEMORY.md", memory_dir_name(&project)),
            &line.repeat(300),
        );
        let got = s.load(Some(&project));
        assert_eq!(row(&got, &file).chars, 25_000);
    }

    #[test]
    fn a_worktree_shares_its_repos_memory() {
        let s = Setup::new();
        let main = s.at("main");
        let git_dir = s.at("main/.git/worktrees/wt");
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(git_dir.join("commondir"), "../..\n").unwrap();
        let worktree = s.at("wt");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(worktree.join(".git"), format!("gitdir: {}\n", git_dir.display())).unwrap();
        let name = memory_dir_name(&crate::projects::canonical(&main));
        let shared = s.write(&format!("home/.claude/projects/{name}/memory/MEMORY.md"), "shared\n");
        // The worktree's own name has a file too, which must not be read.
        s.write(
            &format!("home/.claude/projects/{}/memory/MEMORY.md", memory_dir_name(&worktree)),
            "own\n",
        );
        let got = s.load(Some(&worktree));
        let memory: Vec<_> = got.files.iter().filter(|f| f.role == Role::Memory).collect();
        assert_eq!(memory.len(), 1);
        assert_eq!(memory[0].path, shared);
        assert_eq!(memory[0].chars, 7);
    }

    #[test]
    fn auto_memory_directory_moves_the_folder() {
        let s = Setup::new();
        let project = s.at("work/proj");
        fs::create_dir_all(&project).unwrap();
        s.write(
            &format!("home/.claude/projects/{}/memory/MEMORY.md", memory_dir_name(&project)),
            "default\n",
        );
        let moved = s.at("elsewhere/mem").to_string_lossy().replace('\\', "/");
        s.write(
            "home/.claude/settings.json",
            &format!(r#"{{"autoMemoryDirectory":"{moved}"}}"#),
        );
        s.write("elsewhere/mem/MEMORY.md", "moved text\n");
        let got = s.load(Some(&project));
        let memory: Vec<_> = got.files.iter().filter(|f| f.role == Role::Memory).collect();
        assert_eq!(memory.len(), 1);
        assert_eq!(memory[0].path, PathBuf::from(&moved).join("MEMORY.md"));
        assert_eq!(memory[0].chars, 11);
    }

    #[test]
    fn memory_off_when_auto_memory_is_disabled() {
        let s = Setup::new();
        let project = s.at("work/proj");
        fs::create_dir_all(&project).unwrap();
        s.write(
            &format!("home/.claude/projects/{}/memory/MEMORY.md", memory_dir_name(&project)),
            "memory\n",
        );
        s.write(
            "work/proj/.claude/settings.local.json",
            r#"{"autoMemoryEnabled":false}"#,
        );
        let got = s.load(Some(&project));
        assert!(got.files.iter().all(|f| f.role != Role::Memory));
        s.write("work/proj/.claude/settings.local.json", r#"{"autoMemoryEnabled":true}"#);
        let got = s.load(Some(&project));
        assert!(got.files.iter().any(|f| f.role == Role::Memory));
    }

    #[test]
    fn a_later_file_over_40000_characters_gets_the_note() {
        let s = Setup::new();
        let big = s.write("proj/sub/CLAUDE.md", &"a".repeat(40_001));
        let fine = s.write("proj/other/CLAUDE.md", &"a".repeat(40_000));
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(row(&got, &big).when, When::WhenUsed);
        assert_eq!(
            row(&got, &big).notes,
            vec!["over 40,000 characters: Claude Code loads its start and a note to read the rest".to_string()]
        );
        assert!(row(&got, &fine).notes.is_empty());
    }

    #[test]
    fn a_file_over_200_lines_warns_at_start() {
        let s = Setup::new();
        let long = s.write("proj/CLAUDE.md", &"x\n".repeat(201));
        let short = s.write("proj/AGENTS.md", &"x\n".repeat(200));
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(
            row(&got, &long).notes,
            vec!["over 200 lines: Claude Code warns at start".to_string()]
        );
        assert!(row(&got, &short).notes.is_empty());
    }

    #[test]
    fn a_file_over_4_mib_is_a_cut() {
        let s = Setup::new();
        let path = s.write("proj/CLAUDE.md", &"a".repeat(4 * 1024 * 1024 + 1));
        let got = s.load(Some(&s.at("proj")));
        let r = row(&got, &path);
        assert_eq!(r.when, not_loaded("over 4 MiB"));
        assert_eq!(r.chars, 0);
        assert_eq!(got.cuts.len(), 1);
        let cut = &got.cuts[0];
        assert_eq!(cut.id_part, hash_part(&path));
        assert_eq!(cut.path.as_deref(), Some(path.as_path()));
        assert_eq!(cut.title, "Claude Code skips CLAUDE.md in proj");
        assert_eq!(cut.detail, "It is over 4 MiB, so Claude Code loads none of it.");
        assert_eq!(
            cut.cannot_fix,
            "Shorten it, or split it into files that load when used."
        );
    }

    #[test]
    fn nested_files_load_when_used() {
        let s = Setup::new();
        s.write("proj/.claude/CLAUDE.md", "own dot file, loaded at start");
        let sub = s.write("proj/sub/CLAUDE.md", "sub");
        let local = s.write("proj/sub/CLAUDE.local.md", "sub local");
        let sub_agents = s.write("proj/sub/AGENTS.md", "sub agents");
        let other_agents = s.write("proj/other/AGENTS.md", "other agents");
        s.write("proj/other/README.md", "ignored");
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(row(&got, &sub).when, When::WhenUsed);
        assert_eq!(row(&got, &sub).chars, 3);
        assert_eq!(row(&got, &local).when, When::WhenUsed);
        assert_eq!(row(&got, &sub_agents).when, not_loaded("CLAUDE.md is here"));
        assert_eq!(row(&got, &other_agents).when, When::WhenUsed);
        // The project's own .claude/CLAUDE.md is a start file, listed once.
        let dot = s.at("proj/.claude/CLAUDE.md");
        assert_eq!(got.files.iter().filter(|f| f.path == dot).count(), 1);
        assert_eq!(row(&got, &dot).when, When::Start);
        // Claude-only mode leaves a nested AGENTS.md out.
        s.write("home/.claude/settings.json", &mode_settings("claude-md"));
        let got = s.load(Some(&s.at("proj")));
        assert_eq!(
            row(&got, &other_agents).when,
            not_loaded("Claude Code is set to read CLAUDE.md only")
        );
    }

    #[test]
    fn a_stopped_walk_gives_its_note() {
        let s = Setup::new();
        s.write("proj/a/CLAUDE.md", "a");
        s.write("proj/b/c.txt", "b");
        s.write("proj/c/d.txt", "c");
        let place = Place {
            roots: &s.roots,
            project: Some(&s.at("proj")),
            ceiling: Some(s.tmp.path()),
            claude_managed: None,
            qwen_system_settings: None,
            distro: None,
        };
        let got = load(&place, &mut Reader::with_walk_cap(2));
        assert_eq!(
            got.notes[0],
            "Marshell stopped looking for files in subfolders after 10,000 entries."
        );
    }

    #[test]
    fn memory_dir_name_replaces_non_alphanumerics() {
        assert_eq!(memory_dir_name(Path::new(r"C:\dev\Airport")), "C--dev-Airport");
        assert_eq!(memory_dir_name(Path::new("/home/Ann/My App.2")), "-home-Ann-My-App-2");
    }
}
