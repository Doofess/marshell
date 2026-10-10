# Phase 0 batch 3: onboarding, doctor, settings, Departures, notifications, tray. Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Storybook mockups, built on real reusable components, of phase 0 deliverables 7 to 11 (onboarding with the consent diff and every empty state; doctor report, fix sheet and undo toast; settings page and editor; Departures board with a 10 s motion clip; OS notification mocks, the icon at 16/32 px and the tray menu), reviewed and signed off in three sub-reviews: **A** onboarding, **B** doctor and settings, **C** Departures, notifications and tray.

**Architecture:**
- Pure logic lives in small tested modules (diff parsing, consent plans, doctor reports and fix plans, settings fields and scopes, board rows and the flip schedule, banner rules, tray items). Components are presentational and take view models, exactly as in batches 1 and 2.
- One shared `DiffView` shows every change Marshell proposes (consent, doctor fix, settings change). One shared `Page` frame, `EmptyState` and `Switch` serve the page-like screens.
- Stories render every state. The review page (`scripts/build-review.mjs`, `src/design/review/renderReview.tsx`) server-renders them into one cumulative HTML file; batches `3a`, `3b`, `3c` are added after `2c`.

**Tech Stack:** React 19, TypeScript 7 (strict, `noUncheckedIndexedAccess`), Vite 8, Storybook 10.6 (addon-a11y, addon-themes), Vitest 5 (environment `node`; components are tested through `react-dom/server`; CSS is read with `node:fs`), culori 4, Playwright MCP for browser and axe checks.

**Spec:** `docs/PLAN.md`: Phase 0 table rows 7 to 11, "Key flows" (First run, Doctor fix and undo), "The approve card" (risky requests, OS notifications for risky requests), "Departures board", "Microcopy", "UX risks", section 6 (config safety layer: preview, minimal edits, backup, undo, managed paths), section 7 (notifications and tray per OS), and the batch 2 subsections "Session numbers and manual CLIs", "Keyboard navigation and shortcuts", "Accessibility safety net". `docs/BRIEF.md` Amendments win over the body. Earlier plans: `docs/superpowers/plans/2026-10-10-phase-0-batch-2-card-window-launcher.md` (same machinery, same patterns).

