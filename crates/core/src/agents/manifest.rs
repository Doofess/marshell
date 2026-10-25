//! Agent manifests: the data that lets Marshell find, launch and recognise an agent CLI without a Rust adapter.

use marshell_protocol::api::SessionMode;
pub use marshell_protocol::manifest::{AgentManifest, OwnAgentProblem, PromptStyle};
use std::collections::HashSet;
use std::sync::{Arc, LazyLock};

static SHIPPED: LazyLock<Vec<AgentManifest>> =
    LazyLock::new(|| serde_json::from_str(include_str!("manifests.json")).expect("agents/manifests.json parses"));

/// The manifests Marshell ships, in launcher order.
pub fn shipped() -> &'static [AgentManifest] {
    &SHIPPED
}

/// Frontend tool ids, copied from `TOOL_IDS` in `src/components/AgentMark/tools.ts`. Keep the two in step.
const TOOL_IDS: &[&str] = &[
    "powershell",
    "bash",
    "zsh",
    "cmd",
    "gitbash",
    "wsl",
    "git",
    "docker",
    "kubectl",
    "node",
    "python",
    "cargo",
    "gh",
    "terraform",
    "ssh",
    "aws",
    "gcloud",
    "az",
    "supabase",
    "vercel",
    "program",
];

/// Ids of agents that have a Rust adapter.
const ADAPTER_IDS: &[&str] = &["claude", "codex", "antigravity"];

/// Program names of the adapter agents.
const ADAPTER_EXES: &[&str] = &["claude", "codex", "agy"];

/// Ids and program names an own agent may not take.
pub struct Reserved {
    /// Ids of agents (adapters, shipped manifests, DeepSeek).
    agent_ids: HashSet<String>,
    /// Ids of tools the frontend already has a mark for.
    tool_ids: HashSet<String>,
    exe_names: HashSet<String>,
}

impl Reserved {
    pub fn builtin() -> Self {
        let mut agent_ids: HashSet<String> = ADAPTER_IDS.iter().map(|s| s.to_string()).collect();
        agent_ids.extend(shipped().iter().map(|m| m.id.clone()));
        agent_ids.insert("deepseek".into());
        let tool_ids = TOOL_IDS.iter().map(|s| s.to_string()).collect();
        let mut exe_names: HashSet<String> = ADAPTER_EXES.iter().map(|s| s.to_string()).collect();
        exe_names.extend(
            shipped()
                .iter()
                .flat_map(|m| m.exe_names.iter().map(|e| e.to_lowercase())),
        );
        Self {
            agent_ids,
            tool_ids,
            exe_names,
        }
    }
}

/// The three adapters' launch data plus the shipped and valid own manifests. Cheap to clone.
#[derive(Clone)]
pub struct Agents(Arc<Vec<AgentManifest>>);

impl Agents {
    /// The shipped manifests and no own agents.
    pub fn shipped() -> Self {
        Self::with_own(Vec::new())
    }

    /// The shipped manifests, then `own` (already through `validate_own`).
    pub fn with_own(own: Vec<AgentManifest>) -> Self {
        let mut all = shipped().to_vec();
        all.extend(own);
        Self(Arc::new(all))
    }

    /// The manifest of a manifest agent. None for the adapters (claude, codex, antigravity) and unknown ids.
    pub fn manifest(&self, id: &str) -> Option<&AgentManifest> {
        self.0.iter().find(|m| m.id == id)
    }

    /// An adapter agent or a manifest agent.
    pub fn is_known(&self, id: &str) -> bool {
        ADAPTER_IDS.contains(&id) || self.manifest(id).is_some()
    }

    /// The agent an executable name (as `program_name` gives it) belongs to. Adapters come first.
    pub fn agent_of_exe(&self, name: &str) -> Option<String> {
        if let Some(adapter) = crate::profiles::detect::adapter_of_exe(name) {
            return Some(adapter.to_string());
        }
        self.0
            .iter()
            .find(|m| m.exe_names.iter().any(|e| e.eq_ignore_ascii_case(name)))
            .map(|m| m.id.clone())
    }

