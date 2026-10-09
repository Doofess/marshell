use super::frames::{decode_client, encode_exit, encode_output, encode_reset, ClientFrame};
use super::AppState;
use crate::pty::{Pull, Session, FLUSH_BYTES};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use marshell_protocol::brand;
use std::sync::Arc;
use std::time::{Duration, Instant};

const FLUSH_INTERVAL: Duration = Duration::from_millis(8);

pub async fn handler(State(st): State<AppState>, Path(tab): Path<String>, ws: WebSocketUpgrade) -> Response {
    let Some(session) = st.session(&tab) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // Echo our subprotocol, or browsers drop the connection.
    ws.protocols([brand::WS_SUBPROTOCOL]).on_upgrade(move |socket| run(socket, session))
}

async fn run(socket: WebSocket, session: Arc<Session>) {
    let generation = session.attach();
    let (mut tx, mut rx) = socket.split();
    // None until the client sends RESUME.
    let mut cursor: Option<u64> = None;
    let mut last_flush = Instant::now();

    loop {
        tokio::select! {
            msg = rx.next() => match msg {
                Some(Ok(Message::Binary(frame))) => match decode_client(&frame) {
                    Some(ClientFrame::Input(bytes)) => session.write_input(bytes),
                    Some(ClientFrame::Resize(r)) => { let _ = session.resize(r.cols, r.rows); }
                    Some(ClientFrame::Ack(n)) => session.ack(n),
                    Some(ClientFrame::Resume(from)) => { session.resume(from); cursor = Some(from); }
                    None => {}
                },
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            },
            _ = session.notified(), if cursor.is_some() => {}
            _ = tokio::time::sleep(FLUSH_INTERVAL), if cursor.is_some() => {}
        }

        let Some(mut c) = cursor else { continue };
        // Coalesce: flush when 64 KiB is waiting or 8 ms have passed since the last flush.
        if session.available(c) < FLUSH_BYTES as u64 && last_flush.elapsed() < FLUSH_INTERVAL {
            continue;
        }
        let mut exited = false;
        loop {
            let frame = match session.pull(c, FLUSH_BYTES) {
                Pull::Data { seq, bytes } => {
                    c = seq + bytes.len() as u64;
                    encode_output(seq, &bytes)
                }
                Pull::Reset { seq, screen } => {
                    c = seq;
                    encode_reset(seq, &screen)
                }
                Pull::Exit(code) => {
                    exited = true;
                    encode_exit(code)
                }
                Pull::Idle => break,
            };
            if tx.send(Message::Binary(frame.into())).await.is_err() {
                exited = true;
                break;
            }
            if exited {
                break;
            }
        }
        cursor = Some(c);
        last_flush = Instant::now();
        if exited {
            break;
        }
    }
    session.detach(generation);
}
