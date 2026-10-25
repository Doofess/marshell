import { AgentMark } from "../../components/AgentMark/AgentMark";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { ContextRing } from "../../components/ContextRing/ContextRing";
import { SessionNumber } from "../../components/SessionNumber/SessionNumber";
import type { PointerEventHandler } from "react";
import type { Os } from "../../lib/keymap";
import { RenameBox } from "./RenameBox";
import { inRenameBox, keyboardAnchor, opensRowMenu } from "./rowMenu";
import { togglesPane, togglesPaneKey } from "./rowPick";
import { formatAge, formatUsage, modeCaution, subagentsText } from "./format";
import { rowLabel, rowTitle } from "./labels";
import { SESSION_NUMBER_HINT } from "./sessionNumber";
import { splitForMiddle } from "./truncate";
import type { Density, RowModel } from "./types";
import "./SessionRow.css";

const NEEDS_YOU = new Set(["needs-permission", "needs-question"]);

/** Sessions that cannot continue without the user stand out from the rest of the list. */
const blocked = (status: string) => (NEEDS_YOU.has(status) ? "needs-you" : status === "error" ? "error" : undefined);

/** What a row needs to be dragged to a new place in the list (pointer events, so native file drop can be on). Left out, the row is not a drag source or a drop place. */
export type RowDrag = {
  /** Where a dragged row would land relative to this one: a line is drawn on that edge. */
  dropEdge?: "before" | "after";
  /** This row is the one being dragged. */
  dragging: boolean;
  onPointerDown: PointerEventHandler<HTMLDivElement>;
};
/** What a row does for Peek: Space on the focused row, and the mouse over or off the row. */
export type RowPeek = { onOpen: (el: HTMLElement) => void; onHover: (el: HTMLElement) => void; onLeave: () => void };
/** F2: the name is a text box. */
export type RowRename = { onCommit: (name: string) => void; onCancel: (opts: { refocus: boolean }) => void };

/**
 * One sidebar row (docs/PLAN.md "Sidebar rows"): an option in the sidebar's listbox. Height is fixed per density and
 * hover never changes it. Focus is roving: the list gives exactly one row `tabStop`, arrow keys move it (batch 2).
 * `drag` lets the row be dragged by pointer, and `rename` turns its name into a text box.
 */
