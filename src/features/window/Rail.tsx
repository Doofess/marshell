import { AgentMark } from "../../components/AgentMark/AgentMark";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { rowLabel } from "../sidebar/labels";
import { SearchIcon } from "./SearchIcon";
import { monogram } from "../sidebar/monogram";
import { SessionNumber } from "../../components/SessionNumber/SessionNumber";
import type { RowModel } from "../sidebar/types";
import "./Rail.css";

/**
 * The 52 px rail: the brand stripe, status glyph and a monogram per session, with a needs-you badge stack on top. The
 * monogram is drawn from `data-mono` by CSS, so it is decoration the button's name does not have to repeat.
 * A working session shows its vendor mark moving instead of a glyph, as in the full rows. The list scrolls (wheel, touch,
 * keys) with no scrollbar drawn; the palette tool stays pinned at the foot.
 */
export function Rail({ rows, needsYou, selectedId, os = "windows" }: { rows: RowModel[]; needsYou: number; selectedId?: string; os?: Os }) {
  return (
    <nav className="rail" aria-label="Sessions">
      <div className="rail__list">
        {needsYou > 0 && (
          <div className="rail__badge" role="status" aria-label={`${needsYou} need you`}>
            <StatusGlyph kind="needs-permission" label="Needs you" />
            <span className="rail__count">{needsYou}</span>
          </div>
        )}
        {rows.map((r) => (
          <button key={r.id} type="button" className="rail__item" data-agent={r.agent} data-mono={monogram(r.name)} data-number={r.number} aria-label={rowLabel(r)} aria-current={r.id === selectedId ? "true" : undefined}>
            <span className="rail__stripe" aria-hidden="true" />
            <span className="rail__number">
              <SessionNumber n={r.number} />
            </span>
            {r.status === "working" ? <AgentMark agent={r.agent} working /> : <StatusGlyph kind={r.status} size={16} />}
          </button>
        ))}
      </div>
      {/* The bar's 52 px column holds only the logo, so the palette lives here, where the rail's own tools go. */}
      <div className="rail__tools">
        <button type="button" className="rail__tool" aria-label="Command palette" title={`Search sessions, projects and commands (${shortcutLabel(commandById("palette").binding!, os)})`}>
          <SearchIcon />
        </button>
      </div>
    </nav>
  );
}
