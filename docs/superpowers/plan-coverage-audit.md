# Marshell plan coverage audit

Audited 2026-10-10 on branch `phase-0-design`. Sources: `docs/BRIEF.md` (B), `docs/PLAN.md` (P), `docs/spikes/*.md`, `src/styles/tokens.css`. Citations are `file:line`. "Ph" is the PLAN section 9 phase (P:633-670); BRIEF phases are at B:328-336.

Spike state (for context): S1 only has a Windows reference row, Mac and Linux pending (`S1-webview-gate.md`); S2 passed and adds a sideloaded conpty.dll to phase 2 packaging, WSL row open (`S2-conpty.md:57-63`); S6 and S7 pass on all three OSes; S8 viable with requirements (`S8-elevated-host.md:35`). S3, S4, S5, S9, S10, S11 not yet run.

## 1. Traceability matrix

| BRIEF requirement | Delivered by | Status |
|---|---|---|
| Rename to Marshell, one constants module (B:9-14) | P:16-19, P:350 `brand.rs`, `brand.ts` | Covered |
| Icon: batons, Night shift tile, tray mono (B:15) | P:636 (done), `assets/icon` | Covered. Tauri icon generation of all platform sizes (old B:324) not scheduled |
| Phase 0 accent = signal amber (B:17) | P:262 says "the accent"; no default stated | Partial. `tokens.css` has no `--accent` at all (only comments at :18, :26) |
| Storybook mockups + published private preview, 3 sign-off batches (B:18-22) | P:80-103, P:636 | Partial. Plan has an 11-row order, not the 3 batches; "published preview" has no owner |
| pnpm, MIT, approve default on, refs off, keep history, native recap, chimes (B:24-30) | P:23-34 | Covered |
| Install Codex/Gemini before phase 6 (B:31) | P:29 | Covered, no ticket/owner |
| Gemini `hooksConfig.enabled`, no permission hook (B:33-34) | P:42-47, Ph6 P:665 | Covered |
| Gemini -> Antigravity target decision (B:35) | P:47, P:693 | Open; decision deadline "before phase 6" only |
| Codex PermissionRequest, hook trust, AGENTS.md block 32 KiB (B:37-39) | P:48-52, P:579, P:667 | Covered |
| Claude: model on SessionStart, cleanupPeriodDays, usage dedupe (B:41-43) | P:53-61, P:553, P:696 | Covered |
| Shortcuts Cmd / Ctrl+Shift, needs-you oldest first (B:45-46) | P:607-615 | Covered. Also contradicts old B:269 "newest first" (B amendment wins) |
| xterm 6 DOM fallback; store plugin dropped; config core in Ph3; elevated helper Ph6 (B:48-51) | P:62-72, P:651-665 | Covered |
| UX section binding (B:52) | P:74-341 | Covered |
| Windows/macOS/Linux first class, CI on all three (B:78) | P:584-605, P:641, P:707-712 | Covered (macOS/Linux verification blocked by hardware, see gap 6) |
| Multi-CLI via adapters, generic adapter (B:79) | P:384-405, P:665 | Covered |
| Never touch config silently, tags, uninstall zero trace (B:80) | P:555-582, P:658 | Covered |
| UX first, tokens before UI (B:81) | P:76-103, P:636 | Covered |
| Zero token cost by default (B:82) | Implicit (P:669 opt-in smart recap) | Partial. No explicit invariant or test that no model call occurs by default |
| Nothing lost, own archive (B:83) | P:499, P:553, P:656 | Covered |
| Brand colours, no logos (B:84) | P:266 | Covered |
| Tauri 2 + Rust; swappable shell; Electron fallback (B:88) | P:621, P:689 | Covered. Electron-switch work is unplanned (gap 14) |
| Protocol-first core, token, 127.0.0.1 (B:89) | P:446-482 | Covered |
| Rust crates, Tauri plugins (B:91-92) | P:372-377, P:72, P:749-764 | Covered |
| Frontend React/Vite/xterm/CodeMirror/Radix (B:93) | P:364, P:761-764 | Covered. CodeMirror, search addon, web-links not mentioned in P |
| Config only written by Rust (B:93) | P:555-582 | Covered |
| ts-rs types (B:94) | P:371-375 | Covered |
| Session persistence: relaunch with resume (B:95) | Ph2 "layout restore" P:647 | Partial. Relaunch-with-`--resume` of agents on restart is not stated; only layout restore |
| Repo layout (B:96) | P:345-369 | Covered |
| PTY manager: coalescing, backpressure, ring, OSC 7/133/9/777, BEL (B:102) | P:466-471, P:508-510, P:639, S6 | Covered |
| Hook bridge (B:103) | P:516-553 | Covered |
| Core server (B:104) | P:446-482 | Covered |
| Adapter registry (B:105) | P:384-405 | Covered |
| Config service (B:106) | P:555-582 | Covered |
| Recap worker, doctor engine, notification service (B:107) | Ph8 P:669, Ph7 P:667, Ph4 P:659 | Covered |
| Installer runner, v2 (B:107, B:220-223) | Nowhere | NOT COVERED (v2 by brief; plan silent, P has no v2 section) |
| Adapter interface (B:109-124) | P:384-405 | Covered |
| Shared event model + session state (B:126-130) | P:407-442 | Covered |
| Claude adapter details (B:132-140) | P:53-61, P:516-553 | Covered. `subagentStatusLine` tasks array unused |
| Codex / Gemini adapters (B:142-146) | P:42-52, Ph6 P:665 | Covered |
| Hookless CLI heuristics (B:146) | Ph3 "pty heuristics fallback" P:654; Limited glyph P:175 | Covered |
| 5.1 Sidebar rows, density, expanded details (B:150-158) | P:136-176 | Covered |
| Drag reorder of sessions (B:156) | `POST /sessions/reorder` P:455 | Partial: no phase lists it; Ph2 scope omits drag reorder |
| Optional grouping by project (B:156) | `layout.json groups` P:495 | Partial: no phase delivers it |
| Per-session mute; collapsible sidebar; focus mode (B:157) | P:131, P:171, `PATCH` P:455 | Covered (UX), phase unspecified for focus mode |
| 5.2 Recap tiers (B:161-164) | Event recap Ph3 P:654; smart + native Ph8 P:669; S5 P:625 | Covered |
| 5.3 Settings pane read-only (B:166-169) | Ph6 P:665 | Covered, but sits after doctor-feeding phases; no exit criterion beyond "fixtures pass" |
| 5.4 Doctor checks (B:171-176) | Ph7 P:667 | Partial. Waste checks (unused MCP/skills, token cost estimate, long descriptions, broad scope) not named in P |
| Doctor UI: groups, checkboxes, Fix N (B:177) | P:235-238, P:667 | Covered |
| Fix rules: disable never delete, snapshot, undo (B:178-179) | P:581, P:667-668 | Covered |
| 5.5 Notifications: categories, own sounds, batching, escalation, quiet hours, DND, mute (B:181-189) | P:287, P:326, P:592-594, Ph4 P:659-660 | Covered |
| Click banner jumps to session; badge; taskbar flash (B:186-187) | P:592-593, S3 P:623 | Covered |
| Fallback BEL/OSC (B:188) | Ph3 heuristics | Covered |
| Doctor disables CLI native notifications (B:189) | Ph7 P:667 "detects duplicate notifications" | Covered |
| 5.6 Instruction file tree, markdown editor, frontmatter form, token estimate, "what loads here", templates (B:193) | Ph8 P:669 | Partial. P only says "Editors + instruction library"; sub-features not itemised |
| Settings editor: form + raw, effective value + scope badge, permission/hook/env builders (B:194) | Ph8 P:669, UX row 10 P:90 | Partial: builders not itemised |
| Shared safety layer (B:195) | P:555-582 | Covered |
| "Improve this file" headless (B:196) | Nowhere | NOT COVERED |
| Instruction library: named docs, per-project toggle, agent filter, globs, wiring per CLI (B:200-201) | Ph8 P:669 | Covered at headline level |
| Templates and profiles, one-click project profile (B:202) | Nowhere beyond "Templates" | NOT COVERED (project profile bundle) |
| Budget / threshold 4k tokens warning (B:203) | Nowhere | NOT COVERED |
| 5.7 Layer 1 terminal settings (B:207) | Ph2 themes, profile pages P:644-646 | Partial. Ligatures, background opacity, paste warnings, copy-on-select, bell behaviour not itemised |
| Layer 2 shell profiles (B:208-209) | P:589, Ph2 P:644 | Covered |
| Layer 3 shell init scripts, "make permanent", execution policy read-only (B:210) | Ph2 P:645, P:504 | Covered, execution policy read-only view not mentioned |
| Elevated sessions (B:211) | S8, Ph6 P:665, P:602 | Covered. macOS/Linux elevated tab badge, bypass-confirm rule not in P |
| 5.8 Context ring, plan ring, thresholds (B:215-218) | Ph3 P:655 | Partial. Green/yellow/red thresholds (70/90) not in P; `ok`/`caution`/`error` tokens exist (`tokens.css:19-21`) |
| 5.9 Installer v2 | Nowhere | NOT COVERED (v2) |
| 5.10 History: 3 layers (B:227) | Ph3 archive P:656; recordings Ph2 P:648; event stream | Partial. Archiving Marshell's own event stream (layer c) and incremental copy not scheduled |
| Index sessions started outside app (B:228) | Ph5 P:662 | Covered |
| SQLite FTS5 index and fields (B:229) | P:506-507, S7 | Covered |
| Continuation chains (B:230) | `SessionLinked` P:411, `chains` P:507 | Covered |
| History view, conversation view, replay, files-changed tab (B:231-232) | Ph5 P:661-663 | Partial. Files-changed tab, timeline scrubber not listed |
| History actions: resume, fork, rename, tag, pin, bookmark, export, archive, delete copy (B:233) | P:462, Ph5 P:662 | Partial. Archive and delete-copy not itemised |
| Retention limits, backup mirror, disk usage, export redaction (B:235) | Ph5 P:662 redaction only | Partial. Size/age limits, backup mirror, disk usage UI NOT COVERED |
| 5.11 Project references (B:240-245) | Ph6 P:665; doctor reports tokens P:667 | Partial. `@` picker, alias learning, project map build, <100 ms resolver (P:533 gives 100 ms) not itemised |
| Approve from anywhere (B:268) | P:178-207, S9, Ph3 P:655 | Covered |
| Needs-you lane (B:269) | P:127-133, Ph3 | Covered |
| Peek (B:270) | P:455, P:510, Ph3 | Covered |
| Departures board (B:271) | P:289-296, Ph4 | Covered |
| Launcher (B:272) | P:223-228, Ph2 | Covered |
| Command palette with broadcast, saved prompts (B:273, B:293) | Palette basics Ph2, broadcast + saved prompts Ph8 P:669 | Covered |
| Clickable paths + diff drawer (B:274) | Paths Ph2, diff drawer Ph8 | Covered. "Open in editor" setting (VS Code, Cursor, Zed, Vim) not itemised |
| Attention choreography + sounds (B:275) | P:268-296, Ph4 | Covered |
| Split view (B:276) | P:130, Ph2 | Covered |
| Escape hatches: open in system terminal, copy resume command (B:277) | Nowhere | NOT COVERED (only red header outline P:173 covered) |
| Airport mode (relabel statuses) (B:279) | Nowhere | NOT COVERED (optional, default off) |
| Repo scripts `marshell.json` (B:287) | Ph6 P:665 | Covered |
| Custom agents (B:288) | `agents.json` P:495, Ph6 | Covered |
| `marshell` CLI (new/list/focus/send) + built-in skill (B:289) | P:355, Ph6 | Covered |
| Port chips (B:290) | `ports/` P:352, Ph6 | Covered |
| Stuck detection (B:291) | Ph4 P:659, P:171 | Covered |
| Step timeline (B:292) | Ph8 P:669 | Covered |
| v2 borrowings (review to merge, issue start, checkpoints, file tree, phone check-in, >2 panes, daemon) (B:295-301, B:336) | Nowhere | NOT COVERED (v2 section absent from P) |
| Terminal essentials: scrollback search, OSC 133 jump, links, copy on select, bracketed paste, ligatures, zoom, smooth scroll, cursor opts (B:305) | Ph2 exit "OSC 133 jumps" P:650; search shortcut P:609 | Partial. Per-tab zoom, smooth scroll toggle, 10k-line scrollback (P:509 ok) |
| Theme import (B:306) | Ph2 P:646 | Covered |
| High-contrast mode (B:306) | Ph2 P:646 | Covered |
| F2 rename, drag, reopen closed, close with confirmation (B:307) | P:240-242, Ph2 P:647 | Covered |
| Shortcut cheat sheet (B:308) | P:610, Ph2 | Covered |
| IA: sidebar / header / drawer / settings page nav (B:312-315) | P:117-133 | Partial. Settings left-nav sections (General..Advanced) not in P |
| Visual system numbers, 13 px, 36/48 rows (B:316) | P:136-146, P:246-252, `tokens.css:36-61` | Covered |
| Performance budget + hidden perf panel (B:317) | P:331-341, P:685 | Covered |
| Accessibility: live region, focus rings, large text, xterm accessibility mode (B:318) | Live region Ph3 P:655; large text Ph2 P:646; axe P:681 | Partial. xterm accessibility mode, screen-reader pass on terminal not scheduled |
| Mockups before UI code (B:320) | Ph0 | Covered |
| Brand/icon deliverables incl. Tauri generator (B:322-324) | P:636 | Partial (see icon row) |
| Light / Dark / System themes, true black (B:262) | P:258-261 | Covered. Contradicted by hex values (section 4) |
| 8 accents + custom hex, OS accent default (B:263) | P:82 "8 accents" | Partial. Custom hex and "default to OS accent" contradict B:17 (default amber) |
| Platform fit title bars (B:264) | P:119-123, P:596 | Covered |
| Section 8 deliverables 1-11 (B:340-350) | P:343-712 | Covered. Spike list S3, S5 etc. covered P:617-631 |
| Testing: fixtures, component, one e2e per OS (B:349) | P:672-685 | Covered |
| Trademark / name check (B:350) | P:15-18, P:698 | Partial. Formal search deferred to "before first public release", no owner |
| CI builds + tests all OSes (B:78) | P:368, P:641 | Covered (S6/S7 CI runs exist) |

