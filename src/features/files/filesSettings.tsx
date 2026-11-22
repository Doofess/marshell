import { useCallback, useSyncExternalStore } from "react";
import type { FilesSettings } from "../../generated/FilesSettings";
import { useAppearance } from "../appearance/AppearanceProvider";

export const FILES_DEFAULTS: FilesSettings = { preview_tabs: true, auto_save: false, tab_indents: true, word_wrap: false, indent_guides: true, whitespace: false, scroll_past_end: false, tab_size: 4, tree_position: "automatic" };

const never = () => () => {};

/** The Files settings from the app's `SettingsSync`: `settings` is null until they were read, `failed` after a read that failed. */
export function useFilesSettings(): { settings: FilesSettings | null; failed: boolean; set(change: Partial<FilesSettings>): Promise<void> } {
  const { sync } = useAppearance();
  const subscribe = useCallback((l: () => void) => (sync ? sync.subscribe(l) : never()), [sync]);
  const settings = useSyncExternalStore(subscribe, () => sync?.files ?? null, () => null);
  const failed = useSyncExternalStore(subscribe, () => sync?.readFailed ?? false, () => false);
  const set = useCallback((change: Partial<FilesSettings>) => (sync ? sync.setFiles(change) : Promise.resolve()), [sync]);
  return { settings, failed, set };
}
