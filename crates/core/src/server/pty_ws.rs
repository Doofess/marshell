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
use std::time::Duration;
use tokio::time::Instant;

const FLUSH_INTERVAL: Duration = Duration::from_millis(8);
/// Slow safety re-pull: covers a Notify permit eaten by a stale connection and the exit fallback.
const SAFETY_TICK: Duration = Duration::from_millis(250);
const RESUME_TIMEOUT: Duration = Duration::from_secs(5);
const SEND_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_MESSAGE: usize = 1 << 20;

/// Detaches the session when dropped, so a panic or abort also detaches.
struct AttachGuard {
    session: Arc<Session>,
    generation: u64,
}

impl Drop for AttachGuard {
    fn drop(&mut self) {
        self.session.detach(self.generation);
    }
}

pub async fn handler(State(st): State<AppState>, Path(tab): Path<String>, ws: WebSocketUpgrade) -> Response {
    let Some(session) = st.session(&tab) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // Echo our subprotocol, or browsers drop the connection.
    ws.max_message_size(MAX_MESSAGE).protocols([brand::WS_SUBPROTOCOL]).on_upgrade(move |socket| run(socket, session))
}

/// The flush rule. None: send what is pending now. Some(t): hold it until t.
///
/// Leading-edge flush: after an idle gap (no frame within FLUSH_INTERVAL) output goes out at once.
/// Only a sustained stream is coalesced, until FLUSH_INTERVAL after the previous frame or until
/// FLUSH_BYTES are pending, whichever comes first. With nothing pending we never hold, so the
/// caller still pulls and EXIT is noticed.
fn hold_until(avail: u64, last_sent: Option<Instant>, now: Instant) -> Option<Instant> {
    if avail == 0 || avail >= FLUSH_BYTES as u64 {
        return None;
    }
    last_sent.map(|t| t + FLUSH_INTERVAL).filter(|d| *d > now)
}

async fn run(socket: WebSocket, session: Arc<Session>) {
    let (mut tx, mut rx) = socket.split();
    // Set when the first RESUME arrives; nothing is attached before that.
    let mut guard: Option<AttachGuard> = None;
    let mut cursor: Option<u64> = None;
    // When pending output must be flushed at the latest; derived from `hold_until`.
    let mut deadline: Option<Instant> = None;
    // When the last OUTPUT or RESET frame went out (a RESET is a paint too); decides whether
    // new output is coalesced or sent at once.
    let mut last_sent: Option<Instant> = None;
    let resume_by = Instant::now() + RESUME_TIMEOUT;

    loop {
        let mut check = false;
        tokio::select! {
            msg = rx.next() => match msg {
                Some(Ok(Message::Binary(frame))) => match decode_client(&frame) {
                    Some(ClientFrame::Input(bytes)) => session.write_input(bytes),
                    Some(ClientFrame::Resize(r)) => { let _ = session.resize(r.cols, r.rows); }
                    Some(ClientFrame::Ack(n)) => session.ack(n),
                    Some(ClientFrame::Resume(from)) => {
                        if guard.is_none() {
                            guard = Some(AttachGuard { generation: session.attach(), session: session.clone() });
                        }
                        let from = from.min(session.counters().0);
                        session.resume(from);
                        cursor = Some(from);
                        check = true;
                    }
                    None => {}
                },
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            },
            _ = session.notified(), if cursor.is_some() => check = true,
            _ = tokio::time::sleep_until(deadline.unwrap_or_else(Instant::now)), if deadline.is_some() => check = true,
            _ = tokio::time::sleep(SAFETY_TICK), if cursor.is_some() => check = true,
            _ = tokio::time::sleep_until(resume_by), if cursor.is_none() => break,
        }
        if !check {
            continue;
        }
        let Some(mut c) = cursor else { continue };

        let avail = session.available(c);
        // The deadline is always in the future when armed, and tokio never completes a timer
        // early, so this branch never spins.
        if let Some(d) = hold_until(avail, last_sent, Instant::now()) {
            deadline = Some(d);
            continue;
        }
        let mut done = false;
        loop {
            let (frame, exited) = match session.pull(c, FLUSH_BYTES) {
                Pull::Data { seq, bytes } => {
                    c = seq + bytes.len() as u64;
                    (encode_output(seq, &bytes), false)
                }
                Pull::Reset { seq, screen } => {
                    c = seq;
                    (encode_reset(seq, &screen), false)
                }
                Pull::Exit(code) => (encode_exit(code), true),
                Pull::Idle => break,
            };
            match tokio::time::timeout(SEND_TIMEOUT, tx.send(Message::Binary(frame.into()))).await {
                Ok(Ok(())) => last_sent = Some(Instant::now()),
                // Send failed or timed out: the peer is gone or stuck.
                _ => {
                    done = true;
                    break;
                }
            }
            if exited {
                let _ = tokio::time::timeout(SEND_TIMEOUT, tx.send(Message::Close(None))).await;
                done = true;
                break;
            }
        }
        cursor = Some(c);
        deadline = None;
        if done {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn idle_stream_sends_at_once() {
        let now = Instant::now();
        assert_eq!(hold_until(1, None, now), None);
        assert_eq!(hold_until(1, Some(now - 8 * MS), now), None);
        assert_eq!(hold_until(1, Some(now - 20 * MS), now), None);
    }

    #[test]
    fn sustained_stream_holds_until_one_interval_after_the_last_frame() {
        let now = Instant::now();
        assert_eq!(hold_until(1, Some(now - MS), now), Some(now + 7 * MS));
    }

    #[test]
    fn nothing_pending_never_holds_so_exit_is_noticed() {
        let now = Instant::now();
        assert_eq!(hold_until(0, Some(now - MS), now), None);
    }

    #[test]
    fn a_full_frame_is_sent_at_once() {
        let now = Instant::now();
        assert_eq!(hold_until(FLUSH_BYTES as u64, Some(now - MS), now), None);
    }
}
