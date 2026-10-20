//! `GET /v1/events`: pushes the session list and the held permission requests to the UI. Every
//! message is a text frame: `{"type":"sessions","data":SessionsSnapshot}` and
//! `{"type":"permissions","data":[PermissionView]}` (oldest request first). Each kind is sent
//! once on connect (sessions first), then once per change.
//! The UI sends nothing; whatever text it does send is ignored.

use super::AppState;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures_util::SinkExt;
use futures_util::StreamExt;
use marshell_protocol::api::SessionsSnapshot;
use marshell_protocol::brand;
use marshell_protocol::hook::PermissionView;
use std::time::Duration;
use tokio::sync::watch;

const SEND_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn handler(State(st): State<AppState>, ws: WebSocketUpgrade) -> Response {
    let rx = st.0.registry.subscribe();
    let holds = st.holds().subscribe();
    // Echo our subprotocol, or browsers drop the connection.
    ws.protocols([brand::WS_SUBPROTOCOL])
        .on_upgrade(move |socket| run(socket, st, rx, holds))
}

fn event(snapshot: &SessionsSnapshot) -> Message {
    let json = serde_json::json!({ "type": "sessions", "data": snapshot });
    Message::Text(json.to_string().into())
}

fn permissions_event(views: &[PermissionView]) -> Message {
    let json = serde_json::json!({ "type": "permissions", "data": views });
    Message::Text(json.to_string().into())
}

async fn run(
    socket: WebSocket,
    st: AppState,
    mut rx: watch::Receiver<SessionsSnapshot>,
    mut holds: watch::Receiver<u64>,
) {
    let (mut tx, mut incoming) = socket.split();
    // The first send covers the state at connect; `borrow_and_update` marks it as seen.
    let first = event(&rx.borrow_and_update());
    if !send(&mut tx, first).await {
        return;
    }
    // Mark the counter as seen before reading the list, so a change in between sends another frame.
    // The views are cloned under the holds lock, which is released before anything is sent.
    holds.borrow_and_update();
    if !send_permissions(&mut tx, &st, &mut rx).await {
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
            changed = holds.changed() => {
                if changed.is_err() {
                    break;
                }
                holds.borrow_and_update();
                if !send_permissions(&mut tx, &st, &mut rx).await {
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

/// Read the views first, then send any waiting session update before them. A decision updates
/// the row before publishing the removal, even when select chooses the holds channel first.
async fn send_permissions(
    tx: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    st: &AppState,
    rx: &mut watch::Receiver<SessionsSnapshot>,
) -> bool {
    let msg = permissions_event(&st.permission_views());
    if rx.has_changed().unwrap_or(false) {
        let sessions = event(&rx.borrow_and_update());
        if !send(tx, sessions).await {
            return false;
        }
    }
    send(tx, msg).await
}

/// False when the peer is gone or stuck.
async fn send(tx: &mut futures_util::stream::SplitSink<WebSocket, Message>, msg: Message) -> bool {
    matches!(tokio::time::timeout(SEND_TIMEOUT, tx.send(msg)).await, Ok(Ok(())))
}
