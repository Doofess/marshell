import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { rowLabel } from "../sidebar/labels";
import type { RowModel } from "../sidebar/types";
import "./AllSessionsMenu.css";

/**
 * Where the sidebar's and rail's "+N more" lead: every session in a popover. The main window never scrolls, but a
 * menu may, so this list does (with the themed scrollbar visible).
 */
export function AllSessionsMenu({ rows, selectedId, title = "All sessions" }: { rows: RowModel[]; selectedId?: string; title?: string }) {
  return (
    <div className="all-sessions" role="dialog" aria-label={title}>
      <header className="all-sessions__header">
        <span className="all-sessions__title">{title}</span>
        <span className="all-sessions__count">
          {rows.length} {rows.length === 1 ? "session" : "sessions"}
        </span>
      </header>
      <div className="all-sessions__list scroll-menu">
        {rows.map((r) => (
          <button key={r.id} type="button" className="all-sessions__row" data-agent={r.agent} aria-label={rowLabel(r)} aria-current={r.id === selectedId ? "true" : undefined}>
            <span className="all-sessions__stripe" aria-hidden="true" />
            <StatusGlyph kind={r.status} />
            <span className="all-sessions__name" dir="auto">
              {r.name}
            </span>
            <span className="all-sessions__project" dir="auto">
              {r.project}
            </span>
            <span className="all-sessions__phrase" dir="auto">
              {r.phrase}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
