import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { headline } from "../approval/headline";
import type { ApprovalRequest } from "../approval/types";
import { formatDuration } from "../sidebar/format";

/** A waiting request that is not the target: a 36 px one-liner. */
export function LaneRow({ request }: { request: ApprovalRequest }) {
  const isQuestion = request.detail.kind === "question";
  const what = headline(request.detail);
  return (
    <button type="button" className="lane-row" aria-label={`${request.session.name}: ${what}, waiting ${formatDuration(request.waitingMs)}`} data-agent={request.session.agent}>
      <StatusGlyph kind={isQuestion ? "needs-question" : "needs-permission"} />
      <span className="lane-row__name" dir="auto">
        {request.session.name}
      </span>
      <span className="lane-row__what" dir="auto">
        {what}
      </span>
      <span className="lane-row__time">{formatDuration(request.waitingMs)}</span>
    </button>
  );
}
