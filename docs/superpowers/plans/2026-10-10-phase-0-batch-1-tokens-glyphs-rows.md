# Phase 0 batch 1: tokens, status glyphs, sidebar rows. Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Storybook mockups of phase 0 deliverables 1–3 (tokens and type specimen, status glyph set and motion, sidebar row matrix), built on real components that phase 2 will reuse, for the user to sign off as batch 1.

**Architecture:**
- The CSS custom properties in `src/styles/*.css` are the single source of truth for colour, type, space and motion.
- A small parser reads those files (as `?raw` imports) so that unit tests and the colour stories check the real values, with no copy.
- Glyphs, the context ring and the session row are presentational React components driven by a plain `RowModel` view model.
- Stories render every state, in both themes side by side, using `color-scheme` per panel, because the tokens use `light-dark()`.

**Tech Stack:**
- React 19, TypeScript 7, Vite 8
- Storybook 10.6 (`@storybook/react-vite`), with addon-a11y and addon-themes
- culori 4 (contrast and ΔE2000)
- Vitest 5, environment `node`; components are tested through `react-dom/server`

**Spec:** `docs/PLAN.md`, section "UX and UI design". The parts that matter here are "Phase 0", "Sidebar rows", "Visual system", "Microcopy" and "UX risks". Also read the "Phase 0 decisions (2026-10-10)" in the Amendments of `docs/BRIEF.md`.

## Global Constraints

- **Styles:** follow the good-css skill. Concretely:
  - logical properties only (`inline`/`block`, never left/right/top/bottom);
  - colours in `oklch()` with `none` as the hue of greys;
  - tints via `color-mix(in oklch, …)`;
  - `:hover` only inside `@media (hover: hover) and (pointer: fine)`;
  - focus via `:focus-visible` + `outline`, never `outline: none`;
  - `overflow: clip`, not `hidden`;
  - transitions name their properties, never `all`, never `ease-in`.
- **No literal colours in components.** Every colour is a `var(--…)` from `src/styles/tokens.css` or `src/styles/accents.css`. Hex appears nowhere in `src/components` or `src/features`.
- **Default accent:** signal amber, `light-dark(oklch(58% 0.15 65), oklch(80% 0.16 72))`.
  - The accent is used only for attention (fills, badges, rings, focus) and never as body text.
  - It is never Claude orange.
- **Brand colours** appear only on the 3 px stripe and on dots. No status is ever shown in a brand colour.
- **Every status is readable without colour.** Each one has its own glyph shape plus a text label (screen-reader label and line-2 phrase).
- **Unknown values render as `–`** (U+2013), never a guess. An unknown context % hides the ring.
- **Rows never change height on hover.** Compact is 36 px, comfortable is 48 px. Expanded grows only when the user expands it.
- **Motion:**
  - Every animation uses the duration and easing tokens.
  - Every animation has a reduced-motion version, applied both under `@media (prefers-reduced-motion: reduce)` and under `:root[data-motion="reduce"]`; the latter is the Storybook toggle.
  - Looping animations pause while `:root[data-page-hidden]`.
- **Copy rules:**
  - Sentence case, subject first.
  - Use the agent's name, never "the AI".
  - Use concrete numbers.
  - No "!", "successfully" or "Oops".
- **Numbers** use `font-variant-numeric: tabular-nums`.
- **Contrast floors (WCAG 2.2):**
  - Text tokens are ≥ 4.5:1 on every surface (`--bg-base`, `--bg-raised`, `--bg-overlay`).
  - Accents, status colours and brand stripes are ≥ 3:1 on every surface.
  - `--accent-ink` on `--accent` is ≥ 4.5:1.
- **Distinctness (CIEDE2000):**
  - Every accent is ΔE ≥ 15 from `--error`, `--ok` and `--caution`.
  - The default amber is ΔE ≥ 18 from `--brand-claude`.
- **Dependency pins:**
  - `storybook`, `@storybook/react-vite`, `@storybook/addon-a11y` and `@storybook/addon-themes`: `^10.6.1`
  - `culori`: `^4.0.2`
  - `@types/culori`: `^4.0.1`
  - All of them are devDependencies.
- **Tests:** Vitest with `environment: "node"`. Components are rendered with `renderToStaticMarkup` from `react-dom/server`. No jsdom and no browser runner in this batch.
- **Commits:**
  - Messages follow the repo's style (`feat(ui): …`, `test(ui): …`, `chore(ui): …`).
  - Every commit message ends with a blank line, then `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
  - Stage files by explicit path only.

## Review Focus

1. **Names that don't fit:** CJK, emoji, very long single words, a 1-character name, and RTL text in name, project and branch. Expect the truncation order to hold, no overlap with the right cluster, and the full text in `title` and in the screen-reader label. These are pinned by fixtures and by the `labels` test in Task 4.
2. **Amber vs caution vs error vs Claude orange confused at a glance.** Expect the ΔE floors to hold in both themes. Pinned by the `distinctness` tests in Task 2.
3. **Windows High Contrast (`forced-colors: active`).** Expect the glyphs to stay visible and be drawn in system colours. Pinned by Task 3's `uses only currentColor or tokens` test and its forced-colors CSS block.
4. **Durations and counts at the extremes:** 0 s, 59 s, 1 h+, more than 1 day, 1.2M tokens, $1,234.56, and undefined. Expect compact, stable-width text, with `–` for unknown. Pinned by the `format` tests in Task 4.
5. **Reduced motion and a hidden window.** Expect no glyph animation without a static equivalent, and looping animations to stop when the page is hidden. Pinned by Task 3's `every motion has a reduced form` test and the `visibility` test.

---

### Task 1: Storybook with theme, accent and motion switches

**Files:**
- Modify: `package.json` (devDependencies, scripts)
- Create: `.storybook/main.ts`
- Create: `.storybook/preview.tsx`
- Create: `src/design/ThemePair.tsx`
- Create: `src/design/ThemePair.css`
- Create: `src/design/Welcome.stories.tsx`
- Modify: `.gitignore` (add `storybook-static/`)

**Interfaces:**
- Produces `ThemePair` (`src/design/ThemePair.tsx`):
  ```ts
  export function ThemePair(props: { children: React.ReactNode; label?: string }): JSX.Element
  ```
  It renders its children twice, side by side: a dark panel (`color-scheme: dark`) and a light panel (`color-scheme: light`).
- Produces the Storybook globals `theme` (`light` | `dark` | `system`), `accent` (8 names) and `motion` (`full` | `reduce`). They set `data-theme`, `data-accent` and `data-motion` on `<html>`.

- [ ] **Step 1: Install**

```bash
pnpm add -D storybook@^10.6.1 @storybook/react-vite@^10.6.1 @storybook/addon-a11y@^10.6.1 @storybook/addon-themes@^10.6.1 culori@^4.0.2 @types/culori@^4.0.1
```

Add these scripts to `package.json`:
```json
"storybook": "storybook dev -p 6006",
"build-storybook": "storybook build -o storybook-static"
```

Append `storybook-static/` to `.gitignore`.

- [ ] **Step 2: Write `.storybook/main.ts`**

```ts
import type { StorybookConfig } from "@storybook/react-vite";

const config: StorybookConfig = {
  stories: ["../src/**/*.stories.@(ts|tsx)"],
  addons: ["@storybook/addon-a11y", "@storybook/addon-themes"],
  framework: { name: "@storybook/react-vite", options: {} },
  // The app's vite.config pins port 1420 with strictPort for Tauri; Storybook serves on its own port.
  viteFinal: async (cfg) => ({ ...cfg, server: { ...cfg.server, port: undefined, strictPort: false } }),
};
export default config;
```

- [ ] **Step 3: Write `.storybook/preview.tsx`**

```tsx
import type { Decorator, Preview } from "@storybook/react-vite";
import { withThemeByDataAttribute } from "@storybook/addon-themes";
import "../src/styles/tokens.css";
import "../src/styles/accents.css";
import "../src/styles/base.css";
import "../src/styles/motion.css";

const ACCENTS = ["amber", "blue", "indigo", "violet", "magenta", "cyan", "teal", "slate"] as const;

const withAccentAndMotion: Decorator = (Story, ctx) => {
  const root = document.documentElement;
  root.dataset.accent = String(ctx.globals.accent ?? "amber");
  if (ctx.globals.motion === "reduce") root.dataset.motion = "reduce";
  else delete root.dataset.motion;
  return <Story />;
};

const preview: Preview = {
  globalTypes: {
    accent: {
      description: "Accent",
      toolbar: { title: "Accent", icon: "paintbrush", items: [...ACCENTS], dynamicTitle: true },
    },
    motion: {
      description: "Motion",
      toolbar: { title: "Motion", icon: "play", items: ["full", "reduce"], dynamicTitle: true },
    },
  },
  initialGlobals: { accent: "amber", motion: "full" },
  decorators: [
    withThemeByDataAttribute({
      themes: { dark: "dark", light: "light" },
      defaultTheme: "dark",
      attributeName: "data-theme",
    }),
    withAccentAndMotion,
  ],
  parameters: {
    layout: "padded",
    backgrounds: { disable: true },
    a11y: { test: "todo" },
  },
};
export default preview;
```

`src/styles/accents.css` and `src/styles/motion.css` are created in Tasks 2 and 3. For now, create both as empty files containing a single comment line, so the imports resolve:
- `/* Accents: filled in by Task 2. */`
- `/* Motion: filled in by Task 3. */`

- [ ] **Step 4: Write `ThemePair`**

`src/design/ThemePair.tsx`:
```tsx
import type { ReactNode } from "react";
import "./ThemePair.css";

/** Renders the same content in a dark and a light panel; the tokens use light-dark(), which follows color-scheme. */
export function ThemePair({ children, label }: { children: ReactNode; label?: string }) {
  return (
    <section className="theme-pair" aria-label={label}>
      <div className="theme-pair__panel" style={{ colorScheme: "dark" }} data-panel="dark">
        <p className="theme-pair__caption">Dark</p>
        {children}
      </div>
      <div className="theme-pair__panel" style={{ colorScheme: "light" }} data-panel="light">
        <p className="theme-pair__caption">Light</p>
        {children}
      </div>
    </section>
  );
}
```

`src/design/ThemePair.css`:
```css
.theme-pair {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 360px), 1fr));
  gap: var(--space-4);
}

.theme-pair__panel {
  padding: var(--space-4);
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
  background: var(--bg-base);
  color: var(--text-1);
}

.theme-pair__caption {
  margin: 0 0 var(--space-3);
  color: var(--text-3);
  font-size: var(--text-11);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}
```

- [ ] **Step 5: Write a smoke story**

`src/design/Welcome.stories.tsx`:
```tsx
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "./ThemePair";

const meta: Meta = { title: "Design/Welcome" };
export default meta;