**Branch:** `phase-0-batch-3` (cut from `phase-0-design` while pull request #2 is open). Do not push; the controller asks the owner before pushing.

## Global Constraints

Every task's requirements include this section. Copy of what the batch 2 system already enforces, so the new screens obey it from the start.

- **Platform:** Windows 11 is the only tested platform. macOS and Linux appear only as design mocks (labelled "design only") and as per-OS copy and shortcut labels.
- **Session numbers and marks:** every place that names a session shows its number (`SessionNumber`, stable, lowest free) and its mark (`AgentMark`, which also draws manual CLIs). Accessible names start `Session N, ...` (`src/lib/labelInName.ts` checks that the visible text is inside the name).
- **No boxed outlines:** controls and containers are told apart by fills (`--fill`, `--fill-strong`, `--selected`) and edge bars, never a 1 px border. `src/styles/noBoxedOutlines.test.ts` scans every CSS file under `src/features`, `src/components` and `src/styles` automatically. The only exceptions are the two documented in that test's allowlist; do not add a third without a recorded ruling.
- **Tokens only:** no hex, `rgb()` or raw `oklch()` colours in feature or component CSS (`src/styles/tokensOnly.test.ts` scans automatically). Shadows use `--shadow-menu` / `--shadow-pill`.
- **Forced colours:** every selected, focused or toggled state also has a `@media (forced-colors: active)` rule with system colours only and no `border-*-color` (`src/styles/forcedColorsEdges.test.ts`). Selection uses `selectedProps()` and `src/components/a11y/SelectedMark.css`.
- **Scrolling:** the live main window never scrolls (`src/styles/noScrollInMainWindow.test.ts`, allowlist of class names). **Ruling for this batch:** the page-like screens (Doctor, Settings, the diff sheet) are not the live window; their body (`.page__body`) may scroll, and it keeps a *visible* themed scrollbar (`src/styles/scrollbars.css`), like the right drawer. `.page__body` is added to that allowlist with a test that asserts its scrollbar is not hidden, so the exception cannot spread. Onboarding screens, the Departures board, banners and the tray menu never scroll.
- **One announcer:** no `role="status"`, `aria-live` or `role="alert"` in any new component (`src/components/a11y/sharedBlocks.test.ts` fails otherwise). A screen that must say something returns text for the window's single `Announcer` (`src/components/a11y/Announcer.tsx`, policy in `announce-policy.ts`). A result the user caused ("Fixed 4 issues...") is announced politely in every announce mode; progress is never announced.
- **Shortcuts:** only commands from `src/lib/keymap.ts`, matched by physical key (`KeyboardEvent.code`), no punctuation. Any new binding must pass `problemsIn(COMMANDS, os)` (`src/lib/bindings.ts`) on windows, mac and linux. Plain Tab, Esc and arrows reach the program; navigation uses the held Ctrl+Shift chord.
- **Microcopy** (docs/PLAN.md "Microcopy"): sentence case, subject first, the agent's name (never "the AI"), concrete numbers, say what changes and how to undo it. Never "!", "successfully" or "Oops". Unknown values show an en dash (`–`), never a guess.
- **Config safety:** every proposed change is shown as the exact unified diff, names the backup path and the removal or undo path, and never offers to edit managed or organisation settings (they are listed read-only).
- **Reuse, do not re-invent:** `ThemePair` for dark/light stories, `Chip`, `StatusGlyph`, `AgentMark`, `SessionNumber`, `selectedProps`, `Announcer`, the existing fixtures (`src/features/sidebar/fixtures.ts`, `src/features/window/scenario.ts`).
- **TDD:** each task writes the test first and watches it fail for the stated reason before the implementation. Run the whole suite (`pnpm vitest run`) and `pnpm exec tsc --noEmit` before every task's last commit.
- **Case-insensitive filesystem:** never name a logic file the same as a component (`Foo.ts` next to `Foo.tsx`); use `fooLogic.ts` style names (see `launchChoice.ts`, `laneLayout.ts`).
- **Commits:** end every commit message with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (second `-m`). No push.

## Review Focus

The final reviewer checks each deliberately; each has a pinning test in the owning task.

1. **Zero CLIs detected** (Task 5, Task 7): onboarding shows an `EmptyState` with a way forward (continue with shells and tools only), never a dead end or a blank card.
2. **Home path with a space** (`C:\Users\Home Office\.claude\settings.json`) (Task 4, Task 8, Task 11): paths are shown whole in mono, wrap at separators, never truncate the file name, and a quoted bridge path appears in the diff.
3. **200-line diff** (Task 2, Task 9, Task 11): `DiffView` collapses to 40 lines with "Show all 200 lines", never overflows its container, and the fix sheet's page body scrolls instead of the window.
4. **Config that does not parse** (Task 4, Task 8, Task 10, Task 11): onboarding, doctor and settings all say so, name the file and line, offer no fix and no edit ("fail closed").
5. **Read-only file and managed settings present** (Task 4, Task 8, Task 10, Task 11): managed entries are listed read-only with the reason; no control can change them; a read-only file disables Apply and says why.
6. **12 sessions on Departures, plus zero** (Task 13, Task 14): the board never scrolls the window (it pages by a fixed row count with "+N more" going to the sidebar's all-sessions menu pattern) and zero sessions shows an `EmptyState`.
7. **RTL and very long names** (Tasks 5, 9, 11, 14): names use `dir="auto"`, truncate at the end with the full name in the title, and numbers stay on the start edge.
8. **Banners for risky requests** (Task 15): a risky permission banner offers only "Review", never shows the command, and no banner raises the window.
9. **Stale undo** (Task 8, Task 9): "Undo last fix" is only offered while the files still match the post-fix state; otherwise the UI says why and shows the diff instead.

## Pre-flight: shared interfaces

| Producer | Consumer | What is shared | Finding / ruling |
|---|---|---|---|
| Task 1 (review scaffolding) | Tasks 7, 12, 17 | `UpTo` gains `"3a" \| "3b" \| "3c"`; `BATCHES["3x"]`; `META["3x"]`; CSS list in `build-review.mjs` | Each later task only appends to the arrays Task 1 creates. |
| Task 2 `DiffView` / `parseUnifiedDiff` | Tasks 5, 9, 11 | `parseUnifiedDiff(text): ParsedDiff`, `<DiffView diff label limit />` | One diff component for consent, doctor fix and settings change. |
| Task 2 `unifiedDiff(path, before, after)` | Tasks 4, 8, 10 | builds the diff text shown by `DiffView` from file text | Phase 3 replaces it with the core's `ChangeSet` diff; the string format (git unified) is the contract. |
| Task 3 `Page`, `EmptyState`, `Switch` | Tasks 5, 7, 9, 11 | presentational frames; `.page__body` scroll exception | Allowlist entry added in Task 3 with its guard test. |
| Task 4 `Detection`, `CliId`, `Capability` types | Tasks 5, 8, 10 | the three CLIs, config state (`ok`, `missing`, `unparsable`, `read-only`), `managed` flag | `CliId = "claude" \| "codex" \| "gemini"`; shared with doctor items and settings scopes. |
| Task 8 `DoctorItem` | Task 9, sidebar footer | `SidebarFooter.doctorIssues` (existing, `src/features/window/scenario.ts`) must equal `counts(report).issues` | Task 9 sets it from `counts(DOCTOR_REPORT).issues` and updates the tests that cited 2. |
| Task 10 `Scope` | Task 11 | the scope badge words User / Project / Local / Managed / Marshell | One `SCOPE_LABEL` table. |
| Task 13 `BoardRow` | Task 14 | built from `RowModel` (session number, mark, status, waiting, context) | Manual CLI rows show `RUNNING` / `AT PROMPT` / `EXITED` and `–` for context. |
| Task 15 `bannerFor` | Tasks 16 (tray), 17 | `NotifyEvent`, `Os` types | `Os` is the one from `src/lib/keymap.ts`. |

Rows for tasks that share nothing: Tasks 6 and 18 only consume earlier tasks. The scan found no contradictions between task texts; the one spec tension (Departures "true black" vs the light theme) is ruled in Task 13.

---

### Task 1: Review page scaffolding for batches 3a, 3b, 3c

**Files:**
- Modify: `src/design/review/renderReview.tsx`
- Modify: `scripts/build-review.mjs`
- Test: `src/design/review/renderReview.test.tsx`

**Interfaces:**
- Produces: `UpTo` includes `"3a" | "3b" | "3c"`; `ORDER = ["1","2a","2b","2c","3a","3b","3c"]`; `BATCHES["3a"|"3b"|"3c"]` (empty arrays now); `META["3a"|"3b"|"3c"]` in the build script; `withTerminal` includes the three new ids (the cumulative page still contains the window stories).

- [ ] **Step 1: Write the failing test** (append to `renderReview.test.tsx`)

```tsx
describe("renderReview 3a to 3c", () => {
  it("is cumulative: 3a keeps everything up to 2c", () => {
    const html = renderReview("3a");
    expect(html).toContain('id="launcher"');
    expect(html).toContain('id="command-palette"');
  });
  it("accepts 3b and 3c", () => {
    expect(() => renderReview("3b")).not.toThrow();
    expect(() => renderReview("3c")).not.toThrow();
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `pnpm vitest run src/design/review/renderReview.test.tsx`
Expected: FAIL (`"3a"` is not in `ORDER`, so `ORDER.indexOf` returns -1 and `slice(0, 0)` renders nothing: the first test fails on `id="launcher"`).

- [ ] **Step 3: Implement**

In `renderReview.tsx` change the type and tables:

```tsx
export type UpTo = "1" | "2a" | "2b" | "2c" | "3a" | "3b" | "3c";
const ORDER: UpTo[] = ["1", "2a", "2b", "2c", "3a", "3b", "3c"];
```

and add `"3a": [], "3b": [], "3c": [],` to `BATCHES`.

In `scripts/build-review.mjs`: update the usage comment to `[1|2a|2b|2c|3a|3b|3c]`, add `"3a","3b","3c"` to the `withTerminal` list, and add three META blocks after `META["2c"]` (copy the shape of `META["2c"]`; `lede`, `nav`, `checks`):

```js
META["3a"] = {
  lede: "Sub-review A of 3 in batch 3: onboarding. Batch 2 (approve card, window, launcher, palette) is below.",
  nav: [["onboarding", "Onboarding"], ["empty-states", "Empty states"], ["sign-off", "Sign-off"]],
  checks: ["Placeholder: replaced in Task 7."],
};
META["3b"] = {
  lede: "Sub-review B of 3 in batch 3: the doctor and the settings page. Onboarding and batch 2 are below.",
  nav: [["doctor", "Doctor"], ["settings", "Settings"], ["onboarding", "Onboarding"], ["sign-off", "Sign-off"]],
  checks: ["Placeholder: replaced in Task 12."],
};
META["3c"] = {
  lede: "Sub-review C of 3 in batch 3: Departures, notifications and the tray. Everything signed off earlier is below.",
  nav: [["departures", "Departures"], ["notifications", "Notifications"], ["tray-and-icon", "Tray"], ["sign-off", "Sign-off"]],
  checks: ["Placeholder: replaced in Task 17."],
};
```

(The placeholders are replaced by real check lists in Tasks 7, 11 and 16; the test `buildReviewScript.test.ts` already reads META, run it too.)

- [ ] **Step 4: Run to verify it passes**

Run: `pnpm vitest run src/design/review && pnpm exec tsc --noEmit`
Expected: PASS, tsc clean.

- [ ] **Step 5: Commit**

```bash
git add src/design/review scripts/build-review.mjs
git commit -m "feat(review): scaffolding for batches 3a to 3c" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: DiffView and the unified diff builder

**Files:**
- Create: `src/lib/diff.ts`, `src/lib/diff.test.ts`
- Create: `src/components/DiffView/diff.ts` (parser), `diff.test.ts`, `DiffView.tsx`, `DiffView.css`, `DiffView.test.tsx`, `DiffView.css.test.ts`, `DiffView.stories.tsx`

**Interfaces:**
- Produces:
  - `unifiedDiff(path: string, before: string, after: string, context = 3): string` (git-style: `--- a/<path>`, `+++ b/<path>`, `@@ -a,b +c,d @@` hunks; empty string when equal).
  - `parseUnifiedDiff(text: string): ParsedDiff`; `type DiffKind = "meta" | "hunk" | "context" | "add" | "del"`; `type DiffLine = { kind: DiffKind; text: string; oldNo?: number; newNo?: number }`; `type ParsedDiff = { lines: DiffLine[]; added: number; removed: number; files: string[] }`; `collapse(lines, limit): { shown: DiffLine[]; hidden: number }`; `SIGN: Record<DiffKind, string>`.
  - `<DiffView diff={string} label={string} limit?={number} expanded?={boolean} />`.

- [ ] **Step 1: Write the failing tests**

`src/components/DiffView/diff.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { collapse, parseUnifiedDiff } from "./diff";

const SAMPLE = [
  "--- a/settings.json",
  "+++ b/settings.json",
  "@@ -3,4 +3,6 @@",
  '   "model": "opus",',
  '-  "cleanupPeriodDays": 30,',
  '+  "cleanupPeriodDays": 365,',
  '+  "hooks": {},',
  "   \"theme\": \"dark\"",
].join("\n");

describe("parseUnifiedDiff", () => {
  it("counts additions and removals and finds the file", () => {
    const d = parseUnifiedDiff(SAMPLE);
    expect(d.added).toBe(2);
    expect(d.removed).toBe(1);
    expect(d.files).toEqual(["settings.json"]);
  });
  it("numbers lines from the hunk header on both sides", () => {
    const l = parseUnifiedDiff(SAMPLE).lines;
    const ctx = l.find((x) => x.kind === "context")!;
    expect([ctx.oldNo, ctx.newNo]).toEqual([3, 3]);
    const del = l.find((x) => x.kind === "del")!;
    expect(del.oldNo).toBe(4);
    const add = l.find((x) => x.kind === "add")!;
    expect(add.newNo).toBe(4);
  });
  it("treats a removed line that starts with dashes as a removal inside a hunk", () => {
    const d = parseUnifiedDiff(["--- a/x", "+++ b/x", "@@ -1,2 +1,1 @@", "--- not a header", " keep"].join("\n"));
    expect(d.removed).toBe(1);
  });
  it("reads CRLF input and an empty diff", () => {
    expect(parseUnifiedDiff(SAMPLE.replace(/\n/g, "\r\n")).added).toBe(2);
    expect(parseUnifiedDiff("")).toEqual({ lines: [], added: 0, removed: 0, files: [] });
  });
});

describe("collapse", () => {
  it("keeps the first lines and says how many are hidden", () => {
    const lines = Array.from({ length: 200 }, (_, i) => ({ kind: "context" as const, text: `l${i}` }));
    const c = collapse(lines, 40);
    expect(c.shown).toHaveLength(40);
    expect(c.hidden).toBe(160);
    expect(collapse(lines.slice(0, 10), 40).hidden).toBe(0);
  });
});
```

`src/lib/diff.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../components/DiffView/diff";
import { unifiedDiff } from "./diff";

describe("unifiedDiff", () => {
  it("is empty when nothing changed", () => {
    expect(unifiedDiff("a.json", "{}\n", "{}\n")).toBe("");
  });
  it("names the file with a/ and b/ prefixes and counts the change", () => {
    const d = unifiedDiff("C:/Users/Home Office/.claude/settings.json", '{\n  "a": 1\n}\n', '{\n  "a": 2,\n  "b": 3\n}\n');
    expect(d.split("\n").slice(0, 2)).toEqual(["--- a/C:/Users/Home Office/.claude/settings.json", "+++ b/C:/Users/Home Office/.claude/settings.json"]);
    const p = parseUnifiedDiff(d);
    expect(p.added).toBe(2);
    expect(p.removed).toBe(1);
  });
  it("keeps three lines of context around a change in a long file", () => {
    const before = Array.from({ length: 40 }, (_, i) => `line ${i}`).join("\n") + "\n";
    const after = before.replace("line 20", "LINE 20");
    const ctx = parseUnifiedDiff(unifiedDiff("f.txt", before, after)).lines.filter((l) => l.kind === "context");
    expect(ctx).toHaveLength(6);
  });
});
```

`src/components/DiffView/DiffView.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DiffView } from "./DiffView";

const small = ["--- a/s.json", "+++ b/s.json", "@@ -1,1 +1,2 @@", " a", "-b", "+c", "+d"].join("\n");
const big = ["--- a/s.json", "+++ b/s.json", "@@ -1,1 +1,200 @@", ...Array.from({ length: 200 }, (_, i) => `+line ${i}`)].join("\n");

describe("DiffView", () => {
  it("marks every changed line with a sign as well as a fill, and counts them", () => {
    const h = renderToStaticMarkup(<DiffView diff={small} label="Changes to s.json" />);
    expect(h).toContain('data-kind="add"');
    expect(h).toContain('data-kind="del"');
    expect(h).toMatch(/class="diff__sign"[^>]*>\+</);
    expect(h).toMatch(/class="diff__sign"[^>]*>−</);
    expect(h).toContain("+2");
    expect(h).toContain("−1");
    expect(h).toContain('aria-label="Changes to s.json"');
  });
  it("collapses a 200-line change to 40 lines and offers Show all 200 lines", () => {
    const h = renderToStaticMarkup(<DiffView diff={big} label="Changes" />);
    expect(h).toContain("Show all 203 lines");
    expect((h.match(/class="diff__line"/g) ?? []).length).toBe(40);
  });
  it("shows everything when expanded and has no button", () => {
    const h = renderToStaticMarkup(<DiffView diff={big} label="Changes" expanded />);
    expect(h).not.toContain("Show all");
    expect((h.match(/class="diff__line"/g) ?? []).length).toBe(203);
  });
  it("says so when there is nothing to change", () => {
    expect(renderToStaticMarkup(<DiffView diff="" label="Changes" />)).toContain("No changes");
  });
  it("lets names in any script read in their own direction", () => {
    expect(renderToStaticMarkup(<DiffView diff={small} label="x" />)).toContain('dir="auto"');
  });
});
```

`src/components/DiffView/DiffView.css.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./DiffView.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("DiffView.css", () => {
  it("tints added and removed lines from the status tokens, with no border", () => {
    expect(css).toMatch(/\.diff__line\[data-kind="add"\]\s*\{[^}]*color-mix\(in oklch, var\(--ok\)/);
    expect(css).toMatch(/\.diff__line\[data-kind="del"\]\s*\{[^}]*color-mix\(in oklch, var\(--error\)/);
    expect(css).not.toMatch(/border(?!-radius)/);
  });
  it("wraps long lines instead of overflowing or scrolling sideways", () => {
    expect(css).toMatch(/white-space:\s*pre-wrap/);
    expect(css).toMatch(/overflow-wrap:\s*anywhere/);
    expect(css).not.toMatch(/overflow(-x)?:\s*(auto|scroll)/);
  });
  it("uses the mono font and keeps the sign column fixed", () => {
    expect(css).toMatch(/font-family:\s*var\(--font-mono\)/);
    expect(css).toMatch(/\.diff__sign\s*\{[^}]*inline-size/);
  });
});
```

- [ ] **Step 2: Run to verify they fail**

Run: `pnpm vitest run src/components/DiffView src/lib/diff.test.ts`
Expected: FAIL with "Cannot find module './diff'" and "./DiffView".

- [ ] **Step 3: Implement**

`src/components/DiffView/diff.ts`:

```ts
export type DiffKind = "meta" | "hunk" | "context" | "add" | "del";
export type DiffLine = { kind: DiffKind; text: string; oldNo?: number; newNo?: number };
export type ParsedDiff = { lines: DiffLine[]; added: number; removed: number; files: string[] };

export const SIGN: Record<DiffKind, string> = { meta: "", hunk: "", context: " ", add: "+", del: "−" };

/** Reads a git-style unified diff. Header lines are only headers outside a hunk, so a removed line that starts with dashes still counts as a removal. */
export function parseUnifiedDiff(input: string): ParsedDiff {
  const lines: DiffLine[] = [];
  const files: string[] = [];
  let added = 0;
  let removed = 0;
  let oldNo = 0;
  let newNo = 0;
  let inHunk = false;
  const raw = input.replace(/\r\n/g, "\n").split("\n");
  if (raw[raw.length - 1] === "") raw.pop();
  for (const l of raw) {
    if (l.startsWith("diff ")) {
      inHunk = false;
      lines.push({ kind: "meta", text: l });
      continue;
    }
    if (!inHunk && (l.startsWith("--- ") || l.startsWith("+++ ") || l.startsWith("index "))) {
      if (l.startsWith("+++ ")) files.push(l.slice(4).replace(/^b\//, ""));
      lines.push({ kind: "meta", text: l });
      continue;
    }
    const h = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(l);
    if (h) {
      inHunk = true;
      oldNo = Number(h[1]);
      newNo = Number(h[2]);
      lines.push({ kind: "hunk", text: l });
      continue;
    }
    if (l.startsWith("+")) {
      added++;
      lines.push({ kind: "add", text: l.slice(1), newNo: newNo++ });
    } else if (l.startsWith("-")) {
      removed++;
      lines.push({ kind: "del", text: l.slice(1), oldNo: oldNo++ });
    } else {
      lines.push({ kind: "context", text: l.startsWith(" ") ? l.slice(1) : l, oldNo: oldNo++, newNo: newNo++ });
    }
  }
  return { lines, added, removed, files };
}

export function collapse(lines: DiffLine[], limit: number): { shown: DiffLine[]; hidden: number } {
  return lines.length <= limit ? { shown: lines, hidden: 0 } : { shown: lines.slice(0, limit), hidden: lines.length - limit };
}
```

`src/lib/diff.ts` (line-level LCS; fine for settings-sized files; phase 3 replaces it with the core's diff):

```ts
/** A git-style unified diff of two texts, or "" when they are equal. */
export function unifiedDiff(path: string, before: string, after: string, context = 3): string {
  if (before === after) return "";
  const a = before.split("\n");
  const b = after.split("\n");
  if (a[a.length - 1] === "") a.pop();
  if (b[b.length - 1] === "") b.pop();
  // Longest common subsequence table.
  const n = a.length;
  const m = b.length;
  const t: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) t[i]![j] = a[i] === b[j] ? t[i + 1]![j + 1]! + 1 : Math.max(t[i + 1]![j]!, t[i]![j + 1]!);
  type Op = { k: " " | "-" | "+"; s: string };
  const ops: Op[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) ops.push({ k: " ", s: a[i++]! }), j++;
    else if (t[i + 1]![j]! >= t[i]![j + 1]!) ops.push({ k: "-", s: a[i++]! });
    else ops.push({ k: "+", s: b[j++]! });
  }
  while (i < n) ops.push({ k: "-", s: a[i++]! });
  while (j < m) ops.push({ k: "+", s: b[j++]! });

  const changed = ops.map((o, idx) => (o.k === " " ? -1 : idx)).filter((x) => x >= 0);
  const out = [`--- a/${path}`, `+++ b/${path}`];
  let k = 0;
  while (k < changed.length) {
    let start = Math.max(0, changed[k]! - context);
    let end = Math.min(ops.length, changed[k]! + context + 1);
    while (k + 1 < changed.length && changed[k + 1]! - context <= end) {
      k++;
      end = Math.min(ops.length, changed[k]! + context + 1);
    }
    k++;
    const slice = ops.slice(start, end);
    let oldStart = 1;
    let newStart = 1;
    for (const o of ops.slice(0, start)) {
      if (o.k !== "+") oldStart++;
      if (o.k !== "-") newStart++;
    }
    const oldLen = slice.filter((o) => o.k !== "+").length;
    const newLen = slice.filter((o) => o.k !== "-").length;
    out.push(`@@ -${oldStart},${oldLen} +${newStart},${newLen} @@`);
    for (const o of slice) out.push(o.k + o.s);
    start = end;
  }
  return out.join("\n") + "\n";
}
```

`src/components/DiffView/DiffView.tsx`:

```tsx
import { collapse, parseUnifiedDiff, SIGN } from "./diff";
import "./DiffView.css";

/** A unified diff, read-only: every changed line has a sign as well as a fill, and a long one folds after `limit` lines. */
export function DiffView({ diff, label, limit = 40, expanded = false }: { diff: string; label: string; limit?: number; expanded?: boolean }) {
  const p = parseUnifiedDiff(diff);
  const { shown, hidden } = expanded ? { shown: p.lines, hidden: 0 } : collapse(p.lines, limit);
  return (
    <figure className="diff" aria-label={label}>
      <figcaption className="diff__head">
        <span className="diff__file" dir="auto">
          {p.files.join(", ") || (p.lines.length === 0 ? "No changes" : "Changes")}
        </span>
        {p.lines.length > 0 && (
          <span className="diff__count">
            <span className="diff__plus">+{p.added}</span> <span className="diff__minus">−{p.removed}</span>
          </span>
        )}
      </figcaption>
      <ol className="diff__lines">
        {shown.map((l, i) => (
          <li key={i} className="diff__line" data-kind={l.kind}>
            <span className="diff__no" aria-hidden="true">
              {l.newNo ?? l.oldNo ?? ""}
            </span>
            <span className="diff__sign">{SIGN[l.kind]}</span>
            {l.kind === "add" && <span className="diff__sr">added </span>}
            {l.kind === "del" && <span className="diff__sr">removed </span>}
            <code dir="auto">{l.text}</code>
          </li>
        ))}
      </ol>
      {hidden > 0 && (
        <button type="button" className="diff__more">
          Show all {p.lines.length} lines
        </button>
      )}
    </figure>
  );
}
```

`src/components/DiffView/DiffView.css` (read `good-css` first; key rules):

```css
.diff { margin: 0; display: grid; gap: var(--space-1); font-size: var(--text-12); }
.diff__head { display: flex; justify-content: space-between; gap: var(--space-3); color: var(--text-2); font-variant-numeric: tabular-nums; }
.diff__file { min-inline-size: 0; overflow-wrap: anywhere; }
.diff__lines { margin: 0; padding: 0; list-style: none; border-radius: var(--radius); background: var(--fill); }
.diff__line { display: grid; grid-template-columns: 3ch 1.5ch 1fr; gap: var(--space-2); padding-inline: var(--space-2); font-family: var(--font-mono); white-space: pre-wrap; overflow-wrap: anywhere; }
.diff__line[data-kind="add"] { background: color-mix(in oklch, var(--ok) 16%, transparent); }
.diff__line[data-kind="del"] { background: color-mix(in oklch, var(--error) 16%, transparent); }
.diff__line[data-kind="hunk"] { color: var(--text-2); background: var(--fill-strong); }
.diff__line[data-kind="meta"] { color: var(--text-2); }
.diff__no { color: var(--text-3); text-align: end; font-variant-numeric: tabular-nums; }
.diff__sign { inline-size: 1.5ch; font-family: var(--font-mono); }
.diff__line code { font: inherit; }
.diff__sr { position: absolute; inline-size: 1px; block-size: 1px; overflow: clip; clip-path: inset(50%); white-space: nowrap; }
.diff__more { justify-self: start; padding-inline: var(--space-2); background: var(--fill); color: var(--text-1); border-radius: var(--radius); }
.diff__more:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.diff__more:active { background: var(--fill-strong); }
@media (hover: hover) and (pointer: fine) { .diff__more:hover { background: var(--fill-strong); } }
@media (forced-colors: active) {
  .diff__line[data-kind="add"], .diff__line[data-kind="del"] { forced-color-adjust: none; color: CanvasText; background: Canvas; }
  .diff__more { outline: 1px solid CanvasText; }
}
```

(The `.diff__more` forced-colors outline follows the repo's rule file `src/styles/forcedColorsEdges.test.ts`: look at how `.chip` is listed there and add `DiffView.css` rows in Step 5.)

`DiffView.stories.tsx`: stories `SmallChange`, `LongChange` (200 lines), `Expanded`, `NothingToChange`, `RtlNames` inside `ThemePair` (pattern: `src/features/palette/Palette.stories.tsx`, `title: "Diff view"`).

- [ ] **Step 4: Run to verify they pass**

Run: `pnpm vitest run src/components/DiffView src/lib/diff.test.ts`
Expected: PASS.

- [ ] **Step 5: Add the forced-colours rows and run the whole suite**

In `src/styles/forcedColorsEdges.test.ts` add the `DiffView.css` rows next to the existing component rows (the file holds a table of `[css file, selector, expectation]`; copy the `.chip` entry's shape). Add `"src/components/DiffView/DiffView.css"` to `scripts/build-review.mjs`'s CSS list.
Run: `pnpm vitest run && pnpm exec tsc --noEmit`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add src/lib/diff.ts src/lib/diff.test.ts src/components/DiffView src/styles/forcedColorsEdges.test.ts scripts/build-review.mjs
git commit -m "feat(ui): DiffView and a unified diff builder" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Page frame, EmptyState and Switch

**Files:**
- Create: `src/components/Page/Page.tsx`, `Page.css`, `Page.test.tsx`, `Page.css.test.ts`
- Create: `src/components/EmptyState/EmptyState.tsx`, `EmptyState.css`, `EmptyState.test.tsx`
- Create: `src/components/Switch/Switch.tsx`, `Switch.css`, `Switch.test.tsx`
- Modify: `src/styles/noScrollInMainWindow.test.ts` (allowlist), `scripts/build-review.mjs` (CSS list)

**Interfaces:**
- Produces:
  - `<Page title level? subtitle? actions? children />`: a `section` with an `h2` (or `h1` when `level={1}`), header row and a `.page__body` that scrolls with a visible themed scrollbar.
  - `<EmptyState title body action? />` where `action?: { label: string }`: a plain block (no card) with the title, one sentence and an optional button.
  - `<Switch label checked disabled? reason? />`: a native `<input type="checkbox" role="switch">` with the words "On" / "Off" always visible.

- [ ] **Step 1: Write the failing tests**

```tsx
// src/components/Page/Page.test.tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Page } from "./Page";

describe("Page", () => {
  it("is a labelled section with a heading, actions and a body", () => {
    const h = renderToStaticMarkup(<Page title="Doctor" subtitle="Checked 12 s ago" actions={<button>Run again</button>}>body</Page>);
    expect(h).toContain("<h2");
    expect(h).toContain("Doctor");
    expect(h).toContain("Run again");
    expect(h).toContain('class="page__body"');
    expect(h).toMatch(/<section[^>]*aria-labelledby="([^"]+)"/);
  });
  it("can be the page's h1", () => {
    expect(renderToStaticMarkup(<Page title="Settings" level={1}>x</Page>)).toContain("<h1");
  });
});
```

```ts
// src/components/Page/Page.css.test.ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
const css = readFileSync(new URL("./Page.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
describe("Page.css", () => {
  it("scrolls the body only, and keeps its scrollbar visible (the drawer's exception, not the window's)", () => {
    expect(css).toMatch(/\.page__body\s*\{[^}]*overflow-y:\s*auto/);
    expect(css).not.toMatch(/scrollbar-width:\s*none|::-webkit-scrollbar\s*\{\s*display:\s*none/);
    expect(css).toMatch(/scrollbar-gutter:\s*stable/);
  });
  it("fills the space it is given instead of growing the window", () => {
    expect(css).toMatch(/\.page\s*\{[^}]*(block-size|min-block-size):\s*0|grid-template-rows:\s*auto\s+minmax\(0,\s*1fr\)/);
  });
});
```

```tsx
// src/components/EmptyState/EmptyState.test.tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { EmptyState } from "./EmptyState";
describe("EmptyState", () => {
  it("says what is empty, one way forward, and has no live region", () => {
    const h = renderToStaticMarkup(<EmptyState title="No coding agents found" body="Install Claude Code, Codex or Gemini, or continue with shells and tools." action={{ label: "Continue with shells and tools" }} />);
    expect(h).toContain("No coding agents found");
    expect(h).toContain("Continue with shells and tools");
    expect(h).not.toMatch(/role="status"|aria-live|role="alert"/);
  });
  it("renders without an action", () => {
    expect(renderToStaticMarkup(<EmptyState title="All clear" body="Nothing needs fixing." />)).not.toContain("<button");
  });
});
```

```tsx
// src/components/Switch/Switch.test.tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Switch } from "./Switch";
describe("Switch", () => {
  it("is a switch whose state is a word, so it never depends on colour", () => {
    const on = renderToStaticMarkup(<Switch label="Approve from anywhere" checked />);
    expect(on).toContain('role="switch"');
    expect(on).toContain("checked");
    expect(on).toContain(">On<");
    expect(renderToStaticMarkup(<Switch label="Approve from anywhere" checked={false} />)).toContain(">Off<");
  });
  it("explains why it is unavailable and names the reason in the control's description", () => {
    const h = renderToStaticMarkup(<Switch label="Approve from anywhere" checked={false} disabled reason="Gemini has no approval hook, so Marshell focuses the tab instead." />);
    expect(h).toContain("disabled");
    expect(h).toContain("Gemini has no approval hook");
    expect(h).toMatch(/aria-describedby="([^"]+)"/);
  });
});
```

- [ ] **Step 2: Run to verify they fail**

Run: `pnpm vitest run src/components/Page src/components/EmptyState src/components/Switch`
Expected: FAIL (modules do not exist).

- [ ] **Step 3: Implement**

`Page.tsx`:

```tsx
import { useId, type ReactNode } from "react";
import "./Page.css";

export function Page({ title, subtitle, actions, level = 2, children }: { title: string; subtitle?: string; actions?: ReactNode; level?: 1 | 2; children: ReactNode }) {
  const id = useId();
  const H = level === 1 ? "h1" : "h2";
  return (
    <section className="page" aria-labelledby={id}>
      <header className="page__head">
        <div className="page__titles">
          <H id={id} className="page__title">{title}</H>
          {subtitle && <p className="page__subtitle">{subtitle}</p>}
        </div>
        {actions && <div className="page__actions">{actions}</div>}
      </header>
      <div className="page__body">{children}</div>
    </section>
  );
}
```

`Page.css`: `.page { display: grid; grid-template-rows: auto minmax(0, 1fr); block-size: 100%; min-block-size: 0; background: var(--bg-base); }`, `.page__head` flex with `padding: var(--space-4) var(--space-6)`, `.page__body { overflow-y: auto; scrollbar-gutter: stable; padding: var(--space-4) var(--space-6); }` (scrollbar styled by `src/styles/scrollbars.css`; do not hide it), `.page__title { font-size: var(--text-20); font-weight: 600; margin: 0 }`, `.page__subtitle { color: var(--text-2); font-size: var(--text-12); margin: 0 }`.

`EmptyState.tsx`:

```tsx
export function EmptyState({ title, body, action }: { title: string; body: string; action?: { label: string } }) {
  return (
    <div className="empty-state">
      <p className="empty-state__title">{title}</p>
      <p className="empty-state__body">{body}</p>
      {action && <button type="button" className="empty-state__action">{action.label}</button>}
    </div>
  );
}
```

`Switch.tsx`:

```tsx
import { useId } from "react";
import "./Switch.css";

export function Switch({ label, checked, disabled = false, reason }: { label: string; checked: boolean; disabled?: boolean; reason?: string }) {
  const id = useId();
  const why = `${id}-why`;
  return (
    <div className="switch" data-disabled={disabled || undefined}>
      <label className="switch__label">
        <input type="checkbox" role="switch" className="switch__input" defaultChecked={checked} disabled={disabled} aria-describedby={reason ? why : undefined} />
        <span className="switch__track" aria-hidden="true" />
        <span className="switch__text">{label}</span>
        <span className="switch__state">{checked ? "On" : "Off"}</span>
      </label>
      {reason && <p id={why} className="switch__reason">{reason}</p>}
    </div>
  );
}
```

Both CSS files follow `good-css` (logical properties, `:focus-visible` outline on `.switch__input:focus-visible + .switch__track`, `:active`, hover inside `@media (hover: hover) and (pointer: fine)`); the track is a `--fill-strong` pill with a thumb drawn by `::after`; `:checked + .switch__track` uses `color-mix(in oklch, var(--accent) 45%, transparent)`; `@media (forced-colors: active)` gives the track `outline: 1px solid CanvasText` and `:checked` `background: Highlight`. Add `.page__body` to `ALLOWED` in `src/styles/noScrollInMainWindow.test.ts` (`/^\.page__body$/`) and extend that file's "scrollbar hidden" assertions with a test that `Page.css` does **not** hide the bar (mirror how it asserts `.drawer` stays visible). Add the three CSS files to `build-review.mjs`.

- [ ] **Step 4: Run to verify they pass**

Run: `pnpm vitest run && pnpm exec tsc --noEmit`
Expected: all green (the rule tests scan the new CSS automatically).

- [ ] **Step 5: Commit**

```bash
git add src/components/Page src/components/EmptyState src/components/Switch src/styles/noScrollInMainWindow.test.ts scripts/build-review.mjs
git commit -m "feat(ui): Page frame, EmptyState and Switch" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Sub-review A: onboarding

### Task 4: Onboarding model: detection, capabilities, consent copy

**Files:**
- Create: `src/features/onboarding/types.ts`, `consent.ts`, `consent.test.ts`, `fixtures.ts`, `fixtures.test.ts`

**Interfaces:**
- Produces (all exported from `types.ts` / `consent.ts`):

```ts
export type CliId = "claude" | "codex" | "gemini";
export type ConfigState = "ok" | "missing" | "unparsable" | "read-only";
export type Detection = { cli: CliId; installed: boolean; version?: string; exePath?: string; configPath: string; configState: ConfigState; parseError?: { line: number; message: string }; managed?: { path: string } };
export type Capability = "status" | "approve" | "statusline";
export type Availability = { available: boolean; reason?: string };
export const CAPABILITIES: Record<CliId, Record<Capability, Availability & { entries: number }>>;
export type ConsentPlan = { cli: CliId; on: Record<Capability, boolean>; configPath: string; backupPath: string; removal: string; diff: string; needsTrust: boolean };
export function entryCount(cli: CliId, on: Record<Capability, boolean>): number;
export function consentHeadline(d: Detection, on: Record<Capability, boolean>): string;
export function canEdit(d: Detection): { ok: boolean; reason?: string };
export const CLI_NAME: Record<CliId, string>; // "Claude Code" | "Codex" | "Gemini"
```

- Rules (the tests pin them): Claude status = 8 hook entries, approve = 1 (`PermissionRequest`), statusline = chain (0 entries, a separate clause); Codex status = 6, approve = 1, statusline unavailable ("Codex has no status line to chain."), `needsTrust` true whenever any entry is on; Gemini status = 7, approve unavailable ("Gemini has no approval hook, so Marshell focuses the tab instead."), statusline unavailable.

- [ ] **Step 1: Write the failing tests** (`consent.test.ts`)

```ts
import { describe, expect, it } from "vitest";
import { canEdit, CAPABILITIES, consentHeadline, entryCount, type Detection } from "./consent";

const claude: Detection = { cli: "claude", installed: true, version: "2.1.296", configPath: "C:\\Users\\Home Office\\.claude\\settings.json", configState: "ok" };
const all = { status: true, approve: true, statusline: true };

describe("entryCount", () => {
  it("is 9 hook entries for Claude with status and approval on, and the status line adds none", () => {
    expect(entryCount("claude", all)).toBe(9);
    expect(entryCount("claude", { status: true, approve: false, statusline: true })).toBe(8);
    expect(entryCount("claude", { status: false, approve: false, statusline: false })).toBe(0);
  });
  it("counts only what the CLI can do", () => {
    expect(entryCount("gemini", { status: true, approve: true, statusline: true })).toBe(7);
    expect(entryCount("codex", all)).toBe(7);
  });
});

describe("CAPABILITIES", () => {
  it("says why Gemini has no approval and no status line", () => {
    expect(CAPABILITIES.gemini.approve).toMatchObject({ available: false, reason: "Gemini has no approval hook, so Marshell focuses the tab instead." });
    expect(CAPABILITIES.codex.statusline.available).toBe(false);
  });
});

describe("consentHeadline", () => {
  it("names the count, the status line and the whole path, spaces included", () => {
    expect(consentHeadline(claude, all)).toBe("Add 9 hook entries and chain your status line in C:\\Users\\Home Office\\.claude\\settings.json.");
  });
  it("leaves out the status line when it is off", () => {
    expect(consentHeadline(claude, { ...all, statusline: false })).toBe("Add 9 hook entries in C:\\Users\\Home Office\\.claude\\settings.json.");
  });
  it("says nothing changes when everything is off", () => {
    expect(consentHeadline(claude, { status: false, approve: false, statusline: false })).toBe("Change nothing. Claude Code will show limited status.");
  });
  it("fails closed on a file that does not parse, naming the line", () => {
    const d: Detection = { ...claude, configState: "unparsable", parseError: { line: 14, message: "Unexpected token }" } };
    expect(consentHeadline(d, all)).toBe("C:\\Users\\Home Office\\.claude\\settings.json does not parse (line 14), so Marshell will not edit it.");
    expect(canEdit(d)).toEqual({ ok: false, reason: "The file does not parse." });
  });
  it("never offers to edit a read-only file or a managed one", () => {
    expect(canEdit({ ...claude, configState: "read-only" })).toEqual({ ok: false, reason: "The file is read-only." });
    expect(canEdit({ ...claude, managed: { path: "C:\\Program Files\\ClaudeCode\\managed-settings.json" } }).ok).toBe(true);
  });
  it("follows the copy rules: no exclamation marks, no 'successfully', no 'Oops'", () => {
    for (const s of [consentHeadline(claude, all), consentHeadline(claude, { status: false, approve: false, statusline: false })]) expect(s).not.toMatch(/!|successfully|oops/i);
  });
});
```

(A managed file does not stop the user-level edit: Marshell edits the user file and lists the managed one read-only; the test pins `ok: true`.)

- [ ] **Step 2: Run to verify it fails**

Run: `pnpm vitest run src/features/onboarding/consent.test.ts`
Expected: FAIL ("Cannot find module './consent'").

- [ ] **Step 3: Implement** `types.ts` (the types above) and `consent.ts`:

```ts
import type { Capability, CliId, Detection } from "./types";
export * from "./types";

export const CLI_NAME: Record<CliId, string> = { claude: "Claude Code", codex: "Codex", gemini: "Gemini" };

const no = (reason: string) => ({ available: false, reason, entries: 0 });
export const CAPABILITIES: Record<CliId, Record<Capability, { available: boolean; reason?: string; entries: number }>> = {
  claude: { status: { available: true, entries: 8 }, approve: { available: true, entries: 1 }, statusline: { available: true, entries: 0 } },
  codex: { status: { available: true, entries: 6 }, approve: { available: true, entries: 1 }, statusline: no("Codex has no status line to chain.") },
  gemini: { status: { available: true, entries: 7 }, approve: no("Gemini has no approval hook, so Marshell focuses the tab instead."), statusline: no("Gemini has no status line to chain.") },
};

export function entryCount(cli: CliId, on: Record<Capability, boolean>): number {
  return (Object.keys(CAPABILITIES[cli]) as Capability[]).reduce((n, c) => n + (on[c] && CAPABILITIES[cli][c].available ? CAPABILITIES[cli][c].entries : 0), 0);
}

export function canEdit(d: Detection): { ok: boolean; reason?: string } {
  if (d.configState === "unparsable") return { ok: false, reason: "The file does not parse." };
  if (d.configState === "read-only") return { ok: false, reason: "The file is read-only." };
  return { ok: true };
}

export function consentHeadline(d: Detection, on: Record<Capability, boolean>): string {
  if (d.configState === "unparsable") return `${d.configPath} does not parse (line ${d.parseError?.line ?? 0}), so Marshell will not edit it.`;
  const n = entryCount(d.cli, on);
  const chain = on.statusline && CAPABILITIES[d.cli].statusline.available;
  if (n === 0 && !chain) return `Change nothing. ${CLI_NAME[d.cli]} will show limited status.`;
  const parts = [n > 0 ? `Add ${n} hook ${n === 1 ? "entry" : "entries"}` : ""].filter(Boolean);
  const text = chain ? (n > 0 ? `${parts[0]} and chain your status line` : "Chain your status line") : parts[0]!;
  return `${text} in ${d.configPath}.`;
}
```

`fixtures.ts`: `DETECTIONS` map of named scenarios used by stories: `allThree` (Claude 2.1.296 with the space path, Codex 0.9.3 at `C:\Users\Home Office\.codex\config.toml`, Gemini 0.4.2), `onlyClaude` (others `installed: false`), `none` (empty list), `unparsable` (Gemini `settings.json` parse error line 14), `readOnly` (Codex), `managedPresent` (Claude with `managed: { path: "C:\\Program Files\\ClaudeCode\\managed-settings.json" }`). `CONSENT_DIFFS`: for Claude a 20-line diff built with `unifiedDiff` (Task 2) from a `before` settings text with a `statusLine` entry to an `after` text with nine hook entries and the chain command `"C:/Users/Home Office/.marshell/bin/marshell-hook.exe" statusline --agent claude --tag mshl1`; also a 200-line variant for Codex (many `[[hooks]]` blocks) to exercise the collapse. `fixtures.test.ts` asserts: `none` has zero installed CLIs, every installed CLI's `configPath` contains `Home Office` in the main scenario, `CONSENT_DIFFS.claude` parses with `added >= 9`, `backupPath` is under `~/.marshell/snapshots/`.

- [ ] **Step 4: Run to verify it passes** — `pnpm vitest run src/features/onboarding && pnpm exec tsc --noEmit` → PASS.

- [ ] **Step 5: Commit** — `git add src/features/onboarding && git commit -m "feat(onboarding): detection, capabilities and consent copy" -m "Co-Authored-By: ..."` (full trailer as above).

---

### Task 5: Onboarding screens 1 and 2: Detected and Consent

**Files:**
- Create: `src/features/onboarding/Detected.tsx`, `ConsentCard.tsx`, `Onboarding.css`, `Detected.test.tsx`, `ConsentCard.test.tsx`, `Onboarding.css.test.ts`, `Onboarding.stories.tsx`
- Modify: `scripts/build-review.mjs` (CSS list: `src/features/onboarding/Onboarding.css`)

**Interfaces:**
- Consumes: `Detection`, `CAPABILITIES`, `consentHeadline`, `canEdit`, `CLI_NAME` (Task 4); `DiffView` (Task 2); `Switch`, `EmptyState` (Task 3); `AgentMark`, `Chip`.
- Produces: `<Detected detections />` (one row per CLI: mark, name, version or "Not installed", path; empty list → `EmptyState` "No coding agents found" + "Continue with shells and tools") and `<ConsentCard detection on plan open? />` where `open` expands "View exact changes".

- [ ] **Step 1: Write the failing tests**

```tsx
// Detected.test.tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DETECTIONS } from "./fixtures";
import { Detected } from "./Detected";

describe("Detected", () => {
  it("lists each CLI with its version, or Not installed", () => {
    const h = renderToStaticMarkup(<Detected detections={DETECTIONS.onlyClaude} />);
    expect(h).toContain("Claude Code");
    expect(h).toContain("2.1.296");
    expect(h).toContain("Not installed");
  });
  it("shows the whole path of a config under a folder with a space", () => {
    expect(renderToStaticMarkup(<Detected detections={DETECTIONS.allThree} />)).toContain("C:\\Users\\Home Office\\.claude\\settings.json");
  });
  it("offers a way forward when nothing is installed", () => {
    const h = renderToStaticMarkup(<Detected detections={DETECTIONS.none} />);
    expect(h).toContain("No coding agents found");
    expect(h).toContain("Continue with shells and tools");
  });
  it("says a file that does not parse, is read-only or is managed, in words", () => {
    expect(renderToStaticMarkup(<Detected detections={DETECTIONS.unparsable} />)).toContain("does not parse");
    expect(renderToStaticMarkup(<Detected detections={DETECTIONS.readOnly} />)).toContain("read-only");
    expect(renderToStaticMarkup(<Detected detections={DETECTIONS.managedPresent} />)).toContain("managed by your organisation");
  });
});
```

```tsx
// ConsentCard.test.tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { ConsentCard } from "./ConsentCard";
import { CONSENT } from "./fixtures";

const on = { status: true, approve: true, statusline: true };

describe("ConsentCard", () => {
  it("says what will change in plain words, with the backup and the way out", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} />);
    expect(h).toContain("Add 9 hook entries and chain your status line");
    expect(h).toContain("A backup is saved first");
    expect(h).toContain(".marshell\\snapshots");
    expect(h).toContain("Remove all hooks");
    expect(h).toContain("Skip: use limited status");
  });
  it("has a switch for each capability and explains an unavailable one", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.gemini} on={{ status: true, approve: false, statusline: false }} />);
    expect(h.match(/role="switch"/g)).toHaveLength(3);
    expect(h).toContain("Gemini has no approval hook, so Marshell focuses the tab instead.");
  });
  it("hides the diff until asked, then shows the exact change", () => {
    expect(renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} />)).not.toContain("diff__lines");
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} open />);
    expect(h).toContain("diff__lines");
    expect(h).toContain("marshell-hook.exe");
  });
  it("adds the trust step for Codex", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.codex} on={on} />);
    expect(h).toContain("Trust these hooks in /hooks");
  });
  it("replaces the controls with the reason when the file cannot be edited", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.unparsableGemini} on={on} />);
    expect(h).toContain("does not parse");
    expect(h).not.toContain('role="switch"');
    expect(h).toContain("Skip: use limited status");
  });
  it("lists a managed file read-only and offers no control for it", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claudeManaged} on={on} />);
    expect(h).toContain("managed-settings.json");
    expect(h).toContain("Marshell can't change it");
  });
  it("names its buttons from the text they show", () => {
    const h = renderToStaticMarkup(<ConsentCard scenario={CONSENT.claude} on={on} />);
    for (const b of h.match(/<button[\s\S]*?<\/button>/g) ?? []) expect(nameContainsLabel(b)).toBe(true);
  });
});
```

`CONSENT` (in `fixtures.ts`, add now): `{ scenario: { detection, plan } }` objects `claude`, `codex`, `gemini`, `unparsableGemini`, `claudeManaged`. `ConsentCard` takes `scenario: { detection: Detection; plan: ConsentPlan }`.

`Onboarding.css.test.ts`: reads `Onboarding.css` and asserts: no `border` other than `border-radius`/`border-inline-start` bar; `.consent` uses `--fill` background; `.consent[data-blocked]` has a start-edge bar (`border-inline-start`) and a forced-colours outline; paths use `overflow-wrap: anywhere` and `font-family: var(--font-mono)`; no `overflow: auto|scroll`.

- [ ] **Step 2: Run to verify they fail** — `pnpm vitest run src/features/onboarding` → FAIL (modules missing).

- [ ] **Step 3: Implement** the two components and CSS.

`ConsentCard.tsx` structure (implement fully; copy strings exactly as the tests expect):

```tsx
export function ConsentCard({ scenario, on, open = false }: { scenario: { detection: Detection; plan: ConsentPlan }; on: Record<Capability, boolean>; open?: boolean }) {
  const { detection: d, plan } = scenario;
  const edit = canEdit(d);
  const name = CLI_NAME[d.cli];
  return (
    <article className="consent" data-agent={d.cli} data-blocked={edit.ok ? undefined : ""} aria-labelledby={`consent-${d.cli}`}>
      <header className="consent__head"><AgentMark agent={d.cli} size={20} /><h3 id={`consent-${d.cli}`}>{name}</h3></header>
      <p className="consent__what" dir="auto">{consentHeadline(d, on)}</p>
      {edit.ok ? (
        <>
          <ul className="consent__switches">
            {(["status", "approve", "statusline"] as const).map((c) => (
              <li key={c}><Switch label={CAP_LABEL[c]} checked={on[c] && CAPABILITIES[d.cli][c].available} disabled={!CAPABILITIES[d.cli][c].available} reason={CAPABILITIES[d.cli][c].reason} /></li>
            ))}
          </ul>
          <button type="button" className="consent__toggle" aria-expanded={open}>View exact changes</button>
          {open && <DiffView diff={plan.diff} label={`Exact changes to ${d.configPath}`} />}
          <p className="consent__meta">A backup is saved first, in <code>{plan.backupPath}</code>. To undo: {plan.removal}</p>
          {plan.needsTrust && <p className="consent__trust">Codex runs hooks only after you trust them. Marshell will ask you to open <code>/hooks</code>: Trust these hooks in /hooks.</p>}
        </>
      ) : (
        <p className="consent__why">{edit.reason} Marshell leaves it alone. Fix the file and run the doctor, or skip.</p>
      )}
      {d.managed && <p className="consent__managed"><code>{d.managed.path}</code> is managed by your organisation. Marshell can't change it.</p>}
      <footer className="consent__foot"><button type="button" className="consent__skip">Skip: use limited status</button></footer>
    </article>
  );
}
```

with `CAP_LABEL = { status: "Show status and context", approve: "Approve from anywhere", statusline: "Keep my status line working" }`. Plan strings: `plan.removal = "Settings → Advanced → Remove all hooks restores the file byte for byte."` (the test checks "Remove all hooks"). The "A backup is saved first" sentence is part of the component. `Detected.tsx` renders `<ul>` rows `<li>` with mark, `CLI_NAME`, version chip, path in `<code dir="ltr">`, and a state sentence from `configState`/`managed` ("does not parse", "read-only", "managed by your organisation"). Use `EmptyState` for none.

Stories (`title: "Onboarding"`): `DetectedAll`, `DetectedOnlyClaude`, `DetectedNone`, `DetectedProblems` (unparsable, read-only, managed, in one list), `ConsentClaude`, `ConsentClaudeDiffOpen`, `ConsentCodexTrust`, `ConsentGeminiLimited`, `ConsentUnparsable`, `ConsentManaged`, `ConsentLongDiff` (200 lines), all in `ThemePair`.

- [ ] **Step 4: Run to verify they pass** — `pnpm vitest run && pnpm exec tsc --noEmit` → PASS.

- [ ] **Step 5: Commit** — `git add src/features/onboarding scripts/build-review.mjs && git commit -m "feat(onboarding): detected CLIs and the consent card" -m "<trailer>"`.

---

### Task 6: Onboarding screens 3 and 4, the frame, and every empty state

**Files:**
- Create: `src/features/onboarding/OnboardingFrame.tsx`, `NotificationsStep.tsx`, `FirstSession.tsx`, `emptyStates.ts`, `EmptyStates.stories.tsx`, `onboardingFlow.ts`, `onboardingFlow.test.ts`, `NotificationsStep.test.tsx`, `OnboardingFrame.test.tsx`, `emptyStates.test.ts`
- Modify: `Onboarding.css`, `Onboarding.stories.tsx`

**Interfaces:**
- Produces:
  - `onboardingFlow.ts`: `STEPS = ["detected","consent","notifications","first-session"] as const`; `type Step`; `stepNumber(step)`; `nextStep(step)` / `prevStep(step)` (clamped); `ONBOARDING_BUDGET_MS = 60_000`; `stepLabel(step): "Step 2 of 4: Consent"`.
  - `NotificationPermission = "not-asked" | "granted" | "denied"`; `<NotificationsStep permission os sounds />` with five sound rows (Needs you, Escalation, Done, Subagent done, Error) each with a `▶ Play` button named `Play <Name> sound`.
  - `<FirstSession />` wraps the existing `Launcher` pre-filled with the most recent git folder (read `src/features/launcher/Launcher.stories.tsx` story `RememberedForThisProject` for the props to pass; the sheet is shown inline, not as a modal).
  - `<OnboardingFrame step title primary secondary children />` (one primary button per step, a "Back" link on steps 2 to 4, `Step N of 4` text, a four-segment progress bar drawn with fills, **not** a live region).
  - `emptyStates.ts`: `EMPTY_STATES: Array<{ id: string; title: string; body: string; action?: string }>` for: `no-sessions`, `no-projects`, `no-history`, `no-agents`, `no-matches`, `doctor-clear`, `no-notifications`, `departures-empty`.

- [ ] **Step 1: Write the failing tests**

```ts
// onboardingFlow.test.ts
import { describe, expect, it } from "vitest";
import { nextStep, ONBOARDING_BUDGET_MS, prevStep, stepLabel, STEPS } from "./onboardingFlow";
describe("onboarding flow", () => {
  it("is four steps in the plan's order within a 60 second budget", () => {
    expect(STEPS).toEqual(["detected", "consent", "notifications", "first-session"]);
    expect(ONBOARDING_BUDGET_MS).toBe(60_000);
  });
  it("clamps at both ends", () => {
    expect(prevStep("detected")).toBe("detected");
    expect(nextStep("first-session")).toBe("first-session");
    expect(nextStep("detected")).toBe("consent");
  });
  it("labels a step with its number and name", () => {
    expect(stepLabel("consent")).toBe("Step 2 of 4: Consent");
  });
});
```

```tsx
// NotificationsStep.test.tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { NotificationsStep } from "./NotificationsStep";
describe("NotificationsStep", () => {
  it("previews every sound with a button named after it", () => {
    const h = renderToStaticMarkup(<NotificationsStep permission="not-asked" os="windows" />);
    for (const s of ["Needs you", "Escalation", "Done", "Subagent done", "Error"]) expect(h).toContain(`Play ${s} sound`);
    expect(h).toContain("Ask Windows to allow notifications");
  });
  it("says what still works when notifications are blocked, and where to turn them on", () => {
    const h = renderToStaticMarkup(<NotificationsStep permission="denied" os="windows" />);
    expect(h).toContain("Windows is blocking notifications for Marshell");
    expect(h).toContain("Settings");
    expect(h).toContain("the badge and the lane still work");
  });
  it("is quiet once allowed and says banners never raise the window", () => {
    const h = renderToStaticMarkup(<NotificationsStep permission="granted" os="windows" />);
    expect(h).toContain("Allowed");
    expect(h).toContain("A banner never takes focus from what you are typing in");
  });
  it("has no live region and no exclamation mark", () => {
    const h = renderToStaticMarkup(<NotificationsStep permission="granted" os="windows" />);
    expect(h).not.toMatch(/aria-live|role="status"|role="alert"|!/);
  });
});
```

```ts
// emptyStates.test.ts
import { describe, expect, it } from "vitest";
import { EMPTY_STATES } from "./emptyStates";
describe("EMPTY_STATES", () => {
  it("covers every empty state the plan lists", () => {
    expect(EMPTY_STATES.map((e) => e.id)).toEqual(["no-sessions", "no-projects", "no-history", "no-agents", "no-matches", "doctor-clear", "no-notifications", "departures-empty"]);
  });
  it("each says what is empty and, unless it is good news, how to start", () => {
    for (const e of EMPTY_STATES) {
      expect(e.title.length).toBeGreaterThan(0);
      if (e.id !== "doctor-clear" && e.id !== "no-notifications") expect(e.action, e.id).toBeTruthy();
    }
  });
  it("follows the copy rules", () => {
    for (const e of EMPTY_STATES) expect(`${e.title} ${e.body} ${e.action ?? ""}`).not.toMatch(/!|successfully|oops|the ai/i);
  });
  it("uses the plan's own sentence for no sessions", () => {
    expect(EMPTY_STATES.find((e) => e.id === "no-sessions")!.body).toBe("Start one with Ctrl+Shift+T.");
  });
});
```

`OnboardingFrame.test.tsx`: one primary button (`class="onboarding__primary"`), "Back" present on step 2 and absent on step 1, `Step 3 of 4` text present, `aria-current="step"` on the current progress segment, no live region.

- [ ] **Step 2: Run to verify they fail** — `pnpm vitest run src/features/onboarding` → FAIL (modules missing).

- [ ] **Step 3: Implement.** `onboardingFlow.ts`:

```ts
export const STEPS = ["detected", "consent", "notifications", "first-session"] as const;
export type Step = (typeof STEPS)[number];
export const ONBOARDING_BUDGET_MS = 60_000;
const NAMES: Record<Step, string> = { detected: "Detected", consent: "Consent", notifications: "Notifications", "first-session": "First session" };
export const stepNumber = (s: Step) => STEPS.indexOf(s) + 1;
export const nextStep = (s: Step): Step => STEPS[Math.min(STEPS.length - 1, STEPS.indexOf(s) + 1)]!;
export const prevStep = (s: Step): Step => STEPS[Math.max(0, STEPS.indexOf(s) - 1)]!;
export const stepLabel = (s: Step) => `Step ${stepNumber(s)} of ${STEPS.length}: ${NAMES[s]}`;
```

`emptyStates.ts` rows (exact strings): `no-sessions` "No sessions yet" / "Start one with Ctrl+Shift+T." / action "Start a session"; `no-projects` "No git folders found" / "Type a folder path, or choose a folder." / "Choose a folder"; `no-history` "No history yet" / "Finished sessions are kept here, with their transcript." / "Start a session"; `no-agents` "No coding agents found" / "Install Claude Code, Codex or Gemini, or continue with shells and tools." / "Continue with shells and tools"; `no-matches` "Nothing matches" / "Try fewer letters, or a session number." / "Clear the search"; `doctor-clear` "All clear" / "Nothing needs fixing." (no action); `no-notifications` "No banners yet" / "Banners appear here after the first one." (no action); `departures-empty` "No sessions on the board" / "Start one with Ctrl+Shift+T." / "Start a session". The Windows copy for `NotificationsStep` follows the tests (`Ask Windows to allow notifications`; `Windows is blocking notifications for Marshell. Turn them on in Settings → System → Notifications; the badge and the lane still work.`; `Allowed. A banner never takes focus from what you are typing in.`); for `mac`/`linux` swap the OS word and the settings path (design only, labelled in the story captions). Sound rows are plain list items with `<button type="button" aria-label="Play Needs you sound">▶ <span>Play</span></button>`... **keep label-in-name**: the visible text must be inside the name, so render the visible text as `Play Needs you sound` with the `▶` in an `aria-hidden` span.

`FirstSession.tsx` renders the `Launcher` exactly as the launcher stories do, pre-filled with `my-app · main` and the first-session hint "Enter starts your first session." Stories added to `Onboarding.stories.tsx`: `NotificationsNotAsked`, `NotificationsAllowed`, `NotificationsBlocked`, `NotificationsMac` (design only), `NotificationsLinux` (design only), `FirstSession`, `FrameStep1` to `FrameStep4` (the frame around each screen), and `EmptyStates.stories.tsx` with one `EmptyStates` story rendering every entry in a grid (each inside `EmptyState`) plus `ThemePair`.

- [ ] **Step 4: Run to verify they pass** — `pnpm vitest run && pnpm exec tsc --noEmit` → PASS.

- [ ] **Step 5: Commit** — `git add src/features/onboarding && git commit -m "feat(onboarding): notifications step, first session, frame and every empty state" -m "<trailer>"`.

---

### Task 7: Review page for sub-review A, keyboard and accessibility checks

**Files:**
- Modify: `src/design/review/renderReview.tsx`, `scripts/build-review.mjs`, `src/design/review/renderReview.test.tsx`, `src/styles/forcedColorsEdges.test.ts` (rows for `Onboarding.css`), `docs/a11y/screen-reader-script.md` (add a "Batch 3 A: onboarding" section)

- [ ] **Step 1: Failing test** (append to `renderReview.test.tsx`):

```tsx
describe("renderReview 3a content", () => {
  it("renders onboarding and the empty states", () => {
    const html = renderReview("3a");
    expect(html).toContain('id="onboarding"');
    expect(html).toContain('id="empty-states"');
    for (const name of ["Detected all", "Consent claude diff open", "Notifications blocked", "First session"]) expect(html, name).toContain(name);
  });
});
```

Run: `pnpm vitest run src/design/review` → FAIL (`BATCHES["3a"]` is empty).

- [ ] **Step 2: Implement.** In `renderReview.tsx` import `* as Onboarding from "../../features/onboarding/Onboarding.stories"`, `* as EmptyStates from "../../features/onboarding/EmptyStates.stories"`, `* as DiffStories from "../../components/DiffView/DiffView.stories"` and set `"3a": [[Onboarding, "<blurb>"], [EmptyStates, "<blurb>"], [DiffStories, "<blurb>"]]`. Blurbs (plain sentences): onboarding = "Four screens, under 60 seconds: what was found, what Marshell may change (shown as the exact diff, with the backup and the way out), notifications with a preview of each sound, and the first session. Nothing is changed without a yes, and a file that does not parse is never edited."; empty states = "Every empty state says what is empty and how to start."; diff = "The one diff view used for consent, the doctor's fixes and settings changes."

In `build-review.mjs` replace `META["3a"]` with:

```js
META["3a"] = {
  lede: "Sub-review A of 3 in batch 3: onboarding. Batch 2 (approve card, window, launcher, palette) is signed off and sits below.",
  nav: [["onboarding", "Onboarding"], ["empty-states", "Empty states"], ["diff-view", "Diff"], ["sign-off", "Sign-off"]],
  checks: [
    "In 60 seconds you can get from first launch to a running session, with the keyboard alone.",
    "Each consent card says in one sentence what changes and where; View exact changes shows the exact diff; the backup and the way to undo are named.",
    "A file that does not parse, is read-only or is managed is never offered for editing, and says why.",
    "Zero CLIs found still has a clear way forward; Gemini says it has no approval hook instead of showing a dead switch.",
    "Every empty state says what is empty and how to start.",
    "It still reads well at 150% browser zoom, in Windows High Contrast, and the focus ring is visible in both themes (docs/a11y/screen-reader-script.md has the Narrator and NVDA steps for this batch).",
  ],
};
```

Append the "Batch 3 A" section to `docs/a11y/screen-reader-script.md`: step list for (1) Detected: each row read as "Claude Code, version 2.1.296"; (2) a consent switch read as "Approve from anywhere, switch, on"; View exact changes announces `expanded`; (3) Play sound buttons; (4) the launcher.

- [ ] **Step 3: Verify.** Run `pnpm vitest run && pnpm exec tsc --noEmit`. Then build and check in the browser (the controller's usual loop): `pnpm review:build "$SP/marshell-design-review.html" 3a`, `cp` to `$TEMP/review.html`, open `http://127.0.0.1:6011/review.html?v=N`, run axe (exclude `.xterm`; ignore `landmark|html-has-lang|region|page-has-heading|document-title`) and the in-page scroll scan (`.window *`, `.onboarding *`: nothing may scroll), screenshot `#onboarding` in dark and light, Tab through one consent card (switches, View exact changes, Skip). Fix findings (each with a test), then run `pnpm a11y:matrix` checks through the MCP as in `docs/a11y/a11y-matrix.md`.
Expected: no axe findings; no scrolling containers; forced-colours and reduced-motion checks report 0 findings.

- [ ] **Step 4: Commit** — `git add -A src scripts docs && git commit -m "feat(review): sub-review A page for batch 3" -m "<trailer>"`.

**Controller step (not the implementer):** publish the page to the artifact URL as the next version, report to the owner, wait for sign-off on A, then continue.

---

## Sub-review B: doctor and settings

### Task 8: Doctor model: report, fix plan, undo and copy

**Files:**
- Create: `src/features/doctor/types.ts`, `report.ts`, `report.test.ts`, `undo.ts`, `undo.test.ts`, `fixtures.ts`, `fixtures.test.ts`

**Interfaces:**
- Consumes: `CliId` (Task 4), `unifiedDiff` (Task 2), `parseUnifiedDiff` (Task 2).
- Produces:

```ts
export type Severity = "error" | "warning" | "info";
export type DoctorItem = {
  id: string; cli: CliId | "marshell"; title: string; detail: string; severity: Severity;
  fixable: boolean; unfixableReason?: string;
  file?: string; diff?: string;
  /** Session numbers that only pick the fix up after a restart. */
  restartSessions?: number[];
};
export type DoctorReport = { items: DoctorItem[]; checkedAtMs: number; durationMs: number };
export type FixGroup = { file: string; items: DoctorItem[]; diff: string; added: number; removed: number };

export function counts(r: DoctorReport): { issues: number; fixable: number; unfixable: number };
export function fixHeadline(r: DoctorReport): string;               // "Fix 4 issues" | "Fix 1 issue" | "Nothing to fix"
export function groupByFile(items: DoctorItem[]): FixGroup[];       // fixable items with a file, in file order
export function restartSessionsOf(items: DoctorItem[]): number[];   // unique, ascending
export function listNumbers(n: number[]): string;                    // "3" | "3 and 5" | "1, 2 and 3"
export function afterFixMessage(fixed: number, restart: number[]): string;
export const TOAST_MS = 10_000;
export type UndoState = { fixedAtMs: number; count: number; filesMatch: boolean };
export function toastVisible(u: UndoState, nowMs: number): boolean;
export function undoOffer(u: UndoState, nowMs: number): { kind: "toast" | "header" | "stale"; text: string };
```

- [ ] **Step 1: Write the failing tests**

```ts
// report.test.ts
import { describe, expect, it } from "vitest";
import { afterFixMessage, counts, fixHeadline, groupByFile, listNumbers, restartSessionsOf } from "./report";
import { DOCTOR_CLEAR, DOCTOR_PROBLEMS, DOCTOR_REPORT } from "./fixtures";

describe("counts", () => {
  it("counts issues, not information, and splits fixable from not", () => {
    expect(counts(DOCTOR_REPORT)).toEqual({ issues: 4, fixable: 4, unfixable: 0 });
    expect(counts(DOCTOR_PROBLEMS)).toMatchObject({ unfixable: 3, fixable: 0 });
    expect(counts(DOCTOR_CLEAR)).toEqual({ issues: 0, fixable: 0, unfixable: 0 });
  });
});
describe("fixHeadline", () => {
  it("pluralises and says when there is nothing to fix", () => {
    expect(fixHeadline(DOCTOR_REPORT)).toBe("Fix 4 issues");
    expect(fixHeadline({ ...DOCTOR_REPORT, items: DOCTOR_REPORT.items.slice(0, 1) })).toBe("Fix 1 issue");
    expect(fixHeadline(DOCTOR_PROBLEMS)).toBe("Nothing to fix");
  });
});
describe("groupByFile", () => {
  it("groups fixable items by file with one combined diff and its counts", () => {
    const g = groupByFile(DOCTOR_REPORT.items);
    expect(g.map((x) => x.file)).toEqual(["C:\\Users\\Home Office\\.claude\\settings.json"]);
    expect(g[0]!.items).toHaveLength(4);
    expect(g[0]!.added).toBeGreaterThan(0);
  });
  it("never includes an item that cannot be fixed or has no file", () => {
    expect(groupByFile(DOCTOR_PROBLEMS.items)).toEqual([]);
  });
});
describe("restart wording", () => {
  it("lists session numbers in words", () => {
    expect(listNumbers([3])).toBe("3");
    expect(listNumbers([3, 5])).toBe("3 and 5");
    expect(listNumbers([1, 2, 3])).toBe("1, 2 and 3");
  });
  it("unions and sorts the sessions that need a restart", () => {
    expect(restartSessionsOf(DOCTOR_REPORT.items)).toEqual([3, 5]);
  });
  it("words the result as the plan does, singular and plural, with or without a restart", () => {
    expect(afterFixMessage(4, [3, 5])).toBe("Fixed 4 issues. 2 sessions pick this up after a restart.");
    expect(afterFixMessage(1, [3])).toBe("Fixed 1 issue. 1 session picks this up after a restart.");
    expect(afterFixMessage(2, [])).toBe("Fixed 2 issues.");
  });
  it("never uses the banned words", () => {
    expect(afterFixMessage(4, [3, 5])).not.toMatch(/!|successfully|oops/i);
  });
});
```

```ts
// undo.test.ts
import { describe, expect, it } from "vitest";
import { TOAST_MS, toastVisible, undoOffer, type UndoState } from "./undo";

const u: UndoState = { fixedAtMs: 1_000, count: 4, filesMatch: true };
describe("undo", () => {
  it("shows the toast for 10 seconds, then leaves Undo last fix in the header", () => {
    expect(TOAST_MS).toBe(10_000);
    expect(toastVisible(u, 1_000 + 9_999)).toBe(true);
    expect(toastVisible(u, 1_000 + 10_000)).toBe(false);
    expect(undoOffer(u, 2_000)).toEqual({ kind: "toast", text: "Undo" });
    expect(undoOffer(u, 20_000)).toEqual({ kind: "header", text: "Undo last fix" });
  });
  it("stops offering undo when the files no longer match, and says why", () => {
    expect(undoOffer({ ...u, filesMatch: false }, 20_000)).toEqual({ kind: "stale", text: "A file changed since the fix, so undo would overwrite it. Show differences." });
  });
});
```

```ts
// fixtures.test.ts
import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../../components/DiffView/diff";
import { DOCTOR_PROBLEMS, DOCTOR_REPORT } from "./fixtures";
describe("doctor fixtures", () => {
  it("every fixable item has a file and a diff that parses", () => {
    for (const i of DOCTOR_REPORT.items.filter((x) => x.fixable)) {
      expect(i.file, i.id).toContain("Home Office");
      expect(parseUnifiedDiff(i.diff!).added, i.id).toBeGreaterThan(0);
    }
  });
  it("every unfixable item says why, in words", () => {
    for (const i of DOCTOR_PROBLEMS.items) expect(i.unfixableReason, i.id).toBeTruthy();
  });
  it("includes the managed settings as information that is never fixable", () => {
    const m = DOCTOR_REPORT.items.find((i) => i.severity === "info")!;
    expect(m.fixable).toBe(false);
    expect(m.detail).toContain("never changes");
  });
});
```

- [ ] **Step 2: Run to verify they fail** — `pnpm vitest run src/features/doctor` → FAIL (modules missing).

- [ ] **Step 3: Implement.**

`report.ts`:

```ts
import { parseUnifiedDiff } from "../../components/DiffView/diff";
import type { DoctorItem, DoctorReport, FixGroup } from "./types";
export * from "./types";

const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

export function counts(r: DoctorReport) {
  const issues = r.items.filter((i) => i.severity !== "info");
  const fixable = issues.filter((i) => i.fixable).length;
  return { issues: issues.length, fixable, unfixable: issues.length - fixable };
}
export function fixHeadline(r: DoctorReport): string {
  const n = counts(r).fixable;
  return n === 0 ? "Nothing to fix" : `Fix ${n} ${plural(n, "issue", "issues")}`;
}
export function groupByFile(items: DoctorItem[]): FixGroup[] {
  const by = new Map<string, DoctorItem[]>();
  for (const i of items) if (i.fixable && i.file && i.diff) by.set(i.file, [...(by.get(i.file) ?? []), i]);
  return [...by].map(([file, its]) => {
    const diff = its.map((i) => i.diff!).join("");
    const p = parseUnifiedDiff(diff);
    return { file, items: its, diff, added: p.added, removed: p.removed };
  });
}
export const restartSessionsOf = (items: DoctorItem[]) => [...new Set(items.flatMap((i) => i.restartSessions ?? []))].sort((a, b) => a - b);
export function listNumbers(n: number[]): string {
  if (n.length <= 1) return n.join("");
  return `${n.slice(0, -1).join(", ")} and ${n[n.length - 1]}`;
}
export function afterFixMessage(fixed: number, restart: number[]): string {
  const base = `Fixed ${fixed} ${plural(fixed, "issue", "issues")}.`;
  if (restart.length === 0) return base;
  return `${base} ${restart.length} ${plural(restart.length, "session picks", "sessions pick")} this up after a restart.`;
}
```

`undo.ts`:

```ts
export const TOAST_MS = 10_000;
export type UndoState = { fixedAtMs: number; count: number; filesMatch: boolean };
export const toastVisible = (u: UndoState, nowMs: number) => nowMs - u.fixedAtMs < TOAST_MS;
export function undoOffer(u: UndoState, nowMs: number): { kind: "toast" | "header" | "stale"; text: string } {
  if (!u.filesMatch) return { kind: "stale", text: "A file changed since the fix, so undo would overwrite it. Show differences." };
  return toastVisible(u, nowMs) ? { kind: "toast", text: "Undo" } : { kind: "header", text: "Undo last fix" };
}
```

`fixtures.ts`: build `DOCTOR_REPORT` (four fixable items on `C:\Users\Home Office\.claude\settings.json`; diffs built with `unifiedDiff` from small `before`/`after` JSON texts; items: `claude-permission-hook` "Approve from anywhere is not installed" (restart `[3]`), `claude-statusline-chain` "Your status line is no longer chained" (restart `[3, 5]`), `claude-cleanup-period` "Claude deletes transcripts after 30 days" (raise to 365; detail says Marshell keeps its own copy, raising it keeps Claude's own resume working), `duplicate-notifications` "Two notifications for one request" (illustrative diff; comment that real keys are verified against the CLI's docs in phase 7), plus the info item `managed-present` ("Your organisation manages some Claude settings in C:\Program Files\ClaudeCode\managed-settings.json. Marshell lists them and never changes them.")); `DOCTOR_PROBLEMS` (three unfixable: `codex-hooks-untrusted` warning with reason "Marshell can't trust hooks for you. Open /hooks in Codex and trust them.", `gemini-unparsable` error with reason "The file does not parse (line 14). Marshell never edits a file it can't read.", `codex-readonly` warning "The file is read-only."); `DOCTOR_CLEAR` (empty items); `UNDO_FIXED: UndoState`.

- [ ] **Step 4: Run to verify they pass** — `pnpm vitest run src/features/doctor && pnpm exec tsc --noEmit` → PASS.

- [ ] **Step 5: Commit** — `git add src/features/doctor && git commit -m "feat(doctor): report, fix plan, undo state and copy" -m "<trailer>"`.

---

### Task 9: Doctor page, fix sheet and undo toast

**Files:**
- Create: `src/features/doctor/DoctorPage.tsx`, `DoctorRow.tsx`, `FixSheet.tsx`, `UndoToast.tsx`, `Doctor.css`, `doctorUi.ts` (`showProgress`), `DoctorPage.test.tsx`, `FixSheet.test.tsx`, `Doctor.css.test.ts`, `Doctor.stories.tsx`
- Modify: `src/components/a11y/announce-policy.ts` (+ its test), `src/features/window/scenario.ts` (+ affected tests), `scripts/build-review.mjs` (CSS list), `src/styles/forcedColorsEdges.test.ts`

**Interfaces:**
- Consumes: Task 8 (`counts`, `fixHeadline`, `groupByFile`, `restartSessionsOf`, `afterFixMessage`, `undoOffer`, `toastVisible`), `DiffView`, `Page`, `EmptyState`, `Chip`, `StatusGlyph`, `SessionNumber`.
- Produces: `<DoctorPage report phase undo? nowMs? elapsedMs? failure? />` with `phase: "running" | "ready" | "fixing" | "fixed" | "failed"`; `<FixSheet report />`; `<UndoToast undo nowMs />`; `showProgress(elapsedMs) = elapsedMs > 150`; `doctorAnnouncement(message: string): AnnounceKind` usage described below.
- Announcer: add `"action-result"` to `AnnounceKind` in `announce-policy.ts` and make `shouldAnnounce("action-result", mode)` true for every mode (a result the user caused is always said once, politely). Test in `announce-policy.test.ts`: `shouldAnnounce("action-result", "needs-you")` and `"on-request"` are both `true`.
- `scenario.ts`: `SCENARIO_FOOTER.doctorIssues` becomes `counts(DOCTOR_REPORT).issues` (4); update the tests that cited 2.

- [ ] **Step 1: Write the failing tests**

```tsx
// DoctorPage.test.tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { DoctorPage } from "./DoctorPage";
import { showProgress } from "./doctorUi";
import { DOCTOR_CLEAR, DOCTOR_PROBLEMS, DOCTOR_REPORT, UNDO_FIXED } from "./fixtures";

describe("DoctorPage", () => {
  it("ready: counts the issues and offers the fix and a re-run", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="ready" />);
    expect(h).toContain("4 issues");
    expect(h).toContain("Fix 4 issues");
    expect(h).toContain("Run again");
  });
  it("tells every severity in words as well as a glyph, and says why a problem cannot be fixed", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_PROBLEMS} phase="ready" />);
    expect(h).toMatch(/Error/);
    expect(h).toMatch(/Warning/);
    expect(h).toContain("Marshell never edits a file it can't read.");
    expect(h).not.toContain("Fix 0");
    expect(h).toContain("Nothing to fix");
  });
  it("lists information (managed settings) with no fix control", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="ready" />);
    expect(h).toContain("managed-settings.json");
    expect(h).toMatch(/Information/);
  });
  it("running: says what is being checked and is busy, with no live region", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="running" elapsedMs={400} />);
    expect(h).toContain("Checking Claude Code, Codex and Gemini");
    expect(h).toContain('aria-busy="true"');
    expect(h).not.toMatch(/aria-live|role="status"|role="alert"/);
  });
  it("shows nothing for the first 150 ms", () => {
    expect(showProgress(150)).toBe(false);
    expect(showProgress(151)).toBe(true);
  });
  it("all clear: says so and offers only Run again", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_CLEAR} phase="ready" />);
    expect(h).toContain("All clear");
    expect(h).not.toContain("Fix ");
  });
  it("failed: gives the reason and a way to run it again", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="failed" failure="The core is not running." />);
    expect(h).toContain("The doctor could not run: The core is not running.");
    expect(h).toContain("Run again");
  });
  it("fixed: ticks each row, shows the receipt toast with Restart them and Undo, and keeps Undo last fix in the header afterwards", () => {
    const toast = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="fixed" undo={UNDO_FIXED} nowMs={UNDO_FIXED.fixedAtMs + 2_000} />);
    expect(toast).toContain("Fixed 4 issues. 2 sessions pick this up after a restart.");
    expect(toast).toContain("Restart them");
    expect((toast.match(/data-state="fixed"/g) ?? []).length).toBe(4);
    const later = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="fixed" undo={UNDO_FIXED} nowMs={UNDO_FIXED.fixedAtMs + 60_000} />);
    expect(later).not.toContain("Restart them");
    expect(later).toContain("Undo last fix");
  });
  it("a stale undo says why and shows no undo button", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="fixed" undo={{ ...UNDO_FIXED, filesMatch: false }} nowMs={UNDO_FIXED.fixedAtMs + 60_000} />);
    expect(h).toContain("A file changed since the fix");
    expect(h).not.toContain("Undo last fix");
  });
  it("names its buttons from the text they show", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="ready" />);
    for (const b of h.match(/<button[\s\S]*?<\/button>/g) ?? []) expect(nameContainsLabel(b)).toBe(true);
  });
  it("names the sessions that need a restart by number", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="ready" />);
    expect(h).toContain("Sessions 3 and 5 pick this up after a restart");
  });
});
```

```tsx
// FixSheet.test.tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FixSheet } from "./FixSheet";
import { DOCTOR_REPORT } from "./fixtures";

describe("FixSheet", () => {
  const h = renderToStaticMarkup(<FixSheet report={DOCTOR_REPORT} />);
  it("shows the combined diff grouped by file, with the backup and the way back", () => {
    expect(h).toContain("C:\\Users\\Home Office\\.claude\\settings.json");
    expect(h).toContain("diff__lines");
    expect(h).toContain("A backup is saved first");
    expect(h).toContain("Undo last fix");
  });
  it("has Cancel and the fix as its two buttons, and the fix names the count", () => {
    expect(h).toContain(">Cancel<");
    expect(h).toContain(">Fix 4 issues<");
  });
  it("is a dialog labelled by its title and takes focus on the title, not on an action", () => {
    expect(h).toContain('role="dialog"');
    expect(h).toContain('tabindex="-1"');
    expect(h).not.toMatch(/autofocus/i);
  });
  it("names the sessions that will need a restart", () => {
    expect(h).toContain("Sessions 3 and 5");
  });
});
```

`Doctor.css.test.ts`: asserts no `border` besides radius/start bar; severity rows use fill and a start-edge bar (`border-inline-start`) plus forced-colours outline; `.doctor__row[data-state="fixed"]` has a visible tick (text "Fixed") not colour only; `.toast` has `box-shadow: var(--shadow-pill)`, an 80 ms fade under `prefers-reduced-motion: reduce` and enter animation only under `no-preference`; no `overflow: auto|scroll`.

- [ ] **Step 2: Run to verify they fail** — `pnpm vitest run src/features/doctor src/components/a11y` → FAIL.

- [ ] **Step 3: Implement.**

Behaviour and structure:
- `DoctorPage` renders `Page title="Doctor"` with `subtitle` = `Checked ${n} s ago` (from `nowMs - report.checkedAtMs`, or `–` when unknown) and actions: `Fix N issues` (primary, only when `counts.fixable > 0` and phase `ready`), `Run again`, and the header `Undo last fix` link when `undoOffer(...).kind === "header"`.
- Rows: `DoctorRow` is an `<li>` with `data-severity`, `data-state` (`"fixed"` after the fix), a glyph (`StatusGlyph kind="error"` for error, `AuxGlyph kind="caution"` for warning, a plain `–`-style info mark for information) **and** the severity word (`Error`, `Warning`, `Information`), title, detail, file path in `<code>`, a restart note ("Sessions 3 and 5 pick this up after a restart" with `SessionNumber` for each), and for unfixable items the `unfixableReason`.
- `running`: `aria-busy="true"` on the body and, only when `showProgress(elapsedMs)`, a one-line "Checking Claude Code, Codex and Gemini…".
- `ready` with zero issues: `EmptyState` from `EMPTY_STATES["doctor-clear"]`.
- `failed`: `<p>The doctor could not run: {failure}</p>` + `Run again`.
- `fixed`: rows `data-state="fixed"` with the text "Fixed" in place of the severity word; `UndoToast` while `toastVisible`; header Undo otherwise; stale shows the `stale` text with a "Show differences" button.
- `UndoToast`: a plain block (`.toast`, fill + `--shadow-pill`, no live region) with the `afterFixMessage(count, restart)` text and the buttons `Restart them` and `Undo`. The text is also what the window announces: export `fixAnnouncement(undo, restart)` from `doctorUi.ts` returning `{ text, kind: "action-result" }` (unit-test it).
- `FixSheet`: `<section role="dialog" aria-modal="true" aria-labelledby>` inline, `<h2 tabindex="-1">Fix 4 issues</h2>`, one `DiffView` per `groupByFile` group (`label={`Changes to ${file}`}`), the sentence "A backup is saved first, in `~/.marshell/snapshots/`. To undo: Undo last fix in the Doctor header, for as long as the file still matches." and the restart note, then the two buttons.
- `scenario.ts`: `doctorIssues: counts(DOCTOR_REPORT).issues`.
Stories (`title: "Doctor"`): `Ready`, `ReadyProblems` (unfixable items), `AllClear`, `Running`, `Failed`, `FixSheet`, `FixSheetLong` (200-line diff), `Fixed` (toast visible), `FixedLater` (header Undo), `UndoStale`, `RtlFilePath`.

- [ ] **Step 4: Run to verify they pass** — `pnpm vitest run && pnpm exec tsc --noEmit` → PASS.

- [ ] **Step 5: Commit** — `git add -A src scripts && git commit -m "feat(doctor): report page, fix sheet and undo toast" -m "<trailer>"`.

---

### Task 10: Settings model: scopes, fields, raw parsing and change preview

**Files:**
- Create: `src/features/settings/scope.ts`, `scope.test.ts`, `fields.ts`, `fields.test.ts`, `raw.ts`, `raw.test.ts`, `edit.ts`, `edit.test.ts`, `fixtures.ts`

**Interfaces:**
- Consumes: `unifiedDiff` (Task 2), `CliId` (Task 4).
- Produces:

```ts
export type Scope = "user" | "project" | "local" | "managed" | "marshell";
export const SCOPES: Scope[]; export const SCOPE_LABEL: Record<Scope, string>; export const SCOPE_NOTE: Record<Scope, string>;
export const SCOPE_SHAPE: Record<Scope, "circle" | "square" | "diamond" | "lock" | "flag">; // shape so a scope never relies on colour
export type FieldKind = "boolean" | "enum" | "number" | "text";
export type SettingField = { key: string; label: string; help: string; kind: FieldKind; options?: string[]; value?: unknown; scope: Scope; lockedBy?: "managed"; applies: "live" | "new-sessions" | "restart" };
export type FileState = { readOnly?: boolean; unparsable?: boolean };
export function isEditable(f: SettingField, file?: FileState): { ok: boolean; reason?: string };
export function appliesNote(f: SettingField): string;
export type RawResult = { ok: true; value: Record<string, unknown> } | { ok: false; line: number; column: number; message: string };
export function parseRaw(text: string): RawResult;
export function applyChanges(beforeText: string, changes: Record<string, unknown>): string;
export function previewChange(path: string, beforeText: string, changes: Record<string, unknown>): string;
export function changeSummary(n: number): string;      // "No changes" | "1 change" | "2 changes"
export function pendingChanges(fields: SettingField[], edits: Record<string, unknown>): Record<string, unknown>;
export function fileChanged(openedText: string, currentText: string): boolean;
```

- [ ] **Step 1: Write the failing tests**

```ts
// scope.test.ts
import { describe, expect, it } from "vitest";
import { isReadOnlyScope, SCOPE_LABEL, SCOPE_NOTE, SCOPE_SHAPE, SCOPES } from "./scope";
describe("scopes", () => {
  it("has the five scopes, each with a word, a sentence and a shape", () => {
    expect(SCOPES).toEqual(["user", "project", "local", "managed", "marshell"]);
    expect(SCOPES.map((s) => SCOPE_LABEL[s])).toEqual(["User", "Project", "Local", "Managed", "Marshell"]);
    expect(new Set(SCOPES.map((s) => SCOPE_SHAPE[s])).size).toBe(5);
    expect(SCOPE_NOTE.managed).toBe("Set by your organisation. Marshell can't change it.");
  });
  it("only managed is read-only", () => {
    expect(SCOPES.filter(isReadOnlyScope)).toEqual(["managed"]);
  });
});
```

```ts
// fields.test.ts
import { describe, expect, it } from "vitest";
import { appliesNote, isEditable, type SettingField } from "./fields";
const f = (o: Partial<SettingField> = {}): SettingField => ({ key: "theme", label: "Theme", help: "", kind: "enum", scope: "user", applies: "live", ...o });
describe("isEditable", () => {
  it("allows an ordinary field", () => expect(isEditable(f())).toEqual({ ok: true }));
  it("locks a managed field and says who set it", () => {
    expect(isEditable(f({ scope: "managed" }))).toEqual({ ok: false, reason: "Set by your organisation. Marshell can't change it." });
    expect(isEditable(f({ lockedBy: "managed" })).ok).toBe(false);
  });
  it("locks everything in a read-only or unparsable file, with the reason", () => {
    expect(isEditable(f(), { readOnly: true })).toEqual({ ok: false, reason: "This file is read-only, so Marshell can't save changes." });
    expect(isEditable(f(), { unparsable: true })).toEqual({ ok: false, reason: "This file does not parse, so Marshell won't edit it." });
  });
});
describe("appliesNote", () => {
  it("says when a change takes effect", () => {
    expect(appliesNote(f({ applies: "live" }))).toBe("Applies at once.");
    expect(appliesNote(f({ applies: "new-sessions" }))).toBe("Applies to new sessions.");
    expect(appliesNote(f({ applies: "restart" }))).toBe("Applies after you restart the session.");
  });
});
```

```ts
// raw.test.ts
import { describe, expect, it } from "vitest";
import { parseRaw } from "./raw";
describe("parseRaw", () => {
  it("returns the object for valid JSON", () => expect(parseRaw('{"a":1}')).toEqual({ ok: true, value: { a: 1 } }));
  it("names the line and column of a mistake", () => {
    const r = parseRaw('{\n  "a": 1,\n  "b": }\n');
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.line).toBe(3);
      expect(r.column).toBeGreaterThan(0);
      expect(r.message.length).toBeGreaterThan(0);
    }
  });
  it("refuses a value that is not an object, and an empty file", () => {
    expect(parseRaw("[1]").ok).toBe(false);
    expect(parseRaw("").ok).toBe(false);
  });
});
```

```ts
// edit.test.ts
import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../../components/DiffView/diff";
import { applyChanges, changeSummary, fileChanged, pendingChanges, previewChange } from "./edit";
import type { SettingField } from "./fields";

const before = '{\n  "model": "opus",\n  "cleanupPeriodDays": 30\n}\n';
describe("edit", () => {
  it("sets keys and keeps the rest and the order", () => {
    expect(JSON.parse(applyChanges(before, { cleanupPeriodDays: 365 }))).toEqual({ model: "opus", cleanupPeriodDays: 365 });
    expect(Object.keys(JSON.parse(applyChanges(before, { theme: "dark" })))).toEqual(["model", "cleanupPeriodDays", "theme"]);
  });
  it("previews the exact change as a diff of the whole file", () => {
    const p = parseUnifiedDiff(previewChange("C:/Users/Home Office/.claude/settings.json", before, { cleanupPeriodDays: 365 }));
    expect(p.added).toBe(1);
    expect(p.removed).toBe(1);
    expect(p.files).toEqual(["C:/Users/Home Office/.claude/settings.json"]);
  });
  it("previews nothing when nothing changes", () => expect(previewChange("a.json", before, { model: "opus" })).toBe(""));
  it("only counts a field whose value really changed", () => {
    const fields = [{ key: "a", value: 1 }, { key: "b", value: "x" }] as SettingField[];
    expect(pendingChanges(fields, { a: 1, b: "y" })).toEqual({ b: "y" });
  });
  it("words the count", () => {
    expect([0, 1, 2].map(changeSummary)).toEqual(["No changes", "1 change", "2 changes"]);
  });
  it("notices that the file changed on disk after it was opened", () => {
    expect(fileChanged(before, before)).toBe(false);
    expect(fileChanged(before, before + " ")).toBe(true);
  });
});
```

- [ ] **Step 2: Run to verify they fail** — `pnpm vitest run src/features/settings` → FAIL (modules missing).

- [ ] **Step 3: Implement.**

```ts
// scope.ts
export type Scope = "user" | "project" | "local" | "managed" | "marshell";
export const SCOPES: Scope[] = ["user", "project", "local", "managed", "marshell"];
export const SCOPE_LABEL: Record<Scope, string> = { user: "User", project: "Project", local: "Local", managed: "Managed", marshell: "Marshell" };
export const SCOPE_NOTE: Record<Scope, string> = {
  user: "Applies to every project on this computer.",
  project: "Applies to this project and is shared through git.",
  local: "Applies to this project on this computer only.",
  managed: "Set by your organisation. Marshell can't change it.",
  marshell: "Marshell's own setting, kept in ~/.marshell/settings.json.",
};
export const SCOPE_SHAPE: Record<Scope, "circle" | "square" | "diamond" | "lock" | "flag"> = { user: "circle", project: "square", local: "diamond", managed: "lock", marshell: "flag" };
export const isReadOnlyScope = (s: Scope) => s === "managed";
```

```ts
// fields.ts
import { isReadOnlyScope, SCOPE_NOTE, type Scope } from "./scope";
export type FieldKind = "boolean" | "enum" | "number" | "text";
export type SettingField = { key: string; label: string; help: string; kind: FieldKind; options?: string[]; value?: unknown; scope: Scope; lockedBy?: "managed"; applies: "live" | "new-sessions" | "restart" };
export type FileState = { readOnly?: boolean; unparsable?: boolean };
export function isEditable(f: SettingField, file: FileState = {}): { ok: boolean; reason?: string } {
  if (isReadOnlyScope(f.scope) || f.lockedBy) return { ok: false, reason: SCOPE_NOTE.managed };
  if (file.unparsable) return { ok: false, reason: "This file does not parse, so Marshell won't edit it." };
  if (file.readOnly) return { ok: false, reason: "This file is read-only, so Marshell can't save changes." };
  return { ok: true };
}
export const appliesNote = (f: SettingField) => ({ live: "Applies at once.", "new-sessions": "Applies to new sessions.", restart: "Applies after you restart the session." })[f.applies];
```

`raw.ts`: `parseRaw` uses `JSON.parse` in try/catch; for a failure it reads the error's `position N` (V8 message `... at position 31`) and converts that index to line and column against the text (`line = count("\n" before index) + 1`, `column = index - lastNewline`); if no position is found it reports line 1, column 1. A non-object result or empty text returns `{ ok: false, line: 1, column: 1, message: "The file must hold one JSON object." }`. (Phase 3 swaps this for the core's jsonc CST; raw mode in phase 0 is strict JSON, noted in the comment.)

