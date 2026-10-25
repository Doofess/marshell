mod common;

use futures_util::{SinkExt, StreamExt};
use marshell_core::server::{CoreConfig, RunningCore, start};
use marshell_protocol::api::CreateSessionRequest;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn core() -> (RunningCore, tempfile::TempDir) {
    common::tie_children_to_test();
    let home = tempfile::tempdir().unwrap();
    let core = start(CoreConfig {
        home: home.path().to_path_buf(),
        allowed_origins: vec![],
        programs_dir: None,
        notify: None,
        notify_paused: Default::default(),
        window_hidden: Default::default(),
        import: None,
    })
    .await
    .unwrap();
    (core, home)
}

async fn connect(core: &RunningCore) -> Ws {
    let mut req = format!("ws://127.0.0.1:{}/v1/events", core.endpoint.port)
        .into_client_request()
        .unwrap();
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("marshell.v1, token.{}", core.endpoint.token).parse().unwrap(),
    );
    let (ws, res) = tokio_tungstenite::connect_async(req).await.unwrap();
    assert_eq!(res.headers()["sec-websocket-protocol"], "marshell.v1");
    ws
}

/// Waits for the next text frame and parses it.
async fn next_event(ws: &mut Ws) -> serde_json::Value {
    let msg = tokio::time::timeout(Duration::from_secs(10), ws.next())
        .await
        .expect("no event within 10 s")
        .expect("socket ended")
        .expect("socket error");
    match msg {
        Message::Text(t) => serde_json::from_str(t.as_str()).unwrap(),
        other => panic!("expected a text frame, got {other:?}"),
    }
}

async fn create(core: &RunningCore) {
    let url = format!("http://127.0.0.1:{}/v1/sessions", core.endpoint.port);
    let token = core.endpoint.token.clone();
    let body = CreateSessionRequest {
        shell: Some(vec![env!("CARGO_BIN_EXE_fake-agent").to_string(), "echo".to_string()]),
        cwd: None,
        cols: 80,
        rows: 24,
        profile_id: None,
        agent: None,
        name: None,
        prompt: None,
        mode: None,
        color_scheme: None,
        tool: None,
        preset: None,
    };
    tokio::task::spawn_blocking(move || {
        let res = ureq::post(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .send_json(&body)
            .unwrap();
        assert_eq!(res.status().as_u16(), 200);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn events_send_snapshot_on_connect_and_change() {
    let (core, _home) = core().await;
    let mut ws = connect(&core).await;

    let first = next_event(&mut ws).await;
    assert_eq!(first["type"], "sessions");
    assert_eq!(first["data"]["sessions"].as_array().unwrap().len(), 0);

    // The held permission requests follow the first snapshot, none yet.
    let second = next_event(&mut ws).await;
    assert_eq!(second["type"], "permissions");
    assert_eq!(second["data"], serde_json::json!([]));

    create(&core).await;
    // The session may publish more than one change (reserve, then ready); read until it shows up.
    let mut event = next_event(&mut ws).await;
    while event["type"] != "sessions" || event["data"]["sessions"].as_array().unwrap().is_empty() {
        event = next_event(&mut ws).await;
    }
    assert_eq!(event["type"], "sessions");
    let sessions = event["data"]["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["number"], 1);
}

#[tokio::test]
async fn events_close_when_the_client_closes() {
    let (core, _home) = core().await;
    let mut ws = connect(&core).await;
    next_event(&mut ws).await;
    ws.send(Message::Close(None)).await.unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match ws.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                Some(Ok(_)) => {}
            }
        }
    })
    .await;
    assert!(closed.is_ok(), "socket stayed open after the client's Close");
}
