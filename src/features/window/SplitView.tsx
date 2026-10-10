import type { CSSProperties } from "react";
import type { Os } from "../../lib/keymap";
import type { RowModel } from "../sidebar/types";
import type { TerminalThemeSetting } from "../terminal/terminalTheme";
import { EmptyPane, Pane } from "./Pane";
import { ARRANGEMENTS, CELL, PANE_LIMIT, RATIO, chooseArrangement, paneCells, planFor, tracks, type Arrangement, type Handle, type PaneCount, type Plan, type Ratios, type Size } from "./splitLayout";
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
};

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
export function SplitView({ panes, size, activeIndex = 0, arrangement, ratios = {}, zoomed, choices = [], hidden = 0, composerValues = {}, composers = true, setting = "follow-app", os = "windows" }: SplitViewProps) {
  if (panes.length > PANE_LIMIT) throw new Error(`At most ${PANE_LIMIT} panes can be on screen`);
  const isZoomed = zoomed !== undefined && panes[zoomed] != null;
  const count = (isZoomed ? 1 : Math.max(1, panes.length)) as PaneCount;
  // An arrangement for another number of panes falls back to automatic instead of throwing.
  const valid = arrangement !== undefined && ARRANGEMENTS[count]?.includes(arrangement);
  // Every pane carries its own prompt line, so each costs its height: the arrangement and the narrow warning count it.
  const lineH = composers ? CELL.composer : 0;
  const chosen = isZoomed ? "single" : valid ? arrangement : chooseArrangement(count, size, {}, lineH);
  const plan = planFor(count, chosen);
  const cells = paneCells(plan, size, ratios, lineH);
  const visible = isZoomed ? [{ row: panes[zoomed!]!, index: zoomed! }] : panes.map((row, index) => ({ row, index }));

  return (
    <div
      className="split"
      data-arrangement={chosen}
      data-auto={!valid}
      style={{ gridTemplateColumns: tracks(plan.colTracks, ratios.col), gridTemplateRows: tracks(plan.rowTracks, ratios.row) }}
    >
      {visible.map(({ row, index }, k) =>
        row ? (
          <Pane key={row.id} row={row} active={index === activeIndex} setting={setting} style={gridArea(plan.cells[k]!)} cells={cells[k]} zoomedOf={isZoomed ? panes.length : undefined} runningElsewhere={index === activeIndex && hidden > 0 ? hidden : undefined} value={composerValues[row.id]} composer={composers} os={os} />
        ) : (
          <EmptyPane key={`empty-${index}`} choices={choices} active={index === activeIndex} style={gridArea(plan.cells[k]!)} os={os} />
        ),
      )}
      {plan.handles.map((h, i) => {
        const v = handleValues(plan, h, ratios);
        return (
          <div
            key={i}
            className="split__handle"
            data-orientation={h.orientation}
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
