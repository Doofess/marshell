import { ContextRing } from "../../components/ContextRing/ContextRing";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import type { ApprovalRequest } from "../approval/types";
import { Lane } from "../lane/Lane";
import { laneLayout, pending } from "../lane/laneLayout";
import { SessionRow } from "../sidebar/SessionRow";
import type { RowModel } from "../sidebar/types";
import { listLayout } from "./listLayout";
import "./Sidebar.css";

/** Height of a comfortable row: the `--row-comfortable` token (a test keeps the two equal). */
export const ROW_HEIGHT = 56;
const FOOTER = 40;

export type SidebarFooter = { planPct?: number; fiveHourPct?: number; doctorIssues?: number };

/**
 * The expanded sidebar: the needs-you lane, the session list, and a 40 px footer with plan use and the doctor.
 * Unknown plan usage is left out of the footer, not shown as a dash.
 */
export function Sidebar({ rows, requests, height, width, selectedId, footer }: { rows: RowModel[]; requests: ApprovalRequest[]; height: number; width: number; selectedId?: string; footer: SidebarFooter }) {
  const issues = footer.doctorIssues ?? 0;
  // The main window never scrolls: the list shows the rows that fit under the lane, and a button opens the rest in a menu.
  const lane = laneLayout(pending(requests).length, height);
  const { visible, hidden } = listLayout(rows.length, height - lane.height - FOOTER, ROW_HEIGHT);
  const shown = rows.slice(0, visible);
  const tabStop = shown.some((r) => r.id === selectedId) ? selectedId : shown[0]?.id;
  return (
    <aside className="sidebar" style={{ inlineSize: width }} aria-label="Sessions and approvals">
      <Lane requests={requests} sidebarHeight={height} />
      <div className="sidebar__list">
        <div role="listbox" aria-label="Sessions">
          {shown.map((r) => (
            <SessionRow key={r.id} row={r} density="comfortable" selected={r.id === selectedId} tabStop={r.id === tabStop} />
          ))}
        </div>
        {hidden > 0 && (
          <button type="button" className="sidebar__more" aria-label={`+${hidden} more sessions, show all ${rows.length}`} aria-haspopup="dialog">
            +{hidden} more sessions
          </button>
        )}
      </div>
      <footer className="sidebar__footer">
        <ContextRing pct={footer.planPct} />
        {footer.fiveHourPct !== undefined && <span className="sidebar__plan">5h {Math.round(footer.fiveHourPct)}%</span>}
        <span className="sidebar__spacer" />
        <span className="sidebar__doctor" role="img" aria-label={issues > 0 ? `Doctor: ${issues} issues` : "Doctor: no issues"}>
          <span aria-hidden="true">{issues > 0 ? <AuxGlyph kind="caution" /> : <StatusGlyph kind="done-seen" size={12} />}</span>
          <span aria-hidden="true">{issues}</span>
        </span>
      </footer>
    </aside>
  );
}