export function SessionRow({
  row,
  density,
  selected = false,
  focused = false,
  os = "windows",
  tabStop = false,
  drag,
  rename,
  onSelect,
  onToggle,
  onReview,
  onSetup,
  onMenu,
  peek,
}: {
  row: RowModel;
  density: Density;
  selected?: boolean;
  /** The row of the focused pane: with several rows selected, the one that also gets the bar. */
  focused?: boolean;
  /** Decides which click toggles a pane (Cmd on macOS, Ctrl elsewhere). */
  os?: Os;
  tabStop?: boolean;
  drag?: RowDrag;
  rename?: RowRename;
  /** A click on the row. Left out, a click does nothing (stories). */
  onSelect?: () => void;
  /** Ctrl+click (Cmd on macOS) or Ctrl+Space: add or remove this session's pane. Left out, a click is always a plain click. */
  onToggle?: () => void;
  /** Opens the review of this row's task. Left out, a task row shows its tag but no button (stories). */
  onReview?: () => void;
  /** Opens the hooks setup. Shown on a row with `setupLink`; left out, there is no button (stories). */
  onSetup?: () => void;
  /** Right-click, Shift+F10 or the Menu key: where to open the row's menu. Left out, the row has no menu. */
  onMenu?: (anchor: { x: number; y: number }) => void;
  /** Space opens Peek (focus goes into it); a mouse resting on the row opens it after a delay. Left out, the row has no Peek. */
  peek?: RowPeek;
}) {
  const caution = modeCaution(row.mode);
  const usage = formatUsage(row.usage);
  // Unknown values are left out, not shown as a dash.
  // The second line carries the count in a comfortable row; an expanded one shows it on its meta line instead.
  const lineSubagents = density !== "expanded" && row.subagents !== undefined;
  const hasMeta = Boolean(row.model || row.effort || row.mode === "plan" || caution || row.subagents !== undefined);
  const time = row.waitingMs ?? (row.status === "done-unseen" || row.status === "done-seen" || row.status === "ended" ? row.ageMs : undefined);
  const branch = row.branch ? splitForMiddle(row.branch) : null;
  const reviewable = Boolean(row.task && onReview && !rename);
  const settable = Boolean(row.setupLink && onSetup && !rename);

  return (
    <div
      className="session-row"
      role="option"
      aria-selected={selected}
      data-focused={focused || undefined}
      aria-haspopup={onMenu ? "menu" : undefined}
      tabIndex={tabStop ? 0 : -1}
      title={rowTitle(row)}
      aria-label={rowLabel(row, density)}
      data-density={density}
      data-agent={row.agent}
      data-number={row.number}
      data-nav-row={row.number}
      data-status={row.status}
      data-needs-you={NEEDS_YOU.has(row.status) || undefined}
      data-blocked={blocked(row.status)}
      data-unseen={row.status === "done-unseen" || undefined}
      data-ended={row.status === "ended" || undefined}
      data-muted={row.muted || undefined}
      data-flash={row.flash || undefined}
      data-renaming={rename ? "" : undefined}
      data-review={reviewable ? (selected ? "open" : "") : undefined}
      data-setup={settable ? "" : undefined}
      data-row-id={drag ? row.id : undefined}
      data-dragging={drag?.dragging ? "" : undefined}
      data-drop={drag?.dropEdge}
      // Not while the name is a text box: a press there selects text.
      onPointerDown={drag && !rename ? drag.onPointerDown : undefined}
      onClick={onToggle ? (e) => (togglesPane(e, os) ? onToggle() : onSelect?.()) : onSelect}
      onContextMenu={
        onMenu &&
        ((e) => {
          // The rename box keeps its own native menu (cut, copy, paste); opening ours would cancel the rename.
          if (inRenameBox(e.target)) return;
          e.preventDefault();
          onMenu({ x: e.clientX, y: e.clientY });
        })
      }
      onPointerEnter={peek && !rename ? (e) => e.pointerType === "mouse" && peek.onHover(e.currentTarget) : undefined}
      onPointerLeave={peek ? () => peek.onLeave() : undefined}
      onKeyDown={
        (onMenu || onToggle || peek) &&
        ((e) => {
          if (peek && !rename && e.key === " " && e.target === e.currentTarget && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
            e.preventDefault();
            peek.onOpen(e.currentTarget);
          } else if (onMenu && opensRowMenu(e)) {
            e.preventDefault();
            e.stopPropagation();
            onMenu(keyboardAnchor(e.currentTarget.getBoundingClientRect()));
          } else if (onToggle && togglesPaneKey(e)) {
            e.preventDefault();
            onToggle();
          }
        })
      }
    >
      <span className="session-row__stripe" aria-hidden="true" />
      {focused && <span className="session-row__bar" aria-hidden="true" />}
      {rename && <RenameBox original={row.name} onCommit={rename.onCommit} onCancel={rename.onCancel} />}
      <div className="session-row__line1" aria-hidden="true">
        <AgentMark agent={row.agent} {...(row.agentName === undefined ? {} : { label: row.agentName })} working={row.status === "working"} />
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
          {row.task && (
            <>
              {" "}
              <span className="session-row__task" title={row.branch ? `Task on ${row.branch}` : "Task"}>
                Task
              </span>
            </>
          )}
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
          {time !== undefined && <> <span className="session-row__time">{formatAge(time)}</span></>}
          <ContextRing pct={row.contextPct} />
        </span>
      </div>{" "}
      <div className="session-row__phrase" aria-hidden="true">
        {row.model && (
          <>
            <span className="session-row__model">{row.model}</span>{" "}
            {(row.phrase || row.muted || lineSubagents) && <><span className="session-row__sep">·</span>{" "}</>}
          </>
        )}
        <span className="session-row__status-text">
          {row.phrase}
          {lineSubagents && <span className="session-row__suffix">{row.phrase ? " · " : ""}{subagentsText(row.subagents!)}</span>}
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
              {row.subagents !== undefined && <> <span>{subagentsText(row.subagents)}</span></>}
            </div>
          )}{" "}
          {row.recap && (
            <p className="session-row__recap">
              {row.recap.text} <span className="session-row__age">{formatAge(row.recap.ageMs)} ago</span>
            </p>
          )}{" "}
          {usage && <div className="session-row__usage">{usage}</div>}
        </div>
      )}
      {/* Outside the aria-hidden lines. Rows have no other in-row actions (close, rename and move are keys and palette commands), so this is the one nested control: ARIA treats an option's children as presentational, so a screen reader reaches it by focus (Tab from the row), and reaches the review without it by Ctrl+Shift+R (Cmd+R) and the palette. It follows the row's tab stop: Tab from the row lands here, and the arrow keys never do. */}
      {reviewable && (
        <button
          type="button"
          className="session-row__review"
          aria-label={`Review changes, ${row.name}`}
          tabIndex={tabStop ? 0 : -1}
          onClick={(e) => {
            e.stopPropagation();
            onReview?.();
          }}
        >
          Review changes
        </button>
      )}
      {/* Same rules as the review button: outside the aria-hidden lines, in the row's tab order, and a click here is not a click on the row. */}
      {settable && (
        <button
          type="button"
          className="session-row__setup"
          aria-label={`Set up status for ${row.name}`}
          tabIndex={tabStop ? 0 : -1}
          onClick={(e) => {
            e.stopPropagation();
            onSetup?.();
          }}
          onKeyDown={(e) => {
            // Enter and Space press the button; the row's own key handling (menu, pane toggle) never sees them.
            if (e.key === "Enter" || e.key === " ") e.stopPropagation();
          }}
        >
          set up
        </button>
      )}
    </div>
  );
}
