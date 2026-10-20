import { Fragment, useEffect, useState } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import type { PerfSnapshot } from "../../generated/PerfSnapshot";
import { perf } from "../../lib/api/perf";
import type { RendererKind } from "../terminal/renderer";
import type { LatencyMeter } from "./latency";
import { perfLines } from "./perfLines";

/** Hidden panel for the performance gate. Toggle: Ctrl+Shift+Alt+P (Cmd+Shift+Alt+P on macOS). */
export function PerfOverlay({ meter, renderer, endpoint, tabId }: { meter: LatencyMeter; renderer: RendererKind; endpoint: Endpoint; tabId: string }) {
  const [stats, setStats] = useState(() => meter.stats());
  const [snapshot, setSnapshot] = useState<PerfSnapshot | null>(null);
  useEffect(() => {
    const id = setInterval(() => setStats(meter.stats()), 500);
    return () => clearInterval(id);
  }, [meter]);
  useEffect(() => {
    let stopped = false;
    let inFlight = false;
    const poll = async () => {
      if (inFlight) return;
      inFlight = true;
      try {
        const next = await perf.snapshot(endpoint);
        if (!stopped) setSnapshot(next);
      } catch {
        // Keep the last snapshot.
      } finally {
        inFlight = false;
      }
    };
    void poll();
    const id = setInterval(() => void poll(), 1000);
    return () => {
      stopped = true;
      clearInterval(id);
    };
  }, [endpoint]);
  return (
    <output className="perf-overlay" aria-live="off">
      {/* <output> holds phrasing content only, so the lines are split with <br>. */}
      {perfLines(snapshot, tabId, stats, renderer).map((line, i) => (
        <Fragment key={line.split(" ")[0]}>
          {i > 0 && <br />}
          {line}
        </Fragment>
      ))}
    </output>
  );
}
