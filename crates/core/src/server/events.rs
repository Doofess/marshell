//! `GET /v1/events`: pushes the session list to the UI. Every message is a text frame
//! `{"type":"sessions","data":SessionsSnapshot}`: one on connect, then one per change.
//! The UI sends nothing; whatever text it does send is ignored.

use super::AppState;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures_util::SinkExt;
use futures_util::StreamExt;
use marshell_protocol::api::SessionsSnapshot;
use marshell_protocol::brand;
use std::time::Duration;
use tokio::sync::watch;

const SEND_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn handler(State(st): State<AppState>, ws: WebSocketUpgrade) -> Response {
    let rx = st.0.registry.subscribe();
    // Echo our subprotocol, or browsers drop the connection.
    ws.protocols([brand::WS_SUBPROTOCOL])
        .on_upgrade(move |socket| run(socket, rx))
}

fn event(snapshot: &SessionsSnapshot) -> Message {
    let json = serde_json::json!({ "type": "sessions", "data": snapshot });
    Message::Text(json.to_string().into())
}

async fn run(socket: WebSocket, mut rx: watch::Receiver<SessionsSnapshot>) {
    let (mut tx, mut incoming) = socket.split();
    // The first send covers the state at connect; `borrow_and_update` marks it as seen.
    let first = event(&rx.borrow_and_update());
    if !send(&mut tx, first).await {
        return;
    }
    loop {
        tokio::select! {
            changed = rx.changed() => {
                // Err: the registry is gone, so there is nothing more to push.
                if changed.is_err() {
                    break;
                }
                let msg = event(&rx.borrow_and_update());
                if !send(&mut tx, msg).await {
                    break;
                }
            }
            msg = incoming.next() => match msg {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            },
        }
    }
}

/// False when the peer is gone or stuck.
async fn send(tx: &mut futures_util::stream::SplitSink<WebSocket, Message>, msg: Message) -> bool {
    matches!(tokio::time::timeout(SEND_TIMEOUT, tx.send(msg)).await, Ok(Ok(())))
}
