import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { kindName } from "../../components/AgentMark/kinds";
import { SessionNumber } from "../../components/SessionNumber/SessionNumber";
import { sessionWord } from "../sidebar/sessionNumber";
import { GLYPHS } from "../../components/StatusGlyph/glyphs";
import type { RowModel } from "../sidebar/types";
import { selectedProps } from "../../components/a11y/SelectedMark";
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
          <button key={r.id} type="button" {...selectedProps(r.id === selectedId, "current", "all-sessions__row")} data-agent={r.agent} aria-label={`${sessionWord(r.number)}, ${r.name} ${r.project} ${r.phrase.replace(/`/g, "")}. ${kindName(r.agent, r.agentName)}, ${GLYPHS[r.status].label}.`}>
            <span className="all-sessions__stripe" aria-hidden="true" />
            <StatusGlyph kind={r.status} />
            <SessionNumber n={r.number} />{" "}
            <span className="all-sessions__name" dir="auto">
              {r.name}
            </span>{" "}
            <span className="all-sessions__project" dir="auto">
              {r.project}
            </span>{" "}
            <span className="all-sessions__phrase" dir="auto">
              {r.phrase}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
