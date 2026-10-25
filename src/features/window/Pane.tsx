import type { CSSProperties, ReactNode } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { Chip } from "../../components/Chip/Chip";
import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { SessionNumber } from "../../components/SessionNumber/SessionNumber";
import { GLYPHS } from "../../components/StatusGlyph/glyphs";
import { hasPromptLine } from "../sidebar/promptLine";
import { sessionWord } from "../sidebar/sessionNumber";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { Composer } from "../composer/Composer";
import { headerDanger } from "./SessionHeader";
import type { RowModel } from "../sidebar/types";
import { ScriptedTerminal } from "../terminal/ScriptedTerminal";
import { SHELL_SIZE, shellSession } from "../terminal/scriptedShell";
import type { TerminalThemeSetting } from "../terminal/terminalTheme";
import { COMPOSER } from "./layout";
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
  /** True when the view shows two or more panes, so the focused one carries the ring (`data-split`). */
  split?: boolean;
  /** The pane's slot in the view, which the keyboard cursor reaches it by (`data-nav-pane`). */
  navIndex?: number;
  setting: TerminalThemeSetting;
  style?: CSSProperties;
  /** What the terminal really gets, for the narrow-pane warning. */
  cells?: { cols: number; rows: number };
  /** Set while this pane fills the view: how many panes it stands in for. */
  zoomedOf?: number;
  /** While zoomed: the other sessions still running (an empty slot is not one). Defaults to the other panes. */
  othersRunning?: number;
  /** Running sessions with no pane because the window is too small for more. */
  runningElsewhere?: number;
  /** What has been typed in this pane's own prompt line. */
  value?: string;
  /** False leaves the pane's prompt line out (the window turned them off). */
  composer?: boolean;
  os?: Os;
  /** Replaces the scripted terminal with the real one. */
  renderTerminal?: (row: RowModel) => ReactNode;
  /** A click anywhere in the pane, or the focus moving into it, makes it the focused pane. */
  onFocus?: () => void;
  /** The header's zoom button (it restores the panes while zoomed). */
  onZoom?: () => void;
  /** The header's remove button: the pane leaves the view, the session keeps running. */
  onRemove?: () => void;
  /** A question about this session (the restart bar), drawn in the place of the pane's header. */
  bar?: ReactNode;
};

/**
 * One terminal, its 28 px header and its OWN compact prompt line at the foot. Every session on screen has an individual
 * prompt line, so there is never a question of which session a line of text goes to: the one in the pane it was typed in.
 *
 * One terminal and its 28 px header. The header carries what the glance needs when the sidebar is not in sight:
 * the status glyph (the vendor mark moving, while working), the name, the project and, only when it applies,
 * how tight the pane is. A pane that needs you never recedes with the inactive headers.
 */