    /// The agent an npm package (as `program_of` gives it) belongs to.
    pub fn agent_of_package(&self, name: &str) -> Option<String> {
        if let Some(adapter) = crate::profiles::detect::adapter_of_package(name) {
            return Some(adapter.to_string());
        }
        self.0
            .iter()
            .find(|m| m.packages.iter().any(|p| p.eq_ignore_ascii_case(name)))
            .map(|m| m.id.clone())
    }

    /// The shipped manifests, then the own ones.
    pub fn manifests(&self) -> impl Iterator<Item = &AgentManifest> {
        self.0.iter()
    }
}

const SHELL_CHARS: &[char] = &['&', '|', ';', '<', '>', '^', '%', '$', '`', '"', '\'', '\n', '\r'];

fn id_is_well_formed(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub(crate) fn mode_label(mode: SessionMode) -> &'static str {
    match mode {
        SessionMode::Manual => "Manual",
        SessionMode::Plan => "Plan",
        SessionMode::AutoEdit => "Auto-edit",
        SessionMode::FullAuto => "Full auto",
        SessionMode::Bypass => "Bypass",
        SessionMode::DontAsk => "Don't ask",
    }
}

fn check_one(m: &AgentManifest, reserved: &Reserved, seen: &HashSet<String>) -> Result<(), String> {
    if !id_is_well_formed(&m.id) {
        return Err("An id uses lowercase letters, digits and dashes, up to 32.".into());
    }
    if reserved.agent_ids.contains(&m.id) {
        return Err(format!("The id \"{}\" is taken by a built-in agent.", m.id));
    }
    if reserved.tool_ids.contains(&m.id) {
        return Err(format!("The id \"{}\" is taken by a built-in tool.", m.id));
    }
    if seen.contains(&m.id) {
        return Err(format!("Two agents share the id \"{}\".", m.id));
    }
    if m.name.trim().is_empty() {
        return Err("An agent needs a name.".into());
    }
    if m.commands.is_empty() || m.commands.iter().any(|c| c.trim().is_empty()) {
        return Err("An agent needs a command.".into());
    }
    if let Some(c) = m.commands.iter().find(|c| c.contains(SHELL_CHARS)) {
        return Err(format!("The command \"{c}\" holds a character a shell would act on."));
    }
    if let Some(e) = m
        .exe_names
        .iter()
        .find(|e| reserved.exe_names.contains(&e.to_lowercase()))
    {
        return Err(format!("The program name \"{e}\" belongs to a built-in agent."));
    }
    if m.modes.contains_key(&SessionMode::DontAsk) {
        return Err("Own agents can't map Don't ask.".into());
    }
    if let Some((mode, _)) = m
        .modes
        .iter()
        .find(|(mode, flags)| **mode != SessionMode::Manual && flags.is_empty())
    {
        return Err(format!("The {} mode needs a flag.", mode_label(*mode)));
    }
    Ok(())
}