`edit.ts`: `applyChanges` = `JSON.stringify({ ...JSON.parse(before), ...changes }, null, 2) + "\n"` (comment: phase 3 uses CST edits so formatting survives; the UI only ever shows the diff the core returns); `previewChange` = `unifiedDiff(path, before, applyChanges(before, changes))`; `pendingChanges` compares `JSON.stringify(a) !== JSON.stringify(b)`; `changeSummary` as tested; `fileChanged = (a, b) => a !== b`.

`fixtures.ts`: `SETTINGS_FIELDS` (12 fields: General: theme enum `follow-system|dark|light` scope marshell live; announce mode enum `needs-you|everything|on-request` marshell live, with only `needs-you` selectable noted in `help`; start on login boolean; Notifications: sounds boolean, quiet hours text, subagent sounds boolean off; Terminal: font size number, terminal theme enum, contrast protection boolean; Claude Code: model enum user new-sessions, `cleanupPeriodDays` number user new-sessions value 30, default permission mode enum with `lockedBy: "managed"` and scope user), `SETTINGS_NAV: Array<{ id: string; label: string }>` (General, Notifications, Terminal, Claude Code, Codex, Gemini, Advanced), `SETTINGS_TEXT` (the Claude `settings.json` text with `model` and `cleanupPeriodDays`, a 14-line object), `SETTINGS_TEXT_BROKEN` (line 3 broken), `SETTINGS_TEXT_LONG` (a hooks object producing a 200-line diff when `applyChanges` adds nine entries).

