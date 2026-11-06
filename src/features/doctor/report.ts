import type { DoctorGroup, DoctorItem, DoctorReport } from "./types";
export * from "./types";

const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

/**
 * Issues are Broken and Waste items; Info (such as managed settings) is listed but never counted. `offered` counts every
 * fixable item, Info ones too (transcript retention), since the fix button offers them all.
 */
export function counts(r: DoctorReport) {
  const issues = r.items.filter((i) => i.group !== "info");
  const fixable = issues.filter((i) => i.fixable).length;
  const offered = r.items.filter((i) => i.fixable).length;
  return { issues: issues.length, fixable, unfixable: issues.length - fixable, offered };
}

/** Fixable Broken items start checked, except one marked `unchecked`; an item that was in the previous report keeps the state the person gave it. */
export function initialSelection(r: DoctorReport, prev?: ReadonlySet<string>, prevReport?: DoctorReport): Set<string> {
  const before = new Set(prevReport?.items.map((i) => i.id));
  const on = new Set<string>();
  for (const i of r.items) {
    if (!i.fixable) continue;
    if (prev && before.has(i.id) ? prev.has(i.id) : i.group === "broken" && !i.unchecked) on.add(i.id);
  }
  return on;
}
export const selectedFixable = (r: DoctorReport, sel: ReadonlySet<string>): DoctorItem[] => r.items.filter((i) => i.fixable && sel.has(i.id));
export const fixHeadline = (n: number): string => `Fix ${n} ${plural(n, "issue", "issues")}`;

const HEADINGS: { group: DoctorGroup; heading: "Broken" | "Waste" | "Info" }[] = [
  { group: "broken", heading: "Broken" },
  { group: "waste", heading: "Waste" },
  { group: "info", heading: "Info" },
];
export function groups(r: DoctorReport): { group: DoctorGroup; heading: "Broken" | "Waste" | "Info"; items: DoctorItem[] }[] {
  return HEADINGS.map((h) => ({ ...h, items: r.items.filter((i) => i.group === h.group) })).filter((g) => g.items.length > 0);
}

/** The sessions that only pick a fix up after a restart, counted (the core names them by tab id). */
export const restartNote = (n: number): string => `${n} ${plural(n, "session picks", "sessions pick")} this up after a restart`;
/** Session numbers in order, joined with "and": 3, "3 and 5", "1, 2 and 3". */
export function listNumbers(numbers: readonly number[]): string {
  const n = [...numbers].sort((a, b) => a - b).map(String);
  return n.length <= 1 ? n.join("") : `${n.slice(0, -1).join(", ")} and ${n[n.length - 1]}`;
}
export function afterFixMessage(fixed: number, restart: readonly unknown[], itemIds: readonly string[] = []): string {
  const base = `Fixed ${fixed} ${plural(fixed, "issue", "issues")}.`;
  const parts = [base, ...(restart.length > 0 ? [`${restartNote(restart.length)}.`] : []), ...(fixesCodexHooks(itemIds) ? ["Codex runs the changed hooks once you trust them in /hooks."] : [])];
  return parts.join(" ");
}

/** Whether a fix rewrites Codex's hooks, on Windows or in a distro: Codex runs changed hooks only once the person trusts them (PLAN §6). */
export const fixesCodexHooks = (itemIds: readonly string[]): boolean => itemIds.some((id) => id === "hooks.codex" || id.startsWith("hooks.codex@"));
