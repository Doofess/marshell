import { AgentMark } from "../../components/AgentMark/AgentMark";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { ContextRing } from "../../components/ContextRing/ContextRing";
import { SessionNumber } from "../../components/SessionNumber/SessionNumber";
import type { DragEventHandler } from "react";
import { RenameBox } from "./RenameBox";
import { formatDuration, formatUsage, modeCaution } from "./format";
import { rowLabel, rowTitle } from "./labels";
import { SESSION_NUMBER_HINT } from "./sessionNumber";
import { splitForMiddle } from "./truncate";
import type { Density, RowModel } from "./types";
import "./SessionRow.css";

const NEEDS_YOU = new Set(["needs-permission", "needs-question"]);

/** Sessions that cannot continue without the user stand out from the rest of the list. */
const blocked = (status: string) => (NEEDS_YOU.has(status) ? "needs-you" : status === "error" ? "error" : undefined);

/** What a row needs to be dragged to a new place in the list (native HTML5 drag and drop). Left out, the row is not draggable. */
export type RowDrag = {
  onDragStart: DragEventHandler<HTMLDivElement>;
  onDragOver: DragEventHandler<HTMLDivElement>;
  onDrop: DragEventHandler<HTMLDivElement>;
  onDragEnd: DragEventHandler<HTMLDivElement>;
  /** Where a dragged row would land relative to this one: a line is drawn on that edge. */
  dropEdge?: "before" | "after";
};
/** F2: the name is a text box. */
export type RowRename = { onCommit: (name: string) => void; onCancel: (opts: { refocus: boolean }) => void };

/**
 * One sidebar row (docs/PLAN.md "Sidebar rows"): an option in the sidebar's listbox. Height is fixed per density and
 * hover never changes it. Focus is roving: the list gives exactly one row `tabStop`, arrow keys move it (batch 2).
 * `drag` makes the row draggable, and `rename` turns its name into a text box.
 */
export function SessionRow({
  row,
  density,
  selected = false,
  tabStop = false,
  drag,
  rename,
}: {
  row: RowModel;
  density: Density;
  selected?: boolean;
  tabStop?: boolean;
  drag?: RowDrag;
  rename?: RowRename;
}) {
  const caution = modeCaution(row.mode);
  const usage = formatUsage(row.usage);
  // Unknown values are left out, not shown as a dash.
  const hasMeta = Boolean(row.model || row.effort || row.mode === "plan" || caution || row.subagents !== undefined);
  const time = row.waitingMs ?? (row.status === "done-unseen" || row.status === "done-seen" || row.status === "ended" ? row.ageMs : undefined);
  const branch = row.branch ? splitForMiddle(row.branch) : null;

  return (
    <div
      className="session-row"
      role="option"
      aria-selected={selected}
      tabIndex={tabStop ? 0 : -1}
      title={rowTitle(row)}
      aria-label={rowLabel(row, density)}
      data-density={density}
      data-agent={row.agent}
      data-number={row.number}
      data-status={row.status}
      data-needs-you={NEEDS_YOU.has(row.status) || undefined}
      data-blocked={blocked(row.status)}
      data-unseen={row.status === "done-unseen" || undefined}
      data-ended={row.status === "ended" || undefined}
      data-muted={row.muted || undefined}
      data-flash={row.flash || undefined}
      data-renaming={rename ? "" : undefined}
      draggable={drag && !rename ? true : undefined}
      data-drop={drag?.dropEdge}
      onDragStart={drag?.onDragStart}
      onDragOver={drag?.onDragOver}
      onDrop={drag?.onDrop}
      onDragEnd={drag?.onDragEnd}
    >
      <span className="session-row__stripe" aria-hidden="true" />
      {rename && <RenameBox original={row.name} onCommit={rename.onCommit} onCancel={rename.onCancel} />}
      <div className="session-row__line1" aria-hidden="true">
        <AgentMark agent={row.agent} working={row.status === "working"} />
        {row.elevated && <AuxGlyph kind="elevated" />}
        <SessionNumber n={row.number} title={SESSION_NUMBER_HINT} />{" "}
        {/* The lead clips, so whatever happens to the names, the right cluster always stays in the row. */}
        <span className="session-row__lead">
          <span className="session-row__name" dir="auto">
            {row.name}
          </span>{" "}
          {row.status === "done-unseen" && <span className="session-row__unread" />}
          <span className="session-row__project" dir="auto">
            {row.project}
          </span>
          {branch && (
            <>
              {" "}
              <span className="session-row__sep">·</span>{" "}
              {branch.head && (
                <span className="session-row__branch-head" dir="auto">
                  {branch.head}
                </span>
              )}
              <span className="session-row__branch-tail" dir="auto">
                {branch.tail}
              </span>
            </>
          )}
        </span>
        <span className="session-row__cluster">
          {caution && <AuxGlyph kind="caution" label={caution} />}
          {row.muted && <AuxGlyph kind="muted" />}
          {/* A working session shows as its vendor mark moving, so it needs no glyph. */}
          {row.status !== "working" && <StatusGlyph kind={row.status} />}
          {time !== undefined && <> <span className="session-row__time">{formatDuration(time)}</span></>}
          <ContextRing pct={row.contextPct} />
        </span>
      </div>{" "}
      <div className="session-row__phrase" aria-hidden="true">
        {row.model && (
          <>
            <span className="session-row__model">{row.model}</span>{" "}
            <span className="session-row__sep">·</span>{" "}
          </>
        )}
        <span className="session-row__status-text">
          {row.phrase}
          {row.muted && <span className="session-row__suffix"> · muted</span>}
        </span>
      </div>{" "}
      {density === "expanded" && (
        <div className="session-row__details" aria-hidden="true">
          {hasMeta && (
            <div className="session-row__meta">
              {row.model && <span>{row.model}</span>}
              {row.effort && <> <span>{row.effort}</span></>}
              {row.mode === "plan" && <> <span className="session-row__pill">Plan</span></>}
              {caution && <> <span className="session-row__caution-text">{caution}</span></>}
              {row.subagents !== undefined && <> <span>{`${row.subagents} subagents`}</span></>}
            </div>
          )}{" "}
          {row.recap && (
            <p className="session-row__recap">
              {row.recap.text} <span className="session-row__age">{formatDuration(row.recap.ageMs)} ago</span>
            </p>
          )}{" "}
          {usage && <div className="session-row__usage">{usage}</div>}
        </div>
      )}
    </div>
  );
}
