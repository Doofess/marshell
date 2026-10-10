# Phase 0 batch 2: approve card, main window, launcher and palette. Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Storybook mockups, built on real reusable components, of phase 0 deliverables 4–6 (approve card, main window composite with the 5-agent scenario and a real xterm terminal, launcher and command palette), reviewed and signed off in three sub-reviews: A (card), B (window), C (launcher and palette).

**Architecture:**
- Pure logic lives in small tested modules (headline and payload text, arming and key rules, lane layout, sidebar mode, filters, shortcut labels, terminal palettes). Components are presentational and take view models, as in batch 1.
- Stories render every state. The review page (`scripts/build-review.mjs`) server-renders them into one HTML file; for sub-review B the page also inlines xterm.js so the terminal is real in the published page, not a picture.
- The terminal has its own palette (docs/PLAN.md "Terminal theme"): follow-app by default, live switch, `minimumContrastRatio` on.

**Tech Stack:** React 19, TypeScript 7, Vite 8, Storybook 10.6 (addon-a11y, addon-themes), `@xterm/xterm` 6 with `@xterm/addon-fit`, culori 4, Vitest 5 (environment `node`; components are tested through `react-dom/server`), Playwright MCP for visual and axe checks.

**Spec:** `docs/PLAN.md`, sections "UX and UI design" (Phase 0 table rows 4–6, Layout, The approve card, Key flows, Terminal theme, Microcopy, UX risks, Keyboard routing in section 7). Batch 1 decisions are in the Amendments of `docs/BRIEF.md`. Batch 1 plan: `docs/superpowers/plans/2026-10-10-phase-0-batch-1-tokens-glyphs-rows.md`.

## Already done before this plan was executed

- Sidebar default is 288 px (`--sidebar`), compact density removed, the agent mark leads each row and the status glyph sits in the right cluster, the model is on line 2 (commits `0153366`, `1c53cd0`).
- Rows that stop the workflow (needs you, error) stand out: accent or error band just inside the brand stripe, 12% accent tint (10% error tint), reason line in full-contrast text; `--accent-tint` is 12% and `--accent-flash` 24% (commit `f5ee7c5`).
- `docs/PLAN.md` carries the "Terminal theme" section (commit `0e99fbc`).

## Global Constraints

- **Styles:** follow the good-css skill; read it before writing any CSS.
  - logical properties only (`inline`/`block`, never left/right/top/bottom);
  - colours in `oklch()` with `none` as the hue of greys; tints via `color-mix(in oklch, …)`;
  - `:hover` only inside `@media (hover: hover) and (pointer: fine)`;
  - focus via `:focus-visible` + `outline`, never `outline: none`; everything pressable has an `:active` state;
  - `overflow: clip`, not `hidden`;
  - transitions name their properties, never `all`, never `ease-in`, and sit inside `@media (prefers-reduced-motion: no-preference)` when they move or scale something.
- **No literal colours in components or markup.** Every colour is a `var(--…)` from `src/styles/tokens.css` or `accents.css`. The only literal hex values are the xterm palettes in `src/features/terminal/palettes.ts`, because xterm cannot read CSS variables.
- **Default accent:** signal amber; the accent marks attention only (fills, badges, rings, focus). Brand colours appear only on stripes, dots and the agent mark, never as status.
- **Every status is readable without colour:** a glyph shape plus a text label. Diffs use +/− signs as well as colour.
- **Unknown values render as `–`** (U+2013), never a guess.
- **Rows never change height on hover.** Only two row densities exist: comfortable (56 px) and expanded.
- **Sizes (from docs/PLAN.md "Layout"):** window minimum 720×480; top bar one unified 40 px; sidebar default **288 px** (min 200, max 400, rail 52, focus mode 0); below 960 px wide the sidebar auto-collapses to the rail; sidebar footer 40 px; needs-you lane header always present at 28 px; lane cards accordion (target expanded 120–176 px, others 36 px one-liners); lane at most 40% of the sidebar height; right drawer 360 px (300–560).
- **Approve card safety rules:** Enter never allows. Buttons are inert for 500 ms after the card gains focus or its content changes. No global approve shortcut (Y/N/A/T work only while the card has focus). Risky requests: Allow becomes **hold Y for 600 ms** with a progress ring; "Always" is hidden; a truncated payload must be expanded before Allow arms; the plan's classification list is the core's job, so phase 0 only carries a `risk` field in fixtures.
- **Copy:** sentence case, subject first, the agent's name (not "the AI"), concrete numbers, no "!", no "successfully", no "Oops"; after a decision the card shows a receipt and never implies an undo.
- **Motion:** only tokens from `src/styles/tokens.css`; card receipt shows 1.5 s then exits in `--dur-exit` (140 ms); reduced-motion has a version for each.
- **Shortcuts:** macOS Cmd+key; Windows and Linux Ctrl+Shift+key (docs/PLAN.md section 7 "Keyboard routing"). Labels come from one keymap.
- **Terminal theme:** setting Follow app (default) / Always dark / Always light; applies live to every open terminal; `minimumContrastRatio` 4.5.
- **Tests read CSS from disk** with `node:fs` (`new URL("./X.css", import.meta.url)`), because Vitest returns CSS imports as empty strings. Line endings are LF.
- **Commit messages** end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. Never run destructive git; the user pushes.

## Review Focus

Failure modes the spec implies but a happy-path test would miss, most likely first. Each is pinned by a test in the named task.

1. **Hostile text in a card** (very long unbroken token, CJK, RTL, emoji, a path with spaces, an invalid URL for Fetch). The card wraps and never overflows; the headline never throws. (Task 1, Task 4)
2. **Accidental approval paths.** Enter on a risky card; Y pressed for less than 600 ms; Y before the 500 ms arming delay; Y on a risky card whose payload is truncated and not expanded; "Always" on a risky card. None may allow. (Task 2)
3. **Lane extremes.** 0 requests ("All clear", header still 28 px), 1, and 12 requests in a short sidebar (internal scroll, "+N more", never above 40%). (Task 8)
4. **Terminal theme switch while output is on screen.** Palette swap keeps scrollback; every ANSI colour except dark "black" holds 4.5:1 on its own background; an RGB colour the program painted (dark grey on a light terminal) is lifted by contrast protection. (Task 6, Task 7)
5. **Filter input that looks like a pattern** (`(`, `[`, `\`, `.*`, empty string, only spaces, an emoji). The filters use no regular expressions and never throw; an empty query returns everything in the remembered order. (Task 12)

---

### Task 0: Recheck hover and selected contrast (batch 1 deferred minor)

`overlay(fg, bg, alpha)` already exists in `src/design/contrast.ts` (added with the blocked-row work, commit `f5ee7c5`), and `contrast.test.ts` already checks text on the needs-you and error tints. What remains is the hover and selected check from batch 1's deferred minors.

**Files:**
- Test: `src/design/contrast.test.ts`
- Modify (only if the test fails): `src/styles/tokens.css`

- [ ] **Step 1: Write the test**

Append to `src/design/contrast.test.ts` (`overlay`, `contrast`, `tokens` and `THEMES` are already in scope):

```ts
describe("text on hover and selected rows (batch 1 deferred minor)", () => {
  for (const theme of THEMES)
    for (const [state, alpha] of [["hover", 0.04], ["selected", 0.07]] as const)
      for (const t of ["--text-2", "--text-3"])
        for (const surface of ["--bg-base", "--bg-raised", "--bg-overlay"])
          it(`${t} is at least 4.5:1 on ${surface} under ${state} (${theme})`, () => {
            const bg = overlay(tokens["--text-1"]![theme], tokens[surface]![theme], alpha);
            expect(contrast(tokens[t]![theme], bg)).toBeGreaterThanOrEqual(4.5);
          });
});
```

- [ ] **Step 2: Run it**

Run: `pnpm vitest run src/design/contrast.test.ts`
Expected: PASS. If a `--text-3 (light)` case fails (it measured 4.43 at a 10% hover in batch 1), darken the light value of `--text-3` in `src/styles/tokens.css` by one lightness point (`oklch(53% …)` to `oklch(52% …)`), repeat until green, and ledger `Ruling: light --text-3 darkened to <value> so hover and selected stay AA — cost if wrong: one token`. A red here that also trips the `text on surfaces` tests means the change was too large; step back one point.

- [ ] **Step 3: Whole suite and commit**

Run `pnpm vitest run` and `pnpm tsc --noEmit` (green), then:

```bash
git add src
git commit -m "test(design): text stays AA under hover and selected overlays

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Sub-review A: the approve card

### Task 1: Approval domain: types, headline, payload, fixtures

**Files:**
- Create: `src/features/approval/types.ts`, `src/features/approval/headline.ts`, `src/features/approval/payload.ts`, `src/features/approval/fixtures.ts`
- Test: `src/features/approval/headline.test.ts`, `src/features/approval/payload.test.ts`

**Interfaces:**
- Consumes: `AgentId` from `src/components/AgentMark/agents.ts`.
- Produces (exact):

```ts
// types.ts
export type Risk = { level: "normal" | "risky"; reason?: string };
export type DiffLine = { kind: "context" | "add" | "del"; text: string };
export type ToolDetail =
  | { kind: "bash"; command: string; cwd?: string }
  | { kind: "edit"; path: string; add: number; del: number; hunk: DiffLine[] }
  | { kind: "write"; path: string; preview: string; isNew: boolean }
  | { kind: "fetch"; url: string }
  | { kind: "mcp"; server: string; tool: string; args: Record<string, string> }
  | { kind: "question"; question: string; options: string[] };
export type CardState =
  | { phase: "pending" }
  | { phase: "decided"; verdict: "allowed" | "denied" }
  | { phase: "terminal" }
  | { phase: "released"; afterMs: number };
export type ApprovalRequest = {
  id: string;
  session: { name: string; agent: AgentId };
  sessionCwd: string;
  waitingMs: number;
  detail: ToolDetail;
  risk: Risk;
  /** The exact rule "Always allow" would add, e.g. "Bash(npm test:*)". Absent when no rule can be offered. */
  alwaysRule?: string;
  state: CardState;
};
// headline.ts
export function headline(d: ToolDetail): string;
export function subject(d: ToolDetail): string;
export function receiptText(d: ToolDetail, verdict: "allowed" | "denied"): string;
// payload.ts
export const COLLAPSED_LINES = 4;
export function payloadLines(d: ToolDetail): string[];
export function estimateLines(lines: string[], cols?: number): number;
export function isTruncated(lines: string[], cols?: number): boolean;
export function showAllLabel(total: number): string;
```

- [ ] **Step 1: Write the failing tests**

`src/features/approval/headline.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { headline, receiptText } from "./headline";

describe("headline", () => {
  it("names each tool as a verb phrase", () => {
    expect(headline({ kind: "bash", command: "npm test" })).toBe("Run a command");
    expect(headline({ kind: "edit", path: "src/auth.ts", add: 12, del: 3, hunk: [] })).toBe("Edit src/auth.ts (+12 \u22123)");
    expect(headline({ kind: "write", path: ".env.example", preview: "", isNew: true })).toBe("Create .env.example");
    expect(headline({ kind: "write", path: "README.md", preview: "", isNew: false })).toBe("Overwrite README.md");
    expect(headline({ kind: "fetch", url: "https://api.github.com/repos/x/y" })).toBe("Fetch api.github.com");
    expect(headline({ kind: "mcp", server: "github", tool: "create_pr", args: {} })).toBe("Use github \u00b7 create_pr");
    expect(headline({ kind: "question", question: "Which branch?", options: [] })).toBe("Has a question");
  });
  it("never throws on a URL it cannot parse (review focus 1)", () => {
    expect(headline({ kind: "fetch", url: "not a url \u{1F680}" })).toBe("Fetch not a url \u{1F680}");
  });
  it("keeps paths with spaces and non-Latin names whole", () => {
    expect(headline({ kind: "write", path: "C:/Users/Home Office/\u8a8d\u8a3c.ts", preview: "", isNew: true })).toBe("Create C:/Users/Home Office/\u8a8d\u8a3c.ts");
  });
  it("shows zero changes as +0 \u22120 rather than hiding them", () => {
    expect(headline({ kind: "edit", path: "a.ts", add: 0, del: 0, hunk: [] })).toBe("Edit a.ts (+0 \u22120)");
  });
});

describe("receiptText", () => {
  it("says what was decided and about what", () => {
    expect(receiptText({ kind: "bash", command: "npm test" }, "allowed")).toBe("Allowed \u00b7 npm test");
    expect(receiptText({ kind: "bash", command: "rm -rf build" }, "denied")).toBe("Denied \u00b7 rm -rf build");
    expect(receiptText({ kind: "edit", path: "src/auth.ts", add: 1, del: 1, hunk: [] }, "allowed")).toBe("Allowed \u00b7 src/auth.ts");
    expect(receiptText({ kind: "fetch", url: "https://api.github.com/x" }, "allowed")).toBe("Allowed \u00b7 api.github.com");
    expect(receiptText({ kind: "mcp", server: "github", tool: "create_pr", args: {} }, "allowed")).toBe("Allowed \u00b7 create_pr");
  });
  it("uses only the first line of a command, cut to 40 characters", () => {
    const long = "npm run build -- --mode production --sourcemap --minify --report";
    expect(receiptText({ kind: "bash", command: `${long}\nsecond` }, "allowed")).toBe(`Allowed \u00b7 ${long.slice(0, 39)}\u2026`);
  });
});
```

`src/features/approval/payload.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { COLLAPSED_LINES, estimateLines, isTruncated, payloadLines, showAllLabel } from "./payload";

describe("payloadLines", () => {
  it("splits a command on newlines", () => {
    expect(payloadLines({ kind: "bash", command: "a\nb\nc" })).toEqual(["a", "b", "c"]);
  });
  it("prefixes diff lines with their sign", () => {
    expect(
      payloadLines({ kind: "edit", path: "a.ts", add: 1, del: 1, hunk: [{ kind: "context", text: "x" }, { kind: "del", text: "y" }, { kind: "add", text: "z" }] }),
    ).toEqual(["  x", "- y", "+ z"]);
  });
  it("shows MCP arguments as key: value pairs", () => {
    expect(payloadLines({ kind: "mcp", server: "github", tool: "create_pr", args: { title: "Fix", base: "main" } })).toEqual(["title: Fix", "base: main"]);
  });
  it("shows the URL, the question and the file preview", () => {
    expect(payloadLines({ kind: "fetch", url: "https://a.dev/x" })).toEqual(["https://a.dev/x"]);
    expect(payloadLines({ kind: "question", question: "Which branch?", options: ["main"] })).toEqual(["Which branch?"]);
    expect(payloadLines({ kind: "write", path: "a", preview: "l1\nl2", isNew: true })).toEqual(["l1", "l2"]);
  });
});

describe("truncation", () => {
  it("collapses after four lines", () => {
    expect(COLLAPSED_LINES).toBe(4);
    expect(isTruncated(["1", "2", "3", "4"])).toBe(false);
    expect(isTruncated(["1", "2", "3", "4", "5"])).toBe(true);
  });
  it("counts a long unbroken line as several wrapped lines (review focus 1)", () => {
    expect(estimateLines(["x".repeat(44)], 44)).toBe(1);
    expect(estimateLines(["x".repeat(45)], 44)).toBe(2);
    expect(isTruncated(["x".repeat(400)], 44)).toBe(true);
  });
  it("counts an empty line as one", () => {
    expect(estimateLines(["", ""], 44)).toBe(2);
  });
  it("labels the expander with the real total", () => {
    expect(showAllLabel(23)).toBe("Show all 23 lines (Space)");
  });
});
```

- [ ] **Step 2: Run to see them fail**

Run: `pnpm vitest run src/features/approval`
Expected: FAIL (modules not found).

- [ ] **Step 3: Implement**

`src/features/approval/types.ts`: the type block exactly as in Interfaces, with `import type { AgentId } from "../../components/AgentMark/agents";` at the top.

`src/features/approval/headline.ts`:

```ts
import type { ToolDetail } from "./types";

const MINUS = "\u2212";

/** The host of a URL, or the raw text when it does not parse. */
function host(url: string): string {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}

/** The card's "what", as a verb phrase (docs/PLAN.md "The approve card"). */
export function headline(d: ToolDetail): string {
  switch (d.kind) {
    case "bash":
      return "Run a command";
    case "edit":
      return `Edit ${d.path} (+${d.add} ${MINUS}${d.del})`;
    case "write":
      return d.isNew ? `Create ${d.path}` : `Overwrite ${d.path}`;
    case "fetch":
      return `Fetch ${host(d.url)}`;
    case "mcp":
      return `Use ${d.server} \u00b7 ${d.tool}`;
    case "question":
      return "Has a question";
  }
}

const cut = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1)}\u2026` : s);

/** What a request was about, in a few words: the command's first line, the path, the host or the tool. */
export function subject(d: ToolDetail): string {
  switch (d.kind) {
    case "bash":
      return cut(d.command.split("\n")[0] ?? "", 40);
    case "edit":
    case "write":
      return d.path;
    case "fetch":
      return host(d.url);
    case "mcp":
      return d.tool;
    case "question":
      return "question";
  }
}

/** One line shown after a decision: "Allowed · npm test". */
export function receiptText(d: ToolDetail, verdict: "allowed" | "denied"): string {
  return `${verdict === "allowed" ? "Allowed" : "Denied"} · ${subject(d)}`;
}
```

`src/features/approval/payload.ts`:

```ts
import type { ToolDetail } from "./types";

export const COLLAPSED_LINES = 4;
/** Characters per line in a 12 px mono payload inside a 288 px-wide card, used to estimate wrapped lines. */
const DEFAULT_COLS = 44;
const SIGN = { context: "  ", add: "+ ", del: "- " } as const;

/** The payload as text lines (what the card shows, and what a screen reader reads). */
export function payloadLines(d: ToolDetail): string[] {
  switch (d.kind) {
    case "bash":
      return d.command.split("\n");
    case "edit":
      return d.hunk.map((l) => `${SIGN[l.kind]}${l.text}`);
    case "write":
      return d.preview.split("\n");
    case "fetch":
      return [d.url];
    case "mcp":
      return Object.entries(d.args).map(([k, v]) => `${k}: ${v}`);
    case "question":
      return [d.question];
  }
}

/** Lines after wrapping: every line takes at least one row, plus one per `cols` characters beyond the first. */
export function estimateLines(lines: string[], cols = DEFAULT_COLS): number {
  return lines.reduce((n, l) => n + Math.max(1, Math.ceil(l.length / cols)), 0);
}

export function isTruncated(lines: string[], cols = DEFAULT_COLS): boolean {
  return estimateLines(lines, cols) > COLLAPSED_LINES;
}

export function showAllLabel(total: number): string {
  return `Show all ${total} lines (Space)`;
}
```

- [ ] **Step 3b: Fixtures**

`src/features/approval/fixtures.ts`:

```ts
import type { ApprovalRequest } from "./types";

const MIN = 60_000;
const longCommand = Array.from({ length: 40 }, (_, i) =>
  i === 0 ? "set -euo pipefail" : i % 8 === 0 ? `echo "step ${i}: done"` : `docker build --target stage-${i} -t registry.example.dev/my-app:stage-${i} .`,
).join("\n");

/** One card per state in docs/PLAN.md deliverable 4. Session names match the batch 1 row fixtures where they overlap. */
export const APPROVALS = {
  safeBash: {
    id: "safe-bash",
    session: { name: "billing", agent: "claude" },
    sessionCwd: "C:/dev/payments",
    waitingMs: 65_000,
    detail: { kind: "bash", command: "npm test" },
    risk: { level: "normal" },
    alwaysRule: "Bash(npm test:*)",
    state: { phase: "pending" },
  },
  riskyBash: {
    id: "risky-bash",
    session: { name: "refactor", agent: "claude" },
    sessionCwd: "C:/dev/my-app",
    waitingMs: 12_000,
    detail: { kind: "bash", command: "rm -rf ../old-build", cwd: "C:/dev/my-app/packages/web" },
    risk: { level: "risky", reason: "Deletes files recursively outside this project" },
    state: { phase: "pending" },
  },
  edit: {
    id: "edit",
    session: { name: "api-server", agent: "claude" },
    sessionCwd: "C:/dev/my-app",
    waitingMs: 31_000,
    detail: {
      kind: "edit",
      path: "src/auth.ts",
      add: 12,
      del: 3,
      hunk: [
        { kind: "context", text: "export async function refresh(client: Client) {" },
        { kind: "del", text: "  const token = await client.token();" },
        { kind: "add", text: "  const token = await client.token({ force: true });" },
        { kind: "add", text: "  if (!token) throw new AuthError(\"refresh failed\");" },
        { kind: "context", text: "  return token;" },
        { kind: "context", text: "}" },
      ],
    },
    risk: { level: "normal" },
    alwaysRule: "Edit(src/**)",
    state: { phase: "pending" },
  },
  writeNew: {
    id: "write-new",
    session: { name: "landing", agent: "codex" },
    sessionCwd: "C:/dev/marketing",
    waitingMs: 8_000,
    detail: { kind: "write", path: ".env.example", preview: "API_URL=http://localhost:3000\nSTRIPE_KEY=\nSENTRY_DSN=", isNew: true },
    risk: { level: "normal" },
    state: { phase: "pending" },
  },
  mcp: {
    id: "mcp",
    session: { name: "infra", agent: "claude" },
    sessionCwd: "C:/dev/terraform",
    waitingMs: 95_000,
    detail: { kind: "mcp", server: "github", tool: "create_pr", args: { title: "Upgrade aws provider to 6.x", base: "main", head: "chore/upgrade-aws-provider", draft: "true" } },
    risk: { level: "normal" },
    alwaysRule: "mcp__github__create_pr",
    state: { phase: "pending" },
  },
  longCommand: {
    id: "long-command",
    session: { name: "ci-fix", agent: "gemini" },
    sessionCwd: "C:/dev/my-app",
    waitingMs: 2 * MIN,
    detail: { kind: "bash", command: longCommand },
    risk: { level: "normal" },
    state: { phase: "pending" },
  },
  question: {
    id: "question",
    session: { name: "infra", agent: "gemini" },
    sessionCwd: "C:/dev/terraform",
    waitingMs: 4 * MIN,
    detail: { kind: "question", question: "Which AWS region should the new provider config target?", options: ["us-east-1", "eu-west-1", "ap-southeast-2"] },
    risk: { level: "normal" },
    state: { phase: "pending" },
  },
  receipt: {
    id: "receipt",
    session: { name: "billing", agent: "claude" },
    sessionCwd: "C:/dev/payments",
    waitingMs: 70_000,
    detail: { kind: "bash", command: "npm test" },
    risk: { level: "normal" },
    state: { phase: "decided", verdict: "allowed" },
  },
  answeredInTerminal: {
    id: "answered-in-terminal",
    session: { name: "billing", agent: "claude" },
    sessionCwd: "C:/dev/payments",
    waitingMs: 40_000,
    detail: { kind: "bash", command: "npm test" },
    risk: { level: "normal" },
    state: { phase: "terminal" },
  },
  released: {
    id: "released",
    session: { name: "nightly", agent: "claude" },
    sessionCwd: "C:/dev/perf",
    waitingMs: 120_000,
    detail: { kind: "bash", command: "cargo bench" },
    risk: { level: "normal" },
    state: { phase: "released", afterMs: 120_000 },
  },
} satisfies Record<string, ApprovalRequest>;

export const APPROVAL_LIST: ApprovalRequest[] = Object.values(APPROVALS);
```

- [ ] **Step 4: Run, typecheck, commit**

Run: `pnpm vitest run src/features/approval` (PASS) and `pnpm tsc --noEmit` (clean).

```bash
git add src/features/approval
git commit -m "feat(approval): request model, headline and payload text, and the ten card fixtures

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Arming, hold and key rules (the safety logic)

**Files:**
- Create: `src/features/approval/arming.ts`
- Test: `src/features/approval/arming.test.ts`

**Interfaces:**
- Produces (exact):

```ts
export const ARM_DELAY_MS = 500;
export const HOLD_MS = 600;
export type CardAction = "allow" | "hold-allow" | "deny" | "always" | "terminal" | "expand" | "jump";
export type KeyContext = { risky: boolean; armed: boolean; hasAlways: boolean; truncated: boolean; expanded: boolean };
export function isArmed(armedAtMs: number, nowMs: number): boolean;
export function holdProgress(heldMs: number): number;
export function holdComplete(heldMs: number): boolean;
export function keyAction(key: string, ctx: KeyContext): CardAction | null;
```

- [ ] **Step 1: Write the failing test**

`src/features/approval/arming.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { ARM_DELAY_MS, HOLD_MS, holdComplete, holdProgress, isArmed, keyAction, type KeyContext } from "./arming";

const safe: KeyContext = { risky: false, armed: true, hasAlways: true, truncated: false, expanded: false };
const risky: KeyContext = { ...safe, risky: true, hasAlways: false };

describe("arming", () => {
  it("is inert for 500 ms after the card gains focus or its content changes", () => {
    expect(ARM_DELAY_MS).toBe(500);
    expect(isArmed(1000, 1499)).toBe(false);
    expect(isArmed(1000, 1500)).toBe(true);
  });
});

describe("hold to allow", () => {
  it("needs 600 ms", () => {
    expect(HOLD_MS).toBe(600);
    expect(holdProgress(0)).toBe(0);
    expect(holdProgress(300)).toBe(0.5);
    expect(holdProgress(5000)).toBe(1);
    expect(holdComplete(599)).toBe(false);
    expect(holdComplete(600)).toBe(true);
  });
  it("treats nonsense as no progress", () => {
    expect(holdProgress(-5)).toBe(0);
    expect(holdProgress(Number.NaN)).toBe(0);
  });
});

describe("keyAction on a safe request", () => {
  it("maps Y N A T", () => {
    expect(keyAction("y", safe)).toBe("allow");
    expect(keyAction("Y", safe)).toBe("allow");
    expect(keyAction("n", safe)).toBe("deny");
    expect(keyAction("a", safe)).toBe("always");
    expect(keyAction("t", safe)).toBe("terminal");
  });
  it("never offers Always when there is no rule", () => {
    expect(keyAction("a", { ...safe, hasAlways: false })).toBeNull();
  });
  it("is inert before the arming delay ends", () => {
    for (const k of ["y", "n", "a", "t"]) expect(keyAction(k, { ...safe, armed: false }), k).toBeNull();
  });
});

describe("Enter never allows (review focus 2)", () => {
  it("jumps to the session, even before arming and on a risky card", () => {
    expect(keyAction("Enter", safe)).toBe("jump");
    expect(keyAction("Enter", risky)).toBe("jump");
    expect(keyAction("Enter", { ...risky, armed: false })).toBe("jump");
  });
});

describe("risky requests", () => {
  it("turn Y into a hold, never an instant allow", () => {
    expect(keyAction("y", risky)).toBe("hold-allow");
  });
  it("hide Always", () => {
    expect(keyAction("a", { ...risky, hasAlways: true })).toBeNull();
  });
  it("must have a truncated payload expanded before Allow arms", () => {
    const long = { ...risky, truncated: true, expanded: false };
    expect(keyAction("y", long)).toBe("expand");
    expect(keyAction("y", { ...long, expanded: true })).toBe("hold-allow");
  });
  it("still let Deny through", () => {
    expect(keyAction("n", risky)).toBe("deny");
  });
});

describe("Space", () => {
  it("expands a truncated payload and does nothing otherwise", () => {
    expect(keyAction(" ", { ...safe, truncated: true })).toBe("expand");
    expect(keyAction(" ", { ...safe, truncated: true, expanded: true })).toBeNull();
    expect(keyAction(" ", safe)).toBeNull();
  });
});

describe("other keys", () => {
  it("do nothing, so terminal typing can never reach the card", () => {
    for (const k of ["x", "Escape", "Tab", "1", "Backspace"]) expect(keyAction(k, safe), k).toBeNull();
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `pnpm vitest run src/features/approval/arming.test.ts`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement**

`src/features/approval/arming.ts`:

```ts
export const ARM_DELAY_MS = 500;
export const HOLD_MS = 600;

export type CardAction = "allow" | "hold-allow" | "deny" | "always" | "terminal" | "expand" | "jump";
export type KeyContext = {
  /** Classified risky by the core. */
  risky: boolean;
  /** The 500 ms delay since focus or content change has passed. */
  armed: boolean;
  /** The core offered an exact rule. */
  hasAlways: boolean;
  /** The payload is longer than four lines. */
  truncated: boolean;
  expanded: boolean;
};

/** Buttons and keys are inert until 500 ms after the card gained focus or its content changed. */
export function isArmed(armedAtMs: number, nowMs: number): boolean {
  return nowMs - armedAtMs >= ARM_DELAY_MS;
}

export function holdProgress(heldMs: number): number {
  if (!Number.isFinite(heldMs) || heldMs <= 0) return 0;
  return Math.min(1, heldMs / HOLD_MS);
}

export function holdComplete(heldMs: number): boolean {
  return holdProgress(heldMs) >= 1;
}

