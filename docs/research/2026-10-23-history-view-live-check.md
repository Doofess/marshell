# History view live check (slice 5a)

- Date: 2026-10-23, 21:15 to 21:35 local time.
- Build: `phase-5a` under `pnpm tauri dev` on Windows 11 Pro (26300).
- Home: a scratch `MARSHELL_HOME` in the session's temp folder, with CDP on port 9333. The onboarding was left with Continue, then "Skip: use limited status", which writes only the scratch `settings.json`.
- Data: five archived Claude Code conversations, seeded by the ignored test `seed_history_home` in `crates/core/tests/history_api.rs` (`MARSHELL_SEED_HOME=<home> cargo test -p marshell-core --test history_api seed_history_home -- --ignored`). It copies the five transcript fixtures into the scratch home's archive, with sidecars, updated times and folders:

  | Session (`…000b<n>`) | Fixture | Folder | Updated |
  | --- | --- | --- | --- |
  | `b1` | basic | `work\demo` | 2 h ago |
  | `b2` | tools | `work\demo` | 5 h ago |
  | `b3` | subagent and compact | `work\billing` | 26 h ago |
  | `b4` | commands | `work\billing` | 9 days ago; its original transcript was deleted after the copy |
  | `b5` | version 9.9.9 | `work\demo` | 40 days ago |

- How it was driven: the webview over CDP only, with evaluations returning small JSON, CDP key events inside the webview, emulated forced colours and three screenshots. No desktop mouse or keyboard input was sent.

## Results

| Check | Result |
| --- | --- |
| Open and close | Pass. Ctrl+Shift+Y opens History in the main area, with the sidebar still there and focus in the search box. A second press closes it, as slice decision 3 says. |
| List | Pass. Two groups, newest first: demo (b1, b2, b5) and then billing (b3, b4). Each row has its title and the line "folder · branch · when · turns", for example "demo · feature/demo · 2 h ago · 2 turns". |
| Keys in the list | Pass. ArrowDown in the search box moves into the list. The arrows cross from one group to the next, Home and End go to the first and last rows, ArrowDown on the last row stays there, and Enter opens the selected row. |
| Search | Pass. "Frobnicate", a word that appears only in a tool card's summary ("Used Frobnicate"), finds one hit in b2, with the word in `<mark>`. "five" finds b1 at "demo · turn 2", and opening that hit marks its second turn as the target. |
| Tool cards | Pass after a fix (finding 1). Enter on a card's summary opens and closes it. Tab moves from one card to the next. |
| Tab order | Pass after the same fix. It runs search box, the five filters, the list, the conversation, Resume, then the tool cards. |
| Limited notice | Pass. b5 shows "Status limited: Claude Code 9.9.9 saved this conversation, and Marshell hasn't been tested with that version. Some parts may be missing." (`role="note"`). |
| Esc | Pass. With a search and a conversation open, the first Esc closes the conversation and keeps the search, the second clears the search and the five rows come back, and the third closes History. |
| Ctrl+Shift+Alt+F | Pass. With History closed, it opens the view with focus in the search box. |
| Forced colours | Pass, emulated (not a real high-contrast theme). Filters, rows and cards have system-colour borders, the search match is a Highlight block, and the focused card and the target turn have outlines. |
| Resume b1 | Pass for the command and the folder. Resume opened tab "1 Claude Code demo", closed History and put focus in the terminal. The session ran `pwsh -NoLogo -NoExit -Command "& claude --resume 00000000-0000-4000-8000-0000000000b1"` in `work\demo`, and Claude Code's trust prompt named that folder. It reached the real Claude Code, not the planned shim (finding 2). |
| Resume b4 | Pass. History stayed open with the refusal "Claude Code no longer has this conversation, so it can't be resumed." above the conversation, and the announcer said it. No process started. |
| Search time | Pass. `cargo test --release -p marshell-core search_1000_sessions_under_100ms -- --ignored --nocapture` gives a median of 6.786 ms over 1000 sessions of 20 turns each. The phase's exit criterion is under 100 ms. |

## Findings

1. **Tab was stuck on History's filters and tool cards.** The window's navigation layer takes a plain Tab when focus is "on nothing", meaning to select the first terminal. `focusKindOf` counted buttons, links, options and anything with a `tabindex` as controls, but not a `<select>` or a `<summary>`. So Tab on the first filter, or on a tool card, was taken and did nothing, and the keyboard could not get past the filters or from card to card. Fixed in e29da94: both count as controls, with a `useNavigation.test.ts` case that failed first.
2. **The `claude` shim was not used, and the real Claude Code ran.** The check put a `claude.cmd` shim first on PATH before `pnpm tauri dev`. The session's `pwsh` did not get that PATH: its first entries were PowerShell's own folder and System32. So `& claude` found `C:\Users\Home Office\.local\bin\claude.exe`, the person's real Claude Code, with its real config folder. It stopped at its folder trust prompt, nothing was typed into it, and it was stopped by PID. Before it was stopped, its startup wrote Claude Code's own bookkeeping to `~/.claude`: a `sessions\<pid>.*.key` file, plugin `.in_use\<pid>` markers, and the plugin-sync and model-catalog caches. `settings.json` (same modified time before and after), `projects\` and the trust entries in `~/.claude.json` were not touched. A later check that starts or resumes a Claude Code session sets `CLAUDE_CONFIG_DIR` to a scratch folder, as the 4c departures check did.
3. **Closing History with no sessions leaves focus on the page.** Esc or Ctrl+Shift+Y returns focus to the selected terminal, and with no sessions there is none, so focus goes to `<body>`. The preview pane does the same. Not fixed.
4. **A conversation that starts with a slash command is titled by it.** b4 lists as "/model opus", because the title is the first prompt. Renaming comes in 5b. Not fixed.
5. **Not seen live: "Go to session".** It needs a live tab whose id matches an archived conversation's tab, and the seeded conversations' tabs don't exist in this run. It is covered by the `liveTabFor` and HistoryView tests.
6. **Not seen live: links in assistant text.** The fixtures' paths are code spans, not links. File and web links are covered by the MarkdownText and LiveHistory tests.
7. **A seeding artefact.** Each turn's time is the fixture's own (20 October), while the list shows the updated time the seed gave the sidecar. This is not a bug in the view.
