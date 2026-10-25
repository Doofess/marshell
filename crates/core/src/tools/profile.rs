//! The profile type, the shipped set, and the lookups that tell which tool a command line or a process is.

use marshell_protocol::tools::{NeedsYouPattern, OwnToolProblem, ToolProfile};
use regex::Regex;
use std::collections::HashSet;
use std::ops::Deref;
use std::sync::{Arc, LazyLock};

/// A needs-you regex beside the pattern it was built from (the phrase and the kind).
#[derive(Debug, Clone)]
pub struct CompiledNeedsYou {
    pub regex: Regex,
    pub pattern: NeedsYouPattern,
}

/// A tool profile with its regular expressions compiled once, at load.
#[derive(Debug)]
pub struct Profile {
    pub def: ToolProfile,
    needs_you: Vec<CompiledNeedsYou>,
    ports: Vec<Regex>,
}

impl Deref for Profile {
    type Target = ToolProfile;
    fn deref(&self) -> &ToolProfile {
        &self.def
    }
}

impl Profile {
    /// Compiles every pattern of `def`. The error is the reason an own-tool entry is refused.
    pub fn compile(def: ToolProfile) -> Result<Profile, String> {
        let mut needs_you = Vec::with_capacity(def.needs_you.len());
        for n in &def.needs_you {
            let regex = Regex::new(&n.pattern)
                .map_err(|_| format!("The pattern \"{}\" isn't a valid regular expression.", n.pattern))?;
            needs_you.push(CompiledNeedsYou {
                regex,
                pattern: n.clone(),
            });
        }
        let mut ports = Vec::with_capacity(def.ports.len());
        for p in &def.ports {
            let regex = Regex::new(&p.pattern)
                .map_err(|_| format!("The pattern \"{}\" isn't a valid regular expression.", p.pattern))?;
            if regex.captures_len() < 2 {
                return Err(format!(
                    "The port pattern \"{}\" needs a group around the URL or port.",
                    p.pattern
                ));
            }
            ports.push(regex);
        }
        Ok(Profile { def, needs_you, ports })
    }

    /// The needs-you patterns, in the order of `def.needs_you`.
    pub fn needs_you_regexes(&self) -> &[CompiledNeedsYou] {
        &self.needs_you
    }

    /// The port patterns, in the order of `def.ports`. Group 1 is the URL, `host:port`, `[v6]:port` or port.
    pub fn port_regexes(&self) -> &[Regex] {
        &self.ports
    }
}

static SHIPPED: LazyLock<Vec<Arc<Profile>>> = LazyLock::new(|| {
    let defs: Vec<ToolProfile> =
        serde_json::from_str(include_str!("profiles.json")).expect("tools/profiles.json parses");
    defs.into_iter()
        .map(|d| Arc::new(Profile::compile(d).expect("a shipped tool profile compiles")))
        .collect()
});

/// What a command line or a process turned out to be.
#[derive(Debug, Clone, Default)]
pub struct Classified {
    /// The launcher profiles (`npx`) that apply, then the tool's own, in that order.
    pub profiles: Vec<Arc<Profile>>,
    /// The id of the non-launcher profile, if one matched.
    pub tool: Option<String>,
    /// The command line from the tool's program word on, which a profile's `quiet_ports` prefixes are compared with.
    pub rest: String,
}

/// The shipped profiles and the person's valid own ones. Cheap to clone.
#[derive(Clone)]
pub struct Tools(Arc<Vec<Arc<Profile>>>);

impl Tools {
    /// The shipped profiles and no own ones.
    pub fn shipped() -> Self {
        Self::with_own(Vec::new())
    }

    /// The shipped profiles, then `own` (already through `validate_own`; one that does not compile is dropped).
    pub fn with_own(own: Vec<ToolProfile>) -> Self {
        let mut all = SHIPPED.clone();
        for def in own {
            if let Ok(p) = Profile::compile(def) {
                all.push(Arc::new(p));
            }
        }
        Self(Arc::new(all))
    }

