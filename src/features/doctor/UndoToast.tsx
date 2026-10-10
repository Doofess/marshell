import { afterFixMessage } from "./report";
import { toastVisible, type UndoState } from "./undo";
import "./Doctor.css";

/**
 * The receipt after a fix: what was fixed, which sessions pick it up after a restart, and the two ways forward. It floats at
 * the foot of the page for 10 seconds, then Undo last fix stays in the Doctor header. Undo comes first, so a keyboard user
 * reaches it before Restart them, and it is withdrawn when a file changed since the fix (the page says why). With undo
 * withdrawn and nothing to restart, the toast is the text alone. It is plain markup, not a live region: the window's one
 * announcer says the same text (doctorUi.fixAnnouncement).
 */
export function UndoToast({ undo, nowMs, restart }: { undo: UndoState; nowMs: number; restart: number[] }) {
  if (!toastVisible(undo, nowMs)) return null;
  const hasActions = undo.filesMatch || restart.length > 0;
  return (
    <div className="toast" data-kind="fixed">
      <p className="toast__text">{afterFixMessage(undo.count, restart)}</p>
      {hasActions && (
        <div className="toast__actions">
          {undo.filesMatch && (
            <button type="button" className="toast__btn">
              Undo
            </button>
          )}
          {restart.length > 0 && (
            <button type="button" className="toast__btn">
              Restart them
            </button>
          )}
        </div>
      )}
    </div>
  );
}
