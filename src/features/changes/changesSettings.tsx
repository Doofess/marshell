import { useCallback, useSyncExternalStore } from "react";
import type { ChangesSettings } from "../../generated/ChangesSettings";
import { useAppearance } from "../appearance/AppearanceProvider";

export const CHANGES_DEFAULTS: ChangesSettings = { hide_whitespace: false, wrap: false, pull_requests: true, resolved_comments: false };

const never = () => () => {};

/** The Changes settings from the app's `SettingsSync`: `settings` is null until they were read, `failed` after a read that failed. */
export function useChangesSettings(): { settings: ChangesSettings | null; failed: boolean; set(next: Partial<ChangesSettings>): Promise<void> } {
  const { sync } = useAppearance();
  const subscribe = useCallback((l: () => void) => (sync ? sync.subscribe(l) : never()), [sync]);
  const settings = useSyncExternalStore(subscribe, () => sync?.changes ?? null, () => null);
  const failed = useSyncExternalStore(subscribe, () => sync?.readFailed ?? false, () => false);
  const set = useCallback((next: Partial<ChangesSettings>) => (sync ? sync.setChanges(next) : Promise.resolve()), [sync]);
  return { settings, failed, set };
}
