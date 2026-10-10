import { afterFixMessage } from "./report";
import { toastVisible, type UndoState } from "./undo";
import "./Doctor.css";

/**
 * The receipt after a fix: what was fixed, which sessions pick it up after a restart, and the two ways forward. It floats at
 * the foot of the page for 10 seconds, then Undo last fix stays in the Doctor header. It is plain markup, not a live region:
 * the window's one announcer says the same text (doctorUi.fixAnnouncement).
 */
export function UndoToast({ undo, nowMs, restart }: { undo: UndoState; nowMs: number; restart: number[] }) {
  if (!toastVisible(undo, nowMs)) return null;
  return (
    <div className="toast" data-kind="fixed">
      <p className="toast__text">{afterFixMessage(undo.count, restart)}</p>
      <div className="toast__actions">
        {restart.length > 0 && (
          <button type="button" className="toast__btn">
            Restart them
          </button>
        )}
        <button type="button" className="toast__btn">
          Undo
        </button>
      </div>
    </div>
  );
}