/**
 * What a key does while the card has focus (docs/PLAN.md "Keyboard safety"). Enter never allows: it jumps to the
 * session. Unknown keys do nothing, so no terminal keystroke can ever act on a card.
 */
export function keyAction(key: string, ctx: KeyContext): CardAction | null {
  if (key === "Enter") return "jump";
  if (key === " ") return ctx.truncated && !ctx.expanded ? "expand" : null;
  if (!ctx.armed) return null;
  switch (key.toLowerCase()) {
    case "y":
      if (!ctx.risky) return "allow";
      return ctx.truncated && !ctx.expanded ? "expand" : "hold-allow";
    case "n":
      return "deny";
    case "a":
      return ctx.risky || !ctx.hasAlways ? null : "always";
    case "t":
      return "terminal";
    default:
      return null;
  }
}
```

- [ ] **Step 4: Run and commit**

Run: `pnpm vitest run src/features/approval` (PASS), `pnpm tsc --noEmit` (clean).

```bash
git add src/features/approval
git commit -m "feat(approval): arming delay, hold-to-allow and key rules, Enter never allows

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Keymap and shortcut labels (shared)

**Files:**
- Create: `src/lib/keymap.ts`
- Test: `src/lib/keymap.test.ts`

**Interfaces:**
- Produces (exact):

```ts
export type Os = "windows" | "mac" | "linux";
export type Binding = { key: string; alt?: boolean; plain?: { win: string; mac: string } };
export type Command = { id: string; label: string; binding?: Binding; group: "Sessions" | "View" | "Tools" };
export function shortcutLabel(b: Binding, os: Os): string;
export const COMMANDS: Command[];
export function commandById(id: string): Command;
```

- [ ] **Step 1: Write the failing test**

`src/lib/keymap.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { COMMANDS, commandById, shortcutLabel } from "./keymap";

describe("shortcutLabel", () => {
  it("is Ctrl+Shift+key on Windows and Linux, Cmd+key on macOS", () => {
    expect(shortcutLabel({ key: "K" }, "windows")).toBe("Ctrl+Shift+K");
    expect(shortcutLabel({ key: "K" }, "linux")).toBe("Ctrl+Shift+K");
    expect(shortcutLabel({ key: "K" }, "mac")).toBe("\u2318K");
  });
  it("adds Alt and Option", () => {
    expect(shortcutLabel({ key: "T", alt: true }, "windows")).toBe("Ctrl+Shift+Alt+T");
    expect(shortcutLabel({ key: "T", alt: true }, "mac")).toBe("\u2318\u2325T");
  });
  it("keeps keys the plan leaves as written (F2, Ctrl+1)", () => {
    expect(shortcutLabel({ key: "F2", plain: { win: "F2", mac: "F2" } }, "windows")).toBe("F2");
    expect(shortcutLabel({ key: "1", plain: { win: "Ctrl+1", mac: "\u23181" } }, "mac")).toBe("\u23181");
  });
});

describe("COMMANDS", () => {
  it("lists the bindings from docs/PLAN.md section 7", () => {
    const win = (id: string) => shortcutLabel(commandById(id).binding!, "windows");
    expect(win("palette")).toBe("Ctrl+Shift+K");
    expect(win("history")).toBe("Ctrl+Shift+H");
    expect(win("find")).toBe("Ctrl+Shift+F");
    expect(win("split")).toBe("Ctrl+Shift+\\");
    expect(win("bookmark")).toBe("Ctrl+Shift+B");
    expect(win("launcher")).toBe("Ctrl+Shift+T");
    expect(win("reopen")).toBe("Ctrl+Shift+Alt+T");
    expect(win("close")).toBe("Ctrl+Shift+W");
    expect(win("cheatsheet")).toBe("Ctrl+Shift+/");
    expect(win("global-search")).toBe("Ctrl+Shift+Alt+F");
    expect(win("next-waiting")).toBe("Ctrl+Shift+N");
    expect(win("rename")).toBe("F2");
  });
  it("has unique ids and a label for every command", () => {
    expect(new Set(COMMANDS.map((c) => c.id)).size).toBe(COMMANDS.length);
    for (const c of COMMANDS) expect(c.label.length, c.id).toBeGreaterThan(0);
  });
  it("throws a clear error for an unknown id", () => {
    expect(() => commandById("nope")).toThrow("Unknown command: nope");
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `pnpm vitest run src/lib/keymap.test.ts`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement**

`src/lib/keymap.ts`:

```ts
export type Os = "windows" | "mac" | "linux";
/** The key with the platform's app modifier implied; `plain` overrides keys the plan leaves as written. */
export type Binding = { key: string; alt?: boolean; plain?: { win: string; mac: string } };
export type Command = { id: string; label: string; binding?: Binding; group: "Sessions" | "View" | "Tools" };

/** macOS: Cmd+key. Windows and Linux: Ctrl+Shift+key (docs/PLAN.md section 7, "Keyboard routing"). */
export function shortcutLabel(b: Binding, os: Os): string {
  if (b.plain) return os === "mac" ? b.plain.mac : b.plain.win;
  if (os === "mac") return `\u2318${b.alt ? "\u2325" : ""}${b.key}`;
  return `Ctrl+Shift+${b.alt ? "Alt+" : ""}${b.key}`;
}

export const COMMANDS: Command[] = [
  { id: "launcher", label: "Start a session", group: "Sessions", binding: { key: "T" } },
  { id: "next-waiting", label: "Go to the oldest waiting session", group: "Sessions", binding: { key: "N" } },
  { id: "close", label: "Close session", group: "Sessions", binding: { key: "W" } },
  { id: "reopen", label: "Reopen closed session", group: "Sessions", binding: { key: "T", alt: true } },
  { id: "rename", label: "Rename session", group: "Sessions", binding: { key: "F2", plain: { win: "F2", mac: "F2" } } },
  { id: "split", label: "Split view", group: "View", binding: { key: "\\" } },
  { id: "focus-mode", label: "Focus mode", group: "View" },
  { id: "sidebar", label: "Collapse sidebar to the rail", group: "View" },
  { id: "palette", label: "Command palette", group: "Tools", binding: { key: "K" } },
  { id: "history", label: "History", group: "Tools", binding: { key: "H" } },
  { id: "find", label: "Search in scrollback", group: "Tools", binding: { key: "F" } },
  { id: "global-search", label: "Search all sessions", group: "Tools", binding: { key: "F", alt: true } },
  { id: "bookmark", label: "Bookmark this point", group: "Tools", binding: { key: "B" } },
  { id: "cheatsheet", label: "Keyboard shortcuts", group: "Tools", binding: { key: "/" } },
  { id: "doctor", label: "Open Doctor", group: "Tools" },
  { id: "departures", label: "Open Departures board", group: "Tools" },
];

export function commandById(id: string): Command {
  const c = COMMANDS.find((x) => x.id === id);
  if (!c) throw new Error(`Unknown command: ${id}`);
  return c;
}
```

- [ ] **Step 4: Run and commit**

Run: `pnpm vitest run src/lib/keymap.test.ts` (PASS), `pnpm tsc --noEmit` (clean).

```bash
git add src/lib/keymap.ts src/lib/keymap.test.ts
git commit -m "feat(keymap): one keymap and per-OS shortcut labels

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: ApproveCard component, arming and hold hooks, CSS

**Files:**
- Create: `src/features/approval/ApproveCard.tsx`, `src/features/approval/ApproveCard.css`, `src/features/approval/useArming.ts`, `src/features/approval/useHold.ts`
- Test: `src/features/approval/ApproveCard.test.tsx`, `src/features/approval/ApproveCard.css.test.ts`

**Interfaces:**
- Consumes: `ApprovalRequest`, `headline`, `receiptText`, `subject`, `payloadLines`, `isTruncated`, `showAllLabel`, `COLLAPSED_LINES`, `keyAction`, `ARM_DELAY_MS`, `HOLD_MS`, `CardAction`, `shortcutLabel`/`Os`, `StatusGlyph`/`AuxGlyph`, `formatDuration` (from `../sidebar/format`), `AGENT_NAMES`.
- Produces:

```tsx
export type ApproveCardProps = {
  request: ApprovalRequest;
  /** Payload shown in full (the user pressed Space or the card is in the drawer). */
  expanded?: boolean;
  /** False during the 500 ms after focus or a content change. Defaults to true so static stories show live buttons. */
  armed?: boolean;
  /** 0..1, drawn on the risky Allow button's ring. */
  holdProgress?: number;
  /** One-shot: the receipt is leaving (140 ms). */
  exiting?: boolean;
  onAction?: (a: CardAction | `answer:${number}`) => void;
  onHoldEnd?: () => void;
  onFocus?: () => void;
};
export function ApproveCard(props: ApproveCardProps): JSX.Element;
export function useArming(resetKey: unknown): { armed: boolean; rearm: () => void };
export function useHold(onComplete: () => void): { progress: number; begin: () => void; cancel: () => void };
```

- [ ] **Step 1: Write the failing component tests**

`src/features/approval/ApproveCard.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { APPROVALS } from "./fixtures";
import { ApproveCard } from "./ApproveCard";
import type { ApprovalRequest } from "./types";

const html = (r: ApprovalRequest, p: Partial<Parameters<typeof ApproveCard>[0]> = {}) => renderToStaticMarkup(<ApproveCard request={r} {...p} />);

describe("ApproveCard: a safe request", () => {
  const h = html(APPROVALS.safeBash);
  it("says who, what and for how long", () => {
    expect(h).toContain("billing");
    expect(h).toContain("Run a command");
    expect(h).toContain("1m");
    expect(h).toContain("npm test");
  });
  it("offers Deny, Allow once, the exact Always rule and Answer in terminal, each with its key", () => {
    expect(h).toContain("Deny");
    expect(h).toContain("Allow once");
    expect(h).toContain("Bash(npm test:*)");
    expect(h).toContain("in this project");
    expect(h).toContain("Answer in terminal");
    for (const k of ["N", "Y", "A", "T"]) expect(h).toContain(`<kbd class="approve-card__kbd" aria-hidden="true">${k}</kbd>`);
  });
  it("is a labelled group a screen reader can find", () => {
    expect(h).toContain('role="group"');
    expect(h).toMatch(/aria-label="billing: Run a command/);
    expect(h).toContain('tabindex="0"');
  });
  it("does not show the cwd when it matches the session's", () => {
    expect(h).not.toContain("approve-card__cwd");
  });
});

describe("ApproveCard: a risky request", () => {
  const h = html(APPROVALS.riskyBash);
  it("states the plain reason, with a warning glyph", () => {
    expect(h).toContain("Deletes files recursively outside this project");
    expect(h).toContain('data-kind="caution"');
    expect(h).toContain('data-risk="risky"');
  });
  it("turns Allow into a hold with a progress ring, and hides Always", () => {
    expect(h).toContain("Hold to allow");
    expect(h).toContain("approve-card__ring");
    expect(h).not.toContain("Always allow");
  });
  it("shows the cwd when it differs from the session's", () => {
    expect(h).toContain("approve-card__cwd");
    expect(h).toContain("C:/dev/my-app/packages/web");
  });
  it("draws the hold progress", () => {
    expect(html(APPROVALS.riskyBash, { holdProgress: 0.5 })).toContain('stroke-dasharray="18.84955592153876 37.69911184307752"');
  });
});

describe("ApproveCard: payloads", () => {
  it("collapses a 40-line command and offers the real total", () => {
    const h = html(APPROVALS.longCommand);
    expect(h).toContain("data-collapsed");
    expect(h).toContain("Show all 40 lines (Space)");
  });
  it("shows everything once expanded", () => {
    const h = html(APPROVALS.longCommand, { expanded: true });
    expect(h).not.toContain("data-collapsed");
    expect(h).not.toContain("Show all");
  });
  it("marks diff lines with a sign as well as a colour", () => {
    const h = html(APPROVALS.edit);
    expect(h).toContain('data-kind="add"');
    expect(h).toContain('data-kind="del"');
    expect(h).toContain("+ ");
    expect(h).toContain("- ");
    expect(h).toContain("Edit src/auth.ts (+12 \u22123)");
  });
  it("shows MCP arguments as key: value", () => {
    expect(html(APPROVALS.mcp)).toContain("title: Upgrade aws provider to 6.x");
  });
  it("labels a new file as Create", () => {
    expect(html(APPROVALS.writeNew)).toContain("Create .env.example");
  });
  it("renders hostile text as text, never as markup (review focus 1)", () => {
    const evil: ApprovalRequest = { ...APPROVALS.safeBash, detail: { kind: "bash", command: `echo "<img src=x onerror=alert(1)>" ${"A".repeat(300)} \u8a8d\u8a3c \u062e\u0627\u062f\u0645` } };
    const h = html(evil);
    expect(h).not.toContain("<img");
    expect(h).toContain("&lt;img");
    expect(h).toContain('dir="auto"');
  });
});

describe("ApproveCard: a question", () => {
  const h = html(APPROVALS.question);
  it("shows the question and one button per answer, with no Allow", () => {
    expect(h).toContain("Which AWS region should the new provider config target?");
    for (const o of ["us-east-1", "eu-west-1", "ap-southeast-2"]) expect(h).toContain(o);
    expect(h).not.toContain("Allow once");
  });
  it("still offers the terminal", () => {
    expect(h).toContain("Answer in terminal");
  });
});

describe("ApproveCard: after the request", () => {
  it("shows a one-line receipt, no buttons and no undo", () => {
    const h = html(APPROVALS.receipt);
    expect(h).toContain('role="status"');
    expect(h).toContain("Allowed \u00b7 npm test");
    expect(h).not.toContain("<button");
    expect(h.toLowerCase()).not.toContain("undo");
  });
  it("shows a denied receipt", () => {
    const denied: ApprovalRequest = { ...APPROVALS.receipt, state: { phase: "decided", verdict: "denied" } };
    expect(html(denied)).toContain("Denied \u00b7 npm test");
  });
  it("says it was answered in the terminal", () => {
    expect(html(APPROVALS.answeredInTerminal)).toContain("Answered in terminal \u00b7 npm test");
  });
  it("says a timed-out request moved to the terminal, naming the agent and the wait", () => {
    const h = html(APPROVALS.released);
    expect(h).toContain("Timed out after 2m");
    expect(h).toContain("Claude is asking in its own terminal now");
  });
  it("can mark the receipt as leaving", () => {
    expect(html(APPROVALS.receipt, { exiting: true })).toContain("data-exiting");
  });
});

describe("ApproveCard: arming", () => {
  it("marks every action as inert before the delay ends", () => {
    const h = html(APPROVALS.safeBash, { armed: false });
    expect((h.match(/aria-disabled="true"/g) ?? []).length).toBeGreaterThanOrEqual(4);
  });
  it("is live once armed", () => {
    expect(html(APPROVALS.safeBash)).not.toContain('aria-disabled="true"');
  });
});

describe("ApproveCard: markup hygiene", () => {
  it("never puts a literal colour in markup", () => {
    for (const r of Object.values(APPROVALS)) expect(html(r)).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
  it("never uses clever copy", () => {
    for (const r of Object.values(APPROVALS)) {
      const h = html(r).toLowerCase();
      expect(h).not.toContain("successfully");
      expect(h).not.toContain("oops");
    }
  });
});
```

`src/features/approval/ApproveCard.css.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./ApproveCard.css", import.meta.url), "utf8");
const rule = (selector: string) => {
  const i = css.indexOf(`${selector} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("ApproveCard.css", () => {
  it("wraps payload text and never cuts it off mid-line (review focus 1)", () => {
    expect(rule(".approve-card__payload")).toMatch(/overflow-wrap:\s*anywhere/);
    expect(rule(".approve-card__payload")).toMatch(/white-space:\s*pre-wrap/);
  });
  it("fades a collapsed payload and clips it at four lines", () => {
    const r = rule(".approve-card__payload[data-collapsed]");
    expect(r).toContain("mask-image");
    expect(r).toMatch(/overflow:\s*clip/);
  });
  it("shows focus with :focus-visible and an outline", () => {
    expect(css).toContain(":focus-visible");
    expect(css).not.toMatch(/outline:\s*none/);
  });
  it("only styles hover on devices that hover", () => {
    const hoverRules = css.match(/:hover/g)?.length ?? 0;
    expect(hoverRules).toBeGreaterThan(0);
    expect(css.slice(css.indexOf("@media (hover: hover) and (pointer: fine)"))).toContain(":hover");
    expect(css.slice(0, css.indexOf("@media (hover: hover) and (pointer: fine)"))).not.toContain(":hover");
  });
  it("gives pressable things an active state", () => {
    expect(css).toContain(".approve-card__btn:active");
  });
  it("names transition properties and animates the receipt exit with tokens", () => {
    expect(css).not.toMatch(/transition:\s*all/);
    expect(css).not.toContain("ease-in");
    expect(css).toContain("var(--dur-exit)");
  });
  it("uses tokens for colour, not literals", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
  });
  it("uses logical properties", () => {
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).not.toMatch(/\b(left|right|top|bottom):/);
  });
});
```

- [ ] **Step 2: Run to see them fail**

Run: `pnpm vitest run src/features/approval/ApproveCard`
Expected: FAIL (modules not found).

- [ ] **Step 3: Implement the hooks**

`src/features/approval/useArming.ts`:

```ts
import { useCallback, useEffect, useRef, useState } from "react";
import { ARM_DELAY_MS } from "./arming";

/** False for 500 ms after mount, after `resetKey` changes, and after `rearm()` (called on focus). */
export function useArming(resetKey: unknown): { armed: boolean; rearm: () => void } {
  const [armed, setArmed] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  const rearm = useCallback(() => {
    setArmed(false);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setArmed(true), ARM_DELAY_MS);
  }, []);
  useEffect(() => {
    rearm();
    return () => window.clearTimeout(timer.current);
  }, [resetKey, rearm]);
  return { armed, rearm };
}
```

`src/features/approval/useHold.ts`:

```ts
import { useCallback, useEffect, useRef, useState } from "react";
import { HOLD_MS, holdProgress } from "./arming";

/**
 * Hold-to-allow for risky requests: `begin()` on key or pointer down, `cancel()` on release. `onComplete` fires once
 * after 600 ms. Timing uses requestAnimationFrame, so it is exercised by the Interactive story, not by unit tests;
 * the numbers themselves are tested in arming.test.ts.
 */
export function useHold(onComplete: () => void): { progress: number; begin: () => void; cancel: () => void } {
  const [progress, setProgress] = useState(0);
  const startedAt = useRef<number | null>(null);
  const frame = useRef(0);

  const cancel = useCallback(() => {
    startedAt.current = null;
    cancelAnimationFrame(frame.current);
    setProgress(0);
  }, []);

  const tick = useCallback(() => {
    if (startedAt.current === null) return;
    const held = performance.now() - startedAt.current;
    setProgress(holdProgress(held));
    if (held >= HOLD_MS) {
      startedAt.current = null;
      onComplete();
    } else frame.current = requestAnimationFrame(tick);
  }, [onComplete]);

  const begin = useCallback(() => {
    if (startedAt.current !== null) return;
    startedAt.current = performance.now();
    frame.current = requestAnimationFrame(tick);
  }, [tick]);

  useEffect(() => cancel, [cancel]);
  return { progress, begin, cancel };
}
```

- [ ] **Step 4: Implement the component**

`src/features/approval/ApproveCard.tsx`:

```tsx
import { useId, type KeyboardEvent } from "react";
import { AGENT_NAMES } from "../../components/AgentMark/agents";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { formatDuration } from "../sidebar/format";
import { keyAction, type CardAction } from "./arming";
import { headline, receiptText, subject } from "./headline";
import { isTruncated, payloadLines, showAllLabel } from "./payload";
import type { ApprovalRequest } from "./types";
import "./ApproveCard.css";

export type ApproveCardProps = {
  request: ApprovalRequest;
  expanded?: boolean;
  armed?: boolean;
  holdProgress?: number;
  exiting?: boolean;
  onAction?: (a: CardAction | `answer:${number}`) => void;
  onHoldEnd?: () => void;
  onFocus?: () => void;
};

const RING_R = 6;
const RING_C = 2 * Math.PI * RING_R;

function Ring({ progress }: { progress: number }) {
  return (
    <svg className="approve-card__ring" width={16} height={16} viewBox="0 0 16 16" aria-hidden="true">
      <circle className="approve-card__ring-track" cx="8" cy="8" r={RING_R} fill="none" strokeWidth="2" />
      <circle
        className="approve-card__ring-value"
        cx="8"
        cy="8"
        r={RING_R}
        fill="none"
        strokeWidth="2"
        strokeDasharray={`${RING_C * progress} ${RING_C}`}
        transform="rotate(-90 8 8)"
      />
    </svg>
  );
}

const Kbd = ({ k }: { k: string }) => (
  <kbd className="approve-card__kbd" aria-hidden="true">
    {k}
  </kbd>
);

/**
 * The approve card (docs/PLAN.md "The approve card"). Presentational: the parent owns the request, the arming timer
 * (useArming) and the hold timer (useHold). Keys act only while the card has focus; Enter never allows.
 */
export function ApproveCard({ request, expanded = false, armed = true, holdProgress = 0, exiting = false, onAction, onHoldEnd, onFocus }: ApproveCardProps) {
  const riskId = useId();
  const { detail, risk, state, session } = request;
  const agent = AGENT_NAMES[session.agent];

  if (state.phase === "decided") {
    return (
      <div className="approve-card approve-card--receipt" role="status" data-phase="decided" data-verdict={state.verdict} data-exiting={exiting || undefined}>
        <span aria-hidden="true">
          <StatusGlyph kind={state.verdict === "allowed" ? "done-seen" : "error"} size={12} />
        </span>
        <span className="approve-card__receipt-text" dir="auto">
          {receiptText(detail, state.verdict)}
        </span>
      </div>
    );
  }
  if (state.phase === "terminal") {
    return (
      <div className="approve-card approve-card--receipt" role="status" data-phase="terminal">
        <span className="approve-card__receipt-text" dir="auto">
          Answered in terminal {"\u00b7"} {subject(detail)}
        </span>
      </div>
    );
  }
  if (state.phase === "released") {
    return (
      <div className="approve-card approve-card--receipt" role="status" data-phase="released">
        <span className="approve-card__receipt-text">
          Timed out after {formatDuration(state.afterMs)}. {agent} is asking in its own terminal now.
        </span>
      </div>
    );
  }

  const lines = payloadLines(detail);
  const truncated = isTruncated(lines);
  const collapsed = truncated && !expanded;
  const risky = risk.level === "risky";
  const isQuestion = detail.kind === "question";
  const cwd = detail.kind === "bash" && detail.cwd && detail.cwd !== request.sessionCwd ? detail.cwd : null;
  const ctx = { risky, armed, hasAlways: Boolean(request.alwaysRule), truncated, expanded };

  const fire = (a: CardAction | `answer:${number}`) => {
    if (!armed && a !== "jump" && a !== "expand") return;
    onAction?.(a);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.ctrlKey || e.metaKey || e.altKey || e.repeat) return;
    const a = keyAction(e.key, ctx);
    if (!a) return;
    e.preventDefault();
    onAction?.(a);
  };
  const onKeyUp = (e: KeyboardEvent) => {
    if (risky && e.key.toLowerCase() === "y") onHoldEnd?.();
  };

  const disabled = armed ? undefined : true;

  return (
    <article
      className="approve-card"
      role="group"
      aria-label={`${session.name}: ${headline(detail)}`}
      tabIndex={0}
      data-phase="pending"
      data-risk={risk.level}
      data-armed={armed}
      onKeyDown={onKeyDown}
      onKeyUp={onKeyUp}
      onFocus={onFocus}
    >
      <header className="approve-card__who">
        <StatusGlyph kind={isQuestion ? "needs-question" : "needs-permission"} />
        <span className="approve-card__name" dir="auto">
          {session.name}
        </span>
        <span className="approve-card__wait">{formatDuration(request.waitingMs)}</span>
        <button type="button" className="approve-card__menu" aria-label="More actions">
          <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
            <circle cx="3.5" cy="8" r="1.25" fill="currentColor" />
            <circle cx="8" cy="8" r="1.25" fill="currentColor" />
            <circle cx="12.5" cy="8" r="1.25" fill="currentColor" />
          </svg>
        </button>
      </header>

      <h3 className="approve-card__what" dir="auto">
        {headline(detail)}
      </h3>

      {isQuestion ? (
        <p className="approve-card__question" dir="auto">
          {detail.question}
        </p>
      ) : detail.kind === "edit" ? (
        <pre className="approve-card__payload" dir="auto" data-collapsed={collapsed || undefined}>
          {detail.hunk.map((l, i) => (
            <code key={i} className="approve-card__line" data-kind={l.kind}>
              {l.kind === "add" ? "+ " : l.kind === "del" ? "- " : "  "}
              {l.text}
              {"\n"}
            </code>
          ))}
        </pre>
      ) : (
        <pre className="approve-card__payload" dir="auto" data-collapsed={collapsed || undefined}>
          {lines.join("\n")}
        </pre>
      )}

      {collapsed && (
        <button type="button" className="approve-card__expand" onClick={() => fire("expand")} aria-keyshortcuts="Space">
          {showAllLabel(lines.length)}
        </button>
      )}
      {cwd && (
        <p className="approve-card__cwd" dir="auto">
          in {cwd}
        </p>
      )}
      {risky && risk.reason && (
        <p className="approve-card__risk" id={riskId}>
          <AuxGlyph kind="caution" label="Risky" />
          <span>{risk.reason}</span>
        </p>
      )}

      {isQuestion ? (
        <div className="approve-card__actions approve-card__actions--options" role="group" aria-label="Answers">
          {detail.options.map((o, i) => (
            <button key={o} type="button" className="approve-card__btn" data-kind="option" aria-disabled={disabled} onClick={() => fire(`answer:${i}`)}>
              {o}
            </button>
          ))}
        </div>
      ) : (
        <div className="approve-card__actions">
          <button type="button" className="approve-card__btn" data-kind="deny" aria-disabled={disabled} aria-keyshortcuts="N" onClick={() => fire("deny")}>
            Deny <Kbd k="N" />
          </button>
          {risky ? (
            <button
              type="button"
              className="approve-card__btn"
              data-kind="allow"
              data-holding={holdProgress > 0 || undefined}
              aria-disabled={disabled}
              aria-keyshortcuts="Y"
              aria-describedby={riskId}
              onPointerDown={() => fire(collapsed ? "expand" : "hold-allow")}
              onPointerUp={onHoldEnd}
              onPointerLeave={onHoldEnd}
            >
              <Ring progress={holdProgress} />
              Hold to allow <Kbd k="Y" />
            </button>
          ) : (
            <button type="button" className="approve-card__btn" data-kind="allow" aria-disabled={disabled} aria-keyshortcuts="Y" onClick={() => fire("allow")}>
              Allow once <Kbd k="Y" />
            </button>
          )}
        </div>
      )}

      {!risky && !isQuestion && request.alwaysRule && (
        <button type="button" className="approve-card__link" aria-disabled={disabled} aria-keyshortcuts="A" onClick={() => fire("always")}>
          <span>
            Always allow <code>{request.alwaysRule}</code> in this project
          </span>
          <Kbd k="A" />
        </button>
      )}
      <button type="button" className="approve-card__link" aria-disabled={disabled} aria-keyshortcuts="T" onClick={() => fire("terminal")}>
        <span>Answer in terminal</span>
        <Kbd k="T" />
      </button>
    </article>
  );
}
```

- [ ] **Step 5: Implement the CSS**

`src/features/approval/ApproveCard.css`:

```css
.approve-card {
  display: grid;
  gap: var(--space-2);
  padding: var(--space-3);
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
  background: var(--bg-raised);
  color: var(--text-1);
  font-size: var(--text-13);
  line-height: 20px;
  font-variant-numeric: tabular-nums;
  outline-offset: 2px;
}
.approve-card:focus-visible {
  outline: 2px solid var(--accent);
}
.approve-card[data-risk="risky"] {
  border-color: var(--error);
}

