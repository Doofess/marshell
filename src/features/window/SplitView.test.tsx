import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SCENARIO_ROWS } from "./scenario";
import { COMPOSER } from "./layout";
import { SplitView, type SplitViewProps } from "./SplitView";

const [billing, infra, api, docs, bug] = SCENARIO_ROWS;
const html = (p: Partial<SplitViewProps> = {}) => renderToStaticMarkup(<SplitView panes={[api!, billing!]} size={{ w: 1180, h: 760 }} {...p} />);
const count = (h: string, re: RegExp) => (h.match(re) ?? []).length;

describe("SplitView: panes", () => {
  it("shows one pane and one live terminal per session, two to four of them", () => {
    for (const rows of [[api!, billing!], [api!, billing!, infra!], [api!, billing!, infra!, docs!]]) {
      const h = html({ panes: rows });
      expect(count(h, /class="pane"/g), String(rows.length)).toBe(rows.length);
      expect(count(h, /class="scripted-terminal"/g)).toBe(rows.length);
    }
  });
  it("names every pane and terminal after its session", () => {
    const h = html({ panes: [api!, billing!, infra!] });
    for (const r of [api!, billing!, infra!]) {
      expect(h).toContain(`aria-label="Session ${r.number}, terminal: ${r.name}"`);
      expect(h).toContain(`aria-label="Terminal for ${r.name}"`);
    }
  });
  it("refuses more than four", () => {
    expect(() => html({ panes: [api!, billing!, infra!, docs!, bug!] })).toThrow();
  });
  it("marks exactly one pane active, the first by default", () => {
    const h = html({ panes: [api!, billing!, infra!] });
    expect(count(h, /data-active="true"/g)).toBe(1);
    expect(count(h, /data-active="false"/g)).toBe(2);
    expect(h.indexOf('data-active="true"')).toBeLessThan(h.indexOf('data-active="false"'));
    const second = html({ panes: [api!, billing!], activeIndex: 1 });
    expect(second.indexOf('data-active="false"')).toBeLessThan(second.indexOf('data-active="true"'));
  });
});

describe("SplitView: arrangement", () => {
  it("picks the arrangement from the space and says which it picked", () => {
    expect(html({ panes: [api!, billing!], size: { w: 1400, h: 800 } })).toContain('data-arrangement="columns"');
    expect(html({ panes: [api!, billing!], size: { w: 560, h: 1100 } })).toContain('data-arrangement="rows"');
    expect(html({ panes: [api!, billing!, infra!, docs!], size: { w: 1400, h: 800 } })).toContain('data-arrangement="grid"');
  });
  it("honours an arrangement the user chose", () => {
    const h = html({ panes: [api!, billing!, infra!], arrangement: "main-top" });
    expect(h).toContain('data-arrangement="main-top"');
    expect(h).toContain('data-auto="false"');
    expect(html()).toContain('data-auto="true"');
  });
  it("lays the tracks out in CSS grid, and places each pane in its own cell", () => {
    const h = html({ panes: [api!, billing!, infra!], arrangement: "main-left" });
    expect(h).toContain("grid-template-columns:minmax(0, 0.5fr) 1px minmax(0, 0.5fr)");
    expect(h).toContain("grid-area:1 / 1 / 4 / 2");
  });
  it("gives every divider a separator with its direction and values, 20 to 80 for a two-way split", () => {
    const h = html({ panes: [api!, billing!, infra!, docs!], arrangement: "grid" });
    expect(count(h, /role="separator"/g)).toBe(2);
    expect(h).toContain('aria-orientation="vertical"');
    expect(h).toContain('aria-orientation="horizontal"');
    expect(h).toContain('aria-valuenow="50"');
    expect(h).toContain('aria-valuemin="20"');
    expect(h).toContain('aria-valuemax="80"');
    expect(h).toContain('tabindex="0"');
  });
  it("tells the dividers of three columns apart for a screen reader", () => {
    const h = html({ panes: [api!, billing!, infra!], arrangement: "columns" });
    expect(h).toContain('aria-label="Resize panes 1 and 2"');
    expect(h).toContain('aria-label="Resize panes 2 and 3"');
  });
  it("follows the ratio the user dragged to", () => {
    const h = html({ ratios: { col: 0.7 } });
    expect(h).toContain("minmax(0, 0.7fr) 1px minmax(0, 0.3fr)");
    expect(h).toContain('aria-valuenow="70"');
  });
});