/// Splits the own agents from `agents.json` into the usable ones (mark forced to `generic`) and the refused ones.
pub fn validate_own(list: &[AgentManifest], reserved: &Reserved) -> (Vec<AgentManifest>, Vec<OwnAgentProblem>) {
    let mut ok = Vec::new();
    let mut problems = Vec::new();
    let mut seen = HashSet::new();
    for (index, m) in list.iter().enumerate() {
        match check_one(m, reserved, &seen) {
            Ok(()) => {
                seen.insert(m.id.clone());
                let mut m = m.clone();
                m.mark = "generic".into();
                ok.push(m);
            }
            Err(reason) => problems.push(OwnAgentProblem {
                index: index as u32,
                id: Some(m.id.clone()),
                reason,
            }),
        }
    }
    (ok, problems)
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::SessionMode;
    use marshell_protocol::manifest::PromptStyle;

    #[test]
    fn shipped_manifests_parse_and_have_unique_ids() {
        let ids: Vec<&str> = shipped().iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "copilot", "cursor", "opencode", "amp", "grok", "qwen", "kimi", "aider", "crush"
            ]
        );
    }

    #[test]
    fn shipped_modes_obey_the_explicit_flag_rule() {
        for m in shipped() {
            assert!(!m.modes.contains_key(&SessionMode::DontAsk), "{} maps dont-ask", m.id);
            for (mode, flags) in &m.modes {
                // Amp runs tools without asking by default, so its Bypass mode has no flag. The one exception.
                let amp_bypass = m.id == "amp" && *mode == SessionMode::Bypass;
                if *mode != SessionMode::Manual && !amp_bypass {
                    assert!(!flags.is_empty(), "{} {:?} has no flag", m.id, mode);
                }
            }
        }
    }

    #[test]
    fn grok_manual_is_an_explicit_default() {
        let grok = shipped().iter().find(|m| m.id == "grok").unwrap();
        assert_eq!(grok.modes[&SessionMode::Manual], ["--permission-mode", "default"]);
    }

    fn own(id: &str) -> AgentManifest {
        AgentManifest {
            id: id.into(),
            name: "Mine".into(),
            cli: "Mine".into(),
            mark: "copilot".into(),
            commands: vec!["mine".into()],
            reject_paths: vec![],
            version_shape: None,
            exe_names: vec![],
            packages: vec![],
            prompt: PromptStyle::None,
            continue_args: None,
            modes: Default::default(),
        }
    }

    fn reason_of(m: AgentManifest) -> String {
        let (ok, problems) = validate_own(&[m], &Reserved::builtin());
        assert!(ok.is_empty(), "expected a refusal");
        assert_eq!(problems.len(), 1);
        problems[0].reason.clone()
    }

    #[test]
    fn own_agent_validation_reasons() {
        assert_eq!(
            reason_of(own("claude")),
            "The id \"claude\" is taken by a built-in agent."
        );
        assert_eq!(
            reason_of(own("docker")),
            "The id \"docker\" is taken by a built-in tool."
        );
        assert_eq!(
            reason_of(own("Bad Id")),
            "An id uses lowercase letters, digits and dashes, up to 32."
        );
        assert_eq!(
            reason_of(own(&"a".repeat(33))),
            "An id uses lowercase letters, digits and dashes, up to 32."
        );
        let mut m = own("x");
        m.name = String::new();
        assert_eq!(reason_of(m), "An agent needs a name.");
        let mut m = own("x");
        m.commands.clear();
        assert_eq!(reason_of(m), "An agent needs a command.");
        for bad in [
            "a\nb", "a&b", "a|b", "a;b", "a<b", "a>b", "a^b", "a%b", "a$b", "a`b", "a\"b", "a'b",
        ] {
            let mut m = own("x");
            m.commands = vec![bad.into()];
            assert_eq!(
                reason_of(m),
                format!("The command \"{bad}\" holds a character a shell would act on.")
            );
        }
        let mut m = own("x");
        m.exe_names = vec!["agy".into()];
        assert_eq!(reason_of(m), "The program name \"agy\" belongs to a built-in agent.");
        let mut m = own("x");
        m.modes.insert(SessionMode::DontAsk, vec!["--x".into()]);
        assert_eq!(reason_of(m), "Own agents can't map Don't ask.");
        let mut m = own("x");
        m.modes.insert(SessionMode::Plan, vec![]);
        assert_eq!(reason_of(m), "The Plan mode needs a flag.");

        let (ok, problems) = validate_own(&[own("x"), own("x")], &Reserved::builtin());
        assert_eq!(ok.len(), 1);
        assert_eq!(problems.len(), 1);
        assert_eq!((problems[0].index, problems[0].id.as_deref()), (1, Some("x")));
        assert_eq!(problems[0].reason, "Two agents share the id \"x\".");
    }

    #[test]
    fn a_valid_own_agent_passes_with_the_generic_mark() {
        let mut m = own("my-agent");
        m.modes.insert(SessionMode::Manual, vec![]);
        m.modes.insert(SessionMode::Bypass, vec!["--yolo".into()]);
        let (ok, problems) = validate_own(&[m], &Reserved::builtin());
        assert!(problems.is_empty());
        assert_eq!(ok[0].mark, "generic");
    }
}
