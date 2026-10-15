---
description: General instructions for AI agents working on the Marshell project.
trigger: always_on
---

# Marshell Development Guidelines

This project (Marshell) is a desktop terminal session manager for AI coding-agent CLIs.

When working on this project, please adhere strictly to the guidelines defined in `docs/PLAN.md` and `docs/BRIEF.md`.

## Antigravity CLI Integration

Google has replaced the Gemini CLI with the **Antigravity CLI** (`agy`) for free-tier and Google One users. 
Marshell must include a first-class adapter for the Antigravity CLI.

**Action Required:**
Before modifying the Rust core or implementing the Antigravity adapter in `crates/core/src/adapters/antigravity.rs`, you MUST read the detailed Antigravity integration guide located at:
`docs/research/antigravity-notes/01-overview.md` (unverified research; check `docs/research/` for the confirmed install facts)

That document explains the specific hook payloads, settings file locations, and permission structures that the Antigravity CLI uses, which differ significantly from the old Gemini CLI. Marshell's "approve-from-anywhere" functionality relies on correctly implementing these hooks.

## Modifying the App
- This is a Tauri 2 + Rust + React/TypeScript (Vite) app.
- Follow the design principles in `docs/PLAN.md` (no boxed outlines, status never by color alone, native keyboard shortcuts, etc).
- Use `pnpm` for package management.
- Update `docs/PLAN.md` if any architectural decisions change.
