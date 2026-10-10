# Marshell ideas

A scratch file. Throw anything in under **Inbox**, as rough as you like: one line, a fragment, a pasted link. Nothing here is a commitment and nothing needs to be tidy. Later we go through it together and move each item somewhere (or delete it).

How to add one: put it at the top of Inbox, newest first. A date is optional.

```
- 2026-10-10 the idea, in your own words
```

## Inbox

- **User-configurable navigation modifier, with an onboarding hint.** The chord for moving the selection (Ctrl+Shift by default, see PLAN "Keyboard navigation and shortcuts") is a setting that lists each choice with where it clashes; the first run shows a three-second hint ("Hold Ctrl+Shift, press Tab") and the empty selection keeps teaching it. Later: a visible "hold" indicator in the status bar while the chord is down.
<!-- new ideas go here, newest first -->

- 2026-10-10 **Detect the foreground program of each terminal and show its mark.** A session is not always an AI agent: Marshell reads the foreground process of the terminal (its executable name) and picks a mark from a table (shells, dev tools, remote and cloud tools, then a neutral fallback that shows the executable's name). When the foreground command changes (PowerShell, then ssh, then back) the mark changes with it, and a command that leaves behind a known agent (running `claude` inside PowerShell) is promoted to that agent's own row. Open: how often to poll, wrappers and aliases (`npx`, `winpty`), and sourcing the real logos with their licences.

- 2026-10-10 **A friendly way to change each CLI's settings (not just view them).** The status line, hooks, permissions, model and so on are all plain settings in the CLI's own `settings.json` (or `config.toml`, etc.), so instead of hand-editing JSON the app offers forms: pick a status line from a list (or paste a command), toggle a hook, switch a model. This is where status lines naturally live, since they are a setting rather than a mod or plugin. Already in the plan: the Settings page and editor in `docs/PLAN.md` (form mode, raw mode, scope badges), the read-only settings pane in phase 6 and the editors in phase 8, all built on the config safety layer (preview the exact change as a diff, back up first, one-click undo, never touch managed or organization settings). Per vendor, since each CLI stores settings differently (see `docs/research/2026-10-10-cli-data-sources.md`).

- 2026-10-10 **Choose the terminal font in Settings.** A setting to pick the font used in the terminals. Related plan items (`docs/PLAN.md`, "Visual system"): by default Marshell imports the font already set in Windows Terminal or iTerm so Nerd Font prompts keep working, otherwise it bundles JetBrains Mono at 13 px. Open: font size and line height next to it, a live preview in the picker, and whether the font can differ per terminal or per vendor.

- 2026-10-10 **Link hint bubble in terminals.** In a PowerShell terminal you have to hold Ctrl and click a link to open it, and nothing tells you that. When the mouse hovers a link (or a file path) in any Marshell terminal, a small bubble pops out next to it saying how to open it: "Ctrl+click to open" (Cmd+click on macOS). Notes for later: show it after a short hover delay so it does not flicker while the mouse crosses text, never steal focus, hide on leave or on any key, respect reduced motion, and maybe stop showing it once the user has opened a few links. xterm.js link providers already report hover and leave, and the plan has clickable paths in phase 2 (`docs/PLAN.md`), so this fits there.

- 2026-10-10 **Goal: Marshell solves lost session history entirely.** I want to manage a session's conversation history easily and keep records as far back as I choose (no silent expiry). I want to jump to any session without hunting for exactly where to resume the CLI the way I would in plain PowerShell: pick it and be in it, with its conversation. I have lost session history that way before, and Marshell should make that impossible. Already in the plan and worth tying to this goal: the durable archive copy of every transcript (Claude deletes transcripts after 30 days by default, `cleanupPeriodDays`), the history index and Resume (phase 3), and the History view with search (phase 5). Open: how long "as far as I want" is (a setting per vendor or global), what happens when a vendor's transcript format changes, and one-click "resume here" from any past conversation.

- 2026-10-10 **A Marshell plugin for each CLI.** Users install it into Claude Code, Codex, Antigravity and the others, and it lets a session use Marshell: sessions talk to each other (send a message or a result to another running session), and a session can ask Marshell to open more sessions when it needs them (for example a helper in another project, or a reviewer session). Related to the planned `marshell` CLI and its built-in skill in phase 6 (`docs/PLAN.md`), which could be the way the plugin talks to the app. Status: the addressing it needs is designed (stable session numbers, `marshell send 6 ...`; see `docs/PLAN.md`, Session numbers and manual CLIs); the plugin itself is still an idea.

- 2026-10-10 **Per-vendor panels, each its own menu, all scoped to the vendor of the focused session** (every vendor has different things, so each panel shows only what that CLI really has):
  - **Account and plan:** the connected CLI's vendor data: who is signed in and how (auth method), how long until it needs to sign in again, which subscription plan, and usage stats.
  - **MCP servers:** their own menu (what is connected, its state, which tools it offers).
  - **Skills, plugins and mods:** one menu each, per vendor (a vendor with no plugins simply has no plugins menu).
  - **Help:** the vendor's own `/` commands, listed as a searchable help menu, so you do not have to remember them per CLI.

- 2026-10-10 **Plan coach: learn from each week of usage.** The app records activity per vendor, week by week, and helps the user get more out of their AI subscription. Example: I worked the whole week on one project and finished the week with only 60% of the weekly allowance used, so I could have used stronger models or higher effort throughout. The app learns that and applies it the next week. It looks at many parameters (model, effort, time of day, project, session length, context size, how close each week ends to the limit, resets) and gets better every week.

## Parked

Looked at, liked, not now. Say why in a few words so future us remembers.

## Planned

Promoted to `docs/PLAN.md` or a ticket. Leave a pointer.

- **Catch accessibility problems early** (2026-10-10): shared Announcer and SelectedMark blocks, lint-style rules, a forced-colours/zoom/colour-vision matrix and a screen-reader script. Done for phase 0, see `docs/PLAN.md` "Accessibility safety net" and `docs/a11y/`. Later: run the matrix in CI once Playwright is a dev dependency, and build the `everything` and `on-request` announce modes.

## Dropped

Decided against. Keep the reason; it saves re-arguing.

---

## Left over from building the design (mine, not yours: keep, move or delete)

- A "Jump to latest" pill in the terminal when scrolled up, since the main window shows no scrollbar to say where you are in the conversation.
- Suggest collapsing the sidebar to the rail when four panes would each be under 80 columns (a hint, never automatic).
- A setting to turn "accent follows the vendor" off and use one fixed accent everywhere.
- DeepSeek has no first-party CLI, so it stays out of the launcher until we decide before phase 6.
- Rewrite the stale body of `docs/BRIEF.md` (the Airport name, `hooks.enabled`, the canvas fallback, the newest-first lane).
