mod common;

use futures_util::{SinkExt, StreamExt};
use marshell_core::pty::Pull;
use marshell_core::server::{CoreConfig, RunningCore, start};
use marshell_protocol::api::{CreateSessionRequest, CreateSessionResponse};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

async fn core() -> (RunningCore, tempfile::TempDir) {
    common::tie_children_to_test();
    let home = tempfile::tempdir().unwrap();
    let core = start(CoreConfig {
        home: home.path().to_path_buf(),
        allowed_origins: vec![],
        programs_dir: None,
        notify: None,
        focus: None,
        notify_paused: Default::default(),
        window_hidden: Default::default(),
        import: None,
    })
    .await
    .unwrap();
    (core, home)
}

fn fake(args: &[&str]) -> Vec<String> {
    let mut v = vec![env!("CARGO_BIN_EXE_fake-agent").to_string()];
    v.extend(args.iter().map(|s| s.to_string()));
    v
}

async fn create(core: &RunningCore, shell: Vec<String>, cwd: Option<String>) -> Result<String, (u16, String)> {
    let url = format!("http://127.0.0.1:{}/v1/sessions", core.endpoint.port);
    let token = core.endpoint.token.clone();
    let body = CreateSessionRequest {
        shell: Some(shell),
        cwd,
        cols: 80,
        rows: 24,
        profile_id: None,
        agent: None,
        name: None,
        prompt: None,
        mode: None,
        setup: None,
        color_scheme: None,
        tool: None,
        preset: None,
        cli_task: None,
    };
    tokio::task::spawn_blocking(move || {
        let res = ureq::post(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .config()
            .http_status_as_error(false)
            .build()
            .send_json(&body)
            .unwrap();
        let status = res.status().as_u16();
        let mut res = res;
        if status == 200 {
            Ok(res.body_mut().read_json::<CreateSessionResponse>().unwrap().tab_id)
        } else {
            Err((status, res.body_mut().read_to_string().unwrap_or_default()))
        }
    })
    .await
    .unwrap()
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(core: &RunningCore, tab: &str, resume_from: u64) -> Ws {
    let mut req = format!("ws://127.0.0.1:{}/v1/pty/{tab}", core.endpoint.port)
        .into_client_request()
        .unwrap();
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("marshell.v1, token.{}", core.endpoint.token).parse().unwrap(),
    );
    let (mut ws, res) = tokio_tungstenite::connect_async(req).await.unwrap();
    assert_eq!(res.headers()["sec-websocket-protocol"], "marshell.v1");
    let mut resume = vec![0x13];
    resume.extend_from_slice(&resume_from.to_be_bytes());
    ws.send(Message::Binary(resume.into())).await.unwrap();
    ws
}

struct Received {
    bytes: Vec<u8>,
    exit: Option<i32>,
    end: u64,
    resets: usize,
    /// The offset of every LIVE frame, in arrival order.
    live: Vec<u64>,
}

/// Reads frames, acking each, until EXIT, `stop` matches, or the timeout passes.
async fn read_until(ws: &mut Ws, timeout: Duration, stop: impl Fn(&[u8]) -> bool) -> Received {
    let mut r = Received {
        bytes: vec![],
        exit: None,
        end: 0,
        resets: 0,
        live: vec![],
    };
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline && !stop(&r.bytes) {
        let f = match tokio::time::timeout(Duration::from_millis(200), ws.next()).await {
            Err(_) => continue,
            Ok(Some(Ok(Message::Binary(f)))) => f,
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) | Ok(Some(Err(_))) => break,
            Ok(Some(Ok(_))) => continue,
        };
        let seq = || u64::from_be_bytes(f[1..9].try_into().unwrap());
        match f[0] {
            0x01 => {
                r.bytes.extend_from_slice(&f[9..]);
                r.end = seq() + (f.len() - 9) as u64;
            }
            0x03 => {
                r.resets += 1;
                r.bytes.extend_from_slice(&f[9..]);
                r.end = seq();
            }
            0x02 => {
                r.exit = Some(i32::from_be_bytes(f[1..5].try_into().unwrap()));
                break;
            }
            // LIVE carries no output and needs no ack.
            0x04 => {
                r.live.push(seq());
                continue;
            }
            _ => {}
        }
        let mut ack = vec![0x12];
        ack.extend_from_slice(&r.end.to_be_bytes());
        ws.send(Message::Binary(ack.into())).await.unwrap();
    }
    r
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[tokio::test]
async fn echo_round_trip_and_exit() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["echo"]), None).await.unwrap();
    let mut ws = connect(&core, &tab, 0).await;
    let mut input = vec![0x10];
    input.extend_from_slice(b"ping\r");
    ws.send(Message::Binary(input.into())).await.unwrap();
    let got = read_until(&mut ws, Duration::from_secs(10), |b| text(b).contains("got:ping")).await;
    assert!(text(&got.bytes).contains("got:ping"));
    let mut input = vec![0x10];
    input.extend_from_slice(b"exit\r");
    ws.send(Message::Binary(input.into())).await.unwrap();
    let rest = read_until(&mut ws, Duration::from_secs(10), |_| false).await;
    assert_eq!(rest.exit, Some(0));
}

