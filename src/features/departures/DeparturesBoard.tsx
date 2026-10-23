import type { ReactNode } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { selectedProps } from "../../components/a11y/SelectedMark";
import { EmptyState } from "../../components/EmptyState/EmptyState";
import { SessionNumber } from "../../components/SessionNumber/SessionNumber";
import { SESSION_NUMBER_HINT } from "../sidebar/sessionNumber";
import { ApproveCard } from "../approval/ApproveCard";
import { APPROVALS } from "../approval/fixtures";
import type { RowModel } from "../sidebar/types";
import { boardRowLabel, boardRows, COLUMNS, fitRows, holdGroups, splitGroups, type BoardRow } from "./board";
import { FlipCell } from "./FlipCell";
import { LiveFlipCell } from "./LiveFlipCell";
import { fitLiveBoard } from "./liveBoard";
import { keyHint } from "./nav";
import { EMPTY_STATES } from "../onboarding/emptyStates";
import "./Board.css";

export type DeparturesBoardProps = {
  rows: RowModel[];
  /** The height the board has to fill, in px: only whole rows are shown and the rest are counted. */
  heightPx: number;
  /** Index (in display order) of the row the keyboard is on; clamped to the rows shown, so the board always has its one tab stop. */
  active?: number;
  /** The user's own keyboard layout (code to printed key), when the browser can tell; the key hints use it. */
  layout?: ReadonlyMap<string, string>;
  /** The row whose request is open inline. */
  card?: string;
  /** Keep the keyboard's session in view when the groups change. */
  keep?: string;
  /** Rows that closed while the pointer was over the board: kept in place, dimmed. */
  stale?: string[];
  /** Per-row text changes for the motion clip: [rowId][cell] = { from, at }. */
  flips?: Record<string, Partial<Record<"status" | "waiting", { from: string; at: number }>>>;
  /** Per-row frames for a cell that flips more than once, played from the start of the motion clip. */
  tracks?: Record<string, Partial<Record<"status" | "waiting", Array<{ t: number; text: string }>>>>;
  /** One frozen moment: texts that replace a row's own status or waiting text (the filmstrip of the motion clip). */
  texts?: Record<string, Partial<Record<"status" | "waiting", string>>>;
  reduced?: boolean;
  live?: boolean;
  hold?: { order: string[]; gate: string[] };
  renderCard?: (rowId: string) => ReactNode;
  onAllSessions?: () => void;
  onRowClick?: (rowId: string) => void;
};

const EMPTY = EMPTY_STATES.find((e) => e.id === "departures-empty")!;

/**
 * Departures: a sign, true black with 44 px rows, in both themes. Gate calls (sessions that need you, oldest waiting first)
 * sit on top. Only the status and waiting cells flip. Rows never reorder under the pointer (see freeze.ts), the keyboard
 * moves with the arrows, Enter jumps to the session and Y or N opens the request inline, but Enter never answers it. The
 * gate-call header says which keys those are, with the letter the user's own layout prints on them.
 */