- [ ] **Step 4: Run to verify they pass** — `pnpm vitest run src/features/settings && pnpm exec tsc --noEmit` → PASS.

- [ ] **Step 5: Commit** — `git add src/features/settings && git commit -m "feat(settings): scopes, fields, raw parsing and change preview" -m "<trailer>"`.

---

### Task 11: Settings page and editor: form mode, raw mode, scope badges

**Files:**
- Create: `src/features/settings/ScopeBadge.tsx`, `FieldRow.tsx`, `SettingsForm.tsx`, `RawEditor.tsx`, `ChangeBar.tsx`, `SettingsPage.tsx`, `Settings.css`, `settingsUi.ts`, tests `ScopeBadge.test.tsx`, `SettingsPage.test.tsx`, `RawEditor.test.tsx`, `Settings.css.test.ts`, stories `Settings.stories.tsx`
- Modify: `scripts/build-review.mjs` (CSS list), `src/styles/forcedColorsEdges.test.ts`

**Interfaces:**
- Consumes: Task 10 (all), `DiffView`, `Page`, `Switch`, `EmptyState`, `selectedProps`.
- Produces: `<ScopeBadge scope />`; `<SettingsPage category mode state edits? opened? />` where `mode: "form" | "raw"`, `state: { readOnly?; unparsable?; conflict?; changed?: boolean; reviewing?: boolean }`; `settingsAnnouncement(state, changes): { text: string; kind: "action-result" } | null`.