#[tokio::test]
async fn cwd_with_space() {
    let (core, _home) = core().await;
    let dir = tempfile::Builder::new().prefix("has space ").tempdir().unwrap();
    let tab = create(&core, fake(&["cwd"]), Some(dir.path().display().to_string()))
        .await
        .unwrap();
    let mut ws = connect(&core, &tab, 0).await;
    let got = read_until(&mut ws, Duration::from_secs(10), |_| false).await;
    let name = dir.path().file_name().unwrap().to_string_lossy().into_owned();
    assert!(text(&got.bytes).contains(&name), "cwd output: {}", text(&got.bytes));
}

/// The session sees its number and agent in its environment.
#[cfg(windows)]
#[tokio::test]
async fn session_env_has_number() {
    let (core, _home) = core().await;
    let shell = vec![
        "cmd".to_string(),
        "/c".to_string(),
        "echo env=%MARSHELL_SESSION_NUMBER%,%MARSHELL_AGENT%".to_string(),
    ];
    let tab = create(&core, shell, None).await.unwrap();
    let mut ws = connect(&core, &tab, 0).await;
    let got = read_until(&mut ws, Duration::from_secs(10), |b| text(b).contains("env=1,shell")).await;
    assert!(text(&got.bytes).contains("env=1,shell"), "output: {}", text(&got.bytes));
}

#[cfg(unix)]
#[tokio::test]
async fn session_env_has_number() {
    let (core, _home) = core().await;
    let shell = vec![
        "sh".to_string(),
        "-c".to_string(),
        "echo env=$MARSHELL_SESSION_NUMBER,$MARSHELL_AGENT".to_string(),
    ];
    let tab = create(&core, shell, None).await.unwrap();
    let mut ws = connect(&core, &tab, 0).await;
    let got = read_until(&mut ws, Duration::from_secs(10), |b| text(b).contains("env=1,shell")).await;
    assert!(text(&got.bytes).contains("env=1,shell"), "output: {}", text(&got.bytes));
}

#[tokio::test]
async fn utf8_passthrough() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["print", "héllo ✓ 日本"]), None).await.unwrap();
    let mut ws = connect(&core, &tab, 0).await;
    let got = read_until(&mut ws, Duration::from_secs(10), |_| false).await;
    assert!(text(&got.bytes).contains("héllo ✓ 日本"), "got: {:?}", text(&got.bytes));
}

#[tokio::test]
async fn bad_shell_is_an_error() {
    let (core, _home) = core().await;
    let err = create(&core, vec!["definitely-not-a-shell-xyz".into()], None)
        .await
        .unwrap_err();
    assert!(err.0 >= 400, "status {}", err.0);
    assert!(!err.1.is_empty());
}

#[tokio::test]
async fn reconnect_resumes_without_duplicates() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["echo"]), None).await.unwrap();
    let mut ws = connect(&core, &tab, 0).await;
    let mut input = vec![0x10];
    input.extend_from_slice(b"one\r");
    ws.send(Message::Binary(input.into())).await.unwrap();
    let first = read_until(&mut ws, Duration::from_secs(10), |b| text(b).contains("got:one")).await;
    drop(ws);
    let mut ws = connect(&core, &tab, first.end).await;
    let mut input = vec![0x10];
    input.extend_from_slice(b"two\r");
    ws.send(Message::Binary(input.into())).await.unwrap();
    let second = read_until(&mut ws, Duration::from_secs(10), |b| text(b).contains("got:two")).await;
    assert!(
        !text(&second.bytes).contains("got:one"),
        "replayed output that was already acked"
    );
    assert!(text(&second.bytes).contains("got:two"));
}

