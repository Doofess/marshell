# Phase 0 batch 2: approve card, main window, launcher and palette. Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Storybook mockups, built on real reusable components, of phase 0 deliverables 4–6 (approve card, main window composite with the 5-agent scenario and a real xterm terminal, launcher and command palette), reviewed and signed off in three sub-reviews: A (card), B (window), C (launcher and palette).

**Architecture:**
- Pure logic lives in small tested modules (headline and payload text, arming and key rules, lane layout, sidebar mode, filters, shortcut labels, terminal palettes). Components are presentational and take view models, as in batch 1.
- Stories render every state. The review page (`scripts/build-review.mjs`) server-renders them into one HTML file; for sub-review B the page also inlines xterm.js so the terminal is real in the published page, not a picture.
- The terminal has its own palette (docs/PLAN.md "Terminal theme"): follow-app by default, live switch, `minimumContrastRatio` on.

**Tech Stack:** React 19, TypeScript 7, Vite 8, Storybook 10.6 (addon-a11y, addon-themes), `@xterm/xterm` 6 with `@xterm/addon-fit`, culori 4, Vitest 5 (environment `node`; components are tested through `react-dom/server`), Playwright MCP for visual and axe checks.

**Spec:** `docs/PLAN.md`, sections "UX and UI design" (Phase 0 table rows 4–6, Layout, The approve card, Key flows, Terminal theme, Microcopy, UX risks, Keyboard routing in section 7). Batch 1 decisions are in the Amendments of `docs/BRIEF.md`. Batch 1 plan: `docs/superpowers/plans/2026-10-10-phase-0-batch-1-tokens-glyphs-rows.md`.

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

**Files:**
- Modify: `src/design/contrast.ts` (add `overlay`)
- Test: `src/design/contrast.test.ts`
- Modify (only if the test fails): `src/styles/tokens.css`

**Interfaces:**
- Produces: `overlay(fg: string, bg: string, alpha: number): string`, the colour of `fg` at `alpha` over `bg`, interpolated in oklch like CSS `color-mix(in oklch, fg alpha, bg)`, as a hex string.

- [ ] **Step 1: Write the failing test**

Append to `src/design/contrast.test.ts` (it already imports `contrast`, `minOnSurfaces`, `tokens`, `THEMES`; add `overlay` to the import from `./contrast`):

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

- [ ] **Step 2: Run it to see it fail**

Run: `pnpm vitest run src/design/contrast.test.ts`
Expected: FAIL with "overlay is not a function" (or an import error).

- [ ] **Step 3: Implement `overlay`**

In `src/design/contrast.ts` change the first import to `import { differenceCiede2000, formatHex, interpolate, parse, wcagContrast } from "culori";` and add:

```ts
/** `fg` at `alpha` over `bg`, mixed in oklch the way CSS color-mix does; returns a hex string. */
export function overlay(fg: string, bg: string, alpha: number): string {
  return formatHex(interpolate([color(bg), color(fg)], "oklch")(alpha));
}
```

- [ ] **Step 4: Run it**

Run: `pnpm vitest run src/design/contrast.test.ts`
Expected: PASS. If a `--text-3 … (light)` case fails (it measured 4.43 at 10% hover in batch 1), darken the light value of `--text-3` in `src/styles/tokens.css` by 1 lightness point (`oklch(53% …)` to `oklch(52% …)`), repeat until green, and add a ledger line `Ruling: light --text-3 darkened to <value> so hover and selected stay AA`.

- [ ] **Step 5: Whole suite and commit**

Run: `pnpm vitest run` and `pnpm tsc --noEmit` (both green), then:

```bash
git add src/design src/styles
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
