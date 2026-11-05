import { afterFixMessage } from "./report";
import type { DoctorLastFix } from "./types";
import { toastVisible, undoState } from "./undo";
import "./Doctor.css";

/**
 * The receipt after a fix: what was fixed, how many sessions pick it up after a restart, and the two ways forward. It floats at
 * the foot of the page for 10 seconds, then Undo last fix stays in the Doctor header. Undo comes first, so a keyboard user
 * reaches it before Restart them, and it is withdrawn when a file changed since the fix (the page says why). Restart them
 * appears only when sessions need one. It is plain markup, not a live region: the window's one announcer says the same text
 * (doctorUi.fixAnnouncement).
 */
export function UndoToast({ undo, nowMs, onUndo, onRestart }: { undo: DoctorLastFix; nowMs: number; onUndo: () => void; onRestart: () => void }) {
  if (!toastVisible(undoState(undo), nowMs)) return null;
  const hasActions = undo.files_match || undo.restart.length > 0;
  return (
    <div className="toast" data-kind="fixed">
      <p className="toast__text">{afterFixMessage(undo.count, undo.restart, undo.item_ids)}</p>
      {hasActions && (
        <div className="toast__actions">
          {undo.files_match && (
            <button type="button" className="toast__btn" onClick={onUndo}>
              Undo
            </button>
          )}
          {undo.restart.length > 0 && (
            <button type="button" className="toast__btn" onClick={onRestart}>
              Restart them
            </button>
          )}
        </div>
      )}
    </div>
  );
}