## 2. Uncovered end-to-end needs

Neither doc covers these (or only mentions them in passing).

| Need | Where it falls through |
|---|---|
| Release pipeline: tagged build, artifacts, checksums, release notes, GitHub Releases | P:600-601 lists packagers/signing only; no job in P:368 `ci.yml` or any phase |
| Auto-updater, update channel, signature keys, rollback | Absent. B:223 "update available" refers to CLI updates, not the app. Tauri updater unmentioned |
| Installer/uninstaller UX: per-user NSIS leaves `~/.marshell`; uninstall must run the hook removal first | P:582 covers `marshell uninstall` command but not the OS uninstaller path (add/remove programs, dmg drag-to-trash, deb remove). Uninstall = "zero trace" (B:80) has no test beyond P:658 |
| Code-signing procurement and CI secrets (Azure Artifact Signing, Apple Developer ID, notarization) | P:601, P:695 mention; no phase or owner; Apple Account Holder dependency is only a risk bullet |
| Crash recovery: core thread panic, WebView crash, app killed with live ptys, orphan child processes (S2 finding 4 says the Job Object only lands in Ph2) | P:513 concurrency rule only. No panic hook, no watchdog, no "core restarted" UI state (P:109 "offline core" appears only as a state checklist item) |
| Data corruption recovery: `layout.json`, `settings.json`, `index.sqlite` bad or half-written | P:566 atomic writes for CLI config only; app's own files and SQLite WAL/integrity-check/rebuild not covered |
| Logging and diagnostics: `logs/` dir exists P:503, bridge.log P:547; no log levels, rotation, size cap, redaction, or "export diagnostics bundle" | |
| Settings/data schema migration: `settings.json`, `layout.json`, `managed.json`, SQLite schema, endpoint.json `v1` have no version field or migration story | P:446 `/v1` only for API |
| Managed-entry migration on bridge update: hook command strings contain `--tag mshl1`; what happens on `mshl2`, or when the home dir changes | P:518, P:574 |
| Data export/backup of Marshell's own state (settings, profiles, instruction library) | B:235 backup mirror covers the archive only, and is not in P phases |
| Telemetry and crash-report stance (none? opt-in?) and privacy statement | Absent; matters because the app reads transcripts and archives secrets |
| Secrets handling inside the archive: transcripts and recordings contain keys; storage permissions on `~/.marshell/history`, encryption at rest, redaction only on export (B:235) | P:449 sets 0600 only for endpoint.json |
| Security model document: threat model for loopback server (any local process, other users on multi-user machine, browser DNS rebinding covered P:472-476), `marshell` CLI auth, `/sessions/:id/input` as a keystroke injection primitive, instruction-library prompt injection | Pieces only. No single model; no review gate in a phase |
| Supply chain: dependency audit (cargo-deny, pnpm audit), licence check of bundled fonts (Inter, JetBrains Mono), sounds, conpty.dll/OpenConsole MIT notice (S2) | Only S2 mentions the MIT notice |
| Accessibility beyond Storybook axe: screen-reader walkthrough, terminal a11y mode, large-text, keyboard-only flow | Only P:98 and P:113 |
| i18n / localisation: string externalisation, RTL, non-US keyboard layouts beyond dead-key spikes | Absent; P:299-317 copy rules are English-only. Decide "English only, externalised strings" or defer explicitly |
| Docs/help: README, user guide, first-run help, in-app "learn more", contributing guide for an MIT open-source repo, CODE_OF_CONDUCT, issue/PR templates (PR template is mentioned P:103) | P:703 only moves BRIEF |
| Multi-window: second Marshell window, detaching a tab, multi-monitor, single-instance argv routing | P:481 single-instance event; no spec for a second window. Elevated fallback "separate elevated window" P:665 is the only second window |
| Session persistence across app restarts: what happens to live ptys (they die in v1), what the restored tab shows, relaunch with `--resume` vs. "Ended" state, un-hooked sessions | B:95 and P:647 "layout restore"; no flow designed or exit criterion beyond layout |
| Offline core: the app has no network need, but model-backed recap, Antigravity auth, updater, and `marshell` CLI discovery when the app is closed are unspecified | P:524 bridge exits 0 when app closed; no CLI-side message for `marshell new` when app is not running (does it launch the app?) |
| Performance budgets beyond latency: memory per session, CPU idle, scrollback memory (10k lines x N sessions x vt100 parser), disk growth from recordings, startup with 1000 sessions | P:331-341 latency only; P:663 index search only |
| Accessibility of notifications and sounds (visual alternative to sound, volume floor) | P:281-287 |
| OS integration: autostart at login, default-terminal registration, URL scheme `marshell://` registration and its security (P:593), file association, shell context menu "Open in Marshell" | P:593 deep link only |
| Windows-specific packaging: WebView2 bootstrapper (P:595), sideloaded conpty.dll shipping (S2:62), arm64 | S2 says "phase 2 packaging", P:647-650 Ph2 scope does not list it |
| Process-level hygiene: PATH resolution refresh when user installs a CLI later, `detect()` cache invalidation | P:590 once at start |
| CLI version-compat policy: refuse/degrade when hook format changes; "unsupported CLI version" state | P:690 mitigations only; no UI state or doctor check named |
| Dev/QA: demo mode (P:94 mentions) has no owning task or phase; Playwright baseline updates | P:94 |
| Branch/commit conventions, ADR process (`docs/adr` is empty) | Matches CLAUDE.md rules but not planned |

