import { useRef, type CSSProperties, type KeyboardEvent, type PointerEvent, type ReactNode } from "react";
import type { Os } from "../../lib/keymap";
import { hasPromptLine } from "../sidebar/promptLine";
import type { RowModel } from "../sidebar/types";
import type { TerminalThemeSetting } from "../terminal/terminalTheme";
import { EmptyPane, Pane } from "./Pane";
import { ARRANGEMENTS, CELL, HANDLE, PANE_LIMIT, RATIO, chooseArrangement, dragRatio, paneCells, planFor, tracks, type Arrangement, type Handle, type PaneCount, type Plan, type Ratios, type Size } from "./splitLayout";
import "./SplitView.css";

export type SplitViewProps = {
  /** One entry per pane, two to four; `null` is an empty slot. */
  panes: (RowModel | null)[];
  /** The space the panes share. */
  size: Size;
  activeIndex?: number;
  /** Leave out to pick the arrangement that suits the space. */
  arrangement?: Arrangement;
  ratios?: Ratios;
  /** Index of the pane that fills the view. */
  zoomed?: number;
  /** Sessions not on screen, offered by an empty pane. */
  choices?: RowModel[];
  /** Sessions that are running but not shown because the window is too small for more panes. */
  hidden?: number;
  /** What has been typed in each pane's own prompt line, by session id. */
  composerValues?: Record<string, string>;
  /** False turns every pane's prompt line off, and gives their rows back to the terminals. */
  composers?: boolean;
  setting?: TerminalThemeSetting;
  os?: Os;
  /** Replaces each pane's scripted terminal with the real one. */
  renderTerminal?: (row: RowModel) => ReactNode;
  /** A click in a pane, or the focus moving into it, gives its index. */
  onFocus?: (index: number) => void;
  /** A bar for one session, which replaces that pane's header (when the session has a pane here). */
  paneBar?: { id: string; node: ReactNode };
  /** A pane header's zoom button gives the pane's index. */
  onZoom?: (index: number) => void;
  /** A pane header's remove button gives the pane's index. */
  onRemove?: (index: number) => void;
  /** A choice in an empty pane gives the pane's index and the session. */
  onPick?: (index: number, tabId: string) => void;
  /** A divider dragged or moved with the arrow keys gives the whole new ratios, kept so every pane stays usable. */
  onRatios?: (ratios: Ratios) => void;
};

/** How far one arrow key press moves a divider. */
const NUDGE = 0.02;

const gridArea = (a: { rowStart: number; colStart: number; rowEnd: number; colEnd: number }): CSSProperties => ({ gridArea: `${a.rowStart} / ${a.colStart} / ${a.rowEnd} / ${a.colEnd}` });

/** What the separator reports, 20 to 80 for a two-way split. More than two equal tracks report their position only. */
function handleValues(plan: Plan, h: Handle, ratios: Ratios) {
  const tr = h.orientation === "vertical" ? plan.colTracks : plan.rowTracks;
  const ratio = h.orientation === "vertical" ? ratios.col : ratios.row;
  const index = plan.handles.filter((x) => x.orientation === h.orientation).indexOf(h);
  if (tr === 2) return { now: Math.round(Math.min(RATIO.max, Math.max(RATIO.min, ratio ?? RATIO.default)) * 100), min: RATIO.min * 100, max: RATIO.max * 100 };
  return { now: Math.round(((index + 1) / tr) * 100), min: undefined, max: undefined };
}

function handleLabel(plan: Plan, h: Handle): string {
  if (plan.arrangement === "columns" || plan.arrangement === "rows") {
    const i = plan.handles.indexOf(h);
    return `Resize panes ${i + 1} and ${i + 2}`;
  }
  return h.orientation === "vertical" ? "Resize the left and right panes" : "Resize the top and bottom panes";
}

/**
 * Up to four terminals at once. Auto picks the arrangement that keeps the smallest pane closest to 80 by 24 (judged on even
 * panes, so dragging a divider never flips the layout under the user's hand);
 * the user's choice stands once made. Dividers cost one pixel (the 8 px hit area overhangs the panes). A zoomed
 * pane fills the view and the others keep running.
 */
