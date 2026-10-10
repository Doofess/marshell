import { APPROVALS } from "../approval/fixtures";
import type { ApprovalRequest } from "../approval/types";
import { FIXTURES } from "../sidebar/fixtures";
import type { RowModel } from "../sidebar/types";
import type { PlanUsage } from "../composer/stripItems";
import type { SidebarFooter } from "./Sidebar";

/** The 5-agent scenario: 2 cards in the lane, 1 working, 1 done-unseen, 1 error. It drives every window story and the 5-second test. */
export const SCENARIO_IDS = ["needs-permission", "needs-question", "working", "done-unseen", "error"] as const;

export const SCENARIO_ROWS: RowModel[] = SCENARIO_IDS.map((id) => FIXTURES.find((f) => f.id === id)!);

/** Session names match the rows: billing (65 s, a command) and infra (4 min, a question). */
export const SCENARIO_REQUESTS: ApprovalRequest[] = [APPROVALS.safeBash, APPROVALS.question];

export const SCENARIO_FOOTER: SidebarFooter = { planPct: 38, fiveHourPct: 38, doctorIssues: 2 };
/** The plan's two rolling limits, as the status strip shows them: 5-hour at 38% resetting in 2h 10m, 7-day at 12% resetting in 3d 4h. */
export const SCENARIO_PLAN: PlanUsage = { fiveHour: { pct: 38, resetsInMs: 7_800_000 }, sevenDay: { pct: 12, resetsInMs: 273_600_000 } };
export const SCENARIO_PORTS = ["localhost:3000", "localhost:5173"];
