import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { SCENARIO_ROWS_WITH_TOOLS } from "../window/scenario";
import { boardHeight, boardRowLabel, boardRows, CARD_PX, fitRows, GROUP_PX, HEAD_PX, MORE_PX, ROW_PX, splitGroups, statusText, waitingText } from "./board";

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
    // One row of the room is the "+N more" line, so five rows' worth of room shows four.
    expect(f.others).toHaveLength(4);
    expect(f.hidden).toBe(8);
  });
  it("counts the '+N more' row: twelve sessions in 520 px stay inside 520 px", () => {
    const g = splitGroups(twelve);
    expect(boardHeight(fitRows(520, g), false)).toBeLessThanOrEqual(520);
    expect(fitRows(520, g).hidden).toBeGreaterThan(0);
  });
  it("never renders taller than the height it was given, at any height that has room for a row", () => {
    const gateRow = FIXTURES.find((f) => f.id === "needs-permission")!;
    const mixed = splitGroups(boardRows([{ ...gateRow, id: "g", number: 99, waitingMs: 60_000 }, ...twelve.map((t) => ({ ...FIXTURES[2]!, status: "working" as const, waitingMs: undefined, id: t.id, number: t.number }))]));
    for (let h = HEAD_PX + 2 * GROUP_PX + MORE_PX + ROW_PX; h <= 1400; h += 7) expect(boardHeight(fitRows(h, mixed), false), `height ${h}`).toBeLessThanOrEqual(h);
  });
  it("counts an open inline request, which is taller than a row", () => {
    const g = splitGroups(twelve);
    const withCard = fitRows(700, g, true);
    expect(boardHeight(withCard, true)).toBeLessThanOrEqual(700);
    expect(withCard.others.length).toBeLessThan(fitRows(700, g).others.length);
    expect(CARD_PX).toBeGreaterThan(ROW_PX);
  });
  it("keeps gate calls before anything else", () => {
    const gateRow = FIXTURES.find((f) => f.id === "needs-permission")!;
    const rows = boardRows([{ ...gateRow, id: "g", number: 99 }, ...twelve.map((t) => ({ ...FIXTURES[2]!, status: "working" as const, waitingMs: undefined, id: t.id, number: t.number }))]);
    const f = fitRows(HEAD_PX + 2 * GROUP_PX + 3 * ROW_PX, splitGroups(rows));
    expect(f.gate.map((r) => r.id)).toEqual(["g"]);
    // Three rows of room, one of them the "+N more" line.
    expect(f.others).toHaveLength(1);
  });
  it("shows everything when there is room, and nothing hidden for zero sessions", () => {
    expect(fitRows(2000, splitGroups(twelve)).hidden).toBe(0);
    expect(fitRows(2000, { gate: [], others: [] })).toEqual({ gate: [], others: [], hidden: 0 });
  });
});
