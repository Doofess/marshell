# Antigravity CLI Integration: Overview

## The Shift from Gemini to Antigravity
Google has deprecated the standalone Gemini CLI and replaced it with the **Antigravity CLI** (`agy`) for free-tier and Google One users (effective 2026-06-18). Marshell must pivot its "Gemini" support to Antigravity CLI. 

This is a massive win for Marshell because, unlike the Gemini CLI, the Antigravity CLI **fully supports synchronous permission-granting hooks**, meaning the "approve-from-anywhere" functionality works out of the box!

## Core Responsibilities of the Adapter (`crates/core/src/adapters/antigravity.rs`)
Claude, when implementing this adapter, you must ensure it can:
1. **Detect** the `agy` binary in the user's `PATH`.
2. **Launch** `agy` non-interactively or interactively depending on the session context.
3. **Install & Verify Hooks** in `~/.gemini/antigravity-cli/settings.json` or `.agents/hooks.json`.
4. **Translate Hook Events** via `marshell-hook`, mapping `PreToolUse` payloads into Marshell's `PermissionRequested` events.
5. **Respond to Hooks** by providing `{ "decision": "allow" | "deny" | "ask" }` on stdout.
6. **Parse History** from `~/.gemini/antigravity-cli/transcript.jsonl`.

Please read the subsequent documents in this directory for the exact API contracts and schemas you must implement.
