import { Fragment, useEffect, useMemo, useState } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import type { PerfSnapshot } from "../../generated/PerfSnapshot";
import { perf } from "../../lib/api/perf";
import type { RendererKind } from "../terminal/renderer";
import type { LatencyMeter } from "./latency";
import { perfLines } from "./perfLines";
import { createPerfPolling } from "./perfPolling";

/** Hidden panel for the performance gate. Toggle: Ctrl+Shift+Alt+P (Cmd+Shift+Alt+P on macOS). */
export function PerfOverlay({ meter, renderer, endpoint, tabId, visible }: { meter: LatencyMeter; renderer: RendererKind; endpoint: Endpoint; tabId: string; visible: boolean }) {
  const [stats, setStats] = useState(() => meter.stats());
  const [snapshot, setSnapshot] = useState<PerfSnapshot | null>(null);
  const polling = useMemo(() => createPerfPolling({ poll: () => perf.snapshot(endpoint), intervalMs: 1000, onSnapshot: setSnapshot }), [endpoint]);
  useEffect(() => {
    if (!visible) return;
    setStats(meter.stats());
    const id = setInterval(() => setStats(meter.stats()), 500);
    return () => clearInterval(id);
  }, [meter, visible]);
  useEffect(() => {
    polling.setVisible(visible);
    return polling.stop;
  }, [polling, visible]);
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