describe("SplitView: what each pane header says", () => {
  it("shows the status glyph, name and project, with the brand stripe", () => {
    const h = html({ panes: [billing!, api!] });
    expect(h).toContain("billing");
    expect(h).toContain("pane__stripe");
    expect(h).toContain('data-agent="claude"');
    expect(h).toContain("status-glyph");
  });
  it("shows the animated vendor mark for a working session, as the rail and rows do", () => {
    expect(html({ panes: [api!, billing!] })).toContain("agent-mark");
  });
  it("marks a pane that needs you, and never lets it recede with the inactive headers", () => {
    const h = html({ panes: [api!, billing!] });
    expect(h).toContain('data-needs="true"');
    expect(count(h, /data-needs="true"/g)).toBe(1);
  });
  it("warns, with the numbers, when a pane is too narrow for most CLIs, and stays quiet when it is not", () => {
    const tight = html({ panes: [api!, billing!, infra!, docs!], size: { w: 668, h: 440 } });
    expect(tight).toMatch(/\d+×\d+/);
    expect(tight).toContain("narrower than 80 columns");
    expect(html({ panes: [api!, billing!], size: { w: 1900, h: 1000 } })).not.toMatch(/\d+×\d+/);
  });
  it("gives each pane a zoom and a remove button, named after the session", () => {
    const h = html({ panes: [api!, billing!] });
    expect(h).toContain('aria-label="Zoom api-server"');
    expect(h).toContain('aria-label="Remove billing from the view"');
    expect(h).toContain('aria-pressed="false"');
  });
  it("keeps a session running when its pane is removed, and says so", () => {
    expect(html()).toContain("The session keeps running");
  });
});

describe("SplitView: zoom", () => {
  const zoomed = html({ panes: [api!, billing!, infra!, docs!], zoomed: 2 });
  it("fills the space with the one zoomed pane", () => {
    expect(count(zoomed, /class="pane"/g)).toBe(1);
    expect(zoomed).toContain('aria-label="Session 2, terminal: infra"');
    expect(zoomed).toContain('data-arrangement="single"');
    expect(zoomed).not.toContain('role="separator"');
  });
  it("offers the way back and names how many are still running", () => {
    expect(zoomed).toContain('aria-pressed="true"');
    expect(zoomed).toContain('aria-label="Restore the 4 panes"');
    expect(zoomed).toContain("3 more running");
  });
});

