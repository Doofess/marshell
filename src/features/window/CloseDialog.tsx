import { useId, useRef } from "react";
import { useDialogFocus } from "../../lib/useDialogFocus";
import "./CloseDialog.css";

/**
 * The one-time question the close button asks when nothing is saved yet. Presentational: the owner saves the choice and then
 * closes the window. "Keep running in the tray" is the default and takes the focus; Escape, or a click outside, only closes the dialog.
 */
export function CloseDialog({ busy, error, onChoose, onCancel }: { busy: boolean; error: string | null; onChoose: (keepInTray: boolean) => void; onCancel: () => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  const keep = useRef<HTMLButtonElement>(null);
  const title = useId();
  const description = useId();
  // The default button takes the focus instead of the title: it is the choice Enter should make.
  useDialogFocus(dialog, keep, onCancel);
  return (
    <div className="close-dialog">
      <div className="close-dialog__backdrop" onClick={onCancel} />
      <div ref={dialog} className="close-dialog__sheet" role="dialog" aria-modal="true" aria-labelledby={title} aria-describedby={description}>
        <h2 id={title} className="close-dialog__title">
          When you close the window
        </h2>
        <p id={description} className="close-dialog__line">Marshell keeps running in the tray, so your sessions keep going.</p>
        {error && (
          <p className="close-dialog__error" role="alert">
            {error}
          </p>
        )}
        <footer className="close-dialog__foot">
          <button ref={keep} type="button" className="close-dialog__btn" data-default disabled={busy} onClick={() => onChoose(true)}>
            Keep running in the tray
          </button>
          <button type="button" className="close-dialog__btn" disabled={busy} onClick={() => onChoose(false)}>
            Quit when I close
          </button>
        </footer>
      </div>
    </div>
  );
}