/// The next binary frame of `ws`, which must be LIVE: returns its offset.
async fn expect_live_first(ws: &mut Ws) -> u64 {
    let f = loop {
        match tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .expect("no frame")
        {
            Some(Ok(Message::Binary(f))) => break f,
            Some(Ok(_)) => continue,
            other => panic!("socket ended before the first frame: {other:?}"),
        }
    };
    assert_eq!(f[0], 0x04, "the first frame must be LIVE, got opcode {}", f[0]);
    assert_eq!(f.len(), 9);
    u64::from_be_bytes(f[1..9].try_into().unwrap())
}

/// Starts an echo session, lets it print one line, and waits until its output stops growing.
/// Returns the tab and the final length of its output.
async fn session_with_one_line(core: &RunningCore) -> (String, u64) {
    let tab = create(core, fake(&["echo"]), None).await.unwrap();
    let mut ws = connect(core, &tab, 0).await;
    let mut input = vec![0x10];
    input.extend_from_slice(b"ping\r");
    ws.send(Message::Binary(input.into())).await.unwrap();
    let first = read_until(&mut ws, Duration::from_secs(10), |b| text(b).contains("got:ping")).await;
    assert!(text(&first.bytes).contains("got:ping"));
    drop(ws);
    let session = core.state.session(&tab).unwrap();
    let mut stable = (session.counters().0, Instant::now());
    while stable.1.elapsed() < Duration::from_millis(300) {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let end = session.counters().0;
        if end != stable.0 {
            stable = (end, Instant::now());
        }
    }
    (tab, stable.0)
}

#[tokio::test]
async fn resume_sends_live_before_the_replay() {
    let (core, _home) = core().await;
    let (tab, end) = session_with_one_line(&core).await;
    let mut ws = connect(&core, &tab, 0).await;
    let live = expect_live_first(&mut ws).await;
    assert_eq!(live, end, "live starts where the output ended at attach");
    // The replay covers exactly 0..live.
    let got = read_until(&mut ws, Duration::from_secs(10), |b| b.len() as u64 >= live).await;
    assert_eq!(got.resets, 0);
    assert_eq!(got.bytes.len() as u64, live);
    assert_eq!(got.end, live);
    assert!(text(&got.bytes).contains("got:ping"));
    assert!(got.live.is_empty(), "one Resume gets one LIVE");
}

#[tokio::test]
async fn a_reconnect_gets_its_own_live() {
    let (core, _home) = core().await;
    let (tab, end) = session_with_one_line(&core).await;
    let mut ws = connect(&core, &tab, 0).await;
    assert_eq!(expect_live_first(&mut ws).await, end);
    drop(ws);
    let mut ws = connect(&core, &tab, end).await;
    assert_eq!(expect_live_first(&mut ws).await, end);
}

/// Reads Output frames as (offset, length), acking each, until the output reaches `until`. Other frames are skipped.
async fn output_frames_until(ws: &mut Ws, until: u64) -> Vec<(u64, usize)> {
    let mut frames = vec![];
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut end = 0;
    while end < until {
        assert!(Instant::now() < deadline, "output stopped at {end} of {until}");
        let f = match tokio::time::timeout(Duration::from_millis(200), ws.next()).await {
            Err(_) => continue,
            Ok(Some(Ok(Message::Binary(f)))) => f,
            other => panic!("socket ended at {end} of {until}: {other:?}"),
        };
        if f[0] != 0x01 {
            continue;
        }
        let seq = u64::from_be_bytes(f[1..9].try_into().unwrap());
        frames.push((seq, f.len() - 9));
        end = seq + (f.len() - 9) as u64;
        let mut ack = vec![0x12];
        ack.extend_from_slice(&end.to_be_bytes());
        ws.send(Message::Binary(ack.into())).await.unwrap();
    }
    frames
}

