use super::sessions::program_command;
use super::{AppState, catalog};
use crate::profiles::detect::is_plain_word;
use crate::profiles::store;
use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use marshell_protocol::api::{
    IntegrationInfo, Profile, ProfilesDoc, ShellAgent, ShellAgentProblem, ShellAgents, ShellFamily,
};

pub async fn get(State(st): State<AppState>) -> Json<ProfilesDoc> {
    Json(st.0.profiles.lock().unwrap().clone())
}

/// The shells detection found, as they were found. The profiles page resets an edited one to these.
pub async fn detected(State(st): State<AppState>) -> Json<Vec<Profile>> {
    Json(st.0.detected.clone())
}

/// Whether shell integration applies to this (possibly unsaved) profile, and the script it runs.
pub async fn integration(State(st): State<AppState>, Json(profile): Json<Profile>) -> Json<IntegrationInfo> {
    let dir = st.0.shell_dir.clone();
    Json(crate::shell::describe(&profile, dir.as_deref(), &|k| {
        std::env::var(k).ok()
    }))
}

pub async fn put(
    State(st): State<AppState>,
    Json(doc): Json<ProfilesDoc>,
) -> Result<Json<ProfilesDoc>, (StatusCode, String)> {
    store::validate(&doc).map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;
    let st2 = st.clone();
    // Saving touches the disk: keep it off the async worker threads.
    let saved = tokio::task::spawn_blocking(move || {
        store::save(&st2.0.home, &doc, &st2.0.detected)?;
        Ok::<_, anyhow::Error>(store::normalize(&doc, &st2.0.detected))
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
    *st.0.profiles.lock().unwrap() = saved.clone();
    Ok(Json(saved))
}

type ApiError = (StatusCode, String);

fn failed(err: impl std::fmt::Display) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

/// The agents this shell can start. A Windows shell reads the Windows PATH the way the catalog does; Command Prompt
/// also refuses a program whose path has a space. A WSL distro is asked on its own PATH, which can start a stopped
/// distro, so the probe runs off the async workers and with no lock held.
pub async fn agents(State(st): State<AppState>, UrlPath(id): UrlPath<String>) -> Result<Json<ShellAgents>, ApiError> {
    // A copy: the profiles lock is not held while anything runs.
    let profile = st
        .profiles_snapshot()
        .profiles
        .into_iter()
        .find(|p| p.id == id)
        .ok_or((StatusCode::NOT_FOUND, "No such shell profile.".to_string()))?;
    let (reached, agents) = if profile.family == ShellFamily::Wsl {
        wsl_agents(&st, &profile).await?
    } else {
        (true, windows_agents(&st, &profile).await?)
    };
    Ok(Json(ShellAgents {
        profile_id: profile.id,
        reached,
        agents,
    }))
}

async fn windows_agents(st: &AppState, profile: &Profile) -> Result<Vec<ShellAgent>, ApiError> {
    // `found` is the catalog's own decision, on the PATH this profile's tab gets.
    let (asker, env) = (st.clone(), profile.env.clone());
    let catalog = tokio::task::spawn_blocking(move || catalog::build_for(&asker, &env, false))
        .await
        .map_err(failed)?;
    let mut agents = Vec::with_capacity(catalog.agents.len());
    for agent in catalog.agents {
        let problem = if !agent.found {
            Some(ShellAgentProblem::NotInstalled)
        } else if profile.family == ShellFamily::Cmd {
            // Command Prompt can't start a program whose path has a space.
            match program_command(st, &agent.id, false, &profile.env).await {
                Ok(word) if is_plain_word(&word) => None,
                Ok(_) => Some(ShellAgentProblem::PathHasSpace),
                Err(_) => Some(ShellAgentProblem::NotInstalled),
            }
        } else {
            None
        };
        agents.push(ShellAgent { id: agent.id, problem });
    }
    Ok(agents)
}

async fn wsl_agents(st: &AppState, profile: &Profile) -> Result<(bool, Vec<ShellAgent>), ApiError> {
    let Some(distro) = crate::wsl::distro_of_argv(&profile.command) else {
        return Ok((false, Vec::new()));
    };
    let entries = catalog::wsl_words(st);
    // An agent Marshell doesn't drive in WSL has no word, and is not asked about.
    let asked: Vec<String> = entries.iter().filter_map(|(_, word)| word.clone()).collect();
    let asker = st.clone();
    let found = tokio::task::spawn_blocking(move || asker.agents_in_distro(&distro, &asked))
        .await
        .map_err(failed)?;
    let Some(found) = found else {
        return Ok((false, Vec::new()));
    };
    let agents = entries
        .into_iter()
        .map(|(id, word)| {
            let problem = match word {
                None => Some(ShellAgentProblem::NotDrivenHere),
                Some(word) if found.contains(&word) => None,
                Some(_) => Some(ShellAgentProblem::NotInstalled),
            };
            ShellAgent { id, problem }
        })
        .collect();
    Ok((true, agents))
}

#[cfg(test)]
mod tests {
    use super::super::{AppState, router};
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use marshell_protocol::api::{Profile, ShellAgentProblem, ShellAgents, ShellFamily};
    use std::path::PathBuf;
    use tower::ServiceExt;

    const TOKEN: &str = "test-token";
    const PORT: u16 = 5561;

    fn profile(id: &str, command: &[&str], family: ShellFamily) -> Profile {
        Profile {
            id: id.into(),
            name: id.into(),
            command: command.iter().map(|w| w.to_string()).collect(),
            family,
            cwd: None,
            env: Vec::new(),
            shell_integration: false,
            terminal_theme: None,
            detected: true,
        }
    }

    fn state() -> AppState {
        let home = tempfile::tempdir().unwrap().keep();
        let profiles = vec![
            profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell),
            profile("cmd", &["cmd.exe"], ShellFamily::Cmd),
            profile("ubuntu", &["wsl.exe", "-d", "Ubuntu"], ShellFamily::Wsl),
        ];
        AppState::new(TOKEN.into(), PORT, Vec::new(), home, profiles)
    }

    async fn ask(st: AppState, id: &str, bearer: bool) -> (StatusCode, Vec<u8>) {
        let mut req = Request::builder()
            .method("GET")
            .uri(format!("/v1/profiles/{id}/agents"))
            .header(header::HOST, format!("127.0.0.1:{PORT}"));
        if bearer {
            req = req.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
        }
        let res = router(st).oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, bytes.to_vec())
    }

    async fn shell_agents(st: AppState, id: &str) -> ShellAgents {
        let (status, body) = ask(st, id, true).await;
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        serde_json::from_slice(&body).unwrap()
    }

    fn problem_of(list: &ShellAgents, id: &str) -> Option<ShellAgentProblem> {
        list.agents
            .iter()
            .find(|a| a.id == id)
            .unwrap_or_else(|| panic!("no {id} in {list:?}"))
            .problem
    }

    /// The pinned PATH lookup of the Windows tests: only Claude Code is installed.
    fn only_claude(command: &str) -> Option<PathBuf> {
        (command == "claude").then(|| PathBuf::from(r"C:\tools\claude.exe"))
    }

    fn a_spaced_copilot(command: &str) -> Option<PathBuf> {
        (command == "copilot").then(|| PathBuf::from(r"C:\Users\A B\copilot.exe"))
    }

    #[tokio::test]
    async fn a_wsl_shell_offers_the_driven_clis_its_path_has() {
        // The pinned function runs on a blocking thread: a probe that gets Copilot fails the request.
        let st = state().with_distro_which(|distro, words| {
            assert_eq!(distro, "Ubuntu");
            assert!(!words.iter().any(|w| w == "copilot"), "copilot was probed: {words:?}");
            Some(vec!["claude".into(), "qwen".into()])
        });
        let list = shell_agents(st, "ubuntu").await;
        assert!(list.reached);
        assert_eq!(list.profile_id, "ubuntu");
        assert_eq!(problem_of(&list, "claude"), None);
        assert_eq!(problem_of(&list, "qwen"), None);
        assert_eq!(problem_of(&list, "codex"), Some(ShellAgentProblem::NotInstalled));
        assert_eq!(problem_of(&list, "antigravity"), Some(ShellAgentProblem::NotInstalled));
        assert_eq!(problem_of(&list, "copilot"), Some(ShellAgentProblem::NotDrivenHere));
        let ids: Vec<&str> = list.agents.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(&ids[..5], ["claude", "codex", "antigravity", "qwen", "copilot"]);
    }

    #[tokio::test]
    async fn a_wsl_shell_that_doesnt_answer_is_not_reached() {
        let st = state().with_distro_which(|_, _| None);
        let list = shell_agents(st, "ubuntu").await;
        assert!(!list.reached);
        assert!(list.agents.is_empty());
    }

    #[tokio::test]
    async fn command_prompt_refuses_an_agent_whose_path_has_a_space() {
        let cmd = shell_agents(state().with_agent_path(a_spaced_copilot), "cmd").await;
        assert_eq!(problem_of(&cmd, "copilot"), Some(ShellAgentProblem::PathHasSpace));
        let pwsh = shell_agents(state().with_agent_path(a_spaced_copilot), "pwsh").await;
        assert_eq!(problem_of(&pwsh, "copilot"), None);
    }

    #[tokio::test]
    async fn a_windows_shell_marks_an_agent_off_path_not_installed() {
        let list = shell_agents(state().with_agent_path(only_claude), "pwsh").await;
        assert!(list.reached);
        assert_eq!(problem_of(&list, "claude"), None);
        assert_eq!(problem_of(&list, "codex"), Some(ShellAgentProblem::NotInstalled));
        assert_eq!(problem_of(&list, "copilot"), Some(ShellAgentProblem::NotInstalled));
        // Command Prompt reads the same PATH, and `claude` is a plain word there.
        let cmd = shell_agents(state().with_agent_path(only_claude), "cmd").await;
        assert_eq!(problem_of(&cmd, "claude"), None);
    }

    #[tokio::test]
    async fn an_unknown_profile_is_404() {
        let (status, body) = ask(state(), "nope", true).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(String::from_utf8_lossy(&body), "No such shell profile.");
    }

    #[tokio::test]
    async fn shell_agents_are_refused_without_the_bearer() {
        let (status, _) = ask(state(), "pwsh", false).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