.approve-card__who {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.approve-card__name {
  flex: 1 1 auto;
  min-inline-size: 0;
  overflow: clip;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 600;
}
.approve-card__wait {
  flex: none;
  color: var(--text-2);
  font-size: var(--text-11);
}
.approve-card__menu {
  display: grid;
  place-items: center;
  flex: none;
  inline-size: 24px;
  block-size: 24px;
  padding: 0;
  border: 0;
  border-radius: var(--radius);
  background: transparent;
  color: var(--text-2);
}
.approve-card__menu:focus-visible {
  outline: 2px solid var(--accent);
}
.approve-card__menu:active {
  background: var(--selected);
}

.approve-card__what {
  margin: 0;
  font-size: var(--text-13);
  font-weight: 500;
  overflow-wrap: anywhere;
}

.approve-card__payload {
  margin: 0;
  padding: var(--space-2);
  border-radius: 4px;
  background: var(--bg-overlay);
  font-family: var(--font-mono);
  font-size: var(--text-12);
  line-height: 18px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  unicode-bidi: plaintext;
}
/* After four lines the payload fades; "Show all N lines" opens the rest. */
.approve-card__payload[data-collapsed] {
  max-block-size: calc(4 * 18px + 2 * var(--space-2));
  overflow: clip;
  mask-image: linear-gradient(to bottom, oklch(0% 0 none) 55%, transparent);
}
.approve-card__line {
  display: block;
  font: inherit;
}
.approve-card__line[data-kind="add"] {
  background: color-mix(in oklch, var(--ok) 16%, transparent);
}
.approve-card__line[data-kind="del"] {
  background: color-mix(in oklch, var(--error) 16%, transparent);
}

.approve-card__question {
  margin: 0;
  overflow-wrap: anywhere;
}

.approve-card__expand,
.approve-card__link {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding-block: 4px;
  padding-inline: 0;
  border: 0;
  background: transparent;
  color: var(--text-2);
  font: inherit;
  font-size: var(--text-12);
  text-align: start;
}
.approve-card__expand {
  justify-content: flex-start;
}
.approve-card__link code {
  font-family: var(--font-mono);
  color: var(--text-1);
}
.approve-card__expand:focus-visible,
.approve-card__link:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
.approve-card__expand:active,
.approve-card__link:active {
  color: var(--text-1);
}

.approve-card__cwd {
  margin: 0;
  color: var(--text-3);
  font-family: var(--font-mono);
  font-size: var(--text-11);
  overflow-wrap: anywhere;
}

.approve-card__risk {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  margin: 0;
  color: var(--text-1);
  font-size: var(--text-12);
  line-height: 16px;
}
.approve-card__risk > .aux-glyph {
  margin-block-start: 2px;
}

.approve-card__actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}
.approve-card__actions--options {
  flex-wrap: wrap;
  justify-content: flex-start;
}

.approve-card__btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-block-size: 32px;
  padding-block: 6px;
  padding-inline: var(--space-3);
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
  background: var(--bg-overlay);
  color: var(--text-1);
  font: inherit;
  font-weight: 500;
}
.approve-card__btn[data-kind="allow"] {
  border-color: var(--accent);
  background: var(--accent);
  color: var(--accent-ink);
}
.approve-card__btn[aria-disabled="true"] {
  opacity: 0.55;
}
.approve-card__btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
.approve-card__btn:active:not([aria-disabled="true"]) {
  box-shadow: inset 0 0 0 100vmax var(--selected);
}
@media (hover: hover) and (pointer: fine) {
  .approve-card__btn:hover:not([aria-disabled="true"]) {
    box-shadow: inset 0 0 0 100vmax var(--hover);
  }
  .approve-card__expand:hover,
  .approve-card__link:hover {
    color: var(--text-1);
  }
  .approve-card__menu:hover {
    background: var(--hover);
  }
}

.approve-card__kbd {
  padding-inline: 4px;
  border: 1px solid color-mix(in oklch, currentColor 40%, transparent);
  border-radius: 3px;
  font-family: var(--font-mono);
  font-size: var(--text-11);
  font-weight: 400;
  line-height: 16px;
}

.approve-card__ring {
  flex: none;
}
.approve-card__ring-track {
  stroke: color-mix(in oklch, var(--accent-ink) 30%, transparent);
}
.approve-card__ring-value {
  stroke: var(--accent-ink);
}

/* After the request: a one-line receipt, no buttons. */
.approve-card--receipt {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding-block: var(--space-2);
  background: var(--bg-base);
  color: var(--text-2);
  font-size: var(--text-12);
  line-height: 16px;
}
.approve-card__receipt-text {
  min-inline-size: 0;
  overflow-wrap: anywhere;
}

@keyframes approve-card-out {
  to {
    opacity: 0;
    transform: translateY(-4px);
  }
}
@media (prefers-reduced-motion: no-preference) {
  :root:not([data-motion="reduce"]) .approve-card--receipt[data-exiting] {
    animation: approve-card-out var(--dur-exit) var(--ease-out) forwards;
  }
}
@media (prefers-reduced-motion: reduce) {
  .approve-card--receipt[data-exiting] {
    opacity: 0;
  }
}
:root[data-motion="reduce"] .approve-card--receipt[data-exiting] {
  opacity: 0;
}

/* Windows High Contrast: borders and the allow button use system colours. */
@media (forced-colors: active) {
  .approve-card,
  .approve-card__btn {
    border-color: CanvasText;
  }
  .approve-card__btn[data-kind="allow"] {
    forced-color-adjust: none;
    background: Highlight;
    color: HighlightText;
  }
  .approve-card__ring-value {
    stroke: HighlightText;
  }
}
```

- [ ] **Step 6: Run, fix, commit**

Run: `pnpm vitest run src/features/approval` then `pnpm vitest run` and `pnpm tsc --noEmit`.
Expected: all PASS. If the hover-media CSS test fails because the `:hover` in `@media` also appears in a comment, move the comment; if the `stroke-dasharray` assertion differs by float formatting, copy the exact string React prints from the failure output (it is `2π·6·0.5 = 18.84955592153876`).

```bash
git add src/features/approval
git commit -m "feat(approval): approve card in every state, with arming and hold-to-allow

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Card stories, review page for sub-review A, accessibility gate

**Files:**
- Create: `src/features/approval/ApproveCard.stories.tsx`
- Modify: `src/design/ThemePair.tsx` and `ThemePair.css` (optional `stack` prop), `src/design/review/renderReview.tsx`, `scripts/build-review.mjs`, `.storybook/preview.tsx`, `package.json` (add `axe-core`)

**Interfaces:**
- Consumes: everything from Tasks 1–4.
- Produces: `renderReview(upTo: "1" | "2a" | "2b" | "2c")` (cumulative: batch 1 sections, then each signed-off sub-review); `ThemePair` gains `stack?: boolean`.

- [ ] **Step 1: Write the failing test for the review renderer**

`src/design/review/renderReview.test.tsx`:

```tsx
import { describe, expect, it } from "vitest";
import { renderReview } from "./renderReview";

describe("renderReview", () => {
  it("is cumulative: 2a contains batch 1 and the approve card", () => {
    const html = renderReview("2a");
    expect(html).toContain('id="sidebar-session-rows"');
    expect(html).toContain('id="approve-card"');
  });
  it("batch 1 alone has no approve card", () => {
    expect(renderReview("1")).not.toContain('id="approve-card"');
  });
  it("renders every approve card story", () => {
    const html = renderReview("2a");
    for (const name of ["Safe bash", "Risky bash", "Edit with diff", "Write new file", "Mcp tool", "Long command", "Question card", "After decision", "Answered in terminal", "Released on timeout"])
      expect(html, name).toContain(name);
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `pnpm vitest run src/design/review`
Expected: FAIL ("approve-card" id missing, or `renderReview` ignores its argument).

- [ ] **Step 3: Stack option for ThemePair**

`src/design/ThemePair.tsx`: change the signature to `({ children, label, stack = false }: { children: ReactNode; label?: string; stack?: boolean })` and the `<section>` to `<section className="theme-pair" data-stack={stack || undefined} aria-label={label}>`. In `ThemePair.css` add:

```css
.theme-pair[data-stack] {
  grid-template-columns: minmax(0, 1fr);
}
```

- [ ] **Step 4: Stories**

`src/features/approval/ApproveCard.stories.tsx`:

```tsx
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ThemePair } from "../../design/ThemePair";
import { APPROVALS } from "./fixtures";
import { ApproveCard } from "./ApproveCard";
import { useArming } from "./useArming";
import { useHold } from "./useHold";
import type { ApprovalRequest } from "./types";

const meta: Meta = { title: "Approve card" };
export default meta;

/** The lane's width: the 288 px sidebar minus 8 px gutters. */
const W = 272;

function Card({ request, ...rest }: { request: ApprovalRequest } & Partial<Parameters<typeof ApproveCard>[0]>) {
  return (
    <div style={{ inlineSize: W }}>
      <ApproveCard request={request} {...rest} />
    </div>
  );
}
const one = (label: string, request: ApprovalRequest, extra: Partial<Parameters<typeof ApproveCard>[0]> = {}): StoryObj => ({
  render: () => (
    <ThemePair label={label}>
      <Card request={request} {...extra} />
    </ThemePair>
  ),
});

export const SafeBash: StoryObj = one("Safe command", APPROVALS.safeBash);
export const RiskyBash: StoryObj = one("Risky command, Allow is a hold", APPROVALS.riskyBash);
export const EditWithDiff: StoryObj = one("Edit with the first hunk", APPROVALS.edit);
export const WriteNewFile: StoryObj = one("Write of a new file", APPROVALS.writeNew);
export const McpTool: StoryObj = one("MCP tool, arguments as key: value", APPROVALS.mcp);
export const LongCommand: StoryObj = {
  render: () => (
    <ThemePair label="A 40-line command, collapsed then expanded">
      <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
        <Card request={APPROVALS.longCommand} />
        <Card request={APPROVALS.longCommand} expanded />
      </div>
    </ThemePair>
  ),
};
export const QuestionCard: StoryObj = one("Question", APPROVALS.question);
export const AfterDecision: StoryObj = {
  render: () => (
    <ThemePair label="After a decision">
      <div style={{ display: "grid", gap: "var(--space-2)", justifyItems: "start" }}>
        <Card request={APPROVALS.receipt} />
        <Card request={{ ...APPROVALS.receipt, state: { phase: "decided", verdict: "denied" } }} />
      </div>
    </ThemePair>
  ),
};
export const AnsweredInTerminal: StoryObj = one("Answered in the terminal", APPROVALS.answeredInTerminal);
export const ReleasedOnTimeout: StoryObj = one("Released on timeout", APPROVALS.released);
export const BeforeArming: StoryObj = one("First 500 ms: every action is inert", APPROVALS.safeBash, { armed: false });
export const HoldInProgress: StoryObj = one("Holding Y on a risky request (50%)", APPROVALS.riskyBash, { holdProgress: 0.5 });

/** Try it: Tab to the card, wait 500 ms, then Y / N / A / T / Space. Enter jumps and never allows. On the risky one, hold Y. */
function Interactive({ initial }: { initial: ApprovalRequest }) {
  const [request, setRequest] = useState(initial);
  const [expanded, setExpanded] = useState(false);
  const [log, setLog] = useState("Focus the card, then press a key.");
  const { armed, rearm } = useArming(request.id);
  const hold = useHold(() => decide("allowed"));
  function decide(verdict: "allowed" | "denied") {
    setRequest((r) => ({ ...r, state: { phase: "decided", verdict } }));
    setLog(`Decided: ${verdict}`);
  }
  return (
    <div style={{ inlineSize: W, display: "grid", gap: "var(--space-2)" }}>
      <ApproveCard
        request={request}
        expanded={expanded}
        armed={armed}
        holdProgress={hold.progress}
        onFocus={rearm}
        onHoldEnd={hold.cancel}
        onAction={(a) => {
          if (a === "allow") decide("allowed");
          else if (a === "deny") decide("denied");
          else if (a === "hold-allow") hold.begin();
          else if (a === "expand") setExpanded(true);
          else if (a === "terminal") setRequest((r) => ({ ...r, state: { phase: "terminal" } }));
          else setLog(`Action: ${a}`);
        }}
      />
      <p style={{ margin: 0, color: "var(--text-3)", fontSize: "var(--text-11)" }}>{log}</p>
    </div>
  );
}
export const InteractiveSafe: StoryObj = { render: () => <Interactive initial={APPROVALS.safeBash} /> };
export const InteractiveRisky: StoryObj = { render: () => <Interactive initial={APPROVALS.riskyBash} /> };
```

- [ ] **Step 5: Renderer**

In `src/design/review/renderReview.tsx`: import `* as Card from "../../features/approval/ApproveCard.stories";`; rename `BATCH_1` usage; add

```tsx
const BATCH_2A: Array<[StoryModule, string]> = [
  [Card, "The card in every state from the plan: safe and risky commands, edit with diff, new file, MCP, a 40-line command, a question, the receipt, answered in terminal and released on timeout. Interactive stories need Storybook."],
];

type UpTo = "1" | "2a" | "2b" | "2c";
const ORDER: UpTo[] = ["1", "2a", "2b", "2c"];
const BATCHES: Record<UpTo, Array<[StoryModule, string]>> = { "1": BATCH_1, "2a": BATCH_2A, "2b": [], "2c": [] };

/** Server-renders every story up to and including the given batch into page sections. */
export function renderReview(upTo: UpTo = "2a"): string {
  const modules = ORDER.slice(0, ORDER.indexOf(upTo) + 1).flatMap((k) => BATCHES[k]);
  return modules.map(([mod, blurb]) => { /* existing body unchanged */ }).join("");
}
```

Keep the existing per-module body. The section `id` comes from the story title, so `"Approve card"` gives `id="approve-card"`, which the test expects.

- [ ] **Step 6: Build script**

In `scripts/build-review.mjs`:
- add to `CSS`: `"src/components/AgentMark/..."` already present; add `"src/features/approval/ApproveCard.css"` after the SessionRow entry;
- read the batch from `process.argv[3] ?? "2a"` and pass it: `sections = renderReview(batch)`;
- replace the hard-coded lede, nav and sign-off checklist with a `META` lookup keyed by batch:

```js
const META = {
  "2a": {
    lede: "Sub-review A of 3 in batch 2: the approve card in every state. Batch 1 (tokens, glyphs, rows) is below it, signed off. Windows and launcher follow once this is approved.",
    nav: [["approve-card", "Approve card"], ["sidebar-session-rows", "Rows"], ["design-colours", "Colours"], ["sign-off", "Sign-off"]],
    checks: [
      "From a glance you can tell who is asking, what for and how long they have waited.",
      "The risky card is unmistakable without colour: the warning line, the Hold to allow label and the missing Always.",
      "The 40-line command fades after four lines and Show all reveals it; a long unbroken token wraps instead of overflowing.",
      "The receipt, answered in terminal and timed-out states read as finished, with no hint of an undo.",
      "In Storybook: Tab to the interactive card, press Y before 500 ms (nothing happens), then Y, N, A, T, Space; Enter never allows.",
      "It still reads well at 150% browser zoom, and the focus ring is visible in both themes.",
    ],
  },
};
```

and emit the nav links and `<li>` items from `META[batch]`.

- [ ] **Step 7: Storybook accessibility mode**

In `.storybook/preview.tsx` change `a11y: { test: "todo" }` to `a11y: { test: "error" }` (batch 1 deferred minor; the gate below is the check that actually runs).

- [ ] **Step 8: Run the tests**

Run: `pnpm vitest run` and `pnpm tsc --noEmit`. Expected: PASS.

- [ ] **Step 9: Build the page and run the accessibility gate**

```bash
pnpm add -D axe-core
pnpm review:build "$TEMP/review.html" 2a
cp node_modules/axe-core/axe.min.js "$TEMP/axe.min.js"
```

A static server must be running for `$TEMP` (`cd "$TEMP" && python -m http.server 6011 --bind 127.0.0.1`, started in the background if it is not). Then, with the Playwright MCP browser: navigate to `http://127.0.0.1:6011/review.html` and run

```js
async (page) => {
  await page.addScriptTag({ url: "http://127.0.0.1:6011/axe.min.js" });
  const out = [];
  for (const scheme of ["dark", "light"]) {
    await page.emulateMedia({ colorScheme: scheme });
    const r = await page.evaluate(async () => (await axe.run(document, { resultTypes: ["violations"] })).violations.map((v) => ({ id: v.id, impact: v.impact, nodes: v.nodes.length, sample: v.nodes[0].html.slice(0, 160) })));
    out.push({ scheme, violations: r });
  }
  return out;
}
```

Expected: no violation with impact `serious` or `critical` in either scheme. Fix real findings in the component or CSS (not by disabling rules) and rebuild; `aria-disabled` buttons are exempt from contrast by axe. Record every remaining `moderate` or `minor` finding in the ledger as `Task 5: minor (deferred): …`.

- [ ] **Step 10: Visual check**

Take screenshots (`browser_take_screenshot`) of the safe, risky, edit and long-command cards in dark and light at the page's default width and at 150% zoom (`browser_resize` to 960 wide). Read them and confirm: the payload fade is visible, the risky border and warning line read, the Allow button label never clips, kbd chips align. Fix anything that does not.

- [ ] **Step 11: Commit, then publish for sign-off**

```bash
git add -A src scripts .storybook package.json pnpm-lock.yaml
git commit -m "feat(ui): approve card stories and review page for sub-review A, axe gate

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

Build the page to the scratchpad file (`marshell-design-review.html`), read the CSS head and the first sections of it before publishing (files are read before publishing), then publish to the existing review artifact (`https://claude.ai/artifact/FMbHBbzc8TmqzQRRW5BCX3`) with the label "Sub-review A: approve card". Tell the user what to check (the page's own checklist) and wait for sign-off before Task 6. The user pushes commits.

---

## Sub-review B: the main window

### Task 6: Terminal palettes and theme resolution

**Files:**
- Create: `src/features/terminal/palettes.ts`, `src/features/terminal/terminalTheme.ts`
- Delete: `src/features/terminal/theme.ts`
- Modify: `src/features/terminal/TerminalView.tsx`
- Test: `src/features/terminal/palettes.test.ts`, `src/features/terminal/terminalTheme.test.ts`

**Interfaces:**
- Consumes: `contrast`, `deltaE` from `src/design/contrast.ts`; `parseTokens` from `src/design/cssTokens.ts`.
- Produces (exact):

```ts
// palettes.ts
export type Scheme = "dark" | "light";
export const MIN_CONTRAST = 4.5;
export const PALETTES: Record<Scheme, ITheme>; // from "@xterm/xterm"
// terminalTheme.ts
export type TerminalThemeSetting = "follow-app" | "dark" | "light";
export function resolveScheme(setting: TerminalThemeSetting, app: Scheme): Scheme;
export function schemeFromComputed(colorScheme: string, prefersDark: boolean): Scheme;
export function appScheme(root?: HTMLElement): Scheme;
export function terminalOptions(scheme: Scheme): { theme: ITheme; minimumContrastRatio: number };
```

- [ ] **Step 1: Write the failing tests**

`src/features/terminal/palettes.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { contrast, deltaE } from "../../design/contrast";
import { parseTokens } from "../../design/cssTokens";
import { MIN_CONTRAST, PALETTES } from "./palettes";

const tokens = parseTokens(readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8"), ":root");
const ANSI = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white", "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightMagenta", "brightCyan", "brightWhite"] as const;

for (const scheme of ["dark", "light"] as const) {
  describe(`${scheme} terminal palette`, () => {
    const p = PALETTES[scheme];
    const bg = p.background!;
    it("sits flush with the app's base surface", () => {
      expect(deltaE(bg, tokens["--bg-base"]![scheme])).toBeLessThan(1);
    });
    it("reads at 7:1 for default text", () => {
      expect(contrast(p.foreground!, bg)).toBeGreaterThanOrEqual(7);
    });
    it("keeps the cursor visible", () => {
      expect(contrast(p.cursor!, bg)).toBeGreaterThanOrEqual(3);
    });
    it("keeps selected text readable", () => {
      expect(contrast(p.foreground!, p.selectionBackground!)).toBeGreaterThanOrEqual(4.5);
    });
    it("defines all sixteen ANSI colours", () => {
      for (const n of ANSI) expect(p[n], n).toMatch(/^#[0-9a-f]{6}$/);
    });
    for (const n of ANSI)
      // Dark "black" is the one colour that cannot be readable on black; contrast protection lifts it (review focus 4).
      if (!(scheme === "dark" && n === "black"))
        it(`${n} holds ${MIN_CONTRAST}:1 on its own background`, () => {
          expect(contrast(p[n]!, bg)).toBeGreaterThanOrEqual(MIN_CONTRAST);
        });
  });
}

describe("contrast protection", () => {
  it("defaults to 4.5", () => {
    expect(MIN_CONTRAST).toBe(4.5);
  });
});
```

`src/features/terminal/terminalTheme.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { MIN_CONTRAST, PALETTES } from "./palettes";
import { resolveScheme, schemeFromComputed, terminalOptions } from "./terminalTheme";

describe("resolveScheme", () => {
  it("follows the app by default", () => {
    expect(resolveScheme("follow-app", "dark")).toBe("dark");
    expect(resolveScheme("follow-app", "light")).toBe("light");
  });
  it("can be fixed either way regardless of the app", () => {
    expect(resolveScheme("dark", "light")).toBe("dark");
    expect(resolveScheme("light", "dark")).toBe("light");
  });
});

describe("schemeFromComputed", () => {
  it("reads a forced scheme", () => {
    expect(schemeFromComputed("dark", false)).toBe("dark");
    expect(schemeFromComputed("light", true)).toBe("light");
  });
  it("falls back to the OS preference for 'light dark' or nothing", () => {
    expect(schemeFromComputed("light dark", true)).toBe("dark");
    expect(schemeFromComputed("light dark", false)).toBe("light");
    expect(schemeFromComputed("", true)).toBe("dark");
    expect(schemeFromComputed("normal", false)).toBe("light");
  });
});

describe("terminalOptions", () => {
  it("returns the palette with contrast protection on", () => {
    const o = terminalOptions("light");
    expect(o.theme).toBe(PALETTES.light);
    expect(o.minimumContrastRatio).toBe(MIN_CONTRAST);
  });
});
```

- [ ] **Step 2: Run to see them fail**

Run: `pnpm vitest run src/features/terminal`
Expected: FAIL (modules not found).

- [ ] **Step 3: Implement**

`src/features/terminal/palettes.ts` (the only place hex colours are allowed: xterm cannot read CSS variables; every value is checked in the test above):

```ts
import type { ITheme } from "@xterm/xterm";

export type Scheme = "dark" | "light";
/** Passed to xterm's minimumContrastRatio so colours a program painted for the other scheme stay readable. */
export const MIN_CONTRAST = 4.5;

export const PALETTES: Record<Scheme, ITheme> = {
  dark: {
    background: "#000000",
    foreground: "#e5e5e5",
    cursor: "#e5e5e5",
    cursorAccent: "#000000",
    selectionBackground: "#3a3a3a",
    black: "#3a3a3a",
    red: "#ff6b6b",
    green: "#5ad07a",
    yellow: "#e5c14a",
    blue: "#6ea8ff",
    magenta: "#d28bff",
    cyan: "#4fd1d1",
    white: "#c9c9c9",
    brightBlack: "#8a8a8a",
    brightRed: "#ff8a8a",
    brightGreen: "#7ee89a",
    brightYellow: "#ffd866",
    brightBlue: "#8fbaff",
    brightMagenta: "#e0a8ff",
    brightCyan: "#7de3e3",
    brightWhite: "#ffffff",
  },
  light: {
    background: "#ffffff",
    foreground: "#1c1917",
    cursor: "#1c1917",
    cursorAccent: "#ffffff",
    selectionBackground: "#d6d3d1",
    black: "#1c1917",
    red: "#c62828",
    green: "#1b7a3a",
    yellow: "#8a5a00",
    blue: "#1f5fbf",
    magenta: "#8e3bb8",
    cyan: "#0e7a85",
    white: "#6b6b6b",
    brightBlack: "#6e6e6e",
    brightRed: "#d32f2f",
    brightGreen: "#2e7d32",
    brightYellow: "#946200",
    brightBlue: "#2b6fd6",
    brightMagenta: "#a14fcc",
    brightCyan: "#11808b",
    brightWhite: "#4a4a4a",
  },
};
```

`src/features/terminal/terminalTheme.ts`:

```ts
import type { ITheme } from "@xterm/xterm";
import { MIN_CONTRAST, PALETTES, type Scheme } from "./palettes";

export type { Scheme };
/** docs/PLAN.md "Terminal theme": follow the app by default, or pin the terminal dark or light. */
export type TerminalThemeSetting = "follow-app" | "dark" | "light";

export function resolveScheme(setting: TerminalThemeSetting, app: Scheme): Scheme {
  return setting === "follow-app" ? app : setting;
}

/** The scheme an element renders in, from its computed `color-scheme` and the OS preference as the fallback. */
export function schemeFromComputed(colorScheme: string, prefersDark: boolean): Scheme {
  if (colorScheme === "dark") return "dark";
  if (colorScheme === "light") return "light";
  return prefersDark ? "dark" : "light";
}

/** The scheme the app is showing right now (browser only). */
export function appScheme(root: HTMLElement = document.documentElement): Scheme {
  return schemeFromComputed(getComputedStyle(root).colorScheme, matchMedia("(prefers-color-scheme: dark)").matches);
}

export function terminalOptions(scheme: Scheme): { theme: ITheme; minimumContrastRatio: number } {
  return { theme: PALETTES[scheme], minimumContrastRatio: MIN_CONTRAST };
}
```

- [ ] **Step 4: Wire the live terminal**

In `src/features/terminal/TerminalView.tsx`: replace `import { currentTheme } from "./theme";` with `import { appScheme, resolveScheme, terminalOptions } from "./terminalTheme";`. Replace the `new Terminal({ … theme: currentTheme() })` call with:

```ts
const apply = () => terminalOptions(resolveScheme("follow-app", appScheme()));
const term = new Terminal({ fontFamily, fontSize: 13, cursorBlink: true, scrollback: 10_000, ...apply() });
```

and replace the `onScheme` body with:

```ts
const onScheme = () => {
  const o = apply();
  term.options.theme = o.theme;
  term.options.minimumContrastRatio = o.minimumContrastRatio;
};
```

Also observe `data-theme` changes so a manual app theme switch applies live: after `scheme.addEventListener("change", onScheme);` add

```ts
const themeWatch = new MutationObserver(onScheme);
themeWatch.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
```

and in the cleanup, before `scheme.removeEventListener`, add `themeWatch.disconnect();`. Delete `src/features/terminal/theme.ts` (`git rm`).

- [ ] **Step 5: Run and commit**

Run: `pnpm vitest run` (PASS) and `pnpm tsc --noEmit` (clean). If a palette test fails, adjust that one hex in `palettes.ts` toward more contrast and re-run; record a `Ruling:` line.

```bash
git add -A src/features/terminal
git commit -m "feat(terminal): full dark and light palettes, follow-app resolution, contrast protection

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Scripted Claude-style session and the ScriptedTerminal component

**Files:**
- Create: `src/features/terminal/scriptedSession.ts`, `src/features/terminal/ScriptedTerminal.tsx`, `src/features/terminal/ScriptedTerminal.css`
- Test: `src/features/terminal/scriptedSession.test.ts`, `src/features/terminal/ScriptedTerminal.test.tsx`

**Interfaces:**
- Consumes: `terminalOptions`, `resolveScheme`, `schemeFromComputed`, `TerminalThemeSetting`, `Scheme`, `PALETTES`, `MIN_CONTRAST`.
- Produces:

```ts
// scriptedSession.ts
export const SCRIPTED_SIZE: { cols: number; rows: number };   // { cols: 76, rows: 26 }
export const SCRIPTED_LINES: string[];                         // each line is raw ANSI text, no line ending
export const SCRIPTED_SESSION: string;                         // SCRIPTED_LINES joined with "\r\n", ending "\r\n"
// ScriptedTerminal.tsx
export function ScriptedTerminal(props: { setting?: TerminalThemeSetting; label?: string }): JSX.Element;
```

- [ ] **Step 1: Write the failing tests**

`src/features/terminal/scriptedSession.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { contrast } from "../../design/contrast";
import { MIN_CONTRAST, PALETTES } from "./palettes";
import { SCRIPTED_LINES, SCRIPTED_SESSION, SCRIPTED_SIZE } from "./scriptedSession";

