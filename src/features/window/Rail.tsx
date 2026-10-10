import { AgentMark } from "../../components/AgentMark/AgentMark";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { rowLabel } from "../sidebar/labels";
import { monogram } from "../sidebar/monogram";
import type { RowModel } from "../sidebar/types";
import "./Rail.css";

/**
 * The 52 px rail: the brand stripe, status glyph and a monogram per session, with a needs-you badge stack on top. The
 * monogram is drawn from `data-mono` by CSS, so it is decoration the button's name does not have to repeat.
 * A working session shows its vendor mark moving instead of a glyph, as in the full rows.
 */
export function Rail({ rows, needsYou, selectedId }: { rows: RowModel[]; needsYou: number; selectedId?: string }) {
  return (
    <nav className="rail" aria-label="Sessions">
      {needsYou > 0 && (
        <div className="rail__badge" role="status" aria-label={`${needsYou} need you`}>
          <StatusGlyph kind="needs-permission" label="Needs you" />
          <span className="rail__count">{needsYou}</span>
        </div>
      )}
      {rows.map((r) => (
        <button key={r.id} type="button" className="rail__item" data-agent={r.agent} data-mono={monogram(r.name)} aria-label={rowLabel(r)} aria-current={r.id === selectedId ? "true" : undefined}>
          <span className="rail__stripe" aria-hidden="true" />
          {r.status === "working" ? <AgentMark agent={r.agent} working /> : <StatusGlyph kind={r.status} size={16} />}
        </button>
      ))}
    </nav>
  );
}
