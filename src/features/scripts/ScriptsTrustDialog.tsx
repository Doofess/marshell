import { useRef } from "react";
import type { RepoScripts } from "../../generated/RepoScripts";
import { useDialogFocus } from "../../lib/useDialogFocus";
import "../window/CloseDialog.css";
import { ScriptsTrust } from "./ScriptsTrust";

/**
 * The trust card as a dialog over the window (the header's Run, or the palette's, for a file not yet allowed). It uses the close
 * dialog's sheet. Focus goes to the heading, not a button; Escape or a press outside is Cancel; focus then returns to what opened it.
 */
export function ScriptsTrustDialog({ scripts, busy, error, allowLabel, cancelLabel, onAllow, onCancel }: { scripts: RepoScripts; busy: boolean; error: string | null; allowLabel: string; cancelLabel: string; onAllow: () => void; onCancel: () => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  useDialogFocus(dialog, heading, onCancel);
  return (
    <div className="close-dialog">
      <div className="close-dialog__backdrop" onClick={() => !busy && onCancel()} />
      <div ref={dialog} className="close-dialog__sheet" role="dialog" aria-modal="true" aria-label="Allow this repo's scripts?">
        <ScriptsTrust scripts={scripts} allowLabel={allowLabel} cancelLabel={cancelLabel} busy={busy} headingRef={heading} onAllow={onAllow} onCancel={onCancel} />
        {error && (
          <p className="close-dialog__error">
            {error}
          </p>
        )}
      </div>
    </div>
  );
}
