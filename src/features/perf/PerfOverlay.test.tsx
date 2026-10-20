import { useEffect } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { perf } from "../../lib/api/perf";
import { LatencyMeter } from "./latency";
import { PerfOverlay } from "./PerfOverlay";

// Collect effect setup functions, then run them explicitly: no DOM is needed for polling.
vi.mock("react", async (original) => ({ ...await original<typeof import("react")>(), useEffect: vi.fn() }));

describe("PerfOverlay visibility", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(useEffect).mockClear();
  });
  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it("starts no polling or latency timer for a hidden overlay", async () => {
    const poll = vi.spyOn(perf, "snapshot").mockRejectedValue(new Error("offline"));
    renderToStaticMarkup(<PerfOverlay meter={new LatencyMeter()} renderer="dom" endpoint={{ port: 1, token: "t", platform: "windows" }} tabId="a" visible={false} />);
    const cleanups = vi.mocked(useEffect).mock.calls.map(([setup]) => setup());
    try {
      await vi.advanceTimersByTimeAsync(2000);
      expect(poll).not.toHaveBeenCalled();
      expect(vi.getTimerCount()).toBe(0);
    } finally {
      cleanups.forEach((cleanup) => cleanup?.());
    }
  });
});
