//! The agent catalog the launcher reads, and the person's own agents in `agents.json`.

use super::AppState;
use crate::agents::identify::find;
use crate::agents::manifest::{AgentManifest, PromptStyle, shipped};
use crate::agents::own::{self, SaveError};
use crate::agents::presets;
use crate::profiles::detect::mode_args;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::SessionMode;
use marshell_protocol::manifest::{AgentCatalog, CatalogAgent, CatalogPreset, OwnAgents, OwnAgentsSave};

/// The agents with a Rust adapter, in launcher order: id, name, CLI, first-prompt style.
fn adapters() -> [(&'static str, &'static str, &'static str, PromptStyle); 3] {
    [
        ("claude", "Claude Code", "Claude Code", PromptStyle::Positional),
        ("codex", "Codex", "Codex CLI", PromptStyle::Positional),
        (
            "antigravity",
            "Antigravity",
            "Antigravity CLI",
            PromptStyle::Flag { flag: "-i".into() },
        ),
    ]
}

/// The five modes the launcher offers an adapter (`MODES` in `sessionModes.ts`). Don't ask is only reported, never offered.
const OFFERED: [SessionMode; 5] = [
    SessionMode::Manual,
    SessionMode::Plan,
    SessionMode::AutoEdit,
    SessionMode::FullAuto,
    SessionMode::Bypass,
];

fn manifest_entry(st: &AppState, m: &AgentManifest, own: bool) -> CatalogAgent {
    let found = find(m, &|c| (st.0.agent_path)(c), &|p| st.0.agent_versions.get(p)).ok();
    CatalogAgent {
        id: m.id.clone(),
        name: m.name.clone(),
        cli: m.cli.clone(),
        mark: m.mark.clone(),
        adapter: false,
        own,
        found: found.is_some(),
        version: found.and_then(|f| f.version),
        prompt: m.prompt.clone(),
        modes: m.modes.keys().copied().collect(),
        can_continue: m.continue_args.is_some(),
    }
}

/// Runs processes (`--version`) on a cold cache, so it goes through `spawn_blocking`.
fn build(st: &AppState) -> AgentCatalog {
    let agents = st.agents();
    let profiles = st.0.profiles.lock().unwrap_or_else(|e| e.into_inner()).profiles.clone();
    let mut list: Vec<CatalogAgent> = adapters()
        .into_iter()
        .map(|(id, name, cli, prompt)| {
            let exe = st.adapter_exe(&profiles, id);
            let version = exe.as_deref().and_then(|exe| st.agent_version(id, Some(exe)));
            CatalogAgent {
                id: id.into(),
                name: name.into(),
                cli: cli.into(),
                mark: id.into(),
                adapter: true,
                own: false,
                found: exe.is_some(),
                version,
                prompt,
                modes: OFFERED
                    .into_iter()
                    .filter(|mode| mode_args(&agents, id, *mode).is_some())
                    .collect(),
                can_continue: true,
            }
        })
        .collect();
    let shipped_count = shipped().len();
    list.extend(
        agents
            .manifests()
            .enumerate()
            .map(|(i, m)| manifest_entry(st, m, i >= shipped_count)),
    );
    AgentCatalog {
        agents: list,
        // Whether the key is set is all that is read of it; the value goes nowhere.
        presets: vec![CatalogPreset {
            id: presets::DEEPSEEK.into(),
            name: "DeepSeek".into(),
            agent: "claude".into(),
            mark: presets::DEEPSEEK.into(),
            problem: presets::problem(presets::DEEPSEEK, (st.0.deepseek_key)()),
        }],
    }
}

fn failed(err: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

pub async fn catalog(State(st): State<AppState>) -> Result<Json<AgentCatalog>, (StatusCode, String)> {
    let catalog = tokio::task::spawn_blocking(move || build(&st)).await.map_err(failed)?;
    Ok(Json(catalog))
}

pub async fn get_own(State(st): State<AppState>) -> Result<Json<OwnAgents>, (StatusCode, String)> {
    let loaded = tokio::task::spawn_blocking(move || st.0.own_agents.get(&st.0.home))
        .await
        .map_err(failed)?;
    Ok(Json(loaded.own.clone()))
}

/// 409 when the file changed since it was read, 422 with the problems (a list) when the file can't be read or an entry is
/// refused, 200 with the saved agents otherwise.
pub async fn put_own(State(st): State<AppState>, Json(req): Json<OwnAgentsSave>) -> Response {
    let saved = tokio::task::spawn_blocking(move || {
        let _one_at_a_time = st.0.own_agents_write.lock().unwrap_or_else(|e| e.into_inner());
        let saved = own::save(&st.0.home, req);
        // A failed write may still have changed the file.
        st.0.own_agents.forget();
        saved
    })
    .await;
    match saved {
        Ok(Ok(own)) => Json(own).into_response(),
        Ok(Err(SaveError::Stale)) => (StatusCode::CONFLICT, "agents.json changed since you opened it.").into_response(),
        Ok(Err(SaveError::Unreadable(problems) | SaveError::Invalid(problems))) => {
            (StatusCode::UNPROCESSABLE_ENTITY, Json(problems)).into_response()
        }
        Ok(Err(SaveError::Io(e))) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