/// No replayed Output frame may cross the live offset. The cap is only visible when the session holds output
/// from after attach, so the test asks for the replay a second time, on the same socket (which keeps its LIVE
/// offset), once the child has printed more.
#[tokio::test]
async fn no_replayed_frame_straddles_the_live_offset() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["echo"]), None).await.unwrap();
    let mut ws = connect(&core, &tab, 0).await;
    expect_live_first(&mut ws).await;

    // Print well over two flushes (FLUSH_BYTES = 64 KiB), then wait for the last line.
    let line = "x".repeat(200);
    for _ in 0..400 {
        let mut input = vec![0x10];
        input.extend_from_slice(format!("{line}\r").as_bytes());
        ws.send(Message::Binary(input.into())).await.unwrap();
    }
    let mut input = vec![0x10];
    input.extend_from_slice(b"done\r");
    ws.send(Message::Binary(input.into())).await.unwrap();
    let printed = read_until(&mut ws, Duration::from_secs(20), |b| text(b).contains("got:done")).await;
    assert!(text(&printed.bytes).contains("got:done"));
    drop(ws);
    let session = core.state.session(&tab).unwrap();
    let mut stable = (session.counters().0, Instant::now());
    while stable.1.elapsed() < Duration::from_millis(300) {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let end = session.counters().0;
        if end != stable.0 {
            stable = (end, Instant::now());
        }
    }

    // Attach again: the output so far is the replay, and `live` is where it ends.
    let mut ws = connect(&core, &tab, 0).await;
    let live = expect_live_first(&mut ws).await;
    assert_eq!(live, stable.0);
    assert!(
        live > 2 * 64 * 1024 + 1000,
        "the replay must span several flushes, got {live} bytes"
    );
    assert_ne!(
        live % (64 * 1024),
        0,
        "a live offset on a flush boundary cannot show a straddle"
    );
    // Output from after attach: a line the replay does not hold yet.
    let mut input = vec![0x10];
    input.extend_from_slice(b"after\r");
    ws.send(Message::Binary(input.into())).await.unwrap();
    let seen = read_until(&mut ws, Duration::from_secs(10), |b| text(b).contains("got:after")).await;
    assert!(text(&seen.bytes).contains("got:after"));
    assert!(seen.end > live);
    // The echo's last bytes can trail "got:after"; let the output settle so the replay's length is known.
    let mut stable = (session.counters().0, Instant::now());
    while stable.1.elapsed() < Duration::from_millis(300) {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let end = session.counters().0;
        if end != stable.0 {
            stable = (end, Instant::now());
        }
    }
    let total = stable.0;

    // The second Resume replays from 0 again, with the session now holding bytes past `live`.
    let mut resume = vec![0x13];
    resume.extend_from_slice(&0u64.to_be_bytes());
    ws.send(Message::Binary(resume.into())).await.unwrap();
    // Frames still in flight from before the Resume come first; the Resume's own LIVE precedes its replay.
    loop {
        match tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .expect("no LIVE for the second Resume")
        {
            Some(Ok(Message::Binary(f))) if f[0] == 0x04 => {
                assert_eq!(u64::from_be_bytes(f[1..9].try_into().unwrap()), live);
                break;
            }
            Some(Ok(_)) => continue,
            other => panic!("socket ended before the second LIVE: {other:?}"),
        }
    }
    let frames = output_frames_until(&mut ws, total).await;
    for &(seq, len) in &frames {
        let end = seq + len as u64;
        assert!(
            end <= live || seq >= live,
            "frame {seq}..{end} straddles live at {live}"
        );
    }
    assert!(
        frames.iter().any(|&(seq, len)| seq + len as u64 == live),
        "no frame ends at live ({live})"
    );
    assert!(
        frames.iter().any(|&(seq, _)| seq >= live),
        "no output continues past live"
    );
}