## 3. Homeless work (mentioned without an owning phase or exit criterion)

| Item | Mention | Problem |
|---|---|---|
| Demo mode | P:94 | No phase, no flag, no exit criterion |
| `fake-agent` crate | P:356, P:680 | Phase 1 scope (P:639) omits it; Phase 0 stories "fed by fake-agent" (P:94) but Ph0 precedes it |
| Sideloaded `conpty.dll` + OpenConsole, MIT notice | S2:60-63 "phase 2" | P:644-650 Ph2 scope and exit omit it |
| Job Object `KILL_ON_JOB_CLOSE` | S2:55 "phase 2 when tabs become closable" | P:588 says Job Object as if done; Ph2 scope omits; no exit test for crash orphans |
| Drag reorder, grouping by project | P:455, P:495 | No phase scope |
| Focus mode, rail, split-view thresholds | P:124-133 | Ph2 mentions split only |
| Linux/mac verification of S1 (hardware pending) | `S1-webview-gate.md` table | Ph1 exit "the gate passes" cannot be met without machines; no CI substitute |
| S3, S4, S5, S9, S10, S11 | P:617-631 | Only S3 is named in a phase (Ph4); S4/S10 in Ph3; S5 Ph8; S9 Ph3; S11 Ph6. None has a "run by" date and S3 (notification click) gates the Ph3 badge/sound |
| Gemini vs Antigravity decision | P:47, P:693 | No ticket, no date; blocks Ph6 scope |
| Install Codex and Gemini | P:29 | No ticket; blocks Ph6 fixtures |
| Trademark search | P:698 | "before first public release", no phase |
| GitHub repo, MIT file, issue templates, PR template | P:103, P:704 | "First actions" only; PR template content (10 items) has no file task |
| `/setup-matt-pocock-skills`, `to-spec`/`to-tickets` | P:704 | Process, not phase; fine but unlinked to ticket creation for phases 1-8 |
| Frontend `src/styles/tokens.css` vs `src/tokens/tokens.css` | P:365 vs repo | Plan says `src/tokens/`, repo has `src/styles/tokens.css` |
| Perf panel (hidden) | P:685, B:317 | No phase |
| Storybook test-runner with axe | P:681 | Phase 0 or 1 setup task not assigned |
| Playwright screenshot baseline | P:101 | Needs batch 1 approved; no task |
| Doctor "raise `cleanupPeriodDays`" with consent | P:696 | Ph7 check list (P:667) omits it |
| Codex hook trust check in doctor + onboarding | P:579, P:667 | OK in Ph7 but onboarding (Ph3) Codex step before Ph6 adapter exists |
| `marshell uninstall` and Settings -> Advanced | P:582 | Not in any phase scope; Ph3 exit tests only "zero byte diff" |
| Hook bridge self-update / locked exe rename dance | P:522 | Ph3 implied, not listed |
| WSL bridge (S11) | P:591 | Ph6; but S2 WSL row open and the Windows primary dev OS uses WSL |
| Tray menu | P:358, P:605 | No phase scope or design in Ph0 table |
| OS notification mocks + icon story | P:92 | Last Ph0 row; sign-off batch 3 |
| Sounds: authoring/licensing of the "original" chime set | P:281-287 "Sound brief" | Ph0 says "sound drafts" (P:94); no asset-production task or licence note |
| `docs/adr/` and `GLOSSARY.md` | repo | Empty, but user rules require domain-modeling usage |
| Release checklist (manual macOS run) | P:684 | "manual checklist per release" with no checklist file or owner |

