import { useCallback, useSyncExternalStore } from "react";
import type { BrowserSettings } from "../../generated/BrowserSettings";
import { useAppearance } from "../appearance/AppearanceProvider";

export const BROWSER_DEFAULTS: BrowserSettings = { terminal_links: true, open_agent_html: true, cookies: "always" };

const never = () => () => {};

/** The Browser settings from the app's `SettingsSync`: `settings` is null until they were read, `failed` after a read that failed. */
export function useBrowserSettings(): { settings: BrowserSettings | null; failed: boolean; set(change: Partial<BrowserSettings>): Promise<void> } {
  const { sync } = useAppearance();
  const subscribe = useCallback((l: () => void) => (sync ? sync.subscribe(l) : never()), [sync]);
  const settings = useSyncExternalStore(subscribe, () => sync?.browser ?? null, () => null);
  const failed = useSyncExternalStore(subscribe, () => sync?.readFailed ?? false, () => false);
  const set = useCallback((change: Partial<BrowserSettings>) => (sync ? sync.setBrowser(change) : Promise.resolve()), [sync]);
  return { settings, failed, set };
}
