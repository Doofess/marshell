import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import type { RowModel } from "../sidebar/types";
import { SCENARIO_PLAN, SCENARIO_PORTS } from "../window/scenario";
import { PRIORITY, fitItems, itemWidth, statusItems, stripWidth, type PlanUsage } from "./stripItems";

const row = (id: string): RowModel => FIXTURES.find((f) => f.id === id)!;
const full = statusItems(row("working"), SCENARIO_PLAN, SCENARIO_PORTS);
const ids = (items: { id: string }[]) => items.map((i) => i.id);

describe("statusItems: what the strip says, from known data only", () => {
  it("reads left to right like the user's status line: model, context, the two limits, cost, time, changes, health, version, skills, venv, ports (a core item always comes first, so a separator never leads the line)", () => {
    expect(ids(full)).toEqual(["model", "ctx", "limit5", "limit7", "cost", "time", "dirty", "mcp", "version", "skills", "venv", "ports"]);
  });
  it("leaves out whatever is unknown, and never shows a dash", () => {
    const bare = statusItems({ id: "x", number: 1, name: "x", project: "p", projectKey: "p", agent: "claude", status: "idle", phrase: "Idle" }, undefined, []);
    expect(bare).toEqual([]);
    const noModel = statusItems({ ...row("working"), model: undefined, effort: undefined, usage: undefined, contextPct: undefined }, undefined, []);
    for (const gone of ["model", "ctx", "cost", "limit5", "limit7", "ports"]) expect(ids(noModel)).not.toContain(gone);
    for (const i of full) expect(`${i.value} ${i.detail ?? ""}`).not.toMatch(/–|--|NaN|undefined/);
  });
  it("shows the effort with the model, and the effort alone is never guessed", () => {
    const model = full.find((i) => i.id === "model")!;
    expect(model.value).toBe("Opus 5.5");
    expect(model.detail).toBe("· high");
    expect(statusItems({ ...row("working"), effort: undefined }, undefined, []).find((i) => i.id === "model")!.detail).toBeUndefined();
  });
  it("shows the context as a percentage, a bar and used over size", () => {
    const ctx = full.find((i) => i.id === "ctx")!;
    expect(ctx).toMatchObject({ label: "ctx", value: "42%", pct: 42, bar: true, detail: "84k/200k" });
    expect(ctx.name).toBe("Context 42% used, 84k of 200k tokens");
  });
  it("shows each plan limit with when it resets, and says so in its name", () => {
    const five = full.find((i) => i.id === "limit5")!;
    expect(five).toMatchObject({ label: "5h", value: "38%", pct: 38, bar: true, detail: "resets in 2h 10m" });
    expect(five.name).toBe("5-hour limit 38% used, resets in 2h 10m");
    const seven = full.find((i) => i.id === "limit7")!;
    expect(seven).toMatchObject({ label: "7d", value: "12%", detail: "resets in 3d 4h" });
    expect(seven.name).toBe("7-day limit 12% used, resets in 3d 4h");
  });
  it("leaves the reset out when it is unknown, and the limit out when its percentage is", () => {
    const plan: PlanUsage = { fiveHour: { pct: 50 }, sevenDay: undefined };
    const items = statusItems(row("working"), plan, []);
    expect(items.find((i) => i.id === "limit5")!.detail).toBeUndefined();
    expect(ids(items)).not.toContain("limit7");
  });
  it("shows cost, session time, skills, venv, version, uncommitted changes and ports as plain text", () => {
    const by = (id: string) => full.find((i) => i.id === id)!;
    expect(by("cost").value).toBe("$1.20");
    expect(by("time").value).toBe("1h 05m");
    expect(by("skills")).toMatchObject({ label: "skills", value: "14" });
    expect(by("venv")).toMatchObject({ label: "venv", value: ".venv" });
    expect(by("version").value).toBe("v2.1.296");
    expect(by("dirty").value).toBe("~3 changed");
    expect(by("ports").value).toBe(":3000 :5173");
  });
  it("reads the ports from the row's chips when no list is given", () => {
    const ports = [3000, 5173].map((port) => ({ port, url: `http://localhost:${port}/`, announced: false }));
    const item = statusItems({ ...row("working"), ports }, undefined).find((i) => i.id === "ports")!;
    expect(item.value).toBe(":3000 :5173");
    expect(item.name).toBe("Listening on port 3000, port 5173");
    expect(ids(statusItems(row("working"), undefined))).not.toContain("ports");
  });
  it("leaves out the changes when nothing changed, and skills when there are none", () => {
    const items = statusItems({ ...row("working"), dirty: 0, skills: 0 }, SCENARIO_PLAN, []);
    expect(ids(items)).not.toContain("dirty");
    expect(ids(items)).not.toContain("skills");
  });
  it("shows healthy MCP servers as a count, and a problem by name, in place of the count", () => {
    const ok = full.find((i) => i.id === "mcp")!;
    expect(ok).toMatchObject({ kind: "ok", label: "mcp", value: "3" });
    expect(ok.name).toBe("MCP servers: 3 connected");
    const bad = statusItems({ ...row("working"), mcp: { connected: 2, total: 3, problem: "discord" } }, undefined, []).find((i) => i.id === "mcp")!;
    expect(bad).toMatchObject({ kind: "problem", value: "discord", priority: PRIORITY.mcpProblem });
    expect(bad.name).toBe("MCP problem: discord, 2 of 3 servers connected");
    expect(bad.label).toBeUndefined();
  });
  it("shows a partial MCP connection without a named problem as a plain count of the total", () => {
    const partial = statusItems({ ...row("working"), mcp: { connected: 2, total: 3 } }, undefined, []).find((i) => i.id === "mcp")!;
    expect(partial).toMatchObject({ kind: "ok", value: "2 of 3" });
  });
  it("tones a meter from the same thresholds as the context ring: neutral to 69, caution from 70, error from 90", () => {
    const tone = (pct: number) => statusItems({ ...row("working"), contextPct: pct, usage: undefined }, undefined, []).find((i) => i.id === "ctx")!.tone;
    expect([tone(0), tone(69.9), tone(70), tone(89.9), tone(90), tone(100)]).toEqual(["neutral", "neutral", "caution", "caution", "error", "error"]);
  });
  it("clamps a nonsense percentage instead of drawing it", () => {
    const ctx = statusItems({ ...row("working"), contextPct: 140, usage: undefined }, undefined, []).find((i) => i.id === "ctx")!;
    expect(ctx.pct).toBe(100);
    expect(ctx.value).toBe("100%");
    expect(statusItems({ ...row("working"), contextPct: Number.NaN, usage: undefined }, undefined, []).map((i) => i.id)).not.toContain("ctx");
  });
  it("ranks the items so a problem outranks everything and the three core ones outlast the rest", () => {
    expect(PRIORITY.mcpProblem).toBeGreaterThan(PRIORITY.ctx);
    const order = [PRIORITY.ctx, PRIORITY.limit5, PRIORITY.model, PRIORITY.limit7, PRIORITY.cost, PRIORITY.time, PRIORITY.mcpOk, PRIORITY.ports, PRIORITY.skills, PRIORITY.dirty, PRIORITY.version, PRIORITY.venv];
    expect([...order].sort((a, b) => b - a)).toEqual(order);
    expect(new Set(order).size).toBe(order.length);
  });
});

