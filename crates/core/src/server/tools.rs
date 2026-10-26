//! `GET /v1/tools`: the shipped and the person's own tool profiles, with why own entries were refused. `GET` and
//! `PUT /v1/tools/own`: the person's own profiles as `tools.json` holds them, and saving them.

use super::AppState;
use crate::profiles::detect::command_on_path;
use crate::tools::own::{self, SaveError};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::tools::{CatalogTool, CatalogToolPreset, OwnTools, OwnToolsSave, ToolCatalog};
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

pub async fn get_own(State(st): State<AppState>) -> Result<Json<OwnTools>, (StatusCode, String)> {
    let loaded = tokio::task::spawn_blocking(move || st.0.own_tools.get(&st.0.home))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(loaded.listed.clone()))
}

/// 409 when the file changed since it was read, 422 with the problems (a list) when the file can't be read or an entry is
/// refused, 200 with the saved tools otherwise.
pub async fn put_own(State(st): State<AppState>, Json(req): Json<OwnToolsSave>) -> Response {
    let saved = tokio::task::spawn_blocking(move || {
        let _one_at_a_time = st.0.own_tools_write.lock().unwrap_or_else(|e| e.into_inner());
        let saved = own::save(&st.0.home, req);
        // A failed write may still have changed the file.
        st.0.own_tools.forget();
        saved
    })
    .await;
    match saved {
        Ok(Ok(own)) => Json(own).into_response(),
        Ok(Err(SaveError::Stale)) => (StatusCode::CONFLICT, "tools.json changed since you opened it.").into_response(),
        Ok(Err(SaveError::Unreadable(problems) | SaveError::Invalid(problems))) => {
            (StatusCode::UNPROCESSABLE_ENTITY, Json(problems)).into_response()
        }
        Ok(Err(SaveError::Io(e))) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
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

    async fn send_json(
        app: &axum::Router,
        method: &str,
        uri: &str,
        body: Option<Value>,
        bearer: bool,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::CONTENT_TYPE, "application/json");
        if bearer {
            req = req.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
        }
        let body = body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty);
        let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 22).await.unwrap();
        let body =
            serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, body)
    }

    fn mine(id: &str) -> Value {
        json!({"id": id, "name": "Mine", "commands": [format!("{id}-marshell-test")]})
    }

    #[tokio::test]
    async fn own_tools_routes_are_refused_without_the_bearer() {
        let home = tempfile::tempdir().unwrap();
        let app = app(home.path());
        for method in ["GET", "PUT"] {
            let body = Some(json!({"tools": [], "version": ""}));
            let (status, _) = send_json(&app, method, "/v1/tools/own", body, false).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{method}");
        }
        assert!(!crate::paths::tools_file(home.path()).exists());
    }

    #[tokio::test]
    async fn put_own_saves_and_a_get_and_the_catalog_show_it_with_no_restart() {
        let home = tempfile::tempdir().unwrap();
        let app = app(home.path());
        let (status, before) = send_json(&app, "GET", "/v1/tools/own", None, true).await;
        assert_eq!(status, StatusCode::OK, "{before}");
        assert_eq!(before, json!({"tools": [], "problems": [], "version": ""}));

        let body = Some(json!({"tools": [mine("mine")], "version": ""}));
        let (status, saved) = send_json(&app, "PUT", "/v1/tools/own", body, true).await;
        assert_eq!(status, StatusCode::OK, "{saved}");
        let bytes = std::fs::read(crate::paths::tools_file(home.path())).unwrap();
        assert_eq!(
            saved["version"],
            hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&bytes))
        );
        assert_eq!(saved["tools"][0]["id"], "mine");

        let (_, again) = send_json(&app, "GET", "/v1/tools/own", None, true).await;
        assert_eq!(again, saved);
        let (_, catalog) = get(&app, true).await;
        let own: Vec<&Value> = catalog["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| t["own"] == true)
            .collect();
        assert_eq!(own.len(), 1);
        assert_eq!(own[0]["id"], "mine");
    }

    #[tokio::test]
    async fn put_own_answers_409_with_its_message_and_422_with_a_list() {
        let home = tempfile::tempdir().unwrap();
        let app = app(home.path());
        let body = Some(json!({"tools": [mine("one")], "version": ""}));
        let (_, first) = send_json(&app, "PUT", "/v1/tools/own", body, true).await;
        let file = crate::paths::tools_file(home.path());
        let before = std::fs::read(&file).unwrap();

        // The version it was based on is no longer the file's.
        let body = Some(json!({"tools": [mine("two")], "version": ""}));
        let (status, body) = send_json(&app, "PUT", "/v1/tools/own", body, true).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body, "tools.json changed since you opened it.");
        assert_eq!(std::fs::read(&file).unwrap(), before);

        let body = Some(json!({"tools": [mine("vite"), mine("two")], "version": first["version"]}));
        let (status, body) = send_json(&app, "PUT", "/v1/tools/own", body, true).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(
            body,
            json!([{"index": 0, "id": "vite", "reason": "The id \"vite\" is taken by a shipped tool."}])
        );
        assert_eq!(std::fs::read(&file).unwrap(), before);

        // A file that can't be read is reported as the file's one problem and never overwritten.
        std::fs::write(&file, "{ not json").unwrap();
        let (_, own) = send_json(&app, "GET", "/v1/tools/own", None, true).await;
        let body = Some(json!({"tools": [mine("three")], "version": own["version"]}));
        let (status, body) = send_json(&app, "PUT", "/v1/tools/own", body, true).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body, own["problems"]);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{ not json");
    }

    #[tokio::test]
    async fn put_own_keeps_an_entry_the_form_cannot_show() {
        let home = tempfile::tempdir().unwrap();
        let app = app(home.path());
        let typo = json!({"id": "typo", "name": "Typo", "commands": ["typo-marshell-test"], "colour": "red"});
        std::fs::write(
            crate::paths::tools_file(home.path()),
            json!({"version": 1, "tools": [mine("good"), typo.clone()]}).to_string(),
        )
        .unwrap();
        let (_, own) = send_json(&app, "GET", "/v1/tools/own", None, true).await;
        assert_eq!(own["problems"].as_array().unwrap().len(), 1);
        let mut sent = own["tools"].as_array().unwrap().clone();
        sent.push(mine("added"));
        let body = Some(json!({"tools": sent, "version": own["version"]}));
        let (status, saved) = send_json(&app, "PUT", "/v1/tools/own", body, true).await;
        assert_eq!(status, StatusCode::OK, "{saved}");
        let bytes = std::fs::read(crate::paths::tools_file(home.path())).unwrap();
        let doc: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(doc["tools"][1], typo);
        assert_eq!(doc["tools"][2]["id"], "added");
    }
}
