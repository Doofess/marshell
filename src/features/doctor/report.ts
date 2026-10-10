import { parseUnifiedDiff } from "../../components/DiffView/diff";
import { mergeDiffs } from "./mergeDiff";
import type { DoctorItem, DoctorReport, FixGroup } from "./types";
export * from "./types";

const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

/** Issues are errors and warnings; information (such as managed settings) is listed but never counted. */
export function counts(r: DoctorReport) {
  const issues = r.items.filter((i) => i.severity !== "info");
  const fixable = issues.filter((i) => i.fixable).length;
  return { issues: issues.length, fixable, unfixable: issues.length - fixable };
}
export function fixHeadline(r: DoctorReport): string {
  const n = counts(r).fixable;
  return n === 0 ? "Nothing to fix" : `Fix ${n} ${plural(n, "issue", "issues")}`;
}
export function groupByFile(items: DoctorItem[]): FixGroup[] {
  const by = new Map<string, DoctorItem[]>();
  for (const i of items) if (i.fixable && i.file && i.diff) by.set(i.file, [...(by.get(i.file) ?? []), i]);
  return [...by].map(([file, its]) => {
    const diff = mergeDiffs(its.map((i) => i.diff!));
    const p = parseUnifiedDiff(diff);
    return { file, items: its, diff, added: p.added, removed: p.removed };
  });
}
export const restartSessionsOf = (items: DoctorItem[]) => [...new Set(items.flatMap((i) => i.restartSessions ?? []))].sort((a, b) => a - b);
export function listNumbers(n: number[]): string {
  if (n.length <= 1) return n.join("");
  return `${n.slice(0, -1).join(", ")} and ${n[n.length - 1]}`;
}
export function afterFixMessage(fixed: number, restart: number[]): string {
  const base = `Fixed ${fixed} ${plural(fixed, "issue", "issues")}.`;
  if (restart.length === 0) return base;
  return `${base} ${restart.length} ${plural(restart.length, "session picks", "sessions pick")} this up after a restart.`;
}