describe("SplitView: an empty pane", () => {
  const empty = html({ panes: [api!, billing!, null], choices: [infra!, docs!, bug!] });
  it("asks which session goes there, listing the ones not on screen", () => {
    expect(empty).toContain("Choose a session for this pane");
    for (const r of [infra!, docs!, bug!]) expect(empty).toContain(r.name);
    expect(empty).not.toContain('terminal: null"');
  });
  it("offers to start a new one, with its shortcut", () => {
    expect(empty).toContain("Start a new session");
    expect(empty).toContain("Ctrl+Shift+T");
  });
  it("counts the empty pane in the layout but gives it no terminal", () => {
    expect(count(empty, /class="scripted-terminal"/g)).toBe(2);
    expect(count(empty, /<section class="pane/g)).toBe(3);
  });
  it("shows the Mac shortcut on macOS", () => {
    expect(html({ panes: [api!, null], choices: [infra!], os: "mac" })).toContain("⌘T");
  });
  it("says so when every session is already on screen", () => {
    expect(html({ panes: [api!, null], choices: [] })).toContain("Every session is already on screen");
  });
});

describe("SplitView: sessions with no room", () => {
  it("names how many are running out of sight in the focused pane's header only", () => {
    const h = html({ panes: [api!, billing!], hidden: 2, activeIndex: 1 });
    expect(count(h, /2 more running/g)).toBe(1);
    expect(h.indexOf("2 more running")).toBeGreaterThan(h.indexOf('aria-label="Session 1, terminal: billing"'));
    expect(html({ panes: [api!, billing!] })).not.toContain("more running");
  });
  it("lists at most five sessions in an empty pane and sends the rest to the command palette, since the window never scrolls", () => {
    const many = Array.from({ length: 8 }, (_, i) => ({ ...infra!, id: `x${i}`, name: `session-${i}` }));
    const h = html({ panes: [api!, null], choices: many });
    expect(count(h, /class="pane__choice"/g)).toBe(5);
    expect(h).toContain("+3 more, open the command palette (Ctrl+Shift+K)");
    expect(html({ panes: [api!, null], choices: many.slice(0, 5) })).not.toContain("more, open");
  });
});

describe("SplitView: an arrangement that does not suit the count", () => {
  it("falls back to automatic rather than throwing", () => {
    const h = html({ panes: [api!, billing!], arrangement: "main-left" });
    expect(h).toContain('data-auto="true"');
    expect(["columns", "rows"]).toContain(/data-arrangement="([^"]+)"/.exec(h)![1]);
  });
  it("treats a zoomed empty slot as not zoomed, the same way the window does", () => {
    const h = html({ panes: [api!, null], zoomed: 1, choices: [billing!] });
    expect(count(h, /<section class="pane/g)).toBe(2);
  });
});

describe("SplitView: every pane has its own prompt line", () => {
  const forms = (h: string) => (h.match(/<form class="composer"/g) ?? []).length;
  it("gives each pane with a session its own, named for that pane's session", () => {
    const h = html({ panes: [api!, billing!, infra!, docs!], size: { w: 1700, h: 900 } });
    expect(forms(h)).toBe(4);
    for (const r of [api!, billing!, infra!, docs!]) expect(h).toContain(`aria-label="Write to ${r.name}"`);
  });
  it("docks it at the bottom of its own pane, in a fixed slot the terminal above never loses", () => {
    const h = html({ panes: [api!, billing!] });
    for (const m of h.split("</section>").slice(0, 2)) {
      expect(m).toMatch(/<div class="pane__composer"[^>]*style="block-size:\d+px"[^>]*>\s*<form class="composer"/);
      expect(m.indexOf('class="scripted-terminal"')).toBeLessThan(m.indexOf('class="pane__composer"'));
    }
  });
  it("sizes the slot and the pane's rows from the layout constants, never from a literal", () => {
    const h = html({ panes: [api!, billing!] });
    expect(h).toContain(`block-size:${COMPOSER.compact}px`);
    expect(h).toContain(`grid-template-rows:28px minmax(0, 1fr) ${COMPOSER.compact}px`);
  });
  it("is individual: a pane whose session needs the user has its line off, while the others stay live", () => {
    const h = html({ panes: [api!, billing!, docs!] });
    const sections = h.split("</section>");
    const off = (s: string) => /<textarea[^>]*disabled/.test(s);
    expect(off(sections.find((s) => s.includes('aria-label="Session 1, terminal: billing"'))!)).toBe(true);
    expect(off(sections.find((s) => s.includes('aria-label="Session 3, terminal: api-server"'))!)).toBe(false);
    expect(off(sections.find((s) => s.includes('aria-label="Session 5, terminal: landing"'))!)).toBe(false);
  });
  it("gives each pane the text typed in that pane, by session id", () => {
    const h = html({ panes: [api!, billing!], composerValues: { [api!.id]: "also add tests" } });
    expect(h).toContain("also add tests");
    expect((h.match(/also add tests/g) ?? []).length).toBe(1);
  });
  it("keeps the zoomed pane's own prompt line, and only that one", () => {
    const h = html({ panes: [api!, billing!, infra!], zoomed: 1, size: { w: 1700, h: 900 } });
    expect(forms(h)).toBe(1);
    expect(h).toContain('aria-label="Write to billing"');
  });
  it("gives an empty pane none: its picker is enough", () => {
    const h = html({ panes: [api!, null], choices: [billing!] });
    expect(forms(h)).toBe(1);
    expect(h).not.toContain('aria-label="Write to this pane"');
  });
  it("can be turned off for every pane, and then counts no prompt line in the rows", () => {
    const h = html({ panes: [api!, billing!], composers: false });
    expect(forms(h)).toBe(0);
    expect(h).not.toContain("pane__composer");
    expect(h).not.toContain(`grid-template-rows:28px minmax(0, 1fr) ${COMPOSER.compact}px`);
  });
  it("does not say 'Pane 2 of 4': with a prompt line in each pane the pane itself is the target", () => {
    expect(html({ panes: [api!, billing!, infra!, docs!], size: { w: 1700, h: 900 } })).not.toMatch(/Pane \d of \d/);
  });
});

describe("SplitView: hygiene", () => {
  it("puts no literal colour in the markup", () => {
    expect(html({ panes: [api!, billing!, infra!, docs!] })).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});