export function SplitView({ panes, size, activeIndex = 0, arrangement, ratios = {}, zoomed, choices = [], hidden = 0, composerValues = {}, composers = true, setting = "follow-app", os = "windows", renderTerminal, paneBar, onFocus, onZoom, onRemove, onPick, onRatios }: SplitViewProps) {
  const splitRef = useRef<HTMLDivElement>(null);
  if (panes.length > PANE_LIMIT) throw new Error(`At most ${PANE_LIMIT} panes can be on screen`);
  const isZoomed = zoomed !== undefined && panes[zoomed] != null;
  const count = (isZoomed ? 1 : Math.max(1, panes.length)) as PaneCount;
  // An arrangement for another number of panes falls back to automatic instead of throwing.
  const valid = arrangement !== undefined && ARRANGEMENTS[count]?.includes(arrangement);
  // Every pane carries its own prompt line, so each costs its height: the arrangement and the narrow warning count it.
  const visible = isZoomed ? [{ row: panes[zoomed!]!, index: zoomed! }] : panes.map((row, index) => ({ row, index }));
  // A manual CLI is a plain terminal with no prompt line, so its pane gets those rows back; the arrangement is still judged on the line the others carry.
  const lines = visible.map(({ row }) => (composers && hasPromptLine(row) ? CELL.composer : 0));
  const lineH = lines.some((h) => h > 0) ? CELL.composer : 0;
  const chosen = isZoomed ? "single" : valid ? arrangement : chooseArrangement(count, size, {}, lineH);
  const split = !isZoomed && panes.length >= 2;
  const plan = planFor(count, chosen);
  const cells = paneCells(plan, size, ratios, lines);

  /** Only a two-way split follows a ratio; three or more equal tracks have nothing to drag. */
  const draggable = (h: Handle) => onRatios !== undefined && (h.orientation === "vertical" ? plan.colTracks : plan.rowTracks) === 2;
  const move = (h: Handle, wanted: number) => {
    const axis = h.orientation === "vertical" ? "col" : "row";
    onRatios?.({ ...ratios, [axis]: dragRatio(plan, size, ratios, lines, axis, wanted) });
  };
  /** The pointer's place along the divider's axis, as a share of the two tracks. Counted from the right edge in a right-to-left window. */
  const dragTo = (h: Handle, e: PointerEvent<HTMLElement>) => {
    const el = splitRef.current;
    if (!el || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
    const rect = el.getBoundingClientRect();
    const vertical = h.orientation === "vertical";
    const total = (vertical ? rect.width : rect.height) - HANDLE;
    if (total <= 0) return;
    const rtl = vertical && getComputedStyle(el).direction === "rtl";
    const offset = vertical ? (rtl ? rect.right - e.clientX : e.clientX - rect.left) : e.clientY - rect.top;
    move(h, offset / total);
  };
  const nudge = (h: Handle, e: KeyboardEvent<HTMLElement>) => {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const vertical = h.orientation === "vertical";
    const rtl = vertical && splitRef.current !== null && getComputedStyle(splitRef.current).direction === "rtl";
    const back = vertical ? (rtl ? "ArrowRight" : "ArrowLeft") : "ArrowUp";
    const forward = vertical ? (rtl ? "ArrowLeft" : "ArrowRight") : "ArrowDown";
    if (e.key !== back && e.key !== forward) return;
    e.preventDefault();
    const now = (vertical ? ratios.col : ratios.row) ?? RATIO.default;
    move(h, now + (e.key === forward ? NUDGE : -NUDGE));
  };
  const at = <A extends unknown[]>(f: ((index: number, ...a: A) => void) | undefined, index: number) => (f ? (...a: A) => f(index, ...a) : undefined);

  return (
    <div
      ref={splitRef}
      className="split"
      data-arrangement={chosen}
      data-auto={!valid}
      style={{ gridTemplateColumns: tracks(plan.colTracks, ratios.col), gridTemplateRows: tracks(plan.rowTracks, ratios.row) }}
    >
      {visible.map(({ row, index }, k) =>
        row ? (
          <Pane key={row.id} row={row} active={index === activeIndex} split={split} navIndex={index} setting={setting} style={gridArea(plan.cells[k]!)} cells={cells[k]} zoomedOf={isZoomed ? panes.length : undefined} othersRunning={isZoomed ? panes.filter(Boolean).length - 1 : undefined} runningElsewhere={index === activeIndex && hidden > 0 ? hidden : undefined} value={composerValues[row.id]} composer={composers} os={os} renderTerminal={renderTerminal} bar={paneBar?.id === row.id ? paneBar.node : undefined} onFocus={at(onFocus, index)} onZoom={at(onZoom, index)} onRemove={at(onRemove, index)} />
        ) : (
          <EmptyPane key={`empty-${index}`} choices={choices} active={index === activeIndex} split={split} navIndex={index} style={gridArea(plan.cells[k]!)} os={os} onFocus={at(onFocus, index)} onRemove={at(onRemove, index)} onPick={at(onPick, index)} />
        ),
      )}
      {plan.handles.map((h, i) => {
        const v = handleValues(plan, h, ratios);
        const drags = draggable(h);
        return (
          <div
            key={i}
            className="split__handle"
            data-orientation={h.orientation}
            onPointerDown={drags ? (e) => e.button === 0 && e.currentTarget.setPointerCapture(e.pointerId) : undefined}
            onPointerMove={drags ? (e) => dragTo(h, e) : undefined}
            onKeyDown={drags ? (e) => nudge(h, e) : undefined}
            style={gridArea(h)}
            role="separator"
            aria-orientation={h.orientation}
            aria-label={handleLabel(plan, h)}
            aria-valuenow={v.now}
            aria-valuemin={v.min}
            aria-valuemax={v.max}
            tabIndex={0}
          />
        );
      })}
    </div>
  );
}
