import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { GLYPHS, type GlyphKind } from "../StatusGlyph/glyphs";
import { StatusGlyph } from "../StatusGlyph/StatusGlyph";
import { FIXTURES } from "../../features/sidebar/fixtures";
import { rowLabel } from "../../features/sidebar/labels";
import { SessionRow } from "../../features/sidebar/SessionRow";
import { MainWindow } from "../../features/window/MainWindow";
import { ShellPanel } from "../../features/window/ShellPanel";
import { SCENARIO_REQUESTS, SCENARIO_ROWS } from "../../features/window/scenario";
import { rules } from "./cssRules";

/**
 * Table-driven rules of the accessibility safety net (docs/PLAN.md, "Accessibility safety net"): the things reviewers kept
 * finding late, checked on every fixture so a new state or a new toggle cannot skip them.
 */

// 1. Every status is readable without colour: a glyph SHAPE plus words.
const STATUSES: GlyphKind[] = ["needs-permission", "needs-question", "error", "done-unseen", "done-seen", "working", "stuck", "idle", "ended", "running", "paused"];

describe("every status is readable without colour", () => {
  const svg = (k: GlyphKind) => renderToStaticMarkup(<StatusGlyph kind={k} />);
  const shapeOf = (k: GlyphKind) => svg(k).replace(/<svg[^>]*>/, "").replace("</svg>", "");

  for (const kind of STATUSES) {
    it(`${kind}: a drawn shape and a name in words`, () => {
      expect(shapeOf(kind).length, "has a drawn shape").toBeGreaterThan(20);
      expect(svg(kind)).toContain(`aria-label="${GLYPHS[kind].label}"`);
      expect(GLYPHS[kind].label.length).toBeGreaterThan(3);
    });
  }
  it("no two statuses share a shape, so colour is never the only difference", () => {
    const shapes = STATUSES.map(shapeOf);
    // Needs-permission and needs-question share the badge on purpose; the mark inside (padlock, question mark) tells them apart.
    expect(new Set(shapes).size).toBe(shapes.length);
  });
  it("no two statuses share a name", () => {
    const names = STATUSES.map((k) => GLYPHS[k].label);
    expect(new Set(names).size).toBe(names.length);
  });
  it("every status is shown by some fixture, and each row says it in words (the label) next to the glyph", () => {
    for (const kind of STATUSES) {
      const rows = FIXTURES.filter((f) => f.status === kind);
      expect(rows.length, `a fixture with status ${kind}`).toBeGreaterThan(0);
      for (const row of rows) {
        expect(rowLabel(row), row.id).toContain(GLYPHS[kind].label);
        const html = renderToStaticMarkup(<SessionRow row={row} density="comfortable" />);
        // A working agent shows its moving vendor mark in the glyph's place (by design); every other state shows the glyph.
        expect(/status-glyph|agent-mark/.test(html), row.id).toBe(true);
        expect(html, row.id).toContain(row.phrase.split("`")[0]!.trim().slice(0, 6));
      }
    }
  });
});

// 2. A toggle keeps one accessible name; its state is aria-pressed / aria-expanded, never a changed label.
function toggles(html: string): string[] {
  return [...html.matchAll(/<button\b[^>]*\b(aria-pressed|aria-expanded)=[^>]*>/g)]
    .map((m) => m[0])
    .map((tag) => /aria-label="([^"]*)"/.exec(tag)?.[1] ?? "(named by text)")
    .sort();
}

describe("a toggle keeps one name across its states", () => {
  const win = (p: Partial<Parameters<typeof MainWindow>[0]>) =>
    renderToStaticMarkup(<MainWindow width={1700} height={900} rows={SCENARIO_ROWS} requests={SCENARIO_REQUESTS} selectedId="working" layout="split" splitCount={4} {...p} />);

  it("zoom pane: the same buttons are named the same zoomed and not zoomed", () => {
    const a = toggles(win({ zoomedPane: undefined }));
    const b = toggles(win({ zoomedPane: 1 }));
    expect(a.length).toBeGreaterThan(0);
    expect(b.filter((n) => a.includes(n)).length).toBeGreaterThan(0);
    for (const name of b) expect(a, `"${name}" exists with the zoom off too`).toContain(name);
  });
  it("shell panel: hidden and shown give the same name", () => {
    const shown = toggles(renderToStaticMarkup(<ShellPanel height={284} cwd="my-app" collapsed={false} />));
    const hidden = toggles(renderToStaticMarkup(<ShellPanel height={40} cwd="my-app" collapsed />));
    expect(shown).toEqual(hidden);
  });
  it("the layout button keeps its name open and closed", () => {
    const closed = toggles(win({}));
    const open = toggles(win({ layoutMenuOpen: true } as never));
    for (const name of open) expect(closed).toContain(name);
  });
});

