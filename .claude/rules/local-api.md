---
paths:
  - "crates/core/src/server/**"
---

# Local API rules

- Ask the user before changing `guard.rs`, token handling, or file permissions under the Marshell home folder. (SCOPE.md)
- Every request carries `Authorization: Bearer <token>`. New routes sit in the token-guarded router and need a "refused without the bearer" test. (PLAN.md §3; phase-5a plan)
- A WebSocket token travels as the subprotocol `marshell.v1, token.<t>`, and the upgrade must echo `marshell.v1`. (PLAN.md §3)
- Reject any `Host` header that isn't `127.0.0.1:<port>`. CORS allows only the Tauri origins. (PLAN.md §3)
- Never loosen the CSP. `connect-src` allows loopback only. (SCOPE.md; PLAN.md §3)
- `endpoint.json` is readable by the user only. Never read or copy the token from a running Marshell's `run/endpoint.json`. (PLAN.md §3; SCOPE.md)
- No route takes a file path to write. (phase-3e2 plan)
- Needs-you and permission patches skip the 100 ms batch and go out immediately, to meet the 150 ms hook-to-card budget. (PLAN.md §3, §Latency budgets)
- Never hold a `MutexGuard` across `.await` or I/O. Lock, change, drop, then await. (PLAN.md §4)
- Run blocking file reads and process sampling in `spawn_blocking`. (phase-3e2 plan)
- A new request or response type goes in `crates/protocol`, and `pnpm check:generated` must pass. (AGENTS.md)
- Tests and live checks use `MARSHELL_HOME` or a temp folder, never the real `~/.marshell`. (SCOPE.md)
