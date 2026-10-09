import { describe, expect, it } from "vitest";
import { LatencyMeter } from "./latency";

describe("LatencyMeter", () => {
  it("reports percentiles of recorded samples", () => {
    const m = new LatencyMeter();
    for (let i = 1; i <= 100; i++) m.record(i);
    expect(m.stats()).toEqual({ p50: 50, p95: 95, n: 100 });
  });
  it("keeps only the last 200 samples", () => {
    const m = new LatencyMeter();
    for (let i = 0; i < 300; i++) m.record(1);
    expect(m.stats().n).toBe(200);
  });
  it("is empty before any input", () => {
    expect(new LatencyMeter().stats()).toEqual({ p50: 0, p95: 0, n: 0 });
  });
});
