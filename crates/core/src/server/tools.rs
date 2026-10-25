//! `GET /v1/tools`: the shipped and the person's own tool profiles, with why own entries were refused.

use super::AppState;
use crate::profiles::detect::command_on_path;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use marshell_protocol::tools::{CatalogTool, CatalogToolPreset, ToolCatalog};
use std::collections::HashMap;

/// Reads the file and looks each preset's program up on PATH, so it goes through `spawn_blocking`.
fn build(st: &AppState) -> ToolCatalog {
    let loaded = st.0.own_tools.get(&st.0.home);
    // One lookup per program: several presets run `npx`.
    let mut found: HashMap<&str, bool> = HashMap::new();
    let tools = loaded
        .tools
        .profiles()
        .map(|p| CatalogTool {
            id: p.def.id.clone(),
            name: p.def.name.clone(),
            own: loaded.own.iter().any(|o| o.id == p.def.id),
            presets: p
                .def
                .presets
                .iter()
                .map(|preset| CatalogToolPreset {
                    id: preset.id.clone(),
                    name: preset.name.clone(),
                    fields: preset.fields.clone(),
                    found: *found
                        .entry(preset.program.as_str())
                        .or_insert_with(|| command_on_path(&preset.program)),
                })
                .collect(),
        })
        .collect();
    ToolCatalog {
        tools,
        problems: loaded.problems.clone(),
    }
}

pub async fn catalog(State(st): State<AppState>) -> Result<Json<ToolCatalog>, (StatusCode, String)> {
    let catalog = tokio::task::spawn_blocking(move || build(&st))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(catalog))
}

#[cfg(test)]
mod tests {
    use super::super::router;
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, header};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    const TOKEN: &str = "test-token";
    const PORT: u16 = 5561;

    fn app(home: &std::path::Path) -> axum::Router {
        router(AppState::new(
            TOKEN.into(),
            PORT,
            vec!["http://tauri.localhost".into()],
            home.to_path_buf(),
            Vec::new(),
        ))
    }

    async fn get(app: &axum::Router, bearer: bool) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method("GET")
            .uri("/v1/tools")
            .header(header::HOST, format!("127.0.0.1:{PORT}"));
        if bearer {
            req = req.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
        }
        let res = app.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 22).await.unwrap();
        let body =
            serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, body)
    }

    #[tokio::test]
    async fn tools_route_is_refused_without_the_bearer() {
        let home = tempfile::tempdir().unwrap();
        let (status, _) = get(&app(home.path()), false).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(!crate::paths::tools_file(home.path()).exists());
    }

    #[tokio::test]
    async fn tools_route_lists_shipped_and_own_with_problems() {
        let home = tempfile::tempdir().unwrap();
        let app = app(home.path());
        let (status, body) = get(&app, true).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), crate::tools::Tools::shipped().profiles().count());
        assert!(tools.iter().all(|t| t["own"] == false));
        assert_eq!(body["problems"], json!([]));
        let vite = tools.iter().find(|t| t["id"] == "vite").unwrap();
        assert_eq!(vite["name"], "Vite");
        let dev = &vite["presets"][0];
        assert_eq!(dev["id"], "dev");
        assert!(dev["found"].is_boolean() && dev["fields"] == json!([]));

        std::fs::write(
            crate::paths::tools_file(home.path()),
            json!({"version": 1, "tools": [
                {"id": "mine", "name": "Mine", "commands": ["mine-marshell-test"], "presets": [
                    {"id": "gone", "name": "Not installed", "program": "no-such-program-marshell-test"},
                    {"id": "here", "name": "Installed", "program": "cargo",
                     "args": ["{target}"], "fields": [{"id": "target", "label": "Target"}]}
                ]},
                {"id": "vite", "name": "Taken", "commands": ["x"]}
            ]})
            .to_string(),
        )
        .unwrap();
        let (status, body) = get(&app, true).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.iter().filter(|t| t["own"] == true).count(), 1);
        let mine = tools.last().unwrap();
        assert_eq!((mine["id"].as_str(), mine["own"].as_bool()), (Some("mine"), Some(true)));
        assert_eq!(mine["presets"][0]["found"], false);
        assert_eq!(mine["presets"][1]["found"], true);
        assert_eq!(
            mine["presets"][1]["fields"],
            json!([{"id": "target", "label": "Target"}])
        );
        assert_eq!(
            body["problems"],
            json!([{"index": 1, "id": "vite", "reason": "The id \"vite\" is taken by a shipped tool."}])
        );
    }
}
