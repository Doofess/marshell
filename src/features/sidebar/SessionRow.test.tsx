import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "./fixtures";
import { SessionRow } from "./SessionRow";

describe("SessionRow", () => {
  it("has one fixture per glyph state (and the manual CLI's running) plus the modifiers", () => {
    const kinds = new Set(FIXTURES.map((f) => f.status));
    expect(kinds.size).toBe(12);
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
  it("draws no separator after the model when the phrase is empty", () => {
    const html = renderToStaticMarkup(<SessionRow row={{ ...working, status: "idle", phrase: "", model: "Opus 5.5", subagents: undefined }} density="comfortable" />);
    const line2 = html.slice(html.indexOf("session-row__phrase"));
    expect(line2).toContain("Opus 5.5");
    expect(line2).not.toContain("session-row__sep");
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

describe("SessionRow (subagent count)", () => {
  const row = { ...FIXTURES.find((f) => f.id === "needs-permission")!, model: "Opus 5.5", phrase: "Thinking", muted: false };
  const text = (h: string) => h.replace(/<[^>]+>/g, "");
  const phraseLine = (h: string) => text(h.slice(h.indexOf("session-row__phrase"), h.indexOf("</div>", h.indexOf("session-row__phrase"))));
  // docs/PLAN.md "Sidebar rows": the second line reads like "Editing src/auth.ts · 2 subagents".
  it("follows the phrase on the second line of a comfortable row", () => {
    const h = renderToStaticMarkup(<SessionRow row={{ ...row, subagents: 2 }} density="comfortable" />);
    expect(phraseLine(h)).toContain("Opus 5.5 · Thinking · 2 subagents");
  });
  it("says one subagent in the singular", () => {
    const h = renderToStaticMarkup(<SessionRow row={{ ...row, subagents: 1 }} density="comfortable" />);
    expect(text(h)).toContain("1 subagent");
    expect(text(h)).not.toContain("1 subagents");
  });
  it("shows once in an expanded row, on its meta line", () => {
    const h = renderToStaticMarkup(<SessionRow row={{ ...row, subagents: 2 }} density="expanded" />);
    expect(text(h).split("2 subagents").length - 1).toBe(1);
    expect(phraseLine(h)).not.toContain("subagents");
  });
});

describe("SessionRow (words stay separate for assistive technology)", () => {
  const row = FIXTURES.find((f) => f.id === "needs-permission")!;
  const h = renderToStaticMarkup(<SessionRow row={{ ...row, model: "Opus 5.5", effort: "high", subagents: 2, recap: { text: "Fixed it", ageMs: 60_000 }, usage: { inContext: 1000, window: 2000, tokensIn: 5, tokensOut: 6, costUsd: 0.1 } }} density="expanded" />);
  it("puts a space between the text spans of the first line, the phrase line and the details, so they are not read as one run", () => {
    expect(h).toMatch(/session-row__name"[^>]*>[^<]*<\/span> <span class="session-row__project"/);
    expect(h).toMatch(/session-row__model">[^<]*<\/span> <span class="session-row__sep">/);
    expect(h).toMatch(/<\/span> <span class="session-row__status-text">/);
    expect(h).toMatch(/<span>Opus 5\.5<\/span> <span>high<\/span> <span>2 subagents<\/span>/);
    expect(h).toMatch(/<\/div> <p class="session-row__recap">/);
    expect(h).toMatch(/<\/p> <div class="session-row__usage">/);
    expect(h).toMatch(/<\/div> <div class="session-row__details"/);
  });
  it("does not split a branch name in two", () => {
    expect(h).toMatch(/session-row__branch-head"[^>]*>[^<]*<\/span><span class="session-row__branch-tail"/);
  });
});

describe("SessionRow: a task", () => {
  const base = FIXTURES.find((f) => f.id === "working")!;
  const task = { ...base, task: true };
  it("shows a Task tag after the project name, as text, with the branch in the title and the accessible name", () => {
    const html = renderToStaticMarkup(<SessionRow row={task} density="comfortable" />);
    expect(html).toMatch(/session-row__project"[^>]*>my-app<\/span> <span class="session-row__task"[^>]*>Task<\/span>/);
    expect(html).toMatch(/title="[^"]*feat\/auth-flow/);
    expect(html).toMatch(/aria-label="[^"]*my-app Task · feat\/auth-flow/);
  });
  it("has a Review changes button, reachable by keyboard with the row's tab stop", () => {
    const onRow = renderToStaticMarkup(<SessionRow row={task} density="comfortable" tabStop onReview={() => {}} />);
    expect(onRow).toMatch(/<button[^>]*class="session-row__review"[^>]*tabindex="0"[^>]*>Review changes<\/button>/);
    const off = renderToStaticMarkup(<SessionRow row={task} density="comfortable" onReview={() => {}} />);
    expect(off).toMatch(/<button[^>]*tabindex="-1"[^>]*>Review changes<\/button>/);
  });
  it("shows neither the tag nor the button on a row that is not a task, or where nothing can open a review", () => {
    const plain = renderToStaticMarkup(<SessionRow row={base} density="comfortable" onReview={() => {}} />);
    expect(plain).not.toContain("session-row__task");
    expect(plain).not.toContain("Review changes");
    expect(renderToStaticMarkup(<SessionRow row={task} density="comfortable" />)).not.toContain("Review changes");
  });
  it("marks a row that has the button, so its text under line 1 keeps clear of it (open when selected)", () => {
    expect(renderToStaticMarkup(<SessionRow row={task} density="comfortable" onReview={() => {}} />)).toMatch(/data-review=""/);
    expect(renderToStaticMarkup(<SessionRow row={task} density="comfortable" selected onReview={() => {}} />)).toMatch(/data-review="open"/);
    expect(renderToStaticMarkup(<SessionRow row={task} density="comfortable" />)).not.toContain("data-review");
    expect(renderToStaticMarkup(<SessionRow row={base} density="comfortable" onReview={() => {}} />)).not.toContain("data-review");
  });
  it("leaves the button out while the name is being edited", () => {
    expect(renderToStaticMarkup(<SessionRow row={task} density="comfortable" tabStop onReview={() => {}} rename={{ onCommit: () => {}, onCancel: () => {} }} />)).not.toContain("Review changes");
  });
  describe("the set up button", () => {
    const limited = { ...base, agent: "claude" as const, status: "limited" as const, phrase: "Status limited", setupLink: true };
    it("is a button named for the row whose visible text is set up, reachable by Tab with the row's tab stop", () => {
      const on = renderToStaticMarkup(<SessionRow row={limited} density="comfortable" tabStop onSetup={() => {}} />);
      expect(on).toMatch(/<button[^>]*class="session-row__setup"[^>]*aria-label="Set up status for [^"]+"[^>]*tabindex="0"[^>]*>set up<\/button>/);
      const off = renderToStaticMarkup(<SessionRow row={limited} density="comfortable" onSetup={() => {}} />);
      expect(off).toMatch(/<button[^>]*session-row__setup[^>]*tabindex="-1"/);
    });
    it("is not rendered without onSetup, on a row with no link, or while renaming", () => {
      expect(renderToStaticMarkup(<SessionRow row={limited} density="comfortable" />)).not.toContain("session-row__setup");
      expect(renderToStaticMarkup(<SessionRow row={base} density="comfortable" onSetup={() => {}} />)).not.toContain("session-row__setup");
      expect(renderToStaticMarkup(<SessionRow row={limited} density="comfortable" onSetup={() => {}} rename={{ onCommit: () => {}, onCancel: () => {} }} />)).not.toContain("session-row__setup");
    });
  });
});

describe("SessionRow: the setup note", () => {
  const row = { ...FIXTURES[0]!, setupNote: { text: "Setup failed (exit 1).", tabId: "t-s" } };
  it("shows the note text and no button: the output opens from the row menu", () => {
    const html = renderToStaticMarkup(<SessionRow row={row} density="comfortable" tabStop />);
    expect(html).toContain("Setup failed (exit 1).");
    expect(html).not.toContain("Open its output");
    expect(html).not.toContain("session-row__setup-output");
    expect(html).not.toContain("data-setup-output");
  });
  it("shows nothing for a row with no note", () => {
    const html = renderToStaticMarkup(<SessionRow row={FIXTURES[0]!} density="comfortable" />);
    expect(html).not.toContain("Setup ");
    expect(html).not.toContain("Open its output");
  });
});
