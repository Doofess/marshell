import { useEffect, useState } from "react";
import { Switch } from "../../components/Switch/Switch";
import { failureText } from "../agents/useInstallFlow";
import { SettingsReadError, type SettingsSync } from "../appearance/settingsSync";
import { SETTINGS_UNREADABLE } from "../notifications/NotificationsSection";
import { StorageSection } from "./StorageSection";
// The Notifications section's styles: the same look, so this section adds none of its own.
import "../notifications/NotificationsSection.css";

export const HISTORY_IMPORT_LABEL = "Copy conversations run outside Marshell";
export const HISTORY_IMPORT_HINT = "Marshell reads Claude Code's and Codex's conversation folders and keeps its own copy of each conversation. It never changes their files.";

/** Settings → History: the one switch for copying conversations run outside Marshell. `on` is null until the setting has been read, and no switch shows then (an unread value is never shown as off). */
export function HistorySettingsView({ on, error, sync, announce, onChange }: { on: boolean | null; error: string | null; sync?: SettingsSync; announce?: (text: string) => void; onChange: (on: boolean) => void }) {
  return (
    <div className="history-settings">
      {on === null && !error && <p className="notifications-dialog__line">Reading your settings…</p>}
      {on !== null && <Switch label={HISTORY_IMPORT_LABEL} checked={on} onChange={onChange} reason={HISTORY_IMPORT_HINT} />}
      {on !== null && <StorageSection sync={sync} announce={announce} />}
      {error && <p className="notifications-dialog__error">{error}</p>}
    </div>
  );
}

/** The History section on the app's settings. The switch is saved through `SettingsSync`, so a later appearance save keeps it; the core starts or stops the copy when it sees the saved value. */
export function HistorySection({ sync, announce }: { sync?: SettingsSync; announce?: (text: string) => void }) {
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
  return <HistorySettingsView on={on} error={error} sync={sync} announce={announce} onChange={(next) => void save(next)} />;
}
