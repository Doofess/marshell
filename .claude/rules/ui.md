---
paths:
  - "src/**/*.ts"
  - "src/**/*.tsx"
  - "src/**/*.css"
---

# Frontend rules

- Every UI change works with the keyboard alone and in forced colors. (AGENTS.md)
- Take colors from the tokens in `src/styles/` (`tokens.css`, `accents.css`). Don't put hex values in a stylesheet. (PLAN.md §Visual system)
- Read the good-css skill before writing declarations. (Global CLAUDE.md, Defaults)
- Don't guess status values. Leave an unknown value out of a row or strip; a table or form shows an en dash. (AGENTS.md; PLAN.md §Sidebar rows)
- Use the exact copy from the plan, and the microcopy tone: sentence case, subject first, concrete numbers, no "!" or "successfully". (PLAN.md §Microcopy)
- No boxed outlines: tell cards, buttons and chips apart by fill and tint. `noBoxedOutlines.test.ts` enforces it. (PLAN.md §Visual system)
- The main window shows no scrollbar. Only the terminal conversation and the session list scroll. (PLAN.md §Visual system)
- Use the shared a11y blocks in `src/components/a11y/`. `Announcer` is the only live region, and `SelectedMark` is the only selected-state marker. (PLAN.md §Accessibility safety net)
- A `forced-colors` rule uses system colors only. (PLAN.md §Accessibility safety net)
- The frontend talks only to the core API, never to a CLI directly. (PLAN.md §1)
- Test components through `renderToStaticMarkup`; Vitest runs in node with no DOM. (AGENTS.md)
- xterm.js 6 on Windows: Ctrl+V reaches the CLI as ^V and fires no paste event, and Ctrl+Shift+V is the terminal's paste. (CLAUDE.md, Gotchas)
- Never enable Tauri's native file drop; it disables in-page HTML5 drag and drop on Windows. (CLAUDE.md, Gotchas)