describe("fitItems: it never wraps, it drops the least important first", () => {
  it("keeps everything when there is room", () => {
    expect(fitItems(full, 1000)).toEqual(full.map((i) => ({ ...i, compact: 0 })));
    expect(fitItems(full, Infinity).length).toBe(full.length);
  });
  it("is never wider than the room, for every width down to the floor", () => {
    const floor = stripWidth(fitItems(full, 0));
    for (let w = floor; w <= stripWidth(fitItems(full, Infinity)); w++) expect(stripWidth(fitItems(full, w)), `width ${w}`).toBeLessThanOrEqual(w);
  });
  it("drops venv first, then version, uncommitted changes, skills, ports, healthy MCP, time and cost", () => {
    const dropped: string[] = [];
    let kept = ids(full);
    for (let w = stripWidth(fitItems(full, Infinity)); w >= 0; w--) {
      const now = ids(fitItems(full, w));
      for (const id of kept) if (!now.includes(id) && !dropped.includes(id)) dropped.push(id);
      kept = now;
    }
    expect(dropped.slice(0, 8)).toEqual(["venv", "version", "dirty", "skills", "ports", "mcp", "time", "cost"]);
  });
  it("shortens a meter (detail, then bar) before it drops it, and keeps the three core items to the end", () => {
    const tight = fitItems(full, 0);
    expect(ids(tight)).toEqual(["model", "ctx", "limit5"]);
    expect(tight.every((i) => i.compact === (i.detail || i.bar ? maxCompactOf(i) : 0))).toBe(true);
  });
  it("keeps the original left-to-right order at every width", () => {
    const canonical = ids(full);
    for (const w of [0, 30, 45, 60, 80, 100, 130, 160, 1000]) {
      const got = ids(fitItems(full, w));
      expect(got, String(w)).toEqual(canonical.filter((id) => got.includes(id)));
    }
  });
  it("keeps an MCP problem to the very end, before even the context", () => {
    const sick = statusItems({ ...row("working"), mcp: { connected: 2, total: 3, problem: "discord" } }, SCENARIO_PLAN, SCENARIO_PORTS);
    expect(ids(fitItems(sick, 0))).toContain("mcp");
    const mid = ids(fitItems(sick, 60));
    expect(mid).toContain("mcp");
    expect(mid).not.toContain("venv");
  });
  it("measures an item as its label, value, bar and detail, and a separator between items", () => {
    const ctx = full.find((i) => i.id === "ctx")!;
    expect(itemWidth(ctx, 0)).toBe("ctx 42%".length + 4 + 1 + "84k/200k".length);
    expect(itemWidth(ctx, 1)).toBe("ctx 42%".length + 4);
    expect(itemWidth(ctx, 2)).toBe("ctx 42%".length);
    expect(stripWidth([{ ...ctx, compact: 2 }, { ...ctx, compact: 2 }])).toBe(2 * "ctx 42%".length + 3);
  });
  it("copes with no items and a nonsense width", () => {
    expect(fitItems([], 10)).toEqual([]);
    expect(fitItems(full, Number.NaN).length).toBeGreaterThan(0);
    expect(fitItems(full, -5).length).toBeGreaterThan(0);
  });
});

function maxCompactOf(i: { detail?: string; bar?: boolean }): number {
  return (i.detail ? 1 : 0) + (i.bar ? 1 : 0);
}
