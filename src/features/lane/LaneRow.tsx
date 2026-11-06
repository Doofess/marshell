import { StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { headline } from "../approval/headline";
import type { ApprovalRequest } from "../approval/types";
import { formatAge } from "../sidebar/format";

/** A waiting request that is not the target: a 36 px one-liner. */
export function LaneRow({ request, onJump }: { request: ApprovalRequest; onJump?: (request: ApprovalRequest) => void }) {
  // A session asking for permission in its own terminal (Copilot CLI's every request) shows its row's phrase, not "Has a question".
  const { detail } = request;
  const terminalPermission = request.asks === "permission" && detail.kind === "question";
  const isQuestion = detail.kind === "question" && !terminalPermission;
  const what = terminalPermission ? detail.question || "Needs permission" : headline(detail);
  return (
    <button type="button" className="lane-row" aria-label={`${request.session.name} ${what} ${formatAge(request.waitingMs)}. ${isQuestion ? "Has a question" : "Needs permission"}.`} data-agent={request.session.agent} onClick={() => onJump?.(request)}>
      <StatusGlyph kind={isQuestion ? "needs-question" : "needs-permission"} />
      <span className="lane-row__name" dir="auto">
        {request.session.name}
      </span>{" "}
      <span className="lane-row__what" dir="auto">
        {what}
      </span>{" "}
      <span className="lane-row__time">{formatAge(request.waitingMs)}</span>
    </button>
  );
}
