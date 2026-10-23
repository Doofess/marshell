import { isValidElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { labelOf, nameContainsLabel } from "../../lib/labelInName";
import { SCENARIO_ROWS_WITH_TOOLS } from "../window/scenario";
import { FIXTURES } from "../sidebar/fixtures";
import { DeparturesBoard } from "./DeparturesBoard";
import { LiveFlipCell } from "./LiveFlipCell";
import { heldRows } from "./liveBoard";

const html = (o: Partial<Parameters<typeof DeparturesBoard>[0]> = {}) => renderToStaticMarkup(<DeparturesBoard rows={SCENARIO_ROWS_WITH_TOOLS} heightPx={800} {...o} />);

it("marks only stale rows as disabled", () => {
  const id = SCENARIO_ROWS_WITH_TOOLS[0]!.id;
  const tags = html({ stale: [id] }).match(/<tr[^>]*data-row-id="[^"]*"[^>]*>/g)!;
  const stale = tags.find((tag) => tag.includes(`data-row-id="${id}"`))!;
  expect(stale).toContain('aria-disabled="true"');
  for (const tag of tags.filter((tag) => tag !== stale)) expect(tag).not.toContain("aria-disabled");
});

describe("LiveFlipCell", () => {
  it.each([false, true])("shows the final text without a flip on mount (reduced: %s)", (reduced) => {
    const h = renderToStaticMarkup(<LiveFlipCell text="NEEDS YOU" reduced={reduced} />);
    expect(h).toMatch(/<span class="flip" data-live="true">/);
    expect(h).toContain('<span aria-hidden="true">NEEDS YOU</span>');
    expect(h).toContain('<span class="board__sr">NEEDS YOU</span>');
    expect(h).not.toMatch(/data-frames|data-fade|aria-live|role="status"|role="alert"/);
  });
});

describe("DeparturesBoard", () => {
  it("renders the live card instead of the fixture command", () => {
    const gate = SCENARIO_ROWS_WITH_TOOLS.find((r) => r.status === "needs-permission")!;
    const h = html({ card: gate.id, renderCard: () => <span>Real request</span> });
    expect(h).toContain("Real request");
    expect(h).not.toContain("npm test");
  });
  it("keeps a row's element when its card opens, so the focus on it survives the card closing", () => {
    // A row with its card must not come back as a nested [row, card] array: React would remount the row and drop its focus.
    const gate = SCENARIO_ROWS_WITH_TOOLS.find((r) => r.status === "needs-permission")!;
    const tree = DeparturesBoard({ rows: SCENARIO_ROWS_WITH_TOOLS, heightPx: 800, card: gate.id, renderCard: () => <span>Real request</span> });
    const bodies: ReactElement<{ children: unknown }>[] = [];
    const walk = (node: unknown) => {
      if (Array.isArray(node)) return node.forEach(walk);
      if (!isValidElement<{ children?: unknown }>(node)) return;
      if (node.type === "tbody") bodies.push(node as ReactElement<{ children: unknown }>);
      walk(node.props.children);
    };
    walk(tree);
    expect(bodies.length).toBeGreaterThan(0);
    for (const body of bodies) for (const child of [body.props.children].flat(2)) expect(Array.isArray(child)).toBe(false);
  });
  it("does not render or reserve room for a card whose renderer returns null", () => {
    const gate = SCENARIO_ROWS_WITH_TOOLS.find((r) => r.status === "needs-permission")!;
    const h = html({ card: gate.id, renderCard: () => null, heightPx: 400 });
    expect(h).not.toContain('class="board__card"');
    expect(h.match(/class="board__row/g)?.length).toBe(html({ heightPx: 400 }).match(/class="board__row/g)?.length);
  });
  it("keeps a live request visible when opening it leaves less room for rows", () => {
    const rows = Array.from({ length: 6 }, (_, i) => ({ ...FIXTURES[2]!, id: `s${i}`, number: i + 1, status: "needs-permission" as const }));
    const h = html({ rows, heightPx: 440, card: "s5", renderCard: () => <span>Last request</span> });
    expect(h).toContain("Last request");
    expect(h).toContain('data-row-id="s5"');
    expect(h).toContain("+4 more sessions");
  });
  it("renders a closed row from its kept copy, dimmed", () => {
    const kept = new Map(SCENARIO_ROWS_WITH_TOOLS.map((r) => [r.id, r]));
    const stale = SCENARIO_ROWS_WITH_TOOLS[1]!;
    const rows = heldRows(SCENARIO_ROWS_WITH_TOOLS.filter((r) => r.id !== stale.id), kept, [stale.id]);
    const h = html({ rows, stale: [stale.id] });
    expect(h).toMatch(new RegExp(`data-stale="true"[^>]*title="Session ${stale.number}, `));
  });
  it("keeps a newly waiting row outside Gate calls with its move note while held", () => {
    const working = SCENARIO_ROWS_WITH_TOOLS.find((r) => r.status === "working")!;
    const rows = SCENARIO_ROWS_WITH_TOOLS.map((r) => r.id === working.id ? { ...r, status: "needs-permission" as const } : r);
    const hold = { order: SCENARIO_ROWS_WITH_TOOLS.map((r) => r.id), gate: SCENARIO_ROWS_WITH_TOOLS.filter((r) => r.status === "needs-permission" || r.status === "needs-question").map((r) => r.id) };
    const h = html({ rows, hold });
    const held = h.match(/<tr class="board__row[^>]*>[\s\S]*?<\/tr>/g)!.find((r) => r.includes(`title="Session ${working.number}, `))!;
    expect(h).toContain("Gate calls · 2");
    expect(held).toContain('data-gate="false"');
    expect(held).toContain("Moves up when you move away");
  });
  it("uses live flipping cells for status and waiting only", () => {
    const h = html({ live: true });
    expect(h.match(/data-live="true"/g)).toHaveLength(SCENARIO_ROWS_WITH_TOOLS.length * 2);
  });
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
  it("names every row from its visible text, which carries the session number (read as Session N)", () => {
    const rows = html().match(/<tr class="board__row[^"]*"[\s\S]*?<\/tr>/g) ?? [];
    expect(rows.length).toBeGreaterThan(5);
    for (const row of rows) {
      // Named from its content: an aria-label cannot both start with the session and contain the text in column order.
      expect(labelOf(row)).toBeNull();
      expect(nameContainsLabel(row)).toBe(true);
      expect(row).toMatch(/<span class="board__sr">Session <\/span>/);
      expect(row).toMatch(/title="Session \d+, /);
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
  it("plays a whole clip on a cell from a track of frames, and shows the track's last text", () => {
    const h = html({ tracks: { working: { status: [{ t: 0, text: "WORKING" }, { t: 1000, text: "NEEDS YOU" }] } } });
    expect(h).toMatch(/data-frames="\[\{[^"]*NEEDS YOU[^"]*"[^>]*data-at="0"/);
    expect(h).toMatch(/>NEEDS YOU<\/span>/);
  });
  it("leaves a cell with no track and no flip as plain text", () => {
    expect(html().match(/class="flip"/g)).toBeNull();
  });
  it("can show one frozen moment of the clip: the given texts replace the row's own", () => {
    const h = html({ texts: { working: { status: "NEEDS YOU", waiting: "<1 MIN" } } });
    const row = (h.match(/<tr class="board__row[^"]*"[^>]*data-agent="claude"[\s\S]*?<\/tr>/g) ?? []).find((r) => r.includes("NEEDS YOU") && r.includes("&lt;1 MIN"));
    expect(row).toBeDefined();
  });
});

describe("DeparturesBoard: reachable by keyboard, with table semantics that survive the grid layout", () => {
  const tabStops = (h: string) => (h.match(/<tr class="board__row[^"]*"[^>]*tabindex="0"/g) ?? []).length;
  it("has exactly one tab stop, and it is the first row when no row is active", () => {
    const h = html();
    expect(tabStops(h)).toBe(1);
    const first = h.match(/<tr class="board__row[^"]*"[^>]*>/)![0];
    expect(first).toContain('tabindex="0"');
  });
  it("moves the single tab stop to the active row", () => {
    const h = html({ active: 2 });
    expect(tabStops(h)).toBe(1);
    expect(h.match(/<tr class="board__row[^"]*"[^>]*tabindex="0"/)![0]).toContain('aria-current="true"');
  });
  it("keeps one tab stop when the active index is past the shown rows: it clamps to the last row shown", () => {
    const h = html({ active: 99 });
    expect(tabStops(h)).toBe(1);
    const last = h.match(/<tr class="board__row[^"]*"[^>]*>/g)!.at(-1)!;
    expect(last).toContain('tabindex="0"');
    expect(last).toContain('aria-current="true"');
  });
  it("clamps to a row that is shown, never to one counted under +N more", () => {
    const twelve = Array.from({ length: 12 }, (_, i) => ({ ...FIXTURES[2]!, id: `s${i}`, number: i + 1 }));
    const h = renderToStaticMarkup(<DeparturesBoard rows={twelve} heightPx={400} active={11} />);
    expect(h).toMatch(/\+\d+ more sessions/);
    expect(tabStops(h)).toBe(1);
  });
  it("says what every part is with explicit roles, because the grid layout drops the table's own in some browsers", () => {
    const h = html();
    expect(h).toMatch(/<table[^>]*role="table"/);
    expect(h).toMatch(/<thead[^>]*role="rowgroup"/);
    expect(h).toMatch(/<th[^>]*role="columnheader"[^>]*scope="col"|<th[^>]*scope="col"[^>]*role="columnheader"/);
    expect(h).toMatch(/<tr class="board__row[^"]*"[^>]*role="row"/);
    expect(h).toMatch(/<td[^>]*role="cell"/);
  });
  it("gives each group its own row group with a row-group header, not a column-group one", () => {
    const h = html();
    expect(h).not.toContain('scope="colgroup"');
    const bodies = h.match(/<tbody[^>]*role="rowgroup"[^>]*>/g) ?? [];
    expect(bodies).toHaveLength(2);
    expect((h.match(/<th[^>]*scope="rowgroup"/g) ?? []).length).toBe(2);
  });
  it("puts the '+N more' line in a row group of its own", () => {
    const twelve = Array.from({ length: 12 }, (_, i) => ({ ...FIXTURES[2]!, id: `s${i}`, number: i + 1 }));
    const h = renderToStaticMarkup(<DeparturesBoard rows={twelve} heightPx={400} />);
    expect(h).toMatch(/<tbody[^>]*role="rowgroup"[^>]*><tr class="board__more" role="row">/);
  });
  it("fits twelve sessions in 520 px with the '+N more' line included", () => {
    const twelve = Array.from({ length: 12 }, (_, i) => ({ ...FIXTURES[2]!, id: `s${i}`, number: i + 1, status: "working" as const }));
    const h = renderToStaticMarkup(<DeparturesBoard rows={twelve} heightPx={520} />);
    const shown = (h.match(/<tr class="board__row/g) ?? []).length;
    expect(shown).toBeGreaterThan(0);
    expect(36 + 28 + shown * 44 + 44).toBeLessThanOrEqual(520);
  });
});

describe("DeparturesBoard: the gate-call header says which keys open a request", () => {
  it("shows the letters, from the key's own code when the layout is not known", () => {
    expect(html()).toContain("Y opens the request, N opens it on Deny");
  });
  it("shows the letter the user's own layout prints on the key (Z where a German keyboard has KeyY)", () => {
    const h = html({ layout: new Map([["KeyY", "z"], ["KeyN", "n"]]) });
    expect(h).toContain("Z opens the request, N opens it on Deny");
    expect(h).not.toContain("Y opens");
  });
  it("keeps the hint in the gate-call header, so it costs no row of the fixed height", () => {
    const h = html();
    expect(h).toMatch(/<th[^>]*scope="rowgroup"[^>]*>Gate calls · 2[\s\S]*?opens the request[\s\S]*?<\/th>/);
  });
  it("says nothing about the keys when no row needs you", () => {
    const calm = SCENARIO_ROWS_WITH_TOOLS.filter((r) => r.status !== "needs-permission" && r.status !== "needs-question");
    expect(renderToStaticMarkup(<DeparturesBoard rows={calm} heightPx={800} />)).not.toContain("opens the request");
  });
});

describe("DeparturesBoard: a row that flipped to NEEDS YOU while the board is held still", () => {
  it("says it moves up, so its status and its group do not seem to disagree", () => {
    const working = SCENARIO_ROWS_WITH_TOOLS.find((r) => r.status === "working")!;
    const h = html({ texts: { [working.id]: { status: "NEEDS YOU" } } });
    const row = (h.match(/<tr class="board__row[^"]*"[\s\S]*?<\/tr>/g) ?? []).find((r) => r.includes(`title="Session ${working.number}, `))!;
    expect(row).toContain("NEEDS YOU");
    expect(row).toContain("board__moves");
    expect(row).toContain("Moves up when you move away");
  });
  it("says nothing on a row that is already a gate call", () => {
    const gate = html().match(/<tr class="board__row[^"]*"[^>]*data-gate="true"[\s\S]*?<\/tr>/)![0];
    expect(gate).not.toContain("board__moves");
  });
});
