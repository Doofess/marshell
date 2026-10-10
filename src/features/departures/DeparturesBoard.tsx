import { AgentMark } from "../../components/AgentMark/AgentMark";
import { selectedProps } from "../../components/a11y/SelectedMark";
import { EmptyState } from "../../components/EmptyState/EmptyState";
import { SessionNumber } from "../../components/SessionNumber/SessionNumber";
import { SESSION_NUMBER_HINT } from "../sidebar/sessionNumber";
import { ApproveCard } from "../approval/ApproveCard";
import { APPROVALS } from "../approval/fixtures";
import type { RowModel } from "../sidebar/types";
import { boardRowLabel, boardRows, COLUMNS, fitRows, splitGroups, type BoardRow } from "./board";
import { FlipCell } from "./FlipCell";
import { EMPTY_STATES } from "../onboarding/emptyStates";
import "./Board.css";

export type DeparturesBoardProps = {
  rows: RowModel[];
  /** The height the board has to fill, in px: only whole rows are shown and the rest are counted. */
  heightPx: number;
  /** Index (in display order) of the row the keyboard is on. */
  active?: number;
  /** The row whose request is open inline. */
  card?: string;
  /** Rows that closed while the pointer was over the board: kept in place, dimmed. */
  stale?: string[];
  /** Per-row text changes for the motion clip: [rowId][cell] = { from, at }. */
  flips?: Record<string, Partial<Record<"status" | "waiting", { from: string; at: number }>>>;
  /** Per-row frames for a cell that flips more than once, played from the start of the motion clip. */
  tracks?: Record<string, Partial<Record<"status" | "waiting", Array<{ t: number; text: string }>>>>;
  /** One frozen moment: texts that replace a row's own status or waiting text (the filmstrip of the motion clip). */
  texts?: Record<string, Partial<Record<"status" | "waiting", string>>>;
  reduced?: boolean;
};

const EMPTY = EMPTY_STATES.find((e) => e.id === "departures-empty")!;

/**
 * Departures: a sign, true black with 44 px rows, in both themes. Gate calls (sessions that need you, oldest waiting first)
 * sit on top. Only the status and waiting cells flip. Rows never reorder under the pointer (see freeze.ts), the keyboard
 * moves with the arrows, Enter jumps to the session and Y or N opens the request inline, but Enter never answers it.
 */
export function DeparturesBoard({ rows, heightPx, active, card, stale = [], flips = {}, tracks = {}, texts = {}, reduced = false }: DeparturesBoardProps) {
  const groups = splitGroups(boardRows(rows));
  const fit = fitRows(heightPx, groups);
  const shown = [...fit.gate, ...fit.others];

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
    const source = rows.find((x) => x.id === r.id)!;
    return (
      <tr
        key={r.id}
        {...selectedProps(active === index, "current", "board__row")}
        data-gate={r.gate}
        data-agent={r.agent}
        data-stale={stale.includes(r.id) || undefined}
        title={boardRowLabel(r)}
        tabIndex={active === index ? 0 : -1}
      >
        <td className="board__status">
          <FlipCell from={f?.status?.from ?? statusNow} to={statusNow} at={f?.status?.at} reduced={reduced} track={track?.status} />
        </td>
        <td className="board__session">
          <span className="board__sr">Session </span>
          <SessionNumber n={r.number} title={SESSION_NUMBER_HINT} />{" "}
          <span className="board__name" dir="auto">
            {r.name}
          </span>
        </td>
        <td className="board__where">
          <span dir="auto">{r.where}</span>
        </td>
        <td className="board__agent" data-agent={r.agent}>
          <AgentMark agent={r.agent} size={12} /> <span>{r.agentName}</span>
        </td>
        <td className="board__waiting">
          <FlipCell from={f?.waiting?.from ?? waitingNow} to={waitingNow} at={f?.waiting?.at} reduced={reduced} track={track?.waiting} />
        </td>
        <td className="board__ctx">{r.ctxText}</td>
        <td className="board__recap">
          <span dir="auto" title={source.recap?.text}>
            {r.recap}
          </span>
        </td>
      </tr>
    );
  };

  const withCard = (r: BoardRow) =>
    card === r.id
      ? [
          row(r),
          <tr key={`${r.id}-card`} className="board__card">
            <td colSpan={COLUMNS.length}>
              <ApproveCard request={APPROVALS.safeBash} />
            </td>
          </tr>,
        ]
      : row(r);

  return (
    <section className="board" aria-label="Departures">
      <table className="board__table">
        <thead>
          <tr>
            {COLUMNS.map((c, i) => (
              <th key={c} scope="col" className={`board__th board__th--${i}`}>
                {c}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {fit.gate.length > 0 && (
            <tr className="board__group">
              <th scope="colgroup" colSpan={COLUMNS.length}>
                Gate calls · {groups.gate.length}
              </th>
            </tr>
          )}
          {fit.gate.map(withCard)}
          {fit.others.length > 0 && (
            <tr className="board__group">
              <th scope="colgroup" colSpan={COLUMNS.length}>
                Other sessions · {groups.others.length}
              </th>
            </tr>
          )}
          {fit.others.map(withCard)}
          {fit.hidden > 0 && (
            <tr className="board__more">
              <td colSpan={COLUMNS.length}>
                +{fit.hidden} more sessions{" "}
                <button type="button" className="board__all">
                  All sessions
                </button>
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}