#[tokio::test]
async fn evicted_cursor_gets_a_reset() {
    let (core, _home) = core().await;
    // 9 MiB with nobody attached overflows the 8 MiB ring.
    let tab = create(&core, fake(&["flood", "9437184"]), None).await.unwrap();
    let session = core.state.session(&tab).unwrap();
    let overflow = Instant::now() + Duration::from_secs(20);
    while session.counters().0 < 9_437_184 {
        assert!(Instant::now() < overflow, "flood never overflowed the ring");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let mut ws = connect(&core, &tab, 0).await;
    let got = read_until(&mut ws, Duration::from_secs(20), |_| false).await;
    assert!(got.resets >= 1);
    assert_eq!(got.exit, Some(0));
}

/// Reads OUTPUT frames, acking each, until `needle` shows up. Returns when the first frame at or
/// past `baseline` arrived; leftover frames from before it are read but not timed.
async fn first_frame_of_reply(ws: &mut Ws, needle: &str, baseline: u64) -> Instant {
    let mut first: Option<Instant> = None;
    let mut bytes = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !text(&bytes).contains(needle) {
        assert!(Instant::now() < deadline, "no {needle:?} in {:?}", text(&bytes));
        let f = match tokio::time::timeout(Duration::from_millis(200), ws.next()).await {
            Err(_) => continue,
            Ok(Some(Ok(Message::Binary(f)))) => f,
            Ok(Some(Ok(_))) => continue,
            _ => panic!("socket closed while waiting for {needle:?}"),
        };
        if f[0] != 0x01 {
            continue;
        }
        let seq = u64::from_be_bytes(f[1..9].try_into().unwrap());
        if seq >= baseline {
            first.get_or_insert_with(Instant::now);
        }
        bytes.extend_from_slice(&f[9..]);
        let end = seq + (f.len() - 9) as u64;
        let mut ack = vec![0x12];
        ack.extend_from_slice(&end.to_be_bytes());
        ws.send(Message::Binary(ack.into())).await.unwrap();
    }
    first.unwrap()
}

/// Smoke test: output after an idle gap is sent at once, not held for the 8 ms coalescing window.
/// Measured inside the core: from the moment the bytes land in the session buffer to the moment
/// the first frame reaches the socket. The old code held every such frame for at least 8 ms
/// (about 15 ms on Windows, whose timer ticks at 15.6 ms), so no sample could dip under that.
/// Asserting on the minimum of several rounds keeps a busy CI runner from failing the test; the
/// regression guard proper is the `hold_until` unit test in `server/pty_ws.rs`.
#[tokio::test]
async fn idle_echo_is_not_held_for_the_coalescing_window() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["echo"]), None).await.unwrap();
    let session = core.state.session(&tab).unwrap();
    let mut ws = connect(&core, &tab, 0).await;
    let mut held = Vec::new();
    let mut unusable = 0;
    for i in 0..10 {
        // Let the stream go idle, well past one coalescing window.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let baseline = session.counters().0;
        // Watch the session buffer on a plain thread so the pty arrival is timed independently
        // of the socket. It must not wait on `session.notified()`: that would steal the
        // handler's wake-up. It reports "ready" so the input is only sent once it is spinning.
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let watched = session.clone();
        let landed = tokio::task::spawn_blocking(move || {
            ready_tx.send(()).unwrap();
            let give_up = Instant::now() + Duration::from_secs(10);
            while watched.counters().0 <= baseline {
                assert!(Instant::now() < give_up, "pty produced nothing after input");
                std::thread::yield_now();
            }
            Instant::now()
        });
        ready_rx.await.unwrap();
        let mut input = vec![0x10];
        input.extend_from_slice(format!("p{i}\r").as_bytes());
        ws.send(Message::Binary(input.into())).await.unwrap();
        let sent = first_frame_of_reply(&mut ws, &format!("got:p{i}"), baseline).await;
        let landed = landed.await.unwrap();
        // If the socket beat the watcher, the watcher was descheduled: that sample says nothing.
        match sent.checked_duration_since(landed) {
            Some(d) => held.push(d),
            None => unusable += 1,
        }
    }
    println!("held in core (pty arrival -> first frame): {held:?}, {unusable} unusable");
    let best = held
        .iter()
        .min()
        .expect("every sample was unusable; the watcher thread never ran in time");
    // Half the coalescing window: the old code could not produce any sample under 8 ms.
    assert!(
        *best < Duration::from_millis(4),
        "idle output was held for the coalescing window: {held:?}"
    );
}

/// The server must finish the WebSocket close handshake, not drop the socket after its Close frame.
///
/// The real client acks every frame and can be up to MAX_UNACKED behind. When the server dropped
/// the socket right after EXIT and Close, those late acks hit a closed socket, and Linux answers
/// that with a TCP RST, which throws away everything the client had not read yet, EXIT included.
/// The S6 run in CI failed exactly this way. This client is deliberately slow: it lets the whole
/// burst, EXIT and Close pile up unread, then reads one frame at a time, acking and pausing after
/// each, so at least one ack is sent while frames are still unread.
#[tokio::test]
async fn slow_acking_client_still_gets_exit() {
    let (core, _home) = core().await;
    // Under MAX_UNACKED, so the server never waits for an ack; over FLUSH_BYTES, so the burst
    // takes several frames and the client acks while some are still unread.
    let tab = create(&core, fake(&["flood", "81920"]), None).await.unwrap();
    let session = core.state.session(&tab).unwrap();
    let mut ws = connect(&core, &tab, 0).await;

    // Read nothing until the server can send EXIT, then give it time to send EXIT and Close
    // (and, before the fix, to drop the socket).
    let give_up = Instant::now() + Duration::from_secs(10);
    while !matches!(session.pull(session.counters().0, 1), Pull::Exit(_)) {
        assert!(Instant::now() < give_up, "the flood never exited");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut frames = 0;
    let mut exit = None;
    let deadline = Instant::now() + Duration::from_secs(10);
    while exit.is_none() {
        assert!(Instant::now() < deadline, "no EXIT after {frames} frames");
        let f = match ws.next().await {
            Some(Ok(Message::Binary(f))) => f,
            other => panic!("stream ended before EXIT, after {frames} frames: {other:?}"),
        };
        let end = match f[0] {
            0x01 => u64::from_be_bytes(f[1..9].try_into().unwrap()) + (f.len() - 9) as u64,
            0x02 => {
                exit = Some(i32::from_be_bytes(f[1..5].try_into().unwrap()));
                break;
            }
            _ => continue,
        };
        frames += 1;
        // Ack like the real client, then pause so the ack reaches the server before the next read.
        let mut ack = vec![0x12];
        ack.extend_from_slice(&end.to_be_bytes());
        ws.send(Message::Binary(ack.into()))
            .await
            .expect("ack after the server's Close frame");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(exit, Some(0));
    assert!(
        frames >= 2,
        "the burst must span several frames to exercise the race, got {frames}"
    );
}

#[tokio::test]
async fn no_resume_closes_socket() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["echo"]), None).await.unwrap();
    let mut req = format!("ws://127.0.0.1:{}/v1/pty/{tab}", core.endpoint.port)
        .into_client_request()
        .unwrap();
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("marshell.v1, token.{}", core.endpoint.token).parse().unwrap(),
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(7), async {
        loop {
            match ws.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                Some(Ok(_)) => {}
            }
        }
    })
    .await;
    assert!(closed.is_ok(), "socket stayed open without RESUME");
}

