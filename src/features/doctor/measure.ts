import { agoText } from "./cliDoctors";
import type { DoctorReport } from "./types";

/** The status line under the CLI doctors' line: when the MCP servers were last measured. */
export function measureLine(measuredAtMs: number | undefined, nowMs: number): string {
  if (measuredAtMs === undefined) return "MCP servers haven't been measured. Measuring starts each local server for a moment.";
  return `MCP servers were measured ${agoText(measuredAtMs, nowMs)} ago.`;
}

/** What the announcer says when a measuring ends: how many servers didn't start. */
export function measureDoneAnnouncement(report: DoctorReport): string {
  const n = report.items.filter((i) => i.id.startsWith("mcpfail.")).length;
  if (n === 0) return "No MCP server failed to start.";
  return n === 1 ? "1 MCP server didn't start." : `${n} MCP servers didn't start.`;
}
