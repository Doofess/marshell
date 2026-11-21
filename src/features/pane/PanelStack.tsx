import { Fragment, useContext, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { useLayoutMotion } from "../window/motion";
import { LayoutContext, type Layout } from "../window/paneColumn";
import "./PanelStack.css";
import { PANELS, PANEL_LABEL, type PanelId, type Weights } from "./panels";
import { STACK, belowPerPx, movedPair, pairWeights, stackHeights, stackKey, weightOf } from "./stackLayout";

export type PanelStackProps = {
  /** The stack's panels (`stackPanels`), in order. The same while the pane is hidden or folding. */
  panels: readonly PanelId[];
  weights: Weights;
  /** One panel's frame. Called for a panel that is leaving too, until its motion ends. */
  render(panel: PanelId): ReactNode;
  onWeights(weights: Weights): void;
  /** The divider went away while it had the keyboard. */
  onDividerGone?(): void;
};

export function dividerProps(upper: PanelId, lower: PanelId, upperPx: number, lowerPx: number): { "aria-label": string; "aria-valuemin": number; "aria-valuemax": number; "aria-valuenow": number } {
  // A pane too short for two minimums has nothing to move: the range is the one height it has.
  const room = upperPx + lowerPx >= 2 * STACK.min;
  return {
    "aria-label": `Resize ${PANEL_LABEL[upper]} and ${PANEL_LABEL[lower]}`,
    "aria-valuemin": room ? STACK.min : upperPx,
    "aria-valuemax": room ? upperPx + lowerPx - STACK.min : upperPx,
    "aria-valuenow": upperPx,
  };
}

/** The pair's heights after a key, or null for a key the divider doesn't use. */
export function dividerKey(key: string, upperPx: number, lowerPx: number): { upper: number; lower: number } | null {
  if (key === "ArrowUp") return movedPair(upperPx, lowerPx, -STACK.step);
  if (key === "ArrowDown") return movedPair(upperPx, lowerPx, STACK.step);
  if (key === "Home") return movedPair(upperPx, lowerPx, STACK.min - upperPx);
  if (key === "End") return movedPair(upperPx, lowerPx, lowerPx - STACK.min);
  return null;
}

/**
 * The layout a row's page slot sees: the window's own while nothing in the stack moves and nothing sits below, else with the
 * stack's motion and the share of the stack below the row. The same object when it would add nothing.
 */
export function rowLayout(outer: Layout | null, moving: boolean, bottomPerPx: number): Layout | null {
  if (outer === null) return null;
  if (!moving && bottomPerPx === 0) return outer;
  return { ...outer, moving: outer.moving || moving, bottomPerPx };
}

/** One row's frame. It owns its layout object: a page's placement effect restarts when the context value changes identity, so a new object per frame would keep it from ever reporting. */
function StackRow({ panel, moving, rowMoving, bottomPerPx, children }: { panel: PanelId; moving: boolean; rowMoving: boolean; bottomPerPx: number; children: ReactNode }) {
  const outer = useContext(LayoutContext);
  const value = useMemo(() => rowLayout(outer, moving, bottomPerPx), [outer, moving, bottomPerPx]);
  return (
    <div className="panel-stack__row" data-panel={panel} data-moving={rowMoving ? "" : undefined}>
      <LayoutContext.Provider value={value}>{children}</LayoutContext.Provider>
    </div>
  );
}

export function PanelStack({ panels, weights, render, onWeights, onDividerGone }: PanelStackProps) {
  const root = useRef<HTMLDivElement>(null);
  // Null until the root has been measured (the first render, and every render in a test with no DOM).
  const [total, setTotal] = useState<number | null>(null);
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    setTotal(el.clientHeight);
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => setTotal(el.clientHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const heights = total === null ? null : stackHeights(total, panels, weights);
  const key = stackKey(panels);
  // One motion per panel. The list is the constant PANELS, so the hooks are called in the same order on every render: one call per
  // entry, written out because a loop over a constant is still a loop to the lint rule.
  const motions: Record<PanelId, { value: number; moving: boolean }> = {
    browser: useLayoutMotion(key, heights?.browser ?? 0),
    files: useLayoutMotion(key, heights?.files ?? 0),
  };
  const anyMoving = PANELS.some((p) => motions[p].moving);
  const drawn = PANELS.filter((p) => panels.includes(p) || motions[p].value > 0);

  // The divider is live when it can be dragged: measured, both rows are panels of the stack, and nothing moves.
  const [upper, lower] = drawn;
  const live = heights !== null && !anyMoving && drawn.length === 2 && panels.includes(upper!) && panels.includes(lower!);
  const upperPx = heights?.[upper!] ?? 0;
  const lowerPx = heights?.[lower!] ?? 0;

  // The divider is one element through a motion, so focus on it stays. When it goes away for good (fewer than two panels) with
  // the keyboard on it, the keyboard is handed on.
  const dividerFocused = useRef(false);
  const dividerWasLive = useRef(live);
  const dividerGone = useRef(onDividerGone);
  dividerGone.current = onDividerGone;
  useLayoutEffect(() => {
    if (dividerWasLive.current && !live && panels.length < 2 && dividerFocused.current) {
      dividerFocused.current = false;
      dividerGone.current?.();
    }
    dividerWasLive.current = live;
  });

  const start = useRef<{ y: number; upper: number; lower: number } | null>(null);
  const move = (pair: { upper: number; lower: number }) => {
    if (!live) return;
    // The same heights (at a limit, or in a pane too short) write nothing.
    if (pair.upper === upperPx && pair.lower === lowerPx) return;
    onWeights(pairWeights(weights, upper!, lower!, pair.upper, pair.lower));
  };
  const onPointerDown = (e: ReactPointerEvent<HTMLElement>) => {
    if (!live || e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    start.current = { y: e.clientY, upper: upperPx, lower: lowerPx };
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLElement>) => {
    const s = start.current;
    if (!s || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
    move(movedPair(s.upper, s.lower, e.clientY - s.y));
  };
  const endDrag = () => {
    start.current = null;
  };
  const onKeyDown = (e: KeyboardEvent<HTMLElement>) => {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const pair = dividerKey(e.key, upperPx, lowerPx);
    if (!pair) return;
    e.preventDefault();
    move(pair);
  };

  const tracks: string[] = [];
  drawn.forEach((p, i) => {
    if (i > 0) tracks.push(`${STACK.divider}px`);
    tracks.push(heights === null ? `minmax(0, ${weightOf(weights, p)}fr)` : `${motions[p].value}px`);
  });
  const style: CSSProperties = { gridTemplateRows: tracks.join(" ") };

  return (
    <div ref={root} className="panel-stack" data-drop-pane="" style={style}>
      {drawn.map((p, i) => (
        <Fragment key={p}>
          {i > 0 && (
            // One element, whether live or not, so the keyboard focus on it survives a motion that ends with it live again.
            <div
              className="panel-stack__divider"
              {...(live
                ? {
                    role: "separator",
                    "aria-orientation": "horizontal" as const,
                    tabIndex: 0,
                    ...dividerProps(upper!, lower!, upperPx, lowerPx),
                    onPointerDown,
                    onPointerMove,
                    onPointerUp: endDrag,
                    onPointerCancel: endDrag,
                    onLostPointerCapture: endDrag,
                    onKeyDown,
                    onDoubleClick: () => onWeights(pairWeights(weights, upper!, lower!, 1, 1)),
                    onFocus: () => {
                      dividerFocused.current = true;
                    },
                    onBlur: () => {
                      dividerFocused.current = false;
                    },
                  }
                : { "aria-hidden": true })}
            />
          )}
          <StackRow panel={p} moving={anyMoving} rowMoving={motions[p].moving} bottomPerPx={total === null ? 0 : belowPerPx(total, panels, weights, p)}>
            {render(p)}
          </StackRow>
        </Fragment>
      ))}
    </div>
  );
}