/// Tries a `/v1/pty/{tab}` upgrade with the given `Sec-WebSocket-Protocol` value and optional
/// Origin. Returns the HTTP status the server refused it with.
async fn refused_upgrade(core: &RunningCore, tab: &str, protocols: &str, origin: Option<&str>) -> u16 {
    let mut req = format!("ws://127.0.0.1:{}/v1/pty/{tab}", core.endpoint.port)
        .into_client_request()
        .unwrap();
    req.headers_mut()
        .insert("Sec-WebSocket-Protocol", protocols.parse().unwrap());
    if let Some(origin) = origin {
        req.headers_mut().insert("Origin", origin.parse().unwrap());
    }
    match tokio_tungstenite::connect_async(req).await {
        Err(tokio_tungstenite::tungstenite::Error::Http(res)) => res.status().as_u16(),
        Ok(_) => panic!("the upgrade was accepted"),
        Err(e) => panic!("unexpected error: {e}"),
    }
}

#[tokio::test]
async fn websocket_with_wrong_token_is_unauthorized() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["exit", "0"]), None).await.unwrap();
    assert_eq!(refused_upgrade(&core, &tab, "marshell.v1, token.nope", None).await, 401);
}

#[tokio::test]
async fn websocket_without_token_is_unauthorized() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["exit", "0"]), None).await.unwrap();
    assert_eq!(refused_upgrade(&core, &tab, "marshell.v1", None).await, 401);
}

#[tokio::test]
async fn websocket_from_foreign_origin_is_forbidden() {
    let home = tempfile::tempdir().unwrap();
    let core = start(CoreConfig {
        home: home.path().to_path_buf(),
        allowed_origins: vec!["http://tauri.localhost".into()],
        programs_dir: None,
        notify: None,
        focus: None,
        notify_paused: Default::default(),
        window_hidden: Default::default(),
        import: None,
    })
    .await
    .unwrap();
    let tab = create(&core, fake(&["exit", "0"]), None).await.unwrap();
    let protocols = format!("marshell.v1, token.{}", core.endpoint.token);
    assert_eq!(
        refused_upgrade(&core, &tab, &protocols, Some("https://evil.example")).await,
        403
    );
}

/// S6 throughput. Run with: cargo test -p marshell-core --release --test pty_ws -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn s6_throughput_50mb() {
    let (core, _home) = core().await;
    let tab = create(&core, fake(&["flood", "52428800"]), None).await.unwrap();
    let started = Instant::now();
    let mut ws = connect(&core, &tab, 0).await;
    let got = read_until(&mut ws, Duration::from_secs(120), |_| false).await;
    let secs = started.elapsed().as_secs_f64();
    println!(
        "S6: {} bytes in {secs:.2}s = {:.1} MB/s, exit {:?}",
        got.bytes.len(),
        got.bytes.len() as f64 / secs / 1e6,
        got.exit
    );
    assert_eq!(got.exit, Some(0));
    assert!(got.bytes.len() >= 50 * 1024 * 1024 - 1024);
}
