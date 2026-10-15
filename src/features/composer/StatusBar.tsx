import { SessionNumber } from "../../components/SessionNumber/SessionNumber";
import type { Os } from "../../lib/keymap";
import { sessionWord } from "../sidebar/sessionNumber";
import type { RowModel } from "../sidebar/types";
import { StatusStrip } from "./StatusStrip";
import type { PlanUsage } from "./stripItems";
import "./StatusBar.css";

/**
 * The status line of the window: one row at the foot, for the session of the focused pane, in the spirit of a CLI status line.
 * Model and effort, context, the plan limits, cost, time, MCP health and the rest, with unknown values left out. It is its
 * own container, so the strip hides the least important items by the bar's width. With `width` (in characters) the items are
 * fitted in script instead, for places that cannot measure. An empty pane leaves the bar empty at the same height, so the
 * window never changes size when focus moves. It starts with the session's number and name, which are never dropped, so the
 * facts always say whose they are; the strip fits in what is left.
 */
/** `os` is accepted for symmetry with the other bars; the strip shows no shortcut, so it does not use it. */
export function StatusBar({ row, plan, ports, width }: { row: RowModel | null; plan?: PlanUsage; ports?: string[]; width?: number; os?: Os }) {
  return (
    <div className="status-bar" data-agent={row?.agent}>
      {row && (
        <span className="status-bar__session" role="img" aria-label={`${sessionWord(row.number)}, ${row.name}`}>
          <SessionNumber n={row.number} />
          <span className="status-bar__name" dir="auto">
            {row.name}
          </span>
        </span>
      )}
      <div className="status-bar__facts">
        <StatusStrip row={row} plan={plan} ports={ports} width={width} />
      </div>
    </div>
  );
}
