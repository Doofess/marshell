import { useId, useState } from "react";
import { BYPASS_ACK_TEXT } from "../../lib/adminCopy";
import { resumeLabel } from "../sidebar/sessionMenu";
import type { RowModel } from "../sidebar/types";
import { ENDED_PHRASE } from "./toRow";
import "./ResumeBar.css";
import "./CloseBar.css";
import { restartAllowed } from "./RestartBar";

/** What the bar says about the tab: an Ended one has no process at all, an Exited one finished with a code. */
export const resumeBarStatus = (row: RowModel): string => (row.phrase === ENDED_PHRASE ? "Ended last time. Nothing is running." : `${row.phrase}.`);

/**
 * The strip an Ended or Exited tab shows (under the old terminal for an Exited one): what happened, the exact command Resume
 * will run and where, and the one button. It is not a live region: the result of pressing Resume goes through the window's
 * announcer. A refused Resume leaves the button where it is, so the keys stay on it; with `disabledReason` the button is
 * aria-disabled (still focusable, so the reason can be read) and does nothing.
 */
export function ResumeBar({ row, command, folder, disabledReason, needsAck = false, onResume }: { row: RowModel; command: string; folder: string; disabledReason?: string; needsAck?: boolean; onResume: (acknowledged: boolean) => void }) {
  const id = useId();
  const [ticked, setTicked] = useState(false);
  const commandId = `${id}-command`;
  const reasonId = `${id}-reason`;
  const ackId = `${id}-ack`;
  const allowed = !disabledReason && restartAllowed(needsAck, ticked);
  return (
    <div className="resume-bar">
      <p className="resume-bar__status">{resumeBarStatus(row)}</p>
      <p className="resume-bar__command" id={commandId} dir="auto">
        Resume runs <code className="resume-bar__code">{command}</code> in <span className="resume-bar__folder">{folder}</span>
      </p>
      {needsAck && (
        <label className="close-bar__ack" htmlFor={ackId}>
          <input id={ackId} type="checkbox" checked={ticked} onChange={(e) => setTicked(e.target.checked)} />
          <span>{BYPASS_ACK_TEXT}</span>
        </label>
      )}
      <button
        type="button"
        className="resume-bar__btn"
        data-primary
        aria-describedby={disabledReason ? `${commandId} ${reasonId}` : commandId}
        aria-disabled={allowed ? undefined : true}
        onClick={() => {
          if (allowed) onResume(ticked);
        }}
      >
        {resumeLabel(row.elevated) ?? "Resume"}
      </button>
      {disabledReason && (
        <p className="resume-bar__reason" id={reasonId}>
          {disabledReason}
        </p>
      )}
    </div>
  );
}