    pub fn profile(&self, id: &str) -> Option<&Arc<Profile>> {
        self.0.iter().find(|p| p.def.id == id)
    }

    /// The shipped profiles, then the own ones.
    pub fn profiles(&self) -> impl Iterator<Item = &Arc<Profile>> {
        self.0.iter()
    }

    /// Which tool a command line (as typed in a shell) starts. Launchers (`npx`, `pnpm dlx`, ...) are skipped.
    pub fn classify_line(&self, line: &str) -> Classified {
        self.classify_words(&tokenize(line))
    }

    /// Which tool a process is, from its argument vector. A node or python child is named by its script.
    pub fn classify_argv(&self, argv: &[String]) -> Classified {
        self.by_script(argv).unwrap_or_else(|| self.classify_words(argv))
    }

    /// The profile whose script is the first non-flag argument of a node or python process.
    fn by_script(&self, argv: &[String]) -> Option<Classified> {
        let program = argv.first()?;
        if !is_runtime(&normalize_word(program)) {
            return None;
        }
        let at = 1 + argv[1..].iter().position(|a| !a.starts_with('-'))?;
        let script = argv[at].replace('\\', "/").to_lowercase();
        let p = self.0.iter().find(|p| {
            !p.launcher
                && p.scripts
                    .iter()
                    .any(|s| script.contains(&s.replace('\\', "/").to_lowercase()))
        })?;
        let mut rest = p.commands.first().cloned().unwrap_or_else(|| p.id.clone());
        for a in &argv[at + 1..] {
            rest.push(' ');
            rest.push_str(a);
        }
        Some(Classified {
            profiles: vec![p.clone()],
            tool: Some(p.id.clone()),
            rest,
        })
    }

    fn classify_words(&self, words: &[String]) -> Classified {
        let mut start = 0;
        while words.get(start).is_some_and(|w| w == "&") {
            start += 1;
        }
        let Some(first) = words.get(start) else {
            return Classified::default();
        };
        let mut out = Classified::default();
        let mut at = start;
        if let Some(used) = launcher_words(&normalize_word(first), words.get(start + 1).map(|w| normalize_word(w))) {
            out.profiles.extend(self.0.iter().filter(|p| p.launcher).cloned());
            at = start + used;
            while words.get(at).is_some_and(|w| w.starts_with('-')) {
                at += 1;
            }
        }
        let Some(program) = words.get(at) else {
            return out;
        };
        let key = package_name(&normalize_word(program)).to_string();
        let found = self
            .0
            .iter()
            .find(|p| !p.launcher && p.commands.iter().any(|c| c.eq_ignore_ascii_case(&key)));
        if let Some(p) = found {
            out.profiles.push(p.clone());
            out.tool = Some(p.id.clone());
            out.rest = key;
            for w in &words[at + 1..] {
                out.rest.push(' ');
                out.rest.push_str(w);
            }
        }
        out
    }
}

/// How many words a launcher form takes (`npx` is 1, `pnpm dlx` is 2), or None when the first word is no launcher.
fn launcher_words(first: &str, second: Option<String>) -> Option<usize> {
    match (first, second.as_deref()) {
        ("npx" | "bunx", _) => Some(1),
        ("pnpm", Some("dlx" | "exec")) | ("npm", Some("exec")) | ("yarn", Some("dlx")) => Some(2),
        _ => None,
    }
}

fn is_runtime(program: &str) -> bool {
    matches!(program, "node" | "nodejs" | "py") || program.starts_with("python")
}

/// Splits a command line into words, honouring single and double quotes (a backslash is not an escape).
fn tokenize(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut in_word = false;
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                in_word = true;
            }
            None if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            None => {
                cur.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    words
}

