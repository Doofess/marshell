import { describe, expect, it } from "vitest";
import type { PerfSnapshot } from "../../generated/PerfSnapshot";
import { perfLines } from "./perfLines";

const MB = 1_048_576;
const latency = { p50: 4.04, p95: 9.96, n: 12 };
const first = "webgl · typing p50 4.0 ms · p95 10.0 ms · n 12";

const snap = (over: Partial<PerfSnapshot> = {}): PerfSnapshot => ({
  startup_ms: 812,
  core: { processes: 1, memory_bytes: 50 * MB, cpu_percent: 1.25 },
  sessions: [
    { tab_id: "a", usage: { processes: 2, memory_bytes: 100 * MB, cpu_percent: 3.04 } },
    { tab_id: "b", usage: { processes: 1, memory_bytes: 20 * MB, cpu_percent: 0.5 } },
  ],
  sampled_at_ms: 1,
  ...over,
});

describe("perfLines", () => {
  it("shows a full snapshot", () => {
    expect(perfLines(snap(), "a", latency, "webgl")).toEqual([
      first,
      "startup 812 ms",
      "core 50 MB · cpu 1.3%",
      "this tab 2 processes · 100 MB · cpu 3.0%",
      "all tabs 2 sessions · 120 MB · cpu 3.5%",
    ]);
  });
  it("shows null values as a dash", () => {
    const s = snap({
      startup_ms: null,
      core: { processes: 1, memory_bytes: null, cpu_percent: null },
      sessions: [{ tab_id: "a", usage: { processes: 1, memory_bytes: null, cpu_percent: null } }],
    });
    expect(perfLines(s, "a", latency, "webgl")).toEqual([
      first,
      "startup – ms",
      "core – MB · cpu –%",
      "this tab 1 processes · – MB · cpu –%",
      "all tabs 1 sessions · – MB · cpu –%",
    ]);
  });
  it("counts a null as nothing in the sums", () => {
    const s = snap({
      sessions: [
        { tab_id: "a", usage: { processes: 1, memory_bytes: null, cpu_percent: 2 } },
        { tab_id: "b", usage: { processes: 1, memory_bytes: 10 * MB, cpu_percent: null } },
      ],
    });
    expect(perfLines(s, "a", latency, "webgl")[4]).toBe("all tabs 2 sessions · 10 MB · cpu 2.0%");
  });
  it("leaves out this tab when it has no session", () => {
    const lines = perfLines(snap(), "zzz", latency, "webgl");
    expect(lines).toHaveLength(4);
    expect(lines.some((l) => l.startsWith("this tab"))).toBe(false);
  });
  it("shows only the first line and an unknown startup without a snapshot", () => {
    expect(perfLines(null, "a", latency, "webgl")).toEqual([first, "startup – ms"]);
  });
});