- [ ] **Step 1: Write the failing tests**

```tsx
// ScopeBadge.test.tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SCOPES } from "./scope";
import { ScopeBadge } from "./ScopeBadge";
describe("ScopeBadge", () => {
  it("says the scope in a word and draws its shape, and explains it on hover", () => {
    for (const s of SCOPES) {
      const h = renderToStaticMarkup(<ScopeBadge scope={s} />);
      expect(h).toContain(`data-scope="${s}"`);
      expect(h).toMatch(/<svg/);
      expect(h).toContain("title=");
    }
    expect(renderToStaticMarkup(<ScopeBadge scope="managed" />)).toContain("Managed");
  });
});
```

```tsx
// SettingsPage.test.tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { SettingsPage } from "./SettingsPage";
import { SETTINGS_TEXT, SETTINGS_TEXT_BROKEN, SETTINGS_TEXT_LONG } from "./fixtures";

const base = { category: "claude", mode: "form" as const, state: {} };
describe("SettingsPage: form mode", () => {
  it("shows every field with its label, its scope badge, what it affects and when", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} />);
    expect(h).toContain("Delete transcripts after (days)");
    expect(h).toContain('data-scope="user"');
    expect(h).toContain("Applies to new sessions.");
  });
  it("locks a managed field, says who set it and gives it no editable control", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} />);
    expect(h).toContain("Set by your organisation. Marshell can't change it.");
    expect(h).toMatch(/data-locked="true"[\s\S]*?disabled/);
  });
  it("marks the selected category with aria-current, in a nav", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} />);
    expect(h).toMatch(/<nav[^>]*aria-label="Settings sections"/);
    expect(h).toMatch(/aria-current="true"[^>]*>[\s\S]*?Claude Code/);
  });
  it("has the Edit as radio group with Form and Raw", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} />);
    expect(h).toContain('role="radiogroup"');
    expect(h).toContain("Edit as");
    expect(h.match(/type="radio"/g)).toHaveLength(2);
  });
});

describe("SettingsPage: changes", () => {
  it("counts the changes, then shows the exact diff, the backup and the way to undo before Apply", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ changed: true, reviewing: true }} edits={{ cleanupPeriodDays: 365 }} opened={SETTINGS_TEXT} />);
    expect(h).toContain("1 change");
    expect(h).toContain("diff__lines");
    expect(h).toContain("A backup is saved first");
    expect(h).toContain("Settings → Advanced → Snapshots");
    expect(h).toContain(">Apply<");
    expect(h).toContain(">Discard<");
  });
  it("keeps a 200-line change inside the page (collapsed), not the window", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ changed: true, reviewing: true }} edits={{ hooks: Array.from({ length: 200 }, (_, i) => `h${i}`) }} opened={SETTINGS_TEXT_LONG} />);
    expect(h).toContain("Show all");
  });
  it("has no Apply when nothing changed", () => {
    expect(renderToStaticMarkup(<SettingsPage {...base} />)).not.toContain(">Apply<");
  });
});

describe("SettingsPage: unsafe files", () => {
  it("read-only: says so and disables editing", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ readOnly: true }} />);
    expect(h).toContain("This file is read-only, so Marshell can't save changes.");
    expect(h).not.toContain(">Apply<");
  });
  it("unparsable: fails closed, names the line, and leaves the form off", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ unparsable: true }} opened={SETTINGS_TEXT_BROKEN} mode="raw" />);
    expect(h).toContain("This file does not parse, so Marshell won't edit it.");
    expect(h).toMatch(/Line 3/);
  });
  it("conflict: says the file changed on disk and offers three ways forward", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ conflict: true, changed: true }} />);
    expect(h).toContain("The file changed on disk since you opened it.");
    for (const b of ["Show differences", "Keep mine", "Reload"]) expect(h).toContain(`>${b}<`);
  });
});

describe("SettingsPage: accessibility", () => {
  it("has no live region of its own and no banned words", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ changed: true, reviewing: true }} edits={{ cleanupPeriodDays: 365 }} opened={SETTINGS_TEXT} />);
    expect(h).not.toMatch(/aria-live|role="status"|role="alert"/);
    expect(h).not.toMatch(/successfully|oops|!/i);
  });
  it("names its buttons from the text they show", () => {
    const h = renderToStaticMarkup(<SettingsPage {...base} state={{ changed: true, reviewing: true }} edits={{ cleanupPeriodDays: 365 }} opened={SETTINGS_TEXT} />);
    for (const b of h.match(/<button[\s\S]*?<\/button>/g) ?? []) expect(nameContainsLabel(b)).toBe(true);
  });
});
```

```tsx
// RawEditor.test.tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { RawEditor } from "./RawEditor";
import { SETTINGS_TEXT, SETTINGS_TEXT_BROKEN } from "./fixtures";
describe("RawEditor", () => {
  it("is a labelled monospace text area holding the file text", () => {
    const h = renderToStaticMarkup(<RawEditor path="C:\Users\Home Office\.claude\settings.json" text={SETTINGS_TEXT} />);
    expect(h).toContain("<textarea");
    expect(h).toContain('aria-label="C:\\Users\\Home Office\\.claude\\settings.json"');
    expect(h).toContain('spellcheck="false"');
  });
  it("reports a mistake with its line, linked to the text area by aria-describedby", () => {
    const h = renderToStaticMarkup(<RawEditor path="settings.json" text={SETTINGS_TEXT_BROKEN} />);
    expect(h).toMatch(/Line 3, column \d+:/);
    expect(h).toMatch(/aria-describedby="([^"]+)"/);
    expect(h).toContain('aria-invalid="true"');
  });
});
```

`Settings.css.test.ts`: no `border` besides radius and a start bar; `.scope-badge` has a fill and a forced-colours outline; `.settings__nav [aria-current]` is covered by `SelectedMark.css` (assert the nav item uses `is-selected-mark`); `.field[data-locked]` has a start-edge bar and a lock glyph (text "Set by your organisation"); `.raw-editor` uses `--font-mono` and `field-sizing: content` is not required; no `overflow` besides the page body.

- [ ] **Step 2: Run to verify they fail** — `pnpm vitest run src/features/settings` → FAIL (components missing).

- [ ] **Step 3: Implement.** Components:
- `ScopeBadge`: `<span className="scope-badge" data-scope title={SCOPE_NOTE[scope]}><svg aria-hidden …shape…/>{SCOPE_LABEL[scope]}</span>` (shape = small inline SVG: circle, square, diamond (rotated square), lock (body + shackle), flag).
- `FieldRow`: `<li className="field" data-locked={locked || undefined}>` with `<label>` + help paragraph + `ScopeBadge` + `appliesNote` + control: `boolean` → `Switch`; `enum` → native `<select>`; `number` → `<input type="number">`; `text` → `<input type="text">`; locked → control `disabled` and the reason paragraph.
- `SettingsForm` renders the fields of the category (`SETTINGS_FIELDS.filter(...)`; add a `category: string` property to `SettingField` in the fixtures).
- `RawEditor({ path, text })`: `parseRaw(text)`; textarea (`rows` from line count, `spellCheck={false}`, `aria-label={path}`, `aria-invalid` and `aria-describedby` only on error), diagnostics `<p id>` "Line 3, column 9: message".
- `ChangeBar`: when `state.changed`: `changeSummary(n)` text, `Review changes` button; when `state.reviewing`: `DiffView` of `previewChange(path, opened, edits)`, then "A backup is saved first, in `~/.marshell/snapshots/`. To undo: Settings → Advanced → Snapshots restores the file as it was." and the `Apply` and `Discard` buttons; `Apply` is absent when `readOnly`/`unparsable`/`conflict`.
- `SettingsPage`: two-column layout inside `Page title="Settings" level={1}`: a `<nav aria-label="Settings sections">` list of `SETTINGS_NAV` (items use `selectedProps(selected, "current")`), the body, the `EditMode` radio group (native radios named "Edit as": Form, Raw), banners (plain `<p>` blocks) for read-only / unparsable / conflict (conflict buttons `Show differences`, `Keep mine`, `Reload`).
- `settingsAnnouncement(state, changes)` in `settingsUi.ts`: returns `{ text: "Applied 2 changes to Claude Code settings. A backup was saved.", kind: "action-result" }` after apply and `null` otherwise (unit test it, and assert it never contains the banned words).
Stories (`title: "Settings"`): `FormMode`, `RawMode`, `RawModeWithError`, `ManagedLocked`, `PendingChange`, `ReviewingChange`, `ReviewingLongChange`, `ReadOnlyFile`, `UnparsableFile`, `Conflict`, `NoAgents` (empty category with `EmptyState`), `ScopeBadges` (all five), `NarrowWindow` (720 px frame: nav collapses to a select above the body; assert in `Settings.css.test.ts` with an `@container` or `@media (inline-size < 720px)` rule that stacks the columns), `RtlLabels` (field labels in Hebrew/Arabic with `dir="auto"`).

