import { ApproveCard } from "../approval/ApproveCard";
import type { ApprovalRequest } from "../approval/types";
import { LaneRow } from "./LaneRow";
import { laneHeading, laneLayout, orderLane, pending } from "./laneLayout";
import "./Lane.css";

/**
 * The needs-you lane (docs/PLAN.md "Layout"). The header is always present so the lane never pops in or out. The
 * oldest waiting request is the target and shows as a full card; the rest are one-liners. It is capped at 40% of the
 * sidebar and never scrolls: it shows whole rows and a "+N more" button opens the rest in a menu.
 */
export function Lane({ requests, sidebarHeight }: { requests: ApprovalRequest[]; sidebarHeight: number }) {
  const ordered = orderLane(pending(requests));
  const layout = laneLayout(ordered.length, sidebarHeight);
  const heading = laneHeading(ordered.length);
  const shown = ordered.slice(0, layout.visible);
  return (
    <section className="lane" aria-label="Needs you" style={{ blockSize: layout.height }} data-clipped={layout.clipped || undefined} data-count={ordered.length}>
      <header className="lane__header">
        <div role="status">
          <h2 className="lane__heading">{heading}</h2>
        </div>
        {layout.hiddenCount > 0 && (
          <button type="button" className="lane__more" aria-label={`+${layout.hiddenCount} more, show all ${ordered.length} requests`} aria-haspopup="dialog">
            +{layout.hiddenCount} more
          </button>
        )}
      </header>
      {shown.length > 0 && (
        <div className="lane__list" role="list">
          {shown.map((r, i) => (
            <div key={r.id} role="listitem" className="lane__item">
              {i === 0 && layout.expanded ? <ApproveCard request={r} /> : <LaneRow request={r} />}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
