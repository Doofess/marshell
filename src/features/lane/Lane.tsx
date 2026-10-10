import { ApproveCard } from "../approval/ApproveCard";
import type { ApprovalRequest } from "../approval/types";
import { LaneRow } from "./LaneRow";
import { laneHeading, laneLayout, orderLane, pending } from "./laneLayout";
import "./Lane.css";

/**
 * The needs-you lane (docs/PLAN.md "Layout"). The header is always present so the lane never pops in or out. The
 * oldest waiting request is the target and shows as a full card; the rest are one-liners. It is capped at 40% of the
 * sidebar and scrolls inside that, with "+N more".
 */
export function Lane({ requests, sidebarHeight }: { requests: ApprovalRequest[]; sidebarHeight: number }) {
  const ordered = orderLane(pending(requests));
  const layout = laneLayout(ordered.length, sidebarHeight);
  const heading = laneHeading(ordered.length);
  return (
    <section className="lane" aria-label="Needs you" style={{ blockSize: layout.height }} data-scrolls={layout.scrolls || undefined} data-count={ordered.length}>
      <header className="lane__header">
        <h2 className="lane__heading" role="status">
          {heading}
        </h2>
        {layout.hiddenCount > 0 && <span className="lane__more">+{layout.hiddenCount} more</span>}
      </header>
      {ordered.length > 0 && (
        <div className="lane__list" role="list">
          {ordered.map((r, i) => (
            <div key={r.id} role="listitem" className="lane__item">
              {i === 0 ? <ApproveCard request={r} /> : <LaneRow request={r} />}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
