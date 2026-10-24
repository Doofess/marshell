import { useEffect, useRef, useState } from "react";
import { Switch } from "../../components/Switch/Switch";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { failureText } from "../agents/useInstallFlow";
import { SettingsReadError, type SettingsSync } from "../appearance/settingsSync";
import { SETTINGS_UNREADABLE } from "../notifications/NotificationsDialog";
import { StorageSection } from "./StorageSection";
// The Notifications dialog's sheet: the same look and stacking, so this dialog adds no styles of its own.
import "../notifications/NotificationsDialog.css";

export const HISTORY_IMPORT_LABEL = "Copy conversations run outside Marshell";
export const HISTORY_IMPORT_HINT = "Marshell reads Claude Code's and Codex's conversation folders and keeps its own copy of each conversation. It never changes their files.";

/** Settings → History: the one switch for copying conversations run outside Marshell. `on` is null until the setting has been read, and no switch shows then (an unread value is never shown as off). */
export function HistorySettingsDialogView({ on, error, sync, announce, onChange, onClose }: { on: boolean | null; error: string | null; sync?: SettingsSync; announce?: (text: string) => void; onChange: (on: boolean) => void; onClose: () => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  const title = useRef<HTMLHeadingElement>(null);
  useDialogFocus(dialog, title, onClose);
  return (
    <div className="notifications-dialog">
      <div className="notifications-dialog__backdrop" onClick={onClose} />
      <div ref={dialog} className="notifications-dialog__sheet" role="dialog" aria-modal="true" aria-label="History settings">
        <h2 ref={title} tabIndex={-1} className="notifications-dialog__title">
          History settings
        </h2>
        <div className="notifications-dialog__body">
          {on === null && !error && <p className="notifications-dialog__line">Reading your settings…</p>}
          {on !== null && <Switch label={HISTORY_IMPORT_LABEL} checked={on} onChange={onChange} reason={HISTORY_IMPORT_HINT} />}
          {on !== null && <StorageSection sync={sync} announce={announce} />}
          {error && (
            <p className="notifications-dialog__error">
              {error}
            </p>
          )}
        </div>
        <footer className="notifications-dialog__foot">
          <button type="button" className="notifications-dialog__btn" onClick={onClose}>
            Close
          </button>
        </footer>
      </div>
    </div>
  );
}

/** The History settings dialog on the app's settings. The switch is saved through `SettingsSync`, so a later appearance save keeps it; the core starts or stops the copy when it sees the saved value. */
export function HistorySettingsDialog({ sync, announce, onClose }: { sync?: SettingsSync; announce?: (text: string) => void; onClose: () => void }) {
  const [on, setOn] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    sync?.load().then(
      () => live && setOn(sync.historyImport),
      () => live && setError(SETTINGS_UNREADABLE),
    );
    return () => {
      live = false;
    };
  }, [sync]);
  const save = async (next: boolean) => {
    if (!sync) return;
    setError(null);
    try {
      await sync.setHistoryImport(next);
      setOn(sync.historyImport);
    } catch (e) {
      setError(e instanceof SettingsReadError ? e.message : failureText(e));
    }
  };
  return <HistorySettingsDialogView on={on} error={error} sync={sync} announce={announce} onChange={(next) => void save(next)} onClose={onClose} />;
}