export function Pane({ row, active, split, navIndex, setting, style, cells, zoomedOf, othersRunning, runningElsewhere, value, composer = true, os = "windows", renderTerminal, onFocus, onZoom, onRemove, bar }: PaneProps) {
  const needs = needsYou(row);
  // A manual CLI is a plain terminal: no prompt line of ours, and its rows go to the terminal.
  const line = composer && hasPromptLine(row);
  const zoomed = zoomedOf !== undefined;
  const running = zoomed ? (othersRunning ?? zoomedOf - 1) : 0;
  const zoomKeys = shortcutLabel(commandById("zoom-pane").binding!, os);
  const narrow = cells && cells.cols < NARROW;
  return (
    <section
      className="pane"
      data-active={active}
      data-split={split || undefined}
      data-nav-pane={navIndex}
      data-agent={row.agent}
      data-plain={!hasPromptLine(row) || undefined}
      style={line ? { ...style, gridTemplateRows: `28px minmax(0, 1fr) ${COMPOSER.compact}px` } : style}
      aria-label={`${sessionWord(row.number)}, terminal: ${row.name}`}
      onPointerDown={onFocus}
      onFocus={onFocus}
    >
      {bar ?? (
      <header className="pane__header" data-needs={needs || undefined} data-danger={headerDanger(row.mode) || undefined}>
        {row.status === "working" ? <AgentMark agent={row.agent} {...(row.agentName === undefined ? {} : { label: row.agentName })} size={16} working /> : <StatusGlyph kind={row.status} />}
        <SessionNumber n={row.number} />
        <span className="pane__name" dir="auto">
          {row.name}
        </span>
        <span className="pane__place" dir="auto">
          {row.project}
        </span>
        <span className="pane__spacer" />
        {zoomed && running > 0 && <span className="pane__running">{running} more running</span>}
        {!zoomed && runningElsewhere !== undefined && runningElsewhere > 0 && <span className="pane__running">{runningElsewhere} more running</span>}
        {narrow && (
          <Chip tone="caution" title={`This pane is narrower than ${NARROW} columns, which many CLIs need. Zoom it (${zoomKeys}) or change the layout.`}>
            {cells.cols}×{cells.rows}
          </Chip>
        )}
        <button
          type="button"
          className="pane__button"
          aria-label={`Zoom ${row.name}`}
          aria-pressed={zoomed}
          title={`${zoomed ? "Restore the panes" : "Zoom this pane"} (${zoomKeys})`}
          onClick={onZoom}
        >
          {icon(zoomed ? ZOOM_OUT : ZOOM_IN)}
        </button>
        <button type="button" className="pane__button" aria-label={`Remove ${row.name} from the view`} title="Remove from the view. The session keeps running." onClick={onRemove}>
          {icon(CLOSE)}
        </button>
      </header>
      )}
      {/* A plain terminal shows a plain shell, not an agent's conversation. */}
      {renderTerminal ? renderTerminal(row) : hasPromptLine(row) ? <ScriptedTerminal setting={setting} label={`Terminal for ${row.name}`} /> : <ScriptedTerminal setting={setting} label={`Terminal for ${row.name}`} script={shellSession({ cwd: row.project })} size={SHELL_SIZE} />}
      {line && (
        <div className="pane__composer" style={{ blockSize: COMPOSER.compact }}>
          <Composer row={row} variant="compact" value={value} os={os} focused={active} />
        </div>
      )}
    </section>
  );
}

/** A slot with no session yet: pick one that is not on screen, or start a new one. */
export function EmptyPane({ choices, active, split, navIndex, style, os = "windows", onFocus, onRemove, onPick }: { choices: RowModel[]; active: boolean; split?: boolean; navIndex?: number; style?: CSSProperties; os?: Os; onFocus?: () => void; onRemove?: () => void; onPick?: (tabId: string) => void }) {
  const keys = shortcutLabel(commandById("launcher").binding!, os);
  return (
    <section className="pane pane--empty" data-active={active}
      data-split={split || undefined} data-nav-pane={navIndex} style={style} aria-label="Empty pane" onPointerDown={onFocus} onFocus={onFocus}>
      <header className="pane__header">
        <span className="pane__name">Empty pane</span>
        <span className="pane__spacer" />
        <button type="button" className="pane__button" aria-label="Remove the empty pane from the view" title="Remove from the view" onClick={onRemove}>
          {icon(CLOSE)}
        </button>
      </header>
      <div className="pane__empty">
        <p className="pane__empty-title">Choose a session for this pane</p>
        {choices.length > 0 ? (
          <ul className="pane__choices">
            {choices.slice(0, CHOICE_LIMIT).map((r) => (
              <li key={r.id}>
                <button type="button" className="pane__choice" data-agent={r.agent} aria-label={`${sessionWord(r.number)}, ${r.name} ${r.project}, ${GLYPHS[r.status].label}`} onClick={() => onPick?.(r.id)}>
                  <span className="pane__stripe" aria-hidden="true" />
                  <StatusGlyph kind={r.status} />
                  <SessionNumber n={r.number} />{" "}
                  <span className="pane__choice-name" dir="auto">
                    {r.name}
                  </span>{" "}
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
