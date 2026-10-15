import { DiffView } from "../../components/DiffView/DiffView";
import { changeSummary, previewChange } from "./edit";
import type { SettingsState } from "./settingsUi";
import "./Settings.css";

/** What the strip's buttons do when it is a plain Save and Discard. */
export type SaveActions = { busy?: boolean; onSave: () => void; onDiscard: () => void };

/**
 * The strip under the form: how many changes are waiting, and Review changes. The strip stays small and sticky so it never covers
 * the form; reviewing opens the exact change as a diff, with where the backup goes and how to undo, in the page above it, then Apply and Discard. Apply is absent when the file must not be written.
 * With nothing that really changes (every edit equals the current value) there is nothing to review or apply, so there is no strip.
 * With `save` the strip is a plain Save and Discard (the profiles page, which has no file to diff), and `error` is the core's refusal, word for word.
 */
export function ChangeBar({ path, opened, edits, count, state, save, error }: { path: string; opened: string; edits?: Record<string, unknown>; count?: number; state: SettingsState; save?: SaveActions; error?: string | null }) {
  // A file that does not parse has nothing to review and nothing to apply: the page says why at its top instead.
  if (!state.changed || state.unparsable || count === 0) return null;
  const writable = !state.readOnly && !state.unparsable && !state.conflict;
  return (
    <>
      {!save && state.reviewing && edits && (
        <section className="change-review" aria-label="Review of the changes">
          <DiffView diff={previewChange(path, opened, edits)} label={`Changes to ${path}`} />
          <p className="change-bar__note">
            A backup is saved first, in <code>~/.marshell/snapshots/</code>. To undo, Marshell puts that backup back, for as long as the file is unchanged since this change.
          </p>
        </section>
      )}
      <section className="change-bar" aria-label="Pending changes">
        <p className="change-bar__summary">{count === undefined ? "Changes to the file" : changeSummary(count)}</p>
        {error && (
          <p className="change-bar__error">
            {error}
          </p>
        )}
        <div className="change-bar__actions">
          {save && (
            <>
              <button type="button" className="settings__btn" data-primary disabled={save.busy} onClick={save.onSave}>
                {save.busy ? "Saving…" : "Save"}
              </button>
              <button type="button" className="settings__btn" disabled={save.busy} onClick={save.onDiscard}>
                Discard
              </button>
            </>
          )}
          {!save && !state.reviewing && (
            <button type="button" className="settings__btn">
              Review changes
            </button>
          )}
          {!save && state.reviewing && writable && (
            <button type="button" className="settings__btn" data-primary>
              Apply
            </button>
          )}
          {!save && (
            <button type="button" className="settings__btn">
              Discard
            </button>
          )}
        </div>
      </section>
    </>
  );
}
