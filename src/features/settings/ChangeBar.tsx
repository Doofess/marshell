import { DiffView } from "../../components/DiffView/DiffView";
import { changeSummary, previewChange } from "./edit";
import type { SettingsState } from "./settingsUi";
import "./Settings.css";

/**
 * The strip under the form: how many changes are waiting, and Review changes. Reviewing shows the exact change as a diff,
 * where the backup goes and how to undo, then Apply and Discard. Apply is absent when the file must not be written.
 */
export function ChangeBar({ path, opened, edits, state }: { path: string; opened: string; edits?: Record<string, unknown>; state: SettingsState }) {
  if (!state.changed) return null;
  const n = edits ? Object.keys(edits).length : 1;
  const writable = !state.readOnly && !state.unparsable && !state.conflict;
  return (
    <section className="change-bar" aria-label="Pending changes">
      <p className="change-bar__summary">{changeSummary(n)}</p>
      {state.reviewing && edits && (
        <>
          <DiffView diff={previewChange(path, opened, edits)} label={`Changes to ${path}`} />
          <p className="change-bar__note">
            A backup is saved first, in <code>~/.marshell/snapshots/</code>. To undo: Settings → Advanced → Snapshots restores the file as it was.
          </p>
        </>
      )}
      <div className="change-bar__actions">
        {!state.reviewing && (
          <button type="button" className="settings__btn">
            Review changes
          </button>
        )}
        {state.reviewing && writable && (
          <button type="button" className="settings__btn" data-primary>
            Apply
          </button>
        )}
        <button type="button" className="settings__btn">
          Discard
        </button>
      </div>
    </section>
  );
}