## 4. Contradictions between BRIEF amendments / repo state and PLAN

| # | Where | Contradiction |
|---|---|---|
| C1 | P:256-264 colour table | Hex values and `#FFD60A` / `#B25000` caution. `tokens.css:5-21` is oklch; caution is now yellow (dark, `oklch(90% 0.17 100)`) and olive (light, `oklch(52% 0.11 100)`) specifically to avoid amber (`tokens.css:18`). `#FF453A` / `#D70015` / `#30D158` / `#248A3D` etc. all superseded |
| C2 | P:262 "attention: the accent" | B:17 sets default accent to signal amber. P never names the default; B:263 (body) still says "Default to the OS accent". Amendment wins; PLAN and `tokens.css` have no `--accent` or `--attention` token |
| C3 | P:266 "Claude orange is never a status colour" | `tokens.css:26` Claude brand is "muted terracotta" `oklch(56% 0.08 45)`; B:152 body still says "Claude orange". Plan table never lists brand tokens (codex/gemini/generic: `tokens.css:28-30`); B:152 "OpenAI light yellow" vs `--brand-codex` near-neutral grey/white |
| C4 | P:365 | Tokens at `src/tokens/tokens.css`; repo uses `src/styles/tokens.css` |
| C5 | B:18-22 vs P:76-103 | Amendment: three sign-off batches (tokens/glyphs/rows; approve card/main/launcher; onboarding/departures/doctor/settings/notifications). P: 11 sequential deliverables, "approved" gate once (P:96). Rows 1-3 / 4-6 / 7-11 map cleanly but P:6 does not say so, and P:636 exit is a single "your approval" |
| C6 | B:18 "published private preview" | P has no mention of publishing; P:94 only "Storybook stories" |
| C7 | P:3-5 | Context says brief at `C:\dev\Airport\BRIEF.md`, empty folder, "no git repo" (P:12); repo is now at `docs/BRIEF.md` and phase 1 spikes already ran (S1,S2,S6,S7,S8). P:703 first actions are done |
| C8 | P:588 and P:626 | P:588 states Job Object kills the process tree as a Phase-1 given; S2:55 says phase 1 uses `taskkill /T /F` and Job Object lands in phase 2 |
| C9 | P:588 vs S2:57-62 | P says "optional sideloaded conpty.dll if spike shows stock bugs"; S2 decides to ship it for 4x throughput (not a bug). Plan wording is now wrong and Ph2 scope lacks the task |
| C10 | P:631 / P:11 vs `S1-webview-gate.md` | P:621 pass criterion c: "typing stays under 30 ms"; S1 Windows result: Git Bash needed `ef659b0` fix, `head -n 2M` took 2 min 40 s (S1:~line 20). Floods of short lines are slow on inbox ConPTY; P:621 budget not restated per-shell |
| C11 | P:628 vs `S8-elevated-host.md:35` | P: Ph1 spike "only"; S8 decision is viable-for-phase-6, with token handed on cmdline flagged unsafe (S8:24) and wrong-token / decline paths unrun. P:665 "if S8 passed" now answered, needs requirements section (production must pass token via pipe) |
| C12 | P:269 body of B vs P:615 | B:269 "newest first" vs P:615 oldest first. Amendment (B:46) wins; B body not updated |
| C13 | B:68-69, B:96, B:350 body | Still says Airport / `airport-hook` / `~/.airport`. Amendment covers it (B:9-14), but body is not rewritten; P:703 promised to update BRIEF |
| C14 | B:93 / B:92 body | "tauri-plugin-store" (B:92) and "canvas renderer fallback" (B:93) remain in body; amendments B:48-49 override |
| C15 | B:145 body | `hooks.enabled`; amendment says `hooksConfig.enabled` |
| C16 | B:336 body "phase 9 v2" vs P:633 | P: "phases 1-8 = v1". B:336 lists v2 but P has no v2 section (C: no coverage) |
| C17 | B:328 vs P:638-641 | B Ph1 includes "design tokens, empty layout shell"; P Ph1 explicitly "no chrome until phase 0 is approved". Consistent, but B:328 body is stale |
| C18 | B:330 vs P:651-657 | B Ph3 lists "transcript capture and history index with Resume" and approve etc.; P adds config core, onboarding, badge and sound. P:657 pulls forward; P:659 Ph4 still says "notifications + choreography", consistent |
| C19 | B:263 "~8 accents plus custom hex" vs P:82 | P tokens deliverable covers "8 accents", custom-hex AA validation not mentioned; custom hex can never be pre-validated |
| C20 | P:174-175 vs `tokens.css:18-21` | Glyph table uses ⚠ caution always visible; caution token is now yellow/olive and must be the only non-status-colour; "attention tint 6%" (P:165) uses accent amber and caution yellow is a hue neighbour (`tokens.css:18` claims distinct, but no matrix row for amber vs caution in P) |