- [ ] **Step 4: Run to verify they pass** — `pnpm vitest run && pnpm exec tsc --noEmit` → PASS.

- [ ] **Step 5: Commit** — `git add -A src scripts && git commit -m "feat(settings): page, form and raw editor, scope badges and change review" -m "<trailer>"`.

---

### Task 12: Review page for sub-review B, keyboard and accessibility checks

**Files:**
- Modify: `src/design/review/renderReview.tsx`, `scripts/build-review.mjs`, `src/design/review/renderReview.test.tsx`, `src/styles/forcedColorsEdges.test.ts`, `docs/a11y/screen-reader-script.md`

- [ ] **Step 1: Failing test**

```tsx
describe("renderReview 3b content", () => {
  it("renders the doctor and the settings page", () => {
    const html = renderReview("3b");
    expect(html).toContain('id="doctor"');
    expect(html).toContain('id="settings"');
    for (const name of ["Fix sheet", "Fixed", "Under review", "Unparsable file", "Conflict", "Managed locked"]) expect(html, name).toContain(name);
  });
});
```

(Story names are humanised from export names: `FixSheet` → "Fix sheet", `ReviewingChange` is named "Reviewing change" in Task 11: use the exact humanised names of the stories you created; adjust this list to them.)
Run `pnpm vitest run src/design/review` → FAIL.

- [ ] **Step 2: Implement.** Add `"3b": [[Doctor, "<blurb>"], [Settings, "<blurb>"]]` with blurbs: doctor = "The report, the fix sheet with the combined diff grouped by file, the receipt toast with Restart them and Undo (10 seconds), and Undo last fix in the header afterwards. Problems Marshell cannot fix say why, and managed settings are listed, never changed."; settings = "Form and raw editing of each CLI's settings, with a scope badge on every value (User, Project, Local, Managed, Marshell). Every change is shown as the exact diff, with the backup and the way to undo, before Apply. A file that does not parse, is read-only, or changed on disk is never overwritten."
Replace `META["3b"]` with lede "Sub-review B of 3 in batch 3: the doctor and the settings page. Onboarding (A) and batch 2 are signed off and sit below." and checks:
- "The doctor's four fixes are one sheet with one diff per file; Confirm ticks the rows off inline and the receipt names the sessions that need a restart."
- "Undo last fix stays in the header after the toast, and says why when a file changed since."
- "In settings every value shows where it comes from (scope badge: word, shape and tooltip); a managed value is locked with the reason."
- "Raw mode shows the exact line of a mistake; a file that does not parse is never offered for editing."
- "Review changes shows the exact diff, the backup path and the way to undo before Apply."
- "Works with the keyboard alone, at 150% zoom and in Windows High Contrast (docs/a11y/screen-reader-script.md, batch 3 B)."
Append the "Batch 3 B: doctor and settings" section to `docs/a11y/screen-reader-script.md`: Narrator/NVDA steps and expected phrases (row read as "Warning, Approve from anywhere is not installed"; FixSheet dialog title read on open; toast text announced once "Fixed 4 issues. 2 sessions pick this up after a restart."; scope badge read as "Managed. Set by your organisation. Marshell can't change it."; radiogroup "Edit as, Form, selected").
- [ ] **Step 3: Verify** as in Task 7 step 3 with `3b`: tests, tsc, build, axe, scroll scan (`.page__body` may scroll; nothing else), screenshots of `#doctor` and `#settings`, Tab through the fix sheet and the settings form, `a11y:matrix` checks. Fix findings with tests first.
- [ ] **Step 4: Commit** — `git add -A src scripts docs && git commit -m "feat(review): sub-review B page for batch 3" -m "<trailer>"`.

**Controller step:** publish, report, wait for the owner's sign-off on B.

---

## Sub-review C: Departures, notifications and tray

### Task 13: Departures model: rows, flip schedule, order freeze and keyboard

**Files:**
- Create: `src/features/departures/board.ts`, `board.test.ts`, `flip.ts`, `flip.test.ts`, `freeze.ts`, `freeze.test.ts`, `nav.ts`, `nav.test.ts`, `clip.ts`, `clip.test.ts`

**Interfaces:**
- Consumes: `RowModel`, `SessionKind` (`src/features/sidebar/types.ts`), `GlyphKind` (`src/components/StatusGlyph/glyphs.ts`), `isTool`, `kindName` (`src/components/AgentMark/kinds.ts`).
- Produces:

```ts
export const COLUMNS = ["STATUS", "SESSION", "PROJECT / BRANCH", "AGENT", "WAITING", "CTX", "RECAP"] as const;
export type BoardRow = { id: string; number: number; name: string; where: string; agent: SessionKind; agentName: string; status: GlyphKind; statusText: string; waitingText: string; ctxText: string; recap: string; gate: boolean; waitingMs?: number };
export function boardRows(rows: RowModel[]): BoardRow[];
export function statusText(status: GlyphKind, manual: boolean): string;
export function waitingText(ms?: number): string;
export function splitGroups(rows: BoardRow[]): { gate: BoardRow[]; others: BoardRow[] };
export const ROW_PX = 44, HEAD_PX = 36, GROUP_PX = 28;
export function fitRows(heightPx: number, g: { gate: BoardRow[]; others: BoardRow[] }): { gate: BoardRow[]; others: BoardRow[]; hidden: number };
export function boardRowLabel(r: BoardRow): string;

// flip.ts
export const FLIP_CHAR_MS = 60, FLIP_STAGGER_MS = 15, FLIP_CAP_MS = 400;
export function settleAt(i: number): number;
export function flipDuration(len: number): number;
export function flipFrame(from: string, to: string, tMs: number): string;
export function flipFrames(from: string, to: string, stepMs?: number): Array<{ t: number; text: string }>;

// freeze.ts
export function freezeOrder(current: string[], incoming: string[]): { order: string[]; stale: string[] };

// nav.ts
export type BoardState = { active: number; card: string | null };
export function reduceBoard(s: BoardState, code: string, rows: BoardRow[]): { state: BoardState; jump?: string };

// clip.ts
export const CLIP_DURATION_MS = 10_000;
export type ClipEvent = { at: number; rowId: string; cell: "status" | "waiting"; from: string; to: string };
export const CLIP_EVENTS: ClipEvent[];
export function cellAt(rowId: string, cell: "status" | "waiting", baseline: string, tMs: number): string;
```

- [ ] **Step 1: Write the failing tests**

```ts
// board.test.ts
import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { SCENARIO_ROWS_WITH_TOOLS } from "../window/scenario";
import { boardRowLabel, boardRows, fitRows, GROUP_PX, HEAD_PX, ROW_PX, splitGroups, statusText, waitingText } from "./board";

describe("statusText and waitingText", () => {
  it("writes status in capitals, and says RUNNING / EXITED for a plain terminal", () => {
    expect(statusText("needs-permission", false)).toBe("NEEDS YOU");
    expect(statusText("needs-question", false)).toBe("NEEDS YOU");
    expect(statusText("working", false)).toBe("WORKING");
    expect(statusText("idle", true)).toBe("AT PROMPT");
    expect(statusText("running", true)).toBe("RUNNING");
    expect(statusText("ended", true)).toBe("EXITED");
    expect(statusText("ended", false)).toBe("ENDED");
  });
  it("writes waiting time in whole minutes, and an en dash when unknown", () => {
    expect(waitingText(undefined)).toBe("–");
    expect(waitingText(30_000)).toBe("<1 MIN");
    expect(waitingText(120_000)).toBe("2 MIN");
    expect(waitingText(3_900_000)).toBe("1 H 05 M");
  });
});

describe("boardRows", () => {
  const rows = boardRows(SCENARIO_ROWS_WITH_TOOLS);
  it("keeps the session number, the mark's name and an en dash where a value is unknown", () => {
    const docker = rows.find((r) => r.agent === "docker")!;
    expect(docker.number).toBe(8);
    expect(docker.ctxText).toBe("–");
    expect(docker.statusText).toBe("RUNNING");
    expect(rows.find((r) => r.id === "working")!.ctxText).toMatch(/%$/);
  });
  it("puts only sessions that need you in the gate group, oldest waiting first", () => {
    const g = splitGroups(rows);
    expect(g.gate.every((r) => r.gate)).toBe(true);
    expect(g.gate.map((r) => r.id)).toEqual(
      FIXTURES.filter((f) => f.status === "needs-permission" || f.status === "needs-question")
        .filter((f) => SCENARIO_ROWS_WITH_TOOLS.some((s) => s.id === f.id))
        .sort((a, b) => (b.waitingMs ?? 0) - (a.waitingMs ?? 0))
        .map((f) => f.id),
    );
    expect(g.others.map((r) => r.number)).toEqual([...g.others.map((r) => r.number)].sort((a, b) => a - b));
  });
  it("names a row with its number first and everything it shows", () => {
    const r = rows[0]!;
    const label = boardRowLabel(r);
    expect(label.startsWith(`Session ${r.number}, ${r.name}`)).toBe(true);
    expect(label).toContain(r.statusText.toLowerCase());
  });
});

describe("fitRows", () => {
  const twelve = boardRows(Array.from({ length: 12 }, (_, i) => ({ ...FIXTURES[2]!, id: `s${i}`, number: i + 1, status: "working" as const, waitingMs: undefined })));
  it("shows as many rows as fit and counts the rest, never leaving a partial row", () => {
    const room = HEAD_PX + GROUP_PX + 5 * ROW_PX + 10;
    const f = fitRows(room, splitGroups(twelve));
    expect(f.others).toHaveLength(5);
    expect(f.hidden).toBe(7);
  });
  it("keeps gate calls before anything else", () => {
    const rows = boardRows([{ ...FIXTURES[0]!, id: "g", number: 1 }, ...twelve.map((t) => ({ ...FIXTURES[2]!, id: t.id, number: t.number + 1 }))]);
    const f = fitRows(HEAD_PX + 2 * GROUP_PX + 3 * ROW_PX, splitGroups(rows));
    expect(f.gate.map((r) => r.id)).toContain("g");
  });
  it("shows everything when there is room, and nothing hidden for zero sessions", () => {
    expect(fitRows(2000, splitGroups(twelve)).hidden).toBe(0);
    expect(fitRows(2000, { gate: [], others: [] })).toEqual({ gate: [], others: [], hidden: 0 });
  });
});
```

```ts
// flip.test.ts
import { describe, expect, it } from "vitest";
import { FLIP_CAP_MS, flipDuration, flipFrame, flipFrames, settleAt } from "./flip";

describe("flip schedule", () => {
  it("starts on the old text and ends on the new text", () => {
    expect(flipFrame("WORKING", "NEEDS YOU", 0)).toBe("WORKING");
    expect(flipFrame("WORKING", "NEEDS YOU", flipDuration(9))).toBe("NEEDS YOU");
  });
  it("settles each character 15 ms after the one before, 60 ms after it starts, and never takes more than 400 ms", () => {
    expect(settleAt(0)).toBe(60);
    expect(settleAt(1)).toBe(75);
    expect(flipDuration(100)).toBeLessThanOrEqual(FLIP_CAP_MS);
    expect(flipDuration(0)).toBe(0);
  });
  it("never changes a character again once it has settled", () => {
    const frames = flipFrames("ERROR", "DONE", 8);
    const target = "DONE ";
    for (let i = 0; i < 4; i++) {
      const at = frames.findIndex((f) => f.t >= settleAt(i));
      for (const f of frames.slice(at)) expect(f.text.padEnd(5)[i]).toBe(target[i]);
    }
  });
  it("is deterministic: the same time gives the same frame", () => {
    expect(flipFrame("IDLE", "WORKING", 100)).toBe(flipFrame("IDLE", "WORKING", 100));
  });
  it("keeps spaces where both texts have them", () => {
    expect(flipFrame("NEEDS YOU", "NEEDS YOU", 30)).toBe("NEEDS YOU");
  });
});
```

```ts
// freeze.test.ts
import { describe, expect, it } from "vitest";
import { freezeOrder } from "./freeze";
describe("freezeOrder", () => {
  it("keeps every row where it is, appends new ones, and keeps a closed one in place marked stale", () => {
    expect(freezeOrder(["a", "b", "c"], ["c", "x", "a"])).toEqual({ order: ["a", "b", "c", "x"], stale: ["b"] });
  });
  it("is the incoming order when nothing was there", () => {
    expect(freezeOrder([], ["a", "b"])).toEqual({ order: ["a", "b"], stale: [] });
  });
});
```

```ts
// nav.test.ts
import { describe, expect, it } from "vitest";
import { boardRows } from "./board";
import { SCENARIO_ROWS } from "../window/scenario";
import { reduceBoard } from "./nav";

const rows = boardRows(SCENARIO_ROWS);
const gateIdx = rows.findIndex((r) => r.gate);
const plainIdx = rows.findIndex((r) => !r.gate);
describe("reduceBoard", () => {
  it("moves with the arrows and stops at the ends", () => {
    expect(reduceBoard({ active: 0, card: null }, "ArrowDown", rows).state.active).toBe(1);
    expect(reduceBoard({ active: 0, card: null }, "ArrowUp", rows).state.active).toBe(0);
    expect(reduceBoard({ active: 0, card: null }, "End", rows).state.active).toBe(rows.length - 1);
  });
  it("Enter jumps to the session and never answers a request", () => {
    const r = reduceBoard({ active: gateIdx, card: null }, "Enter", rows);
    expect(r.jump).toBe(rows[gateIdx]!.id);
    expect(r.state.card).toBeNull();
  });
  it("Y and N open the inline card only on a row that needs you", () => {
    expect(reduceBoard({ active: gateIdx, card: null }, "KeyY", rows).state.card).toBe(rows[gateIdx]!.id);
    expect(reduceBoard({ active: gateIdx, card: null }, "KeyN", rows).state.card).toBe(rows[gateIdx]!.id);
    expect(reduceBoard({ active: plainIdx, card: null }, "KeyY", rows).state.card).toBeNull();
  });
  it("Escape closes an open card and otherwise does nothing", () => {
    expect(reduceBoard({ active: 0, card: "x" }, "Escape", rows).state.card).toBeNull();
    expect(reduceBoard({ active: 2, card: null }, "Escape", rows).state).toEqual({ active: 2, card: null });
  });
  it("copes with no rows", () => {
    expect(reduceBoard({ active: 0, card: null }, "ArrowDown", []).state.active).toBe(0);
  });
});
```

```ts
// clip.test.ts
import { describe, expect, it } from "vitest";
import { flipDuration } from "./flip";
import { CLIP_DURATION_MS, CLIP_EVENTS, cellAt } from "./clip";
describe("the 10 second clip", () => {
  it("is ten seconds, in order, and flips only the status and waiting cells", () => {
    expect(CLIP_DURATION_MS).toBe(10_000);
    expect([...CLIP_EVENTS].map((e) => e.at)).toEqual([...CLIP_EVENTS].map((e) => e.at).sort((a, b) => a - b));
    expect(CLIP_EVENTS.every((e) => e.cell === "status" || e.cell === "waiting")).toBe(true);
    expect(CLIP_EVENTS.every((e) => e.at + flipDuration(Math.max(e.from.length, e.to.length)) <= CLIP_DURATION_MS)).toBe(true);
  });
  it("lets a cell finish flipping before it flips again", () => {
    for (const a of CLIP_EVENTS) for (const b of CLIP_EVENTS) if (a !== b && a.rowId === b.rowId && a.cell === b.cell && a.at < b.at) expect(b.at - a.at).toBeGreaterThanOrEqual(flipDuration(Math.max(a.from.length, a.to.length)));
  });
  it("reads the text of a cell at any time", () => {
    const e = CLIP_EVENTS[0]!;
    expect(cellAt(e.rowId, e.cell, e.from, e.at - 1)).toBe(e.from);
    expect(cellAt(e.rowId, e.cell, e.from, e.at + 1_000)).toBe(e.to);
  });
});
```

- [ ] **Step 2: Run to verify they fail** — `pnpm vitest run src/features/departures` → FAIL (modules missing).

- [ ] **Step 3: Implement.**

`board.ts` essentials:

```ts
import { isTool, kindName, type SessionKind } from "../../components/AgentMark/kinds";
import type { GlyphKind } from "../../components/StatusGlyph/glyphs";
import type { RowModel } from "../sidebar/types";

export const COLUMNS = ["STATUS", "SESSION", "PROJECT / BRANCH", "AGENT", "WAITING", "CTX", "RECAP"] as const;
export const ROW_PX = 44, HEAD_PX = 36, GROUP_PX = 28;

export function statusText(status: GlyphKind, manual: boolean): string {
  switch (status) {
    case "needs-permission": case "needs-question": return "NEEDS YOU";
    case "working": return "WORKING";
    case "running": return "RUNNING";
    case "idle": return manual ? "AT PROMPT" : "IDLE";
    case "done-unseen": case "done-seen": return "DONE";
    case "error": return "ERROR";
    case "stuck": return "STUCK";
    case "ended": return manual ? "EXITED" : "ENDED";
    case "limited": return "LIMITED";
  }
}
export function waitingText(ms?: number): string {
  if (ms === undefined) return "–";
  if (ms < 60_000) return "<1 MIN";
  const min = Math.floor(ms / 60_000);
  if (min < 60) return `${min} MIN`;
  return `${Math.floor(min / 60)} H ${String(min % 60).padStart(2, "0")} M`;
}
export function boardRows(rows: RowModel[]): BoardRow[] {
  return rows.map((r) => {
    const manual = isTool(r.agent);
    return {
      id: r.id, number: r.number, name: r.name,
      where: r.branch ? `${r.project} · ${r.branch}` : r.project,
      agent: r.agent, agentName: kindName(r.agent), status: r.status,
      statusText: statusText(r.status, manual), waitingText: waitingText(r.waitingMs),
      ctxText: manual || r.contextPct === undefined ? "–" : `${Math.round(r.contextPct)}%`,
      recap: r.recap?.text ?? "–", gate: r.status === "needs-permission" || r.status === "needs-question", waitingMs: r.waitingMs,
    };
  });
}
export function splitGroups(rows: BoardRow[]) {
  const gate = rows.filter((r) => r.gate).sort((a, b) => (b.waitingMs ?? 0) - (a.waitingMs ?? 0) || a.number - b.number);
  const others = rows.filter((r) => !r.gate).sort((a, b) => a.number - b.number);
  return { gate, others };
}
export function fitRows(heightPx: number, g: { gate: BoardRow[]; others: BoardRow[] }) {
  const groups = (g.gate.length ? 1 : 0) + (g.others.length ? 1 : 0);
  const room = Math.max(0, Math.floor((heightPx - HEAD_PX - groups * GROUP_PX) / ROW_PX));
  const gate = g.gate.slice(0, room);
  const others = g.others.slice(0, Math.max(0, room - gate.length));
  return { gate, others, hidden: g.gate.length + g.others.length - gate.length - others.length };
}
export const boardRowLabel = (r: BoardRow) =>
  `Session ${r.number}, ${r.name}, ${r.statusText.toLowerCase()}, ${r.where}, ${r.agentName}, waiting ${r.waitingText.toLowerCase()}, context ${r.ctxText}, ${r.recap}`;
```

(Fix `BoardRow` type to match the Interfaces block; the `fitRows` test for 12 rows: `room = floor((36+28+5*44+10 − 36 − 28)/44) = 5`.)

`flip.ts`:

```ts
export const FLIP_CHAR_MS = 60, FLIP_STAGGER_MS = 15, FLIP_CAP_MS = 400;
const GLYPHS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
export const settleAt = (i: number) => FLIP_CHAR_MS + Math.min(i * FLIP_STAGGER_MS, FLIP_CAP_MS - FLIP_CHAR_MS);
export const flipDuration = (len: number) => (len <= 0 ? 0 : settleAt(len - 1));
export function flipFrame(from: string, to: string, t: number): string {
  const n = Math.max(from.length, to.length);
  const f = from.padEnd(n);
  const g = to.padEnd(n);
  if (t >= flipDuration(n)) return to;
  let out = "";
  for (let i = 0; i < n; i++) {
    if (f[i] === " " && g[i] === " ") out += " ";
    else if (t >= settleAt(i)) out += g[i];
    else if (t <= i * FLIP_STAGGER_MS) out += f[i];
    else out += GLYPHS[(i * 7 + Math.floor((t - i * FLIP_STAGGER_MS) / (FLIP_CHAR_MS / 4)) * 5) % GLYPHS.length];
  }
  return out.replace(/ +$/, "");
}
export function flipFrames(from: string, to: string, stepMs = 16) {
  const end = flipDuration(Math.max(from.length, to.length));
  const out: Array<{ t: number; text: string }> = [];
  for (let t = 0; t < end; t += stepMs) out.push({ t, text: flipFrame(from, to, t) });
  out.push({ t: end, text: to });
  return out;
}
```

`freeze.ts`:

```ts
export function freezeOrder(current: string[], incoming: string[]) {
  const now = new Set(incoming);
  const fresh = incoming.filter((id) => !current.includes(id));
  return { order: [...current, ...fresh], stale: current.filter((id) => !now.has(id)) };
}
```

`nav.ts`:

```ts
import type { BoardRow } from "./board";
export type BoardState = { active: number; card: string | null };
export function reduceBoard(s: BoardState, code: string, rows: BoardRow[]): { state: BoardState; jump?: string } {
  const last = Math.max(0, rows.length - 1);
  const row = rows[s.active];
  switch (code) {
    case "ArrowDown": return { state: { ...s, active: Math.min(last, s.active + 1) } };
    case "ArrowUp": return { state: { ...s, active: Math.max(0, s.active - 1) } };
    case "Home": return { state: { ...s, active: 0 } };
    case "End": return { state: { ...s, active: last } };
    case "Enter": return row ? { state: { ...s, card: null }, jump: row.id } : { state: s };
    case "KeyY": case "KeyN": return row?.gate ? { state: { ...s, card: row.id } } : { state: s };
    case "Escape": return { state: { ...s, card: null } };
    default: return { state: s };
  }
}
```

`clip.ts`: define `CLIP_EVENTS` exactly:

