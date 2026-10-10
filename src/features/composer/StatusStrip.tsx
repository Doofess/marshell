import { Fragment } from "react";
import type { RowModel } from "../sidebar/types";
import { fitItems, statusItems, type FitItem, type PlanUsage } from "./stripItems";
import "./StatusStrip.css";

const warn = (
  <svg className="status-strip__glyph" width="10" height="10" viewBox="0 0 16 16" aria-hidden="true">
    <path d="M8 2l6.25 11H1.75L8 2z" fill="currentColor" />
  </svg>
);
const check = (
  <svg className="status-strip__glyph" width="10" height="10" viewBox="0 0 16 16" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M3.5 8.5l3 3 6-7" />
  </svg>
);

function Item({ item, first }: { item: FitItem; first: boolean }) {
  const described = item.kind !== "text";
  return (
    <span
      className="status-strip__item"
      data-id={item.id}
      data-kind={item.kind}
      data-tone={item.tone}
      data-priority={item.priority}
      data-compact={item.compact}
      data-first={first || undefined}
      role={described ? "img" : undefined}
      aria-label={described ? item.name : undefined}
      title={item.name}
    >
      {item.kind === "problem" && warn}
      {item.kind === "ok" && check}
      {item.label && (
        <>
          <span className="status-strip__label">{item.label}</span>{" "}
        </>
      )}
      <span className="status-strip__value">{item.value}</span>
      {item.bar && item.pct !== undefined && (
        <>
          {" "}
          <span className="status-strip__bar" aria-hidden="true">
            <i style={{ inlineSize: `${item.pct}%` }} />
          </span>
        </>
      )}
      {item.detail && (
        <>
          {" "}
          <span className="status-strip__detail">{item.detail}</span>
        </>
      )}
    </span>
  );
}

/**
 * One slim line of session facts, shown in the window's status bar: model, context, the plan limits, cost, time, MCP health and the
 * rest, for the session of the focused pane. Unknown values are left out. Without `width` it renders every item and the
 * stylesheet hides the least important ones as the bar narrows (so it fits in server rendering too); with `width` (in characters)
 * the items are fitted here, for places that cannot measure.
 */
export function StatusStrip({ row, plan, ports = [], width }: { row: RowModel | null; plan?: PlanUsage; ports?: string[]; width?: number }) {
  const all = row ? statusItems(row, plan, ports) : [];
  const items: FitItem[] = width === undefined ? all.map((i) => ({ ...i, compact: 0 })) : fitItems(all, width);
  return (
    <div className="status-strip" role={items.length ? "group" : undefined} aria-label={items.length ? "Session status" : undefined} data-fit={width === undefined ? "css" : "js"}>
      {items.map((item, k) => (
        <Fragment key={item.id}>
          {k > 0 && " "}
          <Item item={item} first={k === 0} />
        </Fragment>
      ))}
    </div>
  );
}
