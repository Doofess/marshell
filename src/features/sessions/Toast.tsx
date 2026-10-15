import "../doctor/Doctor.css";
import "./Toast.css";

/**
 * A one-line receipt at the foot of the window ("Closed Shell 1." with Reopen). It uses the Doctor's toast look. It is plain
 * markup, not a live region: the window's one announcer says the same text (MainWindow `notice`).
 */
export function Toast({ text, actionLabel, onAction }: { text: string; actionLabel?: string; onAction?: () => void }) {
  return (
    <div className="toast" data-placement="window">
      <p className="toast__text">{text}</p>
      {actionLabel && (
        <div className="toast__actions">
          <button type="button" className="toast__btn" onClick={onAction}>
            {actionLabel}
          </button>
        </div>
      )}
    </div>
  );
}