// 3. One live region per window: the Announcer.
describe("the window has exactly one live region", () => {
  const live = (h: string) => (h.match(/role="(status|alert)"|aria-live=/g) ?? []).length;
  const WIN = { width: 1700, height: 900, rows: SCENARIO_ROWS, requests: SCENARIO_REQUESTS, selectedId: "working" } as const;
  it("in every layout the markup has role=status/aria-live once (role and aria-live sit on the same element, so the pair counts as one)", () => {
    for (const p of [{}, { layout: "split", splitCount: 2 }, { layout: "split", splitCount: 4 }, { layout: "split", splitCount: 4, zoomedPane: 1 }, { focusMode: true }, { mode: "rail" }] as const) {
      const h = renderToStaticMarkup(<MainWindow {...WIN} {...(p as object)} />);
      expect((h.match(/role="status"/g) ?? []).length, JSON.stringify(p)).toBe(1);
      expect((h.match(/aria-live=/g) ?? []).length, JSON.stringify(p)).toBe(1);
      expect(live(h), JSON.stringify(p)).toBe(2);
    }
  });
  it("it is the shared Announcer, empty on first render", () => {
    const h = renderToStaticMarkup(<MainWindow {...WIN} />);
    expect(/<div[^>]*data-session-announcer[^>]*><\/div>/.test(h)).toBe(true);
  });
});

// 4. Forced colours: system colours only, and no border colours.
const SRC = fileURLToPath(new URL("../../", import.meta.url));
function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}
const SYSTEM = new Set(["canvas", "canvastext", "linktext", "visitedtext", "activetext", "buttonface", "buttontext", "buttonborder", "field", "fieldtext", "highlight", "highlighttext", "graytext", "mark", "marktext", "selecteditem", "selecteditemtext", "accentcolor", "accentcolortext", "transparent", "currentcolor", "inherit", "none", "auto"]);
const COLOUR_PROPS = /^(color|background|background-color|outline|outline-color|border[a-z-]*|fill|stroke|text-decoration-color|caret-color|box-shadow)$/;

describe("every @media (forced-colors: active) rule uses system colours only and no border colours", () => {
  const css = walk(SRC).filter((p) => p.endsWith(".css"));
  for (const file of css) {
    const forced = rules(readFileSync(file, "utf8")).filter((r) => r.forced);
    if (forced.length === 0) continue;
    it(`${relative(SRC, file).split("\\").join("/")}`, () => {
      const bad: string[] = [];
      for (const r of forced)
        for (const d of r.body.split(";")) {
          const [prop, ...rest] = d.split(":");
          const p = prop?.trim().toLowerCase() ?? "";
          const value = rest.join(":").trim();
          if (!COLOUR_PROPS.test(p) || !value) continue;
          if (/^border[a-z-]*color$/.test(p)) bad.push(`${r.selectors.join(", ")} { ${p} } uses a border colour`);
          // Drop lengths and keywords like 2px solid; what remains must be system colour names.
          const words = value.replace(/var\([^)]*\)/g, "VAR").split(/\s+/).filter((w) => w && !/^-?\d/.test(w) && !/^(solid|dashed|dotted|double|inset|outset|thin|medium|thick|!important)$/.test(w));
          for (const w of words) if (!SYSTEM.has(w.toLowerCase()) && w !== "VAR") bad.push(`${r.selectors.join(", ")} { ${p}: ${value} } is not a system colour`);
        }
      expect(bad).toEqual([]);
    });
  }
});
