import { ContextMenu, type MenuItem } from "../../components/ContextMenu/ContextMenu";
import { ContextRing } from "../../components/ContextRing/ContextRing";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import type { ApprovalRequest } from "../approval/types";
import { Lane } from "../lane/Lane";
import { laneLayout, pending } from "../lane/laneLayout";
import { SessionRow, type RowDrag } from "../sidebar/SessionRow";
import { dragOver, dropOrder, TAB_MIME, type DragState } from "../sidebar/reorder";
import { menuStillValid, sameMenuItems } from "../sidebar/rowMenu";
import type { RowModel } from "../sidebar/types";
import { groupBounds, groupedOrder, groupRows } from "../sidebar/grouping";
import { Fragment, useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import type { Os } from "../../lib/keymap";
import type { ShellName } from "../terminal/scriptedShell";
import { shellLayout } from "./shellLayout";
import { ShellPanel } from "./ShellPanel";
import "./Sidebar.css";

/** Height of a comfortable row: the `--row-comfortable` token (a test keeps the two equal). */
export const ROW_HEIGHT = 56;

export type SidebarFooter = { planPct?: number; fiveHourPct?: number; doctorIssues?: number };

/** The plain shell under the list. Left out, the sidebar is just the lane, the list and the footer. */
export type SidebarShell = { shellName?: ShellName; cwd?: string; collapsed?: boolean; os?: Os; terminal?: ReactNode; failed?: boolean; onRetry?: () => void; openDisabled?: boolean; forceOpen?: boolean; onToggle?: () => void; onOpenInPane?: () => void };

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
  visibleIds,
  os,
  footer,
  shell,
  renamingId,
  onRename,
  onRenameCancel,
  onReorder,
  onSelect,
  onToggle,
  onReview,
  menuItems,
  onMenuRun,
  grouped = false,
}: {
  rows: RowModel[];
  requests: ApprovalRequest[];
  height: number;
  width: number;
  /** The focused pane's session. */
  selectedId?: string;
  /** Every session on screen: each is marked selected and the list becomes multi-select. Left out, only `selectedId` is marked. */
  visibleIds?: readonly string[];
  /** Decides which click toggles a pane. Defaults to Windows (Ctrl+click). */
  os?: Os;
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
  /** Ctrl+click (Cmd on macOS) or Ctrl+Space on a row gives its session id: add or remove its pane. */
  onToggle?: (id: string) => void;
  /** Opens the review of a task row's session (its Review changes button). */
  onReview?: (id: string) => void;
  /** The items of a row's right-click menu. Left out, rows have no menu. */
  menuItems?: (id: string) => MenuItem[];
  /** A menu item was chosen: the row's session id and the item's id. */
  onMenuRun?: (id: string, itemId: string, anchor: { x: number; y: number }) => void;
  /** Rows under a heading per project (name and count); a drag moves a row only inside its group. Off by default. */
  grouped?: boolean;
}) {
  const issues = footer?.doctorIssues ?? 0;
  const [drag, setDrag] = useState<DragState | null>(null);
  // The open menu keeps the items it opened with, so they stay the same objects while it is open.
  const [menu, setMenu] = useState<{ id: string; anchor: { x: number; y: number }; items: MenuItem[] } | null>(null);
  const root = useRef<HTMLElement>(null);
  // Grouped, the list is the groups one after another, and a move or a drop stays inside the dragged row's group.
  const groups = grouped ? groupRows(rows) : null;
  const shown = groups ? groupedOrder(groups) : rows;
  const ids = shown.map((r) => r.id);
  // The row's state moving under it (a resume, an exit) changes what it offers: the menu closes rather than show a stale item.
  const menuValid = menuStillValid(menu?.id ?? null, ids) && (!menu || !menuItems || sameMenuItems(menu.items, menuItems(menu.id)));
  // The row closed, went away or changed while its menu was open: the menu goes too, and never acts on it.
  useEffect(() => {
    if (menu && !menuValid) setMenu(null);
  }, [menu, menuValid]);
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
        // Another project's row is not a place to drop: no indicator, and the drop is not allowed.
        const bounds = groups ? groupBounds(groups, drag.id) : undefined;
        if (bounds && (ids.indexOf(id) < bounds.start || ids.indexOf(id) >= bounds.end)) {
          if (drag.over) setDrag({ id: drag.id });
          return;
        }
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
        const next = drag?.over && dropOrder(ids, drag.id, drag.over.id, drag.over.edge, groups ? groupBounds(groups, drag.id) : undefined);
        setDrag(null);
        if (next) onReorder(next);
      },
      onDragEnd: () => setDrag(null),
    };
  };
  const tabStop = shown.some((r) => r.id === selectedId) ? selectedId : shown[0]?.id;
  const row = (r: RowModel) => (
    <SessionRow
      key={r.id}
      row={r}
      density="comfortable"
      // With visible ids, a session in no pane on screen (the store's choice standing in for an empty pane) is not marked.
      selected={visibleIds ? visibleIds.includes(r.id) : r.id === selectedId}
      focused={visibleIds ? visibleIds.includes(r.id) && r.id === selectedId : false}
      os={os}
      tabStop={r.id === tabStop}
      drag={dragFor(r.id)}
      onSelect={onSelect && (() => onSelect(r.id))}
      onToggle={onToggle && (() => onToggle(r.id))}
      onReview={onReview && (() => onReview(r.id))}
      onMenu={
        menuItems &&
        ((anchor) => {
          // The menu acts on the row under it by id and leaves the selection alone: selecting would put the session in the
          // focused pane, which would make "Split into a pane" pointless.
          setMenu({ id: r.id, anchor, items: menuItems(r.id) });
        })
      }
      rename={r.id === renamingId && onRename && onRenameCancel ? { onCommit: (name) => onRename(r.id, name), onCancel: onRenameCancel } : undefined}
    />
  );
  const lane = laneLayout(pending(requests).length, height);
  const panel = shell ? shellLayout(height, lane.height, shell.collapsed, ROW_HEIGHT, shell.forceOpen) : null;
  const style = { inlineSize: width, ...(panel ? { "--shell-h": `${panel.height}px` } : {}), ...(footer ? {} : { "--footer-h": "0px" }) } as CSSProperties;
  return (
    <aside ref={root} className="sidebar" style={style} data-shell={panel ? "" : undefined} aria-label="Sessions and approvals">
      <Lane requests={requests} sidebarHeight={height} />
      <div className="sidebar__list" role="listbox" aria-label="Sessions" aria-multiselectable={visibleIds ? true : undefined}>
        {groups
          ? groups.map((g) => (
              <Fragment key={g.project}>
                <div className="sidebar__group" role="presentation">
                  <span className="sidebar__group-name">{g.project}</span>
                  <span className="sidebar__group-count">{g.rows.length}</span>
                </div>
                {g.rows.map(row)}
              </Fragment>
            ))
          : rows.map(row)}
      </div>
      {panel && shell && <ShellPanel height={panel.height} collapsed={panel.collapsed} auto={panel.auto} cwd={shell.cwd} shellName={shell.shellName} os={shell.os} terminal={shell.terminal} failed={shell.failed} onRetry={shell.onRetry} openDisabled={shell.openDisabled} onToggle={shell.onToggle} onOpenInPane={shell.onOpenInPane} />}
      {menu && menuValid && onMenuRun !== undefined && (
        <ContextMenu
          label={`Session ${rows.find((r) => r.id === menu.id)?.name ?? ""}`}
          items={menu.items}
          anchor={menu.anchor}
          onRun={(itemId) => {
            setMenu(null);
            onMenuRun(menu.id, itemId, menu.anchor);
          }}
          onClose={({ refocus }) => {
            setMenu(null);
            const n = rows.find((r) => r.id === menu.id)?.number;
            if (refocus && n !== undefined) root.current?.querySelector<HTMLElement>(`[data-nav-row="${n}"]`)?.focus();
          }}
        />
      )}
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
