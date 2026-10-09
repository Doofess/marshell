import { useEffect, useState } from "react";
import type { RendererKind } from "../terminal/renderer";
import type { LatencyMeter } from "./latency";

/** Hidden panel for the performance gate. Toggle: Ctrl+Shift+Alt+P (Cmd+Shift+Alt+P on macOS). */
export function PerfOverlay({ meter, renderer }: { meter: LatencyMeter; renderer: RendererKind }) {
  const [stats, setStats] = useState(() => meter.stats());
  useEffect(() => {
    const id = setInterval(() => setStats(meter.stats()), 500);
    return () => clearInterval(id);
  }, [meter]);
  return (
    <output className="perf-overlay" aria-live="off">
      {renderer} · typing p50 {stats.p50.toFixed(1)} ms · p95 {stats.p95.toFixed(1)} ms · n {stats.n}
    </output>
  );
}
