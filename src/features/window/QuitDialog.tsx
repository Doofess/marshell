import { useEffect, useId, useRef } from "react";
import { APP_NAME } from "../../generated/constants";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { useCoverPage } from "../browser/pageCover";
import "./CloseDialog.css";
import "./QuitDialog.css";

/** The question, in the number of pushes that run. */
export const quitQuestion = (count: number) => (count === 1 ? "A push is still running. Quit and stop it?" : `${count.toLocaleString("en-US")} pushes are still running. Quit and stop them?`);
/** What the dialog says when the pushes could not be stopped; the owner announces it as well. */
export const quitStopFailed = (count: number) => `Couldn't stop the ${count === 1 ? "push" : "pushes"}. Try again.`;

/**
 * What a quit asks while a push runs. Presentational, like `CloseDialog`: the owner stops the pushes and then quits. Cancel is the
 * default and takes the focus; Escape, or a click outside, cancels. While the pushes are being stopped both buttons are off
 * (`aria-disabled`, still in the tab order) and Escape does nothing. A failure is a sentence that takes focus; the owner announces it.
 */
export function QuitDialog({ count, busy, error, onQuit, onCancel }: { count: number; busy: boolean; error: string | null; onQuit: () => void; onCancel: () => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const failure = useRef<HTMLParagraphElement>(null);
  const title = useId();
  const description = useId();
  const cancelUnlessBusy = () => {
    if (!busy) onCancel();
  };
  useCoverPage();
  // Cancel takes the focus instead of the title: it is the choice Enter should make.
  useDialogFocus(dialog, cancel, cancelUnlessBusy);
  useEffect(() => {
    if (error !== null) failure.current?.focus();
  }, [error]);
  return (
    <div className="close-dialog">
      <div className="close-dialog__backdrop" onClick={cancelUnlessBusy} />
      <div ref={dialog} className="close-dialog__sheet quit-dialog" role="dialog" aria-modal="true" aria-labelledby={title} aria-describedby={description}>
        <h2 id={title} className="close-dialog__title">
          Quit {APP_NAME}?
        </h2>
        <p id={description} className="close-dialog__line">
          {quitQuestion(count)}
        </p>
        {error !== null && (
          <p ref={failure} className="close-dialog__error quit-dialog__error" tabIndex={-1} data-focus="">
            {error}
          </p>
        )}
        <footer className="close-dialog__foot">
          <button ref={cancel} type="button" className="close-dialog__btn" data-default aria-disabled={busy || undefined} onClick={cancelUnlessBusy}>
            Cancel
          </button>
          <button type="button" className="close-dialog__btn" aria-disabled={busy || undefined} onClick={() => !busy && onQuit()}>
            Quit
          </button>
        </footer>
      </div>
    </div>
  );
}
