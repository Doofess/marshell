---
paths:
  - "crates/protocol/**"
---

# Protocol crate rules

- `crates/protocol` holds the serde types shared by everything, plus the brand constants. (AGENTS.md; PLAN.md §1)
- Put `#[derive(TS)] #[ts(export)]` on every exported type. (PLAN.md §1)
- Every `u64` or `i64` field gets `#[ts(type = "number")]`, because JSON sends plain numbers. (PLAN.md §1)
- A unit test fails if any generated `.ts` file contains `bigint`. (PLAN.md §1)
- Never edit `src/generated/` by hand. Run `cargo test -p marshell-protocol` to regenerate it, and commit the result. (AGENTS.md; SCOPE.md)
- Run `pnpm check:generated` after any protocol change. It fails when `src/generated/` is stale or uses `bigint`. (AGENTS.md)
- Keep one vendor's conventions out of the shared types; vendor-specific behavior stays in adapters. (AGENTS.md)
- The hook bridge depends only on `protocol` and a blocking HTTP client, with no tokio, to keep startup in milliseconds. (PLAN.md §1)
- Run `cargo fmt --all` before every Rust commit. (AGENTS.md)
