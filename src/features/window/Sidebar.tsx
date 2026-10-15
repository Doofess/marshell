import { ContextRing } from "../../components/ContextRing/ContextRing";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import type { ApprovalRequest } from "../approval/types";
import { Lane } from "../lane/Lane";
import { laneLayout, pending } from "../lane/laneLayout";
import { SessionRow } from "../sidebar/SessionRow";
import type { RowModel } from "../sidebar/types";
import type { CSSProperties } from "react";
import type { Os } from "../../lib/keymap";
import type { ShellName } from "../terminal/scriptedShell";
import { shellLayout } from "./shellLayout";
import { ShellPanel } from "./ShellPanel";
import "./Sidebar.css";

/** Height of a comfortable row: the `--row-comfortable` token (a test keeps the two equal). */
export const ROW_HEIGHT = 56;

export type SidebarFooter = { planPct?: number; fiveHourPct?: number; doctorIssues?: number };

/** The plain shell under the list. Left out, the sidebar is just the lane, the list and the footer. */
export type SidebarShell = { shellName?: ShellName; cwd?: string; collapsed?: boolean; os?: Os };

/**
 * The expanded sidebar: the needs-you lane, the session list, an optional plain shell, and a 40 px footer with plan use
 * and the doctor (left out when `footer` is). The list scrolls (wheel, touch, keys) with no scrollbar drawn, and every session is in it; the selected
 * row is the one tab stop and scrolls into view when it takes focus. Unknown plan usage is left out of the footer.
 */
export function Sidebar({ rows, requests, height, width, selectedId, footer, shell }: { rows: RowModel[]; requests: ApprovalRequest[]; height: number; width: number; selectedId?: string; footer?: SidebarFooter; shell?: SidebarShell }) {
  const issues = footer?.doctorIssues ?? 0;
  const tabStop = rows.some((r) => r.id === selectedId) ? selectedId : rows[0]?.id;
  const lane = laneLayout(pending(requests).length, height);
  const panel = shell ? shellLayout(height, lane.height, shell.collapsed, ROW_HEIGHT) : null;
  const style = { inlineSize: width, ...(panel ? { "--shell-h": `${panel.height}px` } : {}), ...(footer ? {} : { "--footer-h": "0px" }) } as CSSProperties;
  return (
    <aside className="sidebar" style={style} data-shell={panel ? "" : undefined} aria-label="Sessions and approvals">
      <Lane requests={requests} sidebarHeight={height} />
      <div className="sidebar__list" role="listbox" aria-label="Sessions">
        {rows.map((r) => (
          <SessionRow key={r.id} row={r} density="comfortable" selected={r.id === selectedId} tabStop={r.id === tabStop} />
        ))}
      </div>
      {panel && shell && <ShellPanel height={panel.height} collapsed={panel.collapsed} auto={panel.auto} cwd={shell.cwd} shellName={shell.shellName} os={shell.os} />}
      {footer && (
        <footer className="sidebar__footer">
          <ContextRing pct={footer.planPct} />
          {footer.fiveHourPct !== undefined && <span className="sidebar__plan">5h {Math.round(footer.fiveHourPct)}%</span>}
          <span className="sidebar__spacer" />
          <span className="sidebar__doctor" role="img" aria-label={issues > 0 ? `Doctor: ${issues} issues` : "Doctor: no issues"}>
            <span aria-hidden="true">{issues > 0 ? <AuxGlyph kind="caution" /> : <StatusGlyph kind="done-seen" size={12} />}</span>
            <span aria-hidden="true">{issues}</span>
          </span>
        </footer>
      )}
    </aside>
  );
}