export const Welcome: StoryObj = {
  render: () => (
    <ThemePair label="Marshell design system">
      <h1 style={{ margin: 0, fontSize: "var(--text-20)", fontWeight: 600 }}>Marshell design system</h1>
      <p style={{ color: "var(--text-2)" }}>
        Batch 1: tokens, status glyphs and sidebar rows. Use the toolbar to switch theme, accent and motion.
      </p>
    </ThemePair>
  ),
};
```

- [ ] **Step 6: Verify**

Run: `pnpm build && pnpm build-storybook && pnpm test`

Expected:
- `tsc` is clean.
- `storybook-static/index.html` exists.
- The 15 existing tests pass.

Then run `pnpm storybook`, open http://localhost:6006, and confirm:
- the Welcome story renders two panels, dark and light;
- the three toolbar switches exist.

- [ ] **Step 7: Commit**

```bash
git add package.json pnpm-lock.yaml .gitignore .storybook/main.ts .storybook/preview.tsx src/design/ThemePair.tsx src/design/ThemePair.css src/design/Welcome.stories.tsx src/styles/accents.css src/styles/motion.css
git commit -m "chore(ui): Storybook with theme, accent and motion switches"
```

---

### Task 2: Colour, type, space and motion tokens with an AA and distinctness matrix

**Files:**
- Modify: `src/styles/tokens.css`
- Modify: `src/styles/accents.css`
- Create: `src/design/cssTokens.ts`
- Create: `src/design/cssTokens.test.ts`
- Create: `src/design/contrast.ts`
- Create: `src/design/contrast.test.ts`
- Create: `src/design/Colours.stories.tsx`
- Create: `src/design/Type.stories.tsx`
- Create: `src/design/SpaceAndMotion.stories.tsx`
- Create: `src/design/specimen.css`

**Interfaces:**
- Produces `parseTokens` (`src/design/cssTokens.ts`):
  ```ts
  export type ThemeValue = { light: string; dark: string };
  export function parseTokens(css: string, selector: string): Record<string, ThemeValue>;
  ```
  It returns every `--name` declared in the first rule whose selector list contains `selector`. A `light-dark(a, b)` value becomes `{ light: a, dark: b }`; any other value becomes `{ light: v, dark: v }`. Declarations whose value contains `var(` are skipped.
- Produces `contrast` and `deltaE` (`src/design/contrast.ts`):
  ```ts
  export function contrast(a: string, b: string): number;
  export function deltaE(a: string, b: string): number;
  export const SURFACES: readonly ["--bg-base", "--bg-raised", "--bg-overlay"];
  export function minOnSurfaces(color: string, tokens: Record<string, ThemeValue>, theme: "light" | "dark"): number;
  ```
  `contrast` is the WCAG 2 ratio and `deltaE` is CIEDE2000. `minOnSurfaces` returns the lowest contrast of `color` against the three surfaces in that theme.
- Produces these CSS tokens, used by Tasks 3 and 4:
  - colour: `--accent`, `--accent-ink`, `--accent-tint`, `--accent-flash`
  - brand: `--brand-claude`, `--brand-codex`, `--brand-gemini`, `--brand-generic`
  - status: `--caution`, `--ok`, `--error`
  - motion: `--dur-flash`, `--dur-bounce`, `--dur-check`, `--dur-enter`, `--dur-exit`, `--dur-peek`, `--dur-palette`, `--dur-drawer`, `--dur-pulse`, `--ease-out`, `--ease-spring`
  - size: `--row-compact`, `--row-comfortable`, `--stripe`, `--glyph`, `--glyph-small`, `--sidebar`, `--sidebar-min`

- [ ] **Step 1: Write the failing parser test**

`src/design/cssTokens.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { parseTokens } from "./cssTokens";

const css = `
:root {
  --a: light-dark(oklch(50% 0.1 20), oklch(80% 0.1 20));
  --b: 13px;
  --c: color-mix(in oklch, var(--a) 10%, transparent);
}
:root,
[data-accent="amber"] {
  --accent: light-dark(oklch(58% 0.15 65), oklch(80% 0.16 72));
}
[data-accent="blue"] { --accent: light-dark(oklch(54% 0.19 258), oklch(70% 0.15 250)); }
`;

describe("parseTokens", () => {
  it("splits light-dark into both themes", () => {
    expect(parseTokens(css, ":root")["--a"]).toEqual({ light: "oklch(50% 0.1 20)", dark: "oklch(80% 0.1 20)" });
  });
  it("keeps plain values for both themes", () => {
    expect(parseTokens(css, ":root")["--b"]).toEqual({ light: "13px", dark: "13px" });
  });
  it("skips values that depend on var()", () => {
    expect(parseTokens(css, ":root")["--c"]).toBeUndefined();
  });
  it("finds a selector inside a selector list", () => {
    expect(parseTokens(css, '[data-accent="amber"]')["--accent"]?.dark).toBe("oklch(80% 0.16 72)");
  });
  it("finds a single-line rule", () => {
    expect(parseTokens(css, '[data-accent="blue"]')["--accent"]?.light).toBe("oklch(54% 0.19 258)");
  });
  it("returns an empty object for a missing selector", () => {
    expect(parseTokens(css, '[data-accent="nope"]')).toEqual({});
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `pnpm vitest run src/design/cssTokens.test.ts`
Expected: FAIL. The suite fails because `./cssTokens` cannot be resolved.

- [ ] **Step 3: Implement the parser**

`src/design/cssTokens.ts`:
```ts
export type ThemeValue = { light: string; dark: string };

/** Splits on commas that are not inside parentheses. */
function splitTopLevel(s: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < s.length; i++) {
    const ch = s[i];
    if (ch === "(") depth++;
    else if (ch === ")") depth--;
    else if (ch === "," && depth === 0) {
      parts.push(s.slice(start, i).trim());
      start = i + 1;
    }
  }
  parts.push(s.slice(start).trim());
  return parts;
}

/**
 * Reads the custom properties of the first rule whose selector list contains `selector`.
 * Only for Marshell's own token files: flat rules, no nesting, no comments inside values.
 */
export function parseTokens(css: string, selector: string): Record<string, ThemeValue> {
  const noComments = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const rule = /([^{}]+)\{([^{}]*)\}/g;
  for (const m of noComments.matchAll(rule)) {
    const selectors = splitTopLevel(m[1]!.trim()).map((s) => s.trim());
    if (!selectors.includes(selector)) continue;
    const out: Record<string, ThemeValue> = {};
    for (const decl of m[2]!.split(";")) {
      const i = decl.indexOf(":");
      if (i < 0) continue;
      const name = decl.slice(0, i).trim();
      const value = decl.slice(i + 1).trim();
      if (!name.startsWith("--") || value.includes("var(")) continue;
      const ld = /^light-dark\(([\s\S]*)\)$/.exec(value);
      if (ld) {
        const [light, dark] = splitTopLevel(ld[1]!);
        out[name] = { light: light!, dark: dark! };
      } else {
        out[name] = { light: value, dark: value };
      }
    }
    return out;
  }
  return {};
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `pnpm vitest run src/design/cssTokens.test.ts`
Expected: PASS (6 tests).

- [ ] **Step 5: Write the tokens**

Replace `src/styles/tokens.css` with this (the surfaces and text 1–2 are unchanged; `--text-3` light, `--caution`, the brand colours, sizes and motion are new or changed):
```css
:root {
  color-scheme: light dark;

  /* Surfaces: true black in dark, white and warm greys in light. */
  --bg-base: light-dark(oklch(100% 0 none), oklch(0% 0 none));
  --bg-raised: light-dark(oklch(98.2% 0.003 85), oklch(14.5% 0 none));
  --bg-overlay: light-dark(oklch(95.6% 0.004 75), oklch(19.1% 0 none));

  /* Text: every tier is at least 4.5:1 on every surface (src/design/contrast.test.ts). */
  --text-1: light-dark(oklch(21.6% 0.006 56), oklch(92.2% 0 none));
  --text-2: light-dark(oklch(44.4% 0.011 74), oklch(71.9% 0 none));
  --text-3: light-dark(oklch(53% 0.013 58), oklch(63.3% 0 none));

  --hairline: color-mix(in oklch, var(--text-1) 10%, transparent);
  --hover: color-mix(in oklch, var(--text-1) 4%, transparent);
  --selected: color-mix(in oklch, var(--text-1) 7%, transparent);

  /* Status. Caution is yellow (dark) and olive (light) so it never reads as the amber accent. */
  --error: light-dark(oklch(56% 0.22 27), oklch(67% 0.21 29));
  --ok: light-dark(oklch(55% 0.14 147), oklch(76.5% 0.19 147));
  --caution: light-dark(oklch(52% 0.11 100), oklch(90% 0.17 100));

  /* Agent brand colours: stripes and dots only, never status. */
  --brand-claude: light-dark(oklch(62% 0.14 42), oklch(68% 0.13 42));
  --brand-codex: light-dark(oklch(32% 0 none), oklch(86% 0 none));
  --brand-gemini: light-dark(oklch(52% 0.17 265), oklch(70% 0.14 265));
  --brand-generic: light-dark(oklch(52% 0.02 250), oklch(65% 0.02 250));

  --font-ui: "Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, "Inter Variable", sans-serif;
  --font-mono: "Cascadia Mono", "JetBrains Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace;

  /* Desktop chrome uses fixed sizes; 13 px is the base. */
  --text-11: 0.6875rem;
  --text-12: 0.75rem;
  --text-13: 0.8125rem;
  --text-15: 0.9375rem;
  --text-20: 1.25rem;
  --text-28: 1.75rem;
  --leading: 1.5;

  /* 4 px steps on an 8 px grid. */
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-6: 24px;
  --space-8: 32px;

  --radius: 6px;

  /* Layout sizes from docs/PLAN.md "Layout" and "Sidebar rows". */
  --sidebar: 264px;
  --sidebar-min: 200px;
  --row-compact: 36px;
  --row-comfortable: 48px;
  --stripe: 3px;
  --glyph: 16px;
  --glyph-small: 12px;

  /* Motion from docs/PLAN.md "Motion". Exits use ease-out too: good-css rules out ease-in. */
  --ease-out: cubic-bezier(0.2, 0.8, 0.2, 1);
  --ease-spring: linear(0, 0.55 18%, 1.12 42%, 0.98 66%, 1.01 82%, 1);
  --dur-palette: 100ms;
  --dur-peek: 120ms;
  --dur-exit: 140ms;
  --dur-enter: 180ms;
  --dur-check: 200ms;
  --dur-drawer: 200ms;
  --dur-bounce: 280ms;
  --dur-flash: 600ms;
  --dur-pulse: 1.6s;

  /* Kept for existing components. */
  --dur-fast: 120ms;
  --dur: 180ms;
  --dur-slow: 200ms;
}

[data-theme="light"] {
  color-scheme: light;
}
[data-theme="dark"] {
  color-scheme: dark;
}
```

Replace `src/styles/accents.css` with this:
```css
/*
 * Eight accents. Amber is the default. The accent marks attention only: fills, badges, rings, focus.
 * --accent-ink is the text colour on an accent fill (at least 4.5:1; src/design/contrast.test.ts).
 */
:root,
[data-accent="amber"] {
  --accent: light-dark(oklch(58% 0.15 65), oklch(80% 0.16 72));
  --accent-ink: light-dark(oklch(0% 0 none), oklch(0% 0 none));
}
[data-accent="blue"] {
  --accent: light-dark(oklch(54% 0.19 258), oklch(70% 0.15 250));
  --accent-ink: light-dark(oklch(100% 0 none), oklch(0% 0 none));
}
[data-accent="indigo"] {
  --accent: light-dark(oklch(50% 0.2 280), oklch(68% 0.16 280));
  --accent-ink: light-dark(oklch(100% 0 none), oklch(0% 0 none));
}
[data-accent="violet"] {
  --accent: light-dark(oklch(52% 0.21 305), oklch(70% 0.17 305));
  --accent-ink: light-dark(oklch(100% 0 none), oklch(0% 0 none));
}
[data-accent="magenta"] {
  --accent: light-dark(oklch(54% 0.22 350), oklch(70% 0.2 345));
  --accent-ink: light-dark(oklch(100% 0 none), oklch(0% 0 none));
}
[data-accent="cyan"] {
  --accent: light-dark(oklch(54% 0.1 215), oklch(78% 0.12 210));
  --accent-ink: light-dark(oklch(100% 0 none), oklch(0% 0 none));
}
[data-accent="teal"] {
  --accent: light-dark(oklch(52% 0.09 185), oklch(76% 0.12 180));
  --accent-ink: light-dark(oklch(100% 0 none), oklch(0% 0 none));
}
[data-accent="slate"] {
  --accent: light-dark(oklch(50% 0.05 255), oklch(74% 0.05 250));
  --accent-ink: light-dark(oklch(100% 0 none), oklch(0% 0 none));
}

/* Derived from whichever accent is active. */
:root {
  --accent-tint: color-mix(in oklch, var(--accent) 6%, transparent);
  --accent-flash: color-mix(in oklch, var(--accent) 12%, transparent);
}
```

These values were pre-checked with culori 4.0.2. Lowest results:

| Check | Lowest | Where |
|---|---|---|
| Accent on any surface | 3.92 | light amber |
| Ink on accent | 4.71 | |
| Text-3 | 4.66 | light, on the overlay |
| ΔE from status colours | 19 | teal vs ok |
| ΔE amber vs Claude | 18.4 | light |

- [ ] **Step 6: Write the failing contrast and distinctness test**

`src/design/contrast.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import tokensCss from "../styles/tokens.css?raw";
import accentsCss from "../styles/accents.css?raw";
import { parseTokens } from "./cssTokens";
import { contrast, deltaE, minOnSurfaces } from "./contrast";

const ACCENTS = ["amber", "blue", "indigo", "violet", "magenta", "cyan", "teal", "slate"] as const;
const THEMES = ["dark", "light"] as const;
const tokens = parseTokens(tokensCss, ":root");
const accent = (name: string) => parseTokens(accentsCss, `[data-accent="${name}"]`);

describe("text on surfaces", () => {
  for (const theme of THEMES)
    for (const t of ["--text-1", "--text-2", "--text-3"])
      it(`${t} is at least 4.5:1 on every surface (${theme})`, () => {
        expect(minOnSurfaces(tokens[t]![theme], tokens, theme)).toBeGreaterThanOrEqual(4.5);
      });
});

describe("status and brand colours on surfaces", () => {
  const names = ["--error", "--ok", "--caution", "--brand-claude", "--brand-codex", "--brand-gemini", "--brand-generic"];
  for (const theme of THEMES)
    for (const n of names)
      it(`${n} is at least 3:1 on every surface (${theme})`, () => {
        expect(minOnSurfaces(tokens[n]![theme], tokens, theme)).toBeGreaterThanOrEqual(3);
      });
});

describe("accents", () => {
  it("defines all eight", () => {
    for (const a of ACCENTS) expect(accent(a)["--accent"], a).toBeDefined();
  });
  for (const theme of THEMES)
    for (const a of ACCENTS) {
      it(`${a} is at least 3:1 on every surface (${theme})`, () => {
        expect(minOnSurfaces(accent(a)["--accent"]![theme], tokens, theme)).toBeGreaterThanOrEqual(3);
      });
      it(`${a} ink is at least 4.5:1 on the accent (${theme})`, () => {
        const v = accent(a);
        expect(contrast(v["--accent-ink"]![theme], v["--accent"]![theme])).toBeGreaterThanOrEqual(4.5);
      });
    }
});

describe("distinctness", () => {
  for (const theme of THEMES)
    for (const a of ACCENTS)
      for (const s of ["--error", "--ok", "--caution"])
        it(`${a} is ΔE ≥ 15 from ${s} (${theme})`, () => {
          expect(deltaE(accent(a)["--accent"]![theme], tokens[s]![theme])).toBeGreaterThanOrEqual(15);
        });
  for (const theme of THEMES)
    it(`default amber is ΔE ≥ 18 from Claude orange (${theme})`, () => {
      expect(deltaE(accent("amber")["--accent"]![theme], tokens["--brand-claude"]![theme])).toBeGreaterThanOrEqual(18);
    });
  it("the :root default is amber", () => {
    expect(parseTokens(accentsCss, ":root")["--accent"]).toEqual(accent("amber")["--accent"]);
  });
});
```

- [ ] **Step 7: Run it to verify it fails**

Run: `pnpm vitest run src/design/contrast.test.ts`
Expected: FAIL. The suite fails because `./contrast` cannot be resolved.

- [ ] **Step 8: Implement the contrast helpers**

`src/design/contrast.ts`:
```ts
import { differenceCiede2000, parse, wcagContrast } from "culori";
import type { ThemeValue } from "./cssTokens";

const ciede = differenceCiede2000();

function color(s: string) {
  const c = parse(s);
  if (!c) throw new Error(`Not a colour: ${s}`);
  return c;
}

/** WCAG 2 contrast ratio, 1 to 21. */
export function contrast(a: string, b: string): number {
  return wcagContrast(color(a), color(b));
}

/** CIEDE2000 colour difference; about 2 is barely visible, 15 or more is clearly different at a glance. */
export function deltaE(a: string, b: string): number {
  return ciede(color(a), color(b));
}

export const SURFACES = ["--bg-base", "--bg-raised", "--bg-overlay"] as const;

/** The lowest contrast of `c` against the three surfaces in one theme. */
export function minOnSurfaces(c: string, tokens: Record<string, ThemeValue>, theme: "light" | "dark"): number {
  return Math.min(...SURFACES.map((s) => contrast(c, tokens[s]![theme])));
}
```

If TypeScript reports a missing `?raw` module type, check that `"types": ["vite/client"]` is in `tsconfig.json`. It already is.

- [ ] **Step 9: Run it to verify it passes**

Run: `pnpm vitest run src/design`
Expected: all PASS, about 110 tests including the parser.

If a value fails, do not change a threshold. Report it as DONE_WITH_CONCERNS, quoting the failing pair. The values above were pre-checked, so a failure means a typo.

- [ ] **Step 10: Write the specimen stylesheet**

`src/design/specimen.css`:
```css
.specimen-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: var(--space-3);
}

.swatch {
  display: grid;
  gap: var(--space-1);
  font-size: var(--text-11);
  color: var(--text-2);
}

.swatch__chip {
  block-size: 40px;
  border-radius: var(--radius);
  border: 1px solid var(--hairline);
}

.swatch__name {
  color: var(--text-1);
  font-family: var(--font-mono);
}

.matrix {
  inline-size: 100%;
  border-collapse: collapse;
  font-size: var(--text-12);
  font-variant-numeric: tabular-nums;
}

.matrix :is(th, td) {
  padding-block: var(--space-1);
  padding-inline: var(--space-2);
  border-block-end: 1px solid var(--hairline);
  text-align: start;
}

.matrix th {
  color: var(--text-3);
  font-weight: 500;
}

.matrix__fail {
  color: var(--error);
  font-weight: 600;
}

.type-row {
  display: grid;
  grid-template-columns: 72px 1fr;
  align-items: baseline;
  gap: var(--space-3);
  padding-block: var(--space-2);
  border-block-end: 1px solid var(--hairline);
}

.type-row__meta {
  color: var(--text-3);
  font-size: var(--text-11);
  font-family: var(--font-mono);
}
```

- [ ] **Step 11: Write the colour stories**

`src/design/Colours.stories.tsx`:
```tsx
import type { Meta, StoryObj } from "@storybook/react-vite";
import tokensCss from "../styles/tokens.css?raw";
import accentsCss from "../styles/accents.css?raw";
import { parseTokens } from "./cssTokens";
import { contrast, deltaE, minOnSurfaces } from "./contrast";
import { ThemePair } from "./ThemePair";
import "./specimen.css";

const meta: Meta = { title: "Design/Colours" };
export default meta;

const ACCENTS = ["amber", "blue", "indigo", "violet", "magenta", "cyan", "teal", "slate"] as const;
const tokens = parseTokens(tokensCss, ":root");

function Swatch({ name }: { name: string }) {
  return (
    <div className="swatch">
      <div className="swatch__chip" style={{ background: `var(${name})` }} />
      <span className="swatch__name">{name}</span>
    </div>
  );
}

const groups: Array<[string, string[]]> = [
  ["Surfaces", ["--bg-base", "--bg-raised", "--bg-overlay", "--hover", "--selected", "--hairline"]],
  ["Text", ["--text-1", "--text-2", "--text-3"]],
  ["Status", ["--error", "--ok", "--caution"]],
  ["Accent (toolbar)", ["--accent", "--accent-tint", "--accent-flash"]],
  ["Agent brands (stripes and dots only)", ["--brand-claude", "--brand-codex", "--brand-gemini", "--brand-generic"]],
];

export const Palette: StoryObj = {
  render: () => (
    <ThemePair label="Palette">
      {groups.map(([title, names]) => (
        <div key={title}>
          <h3 style={{ fontSize: "var(--text-13)", fontWeight: 600 }}>{title}</h3>
          <div className="specimen-grid">
            {names.map((n) => (
              <Swatch key={n} name={n} />
            ))}
          </div>
        </div>
      ))}
    </ThemePair>
  ),
};

function Ratio({ value, floor }: { value: number; floor: number }) {
  return <span className={value < floor ? "matrix__fail" : undefined}>{value.toFixed(2)}</span>;
}

/** Computed live from the CSS files, so this table can never drift from the tokens. */
export const AccentMatrix: StoryObj = {
  render: () => (
    <table className="matrix">
      <thead>
        <tr>
          <th>Accent</th>
          <th>Theme</th>
          <th>Chip</th>
          <th>On surfaces (≥ 3)</th>
          <th>Ink on accent (≥ 4.5)</th>
          <th>ΔE error / ok / caution (≥ 15)</th>
          <th>ΔE Claude</th>
        </tr>
      </thead>
      <tbody>
        {ACCENTS.flatMap((a) => {
          const v = parseTokens(accentsCss, `[data-accent="${a}"]`);
          return (["dark", "light"] as const).map((theme) => {
            const acc = v["--accent"]![theme];
            return (
              <tr key={a + theme}>
                <td>{a}</td>
                <td>{theme}</td>
                <td>
                  <span
                    style={{
                      display: "inline-block",
                      inlineSize: 40,
                      blockSize: 16,
                      borderRadius: 3,
                      background: acc,
                      color: v["--accent-ink"]![theme],
                      fontSize: 10,
                      textAlign: "center",
                    }}
                  >
                    2
                  </span>
                </td>
                <td>
                  <Ratio value={minOnSurfaces(acc, tokens, theme)} floor={3} />
                </td>
                <td>
                  <Ratio value={contrast(v["--accent-ink"]![theme], acc)} floor={4.5} />
                </td>
                <td>
                  {(["--error", "--ok", "--caution"] as const).map((s, i) => (
                    <span key={s}>
                      {i > 0 && " / "}
                      <Ratio value={deltaE(acc, tokens[s]![theme])} floor={15} />
                    </span>
                  ))}
                </td>
                <td>{deltaE(acc, tokens["--brand-claude"]![theme]).toFixed(1)}</td>
              </tr>
            );
          });
        })}
      </tbody>
    </table>
  ),
};

export const TextMatrix: StoryObj = {
  render: () => (
    <table className="matrix">
      <thead>
        <tr>
          <th>Token</th>
          <th>Theme</th>
          <th>Lowest on base / raised / overlay</th>
          <th>Floor</th>
        </tr>
      </thead>
      <tbody>
        {(["--text-1", "--text-2", "--text-3", "--error", "--ok", "--caution"] as const).flatMap((t) =>
          (["dark", "light"] as const).map((theme) => {
            const floor = t.startsWith("--text") ? 4.5 : 3;
            return (
              <tr key={t + theme}>
                <td>{t}</td>
                <td>{theme}</td>
                <td>
                  <Ratio value={minOnSurfaces(tokens[t]![theme], tokens, theme)} floor={floor} />
                </td>
                <td>{floor}</td>
              </tr>
            );
          }),
        )}
      </tbody>
    </table>
  ),
};
```

- [ ] **Step 12: Write the type specimen**

`src/design/Type.stories.tsx`:
```tsx
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "./ThemePair";
import "./specimen.css";

const meta: Meta = { title: "Design/Type" };
export default meta;

const scale: Array<[string, number, string]> = [
  ["--text-28", 600, "3 sessions need you"],
  ["--text-20", 600, "No sessions yet. Start one with Ctrl+Shift+T."],
  ["--text-15", 500, "api-server has waited 1 min to run a command"],
  ["--text-13", 500, "Wants to run npm test"],
  ["--text-13", 400, "Marshell adds 9 entries to your Claude user settings so it can see status and approvals."],
  ["--text-12", 400, "Editing src/auth.ts · 2 subagents"],
  ["--text-11", 400, "84k / 200k · 312k in · 48k out · $1.20"],
];

export const Scale: StoryObj = {
  render: () => (
    <ThemePair label="Type scale">
      {scale.map(([size, weight, text]) => (
        <div className="type-row" key={size + weight + text}>
          <span className="type-row__meta">
            {size.replace("--text-", "")} / {weight}
          </span>
          <span style={{ fontSize: `var(${size})`, fontWeight: weight }}>{text}</span>
        </div>
      ))}
    </ThemePair>
  ),
};

export const TabularFigures: StoryObj = {
  render: () => (
    <ThemePair label="Tabular figures">
      {["1m 05s", "11m 40s", "1h 02m", "84k / 200k", "1.2M / 1M", "$1.20", "$1,234.56"].map((s) => (
        <div key={s} style={{ fontVariantNumeric: "tabular-nums", fontSize: "var(--text-11)", textAlign: "end", inlineSize: 120 }}>
          {s}
        </div>
      ))}
    </ThemePair>
  ),
};

export const Mono: StoryObj = {
  render: () => (
    <ThemePair label="Mono">
      <pre style={{ margin: 0, fontFamily: "var(--font-mono)", fontSize: "var(--text-12)", whiteSpace: "pre-wrap" }}>
        {"rm -rf ./dist && npm run build -- --mode production\nBash(npm test:*)"}
      </pre>
    </ThemePair>
  ),
};
```

- [ ] **Step 13: Write the space, radius and motion token story**

`src/design/SpaceAndMotion.stories.tsx`:
```tsx
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "./ThemePair";
import "./specimen.css";

const meta: Meta = { title: "Design/Space and motion" };
export default meta;

export const Space: StoryObj = {
  render: () => (
    <ThemePair label="Space">
      {["--space-1", "--space-2", "--space-3", "--space-4", "--space-6", "--space-8"].map((s) => (
        <div className="type-row" key={s}>
          <span className="type-row__meta">{s}</span>
          <span style={{ display: "block", blockSize: 8, inlineSize: `var(${s})`, background: "var(--text-2)" }} />
        </div>
      ))}
    </ThemePair>
  ),
};

const durations = [
  "--dur-palette",
  "--dur-peek",
  "--dur-exit",
  "--dur-enter",
  "--dur-check",
  "--dur-drawer",
  "--dur-bounce",
  "--dur-flash",
  "--dur-pulse",
];

export const MotionTokens: StoryObj = {
  render: () => (
    <ThemePair label="Motion tokens">
      {durations.map((d) => (
        <div className="type-row" key={d}>
          <span className="type-row__meta">{d}</span>
          <code style={{ fontSize: "var(--text-12)" }}>
            {getComputedStyle(document.documentElement).getPropertyValue(d)}
          </code>
        </div>
      ))}
    </ThemePair>
  ),
};
```

- [ ] **Step 14: Verify**

Run: `pnpm build && pnpm test && pnpm build-storybook`

Expected:
- `tsc` is clean.
- All tests pass.
- Storybook builds.

Open Design/Colours/Accent matrix and confirm no red cells. Switch the toolbar accent and confirm the Palette "Accent" chips change.

- [ ] **Step 15: Commit**

```bash
git add src/styles/tokens.css src/styles/accents.css src/design/cssTokens.ts src/design/cssTokens.test.ts src/design/contrast.ts src/design/contrast.test.ts src/design/specimen.css src/design/Colours.stories.tsx src/design/Type.stories.tsx src/design/SpaceAndMotion.stories.tsx
git commit -m "feat(ui): colour, type, space and motion tokens with an AA and distinctness matrix"
```

---

### Task 3: Status glyphs, context ring and motion

**Files:**
- Modify: `src/styles/motion.css`
- Create: `src/components/StatusGlyph/glyphs.ts`
- Create: `src/components/StatusGlyph/StatusGlyph.tsx`
- Create: `src/components/StatusGlyph/StatusGlyph.css`
- Create: `src/components/StatusGlyph/StatusGlyph.test.tsx`
- Create: `src/components/StatusGlyph/StatusGlyph.stories.tsx`
- Create: `src/components/ContextRing/ContextRing.tsx`
- Create: `src/components/ContextRing/ContextRing.css`
- Create: `src/components/ContextRing/ContextRing.test.tsx`
- Create: `src/design/visibility.ts`
- Create: `src/design/visibility.test.ts`

**Interfaces:**
- Consumes the tokens from Task 2: `--accent`, `--accent-ink`, `--ok`, `--error`, `--caution`, `--text-1..3`, `--bg-base`, `--dur-*`, `--ease-*`, `--glyph`, `--glyph-small`.
- Produces (`src/components/StatusGlyph/glyphs.ts`):
  ```ts
  export type GlyphKind =
    | "idle" | "working" | "needs-permission" | "needs-question" | "done-unseen"
    | "done-seen" | "error" | "stuck" | "ended" | "limited";
  export type AuxKind = "muted" | "elevated" | "caution";
  export type Motion = "pulse" | "bounce" | "draw" | "flash" | null;
  export type GlyphSpec = { label: string; motion: Motion; reduced: string | null };
  export const GLYPHS: Record<GlyphKind, GlyphSpec>;
  export const AUX_LABELS: Record<AuxKind, string>;
  ```
  `label` is the screen-reader word. `reduced` describes the reduced-motion form and is `null` when there is no motion.
- Produces (`StatusGlyph.tsx`):
  ```tsx
  export function StatusGlyph(props: { kind: GlyphKind; size?: 12 | 16; label?: string }): JSX.Element;
  export function AuxGlyph(props: { kind: AuxKind; label?: string }): JSX.Element; // always 12 px
  ```
  Both render `<svg role="img" aria-label={label ?? default}>`.
- Produces (`ContextRing.tsx`):
  ```tsx
  export function ContextRing(props: { pct: number | undefined }): JSX.Element | null;
  ```
  It returns `null` when `pct` is undefined or not finite. It clamps `pct` to 0..100. Its tone is neutral below 80, caution from 80 to 94, and error at 95 and above.
- Produces (`src/design/visibility.ts`):
  ```ts
  export function syncPageHidden(doc: Pick<Document, "visibilityState" | "documentElement" | "addEventListener" | "removeEventListener">): () => void;
  ```
  It sets or clears `data-page-hidden` on the root and returns an unsubscribe function.

- [ ] **Step 1: Write the failing glyph tests**

`src/components/StatusGlyph/StatusGlyph.test.tsx`:
```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AUX_LABELS, GLYPHS, type AuxKind, type GlyphKind } from "./glyphs";
import { AuxGlyph, StatusGlyph } from "./StatusGlyph";

const kinds = Object.keys(GLYPHS) as GlyphKind[];

describe("glyph registry", () => {
  it("covers the ten row states", () => {
    expect(kinds.sort()).toEqual(
      ["done-seen", "done-unseen", "ended", "error", "idle", "limited", "needs-permission", "needs-question", "stuck", "working"],
    );
  });
  it("every motion has a reduced form", () => {
    for (const k of kinds) {
      const g = GLYPHS[k];
      if (g.motion) expect(g.reduced, k).toBeTruthy();
    }
  });
  it("labels are sentence case words, not empty", () => {
    for (const k of kinds) expect(GLYPHS[k].label, k).toMatch(/^[A-Z][a-z ,:]+$/);
  });
});

describe("StatusGlyph", () => {
  for (const k of kinds) {
    it(`${k} renders an accessible svg with its label`, () => {
      const html = renderToStaticMarkup(<StatusGlyph kind={k} />);
      expect(html).toContain('role="img"');
      expect(html).toContain(`aria-label="${GLYPHS[k].label}"`);
      expect(html).toContain(`data-kind="${k}"`);
    });
    it(`${k} uses only currentColor or tokens`, () => {
      const html = renderToStaticMarkup(<StatusGlyph kind={k} size={12} />);
      expect(html).not.toMatch(/#[0-9a-f]{3,8}\b/i);
      expect(html).not.toMatch(/(fill|stroke)="(?!none|currentColor|var\()/);
    });
  }
  it("accepts a custom label", () => {
    expect(renderToStaticMarkup(<StatusGlyph kind="error" label="Stopped: rate limit" />)).toContain(
      'aria-label="Stopped: rate limit"',
    );
  });
  it("sizes to 12 or 16", () => {
    expect(renderToStaticMarkup(<StatusGlyph kind="idle" size={12} />)).toContain('width="12"');
    expect(renderToStaticMarkup(<StatusGlyph kind="idle" />)).toContain('width="16"');
  });
});

describe("AuxGlyph", () => {
  for (const k of Object.keys(AUX_LABELS) as AuxKind[])
    it(`${k} renders at 12 px with its label`, () => {
      const html = renderToStaticMarkup(<AuxGlyph kind={k} />);
      expect(html).toContain('width="12"');
      expect(html).toContain(`aria-label="${AUX_LABELS[k]}"`);
    });
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `pnpm vitest run src/components/StatusGlyph`
Expected: FAIL. The modules don't exist.

- [ ] **Step 3: Write the registry**

`src/components/StatusGlyph/glyphs.ts`:
```ts
export type GlyphKind =
  | "idle"
  | "working"
  | "needs-permission"
  | "needs-question"
  | "done-unseen"
  | "done-seen"
  | "error"
  | "stuck"
  | "ended"
  | "limited";
export type AuxKind = "muted" | "elevated" | "caution";
export type Motion = "pulse" | "bounce" | "draw" | "flash" | null;
export type GlyphSpec = { label: string; motion: Motion; reduced: string | null };

/** docs/PLAN.md "Sidebar rows" table: shape, motion and its reduced-motion form. */
export const GLYPHS: Record<GlyphKind, GlyphSpec> = {
  idle: { label: "Idle", motion: null, reduced: null },
  working: { label: "Working", motion: "pulse", reduced: "Static dot with a three-quarter arc" },
  "needs-permission": { label: "Needs you: permission", motion: "bounce", reduced: "No bounce" },
  "needs-question": { label: "Needs you: question", motion: "bounce", reduced: "No bounce" },
  "done-unseen": { label: "Done, unseen", motion: "draw", reduced: "Check shown complete" },
  "done-seen": { label: "Done", motion: null, reduced: null },
  error: { label: "Error", motion: "flash", reduced: "No flash" },
  stuck: { label: "Might be stuck", motion: null, reduced: null },
  ended: { label: "Ended", motion: null, reduced: null },
  limited: { label: "Status limited", motion: null, reduced: null },
};

export const AUX_LABELS: Record<AuxKind, string> = {
  muted: "Muted",
  elevated: "Administrator",
  caution: "Runs without asking",
};
```

- [ ] **Step 4: Write the components**

`src/components/StatusGlyph/StatusGlyph.tsx`:
```tsx
import type { ReactNode } from "react";
import { AUX_LABELS, GLYPHS, type AuxKind, type GlyphKind } from "./glyphs";
import "./StatusGlyph.css";

// All shapes are drawn on a 16-unit grid; colour comes from CSS (StatusGlyph.css) via currentColor and tokens.
const shapes: Record<GlyphKind, ReactNode> = {
  idle: <circle cx="8" cy="8" r="5.25" fill="none" stroke="currentColor" strokeWidth="1.5" />,
  working: (
    <>
      <circle className="g-working-arc" cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" strokeWidth="1.25" pathLength="4" strokeDasharray="3 1" />
      <circle className="g-working-dot" cx="8" cy="8" r="4" fill="currentColor" />
    </>
  ),
  "needs-permission": (
    <>
      <rect className="g-badge" x="1.5" y="1.5" width="13" height="13" rx="3.5" fill="var(--accent)" />
      <g fill="none" stroke="var(--accent-ink)" strokeWidth="1.5" strokeLinecap="round">
        <circle cx="5.75" cy="8" r="2" />
        <path d="M7.75 8h4.5M10.75 8v1.75" />
      </g>
    </>
  ),
  "needs-question": (
    <>
      <rect className="g-badge" x="1.5" y="1.5" width="13" height="13" rx="3.5" fill="var(--accent)" />
      <path
        d="M6.25 6.25a1.75 1.75 0 1 1 2.6 1.53c-.5.28-.85.62-.85 1.22v.25"
        fill="none"
        stroke="var(--accent-ink)"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
      <circle cx="8" cy="11.4" r="0.9" fill="var(--accent-ink)" />
    </>
  ),
  "done-unseen": (
    <path className="g-check" d="M3.75 8.25l2.75 2.75L12.25 5.25" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" pathLength="1" />
  ),
  "done-seen": (
    <path d="M3.75 8.25l2.75 2.75L12.25 5.25" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" />
  ),
  error: (
    <>
      <polygon className="g-octagon" points="5.2,1.5 10.8,1.5 14.5,5.2 14.5,10.8 10.8,14.5 5.2,14.5 1.5,10.8 1.5,5.2" fill="currentColor" />
      <path d="M6 6l4 4M10 6l-4 4" fill="none" stroke="var(--bg-base)" strokeWidth="1.5" strokeLinecap="round" />
    </>
  ),
  stuck: <circle cx="8" cy="8" r="5.25" fill="none" stroke="currentColor" strokeWidth="1.5" strokeDasharray="2.2 2" />,
  ended: <rect x="3.25" y="3.25" width="9.5" height="9.5" rx="1.5" fill="none" stroke="currentColor" strokeWidth="1.5" />,
  limited: (
    <circle cx="8" cy="8" r="5.25" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeDasharray="0.01 2.75" />
  ),
};

export function StatusGlyph({ kind, size = 16, label }: { kind: GlyphKind; size?: 12 | 16; label?: string }) {
  return (
    <svg
      className="status-glyph"
      data-kind={kind}
      role="img"
      aria-label={label ?? GLYPHS[kind].label}
      width={size}
      height={size}
      viewBox="0 0 16 16"
    >
      {shapes[kind]}
    </svg>
  );
}

const aux: Record<AuxKind, ReactNode> = {
  // Bell with a slash.
  muted: (
    <g fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4.5 11.5h7l-1-1.5V7a2.5 2.5 0 0 0-4-2" />
      <path d="M5.5 7v3l-1 1.5M7 13.25h2M2.5 2.5l11 11" />
    </g>
  ),
  // Shield.
  elevated: (
    <path d="M8 1.75l5 2v4c0 3.1-2.1 5.4-5 6.5-2.9-1.1-5-3.4-5-6.5v-4l5-2z" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
  ),
  // Warning triangle.
  caution: (
    <>
      <path d="M8 2l6.25 11H1.75L8 2z" fill="currentColor" />
      <path d="M8 6.25v3" stroke="var(--bg-base)" strokeWidth="1.5" strokeLinecap="round" />
      <circle cx="8" cy="11.1" r="0.85" fill="var(--bg-base)" />
    </>
  ),
};

export function AuxGlyph({ kind, label }: { kind: AuxKind; label?: string }) {
  return (
    <svg className="aux-glyph" data-kind={kind} role="img" aria-label={label ?? AUX_LABELS[kind]} width={12} height={12} viewBox="0 0 16 16">
      {aux[kind]}
    </svg>
  );
}
```

`src/components/StatusGlyph/StatusGlyph.css`:
```css
.status-glyph,
.aux-glyph {
  display: block;
  flex: none;
  overflow: visible;
}

/* Colour per state; shapes use currentColor. */
.status-glyph[data-kind="idle"],
.status-glyph[data-kind="stuck"],
.status-glyph[data-kind="ended"],
.status-glyph[data-kind="limited"] {
  color: var(--text-3);
}
.status-glyph[data-kind="working"] {
  color: var(--text-1);
}
.status-glyph[data-kind="done-unseen"] {
  color: var(--ok);
}
.status-glyph[data-kind="done-seen"] {
  color: color-mix(in oklch, var(--ok) 50%, transparent);
}
.status-glyph[data-kind="error"] {
  color: var(--error);
}
.aux-glyph[data-kind="caution"] {
  color: var(--caution);
}
.aux-glyph:is([data-kind="muted"], [data-kind="elevated"]) {
  color: var(--text-2);
}

/* The ¾ arc is the reduced-motion stand-in for the pulse. */
.g-working-arc {
  opacity: 0;
}

/* Windows High Contrast: draw with system colours so every glyph stays visible. */
@media (forced-colors: active) {
  .status-glyph,
  .aux-glyph {
    color: CanvasText;
    forced-color-adjust: none;
  }
  .status-glyph .g-badge {
    fill: Highlight;
  }
}
```

- [ ] **Step 5: Write the motion rules**

Replace `src/styles/motion.css` with:
```css
/*
 * Motion for glyphs and rows (docs/PLAN.md "Motion").
 * Full motion runs only when the OS allows it AND the Storybook/app switch is not "reduce".
 * Looping animation pauses while the page is hidden (src/design/visibility.ts sets data-page-hidden).
 */
@keyframes g-pulse {
  from { opacity: 0.45; }
  to { opacity: 1; }
}
@keyframes g-bounce {
  0%, 100% { transform: scale(1); }
  45% { transform: scale(1.2); }
}
@keyframes g-draw {
  from { stroke-dashoffset: 1; }
  to { stroke-dashoffset: 0; }
}
@keyframes g-flash {
  0%, 100% { opacity: 1; }
  40% { opacity: 0.35; }
}
@keyframes row-flash {
  from { background-color: var(--accent-flash); }
  to { background-color: transparent; }
}

@media (prefers-reduced-motion: no-preference) {
  :root:not([data-motion="reduce"]) .status-glyph[data-kind="working"] .g-working-dot {
    animation: g-pulse var(--dur-pulse) var(--ease-out) infinite alternate;
  }
  :root:not([data-motion="reduce"]) .status-glyph:is([data-kind="needs-permission"], [data-kind="needs-question"]) {
    transform-origin: center;
    animation: g-bounce var(--dur-bounce) var(--ease-spring) 1;
  }
  :root:not([data-motion="reduce"]) .status-glyph[data-kind="done-unseen"] .g-check {
    stroke-dasharray: 1;
    animation: g-draw var(--dur-check) var(--ease-out) 1 both;
  }
  :root:not([data-motion="reduce"]) .status-glyph[data-kind="error"] .g-octagon {
    animation: g-flash var(--dur-flash) var(--ease-out) 1;
  }
  :root:not([data-motion="reduce"]) .session-row[data-flash] {
    animation: row-flash var(--dur-flash) var(--ease-out) 1;
  }
}

/* Reduced: the working dot holds still and the ¾ arc shows instead. */
@media (prefers-reduced-motion: reduce) {
  .status-glyph[data-kind="working"] .g-working-arc {
    opacity: 1;
  }
}
:root[data-motion="reduce"] .status-glyph[data-kind="working"] .g-working-arc {
  opacity: 1;
}

/* Reduced row flash: an instant tint that fades without movement. */
@media (prefers-reduced-motion: reduce) {
  .session-row[data-flash] {
    background-color: var(--accent-tint);
    transition: background-color 300ms var(--ease-out);
  }
}
:root[data-motion="reduce"] .session-row[data-flash] {
  background-color: var(--accent-tint);
  transition: background-color 300ms var(--ease-out);
}

:root[data-page-hidden] * {
  animation-play-state: paused !important;
}
```

- [ ] **Step 6: Run the glyph tests**

Run: `pnpm vitest run src/components/StatusGlyph`
Expected: PASS.

- [ ] **Step 7: Write the failing ring and visibility tests**

`src/components/ContextRing/ContextRing.test.tsx`:
```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ContextRing, ringTone } from "./ContextRing";

describe("ContextRing", () => {
  it("renders nothing when unknown", () => {
    expect(renderToStaticMarkup(<ContextRing pct={undefined} />)).toBe("");
    expect(renderToStaticMarkup(<ContextRing pct={Number.NaN} />)).toBe("");
  });
  it("labels the percentage for screen readers", () => {
    expect(renderToStaticMarkup(<ContextRing pct={42} />)).toContain('aria-label="Context 42% used"');
  });
  it("clamps out-of-range values", () => {
    expect(renderToStaticMarkup(<ContextRing pct={140} />)).toContain('aria-label="Context 100% used"');
    expect(renderToStaticMarkup(<ContextRing pct={-5} />)).toContain('aria-label="Context 0% used"');
  });
  it("picks tones at 80 and 95", () => {
    expect([ringTone(79.9), ringTone(80), ringTone(94.9), ringTone(95)]).toEqual(["neutral", "caution", "caution", "error"]);
  });
});
```

`src/design/visibility.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { syncPageHidden } from "./visibility";

function fakeDoc(state: "visible" | "hidden") {
  const listeners: Array<() => void> = [];
  const attrs = new Map<string, string>();
  const doc = {
    visibilityState: state as DocumentVisibilityState,
    documentElement: {
      setAttribute: (k: string, v: string) => attrs.set(k, v),
      removeAttribute: (k: string) => attrs.delete(k),
    } as unknown as HTMLElement,
    addEventListener: (_: string, fn: () => void) => listeners.push(fn),
    removeEventListener: (_: string, fn: () => void) => listeners.splice(listeners.indexOf(fn), 1),
  };
  return { doc, attrs, listeners };
}

describe("syncPageHidden", () => {
  it("marks a hidden page at once", () => {
    const { doc, attrs } = fakeDoc("hidden");
    syncPageHidden(doc);
    expect(attrs.has("data-page-hidden")).toBe(true);
  });
  it("follows visibility changes and unsubscribes", () => {
    const { doc, attrs, listeners } = fakeDoc("visible");
    const stop = syncPageHidden(doc);
    expect(attrs.has("data-page-hidden")).toBe(false);
    doc.visibilityState = "hidden";
    listeners.forEach((l) => l());
    expect(attrs.has("data-page-hidden")).toBe(true);
    stop();
    expect(listeners).toHaveLength(0);
  });
});
```

- [ ] **Step 8: Run them to verify they fail**

Run: `pnpm vitest run src/components/ContextRing src/design/visibility.test.ts`
Expected: FAIL. The modules don't exist.

- [ ] **Step 9: Implement the ring and visibility**

`src/components/ContextRing/ContextRing.tsx`:
```tsx
import "./ContextRing.css";

export type RingTone = "neutral" | "caution" | "error";

export function ringTone(pct: number): RingTone {
  if (pct >= 95) return "error";
  if (pct >= 80) return "caution";
  return "neutral";
}

const R = 6;
const C = 2 * Math.PI * R;

/** Context-window use as a 16 px ring. Unknown hides it (docs/PLAN.md: unknown values are never guessed). */
export function ContextRing({ pct }: { pct: number | undefined }) {
  if (pct === undefined || !Number.isFinite(pct)) return null;
  const p = Math.min(100, Math.max(0, pct));
  const shown = Math.round(p);
  return (
    <svg className="context-ring" data-tone={ringTone(p)} role="img" aria-label={`Context ${shown}% used`} width={16} height={16} viewBox="0 0 16 16">
      <circle className="context-ring__track" cx="8" cy="8" r={R} fill="none" strokeWidth="2" />
      <circle
        className="context-ring__value"
        cx="8"
        cy="8"
        r={R}
        fill="none"
        strokeWidth="2"
        strokeDasharray={`${(C * p) / 100} ${C}`}
        transform="rotate(-90 8 8)"
      />
    </svg>
  );
}
```

`src/components/ContextRing/ContextRing.css`:
```css
.context-ring {
  display: block;
  flex: none;
}
.context-ring__track {
  stroke: var(--hairline);
}
.context-ring__value {
  stroke: var(--text-2);
}
.context-ring[data-tone="caution"] .context-ring__value {
  stroke: var(--caution);
}
.context-ring[data-tone="error"] .context-ring__value {
  stroke: var(--error);
}
@media (forced-colors: active) {
  .context-ring__value {
    stroke: CanvasText;
  }
}
```

`src/design/visibility.ts`:
```ts
type Doc = Pick<Document, "visibilityState" | "documentElement" | "addEventListener" | "removeEventListener">;

/** Mirrors page visibility onto `data-page-hidden` so CSS can pause looping animation. */
export function syncPageHidden(doc: Doc): () => void {
  const apply = () => {
    if (doc.visibilityState === "hidden") doc.documentElement.setAttribute("data-page-hidden", "");
    else doc.documentElement.removeAttribute("data-page-hidden");
  };
  apply();
  doc.addEventListener("visibilitychange", apply);
  return () => doc.removeEventListener("visibilitychange", apply);
}
```

Call it once from `src/main.tsx`, right before `createRoot(...)`:
```ts
import { syncPageHidden } from "./design/visibility";
syncPageHidden(document);
```

Also call it at the top of `.storybook/preview.tsx`, after the imports:
```ts
import { syncPageHidden } from "../src/design/visibility";
syncPageHidden(document);
```

- [ ] **Step 10: Run them to verify they pass**

Run: `pnpm vitest run src/components src/design`
Expected: PASS.

- [ ] **Step 11: Write the glyph stories**

`src/components/StatusGlyph/StatusGlyph.stories.tsx`:
```tsx
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ContextRing } from "../ContextRing/ContextRing";
import { ThemePair } from "../../design/ThemePair";
import { AUX_LABELS, GLYPHS, type AuxKind, type GlyphKind } from "./glyphs";
import { AuxGlyph, StatusGlyph } from "./StatusGlyph";
import "../../design/specimen.css";

const meta: Meta = { title: "Components/Status glyphs" };
export default meta;

const kinds = Object.keys(GLYPHS) as GlyphKind[];

export const Set: StoryObj = {
  render: () => (
    <ThemePair label="Status glyph set">
      <table className="matrix">
        <thead>
          <tr>
            <th>State</th>
            <th>16 px</th>
            <th>12 px</th>
            <th>Motion</th>
            <th>Reduced motion</th>
          </tr>
        </thead>
        <tbody>
          {kinds.map((k) => (
            <tr key={k}>
              <td>{GLYPHS[k].label}</td>
              <td>
                <StatusGlyph kind={k} />
              </td>
              <td>
                <StatusGlyph kind={k} size={12} />
              </td>
              <td>{GLYPHS[k].motion ?? "–"}</td>
              <td>{GLYPHS[k].reduced ?? "–"}</td>
            </tr>
          ))}
          {(Object.keys(AUX_LABELS) as AuxKind[]).map((k) => (
            <tr key={k}>
              <td>{AUX_LABELS[k]}</td>
              <td colSpan={2}>
                <AuxGlyph kind={k} />
              </td>
              <td>–</td>
              <td>–</td>
            </tr>
          ))}
        </tbody>
      </table>
    </ThemePair>
  ),
};

/** Remounts the one-shot animations so they can be watched again; set the toolbar Motion to "reduce" to compare. */
export const MotionReplay: StoryObj = {
  render: function Replay() {
    const [n, setN] = useState(0);
    return (
      <ThemePair label="Motion">
        <button type="button" onClick={() => setN(n + 1)}>
          Replay
        </button>
        <div key={n} style={{ display: "flex", gap: "var(--space-6)", marginBlockStart: "var(--space-3)", alignItems: "center" }}>
          {(["working", "needs-permission", "needs-question", "done-unseen", "error"] as GlyphKind[]).map((k) => (
            <figure key={k} style={{ margin: 0, display: "grid", justifyItems: "center", gap: "var(--space-1)" }}>
              <StatusGlyph kind={k} />
              <figcaption style={{ fontSize: "var(--text-11)", color: "var(--text-3)" }}>{GLYPHS[k].label}</figcaption>
            </figure>
          ))}
        </div>
      </ThemePair>
    );
  },
};

export const ContextRings: StoryObj = {
  render: () => (
    <ThemePair label="Context ring">
      <div style={{ display: "flex", gap: "var(--space-4)", alignItems: "center" }}>
        {[0, 12, 42, 79, 80, 94, 95, 100].map((p) => (
          <figure key={p} style={{ margin: 0, display: "grid", justifyItems: "center", gap: "var(--space-1)" }}>
            <ContextRing pct={p} />
            <figcaption style={{ fontSize: "var(--text-11)", color: "var(--text-3)", fontVariantNumeric: "tabular-nums" }}>{p}%</figcaption>
          </figure>
        ))}
        <figure style={{ margin: 0, display: "grid", justifyItems: "center", gap: "var(--space-1)" }}>
          <span style={{ blockSize: 16, color: "var(--text-3)" }}>–</span>
          <figcaption style={{ fontSize: "var(--text-11)", color: "var(--text-3)" }}>unknown</figcaption>
        </figure>
      </div>
    </ThemePair>
  ),
};
```

- [ ] **Step 12: Verify**

Run: `pnpm build && pnpm test && pnpm build-storybook`

Expected: all green.

Then, in Storybook:
- **Status glyphs / Set:** check that every shape is distinct in greyscale. A quick way is to set the OS to greyscale, or to look at the 12 px column.
- **Motion replay:** check that motion runs with Motion = full and holds still with Motion = reduce. Working should show the ¾ arc when reduced.

- [ ] **Step 13: Commit**

```bash
git add src/styles/motion.css src/components/StatusGlyph src/components/ContextRing src/design/visibility.ts src/design/visibility.test.ts src/main.tsx .storybook/preview.tsx
git commit -m "feat(ui): status glyphs, context ring and motion with reduced-motion forms"
```

---

### Task 4: The sidebar session row in every state and density

**Files:**
- Create: `src/features/sidebar/types.ts`
- Create: `src/features/sidebar/format.ts`
- Create: `src/features/sidebar/format.test.ts`
- Create: `src/features/sidebar/truncate.ts`
- Create: `src/features/sidebar/truncate.test.ts`
- Create: `src/features/sidebar/labels.ts`
- Create: `src/features/sidebar/labels.test.ts`
- Create: `src/features/sidebar/SessionRow.tsx`
- Create: `src/features/sidebar/SessionRow.css`
- Create: `src/features/sidebar/SessionRow.test.tsx`
- Create: `src/features/sidebar/fixtures.ts`
- Create: `src/features/sidebar/SessionRow.stories.tsx`

**Interfaces:**
- Consumes:
  - `StatusGlyph`, `AuxGlyph`, `GLYPHS`, `GlyphKind` (Task 3)
  - `ContextRing` (Task 3)
  - the tokens (Task 2)
- Produces (`types.ts`):
  ```ts
  export type Density = "compact" | "comfortable" | "expanded";
  export type AgentId = "claude" | "codex" | "gemini" | "generic";
  export type Mode = "manual" | "plan" | "auto-edit" | "full-auto" | "bypass";
  export type Usage = { inContext?: number; window?: number; tokensIn?: number; tokensOut?: number; costUsd?: number };
  export type RowModel = {
    id: string;
    name: string;
    project: string;
    branch?: string;
    agent: AgentId;
    status: GlyphKind;
    phrase: string;          // line 2, e.g. "Wants to run `npm test`"
    waitingMs?: number;      // needs-you only: how long it has waited
    ageMs?: number;          // done/ended: time since
    contextPct?: number;     // undefined hides the ring
    muted?: boolean;
    elevated?: boolean;
    mode?: Mode;
    model?: string;
    effort?: string;
    subagents?: number;
    recap?: { text: string; ageMs: number };
    usage?: Usage;
    flash?: boolean;         // one-shot attention flash on the row
  };
  ```
  This is a phase 0 view model. Phase 3 maps the core's generated session types onto it.
- Produces (`format.ts`):
  - `formatDuration(ms?: number): string`: `–`, `0s`…`59s`, `1m`…`59m`, `1h 05m`, `2d 3h`
  - `formatTokens(n?: number): string`: `–`, `312`, `84k`, `1.2M`
  - `formatCost(usd?: number): string`: `–`, `$0.04`, `$1.20`, `$1,234.56`
  - `formatUsage(u?: Usage): string`: `84k / 200k · 312k in · 48k out · $1.20`, with `–` per unknown part
  - `modeCaution(mode?: Mode): string | null`: `"Full auto"` | `"Bypass permissions"` | `"Auto-accept edits"` | `null`
- Produces (`truncate.ts`):
  - `splitForMiddle(s: string): { head: string; tail: string }`. The tail is the last segment after the final `-`, `/` or `_`, at most 8 characters; otherwise the last 6 characters. The head is the rest, and `head + tail === s` always.
- Produces (`labels.ts`):
  - `rowLabel(row: RowModel): string`: the screen-reader label.
  - `rowTitle(row: RowModel): string`: the tooltip text, with the full untruncated name, project, branch and phrase.
- Produces (`SessionRow.tsx`):
  ```tsx
  export function SessionRow(props: { row: RowModel; density: Density; selected?: boolean }): JSX.Element;
  ```

- [ ] **Step 1: Write the failing formatter tests**

`src/features/sidebar/format.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { formatCost, formatDuration, formatTokens, formatUsage, modeCaution } from "./format";

describe("formatDuration", () => {
  it.each([
    [undefined, "–"],
    [Number.NaN, "–"],
    [-1, "–"],
    [0, "0s"],
    [59_400, "59s"],
    [60_000, "1m"],
    [59 * 60_000 + 59_000, "59m"],
    [3_600_000, "1h 00m"],
    [3_900_000, "1h 05m"],
    [26 * 3_600_000 + 20 * 60_000, "1d 2h"],
  ])("%s ms → %s", (ms, out) => expect(formatDuration(ms)).toBe(out));
});

describe("formatTokens", () => {
  it.each([
    [undefined, "–"],
    [0, "0"],
    [312, "312"],
    [999, "999"],
    [1_000, "1k"],
    [84_400, "84k"],
    [999_499, "999k"],
    [1_200_000, "1.2M"],
    [12_000_000, "12M"],
  ])("%s → %s", (n, out) => expect(formatTokens(n)).toBe(out));
});

describe("formatCost", () => {
  it.each([
    [undefined, "–"],
    [0.04, "$0.04"],
    [1.2, "$1.20"],
    [1234.56, "$1,234.56"],
  ])("%s → %s", (n, out) => expect(formatCost(n)).toBe(out));
});

describe("formatUsage", () => {
  it("joins all parts", () => {
    expect(formatUsage({ inContext: 84_000, window: 200_000, tokensIn: 312_000, tokensOut: 48_000, costUsd: 1.2 })).toBe(
      "84k / 200k · 312k in · 48k out · $1.20",
    );
  });
  it("marks unknown parts with a dash", () => {
    expect(formatUsage({ inContext: 84_000, tokensIn: 312_000 })).toBe("84k / – · 312k in · – out · –");
  });
  it("is a single dash when nothing is known", () => {
    expect(formatUsage(undefined)).toBe("–");
  });
});

describe("modeCaution", () => {
  it("names only the modes that run without asking", () => {
    expect(["manual", "plan", "auto-edit", "full-auto", "bypass", undefined].map((m) => modeCaution(m as never))).toEqual([
      null,
      null,
      "Auto-accept edits",
      "Full auto",
      "Bypass permissions",
      null,
    ]);
  });
});
```

`src/features/sidebar/truncate.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { splitForMiddle } from "./truncate";

describe("splitForMiddle", () => {
  it.each([
    ["feat/auth-flow", "feat/auth", "-flow"],
    ["main", "", "main"],
    ["release/2026-10", "release/2026", "-10"],
    ["fix_the_thing", "fix_the", "_thing"],
    ["feature/very-long-final-segment-name", "feature/very-long-final-segment", "-name"],
    ["abcdefghijklmnop", "abcdefghij", "klmnop"],
    ["fix-🚀🚀🚀", "", "fix-🚀🚀🚀"],
  ])("%s", (s, head, tail) => {
    const r = splitForMiddle(s);
    expect(r.head + r.tail).toBe(s);
    expect(r.head).toBe(head);
    expect(r.tail).toBe(tail);
  });
  it("keeps a long final segment to its last 6 characters", () => {
    expect(splitForMiddle("topic/averyveryverylongsegment")).toEqual({ head: "topic/averyveryverylongs", tail: "egment" });
  });
  it("handles emoji without splitting a surrogate pair", () => {
    const r = splitForMiddle("fix-🚀🚀🚀🚀🚀🚀🚀🚀🚀");
    expect(r.head + r.tail).toBe("fix-🚀🚀🚀🚀🚀🚀🚀🚀🚀");
    expect([...r.tail].length).toBeLessThanOrEqual(8);
    expect(r.tail.charCodeAt(0) >= 0xdc00 && r.tail.charCodeAt(0) <= 0xdfff).toBe(false);
  });
  it("returns the whole string as tail when it is short", () => {
    expect(splitForMiddle("dev")).toEqual({ head: "", tail: "dev" });
  });
});
```

`src/features/sidebar/labels.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { rowLabel, rowTitle } from "./labels";
import type { RowModel } from "./types";

const base: RowModel = {
  id: "1",
  name: "api-server",
  project: "my-app",
  branch: "feat/auth-flow",
  agent: "claude",
  status: "needs-permission",
  phrase: "Wants to run `npm test`",
  waitingMs: 65_000,
  contextPct: 42,
};

describe("rowLabel", () => {
  it("reads name, state, phrase, place and wait", () => {
    expect(rowLabel(base)).toBe(
      "api-server, Needs you: permission. Wants to run npm test. my-app, branch feat/auth-flow. Waiting 1m. Context 42% used.",
    );
  });
  it("mentions muted, elevated and the caution mode", () => {
    expect(rowLabel({ ...base, muted: true, elevated: true, mode: "full-auto" })).toContain("Administrator. Full auto. Muted.");
  });
  it("leaves out what is unknown", () => {
    const l = rowLabel({ ...base, branch: undefined, waitingMs: undefined, contextPct: undefined });
    expect(l).not.toContain("branch");
    expect(l).not.toContain("Context");
    expect(l).not.toContain("Waiting");
  });
  it("keeps names that are not Latin", () => {
    expect(rowLabel({ ...base, name: "認証サーバー 🚀" })).toContain("認証サーバー 🚀,");
  });
});

describe("rowTitle", () => {
  it("has the untruncated text on separate lines", () => {
    expect(rowTitle(base)).toBe("api-server\nmy-app · feat/auth-flow\nWants to run `npm test`");
  });
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `pnpm vitest run src/features/sidebar`
Expected: FAIL. The modules don't exist.

- [ ] **Step 3: Implement types, formatters, truncation and labels**

`src/features/sidebar/types.ts`: use exactly the types given in the Interfaces block above. Import `GlyphKind` with:
```ts
import type { GlyphKind } from "../../components/StatusGlyph/glyphs";
```

`src/features/sidebar/format.ts`:
```ts
import type { Mode, Usage } from "./types";

const DASH = "–";

export function formatDuration(ms?: number): string {
  if (ms === undefined || !Number.isFinite(ms) || ms < 0) return DASH;
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ${String(m % 60).padStart(2, "0")}m`;
  return `${Math.floor(h / 24)}d ${h % 24}h`;
}

export function formatTokens(n?: number): string {
  if (n === undefined || !Number.isFinite(n) || n < 0) return DASH;
  if (n < 1000) return String(Math.round(n));
  if (n < 999_500) return `${Math.round(n / 1000)}k`;
  const m = n / 1_000_000;
  return `${m < 10 ? m.toFixed(1).replace(/\.0$/, "") : Math.round(m)}M`;
}

const usd = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", minimumFractionDigits: 2 });

export function formatCost(n?: number): string {
  if (n === undefined || !Number.isFinite(n) || n < 0) return DASH;
  return usd.format(n);
}

export function formatUsage(u?: Usage): string {
  if (!u || Object.values(u).every((v) => v === undefined)) return DASH;
  return `${formatTokens(u.inContext)} / ${formatTokens(u.window)} · ${formatTokens(u.tokensIn)} in · ${formatTokens(u.tokensOut)} out · ${formatCost(u.costUsd)}`;
}

/** Modes that run without asking always show a caution (docs/PLAN.md "Sidebar rows"). */
export function modeCaution(mode?: Mode): string | null {
  switch (mode) {
    case "auto-edit":
      return "Auto-accept edits";
    case "full-auto":
      return "Full auto";
    case "bypass":
      return "Bypass permissions";
    default:
      return null;
  }
}
```

`src/features/sidebar/truncate.ts`:
```ts
/**
 * Splits a branch name for middle truncation: the head may be cut with an ellipsis, the tail always shows.
 * Tail = the last segment after the final "-", "/" or "_" (with its separator) if that is at most 8 characters,
 * else the last 6 characters. Works on code points so emoji are never split.
 */
export function splitForMiddle(s: string): { head: string; tail: string } {
  const cps = [...s];
  if (cps.length <= 8) return { head: "", tail: s };
  let sep = -1;
  for (let i = cps.length - 1; i >= 0; i--) {
    if (cps[i] === "-" || cps[i] === "/" || cps[i] === "_") {
      sep = i;
      break;
    }
  }
  const cut = sep > 0 && cps.length - sep <= 8 ? sep : cps.length - 6;
  return { head: cps.slice(0, cut).join(""), tail: cps.slice(cut).join("") };
}
```

`src/features/sidebar/labels.ts`:
```ts
import { GLYPHS } from "../../components/StatusGlyph/glyphs";
import { formatDuration, modeCaution } from "./format";
import type { RowModel } from "./types";

/** Strips Markdown code ticks so screen readers don't read "backtick". */
const plain = (s: string) => s.replace(/`/g, "");

export function rowLabel(r: RowModel): string {
  const parts = [`${r.name}, ${GLYPHS[r.status].label}.`, `${plain(r.phrase)}.`];
  parts.push(r.branch ? `${r.project}, branch ${r.branch}.` : `${r.project}.`);
  if (r.elevated) parts.push("Administrator.");
  const caution = modeCaution(r.mode);
  if (caution) parts.push(`${caution}.`);
  if (r.muted) parts.push("Muted.");
  if (r.waitingMs !== undefined) parts.push(`Waiting ${formatDuration(r.waitingMs)}.`);
  if (r.contextPct !== undefined && Number.isFinite(r.contextPct))
    parts.push(`Context ${Math.round(Math.min(100, Math.max(0, r.contextPct)))}% used.`);
  return parts.join(" ");
}

export function rowTitle(r: RowModel): string {
  return [r.name, r.branch ? `${r.project} · ${r.branch}` : r.project, r.phrase].join("\n");
}
```

- [ ] **Step 4: Run them to verify they pass**

Run: `pnpm vitest run src/features/sidebar`
Expected: PASS.

- [ ] **Step 5: Write the failing row render test**

`src/features/sidebar/SessionRow.test.tsx`:
```tsx
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "./fixtures";
import { SessionRow } from "./SessionRow";

describe("SessionRow", () => {
  it("has one fixture per glyph state plus the modifiers", () => {
    const kinds = new Set(FIXTURES.map((f) => f.status));
    expect(kinds.size).toBe(10);
    expect(FIXTURES.some((f) => f.muted)).toBe(true);
    expect(FIXTURES.some((f) => f.elevated)).toBe(true);
    expect(FIXTURES.some((f) => f.mode === "bypass")).toBe(true);
    expect(FIXTURES.some((f) => f.contextPct === undefined)).toBe(true);
  });
  for (const density of ["compact", "comfortable", "expanded"] as const)
    it(`renders every fixture in ${density}`, () => {
      for (const row of FIXTURES) {
        const html = renderToStaticMarkup(<SessionRow row={row} density={density} />);
        expect(html, row.id).toContain(`data-density="${density}"`);
        expect(html, row.id).toContain(`data-agent="${row.agent}"`);
        expect(html, row.id).toContain("aria-label=");
      }
    });
  it("shows line 2 only from comfortable up", () => {
    const row = FIXTURES[0]!;
    expect(renderToStaticMarkup(<SessionRow row={row} density="compact" />)).not.toContain("session-row__phrase");
    expect(renderToStaticMarkup(<SessionRow row={row} density="comfortable" />)).toContain("session-row__phrase");
  });
  it("shows usage only when expanded", () => {
    const row = FIXTURES.find((f) => f.usage)!;
    expect(renderToStaticMarkup(<SessionRow row={row} density="comfortable" />)).not.toContain(" in · ");
    expect(renderToStaticMarkup(<SessionRow row={row} density="expanded" />)).toContain(" in · ");
  });
  it("never puts a literal colour in markup", () => {
    for (const row of FIXTURES)
      expect(renderToStaticMarkup(<SessionRow row={row} density="expanded" />)).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});
```

- [ ] **Step 6: Write the fixtures**

`src/features/sidebar/fixtures.ts`:
```ts
import type { RowModel } from "./types";

const MIN = 60_000;

/** One row per state from docs/PLAN.md "Sidebar rows", plus the modifiers and awkward names. */
export const FIXTURES: RowModel[] = [
  { id: "idle", name: "docs-site", project: "marketing", branch: "main", agent: "codex", status: "idle", phrase: "Idle", contextPct: 8 },
  {
    id: "working",
    name: "api-server",
    project: "my-app",
    branch: "feat/auth-flow",
    agent: "claude",
    status: "working",
    phrase: "Running npm test · 2 subagents",
    contextPct: 42,
    model: "Opus 5.5",
    effort: "high",
    subagents: 2,
    recap: { text: "Added token refresh to the auth client and is running the test suite.", ageMs: 2 * MIN },
    usage: { inContext: 84_000, window: 200_000, tokensIn: 312_000, tokensOut: 48_000, costUsd: 1.2 },
  },
  {
    id: "needs-permission",
    name: "billing",
    project: "payments",
    branch: "fix/retry-backoff",
    agent: "claude",
    status: "needs-permission",
    phrase: "Wants to run `npm test`",
    waitingMs: 65_000,
    contextPct: 61,
    flash: true,
  },
  {
    id: "needs-question",
    name: "infra",
    project: "terraform",
    branch: "chore/upgrade-aws-provider",
    agent: "gemini",
    status: "needs-question",
    phrase: "Has a question",
    waitingMs: 4 * MIN,
    contextPct: 18,
  },
  {
    id: "done-unseen",
    name: "landing",
    project: "marketing",
    branch: "feat/pricing-page",
    agent: "claude",
    status: "done-unseen",
    phrase: "Done · edited 4 files",
    ageMs: 40_000,
    contextPct: 55,
  },
  { id: "done-seen", name: "cli", project: "tools", branch: "main", agent: "codex", status: "done-seen", phrase: "Done 12m ago", ageMs: 12 * MIN, contextPct: 30 },
  {
    id: "error",
    name: "search",
    project: "my-app",
    branch: "feat/fts",
    agent: "claude",
    status: "error",
    phrase: "Stopped: rate limit reached. Resets at 3:40 pm.",
    contextPct: 97,
  },
  {
    id: "stuck",
    name: "migrations",
    project: "db",
    branch: "feat/v2-schema",
    agent: "generic",
    status: "stuck",
    phrase: "Same command 9× in 4 min",
    contextPct: 88,
  },
  { id: "ended", name: "scratch", project: "playground", agent: "generic", status: "ended", phrase: "Ended · exit 0", ageMs: 3 * 3_600_000 },
  {
    id: "limited",
    name: "legacy",
    project: "monolith",
    branch: "develop",
    agent: "codex",
    status: "limited",
    phrase: "Status limited · set up",
  },
  {
    id: "muted",
    name: "nightly-bench",
    project: "perf",
    branch: "main",
    agent: "claude",
    status: "working",
    phrase: "Running cargo bench",
    muted: true,
    contextPct: 12,
  },
  {
    id: "elevated",
    name: "drivers",
    project: "windows-setup",
    agent: "generic",
    status: "idle",
    phrase: "Administrator",
    elevated: true,
  },
  {
    id: "bypass",
    name: "refactor",
    project: "my-app",
    branch: "refactor/extract-billing-module",
    agent: "claude",
    status: "working",
    phrase: "Editing src/billing/index.ts",
    mode: "bypass",
    contextPct: 71,
  },
  {
    id: "unknown",
    name: "new-session",
    project: "my-app",
    branch: "main",
    agent: "gemini",
    status: "working",
    phrase: "Starting Gemini CLI in my-app…",
  },
  {
    id: "long-names",
    name: "a-very-long-session-name-that-will-not-fit",
    project: "an-equally-long-project-directory-name",
    branch: "feature/JIRA-12345-add-the-thing-everyone-asked-for",
    agent: "claude",
    status: "needs-permission",
    phrase: "Wants to run `rm -rf ./dist && npm run build -- --mode production`",
    waitingMs: 75 * MIN,
    contextPct: 50,
  },
  {
    id: "cjk-emoji",
    name: "認証サーバー 🚀",
    project: "프로젝트",
    branch: "fix/絵文字-🚀",
    agent: "claude",
    status: "done-unseen",
    phrase: "Done · edited 2 files",
    ageMs: 5_000,
    contextPct: 22,
  },
  {
    id: "rtl",
    name: "خادم-التحقق",
    project: "مشروع",
    branch: "main",
    agent: "codex",
    status: "idle",
    phrase: "Idle",
  },
  { id: "tiny", name: "x", project: "y", branch: "z", agent: "generic", status: "idle", phrase: "Idle" },
];
```

- [ ] **Step 7: Write the component**

`src/features/sidebar/SessionRow.tsx`:
```tsx
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { ContextRing } from "../../components/ContextRing/ContextRing";
import { formatDuration, formatUsage, modeCaution } from "./format";
import { rowLabel, rowTitle } from "./labels";
import { splitForMiddle } from "./truncate";
import type { Density, RowModel } from "./types";
import "./SessionRow.css";

const NEEDS_YOU = new Set(["needs-permission", "needs-question"]);

/** One sidebar row (docs/PLAN.md "Sidebar rows"). Height is fixed per density; hover never changes it. */
export function SessionRow({ row, density, selected = false }: { row: RowModel; density: Density; selected?: boolean }) {
  const caution = modeCaution(row.mode);
  const time = row.waitingMs ?? (row.status === "done-unseen" || row.status === "done-seen" || row.status === "ended" ? row.ageMs : undefined);
  const branch = row.branch ? splitForMiddle(row.branch) : null;

  return (
    <div
      className="session-row"
      role="listitem"
      tabIndex={0}
      title={rowTitle(row)}
      aria-label={rowLabel(row)}
      aria-current={selected || undefined}
      data-density={density}
      data-agent={row.agent}
      data-status={row.status}
      data-needs-you={NEEDS_YOU.has(row.status) || undefined}
      data-unseen={row.status === "done-unseen" || undefined}
      data-ended={row.status === "ended" || undefined}
      data-flash={row.flash || undefined}
    >
      <span className="session-row__stripe" aria-hidden="true" />
      <div className="session-row__line1" aria-hidden="true">
        <StatusGlyph kind={row.status} />
        {row.elevated && <AuxGlyph kind="elevated" />}
        <span className="session-row__name" style={{ minInlineSize: `${Math.min([...row.name].length, 8)}ch` }}>
          {row.name}
        </span>
        {row.status === "done-unseen" && <span className="session-row__unread" />}
        <span className="session-row__project">{row.project}</span>
        {branch && (
          <>
            <span className="session-row__sep">·</span>
            <span className="session-row__branch-head">{branch.head}</span>
            <span className="session-row__branch-tail">{branch.tail}</span>
          </>
        )}
        <span className="session-row__cluster">
          {caution && <AuxGlyph kind="caution" label={caution} />}
          {row.muted && <AuxGlyph kind="muted" />}
          {time !== undefined && <span className="session-row__time">{formatDuration(time)}</span>}
          <ContextRing pct={row.contextPct} />
        </span>
      </div>
      {density !== "compact" && (
        <div className="session-row__phrase" aria-hidden="true">
          {row.phrase}
          {row.muted && <span className="session-row__suffix"> · muted</span>}
        </div>
      )}
      {density === "expanded" && (
        <div className="session-row__details" aria-hidden="true">
          <div className="session-row__meta">
            <span>{row.model ?? "–"}</span>
            <span>{row.effort ?? "–"}</span>
            {row.mode === "plan" && <span className="session-row__pill">Plan</span>}
            {caution && <span className="session-row__caution-text">{caution}</span>}
            <span>{row.subagents !== undefined ? `${row.subagents} subagents` : "–"}</span>
          </div>
          {row.recap && (
            <p className="session-row__recap">
              {row.recap.text} <span className="session-row__age">{formatDuration(row.recap.ageMs)} ago</span>
            </p>
          )}
          <div className="session-row__usage">{formatUsage(row.usage)}</div>
        </div>
      )}
    </div>
  );
}
```

`src/features/sidebar/SessionRow.css`:
```css
.session-row {
  position: relative;
  display: grid;
  align-content: center;
  padding-inline: var(--space-3) 10px;
  color: var(--text-1);
  font-size: var(--text-13);
  line-height: 20px;
  outline-offset: -2px;
  cursor: default;
}

.session-row[data-density="compact"] {
  block-size: var(--row-compact);
}
.session-row[data-density="comfortable"] {
  block-size: var(--row-comfortable);
}
.session-row[data-density="expanded"] {
  min-block-size: var(--row-comfortable);
  padding-block: var(--space-2);
}

.session-row[aria-current="true"] {
  background: var(--selected);
}
.session-row[data-needs-you] {
  background: var(--accent-tint);
}
.session-row[data-ended] {
  color: color-mix(in oklch, var(--text-1) 60%, transparent);
}

@media (hover: hover) and (pointer: fine) {
  .session-row:hover {
    background: var(--hover);
  }
  .session-row[data-needs-you]:hover {
    background: color-mix(in oklch, var(--accent) 10%, transparent);
  }
}

.session-row:focus-visible {
  outline: 2px solid var(--accent);
}

/* Brand stripe, flush to the inline start. */
.session-row__stripe {
  position: absolute;
  inset-block: 0;
  inset-inline-start: 0;
  inline-size: var(--stripe);
  background: var(--brand-generic);
}
.session-row[data-agent="claude"] .session-row__stripe {
  background: var(--brand-claude);
}
.session-row[data-agent="codex"] .session-row__stripe {
  background: var(--brand-codex);
}
.session-row[data-agent="gemini"] .session-row__stripe {
  background: var(--brand-gemini);
}

/*
 * Truncation order (docs/PLAN.md): branch first (middle), then project (end), then name (end, keeps 8 characters).
 * Large shrink ratios make the branch give up its width before the project, and the project before the name.
 */
.session-row__line1 {
  display: flex;
  align-items: center;
  min-inline-size: 0;
  white-space: nowrap;
}

.session-row__line1 > .status-glyph {
  margin-inline-end: var(--space-2);
}
.session-row__line1 > .aux-glyph {
  margin-inline-end: var(--space-1);
}

/* The minimum (up to 8 characters) is set inline from the name's length; see SessionRow.tsx. */
.session-row__name {
  flex: 0 1 auto;
  overflow: clip;
  text-overflow: ellipsis;
  font-weight: 500;
}
.session-row[data-unseen] .session-row__name {
  font-weight: 600;
}

.session-row__unread {
  flex: none;
  inline-size: 6px;
  block-size: 6px;
  margin-inline-start: var(--space-1);
  border-radius: 50%;
  background: var(--text-1);
}

.session-row__project {
  flex: 0 100 auto;
  min-inline-size: 0;
  margin-inline-start: 6px;
  overflow: clip;
  text-overflow: ellipsis;
  color: var(--text-2);
  font-size: var(--text-12);
}

.session-row__sep {
  flex: none;
  padding-inline: var(--space-1);
  color: var(--text-3);
  font-size: var(--text-12);
}

.session-row__branch-head {
  flex: 0 10000 auto;
  min-inline-size: 0;
  overflow: clip;
  text-overflow: ellipsis;
  color: var(--text-2);
  font-size: var(--text-12);
}
.session-row__branch-tail {
  flex: none;
  color: var(--text-2);
  font-size: var(--text-12);
}

/* The right cluster never truncates. */
.session-row__cluster {
  flex: none;
  display: flex;
  align-items: center;
  gap: 6px;
  margin-inline-start: auto;
  padding-inline-start: var(--space-2);
}

.session-row__time {
  color: var(--text-2);
  font-size: var(--text-11);
  font-variant-numeric: tabular-nums;
}
.session-row[data-needs-you] .session-row__time {
  color: var(--text-1);
  font-weight: 500;
}

.session-row__phrase {
  padding-inline-start: calc(var(--glyph) + var(--space-2));
  overflow: clip;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-2);
  font-size: var(--text-12);
  line-height: 16px;
}

.session-row__details {
  display: grid;
  gap: var(--space-1);
  padding-inline-start: calc(var(--glyph) + var(--space-2));
  margin-block-start: var(--space-1);
  color: var(--text-2);
  font-size: var(--text-12);
  line-height: 16px;
}

.session-row__meta {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1) var(--space-2);
}

.session-row__pill {
  padding-inline: 6px;
  border: 1px solid var(--hairline);
  border-radius: 999px;
  color: var(--text-1);
  font-size: var(--text-11);
}

.session-row__caution-text {
  color: var(--text-1);
}

.session-row__recap {
  margin: 0;
  color: var(--text-1);
}

.session-row__age,
.session-row__usage {
  color: var(--text-3);
  font-size: var(--text-11);
  font-variant-numeric: tabular-nums;
}
```

- [ ] **Step 8: Run the row tests**

Run: `pnpm vitest run src/features/sidebar`
Expected: PASS.

- [ ] **Step 9: Write the row matrix stories**

`src/features/sidebar/SessionRow.stories.tsx`:
```tsx
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { FIXTURES } from "./fixtures";
import { SessionRow } from "./SessionRow";
import type { Density } from "./types";

const meta: Meta = { title: "Sidebar/Session rows" };
export default meta;

function Column({ density, width, ids }: { density: Density; width: number; ids?: string[] }) {
  const rows = ids ? FIXTURES.filter((f) => ids.includes(f.id)) : FIXTURES;
  return (
    <div role="list" style={{ inlineSize: width, background: "var(--bg-raised)", borderInlineEnd: "1px solid var(--hairline)" }}>
      {rows.map((r) => (
        <SessionRow key={r.id} row={r} density={density} selected={r.id === "working"} />
      ))}
    </div>
  );
}

export const Compact: StoryObj = {
  render: () => (
    <ThemePair label="Compact rows at 264 px">
      <Column density="compact" width={264} />
    </ThemePair>
  ),
};

export const Comfortable: StoryObj = {
  render: () => (
    <ThemePair label="Comfortable rows at 264 px">
      <Column density="comfortable" width={264} />
    </ThemePair>
  ),
};

export const Expanded: StoryObj = {
  render: () => (
    <ThemePair label="Expanded rows at 264 px">
      <Column density="expanded" width={264} ids={["working", "needs-permission", "bypass", "unknown", "error"]} />
    </ThemePair>
  ),
};

/** The minimum sidebar width. Check the truncation order: branch (middle), then project, then name (8 characters kept). */
export const TruncationAt200: StoryObj = {
  render: () => (
    <ThemePair label="Truncation at 200 px">
      <Column density="comfortable" width={200} ids={["long-names", "cjk-emoji", "rtl", "tiny", "bypass", "needs-question"]} />
    </ThemePair>
  ),
};

/** The same long row at shrinking widths, to see each truncation step. */
export const TruncationSteps: StoryObj = {
  render: () => (
    <ThemePair label="Truncation steps">
      {[400, 330, 280, 240, 200].map((w) => (
        <div key={w} style={{ marginBlockEnd: "var(--space-2)" }}>
          <div style={{ fontSize: "var(--text-11)", color: "var(--text-3)" }}>{w} px</div>
          <Column density="compact" width={w} ids={["long-names"]} />
        </div>
      ))}
    </ThemePair>
  ),
};
```

- [ ] **Step 10: Verify**

Run: `pnpm build && pnpm test && pnpm build-storybook`

Expected: all green.

If an ellipsis does not render in Chromium with `overflow: clip`, switch only the affected text spans to `overflow: hidden` and say so in your report. good-css prefers `clip`, but the ellipsis is the requirement here.

Then, in Storybook:
- **Sidebar/Session rows / TruncationSteps:** the branch shrinks first with its tail kept (`…-for`), then the project, then the name, which keeps at least 8 characters. The right cluster stays whole at every width.
- **Comfortable:** every row is exactly 48 px, and hovering changes only the background.
- **Toolbar:** switching the accent recolours only the needs-you rows' tint and badges; the brand stripes don't change.

- [ ] **Step 11: Commit**

```bash
git add src/features/sidebar
git commit -m "feat(ui): sidebar session row in every state and density"
```

---

### Task 5: Publish the batch 1 preview and the review checklist (controller)

This task is for the controller, not an implementer. It publishes outward, so it stays with the session that talks to the user.

- [ ] **Step 1:** Run `pnpm build-storybook` and count the files under `storybook-static/`.
  - If the count is at most 255, publish it with the Artifact tool as a multi-file artifact: the page is `storybook-static/index.html`, and every other file goes in `files` at its relative path.
  - If it is above 255, publish over several publishes to the same URL, as the Artifact tool describes.
- [ ] **Step 2:** Give the user the link and this sign-off checklist (from docs/PLAN.md "Phase 0: what approved means", scoped to batch 1):
  1. **Colours:** the AA and distinctness matrix has no red cells, and amber, caution, error and Claude orange read as four different things.
  2. **Glyphs:** each of the ten states is recognisable at 12 px without colour, and the reduced-motion forms make sense.
  3. **Rows:** compact, comfortable and expanded read well, and the truncation order holds at 200 px, including with CJK, emoji and RTL names.
  4. **Scale:** look at it at 100% and 150% browser zoom on Windows.
  5. **Keyboard:** Tab reaches every row, and the focus ring is visible in both themes.
  6. **The 5-second test** waits for batch 2's main-window composite.
- [ ] **Step 3:** Record the user's verdict in the ledger. Batch 2 starts only after sign-off; feedback becomes a fix round on this branch.
