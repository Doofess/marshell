import type { CliId } from "../onboarding/types";

export type Severity = "error" | "warning" | "info";
export type DoctorItem = {
  id: string;
  cli: CliId | "marshell";
  title: string;
  detail: string;
  severity: Severity;
  fixable: boolean;
  /** Why Marshell will not fix it, in words; set on every item that is not fixable and is not information. */
  unfixableReason?: string;
  file?: string;
  diff?: string;
  /** Other files this fix writes besides `file` (such as where a chained status line's original is kept), each with its own diff. */
  alsoWrites?: { file: string; diff: string }[];
  /** Session numbers that only pick the fix up after a restart. */
  restartSessions?: number[];
};
export type DoctorReport = { items: DoctorItem[]; checkedAtMs: number; durationMs: number };
export type FixGroup = { file: string; items: DoctorItem[]; diff: string; added: number; removed: number };
