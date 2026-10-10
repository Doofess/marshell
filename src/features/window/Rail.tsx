import { AgentMark } from "../../components/AgentMark/AgentMark";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { rowLabel } from "../sidebar/labels";
import { monogram } from "../sidebar/monogram";
import type { RowModel } from "../sidebar/types";
import "./Rail.css";

/**
 * The 52 px rail: the brand stripe, status glyph and a monogram per session, with a needs-you badge stack on top. The
 * monogram is drawn from `data-mono` by CSS, so it is decoration the button's name does not have to repeat.
 * A working session shows its vendor mark moving instead of a glyph, as in the full rows.
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
          <button key={r.id} type="button" className="rail__item" data-agent={r.agent} data-mono={monogram(r.name)} aria-label={rowLabel(r)} aria-current={r.id === selectedId ? "true" : undefined}>
            <span className="rail__stripe" aria-hidden="true" />
            {r.status === "working" ? <AgentMark agent={r.agent} working /> : <StatusGlyph kind={r.status} size={16} />}
          </button>
        ))}
      </div>
      {/* The bar's 52 px column holds only the logo, so the palette lives here, where the rail's own tools go. */}
      <div className="rail__tools">
        <button type="button" className="rail__tool" aria-label="Command palette" title={`Command palette (${shortcutLabel(commandById("palette").binding!, os)})`}>
          <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M2.5 4.5L6 7l-3.5 2.5M7.5 10h4" />
          </svg>
        </button>
      </div>
    </nav>
  );
}
