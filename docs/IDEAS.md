# Marshell ideas

A scratch file. Throw anything in under **Inbox**, as rough as you like: one line, a fragment, a pasted link. Nothing here is a commitment and nothing needs to be tidy. Later we go through it together and move each item somewhere (or delete it).

How to add one: put it at the top of Inbox, newest first. A date is optional.

```
- 2026-10-10 the idea, in your own words
```

## Inbox

<!-- new ideas go here, newest first -->

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

## Dropped

Decided against. Keep the reason; it saves re-arguing.

---

## Left over from building the design (mine, not yours: keep, move or delete)

- A "Jump to latest" pill in the terminal when scrolled up, since the main window shows no scrollbar to say where you are in the conversation.
- Suggest collapsing the sidebar to the rail when four panes would each be under 80 columns (a hint, never automatic).
- A setting to turn "accent follows the vendor" off and use one fixed accent everywhere.
- DeepSeek has no first-party CLI, so it stays out of the launcher until we decide before phase 6.
- Rewrite the stale body of `docs/BRIEF.md` (the Airport name, `hooks.enabled`, the canvas fallback, the newest-first lane).
