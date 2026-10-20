import type { PerfSnapshot } from "../../generated/PerfSnapshot";

const DASH = "–";
const mb = (bytes: number | null) => (bytes === null ? DASH : String(Math.round(bytes / 1_048_576)));
const cpu = (percent: number | null) => (percent === null ? DASH : percent.toFixed(1));

/** The sum of the values that are known; null only when none is. */
function sum(values: (number | null)[]): number | null {
  const known = values.filter((v): v is number => v !== null);
  return known.length === 0 ? null : known.reduce((a, b) => a + b, 0);
}

/** The lines of the perf overlay: typing latency, then startup and resource use. */
export function perfLines(
  snapshot: PerfSnapshot | null,
  tabId: string,
  latency: { p50: number; p95: number; n: number },
  renderer: string,
): string[] {
  const lines = [`${renderer} · typing p50 ${latency.p50.toFixed(1)} ms · p95 ${latency.p95.toFixed(1)} ms · n ${latency.n}`];
  if (snapshot === null) return [...lines, `startup ${DASH} ms`];
  lines.push(`startup ${snapshot.startup_ms === null ? DASH : Math.round(snapshot.startup_ms)} ms`);
  lines.push(`core ${mb(snapshot.core.memory_bytes)} MB · cpu ${cpu(snapshot.core.cpu_percent)}%`);
  const tab = snapshot.sessions.find((s) => s.tab_id === tabId);
  if (tab) lines.push(`this tab ${tab.usage.processes} processes · ${mb(tab.usage.memory_bytes)} MB · cpu ${cpu(tab.usage.cpu_percent)}%`);
  const all = snapshot.sessions;
  lines.push(`all tabs ${all.length} sessions · ${mb(sum(all.map((s) => s.usage.memory_bytes)))} MB · cpu ${cpu(sum(all.map((s) => s.usage.cpu_percent)))}%`);
  return lines;
}
