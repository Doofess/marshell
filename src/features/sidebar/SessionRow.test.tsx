import { readFileSync } from "node:fs";
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
  for (const density of ["comfortable", "expanded"] as const)
    it(`renders every fixture in ${density}`, () => {
      for (const row of FIXTURES) {
        const html = renderToStaticMarkup(<SessionRow row={row} density={density} />);
        expect(html, row.id).toContain(`data-density="${density}"`);
        expect(html, row.id).toContain(`data-agent="${row.agent}"`);
        expect(html, row.id).toContain("aria-label=");
      }
    });
  it("shows line 2 in every density", () => {
    const row = FIXTURES[0]!;
    for (const density of ["comfortable", "expanded"] as const)
      expect(renderToStaticMarkup(<SessionRow row={row} density={density} />), density).toContain("session-row__phrase");
  });
  it("shows usage only when expanded", () => {
    const row = FIXTURES.find((f) => f.usage)!;
    expect(renderToStaticMarkup(<SessionRow row={row} density="comfortable" />)).not.toContain(" in · ");
    expect(renderToStaticMarkup(<SessionRow row={row} density="expanded" />)).toContain(" in · ");
  });
  it("shows no usage line and no context ring when the context data is unknown", () => {
    const row = FIXTURES.find((f) => f.id === "unknown")!;
    const html = renderToStaticMarkup(<SessionRow row={row} density="expanded" />);
    expect(html).not.toContain("session-row__usage");
    expect(html).not.toContain("context-ring");
  });
  it("never puts a literal colour in markup", () => {
    for (const row of FIXTURES)
      expect(renderToStaticMarkup(<SessionRow row={row} density="expanded" />)).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});

describe("SessionRow (final review)", () => {
  const row = FIXTURES.find((f) => f.id === "needs-permission")!;
  it("is an option in a listbox with roving focus", () => {
    const sel = renderToStaticMarkup(<SessionRow row={row} density="comfortable" selected tabStop />);
    expect(sel).toContain('role="option"');
    expect(sel).toContain('aria-selected="true"');
    expect(sel).toContain('tabindex="0"');
    const other = renderToStaticMarkup(<SessionRow row={row} density="comfortable" />);
    expect(other).toContain('aria-selected="false"');
    expect(other).toContain('tabindex="-1"');
  });
  it("marks muted rows so their badge does not bounce", () => {
    expect(renderToStaticMarkup(<SessionRow row={{ ...row, muted: true }} density="comfortable" />)).toContain("data-muted");
  });
  it("lets the browser pick the direction of names, projects and branches", () => {
    const html = renderToStaticMarkup(<SessionRow row={FIXTURES.find((f) => f.id === "rtl")!} density="comfortable" />);
    expect(html.match(/dir="auto"/g)?.length).toBeGreaterThanOrEqual(3);
  });
  it("renders no empty branch head, so the tail never reads as a whole word", () => {
    const html = renderToStaticMarkup(<SessionRow row={{ ...row, branch: "main" }} density="comfortable" />);
    expect(html).not.toContain("session-row__branch-head");
    expect(html).toContain("session-row__branch-tail");
  });
  it("has fixtures for the crowded cases at 200 px", () => {
    expect(FIXTURES.some((f) => f.id === "crowded")).toBe(true);
    expect(FIXTURES.some((f) => f.id === "emoji-name")).toBe(true);
    expect(FIXTURES.some((f) => f.id === "muted-needs-you" && f.muted && f.status === "needs-permission")).toBe(true);
  });
});

