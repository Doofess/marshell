import { type Ref, useEffect, useRef, useState } from "react";
import type { DefaultFolder } from "../../generated/DefaultFolder";
import type { Endpoint } from "../../generated/Endpoint";
import { getDefaultFolder } from "../../lib/api/defaultFolder";
import { pickFolder } from "../../lib/folderPicker";
import type { SettingsSync } from "../appearance/settingsSync";
import { chooseBackupFolder, storageErrorText } from "../history/storageActions";
import { lastSegment } from "../launcher/startFolder";
import "./Settings.css";
import "../notifications/NotificationsSection.css";
import "../history/StorageSection.css";
import "./DefaultFolderRow.css";

const DASH = "–";
const LABEL_ID = "default-folder-label";
export const DEFAULT_FOLDER_HELP = "Where the shell panel and new sessions start when no folder is chosen and the shell profile sets none.";
/** Said only for Marshell's own folder: a folder the person chose is never made. */
export const MAKES_IT = "Marshell makes it when it's missing.";
export const MISSING_LINE = "This folder isn't there now, so shells and sessions start in your home folder.";
export const UNREADABLE = "Couldn't read the default folder. Try again in a moment.";

export type DefaultFolderRowViewProps = {
  /** Null until the core has answered, or when it could not be read: the path shows an en dash. */
  folder: DefaultFolder | null;
  error: string | null;
  busy: boolean;
  chooseRef?: Ref<HTMLButtonElement>;
  onChoose: () => void;
  onReset: () => void;
};

/** Settings → Shell profiles → Default folder: where the shell panel and new sessions start when nothing else names a folder. */
export function DefaultFolderRowView({ folder, error, busy, chooseRef, onChoose, onReset }: DefaultFolderRowViewProps) {
  const path = folder === null ? DASH : (folder.path ?? "Your home folder");
  return (
    <div className="default-folder">
      <div className="storage__field" role="group" aria-labelledby={LABEL_ID}>
        <span className="field__label" id={LABEL_ID}>
          Default folder
        </span>
        <span className="storage__path">{path}</span>
        <div className="storage__actions">
          <button type="button" ref={chooseRef} className="notifications-dialog__btn" disabled={busy} onClick={onChoose}>
            Choose folder…
          </button>
          {folder?.chosen && (
            <button type="button" className="notifications-dialog__btn" disabled={busy} onClick={onReset}>
              Use Marshell's folder
            </button>
          )}
        </div>
      </div>
      <p className="field__help">{folder && !folder.chosen && folder.path ? `${DEFAULT_FOLDER_HELP} ${MAKES_IT}` : DEFAULT_FOLDER_HELP}</p>
      {folder?.path && !folder.exists && <p className="notifications-dialog__line">{MISSING_LINE}</p>}
      {error && <p className="notifications-dialog__error">{error}</p>}
    </div>
  );
}

/** The default folder row on Settings. A choice is saved at once through `SettingsSync`; the folder is read from the core again after each save. */
export function DefaultFolderRow({ endpoint, sync, announce }: { endpoint: Endpoint; sync: SettingsSync | undefined; announce?: (text: string) => void }) {
  const [folder, setFolder] = useState<DefaultFolder | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [refocus, setRefocus] = useState(0);
  const chooseRef = useRef<HTMLButtonElement>(null);
  const live = useRef(true);

  const read = async () => {
    try {
      const next = await getDefaultFolder(endpoint);
      if (live.current) setFolder(next);
      return next;
    } catch {
      if (live.current) {
        setFolder(null);
        setError(UNREADABLE);
      }
      return null;
    }
  };
  useEffect(() => {
    live.current = true;
    void read();
    return () => {
      live.current = false;
    };
  }, [endpoint]);
  // Pressing a button disables it, and Use Marshell's folder goes away: focus goes back to Choose folder once the row is enabled.
  useEffect(() => {
    if (refocus > 0 && !busy) chooseRef.current?.focus();
  }, [refocus, busy]);

  const run = async (work: () => Promise<void>) => {
    setError(null);
    setBusy(true);
    try {
      await work();
    } catch (e) {
      if (live.current) setError(storageErrorText(e));
    } finally {
      if (live.current) {
        setBusy(false);
        setRefocus((n) => n + 1);
      }
    }
  };

  return (
    <DefaultFolderRowView
      folder={folder}
      error={error}
      busy={busy}
      chooseRef={chooseRef}
      onChoose={() =>
        void run(async () => {
          if (!sync) return;
          let chosen: string | null = null;
          const saved = await chooseBackupFolder(pickFolder, async (dir) => {
            chosen = dir;
            await sync.setDefaultFolder(dir);
          }, folder?.path ?? null);
          if (!saved) return;
          await read();
          if (chosen !== null) announce?.(`Default folder set to ${lastSegment(chosen)}.`);
        })
      }
      onReset={() =>
        void run(async () => {
          if (!sync) return;
          await sync.setDefaultFolder(null);
          await read();
          announce?.("Default folder set back to Marshell's own.");
        })
      }
    />
  );
}
