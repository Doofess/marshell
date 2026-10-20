import type { ReactNode } from "react";
import { ApproveCard } from "../approval/ApproveCard";
import type { ApprovalRequest } from "../approval/types";
import { LaneRow } from "./LaneRow";
import { LANE, laneHeading, laneLayout, nextWaiting, orderLane, pending } from "./laneLayout";
import "./Lane.css";

/**
 * The needs-you lane (docs/PLAN.md "Layout"). The header is always present so the lane never pops in or out. The
 * oldest waiting request that a card can answer is the target and shows as a full card; the rest are one-liners (a session that asks in
 * its own terminal is only ever a one-liner). Receipts of answers just given show above them and are not counted. It is capped at 40% of the
 * sidebar and never scrolls: it shows whole rows and a "+N more" button opens the rest in a menu.
 */
export function Lane({
  requests,
  sidebarHeight,
  renderCard = (r) => <ApproveCard request={r} />,
  onJump,
}: {
  requests: ApprovalRequest[];
  sidebarHeight: number;
  /** The full card for the target. Defaults to the static card. */
  renderCard?: (request: ApprovalRequest) => ReactNode;
  /** A one-liner was chosen: go to its session. */
  onJump?: (request: ApprovalRequest) => void;
}) {
  const waiting = orderLane(pending(requests));
  const next = nextWaiting(requests);
  const target = next?.kind === "card" ? next.request : undefined;
  const ordered = target ? [target, ...waiting.filter((r) => r !== target)] : waiting;
  const settled = requests.filter((r) => r.state.phase !== "pending");
  const layout = laneLayout(ordered.length, sidebarHeight);
  const heading = laneHeading(ordered.length);
  const shown = ordered.slice(0, layout.visible);
  return (
    <section className="lane" aria-label="Needs you" style={{ blockSize: layout.height + settled.length * LANE.collapsed }} data-clipped={layout.clipped || undefined} data-count={ordered.length}>
      <header className="lane__header">
        <h2 className="lane__heading">{heading}</h2>
        {layout.hiddenCount > 0 && (
          <button type="button" className="lane__more" aria-label={`+${layout.hiddenCount} more, show all ${ordered.length} requests`} aria-haspopup="dialog">
            +{layout.hiddenCount} more
          </button>
        )}
      </header>
      {(shown.length > 0 || settled.length > 0) && (
        <div className="lane__list" role="list">
          {settled.map((r) => (
            <div key={r.id} role="listitem" className="lane__item">
              <ApproveCard request={r} />
            </div>
          ))}
          {shown.map((r, i) => (
            <div key={r.id} role="listitem" className="lane__item">
              {i === 0 && layout.expanded && !r.terminalOnly ? renderCard(r) : <LaneRow request={r} onJump={onJump} />}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
