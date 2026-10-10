import { ContextRing } from "../../components/ContextRing/ContextRing";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import type { ApprovalRequest } from "../approval/types";
import { Lane } from "../lane/Lane";
import { SessionRow } from "../sidebar/SessionRow";
import type { RowModel } from "../sidebar/types";
import "./Sidebar.css";

export type SidebarFooter = { planPct?: number; fiveHourPct?: number; doctorIssues?: number };

/**
 * The expanded sidebar: the needs-you lane, the session list, and a 40 px footer with plan use and the doctor.
 * Unknown plan usage is left out of the footer, not shown as a dash.
 */
export function Sidebar({ rows, requests, height, width, selectedId, footer }: { rows: RowModel[]; requests: ApprovalRequest[]; height: number; width: number; selectedId?: string; footer: SidebarFooter }) {
  const issues = footer.doctorIssues ?? 0;
  const tabStop = rows.some((r) => r.id === selectedId) ? selectedId : rows[0]?.id;
  return (
    <aside className="sidebar" style={{ inlineSize: width }} aria-label="Sessions and approvals">
      <Lane requests={requests} sidebarHeight={height} />
      <div className="sidebar__list" role="listbox" aria-label="Sessions">
        {rows.map((r) => (
          <SessionRow key={r.id} row={r} density="comfortable" selected={r.id === selectedId} tabStop={r.id === tabStop} />
        ))}
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
