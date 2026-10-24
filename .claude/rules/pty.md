---
paths:
  - "crates/core/src/pty/**"
---

# Pty rules

- Pty and ConPTY code is high-effort work. Give a worker `effort: high`. (CLAUDE.md, Subagents)
- portable-pty 0.9 sets `PSEUDOCONSOLE_INHERIT_CURSOR`. ConPTY holds back output until `ESC[6n` is answered, so the core answers it until the renderer attaches. The attach offset (`Session::attach`) is also where the renderer starts answering: the `LIVE` frame carries it, and the replay is capped there. (PLAN.md §Corrections 5; §9, slice 6a2)
- A session's children live in a kill-on-close Job Object, so killing a session or a crash leaves no orphans. (PLAN.md §Corrections 5; §7)
- Treat Windows exit code 259 as "still running". (PLAN.md §7)
- Each session keeps a byte ring buffer of at least 10k lines for replay, and a `vt100::Parser` for the screen state. (PLAN.md §4)
- `seq` on the pty WebSocket is the byte offset of the stream. The unacknowledged limit is 1 MiB. (PLAN.md §3)
- If a resume position is no longer in the ring buffer, send a reset rendered from the `vt100::Parser`, not a mid-stream replay. (PLAN.md §3)
- Run blocking portable-pty reads on a plain `std::thread` per session, forwarding over a tokio channel. (PLAN.md §4)
- Never hold a `MutexGuard` across `.await` or I/O. Prefer a few small mutexes. (PLAN.md §4)
- Don't kill processes by name. Stop only what you started, by PID. (SCOPE.md)
- Don't name a test binary after a real CLI. Use a name like `codex-marshell-test.exe` and run it by full path. (SCOPE.md)
- Marshell never presses Enter in a terminal, and never sends input to a session the user is running. (SCOPE.md; CLAUDE.md, Live check)