```ts
export const CLIP_EVENTS: ClipEvent[] = [
  { at: 1_000, rowId: "working",          cell: "status",  from: "WORKING",   to: "NEEDS YOU" },
  { at: 1_000, rowId: "working",          cell: "waiting", from: "–",         to: "<1 MIN" },
  { at: 3_200, rowId: "needs-permission", cell: "waiting", from: "1 MIN",     to: "2 MIN" },
  { at: 5_000, rowId: "error",            cell: "status",  from: "ERROR",     to: "WORKING" },
  { at: 6_500, rowId: "needs-question",   cell: "waiting", from: "4 MIN",     to: "5 MIN" },
  { at: 8_000, rowId: "working",          cell: "status",  from: "NEEDS YOU", to: "WORKING" },
  { at: 8_000, rowId: "working",          cell: "waiting", from: "<1 MIN",    to: "–" },
];
export function cellAt(rowId: string, cell: "status" | "waiting", baseline: string, t: number): string {
  let text = baseline;
  for (const e of CLIP_EVENTS) if (e.rowId === rowId && e.cell === cell && t >= e.at) text = flipFrame(e.from, e.to, t - e.at);
  return text;
}
```

(The clip changes cell text in place; rows do not change group during the clip, which is the "rows never move while the pointer is over the board" rule shown at its strictest. The board's real behaviour on a status change is `freezeOrder`.)

- [ ] **Step 4: Run to verify they pass** — `pnpm vitest run src/features/departures && pnpm exec tsc --noEmit` → PASS.

- [ ] **Step 5: Commit** — `git add src/features/departures && git commit -m "feat(departures): board rows, flip schedule, order freeze, keyboard and the 10 second clip" -m "<trailer>"`.

---

### Task 14: Departures board, flip cells and the motion clip

**Files:**
- Create: `src/features/departures/DeparturesBoard.tsx`, `FlipCell.tsx`, `Board.css`, `DeparturesBoard.test.tsx`, `Board.css.test.ts`, `Departures.stories.tsx`, `scripts/review-departures.js`, `scripts/review-departures.test.ts`
- Modify: `scripts/build-review.mjs` (CSS list `src/features/departures/Board.css`; inline `review-departures.js` for batch `3c`), `src/styles/forcedColorsEdges.test.ts`

**Interfaces:**
- Consumes: Task 13 (all), `AgentMark`, `SessionNumber`, `selectedProps`, `ApproveCard` and `APPROVALS` (`src/features/approval`), `EmptyState` and `EMPTY_STATES["departures-empty"]` (Tasks 3, 6).
- Produces: `<DeparturesBoard rows heightPx active? card? stale? />`; `<FlipCell from to at? reduced? />` (renders `data-frames` JSON of `flipFrames(from, to)` and `data-at`, and the final text); `Departures.stories.tsx` exports `TwelveSessions`, `FiveAgentScenario`, `WithManualCli`, `NoSessions`, `InlineCard`, `Narrow` (720 px), `RtlNames`, `LongNames`, `ReducedMotion`, `MotionClip`, `MotionFilmstrip`.

**Ruling (Departures colour):** the board is a sign, so the board root sets `color-scheme: dark` in both themes ("true black with 44 px rows", docs/PLAN.md). Tokens still resolve through `light-dark()`, so the contrast tests keep applying to the dark values. Recorded in the plan's rulings and in PLAN.md at wrap-up.

- [ ] **Step 1: Write the failing tests**

```tsx
// DeparturesBoard.test.tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { labelOf, nameContainsLabel } from "../../lib/labelInName";
import { SCENARIO_ROWS_WITH_TOOLS } from "../window/scenario";
import { FIXTURES } from "../sidebar/fixtures";
import { DeparturesBoard } from "./DeparturesBoard";

const html = (o: Partial<Parameters<typeof DeparturesBoard>[0]> = {}) => renderToStaticMarkup(<DeparturesBoard rows={SCENARIO_ROWS_WITH_TOOLS} heightPx={800} {...o} />);

describe("DeparturesBoard", () => {
  it("has the plan's seven columns as real column headers", () => {
    const h = html();
    for (const c of ["STATUS", "SESSION", "PROJECT / BRANCH", "AGENT", "WAITING", "CTX", "RECAP"]) expect(h).toMatch(new RegExp(`<th[^>]*scope="col"[^>]*>${c.replace("/", "\\/")}`));
  });
  it("puts Gate calls first and counts them", () => {
    const h = html();
    expect(h.indexOf("Gate calls")).toBeGreaterThan(-1);
    expect(h.indexOf("Gate calls")).toBeLessThan(h.indexOf('data-gate="false"'));
    expect(h).toMatch(/Gate calls · 2/);
  });
  it("names every row from its visible text, starting with the session number", () => {
    for (const row of html().match(/<tr class="board__row"[\s\S]*?<\/tr>/g) ?? []) {
      expect(labelOf(row)?.startsWith("session ")).toBe(true);
      expect(nameContainsLabel(row)).toBe(true);
    }
  });
  it("marks the active row with the shared selected mark", () => {
    expect(html({ active: 1 })).toContain("is-selected-mark");
    expect(html({ active: 1 })).toContain('aria-current="true"');
  });
  it("shows a plain terminal as RUNNING with an en dash for context", () => {
    const h = html();
    expect(h).toContain("RUNNING");
    expect(h).toContain("–");
  });
  it("never lets twelve sessions overflow: it counts the rest and offers the all-sessions menu", () => {
    const twelve = Array.from({ length: 12 }, (_, i) => ({ ...FIXTURES[2]!, id: `s${i}`, number: i + 1 }));
    const h = renderToStaticMarkup(<DeparturesBoard rows={twelve} heightPx={400} />);
    expect(h).toMatch(/\+\d+ more sessions/);
    expect(h).toContain("All sessions");
  });
  it("says so when there are no sessions", () => {
    expect(renderToStaticMarkup(<DeparturesBoard rows={[]} heightPx={400} />)).toContain("No sessions on the board");
  });
  it("opens the approve card inline under a row that needs you", () => {
    const gate = SCENARIO_ROWS_WITH_TOOLS.find((r) => r.status === "needs-permission")!;
    expect(html({ card: gate.id })).toContain("approve-card");
    expect(html()).not.toContain("approve-card");
  });
  it("lets names read in their own direction and has no live region", () => {
    expect(html()).toContain('dir="auto"');
    expect(html()).not.toMatch(/aria-live|role="status"|role="alert"/);
  });
});
```

```ts
// Board.css.test.ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
const css = readFileSync(new URL("./Board.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
describe("Board.css", () => {
  it("is a dark sign in both themes, with 44 px rows", () => {
    expect(css).toMatch(/\.board\s*\{[^}]*color-scheme:\s*dark/);
    expect(css).toMatch(/\.board__row\s*\{[^}]*block-size:\s*44px/);
  });
  it("writes status, waiting and context in uppercase mono with tabular figures", () => {
    expect(css).toMatch(/\.board__status[^{]*\{[^}]*text-transform:\s*uppercase/);
    expect(css).toMatch(/font-family:\s*var\(--font-mono\)/);
    expect(css).toMatch(/font-variant-numeric:\s*tabular-nums/);
  });
  it("splits a tile at half its height with a gradient hairline, not a border", () => {
    expect(css).toMatch(/linear-gradient\([^)]*50%/);
    expect(css).not.toMatch(/border(?!-radius|-inline-start)/);
  });
  it("flips only under no-preference and cross-fades under reduce", () => {
    expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\)[\s\S]*?\.flip/);
    expect(css).toMatch(/@media \(prefers-reduced-motion: no-preference\)/);
  });
  it("never scrolls", () => {
    expect(css).not.toMatch(/overflow(-x|-y)?:\s*(auto|scroll)/);
  });
  it("colours the agent name from the session's accent", () => {
    expect(css).toMatch(/\.board__agent[^{]*\{[^}]*color:\s*var\(--accent\)/);
  });
});
```

```ts
// scripts/review-departures.test.ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
const js = readFileSync(new URL("./review-departures.js", import.meta.url), "utf8");
describe("review-departures.js", () => {
  it("plays the precomputed frames of every flip cell and replays with the page's button", () => {
    expect(js).toContain("data-frames");
    expect(js).toContain("data-at");
    expect(js).toMatch(/Replay animations|replay/i);
  });
  it("skips the flipping when motion is reduced", () => {
    expect(js).toContain("prefers-reduced-motion");
  });
});
```

- [ ] **Step 2: Run to verify they fail** — `pnpm vitest run src/features/departures scripts/review-departures.test.ts` → FAIL (files missing).

- [ ] **Step 3: Implement.**
- `DeparturesBoard`: `<section className="board" aria-label="Departures">` containing `<table className="board__table">` with `<thead>` (the seven `<th scope="col">`), then `<tbody>` rows from `fitRows(heightPx, splitGroups(boardRows(rows)))`: a group row `<tr class="board__group"><th scope="colgroup" colSpan={7}>Gate calls · N</th></tr>` (and "Other sessions · N" for the second group), then each `<tr className="board__row" data-gate={r.gate} aria-label={boardRowLabel(r)} tabIndex={active === index ? 0 : -1} {...selectedProps(active === index, "current")}>` with cells: status (`StatusGlyph`-free: text only, `FlipCell`), session (`SessionNumber` + name in `<span dir="auto">`), where (`<span dir="auto">`, ellipsis at the end), agent (`AgentMark size=14` + `agentName` in `.board__agent` with `data-agent`), waiting (`FlipCell`), ctx, recap (end ellipsis, full text in `title`). When `card === row.id` render `<tr class="board__card"><td colSpan={7}><ApproveCard …/></td></tr>` (read `ApproveCard.tsx` for props; use `APPROVALS.safeBash` for the gate row). The `hidden > 0` footer row: `<tr><td colSpan={7}>+{hidden} more sessions <button type="button">All sessions</button></td></tr>`. Zero rows: `EmptyState` with the `departures-empty` copy.
- `FlipCell({ from, to, at = 0, reduced = false })`: `<span className="flip" data-frames={JSON.stringify(flipFrames(from, to))} data-at={at} data-reduced={reduced || undefined}>{to}</span>` — the server markup is the **final** text; the review script plays the frames. When `from === to` it renders plain text with no data attributes.
- `Board.css` (read `good-css` first): `.board { color-scheme: dark; background: var(--bg-base); color: var(--text-1); }`; `.board__row { block-size: 44px; }`; tiles: `.board__status, .board__waiting { background-image: linear-gradient(to bottom, transparent calc(50% - 0.5px), var(--hairline) calc(50% - 0.5px) calc(50% + 0.5px), transparent calc(50% + 0.5px)); background-color: var(--bg-raised); font-family: var(--font-mono); text-transform: uppercase; font-variant-numeric: tabular-nums; }`; `.board__agent { color: var(--accent); }`; `.board__row[data-gate="true"]` start-edge bar `border-inline-start: 3px solid var(--attention)` is **not** allowed by the no-border rule if the scan flags it: copy how `.lane` / risky card draw their start bar (pseudo-element) and do the same; forced-colours rules for the selected row (Highlight outline) and tiles (`forced-color-adjust: none` with `CanvasText`); flipping: `@media (prefers-reduced-motion: no-preference) { .flip { /* characters swap via script frames */ } }` and `@media (prefers-reduced-motion: reduce) { .flip[data-frames] { animation: board-crossfade 200ms ease-out; } }`.
- `scripts/review-departures.js`: finds every `.flip[data-frames]`, parses the frames, and on load and on the page's "Replay animations" button (read `build-review.mjs` to see how that button is wired for the existing animations and fire the same event) plays each cell at `data-at` ms using `requestAnimationFrame`, setting `textContent` to the frame at the elapsed time and ending on the final text; when `matchMedia("(prefers-reduced-motion: reduce)").matches` or the page's Reduced motion checkbox is on, it only adds the cross-fade class.
- `build-review.mjs`: for batch `3c` inline `scripts/review-departures.js` after the terminal scripts (same `safeInline` helper) and add the CSS file.
- Stories (`title: "Departures"`): `TwelveSessions` (the 12-fixture board at 520 px height: 2 gate rows, others, "+N more sessions"), `FiveAgentScenario`, `WithManualCli` (the three plain terminals), `NoSessions`, `InlineCard`, `Narrow` (720 px wide: columns CTX and RECAP drop by container query; assert in the CSS test with an `@container` rule), `RtlNames`, `LongNames`, `ReducedMotion` (shows cross-fade markup), `MotionClip` (a board whose cells use `CLIP_EVENTS`: `FlipCell from/to at={event.at}` with a caption "10 seconds: 1 s a session starts waiting, 3.2 s a wait grows, 5 s an error resumes, 8 s an answer arrives. Press Replay animations."), `MotionFilmstrip` (six static frames of the board at `t = 0, 1.1, 3.3, 5.1, 6.6, 8.1 s` built with `cellAt`, so the clip can be judged without playing it).
- Forced-colours rows for `Board.css` in `forcedColorsEdges.test.ts`.

- [ ] **Step 4: Run to verify they pass** — `pnpm vitest run && pnpm exec tsc --noEmit` → PASS.

- [ ] **Step 5: Commit** — `git add -A src scripts && git commit -m "feat(departures): the board, flip cells and the 10 second motion clip" -m "<trailer>"`.

---

### Task 15: Notification banners: rules and OS mocks

**Files:**
- Create: `src/features/notifications/banner.ts`, `banner.test.ts`, `OsBanner.tsx`, `OsBanner.css`, `OsBanner.test.tsx`, `OsBanner.css.test.ts`, `Notifications.stories.tsx`
- Modify: `scripts/build-review.mjs` (CSS list)

**Interfaces:**
- Consumes: `Os` from `src/lib/keymap.ts`; `Logo`.
- Produces:

```ts
export type NotifyKind = "needs-permission" | "needs-question" | "done" | "error" | "stuck" | "subagent-done";
export type NotifyEvent = { kind: NotifyKind; number: number; name: string; project?: string; detail?: string; risky?: boolean; windowFocused?: boolean; backgroundMs?: number; resetsAt?: string; waitedMs?: number };
export type Banner = { title: string; body: string; meta: string; actions: string[]; silent: true; raisesWindow: false; activation: "focus-session"; sessionNumber: number };
export const DONE_MIN_BACKGROUND_MS = 30_000;
export const BATCH_WINDOW_MS = 3_000;
export function bannerFor(e: NotifyEvent, os: Os): Banner | null;
export function batchBanner(events: NotifyEvent[], os: Os): Banner | null;
export function escalationBanner(e: NotifyEvent, os: Os): Banner;
export function bannerAnnouncement(): null; // banners are sounds and OS UI; the window announces nothing extra
```

- [ ] **Step 1: Write the failing tests**

```ts
// banner.test.ts
import { describe, expect, it } from "vitest";
import { batchBanner, bannerFor, DONE_MIN_BACKGROUND_MS, escalationBanner, type NotifyEvent } from "./banner";

const perm: NotifyEvent = { kind: "needs-permission", number: 3, name: "api-server", project: "my-app", detail: "npm test" };
describe("bannerFor: what it says", () => {
  it("a permission request names the session and the command and offers Allow and Deny where the OS can", () => {
    const b = bannerFor(perm, "windows")!;
    expect(b.title).toBe("api-server needs you");
    expect(b.body).toBe("Wants to run `npm test`");
    expect(b.meta).toBe("Session 3 · my-app");
    expect(b.actions).toEqual(["Allow", "Deny"]);
    expect(bannerFor(perm, "mac")!.actions).toEqual(["Allow", "Deny"]);
    expect(bannerFor(perm, "linux")!.actions).toEqual(["Open"]);
  });
  it("a risky request shows only Review and never the command", () => {
    for (const os of ["windows", "mac", "linux"] as const) {
      const b = bannerFor({ ...perm, risky: true, detail: "rm -rf node_modules" }, os)!;
      expect(b.actions).toEqual(["Review"]);
      expect(`${b.title} ${b.body}`).not.toContain("rm -rf");
      expect(b.body).toBe("Needs a careful look.");
    }
  });
  it("a question says so and opens the session", () => {
    const b = bannerFor({ kind: "needs-question", number: 2, name: "infra", project: "terraform" }, "windows")!;
    expect(b.body).toBe("Has a question");
    expect(b.actions).toEqual(["Open"]);
  });
  it("an error says why and when it resets", () => {
    const b = bannerFor({ kind: "error", number: 6, name: "scratch", detail: "rate limit reached", resetsAt: "3:40 pm" }, "windows")!;
    expect(b.title).toBe("scratch stopped");
    expect(b.body).toBe("Stopped: rate limit reached. Resets at 3:40 pm.");
  });
});

describe("bannerFor: when it stays quiet", () => {
  it("is silent while the window is focused", () => {
    expect(bannerFor({ ...perm, windowFocused: true }, "windows")).toBeNull();
  });
  it("never notifies for a stuck session or a finished subagent", () => {
    expect(bannerFor({ kind: "stuck", number: 1, name: "a" }, "windows")).toBeNull();
    expect(bannerFor({ kind: "subagent-done", number: 1, name: "a" }, "windows")).toBeNull();
  });
  it("says done only for a session that has been in the background for more than 30 seconds", () => {
    expect(DONE_MIN_BACKGROUND_MS).toBe(30_000);
    expect(bannerFor({ kind: "done", number: 5, name: "docs", backgroundMs: 29_000 }, "windows")).toBeNull();
    expect(bannerFor({ kind: "done", number: 5, name: "docs", backgroundMs: 31_000 }, "windows")!.title).toBe("docs is done");
  });
});

describe("bannerFor: what every banner promises", () => {
  it("is silent (Marshell plays its own sound), never raises the window, and focuses the session when clicked", () => {
    for (const os of ["windows", "mac", "linux"] as const) {
      const b = bannerFor(perm, os)!;
      expect(b.silent).toBe(true);
      expect(b.raisesWindow).toBe(false);
      expect(b.activation).toBe("focus-session");
      expect(b.sessionNumber).toBe(3);
    }
  });
  it("follows the copy rules", () => {
    const all = [bannerFor(perm, "windows")!, bannerFor({ ...perm, risky: true }, "windows")!, bannerFor({ kind: "error", number: 1, name: "x", detail: "rate limit reached" }, "windows")!];
    for (const b of all) expect(`${b.title} ${b.body}`).not.toMatch(/!|successfully|oops|the ai/i);
  });
});

describe("batching and escalation", () => {
  const q: NotifyEvent = { kind: "needs-question", number: 2, name: "infra" };
  const p2: NotifyEvent = { kind: "needs-permission", number: 1, name: "billing", detail: "npm test" };
  it("folds several requests within 3 seconds into one banner", () => {
    const b = batchBanner([perm, q, p2], "windows")!;
    expect(b.title).toBe("3 sessions need you");
    expect(b.body).toBe("api-server, infra and billing");
    expect(b.actions).toEqual(["Open Marshell"]);
  });
  it("shows only Review when any of them is risky", () => {
    expect(batchBanner([perm, { ...p2, risky: true }], "windows")!.actions).toEqual(["Review"]);
  });
  it("is the ordinary banner for one event and nothing for none", () => {
    expect(batchBanner([perm], "windows")).toEqual(bannerFor(perm, "windows"));
    expect(batchBanner([], "windows")).toBeNull();
  });
  it("escalates once with the wait in minutes", () => {
    expect(escalationBanner({ ...perm, waitedMs: 60_000 }, "windows").body).toBe("api-server has waited 1 min to run a command");
  });
});
```

`OsBanner.test.tsx`: for each `os` and a permission banner, markup has `data-os`, `data-raises-window="false"`, an app name "Marshell", the title, body, the meta line and one `<button>` per action; the macOS and Linux variants include the caption "Design only" outside the banner element; risky banner has exactly one button named "Review"; names are `dir="auto"`; no live region; every button's name contains its text (`nameContainsLabel`).
`OsBanner.css.test.ts`: frames use `--bg-overlay` fill with `--shadow-menu`, no border, per-OS radius differences (`data-os="mac"` larger radius); forced-colours outline on `.os-banner`; no overflow scroll; text wraps with `overflow-wrap: anywhere`.

- [ ] **Step 2: Run to verify they fail** — `pnpm vitest run src/features/notifications` → FAIL.

- [ ] **Step 3: Implement** `banner.ts`:

```ts
import type { Os } from "../../lib/keymap";
export type { Os };
// …types as in Interfaces…

const base = (e: NotifyEvent) => ({ silent: true as const, raisesWindow: false as const, activation: "focus-session" as const, sessionNumber: e.number, meta: `Session ${e.number}${e.project ? ` · ${e.project}` : ""}` });

export function bannerFor(e: NotifyEvent, os: Os): Banner | null {
  if (e.windowFocused) return null;
  switch (e.kind) {
    case "stuck": case "subagent-done": return null;
    case "done":
      return (e.backgroundMs ?? 0) > DONE_MIN_BACKGROUND_MS ? { ...base(e), title: `${e.name} is done`, body: e.detail ?? "Done", actions: ["Open"] } : null;
    case "error":
      return { ...base(e), title: `${e.name} stopped`, body: `Stopped: ${e.detail ?? "an error"}.${e.resetsAt ? ` Resets at ${e.resetsAt}.` : ""}`, actions: ["Open"] };
    case "needs-question":
      return { ...base(e), title: `${e.name} needs you`, body: "Has a question", actions: ["Open"] };
    case "needs-permission":
      if (e.risky) return { ...base(e), title: `${e.name} needs you`, body: "Needs a careful look.", actions: ["Review"] };
      return { ...base(e), title: `${e.name} needs you`, body: `Wants to run \`${e.detail ?? "a command"}\``, actions: os === "linux" ? ["Open"] : ["Allow", "Deny"] };
  }
}
const list = (names: string[]) => (names.length <= 1 ? names.join("") : `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`);
export function batchBanner(events: NotifyEvent[], os: Os): Banner | null {
  const live = events.filter((e) => bannerFor(e, os));
  if (live.length === 0) return null;
  if (live.length === 1) return bannerFor(live[0]!, os);
  const risky = live.some((e) => e.risky);
  return { ...base(live[0]!), meta: `${live.length} sessions`, title: `${live.length} sessions need you`, body: list(live.map((e) => e.name)), actions: risky ? ["Review"] : ["Open Marshell"] };
}
export function escalationBanner(e: NotifyEvent, os: Os): Banner {
  const min = Math.max(1, Math.round((e.waitedMs ?? 60_000) / 60_000));
  const what = e.kind === "needs-question" ? "for an answer" : "to run a command";
  return { ...(bannerFor({ ...e, windowFocused: false }, os) ?? { ...base(e), title: `${e.name} needs you`, actions: ["Open"], body: "" }), body: `${e.name} has waited ${min} min ${what}` };
}
```

`OsBanner.tsx`: `<article className="os-banner" data-os={os} data-raises-window="false" aria-label={`${banner.title}. ${banner.body}`}>`… (the visible text is the title and body, so the name contains them; keep the label equal to them) with `Logo size={16} decorative`, `<p class="os-banner__app">Marshell</p>`, meta, title, body (`dir="auto"`), actions as `<button>`s. The `Design only` caption is rendered by the story wrapper for mac and linux (`<p class="os-banner__note">Design only: macOS and Linux are not tested yet.</p>`), and a caption on every frame: "Layout and content, not the operating system's own styling." The frames are neutral on purpose (no OS logos, no copied system chrome).
Stories (`title: "Notifications"`): `WindowsPermission`, `WindowsRisky`, `WindowsQuestion`, `WindowsError`, `WindowsDone`, `WindowsBatched`, `WindowsEscalation`, `MacPermission` (design only), `LinuxPermission` (design only), `QuietCases` (a table of the events that show **no** banner and why), `Rtl`.

- [ ] **Step 4: Run to verify they pass** — `pnpm vitest run && pnpm exec tsc --noEmit` → PASS.

- [ ] **Step 5: Commit** — `git add -A src scripts && git commit -m "feat(notifications): banner rules and OS banner mocks" -m "<trailer>"`.

---

### Task 16: Tray menu, badge rules and the icon at 16 and 32 px

**Files:**
- Create: `src/features/tray/trayMenu.ts`, `trayMenu.test.ts`, `TrayMenu.tsx`, `TrayIcon.tsx`, `Tray.css`, `TrayMenu.test.tsx`, `TrayIcon.test.tsx`, `Tray.css.test.ts`, `Tray.stories.tsx`
- Modify: `scripts/build-review.mjs` (CSS list)

**Interfaces:**
- Consumes: `COMMANDS`, `shortcutLabel`, `Os` (`src/lib/keymap.ts`); `SessionNumber`, `AgentMark`; `Logo`; `contrast.ts` (`src/design/contrast.ts`, read its exports for the contrast helper).
- Produces:

```ts
export type TraySession = { number: number; name: string; status: GlyphKind; phrase: string; agent: SessionKind };
export type TrayState = { os: Os; sessions: TraySession[]; paused: boolean; doctorIssues: number };
export type TrayItem = { id: string; label: string; kind: "item" | "session" | "separator"; shortcut?: string; number?: number; disabled?: boolean };
export const MAX_TRAY_SESSIONS = 5;
export function trayItems(s: TrayState): TrayItem[];
export function trayTooltip(s: TrayState): string;
export function badgeFor(os: Os, count: number): { kind: "overlay" | "dock" | "none"; text: string };
```

- [ ] **Step 1: Write the failing tests**

```ts
// trayMenu.test.ts
import { describe, expect, it } from "vitest";
import { badgeFor, MAX_TRAY_SESSIONS, trayItems, trayTooltip, type TrayState } from "./trayMenu";