export function DeparturesBoard({ rows, heightPx, active, layout, card, keep, stale = [], flips = {}, tracks = {}, texts = {}, reduced = false, live = false, hold, renderCard, onAllSessions, onRowClick }: DeparturesBoardProps) {
  const sorted = splitGroups(boardRows(rows));
  const groups = hold ? holdGroups(sorted, hold) : sorted;
  const cardContent = card === undefined ? null : renderCard ? renderCard(card) : <ApproveCard request={APPROVALS.safeBash} />;
  const hasCard = cardContent !== null && cardContent !== undefined;
  const fit = renderCard || keep !== undefined ? fitLiveBoard(heightPx, groups, hasCard ? card : undefined, keep) : fitRows(heightPx, groups, hasCard);
  const shown = [...fit.gate, ...fit.others];
  const rowsById = new Map(rows.map((r) => [r.id, r]));
  const gateIds = new Set(groups.gate.map((r) => r.id));
  // One tab stop for the whole board (a roving tabindex): the active row, clamped to the rows shown, or the first when none is.
  const current = Math.min(Math.max(0, active ?? 0), Math.max(0, shown.length - 1));

  if (groups.gate.length + groups.others.length === 0) {
    return (
      <section className="board" aria-label="Departures">
        <EmptyState title={EMPTY.title} body={EMPTY.body} action={EMPTY.action ? { label: EMPTY.action } : undefined} />
      </section>
    );
  }

  const row = (r: BoardRow) => {
    const index = shown.indexOf(r);
    const f = flips[r.id];
    const track = tracks[r.id];
    const t = texts[r.id];
    const statusNow = t?.status ?? r.statusText;
    const waitingNow = t?.waiting ?? r.waitingText;
    const source = rowsById.get(r.id);
    const gate = gateIds.has(r.id);
    return (
      <tr
        key={r.id}
        {...selectedProps(active !== undefined && current === index, "current", "board__row")}
        data-gate={gate}
        data-row-id={r.id}
        data-agent={r.agent}
        data-stale={stale.includes(r.id) || undefined}
        aria-disabled={stale.includes(r.id) || undefined}
        title={boardRowLabel(r)}
        role="row"
        tabIndex={current === index ? 0 : -1}
        onClick={onRowClick && !stale.includes(r.id) ? () => onRowClick(r.id) : undefined}
      >
        <td role="cell" className="board__status">
          {live ? <LiveFlipCell text={statusNow} reduced={reduced} /> : <FlipCell from={f?.status?.from ?? statusNow} to={statusNow} at={f?.status?.at} reduced={reduced} track={track?.status} />}
        </td>
        <td role="cell" className="board__session">
          <span className="board__sr">Session </span>
          <SessionNumber n={r.number} title={SESSION_NUMBER_HINT} />{" "}
          <span className="board__name" dir="auto">
            {r.name}
          </span>
          {/* Rows are held still while the pointer is over the board, so a row that now needs you says it will move. */}
          {!gate && statusNow === "NEEDS YOU" && <span className="board__moves">Moves up when you move away</span>}
        </td>
        <td role="cell" className="board__where">
          <span dir="auto">{r.where}</span>
        </td>
        <td role="cell" className="board__agent" data-agent={r.agent}>
          <AgentMark agent={r.agent} size={12} /> <span>{r.agentName}</span>
        </td>
        <td role="cell" className="board__waiting">
          {live ? <LiveFlipCell text={waitingNow} reduced={reduced} /> : <FlipCell from={f?.waiting?.from ?? waitingNow} to={waitingNow} at={f?.waiting?.at} reduced={reduced} track={track?.waiting} />}
        </td>
        <td role="cell" className="board__ctx">{r.ctxText}</td>
        <td role="cell" className="board__recap">
          <span dir="auto" title={source?.recap?.text}>
            {r.recap}
          </span>
        </td>
      </tr>
    );
  };

  const withCard = (r: BoardRow) =>
    card === r.id && cardContent !== null && cardContent !== undefined
      ? [
          row(r),
          <tr key={`${r.id}-card`} className="board__card" role="row">
            <td role="cell" colSpan={COLUMNS.length}>
              {cardContent}
            </td>
          </tr>,
        ]
      : row(r);

  return (
    <section className="board" aria-label="Departures">
      {/* Explicit roles: the grid layout in Board.css would otherwise drop the table's own semantics in some browsers. */}
      <table className="board__table" role="table" aria-label="Sessions">
        <thead role="rowgroup">
          <tr role="row">
            {COLUMNS.map((c, i) => (
              <th key={c} role="columnheader" scope="col" className={`board__th board__th--${i}`}>
                {c}
              </th>
            ))}
          </tr>
        </thead>
        {fit.gate.length > 0 && (
          <tbody role="rowgroup">
            <tr className="board__group" role="row">
              <th role="rowheader" scope="rowgroup" colSpan={COLUMNS.length}>
                Gate calls · {groups.gate.length}
                <span className="board__keys">
                  {keyHint("KeyY", layout)} opens the request, {keyHint("KeyN", layout)} opens it on Deny
                </span>
              </th>
            </tr>
            {fit.gate.flatMap(withCard)}
          </tbody>
        )}
        {fit.others.length > 0 && (
          <tbody role="rowgroup">
            <tr className="board__group" role="row">
              <th role="rowheader" scope="rowgroup" colSpan={COLUMNS.length}>
                Other sessions · {groups.others.length}
              </th>
            </tr>
            {fit.others.flatMap(withCard)}
          </tbody>
        )}
        {fit.hidden > 0 && (
          <tbody role="rowgroup">
            <tr className="board__more" role="row">
              <td role="cell" colSpan={COLUMNS.length}>
                +{fit.hidden} more {fit.hidden === 1 ? "session" : "sessions"}{" "}
                <button type="button" className="board__all" onClick={onAllSessions}>
                  All sessions
                </button>
              </td>
            </tr>
          </tbody>
        )}
      </table>
    </section>
  );
}