/// A program word as profiles name it: no leading `&`, no quotes, the basename, lowercase, without one of
/// `.exe .cmd .bat .ps1`.
fn normalize_word(word: &str) -> String {
    let w = word.trim().trim_start_matches('&').trim();
    let w = w.trim_matches(|c| c == '"' || c == '\'');
    let base = w.rsplit(['\\', '/']).next().unwrap_or(w).to_lowercase();
    for ext in [".exe", ".cmd", ".bat", ".ps1"] {
        if let Some(stem) = base.strip_suffix(ext) {
            return stem.to_string();
        }
    }
    base
}

/// `vercel@latest` is `vercel`. A leading `@` (a scope) stays.
fn package_name(word: &str) -> &str {
    match word.char_indices().skip(1).find(|(_, c)| *c == '@') {
        Some((i, _)) => &word[..i],
        None => word,
    }
}

fn id_is_well_formed(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn check_one(p: &ToolProfile, shipped: &Tools, seen: &HashSet<String>) -> Result<(), String> {
    if !id_is_well_formed(&p.id) {
        return Err("An id uses lowercase letters, digits and dashes, up to 32.".into());
    }
    if shipped.profile(&p.id).is_some() {
        return Err(format!("The id \"{}\" is taken by a shipped tool.", p.id));
    }
    if seen.contains(&p.id) {
        return Err(format!("Two tools share the id \"{}\".", p.id));
    }
    if p.name.trim().is_empty() {
        return Err("A tool needs a name.".into());
    }
    let named = |v: &[String]| v.iter().any(|s| !s.trim().is_empty());
    if !named(&p.commands) && !named(&p.scripts) {
        return Err("A tool needs a command or a script to recognise it by.".into());
    }
    Profile::compile(p.clone())?;
    for preset in &p.presets {
        for arg in &preset.args {
            let Some(field) = arg.strip_prefix('{').and_then(|a| a.strip_suffix('}')) else {
                continue;
            };
            if !preset.fields.iter().any(|f| f.id == field) {
                return Err(format!(
                    "The preset \"{}\" uses {{{}}}, which it doesn't ask for.",
                    preset.name, field
                ));
            }
        }
        // Task 4 adds the check that refuses a preset argument `secrets::scrub` would change.
    }
    Ok(())
}

/// Splits the own tools from `tools.json` into the usable ones and the refused ones.
pub fn validate_own(list: &[ToolProfile], shipped: &Tools) -> (Vec<ToolProfile>, Vec<OwnToolProblem>) {
    let mut ok = Vec::new();
    let mut problems = Vec::new();
    let mut seen = HashSet::new();
    for (index, p) in list.iter().enumerate() {
        match check_one(p, shipped, &seen) {
            Ok(()) => {
                seen.insert(p.id.clone());
                ok.push(p.clone());
            }
            Err(reason) => problems.push(OwnToolProblem {
                index: index as u32,
                id: Some(p.id.clone()),
                reason,
            }),
        }
    }
    (ok, problems)
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::tools::{LaunchPreset, PortPattern, PresetField, PromptKind};

    fn ids(c: &Classified) -> Vec<&str> {
        c.profiles.iter().map(|p| p.def.id.as_str()).collect()
    }

    #[test]
    fn shipped_profiles_parse_with_unique_ids_and_valid_regexes() {
        let tools = Tools::shipped();
        let got: Vec<&str> = tools.profiles().map(|p| p.def.id.as_str()).collect();
        assert_eq!(
            got,
            [
                "npx",
                "vite",
                "next",
                "vercel",
                "wrangler",
                "netlify",
                "firebase",
                "supabase",
                "docker",
                "gh",
                "kubectl",
                "gcloud",
                "aws",
                "az",
                "terraform",
                "fly",
                "stripe",
                "ngrok",
                "op"
            ]
        );
        for p in tools.profiles() {
            assert_eq!(p.needs_you_regexes().len(), p.def.needs_you.len(), "{}", p.def.id);
            assert_eq!(p.port_regexes().len(), p.def.ports.len(), "{}", p.def.id);
            for re in p.port_regexes() {
                assert!(re.captures_len() > 1, "{}: {} has no group", p.def.id, re.as_str());
            }
            assert!(!p.def.commands.is_empty() || !p.def.scripts.is_empty(), "{}", p.def.id);
        }
        assert!(tools.profile("npx").unwrap().launcher);
        assert!(!tools.profile("vite").unwrap().launcher);
    }

    /// A sample line per shipped port pattern (box characters already replaced by spaces) and group 1 of its match.
    const PORT_SAMPLES: &[(&str, usize, &str, &str)] = &[
        (
            "vite",
            0,
            "  ➜  Local:   http://localhost:5173/",
            "http://localhost:5173/",
        ),
        (
            "next",
            0,
            "   - Local:        http://localhost:3000",
            "http://localhost:3000",
        ),
        (
            "vercel",
            0,
            "> Ready! Available at http://localhost:3000",
            "http://localhost:3000",
        ),
        (
            "wrangler",
            0,
            "[wrangler:info] Ready on http://localhost:8787",
            "http://localhost:8787",
        ),
        (
            "netlify",
            0,
            "   Local dev server ready: http://localhost:8888   ",
            "http://localhost:8888",
        ),
        (
            "firebase",
            0,
            "  Authentication   127.0.0.1:9099   http://127.0.0.1:4000/auth  ",
            "127.0.0.1:9099",
        ),
        (
            "firebase",
            1,
            "i  View Emulator UI at http://127.0.0.1:4000/",
            "http://127.0.0.1:4000/",
        ),
        ("kubectl", 0, "Forwarding from 127.0.0.1:8080 -> 80", "127.0.0.1:8080"),
        ("kubectl", 1, "Starting to serve on 127.0.0.1:8001", "127.0.0.1:8001"),
        (
            "gh",
            0,
            "Forwarding ports: remote 80 <=> local 127.0.0.1:8080",
            "127.0.0.1:8080",
        ),
        ("gcloud", 0, "Listening on port [2222].", "2222"),
        ("aws", 0, "Port 8080 opened for sessionId abc-0123.", "8080"),
    ];

    fn capture(tool: &str, index: usize, line: &str) -> Option<String> {
        let tools = Tools::shipped();
        let re = &tools.profile(tool).unwrap().port_regexes()[index];
        re.captures(line).map(|c| c[1].to_string())
    }

    #[test]
    fn every_shipped_port_pattern_captures_group_one_from_its_sample() {
        let tools = Tools::shipped();
        for (tool, index, line, want) in PORT_SAMPLES {
            assert_eq!(capture(tool, *index, line).as_deref(), Some(*want), "{tool}[{index}]");
        }
        let total: usize = tools.profiles().map(|p| p.def.ports.len()).sum();
        assert_eq!(total, PORT_SAMPLES.len(), "a shipped port pattern has no sample");
        // The v6 shapes.
        assert_eq!(
            capture("kubectl", 0, "Forwarding from [::1]:8080 -> 80").as_deref(),
            Some("[::1]:8080")
        );
        assert_eq!(
            capture("gh", 0, "Forwarding ports: remote 80 <=> local [::]:8080").as_deref(),
            Some("[::]:8080")
        );
        // The table header is not a row.
        assert_eq!(
            capture("firebase", 0, "  Emulator   Host:Port   View in Emulator UI  "),
            None
        );
    }

    #[test]
    fn tools_that_print_secrets_or_nothing_get_no_port_patterns() {
        let tools = Tools::shipped();
        for id in ["supabase", "fly", "docker", "ngrok", "op", "terraform", "stripe"] {
            assert!(tools.profile(id).unwrap().def.ports.is_empty(), "{id}");
        }
        assert!(tools.profile("op").unwrap().def.needs_you.is_empty());
    }

    #[test]
    fn needs_you_patterns_match_their_texts() {
        let tools = Tools::shipped();
        let hit = |tool: &str, line: &str| {
            tools
                .profile(tool)
                .unwrap()
                .needs_you_regexes()
                .iter()
                .find(|n| n.regex.is_match(line))
                .map(|n| (n.pattern.kind, n.pattern.phrase.clone()))
        };
        assert_eq!(hit("npx", "Ok to proceed? ").unwrap().0, PromptKind::Text);
        assert!(hit("npx", "Ok to proceed? (y)").is_none());
        assert!(hit("terraform", "  Enter a value: ").is_some());
        assert!(hit("gcloud", "Do you want to continue (Y/n)?  ").is_some());
        assert_eq!(
            hit("gh", "! First copy your one-time code: 1A2B-3C4D").unwrap().0,
            PromptKind::Browser
        );
        assert!(
            hit(
                "az",
                "go to https://microsoft.com/devicelogin and enter the code ABCD12345"
            )
            .is_some()
        );
        assert!(hit("az", "open https://aka.ms/devicelogin").is_some());
    }

    #[test]
    fn classify_line_skips_launchers() {
        let tools = Tools::shipped();
        let c = tools.classify_line("npx vite --port 3000");
        assert_eq!(ids(&c), ["npx", "vite"]);
        assert_eq!(c.tool.as_deref(), Some("vite"));
        let c = tools.classify_line("pnpm dlx wrangler dev");
        assert_eq!(ids(&c), ["npx", "wrangler"]);
        assert_eq!(c.tool.as_deref(), Some("wrangler"));
        let c = tools.classify_line("bunx --bun vercel dev");
        assert_eq!(ids(&c), ["npx", "vercel"]);
        let c = tools.classify_line("npm exec -- netlify dev");
        assert_eq!(ids(&c), ["npx", "netlify"]);
        let c = tools.classify_line("pnpm exec vite");
        assert_eq!(ids(&c), ["npx", "vite"]);
        let c = tools.classify_line("yarn dlx firebase-tools@latest emulators:start");
        assert_eq!(ids(&c), ["npx", "firebase"]);
        // A launcher with a tool nobody profiled still offers the launcher's own prompt.
        let c = tools.classify_line("npx cowsay hi");
        assert_eq!(ids(&c), ["npx"]);
        assert_eq!(c.tool, None);

        let c = tools.classify_line("& 'C:\\Program Files\\Terraform\\terraform.exe' apply");
        assert_eq!(ids(&c), ["terraform"]);
        assert_eq!(c.tool.as_deref(), Some("terraform"));
        assert_eq!(c.rest, "terraform apply");

        let c = tools.classify_line("docker compose up");
        assert_eq!(c.tool.as_deref(), Some("docker"));
        assert_eq!(c.rest, "docker compose up");

        let c = tools.classify_line("git status");
        assert!(c.profiles.is_empty());
        assert_eq!(c.tool, None);
        assert!(tools.classify_line("").profiles.is_empty());
    }

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn classify_argv_names_a_node_child_by_its_script() {
        let tools = Tools::shipped();
        let c = tools.classify_argv(&argv(&[
            "node.exe",
            "C:\\Users\\a b\\AppData\\Roaming\\npm\\node_modules\\vercel\\dist\\index.js",
            "dev",
        ]));
        assert_eq!(c.tool.as_deref(), Some("vercel"));
        assert_eq!(c.rest, "vercel dev");

        let c = tools.classify_argv(&argv(&[
            "python.exe",
            "C:\\Users\\a\\AppData\\Local\\Google\\Cloud SDK\\google-cloud-sdk\\lib\\gcloud.py",
            "auth",
            "login",
        ]));
        assert_eq!(c.tool.as_deref(), Some("gcloud"));
        assert_eq!(c.rest, "gcloud auth login");

        // A tool run directly, and a node child that is not a profiled script.
        let c = tools.classify_argv(&argv(&["C:\\tools\\Terraform.EXE", "plan"]));
        assert_eq!(c.tool.as_deref(), Some("terraform"));
        assert_eq!(c.rest, "terraform plan");
        assert!(
            tools
                .classify_argv(&argv(&["node.exe", "C:\\work\\server.js"]))
                .profiles
                .is_empty()
        );
    }

    fn own(id: &str) -> ToolProfile {
        ToolProfile {
            id: id.into(),
            name: "Mine".into(),
            commands: vec!["mine".into()],
            scripts: vec![],
            launcher: false,
            needs_you: vec![],
            ports: vec![],
            quiet_ports: vec![],
            presets: vec![],
        }
    }

    fn reason_of(p: ToolProfile) -> String {
        let (ok, problems) = validate_own(&[p], &Tools::shipped());
        assert!(ok.is_empty(), "expected a refusal");
        assert_eq!(problems.len(), 1);
        problems[0].reason.clone()
    }

    fn preset(arg: &str, fields: &[&str]) -> LaunchPreset {
        LaunchPreset {
            id: "p".into(),
            name: "Run it".into(),
            program: "mine".into(),
            args: vec![arg.into()],
            fields: fields
                .iter()
                .map(|f| PresetField {
                    id: (*f).into(),
                    label: (*f).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn own_tool_validation_reasons() {
        assert_eq!(reason_of(own("vite")), "The id \"vite\" is taken by a shipped tool.");
        assert_eq!(
            reason_of(own("Bad Id")),
            "An id uses lowercase letters, digits and dashes, up to 32."
        );
        assert_eq!(
            reason_of(own(&"a".repeat(33))),
            "An id uses lowercase letters, digits and dashes, up to 32."
        );
        let mut p = own("x");
        p.name = " ".into();
        assert_eq!(reason_of(p), "A tool needs a name.");
        let mut p = own("x");
        p.commands.clear();
        assert_eq!(reason_of(p), "A tool needs a command or a script to recognise it by.");
        let mut p = own("x");
        p.needs_you.push(NeedsYouPattern {
            pattern: "(".into(),
            phrase: "Asks to continue".into(),
            kind: PromptKind::Text,
        });
        assert_eq!(reason_of(p), "The pattern \"(\" isn't a valid regular expression.");
        let mut p = own("x");
        p.ports.push(PortPattern { pattern: "[".into() });
        assert_eq!(reason_of(p), "The pattern \"[\" isn't a valid regular expression.");
        let mut p = own("x");
        p.ports.push(PortPattern {
            pattern: "Listening on \\d+".into(),
        });
        assert_eq!(
            reason_of(p),
            "The port pattern \"Listening on \\d+\" needs a group around the URL or port."
        );
        let mut p = own("x");
        p.presets.push(preset("{x}", &[]));
        assert_eq!(
            reason_of(p),
            "The preset \"Run it\" uses {x}, which it doesn't ask for."
        );

        // Two entries with one id: the first stays, the second is refused.
        let (ok, problems) = validate_own(&[own("x"), own("x")], &Tools::shipped());
        assert_eq!(ok.len(), 1);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].index, 1);
        assert_eq!(problems[0].id.as_deref(), Some("x"));
        assert_eq!(problems[0].reason, "Two tools share the id \"x\".");
    }

    #[test]
    fn a_valid_own_tool_is_kept_and_classified() {
        let mut p = own("mine");
        p.ports.push(PortPattern {
            pattern: "serving at (\\S+)".into(),
        });
        p.presets.push(preset("{target}", &["target"]));
        let (ok, problems) = validate_own(&[p], &Tools::shipped());
        assert!(problems.is_empty(), "{problems:?}");
        let tools = Tools::with_own(ok);
        assert_eq!(tools.classify_line("mine --go").tool.as_deref(), Some("mine"));
        assert_eq!(tools.profile("mine").unwrap().port_regexes().len(), 1);
        // The shipped set is unchanged by an own tool.
        assert!(Tools::shipped().profile("mine").is_none());
    }

    #[test]
    #[ignore = "Task 4 adds secrets::scrub and the check that uses it; turn this on then"]
    fn own_tool_preset_holding_a_secret_is_refused() {
        let mut p = own("x");
        p.presets.push(preset("--token=sk_live_0123456789abcdefghijkl", &[]));
        assert_eq!(
            reason_of(p),
            "The preset \"Run it\" holds a secret. Ask for it as a field instead."
        );
    }
}
