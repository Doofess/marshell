import type { SessionKind } from "../../components/AgentMark/kinds";
import type { GlyphKind } from "../../components/StatusGlyph/glyphs";
import { COMMANDS, shortcutLabel, type Os } from "../../lib/keymap";

export type { Os };

export type TraySession = { number: number; name: string; status: GlyphKind; phrase: string; agent: SessionKind };
export type TrayState = { os: Os; sessions: TraySession[]; paused: boolean; doctorIssues: number | null; departures: boolean };
export type TrayItem = { id: string; label: string; kind: "item" | "session" | "separator"; shortcut?: string; number?: number; disabled?: boolean };

const WAITING = new Set<GlyphKind>(["needs-permission", "needs-question"]);
export const MAX_TRAY_SESSIONS = 5;
const sep = (n: number): TrayItem => ({ id: `sep-${n}`, label: "", kind: "separator" });

/**
 * The tray menu: open the window, open Departures, who needs you (at most five, with their numbers, then how many more),
 * go to the oldest waiting, then pause, the doctor, Notifications, and quit last. Only sessions that wait on the user are listed.
 * The live app opens Departures; `doctorIssues: null` leaves out the Doctor until it is available. The design stories show both.
 */
export function trayItems(s: TrayState): TrayItem[] {
  const waiting = s.sessions.filter((x) => WAITING.has(x.status));
  const shown = waiting.slice(0, MAX_TRAY_SESSIONS);
  const next = COMMANDS.find((c) => c.id === "next-waiting")!;
  const items: TrayItem[] = [{ id: "open", label: "Open Marshell", kind: "item" }];
  if (s.departures) items.push({ id: "departures", label: "Open Departures", kind: "item" });
  items.push(sep(1));
  if (waiting.length === 0) items.push({ id: "none", label: "Nothing needs you", kind: "item", disabled: true });
  for (const w of shown) items.push({ id: `session-${w.number}`, label: `${w.name}: ${w.phrase}`, kind: "session", number: w.number });
  if (waiting.length > shown.length) items.push({ id: "more", label: `${waiting.length - shown.length} more…`, kind: "item" });
  items.push(
    { id: "next-waiting", label: "Go to the oldest waiting session", kind: "item", shortcut: shortcutLabel(next.binding!, s.os), disabled: waiting.length === 0 },
    sep(2),
    { id: "pause", label: s.paused ? "Resume notifications" : "Pause notifications", kind: "item" },
  );
  if (s.doctorIssues !== null) items.push({ id: "doctor", label: s.doctorIssues === 0 ? "Doctor: all clear" : `Doctor: ${s.doctorIssues} ${s.doctorIssues === 1 ? "issue" : "issues"}`, kind: "item" });
  items.push({ id: "notifications", label: "Notifications…", kind: "item" }, sep(3), { id: "quit", label: "Quit Marshell", kind: "item" });
  return items;
}

/** The tooltip on the tray icon: how many need you, and when notifications are paused. */
export function trayTooltip(s: TrayState): string {
  const n = s.sessions.filter((x) => WAITING.has(x.status)).length;
  const base = n === 0 ? "Marshell" : `Marshell · ${n} ${n === 1 ? "needs" : "need"} you`;
  return s.paused ? `${base} (notifications paused)` : base;
}

/** Windows has no badge count, so it gets an overlay icon (capped at 9+); macOS a dock badge; Linux's tray is menu-only. */
export function badgeFor(os: Os, count: number): { kind: "overlay" | "dock" | "none"; text: string } {
  if (count <= 0 || os === "linux") return { kind: "none", text: "" };
  return os === "windows" ? { kind: "overlay", text: count > 9 ? "9+" : String(count) } : { kind: "dock", text: String(count) };
}
