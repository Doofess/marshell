import type { CSSProperties } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { Chip } from "../../components/Chip/Chip";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import type { RowModel } from "../sidebar/types";
import { ScriptedTerminal } from "../terminal/ScriptedTerminal";
import type { TerminalThemeSetting } from "../terminal/terminalTheme";
import "./SplitView.css";

const NARROW = 80;
/** The main window never scrolls, so an empty pane lists what fits and sends the rest to the command palette. */
const CHOICE_LIMIT = 5;
export const needsYou = (r: RowModel) => r.status === "needs-permission" || r.status === "needs-question";

const icon = (d: string) => (
  <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
    <path d={d} />
  </svg>
);
const ZOOM_IN = "M6 1h3v3M4 9H1V6M9 1L6 4M1 9l3-3";
const ZOOM_OUT = "M9 4H6V1M1 6h3v3M6 4l3-3M4 6L1 9";
const CLOSE = "M2 2l6 6M8 2L2 8";

export type PaneProps = {
  row: RowModel;
  active: boolean;
  setting: TerminalThemeSetting;
  style?: CSSProperties;
  /** What the terminal really gets, for the narrow-pane warning. */
  cells?: { cols: number; rows: number };
  /** Set while this pane fills the view: how many panes it stands in for. */
  zoomedOf?: number;
  /** Running sessions with no pane because the window is too small for more. */
  runningElsewhere?: number;
  os?: Os;
};

/**
 * One terminal and its 28 px header. The header carries what the glance needs when the sidebar is not in sight:
 * the status glyph (the vendor mark moving, while working), the name, the project and, only when it applies,
 * how tight the pane is. A pane that needs you never recedes with the inactive headers.
 */
export function Pane({ row, active, setting, style, cells, zoomedOf, runningElsewhere, os = "windows" }: PaneProps) {
  const needs = needsYou(row);
  const zoomed = zoomedOf !== undefined;
  const zoomKeys = shortcutLabel(commandById("zoom-pane").binding!, os);
  const narrow = cells && cells.cols < NARROW;
  return (
    <section className="pane" data-active={active} data-agent={row.agent} style={style} aria-label={`Terminal: ${row.name}`}>
      <header className="pane__header" data-needs={needs || undefined}>
        <span className="pane__stripe" aria-hidden="true" />
        {row.status === "working" ? <AgentMark agent={row.agent} size={16} working /> : <StatusGlyph kind={row.status} />}
        <span className="pane__name" dir="auto">
          {row.name}
        </span>
        <span className="pane__place" dir="auto">
          {row.project}
        </span>
        <span className="pane__spacer" />
        {zoomed && zoomedOf > 1 && <span className="pane__running">{zoomedOf - 1} more running</span>}
        {!zoomed && runningElsewhere !== undefined && runningElsewhere > 0 && <span className="pane__running">{runningElsewhere} more running</span>}
        {narrow && (
          <Chip tone="caution" title={`This pane is narrower than ${NARROW} columns, which many CLIs need. Zoom it (${zoomKeys}) or change the layout.`}>
            {cells.cols}×{cells.rows}
          </Chip>
        )}
        <button
          type="button"
          className="pane__button"
          aria-label={zoomed ? `Restore the ${zoomedOf} panes` : `Zoom ${row.name}`}
          aria-pressed={zoomed}
          title={`${zoomed ? "Restore the panes" : "Zoom this pane"} (${zoomKeys})`}
        >
          {icon(zoomed ? ZOOM_OUT : ZOOM_IN)}
        </button>
        <button type="button" className="pane__button" aria-label={`Remove ${row.name} from the view`} title="Remove from the view. The session keeps running.">
          {icon(CLOSE)}
        </button>
      </header>
      <ScriptedTerminal setting={setting} label={`Terminal for ${row.name}`} />
    </section>
  );
}

/** A slot with no session yet: pick one that is not on screen, or start a new one. */
export function EmptyPane({ choices, active, style, os = "windows" }: { choices: RowModel[]; active: boolean; style?: CSSProperties; os?: Os }) {
  const keys = shortcutLabel(commandById("launcher").binding!, os);
  return (
    <section className="pane pane--empty" data-active={active} style={style} aria-label="Empty pane">
      <header className="pane__header">
        <span className="pane__name">Empty pane</span>
        <span className="pane__spacer" />
        <button type="button" className="pane__button" aria-label="Remove the empty pane from the view" title="Remove from the view">
          {icon(CLOSE)}
        </button>
      </header>
      <div className="pane__empty">
        <p className="pane__empty-title">Choose a session for this pane</p>
        {choices.length > 0 ? (
          <ul className="pane__choices">
            {choices.slice(0, CHOICE_LIMIT).map((r) => (
              <li key={r.id}>
                <button type="button" className="pane__choice" data-agent={r.agent}>
                  <span className="pane__stripe" aria-hidden="true" />
                  <StatusGlyph kind={r.status} />
                  <span className="pane__choice-name" dir="auto">
                    {r.name}
                  </span>
                  <span className="pane__place" dir="auto">
                    {r.project}
                  </span>
                </button>
              </li>
            ))}
            {choices.length > CHOICE_LIMIT && (
              <li className="pane__more">
                +{choices.length - CHOICE_LIMIT} more, open the command palette ({shortcutLabel(commandById("palette").binding!, os)})
              </li>
            )}
          </ul>
        ) : (
          <p className="pane__empty-note">Every session is already on screen.</p>
        )}
        <button type="button" className="pane__new">
          <span>Start a new session</span> <kbd className="pane__kbd">{keys}</kbd>
        </button>
      </div>
    </section>
  );
}