const rgbs = [...SCRIPTED_SESSION.matchAll(/\x1b\[38;2;(\d+);(\d+);(\d+)m/g)].map((m) => `#${[1, 2, 3].map((i) => Number(m[i]).toString(16).padStart(2, "0")).join("")}`);

describe("scripted session", () => {
  it("fits its stated size without wrapping", () => {
    expect(SCRIPTED_LINES.length).toBeLessThanOrEqual(SCRIPTED_SIZE.rows);
    for (const l of SCRIPTED_LINES) {
      const visible = l.replace(/\x1b\[[0-9;]*m/g, "");
      expect([...visible].length, visible).toBeLessThanOrEqual(SCRIPTED_SIZE.cols);
    }
  });
  it("ends with a line ending and uses CRLF between lines", () => {
    expect(SCRIPTED_SESSION.endsWith("\r\n")).toBe(true);
    expect(SCRIPTED_SESSION).not.toMatch(/[^\r]\n/);
  });
  it("uses the 16 palette colours, so a theme switch visibly changes them", () => {
    expect(SCRIPTED_SESSION).toMatch(/\x1b\[3[1-6]m/);
    expect(SCRIPTED_SESSION).toMatch(/\x1b\[9[0-7]m/);
  });
  it("paints some 24-bit colours that fail 4.5:1 on a dark terminal and some on a light one (so protection has work to do)", () => {
    expect(rgbs.length).toBeGreaterThan(2);
    expect(rgbs.some((c) => contrast(c, PALETTES.dark.background!) < MIN_CONTRAST)).toBe(true);
    expect(rgbs.some((c) => contrast(c, PALETTES.light.background!) < MIN_CONTRAST)).toBe(true);
  });
  it("contains the beats of a Claude Code session", () => {
    const visible = SCRIPTED_SESSION.replace(/\x1b\[[0-9;]*m/g, "");
    for (const s of ["Claude Code", "retry", "Read(", "Update(", "Bash(npm test)", "passed"]) expect(visible).toContain(s);
  });
  it("draws its welcome box exactly 60 columns wide on every row", () => {
    const box = SCRIPTED_LINES.filter((l) => /^\x1b\[38;2;217;119;87m[\u256d\u2502\u2570]/.test(l)).map((l) => [...l.replace(/\x1b\[[0-9;]*m/g, "")].length);
    expect(box.length).toBe(4);
    for (const w of box) expect(w).toBe(60);
  });
});
```

`src/features/terminal/ScriptedTerminal.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ScriptedTerminal } from "./ScriptedTerminal";

describe("ScriptedTerminal (server markup)", () => {
  it("renders a labelled host element that the client fills with a real xterm", () => {
    const h = renderToStaticMarkup(<ScriptedTerminal />);
    expect(h).toContain('class="scripted-terminal"');
    expect(h).toContain('role="group"');
    expect(h).toContain('aria-label="Terminal preview"');
    expect(h).toContain('data-setting="follow-app"');
  });
  it("carries the setting so the page script can apply it", () => {
    expect(renderToStaticMarkup(<ScriptedTerminal setting="light" />)).toContain('data-setting="light"');
  });
  it("uses a role that allows the terminal's own focusable input inside (not img)", () => {
    expect(renderToStaticMarkup(<ScriptedTerminal />)).not.toContain('role="img"');
  });
});
```

- [ ] **Step 2: Run to see them fail**

Run: `pnpm vitest run src/features/terminal`
Expected: FAIL (modules not found).

- [ ] **Step 3: Implement the session**

`src/features/terminal/scriptedSession.ts`:

```ts
export const SCRIPTED_SIZE = { cols: 76, rows: 26 };

const ESC = "\x1b[";
const reset = `${ESC}0m`;
const sgr = (codes: string, text: string) => `${ESC}${codes}m${text}${reset}`;
const rgb = (r: number, g: number, b: number, text: string) => sgr(`38;2;${r};${g};${b}`, text);
const bg = (r: number, g: number, b: number, text: string) => sgr(`48;2;${r};${g};${b}`, text);
const orange = (t: string) => rgb(217, 119, 87, t);
/** Greys a program picks for a dark terminal: light ones vanish on white, dark ones on black. */
const hintGrey = (t: string) => rgb(170, 170, 170, t);
const deepGrey = (t: string) => rgb(48, 48, 48, t);
const dim = (t: string) => sgr("90", t);
const red = (t: string) => sgr("31", t);
const green = (t: string) => sgr("32", t);
const yellow = (t: string) => sgr("33", t);
const cyan = (t: string) => sgr("36", t);
const bold = (t: string) => sgr("1", t);

const BOX = 60;
const top = (title: string) => {
  const head = `\u256d\u2500\u2500 ${title} `;
  return orange(head + "\u2500".repeat(BOX - [...head].length - 1) + "\u256e");
};
const bottom = () => orange(`\u2570${"\u2500".repeat(BOX - 2)}\u256f`);
const boxRow = (plain: string, style: (t: string) => string) =>
  `${orange("\u2502")} ${style(plain)}${" ".repeat(BOX - 4 - [...plain].length)} ${orange("\u2502")}`;

/** A short Claude Code session as raw terminal output: palette colours, 24-bit colours and background fills. */
export const SCRIPTED_LINES: string[] = [
  top("Claude Code"),
  boxRow("Welcome back", bold),
  boxRow("Opus 5.5 \u00b7 high effort \u00b7 C:/dev/payments", hintGrey),
  bottom(),
  "",
  `${sgr("1;36", ">")} fix the flaky retry test in billing`,
  "",
  `${green("\u25cf")} I'll read the test and the retry helper first.`,
  "",
  `${green("\u25cf")} ${bold("Read")}(src/billing/retry.ts)`,
  `  ${dim("\u23bf")}  ${hintGrey("Read 84 lines")}`,
  `${green("\u25cf")} ${bold("Read")}(src/billing/retry.test.ts)`,
  `  ${dim("\u23bf")}  ${hintGrey("Read 61 lines")}`,
  "",
  `${green("\u25cf")} The backoff uses the wall clock, so the test races it. Fixing the helper.`,
  "",
  `${green("\u25cf")} ${bold("Update")}(src/billing/retry.ts)`,
  `  ${dim("\u23bf")}  ${deepGrey("Updated with 2 additions and 1 removal")}`,
  `     ${bg(70, 30, 34, "- const wait = Date.now() + base * 2 ** attempt;")}`,
  `     ${bg(28, 66, 40, "+ const wait = clock.now() + base * 2 ** attempt;")}`,
  `     ${bg(28, 66, 40, "+ await clock.sleep(wait - clock.now());")}`,
  "",
  `${green("\u25cf")} ${bold("Bash")}(npm test)`,
  `  ${dim("\u23bf")}  ${green("PASS")} src/billing/retry.test.ts ${dim("(0.41s)")}`,
  `     ${yellow("1 warning")}: ${cyan("clock.now")} is deprecated in tests`,
  `     ${green("12 passed")}, ${red("0 failed")}`,
  "",
  `${dim("? for shortcuts")}`,
];

export const SCRIPTED_SESSION: string;                         // SCRIPTED_LINES joined with "\r\n", ending "\r\n"
// ScriptedTerminal.tsx
export function ScriptedTerminal(props: { setting?: TerminalThemeSetting; label?: string }): JSX.Element;
```

- [ ] **Step 1: Write the failing tests**

`src/features/terminal/scriptedSession.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { contrast } from "../../design/contrast";
import { MIN_CONTRAST, PALETTES } from "./palettes";
import { SCRIPTED_LINES, SCRIPTED_SESSION, SCRIPTED_SIZE } from "./scriptedSession";

const rgbs = [...SCRIPTED_SESSION.matchAll(/\x1b\[38;2;(\d+);(\d+);(\d+)m/g)].map((m) => `#${[1, 2, 3].map((i) => Number(m[i]).toString(16).padStart(2, "0")).join("")}`);

describe("scripted session", () => {
  it("fits its stated size without wrapping", () => {
    expect(SCRIPTED_LINES.length).toBeLessThanOrEqual(SCRIPTED_SIZE.rows);
    for (const l of SCRIPTED_LINES) {
      const visible = l.replace(/\x1b\[[0-9;]*m/g, "");
      expect([...visible].length, visible).toBeLessThanOrEqual(SCRIPTED_SIZE.cols);
    }
  });
  it("ends with a line ending and uses CRLF between lines", () => {
    expect(SCRIPTED_SESSION.endsWith("\r\n")).toBe(true);
    expect(SCRIPTED_SESSION).not.toMatch(/[^\r]\n/);
  });
  it("uses the 16 palette colours, so a theme switch visibly changes them", () => {
    expect(SCRIPTED_SESSION).toMatch(/\x1b\[3[1-6]m/);
    expect(SCRIPTED_SESSION).toMatch(/\x1b\[9[0-7]m/);
  });
  it("paints some 24-bit colours that fail 4.5:1 on a dark terminal and some on a light one (so protection has work to do)", () => {
    expect(rgbs.length).toBeGreaterThan(2);
    expect(rgbs.some((c) => contrast(c, PALETTES.dark.background!) < MIN_CONTRAST)).toBe(true);
    expect(rgbs.some((c) => contrast(c, PALETTES.light.background!) < MIN_CONTRAST)).toBe(true);
  });
  it("contains the beats of a Claude Code session", () => {
    for (const s of ["Claude Code", "retry", "Read(", "Update(", "Bash(npm test)", "passed"]) expect(SCRIPTED_SESSION).toContain(s);
  });
});
```

`src/features/terminal/ScriptedTerminal.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ScriptedTerminal } from "./ScriptedTerminal";

describe("ScriptedTerminal (server markup)", () => {
  it("renders a labelled host element that the client fills with a real xterm", () => {
    const h = renderToStaticMarkup(<ScriptedTerminal />);
    expect(h).toContain('class="scripted-terminal"');
    expect(h).toContain('role="group"');
    expect(h).toContain('aria-label="Terminal preview"');
    expect(h).toContain('data-setting="follow-app"');
  });
  it("carries the setting so the page script can apply it", () => {
    expect(renderToStaticMarkup(<ScriptedTerminal setting="light" />)).toContain('data-setting="light"');
  });
  it("uses a role that allows the terminal's own focusable input inside (not img)", () => {
    expect(renderToStaticMarkup(<ScriptedTerminal />)).not.toContain('role="img"');
  });
});
```

- [ ] **Step 2: Run to see them fail**

Run: `pnpm vitest run src/features/terminal`
Expected: FAIL (modules not found).

- [ ] **Step 3: Implement the session**

`src/features/terminal/scriptedSession.ts`:

```ts
export const SCRIPTED_SIZE = { cols: 76, rows: 26 };

const ESC = "\x1b[";
const reset = `${ESC}0m`;
const sgr = (codes: string, text: string) => `${ESC}${codes}m${text}${reset}`;
const rgb = (r: number, g: number, b: number, text: string) => sgr(`38;2;${r};${g};${b}`, text);
const bg = (r: number, g: number, b: number, text: string) => sgr(`48;2;${r};${g};${b}`, text);
const orange = (t: string) => rgb(217, 119, 87, t);
/** Greys a program picks for a dark terminal: light ones vanish on white, dark ones on black. */
const hintGrey = (t: string) => rgb(170, 170, 170, t);
const deepGrey = (t: string) => rgb(48, 48, 48, t);
const dim = (t: string) => sgr("90", t);
const red = (t: string) => sgr("31", t);
const green = (t: string) => sgr("32", t);
const yellow = (t: string) => sgr("33", t);
const cyan = (t: string) => sgr("36", t);
const bold = (t: string) => sgr("1", t);

/** A short Claude Code session as raw terminal output: palette colours, 24-bit colours and background fills. */
export const SCRIPTED_LINES: string[] = [
  orange("\u256d\u2500\u2500 Claude Code \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u256e"),
  `${orange("\u2502")} ${bold("Welcome back")}                                                ${orange("\u2502")}`,
  `${orange("\u2502")} ${hintGrey("Opus 5.5 \u00b7 high effort \u00b7 C:/dev/payments")}                   ${orange("\u2502")}`,
  orange("\u2570\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u256f"),
  "",
  `${sgr("1;36", ">")} fix the flaky retry test in billing`,
  "",
  `${green("\u25cf")} I'll read the test and the retry helper first.`,
  "",
  `${green("\u25cf")} ${bold("Read")}(src/billing/retry.ts)`,
  `  ${dim("\u23bf")}  ${hintGrey("Read 84 lines")}`,
  `${green("\u25cf")} ${bold("Read")}(src/billing/retry.test.ts)`,
  `  ${dim("\u23bf")}  ${hintGrey("Read 61 lines")}`,
  "",
  `${green("\u25cf")} The backoff uses the wall clock, so the test races it. Fixing the helper.`,
  "",
  `${green("\u25cf")} ${bold("Update")}(src/billing/retry.ts)`,
  `  ${dim("\u23bf")}  ${deepGrey("Updated with 2 additions and 1 removal")}`,
  `     ${bg(70, 30, 34, "- const wait = Date.now() + base * 2 ** attempt;")}`,
  `     ${bg(28, 66, 40, "+ const wait = clock.now() + base * 2 ** attempt;")}`,
  `     ${bg(28, 66, 40, "+ await clock.sleep(wait - clock.now());")}`,
  "",
  `${green("\u25cf")} ${bold("Bash")}(npm test)`,
  `  ${dim("\u23bf")}  ${green("PASS")} src/billing/retry.test.ts ${dim("(0.41s)")}`,
  `     ${yellow("1 warning")}: ${cyan("clock.now")} is deprecated in tests`,
  `     ${green("12 passed")}, ${red("0 failed")}`,
  "",
  `${dim("? for shortcuts")}`,
];

export const SCRIPTED_SESSION = `${SCRIPTED_LINES.join("\r\n")}\r\n`;
```

- [ ] **Step 4: Implement the component**

`src/features/terminal/ScriptedTerminal.tsx`:

```tsx
import "@xterm/xterm/css/xterm.css";
import { useEffect, useRef } from "react";
import { SCRIPTED_SESSION, SCRIPTED_SIZE } from "./scriptedSession";
import { appScheme, resolveScheme, terminalOptions, type TerminalThemeSetting } from "./terminalTheme";
import "./ScriptedTerminal.css";

/**
 * A real xterm.js fed canned Claude-style output. On the server it renders only the labelled host; the client
 * (Storybook, or the review page's inlined script) mounts xterm into it and follows the app theme live.
 * xterm is imported inside the effect: the library touches browser globals at load, so a top-level import would
 * break server rendering and the node test environment. The host is a group, not an img: xterm puts a focusable
 * input inside it.
 */
export function ScriptedTerminal({ setting = "follow-app", label = "Terminal preview" }: { setting?: TerminalThemeSetting; label?: string }) {
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    let cancelled = false;
    let dispose = () => {};

    void (async () => {
      const [{ Terminal }, { FitAddon }] = await Promise.all([import("@xterm/xterm"), import("@xterm/addon-fit")]);
      if (cancelled) return;
      const fontFamily = getComputedStyle(document.documentElement).getPropertyValue("--font-mono").trim();
      const scheme = () => resolveScheme(setting, appScheme(el));
      const term = new Terminal({
        fontFamily,
        fontSize: 13,
        cols: SCRIPTED_SIZE.cols,
        rows: SCRIPTED_SIZE.rows,
        disableStdin: true,
        cursorBlink: false,
        scrollback: 1000,
        ...terminalOptions(scheme()),
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      term.open(el);
      term.write(SCRIPTED_SESSION);
      fit.fit();

      const apply = () => {
        const o = terminalOptions(scheme());
        term.options.theme = o.theme;
        term.options.minimumContrastRatio = o.minimumContrastRatio;
      };
      const prefers = matchMedia("(prefers-color-scheme: dark)");
      prefers.addEventListener("change", apply);
      const watch = new MutationObserver(apply);
      watch.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
      const resize = new ResizeObserver(() => fit.fit());
      resize.observe(el);

      dispose = () => {
        prefers.removeEventListener("change", apply);
        watch.disconnect();
        resize.disconnect();
        term.dispose();
      };
    })();

    return () => {
      cancelled = true;
      dispose();
    };
  }, [setting]);

  return <div ref={host} className="scripted-terminal" role="group" aria-label={label} data-setting={setting} />;
}
```

`src/features/terminal/ScriptedTerminal.css`:

```css
.scripted-terminal {
  min-block-size: 0;
  min-inline-size: 0;
  block-size: 100%;
  inline-size: 100%;
  overflow: clip;
  padding: var(--space-2);
  background: var(--bg-base);
}
.scripted-terminal .xterm-viewport {
  overflow: clip;
}
```

(The padded host takes the app's base surface so the terminal sits flush with it; the palette test pins the two together.)

- [ ] **Step 5: Run and commit**

Run: `pnpm vitest run` (PASS) and `pnpm tsc --noEmit` (clean).

```bash
git add src/features/terminal
git commit -m "feat(terminal): scripted Claude-style session and a live ScriptedTerminal that follows the theme

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Needs-you lane

**Files:**
- Create: `src/features/lane/lane.ts`, `src/features/lane/LaneRow.tsx`, `src/features/lane/Lane.tsx`, `src/features/lane/Lane.css`, `src/features/lane/Lane.stories.tsx`
- Test: `src/features/lane/lane.test.ts`, `src/features/lane/Lane.test.tsx`, `src/features/lane/Lane.css.test.ts`

**Interfaces:**
- Consumes: `ApprovalRequest`, `ApproveCard`, `headline`, `formatDuration`, `StatusGlyph`, `APPROVALS`.
- Produces (exact):

```ts
// lane.ts
export const LANE = { header: 28, expanded: 176, collapsed: 36, maxShare: 0.4 } as const;
export function pending(items: ApprovalRequest[]): ApprovalRequest[];       // phase === "pending" only
export function orderLane(items: ApprovalRequest[]): ApprovalRequest[];     // oldest waiting first; ties by id
export function laneHeading(count: number): string;                        // "All clear" | "Needs you · 2"
export function laneLayout(count: number, sidebarHeight: number): { height: number; scrolls: boolean; hiddenCount: number };
// Lane.tsx
export function Lane(props: { requests: ApprovalRequest[]; sidebarHeight: number }): JSX.Element;
```

- [ ] **Step 1: Write the failing logic tests**

`src/features/lane/lane.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { APPROVALS } from "../approval/fixtures";
import type { ApprovalRequest } from "../approval/types";
import { LANE, laneHeading, laneLayout, orderLane, pending } from "./lane";

const req = (id: string, waitingMs: number, phase: "pending" | "terminal" = "pending"): ApprovalRequest => ({
  ...APPROVALS.safeBash,
  id,
  waitingMs,
  state: phase === "pending" ? { phase: "pending" } : { phase: "terminal" },
});

describe("order", () => {
  it("is oldest waiting first, so waiting is first-in first-out", () => {
    expect(orderLane([req("b", 10), req("a", 500), req("c", 60)]).map((r) => r.id)).toEqual(["a", "c", "b"]);
  });
  it("breaks ties by id so the order never flickers", () => {
    expect(orderLane([req("z", 5), req("a", 5)]).map((r) => r.id)).toEqual(["a", "z"]);
  });
  it("does not mutate its input", () => {
    const input = [req("b", 1), req("a", 2)];
    orderLane(input);
    expect(input.map((r) => r.id)).toEqual(["b", "a"]);
  });
  it("counts only pending requests", () => {
    expect(pending([req("a", 1), req("b", 2, "terminal")]).map((r) => r.id)).toEqual(["a"]);
  });
});

describe("heading", () => {
  it("says All clear when nothing waits", () => {
    expect(laneHeading(0)).toBe("All clear");
  });
  it("counts what waits", () => {
    expect(laneHeading(1)).toBe("Needs you \u00b7 1");
    expect(laneHeading(2)).toBe("Needs you \u00b7 2");
  });
});

describe("layout (review focus 3)", () => {
  it("keeps the 28 px header even when nothing waits", () => {
    expect(laneLayout(0, 800)).toEqual({ height: LANE.header, scrolls: false, hiddenCount: 0 });
  });
  it("sizes one expanded card and a 36 px row for each other", () => {
    expect(laneLayout(1, 1000)).toEqual({ height: LANE.header + LANE.expanded, scrolls: false, hiddenCount: 0 });
    expect(laneLayout(2, 1000)).toEqual({ height: LANE.header + LANE.expanded + LANE.collapsed, scrolls: false, hiddenCount: 0 });
  });
  it("never exceeds 40% of the sidebar, scrolling instead and counting what is hidden", () => {
    const l = laneLayout(8, 800);
    expect(l.height).toBe(320);
    expect(l.scrolls).toBe(true);
    expect(l.hiddenCount).toBe(8 - 1 - Math.floor((320 - LANE.header - LANE.expanded) / LANE.collapsed));
  });
  it("copes with twelve requests in a short sidebar", () => {
    const l = laneLayout(12, 400);
    expect(l.height).toBeLessThanOrEqual(0.4 * 400);
    expect(l.scrolls).toBe(true);
    expect(l.hiddenCount).toBe(11);
  });
  it("keeps the header for nonsense heights", () => {
    expect(laneLayout(3, 0).height).toBe(LANE.header);
    expect(laneLayout(3, Number.NaN).height).toBe(LANE.header);
    expect(laneLayout(-1, 800).height).toBe(LANE.header);
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `pnpm vitest run src/features/lane/lane.test.ts`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement the logic**

`src/features/lane/lane.ts`:

```ts
import type { ApprovalRequest } from "../approval/types";

/** docs/PLAN.md "Layout": header always 28 px; the targeted card expands (up to 176 px), the others are 36 px one-liners; at most 40% of the sidebar. */
export const LANE = { header: 28, expanded: 176, collapsed: 36, maxShare: 0.4 } as const;

export const pending = (items: ApprovalRequest[]): ApprovalRequest[] => items.filter((r) => r.state.phase === "pending");

/** Oldest waiting first (docs/PLAN.md section 7: first in, first out). Ties break by id. */
export function orderLane(items: ApprovalRequest[]): ApprovalRequest[] {
  return [...items].sort((a, b) => b.waitingMs - a.waitingMs || a.id.localeCompare(b.id));
}

export function laneHeading(count: number): string {
  return count > 0 ? `Needs you \u00b7 ${count}` : "All clear";
}

export function laneLayout(count: number, sidebarHeight: number): { height: number; scrolls: boolean; hiddenCount: number } {
  const n = Number.isFinite(count) ? Math.max(0, Math.floor(count)) : 0;
  if (n === 0) return { height: LANE.header, scrolls: false, hiddenCount: 0 };
  const max = Number.isFinite(sidebarHeight) && sidebarHeight > 0 ? Math.floor(sidebarHeight * LANE.maxShare) : 0;
  const natural = LANE.header + LANE.expanded + (n - 1) * LANE.collapsed;
  if (natural <= max) return { height: natural, scrolls: false, hiddenCount: 0 };
  const fits = Math.max(0, Math.floor((max - LANE.header - LANE.expanded) / LANE.collapsed));
  return { height: Math.max(LANE.header, max), scrolls: true, hiddenCount: Math.max(0, n - 1 - fits) };
}
```

- [ ] **Step 4: Run the logic tests, then write the failing component tests**

Run: `pnpm vitest run src/features/lane/lane.test.ts` (PASS). Then create:

`src/features/lane/Lane.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { APPROVALS } from "../approval/fixtures";
import { Lane } from "./Lane";

const html = (requests = [APPROVALS.safeBash, APPROVALS.question], h = 800) => renderToStaticMarkup(<Lane requests={requests} sidebarHeight={h} />);

describe("Lane", () => {
  it("always renders its header, saying All clear when empty", () => {
    const h = html([]);
    expect(h).toContain("lane__header");
    expect(h).toContain("All clear");
    expect(h).not.toContain("approve-card");
  });
  it("expands only the oldest waiting request and shows the rest as one-liners", () => {
    const h = html();
    expect((h.match(/data-phase="pending"/g) ?? []).length).toBe(1);
    expect(h).toContain("Which AWS region");
    expect((h.match(/class="lane-row"/g) ?? []).length).toBe(1);
    expect(h.indexOf("Which AWS region")).toBeLessThan(h.indexOf("lane-row"));
  });
  it("tells a screen reader how many need you", () => {
    expect(html()).toContain('role="status"');
    expect(html()).toContain("Needs you \u00b7 2");
  });
  it("shows +N more only when it scrolls", () => {
    expect(html()).not.toContain("more");
    const many = Array.from({ length: 12 }, (_, i) => ({ ...APPROVALS.safeBash, id: `r${i}`, waitingMs: 1000 * (i + 1) }));
    const h = html(many, 400);
    expect(h).toContain("+11 more");
    expect(h).toContain('data-scrolls="true"');
  });
  it("ignores requests that are no longer pending", () => {
    expect(html([APPROVALS.receipt])).toContain("All clear");
  });
});
```

`src/features/lane/Lane.css.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Lane.css", import.meta.url), "utf8");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("Lane.css", () => {
  it("fixes the header at 28 px so the lane never pops in or out", () => {
    expect(rule(".lane__header")).toContain("block-size: 28px");
  });
  it("makes one-liners 36 px", () => {
    expect(rule(".lane-row")).toContain("block-size: 36px");
  });
  it("scrolls internally and fades the cut edge", () => {
    expect(rule(".lane__list")).toMatch(/overflow-y:\s*auto/);
    expect(css).toContain("mask-image");
  });
  it("styles focus and press", () => {
    expect(css).toContain(".lane-row:focus-visible");
    expect(css).toContain(".lane-row:active");
    expect(css).not.toMatch(/outline:\s*none/);
  });
  it("uses tokens and logical properties", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
  });
});
```

- [ ] **Step 5: Run to see them fail**

Run: `pnpm vitest run src/features/lane`
Expected: logic PASS; component tests FAIL (modules not found).

- [ ] **Step 6: Implement the components**

`src/features/lane/LaneRow.tsx`:

```tsx
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { headline } from "../approval/headline";
import type { ApprovalRequest } from "../approval/types";
import { formatDuration } from "../sidebar/format";

/** A waiting request that is not the target: a 36 px one-liner. */
export function LaneRow({ request }: { request: ApprovalRequest }) {
  const isQuestion = request.detail.kind === "question";
  const what = headline(request.detail);
  return (
    <button type="button" className="lane-row" aria-label={`${request.session.name}: ${what}, waiting ${formatDuration(request.waitingMs)}`} data-agent={request.session.agent}>
      <StatusGlyph kind={isQuestion ? "needs-question" : "needs-permission"} />
      <span className="lane-row__name" dir="auto">
        {request.session.name}
      </span>
      <span className="lane-row__what" dir="auto">
        {what}
      </span>
      <span className="lane-row__time">{formatDuration(request.waitingMs)}</span>
    </button>
  );
}
```

`src/features/lane/Lane.tsx`:

```tsx
import { ApproveCard } from "../approval/ApproveCard";
import type { ApprovalRequest } from "../approval/types";
import { LaneRow } from "./LaneRow";
import { laneHeading, laneLayout, orderLane, pending } from "./lane";
import "./Lane.css";

/**
 * The needs-you lane (docs/PLAN.md "Layout"). The header is always present so the lane never pops in or out. The
 * oldest waiting request is the target and shows as a full card; the rest are one-liners. It is capped at 40% of the
 * sidebar and scrolls inside that, with "+N more".
 */
export function Lane({ requests, sidebarHeight }: { requests: ApprovalRequest[]; sidebarHeight: number }) {
  const ordered = orderLane(pending(requests));
  const layout = laneLayout(ordered.length, sidebarHeight);
  const heading = laneHeading(ordered.length);
  return (
    <section className="lane" aria-label="Needs you" style={{ blockSize: layout.height }} data-scrolls={layout.scrolls || undefined} data-count={ordered.length}>
      <header className="lane__header">
        <h2 className="lane__heading" role="status">
          {heading}
        </h2>
        {layout.hiddenCount > 0 && <span className="lane__more">+{layout.hiddenCount} more</span>}
      </header>
      {ordered.length > 0 && (
        <div className="lane__list" role="list">
          {ordered.map((r, i) => (
            <div key={r.id} role="listitem" className="lane__item">
              {i === 0 ? <ApproveCard request={r} /> : <LaneRow request={r} />}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
```

`src/features/lane/Lane.css`:

```css
.lane {
  display: grid;
  grid-template-rows: 28px minmax(0, 1fr);
  min-block-size: 0;
  border-block-end: 1px solid var(--hairline);
  background: var(--bg-raised);
}

.lane__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  block-size: 28px;
  padding-inline: 14px 12px;
}
.lane__heading {
  margin: 0;
  color: var(--text-2);
  font-size: var(--text-11);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.lane[data-count]:not([data-count="0"]) .lane__heading {
  color: var(--text-1);
}
.lane__more {
  color: var(--text-2);
  font-size: var(--text-11);
}

.lane__list {
  display: grid;
  align-content: start;
  gap: var(--space-1);
  min-block-size: 0;
  padding-inline: var(--space-2);
  padding-block-end: var(--space-2);
  overflow-y: auto;
  overscroll-behavior: contain;
}
.lane[data-scrolls] .lane__list {
  mask-image: linear-gradient(to bottom, oklch(0% 0 none) calc(100% - 24px), transparent);
}

.lane-row {
  display: grid;
  grid-template-columns: auto minmax(0, auto) minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--space-2);
  block-size: 36px;
  padding-inline: var(--space-2);
  border: 0;
  border-radius: var(--radius);
  background: transparent;
  color: var(--text-1);
  font: inherit;
  font-size: var(--text-13);
  text-align: start;
  white-space: nowrap;
}
.lane-row__name {
  overflow: clip;
  text-overflow: ellipsis;
  font-weight: 500;
}
.lane-row__what {
  overflow: clip;
  text-overflow: ellipsis;
  color: var(--text-2);
  font-size: var(--text-12);
}
.lane-row__time {
  color: var(--text-1);
  font-size: var(--text-11);
  font-variant-numeric: tabular-nums;
}
.lane-row:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}
.lane-row:active {
  box-shadow: inset 0 0 0 100vmax var(--selected);
}
@media (hover: hover) and (pointer: fine) {
  .lane-row:hover {
    background: var(--hover);
  }
}
```

- [ ] **Step 7: Stories**

`src/features/lane/Lane.stories.tsx`:

```tsx
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { APPROVALS } from "../approval/fixtures";
import { Lane } from "./Lane";

const meta: Meta = { title: "Needs-you lane" };
export default meta;

const frame = (children: React.ReactNode) => (
  <div style={{ inlineSize: 288, background: "var(--bg-raised)", borderInlineEnd: "1px solid var(--hairline)" }}>{children}</div>
);

export const AllClear: StoryObj = { render: () => <ThemePair label="All clear">{frame(<Lane requests={[]} sidebarHeight={800} />)}</ThemePair> };
export const OneWaiting: StoryObj = { render: () => <ThemePair label="One waiting">{frame(<Lane requests={[APPROVALS.safeBash]} sidebarHeight={800} />)}</ThemePair> };
export const TwoWaiting: StoryObj = {
  render: () => <ThemePair label="Two waiting: the oldest is expanded">{frame(<Lane requests={[APPROVALS.safeBash, APPROVALS.question]} sidebarHeight={800} />)}</ThemePair>,
};
const many = Array.from({ length: 12 }, (_, i) => ({ ...APPROVALS.safeBash, id: `r${i}`, session: { ...APPROVALS.safeBash.session, name: `session-${i + 1}` }, waitingMs: 60_000 * (i + 1) }));
export const TwelveWaiting: StoryObj = { render: () => <ThemePair label="Twelve waiting in a 520 px sidebar: scrolls, +N more">{frame(<Lane requests={many} sidebarHeight={520} />)}</ThemePair> };
```

- [ ] **Step 8: Run and commit**

Run: `pnpm vitest run` (PASS) and `pnpm tsc --noEmit` (clean).

```bash
git add src/features/lane
git commit -m "feat(lane): needs-you lane with a permanent header, oldest-first accordion and a 40% cap

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Layout rules, monogram, Chip, session header, rail

**Files:**
- Create: `src/features/window/layout.ts`, `src/features/sidebar/monogram.ts`, `src/components/Chip/Chip.tsx`, `src/components/Chip/Chip.css`, `src/features/window/SessionHeader.tsx`, `src/features/window/SessionHeader.css`, `src/features/window/Rail.tsx`, `src/features/window/Rail.css`
- Test: `src/features/window/layout.test.ts`, `src/features/sidebar/monogram.test.ts`, `src/components/Chip/Chip.test.tsx`, `src/features/window/SessionHeader.test.tsx`, `src/features/window/Rail.test.tsx`
- Modify: `src/features/sidebar/fixtures.ts` (give the `working` fixture `mode: "plan"`)

**Interfaces:**
- Consumes: `RowModel`, `Mode`, `modeCaution`, `rowLabel`, `ContextRing`, `StatusGlyph`, `AuxGlyph`, `AgentMark`, `formatDuration`.
- Produces (exact):

```ts
// layout.ts
export const SIDEBAR = { default: 288, min: 200, max: 400, rail: 52 } as const;
export const AUTO_COLLAPSE_BELOW = 960;
export const MIN_WINDOW = { width: 720, height: 480 } as const;
export const DRAWER = { default: 360, min: 300, max: 560 } as const;
export type SidebarPref = "expanded" | "rail" | "focus";
export type SidebarMode = "expanded" | "rail" | "focus";
export function sidebarMode(windowWidth: number, pref: SidebarPref): SidebarMode;
export function sidebarWidth(mode: SidebarMode, requested?: number): number;
export function drawerPlacement(windowWidth: number, mode: SidebarMode, drawerWidth?: number): "push" | "overlay";
// monogram.ts
export function monogram(name: string): string;
// Chip.tsx
export function Chip(props: { tone?: "neutral" | "caution" | "accent"; children: ReactNode; title?: string }): JSX.Element;
// SessionHeader.tsx
export function headerOutlined(mode?: Mode): boolean;
export function modeLabel(mode?: Mode): string;
export function SessionHeader(props: { row: RowModel; ports?: string[] }): JSX.Element;
// Rail.tsx
export function Rail(props: { rows: RowModel[]; needsYou: number; selectedId?: string }): JSX.Element;
```

- [ ] **Step 1: Write the failing tests**

`src/features/window/layout.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { AUTO_COLLAPSE_BELOW, DRAWER, MIN_WINDOW, SIDEBAR, drawerPlacement, sidebarMode, sidebarWidth } from "./layout";

describe("constants from docs/PLAN.md", () => {
  it("match", () => {
    expect(SIDEBAR).toEqual({ default: 288, min: 200, max: 400, rail: 52 });
    expect(AUTO_COLLAPSE_BELOW).toBe(960);
    expect(MIN_WINDOW).toEqual({ width: 720, height: 480 });
    expect(DRAWER).toEqual({ default: 360, min: 300, max: 560 });
  });
});

describe("sidebarMode", () => {
  it("collapses to the rail below 960 px", () => {
    expect(sidebarMode(959, "expanded")).toBe("rail");
    expect(sidebarMode(960, "expanded")).toBe("expanded");
    expect(sidebarMode(720, "expanded")).toBe("rail");
  });
  it("keeps what the user chose otherwise", () => {
    expect(sidebarMode(1400, "rail")).toBe("rail");
    expect(sidebarMode(1400, "focus")).toBe("focus");
    expect(sidebarMode(800, "focus")).toBe("focus");
  });
});

describe("sidebarWidth", () => {
  it("is 288 by default, clamped between 200 and 400", () => {
    expect(sidebarWidth("expanded")).toBe(288);
    expect(sidebarWidth("expanded", 150)).toBe(200);
    expect(sidebarWidth("expanded", 999)).toBe(400);
    expect(sidebarWidth("expanded", Number.NaN)).toBe(288);
  });
  it("is 52 for the rail and 0 in focus mode", () => {
    expect(sidebarWidth("rail")).toBe(52);
    expect(sidebarWidth("focus")).toBe(0);
  });
});

describe("drawerPlacement", () => {
  it("pushes the terminal when it stays at 720 px or more", () => {
    expect(drawerPlacement(1600, "expanded", 360)).toBe("push");
  });
  it("overlays when the terminal would drop below 720 px", () => {
    expect(drawerPlacement(1280, "expanded", 360)).toBe("overlay");
    expect(drawerPlacement(720, "rail", 360)).toBe("overlay");
  });
});
```

`src/features/sidebar/monogram.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { monogram } from "./monogram";

describe("monogram", () => {
  it("takes the initials of the first two words", () => {
    expect(monogram("api-server")).toBe("AS");
    expect(monogram("my_cool project")).toBe("MC");
  });
  it("takes the first two letters of one word", () => {
    expect(monogram("billing")).toBe("BI");
    expect(monogram("x")).toBe("X");
  });
  it("counts whole characters, not code units (review focus 1)", () => {
    expect(monogram("\u2705\u2705\u2705\u2705")).toBe("\u2705\u2705");
    expect(monogram("\u8a8d\u8a3c\u30b5\u30fc\u30d0\u30fc")).toBe("\u8a8d\u8a3c");
    expect(monogram("\u{1F680}rocket")).toBe("\u{1F680}R");
  });
  it("handles right-to-left names", () => {
    expect(monogram("\u062e\u0627\u062f\u0645-\u0627\u0644\u062a\u062d\u0642\u0642").length).toBeGreaterThan(0);
  });
  it("returns nothing for an empty or blank name", () => {
    expect(monogram("")).toBe("");
    expect(monogram("   ")).toBe("");
  });
});
```

`src/components/Chip/Chip.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Chip } from "./Chip";

describe("Chip", () => {
  it("renders text with a tone", () => {
    const h = renderToStaticMarkup(<Chip tone="caution">Bypass</Chip>);
    expect(h).toContain("Bypass");
    expect(h).toContain('data-tone="caution"');
  });
  it("defaults to neutral", () => {
    expect(renderToStaticMarkup(<Chip>Plan</Chip>)).toContain('data-tone="neutral"');
  });
});
```

`src/features/window/SessionHeader.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { SessionHeader, headerOutlined, modeLabel } from "./SessionHeader";

const row = (id: string) => FIXTURES.find((f) => f.id === id)!;

describe("modeLabel / headerOutlined", () => {
  it("names each mode", () => {
    expect(modeLabel("manual")).toBe("Manual");
    expect(modeLabel("plan")).toBe("Plan");
    expect(modeLabel("auto-edit")).toBe("Auto-edit");
    expect(modeLabel("full-auto")).toBe("Full auto");
    expect(modeLabel("bypass")).toBe("Bypass");
    expect(modeLabel(undefined)).toBe("\u2013");
  });
  it("outlines the header in red only for bypass and auto", () => {
    expect(headerOutlined("bypass")).toBe(true);
    expect(headerOutlined("full-auto")).toBe(true);
    expect(headerOutlined("auto-edit")).toBe(false);
    expect(headerOutlined("plan")).toBe(false);
    expect(headerOutlined(undefined)).toBe(false);
  });
});

describe("SessionHeader", () => {
  const h = renderToStaticMarkup(<SessionHeader row={row("working")} ports={["localhost:3000"]} />);
  it("shows name, project and branch, mode, model and effort, subagents, context and ports in one bar", () => {
    for (const s of ["api-server", "my-app", "feat/auth-flow", "Plan", "Opus 5.5", "high", "2 subagents", "Context 42% used", "localhost:3000"]) expect(h, s).toContain(s);
  });
  it("shows unknown values as a dash, never a guess", () => {
    const u = renderToStaticMarkup(<SessionHeader row={row("unknown")} />);
    expect(u).toContain("\u2013");
    expect(u).not.toContain("subagents");
  });
  it("is outlined for a bypass session", () => {
    expect(renderToStaticMarkup(<SessionHeader row={row("bypass")} />)).toContain("data-outlined");
    expect(h).not.toContain("data-outlined");
  });
  it("labels an elevated session for the whole header", () => {
    expect(renderToStaticMarkup(<SessionHeader row={row("elevated")} />)).toContain("Administrator");
  });
  it("is a labelled banner-level region", () => {
    expect(h).toContain('role="group"');
    expect(h).toContain('aria-label="Session api-server"');
  });
});
```

`src/features/window/Rail.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { Rail } from "./Rail";

const rows = ["needs-permission", "working", "done-unseen"].map((id) => FIXTURES.find((f) => f.id === id)!);

describe("Rail", () => {
  it("has one button per session with the monogram and the status glyph", () => {
    const h = renderToStaticMarkup(<Rail rows={rows} needsYou={1} />);
    expect((h.match(/class="rail__item"/g) ?? []).length).toBe(3);
    expect(h).toContain("BI");
    expect(h).toContain("AS");
    expect(h).toContain("status-glyph");
  });
  it("labels each button with the same text the row has", () => {
    const h = renderToStaticMarkup(<Rail rows={rows} needsYou={1} />);
    expect(h).toContain("billing, Claude Sonnet 5.5, Needs you: permission");
  });
  it("stacks a needs-you badge with a count at the top, only when something waits", () => {
    expect(renderToStaticMarkup(<Rail rows={rows} needsYou={2} />)).toContain("rail__badge");
    expect(renderToStaticMarkup(<Rail rows={rows} needsYou={0} />)).not.toContain("rail__badge");
  });
  it("marks the selected session", () => {
    expect(renderToStaticMarkup(<Rail rows={rows} needsYou={0} selectedId="working" />)).toContain('aria-current="true"');
  });
  it("carries the agent so the brand stripe colours", () => {
    expect(renderToStaticMarkup(<Rail rows={rows} needsYou={0} />)).toContain('data-agent="claude"');
  });
});
```

- [ ] **Step 2: Run to see them fail**

Run: `pnpm vitest run src/features/window src/features/sidebar/monogram src/components/Chip`
Expected: FAIL (modules not found).

- [ ] **Step 3: Implement layout and monogram**

`src/features/window/layout.ts`:

```ts
/** Sizes and rules from docs/PLAN.md "Layout". */
export const SIDEBAR = { default: 288, min: 200, max: 400, rail: 52 } as const;
export const AUTO_COLLAPSE_BELOW = 960;
export const MIN_WINDOW = { width: 720, height: 480 } as const;
export const DRAWER = { default: 360, min: 300, max: 560 } as const;

export type SidebarPref = "expanded" | "rail" | "focus";
export type SidebarMode = "expanded" | "rail" | "focus";

/** Below 960 px the expanded sidebar collapses to the rail by itself; the user's rail and focus choices always stand. */
export function sidebarMode(windowWidth: number, pref: SidebarPref): SidebarMode {
  if (pref !== "expanded") return pref;
  return windowWidth < AUTO_COLLAPSE_BELOW ? "rail" : "expanded";
}

export function sidebarWidth(mode: SidebarMode, requested: number = SIDEBAR.default): number {
  if (mode === "focus") return 0;
  if (mode === "rail") return SIDEBAR.rail;
  const r = Number.isFinite(requested) ? requested : SIDEBAR.default;
  return Math.min(SIDEBAR.max, Math.max(SIDEBAR.min, r));
}

/** The drawer pushes the terminal, and overlays it instead when the terminal would drop below 720 px. */
export function drawerPlacement(windowWidth: number, mode: SidebarMode, drawerWidth: number = DRAWER.default): "push" | "overlay" {
  return windowWidth - sidebarWidth(mode) - drawerWidth >= MIN_WINDOW.width ? "push" : "overlay";
}
```

`src/features/sidebar/monogram.ts`:

```ts
const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });
const first = (s: string) => [...graphemes.segment(s)][0]?.segment ?? "";

/** Two characters that stand for a session in the 52 px rail: word initials, else the first two characters. */
export function monogram(name: string): string {
  const words = name.split(/[\s\-_./\\]+/).filter(Boolean);
  if (words.length >= 2) return (first(words[0]!) + first(words[1]!)).toLocaleUpperCase();
  const chars = [...graphemes.segment(words[0] ?? "")].map((g) => g.segment);
  return chars.slice(0, 2).join("").toLocaleUpperCase();
}
```

Run: `pnpm vitest run src/features/window/layout src/features/sidebar/monogram` (PASS).

- [ ] **Step 4: Implement Chip**

`src/components/Chip/Chip.tsx`:

```tsx
import type { ReactNode } from "react";
import "./Chip.css";

/** A small label pill: mode, port, state. */
export function Chip({ tone = "neutral", children, title }: { tone?: "neutral" | "caution" | "accent"; children: ReactNode; title?: string }) {
  return (
    <span className="chip" data-tone={tone} title={title}>
      {children}
    </span>
  );
}
```

`src/components/Chip/Chip.css`:

```css
.chip {
  display: inline-flex;
  align-items: center;
  flex: none;
  padding-inline: 6px;
  border: 1px solid var(--hairline);
  border-radius: 999px;
  color: var(--text-1);
  font-size: var(--text-11);
  line-height: 18px;
  white-space: nowrap;
}
.chip[data-tone="caution"] {
  border-color: var(--caution);
}
.chip[data-tone="accent"] {
  border-color: var(--accent);
}
```

- [ ] **Step 5: Implement the header**

First add `mode: "plan",` to the `working` fixture in `src/features/sidebar/fixtures.ts` (after `subagents: 2,`).

`src/features/window/SessionHeader.tsx`:

```tsx
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { Chip } from "../../components/Chip/Chip";
import { ContextRing } from "../../components/ContextRing/ContextRing";
import { AuxGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { modeCaution } from "../sidebar/format";
import type { Mode, RowModel } from "../sidebar/types";
import "./SessionHeader.css";

const DASH = "\u2013";
const MODE_LABELS: Record<Mode, string> = { manual: "Manual", plan: "Plan", "auto-edit": "Auto-edit", "full-auto": "Full auto", bypass: "Bypass" };

export const modeLabel = (mode?: Mode) => (mode ? MODE_LABELS[mode] : DASH);
/** docs/PLAN.md "Sidebar rows": the header is outlined red for bypass and auto. */
export const headerOutlined = (mode?: Mode) => mode === "bypass" || mode === "full-auto";

/** The session header in the unified 40 px top bar: name, place, mode, model and effort, subagents, context, ports. */
export function SessionHeader({ row, ports = [] }: { row: RowModel; ports?: string[] }) {
  const caution = modeCaution(row.mode);
  return (
    <div className="session-header" role="group" aria-label={`Session ${row.name}`} data-outlined={headerOutlined(row.mode) || undefined} data-agent={row.agent}>
      <AgentMark agent={row.agent} />
      {row.elevated && <AuxGlyph kind="elevated" label="Administrator" />}
      <span className="session-header__name" dir="auto">
        {row.name}
      </span>
      {row.elevated && <span className="session-header__label">Administrator</span>}
      <span className="session-header__place" dir="auto">
        {row.branch ? `${row.project} \u00b7 ${row.branch}` : row.project}
      </span>
      <Chip tone={caution ? "caution" : "neutral"}>{modeLabel(row.mode)}</Chip>
      <span className="session-header__meta">
        {row.model ?? DASH} {"\u00b7"} {row.effort ?? DASH}
      </span>
      {row.subagents !== undefined && <span className="session-header__meta">{row.subagents} subagents</span>}
      <span className="session-header__spacer" />
      <ContextRing pct={row.contextPct} />
      {ports.map((p) => (
        <Chip key={p}>{p}</Chip>
      ))}
    </div>
  );
}
```

`src/features/window/SessionHeader.css`:

```css
.session-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-inline-size: 0;
  block-size: 40px;
  padding-inline: var(--space-3);
  overflow: clip;
  white-space: nowrap;
  font-size: var(--text-13);
}
.session-header[data-outlined] {
  box-shadow: inset 0 0 0 2px var(--error);
}
.session-header__name {
  flex: 0 1 auto;
  min-inline-size: 0;
  overflow: clip;
  text-overflow: ellipsis;
  font-weight: 600;
}
.session-header__label {
  color: var(--text-2);
  font-size: var(--text-12);
}
.session-header__place,
.session-header__meta {
  min-inline-size: 0;
  overflow: clip;
  text-overflow: ellipsis;
  color: var(--text-2);
  font-size: var(--text-12);
}
.session-header__spacer {
  flex: 1 1 0;
}
@media (forced-colors: active) {
  .session-header[data-outlined] {
    outline: 2px solid CanvasText;
  }
}
```

- [ ] **Step 6: Implement the rail**

`src/features/window/Rail.tsx`:

```tsx
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { rowLabel } from "../sidebar/labels";
import { monogram } from "../sidebar/monogram";
import type { RowModel } from "../sidebar/types";
import "./Rail.css";

/** The 52 px rail: the brand stripe, status glyph and a monogram per session, with a needs-you badge stack on top. */
export function Rail({ rows, needsYou, selectedId }: { rows: RowModel[]; needsYou: number; selectedId?: string }) {
  return (
    <nav className="rail" aria-label="Sessions">
      {needsYou > 0 && (
        <div className="rail__badge" role="status" aria-label={`${needsYou} need you`}>
          <StatusGlyph kind="needs-permission" label="Needs you" />
          <span className="rail__count">{needsYou}</span>
        </div>
      )}
      {rows.map((r) => (
        <button key={r.id} type="button" className="rail__item" data-agent={r.agent} aria-label={rowLabel(r)} aria-current={r.id === selectedId ? "true" : undefined}>
          <span className="rail__stripe" aria-hidden="true" />
          <StatusGlyph kind={r.status} size={16} />
          <span className="rail__mono" aria-hidden="true">
            {monogram(r.name)}
          </span>
        </button>
      ))}
    </nav>
  );
}
```

`src/features/window/Rail.css`:

```css
.rail {
  display: grid;
  align-content: start;
  justify-items: center;
  gap: var(--space-1);
  inline-size: 52px;
  block-size: 100%;
  padding-block: var(--space-2);
  overflow-y: auto;
  background: var(--bg-raised);
  border-inline-end: 1px solid var(--hairline);
}
.rail__badge {
  display: grid;
  justify-items: center;
  gap: 2px;
  padding-block-end: var(--space-2);
}
.rail__count {
  color: var(--text-1);
  font-size: var(--text-11);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.rail__item {
  position: relative;
  display: grid;
  justify-items: center;
  gap: 2px;
  inline-size: 100%;
  padding-block: 6px;
  border: 0;
  background: transparent;
  color: var(--text-1);
  font: inherit;
}
.rail__stripe {
  position: absolute;
  inset-block: 0;
  inset-inline-start: 0;
  inline-size: var(--stripe);
  background: var(--agent, var(--brand-generic));
}
.rail__mono {
  font-size: var(--text-11);
  font-weight: 600;
  letter-spacing: 0.04em;
}
.rail__item[aria-current="true"] {
  box-shadow: inset 0 0 0 100vmax var(--selected);
}
.rail__item:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}
.rail__item:active {
  box-shadow: inset 0 0 0 100vmax var(--selected);
}
@media (hover: hover) and (pointer: fine) {
  .rail__item:hover {
    background: var(--hover);
  }
}
```

- [ ] **Step 7: Run and commit**

Run: `pnpm vitest run` (PASS; the batch 1 row tests still pass with `mode: "plan"` on `working`) and `pnpm tsc --noEmit` (clean). If `AgentMark` is unused in `Rail`, nothing to do; if the header test cannot find "2 subagents" because JSX splits the number and the word into two text nodes, the static markup still concatenates them (no change needed).

```bash
git add -A src
git commit -m "feat(window): layout rules, monogram, chip, session header and rail

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Sidebar, 5-agent scenario and the main window composite

**Files:**
- Create: `src/features/window/scenario.ts`, `src/features/window/Sidebar.tsx`, `src/features/window/Sidebar.css`, `src/features/window/MainWindow.tsx`, `src/features/window/MainWindow.css`, `src/features/window/MainWindow.stories.tsx`
- Test: `src/features/window/scenario.test.ts`, `src/features/window/Sidebar.test.tsx`, `src/features/window/MainWindow.test.tsx`, `src/features/window/MainWindow.css.test.ts`

**Interfaces:**
- Consumes: everything from Tasks 1–9: `SessionRow`, `Lane`, `Rail`, `SessionHeader`, `ScriptedTerminal`, `ApproveCard`, `sidebarMode`, `sidebarWidth`, `drawerPlacement`, `SIDEBAR`, `MIN_WINDOW`, `DRAWER`, `shortcutLabel`, `commandById`, `FIXTURES`, `APPROVALS`, `ContextRing`, `AuxGlyph`, `StatusGlyph`, `AgentMark`.
- Produces (exact):

```ts
// scenario.ts
export const SCENARIO_IDS: readonly ["needs-permission", "needs-question", "working", "done-unseen", "error"];
export const SCENARIO_ROWS: RowModel[];
export const SCENARIO_REQUESTS: ApprovalRequest[];     // question (4m, infra) and safe bash (65s, billing)
export const SCENARIO_FOOTER: SidebarFooter;
export const SCENARIO_PORTS: string[];
// Sidebar.tsx
export type SidebarFooter = { planPct?: number; fiveHourPct?: number; doctorIssues?: number };
export function Sidebar(props: { rows: RowModel[]; requests: ApprovalRequest[]; height: number; width: number; selectedId?: string; footer: SidebarFooter }): JSX.Element;
// MainWindow.tsx
export type WindowLayout = "default" | "split" | "focus" | "rail";
export type MainWindowProps = {
  width: number;
  height: number;
  layout?: WindowLayout;
  sidebarPref?: SidebarPref;          // "expanded" by default; "focus" for focus mode, "rail" to pin the rail
  rows?: RowModel[];
  requests?: ApprovalRequest[];
  selectedId?: string;
  os?: Os;
  terminalSetting?: TerminalThemeSetting;
  headerPeek?: boolean;               // focus mode: show the header as if the top edge were hovered
  drawer?: boolean;                   // show the right drawer with the first request's full payload
};
export function MainWindow(props: MainWindowProps): JSX.Element;
```

- [ ] **Step 1: Write the failing tests**

`src/features/window/scenario.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { orderLane } from "../lane/lane";
import { SCENARIO_FOOTER, SCENARIO_IDS, SCENARIO_PORTS, SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";

describe("the 5-agent scenario (docs/PLAN.md deliverable 5)", () => {
  it("has two needing you, one working, one done-unseen and one error", () => {
    expect(SCENARIO_ROWS.length).toBe(5);
    const by = (s: string) => SCENARIO_ROWS.filter((r) => r.status === s).length;
    expect(by("needs-permission") + by("needs-question")).toBe(2);
    expect(by("working")).toBe(1);
    expect(by("done-unseen")).toBe(1);
    expect(by("error")).toBe(1);
  });
  it("keeps the ids in the plan's order", () => {
    expect(SCENARIO_ROWS.map((r) => r.id)).toEqual([...SCENARIO_IDS]);
  });
  it("has a lane request for each needs-you row, matched by session name", () => {
    const needs = SCENARIO_ROWS.filter((r) => r.status.startsWith("needs"));
    expect(SCENARIO_REQUESTS.map((q) => q.session.name).sort()).toEqual(needs.map((r) => r.name).sort());
  });
  it("targets the question first because it has waited longest", () => {
    expect(orderLane(SCENARIO_REQUESTS)[0]!.detail.kind).toBe("question");
  });
  it("has a footer and ports to show", () => {
    expect(SCENARIO_FOOTER.doctorIssues).toBeGreaterThan(0);
    expect(SCENARIO_PORTS.length).toBeGreaterThan(0);
  });
});
```

`src/features/window/Sidebar.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SCENARIO_FOOTER, SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import { Sidebar } from "./Sidebar";

const html = (p: Partial<Parameters<typeof Sidebar>[0]> = {}) =>
  renderToStaticMarkup(<Sidebar rows={SCENARIO_ROWS} requests={SCENARIO_REQUESTS} height={760} width={288} footer={SCENARIO_FOOTER} {...p} />);

describe("Sidebar", () => {
  it("stacks the lane above the session list above a 40 px footer", () => {
    const h = html();
    expect(h.indexOf("class=\"lane\"")).toBeLessThan(h.indexOf('role="listbox"'));
    expect(h.indexOf('role="listbox"')).toBeLessThan(h.indexOf("sidebar__footer"));
  });
  it("lists every session as an option with one tab stop", () => {
    const h = html({ selectedId: "working" });
    expect((h.match(/role="option"/g) ?? []).length).toBe(5);
    expect((h.match(/tabindex="0"/g) ?? []).length).toBeGreaterThanOrEqual(1);
    expect(h).toContain('aria-selected="true"');
  });
  it("shows the plan ring, the 5h percentage and the doctor count in the footer", () => {
    const h = html();
    expect(h).toContain("5h 38%");
    expect(h).toContain("Doctor: 2 issues");
  });
  it("shows unknown plan usage as a dash and a clean doctor as such", () => {
    const h = html({ footer: {} });
    expect(h).toContain("5h \u2013");
    expect(h).toContain("Doctor: no issues");
  });
  it("keeps the lane header when nothing waits", () => {
    expect(html({ requests: [] })).toContain("All clear");
  });
  it("sets its own width", () => {
    expect(html({ width: 300 })).toContain("inline-size:300px");
  });
});
```

`src/features/window/MainWindow.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MainWindow } from "./MainWindow";

const html = (p: Partial<Parameters<typeof MainWindow>[0]> = {}) => renderToStaticMarkup(<MainWindow width={1180} height={800} selectedId="working" {...p} />);

describe("MainWindow: default", () => {
  const h = html();
  it("has one unified 40 px top bar, a sidebar and one terminal", () => {
    expect(h).toContain("window__bar");
    expect(h).toContain("session-header");
    expect(h).toContain('class="sidebar"');
    expect((h.match(/class="scripted-terminal"/g) ?? []).length).toBe(1);
  });
  it("shows the lane, with the oldest request expanded", () => {
    expect(h).toContain("Needs you \u00b7 2");
    expect(h).toContain("Which AWS region");
  });
  it("has Windows window controls and a palette button in the bar", () => {
    for (const s of ["Minimize", "Maximize", "Close", "Command palette"]) expect(h, s).toContain(`aria-label="${s}"`);
  });
  it("shows the session the user selected in the header", () => {
    expect(h).toContain('aria-label="Session api-server"');
  });
});

describe("MainWindow: layouts", () => {
  it("collapses to the rail below 960 px", () => {
    const h = html({ width: 940 });
    expect(h).toContain('class="rail"');
    expect(h).not.toContain('class="sidebar"');
  });
  it("pins the rail when asked, at any width", () => {
    expect(html({ sidebarPref: "rail" })).toContain('class="rail"');
  });
  it("is 720 by 480 at the minimum, still showing a terminal and the header", () => {
    const h = html({ width: 720, height: 480 });
    expect(h).toContain("inline-size:720px");
    expect(h).toContain("scripted-terminal");
    expect(h).toContain("session-header");
  });
  it("focus mode hides the sidebar and header, and floats a pill naming the shortcut", () => {
    const h = html({ layout: "focus", sidebarPref: "focus" });
    expect(h).not.toContain('class="sidebar"');
    expect(h).not.toContain("session-header");
    expect(h).toContain("focus-pill");
    expect(h).toContain("2 need you");
    expect(h).toContain("Ctrl+Shift+N");
  });
  it("focus mode can show the header as the top edge is hovered", () => {
    expect(html({ layout: "focus", sidebarPref: "focus", headerPeek: true })).toContain("session-header");
  });
  it("uses the Mac shortcut label on macOS", () => {
    expect(html({ layout: "focus", sidebarPref: "focus", os: "mac" })).toContain("\u2318N");
  });
  it("split view has two panes, a separator and exactly one active pane", () => {
    const h = html({ layout: "split" });
    expect((h.match(/class="scripted-terminal"/g) ?? []).length).toBe(2);
    expect(h).toContain('role="separator"');
    expect((h.match(/data-active="true"/g) ?? []).length).toBe(1);
    expect(h).toContain('data-active="false"');
  });
  it("the drawer shows the first request in full and pushes or overlays by width", () => {
    expect(html({ drawer: true, width: 1700 })).toContain('data-placement="push"');
    expect(html({ drawer: true, width: 1180 })).toContain('data-placement="overlay"');
  });
  it("passes the terminal setting through", () => {
    expect(html({ terminalSetting: "light" })).toContain('data-setting="light"');
  });
});

describe("MainWindow: hygiene", () => {
  it("never puts a literal colour in markup", () => {
    for (const layout of ["default", "split", "focus", "rail"] as const)
      expect(html({ layout, sidebarPref: layout === "focus" ? "focus" : layout === "rail" ? "rail" : "expanded" })).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});
```

`src/features/window/MainWindow.css.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./MainWindow.css", import.meta.url), "utf8");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("MainWindow.css", () => {
  it("makes the top bar one 40 px row", () => {
    expect(rule(".window__bar")).toContain("block-size: 40px");
  });
  it("sizes the window controls 46 by 40", () => {
    const r = rule(".window__control");
    expect(r).toContain("inline-size: 46px");
    expect(r).toContain("block-size: 40px");
  });
  it("dims an inactive pane header to 60% but never the terminal text", () => {
    expect(rule('.pane[data-active="false"] .pane__header')).toContain("opacity: 0.6");
    expect(css).not.toMatch(/\.pane[^{]*\.scripted-terminal[^{]*\{[^}]*opacity/);
  });
  it("marks the active pane with a 2 px accent bar under its header", () => {
    expect(rule('.pane[data-active="true"] .pane__header')).toContain("2px");
    expect(rule('.pane[data-active="true"] .pane__header')).toContain("var(--accent)");
  });
  it("gives the split separator an 8 px hit area around a 1 px line", () => {
    expect(rule(".split__handle")).toContain("inline-size: 8px");
    expect(css).toContain("1px");
  });
  it("styles focus and press on the bar buttons", () => {
    expect(css).toContain(".window__control:focus-visible");
    expect(css).toContain(".window__control:active");
    expect(css).not.toMatch(/outline:\s*none/);
  });
  it("uses tokens and logical properties only", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).not.toMatch(/\b(left|right|top|bottom):/);
  });
});
```

- [ ] **Step 2: Run to see them fail**

Run: `pnpm vitest run src/features/window`
Expected: FAIL for the new test files (modules not found); Task 9's tests still PASS.

- [ ] **Step 3: Scenario**

`src/features/window/scenario.ts`:

```ts
import { APPROVALS } from "../approval/fixtures";
import type { ApprovalRequest } from "../approval/types";
import { FIXTURES } from "../sidebar/fixtures";
import type { RowModel } from "../sidebar/types";
import type { SidebarFooter } from "./Sidebar";

/** The 5-agent scenario: 2 cards in the lane, 1 working, 1 done-unseen, 1 error. It drives every window story and the 5-second test. */
export const SCENARIO_IDS = ["needs-permission", "needs-question", "working", "done-unseen", "error"] as const;

export const SCENARIO_ROWS: RowModel[] = SCENARIO_IDS.map((id) => FIXTURES.find((f) => f.id === id)!);

/** Session names match the rows: billing (65 s, a command) and infra (4 min, a question). */
export const SCENARIO_REQUESTS: ApprovalRequest[] = [APPROVALS.safeBash, APPROVALS.question];

export const SCENARIO_FOOTER: SidebarFooter = { planPct: 38, fiveHourPct: 38, doctorIssues: 2 };
export const SCENARIO_PORTS = ["localhost:3000", "localhost:5173"];
```

- [ ] **Step 4: Sidebar**

`src/features/window/Sidebar.tsx`:

```tsx
import { ContextRing } from "../../components/ContextRing/ContextRing";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import type { ApprovalRequest } from "../approval/types";
import { Lane } from "../lane/Lane";
import { SessionRow } from "../sidebar/SessionRow";
import type { RowModel } from "../sidebar/types";
import "./Sidebar.css";

export type SidebarFooter = { planPct?: number; fiveHourPct?: number; doctorIssues?: number };

const DASH = "\u2013";

/** The expanded sidebar: the needs-you lane, the session list, and a 40 px footer with plan use and the doctor. */
export function Sidebar({ rows, requests, height, width, selectedId, footer }: { rows: RowModel[]; requests: ApprovalRequest[]; height: number; width: number; selectedId?: string; footer: SidebarFooter }) {
  const issues = footer.doctorIssues ?? 0;
  const tabStop = rows.some((r) => r.id === selectedId) ? selectedId : rows[0]?.id;
  return (
    <aside className="sidebar" style={{ inlineSize: width }} aria-label="Sessions and approvals">
      <Lane requests={requests} sidebarHeight={height} />
      <div className="sidebar__list" role="listbox" aria-label="Sessions">
        {rows.map((r) => (
          <SessionRow key={r.id} row={r} density="comfortable" selected={r.id === selectedId} tabStop={r.id === tabStop} />
        ))}
      </div>
      <footer className="sidebar__footer">
        <ContextRing pct={footer.planPct} />
        <span className="sidebar__plan">5h {footer.fiveHourPct === undefined ? DASH : `${Math.round(footer.fiveHourPct)}%`}</span>
        <span className="sidebar__spacer" />
        <span className="sidebar__doctor" role="img" aria-label={issues > 0 ? `Doctor: ${issues} issues` : "Doctor: no issues"}>
          <span aria-hidden="true">{issues > 0 ? <AuxGlyph kind="caution" /> : <StatusGlyph kind="done-seen" size={12} />}</span>
          <span aria-hidden="true">{issues}</span>
        </span>
      </footer>
    </aside>
  );
}
```

`src/features/window/Sidebar.css`:

```css
.sidebar {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) 40px;
  min-block-size: 0;
  block-size: 100%;
  background: var(--bg-raised);
  border-inline-end: 1px solid var(--hairline);
}
.sidebar__list {
  min-block-size: 0;
  overflow-y: auto;
}
.sidebar__footer {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  block-size: 40px;
  padding-inline: 14px 12px;
  border-block-start: 1px solid var(--hairline);
  color: var(--text-2);
  font-size: var(--text-12);
  font-variant-numeric: tabular-nums;
}
.sidebar__spacer {
  flex: 1 1 0;
}
.sidebar__doctor {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  color: var(--text-1);
}
```

- [ ] **Step 5: Main window**

`src/features/window/MainWindow.tsx`:

```tsx
import type { CSSProperties } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { ApproveCard } from "../approval/ApproveCard";
import { APPROVALS } from "../approval/fixtures";
import type { ApprovalRequest } from "../approval/types";
import { pending } from "../lane/lane";
import type { RowModel } from "../sidebar/types";
import { ScriptedTerminal } from "../terminal/ScriptedTerminal";
import type { TerminalThemeSetting } from "../terminal/terminalTheme";
import { DRAWER, drawerPlacement, sidebarMode, sidebarWidth, type SidebarPref } from "./layout";
import { Rail } from "./Rail";
import { SCENARIO_FOOTER, SCENARIO_PORTS, SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import { SessionHeader } from "./SessionHeader";
import { Sidebar } from "./Sidebar";
import "./MainWindow.css";

/** The drawer shows the request's full payload; the 40-line command is the long one. */
const DRAWER_DETAIL = APPROVALS.longCommand.detail;

export type WindowLayout = "default" | "split" | "focus" | "rail";
export type MainWindowProps = {
  width: number;
  height: number;
  layout?: WindowLayout;
  sidebarPref?: SidebarPref;
  rows?: RowModel[];
  requests?: ApprovalRequest[];
  selectedId?: string;
  os?: Os;
  terminalSetting?: TerminalThemeSetting;
  headerPeek?: boolean;
  drawer?: boolean;
};

const icon = (d: string) => (
  <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1">
    <path d={d} />
  </svg>
);

function Pane({ row, active, setting }: { row: RowModel; active: boolean; setting: TerminalThemeSetting }) {
  return (
    <section className="pane" data-active={active} aria-label={`Terminal: ${row.name}`}>
      <header className="pane__header">
        <AgentMark agent={row.agent} size={12} />
        <span className="pane__name" dir="auto">
          {row.name}
        </span>
        <span className="pane__place" dir="auto">
          {row.project}
        </span>
      </header>
      <ScriptedTerminal setting={setting} label={`Terminal for ${row.name}`} />
    </section>
  );
}

/** The main window composite (docs/PLAN.md deliverable 5): a Windows-style frame around the sidebar or rail, the session header and the terminal. */
export function MainWindow({
  width,
  height,
  layout = "default",
  sidebarPref,
  rows = SCENARIO_ROWS,
  requests = SCENARIO_REQUESTS,
  selectedId = "working",
  os = "windows",
  terminalSetting = "follow-app",
  headerPeek = false,
  drawer = false,
}: MainWindowProps) {
  const pref: SidebarPref = sidebarPref ?? (layout === "focus" ? "focus" : layout === "rail" ? "rail" : "expanded");
  const mode = sidebarMode(width, pref);
  const sideW = sidebarWidth(mode);
  const selected = rows.find((r) => r.id === selectedId) ?? rows[0]!;
  const second = rows.find((r) => r.id !== selected.id) ?? selected;
  const waiting = pending(requests).length;
  const showHeader = mode !== "focus" || headerPeek;
  const placement = drawerPlacement(width, mode, DRAWER.default);
  const pillKeys = shortcutLabel(commandById("next-waiting").binding!, os);

  return (
    <div className="window" style={{ inlineSize: width, blockSize: height, "--side-w": `${sideW}px` } as CSSProperties} data-mode={mode} data-layout={layout}>
      <header className="window__bar">
        <div className="window__bar-side">
          <button type="button" className="window__control window__control--menu" aria-label="App menu">
            {icon("M1 2.5h8M1 5h8M1 7.5h8")}
          </button>
          <button type="button" className="window__control window__control--menu" aria-label="Command palette">
            {icon("M1 5h8M5 1v8")}
          </button>
        </div>
        <div className="window__bar-main">{showHeader && <SessionHeader row={selected} ports={SCENARIO_PORTS} />}</div>
        <div className="window__controls">
          <button type="button" className="window__control" aria-label="Minimize">
            {icon("M0 5h10")}
          </button>
          <button type="button" className="window__control" aria-label="Maximize">
            {icon("M0.5 0.5h9v9h-9z")}
          </button>
          <button type="button" className="window__control window__control--close" aria-label="Close">
            {icon("M0 0l10 10M10 0L0 10")}
          </button>
        </div>
      </header>

      <div className="window__body">
        {mode === "expanded" && <Sidebar rows={rows} requests={requests} height={height - 40} width={sideW} selectedId={selected.id} footer={SCENARIO_FOOTER} />}
        {mode === "rail" && <Rail rows={rows} needsYou={waiting} selectedId={selected.id} />}

        <main className="window__main" data-placement={drawer ? placement : undefined}>
          {layout === "split" ? (
            <div className="split">
              <Pane row={selected} active setting={terminalSetting} />
              <div className="split__handle" role="separator" aria-orientation="vertical" aria-label="Resize panes" tabIndex={0} />
              <Pane row={second} active={false} setting={terminalSetting} />
            </div>
          ) : (
            <ScriptedTerminal setting={terminalSetting} label={`Terminal for ${selected.name}`} />
          )}

          {mode === "focus" && waiting > 0 && (
            <button type="button" className="focus-pill" aria-label={`${waiting} need you, ${pillKeys}`}>
              <span aria-hidden="true">
                <StatusGlyph kind="needs-permission" />
              </span>
              <span>{waiting} need you</span>
              <kbd className="focus-pill__kbd">{pillKeys}</kbd>
            </button>
          )}

          {drawer && requests[0] && (
            <aside className="drawer" data-placement={placement} style={{ inlineSize: DRAWER.default }} aria-label="Request details">
              <ApproveCard request={{ ...requests[0], detail: DRAWER_DETAIL }} expanded />
            </aside>
          )}
        </main>
      </div>
    </div>
  );
}

```

The test expects `data-placement` on `main` and on the drawer; `push` or `overlay` appears on both.

`src/features/window/MainWindow.css`:

```css
.window {
  display: grid;
  grid-template-rows: 40px minmax(0, 1fr);
  min-inline-size: 0;
  border: 1px solid var(--hairline);
  border-radius: 8px;
  background: var(--bg-base);
  color: var(--text-1);
  overflow: clip;
}

/* One unified 40 px bar: sidebar column | session header | window controls. */
.window__bar {
  display: grid;
  grid-template-columns: var(--side-w) minmax(0, 1fr) auto;
  align-items: center;
  block-size: 40px;
  border-block-end: 1px solid var(--hairline);
  background: var(--bg-raised);
}
.window__bar-side {
  display: flex;
  align-items: center;
  min-inline-size: 0;
  padding-inline: var(--space-1);
}
.window[data-mode="focus"] .window__bar-side {
  inline-size: 0;
  padding-inline: 0;
  overflow: clip;
}
.window__bar-main {
  min-inline-size: 0;
}
.window__controls {
  display: flex;
}
.window__control {
  display: grid;
  place-items: center;
  inline-size: 46px;
  block-size: 40px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-2);
}
.window__control--menu {
  inline-size: 32px;
  block-size: 32px;
  border-radius: var(--radius);
}
.window__control:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}
.window__control:active {
  background: var(--selected);
}
@media (hover: hover) and (pointer: fine) {
  .window__control:hover {
    background: var(--hover);
    color: var(--text-1);
  }
  .window__control--close:hover {
    background: var(--error);
    color: var(--bg-base);
  }
}

.window__body {
  display: flex;
  min-block-size: 0;
}
.window__main {
  position: relative;
  display: grid;
  flex: 1 1 0;
  min-inline-size: 0;
  min-block-size: 0;
}

.split {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 8px minmax(0, 1fr);
  min-block-size: 0;
}
/* An 8 px hit area around a 1 px hairline. */
.split__handle {
  inline-size: 8px;
  background: linear-gradient(to right, transparent 3.5px, var(--hairline) 3.5px, var(--hairline) 4.5px, transparent 4.5px);
  cursor: col-resize;
}
.split__handle:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}
.pane {
  display: grid;
  grid-template-rows: 28px minmax(0, 1fr);
  min-inline-size: 0;
  min-block-size: 0;
}
.pane__header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding-inline: var(--space-3);
  border-block-end: 2px solid transparent;
  font-size: var(--text-12);
}
.pane[data-active="true"] .pane__header {
  border-block-end: 2px solid var(--accent);
}
/* The inactive header recedes; terminal text is never dimmed. */
.pane[data-active="false"] .pane__header {
  opacity: 0.6;
}
.pane__name {
  font-weight: 600;
}
.pane__place {
  color: var(--text-2);
}

.focus-pill {
  position: absolute;
  inset-block-start: var(--space-3);
  inset-inline-end: var(--space-3);
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding-block: 4px;
  padding-inline: var(--space-3);
  border: 1px solid var(--hairline);
  border-radius: 999px;
  background: var(--bg-overlay);
  color: var(--text-1);
  font: inherit;
  font-size: var(--text-12);
}
.focus-pill__kbd {
  color: var(--text-2);
  font-family: var(--font-mono);
  font-size: var(--text-11);
}
.focus-pill:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
.focus-pill:active {
  box-shadow: inset 0 0 0 100vmax var(--selected);
}

/* The drawer pushes the terminal, or overlays it when the terminal would drop below 720 px. */
.drawer {
  display: grid;
  align-content: start;
  padding: var(--space-3);
  overflow-y: auto;
  background: var(--bg-raised);
  border-inline-start: 1px solid var(--hairline);
}
.drawer[data-placement="overlay"] {
  position: absolute;
  inset-block: 0;
  inset-inline-end: 0;
  box-shadow: 0 0 24px color-mix(in oklch, var(--text-1) 25%, transparent);
}
.window__main[data-placement="push"] {
  grid-auto-flow: column;
  grid-template-columns: minmax(0, 1fr) auto;
}

@media (forced-colors: active) {
  .window,
  .window__bar,
  .drawer {
    border-color: CanvasText;
  }
}
```

(The `box-shadow` on the overlay drawer and `left`-style gradient stops are the only non-logical spots: gradient direction `to right` is a paint direction, not a layout property, and RTL layouts mirror the whole window through `dir`, so the handle gradient is centred and symmetric; the CSS test only forbids physical *properties*.)

- [ ] **Step 6: Stories**

`src/features/window/MainWindow.stories.tsx`:

```tsx
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { MainWindow, type MainWindowProps } from "./MainWindow";

const meta: Meta = { title: "Main window", parameters: { layout: "fullscreen" } };
export default meta;

const stacked = (label: string, props: MainWindowProps): StoryObj => ({
  render: () => (
    <ThemePair label={label} stack>
      <div style={{ overflowX: "auto" }}>
        <MainWindow {...props} />
      </div>
    </ThemePair>
  ),
});

export const FiveAgents: StoryObj = stacked("The 5-agent scenario: two need you, one working, one done, one error", { width: 1180, height: 800 });
export const SplitView: StoryObj = stacked("Split view: the active pane has the accent bar, the other header recedes, terminal text is never dimmed", { width: 1180, height: 800, layout: "split" });
export const FocusMode: StoryObj = stacked("Focus mode: sidebar and header gone, a quiet pill top right", { width: 1180, height: 800, layout: "focus" });
export const FocusModeHeaderRevealed: StoryObj = stacked("Focus mode with the pointer on the top edge: the header slides in", { width: 1180, height: 800, layout: "focus", headerPeek: true });
export const CollapsedRail: StoryObj = stacked("Collapsed rail (52 px): stripe, glyph and monogram, needs-you badge stack on top", { width: 1180, height: 800, layout: "rail" });
export const AutoCollapseAt940: StoryObj = stacked("Below 960 px the sidebar collapses to the rail by itself", { width: 940, height: 620 });
export const MinimumWindow720: StoryObj = stacked("720 by 480, the minimum window", { width: 720, height: 480 });
export const DrawerPushes: StoryObj = stacked("Right drawer at 1700 px wide: pushes the terminal", { width: 1700, height: 800, drawer: true });
export const DrawerOverlays: StoryObj = stacked("Right drawer at 1180 px wide: overlays, the terminal would drop below 720", { width: 1180, height: 800, drawer: true });
export const LightTerminalInDarkApp: StoryObj = {
  render: () => (
    <ThemePair label="Terminal pinned light while the app is dark (and the reverse)" stack>
      <MainWindow width={1180} height={640} terminalSetting="light" />
    </ThemePair>
  ),
};
export const MacShortcuts: StoryObj = stacked("macOS shortcut labels (focus pill)", { width: 1180, height: 560, layout: "focus", os: "mac" });
```

- [ ] **Step 7: Run and commit**

Run: `pnpm vitest run` (PASS) and `pnpm tsc --noEmit` (clean). If the `inline-size:300px` assertion fails, copy the exact serialized style string React prints (it is `inline-size:300px` with no space).

```bash
git add -A src
git commit -m "feat(window): sidebar, 5-agent scenario and the main window composite

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Review page for sub-review B, real xterm in the page, checks

**Files:**
- Create: `scripts/review-terminal.js`
- Modify: `src/design/review/renderReview.tsx`, `scripts/build-review.mjs`
- Test: `src/design/review/renderReview.test.tsx`

**Interfaces:**
- Consumes: stories from Tasks 8–10; `PALETTES`, `MIN_CONTRAST`, `SCRIPTED_SESSION`, `SCRIPTED_SIZE`.
- Produces: `renderReview("2b")`; a page whose `.scripted-terminal` hosts are mounted by an inlined xterm, with page controls "Terminal theme" (Follow app / Always dark / Always light) and "Contrast protection".

- [ ] **Step 1: Extend the renderer test (failing)**

Append to `src/design/review/renderReview.test.tsx`:

```tsx
describe("renderReview 2b", () => {
  it("adds the lane and the main window to the approve card and batch 1", () => {
    const html = renderReview("2b");
    expect(html).toContain('id="approve-card"');
    expect(html).toContain('id="needs-you-lane"');
    expect(html).toContain('id="main-window"');
  });
  it("renders every window story", () => {
    const html = renderReview("2b");
    for (const name of ["Five agents", "Split view", "Focus mode", "Collapsed rail", "Auto collapse at 940", "Minimum window 720", "Drawer pushes", "Drawer overlays", "Light terminal in dark app"])
      expect(html, name).toContain(name);
  });
  it("leaves terminals as empty labelled hosts for the page script", () => {
    expect(renderReview("2b")).toContain('class="scripted-terminal"');
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `pnpm vitest run src/design/review`
Expected: FAIL (`needs-you-lane` missing, or `2b` empty).

- [ ] **Step 3: Renderer**

In `src/design/review/renderReview.tsx` import `* as Lane from "../../features/lane/Lane.stories";` and `* as Window from "../../features/window/MainWindow.stories";` and set

```tsx
"2b": [
  [Lane, "The lane header is always there at 28 px. The oldest waiting request is the target and shows as a full card; the rest are 36 px one-liners; it never takes more than 40% of the sidebar."],
  [Window, "The 5-agent scenario in the real layout, with a real xterm.js terminal that follows the theme. Use the Terminal theme control above to pin it dark or light, and Contrast protection to see what it rescues."],
],
```

The story titles are `"Needs-you lane"` (id `needs-you-lane`) and `"Main window"` (id `main-window`).

- [ ] **Step 4: The page script**

`scripts/review-terminal.js` (plain browser JavaScript, inlined into the page after xterm; it reads a JSON block the builder writes):

```js
// Mounts a real xterm.js into every .scripted-terminal on the review page and applies the page's terminal controls.
(() => {
  const data = JSON.parse(document.getElementById("terminal-data").textContent);
  const mounted = [];

  // Same rule as schemeFromComputed in src/features/terminal/terminalTheme.ts.
  function schemeOf(el) {
    const cs = getComputedStyle(el).colorScheme;
    if (cs === "dark" || cs === "light") return cs;
    return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  function options(el) {
    const select = document.getElementById("terminal-theme");
    const setting = el.dataset.setting !== "follow-app" ? el.dataset.setting : select.value;
    const scheme = setting === "follow-app" ? schemeOf(el) : setting;
    const protect = document.getElementById("terminal-contrast").checked;
    return { theme: data.palettes[scheme], minimumContrastRatio: protect ? data.minContrast : 1 };
  }

  function mount(el) {
    const fontFamily = getComputedStyle(document.documentElement).getPropertyValue("--font-mono").trim();
    const term = new Terminal({ fontFamily, fontSize: 13, cols: data.size.cols, rows: data.size.rows, disableStdin: true, cursorBlink: false, scrollback: 1000, ...options(el) });
    const fit = new FitAddon.FitAddon();
    term.loadAddon(fit);
    term.open(el);
    term.write(data.session);
    fit.fit();
    new ResizeObserver(() => fit.fit()).observe(el);
    mounted.push({ el, term });
  }

  function apply() {
    for (const { el, term } of mounted) {
      const o = options(el);
      term.options.theme = o.theme;
      term.options.minimumContrastRatio = o.minimumContrastRatio;
    }
  }

  document.querySelectorAll(".scripted-terminal").forEach(mount);
  document.getElementById("terminal-theme").addEventListener("change", apply);
  document.getElementById("terminal-contrast").addEventListener("change", apply);
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", apply);
})();
```

- [ ] **Step 5: Build script**

First confirm the xterm UMD globals: run `grep -c "exports.Terminal\|Terminal=" node_modules/@xterm/xterm/lib/xterm.js` and `grep -c "FitAddon" node_modules/@xterm/addon-fit/lib/addon-fit.js` (both non-zero). If the xterm bundle does not define a global `Terminal`, load it with `new Function` and read `module.exports`; record a ruling.

In `scripts/build-review.mjs`:
- add `"src/features/lane/Lane.css"`, `"src/features/window/MainWindow.css"`, `"src/features/window/Sidebar.css"`, `"src/features/window/SessionHeader.css"`, `"src/features/window/Rail.css"`, `"src/components/Chip/Chip.css"`, `"src/features/terminal/ScriptedTerminal.css"`, `"node_modules/@xterm/xterm/css/xterm.css"` to `CSS` (before the `SessionRow` entry is fine);
- load `PALETTES`, `MIN_CONTRAST`, `SCRIPTED_SESSION`, `SCRIPTED_SIZE` inside the same Vite SSR server session: `const t = await server.ssrLoadModule("/src/features/terminal/palettes.ts"); const s = await server.ssrLoadModule("/src/features/terminal/scriptedSession.ts");` and keep `terminalData = { palettes: t.PALETTES, minContrast: t.MIN_CONTRAST, session: s.SCRIPTED_SESSION, size: s.SCRIPTED_SIZE }`;
- add `META["2b"]` (nav entries `["needs-you-lane","Lane"]`, `["main-window","Window"]`, plus the earlier ones) with lede "Sub-review B of 3 in batch 2: the main window with a real terminal. Sub-review A (approve card) and batch 1 are signed off and sit below." and checks:
  1. "In 5 seconds you can name which two sessions need you and why (the 5-second test)."
  2. "The lane header is always there; the oldest request is the expanded card; Twelve waiting scrolls and says +11 more."
  3. "Split view: the active pane is obvious, the other header recedes, terminal text is not dimmed."
  4. "Focus mode, the rail, the 940 px auto-collapse and the 720 px window all hold together."
  5. "Pin the terminal light while the app is dark (and the reverse) with the Terminal theme control: the palette changes at once, nothing re-flows, dim text is lifted by Contrast protection."
  6. "It still reads well at 150% browser zoom, and the focus ring is visible in both themes."
- when the batch is `2b` or later, add to the review bar: `<label for="terminal-theme">Terminal theme <select id="terminal-theme"><option value="follow-app">Follow app</option><option value="dark">Always dark</option><option value="light">Always light</option></select></label><label for="terminal-contrast"><input type="checkbox" id="terminal-contrast" checked> Contrast protection</label>`;
- when the batch is `2b` or later, before the closing `</main>` script block, inline (replacing any `</script` inside a file with `<\/script` first): `<script>` + the contents of `node_modules/@xterm/xterm/lib/xterm.js` + `</script>`, the same for `node_modules/@xterm/addon-fit/lib/addon-fit.js`, `<script type="application/json" id="terminal-data">` + `JSON.stringify(terminalData)` (escape `</` as `<\/`) + `</script>`, then `<script>` + `readFileSync("scripts/review-terminal.js", "utf8")` + `</script>`. The page's own existing script keeps handling accent, motion and replay.

- [ ] **Step 6: Run the tests, build and check in a browser**

Run: `pnpm vitest run` (PASS) and `pnpm tsc --noEmit` (clean), then

```bash
pnpm review:build "$TEMP/review.html" 2b
```

With the static server running on `$TEMP`, in the Playwright browser open `http://127.0.0.1:6011/review.html` and:

1. Run (the terminals mount and show text):

```js
async (page) => {
  await page.waitForSelector(".scripted-terminal .xterm-screen");
  return page.evaluate(() => ({
    terminals: document.querySelectorAll(".scripted-terminal .xterm").length,
    text: document.querySelector(".scripted-terminal .xterm-rows")?.textContent.includes("Claude Code"),
  }));
}
```

Expected: `terminals` is at least 20 (every window story times two themes) and `text` is true.

2. Switch the theme control and confirm the palette follows:

```js
async (page) => {
  const bgs = async () => page.evaluate(() => [...document.querySelectorAll(".scripted-terminal .xterm-viewport")].slice(0, 4).map((v) => getComputedStyle(v).backgroundColor));
  const follow = await bgs();
  await page.selectOption("#terminal-theme", "light");
  const light = await bgs();
  await page.selectOption("#terminal-theme", "dark");
  const dark = await bgs();
  return { follow, light, dark };
}
```

Expected: `light` entries are all white (`rgb(255, 255, 255)`), `dark` all black (`rgb(0, 0, 0)`), `follow` is dark for the dark panels and white for the light ones.

3. Contrast protection: with "Always light" selected, find the span for "Read 84 lines" (painted `170,170,170`) and compute its contrast against white with and without the checkbox:

```js
async (page) => {
  await page.selectOption("#terminal-theme", "light");
  const lum = (c) => { const [r, g, b] = c.match(/\d+/g).slice(0, 3).map(Number).map((v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
  const ratio = () => page.evaluate((src) => { const f = new Function("c", `return (${src})(c)`); const span = [...document.querySelectorAll(".scripted-terminal .xterm-rows span")].find((s) => s.textContent.includes("Read 84 lines")); const l = f(getComputedStyle(span).color); return (1.05) / (l + 0.05); }, lum.toString());
  await page.setChecked("#terminal-contrast", false);
  const off = await ratio();
  await page.setChecked("#terminal-contrast", true);
  const on = await ratio();
  return { off, on };
}
```

Expected: `off` is below 4.5 (about 2.3) and `on` is at least 4.5. If the DOM renderer does not rewrite the span colour (xterm's DOM renderer applies minimum contrast through its own colour pass), take the evidence from a screenshot of both states instead and say so in the ledger.

4. Accessibility, with the xterm internals excluded from `color-contrast` because the terminal's colours are program output governed by the contrast protection above:

```js
async (page) => {
  await page.addScriptTag({ url: "http://127.0.0.1:6011/axe.min.js" });
  const out = [];
  for (const scheme of ["dark", "light"]) {
    await page.emulateMedia({ colorScheme: scheme });
    out.push({ scheme, v: await page.evaluate(async () => (await axe.run({ exclude: [[".xterm"]] }, { resultTypes: ["violations"] })).violations.map((v) => ({ id: v.id, impact: v.impact, nodes: v.nodes.length, sample: v.nodes[0].html.slice(0, 140) }))) });
  }
  return out;
}
```

Expected: nothing `serious` or `critical`. Fix real findings; record the rest as `Task 11: minor (deferred)` lines.

5. Measure the lane. The design numbers (target card 120 to 176 px) are a starting point; the real card decides:

```js
async (page) => page.evaluate(() => [...document.querySelectorAll(".lane .approve-card")].map((c) => Math.round(c.getBoundingClientRect().height)))
```

If the tallest card is above `LANE.expanded` (176), set `LANE.expanded` in `src/features/lane/lane.ts` to that height rounded up to a multiple of 4 (the tests compute from `LANE`, so none need editing), update the "120–176 px" figure in `docs/PLAN.md` to the measured range, ledger `Ruling: lane target card is N px, not 176 — the real card with its Always and terminal links needs it — cost if wrong: one constant`, rebuild, and confirm the five-agents window's lane shows both requests with no scrolling (raise that story's height if the 40% cap bites).

6. Screenshots (`browser_take_screenshot`) of: the five-agents window, split view, focus mode, the rail, the 720 px window, the light terminal in the dark app, all at the page's default width, and the five-agents window at 150% zoom (`browser_resize` to 960). Read them. Confirm: the lane header and cards are not clipped, the 288 px rows read (name, project, branch), the terminal fills its pane with no scrollbar, the focus pill is quiet, the rail monograms line up. Fix what is wrong and re-check. Also check, in the rail and the focus pill, that no glyph is exposed twice to a screen reader.

- [ ] **Step 7: Commit and publish for sign-off**

```bash
git add -A src scripts
git commit -m "feat(ui): review page for sub-review B with a real xterm and terminal theme controls

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

Build the page to the scratchpad file (`marshell-design-review.html`) with `pnpm review:build <path> 2b`, read the head of the file and the new sections' CSS before publishing, then publish to the review artifact with the label "Sub-review B: main window". Tell the user what to check and wait for sign-off before Task 12.

---

## Sub-review C: launcher and command palette

### Task 12: Filters, highlighting and letter picking

**Files:**
- Create: `src/features/launcher/filter.ts`
- Test: `src/features/launcher/filter.test.ts`

**Interfaces:**
- Consumes: `Command` from `src/lib/keymap.ts`.
- Produces (exact):

```ts
export type Project = { id: string; name: string; branch?: string; path: string; lastUsedMs: number };
export type Range = [start: number, end: number];            // code-point indices, end exclusive
export function matchRanges(query: string, text: string): Range[] | null;
export type Hit<T> = { item: T; ranges: Range[] };
export function projectLabel(p: Project): string;             // "my-app · main" or "my-app"
export function filterProjects(query: string, projects: Project[]): Hit<Project>[];
export function filterCommands(query: string, commands: Command[]): Hit<Command>[];
export type Segment = { text: string; hit: boolean };
export function highlight(text: string, ranges: Range[]): Segment[];
export function pickByLetter(labels: string[], key: string, current?: number): number;
```

- [ ] **Step 1: Write the failing test**

`src/features/launcher/filter.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { COMMANDS } from "../../lib/keymap";
import { filterCommands, filterProjects, highlight, matchRanges, pickByLetter, projectLabel, type Project } from "./filter";

const P = (id: string, name: string, branch: string | undefined, lastUsedMs: number): Project => ({ id, name, branch, path: `C:/dev/${name}`, lastUsedMs });
const projects = [P("a", "my-app", "main", 300), P("b", "app-server", "develop", 200), P("c", "snapshot", undefined, 100), P("d", "marketing", "feat/pricing", 400)];

describe("matchRanges", () => {
  it("matches a subsequence, case-insensitively, and reports where", () => {
    expect(matchRanges("MA", "my-app")).toEqual([[0, 1], [3, 4]]);
    expect(matchRanges("app", "my-app")).toEqual([[3, 6]]);
  });
  it("returns null when the text does not contain the query's letters in order", () => {
    expect(matchRanges("zz", "my-app")).toBeNull();
    expect(matchRanges("ppa", "my-app")).toBeNull();
  });
  it("matches everything, with nothing to highlight, for an empty or blank query", () => {
    expect(matchRanges("", "x")).toEqual([]);
    expect(matchRanges("   ", "x")).toEqual([]);
  });
  it("counts code points, so emoji and CJK ranges line up (review focus 5)", () => {
    expect(matchRanges("\u{1F680}", "go \u{1F680} now")).toEqual([[3, 4]]);
    expect(matchRanges("\u8a8d", "\u7d71\u8a8d\u8a3c")).toEqual([[1, 2]]);
  });
});

describe("filterProjects (review focus 5)", () => {
  it("returns everything in the remembered order for an empty query", () => {
    expect(filterProjects("", projects).map((h) => h.item.id)).toEqual(["d", "a", "b", "c"]);
    expect(filterProjects("  ", projects).map((h) => h.item.id)).toEqual(["d", "a", "b", "c"]);
  });
  it("never throws on characters that look like a pattern", () => {
    for (const q of ["(", "[", "\\", ".*", "+", "?", "$", "^", "{1,2}", "|", "\u{1F680}"]) expect(() => filterProjects(q, projects), q).not.toThrow();
  });
  it("treats those characters literally", () => {
    expect(filterProjects(".*", projects)).toEqual([]);
    expect(filterProjects("feat/pr", projects).map((h) => h.item.id)).toEqual(["d"]);
  });
  it("ranks a word-start match above a scattered one", () => {
    expect(filterProjects("ap", projects).map((h) => h.item.id)[0]).toBe("b");
  });
  it("breaks equal scores by most recently used", () => {
    const same = [P("x", "web", undefined, 1), P("y", "web", undefined, 9)];
    expect(filterProjects("web", same).map((h) => h.item.id)).toEqual(["y", "x"]);
  });
  it("drops projects that do not match", () => {
    expect(filterProjects("zzz", projects)).toEqual([]);
  });
  it("labels a project with its branch", () => {
    expect(projectLabel(projects[0]!)).toBe("my-app \u00b7 main");
    expect(projectLabel(projects[2]!)).toBe("snapshot");
  });
  it("returns ranges against the label", () => {
    const hit = filterProjects("main", projects)[0]!;
    expect(highlight(projectLabel(hit.item), hit.ranges).filter((s) => s.hit).map((s) => s.text).join("")).toBe("main");
  });
});

describe("filterCommands", () => {
  it("lists every command, in order, for an empty query", () => {
    expect(filterCommands("", COMMANDS).map((h) => h.item.id)).toEqual(COMMANDS.map((c) => c.id));
  });
  it("finds Split view by typing split", () => {
    expect(filterCommands("split", COMMANDS)[0]!.item.id).toBe("split");
  });
  it("finds a command by scattered letters", () => {
    expect(filterCommands("stse", COMMANDS).map((h) => h.item.id)).toContain("launcher");
  });
  it("returns nothing for a query that matches nothing", () => {
    expect(filterCommands("qqqq", COMMANDS)).toEqual([]);
  });
});

describe("highlight", () => {
  it("splits text into hit and plain segments", () => {
    expect(highlight("my-app", [[3, 6]])).toEqual([{ text: "my-", hit: false }, { text: "app", hit: true }]);
  });
  it("returns the whole text as one plain segment with no ranges", () => {
    expect(highlight("abc", [])).toEqual([{ text: "abc", hit: false }]);
  });
  it("keeps emoji whole", () => {
    expect(highlight("a\u{1F680}b", [[1, 2]])).toEqual([{ text: "a", hit: false }, { text: "\u{1F680}", hit: true }, { text: "b", hit: false }]);
  });
});

describe("pickByLetter", () => {
  const labels = ["Claude", "Codex", "Gemini"];
  it("picks the first option starting with the letter", () => {
    expect(pickByLetter(labels, "g")).toBe(2);
    expect(pickByLetter(labels, "G")).toBe(2);
  });
  it("cycles through options that share the letter", () => {
    expect(pickByLetter(labels, "c", -1)).toBe(0);
    expect(pickByLetter(labels, "c", 0)).toBe(1);
    expect(pickByLetter(labels, "c", 1)).toBe(0);
  });
  it("returns -1 when nothing matches or the key is not a single letter", () => {
    expect(pickByLetter(labels, "x")).toBe(-1);
    expect(pickByLetter(labels, "1")).toBe(-1);
    expect(pickByLetter(labels, "Enter")).toBe(-1);
    expect(pickByLetter(labels, "")).toBe(-1);
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `pnpm vitest run src/features/launcher/filter.test.ts`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement**

`src/features/launcher/filter.ts`:

```ts
import type { Command } from "../../lib/keymap";

export type Project = { id: string; name: string; branch?: string; path: string; lastUsedMs: number };
/** Code-point indices into the text, end exclusive. */
export type Range = [start: number, end: number];
export type Hit<T> = { item: T; ranges: Range[] };
export type Segment = { text: string; hit: boolean };

const lower = (cp: string) => cp.toLocaleLowerCase();
const WORD_START = new Set([" ", "-", "_", "/", "\\", ".", "\u00b7"]);

/**
 * Case-insensitive subsequence match, with no regular expressions so no input can be read as a pattern.
 * Returns the matched ranges (merged when adjacent), [] for an empty query, or null for no match.
 */
export function matchRanges(query: string, text: string): Range[] | null {
  const q = Array.from(query.trim(), lower);
  if (q.length === 0) return [];
  const t = Array.from(text, lower);
  const ranges: Range[] = [];
  let qi = 0;
  for (let i = 0; i < t.length && qi < q.length; i++) {
    if (t[i] !== q[qi]) continue;
    const last = ranges[ranges.length - 1];
    if (last && last[1] === i) last[1] = i + 1;
    else ranges.push([i, i + 1]);
    qi++;
  }
  return qi === q.length ? ranges : null;
}

/** Longer runs and word-start matches score higher; shorter text breaks near-ties. */
function score(ranges: Range[], text: string): number {
  const t = Array.from(text);
  let s = 0;
  for (const [a, b] of ranges) {
    s += (b - a) * 2 + (b - a - 1) * 2;
    if (a === 0 || WORD_START.has(t[a - 1] ?? "")) s += 6;
  }
  return s - t.length * 0.01;
}

function rank<T>(items: T[], query: string, label: (i: T) => string, recent: (i: T) => number): Hit<T>[] {
  const scored: Array<{ hit: Hit<T>; score: number; recent: number }> = [];
  for (const item of items) {
    const text = label(item);
    const ranges = matchRanges(query, text);
    if (ranges === null) continue;
    scored.push({ hit: { item, ranges }, score: ranges.length === 0 ? 0 : score(ranges, text), recent: recent(item) });
  }
  if (query.trim() === "") return scored.sort((a, b) => b.recent - a.recent).map((s) => s.hit);
  return scored.sort((a, b) => b.score - a.score || b.recent - a.recent).map((s) => s.hit);
}

export const projectLabel = (p: Project) => (p.branch ? `${p.name} \u00b7 ${p.branch}` : p.name);

/** Most recently used first when the query is empty; best match first otherwise. */
export function filterProjects(query: string, projects: Project[]): Hit<Project>[] {
  return rank(projects, query, projectLabel, (p) => p.lastUsedMs);
}

/** Keeps the keymap's order for an empty query; best match first otherwise. */
export function filterCommands(query: string, commands: Command[]): Hit<Command>[] {
  if (query.trim() === "") return commands.map((item) => ({ item, ranges: [] }));
  return rank(commands, query, (c) => c.label, () => 0);
}

/** Splits text into hit and plain segments, by code point so emoji stay whole. */
export function highlight(text: string, ranges: Range[]): Segment[] {
  const cps = Array.from(text);
  const out: Segment[] = [];
  let at = 0;
  for (const [a, b] of ranges) {
    if (a > at) out.push({ text: cps.slice(at, a).join(""), hit: false });
    out.push({ text: cps.slice(a, b).join(""), hit: true });
    at = b;
  }
  if (at < cps.length || out.length === 0) out.push({ text: cps.slice(at).join(""), hit: false });
  return out;
}

/** Letter-to-value picking in a chip: the next option after `current` whose label starts with the key, wrapping. */
export function pickByLetter(labels: string[], key: string, current = -1): number {
  if (!/^\p{L}$/u.test(key)) return -1;
  const k = lower(key);
  for (let step = 1; step <= labels.length; step++) {
    const i = (current + step + labels.length) % labels.length;
    if (lower(Array.from(labels[i] ?? "")[0] ?? "") === k) return i;
  }
  return -1;
}
```

- [ ] **Step 4: Run and commit**

Run: `pnpm vitest run src/features/launcher` (PASS) and `pnpm tsc --noEmit` (clean). If "ap ranks app-server first" fails, the cause is the scoring weights: both "my-app" (word-start at index 3 after "-") and "app-server" (index 0) get +6, so the tie breaks on text length; make the start-of-text bonus larger (`a === 0 ? 8 : …`) rather than loosening the test.

```bash
git add src/features/launcher
git commit -m "feat(launcher): pattern-free project and command filters, highlighting and letter picking

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 13: Launcher sheet

**Files:**
- Create: `src/features/launcher/launcher.ts`, `src/features/launcher/fixtures.ts`, `src/features/launcher/Launcher.tsx`, `src/features/launcher/Launcher.css`, `src/features/launcher/Launcher.stories.tsx`
- Test: `src/features/launcher/launcher.test.ts`, `src/features/launcher/Launcher.test.tsx`, `src/features/launcher/Launcher.css.test.ts`

**Interfaces:**
- Consumes: `Project`, `highlight`, `projectLabel`, `filterProjects`, `AgentId`/`AGENT_NAMES`, `AgentMark`.
- Produces (exact):

```ts
// launcher.ts
export type Mode = "Manual" | "Plan" | "Auto-edit";
export type Pace = "Quick" | "Balanced" | "Deep";
export const MODES: Mode[]; export const PACES: Pace[];
export const LAUNCH_AGENTS: Array<{ id: AgentId; label: string; cli: string }>;
export type LauncherChoice = { agent: AgentId; projectId: string; mode: Mode; pace: Pace; worktree: boolean };
export type Remembered = Partial<Pick<LauncherChoice, "mode" | "pace" | "worktree">>;
export function launcherSentence(c: LauncherChoice, projects: Project[]): string;
export function startingText(agent: AgentId, projectName: string): string;
export function applyRemembered(c: LauncherChoice, r: Remembered | undefined): LauncherChoice;
export function rememberedNote(projectName: string, r: Remembered | undefined): string | null;
// Launcher.tsx
export type LauncherView = {
  choice: LauncherChoice;
  /** Every folder the launcher knows, most recently used first; the chip and the sentence read from it. */
  projects: Project[];
  /** Non-null when the folder chip is open for typing; the listbox shows `filterProjects(query, projects)`. */
  query: string | null;
  activeIndex: number;
  remembered?: Remembered;
  prompt?: string;
};
export function Launcher(props: {
  view: LauncherView;
  onChange?: (patch: Partial<LauncherChoice>) => void;
  onQuery?: (q: string | null) => void;
  onPick?: (projectId: string) => void;
  onStart?: () => void;
}): JSX.Element;
```

- [ ] **Step 1: Write the failing logic tests**

`src/features/launcher/launcher.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { MODES, PACES, applyRemembered, launcherSentence, rememberedNote, startingText, type LauncherChoice } from "./launcher";
import { PROJECTS } from "./fixtures";

const base: LauncherChoice = { agent: "claude", projectId: "my-app", mode: "Plan", pace: "Quick", worktree: false };

describe("launcherSentence", () => {
  it("reads as one sentence", () => {
    expect(launcherSentence(base, PROJECTS)).toBe("Start Claude in my-app on main, Plan mode, Quick, no worktree");
  });
  it("says worktree when it is on, and drops the branch when there is none", () => {
    expect(launcherSentence({ ...base, worktree: true, projectId: "scratch" }, PROJECTS)).toBe("Start Claude in scratch, Plan mode, Quick, in a new worktree");
  });
  it("copes with a project that is not in the list", () => {
    expect(launcherSentence({ ...base, projectId: "gone" }, PROJECTS)).toContain("in an unknown folder");
  });
});

describe("options", () => {
  it("has the modes and paces the sentence uses", () => {
    expect(MODES).toEqual(["Manual", "Plan", "Auto-edit"]);
    expect(PACES).toEqual(["Quick", "Balanced", "Deep"]);
  });
});

describe("startingText", () => {
  it("is what the tab shows at once, before the process answers", () => {
    expect(startingText("claude", "my-app")).toBe("Starting Claude Code in my-app\u2026");
    expect(startingText("gemini", "my-app")).toBe("Starting Gemini CLI in my-app\u2026");
    expect(startingText("codex", "my-app")).toBe("Starting Codex CLI in my-app\u2026");
  });
});

describe("remembered choices", () => {
  it("applies what was remembered for the project and leaves the rest", () => {
    expect(applyRemembered(base, { mode: "Auto-edit", worktree: true })).toEqual({ ...base, mode: "Auto-edit", worktree: true });
    expect(applyRemembered(base, undefined)).toEqual(base);
  });
  it("explains what it applied, naming the project", () => {
    expect(rememberedNote("my-app", { mode: "Plan", pace: "Quick" })).toBe("Remembered for my-app: Plan \u00b7 Quick. Change a choice to update it.");
    expect(rememberedNote("my-app", { worktree: true })).toBe("Remembered for my-app: worktree. Change a choice to update it.");
  });
  it("says nothing when nothing was remembered", () => {
    expect(rememberedNote("my-app", undefined)).toBeNull();
    expect(rememberedNote("my-app", {})).toBeNull();
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `pnpm vitest run src/features/launcher/launcher.test.ts`
Expected: FAIL (modules not found).

- [ ] **Step 3: Implement logic and fixtures**

`src/features/launcher/launcher.ts`:

```ts
import { AGENT_NAMES, type AgentId } from "../../components/AgentMark/agents";
import type { Project } from "./filter";

export type Mode = "Manual" | "Plan" | "Auto-edit";
export type Pace = "Quick" | "Balanced" | "Deep";
export const MODES: Mode[] = ["Manual", "Plan", "Auto-edit"];
export const PACES: Pace[] = ["Quick", "Balanced", "Deep"];
export const LAUNCH_AGENTS: Array<{ id: AgentId; label: string; cli: string }> = [
  { id: "claude", label: "Claude", cli: "Claude Code" },
  { id: "codex", label: "Codex", cli: "Codex CLI" },
  { id: "gemini", label: "Gemini", cli: "Gemini CLI" },
];

export type LauncherChoice = { agent: AgentId; projectId: string; mode: Mode; pace: Pace; worktree: boolean };
export type Remembered = Partial<Pick<LauncherChoice, "mode" | "pace" | "worktree">>;

/** The launcher as one sentence, for the dialog's accessible description. */
export function launcherSentence(c: LauncherChoice, projects: Project[]): string {
  const agent = LAUNCH_AGENTS.find((a) => a.id === c.agent)?.label ?? AGENT_NAMES[c.agent];
  const p = projects.find((x) => x.id === c.projectId);
  const where = p ? (p.branch ? `${p.name} on ${p.branch}` : p.name) : "an unknown folder";
  return `Start ${agent} in ${where}, ${c.mode} mode, ${c.pace}, ${c.worktree ? "in a new worktree" : "no worktree"}`;
}

/** What the new tab shows within 100 ms, before the process answers. */
export function startingText(agent: AgentId, projectName: string): string {
  const cli = LAUNCH_AGENTS.find((a) => a.id === agent)?.cli ?? AGENT_NAMES[agent];
  return `Starting ${cli} in ${projectName}\u2026`;
}

export function applyRemembered(c: LauncherChoice, r: Remembered | undefined): LauncherChoice {
  return { ...c, ...r };
}

export function rememberedNote(projectName: string, r: Remembered | undefined): string | null {
  if (!r) return null;
  const parts = [r.mode, r.pace, r.worktree ? "worktree" : undefined].filter((x): x is string => Boolean(x));
  return parts.length === 0 ? null : `Remembered for ${projectName}: ${parts.join(" \u00b7 ")}. Change a choice to update it.`;
}
```

`src/features/launcher/fixtures.ts`:

```ts
import type { Project } from "./filter";
import type { LauncherChoice, Remembered } from "./launcher";

const DAY = 86_400_000;

export const PROJECTS: Project[] = [
  { id: "my-app", name: "my-app", branch: "main", path: "C:/dev/my-app", lastUsedMs: 9 * DAY },
  { id: "payments", name: "payments", branch: "fix/retry-backoff", path: "C:/dev/payments", lastUsedMs: 8 * DAY },
  { id: "marketing", name: "marketing", branch: "feat/pricing-page", path: "C:/dev/marketing", lastUsedMs: 7 * DAY },
  { id: "terraform", name: "terraform", branch: "chore/upgrade-aws-provider", path: "C:/dev/terraform", lastUsedMs: 6 * DAY },
  { id: "scratch", name: "scratch", path: "C:/Users/Home Office/scratch", lastUsedMs: 5 * DAY },
  { id: "tools", name: "tools", branch: "main", path: "C:/dev/tools", lastUsedMs: 4 * DAY },
];

export const DEFAULT_CHOICE: LauncherChoice = { agent: "claude", projectId: "my-app", mode: "Manual", pace: "Balanced", worktree: false };
export const REMEMBERED: Record<string, Remembered> = { "my-app": { mode: "Plan", pace: "Quick" } };
```

Run: `pnpm vitest run src/features/launcher` (PASS).

- [ ] **Step 4: Write the failing component tests**

`src/features/launcher/Launcher.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DEFAULT_CHOICE, PROJECTS, REMEMBERED } from "./fixtures";
import { applyRemembered } from "./launcher";
import { Launcher, type LauncherView } from "./Launcher";

const view = (p: Partial<LauncherView> = {}): LauncherView => ({ choice: DEFAULT_CHOICE, projects: PROJECTS, query: null, activeIndex: 0, ...p });
const html = (v: LauncherView = view()) => renderToStaticMarkup(<Launcher view={v} />);

describe("Launcher: empty", () => {
  const h = html();
  it("is a modal dialog with an accessible sentence", () => {
    expect(h).toContain('role="dialog"');
    expect(h).toContain('aria-modal="true"');
    expect(h).toContain("Start Claude in my-app on main, Manual mode, Balanced, no worktree");
  });
  it("reads as one sentence of chips, in order", () => {
    const order = ["Claude", "my-app \u00b7 main", "Manual", "Balanced", "worktree"].map((s) => h.indexOf(s));
    expect(order.every((i) => i >= 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
    expect(h).toContain(" in ");
  });
  it("makes every chip a button the keyboard reaches with Tab", () => {
    expect((h.match(/class="launcher__chip"/g) ?? []).length).toBe(5);
    expect(h).not.toContain('tabindex="-1" class="launcher__chip"');
  });
  it("has a prompt box and a hint line naming Enter and Esc", () => {
    expect(h).toContain("<textarea");
    expect(h).toContain("Enter");
    expect(h).toContain("Esc");
  });
  it("shows no project list and no remembered note", () => {
    expect(h).not.toContain('role="listbox"');
    expect(h).not.toContain("Remembered for");
  });
  it("shows the worktree chip as a checkbox", () => {
    expect(h).toContain('role="checkbox"');
    expect(h).toContain('aria-checked="false"');
  });
});

describe("Launcher: filtering", () => {
  const h = html(view({ query: "ap" }));
  it("turns the project chip into a combobox with a listbox of matches", () => {
    expect(h).toContain('role="combobox"');
    expect(h).toContain('role="listbox"');
    expect(h).toContain('aria-expanded="true"');
  });
  it("highlights what matched, and points at the active option", () => {
    expect(h).toContain("<mark");
    expect(h).toMatch(/aria-activedescendant="launcher-opt-0"/);
    expect(h).toContain('id="launcher-opt-0"');
  });
  it("shows an empty state when nothing matches", () => {
    expect(html(view({ query: "zzz" }))).toContain("No folder matches");
  });
  it("renders hostile text as text", () => {
    const evil = [{ ...PROJECTS[0]!, name: "<img src=x onerror=alert(1)>" }];
    const out = html(view({ query: "i", projects: evil, choice: { ...DEFAULT_CHOICE, projectId: PROJECTS[0]!.id } }));
    expect(out).not.toContain("<img");
  });
});

describe("Launcher: remembered for this project", () => {
  const choice = applyRemembered(DEFAULT_CHOICE, REMEMBERED["my-app"]);
  const h = html(view({ choice, remembered: REMEMBERED["my-app"] }));
  it("applies what was remembered and says so", () => {
    expect(h).toContain("Plan");
    expect(h).toContain("Quick");
    expect(h).toContain("Remembered for my-app: Plan \u00b7 Quick. Change a choice to update it.");
  });
});

describe("Launcher: hygiene", () => {
  it("never puts a literal colour in markup", () => {
    expect(html()).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
  it("uses sentence case, with no exclamation marks", () => {
    expect(html()).not.toContain("!");
  });
});
```

`src/features/launcher/Launcher.css.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Launcher.css", import.meta.url), "utf8");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("Launcher.css", () => {
  it("is a 560 px sheet 20% from the top", () => {
    expect(rule(".launcher__sheet")).toContain("inline-size: min(560px");
    expect(rule(".launcher__sheet")).toContain("inset-block-start: 20%");
  });
  it("styles focus and press on chips and options", () => {
    expect(css).toContain(".launcher__chip:focus-visible");
    expect(css).toContain(".launcher__chip:active");
    expect(css).toContain(".launcher__opt:active");
    expect(css).not.toMatch(/outline:\s*none/);
  });
  it("only styles hover where the device hovers", () => {
    expect(css.slice(0, css.indexOf("@media (hover: hover) and (pointer: fine)"))).not.toContain(":hover");
  });
  it("makes the matched letters stand out without relying on colour alone", () => {
    expect(rule(".launcher__opt mark")).toContain("font-weight");
    expect(rule(".launcher__opt mark")).toContain("text-decoration");
  });
  it("uses tokens and logical properties only", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).not.toMatch(/\b(left|right|top|bottom):/);
  });
});
```

- [ ] **Step 5: Run to see them fail**

Run: `pnpm vitest run src/features/launcher`
Expected: logic PASS; component and CSS tests FAIL.

- [ ] **Step 6: Implement the component**

`src/features/launcher/Launcher.tsx`:

```tsx
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { filterProjects, highlight, projectLabel, type Project, type Range } from "./filter";
import { LAUNCH_AGENTS, MODES, PACES, launcherSentence, rememberedNote, type LauncherChoice, type Remembered } from "./launcher";
import "./Launcher.css";

export type LauncherView = {
  choice: LauncherChoice;
  projects: Project[];
  query: string | null;
  activeIndex: number;
  remembered?: Remembered;
  prompt?: string;
};

type Props = {
  view: LauncherView;
  onChange?: (patch: Partial<LauncherChoice>) => void;
  onQuery?: (q: string | null) => void;
  onPick?: (projectId: string) => void;
  onStart?: () => void;
};

const Marked = ({ text, ranges }: { text: string; ranges: Range[] }) => (
  <>
    {highlight(text, ranges).map((s, i) =>
      s.hit ? (
        <mark key={i} dir="auto">
          {s.text}
        </mark>
      ) : (
        <span key={i}>{s.text}</span>
      ),
    )}
  </>
);

/**
 * The launcher sheet (docs/PLAN.md "Launch a session"): one sentence of chips and a prompt box. Tab moves between
 * the chips, typing in the folder chip filters folders, letters pick values, Enter starts. Presentational: the parent
 * owns the state.
 */
export function Launcher({ view, onChange, onQuery, onPick, onStart }: Props) {
  const { choice, projects, query, activeIndex, remembered } = view;
  const project = projects.find((p) => p.id === choice.projectId);
  const note = rememberedNote(project?.name ?? "this folder", remembered);
  const open = query !== null;
  const options = open ? filterProjects(query, projects) : [];
  const agent = LAUNCH_AGENTS.find((a) => a.id === choice.agent)!;
  const cycle = <T,>(all: T[], now: T) => all[(all.indexOf(now) + 1) % all.length]!;

  return (
    <div className="launcher" data-open={open || undefined}>
      <div className="launcher__backdrop" />
      <div className="launcher__sheet" role="dialog" aria-modal="true" aria-label="Start a session" aria-describedby="launcher-desc">
        <p className="launcher__sentence">
          <button type="button" className="launcher__chip" onClick={() => onChange?.({})}>
            <AgentMark agent={choice.agent} size={12} />
            {agent.label}
          </button>
          <span> in </span>
          {open ? (
            <input
              className="launcher__input"
              role="combobox"
              aria-expanded="true"
              aria-controls="launcher-list"
              aria-activedescendant={options.length > 0 ? `launcher-opt-${activeIndex}` : undefined}
              aria-label="Folder"
              value={query}
              onChange={(e) => onQuery?.(e.target.value)}
              autoFocus
            />
          ) : (
            <button type="button" className="launcher__chip" aria-haspopup="listbox" onClick={() => onQuery?.("")}>
              {project ? projectLabel(project) : "Choose a folder"}
            </button>
          )}
          <span> {"\u00b7"} </span>
          <button type="button" className="launcher__chip" onClick={() => onChange?.({ mode: cycle(MODES, choice.mode) })}>
            {choice.mode}
          </button>
          <span> {"\u00b7"} </span>
          <button type="button" className="launcher__chip" onClick={() => onChange?.({ pace: cycle(PACES, choice.pace) })}>
            {choice.pace}
          </button>
          <span> {"\u00b7"} </span>
          <button type="button" className="launcher__chip" role="checkbox" aria-checked={choice.worktree} onClick={() => onChange?.({ worktree: !choice.worktree })}>
            <span className="launcher__box" aria-hidden="true" data-checked={choice.worktree || undefined} />
            worktree
          </button>
        </p>

        {open && (
          <ul className="launcher__list" id="launcher-list" role="listbox" aria-label="Folders">
            {options.length === 0 ? (
              <li className="launcher__empty" role="status">
                No folder matches {"\u201c"}
                {query}
                {"\u201d"}
              </li>
            ) : (
              options.map((h, i) => (
                <li
                  key={h.item.id}
                  id={`launcher-opt-${i}`}
                  role="option"
                  aria-selected={i === activeIndex}
                  className="launcher__opt"
                  data-active={i === activeIndex || undefined}
                  onClick={() => onPick?.(h.item.id)}
                >
                  <span className="launcher__opt-name" dir="auto">
                    <Marked text={projectLabel(h.item)} ranges={h.ranges} />
                  </span>
                  <span className="launcher__opt-path" dir="auto">
                    {h.item.path}
                  </span>
                </li>
              ))
            )}
          </ul>
        )}

        {note && !open && <p className="launcher__note">{note}</p>}

        <textarea className="launcher__prompt" aria-label="First prompt (optional)" placeholder={`What should ${agent.label} do first? (optional)`} rows={3} defaultValue={view.prompt} />

        <footer className="launcher__footer">
          <span className="launcher__hint">
            <kbd>Tab</kbd> moves between choices {"\u00b7"} type a letter to pick {"\u00b7"} <kbd>Esc</kbd> closes
          </span>
          <button type="button" className="launcher__start" onClick={onStart}>
            Start <kbd>Enter</kbd>
          </button>
        </footer>
        <p className="launcher__sr" id="launcher-desc">
          {launcherSentence(choice, projects)}
        </p>
      </div>
    </div>
  );
}
```

The sentence for assistive technology sits last in the DOM on purpose: `aria-describedby` finds it anywhere, and the visual chips stay first.

`src/features/launcher/Launcher.css`:

```css
.launcher {
  position: relative;
  block-size: 100%;
  min-block-size: 480px;
  overflow: clip;
  color: var(--text-1);
  font-size: var(--text-13);
  line-height: 20px;
}
.launcher__backdrop {
  position: absolute;
  inset: 0;
  background: color-mix(in oklch, var(--bg-base) 55%, transparent);
}
.launcher__sheet {
  position: absolute;
  inset-block-start: 20%;
  inset-inline: 0;
  margin-inline: auto;
  inline-size: min(560px, calc(100% - 32px));
  display: grid;
  gap: var(--space-3);
  padding: var(--space-4);
  border: 1px solid var(--hairline);
  border-radius: 10px;
  background: var(--bg-overlay);
  box-shadow: 0 12px 40px color-mix(in oklch, var(--text-1) 22%, transparent);
}

.launcher__sentence {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--text-2);
  font-size: var(--text-15);
}
.launcher__chip,
.launcher__input {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  min-block-size: 28px;
  padding-inline: var(--space-2);
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
  background: var(--bg-raised);
  color: var(--text-1);
  font: inherit;
  font-weight: 500;
}
.launcher__input {
  inline-size: 14em;
  border-color: var(--accent);
}
.launcher__chip:focus-visible,
.launcher__input:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
.launcher__chip:active {
  box-shadow: inset 0 0 0 100vmax var(--selected);
}
.launcher__box {
  inline-size: 12px;
  block-size: 12px;
  border: 1.5px solid currentColor;
  border-radius: 3px;
}
.launcher__box[data-checked] {
  background: var(--accent);
  border-color: var(--accent);
}

.launcher__list {
  display: grid;
  margin: 0;
  padding: 0;
  list-style: none;
  max-block-size: 220px;
  overflow-y: auto;
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
  background: var(--bg-raised);
}
.launcher__opt {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-3);
  padding-block: 6px;
  padding-inline: var(--space-3);
}
.launcher__opt {
  border-inline-start: 3px solid transparent;
}
.launcher__opt[data-active] {
  background: var(--selected);
  border-inline-start: 3px solid var(--accent);
}
.launcher__opt:active {
  background: var(--selected);
}
.launcher__opt-name {
  min-inline-size: 0;
  overflow: clip;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
}
.launcher__opt mark {
  background: transparent;
  color: var(--text-1);
  font-weight: 700;
  text-decoration: underline;
  text-underline-offset: 2px;
}
.launcher__opt-path {
  min-inline-size: 0;
  overflow: clip;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-3);
  font-family: var(--font-mono);
  font-size: var(--text-11);
}
/* The sentence is for screen readers only. */
.launcher__sr {
  position: absolute;
  inline-size: 1px;
  block-size: 1px;
  margin: -1px;
  overflow: clip;
  clip-path: inset(50%);
  white-space: nowrap;
}
.launcher__empty {
  padding: var(--space-3);
  color: var(--text-2);
}

.launcher__note {
  margin: 0;
  color: var(--text-2);
  font-size: var(--text-12);
}

.launcher__prompt {
  inline-size: 100%;
  min-block-size: 72px;
  padding: var(--space-2);
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
  background: var(--bg-raised);
  color: var(--text-1);
  font: inherit;
  resize: vertical;
}
.launcher__prompt:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.launcher__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}
.launcher__hint {
  color: var(--text-2);
  font-size: var(--text-12);
}
.launcher kbd {
  padding-inline: 4px;
  border: 1px solid color-mix(in oklch, currentColor 40%, transparent);
  border-radius: 3px;
  font-family: var(--font-mono);
  font-size: var(--text-11);
  line-height: 16px;
}
.launcher__start {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-block-size: 32px;
  padding-inline: var(--space-3);
  border: 1px solid var(--accent);
  border-radius: var(--radius);
  background: var(--accent);
  color: var(--accent-ink);
  font: inherit;
  font-weight: 500;
}
.launcher__start:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
.launcher__start:active {
  box-shadow: inset 0 0 0 100vmax var(--selected);
}
@media (hover: hover) and (pointer: fine) {
  .launcher__chip:hover,
  .launcher__start:hover {
    box-shadow: inset 0 0 0 100vmax var(--hover);
  }
  .launcher__opt:hover {
    background: var(--hover);
  }
}

@media (forced-colors: active) {
  .launcher__sheet {
    border-color: CanvasText;
  }
  .launcher__opt[data-active] {
    outline: 2px solid Highlight;
  }
}
```

- [ ] **Step 7: Stories**

`src/features/launcher/Launcher.stories.tsx`:

```tsx
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ThemePair } from "../../design/ThemePair";
import { DEFAULT_CHOICE, PROJECTS, REMEMBERED } from "./fixtures";
import { Launcher, type LauncherView } from "./Launcher";
import { applyRemembered, startingText, type LauncherChoice } from "./launcher";

const meta: Meta = { title: "Launcher", parameters: { layout: "fullscreen" } };
export default meta;

const frame = (v: LauncherView, h: Partial<Parameters<typeof Launcher>[0]> = {}) => (
  <div style={{ blockSize: 460, position: "relative", background: "var(--bg-base)", border: "1px solid var(--hairline)", borderRadius: 8, overflow: "clip" }}>
    <Launcher view={v} {...h} />
  </div>
);
const base: LauncherView = { choice: DEFAULT_CHOICE, projects: PROJECTS, query: null, activeIndex: 0 };

export const Empty: StoryObj = { render: () => <ThemePair label="Empty: pre-filled with the most recent folder" stack>{frame(base)}</ThemePair> };
export const Filtering: StoryObj = { render: () => <ThemePair label={'Filtering: "ap" in the folder chip'} stack>{frame({ ...base, query: "ap" })}</ThemePair> };
export const NoMatch: StoryObj = { render: () => <ThemePair label="No folder matches" stack>{frame({ ...base, query: "zzz" })}</ThemePair> };
export const RememberedForThisProject: StoryObj = {
  render: () => (
    <ThemePair label="Remembered for this project: Plan and Quick are pre-set" stack>
      {frame({ ...base, choice: applyRemembered(DEFAULT_CHOICE, REMEMBERED["my-app"]), remembered: REMEMBERED["my-app"] })}
    </ThemePair>
  ),
};
export const AfterEnter: StoryObj = {
  render: () => (
    <ThemePair label="After Enter: the tab appears at once and never waits for the process (under 100 ms)">
      <p style={{ margin: 0, padding: "var(--space-3)", border: "1px solid var(--hairline)", borderRadius: "var(--radius)", fontFamily: "var(--font-mono)", color: "var(--text-2)" }}>
        {startingText("claude", "my-app")}
      </p>
    </ThemePair>
  ),
};

/** Try it: click the folder chip, type to filter, click an option, click a chip to cycle its value. */
function Live() {
  const [choice, setChoice] = useState<LauncherChoice>(DEFAULT_CHOICE);
  const [query, setQuery] = useState<string | null>(null);
  return frame(
    { choice, projects: PROJECTS, query, activeIndex: 0, remembered: REMEMBERED[choice.projectId] },
    {
      onChange: (p) => setChoice((c) => ({ ...c, ...p })),
      onQuery: setQuery,
      onPick: (id) => {
        setChoice((c) => applyRemembered({ ...c, projectId: id }, REMEMBERED[id]));
        setQuery(null);
      },
    },
  );
}
export const Interactive: StoryObj = { render: () => <Live /> };
```

- [ ] **Step 8: Run and commit**

Run: `pnpm vitest run` (PASS) and `pnpm tsc --noEmit` (clean).

```bash
git add -A src
git commit -m "feat(launcher): launcher sheet with chips, folder filtering and remembered choices

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 14: Command palette

**Files:**
- Create: `src/features/palette/Palette.tsx`, `src/features/palette/Palette.css`, `src/features/palette/Palette.stories.tsx`
- Test: `src/features/palette/Palette.test.tsx`, `src/features/palette/Palette.css.test.ts`

**Interfaces:**
- Consumes: `COMMANDS`, `Command`, `shortcutLabel`, `Os`, `filterCommands`, `highlight`, `Hit`.
- Produces (exact):

```tsx
export type PaletteView = { query: string; activeIndex: number };
export function Palette(props: { view: PaletteView; os?: Os; onQuery?: (q: string) => void; onRun?: (id: string) => void }): JSX.Element;
```

- [ ] **Step 1: Write the failing tests**

`src/features/palette/Palette.test.tsx`:

```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { COMMANDS } from "../../lib/keymap";
import { Palette } from "./Palette";

const html = (query = "", os: "windows" | "mac" | "linux" = "windows", activeIndex = 0) => renderToStaticMarkup(<Palette view={{ query, activeIndex }} os={os} />);

describe("Palette: empty", () => {
  const h = html();
  it("is a modal dialog with a combobox over a listbox", () => {
    expect(h).toContain('role="dialog"');
    expect(h).toContain('aria-modal="true"');
    expect(h).toContain('role="combobox"');
    expect(h).toContain('role="listbox"');
  });
  it("lists every command, grouped, in the keymap's order", () => {
    for (const c of COMMANDS) expect(h, c.id).toContain(c.label);
    for (const g of ["Sessions", "View", "Tools"]) expect(h).toContain(g);
  });
  it("shows each command's shortcut, rebindable, per OS", () => {
    expect(h).toContain("Ctrl+Shift+T");
    expect(html("", "mac")).toContain("\u2318T");
    expect(h).toContain("Rebind in Settings");
  });
  it("gives every option an id and points the input at the active one", () => {
    expect(h).toMatch(/aria-activedescendant="palette-opt-0"/);
    expect((h.match(/role="option"/g) ?? []).length).toBe(COMMANDS.length);
  });
  it("shows commands that have no shortcut without a gap or a dash", () => {
    expect(h).toContain("Open Doctor");
  });
});

describe("Palette: filtering", () => {
  it("ranks the best match first and highlights it", () => {
    const h = html("split");
    expect(h).toContain("Split</mark>");
    expect(h).toContain("<mark");
    expect((h.match(/role="option"/g) ?? []).length).toBeLessThan(COMMANDS.length);
  });
  it("drops the group headings while filtering", () => {
    expect(html("split")).not.toContain("palette__group");
  });
  it("says what happened when nothing matches", () => {
    const h = html("qqqq");
    expect(h).toContain("No command matches");
    expect(h).toContain('role="status"');
  });
  it("renders hostile text as text", () => {
    expect(html("<img src=x onerror=alert(1)>")).not.toContain("<img");
  });
});

describe("Palette: hygiene", () => {
  it("never puts a literal colour in markup", () => {
    expect(html()).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});
```

`src/features/palette/Palette.css.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Palette.css", import.meta.url), "utf8");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("Palette.css", () => {
  it("is a 560 px sheet 20% from the top, like the launcher", () => {
    expect(rule(".palette__sheet")).toContain("inline-size: min(560px");
    expect(rule(".palette__sheet")).toContain("inset-block-start: 20%");
  });
  it("styles focus and press, and hover only where hover exists", () => {
    expect(css).toContain(".palette__input:focus-visible");
    expect(css).toContain(".palette__opt:active");
    expect(css.slice(0, css.indexOf("@media (hover: hover) and (pointer: fine)"))).not.toContain(":hover");
  });
  it("marks the active option with a bar as well as a fill", () => {
    expect(rule('.palette__opt[data-active]')).toContain("border-inline-start");
  });
  it("makes matched letters bold and underlined", () => {
    expect(rule(".palette__opt mark")).toContain("font-weight");
    expect(rule(".palette__opt mark")).toContain("text-decoration");
  });
  it("uses tokens and logical properties only", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).not.toMatch(/\b(left|right|top|bottom):/);
  });
});
```

- [ ] **Step 2: Run to see them fail**

Run: `pnpm vitest run src/features/palette`
Expected: FAIL (modules not found).

- [ ] **Step 3: Implement**

`src/features/palette/Palette.tsx`:

```tsx
import { COMMANDS, shortcutLabel, type Command, type Os } from "../../lib/keymap";
import { filterCommands, highlight, type Hit } from "../launcher/filter";
import "./Palette.css";

export type PaletteView = { query: string; activeIndex: number };

const GROUPS: Command["group"][] = ["Sessions", "View", "Tools"];

function Option({ hit, index, active, os, onRun }: { hit: Hit<Command>; index: number; active: boolean; os: Os; onRun?: (id: string) => void }) {
  const c = hit.item;
  return (
    <li id={`palette-opt-${index}`} role="option" aria-selected={active} className="palette__opt" data-active={active || undefined} onClick={() => onRun?.(c.id)}>
      <span className="palette__label" dir="auto">
        {highlight(c.label, hit.ranges).map((s, i) =>
          s.hit ? (
            <mark key={i} dir="auto">
              {s.text}
            </mark>
          ) : (
            <span key={i}>{s.text}</span>
          ),
        )}
      </span>
      {c.binding && <kbd className="palette__kbd">{shortcutLabel(c.binding, os)}</kbd>}
    </li>
  );
}

/**
 * The command palette. Every action in the app is here, with its shortcut shown; shortcuts are rebindable in
 * Settings. Presentational: the parent owns the query and the active row.
 */
export function Palette({ view, os = "windows", onQuery, onRun }: { view: PaletteView; os?: Os; onQuery?: (q: string) => void; onRun?: (id: string) => void }) {
  const { query, activeIndex } = view;
  const hits = filterCommands(query, COMMANDS);
  const filtering = query.trim() !== "";
  let index = -1;

  return (
    <div className="palette">
      <div className="palette__backdrop" />
      <div className="palette__sheet" role="dialog" aria-modal="true" aria-label="Command palette">
        <input
          className="palette__input"
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-activedescendant={hits.length > 0 ? `palette-opt-${activeIndex}` : undefined}
          aria-label="Type a command"
          placeholder="Type a command"
          value={query}
          onChange={(e) => onQuery?.(e.target.value)}
        />
        {hits.length === 0 ? (
          <p className="palette__empty" role="status">
            No command matches {"\u201c"}
            {query}
            {"\u201d"}
          </p>
        ) : (
          <ul className="palette__list" id="palette-list" role="listbox" aria-label="Commands">
            {filtering
              ? hits.map((h) => <Option key={h.item.id} hit={h} index={++index} active={index === activeIndex} os={os} onRun={onRun} />)
              : GROUPS.map((g) => (
                  <li key={g} role="presentation" className="palette__group-item">
                    <span className="palette__group" role="presentation">
                      {g}
                    </span>
                    <ul role="group" aria-label={g} className="palette__sub">
                      {hits.filter((h) => h.item.group === g).map((h) => <Option key={h.item.id} hit={h} index={++index} active={index === activeIndex} os={os} onRun={onRun} />)}
                    </ul>
                  </li>
                ))}
          </ul>
        )}
        <footer className="palette__footer">
          <span>
            <kbd>{"\u2191"}</kbd> <kbd>{"\u2193"}</kbd> choose {"\u00b7"} <kbd>Enter</kbd> run {"\u00b7"} <kbd>Esc</kbd> close
          </span>
          <span>Rebind in Settings</span>
        </footer>
      </div>
    </div>
  );
}
```

`src/features/palette/Palette.css`:

```css
.palette {
  position: relative;
  block-size: 100%;
  min-block-size: 480px;
  overflow: clip;
  color: var(--text-1);
  font-size: var(--text-13);
  line-height: 20px;
}
.palette__backdrop {
  position: absolute;
  inset: 0;
  background: color-mix(in oklch, var(--bg-base) 55%, transparent);
}
.palette__sheet {
  position: absolute;
  inset-block-start: 20%;
  inset-inline: 0;
  margin-inline: auto;
  inline-size: min(560px, calc(100% - 32px));
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  max-block-size: 70%;
  border: 1px solid var(--hairline);
  border-radius: 10px;
  background: var(--bg-overlay);
  box-shadow: 0 12px 40px color-mix(in oklch, var(--text-1) 22%, transparent);
  overflow: clip;
}
.palette__input {
  padding: var(--space-3) var(--space-4);
  border: 0;
  border-block-end: 1px solid var(--hairline);
  background: transparent;
  color: var(--text-1);
  font: inherit;
  font-size: var(--text-15);
}
.palette__input:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}
.palette__list {
  margin: 0;
  padding: var(--space-1) 0;
  list-style: none;
  overflow-y: auto;
}
.palette__group-item {
  list-style: none;
}
.palette__sub {
  margin: 0;
  padding: 0;
  list-style: none;
}
.palette__group {
  display: block;
  padding-block: var(--space-2) var(--space-1);
  padding-inline: var(--space-4);
  color: var(--text-2);
  font-size: var(--text-11);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.palette__opt {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding-block: 6px;
  padding-inline: var(--space-4);
  border-inline-start: 3px solid transparent;
}
.palette__opt[data-active] {
  background: var(--selected);
  border-inline-start: 3px solid var(--accent);
}
.palette__opt:active {
  background: var(--selected);
}
.palette__label {
  min-inline-size: 0;
  overflow: clip;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.palette__opt mark {
  background: transparent;
  color: var(--text-1);
  font-weight: 700;
  text-decoration: underline;
  text-underline-offset: 2px;
}
.palette__kbd,
.palette__footer kbd {
  flex: none;
  padding-inline: 4px;
  border: 1px solid color-mix(in oklch, currentColor 40%, transparent);
  border-radius: 3px;
  color: var(--text-2);
  font-family: var(--font-mono);
  font-size: var(--text-11);
  line-height: 16px;
}
.palette__empty {
  margin: 0;
  padding: var(--space-4);
  color: var(--text-2);
}
.palette__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding-block: var(--space-2);
  padding-inline: var(--space-4);
  border-block-start: 1px solid var(--hairline);
  color: var(--text-2);
  font-size: var(--text-12);
}
@media (hover: hover) and (pointer: fine) {
  .palette__opt:hover {
    background: var(--hover);
  }
}
@media (forced-colors: active) {
  .palette__sheet {
    border-color: CanvasText;
  }
  .palette__opt[data-active] {
    outline: 2px solid Highlight;
  }
}
```

`src/features/palette/Palette.stories.tsx`:

```tsx
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ThemePair } from "../../design/ThemePair";
import { Palette } from "./Palette";

const meta: Meta = { title: "Command palette", parameters: { layout: "fullscreen" } };
export default meta;

const frame = (query: string, activeIndex = 0, os: "windows" | "mac" = "windows") => (
  <div style={{ blockSize: 520, position: "relative", background: "var(--bg-base)", border: "1px solid var(--hairline)", borderRadius: 8, overflow: "clip" }}>
    <Palette view={{ query, activeIndex }} os={os} />
  </div>
);

export const Empty: StoryObj = { render: () => <ThemePair label="Empty: every command, grouped, with its shortcut" stack>{frame("")}</ThemePair> };
export const Filtering: StoryObj = { render: () => <ThemePair label={'Filtering: "split"'} stack>{frame("split")}</ThemePair> };
export const ScatteredLetters: StoryObj = { render: () => <ThemePair label={'Scattered letters: "stse" finds Start a session'} stack>{frame("stse")}</ThemePair> };
export const NoMatch: StoryObj = { render: () => <ThemePair label="Nothing matches" stack>{frame("qqqq")}</ThemePair> };
export const MacShortcuts: StoryObj = { render: () => <ThemePair label="macOS shortcut labels" stack>{frame("", 0, "mac")}</ThemePair> };

/** Try it: type to filter. */
function Live() {
  const [q, setQ] = useState("");
  return (
    <div style={{ blockSize: 520, position: "relative", background: "var(--bg-base)", border: "1px solid var(--hairline)", borderRadius: 8, overflow: "clip" }}>
      <Palette view={{ query: q, activeIndex: 0 }} onQuery={setQ} />
    </div>
  );
}
export const Interactive: StoryObj = { render: () => <Live /> };
```

- [ ] **Step 4: Run and commit**

Run: `pnpm vitest run` (PASS) and `pnpm tsc --noEmit` (clean). 
```bash
git add -A src
git commit -m "feat(palette): command palette with grouped commands, per-OS shortcuts and fuzzy filtering

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 15: Review page for sub-review C, keyboard and accessibility checks

**Files:**
- Modify: `src/design/review/renderReview.tsx`, `scripts/build-review.mjs`
- Test: `src/design/review/renderReview.test.tsx`

**Interfaces:**
- Produces: `renderReview("2c")` containing the launcher and palette sections after the window.

- [ ] **Step 1: Extend the renderer test (failing)**

Append to `src/design/review/renderReview.test.tsx`:

```tsx
describe("renderReview 2c", () => {
  it("adds the launcher and the palette", () => {
    const html = renderReview("2c");
    expect(html).toContain('id="launcher"');
    expect(html).toContain('id="command-palette"');
  });
  it("renders every launcher and palette story", () => {
    const html = renderReview("2c");
    for (const name of ["Empty", "Filtering", "No match", "Remembered for this project", "After enter", "Scattered letters", "Mac shortcuts"]) expect(html, name).toContain(name);
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `pnpm vitest run src/design/review`
Expected: FAIL (`launcher` id missing).

- [ ] **Step 3: Renderer and build script**

In `renderReview.tsx` import `* as Launcher from "../../features/launcher/Launcher.stories";` and `* as Palette from "../../features/palette/Palette.stories";` and set

```tsx
"2c": [
  [Launcher, "One sentence of chips and a prompt. Tab moves between choices, typing in the folder chip filters, Enter starts and the tab appears at once. The remembered state shows a project's last choices pre-set."],
  [Palette, "Every action with its shortcut, per OS, grouped when empty and ranked when filtering."],
],
```

In `scripts/build-review.mjs` add `"src/features/launcher/Launcher.css"` and `"src/features/palette/Palette.css"` to `CSS`, and `META["2c"]`: lede "Sub-review C of 3 in batch 2: the launcher and the command palette. The approve card and the main window are signed off and sit below.", nav entries for `launcher` and `command-palette` first, and checks:
1. "The launcher reads as one sentence you could say aloud, and Tab visits each choice in that order."
2. "Typing in the folder chip filters by scattered letters and bolds what matched, without relying on colour."
3. "The remembered state says what it applied and how to change it."
4. "The palette shows every action with its shortcut for your OS, and an empty result says what happened."
5. "Both sheets sit 20% from the top at 560 px and look related."
6. "It still reads well at 150% browser zoom, and the focus ring is visible in both themes."

- [ ] **Step 4: Run, build, check**

Run: `pnpm vitest run` and `pnpm tsc --noEmit` (PASS, clean), then `pnpm review:build "$TEMP/review.html" 2c`.

In the Playwright browser, on `http://127.0.0.1:6011/review.html`:

1. Axe (same script as Task 11 step 6.4, `exclude` `.xterm`): nothing `serious` or `critical` in either scheme. Note: the launcher and palette are `role="dialog"` regions rendered inline for review, so axe may report `aria-dialog-name` satisfied but `landmark`-type moderates; ledger those.
2. Keyboard order in the empty launcher: focus the first chip (`page.focus` on the first `.launcher__chip`), then press Tab five times, reading `document.activeElement.className` and text after each:

```js
async (page) => {
  await page.locator("#launcher .launcher__chip").first().focus();
  const seen = [];
  for (let i = 0; i < 7; i++) {
    seen.push(await page.evaluate(() => `${document.activeElement.tagName}.${document.activeElement.className}:${document.activeElement.textContent.trim().slice(0, 20)}`));
    await page.keyboard.press("Tab");
  }
  return seen;
}
```

Expected order: agent chip, folder chip, mode chip, pace chip, worktree chip, prompt box, Start button.
3. Screenshots of the launcher (empty, filtering, remembered) and the palette (empty, filtering, no match) in both themes and at 150% zoom. Read them. Confirm both sheets align (same width, same top offset), the matched letters are bold and underlined, the active option has the bar, the footer hints do not wrap badly at 960 px wide.

- [ ] **Step 5: Commit and publish for sign-off**

```bash
git add -A src scripts
git commit -m "feat(ui): review page for sub-review C

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

Build the page to the scratchpad file with `pnpm review:build <path> 2c`, read its head and new sections' CSS, publish to the review artifact with the label "Sub-review C: launcher and palette", tell the user what to check and wait for sign-off.

---

## Close-out

### Task 16: Wrap-up of batch 2

**Files:**
- Modify: `docs/PLAN.md`, `docs/BRIEF.md` (Amendments), `.superpowers/sdd/<this plan>/progress.md` (ledger)

- [ ] **Step 1: Update the plan and the brief**

In `docs/PLAN.md` section 9, phase 0: mark batch 2 (deliverables 4–6) as signed off with its date, and keep batch 3 (deliverables 7–11) pending. In `docs/BRIEF.md` Amendments, add one dated line each for: the sidebar default of 288 px; only comfortable and expanded row densities; the agent mark and the status glyph position; blocked rows (needs you and error) standing out; the terminal theme setting; the approve-card keyboard rules as built; any `Ruling:` from the ledger that changed a number in the plan (the lane's target card height).

- [ ] **Step 2: Full verification**

Run `pnpm vitest run` and `pnpm tsc --noEmit` and read the output (verification-before-completion). Both must be green.

- [ ] **Step 3: Final review**

Per executing-plans: generate the review package for the whole branch (`review-package PLAN_FILE $(git merge-base main HEAD) HEAD`), dispatch one fresh reviewer on the most capable model with the package, this plan, `docs/PLAN.md` and the ledger's `Ruling:` lines, and the Review Focus section verbatim. Re-grade its findings, fix Critical and Important ones in one pass with a failing test first, ledger the minors as `Final: minor (deferred)`.

- [ ] **Step 4: Report and finish**

Final message lists every ledger `Ruling:` line under "Rulings I made" and every deferred minor under "Deferred minors". Delete this plan's workspace directory. Then `superpowers:finishing-a-development-branch`: tests green, offer the three options, recommend holding the PR until batch 3 is signed off. The user pushes: `! git -C C:/dev/Airport push`.
