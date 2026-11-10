import { useId, useRef } from "react";
import { useDialogFocus } from "../../lib/useDialogFocus";
import "../window/CloseDialog.css";
import { useCoverPage } from "./pageCover";

/**
 * "Clear the browser's data…" asks first. Presentational: the owner clears and then closes it. Focus goes to the heading, not a
 * button, so Enter cannot clear by reflex; Escape, a press outside and Cancel only close the dialog.
 */
export function ClearDataDialog({ busy, onClear, onCancel }: { busy: boolean; onClear: () => void; onCancel: () => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const title = useId();
  const description = useId();
  useCoverPage();
  useDialogFocus(dialog, heading, onCancel);
  return (
    <div className="close-dialog">
      <div className="close-dialog__backdrop" onClick={() => !busy && onCancel()} />
      <div ref={dialog} className="close-dialog__sheet" role="dialog" aria-modal="true" aria-labelledby={title} aria-describedby={description}>
        <h2 ref={heading} id={title} tabIndex={-1} className="close-dialog__title">
          Clear the browser's data?
        </h2>
        <p id={description} className="close-dialog__line">
          This signs you out of every site in Marshell's browser and clears its cookies, cache and site permissions. Your own browser isn't touched.
        </p>
        <footer className="close-dialog__foot">
          <button type="button" className="close-dialog__btn" data-default disabled={busy} onClick={onClear}>
            Clear data
          </button>
          <button type="button" className="close-dialog__btn" disabled={busy} onClick={onCancel}>
            Cancel
          </button>
        </footer>
      </div>
    </div>
  );
}
