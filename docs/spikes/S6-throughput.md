# S6: WebSocket throughput with coalescing and backpressure

Pass: 50 MB through the full pipeline in under 2 s end to end (pty -> core -> WS). Memory stays bounded
(unacked ≤ 1 MiB + 64 KiB, proven by `backpressure_bounds_unacked_output`). Typing in another tab stays under 30 ms.

| OS | `s6_throughput_50mb` output | Pass |
|---|---|---|
| Windows (dev box) | `S6: 58981347 bytes in 1.63s = 36.1 MB/s, exit Some(0)` (median of 3; raw ConPTY 1.58 s, pipeline +3%) | PASS |
| Windows (CI) | pipeline `58980257 bytes in 3.57s = 16.5 MB/s`; raw `3.70s = 15.9 MB/s` ([run](https://github.com/Doofess/marshell/actions/runs/37999636586)) | Pass by the 20% rule: misses 2 s, but the pipeline adds nothing over raw ConPTY on the shared runner |
| macOS (CI) | pipeline `53084960 bytes in 1.72s = 30.9 MB/s`; raw `1.72s` ([run](https://github.com/Doofess/marshell/actions/runs/37999636586)) | Pass |
| Linux (CI) | pipeline `53084160 bytes in 1.03s = 51.8 MB/s`; raw `1.12s = 47.3 MB/s` ([run](https://github.com/Doofess/marshell/actions/runs/37999636586)) | Pass |

If Windows misses 2 s, the likely ceiling is ConPTY itself. Measure a raw ConPTY read of the same flood (a
`Session::pull` loop with no WebSocket) and record both numbers. Our pipeline passes if it adds under 20%.

## Measurements

Windows 11 dev box, release build, 2026-10-09. Byte counts are above 50 MiB (52,428,800) because ConPTY
re-encodes the flood (`\n` -> `\r\n` plus cursor and attribute sequences). The two tests:

- `s6_throughput_50mb` (`crates/core/tests/pty_ws.rs`): full pipeline, pty -> core -> WebSocket -> client that acks every frame.
- `s6_raw_conpty_50mb` (`crates/core/tests/session.rs`): the same flood through `Session::pull` with immediate acks, no WebSocket.

Run with `cargo test -p marshell-core --release --test <pty_ws|session> -- --ignored --nocapture s6_`.

| Flood write pattern | Raw ConPTY (`Session::pull`) | Full pipeline (WebSocket) | Pipeline overhead |
|---|---|---|---|
| Before: one console write per 80-byte line (`StdoutLock` is a `LineWriter`, ~655k writes) | 31.77 s = 1.9 MB/s (1 run) | 41.98 s = 1.4 MB/s (1 run) | +32% |
| After: `write_all` of whole 64 KiB chunks (819 lines each) | 1.56 / 1.58 / 1.67 s, median 1.58 s = 37.4 MB/s | 1.63 / 1.63 / 1.74 s, median 1.63 s = 36.1 MB/s | +3% (median), +4% (mean) |

Finding: the original 20-40 s measured `fake-agent`'s syscall pattern, not our pipeline. From the raw run,
one console write into ConPTY costs about 48 µs (31.77 s / 655k writes) whatever its size, so 655k one-line
writes dominated everything. With chunked writes
the same bytes clear the whole pipeline in about 1.6 s, with the WebSocket hop adding ~50 ms over a raw
`Session::pull` loop. The core's reader buffer (16 KiB) was left as is; it is not on the critical path at these numbers.

The "before" rows are single runs taken while the box had its usual background load; they are an order of
magnitude off, so run-to-run noise does not change the conclusion.

## Pending

- CI rows come from CI's two S6 steps, which run both ignored tests by name.
- "Typing in another tab stays under 30 ms" waits for phase 2 (multiple tabs). Not measured here.