## 5. Top 15 gaps, ranked by impact

1. **No release/update pipeline (build, sign, notarize, publish, auto-update).** Without it nothing ships. Fix: add a release workflow (tagged build, signed artifacts, checksums, Tauri updater with signing key, rollback) and own it in a new Ph8.5 "Release" before first public build; start secret procurement in Ph3. Owner: new release phase, secrets in Ph3.
2. **App crash/recovery and data-integrity story is missing** (core panic, WebView crash, orphaned ptys, bad `layout.json`/`settings.json`/SQLite). Fix: panic hook plus core supervisor, Job Object with kill-on-close, atomic writes + `.bak` + schema version for all `~/.marshell` files, SQLite `integrity_check` with rebuild-from-archive. Owner: Ph2 (Job Object, layout) and Ph3 (core state).
3. **Session persistence across restart is undesigned.** Fix: specify restore flow (relaunch `--resume`, "Ended" state for non-resumable, limited-status for hookless) with an exit criterion "kill app mid-task, relaunch, all tabs back". Owner: Ph2 (layout) plus Ph3 (Claude resume).
4. **Security model and secrets-at-rest.** Archive and recordings hold keys; loopback input injection route; `marshell://` deep link. Fix: write `docs/adr` threat model, file ACLs for all of `~/.marshell`, optional at-rest encryption decision, authenticated deep link, security review gate before Ph3 ships approve-from-anywhere. Owner: Ph3.
5. **Uninstall from the OS and "zero trace" beyond the CLI.** `marshell uninstall` exists (P:582) but OS uninstallers (NSIS, dmg, deb) don't run it. Fix: NSIS uninstall hook that runs unhook, and a doctor "orphaned hooks" check that fires when `marshell-hook` is missing; test in CI. Owner: Ph3 (hook plan) plus Release phase.
6. **Cross-platform verification has no hardware or CI substitute.** S1 Mac and Linux rows pending, S3/S9/S10/S11 unrun, the Electron fallback is unplanned. Fix: add macOS and Linux (xvfb + WebKitGTK) smoke jobs to CI, acquire/rent machines, set a date for the S1 gate, and write the Electron-shell cost estimate. Owner: Ph1.
7. **Hook/CLI version drift has no degrade path.** Claude's transcript is "internal" (P:60). Fix: version-gated adapters, "unsupported CLI version" state, a doctor check, fixture matrix in CI, kill-switch to disable normalisation per CLI. Owner: Ph3 and Ph7.
8. **Logging, diagnostics and support bundle.** Only `bridge.log`. Fix: tracing with rotation, redaction, "Export diagnostics" in Settings -> Advanced, and the perf panel. Owner: Ph3.
9. **Telemetry/privacy stance and privacy copy.** Fix: ADR "no telemetry, no network except user-initiated" plus onboarding line; revisit if crash reporting added. Owner: Ph0/Ph3 onboarding.
10. **Contradictions C1-C3 (colour table, accent default, Claude brand) will mislead implementers.** Fix: replace P:254-266 with a pointer to `tokens.css`, state default accent = signal amber, add `--accent` token and amber/caution/Claude distinctness rows to the AA matrix. Owner: Ph0 batch 1.
11. **Retention, backup mirror, disk usage, and recording growth unowned.** Fix: put limits, backup mirror, disk-usage UI and compression/GC into Ph5 scope and exit; add a budget (MB per hour of recording). Owner: Ph5.
12. **Docs/help, community files, and i18n decision.** Fix: README, CONTRIBUTING, SECURITY.md, issue/PR templates (the 10-item checklist) in Ph1; "English-only with externalised strings" ADR; in-app help links from onboarding. Owner: Ph1.
13. **Instruction library details missing** ("Improve this file", project profiles, 4k budget warning, templates, per-project toggle). Fix: itemise Ph8 scope and exit criteria beyond "one doc wired into all three CLIs" (P:670). Owner: Ph8.
14. **Homeless packaging and platform tasks**: sideloaded conpty.dll + notice, Job Object, WebView2 bootstrapper, arm64, demo mode, fake-agent timing, tray menu. Fix: add to Ph2/Ph1 scope and exit lines; move `fake-agent` into Ph1 (needed before Ph0 stories claim they use it). Owner: Ph1/Ph2.
15. **BRIEF body still says Airport/`hooks.enabled`/canvas/newest-first, and v2 has no plan section.** Fix: rewrite BRIEF body per P:703 (or mark body stale) and add a short "v2 and later" section in PLAN with the installer, daemon, review-to-merge, and `@` picker so "NOT COVERED" items become explicit deferrals. Owner: Ph0 housekeeping.