const wait = (n: number, name: string, status: "needs-permission" | "needs-question" = "needs-permission") => ({ number: n, name, status, phrase: status === "needs-permission" ? "Wants to run `npm test`" : "Has a question", agent: "claude" as const });
const state: TrayState = { os: "windows", sessions: [wait(3, "api-server"), wait(2, "infra", "needs-question")], paused: false, doctorIssues: 4 };
const ids = (s: TrayState) => trayItems(s).map((i) => i.id);

describe("trayItems", () => {
  it("opens the window first, lists who needs you with their numbers, then the actions, then quits last", () => {
    const items = trayItems(state);
    expect(items[0]).toMatchObject({ id: "open", label: "Open Marshell" });
    expect(items.filter((i) => i.kind === "session").map((i) => `${i.number} · ${i.label}`)).toEqual(["3 · api-server: Wants to run `npm test`", "2 · infra: Has a question"]);
    expect(items[items.length - 1]).toMatchObject({ id: "quit", label: "Quit Marshell" });
  });
  it("says nothing needs you when that is so, as a disabled line", () => {
    const items = trayItems({ ...state, sessions: [] });
    expect(items.find((i) => i.id === "none")).toMatchObject({ label: "Nothing needs you", disabled: true });
  });
  it("shows at most five, then how many more", () => {
    const many = Array.from({ length: 8 }, (_, i) => wait(i + 1, `s${i}`));
    const items = trayItems({ ...state, sessions: many });
    expect(items.filter((i) => i.kind === "session")).toHaveLength(MAX_TRAY_SESSIONS);
    expect(items.find((i) => i.id === "more")!.label).toBe("3 more…");
  });
  it("offers to go to the oldest waiting session with the real shortcut for the OS", () => {
    expect(trayItems(state).find((i) => i.id === "next-waiting")!.shortcut).toBe("Ctrl+Shift+N");
    expect(trayItems({ ...state, os: "mac" }).find((i) => i.id === "next-waiting")!.shortcut).toBe("⌘N");
  });
  it("toggles pause and resume, and names the doctor's count", () => {
    expect(trayItems(state).find((i) => i.id === "pause")!.label).toBe("Pause notifications");
    expect(trayItems({ ...state, paused: true }).find((i) => i.id === "pause")!.label).toBe("Resume notifications");
    expect(trayItems(state).find((i) => i.id === "doctor")!.label).toBe("Doctor: 4 issues");
    expect(trayItems({ ...state, doctorIssues: 0 }).find((i) => i.id === "doctor")!.label).toBe("Doctor: all clear");
    expect(trayItems({ ...state, doctorIssues: 1 }).find((i) => i.id === "doctor")!.label).toBe("Doctor: 1 issue");
  });
  it("never lists a session that is not waiting", () => {
    const s = { ...state, sessions: [{ ...wait(1, "busy"), status: "working" as const }] };
    expect(ids(s)).not.toContain("session-1");
  });
});

describe("trayTooltip", () => {
  it("says how many need you, and when notifications are paused", () => {
    expect(trayTooltip(state)).toBe("Marshell · 2 need you");
    expect(trayTooltip({ ...state, sessions: [wait(1, "a")] })).toBe("Marshell · 1 needs you");
    expect(trayTooltip({ ...state, sessions: [] })).toBe("Marshell");
    expect(trayTooltip({ ...state, paused: true })).toBe("Marshell · 2 need you (notifications paused)");
  });
});

describe("badgeFor", () => {
  it("uses an overlay icon on Windows (the badge count is not supported there), capped at 9+", () => {
    expect(badgeFor("windows", 2)).toEqual({ kind: "overlay", text: "2" });
    expect(badgeFor("windows", 12)).toEqual({ kind: "overlay", text: "9+" });
  });
  it("uses the dock badge on macOS and nothing on Linux, where the tray is menu-only", () => {
    expect(badgeFor("mac", 3)).toEqual({ kind: "dock", text: "3" });
    expect(badgeFor("linux", 3)).toEqual({ kind: "none", text: "" });
  });
  it("shows nothing for zero", () => {
    for (const os of ["windows", "mac", "linux"] as const) expect(badgeFor(os, 0).text).toBe("");
  });
});
```

(`trayItems` ids: `open`, `session-<number>`, `more`, `none`, `next-waiting`, `pause`, `doctor`, `settings`, `quit`, and `kind: "separator"` ids `sep-1..`.)

`TrayIcon.test.tsx`: renders an `<svg>` with `viewBox="0 0 1024 1024"`, two `<line>` strokes with `stroke="currentColor"`, `stroke-width="170"`, width/height equal to `size` (16 or 32); the badge variant adds one `<circle>` using the accent; the wrapper is `role="img"` with `aria-label="Marshell, 2 need you"` while the inner svg is `aria-hidden`; no live region.
`TrayMenu.test.tsx`: `role="menu"`, each item `role="menuitem"`, disabled item `aria-disabled="true"`, session items carry `SessionNumber` and `AgentMark`, the Linux variant carries the note "On Linux the tray is menu-only: it has no click events." and the mac/linux variants carry the "Design only" caption, every item's accessible name contains its visible text, separators are `role="separator"`.
`Tray.css.test.ts`: menu uses `--bg-overlay` + `--shadow-menu`, items use `--hover` fill (hover only inside `@media (hover: hover) and (pointer: fine)`), `[aria-disabled="true"]` uses `--text-3`, separators are a `block-size: 1px` fill (no border), forced-colours outline on the menu, icon backgrounds use `color-scheme: light` / `dark` panels with `--bg-raised`.
A contrast test (in `Tray.css.test.ts` or `TrayIcon.test.tsx`, using `src/design/contrast.ts`): the icon's `--text-1` stroke on the light and the dark panel background is at least 3:1 (non-text contrast).

- [ ] **Step 2: Run to verify they fail** — `pnpm vitest run src/features/tray` → FAIL.

- [ ] **Step 3: Implement.**

```ts
// trayMenu.ts
import { COMMANDS, shortcutLabel, type Os } from "../../lib/keymap";
// … types …
const WAITING = new Set(["needs-permission", "needs-question"]);
export const MAX_TRAY_SESSIONS = 5;
const sep = (n: number): TrayItem => ({ id: `sep-${n}`, label: "", kind: "separator" });

export function trayItems(s: TrayState): TrayItem[] {
  const waiting = s.sessions.filter((x) => WAITING.has(x.status));
  const shown = waiting.slice(0, MAX_TRAY_SESSIONS);
  const next = COMMANDS.find((c) => c.id === "next-waiting")!;
  const items: TrayItem[] = [{ id: "open", label: "Open Marshell", kind: "item" }, sep(1)];
  if (waiting.length === 0) items.push({ id: "none", label: "Nothing needs you", kind: "item", disabled: true });
  for (const w of shown) items.push({ id: `session-${w.number}`, label: `${w.name}: ${w.phrase}`, kind: "session", number: w.number });
  if (waiting.length > shown.length) items.push({ id: "more", label: `${waiting.length - shown.length} more…`, kind: "item" });
  items.push(
    { id: "next-waiting", label: "Go to the oldest waiting session", kind: "item", shortcut: shortcutLabel(next.binding!, s.os), disabled: waiting.length === 0 },
    sep(2),
    { id: "pause", label: s.paused ? "Resume notifications" : "Pause notifications", kind: "item" },
    { id: "doctor", label: s.doctorIssues === 0 ? "Doctor: all clear" : `Doctor: ${s.doctorIssues} ${s.doctorIssues === 1 ? "issue" : "issues"}`, kind: "item" },
    { id: "settings", label: "Settings…", kind: "item" },
    sep(3),
    { id: "quit", label: "Quit Marshell", kind: "item" },
  );
  return items;
}
export function trayTooltip(s: TrayState): string {
  const n = s.sessions.filter((x) => WAITING.has(x.status)).length;
  const base = n === 0 ? "Marshell" : `Marshell · ${n} ${n === 1 ? "needs" : "need"} you`;
  return s.paused ? `${base} (notifications paused)` : base;
}
export function badgeFor(os: Os, count: number): { kind: "overlay" | "dock" | "none"; text: string } {
  if (count <= 0 || os === "linux") return { kind: "none", text: "" };
  return os === "windows" ? { kind: "overlay", text: count > 9 ? "9+" : String(count) } : { kind: "dock", text: String(count) };
}
```

(The test expects the list line as `3 · api-server: Wants to run ...` where `3 ·` comes from `number`; keep `label` without the number and let the menu render `SessionNumber` before it.)
`TrayIcon.tsx`: inline SVG copied from `assets/icon/tray-mono.svg` (`viewBox="0 0 1024 1024"`, `<g stroke="currentColor" stroke-width="170" stroke-linecap="round"><line x1="400" y1="884" x2="168" y2="150"/><line x1="624" y1="884" x2="856" y2="150"/></g>`) plus an optional badge `<circle cx="830" cy="190" r="170" fill="var(--accent)"/>`; props `{ size: 16 | 32; waiting?: number }`.
`TrayMenu.tsx`: `<div role="menu" aria-label="Marshell tray menu">` mapping items (`<button role="menuitem">` with the number badge, shortcut `<kbd>` at the end; `<div role="separator">`).
Stories (`title: "Tray and icon"`): `IconSizes` (16 and 32 px on a light and a dark panel, idle and with the needs-you dot, plus the app icon `Logo` at 16/32/48 on both), `TrayMenuWindows` (2 waiting), `TrayMenuNothing`, `TrayMenuMany` (8 waiting, "3 more…"), `TrayMenuPaused`, `TrayMenuMac` and `TrayMenuLinux` (design only; Linux carries the menu-only note), `Badges` (Windows overlay 1, 2, 9+; macOS dock 3; Linux none), `Tooltips`.

- [ ] **Step 4: Run to verify they pass** — `pnpm vitest run && pnpm exec tsc --noEmit` → PASS.

- [ ] **Step 5: Commit** — `git add -A src scripts && git commit -m "feat(tray): menu, badge rules and the icon at 16 and 32 px" -m "<trailer>"`.

---

### Task 17: Review page for sub-review C, keyboard and accessibility checks

**Files:**
- Modify: `src/design/review/renderReview.tsx`, `scripts/build-review.mjs`, `src/design/review/renderReview.test.tsx`, `docs/a11y/screen-reader-script.md`

- [ ] **Step 1: Failing test**

```tsx
describe("renderReview 3c content", () => {
  it("renders Departures, notifications and the tray", () => {
    const html = renderReview("3c");
    expect(html).toContain('id="departures"');
    expect(html).toContain('id="notifications"');
    expect(html).toContain('id="tray-and-icon"');
    for (const name of ["Twelve sessions", "Motion clip", "Motion filmstrip", "Windows risky", "Windows batched", "Icon sizes", "Tray menu windows"]) expect(html, name).toContain(name);
  });
  it("is cumulative", () => {
    expect(renderReview("3c")).toContain('id="doctor"');
  });
});
```

Run `pnpm vitest run src/design/review` → FAIL.

- [ ] **Step 2: Implement.** `BATCHES["3c"] = [[Departures, ...], [Notifications, ...], [Tray, ...]]` with blurbs: departures = "The board is a dark sign in both themes: 44 px rows, status and waiting cells that flip (60 ms a character, 15 ms apart, never longer than 400 ms; a cross-fade with reduced motion), Gate calls on top, and rows that never move under the pointer. Arrow keys move, Enter jumps to the session, Y or N opens the request inline. Press Replay animations to play the 10 second clip; the filmstrip shows it frame by frame."; notifications = "What a banner says, per operating system (layout and content, not the system's own styling; macOS and Linux are design only). A risky request shows only Review and never the command. No banner takes focus or raises the window. Several requests within 3 seconds become one banner."; tray = "The tray menu (who needs you, with numbers; go to the oldest; pause; doctor; settings; quit), the badge rules per OS, and the icon at 16 and 32 px on light and dark."
Replace `META["3c"]` with lede "Sub-review C of 3 in batch 3: Departures, notifications and the tray. Onboarding, doctor and settings (A and B) and batch 2 are signed off and sit below." and checks:
- "The board reads at a glance: you can say which sessions need you and for how long, from the gate group alone."
- "Press Replay animations: only the status and waiting cells flip, within 400 ms; with Reduced motion on they cross-fade."
- "Twelve sessions fit without scrolling the window and say how many more there are."
- "A risky banner offers only Review and does not show the command; no banner raises the window."
- "The icon is legible at 16 px on both light and dark taskbars; the overlay and dock badges follow each OS's rule."
- "Keyboard: the board moves with arrows, Enter jumps, Y or N opens the card inline; works at 150% zoom and in Windows High Contrast (docs/a11y/screen-reader-script.md, batch 3 C)."
Append "Batch 3 C" to `docs/a11y/screen-reader-script.md` (row read as "Session 3, api-server, needs you, ..."; the inline card via Y; a banner is read as title then body; tray menu items read with their shortcuts).
- [ ] **Step 3: Verify** as in Task 7 step 3 with `3c`: tests, tsc, build, axe, scroll scan, screenshots of the three sections in both themes, the clip replay (take frames of the page at 0.5 s, 1.1 s and 3.3 s after pressing Replay), a Tab/arrow walk of the board, `a11y:matrix` checks including colour-blindness filters (every status on the board must still be told apart by its words). Fix findings with tests first.
- [ ] **Step 4: Commit** — `git add -A src scripts docs && git commit -m "feat(review): sub-review C page for batch 3" -m "<trailer>"`.

**Controller step:** publish, report, wait for the owner's sign-off on C.

---

### Task 18: Wrap-up of batch 3

**Files:**
- Modify: `docs/PLAN.md`, `docs/BRIEF.md` (Amendments), `docs/IDEAS.md`
- Create (git-ignored scratch): the ledger at `.superpowers/sdd/2026-10-10-phase-0-batch-3-onboarding-doctor-settings-departures/progress.md`

- [ ] **Step 1: Update the docs.** In `docs/PLAN.md` add rows or subsections for what batch 3 decided (one short paragraph each, in the style of the batch 2 rows): **Onboarding** (four screens, the 60 second budget, consent card contents, fail-closed rules, the empty-state catalogue), **Doctor** (report, fix sheet, 10 s toast, header Undo, stale undo rule, announcement policy `action-result`), **Settings** (scopes and badges, form and raw, change review, conflict handling, managed locked), **Departures** (dark sign in both themes, 44 px rows, flip schedule 60/15/400 ms, order freeze, fit rule and "+N more", inline card keys), **Notifications** (rules table: focused, stuck, subagent, done after 30 s, risky shows Review only, batching 3 s, escalation once), **Tray and icon** (items, badge per OS, Linux menu-only, 16/32 px). Mark rows 7 to 11 of the Phase 0 table as designed. Add the **Page scroll exception** (`.page__body` visible bar) beside the existing no-scroll rule. In `docs/BRIEF.md` Amendments add one bullet "Batch 3 design decisions (date)" summarising the same. In `docs/IDEAS.md` move anything resolved to Planned and add the left-over items found while building (e.g. real jsonc CST edits in phase 3, verified key names for the doctor's illustrative fixes).
- [ ] **Step 2: Whole-batch verification.** Run `pnpm vitest run`, `pnpm exec tsc --noEmit`, `pnpm review:build "$SP/marshell-design-review.html" 3c`, the browser axe run (no findings beyond the ignored page-level rules), the in-page scroll scan (only `.sidebar__list`, `.drawer`, `.page__body` may scroll), and the a11y matrix checks (forced colours, reduced motion, 150% zoom, colour-blindness). Record each result in the ledger.
- [ ] **Step 3: Fresh whole-branch review.** Run `.../superpowers/.../subagent-driven-development/scripts/review-package docs/superpowers/plans/2026-10-10-phase-0-batch-3-onboarding-doctor-settings-departures.md <merge-base with phase-0-design> HEAD` and dispatch the code reviewer on the most capable model (`superpowers:requesting-code-review` `code-reviewer.md`), giving it the plan's Review Focus verbatim and the ledger's `Ruling:` lines. Also give it `docs/PLAN.md` and `docs/BRIEF.md` as the spec and tell it to check the Global Constraints.
- [ ] **Step 4: One fix pass.** Re-grade the findings by effect (Critical and Important enter the pass; Minor goes to the ledger as `Final: minor (deferred)`). Each fix gets a failing test first, then the whole suite. Record `Final: fixed ... RED→GREEN, suite N/N`.
- [ ] **Step 5: Commit** — `git add -A docs src scripts && git commit -m "docs: batch 3 decisions in PLAN, BRIEF and IDEAS" -m "<trailer>"`.
- [ ] **Step 6: Finish.** Report the rulings (every `Ruling:` line, with what each costs if wrong) and the deferred minors to the owner, then use `superpowers:finishing-a-development-branch`: the base is `phase-0-design` while pull request #2 is open (retarget the new pull request to `main` once #2 has merged). **Do not push or open the pull request without the owner's yes.**

## Execution notes

- **Ledger:** `.superpowers/sdd/2026-10-10-phase-0-batch-3-onboarding-doctor-settings-departures/progress.md` (git-ignored; first line `# SDD ledger — plan: docs/superpowers/plans/2026-10-10-phase-0-batch-3-onboarding-doctor-settings-departures.md`). One `Task N: complete (...)` line per task; `Ruling:` lines for every deviation.
- **Rulings already made in this plan** (copy them into the ledger at the start): (1) page bodies (`.page__body`) scroll with a visible themed bar and are allowlisted with a guard test; (2) the Departures board is a dark sign in both themes; (3) the fix sheet takes focus on its title, not on an action, and no key confirms it by itself; (4) a result the user caused is announced politely in every announce mode (`action-result`); (5) the settings mock re-serialises JSON (phase 3 uses CST edits) and the UI only ever shows the diff; (6) OS banner mocks are neutral layouts, not copies of the operating systems' own styling; (7) doctor fixes shown for the duplicate-notification check are illustrative until phase 7 verifies the keys.
- **Order and checkpoints:** Tasks 1 to 3 are shared groundwork. Tasks 4 to 7 then end at the sub-review A checkpoint; 8 to 12 at B; 13 to 17 at C; 18 closes the batch. Each checkpoint is a published review page version and the owner's sign-off before the next sub-review starts, as in batch 2.
- **Open questions for the owner** (decide as rulings if unanswered, keep going): whether the doctor should offer Fix all only, or per-item fixing too (the plan has all-at-once, as in docs/PLAN.md "Doctor fix and undo"); whether the Departures board should be reachable from the tray menu (not in the plan; add "Departures" to the tray list if yes); whether Linux should get Allow and Deny on banners through the deep-link fallback instead of "Open".
