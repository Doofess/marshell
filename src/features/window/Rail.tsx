import { AgentMark } from "../../components/AgentMark/AgentMark";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { rowLabel } from "../sidebar/labels";
import { listLayout } from "./listLayout";
import { SearchIcon } from "./SearchIcon";
import { monogram } from "../sidebar/monogram";
import type { RowModel } from "../sidebar/types";
import "./Rail.css";

/**
 * The 52 px rail: the brand stripe, status glyph and a monogram per session, with a needs-you badge stack on top. The
 * monogram is drawn from `data-mono` by CSS, so it is decoration the button's name does not have to repeat.
 * A working session shows its vendor mark moving instead of a glyph, as in the full rows.
 */
/** Fixed sizes, so how many sessions fit is arithmetic: an item, the needs-you badge stack, the 40 px bar above, the tools below, and the list's 8 px padding. */
export const RAIL_ITEM = 48;
export const RAIL_BADGE = 56;
const BAR = 40;
const TOOLS = 40;
const PAD = 16;

export function Rail({ rows, needsYou, selectedId, os = "windows", height = 10_000 }: { rows: RowModel[]; needsYou: number; selectedId?: string; os?: Os; height?: number }) {
  // The main window never scrolls: the rail shows the sessions that fit and a "+N" button for the rest.
  const area = height - BAR - TOOLS - PAD - (needsYou > 0 ? RAIL_BADGE : 0);
  const { visible, hidden } = listLayout(rows.length, area, RAIL_ITEM);
  const shown = rows.slice(0, visible);
  return (
    <nav className="rail" aria-label="Sessions">
      <div className="rail__list">
        {needsYou > 0 && (
          <div className="rail__badge" role="status" aria-label={`${needsYou} need you`}>
            <StatusGlyph kind="needs-permission" label="Needs you" />
            <span className="rail__count">{needsYou}</span>
          </div>
        )}
        {shown.map((r) => (
          <button key={r.id} type="button" className="rail__item" data-agent={r.agent} data-mono={monogram(r.name)} aria-label={rowLabel(r)} aria-current={r.id === selectedId ? "true" : undefined}>
            <span className="rail__stripe" aria-hidden="true" />
            {r.status === "working" ? <AgentMark agent={r.agent} working /> : <StatusGlyph kind={r.status} size={16} />}
          </button>
        ))}
        {hidden > 0 && (
          <button type="button" className="rail__more" aria-label={`+${hidden}, show all ${rows.length} sessions`} aria-haspopup="dialog">
            +{hidden}
          </button>
        )}
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
