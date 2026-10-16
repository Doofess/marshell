import { ContextRing } from "../../components/ContextRing/ContextRing";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import type { ApprovalRequest } from "../approval/types";
import { Lane } from "../lane/Lane";
import { laneLayout, pending } from "../lane/laneLayout";
import { SessionRow, type RowDrag } from "../sidebar/SessionRow";
import { dragOver, dropOrder, TAB_MIME, type DragState } from "../sidebar/reorder";
import type { RowModel } from "../sidebar/types";
import { useState, type CSSProperties, type ReactNode } from "react";
import type { Os } from "../../lib/keymap";
import type { ShellName } from "../terminal/scriptedShell";
import { shellLayout } from "./shellLayout";
import { ShellPanel } from "./ShellPanel";
import "./Sidebar.css";

/** Height of a comfortable row: the `--row-comfortable` token (a test keeps the two equal). */
export const ROW_HEIGHT = 56;

export type SidebarFooter = { planPct?: number; fiveHourPct?: number; doctorIssues?: number };

/** The plain shell under the list. Left out, the sidebar is just the lane, the list and the footer. */
export type SidebarShell = { shellName?: ShellName; cwd?: string; collapsed?: boolean; os?: Os; terminal?: ReactNode; onToggle?: () => void; onOpenInPane?: () => void };

/**
 * The expanded sidebar: the needs-you lane, the session list, an optional plain shell, and a 40 px footer with plan use
 * and the doctor (left out when `footer` is). The list scrolls (wheel, touch, keys) with no scrollbar drawn, and every session is in it; the selected
 * row is the one tab stop and scrolls into view when it takes focus. Unknown plan usage is left out of the footer.
 */
export function Sidebar({
  rows,
  requests,
  height,
  width,
  selectedId,
  footer,
  shell,
  renamingId,
  onRename,
  onRenameCancel,
  onReorder,
  onSelect,
}: {
  rows: RowModel[];
  requests: ApprovalRequest[];
  height: number;
  width: number;
  selectedId?: string;
  footer?: SidebarFooter;
  shell?: SidebarShell;
  /** F2: the row whose name is a text box, and what its Enter and its Escape or blur do. */
  renamingId?: string | null;
  onRename?: (id: string, name: string) => void;
  onRenameCancel?: (opts: { refocus: boolean }) => void;
  /** Dragging a row to a new place gives the whole new order. Left out, rows are not draggable. */
  onReorder?: (ids: string[]) => void;
  /** A click on a row gives its session id. */
  onSelect?: (id: string) => void;
}) {
  const issues = footer?.doctorIssues ?? 0;
  const [drag, setDrag] = useState<DragState | null>(null);
  const ids = rows.map((r) => r.id);
  const dragFor = (id: string): RowDrag | undefined => {
    if (!onReorder) return undefined;
    return {
      dropEdge: drag?.over?.id === id ? drag.over.edge : undefined,
      onDragStart: (e) => {
        e.dataTransfer.effectAllowed = "move";
        // A custom type only: dropping the row on a text field must not insert the tab id there.
        e.dataTransfer.setData(TAB_MIME, id);
        setDrag({ id });
      },
      onDragOver: (e) => {
        if (!drag) return;
        const box = e.currentTarget.getBoundingClientRect();
        const next = dragOver(drag, id, e.clientY < box.top + box.height / 2 ? "before" : "after");
        if (next.over?.id !== drag.over?.id || next.over?.edge !== drag.over?.edge) setDrag(next);
        // Over its own row there is nowhere to drop.
        if (id === drag.id) return;
        e.preventDefault();
        e.dataTransfer.dropEffect = "move";
      },
      onDrop: (e) => {
        e.preventDefault();
        const next = drag?.over && dropOrder(ids, drag.id, drag.over.id, drag.over.edge);
        setDrag(null);
        if (next) onReorder(next);
      },
      onDragEnd: () => setDrag(null),
    };
  };
  const tabStop = rows.some((r) => r.id === selectedId) ? selectedId : rows[0]?.id;
  const lane = laneLayout(pending(requests).length, height);
  const panel = shell ? shellLayout(height, lane.height, shell.collapsed, ROW_HEIGHT) : null;
  const style = { inlineSize: width, ...(panel ? { "--shell-h": `${panel.height}px` } : {}), ...(footer ? {} : { "--footer-h": "0px" }) } as CSSProperties;
  return (
    <aside className="sidebar" style={style} data-shell={panel ? "" : undefined} aria-label="Sessions and approvals">
      <Lane requests={requests} sidebarHeight={height} />
      <div className="sidebar__list" role="listbox" aria-label="Sessions">
        {rows.map((r) => (
          <SessionRow
            key={r.id}
            row={r}
            density="comfortable"
            selected={r.id === selectedId}
            tabStop={r.id === tabStop}
            drag={dragFor(r.id)}
            onSelect={onSelect && (() => onSelect(r.id))}
            rename={r.id === renamingId && onRename && onRenameCancel ? { onCommit: (name) => onRename(r.id, name), onCancel: onRenameCancel } : undefined}
          />
        ))}
      </div>
      {panel && shell && <ShellPanel height={panel.height} collapsed={panel.collapsed} auto={panel.auto} cwd={shell.cwd} shellName={shell.shellName} os={shell.os} terminal={shell.terminal} onToggle={shell.onToggle} onOpenInPane={shell.onOpenInPane} />}
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