describe("SessionRow (agent mark and model)", () => {
  const working = FIXTURES.find((f) => f.id === "working")!;
  const lead = (html: string) => html.slice(html.indexOf("session-row__line1"));
  it("leads with the agent mark, not the status glyph", () => {
    const html = lead(renderToStaticMarkup(<SessionRow row={working} density="comfortable" />));
    expect(html.indexOf("agent-mark")).toBeGreaterThan(-1);
    expect(html.indexOf("agent-mark")).toBeLessThan(html.indexOf("session-row__name"));
  });
  it("puts the status glyph in the right cluster, before the timer", () => {
    const row = FIXTURES.find((f) => f.id === "needs-permission")!;
    const html = renderToStaticMarkup(<SessionRow row={row} density="comfortable" />);
    const cluster = html.slice(html.indexOf("session-row__cluster"));
    expect(cluster).toContain('class="status-glyph"');
    expect(cluster.indexOf("status-glyph")).toBeLessThan(cluster.indexOf("session-row__time"));
    expect(html.slice(0, html.indexOf("session-row__cluster"))).not.toContain("status-glyph");
  });
  it("shows the model first on line 2", () => {
    const html = renderToStaticMarkup(<SessionRow row={working} density="comfortable" />);
    const line2 = html.slice(html.indexOf("session-row__phrase"));
    expect(line2).toContain("session-row__model");
    expect(line2.indexOf("Opus 5.5")).toBeLessThan(line2.indexOf("Running npm test"));
  });
  it("omits the model on line 2 when the CLI does not report one, and never guesses", () => {
    const row = FIXTURES.find((f) => f.id === "unknown")!;
    expect(renderToStaticMarkup(<SessionRow row={row} density="comfortable" />)).not.toContain("session-row__model");
  });
  it("names the agent and model for screen readers and the tooltip", () => {
    const html = renderToStaticMarkup(<SessionRow row={working} density="comfortable" />);
    expect(html).toMatch(/aria-label="[^"]*Claude[^"]*Opus 5\.5/);
    expect(html).toMatch(/title="[^"]*Claude[^"]*Opus 5\.5/);
  });
  it("has only comfortable and expanded densities", () => {
    const css = readFileSync(new URL("./SessionRow.css", import.meta.url), "utf8");
    expect(css).not.toContain("data-density=\"compact\"");
    expect(readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8")).not.toContain("--row-compact");
  });
});

describe("SessionRow (blocked rows stand out)", () => {
  const html = (id: string) => renderToStaticMarkup(<SessionRow row={FIXTURES.find((f) => f.id === id)!} density="comfortable" />);
  it("marks sessions waiting on the user as blocked: needs-you", () => {
    expect(html("needs-permission")).toContain('data-blocked="needs-you"');
    expect(html("needs-question")).toContain('data-blocked="needs-you"');
  });
  it("marks a stopped session as blocked: error", () => {
    expect(html("error")).toContain('data-blocked="error"');
  });
  it("leaves every other state alone", () => {
    for (const id of ["idle", "working", "done-unseen", "done-seen", "stuck", "ended", "limited"]) expect(html(id), id).not.toContain("data-blocked");
  });
  it("keeps a muted session that needs you marked, because it still needs you", () => {
    expect(html("muted-needs-you")).toContain('data-blocked="needs-you"');
  });
});

describe("SessionRow (working shows as a moving logo)", () => {
  const html = (id: string) => renderToStaticMarkup(<SessionRow row={FIXTURES.find((f) => f.id === id)!} density="comfortable" />);
  it("animates the vendor mark and drops the glyph from the cluster", () => {
    const h = html("working");
    expect(h).toContain("data-working");
    expect(h.slice(h.indexOf("session-row__cluster"))).not.toContain('data-kind="working"');
  });
  it("still says Working to a screen reader", () => {
    expect(html("working")).toContain("Working");
  });
  it("leaves the mark still on every other state", () => {
    for (const id of ["idle", "needs-permission", "done-unseen", "error", "ended"]) expect(html(id), id).not.toContain("data-working");
  });
  it("keeps the other states' glyph in the cluster", () => {
    const h = html("needs-permission");
    expect(h.slice(h.indexOf("session-row__cluster"))).toContain('data-kind="needs-permission"');
  });
});

describe("SessionRow (unknown values are not shown)", () => {
  const unknown = FIXTURES.find((f) => f.id === "unknown")!;
  const expanded = (row: typeof unknown) => renderToStaticMarkup(<SessionRow row={row} density="expanded" />);
  it("shows no dash for a model, effort or subagent count that is not known", () => {
    const h = expanded(unknown);
    expect(h).not.toContain(">–<");
    expect(h).not.toContain("subagents");
  });
  it("shows no details line at all when nothing about the session is known", () => {
    expect(expanded({ ...unknown, mode: undefined })).not.toContain("session-row__meta");
  });
  it("still shows what is known, with nothing in place of the rest", () => {
    const h = expanded({ ...unknown, model: "Opus 5.5", subagents: 2 });
    expect(h).toContain("Opus 5.5");
    expect(h).toContain("2 subagents");
    expect(h).not.toContain(">–<");
  });
});
