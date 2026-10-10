import type { Os } from "../../lib/keymap";
import type { RowModel } from "../sidebar/types";
import { StatusStrip } from "./StatusStrip";
import type { PlanUsage } from "./stripItems";
import "./StatusBar.css";

/**
 * The status line of the window: one row at the foot, for the session of the focused pane, in the spirit of a CLI status line.
 * Model and effort, context, the plan limits, cost, time, MCP health and the rest, with unknown values left out. It is its
 * own container, so the strip hides the least important items by the bar's width. With `width` (in characters) the items are
 * fitted in script instead, for places that cannot measure. An empty pane leaves the bar empty at the same height, so the
 * window never changes size when focus moves.
 */
/** `os` is accepted for symmetry with the other bars; the strip shows no shortcut, so it does not use it. */
export function StatusBar({ row, plan, ports, width }: { row: RowModel | null; plan?: PlanUsage; ports?: string[]; width?: number; os?: Os }) {
  return (
    <div className="status-bar" data-agent={row?.agent}>
      <StatusStrip row={row} plan={plan} ports={ports} width={width} />
    </div>
  );
}
