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
    const gateRow = FIXTURES.find((f) => f.id === "needs-permission")!;
    const rows = boardRows([{ ...gateRow, id: "g", number: 99 }, ...twelve.map((t) => ({ ...FIXTURES[2]!, status: "working" as const, waitingMs: undefined, id: t.id, number: t.number }))]);
    const f = fitRows(HEAD_PX + 2 * GROUP_PX + 3 * ROW_PX, splitGroups(rows));
    expect(f.gate.map((r) => r.id)).toEqual(["g"]);
    expect(f.others).toHaveLength(2);
  });
  it("shows everything when there is room, and nothing hidden for zero sessions", () => {
    expect(fitRows(2000, splitGroups(twelve)).hidden).toBe(0);
    expect(fitRows(2000, { gate: [], others: [] })).toEqual({ gate: [], others: [], hidden: 0 });
  });
});
